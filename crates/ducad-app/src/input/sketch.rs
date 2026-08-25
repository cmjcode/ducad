use std::collections::HashSet;
use ducad_core::{BodyId, Command};
use ducad_sketch::constraint::Constraint;
use ducad_sketch::{
    arc_from_three_points, compute_chamfer_2d, compute_fillet_2d, find_corner_lines_at_point,
    find_region_at_point, find_region_containing_entity, find_snap, find_snap_with_extra,
    line_intersection_params_in_sketch, mirror_entity, offset_entity, project_t, trim_segments,
    Chamfer2DResult, ClosedRegion, DeleteEntities, Entity, EntityId, Fillet2DResult,
    InsertEntities, ReplaceEntities, Sketch, ToggleConstruction, TranslateEntities,
};
use eframe::egui;
use glam::{DVec2, Vec3};

use crate::app::DuCADApp;
use crate::model::{AddSolidCommand, BodyGeometry, ReplaceGeometryCommand};
use crate::types::{required_points, Measurement, PickMode, RoundKind, RoundStyle, ToolKind};
use crate::viewport::{hit_test_cycled, pixel_tolerance_to_world, screen_to_plane_point};

/// Untuk tool Trim: segmen (awal,akhir) yang akan terhapus jika `hover` diklik sekarang pada Line `id`.
pub fn trim_removal_preview(
    sketch: &Sketch,
    id: EntityId,
    hover: DVec2,
) -> Option<(DVec2, DVec2)> {
    let Entity::Line { start, end, .. } = sketch.entities.get(id)?.clone() else {
        return None;
    };
    let click_t = project_t(start, end, hover).clamp(0.0, 1.0);
    let mut ts: Vec<f64> = line_intersection_params_in_sketch(sketch, (start, end), id)
        .into_iter()
        .filter(|t| *t > 1e-6 && *t < 1.0 - 1e-6)
        .collect();
    ts.push(0.0);
    ts.push(1.0);
    ts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ts.windows(2)
        .find(|w| click_t >= w[0] && click_t <= w[1])
        .map(|w| (start + (end - start) * w[0], start + (end - start) * w[1]))
}

/// Untuk tool Fillet2D: hasil kalkulasi fillet jika hover di dekat sudut atau garis.
pub fn fillet_2d_preview(
    sketch: &Sketch,
    hover: DVec2,
    radius: f64,
    tol: f64,
    first_selected: Option<EntityId>,
) -> Option<Fillet2DResult> {
    if let Some((id1, id2, _corner)) = find_corner_lines_at_point(sketch, hover, tol) {
        let Entity::Line { start: s1, end: e1, .. } = sketch.entities.get(id1)?.clone() else {
            return None;
        };
        let Entity::Line { start: s2, end: e2, .. } = sketch.entities.get(id2)?.clone() else {
            return None;
        };
        compute_fillet_2d((s1, e1), (s2, e2), radius)
    } else if let Some(id1) = first_selected {
        let Entity::Line { start: s1, end: e1, .. } = sketch.entities.get(id1)?.clone() else {
            return None;
        };
        let hovered_id = sketch.hit_test(hover, tol)?;
        if hovered_id != id1 {
            let Entity::Line { start: s2, end: e2, .. } = sketch.entities.get(hovered_id)?.clone() else {
                return None;
            };
            compute_fillet_2d((s1, e1), (s2, e2), radius)
        } else {
            None
        }
    } else {
        None
    }
}

/// Untuk tool Chamfer2D: hasil kalkulasi chamfer jika hover di dekat sudut atau garis.
pub fn chamfer_2d_preview(
    sketch: &Sketch,
    hover: DVec2,
    dist: f64,
    tol: f64,
    first_selected: Option<EntityId>,
) -> Option<Chamfer2DResult> {
    if let Some((id1, id2, _corner)) = find_corner_lines_at_point(sketch, hover, tol) {
        let Entity::Line { start: s1, end: e1, .. } = sketch.entities.get(id1)?.clone() else {
            return None;
        };
        let Entity::Line { start: s2, end: e2, .. } = sketch.entities.get(id2)?.clone() else {
            return None;
        };
        compute_chamfer_2d((s1, e1), (s2, e2), dist, dist)
    } else if let Some(id1) = first_selected {
        let Entity::Line { start: s1, end: e1, .. } = sketch.entities.get(id1)?.clone() else {
            return None;
        };
        let hovered_id = sketch.hit_test(hover, tol)?;
        if hovered_id != id1 {
            let Entity::Line { start: s2, end: e2, .. } = sketch.entities.get(hovered_id)?.clone() else {
                return None;
            };
            compute_chamfer_2d((s1, e1), (s2, e2), dist, dist)
        } else {
            None
        }
    } else {
        None
    }
}

impl DuCADApp {
    pub const LINE_CHAIN_DEGENERATE_EPS: f64 = 1e-6;

    pub fn find_current_snap(
        &self,
        raw: DVec2,
        tol: f64,
        grid_step: f64,
        exclude: Option<EntityId>,
    ) -> Option<ducad_sketch::SnapHit> {
        find_snap_with_extra(
            self.sketch(),
            raw,
            tol,
            grid_step,
            exclude,
            &self.pending_points,
        )
    }

    pub fn set_tool(&mut self, tool: ToolKind) {
        if self.tool == ToolKind::Loft && tool != ToolKind::Loft {
            if let Some(staged_id) = self.loft_staged_body_id.take() {
                self.model.geometry.remove(staged_id);
                self.model.doc.bodies.remove(staged_id);
            }
        }
        if self.tool == ToolKind::Sweep && tool != ToolKind::Sweep {
            self.pending_sweep_profile = None;
            self.pending_sweep_path = None;
            self.sweep_path_plane_idx = None;
            self.hovered_plane_idx = None;
        }
        self.tool = tool;
        self.pending_points.clear();
        self.pending_point_refs.clear();
        self.offset_source = None;
        self.line_chain_start = None;
        self.line_chain_segments = 0;
        self.last_snap = None;
        self.dynamic_input.clear();
        self.dynamic_focus_pending = false;
        self.sketch_move_armed = false;
        self.sketch_move_target = None;
        self.body_move_armed = false;
        self.body_move_target = None;
        self.loft_alignment_dismissed = false;
        self.loft_staged_body_id = None;
        self.fillet_chamfer_first_line = None;
        self.active_sketch_corner = None;
        self.active_sketch_fillet_arc = None;
        self.sketch_corner_gizmo_active = false;
        self.sketch_corner_dimension_editing = false;

        if tool == ToolKind::Pattern {
            self.pattern_dimension_editing_x = false;
            self.pattern_dimension_editing_y = false;
            self.pattern_dimension_editing_z = false;
            self.pattern_dimension_editing_angle = false;
            self.pattern_dimension_editing_radius = false;
            self.pattern_custom_pivot_2d = None;
            self.pattern_custom_pivot_3d = None;

            if !self.selected.is_empty() {
                let entities: Vec<Entity> = self
                    .selected
                    .iter()
                    .filter_map(|id| self.sketch().entities.get(*id).cloned())
                    .collect();
                if let Some(c) = ducad_sketch::compute_entities_centroid(&entities) {
                    let d = c.length();
                    self.pattern_circ_radius = if d >= 1.0 { d } else { 30.0 };
                }
            } else if !self.selected_bodies.is_empty() {
                for &bid in &self.selected_bodies {
                    if let Some(geo) = self.model.geometry.get(bid) {
                        let c = geo.mesh.center();
                        let d = ((c[0] * c[0] + c[1] * c[1] + c[2] * c[2]) as f64).sqrt();
                        self.pattern_circ_radius = if d >= 1.0 { d } else { 30.0 };
                        break;
                    }
                }
            }
        }
    }

