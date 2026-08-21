use ducad_core::{BodyId, Command};
use ducad_sketch::constraint::Constraint;
use ducad_sketch::{
    arc_from_three_points, find_region_at_point, find_region_containing_entity, find_snap,
    line_intersection_params_in_sketch, mirror_entity, offset_entity, project_t, trim_segments,
    ClosedRegion, DeleteEntities, Entity, EntityId, InsertEntities, ReplaceEntities, Sketch,
    TranslateEntities,
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
    let Entity::Line { start, end } = sketch.entities.get(id)?.clone() else {
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

impl DuCADApp {
    pub const LINE_CHAIN_DEGENERATE_EPS: f64 = 1e-6;

    pub fn set_tool(&mut self, tool: ToolKind) {
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
            vec![Entity::Line { start: last, end }],
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
                    .map(|i| Entity::Line {
                        start: corners[i],
                        end: corners[(i + 1) % 4],
                    })
                    .collect();
                Some(Box::new(InsertEntities::new("Persegi", lines)))
            }
            ToolKind::Circle => {
                let radius = (pts[1] - pts[0]).length();
                (radius > 1e-6).then(|| {
                    Box::new(InsertEntities::new(
                        "Lingkaran",
                        vec![Entity::Circle {
                            center: pts[0],
                            radius,
                        }],
                    )) as Box<dyn Command<Sketch>>
                })
            }
            ToolKind::Ellipse => {
                let radius_x = (pts[1].x - pts[0].x).abs();
                let radius_y = (pts[1].y - pts[0].y).abs();
                (radius_x > 1e-6 && radius_y > 1e-6).then(|| {
                    Box::new(InsertEntities::new(
                        "Ellips",
                        vec![Entity::Ellipse {
                            center: pts[0],
                            radius_x,
                            radius_y,
                        }],
                    )) as Box<dyn Command<Sketch>>
                })
            }
            ToolKind::Arc => arc_from_three_points(pts[0], pts[1], pts[2])
                .map(|e| Box::new(InsertEntities::new("Arc", vec![e])) as _),
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
                    self.alert_modal.show_error(
                        "Revolve Gagal: Sumbu Terlalu Pendek",
                        "Dua titik sumbu yang Anda klik berada di posisi yang sama atau terlalu dekat.",
                        vec![
                            "Klik dua titik yang berjarak jelas untuk membentuk garis sumbu.",
                            "Atau gunakan preset 'Sumbu Y' / 'Sumbu X' di jendela opsi Revolve.",
                        ],
                    );
                    self.model_status =
                        Some("Revolve gagal: dua titik axis sama/terlalu dekat".to_string());
                } else {
                    self.revolve_staged_axis = Some((axis_origin, axis_end));
                    self.model_status = Some("Sumbu poros terpasang. Sesuaikan sudut & arah lalu klik Terapkan (atau tekan Enter)".to_string());
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
                    self.model_undo.execute(Box::new(cmd), &mut self.model);
                    self.model_status = Some(format!(
                        "Body diduplikasi & digeser ({:.1}, {:.1}, {:.1}) mm",
                        delta.x, delta.y, delta.z
                    ));
                } else {
                    self.model_undo.execute(
                        Box::new(ReplaceGeometryCommand::new(
                            "Geser Body",
                            target_id,
                            new_geo,
                        )),
                        &mut self.model,
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
                    self.model_undo.execute(Box::new(cmd), &mut self.model);
                    self.model_status = Some(format!("Body diduplikasi & diputar {:.1}°", angle_deg));
                } else {
                    self.model_undo.execute(
                        Box::new(ReplaceGeometryCommand::new(
                            "Putar Body",
                            target_id,
                            new_geo,
                        )),
                        &mut self.model,
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
    /// cuma dukung faktor seragam (lihat `vendor/README.md` Perubahan #10), jadi 2 sumbu lain
    /// ikut proporsional otomatis (angkanya update sendiri di frame berikutnya, dihitung ulang
    /// langsung dari mesh — bukan disimpan terpisah). Pivot = centroid bbox supaya body
    /// tumbuh/menyusut simetris di tempat, tidak bergeser.
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
            self.model_status = Some("Resize body gagal: bounding box terlalu kecil".to_string());
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
                    self.model_undo.execute(Box::new(cmd), &mut self.model);
                    self.model_status =
                        Some(format!("Body diduplikasi & diresize {:.0}%", factor * 100.0));
                } else {
                    self.model_undo.execute(
                        Box::new(ReplaceGeometryCommand::new(
                            "Resize Body",
                            target_id,
                            new_geo,
                        )),
                        &mut self.model,
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
                    self.model_undo.execute(Box::new(cmd), &mut self.model);
                    self.model_status =
                        Some(format!("Body diduplikasi & diubah ukurannya ke {:.2} mm", new_length_mm));
                } else {
                    self.model_undo.execute(
                        Box::new(ReplaceGeometryCommand::new(
                            "Ubah Ukuran Rusuk",
                            body_id,
                            new_geo,
                        )),
                        &mut self.model,
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
                if ui.input(|i| i.key_pressed(egui::Key::V)) {
                    self.open_revolve_dialog();
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
            ToolKind::Select => {
                self.last_snap = None;

                if self.extruding_from_gizmo {
                    return;
                }

                let region_hit: Option<ClosedRegion> =
                    if !self.sketch().entities.is_empty() && response.hovered() {
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

                self.hovered = if region_hit.is_some() {
                    None
                } else {
                    response
                        .hovered()
                        .then(|| self.hit_test_hover(rect, response, tol))
                        .flatten()
                };

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
                    let click_pos = response
                        .hover_pos()
                        .or_else(|| ui.input(|i| i.pointer.latest_pos()))
                        .or_else(|| ui.input(|i| i.pointer.interact_pos()));

                    let face_pick_3d = if !self.is_sketching && !shift {
                        click_pos.and_then(|pos| self.pick_body_face_at_cursor(rect, pos))
                    } else {
                        None
                    };

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
                    let is_double_click = response.double_clicked()
                        || self.last_body_select_click.as_ref().is_some_and(|(last_id, last_time)| {
                            face_pick_3d.as_ref().is_some_and(|(b_id, ..)| *last_id == *b_id)
                                && now.duration_since(*last_time).as_millis() < 500
                        })
                        || (face_pick_3d.as_ref().is_some_and(|(b_id, ..)| {
                            self.active_face.as_ref().is_some_and(|(cur_id, ..)| cur_id == b_id)
                        }));

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
                    } else if let Some((b_id, ray, hit)) = face_pick_3d {
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
                            self.active_face = Some((b_id, ray, hit));
                            self.active_vertex = None;
                            self.active_edge = None;
                            self.editing_round = None;
                            self.body_move_target = None;
                            self.face_gizmo_distance = 15.0;
                            self.face_gizmo_edit_input = "15".to_string();
                            self.last_body_select_click = Some((b_id, now));
                            self.model_status = Some(
                                "Sisi (face) 3D terpilih — tarik panah gizmo atau masukkan jarak extrude".to_string(),
                            );
                        }
                    } else if let Some(reg) = region_hit {
                        self.active_face = None;
                        self.active_vertex = None;
                        self.active_edge = None;
                        self.editing_round = None;
                        if shift {
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
                        match (cycled_hit.or(self.hovered), shift) {
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
                                    if let Some((b_id, ray, hit)) =
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
                                            self.active_face = Some((b_id, ray, hit));
                                            self.active_vertex = None;
                                            self.active_edge = None;
                                            self.body_move_target = None;
                                            self.face_gizmo_distance = 15.0;
                                            self.face_gizmo_edit_input = "15".to_string();
                                            self.last_body_select_click = Some((b_id, now));
                                            self.model_status = Some("Sisi (face) 3D terpilih — tarik panah gizmo atau masukkan jarak extrude".to_string());
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
            }
            ToolKind::Line => {
                self.hovered = None;
                self.last_snap = response
                    .hovered()
                    .then(|| find_snap(self.sketch(), raw, tol, grid_step, None))
                    .flatten();
                if response.clicked() {
                    let effective = self.snapped_or(raw);
                    self.handle_line_chain_click(effective, tol);
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
                    .then(|| find_snap(self.sketch(), raw, tol, grid_step, None))
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
                        .then(|| find_snap(self.sketch(), raw, tol, grid_step, None))
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
                        if let Some(Entity::Line { start, end }) =
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
                                .map(|(s, e)| Entity::Line { start: s, end: e })
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
            | ToolKind::Loft
            | ToolKind::FilletChamfer
            | ToolKind::Shell
            | ToolKind::Boolean
            | ToolKind::SectionView
            | ToolKind::History => {
                self.last_snap = None;
            }
        }
    }
}
