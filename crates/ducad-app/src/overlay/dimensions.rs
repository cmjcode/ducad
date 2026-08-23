use ducad_core::BodyId;
use ducad_kernel::SurfaceKind;
use ducad_render::sketch::TransformGizmoPart;
use ducad_render::SketchPlane;
use ducad_sketch::{
    detect_rectangle, find_region_containing_entity, find_snap, Entity, EntityId, RectAnchor,
    ResizeRectangle, UpdateEntity,
};
use ducad_ui::{CanvasHud, ToolGuides};
use eframe::egui;
use glam::{DVec2, Vec3};
use slotmap::Key;

use crate::app::DuCADApp;
use crate::types::{RoundKind, ToolKind};
use crate::viewport::{pixel_tolerance_to_world, screen_to_plane_point, world_to_screen_pos};

impl DuCADApp {
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

    /// Terapkan hasil edit satu pill dimensi (Fase 3 — "Tampilkan Semua Ukuran" interaktif).
    /// Line yg jadi bagian dari rectangle tertutup di-resize lewat `ResizeRectangle` (P atau L,
    /// anchor Center) supaya sisi lainnya ikut konsisten, bukan cuma menggeser 1 endpoint sendirian.
    fn commit_dimension_pill_edit(&mut self, id: EntityId, new_value_mm: f64) {
        let new_value_mm = new_value_mm.max(1e-3);
        let Some(entity) = self.sketch().entities.get(id).cloned() else {
            return;
        };
        match entity {
            Entity::Line { start, end } => {
                if let Some(region) = find_region_containing_entity(self.sketch(), id) {
                    if let Some(r) = detect_rectangle(self.sketch(), &region.entity_ids) {
                        if let Some(side_idx) = r.entity_ids.iter().position(|&e| e == id) {
                            let new_lines = if side_idx % 2 == 0 {
                                r.resized_lines(new_value_mm, r.length_l, RectAnchor::Center)
                            } else {
                                r.resized_lines(r.length_p, new_value_mm, RectAnchor::Center)
                            };
                            self.execute_sketch_command(Box::new(ResizeRectangle::new(
                                "Ubah Ukuran Rectangle",
                                new_lines,
                            )));
                            return;
                        }
                    }
                }
                let dir = (end - start).normalize_or_zero();
                let new_end = start + dir * new_value_mm;
                self.execute_sketch_command(Box::new(UpdateEntity::new(
                    "Ubah Panjang Garis",
                    id,
                    Entity::Line { start, end: new_end },
                )));
            }
            Entity::Circle { center, .. } => {
                self.execute_sketch_command(Box::new(UpdateEntity::new(
                    "Ubah Radius Lingkaran",
                    id,
                    Entity::Circle { center, radius: new_value_mm },
                )));
            }
            Entity::Arc { center, start_angle, end_angle, .. } => {
                self.execute_sketch_command(Box::new(UpdateEntity::new(
                    "Ubah Radius Busur",
                    id,
                    Entity::Arc { center, radius: new_value_mm, start_angle, end_angle },
                )));
            }
            // Ellipse punya 2 angka (Rx/Ry) sekaligus — popup 1-angka tidak pas, jadi
            // sengaja tetap pill statis (non-interaktif), lihat loop render di bawah.
            Entity::Ellipse { .. } => {}
        }
    }

