//! Revolve 3D Tool Popup.

use egui::{Color32, Context, Rect, RichText, Vec2};
use egui_material_icons::icons::ICON_REFRESH;

use super::{render_bottom_right_popup, ToolPopupEvent};
use crate::theme::{ACCENT_BLUE, ACCENT_ORANGE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY};

#[derive(Debug, Clone)]
pub struct RevolvePopupState {
    pub axis_preset: u8,
    pub angle_input: String,
    pub reverse: bool,
}

impl Default for RevolvePopupState {
    fn default() -> Self {
        Self {
            axis_preset: 0,
            angle_input: "360.0".to_string(),
            reverse: false,
        }
    }
}

pub struct RevolvePopup;

impl RevolvePopup {
    pub fn show(
        ctx: &Context,
        state: &mut RevolvePopupState,
        screen_rect: Rect,
    ) -> Option<ToolPopupEvent> {
        let (event_opt, close) = render_bottom_right_popup(
            ctx,
            "ducad-revolve-popup",
            "Revolve 3D (Benda Putar)",
            ICON_REFRESH.codepoint,
            ACCENT_BLUE,
            screen_rect,
            |ui| {
                let mut ev = None;

                // 1. Poros Sumbu
                ui.label(RichText::new("Poros Sumbu Putar:").size(10.5).color(TEXT_SECONDARY));
                ui.radio_value(&mut state.axis_preset, 0, RichText::new("Sumbu Y (Vertikal)").size(10.5));
                ui.radio_value(&mut state.axis_preset, 1, RichText::new("Sumbu X (Horizontal)").size(10.5));
                ui.radio_value(&mut state.axis_preset, 2, RichText::new("Tepi Kiri Sketsa").size(10.5));
                ui.radio_value(&mut state.axis_preset, 3, RichText::new("Tepi Bawah Sketsa").size(10.5));
                ui.radio_value(&mut state.axis_preset, 4, RichText::new("✏️ Gambar 2 Titik Manual").size(10.5));

                ui.add_space(3.0);

                // 2. Preset & Input Sudut
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Sudut:").size(10.5).color(TEXT_SECONDARY));
                    if ui.small_button("360°").clicked() {
                        state.angle_input = "360.0".to_string();
                    }
                    if ui.small_button("180°").clicked() {
                        state.angle_input = "180.0".to_string();
                    }
                    if ui.small_button("90°").clicked() {
                        state.angle_input = "90.0".to_string();
                    }
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Derajat:").size(10.5).color(TEXT_SECONDARY));
                    ui.add_sized(
                        Vec2::new(60.0, 18.0),
                        egui::TextEdit::singleline(&mut state.angle_input),
                    );
                    ui.label(RichText::new("°").size(10.5).color(TEXT_MUTED));
                });

                ui.add_space(2.0);

                // 3. Arah Putar
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Arah:").size(10.5).color(TEXT_SECONDARY));
                    let dir_label = if state.reverse { "↻ Balik (CW)" } else { "↺ Normal (CCW)" };
                    if ui
                        .button(
                            RichText::new(dir_label)
                                .size(10.5)
                                .color(if state.reverse { ACCENT_ORANGE } else { TEXT_PRIMARY }),
                        )
                        .clicked()
                    {
                        state.reverse = !state.reverse;
                    }
                });

                ui.add_space(4.0);

                // 4. Tombol Eksekusi
                if state.axis_preset == 4 {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("✏️ Klik 2 Titik di Kanvas")
                                    .size(11.0)
                                    .color(Color32::WHITE),
                            )
                            .fill(ACCENT_BLUE),
                        )
                        .clicked()
                    {
                        ev = Some(ToolPopupEvent::StartManualRevolve);
                    }
                } else {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("🚀 Eksekusi Revolve")
                                    .size(11.0)
                                    .color(Color32::WHITE),
                            )
                            .fill(ACCENT_BLUE),
                        )
                        .clicked()
                    {
                        let angle = state.angle_input.trim().parse::<f64>().unwrap_or(360.0);
                        ev = Some(ToolPopupEvent::ApplyRevolvePreset {
                            preset_idx: state.axis_preset,
                            angle_deg: angle,
                        });
                    }
                }

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
