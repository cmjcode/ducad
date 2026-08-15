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
}