    /// Eksekusi commit Fillet 2D atau Chamfer 2D saat drag gizmo sudut 2D selesai atau angka dimasukkan.
    pub fn commit_sketch_corner_fillet_or_chamfer(&mut self) {
        if let Some((id1, id2, _corner)) = self.active_sketch_corner {
            let r = self.sketch_corner_gizmo_radius;

            // Jika ini revisi fillet arc yang sudah ada
            if let Some(arc_id) = self.active_sketch_fillet_arc {
                let arc_opt = self.sketch().entities.get(arc_id).cloned();
                let (e1_opt, e2_opt) = (
                    self.sketch().entities.get(id1).cloned(),
                    self.sketch().entities.get(id2).cloned(),
                );
                if let (
                    Some(Entity::Arc { center, start_angle, end_angle, .. }),
                    Some(Entity::Line { start: s1, end: e1, .. }),
                    Some(Entity::Line { start: s2, end: e2, .. }),
                ) = (arc_opt, e1_opt, e2_opt) {
                    let ap1 = center + glam::DVec2::new(start_angle.cos(), start_angle.sin()) * 1.0;
                    let _ap2 = center + glam::DVec2::new(end_angle.cos(), end_angle.sin()) * 1.0;

                    let (n1, f1) = if (s1 - ap1).length() < (e1 - ap1).length() { (s1, e1) } else { (e1, s1) };
                    let (n2, f2) = if (s2 - ap1).length() < (e2 - ap1).length() { (s2, e2) } else { (e2, s2) };

                    let d1 = n1 - f1;
                    let d2 = n2 - f2;
                    let det = d1.x * d2.y - d1.y * d2.x;
                    if det.abs() > 1e-6 {
                        let t = ((f2 - f1).x * d2.y - (f2 - f1).y * d2.x) / det;
                        let apex = f1 + d1 * t;

                        if r > 0.1 {
                            if let Some(res) = compute_fillet_2d((f1, apex), (f2, apex), r) {
                                self.execute_sketch_command(Box::new(ReplaceEntities::new(
                                    "Revisi Fillet 2D",
                                    vec![id1, id2, arc_id],
                                    vec![res.trimmed_line1, res.trimmed_line2, res.arc],
                                )));
                                self.model_status = Some(format!(
                                    "✓ Fillet 2D direvisi — R {}", self.unit.format(r)
                                ));
                            }
                        } else {
                            // Radius <= 0.1 -> Kembalikan ke sudut tajam
                            self.execute_sketch_command(Box::new(ReplaceEntities::new(
                                "Hapus Fillet 2D",
                                vec![id1, id2, arc_id],
                                vec![
                                    Entity::line(f1, apex),
                                    Entity::line(f2, apex),
                                ],
                            )));
                            self.model_status = Some("✓ Fillet 2D dihapus — dikembalikan ke sudut tajam".to_string());
                        }
                    }
                }
                self.active_sketch_corner = None;
                self.active_sketch_fillet_arc = None;
                self.sketch_corner_gizmo_active = false;
                self.sketch_corner_dimension_editing = false;
                return;
            }

            if r.abs() < 0.1 {
                self.active_sketch_corner = None;
                self.active_sketch_fillet_arc = None;
                self.sketch_corner_gizmo_active = false;
                self.sketch_corner_dimension_editing = false;
                return;
            }
            let (e1_opt, e2_opt) = (
                self.sketch().entities.get(id1).cloned(),
                self.sketch().entities.get(id2).cloned(),
            );
            match (e1_opt, e2_opt) {
                (Some(Entity::Line { start: s1, end: e1, .. }), Some(Entity::Line { start: s2, end: e2, .. })) => {
                    if r > 0.0 {
                        if let Some(res) = compute_fillet_2d((s1, e1), (s2, e2), r) {
                            self.execute_sketch_command(Box::new(ReplaceEntities::new(
                                "Fillet 2D",
                                vec![id1, id2],
                                vec![res.trimmed_line1, res.trimmed_line2, res.arc],
                            )));
                            self.model_status = Some(format!(
                                "✓ Fillet 2D diterapkan — R {}", self.unit.format(r)
                            ));
                        }
                    } else {
                        let d = -r;
                        if let Some(res) = compute_chamfer_2d((s1, e1), (s2, e2), d, d) {
                            self.execute_sketch_command(Box::new(ReplaceEntities::new(
                                "Chamfer 2D",
                                vec![id1, id2],
                                vec![res.trimmed_line1, res.trimmed_line2, res.bevel_line],
                            )));
                            self.model_status = Some(format!(
                                "✓ Chamfer 2D diterapkan — C {}", self.unit.format(d)
                            ));
                        }
                    }
                }
                _ => {
                    self.model_status = Some(
                        "⚠ Fillet/Chamfer 2D hanya bisa diterapkan pada pertemuan dua garis lurus".to_string(),
                    );
                }
            }
        }
        self.active_sketch_corner = None;
        self.active_sketch_fillet_arc = None;
        self.sketch_corner_gizmo_active = false;
        self.sketch_corner_dimension_editing = false;
    }

    pub fn snapped_or(&self, raw: DVec2) -> DVec2 {
        self.last_snap.map(|s| s.point).unwrap_or(raw)
    }

    pub fn symmetric_axis(&self) -> Option<EntityId> {
        self.selected
            .iter()
            .copied()
            .find(|id| matches!(self.sketch().entities.get(*id), Some(Entity::Line { .. })))
    }

    pub fn hit_test_hover(
        &self,
        rect: egui::Rect,
        response: &egui::Response,
        tolerance: f64,
    ) -> Option<EntityId> {
        let pos = response.hover_pos()?;
        let p = screen_to_plane_point(&self.camera, rect, pos, &self.active_plane)?;
        hit_test_cycled(self.sketch(), p, tolerance, 0)
    }

    pub fn hit_test_hover_multi_plane(
        &self,
        rect: egui::Rect,
        pos: egui::Pos2,
        tolerance: f64,
    ) -> Option<(usize, EntityId)> {
        hit_test_multi_plane(&self.camera, rect, &self.sketches, pos, tolerance, 0)
    }

    pub fn hit_test_click_cycled(
        &mut self,
        rect: egui::Rect,
        pos: egui::Pos2,
        tolerance: f64,
    ) -> Option<EntityId> {
        const SELECT_CYCLE_CLICK_PX: f32 = 4.0;
        let cycle = match self.last_select_click {
            Some((last_pos, last_cycle))
                if last_pos.distance(pos) < SELECT_CYCLE_CLICK_PX =>
            {
                last_cycle + 1
            }
            _ => 0,
        };
        self.last_select_click = Some((pos, cycle));
        let p = screen_to_plane_point(&self.camera, rect, pos, &self.active_plane)?;
        hit_test_cycled(self.sketch(), p, tolerance, cycle)
    }

    pub fn hit_test_click_multi_plane(
        &mut self,
        rect: egui::Rect,
        pos: egui::Pos2,
        tolerance: f64,
    ) -> Option<(usize, EntityId)> {
        const SELECT_CYCLE_CLICK_PX: f32 = 4.0;
        let cycle = match self.last_select_click {
            Some((last_pos, last_cycle))
                if last_pos.distance(pos) < SELECT_CYCLE_CLICK_PX =>
            {
                last_cycle + 1
            }
            _ => 0,
        };
        self.last_select_click = Some((pos, cycle));
        hit_test_multi_plane(&self.camera, rect, &self.sketches, pos, tolerance, cycle)
    }

    pub fn on_click_point(&mut self, p: DVec2) {
        self.pending_points.push(p);
        if self.pending_points.len() == 1 {
            self.dynamic_focus_pending = true;
        }
        if self.pending_points.len() >= required_points(self.tool) {
            self.finish_multipoint();
        }
    }

    pub fn handle_line_chain_click(&mut self, p: DVec2, close_tol: f64) {
        let Some(&last) = self.pending_points.first() else {
            self.pending_points.push(p);
            self.line_chain_start = Some(p);
            self.dynamic_focus_pending = true;
            return;
        };

        if (p - last).length() < Self::LINE_CHAIN_DEGENERATE_EPS {
            return;
        }

        let closing = self
            .line_chain_start
            .is_some_and(|start| self.line_chain_segments >= 2 && (p - start).length() <= close_tol);

        let end = if closing {
            self.line_chain_start.unwrap()
        } else {
            p
        };

        self.execute_sketch_command(Box::new(InsertEntities::new(
            "Garis",
            vec![Entity::line(last, end).with_construction(self.construction_mode)],
        )));
        self.line_chain_segments += 1;

        if closing {
            self.pending_points.clear();
            self.line_chain_start = None;
            self.line_chain_segments = 0;
            self.dynamic_input.clear();
            self.dynamic_focus_pending = false;
        } else {
            self.pending_points = vec![end];
            self.dynamic_focus_pending = true;
        }
    }

    pub fn handle_spline_click(&mut self, p: DVec2, close_tol: f64) {
        if self.pending_points.is_empty() {
            self.pending_points.push(p);
            return;
        }

        let first = self.pending_points[0];
        let closing = self.pending_points.len() >= 2 && (p - first).length() <= close_tol;
        let is_last_repeat = self
            .pending_points
            .last()
            .is_some_and(|last| (*last - p).length() < 1e-6);

        if closing {
            self.pending_points.push(first);
            self.finish_multipoint();
        } else if is_last_repeat {
            self.finish_multipoint();
        } else {
            self.pending_points.push(p);
        }
    }

    pub fn finish_multipoint(&mut self) {
        let pts = std::mem::take(&mut self.pending_points);
        let cmd: Option<Box<dyn Command<Sketch>>> = match self.tool {
            ToolKind::Rectangle => {
                let min = pts[0].min(pts[1]);
                let max = pts[0].max(pts[1]);
                let corners = [
                    DVec2::new(min.x, min.y),
                    DVec2::new(max.x, min.y),
                    DVec2::new(max.x, max.y),
                    DVec2::new(min.x, max.y),
                ];
                let lines = (0..4)
                    .map(|i| {
                        Entity::line(corners[i], corners[(i + 1) % 4])
                            .with_construction(self.construction_mode)
                    })
                    .collect();
                Some(Box::new(InsertEntities::new("Persegi", lines)))
            }
            ToolKind::Circle => {
                let radius = (pts[1] - pts[0]).length();
                (radius > 1e-6).then(|| {
                    Box::new(InsertEntities::new(
                        "Lingkaran",
                        vec![Entity::circle(pts[0], radius).with_construction(self.construction_mode)],
                    )) as Box<dyn Command<Sketch>>
                })
            }
            ToolKind::Ellipse => {
                let radius_x = (pts[1].x - pts[0].x).abs();
                let radius_y = (pts[1].y - pts[0].y).abs();
                (radius_x > 1e-6 && radius_y > 1e-6).then(|| {
                    Box::new(InsertEntities::new(
                        "Ellips",
                        vec![
                            Entity::ellipse(pts[0], radius_x, radius_y)
                                .with_construction(self.construction_mode),
                        ],
                    )) as Box<dyn Command<Sketch>>
                })
            }
            ToolKind::Spline => {
                (pts.len() >= 2).then(|| {
                    Box::new(InsertEntities::new(
                        "Spline",
                        vec![Entity::spline(pts).with_construction(self.construction_mode)],
                    )) as Box<dyn Command<Sketch>>
                })
            }
            ToolKind::Arc => arc_from_three_points(pts[0], pts[1], pts[2]).map(|e| {
                Box::new(InsertEntities::new(
                    "Arc",
                    vec![e.with_construction(self.construction_mode)],
                )) as _
            }),
            ToolKind::Mirror => {
                let (axis_a, axis_b) = (pts[0], pts[1]);
                let mirrored: Vec<Entity> = self
                    .selected
                    .iter()
                    .filter_map(|id| self.sketch().entities.get(*id))
                    .filter_map(|e| mirror_entity(e, axis_a, axis_b))
                    .collect();
                (!mirrored.is_empty())
                    .then(|| Box::new(InsertEntities::new("Cerminkan", mirrored)) as _)
            }
            ToolKind::Revolve => {
                let (axis_origin, axis_end) = (pts[0], pts[1]);
                let raw_dir = axis_end - axis_origin;
                if raw_dir.length() < 1e-6 {
                    let err_title = ducad_i18n::t!("revolve-axis-too-short-title");
                    let err_desc = ducad_i18n::t!("revolve-axis-too-short-desc");
                    let tip_1 = ducad_i18n::t!("revolve-axis-tip-1");
                    let tip_2 = ducad_i18n::t!("revolve-axis-tip-2");
                    self.alert_modal.show_error(
                        err_title.clone(),
                        err_desc,
                        vec![
                            tip_1.as_str(),
                            tip_2.as_str(),
                        ],
                    );
                    self.model_status = Some(err_title);
                } else {
                    self.revolve_staged_axis = Some((axis_origin, axis_end));
                    self.model_status = Some(ducad_i18n::t!("revolve-axis-staged-status"));
                }
                None
            }
            ToolKind::Measure => {
                self.measurements
                    .push(Measurement::Distance { a: pts[0], b: pts[1] });
                None
            }
            ToolKind::MeasureAngle => {
                self.measurements.push(Measurement::Angle {
                    a: pts[0],
                    vertex: pts[1],
                    b: pts[2],
                });
                None
            }
            _ => None,
        };
        if let Some(c) = cmd {
            self.execute_sketch_command(c);
            self.dynamic_input.clear();
            self.dynamic_focus_pending = false;
        }
    }

