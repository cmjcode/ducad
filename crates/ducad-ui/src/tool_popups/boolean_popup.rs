use ducad_i18n::t;
use egui::{Context, Rect, RichText};
use egui_material_icons::icons::ICON_CALL_MERGE;

use super::{render_bottom_right_popup, ToolPopupEvent};
use crate::theme::{ACCENT_BLUE, TEXT_SECONDARY};

#[derive(Debug, Clone)]
pub struct BooleanPopupState {
    pub selected_bodies_count: usize,
}

impl Default for BooleanPopupState {
    fn default() -> Self {
        Self {
            selected_bodies_count: 0,
        }
    }
}

pub struct BooleanPopup;

impl BooleanPopup {
    pub fn show(
        ctx: &Context,
        state: &mut BooleanPopupState,
        screen_rect: Rect,
    ) -> Option<ToolPopupEvent> {
        let (event_opt, close) = render_bottom_right_popup(
            ctx,
            "ducad-boolean-popup",
            &t!("popup-boolean-title"),
            ICON_CALL_MERGE.codepoint,
            ACCENT_BLUE,
            screen_rect,
            |ui| {
                let mut ev = None;

                ui.label(
                    RichText::new(t!("popup-boolean-desc", count = state.selected_bodies_count))
                    .size(10.5)
                    .color(TEXT_SECONDARY),
                );

                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    let has_min_2 = state.selected_bodies_count >= 2;

                    if ui
                        .add_enabled(has_min_2, egui::Button::new(RichText::new(t!("boolean-union")).size(11.0)))
                        .clicked()
                    {
                        ev = Some(ToolPopupEvent::ApplyBooleanUnion);
                    }

                    if ui
                        .add_enabled(has_min_2, egui::Button::new(RichText::new(t!("boolean-subtract")).size(11.0)))
                        .clicked()
                    {
                        ev = Some(ToolPopupEvent::ApplyBooleanSubtract);
                    }

                    if ui
                        .add_enabled(has_min_2, egui::Button::new(RichText::new(t!("boolean-intersect")).size(11.0)))
                        .clicked()
                    {
                        ev = Some(ToolPopupEvent::ApplyBooleanIntersect);
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
