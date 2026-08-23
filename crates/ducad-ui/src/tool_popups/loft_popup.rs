use ducad_i18n::t;
use egui::{Color32, Context, Rect, RichText, Vec2};
use egui_material_icons::icons::ICON_LAYERS;

use super::{render_bottom_right_popup, ToolPopupEvent};
use crate::theme::{ACCENT_BLUE, ACCENT_GREEN, TEXT_SECONDARY};

#[derive(Debug, Clone)]
pub struct LoftPopupState {
    pub loft_height_input: String,
    pub loft_bottom_staged: bool,
}

impl Default for LoftPopupState {
    fn default() -> Self {
        Self {
            loft_height_input: "20.0".to_string(),
            loft_bottom_staged: false,
        }
    }
}

pub struct LoftPopup;

impl LoftPopup {
    pub fn show(
        ctx: &Context,
        state: &mut LoftPopupState,
        screen_rect: Rect,
    ) -> Option<ToolPopupEvent> {
        let (event_opt, close) = render_bottom_right_popup(
            ctx,
            "ducad-loft-popup",
            &t!("popup-loft-title"),
            ICON_LAYERS.codepoint,
            ACCENT_BLUE,
            screen_rect,
            |ui| {
                let mut ev = None;

                ui.label(
                    RichText::new(t!("popup-loft-desc"))
                        .size(10.5)
                        .color(TEXT_SECONDARY),
                );

                ui.add_space(2.0);
                ui.label(
                    RichText::new(t!("popup-loft-step-1"))
                        .size(11.0)
                        .strong(),
                );

                let staged_text = if state.loft_bottom_staged {
                    RichText::new(t!("popup-loft-bottom-saved")).color(ACCENT_GREEN).size(11.0)
                } else {
                    RichText::new(t!("popup-loft-click-p1")).color(TEXT_SECONDARY).size(10.5)
                };
                ui.label(staged_text);

                if ui
                    .button(RichText::new(t!("popup-loft-set-bottom")).size(10.5))
                    .clicked()
                {
                    ev = Some(ToolPopupEvent::StageLoftBottom);
                }

                ui.add_space(4.0);
                ui.label(
                    RichText::new(t!("popup-loft-step-2"))
                        .size(11.0)
                        .strong(),
                );
                ui.label(
                    RichText::new(t!("popup-loft-click-p2"))
                        .size(10.5)
                        .color(TEXT_SECONDARY),
                );

                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{}:", t!("param-height"))).size(11.0));
                    ui.add_sized(
                        Vec2::new(70.0, 20.0),
                        egui::TextEdit::singleline(&mut state.loft_height_input),
                    );
                });

                ui.add_space(3.0);
                let btn_enabled = state.loft_bottom_staged;
                if ui
                    .add_enabled(
                        btn_enabled,
                        egui::Button::new(
                            RichText::new(format!("🚀 {}", t!("tool-loft-name"))).size(11.0).color(Color32::WHITE),
                        )
                        .fill(ACCENT_BLUE),
                    )
                    .clicked()
                {
                    if let Ok(h) = state.loft_height_input.trim().parse::<f64>() {
                        ev = Some(ToolPopupEvent::ApplyLoft { height: h });
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
