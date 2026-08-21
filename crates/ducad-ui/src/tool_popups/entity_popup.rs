//! 2D Entity & Selection Parameters Tool Popup.

use egui::{Color32, Context, Rect, RichText, Vec2};
use egui_material_icons::icons::{ICON_DELETE, ICON_EDIT, ICON_LOCK};

use super::{render_bottom_right_popup, ToolPopupEvent};
use crate::feature_inspector::{
    InspectorConstraintAction, InspectorRectAnchor, SelectedBodyData, SelectedEntityData,
};
use crate::theme::{ACCENT_BLUE, ACCENT_GREEN, TEXT_SECONDARY};

#[derive(Debug, Clone)]
pub struct Entity2dPopupState {
    pub selected_entity: SelectedEntityData,
    pub selected_body: Option<SelectedBodyData>,
    pub p1_x: String,
    pub p1_y: String,
    pub p2_x: String,
    pub p2_y: String,
    pub val_1: String,
    pub val_2: String,
    pub val_3: String,
    pub rect_p: String,
    pub rect_l: String,
    pub rect_anchor: InspectorRectAnchor,
}

impl Default for Entity2dPopupState {
    fn default() -> Self {
        Self {
            selected_entity: SelectedEntityData::None,
            selected_body: None,
            p1_x: String::new(),
            p1_y: String::new(),
            p2_x: String::new(),
            p2_y: String::new(),
            val_1: String::new(),
            val_2: String::new(),
            val_3: String::new(),
            rect_p: String::new(),
            rect_l: String::new(),
            rect_anchor: InspectorRectAnchor::Center,
        }
    }
}

pub struct Entity2dPopup;

