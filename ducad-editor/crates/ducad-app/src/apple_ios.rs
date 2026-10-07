//! Integrasi iPadOS lewat UIKit (`objc2`), dipanggil hanya dari thread utama.
//!
//! Semua yang tidak diberikan winit/eframe di iOS tetapi dibutuhkan
//! aplikasi CAD di tablet:
//!
//! - **Peringatan memori** (`UIApplicationDidReceiveMemoryWarning`) →
//!   [`crate::platform::signal_memory_warning`]; eframe sendiri membuang
//!   `Event::MemoryWarning` winit.
//! - **Files.app**: `UIDocumentPickerViewController` (mode salin) untuk
//!   membuka berkas dari iCloud Drive/penyedia lain, dan
//!   `UIActivityViewController` (share sheet) untuk hasil ekspor.
//! - **Apple Pencil**: ketuk ganda (`UIPencilInteraction`) dan hover
//!   sebelum menyentuh (`UIHoverGestureRecognizer`, Pencil 2/Pro).
//! - **Haptik** ringan saat tool berganti lewat Pencil.
//!
//! Objek ObjC yang harus tetap hidup (delegate, recognizer) disimpan di
//! `thread_local` thread utama; `Retained<MainThreadOnly>` tidak `Send`.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::mpsc::Sender;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly, Message};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{
    NSArray, NSNotification, NSNotificationCenter, NSObject, NSObjectProtocol, NSString, NSURL,
};
use objc2_ui_kit::{
    UIActivityViewController, UIApplication, UIApplicationDidReceiveMemoryWarningNotification,
    UIDocumentPickerDelegate, UIDocumentPickerViewController, UIGestureRecognizerState,
    UIHoverGestureRecognizer, UIPencilInteraction, UIPencilInteractionDelegate,
    UISelectionFeedbackGenerator, UIView, UIViewController,
};
use objc2_uniform_type_identifiers::UTType;

use crate::mobile::PencilEvent;

type Observer = Retained<ProtocolObject<dyn NSObjectProtocol>>;

thread_local! {
    static OBSERVERS: RefCell<Vec<Observer>> = const { RefCell::new(Vec::new()) };
    static ACTIVE_PICKER: RefCell<Option<(Retained<UIDocumentPickerViewController>, Retained<PickerDelegate>)>> =
        const { RefCell::new(None) };
    static PENCIL: RefCell<Option<PencilHandles>> = const { RefCell::new(None) };
}

struct PencilHandles {
    _bridge: Retained<PencilBridge>,
    _interaction: Retained<UIPencilInteraction>,
    _hover: Retained<UIHoverGestureRecognizer>,
}

// ---------------------------------------------------------------------------
// Peringatan memori
// ---------------------------------------------------------------------------

/// Daftarkan observer peringatan memori. Aman dipanggil berkali-kali
/// (observer tambahan hanya menyalakan flag yang sama).
pub fn install_memory_warning_observer() {
    if MainThreadMarker::new().is_none() {
        return;
    }
    let center = NSNotificationCenter::defaultCenter();
    let block = block2::RcBlock::new(|_: NonNull<NSNotification>| {
        crate::platform::signal_memory_warning();
    });
    // SAFETY: nama notifikasi adalah konstanta UIKit yang valid; block hidup
    // selama observer disimpan di `OBSERVERS`.
    let token = unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(UIApplicationDidReceiveMemoryWarningNotification),
            None,
            None,
            &block,
        )
    };
    OBSERVERS.with(|o| o.borrow_mut().push(token));
}

// ---------------------------------------------------------------------------
// Files.app: picker & share sheet
// ---------------------------------------------------------------------------

struct PickerIvars {
    tx: Sender<Option<PathBuf>>,
}

