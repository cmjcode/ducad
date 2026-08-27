//! Extrude & Face Push-Pull Tool Popup.

use ducad_i18n::t;
use egui::{Color32, Context, Rect, RichText, Vec2};
use egui_icons::icons::ICON_OPEN_IN_FULL;

use super::{render_bottom_right_popup, ToolPopupEvent};
use crate::theme::{ACCENT_BLUE, TEXT_SECONDARY};

#[derive(Debug, Clone)]
pub struct ExtrudePopupState {
    pub extrude_input: String,
    pub is_face_extrude: bool,
    pub face_extrude_input: String,
    pub has_2d_selection: bool,
    pub has_face_selection: bool,
}

impl Default for ExtrudePopupState {
    fn default() -> Self {
        Self {
            extrude_input: "10.0".to_string(),
            is_face_extrude: false,
            face_extrude_input: "5.0".to_string(),
            has_2d_selection: false,
            has_face_selection: false,
        }
    }
}

pub struct ExtrudePopup;

impl ExtrudePopup {
    pub fn show(
        ctx: &Context,
        state: &mut ExtrudePopupState,
        screen_rect: Rect,
    ) -> Option<ToolPopupEvent> {
        let title = if state.is_face_extrude || state.has_face_selection {
            t!("popup-extrude-face-title")
        } else {
            t!("popup-extrude-profile-title")
        };

        let (event_opt, close) = render_bottom_right_popup(
            ctx,
            "ducad-extrude-popup",
            &title,
            ICON_OPEN_IN_FULL.codepoint,
            ACCENT_BLUE,
            screen_rect,
            |ui| {
                let mut ev = None;

                if state.is_face_extrude || state.has_face_selection {
                    // Mode Face Extrude
                    ui.label(
                        RichText::new(t!("popup-extrude-face-desc"))
                            .size(10.5)
                            .color(TEXT_SECONDARY),
                    );

                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{}:", t!("param-distance"))).size(11.0));
                        ui.add_sized(
                            Vec2::new(75.0, 20.0),
                            egui::TextEdit::singleline(&mut state.face_extrude_input),
                        );
                    });

                    ui.add_space(3.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(format!("🚀 {}", t!("tool-extrude-name"))).size(11.0).color(Color32::WHITE),
                                )
                                .fill(ACCENT_BLUE),
                            )
                            .clicked()
                        {
                            if let Ok(dist) = state.face_extrude_input.trim().parse::<f64>() {
                                ev = Some(ToolPopupEvent::ApplyFaceExtrude { distance: dist });
                            }
                        }

                        if ui.button(RichText::new(t!("popup-sketch-on-face")).size(11.0)).clicked() {
                            ev = Some(ToolPopupEvent::SketchOnFace);
                        }
                    });
                } else {
                    // Mode 2D Profile Extrude
                    ui.label(
                        RichText::new(t!("popup-extrude-profile-desc"))
                            .size(10.5)
                            .color(TEXT_SECONDARY),
                    );

                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{}:", t!("param-distance"))).size(11.0));
                        ui.add_sized(
                            Vec2::new(75.0, 20.0),
                            egui::TextEdit::singleline(&mut state.extrude_input),
                        );
                    });

                    ui.add_space(3.0);
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(format!("🚀 {}", t!("tool-extrude-name"))).size(11.0).color(Color32::WHITE),
                            )
                            .fill(ACCENT_BLUE),
                        )
                        .clicked()
                    {
                        if let Ok(dist) = state.extrude_input.trim().parse::<f64>() {
                            ev = Some(ToolPopupEvent::ApplyExtrude { distance: dist });
                        }
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
