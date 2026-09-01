use ducad_core::BodyId;
use ducad_kernel::{FaceHit, KernelShape, PickRay, SurfaceKind};
use eframe::egui;
use glam::Vec3;

use crate::app::DuCADApp;
use crate::model::{BodyGeometry, ReplaceGeometryCommand};
use crate::types::{RoundFeature, RoundHistory, RoundKind, RoundStyle};
use crate::viewport::{pixel_tolerance_to_world, screen_to_ray, world_to_screen_pos};

impl DuCADApp {
    pub const EDGE_REAPPLY_TOLERANCE_MM: f64 = 5.0;
    pub const ROUND_SHARP_MM: f64 = 0.05;

    pub fn find_round_feature_at_cursor(
        &self,
        rect: egui::Rect,
        pos: egui::Pos2,
    ) -> Option<(BodyId, usize)> {
        let mut best: Option<(BodyId, usize, f32)> = None;
        for (body_id, hist) in &self.round_history {
            if let Some(body) = self.model.doc.bodies.get(*body_id) {
                if !body.visible {
                    continue;
                }
            }
            for (idx, f) in hist.features.iter().enumerate() {
                let v3 = Vec3::new(f.anchor.0 as f32, f.anchor.1 as f32, f.anchor.2 as f32);
                if let Some(sp) = world_to_screen_pos(&self.camera, rect, v3) {
                    let s_dist = (sp - pos).length();
                    if s_dist <= 12.0 {
                        if best.as_ref().is_none_or(|(_, _, bd)| s_dist < *bd) {
                            best = Some((*body_id, idx, s_dist));
                        }
                    }
                }
            }
        }
        best.map(|(bid, idx, _)| (bid, idx))
    }

