//! Parametric Properties & Feature Inspector bergaya Shapr3D dengan Material Icons.

pub mod entity_2d;
pub mod features_3d;
pub mod types;

use egui::{Color32, RichText, ScrollArea, Ui, Vec2};
use egui_material_icons::icons::{ICON_CLOSE, ICON_PUSH_PIN, ICON_TUNE};

use crate::theme::{glass_frame, ACCENT_BLUE, TEXT_PRIMARY, TEXT_SECONDARY};

pub use types::{
    FeatureInspectorState, InspectorBooleanKind, InspectorConstraintAction, InspectorEvent,
    InspectorPickMode, SelectedBodyData, SelectedEntityData,
};

use entity_2d::show_2d_entity_cards;
use features_3d::{show_3d_cards, show_measurements_card};

pub struct FeatureInspector;

impl FeatureInspector {
    /// Render panel inspector properti & fitur yang fixed di kanan kanvas.
    pub fn show(ui: &mut Ui, state: &mut FeatureInspectorState) -> Option<InspectorEvent> {
        let mut event = None;

        glass_frame().show(ui, |ui| {
            ui.set_width(244.0);
            ui.spacing_mut().item_spacing = Vec2::new(3.0, 4.0);

            // 1. Header: Title, Auto-Hide Toggle, Minimize/Close
            let header_title = match (&state.selected_entity, &state.selected_body) {
                (SelectedEntityData::Line { .. }, _) => "Properti Garis",
                (SelectedEntityData::Circle { .. }, _) => "Properti Lingkaran",
                (SelectedEntityData::Arc { .. }, _) => "Properti Busur",
                (SelectedEntityData::Ellipse { .. }, _) => "Properti Elips",
                (SelectedEntityData::MultipleEntities { .. }, _) => "Seleksi 2D",
                (_, Some(_)) => "Properti 3D Body",
                _ if state.selected_bodies_count > 1 => "Seleksi 3D",
                _ => "Properti & Fitur",
            };

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{} {}", ICON_TUNE, header_title))
                        .strong()
                        .size(12.5)
                        .color(TEXT_PRIMARY),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Tombol Tutup / Sembunyikan
                    if ui
                        .small_button(RichText::new(ICON_CLOSE).size(12.0).color(TEXT_SECONDARY))
                        .on_hover_text("Sembunyikan Panel")
                        .clicked()
                    {
                        event = Some(InspectorEvent::CloseInspector);
                    }

                    // Toggle Auto-Hide (Pin vs Auto)
                    let pin_color = if state.auto_hide_enabled {
                        ACCENT_BLUE
                    } else {
                        TEXT_SECONDARY
                    };
                    let pin_text = if state.auto_hide_enabled { "Auto" } else { "Pin" };
                    if ui
                        .small_button(
                            RichText::new(format!("{} {}", ICON_PUSH_PIN, pin_text))
                                .size(10.0)
                                .color(pin_color),
                        )
                        .on_hover_text(if state.auto_hide_enabled {
                            "Auto-Hide: Aktif (Otomatis sembunyi jika tak ada seleksi). Klik untuk Pin."
                        } else {
                            "Auto-Hide: Nonaktif (Panel selalu terbuka/Pin). Klik untuk Auto-Hide."
                        })
                        .clicked()
                    {
                        event = Some(InspectorEvent::ToggleAutoHide);
                    }
                });
            });

            ui.separator();

            ScrollArea::vertical()
                .auto_shrink([false, true])
                .max_height(state.max_panel_height)
                .show(ui, |ui| {
                    show_measurements_card(ui, state, &mut event);
                    show_2d_entity_cards(ui, state, &mut event);
                    show_3d_cards(ui, state, &mut event);

                    // Status / Error message
                    if let Some(msg) = &state.status_message {
                        ui.separator();
                        ui.label(
                            RichText::new(msg)
                                .color(Color32::from_rgb(240, 100, 100))
                                .size(10.5),
                        );
                    }
                });
        });

        event
    }
}
