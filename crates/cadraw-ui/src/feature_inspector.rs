//! Parametric Properties & Feature Inspector bergaya Shapr3D dengan Material Icons.
//!
//! Menampilkan panel properti dan fitur parametrik yang terpasang di sisi kanan kanvas:
//! - Inspeksi & modifikasi data geometris entitas 2D terpilih (Line, Circle, Arc, Ellipse)
//! - Tombol constraint geometris kontekstual terintegrasi
//! - Operasi 3D parametrik (Extrude, Revolve, Loft, Boolean, Fillet, Chamfer, Shell, Section View)
//! - Kontrol Auto-Hide dan tombol Sembunyikan (Hide/Collapse)

use egui::{Color32, RichText, ScrollArea, Slider, Ui, Vec2};
use egui_material_icons::icons::{
    ICON_CALL_MERGE, ICON_CATEGORY, ICON_CLOSE, ICON_CONTENT_CUT, ICON_DELETE,
    ICON_EDIT, ICON_LOCK, ICON_OPEN_IN_FULL, ICON_PUSH_PIN, ICON_REDO,
    ICON_REFRESH, ICON_TUNE, ICON_UNDO,
};
use crate::theme::{card_frame, glass_frame, ACCENT_BLUE, ACCENT_GREEN, ACCENT_ORANGE, TEXT_PRIMARY, TEXT_SECONDARY};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorConstraintAction {
    Horizontal,
    Vertical,
    Parallel,
    Perpendicular,
    EqualLength,
    EqualRadius,
    Tangent,
    Coincident,
    Fixed,
    Symmetric,
}

#[derive(Debug, Clone)]
pub enum SelectedEntityData {
    None,
    Line {
        id_raw: u64,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
        length: f64,
        angle_deg: f64,
    },
    Circle {
        id_raw: u64,
        center_x: f64,
        center_y: f64,
        radius: f64,
        diameter: f64,
    },
    Arc {
        id_raw: u64,
        center_x: f64,
        center_y: f64,
        radius: f64,
        start_angle_deg: f64,
        end_angle_deg: f64,
    },
    Ellipse {
        id_raw: u64,
        center_x: f64,
        center_y: f64,
        radius_x: f64,
        radius_y: f64,
    },
    MultipleEntities {
        count: usize,
    },
}

#[derive(Debug, Clone)]
pub struct SelectedBodyData {
    pub id_raw: u64,
    pub name: String,
    pub vertices_count: usize,
    pub triangles_count: usize,
    pub bbox_size: [f32; 3],
}

#[derive(Debug, Clone)]
pub enum InspectorEvent {
    CloseInspector,
    ToggleAutoHide,
    UndoModel,
    RedoModel,
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
    ApplyConstraint(InspectorConstraintAction),
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
    pub auto_hide_enabled: bool,
    pub selected_entity: SelectedEntityData,
    pub selected_body: Option<SelectedBodyData>,
    pub selected_bodies_count: usize,
    pub selected_edges_count: usize,
    pub selected_faces_count: usize,
    pub total_entities_count: usize,
    pub total_bodies_count: usize,

    // Inputs for 2D entity property edit
    pub entity_p1_x: String,
    pub entity_p1_y: String,
    pub entity_p2_x: String,
    pub entity_p2_y: String,
    pub entity_val_1: String,
    pub entity_val_2: String,

    // Inputs for 3D operations
    pub extrude_input: String,
    pub loft_height_input: String,
    pub loft_bottom_staged: bool,
    pub fillet_input: String,
    pub chamfer_input: String,
    pub shell_input: String,
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
            auto_hide_enabled: true,
            selected_entity: SelectedEntityData::None,
            selected_body: None,
            selected_bodies_count: 0,
            selected_edges_count: 0,
            selected_faces_count: 0,
            total_entities_count: 0,
            total_bodies_count: 0,

            entity_p1_x: String::new(),
            entity_p1_y: String::new(),
            entity_p2_x: String::new(),
            entity_p2_y: String::new(),
            entity_val_1: String::new(),
            entity_val_2: String::new(),

