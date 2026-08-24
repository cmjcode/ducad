//! In-Canvas HUD & Floating Dimension Pills bergaya Shapr3D dengan Material Icons.
//!
//! Menampilkan widget HUD mengambang langsung di atas kanvas 3D:
//! tombol kapsul "Normal to Sketch", banner peringatan Section View,
//! badge dimensi in-situ, dan status seleksi mengambang di pojok kiri atas kanvas.

use crate::theme::{
    pill_frame, ACCENT_BLUE, ACCENT_GREEN, ACCENT_ORANGE, BORDER_SUBTLE, TEXT_MUTED, TEXT_PRIMARY,
    TEXT_SECONDARY,
};
use ducad_i18n::t;
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Stroke, StrokeKind, Ui, Vec2};
use egui_material_icons::icons::{ICON_3D_ROTATION, ICON_CHECK, ICON_DRIVE_FILE_RENAME_OUTLINE, ICON_LOCK, ICON_STRAIGHTEN};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RevolveHudAction {
    SetAngle(f64),
    ToggleReverse,
    Commit,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SweepHudAction {
    Commit,
    ResetProfile,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoftHudAction {
    SetHeight(f64),
    AlignCentroids,
    DismissAlignmentDialog,
    ToggleFlip,
    Commit,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShellHudAction {
    SetThickness(f64),
    ToggleVariableMode,
    Commit,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RibHudAction {
    SetThickness(f64),
    SetDepth(f64),
    SetDraftAngle(f64),
    SetAngle(f64),
    Commit,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DraftHudAction {
    SetAngle(f64),
    SetPullDir(DraftPullDir),
    Commit,
    Cancel,
}

/// Preset arah pull cetakan yang umum dipakai.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftPullDir {
    /// Arah +Z (atas) — paling umum untuk cetakan vertikal.
    PosZ,
    /// Arah -Z (bawah).
    NegZ,
    /// Arah +Y (depan).
    PosY,
    /// Arah -Y (belakang).
    NegY,
    /// Arah +X (kanan).
    PosX,
    /// Arah -X (kiri).
    NegX,
}

impl DraftPullDir {
    pub fn label(&self) -> &'static str {
        match self {
            Self::PosZ => "+Z (Atas)",
            Self::NegZ => "-Z (Bawah)",
            Self::PosY => "+Y (Depan)",
            Self::NegY => "-Y (Belakang)",
            Self::PosX => "+X (Kanan)",
            Self::NegX => "-X (Kiri)",
        }
    }

    /// Konversi ke komponen vektor (x, y, z)
    pub fn to_vec(&self) -> (f64, f64, f64) {
        match self {
            Self::PosZ => (0.0, 0.0, 1.0),
            Self::NegZ => (0.0, 0.0, -1.0),
            Self::PosY => (0.0, 1.0, 0.0),
            Self::NegY => (0.0, -1.0, 0.0),
            Self::PosX => (1.0, 0.0, 0.0),
            Self::NegX => (-1.0, 0.0, 0.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SplitMode {
    #[default]
    SplitBody,
    SplitFace,
}

impl SplitMode {
    pub fn label(&self) -> String {
        match self {
            Self::SplitBody => t!("popup-split-mode-body"),
            Self::SplitFace => t!("popup-split-mode-face"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SplitPlaneKind {
    #[default]
    XY,
    XZ,
    YZ,
    PickedFace,
}

impl SplitPlaneKind {
    pub fn label(&self) -> String {
        match self {
            Self::XY => t!("popup-split-plane-xy"),
            Self::XZ => t!("popup-split-plane-xz"),
            Self::YZ => t!("popup-split-plane-yz"),
            Self::PickedFace => t!("popup-split-plane-face"),
        }
    }

    pub fn default_normal(&self) -> (f64, f64, f64) {
        match self {
            Self::XY => (0.0, 0.0, 1.0),
            Self::XZ => (0.0, 1.0, 0.0),
            Self::YZ => (1.0, 0.0, 0.0),
            Self::PickedFace => (0.0, 0.0, 1.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SplitHudAction {
    SetMode(SplitMode),
    SetPlane(SplitPlaneKind),
    SetOffset(f64),
    Commit,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BooleanHudAction {
    SelectOp(BooleanOpKind),
    Commit,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BooleanOpKind {
    Union,
    Subtract,
    Intersect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PatternKind {
    #[default]
    Linear,
    Circular,
}

impl PatternKind {
    pub fn label(&self) -> String {
        match self {
            Self::Linear => t!("pattern-mode-linear"),
            Self::Circular => t!("pattern-mode-circular"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PatternAxisPreset {
    #[default]
    Z,
    Y,
    X,
    Center,
}

impl PatternAxisPreset {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Z => "Sumbu Z (0,0,1)",
            Self::Y => "Sumbu Y (0,1,0)",
            Self::X => "Sumbu X (1,0,0)",
            Self::Center => "Pusat (0,0)",
        }
    }

    pub fn to_dir(&self) -> (f64, f64, f64) {
        match self {
            Self::Z | Self::Center => (0.0, 0.0, 1.0),
            Self::Y => (0.0, 1.0, 0.0),
            Self::X => (1.0, 0.0, 0.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PatternHudAction {
    SetKind(PatternKind),
    SetAxis(PatternAxisPreset),
    Commit,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasHudEvent {
    OrientNormalToSketch,
    TurnOffSectionView,
    OpenMeasurements,
}

/// Event dari popup rename yang muncul di area top HUD.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenamePopupEvent {
    /// User mengklik Simpan / tekan Enter — berisi nama baru.
    Confirm(String),
    /// User mengklik Batal / tekan Esc.
    Cancel,
}

/// Titik-titik poligon konveks rounded-rect berpusat di origin (belum
/// diputar/digeser) — dipakai `render_dimension_pill_aligned` karena
/// `Painter::rect_filled` (rounded-rect bawaan egui) tidak punya varian
/// berotasi. `segments_per_corner` mengatur kehalusan lengkungan tiap sudut
/// (6 sudah cukup mulus untuk ukuran pill sekecil ini).
fn rounded_rect_local_points(half: Vec2, radius: f32, segments_per_corner: usize) -> Vec<Pos2> {
    let r = radius.min(half.x).min(half.y).max(0.0);
    let centers_and_angles = [
        (
            Vec2::new(-half.x + r, -half.y + r),
            std::f32::consts::PI,
            1.5 * std::f32::consts::PI,
        ), // kiri-atas
        (
            Vec2::new(half.x - r, -half.y + r),
            1.5 * std::f32::consts::PI,
            2.0 * std::f32::consts::PI,
        ), // kanan-atas
        (
            Vec2::new(half.x - r, half.y - r),
            0.0,
            0.5 * std::f32::consts::PI,
        ), // kanan-bawah
        (
            Vec2::new(-half.x + r, half.y - r),
            0.5 * std::f32::consts::PI,
            std::f32::consts::PI,
        ), // kiri-bawah
    ];

    let mut points = Vec::with_capacity((segments_per_corner + 1) * 4);
    for (center, start_angle, end_angle) in centers_and_angles {
        for i in 0..=segments_per_corner {
            let t = i as f32 / segments_per_corner as f32;
            let angle = start_angle + (end_angle - start_angle) * t;
            let p = center + Vec2::new(angle.cos(), angle.sin()) * r;
            points.push(Pos2::new(p.x, p.y));
        }
    }
    points
}

pub struct CanvasHud;

impl CanvasHud {
    /// Render tombol kapsul mengambang "Normal to Sketch" di dalam container UI yang diberikan.
    pub fn show_normal_to_sketch_btn(ui: &mut Ui) -> Option<CanvasHudEvent> {
        let mut event = None;
        pill_frame().show(ui, |ui| {
            let btn = ui.button(
                RichText::new(format!("{} {}", ICON_3D_ROTATION.codepoint, t!("hud-normal-to-sketch")))
                    .size(12.0)
                    .strong()
                    .color(TEXT_PRIMARY),
            );
            if btn.clicked() {
                event = Some(CanvasHudEvent::OrientNormalToSketch);
            }
        });
        event
    }

    /// Render banner informasi Section View di dalam container UI yang diberikan.
    pub fn show_section_view_banner(ui: &mut Ui) -> Option<CanvasHudEvent> {
        let mut event = None;
        pill_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(t!("hud-section-banner"))
                        .size(11.0)
                        .color(ACCENT_ORANGE),
                );
                if ui
                    .small_button(RichText::new(t!("hud-turn-off")).size(10.0))
                    .clicked()
                {
                    event = Some(CanvasHudEvent::TurnOffSectionView);
                }
            });
        });
        event
    }

    /// Render status badge seleksi, aksi "Normal to Sketch", & pengukuran mengambang di dalam container UI yang diberikan.
    pub fn show_status_pill(
        ui: &mut Ui,
        selection_summary: &str,
        measurement_summary: Option<&str>,
        show_normal_to_sketch: bool,
    ) -> Option<CanvasHudEvent> {
        let mut event = None;
        pill_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                // Ringkasan seleksi / informasi status tool
                ui.label(
                    RichText::new(selection_summary)
                        .size(11.0)
                        .strong()
                        .color(ACCENT_BLUE),
                );

                // Tombol "Normal to Sketch" menyatu di pill bawah bila mode sketsa/tool aktif
                if show_normal_to_sketch {
                    ui.label(RichText::new("|").color(TEXT_SECONDARY));
                    let btn = ui.button(
                        RichText::new(format!("{} {}", ICON_3D_ROTATION.codepoint, t!("hud-normal-to-sketch")))
                            .size(11.0)
                            .strong()
                            .color(TEXT_PRIMARY),
                    );
                    if btn.clicked() {
                        event = Some(CanvasHudEvent::OrientNormalToSketch);
                    }
                }

                // Ringkasan pengukuran jika ada
                if let Some(m) = measurement_summary {
                    ui.label(RichText::new("|").color(TEXT_SECONDARY));
                    let resp = ui.selectable_label(
                        false,
                        RichText::new(format!("{} {}", ICON_STRAIGHTEN.codepoint, m))
                            .size(11.0)
                            .color(TEXT_PRIMARY),
                    );
                    if resp.clicked() {
                        event = Some(CanvasHudEvent::OpenMeasurements);
                    }
                }
            });
        });
        event
    }

    #[inline]
    pub fn show_bottom_status_pill(
        ui: &mut Ui,
        selection_summary: &str,
        measurement_summary: Option<&str>,
        show_normal_to_sketch: bool,
    ) -> Option<CanvasHudEvent> {
        Self::show_status_pill(
            ui,
            selection_summary,
            measurement_summary,
            show_normal_to_sketch,
        )
    }

    /// Render badge dimensi mengambang putih langsung pada posisi 2D di kanvas.
    pub fn render_dimension_pill(ui: &mut Ui, pos_2d: Pos2, value_text: &str, locked: bool) {
        if !pos_2d.x.is_finite() || !pos_2d.y.is_finite() {
            return;
        }
        let painter = ui.painter();
        let lock_icon = if locked {
            format!("{} ", ICON_LOCK.codepoint)
        } else {
            "".to_string()
        };
        let full_text = format!("{}{}", lock_icon, value_text);

        let font = FontId::proportional(11.0);
        let galley = painter.layout_no_wrap(full_text, font, Color32::from_rgb(20, 20, 22));
        let rect = egui::Rect::from_center_size(pos_2d, galley.size() + Vec2::new(16.0, 8.0));

        // Gambar latar belakang pill putih/terang dengan bayangan
        painter.rect_filled(
            rect,
            10.0,
            Color32::from_rgba_premultiplied(245, 246, 250, 245),
        );
        painter.rect_stroke(
            rect,
            10.0,
            Stroke::new(1.0, Color32::from_gray(180)),
            StrokeKind::Inside,
        );
        painter.galley(
            rect.min + Vec2::new(8.0, 4.0),
            galley,
            Color32::from_rgb(20, 20, 22),
        );
    }

    /// Render badge dimensi mengambang yang DIPUTAR sejajar arah garis pengukuran
    /// (bukan selalu horizontal seperti `render_dimension_pill`) — dipakai label
    /// nominal jarak/sudut supaya menempel rapi di garisnya sendiri dan tidak
    /// numpuk visual dengan garis pengukuran lain yang miring. `angle_rad` harus
    /// SUDAH dinormalisasi pemanggil ke rentang -90°..90° (mis. lewat
    /// `Vec2::angle()` lalu di-`±π` kalau di luar rentang itu) supaya teksnya
    /// tidak pernah kebalik/terbaca dari bawah ke atas.
    pub fn render_dimension_pill_aligned(
        ui: &mut Ui,
        center_2d: Pos2,
        angle_rad: f32,
        value_text: &str,
    ) {
        if !center_2d.x.is_finite() || !center_2d.y.is_finite() || !angle_rad.is_finite() {
            return;
        }
        let painter = ui.painter();
        let font = FontId::proportional(11.0);
        let galley =
            painter.layout_no_wrap(value_text.to_string(), font, Color32::from_rgb(20, 20, 22));
        let half = (galley.size() + Vec2::new(16.0, 8.0)) * 0.5;
        let rot = egui::emath::Rot2::from_angle(angle_rad);

        // Sudut membulat (radius 10, sama seperti `render_dimension_pill` yang
        // tidak diputar) diaproksimasi jadi polygon konveks — rounded-rect
        // bawaan egui (`rect_filled`) tidak punya varian berotasi — lalu semua
        // titiknya diputar & digeser ke `center_2d`.
        let corners: Vec<Pos2> = rounded_rect_local_points(half, 10.0, 6)
            .into_iter()
            .map(|c| center_2d + rot * c.to_vec2())
            .collect();

        // Agak transparan (dari solid 245 -> 210) supaya garis kuning di
        // baliknya tetap samar kelihatan, bukan ketutup penuh sama pill-nya.
        painter.add(egui::epaint::PathShape::convex_polygon(
            corners,
            Color32::from_rgba_premultiplied(245, 246, 250, 210),
            Stroke::new(1.0, Color32::from_rgba_premultiplied(170, 170, 175, 210)),
        ));

        // `TextShape::pos` adalah pojok kiri-atas galley SEBELUM rotasi, dengan
        // pivot rotasi di titik itu juga — jadi dihitung mundur dari pusat
        // supaya, setelah diputar `rot`, teksnya jatuh center di `center_2d`.
        let text_pos = center_2d + rot * (Vec2::new(-half.x, -half.y) + Vec2::new(8.0, 4.0));
        let mut text_shape =
            egui::epaint::TextShape::new(text_pos, galley, Color32::from_rgb(20, 20, 22));
        text_shape.angle = angle_rad;
        painter.add(text_shape);
    }

    /// Render badge dimensi interaktif putih (dengan border biru bila aktif / hover)
    /// yang dapat diklik untuk memasukkan angka presisi.
    pub fn render_interactive_dimension_pill(
        ui: &mut Ui,
        pos_2d: Pos2,
        value_text: &str,
        is_active: bool,
    ) -> egui::Response {
        if !pos_2d.x.is_finite() || !pos_2d.y.is_finite() {
            return ui.allocate_rect(egui::Rect::NOTHING, egui::Sense::hover());
        }
        let font = FontId::proportional(11.5);
        let galley = ui.painter().layout_no_wrap(
            value_text.to_string(),
            font,
            Color32::from_rgb(20, 20, 25),
        );
        let size = galley.size() + Vec2::new(18.0, 10.0);
        let rect = egui::Rect::from_center_size(pos_2d, size);

        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        let is_hovered = response.hovered();

        // Background & border
        let bg_color = if is_active {
            Color32::from_rgba_premultiplied(235, 245, 255, 250)
        } else {
            Color32::from_rgba_premultiplied(250, 250, 252, 245)
        };
        let border_color = if is_active || is_hovered {
            ACCENT_BLUE
        } else {
            Color32::from_gray(185)
        };
        let stroke_width = if is_active { 1.8 } else { 1.0 };

        let painter = ui.painter();
        painter.rect_filled(rect, 8.0, bg_color);
        painter.rect_stroke(
            rect,
            8.0,
            Stroke::new(stroke_width, border_color),
            StrokeKind::Inside,
        );
        painter.galley(
            rect.min + Vec2::new(9.0, 5.0),
            galley,
            Color32::from_rgb(20, 20, 25),
        );

        response
    }

    /// Render badge dimensi interaktif yang DIPUTAR sejajar arah garis pengukuran
    /// (dengan border biru bila aktif / hover) yang dapat diklik untuk memasukkan angka presisi.
    pub fn render_interactive_dimension_pill_aligned(
        ui: &mut Ui,
        center_2d: Pos2,
        angle_rad: f32,
        value_text: &str,
        is_active: bool,
    ) -> egui::Response {
        if !center_2d.x.is_finite() || !center_2d.y.is_finite() || !angle_rad.is_finite() {
            return ui.allocate_rect(egui::Rect::NOTHING, egui::Sense::hover());
        }
        let font = FontId::proportional(11.5);
        let galley = ui.painter().layout_no_wrap(
            value_text.to_string(),
            font,
            Color32::from_rgb(20, 20, 25),
        );
        let half = (galley.size() + Vec2::new(18.0, 10.0)) * 0.5;
        let size = half * 2.0;
        let max_dim = size.x.max(size.y);
        let rect = egui::Rect::from_center_size(center_2d, Vec2::splat(max_dim));

        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        let is_hovered = response.hovered();

        let bg_color = if is_active {
            Color32::from_rgba_premultiplied(235, 245, 255, 250)
        } else if is_hovered {
            Color32::from_rgba_premultiplied(255, 255, 255, 250)
        } else {
            Color32::from_rgba_premultiplied(245, 246, 250, 220)
        };
        let border_color = if is_active || is_hovered {
            ACCENT_BLUE
        } else {
            Color32::from_rgba_premultiplied(170, 170, 175, 220)
        };
        let stroke_width = if is_active || is_hovered { 1.8 } else { 1.0 };

        let rot = egui::emath::Rot2::from_angle(angle_rad);
        let corners: Vec<Pos2> = rounded_rect_local_points(half, 8.0, 6)
            .into_iter()
            .map(|c| center_2d + rot * c.to_vec2())
            .collect();

        let painter = ui.painter();
        painter.add(egui::epaint::PathShape::convex_polygon(
            corners,
            bg_color,
            Stroke::new(stroke_width, border_color),
        ));

        let text_pos = center_2d + rot * (Vec2::new(-half.x, -half.y) + Vec2::new(9.0, 5.0));
        let mut text_shape =
            egui::epaint::TextShape::new(text_pos, galley, Color32::from_rgb(20, 20, 25));
        text_shape.angle = angle_rad;
        painter.add(text_shape);

        response
    }

    /// Render badge tombol "Copy" floating (gaya Shapr3D) di bawah Transform Gizmo.
    pub fn render_copy_toggle_badge(
        ui: &mut Ui,
        pos_2d: Pos2,
        is_copy_active: bool,
    ) -> egui::Response {
        if !pos_2d.x.is_finite() || !pos_2d.y.is_finite() {
            return ui.allocate_rect(egui::Rect::NOTHING, egui::Sense::hover());
        }
        let font = FontId::proportional(12.0);
        let text = "Copy";
        let galley = ui.painter().layout_no_wrap(
            text.to_string(),
            font.clone(),
            Color32::from_rgb(20, 20, 25),
        );
        let size = galley.size() + Vec2::new(20.0, 10.0);
        let rect = egui::Rect::from_center_size(pos_2d, size);

        let response = ui.allocate_rect(rect, egui::Sense::click());
        let is_hovered = response.hovered();

        let bg_color = if is_copy_active {
            Color32::from_rgb(40, 130, 250)
        } else if is_hovered {
            Color32::from_rgba_premultiplied(240, 245, 255, 250)
        } else {
            Color32::from_rgba_premultiplied(255, 255, 255, 245)
        };
        let text_color = if is_copy_active {
            Color32::WHITE
        } else {
            Color32::from_rgb(20, 20, 25)
        };
        let border_color = if is_copy_active {
            ACCENT_BLUE
        } else if is_hovered {
            ACCENT_BLUE
        } else {
            Color32::from_gray(180)
        };

        let painter = ui.painter();
        painter.rect_filled(rect, 8.0, bg_color);
        painter.rect_stroke(
            rect,
            8.0,
            Stroke::new(1.2, border_color),
            StrokeKind::Inside,
        );
        let text_galley = painter.layout_no_wrap(text.to_string(), font, text_color);
        painter.galley(rect.min + Vec2::new(10.0, 5.0), text_galley, text_color);

        if is_hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        response
    }

    /// Render badge sudut interaktif (mis. "45.0°") yang dapat diklik untuk mengetik angka sudut rotasi.
    pub fn render_interactive_angle_pill(
        ui: &mut Ui,
        pos_2d: Pos2,
        value_text: &str,
        is_active: bool,
    ) -> egui::Response {
        if !pos_2d.x.is_finite() || !pos_2d.y.is_finite() {
            return ui.allocate_rect(egui::Rect::NOTHING, egui::Sense::hover());
        }
        let font = FontId::proportional(11.5);
        let galley = ui.painter().layout_no_wrap(
            value_text.to_string(),
            font,
            Color32::from_rgb(20, 20, 25),
        );
        let size = galley.size() + Vec2::new(18.0, 10.0);
        let rect = egui::Rect::from_center_size(pos_2d, size);

        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        let is_hovered = response.hovered();

        let bg_color = if is_active {
            Color32::from_rgba_premultiplied(235, 245, 255, 250)
        } else {
            Color32::from_rgba_premultiplied(250, 250, 252, 245)
        };
        let border_color = if is_active || is_hovered {
            ACCENT_BLUE
        } else {
            Color32::from_gray(185)
        };
        let stroke_width = if is_active { 1.8 } else { 1.0 };

        let painter = ui.painter();
        painter.rect_filled(rect, 8.0, bg_color);
        painter.rect_stroke(
            rect,
            8.0,
            Stroke::new(stroke_width, border_color),
            StrokeKind::Inside,
        );
        painter.galley(
            rect.min + Vec2::new(9.0, 5.0),
            galley,
            Color32::from_rgb(20, 20, 25),
        );

        response
    }

    /// Area sense-drag utk gizmo panah dua-sisi push/pull/extrude/rounding
    pub fn render_draggable_double_arrow_handle(
        ui: &mut Ui,
        pos_2d: Pos2,
        is_dragging: bool,
        dir_2d: Option<Vec2>,
    ) -> egui::Response {
        if !pos_2d.x.is_finite() || !pos_2d.y.is_finite() {
            return ui.allocate_rect(egui::Rect::NOTHING, egui::Sense::hover());
        }
        let handle_radius = if is_dragging { 18.0 } else { 16.0 };
        let rect = egui::Rect::from_center_size(pos_2d, Vec2::splat(handle_radius * 2.0 + 8.0));
        let response = ui.allocate_rect(rect, egui::Sense::drag());
        let is_hovered = response.hovered();

        let dir_u = match dir_2d {
            Some(d) if d.length() > 1e-4 => d.normalized(),
            _ => Vec2::new(0.0, -1.0),
        };

        if is_hovered || is_dragging {
            let cursor = if dir_u.x.abs() > dir_u.y.abs() * 2.0 {
                egui::CursorIcon::ResizeHorizontal
            } else if dir_u.y.abs() > dir_u.x.abs() * 2.0 {
                egui::CursorIcon::ResizeVertical
            } else if dir_u.x * dir_u.y < 0.0 {
                egui::CursorIcon::ResizeNeSw
            } else {
                egui::CursorIcon::ResizeNwSe
            };
            ui.ctx().set_cursor_icon(cursor);
        }

        // --- VISUAL ---
        let painter = ui.painter();
        let accent = if is_dragging {
            Color32::from_rgb(0, 210, 180)
        } else if is_hovered {
            Color32::from_rgb(80, 200, 255)
        } else {
            Color32::from_rgb(0, 180, 255)
        };
        let bg = Color32::from_rgba_premultiplied(10, 20, 35, 220);

        // Diamond background
        let r = if is_dragging { 9.0 } else { 7.0 };
        let diamond = vec![
            pos_2d + Vec2::new(0.0, -r),
            pos_2d + Vec2::new(r, 0.0),
            pos_2d + Vec2::new(0.0, r),
            pos_2d + Vec2::new(-r, 0.0),
        ];
        painter.add(egui::Shape::convex_polygon(diamond.clone(), bg, Stroke::new(1.5, accent)));

        // Double arrow along dir_u
        let arrow_len = if is_dragging { 13.0 } else { 11.0 };
        let perp = Vec2::new(-dir_u.y, dir_u.x) * 2.5;

        for sign in [-1.0f32, 1.0f32] {
            let tip = pos_2d + dir_u * (r + arrow_len) * sign;
            let base = pos_2d + dir_u * r * sign;
            painter.line_segment([base, tip], Stroke::new(1.5, accent));
            // Arrowhead
            let head_size = 5.0;
            let side1 = tip - dir_u * head_size * sign + perp;
            let side2 = tip - dir_u * head_size * sign - perp;
            painter.add(egui::Shape::convex_polygon(
                vec![tip, side1, side2],
                accent,
                Stroke::NONE,
            ));
        }

        response
    }

    /// Crosshair "+" draggable
    pub fn render_draggable_move_handle(
        ui: &mut Ui,
        pos_2d: Pos2,
        is_dragging: bool,
        is_armed: bool,
    ) -> egui::Response {
        if !pos_2d.x.is_finite() || !pos_2d.y.is_finite() {
            return ui.allocate_rect(egui::Rect::NOTHING, egui::Sense::hover());
        }
        let active = is_dragging || is_armed;
        let handle_radius = if active { 9.0 } else { 7.0 };
        let rect = egui::Rect::from_center_size(pos_2d, Vec2::splat(handle_radius * 2.0 + 20.0));
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        let is_hovered = response.hovered();

        if is_hovered || active {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Move);
        }

        let painter = ui.painter();
        let blend_color = Color32::from_rgb(77, 166, 255);

        if is_hovered || is_dragging {
            painter.circle_filled(
                pos_2d,
                handle_radius + 6.0,
                blend_color.gamma_multiply(0.22),
            );
        } else if is_armed {
            painter.circle_stroke(
                pos_2d,
                handle_radius + 6.0,
                Stroke::new(1.2, blend_color.gamma_multiply(0.8)),
            );
        }

        let dot_radius = if active { 3.0 } else { 2.0 };
        painter.circle_filled(pos_2d, dot_radius, blend_color);
        if active {
            painter.circle_stroke(pos_2d, dot_radius, Stroke::new(1.0, Color32::WHITE));
        }

        let arm = handle_radius + (if active { 6.0 } else { 4.0 });
        let gap = dot_radius + 1.5;
        let stroke_w = if active { 2.0 } else { 1.4 };
        let stroke = Stroke::new(stroke_w, blend_color);
        painter.line_segment(
            [pos_2d - Vec2::new(arm, 0.0), pos_2d - Vec2::new(gap, 0.0)],
            stroke,
        );
        painter.line_segment(
            [pos_2d + Vec2::new(gap, 0.0), pos_2d + Vec2::new(arm, 0.0)],
            stroke,
        );
        painter.line_segment(
            [pos_2d - Vec2::new(0.0, arm), pos_2d - Vec2::new(0.0, gap)],
            stroke,
        );
        painter.line_segment(
            [pos_2d + Vec2::new(0.0, gap), pos_2d + Vec2::new(0.0, arm)],
            stroke,
        );

        response
    }

    /// Render animasi interaktif panduan Revolve di atas kanvas beserta tombol cepat pengaturan sudut dan arah putar.
    #[allow(clippy::too_many_arguments)]
    pub fn render_revolve_animated_guide(
        ui: &mut Ui,
        canvas_rect: Rect,
        pending_points_count: usize,
        has_selection: bool,
        current_angle: f64,
        angle_input: &mut String,
        is_reversed: bool,
        is_staged: bool,
        _time: f64,
    ) -> Option<RevolveHudAction> {
        ui.ctx().request_repaint();

        if angle_input.is_empty() {
            *angle_input = format!("{:.0}", current_angle);
        }

        let mut hud_action = None;

        // 1. Floating Pill Banner di bagian atas tengah kanvas (Diposisikan di bawah Top Bar)
        let banner_w = if is_staged { 760.0 } else { 660.0 };
        let banner_pos = Pos2::new(canvas_rect.center().x, canvas_rect.top() + 80.0);
        let banner_rect = egui::Rect::from_center_size(banner_pos, Vec2::new(banner_w, 36.0));

        ui.painter().rect_filled(
            banner_rect,
            18.0,
            Color32::from_rgba_premultiplied(15, 18, 24, 240),
        );
        ui.painter().rect_stroke(
            banner_rect,
            18.0,
            Stroke::new(
                1.2,
                if is_staged {
                    ACCENT_GREEN
                } else {
                    ACCENT_BLUE.gamma_multiply(0.8)
                },
            ),
            StrokeKind::Inside,
        );

        let step_text = if !has_selection {
            t!("hud-revolve-prompt-select")
        } else if is_staged {
            t!("hud-revolve-prompt-ready")
        } else if pending_points_count == 0 {
            t!("hud-revolve-prompt-step-1")
        } else {
            t!("hud-revolve-prompt-step-2")
        };

        // Layout horizontal di dalam banner
        let mut banner_ui = ui.new_child(egui::UiBuilder::new().max_rect(banner_rect));
        banner_ui.horizontal_centered(|ui| {
            ui.add_space(14.0);
            ui.label(
                RichText::new(&step_text)
                    .size(11.5)
                    .strong()
                    .color(if is_staged {
                        ACCENT_GREEN
                    } else {
                        Color32::WHITE
                    }),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);

                if is_staged {
                    // Tombol Konfirmasi Selesai / Terapkan
                    let apply_btn = egui::Button::new(
                        RichText::new(t!("hud-apply-enter"))
                            .size(11.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(ACCENT_GREEN);

                    if ui.add(apply_btn).clicked() {
                        hud_action = Some(RevolveHudAction::Commit);
                    }

                    ui.add_space(6.0);
                }

                // Tombol Toggle Arah Putar (CW vs CCW)
                let dir_text = if is_reversed {
                    format!("{}: CW", t!("param-direction"))
                } else {
                    format!("{}: CCW", t!("param-direction"))
                };
                let dir_btn = egui::Button::new(RichText::new(dir_text).size(10.5).strong().color(
                    if is_reversed {
                        ACCENT_ORANGE
                    } else {
                        TEXT_PRIMARY
                    },
                ))
                .fill(if is_reversed {
                    Color32::from_rgba_premultiplied(70, 45, 15, 200)
                } else {
                    Color32::from_rgba_premultiplied(40, 44, 52, 180)
                });

                if ui.add(dir_btn).clicked() {
                    hud_action = Some(RevolveHudAction::ToggleReverse);
                }

                ui.add_space(6.0);

                // Input Sudut Kustom (Manual TextEdit misal 27°)
                ui.label(RichText::new("°").size(11.0).color(TEXT_SECONDARY));
                let angle_edit = egui::TextEdit::singleline(angle_input)
                    .desired_width(44.0)
                    .font(egui::FontId::proportional(11.0))
                    .margin(egui::Margin::symmetric(4, 3));
                let resp = ui.add(angle_edit);
                if resp.changed() {
                    if let Ok(val) = angle_input.trim().parse::<f64>() {
                        if val > 0.0 && val <= 360.0 {
                            hud_action = Some(RevolveHudAction::SetAngle(val));
                        }
                    }
                }

                ui.add_space(4.0);

                // Tombol Pilihan Sudut Preset (360°, 270°, 180°, 90°)
                for &deg in &[360.0, 270.0, 180.0, 90.0] {
                    let is_active = (current_angle - deg).abs() < 1e-3;
                    let label = format!("{:.0}°", deg);
                    let btn = egui::Button::new(RichText::new(label).size(11.0).strong().color(
                        if is_active {
                            Color32::WHITE
                        } else {
                            TEXT_SECONDARY
                        },
                    ))
                    .fill(if is_active {
                        ACCENT_BLUE
                    } else {
                        Color32::from_rgba_premultiplied(40, 44, 52, 180)
                    });

                    if ui.add(btn).clicked() {
                        *angle_input = format!("{:.0}", deg);
                        hud_action = Some(RevolveHudAction::SetAngle(deg));
                    }
                }
                ui.label(RichText::new(format!("{}:", t!("param-angle"))).size(10.5).color(TEXT_SECONDARY));
            });
        });

        // 2. Mini Tutorial Animated Pointer Card di pojok kiri bawah kanvas
        let card_w = 240.0;
        let card_h = 145.0;
        let guide_center = Pos2::new(
            canvas_rect.left() + card_w * 0.5 + 20.0,
            canvas_rect.bottom() - card_h * 0.5 - 20.0,
        );
        let card_rect = egui::Rect::from_center_size(guide_center, Vec2::new(card_w, card_h));

        let painter = ui.painter();
        painter.rect_filled(
            card_rect,
            10.0,
            Color32::from_rgba_premultiplied(18, 20, 26, 235),
        );
        painter.rect_stroke(
            card_rect,
            10.0,
            Stroke::new(1.0, BORDER_SUBTLE),
            StrokeKind::Inside,
        );

        let p_top = Pos2::new(card_rect.left() + 10.0, card_rect.top() + 8.0);
        painter.text(
            p_top,
            Align2::LEFT_TOP,
            &t!("tool-revolve-name"),
            egui::FontId::proportional(10.0),
            TEXT_SECONDARY,
        );

        let p_step = Pos2::new(card_rect.left() + 10.0, card_rect.top() + 21.0);
        painter.text(
            p_step,
            Align2::LEFT_TOP,
            &step_text,
            egui::FontId::proportional(11.0),
            if is_staged { ACCENT_GREEN } else { ACCENT_ORANGE },
        );

        let c_sketch = Pos2::new(card_rect.left() + 65.0, card_rect.center().y + 8.0);
        let axis_x = card_rect.left() + 115.0;
        let r_sketch = egui::Rect::from_center_size(c_sketch, Vec2::new(32.0, 42.0));

        painter.rect_filled(
            r_sketch,
            2.0,
            if has_selection {
                ACCENT_BLUE.gamma_multiply(0.35)
            } else {
                Color32::from_rgba_premultiplied(40, 44, 52, 100)
            },
        );
        painter.rect_stroke(
            r_sketch,
            2.0,
            Stroke::new(
                1.5,
                if has_selection {
                    ACCENT_BLUE
                } else {
                    TEXT_MUTED
                },
            ),
            StrokeKind::Inside,
        );

        let a1 = Pos2::new(axis_x, card_rect.top() + 40.0);
        let a2 = Pos2::new(axis_x, card_rect.bottom() - 35.0);
        painter.line_segment(
            [a1, a2],
            Stroke::new(
                1.5,
                if is_staged {
                    ACCENT_GREEN
                } else {
                    ACCENT_ORANGE
                },
            ),
        );

        if is_staged {
            let rot_c = Pos2::new(axis_x, card_rect.center().y + 8.0);
            painter.circle_stroke(rot_c, 18.0, Stroke::new(1.2, ACCENT_GREEN.gamma_multiply(0.8)));
        }

        let p_tip = Pos2::new(card_rect.left() + 10.0, card_rect.bottom() - 10.0);
        painter.text(
            p_tip,
            Align2::LEFT_BOTTOM,
            &t!("hud-revolve-prompt-ready"),
            egui::FontId::proportional(9.0),
            TEXT_MUTED,
        );

        hud_action
    }

    /// Render Top Bar HUD mengambang untuk mode Loft 3D (seperti Revolve)
    /// + modal dialog Penyelarasan Titik Pusat jika titik tengah belum menyatu.
    #[allow(clippy::too_many_arguments)]
    pub fn render_loft_top_bar_hud(
        ui: &mut Ui,
        canvas_rect: Rect,
        selected_regions_count: usize,
        current_height: f64,
        height_input: &mut String,
        centroids_offset: Option<f64>,
        alignment_dismissed: bool,
        is_flipped: bool,
        is_staged: bool,
    ) -> Option<LoftHudAction> {
        let mut hud_action = None;
        let is_ready = selected_regions_count == 2;

        // 1. Top Horizontal HUD Banner
        let banner_w = 680.0;
        let banner_pos = Pos2::new(canvas_rect.center().x, canvas_rect.top() + 84.0);
        let banner_rect = egui::Rect::from_center_size(banner_pos, Vec2::new(banner_w, 36.0));

        ui.painter().rect_filled(
            banner_rect,
            18.0,
            Color32::from_rgba_premultiplied(15, 18, 24, 240),
        );
        ui.painter().rect_stroke(
            banner_rect,
            18.0,
            Stroke::new(
                1.2,
                if is_staged || is_ready {
                    ACCENT_GREEN
                } else {
                    ACCENT_BLUE.gamma_multiply(0.8)
                },
            ),
            StrokeKind::Inside,
        );

        let step_text = match selected_regions_count {
            0 => t!("hud-loft-prompt-0"),
            1 => t!("hud-loft-prompt-1"),
            _ => t!("hud-loft-prompt-ready"),
        };

        // Layout horizontal di dalam banner
        let mut banner_ui = ui.new_child(egui::UiBuilder::new().max_rect(banner_rect));
        banner_ui.horizontal_centered(|ui| {
            ui.add_space(14.0);
            ui.label(RichText::new(&step_text).size(11.5).strong().color(
                if is_staged || is_ready {
                    ACCENT_GREEN
                } else {
                    Color32::WHITE
                },
            ));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);

                if is_ready {
                    // Tombol Selesai (Commit)
                    let main_label = t!("hud-loft-create-enter");
                    let create_btn = egui::Button::new(
                        RichText::new(main_label)
                            .size(11.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(if is_staged { ACCENT_GREEN } else { ACCENT_BLUE });

                    if ui.add(create_btn).clicked() {
                        hud_action = Some(LoftHudAction::Commit);
                    }

                    ui.add_space(6.0);

                    // Tombol Balik Posisi Atas / Bawah
                    let flip_label = if is_flipped {
                        format!("↕ {}: 2", t!("param-direction"))
                    } else {
                        format!("↕ {}: 1", t!("param-direction"))
                    };
                    let flip_btn =
                        egui::Button::new(RichText::new(flip_label).size(10.5).strong().color(
                            if is_flipped {
                                ACCENT_ORANGE
                            } else {
                                TEXT_PRIMARY
                            },
                        ))
                        .fill(if is_flipped {
                            Color32::from_rgba_premultiplied(70, 45, 15, 200)
                        } else {
                            Color32::from_rgba_premultiplied(40, 44, 52, 180)
                        });

                    if ui
                        .add(flip_btn)
                        .clicked()
                    {
                        hud_action = Some(LoftHudAction::ToggleFlip);
                    }

                    ui.add_space(6.0);
                }

                // Input Tinggi Kustom (Manual TextEdit misal 20.0 mm)
                ui.label(RichText::new("mm").size(11.0).color(TEXT_SECONDARY));
                let height_edit = egui::TextEdit::singleline(height_input)
                    .desired_width(48.0)
                    .font(egui::FontId::proportional(11.0))
                    .margin(egui::Margin::symmetric(4, 3));
                let resp = ui.add(height_edit);
                if resp.changed() {
                    if let Ok(val) = height_input.trim().parse::<f64>() {
                        if val > 0.0 {
                            hud_action = Some(LoftHudAction::SetHeight(val));
                        }
                    }
                }

                ui.add_space(4.0);

                // Tombol Pilihan Tinggi Preset (10mm, 20mm, 30mm, 50mm, 100mm)
                for &h in &[10.0, 20.0, 30.0, 50.0, 100.0] {
                    let is_active = (current_height - h).abs() < 1e-3;
                    let label = format!("{:.0}", h);
                    let btn = egui::Button::new(RichText::new(label).size(11.0).strong().color(
                        if is_active {
                            Color32::WHITE
                        } else {
                            TEXT_SECONDARY
                        },
                    ))
                    .fill(if is_active {
                        ACCENT_BLUE
                    } else {
                        Color32::from_rgba_premultiplied(40, 44, 52, 180)
                    });

                    if ui.add(btn).clicked() {
                        *height_input = format!("{:.1}", h);
                        hud_action = Some(LoftHudAction::SetHeight(h));
                    }
                }
                ui.label(RichText::new(format!("{}:", t!("param-height"))).size(10.5).color(TEXT_SECONDARY));
            });
        });

        // 2. Dialog Modal / Floating Card Penyelarasan Titik Pusat jika centroid offset > 0.1 mm
        if is_ready && !alignment_dismissed {
            if let Some(dist) = centroids_offset {
                if dist > 0.1 {
                    let modal_w = 400.0;
                    let modal_h = 105.0;
                    let modal_pos = Pos2::new(canvas_rect.center().x, canvas_rect.top() + 160.0);
                    let modal_rect =
                        egui::Rect::from_center_size(modal_pos, Vec2::new(modal_w, modal_h));

                    ui.painter().rect_filled(
                        modal_rect,
                        10.0,
                        Color32::from_rgba_premultiplied(20, 24, 34, 250),
                    );
                    ui.painter().rect_stroke(
                        modal_rect,
                        10.0,
                        Stroke::new(1.2, ACCENT_ORANGE),
                        StrokeKind::Inside,
                    );

                    let mut modal_ui =
                        ui.new_child(egui::UiBuilder::new().max_rect(modal_rect.shrink(10.0)));
                    modal_ui.vertical_centered(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(t!("hud-loft-warn-unaligned"))
                                    .size(12.0)
                                    .strong()
                                    .color(ACCENT_ORANGE),
                            );
                            ui.label(
                                RichText::new(format!("(Offset: {:.1} mm)", dist))
                                    .size(11.0)
                                    .color(TEXT_SECONDARY),
                            );
                        });
                        ui.add_space(3.0);
                        ui.label(
                            RichText::new(t!("hud-loft-align-question"))
                                .size(10.5)
                                .color(TEXT_PRIMARY),
                        );
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            let align_btn = egui::Button::new(
                                RichText::new(t!("hud-loft-align-center")).size(11.0).strong().color(Color32::WHITE),
                            )
                            .fill(ACCENT_BLUE);
                            if ui.add(align_btn).clicked() {
                                hud_action = Some(LoftHudAction::AlignCentroids);
                            }

                            ui.add_space(8.0);
                            let keep_btn = egui::Button::new(
                                RichText::new(t!("hud-loft-keep-offset")).size(10.5).color(TEXT_PRIMARY),
                            )
                            .fill(Color32::from_rgba_premultiplied(50, 55, 65, 200));
                            if ui.add(keep_btn).clicked() {
                                hud_action = Some(LoftHudAction::DismissAlignmentDialog);
                            }
                        });
                    });
                }
            }
        }

        hud_action
    }

    /// Render Top Bar HUD mengambang untuk mode Shell / Hollow 3D (seperti Revolve & Loft)
    pub fn render_shell_top_bar_hud(
        ui: &mut Ui,
        canvas_rect: Rect,
        has_face_selection: bool,
        current_thickness: f64,
        thickness_input: &mut String,
    ) -> Option<ShellHudAction> {
        let mut hud_action = None;

        let banner_w = 680.0;
        let banner_pos = Pos2::new(canvas_rect.center().x, canvas_rect.top() + 84.0);
        let banner_rect = egui::Rect::from_center_size(banner_pos, Vec2::new(banner_w, 36.0));

        ui.painter().rect_filled(
            banner_rect,
            18.0,
            Color32::from_rgba_premultiplied(15, 18, 24, 240),
        );
        ui.painter().rect_stroke(
            banner_rect,
            18.0,
            Stroke::new(
                1.2,
                if has_face_selection {
                    ACCENT_GREEN
                } else {
                    ACCENT_BLUE.gamma_multiply(0.8)
                },
            ),
            StrokeKind::Inside,
        );

        let step_text = if has_face_selection {
            t!("hud-shell-prompt-ready")
        } else {
            t!("hud-shell-prompt-select")
        };

        // Layout horizontal di dalam banner
        let mut banner_ui = ui.new_child(egui::UiBuilder::new().max_rect(banner_rect));
        banner_ui.horizontal_centered(|ui| {
            ui.add_space(14.0);
            ui.label(
                RichText::new(&step_text)
                    .size(11.5)
                    .strong()
                    .color(if has_face_selection {
                        ACCENT_GREEN
                    } else {
                        Color32::WHITE
                    }),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);

                if has_face_selection {
                    // Tombol Eksekusi Shell (Commit)
                    let exec_btn = egui::Button::new(
                        RichText::new(t!("hud-shell-exec-enter"))
                            .size(11.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(ACCENT_GREEN);

                    if ui.add(exec_btn).clicked() {
                        hud_action = Some(ShellHudAction::Commit);
                    }

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // Input Textbox Tebal
                    let text_edit = egui::TextEdit::singleline(thickness_input)
                        .desired_width(46.0)
                        .font(egui::FontId::monospace(11.0));
                    let resp = ui.add(text_edit);
                    if resp.changed() {
                        if let Ok(t) = thickness_input.trim().parse::<f64>() {
                            hud_action = Some(ShellHudAction::SetThickness(t));
                        }
                    }

                    ui.label(RichText::new("mm").size(10.5).color(TEXT_SECONDARY));

                    // Quick Preset Buttons [5 mm, 3 mm, 2 mm, 1 mm]
                    for &t in &[5.0, 3.0, 2.0, 1.0] {
                        let is_active = (current_thickness - t).abs() < 0.05;
                        let btn = egui::Button::new(
                            RichText::new(format!("{:.0}", t))
                                .size(10.5)
                                .color(if is_active { ACCENT_BLUE } else { TEXT_PRIMARY }),
                        )
                        .fill(if is_active {
                            Color32::from_rgba_premultiplied(40, 60, 90, 200)
                        } else {
                            Color32::from_rgba_premultiplied(35, 40, 50, 180)
                        });

                        if ui.add(btn).clicked() {
                            *thickness_input = format!("{:.1}", t);
                            hud_action = Some(ShellHudAction::SetThickness(t));
                        }
                    }
                    ui.label(RichText::new(format!("{}:", t!("param-thickness"))).size(10.5).color(TEXT_SECONDARY));
                }
            });
        });

        hud_action
    }

    /// Render Top Bar HUD mengambang untuk mode Rib / Tulang Penguat 3D (Stiffener Support).
    #[allow(clippy::too_many_arguments)]
    pub fn render_rib_top_bar_hud(
        ui: &mut Ui,
        canvas_rect: Rect,
        has_geometry: bool,
        angle_input: &mut String,
        thickness_input: &mut String,
        depth_input: &mut String,
        draft_input: &mut String,
    ) -> Option<RibHudAction> {
        let mut hud_action = None;

        let banner_w = 940.0;
        let banner_pos = Pos2::new(canvas_rect.center().x, canvas_rect.top() + 84.0);
        let banner_rect = egui::Rect::from_center_size(banner_pos, Vec2::new(banner_w, 38.0));

        ui.painter().rect_filled(
            banner_rect,
            19.0,
            Color32::from_rgba_premultiplied(15, 18, 24, 240),
        );
        ui.painter().rect_stroke(
            banner_rect,
            19.0,
            Stroke::new(
                1.2,
                if has_geometry {
                    ACCENT_GREEN
                } else {
                    ACCENT_BLUE.gamma_multiply(0.8)
                },
            ),
            StrokeKind::Inside,
        );

        let step_text = if has_geometry {
            "🦴 Face Terpilih"
        } else {
            "🦴 Pilih Face Casing"
        };

        let mut banner_ui = ui.new_child(egui::UiBuilder::new().max_rect(banner_rect));
        banner_ui.horizontal_centered(|ui| {
            ui.add_space(14.0);
            ui.label(
                RichText::new(step_text)
                    .size(11.5)
                    .strong()
                    .color(if has_geometry {
                        ACCENT_GREEN
                    } else {
                        Color32::WHITE
                    }),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);

                if has_geometry {
                    let exec_btn = egui::Button::new(
                        RichText::new(t!("hud-rib-exec-enter"))
                            .size(11.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(ACCENT_GREEN);

                    if ui.add(exec_btn).clicked() {
                        hud_action = Some(RibHudAction::Commit);
                    }

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // Draft angle input
                    let draft_edit = egui::TextEdit::singleline(draft_input)
                        .desired_width(32.0)
                        .font(egui::FontId::monospace(11.0));
                    let r3 = ui.add(draft_edit);
                    if r3.changed() {
                        if let Ok(d) = draft_input.trim().parse::<f64>() {
                            hud_action = Some(RibHudAction::SetDraftAngle(d));
                        }
                    }
                    ui.label(RichText::new("°").size(10.5).color(TEXT_SECONDARY));
                    ui.label(RichText::new(format!("{}:", t!("param-draft"))).size(10.5).color(TEXT_SECONDARY));

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // Depth input
                    let depth_edit = egui::TextEdit::singleline(depth_input)
                        .desired_width(38.0)
                        .font(egui::FontId::monospace(11.0));
                    let r2 = ui.add(depth_edit);
                    if r2.changed() {
                        if let Ok(d) = depth_input.trim().parse::<f64>() {
                            hud_action = Some(RibHudAction::SetDepth(d));
                        }
                    }
                    ui.label(RichText::new("mm").size(10.5).color(TEXT_SECONDARY));
                    ui.label(RichText::new(format!("{}:", t!("param-depth"))).size(10.5).color(TEXT_SECONDARY));

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // Thickness input
                    let thick_edit = egui::TextEdit::singleline(thickness_input)
                        .desired_width(38.0)
                        .font(egui::FontId::monospace(11.0));
                    let r1 = ui.add(thick_edit);
                    if r1.changed() {
                        if let Ok(t) = thickness_input.trim().parse::<f64>() {
                            hud_action = Some(RibHudAction::SetThickness(t));
                        }
                    }
                    ui.label(RichText::new("mm").size(10.5).color(TEXT_SECONDARY));
                    ui.label(RichText::new(format!("{}:", t!("param-thickness"))).size(10.5).color(TEXT_SECONDARY));

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // Angle Input & Preset Buttons (0° Horisontal, 90° Vertikal, 45° Diagonal)
                    let current_ang = angle_input.trim().parse::<f64>().unwrap_or(0.0);

                    let btn_45 = egui::Button::new(RichText::new("45°").size(10.0).color(Color32::WHITE))
                        .fill(if (current_ang - 45.0).abs() < 1e-2 {
                            ACCENT_BLUE
                        } else {
                            Color32::from_rgba_premultiplied(40, 50, 65, 200)
                        });
                    if ui.add(btn_45).clicked() {
                        *angle_input = "45.0".to_string();
                        hud_action = Some(RibHudAction::SetAngle(45.0));
                    }

                    let btn_90 = egui::Button::new(RichText::new("90° Vertikal").size(10.0).color(Color32::WHITE))
                        .fill(if (current_ang - 90.0).abs() < 1e-2 {
                            ACCENT_BLUE
                        } else {
                            Color32::from_rgba_premultiplied(40, 50, 65, 200)
                        });
                    if ui.add(btn_90).clicked() {
                        *angle_input = "90.0".to_string();
                        hud_action = Some(RibHudAction::SetAngle(90.0));
                    }

                    let btn_0 = egui::Button::new(RichText::new("0° Horisontal").size(10.0).color(Color32::WHITE))
                        .fill(if current_ang.abs() < 1e-2 {
                            ACCENT_BLUE
                        } else {
                            Color32::from_rgba_premultiplied(40, 50, 65, 200)
                        });
                    if ui.add(btn_0).clicked() {
                        *angle_input = "0.0".to_string();
                        hud_action = Some(RibHudAction::SetAngle(0.0));
                    }

                    let ang_edit = egui::TextEdit::singleline(angle_input)
                        .desired_width(34.0)
                        .font(egui::FontId::monospace(11.0));
                    let r_ang = ui.add(ang_edit);
                    if r_ang.changed() {
                        if let Ok(a) = angle_input.trim().parse::<f64>() {
                            hud_action = Some(RibHudAction::SetAngle(a));
                        }
                    }
                    ui.label(RichText::new("°").size(10.5).color(TEXT_SECONDARY));
                    ui.label(RichText::new("Sudut:").size(10.5).color(TEXT_SECONDARY));
                }
            });
        });

        hud_action
    }

    /// Render Top Bar HUD mengambang untuk mode Draft Angle 3D (Kemiringan Cetakan)
    pub fn render_draft_top_bar_hud(
        ui: &mut Ui,
        canvas_rect: Rect,
        selected_faces_count: usize,
        current_angle: f64,
        angle_input: &mut String,
        current_pull_dir: &mut DraftPullDir,
    ) -> Option<DraftHudAction> {
        let mut hud_action = None;
        let has_face_selection = selected_faces_count > 0;

        let banner_w = 780.0;
        let banner_pos = Pos2::new(canvas_rect.center().x, canvas_rect.top() + 84.0);
        let banner_rect = egui::Rect::from_center_size(banner_pos, Vec2::new(banner_w, 36.0));

        ui.painter().rect_filled(
            banner_rect,
            18.0,
            Color32::from_rgba_premultiplied(15, 18, 24, 240),
        );
        ui.painter().rect_stroke(
            banner_rect,
            18.0,
            Stroke::new(
                1.2,
                if has_face_selection {
                    ACCENT_ORANGE
                } else {
                    ACCENT_BLUE.gamma_multiply(0.8)
                },
            ),
            StrokeKind::Inside,
        );

        let step_text = if selected_faces_count > 0 {
            t!("popup-draft-faces-count", count = selected_faces_count)
        } else {
            t!("popup-draft-no-face")
        };

        // Layout horizontal di dalam banner
        let mut banner_ui = ui.new_child(egui::UiBuilder::new().max_rect(banner_rect));
        banner_ui.horizontal_centered(|ui| {
            ui.add_space(14.0);
            ui.label(
                RichText::new(format!("📐 {}", &step_text))
                    .size(11.5)
                    .strong()
                    .color(if has_face_selection {
                        ACCENT_ORANGE
                    } else {
                        Color32::WHITE
                    }),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);

                if has_face_selection {
                    // Tombol Eksekusi Draft Angle
                    let exec_btn = egui::Button::new(
                        RichText::new(format!("✓ {}", t!("popup-draft-apply")))
                            .size(11.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(ACCENT_ORANGE);

                    if ui.add(exec_btn).clicked() {
                        hud_action = Some(DraftHudAction::Commit);
                    }

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // Dropdown Arah Bukaan Cetakan (Pull Direction)
                    egui::ComboBox::from_id_salt("ducad-draft-top-hud-pull-dir")
                        .selected_text(
                            RichText::new(current_pull_dir.label())
                                .size(11.0)
                                .color(TEXT_PRIMARY),
                        )
                        .width(95.0)
                        .show_ui(ui, |ui| {
                            for dir in &[
                                DraftPullDir::PosZ,
                                DraftPullDir::NegZ,
                                DraftPullDir::PosY,
                                DraftPullDir::NegY,
                                DraftPullDir::PosX,
                                DraftPullDir::NegX,
                            ] {
                                if ui
                                    .selectable_value(
                                        current_pull_dir,
                                        *dir,
                                        RichText::new(dir.label()).size(11.0),
                                    )
                                    .clicked()
                                {
                                    hud_action = Some(DraftHudAction::SetPullDir(*dir));
                                }
                            }
                        });

                    ui.label(RichText::new(format!("{}:", t!("param-pull-dir"))).size(10.5).color(TEXT_SECONDARY));

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // Input Textbox Sudut
                    let text_edit = egui::TextEdit::singleline(angle_input)
                        .desired_width(42.0)
                        .font(egui::FontId::monospace(11.0));
                    let resp = ui.add(text_edit);
                    if resp.changed() {
                        if let Ok(a) = angle_input.trim().parse::<f64>() {
                            hud_action = Some(DraftHudAction::SetAngle(a));
                        }
                    }

                    ui.label(RichText::new("°").size(10.5).color(TEXT_SECONDARY));

                    // Quick Preset Buttons [1°, 2°, 3°, 5°, 7°]
                    for &a in &[7.0, 5.0, 3.0, 2.0, 1.0] {
                        let is_active = (current_angle - a).abs() < 0.05;
                        let btn = egui::Button::new(
                            RichText::new(format!("{:.0}°", a))
                                .size(10.5)
                                .color(if is_active { ACCENT_ORANGE } else { TEXT_PRIMARY }),
                        )
                        .fill(if is_active {
                            Color32::from_rgba_premultiplied(90, 50, 30, 200)
                        } else {
                            Color32::from_rgba_premultiplied(35, 40, 50, 180)
                        });

                        if ui.add(btn).clicked() {
                            *angle_input = format!("{:.1}", a);
                            hud_action = Some(DraftHudAction::SetAngle(a));
                        }
                    }
                    ui.label(RichText::new(format!("{}:", t!("param-draft-angle"))).size(10.5).color(TEXT_SECONDARY));
                }
            });
        });

        hud_action
    }

    /// Render Top Bar HUD mengambang untuk mode Split Body & Split Face 3D
    pub fn render_split_top_bar_hud(
        ui: &mut Ui,
        canvas_rect: Rect,
        has_target_body: bool,
        split_mode: &mut SplitMode,
        current_plane: &mut SplitPlaneKind,
        offset_val: f64,
        offset_input: &mut String,
    ) -> Option<SplitHudAction> {
        let mut hud_action = None;

        let banner_w = 780.0;
        let banner_pos = Pos2::new(canvas_rect.center().x, canvas_rect.top() + 84.0);
        let banner_rect = egui::Rect::from_center_size(banner_pos, Vec2::new(banner_w, 38.0));

        ui.painter().rect_filled(
            banner_rect,
            19.0,
            Color32::from_rgba_premultiplied(15, 18, 24, 240),
        );
        ui.painter().rect_stroke(
            banner_rect,
            19.0,
            Stroke::new(
                1.2,
                if has_target_body {
                    ACCENT_BLUE
                } else {
                    Color32::from_rgb(180, 180, 180).gamma_multiply(0.8)
                },
            ),
            StrokeKind::Inside,
        );

        let mut banner_ui = ui.new_child(egui::UiBuilder::new().max_rect(banner_rect));
        banner_ui.horizontal_centered(|ui| {
            ui.add_space(14.0);

            // Judul Tool
            ui.label(
                RichText::new("✂ Split")
                    .size(12.0)
                    .strong()
                    .color(if has_target_body {
                        ACCENT_BLUE
                    } else {
                        TEXT_SECONDARY
                    }),
            );

            if !has_target_body {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(t!("popup-split-no-body"))
                        .size(11.0)
                        .color(TEXT_MUTED),
                );
                return;
            }

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);

            // Mode Selector: [Body] [Face]
            for mode in &[SplitMode::SplitBody, SplitMode::SplitFace] {
                let is_active = *split_mode == *mode;
                let btn = egui::Button::new(
                    RichText::new(mode.label())
                        .size(11.0)
                        .strong()
                        .color(if is_active { ACCENT_BLUE } else { TEXT_PRIMARY }),
                )
                .fill(if is_active {
                    Color32::from_rgba_premultiplied(30, 60, 100, 200)
                } else {
                    Color32::from_rgba_premultiplied(35, 40, 50, 180)
                });

                if ui.add(btn).clicked() {
                    *split_mode = *mode;
                    hud_action = Some(SplitHudAction::SetMode(*mode));
                }
            }

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);

            // Dropdown Bidang (Plane)
            ui.label(RichText::new(format!("{}:", t!("popup-split-plane"))).size(10.5).color(TEXT_SECONDARY));
            egui::ComboBox::from_id_salt("ducad-split-top-hud-plane")
                .selected_text(
                    RichText::new(current_plane.label())
                        .size(11.0)
                        .color(TEXT_PRIMARY),
                )
                .width(105.0)
                .show_ui(ui, |ui| {
                    for pln in &[
                        SplitPlaneKind::XY,
                        SplitPlaneKind::XZ,
                        SplitPlaneKind::YZ,
                        SplitPlaneKind::PickedFace,
                    ] {
                        if ui
                            .selectable_value(
                                current_plane,
                                *pln,
                                RichText::new(pln.label()).size(11.0),
                            )
                            .clicked()
                        {
                            hud_action = Some(SplitHudAction::SetPlane(*pln));
                        }
                    }
                });

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);

            // Offset Input & Quick Buttons
            ui.label(RichText::new(format!("{}:", t!("popup-split-offset"))).size(10.5).color(TEXT_SECONDARY));

            for &off in &[-10.0, 0.0, 10.0] {
                let is_active = (offset_val - off).abs() < 0.05;
                let btn = egui::Button::new(
                    RichText::new(format!("{:+0.0}", off))
                        .size(10.0)
                        .color(if is_active { ACCENT_BLUE } else { TEXT_PRIMARY }),
                )
                .fill(if is_active {
                    Color32::from_rgba_premultiplied(30, 60, 100, 200)
                } else {
                    Color32::from_rgba_premultiplied(35, 40, 50, 180)
                });

                if ui.add(btn).clicked() {
                    *offset_input = format!("{:.1}", off);
                    hud_action = Some(SplitHudAction::SetOffset(off));
                }
            }

            let text_edit = egui::TextEdit::singleline(offset_input)
                .desired_width(45.0)
                .font(egui::FontId::monospace(11.0));
            let resp = ui.add(text_edit);
            if resp.changed() {
                if let Ok(val) = offset_input.trim().parse::<f64>() {
                    hud_action = Some(SplitHudAction::SetOffset(val));
                }
            }
            ui.label(RichText::new("mm").size(10.5).color(TEXT_SECONDARY));

            // Tombol Eksekusi di Kanan
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(10.0);
                let btn_label = match *split_mode {
                    SplitMode::SplitBody => format!("✓ {}", t!("popup-split-apply")),
                    SplitMode::SplitFace => format!("✓ {}", t!("popup-split-apply-face")),
                };
                let exec_btn = egui::Button::new(
                    RichText::new(btn_label)
                        .size(11.0)
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(ACCENT_BLUE);

                if ui.add(exec_btn).clicked() {
                    hud_action = Some(SplitHudAction::Commit);
                }
            });
        });

        hud_action
    }

    /// Render Top Bar HUD mengambang untuk mode Operasi Boolean 3D (Union, Subtract, Intersect)
    pub fn render_boolean_top_bar_hud(
        ui: &mut Ui,
        canvas_rect: Rect,
        selected_bodies_count: usize,
        selected_op: BooleanOpKind,
    ) -> Option<BooleanHudAction> {
        let mut hud_action = None;

        let has_enough_bodies = selected_bodies_count >= 2;
        let banner_w = 660.0;
        let banner_pos = Pos2::new(canvas_rect.center().x, canvas_rect.top() + 84.0);
        let banner_rect = egui::Rect::from_center_size(banner_pos, Vec2::new(banner_w, 36.0));

        ui.painter().rect_filled(
            banner_rect,
            18.0,
            Color32::from_rgba_premultiplied(15, 18, 24, 240),
        );
        ui.painter().rect_stroke(
            banner_rect,
            18.0,
            Stroke::new(
                1.2,
                if has_enough_bodies {
                    ACCENT_GREEN
                } else {
                    ACCENT_BLUE.gamma_multiply(0.8)
                },
            ),
            StrokeKind::Inside,
        );

        let step_text = if has_enough_bodies {
            t!("hud-boolean-prompt-ready")
        } else {
            t!("hud-boolean-prompt-select")
        };

        // Layout horizontal di dalam banner
        let mut banner_ui = ui.new_child(egui::UiBuilder::new().max_rect(banner_rect));
        banner_ui.horizontal_centered(|ui| {
            ui.add_space(14.0);
            ui.label(
                RichText::new(&step_text)
                    .size(11.5)
                    .strong()
                    .color(if has_enough_bodies {
                        ACCENT_GREEN
                    } else {
                        Color32::WHITE
                    }),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);

                if has_enough_bodies {
                    // Tombol Eksekusi / Terapkan (Commit)
                    let exec_btn = egui::Button::new(
                        RichText::new(t!("hud-apply-enter"))
                            .size(11.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(ACCENT_GREEN);

                    if ui.add(exec_btn).clicked() {
                        hud_action = Some(BooleanHudAction::Commit);
                    }

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // Pilihan Operasi Boolean: Union, Subtract, Intersect
                    let ops = [
                        (BooleanOpKind::Intersect, t!("boolean-intersect")),
                        (BooleanOpKind::Subtract, t!("boolean-subtract")),
                        (BooleanOpKind::Union, t!("boolean-union")),
                    ];

                    for (op, label) in ops {
                        let is_active = selected_op == op;
                        let btn =
                            egui::Button::new(RichText::new(label).size(11.0).strong().color(
                                if is_active {
                                    Color32::WHITE
                                } else {
                                    TEXT_PRIMARY
                                },
                            ))
                            .fill(if is_active {
                                ACCENT_BLUE
                            } else {
                                Color32::from_rgba_premultiplied(40, 44, 52, 180)
                            });

                        if ui.add(btn).clicked() {
                            hud_action = Some(BooleanHudAction::SelectOp(op));
                        }
                    }

                    ui.label(RichText::new(format!("{}:", t!("param-operation"))).size(10.5).color(TEXT_SECONDARY));
                }
            });
        });

        hud_action
    }

    /// Render Top Bar HUD mengambang untuk mode 3D Sweep (Sapu Profil 2D Menyusuri Jalur)
    pub fn render_sweep_top_bar_hud(
        ui: &mut Ui,
        canvas_rect: Rect,
        has_profile: bool,
        has_path: bool,
    ) -> Option<SweepHudAction> {
        let mut hud_action = None;
        let is_ready = has_profile && has_path;

        let banner_w = 680.0;
        let banner_pos = Pos2::new(canvas_rect.center().x, canvas_rect.top() + 84.0);
        let banner_rect = egui::Rect::from_center_size(banner_pos, Vec2::new(banner_w, 36.0));

        ui.painter().rect_filled(
            banner_rect,
            18.0,
            Color32::from_rgba_premultiplied(15, 18, 24, 240),
        );
        ui.painter().rect_stroke(
            banner_rect,
            18.0,
            Stroke::new(
                1.2,
                if is_ready {
                    ACCENT_GREEN
                } else if has_profile {
                    ACCENT_BLUE
                } else {
                    ACCENT_BLUE.gamma_multiply(0.8)
                },
            ),
            StrokeKind::Inside,
        );

        let step_text = if is_ready {
            t!("hud-sweep-prompt-ready")
        } else if has_profile {
            t!("hud-sweep-prompt-path")
        } else {
            t!("hud-sweep-prompt-profile")
        };

        // Layout horizontal di dalam banner
        let mut banner_ui = ui.new_child(egui::UiBuilder::new().max_rect(banner_rect));
        banner_ui.horizontal_centered(|ui| {
            ui.add_space(14.0);
            ui.label(
                RichText::new(&step_text)
                    .size(11.5)
                    .strong()
                    .color(if is_ready {
                        ACCENT_GREEN
                    } else if has_profile {
                        ACCENT_BLUE
                    } else {
                        Color32::WHITE
                    }),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);

                // Tombol Batal
                let cancel_btn = egui::Button::new(
                    RichText::new(t!("hud-sweep-cancel"))
                        .size(10.5)
                        .color(TEXT_SECONDARY),
                )
                .fill(Color32::from_rgba_premultiplied(40, 44, 52, 180));
                if ui.add(cancel_btn).clicked() {
                    hud_action = Some(SweepHudAction::Cancel);
                }

                ui.add_space(4.0);

                if is_ready {
                    // Tombol Eksekusi Sweep (Commit)
                    let exec_btn = egui::Button::new(
                        RichText::new(t!("hud-sweep-exec-btn"))
                            .size(11.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(ACCENT_GREEN);

                    if ui.add(exec_btn).clicked() {
                        hud_action = Some(SweepHudAction::Commit);
                    }

                    ui.add_space(4.0);
                }

                if has_profile {
                    // Tombol Ganti Profil
                    let reset_btn = egui::Button::new(
                        RichText::new(t!("hud-sweep-reset-profile"))
                            .size(10.5)
                            .color(TEXT_PRIMARY),
                    )
                    .fill(Color32::from_rgba_premultiplied(40, 44, 52, 180));

                    if ui.add(reset_btn).clicked() {
                        hud_action = Some(SweepHudAction::ResetProfile);
                    }

                    ui.add_space(4.0);
                    ui.separator();
                }
            });
        });

        hud_action
    }

    /// Render Top Bar HUD mengambang untuk fitur Pattern / Array (2D Sketch & 3D Solids).
    #[allow(clippy::too_many_arguments)]
    pub fn render_pattern_top_bar_hud(
        ui: &mut Ui,
        canvas_rect: Rect,
        is_3d: bool,
        has_selection: bool,
        pattern_kind: &mut PatternKind,
        count_x: &mut usize,
        pitch_x: &mut f64,
        count_y: &mut usize,
        pitch_y: &mut f64,
        count_z: &mut usize,
        pitch_z: &mut f64,
        circ_count: &mut usize,
        circ_angle_deg: &mut f64,
        circ_radius: &mut f64,
        circ_axis: &mut PatternAxisPreset,
    ) -> Option<PatternHudAction> {
        let mut hud_action = None;

        let banner_w = if is_3d { 940.0 } else { 880.0 };
        let banner_pos = Pos2::new(canvas_rect.center().x, canvas_rect.top() + 84.0);
        let banner_rect = egui::Rect::from_center_size(banner_pos, Vec2::new(banner_w, 38.0));

        ui.painter().rect_filled(
            banner_rect,
            19.0,
            Color32::from_rgba_premultiplied(15, 18, 24, 240),
        );
        ui.painter().rect_stroke(
            banner_rect,
            19.0,
            Stroke::new(
                1.2,
                if has_selection {
                    ACCENT_BLUE
                } else {
                    Color32::from_rgb(180, 180, 180).gamma_multiply(0.8)
                },
            ),
            StrokeKind::Inside,
        );

        let mut banner_ui = ui.new_child(egui::UiBuilder::new().max_rect(banner_rect));
        banner_ui.horizontal_centered(|ui| {
            ui.add_space(14.0);

            // Judul Tool
            ui.label(
                RichText::new("⊞ Pattern")
                    .size(12.0)
                    .strong()
                    .color(if has_selection {
                        ACCENT_BLUE
                    } else {
                        TEXT_SECONDARY
                    }),
            );

            if !has_selection {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(if is_3d {
                        t!("popup-pattern-no-selection-3d")
                    } else {
                        t!("popup-pattern-no-selection-2d")
                    })
                    .size(11.0)
                    .color(TEXT_MUTED),
                );
                return;
            }

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);

            // Mode Selector: [Linier] [Sirkular]
            for mode in &[PatternKind::Linear, PatternKind::Circular] {
                let is_active = *pattern_kind == *mode;
                let btn = egui::Button::new(
                    RichText::new(mode.label())
                        .size(11.0)
                        .strong()
                        .color(if is_active { ACCENT_BLUE } else { TEXT_PRIMARY }),
                )
                .fill(if is_active {
                    Color32::from_rgba_premultiplied(30, 60, 100, 200)
                } else {
                    Color32::from_rgba_premultiplied(35, 40, 50, 180)
                });

                if ui.add(btn).clicked() {
                    *pattern_kind = *mode;
                    hud_action = Some(PatternHudAction::SetKind(*mode));
                }
            }

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);

            match pattern_kind {
                PatternKind::Linear => {
                    // X params
                    ui.label(RichText::new("X:").size(11.0).strong().color(Color32::from_rgb(255, 100, 100)));
                    ui.add(egui::DragValue::new(count_x).range(1..=50).prefix("qty: "));
                    ui.add(egui::DragValue::new(pitch_x).speed(1.0).suffix("mm"));

                    ui.add_space(4.0);
                    // Y params
                    ui.label(RichText::new("Y:").size(11.0).strong().color(Color32::from_rgb(100, 220, 100)));
                    ui.add(egui::DragValue::new(count_y).range(1..=50).prefix("qty: "));
                    ui.add(egui::DragValue::new(pitch_y).speed(1.0).suffix("mm"));

                    if is_3d {
                        ui.add_space(4.0);
                        // Z params
                        ui.label(RichText::new("Z:").size(11.0).strong().color(Color32::from_rgb(100, 150, 255)));
                        ui.add(egui::DragValue::new(count_z).range(1..=50).prefix("qty: "));
                        ui.add(egui::DragValue::new(pitch_z).speed(1.0).suffix("mm"));
                    }
                }
                PatternKind::Circular => {
                    // Count
                    ui.label(RichText::new("Qty:").size(10.5).color(TEXT_SECONDARY));
                    ui.add(egui::DragValue::new(circ_count).range(2..=120));

                    ui.add_space(4.0);
                    // Radius
                    ui.label(RichText::new("Radius:").size(10.5).color(TEXT_SECONDARY));
                    ui.add(egui::DragValue::new(circ_radius).speed(1.0).range(0.1..=10000.0).suffix("mm"));

                    ui.add_space(4.0);
                    // Angle
                    ui.label(RichText::new("Sudut:").size(10.5).color(TEXT_SECONDARY));
                    ui.add(egui::DragValue::new(circ_angle_deg).speed(1.0).range(-360.0..=360.0).suffix("°"));

                    // Quick Angle buttons
                    for angle in &[360.0, 180.0, 90.0] {
                        let is_active = (*circ_angle_deg - angle).abs() < 1e-3;
                        let btn = egui::Button::new(RichText::new(format!("{:.0}°", angle)).size(10.0))
                            .fill(if is_active { ACCENT_BLUE.gamma_multiply(0.6) } else { Color32::from_rgba_premultiplied(40, 45, 55, 160) });
                        if ui.add(btn).clicked() {
                            *circ_angle_deg = *angle;
                        }
                    }

                    if is_3d {
                        ui.add_space(4.0);
                        // Dropdown Axis
                        egui::ComboBox::from_id_salt("ducad-pattern-axis-combo")
                            .selected_text(RichText::new(circ_axis.label()).size(10.5).color(TEXT_PRIMARY))
                            .width(110.0)
                            .show_ui(ui, |ui| {
                                for ax in &[PatternAxisPreset::Z, PatternAxisPreset::Y, PatternAxisPreset::X] {
                                    if ui.selectable_value(circ_axis, *ax, RichText::new(ax.label()).size(10.5)).clicked() {
                                        hud_action = Some(PatternHudAction::SetAxis(*ax));
                                    }
                                }
                            });
                    }
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);

                // Tombol Eksekusi Pattern
                let exec_btn = egui::Button::new(
                    RichText::new(format!("✓ {}", t!("popup-pattern-apply")))
                        .size(11.0)
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(ACCENT_BLUE);

                if ui.add(exec_btn).clicked() {
                    hud_action = Some(PatternHudAction::Commit);
                }

                ui.add_space(4.0);

                // Tombol Batal
                let cancel_btn = egui::Button::new(
                    RichText::new("✕ Batal")
                        .size(10.5)
                        .color(TEXT_SECONDARY),
                )
                .fill(Color32::from_rgba_premultiplied(45, 50, 60, 160));

                if ui.add(cancel_btn).clicked() {
                    hud_action = Some(PatternHudAction::Cancel);
                }
            });
        });

        hud_action
    }

    /// Render popup rename mengambang di area top-center kanvas.
    ///
    /// `label` — judul yang ditampilkan (mis. "Nama Grup 2D" atau "Nama Body 3D").
    /// `input_buf` — buffer teks yang dibaca+ditulis secara langsung (mutable ref).
    ///
    /// Mengembalikan `Some(RenamePopupEvent::Confirm(nama))` saat user klik Simpan/Enter,
    /// `Some(RenamePopupEvent::Cancel)` saat Batal/Esc, atau `None` saat popup masih aktif.
    pub fn show_rename_popup(
        ui: &mut Ui,
        label: &str,
        input_buf: &mut String,
    ) -> Option<RenamePopupEvent> {
        use crate::theme::glass_frame;

        let mut event = None;

        let frame_resp = glass_frame().show(ui, |ui| {
            ui.set_width(320.0);
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{} {}", ICON_DRIVE_FILE_RENAME_OUTLINE.codepoint, label))
                        .size(12.0)
                        .strong()
                        .color(ACCENT_BLUE),
                );
            });

            ui.separator();

            ui.horizontal(|ui| {
                let te = egui::TextEdit::singleline(input_buf)
                    .hint_text("Ketik nama…")
                    .desired_width(220.0)
                    .font(egui::FontId::proportional(13.0));
                let te_resp = ui.add(te);

                // Auto-focus saat baru muncul
                te_resp.request_focus();

                // Tekan Enter untuk konfirmasi
                if te_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    event = Some(RenamePopupEvent::Confirm(input_buf.trim().to_string()));
                }

                // Tombol Simpan
                let save_btn = egui::Button::new(
                    RichText::new(format!("{} Simpan", ICON_CHECK.codepoint))
                        .size(11.5)
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(ACCENT_BLUE);
                if ui.add(save_btn).on_hover_text("Simpan nama (Enter)").clicked() {
                    event = Some(RenamePopupEvent::Confirm(input_buf.trim().to_string()));
                }
            });
        });

        // Tap/click di luar area popup untuk batal
        if event.is_none() {
            if let Some(pointer_pos) = ui.input(|i| i.pointer.interact_pos()) {
                if ui.input(|i| i.pointer.any_pressed()) && !frame_resp.response.rect.contains(pointer_pos) {
                    event = Some(RenamePopupEvent::Cancel);
                }
            }
        }

        // Tekan Esc untuk batal juga di level global
        if event.is_none() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            event = Some(RenamePopupEvent::Cancel);
        }

        event
    }

    /// Render Shapr3D-style interactive floating stepper pill `[ - ] Label: 4 [ + ]`
    pub fn render_stepper_pill(
        ui: &mut egui::Ui,
        center_pos: egui::Pos2,
        label: &str,
        count: usize,
        min: usize,
        max: usize,
    ) -> (egui::Response, Option<usize>) {
        let size = egui::vec2(110.0, 26.0);
        let rect = egui::Rect::from_center_size(center_pos, size);
        let mut new_count = None;

        let response = ui.allocate_rect(rect, egui::Sense::hover());

        // Background pill
        ui.painter().rect_filled(
            rect,
            13.0,
            Color32::from_rgba_premultiplied(16, 20, 28, 235),
        );
        ui.painter().rect_stroke(
            rect,
            13.0,
            egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(0, 160, 255, 180)),
            egui::StrokeKind::Inside,
        );

        let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        child_ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 0.0);
            ui.add_space(4.0);

            // Minus button
            let minus_btn = egui::Button::new(RichText::new("−").size(12.0).strong().color(if count > min { Color32::WHITE } else { TEXT_MUTED }))
                .fill(Color32::from_rgba_premultiplied(40, 48, 60, 200))
                .min_size(egui::vec2(18.0, 18.0));
            if ui.add(minus_btn).on_hover_text("Kurangi jumlah").clicked() && count > min {
                new_count = Some(count - 1);
            }

            // Value text
            let text = format!("{}: {}", label, count);
            ui.label(RichText::new(text).size(10.5).strong().color(ACCENT_BLUE));

            // Plus button
            let plus_btn = egui::Button::new(RichText::new("+").size(12.0).strong().color(if count < max { Color32::WHITE } else { TEXT_MUTED }))
                .fill(Color32::from_rgba_premultiplied(40, 48, 60, 200))
                .min_size(egui::vec2(18.0, 18.0));
            if ui.add(plus_btn).on_hover_text("Tambah jumlah").clicked() && count < max {
                new_count = Some(count + 1);
            }
        });

        (response, new_count)
    }

    /// Render Shapr3D-style draggable circular pivot pin
    pub fn render_circular_pivot_pin(
        ui: &mut egui::Ui,
        center_pos: egui::Pos2,
        is_active: bool,
    ) -> egui::Response {
        let size = egui::vec2(22.0, 22.0);
        let rect = egui::Rect::from_center_size(center_pos, size);
        let response = ui.allocate_rect(rect, egui::Sense::drag().union(egui::Sense::click()));

        let is_hovered = response.hovered();
        let color = if is_active {
            ACCENT_ORANGE
        } else if is_hovered {
            ACCENT_BLUE
        } else {
            Color32::from_rgb(0, 180, 255)
        };

        let painter = ui.painter();
        painter.circle_filled(center_pos, 8.0, Color32::from_rgba_premultiplied(10, 20, 35, 200));
        painter.circle_stroke(center_pos, 8.0, egui::Stroke::new(1.8, color));
        painter.circle_filled(center_pos, 3.0, color);

        // Crosshair ticks
        painter.line_segment([center_pos - egui::vec2(12.0, 0.0), center_pos + egui::vec2(12.0, 0.0)], egui::Stroke::new(1.0, color));
        painter.line_segment([center_pos - egui::vec2(0.0, 12.0), center_pos + egui::vec2(0.0, 12.0)], egui::Stroke::new(1.0, color));

        if is_hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        if response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }

        response
    }
}