    /// Toggle mode garis konstruksi atau toggle status garis konstruksi pada entitas yang dipilih / di-hover.
    pub fn toggle_construction_action(&mut self) {
        if !self.selected.is_empty() {
            let ids: Vec<EntityId> = self.selected.iter().copied().collect();
            let all_construction = ids.iter().all(|id| {
                self.sketch().entities.get(*id).map_or(false, |e| e.is_construction())
            });
            let target_construction = !all_construction;
            self.execute_sketch_command(Box::new(ToggleConstruction::new(ids, target_construction)));
            self.model_status = Some(if target_construction {
                "Entitas diubah menjadi Garis Konstruksi (Reference Line)".to_string()
            } else {
                "Entitas diubah menjadi Garis Standar".to_string()
            });
        } else if let Some(hovered_id) = self.hovered {
            let curr = self.sketch().entities.get(hovered_id).map_or(false, |e| e.is_construction());
            let target = !curr;
            self.execute_sketch_command(Box::new(ToggleConstruction::new(vec![hovered_id], target)));
            self.model_status = Some(if target {
                "Garis Konstruksi diaktifkan".to_string()
            } else {
                "Garis Standar diaktifkan".to_string()
            });
        } else {
            self.construction_mode = !self.construction_mode;
            self.model_status = Some(if self.construction_mode {
                "Mode Garis Konstruksi: AKTIF (Oranye Putus-putus)".to_string()
            } else {
                "Mode Garis Konstruksi: NONAKTIF (Garis Normal)".to_string()
            });
        }
    }

    pub fn commit_sketch_move_drag(&mut self) {
        self.sketch_move_dragging = false;
        let delta = std::mem::take(&mut self.sketch_move_delta);
        let Some(ids) = self
            .sketch_move_target
            .take()
            .map(|s| s.into_iter().collect::<Vec<_>>())
        else {
            return;
        };
        if delta.length() < 1e-6 || ids.is_empty() {
            return;
        }
        self.execute_sketch_command(Box::new(TranslateEntities::new("Geser Sketch", ids, delta)));
    }

    pub fn translate_selected_body(&mut self, delta: Vec3) {
        let Some((target_id, _)) = self.selected_single_body_center() else {
            return;
        };
        let Some(target_geo) = self.model.geometry.get(target_id) else {
            return;
        };
        match ducad_kernel::translate_shape(
            &target_geo.shape,
            delta.x as f64,
            delta.y as f64,
            delta.z as f64,
        ) {
            Ok(new_shape) => {
                let new_geo = BodyGeometry::from_shape(new_shape);
                if self.body_copy_mode {
                    let cmd = AddSolidCommand::new("Salin Body", new_geo);
                    self.execute_model_command(
                        Box::new(cmd),
                        &format!("Menduplikasi & menggeser ({:.1}, {:.1}, {:.1}) mm", delta.x, delta.y, delta.z),
                    );
                    self.model_status = Some(format!(
                        "Body diduplikasi & digeser ({:.1}, {:.1}, {:.1}) mm",
                        delta.x, delta.y, delta.z
                    ));
                } else {
                    self.execute_model_command(
                        Box::new(ReplaceGeometryCommand::new(
                            "Geser Body",
                            target_id,
                            new_geo,
                        )),
                        &format!("Menggeser posisi objek sejauh ({:.1}, {:.1}, {:.1}) mm", delta.x, delta.y, delta.z),
                    );
                    self.round_history.remove(&target_id);
                    self.model_status = Some(format!(
                        "Body digeser ({:.1}, {:.1}, {:.1}) mm",
                        delta.x, delta.y, delta.z
                    ));
                }
            }
            Err(e) => {
                self.model_status = Some(format!("Geser body gagal: {e}"));
            }
        }
    }

    pub fn rotate_selected_body(&mut self, axis: Vec3, angle_deg: f64) {
        let Some((target_id, center)) = self.selected_single_body_center() else {
            return;
        };
        let Some(target_geo) = self.model.geometry.get(target_id) else {
            return;
        };
        let angle_rad = (angle_deg as f64).to_radians();
        let pivot = (center.x as f64, center.y as f64, center.z as f64);
        let axis_tup = (axis.x as f64, axis.y as f64, axis.z as f64);
        match ducad_kernel::rotate_shape(&target_geo.shape, pivot, axis_tup, angle_rad) {
            Ok(new_shape) => {
                let new_geo = BodyGeometry::from_shape(new_shape);
                if self.body_copy_mode {
                    let cmd = AddSolidCommand::new("Salin Body", new_geo);
                    self.execute_model_command(Box::new(cmd), &format!("Menduplikasi & memutar rotasi {:.1}°", angle_deg));
                    self.model_status = Some(format!("Body diduplikasi & diputar {:.1}°", angle_deg));
                } else {
                    self.execute_model_command(
                        Box::new(ReplaceGeometryCommand::new(
                            "Putar Body",
                            target_id,
                            new_geo,
                        )),
                        &format!("Memutar rotasi objek sebesar {:.1}°", angle_deg),
                    );
                    self.round_history.remove(&target_id);
                    self.model_status = Some(format!("Body diputar {:.1}°", angle_deg));
                }
            }
            Err(e) => {
                self.model_status = Some(format!("Putar body gagal: {e}"));
            }
        }
    }

    /// Resize body terpilih lewat SATU pill dimensi bbox (`axis` 0=X/1=Y/2=Z) yg diklik
    /// langsung di viewport — Fase 4 revisi UX (dulu panel X/Y/Z + tombol Terapkan, gampang
    /// bikin nilai non-proporsional yg diam-diam ditolak & terkesan "tidak ngapa-ngapain").
    /// Faktor SELALU dihitung dari 1 sumbu yg diedit itu (`new_length_mm` / panjang sumbu
    /// itu sekarang) lalu diterapkan uniform ke X/Y/Z sekaligus — `ducad_kernel::scale_shape`
    /// memakai uniform scaling murni OpenCASCADE `BRepBuilderAPI_Transform` yang stabil &
    /// tidak merusak geometri.
    pub fn scale_selected_body_by_axis(&mut self, axis: usize, new_length_mm: f64) {
        let Some((target_id, center)) = self.selected_single_body_center() else {
            return;
        };
        let Some(target_geo) = self.model.geometry.get(target_id) else {
            return;
        };
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for p in &target_geo.mesh.positions {
            for k in 0..3 {
                min[k] = min[k].min(p[k]);
                max[k] = max[k].max(p[k]);
            }
        }
        let old_len = (max[axis] - min[axis]).abs() as f64;
        if old_len < 1e-4 {
            self.model_status = Some("Resize body gagal: ukuran awal terlalu kecil".to_string());
            return;
        }
        if new_length_mm <= 0.0 {
            self.model_status = Some("Resize body gagal: ukuran harus > 0".to_string());
            return;
        }
        let factor = new_length_mm / old_len;

        let pivot = (center.x as f64, center.y as f64, center.z as f64);
        match ducad_kernel::scale_shape(&target_geo.shape, pivot, factor) {
            Ok(new_shape) => {
                let new_geo = BodyGeometry::from_shape(new_shape);
                if self.body_copy_mode {
                    let cmd = AddSolidCommand::new("Salin Body", new_geo);
                    self.execute_model_command(
                        Box::new(cmd),
                        &format!("Skala {:.0}%", factor * 100.0),
                    );
                    self.model_status =
                        Some(format!("Body diduplikasi & diresize {:.0}%", factor * 100.0));
                } else {
                    self.execute_model_command(
                        Box::new(ReplaceGeometryCommand::new(
                            "Resize Body",
                            target_id,
                            new_geo,
                        )),
                        &format!("Skala {:.0}%", factor * 100.0),
                    );
                    self.round_history.remove(&target_id);
                    self.model_status =
                        Some(format!("Body diresize {:.0}%", factor * 100.0));
                }
            }
            Err(e) => {
                self.model_status = Some(format!("Resize body gagal: {e}"));
            }
        }
    }

