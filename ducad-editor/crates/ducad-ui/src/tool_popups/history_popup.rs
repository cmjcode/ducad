//! Document History & Undo/Redo Tool Popup.

use egui::{Color32, Context, Rect, RichText};
use egui_icons::icons::{ICON_HISTORY, ICON_REDO, ICON_UNDO};

use super::{render_bottom_right_popup, ToolPopupEvent};
use crate::theme::{ACCENT_BLUE, TEXT_MUTED, TEXT_SECONDARY};

#[derive(Debug, Clone)]
pub struct HistoryPopupState {
    pub can_undo_model: bool,
    pub can_redo_model: bool,
    pub total_entities_count: usize,
    pub total_bodies_count: usize,
    pub status_message: Option<String>,
}

impl Default for HistoryPopupState {
    fn default() -> Self {
        Self {
            can_undo_model: false,
            can_redo_model: false,
            total_entities_count: 0,
            total_bodies_count: 0,
            status_message: None,
        }
    }
}

pub struct HistoryPopup;

impl HistoryPopup {
    pub fn show(
        ctx: &Context,
        state: &mut HistoryPopupState,
        screen_rect: Rect,
    ) -> Option<ToolPopupEvent> {
        let (event_opt, close) = render_bottom_right_popup(
            ctx,
            "ducad-history-popup",
            "Riwayat & Undo/Redo",
            ICON_HISTORY.codepoint,
            ACCENT_BLUE,
            screen_rect,
            |ui| {
                let mut ev = None;

                // 1. Undo & Redo Actions
                ui.horizontal(|ui| {
                    let undo_btn = egui::Button::new(
                        RichText::new(format!("{} Undo (⌘Z)", ICON_UNDO.codepoint)).size(11.0),
                    );
                    if ui.add_enabled(state.can_undo_model, undo_btn).clicked() {
                        ev = Some(ToolPopupEvent::UndoModel);
                    }

                    let redo_btn = egui::Button::new(
                        RichText::new(format!("{} Redo (⌘+Shift+Z)", ICON_REDO.codepoint)).size(11.0),
                    );
                    if ui.add_enabled(state.can_redo_model, redo_btn).clicked() {
                        ev = Some(ToolPopupEvent::RedoModel);
                    }
                });

                ui.separator();

                // 2. Info Dokumen
                ui.label(
                    RichText::new("Statistik Dokumen:")
                        .strong()
                        .size(10.5)
                        .color(TEXT_SECONDARY),
                );
                ui.label(
                    RichText::new(format!("• 2D Sketsa: {} entitas", state.total_entities_count))
                        .size(10.0)
                        .color(TEXT_SECONDARY),
                );
                ui.label(
                    RichText::new(format!("• 3D Model: {} bodies", state.total_bodies_count))
                        .size(10.0)
                        .color(TEXT_SECONDARY),
                );

                if let Some(msg) = &state.status_message {
                    ui.separator();
                    ui.label(
                        RichText::new(msg)
                            .size(10.0)
                            .color(Color32::from_rgb(240, 100, 100)),
                    );
                } else {
                    ui.label(
                        RichText::new("Riwayat langkah modeling tersimpan.")
                            .size(9.0)
                            .italics()
                            .color(TEXT_MUTED),
                    );
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
