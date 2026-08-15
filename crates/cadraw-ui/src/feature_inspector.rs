//! Parametric Feature Inspector & History Cards bergaya Shapr3D dengan Material Icons.
//!
//! Menampilkan kartu-kartu operasi 3D mengambang di sisi kanan kanvas
//! (Extrude, Revolve, Loft, Boolean, Fillet, Chamfer, Shell, Section View)
//! lengkap dengan parameter yang dapat diperluas (*expandable cards*).

use egui::{Color32, RichText, ScrollArea, Slider, Ui, Vec2};
use egui_material_icons::icons::{
    ICON_CALL_MERGE, ICON_CATEGORY, ICON_CLOSE, ICON_CONTENT_CUT, ICON_DELETE,
    ICON_OPEN_IN_FULL, ICON_REDO, ICON_REFRESH, ICON_SETTINGS, ICON_UNDO,
};
use crate::theme::{card_frame, glass_frame, ACCENT_BLUE, ACCENT_ORANGE, TEXT_PRIMARY, TEXT_SECONDARY};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorBooleanKind {
    Union,
    Subtract,
    Intersect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorPickMode {
    None,
    Edge,
    Face,
}

#[derive(Debug, Clone)]
pub enum InspectorEvent {
    CloseInspector,
    UndoModel,
    RedoModel,
    ApplyExtrude { distance: f64 },
    ApplyRevolve,
    StageLoftBottom,
    ApplyLoft { height: f64 },
    ApplyBoolean(InspectorBooleanKind),
    ToggleEdgePicking,
    ResetEdgePicking,
    ApplyFillet { radius: f64 },
    ApplyChamfer { distance: f64 },
    ToggleFacePicking,
    ApplyShell { thickness: f64 },
    DeleteSelectedBodies,
    SectionViewChanged,
}

pub struct FeatureInspectorState {
    pub extrude_input: String,
    pub loft_height_input: String,
    pub loft_bottom_staged: bool,
    pub fillet_input: String,
    pub chamfer_input: String,
    pub shell_input: String,
    pub selected_bodies_count: usize,
    pub selected_edges_count: usize,
    pub selected_faces_count: usize,
    pub picking_mode: InspectorPickMode,
    pub can_undo_model: bool,
    pub can_redo_model: bool,
    pub status_message: Option<String>,
    pub section_enabled: bool,
    pub section_axis: u8, // 0 = X, 1 = Y, 2 = Z
    pub section_offset: f32,
    pub section_invert: bool,
}

impl Default for FeatureInspectorState {
    fn default() -> Self {
        Self {
            extrude_input: "20.0".to_string(),
            loft_height_input: "30.0".to_string(),
            loft_bottom_staged: false,
            fillet_input: "5.0".to_string(),
            chamfer_input: "2.0".to_string(),
            shell_input: "2.0".to_string(),
            selected_bodies_count: 0,
            selected_edges_count: 0,
            selected_faces_count: 0,
            picking_mode: InspectorPickMode::None,
            can_undo_model: false,
            can_redo_model: false,
            status_message: None,
            section_enabled: false,
            section_axis: 2, // Z
            section_offset: 0.0,
            section_invert: false,
        }
    }
}

pub struct FeatureInspector;

impl FeatureInspector {
    /// Render panel inspector fitur parametrik mengambang di kanan.
    pub fn show(
        ui: &mut Ui,
        state: &mut FeatureInspectorState,
    ) -> Option<InspectorEvent> {
        let mut event = None;

        glass_frame().show(ui, |ui| {
            ui.set_width(235.0);
            ui.set_max_height(580.0);
            ui.spacing_mut().item_spacing = Vec2::new(3.0, 4.0);

            // 1. Header & Model History Controls
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} Features", ICON_SETTINGS)).strong().size(12.0).color(TEXT_PRIMARY));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button(RichText::new(ICON_CLOSE).size(12.0).color(TEXT_SECONDARY)).clicked() {
                        event = Some(InspectorEvent::CloseInspector);
                    }
                });
            });

            ui.horizontal(|ui| {
                let undo_label = format!("{} Undo", ICON_UNDO);
                if ui.add_enabled(state.can_undo_model, egui::Button::new(RichText::new(undo_label).size(11.0))).clicked() {
                    event = Some(InspectorEvent::UndoModel);
                }
                let redo_label = format!("{} Redo", ICON_REDO);
                if ui.add_enabled(state.can_redo_model, egui::Button::new(RichText::new(redo_label).size(11.0))).clicked() {
                    event = Some(InspectorEvent::RedoModel);
                }
            });

            ui.separator();

            ScrollArea::vertical().show(ui, |ui| {
                // 2. Extrude Card
                card_frame().show(ui, |ui| {
                    ui.label(RichText::new(format!("{} Extrude (3D)", ICON_OPEN_IN_FULL)).strong().size(11.5).color(ACCENT_BLUE));
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Jarak (mm):").size(10.5).color(TEXT_SECONDARY));
                        ui.text_edit_singleline(&mut state.extrude_input);
                    });
                    if ui.button(RichText::new("Eksekusi Extrude").size(11.0)).clicked() {
                        if let Ok(dist) = state.extrude_input.trim().parse::<f64>() {
                            event = Some(InspectorEvent::ApplyExtrude { distance: dist });
                        }
                    }
                });

                ui.add_space(3.0);

                // 3. Revolve & Loft Card
                card_frame().show(ui, |ui| {
                    ui.label(RichText::new(format!("{} Revolve & Loft", ICON_REFRESH)).strong().color(ACCENT_BLUE));
                    if ui.button(format!("{} Revolve (Pilih Axis)", ICON_REFRESH)).clicked() {
                        event = Some(InspectorEvent::ApplyRevolve);
                    }
                    ui.separator();
                    ui.label(RichText::new("Loft:").size(11.0).color(TEXT_SECONDARY));
                    let staged_label = if state.loft_bottom_staged {
                        "Profil bawah: ✓ Staged"
                    } else {
                        "Profil bawah: Belum diset"
                    };
                    ui.weak(staged_label);
                    if ui.button("Set Profil Bawah dari Seleksi").clicked() {
                        event = Some(InspectorEvent::StageLoftBottom);
                    }
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Tinggi (mm):").size(11.0).color(TEXT_SECONDARY));
                        ui.text_edit_singleline(&mut state.loft_height_input);
                    });
                    if ui.button("Eksekusi Loft").clicked() {
                        if let Ok(h) = state.loft_height_input.trim().parse::<f64>() {
                            event = Some(InspectorEvent::ApplyLoft { height: h });
                        }
                    }
                });

                ui.add_space(3.0);

                // 4. Boolean Operations Card
                card_frame().show(ui, |ui| {
                    ui.label(RichText::new(format!("{} Boolean ({} Bodies)", ICON_CALL_MERGE, state.selected_bodies_count)).strong().color(ACCENT_BLUE));
                    ui.horizontal(|ui| {
                        if ui.button("Union").clicked() {
                            event = Some(InspectorEvent::ApplyBoolean(InspectorBooleanKind::Union));
                        }
                        if ui.button("Subtract").clicked() {
                            event = Some(InspectorEvent::ApplyBoolean(InspectorBooleanKind::Subtract));
                        }
                        if ui.button("Intersect").clicked() {
                            event = Some(InspectorEvent::ApplyBoolean(InspectorBooleanKind::Intersect));
                        }
                    });
                });

                ui.add_space(3.0);

                // 5. Fillet & Chamfer Card (dengan 3D Edge Picking)
                card_frame().show(ui, |ui| {
                    ui.label(RichText::new(format!("{} Fillet & Chamfer", ICON_CATEGORY)).strong().color(ACCENT_BLUE));
                    
                    // Edge picking toggle button
                    let edge_btn_label = if state.picking_mode == InspectorPickMode::Edge {
                        "[x] Mode Pilih Tepi (Aktif)"
                    } else {
                        "[ ] Mode Pilih Tepi Manual"
                    };
                    ui.horizontal(|ui| {
                        let single = state.selected_bodies_count == 1;
                        if ui.add_enabled(single, egui::Button::new(edge_btn_label)).clicked() {
                            event = Some(InspectorEvent::ToggleEdgePicking);
                        }
                        ui.label(RichText::new(format!("{} tepi", state.selected_edges_count)).size(11.0).color(TEXT_SECONDARY));
                    });

                    if state.selected_edges_count > 0 {
                        if ui.small_button("Reset Seleksi Tepi").clicked() {
                            event = Some(InspectorEvent::ResetEdgePicking);
                        }
                    }

                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Radius:").size(11.0).color(TEXT_SECONDARY));
                        ui.text_edit_singleline(&mut state.fillet_input);
                        if ui.button("Fillet").clicked() {
                            if let Ok(r) = state.fillet_input.trim().parse::<f64>() {
                                event = Some(InspectorEvent::ApplyFillet { radius: r });
                            }
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Jarak:").size(11.0).color(TEXT_SECONDARY));
                        ui.text_edit_singleline(&mut state.chamfer_input);
                        if ui.button("Chamfer").clicked() {
                            if let Ok(d) = state.chamfer_input.trim().parse::<f64>() {
                                event = Some(InspectorEvent::ApplyChamfer { distance: d });
                            }
                        }
                    });
                });

                ui.add_space(3.0);

                // 6. Shell / Hollow Card (dengan 3D Face Picking)
                card_frame().show(ui, |ui| {
                    ui.label(RichText::new(format!("{} Shell / Hollow", ICON_OPEN_IN_FULL)).strong().color(ACCENT_BLUE));
                    let face_btn_label = if state.picking_mode == InspectorPickMode::Face {
                        "[x] Mode Pilih Wajah (Aktif)"
                    } else {
                        "[ ] Mode Pilih Wajah Manual"
                    };
                    ui.horizontal(|ui| {
                        let single = state.selected_bodies_count == 1;
                        if ui.add_enabled(single, egui::Button::new(face_btn_label)).clicked() {
                            event = Some(InspectorEvent::ToggleFacePicking);
                        }
                        ui.label(RichText::new(format!("{} wajah", state.selected_faces_count)).size(11.0).color(TEXT_SECONDARY));
                    });

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Tebal (mm):").size(11.0).color(TEXT_SECONDARY));
                        ui.text_edit_singleline(&mut state.shell_input);
                    });
                    if ui.button("Eksekusi Shell").clicked() {
                        if let Ok(t) = state.shell_input.trim().parse::<f64>() {
                            event = Some(InspectorEvent::ApplyShell { thickness: t });
                        }
                    }
                });

                ui.add_space(3.0);

                // 7. Section View Card
                card_frame().show(ui, |ui| {
                    ui.label(RichText::new(format!("{} Section View", ICON_CONTENT_CUT)).strong().color(ACCENT_ORANGE));
                    if ui.checkbox(&mut state.section_enabled, "Aktifkan").changed() {
                        event = Some(InspectorEvent::SectionViewChanged);
                    }
                    if state.section_enabled {
                        ui.horizontal(|ui| {
                            ui.label("Sumbu:");
                            if ui.selectable_value(&mut state.section_axis, 0, "X").changed() {
                                event = Some(InspectorEvent::SectionViewChanged);
                            }
                            if ui.selectable_value(&mut state.section_axis, 1, "Y").changed() {
                                event = Some(InspectorEvent::SectionViewChanged);
                            }
                            if ui.selectable_value(&mut state.section_axis, 2, "Z").changed() {
                                event = Some(InspectorEvent::SectionViewChanged);
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("Offset:");
                            if ui.add(Slider::new(&mut state.section_offset, -500.0..=500.0)).changed() {
                                event = Some(InspectorEvent::SectionViewChanged);
                            }
                        });
                        if ui.checkbox(&mut state.section_invert, "Balik arah").changed() {
                            event = Some(InspectorEvent::SectionViewChanged);
                        }
                    }
                });

                ui.add_space(3.0);

                if state.selected_bodies_count > 0 {
                    let del_text = format!("{} Hapus Body Terpilih", ICON_DELETE);
                    if ui.button(RichText::new(del_text).color(Color32::from_rgb(240, 90, 90))).clicked() {
                        event = Some(InspectorEvent::DeleteSelectedBodies);
                    }
                }

                // Status / Error message
                if let Some(msg) = &state.status_message {
                    ui.separator();
                    ui.label(RichText::new(msg).color(Color32::from_rgb(240, 100, 100)).size(11.0));
                }
            });
        });

        event
    }
}
