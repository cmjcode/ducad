use ducad_i18n::t;
use egui::{Color32, Context, Rect, RichText};
use egui_material_icons::icons::ICON_ROUTE;

use super::{render_bottom_right_popup, ToolPopupEvent};
use crate::theme::{ACCENT_BLUE, ACCENT_GREEN, TEXT_SECONDARY};

#[derive(Debug, Clone, Default)]
pub struct SweepPopupState {
    pub sweep_profile_staged: bool,
    pub sweep_path_staged: bool,
}

pub struct SweepPopup;

impl SweepPopup {
    pub fn show(
        ctx: &Context,
        state: &mut SweepPopupState,
        screen_rect: Rect,
    ) -> Option<ToolPopupEvent> {
        let (event_opt, close) = render_bottom_right_popup(
            ctx,
            "ducad-sweep-popup",
            &t!("popup-sweep-title"),
            ICON_ROUTE.codepoint,
            ACCENT_BLUE,
            screen_rect,
            |ui| {
                let mut ev = None;

                ui.label(
                    RichText::new(t!("popup-sweep-desc"))
                        .size(10.5)
                        .color(TEXT_SECONDARY),
                );

                ui.add_space(2.0);
                ui.label(
                    RichText::new(t!("popup-sweep-step-1"))
                        .size(11.0)
                        .strong(),
                );

                let profile_staged_text = if state.sweep_profile_staged {
                    RichText::new(t!("popup-sweep-profile-saved")).color(ACCENT_GREEN).size(11.0)
                } else {
                    RichText::new(t!("popup-sweep-click-profile")).color(TEXT_SECONDARY).size(10.5)
                };
                ui.label(profile_staged_text);

                if ui
                    .button(RichText::new(t!("popup-sweep-set-profile")).size(10.5))
                    .clicked()
                {
                    ev = Some(ToolPopupEvent::StageSweepProfile);
                }

                ui.add_space(4.0);
                ui.label(
                    RichText::new(t!("popup-sweep-step-2"))
                        .size(11.0)
                        .strong(),
                );

                let path_staged_text = if state.sweep_path_staged {
                    RichText::new(t!("popup-sweep-path-saved")).color(ACCENT_GREEN).size(11.0)
                } else {
                    RichText::new(t!("popup-sweep-click-path")).color(TEXT_SECONDARY).size(10.5)
                };
                ui.label(path_staged_text);

                if ui
                    .button(RichText::new(t!("popup-sweep-set-path")).size(10.5))
                    .clicked()
                {
                    ev = Some(ToolPopupEvent::StageSweepPath);
                }

                ui.add_space(4.0);
                let btn_enabled = state.sweep_profile_staged && state.sweep_path_staged;
                if ui
                    .add_enabled(
                        btn_enabled,
                        egui::Button::new(
                            RichText::new(t!("popup-sweep-create-btn")).size(11.0).color(Color32::WHITE),
                        )
                        .fill(ACCENT_BLUE),
                    )
                    .clicked()
                {
                    ev = Some(ToolPopupEvent::ApplySweep);
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
