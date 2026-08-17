//! Shell desktop CADRAW: jendela eframe + viewport 3D + sketching 2D di
//! bidang XY (Fase 1 + Fase 1 lanjutan).
//!
//! Navigasi kamera:
//! - Drag kiri (tool Pilih) / drag tengah : orbit
//! - Shift+drag / drag kanan              : pan
//! - Scroll / pinch                       : zoom
//! - Dua jari (touch/trackpad)            : orbit + pinch zoom
//!
//! Tool sketch & shortcut: Pilih, Garis (L), Persegi (R), Lingkaran (C),
//! Ellips (E), Arc (A, 3 titik), Offset (O), Mirror (M, perlu seleksi
//! lebih dulu di tool Pilih), Trim (T). Klik menempatkan titik dengan snap
//! otomatis ke endpoint/midpoint/center/intersection/grid; Line/Rectangle/
//! Circle juga menerima dynamic input (ketik panjang/radius + Enter). Esc
//! membatalkan titik pending / kembali ke Pilih. Delete/Backspace menghapus
//! seleksi. Ctrl/Cmd+Z undo, Ctrl/Cmd+Shift+Z atau Ctrl+Y redo.
//!
//! Panel Constraint (Fase 2, kanan layar): muncul saat tool Pilih aktif
//! dan ada 1-2 entitas terpilih, menawarkan tombol constraint yang relevan
//! untuk kombinasi jenis entitas itu (mis. 2 Line terpilih → Sejajar/Tegak
//! Lurus/Sama Panjang/Sudut/Tangent bila salah satu radial). Constraint
//! di-solve langsung (dry-run di atas clone sketch dulu, baru dikirim ke
//! undo stack kalau konvergen — kalau gagal, ditolak dengan pesan status,
//! sketch tidak berubah).
//!
//! Tool pemilihan titik (Fase 2 lanjutan): CoincidentPick — klik 2 titik
//! (endpoint/center, lewat snap) untuk membuat keduanya berimpit; FixedPick
//! — klik 1 titik untuk menahannya di posisi sekarang; SymmetricPick —
//! perlu 1 Line terpilih dulu sebagai sumbu (pola sama dengan Mirror), lalu
//! klik 2 titik yang mau dibuat saling cermin. Titik yang sudah diklik
//! ditandai marker silang ungu (beda dari glyph snap oranye).
//!
//! Panel Model (Fase 3, kanan layar juga — muncul berdampingan/gantian
//! dengan panel Constraint tergantung tool aktif): Extrude membangun
//! `Profile` kernel dari seleksi entitas sketch (1 Circle, atau ≥3
//! Line/Arc yang membentuk loop tertutup — urutan bebas, lihat
//! `model::build_profile_from_selection`) lalu memanggil
//! `cadraw_kernel::extrude_profile`. Union/Subtract butuh 2 body terpilih
//! di daftar body; Fillet/Chamfer semua tepi & Shell/Hollow butuh 1 body.
//! Semua operasi model dry-run dulu (gagal → pesan error, `model` tidak
//! berubah) sebelum masuk `model_undo` — pola yang sama dengan
//! `apply_constraint`. Undo/redo Model TERPISAH dari Sketch (dua tombol
//! sendiri di panel, bukan Ctrl+Z global) — lihat docs/PLAN.md.
//!
//! UX shell (Fase 4, `cadraw-ui`): Ctrl/Cmd+K membuka command palette
//! (cari aksi apa saja — ganti tool, undo/redo, hapus seleksi, toggle
//! tema — lewat substring, Enter eksekusi). Long-press (tekan tahan ~0.4
//! detik tanpa bergerak) di viewport saat tool Pilih aktif membuka radial
//! menu di bawah titik tekan — geser ke slice lalu lepas untuk ganti tool,
//! lepas di zona mati tengah untuk batal; ini jalur utama ganti tool di
//! sentuh (iPad), toolbar tetap ada untuk mouse/trackpad. Menu "⚙
//! Pengaturan" di toolbar mengumpulkan toggle tema (`cadraw_ui::ThemeMode`),
//! pembuka command palette, dan referensi pintasan keyboard — dikumpulkan
//! di satu tempat (bukan tombol lepas di toolbar utama) karena jarang
//! disentuh lebih dari sekali per sesi. Semua widget interaktif punya
//! tinggi minimum 44pt (target sentuh Apple HIG) lewat
//! `cadraw_ui::apply_theme` yang dipanggil sekali di `new()`.
//!
//! Poles & performa (Fase 7): tool "📏 Ukur ▾" — Ukur Jarak (2 klik) & Ukur
//! Sudut (3 klik) — non-destruktif (`cadraw_sketch::measure`, tidak masuk
//! undo stack manapun), hasil digambar permanen (garis kuning) dan
//! didaftar di kartu "📏 Pengukuran" pada panel Properties kanan (sama
//! seperti panel properti lain, bukan jendela mengambang terpisah lagi).
//! Panel "✂ Section View" di panel
//! Model 3D — bidang potong murni efek shader (`SceneRenderer::
//! set_clip_plane`), aman digeser real-time karena TIDAK memanggil kernel
//! OCCT sama sekali (beda dari Boolean). Import STEP sekarang lewat
//! `import_worker` (thread latar belakang, `poll_import_worker` tiap
//! frame di `update()`) supaya UI tidak beku untuk file besar —
//! `KernelShape` sendiri TERBUKTI TIDAK `Send` (dibuktikan lewat
//! compile-time check), jadi worker cuma mengirim `String`/`KernelMesh`
//! lewat channel, dan `cadraw-kernel::KERNEL_LOCK` (baru, produksi bukan
//! cuma test) menyerialkan semua panggilan OCCT lintas thread supaya
//! tidak crash kalau Extrude diklik persis saat import masih jalan —
//! lihat "Status Fase 7" di docs/PLAN.md untuk kenapa background thread
//! BELUM diperluas ke operasi kernel lain (Extrude/Fillet/dst).
//!
//! Modeling 3D lanjutan (Fase 8): tool **Revolve (V)** — pola klik identik
//! Mirror (pilih profil dulu di tool Pilih, lalu 2 klik sumbu 2D), 360°
//! penuh lewat `cadraw_kernel::revolve_profile`. **Loft** (panel Model 3D)
//! — profil bawah di-stage (tombol "Set Profil Bawah dari Seleksi"), profil
//! atas dibaca dari seleksi sketch saat tombol "Loft" diklik, diangkat ke
//! Z=tinggi (BUKAN loft lintas-workplane sungguhan — sketch CADRAW masih
//! satu bidang XY). **Boolean Intersect** — tombol ketiga di baris Boolean,
//! pola sama dengan Union/Subtract. **Picking edge/face 3D** — tombol
//! "Pilih Tepi/Wajah Manual" di section Fillet/Chamfer/Shell (butuh persis
//! 1 body terpilih), klik viewport menambah ke `selected_edges`/
//! `selected_faces` (disimpan sebagai RAY DUNIA, bukan index/handle OCCT —
//! lihat desain di `cadraw_kernel::PickRay`/docs/PLAN.md untuk kenapa);
//! Fillet/Chamfer/Shell memakai seleksi itu kalau tidak kosong, jatuh ke
//! perilaku lama ("semua tepi"/arah otomatis) kalau kosong. Tepi terpilih
//! di-highlight garis oranye; wajah terpilih baru hitungan angka (belum
//! highlight 3D).
//!
//! Lingkup yang sengaja belum digarap (bukan lupa — lihat docs/PLAN.md):
//! spline, fillet 2D, extend, offset untuk Ellipse, toleransi snap adaptif
//! mouse-vs-sentuh presisi, interaksi drag-satu-gesture, browser/penghapus
//! constraint selain lewat Undo, constraint pada titik ujung Arc (PointRef
//! belum mencakupnya), Tangent Line-Line (tak masuk akal secara geometris),
//! sweep sepanjang jalur (gap upstream `opencascade-sys`, bukan CADRAW),
//! Revolve sudut parsial (baru 360°), sketch-on-face (butuh konsep
//! workplane baru, cross-cutting — lihat docs/PLAN.md Fase 8), picking
//! body lewat klik viewport (baru face/edge pada body yang SUDAH terpilih
//! dari daftar panel), toggle-off klik ulang tepi/wajah terpilih (baru
//! tombol "Reset Pilihan"), highlight 3D wajah terpilih, radial menu untuk
//! konteks selain ganti tool (mis. aksi Model 3D), deteksi tema sistem
//! otomatis, pengukuran 3D sungguhan (baru titik sketch 2D), kontrol
//! kualitas tessellation.

mod import_worker;
mod model;

use std::collections::HashSet;
use std::path::PathBuf;

use cadraw_core::{BodyId, LengthUnit};
use cadraw_kernel::{FaceHit, KernelMesh, KernelShape, PickRay, SurfaceKind};
use cadraw_render::camera::ViewPreset;
use cadraw_render::{sketch as sketch_render, LineVertex, OrbitCamera, PlaneKind, SceneRenderer, SketchPlane};
use cadraw_sketch::constraint::{self, AddConstraint, Constraint};
use cadraw_sketch::{
    arc_from_three_points, find_closed_regions, find_region_at_point, find_region_containing_entity,
    find_snap, line_intersection_params_in_sketch, mirror_entity, offset_entity, project_t,
    trim_segments, ClosedRegion, DeleteEntities, Entity, EntityId, InsertEntities, ReplaceEntities,
    Sketch, SnapHit, UpdateEntity,
};
use cadraw_ui::{
    BodyItemInfo, CanvasHud, CanvasHudEvent, CommandPalette, FeatureInspector,
    FeatureInspectorState, InspectorBooleanKind, InspectorConstraintAction, InspectorEvent,
    InspectorPickMode, ItemsDrawer, ItemsDrawerEvent, LeftToolbar, RadialMenu,
    SelectedBodyData, SelectedEntityData, SketchPlaneItemInfo, ThemeMode, ToolbarEvent,
    ToolbarTool, TopBar, TopBarEvent, TopBarFileOp, TopBarState, ViewCube, ViewCubeAction,
};
// Import egui IconData directly
use eframe::egui;
use egui::IconData;
// Image crate for PNG/ICO fallback
use image;
// usvg for SVG parsing
use usvg::{Tree, Options, TreeParsing};
// resvg for converting usvg Tree to renderable tree, dan re-export tiny-skia
// miliknya sendiri (BUKAN dependency `tiny-skia` terpisah — versinya harus
// sama persis dengan yang dipakai resvg internal, kalau tidak `PixmapMut`/
// `Transform` jadi 2 tipe beda secara nominal walau strukturnya identik).
use resvg::Tree as ResvgTree;
use resvg::tiny_skia::{Pixmap, Transform};
use std::fs;
use glam::{DVec2, Mat4, Vec3};
use import_worker::{ImportJob, ImportWorker};
use model::{AddSolidCommand, BodyGeometry, BooleanCommand, BooleanKind, DeleteBodyCommand, ModelDoc, ReplaceGeometryCommand};
use slotmap::Key;

/// Folder yang terlihat lewat Files.app (Fase 6) — bukan lewat API UIKit
/// resmi (`NSSearchPathForDirectoriesInDomains`, yang butuh dependensi
/// bridging tambahan), tapi lewat env var `HOME` yang di iOS ATURAN OS-nya
/// sendiri diarahkan ke root sandbox container app; subfolder "Documents"
/// di dalamnya adalah SATU-SATUNYA folder yang Files.app tampilkan untuk
/// app ini, dan HANYA kalau `Info.plist` app punya
/// `UIFileSharingEnabled=true` + `LSSupportsOpeningDocumentsInPlace=true`
/// (lihat `ios/Info.plist.template` — belum otomatis aktif sampai project
/// Xcode sungguhan menyematkannya, agen tak punya akses Xcode GUI).
#[cfg(target_os = "ios")]
fn ios_documents_dir() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string())).join("Documents")
}

fn main() -> eframe::Result {
    env_logger::init();

    fn load_icon() -> IconData {
        // Asset paths relative to the project root (working directory)
        let svg_path = "images/logo.svg";
        let png_path = "images/logo.png";
        let ico_path = "images/logo_polos.ico";

        // Attempt to load SVG and rasterize it. `tree.size` adalah ukuran
        // INTRINSIK dari `viewBox` SVG (utk logo.svg ini 36184x36184 —
        // artwork vector resolusi tinggi, BUKAN ukuran icon target) —
        // memakainya langsung sebagai ukuran `Pixmap` pernah membuat buffer
        // raster ~5.2 GB dan resvg merender ke kanvas sebesar itu di build
        // debug (butuh ~2 menit sebelum window sempat tampil, ditemukan
        // lewat isolasi: proses `cadraw` tetap hidup & tidak error, cuma
        // lambat — root cause di sini, BUKAN di wgpu). Icon window OS cukup
        // puluhan-ratusan piksel — render langsung ke ukuran target lewat
        // scale transform, jangan pernah rasterisasi di resolusi intrinsik
        // artwork vector.
        const ICON_TARGET_PX: u32 = 256;
        if let Ok(svg_bytes) = fs::read(svg_path) {
            if let Ok(tree) = Tree::from_data(&svg_bytes, &Options::default()) {
                let intrinsic = tree.size;
                let width = ICON_TARGET_PX;
                let height = ICON_TARGET_PX;
                if let Some(mut pixmap) = Pixmap::new(width, height) {
                    let scale_x = width as f32 / intrinsic.width() as f32;
                    let scale_y = height as f32 / intrinsic.height() as f32;
                    let rtree = ResvgTree::from_usvg(&tree);
                    rtree.render(Transform::from_scale(scale_x, scale_y), &mut pixmap.as_mut());
                    let rgba = pixmap.data().to_vec();
                    return IconData { rgba, width, height };
                }
            }
        }
        // Fallback to PNG
        if let Ok(png_bytes) = fs::read(png_path) {
            if let Ok(img) = image::load_from_memory(&png_bytes) {
                let rgba = img.to_rgba8();
                let (width, height) = rgba.dimensions();
                return IconData { rgba: rgba.into_raw(), width, height };
            }
        }
        // Fallback to ICO
        if let Ok(ico_bytes) = fs::read(ico_path) {
            if let Ok(img) = image::load_from_memory(&ico_bytes) {
                let rgba = img.to_rgba8();
                let (width, height) = rgba.dimensions();
                return IconData { rgba: rgba.into_raw(), width, height };
            }
        }
        // Return empty icon on failure
        IconData { rgba: Vec::new(), width: 0, height: 0 }
    }

    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        depth_buffer: 32,
        viewport: egui::ViewportBuilder::default()
            .with_title("CADRAW")
            .with_inner_size([1440.0, 900.0])
            .with_icon(load_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "CADRAW",
        options,
        Box::new(|cc| Ok(Box::new(CadrawApp::new(cc)))),
    )
}

/// Tool sketch aktif. Titik yang sudah diklik untuk tool multi-titik
/// disimpan terpisah di `CadrawApp::pending_points` supaya beralih tool
/// tidak perlu memindah state antar varian enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolKind {
    Select,
    Line,
    Rectangle,
    Circle,
    Ellipse,
    /// 3 titik: awal, akhir, titik di busur (menentukan sisi/arah).
    Arc,
    /// Klik entitas sumber, lalu klik sisi & jarak hasil offset.
    Offset,
    /// Perlu seleksi non-kosong (dari tool Pilih) sebelum bisa memilih 2
    /// titik sumbu cermin.
    Mirror,
    /// Klik segmen Line yang mau dipotong di antara/di luar perpotongan
    /// dengan entitas Line lain.
    Trim,
    /// Revolve (Fase 8): perlu seleksi non-kosong (profil, sama prasyarat
    /// dengan Mirror), lalu 2 klik menentukan sumbu 2D (di bidang XY sama
    /// seperti sketch-nya) — revolve 360° penuh lewat
    /// `cadraw_kernel::revolve_profile`. Bukan tool sketch (tidak
    /// menghasilkan entitas) — hasilnya body baru di `model`, sama seperti
    /// Extrude.
    Revolve,
    /// Klik 2 titik (endpoint/center, lewat snap) untuk membuat keduanya
    /// berimpit — constraint Coincident. Bukan tool multi-titik biasa;
    /// pakai `pending_point_refs`, bukan `pending_points`, karena butuh
    /// tahu ENTITAS+bagian mana yang diklik, bukan cuma koordinatnya.
    CoincidentPick,
    /// Klik 1 titik (endpoint/center) untuk menahannya di posisi sekarang
    /// — constraint Fixed. Sekali klik langsung commit.
    FixedPick,
    /// Perlu 1 entitas Line terpilih dulu (sumbu, dari tool Pilih), lalu
    /// klik 2 titik yang mau dibuat saling cermin — constraint Symmetric.
    SymmetricPick,
    /// Klik 2 titik (snap) → jarak lurus di antaranya, ditambahkan ke
    /// `CadrawApp::measurements` (Fase 7). Non-destruktif: TIDAK membuat
    /// entitas atau menyentuh undo stack sama sekali, beda dari tool
    /// menggambar — lihat `cadraw_sketch::measure`.
    Measure,
    /// Klik 3 titik (awal, titik-sudut/vertex, akhir) → sudut interior di
    /// vertex, sama pola non-destruktif dengan `Measure`.
    MeasureAngle,
}

impl ToolKind {
    fn to_toolbar_tool(self) -> ToolbarTool {
        match self {
            ToolKind::Select => ToolbarTool::Select,
            ToolKind::Line => ToolbarTool::Line,
            ToolKind::Arc => ToolbarTool::Arc,
            ToolKind::Rectangle => ToolbarTool::Rectangle,
            ToolKind::Circle => ToolbarTool::Circle,
            ToolKind::Ellipse => ToolbarTool::Ellipse,
            ToolKind::Offset => ToolbarTool::Offset,
            ToolKind::Mirror => ToolbarTool::Mirror,
            ToolKind::Trim => ToolbarTool::Trim,
            ToolKind::Revolve => ToolbarTool::Revolve,
            ToolKind::CoincidentPick => ToolbarTool::PointCoincident,
            ToolKind::FixedPick => ToolbarTool::PointFixed,
            ToolKind::SymmetricPick => ToolbarTool::PointSymmetric,
            ToolKind::Measure => ToolbarTool::Measure,
            ToolKind::MeasureAngle => ToolbarTool::MeasureAngle,
        }
    }

    fn from_toolbar_tool(tool: ToolbarTool) -> Self {
        match tool {
            ToolbarTool::Select => ToolKind::Select,
            ToolbarTool::Line => ToolKind::Line,
            ToolbarTool::Arc => ToolKind::Arc,
            ToolbarTool::Rectangle => ToolKind::Rectangle,
            ToolbarTool::Circle => ToolKind::Circle,
            ToolbarTool::Ellipse => ToolKind::Ellipse,
            ToolbarTool::Offset => ToolKind::Offset,
            ToolbarTool::Mirror => ToolKind::Mirror,
            ToolbarTool::Trim => ToolKind::Trim,
            ToolbarTool::Revolve => ToolKind::Revolve,
            ToolbarTool::PointCoincident => ToolKind::CoincidentPick,
            ToolbarTool::PointFixed => ToolKind::FixedPick,
            ToolbarTool::PointSymmetric => ToolKind::SymmetricPick,
            ToolbarTool::Measure => ToolKind::Measure,
            ToolbarTool::MeasureAngle => ToolKind::MeasureAngle,
        }
    }
}

/// Tool yang ditawarkan radial menu (Fase 4) saat long-press di viewport
/// dengan tool Pilih aktif — subset tool sketch yang paling sering dipakai
/// gaya Shapr3D; tool pemilihan titik (Coincident/Fixed/Symmetric) dan
/// Pilih sendiri sengaja tidak dimasukkan (Pilih sudah aktif, memutar ke
/// dirinya sendiri tidak berguna; tool titik tetap lewat toolbar/palette).
const RADIAL_TOOLS: [(ToolKind, &str); 8] = [
    (ToolKind::Line, "Garis"),
    (ToolKind::Rectangle, "Persegi"),
    (ToolKind::Circle, "Lingkaran"),
    (ToolKind::Ellipse, "Ellips"),
    (ToolKind::Arc, "Arc"),
    (ToolKind::Offset, "Offset"),
    (ToolKind::Mirror, "Mirror"),
    (ToolKind::Trim, "Trim"),
];

/// Referensi pintasan keyboard, ditampilkan apa adanya di menu
/// "⚙ Pengaturan" — daftar bantuan statis, BUKAN pintasan yang bisa
/// di-remap (remapping ada di luar lingkup putaran ini).
#[allow(dead_code)]
const KEYBOARD_SHORTCUTS: [(&str, &str); 17] = [
    ("L", "Tool Garis"),
    ("R", "Tool Persegi"),
    ("C", "Tool Lingkaran"),
    ("E", "Tool Ellips"),
    ("A", "Tool Arc"),
    ("O", "Tool Offset"),
    ("M", "Tool Mirror"),
    ("T", "Tool Trim"),
    ("V", "Tool Revolve"),
    ("Esc", "Batal titik pending, atau kembali ke tool Pilih"),
    ("Delete / Backspace", "Hapus seleksi"),
    ("Ctrl/Cmd+Z", "Undo sketch"),
    ("Ctrl/Cmd+Shift+Z atau Ctrl+Y", "Redo sketch"),
    ("Ctrl/Cmd+O", "Buka dokumen .cadraw"),
    ("Ctrl/Cmd+S", "Simpan dokumen"),
    ("Ctrl/Cmd+Shift+S", "Simpan Sebagai…"),
    ("Ctrl/Cmd+K", "Buka/tutup command palette"),
];

/// Operasi file (Fase 5) — satu variant `PaletteAction::File(FileOp)`
/// alih-alih 10 variant `PaletteAction` terpisah, supaya enum itu tidak
/// membengkak untuk sesuatu yang semuanya cuma memanggil satu method
/// `CadrawApp` masing-masing (lihat `run_palette_action`).
#[derive(Debug, Clone, Copy)]
enum FileOp {
    New,
    Open,
    Save,
    SaveAs,
    ImportStep,
    ImportDxf,
    ExportStep,
    ExportStl,
    ExportObj,
    ExportDxf,
}

/// Aksi yang bisa dieksekusi lewat command palette (Fase 4) — dipetakan
/// dari index yang dikembalikan `CommandPalette::show`, lihat
/// `CadrawApp::palette_actions`.
#[derive(Debug, Clone, Copy)]
enum PaletteAction {
    SetTool(ToolKind),
    Undo,
    Redo,
    ModelUndo,
    ModelRedo,
    DeleteSelection,
    ToggleTheme,
    SetSketchPlane(PlaneKind),
    EnterSketching,
    ExitSketching,
    File(FileOp),
    /// Kosongkan `CadrawApp::measurements` (Fase 7) — cuma muncul di
    /// palette saat ada isinya, sama pola dengan `DeleteSelection`.
    ClearMeasurements,
}

/// Berapa titik yang dibutuhkan tool sebelum di-commit lewat
/// `CadrawApp::finish_multipoint`. Offset/Trim/CoincidentPick/FixedPick/
/// SymmetricPick ditangani jalur terpisah (bergantung entitas/PointRef
/// yang diklik, bukan sekadar koordinat titik).
fn required_points(tool: ToolKind) -> usize {
    match tool {
        ToolKind::Line | ToolKind::Rectangle | ToolKind::Circle | ToolKind::Ellipse
        | ToolKind::Mirror | ToolKind::Revolve | ToolKind::Measure => 2,
        ToolKind::Arc | ToolKind::MeasureAngle => 3,
        ToolKind::Select
        | ToolKind::Offset
        | ToolKind::Trim
        | ToolKind::CoincidentPick
        | ToolKind::FixedPick
        | ToolKind::SymmetricPick => 0,
    }
}

/// Satu hasil pengukuran non-destruktif (Fase 7, tool Ukur/Ukur Sudut) —
/// disimpan mentah-mentah (titik, bukan cuma angka) supaya overlay bisa
/// menggambar ulang garis penghubungnya lewat
/// `cadraw_render::sketch::measurement_lines` tanpa perhitungan ulang.
enum Measurement {
    Distance { a: DVec2, b: DVec2 },
    Angle { a: DVec2, vertex: DVec2, b: DVec2 },
}

impl Measurement {
    /// Label siap tampil di panel Pengukuran — `None` untuk sudut degenerate
    /// (dua titik berimpit dengan vertex, lihat `cadraw_sketch::measure`).
    fn label(&self) -> String {
        match self {
            Measurement::Distance { a, b } => {
                format!("Jarak: {:.3} mm", cadraw_sketch::measure::distance(*a, *b))
            }
            Measurement::Angle { a, vertex, b } => match cadraw_sketch::measure::angle_degrees(*a, *vertex, *b) {
                Some(angle) => format!("Sudut: {angle:.2}°"),
                None => "Sudut: tidak terdefinisi (titik berimpit)".to_string(),
            },
        }
    }

    /// Titik-titik untuk digambar ulang lewat `measurement_lines` — urutan
    /// SENGAJA a→vertex→b untuk Angle (vertex di tengah, sama urutan yang
    /// dipakai `finish_multipoint` saat commit).
    fn points(&self) -> Vec<DVec2> {
        match self {
            Measurement::Distance { a, b } => vec![*a, *b],
            Measurement::Angle { a, vertex, b } => vec![*a, *vertex, *b],
        }
    }

    /// Nilai pendek untuk badge dimensi in-situ langsung di atas garisnya
    /// (`dynamic_input_ui`) — beda dari `label()` yang punya prefix
    /// "Jarak:"/"Sudut:" untuk konteks daftar di panel Properties kanan.
    /// `None` untuk sudut degenerate (dua titik berimpit dengan vertex).
    fn inline_value(&self, unit: LengthUnit) -> Option<String> {
        match self {
            Measurement::Distance { a, b } => {
                Some(unit.format_precise(cadraw_sketch::measure::distance(*a, *b)))
            }
            Measurement::Angle { a, vertex, b } => {
                cadraw_sketch::measure::angle_degrees(*a, *vertex, *b).map(|deg| format!("{deg:.1}°"))
            }
        }
    }
}

/// Sumbu bidang potong Section View (Fase 7) — normal sejajar sumbu dunia,
/// sisi mana yang dibuang ditentukan `CadrawApp::section_invert`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum SectionAxis {
    X,
    Y,
    Z,
}

impl SectionAxis {
    fn normal(self) -> Vec3 {
        match self {
            SectionAxis::X => Vec3::X,
            SectionAxis::Y => Vec3::Y,
            SectionAxis::Z => Vec3::Z,
        }
    }

    #[allow(dead_code)]
    fn label(self) -> &'static str {
        match self {
            SectionAxis::X => "X",
            SectionAxis::Y => "Y",
            SectionAxis::Z => "Z",
        }
    }
}

/// Mode picking 3D aktif di viewport (Fase 8) — ORTOGONAL terhadap
/// `ToolKind` (bukan varian tool baru): dipicu tombol toggle di panel
/// Model 3D (section Fillet/Chamfer/Shell), butuh PERSIS 1 body terpilih
/// (precondition sama dengan operasi "semua tepi/1 face" yang sudah ada).
/// Selama aktif, klik viewport di-intercept `handle_sketch_input` SEBELUM
/// hit-test sketch biasa — lihat `CadrawApp::handle_3d_picking`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum PickMode {
    #[default]
    None,
    Edge,
    Face,
}

/// "Sidik jari" ringkas dari seluruh state yang memengaruhi tinggi konten Panel
/// Properti. egui meng-cache tinggi `Area` dari frame sebelumnya sebagai patokan
/// "ruang tersedia" frame berikutnya — jika kontennya membesar (mis. ganti seleksi
/// dari kosong ke body dengan banyak fitur), tinggi lama yang sudah ter-clip oleh
/// ScrollArea akan terjebak permanen di ukuran kecil itu. Membandingkan sig ini
/// tiap frame membiarkan kita memaksa `Area::sizing_pass(true)` (ukur ulang dari
/// nol) tepat pada frame kontennya berubah, lalu diam lagi di frame-frame stabil.
type InspectorContentSig = (
    std::mem::Discriminant<SelectedEntityData>,
    bool,
    usize,
    usize,
    usize,
    bool,
    bool,
    bool,
    PickMode,
    usize,
    bool,
);

/// Satu tepi 3D terpilih lewat picking (Fase 8): `ray` dunia yang dipakai
/// klik (di-cast ULANG terhadap shape hasil `deep_clone` saat apply — lihat
/// desain `cadraw_kernel::PickRay`), plus `polyline` hasil pick SEKARANG
/// (di-cache di sini supaya highlight overlay tidak query kernel ulang tiap
/// frame render).
struct PickedEdge {
    ray: PickRay,
    polyline: Vec<(f64, f64, f64)>,
}

/// Jenis target satu fitur rounding parametrik: titik sudut (blend semua
/// rusuk yang bertemu di vertex, via `fillet_vertex`) atau satu rusuk
/// (via `fillet_edges` 1 ray).
#[derive(Clone, Copy, PartialEq, Eq)]
enum RoundKind {
    Vertex,
    Edge,
}

/// Satu fitur rounding parametrik pada sebuah body. `ray` di-resolve ULANG
/// terhadap shape dasar tiap rebuild (pola ray-based yang sama dengan
/// `active_face`), `anchor` = titik klik semula (jangkar gizmo + tes
/// kedekatan saat user mengklik sudut yang SUDAH bulat untuk mengeditnya).
#[derive(Clone)]
struct RoundFeature {
    kind: RoundKind,
    ray: PickRay,
    anchor: (f64, f64, f64),
    radius: f64,
    /// Polyline rusuk asli (kosong utk `RoundKind::Vertex`) — dipakai tes
    /// kedekatan klik SEPANJANG rusuk yang sudah bulat, bukan cuma di
    /// sekitar titik klik semula (`anchor`).
    polyline: Vec<(f64, f64, f64)>,
}

/// Riwayat rounding satu body: shape dasar SEBELUM rounding pertama +
/// daftar fitur yang di-apply berurutan di atasnya. Mengubah radius (atau
/// menghapus fitur, radius→0 = kembali menyiku) = rebuild dari `base`,
/// BUKAN memfillet ulang shape yang sudah difillet — inilah yang membuat
/// gizmo rounding bisa dua arah (pull membesarkan radius, push mengecilkan
/// sampai siku), bukan hanya menumpuk fillet baru.
struct RoundHistory {
    base: KernelShape,
    features: Vec<RoundFeature>,
}

struct CadrawApp {
    camera: OrbitCamera,

    /// Sketsa 2D terisolasi untuk tiap datum plane: index 0 = Top (XY), 1 = Front (XZ), 2 = Right (YZ).
    sketches: [Sketch; 3],
    /// Undo stack terisolasi untuk tiap datum plane: index 0 = Top (XY), 1 = Front (XZ), 2 = Right (YZ).
    undos: [cadraw_sketch::UndoStack; 3],

    tool: ToolKind,
    pending_points: Vec<DVec2>,
    /// Titik (via PointRef) yang sudah diklik untuk CoincidentPick/
    /// SymmetricPick — terpisah dari `pending_points` karena tool ini
    /// butuh identitas entitas+bagian, bukan cuma koordinat.
    pending_point_refs: Vec<constraint::PointRef>,
    /// Entitas sumber untuk tool Offset, di-set pada klik pertama.
    offset_source: Option<EntityId>,

    hovered: Option<EntityId>,
    selected: HashSet<EntityId>,
    last_snap: Option<SnapHit>,

    dynamic_input: String,
    dynamic_focus_pending: bool,

    /// Pesan gagal terakhir (mis. constraint tidak konvergen), ditampilkan
    /// di panel sampai constraint berikutnya berhasil atau seleksi berubah.
    constraint_status: Option<String>,

    /// Dokumen 3D (Fase 3): metadata body + geometri kernel, lihat modul
    /// `model`. Undo/redo-nya terpisah dari `undo` (sketch) — lihat doc
    /// comment atas file.
    model: ModelDoc,
    model_undo: cadraw_core::UndoStack<ModelDoc>,
    selected_bodies: HashSet<BodyId>,
    /// Pesan gagal terakhir dari operasi Model (dry-run gagal), sama pola
    /// dengan `constraint_status`.
    model_status: Option<String>,
    extrude_distance_input: String,
    fillet_radius_input: String,
    chamfer_distance_input: String,
    shell_thickness_input: String,
    shell_direction: cadraw_kernel::Direction,

