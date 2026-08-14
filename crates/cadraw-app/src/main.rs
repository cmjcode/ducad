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
//! Lingkup yang sengaja belum digarap (bukan lupa — lihat docs/PLAN.md):
//! spline, fillet 2D, extend, offset untuk Ellipse, toleransi snap adaptif
//! mouse-vs-sentuh presisi, interaksi drag-satu-gesture, browser/penghapus
//! constraint selain lewat Undo, constraint pada titik ujung Arc (PointRef
//! belum mencakupnya), Tangent Line-Line (tak masuk akal secara geometris),
//! Revolve/sweep/loft, boolean intersect, sketch-on-face, picking body/face
//! 3D lewat klik viewport (body dipilih dari daftar di panel Model),
//! fillet/chamfer per-tepi individual (baru "semua tepi sekaligus"),
//! radial menu untuk konteks selain ganti tool (mis. aksi Model 3D),
//! deteksi tema sistem otomatis.

mod model;

use std::collections::HashSet;

use cadraw_core::BodyId;
use cadraw_render::{sketch as sketch_render, LineVertex, OrbitCamera, SceneRenderer};
use cadraw_sketch::constraint::{self, AddConstraint, Constraint};
use cadraw_sketch::{
    arc_from_three_points, find_snap, line_intersection_params_in_sketch, mirror_entity,
    offset_entity, project_t, trim_segments, DeleteEntities, Entity, EntityId, InsertEntities,
    ReplaceEntities, Sketch, SnapHit,
};
use cadraw_ui::{CommandPalette, RadialMenu, ThemeMode};
use eframe::egui;
use glam::{DVec2, Mat4, Vec3};
use model::{AddSolidCommand, BodyGeometry, BooleanCommand, BooleanKind, DeleteBodyCommand, ModelDoc, ReplaceGeometryCommand};

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
const KEYBOARD_SHORTCUTS: [(&str, &str); 13] = [
    ("L", "Tool Garis"),
    ("R", "Tool Persegi"),
    ("C", "Tool Lingkaran"),
    ("E", "Tool Ellips"),
    ("A", "Tool Arc"),
    ("O", "Tool Offset"),
    ("M", "Tool Mirror"),
    ("T", "Tool Trim"),
    ("Esc", "Batal titik pending, atau kembali ke tool Pilih"),
    ("Delete / Backspace", "Hapus seleksi"),
    ("Ctrl/Cmd+Z", "Undo sketch"),
    ("Ctrl/Cmd+Shift+Z atau Ctrl+Y", "Redo sketch"),
    ("Ctrl/Cmd+K", "Buka/tutup command palette"),
];

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
}

/// Berapa titik yang dibutuhkan tool sebelum di-commit lewat
/// `CadrawApp::finish_multipoint`. Offset/Trim/CoincidentPick/FixedPick/
/// SymmetricPick ditangani jalur terpisah (bergantung entitas/PointRef
/// yang diklik, bukan sekadar koordinat titik).
fn required_points(tool: ToolKind) -> usize {
    match tool {
        ToolKind::Line | ToolKind::Rectangle | ToolKind::Circle | ToolKind::Ellipse
        | ToolKind::Mirror => 2,
        ToolKind::Arc => 3,
        ToolKind::Select
        | ToolKind::Offset
        | ToolKind::Trim
        | ToolKind::CoincidentPick
        | ToolKind::FixedPick
        | ToolKind::SymmetricPick => 0,
    }
}

struct CadrawApp {
    camera: OrbitCamera,

    sketch: Sketch,
    undo: cadraw_sketch::UndoStack,

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

    /// Input teks bebas untuk nilai constraint dimensional (Panjang,
    /// Radius, Sudut) di panel Constraint.
    constraint_value_input: String,
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
            sketch: Sketch::default(),
            undo: cadraw_sketch::UndoStack::default(),
            tool: ToolKind::Select,
            pending_points: Vec::new(),
            pending_point_refs: Vec::new(),
            offset_source: None,
            hovered: None,
            selected: HashSet::new(),
            last_snap: None,
            dynamic_input: String::new(),
            dynamic_focus_pending: false,
            constraint_value_input: String::new(),
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

