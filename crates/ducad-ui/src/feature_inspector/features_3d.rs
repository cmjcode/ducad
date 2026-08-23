use ducad_i18n::t;
use egui::{Color32, RichText, Slider, Ui, Vec2};
use egui_material_icons::icons::{
    ICON_CALL_MERGE, ICON_CATEGORY, ICON_CLOSE, ICON_CONTENT_CUT, ICON_DELETE,
    ICON_REDO, ICON_REFRESH, ICON_STRAIGHTEN, ICON_UNDO,
};

use crate::feature_inspector::types::{
    FeatureInspectorState, InspectorBooleanKind, InspectorEvent, InspectorPickMode,
    SelectedEntityData,
};
use crate::theme::{
    card_frame, ACCENT_BLUE, ACCENT_GREEN, ACCENT_ORANGE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};

pub fn show_measurements_card(
    ui: &mut Ui,
    state: &mut FeatureInspectorState,
    event: &mut Option<InspectorEvent>,
) {
    card_frame().show(ui, |ui| {
        ui.label(
            RichText::new(format!("{} {}", ICON_STRAIGHTEN.codepoint, t!("tool-measure-name")))
                .strong()
                .size(11.5)
                .color(ACCENT_ORANGE),
        );

        if ui
            .checkbox(&mut state.show_all_dimensions, t!("hud-show-dimensions"))
            .on_hover_text(t!("inspector-show-all-dim-tooltip"))
            .changed()
        {
            *event = Some(InspectorEvent::ToggleShowAllDimensions);
        }

        if state.measurement_tool_active || !state.measurements.is_empty() {
            ui.separator();
        }
        if state.measurement_tool_active && state.measurements.is_empty() {
            ui.label(
                RichText::new(t!("inspector-measure-hint"))
                    .size(10.0)
                    .color(TEXT_SECONDARY),
            );
        }
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
            *event = Some(InspectorEvent::RemoveMeasurement(i));
        }
        if !state.measurements.is_empty() {
            ui.separator();
            if ui.button(RichText::new(t!("inspector-clear-all")).size(10.5)).clicked() {
                *event = Some(InspectorEvent::ClearMeasurements);
            }
        }
    });
    ui.add_space(3.0);
}