    /// Popup kecil "ketik angka baru" di bawah pill dimensi yg sedang diedit — persis pola
    /// popup gizmo fillet/extrude di bawah (`gizmo_dimension_editing` dkk.), cuma generik utk
    /// entity sketch mana pun lewat `commit` (di-set, bukan langsung dieksekusi, supaya
    /// `render_all_element_dimensions` tetap satu titik yg memanggil `commit_dimension_pill_edit`).
    fn show_dimension_pill_edit_popup(
        &mut self,
        ui: &mut egui::Ui,
        id: EntityId,
        pos_2d: egui::Pos2,
        commit: &mut Option<(EntityId, f64)>,
    ) {
        let popup_rect =
            egui::Rect::from_center_size(pos_2d + egui::vec2(0.0, 28.0), egui::vec2(100.0, 32.0));
        egui::Area::new(egui::Id::new(("ducad-dim-pill-edit-popup", id.data().as_ffi())))
            .fixed_pos(popup_rect.min)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    let resp = ui.text_edit_singleline(&mut self.editing_dimension_input);
                    resp.request_focus();
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.editing_dimension_entity = None;
                    } else if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if let Ok(val) = self.editing_dimension_input.trim().parse::<f64>() {
                            *commit = Some((id, self.unit.to_internal_mm(val)));
                        }
                        self.editing_dimension_entity = None;
                    } else if resp.lost_focus() {
                        self.editing_dimension_entity = None;
                    }
                });
            });
    }

    /// Radius klik (px) di sekitar pusat pill dimensi bbox body 3D — dipakai gating raycast
    /// pick vertex/edge/face (`input/sketch.rs`) supaya klik di pill tidak "diserobot" jadi
    /// pilih rusuk/sudut utk fillet/chamfer. Posisi pill SENGAJA persis di tengah rusuk bbox,
    /// yg pada body axis-aligned sederhana sering berhimpit persis dgn rusuk asli objek —
    /// akar bug "klik ukuran malah muncul gizmo rounded".
    const BODY_DIM_PILL_HIT_RADIUS_PX: f32 = 28.0;

    /// Posisi layar + panjang (mm) tiap pill dimensi bbox X/Y/Z body 3D yg SEDANG ditampilkan
    /// — kosong kalau checkbox "Tampilkan Semua Ukuran" nonaktif, sedang mode sketch, atau
    /// tidak ada tepat 1 body terpilih. Dipakai render pill itu sendiri DAN guard klik di
    /// `input/sketch.rs`, supaya logika bbox/gating cuma ada di satu tempat.
    pub fn body_dim_pill_screen_hits(&self, rect: egui::Rect) -> Vec<(usize, egui::Pos2, f64)> {
        if !self.show_all_dimensions || self.is_sketching {
            return Vec::new();
        }
        let Some((body_id, _)) = self.selected_single_body_center() else {
            return Vec::new();
        };
        let Some(geo) = self.model.geometry.get(body_id) else {
            return Vec::new();
        };
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for p in &geo.mesh.positions {
            for k in 0..3 {
                min[k] = min[k].min(p[k]);
                max[k] = max[k].max(p[k]);
            }
        }
        let world_positions = [
            Vec3::new((min[0] + max[0]) * 0.5, min[1], min[2]),
            Vec3::new(min[0], (min[1] + max[1]) * 0.5, min[2]),
            Vec3::new(min[0], min[1], (min[2] + max[2]) * 0.5),
        ];
        let lengths = [
            (max[0] - min[0]).abs() as f64,
            (max[1] - min[1]).abs() as f64,
            (max[2] - min[2]).abs() as f64,
        ];
        (0..3)
            .filter_map(|axis| {
                world_to_screen_pos(&self.camera, rect, world_positions[axis])
                    .map(|pos_2d| (axis, pos_2d, lengths[axis]))
            })
            .collect()
    }

    /// True kalau `screen_pos` (posisi klik) jatuh di dekat salah satu pill dimensi bbox body
    /// 3D yg sedang tampil — dipakai `input/sketch.rs` utk skip raycast pick vertex/edge/face
    /// SEBELUM dieksekusi, bukan sesudahnya (klik tunggal cuma boleh berarti SATU hal: edit
    /// ukuran ATAU pilih rusuk/sudut, tidak dua-duanya sekaligus).
    pub fn body_dim_pill_hit_at(&self, rect: egui::Rect, screen_pos: egui::Pos2) -> bool {
        if !self.show_all_dimensions || self.is_sketching {
            return false;
        }
        if self.body_dim_pill_screen_hits(rect)
            .iter()
            .any(|(_, pos_2d, _)| pos_2d.distance(screen_pos) < Self::BODY_DIM_PILL_HIT_RADIUS_PX)
        {
            return true;
        }
        for (id, geo) in self.model.geometry.iter() {
            let visible = self.model.doc.bodies.get(id).is_some_and(|b| b.visible);
            if !visible {
                continue;
            }
            for (mid, _, _, _) in &geo.edge_dims {
                let world_pt = Vec3::new(mid.0 as f32, mid.1 as f32, mid.2 as f32);
                if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, world_pt) {
                    if pos_2d.distance(screen_pos) < Self::BODY_DIM_PILL_HIT_RADIUS_PX {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn render_all_element_dimensions(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        const COINCIDENCE_POS_EPS: f32 = 1e-3;
        const COINCIDENCE_LEN_EPS: f64 = 1e-3;

        let mut line_anchors_2d: Vec<(Vec3, f64)> = Vec::new();
        let mut shown_3d_edges: Vec<(Vec3, f64)> = Vec::new();
        let mut edge_dim_commit: Option<(BodyId, usize, f64)> = None;

        // 1. Jika di Mode 3D (!is_sketching): gambar dimensi rusuk 3D INTERAKTIF terlebih dahulu.
        // Seluruh rusuk body (termasuk rusuk bawah yang berhimpit dengan sketch profil awal)
        // diprioritaskan sebagai dimensi 3D interaktif yang bisa langsung diedit.
        if !self.is_sketching {
            for (id, geo) in self.model.geometry.iter() {
                let visible = self.model.doc.bodies.get(id).is_some_and(|b| b.visible);
                if !visible {
                    continue;
                }
                let bid_raw = id.data().as_ffi();
                for (edge_idx, (mid, start, end, length)) in geo.edge_dims.iter().enumerate() {
                    let world_pt = Vec3::new(mid.0 as f32, mid.1 as f32, mid.2 as f32);
                    shown_3d_edges.push((world_pt, *length));

                    if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, world_pt) {
                        let start_pt = Vec3::new(start.0 as f32, start.1 as f32, start.2 as f32);
                        let end_pt = Vec3::new(end.0 as f32, end.1 as f32, end.2 as f32);
                        let angle = self.screen_angle_between_world_points(rect, start_pt, end_pt);
                        let is_editing = self.editing_edge_dim == Some((id, edge_idx));
                        let text = self.unit.format_precise(*length);
                        let resp = ui
                            .push_id(("ducad-edge-dim-pill", bid_raw, edge_idx), |ui| {
                                CanvasHud::render_interactive_dimension_pill_aligned(
                                    ui,
                                    pos_2d,
                                    angle,
                                    &text,
                                    is_editing,
                                )
                            })
                            .inner;
                        if resp.clicked() && !is_editing {
                            self.editing_edge_dim = Some((id, edge_idx));
                            self.editing_edge_dim_input =
                                format!("{:.2}", self.unit.to_display_val(*length));
                            self.selected_bodies.clear();
                            self.selected_bodies.insert(id);
                        }
                        if is_editing {
                            let popup_rect = egui::Rect::from_center_size(
                                pos_2d + egui::vec2(0.0, 28.0),
                                egui::vec2(100.0, 32.0),
                            );
                            egui::Area::new(egui::Id::new((
                                "ducad-edge-dim-edit-popup",
                                bid_raw,
                                edge_idx,
                            )))
                            .fixed_pos(popup_rect.min)
                            .order(egui::Order::Foreground)
                            .show(ui.ctx(), |ui| {
                                egui::Frame::popup(ui.style()).show(ui, |ui| {
                                    let resp =
                                        ui.text_edit_singleline(&mut self.editing_edge_dim_input);
                                    resp.request_focus();
                                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                        self.editing_edge_dim = None;
                                    } else if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                        if let Ok(val) =
                                            self.editing_edge_dim_input.trim().parse::<f64>()
                                        {
                                            let new_len_mm = self.unit.to_internal_mm(val);
                                            edge_dim_commit = Some((id, edge_idx, new_len_mm));
                                        }
                                        self.editing_edge_dim = None;
                                    } else if resp.lost_focus() {
                                        self.editing_edge_dim = None;
                                    }
                                });
                            });
                        }
                    }
                }
            }

            if let Some((b_id, e_idx, new_len_mm)) = edge_dim_commit {
                self.scale_body_by_edge(b_id, e_idx, new_len_mm);
                self.editing_edge_dim = None;
            }
        }

        // 2. Render entitas sketch 2D:
        // - Di mode sketch (`is_sketching`): interaktif & bebas diedit.
        // - Di mode 3D (`!is_sketching`): hanya digambar jika TIDAK berhimpit dengan rusuk 3D solid body.
        let entities: Vec<(EntityId, Entity)> = self
            .sketch()
            .entities
            .iter()
            .filter(|(id, _)| !self.sketch().is_hidden(*id))
            .map(|(id, e)| (id, e.clone()))
            .collect();
        let mut commit: Option<(EntityId, f64)> = None;

        for (id, entity) in &entities {
            let id = *id;
            match entity {
                Entity::Line { start, end } => {
                    let len = (*end - *start).length();
                    let mid = (*start + *end) * 0.5;
                    let label_3d = self.active_plane.to_world(mid, 0.0);
                    line_anchors_2d.push((label_3d, len));

                    // Di mode 3D, jika garis sketch sudah terwakili oleh rusuk 3D solid body, jangan gambar duplikatnya
                    if !self.is_sketching {
                        let already_covered_by_3d = shown_3d_edges.iter().any(|(anchor, elen)| {
                            (label_3d - *anchor).length() < COINCIDENCE_POS_EPS
                                && (len - elen).abs() < COINCIDENCE_LEN_EPS
                        });
                        if already_covered_by_3d {
                            continue;
                        }
                    }

                    if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, label_3d) {
                        if self.is_sketching {
                            let is_editing = self.editing_dimension_entity == Some(id);
                            let text = self.unit.format_precise(len);
                            let resp = ui
                                .push_id(("ducad-dim-pill-line", id.data().as_ffi()), |ui| {
                                    CanvasHud::render_interactive_dimension_pill(
                                        ui, pos_2d, &text, is_editing,
                                    )
                                })
                                .inner;
                            if resp.clicked() && !is_editing {
                                self.editing_dimension_entity = Some(id);
                                self.editing_dimension_input =
                                    format!("{:.2}", self.unit.to_display_val(len));
                            }
                            if is_editing {
                                self.show_dimension_pill_edit_popup(ui, id, pos_2d, &mut commit);
                            }
                        } else {
                            let angle = self.screen_line_angle(rect, *start, *end);
                            CanvasHud::render_dimension_pill_aligned(
                                ui,
                                pos_2d,
                                angle,
                                &self.unit.format_precise(len),
                            );
                        }
                    }
                }
                Entity::Circle { center, radius } => {
                    let edge_pt = *center + DVec2::new(*radius, 0.0);
                    let label_3d = self.active_plane.to_world(edge_pt, 0.0);

                    if !self.is_sketching {
                        let already_covered_by_3d = shown_3d_edges.iter().any(|(anchor, _)| {
                            (label_3d - *anchor).length() < COINCIDENCE_POS_EPS
                        });
                        if already_covered_by_3d {
                            continue;
                        }
                    }

                    if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, label_3d) {
                        let text = format!("R {}", self.unit.format_precise(*radius));
                        if self.is_sketching {
                            let is_editing = self.editing_dimension_entity == Some(id);
                            let resp = ui
                                .push_id(("ducad-dim-pill-circle", id.data().as_ffi()), |ui| {
                                    CanvasHud::render_interactive_dimension_pill(
                                        ui, pos_2d, &text, is_editing,
                                    )
                                })
                                .inner;
                            if resp.clicked() && !is_editing {
                                self.editing_dimension_entity = Some(id);
                                self.editing_dimension_input =
                                    format!("{:.2}", self.unit.to_display_val(*radius));
                            }
                            if is_editing {
                                self.show_dimension_pill_edit_popup(ui, id, pos_2d, &mut commit);
                            }
                        } else {
                            CanvasHud::render_dimension_pill(ui, pos_2d, &text, false);
                        }
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

                    if !self.is_sketching {
                        let already_covered_by_3d = shown_3d_edges.iter().any(|(anchor, _)| {
                            (label_3d - *anchor).length() < COINCIDENCE_POS_EPS
                        });
                        if already_covered_by_3d {
                            continue;
                        }
                    }

                    if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, label_3d) {
                        let text = format!("R {}", self.unit.format_precise(*radius));
                        if self.is_sketching {
                            let is_editing = self.editing_dimension_entity == Some(id);
                            let resp = ui
                                .push_id(("ducad-dim-pill-arc", id.data().as_ffi()), |ui| {
                                    CanvasHud::render_interactive_dimension_pill(
                                        ui, pos_2d, &text, is_editing,
                                    )
                                })
                                .inner;
                            if resp.clicked() && !is_editing {
                                self.editing_dimension_entity = Some(id);
                                self.editing_dimension_input =
                                    format!("{:.2}", self.unit.to_display_val(*radius));
                            }
                            if is_editing {
                                self.show_dimension_pill_edit_popup(ui, id, pos_2d, &mut commit);
                            }
                        } else {
                            CanvasHud::render_dimension_pill(ui, pos_2d, &text, false);
                        }
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

        if let Some((id, new_value_mm)) = commit {
            self.commit_dimension_pill_edit(id, new_value_mm);
            self.editing_dimension_entity = None;
        }

        // 3. Jika di mode sketch (`is_sketching`), render dimensi rusuk 3D yang TIDAK berhimpit dengan sketch
        if self.is_sketching {
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

        if self.tool == ToolKind::Revolve {
            let is_staged = self.revolve_staged_axis.is_some();
            let has_selection = !self.selected.is_empty() || self.active_face.is_some();
            if let Some(action) = CanvasHud::render_revolve_animated_guide(
                ui,
                rect,
                self.pending_points.len(),
                has_selection,
                self.revolve_angle_setting,
                &mut self.revolve_dialog.angle_input,
                self.revolve_reverse,
                is_staged,
                ui.input(|i| i.time),
            ) {
                match action {
                    ducad_ui::RevolveHudAction::SetAngle(angle) => {
                        self.revolve_angle_setting = angle;
                        self.revolve_dialog.angle_deg = angle;
                    }
                    ducad_ui::RevolveHudAction::ToggleReverse => {
                        self.revolve_reverse = !self.revolve_reverse;
                    }
                    ducad_ui::RevolveHudAction::Commit => {
                        self.commit_staged_revolve();
                    }
                    ducad_ui::RevolveHudAction::Cancel => {
                        self.cancel_staged_revolve();
                    }
                }
            }

            if is_staged {
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.commit_staged_revolve();
                } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.cancel_staged_revolve();
                }
            }
        } else if self.tool == ToolKind::Loft {
            let all_regions = ducad_sketch::region::find_closed_regions(self.sketch());
            let selected_regions: Vec<ducad_sketch::region::ClosedRegion> = all_regions
                .into_iter()
                .filter(|r| {
                    !r.entity_ids.is_empty()
                        && r.entity_ids.iter().all(|id| self.selected.contains(id))
                })
                .collect();
            let regions_count = selected_regions.len();
            let centroids_offset = if regions_count == 2 {
                Some((selected_regions[0].centroid - selected_regions[1].centroid).length())
            } else {
                None
            };
            let current_height = self.loft_height_input.trim().parse::<f64>().unwrap_or(20.0);
            let is_staged = self.loft_staged_body_id.is_some();

            if let Some(action) = CanvasHud::render_loft_top_bar_hud(
                ui,
                rect,
                regions_count,
                current_height,
                &mut self.loft_height_input,
                centroids_offset,
                self.loft_alignment_dismissed,
                self.loft_is_flipped,
                is_staged,
            ) {
                match action {
                    ducad_ui::LoftHudAction::SetHeight(h) => {
                        self.loft_height_input = format!("{:.1}", h);
                        if self.loft_staged_body_id.is_some() && regions_count == 2 {
                            self.update_staged_loft(&selected_regions);
                        }
                    }
                    ducad_ui::LoftHudAction::AlignCentroids => {
                        if regions_count == 2 {
                            let c1 = selected_regions[0].centroid;
                            let c2 = selected_regions[1].centroid;
                            let delta = c1 - c2;
                            let ids: Vec<ducad_sketch::EntityId> =
                                selected_regions[1].entity_ids.iter().copied().collect();
                            self.execute_sketch_command(Box::new(
                                ducad_sketch::TranslateEntities::new(
                                    "Satukan Titik Tengah Loft",
                                    ids,
                                    delta,
                                ),
                            ));
                            self.loft_alignment_dismissed = false;

                            let new_all = ducad_sketch::region::find_closed_regions(self.sketch());
                            let new_regions: Vec<ducad_sketch::region::ClosedRegion> = new_all
                                .into_iter()
                                .filter(|r| {
                                    !r.entity_ids.is_empty()
                                        && r.entity_ids.iter().all(|id| self.selected.contains(id))
                                })
                                .collect();
                            if self.loft_staged_body_id.is_some() && new_regions.len() == 2 {
                                self.update_staged_loft(&new_regions);
                            }
                        }
                    }
                    ducad_ui::LoftHudAction::DismissAlignmentDialog => {
                        self.loft_alignment_dismissed = true;
                    }
                    ducad_ui::LoftHudAction::ToggleFlip => {
                        self.loft_is_flipped = !self.loft_is_flipped;
                        if regions_count == 2 {
                            self.update_staged_loft(&selected_regions);
                        }
                    }
                    ducad_ui::LoftHudAction::Commit => {
                        if is_staged {
                            self.commit_staged_loft(&selected_regions);
                        } else if regions_count == 2 {
                            self.update_staged_loft(&selected_regions);
                            self.model_status = Some("✓ Loft 3D terbentuk — Anda bisa ubah tinggi, klik flip, atau tekan Selesai".to_string());
                        }
                    }
                    ducad_ui::LoftHudAction::Cancel => {
                        self.cancel_staged_loft();
                    }
                }
            }

            if regions_count == 2 {
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if is_staged {
                        self.commit_staged_loft(&selected_regions);
                    } else {
                        self.update_staged_loft(&selected_regions);
                        self.model_status = Some("✓ Loft 3D terbentuk — Anda bisa ubah tinggi, klik flip, atau tekan Selesai".to_string());
                    }
                } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.cancel_staged_loft();
                }
            }

            ToolGuides::render_tool_guide(
                ui,
                rect,
                self.tool.to_toolbar_tool(),
                self.pending_points.len(),
                !self.selected.is_empty(),
                ui.input(|i| i.time),
            );
        } else if self.tool == ToolKind::Shell {
            let has_face_selection = self.active_face.is_some();
            let current_thickness = self.shell_thickness_input.trim().parse::<f64>().unwrap_or(2.0);

            if let Some(action) = CanvasHud::render_shell_top_bar_hud(
                ui,
                rect,
                has_face_selection,
                current_thickness,
                &mut self.shell_thickness_input,
            ) {
                match action {
                    ducad_ui::ShellHudAction::SetThickness(t) => {
                        self.shell_thickness_input = format!("{:.1}", t);
                    }
                    ducad_ui::ShellHudAction::Commit => {
                        self.shell_active_face();
                    }
                    ducad_ui::ShellHudAction::Cancel => {
                        self.set_tool(ToolKind::Select);
                    }
                }
            }

            if has_face_selection {
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.shell_active_face();
                } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.set_tool(ToolKind::Select);
                }
            }
        } else if self.tool == ToolKind::Boolean {
            let selected_count = self.selected_bodies.len();

            if let Some(action) = CanvasHud::render_boolean_top_bar_hud(
                ui,
                rect,
                selected_count,
                self.boolean_op,
            ) {
                match action {
                    ducad_ui::BooleanHudAction::SelectOp(op) => {
                        self.boolean_op = op;
                    }
                    ducad_ui::BooleanHudAction::Commit => {
                        self.apply_current_boolean_op();
                    }
                    ducad_ui::BooleanHudAction::Cancel => {
                        self.set_tool(ToolKind::Select);
                    }
                }
            }

            if selected_count >= 2 {
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.apply_current_boolean_op();
                } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.set_tool(ToolKind::Select);
                }
            } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.set_tool(ToolKind::Select);
            }

            ToolGuides::render_tool_guide(
                ui,
                rect,
                self.tool.to_toolbar_tool(),
                self.pending_points.len(),
                !self.selected.is_empty(),
                ui.input(|i| i.time),
            );
        } else {
            ToolGuides::render_tool_guide(
                ui,
                rect,
                self.tool.to_toolbar_tool(),
                self.pending_points.len(),
                !self.selected.is_empty(),
                ui.input(|i| i.time),
            );
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

        if self.tool == ToolKind::Select {
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
                    self.auto_enter_3d_mode_on_extrude_drag();
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
                    egui::Area::new(egui::Id::new("ducad-gizmo-edit-popup"))
                        .fixed_pos(popup_rect.min)
                        .order(egui::Order::Foreground)
                        .show(ui.ctx(), |ui| {
                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                let resp = ui.text_edit_singleline(&mut self.gizmo_edit_input);
                                resp.request_focus();
                                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                    self.gizmo_dimension_editing = false;
                                } else if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    if let Ok(val) =
                                        self.gizmo_edit_input.trim().parse::<f64>()
                                    {
                                        self.gizmo_distance = self.unit.to_internal_mm(val);
                                        self.commit_gizmo_extrusion();
                                    }
                                    self.gizmo_dimension_editing = false;
                                } else if resp.lost_focus() {
                                    self.gizmo_dimension_editing = false;
                                }
                            });
                        });
                }
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
                    self.auto_enter_3d_mode_on_extrude_drag();
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
                    egui::Area::new(egui::Id::new("ducad-face-gizmo-edit-popup"))
                        .fixed_pos(popup_rect.min)
                        .order(egui::Order::Foreground)
                        .show(ui.ctx(), |ui| {
                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                let resp = ui.text_edit_singleline(&mut self.face_gizmo_edit_input);
                                resp.request_focus();
                                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                    self.face_gizmo_dimension_editing = false;
                                } else if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    if let Ok(val) =
                                        self.face_gizmo_edit_input.trim().parse::<f64>()
                                    {
                                        let dist = self.unit.to_internal_mm(val);
                                        self.face_gizmo_distance = dist;
                                        self.extrude_active_face(dist);
                                    }
                                    self.face_gizmo_dimension_editing = false;
                                } else if resp.lost_focus() {
                                    self.face_gizmo_dimension_editing = false;
                                }
                            });
                        });
                }
            }
        }

        if let Some((c_base, pull_dir)) = self.active_vertex_gizmo_dir() {
            let z_pos = if self.filleting_vertex_from_gizmo {
                self.vertex_gizmo_radius.abs().max(0.1) as f32
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
                    if self.vertex_gizmo_radius.abs() < Self::ROUND_SHARP_MM {
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
                    // Tarik (delta_mm > 0, menjauhi sudut) => fillet membesar.
                    // Dorong sampai lewat nol (delta_mm < 0) => jadi chamfer
                    // (potong lurus) yang membesar, bukan diklem di 0.
                    let candidate_radius = self.vertex_gizmo_radius + delta_mm;
                    if candidate_radius.abs() < Self::ROUND_SHARP_MM
                        || self
                            .round_gizmo_preview_shape(RoundKind::Vertex, candidate_radius)
                            .is_some()
                    {
                        self.vertex_gizmo_radius = candidate_radius;
                    }
                    self.vertex_gizmo_edit_input = format!(
                        "{:.1}",
                        self.unit.to_display_val(self.vertex_gizmo_radius.abs())
                    );
                }

                if handle_resp.drag_stopped() {
                    self.commit_vertex_fillet();
                    self.filleting_vertex_from_gizmo = false;
                }

                let pill_pos = handle_2d + egui::vec2(0.0, -32.0);
                let text = if self.vertex_gizmo_radius.abs() < Self::ROUND_SHARP_MM {
                    "0 (siku)".to_string()
                } else if self.vertex_gizmo_radius > 0.0 {
                    format!("R {}", self.unit.format(self.vertex_gizmo_radius))
                } else {
                    format!("C {}", self.unit.format(-self.vertex_gizmo_radius))
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
                        self.unit.to_display_val(self.vertex_gizmo_radius.abs())
                    );
                }

                if self.vertex_gizmo_dimension_editing {
                    let popup_rect = egui::Rect::from_center_size(
                        pill_pos + egui::vec2(0.0, 28.0),
                        egui::vec2(100.0, 32.0),
                    );
                    egui::Area::new(egui::Id::new("ducad-vertex-gizmo-edit-popup"))
                        .fixed_pos(popup_rect.min)
                        .order(egui::Order::Foreground)
                        .show(ui.ctx(), |ui| {
                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                let resp =
                                    ui.text_edit_singleline(&mut self.vertex_gizmo_edit_input);
                                resp.request_focus();
                                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                    self.vertex_gizmo_dimension_editing = false;
                                } else if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    if let Ok(val) =
                                        self.vertex_gizmo_edit_input.trim().parse::<f64>()
                                    {
                                        self.vertex_gizmo_radius =
                                            self.unit.to_internal_mm(val).max(0.0);
                                        self.commit_vertex_fillet();
                                    }
                                    self.vertex_gizmo_dimension_editing = false;
                                } else if resp.lost_focus() {
                                    self.vertex_gizmo_dimension_editing = false;
                                }
                            });
                        });
                }
            }
        }

        if let Some((c_base, pull_dir)) = self.active_edge_gizmo_dir() {
            let z_pos = if self.filleting_edge_from_gizmo {
                self.edge_gizmo_radius.abs().max(0.1) as f32
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
                    if self.edge_gizmo_radius.abs() < Self::ROUND_SHARP_MM {
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
                    // Tarik => fillet membesar. Dorong lewat nol => chamfer
                    // (potong lurus) yang membesar, tidak diklem di 0.
                    let candidate_radius = self.edge_gizmo_radius + delta_mm;
                    if candidate_radius.abs() < Self::ROUND_SHARP_MM
                        || self
                            .round_gizmo_preview_shape(RoundKind::Edge, candidate_radius)
                            .is_some()
                    {
                        self.edge_gizmo_radius = candidate_radius;
                    }
                    self.edge_gizmo_edit_input = format!(
                        "{:.1}",
                        self.unit.to_display_val(self.edge_gizmo_radius.abs())
                    );
                }

                if handle_resp.drag_stopped() {
                    self.commit_edge_fillet_single();
                    self.filleting_edge_from_gizmo = false;
                }

                let pill_pos = handle_2d + egui::vec2(0.0, -32.0);
                let text = if self.edge_gizmo_radius.abs() < Self::ROUND_SHARP_MM {
                    "0 (siku)".to_string()
                } else if self.edge_gizmo_radius > 0.0 {
                    format!("R {}", self.unit.format(self.edge_gizmo_radius))
                } else {
                    format!("C {}", self.unit.format(-self.edge_gizmo_radius))
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
                        self.unit.to_display_val(self.edge_gizmo_radius.abs())
                    );
                }

                if self.edge_gizmo_dimension_editing {
                    let popup_rect = egui::Rect::from_center_size(
                        pill_pos + egui::vec2(0.0, 28.0),
                        egui::vec2(100.0, 32.0),
                    );
                    egui::Area::new(egui::Id::new("ducad-edge-gizmo-edit-popup"))
                        .fixed_pos(popup_rect.min)
                        .order(egui::Order::Foreground)
                        .show(ui.ctx(), |ui| {
                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                let resp =
                                    ui.text_edit_singleline(&mut self.edge_gizmo_edit_input);
                                resp.request_focus();
                                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                    self.edge_gizmo_dimension_editing = false;
                                } else if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    if let Ok(val) =
                                        self.edge_gizmo_edit_input.trim().parse::<f64>()
                                    {
                                        self.edge_gizmo_radius =
                                            self.unit.to_internal_mm(val).max(0.0);
                                        self.commit_edge_fillet_single();
                                    }
                                    self.edge_gizmo_dimension_editing = false;
                                } else if resp.lost_focus() {
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
                let handle_id = egui::Id::new(("ducad_sketch_move_handle", key_ids));

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

        if !self.feature_pick_active() {
            if let Some((body_id, center)) = self.selected_single_body_center() {
                let Some(s_center) = world_to_screen_pos(&self.camera, rect, center) else {
                    return;
                };
                let world_scale = pixel_tolerance_to_world(&self.camera, rect);
                let s = (55.0 * world_scale) as f32;

                // 0. Pill dimensi bbox X/Y/Z langsung di objek (Fase 4 revisi UX — gantikan
                // panel X/Y/Z + tombol "Terapkan" yg gampang bikin nilai non-proporsional yg
                // diam2 ditolak). Sama persis pola pill sketch 2D: klik → popup angka → Enter
                // commit. Hanya muncul di mode 3D (`!is_sketching`, digerbang di dalam
                // `body_dim_pill_screen_hits`) — kalau ikut tampil pas sketching, posisinya
                // sering berhimpit persis dgn pill garis sketch profil (rusuk bbox axis-aligned
                // == rusuk sketch yg di-extrude jadi body itu), dua target klik yg beririsan itu
                // sumber bug "yg berubah malah sketch 2D".
                {
                    let bid_raw = body_id.data().as_ffi();
                    for (axis, pos_2d, length) in self.body_dim_pill_screen_hits(rect) {
                        let is_editing = self.editing_body_dim_axis == Some(axis);
                        let axis_label = ["X", "Y", "Z"][axis];
                        let text = format!("{axis_label} {}", self.unit.format_precise(length));
                        let resp = ui
                            .push_id(("ducad-body-dim-pill", bid_raw, axis), |ui| {
                                CanvasHud::render_interactive_dimension_pill(
                                    ui, pos_2d, &text, is_editing,
                                )
                            })
                            .inner;
                        if resp.clicked() && !is_editing {
                            self.editing_body_dim_axis = Some(axis);
                            self.editing_body_dim_input =
                                format!("{:.2}", self.unit.to_display_val(length));
                        }
                        if is_editing {
                            let popup_rect = egui::Rect::from_center_size(
                                pos_2d + egui::vec2(0.0, 28.0),
                                egui::vec2(100.0, 32.0),
                            );
                            egui::Area::new(egui::Id::new((
                                "ducad-body-dim-edit-popup",
                                bid_raw,
                                axis,
                            )))
                            .fixed_pos(popup_rect.min)
                            .order(egui::Order::Foreground)
                            .show(ui.ctx(), |ui| {
                                egui::Frame::popup(ui.style()).show(ui, |ui| {
                                    let resp =
                                        ui.text_edit_singleline(&mut self.editing_body_dim_input);
                                    resp.request_focus();
                                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                        self.editing_body_dim_axis = None;
                                    } else if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                        if let Ok(val) =
                                            self.editing_body_dim_input.trim().parse::<f64>()
                                        {
                                            let new_len_mm = self.unit.to_internal_mm(val);
                                            self.scale_selected_body_by_axis(axis, new_len_mm);
                                        }
                                        self.editing_body_dim_axis = None;
                                    } else if resp.lost_focus() {
                                        self.editing_body_dim_axis = None;
                                    }
                                });
                            });
                        }
                    }
                }

                // 1. Tombol Badge "Copy" mengambang di bawah gizmo (bukan center badan,
                // agar sejajar dengan posisi gizmo yang sudah digeser ke atas).
                // s_center dipakai sebagai fallback \u2014 gizmo_center_3d belum dihitung di sini,
                // jadi offset screen-space sederhana cukup: +110px ke bawah dari s_center.
                let s_copy = s_center + egui::vec2(0.0, 110.0);
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
                // Gunakan gizmo_center (dengan offset ke atas) dan arah camera-facing
                // agar titik hit 2D cocok dengan posisi panah yang dirender.
                let gizmo_scale = s;
                let gizmo_center_3d = center + Vec3::Z * (gizmo_scale * 0.25);
                let eye = self.camera.eye();
                let to_eye = eye - gizmo_center_3d;
                let dir_x = if to_eye.dot(Vec3::X) >= 0.0 { Vec3::X } else { Vec3::NEG_X };
                let dir_y = if to_eye.dot(Vec3::Y) >= 0.0 { Vec3::Y } else { Vec3::NEG_Y };
                let dir_z = if to_eye.dot(Vec3::Z) >= 0.0 { Vec3::Z } else { Vec3::NEG_Z };

                let p_x = gizmo_center_3d + dir_x * (s * 1.60);
                let p_y = gizmo_center_3d + dir_y * (s * 1.60);
                let p_z = gizmo_center_3d + dir_z * (s * 1.60);

                let s_x = world_to_screen_pos(&self.camera, rect, p_x);
                let s_y = world_to_screen_pos(&self.camera, rect, p_y);
                let s_z = world_to_screen_pos(&self.camera, rect, p_z);

                // 4. Posisi 2D Handle busur rotasi (camera-facing)
                let p_rot_z = gizmo_center_3d + (dir_x + dir_y).normalize() * (s * 1.08);
                let p_rot_x = gizmo_center_3d + (dir_y + dir_z).normalize() * (s * 1.08);
                let p_rot_y = gizmo_center_3d + (dir_z + dir_x).normalize() * (s * 1.08);

                let s_rot_z = world_to_screen_pos(&self.camera, rect, p_rot_z);
                let s_rot_x = world_to_screen_pos(&self.camera, rect, p_rot_x);
                let s_rot_y = world_to_screen_pos(&self.camera, rect, p_rot_y);

                let mut current_hover_part: Option<TransformGizmoPart> = None;

                // Hit rect Translation X — 44px (standar touch target Apple HIG)
                if let Some(sx) = s_x {
                    let rx = egui::Rect::from_center_size(sx, egui::Vec2::splat(44.0));
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

                // Hit rect Translation Y — 44px
                if let Some(sy) = s_y {
                    let ry = egui::Rect::from_center_size(sy, egui::Vec2::splat(44.0));
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

                // Hit rect Translation Z — 44px
                if let Some(sz) = s_z {
                    let rz = egui::Rect::from_center_size(sz, egui::Vec2::splat(44.0));
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

                // ── Rotation handles dialokasi LEBIH DAHULU (z-order lebih rendah) ────────────
                // Planar tiles akan dialokasi setelahnya, sehingga saat overlap,
                // planar tile menang (egui: widget terakhir = prioritas input tertinggi).

                // Hit rect Rotation Z — 44px
                if let Some(srz) = s_rot_z {
                    let rrz = egui::Rect::from_center_size(srz, egui::Vec2::splat(44.0));
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

                // Hit rect Rotation X — 44px
                if let Some(srx) = s_rot_x {
                    let rrx = egui::Rect::from_center_size(srx, egui::Vec2::splat(44.0));
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

                // Hit rect Rotation Y — 44px
                if let Some(sry) = s_rot_y {
                    let rry = egui::Rect::from_center_size(sry, egui::Vec2::splat(44.0));
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

                // Center Pivot — hit area 36px, selaraskan ke gizmo_center_3d
                let s_gizmo_center = world_to_screen_pos(&self.camera, rect, gizmo_center_3d)
                    .unwrap_or(s_center);
                let r_center = egui::Rect::from_center_size(s_gizmo_center, egui::Vec2::splat(36.0));
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
