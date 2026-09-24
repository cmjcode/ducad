//! Color Picker interaktif: HSV / RGB sliders + Hex input + Alpha + Swatches (M2.7).

use ducad_sketch::Rgba;
use egui::{RichText, Ui, Vec2};
use super::swatches::SwatchManager;

/// Aksi yang dihasilkan dari interaksi Color Picker.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorPickerAction {
    Changed(Rgba),
    AddDocumentSwatch(Rgba),
    StartEyedropper,
}

/// State widget Color Picker.
#[derive(Debug, Clone)]
pub struct ColorPickerState {
    pub color: Rgba,
    pub hex_buffer: String,
    pub is_open: bool,
}

impl Default for ColorPickerState {
    fn default() -> Self {
        Self {
            color: Rgba::BLACK,
            hex_buffer: "#000000".to_string(),
            is_open: false,
        }
    }
}

impl ColorPickerState {
    pub fn new(color: Rgba) -> Self {
        Self {
            color,
            hex_buffer: color.to_hex(),
            is_open: false,
        }
    }

    pub fn set_color(&mut self, color: Rgba) {
        self.color = color;
        self.hex_buffer = color.to_hex();
    }

    /// Render popup / inline color picker.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        recent: &mut SwatchManager,
        doc_swatches: &[Rgba],
    ) -> Option<ColorPickerAction> {
        let mut action = None;

        ui.vertical(|ui| {
            // Preview kotak warna aktif + tombol Eyedropper
            ui.horizontal(|ui| {
                let egui_col = egui::Color32::from_rgba_unmultiplied(
                    (self.color.0[0] * 255.0) as u8,
                    (self.color.0[1] * 255.0) as u8,
                    (self.color.0[2] * 255.0) as u8,
                    (self.color.0[3] * 255.0) as u8,
                );
                let (rect, _) = ui.allocate_exact_size(Vec2::new(32.0, 24.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 4.0, egui_col);
                ui.painter().rect_stroke(
                    rect,
                    4.0,
                    egui::Stroke::new(1.0, egui::Color32::from_white_alpha(50)),
                    egui::StrokeKind::Inside,
                );


                if ui.button(RichText::new("").size(14.0)).on_hover_text("Eyedropper (I)").clicked() {
                    action = Some(ColorPickerAction::StartEyedropper);
                }

                // Hex input
                let hex_edit = ui.add(
                    egui::TextEdit::singleline(&mut self.hex_buffer)
                        .desired_width(75.0)
                        .hint_text("#RRGGBB"),
                );
                if hex_edit.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if let Some(parsed) = Rgba::from_hex(&self.hex_buffer) {
                        self.color = parsed;
                        recent.push_recent(parsed);
                        action = Some(ColorPickerAction::Changed(parsed));
                    } else {
                        self.hex_buffer = self.color.to_hex();
                    }
                }
            });

            ui.separator();

            // Slider RGB / Alpha
            let mut r = self.color.0[0];
            let mut g = self.color.0[1];
            let mut b = self.color.0[2];
            let mut a = self.color.0[3];
            let mut changed = false;

            ui.horizontal(|ui| {
                ui.label("R");
                changed |= ui.add(egui::Slider::new(&mut r, 0.0..=1.0).show_value(false)).changed();
            });
            ui.horizontal(|ui| {
                ui.label("G");
                changed |= ui.add(egui::Slider::new(&mut g, 0.0..=1.0).show_value(false)).changed();
            });
            ui.horizontal(|ui| {
                ui.label("B");
                changed |= ui.add(egui::Slider::new(&mut b, 0.0..=1.0).show_value(false)).changed();
            });
            ui.horizontal(|ui| {
                ui.label("A");
                changed |= ui.add(egui::Slider::new(&mut a, 0.0..=1.0).show_value(false)).changed();
            });

            if changed {
                let new_col = Rgba([r, g, b, a]);
                self.color = new_col;
                self.hex_buffer = new_col.to_hex();
                recent.push_recent(new_col);
                action = Some(ColorPickerAction::Changed(new_col));
            }

            ui.separator();

            // Swatches: Recent (maksimal 8)
            if !recent.recent.is_empty() {
                ui.label(RichText::new("Terakhir Digunakan").size(10.0).weak());
                if let Some(swatch) = SwatchManager::show_swatch_row(ui, &recent.recent, Vec2::new(18.0, 18.0)) {
                    self.set_color(swatch);
                    action = Some(ColorPickerAction::Changed(swatch));
                }
            }

            // Swatches: Dokumen
            ui.horizontal(|ui| {
                ui.label(RichText::new("Palet Dokumen").size(10.0).weak());
                if ui.small_button("+").on_hover_text("Tambah warna saat ini ke palet dokumen").clicked() {
                    action = Some(ColorPickerAction::AddDocumentSwatch(self.color));
                }
            });
            if !doc_swatches.is_empty() {
                if let Some(swatch) = SwatchManager::show_swatch_row(ui, doc_swatches, Vec2::new(18.0, 18.0)) {
                    self.set_color(swatch);
                    action = Some(ColorPickerAction::Changed(swatch));
                }
            }
        });

        action
    }
}