    /// Resize body 3D berdasarkan panjang rusuk (edge) yang diedit user langsung pada pill
    /// dimensi viewport. Hanya dimensi yang bersangkutan (misal tinggi balok) yang berubah,
    /// sedangkan sisi lainnya tetap utuh via `resize_shape_along_edge`.
    pub fn scale_body_by_edge(&mut self, body_id: BodyId, edge_idx: usize, new_length_mm: f64) {
        let Some(target_geo) = self.model.geometry.get(body_id) else {
            return;
        };
        let Some((_, start, end, old_len)) = target_geo.edge_dims.get(edge_idx).copied() else {
            return;
        };
        if old_len < 1e-4 {
            self.model_status = Some("Resize body gagal: panjang rusuk terlalu kecil".to_string());
            return;
        }
        if new_length_mm <= 0.0 {
            self.model_status = Some("Resize body gagal: ukuran harus > 0".to_string());
            return;
        }
        if (new_length_mm - old_len).abs() < 1e-4 {
            return;
        }

        match ducad_kernel::resize_shape_along_edge(&target_geo.shape, start, end, new_length_mm) {
            Ok(new_shape) => {
                let new_geo = BodyGeometry::from_shape(new_shape);
                if self.body_copy_mode {
                    let cmd = AddSolidCommand::new("Salin Body", new_geo);
                    self.execute_model_command(
                        Box::new(cmd),
                        &format!("Ubah Rusuk {:.2} mm", new_length_mm),
                    );
                    self.model_status =
                        Some(format!("Body diduplikasi & diubah ukurannya ke {:.2} mm", new_length_mm));
                } else {
                    self.execute_model_command(
                        Box::new(ReplaceGeometryCommand::new(
                            "Ubah Ukuran Rusuk",
                            body_id,
                            new_geo,
                        )),
                        &format!("Ubah Rusuk {:.2} mm", new_length_mm),
                    );
                    self.round_history.remove(&body_id);
                    self.model_status =
                        Some(format!("Ukuran rusuk diubah ke {:.2} mm", new_length_mm));
                }
            }
            Err(e) => {
                self.model_status = Some(format!("Ubah ukuran gagal: {e}"));
            }
        }
    }