            extrude_input: "20.0".to_string(),
            loft_height_input: "30.0".to_string(),
            loft_bottom_staged: false,
            fillet_input: "5.0".to_string(),
            chamfer_input: "2.0".to_string(),
            shell_input: "2.0".to_string(),
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
    /// Render panel inspector properti & fitur yang fixed di kanan kanvas.
    pub fn show(
        ui: &mut Ui,
        state: &mut FeatureInspectorState,
    ) -> Option<InspectorEvent> {
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
                        .small_button(RichText::new(format!("{} {}", ICON_PUSH_PIN, pin_text)).size(10.0).color(pin_color))
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

            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                // 2. Tampilkan Konten Sesuai Status Seleksi
                match &state.selected_entity {
                    SelectedEntityData::Line {
                        id_raw,
                        start_x: _,
                        start_y: _,
                        end_x: _,
                        end_y: _,
                        length,
                        angle_deg,
                    } => {
                        let id_raw = *id_raw;
                        let length = *length;
                        let angle_deg = *angle_deg;
                        card_frame().show(ui, |ui| {
                            ui.label(
                                RichText::new(format!("{} Garis (Line)", ICON_EDIT))
                                    .strong()
                                    .size(11.5)
                                    .color(ACCENT_BLUE),
                            );

                            ui.add_space(2.0);
                            ui.label(RichText::new("Titik Awal (Start):").size(10.0).color(TEXT_SECONDARY));
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("X:").size(10.5));
                                ui.add_sized(Vec2::new(72.0, 18.0), egui::TextEdit::singleline(&mut state.entity_p1_x));
                                ui.label(RichText::new("Y:").size(10.5));
                                ui.add_sized(Vec2::new(72.0, 18.0), egui::TextEdit::singleline(&mut state.entity_p1_y));
                            });

                            ui.label(RichText::new("Titik Akhir (End):").size(10.0).color(TEXT_SECONDARY));
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("X:").size(10.5));
                                ui.add_sized(Vec2::new(72.0, 18.0), egui::TextEdit::singleline(&mut state.entity_p2_x));
                                ui.label(RichText::new("Y:").size(10.5));
                                ui.add_sized(Vec2::new(72.0, 18.0), egui::TextEdit::singleline(&mut state.entity_p2_y));
                            });

                            ui.add_space(2.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(format!("Panjang: {:.2} mm", length)).size(10.5).color(TEXT_SECONDARY));
                                ui.label(RichText::new(format!("Sudut: {:.1}°", angle_deg)).size(10.5).color(TEXT_SECONDARY));
                            });

                            ui.add_space(2.0);
                            if ui.button(RichText::new("Terapkan Koordinat").size(11.0)).clicked() {
                                if let (Ok(x1), Ok(y1), Ok(x2), Ok(y2)) = (
                                    state.entity_p1_x.trim().parse::<f64>(),
                                    state.entity_p1_y.trim().parse::<f64>(),
                                    state.entity_p2_x.trim().parse::<f64>(),
                                    state.entity_p2_y.trim().parse::<f64>(),
                                ) {
                                    event = Some(InspectorEvent::UpdateEntityLine {
                                        id_raw,
                                        start_x: x1,
                                        start_y: y1,
                                        end_x: x2,
                                        end_y: y2,
                                    });
                                }
                            }

                            ui.separator();
                            ui.label(RichText::new("Constraint Cepat:").size(10.0).color(TEXT_SECONDARY));
                            ui.horizontal(|ui| {
                                if ui.button(RichText::new("— Horiz").size(10.5)).on_hover_text("Bikin garis horizontal").clicked() {
                                    event = Some(InspectorEvent::ApplyConstraint(InspectorConstraintAction::Horizontal));
                                }
                                if ui.button(RichText::new("| Vert").size(10.5)).on_hover_text("Bikin garis vertikal").clicked() {
                                    event = Some(InspectorEvent::ApplyConstraint(InspectorConstraintAction::Vertical));
                                }
                                if ui.button(RichText::new(format!("{} Lock", ICON_LOCK)).size(10.5)).on_hover_text("Kunci posisi").clicked() {
                                    event = Some(InspectorEvent::ApplyConstraint(InspectorConstraintAction::Fixed));
                                }
                            });
                        });
                        ui.add_space(3.0);
                    }

                    SelectedEntityData::Circle {
                        id_raw,
                        center_x: _,
                        center_y: _,
                        radius: _,
                        diameter,
                    } => {
                        let id_raw = *id_raw;
                        let diameter = *diameter;
                        card_frame().show(ui, |ui| {
                            ui.label(
                                RichText::new(format!("{} Lingkaran (Circle)", ICON_EDIT))
                                    .strong()
                                    .size(11.5)
                                    .color(ACCENT_BLUE),
                            );

                            ui.add_space(2.0);
                            ui.label(RichText::new("Pusat (Center):").size(10.0).color(TEXT_SECONDARY));
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("X:").size(10.5));
                                ui.add_sized(Vec2::new(72.0, 18.0), egui::TextEdit::singleline(&mut state.entity_p1_x));
                                ui.label(RichText::new("Y:").size(10.5));
                                ui.add_sized(Vec2::new(72.0, 18.0), egui::TextEdit::singleline(&mut state.entity_p1_y));
                            });

                            ui.label(RichText::new("Radius (Jari-jari mm):").size(10.0).color(TEXT_SECONDARY));
                            ui.horizontal(|ui| {
                                ui.add_sized(Vec2::new(90.0, 18.0), egui::TextEdit::singleline(&mut state.entity_val_1));
                                ui.label(RichText::new(format!("Ø {:.2} mm", diameter)).size(10.5).color(TEXT_SECONDARY));
                            });

                            ui.add_space(2.0);
                            if ui.button(RichText::new("Terapkan Dimensi").size(11.0)).clicked() {
                                if let (Ok(cx), Ok(cy), Ok(r)) = (
                                    state.entity_p1_x.trim().parse::<f64>(),
                                    state.entity_p1_y.trim().parse::<f64>(),
                                    state.entity_val_1.trim().parse::<f64>(),
                                ) {
                                    event = Some(InspectorEvent::UpdateEntityCircle {
                                        id_raw,
                                        center_x: cx,
                                        center_y: cy,
                                        radius: r,
                                    });
                                }
                            }

                            ui.separator();
                            if ui.button(RichText::new(format!("{} Lock Pusat", ICON_LOCK)).size(10.5)).clicked() {
                                event = Some(InspectorEvent::ApplyConstraint(InspectorConstraintAction::Fixed));
                            }
                        });
                        ui.add_space(3.0);
                    }

                    SelectedEntityData::Arc {
                        id_raw,
                        center_x: _,
                        center_y: _,
                        radius: _,
                        start_angle_deg: _,
                        end_angle_deg: _,
                    } => {
                        let id_raw = *id_raw;
                        card_frame().show(ui, |ui| {
                            ui.label(
                                RichText::new(format!("{} Busur (Arc)", ICON_EDIT))
                                    .strong()
                                    .size(11.5)
                                    .color(ACCENT_BLUE),
                            );

                            ui.add_space(2.0);
                            ui.label(RichText::new("Pusat (Center):").size(10.0).color(TEXT_SECONDARY));
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("X:").size(10.5));
                                ui.add_sized(Vec2::new(72.0, 18.0), egui::TextEdit::singleline(&mut state.entity_p1_x));
                                ui.label(RichText::new("Y:").size(10.5));
                                ui.add_sized(Vec2::new(72.0, 18.0), egui::TextEdit::singleline(&mut state.entity_p1_y));
                            });

                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Radius:").size(10.5));
                                ui.add_sized(Vec2::new(80.0, 18.0), egui::TextEdit::singleline(&mut state.entity_val_1));
                            });

                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Sudut:").size(10.5));
                                ui.add_sized(Vec2::new(50.0, 18.0), egui::TextEdit::singleline(&mut state.entity_val_2));
                                ui.label(RichText::new("s/d").size(10.0));
                                ui.add_sized(Vec2::new(50.0, 18.0), egui::TextEdit::singleline(&mut state.entity_p2_x));
                            });

                            ui.add_space(2.0);
                            if ui.button(RichText::new("Terapkan Dimensi").size(11.0)).clicked() {
                                if let (Ok(cx), Ok(cy), Ok(r), Ok(a1), Ok(a2)) = (
                                    state.entity_p1_x.trim().parse::<f64>(),
                                    state.entity_p1_y.trim().parse::<f64>(),
                                    state.entity_val_1.trim().parse::<f64>(),
                                    state.entity_val_2.trim().parse::<f64>(),
                                    state.entity_p2_x.trim().parse::<f64>(),
                                ) {
                                    event = Some(InspectorEvent::UpdateEntityArc {
                                        id_raw,
                                        center_x: cx,
                                        center_y: cy,
                                        radius: r,
                                        start_angle_deg: a1,
                                        end_angle_deg: a2,
                                    });
                                }
                            }
                        });
                        ui.add_space(3.0);
                    }

                    SelectedEntityData::Ellipse {
                        id_raw,
                        center_x: _,
                        center_y: _,
                        radius_x: _,
                        radius_y: _,
                    } => {
                        let id_raw = *id_raw;
                        card_frame().show(ui, |ui| {
                            ui.label(
                                RichText::new(format!("{} Elips (Ellipse)", ICON_EDIT))
                                    .strong()
                                    .size(11.5)
                                    .color(ACCENT_BLUE),
                            );

                            ui.add_space(2.0);
                            ui.label(RichText::new("Pusat (Center):").size(10.0).color(TEXT_SECONDARY));
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("X:").size(10.5));
                                ui.add_sized(Vec2::new(72.0, 18.0), egui::TextEdit::singleline(&mut state.entity_p1_x));
                                ui.label(RichText::new("Y:").size(10.5));
                                ui.add_sized(Vec2::new(72.0, 18.0), egui::TextEdit::singleline(&mut state.entity_p1_y));
                            });

                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Rx:").size(10.5));
                                ui.add_sized(Vec2::new(60.0, 18.0), egui::TextEdit::singleline(&mut state.entity_val_1));
                                ui.label(RichText::new("Ry:").size(10.5));
                                ui.add_sized(Vec2::new(60.0, 18.0), egui::TextEdit::singleline(&mut state.entity_val_2));
                            });

                            ui.add_space(2.0);
                            if ui.button(RichText::new("Terapkan Dimensi").size(11.0)).clicked() {
                                if let (Ok(cx), Ok(cy), Ok(rx), Ok(ry)) = (
                                    state.entity_p1_x.trim().parse::<f64>(),
                                    state.entity_p1_y.trim().parse::<f64>(),
                                    state.entity_val_1.trim().parse::<f64>(),
                                    state.entity_val_2.trim().parse::<f64>(),
                                ) {
                                    event = Some(InspectorEvent::UpdateEntityEllipse {
                                        id_raw,
                                        center_x: cx,
                                        center_y: cy,
                                        radius_x: rx,
                                        radius_y: ry,
                                    });
                                }
                            }
                        });
                        ui.add_space(3.0);
                    }

                    SelectedEntityData::MultipleEntities { count } => {
                        let count = *count;
                        card_frame().show(ui, |ui| {
                            ui.label(
                                RichText::new(format!("{} {} Entitas 2D Terpilih", ICON_EDIT, count))
                                    .strong()
                                    .size(11.5)
                                    .color(ACCENT_BLUE),
                            );

                            ui.add_space(2.0);
                            ui.label(RichText::new("Terapkan Constraint Bersama:").size(10.0).color(TEXT_SECONDARY));

                            ui.horizontal(|ui| {
                                if ui.button(RichText::new("// Sejajar").size(10.0)).on_hover_text("Parallel (2 Garis)").clicked() {
                                    event = Some(InspectorEvent::ApplyConstraint(InspectorConstraintAction::Parallel));
                                }
                                if ui.button(RichText::new("⊥ Siku").size(10.0)).on_hover_text("Perpendicular (2 Garis)").clicked() {
                                    event = Some(InspectorEvent::ApplyConstraint(InspectorConstraintAction::Perpendicular));
                                }
                                if ui.button(RichText::new("== Panjang").size(10.0)).on_hover_text("Equal Length").clicked() {
                                    event = Some(InspectorEvent::ApplyConstraint(InspectorConstraintAction::EqualLength));
                                }
                            });

                            ui.horizontal(|ui| {
                                if ui.button(RichText::new("=R Radius").size(10.0)).on_hover_text("Equal Radius").clicked() {
                                    event = Some(InspectorEvent::ApplyConstraint(InspectorConstraintAction::EqualRadius));
                                }
                                if ui.button(RichText::new("tan Singgung").size(10.0)).on_hover_text("Tangent").clicked() {
                                    event = Some(InspectorEvent::ApplyConstraint(InspectorConstraintAction::Tangent));
                                }
                                if ui.button(RichText::new(">< Berimpit").size(10.0)).on_hover_text("Coincident").clicked() {
                                    event = Some(InspectorEvent::ApplyConstraint(InspectorConstraintAction::Coincident));
                                }
                            });
                        });
                        ui.add_space(3.0);
                    }

                    SelectedEntityData::None => {}
                }

                // 3. 3D Body Info (jika body dipilih)
                if let Some(body) = &state.selected_body {
                    card_frame().show(ui, |ui| {
                        ui.label(
                            RichText::new(format!("{} {}", ICON_CATEGORY, body.name))
                                .strong()
                                .size(11.5)
                                .color(ACCENT_GREEN),
                        );
                        ui.label(RichText::new(format!("Mesh: {} vert, {} tri", body.vertices_count, body.triangles_count)).size(10.0).color(TEXT_SECONDARY));
                        ui.label(RichText::new(format!("BBox: {:.1}×{:.1}×{:.1} mm", body.bbox_size[0], body.bbox_size[1], body.bbox_size[2])).size(10.0).color(TEXT_SECONDARY));
                    });
                    ui.add_space(3.0);
                }

                // 4. Extrude Card (jika ada seleksi 2D untuk di-extrude)
                let has_2d_selection = !matches!(state.selected_entity, SelectedEntityData::None);
                if has_2d_selection {
                    card_frame().show(ui, |ui| {
                        ui.label(RichText::new(format!("{} Extrude Profil (3D)", ICON_OPEN_IN_FULL)).strong().size(11.5).color(ACCENT_BLUE));
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Jarak (mm):").size(10.5).color(TEXT_SECONDARY));
                            ui.add_sized(Vec2::new(70.0, 18.0), egui::TextEdit::singleline(&mut state.extrude_input));
                        });
                        if ui.button(RichText::new("Eksekusi Extrude").size(11.0)).clicked() {
                            if let Ok(dist) = state.extrude_input.trim().parse::<f64>() {
                                event = Some(InspectorEvent::ApplyExtrude { distance: dist });
                            }
                        }
                    });
                    ui.add_space(3.0);

                    // Revolve & Loft
                    card_frame().show(ui, |ui| {
                        ui.label(RichText::new(format!("{} Revolve & Loft", ICON_REFRESH)).strong().color(ACCENT_BLUE));
                        if ui.button(format!("{} Revolve (Pilih Axis)", ICON_REFRESH)).clicked() {
                            event = Some(InspectorEvent::ApplyRevolve);
                        }
                        ui.separator();
                        ui.label(RichText::new("Loft:").size(10.5).color(TEXT_SECONDARY));
                        let staged_label = if state.loft_bottom_staged {
                            "Profil bawah: ✓ Staged"
                        } else {
                            "Profil bawah: Belum diset"
                        };
                        ui.weak(staged_label);
                        if ui.button(RichText::new("Set Profil Bawah").size(10.5)).clicked() {
                            event = Some(InspectorEvent::StageLoftBottom);
                        }
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Tinggi:").size(10.5).color(TEXT_SECONDARY));
                            ui.add_sized(Vec2::new(60.0, 18.0), egui::TextEdit::singleline(&mut state.loft_height_input));
                        });
                        if ui.button(RichText::new("Eksekusi Loft").size(11.0)).clicked() {
                            if let Ok(h) = state.loft_height_input.trim().parse::<f64>() {
                                event = Some(InspectorEvent::ApplyLoft { height: h });
                            }
                        }
                    });
                    ui.add_space(3.0);
                }

                // 5. Boolean Operations (jika ada body terpilih)
                if state.selected_bodies_count > 0 {
                    card_frame().show(ui, |ui| {
                        ui.label(RichText::new(format!("{} Boolean ({} Bodies)", ICON_CALL_MERGE, state.selected_bodies_count)).strong().color(ACCENT_BLUE));
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("Union").size(10.5)).clicked() {
                                event = Some(InspectorEvent::ApplyBoolean(InspectorBooleanKind::Union));
                            }
                            if ui.button(RichText::new("Subtract").size(10.5)).clicked() {
                                event = Some(InspectorEvent::ApplyBoolean(InspectorBooleanKind::Subtract));
                            }
                            if ui.button(RichText::new("Intersect").size(10.5)).clicked() {
                                event = Some(InspectorEvent::ApplyBoolean(InspectorBooleanKind::Intersect));
                            }
                        });
                    });
                    ui.add_space(3.0);

                    // Fillet & Chamfer Card
                    card_frame().show(ui, |ui| {
                        ui.label(RichText::new(format!("{} Fillet & Chamfer", ICON_CATEGORY)).strong().color(ACCENT_BLUE));
                        
                        let edge_btn_label = if state.picking_mode == InspectorPickMode::Edge {
                            "[x] Mode Pilih Tepi (Aktif)"
                        } else {
                            "[ ] Mode Pilih Tepi Manual"
                        };
                        ui.horizontal(|ui| {
                            let single = state.selected_bodies_count == 1;
                            if ui.add_enabled(single, egui::Button::new(RichText::new(edge_btn_label).size(10.5))).clicked() {
                                event = Some(InspectorEvent::ToggleEdgePicking);
                            }
                            ui.label(RichText::new(format!("{} tepi", state.selected_edges_count)).size(10.5).color(TEXT_SECONDARY));
                        });

                        if state.selected_edges_count > 0 {
                            if ui.small_button("Reset Seleksi Tepi").clicked() {
                                event = Some(InspectorEvent::ResetEdgePicking);
                            }
                        }

                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Radius:").size(10.5).color(TEXT_SECONDARY));
                            ui.add_sized(Vec2::new(55.0, 18.0), egui::TextEdit::singleline(&mut state.fillet_input));
                            if ui.button(RichText::new("Fillet").size(10.5)).clicked() {
                                if let Ok(r) = state.fillet_input.trim().parse::<f64>() {
                                    event = Some(InspectorEvent::ApplyFillet { radius: r });
                                }
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Jarak:").size(10.5).color(TEXT_SECONDARY));
                            ui.add_sized(Vec2::new(55.0, 18.0), egui::TextEdit::singleline(&mut state.chamfer_input));
                            if ui.button(RichText::new("Chamfer").size(10.5)).clicked() {
                                if let Ok(d) = state.chamfer_input.trim().parse::<f64>() {
                                    event = Some(InspectorEvent::ApplyChamfer { distance: d });
                                }
                            }
                        });
                    });
                    ui.add_space(3.0);

                    // Shell / Hollow Card
                    card_frame().show(ui, |ui| {
                        ui.label(RichText::new(format!("{} Shell / Hollow", ICON_OPEN_IN_FULL)).strong().color(ACCENT_BLUE));
                        let face_btn_label = if state.picking_mode == InspectorPickMode::Face {
                            "[x] Mode Pilih Wajah (Aktif)"
                        } else {
                            "[ ] Mode Pilih Wajah Manual"
                        };
                        ui.horizontal(|ui| {
                            let single = state.selected_bodies_count == 1;
                            if ui.add_enabled(single, egui::Button::new(RichText::new(face_btn_label).size(10.5))).clicked() {
                                event = Some(InspectorEvent::ToggleFacePicking);
                            }
                            ui.label(RichText::new(format!("{} wajah", state.selected_faces_count)).size(10.5).color(TEXT_SECONDARY));
                        });

                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Tebal:").size(10.5).color(TEXT_SECONDARY));
                            ui.add_sized(Vec2::new(60.0, 18.0), egui::TextEdit::singleline(&mut state.shell_input));
                        });
                        if ui.button(RichText::new("Eksekusi Shell").size(10.5)).clicked() {
                            if let Ok(t) = state.shell_input.trim().parse::<f64>() {
                                event = Some(InspectorEvent::ApplyShell { thickness: t });
                            }
                        }
                    });
                    ui.add_space(3.0);

                    // Hapus Body
                    let del_text = format!("{} Hapus Body Terpilih", ICON_DELETE);
                    if ui.button(RichText::new(del_text).size(11.0).color(Color32::from_rgb(240, 90, 90))).clicked() {
                        event = Some(InspectorEvent::DeleteSelectedBodies);
                    }
                    ui.add_space(3.0);
                }

                // 6. Section View Card
                card_frame().show(ui, |ui| {
                    ui.label(RichText::new(format!("{} Section View", ICON_CONTENT_CUT)).strong().color(ACCENT_ORANGE));
                    if ui.checkbox(&mut state.section_enabled, "Aktifkan Potongan").changed() {
                        event = Some(InspectorEvent::SectionViewChanged);
                    }
                    if state.section_enabled {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Sumbu:").size(10.5));
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
                            ui.label(RichText::new("Offset:").size(10.5));
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

                // 7. Model History (Undo / Redo 3D Model)
                card_frame().show(ui, |ui| {
                    ui.label(RichText::new("Riwayat Model 3D:").size(10.5).color(TEXT_SECONDARY));
                    ui.horizontal(|ui| {
                        let undo_label = format!("{} Undo", ICON_UNDO);
                        if ui.add_enabled(state.can_undo_model, egui::Button::new(RichText::new(undo_label).size(10.5))).clicked() {
                            event = Some(InspectorEvent::UndoModel);
                        }
                        let redo_label = format!("{} Redo", ICON_REDO);
                        if ui.add_enabled(state.can_redo_model, egui::Button::new(RichText::new(redo_label).size(10.5))).clicked() {
                            event = Some(InspectorEvent::RedoModel);
                        }
                    });
                });

                // 8. Overview jika kosong
                if !has_2d_selection && state.selected_bodies_count == 0 {
                    ui.add_space(2.0);
                    card_frame().show(ui, |ui| {
                        ui.label(RichText::new("Dokumen CADRAW").strong().size(11.0).color(TEXT_PRIMARY));
                        ui.label(RichText::new(format!("• 2D Entitas: {} objek", state.total_entities_count)).size(10.0).color(TEXT_SECONDARY));
                        ui.label(RichText::new(format!("• 3D Bodies: {} objek", state.total_bodies_count)).size(10.0).color(TEXT_SECONDARY));
                        ui.separator();
                        ui.label(
                            RichText::new("Pilih objek di kanvas atau pohon item untuk melihat & mengubah dimensinya.")
                                .italics()
                                .size(9.5)
                                .color(TEXT_SECONDARY),
                        );
                    });
                }

                // Status / Error message
                if let Some(msg) = &state.status_message {
                    ui.separator();
                    ui.label(RichText::new(msg).color(Color32::from_rgb(240, 100, 100)).size(10.5));
                }
            });
        });

        event
    }
}
