//! In-Canvas HUD & Floating Dimension Pills bergaya Shapr3D dengan Material Icons.
//!
//! Menampilkan widget HUD mengambang langsung di atas kanvas 3D:
//! tombol kapsul "Normal to Sketch", banner peringatan Section View,
//! badge dimensi in-situ, dan status seleksi mengambang di pojok kiri atas kanvas.

use crate::theme::{
    pill_frame, ACCENT_BLUE, ACCENT_GREEN, ACCENT_ORANGE, BORDER_SUBTLE, TEXT_MUTED, TEXT_PRIMARY,
    TEXT_SECONDARY,
};
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Stroke, StrokeKind, Ui, Vec2};
use egui_material_icons::icons::{ICON_3D_ROTATION, ICON_CHECK, ICON_CLOSE, ICON_DRIVE_FILE_RENAME_OUTLINE, ICON_LOCK, ICON_STRAIGHTEN};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RevolveHudAction {
    SetAngle(f64),
    ToggleReverse,
    Commit,
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
                RichText::new(format!("{} Normal to Sketch", ICON_3D_ROTATION.codepoint))
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
                    RichText::new("Turn off Section View to show hidden parts")
                        .size(11.0)
                        .color(ACCENT_ORANGE),
                );
                if ui
                    .small_button(RichText::new("Turn off").size(10.0))
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
                        RichText::new(format!("{} Normal to Sketch", ICON_3D_ROTATION.codepoint))
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
    pub fn render_revolve_animated_guide(
        ui: &mut Ui,
        canvas_rect: egui::Rect,
        pending_points_count: usize,
        has_selection: bool,
        current_angle: f64,
        angle_input: &mut String,
        is_reversed: bool,
        is_staged: bool,
        time: f64,
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
            "Pilih profil sketsa 2D tertutup dulu"
        } else if is_staged {
            "Sumbu Siap! Atur Sudut & Terapkan"
        } else if pending_points_count == 0 {
            "Langkah 1: Klik Titik 1 Sumbu Poros"
        } else {
            "Langkah 2: Klik Titik 2 Sumbu Poros"
        };

        // Layout horizontal di dalam banner
        let mut banner_ui = ui.new_child(egui::UiBuilder::new().max_rect(banner_rect));
        banner_ui.horizontal_centered(|ui| {
            ui.add_space(14.0);
            ui.label(
                RichText::new(step_text)
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
                        RichText::new("Terapkan (Enter)")
                            .size(11.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(ACCENT_GREEN);

                    if ui.add(apply_btn).clicked() {
                        hud_action = Some(RevolveHudAction::Commit);
                    }

                    // Tombol Batal
                    let cancel_btn = egui::Button::new(
                        RichText::new("Batal (Esc)").size(10.5).color(TEXT_PRIMARY),
                    )
                    .fill(Color32::from_rgba_premultiplied(65, 25, 25, 200));

                    if ui.add(cancel_btn).clicked() {
                        hud_action = Some(RevolveHudAction::Cancel);
                    }

                    ui.add_space(6.0);
                }

                // Tombol Toggle Arah Putar (CW vs CCW)
                let dir_text = if is_reversed {
                    "Arah: Balik (CW)"
                } else {
                    "Arah: Normal (CCW)"
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
                    .desired_width(42.0)
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
                for &deg in &[90.0, 180.0, 270.0, 360.0] {
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
                ui.label(RichText::new("Sudut:").size(10.5).color(TEXT_SECONDARY));
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

        // Waktu animasi siklus (3.4 detik)
        let cycle = 3.4;
        let phase = ((time % cycle) / cycle) as f32; // 0.0 .. 1.0

        // Subtitle status langkah animasi dinamis
        let (step_title, step_color) = if phase < 0.28 {
            ("1. Klik Titik 1 Sumbu", ACCENT_ORANGE)
        } else if phase < 0.58 {
            ("2. Klik Titik 2 Sumbu", ACCENT_ORANGE)
        } else {
            ("3. Putar Sudut & Arah (↺/↻)", ACCENT_GREEN)
        };

        painter.text(
            Pos2::new(card_rect.left() + 10.0, card_rect.top() + 8.0),
            Align2::LEFT_TOP,
            "Panduan Revolve & Arah Putar:",
            FontId::proportional(10.0),
            TEXT_SECONDARY,
        );
        painter.text(
            Pos2::new(card_rect.left() + 10.0, card_rect.top() + 21.0),
            Align2::LEFT_TOP,
            step_title,
            FontId::proportional(11.0),
            step_color,
        );

        // Ilustrasi Diagram: Sumbu + Profil Kotak + Panah Putaran 3D
        let axis_x = card_rect.left() + 48.0;
        let p1 = Pos2::new(axis_x, card_rect.bottom() - 36.0);
        let p2 = Pos2::new(axis_x, card_rect.top() + 38.0);

        // Gambar profil 2D demo di kanan sumbu
        let profile_rect = egui::Rect::from_min_max(
            Pos2::new(axis_x + 10.0, card_rect.top() + 40.0),
            Pos2::new(axis_x + 52.0, card_rect.bottom() - 38.0),
        );
        painter.rect_filled(profile_rect, 3.0, ACCENT_BLUE.gamma_multiply(0.25));
        painter.rect_stroke(
            profile_rect,
            3.0,
            Stroke::new(1.2, ACCENT_BLUE),
            StrokeKind::Inside,
        );

        let (pointer_pos, is_clicking, axis_progress, show_spin, spin_deg_label, anim_dir_cw) =
            if phase < 0.28 {
                // Bergerak ke Titik 1 lalu klik
                let t = (phase / 0.28).clamp(0.0, 1.0);
                let pos = Pos2::new(p1.x + (1.0 - t) * 20.0, p1.y + (1.0 - t) * 10.0);
                (pos, t > 0.7, 0.0, false, "360°", is_reversed)
            } else if phase < 0.58 {
                // Bergerak dari Titik 1 ke Titik 2
                let t = ((phase - 0.28) / 0.30).clamp(0.0, 1.0);
                let pos = Pos2::new(p1.x, p1.y + (p2.y - p1.y) * t);
                (pos, t > 0.85, t, false, "360°", is_reversed)
            } else if phase < 0.88 {
                // Putar revolusi 3D dengan sudut dan arah dinamis
                let t = ((phase - 0.58) / 0.30).clamp(0.0, 1.0);
                let deg_str = if t < 0.33 {
                    "90°"
                } else if t < 0.66 {
                    "180°"
                } else {
                    "360°"
                };
                (p2, false, 1.0, true, deg_str, is_reversed)
            } else {
                // Selesai loop
                (p2, false, 1.0, true, "360°", is_reversed)
            };

        // Garis sumbu yang sedang digambar
        if axis_progress > 0.0 {
            let current_p2 = Pos2::new(p1.x, p1.y + (p2.y - p1.y) * axis_progress);
            painter.line_segment([p1, current_p2], Stroke::new(2.0, ACCENT_ORANGE));
        }

        // Titik 1 & Titik 2 indicator
        painter.circle_filled(
            p1,
            3.5,
            if phase >= 0.22 {
                ACCENT_ORANGE
            } else {
                TEXT_MUTED
            },
        );
        painter.circle_filled(
            p2,
            3.5,
            if phase >= 0.55 {
                ACCENT_ORANGE
            } else {
                TEXT_MUTED
            },
        );

        // Efek ripple klik saat pointer mengklik titik
        if is_clicking {
            let ripple_radius = 4.0 + (time * 15.0).sin().abs() as f32 * 6.0;
            painter.circle_stroke(
                pointer_pos,
                ripple_radius,
                Stroke::new(1.5, ACCENT_BLUE.gamma_multiply(0.8)),
            );
        }

        // Panah revolusi 3D melingkari sumbu + label sudut & arah
        if show_spin {
            let spin_center = Pos2::new(axis_x + 31.0, card_rect.center().y + 4.0);
            let dir_mult = if anim_dir_cw { -1.0 } else { 1.0 };
            let angle = (time * 6.0 * dir_mult) as f32;
            let rx = 26.0;
            let ry = 9.0;
            let arc_p = Pos2::new(
                spin_center.x + rx * angle.cos(),
                spin_center.y + ry * angle.sin(),
            );
            let spin_color = if anim_dir_cw {
                ACCENT_ORANGE
            } else {
                ACCENT_GREEN
            };
            painter.circle_stroke(
                spin_center,
                rx,
                Stroke::new(1.2, spin_color.gamma_multiply(0.5)),
            );
            painter.circle_filled(arc_p, 3.5, spin_color);

            // Badge Sudut & Arah Putar yang sedang didemonstrasikan
            let badge_pos = Pos2::new(card_rect.right() - 40.0, card_rect.center().y + 4.0);
            let badge_rect = egui::Rect::from_center_size(badge_pos, Vec2::new(56.0, 22.0));
            painter.rect_filled(badge_rect, 4.0, spin_color.gamma_multiply(0.25));
            painter.rect_stroke(
                badge_rect,
                4.0,
                Stroke::new(1.0, spin_color),
                StrokeKind::Inside,
            );

            let dir_symbol = if anim_dir_cw { "↻" } else { "↺" };
            painter.text(
                badge_pos,
                Align2::CENTER_CENTER,
                format!("{dir_symbol} {spin_deg_label}"),
                FontId::proportional(10.5),
                Color32::WHITE,
            );
        }

        // Footer penjelasan
        painter.text(
            Pos2::new(card_rect.left() + 10.0, card_rect.bottom() - 10.0),
            Align2::LEFT_BOTTOM,
            "💡 Arah: tombol 🔄 Balik / balik urutan titik 1 & 2",
            FontId::proportional(9.0),
            TEXT_MUTED,
        );

        // Gambar Pointer Cursor Animasi (Stylized Arrow)
        let arrow_points = [
            pointer_pos,
            pointer_pos + Vec2::new(11.0, 9.0),
            pointer_pos + Vec2::new(5.0, 10.0),
            pointer_pos + Vec2::new(8.0, 16.0),
            pointer_pos + Vec2::new(5.0, 17.0),
            pointer_pos + Vec2::new(2.0, 11.0),
            pointer_pos + Vec2::new(-2.0, 14.0),
        ];
        painter.add(egui::Shape::convex_polygon(
            arrow_points.to_vec(),
            Color32::WHITE,
            Stroke::new(1.2, Color32::BLACK),
        ));

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

        // 1. Top Horizontal HUD Banner
        let banner_w = 720.0;
        let banner_pos = Pos2::new(canvas_rect.center().x, canvas_rect.top() + 84.0);
        let banner_rect = egui::Rect::from_center_size(banner_pos, Vec2::new(banner_w, 36.0));

        let is_ready = selected_regions_count == 2;

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
                } else if is_ready {
                    ACCENT_BLUE
                } else {
                    ACCENT_BLUE.gamma_multiply(0.8)
                },
            ),
            StrokeKind::Inside,
        );

        let step_text = match selected_regions_count {
            0 => "Pilih 2 profil sketsa 2D (klik / drag kotak)",
            1 => "1 profil terpilih. Pilih profil ke-2 (klik / drag)",
            2 if is_staged => "✨ Loft 3D Terbentuk — Ubah Tinggi / Balik Profil / Selesai",
            2 => "2 Profil Siap! Atur Tinggi & Buat 3D",
            _ => "Lebih dari 2 profil terpilih (pilih tepat 2 profil)",
        };

        // Layout horizontal di dalam banner
        let mut banner_ui = ui.new_child(egui::UiBuilder::new().max_rect(banner_rect));
        banner_ui.horizontal_centered(|ui| {
            ui.add_space(14.0);
            ui.label(RichText::new(step_text).size(11.5).strong().color(
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
                    let main_label = if is_staged {
                        "✓ Selesai (Enter)"
                    } else {
                        "🚀 Buat 3D (Enter)"
                    };
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

                    // Tombol Batal
                    let cancel_btn = egui::Button::new(
                        RichText::new("Batal (Esc)").size(10.5).color(TEXT_PRIMARY),
                    )
                    .fill(Color32::from_rgba_premultiplied(65, 25, 25, 200));

                    if ui.add(cancel_btn).clicked() {
                        hud_action = Some(LoftHudAction::Cancel);
                    }

                    ui.add_space(6.0);

                    // Tombol Balik Posisi Atas / Bawah
                    let flip_label = if is_flipped {
                        "↕ Bawah: Profil 2"
                    } else {
                        "↕ Bawah: Profil 1"
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
                        .on_hover_text("Tukar urutan profil bawah (Z=0) dan profil atas (Z=Tinggi)")
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
                ui.label(RichText::new("Tinggi:").size(10.5).color(TEXT_SECONDARY));
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
                                RichText::new("⚠️ Titik Tengah Belum Menyatu")
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
                            RichText::new("Ingin satukan titik tengah (simetris) atau biarkan menceng (offset)?")
                                .size(10.5)
                                .color(TEXT_PRIMARY),
                        );
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            let align_btn = egui::Button::new(
                                RichText::new("🎯 Satukan Titik Tengah").size(11.0).strong().color(Color32::WHITE),
                            )
                            .fill(ACCENT_BLUE);
                            if ui.add(align_btn).clicked() {
                                hud_action = Some(LoftHudAction::AlignCentroids);
                            }

                            ui.add_space(8.0);
                            let keep_btn = egui::Button::new(
                                RichText::new("Biarkan Menceng (Offset)").size(10.5).color(TEXT_PRIMARY),
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
            "Atur Ketebalan Dinding"
        } else {
            "Pilih salah satu sisi objek 3D"
        };

        // Layout horizontal di dalam banner
        let mut banner_ui = ui.new_child(egui::UiBuilder::new().max_rect(banner_rect));
        banner_ui.horizontal_centered(|ui| {
            ui.add_space(14.0);
            ui.label(
                RichText::new(step_text)
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
                        RichText::new("🚀 Eksekusi Shell (Enter)")
                            .size(11.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(ACCENT_GREEN);

                    if ui.add(exec_btn).clicked() {
                        hud_action = Some(ShellHudAction::Commit);
                    }

                    // Tombol Batal
                    let cancel_btn = egui::Button::new(
                        RichText::new("Batal (Esc)").size(10.5).color(TEXT_PRIMARY),
                    )
                    .fill(Color32::from_rgba_premultiplied(65, 25, 25, 200));

                    if ui.add(cancel_btn).clicked() {
                        hud_action = Some(ShellHudAction::Cancel);
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
                    ui.label(RichText::new("Tebal:").size(10.5).color(TEXT_SECONDARY));
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
            format!("({} Body Terpilih)", selected_bodies_count)
        } else {
            "Pilih min 2 body (Tahan Shift + Klik)".to_string()
        };

        // Layout horizontal di dalam banner
        let mut banner_ui = ui.new_child(egui::UiBuilder::new().max_rect(banner_rect));
        banner_ui.horizontal_centered(|ui| {
            ui.add_space(14.0);
            ui.label(
                RichText::new(step_text)
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
                        RichText::new("🚀 Terapkan (Enter)")
                            .size(11.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(ACCENT_GREEN);

                    if ui.add(exec_btn).clicked() {
                        hud_action = Some(BooleanHudAction::Commit);
                    }

                    // Tombol Batal
                    let cancel_btn = egui::Button::new(
                        RichText::new("Batal (Esc)").size(10.5).color(TEXT_PRIMARY),
                    )
                    .fill(Color32::from_rgba_premultiplied(65, 25, 25, 200));

                    if ui.add(cancel_btn).clicked() {
                        hud_action = Some(BooleanHudAction::Cancel);
                    }

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // Pilihan Operasi Boolean: Union, Subtract, Intersect
                    let ops = [
                        (BooleanOpKind::Intersect, "Intersect (Irisan)"),
                        (BooleanOpKind::Subtract, "Subtract (Potong)"),
                        (BooleanOpKind::Union, "Union (Gabung)"),
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

                    ui.label(RichText::new("Operasi:").size(10.5).color(TEXT_SECONDARY));
                } else {
                    let cancel_btn = egui::Button::new(
                        RichText::new("Batal (Esc)").size(10.5).color(TEXT_PRIMARY),
                    )
                    .fill(Color32::from_rgba_premultiplied(65, 25, 25, 200));

                    if ui.add(cancel_btn).clicked() {
                        hud_action = Some(BooleanHudAction::Cancel);
                    }
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

        glass_frame().show(ui, |ui| {
            ui.set_width(340.0);
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

                // Tombol Batal
                let cancel_btn = egui::Button::new(
                    RichText::new(ICON_CLOSE.codepoint)
                        .size(12.0)
                        .color(TEXT_SECONDARY),
                );
                if ui.add(cancel_btn).on_hover_text("Batal (Esc)").clicked() {
                    event = Some(RenamePopupEvent::Cancel);
                }
            });
        });

        // Tekan Esc untuk batal juga di level global
        if event.is_none() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            event = Some(RenamePopupEvent::Cancel);
        }

        event
    }
}
