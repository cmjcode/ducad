use ducad_render::{sketch as sketch_render, LineVertex};
use ducad_sketch::{arc_from_three_points, mirror_entity, offset_entity, Entity};
use glam::{DVec2, Vec3};

use crate::app::DuCADApp;
use crate::input::trim_removal_preview;
use crate::types::{RoundKind, ToolKind};

impl DuCADApp {
    pub fn build_overlay_lines(
        &self,
        raw_cursor: Option<DVec2>,
        world_scale: f64,
    ) -> Vec<LineVertex> {
        let mut verts = Vec::new();

        for idx in 0..3 {
            let plane = Self::plane_for_index(idx);
            if idx == self.active_plane_index() {
                verts.extend(sketch_render::entity_lines(
                    &self.sketches[idx],
                    self.hovered,
                    &self.selected,
                    &plane,
                ));
            } else {
                verts.extend(sketch_render::inactive_entity_lines(
                    &self.sketches[idx],
                    &plane,
                ));
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

        if self.tool == ToolKind::Select {
            if let Some(centroid) = self.selected_closed_region_centroid() {
                let c_base_pt = self.active_plane.to_world(centroid, 0.02);
                let c_base = [c_base_pt.x, c_base_pt.y, c_base_pt.z];

            if self.extruding_from_gizmo {
                let c_top_pt = self
                    .active_plane
                    .to_world(centroid, self.gizmo_distance as f32);
                let c_top = [c_top_pt.x, c_top_pt.y, c_top_pt.z];
                verts.extend(sketch_render::dashed_line_3d(
                    c_base,
                    c_top,
                    4.0,
                    [0.15, 0.70, 1.0, 0.95],
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
                let anchor = hit.gizmo_anchor();
                let c_base = [anchor.0 as f32, anchor.1 as f32, anchor.2 as f32];
                let pull_dir = Vec3::new(
                    hit.pull_dir.0 as f32,
                    hit.pull_dir.1 as f32,
                    hit.pull_dir.2 as f32,
                );

                if self.extruding_face_from_gizmo {
                    let dist = self.face_gizmo_distance as f32;
                    let c_top = Vec3::from(c_base) + pull_dir * dist;
                    let c_top_arr = [c_top.x, c_top.y, c_top.z];
                    verts.extend(sketch_render::dashed_line_3d(
                        c_base,
                        c_top_arr,
                        4.0,
                        [0.15, 0.80, 1.0, 0.95],
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

        if let Some(raw) = raw_cursor {
            let offset_dist = (14.0 * world_scale).max(8.0);
            match self.tool {
                ToolKind::Line if self.pending_points.len() == 1 => {
                    let start = self.pending_points[0];
                    let end = self.snapped_or(raw);
                    let preview = Entity::Line { start, end };
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
                        let preview = Entity::Line {
                            start: corners[i],
                            end: corners[(i + 1) % 4],
                        };
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
                    let preview = Entity::Circle {
                        center: first,
                        radius,
                    };
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
                        let preview = Entity::Ellipse {
                            center: first,
                            radius_x,
                            radius_y,
                        };
                        verts.extend(sketch_render::preview_lines(&preview, &self.active_plane));
                    }
                }
                ToolKind::Arc => {
                    let effective = self.snapped_or(raw);
                    match self.pending_points.len() {
                        1 => {
                            let preview = Entity::Line {
                                start: self.pending_points[0],
                                end: effective,
                            };
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
                            if let Some(preview) = arc_from_three_points(
                                self.pending_points[0],
                                self.pending_points[1],
                                effective,
                            ) {
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
                    let axis_preview = Entity::Line {
                        start: axis_a,
                        end: axis_b,
                    };
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
                        let axis_preview = Entity::Line {
                            start: self.pending_points[0],
                            end: self.snapped_or(raw),
                        };
                        verts.extend(sketch_render::preview_lines(
                            &axis_preview,
                            &self.active_plane,
                        ));
                    } else if let Some((start, end)) = self.revolve_staged_axis {
                        let axis_preview = Entity::Line { start, end };
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

        if let Some(hit) = &self.last_snap {
            verts.extend(sketch_render::snap_glyph(hit, &self.active_plane));
        }

        verts
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
            push_mesh(
                &mut positions,
                &mut normals,
                &mut colors,
                &mut indices,
                [p.x, p.y, p.z],
                GIZMO_ARROW_COLOR,
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
                push_mesh(
                    &mut positions,
                    &mut normals,
                    &mut colors,
                    &mut indices,
                    [p.x, p.y, p.z],
                    FACE_GIZMO_COLOR,
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


        (positions, normals, colors, indices)
    }

    #[allow(clippy::type_complexity)]
    pub fn build_combined_body_mesh(
        &self,
    ) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 4]>, Vec<u32>) {
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut colors = Vec::new();
        let mut indices = Vec::new();

        const CAD_GREY: [f32; 4] = [0.62, 0.68, 0.76, 1.0];
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

            let body_color = if is_cutting_target {
                CUT_RED
            } else if is_selected_body {
                SELECTED_CYAN
            } else {
                CAD_GREY
            };

            let face_info = if let Some((active_id, _, hit)) = &self.active_face {
                if *active_id == id {
                    Some((
                        Vec3::new(hit.hit_point.0 as f32, hit.hit_point.1 as f32, hit.hit_point.2 as f32),
                        Vec3::new(hit.normal.0 as f32, hit.normal.1 as f32, hit.normal.2 as f32),
                        hit.surface_kind,
                    ))
                } else {
                    None
                }
            } else {
                None
            };

            let mut face_vertex_indices = std::collections::HashSet::new();
            if let Some((fpt, fnorm, skind)) = face_info {
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
                                dist < 0.6 && align > 0.5
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
                let v_color = if face_vertex_indices.contains(&i) {
                    FACE_SELECT_CYAN
                } else {
                    body_color
                };
                colors.push(v_color);
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

                        const PREVIEW_CYAN: [f32; 4] = [0.0, 0.85, 1.0, 0.90];
                        let prev_len = prev_positions.len();
                        positions.extend(prev_positions);
                        normals.extend(prev_normals);
                        indices.extend(prev_indices.into_iter().map(|idx| idx + preview_start_idx));
                        colors.extend(std::iter::repeat_n(PREVIEW_CYAN, prev_len));
                    }
                }
            }
        }

        if self.extruding_from_gizmo && !self.gizmo_is_cutting {
            if let Ok(profile) =
                crate::model::build_profile_from_selection(self.sketch(), &self.selected)
            {
                if let Ok(swept) =
                    self.extrude_profile_active_plane(&profile, self.gizmo_distance)
                {
                    let tess = swept.tessellate();
                    let base_idx = positions.len() as u32;
                    positions.extend(&tess.positions);
                    normals.extend(&tess.normals);
                    indices.extend(tess.indices.iter().map(|i| i + base_idx));
                    const PREVIEW_CYAN: [f32; 4] = [0.10, 0.70, 0.95, 0.75];
                    colors.extend(std::iter::repeat_n(PREVIEW_CYAN, tess.positions.len()));
                }
            }
        }

        (positions, normals, colors, indices)
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
}
