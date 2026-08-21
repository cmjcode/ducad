//! Fillet & Chamfer Tool Popup.

use egui::{Color32, Context, Rect, RichText, Vec2};

use super::{render_bottom_right_popup, ToolPopupEvent};
use crate::theme::{ACCENT_BLUE, TEXT_PRIMARY, TEXT_SECONDARY};

#[derive(Debug, Clone)]
pub struct FilletPopupState {
    pub fillet_input: String,
    pub chamfer_input: String,
    pub is_edge_picking_active: bool,
    pub selected_edges_count: usize,
    pub selected_bodies_count: usize,
}

impl Default for FilletPopupState {
    fn default() -> Self {
        Self {
            fillet_input: "2.0".to_string(),
            chamfer_input: "2.0".to_string(),
            is_edge_picking_active: false,
            selected_edges_count: 0,
            selected_bodies_count: 0,
        }
    }
}

pub struct FilletPopup;

impl FilletPopup {
    pub fn show(
        ctx: &Context,
        state: &mut FilletPopupState,
        screen_rect: Rect,
    ) -> Option<ToolPopupEvent> {
        let (event_opt, close) = render_bottom_right_popup(
            ctx,
            "ducad-fillet-popup",
            "Fillet & Chamfer",
            "⤹",
            ACCENT_BLUE,
            screen_rect,
            |ui| {
                let mut ev = None;

                let edge_btn_label = if state.is_edge_picking_active {
                    "✓ Mode Pilih Tepi (Aktif)"
                } else {
                    "○ Aktifkan Pilih Tepi"
                };

                let single = state.selected_bodies_count == 1;
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(single, egui::Button::new(RichText::new(edge_btn_label).size(11.0)))
                        .clicked()
                    {
                        ev = Some(ToolPopupEvent::ToggleEdgePicking);
                    }
                    ui.label(
                        RichText::new(format!("{} tepi", state.selected_edges_count))
                            .size(11.0)
                            .color(TEXT_SECONDARY),
                    );
                });

                if state.selected_edges_count > 0 && ui.small_button("Reset Seleksi Tepi").clicked() {
                    ev = Some(ToolPopupEvent::ResetEdgePicking);
                }

                ui.separator();

                // Fillet Row
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Radius:").size(11.0));
                    ui.add_sized(
                        Vec2::new(55.0, 18.0),
                        egui::TextEdit::singleline(&mut state.fillet_input),
                    );
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("Fillet").size(11.0).color(Color32::WHITE),
                            )
                            .fill(ACCENT_BLUE),
                        )
                        .clicked()
                    {
                        if let Ok(r) = state.fillet_input.trim().parse::<f64>() {
                            ev = Some(ToolPopupEvent::ApplyFillet { radius: r });
                        }
                    }
                });

                // Chamfer Row
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Jarak:").size(11.0));
                    ui.add_sized(
                        Vec2::new(55.0, 18.0),
                        egui::TextEdit::singleline(&mut state.chamfer_input),
                    );
                    if ui.button(RichText::new("Chamfer").size(11.0).color(TEXT_PRIMARY)).clicked() {
                        if let Ok(d) = state.chamfer_input.trim().parse::<f64>() {
                            ev = Some(ToolPopupEvent::ApplyChamfer { distance: d });
                        }
                    }
                });

                (ev, false)
            },
        );

        if close {
            Some(ToolPopupEvent::Close)
        } else {
            event_opt.flatten()
        }
    }
}
