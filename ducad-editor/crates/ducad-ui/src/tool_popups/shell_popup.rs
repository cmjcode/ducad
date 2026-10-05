use ducad_i18n::t;
use egui::{Color32, Context, Rect, RichText, Vec2};

use super::{render_bottom_right_popup, ToolPopupEvent};
use crate::theme::{ACCENT_BLUE, TEXT_SECONDARY};

#[derive(Debug, Clone)]
pub struct ShellPopupState {
    pub shell_input: String,
    /// Kedalaman rongga dari sisi terbuka; "0" = rongga penuh.
    pub depth_input: String,
    pub is_face_picking_active: bool,
    pub selected_faces_count: usize,
    pub selected_bodies_count: usize,
}

impl Default for ShellPopupState {
    fn default() -> Self {
        Self {
            shell_input: "2.0".to_string(),
            depth_input: "0".to_string(),
            is_face_picking_active: false,
            selected_faces_count: 0,
            selected_bodies_count: 0,
        }
    }
}

pub struct ShellPopup;

impl ShellPopup {
    pub fn show(
        ctx: &Context,
        state: &mut ShellPopupState,
        screen_rect: Rect,
    ) -> Option<ToolPopupEvent> {
        let (event_opt, close) = render_bottom_right_popup(
            ctx,
            "ducad-shell-popup",
            &t!("popup-shell-title"),
            egui_icons::icons::ICON_FILTER_NONE.codepoint,
            ACCENT_BLUE,
            screen_rect,
            |ui| {
                let mut ev = None;

                let face_btn_label = if state.is_face_picking_active {
                    t!("popup-shell-face-active")
                } else {
                    t!("popup-shell-face-enable")
                };

                let single = state.selected_bodies_count == 1;
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(single, egui::Button::new(RichText::new(face_btn_label).size(11.0)))
                        .clicked()
                    {
                        ev = Some(ToolPopupEvent::ToggleFacePicking);
                    }
                    ui.label(
                        RichText::new(t!("popup-shell-faces-count", count = state.selected_faces_count))
                            .size(11.0)
                            .color(TEXT_SECONDARY),
                    );
                });

                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{}:", t!("param-thickness"))).size(11.0));
                    ui.add_sized(
                        Vec2::new(60.0, 18.0),
                        egui::TextEdit::singleline(&mut state.shell_input),
                    );
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{}:", t!("param-depth"))).size(11.0));
                    ui.add_sized(
                        Vec2::new(60.0, 18.0),
                        egui::TextEdit::singleline(&mut state.depth_input).hint_text("0"),
                    )
                    .on_hover_text(t!("hud-shell-depth-hint"));
                });

                ui.add_space(3.0);
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(format!("🚀 {}", t!("tool-shell-name"))).size(11.0).color(Color32::WHITE),
                        )
                        .fill(ACCENT_BLUE),
                    )
                    .clicked()
                {
                    let depth = state.depth_input.trim();
                    let depth = if depth.is_empty() { Ok(0.0) } else { depth.parse::<f64>() };
                    if let (Ok(t), Ok(depth)) = (state.shell_input.trim().parse::<f64>(), depth) {
                        ev = Some(ToolPopupEvent::ApplyShell { thickness: t, depth });
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