    /// Fase 8: profil BAWAH loft, di-stage lewat tombol "Set Profil Bawah
    /// dari Seleksi" di panel Model 3D — profil ATAS dibaca dari seleksi
    /// sketch SAAT TOMBOL LOFT DIKLIK (bukan di-stage juga), sama pola
    /// "hitung dulu, dry-run" seperti operasi model lain.
    pending_loft_bottom: Option<cadraw_kernel::Profile>,
    loft_height_input: String,

    /// Picking edge/face 3D (Fase 8) — lihat `PickMode`. Body yang ditarget
    /// selalu `selected_bodies` (harus persis 1) saat mode diaktifkan.
    picking_mode: PickMode,
    selected_edges: Vec<PickedEdge>,
    selected_faces: Vec<PickRay>,
    active_face: Option<(BodyId, PickRay, FaceHit)>,
    face_extrude_distance_input: String,

    /// Vertex fillet gizmo (Fase 2 — Rounded Sudut 3D): sudut (vertex) 3D
    /// yang sedang aktif. Simpan `ray` yang dipakai saat pick (bukan cuma
    /// titik hasil) supaya resolusi ULANG vertex saat fillet sungguhan
    /// dieksekusi tetap konsisten dgn body hasil `deep_clone` — pola sama
    /// persis dgn `active_face`/`PickRay`. Di klik viewport mode 3D, pick
    /// vertex dicoba DULUAN dan menang atas pick face (lihat
    /// `handle_sketch_input`) karena target vertex jauh lebih kecil secara
    /// visual dan gampang "ketutup" face di baliknya kalau tidak
    /// diutamakan.
    active_vertex: Option<(BodyId, PickRay, (f64, f64, f64))>,

    /// Fase 5 (File I/O). Path file `.cadraw` aktif — `None` sampai dokumen
    /// pernah disimpan/dibuka sekali; menentukan apakah "Simpan" (⌘S)
    /// langsung menulis ke sini atau jatuh ke "Simpan Sebagai" (dialog).
    current_file_path: Option<PathBuf>,
    /// Pesan hasil operasi file terakhir (sukses ATAU gagal — beda dari
    /// `model_status`/`constraint_status` yang cuma terisi saat gagal,
    /// karena "Tersimpan ke X" juga informasi berguna bagi user).
    /// Ditampilkan di status bar bawah.
    file_status: Option<String>,

    /// Fase 4 (UX shell). `theme` diterapkan lewat `cadraw_ui::apply_theme`
    /// tiap kali berubah, bukan cuma dibaca — style egui bukan reaktif
    /// otomatis terhadap field ini.
    theme: ThemeMode,
    palette: CommandPalette,
    radial_menu: RadialMenu,
    /// Titik & waktu mulai (detik, `egui::Context::input().time`) tekan
    /// primer di viewport yang masih berlangsung — dipakai mendeteksi
    /// long-press (lihat `handle_radial_menu`). `None` berarti tak ada
    /// tekan primer aktif, atau sudah dibatalkan karena bergerak (jadi
    /// drag/orbit biasa, bukan long-press).
    radial_press: Option<(egui::Pos2, f64)>,
    /// Set `true` saat long-press baru saja membuka radial menu, supaya
    /// `response.clicked()` dari pelepasan pointer yang sama tidak ikut
    /// diproses sebagai klik seleksi/tool biasa. Dikonsumsi (di-reset ke
    /// `false`) sekali oleh `handle_sketch_input` tiap frame.
    radial_suppress_click: bool,

    /// Hasil tool Ukur/Ukur Sudut (Fase 7), terkumpul sampai "Hapus Semua"
    /// ditekan atau dokumen baru dibuat — TIDAK ikut undo stack manapun
    /// (non-destruktif, lihat `Measurement`).
    measurements: Vec<Measurement>,

    /// Section view (Fase 7): bidang potong viewport 3D, murni efek render
    /// (`SceneRenderer::set_clip_plane`) — tidak pernah memanggil kernel
    /// OCCT, jadi aman digeser real-time. Lihat header komentar modul.
    section_enabled: bool,
    section_axis: SectionAxis,
    /// Jarak bidang potong dari origin di sepanjang `section_axis` (mm).
    section_offset: f32,
    /// `false` (default): buang sisi POSITIF sumbu (mis. X → sisakan
    /// x ≤ offset). `true`: balik, buang sisi negatif.
    section_invert: bool,

    /// Checkbox "Tampilkan Semua Ukuran" di kartu Pengukuran (ruler
    /// properties, panel Properties kanan): saat true, `dynamic_input_ui`
    /// melabeli nominal panjang/radius SEMUA entitas sketsa 2D di bidang
    /// aktif DAN semua rusuk 3D dari semua body visible — lihat
    /// `render_all_element_dimensions`.
    show_all_dimensions: bool,

    /// Worker latar belakang Import STEP (Fase 7) — lihat modul
    /// `import_worker`. Di-poll tiap frame di `update()`.
    import_worker: ImportWorker,
    /// Jumlah job import yang sudah di-`submit` tapi belum kembali lewat
    /// `poll`. `> 0` memaksa `update()` minta repaint terus-menerus (egui
    /// biasanya cuma redraw saat ada input) supaya hasil worker langsung
    /// muncul begitu selesai, bukan menunggu user menggerakkan mouse.
    pending_imports: u32,

    // Komponen UI Floating Shapr3D
    left_toolbar: LeftToolbar,
    items_drawer: ItemsDrawer,
    viewcube: ViewCube,
    feature_inspector_open: bool,
    auto_hide_properties: bool,
    /// Popup drawer "Items" (daftar sketch & body) — tombolnya sekarang ada
    /// di header (`TopBar`), tapi status buka/tutup tetap disimpan di sini
    /// karena drawer-nya sendiri masih dirender lewat `ItemsDrawer` terpisah.
    items_drawer_open: bool,
    /// Popup dropdown pemilih Sketch Plane di header — dibaca/ditulis lewat
    /// `TopBarState::plane_menu_open` tiap frame (lihat pola `inspector_state`).
    plane_menu_open: bool,
    /// Sig konten Left Toolbar dari frame terakhir (cuma `is_sketching`,
    /// karena itu satu-satunya hal yang mengubah tinggi kontennya sekarang)
    /// dipakai buat memutuskan apakah `Area`-nya perlu `sizing_pass` ulang
    /// frame ini supaya pemusatan vertikalnya akurat begitu tinggi berubah
    /// (meniru pola `InspectorContentSig`).
    left_toolbar_content_sig: Option<bool>,
    /// Sig konten Panel Properti dari frame terakhir dipakai untuk memutuskan
    /// apakah `Area`-nya perlu `sizing_pass` ulang frame ini (lihat `InspectorContentSig`).
    inspector_content_sig: Option<InspectorContentSig>,
    prop_input_p1_x: String,
    prop_input_p1_y: String,
    prop_input_p2_x: String,
    prop_input_p2_y: String,
    prop_input_val_1: String,
    prop_input_val_2: String,
    last_inspected_entity_id: Option<u64>,

    // State Sketch Mode & Satuan
    is_sketching: bool,
    active_plane: SketchPlane,
    unit: LengthUnit,

    // State Direct Extrude Gizmo & Smart Boolean Cut
    extruding_from_gizmo: bool,
    gizmo_distance: f64,
    gizmo_dimension_editing: bool,
    gizmo_edit_input: String,
    gizmo_is_cutting: bool,
    gizmo_target_body: Option<BodyId>,

    // State 3D Face Extrude Gizmo
    extruding_face_from_gizmo: bool,
    face_gizmo_distance: f64,
    face_gizmo_dimension_editing: bool,
    face_gizmo_edit_input: String,

    // State Vertex Fillet Gizmo (Fase 2: Rounded Sudut 3D)
    filleting_vertex_from_gizmo: bool,
    vertex_gizmo_radius: f64,
    vertex_gizmo_dimension_editing: bool,
    vertex_gizmo_edit_input: String,

    /// Vertex TERDEKAT ke kursor SEKARANG (bukan hasil klik) dalam
    /// toleransi pick — dihitung ULANG tiap frame di `handle_sketch_input`
    /// saat mode 3D & tool Select, dipakai `build_overlay_lines` buat
    /// highlight marker vertex yang bakal kena kalau diklik. Terpisah dari
    /// `active_vertex` (yang cuma terisi SETELAH klik) supaya user dapat
    /// feedback SEBELUM klik — keluhan awal fitur ini persis "tanpa
    /// feedback hover, praktis mustahil dikenai".
    hovered_vertex_marker: Option<(BodyId, (f64, f64, f64))>,

    /// Rusuk (edge) 3D yang sedang aktif lewat gizmo rounding — cermin
    /// `active_vertex` tapi menyasar RUSUK, bukan sudut. Dipakai untuk
    /// kasus "klik rusuk pojok kubus" (mis. sisi tegak sudut box) yang
    /// secara visual sering kena duluan sebelum titik vertex-nya sendiri
    /// (lihat komentar di `pick_body_edge_at_cursor`). Titik yang disimpan
    /// adalah titik klik PADA rusuk (dipakai sbg jangkar gizmo), `ray`
    /// dipakai resolusi ULANG rusuk saat commit (pola sama dgn
    /// `active_face`/`active_vertex`).
    active_edge: Option<(BodyId, PickRay, (f64, f64, f64))>,

    // State Edge Fillet Gizmo ("klik rusuk pojok -> rusuk membulat")
    filleting_edge_from_gizmo: bool,
    edge_gizmo_radius: f64,
    edge_gizmo_dimension_editing: bool,
    edge_gizmo_edit_input: String,

    /// Riwayat rounding parametrik per body (lihat `RoundHistory`).
    /// HANYA hidup selama sesi — tidak diserialisasi ke dokumen. Entri
    /// body di-invalidate (fitur "dibake") begitu geometri body diubah
    /// operasi lain (extrude face, boolean, fillet/chamfer/shell panel),
    /// karena `base`-nya tidak lagi merepresentasikan shape saat ini.
    round_history: std::collections::HashMap<BodyId, RoundHistory>,
    /// `Some((body, index fitur))` saat gizmo rounding sedang MENGEDIT
    /// fitur yang sudah ada (klik sudut yang sudah bulat), bukan membuat
    /// fitur baru — commit mengubah/menghapus fitur itu lalu rebuild.
    editing_round: Option<(BodyId, usize)>,
}