define_class!(
    // SAFETY: NSObject tidak punya syarat subclass; tidak mengimplementasikan Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "DucadDocumentPickerDelegate"]
    #[ivars = PickerIvars]
    struct PickerDelegate;

    unsafe impl NSObjectProtocol for PickerDelegate {}

    unsafe impl UIDocumentPickerDelegate for PickerDelegate {
        #[unsafe(method(documentPicker:didPickDocumentsAtURLs:))]
        fn did_pick(&self, _controller: &UIDocumentPickerViewController, urls: &NSArray<NSURL>) {
            let path = urls
                .firstObject()
                .and_then(|url| url.path())
                .map(|p| PathBuf::from(p.to_string()));
            let _ = self.ivars().tx.send(path);
            finish_picker();
        }

        #[unsafe(method(documentPickerWasCancelled:))]
        fn cancelled(&self, _controller: &UIDocumentPickerViewController) {
            let _ = self.ivars().tx.send(None);
            finish_picker();
        }
    }
);

impl PickerDelegate {
    fn new(mtm: MainThreadMarker, tx: Sender<Option<PathBuf>>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(PickerIvars { tx });
        // SAFETY: init NSObject standar.
        unsafe { msg_send![super(this), init] }
    }
}

fn finish_picker() {
    ACTIVE_PICKER.with(|p| *p.borrow_mut() = None);
}

#[allow(deprecated)] // `windows`: jalur paling sederhana tanpa UIScene.
fn root_view_controller(mtm: MainThreadMarker) -> Option<Retained<UIViewController>> {
    let app = UIApplication::sharedApplication(mtm);
    let windows = app.windows();
    let key = windows
        .iter()
        .find(|w| w.isKeyWindow())
        .or_else(|| windows.firstObject())?;
    key.rootViewController()
}

/// Tampilkan picker Files.app untuk ekstensi yang diberikan. Hasil (atau
/// `None` saat dibatalkan) dikirim lewat `tx`; berkas disalin ke sandbox
/// aplikasi (`asCopy`) sehingga tidak perlu security-scoped bookmark.
/// Mengembalikan `false` bila picker tidak bisa ditampilkan.
pub fn present_document_picker(extensions: &[&str], tx: Sender<Option<PathBuf>>) -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    let types: Vec<Retained<UTType>> = extensions
        .iter()
        .filter_map(|ext| UTType::typeWithFilenameExtension(&NSString::from_str(ext)))
        .collect();
    if types.is_empty() {
        return false;
    }
    let Some(root) = root_view_controller(mtm) else {
        return false;
    };
    let content_types = NSArray::from_retained_slice(&types);
    let picker = UIDocumentPickerViewController::initForOpeningContentTypes_asCopy(
        UIDocumentPickerViewController::alloc(mtm),
        &content_types,
        true,
    );
    let delegate = PickerDelegate::new(mtm, tx);
    picker.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    root.presentViewController_animated_completion(&picker, true, None);
    ACTIVE_PICKER.with(|p| *p.borrow_mut() = Some((picker, delegate)));
    true
}

/// Share sheet untuk satu berkas (hasil ekspor). `anchor` adalah rect
/// (poin egui) tombol pemicu: di iPad share sheet wajib popover.
pub fn share_file(path: &Path, anchor: Option<egui::Rect>) -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    let Some(path_str) = path.to_str() else {
        return false;
    };
    let Some(root) = root_view_controller(mtm) else {
        return false;
    };
    let url = NSURL::fileURLWithPath(&NSString::from_str(path_str));
    let items = NSArray::from_slice(&[&*url as &objc2::runtime::AnyObject]);
    // SAFETY: activity items adalah NSURL berkas yang ada.
    let vc = unsafe {
        UIActivityViewController::initWithActivityItems_applicationActivities(
            UIActivityViewController::alloc(mtm),
            &items,
            None,
        )
    };
    if let (Some(pop), Some(view)) = (vc.popoverPresentationController(), root.view()) {
        pop.setSourceView(Some(&view));
        let rect = match anchor {
            Some(r) => CGRect::new(
                CGPoint::new(f64::from(r.min.x), f64::from(r.min.y)),
                CGSize::new(f64::from(r.width()), f64::from(r.height())),
            ),
            None => {
                let b = view.bounds();
                CGRect::new(
                    CGPoint::new(b.size.width / 2.0, b.size.height - 1.0),
                    CGSize::new(1.0, 1.0),
                )
            }
        };
        pop.setSourceRect(rect);
    }
    root.presentViewController_animated_completion(&vc, true, None);
    true
}

// ---------------------------------------------------------------------------
// Apple Pencil: ketuk ganda + hover
// ---------------------------------------------------------------------------

