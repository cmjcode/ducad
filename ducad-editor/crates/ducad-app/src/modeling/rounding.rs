use ducad_core::BodyId;
use ducad_kernel::{FaceHit, KernelShape, PickRay, SurfaceKind};
use eframe::egui;
use glam::Vec3;

use crate::app::DuCADApp;
use crate::model::{BodyGeometry, ReplaceGeometryCommand};
use crate::types::{RoundFeature, RoundHistory, RoundKind, RoundPostOp, RoundPostStep, RoundStyle};
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
            if self.fresh_round_history(*body_id).is_none() {
                continue;
            }
            if let Some(body) = self.model.doc.bodies.get(*body_id) {
                if !body.visible {
                    continue;
                }
            }
            for (idx, f) in hist.features.iter().enumerate() {
                let v3 = Vec3::new(f.anchor.0 as f32, f.anchor.1 as f32, f.anchor.2 as f32);
                if let Some(sp) = world_to_screen_pos(&self.camera, rect, v3) {
                    let s_dist = (sp - pos).length();
                    if s_dist <= 12.0 && best.as_ref().is_none_or(|(_, _, bd)| s_dist < *bd) {
                        best = Some((*body_id, idx, s_dist));
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
        let hist = self.fresh_round_history(body_id)?;
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

        // 2. Fallback: Ray-cast kernel
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

        let k_dir = ducad_kernel::vertex_outward_normal(&geo.shape, (vhit.0, vhit.1, vhit.2));
        let mut dir = Vec3::new(k_dir.0 as f32, k_dir.1 as f32, k_dir.2 as f32).normalize_or_zero();

        if dir == Vec3::ZERO {
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
            dir = (vertex - center).normalize_or_zero();
        }

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
        let fallback_anchor = Vec3::new(point.0 as f32, point.1 as f32, point.2 as f32);
        let geo = self.model.geometry.get(body_id)?;

        if let Some((anchor_tup, dir_tup)) = ducad_kernel::edge_outward_normal(&geo.shape, ray, Self::EDGE_REAPPLY_TOLERANCE_MM) {
            let anchor = Vec3::new(anchor_tup.0 as f32, anchor_tup.1 as f32, anchor_tup.2 as f32);
            let dir = Vec3::new(dir_tup.0 as f32, dir_tup.1 as f32, dir_tup.2 as f32).normalize_or_zero();
            if dir != Vec3::ZERO {
                return Some((anchor, dir));
            }
        }

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

        let mut dir = (fallback_anchor - center).normalize_or_zero();
        if dir == Vec3::ZERO {
            dir = Vec3::Z;
        }
        Some((fallback_anchor, dir))
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
        eprintln!(
            "[FILLET-CHAMFER][KERNEL_BUILD] >>> [FUNCTION: DuCADApp::build_rounded_shape] \
            Building rounded shape with {} features",
            features.len()
        );
        let mut shape = ducad_kernel::clone_shape(base).map_err(|e| e.to_string())?;

        let mut i = 0;
        while i < features.len() {
            let f = &features[i];

            // Batch operasi rusuk yang memiliki style dan radius sama agar kernel
            // menghitung transisi sudut bersama (corner blend) dalam satu langkah.
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

                eprintln!(
                    "[FILLET-CHAMFER][KERNEL_BUILD] >>> [FUNCTION: DuCADApp::build_rounded_shape] \
                    Batch applying {} edge features with style={:?}, radius={:.2}mm",
                    batch_rays.len(), f.style, f.radius
                );

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
                        eprintln!(
                            "[FILLET-CHAMFER][KERNEL_BUILD_ERROR] >>> [FUNCTION: DuCADApp::build_rounded_shape] \
                            Batch edge rounding failed: {}", e
                        );
                        return Err(e.to_string());
                    }
                }

                i = j;
                continue;
            }

            eprintln!(
                "[FILLET-CHAMFER][KERNEL_BUILD] >>> [FUNCTION: DuCADApp::build_rounded_shape] \
                Applying feature [{}/{}]: kind={:?}, style={:?}, radius={:.2}mm, anchor=({:.2}, {:.2}, {:.2})",
                i + 1, features.len(), f.kind, f.style, f.radius, f.anchor.0, f.anchor.1, f.anchor.2
            );

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
                    eprintln!(
                        "[FILLET-CHAMFER][KERNEL_BUILD_ERROR] >>> [FUNCTION: DuCADApp::build_rounded_shape] \
                        Single rounding feature failed: {}", e
                    );
                    return Err(e.to_string());
                }
            }
            i += 1;
        }
        eprintln!(
            "[FILLET-CHAMFER][KERNEL_BUILD] >>> [FUNCTION: DuCADApp::build_rounded_shape] \
            All {} features successfully built into new 3D shape",
            features.len()
        );
        Ok(shape)
    }

    /// Bangun shape dari riwayat rounding lengkap: fitur rounding diselingi
    /// edit susulan pada posisinya (`base → fitur[..a] → edit → fitur[a..]`),
    /// sehingga mengubah radius fillet lama tidak menghapus push/pull sesudahnya.
    pub fn build_round_history_shape(
        base: &KernelShape,
        features: &[RoundFeature],
        post_ops: &[RoundPostStep],
    ) -> Result<KernelShape, String> {
        let mut steps: Vec<&RoundPostStep> = post_ops.iter().collect();
        steps.sort_by_key(|s| s.after);
        let mut shape = ducad_kernel::clone_shape(base).map_err(|e| e.to_string())?;
        let mut done = 0;
        for step in steps {
            let upto = step.after.min(features.len());
            if upto > done {
                shape = Self::build_rounded_shape(&shape, &features[done..upto])?;
                done = upto;
            }
            shape = match step.op {
                RoundPostOp::ExtrudeFace { ray, distance } => {
                    ducad_kernel::extrude_face(&shape, ray, distance).map_err(|e| e.to_string())?
                }
            };
        }
        if done < features.len() {
            shape = Self::build_rounded_shape(&shape, &features[done..])?;
        }
        Ok(shape)
    }

    /// Fitur `idx` dihapus: edit susulan sesudahnya bergeser satu posisi.
    fn shift_post_ops_after_removal(post_ops: &mut [RoundPostStep], idx: usize) {
        for s in post_ops.iter_mut().filter(|s| s.after > idx) {
            s.after -= 1;
        }
    }

    /// Riwayat rounding `body_id` bila masih menggambarkan geometri body
    /// sekarang. Riwayat basi (undo, edit lain) diabaikan: membangun ulang
    /// darinya akan menimpa geometri dengan versi lama.
    pub fn fresh_round_history(&self, body_id: BodyId) -> Option<&RoundHistory> {
        let geo = self.model.geometry.get(body_id)?;
        self.round_history
            .get(&body_id)
            .filter(|h| h.result_fingerprint == geo.mesh_fingerprint)
    }

    /// Lepas riwayat rounding body ke arsip (bukan dibuang): undo yang
    /// mengembalikan geometrinya membuat rounding bisa diedit lagi.
    pub fn archive_round_history(&mut self, body_id: BodyId) {
        const MAX_ARCHIVE: usize = 64;
        if let Some(h) = self.round_history.remove(&body_id) {
            self.round_history_archive.push((body_id, h));
            if self.round_history_archive.len() > MAX_ARCHIVE {
                self.round_history_archive.remove(0);
            }
        }
    }

    /// Selaraskan riwayat rounding dengan geometri sekarang setelah
    /// undo/redo: riwayat yang basi diarsipkan, versi arsip yang cocok
    /// dengan geometri body dipulihkan. Murah (bandingkan fingerprint saja).
    pub fn sync_round_history(&mut self) {
        let ids: Vec<BodyId> = self.model.geometry.iter().map(|(id, _)| id).collect();
        for id in ids {
            if self.fresh_round_history(id).is_some() {
                continue;
            }
            let Some(fp) = self.model.geometry.get(id).map(|g| g.mesh_fingerprint) else {
                continue;
            };
            let found = self
                .round_history_archive
                .iter()
                .rposition(|(b, h)| *b == id && h.result_fingerprint == fp);
            if let Some(pos) = found {
                self.archive_round_history(id);
                let (_, h) = self.round_history_archive.remove(pos);
                self.round_history.insert(id, h);
            }
        }
    }

    /// Push/pull sisi pada body berounding: catat sebagai edit susulan agar
    /// rounding tetap bisa diedit. `new_fingerprint` = geometri hasil edit.
    pub fn record_round_post_op(&mut self, body_id: BodyId, op: RoundPostOp, old_fingerprint: u64, new_fingerprint: u64) {
        match self.round_history.get_mut(&body_id) {
            Some(h) if h.result_fingerprint == old_fingerprint => {
                // Salinan untuk undo: riwayat SEBELUM edit ini tetap di arsip.
                if let Ok(base) = ducad_kernel::clone_shape(&h.base) {
                    let before = RoundHistory {
                        base,
                        features: h.features.clone(),
                        post_ops: h.post_ops.clone(),
                        result_fingerprint: h.result_fingerprint,
                    };
                    let after = h.features.len();
                    h.post_ops.push(RoundPostStep { after, op });
                    h.result_fingerprint = new_fingerprint;
                    self.round_history_archive.push((body_id, before));
                } else {
                    self.archive_round_history(body_id);
                }
            }
            _ => self.archive_round_history(body_id),
        }
    }

    /// Body berounding dipindah/diputar/diskala utuh: transformasikan juga
    /// riwayatnya (shape dasar, sinar pick, jangkar) supaya rounding tetap
    /// bisa diedit di posisi barunya.
    pub fn transform_round_history(
        &mut self,
        body_id: BodyId,
        xf: RoundTransform,
        old_fingerprint: u64,
        new_fingerprint: u64,
    ) {
        let fresh = self
            .round_history
            .get(&body_id)
            .is_some_and(|h| h.result_fingerprint == old_fingerprint);
        if !fresh {
            self.archive_round_history(body_id);
            return;
        }
        let Some(h) = self.round_history.get(&body_id) else { return };
        let Ok(base) = xf.apply_shape(&h.base) else {
            self.archive_round_history(body_id);
            return;
        };
        let moved = RoundHistory {
            base,
            features: h
                .features
                .iter()
                .map(|f| RoundFeature {
                    ray: xf.ray(f.ray),
                    anchor: xf.point(f.anchor),
                    radius: f.radius * xf.length_factor(),
                    radius_end: f.radius_end.map(|r| r * xf.length_factor()),
                    polyline: f.polyline.iter().map(|p| xf.point(*p)).collect(),
                    ..f.clone()
                })
                .collect(),
            post_ops: h
                .post_ops
                .iter()
                .map(|s| RoundPostStep {
                    after: s.after,
                    op: match s.op {
                        RoundPostOp::ExtrudeFace { ray, distance } => RoundPostOp::ExtrudeFace {
                            ray: xf.ray(ray),
                            distance: distance * xf.length_factor(),
                        },
                    },
                })
                .collect(),
            result_fingerprint: new_fingerprint,
        };
        self.archive_round_history(body_id);
        self.round_history.insert(body_id, moved);
    }

    pub fn clear_round_gizmo(&mut self, kind: RoundKind) {
        eprintln!(
            "[FILLET-CHAMFER][CLEAR_GIZMO] >>> [FUNCTION: DuCADApp::clear_round_gizmo] \
            Clearing round gizmo state for kind={:?}",
            kind
        );
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
        eprintln!(
            "[FILLET-CHAMFER][COMMIT_OBJECT] >>> [FUNCTION: DuCADApp::commit_round] \
            COMMIT rounding feature permanently | body_id={:?}, kind={:?}, style={:?}, magnitude={:.2}mm, is_sharp={}",
            body_id, kind, style, magnitude, sharp
        );
        let Some(geo) = self.model.geometry.get(body_id) else {
            self.model_status = Some("Body terpilih tidak ditemukan".to_string());
            return;
        };

        let (mut features, mut post_ops) = self
            .fresh_round_history(body_id)
            .map(|h| (h.features.clone(), h.post_ops.clone()))
            .unwrap_or_default();
        match self.editing_round {
            Some((b, idx)) if b == body_id && idx < features.len() => {
                if sharp {
                    features.remove(idx);
                    Self::shift_post_ops_after_removal(&mut post_ops, idx);
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

        let (build, new_base) = if let Some(h) = self.fresh_round_history(body_id) {
            (Self::build_round_history_shape(&h.base, &features, &post_ops), None)
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
                let new_geo = BodyGeometry::from_shape(shape);
                // Versi riwayat sebelum edit ini tetap di arsip (untuk undo).
                let old_base = match self.fresh_round_history(body_id) {
                    Some(h) => ducad_kernel::clone_shape(&h.base).ok(),
                    None => None,
                };
                self.archive_round_history(body_id);
                if !features.is_empty() {
                    if let Some(base) = new_base.or(old_base) {
                        self.round_history.insert(
                            body_id,
                            RoundHistory {
                                base,
                                features,
                                post_ops,
                                result_fingerprint: new_geo.mesh_fingerprint,
                            },
                        );
                    }
                }
                let label = match style {
                    RoundStyle::Fillet => "Fillet",
                    RoundStyle::Chamfer => "Chamfer",
                };
                eprintln!(
                    "[FILLET-CHAMFER][3D_OBJECT_PERMANENT] >>> [FUNCTION: DuCADApp::commit_round] \
                    Replacing body geometry in model with {} vertices and {} edge lines",
                    new_geo.mesh.positions.len(), new_geo.edge_lines.len()
                );
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
            eprintln!(
                "[FILLET-CHAMFER][PREVIEW_CACHE] >>> [FUNCTION: DuCADApp::update_round_preview_cache] \
                Radius near 0 ({:.3}mm) -> Reset preview cache to Siku (base shape)",
                radius
            );
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
                face_ranges: tess.face_ranges.clone(),
            };
            let edge_lines = ducad_kernel::extract_shape_edges(&shape, Some(&mesh));

            let mut min = [f32::INFINITY; 3];
            let mut max = [f32::NEG_INFINITY; 3];
            for p in &mesh.positions {
                for k in 0..3 {
                    min[k] = min[k].min(p[k]);
                    max[k] = max[k].max(p[k]);
                }
            }

            eprintln!(
                "[FILLET-CHAMFER][3D_GEOMETRY_CHANGED] >>> [FUNCTION: DuCADApp::update_round_preview_cache] \
                3D PREVIEW OBJECT UPDATED | body_id={:?}, kind={:?}, radius={:.2}mm, style={} | \
                Mesh: {} vertices, {} triangles, {} edge lines | BoundingBox: [{:.2}, {:.2}, {:.2}] to [{:.2}, {:.2}, {:.2}]",
                id, kind, radius, if radius > 0.0 { "Fillet" } else { "Chamfer" },
                mesh.positions.len(), mesh.indices.len() / 3, edge_lines.len(),
                min[0], min[1], min[2], max[0], max[1], max[2]
            );

            self.round_preview_cache = Some((kind, radius, id, mesh, edge_lines));
            true
        } else {
            eprintln!(
                "[FILLET-CHAMFER][3D_GEOMETRY_FAILED] >>> [FUNCTION: DuCADApp::update_round_preview_cache] \
                FAILED to generate 3D preview shape | body_id={:?}, kind={:?}, radius={:.2}mm",
                body_id, kind, radius
            );
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

        let (mut features, mut post_ops) = self
            .fresh_round_history(body_id)
            .map(|h| (h.features.clone(), h.post_ops.clone()))
            .unwrap_or_default();

        let sharp = radius.abs() < Self::ROUND_SHARP_MM;
        let (style, magnitude) = Self::round_style_and_magnitude(radius);

        match self.editing_round {
            Some((b, idx)) if b == body_id && idx < features.len() => {
                if sharp {
                    features.remove(idx);
                    Self::shift_post_ops_after_removal(&mut post_ops, idx);
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
        let base: &KernelShape = match self.fresh_round_history(body_id) {
            Some(h) => &h.base,
            None => {
                base_owned = ducad_kernel::clone_shape(&geo.shape).ok()?;
                post_ops.clear();
                &base_owned
            }
        };

        Self::build_round_history_shape(base, &features, &post_ops)
            .ok()
            .map(|shape| (body_id, shape))
    }
}

/// Transformasi utuh body yang ikut diterapkan ke riwayat rounding.
#[derive(Clone, Copy, Debug)]
pub enum RoundTransform {
    Translate(glam::DVec3),
    /// Putar `angle_rad` (tangan kanan) mengelilingi sumbu lewat `pivot`.
    Rotate { pivot: glam::DVec3, axis: glam::DVec3, angle_rad: f64 },
    /// Skala uniform terhadap `pivot`.
    Scale { pivot: glam::DVec3, factor: f64 },
}

impl RoundTransform {
    fn apply_shape(self, shape: &KernelShape) -> anyhow::Result<KernelShape> {
        let t = |v: glam::DVec3| (v.x, v.y, v.z);
        match self {
            Self::Translate(d) => ducad_kernel::translate_shape(shape, d.x, d.y, d.z),
            Self::Rotate { pivot, axis, angle_rad } => {
                ducad_kernel::rotate_shape(shape, t(pivot), t(axis), angle_rad)
            }
            Self::Scale { pivot, factor } => ducad_kernel::scale_shape(shape, t(pivot), factor),
        }
    }

    fn vec3(p: (f64, f64, f64)) -> glam::DVec3 {
        glam::DVec3::new(p.0, p.1, p.2)
    }

    fn point(self, p: (f64, f64, f64)) -> (f64, f64, f64) {
        let v = Self::vec3(p);
        let r = match self {
            Self::Translate(d) => v + d,
            Self::Rotate { pivot, axis, angle_rad } => {
                pivot + glam::DQuat::from_axis_angle(axis.normalize_or_zero(), angle_rad) * (v - pivot)
            }
            Self::Scale { pivot, factor } => pivot + (v - pivot) * factor,
        };
        (r.x, r.y, r.z)
    }

    fn ray(self, ray: PickRay) -> PickRay {
        let dir = match self {
            Self::Rotate { axis, angle_rad, .. } => {
                let d = glam::DQuat::from_axis_angle(axis.normalize_or_zero(), angle_rad)
                    * Self::vec3(ray.dir);
                (d.x, d.y, d.z)
            }
            _ => ray.dir,
        };
        PickRay { origin: self.point(ray.origin), dir }
    }

    fn length_factor(self) -> f64 {
        match self {
            Self::Scale { factor, .. } => factor.abs(),
            _ => 1.0,
        }
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
            post_ops: Vec::new(),
            result_fingerprint: app.model.geometry.get(id).unwrap().mesh_fingerprint,
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
            assert!(!mesh.positions.is_empty());
            assert!(!mesh.indices.is_empty());
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

    fn make_test_l_bracket() -> (ducad_kernel::KernelShape, ducad_kernel::KernelShape) {
        let profile1 = Profile::Loop(vec![
            ProfileSegment::Line { start: (0.0, 0.0), end: (30.0, 0.0) },
            ProfileSegment::Line { start: (30.0, 0.0), end: (30.0, 10.0) },
            ProfileSegment::Line { start: (30.0, 10.0), end: (10.0, 10.0) },
            ProfileSegment::Line { start: (10.0, 10.0), end: (10.0, 30.0) },
            ProfileSegment::Line { start: (10.0, 30.0), end: (0.0, 30.0) },
            ProfileSegment::Line { start: (0.0, 30.0), end: (0.0, 0.0) },
        ]);
        let profile2 = Profile::Loop(vec![
            ProfileSegment::Line { start: (0.0, 0.0), end: (30.0, 0.0) },
            ProfileSegment::Line { start: (30.0, 0.0), end: (30.0, 10.0) },
            ProfileSegment::Line { start: (30.0, 10.0), end: (10.0, 10.0) },
            ProfileSegment::Line { start: (10.0, 10.0), end: (10.0, 30.0) },
            ProfileSegment::Line { start: (10.0, 30.0), end: (0.0, 30.0) },
            ProfileSegment::Line { start: (0.0, 30.0), end: (0.0, 0.0) },
        ]);
        (
            extrude_profile(&profile1, 20.0).unwrap(),
            extrude_profile(&profile2, 20.0).unwrap(),
        )
    }

    #[test]
    fn test_concave_inner_edge_and_vertex_gizmo_dir_points_outward_to_pocket() {
        let mut app = DuCADApp::new_for_test();
        let (shape1, _) = make_test_l_bracket();
        let id = app.model.doc.add_body_with_material("LBracket", ducad_core::Material::default());
        app.model.geometry.insert(id, BodyGeometry::from_shape(shape1));

        // Rusuk dalam di (10, 10, z=0..20)
        let ray = PickRay {
            origin: (25.0, 25.0, 10.0),
            dir: (-1.0, -1.0, 0.0),
        };
        app.active_edge = Some((id, ray, (10.0, 10.0, 10.0)));

        let (anchor, dir) = app.active_edge_gizmo_dir().expect("active_edge_gizmo_dir must return Some");
        assert!((anchor.x - 10.0).abs() < 1e-3);
        assert!((anchor.y - 10.0).abs() < 1e-3);
        // Pada sudut/rusuk dalam L-bracket, gizmo HARUS mengarah ke luar material (+X dan +Y, menuju pocket terbuka)
        assert!(dir.x > 0.1, "dir.x harus positif (menuju ruang terbuka), bukan masuk ke dalam solid: {:?}", dir);
        assert!(dir.y > 0.1, "dir.y harus positif (menuju ruang terbuka), bukan masuk ke dalam solid: {:?}", dir);

        // Vertex dalam di (10, 10, 0)
        let v_ray = PickRay {
            origin: (25.0, 25.0, -10.0),
            dir: (-1.0, -1.0, 0.67),
        };
        app.active_vertex = Some((id, v_ray, (10.0, 10.0, 0.0)));
        let (v_anchor, v_dir) = app.active_vertex_gizmo_dir().expect("active_vertex_gizmo_dir must return Some");
        assert!((v_anchor.x - 10.0).abs() < 1e-3);
        assert!((v_anchor.y - 10.0).abs() < 1e-3);
        assert!(v_dir.x > 0.1, "v_dir.x harus positif: {:?}", v_dir);
        assert!(v_dir.y > 0.1, "v_dir.y harus positif: {:?}", v_dir);
    }

    #[test]
    fn test_concave_inner_edge_fillet_and_chamfer_preview_and_commit() {
        let mut app = DuCADApp::new_for_test();
        let (shape1, _) = make_test_l_bracket();
        let id = app.model.doc.add_body_with_material("LBracket", ducad_core::Material::default());
        app.model.geometry.insert(id, BodyGeometry::from_shape(shape1));

        let ray = PickRay {
            origin: (25.0, 25.0, 10.0),
            dir: (-1.0, -1.0, 0.0),
        };
        app.active_edge = Some((id, ray, (10.0, 10.0, 10.0)));

        // Test inner edge Fillet
        app.round_gizmo_style = RoundStyle::Fillet;
        app.edge_gizmo_radius = 2.0;
        let ok_fillet = app.update_round_preview_cache(RoundKind::Edge, 2.0);
        assert!(ok_fillet, "Inner edge fillet preview cache update must succeed");
        assert!(app.round_preview_cache.is_some());

        // Test inner edge Chamfer
        app.round_gizmo_style = RoundStyle::Chamfer;
        app.edge_gizmo_radius = -2.0;
        let ok_chamfer = app.update_round_preview_cache(RoundKind::Edge, -2.0);
        assert!(ok_chamfer, "Inner edge chamfer preview cache update must succeed");
        assert!(app.round_preview_cache.is_some());
    }

    /// Kotak 20 mm + fillet R3 pada rusuk vertikal x=y=0 lewat gizmo.
    fn app_with_filleted_box() -> (DuCADApp, ducad_core::BodyId, PickRay) {
        let mut app = DuCADApp::new_for_test();
        let (shape, _) = make_test_box();
        let id = app.model.doc.add_body_with_material("Box", ducad_core::Material::default());
        app.model.geometry.insert(id, BodyGeometry::from_shape(shape));
        let ray = PickRay { origin: (-5.0, -5.0, 10.0), dir: (1.0, 1.0, 0.0) };
        app.active_edge = Some((id, ray, (0.0, 0.0, 10.0)));
        app.edge_gizmo_radius = 3.0;
        app.commit_round(RoundKind::Edge);
        assert!(app.fresh_round_history(id).is_some(), "fillet harus punya riwayat");
        (app, id, ray)
    }

    fn top_z(app: &DuCADApp, id: ducad_core::BodyId) -> f64 {
        app.model.geometry.get(id).unwrap().shape.bounding_box().unwrap().1[2]
    }

    fn volume(app: &DuCADApp, id: ducad_core::BodyId) -> f64 {
        app.model.geometry.get(id).unwrap().shape.volume()
    }

    fn pull_top_face(app: &mut DuCADApp, id: ducad_core::BodyId, distance: f64) {
        let face_ray = PickRay { origin: (15.0, 15.0, 80.0), dir: (0.0, 0.0, -1.0) };
        let hit = ducad_kernel::pick_face_details(&app.model.geometry.get(id).unwrap().shape, face_ray)
            .expect("sisi atas harus terpilih");
        app.active_face = Some((id, face_ray, hit));
        app.extrude_active_face(distance);
    }

    fn edit_fillet(app: &mut DuCADApp, id: ducad_core::BodyId, ray: PickRay, radius: f64) {
        app.editing_round = Some((id, 0));
        app.active_edge = Some((id, ray, (0.0, 0.0, 10.0)));
        app.edge_gizmo_radius = radius;
        app.commit_round(RoundKind::Edge);
    }

    /// Laporan pengguna: fillet lalu push/pull sisi → fillet tidak bisa
    /// diedit lagi (klik/drag permukaan lengkung malah jadi extrude).
    #[test]
    fn fillet_stays_editable_after_push_pull() {
        let (mut app, id, ray) = app_with_filleted_box();
        pull_top_face(&mut app, id, 10.0);
        assert!((top_z(&app, id) - 30.0).abs() < 1e-3);
        let hist = app.fresh_round_history(id).expect("riwayat fillet tidak boleh hilang");
        assert_eq!(hist.post_ops.len(), 1);

        let v_r3 = volume(&app, id);
        edit_fillet(&mut app, id, ray, 6.0);
        let hist = app.fresh_round_history(id).expect("fillet tetap bisa diedit");
        assert!((hist.features[0].radius - 6.0).abs() < 1e-9);
        // Push/pull ikut diputar ulang sesudah fillet baru.
        assert!((top_z(&app, id) - 30.0).abs() < 1e-3, "tarikan sisi hilang saat fillet diedit");
        assert!(volume(&app, id) < v_r3 - 1.0, "radius lebih besar harus membuang material");
    }

    #[test]
    fn fillet_history_follows_undo_and_redo() {
        let (mut app, id, ray) = app_with_filleted_box();
        pull_top_face(&mut app, id, 10.0);
        edit_fillet(&mut app, id, ray, 6.0);

        app.undo_model();
        let hist = app.fresh_round_history(id).expect("undo edit fillet → riwayat R3 kembali");
        assert!((hist.features[0].radius - 3.0).abs() < 1e-9);
        assert_eq!(hist.post_ops.len(), 1);

        app.undo_model();
        let hist = app.fresh_round_history(id).expect("undo push/pull → riwayat tanpa tarikan");
        assert!(hist.post_ops.is_empty());
        assert!((top_z(&app, id) - 20.0).abs() < 1e-3);

        app.redo_model();
        app.redo_model();
        let hist = app.fresh_round_history(id).expect("redo → riwayat R6 kembali");
        assert!((hist.features[0].radius - 6.0).abs() < 1e-9);
    }

    #[test]
    fn fillet_stays_editable_after_moving_body() {
        let (mut app, id, _) = app_with_filleted_box();
        app.tool = crate::types::ToolKind::Select;
        app.selected_bodies.clear();
        app.selected_bodies.insert(id);
        app.translate_selected_body(Vec3::new(40.0, 0.0, 0.0));
        let hist = app.fresh_round_history(id).expect("geser body tidak boleh membuang fillet");
        assert!((hist.features[0].anchor.0 - 40.0).abs() < 1e-9);

        let (moved_ray, moved_anchor) = (hist.features[0].ray, hist.features[0].anchor);
        app.editing_round = Some((id, 0));
        app.active_edge = Some((id, moved_ray, moved_anchor));
        app.edge_gizmo_radius = 5.0;
        app.commit_round(RoundKind::Edge);
        let hist = app.fresh_round_history(id).expect("fillet di posisi baru tetap bisa diedit");
        assert!((hist.features[0].radius - 5.0).abs() < 1e-9);
        let min_x = app.model.geometry.get(id).unwrap().shape.bounding_box().unwrap().0[0];
        assert!((min_x - 40.0).abs() < 1e-3, "body harus tetap di posisi barunya");
    }

}