impl CadrawApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let render_state = cc
            .wgpu_render_state
            .as_ref()
            .expect("CADRAW membutuhkan backend wgpu");
        let scene = SceneRenderer::new(
            &render_state.device,
            render_state.target_format,
            Some(cadraw_render::wgpu::TextureFormat::Depth32Float),
        );
        render_state
            .renderer
            .write()
            .callback_resources
            .insert(scene);

        let theme = ThemeMode::default();
        cadraw_ui::apply_theme(&cc.egui_ctx, theme);

        // egui default: SHIFT dianggap "horizontal_scroll_modifier", jadi
        // event MouseWheel dengan Shift ditekan otomatis diratakan jadi
        // vec2(dx+dy, 0.0) SEBELUM sampai ke handle_navigation() (lihat
        // egui input_state::process_events). Ini bikin gesture Shift + 2
        // jari trackpad (pan bebas segala arah) cuma bisa geser kiri-kanan,
        // padahal camera.pan() sendiri sudah benar menangani dx & dy.
        // Matikan remap bawaan itu supaya delta 2D utuh sampai ke pan().
        cc.egui_ctx
            .options_mut(|o| o.input_options.horizontal_scroll_modifier = egui::Modifiers::NONE);

        Self {
            camera: OrbitCamera::default(),
            sketches: [Sketch::default(), Sketch::default(), Sketch::default()],
            undos: [
                cadraw_sketch::UndoStack::default(),
                cadraw_sketch::UndoStack::default(),
                cadraw_sketch::UndoStack::default(),
            ],
            tool: ToolKind::Select,
            pending_points: Vec::new(),
            pending_point_refs: Vec::new(),
            offset_source: None,
            hovered: None,
            selected: HashSet::new(),
            last_snap: None,
            dynamic_input: String::new(),
            dynamic_focus_pending: false,
            constraint_status: None,

            model: ModelDoc::default(),
            model_undo: cadraw_core::UndoStack::default(),
            selected_bodies: HashSet::new(),
            model_status: None,
            extrude_distance_input: "10".to_string(),
            fillet_radius_input: "2".to_string(),
            chamfer_distance_input: "2".to_string(),
            shell_thickness_input: "2".to_string(),
            shell_direction: cadraw_kernel::Direction::PosZ,

            pending_loft_bottom: None,
            loft_height_input: "10".to_string(),
            picking_mode: PickMode::default(),
            selected_edges: Vec::new(),
            selected_faces: Vec::new(),
            active_face: None,
            face_extrude_distance_input: "15".to_string(),
            active_vertex: None,

            current_file_path: None,
            file_status: None,

            theme,
            palette: CommandPalette::default(),
            radial_menu: RadialMenu::default(),
            radial_press: None,
            radial_suppress_click: false,

            measurements: Vec::new(),

            section_enabled: false,
            section_axis: SectionAxis::Z,
            section_offset: 0.0,
            section_invert: false,
            show_all_dimensions: false,

            import_worker: ImportWorker::spawn(),
            pending_imports: 0,

            left_toolbar: LeftToolbar::default(),
            items_drawer: ItemsDrawer::default(),
            viewcube: ViewCube::default(),
            feature_inspector_open: true,
            auto_hide_properties: true,
            items_drawer_open: false,
            plane_menu_open: false,
            left_toolbar_content_sig: None,
            inspector_content_sig: None,
            prop_input_p1_x: String::new(),
            prop_input_p1_y: String::new(),
            prop_input_p2_x: String::new(),
            prop_input_p2_y: String::new(),
            prop_input_val_1: String::new(),
            prop_input_val_2: String::new(),
            last_inspected_entity_id: None,

            is_sketching: true,
            active_plane: SketchPlane::top(),
            unit: LengthUnit::Millimeters,

            extruding_from_gizmo: false,
            gizmo_distance: 20.0,
            gizmo_dimension_editing: false,
            gizmo_edit_input: "20".to_string(),
            gizmo_is_cutting: false,
            gizmo_target_body: None,

            extruding_face_from_gizmo: false,
            face_gizmo_distance: 15.0,
            face_gizmo_dimension_editing: false,
            face_gizmo_edit_input: "15".to_string(),

            filleting_vertex_from_gizmo: false,
            vertex_gizmo_radius: 3.0,
            vertex_gizmo_dimension_editing: false,
            vertex_gizmo_edit_input: "3".to_string(),

            hovered_vertex_marker: None,
            active_edge: None,
            filleting_edge_from_gizmo: false,
            edge_gizmo_radius: 3.0,
            edge_gizmo_dimension_editing: false,
            edge_gizmo_edit_input: "3".to_string(),

            round_history: std::collections::HashMap::new(),
            editing_round: None,
        }
    }

    #[inline]
    pub fn plane_for_index(idx: usize) -> SketchPlane {
        match idx {
            0 => SketchPlane::top(),
            1 => SketchPlane::front(),
            2 => SketchPlane::right(),
            _ => SketchPlane::top(),
        }
    }

    #[inline]
    fn active_plane_index(&self) -> usize {
        match self.active_plane.kind {
            PlaneKind::Top => 0,
            PlaneKind::Front => 1,
            PlaneKind::Right => 2,
        }
    }

    #[inline]
    fn sketch(&self) -> &Sketch {
        &self.sketches[self.active_plane_index()]
    }

    #[inline]
    #[allow(dead_code)]
    fn sketch_mut(&mut self) -> &mut Sketch {
        let idx = self.active_plane_index();
        &mut self.sketches[idx]
    }

    #[inline]
    fn execute_sketch_command(&mut self, cmd: Box<dyn cadraw_core::Command<Sketch>>) {
        let idx = self.active_plane_index();
        self.undos[idx].execute(cmd, &mut self.sketches[idx]);
    }

    #[inline]
    fn undo_active_sketch(&mut self) {
        let idx = self.active_plane_index();
        self.undos[idx].undo(&mut self.sketches[idx]);
    }

    #[inline]
    fn redo_active_sketch(&mut self) {
        let idx = self.active_plane_index();
        self.undos[idx].redo(&mut self.sketches[idx]);
    }

    /// Ubah bidang kerja sketsa aktif (`Top`, `Front`, atau `Right`) dan selaraskan kamera.
    fn set_sketch_plane(&mut self, kind: PlaneKind) {
        if self.active_plane.kind != kind {
            self.selected.clear();
            self.hovered = None;
            self.pending_points.clear();
            self.pending_point_refs.clear();
            self.offset_source = None;
            self.last_snap = None;
            self.active_plane = SketchPlane::from_kind(kind);
        }
        self.is_sketching = true;
        self.left_toolbar.is_sketching = true;
        self.camera.orient_to_plane(&self.active_plane);
    }

    /// Extrude profil pada bidang sketsa aktif sepanjang `distance`.
    fn extrude_profile_active_plane(
        &self,
        profile: &cadraw_kernel::Profile,
        distance: f64,
    ) -> anyhow::Result<cadraw_kernel::KernelShape> {
        let orig = self.active_plane.to_world_f64((0.0, 0.0), 0.0);
        let u_ax = [
            self.active_plane.u_axis.x as f64,
            self.active_plane.u_axis.y as f64,
            self.active_plane.u_axis.z as f64,
        ];
        let v_ax = [
            self.active_plane.v_axis.x as f64,
            self.active_plane.v_axis.y as f64,
            self.active_plane.v_axis.z as f64,
        ];
        let n_ax = [
            self.active_plane.normal.x as f64,
            self.active_plane.normal.y as f64,
            self.active_plane.normal.z as f64,
        ];
        cadraw_kernel::extrude_profile_on_plane(profile, orig, u_ax, v_ax, n_ax, distance)
    }

    fn set_tool(&mut self, tool: ToolKind) {
        self.tool = tool;
        self.pending_points.clear();
        self.pending_point_refs.clear();
        self.offset_source = None;
        self.last_snap = None;
        self.dynamic_input.clear();
        self.dynamic_focus_pending = false;
    }

    fn snapped_or(&self, raw: DVec2) -> DVec2 {
        self.last_snap.map(|s| s.point).unwrap_or(raw)
    }

    /// Entitas Line pertama dalam seleksi saat ini — dipakai sumbu cermin
    /// tool SymmetricPick (prasyarat: pilih 1 Line lewat tool Pilih dulu,
    /// pola yang sama dengan Mirror).
    fn symmetric_axis(&self) -> Option<EntityId> {
        self.selected
            .iter()
            .copied()
            .find(|id| matches!(self.sketch().entities.get(*id), Some(Entity::Line { .. })))
    }

    /// Terima satu titik klik untuk tool multi-titik aktif; commit otomatis
    /// begitu jumlah titik yang dibutuhkan tool tercapai.
    fn on_click_point(&mut self, p: DVec2) {
        self.pending_points.push(p);
        if self.pending_points.len() == 1 {
            self.dynamic_focus_pending = true;
        }
        if self.pending_points.len() >= required_points(self.tool) {
            self.finish_multipoint();
        }
    }

    /// Bangun entitas/command dari `pending_points` yang sudah lengkap dan
    /// eksekusi lewat undo stack.
    fn finish_multipoint(&mut self) {
        let pts = std::mem::take(&mut self.pending_points);
        let cmd: Option<Box<dyn cadraw_core::Command<Sketch>>> = match self.tool {
            ToolKind::Line => Some(Box::new(InsertEntities::new(
                "Garis",
                vec![Entity::Line {
                    start: pts[0],
                    end: pts[1],
                }],
            ))),
            ToolKind::Rectangle => {
                let min = pts[0].min(pts[1]);
                let max = pts[0].max(pts[1]);
                let corners = [
                    DVec2::new(min.x, min.y),
                    DVec2::new(max.x, min.y),
                    DVec2::new(max.x, max.y),
                    DVec2::new(min.x, max.y),
                ];
                let lines = (0..4)
                    .map(|i| Entity::Line {
                        start: corners[i],
                        end: corners[(i + 1) % 4],
                    })
                    .collect();
                Some(Box::new(InsertEntities::new("Persegi", lines)))
            }
            ToolKind::Circle => {
                let radius = (pts[1] - pts[0]).length();
                (radius > 1e-6).then(|| {
                    Box::new(InsertEntities::new(
                        "Lingkaran",
                        vec![Entity::Circle {
                            center: pts[0],
                            radius,
                        }],
                    )) as Box<dyn cadraw_core::Command<Sketch>>
                })
            }
            ToolKind::Ellipse => {
                let radius_x = (pts[1].x - pts[0].x).abs();
                let radius_y = (pts[1].y - pts[0].y).abs();
                (radius_x > 1e-6 && radius_y > 1e-6).then(|| {
                    Box::new(InsertEntities::new(
                        "Ellips",
                        vec![Entity::Ellipse {
                            center: pts[0],
                            radius_x,
                            radius_y,
                        }],
                    )) as Box<dyn cadraw_core::Command<Sketch>>
                })
            }
            ToolKind::Arc => arc_from_three_points(pts[0], pts[1], pts[2])
                .map(|e| Box::new(InsertEntities::new("Arc", vec![e])) as _),
            ToolKind::Mirror => {
                let (axis_a, axis_b) = (pts[0], pts[1]);
                let mirrored: Vec<Entity> = self
                    .selected
                    .iter()
                    .filter_map(|id| self.sketch().entities.get(*id))
                    .filter_map(|e| mirror_entity(e, axis_a, axis_b))
                    .collect();
                (!mirrored.is_empty())
                    .then(|| Box::new(InsertEntities::new("Cerminkan", mirrored)) as _)
            }
            ToolKind::Revolve => {
                // Bukan entitas sketch — hasilnya body baru di `model`,
                // sama pola dengan Extrude (dry-run kernel dulu, baru
                // masuk `model_undo` kalau sukses). Selalu `None` di sini
                // (tidak ada Command sketch yang dikembalikan), sama
                // seperti Measure/MeasureAngle di bawah.
                let (axis_origin, axis_end) = (pts[0], pts[1]);
                let axis_dir = axis_end - axis_origin;
                if axis_dir.length() < 1e-6 {
                    self.model_status = Some("Revolve gagal: dua titik axis sama/terlalu dekat".to_string());
                } else {
                    match model::build_profile_from_selection(self.sketch(), &self.selected) {
                        Ok(profile) => match cadraw_kernel::revolve_profile(
                            &profile,
                            (axis_origin.x, axis_origin.y),
                            (axis_dir.x, axis_dir.y),
                            None,
                        ) {
                            Ok(shape) => {
                                let geo = BodyGeometry::from_shape(shape);
                                self.model_undo.execute(
                                    Box::new(AddSolidCommand::new("Revolve", geo)),
                                    &mut self.model,
                                );
                                self.model_status = None;
                            }
                            Err(e) => self.model_status = Some(format!("Revolve gagal: {e}")),
                        },
                        Err(msg) => self.model_status = Some(msg),
                    }
                }
                None
            }
            ToolKind::Measure => {
                self.measurements.push(Measurement::Distance { a: pts[0], b: pts[1] });
                None
            }
            ToolKind::MeasureAngle => {
                self.measurements.push(Measurement::Angle {
                    a: pts[0],
                    vertex: pts[1],
                    b: pts[2],
                });
                None
            }
            ToolKind::Select
            | ToolKind::Offset
            | ToolKind::Trim
            | ToolKind::CoincidentPick
            | ToolKind::FixedPick
            | ToolKind::SymmetricPick => None,
        };
        if let Some(cmd) = cmd {
            self.execute_sketch_command(cmd);
        }
        self.dynamic_input.clear();
        self.dynamic_focus_pending = false;
    }

    /// Toolbar kontekstual (Fase 4): tool sketch inti tetap sebagai tombol
    /// langsung (dipakai tiap sesi), sedangkan tool pemilihan titik —
    /// dipakai jauh lebih jarang — dikumpulkan di satu `menu_button` "Titik"
    /// supaya toolbar tidak penuh sesak. Label menu berubah menampilkan
    /// tool titik yang sedang aktif, jadi statusnya tetap kelihatan walau
    /// menu tertutup.
    #[allow(dead_code)]
    fn tool_buttons(&mut self, ui: &mut egui::Ui) {
        for (kind, label) in [
            (ToolKind::Select, "Pilih"),
            (ToolKind::Line, "Garis (L)"),
            (ToolKind::Rectangle, "Persegi (R)"),
            (ToolKind::Circle, "Lingkaran (C)"),
            (ToolKind::Ellipse, "Ellips (E)"),
            (ToolKind::Arc, "Arc (A)"),
            (ToolKind::Offset, "Offset (O)"),
            (ToolKind::Mirror, "Mirror (M)"),
            (ToolKind::Trim, "Trim (T)"),
            (ToolKind::Revolve, "Revolve (V)"),
        ] {
            if ui.selectable_label(self.tool == kind, label).clicked() {
                self.set_tool(kind);
            }
        }
        ui.separator();

        let point_tools = [
            (ToolKind::CoincidentPick, "Coincident (titik)"),
            (ToolKind::FixedPick, "Fixed (titik)"),
            (ToolKind::SymmetricPick, "Symmetric (titik)"),
        ];
        let active_label = point_tools
            .iter()
            .find(|(kind, _)| *kind == self.tool)
            .map(|(_, label)| format!("● {label}"))
            .unwrap_or_else(|| "Titik ▾".to_string());
        ui.menu_button(active_label, |ui| {
            for (kind, label) in point_tools {
                if ui.selectable_label(self.tool == kind, label).clicked() {
                    self.set_tool(kind);
                    ui.close();
                }
            }
        });

        let measure_tools = [
            (ToolKind::Measure, "Ukur Jarak"),
            (ToolKind::MeasureAngle, "Ukur Sudut"),
        ];
        let measure_active_label = measure_tools
            .iter()
            .find(|(kind, _)| *kind == self.tool)
            .map(|(_, label)| format!("● {label}"))
            .unwrap_or_else(|| "📏 Ukur ▾".to_string());
        ui.menu_button(measure_active_label, |ui| {
            for (kind, label) in measure_tools {
                if ui.selectable_label(self.tool == kind, label).clicked() {
                    self.set_tool(kind);
                    ui.close();
                }
            }
        });
    }

    /// Reset ke dokumen kosong — sketch, model, KEDUA undo stack, seleksi,
    /// dan path file aktif semua dibersihkan. Kamera & tema TIDAK direset
    /// (preferensi tampilan pemakai, bukan bagian dokumen).
    fn new_document(&mut self) {
        self.sketches = [Sketch::default(), Sketch::default(), Sketch::default()];
        self.undos = [
            cadraw_sketch::UndoStack::default(),
            cadraw_sketch::UndoStack::default(),
            cadraw_sketch::UndoStack::default(),
        ];
        self.model = ModelDoc::default();
        self.model_undo = cadraw_core::UndoStack::default();
        self.selected.clear();
        self.selected_bodies.clear();
        self.measurements.clear();
        self.set_tool(ToolKind::Select);
        self.current_file_path = None;
        self.file_status = Some("Dokumen baru".to_string());
    }

    /// Kumpulkan (nama, visible, shape) tiap body yang PUNYA geometri —
    /// dipakai `native::save` (SEMUA body, terlepas visible atau tidak,
    /// karena format native harus menyimpan dokumen apa adanya).
    fn native_body_refs(&self) -> Vec<(&str, bool, &KernelShape)> {
        self.model
            .doc
            .bodies
            .iter()
            .filter_map(|(id, body)| {
                self.model
                    .geometry
                    .get(id)
                    .map(|geo| (body.name.as_str(), body.visible, &geo.shape))
            })
            .collect()
    }

    /// Sama seperti `native_body_refs`, tapi cuma shape (dipakai Export
    /// STEP — SEMUA body, sama alasan dengan `native_body_refs`).
    fn all_body_shapes(&self) -> Vec<&KernelShape> {
        self.model
            .doc
            .bodies
            .keys()
            .filter_map(|id| self.model.geometry.get(id).map(|geo| &geo.shape))
            .collect()
    }

    /// (nama, mesh) body yang `visible` SAJA — dipakai Export STL/OBJ.
    /// Beda dari `native_body_refs`/`all_body_shapes`: STL/OBJ mewakili
    /// hasil cetak/tampilan fisik, bukan arsip dokumen, jadi body yang
    /// disembunyikan pemakai wajar tidak ikut (konsisten dengan
    /// `build_combined_body_mesh` yang dipakai render viewport).
    fn visible_body_meshes(&self) -> Vec<(&str, &KernelMesh)> {
        self.model
            .doc
            .bodies
            .iter()
            .filter(|(_, body)| body.visible)
            .filter_map(|(id, body)| self.model.geometry.get(id).map(|geo| (body.name.as_str(), &geo.mesh)))
            .collect()
    }

    /// Dialog "Buka" lintas platform — desktop lewat `rfd` (native picker
    /// OS), path native. Di iOS `rfd` bahkan tidak COMPILE (tidak ada
    /// backend UIKit sama sekali — dibuktikan lewat probe crate terpisah
    /// saat Fase 6, bukan cuma dugaan), jadi dependensinya digeser jadi
    /// target-specific di `Cargo.toml` (`cfg(not(target_os = "ios"))`) dan
    /// fungsi ini dapat kembaran `cfg(target_os = "ios")` — lihat
    /// `ios_documents_dir` untuk pendekatan Files.app-nya (BUKAN
    /// `UIDocumentPickerViewController` — itu masih ditunda, butuh
    /// bridging UIKit; ini pendekatan lebih sederhana yang tidak butuh
    /// dependensi tambahan sama sekali).
    #[cfg(not(target_os = "ios"))]
    fn pick_open_path(&mut self, filter_name: &str, extensions: &[&str]) -> Option<PathBuf> {
        rfd::FileDialog::new().add_filter(filter_name, extensions).pick_file()
    }

    /// iOS: TIDAK ADA picker — ambil file BERTANGGAL PALING BARU berekstensi
    /// cocok dari folder `Documents` app (lihat `ios_documents_dir`). User
    /// menaruh file di sana lewat Files.app (AirDrop/salin ke "Di iPad Ini
    /// ▸ CADRAW", muncul karena `UIFileSharingEnabled` + `LSSupports
    /// OpeningDocumentsInPlace` di `ios/Info.plist.template` — TIDAK
    /// otomatis aktif sampai project Xcode sungguhan dibuat & dipasang
    /// key itu, lihat catatan Fase 6 di PLAN.md). Heuristik "file terbaru"
    /// sengaja dipilih dibanding "satu nama tetap" supaya tetap berguna
    /// walau ada beberapa file — bukan solusi permanen, cuma jembatan
    /// sampai `UIDocumentPickerViewController` sungguhan ada.
    #[cfg(target_os = "ios")]
    fn pick_open_path(&mut self, filter_name: &str, extensions: &[&str]) -> Option<PathBuf> {
        let dir = ios_documents_dir();
        let newest = std::fs::read_dir(&dir)
            .ok()?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| extensions.iter().any(|want| want.eq_ignore_ascii_case(ext)))
            })
            .max_by_key(|path| std::fs::metadata(path).and_then(|meta| meta.modified()).ok());
        if newest.is_none() {
            self.file_status = Some(format!(
                "Tidak ada file {filter_name} di folder Documents app (Files.app ▸ Di iPad Ini ▸ CADRAW) — salin file ke sana dulu"
            ));
        }
        newest
    }

    /// Dialog "Simpan"/"Ekspor" lintas platform — pasangan `pick_open_path`
    /// di atas, alasan sama.
    #[cfg(not(target_os = "ios"))]
    fn pick_save_path(&mut self, filter_name: &str, extensions: &[&str], default_name: &str) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter(filter_name, extensions)
            .set_file_name(default_name)
            .save_file()
    }

    /// iOS: tulis ke `Documents/<default_name>` (mis. "untitled.cadraw",
    /// "export.step") — TANPA dialog nama/lokasi (belum ada picker), file
    /// lama dengan nama sama tertimpa. Terlihat & bisa di-share keluar
    /// lewat Files.app karena folder yang sama dengan `pick_open_path`.
    #[cfg(target_os = "ios")]
    fn pick_save_path(&mut self, _filter_name: &str, _extensions: &[&str], default_name: &str) -> Option<PathBuf> {
        let dir = ios_documents_dir();
        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.file_status = Some(format!("Gagal mengakses folder Documents iOS: {e}"));
            return None;
        }
        Some(dir.join(default_name))
    }

    fn save_native_to(&mut self, path: PathBuf) {
        let refs = self.native_body_refs();
        match cadraw_io::native::save_multi_plane(&path, &self.sketches, &refs) {
            Ok(()) => {
                self.file_status = Some(format!("Tersimpan: {}", path.display()));
                self.current_file_path = Some(path);
            }
            Err(e) => self.file_status = Some(format!("Gagal menyimpan: {e}")),
        }
    }

    /// "Simpan" (⌘S) — tulis ke `current_file_path` kalau sudah pernah
    /// disimpan/dibuka, atau jatuh ke `save_native_as` (dialog) kalau
    /// belum pernah sama sekali (dokumen baru).
    fn save_native(&mut self) {
        match self.current_file_path.clone() {
            Some(path) => self.save_native_to(path),
            None => self.save_native_as(),
        }
    }

    /// "Simpan Sebagai…" (⌘⇧S) — SELALU tampilkan dialog, walau dokumen
    /// sudah punya `current_file_path`.
    fn save_native_as(&mut self) {
        if let Some(path) = self.pick_save_path("CADRAW", &["cadraw"], "untitled.cadraw") {
            self.save_native_to(path);
        }
    }

    /// "Buka…" (⌘O) — mengganti SELURUH state dokumen (sketch+model),
    /// mereset kedua undo stack (undo lintas-dokumen tidak masuk akal) dan
    /// seleksi, sama pola dengan `new_document`.
    fn open_native(&mut self) {
        let Some(path) = self.pick_open_path("CADRAW", &["cadraw"]) else {
            return;
        };
        match cadraw_io::native::load(&path) {
            Ok(loaded) => {
                let cadraw_io::native::LoadedDocument {
                    sketch,
                    front_sketch,
                    right_sketch,
                    bodies,
                } = loaded;
                self.sketches = [sketch, front_sketch, right_sketch];
                self.undos = [
                    cadraw_sketch::UndoStack::default(),
                    cadraw_sketch::UndoStack::default(),
                    cadraw_sketch::UndoStack::default(),
                ];

                let mut model = ModelDoc::default();
                for body in bodies {
                    let id = model.doc.add_body(body.name);
                    if let Some(b) = model.doc.bodies.get_mut(id) {
                        b.visible = body.visible;
                    }
                    model.geometry.insert(id, BodyGeometry::from_shape(body.shape));
                }
                self.model = model;
                self.model_undo = cadraw_core::UndoStack::default();

                self.selected.clear();
                self.selected_bodies.clear();
                self.set_tool(ToolKind::Select);
                self.file_status = Some(format!("Dibuka: {}", path.display()));
                self.current_file_path = Some(path);
            }
            Err(e) => self.file_status = Some(format!("Gagal membuka file: {e}")),
        }
    }

    /// Export SEMUA body ke satu file STEP (masing-masing tetap solid
    /// terpisah — lihat `cadraw_io::step_io::export`).
    fn export_step(&mut self) {
        if self.all_body_shapes().is_empty() {
            self.file_status = Some("Tidak ada body untuk diekspor ke STEP".to_string());
            return;
        }
        // `pick_save_path` butuh `&mut self`, jadi `shapes` (berisi &referensi
        // ke self.model) tidak boleh masih hidup melintasi panggilan itu —
        // dihitung ulang SESUDAH path dipilih (murah, cuma iterasi+collect).
        let Some(path) = self.pick_save_path("STEP", &["step", "stp"], "export.step") else {
            return;
        };
        let shapes = self.all_body_shapes();
        match cadraw_io::step_io::export(&shapes, &path) {
            Ok(()) => self.file_status = Some(format!("STEP diekspor: {}", path.display())),
            Err(e) => self.file_status = Some(format!("Export STEP gagal: {e}")),
        }
    }

    /// Import satu file STEP jadi body baru — undo-able lewat
    /// `model_undo` (sama seperti Extrude), nama body dari nama file.
    /// Submit ke `import_worker` (Fase 7) — TIDAK lagi blocking UI selama
    /// `read_step`+tessellate. Body baru baru benar-benar masuk `model_undo`
    /// belakangan, saat `poll_import_worker` (dipanggil tiap frame dari
    /// `update()`) menerima hasilnya lewat channel.
    fn import_step(&mut self) {
        let Some(path) = self.pick_open_path("STEP", &["step", "stp"]) else {
            return;
        };
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Import STEP".to_string());
        self.file_status = Some(format!("Mengimpor STEP di latar belakang: {}", path.display()));
        self.import_worker.submit(ImportJob { name, path });
        self.pending_imports += 1;
    }

    /// Non-blocking, dipanggil tiap frame dari `update()`: pasang body baru
    /// untuk setiap `ImportResult` yang sudah siap dari `import_worker`.
    /// `KernelShape::from_step_string` di sini membangun shape MILIK UI
    /// thread sendiri dari teks STEP yang dikirim worker (bukan shape asli
    /// worker — itu tidak pernah menyeberang thread, lihat komentar modul
    /// `import_worker`); mesh dipakai apa adanya (sudah dihitung worker,
    /// tidak perlu tessellate ulang).
    fn poll_import_worker(&mut self) {
        for result in self.import_worker.poll() {
            self.pending_imports = self.pending_imports.saturating_sub(1);
            match result.outcome {
                Ok((step, mesh)) => match KernelShape::from_step_string(&step) {
                    Ok(shape) => {
                        let geo = BodyGeometry::from_shape_with_mesh(shape, mesh);
                        self.model_undo.execute(
                            Box::new(AddSolidCommand::new(result.name.clone(), geo)),
                            &mut self.model,
                        );
                        self.file_status = Some(format!("STEP diimpor: {}", result.name));
                    }
                    Err(e) => {
                        self.file_status = Some(format!("Import STEP gagal ({}): {e}", result.name))
                    }
                },
                Err(e) => self.file_status = Some(format!("Import STEP gagal ({}): {e}", result.name)),
            }
        }
    }

    /// Export body `visible` sebagai satu STL biner (digabung lewat
    /// `KernelMesh::merge` — sama helper yang dipakai render viewport).
    fn export_stl(&mut self) {
        let meshes = self.visible_body_meshes();
        if meshes.is_empty() {
            self.file_status = Some("Tidak ada body visible untuk diekspor ke STL".to_string());
            return;
        }
        let mesh_refs: Vec<&KernelMesh> = meshes.iter().map(|(_, m)| *m).collect();
        let merged = KernelMesh::merge(&mesh_refs);
        let Some(path) = self.pick_save_path("STL", &["stl"], "export.stl") else {
            return;
        };
        match cadraw_io::mesh_export::write_stl_binary(&merged, &path) {
            Ok(()) => self.file_status = Some(format!("STL diekspor: {}", path.display())),
            Err(e) => self.file_status = Some(format!("Export STL gagal: {e}")),
        }
    }

    /// Export body `visible` sebagai OBJ (satu blok `o <nama>` per body).
    fn export_obj(&mut self) {
        if self.visible_body_meshes().is_empty() {
            self.file_status = Some("Tidak ada body visible untuk diekspor ke OBJ".to_string());
            return;
        }
        // Sama alasan dengan `export_step`: `bodies` dihitung ulang sesudah
        // path dipilih supaya tidak melintasi panggilan `&mut self`.
        let Some(path) = self.pick_save_path("OBJ", &["obj"], "export.obj") else {
            return;
        };
        let bodies = self.visible_body_meshes();
        match cadraw_io::mesh_export::write_obj(&bodies, &path) {
            Ok(()) => self.file_status = Some(format!("OBJ diekspor: {}", path.display())),
            Err(e) => self.file_status = Some(format!("Export OBJ gagal: {e}")),
        }
    }

    /// Export entitas Line/Circle/Arc sketch aktif ke DXF R12. Ellipse
    /// dilewati (DXF R12 tidak punya entitas ELLIPSE) — dilaporkan lewat
    /// status, bukan didiamkan.
    fn export_dxf(&mut self) {
        let Some(path) = self.pick_save_path("DXF", &["dxf"], "export.dxf") else {
            return;
        };
        match cadraw_io::dxf::export(self.sketch(), &path) {
            Ok(0) => self.file_status = Some(format!("DXF diekspor: {}", path.display())),
            Ok(skipped) => {
                self.file_status = Some(format!(
                    "DXF diekspor: {} ({skipped} Ellipse dilewati — DXF R12 tidak mendukungnya)",
                    path.display()
                ))
            }
            Err(e) => self.file_status = Some(format!("Export DXF gagal: {e}")),
        }
    }

    /// Import LINE/CIRCLE/ARC dari file DXF ke sketch aktif — undo-able
    /// lewat `undo` (satu langkah `InsertEntities`, sama pola dengan
    /// menggambar tool sketch manapun).
    fn import_dxf(&mut self) {
        let Some(path) = self.pick_open_path("DXF", &["dxf"]) else {
            return;
        };
        match cadraw_io::dxf::import(&path) {
            Ok(result) => {
                let count = result.entities.len();
                if count > 0 {
                    self.execute_sketch_command(Box::new(InsertEntities::new("Import DXF", result.entities)));
                }
                self.file_status = Some(if result.skipped > 0 {
                    format!("DXF diimpor: {count} entitas ({} dilewati, jenis tak dikenal)", result.skipped)
                } else {
                    format!("DXF diimpor: {count} entitas")
                });
            }
            Err(e) => self.file_status = Some(format!("Import DXF gagal: {e}")),
        }
    }

    /// Menu "📄 File" di toolbar: dokumen baru, buka/simpan format native
    /// `.cadraw`, dan submenu Import/Export untuk STEP/DXF/STL/OBJ.
    #[allow(dead_code)]
    fn file_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("📄 File", |ui| {
            if ui.button("Baru").clicked() {
                self.new_document();
                ui.close();
            }
            ui.separator();
            if ui.button("Buka… (⌘O)").clicked() {
                self.open_native();
                ui.close();
            }
            if ui.button("Simpan (⌘S)").clicked() {
                self.save_native();
                ui.close();
            }
            if ui.button("Simpan Sebagai… (⌘⇧S)").clicked() {
                self.save_native_as();
                ui.close();
            }
            ui.separator();
            ui.menu_button("Import", |ui| {
                if ui.button("STEP…").clicked() {
                    self.import_step();
                    ui.close();
                }
                if ui.button("DXF…").clicked() {
                    self.import_dxf();
                    ui.close();
                }
            });
            ui.menu_button("Export", |ui| {
                if ui.button("STEP… (semua body)").clicked() {
                    self.export_step();
                    ui.close();
                }
                if ui.button("STL… (body visible)").clicked() {
                    self.export_stl();
                    ui.close();
                }
                if ui.button("OBJ… (body visible)").clicked() {
                    self.export_obj();
                    ui.close();
                }
                if ui.button("DXF… (sketch)").clicked() {
                    self.export_dxf();
                    ui.close();
                }
            });
        });
    }

    /// Menu "⚙ Pengaturan" di toolbar: tema, pembuka command palette, dan
    /// referensi pintasan keyboard — dikumpulkan di satu dropdown alih-alih
    /// jadi tombol lepas di toolbar utama, karena ketiganya jarang disentuh
    /// lebih dari sekali per sesi (beda dengan tool sketch yang dipakai
    /// terus-menerus).
    #[allow(dead_code)]
    fn settings_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("⚙ Pengaturan", |ui| {
            ui.label("Tema");
            if ui.button(self.theme.label()).clicked() {
                self.theme = self.theme.toggled();
                cadraw_ui::apply_theme(ui.ctx(), self.theme);
            }

            ui.separator();
            if ui.button("⌘K Buka Command Palette").clicked() {
                self.palette.open();
                ui.close();
            }

            ui.separator();
            ui.collapsing("Pintasan Keyboard", |ui| {
                egui::Grid::new("settings-keyboard-shortcuts")
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| {
                        for (key, desc) in KEYBOARD_SHORTCUTS {
                            ui.strong(key);
                            ui.label(desc);
                            ui.end_row();
                        }
                    });
            });
        });
    }

    /// Daftar aksi command palette (Fase 4), dibangun ulang tiap frame
    /// (murah — belasan entri) supaya "Hapus Seleksi" cuma muncul saat ada
    /// seleksi, dan label tool selalu sinkron dengan `ToolKind`. Index hasil
    /// `CommandPalette::show` menunjuk balik ke Vec ini.
    fn palette_actions(&self) -> Vec<(String, String, PaletteAction)> {
        let mut actions = vec![
            ("Dokumen Baru".to_string(), String::new(), PaletteAction::File(FileOp::New)),
            ("Buka…".to_string(), "⌘O".to_string(), PaletteAction::File(FileOp::Open)),
            ("Simpan".to_string(), "⌘S".to_string(), PaletteAction::File(FileOp::Save)),
            ("Simpan Sebagai…".to_string(), "⌘⇧S".to_string(), PaletteAction::File(FileOp::SaveAs)),
            ("Import STEP…".to_string(), String::new(), PaletteAction::File(FileOp::ImportStep)),
            ("Import DXF…".to_string(), String::new(), PaletteAction::File(FileOp::ImportDxf)),
            ("Export STEP… (semua body)".to_string(), String::new(), PaletteAction::File(FileOp::ExportStep)),
            ("Export STL… (body visible)".to_string(), String::new(), PaletteAction::File(FileOp::ExportStl)),
            ("Export OBJ… (body visible)".to_string(), String::new(), PaletteAction::File(FileOp::ExportObj)),
            ("Export DXF… (sketch)".to_string(), String::new(), PaletteAction::File(FileOp::ExportDxf)),
            ("Pilih".to_string(), String::new(), PaletteAction::SetTool(ToolKind::Select)),
            ("Garis".to_string(), "L".to_string(), PaletteAction::SetTool(ToolKind::Line)),
            ("Persegi".to_string(), "R".to_string(), PaletteAction::SetTool(ToolKind::Rectangle)),
            ("Lingkaran".to_string(), "C".to_string(), PaletteAction::SetTool(ToolKind::Circle)),
            ("Ellips".to_string(), "E".to_string(), PaletteAction::SetTool(ToolKind::Ellipse)),
            ("Arc".to_string(), "A".to_string(), PaletteAction::SetTool(ToolKind::Arc)),
            ("Offset".to_string(), "O".to_string(), PaletteAction::SetTool(ToolKind::Offset)),
            ("Mirror".to_string(), "M".to_string(), PaletteAction::SetTool(ToolKind::Mirror)),
            ("Trim".to_string(), "T".to_string(), PaletteAction::SetTool(ToolKind::Trim)),
            ("Revolve".to_string(), "V".to_string(), PaletteAction::SetTool(ToolKind::Revolve)),
            ("Coincident (titik)".to_string(), String::new(), PaletteAction::SetTool(ToolKind::CoincidentPick)),
            ("Fixed (titik)".to_string(), String::new(), PaletteAction::SetTool(ToolKind::FixedPick)),
            ("Symmetric (titik)".to_string(), String::new(), PaletteAction::SetTool(ToolKind::SymmetricPick)),
            ("Ukur Jarak".to_string(), String::new(), PaletteAction::SetTool(ToolKind::Measure)),
            ("Ukur Sudut".to_string(), String::new(), PaletteAction::SetTool(ToolKind::MeasureAngle)),
            ("Undo Sketch".to_string(), "⌘Z".to_string(), PaletteAction::Undo),
            ("Redo Sketch".to_string(), "⌘⇧Z".to_string(), PaletteAction::Redo),
            ("Undo Model".to_string(), String::new(), PaletteAction::ModelUndo),
            ("Redo Model".to_string(), String::new(), PaletteAction::ModelRedo),
            ("Sketch: Bidang Top (XY)".to_string(), String::new(), PaletteAction::SetSketchPlane(PlaneKind::Top)),
            ("Sketch: Bidang Vertikal Front (XZ)".to_string(), String::new(), PaletteAction::SetSketchPlane(PlaneKind::Front)),
            ("Sketch: Bidang Vertikal Right (YZ)".to_string(), String::new(), PaletteAction::SetSketchPlane(PlaneKind::Right)),
            ("Mode Sketch (2D)".to_string(), "⌘⇧2".to_string(), PaletteAction::EnterSketching),
            ("Mode 3D".to_string(), "⌘⇧3".to_string(), PaletteAction::ExitSketching),
            (
                format!("Ganti Tema ({})", self.theme.toggled().label()),
                String::new(),
                PaletteAction::ToggleTheme,
            ),
        ];
        if !self.selected.is_empty() {
            actions.push((
                format!("Hapus Seleksi ({} entitas)", self.selected.len()),
                "Del".to_string(),
                PaletteAction::DeleteSelection,
            ));
        }
        if !self.measurements.is_empty() {
            actions.push((
                format!("Hapus Semua Pengukuran ({})", self.measurements.len()),
                String::new(),
                PaletteAction::ClearMeasurements,
            ));
        }
        actions
    }

    /// Eksekusi satu `PaletteAction` — dipanggil dari `update()` saat
    /// command palette mengembalikan index terpilih.
    fn run_palette_action(&mut self, ctx: &egui::Context, action: PaletteAction) {
        match action {
            PaletteAction::SetTool(kind) => self.set_tool(kind),
            PaletteAction::SetSketchPlane(kind) => self.set_sketch_plane(kind),
            PaletteAction::EnterSketching => {
                self.is_sketching = true;
                self.left_toolbar.is_sketching = true;
                self.camera.orient_to_plane(&self.active_plane);
            }
            PaletteAction::ExitSketching => {
                self.is_sketching = false;
                self.left_toolbar.is_sketching = false;
                self.set_tool(ToolKind::Select);
            }
            PaletteAction::Undo => {
                self.undo_active_sketch();
            }
            PaletteAction::Redo => {
                self.redo_active_sketch();
            }
            PaletteAction::ModelUndo => {
                self.model_undo.undo(&mut self.model);
                self.selected_bodies.clear();
            }
            PaletteAction::ModelRedo => {
                self.model_undo.redo(&mut self.model);
                self.selected_bodies.clear();
            }
            PaletteAction::DeleteSelection => {
                if !self.selected.is_empty() {
                    let ids: Vec<_> = self.selected.drain().collect();
                    self.execute_sketch_command(Box::new(DeleteEntities::new(ids)));
                }
            }
            PaletteAction::ToggleTheme => {
                self.theme = self.theme.toggled();
                cadraw_ui::apply_theme(ctx, self.theme);
            }
            PaletteAction::ClearMeasurements => {
                self.measurements.clear();
            }
            PaletteAction::File(op) => match op {
                FileOp::New => self.new_document(),
                FileOp::Open => self.open_native(),
                FileOp::Save => self.save_native(),
                FileOp::SaveAs => self.save_native_as(),
                FileOp::ImportStep => self.import_step(),
                FileOp::ImportDxf => self.import_dxf(),
                FileOp::ExportStep => self.export_step(),
                FileOp::ExportStl => self.export_stl(),
                FileOp::ExportObj => self.export_obj(),
                FileOp::ExportDxf => self.export_dxf(),
            },
        }
    }

    /// Deteksi long-press primer di viewport (tool Pilih aktif, tekan tahan
    /// diam ≥ `LONG_PRESS_SECS` tanpa bergerak lebih dari `MOVE_TOLERANCE`
    /// piksel) dan buka radial menu di titik tekan — jalur ganti tool utama
    /// untuk sentuh (iPad), pelengkap toolbar/shortcut huruf untuk mouse.
    /// Kalau menu sudah terbuka, method ini menggambar & memprosesnya
    /// (drag ke slice, lepas untuk pilih) alih-alih mendeteksi tekan baru.
    fn handle_radial_menu(&mut self, ui: &egui::Ui, response: &egui::Response) {
        const LONG_PRESS_SECS: f64 = 0.42;
        const MOVE_TOLERANCE: f32 = 6.0;

        if self.radial_menu.is_open() {
            let items: Vec<&str> = RADIAL_TOOLS.iter().map(|(_, label)| *label).collect();
            if let Some(idx) = self.radial_menu.show(ui.ctx(), &items) {
                self.set_tool(RADIAL_TOOLS[idx].0);
            }
            return;
        }

        // Radial cuma dipicu dari tool Pilih -- tool lain sudah punya arti
        // sendiri untuk klik primer (menempatkan titik), long-press di sana
        // akan membingungkan (dua gestur bersaing untuk klik yang sama).
        if self.tool != ToolKind::Select {
            self.radial_press = None;
            return;
        }

        let now = ui.input(|i| i.time);
        if response.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_down()) {
            let pos = response
                .interact_pointer_pos()
                .unwrap_or_else(|| ui.input(|i| i.pointer.hover_pos()).unwrap_or_default());
            match self.radial_press {
                None => self.radial_press = Some((pos, now)),
                Some((start_pos, start_time)) => {
                    if pos.distance(start_pos) > MOVE_TOLERANCE {
                        // Bergerak cukup jauh -- ini drag/orbit biasa, bukan
                        // long-press diam. Batalkan deteksi.
                        self.radial_press = None;
                    } else if now - start_time >= LONG_PRESS_SECS {
                        self.radial_menu.open_at(start_pos);
                        self.radial_suppress_click = true;
                        self.radial_press = None;
                    }
                }
            }
        } else {
            self.radial_press = None;
        }
    }

    fn status_text(&self) -> String {
        let hint = match self.tool {
            ToolKind::Select => {
                "Pilih: klik entitas, Shift+klik multi-pilih, Delete hapus".to_string()
            }
            ToolKind::Line => match self.pending_points.len() {
                0 => "Garis: klik titik awal (L)".to_string(),
                _ => "Garis: klik titik akhir, atau ketik panjang lalu Enter".to_string(),
            },
            ToolKind::Rectangle => match self.pending_points.len() {
                0 => "Persegi: klik sudut pertama (R)".to_string(),
                _ => "Persegi: klik sudut berlawanan".to_string(),
            },
            ToolKind::Circle => match self.pending_points.len() {
                0 => "Lingkaran: klik titik pusat (C)".to_string(),
                _ => "Lingkaran: klik untuk radius, atau ketik radius lalu Enter".to_string(),
            },
            ToolKind::Ellipse => match self.pending_points.len() {
                0 => "Ellips: klik titik pusat (E)".to_string(),
                _ => "Ellips: klik sudut kotak pembatas".to_string(),
            },
            ToolKind::Arc => match self.pending_points.len() {
                0 => "Arc: klik titik awal (A)".to_string(),
                1 => "Arc: klik titik akhir".to_string(),
                _ => "Arc: klik titik di busur (menentukan sisi)".to_string(),
            },
            ToolKind::Offset => match self.offset_source {
                None => "Offset: klik entitas sumber (O)".to_string(),
                Some(_) => "Offset: klik sisi & jarak hasil offset".to_string(),
            },
            ToolKind::Mirror => {
                if self.selected.is_empty() {
                    "Mirror: pilih entitas di tool Pilih dulu, lalu tekan M".to_string()
                } else {
                    match self.pending_points.len() {
                        0 => format!(
                            "Mirror: klik titik 1 sumbu cermin ({} entitas terpilih)",
                            self.selected.len()
                        ),
                        _ => "Mirror: klik titik 2 sumbu cermin".to_string(),
                    }
                }
            }
            ToolKind::Trim => "Trim: klik segmen garis yang mau dipotong (T)".to_string(),
            ToolKind::Revolve => {
                if self.selected.is_empty() {
                    "Revolve: pilih profil di tool Pilih dulu, lalu tekan V".to_string()
                } else {
                    match self.pending_points.len() {
                        0 => format!(
                            "Revolve: klik titik 1 sumbu ({} entitas terpilih, 360°)",
                            self.selected.len()
                        ),
                        _ => "Revolve: klik titik 2 sumbu".to_string(),
                    }
                }
            }
            ToolKind::CoincidentPick => match self.pending_point_refs.len() {
                0 => "Coincident: klik titik pertama (endpoint/center)".to_string(),
                _ => "Coincident: klik titik kedua".to_string(),
            },
            ToolKind::FixedPick => {
                "Fixed: klik titik (endpoint/center) untuk menahannya di posisi sekarang".to_string()
            }
            ToolKind::SymmetricPick => match self.symmetric_axis() {
                None => "Symmetric: pilih 1 Line jadi sumbu di tool Pilih dulu".to_string(),
                Some(_) => match self.pending_point_refs.len() {
                    0 => "Symmetric: klik titik pertama (endpoint/center)".to_string(),
                    _ => "Symmetric: klik titik kedua".to_string(),
                },
            },
            ToolKind::Measure => match self.pending_points.len() {
                0 => "Ukur: klik titik pertama".to_string(),
                _ => "Ukur: klik titik kedua".to_string(),
            },
            ToolKind::MeasureAngle => match self.pending_points.len() {
                0 => "Ukur Sudut: klik titik awal".to_string(),
                1 => "Ukur Sudut: klik titik sudut (vertex)".to_string(),
                _ => "Ukur Sudut: klik titik akhir".to_string(),
            },
        };
        match &self.last_snap {
            Some(snap) => format!("{hint}  ·  snap: {:?}", snap.kind),
            None => hint,
        }
    }

    /// Hitung centroid rata-rata dari profil sketch tertutup yang sedang aktif terpilih
    fn selected_closed_region_centroid(&self) -> Option<DVec2> {
        if self.tool != ToolKind::Select || self.selected.is_empty() {
            return None;
        }
        let closed_regions = find_closed_regions(self.sketch());
        let selected_regions: Vec<&ClosedRegion> = closed_regions
            .iter()
            .filter(|r| r.entity_ids.is_subset(&self.selected))
            .collect();
        if selected_regions.is_empty() {
            return None;
        }
        let total_area: f64 = selected_regions.iter().map(|r| r.area.max(1e-4)).sum();
        let mut cx = 0.0;
        let mut cy = 0.0;
        for r in &selected_regions {
            cx += r.centroid.x * r.area.max(1e-4);
            cy += r.centroid.y * r.area.max(1e-4);
        }
        Some(DVec2::new(cx / total_area, cy / total_area))
    }

    /// Hitung delta pergeseran (dalam mm dunia) dari pergeseran mouse layar (egui `Vec2`),
    /// diproyeksikan langsung ke sumbu normal 3D sembarang di layar.
    fn project_screen_drag_to_world_axis(
        &self,
        rect: egui::Rect,
        origin_3d: Vec3,
        normal_3d: Vec3,
        drag_delta: egui::Vec2,
    ) -> (f64, Option<egui::Vec2>) {
        let normal = normal_3d.normalize_or_zero();
        let p_base = origin_3d;
        // Titik acuan 10 mm sepanjang vektor normal
        let p_ref = p_base + normal * 10.0;
        let s_base = world_to_screen_pos(&self.camera, rect, p_base);
        let s_ref = world_to_screen_pos(&self.camera, rect, p_ref);

        if let (Some(sb), Some(sr)) = (s_base, s_ref) {
            let arrow_vec = sr - sb; // Vektor 2D pada layar untuk 10 mm dunia
            let len_sq = arrow_vec.length_sq();
            if len_sq > 1e-4 {
                // Dot product delta mouse dengan vektor panah
                let dot = drag_delta.x * arrow_vec.x + drag_delta.y * arrow_vec.y;
                let delta_mm = (dot / len_sq) * 10.0;
                return (delta_mm as f64, Some(arrow_vec));
            }
        }

        // Fallback jika proyeksi kamera singular:
        let world_scale = pixel_tolerance_to_world(&self.camera, rect);
        ((-drag_delta.y as f64) * world_scale * 1.6, None)
    }

    /// Hitung delta pergeseran (dalam mm dunia) dari pergeseran mouse layar (egui `Vec2`),
    /// diproyeksikan langsung ke sumbu normal bidang sketsa aktif di layar.
    fn project_screen_drag_to_extrude_axis(
        &self,
        rect: egui::Rect,
        centroid: DVec2,
        drag_delta: egui::Vec2,
    ) -> (f64, Option<egui::Vec2>) {
        let p_base = self.active_plane.to_world(centroid, 0.0);
        self.project_screen_drag_to_world_axis(rect, p_base, self.active_plane.normal, drag_delta)
    }

    /// Deteksi live apakah extrude saat ini memotong solid yang ada (Smart Boolean Cut)
    fn update_gizmo_boolean_detection(&mut self) {
        if let Ok(profile) = model::build_profile_from_selection(self.sketch(), &self.selected) {
            if let Ok(swept) = self.extrude_profile_active_plane(&profile, self.gizmo_distance) {
                let mut is_cutting = false;
                for (b_id, b_geo) in self.model.geometry.iter() {
                    if let Some(body) = self.model.doc.bodies.get(b_id) {
                        if body.visible {
                            // Cek apakah ada irisan fisik nyata (volumetric overlap) antara body dan swept
                            if let Ok(intersect_shape) = cadraw_kernel::intersect(&b_geo.shape, &swept) {
                                let tri_count = intersect_shape.tessellate().triangle_count();
                                if tri_count > 0 {
                                    if let Ok(_cut_res) = cadraw_kernel::subtract(&b_geo.shape, &swept) {
                                        is_cutting = true;
                                        self.gizmo_is_cutting = true;
                                        self.gizmo_target_body = Some(b_id);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                if !is_cutting {
                    self.gizmo_is_cutting = false;
                    self.gizmo_target_body = None;
                }
            }
        }
    }

    /// Eksekusi commit extrude/cut saat drag gizmo selesai atau nilai presisi di-enter
    fn commit_gizmo_extrusion(&mut self) {
        if self.gizmo_distance.abs() > 0.1 {
            if let Ok(profile) = model::build_profile_from_selection(self.sketch(), &self.selected) {
                if let Ok(swept) = self.extrude_profile_active_plane(&profile, self.gizmo_distance) {
                    if self.gizmo_is_cutting {
                        if let Some(target_id) = self.gizmo_target_body {
                            if let Some(target_geo) = self.model.geometry.get(target_id) {
                                if let Ok(cut_res) = cadraw_kernel::subtract(&target_geo.shape, &swept) {
                                    let new_geo = BodyGeometry::from_shape(cut_res);
                                    self.model_undo.execute(
                                        Box::new(ReplaceGeometryCommand::new("Cut Extrude", target_id, new_geo)),
                                        &mut self.model,
                                    );
                                    // Riwayat rounding basi begitu geometri
                                    // diubah operasi non-rounding.
                                    self.round_history.remove(&target_id);
                                }
                            }
                        }
                    } else {
                        let geo = BodyGeometry::from_shape(swept);
                        let cmd = AddSolidCommand::new("Extrude", geo);
                        self.model_undo.execute(Box::new(cmd), &mut self.model);
                    }
                    self.selected.clear();
                }
            }
        }
        self.extruding_from_gizmo = false;
        self.gizmo_is_cutting = false;
        self.gizmo_target_body = None;
        self.gizmo_distance = 20.0;
        self.gizmo_edit_input = format!("{:.0}", self.unit.to_display_val(self.gizmo_distance));
    }

    /// Cek apakah posisi mouse saat ini berada dekat dengan gizmo panah atau dasar profil
    fn check_near_gizmo(&self, rect: egui::Rect, hover_pos: Option<egui::Pos2>) -> bool {
        let Some(pos) = hover_pos else { return false; };

        // 1. Cek gizmo sketsa 2D
        if let Some(c) = self.selected_closed_region_centroid() {
            let z_top = if self.extruding_from_gizmo { self.gizmo_distance as f32 } else { 16.0 };
            let top_3d = self.active_plane.to_world(c, z_top);
            let bot_3d = self.active_plane.to_world(c, 0.0);
            let near_top = world_to_screen_pos(&self.camera, rect, top_3d).map_or(false, |s| s.distance(pos) < 36.0);
            let near_bot = world_to_screen_pos(&self.camera, rect, bot_3d).map_or(false, |s| s.distance(pos) < 36.0);
            if near_top || near_bot {
                return true;
            }
        }

        // 2. Cek gizmo face 3D
        if let Some((_, _, hit)) = &self.active_face {
            // Fase 8 lanjutan: anchor gizmo pakai `gizmo_anchor()` (bukan
            // `centroid` mentah) — utk face lengkung `centroid` bisa jatuh
            // di dalam material (lihat dokumentasi `FaceHit::gizmo_anchor`).
            let anchor = hit.gizmo_anchor();
            let c_base = Vec3::new(anchor.0 as f32, anchor.1 as f32, anchor.2 as f32);
            // Fase 4: arah gizmo pakai `pull_dir` (radial di Cylinder/Cone/
            // Sphere), bukan `normal` — lihat dokumentasi `FaceHit::pull_dir`.
            let pull_dir = Vec3::new(hit.pull_dir.0 as f32, hit.pull_dir.1 as f32, hit.pull_dir.2 as f32);
            let dist = if self.extruding_face_from_gizmo { self.face_gizmo_distance as f32 } else { 18.0 };
            let top_3d = c_base + pull_dir * dist;
            let mid_3d = (c_base + top_3d) * 0.5;
            let near_top = world_to_screen_pos(&self.camera, rect, top_3d).map_or(false, |s| s.distance(pos) < 40.0);
            let near_bot = world_to_screen_pos(&self.camera, rect, c_base).map_or(false, |s| s.distance(pos) < 40.0);
            let near_mid = world_to_screen_pos(&self.camera, rect, mid_3d).map_or(false, |s| s.distance(pos) < 40.0);
            if near_top || near_bot || near_mid {
                return true;
            }
        }

        false
    }

    fn viewport(&mut self, ui: &mut egui::Ui) {
        let (rect, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());

        let raw_cursor = response
            .hover_pos()
            .and_then(|p| screen_to_plane_point(&self.camera, rect, p, &self.active_plane));

        self.handle_radial_menu(ui, &response);

        let is_near_gizmo = self.check_near_gizmo(rect, response.hover_pos());
        if is_near_gizmo || self.extruding_from_gizmo || self.extruding_face_from_gizmo {
            let arrow_opt = if let Some(c) = self.selected_closed_region_centroid() {
                let (_, arrow) = self.project_screen_drag_to_extrude_axis(rect, c, egui::Vec2::ZERO);
                arrow
            } else if let Some((_, _, hit)) = &self.active_face {
                let anchor = hit.gizmo_anchor();
                let c_base = Vec3::new(anchor.0 as f32, anchor.1 as f32, anchor.2 as f32);
                let pull_dir = Vec3::new(hit.pull_dir.0 as f32, hit.pull_dir.1 as f32, hit.pull_dir.2 as f32);
                let (_, arrow) = self.project_screen_drag_to_world_axis(rect, c_base, pull_dir, egui::Vec2::ZERO);
                arrow
            } else {
                None
            };
            if let Some(dir) = arrow_opt {
                let u = dir.normalized();
                let cursor = if u.x.abs() > u.y.abs() * 2.0 {
                    egui::CursorIcon::ResizeHorizontal
                } else if u.y.abs() > u.x.abs() * 2.0 {
                    egui::CursorIcon::ResizeVertical
                } else if u.x * u.y < 0.0 {
                    egui::CursorIcon::ResizeNeSw
                } else {
                    egui::CursorIcon::ResizeNwSe
                };
                ui.ctx().set_cursor_icon(cursor);
            } else {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
            }
        }

        // Direct Drag Handler untuk 3D Face Extrude Gizmo
        if let Some((_, _, hit)) = &self.active_face {
            let anchor = hit.gizmo_anchor();
            let c_base = Vec3::new(anchor.0 as f32, anchor.1 as f32, anchor.2 as f32);
            // Fase 4: proyeksi screen-space drag pakai `pull_dir` — mekanik
            // drag itu sendiri (`project_screen_drag_to_world_axis`) TIDAK
            // berubah, cuma sumbu arahnya yang sekarang bisa radial.
            let pull_dir = Vec3::new(hit.pull_dir.0 as f32, hit.pull_dir.1 as f32, hit.pull_dir.2 as f32);

            if is_near_gizmo && response.drag_started_by(egui::PointerButton::Primary) {
                self.extruding_face_from_gizmo = true;
                if self.face_gizmo_distance == 0.0 {
                    self.face_gizmo_distance = 15.0;
                }
            }

            if self.extruding_face_from_gizmo && response.dragged_by(egui::PointerButton::Primary) {
                let (delta_mm, _) = self.project_screen_drag_to_world_axis(rect, c_base, pull_dir, response.drag_delta());
                self.face_gizmo_distance += delta_mm;
                self.face_gizmo_edit_input = format!("{:.0}", self.unit.to_display_val(self.face_gizmo_distance));
            }

            if self.extruding_face_from_gizmo && response.drag_stopped() {
                if self.face_gizmo_distance.abs() > 0.1 {
                    self.extrude_active_face(self.face_gizmo_distance);
                }
                self.extruding_face_from_gizmo = false;
                self.face_gizmo_distance = 15.0;
                self.face_gizmo_edit_input = "15".to_string();
            }
        }

        // Direct Drag Handler untuk 2D Sketch Extrude Gizmo
        if let Some(c) = self.selected_closed_region_centroid() {
            if is_near_gizmo && response.drag_started_by(egui::PointerButton::Primary) {
                self.extruding_from_gizmo = true;
                if self.gizmo_distance == 0.0 {
                    self.gizmo_distance = 20.0;
                }
            }

            if self.extruding_from_gizmo && response.dragged_by(egui::PointerButton::Primary) {
                let (delta_mm, _) = self.project_screen_drag_to_extrude_axis(rect, c, response.drag_delta());
                self.gizmo_distance += delta_mm;
                self.update_gizmo_boolean_detection();
            }

            if self.extruding_from_gizmo && response.drag_stopped() {
                self.commit_gizmo_extrusion();
            }
        }

        // Orbit primer hanya untuk tool Pilih, dan cuma saat radial menu
        // TIDAK terbuka/sedang dideteksi lewat long-press dan mouse TIDAK di atas gizmo
        let radial_active = self.radial_menu.is_open() || self.radial_press.is_some();
        let allow_primary_orbit = self.tool == ToolKind::Select
            && !radial_active
            && !is_near_gizmo
            && !self.extruding_from_gizmo
            && !self.extruding_face_from_gizmo;
        self.handle_navigation(ui, &response, rect, allow_primary_orbit);
        self.handle_sketch_input(ui, &response, rect, raw_cursor);

        let aspect = rect.width() / rect.height().max(1.0);
        let world_scale = pixel_tolerance_to_world(&self.camera, rect);
        let overlay = self.build_overlay_lines(raw_cursor, world_scale);
        let (body_positions, body_normals, body_colors, body_indices) = self.build_combined_body_mesh();
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            ViewportCallback {
                view_proj: self.camera.view_proj(aspect),
                eye: self.camera.eye(),
                sketch_plane: self.active_plane,
                overlay_lines: overlay,
                body_positions,
                body_normals,
                body_colors,
                body_indices,
                clip_plane: self.section_clip_plane(),
            },
        ));

        self.dynamic_input_ui(ui, rect, raw_cursor);
    }

    fn handle_navigation(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: egui::Rect,
        allow_primary_orbit: bool,
    ) {
        let delta = response.drag_delta();
        let modifiers = ui.input(|i| i.modifiers);

        let orbiting = (allow_primary_orbit
            && response.dragged_by(egui::PointerButton::Primary)
            && !modifiers.shift
            && !self.extruding_from_gizmo
            && !self.extruding_face_from_gizmo)
            || (response.dragged_by(egui::PointerButton::Middle) && !modifiers.shift);
        let panning = response.dragged_by(egui::PointerButton::Secondary)
            || (modifiers.shift
                && !self.extruding_from_gizmo
                && !self.extruding_face_from_gizmo
                && (response.dragged_by(egui::PointerButton::Primary)
                    || response.dragged_by(egui::PointerButton::Middle)));

        if panning {
            self.camera.pan(delta.x, delta.y, rect.height());
        } else if orbiting {
            self.camera.orbit(delta.x, delta.y);
        }

        if response.hovered() {
            // Trackpad pinch zoom (2 jari pinch in/out)
            let pinch = ui.input(|i| i.zoom_delta());
            if pinch != 1.0 {
                self.camera.zoom(pinch);
            }

            // Trackpad 2-finger pan / swipe (Shapr3D trackpad navigator):
            // - 2 jari geser (pan) tanpa Shift -> Orbit / rotasi kamera
            // - Shift + 2 jari geser -> Pan / geser posisi kamera
            let smooth_scroll = ui.input(|i| i.smooth_scroll_delta);
            if smooth_scroll != egui::Vec2::ZERO {
                if modifiers.shift {
                    self.camera.pan(smooth_scroll.x, smooth_scroll.y, rect.height());
                } else if !modifiers.command && !modifiers.ctrl {
                    self.camera.orbit(smooth_scroll.x, smooth_scroll.y);
                }
            }

            let raw_wheel_y = ui.input(|i| i.raw_scroll_delta.y);
            if raw_wheel_y != 0.0 && smooth_scroll == egui::Vec2::ZERO {
                self.camera.zoom((raw_wheel_y * 0.003).exp());
            }
        }

        // Gesture multi-touch (trackpad / iPad sentuh)
        if let Some(touch) = ui.input(|i| i.multi_touch()) {
            if modifiers.shift {
                self.camera.pan(touch.translation_delta.x, touch.translation_delta.y, rect.height());
            } else {
                self.camera.orbit(touch.translation_delta.x, touch.translation_delta.y);
            }
            self.camera.zoom(touch.zoom_delta);
        }
    }

    fn handle_sketch_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: egui::Rect,
        raw_cursor: Option<DVec2>,
    ) {
        // Fase 8: picking edge/face 3D — mode ORTOGONAL terhadap `tool`
        // (lihat `PickMode`), jadi diintersep di sini SEBELUM hit-test
        // sketch biasa & shortcut tool (mengubah tool via L/R/C dst masih
        // dibiarkan jalan di bawah — cuma KLIK viewport yang dialihkan).
        if self.picking_mode != PickMode::None {
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.picking_mode = PickMode::None;
            } else {
                self.handle_3d_picking(response, rect);
            }
            return;
        }

        let text_focused = ui.ctx().memory(|m| m.focused().is_some());

        if !text_focused {
            if !self.selected.is_empty()
                && ui.input(|i| {
                    i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)
                })
            {
                let ids: Vec<_> = self.selected.drain().collect();
                self.execute_sketch_command(Box::new(DeleteEntities::new(ids)));
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                if self.active_vertex.is_some() || self.active_edge.is_some() {
                    self.active_vertex = None;
                    self.active_edge = None;
                    self.editing_round = None;
                } else if !self.pending_points.is_empty()
                    || !self.pending_point_refs.is_empty()
                    || self.offset_source.is_some()
                {
                    self.pending_points.clear();
                    self.pending_point_refs.clear();
                    self.offset_source = None;
                    self.dynamic_input.clear();
                    self.dynamic_focus_pending = false;
                } else if !self.selected.is_empty() {
                    self.selected.clear();
                } else if self.is_sketching {
                    self.is_sketching = false;
                    self.left_toolbar.is_sketching = false;
                } else {
                    self.set_tool(ToolKind::Select);
                }
            }
            if ui.input(|i| i.key_pressed(egui::Key::S)) {
                if !self.is_sketching {
                    self.is_sketching = true;
                    self.left_toolbar.is_sketching = true;
                    self.camera.orient_to_plane(&self.active_plane);
                }
            }
            if self.is_sketching {
                if ui.input(|i| i.key_pressed(egui::Key::L)) {
                    self.set_tool(ToolKind::Line);
                }
                if ui.input(|i| i.key_pressed(egui::Key::R)) {
                    self.set_tool(ToolKind::Rectangle);
                }
                if ui.input(|i| i.key_pressed(egui::Key::C)) {
                    self.set_tool(ToolKind::Circle);
                }
                if ui.input(|i| i.key_pressed(egui::Key::E)) {
                    self.set_tool(ToolKind::Ellipse);
                }
                if ui.input(|i| i.key_pressed(egui::Key::A)) {
                    self.set_tool(ToolKind::Arc);
                }
                if ui.input(|i| i.key_pressed(egui::Key::O)) {
                    self.set_tool(ToolKind::Offset);
                }
                if ui.input(|i| i.key_pressed(egui::Key::M)) {
                    self.set_tool(ToolKind::Mirror);
                }
                if ui.input(|i| i.key_pressed(egui::Key::T)) {
                    self.set_tool(ToolKind::Trim);
                }
                if ui.input(|i| i.key_pressed(egui::Key::V)) {
                    self.set_tool(ToolKind::Revolve);
                }
            }
        }

        // Konsumsi flag radial SEKALI per frame
        let suppress_click_from_radial = std::mem::take(&mut self.radial_suppress_click);

        let Some(raw) = raw_cursor else {
            self.hovered = None;
            self.last_snap = None;
            return;
        };
        let tol = pixel_tolerance_to_world(&self.camera, rect) * 14.0;
        let grid_step = 10.0;

        match self.tool {
            ToolKind::Select => {
                self.last_snap = None;

                if self.extruding_from_gizmo {
                    return;
                }

                // 2. Closed region & Entity hit testing (Klik garis atau tengah region = pilih 1 kesatuan)
                let region_hit: Option<ClosedRegion> = if !self.sketch().entities.is_empty() && response.hovered() {
                    if let Some(r) = find_region_at_point(self.sketch(), raw) {
                        Some(r)
                    } else if let Some(hit) = self.sketch().hit_test(raw, tol) {
                        find_region_containing_entity(self.sketch(), hit)
                    } else {
                        None
                    }
                } else {
                    None
                };

                self.hovered = if region_hit.is_some() {
                    None
                } else {
                    response
                        .hovered()
                        .then(|| self.sketch().hit_test(raw, tol))
                        .flatten()
                };

                // Hover highlight vertex 3D (mode 3D, tool Select) — dihitung
                // TIAP FRAME kursor ada di viewport, terpisah dari klik, supaya
                // `build_overlay_lines` bisa menyorot sudut yang bakal kena
                // SEBELUM diklik (lihat `hovered_vertex_marker`). Dilewati
                // selagi drag gizmo apa pun supaya tidak query kernel percuma
                // tiap frame drag.
                self.hovered_vertex_marker = if !self.is_sketching
                    && response.hovered()
                    && !self.filleting_vertex_from_gizmo
                    && !self.filleting_edge_from_gizmo
                    && !self.extruding_face_from_gizmo
                {
                    response
                        .hover_pos()
                        .and_then(|pos| self.pick_body_vertex_at_cursor(rect, pos))
                        .map(|(id, _, vhit)| (id, vhit))
                } else {
                    None
                };

                if response.clicked() {
                    eprintln!(
                        "[DEBUG click] response.clicked()=true, suppress_click_from_radial={}",
                        suppress_click_from_radial
                    );
                }
                if response.clicked() && !suppress_click_from_radial {
                    let shift = ui.input(|i| i.modifiers.shift);
                    let click_pos = response.hover_pos()
                        .or_else(|| ui.input(|i| i.pointer.latest_pos()))
                        .or_else(|| ui.input(|i| i.pointer.interact_pos()));

                    eprintln!(
                        "[DEBUG click] tool={:?} is_sketching={} shift={} click_pos={:?} region_hit={} hovered={:?}",
                        self.tool, self.is_sketching, shift, click_pos, region_hit.is_some(), self.hovered
                    );

                    // Di mode 3D (bukan sketching), utamakan pick sisi (face) solid
                    // DULU — proyeksi kursor ke bidang sketsa dasar sering "kena"
                    // region 2D yang sudah ter-extrude jadi solid (sketsa dasar
                    // tidak dihapus setelah extrude), padahal user mengklik untuk
                    // memilih face 3D-nya. Tanpa ini gizmo panah face tidak pernah
                    // muncul karena cabang region_hit di bawah selalu menang duluan.
                    // Vertex fillet gizmo (Fase 2): pick vertex (sudut) DULUAN,
                    // sebelum edge/face_pick_3d — target vertex kecil secara
                    // visual dan sering "ketutup" face di baliknya, jadi harus
                    // menang prioritas kalau kena keduanya. Edge fillet gizmo
                    // ("klik rusuk pojok kubus"): dicoba SETELAH vertex meleset,
                    // SEBELUM face — kasus "klik sudut kubus" yang sebenarnya
                    // jatuh di rusuk vertikal pojok (bukan titik vertex-nya)
                    // harus tetap kena gizmo rounding, bukan langsung jatuh ke
                    // face di baliknya (lihat `pick_body_edge_at_cursor`).
                    // Face pick dihitung PALING AWAL (walau prioritas
                    // pilihnya paling akhir) karena titik hit face dipakai
                    // tes intersepsi "edit rounding": klik pada sudut/rusuk
                    // yang SUDAH dibulatkan harus membuka KEMBALI gizmo
                    // rounding untuk mengubah radius fitur itu (termasuk
                    // push sampai 0 = kembali menyiku), BUKAN jatuh ke
                    // vertex/edge pick baru di patch fillet (menumpuk
                    // fillet di atas fillet) apalagi ke gizmo extrude face
                    // (yang malah membesarkan objek).
                    let face_pick_3d = if !self.is_sketching && !shift {
                        eprintln!("[DEBUG click] mencoba face_pick_3d (mode 3D, tanpa shift)...");
                        click_pos.and_then(|pos| self.pick_body_face_at_cursor(rect, pos))
                    } else {
                        eprintln!(
                            "[DEBUG click] SKIP face_pick_3d karena is_sketching={} atau shift={}",
                            self.is_sketching, shift
                        );
                        None
                    };

                    let round_edit = face_pick_3d.as_ref().and_then(|(b_id, _, hit)| {
                        self.find_round_feature_near(*b_id, hit.hit_point, rect)
                            .map(|idx| (*b_id, idx))
                    });

                    let vertex_pick_3d = if round_edit.is_none() && !self.is_sketching && !shift {
                        eprintln!("[DEBUG click] mencoba vertex_pick_3d (mode 3D, tanpa shift)...");
                        click_pos.and_then(|pos| self.pick_body_vertex_at_cursor(rect, pos))
                    } else {
                        None
                    };

                    let edge_pick_3d = if round_edit.is_none() && vertex_pick_3d.is_none() && !self.is_sketching && !shift {
                        eprintln!("[DEBUG click] mencoba edge_pick_3d (mode 3D, tanpa shift)...");
                        click_pos.and_then(|pos| self.pick_body_edge_at_cursor(rect, pos))
                    } else {
                        None
                    };

                    if let Some((b_id, idx)) = round_edit {
                        eprintln!("[DEBUG click] -> cabang ROUND_EDIT diambil, body={b_id:?} fitur #{idx}");
                        let feature = self.round_history[&b_id].features[idx].clone();
                        self.selected.clear();
                        self.selected_bodies.clear();
                        self.selected_bodies.insert(b_id);
                        self.editing_round = Some((b_id, idx));
                        self.active_face = None;
                        match feature.kind {
                            RoundKind::Vertex => {
                                self.active_vertex = Some((b_id, feature.ray, feature.anchor));
                                self.active_edge = None;
                                self.vertex_gizmo_radius = feature.radius;
                                self.vertex_gizmo_edit_input =
                                    format!("{:.1}", self.unit.to_display_val(feature.radius));
                            }
                            RoundKind::Edge => {
                                self.active_edge = Some((b_id, feature.ray, feature.anchor));
                                self.active_vertex = None;
                                self.edge_gizmo_radius = feature.radius;
                                self.edge_gizmo_edit_input =
                                    format!("{:.1}", self.unit.to_display_val(feature.radius));
                            }
                        }
                        self.model_status = Some(
                            "Rounding terpilih — tarik/dorong handle utk ubah radius, dorong sampai 0 utk kembali menyiku".to_string(),
                        );
                    } else if let Some((b_id, ray, vhit)) = vertex_pick_3d {
                        eprintln!("[DEBUG click] -> cabang VERTEX_PICK_3D diambil, body={b_id:?}");
                        self.selected.clear();
                        self.selected_bodies.clear();
                        self.selected_bodies.insert(b_id);
                        self.active_vertex = Some((b_id, ray, vhit));
                        self.active_face = None;
                        self.active_edge = None;
                        self.editing_round = None;
                        self.vertex_gizmo_radius = 3.0;
                        self.vertex_gizmo_edit_input = "3".to_string();
                        self.model_status = Some("Sudut (vertex) 3D terpilih — masukkan radius fillet".to_string());
                    } else if let Some((b_id, ray, point)) = edge_pick_3d {
                        eprintln!("[DEBUG click] -> cabang EDGE_PICK_3D diambil, body={b_id:?}");
                        self.selected.clear();
                        self.selected_bodies.clear();
                        self.selected_bodies.insert(b_id);
                        self.active_edge = Some((b_id, ray, point));
                        self.active_face = None;
                        self.active_vertex = None;
                        self.editing_round = None;
                        self.edge_gizmo_radius = 3.0;
                        self.edge_gizmo_edit_input = "3".to_string();
                        self.model_status = Some("Rusuk (edge) 3D terpilih — masukkan radius fillet".to_string());
                    } else if let Some((b_id, ray, hit)) = face_pick_3d {
                        eprintln!("[DEBUG click] -> cabang FACE_PICK_3D diambil, body={b_id:?}");
                        self.selected.clear();
                        self.selected_bodies.clear();
                        self.selected_bodies.insert(b_id);
                        self.active_face = Some((b_id, ray, hit));
                        self.active_vertex = None;
                        self.active_edge = None;
                        self.editing_round = None;
                        self.face_gizmo_distance = 15.0;
                        self.face_gizmo_edit_input = "15".to_string();
                        self.model_status = Some("Sisi (face) 3D terpilih — tarik panah gizmo atau masukkan jarak extrude".to_string());
                    } else if let Some(reg) = region_hit {
                        eprintln!("[DEBUG click] -> cabang REGION_HIT diambil (sketsa 2D menang)");
                        self.active_face = None;
                        self.active_vertex = None;
                        self.active_edge = None;
                        self.editing_round = None;
                        if shift {
                            let already_selected = reg.entity_ids.iter().all(|id| self.selected.contains(id));
                            if already_selected {
                                for id in &reg.entity_ids {
                                    self.selected.remove(id);
                                }
                            } else {
                                for id in &reg.entity_ids {
                                    self.selected.insert(*id);
                                }
                            }
                        } else {
                            self.selected.clear();
                            for id in &reg.entity_ids {
                                self.selected.insert(*id);
                            }
                        }
                        self.gizmo_distance = 20.0;
                        self.gizmo_edit_input = format!("{:.0}", self.unit.to_display_val(self.gizmo_distance));
                    } else {
                        eprintln!("[DEBUG click] -> cabang HOVERED/FALLBACK, self.hovered={:?} shift={}", self.hovered, shift);
                        match (self.hovered, shift) {
                            (Some(hit), true) => {
                                if !self.selected.remove(&hit) {
                                    self.selected.insert(hit);
                                }
                            }
                            (Some(hit), false) => {
                                self.selected.clear();
                                self.active_face = None;
                                self.active_vertex = None;
                                self.active_edge = None;
                                self.selected.insert(hit);
                            }
                            (None, false) => {
                                self.selected.clear();
                                eprintln!("[DEBUG click]    fallback (None,false) -> coba pick_body_face_at_cursor lagi");
                                if let Some(pos) = click_pos {
                                    if let Some((b_id, ray, hit)) = self.pick_body_face_at_cursor(rect, pos) {
                                        self.selected_bodies.clear();
                                        self.selected_bodies.insert(b_id);
                                        self.active_face = Some((b_id, ray, hit));
                                        self.active_vertex = None;
                                        self.active_edge = None;
                                        self.face_gizmo_distance = 15.0;
                                        self.face_gizmo_edit_input = "15".to_string();
                                        self.model_status = Some("Sisi (face) 3D terpilih — tarik panah gizmo atau masukkan jarak extrude".to_string());
                                    } else {
                                        eprintln!("[DEBUG click]    fallback pick_body_face_at_cursor JUGA None -> active_face/active_vertex di-clear");
                                        self.active_face = None;
                                        self.active_vertex = None;
                                        self.active_edge = None;
                                    }
                                } else {
                                    eprintln!("[DEBUG click]    fallback: click_pos None sama sekali (hover_pos/pointer semua None)");
                                }
                            }
                            (None, true) => {}
                        }
                    }
                    self.constraint_status = None;
                }
            }
            ToolKind::Line | ToolKind::Rectangle | ToolKind::Circle | ToolKind::Ellipse
            | ToolKind::Arc | ToolKind::Measure | ToolKind::MeasureAngle => {
                self.hovered = None;
                self.last_snap = response
                    .hovered()
                    .then(|| find_snap(self.sketch(), raw, tol, grid_step, None))
                    .flatten();
                if response.clicked() {
                    let effective = self.snapped_or(raw);
                    self.on_click_point(effective);
                }
            }
            ToolKind::Mirror | ToolKind::Revolve => {
                self.hovered = None;
                self.last_snap = None;
                if !self.selected.is_empty() {
                    self.last_snap = response
                        .hovered()
                        .then(|| find_snap(self.sketch(), raw, tol, grid_step, None))
                        .flatten();
                    if response.clicked() {
                        let effective = self.snapped_or(raw);
                        self.on_click_point(effective);
                    }
                }
            }
            ToolKind::Offset => {
                self.last_snap = None;
                match self.offset_source {
                    None => {
                        self.hovered = response
                            .hovered()
                            .then(|| self.sketch().hit_test(raw, tol))
                            .flatten();
                        if response.clicked() {
                            self.offset_source = self.hovered;
                        }
                    }
                    Some(source_id) => {
                        self.hovered = None;
                        if response.clicked() {
                            if let Some(entity) = self.sketch().entities.get(source_id) {
                                if let Some(new_entity) = offset_entity(entity, raw) {
                                    self.execute_sketch_command(
                                        Box::new(InsertEntities::new("Offset", vec![new_entity])),
                                    );
                                }
                            }
                            self.offset_source = None;
                        }
                    }
                }
            }
            ToolKind::Trim => {
                self.last_snap = None;
                self.hovered = response
                    .hovered()
                    .then(|| self.sketch().hit_test(raw, tol))
                    .flatten()
                    .filter(|id| matches!(self.sketch().entities.get(*id), Some(Entity::Line { .. })));
                if response.clicked() {
                    if let Some(id) = self.hovered {
                        if let Some(Entity::Line { start, end }) =
                            self.sketch().entities.get(id).cloned()
                        {
                            let click_t = project_t(start, end, raw).clamp(0.0, 1.0);
                            let cuts =
                                line_intersection_params_in_sketch(self.sketch(), (start, end), id);
                            let remaining = trim_segments(start, end, &cuts, click_t);
                            let new_lines = remaining
                                .into_iter()
                                .map(|(s, e)| Entity::Line { start: s, end: e })
                                .collect();
                            self.execute_sketch_command(
                                Box::new(ReplaceEntities::new("Trim", vec![id], new_lines)),
                            );
                            self.hovered = None;
                        }
                    }
                }
            }
            ToolKind::CoincidentPick => {
                self.hovered = None;
                self.last_snap = response
                    .hovered()
                    .then(|| find_snap(self.sketch(), raw, tol, grid_step, None))
                    .flatten();
                if response.clicked() {
                    if let Some(source) = self.last_snap.and_then(|s| s.source) {
                        self.pending_point_refs.push(source);
                        if self.pending_point_refs.len() >= 2 {
                            let refs = std::mem::take(&mut self.pending_point_refs);
                            self.apply_constraint(Constraint::Coincident { a: refs[0], b: refs[1] });
                        }
                    }
                }
            }
            ToolKind::FixedPick => {
                self.hovered = None;
                self.last_snap = response
                    .hovered()
                    .then(|| find_snap(self.sketch(), raw, tol, grid_step, None))
                    .flatten();
                if response.clicked() {
                    if let Some(hit) = self.last_snap {
                        if let Some(source) = hit.source {
                            self.apply_constraint(Constraint::Fixed {
                                point: source,
                                target: hit.point,
                            });
                        }
                    }
                }
            }
            ToolKind::SymmetricPick => {
                self.hovered = None;
                self.last_snap = None;
                if let Some(axis_id) = self.symmetric_axis() {
                    self.last_snap = response
                        .hovered()
                        .then(|| find_snap(self.sketch(), raw, tol, grid_step, Some(axis_id)))
                        .flatten();
                    if response.clicked() {
                        if let Some(source) = self.last_snap.and_then(|s| s.source) {
                            self.pending_point_refs.push(source);
                            if self.pending_point_refs.len() >= 2 {
                                let refs = std::mem::take(&mut self.pending_point_refs);
                                self.apply_constraint(Constraint::Symmetric {
                                    a: refs[0],
                                    b: refs[1],
                                    axis: axis_id,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    /// Klik viewport saat `picking_mode` aktif (Fase 8)
    fn handle_3d_picking(&mut self, response: &egui::Response, rect: egui::Rect) {
        if !response.clicked() {
            return;
        }
        let Some(pos) = response.interact_pointer_pos() else {
            return;
        };
        let Some(&id) = self.selected_bodies.iter().next().filter(|_| self.selected_bodies.len() == 1) else {
            return;
        };
        let Some(geo) = self.model.geometry.get(id) else {
            return;
        };
        let (origin, dir) = screen_to_ray(&self.camera, rect, pos);
        let ray = PickRay {
            origin: (origin.x as f64, origin.y as f64, origin.z as f64),
            dir: (dir.x as f64, dir.y as f64, dir.z as f64),
        };
        match self.picking_mode {
            PickMode::None => {}
            PickMode::Edge => {
                let tol = pixel_tolerance_to_world(&self.camera, rect) * 14.0;
                if let Some((_, polyline)) = cadraw_kernel::pick_edge(&geo.shape, ray, tol) {
                    self.selected_edges.push(PickedEdge { ray, polyline });
                }
            }
            PickMode::Face => {
                if let Some(hit) = cadraw_kernel::pick_face_details(&geo.shape, ray) {
                    self.selected_faces.push(ray);
                    self.active_face = Some((id, ray, hit));
                }
            }
        }
    }

    fn build_overlay_lines(&self, raw_cursor: Option<DVec2>, world_scale: f64) -> Vec<LineVertex> {
        let mut verts = Vec::new();

        // 1. Gambar seluruh entitas sketsa di SEMUA bidang pada posisi 3D masing-masing
        for idx in 0..3 {
            let plane = Self::plane_for_index(idx);
            if idx == self.active_plane_index() {
                verts.extend(sketch_render::entity_lines(&self.sketches[idx], self.hovered, &self.selected, &plane));
            } else {
                verts.extend(sketch_render::inactive_entity_lines(&self.sketches[idx], &plane));
            }
        }

        // Gambar Gizmo Panah 2 Sisi (↕) dan garis putus-putus extrude jika profil tertutup terpilih (Screenshot 2, 3, 4)
        if let Some(centroid) = self.selected_closed_region_centroid() {
            let c_base_pt = self.active_plane.to_world(centroid, 0.02);
            let c_base = [c_base_pt.x, c_base_pt.y, c_base_pt.z];
            const GIZMO_ARROW_COLOR: [f32; 4] = [0.0, 0.78, 1.0, 1.0];

            if self.extruding_from_gizmo {
                let c_top_pt = self.active_plane.to_world(centroid, self.gizmo_distance as f32);
                let c_top = [c_top_pt.x, c_top_pt.y, c_top_pt.z];
                // Garis putus-putus dari base ke ketinggian extrude
                verts.extend(sketch_render::dashed_line_3d(c_base, c_top, 4.0, [0.15, 0.70, 1.0, 0.95]));
                // Gizmo panah tebal di posisi ujung extrude
                verts.extend(sketch_render::double_arrow_gizmo_lines(c_top, 22.0, 5.0, GIZMO_ARROW_COLOR, self.active_plane.normal));
            } else {
                let gizmo_pt = self.active_plane.to_world(centroid, 18.0);
                let gizmo_pos = [gizmo_pt.x, gizmo_pt.y, gizmo_pt.z];
                verts.extend(sketch_render::dashed_line_3d(c_base, gizmo_pos, 2.5, [0.15, 0.70, 1.0, 0.75]));
                verts.extend(sketch_render::double_arrow_gizmo_lines(gizmo_pos, 22.0, 5.0, GIZMO_ARROW_COLOR, self.active_plane.normal));
            }
        }

        // Gambar Gizmo Panah 3D jika ada sisi (face) solid terpilih
        if let Some((_, _, hit)) = &self.active_face {
            let anchor = hit.gizmo_anchor();
            let c_base = [anchor.0 as f32, anchor.1 as f32, anchor.2 as f32];
            // Fase 4: panah gizmo mengikuti `pull_dir` (radial di
            // Cylinder/Cone/Sphere, sama seperti `normal` di Plane) —
            // supaya panah visual menunjuk arah yang benar-benar mengubah
            // radius, bukan normal permukaan lokal yang konstan per-face.
            let pull_dir = Vec3::new(hit.pull_dir.0 as f32, hit.pull_dir.1 as f32, hit.pull_dir.2 as f32);
            const FACE_GIZMO_COLOR: [f32; 4] = [0.0, 0.85, 1.0, 1.0];

            if self.extruding_face_from_gizmo {
                let dist = self.face_gizmo_distance as f32;
                let c_top = Vec3::from(c_base) + pull_dir * dist;
                let c_top_arr = [c_top.x, c_top.y, c_top.z];
                verts.extend(sketch_render::dashed_line_3d(c_base, c_top_arr, 4.0, [0.15, 0.80, 1.0, 0.95]));
                verts.extend(sketch_render::double_arrow_gizmo_lines(c_top_arr, 24.0, 5.5, FACE_GIZMO_COLOR, pull_dir));
            } else {
                let gizmo_pt = Vec3::from(c_base) + pull_dir * 18.0;
                let gizmo_pos = [gizmo_pt.x, gizmo_pt.y, gizmo_pt.z];
                verts.extend(sketch_render::dashed_line_3d(c_base, gizmo_pos, 2.5, [0.15, 0.80, 1.0, 0.85]));
                verts.extend(sketch_render::double_arrow_gizmo_lines(gizmo_pos, 24.0, 5.5, FACE_GIZMO_COLOR, pull_dir));
            }
        }

        // Gambar Gizmo Vertex Fillet (Fase 3 — Rounded Sudut) jika ada
        // sudut (vertex) solid terpilih: kotak kawat kecil di vertex +
        // garis putus-putus ke handle + ikon kuadran lingkaran, warna
        // dibedakan dari `FACE_GIZMO_COLOR` supaya tidak tertukar visual.
        if let Some((vertex, out_dir)) = self.active_vertex_gizmo_dir() {
            const VERTEX_GIZMO_COLOR: [f32; 4] = [1.0, 0.35, 0.85, 1.0];
            let handle_dist = if self.filleting_vertex_from_gizmo {
                self.vertex_gizmo_radius.max(0.1) as f32
            } else {
                12.0
            };
            verts.extend(sketch_render::vertex_fillet_marker_lines(
                [vertex.x, vertex.y, vertex.z],
                out_dir,
                handle_dist,
                VERTEX_GIZMO_COLOR,
            ));
        }

        // Gambar Gizmo Edge Fillet ("klik rusuk pojok kubus" -> rusuk
        // membulat) — visual SAMA dgn gizmo vertex fillet di atas
        // (ikon+garis putus-putus+kuadran), warna sama juga supaya
        // keduanya jelas satu keluarga "gizmo rounding", cuma berlabuh di
        // titik KLIK pada rusuk (bukan titik vertex resmi B-rep), lihat
        // `active_edge_gizmo_dir`.
        if let Some((point, out_dir)) = self.active_edge_gizmo_dir() {
            const EDGE_ROUND_GIZMO_COLOR: [f32; 4] = [1.0, 0.35, 0.85, 1.0];
            let handle_dist = if self.filleting_edge_from_gizmo {
                self.edge_gizmo_radius.max(0.1) as f32
            } else {
                12.0
            };
            verts.extend(sketch_render::vertex_fillet_marker_lines(
                [point.x, point.y, point.z],
                out_dir,
                handle_dist,
                EDGE_ROUND_GIZMO_COLOR,
            ));
        }

        // Marker vertex 3D (sudut yang bisa diklik utk gizmo rounding) —
        // digambar di SEMUA sudut body visible saat mode 3D, supaya user
        // tahu ke mana harus klik (keluhan awal fitur ini: target vertex
        // tanpa feedback visual, "praktis mustahil dikenai"). Sudut yang
        // sedang di-hover kursor SEKARANG (`hovered_vertex_marker`,
        // dihitung tiap frame di `handle_sketch_input`) digambar lebih
        // besar + warna beda supaya jelas mana yang bakal kena kalau
        // diklik.
        if !self.is_sketching {
            const VERTEX_MARKER_COLOR: [f32; 4] = [0.85, 0.85, 0.92, 0.55];
            const VERTEX_MARKER_HOVER_COLOR: [f32; 4] = [1.0, 0.85, 0.15, 1.0];
            for (id, geo) in self.model.geometry.iter() {
                let visible = self.model.doc.bodies.get(id).is_some_and(|b| b.visible);
                if !visible {
                    continue;
                }
                let vertices: Vec<[f32; 3]> = cadraw_kernel::shape_vertices(&geo.shape)
                    .into_iter()
                    .map(|(x, y, z)| [x as f32, y as f32, z as f32])
                    .collect();
                let hover_point = self
                    .hovered_vertex_marker
                    .and_then(|(hid, hv)| (hid == id).then_some([hv.0 as f32, hv.1 as f32, hv.2 as f32]));
                verts.extend(sketch_render::vertex_dot_markers(
                    &vertices,
                    hover_point,
                    VERTEX_MARKER_COLOR,
                    VERTEX_MARKER_HOVER_COLOR,
                ));
            }
        }

        // Pengukuran (Fase 7) tergambar permanen — garis + kepala panah kedua
        // ujung bergaya dimension line (↔), nilai jaraknya ditampilkan langsung
        // di atas garis lewat pill di `dynamic_input_ui` (lihat `inline_value`).
        for measurement in &self.measurements {
            let pts = measurement.points();
            verts.extend(sketch_render::measurement_lines(&pts, &self.active_plane));
            verts.extend(sketch_render::measurement_arrowheads(&pts, &self.active_plane));
        }

        // Highlight tepi 3D terpilih via picking
        const EDGE_PICK_COLOR: [f32; 4] = [1.0, 0.55, 0.15, 1.0];
        for picked in &self.selected_edges {
            for pair in picked.polyline.windows(2) {
                verts.push(LineVertex {
                    position: [pair[0].0 as f32, pair[0].1 as f32, pair[0].2 as f32],
                    color: EDGE_PICK_COLOR,
                });
                verts.push(LineVertex {
                    position: [pair[1].0 as f32, pair[1].1 as f32, pair[1].2 as f32],
                    color: EDGE_PICK_COLOR,
                });
            }
        }

        // Offset preview
        if self.tool == ToolKind::Offset {
            if let Some(entity) = self.offset_source.and_then(|id| self.sketch().entities.get(id)) {
                verts.extend(sketch_render::preview_lines(entity, &self.active_plane));
            }
        }

        // Coincident / Symmetric markers
        if matches!(self.tool, ToolKind::CoincidentPick | ToolKind::SymmetricPick) {
            for pr in &self.pending_point_refs {
                if let Some(p) = constraint::point_ref_position(self.sketch(), pr) {
                    verts.extend(sketch_render::picked_point_glyph(p, &self.active_plane));
                }
            }
        }

        if let Some(raw) = raw_cursor {
            let offset_dist = (14.0 * world_scale).max(8.0);
            match self.tool {
                ToolKind::Line if self.pending_points.len() == 1 => {
                    let start = self.pending_points[0];
                    let end = self.snapped_or(raw);
                    let preview = Entity::Line { start, end };
                    verts.extend(sketch_render::preview_lines(&preview, &self.active_plane));
                    verts.extend(sketch_render::dimension_leader_lines(start, end, offset_dist, &self.active_plane));
                }
                ToolKind::Rectangle if self.pending_points.len() == 1 => {
                    let first = self.pending_points[0];
                    let effective = self.snapped_or(raw);
                    let min = first.min(effective);
                    let max = first.max(effective);
                    let corners = [
                        DVec2::new(min.x, min.y),
                        DVec2::new(max.x, min.y),
                        DVec2::new(max.x, max.y),
                        DVec2::new(min.x, max.y),
                    ];
                    for i in 0..4 {
                        let preview = Entity::Line {
                            start: corners[i],
                            end: corners[(i + 1) % 4],
                        };
                        verts.extend(sketch_render::preview_lines(&preview, &self.active_plane));
                    }
                    // Leader lines untuk lebar dan tinggi (Screenshot 1)
                    verts.extend(sketch_render::dimension_leader_lines(corners[0], corners[1], offset_dist, &self.active_plane));
                    verts.extend(sketch_render::dimension_leader_lines(corners[1], corners[2], offset_dist, &self.active_plane));
                }
                ToolKind::Circle if self.pending_points.len() == 1 => {
                    let first = self.pending_points[0];
                    let effective = self.snapped_or(raw);
                    let radius = (effective - first).length();
                    let preview = Entity::Circle {
                        center: first,
                        radius,
                    };
                    verts.extend(sketch_render::preview_lines(&preview, &self.active_plane));
                    verts.extend(sketch_render::dimension_leader_lines(first, effective, offset_dist * 0.5, &self.active_plane));
                }
                ToolKind::Ellipse if self.pending_points.len() == 1 => {
                    let first = self.pending_points[0];
                    let effective = self.snapped_or(raw);
                    let radius_x = (effective.x - first.x).abs();
                    let radius_y = (effective.y - first.y).abs();
                    if radius_x > 1e-6 && radius_y > 1e-6 {
                        let preview = Entity::Ellipse {
                            center: first,
                            radius_x,
                            radius_y,
                        };
                        verts.extend(sketch_render::preview_lines(&preview, &self.active_plane));
                    }
                }
                ToolKind::Arc => {
                    let effective = self.snapped_or(raw);
                    match self.pending_points.len() {
                        1 => {
                            let preview = Entity::Line {
                                start: self.pending_points[0],
                                end: effective,
                            };
                            verts.extend(sketch_render::preview_lines(&preview, &self.active_plane));
                            verts.extend(sketch_render::dimension_leader_lines(self.pending_points[0], effective, offset_dist, &self.active_plane));
                        }
                        2 => {
                            if let Some(preview) = arc_from_three_points(
                                self.pending_points[0],
                                self.pending_points[1],
                                effective,
                            ) {
                                verts.extend(sketch_render::preview_lines(&preview, &self.active_plane));
                            }
                        }
                        _ => {}
                    }
                }
                ToolKind::Mirror if !self.selected.is_empty() && self.pending_points.len() == 1 => {
                    let axis_a = self.pending_points[0];
                    let axis_b = self.snapped_or(raw);
                    let axis_preview = Entity::Line {
                        start: axis_a,
                        end: axis_b,
                    };
                    verts.extend(sketch_render::preview_lines(&axis_preview, &self.active_plane));
                    for entity in self
                        .selected
                        .iter()
                        .filter_map(|id| self.sketch().entities.get(*id))
                    {
                        if let Some(mirrored) = mirror_entity(entity, axis_a, axis_b) {
                            verts.extend(sketch_render::preview_lines(&mirrored, &self.active_plane));
                        }
                    }
                }
                ToolKind::Revolve if !self.selected.is_empty() && self.pending_points.len() == 1 => {
                    let axis_preview = Entity::Line {
                        start: self.pending_points[0],
                        end: self.snapped_or(raw),
                    };
                    verts.extend(sketch_render::preview_lines(&axis_preview, &self.active_plane));
                }
                ToolKind::Offset => {
                    if let Some(entity) =
                        self.offset_source.and_then(|id| self.sketch().entities.get(id))
                    {
                        if let Some(preview) = offset_entity(entity, raw) {
                            verts.extend(sketch_render::preview_lines(&preview, &self.active_plane));
                        }
                    }
                }
                ToolKind::Trim => {
                    if let Some(id) = self.hovered {
                        if let Some((a, b)) = trim_removal_preview(self.sketch(), id, raw) {
                            verts.extend(sketch_render::removal_preview_lines(a, b, &self.active_plane));
                        }
                    }
                }
                ToolKind::Measure | ToolKind::MeasureAngle => {
                    let effective = self.snapped_or(raw);
                    let mut preview_points = self.pending_points.clone();
                    preview_points.push(effective);
                    verts.extend(sketch_render::measurement_lines(&preview_points, &self.active_plane));
                    verts.extend(sketch_render::measurement_arrowheads(&preview_points, &self.active_plane));
                }
                _ => {}
            }
        }

        if let Some(hit) = &self.last_snap {
            verts.extend(sketch_render::snap_glyph(hit, &self.active_plane));
        }

        verts
    }

    /// Sudut layar (radian, dinormalisasi ke -90°..90°) garis dunia `a`→`b`
    /// pada bidang aktif — dipakai memutar pill nominal pengukuran
    /// (`CanvasHud::render_dimension_pill_aligned`) supaya sejajar garisnya
    /// sendiri alih-alih selalu horizontal, biar tidak numpuk dengan garis
    /// pengukuran lain yang miring. Dinormalisasi (bukan angle mentah
    /// `Vec2::angle()`) supaya teksnya tidak pernah kebalik/terbaca dari
    /// bawah ke atas kalau garisnya miring "ke kiri". Fallback 0.0 (horizontal)
    /// kalau salah satu ujung gagal diproyeksikan ke layar (di belakang kamera).
    fn screen_line_angle(&self, rect: egui::Rect, a: DVec2, b: DVec2) -> f32 {
        let a_3d = self.active_plane.to_world(a, 0.0);
        let b_3d = self.active_plane.to_world(b, 0.0);
        self.screen_angle_between_world_points(rect, a_3d, b_3d)
    }

    /// Sama seperti `screen_line_angle`, tapi menerima titik DUNIA langsung
    /// (bukan koordinat 2D bidang sketsa aktif) — dipakai rusuk 3D body
    /// (`render_all_element_dimensions`) yang endpoint-nya sudah dalam
    /// world space (`EdgeDimension::start`/`end`, lihat `cadraw-kernel`),
    /// tidak lewat `active_plane.to_world`. Dihitung ULANG dari proyeksi
    /// kamera TIAP FRAME (bukan sekali saat body dibuat) — makanya label
    /// tetap sejajar rusuknya masing-masing walau kamera diputar-putar.
    fn screen_angle_between_world_points(&self, rect: egui::Rect, a_3d: Vec3, b_3d: Vec3) -> f32 {
        match (
            world_to_screen_pos(&self.camera, rect, a_3d),
            world_to_screen_pos(&self.camera, rect, b_3d),
        ) {
            (Some(pa), Some(pb)) => {
                let mut angle = (pb - pa).angle();
                if angle > std::f32::consts::FRAC_PI_2 {
                    angle -= std::f32::consts::PI;
                } else if angle < -std::f32::consts::FRAC_PI_2 {
                    angle += std::f32::consts::PI;
                }
                angle
            }
            _ => 0.0,
        }
    }

    /// Label nominal ukuran SEMUA elemen — dipanggil `dynamic_input_ui` saat
    /// checkbox "Tampilkan Semua Ukuran" (kartu Pengukuran, ruler properties
    /// panel Properties kanan) aktif. Dua sumber independen digambar
    /// bersamaan karena kanvas CADRAW selalu menumpuk sketsa bidang aktif +
    /// body 3D sekaligus (tidak ada toggle mode 2D/3D terpisah):
    /// - Sketsa 2D: SEMUA entitas (Line/Circle/Arc/Ellipse) di bidang aktif,
    ///   dihitung ulang tiap frame (murah — cuma aritmatika DVec2, sama
    ///   biayanya dengan preview pill tool Line/Rectangle/Circle di bawah).
    /// - 3D: SEMUA rusuk dari SEMUA body yang visible, dari `edge_dims` yang
    ///   sudah di-cache SEKALI per body (lihat `BodyGeometry::from_shape`)
    ///   — bukan panggilan kernel OCCT tiap frame.
    ///
    /// Body hasil Extrude/Loft/Revolve dibangun DARI profil sketsa bidang
    /// aktif, dan entitasnya TETAP ada di `self.sketch()` sesudahnya (biar
    /// bisa diedit ulang) — jadi rusuk DASAR body sering berimpit PERSIS
    /// dgn entitas Line 2D di atas: tanpa dedup, nominalnya dobel tepat di
    /// titik "sumber sketsa jadi 3D" itu (satu pill sejajar garis dari loop
    /// 2D, satu lagi pill datar dari loop rusuk 3D, di posisi dunia yang
    /// sama). `line_anchors_2d` merekam posisi dunia + panjang tiap Line 2D
    /// yang sudah dilabeli, supaya loop rusuk 3D di bawah bisa melewati
    /// rusuk yang cocok alih-alih melabeli ulang.
    fn render_all_element_dimensions(&self, ui: &mut egui::Ui, rect: egui::Rect) {
        let mut line_anchors_2d: Vec<(Vec3, f64)> = Vec::new();

        for (_, entity) in self.sketch().entities.iter() {
            match entity {
                Entity::Line { start, end } => {
                    let len = (*end - *start).length();
                    let mid = (*start + *end) * 0.5;
                    let label_3d = self.active_plane.to_world(mid, 0.0);
                    line_anchors_2d.push((label_3d, len));
                    if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, label_3d) {
                        let angle = self.screen_line_angle(rect, *start, *end);
                        CanvasHud::render_dimension_pill_aligned(ui, pos_2d, angle, &self.unit.format_precise(len));
                    }
                }
                Entity::Circle { center, radius } => {
                    let edge_pt = *center + DVec2::new(*radius, 0.0);
                    let label_3d = self.active_plane.to_world(edge_pt, 0.0);
                    if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, label_3d) {
                        let text = format!("R {}", self.unit.format_precise(*radius));
                        CanvasHud::render_dimension_pill(ui, pos_2d, &text, false);
                    }
                }
                Entity::Arc { center, radius, start_angle, end_angle } => {
                    let mid_angle = (start_angle + end_angle) * 0.5;
                    let mid_pt = *center + DVec2::new(radius * mid_angle.cos(), radius * mid_angle.sin());
                    let label_3d = self.active_plane.to_world(mid_pt, 0.0);
                    if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, label_3d) {
                        let text = format!("R {}", self.unit.format_precise(*radius));
                        CanvasHud::render_dimension_pill(ui, pos_2d, &text, false);
                    }
                }
                Entity::Ellipse { center, radius_x, radius_y } => {
                    let label_3d = self.active_plane.to_world(*center, 0.0);
                    if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, label_3d) {
                        let text = format!(
                            "Rx {} Ry {}",
                            self.unit.format_precise(*radius_x),
                            self.unit.format_precise(*radius_y)
                        );
                        CanvasHud::render_dimension_pill(ui, pos_2d, &text, false);
                    }
                }
            }
        }

        // Toleransi kombinasi jarak posisi (mm dunia) + panjang (mm) untuk
        // anggap satu rusuk 3D "sama" dengan satu Line 2D di atas — cukup
        // ketat (rusuk yang genuinely berbeda tapi kebetulan mirip ukuran
        // & lokasi itu sangat tidak mungkin) tapi longgar dari epsilon
        // float murni supaya tetap match walau lewat 2 jalur precision
        // berbeda (DVec2 sketsa f64 vs edge_dimensions kernel f64 -> Vec3
        // f32 saat proyeksi).
        const COINCIDENCE_POS_EPS: f32 = 1e-3;
        const COINCIDENCE_LEN_EPS: f64 = 1e-3;

        for (id, geo) in self.model.geometry.iter() {
            let visible = self.model.doc.bodies.get(id).is_some_and(|b| b.visible);
            if !visible {
                continue;
            }
            for (mid, start, end, length) in &geo.edge_dims {
                let world_pt = Vec3::new(mid.0 as f32, mid.1 as f32, mid.2 as f32);
                let already_shown_by_sketch = line_anchors_2d.iter().any(|(anchor, len)| {
                    (world_pt - *anchor).length() < COINCIDENCE_POS_EPS && (length - len).abs() < COINCIDENCE_LEN_EPS
                });
                if already_shown_by_sketch {
                    continue;
                }
                if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, world_pt) {
                    // Sejajar arah rusuk di layar (bukan selalu datar) —
                    // dihitung ulang dari proyeksi kamera SEKARANG, jadi
                    // ikut berputar bersama kamera, sama seperti pill
                    // dimensi sketsa 2D.
                    let start_pt = Vec3::new(start.0 as f32, start.1 as f32, start.2 as f32);
                    let end_pt = Vec3::new(end.0 as f32, end.1 as f32, end.2 as f32);
                    let angle = self.screen_angle_between_world_points(rect, start_pt, end_pt);
                    CanvasHud::render_dimension_pill_aligned(ui, pos_2d, angle, &self.unit.format_precise(*length));
                }
            }
        }
    }

    /// Kotak input mengambang dan badge dimensi in-situ (Screenshot 1, 2, 3, 4)
    fn dynamic_input_ui(&mut self, ui: &mut egui::Ui, rect: egui::Rect, raw_cursor: Option<DVec2>) {
        // 0. Nilai pengukuran yang SUDAH di-commit (bukan lagi sedang ditarik) —
        // pill ditaruh TEPAT DI ATAS garis kuningnya sendiri (bukan digeser ke
        // samping seperti pill tool Line/Rectangle/Circle), supaya nominalnya
        // kebaca langsung menempel di garisnya (bukan cuma di kartu "📏
        // Pengukuran" panel Properties kanan).
        if !self.measurements.is_empty() {
            for m in &self.measurements {
                let Some(value) = m.inline_value(self.unit) else { continue };
                let pts = m.points();
                let (Some(&a), Some(&b)) = (pts.first(), pts.last()) else { continue };
                let mid = (a + b) * 0.5;
                let label_3d = self.active_plane.to_world(mid, 0.0);
                if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, label_3d) {
                    let angle = self.screen_line_angle(rect, a, b);
                    CanvasHud::render_dimension_pill_aligned(ui, pos_2d, angle, &value);
                }
            }
        }

        // 0b. Checkbox "Tampilkan Semua Ukuran" (ruler properties, panel
        // Properties kanan) — label nominal SEMUA entitas sketsa 2D + SEMUA
        // rusuk 3D body visible, independen dari hasil tool "Ukur" di atas.
        if self.show_all_dimensions {
            self.render_all_element_dimensions(ui, rect);
        }

        // 1. Floating Dimension Pills saat sedang menggambar (Screenshot 1)
        if let Some(raw) = raw_cursor {
            let effective = self.snapped_or(raw);
            let world_scale = pixel_tolerance_to_world(&self.camera, rect);
            let offset_dist = (14.0 * world_scale).max(8.0);

            match self.tool {
                ToolKind::Line if self.pending_points.len() == 1 => {
                    let start = self.pending_points[0];
                    let len = (effective - start).length();
                    let mid = (start + effective) * 0.5;
                    let dir = (effective - start).normalize_or_zero();
                    let normal = DVec2::new(-dir.y, dir.x);
                    let label_pos = mid + normal * offset_dist;
                    let label_3d = self.active_plane.to_world(label_pos, 0.0);
                    if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, label_3d) {
                        CanvasHud::render_dimension_pill(ui, pos_2d, &self.unit.format_precise(len), false);
                    }
                }
                ToolKind::Rectangle if self.pending_points.len() == 1 => {
                    let first = self.pending_points[0];
                    let min = first.min(effective);
                    let max = first.max(effective);
                    let w = (max.x - min.x).abs();
                    let h = (max.y - min.y).abs();

                    // Pill lebar di sisi bawah
                    let bot_mid = DVec2::new((min.x + max.x) * 0.5, min.y - offset_dist);
                    let bot_3d = self.active_plane.to_world(bot_mid, 0.0);
                    if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, bot_3d) {
                        CanvasHud::render_dimension_pill(ui, pos_2d, &self.unit.format_precise(w), false);
                    }
                    // Pill tinggi di sisi kanan
                    let right_mid = DVec2::new(max.x + offset_dist, (min.y + max.y) * 0.5);
                    let right_3d = self.active_plane.to_world(right_mid, 0.0);
                    if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, right_3d) {
                        CanvasHud::render_dimension_pill(ui, pos_2d, &self.unit.format_precise(h), false);
                    }
                }
                ToolKind::Circle if self.pending_points.len() == 1 => {
                    let first = self.pending_points[0];
                    let radius = (effective - first).length();
                    let mid = (first + effective) * 0.5;
                    let mid_3d = self.active_plane.to_world(mid, 0.0);
                    if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, mid_3d) {
                        CanvasHud::render_dimension_pill(ui, pos_2d, &format!("R {}", self.unit.format_precise(radius)), false);
                    }
                }
                ToolKind::Measure if self.pending_points.len() == 1 => {
                    // Tool Ukur Jarak: pill ditaruh TEPAT DI ATAS garis kuning
                    // pengukurannya sendiri, DIPUTAR sejajar arah garisnya
                    // (`render_dimension_pill_aligned`) selagi ditarik, sebelum
                    // klik kedua meng-commit ke `self.measurements`.
                    let start = self.pending_points[0];
                    let len = (effective - start).length();
                    let mid = (start + effective) * 0.5;
                    let label_3d = self.active_plane.to_world(mid, 0.0);
                    if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, label_3d) {
                        let angle = self.screen_line_angle(rect, start, effective);
                        CanvasHud::render_dimension_pill_aligned(ui, pos_2d, angle, &self.unit.format_precise(len));
                    }
                }
                _ => {}
            }
        }

        // 2. Interactive Draggable Double Arrow Handle & Dimension Pill di atas Gizmo (Screenshot 2, 3, 4)
        if let Some(centroid) = self.selected_closed_region_centroid() {
            let z_pos = if self.extruding_from_gizmo { self.gizmo_distance } else { 18.0 };
            let handle_3d = self.active_plane.to_world(centroid, z_pos as f32);

            if let Some(handle_2d) = world_to_screen_pos(&self.camera, rect, handle_3d) {
                let (_, arrow_vec_opt) = self.project_screen_drag_to_extrude_axis(rect, centroid, egui::Vec2::ZERO);

                // Handle panah 2 sisi tebal dan draggable (rotasi otomatis sesuai sudut proyeksi 3D)
                let handle_resp = CanvasHud::render_draggable_double_arrow_handle(
                    ui,
                    handle_2d,
                    self.extruding_from_gizmo,
                    arrow_vec_opt,
                );

                if handle_resp.drag_started() {
                    self.extruding_from_gizmo = true;
                    if self.gizmo_distance == 0.0 {
                        self.gizmo_distance = 20.0;
                    }
                }

                if handle_resp.dragged() {
                    self.extruding_from_gizmo = true;
                    let (delta_mm, _) = self.project_screen_drag_to_extrude_axis(rect, centroid, handle_resp.drag_delta());
                    self.gizmo_distance += delta_mm;
                    self.update_gizmo_boolean_detection();
                }

                if handle_resp.drag_stopped() {
                    self.commit_gizmo_extrusion();
                }

                // Interactive Dimension Pill diletakkan di atas handle panah
                let pill_pos = handle_2d + egui::vec2(0.0, -32.0);
                let text = self.unit.format(self.gizmo_distance.abs());
                let pill_resp = CanvasHud::render_interactive_dimension_pill(ui, pill_pos, &text, self.gizmo_dimension_editing);
                if pill_resp.clicked() {
                    self.gizmo_dimension_editing = !self.gizmo_dimension_editing;
                    self.gizmo_edit_input = format!("{:.0}", self.unit.to_display_val(self.gizmo_distance));
                }

                if self.gizmo_dimension_editing {
                    let popup_rect = egui::Rect::from_center_size(pill_pos + egui::vec2(0.0, 28.0), egui::vec2(100.0, 32.0));
                    egui::Area::new(egui::Id::new("cadraw-gizmo-edit-popup"))
                        .fixed_pos(popup_rect.min)
                        .order(egui::Order::Foreground)
                        .show(ui.ctx(), |ui| {
                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                let resp = ui.text_edit_singleline(&mut self.gizmo_edit_input);
                                resp.request_focus();
                                if resp.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    if let Ok(val) = self.gizmo_edit_input.trim().parse::<f64>() {
                                        self.gizmo_distance = self.unit.to_internal_mm(val);
                                        // Commit Extrude dengan ukuran presisi
                                        self.commit_gizmo_extrusion();
                                    }
                                    self.gizmo_dimension_editing = false;
                                }
                            });
                        });
                }
            }
        }

        // 3. Interactive Draggable Double Arrow Handle & Dimension Pill untuk 3D Face Extrude Gizmo
        if let Some((_, _, hit)) = &self.active_face {
            let anchor = hit.gizmo_anchor();
            let c_base = Vec3::new(anchor.0 as f32, anchor.1 as f32, anchor.2 as f32);
            // Fase 4: handle & proyeksi drag pill pakai `pull_dir`.
            let pull_dir = Vec3::new(hit.pull_dir.0 as f32, hit.pull_dir.1 as f32, hit.pull_dir.2 as f32);
            // `surface_kind` disalin lepas dari `hit` (bukan `&hit.surface_kind`
            // di bawah) supaya pinjaman `&self.active_face` tidak "tembus"
            // sampai teks pill dibentuk — di antaranya ada pemanggilan
            // `&mut self` (mis. `self.extrude_active_face(..)`), yang bakal
            // ditolak borrow checker kalau `hit` masih dipinjam sampai situ.
            let surface_kind = hit.surface_kind;
            let z_pos = if self.extruding_face_from_gizmo { self.face_gizmo_distance as f32 } else { 18.0 };
            let handle_3d = c_base + pull_dir * z_pos;

            if let Some(handle_2d) = world_to_screen_pos(&self.camera, rect, handle_3d) {
                let (_, arrow_vec_opt) = self.project_screen_drag_to_world_axis(rect, c_base, pull_dir, egui::Vec2::ZERO);

                // Handle panah 2 sisi tebal dan draggable (rotasi otomatis sesuai sudut pull_dir 3D)
                let handle_resp = CanvasHud::render_draggable_double_arrow_handle(
                    ui,
                    handle_2d,
                    self.extruding_face_from_gizmo,
                    arrow_vec_opt,
                );

                if handle_resp.drag_started() {
                    self.extruding_face_from_gizmo = true;
                    if self.face_gizmo_distance == 0.0 {
                        self.face_gizmo_distance = 15.0;
                    }
                }

                if handle_resp.dragged() {
                    self.extruding_face_from_gizmo = true;
                    let (delta_mm, _) = self.project_screen_drag_to_world_axis(rect, c_base, pull_dir, handle_resp.drag_delta());
                    self.face_gizmo_distance += delta_mm;
                    self.face_gizmo_edit_input = format!("{:.0}", self.unit.to_display_val(self.face_gizmo_distance));
                }

                if handle_resp.drag_stopped() {
                    if self.face_gizmo_distance.abs() > 0.1 {
                        self.extrude_active_face(self.face_gizmo_distance);
                    }
                    self.extruding_face_from_gizmo = false;
                    self.face_gizmo_distance = 15.0;
                    self.face_gizmo_edit_input = "15".to_string();
                }

                // Interactive Dimension Pill diletakkan di atas handle panah.
                // Fase 4: permukaan radial (Cylinder/Cone/Sphere) tampilkan
                // "ΔR ±<jarak>" (delta radius) alih-alih jarak polos —
                // drag di sini mengubah RADIUS, bukan menggeser bidang.
                let pill_pos = handle_2d + egui::vec2(0.0, -32.0);
                let text = self.format_face_gizmo_dimension_text(surface_kind, self.face_gizmo_distance);
                let pill_resp = CanvasHud::render_interactive_dimension_pill(ui, pill_pos, &text, self.face_gizmo_dimension_editing);
                if pill_resp.clicked() {
                    self.face_gizmo_dimension_editing = !self.face_gizmo_dimension_editing;
                    self.face_gizmo_edit_input = format!("{:.0}", self.unit.to_display_val(self.face_gizmo_distance));
                }

                if self.face_gizmo_dimension_editing {
                    let popup_rect = egui::Rect::from_center_size(pill_pos + egui::vec2(0.0, 28.0), egui::vec2(100.0, 32.0));
                    egui::Area::new(egui::Id::new("cadraw-face-gizmo-edit-popup"))
                        .fixed_pos(popup_rect.min)
                        .order(egui::Order::Foreground)
                        .show(ui.ctx(), |ui| {
                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                let resp = ui.text_edit_singleline(&mut self.face_gizmo_edit_input);
                                resp.request_focus();
                                if resp.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    if let Ok(val) = self.face_gizmo_edit_input.trim().parse::<f64>() {
                                        let dist = self.unit.to_internal_mm(val);
                                        self.face_gizmo_distance = dist;
                                        self.extrude_active_face(dist);
                                    }
                                    self.face_gizmo_dimension_editing = false;
                                }
                            });
                        });
                }
            }
        }

        // 4. Interactive Draggable Double Arrow Handle & Dimension Pill
        // untuk Gizmo Vertex Fillet 3D (Fase 3 — Rounded Sudut). Cermin
        // blok gizmo face di atas: `vertex`/`out_dir` diambil sbg nilai
        // `Vec3` MILIK SENDIRI (bukan pinjaman `&self.active_vertex`)
        // lewat `active_vertex_gizmo_dir()` SEBELUM panggilan `&mut self`
        // (mis. `self.commit_vertex_fillet()`) di bawah — pola sama dgn
        // kenapa `hit.gizmo_anchor()`/`surface_kind` disalin lepas di blok
        // face, supaya borrow checker tidak menolak.
        if let Some((c_base, pull_dir)) = self.active_vertex_gizmo_dir() {
            let z_pos = if self.filleting_vertex_from_gizmo {
                self.vertex_gizmo_radius.max(0.1) as f32
            } else {
                12.0
            };
            let handle_3d = c_base + pull_dir * z_pos;

            if let Some(handle_2d) = world_to_screen_pos(&self.camera, rect, handle_3d) {
                let (_, arrow_vec_opt) = self.project_screen_drag_to_world_axis(rect, c_base, pull_dir, egui::Vec2::ZERO);

                // Handle panah 2 sisi tebal dan draggable (styling handle itu
                // sendiri sama dgn gizmo lain; pembeda warna vertex-fillet
                // vs face ada di overlay garis `VERTEX_GIZMO_COLOR`, lihat
                // `build_overlay_lines`).
                let handle_resp = CanvasHud::render_draggable_double_arrow_handle(
                    ui,
                    handle_2d,
                    self.filleting_vertex_from_gizmo,
                    arrow_vec_opt,
                );

                if handle_resp.drag_started() {
                    self.filleting_vertex_from_gizmo = true;
                    if self.vertex_gizmo_radius <= 0.0 {
                        self.vertex_gizmo_radius = 3.0;
                    }
                }

                if handle_resp.dragged() {
                    self.filleting_vertex_from_gizmo = true;
                    let (delta_mm, _) = self.project_screen_drag_to_world_axis(rect, c_base, pull_dir, handle_resp.drag_delta());
                    // Boleh sampai 0 (bukan clamp 0.1): dorong ke dalam =
                    // kecilkan radius sampai siku, commit menerjemahkan
                    // radius < ROUND_SHARP_MM jadi hapus/skip fitur.
                    let candidate_radius = (self.vertex_gizmo_radius + delta_mm).max(0.0);
                    // Terima kandidat radius baru HANYA kalau memang bisa
                    // dibangun — dicoba LANGSUNG lewat `round_gizmo_preview_
                    // shape` (yang ujungnya manggil `fillet_vertex`/OCCT
                    // sungguhan), BUKAN formula perkiraan geometris. Dua
                    // formula precheck sebelumnya (`radius <= tepi/2`, lalu
                    // `radius <= tepi`) TERBUKTI dua-duanya salah — kelewat
                    // konservatif dgn cara berbeda (dilaporkan user lewat
                    // screenshot 2x berturut-turut), dan formula MANAPUN
                    // rawan tidak cocok dgn batas SEBENARNYA yg dihitung
                    // OCCT sendiri (blend 3 arah di 1 vertex bukan geometri
                    // sederhana). Kalau kandidat gagal, radius TETAP di
                    // nilai valid TERAKHIR — gizmo "berhenti" TEPAT di
                    // radius maksimum yg BENAR-BENAR bisa dibangun (bukan
                    // radius perkiraan), pill & visual SELALU sinkron
                    // (`round_gizmo_preview_shape` dipanggil ULANG dgn
                    // radius yg SAMA ini di render pass, dijamin sukses).
                    if candidate_radius < Self::ROUND_SHARP_MM
                        || self.round_gizmo_preview_shape(RoundKind::Vertex, candidate_radius).is_some()
                    {
                        self.vertex_gizmo_radius = candidate_radius;
                    }
                    self.vertex_gizmo_edit_input = format!("{:.1}", self.unit.to_display_val(self.vertex_gizmo_radius));
                }

                if handle_resp.drag_stopped() {
                    // Reset nilai gizmo TIDAK dilakukan di sini — commit
                    // sukses me-reset lewat `clear_round_gizmo`; commit
                    // gagal membiarkan nilai supaya bisa dikoreksi user.
                    self.commit_vertex_fillet();
                    self.filleting_vertex_from_gizmo = false;
                }

                // Interactive Dimension Pill "R <nilai><unit>" di atas handle panah.
                let pill_pos = handle_2d + egui::vec2(0.0, -32.0);
                let text = if self.vertex_gizmo_radius < Self::ROUND_SHARP_MM {
                    "R 0 (siku)".to_string()
                } else {
                    format!("R {}", self.unit.format(self.vertex_gizmo_radius))
                };
                let pill_resp = CanvasHud::render_interactive_dimension_pill(ui, pill_pos, &text, self.vertex_gizmo_dimension_editing);
                if pill_resp.clicked() {
                    self.vertex_gizmo_dimension_editing = !self.vertex_gizmo_dimension_editing;
                    self.vertex_gizmo_edit_input = format!("{:.1}", self.unit.to_display_val(self.vertex_gizmo_radius));
                }

                if self.vertex_gizmo_dimension_editing {
                    let popup_rect = egui::Rect::from_center_size(pill_pos + egui::vec2(0.0, 28.0), egui::vec2(100.0, 32.0));
                    egui::Area::new(egui::Id::new("cadraw-vertex-gizmo-edit-popup"))
                        .fixed_pos(popup_rect.min)
                        .order(egui::Order::Foreground)
                        .show(ui.ctx(), |ui| {
                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                let resp = ui.text_edit_singleline(&mut self.vertex_gizmo_edit_input);
                                resp.request_focus();
                                if resp.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    if let Ok(val) = self.vertex_gizmo_edit_input.trim().parse::<f64>() {
                                        // 0 valid: berarti hapus rounding (siku).
                                        self.vertex_gizmo_radius = self.unit.to_internal_mm(val).max(0.0);
                                        self.commit_vertex_fillet();
                                    }
                                    self.vertex_gizmo_dimension_editing = false;
                                }
                            });
                        });
                }
            }
        }

        // 5. Interactive Draggable Double Arrow Handle & Dimension Pill
        // untuk Gizmo Edge Fillet 3D ("klik rusuk pojok kubus" -> rusuk
        // membulat). Cermin PERSIS blok gizmo vertex fillet di atas — ikon,
        // pill "R", popup input — cuma jangkarnya titik klik pada rusuk
        // (`active_edge_gizmo_dir`) dan commit-nya `commit_edge_fillet_single`
        // (`fillet_edges` 1 ray) alih-alih `fillet_vertex`.
        if let Some((c_base, pull_dir)) = self.active_edge_gizmo_dir() {
            let z_pos = if self.filleting_edge_from_gizmo {
                self.edge_gizmo_radius.max(0.1) as f32
            } else {
                12.0
            };
            let handle_3d = c_base + pull_dir * z_pos;

            if let Some(handle_2d) = world_to_screen_pos(&self.camera, rect, handle_3d) {
                let (_, arrow_vec_opt) = self.project_screen_drag_to_world_axis(rect, c_base, pull_dir, egui::Vec2::ZERO);

                let handle_resp = CanvasHud::render_draggable_double_arrow_handle(
                    ui,
                    handle_2d,
                    self.filleting_edge_from_gizmo,
                    arrow_vec_opt,
                );

                if handle_resp.drag_started() {
                    self.filleting_edge_from_gizmo = true;
                    if self.edge_gizmo_radius <= 0.0 {
                        self.edge_gizmo_radius = 3.0;
                    }
                }

                if handle_resp.dragged() {
                    self.filleting_edge_from_gizmo = true;
                    let (delta_mm, _) = self.project_screen_drag_to_world_axis(rect, c_base, pull_dir, handle_resp.drag_delta());
                    // Boleh sampai 0 — lihat komentar di gizmo vertex.
                    let candidate_radius = (self.edge_gizmo_radius + delta_mm).max(0.0);
                    // Validasi lewat trial langsung — lihat komentar
                    // panjang di gizmo vertex (drag handler di atas).
                    if candidate_radius < Self::ROUND_SHARP_MM
                        || self.round_gizmo_preview_shape(RoundKind::Edge, candidate_radius).is_some()
                    {
                        self.edge_gizmo_radius = candidate_radius;
                    }
                    self.edge_gizmo_edit_input = format!("{:.1}", self.unit.to_display_val(self.edge_gizmo_radius));
                }

                if handle_resp.drag_stopped() {
                    // Reset TIDAK di sini — lihat komentar di gizmo vertex.
                    self.commit_edge_fillet_single();
                    self.filleting_edge_from_gizmo = false;
                }

                // Interactive Dimension Pill "R <nilai><unit>" di atas handle panah.
                let pill_pos = handle_2d + egui::vec2(0.0, -32.0);
                let text = if self.edge_gizmo_radius < Self::ROUND_SHARP_MM {
                    "R 0 (siku)".to_string()
                } else {
                    format!("R {}", self.unit.format(self.edge_gizmo_radius))
                };
                let pill_resp = CanvasHud::render_interactive_dimension_pill(ui, pill_pos, &text, self.edge_gizmo_dimension_editing);
                if pill_resp.clicked() {
                    self.edge_gizmo_dimension_editing = !self.edge_gizmo_dimension_editing;
                    self.edge_gizmo_edit_input = format!("{:.1}", self.unit.to_display_val(self.edge_gizmo_radius));
                }

                if self.edge_gizmo_dimension_editing {
                    let popup_rect = egui::Rect::from_center_size(pill_pos + egui::vec2(0.0, 28.0), egui::vec2(100.0, 32.0));
                    egui::Area::new(egui::Id::new("cadraw-edge-gizmo-edit-popup"))
                        .fixed_pos(popup_rect.min)
                        .order(egui::Order::Foreground)
                        .show(ui.ctx(), |ui| {
                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                let resp = ui.text_edit_singleline(&mut self.edge_gizmo_edit_input);
                                resp.request_focus();
                                if resp.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    if let Ok(val) = self.edge_gizmo_edit_input.trim().parse::<f64>() {
                                        // 0 valid: berarti hapus rounding (siku).
                                        self.edge_gizmo_radius = self.unit.to_internal_mm(val).max(0.0);
                                        self.commit_edge_fillet_single();
                                    }
                                    self.edge_gizmo_dimension_editing = false;
                                }
                            });
                        });
                }
            }
        }
    }

    /// Coba terapkan `constraint`: dry-run solve di atas clone sketch dulu
    /// (termasuk constraint yang sudah ada + yang baru), baru dikirim ke
    /// undo stack kalau konvergen. Sketch nyata tidak tersentuh sama
    /// sekali kalau gagal — hanya `constraint_status` terisi pesan error.
    fn apply_constraint(&mut self, new_constraint: Constraint) {
        let mut trial = self.sketch().clone();
        trial.constraints.push(new_constraint.clone());
        let snapshot = trial.constraints.clone();
        let result = constraint::solve(&mut trial, &snapshot);

        if result.converged {
            self.execute_sketch_command(Box::new(AddConstraint::new(new_constraint)));
            self.constraint_status = None;
        } else {
            self.constraint_status = Some(format!(
                "Constraint gagal diselesaikan (sisa residual {:.4}) — dibatalkan, sketch tidak berubah",
                result.final_residual_norm
            ));
        }
    }


    // ---- Fase 3: Model 3D ------------------------------------------------

    /// Extrude profil dari seleksi entitas sketch saat ini. Dry-run: kalau
    /// `build_profile_from_selection` atau `extrude_profile` gagal, cuma
    /// `model_status` terisi — `model` tidak tersentuh.
    fn extrude_selected(&mut self) {
        let distance: f64 = match self.extrude_distance_input.trim().parse() {
            Ok(v) => v,
            Err(_) => {
                self.model_status = Some("Jarak extrude tidak valid".to_string());
                return;
            }
        };
        let profile = match model::build_profile_from_selection(self.sketch(), &self.selected) {
            Ok(p) => p,
            Err(msg) => {
                self.model_status = Some(msg);
                return;
            }
        };
        match self.extrude_profile_active_plane(&profile, distance) {
            Ok(shape) => {
                let geo = BodyGeometry::from_shape(shape);
                self.model_undo.execute(
                    Box::new(AddSolidCommand::new("Extrude", geo)),
                    &mut self.model,
                );
                self.model_status = None;
            }
            Err(e) => self.model_status = Some(format!("Extrude gagal: {e}")),
        }
    }

    /// Loft antara `pending_loft_bottom` (di-stage sebelumnya) dan profil
    /// dari seleksi sketch SAAT INI (profil atas) — lihat catatan di
    /// `model_panel` untuk kenapa ini bukan loft lintas-workplane
    /// sungguhan.
    fn loft_selected(&mut self) {
        let Some(bottom) = self.pending_loft_bottom.clone() else {
            self.model_status = Some("Set Profil Bawah dari Seleksi dulu sebelum Loft".to_string());
            return;
        };
        let height: f64 = match self.loft_height_input.trim().parse() {
            Ok(v) => v,
            Err(_) => {
                self.model_status = Some("Tinggi loft tidak valid".to_string());
                return;
            }
        };
        let top = match model::build_profile_from_selection(self.sketch(), &self.selected) {
            Ok(p) => p,
            Err(msg) => {
                self.model_status = Some(format!("Profil atas: {msg}"));
                return;
            }
        };
        match cadraw_kernel::loft_profiles(&bottom, &top, height) {
            Ok(shape) => {
                let geo = BodyGeometry::from_shape(shape);
                self.model_undo.execute(Box::new(AddSolidCommand::new("Loft", geo)), &mut self.model);
                self.pending_loft_bottom = None;
                self.model_status = None;
            }
            Err(e) => self.model_status = Some(format!("Loft gagal: {e}")),
        }
    }

    /// Union/Subtract/Intersect dua body terpilih (butuh persis 2). Seleksi
    /// body dikosongkan setelah sukses — keduanya lenyap, digantikan 1
    /// body hasil dengan `BodyId` baru (lihat catatan `model::BooleanCommand`).
    fn boolean_selected(&mut self, kind: BooleanKind, label: &'static str, result_name: &str) {
        let ids: Vec<BodyId> = self.selected_bodies.iter().copied().collect();
        let [a, b] = ids.as_slice() else {
            self.model_status = Some("Pilih persis 2 body di daftar untuk operasi ini".to_string());
            return;
        };
        let (a_id, b_id) = (*a, *b);
        match BooleanCommand::try_new(&self.model, kind, label, result_name, a_id, b_id) {
            Ok(cmd) => {
                self.model_undo.execute(Box::new(cmd), &mut self.model);
                self.selected_bodies.clear();
                // Kedua body sumber lenyap — riwayat rounding-nya ikut.
                self.round_history.remove(&a_id);
                self.round_history.remove(&b_id);
                self.model_status = None;
            }
            Err(msg) => self.model_status = Some(msg),
        }
    }

    /// Fillet SEMUA tepi 1 body terpilih — atau, kalau `selected_edges`
    /// tidak kosong (mode "Pilih Tepi Manual" dipakai), HANYA tepi yang
    /// di-pick lewat `cadraw_kernel::fillet_edges` (Fase 8). Toleransi
    /// pick dihitung ULANG dari kamera SEKARANG (bukan disimpan dari
    /// waktu klik) — konsisten dengan cara `tol` dihitung tiap frame di
    /// `handle_sketch_input`.
    fn fillet_selected_body(&mut self) {
        let Some(&id) = self.selected_bodies.iter().next().filter(|_| self.selected_bodies.len() == 1) else {
            self.model_status = Some("Pilih persis 1 body untuk Fillet".to_string());
            return;
        };
        let Ok(radius) = self.fillet_radius_input.trim().parse::<f64>() else {
            self.model_status = Some("Radius fillet tidak valid".to_string());
            return;
        };
        let Some(geo) = self.model.geometry.get(id) else {
            return;
        };
        let rays: Vec<PickRay> = self.selected_edges.iter().map(|e| e.ray).collect();
        let result = if rays.is_empty() {
            cadraw_kernel::fillet_all(&geo.shape, radius)
        } else {
            cadraw_kernel::fillet_edges(&geo.shape, radius, &rays, Self::EDGE_REAPPLY_TOLERANCE_MM)
        };
        match result {
            Ok(shape) => {
                let new_geo = BodyGeometry::from_shape(shape);
                self.model_undo.execute(
                    Box::new(ReplaceGeometryCommand::new("Fillet", id, new_geo)),
                    &mut self.model,
                );
                self.round_history.remove(&id);
                self.selected_edges.clear();
                self.picking_mode = PickMode::None;
                self.model_status = None;
            }
            Err(e) => self.model_status = Some(format!("Fillet gagal: {e}")),
        }
    }

    /// Fillet sudut (vertex) 3D yang sedang aktif lewat gizmo (Fase 3),
    /// radius `self.vertex_gizmo_radius` (di-clamp minimal 0.1 mm supaya
    /// tidak pernah dikirim 0/negatif ke `cadraw_kernel::fillet_vertex`,
    /// yang menolaknya). Jalur undo SAMA dengan `fillet_selected_body`
    /// (`ReplaceGeometryCommand`) — dipanggil dari drag-stop handle gizmo
    /// dan dari Enter/lost-focus popup input pill di `dynamic_input_ui`.
    /// Gagal (mis. radius kebesaran, OCCT menolak) → `model_status` terisi
    /// pesan error, shape body ASLI tidak tersentuh (`fillet_vertex`
    /// bekerja di atas clone, lihat dokumentasinya di cadraw-kernel).
    fn commit_vertex_fillet(&mut self) {
        self.commit_round(RoundKind::Vertex);
    }

    /// Fillet 1 rusuk (edge) 3D yang sedang aktif lewat gizmo rounding
    /// ("klik rusuk pojok kubus" — cermin `commit_vertex_fillet`). Keduanya
    /// sekarang delegasi ke `commit_round` (rounding parametrik).
    fn commit_edge_fillet_single(&mut self) {
        self.commit_round(RoundKind::Edge);
    }

    /// Batas radius di bawah mana rounding dianggap "siku": commit dengan
    /// radius < ini MENGHAPUS fitur (saat mengedit) atau tidak membuat
    /// fitur sama sekali (saat membuat baru) — dorongan handle gizmo ke
    /// dalam sampai (mendekati) 0 berarti kembali ke sudut tajam.
    const ROUND_SHARP_MM: f64 = 0.2;

    /// Rebuild shape dari `base` + daftar fitur rounding, diterapkan
    /// berurutan (tiap ray di-resolve ulang terhadap shape hasil langkah
    /// sebelumnya — fillet sudut A tidak menghilangkan rusuk/vertex sudut
    /// B, jadi resolusi ray-based tetap menemukan targetnya).
    fn build_rounded_shape(base: &KernelShape, features: &[RoundFeature]) -> Result<KernelShape, String> {
        let mut shape = cadraw_kernel::clone_shape(base).map_err(|e| e.to_string())?;
        for f in features {
            shape = match f.kind {
                RoundKind::Vertex => {
                    cadraw_kernel::fillet_vertex(&shape, f.radius, f.ray, Self::EDGE_REAPPLY_TOLERANCE_MM)
                }
                RoundKind::Edge => {
                    cadraw_kernel::fillet_edges(&shape, f.radius, &[f.ray], Self::EDGE_REAPPLY_TOLERANCE_MM)
                }
            }
            .map_err(|e| e.to_string())?;
        }
        Ok(shape)
    }

    /// Reset state gizmo rounding (dipanggil setelah commit sukses / batal
    /// eksplisit) — TIDAK dipanggil saat commit gagal, supaya user masih
    /// bisa mengoreksi radius tanpa memilih ulang sudut/rusuknya.
    fn clear_round_gizmo(&mut self, kind: RoundKind) {
        self.editing_round = None;
        match kind {
            RoundKind::Vertex => {
                self.active_vertex = None;
                self.vertex_gizmo_radius = 3.0;
                self.vertex_gizmo_edit_input = "3".to_string();
            }
            RoundKind::Edge => {
                self.active_edge = None;
                self.edge_gizmo_radius = 3.0;
                self.edge_gizmo_edit_input = "3".to_string();
            }
        }
    }

    /// Commit rounding PARAMETRIK: alih-alih memfillet shape yang sudah
    /// difillet (destruktif — radius tidak bisa dikecilkan lagi, dan klik
    /// berikutnya di sudut bulat jatuh ke gizmo extrude yang membesarkan
    /// objek), riwayat per body menyimpan shape DASAR + daftar fitur; tiap
    /// commit (baru maupun edit lewat intersepsi `find_round_feature_near`)
    /// menyusun daftar fitur baru lalu rebuild dari dasar. Radius <
    /// `ROUND_SHARP_MM` = fitur dihapus (sudut kembali menyiku). Gagal
    /// (mis. radius kebesaran, OCCT menolak) → riwayat & geometry TIDAK
    /// tersentuh, cuma `model_status` terisi.
    fn commit_round(&mut self, kind: RoundKind) {
        let (body_id, ray, anchor, radius) = match kind {
            RoundKind::Vertex => {
                let Some((b, r, a)) = self.active_vertex else { return };
                (b, r, a, self.vertex_gizmo_radius)
            }
            RoundKind::Edge => {
                let Some((b, r, a)) = self.active_edge else { return };
                (b, r, a, self.edge_gizmo_radius)
            }
        };
        let sharp = radius < Self::ROUND_SHARP_MM;
        let Some(geo) = self.model.geometry.get(body_id) else {
            self.model_status = Some("Body terpilih tidak ditemukan".to_string());
            return;
        };

        // Susun daftar fitur BARU dulu di salinan — riwayat asli baru
        // disentuh kalau rebuild-nya sukses (pola dry-run yang sama dengan
        // `apply_constraint`).
        let mut features: Vec<RoundFeature> = self
            .round_history
            .get(&body_id)
            .map(|h| h.features.clone())
            .unwrap_or_default();
        match self.editing_round {
            Some((b, idx)) if b == body_id && idx < features.len() => {
                if sharp {
                    features.remove(idx);
                } else {
                    features[idx].radius = radius;
                }
            }
            _ => {
                if sharp {
                    self.model_status = Some("Radius 0 — sudut dibiarkan menyiku".to_string());
                    self.clear_round_gizmo(kind);
                    return;
                }
                let polyline = if kind == RoundKind::Edge {
                    cadraw_kernel::pick_edge(&geo.shape, ray, Self::EDGE_REAPPLY_TOLERANCE_MM)
                        .map(|(_, pl)| pl)
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                features.push(RoundFeature { kind, ray, anchor, radius, polyline });
            }
        }

        // Base rebuild: dari riwayat kalau sudah ada; kalau ini rounding
        // PERTAMA body tsb, clone shape saat ini (sekaligus kandidat base
        // riwayat baru — di-clone SEBELUM geometry diganti).
        let (build, new_base) = if let Some(h) = self.round_history.get(&body_id) {
            (Self::build_rounded_shape(&h.base, &features), None)
        } else {
            match cadraw_kernel::clone_shape(&geo.shape) {
                Ok(base) => (Self::build_rounded_shape(&base, &features), Some(base)),
                Err(e) => {
                    self.model_status = Some(format!("Gagal menyimpan shape dasar rounding: {e}"));
                    return;
                }
            }
        };

        match build {
            Ok(shape) => {
                if let Some(base) = new_base {
                    self.round_history.insert(body_id, RoundHistory { base, features: Vec::new() });
                }
                if features.is_empty() {
                    self.round_history.remove(&body_id);
                } else if let Some(h) = self.round_history.get_mut(&body_id) {
                    h.features = features;
                }
                let new_geo = BodyGeometry::from_shape(shape);
                self.model_undo.execute(
                    Box::new(ReplaceGeometryCommand::new("Rounding", body_id, new_geo)),
                    &mut self.model,
                );
                self.model_status = Some(if sharp {
                    "Rounding dihapus — sudut kembali menyiku".to_string()
                } else {
                    format!("Rounding {:.1} mm sukses — klik sudutnya lagi utk mengubah/menghapus", radius)
                });
                self.clear_round_gizmo(kind);
            }
            Err(e) => self.model_status = Some(format!("Rounding gagal: {e}")),
        }
    }

    /// Preview shape gizmo rounding (vertex ATAU edge) di `radius` — cermin
    /// persis logika `commit_round` (fitur baru/edit disusun dari
    /// `round_history` + `radius`, lalu di-rebuild dari base), TAPI dry-run
    /// murni: tidak menyentuh `round_history`/`model_undo`/`model_status`.
    /// Dipanggil dua tempat: `build_combined_body_mesh` tiap frame (dgn
    /// radius TERKINI, `self.vertex_gizmo_radius`/`edge_gizmo_radius`) —
    /// supaya gizmo rounding di pojokan ikut real-time seperti gizmo
    /// extrude sketch & extrude face — DAN drag handler gizmo (dgn radius
    /// KANDIDAT, sebelum diterima) supaya nilai radius TIDAK PERNAH maju
    /// ke angka yang tidak bisa dibangun (lihat komentar drag handler:
    /// riwayat 2 formula precheck yang salah-salah terus di kernel
    /// membuktikan "coba langsung ke OCCT" lebih dapat diandalkan daripada
    /// formula perkiraan). `radius` diambil sbg PARAMETER (bukan dibaca
    /// langsung dari `self.*_gizmo_radius`) justru supaya kedua pemanggil
    /// itu bisa memakai fungsi yg SAMA dgn radius yg BEDA.
    ///
    /// `None` kalau gizmo tidak aktif, `radius` masih "siku" (<
    /// `ROUND_SHARP_MM`, preview-nya = body apa adanya, tidak perlu
    /// di-rebuild), atau rebuild gagal (radius kebesaran dsb).
    fn round_gizmo_preview_shape(&self, kind: RoundKind, radius: f64) -> Option<(BodyId, KernelShape)> {
        let (body_id, ray, anchor) = match kind {
            RoundKind::Vertex => self.active_vertex?,
            RoundKind::Edge => self.active_edge?,
        };
        if radius < Self::ROUND_SHARP_MM {
            return None;
        }
        let geo = self.model.geometry.get(body_id)?;

        let mut features: Vec<RoundFeature> = self
            .round_history
            .get(&body_id)
            .map(|h| h.features.clone())
            .unwrap_or_default();
        match self.editing_round {
            Some((b, idx)) if b == body_id && idx < features.len() => {
                features[idx].radius = radius;
            }
            _ => {
                let polyline = if kind == RoundKind::Edge {
                    cadraw_kernel::pick_edge(&geo.shape, ray, Self::EDGE_REAPPLY_TOLERANCE_MM)
                        .map(|(_, pl)| pl)
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                features.push(RoundFeature { kind, ray, anchor, radius, polyline });
            }
        }

        let base_owned;
        let base: &KernelShape = match self.round_history.get(&body_id) {
            Some(h) => &h.base,
            None => {
                base_owned = cadraw_kernel::clone_shape(&geo.shape).ok()?;
                &base_owned
            }
        };

        Self::build_rounded_shape(base, &features).ok().map(|shape| (body_id, shape))
    }

    /// Cari fitur rounding milik `body_id` yang dekat `hit_point` (titik
    /// klik pada permukaan body): dalam `radius·1.5 + toleransi layar` dari
    /// anchor fitur, atau — utk fitur rusuk — dari polyline rusuk aslinya
    /// (supaya klik DI SEPANJANG rusuk bulat juga kena, bukan cuma di titik
    /// klik semula). Dipakai intersepsi klik "edit rounding".
    fn find_round_feature_near(&self, body_id: BodyId, hit_point: (f64, f64, f64), rect: egui::Rect) -> Option<usize> {
        let hist = self.round_history.get(&body_id)?;
        let tol = pixel_tolerance_to_world(&self.camera, rect) * 14.0;
        let hp = glam::DVec3::new(hit_point.0, hit_point.1, hit_point.2);
        let mut best: Option<(usize, f64)> = None;
        for (idx, f) in hist.features.iter().enumerate() {
            let mut d = (hp - glam::DVec3::new(f.anchor.0, f.anchor.1, f.anchor.2)).length();
            for pair in f.polyline.windows(2) {
                let a = glam::DVec3::new(pair[0].0, pair[0].1, pair[0].2);
                let b = glam::DVec3::new(pair[1].0, pair[1].1, pair[1].2);
                let ab = b - a;
                let t = if ab.length_squared() > 1e-12 {
                    ((hp - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                d = d.min((a + ab * t - hp).length());
            }
            let reach = f.radius * 1.5 + tol;
            if d <= reach && best.as_ref().is_none_or(|(_, bd)| d < *bd) {
                best = Some((idx, d));
            }
        }
        best.map(|(idx, _)| idx)
    }

    /// Chamfer — lihat `fillet_selected_body`, pola identik.
    fn chamfer_selected_body(&mut self) {
        let Some(&id) = self.selected_bodies.iter().next().filter(|_| self.selected_bodies.len() == 1) else {
            self.model_status = Some("Pilih persis 1 body untuk Chamfer".to_string());
            return;
        };
        let Ok(distance) = self.chamfer_distance_input.trim().parse::<f64>() else {
            self.model_status = Some("Jarak chamfer tidak valid".to_string());
            return;
        };
        let Some(geo) = self.model.geometry.get(id) else {
            return;
        };
        let rays: Vec<PickRay> = self.selected_edges.iter().map(|e| e.ray).collect();
        let result = if rays.is_empty() {
            cadraw_kernel::chamfer_all(&geo.shape, distance)
        } else {
            cadraw_kernel::chamfer_edges(&geo.shape, distance, &rays, Self::EDGE_REAPPLY_TOLERANCE_MM)
        };
        match result {
            Ok(shape) => {
                let new_geo = BodyGeometry::from_shape(shape);
                self.model_undo.execute(
                    Box::new(ReplaceGeometryCommand::new("Chamfer", id, new_geo)),
                    &mut self.model,
                );
                self.round_history.remove(&id);
                self.selected_edges.clear();
                self.picking_mode = PickMode::None;
                self.model_status = None;
            }
            Err(e) => self.model_status = Some(format!("Chamfer gagal: {e}")),
        }
    }

    /// Shell/Hollow 1 body terpilih — buang face terjauh ke arah
    /// `shell_direction`, sisakan dinding setebal `shell_thickness_input`.
    /// Kalau `selected_faces` tidak kosong (mode "Pilih Wajah Manual"
    /// dipakai), buang wajah yang di-pick lewat
    /// `cadraw_kernel::shell_hollow_faces` alih-alih arah otomatis
    /// (Fase 8) — bisa >1 wajah sekaligus, beda dari `shell_hollow` lama.
    fn shell_selected_body(&mut self) {
        let Some(&id) = self.selected_bodies.iter().next().filter(|_| self.selected_bodies.len() == 1) else {
            self.model_status = Some("Pilih persis 1 body untuk Shell/Hollow".to_string());
            return;
        };
        let Ok(thickness) = self.shell_thickness_input.trim().parse::<f64>() else {
            self.model_status = Some("Tebal shell tidak valid".to_string());
            return;
        };
        let Some(geo) = self.model.geometry.get(id) else {
            return;
        };
        let result = if self.selected_faces.is_empty() {
            cadraw_kernel::shell_hollow(&geo.shape, thickness, self.shell_direction)
        } else {
            cadraw_kernel::shell_hollow_faces(&geo.shape, thickness, &self.selected_faces)
        };
        match result {
            Ok(shape) => {
                let new_geo = BodyGeometry::from_shape(shape);
                self.model_undo.execute(
                    Box::new(ReplaceGeometryCommand::new("Shell", id, new_geo)),
                    &mut self.model,
                );
                self.round_history.remove(&id);
                self.selected_faces.clear();
                self.picking_mode = PickMode::None;
                self.model_status = None;
            }
            Err(e) => self.model_status = Some(format!("Shell gagal: {e}")),
        }
    }

    /// Toleransi RESOLUSI ULANG ray tersimpan (`fillet_edges`/
    /// `chamfer_edges`, dipanggil dari tombol panel, BUKAN dari dalam
    /// `viewport()` — tidak ada akses `rect` layar sekarang di sini,
    /// beda dari toleransi PICK di `handle_3d_picking` yang piksel-based
    /// lewat `pixel_tolerance_to_world`). Tidak perlu sama persis dengan
    /// toleransi pick: ray & geometri TIDAK berubah antara pick dan apply
    /// (`deep_clone` cuma menyalin, tidak menggeser), jadi tepi yang sudah
    /// ke-pick (jarak ray-ke-tepi ≈ 0) akan selalu ditemukan lagi asal
    /// toleransi "cukup longgar" — nilai tetap dalam mm, bukan berbasis
    /// piksel/kamera, cukup untuk itu.
    const EDGE_REAPPLY_TOLERANCE_MM: f64 = 5.0;

    /// Raycast terhadap semua body solid 3D yang visible, mencari face terdekat yang terkena kursor mouse.
    fn pick_body_face_at_cursor(&self, rect: egui::Rect, pos: egui::Pos2) -> Option<(BodyId, PickRay, FaceHit)> {
        let (origin, dir) = screen_to_ray(&self.camera, rect, pos);
        let ray = PickRay {
            origin: (origin.x as f64, origin.y as f64, origin.z as f64),
            dir: (dir.x as f64, dir.y as f64, dir.z as f64),
        };
        eprintln!(
            "[DEBUG pick_body_face_at_cursor] pos={pos:?} ray.origin={:?} ray.dir={:?} total_geometry={} total_bodies={}",
            ray.origin, ray.dir, self.model.geometry.len(), self.model.doc.bodies.len()
        );
        let mut closest: Option<(BodyId, PickRay, FaceHit, f64)> = None;
        for (id, geo) in self.model.geometry.iter() {
            match self.model.doc.bodies.get(id) {
                Some(body) => {
                    let mut min = [f32::INFINITY; 3];
                    let mut max = [f32::NEG_INFINITY; 3];
                    for p in &geo.mesh.positions {
                        for k in 0..3 {
                            min[k] = min[k].min(p[k]);
                            max[k] = max[k].max(p[k]);
                        }
                    }
                    eprintln!(
                        "[DEBUG pick_body_face_at_cursor]   body id={id:?} visible={} mesh_aabb_min={min:?} mesh_aabb_max={max:?} vertex_count={}",
                        body.visible, geo.mesh.positions.len()
                    );
                    if body.visible {
                        match cadraw_kernel::pick_face_details(&geo.shape, ray) {
                            Some(hit) => {
                                eprintln!("[DEBUG pick_body_face_at_cursor]     HIT hit_point={:?} normal={:?}", hit.hit_point, hit.normal);
                                let hit_vec = glam::DVec3::new(hit.hit_point.0, hit.hit_point.1, hit.hit_point.2);
                                let orig_vec = glam::DVec3::new(ray.origin.0, ray.origin.1, ray.origin.2);
                                let dist_sq = (hit_vec - orig_vec).length_squared();
                                if closest.as_ref().is_none_or(|(_, _, _, d)| dist_sq < *d) {
                                    closest = Some((id, ray, hit, dist_sq));
                                }
                            }
                            None => eprintln!("[DEBUG pick_body_face_at_cursor]     MISS (pick_face_details None)"),
                        }
                    }
                }
                None => eprintln!("[DEBUG pick_body_face_at_cursor]   body id={id:?} TIDAK ADA di doc.bodies (geometry yatim?)"),
            }
        }
        eprintln!("[DEBUG pick_body_face_at_cursor] result = {}", if closest.is_some() { "Some" } else { "None" });
        closest.map(|(id, ray, hit, _)| (id, ray, hit))
    }

    /// Raycast terhadap semua body solid 3D yang visible, mencari VERTEX
    /// (sudut) terdekat yang terkena kursor mouse — dipakai gizmo vertex
    /// fillet (Fase 2). Toleransi piksel dibuat LEBIH LONGGAR (~18x, vs
    /// ~14x buat face/edge di `pick_body_face_at_cursor`/
    /// `pick_body_edge_at_cursor`, dan dikalikan lagi ke
    /// `pixel_tolerance_to_world` yang sudah dalam mm) karena target vertex
    /// jauh lebih kecil secara visual daripada face ATAU edge — dicoba
    /// klik SEBELUM `pick_body_edge_at_cursor`/`pick_body_face_at_cursor`
    /// di `handle_sketch_input` supaya menang prioritas dan sudut tetap
    /// bisa dipilih walau ketutup face. Nilai lama (12x, ≈2.76mm/12px di
    /// zoom umum) ternyata LEBIH KETAT dari yang dimaksud komentar ini —
    /// klik di rusuk (edge) pojok box, cuma ~2.5cm dari vertex-nya, masih
    /// meleset; 18x mendekati diameter marker vertex yang digambar
    /// `build_overlay_lines` supaya "yang terlihat" dan "yang bisa diklik"
    /// konsisten.
    fn pick_body_vertex_at_cursor(&self, rect: egui::Rect, pos: egui::Pos2) -> Option<(BodyId, PickRay, (f64, f64, f64))> {
        let (origin, dir) = screen_to_ray(&self.camera, rect, pos);
        let ray = PickRay {
            origin: (origin.x as f64, origin.y as f64, origin.z as f64),
            dir: (dir.x as f64, dir.y as f64, dir.z as f64),
        };
        let tolerance = pixel_tolerance_to_world(&self.camera, rect) * 18.0;
        let mut closest: Option<(BodyId, PickRay, (f64, f64, f64), f64)> = None;
        for (id, geo) in self.model.geometry.iter() {
            let Some(body) = self.model.doc.bodies.get(id) else {
                continue;
            };
            if !body.visible {
                continue;
            }
            if let Some(hit) = cadraw_kernel::pick_vertex(&geo.shape, ray, tolerance) {
                let hit_vec = glam::DVec3::new(hit.0, hit.1, hit.2);
                let orig_vec = glam::DVec3::new(ray.origin.0, ray.origin.1, ray.origin.2);
                let dist_sq = (hit_vec - orig_vec).length_squared();
                if closest.as_ref().is_none_or(|(_, _, _, d)| dist_sq < *d) {
                    closest = Some((id, ray, hit, dist_sq));
                }
            } else if let Some(nearest) = cadraw_kernel::pick_vertex(&geo.shape, ray, f64::INFINITY) {
                let v = glam::DVec3::new(nearest.0, nearest.1, nearest.2);
                let o = glam::DVec3::new(ray.origin.0, ray.origin.1, ray.origin.2);
                let d = glam::DVec3::new(ray.dir.0, ray.dir.1, ray.dir.2);
                let t = d.dot(v - o) / d.length_squared();
                let dist = (o + d * t - v).length();
                eprintln!(
                    "[DEBUG pick_vertex] body={id:?} MISS: vertex terdekat=({:.1}, {:.1}, {:.1}) jarak_ke_ray={dist:.2}mm > tolerance={tolerance:.2}mm",
                    nearest.0, nearest.1, nearest.2
                );
            }
        }
        closest.map(|(id, ray, hit, _)| (id, ray, hit))
    }

    /// Raycast terhadap semua body solid 3D yang visible, mencari RUSUK
    /// (edge) terdekat yang terkena kursor mouse — dicoba SETELAH
    /// `pick_body_vertex_at_cursor` meleset, SEBELUM `pick_body_face_at_cursor`,
    /// buat kasus "klik sudut kubus" yang sebenarnya jatuh di rusuk
    /// vertikal pojok (seperti sudut tembok ruangan), bukan di titik
    /// vertex kecilnya (lihat laporan bug: klik konsisten ~22mm dari
    /// vertex B-rep terdekat, tapi tepat di tengah rusuk vertikal). Kalau
    /// kena, `handle_sketch_input` menampilkan gizmo rounding (sama
    /// persis dgn gizmo vertex, cuma commit-nya `fillet_edges` 1 rusuk
    /// alih-alih `fillet_vertex`) berlabuh di titik klik pada rusuk itu.
    /// Toleransi 14x — SAMA dgn `pick_body_face_at_cursor` dan pick edge
    /// manual di `PickMode::Edge` (`handle_3d_picking`) — sudah cukup
    /// longgar utk rusuk tipis secara visual tanpa menelan face di
    /// sekitarnya.
    fn pick_body_edge_at_cursor(&self, rect: egui::Rect, pos: egui::Pos2) -> Option<(BodyId, PickRay, (f64, f64, f64))> {
        let (origin, dir) = screen_to_ray(&self.camera, rect, pos);
        let ray = PickRay {
            origin: (origin.x as f64, origin.y as f64, origin.z as f64),
            dir: (dir.x as f64, dir.y as f64, dir.z as f64),
        };
        let tolerance = pixel_tolerance_to_world(&self.camera, rect) * 14.0;
        let mut closest: Option<(BodyId, PickRay, (f64, f64, f64), f64)> = None;
        for (id, geo) in self.model.geometry.iter() {
            let Some(body) = self.model.doc.bodies.get(id) else {
                continue;
            };
            if !body.visible {
                continue;
            }
            if let Some((point, _polyline)) = cadraw_kernel::pick_edge(&geo.shape, ray, tolerance) {
                let hit_vec = glam::DVec3::new(point.0, point.1, point.2);
                let orig_vec = glam::DVec3::new(ray.origin.0, ray.origin.1, ray.origin.2);
                let dist_sq = (hit_vec - orig_vec).length_squared();
                if closest.as_ref().is_none_or(|(_, _, _, d)| dist_sq < *d) {
                    closest = Some((id, ray, point, dist_sq));
                }
            }
        }
        closest.map(|(id, ray, point, _)| (id, ray, point))
    }

    /// Posisi 3D `active_vertex` saat ini + arah "keluar" gizmo
    /// (`normalize(vertex − pusat bbox body)`) — dipakai bareng oleh overlay
    /// garis (`build_overlay_lines`) dan HUD interaktif (`dynamic_input_ui`)
    /// gizmo vertex fillet (Fase 3), supaya keduanya selalu sepakat soal ke
    /// mana gizmo mengarah. Pusat bbox dihitung dari AABB mesh tessellasi
    /// body (pola sama dengan `pick_body_face_at_cursor`) — bukan pusat
    /// solid B-rep yang presisi, tapi cukup untuk arah kasar "menjauhi
    /// body" dan jauh lebih murah daripada query kernel. Fallback ke
    /// `Vec3::Z` kalau vertex kebetulan persis di pusat bbox (arah nol).
    fn active_vertex_gizmo_dir(&self) -> Option<(Vec3, Vec3)> {
        let (body_id, _, vhit) = self.active_vertex?;
        let vertex = Vec3::new(vhit.0 as f32, vhit.1 as f32, vhit.2 as f32);
        let geo = self.model.geometry.get(body_id)?;
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for p in &geo.mesh.positions {
            for k in 0..3 {
                min[k] = min[k].min(p[k]);
                max[k] = max[k].max(p[k]);
            }
        }
        let center = Vec3::new((min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5, (min[2] + max[2]) * 0.5);
        let mut dir = (vertex - center).normalize_or_zero();
        if dir == Vec3::ZERO {
            dir = Vec3::Z;
        }
        Some((vertex, dir))
    }

    /// Cermin `active_vertex_gizmo_dir`, tapi utk `active_edge`: jangkar
    /// gizmo adalah titik KLIK pada rusuk (bukan titik vertex resmi B-rep)
    /// supaya gizmo rounding "berlabuh di titik klik pada edge" persis
    /// seperti diminta — arah "keluar" tetap dihitung sama, relatif ke
    /// pusat AABB body.
    fn active_edge_gizmo_dir(&self) -> Option<(Vec3, Vec3)> {
        let (body_id, _, point) = self.active_edge?;
        let anchor = Vec3::new(point.0 as f32, point.1 as f32, point.2 as f32);
        let geo = self.model.geometry.get(body_id)?;
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for p in &geo.mesh.positions {
            for k in 0..3 {
                min[k] = min[k].min(p[k]);
                max[k] = max[k].max(p[k]);
            }
        }
        let center = Vec3::new((min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5, (min[2] + max[2]) * 0.5);
        let mut dir = (anchor - center).normalize_or_zero();
        if dir == Vec3::ZERO {
            dir = Vec3::Z;
        }
        Some((anchor, dir))
    }

    /// Teks HUD pill gizmo face (CADRAW Fase 4): permukaan radial
    /// (Cylinder/Cone/Sphere — di mana `FaceHit::pull_dir` benar-benar
    /// arah radial, lihat dokumentasinya) diberi label "ΔR" + tanda eksplisit
    /// (mis. "ΔR +2.0 mm") karena drag di situ mengubah RADIUS, bukan
    /// menggeser bidang datar; permukaan lain (Plane, dan fallback
    /// Torus/Other yang masih memakai `normal` sbg `pull_dir`) tetap tampil
    /// sebagai jarak polos (mis. "2.0 mm"), perilaku lama.
    fn format_face_gizmo_dimension_text(&self, surface_kind: SurfaceKind, distance: f64) -> String {
        let formatted = self.unit.format(distance);
        if matches!(surface_kind, SurfaceKind::Cylinder | SurfaceKind::Cone | SurfaceKind::Sphere) {
            if distance >= 0.0 {
                format!("ΔR +{formatted}")
            } else {
                // `formatted` sudah mengandung tanda minus dari format float Rust.
                format!("ΔR {formatted}")
            }
        } else {
            formatted
        }
    }

    /// Extrude sisi/face 3D yang sedang aktif sepanjang `distance` mm.
    fn extrude_active_face(&mut self, distance: f64) {
        let Some((target_id, ray, _hit)) = self.active_face else {
            self.model_status = Some("Pilih salah satu sisi (face) objek terlebih dahulu".to_string());
            return;
        };
        let Some(target_geo) = self.model.geometry.get(target_id) else {
            self.model_status = Some("Body terpilih tidak ditemukan".to_string());
            return;
        };
        match cadraw_kernel::extrude_face(&target_geo.shape, ray, distance) {
            Ok(new_shape) => {
                let new_geo = BodyGeometry::from_shape(new_shape);
                let label = if distance > 0.0 { "Extrude Face" } else { "Cut Face" };
                self.model_undo.execute(
                    Box::new(ReplaceGeometryCommand::new(label, target_id, new_geo)),
                    &mut self.model,
                );
                self.round_history.remove(&target_id);
                self.active_face = None;
                self.model_status = Some(format!("Extrude face {:.1} mm sukses", distance));
            }
            Err(e) => {
                self.model_status = Some(format!("Extrude face gagal: {e}"));
            }
        }
    }

    /// Jadikan permukaan sisi (face) 3D yang sedang aktif sebagai bidang sketsa baru (Sketch on Face).
    fn sketch_on_active_face(&mut self) {
        let Some((_target_id, _ray, hit)) = self.active_face else {
            self.model_status = Some("Pilih salah satu sisi (face) objek terlebih dahulu".to_string());
            return;
        };
        let origin = Vec3::new(hit.centroid.0 as f32, hit.centroid.1 as f32, hit.centroid.2 as f32);
        let normal = Vec3::new(hit.normal.0 as f32, hit.normal.1 as f32, hit.normal.2 as f32);
        self.active_plane = SketchPlane::from_origin_normal(origin, normal);
        self.is_sketching = true;
        self.left_toolbar.is_sketching = true;
        self.camera.orient_to_plane(&self.active_plane);
        self.active_face = None;
        self.model_status = Some("Sketsa aktif pada permukaan sisi objek".to_string());
    }

    /// Hapus semua body terpilih (masing-masing 1 command undo-able).
    fn delete_selected_bodies(&mut self) {
        for id in std::mem::take(&mut self.selected_bodies) {
            self.model_undo
                .execute(Box::new(DeleteBodyCommand::new(id)), &mut self.model);
            self.round_history.remove(&id);
        }
    }

    /// Merge mesh semua body VISIBLE + highlight face cyan 2D + preview extrude/boolean
    /// jadi satu buffer gabungan dengan per-vertex color.
    fn build_combined_body_mesh(&self) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 4]>, Vec<u32>) {
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut colors = Vec::new();
        let mut indices = Vec::new();

        const CAD_GREY: [f32; 4] = [0.62, 0.68, 0.76, 1.0];
        const CYAN_FACE_SELECTED: [f32; 4] = [0.0, 0.75, 1.0, 0.85];
        const CYAN_CUT_PREVIEW: [f32; 4] = [0.0, 0.80, 1.0, 0.90];

        // Precompute SEMUA preview live (boolean cut dari sketch, extrude
        // face, rounding vertex/edge) SEBELUM loop body normal di bawah —
        // supaya body asli HANYA disembunyikan kalau mesh penggantinya
        // BENAR-BENAR berhasil dihitung frame ini, bukan cuma "berdasarkan
        // flag drag aktif". User melaporkan (screenshot): begitu drag
        // mendekati/melewati batas radius rounding, objek 3D-nya hilang
        // total sampai drag dilepas — sebab lama: body disembunyikan
        // duluan (berdasarkan flag), lalu preview-nya BISA gagal (mis.
        // radius pas di batas paling ekstrem, OCCT sendiri masih bisa
        // `Err` walau lolos precheck `max_fillet_radius` kita — lihat
        // dokumentasinya), jadi tidak ada apa pun yang menggantikannya.
        // Pola precompute-lalu-cek-`Some` di bawah berlaku SAMA utk ketiga
        // jenis preview supaya bug kelas yang sama tidak muncul lagi di
        // gizmo lain.
        let sketch_cut_preview: Option<(BodyId, KernelMesh)> =
            (self.extruding_from_gizmo && self.gizmo_is_cutting && self.gizmo_distance.abs() > 0.01)
                .then(|| self.gizmo_target_body)
                .flatten()
                .and_then(|target_id| {
                    let target_geo = self.model.geometry.get(target_id)?;
                    let profile = model::build_profile_from_selection(self.sketch(), &self.selected).ok()?;
                    let extruded_shape = self.extrude_profile_active_plane(&profile, self.gizmo_distance).ok()?;
                    let cut_shape = cadraw_kernel::subtract(&target_geo.shape, &extruded_shape).ok()?;
                    Some((target_id, cut_shape.tessellate()))
                });

        // Extrude sketch BUKAN cut: tidak menyembunyikan body apa pun (body
        // barunya belum ada), jadi tidak butuh id — cukup mesh preview-nya.
        let sketch_extrude_preview: Option<KernelMesh> =
            (self.extruding_from_gizmo && !self.gizmo_is_cutting && self.gizmo_distance.abs() > 0.01)
                .then(|| {
                    let profile = model::build_profile_from_selection(self.sketch(), &self.selected).ok()?;
                    let extruded_shape = self.extrude_profile_active_plane(&profile, self.gizmo_distance).ok()?;
                    Some(extruded_shape.tessellate())
                })
                .flatten();

        let face_extrude_preview: Option<(BodyId, KernelMesh)> =
            (self.extruding_face_from_gizmo && self.face_gizmo_distance.abs() > 0.01)
                .then(|| self.active_face)
                .flatten()
                .and_then(|(target_id, ray, _hit)| {
                    let target_geo = self.model.geometry.get(target_id)?;
                    let preview_shape = cadraw_kernel::extrude_face(&target_geo.shape, ray, self.face_gizmo_distance).ok()?;
                    Some((target_id, preview_shape.tessellate()))
                });

        let vertex_round_preview: Option<(BodyId, KernelMesh)> = self
            .filleting_vertex_from_gizmo
            .then(|| self.round_gizmo_preview_shape(RoundKind::Vertex, self.vertex_gizmo_radius))
            .flatten()
            .map(|(id, shape)| (id, shape.tessellate()));

        let edge_round_preview: Option<(BodyId, KernelMesh)> = self
            .filleting_edge_from_gizmo
            .then(|| self.round_gizmo_preview_shape(RoundKind::Edge, self.edge_gizmo_radius))
            .flatten()
            .map(|(id, shape)| (id, shape.tessellate()));

        // 1. Solid bodies normal — body disembunyikan HANYA kalau preview
        // penggantinya (di-precompute di atas) benar-benar ada (`Some`).
        for (id, geo) in self.model.geometry.iter() {
            if let Some(body) = self.model.doc.bodies.get(id) {
                if body.visible {
                    if sketch_cut_preview.as_ref().is_some_and(|(target_id, _)| *target_id == id) {
                        continue;
                    }
                    if face_extrude_preview.as_ref().is_some_and(|(target_id, _)| *target_id == id) {
                        continue;
                    }
                    if vertex_round_preview.as_ref().is_some_and(|(target_id, _)| *target_id == id) {
                        continue;
                    }
                    if edge_round_preview.as_ref().is_some_and(|(target_id, _)| *target_id == id) {
                        continue;
                    }
                    let offset = positions.len() as u32;
                    positions.extend_from_slice(&geo.mesh.positions);
                    normals.extend_from_slice(&geo.mesh.normals);
                    for _ in 0..geo.mesh.positions.len() {
                        colors.push(CAD_GREY);
                    }
                    indices.extend(geo.mesh.indices.iter().map(|i| i + offset));
                }
            }
        }

        // 2. 2D Active Face Highlight untuk profil yang terpilih (Screenshot 2)
        let closed_regions = find_closed_regions(self.sketch());
        for reg in &closed_regions {
            if reg.entity_ids.is_subset(&self.selected) {
                let tris = reg.triangulate();
                for chunk in tris.chunks(3) {
                    if chunk.len() == 3 {
                        let offset = positions.len() as u32;
                        for p in chunk {
                            let p_3d = self.active_plane.to_world(*p, 0.015);
                            positions.push([p_3d.x, p_3d.y, p_3d.z]);
                            normals.push([self.active_plane.normal.x, self.active_plane.normal.y, self.active_plane.normal.z]);
                            colors.push(CYAN_FACE_SELECTED);
                        }
                        indices.push(offset);
                        indices.push(offset + 1);
                        indices.push(offset + 2);
                    }
                }
            }
        }

        // 3. Live Extrude / Boolean Cut preview jika sedang drag gizmo
        // (nilai sudah dihitung di atas, sebelum loop body normal).
        if let Some((_, cut_mesh)) = &sketch_cut_preview {
            let offset = positions.len() as u32;
            positions.extend_from_slice(&cut_mesh.positions);
            normals.extend_from_slice(&cut_mesh.normals);
            for _ in 0..cut_mesh.positions.len() {
                colors.push(CYAN_CUT_PREVIEW);
            }
            indices.extend(cut_mesh.indices.iter().map(|i| i + offset));
        }
        if let Some(preview_mesh) = &sketch_extrude_preview {
            let offset = positions.len() as u32;
            positions.extend_from_slice(&preview_mesh.positions);
            normals.extend_from_slice(&preview_mesh.normals);
            for n in &preview_mesh.normals {
                if n[2].abs() > 0.7 {
                    colors.push(CYAN_FACE_SELECTED);
                } else {
                    colors.push([0.55, 0.62, 0.72, 1.0]);
                }
            }
            indices.extend(preview_mesh.indices.iter().map(|i| i + offset));
        }

        // 4. Live Face Extrude preview jika sedang drag gizmo face 3D —
        // cermin blok 3 di atas tapi utk `extrude_face` (bukan extrude dari
        // sketch): body asli sudah disembunyikan di blok 1 (KALAU preview
        // ini ada), di sini mesh hasil `extrude_face` versi TERKINI
        // (`face_gizmo_distance`) dipakai GANTI-nya tiap frame, bukan cuma
        // sekali saat drag dilepas — supaya pull/push face 3D langsung
        // kelihatan real-time sama seperti extrude dari sketch.
        if let Some((_, preview_mesh)) = &face_extrude_preview {
            let offset = positions.len() as u32;
            positions.extend_from_slice(&preview_mesh.positions);
            normals.extend_from_slice(&preview_mesh.normals);
            for _ in 0..preview_mesh.positions.len() {
                colors.push(CYAN_FACE_SELECTED);
            }
            indices.extend(preview_mesh.indices.iter().map(|i| i + offset));
        }

        // 5. Live Rounding (vertex/edge fillet) preview jika sedang drag
        // gizmo di pojokan — cermin blok 3/4 di atas: body asli sudah
        // disembunyikan di blok 1 (KALAU preview ini ada), di sini mesh
        // hasil `round_gizmo_preview_shape` versi TERKINI (radius drag saat
        // ini) dipakai GANTI-nya tiap frame, supaya rounding sudut/rusuk
        // langsung kelihatan real-time alih-alih cuma saat drag dilepas.
        if let Some((_, preview_mesh)) = &vertex_round_preview {
            let offset = positions.len() as u32;
            positions.extend_from_slice(&preview_mesh.positions);
            normals.extend_from_slice(&preview_mesh.normals);
            for _ in 0..preview_mesh.positions.len() {
                colors.push(CAD_GREY);
            }
            indices.extend(preview_mesh.indices.iter().map(|i| i + offset));
        }
        if let Some((_, preview_mesh)) = &edge_round_preview {
            let offset = positions.len() as u32;
            positions.extend_from_slice(&preview_mesh.positions);
            normals.extend_from_slice(&preview_mesh.normals);
            for _ in 0..preview_mesh.positions.len() {
                colors.push(CAD_GREY);
            }
            indices.extend(preview_mesh.indices.iter().map(|i| i + offset));
        }

        (positions, normals, colors, indices)
    }

    /// Bidang potong Section View siap kirim ke `SceneRenderer::set_clip_plane`
    /// — `None` kalau nonaktif. `section_invert` membalik `(normal, offset)`
    /// SEKALIGUS (bukan cuma normal) — plane `dot(n,p)=offset` dan
    /// `dot(-n,p)=-offset` adalah bidang yang SAMA persis secara geometris,
    /// jadi posisi potong di slider tidak ikut lompat saat cuma membalik
    /// sisi mana yang dibuang.
    fn section_clip_plane(&self) -> Option<(Vec3, f32)> {
        self.section_enabled.then(|| {
            let normal = self.section_axis.normal();
            if self.section_invert {
                (-normal, -self.section_offset)
            } else {
                (normal, self.section_offset)
            }
        })
    }

}

/// Untuk tool Trim: segmen (awal,akhir) yang akan terhapus jika `hover`
/// diklik sekarang pada entitas Line `id`. Dipakai preview hover; commit
/// klik menghitung ulang lewat `trim_segments` (lihat `handle_sketch_input`)
/// karena butuh daftar lengkap titik potong, bukan cuma satu bracket.
fn trim_removal_preview(sketch: &Sketch, id: EntityId, hover: DVec2) -> Option<(DVec2, DVec2)> {
    let Entity::Line { start, end } = sketch.entities.get(id)?.clone() else {
        return None;
    };
    let click_t = project_t(start, end, hover).clamp(0.0, 1.0);
    let mut ts: Vec<f64> = line_intersection_params_in_sketch(sketch, (start, end), id)
        .into_iter()
        .filter(|t| *t > 1e-6 && *t < 1.0 - 1e-6)
        .collect();
    ts.push(0.0);
    ts.push(1.0);
    ts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ts.windows(2)
        .find(|w| click_t >= w[0] && click_t <= w[1])
        .map(|w| (start + (end - start) * w[0], start + (end - start) * w[1]))
}

/// Ray dunia (titik dekat + arah) dari posisi kursor layar, lewat
/// unprojection kamera near/far — dipakai `screen_to_plane_point`
/// (intersect bidang Z=0, sketch) DAN picking edge/face 3D (Fase 8,
/// `CadrawApp::handle_3d_picking`, intersect langsung ke B-rep lewat
/// `cadraw_kernel::pick_face`/`pick_edge` — bukan bidang Z=0).
fn screen_to_ray(camera: &OrbitCamera, rect: egui::Rect, pos: egui::Pos2) -> (Vec3, Vec3) {
    let aspect = rect.width() / rect.height().max(1.0);
    let inv = camera.view_proj(aspect).inverse();

    let ndc_x = ((pos.x - rect.min.x) / rect.width()) * 2.0 - 1.0;
    let ndc_y = 1.0 - ((pos.y - rect.min.y) / rect.height()) * 2.0;

    // Konvensi kedalaman wgpu (Mat4::perspective_rh): NDC z ∈ [0, 1].
    let p_near = inv.project_point3(Vec3::new(ndc_x, ndc_y, 0.0));
    let p_far = inv.project_point3(Vec3::new(ndc_x, ndc_y, 1.0));
    (p_near, p_far - p_near)
}

/// Konversi posisi kursor layar → titik di bidang sketch aktif (`Top`, `Front`, `Right`),
/// lewat unprojection ray kamera dan interseksi ray-bidang.
fn screen_to_plane_point(
    camera: &OrbitCamera,
    rect: egui::Rect,
    pos: egui::Pos2,
    plane: &SketchPlane,
) -> Option<DVec2> {
    let (p_near, dir) = screen_to_ray(camera, rect, pos);
    plane.ray_intersection(p_near, dir)
}

/// Perkiraan unit-dunia per piksel layar pada kedalaman target kamera —
/// dipakai mengonversi toleransi hit-test/snap dari piksel ke mm. Toleransi
/// adaptif mouse-vs-sentuh yang lebih presisi menyusul di Fase 4.
fn pixel_tolerance_to_world(camera: &OrbitCamera, rect: egui::Rect) -> f64 {
    let world_per_pixel =
        2.0 * camera.distance * (camera.fov_y * 0.5).tan() / rect.height().max(1.0);
    world_per_pixel as f64
}

/// Proyeksikan titik 3D dunia ke koordinat piksel layar egui.
fn world_to_screen_pos(camera: &OrbitCamera, rect: egui::Rect, world_pt: Vec3) -> Option<egui::Pos2> {
    let aspect = rect.width() / rect.height().max(1.0);
    let vp = camera.view_proj(aspect);
    let clip = vp.project_point3(world_pt);
    if clip.z < 0.0 || clip.z > 1.0 {
        return None;
    }
    let screen_x = rect.min.x + (clip.x + 1.0) * 0.5 * rect.width();
    let screen_y = rect.min.y + (1.0 - clip.y) * 0.5 * rect.height();
    Some(egui::pos2(screen_x, screen_y))
}

impl eframe::App for CadrawApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. Polling worker latar belakang Import STEP
        self.poll_import_worker();
        if self.pending_imports > 0 {
            ctx.request_repaint();
        }

        // 2. Keyboard shortcuts global (⌘Z / ⌘⇧Z / ⌘S / ⌘O / ⌘K / ⌘⇧2 / ⌘⇧3)
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::K)) {
            self.palette.toggle();
        }
        let mode_sketch_pressed =
            ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::Num2));
        let mode_3d_pressed =
            ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::Num3));
        if mode_sketch_pressed {
            if !self.is_sketching {
                self.is_sketching = true;
                self.left_toolbar.is_sketching = true;
                self.camera.orient_to_plane(&self.active_plane);
            }
        }
        if mode_3d_pressed {
            if self.is_sketching {
                self.is_sketching = false;
                self.left_toolbar.is_sketching = false;
                self.set_tool(ToolKind::Select);
            }
        }

        let undo_pressed =
            ctx.input(|i| i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::Z));
        let redo_pressed = ctx.input(|i| {
            i.modifiers.command
                && (i.key_pressed(egui::Key::Y) || (i.modifiers.shift && i.key_pressed(egui::Key::Z)))
        });
        if undo_pressed {
            self.undo_active_sketch();
        }
        if redo_pressed {
            self.redo_active_sketch();
        }

        let save_as_pressed =
            ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::S));
        let save_pressed =
            ctx.input(|i| i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::S));
        let open_pressed = ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::O));
        if save_as_pressed {
            self.save_native_as();
        } else if save_pressed {
            self.save_native();
        }
        if open_pressed {
            self.open_native();
        }

        // 3. CentralPanel: 100% Full Viewport Canvas
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                self.viewport(ui);
            });

        let screen_rect = ctx.content_rect();
        let screen_center_x = screen_rect.center().x;

        // 4. Modern Top Bar (Header Full Sampai Kanan) — Berisi Mode Switcher,
        // Items, Search, Sketch Plane (khusus Sketch Mode), Section View,
        // Measurements, Delete: SEMUA kontrol yang selalu sama di kedua mode.
        // Dirender LEBIH DULU dari Items Drawer supaya tombol Items-nya sudah
        // punya rect layar (`items_button_rect`) buat menempatkan popup drawer.
        let topbar_margin_right = 12.0;
        let topbar_x = 12.0;
        let topbar_w = (screen_rect.max.x - topbar_x - topbar_margin_right).max(200.0);
        let doc_name = self
            .current_file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled.cadraw")
            .to_string();
        let is_saved = self.current_file_path.is_some();

        let mut topbar_state = TopBarState {
            document_name: doc_name,
            status_saved: is_saved,
            current_unit: self.model.doc.unit,
            is_sketching: self.is_sketching,
            items_drawer_open: self.items_drawer_open,
            section_view_active: self.section_enabled,
            is_measure_active: matches!(self.tool, ToolKind::Measure | ToolKind::MeasureAngle),
            active_plane_name: self.active_plane.name().to_string(),
            plane_menu_open: self.plane_menu_open,
            items_button_rect: egui::Rect::NOTHING,
        };

        egui::Area::new(egui::Id::new("cadraw-topbar-floating"))
            .fixed_pos(egui::pos2(topbar_x, 8.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.set_width(topbar_w);
                if let Some(top_event) = TopBar::show(ui, &mut topbar_state) {
                    match top_event {
                        TopBarEvent::HomeClicked => {
                            self.new_document();
                        }
                        TopBarEvent::SetUnit(u) => {
                            self.unit = u;
                            self.model.doc.unit = u;
                        }
                        TopBarEvent::File(op) => match op {
                            TopBarFileOp::New => self.new_document(),
                            TopBarFileOp::Open => self.open_native(),
                            TopBarFileOp::Save => self.save_native(),
                            TopBarFileOp::SaveAs => self.save_native_as(),
                            TopBarFileOp::ImportStep => self.import_step(),
                            TopBarFileOp::ImportDxf => self.import_dxf(),
                            TopBarFileOp::ExportStep => self.export_step(),
                            TopBarFileOp::ExportStl => self.export_stl(),
                            TopBarFileOp::ExportObj => self.export_obj(),
                            TopBarFileOp::ExportDxf => self.export_dxf(),
                        },
                        TopBarEvent::ToggleTheme => {
                            self.theme = self.theme.toggled();
                            cadraw_ui::apply_theme(ctx, self.theme);
                        }
                        TopBarEvent::OpenCommandPalette => {
                            self.palette.open();
                        }
                        TopBarEvent::ToggleItemsDrawer => {
                            self.items_drawer_open = !self.items_drawer_open;
                        }
                        TopBarEvent::OpenSearch => {
                            self.palette.open();
                        }
                        TopBarEvent::EnterSketching => {
                            self.is_sketching = true;
                            self.left_toolbar.is_sketching = true;
                            self.camera.orient_to_plane(&self.active_plane);
                        }
                        TopBarEvent::ExitSketching => {
                            self.is_sketching = false;
                            self.left_toolbar.is_sketching = false;
                            self.set_tool(ToolKind::Select);
                        }
                        TopBarEvent::SelectSketchPlane(idx) => {
                            let kind = match idx {
                                0 => PlaneKind::Top,
                                1 => PlaneKind::Front,
                                2 => PlaneKind::Right,
                                _ => PlaneKind::Top,
                            };
                            self.set_sketch_plane(kind);
                        }
                        TopBarEvent::ToggleSectionView => {
                            self.section_enabled = !self.section_enabled;
                        }
                        TopBarEvent::ToggleMeasurements => {
                            // Klik lagi tombol "📏 Ukur" saat tool Ukur Jarak/Ukur
                            // Sudut sudah aktif -> deactivate (balik ke Select),
                            // bukan cuma reset ke Measure lagi.
                            let already_active =
                                matches!(self.tool, ToolKind::Measure | ToolKind::MeasureAngle);
                            self.set_tool(if already_active { ToolKind::Select } else { ToolKind::Measure });
                        }
                        TopBarEvent::DeleteSelection => {
                            if !self.selected.is_empty() {
                                let to_delete: Vec<EntityId> = self.selected.iter().copied().collect();
                                self.execute_sketch_command(Box::new(DeleteEntities::new(to_delete)));
                                self.selected.clear();
                            }
                            if !self.selected_bodies.is_empty() {
                                self.delete_selected_bodies();
                            }
                        }
                    }
                }
            });

        // Salin balik state yang bisa dimutasi TopBar::show ke App (pola sama
        // seperti `inspector_state` di bawah — field lain di-set langsung
        // lewat event handler di atas, bukan lewat state ini).
        self.plane_menu_open = topbar_state.plane_menu_open;
        let items_button_rect = topbar_state.items_button_rect;

        // 5. Left Floating Toolbar — Tool-Tool Spesifik Mode (Pilih + Sketsa
        // 2D, muncul saat Sketch Mode), Dipusatkan Vertikal Antara Atas &
        // Bawah Viewport (meniru pola Panel Properti kanan, lihat
        // `left_toolbar_content_sig` / `InspectorContentSig`).
        self.left_toolbar.is_sketching = self.is_sketching;
        let left_toolbar_force_resize = self.left_toolbar_content_sig != Some(self.is_sketching);
        self.left_toolbar_content_sig = Some(self.is_sketching);
        egui::Area::new(egui::Id::new("cadraw-left-toolbar-area"))
            .fixed_pos(egui::pos2(12.0, screen_rect.center().y))
            .pivot(egui::Align2::LEFT_CENTER)
            .constrain_to(screen_rect)
            .default_size(egui::vec2(60.0, 460.0))
            .sizing_pass(left_toolbar_force_resize)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                if let Some(tb_ev) = self.left_toolbar.show(ui, self.tool.to_toolbar_tool()) {
                    match tb_ev {
                        ToolbarEvent::SelectTool(t) => {
                            self.set_tool(ToolKind::from_toolbar_tool(t));
                        }
                    }
                }
            });

        // 6. Items Tree Drawer (Muncul di bawah tombol Items di header saat dibuka)
        if self.items_drawer_open {
            let sketch_planes = vec![
                SketchPlaneItemInfo {
                    index: 0,
                    name: format!("Plane 01 - Top (XY) ({})", self.sketches[0].entities.len()),
                    active: self.active_plane.kind == PlaneKind::Top,
                    visible: true,
                },
                SketchPlaneItemInfo {
                    index: 1,
                    name: format!("Plane 02 - Front (XZ) ({})", self.sketches[1].entities.len()),
                    active: self.active_plane.kind == PlaneKind::Front,
                    visible: true,
                },
                SketchPlaneItemInfo {
                    index: 2,
                    name: format!("Plane 03 - Right (YZ) ({})", self.sketches[2].entities.len()),
                    active: self.active_plane.kind == PlaneKind::Right,
                    visible: true,
                },
            ];
            let bodies: Vec<BodyItemInfo> = self
                .model
                .doc
                .bodies
                .iter()
                .map(|(id, b)| BodyItemInfo {
                    id_raw: id.data().as_ffi(),
                    name: b.name.clone(),
                    visible: b.visible,
                    selected: self.selected_bodies.contains(&id),
                })
                .collect();

            let drawer_pos = egui::pos2(items_button_rect.left(), items_button_rect.bottom() + 6.0);
            egui::Area::new(egui::Id::new("cadraw-items-drawer-area"))
                .fixed_pos(drawer_pos)
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    if let Some(ev) = self.items_drawer.show(ui, &sketch_planes, &bodies) {
                        match ev {
                            ItemsDrawerEvent::ToggleBodyVisibility(raw_id) => {
                                for (id, b) in self.model.doc.bodies.iter_mut() {
                                    if id.data().as_ffi() == raw_id {
                                        b.visible = !b.visible;
                                        break;
                                    }
                                }
                            }
                            ItemsDrawerEvent::SelectBody { id_raw, extend } => {
                                for (id, _) in self.model.doc.bodies.iter() {
                                    if id.data().as_ffi() == id_raw {
                                        if !extend {
                                            self.selected_bodies.clear();
                                        }
                                        if !self.selected_bodies.remove(&id) {
                                            self.selected_bodies.insert(id);
                                        }
                                        self.model_status = None;
                                        break;
                                    }
                                }
                            }
                            ItemsDrawerEvent::ToggleSketchVisibility(_) => {}
                            ItemsDrawerEvent::SelectSketchPlane(idx) => {
                                let kind = match idx {
                                    0 => PlaneKind::Top,
                                    1 => PlaneKind::Front,
                                    2 => PlaneKind::Right,
                                    _ => PlaneKind::Top,
                                };
                                self.set_sketch_plane(kind);
                            }
                        }
                    }
                });
        }

        // 7. Right Properties & Features Inspector (Fixed di Kanan Kanvas, Sejajar Tepi Kanan dg Header)
        let is_editing_or_drawing = self.tool != ToolKind::Select;
        let has_active_selection = !self.selected.is_empty() || !self.selected_bodies.is_empty();
        // Tool Ukur/Ukur Sudut aktif, atau masih ada hasil pengukuran tersimpan,
        // memaksa panel tetap tampil (kartu "📏 Pengukuran" di dalamnya) —
        // dulu ini kondisi yang sama dipakai jendela mengambang terpisah,
        // sekarang panel yang sama dipakai supaya konsisten dengan panel lain.
        let measure_tool_active = matches!(self.tool, ToolKind::Measure | ToolKind::MeasureAngle);
        let has_measurements = !self.measurements.is_empty();
        let show_right_sidebar = if self.auto_hide_properties {
            ((!is_editing_or_drawing && has_active_selection) || measure_tool_active || has_measurements)
                && self.feature_inspector_open
        } else {
            self.feature_inspector_open
        };

        // 8. Interactive 3D ViewCube (Selalu di Pojok Kanan Atas, Tidak Bergeser Lagi
        // Karena Panel Properti Sekarang Ada di Tengah Vertikal, Bukan di Pojok yang Sama)
        let viewcube_y = 102.0;
        let viewcube_pos = egui::pos2(screen_rect.max.x - topbar_margin_right - 42.0, viewcube_y);
        egui::Area::new(egui::Id::new("cadraw-viewcube-area"))
            .fixed_pos(viewcube_pos - egui::vec2(42.0, 42.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                if let Some(action) = self.viewcube.show(ui, viewcube_pos, self.camera.yaw, self.camera.pitch) {
                    match action {
                        ViewCubeAction::Top => self.camera.set_preset(ViewPreset::Top),
                        ViewCubeAction::Bottom => self.camera.set_preset(ViewPreset::Bottom),
                        ViewCubeAction::Front => self.camera.set_preset(ViewPreset::Front),
                        ViewCubeAction::Back => self.camera.set_preset(ViewPreset::Back),
                        ViewCubeAction::Right => self.camera.set_preset(ViewPreset::Right),
                        ViewCubeAction::Left => self.camera.set_preset(ViewPreset::Left),
                        ViewCubeAction::Isometric => self.camera.set_preset(ViewPreset::Isometric),
                    }
                }
            });

        // In-Canvas HUD: Top Center Normal to Sketch Button & Section Banner
        if self.tool != ToolKind::Select {
            egui::Area::new(egui::Id::new("cadraw-hud-normal-to-sketch"))
                .fixed_pos(egui::pos2(screen_center_x - 75.0, 56.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    if let Some(hud_ev) = CanvasHud::show_normal_to_sketch_btn(ui) {
                        if hud_ev == CanvasHudEvent::OrientNormalToSketch {
                            self.camera.orient_to_plane(&self.active_plane);
                        }
                    }
                });
        }
        if self.section_enabled {
            egui::Area::new(egui::Id::new("cadraw-hud-section-banner"))
                .fixed_pos(egui::pos2(screen_center_x - 140.0, 94.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    if let Some(hud_ev) = CanvasHud::show_section_view_banner(ui) {
                        if hud_ev == CanvasHudEvent::TurnOffSectionView {
                            self.section_enabled = false;
                        }
                    }
                });
        }

        // Populate SelectedEntityData
        let selected_entity_data = if self.selected.len() == 1 {
            let &id = self.selected.iter().next().unwrap();
            let id_raw = id.data().as_ffi();
            let entity_opt = self.sketch().entities.get(id).cloned();
            match entity_opt {
                Some(Entity::Line { start, end }) => {
                    let length = (end - start).length();
                    let angle_deg = (end - start).y.atan2((end - start).x).to_degrees();
                    if self.last_inspected_entity_id != Some(id_raw) {
                        self.prop_input_p1_x = format!("{:.2}", start.x);
                        self.prop_input_p1_y = format!("{:.2}", start.y);
                        self.prop_input_p2_x = format!("{:.2}", end.x);
                        self.prop_input_p2_y = format!("{:.2}", end.y);
                        self.prop_input_val_1 = format!("{:.2}", length);
                        self.prop_input_val_2 = format!("{:.1}", angle_deg);
                        self.last_inspected_entity_id = Some(id_raw);
                    }
                    SelectedEntityData::Line {
                        id_raw,
                        start_x: start.x,
                        start_y: start.y,
                        end_x: end.x,
                        end_y: end.y,
                        length,
                        angle_deg,
                    }
                }
                Some(Entity::Circle { center, radius }) => {
                    let diameter = radius * 2.0;
                    if self.last_inspected_entity_id != Some(id_raw) {
                        self.prop_input_p1_x = format!("{:.2}", center.x);
                        self.prop_input_p1_y = format!("{:.2}", center.y);
                        self.prop_input_val_1 = format!("{:.2}", radius);
                        self.prop_input_val_2 = format!("{:.2}", diameter);
                        self.last_inspected_entity_id = Some(id_raw);
                    }
                    SelectedEntityData::Circle {
                        id_raw,
                        center_x: center.x,
                        center_y: center.y,
                        radius,
                        diameter,
                    }
                }
                Some(Entity::Arc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                }) => {
                    let start_deg = start_angle.to_degrees();
                    let end_deg = end_angle.to_degrees();
                    if self.last_inspected_entity_id != Some(id_raw) {
                        self.prop_input_p1_x = format!("{:.2}", center.x);
                        self.prop_input_p1_y = format!("{:.2}", center.y);
                        self.prop_input_val_1 = format!("{:.2}", radius);
                        self.prop_input_val_2 = format!("{:.1}", start_deg);
                        self.prop_input_p2_x = format!("{:.1}", end_deg);
                        self.last_inspected_entity_id = Some(id_raw);
                    }
                    SelectedEntityData::Arc {
                        id_raw,
                        center_x: center.x,
                        center_y: center.y,
                        radius,
                        start_angle_deg: start_deg,
                        end_angle_deg: end_deg,
                    }
                }
                Some(Entity::Ellipse {
                    center,
                    radius_x,
                    radius_y,
                }) => {
                    if self.last_inspected_entity_id != Some(id_raw) {
                        self.prop_input_p1_x = format!("{:.2}", center.x);
                        self.prop_input_p1_y = format!("{:.2}", center.y);
                        self.prop_input_val_1 = format!("{:.2}", radius_x);
                        self.prop_input_val_2 = format!("{:.2}", radius_y);
                        self.last_inspected_entity_id = Some(id_raw);
                    }
                    SelectedEntityData::Ellipse {
                        id_raw,
                        center_x: center.x,
                        center_y: center.y,
                        radius_x,
                        radius_y,
                    }
                }
                None => {
                    self.last_inspected_entity_id = None;
                    SelectedEntityData::None
                }
            }
        } else if self.selected.len() > 1 {
            self.last_inspected_entity_id = None;
            SelectedEntityData::MultipleEntities {
                count: self.selected.len(),
            }
        } else {
            self.last_inspected_entity_id = None;
            SelectedEntityData::None
        };

        // Populate SelectedBodyData
        let selected_body_data = if self.selected_bodies.len() == 1 {
            let &bid = self.selected_bodies.iter().next().unwrap();
            let body_name = self
                .model
                .doc
                .bodies
                .get(bid)
                .map(|b| b.name.clone())
                .unwrap_or_else(|| "Solid Body".to_string());
            if let Some(geo) = self.model.geometry.get(bid) {
                let v_count = geo.mesh.positions.len();
                let t_count = geo.mesh.indices.len() / 3;
                let mut min_p = [f32::INFINITY; 3];
                let mut max_p = [f32::NEG_INFINITY; 3];
                for pos in &geo.mesh.positions {
                    for i in 0..3 {
                        min_p[i] = min_p[i].min(pos[i]);
                        max_p[i] = max_p[i].max(pos[i]);
                    }
                }
                let bbox_size = [
                    (max_p[0] - min_p[0]).abs().max(0.0),
                    (max_p[1] - min_p[1]).abs().max(0.0),
                    (max_p[2] - min_p[2]).abs().max(0.0),
                ];
                Some(SelectedBodyData {
                    id_raw: bid.data().as_ffi(),
                    name: body_name,
                    vertices_count: v_count,
                    triangles_count: t_count,
                    bbox_size,
                })
            } else {
                None
            }
        } else {
            None
        };

        // Tombol Toggle Buka Sidebar Kanan jika sedang tertutup/tersembunyi
        // (ditempatkan di tengah vertikal, sejajar dengan posisi panel saat terbuka)
        if !show_right_sidebar {
            egui::Area::new(egui::Id::new("cadraw-inspector-toggle-area"))
                .fixed_pos(egui::pos2(screen_rect.max.x - topbar_margin_right, screen_rect.center().y))
                .pivot(egui::Align2::RIGHT_CENTER)
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    let btn = egui::Button::new(egui::RichText::new("⚙ Properties").size(12.0).color(egui::Color32::from_rgb(220, 230, 242)))
                        .fill(egui::Color32::from_rgba_premultiplied(22, 27, 34, 235))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(48, 54, 61)))
                        .corner_radius(egui::CornerRadius::same(8));
                    if ui.add(btn).clicked() {
                        self.feature_inspector_open = true;
                        self.auto_hide_properties = false;
                    }
                });
        }

        // 7. Right Floating Feature Inspector (Tengah Vertikal, Tinggi Maksimum
        // Dibatasi dari Bawah ViewCube sampai Bawah Layar — Menyusut Jika Isi Sedikit)
        let inspector_top_bound = viewcube_y + 52.0;
        let inspector_bottom_margin = 12.0;
        let inspector_max_h = (screen_rect.max.y - inspector_top_bound - inspector_bottom_margin).max(120.0);
        if show_right_sidebar {
            let inspector_sig: InspectorContentSig = (
                std::mem::discriminant(&selected_entity_data),
                selected_body_data.is_some(),
                self.selected_bodies.len(),
                self.selected_edges.len(),
                self.selected_faces.len(),
                self.active_face.is_some(),
                self.pending_loft_bottom.is_some(),
                self.section_enabled,
                self.picking_mode,
                self.measurements.len(),
                measure_tool_active,
            );
            // Konten berubah dari frame sebelumnya (mis. ganti seleksi) -> paksa Area
            // mengukur ulang tingginya dari nol, alih-alih terjebak di tinggi lama
            // yang sudah ter-clip ScrollArea (lihat dok `InspectorContentSig`).
            let inspector_force_resize = self.inspector_content_sig != Some(inspector_sig);
            self.inspector_content_sig = Some(inspector_sig);

            let mut inspector_state = FeatureInspectorState {
                auto_hide_enabled: self.auto_hide_properties,
                selected_entity: selected_entity_data,
                selected_body: selected_body_data,
                selected_bodies_count: self.selected_bodies.len(),
                selected_edges_count: self.selected_edges.len(),
                selected_faces_count: self.selected_faces.len(),
                total_entities_count: self.sketch().entities.len(),
                total_bodies_count: self.model.doc.bodies.len(),

                entity_p1_x: self.prop_input_p1_x.clone(),
                entity_p1_y: self.prop_input_p1_y.clone(),
                entity_p2_x: self.prop_input_p2_x.clone(),
                entity_p2_y: self.prop_input_p2_y.clone(),
                entity_val_1: self.prop_input_val_1.clone(),
                entity_val_2: self.prop_input_val_2.clone(),

                extrude_input: self.extrude_distance_input.clone(),
                active_face_selected: self.active_face.is_some(),
                face_extrude_input: self.face_extrude_distance_input.clone(),
                loft_height_input: self.loft_height_input.clone(),
                loft_bottom_staged: self.pending_loft_bottom.is_some(),
                fillet_input: self.fillet_radius_input.clone(),
                chamfer_input: self.chamfer_distance_input.clone(),
                shell_input: self.shell_thickness_input.clone(),
                picking_mode: match self.picking_mode {
                    PickMode::None => InspectorPickMode::None,
                    PickMode::Edge => InspectorPickMode::Edge,
                    PickMode::Face => InspectorPickMode::Face,
                },
                can_undo_model: self.model_undo.can_undo(),
                can_redo_model: self.model_undo.can_redo(),
                status_message: self.model_status.clone(),
                section_enabled: self.section_enabled,
                section_axis: match self.section_axis {
                    SectionAxis::X => 0,
                    SectionAxis::Y => 1,
                    SectionAxis::Z => 2,
                },
                section_offset: self.section_offset,
                section_invert: self.section_invert,

                measurements: self.measurements.iter().map(|m| m.label()).collect(),
                measurement_tool_active: measure_tool_active,
                show_all_dimensions: self.show_all_dimensions,

                max_panel_height: inspector_max_h,
            };

            egui::Area::new(egui::Id::new("cadraw-inspector-area"))
                .fixed_pos(egui::pos2(screen_rect.max.x - topbar_margin_right, screen_rect.center().y))
                .pivot(egui::Align2::RIGHT_CENTER)
                .constrain_to(screen_rect)
                // Beri batas ukur yang cukup lega (setinggi inspector_max_h) supaya
                // sizing_pass di atas benar-benar bisa mengukur konten sampai
                // setinggi itu, bukan cuma sampai 400px bawaan egui.
                .default_size(egui::vec2(264.0, inspector_max_h))
                .sizing_pass(inspector_force_resize)
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    if let Some(insp_ev) = FeatureInspector::show(ui, &mut inspector_state) {
                        self.prop_input_p1_x = inspector_state.entity_p1_x;
                        self.prop_input_p1_y = inspector_state.entity_p1_y;
                        self.prop_input_p2_x = inspector_state.entity_p2_x;
                        self.prop_input_p2_y = inspector_state.entity_p2_y;
                        self.prop_input_val_1 = inspector_state.entity_val_1;
                        self.prop_input_val_2 = inspector_state.entity_val_2;
                        self.face_extrude_distance_input = inspector_state.face_extrude_input;

                        match insp_ev {
                            InspectorEvent::CloseInspector => {
                                self.feature_inspector_open = false;
                            }
                            InspectorEvent::ToggleAutoHide => {
                                self.auto_hide_properties = !self.auto_hide_properties;
                            }
                            InspectorEvent::ToggleShowAllDimensions => {
                                self.show_all_dimensions = !self.show_all_dimensions;
                            }
                            InspectorEvent::UpdateEntityLine {
                                id_raw,
                                start_x,
                                start_y,
                                end_x,
                                end_y,
                            } => {
                                if let Some(&id) = self.selected.iter().find(|i| i.data().as_ffi() == id_raw) {
                                    let new_entity = Entity::Line {
                                        start: DVec2::new(start_x, start_y),
                                        end: DVec2::new(end_x, end_y),
                                    };
                                    self.execute_sketch_command(
                                        Box::new(UpdateEntity::new("Ubah Garis", id, new_entity)),
                                    );
                                }
                            }
                            InspectorEvent::UpdateEntityCircle {
                                id_raw,
                                center_x,
                                center_y,
                                radius,
                            } => {
                                if let Some(&id) = self.selected.iter().find(|i| i.data().as_ffi() == id_raw) {
                                    let new_entity = Entity::Circle {
                                        center: DVec2::new(center_x, center_y),
                                        radius,
                                    };
                                    self.execute_sketch_command(
                                        Box::new(UpdateEntity::new("Ubah Lingkaran", id, new_entity)),
                                    );
                                }
                            }
                            InspectorEvent::UpdateEntityArc {
                                id_raw,
                                center_x,
                                center_y,
                                radius,
                                start_angle_deg,
                                end_angle_deg,
                            } => {
                                if let Some(&id) = self.selected.iter().find(|i| i.data().as_ffi() == id_raw) {
                                    let new_entity = Entity::Arc {
                                        center: DVec2::new(center_x, center_y),
                                        radius,
                                        start_angle: start_angle_deg.to_radians(),
                                        end_angle: end_angle_deg.to_radians(),
                                    };
                                    self.execute_sketch_command(
                                        Box::new(UpdateEntity::new("Ubah Busur", id, new_entity)),
                                    );
                                }
                            }
                            InspectorEvent::UpdateEntityEllipse {
                                id_raw,
                                center_x,
                                center_y,
                                radius_x,
                                radius_y,
                            } => {
                                if let Some(&id) = self.selected.iter().find(|i| i.data().as_ffi() == id_raw) {
                                    let new_entity = Entity::Ellipse {
                                        center: DVec2::new(center_x, center_y),
                                        radius_x,
                                        radius_y,
                                    };
                                    self.execute_sketch_command(
                                        Box::new(UpdateEntity::new("Ubah Elips", id, new_entity)),
                                    );
                                }
                            }
                            InspectorEvent::ApplyConstraint(act) => {
                                let ids: Vec<EntityId> = self.selected.iter().copied().collect();
                                match act {
                                    InspectorConstraintAction::Horizontal => {
                                        if let [id] = ids.as_slice() {
                                            self.apply_constraint(Constraint::Horizontal { line: *id });
                                        }
                                    }
                                    InspectorConstraintAction::Vertical => {
                                        if let [id] = ids.as_slice() {
                                            self.apply_constraint(Constraint::Vertical { line: *id });
                                        }
                                    }
                                    InspectorConstraintAction::Parallel => {
                                        if let [a, b] = ids.as_slice() {
                                            self.apply_constraint(Constraint::Parallel { a: *a, b: *b });
                                        }
                                    }
                                    InspectorConstraintAction::Perpendicular => {
                                        if let [a, b] = ids.as_slice() {
                                            self.apply_constraint(Constraint::Perpendicular { a: *a, b: *b });
                                        }
                                    }
                                    InspectorConstraintAction::EqualLength => {
                                        if let [a, b] = ids.as_slice() {
                                            self.apply_constraint(Constraint::EqualLength { a: *a, b: *b });
                                        }
                                    }
                                    InspectorConstraintAction::EqualRadius => {
                                        if let [a, b] = ids.as_slice() {
                                            self.apply_constraint(Constraint::EqualRadius { a: *a, b: *b });
                                        }
                                    }
                                    InspectorConstraintAction::Tangent => {
                                        if let [a, b] = ids.as_slice() {
                                            self.apply_constraint(Constraint::Tangent { a: *a, b: *b });
                                        }
                                    }
                                    InspectorConstraintAction::Coincident => {
                                        self.set_tool(ToolKind::CoincidentPick);
                                    }
                                    InspectorConstraintAction::Fixed => {
                                        self.set_tool(ToolKind::FixedPick);
                                    }
                                    InspectorConstraintAction::Symmetric => {
                                        self.set_tool(ToolKind::SymmetricPick);
                                    }
                                }
                            }
                            InspectorEvent::UndoModel => {
                                self.model_undo.undo(&mut self.model);
                                self.selected_bodies.clear();
                            }
                            InspectorEvent::RedoModel => {
                                self.model_undo.redo(&mut self.model);
                                self.selected_bodies.clear();
                            }
                            InspectorEvent::ApplyExtrude { distance } => {
                                self.extrude_distance_input = distance.to_string();
                                self.extrude_selected();
                            }
                            InspectorEvent::ApplyFaceExtrude { distance } => {
                                self.face_extrude_distance_input = distance.to_string();
                                self.extrude_active_face(distance);
                            }
                            InspectorEvent::SketchOnFace => {
                                self.sketch_on_active_face();
                            }
                            InspectorEvent::ApplyRevolve => {
                                self.set_tool(ToolKind::Revolve);
                            }
                            InspectorEvent::StageLoftBottom => {
                                match model::build_profile_from_selection(self.sketch(), &self.selected) {
                                    Ok(profile) => {
                                        self.pending_loft_bottom = Some(profile);
                                        self.model_status = None;
                                    }
                                    Err(msg) => self.model_status = Some(msg),
                                }
                            }
                            InspectorEvent::ApplyLoft { height } => {
                                self.loft_height_input = height.to_string();
                                self.loft_selected();
                            }
                            InspectorEvent::ApplyBoolean(kind) => {
                                let (b_kind, label) = match kind {
                                    InspectorBooleanKind::Union => (BooleanKind::Union, "Union"),
                                    InspectorBooleanKind::Subtract => (BooleanKind::Subtract, "Subtract"),
                                    InspectorBooleanKind::Intersect => (BooleanKind::Intersect, "Intersect"),
                                };
                                self.boolean_selected(b_kind, label, label);
                            }
                            InspectorEvent::ToggleEdgePicking => {
                                self.picking_mode = if self.picking_mode == PickMode::Edge {
                                    PickMode::None
                                } else {
                                    PickMode::Edge
                                };
                            }
                            InspectorEvent::ResetEdgePicking => {
                                self.selected_edges.clear();
                            }
                            InspectorEvent::ApplyFillet { radius } => {
                                self.fillet_radius_input = radius.to_string();
                                self.fillet_selected_body();
                            }
                            InspectorEvent::ApplyChamfer { distance } => {
                                self.chamfer_distance_input = distance.to_string();
                                self.chamfer_selected_body();
                            }
                            InspectorEvent::ToggleFacePicking => {
                                self.picking_mode = if self.picking_mode == PickMode::Face {
                                    PickMode::None
                                } else {
                                    PickMode::Face
                                };
                            }
                            InspectorEvent::ApplyShell { thickness } => {
                                self.shell_thickness_input = thickness.to_string();
                                self.shell_selected_body();
                            }
                            InspectorEvent::DeleteSelectedBodies => {
                                self.delete_selected_bodies();
                            }
                            InspectorEvent::SectionViewChanged => {
                                self.section_enabled = inspector_state.section_enabled;
                                self.section_axis = match inspector_state.section_axis {
                                    0 => SectionAxis::X,
                                    1 => SectionAxis::Y,
                                    _ => SectionAxis::Z,
                                };
                                self.section_offset = inspector_state.section_offset;
                                self.section_invert = inspector_state.section_invert;
                            }
                            InspectorEvent::RemoveMeasurement(i) => {
                                if i < self.measurements.len() {
                                    self.measurements.remove(i);
                                }
                            }
                            InspectorEvent::ClearMeasurements => {
                                self.measurements.clear();
                            }
                        }
                    }
                });
        }

        // 11. Bottom Floating Status Pill
        let bottom_center = egui::pos2(screen_center_x, screen_rect.max.y);
        let sel_summary = if !self.selected.is_empty() {
            format!("{} entitas terpilih", self.selected.len())
        } else if !self.selected_bodies.is_empty() {
            format!("{} body terpilih", self.selected_bodies.len())
        } else {
            self.status_text()
        };
        let m_summary = self.measurements.last().map(|m| m.label());

        egui::Area::new(egui::Id::new("cadraw-hud-bottom-status-area"))
            .fixed_pos(bottom_center - egui::vec2(130.0, 48.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                if let Some(ev) = CanvasHud::show_bottom_status_pill(ui, &sel_summary, m_summary.as_deref()) {
                    if ev == CanvasHudEvent::OpenMeasurements {
                        self.set_tool(ToolKind::Measure);
                    }
                }
            });

        // 13. Command Palette Overlay
        let palette_actions = self.palette_actions();
        let palette_entries: Vec<(&str, &str)> = palette_actions
            .iter()
            .map(|(label, hint, _)| (label.as_str(), hint.as_str()))
            .collect();
        if let Some(idx) = self.palette.show(ctx, &palette_entries) {
            let action = palette_actions[idx].2;
            self.run_palette_action(ctx, action);
        }
    }
}

