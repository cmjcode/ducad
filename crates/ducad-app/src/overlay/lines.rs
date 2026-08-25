use ducad_render::{sketch as sketch_render, LineVertex};
use ducad_sketch::{
    all_snap_candidate_points_with_exclude_set, arc_from_three_points, mirror_entity,
    offset_entity, Entity,
};
use glam::{DVec2, Vec3};

use crate::app::DuCADApp;
use crate::input::{chamfer_2d_preview, fillet_2d_preview, trim_removal_preview};
use crate::types::{RoundKind, ToolKind};

impl DuCADApp {
    pub fn build_overlay_lines(
        &self,
        raw_cursor: Option<DVec2>,
        world_scale: f64,
    ) -> Vec<LineVertex> {
        let mut verts = Vec::new();

        let empty_set = std::collections::HashSet::new();
        for idx in 0..3 {
            let plane = Self::plane_for_index(idx);
            let is_active_plane = idx == self.active_plane_index();
            let is_sweep = self.tool == ToolKind::Sweep;

            if is_active_plane || is_sweep {
                let h = if is_sweep {
                    if self.hovered_plane_idx == Some(idx) {
                        self.hovered
                    } else {
                        None
                    }
                } else if is_active_plane {
                    self.hovered
                } else {
                    None
                };

                let sel = if is_sweep {
                    if self.sweep_path_plane_idx == Some(idx) {
                        &self.selected
                    } else {
                        &empty_set
                    }
                } else if is_active_plane {
                    &self.selected
                } else {
                    &empty_set
                };

                verts.extend(sketch_render::entity_lines(
                    &self.sketches[idx],
                    h,
                    sel,
                    &plane,
                ));
            } else {
                verts.extend(sketch_render::inactive_entity_lines(
                    &self.sketches[idx],
                    &plane,
                ));
            }

            if !is_active_plane {
                let outline_color: [f32; 4] = if self.is_sketching {
                    [0.10, 0.55, 0.95, 0.30]
                } else {
                    [0.10, 0.55, 0.95, 0.18]
                };
                verts.extend(ducad_render::grid::plane_outline(
                    &plane,
                    ducad_render::grid::INACTIVE_PLANE_HALF_EXTENT,
                    outline_color,
                ));
            }
        }

        if self.tool == ToolKind::Sweep {
            if let Some((profile, profile_plane)) = &self.pending_sweep_profile {
                let color_staged = [0.15, 0.85, 0.40, 1.0];
                match profile {
                    ducad_kernel::Profile::Circle { center, radius } => {
                        let segs = 32;
                        for i in 0..segs {
                            let a1 = i as f64 * std::f64::consts::TAU / segs as f64;
                            let a2 = (i + 1) as f64 * std::f64::consts::TAU / segs as f64;
                            let p1 = profile_plane.to_world(glam::DVec2::new(center.0 + a1.cos() * radius, center.1 + a1.sin() * radius), 0.05);
                            let p2 = profile_plane.to_world(glam::DVec2::new(center.0 + a2.cos() * radius, center.1 + a2.sin() * radius), 0.05);
                            verts.push(LineVertex { position: [p1.x, p1.y, p1.z], color: color_staged });
                            verts.push(LineVertex { position: [p2.x, p2.y, p2.z], color: color_staged });
                        }
                    }
                    ducad_kernel::Profile::Ellipse { center, radius_x, radius_y } => {
                        let segs = 32;
                        for i in 0..segs {
                            let a1 = i as f64 * std::f64::consts::TAU / segs as f64;
                            let a2 = (i + 1) as f64 * std::f64::consts::TAU / segs as f64;
                            let p1 = profile_plane.to_world(glam::DVec2::new(center.0 + a1.cos() * radius_x, center.1 + a1.sin() * radius_y), 0.05);
                            let p2 = profile_plane.to_world(glam::DVec2::new(center.0 + a2.cos() * radius_x, center.1 + a2.sin() * radius_y), 0.05);
                            verts.push(LineVertex { position: [p1.x, p1.y, p1.z], color: color_staged });
                            verts.push(LineVertex { position: [p2.x, p2.y, p2.z], color: color_staged });
                        }
                    }
                    ducad_kernel::Profile::Loop(segments) => {
                        for seg in segments {
                            match seg {
                                ducad_kernel::ProfileSegment::Line { start, end } => {
                                    let p1 = profile_plane.to_world(glam::DVec2::new(start.0, start.1), 0.05);
                                    let p2 = profile_plane.to_world(glam::DVec2::new(end.0, end.1), 0.05);
                                    verts.push(LineVertex { position: [p1.x, p1.y, p1.z], color: color_staged });
                                    verts.push(LineVertex { position: [p2.x, p2.y, p2.z], color: color_staged });
                                }
                                ducad_kernel::ProfileSegment::Arc { start, via, end } => {
                                    let s = glam::DVec2::new(start.0, start.1);
                                    let v = glam::DVec2::new(via.0, via.1);
                                    let e = glam::DVec2::new(end.0, end.1);
                                    if let Some(arc) = ducad_sketch::arc_from_three_points(s, v, e) {
                                        verts.extend(sketch_render::preview_lines(&arc, profile_plane));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Live 2D Pattern Ghost Preview
        if self.tool == ToolKind::Pattern && !self.selected.is_empty() {
            let entities: Vec<Entity> = self
                .selected
                .iter()
                .filter_map(|id| self.sketch().entities.get(*id).cloned())
                .collect();

            if !entities.is_empty() {
                let ghost_entities = match self.pattern_kind {
                    ducad_ui::PatternKind::Linear => {
                        ducad_sketch::linear_pattern_entities(
                            &entities,
                            self.pattern_count_x,
                            self.pattern_pitch_x,
                            self.pattern_count_y,
                            self.pattern_pitch_y,
                        )
                    }
                    ducad_ui::PatternKind::Circular => {
                        let pivot = self.pattern_custom_pivot_2d.unwrap_or(DVec2::ZERO);
                        let total_angle_rad = self.pattern_circ_angle_deg.to_radians();
                        ducad_sketch::circular_pattern_entities_with_radius(
                            &entities,
                            pivot,
                            self.pattern_circ_count,
                            total_angle_rad,
                            Some(self.pattern_circ_radius),
                        )
                    }
                };

                let ghost_color = [0.0, 0.85, 1.0, 0.75];
                for entity in ghost_entities {
                    match &entity {
                        Entity::Line { start, end, .. } => {
                            let p1 = self.active_plane.to_world(*start, 0.03);
                            let p2 = self.active_plane.to_world(*end, 0.03);
                            verts.push(LineVertex { position: [p1.x, p1.y, p1.z], color: ghost_color });
                            verts.push(LineVertex { position: [p2.x, p2.y, p2.z], color: ghost_color });
                        }
                        Entity::Circle { center, radius, .. } => {
                            let segs = 36;
                            for i in 0..segs {
                                let a1 = i as f64 * std::f64::consts::TAU / segs as f64;
                                let a2 = (i + 1) as f64 * std::f64::consts::TAU / segs as f64;
                                let p1 = self.active_plane.to_world(glam::DVec2::new(center.x + a1.cos() * radius, center.y + a1.sin() * radius), 0.03);
                                let p2 = self.active_plane.to_world(glam::DVec2::new(center.x + a2.cos() * radius, center.y + a2.sin() * radius), 0.03);
                                verts.push(LineVertex { position: [p1.x, p1.y, p1.z], color: ghost_color });
                                verts.push(LineVertex { position: [p2.x, p2.y, p2.z], color: ghost_color });
                            }
                        }
                        Entity::Arc { center, radius, start_angle, end_angle, .. } => {
                            let (p_start, p_via, p_end) = {
                                let tau = std::f64::consts::TAU;
                                let span = if end_angle - start_angle <= 0.0 { end_angle - start_angle + tau } else { end_angle - start_angle };
                                let mid = start_angle + span * 0.5;
                                (
                                    *center + DVec2::new(radius * start_angle.cos(), radius * start_angle.sin()),
                                    *center + DVec2::new(radius * mid.cos(), radius * mid.sin()),
                                    *center + DVec2::new(radius * end_angle.cos(), radius * end_angle.sin()),
                                )
                            };
                            if let Some(arc) = ducad_sketch::arc_from_three_points(p_start, p_via, p_end) {
                                verts.extend(sketch_render::preview_lines(&arc, &self.active_plane));
                            }
                        }
                        Entity::Ellipse { .. } | Entity::Spline { .. } => {
                            verts.extend(sketch_render::preview_lines(&entity, &self.active_plane));
                        }
                    }
                }

                // Render axes gizmo lines from centroid
                if let Some(centroid) = ducad_sketch::compute_entities_centroid(&entities) {
                    match self.pattern_kind {
                        ducad_ui::PatternKind::Linear => {
                            let origin_3d = self.active_plane.to_world(centroid, 0.04);
                            let target_x = centroid + DVec2::new(self.pattern_pitch_x, 0.0);
                            let target_y = centroid + DVec2::new(0.0, self.pattern_pitch_y);
                            let pt_x_3d = self.active_plane.to_world(target_x, 0.04);
                            let pt_y_3d = self.active_plane.to_world(target_y, 0.04);

                            let col_x = [1.0, 0.35, 0.35, 0.90];
                            let col_y = [0.35, 0.85, 0.35, 0.90];
                            verts.extend(sketch_render::dashed_line_3d([origin_3d.x, origin_3d.y, origin_3d.z], [pt_x_3d.x, pt_x_3d.y, pt_x_3d.z], 2.5, col_x));
                            verts.extend(sketch_render::dashed_line_3d([origin_3d.x, origin_3d.y, origin_3d.z], [pt_y_3d.x, pt_y_3d.y, pt_y_3d.z], 2.5, col_y));
                        }
                        ducad_ui::PatternKind::Circular => {
                            let pivot = self.pattern_custom_pivot_2d.unwrap_or(DVec2::ZERO);
                            let pivot_3d = self.active_plane.to_world(pivot, 0.04);
                            let centroid_3d = self.active_plane.to_world(centroid, 0.04);
                            let col_axis = [1.0, 0.75, 0.20, 0.85];
                            verts.extend(sketch_render::dashed_line_3d([pivot_3d.x, pivot_3d.y, pivot_3d.z], [centroid_3d.x, centroid_3d.y, centroid_3d.z], 2.0, col_axis));

                            // Orbit Ring Guide Circle
                            let r = self.pattern_circ_radius;
                            let segs = 64;
                            let col_ring = [0.0, 0.80, 1.0, 0.50];
                            for i in 0..segs {
                                let a1 = i as f64 * std::f64::consts::TAU / segs as f64;
                                let a2 = (i + 1) as f64 * std::f64::consts::TAU / segs as f64;
                                let p1 = self.active_plane.to_world(glam::DVec2::new(pivot.x + a1.cos() * r, pivot.y + a1.sin() * r), 0.02);
                                let p2 = self.active_plane.to_world(glam::DVec2::new(pivot.x + a2.cos() * r, pivot.y + a2.sin() * r), 0.02);
                                if i % 2 == 0 {
                                    verts.push(LineVertex { position: [p1.x, p1.y, p1.z], color: col_ring });
                                    verts.push(LineVertex { position: [p2.x, p2.y, p2.z], color: col_ring });
                                }
                            }
                        }
                    }
                }
            }
        }

        if self.tool == ToolKind::Select {
            if let Some(centroid) = self.selected_closed_region_centroid() {
                let c_base_pt = self.active_plane.to_world(centroid, 0.02);
                let c_base = [c_base_pt.x, c_base_pt.y, c_base_pt.z];

            if self.extruding_from_gizmo {
                let c_top_pt = self
                    .active_plane
                    .to_world(centroid, self.gizmo_distance as f32);
                let c_top = [c_top_pt.x, c_top_pt.y, c_top_pt.z];
                let line_color = if self.gizmo_is_cutting {
                    [1.0, 0.35, 0.25, 0.95]
                } else {
                    [0.15, 0.70, 1.0, 0.95]
                };
                verts.extend(sketch_render::dashed_line_3d(
                    c_base,
                    c_top,
                    4.0,
                    line_color,
                ));
            } else {
                let gizmo_pt = self.active_plane.to_world(centroid, 18.0);
                let gizmo_pos = [gizmo_pt.x, gizmo_pt.y, gizmo_pt.z];
                verts.extend(sketch_render::dashed_line_3d(
                    c_base,
                    gizmo_pos,
                    2.5,
                    [0.15, 0.70, 1.0, 0.75],
                ));
            }
        }
    }

        if let Some((p1, p2)) = self.selection_box {
            let min = p1.min(p2);
            let max = p1.max(p2);
            let c1 = self.active_plane.to_world(min, 0.05);
            let c2 = self.active_plane.to_world(glam::DVec2::new(max.x, min.y), 0.05);
            let c3 = self.active_plane.to_world(max, 0.05);
            let c4 = self.active_plane.to_world(glam::DVec2::new(min.x, max.y), 0.05);

            let p_c1 = [c1.x, c1.y, c1.z];
            let p_c2 = [c2.x, c2.y, c2.z];
            let p_c3 = [c3.x, c3.y, c3.z];
            let p_c4 = [c4.x, c4.y, c4.z];

            let color = [0.15, 0.65, 1.0, 0.90];
            verts.extend(sketch_render::dashed_line_3d(p_c1, p_c2, 2.0, color));
            verts.extend(sketch_render::dashed_line_3d(p_c2, p_c3, 2.0, color));
            verts.extend(sketch_render::dashed_line_3d(p_c3, p_c4, 2.0, color));
            verts.extend(sketch_render::dashed_line_3d(p_c4, p_c1, 2.0, color));
        }

        if self.sketch_move_dragging {
            if let Some(anchor) = self.sketch_move_anchor() {
                let current = anchor
                    + self.active_plane.u_axis * self.sketch_move_delta.x as f32
                    + self.active_plane.v_axis * self.sketch_move_delta.y as f32;
                let base = [anchor.x, anchor.y, anchor.z];
                let cur = [current.x, current.y, current.z];
                verts.extend(sketch_render::dashed_line_3d(
                    base,
                    cur,
                    2.5,
                    [1.0, 0.75, 0.0, 0.85],
                ));
            }
        }

        if let Some((_, center)) = self.selected_single_body_center() {
            if self.body_move_dragging {
                let current = center + self.body_move_delta;
                let base = [center.x, center.y, center.z];
                let cur = [current.x, current.y, current.z];
                verts.extend(sketch_render::dashed_line_3d(
                    base,
                    cur,
                    2.5,
                    [1.0, 0.75, 0.0, 0.85],
                ));
            }
        }

        if let Some((active_id, _, hit)) = &self.active_face {
            if self.model.doc.bodies.get(*active_id).is_some_and(|b| b.visible) {
                // Outline garis kontur sekeliling face yang aktif
                if hit.boundary_points.len() >= 2 {
                    let outline_color = [0.0, 0.90, 1.0, 0.95];
                    for i in 0..hit.boundary_points.len() {
                        let j = (i + 1) % hit.boundary_points.len();
                        let p1 = hit.boundary_points[i];
                        let p2 = hit.boundary_points[j];
                        verts.push(LineVertex {
                            position: [p1.0 as f32, p1.1 as f32, p1.2 as f32],
                            color: outline_color,
                        });
                        verts.push(LineVertex {
                            position: [p2.0 as f32, p2.1 as f32, p2.2 as f32],
                            color: outline_color,
                        });
                    }
                }

                let anchor = hit.gizmo_anchor();
                let c_base = [anchor.0 as f32, anchor.1 as f32, anchor.2 as f32];
                let pull_dir = Vec3::new(
                    hit.pull_dir.0 as f32,
                    hit.pull_dir.1 as f32,
                    hit.pull_dir.2 as f32,
                );

                if self.tool == ToolKind::HoleWizard {
                    let (hole_pos, u_axis, v_axis, _normal) =
                        self.compute_active_hole_position_and_basis(hit);
                    let outer_radius = match self.hole_popup_state.spec.kind {
                        ducad_core::hole::HoleKind::Counterbore => {
                            self.hole_popup_state.spec.counterbore_diameter / 2.0
                        }
                        ducad_core::hole::HoleKind::Countersink => {
                            self.hole_popup_state.spec.countersink_diameter / 2.0
                        }
                        ducad_core::hole::HoleKind::Tapped => {
                            self.hole_popup_state.spec.thread_size.standard_params().0 / 2.0
                        }
                        ducad_core::hole::HoleKind::Simple => {
                            self.hole_popup_state.spec.diameter / 2.0
                        }
                    } as f32;
                    let inner_radius = (self.hole_popup_state.spec.diameter / 2.0) as f32;

                    // 1. Lingkaran preview lubang luar (Merah menyala)
                    const HOLE_RING_COLOR: [f32; 4] = [1.0, 0.12, 0.12, 1.0]; // Merah Menyala
                    const SEGMENTS: usize = 36;
                    for k in 0..SEGMENTS {
                        let a1 = (k as f32) * std::f32::consts::TAU / (SEGMENTS as f32);
                        let a2 = ((k + 1) as f32) * std::f32::consts::TAU / (SEGMENTS as f32);
                        let p1 = hole_pos + (u_axis * a1.cos() + v_axis * a1.sin()) * outer_radius;
                        let p2 = hole_pos + (u_axis * a2.cos() + v_axis * a2.sin()) * outer_radius;
                        verts.push(LineVertex {
                            position: [p1.x, p1.y, p1.z],
                            color: HOLE_RING_COLOR,
                        });
                        verts.push(LineVertex {
                            position: [p2.x, p2.y, p2.z],
                            color: HOLE_RING_COLOR,
                        });
                    }

                    // 2. Lingkaran dalam (jika bertingkat / counterbore / countersink / tapped)
                    if (outer_radius - inner_radius).abs() > 0.05 {
                        const INNER_RING_COLOR: [f32; 4] = [0.95, 0.28, 0.28, 0.95];
                        for k in 0..SEGMENTS {
                            let a1 = (k as f32) * std::f32::consts::TAU / (SEGMENTS as f32);
                            let a2 = ((k + 1) as f32) * std::f32::consts::TAU / (SEGMENTS as f32);
                            let p1 = hole_pos + (u_axis * a1.cos() + v_axis * a1.sin()) * inner_radius;
                            let p2 = hole_pos + (u_axis * a2.cos() + v_axis * a2.sin()) * inner_radius;
                            verts.push(LineVertex {
                                position: [p1.x, p1.y, p1.z],
                                color: INNER_RING_COLOR,
                            });
                            verts.push(LineVertex {
                                position: [p2.x, p2.y, p2.z],
                                color: INNER_RING_COLOR,
                            });
                        }
                    }

                    // 3. Crosshair target pada titik pusat lubang (Merah Menyala)
                    let cross_len = (outer_radius + 4.5).max(7.0);
                    const CROSS_COLOR: [f32; 4] = [1.0, 0.10, 0.10, 1.0]; // Merah Menyala
                    let cu1 = hole_pos - u_axis * cross_len;
                    let cu2 = hole_pos + u_axis * cross_len;
                    let cv1 = hole_pos - v_axis * cross_len;
                    let cv2 = hole_pos + v_axis * cross_len;
                    verts.push(LineVertex {
                        position: [cu1.x, cu1.y, cu1.z],
                        color: CROSS_COLOR,
                    });
                    verts.push(LineVertex {
                        position: [cu2.x, cu2.y, cu2.z],
                        color: CROSS_COLOR,
                    });
                    verts.push(LineVertex {
                        position: [cv1.x, cv1.y, cv1.z],
                        color: CROSS_COLOR,
                    });
                    verts.push(LineVertex {
                        position: [cv2.x, cv2.y, cv2.z],
                        color: CROSS_COLOR,
                    });

                    // 4. Ruler bantu ke tepi-tepi terdekat (Garis Putus-Putus Merah & Titik Hitam)
                    let closest_edges = self.compute_hole_closest_edges(hit, hole_pos);
                    const RULER_LINE_COLOR: [f32; 4] = [1.0, 0.18, 0.18, 0.95]; // Merah Menyala
                    for (q, dist, _, _) in &closest_edges {
                        if *dist > 0.1 {
                            verts.extend(sketch_render::dashed_line_3d(
                                [hole_pos.x, hole_pos.y, hole_pos.z],
                                [q.x, q.y, q.z],
                                3.0,
                                RULER_LINE_COLOR,
                            ));
                            let tick_dir = (hole_pos - *q).normalize_or_zero();
                            let tick_p1 = *q + tick_dir * 1.5;
                            let tick_p2 = *q - tick_dir * 1.5;
                            verts.push(LineVertex {
                                position: [tick_p1.x, tick_p1.y, tick_p1.z],
                                color: [0.08, 0.08, 0.10, 1.0], // Hitam
                            });
                            verts.push(LineVertex {
                                position: [tick_p2.x, tick_p2.y, tick_p2.z],
                                color: [0.08, 0.08, 0.10, 1.0], // Hitam
                            });
                        }
                    }
                } else if self.extruding_face_from_gizmo {
                    let dist = self.face_gizmo_distance as f32;
                    let c_top = Vec3::from(c_base) + pull_dir * dist;
                    let c_top_arr = [c_top.x, c_top.y, c_top.z];
                    let line_color = if self.face_gizmo_distance < 0.0 {
                        [1.0, 0.35, 0.25, 0.95]
                    } else {
                        [0.15, 0.80, 1.0, 0.95]
                    };
                    verts.extend(sketch_render::dashed_line_3d(
                        c_base,
                        c_top_arr,
                        4.0,
                        line_color,
                    ));
                } else {
                    let gizmo_pt = Vec3::from(c_base) + pull_dir * 18.0;
                    let gizmo_pos = [gizmo_pt.x, gizmo_pt.y, gizmo_pt.z];
                    verts.extend(sketch_render::dashed_line_3d(
                        c_base,
                        gizmo_pos,
                        2.5,
                        [0.15, 0.80, 1.0, 0.85],
                    ));
                }
            }
        }

        if let Some((vertex, out_dir)) = self.active_vertex_gizmo_dir() {
            const VERTEX_GIZMO_COLOR: [f32; 4] = [1.0, 0.35, 0.85, 1.0];
            let handle_dist = if self.filleting_vertex_from_gizmo {
                self.vertex_gizmo_radius.max(0.1) as f32
            } else {
                12.0
            };
            verts.extend(sketch_render::vertex_fillet_marker_lines(
                [vertex.x, vertex.y, vertex.z],
                out_dir,
                handle_dist,
                VERTEX_GIZMO_COLOR,
            ));
        }

        if let Some((point, out_dir)) = self.active_edge_gizmo_dir() {
            const EDGE_ROUND_GIZMO_COLOR: [f32; 4] = [1.0, 0.35, 0.85, 1.0];
            let handle_dist = if self.filleting_edge_from_gizmo {
                self.edge_gizmo_radius.max(0.1) as f32
            } else {
                12.0
            };
            verts.extend(sketch_render::vertex_fillet_marker_lines(
                [point.x, point.y, point.z],
                out_dir,
                handle_dist,
                EDGE_ROUND_GIZMO_COLOR,
            ));
        }

        if !self.is_sketching {
            const VERTEX_MARKER_COLOR: [f32; 4] = [0.85, 0.85, 0.92, 0.55];
            const VERTEX_MARKER_HOVER_COLOR: [f32; 4] = [1.0, 0.85, 0.15, 1.0];
            for (id, geo) in self.model.geometry.iter() {
                let visible = self.model.doc.bodies.get(id).is_some_and(|b| b.visible);
                if !visible {
                    continue;
                }
                let vertices: Vec<[f32; 3]> = ducad_kernel::shape_vertices(&geo.shape)
                    .into_iter()
                    .map(|(x, y, z)| [x as f32, y as f32, z as f32])
                    .collect();
                let hover_point = self
                    .hovered_vertex_marker
                    .and_then(|(hid, hv)| (hid == id).then_some([hv.0 as f32, hv.1 as f32, hv.2 as f32]));
                verts.extend(sketch_render::vertex_dot_markers(
                    &vertices,
                    hover_point,
                    VERTEX_MARKER_COLOR,
                    VERTEX_MARKER_HOVER_COLOR,
                ));
            }
        }

        for measurement in &self.measurements {
            let pts = measurement.points();
            verts.extend(sketch_render::measurement_lines(&pts, &self.active_plane));
            verts.extend(sketch_render::measurement_arrowheads(
                &pts,
                &self.active_plane,
            ));
        }

        const EDGE_PICK_COLOR: [f32; 4] = [1.0, 0.55, 0.15, 1.0];
        for picked in &self.selected_edges {
            for pair in picked.polyline.windows(2) {
                verts.push(LineVertex {
                    position: [pair[0].0 as f32, pair[0].1 as f32, pair[0].2 as f32],
                    color: EDGE_PICK_COLOR,
                });
                verts.push(LineVertex {
                    position: [pair[1].0 as f32, pair[1].1 as f32, pair[1].2 as f32],
                    color: EDGE_PICK_COLOR,
                });
            }
        }

        if self.tool == ToolKind::Offset {
            if let Some(entity) = self
                .offset_source
                .and_then(|id| self.sketch().entities.get(id))
            {
                verts.extend(sketch_render::preview_lines(entity, &self.active_plane));
            }
        }

        if matches!(
            self.tool,
            ToolKind::CoincidentPick | ToolKind::SymmetricPick
        ) {
            for pr in &self.pending_point_refs {
                if let Some(p) =
                    ducad_sketch::constraint::point_ref_position(self.sketch(), pr)
                {
                    verts.extend(sketch_render::picked_point_glyph(p, &self.active_plane));
                }
            }
        }

        if let Some((_, _, corner_2d)) = self.active_sketch_corner {
            verts.extend(sketch_render::picked_point_glyph(corner_2d, &self.active_plane));
        } else if let Some(corner_2d) = self.hovered_corner_2d {
            verts.extend(sketch_render::picked_point_glyph(corner_2d, &self.active_plane));
        }

        if let Some((id1, id2, _corner_2d)) = self.active_sketch_corner {
            let r = self.sketch_corner_gizmo_radius;
            let (e1_opt, e2_opt) = (
                self.sketch().entities.get(id1),
                self.sketch().entities.get(id2),
            );
            if let (
                Some(Entity::Line { start: s1, end: e1, .. }),
                Some(Entity::Line { start: s2, end: e2, .. }),
            ) = (e1_opt, e2_opt)
            {
                if let Some(arc_id) = self.active_sketch_fillet_arc {
                    if let Some(Entity::Arc { center, start_angle, .. }) =
                        self.sketch().entities.get(arc_id)
                    {
                        let ap1 = *center + glam::DVec2::new(start_angle.cos(), start_angle.sin());
                        let (n1, f1) = if (*s1 - ap1).length() < (*e1 - ap1).length() { (*s1, *e1) } else { (*e1, *s1) };
                        let (n2, f2) = if (*s2 - ap1).length() < (*e2 - ap1).length() { (*s2, *e2) } else { (*e2, *s2) };
                        let d1 = n1 - f1;
                        let d2 = n2 - f2;
                        let det = d1.x * d2.y - d1.y * d2.x;
                        if det.abs() > 1e-6 {
                            let t = ((f2 - f1).x * d2.y - (f2 - f1).y * d2.x) / det;
                            let apex = f1 + d1 * t;
                            if r > 0.1 {
                                if let Some(res) = ducad_sketch::compute_fillet_2d((f1, apex), (f2, apex), r) {
                                    verts.extend(sketch_render::preview_lines(&res.arc, &self.active_plane));
                                }
                            }
                        }
                    }
                } else if r > 0.1 {
                    if let Some(res) = ducad_sketch::compute_fillet_2d((*s1, *e1), (*s2, *e2), r) {
                        verts.extend(sketch_render::preview_lines(&res.arc, &self.active_plane));
                    }
                } else if r < -0.1 {
                    let d = -r;
                    if let Some(res) = ducad_sketch::compute_chamfer_2d((*s1, *e1), (*s2, *e2), d, d) {
                        verts.extend(sketch_render::preview_lines(&res.bevel_line, &self.active_plane));
                    }
                }
            }
        }

        if let Some(raw) = raw_cursor {
            let offset_dist = (14.0 * world_scale).max(8.0);
            match self.tool {
                ToolKind::Line if self.pending_points.len() == 1 => {
                    let start = self.pending_points[0];
                    let end = self.snapped_or(raw);
                    let preview = Entity::line(start, end).with_construction(self.construction_mode);
                    verts.extend(sketch_render::preview_lines(&preview, &self.active_plane));
                    verts.extend(sketch_render::dimension_leader_lines(
                        start,
                        end,
                        offset_dist,
                        &self.active_plane,
                    ));
                }
                ToolKind::Rectangle if self.pending_points.len() == 1 => {
                    let first = self.pending_points[0];
                    let effective = self.snapped_or(raw);
                    let min = first.min(effective);
                    let max = first.max(effective);
                    let corners = [
                        DVec2::new(min.x, min.y),
                        DVec2::new(max.x, min.y),
                        DVec2::new(max.x, max.y),
                        DVec2::new(min.x, max.y),
                    ];
                    for i in 0..4 {
                        let preview = Entity::line(
                            corners[i],
                            corners[(i + 1) % 4],
                        ).with_construction(self.construction_mode);
                        verts.extend(sketch_render::preview_lines(&preview, &self.active_plane));
                    }
                    verts.extend(sketch_render::dimension_leader_lines(
                        corners[0],
                        corners[1],
                        offset_dist,
                        &self.active_plane,
                    ));
                    verts.extend(sketch_render::dimension_leader_lines(
                        corners[1],
                        corners[2],
                        offset_dist,
                        &self.active_plane,
                    ));
                }
                ToolKind::Circle if self.pending_points.len() == 1 => {
                    let first = self.pending_points[0];
                    let effective = self.snapped_or(raw);
                    let radius = (effective - first).length();
                    let preview = Entity::circle(
                        first,
                        radius,
                    ).with_construction(self.construction_mode);
                    verts.extend(sketch_render::preview_lines(&preview, &self.active_plane));
                    verts.extend(sketch_render::dimension_leader_lines(
                        first,
                        effective,
                        offset_dist * 0.5,
                        &self.active_plane,
                    ));
                }
                ToolKind::Ellipse if self.pending_points.len() == 1 => {
                    let first = self.pending_points[0];
                    let effective = self.snapped_or(raw);
                    let radius_x = (effective.x - first.x).abs();
                    let radius_y = (effective.y - first.y).abs();
                    if radius_x > 1e-6 && radius_y > 1e-6 {
                        let preview = Entity::ellipse(
                            first,
                            radius_x,
                            radius_y,
                        ).with_construction(self.construction_mode);
                        verts.extend(sketch_render::preview_lines(&preview, &self.active_plane));
                    }
                }
                ToolKind::Polygon if self.pending_points.len() == 1 => {
                    let first = self.pending_points[0];
                    let effective = self.snapped_or(raw);
                    if let Some(entities) = ducad_sketch::regular_polygon_entities(
                        first,
                        effective,
                        self.polygon_sides,
                        self.polygon_mode,
                        self.construction_mode,
                    ) {
                        for e in &entities {
                            verts.extend(sketch_render::preview_lines(e, &self.active_plane));
                        }

                        // Reference dashed guide circle and radius line
                        let radius = (effective - first).length();
                        let guide_circle = Entity::circle(first, radius).with_construction(true);
                        verts.extend(sketch_render::preview_lines(&guide_circle, &self.active_plane));

                        let guide_line = Entity::line(first, effective).with_construction(true);
                        verts.extend(sketch_render::preview_lines(&guide_line, &self.active_plane));

                        verts.extend(sketch_render::dimension_leader_lines(
                            first,
                            effective,
                            offset_dist * 0.5,
                            &self.active_plane,
                        ));
                    }
                }
                ToolKind::Slot => {
                    let effective = self.snapped_or(raw);
                    match self.pending_points.len() {
                        1 => {
                            let first = self.pending_points[0];
                            let preview = Entity::line(first, effective).with_construction(self.construction_mode);
                            verts.extend(sketch_render::preview_lines(&preview, &self.active_plane));
                            verts.extend(sketch_render::dimension_leader_lines(
                                first,
                                effective,
                                offset_dist * 0.5,
                                &self.active_plane,
                            ));
                        }
                        2 => {
                            let p1 = self.pending_points[0];
                            let p2 = self.pending_points[1];
                            if let Some(entities) = ducad_sketch::slot_from_points(
                                p1,
                                p2,
                                effective,
                                self.slot_mode,
                                self.construction_mode,
                            ) {
                                for e in &entities {
                                    verts.extend(sketch_render::preview_lines(e, &self.active_plane));
                                }

                                // Dashed centerline axis
                                let center_axis = Entity::line(p1, p2).with_construction(true);
                                verts.extend(sketch_render::preview_lines(&center_axis, &self.active_plane));

                                // Leader line for radius / width
                                let axis_dir = (p2 - p1).normalize_or_zero();
                                if axis_dir != glam::DVec2::ZERO {
                                    let proj = p1 + axis_dir * (effective - p1).dot(axis_dir);
                                    let perp_line = Entity::line(proj, effective).with_construction(true);
                                    verts.extend(sketch_render::preview_lines(&perp_line, &self.active_plane));
                                }
                            }
                        }
                        _ => {}
                    }
                }
                ToolKind::Spline => {
                    let effective = self.snapped_or(raw);
                    let mut pts = self.pending_points.clone();
                    pts.push(effective);
                    if pts.len() >= 2 {
                        let preview = Entity::spline(pts).with_construction(self.construction_mode);
                        verts.extend(sketch_render::preview_lines(&preview, &self.active_plane));
                    }
                }
                ToolKind::Arc => {
                    let effective = self.snapped_or(raw);
                    match self.pending_points.len() {
                        1 => {
                            let preview = Entity::line(
                                self.pending_points[0],
                                effective,
                            ).with_construction(self.construction_mode);
                            verts.extend(sketch_render::preview_lines(
                                &preview,
                                &self.active_plane,
                            ));
                            verts.extend(sketch_render::dimension_leader_lines(
                                self.pending_points[0],
                                effective,
                                offset_dist,
                                &self.active_plane,
                            ));
                        }
                        2 => {
                            if let Some(mut preview) = arc_from_three_points(
                                self.pending_points[0],
                                self.pending_points[1],
                                effective,
                            ) {
                                preview.set_construction(self.construction_mode);
                                verts.extend(sketch_render::preview_lines(
                                    &preview,
                                    &self.active_plane,
                                ));
                            }
                        }
                        _ => {}
                    }
                }
                ToolKind::Mirror
                    if !self.selected.is_empty() && self.pending_points.len() == 1 =>
                {
                    let axis_a = self.pending_points[0];
                    let axis_b = self.snapped_or(raw);
                    let axis_preview = Entity::line(
                        axis_a,
                        axis_b,
                    );
                    verts.extend(sketch_render::preview_lines(
                        &axis_preview,
                        &self.active_plane,
                    ));
                    for entity in self
                        .selected
                        .iter()
                        .filter_map(|id| self.sketch().entities.get(*id))
                    {
                        if let Some(mirrored) = mirror_entity(entity, axis_a, axis_b) {
                            verts.extend(sketch_render::preview_lines(
                                &mirrored,
                                &self.active_plane,
                            ));
                        }
                    }
                }
                ToolKind::Revolve => {
                    let has_target = !self.selected.is_empty() || self.active_face.is_some();
                    if has_target && self.pending_points.len() == 1 {
                        let axis_preview = Entity::line(
                            self.pending_points[0],
                            self.snapped_or(raw),
                        );
                        verts.extend(sketch_render::preview_lines(
                            &axis_preview,
                            &self.active_plane,
                        ));
                    } else if let Some((start, end)) = self.revolve_staged_axis {
                        let axis_preview = Entity::line(start, end);
                        verts.extend(sketch_render::preview_lines(
                            &axis_preview,
                            &self.active_plane,
                        ));
                    }
                }
                ToolKind::Offset => {
                    if let Some(entity) = self
                        .offset_source
                        .and_then(|id| self.sketch().entities.get(id))
                    {
                        if let Some(preview) = offset_entity(entity, raw) {
                            verts.extend(sketch_render::preview_lines(
                                &preview,
                                &self.active_plane,
                            ));
                        }
                    }
                }
                ToolKind::Trim => {
                    if let Some(id) = self.hovered {
                        if let Some((a, b)) = trim_removal_preview(self.sketch(), id, raw) {
                            verts.extend(sketch_render::removal_preview_lines(
                                a,
                                b,
                                &self.active_plane,
                            ));
                        }
                    }
                }
                ToolKind::Fillet2D => {
                    let tol = 12.0 * world_scale;
                    if let Some(res) = fillet_2d_preview(
                        self.sketch(),
                        raw,
                        self.fillet_2d_radius,
                        tol,
                        self.fillet_chamfer_first_line,
                    ) {
                        verts.extend(sketch_render::preview_lines(
                            &res.arc,
                            &self.active_plane,
                        ));
                    }
                }
                ToolKind::Chamfer2D => {
                    let tol = 12.0 * world_scale;
                    if let Some(res) = chamfer_2d_preview(
                        self.sketch(),
                        raw,
                        self.chamfer_2d_dist,
                        tol,
                        self.fillet_chamfer_first_line,
                    ) {
                        verts.extend(sketch_render::preview_lines(
                            &res.bevel_line,
                            &self.active_plane,
                        ));
                    }
                }
                ToolKind::Text => {
                    let origin = self.pending_points.first().copied().unwrap_or_else(|| self.snapped_or(raw));
                    let options = ducad_sketch::TextOptions {
                        font_height_mm: self.text_popup_state.font_height_mm,
                        letter_spacing: self.text_popup_state.letter_spacing,
                        line_spacing: self.text_popup_state.line_spacing,
                        align: self.text_popup_state.align,
                        font_preset: self.text_popup_state.font_preset,
                        is_construction: self.construction_mode,
                    };
                    if let Ok(entities) = ducad_sketch::text_to_entities(
                        &self.text_popup_state.text,
                        origin,
                        &options,
                        self.custom_font_bytes.as_deref(),
                    ) {
                        for e in &entities {
                            verts.extend(sketch_render::preview_lines(e, &self.active_plane));
                        }
                    }
                }
                ToolKind::Measure | ToolKind::MeasureAngle => {
                    let effective = self.snapped_or(raw);
                    let mut preview_points = self.pending_points.clone();
                    preview_points.push(effective);
                    verts.extend(sketch_render::measurement_lines(
                        &preview_points,
                        &self.active_plane,
                    ));
                    verts.extend(sketch_render::measurement_arrowheads(
                        &preview_points,
                        &self.active_plane,
                    ));
                }
                _ => {}
            }
        }

        let show_candidate_points = self.sketch_move_dragging
            || self.tool == ToolKind::Pattern;

        if show_candidate_points {
            let exclude_set = if self.sketch_move_dragging {
                self.sketch_move_target.as_ref()
            } else {
                None
            };
            let mut candidates = all_snap_candidate_points_with_exclude_set(
                self.sketch(),
                exclude_set,
            );

            // Filter kandidat berdasarkan kedekatan dengan kursor jika ada banyak entitas
            if let Some(cursor) = raw_cursor {
                let max_snap_dist = (35.0 * world_scale).max(15.0);
                candidates.retain(|(pt, _)| (*pt - cursor).length() <= max_snap_dist);
                if candidates.len() > 30 {
                    candidates.sort_by(|(a, _), (b, _)| {
                        (*a - cursor).length_squared().partial_cmp(&(*b - cursor).length_squared()).unwrap()
                    });
                    candidates.truncate(30);
                }
            } else if candidates.len() > 40 {
                candidates.truncate(40);
            }

            verts.extend(sketch_render::candidate_snap_points_glyphs(
                &candidates,
                &self.active_plane,
            ));
        }

        if let Some(hit) = &self.last_snap {
            verts.extend(sketch_render::snap_glyph(hit, &self.active_plane));
        }

        // =========================================================================
        // PULL DIRECTION 3D AXIS LINE & RING (Draft Angle Heatmap Inspector)
        // =========================================================================
        if self.draft_config.enabled {
            let (center, size) = self.draft_inspection_anchor_and_size();
            let pull_dir = Vec3::new(
                self.draft_config.pull_dir[0],
                self.draft_config.pull_dir[1],
                self.draft_config.pull_dir[2],
            )
            .normalize_or_zero();

            if pull_dir != Vec3::ZERO {
                let half_line_len = (size * 0.35).clamp(12.0, 40.0);
                let p_start = center - pull_dir * half_line_len;
                let p_end = center + pull_dir * half_line_len;
                // Warna Electric Royal Blue (bukan merah, kuning, atau hijau)
                const AXIS_COLOR: [f32; 4] = [0.05, 0.58, 1.0, 0.75];
                verts.push(LineVertex {
                    position: [p_start.x, p_start.y, p_start.z],
                    color: AXIS_COLOR,
                });
                verts.push(LineVertex {
                    position: [p_end.x, p_end.y, p_end.z],
                    color: AXIS_COLOR,
                });

                // Lingkaran pembagi (parting plane ring) di pusat benda tegak lurus sumbu pull
                let (t1, t2) = if pull_dir.z.abs() < 0.95 {
                    let t1 = pull_dir.cross(Vec3::Z).normalize();
                    let t2 = pull_dir.cross(t1).normalize();
                    (t1, t2)
                } else {
                    let t1 = pull_dir.cross(Vec3::Y).normalize();
                    let t2 = pull_dir.cross(t1).normalize();
                    (t1, t2)
                };
                let ring_radius = (size * 0.12).clamp(6.0, 20.0);
                let ring_segs = 20;
                for i in 0..ring_segs {
                    let a0 = std::f32::consts::TAU * (i as f32 / ring_segs as f32);
                    let a1 = std::f32::consts::TAU * ((i + 1) as f32 / ring_segs as f32);
                    let pt0 =
                        center + t1 * (ring_radius * a0.cos()) + t2 * (ring_radius * a0.sin());
                    let pt1 =
                        center + t1 * (ring_radius * a1.cos()) + t2 * (ring_radius * a1.sin());
                    verts.push(LineVertex {
                        position: [pt0.x, pt0.y, pt0.z],
                        color: [0.05, 0.58, 1.0, 0.55],
                    });
                    verts.push(LineVertex {
                        position: [pt1.x, pt1.y, pt1.z],
                        color: [0.05, 0.58, 1.0, 0.55],
                    });
                }
            }
        }

        // Indikator garis pembatas halus (bounding box wireframe) saat CMF drawer aktif
        if self.cmf_drawer_open && !self.selected_bodies.is_empty() {
            for &bid in &self.selected_bodies {
                if let Some(geo) = self.model.geometry.get(bid) {
                    if let Some((min, max)) = geo.mesh.bounding_box() {
                        let col = [0.0, 0.75, 1.0, 0.75];
                        let p000 = [min[0], min[1], min[2]];
                        let p100 = [max[0], min[1], min[2]];
                        let p010 = [min[0], max[1], min[2]];
                        let p110 = [max[0], max[1], min[2]];
                        let p001 = [min[0], min[1], max[2]];
                        let p101 = [max[0], min[1], max[2]];
                        let p011 = [min[0], max[1], max[2]];
                        let p111 = [max[0], max[1], max[2]];

                        let edges = [
                            (p000, p100), (p100, p110), (p110, p010), (p010, p000),
                            (p001, p101), (p101, p111), (p111, p011), (p011, p001),
                            (p000, p001), (p100, p101), (p110, p111), (p010, p011),
                        ];
                        for (a, b) in edges {
                            verts.extend(sketch_render::dashed_line_3d(a, b, 4.0, col));
                        }
                    }
                }
            }
        }

        verts
    }

    /// Hitung titik pusat dan dimensi perkiraan model 3D (atau body yang dipilih) untuk penempatan visualizer pull direction.
    pub fn draft_inspection_anchor_and_size(&self) -> (Vec3, f32) {
        if !self.selected_bodies.is_empty() {
            let mut min = Vec3::splat(f32::INFINITY);
            let mut max = Vec3::splat(f32::NEG_INFINITY);
            let mut found = false;
            for &bid in &self.selected_bodies {
                if let Some(geo) = self.model.geometry.get(bid) {
                    for p in &geo.mesh.positions {
                        let v = Vec3::new(p[0], p[1], p[2]);
                        min = min.min(v);
                        max = max.max(v);
                        found = true;
                    }
                }
            }
            if found {
                let center = (min + max) * 0.5;
                let size = (max - min).length().max(20.0);
                return (center, size);
            }
        }

        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        let mut found = false;
        for (bid, b) in self.model.doc.bodies.iter() {
            if b.visible {
                if let Some(geo) = self.model.geometry.get(bid) {
                    for p in &geo.mesh.positions {
                        let v = Vec3::new(p[0], p[1], p[2]);
                        min = min.min(v);
                        max = max.max(v);
                        found = true;
                    }
                }
            }
        }
        if found {
            let center = (min + max) * 0.5;
            let size = (max - min).length().max(20.0);
            (center, size)
        } else {
            (Vec3::ZERO, 50.0)
        }
    }

    #[allow(clippy::type_complexity)]
    pub fn build_gizmo_mesh(
        &self,
        world_scale: f64,
    ) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 4]>, Vec<u32>) {
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut colors = Vec::new();
        let mut indices = Vec::new();

        const ARROW_PX: f32 = 6.5;
        const HEIGHT_PX: f32 = 30.0;
        let arrow_size = (ARROW_PX as f64 * world_scale) as f32;
        let height = (HEIGHT_PX as f64 * world_scale) as f32;

        let push_mesh = |positions: &mut Vec<[f32; 3]>,
                         normals: &mut Vec<[f32; 3]>,
                         colors: &mut Vec<[f32; 4]>,
                         indices: &mut Vec<u32>,
                         center: [f32; 3],
                         color: [f32; 4],
                         dir: Vec3| {
            let (p, n, c, i) = sketch_render::solid_double_arrow_gizmo_mesh(
                center, height, arrow_size, color, dir,
            );
            let base = positions.len() as u32;
            positions.extend(p);
            normals.extend(n);
            colors.extend(c);
            indices.extend(i.into_iter().map(|idx| idx + base));
        };

        const GIZMO_ARROW_COLOR: [f32; 4] = [0.0, 0.78, 1.0, 1.0];
        const FACE_GIZMO_COLOR: [f32; 4] = [0.0, 0.85, 1.0, 1.0];
        // Pink = fillet (ditarik, membulat), kuning = chamfer (didorong,
        // potong lurus) — warna beda supaya arah tarik/dorong kelihatan
        // langsung dari ikonnya, bukan cuma dari bentuk sudut yang berubah.
        const FILLET_GIZMO_COLOR: [f32; 4] = [1.0, 0.35, 0.85, 1.0];
        const CHAMFER_GIZMO_COLOR: [f32; 4] = [1.0, 0.75, 0.1, 1.0];

        if let Some(centroid) = self.selected_closed_region_centroid() {
            let z = if self.extruding_from_gizmo {
                self.gizmo_distance as f32
            } else {
                18.0
            };
            let p = self.active_plane.to_world(centroid, z);
            let arrow_color = if self.extruding_from_gizmo && self.gizmo_is_cutting {
                [1.0, 0.35, 0.25, 1.0]
            } else {
                GIZMO_ARROW_COLOR
            };
            push_mesh(
                &mut positions,
                &mut normals,
                &mut colors,
                &mut indices,
                [p.x, p.y, p.z],
                arrow_color,
                self.active_plane.normal,
            );
        }

        if let Some((active_id, _, hit)) = &self.active_face {
            if self.model.doc.bodies.get(*active_id).is_some_and(|b| b.visible) {
                let anchor = hit.gizmo_anchor();
                let c_base = Vec3::new(anchor.0 as f32, anchor.1 as f32, anchor.2 as f32);
                let pull_dir = Vec3::new(
                    hit.pull_dir.0 as f32,
                    hit.pull_dir.1 as f32,
                    hit.pull_dir.2 as f32,
                );
                let dist = if self.extruding_face_from_gizmo {
                    self.face_gizmo_distance as f32
                } else {
                    18.0
                };
                let p = c_base + pull_dir * dist;
                let arrow_color = if self.extruding_face_from_gizmo && self.face_gizmo_distance < 0.0 {
                    [1.0, 0.35, 0.25, 1.0]
                } else {
                    FACE_GIZMO_COLOR
                };
                push_mesh(
                    &mut positions,
                    &mut normals,
                    &mut colors,
                    &mut indices,
                    [p.x, p.y, p.z],
                    arrow_color,
                    pull_dir,
                );
            }
        }

        if let Some((c_base, pull_dir)) = self.active_vertex_gizmo_dir() {
            let dist = if self.filleting_vertex_from_gizmo {
                self.vertex_gizmo_radius.abs().max(0.1) as f32
            } else {
                12.0
            };
            let color = if self.vertex_gizmo_radius < 0.0 {
                CHAMFER_GIZMO_COLOR
            } else {
                FILLET_GIZMO_COLOR
            };
            let p = c_base + pull_dir * dist;
            push_mesh(
                &mut positions,
                &mut normals,
                &mut colors,
                &mut indices,
                [p.x, p.y, p.z],
                color,
                pull_dir,
            );
        }

        if let Some((c_base, pull_dir)) = self.active_edge_gizmo_dir() {
            let dist = if self.filleting_edge_from_gizmo {
                self.edge_gizmo_radius.abs().max(0.1) as f32
            } else {
                12.0
            };
            let color = if self.edge_gizmo_radius < 0.0 {
                CHAMFER_GIZMO_COLOR
            } else {
                FILLET_GIZMO_COLOR
            };
            let p = c_base + pull_dir * dist;
            push_mesh(
                &mut positions,
                &mut normals,
                &mut colors,
                &mut indices,
                [p.x, p.y, p.z],
                color,
                pull_dir,
            );
        }

        if let Some((_, center)) = self.selected_single_body_center() {
            if !self.feature_pick_active() {
                let gizmo_scale = (55.0 * world_scale) as f32;
                // Geser gizmo ke atas di world-space agar tidak tumpang-tindih dengan badan benda.
                // Offset sebesar ~25% dari gizmo_scale ke arah Z global (atas).
                let gizmo_center = center + glam::Vec3::Z * (gizmo_scale * 0.25);
                let eye = self.camera.eye();
                let (gp, gn, gc, gi) = sketch_render::solid_shapr3d_transform_gizmo_mesh(
                    [gizmo_center.x, gizmo_center.y, gizmo_center.z],
                    gizmo_scale,
                    self.body_transform_part,
                    Some(eye),
                );
                let base = positions.len() as u32;
                positions.extend(gp);
                normals.extend(gn);
                colors.extend(gc);
                indices.extend(gi.into_iter().map(|idx| idx + base));
            }
        }

        // =========================================================================
        // SOLID 3D ARROW PULL DIRECTION (Draft Angle Heatmap Inspector)
        // =========================================================================
        if self.draft_config.enabled {
            let (center, _size) = self.draft_inspection_anchor_and_size();
            let pull_dir = Vec3::new(
                self.draft_config.pull_dir[0],
                self.draft_config.pull_dir[1],
                self.draft_config.pull_dir[2],
            )
            .normalize_or_zero();

            if pull_dir != Vec3::ZERO {
                // Panjang panah yang kompak dan pendek (22mm - 34mm)
                let arrow_length = (22.0 * world_scale as f32).clamp(16.0, 32.0);
                let arrow_size = (ARROW_PX as f64 * world_scale * 1.0).clamp(4.0, 10.0) as f32;
                // Warna Electric Royal Blue (bukan merah, kuning, atau hijau)
                const PULL_ARROW_COLOR: [f32; 4] = [0.05, 0.58, 1.0, 1.0];
                let (p, n, c, i) = sketch_render::solid_directional_arrow_mesh(
                    [center.x, center.y, center.z],
                    arrow_length,
                    arrow_size,
                    PULL_ARROW_COLOR,
                    pull_dir,
                );
                let base = positions.len() as u32;
                positions.extend(p);
                normals.extend(n);
                colors.extend(c);
                indices.extend(i.into_iter().map(|idx| idx + base));
            }
        }

        (positions, normals, colors, indices)
    }

    #[allow(clippy::type_complexity)]
    pub fn build_combined_body_mesh(
        &self,
    ) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 4]>, Vec<[f32; 4]>, Vec<u32>) {
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut colors = Vec::new();
        let mut materials = Vec::new();
        let mut indices = Vec::new();

        const SELECTED_CYAN: [f32; 4] = [0.0, 0.75, 1.0, 1.0];
        const CUT_RED: [f32; 4] = [1.0, 0.25, 0.25, 0.90];
        const FACE_SELECT_CYAN: [f32; 4] = [0.0, 0.90, 1.0, 1.0];

        let vertex_round_preview = if self.filleting_vertex_from_gizmo {
            self.round_gizmo_preview_shape(RoundKind::Vertex, self.vertex_gizmo_radius)
        } else {
            None
        };
        let edge_round_preview = if self.filleting_edge_from_gizmo {
            self.round_gizmo_preview_shape(RoundKind::Edge, self.edge_gizmo_radius)
        } else {
            None
        };
        let round_override = vertex_round_preview.or(edge_round_preview);

        for (id, geo) in self.model.geometry.iter() {
            let Some(body) = self.model.doc.bodies.get(id) else {
                continue;
            };
            if !body.visible {
                continue;
            }

            let is_cutting_target = self.gizmo_is_cutting && self.gizmo_target_body == Some(id);

            let mesh_to_render = if let Some((override_id, override_shape)) = &round_override {
                if *override_id == id {
                    let tess = override_shape.tessellate();
                    let km = ducad_kernel::KernelMesh {
                        positions: tess.positions.clone(),
                        normals: tess.normals.clone(),
                        indices: tess.indices.clone(),
                    };
                    std::borrow::Cow::Owned(km)
                } else {
                    std::borrow::Cow::Borrowed(&geo.mesh)
                }
            } else {
                std::borrow::Cow::Borrowed(&geo.mesh)
            };

            let mut transformed_positions = mesh_to_render.positions.clone();
            let mut transformed_normals = mesh_to_render.normals.clone();
            let is_selected_body = self.selected_bodies.contains(&id) && self.active_face.is_none();

            if is_selected_body {
                if self.body_move_dragging && self.body_move_delta.length_squared() > 1e-6 {
                    for p in &mut transformed_positions {
                        p[0] += self.body_move_delta.x;
                        p[1] += self.body_move_delta.y;
                        p[2] += self.body_move_delta.z;
                    }
                } else if self.body_rotate_dragging && self.body_rotate_angle_deg.abs() > 0.01 {
                    if let Some((_, center)) = self.selected_single_body_center() {
                        let rad = (self.body_rotate_angle_deg as f32).to_radians();
                        let rot_mat = glam::Mat4::from_axis_angle(self.body_rotate_axis, rad);
                        for p in &mut transformed_positions {
                            let p_rel = Vec3::from_slice(p) - center;
                            let p_rot = center + rot_mat.transform_vector3(p_rel);
                            p[0] = p_rot.x;
                            p[1] = p_rot.y;
                            p[2] = p_rot.z;
                        }
                        for n in &mut transformed_normals {
                            let n_rot = rot_mat.transform_vector3(Vec3::from_slice(n));
                            n[0] = n_rot.x;
                            n[1] = n_rot.y;
                            n[2] = n_rot.z;
                        }
                    }
                }
            }

            let mat = &body.material;
            let mat_params = [mat.roughness, mat.metallic, mat.clearcoat, 0.0];
            let base_color = mat.base_color;

            let (body_color, body_mat) = if is_cutting_target {
                (CUT_RED, [0.4, 0.0, 0.2, 0.0])
            } else if is_selected_body && !self.cmf_drawer_open {
                (SELECTED_CYAN, [0.35, 0.0, 0.50, 0.0])
            } else {
                (base_color, mat_params)
            };

            let face_info = if let Some((active_id, _, hit)) = &self.active_face {
                if *active_id == id {
                    Some((
                        Vec3::new(hit.hit_point.0 as f32, hit.hit_point.1 as f32, hit.hit_point.2 as f32),
                        Vec3::new(hit.centroid.0 as f32, hit.centroid.1 as f32, hit.centroid.2 as f32),
                        Vec3::new(hit.normal.0 as f32, hit.normal.1 as f32, hit.normal.2 as f32),
                        hit.surface_kind,
                        &hit.boundary_points,
                    ))
                } else {
                    None
                }
            } else {
                None
            };

            let mut face_vertex_indices = std::collections::HashSet::new();
            if let Some((fpt, fcentroid, fnorm, skind, boundary_pts)) = face_info {
                let u_axis = if boundary_pts.len() >= 3 {
                    (Vec3::new(boundary_pts[0].0 as f32, boundary_pts[0].1 as f32, boundary_pts[0].2 as f32) - fcentroid).normalize_or_zero()
                } else {
                    Vec3::ZERO
                };
                let v_axis = fnorm.cross(u_axis).normalize_or_zero();
                let poly2d: Vec<(f64, f64)> = if u_axis != Vec3::ZERO && v_axis != Vec3::ZERO {
                    boundary_pts
                        .iter()
                        .map(|&(x, y, z)| {
                            let d = Vec3::new(x as f32, y as f32, z as f32) - fcentroid;
                            (d.dot(u_axis) as f64, d.dot(v_axis) as f64)
                        })
                        .collect()
                } else {
                    Vec::new()
                };

                for chunk in mesh_to_render.indices.chunks_exact(3) {
                    let idx0 = chunk[0] as usize;
                    let idx1 = chunk[1] as usize;
                    let idx2 = chunk[2] as usize;
                    if idx0 < mesh_to_render.positions.len()
                        && idx1 < mesh_to_render.positions.len()
                        && idx2 < mesh_to_render.positions.len()
                    {
                        let p0 = Vec3::from_slice(&mesh_to_render.positions[idx0]);
                        let p1 = Vec3::from_slice(&mesh_to_render.positions[idx1]);
                        let p2 = Vec3::from_slice(&mesh_to_render.positions[idx2]);
                        let tri_center = (p0 + p1 + p2) / 3.0;
                        let tri_norm = (p1 - p0).cross(p2 - p0).normalize_or_zero();

                        let is_match = match skind {
                            ducad_kernel::SurfaceKind::Plane => {
                                let dist = (tri_center - fpt).dot(fnorm).abs();
                                let align = tri_norm.dot(fnorm).abs();
                                let in_bounds = if poly2d.len() >= 3 {
                                    let d = tri_center - fcentroid;
                                    let p2d = (d.dot(u_axis) as f64, d.dot(v_axis) as f64);
                                    ducad_kernel::point_in_polygon_2d(p2d, &poly2d)
                                } else {
                                    true
                                };
                                dist < 0.6 && align > 0.5 && in_bounds
                            }
                            _ => (tri_center - fpt).length() < 30.0,
                        };
                        if is_match {
                            face_vertex_indices.insert(idx0);
                            face_vertex_indices.insert(idx1);
                            face_vertex_indices.insert(idx2);
                        }
                    }
                }
            }

            let base_idx = positions.len() as u32;
            positions.extend(&transformed_positions);
            normals.extend(&transformed_normals);
            indices.extend(mesh_to_render.indices.iter().map(|i| i + base_idx));

            for (i, _) in mesh_to_render.positions.iter().enumerate() {
                let (v_color, v_mat) = if face_vertex_indices.contains(&i) {
                    (FACE_SELECT_CYAN, [0.35, 0.0, 0.8, 0.0])
                } else {
                    (body_color, body_mat)
                };
                colors.push(v_color);
                materials.push(v_mat);
            }

            // Real-time Extrude Preview: Tambahkan prisma solid (cap + side walls)
            // yang menempel pada face saat di-drag, tanpa memisahkan/merusak mesh body dasar
            if self.extruding_face_from_gizmo && self.face_gizmo_distance.abs() > 0.01 {
                if let Some((active_id, _, hit)) = &self.active_face {
                    if *active_id == id {
                        let pull_vec = Vec3::new(
                            hit.pull_dir.0 as f32,
                            hit.pull_dir.1 as f32,
                            hit.pull_dir.2 as f32,
                        ) * (self.face_gizmo_distance as f32);

                        let preview_start_idx = positions.len() as u32;
                        let mut prev_positions = Vec::new();
                        let mut prev_normals = Vec::new();
                        let mut prev_indices = Vec::new();

                        let mut edge_count: std::collections::HashMap<(u32, u32), usize> = std::collections::HashMap::new();

                        // 1. Cap face yang digeser
                        for chunk in mesh_to_render.indices.chunks_exact(3) {
                            let idx0 = chunk[0];
                            let idx1 = chunk[1];
                            let idx2 = chunk[2];
                            if face_vertex_indices.contains(&(idx0 as usize))
                                && face_vertex_indices.contains(&(idx1 as usize))
                                && face_vertex_indices.contains(&(idx2 as usize))
                            {
                                let p0 = Vec3::from_slice(&mesh_to_render.positions[idx0 as usize]) + pull_vec;
                                let p1 = Vec3::from_slice(&mesh_to_render.positions[idx1 as usize]) + pull_vec;
                                let p2 = Vec3::from_slice(&mesh_to_render.positions[idx2 as usize]) + pull_vec;
                                let fnorm = (p1 - p0).cross(p2 - p0).normalize_or_zero();

                                let base_c = prev_positions.len() as u32;
                                prev_positions.push([p0.x, p0.y, p0.z]);
                                prev_positions.push([p1.x, p1.y, p1.z]);
                                prev_positions.push([p2.x, p2.y, p2.z]);
                                prev_normals.push([fnorm.x, fnorm.y, fnorm.z]);
                                prev_normals.push([fnorm.x, fnorm.y, fnorm.z]);
                                prev_normals.push([fnorm.x, fnorm.y, fnorm.z]);
                                prev_indices.extend_from_slice(&[base_c, base_c + 1, base_c + 2]);

                                for (e0, e1) in [(idx0, idx1), (idx1, idx2), (idx2, idx0)] {
                                    let key = if e0 < e1 { (e0, e1) } else { (e1, e0) };
                                    *edge_count.entry(key).or_insert(0) += 1;
                                }
                            }
                        }

                        // 2. Dinding samping (skirt)
                        for chunk in mesh_to_render.indices.chunks_exact(3) {
                            let idx0 = chunk[0];
                            let idx1 = chunk[1];
                            let idx2 = chunk[2];
                            if face_vertex_indices.contains(&(idx0 as usize))
                                && face_vertex_indices.contains(&(idx1 as usize))
                                && face_vertex_indices.contains(&(idx2 as usize))
                            {
                                for (ea, eb) in [(idx0, idx1), (idx1, idx2), (idx2, idx0)] {
                                    let key = if ea < eb { (ea, eb) } else { (eb, ea) };
                                    if edge_count.get(&key) == Some(&1) {
                                        let p_a0 = Vec3::from_slice(&mesh_to_render.positions[ea as usize]);
                                        let p_b0 = Vec3::from_slice(&mesh_to_render.positions[eb as usize]);
                                        let p_a1 = p_a0 + pull_vec;
                                        let p_b1 = p_b0 + pull_vec;

                                        let snorm = (p_b0 - p_a0).cross(p_a1 - p_a0).normalize_or_zero();

                                        let base_s = prev_positions.len() as u32;
                                        prev_positions.push([p_a0.x, p_a0.y, p_a0.z]);
                                        prev_positions.push([p_b0.x, p_b0.y, p_b0.z]);
                                        prev_positions.push([p_b1.x, p_b1.y, p_b1.z]);
                                        prev_positions.push([p_a1.x, p_a1.y, p_a1.z]);

                                        for _ in 0..4 {
                                            prev_normals.push([snorm.x, snorm.y, snorm.z]);
                                        }
                                        prev_indices.extend_from_slice(&[
                                            base_s, base_s + 1, base_s + 2,
                                            base_s, base_s + 2, base_s + 3,
                                        ]);
                                    }
                                }
                            }
                        }

                        let preview_color = if self.face_gizmo_distance < 0.0 {
                            [0.95, 0.25, 0.20, 0.85]
                        } else {
                            [0.0, 0.85, 1.0, 0.90]
                        };
                        let prev_len = prev_positions.len();
                        positions.extend(prev_positions);
                        normals.extend(prev_normals);
                        indices.extend(prev_indices.into_iter().map(|idx| idx + preview_start_idx));
                        colors.extend(std::iter::repeat_n(preview_color, prev_len));
                        materials.extend(std::iter::repeat_n([0.35, 0.0, 0.60, 0.0], prev_len));
                    }
                }
            }
        }

        if self.extruding_from_gizmo {
            if let Ok(profile) =
                crate::model::build_profile_from_selection(self.sketch(), &self.selected)
            {
                if let Ok(swept) =
                    self.extrude_profile_active_plane(&profile, self.gizmo_distance)
                {
                    let tess = swept.tessellate();
                    let base_idx = positions.len() as u32;
                    let v_count = tess.positions.len();
                    positions.extend(&tess.positions);
                    normals.extend(&tess.normals);
                    indices.extend(tess.indices.iter().map(|i| i + base_idx));
                    let preview_color = if self.gizmo_is_cutting {
                        [0.95, 0.25, 0.20, 0.85]
                    } else {
                        [0.10, 0.70, 0.95, 0.75]
                    };
                    colors.extend(std::iter::repeat_n(preview_color, v_count));
                    materials.extend(std::iter::repeat_n([0.35, 0.0, 0.60, 0.0], v_count));
                }
            }
        }

        // 3D Live Ghost Pattern Preview
        if self.tool == ToolKind::Pattern && !self.selected_bodies.is_empty() {
            const GHOST_CYAN: [f32; 4] = [0.0, 0.80, 1.0, 0.40];
            for &body_id in &self.selected_bodies {
                let Some(geo) = self.model.geometry.get(body_id) else {
                    continue;
                };
                let duplicated_shapes = match self.pattern_kind {
                    ducad_ui::PatternKind::Linear => {
                        ducad_kernel::linear_pattern_shape(
                            &geo.shape,
                            self.pattern_count_x,
                            self.pattern_pitch_x,
                            self.pattern_count_y,
                            self.pattern_pitch_y,
                            self.pattern_count_z,
                            self.pattern_pitch_z,
                        )
                    }
                    ducad_ui::PatternKind::Circular => {
                        let center = geo.mesh.center();
                        let pivot = self.pattern_custom_pivot_3d
                            .map(|v| (v.x as f64, v.y as f64, v.z as f64))
                            .unwrap_or((center[0] as f64, center[1] as f64, center[2] as f64));
                        let axis = self.pattern_circ_axis.to_dir();
                        let total_angle_rad = self.pattern_circ_angle_deg.to_radians();

                        ducad_kernel::circular_pattern_shape(
                            &geo.shape,
                            pivot,
                            axis,
                            self.pattern_circ_count,
                            total_angle_rad,
                        )
                    }
                };

                if let Ok(shapes) = duplicated_shapes {
                    for shape in shapes {
                        let tess = shape.tessellate();
                        let base_index = positions.len() as u32;
                        let tri_count = tess.triangle_count();
                        positions.extend(tess.positions);
                        normals.extend(tess.normals);
                        colors.extend(vec![GHOST_CYAN; tri_count * 3]);
                        materials.extend(vec![[0.40, 0.0, 0.0, 0.0]; tri_count * 3]);
                        indices.extend(tess.indices.into_iter().map(|idx| idx + base_index));
                    }
                }
            }
        }

        (positions, normals, colors, materials, indices)
    }

    pub fn section_clip_plane(&self) -> Option<(Vec3, f32)> {
        if !self.section_enabled {
            return None;
        }
        let normal = self.section_axis.normal();
        let (n, offset) = if self.section_invert {
            (-normal, -self.section_offset)
        } else {
            (normal, self.section_offset)
        };
        Some((n, offset))
    }

    /// Hitung posisi 3D titik pusat lubang aktif dan basis vektor (U, V, Normal) pada permukaan face.
    pub fn compute_active_hole_position_and_basis(
        &self,
        hit: &ducad_kernel::FaceHit,
    ) -> (Vec3, Vec3, Vec3, Vec3) {
        let normal = Vec3::new(
            hit.normal.0 as f32,
            hit.normal.1 as f32,
            hit.normal.2 as f32,
        )
        .normalize_or_zero();
        let normal = if normal == Vec3::ZERO {
            Vec3::Z
        } else {
            normal
        };

        let u_axis = if hit.boundary_points.len() >= 2 {
            let p0 = Vec3::new(
                hit.boundary_points[0].0 as f32,
                hit.boundary_points[0].1 as f32,
                hit.boundary_points[0].2 as f32,
            );
            let p1 = Vec3::new(
                hit.boundary_points[1].0 as f32,
                hit.boundary_points[1].1 as f32,
                hit.boundary_points[1].2 as f32,
            );
            let raw_u = (p1 - p0).normalize_or_zero();
            let perp_u = (raw_u - normal * raw_u.dot(normal)).normalize_or_zero();
            if perp_u != Vec3::ZERO {
                perp_u
            } else if normal.dot(Vec3::Z).abs() < 0.9 {
                Vec3::Z.cross(normal).normalize_or_zero()
            } else {
                Vec3::X.cross(normal).normalize_or_zero()
            }
        } else if normal.dot(Vec3::Z).abs() < 0.9 {
            Vec3::Z.cross(normal).normalize_or_zero()
        } else {
            Vec3::X.cross(normal).normalize_or_zero()
        };

        let v_axis = normal.cross(u_axis).normalize_or_zero();

        let base_pt = Vec3::new(
            hit.hit_point.0 as f32,
            hit.hit_point.1 as f32,
            hit.hit_point.2 as f32,
        );

        let hole_pos = if let Some(custom) = self.hole_popup_state.current_pos_3d {
            Vec3::new(custom.0 as f32, custom.1 as f32, custom.2 as f32)
        } else {
            base_pt
                + u_axis * (self.hole_popup_state.offset_u as f32)
                + v_axis * (self.hole_popup_state.offset_v as f32)
        };

        (hole_pos, u_axis, v_axis, normal)
    }

    /// Hitung jarak dan titik proyeksi ke tepi-tepi terdekat dari face untuk ruler bantu Hole Wizard.
    pub fn compute_hole_closest_edges(
        &self,
        hit: &ducad_kernel::FaceHit,
        hole_pos: Vec3,
    ) -> Vec<(Vec3, f32, Vec3, Vec3)> {
        if hit.boundary_points.len() < 2 {
            return Vec::new();
        }

        let mut edge_hits = Vec::new();
        let len = hit.boundary_points.len();

        for i in 0..len {
            let j = (i + 1) % len;
            let a = Vec3::new(
                hit.boundary_points[i].0 as f32,
                hit.boundary_points[i].1 as f32,
                hit.boundary_points[i].2 as f32,
            );
            let b = Vec3::new(
                hit.boundary_points[j].0 as f32,
                hit.boundary_points[j].1 as f32,
                hit.boundary_points[j].2 as f32,
            );
            let ab = b - a;
            let ab_len_sq = ab.length_squared();
            if ab_len_sq > 1e-6 {
                let t = ((hole_pos - a).dot(ab) / ab_len_sq).clamp(0.0, 1.0);
                let q = a + ab * t;
                let dist = (hole_pos - q).length();
                edge_hits.push((q, dist, a, b));
            }
        }

        edge_hits.sort_by(|a, b| {
            a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
        });
        // Ambil maksimal 4 tepi terdekat
        edge_hits.truncate(4);
        edge_hits
    }
}