    pub fn handle_sketch_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: egui::Rect,
        raw_cursor: Option<DVec2>,
    ) {
        if self.picking_mode != PickMode::None {
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.picking_mode = PickMode::None;
            } else {
                self.handle_3d_picking(response, rect);
            }
            return;
        }

        if self.handle_plane_activation(ui, response, rect) {
            return;
        }

        let text_focused = ui.ctx().memory(|m| m.focused().is_some());

        if !text_focused {
            if !self.selected.is_empty()
                && ui.input(|i| {
                    i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)
                })
            {
                let ids: Vec<_> = self.selected.drain().collect();
                self.execute_sketch_command(Box::new(DeleteEntities::new(ids)));
                self.sketch_move_armed = false;
                self.sketch_move_target = None;
            }

            let cmd_held = ui.input(|i| i.modifiers.command);
            if self.is_sketching && (cmd_held || self.sketch_move_armed) {
                if let Some(ids) = self.nudge_target_ids() {
                    const NUDGE_STEP_MM: f64 = 1.0;
                    let nudge = ui.input(|i| {
                        if i.key_pressed(egui::Key::ArrowLeft) {
                            Some(DVec2::new(-NUDGE_STEP_MM, 0.0))
                        } else if i.key_pressed(egui::Key::ArrowRight) {
                            Some(DVec2::new(NUDGE_STEP_MM, 0.0))
                        } else if i.key_pressed(egui::Key::ArrowUp) {
                            Some(DVec2::new(0.0, NUDGE_STEP_MM))
                        } else if i.key_pressed(egui::Key::ArrowDown) {
                            Some(DVec2::new(0.0, -NUDGE_STEP_MM))
                        } else {
                            None
                        }
                    });
                    if let Some(delta) = nudge {
                        self.execute_sketch_command(Box::new(TranslateEntities::new(
                            "Geser Sketch (Panah)",
                            ids,
                            delta,
                        )));
                    }
                }
            }

            if self.body_move_armed {
                const NUDGE_STEP_MM: f32 = 1.0;
                let nudge = ui.input(|i| {
                    if i.key_pressed(egui::Key::ArrowLeft) {
                        Some(Vec3::new(-NUDGE_STEP_MM, 0.0, 0.0))
                    } else if i.key_pressed(egui::Key::ArrowRight) {
                        Some(Vec3::new(NUDGE_STEP_MM, 0.0, 0.0))
                    } else if i.key_pressed(egui::Key::ArrowUp) {
                        Some(Vec3::new(0.0, NUDGE_STEP_MM, 0.0))
                    } else if i.key_pressed(egui::Key::ArrowDown) {
                        Some(Vec3::new(0.0, -NUDGE_STEP_MM, 0.0))
                    } else if i.key_pressed(egui::Key::PageUp) {
                        Some(Vec3::new(0.0, 0.0, NUDGE_STEP_MM))
                    } else if i.key_pressed(egui::Key::PageDown) {
                        Some(Vec3::new(0.0, 0.0, -NUDGE_STEP_MM))
                    } else {
                        None
                    }
                });
                if let Some(delta) = nudge {
                    self.translate_selected_body(delta);
                }
            }

            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                if self.editing_dimension_entity.is_some()
                    || self.editing_edge_dim.is_some()
                    || self.editing_body_dim_axis.is_some()
                    || self.gizmo_dimension_editing
                    || self.face_gizmo_dimension_editing
                    || self.vertex_gizmo_dimension_editing
                    || self.edge_gizmo_dimension_editing
                {
                    self.editing_dimension_entity = None;
                    self.editing_edge_dim = None;
                    self.editing_body_dim_axis = None;
                    self.gizmo_dimension_editing = false;
                    self.face_gizmo_dimension_editing = false;
                    self.vertex_gizmo_dimension_editing = false;
                    self.edge_gizmo_dimension_editing = false;
                } else if self.active_vertex.is_some() || self.active_edge.is_some() {
                    self.active_vertex = None;
                    self.active_edge = None;
                    self.editing_round = None;
                } else if !self.pending_points.is_empty()
                    || !self.pending_point_refs.is_empty()
                    || self.offset_source.is_some()
                {
                    self.pending_points.clear();
                    self.pending_point_refs.clear();
                    self.offset_source = None;
                    self.line_chain_start = None;
                    self.line_chain_segments = 0;
                    self.dynamic_input.clear();
                    self.dynamic_focus_pending = false;
                } else if self.sketch_move_armed {
                    self.sketch_move_armed = false;
                    self.sketch_move_target = None;
                } else if self.body_move_armed {
                    self.body_move_armed = false;
                    self.body_move_target = None;
                } else if !self.selected.is_empty() {
                    self.selected.clear();
                } else if self.is_sketching {
                    self.is_sketching = false;
                    self.left_toolbar.is_sketching = false;
                } else {
                    self.set_tool(ToolKind::Select);
                }
            }
            if ui.input(|i| i.key_pressed(egui::Key::S)) && !self.is_sketching {
                self.is_sketching = true;
                self.left_toolbar.is_sketching = true;
                self.camera.orient_to_plane(&self.active_plane);
            }
            if ui.input(|i| i.key_pressed(egui::Key::P)) && !self.is_sketching {
                self.set_tool(ToolKind::Pattern);
            }
            if self.is_sketching {
                if ui.input(|i| i.key_pressed(egui::Key::L)) {
                    self.set_tool(ToolKind::Line);
                }
                if ui.input(|i| i.key_pressed(egui::Key::R)) {
                    self.set_tool(ToolKind::Rectangle);
                }
                if ui.input(|i| i.key_pressed(egui::Key::C)) {
                    self.set_tool(ToolKind::Circle);
                }
                if ui.input(|i| i.key_pressed(egui::Key::E)) {
                    self.set_tool(ToolKind::Ellipse);
                }
                if ui.input(|i| i.key_pressed(egui::Key::A)) {
                    self.set_tool(ToolKind::Arc);
                }
                if ui.input(|i| i.key_pressed(egui::Key::O)) {
                    self.set_tool(ToolKind::Offset);
                }
                if ui.input(|i| i.key_pressed(egui::Key::M)) {
                    self.set_tool(ToolKind::Mirror);
                }
                if ui.input(|i| i.key_pressed(egui::Key::T)) {
                    self.set_tool(ToolKind::Trim);
                }
                if ui.input(|i| i.key_pressed(egui::Key::P)) {
                    self.set_tool(ToolKind::Pattern);
                }
                if ui.input(|i| i.key_pressed(egui::Key::V)) {
                    self.open_revolve_dialog();
                }
                if ui.input(|i| i.key_pressed(egui::Key::X)) {
                    self.toggle_construction_action();
                }
            }
        }

        let suppress_click_from_radial = std::mem::take(&mut self.radial_suppress_click);

        // Klik yg jatuh di pill dimensi bbox body 3D (`body_dim_pill_screen_hits`, "Tampilkan
        // Semua Ukuran" di mode 3D) HARUS berarti "edit ukuran ini" — bukan raycast pilih
        // rusuk/sudut buat fillet/chamfer, walau posisinya sengaja persis di tengah rusuk bbox
        // (yg pada body axis-aligned sederhana sering berhimpit dgn rusuk asli objek). Dicek di
        // sini (SEBELUM raycast pick di bawah dieksekusi, pakai posisi kursor SEKARANG — pill
        // itu sendiri baru digambar belakangan di `dynamic_input_ui` frame yg sama, tapi klik-nya
        // sendiri tetap kedeteksi widget-nya independen krn egui tidak exclusive-consume per
        // klik), bukan sesudahnya — satu klik cuma boleh berarti satu hal.
        let click_hits_body_dim_pill = response
            .hover_pos()
            .or_else(|| ui.input(|i| i.pointer.latest_pos()))
            .is_some_and(|pos| self.body_dim_pill_hit_at(rect, pos));

        let Some(raw) = raw_cursor else {
            self.hovered = None;
            self.last_snap = None;
            return;
        };
        let tol = pixel_tolerance_to_world(&self.camera, rect) * 14.0;
        let grid_step = 10.0;

        match self.tool {
            ToolKind::Select | ToolKind::Loft | ToolKind::Sweep => {
                self.last_snap = None;

                if self.extruding_from_gizmo {
                    return;
                }

                let mut region_hit: Option<ClosedRegion> = None;

                if self.tool == ToolKind::Sweep {
                    self.hovered_vertex_marker = None;
                    self.hovered_corner_2d = None;
                    if let Some(pos) = response.hover_pos() {
                        if let Some((plane_idx, ent_id)) = self.hit_test_hover_multi_plane(rect, pos, tol) {
                            self.hovered = Some(ent_id);
                            self.hovered_plane_idx = Some(plane_idx);
                        } else {
                            self.hovered = None;
                            self.hovered_plane_idx = None;
                        }
                    }
                } else {
                    let corner_hover_2d = if self.is_sketching && response.hovered() && !self.sketch().entities.is_empty() {
                        find_corner_lines_at_point(self.sketch(), raw, tol * 2.5)
                    } else {
                        None
                    };
                    self.hovered_corner_2d = corner_hover_2d.map(|(_, _, pt)| pt);

                    region_hit =
                        if corner_hover_2d.is_none() && !self.sketch().entities.is_empty() && response.hovered() {
                            if let Some(r) = find_region_at_point(self.sketch(), raw) {
                                Some(r)
                            } else if let Some(hit) = self.hit_test_hover(rect, response, tol) {
                                find_region_containing_entity(self.sketch(), hit)
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                    self.hovered = if region_hit.is_some() || corner_hover_2d.is_some() {
                        None
                    } else {
                        response
                            .hovered()
                            .then(|| self.hit_test_hover(rect, response, tol))
                            .flatten()
                    };
                    self.hovered_plane_idx = Some(self.active_plane_index());
                }

                self.hovered_vertex_marker = if !self.is_sketching
                    && response.hovered()
                    && !self.filleting_vertex_from_gizmo
                    && !self.filleting_edge_from_gizmo
                    && !self.extruding_face_from_gizmo
                {
                    response
                        .hover_pos()
                        .and_then(|pos| self.pick_body_vertex_at_cursor(rect, pos))
                        .map(|(id, _, vhit)| (id, vhit))
                } else {
                    None
                };

                if response.clicked() && !suppress_click_from_radial && !click_hits_body_dim_pill {
                    self.sketch_move_armed = false;
                    self.sketch_move_target = None;
                    self.body_move_armed = false;
                    self.body_move_target = None;
                    let shift = ui.input(|i| i.modifiers.shift);
                    let cmd = ui.input(|i| i.modifiers.command);
                    let is_multi = shift || cmd;
                    let click_pos = response
                        .hover_pos()
                        .or_else(|| ui.input(|i| i.pointer.latest_pos()))
                        .or_else(|| ui.input(|i| i.pointer.interact_pos()));

                    // Prioritas 1: Jika mengklik sudut pertemuan 2 garis sketsa 2D
                    let corner_pick_2d = if !shift && self.is_sketching && !self.sketch().entities.is_empty() {
                        find_corner_lines_at_point(self.sketch(), raw, tol * 2.5)
                    } else {
                        None
                    };

                    if let Some((id1, id2, corner)) = corner_pick_2d {
                        self.selected.clear();
                        self.selected_bodies.clear();
                        self.active_face = None;
                        self.active_vertex = None;
                        self.active_edge = None;
                        self.editing_round = None;
                        self.body_move_target = None;
                        self.active_sketch_corner = Some((id1, id2, corner));
                        self.sketch_corner_gizmo_radius = 5.0;
                        self.sketch_corner_edit_input = "5.0".to_string();
                        self.sketch_corner_dimension_editing = false;
                        self.model_status = Some(
                            "Sudut sketsa 2D terpilih — tarik gizmo = fillet bulat, dorong = chamfer lurus".to_string(),
                        );
                        return;
                    }

                    if !self.sketch_corner_gizmo_active && !self.sketch_corner_dimension_editing {
                        self.active_sketch_corner = None;
                    }

                    let face_pick_3d = if !self.is_sketching {
                        click_pos.and_then(|pos| self.pick_body_face_at_cursor(rect, pos))
                    } else {
                        None
                    };

                    if is_multi && !self.is_sketching {
                        if self.tool == ToolKind::DraftAngle || self.tool == ToolKind::Shell || self.tool == ToolKind::Rib {
                            if let Some((b_id, ray, _hit)) = face_pick_3d {
                                self.selected.clear();
                                self.selected_bodies.clear();
                                self.selected_bodies.insert(b_id);
                                if let Some((active_id, active_ray, _)) = self.active_face.take() {
                                    if active_id == b_id && !self.selected_faces.contains(&active_ray) {
                                        self.selected_faces.push(active_ray);
                                    }
                                }
                                if !self.selected_faces.contains(&ray) {
                                    self.selected_faces.push(ray);
                                } else {
                                    self.selected_faces.retain(|r| *r != ray);
                                }
                                let count = self.selected_faces.len();
                                self.model_status = Some(format!("{} face terpilih", count));
                                return;
                            }
                        } else if let Some((b_id, ..)) = face_pick_3d {
                            self.selected.clear();
                            if let Some((prev_id, ..)) = self.active_face.take() {
                                self.selected_bodies.insert(prev_id);
                            }
                            self.active_vertex = None;
                            self.active_edge = None;
                            self.editing_round = None;
                            self.body_move_target = None;
                            if !self.selected_bodies.remove(&b_id) {
                                self.selected_bodies.insert(b_id);
                            }
                            let count = self.selected_bodies.len();
                            self.model_status = Some(format!("{} body terpilih", count));
                            return;
                        }
                    }

                    let round_edit = face_pick_3d.as_ref().and_then(|(b_id, _, hit)| {
                        self.find_round_feature_near(
                            *b_id,
                            hit.hit_point,
                            hit.surface_kind,
                            rect,
                        )
                        .map(|idx| (*b_id, idx))
                    });

                    let vertex_pick_3d = if round_edit.is_none() && !self.is_sketching && !shift
                    {
                        click_pos.and_then(|pos| self.pick_body_vertex_at_cursor(rect, pos))
                    } else {
                        None
                    };

                    let edge_pick_3d = if round_edit.is_none()
                        && vertex_pick_3d.is_none()
                        && !self.is_sketching
                        && !shift
                    {
                        click_pos.and_then(|pos| self.pick_body_edge_at_cursor(rect, pos))
                    } else {
                        None
                    };

                    let now = std::time::Instant::now();
                    let is_face_tool = self.tool == ToolKind::DraftAngle || self.tool == ToolKind::Shell || self.tool == ToolKind::Rib;
                    let is_double_click = !is_face_tool && (
                        response.double_clicked()
                        || self.last_body_select_click.as_ref().is_some_and(|(last_id, last_time)| {
                            face_pick_3d.as_ref().is_some_and(|(b_id, ..)| *last_id == *b_id)
                                && now.duration_since(*last_time).as_millis() < 500
                        })
                        || (face_pick_3d.as_ref().is_some_and(|(b_id, ..)| {
                            self.active_face.as_ref().is_some_and(|(cur_id, ..)| cur_id == b_id)
                        }))
                    );

                    if let Some((b_id, idx)) = round_edit {
                        let feature = self.round_history[&b_id].features[idx].clone();
                        self.selected.clear();
                        self.selected_bodies.clear();
                        self.selected_bodies.insert(b_id);
                        self.editing_round = Some((b_id, idx));
                        self.active_face = None;
                        self.last_body_select_click = None;
                        // Nilai kerja gizmo BERTANDA: fitur `Chamfer` dibuka
                        // kembali sebagai negatif supaya lanjut mendorong
                        // tetap kontinu jadi chamfer (bukan lompat balik ke
                        // fillet) — lihat `RoundStyle`/`round_style_and_magnitude`.
                        let signed_radius = match feature.style {
                            RoundStyle::Fillet => feature.radius,
                            RoundStyle::Chamfer => -feature.radius,
                        };
                        match feature.kind {
                            RoundKind::Vertex => {
                                self.active_vertex =
                                    Some((b_id, feature.ray, feature.anchor));
                                self.active_edge = None;
                                self.vertex_gizmo_radius = signed_radius;
                                self.vertex_gizmo_edit_input =
                                    format!("{:.1}", self.unit.to_display_val(feature.radius));
                            }
                            RoundKind::Edge => {
                                self.active_edge =
                                    Some((b_id, feature.ray, feature.anchor));
                                self.active_vertex = None;
                                self.edge_gizmo_radius = signed_radius;
                                self.edge_gizmo_edit_input =
                                    format!("{:.1}", self.unit.to_display_val(feature.radius));
                            }
                        }
                        self.model_status = Some(
                            "Rounding terpilih — tarik = fillet bulat, dorong = chamfer lurus, dorong sampai 0 utk kembali menyiku".to_string(),
                        );
                    } else if let Some((b_id, ray, vhit)) = vertex_pick_3d {
                        self.selected.clear();
                        self.selected_bodies.clear();
                        self.selected_bodies.insert(b_id);
                        self.active_vertex = Some((b_id, ray, vhit));
                        self.active_face = None;
                        self.active_edge = None;
                        self.editing_round = None;
                        self.last_body_select_click = None;
                        self.vertex_gizmo_radius = 3.0;
                        self.vertex_gizmo_edit_input = "3".to_string();
                        self.model_status = Some(
                            "Sudut (vertex) 3D terpilih — tarik gizmo = fillet bulat, dorong = chamfer lurus".to_string(),
                        );
                    } else if let Some((b_id, ray, point)) = edge_pick_3d {
                        self.selected.clear();
                        self.selected_bodies.clear();
                        self.selected_bodies.insert(b_id);
                        self.active_edge = Some((b_id, ray, point));
                        self.active_face = None;
                        self.active_vertex = None;
                        self.editing_round = None;
                        self.last_body_select_click = None;
                        self.edge_gizmo_radius = 3.0;
                        self.edge_gizmo_edit_input = "3".to_string();
                        self.model_status = Some(
                            "Rusuk (edge) 3D terpilih — tarik gizmo = fillet bulat, dorong = chamfer lurus".to_string(),
                        );
                    } else if let Some((b_id, ray, mut hit)) = face_pick_3d {
                        self.selected.clear();
                        if is_double_click {
                            // Klik 2x / Klik ulang: Memilih seluruh objek (body) -> memunculkan 3D Transform Gizmo
                            self.selected_bodies.clear();
                            self.selected_bodies.insert(b_id);
                            self.active_face = None;
                            self.active_vertex = None;
                            self.active_edge = None;
                            self.editing_round = None;
                            self.body_move_target = Some(b_id);
                            self.body_move_delta = Vec3::ZERO;
                            self.body_rotate_angle_deg = 0.0;
                            self.last_body_select_click = None;
                            self.model_status = Some(
                                "Objek (solid body) terpilih — gunakan 3D Gizmo untuk geser atau putar".to_string(),
                            );
                        } else {
                            // Klik 1x: Memilih face / sisi yang diklik saja -> memunculkan handle extrude face
                            self.selected_bodies.clear();

                            // Deteksi apakah face/area yang diklik adalah lubang yang sudah ada (Hole Feature Memory)
                            self.editing_hole_idx = None;
                            let mut detected_hole = false;
                            if let Some(hist) = self.hole_history.get(&b_id) {
                                for (feat_idx, feat) in hist.features.iter().enumerate() {
                                    let feat_p = Vec3::new(
                                        feat.pos.0 as f32,
                                        feat.pos.1 as f32,
                                        feat.pos.2 as f32,
                                    );
                                    let click_p = Vec3::new(
                                        hit.hit_point.0 as f32,
                                        hit.hit_point.1 as f32,
                                        hit.hit_point.2 as f32,
                                    );
                                    let center_p = Vec3::new(
                                        hit.centroid.0 as f32,
                                        hit.centroid.1 as f32,
                                        hit.centroid.2 as f32,
                                    );
                                    let max_r = (feat.spec.counterbore_diameter.max(feat.spec.diameter)
                                        / 2.0
                                        + 4.0) as f32;
                                    if (click_p - feat_p).length() <= max_r
                                        || (center_p - feat_p).length() <= max_r
                                    {
                                        self.editing_hole_idx = Some((b_id, feat_idx));
                                        self.hole_popup_state.spec = feat.spec.clone();
                                        self.hole_popup_state.offset_u = feat.offset_u;
                                        self.hole_popup_state.offset_v = feat.offset_v;
                                        self.hole_popup_state.current_pos_3d = Some(feat.pos);
                                        self.hole_popup_state.has_existing_hole = true;
                                        self.hole_popup_state.mode = ducad_ui::HoleOperationMode::EditHole;
                                        hit = feat.face_hit.clone();
                                        detected_hole = true;
                                        break;
                                    }
                                }
                            }

                            if !detected_hole {
                                self.hole_popup_state.has_existing_hole = false;
                                self.hole_popup_state.mode = ducad_ui::HoleOperationMode::NewHole;
                                if self.tool == ToolKind::HoleWizard {
                                    self.hole_popup_state.current_pos_3d = Some(hit.hit_point);
                                    self.hole_popup_state.offset_u = 0.0;
                                    self.hole_popup_state.offset_v = 0.0;
                                }
                            }

                            self.active_face = Some((b_id, ray, hit));
                            self.active_vertex = None;
                            self.active_edge = None;
                            self.editing_round = None;
                            self.body_move_target = None;
                            self.face_gizmo_distance = 15.0;
                            self.face_gizmo_edit_input = "15".to_string();
                            self.last_body_select_click = Some((b_id, now));
                            self.model_status = Some(
                                if detected_hole {
                                    "Fitur lubang terpilih — klik Hole Wizard untuk ubah dimensi / geser posisi".to_string()
                                } else if self.tool == ToolKind::HoleWizard {
                                    "Titik lokasi lubang ditempatkan pada titik klik face ✓".to_string()
                                } else {
                                    "Sisi (face) 3D terpilih — tarik panah gizmo atau masukkan jarak extrude".to_string()
                                },
                            );
                        }
                    } else if self.tool == ToolKind::Sweep {
                        self.active_face = None;
                        self.active_vertex = None;
                        self.active_edge = None;
                        self.editing_round = None;
                        let multi_hit = click_pos.and_then(|pos| self.hit_test_click_multi_plane(rect, pos, tol));
                        let target = multi_hit.or_else(|| self.hovered.and_then(|h| self.hovered_plane_idx.map(|p| (p, h))));
                        if let Some((plane_idx, ent_id)) = target {
                            let plane = Self::plane_for_index(plane_idx);
                            if self.pending_sweep_profile.is_none() {
                                if let Some(r) = find_region_containing_entity(&self.sketches[plane_idx], ent_id) {
                                    let ids: HashSet<EntityId> = r.entity_ids.into_iter().collect();
                                    if let Ok(profile) = crate::model::build_profile_from_selection(&self.sketches[plane_idx], &ids) {
                                        self.pending_sweep_profile = Some((profile, plane));
                                        self.selected.clear();
                                        self.sweep_path_plane_idx = None;
                                        self.model_status = Some("✓ Profil tersimpan! Sekarang klik kurva jalur pada bidang lain.".to_string());
                                    }
                                } else if let Ok(profile) = crate::model::build_profile_from_selection(&self.sketches[plane_idx], &std::iter::once(ent_id).collect()) {
                                    self.pending_sweep_profile = Some((profile, plane));
                                    self.selected.clear();
                                    self.sweep_path_plane_idx = None;
                                    self.model_status = Some("✓ Profil tersimpan! Sekarang klik kurva jalur pada bidang lain.".to_string());
                                } else {
                                    self.model_status = Some("Entitas ini bukan profil 2D tertutup. Pilih profil tertutup.".to_string());
                                }
                            } else {
                                if self.sweep_path_plane_idx != Some(plane_idx) {
                                    self.selected.clear();
                                    self.sweep_path_plane_idx = Some(plane_idx);
                                }
                                if shift {
                                    if !self.selected.remove(&ent_id) {
                                        self.selected.insert(ent_id);
                                    }
                                } else {
                                    self.selected.clear();
                                    self.selected.insert(ent_id);
                                }

                                if let Ok(path) = crate::model::build_path_from_selection_on_plane(&self.sketches[plane_idx], &self.selected, &plane) {
                                    self.pending_sweep_path = Some(path);
                                    self.model_status = Some("✓ Profil & Jalur terpilih! Tekan 'Buat Sweep 3D' di atas atau tekan Enter".to_string());
                                } else {
                                    self.pending_sweep_path = None;
                                }
                            }
                        } else if !shift {
                            self.selected.clear();
                            self.pending_sweep_path = None;
                            self.sweep_path_plane_idx = None;
                        }
                    } else if let Some(reg) = region_hit {
                        self.active_face = None;
                        self.active_vertex = None;
                        self.active_edge = None;
                        self.editing_round = None;
                        if shift || self.tool == ToolKind::Loft || self.tool == ToolKind::Sweep {
                            let already_selected =
                                reg.entity_ids.iter().all(|id| self.selected.contains(id));
                            if already_selected {
                                for id in &reg.entity_ids {
                                    self.selected.remove(id);
                                }
                            } else {
                                for id in &reg.entity_ids {
                                    self.selected.insert(*id);
                                }
                            }
                        } else {
                            self.selected.clear();
                            for id in &reg.entity_ids {
                                self.selected.insert(*id);
                            }
                        }
                        self.gizmo_distance = 20.0;
                        self.gizmo_edit_input = format!(
                            "{:.0}",
                            self.unit.to_display_val(self.gizmo_distance)
                        );
                    } else {
                        let cycled_hit =
                            click_pos.and_then(|pos| self.hit_test_click_cycled(rect, pos, tol));

                        match (cycled_hit.or(self.hovered), shift || self.tool == ToolKind::Loft || self.tool == ToolKind::Sweep) {
                            (Some(hit), true) => {
                                if !self.selected.remove(&hit) {
                                    self.selected.insert(hit);
                                }
                            }
                            (Some(hit), false) => {
                                self.selected.clear();
                                self.active_face = None;
                                self.active_vertex = None;
                                self.active_edge = None;
                                self.selected.insert(hit);
                            }
                            (None, false) => {
                                self.selected.clear();
                                if let Some(pos) = click_pos {
                                    if let Some((b_id, ray, mut hit)) =
                                        self.pick_body_face_at_cursor(rect, pos)
                                    {
                                        let is_double = is_double_click
                                            || self.active_face.as_ref().is_some_and(|(cur_id, ..)| *cur_id == b_id);
                                        if is_double {
                                            self.selected_bodies.clear();
                                            self.selected_bodies.insert(b_id);
                                            self.active_face = None;
                                            self.active_vertex = None;
                                            self.active_edge = None;
                                            self.body_move_target = Some(b_id);
                                            self.last_body_select_click = None;
                                            self.model_status = Some("Objek (solid body) terpilih — gunakan 3D Gizmo untuk geser atau putar".to_string());
                                        } else {
                                            self.selected_bodies.clear();

                                            // Deteksi apakah face/area yang diklik adalah lubang yang sudah ada (Hole Feature Memory)
                                            self.editing_hole_idx = None;
                                            let mut detected_hole = false;
                                            if let Some(hist) = self.hole_history.get(&b_id) {
                                                for (feat_idx, feat) in hist.features.iter().enumerate() {
                                                    let feat_p = Vec3::new(
                                                        feat.pos.0 as f32,
                                                        feat.pos.1 as f32,
                                                        feat.pos.2 as f32,
                                                    );
                                                    let click_p = Vec3::new(
                                                        hit.hit_point.0 as f32,
                                                        hit.hit_point.1 as f32,
                                                        hit.hit_point.2 as f32,
                                                    );
                                                    let center_p = Vec3::new(
                                                        hit.centroid.0 as f32,
                                                        hit.centroid.1 as f32,
                                                        hit.centroid.2 as f32,
                                                    );
                                                    let max_r = (feat.spec.counterbore_diameter.max(feat.spec.diameter)
                                                        / 2.0
                                                        + 4.0) as f32;
                                                    if (click_p - feat_p).length() <= max_r
                                                        || (center_p - feat_p).length() <= max_r
                                                    {
                                                        self.editing_hole_idx = Some((b_id, feat_idx));
                                                        self.hole_popup_state.spec = feat.spec.clone();
                                                        self.hole_popup_state.offset_u = feat.offset_u;
                                                        self.hole_popup_state.offset_v = feat.offset_v;
                                                        self.hole_popup_state.current_pos_3d = Some(feat.pos);
                                                        self.hole_popup_state.has_existing_hole = true;
                                                        self.hole_popup_state.mode = ducad_ui::HoleOperationMode::EditHole;
                                                        hit = feat.face_hit.clone();
                                                        detected_hole = true;
                                                        break;
                                                    }
                                                }
                                            }

                                            if !detected_hole {
                                                self.hole_popup_state.has_existing_hole = false;
                                                self.hole_popup_state.mode = ducad_ui::HoleOperationMode::NewHole;
                                                if self.tool == ToolKind::HoleWizard {
                                                    self.hole_popup_state.current_pos_3d = Some(hit.hit_point);
                                                    self.hole_popup_state.offset_u = 0.0;
                                                    self.hole_popup_state.offset_v = 0.0;
                                                }
                                            }

                                            self.active_face = Some((b_id, ray, hit));
                                            self.active_vertex = None;
                                            self.active_edge = None;
                                            self.body_move_target = None;
                                            self.face_gizmo_distance = 15.0;
                                            self.face_gizmo_edit_input = "15".to_string();
                                            self.last_body_select_click = Some((b_id, now));
                                            self.model_status = Some(
                                                if detected_hole {
                                                    "Fitur lubang terpilih — klik Hole Wizard untuk ubah dimensi / geser posisi".to_string()
                                                } else if self.tool == ToolKind::HoleWizard {
                                                    "Titik lokasi lubang ditempatkan pada titik klik face ✓".to_string()
                                                } else {
                                                    "Sisi (face) 3D terpilih — tarik panah gizmo atau masukkan jarak extrude".to_string()
                                                },
                                            );
                                        }
                                    } else {
                                        self.selected_bodies.clear();
                                        self.active_face = None;
                                        self.active_vertex = None;
                                        self.active_edge = None;
                                        self.body_move_target = None;
                                        self.last_body_select_click = None;
                                    }
                                }
                            }
                            (None, true) => {}
                        }
                    }
                    self.constraint_status = None;
                }

                // Drag box / rubber-band selection di kanvas sketsa 2D
                if response.drag_started_by(egui::PointerButton::Primary)
                    && !self.extruding_from_gizmo
                    && !self.sketch_move_armed
                    && self.body_move_target.is_none()
                    && !click_hits_body_dim_pill
                {
                    if let Some(pos) = response.interact_pointer_pos() {
                        if let Some(p) = screen_to_plane_point(&self.camera, rect, pos, &self.active_plane) {
                            self.selection_box = Some((p, p));
                        }
                    }
                }
                if response.dragged_by(egui::PointerButton::Primary) {
                    if let Some((start_p, _)) = self.selection_box {
                        if let Some(pos) = response.interact_pointer_pos() {
                            if let Some(cur_p) = screen_to_plane_point(&self.camera, rect, pos, &self.active_plane) {
                                self.selection_box = Some((start_p, cur_p));
                            }
                        }
                    }
                }
                if response.drag_stopped() {
                    if let Some((p1, p2)) = self.selection_box.take() {
                        let min = p1.min(p2);
                        let max = p1.max(p2);
                        let drag_dist = (p1 - p2).length();
                        if drag_dist > 2.0 {
                            let matched_ids: Vec<EntityId> = self
                                .sketch()
                                .entities
                                .iter()
                                .filter(|(id, _)| !self.sketch().is_hidden(*id))
                                .filter_map(|(id, entity)| {
                                    let inside = match entity {
                                        Entity::Line { start, end, .. } => {
                                            (start.x >= min.x && start.x <= max.x && start.y >= min.y && start.y <= max.y)
                                                || (end.x >= min.x && end.x <= max.x && end.y >= min.y && end.y <= max.y)
                                        }
                                        Entity::Circle { center, radius, .. } => {
                                            center.x + radius >= min.x
                                                && center.x - radius <= max.x
                                                && center.y + radius >= min.y
                                                && center.y - radius <= max.y
                                        }
                                        Entity::Ellipse { center, radius_x, radius_y, .. } => {
                                            center.x + radius_x >= min.x
                                                && center.x - radius_x <= max.x
                                                && center.y + radius_y >= min.y
                                                && center.y - radius_y <= max.y
                                        }
                                        Entity::Arc { center, radius, .. } => {
                                            center.x + radius >= min.x
                                                && center.x - radius <= max.x
                                                && center.y + radius >= min.y
                                                && center.y - radius <= max.y
                                        }
                                        Entity::Spline { points, .. } => {
                                            points.iter().any(|pt| {
                                                pt.x >= min.x && pt.x <= max.x && pt.y >= min.y && pt.y <= max.y
                                            })
                                        }
                                    };
                                    if inside {
                                        Some(id)
                                    } else {
                                        None
                                    }
                                })
                                .collect();

                            for id in matched_ids {
                                self.selected.insert(id);
                            }
                            self.loft_alignment_dismissed = false;
                        }
                    }
                }
            }
            ToolKind::Line => {
                self.hovered = None;
                self.last_snap = response
                    .hovered()
                    .then(|| self.find_current_snap(raw, tol, grid_step, None))
                    .flatten();
                if response.clicked() {
                    let effective = self.snapped_or(raw);
                    self.handle_line_chain_click(effective, tol);
                }
            }
            ToolKind::Spline => {
                self.hovered = None;
                self.last_snap = response
                    .hovered()
                    .then(|| self.find_current_snap(raw, tol, grid_step, None))
                    .flatten();
                if response.clicked() {
                    let effective = self.snapped_or(raw);
                    self.handle_spline_click(effective, tol);
                }
            }
            ToolKind::Rectangle
            | ToolKind::Circle
            | ToolKind::Ellipse
            | ToolKind::Arc
            | ToolKind::Measure
            | ToolKind::MeasureAngle => {
                self.hovered = None;
                self.last_snap = response
                    .hovered()
                    .then(|| self.find_current_snap(raw, tol, grid_step, None))
                    .flatten();
                if response.clicked() {
                    let effective = self.snapped_or(raw);
                    self.on_click_point(effective);
                }
            }
            ToolKind::Mirror | ToolKind::Revolve => {
                self.hovered = None;
                self.last_snap = None;
                let has_target = !self.selected.is_empty() || (self.tool == ToolKind::Revolve && self.active_face.is_some());
                if has_target {
                    self.last_snap = response
                        .hovered()
                        .then(|| self.find_current_snap(raw, tol, grid_step, None))
                        .flatten();
                    if response.clicked() {
                        let effective = self.snapped_or(raw);
                        self.on_click_point(effective);
                    }
                } else if response.clicked() {
                    self.open_revolve_dialog();
                }
            }
            ToolKind::Offset => {
                self.last_snap = None;
                match self.offset_source {
                    None => {
                        self.hovered = response
                            .hovered()
                            .then(|| self.hit_test_hover(rect, response, tol))
                            .flatten();
                        if response.clicked() {
                            self.offset_source = self.hovered;
                        }
                    }
                    Some(source_id) => {
                        self.hovered = None;
                        if response.clicked() {
                            if let Some(entity) = self.sketch().entities.get(source_id) {
                                if let Some(new_entity) = offset_entity(entity, raw) {
                                    self.execute_sketch_command(Box::new(InsertEntities::new(
                                        "Offset",
                                        vec![new_entity],
                                    )));
                                }
                            }
                            self.offset_source = None;
                        }
                    }
                }
            }
            ToolKind::Trim => {
                self.last_snap = None;
                self.hovered = response
                    .hovered()
                    .then(|| self.hit_test_hover(rect, response, tol))
                    .flatten()
                    .filter(|id| {
                        matches!(self.sketch().entities.get(*id), Some(Entity::Line { .. }))
                    });
                if response.clicked() {
                    if let Some(id) = self.hovered {
                        if let Some(Entity::Line { start, end, .. }) =
                            self.sketch().entities.get(id).cloned()
                        {
                            let click_t = project_t(start, end, raw).clamp(0.0, 1.0);
                            let cuts = line_intersection_params_in_sketch(
                                self.sketch(),
                                (start, end),
                                id,
                            );
                            let remaining = trim_segments(start, end, &cuts, click_t);
                            let new_lines = remaining
                                .into_iter()
                                .map(|(s, e)| Entity::line(s, e))
                                .collect();
                            self.execute_sketch_command(Box::new(ReplaceEntities::new(
                                "Trim",
                                vec![id],
                                new_lines,
                            )));
                            self.hovered = None;
                        }
                    }
                }
            }
            ToolKind::Fillet2D => {
                self.last_snap = None;
                if let Ok(val) = self.dynamic_input.trim().parse::<f64>() {
                    if val > 0.0 {
                        self.fillet_2d_radius = val;
                    }
                }
                let corner_hit = find_corner_lines_at_point(self.sketch(), raw, tol * 2.0);
                if let Some((id1, _id2, _corner)) = corner_hit {
                    self.hovered = Some(id1);
                } else {
                    self.hovered = response
                        .hovered()
                        .then(|| self.hit_test_hover(rect, response, tol))
                        .flatten()
                        .filter(|id| {
                            matches!(self.sketch().entities.get(*id), Some(Entity::Line { .. }))
                        });
                }
                if response.clicked() {
                    if let Some((id1, id2, _corner)) = corner_hit {
                        if let (
                            Some(Entity::Line { start: s1, end: e1, .. }),
                            Some(Entity::Line { start: s2, end: e2, .. }),
                        ) = (
                            self.sketch().entities.get(id1).cloned(),
                            self.sketch().entities.get(id2).cloned(),
                        ) {
                            if let Some(res) =
                                compute_fillet_2d((s1, e1), (s2, e2), self.fillet_2d_radius)
                            {
                                self.execute_sketch_command(Box::new(ReplaceEntities::new(
                                    "Fillet 2D",
                                    vec![id1, id2],
                                    vec![res.trimmed_line1, res.trimmed_line2, res.arc],
                                )));
                                self.fillet_chamfer_first_line = None;
                                self.hovered = None;
                            }
                        }
                    } else if let Some(clicked_id) = self.hovered {
                        if let Some(first_id) = self.fillet_chamfer_first_line {
                            if first_id != clicked_id {
                                if let (
                                    Some(Entity::Line { start: s1, end: e1, .. }),
                                    Some(Entity::Line { start: s2, end: e2, .. }),
                                ) = (
                                    self.sketch().entities.get(first_id).cloned(),
                                    self.sketch().entities.get(clicked_id).cloned(),
                                ) {
                                    if let Some(res) =
                                        compute_fillet_2d((s1, e1), (s2, e2), self.fillet_2d_radius)
                                    {
                                        self.execute_sketch_command(Box::new(
                                            ReplaceEntities::new(
                                                "Fillet 2D",
                                                vec![first_id, clicked_id],
                                                vec![
                                                    res.trimmed_line1,
                                                    res.trimmed_line2,
                                                    res.arc,
                                                ],
                                            ),
                                        ));
                                    }
                                }
                                self.fillet_chamfer_first_line = None;
                                self.hovered = None;
                            }
                        } else {
                            self.fillet_chamfer_first_line = Some(clicked_id);
                        }
                    }
                }
            }
            ToolKind::Chamfer2D => {
                self.last_snap = None;
                if let Ok(val) = self.dynamic_input.trim().parse::<f64>() {
                    if val > 0.0 {
                        self.chamfer_2d_dist = val;
                    }
                }
                let corner_hit = find_corner_lines_at_point(self.sketch(), raw, tol * 2.0);
                if let Some((id1, _id2, _corner)) = corner_hit {
                    self.hovered = Some(id1);
                } else {
                    self.hovered = response
                        .hovered()
                        .then(|| self.hit_test_hover(rect, response, tol))
                        .flatten()
                        .filter(|id| {
                            matches!(self.sketch().entities.get(*id), Some(Entity::Line { .. }))
                        });
                }
                if response.clicked() {
                    if let Some((id1, id2, _corner)) = corner_hit {
                        if let (
                            Some(Entity::Line { start: s1, end: e1, .. }),
                            Some(Entity::Line { start: s2, end: e2, .. }),
                        ) = (
                            self.sketch().entities.get(id1).cloned(),
                            self.sketch().entities.get(id2).cloned(),
                        ) {
                            if let Some(res) = compute_chamfer_2d(
                                (s1, e1),
                                (s2, e2),
                                self.chamfer_2d_dist,
                                self.chamfer_2d_dist,
                            ) {
                                self.execute_sketch_command(Box::new(ReplaceEntities::new(
                                    "Chamfer 2D",
                                    vec![id1, id2],
                                    vec![res.trimmed_line1, res.trimmed_line2, res.bevel_line],
                                )));
                                self.fillet_chamfer_first_line = None;
                                self.hovered = None;
                            }
                        }
                    } else if let Some(clicked_id) = self.hovered {
                        if let Some(first_id) = self.fillet_chamfer_first_line {
                            if first_id != clicked_id {
                                if let (
                                    Some(Entity::Line { start: s1, end: e1, .. }),
                                    Some(Entity::Line { start: s2, end: e2, .. }),
                                ) = (
                                    self.sketch().entities.get(first_id).cloned(),
                                    self.sketch().entities.get(clicked_id).cloned(),
                                ) {
                                    if let Some(res) = compute_chamfer_2d(
                                        (s1, e1),
                                        (s2, e2),
                                        self.chamfer_2d_dist,
                                        self.chamfer_2d_dist,
                                    ) {
                                        self.execute_sketch_command(Box::new(
                                            ReplaceEntities::new(
                                                "Chamfer 2D",
                                                vec![first_id, clicked_id],
                                                vec![
                                                    res.trimmed_line1,
                                                    res.trimmed_line2,
                                                    res.bevel_line,
                                                ],
                                            ),
                                        ));
                                    }
                                }
                                self.fillet_chamfer_first_line = None;
                                self.hovered = None;
                            }
                        } else {
                            self.fillet_chamfer_first_line = Some(clicked_id);
                        }
                    }
                }
            }
            ToolKind::CoincidentPick => {
                self.hovered = None;
                self.last_snap = response
                    .hovered()
                    .then(|| find_snap(self.sketch(), raw, tol, grid_step, None))
                    .flatten();
                if response.clicked() {
                    if let Some(source) = self.last_snap.and_then(|s| s.source) {
                        self.pending_point_refs.push(source);
                        if self.pending_point_refs.len() >= 2 {
                            let refs = std::mem::take(&mut self.pending_point_refs);
                            self.apply_constraint(Constraint::Coincident {
                                a: refs[0],
                                b: refs[1],
                            });
                        }
                    }
                }
            }
            ToolKind::FixedPick => {
                self.hovered = None;
                self.last_snap = response
                    .hovered()
                    .then(|| find_snap(self.sketch(), raw, tol, grid_step, None))
                    .flatten();
                if response.clicked() {
                    if let Some(hit) = self.last_snap {
                        if let Some(source) = hit.source {
                            self.apply_constraint(Constraint::Fixed {
                                point: source,
                                target: hit.point,
                            });
                        }
                    }
                }
            }
            ToolKind::SymmetricPick => {
                self.hovered = None;
                self.last_snap = None;
                if let Some(axis_id) = self.symmetric_axis() {
                    self.last_snap = response
                        .hovered()
                        .then(|| find_snap(self.sketch(), raw, tol, grid_step, Some(axis_id)))
                        .flatten();
                    if response.clicked() {
                        if let Some(source) = self.last_snap.and_then(|s| s.source) {
                            self.pending_point_refs.push(source);
                            if self.pending_point_refs.len() >= 2 {
                                let refs = std::mem::take(&mut self.pending_point_refs);
                                self.apply_constraint(Constraint::Symmetric {
                                    a: refs[0],
                                    b: refs[1],
                                    axis: axis_id,
                                });
                            }
                        }
                    }
                }
            }
            ToolKind::Extrude
            | ToolKind::Shell
            | ToolKind::Rib
            | ToolKind::DraftAngle
            | ToolKind::SplitBody
            | ToolKind::Pattern
            | ToolKind::Boolean
            | ToolKind::SectionView
            | ToolKind::ZebraInspection
            | ToolKind::DraftAnalysis
            | ToolKind::HoleWizard
            | ToolKind::History => {
                self.last_snap = None;
            }
        }
    }
}

