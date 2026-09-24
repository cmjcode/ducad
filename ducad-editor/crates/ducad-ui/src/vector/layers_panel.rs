//! Panel Layer Vektor (M2.7).
//!
//! Menampilkan daftar layer dalam urutan gambar terbalik (atas = depan),
//! visibilitas, gembok, pemilihan layer aktif, penataan urutan, dan pembuatan/penghapusan.

use ducad_sketch::layer::{Layer, LayerId};
use ducad_sketch::Rgba;
use egui::{RichText, Ui, Vec2};

/// Aksi / event yang dihasilkan oleh interaksi di Panel Layer.
#[derive(Debug, Clone, PartialEq)]
pub enum LayersPanelEvent {
    SelectActive(LayerId),
    ToggleVisibility(LayerId, bool),
    ToggleLocked(LayerId, bool),
    Rename(LayerId, String),
    MoveUp(LayerId),
    MoveDown(LayerId),
    Create(Layer),
    Delete(LayerId),
}

/// State widget Panel Layer.
#[derive(Debug, Clone)]
pub struct LayersPanelState {
    pub is_visible: bool,
    pub editing_layer: Option<(LayerId, String)>,
}

impl Default for LayersPanelState {
    fn default() -> Self {
        Self {
            is_visible: true,
            editing_layer: None,
        }
    }
}

impl LayersPanelState {
    pub fn new() -> Self {
        Self::default()
    }


    /// Render isi panel layer.
    /// `layers`: daftar layer dengan ID-nya, dalam urutan `layer_order.iter().rev()` (atas = depan).
    /// `active_layer`: ID layer yang saat ini aktif untuk entitas baru.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        layer_order_reversed: &[(LayerId, Layer)],
        active_layer: Option<LayerId>,
    ) -> Option<LayersPanelEvent> {
        if !self.is_visible {
            return None;
        }

        let mut event = None;

        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.heading(RichText::new("Layer").size(14.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("+").on_hover_text("Tambah layer baru").clicked() {
                        let new_color = Rgba([0.4, 0.6, 1.0, 1.0]);
                        let new_name = format!("Layer {}", layer_order_reversed.len() + 1);
                        event = Some(LayersPanelEvent::Create(Layer::new(new_name, new_color)));
                    }
                });
            });

            ui.separator();

            // Daftar layer (urutan terbalik: atas = depan)
            egui::ScrollArea::vertical()
                .max_height(240.0)
                .show(ui, |ui| {
                    for (idx, (lid, layer)) in layer_order_reversed.iter().enumerate() {
                        let is_active = Some(*lid) == active_layer;
                        let lid = *lid;

                        ui.horizontal(|ui| {
                            // 1. Indikator aktif (klik untuk jadikan aktif)
                            let marker = if is_active { "●" } else { "○" };
                            if ui.selectable_label(is_active, marker).on_hover_text("Klik untuk jadikan layer aktif").clicked() {
                                event = Some(LayersPanelEvent::SelectActive(lid));
                            }

                            // 2. Kotak warna layer
                            let egui_col = egui::Color32::from_rgba_unmultiplied(
                                (layer.color.0[0] * 255.0) as u8,
                                (layer.color.0[1] * 255.0) as u8,
                                (layer.color.0[2] * 255.0) as u8,
                                255,
                            );
                            let (c_rect, _) = ui.allocate_exact_size(Vec2::new(12.0, 12.0), egui::Sense::hover());
                            ui.painter().rect_filled(c_rect, 2.0, egui_col);

                            // 3. Toggle Mata (Visibilitas)
                            let eye_icon = if layer.visible { "👁" } else { "Ø" };
                            if ui.small_button(eye_icon).on_hover_text("Tampilkan/Sembunyikan layer").clicked() {
                                event = Some(LayersPanelEvent::ToggleVisibility(lid, !layer.visible));
                            }

                            // 4. Toggle Gembok (Kunci)
                            let lock_icon = if layer.locked { "🔒" } else { "🔓" };
                            if ui.small_button(lock_icon).on_hover_text("Kunci/Buka kunci layer").clicked() {
                                event = Some(LayersPanelEvent::ToggleLocked(lid, !layer.locked));
                            }

                            // 5. Nama layer (edit jika sedang diedit)
                            if let Some((editing_id, ref mut name_buf)) = self.editing_layer {
                                if editing_id == lid {
                                    let resp = ui.text_edit_singleline(name_buf);
                                    if resp.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                        event = Some(LayersPanelEvent::Rename(lid, name_buf.clone()));
                                        self.editing_layer = None;
                                    }
                                } else {
                                    render_layer_name(ui, layer, &mut self.editing_layer, lid);
                                }
                            } else {
                                render_layer_name(ui, layer, &mut self.editing_layer, lid);
                            }

                            // 6. Urutan: Naik / Turun
                            if idx > 0 && ui.small_button("▲").on_hover_text("Pindah ke atas (ke depan)").clicked() {
                                event = Some(LayersPanelEvent::MoveUp(lid));
                            }
                            if idx + 1 < layer_order_reversed.len() && ui.small_button("▼").on_hover_text("Pindah ke bawah (ke belakang)").clicked() {
                                event = Some(LayersPanelEvent::MoveDown(lid));
                            }

                            // 7. Tombol hapus layer
                            if layer_order_reversed.len() > 1 && ui.small_button("−").on_hover_text("Hapus layer").clicked() {
                                event = Some(LayersPanelEvent::Delete(lid));
                            }
                        });
                    }
                });
        });

        event
    }
}

fn render_layer_name(
    ui: &mut Ui,
    layer: &Layer,
    editing_state: &mut Option<(LayerId, String)>,
    lid: LayerId,
) {
    let resp = ui.selectable_label(false, &layer.name);
    if resp.double_clicked() {
        *editing_state = Some((lid, layer.name.clone()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layers_panel_state_default() {
        let state = LayersPanelState::default();
        assert!(state.is_visible);
        assert!(state.editing_layer.is_none());
    }
}
