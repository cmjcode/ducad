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
//! didaftar di jendela "📏 Pengukuran". Panel "✂ Section View" di panel
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
use cadraw_kernel::{KernelMesh, KernelShape, PickRay};
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
    ToolbarTool, TopBar, TopBarEvent, TopBarFileOp, ViewCube, ViewCubeAction,
};
use eframe::egui;
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
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        depth_buffer: 32,
        viewport: egui::ViewportBuilder::default()
            .with_title("CADRAW")
            .with_inner_size([1440.0, 900.0]),
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

/// Satu tepi 3D terpilih lewat picking (Fase 8): `ray` dunia yang dipakai
/// klik (di-cast ULANG terhadap shape hasil `deep_clone` saat apply — lihat
/// desain `cadraw_kernel::PickRay`), plus `polyline` hasil pick SEKARANG
/// (di-cache di sini supaya highlight overlay tidak query kernel ulang tiap
/// frame render).
struct PickedEdge {
    ray: PickRay,
    polyline: Vec<(f64, f64, f64)>,
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
    gizmo_drag_start_y: f32,
    gizmo_dimension_editing: bool,
    gizmo_edit_input: String,
    gizmo_is_cutting: bool,
    gizmo_target_body: Option<BodyId>,
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

            import_worker: ImportWorker::spawn(),
            pending_imports: 0,

            left_toolbar: LeftToolbar::default(),
            items_drawer: ItemsDrawer::default(),
            viewcube: ViewCube::default(),
            feature_inspector_open: true,
            auto_hide_properties: true,
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
            gizmo_drag_start_y: 0.0,
            gizmo_dimension_editing: false,
            gizmo_edit_input: "20".to_string(),
            gizmo_is_cutting: false,
            gizmo_target_body: None,
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
                        let geo = BodyGeometry { shape, mesh };
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

    /// Cek apakah posisi mouse saat ini berada dekat dengan gizmo panah atau dasar profil
    fn check_near_gizmo(&self, rect: egui::Rect, hover_pos: Option<egui::Pos2>) -> bool {
        let Some(pos) = hover_pos else { return false; };
        let Some(c) = self.selected_closed_region_centroid() else { return false; };
        let z_top = if self.extruding_from_gizmo { self.gizmo_distance as f32 } else { 16.0 };
        let top_3d = self.active_plane.to_world(c, z_top);
        let bot_3d = self.active_plane.to_world(c, 0.0);
        let near_top = world_to_screen_pos(&self.camera, rect, top_3d).map_or(false, |s| s.distance(pos) < 32.0);
        let near_bot = world_to_screen_pos(&self.camera, rect, bot_3d).map_or(false, |s| s.distance(pos) < 32.0);
        near_top || near_bot
    }