pub fn show_3d_cards(
    ui: &mut Ui,
    state: &mut FeatureInspectorState,
    event: &mut Option<InspectorEvent>,
) {
    // 3. 3D Body Info (jika body dipilih)
    if let Some(body) = &state.selected_body {
        card_frame().show(ui, |ui| {
            ui.label(
                RichText::new(format!("{} {}", ICON_CATEGORY.codepoint, body.name))
                    .strong()
                    .size(11.5)
                    .color(ACCENT_GREEN),
            );
            ui.label(
                RichText::new(format!(
                    "Mesh: {} vert, {} tri",
                    body.vertices_count, body.triangles_count
                ))
                .size(10.0)
                .color(TEXT_SECONDARY),
            );
            ui.label(
                RichText::new(format!(
                    "BBox: {:.1}×{:.1}×{:.1} mm",
                    body.bbox_size[0], body.bbox_size[1], body.bbox_size[2]
                ))
                .size(10.0)
                .color(TEXT_SECONDARY),
            );

            ui.add_space(3.0);
            ui.label(
                RichText::new(t!("inspector-resize-tip"))
                    .size(9.0)
                    .italics()
                    .color(TEXT_SECONDARY),
            );
            ui.label(
                RichText::new(t!("inspector-uniform-scale-note"))
                    .size(8.5)
                    .italics()
                    .color(TEXT_SECONDARY),
            );
        });
        ui.add_space(3.0);
    }

    let has_2d_selection = !matches!(state.selected_entity, SelectedEntityData::None);
    if has_2d_selection {
        // Revolve Card (Properties Panel Kanan)
        card_frame().show(ui, |ui| {
            ui.label(
                RichText::new(format!("{} {}", ICON_REFRESH.codepoint, t!("inspector-revolve-3d")))
                    .strong()
                    .size(11.5)
                    .color(ACCENT_BLUE),
            );
            ui.add_space(2.0);

            // Pilihan Sumbu
            ui.label(RichText::new(t!("inspector-revolve-axis")).size(10.5).color(TEXT_SECONDARY));
            ui.radio_value(&mut state.revolve_axis_preset, 0, RichText::new(t!("inspector-axis-y-vert")).size(10.5));
            ui.radio_value(&mut state.revolve_axis_preset, 1, RichText::new(t!("inspector-axis-x-horiz")).size(10.5));
            ui.radio_value(&mut state.revolve_axis_preset, 2, RichText::new(t!("inspector-axis-sketch-left")).size(10.5));
            ui.radio_value(&mut state.revolve_axis_preset, 3, RichText::new(t!("inspector-axis-sketch-bottom")).size(10.5));
            ui.radio_value(&mut state.revolve_axis_preset, 4, RichText::new(t!("inspector-draw-2-points-manual")).size(10.5));

            ui.add_space(3.0);

            // Sudut
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{}:", t!("param-angle"))).size(10.5).color(TEXT_SECONDARY));
                if ui.small_button("360°").clicked() {
                    state.revolve_angle_input = "360.0".to_string();
                }
                if ui.small_button("180°").clicked() {
                    state.revolve_angle_input = "180.0".to_string();
                }
                if ui.small_button("90°").clicked() {
                    state.revolve_angle_input = "90.0".to_string();
                }
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{}:", t!("param-angle"))).size(10.5).color(TEXT_SECONDARY));
                ui.add_sized(
                    Vec2::new(60.0, 18.0),
                    egui::TextEdit::singleline(&mut state.revolve_angle_input),
                );
                ui.label(RichText::new("°").size(10.5).color(TEXT_MUTED));
            });

            ui.add_space(3.0);

            // Arah Putar (CW vs CCW)
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{}:", t!("param-direction"))).size(10.5).color(TEXT_SECONDARY));
                let dir_label = if state.revolve_reverse { "↻ CW" } else { "↺ CCW" };
                if ui
                    .button(RichText::new(dir_label).size(10.5).color(if state.revolve_reverse { ACCENT_ORANGE } else { TEXT_PRIMARY }))
                    .clicked()
                {
                    state.revolve_reverse = !state.revolve_reverse;
                }
            });

            ui.add_space(4.0);

            // Tombol Eksekusi
            if state.revolve_axis_preset == 4 {
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(t!("inspector-click-2-points-canvas"))
                                .size(11.0)
                                .color(Color32::WHITE),
                        )
                        .fill(ACCENT_BLUE),
                    )
                    .clicked()
                {
                    *event = Some(InspectorEvent::StartManualRevolve);
                }
            } else {
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(t!("inspector-exec-revolve"))
                                .size(11.0)
                                .color(Color32::WHITE),
                        )
                        .fill(ACCENT_BLUE),
                    )
                    .clicked()
                {
                    let angle = state.revolve_angle_input.trim().parse::<f64>().unwrap_or(360.0);
                    *event = Some(InspectorEvent::ApplyRevolvePreset {
                        preset_idx: state.revolve_axis_preset,
                        angle_deg: angle,
                    });
                }
            }
        });
        ui.add_space(3.0);

        // Loft Card
        card_frame().show(ui, |ui| {
            ui.label(
                RichText::new(format!("{} {}", ICON_REFRESH.codepoint, t!("tool-loft-name")))
                    .strong()
                    .size(11.5)
                    .color(ACCENT_BLUE),
            );
            ui.separator();
            ui.label(RichText::new(format!("{}:", t!("tool-loft-name"))).size(10.5).color(TEXT_SECONDARY));
            let staged_label = if state.loft_bottom_staged {
                t!("inspector-loft-staged")
            } else {
                t!("inspector-loft-unstaged")
            };
            ui.weak(staged_label);
            if ui.button(RichText::new(t!("inspector-set-bottom-profile")).size(10.5)).clicked() {
                *event = Some(InspectorEvent::StageLoftBottom);
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{}:", t!("param-height"))).size(10.5).color(TEXT_SECONDARY));
                ui.add_sized(
                    Vec2::new(60.0, 18.0),
                    egui::TextEdit::singleline(&mut state.loft_height_input),
                );
            });
            if ui.button(RichText::new(t!("inspector-exec-loft")).size(11.0)).clicked() {
                if let Ok(h) = state.loft_height_input.trim().parse::<f64>() {
                    *event = Some(InspectorEvent::ApplyLoft { height: h });
                }
            }
        });
        ui.add_space(3.0);
    }

    // 5. Boolean Operations (jika ada body terpilih)
    if state.selected_bodies_count > 0 {
        card_frame().show(ui, |ui| {
            ui.label(
                RichText::new(format!(
                    "{} Boolean ({} Bodies)",
                    ICON_CALL_MERGE.codepoint, state.selected_bodies_count
                ))
                .strong()
                .color(ACCENT_BLUE),
            );
            ui.horizontal(|ui| {
                if ui.button(RichText::new(t!("boolean-union")).size(10.5)).clicked() {
                    *event = Some(InspectorEvent::ApplyBoolean(InspectorBooleanKind::Union));
                }
                if ui.button(RichText::new(t!("boolean-subtract")).size(10.5)).clicked() {
                    *event = Some(InspectorEvent::ApplyBoolean(InspectorBooleanKind::Subtract));
                }
                if ui.button(RichText::new(t!("boolean-intersect")).size(10.5)).clicked() {
                    *event = Some(InspectorEvent::ApplyBoolean(InspectorBooleanKind::Intersect));
                }
            });
        });
        ui.add_space(3.0);

        // Fillet & Chamfer Card
        card_frame().show(ui, |ui| {
            ui.label(
                RichText::new(format!("{} Fillet & Chamfer", ICON_CATEGORY.codepoint))
                    .strong()
                    .color(ACCENT_BLUE),
            );

            let edge_btn_label = if state.picking_mode == InspectorPickMode::Edge {
                t!("inspector-edge-pick-active")
            } else {
                t!("inspector-edge-pick-manual")
            };
            ui.horizontal(|ui| {
                let single = state.selected_bodies_count == 1;
                if ui
                    .add_enabled(
                        single,
                        egui::Button::new(RichText::new(edge_btn_label).size(10.5)),
                    )
                    .clicked()
                {
                    *event = Some(InspectorEvent::ToggleEdgePicking);
                }
                ui.label(
                    RichText::new(t!("inspector-edge-count", count = state.selected_edges_count))
                        .size(10.5)
                        .color(TEXT_SECONDARY),
                );
            });

            if state.selected_edges_count > 0 && ui.small_button(t!("inspector-reset-edge-pick")).clicked() {
                *event = Some(InspectorEvent::ResetEdgePicking);
            }

            ui.separator();
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{}:", t!("param-radius"))).size(10.5).color(TEXT_SECONDARY));
                ui.add_sized(
                    Vec2::new(55.0, 18.0),
                    egui::TextEdit::singleline(&mut state.fillet_input),
                );
                if ui.button(RichText::new(t!("tool-fillet-name")).size(10.5)).clicked() {
                    if let Ok(r) = state.fillet_input.trim().parse::<f64>() {
                        *event = Some(InspectorEvent::ApplyFillet { radius: r });
                    }
                }
            });

            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{}:", t!("param-distance"))).size(10.5).color(TEXT_SECONDARY));
                ui.add_sized(
                    Vec2::new(55.0, 18.0),
                    egui::TextEdit::singleline(&mut state.chamfer_input),
                );
                if ui.button(RichText::new(t!("tool-chamfer-name")).size(10.5)).clicked() {
                    if let Ok(d) = state.chamfer_input.trim().parse::<f64>() {
                        *event = Some(InspectorEvent::ApplyChamfer { distance: d });
                    }
                }
            });
        });
        ui.add_space(3.0);


        // Hapus Body
        let del_text = format!("{} {}", ICON_DELETE.codepoint, t!("inspector-delete-selected-bodies"));
        if ui
            .button(
                RichText::new(del_text)
                    .size(11.0)
                    .color(Color32::from_rgb(240, 90, 90)),
            )
            .clicked()
        {
            *event = Some(InspectorEvent::DeleteSelectedBodies);
        }
        ui.add_space(3.0);
    }

    // 6. Section View Card
    card_frame().show(ui, |ui| {
        ui.label(
            RichText::new(format!("{} {}", ICON_CONTENT_CUT.codepoint, t!("tool-section-view-name")))
                .strong()
                .color(ACCENT_ORANGE),
        );
        if ui.checkbox(&mut state.section_enabled, t!("inspector-enable-section")).changed() {
            *event = Some(InspectorEvent::SectionViewChanged);
        }
        if state.section_enabled {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{}:", t!("inspector-revolve-axis"))).size(10.5));
                if ui.selectable_value(&mut state.section_axis, 0, "X").changed() {
                    *event = Some(InspectorEvent::SectionViewChanged);
                }
                if ui.selectable_value(&mut state.section_axis, 1, "Y").changed() {
                    *event = Some(InspectorEvent::SectionViewChanged);
                }
                if ui.selectable_value(&mut state.section_axis, 2, "Z").changed() {
                    *event = Some(InspectorEvent::SectionViewChanged);
                }
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("Offset:").size(10.5));
                if ui.add(Slider::new(&mut state.section_offset, -500.0..=500.0)).changed() {
                    *event = Some(InspectorEvent::SectionViewChanged);
                }
            });
            if ui.checkbox(&mut state.section_invert, t!("inspector-invert-direction")).changed() {
                *event = Some(InspectorEvent::SectionViewChanged);
            }
        }
    });
    ui.add_space(3.0);

    // 7. Model History (Undo / Redo 3D Model)
    card_frame().show(ui, |ui| {
        ui.label(RichText::new(t!("inspector-model-history")).size(10.5).color(TEXT_SECONDARY));
        ui.horizontal(|ui| {
            let undo_label = format!("{} {}", ICON_UNDO.codepoint, t!("drawer-undo"));
            let undo_btn = egui::Button::new(RichText::new(undo_label).size(10.5));
            if ui.add_enabled(state.can_undo_model, undo_btn).clicked() {
                *event = Some(InspectorEvent::UndoModel);
            }

            let redo_label = format!("{} {}", ICON_REDO.codepoint, t!("drawer-redo"));
            if ui
                .add_enabled(
                    state.can_redo_model,
                    egui::Button::new(RichText::new(redo_label).size(10.5)),
                )
                .clicked()
            {
                *event = Some(InspectorEvent::RedoModel);
            }
        });
    });

    // 8. Overview jika kosong
    if !has_2d_selection && state.selected_bodies_count == 0 {
        ui.add_space(2.0);
        card_frame().show(ui, |ui| {
            ui.label(RichText::new(t!("file-doc-ducad")).strong().size(11.0).color(TEXT_PRIMARY));
            ui.label(
                RichText::new(t!("inspector-entities-count", count = state.total_entities_count))
                .size(10.0)
                .color(TEXT_SECONDARY),
            );
            ui.label(
                RichText::new(t!("inspector-bodies-count", count = state.total_bodies_count))
                    .size(10.0)
                    .color(TEXT_SECONDARY),
            );
            ui.separator();
            ui.label(
                RichText::new(t!("inspector-select-object-hint"))
                .italics()
                .size(9.5)
                .color(TEXT_SECONDARY),
            );
        });
    }
}
