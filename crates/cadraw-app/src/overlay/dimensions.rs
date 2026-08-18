use cadraw_kernel::SurfaceKind;
use cadraw_render::sketch::TransformGizmoPart;
use cadraw_render::SketchPlane;
use cadraw_sketch::{find_snap, Entity};
use cadraw_ui::CanvasHud;
use eframe::egui;
use glam::{DVec2, Vec3};
use slotmap::Key;

use crate::app::CadrawApp;
use crate::types::{RoundKind, ToolKind};
use crate::viewport::{pixel_tolerance_to_world, screen_to_plane_point, world_to_screen_pos};

impl CadrawApp {
    pub fn screen_line_angle(&self, rect: egui::Rect, a: DVec2, b: DVec2) -> f32 {
        let a_3d = self.active_plane.to_world(a, 0.0);
        let b_3d = self.active_plane.to_world(b, 0.0);
        self.screen_angle_between_world_points(rect, a_3d, b_3d)
    }

    pub fn screen_angle_between_world_points(&self, rect: egui::Rect, a_3d: Vec3, b_3d: Vec3) -> f32 {
        match (
            world_to_screen_pos(&self.camera, rect, a_3d),
            world_to_screen_pos(&self.camera, rect, b_3d),
        ) {
            (Some(pa), Some(pb)) => {
                let mut angle = (pb - pa).angle();
                if angle > std::f32::consts::FRAC_PI_2 {
                    angle -= std::f32::consts::PI;
                } else if angle < -std::f32::consts::FRAC_PI_2 {
                    angle += std::f32::consts::PI;
                }
                angle
            }
            _ => 0.0,
        }
    }

    pub fn format_face_gizmo_dimension_text(
        &self,
        surface_kind: SurfaceKind,
        distance: f64,
    ) -> String {
        let formatted = self.unit.format(distance);
        if matches!(
            surface_kind,
            SurfaceKind::Cylinder | SurfaceKind::Cone | SurfaceKind::Sphere
        ) {
            if distance >= 0.0 {
                format!("ΔR +{formatted}")
            } else {
                format!("ΔR {formatted}")
            }
        } else {
            formatted
        }
    }

