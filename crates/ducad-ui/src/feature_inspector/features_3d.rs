use ducad_i18n::t;
use egui::{Color32, RichText, Slider, Ui, Vec2};
use egui_icons::icons::{
    ICON_AUTO_AWESOME, ICON_CALL_MERGE, ICON_CATEGORY, ICON_CLOSE, ICON_CONTENT_CUT, ICON_DELETE,
    ICON_PALETTE, ICON_REDO, ICON_REFRESH, ICON_SHIELD, ICON_STRAIGHTEN, ICON_TEXTURE, ICON_UNDO,
    ICON_WATER_DROP,
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

        // 4. CMF & Material Desain Industri
        card_frame().show(ui, |ui| {
            ui.label(
                RichText::new(format!("{} {}", ICON_PALETTE.codepoint, t!("inspector-cmf-title")))
                    .strong()
                    .size(11.5)
                    .color(ACCENT_BLUE),
            );
            ui.add_space(2.0);

            let mut current_mat = body.material;
            let mut mat_changed = false;

            // 1. Preset Material Buttons
            ui.label(RichText::new(t!("inspector-cmf-presets")).size(10.0).color(TEXT_SECONDARY));

            let presets = [
                (ducad_core::MaterialPreset::MattePlastic, t!("material-matte-plastic"), "ABS / PC"),
                (ducad_core::MaterialPreset::GlossyPlastic, t!("material-glossy-plastic"), "High-Gloss"),
                (ducad_core::MaterialPreset::AnodizedAluminum, t!("material-anodized-aluminum"), "Satin / Brushed"),
                (ducad_core::MaterialPreset::PolishedChrome, t!("material-polished-chrome"), "Mirror Finish"),
                (ducad_core::MaterialPreset::TranslucentGlass, t!("material-translucent-glass"), "Clear Acrylic"),
            ];

            for (preset, name, desc) in presets {
                let is_selected = current_mat.preset == preset;
                let bg_color = if is_selected {
                    Color32::from_rgb(28, 55, 95)
                } else {
                    Color32::from_rgb(24, 28, 36)
                };
                let stroke_color = if is_selected {
                    ACCENT_BLUE
                } else {
                    Color32::from_rgb(45, 52, 65)
                };

                let btn_resp = egui::Frame::new()
                    .fill(bg_color)
                    .stroke(egui::Stroke::new(if is_selected { 1.2 } else { 0.5 }, stroke_color))
                    .corner_radius(egui::CornerRadius::same(5))
                    .inner_margin(egui::Margin::symmetric(6, 4))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let icon = match preset {
                                ducad_core::MaterialPreset::MattePlastic => ICON_TEXTURE.codepoint,
                                ducad_core::MaterialPreset::GlossyPlastic => ICON_AUTO_AWESOME.codepoint,
                                ducad_core::MaterialPreset::AnodizedAluminum => ICON_SHIELD.codepoint,
                                ducad_core::MaterialPreset::PolishedChrome => ICON_AUTO_AWESOME.codepoint,
                                ducad_core::MaterialPreset::TranslucentGlass => ICON_WATER_DROP.codepoint,
                                _ => ICON_PALETTE.codepoint,
                            };
                            ui.label(RichText::new(icon).size(12.0).color(if is_selected { ACCENT_BLUE } else { TEXT_SECONDARY }));
                            ui.vertical(|ui| {
                                ui.label(RichText::new(name).size(10.5).strong().color(if is_selected { TEXT_PRIMARY } else { TEXT_SECONDARY }));
                                ui.label(RichText::new(desc).size(8.5).color(TEXT_MUTED));
                            });
                        });
                    });

                if btn_resp.response.interact(egui::Sense::click()).clicked() {
                    current_mat = match preset {
                        ducad_core::MaterialPreset::MattePlastic => ducad_core::Material::matte_plastic(Some(current_mat.base_color)),
                        ducad_core::MaterialPreset::GlossyPlastic => ducad_core::Material::glossy_plastic(Some(current_mat.base_color)),
                        ducad_core::MaterialPreset::AnodizedAluminum => ducad_core::Material::anodized_aluminum(Some(current_mat.base_color)),
                        ducad_core::MaterialPreset::PolishedChrome => ducad_core::Material::polished_chrome(Some(current_mat.base_color)),
                        ducad_core::MaterialPreset::TranslucentGlass => ducad_core::Material::translucent_glass(Some(current_mat.base_color)),
                        _ => current_mat,
                    };
                    mat_changed = true;
                }
            }

            ui.add_space(4.0);
            ui.separator();

            // 2. Industrial Color Palette Swatches
            ui.label(RichText::new(t!("inspector-cmf-color")).size(10.0).color(TEXT_SECONDARY));

            let swatches: &[([f32; 4], &str)] = match current_mat.preset {
                ducad_core::MaterialPreset::MattePlastic => &[
                    ([0.22, 0.24, 0.27, 1.0], "Stealth Charcoal"),
                    ([0.55, 0.58, 0.62, 1.0], "Industrial Slate"),
                    ([0.88, 0.90, 0.92, 1.0], "Pure White"),
                    ([0.18, 0.32, 0.48, 1.0], "Nordic Blue"),
                    ([0.32, 0.40, 0.28, 1.0], "Olive Green"),
                ],
                ducad_core::MaterialPreset::GlossyPlastic => &[
                    ([0.08, 0.08, 0.10, 1.0], "Piano Black"),
                    ([0.96, 0.38, 0.12, 1.0], "Signal Orange"),
                    ([0.92, 0.18, 0.18, 1.0], "Racing Red"),
                    ([0.96, 0.82, 0.10, 1.0], "Cyber Yellow"),
                    ([0.98, 0.98, 0.98, 1.0], "Ceramic White"),
                ],
                ducad_core::MaterialPreset::AnodizedAluminum => &[
                    ([0.72, 0.75, 0.80, 1.0], "Space Gray"),
                    ([0.88, 0.90, 0.92, 1.0], "Satin Silver"),
                    ([0.22, 0.35, 0.52, 1.0], "Midnight Blue"),
                    ([0.85, 0.78, 0.65, 1.0], "Champagne Gold"),
                    ([0.82, 0.65, 0.68, 1.0], "Rose Titanium"),
                ],
                ducad_core::MaterialPreset::PolishedChrome => &[
                    ([0.92, 0.94, 0.96, 1.0], "Mirror Chrome"),
                    ([0.75, 0.78, 0.82, 1.0], "Stainless Steel"),
                    ([0.45, 0.48, 0.52, 1.0], "Gunmetal"),
                    ([0.82, 0.68, 0.48, 1.0], "Polished Bronze"),
                ],
                ducad_core::MaterialPreset::TranslucentGlass => &[
                    ([0.75, 0.88, 0.96, 0.38], "Clear Ice Glass"),
                    ([0.28, 0.30, 0.35, 0.45], "Smoky Gray Glass"),
                    ([0.25, 0.82, 0.88, 0.38], "Cyan Tint Glass"),
                    ([0.28, 0.78, 0.48, 0.38], "Emerald Glass"),
                    ([0.88, 0.22, 0.32, 0.38], "Ruby Glass"),
                ],
                _ => &[
                    ([0.62, 0.68, 0.76, 1.0], "CAD Grey"),
                    ([0.20, 0.65, 0.95, 1.0], "Accent Blue"),
                    ([0.95, 0.40, 0.15, 1.0], "Accent Orange"),
                ],
            };

            ui.horizontal(|ui| {
                for &(col, label) in swatches {
                    let c32 = Color32::from_rgba_unmultiplied(
                        (col[0] * 255.0) as u8,
                        (col[1] * 255.0) as u8,
                        (col[2] * 255.0) as u8,
                        (col[3] * 255.0) as u8,
                    );
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(18.0, 18.0), egui::Sense::click());
                    let is_active = (current_mat.base_color[0] - col[0]).abs() < 0.05
                        && (current_mat.base_color[1] - col[1]).abs() < 0.05
                        && (current_mat.base_color[2] - col[2]).abs() < 0.05;

                    ui.painter().circle_filled(rect.center(), 8.0, c32);
                    ui.painter().circle_stroke(
                        rect.center(),
                        8.0,
                        egui::Stroke::new(if is_active { 1.8 } else { 0.8 }, if is_active { ACCENT_BLUE } else { Color32::from_rgb(70, 78, 92) }),
                    );

                    if resp.on_hover_text(label).clicked() {
                        current_mat.base_color = col;
                        mat_changed = true;
                    }
                }

                // Custom Color Picker
                if ui.color_edit_button_rgba_unmultiplied(&mut current_mat.base_color).changed() {
                    current_mat.preset = ducad_core::MaterialPreset::Custom;
                    mat_changed = true;
                }
            });

            ui.add_space(4.0);

            // 3. Fine-Tuning Sliders
            ui.collapsing(RichText::new(format!("⚙ {}", t!("inspector-cmf-fine-tune"))).size(10.0).color(TEXT_SECONDARY), |ui| {
                // Roughness
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t!("material-roughness")).size(9.5).color(TEXT_SECONDARY));
                    let r_slider = ui.add(Slider::new(&mut current_mat.roughness, 0.02..=1.0).show_value(true).fixed_decimals(2));
                    if r_slider.changed() {
                        current_mat.preset = ducad_core::MaterialPreset::Custom;
                        mat_changed = true;
                    }
                });

                // Metallic
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t!("material-metallic")).size(9.5).color(TEXT_SECONDARY));
                    let m_slider = ui.add(Slider::new(&mut current_mat.metallic, 0.0..=1.0).show_value(true).fixed_decimals(2));
                    if m_slider.changed() {
                        current_mat.preset = ducad_core::MaterialPreset::Custom;
                        mat_changed = true;
                    }
                });

                // Clearcoat
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t!("material-clearcoat")).size(9.5).color(TEXT_SECONDARY));
                    let c_slider = ui.add(Slider::new(&mut current_mat.clearcoat, 0.0..=1.0).show_value(true).fixed_decimals(2));
                    if c_slider.changed() {
                        current_mat.preset = ducad_core::MaterialPreset::Custom;
                        mat_changed = true;
                    }
                });

                // Opacity / Alpha
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t!("material-opacity")).size(9.5).color(TEXT_SECONDARY));
                    let mut alpha = current_mat.base_color[3];
                    let a_slider = ui.add(Slider::new(&mut alpha, 0.05..=1.0).show_value(true).fixed_decimals(2));
                    if a_slider.changed() {
                        current_mat.base_color[3] = alpha;
                        current_mat.preset = ducad_core::MaterialPreset::Custom;
                        mat_changed = true;
                    }
                });
            });

            if mat_changed {
                *event = Some(InspectorEvent::SetBodyMaterial {
                    id_raw: body.id_raw,
                    material: current_mat,
                });
            }
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
            ui.checkbox(
                &mut state.fillet_variable_enabled,
                RichText::new(t!("inspector-fillet-variable-toggle")).size(10.0),
            );
            if state.fillet_variable_enabled {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{}:", t!("param-radius-start")))
                            .size(10.0)
                            .color(TEXT_SECONDARY),
                    );
                    ui.add_sized(
                        Vec2::new(42.0, 18.0),
                        egui::TextEdit::singleline(&mut state.fillet_input),
                    );
                    ui.label(
                        RichText::new(format!("{}:", t!("param-radius-end")))
                            .size(10.0)
                            .color(TEXT_SECONDARY),
                    );
                    ui.add_sized(
                        Vec2::new(42.0, 18.0),
                        egui::TextEdit::singleline(&mut state.fillet_radius_end_input),
                    );
                });
                if ui
                    .button(RichText::new(t!("tool-fillet-variable")).size(10.5))
                    .clicked()
                {
                    if let (Ok(r1), Ok(r2)) = (
                        state.fillet_input.trim().parse::<f64>(),
                        state.fillet_radius_end_input.trim().parse::<f64>(),
                    ) {
                        *event = Some(InspectorEvent::ApplyVariableFillet {
                            radius_start: r1,
                            radius_end: r2,
                        });
                    }
                }
            } else {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{}:", t!("param-radius")))
                            .size(10.5)
                            .color(TEXT_SECONDARY),
                    );
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
            }

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