/// Callback egui_wgpu: jembatan per-frame ke SceneRenderer di
/// `callback_resources`.
struct ViewportCallback {
    view_proj: Mat4,
    eye: Vec3,
    sketch_plane: SketchPlane,
    overlay_lines: Vec<LineVertex>,
    body_positions: Vec<[f32; 3]>,
    body_normals: Vec<[f32; 3]>,
    body_colors: Vec<[f32; 4]>,
    body_indices: Vec<u32>,
    /// Section View (Fase 7) — lihat `CadrawApp::section_clip_plane`.
    clip_plane: Option<(Vec3, f32)>,
}

impl egui_wgpu::CallbackTrait for ViewportCallback {
    fn prepare(
        &self,
        device: &egui_wgpu::wgpu::Device,
        queue: &egui_wgpu::wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut egui_wgpu::wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<egui_wgpu::wgpu::CommandBuffer> {
        if let Some(scene) = resources.get_mut::<SceneRenderer>() {
            scene.set_grid_plane(device, &self.sketch_plane);
            scene.set_overlay_lines(device, &self.overlay_lines);
            scene.set_mesh(
                device,
                &self.body_positions,
                &self.body_normals,
                Some(&self.body_colors),
                &self.body_indices,
            );
            scene.set_clip_plane(self.clip_plane);
            scene.prepare(queue, self.view_proj, self.eye);
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        rpass: &mut egui_wgpu::wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(scene) = resources.get::<SceneRenderer>() {
            scene.paint(rpass);
        }
    }
}