    pub fn render_all_element_dimensions(&self, ui: &mut egui::Ui, rect: egui::Rect) {
        let mut line_anchors_2d: Vec<(Vec3, f64)> = Vec::new();

        for (_, entity) in self.sketch().entities.iter() {
            match entity {
                Entity::Line { start, end } => {
                    let len = (*end - *start).length();
                    let mid = (*start + *end) * 0.5;
                    let label_3d = self.active_plane.to_world(mid, 0.0);
                    line_anchors_2d.push((label_3d, len));
                    if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, label_3d) {
                        let angle = self.screen_line_angle(rect, *start, *end);
                        CanvasHud::render_dimension_pill_aligned(
                            ui,
                            pos_2d,
                            angle,
                            &self.unit.format_precise(len),
                        );
                    }
                }
                Entity::Circle { center, radius } => {
                    let edge_pt = *center + DVec2::new(*radius, 0.0);
                    let label_3d = self.active_plane.to_world(edge_pt, 0.0);
                    if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, label_3d) {
                        let text = format!("R {}", self.unit.format_precise(*radius));
                        CanvasHud::render_dimension_pill(ui, pos_2d, &text, false);
                    }
                }
                Entity::Arc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                } => {
                    let mid_angle = (start_angle + end_angle) * 0.5;
                    let mid_pt =
                        *center + DVec2::new(radius * mid_angle.cos(), radius * mid_angle.sin());
                    let label_3d = self.active_plane.to_world(mid_pt, 0.0);
                    if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, label_3d) {
                        let text = format!("R {}", self.unit.format_precise(*radius));
                        CanvasHud::render_dimension_pill(ui, pos_2d, &text, false);
                    }
                }
                Entity::Ellipse {
                    center,
                    radius_x,
                    radius_y,
                } => {
                    let label_3d = self.active_plane.to_world(*center, 0.0);
                    if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, label_3d) {
                        let text = format!(
                            "Rx {} Ry {}",
                            self.unit.format_precise(*radius_x),
                            self.unit.format_precise(*radius_y)
                        );
                        CanvasHud::render_dimension_pill(ui, pos_2d, &text, false);
                    }
                }
            }
        }

        const COINCIDENCE_POS_EPS: f32 = 1e-3;
        const COINCIDENCE_LEN_EPS: f64 = 1e-3;

        for (id, geo) in self.model.geometry.iter() {
            let visible = self.model.doc.bodies.get(id).is_some_and(|b| b.visible);
            if !visible {
                continue;
            }
            for (mid, start, end, length) in &geo.edge_dims {
                let world_pt = Vec3::new(mid.0 as f32, mid.1 as f32, mid.2 as f32);
                let already_shown_by_sketch = line_anchors_2d.iter().any(|(anchor, len)| {
                    (world_pt - *anchor).length() < COINCIDENCE_POS_EPS
                        && (length - len).abs() < COINCIDENCE_LEN_EPS
                });
                if already_shown_by_sketch {
                    continue;
                }
                if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, world_pt) {
                    let start_pt = Vec3::new(start.0 as f32, start.1 as f32, start.2 as f32);
                    let end_pt = Vec3::new(end.0 as f32, end.1 as f32, end.2 as f32);
                    let angle = self.screen_angle_between_world_points(rect, start_pt, end_pt);
                    CanvasHud::render_dimension_pill_aligned(
                        ui,
                        pos_2d,
                        angle,
                        &self.unit.format_precise(*length),
                    );
                }
            }
        }
    }

    pub fn dynamic_input_ui(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        raw_cursor: Option<DVec2>,
    ) {
        if !self.measurements.is_empty() {
            for m in &self.measurements {
                let Some(value) = m.inline_value(self.unit) else {
                    continue;
                };
                let pts = m.points();
                let (Some(&a), Some(&b)) = (pts.first(), pts.last()) else {
                    continue;
                };
                let mid = (a + b) * 0.5;
                let label_3d = self.active_plane.to_world(mid, 0.0);
                if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, label_3d) {
                    let angle = self.screen_line_angle(rect, a, b);
                    CanvasHud::render_dimension_pill_aligned(ui, pos_2d, angle, &value);
                }
            }
        }

        if self.show_all_dimensions {
            self.render_all_element_dimensions(ui, rect);
        }

        if let Some(raw) = raw_cursor {
            let effective = self.snapped_or(raw);
            let world_scale = pixel_tolerance_to_world(&self.camera, rect);
            let offset_dist = (14.0 * world_scale).max(8.0);

            match self.tool {
                ToolKind::Line if self.pending_points.len() == 1 => {
                    let start = self.pending_points[0];
                    let len = (effective - start).length();
                    let mid = (start + effective) * 0.5;
                    let dir = (effective - start).normalize_or_zero();
                    let normal = DVec2::new(-dir.y, dir.x);
                    let label_pos = mid + normal * offset_dist;
                    let label_3d = self.active_plane.to_world(label_pos, 0.0);
                    if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, label_3d) {
                        CanvasHud::render_dimension_pill(
                            ui,
                            pos_2d,
                            &self.unit.format_precise(len),
                            false,
                        );
                    }
                }
                ToolKind::Rectangle if self.pending_points.len() == 1 => {
                    let first = self.pending_points[0];
                    let min = first.min(effective);
                    let max = first.max(effective);
                    let w = (max.x - min.x).abs();
                    let h = (max.y - min.y).abs();

                    let bot_mid = DVec2::new((min.x + max.x) * 0.5, min.y - offset_dist);
                    let bot_3d = self.active_plane.to_world(bot_mid, 0.0);
                    if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, bot_3d) {
                        CanvasHud::render_dimension_pill(
                            ui,
                            pos_2d,
                            &self.unit.format_precise(w),
                            false,
                        );
                    }
                    let right_mid = DVec2::new(max.x + offset_dist, (min.y + max.y) * 0.5);
                    let right_3d = self.active_plane.to_world(right_mid, 0.0);
                    if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, right_3d) {
                        CanvasHud::render_dimension_pill(
                            ui,
                            pos_2d,
                            &self.unit.format_precise(h),
                            false,
                        );
                    }
                }
                ToolKind::Circle if self.pending_points.len() == 1 => {
                    let first = self.pending_points[0];
                    let radius = (effective - first).length();
                    let mid = (first + effective) * 0.5;
                    let mid_3d = self.active_plane.to_world(mid, 0.0);
                    if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, mid_3d) {
                        CanvasHud::render_dimension_pill(
                            ui,
                            pos_2d,
                            &format!("R {}", self.unit.format_precise(radius)),
                            false,
                        );
                    }
                }
                ToolKind::Measure if self.pending_points.len() == 1 => {
                    let start = self.pending_points[0];
                    let len = (effective - start).length();
                    let mid = (start + effective) * 0.5;
                    let label_3d = self.active_plane.to_world(mid, 0.0);
                    if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, label_3d) {
                        let angle = self.screen_line_angle(rect, start, effective);
                        CanvasHud::render_dimension_pill_aligned(
                            ui,
                            pos_2d,
                            angle,
                            &self.unit.format_precise(len),
                        );
                    }
                }
                _ => {}
            }
        }

        if let Some(centroid) = self.selected_closed_region_centroid() {
            let z_pos = if self.extruding_from_gizmo {
                self.gizmo_distance
            } else {
                18.0
            };
            let handle_3d = self.active_plane.to_world(centroid, z_pos as f32);

            if let Some(handle_2d) = world_to_screen_pos(&self.camera, rect, handle_3d) {
                let (_, arrow_vec_opt) = self.project_screen_drag_to_extrude_axis(
                    rect,
                    centroid,
                    egui::Vec2::ZERO,
                );

                let handle_resp = CanvasHud::render_draggable_double_arrow_handle(
                    ui,
                    handle_2d,
                    self.extruding_from_gizmo,
                    arrow_vec_opt,
                );

                if handle_resp.drag_started() {
                    self.extruding_from_gizmo = true;
                    if self.gizmo_distance == 0.0 {
                        self.gizmo_distance = 20.0;
                    }
                }

                if handle_resp.dragged() {
                    self.extruding_from_gizmo = true;
                    let (delta_mm, _) = self.project_screen_drag_to_extrude_axis(
                        rect,
                        centroid,
                        handle_resp.drag_delta(),
                    );
                    self.gizmo_distance += delta_mm;
                    self.update_gizmo_boolean_detection();
                }

                if handle_resp.drag_stopped() {
                    self.commit_gizmo_extrusion();
                }

                let pill_pos = handle_2d + egui::vec2(0.0, -32.0);
                let text = self.unit.format(self.gizmo_distance.abs());
                let pill_resp = CanvasHud::render_interactive_dimension_pill(
                    ui,
                    pill_pos,
                    &text,
                    self.gizmo_dimension_editing,
                );
                if pill_resp.clicked() {
                    self.gizmo_dimension_editing = !self.gizmo_dimension_editing;
                    self.gizmo_edit_input = format!(
                        "{:.0}",
                        self.unit.to_display_val(self.gizmo_distance)
                    );
                }

                if self.gizmo_dimension_editing {
                    let popup_rect = egui::Rect::from_center_size(
                        pill_pos + egui::vec2(0.0, 28.0),
                        egui::vec2(100.0, 32.0),
                    );
                    egui::Area::new(egui::Id::new("cadraw-gizmo-edit-popup"))
                        .fixed_pos(popup_rect.min)
                        .order(egui::Order::Foreground)
                        .show(ui.ctx(), |ui| {
                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                let resp = ui.text_edit_singleline(&mut self.gizmo_edit_input);
                                resp.request_focus();
                                if resp.lost_focus()
                                    || ui.input(|i| i.key_pressed(egui::Key::Enter))
                                {
                                    if let Ok(val) =
                                        self.gizmo_edit_input.trim().parse::<f64>()
                                    {
                                        self.gizmo_distance = self.unit.to_internal_mm(val);
                                        self.commit_gizmo_extrusion();
                                    }
                                    self.gizmo_dimension_editing = false;
                                }
                            });
                        });
                }
            }
        }

        if let Some((_, _, hit)) = &self.active_face {
            let anchor = hit.gizmo_anchor();
            let c_base = Vec3::new(anchor.0 as f32, anchor.1 as f32, anchor.2 as f32);
            let pull_dir = Vec3::new(
                hit.pull_dir.0 as f32,
                hit.pull_dir.1 as f32,
                hit.pull_dir.2 as f32,
            );
            let surface_kind = hit.surface_kind;
            let z_pos = if self.extruding_face_from_gizmo {
                self.face_gizmo_distance as f32
            } else {
                18.0
            };
            let handle_3d = c_base + pull_dir * z_pos;

            if let Some(handle_2d) = world_to_screen_pos(&self.camera, rect, handle_3d) {
                let (_, arrow_vec_opt) = self.project_screen_drag_to_world_axis(
                    rect,
                    c_base,
                    pull_dir,
                    egui::Vec2::ZERO,
                );

                let handle_resp = CanvasHud::render_draggable_double_arrow_handle(
                    ui,
                    handle_2d,
                    self.extruding_face_from_gizmo,
                    arrow_vec_opt,
                );

                if handle_resp.drag_started() {
                    self.extruding_face_from_gizmo = true;
                    if self.face_gizmo_distance == 0.0 {
                        self.face_gizmo_distance = 15.0;
                    }
                }

                if handle_resp.dragged() {
                    self.extruding_face_from_gizmo = true;
                    let (delta_mm, _) = self.project_screen_drag_to_world_axis(
                        rect,
                        c_base,
                        pull_dir,
                        handle_resp.drag_delta(),
                    );
                    self.face_gizmo_distance += delta_mm;
                    self.face_gizmo_edit_input = format!(
                        "{:.0}",
                        self.unit.to_display_val(self.face_gizmo_distance)
                    );
                    ui.ctx().request_repaint();
                }

                if handle_resp.drag_stopped() {
                    if self.face_gizmo_distance.abs() > 0.1 {
                        self.extrude_active_face(self.face_gizmo_distance);
                    }
                    self.extruding_face_from_gizmo = false;
                    self.face_gizmo_distance = 15.0;
                    self.face_gizmo_edit_input = "15".to_string();
                }

                let pill_pos = handle_2d + egui::vec2(0.0, -32.0);
                let text = self.format_face_gizmo_dimension_text(
                    surface_kind,
                    self.face_gizmo_distance,
                );
                let pill_resp = CanvasHud::render_interactive_dimension_pill(
                    ui,
                    pill_pos,
                    &text,
                    self.face_gizmo_dimension_editing,
                );
                if pill_resp.clicked() {
                    self.face_gizmo_dimension_editing = !self.face_gizmo_dimension_editing;
                    self.face_gizmo_edit_input = format!(
                        "{:.0}",
                        self.unit.to_display_val(self.face_gizmo_distance)
                    );
                }

                if self.face_gizmo_dimension_editing {
                    let popup_rect = egui::Rect::from_center_size(
                        pill_pos + egui::vec2(0.0, 28.0),
                        egui::vec2(100.0, 32.0),
                    );
                    egui::Area::new(egui::Id::new("cadraw-face-gizmo-edit-popup"))
                        .fixed_pos(popup_rect.min)
                        .order(egui::Order::Foreground)
                        .show(ui.ctx(), |ui| {
                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                let resp = ui.text_edit_singleline(&mut self.face_gizmo_edit_input);
                                resp.request_focus();
                                if resp.lost_focus()
                                    || ui.input(|i| i.key_pressed(egui::Key::Enter))
                                {
                                    if let Ok(val) =
                                        self.face_gizmo_edit_input.trim().parse::<f64>()
                                    {
                                        let dist = self.unit.to_internal_mm(val);
                                        self.face_gizmo_distance = dist;
                                        self.extrude_active_face(dist);
                                    }
                                    self.face_gizmo_dimension_editing = false;
                                }
                            });
                        });
                }
            }
        }

        if let Some((c_base, pull_dir)) = self.active_vertex_gizmo_dir() {
            let z_pos = if self.filleting_vertex_from_gizmo {
                self.vertex_gizmo_radius.max(0.1) as f32
            } else {
                12.0
            };
            let handle_3d = c_base + pull_dir * z_pos;

            if let Some(handle_2d) = world_to_screen_pos(&self.camera, rect, handle_3d) {
                let (_, arrow_vec_opt) = self.project_screen_drag_to_world_axis(
                    rect,
                    c_base,
                    pull_dir,
                    egui::Vec2::ZERO,
                );

                let handle_resp = CanvasHud::render_draggable_double_arrow_handle(
                    ui,
                    handle_2d,
                    self.filleting_vertex_from_gizmo,
                    arrow_vec_opt,
                );

                if handle_resp.drag_started() {
                    self.filleting_vertex_from_gizmo = true;
                    if self.vertex_gizmo_radius <= 0.0 {
                        self.vertex_gizmo_radius = 3.0;
                    }
                }

                if handle_resp.dragged() {
                    self.filleting_vertex_from_gizmo = true;
                    let (delta_mm, _) = self.project_screen_drag_to_world_axis(
                        rect,
                        c_base,
                        pull_dir,
                        handle_resp.drag_delta(),
                    );
                    let candidate_radius = (self.vertex_gizmo_radius + delta_mm).max(0.0);
                    if candidate_radius < Self::ROUND_SHARP_MM
                        || self
                            .round_gizmo_preview_shape(RoundKind::Vertex, candidate_radius)
                            .is_some()
                    {
                        self.vertex_gizmo_radius = candidate_radius;
                    }
                    self.vertex_gizmo_edit_input = format!(
                        "{:.1}",
                        self.unit.to_display_val(self.vertex_gizmo_radius)
                    );
                }

                if handle_resp.drag_stopped() {
                    self.commit_vertex_fillet();
                    self.filleting_vertex_from_gizmo = false;
                }

                let pill_pos = handle_2d + egui::vec2(0.0, -32.0);
                let text = if self.vertex_gizmo_radius < Self::ROUND_SHARP_MM {
                    "R 0 (siku)".to_string()
                } else {
                    format!("R {}", self.unit.format(self.vertex_gizmo_radius))
                };
                let pill_resp = CanvasHud::render_interactive_dimension_pill(
                    ui,
                    pill_pos,
                    &text,
                    self.vertex_gizmo_dimension_editing,
                );
                if pill_resp.clicked() {
                    self.vertex_gizmo_dimension_editing =
                        !self.vertex_gizmo_dimension_editing;
                    self.vertex_gizmo_edit_input = format!(
                        "{:.1}",
                        self.unit.to_display_val(self.vertex_gizmo_radius)
                    );
                }

                if self.vertex_gizmo_dimension_editing {
                    let popup_rect = egui::Rect::from_center_size(
                        pill_pos + egui::vec2(0.0, 28.0),
                        egui::vec2(100.0, 32.0),
                    );
                    egui::Area::new(egui::Id::new("cadraw-vertex-gizmo-edit-popup"))
                        .fixed_pos(popup_rect.min)
                        .order(egui::Order::Foreground)
                        .show(ui.ctx(), |ui| {
                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                let resp =
                                    ui.text_edit_singleline(&mut self.vertex_gizmo_edit_input);
                                resp.request_focus();
                                if resp.lost_focus()
                                    || ui.input(|i| i.key_pressed(egui::Key::Enter))
                                {
                                    if let Ok(val) =
                                        self.vertex_gizmo_edit_input.trim().parse::<f64>()
                                    {
                                        self.vertex_gizmo_radius =
                                            self.unit.to_internal_mm(val).max(0.0);
                                        self.commit_vertex_fillet();
                                    }
                                    self.vertex_gizmo_dimension_editing = false;
                                }
                            });
                        });
                }
            }
        }

        if let Some((c_base, pull_dir)) = self.active_edge_gizmo_dir() {
            let z_pos = if self.filleting_edge_from_gizmo {
                self.edge_gizmo_radius.max(0.1) as f32
            } else {
                12.0
            };
            let handle_3d = c_base + pull_dir * z_pos;

            if let Some(handle_2d) = world_to_screen_pos(&self.camera, rect, handle_3d) {
                let (_, arrow_vec_opt) = self.project_screen_drag_to_world_axis(
                    rect,
                    c_base,
                    pull_dir,
                    egui::Vec2::ZERO,
                );

                let handle_resp = CanvasHud::render_draggable_double_arrow_handle(
                    ui,
                    handle_2d,
                    self.filleting_edge_from_gizmo,
                    arrow_vec_opt,
                );

                if handle_resp.drag_started() {
                    self.filleting_edge_from_gizmo = true;
                    if self.edge_gizmo_radius <= 0.0 {
                        self.edge_gizmo_radius = 3.0;
                    }
                }

                if handle_resp.dragged() {
                    self.filleting_edge_from_gizmo = true;
                    let (delta_mm, _) = self.project_screen_drag_to_world_axis(
                        rect,
                        c_base,
                        pull_dir,
                        handle_resp.drag_delta(),
                    );
                    let candidate_radius = (self.edge_gizmo_radius + delta_mm).max(0.0);
                    if candidate_radius < Self::ROUND_SHARP_MM
                        || self
                            .round_gizmo_preview_shape(RoundKind::Edge, candidate_radius)
                            .is_some()
                    {
                        self.edge_gizmo_radius = candidate_radius;
                    }
                    self.edge_gizmo_edit_input = format!(
                        "{:.1}",
                        self.unit.to_display_val(self.edge_gizmo_radius)
                    );
                }

                if handle_resp.drag_stopped() {
                    self.commit_edge_fillet_single();
                    self.filleting_edge_from_gizmo = false;
                }

                let pill_pos = handle_2d + egui::vec2(0.0, -32.0);
                let text = if self.edge_gizmo_radius < Self::ROUND_SHARP_MM {
                    "R 0 (siku)".to_string()
                } else {
                    format!("R {}", self.unit.format(self.edge_gizmo_radius))
                };
                let pill_resp = CanvasHud::render_interactive_dimension_pill(
                    ui,
                    pill_pos,
                    &text,
                    self.edge_gizmo_dimension_editing,
                );
                if pill_resp.clicked() {
                    self.edge_gizmo_dimension_editing = !self.edge_gizmo_dimension_editing;
                    self.edge_gizmo_edit_input = format!(
                        "{:.1}",
                        self.unit.to_display_val(self.edge_gizmo_radius)
                    );
                }

                if self.edge_gizmo_dimension_editing {
                    let popup_rect = egui::Rect::from_center_size(
                        pill_pos + egui::vec2(0.0, 28.0),
                        egui::vec2(100.0, 32.0),
                    );
                    egui::Area::new(egui::Id::new("cadraw-edge-gizmo-edit-popup"))
                        .fixed_pos(popup_rect.min)
                        .order(egui::Order::Foreground)
                        .show(ui.ctx(), |ui| {
                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                let resp =
                                    ui.text_edit_singleline(&mut self.edge_gizmo_edit_input);
                                resp.request_focus();
                                if resp.lost_focus()
                                    || ui.input(|i| i.key_pressed(egui::Key::Enter))
                                {
                                    if let Ok(val) =
                                        self.edge_gizmo_edit_input.trim().parse::<f64>()
                                    {
                                        self.edge_gizmo_radius =
                                            self.unit.to_internal_mm(val).max(0.0);
                                        self.commit_edge_fillet_single();
                                    }
                                    self.edge_gizmo_dimension_editing = false;
                                }
                            });
                        });
                }
            }
        }

        if self.selected_bodies.is_empty() {
            for group in self.sketch_move_groups() {
                let Some(centroid) = self.group_centroid(&group) else {
                    continue;
                };
                let is_target = self.sketch_move_target.as_ref() == Some(&group);
                let delta = if is_target {
                    self.sketch_move_delta
                } else {
                    DVec2::ZERO
                };
                let anchor = self.active_plane.to_world(centroid, 0.05);
                let handle_3d = anchor
                    + self.active_plane.u_axis * delta.x as f32
                    + self.active_plane.v_axis * delta.y as f32;
                let Some(handle_2d) = world_to_screen_pos(&self.camera, rect, handle_3d) else {
                    continue;
                };

                let mut key_ids: Vec<u64> = group.iter().map(|id| id.data().as_ffi()).collect();
                key_ids.sort_unstable();
                let handle_id = egui::Id::new(("cadraw_sketch_move_handle", key_ids));

                let is_dragging_this = is_target && self.sketch_move_dragging;
                let is_armed_this = is_target && self.sketch_move_armed;
                let handle_resp = ui
                    .push_id(handle_id, |ui| {
                        CanvasHud::render_draggable_move_handle(
                            ui,
                            handle_2d,
                            is_dragging_this,
                            is_armed_this,
                        )
                    })
                    .inner;

                if handle_resp.drag_started() {
                    self.sketch_move_target = Some(group.clone());
                    self.sketch_move_dragging = true;
                    self.sketch_move_armed = false;
                    self.sketch_move_delta = DVec2::ZERO;
                }
                if is_dragging_this && handle_resp.dragged() {
                    if let Some(pointer_pos) = handle_resp.interact_pointer_pos() {
                        if let Some(mut target_pt) = screen_to_plane_point(
                            &self.camera,
                            rect,
                            pointer_pos,
                            &self.active_plane,
                        ) {
                            let tol = pixel_tolerance_to_world(&self.camera, rect) * 14.0;
                            if let Some(hit) =
                                find_snap(self.sketch(), target_pt, tol, 10.0, None)
                            {
                                target_pt = hit.point;
                            } else if let Some(region_center) =
                                self.find_region_center_snap(&group, target_pt, tol)
                            {
                                target_pt = region_center;
                            }
                            self.sketch_move_delta = target_pt - centroid;
                        }
                    }
                }
                if is_dragging_this && handle_resp.drag_stopped() {
                    self.commit_sketch_move_drag();
                }
                if handle_resp.clicked() {
                    if is_target && self.sketch_move_armed {
                        self.sketch_move_armed = false;
                        self.sketch_move_target = None;
                    } else {
                        self.sketch_move_target = Some(group.clone());
                        self.sketch_move_armed = true;
                        self.sketch_move_dragging = false;
                        self.sketch_move_delta = DVec2::ZERO;
                    }
                }
            }
        }

        if self.active_face.is_none() {
            if let Some((body_id, center)) = self.selected_single_body_center() {
                let Some(s_center) = world_to_screen_pos(&self.camera, rect, center) else {
                    return;
                };
                let world_scale = pixel_tolerance_to_world(&self.camera, rect);
                let s = (55.0 * world_scale) as f32;

                // 1. Tombol Badge "Copy" mengambang di bawah widget
                let s_copy = s_center + egui::vec2(0.0, 52.0);
                let copy_resp = CanvasHud::render_copy_toggle_badge(ui, s_copy, self.body_copy_mode);
                if copy_resp.clicked() {
                    self.body_copy_mode = !self.body_copy_mode;
                    if self.body_copy_mode {
                        self.model_status = Some("Mode Salin Aktif — geser atau putar untuk menduplikasi objek".to_string());
                    } else {
                        self.model_status = Some("Mode Salin Nonaktif".to_string());
                    }
                }

                // 2. Posisi 2D Handle sumbu translasi
                let p_x = center + Vec3::X * (s * 1.5);
                let p_y = center + Vec3::Y * (s * 1.5);
                let p_z = center + Vec3::Z * (s * 1.5);

                let s_x = world_to_screen_pos(&self.camera, rect, p_x);
                let s_y = world_to_screen_pos(&self.camera, rect, p_y);
                let s_z = world_to_screen_pos(&self.camera, rect, p_z);

                // 3. Posisi 2D Handle kotak planar
                let p_xy = center + (Vec3::X + Vec3::Y) * (s * 0.65);
                let p_yz = center + (Vec3::Y + Vec3::Z) * (s * 0.65);
                let p_zx = center + (Vec3::Z + Vec3::X) * (s * 0.65);

                let s_xy = world_to_screen_pos(&self.camera, rect, p_xy);
                let s_yz = world_to_screen_pos(&self.camera, rect, p_yz);
                let s_zx = world_to_screen_pos(&self.camera, rect, p_zx);

                // 4. Posisi 2D Handle busur rotasi
                let p_rot_z = center + (Vec3::X + Vec3::Y).normalize() * (s * 1.05);
                let p_rot_x = center + (Vec3::Y + Vec3::Z).normalize() * (s * 1.05);
                let p_rot_y = center + (Vec3::Z + Vec3::X).normalize() * (s * 1.05);

                let s_rot_z = world_to_screen_pos(&self.camera, rect, p_rot_z);
                let s_rot_x = world_to_screen_pos(&self.camera, rect, p_rot_x);
                let s_rot_y = world_to_screen_pos(&self.camera, rect, p_rot_y);

                let mut current_hover_part: Option<TransformGizmoPart> = None;

                // Handle Translation X
                if let Some(sx) = s_x {
                    let rx = egui::Rect::from_center_size(sx, egui::Vec2::splat(22.0));
                    let resp = ui.allocate_rect(rx, egui::Sense::drag());
                    if resp.hovered() || resp.dragged() {
                        current_hover_part = Some(TransformGizmoPart::TranslateX);
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                    }
                    if resp.drag_started() {
                        self.body_move_target = Some(body_id);
                        self.body_move_dragging = true;
                        self.body_transform_part = Some(TransformGizmoPart::TranslateX);
                        self.body_move_delta = Vec3::ZERO;
                    }
                    if resp.dragged() {
                        let (dx, _) = self.project_screen_drag_to_world_axis(rect, center, Vec3::X, resp.drag_delta());
                        self.body_move_delta.x += dx as f32;
                        ui.ctx().request_repaint();
                    }
                    if resp.drag_stopped() {
                        if self.body_move_delta.length_squared() > 1e-6 {
                            self.translate_selected_body(self.body_move_delta);
                        }
                        self.body_move_dragging = false;
                        self.body_move_delta = Vec3::ZERO;
                    }
                    if self.body_move_dragging && self.body_transform_part == Some(TransformGizmoPart::TranslateX) {
                        let pill_pos = sx + egui::vec2(0.0, -24.0);
                        let val_str = format!("{:+0.1} mm", self.body_move_delta.x);
                        CanvasHud::render_interactive_dimension_pill(ui, pill_pos, &val_str, true);
                    }
                }

                // Handle Translation Y
                if let Some(sy) = s_y {
                    let ry = egui::Rect::from_center_size(sy, egui::Vec2::splat(22.0));
                    let resp = ui.allocate_rect(ry, egui::Sense::drag());
                    if resp.hovered() || resp.dragged() {
                        current_hover_part = Some(TransformGizmoPart::TranslateY);
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                    }
                    if resp.drag_started() {
                        self.body_move_target = Some(body_id);
                        self.body_move_dragging = true;
                        self.body_transform_part = Some(TransformGizmoPart::TranslateY);
                        self.body_move_delta = Vec3::ZERO;
                    }
                    if resp.dragged() {
                        let (dy, _) = self.project_screen_drag_to_world_axis(rect, center, Vec3::Y, resp.drag_delta());
                        self.body_move_delta.y += dy as f32;
                        ui.ctx().request_repaint();
                    }
                    if resp.drag_stopped() {
                        if self.body_move_delta.length_squared() > 1e-6 {
                            self.translate_selected_body(self.body_move_delta);
                        }
                        self.body_move_dragging = false;
                        self.body_move_delta = Vec3::ZERO;
                    }
                    if self.body_move_dragging && self.body_transform_part == Some(TransformGizmoPart::TranslateY) {
                        let pill_pos = sy + egui::vec2(0.0, -24.0);
                        let val_str = format!("{:+0.1} mm", self.body_move_delta.y);
                        CanvasHud::render_interactive_dimension_pill(ui, pill_pos, &val_str, true);
                    }
                }

                // Handle Translation Z
                if let Some(sz) = s_z {
                    let rz = egui::Rect::from_center_size(sz, egui::Vec2::splat(22.0));
                    let resp = ui.allocate_rect(rz, egui::Sense::drag());
                    if resp.hovered() || resp.dragged() {
                        current_hover_part = Some(TransformGizmoPart::TranslateZ);
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                    }
                    if resp.drag_started() {
                        self.body_move_target = Some(body_id);
                        self.body_move_dragging = true;
                        self.body_transform_part = Some(TransformGizmoPart::TranslateZ);
                        self.body_move_delta = Vec3::ZERO;
                    }
                    if resp.dragged() {
                        let (dz, _) = self.project_screen_drag_to_world_axis(rect, center, Vec3::Z, resp.drag_delta());
                        self.body_move_delta.z += dz as f32;
                        ui.ctx().request_repaint();
                    }
                    if resp.drag_stopped() {
                        if self.body_move_delta.length_squared() > 1e-6 {
                            self.translate_selected_body(self.body_move_delta);
                        }
                        self.body_move_dragging = false;
                        self.body_move_delta = Vec3::ZERO;
                    }
                    if self.body_move_dragging && self.body_transform_part == Some(TransformGizmoPart::TranslateZ) {
                        let pill_pos = sz + egui::vec2(0.0, -24.0);
                        let val_str = format!("{:+0.1} mm", self.body_move_delta.z);
                        CanvasHud::render_interactive_dimension_pill(ui, pill_pos, &val_str, true);
                    }
                }

                // Handle Planar XY
                if let Some(sxy) = s_xy {
                    let rxy = egui::Rect::from_center_size(sxy, egui::Vec2::splat(18.0));
                    let resp = ui.allocate_rect(rxy, egui::Sense::drag());
                    if resp.hovered() || resp.dragged() {
                        current_hover_part = Some(TransformGizmoPart::PlaneXY);
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Move);
                    }
                    if resp.drag_started() {
                        self.body_move_target = Some(body_id);
                        self.body_move_dragging = true;
                        self.body_transform_part = Some(TransformGizmoPart::PlaneXY);
                        self.body_move_delta = Vec3::ZERO;
                    }
                    if resp.dragged() {
                        let (dx, _) = self.project_screen_drag_to_world_axis(rect, center, Vec3::X, resp.drag_delta());
                        let (dy, _) = self.project_screen_drag_to_world_axis(rect, center, Vec3::Y, resp.drag_delta());
                        self.body_move_delta.x += dx as f32;
                        self.body_move_delta.y += dy as f32;
                        ui.ctx().request_repaint();
                    }
                    if resp.drag_stopped() {
                        if self.body_move_delta.length_squared() > 1e-6 {
                            self.translate_selected_body(self.body_move_delta);
                        }
                        self.body_move_dragging = false;
                        self.body_move_delta = Vec3::ZERO;
                    }
                    if self.body_move_dragging && self.body_transform_part == Some(TransformGizmoPart::PlaneXY) {
                        let pill_pos = sxy + egui::vec2(0.0, -24.0);
                        let val_str = format!("ΔX:{:+0.0} ΔY:{:+0.0}", self.body_move_delta.x, self.body_move_delta.y);
                        CanvasHud::render_interactive_dimension_pill(ui, pill_pos, &val_str, true);
                    }
                }

                // Handle Planar YZ
                if let Some(syz) = s_yz {
                    let ryz = egui::Rect::from_center_size(syz, egui::Vec2::splat(18.0));
                    let resp = ui.allocate_rect(ryz, egui::Sense::drag());
                    if resp.hovered() || resp.dragged() {
                        current_hover_part = Some(TransformGizmoPart::PlaneYZ);
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Move);
                    }
                    if resp.drag_started() {
                        self.body_move_target = Some(body_id);
                        self.body_move_dragging = true;
                        self.body_transform_part = Some(TransformGizmoPart::PlaneYZ);
                        self.body_move_delta = Vec3::ZERO;
                    }
                    if resp.dragged() {
                        let (dy, _) = self.project_screen_drag_to_world_axis(rect, center, Vec3::Y, resp.drag_delta());
                        let (dz, _) = self.project_screen_drag_to_world_axis(rect, center, Vec3::Z, resp.drag_delta());
                        self.body_move_delta.y += dy as f32;
                        self.body_move_delta.z += dz as f32;
                        ui.ctx().request_repaint();
                    }
                    if resp.drag_stopped() {
                        if self.body_move_delta.length_squared() > 1e-6 {
                            self.translate_selected_body(self.body_move_delta);
                        }
                        self.body_move_dragging = false;
                        self.body_move_delta = Vec3::ZERO;
                    }
                    if self.body_move_dragging && self.body_transform_part == Some(TransformGizmoPart::PlaneYZ) {
                        let pill_pos = syz + egui::vec2(0.0, -24.0);
                        let val_str = format!("ΔY:{:+0.0} ΔZ:{:+0.0}", self.body_move_delta.y, self.body_move_delta.z);
                        CanvasHud::render_interactive_dimension_pill(ui, pill_pos, &val_str, true);
                    }
                }

                // Handle Planar ZX
                if let Some(szx) = s_zx {
                    let rzx = egui::Rect::from_center_size(szx, egui::Vec2::splat(18.0));
                    let resp = ui.allocate_rect(rzx, egui::Sense::drag());
                    if resp.hovered() || resp.dragged() {
                        current_hover_part = Some(TransformGizmoPart::PlaneZX);
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Move);
                    }
                    if resp.drag_started() {
                        self.body_move_target = Some(body_id);
                        self.body_move_dragging = true;
                        self.body_transform_part = Some(TransformGizmoPart::PlaneZX);
                        self.body_move_delta = Vec3::ZERO;
                    }
                    if resp.dragged() {
                        let (dz, _) = self.project_screen_drag_to_world_axis(rect, center, Vec3::Z, resp.drag_delta());
                        let (dx, _) = self.project_screen_drag_to_world_axis(rect, center, Vec3::X, resp.drag_delta());
                        self.body_move_delta.z += dz as f32;
                        self.body_move_delta.x += dx as f32;
                        ui.ctx().request_repaint();
                    }
                    if resp.drag_stopped() {
                        if self.body_move_delta.length_squared() > 1e-6 {
                            self.translate_selected_body(self.body_move_delta);
                        }
                        self.body_move_dragging = false;
                        self.body_move_delta = Vec3::ZERO;
                    }
                    if self.body_move_dragging && self.body_transform_part == Some(TransformGizmoPart::PlaneZX) {
                        let pill_pos = szx + egui::vec2(0.0, -24.0);
                        let val_str = format!("ΔZ:{:+0.0} ΔX:{:+0.0}", self.body_move_delta.z, self.body_move_delta.x);
                        CanvasHud::render_interactive_dimension_pill(ui, pill_pos, &val_str, true);
                    }
                }

                // Handle Rotation Z
                if let Some(srz) = s_rot_z {
                    let rrz = egui::Rect::from_center_size(srz, egui::Vec2::splat(22.0));
                    let resp = ui.allocate_rect(rrz, egui::Sense::drag());
                    if resp.hovered() || resp.dragged() {
                        current_hover_part = Some(TransformGizmoPart::RotateZ);
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                    }
                    if resp.drag_started() {
                        self.body_move_target = Some(body_id);
                        self.body_rotate_dragging = true;
                        self.body_transform_part = Some(TransformGizmoPart::RotateZ);
                        self.body_rotate_axis = Vec3::Z;
                        self.body_rotate_angle_deg = 0.0;
                    }
                    if resp.dragged() {
                        let delta = resp.drag_delta();
                        let ang_delta = (delta.x - delta.y) * 0.8;
                        self.body_rotate_angle_deg += ang_delta as f64;
                        ui.ctx().request_repaint();
                    }
                    if resp.drag_stopped() {
                        let shift = ui.input(|i| i.modifiers.shift);
                        let effective_angle = if !shift {
                            (self.body_rotate_angle_deg / 5.0).round() * 5.0
                        } else {
                            self.body_rotate_angle_deg
                        };
                        if effective_angle.abs() > 0.5 {
                            self.rotate_selected_body(Vec3::Z, effective_angle);
                        }
                        self.body_rotate_dragging = false;
                        self.body_rotate_angle_deg = 0.0;
                    }
                    if self.body_rotate_dragging && self.body_transform_part == Some(TransformGizmoPart::RotateZ) {
                        let pill_pos = srz + egui::vec2(0.0, -24.0);
                        let ang_str = format!("{:+0.1}°", self.body_rotate_angle_deg);
                        CanvasHud::render_interactive_angle_pill(ui, pill_pos, &ang_str, true);
                    }
                }

                // Handle Rotation X
                if let Some(srx) = s_rot_x {
                    let rrx = egui::Rect::from_center_size(srx, egui::Vec2::splat(22.0));
                    let resp = ui.allocate_rect(rrx, egui::Sense::drag());
                    if resp.hovered() || resp.dragged() {
                        current_hover_part = Some(TransformGizmoPart::RotateX);
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                    }
                    if resp.drag_started() {
                        self.body_move_target = Some(body_id);
                        self.body_rotate_dragging = true;
                        self.body_transform_part = Some(TransformGizmoPart::RotateX);
                        self.body_rotate_axis = Vec3::X;
                        self.body_rotate_angle_deg = 0.0;
                    }
                    if resp.dragged() {
                        let delta = resp.drag_delta();
                        let ang_delta = (delta.x + delta.y) * 0.8;
                        self.body_rotate_angle_deg += ang_delta as f64;
                        ui.ctx().request_repaint();
                    }
                    if resp.drag_stopped() {
                        let shift = ui.input(|i| i.modifiers.shift);
                        let effective_angle = if !shift {
                            (self.body_rotate_angle_deg / 5.0).round() * 5.0
                        } else {
                            self.body_rotate_angle_deg
                        };
                        if effective_angle.abs() > 0.5 {
                            self.rotate_selected_body(Vec3::X, effective_angle);
                        }
                        self.body_rotate_dragging = false;
                        self.body_rotate_angle_deg = 0.0;
                    }
                    if self.body_rotate_dragging && self.body_transform_part == Some(TransformGizmoPart::RotateX) {
                        let pill_pos = srx + egui::vec2(0.0, -24.0);
                        let ang_str = format!("{:+0.1}°", self.body_rotate_angle_deg);
                        CanvasHud::render_interactive_angle_pill(ui, pill_pos, &ang_str, true);
                    }
                }

                // Handle Rotation Y
                if let Some(sry) = s_rot_y {
                    let rry = egui::Rect::from_center_size(sry, egui::Vec2::splat(22.0));
                    let resp = ui.allocate_rect(rry, egui::Sense::drag());
                    if resp.hovered() || resp.dragged() {
                        current_hover_part = Some(TransformGizmoPart::RotateY);
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                    }
                    if resp.drag_started() {
                        self.body_move_target = Some(body_id);
                        self.body_rotate_dragging = true;
                        self.body_transform_part = Some(TransformGizmoPart::RotateY);
                        self.body_rotate_axis = Vec3::Y;
                        self.body_rotate_angle_deg = 0.0;
                    }
                    if resp.dragged() {
                        let delta = resp.drag_delta();
                        let ang_delta = (delta.x - delta.y) * 0.8;
                        self.body_rotate_angle_deg += ang_delta as f64;
                        ui.ctx().request_repaint();
                    }
                    if resp.drag_stopped() {
                        let shift = ui.input(|i| i.modifiers.shift);
                        let effective_angle = if !shift {
                            (self.body_rotate_angle_deg / 5.0).round() * 5.0
                        } else {
                            self.body_rotate_angle_deg
                        };
                        if effective_angle.abs() > 0.5 {
                            self.rotate_selected_body(Vec3::Y, effective_angle);
                        }
                        self.body_rotate_dragging = false;
                        self.body_rotate_angle_deg = 0.0;
                    }
                    if self.body_rotate_dragging && self.body_transform_part == Some(TransformGizmoPart::RotateY) {
                        let pill_pos = sry + egui::vec2(0.0, -24.0);
                        let ang_str = format!("{:+0.1}°", self.body_rotate_angle_deg);
                        CanvasHud::render_interactive_angle_pill(ui, pill_pos, &ang_str, true);
                    }
                }

                // Center Pivot Handle
                let r_center = egui::Rect::from_center_size(s_center, egui::Vec2::splat(16.0));
                let resp_center = ui.allocate_rect(r_center, egui::Sense::click_and_drag());
                if resp_center.hovered() || resp_center.dragged() {
                    current_hover_part = Some(TransformGizmoPart::CenterPivot);
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Move);
                }
                if resp_center.drag_started() {
                    self.body_move_target = Some(body_id);
                    self.body_move_dragging = true;
                    self.body_transform_part = Some(TransformGizmoPart::CenterPivot);
                    self.body_move_delta = Vec3::ZERO;
                }
                if resp_center.dragged() {
                    let ground_plane = SketchPlane {
                        origin: center,
                        ..SketchPlane::top()
                    };
                    if let Some(target_uv) = screen_to_plane_point(
                        &self.camera,
                        rect,
                        resp_center.interact_pointer_pos().unwrap_or(s_center),
                        &ground_plane,
                    ) {
                        self.body_move_delta.x = target_uv.x as f32;
                        self.body_move_delta.y = target_uv.y as f32;
                    }
                    ui.ctx().request_repaint();
                }
                if resp_center.drag_stopped() {
                    if self.body_move_delta.length_squared() > 1e-6 {
                        self.translate_selected_body(self.body_move_delta);
                    }
                    self.body_move_dragging = false;
                    self.body_move_delta = Vec3::ZERO;
                }

                if !self.body_move_dragging && !self.body_rotate_dragging {
                    self.body_transform_part = current_hover_part;
                }
            }
        }
    }
}