impl Entity2dPopup {
    pub fn show(
        ctx: &Context,
        state: &mut Entity2dPopupState,
        screen_rect: Rect,
    ) -> Option<ToolPopupEvent> {
        let (title, icon) = match (&state.selected_entity, &state.selected_body) {
            (SelectedEntityData::Rectangle { .. }, _) => ("Persegi Panjang", ICON_EDIT.codepoint),
            (SelectedEntityData::Line { .. }, _) => ("Garis (Line)", ICON_EDIT.codepoint),
            (SelectedEntityData::Circle { .. }, _) => ("Lingkaran", ICON_EDIT.codepoint),
            (SelectedEntityData::Arc { .. }, _) => ("Busur (Arc)", ICON_EDIT.codepoint),
            (SelectedEntityData::Ellipse { .. }, _) => ("Elips", ICON_EDIT.codepoint),
            (SelectedEntityData::MultipleEntities { .. }, _) => ("Seleksi Jamak", ICON_EDIT.codepoint),
            (_, Some(_)) => ("Properti Body 3D", ICON_EDIT.codepoint),
            _ => return None,
        };

        let (event_opt, close) = render_bottom_right_popup(
            ctx,
            "ducad-entity-popup",
            title,
            icon,
            ACCENT_BLUE,
            screen_rect,
            |ui| {
                let mut ev = None;

                match &state.selected_entity {
                    SelectedEntityData::Rectangle {
                        entity_ids,
                        length_p,
                        length_l,
                    } => {
                        let entity_ids = *entity_ids;
                        let lp = *length_p;
                        let ll = *length_l;

                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Panjang (P):").size(10.5));
                            ui.add_sized(
                                Vec2::new(75.0, 18.0),
                                egui::TextEdit::singleline(&mut state.rect_p),
                            );
                        });

                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Lebar (L):").size(10.5));
                            ui.add_sized(
                                Vec2::new(75.0, 18.0),
                                egui::TextEdit::singleline(&mut state.rect_l),
                            );
                        });

                        ui.add_space(2.0);
                        ui.label(RichText::new("Titik Tetap (Anchor):").size(9.5).color(TEXT_SECONDARY));
                        ui.horizontal_wrapped(|ui| {
                            let anchors = [
                                (InspectorRectAnchor::Center, "Tengah"),
                                (InspectorRectAnchor::Corner0, "A (Kiri Bwh)"),
                                (InspectorRectAnchor::Corner1, "B (Kanan Bwh)"),
                                (InspectorRectAnchor::Corner2, "C (Kanan Atas)"),
                                (InspectorRectAnchor::Corner3, "D (Kiri Atas)"),
                            ];
                            for (anc, lbl) in anchors {
                                if ui.selectable_value(&mut state.rect_anchor, anc, RichText::new(lbl).size(10.0)).clicked() {
                                    state.rect_anchor = anc;
                                }
                            }
                        });

                        ui.add_space(3.0);
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Terapkan Dimensi").size(11.0).color(Color32::WHITE),
                                )
                                .fill(ACCENT_BLUE),
                            )
                            .clicked()
                        {
                            let p = state.rect_p.trim().parse::<f64>().unwrap_or(lp);
                            let l = state.rect_l.trim().parse::<f64>().unwrap_or(ll);
                            ev = Some(ToolPopupEvent::UpdateEntityRectangle {
                                entity_ids,
                                length_p: p,
                                length_l: l,
                                anchor: state.rect_anchor,
                            });
                        }
                    }
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

                        ui.label(RichText::new("Titik Awal (Start):").size(10.0).color(TEXT_SECONDARY));
                        ui.horizontal(|ui| {
                            ui.label("X:");
                            ui.add_sized(Vec2::new(65.0, 18.0), egui::TextEdit::singleline(&mut state.p1_x));
                            ui.label("Y:");
                            ui.add_sized(Vec2::new(65.0, 18.0), egui::TextEdit::singleline(&mut state.p1_y));
                        });

                        ui.label(RichText::new("Titik Akhir (End):").size(10.0).color(TEXT_SECONDARY));
                        ui.horizontal(|ui| {
                            ui.label("X:");
                            ui.add_sized(Vec2::new(65.0, 18.0), egui::TextEdit::singleline(&mut state.p2_x));
                            ui.label("Y:");
                            ui.add_sized(Vec2::new(65.0, 18.0), egui::TextEdit::singleline(&mut state.p2_y));
                        });

                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("P: {:.2} mm", length)).size(10.0).color(TEXT_SECONDARY));
                            ui.label(RichText::new(format!("∠: {:.1}°", angle_deg)).size(10.0).color(TEXT_SECONDARY));
                        });

                        ui.add_space(2.0);
                        if ui.button(RichText::new("Terapkan Koordinat").size(11.0)).clicked() {
                            if let (Ok(x1), Ok(y1), Ok(x2), Ok(y2)) = (
                                state.p1_x.trim().parse::<f64>(),
                                state.p1_y.trim().parse::<f64>(),
                                state.p2_x.trim().parse::<f64>(),
                                state.p2_y.trim().parse::<f64>(),
                            ) {
                                ev = Some(ToolPopupEvent::UpdateEntityLine {
                                    id_raw,
                                    start_x: x1,
                                    start_y: y1,
                                    end_x: x2,
                                    end_y: y2,
                                });
                            }
                        }

                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("— Horiz").size(10.0)).clicked() {
                                ev = Some(ToolPopupEvent::ApplyConstraint(InspectorConstraintAction::Horizontal));
                            }
                            if ui.button(RichText::new("| Vert").size(10.0)).clicked() {
                                ev = Some(ToolPopupEvent::ApplyConstraint(InspectorConstraintAction::Vertical));
                            }
                            if ui.button(RichText::new(format!("{} Lock", ICON_LOCK.codepoint)).size(10.0)).clicked() {
                                ev = Some(ToolPopupEvent::ApplyConstraint(InspectorConstraintAction::Fixed));
                            }
                        });
                    }
                    SelectedEntityData::Circle {
                        id_raw,
                        center_x: _,
                        center_y: _,
                        radius,
                        diameter,
                    } => {
                        let id_raw = *id_raw;
                        let r = *radius;

                        ui.horizontal(|ui| {
                            ui.label("Pusat X:");
                            ui.add_sized(Vec2::new(60.0, 18.0), egui::TextEdit::singleline(&mut state.p1_x));
                            ui.label("Y:");
                            ui.add_sized(Vec2::new(60.0, 18.0), egui::TextEdit::singleline(&mut state.p1_y));
                        });

                        ui.horizontal(|ui| {
                            ui.label("Radius (R):");
                            ui.add_sized(Vec2::new(75.0, 18.0), egui::TextEdit::singleline(&mut state.val_1));
                        });

                        ui.label(RichText::new(format!("Diameter (Ø): {:.2} mm", diameter)).size(10.0).color(TEXT_SECONDARY));

                        if ui.button(RichText::new("Terapkan Lingkaran").size(11.0)).clicked() {
                            let cx = state.p1_x.trim().parse::<f64>().unwrap_or(0.0);
                            let cy = state.p1_y.trim().parse::<f64>().unwrap_or(0.0);
                            let rad = state.val_1.trim().parse::<f64>().unwrap_or(r);
                            ev = Some(ToolPopupEvent::UpdateEntityCircle {
                                id_raw,
                                center_x: cx,
                                center_y: cy,
                                radius: rad,
                            });
                        }
                    }
                    SelectedEntityData::Arc {
                        id_raw,
                        center_x: _,
                        center_y: _,
                        radius,
                        start_angle_deg,
                        end_angle_deg,
                    } => {
                        let id_raw = *id_raw;
                        let r = *radius;
                        let sa = *start_angle_deg;
                        let ea = *end_angle_deg;

                        ui.label(RichText::new(format!("Radius: {:.2} mm", r)).size(10.5).color(TEXT_SECONDARY));
                        ui.horizontal(|ui| {
                            ui.label("Pusat X/Y:");
                            ui.add_sized(Vec2::new(60.0, 18.0), egui::TextEdit::singleline(&mut state.p1_x));
                            ui.add_sized(Vec2::new(60.0, 18.0), egui::TextEdit::singleline(&mut state.p1_y));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Radius:");
                            ui.add_sized(Vec2::new(65.0, 18.0), egui::TextEdit::singleline(&mut state.val_1));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Sudut Awal/Akhir:");
                            ui.add_sized(Vec2::new(50.0, 18.0), egui::TextEdit::singleline(&mut state.val_2));
                            ui.add_sized(Vec2::new(50.0, 18.0), egui::TextEdit::singleline(&mut state.val_3));
                        });
                        if ui.button(RichText::new("Terapkan Busur").size(11.0)).clicked() {
                            let cx = state.p1_x.trim().parse::<f64>().unwrap_or(0.0);
                            let cy = state.p1_y.trim().parse::<f64>().unwrap_or(0.0);
                            let rad = state.val_1.trim().parse::<f64>().unwrap_or(r);
                            let s_deg = state.val_2.trim().parse::<f64>().unwrap_or(sa);
                            let e_deg = state.val_3.trim().parse::<f64>().unwrap_or(ea);
                            ev = Some(ToolPopupEvent::UpdateEntityArc {
                                id_raw,
                                center_x: cx,
                                center_y: cy,
                                radius: rad,
                                start_angle_deg: s_deg,
                                end_angle_deg: e_deg,
                            });
                        }
                    }
                    SelectedEntityData::Ellipse {
                        id_raw,
                        center_x: _,
                        center_y: _,
                        radius_x,
                        radius_y,
                    } => {
                        let id_raw = *id_raw;
                        let rx = *radius_x;
                        let ry = *radius_y;

                        ui.horizontal(|ui| {
                            ui.label("Pusat X/Y:");
                            ui.add_sized(Vec2::new(60.0, 18.0), egui::TextEdit::singleline(&mut state.p1_x));
                            ui.add_sized(Vec2::new(60.0, 18.0), egui::TextEdit::singleline(&mut state.p1_y));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Radius Rx/Ry:");
                            ui.add_sized(Vec2::new(60.0, 18.0), egui::TextEdit::singleline(&mut state.val_1));
                            ui.add_sized(Vec2::new(60.0, 18.0), egui::TextEdit::singleline(&mut state.val_2));
                        });
                        if ui.button(RichText::new("Terapkan Elips").size(11.0)).clicked() {
                            let cx = state.p1_x.trim().parse::<f64>().unwrap_or(0.0);
                            let cy = state.p1_y.trim().parse::<f64>().unwrap_or(0.0);
                            let rx_val = state.val_1.trim().parse::<f64>().unwrap_or(rx);
                            let ry_val = state.val_2.trim().parse::<f64>().unwrap_or(ry);
                            ev = Some(ToolPopupEvent::UpdateEntityEllipse {
                                id_raw,
                                center_x: cx,
                                center_y: cy,
                                radius_x: rx_val,
                                radius_y: ry_val,
                            });
                        }
                    }
                    SelectedEntityData::MultipleEntities { count } => {
                        ui.label(RichText::new(format!("{} entitas terpilih", count)).size(11.0));
                        ui.label(RichText::new("Constraint Bersama:").size(10.0).color(TEXT_SECONDARY));
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("— Horiz").clicked() {
                                ev = Some(ToolPopupEvent::ApplyConstraint(InspectorConstraintAction::Horizontal));
                            }
                            if ui.button("| Vert").clicked() {
                                ev = Some(ToolPopupEvent::ApplyConstraint(InspectorConstraintAction::Vertical));
                            }
                            if ui.button("= Eq Length").clicked() {
                                ev = Some(ToolPopupEvent::ApplyConstraint(InspectorConstraintAction::EqualLength));
                            }
                            if ui.button("// Parallel").clicked() {
                                ev = Some(ToolPopupEvent::ApplyConstraint(InspectorConstraintAction::Parallel));
                            }
                            if ui.button("⊥ Perpend").clicked() {
                                ev = Some(ToolPopupEvent::ApplyConstraint(InspectorConstraintAction::Perpendicular));
                            }
                        });
                    }
                    _ => {}
                }

                // Jika ada 3D body dipilih
                if let Some(body) = &state.selected_body {
                    ui.label(
                        RichText::new(format!("Body: {}", body.name))
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
                }

                // Tombol Hapus Terpilih
                ui.separator();
                let del_lbl = if state.selected_body.is_some() {
                    format!("{} Hapus Body", ICON_DELETE.codepoint)
                } else {
                    format!("{} Hapus Entitas", ICON_DELETE.codepoint)
                };

                if ui
                    .button(RichText::new(del_lbl).size(10.5).color(Color32::from_rgb(240, 90, 90)))
                    .clicked()
                {
                    if state.selected_body.is_some() {
                        ev = Some(ToolPopupEvent::DeleteSelectedBodies);
                    } else {
                        ev = Some(ToolPopupEvent::DeleteSelectedEntities);
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
