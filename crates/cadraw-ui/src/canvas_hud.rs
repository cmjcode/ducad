//! In-Canvas HUD & Floating Dimension Pills bergaya Shapr3D dengan Material Icons.
//!
//! Menampilkan widget HUD mengambang langsung di atas kanvas 3D:
//! tombol kapsul "Normal to Sketch", banner peringatan Section View,
//! badge dimensi in-situ, dan status seleksi mengambang di bawah tengah kanvas.

use egui::{Color32, FontId, Pos2, RichText, Stroke, StrokeKind, Ui, Vec2};
use egui_material_icons::icons::{ICON_3D_ROTATION, ICON_LOCK, ICON_STRAIGHTEN};
use crate::theme::{pill_frame, ACCENT_BLUE, ACCENT_ORANGE, TEXT_PRIMARY, TEXT_SECONDARY};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasHudEvent {
    OrientNormalToSketch,
    TurnOffSectionView,
    OpenMeasurements,
}

pub struct CanvasHud;

impl CanvasHud {
    /// Render tombol kapsul mengambang "Normal to Sketch" di dalam container UI yang diberikan.
    pub fn show_normal_to_sketch_btn(ui: &mut Ui) -> Option<CanvasHudEvent> {
        let mut event = None;
        pill_frame().show(ui, |ui| {
            let btn = ui.button(
                RichText::new(format!("{} Normal to Sketch", ICON_3D_ROTATION))
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
                if ui.small_button(RichText::new("Turn off").size(10.0)).clicked() {
                    event = Some(CanvasHudEvent::TurnOffSectionView);
                }
            });
        });
        event
    }

    /// Render status badge seleksi & pengukuran mengambang di dalam container UI yang diberikan.
    pub fn show_bottom_status_pill(
        ui: &mut Ui,
        selection_summary: &str,
        measurement_summary: Option<&str>,
    ) -> Option<CanvasHudEvent> {
        let mut event = None;
        pill_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                // Ringkasan seleksi
                ui.label(
                    RichText::new(selection_summary)
                        .size(11.0)
                        .strong()
                        .color(ACCENT_BLUE),
                );

                // Ringkasan pengukuran jika ada
                if let Some(m) = measurement_summary {
                    ui.label(RichText::new("|").color(TEXT_SECONDARY));
                    let resp = ui.selectable_label(
                        false,
                        RichText::new(format!("{} {}", ICON_STRAIGHTEN, m))
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

    /// Render badge dimensi mengambang putih langsung pada posisi 2D di kanvas.
    pub fn render_dimension_pill(
        ui: &mut Ui,
        pos_2d: Pos2,
        value_text: &str,
        locked: bool,
    ) {
        let painter = ui.painter();
        let lock_icon = if locked { format!("{} ", ICON_LOCK) } else { "".to_string() };
        let full_text = format!("{}{}", lock_icon, value_text);

        let font = FontId::proportional(11.0);
        let galley = painter.layout_no_wrap(full_text, font, Color32::from_rgb(20, 20, 22));
        let rect = egui::Rect::from_center_size(
            pos_2d,
            galley.size() + Vec2::new(16.0, 8.0),
        );

        // Gambar latar belakang pill putih/terang dengan bayangan
        painter.rect_filled(rect, 10.0, Color32::from_rgba_premultiplied(245, 246, 250, 245));
        painter.rect_stroke(rect, 10.0, Stroke::new(1.0, Color32::from_gray(180)), StrokeKind::Inside);
        painter.galley(rect.min + Vec2::new(8.0, 4.0), galley, Color32::from_rgb(20, 20, 22));
    }

    /// Render badge dimensi mengambang yang DIPUTAR sejajar arah garis pengukuran
    /// (bukan selalu horizontal seperti `render_dimension_pill`) — dipakai label
    /// nominal jarak/sudut supaya menempel rapi di garisnya sendiri dan tidak
    /// numpuk visual dengan garis pengukuran lain yang miring. `angle_rad` harus
    /// SUDAH dinormalisasi pemanggil ke rentang -90°..90° (mis. lewat
    /// `Vec2::angle()` lalu di-`±π` kalau di luar rentang itu) supaya teksnya
    /// tidak pernah kebalik/terbaca dari bawah ke atas.
    pub fn render_dimension_pill_aligned(ui: &mut Ui, center_2d: Pos2, angle_rad: f32, value_text: &str) {
        let painter = ui.painter();
        let font = FontId::proportional(11.0);
        let galley = painter.layout_no_wrap(value_text.to_string(), font, Color32::from_rgb(20, 20, 22));
        let half = (galley.size() + Vec2::new(16.0, 8.0)) * 0.5;
        let rot = egui::emath::Rot2::from_angle(angle_rad);

        // 4 sudut pill relatif ke pusat SEBELUM dirotasi, lalu diputar & digeser
        // ke `center_2d` — dipakai polygon konveks karena rounded-rect bawaan
        // egui tidak punya varian berotasi.
        let corners: Vec<Pos2> = [
            Vec2::new(-half.x, -half.y),
            Vec2::new(half.x, -half.y),
            Vec2::new(half.x, half.y),
            Vec2::new(-half.x, half.y),
        ]
        .into_iter()
        .map(|c| center_2d + rot * c)
        .collect();

        painter.add(egui::epaint::PathShape::convex_polygon(
            corners,
            Color32::from_rgba_premultiplied(245, 246, 250, 245),
            Stroke::new(1.0, Color32::from_gray(180)),
        ));

        // `TextShape::pos` adalah pojok kiri-atas galley SEBELUM rotasi, dengan
        // pivot rotasi di titik itu juga — jadi dihitung mundur dari pusat
        // supaya, setelah diputar `rot`, teksnya jatuh center di `center_2d`.
        let text_pos = center_2d + rot * (Vec2::new(-half.x, -half.y) + Vec2::new(8.0, 4.0));
        let mut text_shape = egui::epaint::TextShape::new(text_pos, galley, Color32::from_rgb(20, 20, 22));
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
        let font = FontId::proportional(11.5);
        let galley = ui.painter().layout_no_wrap(value_text.to_string(), font, Color32::from_rgb(20, 20, 25));
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
        painter.rect_stroke(rect, 8.0, Stroke::new(stroke_width, border_color), StrokeKind::Inside);
        painter.galley(rect.min + Vec2::new(9.0, 5.0), galley, Color32::from_rgb(20, 20, 25));

        response
    }

    /// Render handle icon panah 2 sisi (`↕`) tebal dan draggable (Screenshot 2, 3, 4)
    /// yang dapat langsung di-drag untuk mengubah ketebalan extrude/cut secara instan.
    pub fn render_draggable_double_arrow_handle(
        ui: &mut Ui,
        pos_2d: Pos2,
        is_dragging: bool,
        dir_2d: Option<Vec2>,
    ) -> egui::Response {
        let handle_radius = if is_dragging { 18.0 } else { 16.0 };
        let rect = egui::Rect::from_center_size(pos_2d, Vec2::splat(handle_radius * 2.0 + 8.0));
        let response = ui.allocate_rect(rect, egui::Sense::drag());
        let is_hovered = response.hovered();

        let (dir_u, dir_v) = if let Some(d) = dir_2d {
            let len = d.length();
            if len > 1e-4 {
                let u = d / len;
                let v = Vec2::new(-u.y, u.x);
                (u, v)
            } else {
                (Vec2::new(0.0, -1.0), Vec2::new(1.0, 0.0))
            }
        } else {
            (Vec2::new(0.0, -1.0), Vec2::new(1.0, 0.0))
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

        let painter = ui.painter();

        // 1. Drop shadow & outer glow ring saat hover/drag
        if is_hovered || is_dragging {
            painter.circle_filled(
                pos_2d,
                handle_radius + 5.0,
                Color32::from_rgba_premultiplied(0, 160, 255, 70),
            );
        } else {
            painter.circle_filled(
                pos_2d,
                handle_radius + 3.0,
                Color32::from_rgba_premultiplied(0, 0, 0, 40),
            );
        }

        // 2. Lingkaran background utama handle (Cyan / Putih mengkilap)
        let bg_color = if is_dragging {
            Color32::from_rgb(0, 120, 235)
        } else if is_hovered {
            Color32::from_rgb(0, 150, 255)
        } else {
            Color32::from_rgb(0, 140, 240)
        };
        painter.circle_filled(pos_2d, handle_radius, bg_color);
        painter.circle_stroke(
            pos_2d,
            handle_radius,
            Stroke::new(2.0, Color32::WHITE),
        );

        // 3. Icon panah 2 arah (`▲ - ▼`) tebal dan tajam di dalam handle (berputar sesuai arah proyeksi 3D)
        let icon_color = Color32::WHITE;

        // Segitiga panah forward (Filled)
        let top_tri = [
            pos_2d + dir_u * 9.5,
            pos_2d + dir_u * 3.5 - dir_v * 5.0,
            pos_2d + dir_u * 3.5 + dir_v * 5.0,
        ];
        painter.add(egui::epaint::Shape::convex_polygon(
            top_tri.to_vec(),
            icon_color,
            Stroke::NONE,
        ));

        // Batang poros panah (Thick bar)
        painter.line_segment(
            [pos_2d - dir_u * 3.5, pos_2d + dir_u * 3.5],
            Stroke::new(3.0, icon_color),
        );

        // Segitiga panah backward (Filled)
        let bot_tri = [
            pos_2d - dir_u * 9.5,
            pos_2d - dir_u * 3.5 - dir_v * 5.0,
            pos_2d - dir_u * 3.5 + dir_v * 5.0,
        ];
        painter.add(egui::epaint::Shape::convex_polygon(
            bot_tri.to_vec(),
            icon_color,
            Stroke::NONE,
        ));

        response
    }
}