/// Cari entitas yang kena ray kursor di antara 3 bidang kerja (Top, Front, Right).
pub fn hit_test_multi_plane(
    camera: &ducad_render::OrbitCamera,
    rect: egui::Rect,
    sketches: &[Sketch; 3],
    pos: egui::Pos2,
    tolerance: f64,
    cycle: usize,
) -> Option<(usize, EntityId)> {
    let (p_near, dir) = crate::viewport::screen_to_ray(camera, rect, pos);
    let mut best: Option<(usize, EntityId, f32)> = None;

    for idx in 0..3 {
        let plane = DuCADApp::plane_for_index(idx);
        let Some(uv) = plane.ray_intersection(p_near, dir) else {
            continue;
        };
        if let Some(id) = hit_test_cycled(&sketches[idx], uv, tolerance, cycle) {
            let hit_3d = plane.to_world(uv, 0.0);
            let dist = (hit_3d - p_near).length();
            if best.as_ref().map_or(true, |(_, _, d)| dist < *d) {
                best = Some((idx, id, dist));
            }
        }
    }

    best.map(|(idx, id, _)| (idx, id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;

    #[test]
    fn test_multi_plane_hit_testing_detects_entities_on_different_planes() {
        let mut sketches = [Sketch::default(), Sketch::default(), Sketch::default()];
        let mut camera = ducad_render::OrbitCamera::default();
        camera.orbit(-0.5, 0.4);

        // Top sketch (plane 0) has a circle
        let c_id = sketches[0].entities.insert(Entity::circle(
            DVec2::new(0.0, 0.0),
            10.0,
        ));
        // Front sketch (plane 1) has a line along Z
        let l_id = sketches[1].entities.insert(Entity::line(
            DVec2::new(0.0, 0.0),
            DVec2::new(0.0, 40.0),
        ));

        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(800.0, 600.0));
        // Hit on front sketch (plane 1) line at Z=20
        let p_screen = crate::viewport::world_to_screen_pos(&camera, rect, glam::vec3(0.0, 0.0, 20.0)).unwrap();
        let hit = hit_test_multi_plane(&camera, rect, &sketches, p_screen, 2.0, 0);
        assert_eq!(hit, Some((1, l_id)));

        // Hit on top sketch (plane 0) circle boundary at (10, 0, 0)
        let p_circle_screen = crate::viewport::world_to_screen_pos(&camera, rect, glam::vec3(10.0, 0.0, 0.0)).unwrap();
        let hit_c = hit_test_multi_plane(&camera, rect, &sketches, p_circle_screen, 2.0, 0);
        assert_eq!(hit_c, Some((0, c_id)));
    }
}