    fn viewport(&mut self, ui: &mut egui::Ui) {
        let (rect, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());

        let raw_cursor = response
            .hover_pos()
            .and_then(|p| screen_to_plane_point(&self.camera, rect, p, &self.active_plane));

        self.handle_radial_menu(ui, &response);

        let is_near_gizmo = self.check_near_gizmo(rect, response.hover_pos());
        if is_near_gizmo || self.extruding_from_gizmo {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }

        // Orbit primer hanya untuk tool Pilih, dan cuma saat radial menu
        // TIDAK terbuka/sedang dideteksi lewat long-press dan mouse TIDAK di atas gizmo
        let radial_active = self.radial_menu.is_open() || self.radial_press.is_some();
        let allow_primary_orbit = self.tool == ToolKind::Select && !radial_active && !is_near_gizmo && !self.extruding_from_gizmo;
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
            && !self.extruding_from_gizmo)
            || (response.dragged_by(egui::PointerButton::Middle) && !modifiers.shift);
        let panning = response.dragged_by(egui::PointerButton::Secondary)
            || (modifiers.shift
                && !self.extruding_from_gizmo
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
                if !self.pending_points.is_empty()
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

                // 1. Interaksi Drag Gizmo Panah 2 Sisi (↕) jika ada profil tertutup terpilih
                if let Some(centroid) = self.selected_closed_region_centroid() {
                    let gizmo_3d = self.active_plane.to_world(
                        centroid,
                        if self.extruding_from_gizmo { self.gizmo_distance as f32 } else { 18.0 },
                    );
                    let base_3d = self.active_plane.to_world(centroid, 0.0);
                    let gizmo_s = world_to_screen_pos(&self.camera, rect, gizmo_3d);
                    let base_s = world_to_screen_pos(&self.camera, rect, base_3d);

                    if let Some(pos) = response.hover_pos() {
                        let near_gizmo = gizmo_s.map_or(false, |s| s.distance(pos) < 36.0);
                        let near_base = base_s.map_or(false, |s| s.distance(pos) < 36.0);

                        if response.drag_started() && (near_gizmo || near_base) {
                            self.extruding_from_gizmo = true;
                            self.gizmo_drag_start_y = pos.y;
                            if self.gizmo_distance == 0.0 {
                                self.gizmo_distance = 20.0;
                            }
                        }
                    }

                    if self.extruding_from_gizmo {
                        let delta = response.drag_delta();
                        let world_scale = pixel_tolerance_to_world(&self.camera, rect);
                        self.gizmo_distance += (-delta.y as f64) * world_scale * 1.6;

                        // Smart Boolean Cut vs Extrude Detection (Screenshot 3 & 4)
                        if let Ok(profile) = model::build_profile_from_selection(self.sketch(), &self.selected) {
                            if let Ok(swept) = self.extrude_profile_active_plane(&profile, self.gizmo_distance) {
                                let mut is_cutting = false;
                                for (b_id, b_geo) in self.model.geometry.iter() {
                                    if let Some(body) = self.model.doc.bodies.get(b_id) {
                                        if body.visible {
                                            if let Ok(cut_res) = cadraw_kernel::subtract(&b_geo.shape, &swept) {
                                                let orig_verts = b_geo.mesh.positions.len();
                                                let cut_verts = cut_res.tessellate().positions.len();
                                                if cut_verts > 0 && orig_verts > 0 {
                                                    is_cutting = true;
                                                    self.gizmo_is_cutting = true;
                                                    self.gizmo_target_body = Some(b_id);
                                                    break;
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

                        if response.drag_stopped() {
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
                                                    }
                                                }
                                            }
                                        } else {
                                            let geo = BodyGeometry::from_shape(swept);
                                            let cmd = AddSolidCommand::new("Extrude", geo);
                                            self.model_undo.execute(Box::new(cmd), &mut self.model);
                                        }
                                    }
                                }
                            }
                            self.extruding_from_gizmo = false;
                        }
                        return;
                    }
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

                if response.clicked() && !suppress_click_from_radial {
                    let shift = ui.input(|i| i.modifiers.shift);
                    if let Some(reg) = region_hit {
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
                        match (self.hovered, shift) {
                            (Some(hit), true) => {
                                if !self.selected.remove(&hit) {
                                    self.selected.insert(hit);
                                }
                            }
                            (Some(hit), false) => {
                                self.selected.clear();
                                self.selected.insert(hit);
                            }
                            (None, false) => self.selected.clear(),
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
                if cadraw_kernel::pick_face(&geo.shape, ray).is_some() {
                    self.selected_faces.push(ray);
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

        // Pengukuran (Fase 7) tergambar permanen
        for measurement in &self.measurements {
            verts.extend(sketch_render::measurement_lines(&measurement.points(), &self.active_plane));
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
                }
                _ => {}
            }
        }

        if let Some(hit) = &self.last_snap {
            verts.extend(sketch_render::snap_glyph(hit, &self.active_plane));
        }

        verts
    }

    /// Kotak input mengambang dan badge dimensi in-situ (Screenshot 1, 2, 3, 4)
    fn dynamic_input_ui(&mut self, ui: &mut egui::Ui, rect: egui::Rect, raw_cursor: Option<DVec2>) {
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
                _ => {}
            }
        }

        // 2. Interactive Draggable Double Arrow Handle & Dimension Pill di atas Gizmo (Screenshot 2, 3, 4)
        if let Some(centroid) = self.selected_closed_region_centroid() {
            let z_pos = if self.extruding_from_gizmo { self.gizmo_distance } else { 18.0 };
            let handle_3d = self.active_plane.to_world(centroid, z_pos as f32);

            if let Some(handle_2d) = world_to_screen_pos(&self.camera, rect, handle_3d) {
                // Handle panah 2 sisi tebal dan draggable
                let handle_resp = CanvasHud::render_draggable_double_arrow_handle(ui, handle_2d, self.extruding_from_gizmo);

                if handle_resp.drag_started() {
                    self.extruding_from_gizmo = true;
                    if self.gizmo_distance == 0.0 {
                        self.gizmo_distance = 20.0;
                    }
                }

                if handle_resp.dragged() {
                    self.extruding_from_gizmo = true;
                    let world_scale = pixel_tolerance_to_world(&self.camera, rect);
                    self.gizmo_distance += (-handle_resp.drag_delta().y as f64) * world_scale * 1.6;

                    // Smart Boolean Cut vs Extrude Detection (Screenshot 3 & 4)
                    if let Ok(profile) = model::build_profile_from_selection(self.sketch(), &self.selected) {
                        if let Ok(swept) = self.extrude_profile_active_plane(&profile, self.gizmo_distance) {
                            let mut is_cutting = false;
                            for (b_id, b_geo) in self.model.geometry.iter() {
                                if let Some(body) = self.model.doc.bodies.get(b_id) {
                                    if body.visible {
                                        if let Ok(cut_res) = cadraw_kernel::subtract(&b_geo.shape, &swept) {
                                            let orig_verts = b_geo.mesh.positions.len();
                                            let cut_verts = cut_res.tessellate().positions.len();
                                            if cut_verts > 0 && orig_verts > 0 {
                                                is_cutting = true;
                                                self.gizmo_is_cutting = true;
                                                self.gizmo_target_body = Some(b_id);
                                                break;
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

                if handle_resp.drag_stopped() {
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
                                            }
                                        }
                                    }
                                } else {
                                    let geo = BodyGeometry::from_shape(swept);
                                    let cmd = AddSolidCommand::new("Extrude", geo);
                                    self.model_undo.execute(Box::new(cmd), &mut self.model);
                                }
                            }
                        }
                    }
                    self.extruding_from_gizmo = false;
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
                                        if let Ok(profile) = model::build_profile_from_selection(self.sketch(), &self.selected) {
                                            if let Ok(swept) = self.extrude_profile_active_plane(&profile, self.gizmo_distance) {
                                                let geo = BodyGeometry::from_shape(swept);
                                                let cmd = AddSolidCommand::new("Extrude", geo);
                                                self.model_undo.execute(Box::new(cmd), &mut self.model);
                                            }
                                        }
                                    }
                                    self.gizmo_dimension_editing = false;
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
        match BooleanCommand::try_new(&self.model, kind, label, result_name, *a, *b) {
            Ok(cmd) => {
                self.model_undo.execute(Box::new(cmd), &mut self.model);
                self.selected_bodies.clear();
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
                self.selected_edges.clear();
                self.picking_mode = PickMode::None;
                self.model_status = None;
            }
            Err(e) => self.model_status = Some(format!("Fillet gagal: {e}")),
        }
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

    /// Hapus semua body terpilih (masing-masing 1 command undo-able).
    fn delete_selected_bodies(&mut self) {
        for id in std::mem::take(&mut self.selected_bodies) {
            self.model_undo
                .execute(Box::new(DeleteBodyCommand::new(id)), &mut self.model);
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

        // 1. Solid bodies normal
        for (id, geo) in self.model.geometry.iter() {
            if let Some(body) = self.model.doc.bodies.get(id) {
                if body.visible {
                    if self.extruding_from_gizmo && self.gizmo_is_cutting && self.gizmo_target_body == Some(id) {
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
        if self.extruding_from_gizmo && self.gizmo_distance.abs() > 0.01 {
            if let Ok(profile) = model::build_profile_from_selection(self.sketch(), &self.selected) {
                if let Ok(extruded_shape) = self.extrude_profile_active_plane(&profile, self.gizmo_distance) {
                    if self.gizmo_is_cutting {
                        if let Some(target_id) = self.gizmo_target_body {
                            if let Some(target_geo) = self.model.geometry.get(target_id) {
                                if let Ok(cut_shape) = cadraw_kernel::subtract(&target_geo.shape, &extruded_shape) {
                                    let cut_mesh = cut_shape.tessellate();
                                    let offset = positions.len() as u32;
                                    positions.extend_from_slice(&cut_mesh.positions);
                                    normals.extend_from_slice(&cut_mesh.normals);
                                    for _ in 0..cut_mesh.positions.len() {
                                        colors.push(CYAN_CUT_PREVIEW);
                                    }
                                    indices.extend(cut_mesh.indices.iter().map(|i| i + offset));
                                }
                            }
                        }
                    } else {
                        let preview_mesh = extruded_shape.tessellate();
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
                }
            }
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

    /// Panel "📏 Pengukuran" (Fase 7) — jendela mengambang kecil, cuma
    /// tampil kalau ada isinya ATAU tool Ukur/Ukur Sudut sedang aktif
    /// (supaya tidak menambah dua panel sisi permanen lagi di layar yang
    /// sudah punya panel Model + Constraint). Daftar bisa dihapus satu-satu
    /// (✕) atau semua sekaligus — non-destruktif, tidak menyentuh undo
    /// stack manapun (lihat `Measurement`).
    fn measurement_panel(&mut self, ctx: &egui::Context) {
        let tool_active = matches!(self.tool, ToolKind::Measure | ToolKind::MeasureAngle);
        if self.measurements.is_empty() && !tool_active {
            return;
        }
        egui::Window::new("📏 Pengukuran")
            .default_pos(egui::pos2(260.0, 80.0))
            .resizable(false)
            .show(ctx, |ui| {
                if self.measurements.is_empty() {
                    ui.weak("(belum ada — klik 2 titik untuk jarak, 3 titik untuk sudut)");
                }
                let mut remove_at: Option<usize> = None;
                for (i, m) in self.measurements.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(m.label());
                        if ui.small_button("✕").clicked() {
                            remove_at = Some(i);
                        }
                    });
                }
                if let Some(i) = remove_at {
                    self.measurements.remove(i);
                }
                if !self.measurements.is_empty() {
                    ui.separator();
                    if ui.button("Hapus Semua").clicked() {
                        self.measurements.clear();
                    }
                }
            });
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

        let screen_rect = ctx.screen_rect();
        let screen_center_x = screen_rect.center().x;

        // 4. Left Floating Toolbar (Pojok Kiri Atas)
        self.left_toolbar.section_view_active = self.section_enabled;
        self.left_toolbar.is_sketching = self.is_sketching;
        egui::Area::new(egui::Id::new("cadraw-left-toolbar-area"))
            .fixed_pos(egui::pos2(12.0, 12.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                if let Some(tb_ev) = self.left_toolbar.show(ui, self.tool.to_toolbar_tool(), self.active_plane.name()) {
                    match tb_ev {
                        ToolbarEvent::SelectTool(t) => {
                            self.set_tool(ToolKind::from_toolbar_tool(t));
                        }
                        ToolbarEvent::ToggleItemsDrawer => {
                            // Status drawer dikelola di dalam LeftToolbar
                        }
                        ToolbarEvent::OpenSearch => {
                            self.palette.open();
                        }
                        ToolbarEvent::SelectSketchPlane(idx) => {
                            let kind = match idx {
                                0 => PlaneKind::Top,
                                1 => PlaneKind::Front,
                                2 => PlaneKind::Right,
                                _ => PlaneKind::Top,
                            };
                            self.set_sketch_plane(kind);
                        }
                        ToolbarEvent::EnterSketching => {
                            self.is_sketching = true;
                            self.left_toolbar.is_sketching = true;
                            self.camera.orient_to_plane(&self.active_plane);
                        }
                        ToolbarEvent::ExitSketching => {
                            self.is_sketching = false;
                            self.left_toolbar.is_sketching = false;
                            self.set_tool(ToolKind::Select);
                        }
                        ToolbarEvent::ToggleSectionView => {
                            self.section_enabled = !self.section_enabled;
                        }
                        ToolbarEvent::ToggleMeasurements => {
                            self.set_tool(ToolKind::Measure);
                        }
                        ToolbarEvent::DeleteSelection => {
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

        // 5. Items Tree Drawer (Muncul di sebelah kanan toolbar saat dibuka)
        if self.left_toolbar.items_drawer_open {
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

            egui::Area::new(egui::Id::new("cadraw-items-drawer-area"))
                .fixed_pos(egui::pos2(68.0, 12.0))
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

        // 6. Modern Top Bar (Header Full Sampai Kanan)
        let topbar_margin_right = 12.0;
        let topbar_x = if self.left_toolbar.items_drawer_open { 312.0 } else { 70.0 };
        let topbar_w = (screen_rect.max.x - topbar_x - topbar_margin_right).max(200.0);
        let doc_name = self
            .current_file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled.cadraw")
            .to_string();
        let is_saved = self.current_file_path.is_some();

        egui::Area::new(egui::Id::new("cadraw-topbar-floating"))
            .fixed_pos(egui::pos2(topbar_x, 8.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.set_width(topbar_w);
                if let Some(top_event) = TopBar::show(ui, &doc_name, is_saved, self.model.doc.unit) {
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
                    }
                }
            });

        // 7. Right Properties & Features Inspector (Fixed di Kanan Kanvas, Sejajar Tepi Kanan dg Header)
        let is_editing_or_drawing = self.tool != ToolKind::Select;
        let has_active_selection = !self.selected.is_empty() || !self.selected_bodies.is_empty();
        let show_right_sidebar = if self.auto_hide_properties {
            !is_editing_or_drawing && has_active_selection && self.feature_inspector_open
        } else {
            self.feature_inspector_open
        };

        let inspector_outer_w = 260.0;
        let inspector_x = (screen_rect.max.x - topbar_margin_right - inspector_outer_w).max(180.0);
        let inspector_y = 56.0;
        let _inspector_h = (screen_rect.max.y - inspector_y - 12.0).max(200.0);

        // 8. Interactive 3D ViewCube (Otomatis Bergeser ke Kiri Sidebar Saat Sidebar Terbuka)
        let viewcube_y = 102.0;
        let viewcube_pos = if show_right_sidebar {
            egui::pos2(inspector_x - 52.0, viewcube_y)
        } else {
            egui::pos2(screen_rect.max.x - topbar_margin_right - 42.0, viewcube_y)
        };
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

        // Tombol Toggle Buka Sidebar Kanan jika sedang tertutup/tersembunyi (ditempatkan rapi di bawah ViewCube)
        if !show_right_sidebar {
            egui::Area::new(egui::Id::new("cadraw-inspector-toggle-area"))
                .fixed_pos(egui::pos2(screen_rect.max.x - topbar_margin_right - 88.0, 154.0))
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

        // 7. Right Floating Feature Inspector (Pojok Kanan Atas di bawah ViewCube)
        if show_right_sidebar {
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
            };

            egui::Area::new(egui::Id::new("cadraw-inspector-area"))
                .fixed_pos(egui::pos2(screen_rect.max.x - topbar_margin_right - 264.0, 154.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    if let Some(insp_ev) = FeatureInspector::show(ui, &mut inspector_state) {
                        self.prop_input_p1_x = inspector_state.entity_p1_x;
                        self.prop_input_p1_y = inspector_state.entity_p1_y;
                        self.prop_input_p2_x = inspector_state.entity_p2_x;
                        self.prop_input_p2_y = inspector_state.entity_p2_y;
                        self.prop_input_val_1 = inspector_state.entity_val_1;
                        self.prop_input_val_2 = inspector_state.entity_val_2;

                        match insp_ev {
                            InspectorEvent::CloseInspector => {
                                self.feature_inspector_open = false;
                            }
                            InspectorEvent::ToggleAutoHide => {
                                self.auto_hide_properties = !self.auto_hide_properties;
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

        // 12. Floating Measurement Window (jika ada pengukuran aktif)
        self.measurement_panel(ctx);

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
