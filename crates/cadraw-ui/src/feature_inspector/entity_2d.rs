use egui::{RichText, Ui, Vec2};
use egui_material_icons::icons::{ICON_EDIT, ICON_LOCK};

use crate::feature_inspector::types::{
    FeatureInspectorState, InspectorConstraintAction, InspectorEvent, SelectedEntityData,
};
use crate::theme::{card_frame, ACCENT_BLUE, TEXT_SECONDARY};

pub fn show_2d_entity_cards(
    ui: &mut Ui,
    state: &mut FeatureInspectorState,
    event: &mut Option<InspectorEvent>,
) {
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
                    ui.add_sized(
                        Vec2::new(72.0, 18.0),
                        egui::TextEdit::singleline(&mut state.entity_p1_x),
                    );
                    ui.label(RichText::new("Y:").size(10.5));
                    ui.add_sized(
                        Vec2::new(72.0, 18.0),
                        egui::TextEdit::singleline(&mut state.entity_p1_y),
                    );
                });

                ui.label(RichText::new("Titik Akhir (End):").size(10.0).color(TEXT_SECONDARY));
                ui.horizontal(|ui| {
                    ui.label(RichText::new("X:").size(10.5));
                    ui.add_sized(
                        Vec2::new(72.0, 18.0),
                        egui::TextEdit::singleline(&mut state.entity_p2_x),
                    );
                    ui.label(RichText::new("Y:").size(10.5));
                    ui.add_sized(
                        Vec2::new(72.0, 18.0),
                        egui::TextEdit::singleline(&mut state.entity_p2_y),
                    );
                });

                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("Panjang: {:.2} mm", length))
                            .size(10.5)
                            .color(TEXT_SECONDARY),
                    );
                    ui.label(
                        RichText::new(format!("Sudut: {:.1}°", angle_deg))
                            .size(10.5)
                            .color(TEXT_SECONDARY),
                    );
                });

                ui.add_space(2.0);
                if ui.button(RichText::new("Terapkan Koordinat").size(11.0)).clicked() {
                    if let (Ok(x1), Ok(y1), Ok(x2), Ok(y2)) = (
                        state.entity_p1_x.trim().parse::<f64>(),
                        state.entity_p1_y.trim().parse::<f64>(),
                        state.entity_p2_x.trim().parse::<f64>(),
                        state.entity_p2_y.trim().parse::<f64>(),
                    ) {
                        *event = Some(InspectorEvent::UpdateEntityLine {
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
                    if ui
                        .button(RichText::new("— Horiz").size(10.5))
                        .on_hover_text("Bikin garis horizontal")
                        .clicked()
                    {
                        *event = Some(InspectorEvent::ApplyConstraint(
                            InspectorConstraintAction::Horizontal,
                        ));
                    }
                    if ui
                        .button(RichText::new("| Vert").size(10.5))
                        .on_hover_text("Bikin garis vertikal")
                        .clicked()
                    {
                        *event = Some(InspectorEvent::ApplyConstraint(
                            InspectorConstraintAction::Vertical,
                        ));
                    }
                    if ui
                        .button(RichText::new(format!("{} Lock", ICON_LOCK)).size(10.5))
                        .on_hover_text("Kunci posisi")
                        .clicked()
                    {
                        *event =
                            Some(InspectorEvent::ApplyConstraint(InspectorConstraintAction::Fixed));
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
                    ui.add_sized(
                        Vec2::new(72.0, 18.0),
                        egui::TextEdit::singleline(&mut state.entity_p1_x),
                    );
                    ui.label(RichText::new("Y:").size(10.5));
                    ui.add_sized(
                        Vec2::new(72.0, 18.0),
                        egui::TextEdit::singleline(&mut state.entity_p1_y),
                    );
                });

                ui.label(
                    RichText::new("Radius (Jari-jari mm):")
                        .size(10.0)
                        .color(TEXT_SECONDARY),
                );
                ui.horizontal(|ui| {
                    ui.add_sized(
                        Vec2::new(90.0, 18.0),
                        egui::TextEdit::singleline(&mut state.entity_val_1),
                    );
                    ui.label(
                        RichText::new(format!("Ø {:.2} mm", diameter))
                            .size(10.5)
                            .color(TEXT_SECONDARY),
                    );
                });

                ui.add_space(2.0);
                if ui.button(RichText::new("Terapkan Dimensi").size(11.0)).clicked() {
                    if let (Ok(cx), Ok(cy), Ok(r)) = (
                        state.entity_p1_x.trim().parse::<f64>(),
                        state.entity_p1_y.trim().parse::<f64>(),
                        state.entity_val_1.trim().parse::<f64>(),
                    ) {
                        *event = Some(InspectorEvent::UpdateEntityCircle {
                            id_raw,
                            center_x: cx,
                            center_y: cy,
                            radius: r,
                        });
                    }
                }

                ui.separator();
                if ui
                    .button(RichText::new(format!("{} Lock Pusat", ICON_LOCK)).size(10.5))
                    .clicked()
                {
                    *event =
                        Some(InspectorEvent::ApplyConstraint(InspectorConstraintAction::Fixed));
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
                    ui.add_sized(
                        Vec2::new(72.0, 18.0),
                        egui::TextEdit::singleline(&mut state.entity_p1_x),
                    );
                    ui.label(RichText::new("Y:").size(10.5));
                    ui.add_sized(
                        Vec2::new(72.0, 18.0),
                        egui::TextEdit::singleline(&mut state.entity_p1_y),
                    );
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Radius:").size(10.5));
                    ui.add_sized(
                        Vec2::new(80.0, 18.0),
                        egui::TextEdit::singleline(&mut state.entity_val_1),
                    );
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Sudut:").size(10.5));
                    ui.add_sized(
                        Vec2::new(50.0, 18.0),
                        egui::TextEdit::singleline(&mut state.entity_val_2),
                    );
                    ui.label(RichText::new("s/d").size(10.0));
                    ui.add_sized(
                        Vec2::new(50.0, 18.0),
                        egui::TextEdit::singleline(&mut state.entity_p2_x),
                    );
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
                        *event = Some(InspectorEvent::UpdateEntityArc {
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
                    ui.add_sized(
                        Vec2::new(72.0, 18.0),
                        egui::TextEdit::singleline(&mut state.entity_p1_x),
                    );
                    ui.label(RichText::new("Y:").size(10.5));
                    ui.add_sized(
                        Vec2::new(72.0, 18.0),
                        egui::TextEdit::singleline(&mut state.entity_p1_y),
                    );
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Rx:").size(10.5));
                    ui.add_sized(
                        Vec2::new(60.0, 18.0),
                        egui::TextEdit::singleline(&mut state.entity_val_1),
                    );
                    ui.label(RichText::new("Ry:").size(10.5));
                    ui.add_sized(
                        Vec2::new(60.0, 18.0),
                        egui::TextEdit::singleline(&mut state.entity_val_2),
                    );
                });

                ui.add_space(2.0);
                if ui.button(RichText::new("Terapkan Dimensi").size(11.0)).clicked() {
                    if let (Ok(cx), Ok(cy), Ok(rx), Ok(ry)) = (
                        state.entity_p1_x.trim().parse::<f64>(),
                        state.entity_p1_y.trim().parse::<f64>(),
                        state.entity_val_1.trim().parse::<f64>(),
                        state.entity_val_2.trim().parse::<f64>(),
                    ) {
                        *event = Some(InspectorEvent::UpdateEntityEllipse {
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
                ui.label(
                    RichText::new("Terapkan Constraint Bersama:")
                        .size(10.0)
                        .color(TEXT_SECONDARY),
                );

                ui.horizontal(|ui| {
                    if ui
                        .button(RichText::new("// Sejajar").size(10.0))
                        .on_hover_text("Parallel (2 Garis)")
                        .clicked()
                    {
                        *event = Some(InspectorEvent::ApplyConstraint(
                            InspectorConstraintAction::Parallel,
                        ));
                    }
                    if ui
                        .button(RichText::new("⊥ Siku").size(10.0))
                        .on_hover_text("Perpendicular (2 Garis)")
                        .clicked()
                    {
                        *event = Some(InspectorEvent::ApplyConstraint(
                            InspectorConstraintAction::Perpendicular,
                        ));
                    }
                    if ui
                        .button(RichText::new("== Panjang").size(10.0))
                        .on_hover_text("Equal Length")
                        .clicked()
                    {
                        *event = Some(InspectorEvent::ApplyConstraint(
                            InspectorConstraintAction::EqualLength,
                        ));
                    }
                });

                ui.horizontal(|ui| {
                    if ui
                        .button(RichText::new("=R Radius").size(10.0))
                        .on_hover_text("Equal Radius")
                        .clicked()
                    {
                        *event = Some(InspectorEvent::ApplyConstraint(
                            InspectorConstraintAction::EqualRadius,
                        ));
                    }
                    if ui
                        .button(RichText::new("tan Singgung").size(10.0))
                        .on_hover_text("Tangent")
                        .clicked()
                    {
                        *event = Some(InspectorEvent::ApplyConstraint(
                            InspectorConstraintAction::Tangent,
                        ));
                    }
                    if ui
                        .button(RichText::new(">< Berimpit").size(10.0))
                        .on_hover_text("Coincident")
                        .clicked()
                    {
                        *event = Some(InspectorEvent::ApplyConstraint(
                            InspectorConstraintAction::Coincident,
                        ));
                    }
                });
            });
            ui.add_space(3.0);
        }

        SelectedEntityData::None => {}
    }
}
