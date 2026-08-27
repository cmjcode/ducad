use ducad_core::BodyId;
use ducad_kernel::SurfaceKind;
use ducad_render::sketch::TransformGizmoPart;
use ducad_render::SketchPlane;
use ducad_sketch::{
    detect_rectangle, find_region_containing_entity, find_snap, find_snap_with_exclude_set, Entity,
    EntityId, RectAnchor, ResizeRectangle, UpdateEntity,
};

use ducad_i18n::t;
use ducad_ui::{CanvasHud, MateHudAction, ToolGuides};
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
            Entity::Line { start, end, is_construction } => {
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
                    Entity::line(start, new_end).with_construction(is_construction),
                )));
            }
            Entity::Circle { center, is_construction, .. } => {
                self.execute_sketch_command(Box::new(UpdateEntity::new(
                    "Ubah Radius Lingkaran",
                    id,
                    Entity::circle(center, new_value_mm).with_construction(is_construction),
                )));
            }
            Entity::Arc { center, start_angle, end_angle, is_construction, .. } => {
                self.execute_sketch_command(Box::new(UpdateEntity::new(
                    "Ubah Radius Busur",
                    id,
                    Entity::arc(center, new_value_mm, start_angle, end_angle).with_construction(is_construction),
                )));
            }
            // Ellipse punya 2 angka (Rx/Ry) sekaligus — popup 1-angka tidak pas, jadi
            // sengaja tetap pill statis (non-interaktif), lihat loop render di bawah.
            Entity::Ellipse { .. } | Entity::Spline { .. } => {}
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
                Entity::Line { start, end, .. } => {
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
                Entity::Circle { center, radius, .. } => {
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
                    ..
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
                    ..
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
                Entity::Spline { .. } => {}
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

    /// Render panel konfigurasi Mate di luar CentralPanel menggunakan `egui::Context` langsung.
    /// Harus dipanggil setelah CentralPanel selesai di-render agar tidak terblokir oleh
    /// `Sense::click_and_drag` pada area canvas viewport.
    pub fn show_mate_hud_ctx(&mut self, ctx: &egui::Context, screen_rect: egui::Rect) {
        if self.drawing_sheet_state.is_open {
            return;
        }
        let Some(mate_kind) = self.staged_mate_kind.clone() else {
            return;
        };

        let (name, is_dist, is_ang, is_conc) = match &mate_kind {
            ducad_core::MateKind::Concentric { .. } => (
                t!("assembly-mate-concentric"),
                false,
                false,
                true,
            ),
            ducad_core::MateKind::Coincident { .. } => (
                t!("assembly-mate-coincident"),
                false,
                false,
                false,
            ),
            ducad_core::MateKind::Distance { .. } => (
                t!("assembly-mate-distance"),
                true,
                false,
                false,
            ),
            ducad_core::MateKind::Angle { .. } => (
                t!("assembly-mate-angle"),
                false,
                true,
                false,
            ),
        };

        // Panggil langsung dengan ctx — tidak perlu wrapper Area tambahan.
        // render_header_hud_container_ctx sudah membuat Area-nya sendiri di Order::Foreground.
        if let Some(action) = CanvasHud::show_mate_config_panel_ctx(
            ctx,
            screen_rect,
            &name,
            &mut self.mate_offset_distance,
            &mut self.mate_angle_deg,
            &mut self.mate_flip_alignment,
            &mut self.mate_lock_rotation,
            is_dist,
            is_ang,
            is_conc,
        ) {
            match action {
                MateHudAction::SetOffset(d) => self.mate_offset_distance = d,
                MateHudAction::SetAngle(a) => self.mate_angle_deg = a,
                MateHudAction::ToggleFlip => {}
                MateHudAction::ToggleLockRotation => {}
                MateHudAction::Commit => {
                    match &mate_kind {
                        ducad_core::MateKind::Concentric { .. } => {
                            self.staged_mate_kind = Some(ducad_core::MateKind::Concentric {
                                aligned: !self.mate_flip_alignment,
                                lock_rotation: self.mate_lock_rotation,
                            });
                        }
                        ducad_core::MateKind::Coincident { .. } => {
                            self.staged_mate_kind =
                                Some(ducad_core::MateKind::Coincident {
                                    opposite_normal: !self.mate_flip_alignment,
                                });
                        }
                        ducad_core::MateKind::Distance { .. } => {
                            self.staged_mate_kind = Some(ducad_core::MateKind::Distance {
                                offset: self.mate_offset_distance,
                                opposite_normal: !self.mate_flip_alignment,
                            });
                        }
                        ducad_core::MateKind::Angle { .. } => {
                            self.staged_mate_kind = Some(ducad_core::MateKind::Angle {
                                angle_deg: self.mate_angle_deg,
                                opposite_normal: !self.mate_flip_alignment,
                            });
                        }
                    }
                    self.apply_staged_mate();
                }
                MateHudAction::Cancel => {
                    self.staged_mate_kind = None;
                    self.staged_mate_targets.clear();
                }
            }
        }


        // Keyboard shortcuts: Enter untuk commit, Escape untuk cancel
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            match &mate_kind {
                ducad_core::MateKind::Concentric { .. } => {
                    self.staged_mate_kind = Some(ducad_core::MateKind::Concentric {
                        aligned: !self.mate_flip_alignment,
                        lock_rotation: self.mate_lock_rotation,
                    });
                }
                ducad_core::MateKind::Coincident { .. } => {
                    self.staged_mate_kind = Some(ducad_core::MateKind::Coincident {
                        opposite_normal: !self.mate_flip_alignment,
                    });
                }
                ducad_core::MateKind::Distance { .. } => {
                    self.staged_mate_kind = Some(ducad_core::MateKind::Distance {
                        offset: self.mate_offset_distance,
                        opposite_normal: !self.mate_flip_alignment,
                    });
                }
                ducad_core::MateKind::Angle { .. } => {
                    self.staged_mate_kind = Some(ducad_core::MateKind::Angle {
                        angle_deg: self.mate_angle_deg,
                        opposite_normal: !self.mate_flip_alignment,
                    });
                }
            }
            self.apply_staged_mate();
        } else if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.staged_mate_kind = None;
            self.staged_mate_targets.clear();
        }
    }

    pub fn dynamic_input_ui(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        raw_cursor: Option<DVec2>,
    ) {
        if self.drawing_sheet_state.is_open {
            return;
        }
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

        // Mate HUD sekarang di-render oleh show_mate_hud_ctx() di luar CentralPanel
        // agar klik tombol Apply tidak terblokir oleh Sense::click_and_drag canvas.

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
            let has_face_selection = self.active_face.is_some() || !self.selected_faces.is_empty() || !self.selected_bodies.is_empty();
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
                    ducad_ui::ShellHudAction::ToggleVariableMode => {
                        self.shell_is_variable_mode = !self.shell_is_variable_mode;
                    }
                    ducad_ui::ShellHudAction::Commit => {
                        if !self.shell_variable_faces.is_empty() || self.shell_is_variable_mode {
                            self.shell_variable_selected_body();
                        } else if self.active_face.is_some() {
                            self.shell_active_face();
                        } else {
                            self.shell_selected_body();
                        }
                    }
                    ducad_ui::ShellHudAction::Cancel => {
                        self.set_tool(ToolKind::Select);
                    }
                }
            }

            if has_face_selection {
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if !self.shell_variable_faces.is_empty() || self.shell_is_variable_mode {
                        self.shell_variable_selected_body();
                    } else if self.active_face.is_some() {
                        self.shell_active_face();
                    } else {
                        self.shell_selected_body();
                    }
                } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.set_tool(ToolKind::Select);
                }
            }
        } else if self.tool == ToolKind::Rib {
            let has_target = !self.selected_bodies.is_empty() || self.active_face.is_some() || !self.selected.is_empty();

            if let Some(action) = CanvasHud::render_rib_top_bar_hud(
                ui,
                rect,
                has_target,
                &mut self.rib_angle_input,
                &mut self.rib_thickness_input,
                &mut self.rib_depth_input,
                &mut self.rib_draft_input,
            ) {
                match action {
                    ducad_ui::RibHudAction::SetThickness(t) => {
                        self.rib_thickness_input = format!("{:.1}", t);
                    }
                    ducad_ui::RibHudAction::SetDepth(d) => {
                        self.rib_depth_input = format!("{:.1}", d);
                    }
                    ducad_ui::RibHudAction::SetDraftAngle(a) => {
                        self.rib_draft_input = format!("{:.1}", a);
                    }
                    ducad_ui::RibHudAction::SetAngle(a) => {
                        self.rib_angle_input = format!("{:.1}", a);
                    }
                    ducad_ui::RibHudAction::Commit => {
                        self.apply_rib_to_body();
                    }
                    ducad_ui::RibHudAction::Cancel => {
                        self.set_tool(ToolKind::Select);
                    }
                }
            }

            if has_target {
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.apply_rib_to_body();
                } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.set_tool(ToolKind::Select);
                }
            } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.set_tool(ToolKind::Select);
            }
        } else if self.tool == ToolKind::DraftAngle {
            let selected_count = if !self.selected_faces.is_empty() {
                self.selected_faces.len()
            } else if self.active_face.is_some() {
                1
            } else {
                0
            };
            let current_angle = self.draft_angle_input.trim().parse::<f64>().unwrap_or(3.0);

            if let Some(action) = CanvasHud::render_draft_top_bar_hud(
                ui,
                rect,
                selected_count,
                current_angle,
                &mut self.draft_angle_input,
                &mut self.draft_pull_dir,
            ) {
                match action {
                    ducad_ui::DraftHudAction::SetAngle(a) => {
                        self.draft_angle_input = format!("{:.1}", a);
                    }
                    ducad_ui::DraftHudAction::SetPullDir(dir) => {
                        self.draft_pull_dir = dir;
                    }
                    ducad_ui::DraftHudAction::Commit => {
                        let (px, py, pz) = self.draft_pull_dir.to_vec();
                        self.apply_draft_angle(current_angle, (px, py, pz));
                    }
                    ducad_ui::DraftHudAction::Cancel => {
                        self.set_tool(ToolKind::Select);
                    }
                }
            }

            if selected_count > 0 {
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let (px, py, pz) = self.draft_pull_dir.to_vec();
                    self.apply_draft_angle(current_angle, (px, py, pz));
                } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.set_tool(ToolKind::Select);
                }
            }
        } else if self.tool == ToolKind::SplitBody {
            let has_target_body = !self.selected_bodies.is_empty() || self.active_face.is_some();
            let current_offset = self.split_offset_input.trim().parse::<f64>().unwrap_or(0.0);

            if let Some(action) = CanvasHud::render_split_top_bar_hud(
                ui,
                rect,
                has_target_body,
                &mut self.split_mode,
                &mut self.split_plane,
                current_offset,
                &mut self.split_offset_input,
            ) {
                match action {
                    ducad_ui::SplitHudAction::SetMode(m) => {
                        self.split_mode = m;
                    }
                    ducad_ui::SplitHudAction::SetPlane(pln) => {
                        self.split_plane = pln;
                    }
                    ducad_ui::SplitHudAction::SetOffset(off) => {
                        self.split_offset_input = format!("{:.1}", off);
                    }
                    ducad_ui::SplitHudAction::Commit => {
                        self.apply_split(self.split_mode, self.split_plane, current_offset);
                    }
                    ducad_ui::SplitHudAction::Cancel => {
                        self.set_tool(ToolKind::Select);
                    }
                }
            }

            if has_target_body {
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.apply_split(self.split_mode, self.split_plane, current_offset);
                } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.set_tool(ToolKind::Select);
                }
            } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.set_tool(ToolKind::Select);
            }

            // Visual 3D Preview Bidang Pemotong (Cutting Plane)
            let target_body_id = self.selected_bodies.iter().next().copied()
                .or_else(|| self.active_face.as_ref().map(|(id, _, _)| *id));

            if let Some(target_id) = target_body_id {
                if let Some(geo) = self.model.geometry.get(target_id) {
                    let center = geo.mesh.center();
                    let (min, max) = geo.mesh.bounding_box().unwrap_or(([-30.0, -30.0, -30.0], [30.0, 30.0, 30.0]));
                    let span_x = (max[0] - min[0]).abs().max(40.0) * 0.75;
                    let span_y = (max[1] - min[1]).abs().max(40.0) * 0.75;
                    let span_z = (max[2] - min[2]).abs().max(40.0) * 0.75;
                    let span = span_x.max(span_y).max(span_z);

                    let off = current_offset as f32;
                    let (corners_3d, plane_center_3d) = match self.split_plane {
                        ducad_ui::SplitPlaneKind::XY => {
                            let z = center[2] + off;
                            let p1 = glam::vec3(center[0] - span, center[1] - span, z);
                            let p2 = glam::vec3(center[0] + span, center[1] - span, z);
                            let p3 = glam::vec3(center[0] + span, center[1] + span, z);
                            let p4 = glam::vec3(center[0] - span, center[1] + span, z);
                            ([p1, p2, p3, p4], glam::vec3(center[0], center[1], z))
                        }
                        ducad_ui::SplitPlaneKind::XZ => {
                            let y = center[1] + off;
                            let p1 = glam::vec3(center[0] - span, y, center[2] - span);
                            let p2 = glam::vec3(center[0] + span, y, center[2] - span);
                            let p3 = glam::vec3(center[0] + span, y, center[2] + span);
                            let p4 = glam::vec3(center[0] - span, y, center[2] + span);
                            ([p1, p2, p3, p4], glam::vec3(center[0], y, center[2]))
                        }
                        ducad_ui::SplitPlaneKind::YZ => {
                            let x = center[0] + off;
                            let p1 = glam::vec3(x, center[1] - span, center[2] - span);
                            let p2 = glam::vec3(x, center[1] + span, center[2] - span);
                            let p3 = glam::vec3(x, center[1] + span, center[2] + span);
                            let p4 = glam::vec3(x, center[1] - span, center[2] + span);
                            ([p1, p2, p3, p4], glam::vec3(x, center[1], center[2]))
                        }
                        ducad_ui::SplitPlaneKind::PickedFace => {
                            let normal = if let Some((_, _, hit)) = &self.active_face {
                                glam::vec3(hit.normal.0 as f32, hit.normal.1 as f32, hit.normal.2 as f32).normalize()
                            } else {
                                glam::vec3(0.0, 0.0, 1.0)
                            };
                            let c_pos = glam::vec3(center[0], center[1], center[2]) + normal * off;
                            let up = if normal.z.abs() < 0.9 { glam::Vec3::Z } else { glam::Vec3::Y };
                            let u_axis = normal.cross(up).normalize() * span;
                            let v_axis = normal.cross(u_axis).normalize() * span;
                            let p1 = c_pos - u_axis - v_axis;
                            let p2 = c_pos + u_axis - v_axis;
                            let p3 = c_pos + u_axis + v_axis;
                            let p4 = c_pos - u_axis + v_axis;
                            ([p1, p2, p3, p4], c_pos)
                        }
                    };

                    let s1 = world_to_screen_pos(&self.camera, rect, corners_3d[0]);
                    let s2 = world_to_screen_pos(&self.camera, rect, corners_3d[1]);
                    let s3 = world_to_screen_pos(&self.camera, rect, corners_3d[2]);
                    let s4 = world_to_screen_pos(&self.camera, rect, corners_3d[3]);

                    if let (Some(p1), Some(p2), Some(p3), Some(p4)) = (s1, s2, s3, s4) {
                        let painter = ui.painter();
                        let poly = vec![p1, p2, p3, p4];
                        painter.add(egui::Shape::convex_polygon(
                            poly,
                            egui::Color32::from_rgba_premultiplied(0, 140, 255, 45),
                            egui::Stroke::new(1.8, egui::Color32::from_rgb(0, 160, 255)),
                        ));

                        // Diagonal cross grid
                        painter.line_segment([p1, p3], egui::Stroke::new(0.8, egui::Color32::from_rgba_premultiplied(0, 160, 255, 80)));
                        painter.line_segment([p2, p4], egui::Stroke::new(0.8, egui::Color32::from_rgba_premultiplied(0, 160, 255, 80)));

                        if let Some(s_center) = world_to_screen_pos(&self.camera, rect, plane_center_3d) {
                            let action_name = match self.split_mode {
                                ducad_ui::SplitMode::SplitBody => ducad_i18n::t!("popup-split-apply"),
                                ducad_ui::SplitMode::SplitFace => ducad_i18n::t!("popup-split-apply-face"),
                            };
                            let label = format!("✂ {} ({:+0.1} mm)", action_name, current_offset);
                            let gal = painter.layout_no_wrap(
                                label,
                                egui::FontId::proportional(11.0),
                                egui::Color32::WHITE,
                            );
                            let bg_r = egui::Rect::from_center_size(s_center, gal.size() + egui::vec2(12.0, 6.0));
                            painter.rect_filled(bg_r, 4.0, egui::Color32::from_rgba_premultiplied(10, 20, 35, 230));
                            painter.rect_stroke(bg_r, 4.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(0, 160, 255)), egui::StrokeKind::Inside);
                            painter.galley(bg_r.min + egui::vec2(6.0, 3.0), gal, egui::Color32::WHITE);
                        }
                    }
                }
            }
        } else if self.tool == ToolKind::DatumPlane {
            let offset_val = self.datum_offset_input.trim().parse::<f64>().unwrap_or(20.0);
            let angle_val = self.datum_angle_input.trim().parse::<f64>().unwrap_or(45.0);
            let plane_names: Vec<(usize, String)> = self.all_planes().into_iter().map(|(i, _, n)| (i, n)).collect();
            let points_count = self.datum_selected_points.len();
            let has_edge = !self.selected_edges.is_empty();
            let has_face = self.active_face.is_some();

            if let Some(action) = CanvasHud::render_datum_plane_hud(
                ui,
                rect,
                &mut self.datum_mode,
                &mut self.datum_base_plane_idx,
                &plane_names,
                offset_val,
                &mut self.datum_offset_input,
                angle_val,
                &mut self.datum_angle_input,
                self.datum_flip,
                points_count,
                has_edge,
                has_face,
            ) {
                match action {
                    ducad_ui::DatumPlaneHudAction::SetMode(m) => {
                        self.datum_mode = m;
                    }
                    ducad_ui::DatumPlaneHudAction::SetBasePlane(idx) => {
                        self.datum_base_plane_idx = idx;
                    }
                    ducad_ui::DatumPlaneHudAction::SetOffset(off) => {
                        self.datum_offset_input = format!("{:.1}", off);
                    }
                    ducad_ui::DatumPlaneHudAction::SetAngle(ang) => {
                        self.datum_angle_input = format!("{:.1}", ang);
                    }
                    ducad_ui::DatumPlaneHudAction::ToggleFlip => {
                        self.datum_flip = !self.datum_flip;
                    }
                    ducad_ui::DatumPlaneHudAction::ClearPoints => {
                        self.datum_selected_points.clear();
                    }
                    ducad_ui::DatumPlaneHudAction::Commit => {
                        self.apply_create_datum_plane();
                    }
                    ducad_ui::DatumPlaneHudAction::Cancel => {
                        self.set_tool(ToolKind::Select);
                    }
                }
            }

            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.apply_create_datum_plane();
            } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.set_tool(ToolKind::Select);
            }

            // Visual 3D Preview of candidate plane
            let candidate_plane: Option<SketchPlane> = match self.datum_mode {
                ducad_ui::DatumPlaneMode::Offset => {
                    let dist = if self.datum_flip { -offset_val } else { offset_val } as f32;
                    if let Some((_, _, hit)) = &self.active_face {
                        let origin = glam::vec3(hit.hit_point.0 as f32, hit.hit_point.1 as f32, hit.hit_point.2 as f32);
                        let norm = glam::vec3(hit.normal.0 as f32, hit.normal.1 as f32, hit.normal.2 as f32);
                        Some(SketchPlane::from_face_offset(origin, norm, dist))
                    } else {
                        let base = self.plane_for_index(self.datum_base_plane_idx);
                        Some(base.offset(dist))
                    }
                }
                ducad_ui::DatumPlaneMode::Angled => {
                    let ang = if self.datum_flip { -angle_val } else { angle_val } as f32;
                    if let Some(edge) = self.selected_edges.first() {
                        let p1 = edge.polyline.first().map(|&(x, y, z)| glam::vec3(x as f32, y as f32, z as f32)).unwrap_or(glam::Vec3::ZERO);
                        let p2 = edge.polyline.last().map(|&(x, y, z)| glam::vec3(x as f32, y as f32, z as f32)).unwrap_or(glam::Vec3::new(50.0, 0.0, 0.0));
                        let ref_norm = glam::Vec3::Z;
                        Some(SketchPlane::from_angle_and_edge(p1, p2, ref_norm, ang))
                    } else {
                        Some(SketchPlane::from_angle_and_edge(glam::Vec3::ZERO, glam::Vec3::new(50.0, 0.0, 0.0), glam::Vec3::Z, ang))
                    }
                }
                ducad_ui::DatumPlaneMode::ThreePoints => {
                    if self.datum_selected_points.len() >= 3 {
                        SketchPlane::from_3_points(
                            self.datum_selected_points[0],
                            self.datum_selected_points[1],
                            self.datum_selected_points[2],
                        )
                    } else {
                        None
                    }
                }
            };

            if self.datum_mode == ducad_ui::DatumPlaneMode::ThreePoints {
                let painter = ui.painter();
                let mut screen_pts = Vec::new();
                for (i, pt) in self.datum_selected_points.iter().enumerate() {
                    if let Some(sp) = crate::viewport::world_to_screen_pos(&self.camera, rect, *pt) {
                        screen_pts.push(sp);
                        // Glowing outer pin
                        painter.circle_stroke(sp, 11.0, egui::Stroke::new(2.5, egui::Color32::from_rgb(0, 180, 255)));
                        painter.circle_filled(sp, 8.5, egui::Color32::from_rgb(15, 25, 45));
                        let label = format!("P{}", i + 1);
                        let gal = painter.layout_no_wrap(label, egui::FontId::proportional(11.0), egui::Color32::WHITE);
                        painter.galley(sp - gal.size() * 0.5, gal, egui::Color32::WHITE);
                    }
                }
                if screen_pts.len() >= 2 {
                    for w in screen_pts.windows(2) {
                        painter.line_segment([w[0], w[1]], egui::Stroke::new(2.0, egui::Color32::from_rgba_unmultiplied(0, 200, 255, 200)));
                    }
                    if screen_pts.len() == 3 {
                        painter.line_segment([screen_pts[2], screen_pts[0]], egui::Stroke::new(2.0, egui::Color32::from_rgba_unmultiplied(0, 200, 255, 200)));
                    }
                }
            }

            if let Some(plane) = candidate_plane {
                let half_extent = ducad_render::grid::INACTIVE_PLANE_HALF_EXTENT;
                let c_top_left = plane.to_world(glam::DVec2::new(-half_extent as f64, -half_extent as f64), 0.0);
                let c_top_right = plane.to_world(glam::DVec2::new(half_extent as f64, -half_extent as f64), 0.0);
                let c_bot_right = plane.to_world(glam::DVec2::new(half_extent as f64, half_extent as f64), 0.0);
                let c_bot_left = plane.to_world(glam::DVec2::new(-half_extent as f64, half_extent as f64), 0.0);

                let scr1 = crate::viewport::world_to_screen_pos(&self.camera, rect, c_top_left);
                let scr2 = crate::viewport::world_to_screen_pos(&self.camera, rect, c_top_right);
                let scr3 = crate::viewport::world_to_screen_pos(&self.camera, rect, c_bot_right);
                let scr4 = crate::viewport::world_to_screen_pos(&self.camera, rect, c_bot_left);

                if let (Some(s1), Some(s2), Some(s3), Some(s4)) = (scr1, scr2, scr3, scr4) {
                    let painter = ui.painter();
                    // Semi-transparent quad fill
                    painter.add(egui::Shape::convex_polygon(
                        vec![s1, s2, s3, s4],
                        egui::Color32::from_rgba_unmultiplied(0, 180, 255, 35),
                        egui::Stroke::new(1.5, egui::Color32::from_rgb(0, 200, 255)),
                    ));

                    // Label badge interaktif dengan dukungan drag
                    let center_pos = egui::pos2((s1.x + s2.x + s3.x + s4.x) * 0.25, (s1.y + s2.y + s3.y + s4.y) * 0.25);
                    let label_text = match self.datum_mode {
                        ducad_ui::DatumPlaneMode::Offset => format!("✨ Datum Plane ({:+0.1} mm) ↕ Drag", offset_val),
                        ducad_ui::DatumPlaneMode::Angled => format!("✨ Datum Plane ({:0.1}°) ↕ Drag", angle_val),
                        ducad_ui::DatumPlaneMode::ThreePoints => "✨ 3-Point Datum Plane Preview".to_string(),
                    };
                    let gal = painter.layout_no_wrap(label_text, egui::FontId::proportional(11.0), egui::Color32::WHITE);
                    let bg_r = egui::Rect::from_center_size(center_pos, gal.size() + egui::vec2(14.0, 8.0));
                    painter.rect_filled(bg_r, 4.0, egui::Color32::from_rgba_unmultiplied(15, 25, 40, 230));
                    painter.rect_stroke(bg_r, 4.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(0, 180, 255)), egui::StrokeKind::Inside);
                    painter.galley(bg_r.min + egui::vec2(7.0, 4.0), gal, egui::Color32::WHITE);

                    // Dukungan Drag Langsung pada Viewport
                    if self.datum_mode == ducad_ui::DatumPlaneMode::Offset {
                        let handle_id = egui::Id::new("datum_offset_viewport_drag");
                        let handle_resp = ui.interact(bg_r, handle_id, egui::Sense::drag());
                        if handle_resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                        }
                        if handle_resp.dragged() {
                            let delta = -handle_resp.drag_delta().y as f64 * 0.3;
                            let mut cur = self.datum_offset_input.trim().parse::<f64>().unwrap_or(20.0);
                            cur = (cur + delta).clamp(-500.0, 500.0);
                            self.datum_offset_input = format!("{:.1}", cur);
                        }
                    } else if self.datum_mode == ducad_ui::DatumPlaneMode::Angled {
                        let handle_id = egui::Id::new("datum_angle_viewport_drag");
                        let handle_resp = ui.interact(bg_r, handle_id, egui::Sense::drag());
                        if handle_resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                        }
                        if handle_resp.dragged() {
                            let delta = -handle_resp.drag_delta().y as f64 * 0.5;
                            let mut cur = self.datum_angle_input.trim().parse::<f64>().unwrap_or(45.0);
                            cur = (cur + delta).clamp(-360.0, 360.0);
                            self.datum_angle_input = format!("{:.1}", cur);
                        }
                    }
                }
            }

            ToolGuides::render_datum_plane_guide(
                ui,
                rect,
                self.datum_mode,
                self.datum_selected_points.len(),
                self.active_face.is_some() || !self.selected_edges.is_empty(),
                ui.input(|i| i.time),
            );
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
        } else if self.tool == ToolKind::Pattern {
            let is_3d = !self.is_sketching;
            let has_selection = if is_3d {
                !self.selected_bodies.is_empty()
            } else {
                !self.selected.is_empty()
            };

            if let Some(action) = CanvasHud::render_pattern_top_bar_hud(
                ui,
                rect,
                is_3d,
                has_selection,
                &mut self.pattern_kind,
                &mut self.pattern_count_x,
                &mut self.pattern_pitch_x,
                &mut self.pattern_count_y,
                &mut self.pattern_pitch_y,
                &mut self.pattern_count_z,
                &mut self.pattern_pitch_z,
                &mut self.pattern_circ_count,
                &mut self.pattern_circ_angle_deg,
                &mut self.pattern_circ_radius,
                &mut self.pattern_circ_axis,
            ) {
                match action {
                    ducad_ui::PatternHudAction::SetKind(k) => {
                        self.pattern_kind = k;
                    }
                    ducad_ui::PatternHudAction::SetAxis(ax) => {
                        self.pattern_circ_axis = ax;
                    }
                    ducad_ui::PatternHudAction::Commit => {
                        if is_3d {
                            self.apply_pattern_3d();
                        } else {
                            self.apply_pattern_2d();
                        }
                    }
                    ducad_ui::PatternHudAction::Cancel => {
                        self.set_tool(ToolKind::Select);
                    }
                }
            }

            if has_selection {
                // Compute Centroid
                let centroid_3d = if is_3d {
                    let mut sum_c = Vec3::ZERO;
                    let mut count = 0;
                    for &bid in &self.selected_bodies {
                        if let Some(geo) = self.model.geometry.get(bid) {
                            let c = geo.mesh.center();
                            sum_c += Vec3::new(c[0], c[1], c[2]);
                            count += 1;
                        }
                    }
                    if count > 0 { sum_c / (count as f32) } else { Vec3::ZERO }
                } else {
                    let entities: Vec<ducad_sketch::Entity> = self
                        .selected
                        .iter()
                        .filter_map(|id| self.sketch().entities.get(*id).cloned())
                        .collect();
                    let c2d = ducad_sketch::compute_entities_centroid(&entities).unwrap_or(glam::DVec2::ZERO);
                    self.active_plane.to_world(c2d, 0.0)
                };

                match self.pattern_kind {
                    ducad_ui::PatternKind::Linear => {
                        // 1. AXIS X HANDLE & STEPPER
                        let dir_x = if is_3d { Vec3::X } else { self.active_plane.u_axis };
                        let handle_x_3d = centroid_3d + dir_x * (self.pattern_pitch_x as f32);
                        if let Some(handle_x_2d) = world_to_screen_pos(&self.camera, rect, handle_x_3d) {
                            let (_, arrow_vec_opt) = self.project_screen_drag_to_world_axis(rect, centroid_3d, dir_x, egui::Vec2::ZERO);
                            let resp_x = CanvasHud::render_draggable_double_arrow_handle(ui, handle_x_2d, false, arrow_vec_opt);
                            if resp_x.dragged() {
                                let (delta_mm, _) = self.project_screen_drag_to_world_axis(rect, centroid_3d, dir_x, resp_x.drag_delta());
                                self.pattern_pitch_x = (self.pattern_pitch_x + delta_mm).max(1.0);
                                self.pattern_dimension_edit_input = format!("{:.1}", self.unit.to_display_val(self.pattern_pitch_x));
                            }

                            // Stepper X (Atas)
                            let (_, new_cx) = CanvasHud::render_stepper_pill(ui, handle_x_2d + egui::vec2(0.0, -32.0), "X Qty", self.pattern_count_x, 1, 50);
                            if let Some(val) = new_cx {
                                self.pattern_count_x = val;
                            }

                            // Distance Pill X (Bawah)
                            let pill_pos_x = handle_x_2d + egui::vec2(0.0, 32.0);
                            let pill_resp_x = CanvasHud::render_interactive_dimension_pill(
                                ui,
                                pill_pos_x,
                                &format!("X: {}", self.unit.format(self.pattern_pitch_x)),
                                self.pattern_dimension_editing_x,
                            );
                            if pill_resp_x.clicked() {
                                self.pattern_dimension_editing_x = !self.pattern_dimension_editing_x;
                                self.pattern_dimension_editing_y = false;
                                self.pattern_dimension_editing_z = false;
                                self.pattern_dimension_editing_angle = false;
                                self.pattern_dimension_editing_radius = false;
                                self.pattern_dimension_edit_input = format!("{:.1}", self.unit.to_display_val(self.pattern_pitch_x));
                            }
                            if self.pattern_dimension_editing_x {
                                let popup_rect = egui::Rect::from_center_size(pill_pos_x + egui::vec2(0.0, 28.0), egui::vec2(100.0, 32.0));
                                egui::Area::new(egui::Id::new("ducad-pattern-edit-popup-x"))
                                    .fixed_pos(popup_rect.min)
                                    .order(egui::Order::Foreground)
                                    .show(ui.ctx(), |ui| {
                                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                                            let resp = ui.text_edit_singleline(&mut self.pattern_dimension_edit_input);
                                            resp.request_focus();
                                            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                                self.pattern_dimension_editing_x = false;
                                            } else if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                                if let Ok(val) = self.pattern_dimension_edit_input.trim().parse::<f64>() {
                                                    self.pattern_pitch_x = self.unit.to_internal_mm(val).max(1.0);
                                                }
                                                self.pattern_dimension_editing_x = false;
                                            } else if resp.lost_focus() {
                                                self.pattern_dimension_editing_x = false;
                                            }
                                        });
                                    });
                            }
                        }

                        // 2. AXIS Y HANDLE & STEPPER
                        let dir_y = if is_3d { Vec3::Y } else { self.active_plane.v_axis };
                        let handle_y_3d = centroid_3d + dir_y * (self.pattern_pitch_y as f32);
                        if let Some(handle_y_2d) = world_to_screen_pos(&self.camera, rect, handle_y_3d) {
                            let (_, arrow_vec_opt) = self.project_screen_drag_to_world_axis(rect, centroid_3d, dir_y, egui::Vec2::ZERO);
                            let resp_y = CanvasHud::render_draggable_double_arrow_handle(ui, handle_y_2d, false, arrow_vec_opt);
                            if resp_y.dragged() {
                                let (delta_mm, _) = self.project_screen_drag_to_world_axis(rect, centroid_3d, dir_y, resp_y.drag_delta());
                                self.pattern_pitch_y = (self.pattern_pitch_y + delta_mm).max(1.0);
                                self.pattern_dimension_edit_input = format!("{:.1}", self.unit.to_display_val(self.pattern_pitch_y));
                            }

                            // Stepper Y (Atas)
                            let (_, new_cy) = CanvasHud::render_stepper_pill(ui, handle_y_2d + egui::vec2(0.0, -32.0), "Y Qty", self.pattern_count_y, 1, 50);
                            if let Some(val) = new_cy {
                                self.pattern_count_y = val;
                            }

                            // Distance Pill Y (Bawah)
                            let pill_pos_y = handle_y_2d + egui::vec2(0.0, 32.0);
                            let pill_resp_y = CanvasHud::render_interactive_dimension_pill(
                                ui,
                                pill_pos_y,
                                &format!("Y: {}", self.unit.format(self.pattern_pitch_y)),
                                self.pattern_dimension_editing_y,
                            );
                            if pill_resp_y.clicked() {
                                self.pattern_dimension_editing_y = !self.pattern_dimension_editing_y;
                                self.pattern_dimension_editing_x = false;
                                self.pattern_dimension_editing_z = false;
                                self.pattern_dimension_editing_angle = false;
                                self.pattern_dimension_editing_radius = false;
                                self.pattern_dimension_edit_input = format!("{:.1}", self.unit.to_display_val(self.pattern_pitch_y));
                            }
                            if self.pattern_dimension_editing_y {
                                let popup_rect = egui::Rect::from_center_size(pill_pos_y + egui::vec2(0.0, 28.0), egui::vec2(100.0, 32.0));
                                egui::Area::new(egui::Id::new("ducad-pattern-edit-popup-y"))
                                    .fixed_pos(popup_rect.min)
                                    .order(egui::Order::Foreground)
                                    .show(ui.ctx(), |ui| {
                                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                                            let resp = ui.text_edit_singleline(&mut self.pattern_dimension_edit_input);
                                            resp.request_focus();
                                            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                                self.pattern_dimension_editing_y = false;
                                            } else if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                                if let Ok(val) = self.pattern_dimension_edit_input.trim().parse::<f64>() {
                                                    self.pattern_pitch_y = self.unit.to_internal_mm(val).max(1.0);
                                                }
                                                self.pattern_dimension_editing_y = false;
                                            } else if resp.lost_focus() {
                                                self.pattern_dimension_editing_y = false;
                                            }
                                        });
                                    });
                            }
                        }

                        // 3. AXIS Z HANDLE & STEPPER (Hanya jika 3D)
                        if is_3d {
                            let dir_z = Vec3::Z;
                            let handle_z_3d = centroid_3d + dir_z * (self.pattern_pitch_z as f32);
                            if let Some(handle_z_2d) = world_to_screen_pos(&self.camera, rect, handle_z_3d) {
                                let (_, arrow_vec_opt) = self.project_screen_drag_to_world_axis(rect, centroid_3d, dir_z, egui::Vec2::ZERO);
                                let resp_z = CanvasHud::render_draggable_double_arrow_handle(ui, handle_z_2d, false, arrow_vec_opt);
                                if resp_z.dragged() {
                                    let (delta_mm, _) = self.project_screen_drag_to_world_axis(rect, centroid_3d, dir_z, resp_z.drag_delta());
                                    self.pattern_pitch_z = (self.pattern_pitch_z + delta_mm).max(1.0);
                                    self.pattern_dimension_edit_input = format!("{:.1}", self.unit.to_display_val(self.pattern_pitch_z));
                                }

                                // Stepper Z (Atas)
                                let (_, new_cz) = CanvasHud::render_stepper_pill(ui, handle_z_2d + egui::vec2(0.0, -32.0), "Z Qty", self.pattern_count_z, 1, 50);
                                if let Some(val) = new_cz {
                                    self.pattern_count_z = val;
                                }

                                // Distance Pill Z (Bawah)
                                let pill_pos_z = handle_z_2d + egui::vec2(0.0, 32.0);
                                let pill_resp_z = CanvasHud::render_interactive_dimension_pill(
                                    ui,
                                    pill_pos_z,
                                    &format!("Z: {}", self.unit.format(self.pattern_pitch_z)),
                                    self.pattern_dimension_editing_z,
                                );
                                if pill_resp_z.clicked() {
                                    self.pattern_dimension_editing_z = !self.pattern_dimension_editing_z;
                                    self.pattern_dimension_editing_x = false;
                                    self.pattern_dimension_editing_y = false;
                                    self.pattern_dimension_editing_angle = false;
                                    self.pattern_dimension_editing_radius = false;
                                    self.pattern_dimension_edit_input = format!("{:.1}", self.unit.to_display_val(self.pattern_pitch_z));
                                }
                                if self.pattern_dimension_editing_z {
                                    let popup_rect = egui::Rect::from_center_size(pill_pos_z + egui::vec2(0.0, 28.0), egui::vec2(100.0, 32.0));
                                    egui::Area::new(egui::Id::new("ducad-pattern-edit-popup-z"))
                                        .fixed_pos(popup_rect.min)
                                        .order(egui::Order::Foreground)
                                        .show(ui.ctx(), |ui| {
                                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                                let resp = ui.text_edit_singleline(&mut self.pattern_dimension_edit_input);
                                                resp.request_focus();
                                                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                                    self.pattern_dimension_editing_z = false;
                                                } else if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                                    if let Ok(val) = self.pattern_dimension_edit_input.trim().parse::<f64>() {
                                                        self.pattern_pitch_z = self.unit.to_internal_mm(val).max(1.0);
                                                    }
                                                    self.pattern_dimension_editing_z = false;
                                                } else if resp.lost_focus() {
                                                    self.pattern_dimension_editing_z = false;
                                                }
                                            });
                                        });
                                }
                            }
                        }
                    }
                    ducad_ui::PatternKind::Circular => {
                        // 1. PIVOT PIN GIZMO (Draggable)
                        let pivot_3d = if is_3d {
                            self.pattern_custom_pivot_3d.unwrap_or(Vec3::ZERO)
                        } else {
                            let p2d = self.pattern_custom_pivot_2d.unwrap_or(glam::DVec2::ZERO);
                            self.active_plane.to_world(p2d, 0.0)
                        };

                        if let Some(pivot_screen) = world_to_screen_pos(&self.camera, rect, pivot_3d) {
                            let is_custom = self.pattern_custom_pivot_2d.is_some() || self.pattern_custom_pivot_3d.is_some();
                            let pivot_resp = CanvasHud::render_circular_pivot_pin(ui, pivot_screen, is_custom);

                            if pivot_resp.dragged() {
                                if !is_3d {
                                    if let Some(pointer_pos) = pivot_resp.interact_pointer_pos() {
                                        if let Some(mut pt_uv) = screen_to_plane_point(
                                            &self.camera,
                                            rect,
                                            pointer_pos,
                                            &self.active_plane,
                                        ) {
                                            let tol = pixel_tolerance_to_world(&self.camera, rect) * 14.0;
                                            if let Some(hit) = find_snap(self.sketch(), pt_uv, tol, 10.0, None) {
                                                pt_uv = hit.point;
                                                self.last_snap = Some(hit);
                                            } else {
                                                self.last_snap = None;
                                            }
                                            self.pattern_custom_pivot_2d = Some(pt_uv);
                                        }
                                    }
                                } else {
                                    let ground_plane = SketchPlane {
                                        origin: pivot_3d,
                                        ..SketchPlane::top()
                                    };
                                    if let Some(pointer_pos) = pivot_resp.interact_pointer_pos() {
                                        if let Some(mut pt_uv) = screen_to_plane_point(
                                            &self.camera,
                                            rect,
                                            pointer_pos,
                                            &ground_plane,
                                        ) {
                                            let tol = pixel_tolerance_to_world(&self.camera, rect) * 14.0;
                                            if let Some(hit) = find_snap(self.sketch(), pt_uv, tol, 10.0, None) {
                                                pt_uv = hit.point;
                                                self.last_snap = Some(hit);
                                            } else {
                                                self.last_snap = None;
                                            }
                                            self.pattern_custom_pivot_3d = Some(Vec3::new(pt_uv.x as f32, pt_uv.y as f32, pivot_3d.z));
                                        }
                                    }
                                }
                            }
                            if pivot_resp.drag_stopped() {
                                self.last_snap = None;
                            }

                            // Stepper Qty (Atas Pivot)
                            let (_, new_cc) = CanvasHud::render_stepper_pill(ui, pivot_screen + egui::vec2(0.0, -34.0), "Qty", self.pattern_circ_count, 2, 120);
                            if let Some(val) = new_cc {
                                self.pattern_circ_count = val;
                            }

                            // Sudut Pill (Bawah Pivot)
                            let pill_pos_ang = pivot_screen + egui::vec2(0.0, 34.0);
                            let pill_resp_ang = CanvasHud::render_interactive_dimension_pill(
                                ui,
                                pill_pos_ang,
                                &format!("Sudut: {:.0}°", self.pattern_circ_angle_deg),
                                self.pattern_dimension_editing_angle,
                            );
                            if pill_resp_ang.clicked() {
                                self.pattern_dimension_editing_angle = !self.pattern_dimension_editing_angle;
                                self.pattern_dimension_editing_x = false;
                                self.pattern_dimension_editing_y = false;
                                self.pattern_dimension_editing_z = false;
                                self.pattern_dimension_editing_radius = false;
                                self.pattern_dimension_edit_input = format!("{:.0}", self.pattern_circ_angle_deg);
                            }
                            if self.pattern_dimension_editing_angle {
                                let popup_rect = egui::Rect::from_center_size(pill_pos_ang + egui::vec2(0.0, 28.0), egui::vec2(100.0, 32.0));
                                egui::Area::new(egui::Id::new("ducad-pattern-edit-popup-ang"))
                                    .fixed_pos(popup_rect.min)
                                    .order(egui::Order::Foreground)
                                    .show(ui.ctx(), |ui| {
                                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                                            let resp = ui.text_edit_singleline(&mut self.pattern_dimension_edit_input);
                                            resp.request_focus();
                                            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                                self.pattern_dimension_editing_angle = false;
                                            } else if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                                if let Ok(val) = self.pattern_dimension_edit_input.trim().parse::<f64>() {
                                                    self.pattern_circ_angle_deg = val.clamp(-360.0, 360.0);
                                                }
                                                self.pattern_dimension_editing_angle = false;
                                            } else if resp.lost_focus() {
                                                self.pattern_dimension_editing_angle = false;
                                            }
                                        });
                                    });
                            }
                        }

                        // 2. RADIUS DRAG HANDLE & PILL (Pada Orbit Ring)
                        let rad_vec = centroid_3d - pivot_3d;
                        let rad_dir = if rad_vec.length_squared() > 1e-4 {
                            rad_vec.normalize()
                        } else if is_3d {
                            Vec3::X
                        } else {
                            self.active_plane.u_axis
                        };
                        let rad_handle_3d = pivot_3d + rad_dir * (self.pattern_circ_radius as f32);
                        if let Some(rad_handle_2d) = world_to_screen_pos(&self.camera, rect, rad_handle_3d) {
                            let (_, arrow_vec_opt) = self.project_screen_drag_to_world_axis(rect, pivot_3d, rad_dir, egui::Vec2::ZERO);
                            let resp_rad = CanvasHud::render_draggable_double_arrow_handle(ui, rad_handle_2d, false, arrow_vec_opt);
                            if resp_rad.dragged() {
                                let (delta_mm, _) = self.project_screen_drag_to_world_axis(rect, pivot_3d, rad_dir, resp_rad.drag_delta());
                                self.pattern_circ_radius = (self.pattern_circ_radius + delta_mm).max(0.5);
                                self.pattern_dimension_edit_input = format!("{:.1}", self.unit.to_display_val(self.pattern_circ_radius));
                            }

                            // Radius Pill (Bawah Handle Radius)
                            let pill_pos_rad = rad_handle_2d + egui::vec2(0.0, 28.0);
                            let pill_resp_rad = CanvasHud::render_interactive_dimension_pill(
                                ui,
                                pill_pos_rad,
                                &format!("Radius: {}", self.unit.format(self.pattern_circ_radius)),
                                self.pattern_dimension_editing_radius,
                            );
                            if pill_resp_rad.clicked() {
                                self.pattern_dimension_editing_radius = !self.pattern_dimension_editing_radius;
                                self.pattern_dimension_editing_x = false;
                                self.pattern_dimension_editing_y = false;
                                self.pattern_dimension_editing_z = false;
                                self.pattern_dimension_editing_angle = false;
                                self.pattern_dimension_edit_input = format!("{:.1}", self.unit.to_display_val(self.pattern_circ_radius));
                            }
                            if self.pattern_dimension_editing_radius {
                                let popup_rect = egui::Rect::from_center_size(pill_pos_rad + egui::vec2(0.0, 28.0), egui::vec2(100.0, 32.0));
                                egui::Area::new(egui::Id::new("ducad-pattern-edit-popup-rad"))
                                    .fixed_pos(popup_rect.min)
                                    .order(egui::Order::Foreground)
                                    .show(ui.ctx(), |ui| {
                                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                                            let resp = ui.text_edit_singleline(&mut self.pattern_dimension_edit_input);
                                            resp.request_focus();
                                            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                                self.pattern_dimension_editing_radius = false;
                                            } else if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                                if let Ok(val) = self.pattern_dimension_edit_input.trim().parse::<f64>() {
                                                    self.pattern_circ_radius = self.unit.to_internal_mm(val).max(0.5);
                                                }
                                                self.pattern_dimension_editing_radius = false;
                                            } else if resp.lost_focus() {
                                                self.pattern_dimension_editing_radius = false;
                                            }
                                        });
                                    });
                            }
                        }
                    }
                }

                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if is_3d {
                        self.apply_pattern_3d();
                    } else {
                        self.apply_pattern_2d();
                    }
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
        } else if self.tool == ToolKind::Sweep {
            let has_profile = self.pending_sweep_profile.is_some();
            let has_path = self.pending_sweep_path.is_some();

            if let Some(action) = CanvasHud::render_sweep_top_bar_hud(
                ui,
                rect,
                has_profile,
                has_path,
            ) {
                match action {
                    ducad_ui::SweepHudAction::Commit => {
                        self.sweep_selected();
                    }
                    ducad_ui::SweepHudAction::ResetProfile => {
                        self.pending_sweep_profile = None;
                        self.pending_sweep_path = None;
                        self.sweep_path_plane_idx = None;
                        self.selected.clear();
                        self.model_status = Some("Pilih profil 2D tertutup pada bidang manapun di kanvas.".to_string());
                    }
                    ducad_ui::SweepHudAction::Cancel => {
                        self.pending_sweep_profile = None;
                        self.pending_sweep_path = None;
                        self.sweep_path_plane_idx = None;
                        self.hovered_plane_idx = None;
                        self.selected.clear();
                        self.set_tool(ToolKind::Select);
                    }
                }
            }

            if has_profile && has_path {
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.sweep_selected();
                } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.pending_sweep_profile = None;
                    self.pending_sweep_path = None;
                    self.sweep_path_plane_idx = None;
                    self.hovered_plane_idx = None;
                    self.selected.clear();
                    self.set_tool(ToolKind::Select);
                }
            } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.pending_sweep_profile = None;
                self.pending_sweep_path = None;
                self.sweep_path_plane_idx = None;
                self.hovered_plane_idx = None;
                self.selected.clear();
                self.set_tool(ToolKind::Select);
            }

            ToolGuides::render_tool_guide(
                ui,
                rect,
                self.tool.to_toolbar_tool(),
                self.pending_points.len(),
                has_profile || has_path,
                ui.input(|i| i.time),
            );
        } else if self.tool == ToolKind::Helix {
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let (params, profile) = self.helix_popup_state.to_kernel_params([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]);
                self.create_helix_coil_with_params(params, profile);
            } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.set_tool(ToolKind::Select);
            }

            ToolGuides::render_tool_guide(
                ui,
                rect,
                self.tool.to_toolbar_tool(),
                self.pending_points.len(),
                true,
                ui.input(|i| i.time),
            );
        } else if self.tool == ToolKind::Polygon {
            if let Some(action) = CanvasHud::render_polygon_top_bar_hud(
                ui,
                rect,
                self.pending_points.len(),
                self.polygon_sides,
                self.polygon_mode,
            ) {
                match action {
                    ducad_ui::PolygonHudAction::SetSides(sides) => {
                        self.polygon_sides = sides.clamp(3, 64);
                    }
                    ducad_ui::PolygonHudAction::SetMode(mode) => {
                        self.polygon_mode = mode;
                    }
                }
            }

            ToolGuides::render_tool_guide(
                ui,
                rect,
                self.tool.to_toolbar_tool(),
                self.pending_points.len(),
                false,
                ui.input(|i| i.time),
            );
        } else if self.tool == ToolKind::Slot {
            if let Some(action) = CanvasHud::render_slot_top_bar_hud(
                ui,
                rect,
                self.pending_points.len(),
                self.slot_mode,
                self.slot_width,
            ) {
                match action {
                    ducad_ui::SlotHudAction::SetMode(mode) => {
                        self.slot_mode = mode;
                    }
                    ducad_ui::SlotHudAction::SetWidth(w) => {
                        self.slot_width = w.max(0.1);
                    }
                }
            }

            ToolGuides::render_tool_guide(
                ui,
                rect,
                self.tool.to_toolbar_tool(),
                self.pending_points.len(),
                false,
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
                ToolKind::Polygon if self.pending_points.len() == 1 => {
                    let first = self.pending_points[0];
                    let radius = (effective - first).length();
                    let delta = effective - first;
                    let angle_deg = delta.y.atan2(delta.x).to_degrees();
                    let mid = (first + effective) * 0.5;
                    let mid_3d = self.active_plane.to_world(mid, 0.0);
                    if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, mid_3d) {
                        let mode_str = match self.polygon_mode {
                            ducad_sketch::PolygonMode::Inscribed => ducad_i18n::t!("dim-polygon-inscribed"),
                            ducad_sketch::PolygonMode::Circumscribed => ducad_i18n::t!("dim-polygon-circumscribed"),
                        };
                        CanvasHud::render_dimension_pill(
                            ui,
                            pos_2d,
                            &format!("{} {} · ∠ {:.1}° (N={})", mode_str, self.unit.format_precise(radius), angle_deg, self.polygon_sides),
                            false,
                        );
                    }
                }
                ToolKind::Slot if self.pending_points.len() == 1 => {
                    let first = self.pending_points[0];
                    let len = (effective - first).length();
                    let mid = (first + effective) * 0.5;
                    let mid_3d = self.active_plane.to_world(mid, 0.0);
                    if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, mid_3d) {
                        let mode_str = match self.slot_mode {
                            ducad_sketch::SlotMode::CenterToCenter => ducad_i18n::t!("dim-slot-c2c"),
                            ducad_sketch::SlotMode::Overall => ducad_i18n::t!("dim-slot-overall"),
                        };
                        CanvasHud::render_dimension_pill(
                            ui,
                            pos_2d,
                            &format!("{} L {}", mode_str, self.unit.format_precise(len)),
                            false,
                        );
                    }
                }
                ToolKind::Slot if self.pending_points.len() == 2 => {
                    let p1 = self.pending_points[0];
                    let p2 = self.pending_points[1];
                    let len = (p2 - p1).length();
                    let axis_dir = (p2 - p1).normalize_or_zero();
                    if axis_dir != glam::DVec2::ZERO {
                        let normal = glam::DVec2::new(-axis_dir.y, axis_dir.x);
                        let radius = ((effective - p1).dot(normal)).abs();
                        let width = radius * 2.0;
                        let mid = (p1 + p2) * 0.5;
                        let mid_3d = self.active_plane.to_world(mid, 0.0);
                        if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, mid_3d) {
                            CanvasHud::render_dimension_pill(
                                ui,
                                pos_2d,
                                &format!("Ø {} (R {}) · L {}", self.unit.format_precise(width), self.unit.format_precise(radius), self.unit.format_precise(len)),
                                false,
                            );
                        }
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

        if self.active_vertex.is_none() && self.active_edge.is_none() && self.active_sketch_corner.is_none() {
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

                if self.filleting_vertex_from_gizmo && !self.vertex_gizmo_dimension_editing {
                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.commit_vertex_fillet();
                        self.filleting_vertex_from_gizmo = false;
                    } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.clear_round_gizmo(RoundKind::Vertex);
                        self.filleting_vertex_from_gizmo = false;
                        self.vertex_gizmo_dimension_editing = false;
                        self.model_status = Some("Fillet Vertex dibatalkan".to_string());
                    }
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
                                        self.filleting_vertex_from_gizmo = false;
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

                if self.filleting_edge_from_gizmo && !self.edge_gizmo_dimension_editing {
                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.commit_edge_fillet_single();
                        self.filleting_edge_from_gizmo = false;
                    } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.clear_round_gizmo(RoundKind::Edge);
                        self.filleting_edge_from_gizmo = false;
                        self.edge_gizmo_dimension_editing = false;
                        self.model_status = Some("Fillet Edge dibatalkan".to_string());
                    }
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
                                        self.filleting_edge_from_gizmo = false;
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
                            if let Some(hit) = find_snap_with_exclude_set(
                                self.sketch(),
                                target_pt,
                                tol,
                                10.0,
                                Some(&group),
                                &[],
                            ) {
                                target_pt = hit.point;
                                self.last_snap = Some(hit);
                            } else {
                                self.last_snap = None;
                            }
                            self.sketch_move_delta = target_pt - centroid;
                        }
                    }
                }
                if is_dragging_this && handle_resp.drag_stopped() {
                    self.last_snap = None;
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

        // ==========================================
        // 2D SKETCH CORNER FILLET & CHAMFER GIZMOS
        // Tampil di mode Sketsa MAUPUN mode 3D selama tool Select aktif
        // ==========================================
        if self.tool == ToolKind::Select && !self.sketch().entities.is_empty() {
        let all_targets = ducad_sketch::find_all_fillet_targets(self.sketch());
        for target in all_targets {
            let (id1, id2, arc_id_opt, corner_2d, bisector_2d, base_radius) = match target {
                ducad_sketch::FilletTarget::SharpCorner {
                    line1,
                    line2,
                    corner,
                    bisector,
                } => (line1, line2, None, corner, bisector, 5.0),
                ducad_sketch::FilletTarget::ExistingFillet {
                    arc_id,
                    line1,
                    line2,
                    apex,
                    bisector,
                    radius,
                    ..
                } => (line1, line2, Some(arc_id), apex, bisector, radius),
            };

            let is_this_corner_active = self.active_sketch_corner.as_ref().is_some_and(|(a1, a2, pt)| {
                (*a1 == id1 && *a2 == id2 || *a1 == id2 && *a2 == id1) || (*pt - corner_2d).length() < 1e-2
            });

            let is_corner_hovered = self.hovered_corner_2d.is_some_and(|pt| (pt - corner_2d).length() < 1e-2);
            let is_corner_selected = (self.selected.contains(&id1) || self.selected.contains(&id2))
                && self.selected.len() <= 8;

            if !is_this_corner_active && !is_corner_hovered && !is_corner_selected {
                continue;
            }

            let corner_3d = self.active_plane.to_world(corner_2d, 0.0);
            let b_end_3d = self.active_plane.to_world(corner_2d + bisector_2d, 0.0);
            let pull_dir_3d = (b_end_3d - corner_3d).normalize_or_zero();
            if pull_dir_3d == Vec3::ZERO {
                continue;
            }

            let z_pos = if is_this_corner_active && self.sketch_corner_gizmo_active {
                self.sketch_corner_gizmo_radius.abs().max(0.1) as f32
            } else {
                base_radius.abs().max(1.0) as f32
            };
            let handle_3d = corner_3d + pull_dir_3d * z_pos;

            if let Some(handle_2d) = world_to_screen_pos(&self.camera, rect, handle_3d) {
                let (_, arrow_vec_opt) = self.project_screen_drag_to_world_axis(
                    rect,
                    corner_3d,
                    pull_dir_3d,
                    egui::Vec2::ZERO,
                );

                let handle_resp = CanvasHud::render_draggable_double_arrow_handle(
                    ui,
                    handle_2d,
                    is_this_corner_active && self.sketch_corner_gizmo_active,
                    arrow_vec_opt,
                );

                if handle_resp.drag_started() {
                    self.active_sketch_corner = Some((id1, id2, corner_2d));
                    self.active_sketch_fillet_arc = arc_id_opt;
                    self.sketch_corner_gizmo_active = true;
                    if self.sketch_corner_gizmo_radius.abs() < 0.1 {
                        self.sketch_corner_gizmo_radius = base_radius;
                    }
                }

                if is_this_corner_active && handle_resp.dragged() {
                    self.sketch_corner_gizmo_active = true;
                    let (delta_mm, _) = self.project_screen_drag_to_world_axis(
                        rect,
                        corner_3d,
                        pull_dir_3d,
                        handle_resp.drag_delta(),
                    );
                    self.sketch_corner_gizmo_radius += delta_mm;
                    self.sketch_corner_edit_input = format!(
                        "{:.1}",
                        self.unit.to_display_val(self.sketch_corner_gizmo_radius.abs())
                    );
                }

                if is_this_corner_active && handle_resp.drag_stopped() {
                    self.commit_sketch_corner_fillet_or_chamfer();
                }

                if is_this_corner_active && self.sketch_corner_gizmo_active && !self.sketch_corner_dimension_editing {
                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.commit_sketch_corner_fillet_or_chamfer();
                    } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.active_sketch_corner = None;
                        self.active_sketch_fillet_arc = None;
                        self.sketch_corner_gizmo_active = false;
                        self.sketch_corner_dimension_editing = false;
                        self.model_status = Some("Fillet/Chamfer 2D dibatalkan".to_string());
                    }
                }

                let pill_pos = handle_2d + egui::vec2(0.0, -30.0);
                let is_hovering_combined = handle_resp.hovered()
                    || (ui.rect_contains_pointer(egui::Rect::from_center_size(
                        pill_pos,
                        egui::vec2(90.0, 32.0),
                    )));

                if is_this_corner_active || is_hovering_combined {
                    let display_r = if is_this_corner_active && self.sketch_corner_gizmo_radius.abs() >= 0.1 {
                        self.sketch_corner_gizmo_radius
                    } else {
                        base_radius
                    };

                    let text = if display_r >= 0.0 {
                        format!("R {}", self.unit.format(display_r))
                    } else {
                        format!("C {}", self.unit.format(-display_r))
                    };

                    let pill_resp = CanvasHud::render_interactive_dimension_pill(
                        ui,
                        pill_pos,
                        &text,
                        is_this_corner_active && self.sketch_corner_dimension_editing,
                    );

                    if pill_resp.clicked() {
                        self.active_sketch_corner = Some((id1, id2, corner_2d));
                        self.active_sketch_fillet_arc = arc_id_opt;
                        self.sketch_corner_gizmo_radius = display_r;
                        self.sketch_corner_gizmo_active = true;
                        self.sketch_corner_dimension_editing = !self.sketch_corner_dimension_editing;
                        self.sketch_corner_edit_input = format!(
                            "{:.1}",
                            self.unit.to_display_val(display_r.abs())
                        );
                    }

                    if is_this_corner_active && self.sketch_corner_dimension_editing {
                        let popup_rect = egui::Rect::from_center_size(
                            pill_pos + egui::vec2(0.0, 28.0),
                            egui::vec2(100.0, 32.0),
                        );
                        egui::Area::new(egui::Id::new("ducad-sketch-corner-gizmo-edit-popup"))
                            .fixed_pos(popup_rect.min)
                            .order(egui::Order::Foreground)
                            .show(ui.ctx(), |ui| {
                                egui::Frame::popup(ui.style()).show(ui, |ui| {
                                    let resp =
                                        ui.text_edit_singleline(&mut self.sketch_corner_edit_input);
                                    resp.request_focus();
                                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                        self.sketch_corner_dimension_editing = false;
                                    } else if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                        if let Ok(val) =
                                            self.sketch_corner_edit_input.trim().parse::<f64>()
                                        {
                                            let target_r = self.unit.to_internal_mm(val).max(0.0);
                                            self.sketch_corner_gizmo_radius =
                                                if self.sketch_corner_gizmo_radius < 0.0 {
                                                    -target_r
                                                } else {
                                                    target_r
                                                };
                                            self.commit_sketch_corner_fillet_or_chamfer();
                                        }
                                        self.sketch_corner_dimension_editing = false;
                                    } else if resp.lost_focus() {
                                        self.sketch_corner_dimension_editing = false;
                                    }
                                });
                            });
                    }
                }
            }
        }
        } // end if self.is_sketching

        // =========================================================================
        // HOLE WIZARD — RULER DIMENSION PILLS & DRAGGABLE TARGET HANDLE (FASE 9.2)
        // =========================================================================
        if self.tool == ToolKind::HoleWizard {
            if let Some((_, _, hit)) = &self.active_face {
                let (hole_pos, u_axis, v_axis, normal) =
                    self.compute_active_hole_position_and_basis(hit);
                let closest_edges = self.compute_hole_closest_edges(hit, hole_pos);

                // 1. Render label penggaris jarak (ruler distance pills) ke tepi terdekat
                let base_pt = Vec3::new(
                    hit.hit_point.0 as f32,
                    hit.hit_point.1 as f32,
                    hit.hit_point.2 as f32,
                );

                for (idx, (q, dist, _, _)) in closest_edges.iter().enumerate() {
                    if *dist > 0.4 {
                        let mid = (hole_pos + *q) * 0.5;
                        if let Some(pos_2d) = world_to_screen_pos(&self.camera, rect, mid) {
                            let text = format!("📏 {}", self.unit.format(*dist as f64));
                            CanvasHud::render_dimension_pill(ui, pos_2d, &text, false);

                            let pill_rect =
                                egui::Rect::from_center_size(pos_2d, egui::vec2(60.0, 22.0));
                            let pill_resp = ui.interact(
                                pill_rect,
                                ui.make_persistent_id(("hole_ruler_pill_click", idx)),
                                egui::Sense::click(),
                            );

                            if pill_resp.clicked() {
                                self.editing_hole_ruler_idx = Some(idx);
                                self.editing_hole_ruler_input = format!(
                                    "{:.1}",
                                    self.unit.to_display_val(*dist as f64)
                                );
                            }

                            if self.editing_hole_ruler_idx == Some(idx) {
                                let popup_rect = egui::Rect::from_center_size(
                                    pos_2d + egui::vec2(0.0, 26.0),
                                    egui::vec2(90.0, 30.0),
                                );
                                egui::Area::new(egui::Id::new((
                                    "ducad-hole-ruler-edit-popup",
                                    idx,
                                )))
                                .fixed_pos(popup_rect.min)
                                .order(egui::Order::Foreground)
                                .show(ui.ctx(), |ui| {
                                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                                        let resp = ui.text_edit_singleline(
                                            &mut self.editing_hole_ruler_input,
                                        );
                                        resp.request_focus();
                                        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                            self.editing_hole_ruler_idx = None;
                                        } else if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                            if let Ok(val) = self
                                                .editing_hole_ruler_input
                                                .trim()
                                                .parse::<f64>()
                                            {
                                                let target_dist_mm = self
                                                    .unit
                                                    .to_internal_mm(val)
                                                    .max(0.0)
                                                    as f32;
                                                let dir =
                                                    (hole_pos - *q).normalize_or_zero();
                                                let new_hole_pos = *q + dir * target_dist_mm;
                                                self.hole_popup_state.current_pos_3d = Some((
                                                    new_hole_pos.x as f64,
                                                    new_hole_pos.y as f64,
                                                    new_hole_pos.z as f64,
                                                ));
                                                let diff = new_hole_pos - base_pt;
                                                self.hole_popup_state.offset_u = (diff.dot(u_axis)
                                                    as f64
                                                    * 10.0)
                                                    .round()
                                                    / 10.0;
                                                self.hole_popup_state.offset_v = (diff.dot(v_axis)
                                                    as f64
                                                    * 10.0)
                                                    .round()
                                                    / 10.0;
                                            }
                                            self.editing_hole_ruler_idx = None;
                                        } else if resp.lost_focus() {
                                            self.editing_hole_ruler_idx = None;
                                        }
                                    });
                                });
                            }
                        }
                    }
                }

                // 2. Render target handle di pusat lubang yang dapat di-drag
                if let Some(hole_2d) = world_to_screen_pos(&self.camera, rect, hole_pos) {
                    let handle_id = ui.make_persistent_id("hole_wizard_target_drag_handle");
                    let handle_rect = egui::Rect::from_center_size(hole_2d, egui::vec2(28.0, 28.0));
                    let handle_resp = ui.interact(handle_rect, handle_id, egui::Sense::drag());

                    let is_hovered = handle_resp.hovered()
                        || handle_resp.dragged()
                        || self.hole_popup_state.is_dragging;
                    let red_color = if is_hovered {
                        egui::Color32::from_rgb(255, 60, 60)
                    } else {
                        egui::Color32::from_rgb(255, 20, 20)
                    };
                    let black_color = egui::Color32::from_rgb(15, 15, 20);

                    // 1. Lingkaran luar hitam tebal (kontras tinggi)
                    ui.painter().circle_filled(hole_2d, 12.0, black_color);
                    // 2. Stroke merah menyala
                    ui.painter().circle_stroke(hole_2d, 10.0, egui::Stroke::new(2.5, red_color));
                    // 3. Ring hitam tengah
                    ui.painter().circle_filled(hole_2d, 5.5, black_color);
                    // 4. Titik bullseye merah menyala di tengah
                    ui.painter().circle_filled(hole_2d, 3.5, red_color);

                    if handle_resp.drag_started() {
                        self.hole_popup_state.is_dragging = true;
                    }

                    if handle_resp.dragged()
                        || (self.hole_popup_state.is_dragging
                            && ui.input(|i| i.pointer.is_decidedly_dragging()))
                    {
                        if let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos()) {
                            let (ray_origin, ray_dir) =
                                crate::viewport::screen_to_ray(&self.camera, rect, mouse_pos);
                            let denom = ray_dir.dot(normal);
                            if denom.abs() > 1e-4 {
                                let face_center = Vec3::new(
                                    hit.centroid.0 as f32,
                                    hit.centroid.1 as f32,
                                    hit.centroid.2 as f32,
                                );
                                let t = (face_center - ray_origin).dot(normal) / denom;
                                let new_3d = ray_origin + ray_dir * t;
                                self.hole_popup_state.current_pos_3d =
                                    Some((new_3d.x as f64, new_3d.y as f64, new_3d.z as f64));
                                let base_pt = Vec3::new(
                                    hit.hit_point.0 as f32,
                                    hit.hit_point.1 as f32,
                                    hit.hit_point.2 as f32,
                                );
                                let diff = new_3d - base_pt;
                                self.hole_popup_state.offset_u =
                                    (diff.dot(u_axis) as f64 * 10.0).round() / 10.0;
                                self.hole_popup_state.offset_v =
                                    (diff.dot(v_axis) as f64 * 10.0).round() / 10.0;
                            }
                        }
                    }

                    if handle_resp.drag_stopped() {
                        self.hole_popup_state.is_dragging = false;
                    }
                }
            }
        }
    }
}