            theme,
            palette: CommandPalette::default(),
            radial_menu: RadialMenu::default(),
            radial_press: None,
            radial_suppress_click: false,
        }
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
            .find(|id| matches!(self.sketch.entities.get(*id), Some(Entity::Line { .. })))
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
                    .filter_map(|id| self.sketch.entities.get(*id))
                    .filter_map(|e| mirror_entity(e, axis_a, axis_b))
                    .collect();
                (!mirrored.is_empty())
                    .then(|| Box::new(InsertEntities::new("Cerminkan", mirrored)) as _)
            }
            ToolKind::Select
            | ToolKind::Offset
            | ToolKind::Trim
            | ToolKind::CoincidentPick
            | ToolKind::FixedPick
            | ToolKind::SymmetricPick => None,
        };
        if let Some(cmd) = cmd {
            self.undo.execute(cmd, &mut self.sketch);
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
    }

    /// Menu "⚙ Pengaturan" di toolbar: tema, pembuka command palette, dan
    /// referensi pintasan keyboard — dikumpulkan di satu dropdown alih-alih
    /// jadi tombol lepas di toolbar utama, karena ketiganya jarang disentuh
    /// lebih dari sekali per sesi (beda dengan tool sketch yang dipakai
    /// terus-menerus).
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
            ("Pilih".to_string(), String::new(), PaletteAction::SetTool(ToolKind::Select)),
            ("Garis".to_string(), "L".to_string(), PaletteAction::SetTool(ToolKind::Line)),
            ("Persegi".to_string(), "R".to_string(), PaletteAction::SetTool(ToolKind::Rectangle)),
            ("Lingkaran".to_string(), "C".to_string(), PaletteAction::SetTool(ToolKind::Circle)),
            ("Ellips".to_string(), "E".to_string(), PaletteAction::SetTool(ToolKind::Ellipse)),
            ("Arc".to_string(), "A".to_string(), PaletteAction::SetTool(ToolKind::Arc)),
            ("Offset".to_string(), "O".to_string(), PaletteAction::SetTool(ToolKind::Offset)),
            ("Mirror".to_string(), "M".to_string(), PaletteAction::SetTool(ToolKind::Mirror)),
            ("Trim".to_string(), "T".to_string(), PaletteAction::SetTool(ToolKind::Trim)),
            ("Coincident (titik)".to_string(), String::new(), PaletteAction::SetTool(ToolKind::CoincidentPick)),
            ("Fixed (titik)".to_string(), String::new(), PaletteAction::SetTool(ToolKind::FixedPick)),
            ("Symmetric (titik)".to_string(), String::new(), PaletteAction::SetTool(ToolKind::SymmetricPick)),
            ("Undo Sketch".to_string(), "⌘Z".to_string(), PaletteAction::Undo),
            ("Redo Sketch".to_string(), "⌘⇧Z".to_string(), PaletteAction::Redo),
            ("Undo Model".to_string(), String::new(), PaletteAction::ModelUndo),
            ("Redo Model".to_string(), String::new(), PaletteAction::ModelRedo),
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
        actions
    }

    /// Eksekusi satu `PaletteAction` — dipanggil dari `update()` saat
    /// command palette mengembalikan index terpilih.
    fn run_palette_action(&mut self, ctx: &egui::Context, action: PaletteAction) {
        match action {
            PaletteAction::SetTool(kind) => self.set_tool(kind),
            PaletteAction::Undo => {
                self.undo.undo(&mut self.sketch);
            }
            PaletteAction::Redo => {
                self.undo.redo(&mut self.sketch);
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
                    self.undo
                        .execute(Box::new(DeleteEntities::new(ids)), &mut self.sketch);
                }
            }
            PaletteAction::ToggleTheme => {
                self.theme = self.theme.toggled();
                cadraw_ui::apply_theme(ctx, self.theme);
            }
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
        };
        match &self.last_snap {
            Some(snap) => format!("{hint}  ·  snap: {:?}", snap.kind),
            None => hint,
        }
    }

    fn viewport(&mut self, ui: &mut egui::Ui) {
        let (rect, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());

        let raw_cursor = response
            .hover_pos()
            .and_then(|p| screen_to_plane_point(&self.camera, rect, p));

        self.handle_radial_menu(ui, &response);

        // Orbit primer hanya untuk tool Pilih, dan cuma saat radial menu
        // TIDAK terbuka/sedang dideteksi lewat long-press (radial memakai
        // drag primer yang sama untuk memilih slice — kalau orbit ikut
        // jalan, kamera akan berputar liar saat pengguna menggeser jari ke
        // arah slice). Orbit tetap tersedia lewat drag tengah / dua jari di
        // semua tool & kapanpun.
        let radial_active = self.radial_menu.is_open() || self.radial_press.is_some();
        self.handle_navigation(ui, &response, rect, self.tool == ToolKind::Select && !radial_active);
        self.handle_sketch_input(ui, &response, rect, raw_cursor);

        let aspect = rect.width() / rect.height().max(1.0);
        let overlay = self.build_overlay_lines(raw_cursor);
        let (body_positions, body_normals, body_indices) = self.build_combined_body_mesh();
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            ViewportCallback {
                view_proj: self.camera.view_proj(aspect),
                eye: self.camera.eye(),
                overlay_lines: overlay,
                body_positions,
                body_normals,
                body_indices,
            },
        ));

        self.dynamic_input_ui(ui, rect);
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
            && !modifiers.shift)
            || (response.dragged_by(egui::PointerButton::Middle) && !modifiers.shift);
        let panning = response.dragged_by(egui::PointerButton::Secondary)
            || (modifiers.shift
                && (response.dragged_by(egui::PointerButton::Primary)
                    || response.dragged_by(egui::PointerButton::Middle)));

        if panning {
            self.camera.pan(delta.x, delta.y, rect.height());
        } else if orbiting {
            self.camera.orbit(delta.x, delta.y);
        }

        if response.hovered() {
            let pinch = ui.input(|i| i.zoom_delta());
            if pinch != 1.0 {
                self.camera.zoom(pinch);
            }
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.camera.zoom((scroll * 0.003).exp());
            }
        }

        // Dua jari selalu navigasi, terlepas dari tool aktif (gaya Shapr3D:
        // satu jari menggambar/memilih, dua jari mengarahkan kamera).
        if let Some(touch) = ui.input(|i| i.multi_touch()) {
            self.camera
                .orbit(touch.translation_delta.x, touch.translation_delta.y);
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
        let text_focused = ui.ctx().memory(|m| m.focused().is_some());

        if !text_focused {
            if !self.selected.is_empty()
                && ui.input(|i| {
                    i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)
                })
            {
                let ids: Vec<_> = self.selected.drain().collect();
                self.undo
                    .execute(Box::new(DeleteEntities::new(ids)), &mut self.sketch);
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
                } else {
                    self.set_tool(ToolKind::Select);
                }
            }
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
        }

        // Konsumsi flag radial SEKALI per frame, terlepas dari apakah tool
        // aktif Select (satu-satunya tool yang bisa memicu radial) --
        // supaya tidak pernah bocor jadi menempel di frame/klik berikutnya
        // kalau frame ini kebetulan tidak melewati cabang yang memakainya.
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
                self.hovered = response
                    .hovered()
                    .then(|| self.sketch.hit_test(raw, tol))
                    .flatten();
                if response.clicked() && !suppress_click_from_radial {
                    let shift = ui.input(|i| i.modifiers.shift);
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
                    self.constraint_status = None;
                }
            }
            ToolKind::Line | ToolKind::Rectangle | ToolKind::Circle | ToolKind::Ellipse
            | ToolKind::Arc => {
                self.hovered = None;
                self.last_snap = response
                    .hovered()
                    .then(|| find_snap(&self.sketch, raw, tol, grid_step, None))
                    .flatten();
                if response.clicked() {
                    let effective = self.snapped_or(raw);
                    self.on_click_point(effective);
                }
            }
            ToolKind::Mirror => {
                self.hovered = None;
                self.last_snap = None;
                if !self.selected.is_empty() {
                    self.last_snap = response
                        .hovered()
                        .then(|| find_snap(&self.sketch, raw, tol, grid_step, None))
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
                            .then(|| self.sketch.hit_test(raw, tol))
                            .flatten();
                        if response.clicked() {
                            self.offset_source = self.hovered;
                        }
                    }
                    Some(source_id) => {
                        self.hovered = None;
                        if response.clicked() {
                            if let Some(entity) = self.sketch.entities.get(source_id) {
                                if let Some(new_entity) = offset_entity(entity, raw) {
                                    self.undo.execute(
                                        Box::new(InsertEntities::new("Offset", vec![new_entity])),
                                        &mut self.sketch,
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
                // Dibatasi ke entitas Line saja (Fase 1 lanjutan): hit_test
                // global bisa saja menemukan entitas non-Line lebih dekat
                // lalu difilter di sini, jadi kadang tidak memilih Line
                // terdekat kalau ada entitas jenis lain yang lebih dekat —
                // batasan kecil yang bisa disempurnakan nanti (hit-test
                // khusus per-jenis) jika terasa mengganggu.
                self.hovered = response
                    .hovered()
                    .then(|| self.sketch.hit_test(raw, tol))
                    .flatten()
                    .filter(|id| matches!(self.sketch.entities.get(*id), Some(Entity::Line { .. })));
                if response.clicked() {
                    if let Some(id) = self.hovered {
                        if let Some(Entity::Line { start, end }) =
                            self.sketch.entities.get(id).cloned()
                        {
                            let click_t = project_t(start, end, raw).clamp(0.0, 1.0);
                            let cuts =
                                line_intersection_params_in_sketch(&self.sketch, (start, end), id);
                            let remaining = trim_segments(start, end, &cuts, click_t);
                            let new_lines = remaining
                                .into_iter()
                                .map(|(s, e)| Entity::Line { start: s, end: e })
                                .collect();
                            self.undo.execute(
                                Box::new(ReplaceEntities::new("Trim", vec![id], new_lines)),
                                &mut self.sketch,
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
                    .then(|| find_snap(&self.sketch, raw, tol, grid_step, None))
                    .flatten();
                if response.clicked() {
                    if let Some(source) = self.last_snap.and_then(|s| s.source) {
                        self.pending_point_refs.push(source);
                        if self.pending_point_refs.len() >= 2 {
                            let refs = std::mem::take(&mut self.pending_point_refs);
                            self.apply_constraint(Constraint::Coincident { a: refs[0], b: refs[1] });
                        }
                    }
                    // Klik tanpa sumber titik valid (mis. snap ke midpoint/
                    // intersection/grid, atau tak ada snap sama sekali)
                    // sengaja diabaikan — bukan crash, cuma tak berefek.
                }
            }
            ToolKind::FixedPick => {
                self.hovered = None;
                self.last_snap = response
                    .hovered()
                    .then(|| find_snap(&self.sketch, raw, tol, grid_step, None))
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
                        .then(|| find_snap(&self.sketch, raw, tol, grid_step, Some(axis_id)))
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

    fn build_overlay_lines(&self, raw_cursor: Option<DVec2>) -> Vec<LineVertex> {
        let mut verts = sketch_render::entity_lines(&self.sketch, self.hovered, &self.selected);

        // Offset: sumber tetap ditandai sebagai preview walau hover pindah.
        if self.tool == ToolKind::Offset {
            if let Some(entity) = self.offset_source.and_then(|id| self.sketch.entities.get(id)) {
                verts.extend(sketch_render::preview_lines(entity));
            }
        }

        // Coincident/Symmetric: tandai titik yang sudah diklik (beda warna
        // dari glyph snap oranye supaya tak tertukar dengan hover kursor).
        if matches!(self.tool, ToolKind::CoincidentPick | ToolKind::SymmetricPick) {
            for pr in &self.pending_point_refs {
                if let Some(p) = constraint::point_ref_position(&self.sketch, pr) {
                    verts.extend(sketch_render::picked_point_glyph(p));
                }
            }
        }

        if let Some(raw) = raw_cursor {
            match self.tool {
                ToolKind::Line if self.pending_points.len() == 1 => {
                    let preview = Entity::Line {
                        start: self.pending_points[0],
                        end: self.snapped_or(raw),
                    };
                    verts.extend(sketch_render::preview_lines(&preview));
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
                        verts.extend(sketch_render::preview_lines(&preview));
                    }
                }
                ToolKind::Circle if self.pending_points.len() == 1 => {
                    let first = self.pending_points[0];
                    let effective = self.snapped_or(raw);
                    let preview = Entity::Circle {
                        center: first,
                        radius: (effective - first).length(),
                    };
                    verts.extend(sketch_render::preview_lines(&preview));
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
                        verts.extend(sketch_render::preview_lines(&preview));
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
                            verts.extend(sketch_render::preview_lines(&preview));
                        }
                        2 => {
                            if let Some(preview) = arc_from_three_points(
                                self.pending_points[0],
                                self.pending_points[1],
                                effective,
                            ) {
                                verts.extend(sketch_render::preview_lines(&preview));
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
                    verts.extend(sketch_render::preview_lines(&axis_preview));
                    for entity in self
                        .selected
                        .iter()
                        .filter_map(|id| self.sketch.entities.get(*id))
                    {
                        if let Some(mirrored) = mirror_entity(entity, axis_a, axis_b) {
                            verts.extend(sketch_render::preview_lines(&mirrored));
                        }
                    }
                }
                ToolKind::Offset => {
                    if let Some(entity) =
                        self.offset_source.and_then(|id| self.sketch.entities.get(id))
                    {
                        if let Some(preview) = offset_entity(entity, raw) {
                            verts.extend(sketch_render::preview_lines(&preview));
                        }
                    }
                }
                ToolKind::Trim => {
                    if let Some(id) = self.hovered {
                        if let Some((a, b)) = trim_removal_preview(&self.sketch, id, raw) {
                            verts.extend(sketch_render::removal_preview_lines(a, b));
                        }
                    }
                }
                _ => {}
            }
        }

        if let Some(hit) = &self.last_snap {
            verts.extend(sketch_render::snap_glyph(hit));
        }

        verts
    }

    /// Kotak input mengambang di dekat kursor untuk mengetik panjang
    /// (Garis) / radius (Lingkaran) / sisi (Persegi) — dynamic input gaya
    /// AutoCAD. Belum tersedia untuk Ellips/Arc/Offset/Mirror/Trim (lihat
    /// keterbatasan Fase 1 lanjutan di docs/PLAN.md).
    fn dynamic_input_ui(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        let supports_dynamic_input =
            matches!(self.tool, ToolKind::Line | ToolKind::Rectangle | ToolKind::Circle);
        if !supports_dynamic_input || self.pending_points.len() != 1 {
            return;
        }
        let first = self.pending_points[0];
        let Some(cursor) = ui.input(|i| i.pointer.hover_pos()) else {
            return;
        };

        egui::Area::new(egui::Id::new("cadraw-dynamic-input"))
            .fixed_pos(cursor + egui::vec2(16.0, 16.0))
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    let label = match self.tool {
                        ToolKind::Line => "Panjang (mm)",
                        ToolKind::Circle => "Radius (mm)",
                        ToolKind::Rectangle => "Sisi (mm)",
                        _ => "",
                    };
                    ui.horizontal(|ui| {
                        ui.label(label);
                        let resp = ui.text_edit_singleline(&mut self.dynamic_input);
                        if self.dynamic_focus_pending {
                            resp.request_focus();
                            self.dynamic_focus_pending = false;
                        }
                        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            if let Ok(value) = self.dynamic_input.trim().parse::<f64>() {
                                if let Some(raw) = screen_to_plane_point(&self.camera, rect, cursor)
                                {
                                    let dir = (raw - first).normalize_or_zero();
                                    let dir = if dir == DVec2::ZERO { DVec2::X } else { dir };
                                    self.on_click_point(first + dir * value);
                                }
                            }
                        }
                    });
                });
            });
    }

    /// Coba terapkan `constraint`: dry-run solve di atas clone sketch dulu
    /// (termasuk constraint yang sudah ada + yang baru), baru dikirim ke
    /// undo stack kalau konvergen. Sketch nyata tidak tersentuh sama
    /// sekali kalau gagal — hanya `constraint_status` terisi pesan error.
    fn apply_constraint(&mut self, new_constraint: Constraint) {
        let mut trial = self.sketch.clone();
        trial.constraints.push(new_constraint.clone());
        let snapshot = trial.constraints.clone();
        let result = constraint::solve(&mut trial, &snapshot);

        if result.converged {
            self.undo
                .execute(Box::new(AddConstraint::new(new_constraint)), &mut self.sketch);
            self.constraint_status = None;
        } else {
            self.constraint_status = Some(format!(
                "Constraint gagal diselesaikan (sisa residual {:.4}) — dibatalkan, sketch tidak berubah",
                result.final_residual_norm
            ));
        }
    }

    /// Panel kontekstual (kanan layar): muncul saat tool Pilih aktif dan
    /// ada 1-2 entitas terpilih. Coincident/Fixed sengaja tidak ditawarkan
    /// di sini — keduanya butuh seleksi titik individual (endpoint/center)
    /// yang belum ada infrastruktur UI-nya (lihat docs/PLAN.md).
    fn constraint_panel(&mut self, ctx: &egui::Context) {
        if self.tool != ToolKind::Select || self.selected.is_empty() {
            return;
        }
        let ids: Vec<EntityId> = self.selected.iter().copied().collect();

        egui::SidePanel::right("constraints")
            .resizable(false)
            .min_width(220.0)
            .show(ctx, |ui| {
                ui.heading("Constraint");
                ui.separator();
                match ids.as_slice() {
                    [a] => self.constraint_buttons_single(ui, *a),
                    [a, b] => self.constraint_buttons_pair(ui, *a, *b),
                    _ => {
                        ui.label("Pilih 1 atau 2 entitas untuk memasang constraint.");
                    }
                }
                if let Some(msg) = self.constraint_status.clone() {
                    ui.separator();
                    ui.colored_label(egui::Color32::from_rgb(230, 90, 90), msg);
                }
            });
    }

    fn constraint_buttons_single(&mut self, ui: &mut egui::Ui, id: EntityId) {
        let Some(entity) = self.sketch.entities.get(id).cloned() else {
            return;
        };
        match entity {
            Entity::Line { .. } => {
                ui.horizontal(|ui| {
                    if ui.button("Horizontal").clicked() {
                        self.apply_constraint(Constraint::Horizontal { line: id });
                    }
                    if ui.button("Vertikal").clicked() {
                        self.apply_constraint(Constraint::Vertical { line: id });
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Panjang (mm):");
                    ui.text_edit_singleline(&mut self.constraint_value_input);
                    if ui.button("Terapkan").clicked() {
                        if let Ok(value) = self.constraint_value_input.trim().parse::<f64>() {
                            self.apply_constraint(Constraint::Distance {
                                a: constraint::PointRef::LineStart(id),
                                b: constraint::PointRef::LineEnd(id),
                                value,
                            });
                        }
                    }
                });
            }
            Entity::Circle { .. } | Entity::Arc { .. } => {
                ui.horizontal(|ui| {
                    ui.label("Radius (mm):");
                    ui.text_edit_singleline(&mut self.constraint_value_input);
                    if ui.button("Terapkan").clicked() {
                        if let Ok(value) = self.constraint_value_input.trim().parse::<f64>() {
                            self.apply_constraint(Constraint::Radius { entity: id, value });
                        }
                    }
                });
            }
            Entity::Ellipse { .. } => {
                ui.label("Constraint untuk Ellips belum didukung.");
            }
        }
    }

    fn constraint_buttons_pair(&mut self, ui: &mut egui::Ui, a: EntityId, b: EntityId) {
        let (ea, eb) = (
            self.sketch.entities.get(a).cloned(),
            self.sketch.entities.get(b).cloned(),
        );
        match (ea, eb) {
            (Some(Entity::Line { .. }), Some(Entity::Line { .. })) => {
                ui.horizontal(|ui| {
                    if ui.button("Sejajar").clicked() {
                        self.apply_constraint(Constraint::Parallel { a, b });
                    }
                    if ui.button("Tegak Lurus").clicked() {
                        self.apply_constraint(Constraint::Perpendicular { a, b });
                    }
                });
                if ui.button("Sama Panjang").clicked() {
                    self.apply_constraint(Constraint::EqualLength { a, b });
                }
                ui.horizontal(|ui| {
                    ui.label("Sudut (°):");
                    ui.text_edit_singleline(&mut self.constraint_value_input);
                    if ui.button("Terapkan").clicked() {
                        if let Ok(deg) = self.constraint_value_input.trim().parse::<f64>() {
                            self.apply_constraint(Constraint::Angle {
                                a,
                                b,
                                value: deg.to_radians(),
                            });
                        }
                    }
                });
            }
            (Some(Entity::Circle { .. } | Entity::Arc { .. }), Some(Entity::Circle { .. } | Entity::Arc { .. })) => {
                ui.horizontal(|ui| {
                    if ui.button("Sama Radius").clicked() {
                        self.apply_constraint(Constraint::EqualRadius { a, b });
                    }
                    if ui.button("Bersinggungan (Tangent)").clicked() {
                        self.apply_constraint(Constraint::Tangent { a, b });
                    }
                });
            }
            (Some(Entity::Line { .. }), Some(Entity::Circle { .. } | Entity::Arc { .. }))
            | (Some(Entity::Circle { .. } | Entity::Arc { .. }), Some(Entity::Line { .. })) => {
                if ui.button("Bersinggungan (Tangent)").clicked() {
                    self.apply_constraint(Constraint::Tangent { a, b });
                }
            }
            _ => {
                ui.label("Kombinasi entitas ini belum punya constraint yang didukung.");
            }
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
        let profile = match model::build_profile_from_selection(&self.sketch, &self.selected) {
            Ok(p) => p,
            Err(msg) => {
                self.model_status = Some(msg);
                return;
            }
        };
        match cadraw_kernel::extrude_profile(&profile, distance) {
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

    /// Union/Subtract dua body terpilih (butuh persis 2). Seleksi body
    /// dikosongkan setelah sukses — keduanya lenyap, digantikan 1 body
    /// hasil dengan `BodyId` baru (lihat catatan `model::BooleanCommand`).
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

    /// Fillet SEMUA tepi 1 body terpilih.
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
        match cadraw_kernel::fillet_all(&geo.shape, radius) {
            Ok(shape) => {
                let new_geo = BodyGeometry::from_shape(shape);
                self.model_undo.execute(
                    Box::new(ReplaceGeometryCommand::new("Fillet", id, new_geo)),
                    &mut self.model,
                );
                self.model_status = None;
            }
            Err(e) => self.model_status = Some(format!("Fillet gagal: {e}")),
        }
    }

    /// Chamfer SEMUA tepi 1 body terpilih.
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
        match cadraw_kernel::chamfer_all(&geo.shape, distance) {
            Ok(shape) => {
                let new_geo = BodyGeometry::from_shape(shape);
                self.model_undo.execute(
                    Box::new(ReplaceGeometryCommand::new("Chamfer", id, new_geo)),
                    &mut self.model,
                );
                self.model_status = None;
            }
            Err(e) => self.model_status = Some(format!("Chamfer gagal: {e}")),
        }
    }

    /// Shell/Hollow 1 body terpilih — buang face terjauh ke arah
    /// `shell_direction`, sisakan dinding setebal `shell_thickness_input`.
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
        match cadraw_kernel::shell_hollow(&geo.shape, thickness, self.shell_direction) {
            Ok(shape) => {
                let new_geo = BodyGeometry::from_shape(shape);
                self.model_undo.execute(
                    Box::new(ReplaceGeometryCommand::new("Shell", id, new_geo)),
                    &mut self.model,
                );
                self.model_status = None;
            }
            Err(e) => self.model_status = Some(format!("Shell gagal: {e}")),
        }
    }

    /// Hapus semua body terpilih (masing-masing 1 command undo-able).
    fn delete_selected_bodies(&mut self) {
        for id in std::mem::take(&mut self.selected_bodies) {
            self.model_undo
                .execute(Box::new(DeleteBodyCommand::new(id)), &mut self.model);
        }
    }

    /// Merge mesh semua body VISIBLE jadi satu buffer gabungan, siap
    /// diupload lewat `SceneRenderer::set_mesh` (satu draw call). Indeks
    /// tiap body digeser sebesar jumlah vertex yang sudah terkumpul.
    fn build_combined_body_mesh(&self) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<u32>) {
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut indices = Vec::new();
        for (id, body) in self.model.doc.bodies.iter() {
            if !body.visible {
                continue;
            }
            let Some(geo) = self.model.geometry.get(id) else {
                continue;
            };
            let offset = positions.len() as u32;
            positions.extend_from_slice(&geo.mesh.positions);
            normals.extend_from_slice(&geo.mesh.normals);
            indices.extend(geo.mesh.indices.iter().map(|i| i + offset));
        }
        (positions, normals, indices)
    }

    /// Panel Model (kanan layar): daftar body (klik pilih, checkbox
    /// visible, Ctrl/Cmd+klik multi-pilih) + tombol operasi. Muncul
    /// kapanpun (tidak bergantung tool sketch aktif) — beda dari panel
    /// Constraint yang cuma muncul saat tool Pilih + seleksi sketch.
    fn model_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("model")
            .resizable(false)
            .min_width(240.0)
            .show(ctx, |ui| {
                ui.heading("Model 3D");
                ui.separator();

                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(self.model_undo.can_undo(), egui::Button::new("↶ Undo Model"))
                        .clicked()
                    {
                        self.model_undo.undo(&mut self.model);
                        self.selected_bodies.clear();
                    }
                    if ui
                        .add_enabled(self.model_undo.can_redo(), egui::Button::new("↷ Redo Model"))
                        .clicked()
                    {
                        self.model_undo.redo(&mut self.model);
                        self.selected_bodies.clear();
                    }
                });
                ui.separator();

                ui.label("Body:");
                egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                    let ids: Vec<BodyId> = self.model.doc.bodies.keys().collect();
                    for id in ids {
                        let Some(body) = self.model.doc.bodies.get_mut(id) else {
                            continue;
                        };
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut body.visible, "");
                            let selected = self.selected_bodies.contains(&id);
                            if ui.selectable_label(selected, &body.name).clicked() {
                                let extend = ctx.input(|i| i.modifiers.command || i.modifiers.shift);
                                if !extend {
                                    self.selected_bodies.clear();
                                }
                                if !self.selected_bodies.remove(&id) {
                                    self.selected_bodies.insert(id);
                                }
                                self.model_status = None;
                            }
                        });
                    }
                    if self.model.doc.bodies.is_empty() {
                        ui.weak("(belum ada body — Extrude dari seleksi sketch)");
                    }
                });
                ui.separator();

                ui.label("Extrude dari seleksi sketch:");
                ui.horizontal(|ui| {
                    ui.label("Jarak (mm):");
                    ui.text_edit_singleline(&mut self.extrude_distance_input);
                    if ui.button("Extrude").clicked() {
                        self.extrude_selected();
                    }
                });
                ui.separator();

                ui.label(format!("Boolean ({} body terpilih):", self.selected_bodies.len()));
                ui.horizontal(|ui| {
                    if ui.button("Union").clicked() {
                        self.boolean_selected(BooleanKind::Union, "Union", "Union");
                    }
                    if ui.button("Subtract (A-B)").clicked() {
                        self.boolean_selected(BooleanKind::Subtract, "Subtract", "Subtract");
                    }
                });
                ui.separator();

                ui.label("Fillet semua tepi:");
                ui.horizontal(|ui| {
                    ui.label("Radius (mm):");
                    ui.text_edit_singleline(&mut self.fillet_radius_input);
                    if ui.button("Fillet").clicked() {
                        self.fillet_selected_body();
                    }
                });
                ui.label("Chamfer semua tepi:");
                ui.horizontal(|ui| {
                    ui.label("Jarak (mm):");
                    ui.text_edit_singleline(&mut self.chamfer_distance_input);
                    if ui.button("Chamfer").clicked() {
                        self.chamfer_selected_body();
                    }
                });
                ui.separator();

                ui.label("Shell / Hollow (buang face terjauh ke arah ini):");
                egui::ComboBox::from_id_salt("shell-direction")
                    .selected_text(format!("{:?}", self.shell_direction))
                    .show_ui(ui, |ui| {
                        for dir in [
                            cadraw_kernel::Direction::PosZ,
                            cadraw_kernel::Direction::NegZ,
                            cadraw_kernel::Direction::PosX,
                            cadraw_kernel::Direction::NegX,
                            cadraw_kernel::Direction::PosY,
                            cadraw_kernel::Direction::NegY,
                        ] {
                            ui.selectable_value(&mut self.shell_direction, dir, format!("{dir:?}"));
                        }
                    });
                ui.horizontal(|ui| {
                    ui.label("Tebal (mm):");
                    ui.text_edit_singleline(&mut self.shell_thickness_input);
                    if ui.button("Shell").clicked() {
                        self.shell_selected_body();
                    }
                });
                ui.separator();

                if ui
                    .add_enabled(!self.selected_bodies.is_empty(), egui::Button::new("Hapus Body Terpilih"))
                    .clicked()
                {
                    self.delete_selected_bodies();
                }

                if let Some(msg) = self.model_status.clone() {
                    ui.separator();
                    ui.colored_label(egui::Color32::from_rgb(230, 90, 90), msg);
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

/// Konversi posisi kursor layar → titik di bidang sketch (Z=0), lewat
/// unprojection ray kamera dan interseksi ray-bidang.
fn screen_to_plane_point(camera: &OrbitCamera, rect: egui::Rect, pos: egui::Pos2) -> Option<DVec2> {
    let aspect = rect.width() / rect.height().max(1.0);
    let inv = camera.view_proj(aspect).inverse();

    let ndc_x = ((pos.x - rect.min.x) / rect.width()) * 2.0 - 1.0;
    let ndc_y = 1.0 - ((pos.y - rect.min.y) / rect.height()) * 2.0;

    // Konvensi kedalaman wgpu (Mat4::perspective_rh): NDC z ∈ [0, 1].
    let p_near = inv.project_point3(Vec3::new(ndc_x, ndc_y, 0.0));
    let p_far = inv.project_point3(Vec3::new(ndc_x, ndc_y, 1.0));
    let dir = p_far - p_near;
    if dir.z.abs() < 1e-6 {
        return None; // ray sejajar bidang XY — tidak ada perpotongan berguna
    }
    let t = -p_near.z / dir.z;
    let hit = p_near + dir * t;
    Some(DVec2::new(hit.x as f64, hit.y as f64))
}

/// Perkiraan unit-dunia per piksel layar pada kedalaman target kamera —
/// dipakai mengonversi toleransi hit-test/snap dari piksel ke mm. Toleransi
/// adaptif mouse-vs-sentuh yang lebih presisi menyusul di Fase 4.
fn pixel_tolerance_to_world(camera: &OrbitCamera, rect: egui::Rect) -> f64 {
    let world_per_pixel =
        2.0 * camera.distance * (camera.fov_y * 0.5).tan() / rect.height().max(1.0);
    world_per_pixel as f64
}

impl eframe::App for CadrawApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong("CADRAW");
                ui.separator();
                self.tool_buttons(ui);
                ui.separator();
                if ui
                    .add_enabled(self.undo.can_undo(), egui::Button::new("↶ Undo"))
                    .clicked()
                {
                    self.undo.undo(&mut self.sketch);
                }
                if ui
                    .add_enabled(self.undo.can_redo(), egui::Button::new("↷ Redo"))
                    .clicked()
                {
                    self.undo.redo(&mut self.sketch);
                }
                ui.separator();
                self.settings_menu(ui);
            });
        });

        // Ctrl/Cmd+K: buka/tutup command palette. Dicek di sini (bukan
        // `handle_sketch_input`, yang menahan shortcut huruf tunggal saat
        // ada widget teks fokus) supaya tetap jalan walau fokus lagi ada
        // di kotak cari palette itu sendiri -- pola sama dengan Escape di
        // dalam `CommandPalette::show`.
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::K)) {
            self.palette.toggle();
        }
        let palette_actions = self.palette_actions();
        let palette_entries: Vec<(&str, &str)> = palette_actions
            .iter()
            .map(|(label, hint, _)| (label.as_str(), hint.as_str()))
            .collect();
        if let Some(idx) = self.palette.show(ctx, &palette_entries) {
            let action = palette_actions[idx].2;
            self.run_palette_action(ctx, action);
        }

        self.model_panel(ctx);
        self.constraint_panel(ctx);

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(self.status_text());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!(
                        "{} entitas · {} constraint · {} terpilih · {} body · kamera {:.0} mm",
                        self.sketch.entities.len(),
                        self.sketch.constraints.len(),
                        self.selected.len(),
                        self.model.doc.bodies.len(),
                        self.camera.distance,
                    ));
                });
            });
        });

        let undo_pressed =
            ctx.input(|i| i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::Z));
        let redo_pressed = ctx.input(|i| {
            i.modifiers.command
                && (i.key_pressed(egui::Key::Y) || (i.modifiers.shift && i.key_pressed(egui::Key::Z)))
        });
        if undo_pressed {
            self.undo.undo(&mut self.sketch);
        }
        if redo_pressed {
            self.undo.redo(&mut self.sketch);
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                self.viewport(ui);
            });
    }
}

/// Callback egui_wgpu: jembatan per-frame ke SceneRenderer di
/// `callback_resources`.
struct ViewportCallback {
    view_proj: Mat4,
    eye: Vec3,
    overlay_lines: Vec<LineVertex>,
    /// Mesh body 3D (Fase 3) sudah digabung jadi satu buffer lewat
    /// `CadrawApp::build_combined_body_mesh` — cuma body `visible` yang
    /// masuk sini.
    body_positions: Vec<[f32; 3]>,
    body_normals: Vec<[f32; 3]>,
    body_indices: Vec<u32>,
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
            scene.set_overlay_lines(device, &self.overlay_lines);
            scene.set_mesh(device, &self.body_positions, &self.body_normals, &self.body_indices);
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