    pub fn find_round_feature_near(
        &self,
        body_id: BodyId,
        hit_point: (f64, f64, f64),
        surface_kind: SurfaceKind,
        rect: egui::Rect,
    ) -> Option<usize> {
        if surface_kind == SurfaceKind::Plane {
            return None;
        }
        let hist = self.round_history.get(&body_id)?;
        let tol = pixel_tolerance_to_world(&self.camera, rect) * 14.0;
        let hp = glam::DVec3::new(hit_point.0, hit_point.1, hit_point.2);
        let mut best: Option<(usize, f64)> = None;
        for (idx, f) in hist.features.iter().enumerate() {
            let mut d = (hp - glam::DVec3::new(f.anchor.0, f.anchor.1, f.anchor.2)).length();
            for pair in f.polyline.windows(2) {
                let a = glam::DVec3::new(pair[0].0, pair[0].1, pair[0].2);
                let b = glam::DVec3::new(pair[1].0, pair[1].1, pair[1].2);
                let ab = b - a;
                let t = if ab.length_squared() > 1e-12 {
                    ((hp - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                d = d.min((a + ab * t - hp).length());
            }
            let reach = f.radius * 1.5 + tol;
            if d <= reach && best.as_ref().is_none_or(|(_, bd)| d < *bd) {
                best = Some((idx, d));
            }
        }
        best.map(|(idx, _)| idx)
    }

    pub fn pick_body_face_at_cursor(
        &self,
        rect: egui::Rect,
        pos: egui::Pos2,
    ) -> Option<(BodyId, PickRay, FaceHit)> {
        let (origin, dir) = screen_to_ray(&self.camera, rect, pos);
        let ray = PickRay {
            origin: (origin.x as f64, origin.y as f64, origin.z as f64),
            dir: (dir.x as f64, dir.y as f64, dir.z as f64),
        };
        let mut closest: Option<(BodyId, PickRay, FaceHit, f64)> = None;
        for (id, geo) in self.model.geometry.iter() {
            if let Some(body) = self.model.doc.bodies.get(id) {
                if body.visible {
                    if let Some(hit) = ducad_kernel::pick_face_details(&geo.shape, ray) {
                        let hit_vec =
                            glam::DVec3::new(hit.hit_point.0, hit.hit_point.1, hit.hit_point.2);
                        let orig_vec = glam::DVec3::new(ray.origin.0, ray.origin.1, ray.origin.2);
                        let dist_sq = (hit_vec - orig_vec).length_squared();
                        if closest.as_ref().is_none_or(|(_, _, _, d)| dist_sq < *d) {
                            closest = Some((id, ray, hit, dist_sq));
                        }
                    }
                }
            }
        }
        closest.map(|(id, ray, hit, _)| (id, ray, hit))
    }

    #[allow(clippy::type_complexity)]
    pub fn pick_body_vertex_at_cursor(
        &self,
        rect: egui::Rect,
        pos: egui::Pos2,
    ) -> Option<(BodyId, PickRay, (f64, f64, f64))> {
        let (origin, dir) = screen_to_ray(&self.camera, rect, pos);
        let ray = PickRay {
            origin: (origin.x as f64, origin.y as f64, origin.z as f64),
            dir: (dir.x as f64, dir.y as f64, dir.z as f64),
        };

        // 0. Prioritas puncak: Jika ada marker vertex yang sedang di-hover
        if let Some((hid, hpos)) = self.hovered_vertex_marker {
            let v3 = Vec3::new(hpos.0 as f32, hpos.1 as f32, hpos.2 as f32);
            if let Some(sp) = world_to_screen_pos(&self.camera, rect, v3) {
                if (sp - pos).length() <= 38.0 {
                    return Some((hid, ray, hpos));
                }
            }
        }

        // 1. Prioritas utama: Deteksi kedekatan layar 2D (screen-space, toleransi 38px)
        let mut best_screen: Option<(BodyId, (f64, f64, f64), f32, f64)> = None;
        for (id, geo) in self.model.geometry.iter() {
            let Some(body) = self.model.doc.bodies.get(id) else {
                continue;
            };
            if !body.visible {
                continue;
            }
            for (x, y, z) in ducad_kernel::shape_vertices(&geo.shape) {
                let v3 = Vec3::new(x as f32, y as f32, z as f32);
                if let Some(sp) = world_to_screen_pos(&self.camera, rect, v3) {
                    let s_dist = (sp - pos).length();
                    if s_dist <= 38.0 {
                        let cam_dist = (v3 - self.camera.eye()).length_squared() as f64;
                        if best_screen.as_ref().is_none_or(|(_, _, best_s, best_d)| {
                            s_dist < *best_s || (s_dist == *best_s && cam_dist < *best_d)
                        }) {
                            best_screen = Some((id, (x, y, z), s_dist, cam_dist));
                        }
                    }
                }
            }
        }
        if let Some((id, hit, ..)) = best_screen {
            return Some((id, ray, hit));
        }

        // 2. Fallback: Ray-cast kernel OCCT
        let tolerance = pixel_tolerance_to_world(&self.camera, rect) * 24.0;
        let mut closest: Option<(BodyId, PickRay, (f64, f64, f64), f64)> = None;
        for (id, geo) in self.model.geometry.iter() {
            let Some(body) = self.model.doc.bodies.get(id) else {
                continue;
            };
            if !body.visible {
                continue;
            }
            if let Some(hit) = ducad_kernel::pick_vertex(&geo.shape, ray, tolerance) {
                let hit_vec = glam::DVec3::new(hit.0, hit.1, hit.2);
                let orig_vec = glam::DVec3::new(ray.origin.0, ray.origin.1, ray.origin.2);
                let dist_sq = (hit_vec - orig_vec).length_squared();
                if closest.as_ref().is_none_or(|(_, _, _, d)| dist_sq < *d) {
                    closest = Some((id, ray, hit, dist_sq));
                }
            }
        }
        closest.map(|(id, ray, hit, _)| (id, ray, hit))
    }

    #[allow(clippy::type_complexity)]
    pub fn pick_body_edge_at_cursor(
        &self,
        rect: egui::Rect,
        pos: egui::Pos2,
    ) -> Option<(BodyId, PickRay, (f64, f64, f64))> {
        let (origin, dir) = screen_to_ray(&self.camera, rect, pos);
        let ray = PickRay {
            origin: (origin.x as f64, origin.y as f64, origin.z as f64),
            dir: (dir.x as f64, dir.y as f64, dir.z as f64),
        };
        let tolerance = pixel_tolerance_to_world(&self.camera, rect) * 14.0;
        let mut closest: Option<(BodyId, PickRay, (f64, f64, f64), f64)> = None;
        for (id, geo) in self.model.geometry.iter() {
            let Some(body) = self.model.doc.bodies.get(id) else {
                continue;
            };
            if !body.visible {
                continue;
            }
            if let Some((point, _polyline)) =
                ducad_kernel::pick_edge(&geo.shape, ray, tolerance)
            {
                let hit_vec = glam::DVec3::new(point.0, point.1, point.2);
                let orig_vec = glam::DVec3::new(ray.origin.0, ray.origin.1, ray.origin.2);
                let dist_sq = (hit_vec - orig_vec).length_squared();
                if closest.as_ref().is_none_or(|(_, _, _, d)| dist_sq < *d) {
                    closest = Some((id, ray, point, dist_sq));
                }
            }
        }
        closest.map(|(id, ray, point, _)| (id, ray, point))
    }

    pub fn active_vertex_gizmo_dir(&self) -> Option<(Vec3, Vec3)> {
        let (body_id, _, vhit) = self.active_vertex?;
        if !self.model.doc.bodies.get(body_id).is_some_and(|b| b.visible) {
            return None;
        }
        let vertex = Vec3::new(vhit.0 as f32, vhit.1 as f32, vhit.2 as f32);
        let geo = self.model.geometry.get(body_id)?;
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for p in &geo.mesh.positions {
            for k in 0..3 {
                min[k] = min[k].min(p[k]);
                max[k] = max[k].max(p[k]);
            }
        }
        let center = Vec3::new(
            (min[0] + max[0]) * 0.5,
            (min[1] + max[1]) * 0.5,
            (min[2] + max[2]) * 0.5,
        );
        let mut dir = (vertex - center).normalize_or_zero();
        if dir == Vec3::ZERO {
            dir = Vec3::Z;
        }
        Some((vertex, dir))
    }

    pub fn active_edge_gizmo_dir(&self) -> Option<(Vec3, Vec3)> {
        let (body_id, ray, point) = self.active_edge?;
        if !self.model.doc.bodies.get(body_id).is_some_and(|b| b.visible) {
            return None;
        }
        let anchor = Vec3::new(point.0 as f32, point.1 as f32, point.2 as f32);
        let geo = self.model.geometry.get(body_id)?;
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for p in &geo.mesh.positions {
            for k in 0..3 {
                min[k] = min[k].min(p[k]);
                max[k] = max[k].max(p[k]);
            }
        }
        let center = Vec3::new(
            (min[0] + max[0]) * 0.5,
            (min[1] + max[1]) * 0.5,
            (min[2] + max[2]) * 0.5,
        );

        let mut dir = if let Some((_, polyline)) = ducad_kernel::pick_edge(&geo.shape, ray, Self::EDGE_REAPPLY_TOLERANCE_MM) {
            if polyline.len() >= 2 {
                let p1 = polyline.first().unwrap();
                let p2 = polyline.last().unwrap();
                let edge_tan = Vec3::new((p2.0 - p1.0) as f32, (p2.1 - p1.1) as f32, (p2.2 - p1.2) as f32).normalize_or_zero();
                if edge_tan != Vec3::ZERO {
                    let radial = (anchor - center) - edge_tan * (anchor - center).dot(edge_tan);
                    radial.normalize_or_zero()
                } else {
                    Vec3::ZERO
                }
            } else {
                Vec3::ZERO
            }
        } else {
            Vec3::ZERO
        };

        if dir == Vec3::ZERO {
            dir = (anchor - center).normalize_or_zero();
        }
        if dir == Vec3::ZERO {
            dir = Vec3::Z;
        }
        Some((anchor, dir))
    }

    /// True kalau ada pick sudut/rusuk/wajah 3D yang aktif (mode edit fitur
    /// tunggal) — dipakai buat MENYEMBUNYIKAN gizmo transform seluruh body
    /// (translate/rotate/copy) supaya tidak menimpa/menutupi ikon gizmo
    /// fillet/chamfer/extrude yang lebih kecil hanya karena body pemilik
    /// vertex/edge terpilih ikut masuk `selected_bodies` (dipakai jg utk
    /// highlight cyan, bukan pertanda "seluruh body" terpilih).
    pub fn feature_pick_active(&self) -> bool {
        self.active_face.is_some()
            || self.active_vertex.is_some()
            || self.active_edge.is_some()
            || self.staged_mate_targets.len() >= 2
    }

    pub fn selected_single_body_center(&self) -> Option<(BodyId, Vec3)> {
        if self.tool != crate::types::ToolKind::Select || self.selected_bodies.len() != 1 {
            return None;
        }
        let body_id = *self.selected_bodies.iter().next()?;
        if !self.model.doc.bodies.get(body_id).is_some_and(|b| b.visible) {
            return None;
        }
        let geo = self.model.geometry.get(body_id)?;
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for p in &geo.mesh.positions {
            for k in 0..3 {
                min[k] = min[k].min(p[k]);
                max[k] = max[k].max(p[k]);
            }
        }
        let center = Vec3::new(
            (min[0] + max[0]) * 0.5,
            (min[1] + max[1]) * 0.5,
            (min[2] + max[2]) * 0.5,
        );
        Some((body_id, center))
    }

    pub fn commit_vertex_fillet(&mut self) {
        self.commit_round(RoundKind::Vertex);
    }

    pub fn commit_edge_fillet_single(&mut self) {
        self.commit_round(RoundKind::Edge);
    }

    pub fn build_rounded_shape(
        base: &KernelShape,
        features: &[RoundFeature],
    ) -> Result<KernelShape, String> {
        let mut shape = ducad_kernel::clone_shape(base).map_err(|e| e.to_string())?;

        let mut i = 0;
        while i < features.len() {
            let f = &features[i];

            // Batch operasi rusuk yang memiliki style dan radius sama agar OpenCASCADE
            // menghitung transisi sudut bersama (corner blend) dalam satu langkah BRep API.
            if f.kind == RoundKind::Edge && f.radius_end.is_none() {
                let mut batch_rays = vec![f.ray];
                let mut j = i + 1;
                while j < features.len() {
                    let next = &features[j];
                    if next.kind == RoundKind::Edge
                        && next.style == f.style
                        && (next.radius - f.radius).abs() < 1e-4
                        && next.radius_end.is_none()
                    {
                        batch_rays.push(next.ray);
                        j += 1;
                    } else {
                        break;
                    }
                }

                let res = match f.style {
                    RoundStyle::Fillet => ducad_kernel::fillet_edges(
                        &shape,
                        f.radius,
                        &batch_rays,
                        Self::EDGE_REAPPLY_TOLERANCE_MM,
                    ),
                    RoundStyle::Chamfer => ducad_kernel::chamfer_edges(
                        &shape,
                        f.radius,
                        &batch_rays,
                        Self::EDGE_REAPPLY_TOLERANCE_MM,
                    ),
                };

                match res {
                    Ok(new_s) => {
                        shape = new_s;
                    }
                    Err(e) => {
                        return Err(e.to_string());
                    }
                }

                i = j;
                continue;
            }


            let res = match (f.kind, f.style) {
                (RoundKind::Vertex, RoundStyle::Fillet) => ducad_kernel::fillet_vertex(
                    &shape,
                    f.radius,
                    f.ray,
                    Self::EDGE_REAPPLY_TOLERANCE_MM,
                ),
                (RoundKind::Vertex, RoundStyle::Chamfer) => ducad_kernel::chamfer_vertex(
                    &shape,
                    f.radius,
                    f.ray,
                    Self::EDGE_REAPPLY_TOLERANCE_MM,
                ),
                (RoundKind::Edge, RoundStyle::Fillet) => {
                    if let Some(r_end) = f.radius_end {
                        if (r_end - f.radius).abs() > 1e-4 {
                            ducad_kernel::fillet_edges_variable(
                                &shape,
                                f.radius,
                                r_end,
                                &[f.ray],
                                Self::EDGE_REAPPLY_TOLERANCE_MM,
                            )
                        } else {
                            ducad_kernel::fillet_edges(
                                &shape,
                                f.radius,
                                &[f.ray],
                                Self::EDGE_REAPPLY_TOLERANCE_MM,
                            )
                        }
                    } else {
                        ducad_kernel::fillet_edges(
                            &shape,
                            f.radius,
                            &[f.ray],
                            Self::EDGE_REAPPLY_TOLERANCE_MM,
                        )
                    }
                }
                (RoundKind::Edge, RoundStyle::Chamfer) => ducad_kernel::chamfer_edges(
                    &shape,
                    f.radius,
                    &[f.ray],
                    Self::EDGE_REAPPLY_TOLERANCE_MM,
                ),
            };
            match res {
                Ok(new_s) => {
                    shape = new_s;
                }
                Err(e) => {
                    return Err(e.to_string());
                }
            }
            i += 1;
        }
        Ok(shape)
    }

    pub fn clear_round_gizmo(&mut self, kind: RoundKind) {
        self.editing_round = None;
        self.round_preview_cache = None;
        match kind {
            RoundKind::Vertex => {
                self.active_vertex = None;
                self.vertex_gizmo_radius = 0.0;
                self.vertex_gizmo_edit_input = "0".to_string();
            }
            RoundKind::Edge => {
                self.active_edge = None;
                self.edge_gizmo_radius = 0.0;
                self.edge_gizmo_edit_input = "0".to_string();
            }
        }
    }

    /// Pecah nilai gizmo BERTANDA (>0 = ditarik/fillet, <0 = didorong/chamfer)
    /// jadi `(style, magnitude>0)` — dipakai `commit_round` &
    /// `round_gizmo_preview_shape` supaya keduanya konsisten.
    fn round_style_and_magnitude(signed: f64) -> (RoundStyle, f64) {
        if signed < 0.0 {
            (RoundStyle::Chamfer, -signed)
        } else {
            (RoundStyle::Fillet, signed)
        }
    }

    pub fn commit_round(&mut self, kind: RoundKind) {
        let (body_id, ray, anchor, radius) = match kind {
            RoundKind::Vertex => {
                let Some((b, r, a)) = self.active_vertex else {
                    return;
                };
                (b, r, a, self.vertex_gizmo_radius)
            }
            RoundKind::Edge => {
                let Some((b, r, a)) = self.active_edge else {
                    return;
                };
                (b, r, a, self.edge_gizmo_radius)
            }
        };
        let sharp = radius.abs() < Self::ROUND_SHARP_MM;
        let (style, magnitude) = Self::round_style_and_magnitude(radius);
        let Some(geo) = self.model.geometry.get(body_id) else {
            self.model_status = Some("Body terpilih tidak ditemukan".to_string());
            return;
        };

        let mut features: Vec<RoundFeature> = self
            .round_history
            .get(&body_id)
            .map(|h| h.features.clone())
            .unwrap_or_default();
        match self.editing_round {
            Some((b, idx)) if b == body_id && idx < features.len() => {
                if sharp {
                    features.remove(idx);
                } else {
                    features[idx].radius = magnitude;
                    features[idx].style = style;
                }
            }
            _ => {
                if sharp {
                    self.model_status = Some("Radius 0 — sudut dibiarkan menyiku".to_string());
                    self.clear_round_gizmo(kind);
                    return;
                }
                let polyline = if kind == RoundKind::Edge {
                    ducad_kernel::pick_edge(
                        &geo.shape,
                        ray,
                        Self::EDGE_REAPPLY_TOLERANCE_MM,
                    )
                    .map(|(_, pl)| pl)
                    .unwrap_or_default()
                } else {
                    Vec::new()
                };
                features.push(RoundFeature {
                    kind,
                    style,
                    ray,
                    anchor,
                    radius: magnitude,
                    radius_end: None,
                    polyline,
                });
            }
        }

        let (build, new_base) = if let Some(h) = self.round_history.get(&body_id) {
            (Self::build_rounded_shape(&h.base, &features), None)
        } else {
            match ducad_kernel::clone_shape(&geo.shape) {
                Ok(base) => (Self::build_rounded_shape(&base, &features), Some(base)),
                Err(e) => {
                    self.model_status =
                        Some(format!("Gagal menyimpan shape dasar rounding: {e}"));
                    return;
                }
            }
        };

        match build {
            Ok(shape) => {
                if let Some(base) = new_base {
                    self.round_history.insert(
                        body_id,
                        RoundHistory {
                            base,
                            features: Vec::new(),
                        },
                    );
                }
                if features.is_empty() {
                    self.round_history.remove(&body_id);
                } else if let Some(h) = self.round_history.get_mut(&body_id) {
                    h.features = features;
                }
                let new_geo = BodyGeometry::from_shape(shape);
                let label = match style {
                    RoundStyle::Fillet => "Fillet",
                    RoundStyle::Chamfer => "Chamfer",
                };
                self.execute_model_command(
                    Box::new(ReplaceGeometryCommand::new(label, body_id, new_geo)),
                    &format!("{:.1} mm", magnitude),
                );
                if !sharp {
                    match style {
                        RoundStyle::Fillet => {
                            self.record_fillet_feature(magnitude, None);
                        }
                        RoundStyle::Chamfer => {
                            self.record_chamfer_feature(magnitude);
                        }
                    }
                }
                self.model_status = Some(if sharp {
                    "Rounding dihapus — sudut kembali menyiku".to_string()
                } else {
                    format!(
                        "{label} {:.1} mm sukses — klik sudutnya lagi utk mengubah/menghapus",
                        magnitude
                    )
                });
                self.clear_round_gizmo(kind);
            }
            Err(e) => self.model_status = Some(format!("Rounding gagal: {e}")),
        }
    }

    /// Update the internal preview cache when the radius (or active target) changes.
    /// Call this whenever `vertex_gizmo_radius` / `edge_gizmo_radius` is updated during drag.
    /// Returns `true` if the preview shape is valid or unchanged, `false` if build failed.
    pub fn update_round_preview_cache(&mut self, kind: crate::types::RoundKind, radius: f64) -> bool {
        use crate::types::RoundKind;

        // Determine the active body id for this kind
        let body_id_for_kind = match kind {
            RoundKind::Vertex => self.active_vertex.map(|(id, _, _)| id),
            RoundKind::Edge   => self.active_edge.map(|(id, _, _)| id),
        };
        let body_id = match body_id_for_kind {
            Some(id) => id,
            None => {
                self.round_preview_cache = None;
                return false;
            }
        };

        if radius.abs() < Self::ROUND_SHARP_MM {
            self.round_preview_cache = None;
            return true;
        }

        // Skip expensive recomputation if nothing changed
        if let Some((ck, cr, cid, ..)) = &self.round_preview_cache {
            if *ck == kind && (*cr - radius).abs() < 1e-9 && *cid == body_id {
                return true;
            }
        }

        let result = self.round_gizmo_preview_shape(kind, radius);
        if let Some((id, shape)) = result {
            let tess = shape.tessellate();
            let mesh = ducad_kernel::KernelMesh {
                positions: tess.positions.clone(),
                normals: tess.normals.clone(),
                indices: tess.indices.clone(),
            };
            let edge_lines = ducad_kernel::extract_shape_edges(&shape, Some(&mesh));
            self.round_preview_cache = Some((kind, radius, id, mesh, edge_lines));
            true
        } else {
            false
        }
    }

    pub fn round_gizmo_preview_shape(
        &self,
        kind: RoundKind,
        radius: f64,
    ) -> Option<(BodyId, KernelShape)> {
        let (body_id, ray, anchor) = match kind {
            RoundKind::Vertex => self.active_vertex?,
            RoundKind::Edge   => self.active_edge?,
        };
        let geo = self.model.geometry.get(body_id)?;

        let mut features: Vec<RoundFeature> = self
            .round_history
            .get(&body_id)
            .map(|h| h.features.clone())
            .unwrap_or_default();

        let sharp = radius.abs() < Self::ROUND_SHARP_MM;
        let (style, magnitude) = Self::round_style_and_magnitude(radius);

        match self.editing_round {
            Some((b, idx)) if b == body_id && idx < features.len() => {
                if sharp {
                    features.remove(idx);
                } else {
                    features[idx].radius = magnitude;
                    features[idx].style = style;
                }
            }
            _ => {
                if sharp {
                    return None;
                }
                features.push(RoundFeature {
                    kind,
                    style,
                    ray,
                    anchor,
                    radius: magnitude,
                    radius_end: None,
                    polyline: Vec::new(),
                });
            }
        }

        let base_owned;
        let base: &KernelShape = match self.round_history.get(&body_id) {
            Some(h) => &h.base,
            None => {
                base_owned = ducad_kernel::clone_shape(&geo.shape).ok()?;
                &base_owned
            }
        };

        Self::build_rounded_shape(base, &features)
            .ok()
            .map(|shape| (body_id, shape))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::DuCADApp;
    use crate::model::BodyGeometry;
    use ducad_kernel::{extrude_profile, PickRay, Profile, ProfileSegment};

    fn make_test_box() -> (ducad_kernel::KernelShape, ducad_kernel::KernelShape) {
        let s1 = extrude_profile(
            &Profile::Loop(vec![
                ProfileSegment::Line { start: (0.0, 0.0), end: (20.0, 0.0) },
                ProfileSegment::Line { start: (20.0, 0.0), end: (20.0, 20.0) },
                ProfileSegment::Line { start: (20.0, 20.0), end: (0.0, 20.0) },
                ProfileSegment::Line { start: (0.0, 20.0), end: (0.0, 0.0) },
            ]),
            20.0,
        ).unwrap();
        let s2 = extrude_profile(
            &Profile::Loop(vec![
                ProfileSegment::Line { start: (0.0, 0.0), end: (20.0, 0.0) },
                ProfileSegment::Line { start: (20.0, 0.0), end: (20.0, 20.0) },
                ProfileSegment::Line { start: (20.0, 20.0), end: (0.0, 20.0) },
                ProfileSegment::Line { start: (0.0, 20.0), end: (0.0, 0.0) },
            ]),
            20.0,
        ).unwrap();
        (s1, s2)
    }

    #[test]
    fn test_round_style_and_magnitude() {
        let (style_pos, mag_pos) = DuCADApp::round_style_and_magnitude(3.5);
        assert_eq!(style_pos, RoundStyle::Fillet);
        assert!((mag_pos - 3.5).abs() < 1e-6);

        let (style_neg, mag_neg) = DuCADApp::round_style_and_magnitude(-8.2);
        assert_eq!(style_neg, RoundStyle::Chamfer);
        assert!((mag_neg - 8.2).abs() < 1e-6);
    }

    #[test]
    fn test_build_rounded_shape_chamfer_and_fillet() {
        let (base, _) = make_test_box();
        let ray = PickRay {
            origin: (-5.0, -5.0, 10.0),
            dir: (1.0, 1.0, 0.0),
        };
        let chamfer_feature = RoundFeature {
            kind: RoundKind::Edge,
            style: RoundStyle::Chamfer,
            ray,
            anchor: (0.0, 0.0, 10.0),
            radius: 2.0,
            radius_end: None,
            polyline: vec![],
        };
        let result = DuCADApp::build_rounded_shape(&base, &[chamfer_feature]);
        assert!(result.is_ok(), "Chamfer edge build must succeed: {:?}", result.err());

        // Test various chamfer distances on the 20x20x20 box
        for dist in [0.5, 1.0, 2.0, 5.0, 9.9, 10.0, 15.0, 18.0] {
            let f = RoundFeature {
                kind: RoundKind::Edge,
                style: RoundStyle::Chamfer,
                ray,
                anchor: (0.0, 0.0, 10.0),
                radius: dist,
                radius_end: None,
                polyline: vec![],
            };
            let _ = DuCADApp::build_rounded_shape(&base, &[f]);
        }

        let fillet_feature = RoundFeature {
            kind: RoundKind::Edge,
            style: RoundStyle::Fillet,
            ray,
            anchor: (0.0, 0.0, 10.0),
            radius: 2.0,
            radius_end: None,
            polyline: vec![],
        };
        let result_f = DuCADApp::build_rounded_shape(&base, &[fillet_feature]);
        assert!(result_f.is_ok(), "Fillet edge build must succeed: {:?}", result_f.err());
    }

    #[test]
    fn test_round_gizmo_preview_shape_drag_to_siku_rebuilds_base() {
        let mut app = DuCADApp::new_for_test();
        let (shape1, shape2) = make_test_box();
        let id = app.model.doc.add_body_with_material("Box", ducad_core::Material::default());
        app.model.geometry.insert(id, BodyGeometry::from_shape(shape1));

        let ray = PickRay {
            origin: (-5.0, -5.0, 10.0),
            dir: (1.0, 1.0, 0.0),
        };
        let feature = RoundFeature {
            kind: RoundKind::Edge,
            style: RoundStyle::Chamfer,
            ray,
            anchor: (0.0, 0.0, 10.0),
            radius: 2.0,
            radius_end: None,
            polyline: vec![],
        };
        app.round_history.insert(id, RoundHistory {
            base: shape2,
            features: vec![feature],
        });

        // Set editing round active on this feature
        app.active_edge = Some((id, ray, (0.0, 0.0, 10.0)));
        app.editing_round = Some((id, 0));

        // When dragged to sharp (0.0 radius), preview should return the base shape without the chamfer
        let preview = app.round_gizmo_preview_shape(RoundKind::Edge, 0.0);
        assert!(preview.is_some(), "Preview shape when dragging existing feature to siku must be Some(base)");
        let (target_id, _) = preview.unwrap();
        assert_eq!(target_id, id);
    }

    #[test]
    fn test_chamfer_drag_preview_cache_live_updates() {
        let mut app = DuCADApp::new_for_test();
        let (shape1, _) = make_test_box();
        let id = app.model.doc.add_body_with_material("Box", ducad_core::Material::default());
        app.model.geometry.insert(id, BodyGeometry::from_shape(shape1));

        let ray = PickRay {
            origin: (-5.0, -5.0, 10.0),
            dir: (1.0, 1.0, 0.0),
        };
        app.active_edge = Some((id, ray, (0.0, 0.0, 10.0)));
        app.round_gizmo_style = RoundStyle::Chamfer;

        // User drags / changes chamfer radius
        for r in [1.0, 2.5, 4.0, 6.0, 8.5] {
            let signed_r = -r;
            app.edge_gizmo_radius = signed_r;
            app.update_round_preview_cache(RoundKind::Edge, signed_r);
            assert!(
                app.round_preview_cache.is_some(),
                "Preview cache must be Some for chamfer radius {signed_r}"
            );
            let (kind, cached_r, cached_id, mesh, _) = app.round_preview_cache.as_ref().unwrap();
            assert_eq!(*kind, RoundKind::Edge);
            assert_eq!(*cached_r, signed_r);
            assert_eq!(*cached_id, id);
            assert!(mesh.positions.len() > 0);
            assert!(mesh.indices.len() > 0);
        }
    }

    #[test]
    fn test_vertex_chamfer_and_fillet_preview_cache_live_updates() {
        let mut app = DuCADApp::new_for_test();
        let (shape1, _) = make_test_box();
        let id = app.model.doc.add_body_with_material("Box", ducad_core::Material::default());
        app.model.geometry.insert(id, BodyGeometry::from_shape(shape1));

        let ray = PickRay {
            origin: (-5.0, -5.0, -5.0),
            dir: (1.0, 1.0, 1.0),
        };
        app.active_vertex = Some((id, ray, (0.0, 0.0, 0.0)));

        // Test vertex chamfer
        app.round_gizmo_style = RoundStyle::Chamfer;
        app.vertex_gizmo_radius = -2.5;
        app.update_round_preview_cache(RoundKind::Vertex, -2.5);
        assert!(app.round_preview_cache.is_some());
        let (kind, cached_r, _, mesh, _) = app.round_preview_cache.as_ref().unwrap();
        assert_eq!(*kind, RoundKind::Vertex);
        assert_eq!(*cached_r, -2.5);
        assert!(mesh.triangle_count() > 0);

        // Test vertex fillet
        app.round_gizmo_style = RoundStyle::Fillet;
        app.vertex_gizmo_radius = 2.5;
        app.update_round_preview_cache(RoundKind::Vertex, 2.5);
        assert!(app.round_preview_cache.is_some());
        let (kind, cached_r, _, mesh, _) = app.round_preview_cache.as_ref().unwrap();
        assert_eq!(*kind, RoundKind::Vertex);
        assert_eq!(*cached_r, 2.5);
        assert!(mesh.triangle_count() > 0);
    }
}
