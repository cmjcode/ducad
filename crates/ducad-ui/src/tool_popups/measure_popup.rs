//! Measurement & Inspect Tool Popup.

use egui::{Context, Rect, RichText};
use egui_material_icons::icons::{ICON_CLOSE, ICON_STRAIGHTEN};

use super::{render_bottom_right_popup, ToolPopupEvent};
use crate::theme::{ACCENT_ORANGE, TEXT_SECONDARY};

#[derive(Debug, Clone)]
pub struct MeasurePopupState {
    pub show_all_dimensions: bool,
    pub measurements: Vec<String>,
}

impl Default for MeasurePopupState {
    fn default() -> Self {
        Self {
            show_all_dimensions: false,
            measurements: Vec::new(),
        }
    }
}

pub struct MeasurePopup;

impl MeasurePopup {
    pub fn show(
        ctx: &Context,
        state: &mut MeasurePopupState,
        screen_rect: Rect,
    ) -> Option<ToolPopupEvent> {
        let (event_opt, close) = render_bottom_right_popup(
            ctx,
            "ducad-measure-popup",
            "Pengukuran & Dimensi",
            ICON_STRAIGHTEN.codepoint,
            ACCENT_ORANGE,
            screen_rect,
            |ui| {
                let mut ev = None;

                if ui
                    .checkbox(&mut state.show_all_dimensions, "Tampilkan Semua Ukuran")
                    .on_hover_text("Tampilkan angka nominal dimensi tiap garis/rusuk di kanvas")
                    .changed()
                {
                    ev = Some(ToolPopupEvent::ToggleShowAllDimensions);
                }

                ui.separator();

                if state.measurements.is_empty() {
                    ui.label(
                        RichText::new("💡 Klik 2 titik di kanvas untuk ukur jarak, atau 3 titik untuk sudut.")
                            .size(10.0)
                            .color(TEXT_SECONDARY),
                    );
                } else {
                    ui.label(
                        RichText::new("Hasil Pengukuran:")
                            .strong()
                            .size(10.5)
                            .color(TEXT_SECONDARY),
                    );

                    let mut remove_at: Option<usize> = None;
                    for (i, label) in state.measurements.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(label).size(10.5).color(TEXT_SECONDARY));
                            if ui.small_button(RichText::new(ICON_CLOSE.codepoint).size(10.0)).clicked() {
                                remove_at = Some(i);
                            }
                        });
                    }

                    if let Some(i) = remove_at {
                        ev = Some(ToolPopupEvent::RemoveMeasurement(i));
                    }

                    ui.add_space(2.0);
                    if ui.button(RichText::new("Hapus Semua Pengukuran").size(10.5)).clicked() {
                        ev = Some(ToolPopupEvent::ClearMeasurements);
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