struct BridgeIvars {
    tx: Sender<PencilEvent>,
    ctx: egui::Context,
    view: Retained<UIView>,
}

define_class!(
    // SAFETY: NSObject tidak punya syarat subclass; tidak mengimplementasikan Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "DucadPencilBridge"]
    #[ivars = BridgeIvars]
    struct PencilBridge;

    unsafe impl NSObjectProtocol for PencilBridge {}

    impl PencilBridge {
        #[unsafe(method(onHover:))]
        fn on_hover(&self, recognizer: &UIHoverGestureRecognizer) {
            let ivars = self.ivars();
            let state = recognizer.state();
            let event = if state == UIGestureRecognizerState::Ended
                || state == UIGestureRecognizerState::Cancelled
                || state == UIGestureRecognizerState::Failed
            {
                PencilEvent::HoverEnded
            } else {
                let p = recognizer.locationInView(Some(&ivars.view));
                PencilEvent::Hover(egui::pos2(p.x as f32, p.y as f32))
            };
            let _ = ivars.tx.send(event);
            ivars.ctx.request_repaint();
        }
    }

    #[allow(deprecated)] // `pencilInteractionDidTap:` masih dipanggil iPadOS 12–18.
    unsafe impl UIPencilInteractionDelegate for PencilBridge {
        #[unsafe(method(pencilInteractionDidTap:))]
        fn did_tap(&self, _interaction: &UIPencilInteraction) {
            let ivars = self.ivars();
            let _ = ivars.tx.send(PencilEvent::DoubleTap);
            ivars.ctx.request_repaint();
        }
    }
);

impl PencilBridge {
    fn new(
        mtm: MainThreadMarker,
        tx: Sender<PencilEvent>,
        ctx: egui::Context,
        view: Retained<UIView>,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(BridgeIvars { tx, ctx, view });
        // SAFETY: init NSObject standar.
        unsafe { msg_send![super(this), init] }
    }
}

/// `UIView` winit dari handle jendela eframe.
fn ui_view_from(cc: &eframe::CreationContext<'_>) -> Option<Retained<UIView>> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = cc.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::UiKit(h) => {
            let ptr = h.ui_view.as_ptr().cast::<UIView>();
            // SAFETY: winit menjamin pointer ini `UIView` yang hidup selama
            // jendela ada; kita me-retain supaya aman disimpan.
            Some(unsafe { &*ptr }.retain())
        }
        _ => None,
    }
}

/// Pasang `UIPencilInteraction` + `UIHoverGestureRecognizer` pada view
/// winit. Event dikirim lewat `tx`; `ctx.request_repaint()` dipanggil
/// supaya frame egui berjalan walau tidak ada sentuhan.
pub fn install_pencil_bridge(
    cc: &eframe::CreationContext<'_>,
    ctx: egui::Context,
    tx: Sender<PencilEvent>,
) -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    let Some(view) = ui_view_from(cc) else {
        return false;
    };
    let bridge = PencilBridge::new(mtm, tx, ctx, view.clone());

    let interaction = UIPencilInteraction::new(mtm);
    interaction.setDelegate(Some(ProtocolObject::from_ref(&*bridge)));
    view.addInteraction(ProtocolObject::from_ref(&*interaction));

    // SAFETY: target `bridge` hidup selama disimpan di `PENCIL`; selector
    // `onHover:` didefinisikan di `PencilBridge`.
    let hover = unsafe {
        UIHoverGestureRecognizer::initWithTarget_action(
            UIHoverGestureRecognizer::alloc(mtm),
            Some(&bridge),
            Some(sel!(onHover:)),
        )
    };
    view.addGestureRecognizer(&hover);

    PENCIL.with(|p| {
        *p.borrow_mut() = Some(PencilHandles {
            _bridge: bridge,
            _interaction: interaction,
            _hover: hover,
        })
    });
    true
}

/// Haptik "pilihan berubah" (tool berganti lewat Pencil).
pub fn haptic_selection_changed() {
    if let Some(mtm) = MainThreadMarker::new() {
        UISelectionFeedbackGenerator::new(mtm).selectionChanged();
    }
}
