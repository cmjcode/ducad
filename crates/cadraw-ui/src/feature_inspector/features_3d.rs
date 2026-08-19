use egui::{Color32, RichText, Slider, Ui, Vec2};
use egui_material_icons::icons::{
    ICON_CALL_MERGE, ICON_CATEGORY, ICON_CLOSE, ICON_CONTENT_CUT, ICON_DELETE, ICON_OPEN_IN_FULL,
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
            RichText::new(format!("{} Pengukuran", ICON_STRAIGHTEN.codepoint))
                .strong()
                .size(11.5)
                .color(ACCENT_ORANGE),
        );

        if ui
            .checkbox(&mut state.show_all_dimensions, "Tampilkan Semua Ukuran")
            .on_hover_text("Tampilkan nominal ukuran tiap garis/rusuk elemen di kanvas")
            .changed()
        {
            *event = Some(InspectorEvent::ToggleShowAllDimensions);
        }

        if state.measurement_tool_active || !state.measurements.is_empty() {
            ui.separator();
        }
        if state.measurement_tool_active && state.measurements.is_empty() {
            ui.label(
                RichText::new("Klik 2 titik untuk jarak, 3 titik untuk sudut")
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
            if ui.button(RichText::new("Hapus Semua").size(10.5)).clicked() {
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
                RichText::new("💡 Resize: aktifkan \"Tampilkan Semua Ukuran\" (kartu Pengukuran di atas), lalu klik angka X/Y/Z yg muncul langsung di objek → ketik → Enter.")
                    .size(9.0)
                    .italics()
                    .color(TEXT_SECONDARY),
            );
            ui.label(
                RichText::new("Catatan: scale seragam (proporsional) — fillet/chamfer bisa ikut berubah bentuk kalau ukurannya besar sekali.")
                    .size(8.5)
                    .italics()
                    .color(TEXT_SECONDARY),
            );
        });
        ui.add_space(3.0);
    }

    // 3b. Face Extrude / Push-Pull Card (jika sisi 3D dipilih)
    if state.active_face_selected {
        card_frame().show(ui, |ui| {
            ui.label(
                RichText::new(format!("{} Extrude Sisi (Face)", ICON_OPEN_IN_FULL.codepoint))
                    .strong()
                    .size(11.5)
                    .color(ACCENT_BLUE),
            );
            ui.horizontal(|ui| {
                ui.label(RichText::new("Jarak (mm):").size(10.5).color(TEXT_SECONDARY));
                ui.add_sized(
                    Vec2::new(70.0, 18.0),
                    egui::TextEdit::singleline(&mut state.face_extrude_input),
                );
            });
            ui.horizontal(|ui| {
                if ui
                    .button(RichText::new("Tarik Extrude (+ / -)").size(11.0))
                    .clicked()
                {
                    if let Ok(dist) = state.face_extrude_input.trim().parse::<f64>() {
                        *event = Some(InspectorEvent::ApplyFaceExtrude { distance: dist });
                    }
                }
                if ui
                    .button(RichText::new("✏ Sketsa pada Sisi Ini").size(11.0))
                    .clicked()
                {
                    *event = Some(InspectorEvent::SketchOnFace);
                }
            });
        });
        ui.add_space(3.0);
    }

    // 4. Extrude Card (jika ada seleksi 2D untuk di-extrude)
    let has_2d_selection = !matches!(state.selected_entity, SelectedEntityData::None);
    if has_2d_selection {
        card_frame().show(ui, |ui| {
            ui.label(
                RichText::new(format!("{} Extrude Profil (3D)", ICON_OPEN_IN_FULL.codepoint))
                    .strong()
                    .size(11.5)
                    .color(ACCENT_BLUE),
            );
            ui.horizontal(|ui| {
                ui.label(RichText::new("Jarak (mm):").size(10.5).color(TEXT_SECONDARY));
                ui.add_sized(
                    Vec2::new(70.0, 18.0),
                    egui::TextEdit::singleline(&mut state.extrude_input),
                );
            });
            if ui.button(RichText::new("Eksekusi Extrude").size(11.0)).clicked() {
                if let Ok(dist) = state.extrude_input.trim().parse::<f64>() {
                    *event = Some(InspectorEvent::ApplyExtrude { distance: dist });
                }
            }
        });
        ui.add_space(3.0);

        // Revolve Card (Properties Panel Kanan)
        card_frame().show(ui, |ui| {
            ui.label(
                RichText::new(format!("{} Revolve 3D (Benda Putar)", ICON_REFRESH.codepoint))
                    .strong()
                    .size(11.5)
                    .color(ACCENT_BLUE),
            );
            ui.add_space(2.0);

            // Pilihan Sumbu
            ui.label(RichText::new("Poros Sumbu:").size(10.5).color(TEXT_SECONDARY));
            ui.radio_value(&mut state.revolve_axis_preset, 0, RichText::new("Sumbu Y (Vertikal)").size(10.5));
            ui.radio_value(&mut state.revolve_axis_preset, 1, RichText::new("Sumbu X (Horizontal)").size(10.5));
            ui.radio_value(&mut state.revolve_axis_preset, 2, RichText::new("Tepi Kiri Sketsa").size(10.5));
            ui.radio_value(&mut state.revolve_axis_preset, 3, RichText::new("Tepi Bawah Sketsa").size(10.5));
            ui.radio_value(&mut state.revolve_axis_preset, 4, RichText::new("✏️ Gambar 2 Titik Manual").size(10.5));

            ui.add_space(3.0);

            // Sudut
            ui.horizontal(|ui| {
                ui.label(RichText::new("Sudut:").size(10.5).color(TEXT_SECONDARY));
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
                ui.label(RichText::new("Derajat:").size(10.5).color(TEXT_SECONDARY));
                ui.add_sized(
                    Vec2::new(60.0, 18.0),
                    egui::TextEdit::singleline(&mut state.revolve_angle_input),
                );
                ui.label(RichText::new("°").size(10.5).color(TEXT_MUTED));
            });

            ui.add_space(3.0);

            // Arah Putar (CW vs CCW)
            ui.horizontal(|ui| {
                ui.label(RichText::new("Arah:").size(10.5).color(TEXT_SECONDARY));
                let dir_label = if state.revolve_reverse { "↻ Balik Arah (CW)" } else { "↺ Normal (CCW)" };
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
                            RichText::new("✏️ Klik 2 Titik di Kanvas")
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
                            RichText::new("🚀 Eksekusi Revolve")
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
                RichText::new(format!("{} Loft 3D", ICON_REFRESH.codepoint))
                    .strong()
                    .size(11.5)
                    .color(ACCENT_BLUE),
            );
            ui.separator();
            ui.label(RichText::new("Loft:").size(10.5).color(TEXT_SECONDARY));
            let staged_label = if state.loft_bottom_staged {
                "Profil bawah: ✓ Staged"
            } else {
                "Profil bawah: Belum diset"
            };
            ui.weak(staged_label);
            if ui.button(RichText::new("Set Profil Bawah").size(10.5)).clicked() {
                *event = Some(InspectorEvent::StageLoftBottom);
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new("Tinggi:").size(10.5).color(TEXT_SECONDARY));
                ui.add_sized(
                    Vec2::new(60.0, 18.0),
                    egui::TextEdit::singleline(&mut state.loft_height_input),
                );
            });
            if ui.button(RichText::new("Eksekusi Loft").size(11.0)).clicked() {
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
                if ui.button(RichText::new("Union").size(10.5)).clicked() {
                    *event = Some(InspectorEvent::ApplyBoolean(InspectorBooleanKind::Union));
                }
                if ui.button(RichText::new("Subtract").size(10.5)).clicked() {
                    *event = Some(InspectorEvent::ApplyBoolean(InspectorBooleanKind::Subtract));
                }
                if ui.button(RichText::new("Intersect").size(10.5)).clicked() {
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
                "[x] Mode Pilih Tepi (Aktif)"
            } else {
                "[ ] Mode Pilih Tepi Manual"
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
                    RichText::new(format!("{} tepi", state.selected_edges_count))
                        .size(10.5)
                        .color(TEXT_SECONDARY),
                );
            });

            if state.selected_edges_count > 0 && ui.small_button("Reset Seleksi Tepi").clicked() {
                *event = Some(InspectorEvent::ResetEdgePicking);
            }

            ui.separator();
            ui.horizontal(|ui| {
                ui.label(RichText::new("Radius:").size(10.5).color(TEXT_SECONDARY));
                ui.add_sized(
                    Vec2::new(55.0, 18.0),
                    egui::TextEdit::singleline(&mut state.fillet_input),
                );
                if ui.button(RichText::new("Fillet").size(10.5)).clicked() {
                    if let Ok(r) = state.fillet_input.trim().parse::<f64>() {
                        *event = Some(InspectorEvent::ApplyFillet { radius: r });
                    }
                }
            });

            ui.horizontal(|ui| {
                ui.label(RichText::new("Jarak:").size(10.5).color(TEXT_SECONDARY));
                ui.add_sized(
                    Vec2::new(55.0, 18.0),
                    egui::TextEdit::singleline(&mut state.chamfer_input),
                );
                if ui.button(RichText::new("Chamfer").size(10.5)).clicked() {
                    if let Ok(d) = state.chamfer_input.trim().parse::<f64>() {
                        *event = Some(InspectorEvent::ApplyChamfer { distance: d });
                    }
                }
            });
        });
        ui.add_space(3.0);

        // Shell / Hollow Card
        card_frame().show(ui, |ui| {
            ui.label(
                RichText::new(format!("{} Shell / Hollow", ICON_OPEN_IN_FULL.codepoint))
                    .strong()
                    .color(ACCENT_BLUE),
            );
            let face_btn_label = if state.picking_mode == InspectorPickMode::Face {
                "[x] Mode Pilih Wajah (Aktif)"
            } else {
                "[ ] Mode Pilih Wajah Manual"
            };
            ui.horizontal(|ui| {
                let single = state.selected_bodies_count == 1;
                if ui
                    .add_enabled(
                        single,
                        egui::Button::new(RichText::new(face_btn_label).size(10.5)),
                    )
                    .clicked()
                {
                    *event = Some(InspectorEvent::ToggleFacePicking);
                }
                ui.label(
                    RichText::new(format!("{} wajah", state.selected_faces_count))
                        .size(10.5)
                        .color(TEXT_SECONDARY),
                );
            });

            ui.horizontal(|ui| {
                ui.label(RichText::new("Tebal:").size(10.5).color(TEXT_SECONDARY));
                ui.add_sized(
                    Vec2::new(60.0, 18.0),
                    egui::TextEdit::singleline(&mut state.shell_input),
                );
            });
            if ui.button(RichText::new("Eksekusi Shell").size(10.5)).clicked() {
                if let Ok(t) = state.shell_input.trim().parse::<f64>() {
                    *event = Some(InspectorEvent::ApplyShell { thickness: t });
                }
            }
        });
        ui.add_space(3.0);

        // Hapus Body
        let del_text = format!("{} Hapus Body Terpilih", ICON_DELETE.codepoint);
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
            RichText::new(format!("{} Section View", ICON_CONTENT_CUT.codepoint))
                .strong()
                .color(ACCENT_ORANGE),
        );
        if ui.checkbox(&mut state.section_enabled, "Aktifkan Potongan").changed() {
            *event = Some(InspectorEvent::SectionViewChanged);
        }
        if state.section_enabled {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Sumbu:").size(10.5));
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
            if ui.checkbox(&mut state.section_invert, "Balik arah").changed() {
                *event = Some(InspectorEvent::SectionViewChanged);
            }
        }
    });
    ui.add_space(3.0);

    // 7. Model History (Undo / Redo 3D Model)
    card_frame().show(ui, |ui| {
        ui.label(RichText::new("Riwayat Model 3D:").size(10.5).color(TEXT_SECONDARY));
        ui.horizontal(|ui| {
            let undo_label = format!("{} Undo", ICON_UNDO.codepoint);
            let undo_btn = egui::Button::new(RichText::new(undo_label).size(10.5));
            if ui.add_enabled(state.can_undo_model, undo_btn).clicked() {
                *event = Some(InspectorEvent::UndoModel);
            }

            let redo_label = format!("{} Redo", ICON_REDO.codepoint);
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
            ui.label(RichText::new("Dokumen CADRAW").strong().size(11.0).color(TEXT_PRIMARY));
            ui.label(
                RichText::new(format!(
                    "• 2D Entitas: {} objek",
                    state.total_entities_count
                ))
                .size(10.0)
                .color(TEXT_SECONDARY),
            );
            ui.label(
                RichText::new(format!("• 3D Bodies: {} objek", state.total_bodies_count))
                    .size(10.0)
                    .color(TEXT_SECONDARY),
            );
            ui.separator();
            ui.label(
                RichText::new(
                    "Pilih objek di kanvas atau pohon item untuk melihat & mengubah dimensinya.",
                )
                .italics()
                .size(9.5)
                .color(TEXT_SECONDARY),
            );
        });
    }
}
