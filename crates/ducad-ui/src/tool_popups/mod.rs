//! Modular Tool Popups for DUCAD — Pojok Kanan Bawah.
//!
//! Setiap tool memiliki dialog / window kecil tersendiri yang mengambang
//! di pojok kanan bawah kanvas secara kontekstual hanya saat tool tersebut aktif.

pub mod boolean_popup;
pub mod entity_popup;
pub mod extrude_popup;
pub mod history_popup;
pub mod loft_popup;
pub mod measure_popup;
pub mod revolve_popup;
pub mod shell_popup;

use egui::{Align2, Color32, Context, Id, Pos2, Rect, RichText, Ui, Vec2};
use egui_material_icons::icons::ICON_CLOSE;

use crate::theme::{glass_frame, TEXT_SECONDARY};

pub use boolean_popup::{BooleanPopup, BooleanPopupState};
pub use entity_popup::{Entity2dPopup, Entity2dPopupState};
pub use extrude_popup::{ExtrudePopup, ExtrudePopupState};
pub use history_popup::{HistoryPopup, HistoryPopupState};
pub use loft_popup::{LoftPopup, LoftPopupState};
pub use measure_popup::{MeasurePopup, MeasurePopupState};
pub use revolve_popup::{RevolvePopup, RevolvePopupState};
pub use shell_popup::{ShellPopup, ShellPopupState};

/// Event umum yang dihasilkan oleh berbagai Tool Popup.
#[derive(Debug, Clone, PartialEq)]
pub enum ToolPopupEvent {
    Close,
    // Extrude
    ApplyExtrude { distance: f64 },
    ApplyFaceExtrude { distance: f64 },
    SketchOnFace,
    // Revolve
    ApplyRevolvePreset { preset_idx: u8, angle_deg: f64 },
    StartManualRevolve,
    // Loft
    StageLoftBottom,
    ApplyLoft { height: f64 },
    // Shell
    ToggleFacePicking,
    ApplyShell { thickness: f64 },
    // Boolean
    ApplyBooleanUnion,
    ApplyBooleanSubtract,
    ApplyBooleanIntersect,
    // History
    UndoModel,
    RedoModel,
    // Measure
    ToggleShowAllDimensions,
    RemoveMeasurement(usize),
    ClearMeasurements,
    // Entity 2D
    UpdateEntityLine {
        id_raw: u64,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
    },
    UpdateEntityCircle {
        id_raw: u64,
        center_x: f64,
        center_y: f64,
        radius: f64,
    },
    UpdateEntityArc {
        id_raw: u64,
        center_x: f64,
        center_y: f64,
        radius: f64,
        start_angle_deg: f64,
        end_angle_deg: f64,
    },
    UpdateEntityEllipse {
        id_raw: u64,
        center_x: f64,
        center_y: f64,
        radius_x: f64,
        radius_y: f64,
    },
    UpdateEntityRectangle {
        entity_ids: [u64; 4],
        length_p: f64,
        length_l: f64,
        anchor: crate::feature_inspector::InspectorRectAnchor,
    },
    ApplyConstraint(crate::feature_inspector::InspectorConstraintAction),
    DeleteSelectedEntities,
    DeleteSelectedBodies,
}

/// Helper pembungkus kartu popup mengambang di pojok kanan bawah.
pub fn render_bottom_right_popup<R>(
    ctx: &Context,
    id_str: &str,
    title: &str,
    icon: &str,
    accent_color: Color32,
    screen_rect: Rect,
    content: impl FnOnce(&mut Ui) -> (R, bool),
) -> (Option<R>, bool) {
    let mut close_clicked = false;
    let mut result = None;

    let margin_right = 16.0;
    let margin_bottom = 28.0;
    let pos = Pos2::new(screen_rect.max.x - margin_right, screen_rect.max.y - margin_bottom);

    egui::Area::new(Id::new(id_str))
        .fixed_pos(pos)
        .pivot(Align2::RIGHT_BOTTOM)
        .constrain_to(screen_rect)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            glass_frame().show(ui, |ui| {
                ui.set_width(260.0);
                ui.spacing_mut().item_spacing = Vec2::new(3.0, 4.0);

                // Header
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{} {}", icon, title))
                            .strong()
                            .size(12.5)
                            .color(accent_color),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button(RichText::new(ICON_CLOSE.codepoint).size(12.0).color(TEXT_SECONDARY))
                            .on_hover_text("Tutup Popup (Esc)")
                            .clicked()
                        {
                            close_clicked = true;
                        }
                    });
                });

                ui.separator();

                let (res, req_close) = content(ui);
                result = Some(res);
                if req_close {
                    close_clicked = true;
                }
            });
        });

    (result, close_clicked)
}
