use ducad_i18n::t;
use egui::{RichText, Ui, Vec2};
use egui_material_icons::icons::{ICON_EDIT, ICON_LOCK};

use crate::feature_inspector::types::{
    FeatureInspectorState, InspectorConstraintAction, InspectorEvent, InspectorRectAnchor,
    SelectedEntityData,
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
                    RichText::new(format!("{} {}", ICON_EDIT.codepoint, t!("tool-line-name")))
                        .strong()
                        .size(11.5)
                        .color(ACCENT_BLUE),
                );

                ui.add_space(2.0);
                ui.label(RichText::new(t!("inspector-start-point")).size(10.0).color(TEXT_SECONDARY));
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

                ui.label(RichText::new(t!("inspector-end-point")).size(10.0).color(TEXT_SECONDARY));
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
                        RichText::new(format!("{}: {:.2} mm", t!("param-length"), length))
                            .size(10.5)
                            .color(TEXT_SECONDARY),
                    );
                    ui.label(
                        RichText::new(format!("{}: {:.1}°", t!("param-angle"), angle_deg))
                            .size(10.5)
                            .color(TEXT_SECONDARY),
                    );
                });

                ui.add_space(2.0);
                if ui.button(RichText::new(t!("inspector-apply-coords")).size(11.0)).clicked() {
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
                ui.label(RichText::new(t!("inspector-quick-constraints")).size(10.0).color(TEXT_SECONDARY));
                ui.horizontal(|ui| {
                    if ui
                        .button(RichText::new(t!("inspector-horiz")).size(10.5))
                        .clicked()
                    {
                        *event = Some(InspectorEvent::ApplyConstraint(
                            InspectorConstraintAction::Horizontal,
                        ));
                    }
                    if ui
                        .button(RichText::new(t!("inspector-vert")).size(10.5))
                        .clicked()
                    {
                        *event = Some(InspectorEvent::ApplyConstraint(
                            InspectorConstraintAction::Vertical,
                        ));
                    }
                    if ui
                        .button(RichText::new(format!("{} {}", ICON_LOCK.codepoint, t!("inspector-fix"))).size(10.5))
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
            diameter: _,
        } => {
            let id_raw = *id_raw;
            card_frame().show(ui, |ui| {
                ui.label(
                    RichText::new(format!("{} {}", ICON_EDIT.codepoint, t!("tool-circle-name")))
                        .strong()
                        .size(11.5)
                        .color(ACCENT_BLUE),
                );

                ui.add_space(2.0);
                ui.label(RichText::new(t!("inspector-center-point")).size(10.0).color(TEXT_SECONDARY));
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

                ui.label(RichText::new(t!("inspector-radius-diameter")).size(10.0).color(TEXT_SECONDARY));
                ui.horizontal(|ui| {
                    ui.label(RichText::new("R:").size(10.5));
                    let r_resp = ui.add_sized(
                        Vec2::new(70.0, 18.0),
                        egui::TextEdit::singleline(&mut state.entity_val_1),
                    );
                    ui.label(RichText::new("Ø:").size(10.5));
                    let d_resp = ui.add_sized(
                        Vec2::new(70.0, 18.0),
                        egui::TextEdit::singleline(&mut state.entity_val_3),
                    );

                    if r_resp.changed() {
                        if let Ok(r) = state.entity_val_1.trim().parse::<f64>() {
                            state.entity_val_3 = format!("{:.2}", r * 2.0);
                        }
                    } else if d_resp.changed() {
                        if let Ok(d) = state.entity_val_3.trim().parse::<f64>() {
                            state.entity_val_1 = format!("{:.2}", d * 0.5);
                        }
                    }
                });

                ui.add_space(2.0);
                if ui.button(RichText::new(t!("inspector-apply-dimensions")).size(11.0)).clicked() {
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
                    .button(RichText::new(format!("{} {}", ICON_LOCK.codepoint, t!("inspector-fix"))).size(10.5))
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
                    RichText::new(format!("{} {}", ICON_EDIT.codepoint, t!("tool-arc-name")))
                        .strong()
                        .size(11.5)
                        .color(ACCENT_BLUE),
                );

                ui.add_space(2.0);
                ui.label(RichText::new(t!("inspector-center-point")).size(10.0).color(TEXT_SECONDARY));
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
                    ui.label(RichText::new(format!("{}:", t!("param-radius"))).size(10.5));
                    ui.add_sized(
                        Vec2::new(80.0, 18.0),
                        egui::TextEdit::singleline(&mut state.entity_val_1),
                    );
                });

                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{}:", t!("param-angle"))).size(10.5));
                    ui.add_sized(
                        Vec2::new(50.0, 18.0),
                        egui::TextEdit::singleline(&mut state.entity_val_2),
                    );
                    ui.label(RichText::new("..").size(10.0));
                    ui.add_sized(
                        Vec2::new(50.0, 18.0),
                        egui::TextEdit::singleline(&mut state.entity_p2_x),
                    );
                });

                ui.add_space(2.0);
                if ui.button(RichText::new(t!("inspector-apply-dimensions")).size(11.0)).clicked() {
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
                    RichText::new(format!("{} {}", ICON_EDIT.codepoint, t!("tool-ellipse-name")))
                        .strong()
                        .size(11.5)
                        .color(ACCENT_BLUE),
                );

                ui.add_space(2.0);
                ui.label(RichText::new(t!("inspector-center-point")).size(10.0).color(TEXT_SECONDARY));
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
                if ui.button(RichText::new(t!("inspector-apply-dimensions")).size(11.0)).clicked() {
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

        SelectedEntityData::Rectangle {
            entity_ids,
            length_p: _,
            length_l: _,
        } => {
            let entity_ids = *entity_ids;
            card_frame().show(ui, |ui| {
                ui.label(
                    RichText::new(format!("{} {}", ICON_EDIT.codepoint, t!("tool-rect-name")))
                        .strong()
                        .size(11.5)
                        .color(ACCENT_BLUE),
                );

                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t!("inspector-length-p")).size(10.5).color(TEXT_SECONDARY));
                    ui.add_sized(
                        Vec2::new(70.0, 18.0),
                        egui::TextEdit::singleline(&mut state.rect_length_p_input),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t!("inspector-width-w")).size(10.5).color(TEXT_SECONDARY));
                    ui.add_sized(
                        Vec2::new(70.0, 18.0),
                        egui::TextEdit::singleline(&mut state.rect_length_l_input),
                    );
                });

                ui.add_space(3.0);
                ui.label(
                    RichText::new(t!("inspector-anchor-help"))
                        .size(9.5)
                        .color(TEXT_SECONDARY),
                );
                ui.horizontal(|ui| {
                    let options = [
                        (InspectorRectAnchor::Center, "Center"),
                        (InspectorRectAnchor::Corner0, "Corner A"),
                        (InspectorRectAnchor::Corner1, "Corner B"),
                    ];
                    for (anchor, label) in options {
                        if ui
                            .selectable_label(state.rect_anchor == anchor, RichText::new(label).size(10.0))
                            .clicked()
                        {
                            state.rect_anchor = anchor;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    let options = [
                        (InspectorRectAnchor::Corner2, "Corner C"),
                        (InspectorRectAnchor::Corner3, "Corner D"),
                    ];
                    for (anchor, label) in options {
                        if ui
                            .selectable_label(state.rect_anchor == anchor, RichText::new(label).size(10.0))
                            .clicked()
                        {
                            state.rect_anchor = anchor;
                        }
                    }
                });

                ui.add_space(2.0);
                if ui.button(RichText::new(t!("inspector-apply-dimensions")).size(11.0)).clicked() {
                    if let (Ok(p), Ok(l)) = (
                        state.rect_length_p_input.trim().parse::<f64>(),
                        state.rect_length_l_input.trim().parse::<f64>(),
                    ) {
                        *event = Some(InspectorEvent::UpdateEntityRectangle {
                            entity_ids,
                            length_p: p,
                            length_l: l,
                            anchor: state.rect_anchor,
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
                    RichText::new(format!("{} {}", ICON_EDIT.codepoint, t!("inspector-multi-selection", count = count)))
                        .strong()
                        .size(11.5)
                        .color(ACCENT_BLUE),
                );

                ui.add_space(2.0);
                ui.label(
                    RichText::new(t!("inspector-apply-joint-constraints"))
                        .size(10.0)
                        .color(TEXT_SECONDARY),
                );

                ui.horizontal(|ui| {
                    if ui
                        .button(RichText::new(format!("// {}", t!("inspector-parallel"))).size(10.0))
                        .clicked()
                    {
                        *event = Some(InspectorEvent::ApplyConstraint(
                            InspectorConstraintAction::Parallel,
                        ));
                    }
                    if ui
                        .button(RichText::new(format!("⊥ {}", t!("inspector-perpendicular"))).size(10.0))
                        .clicked()
                    {
                        *event = Some(InspectorEvent::ApplyConstraint(
                            InspectorConstraintAction::Perpendicular,
                        ));
                    }
                    if ui
                        .button(RichText::new(format!("== {}", t!("inspector-equal"))).size(10.0))
                        .clicked()
                    {
                        *event = Some(InspectorEvent::ApplyConstraint(
                            InspectorConstraintAction::EqualLength,
                        ));
                    }
                });

                ui.horizontal(|ui| {
                    if ui
                        .button(RichText::new(format!("=R {}", t!("constraint-equal-radius"))).size(10.0))
                        .clicked()
                    {
                        *event = Some(InspectorEvent::ApplyConstraint(
                            InspectorConstraintAction::EqualRadius,
                        ));
                    }
                    if ui
                        .button(RichText::new(format!("tan {}", t!("inspector-tangent"))).size(10.0))
                        .clicked()
                    {
                        *event = Some(InspectorEvent::ApplyConstraint(
                            InspectorConstraintAction::Tangent,
                        ));
                    }
                    if ui
                        .button(RichText::new(format!(">< {}", t!("inspector-coincident"))).size(10.0))
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
