//! Fillet/chamfer gizmo sebagai op oplog.
//!
//! Rounding dari gizmo sudut/rusuk dicatat sebagai op `fillet`, `chamfer`,
//! atau `round_vertex`, jadi tetap bisa diedit selama op itu ada — walau
//! sesudahnya body ditarik sisinya, digeser, atau di-undo/redo. Klik
//! sudutnya lagi = edit op itu (replay); radius 0 = hapus op.
//!
//! [`RoundMarker`] menyimpan titik klik dalam kerangka saat op dibuat; posisi
//! sekarang diturunkan dari op `transform`/`scale` sesudahnya di oplog,
//! sehingga ikut benar setelah undo/redo tanpa pembukuan tambahan.

#![allow(clippy::result_large_err)]

use ducad_core::BodyId;
use ducad_engine::ops::Op;
use ducad_engine::OpError;
use ducad_kernel::{KernelShape, PickRay, SurfaceKind};
use eframe::egui;
use glam::{DVec3, Vec3};

use crate::app::DuCADApp;
use crate::modeling::rounding::RoundTransform;
use crate::types::{RoundKind, RoundMarker, RoundStyle};
use crate::viewport::{pixel_tolerance_to_world, world_to_screen_pos};

/// Isi op rounding yang relevan untuk gizmo.
#[derive(Clone, Debug, PartialEq)]
pub struct RoundOpInfo {
    pub index: usize,
    pub body: String,
    pub radius: f64,
    pub style: RoundStyle,
    pub kind: RoundKind,
}

/// Penanda dalam posisi SEKARANG.
#[derive(Clone, Debug)]
pub struct RoundMarkerNow {
    pub op_id: String,
    pub body: BodyId,
    pub info: RoundOpInfo,
    pub ray: PickRay,
    pub anchor: (f64, f64, f64),
    pub polyline: Vec<(f64, f64, f64)>,
    pub transforms: Vec<RoundTransform>,
}

fn vec3(v: &serde_json::Value) -> Option<DVec3> {
    let a = v.as_array()?;
    Some(DVec3::new(a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?))
}

fn op_json(op: &Op) -> Option<serde_json::Value> {
    serde_json::to_value(op).ok()
}

impl DuCADApp {
    /// Info op rounding `op_id`; `None` bila op tidak ada (mis. di-undo)
    /// atau bukan op rounding.
    pub fn round_op_info(&self, op_id: &str) -> Option<RoundOpInfo> {
        let (index, op) = self
            .agent_meta
            .design
            .oplog
            .iter()
            .enumerate()
            .find(|(_, o)| o.id() == op_id)?;
        let v = op_json(op)?;
        let body = v.get("body")?.as_str()?.to_string();
        let (kind, style, key) = match v.get("op")?.as_str()? {
            "fillet" => (RoundKind::Edge, RoundStyle::Fillet, "radius"),
            "chamfer" => (RoundKind::Edge, RoundStyle::Chamfer, "distance"),
            "round_vertex" => {
                let chamfer = v.get("chamfer").and_then(|c| c.as_bool()).unwrap_or(false);
                let style = if chamfer { RoundStyle::Chamfer } else { RoundStyle::Fillet };
                (RoundKind::Vertex, style, "radius")
            }
            _ => return None,
        };
        let radius = v.get(key)?.as_f64()?;
        Some(RoundOpInfo { index, body, radius, style, kind })
    }

    /// Transformasi utuh body `body` oleh op sesudah indeks `index`.
    fn transforms_after(&self, index: usize, body: &str) -> Vec<RoundTransform> {
        let mut out = Vec::new();
        for op in self.agent_meta.design.oplog.iter().skip(index + 1) {
            let Some(v) = op_json(op) else { continue };
            if v.get("body").and_then(|b| b.as_str()) != Some(body) {
                continue;
            }
            if v.get("copy").and_then(|c| c.as_bool()).unwrap_or(false) {
                continue;
            }
            match v.get("op").and_then(|o| o.as_str()) {
                Some("transform") => {
                    if let Some(t) = v.get("translate").and_then(vec3) {
                        out.push(RoundTransform::Translate(t));
                    }
                    if let Some(r) = v.get("rotate") {
                        if let (Some(pivot), Some(axis), Some(angle)) = (
                            r.get("pivot").and_then(vec3),
                            r.get("axis").and_then(vec3),
                            r.get("angle_deg").and_then(|a| a.as_f64()),
                        ) {
                            out.push(RoundTransform::Rotate { pivot, axis, angle_rad: angle.to_radians() });
                        }
                    }
                }
                Some("scale") => {
                    let pivot = v.get("pivot").and_then(vec3).unwrap_or(DVec3::ZERO);
                    if let Some(factor) = v.get("factor").and_then(|f| f.as_f64()) {
                        out.push(RoundTransform::Scale { pivot, factor });
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// Penanda `m` dalam posisi sekarang; `None` bila op-nya tidak ada lagi.
    pub fn round_marker_now(&self, m: &RoundMarker) -> Option<RoundMarkerNow> {
        let info = self.round_op_info(&m.op_id)?;
        let (body, _) = self.model.doc.bodies.iter().find(|(_, b)| b.name == info.body)?;
        let transforms = self.transforms_after(info.index, &info.body);
        let mut ray = m.ray();
        let mut anchor = m.anchor;
        let mut polyline = m.polyline.clone();
        for t in &transforms {
            ray = t.ray(ray);
            anchor = t.point(anchor);
            polyline = polyline.iter().map(|p| t.point(*p)).collect();
        }
        Some(RoundMarkerNow { op_id: m.op_id.clone(), body, info, ray, anchor, polyline, transforms })
    }

    /// Penanda rounding-op di bawah kursor: ikon jangkar (≤ 12 px) atau
    /// permukaan lengkung hasil rounding di dekat rusuknya.
    ///
    /// Face DATAR hasil chamfer tidak pernah direbut penanda: di layar sudut
    /// lama berimpit dengan face pangkas itu, sedangkan face-nya harus tetap
    /// bisa dipilih lalu ditarik (push/pull). Edit chamfer: klik face
    /// tetangga di dekat sudut lamanya, atau permukaan lengkung fillet.
    pub fn find_round_op_at(
        &self,
        rect: egui::Rect,
        pos: egui::Pos2,
        face: Option<&(BodyId, PickRay, ducad_kernel::FaceHit)>,
    ) -> Option<String> {
        let markers: Vec<RoundMarkerNow> = self
            .round_markers
            .iter()
            .filter_map(|m| self.round_marker_now(m))
            .filter(|m| self.model.doc.bodies.get(m.body).is_some_and(|b| b.visible))
            .collect();
        let tol = pixel_tolerance_to_world(&self.camera, rect) * 14.0;
        // Face datar yang seluruhnya lahir dari rounding (face chamfer).
        let planar_round_face = face.is_some_and(|(b_id, _, hit)| {
            hit.surface_kind == SurfaceKind::Plane
                && Self::round_op_for_face_in(&markers, *b_id, hit, tol).is_some()
        });
        if planar_round_face {
            return None;
        }
        let mut best: Option<(&str, f32)> = None;
        for m in &markers {
            let v = Vec3::new(m.anchor.0 as f32, m.anchor.1 as f32, m.anchor.2 as f32);
            if let Some(sp) = world_to_screen_pos(&self.camera, rect, v) {
                let d = (sp - pos).length();
                if d <= 12.0 && best.is_none_or(|(_, bd)| d < bd) {
                    best = Some((&m.op_id, d));
                }
            }
        }
        if let Some((id, _)) = best {
            return Some(id.to_string());
        }
        let (b_id, _, hit) = face?;
        if hit.surface_kind == SurfaceKind::Plane {
            return None;
        }
        let hp = DVec3::from(hit.hit_point);
        markers
            .iter()
            .filter(|m| m.body == *b_id)
            .map(|m| (m, Self::dist_to_round_region(m, hp)))
            .filter(|(m, d)| *d <= m.info.radius * 1.5 + tol)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(m, _)| m.op_id.clone())
    }

    /// Op rounding yang melahirkan face `hit` pada body `body` (mis. face
    /// chamfer): centroid face berada di daerah sudut/rusuk asal penanda.
    pub fn round_op_for_face(
        &self,
        rect: egui::Rect,
        body: BodyId,
        hit: &ducad_kernel::FaceHit,
    ) -> Option<String> {
        let markers: Vec<RoundMarkerNow> = self
            .round_markers
            .iter()
            .filter_map(|m| self.round_marker_now(m))
            .collect();
        let tol = pixel_tolerance_to_world(&self.camera, rect) * 14.0;
        Self::round_op_for_face_in(&markers, body, hit, tol)
    }

    fn round_op_for_face_in(
        markers: &[RoundMarkerNow],
        body: BodyId,
        hit: &ducad_kernel::FaceHit,
        tol: f64,
    ) -> Option<String> {
        let c = DVec3::from(hit.centroid);
        markers
            .iter()
            .filter(|m| m.body == body)
            .map(|m| (m, Self::dist_to_round_region(m, c)))
            .filter(|(m, d)| *d <= m.info.radius * 1.5 + tol)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(m, _)| m.op_id.clone())
    }

    /// Jarak `p` ke daerah rounding penanda `m`: sudut asal (anchor) atau
    /// rusuk asal (polyline).
    fn dist_to_round_region(m: &RoundMarkerNow, p: DVec3) -> f64 {
        let mut d = (p - DVec3::from(m.anchor)).length();
        for w in m.polyline.windows(2) {
            let (a, b) = (DVec3::from(w[0]), DVec3::from(w[1]));
            let ab = b - a;
            let t = if ab.length_squared() > 1e-12 {
                ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
            } else {
                0.0
            };
            d = d.min((a + ab * t - p).length());
        }
        d
    }

    /// Shape body tepat sebelum op `info` (replay sebagian), di-cache.
    fn round_op_base_shape(&mut self, op_id: &str, info: &RoundOpInfo) -> Option<()> {
        let len = self.agent_meta.design.oplog.len();
        if let Some((id, n, _)) = &self.round_op_base {
            if id == op_id && *n == len {
                return Some(());
            }
        }
        let mut design = self.agent_meta.design.clone();
        design.oplog.truncate(info.index);
        design.fingerprint = String::new();
        let session = ducad_engine::Session::replay(design).ok()?;
        let shape = ducad_kernel::clone_shape(&session.body(&info.body).ok()?.1.shape).ok()?;
        self.round_op_base = Some((op_id.to_string(), len, shape));
        Some(())
    }

    /// Buka gizmo untuk mengedit op rounding `op_id` (klik sudutnya).
    pub fn begin_round_op_edit(&mut self, op_id: &str) -> bool {
        let Some(m) = self.round_markers.iter().find(|m| m.op_id == op_id).cloned() else {
            return false;
        };
        let Some(now) = self.round_marker_now(&m) else {
            return false;
        };
        // Pratinjau butuh shape sebelum op; replay gagal → tetap bisa edit
        // (commit memakai replay penuh), hanya tanpa pratinjau.
        let _ = self.round_op_base_shape(op_id, &now.info);
        self.selected.clear();
        self.selected_bodies.clear();
        self.selected_bodies.insert(now.body);
        self.editing_round = None;
        self.editing_round_op = Some(op_id.to_string());
        self.active_face = None;
        self.last_body_select_click = None;
        let signed = match now.info.style {
            RoundStyle::Fillet => now.info.radius,
            RoundStyle::Chamfer => -now.info.radius,
        };
        self.round_gizmo_style = now.info.style;
        let shown = format!("{:.1}", self.unit.to_display_val(now.info.radius));
        match now.info.kind {
            RoundKind::Vertex => {
                self.active_vertex = Some((now.body, now.ray, now.anchor));
                self.active_edge = None;
                self.vertex_gizmo_radius = signed;
                self.vertex_gizmo_edit_input = shown;
                self.update_round_preview_cache(RoundKind::Vertex, signed);
            }
            RoundKind::Edge => {
                self.active_edge = Some((now.body, now.ray, now.anchor));
                self.active_vertex = None;
                self.edge_gizmo_radius = signed;
                self.edge_gizmo_edit_input = shown;
                self.update_round_preview_cache(RoundKind::Edge, signed);
            }
        }
        self.model_status = Some(
            "Rounding terpilih — tarik = fillet bulat, dorong = chamfer lurus, dorong sampai 0 utk kembali menyiku"
                .to_string(),
        );
        true
    }

    /// Pratinjau edit op rounding: shape sebelum op + rounding baru, lalu
    /// geser/putar/skala sesudahnya. Edit lain sesudah op tidak ikut
    /// dipratinjau (commit memakai replay penuh).
    pub(crate) fn round_op_preview_shape(&self, radius: f64) -> Option<(BodyId, KernelShape)> {
        let op_id = self.editing_round_op.as_ref()?;
        let m = self.round_markers.iter().find(|m| &m.op_id == op_id)?;
        let now = self.round_marker_now(m)?;
        let (_, _, base) = self.round_op_base.as_ref().filter(|(id, _, _)| id == op_id)?;
        let tol = Self::EDGE_REAPPLY_TOLERANCE_MM;
        let ray = m.ray();
        let mut shape = if radius.abs() < Self::ROUND_SHARP_MM {
            ducad_kernel::clone_shape(base).ok()?
        } else {
            let r = radius.abs();
            match (now.info.kind, radius > 0.0) {
                (RoundKind::Edge, true) => ducad_kernel::fillet_edges(base, r, &[ray], tol),
                (RoundKind::Edge, false) => ducad_kernel::chamfer_edges(base, r, &[ray], tol),
                (RoundKind::Vertex, true) => ducad_kernel::fillet_vertex(base, r, ray, tol),
                (RoundKind::Vertex, false) => ducad_kernel::chamfer_vertex(base, r, ray, tol),
            }
            .ok()?
        };
        for t in &now.transforms {
            shape = t.apply_shape(&shape).ok()?;
        }
        Some((now.body, shape))
    }

    /// Commit edit op rounding: radius ≈ 0 → hapus op; selain itu ganti op
    /// (gaya bisa berubah fillet ↔ chamfer) lalu replay.
    pub(crate) fn commit_round_op_edit(&mut self, kind: RoundKind, op_id: &str, signed: f64) {
        self.round_op_base = None;
        if signed.abs() < Self::ROUND_SHARP_MM {
            self.delete_op(op_id);
            self.model_status = Some("Rounding dihapus — sudut kembali menyiku".to_string());
            self.clear_round_gizmo(kind);
            return;
        }
        let Some(op) = self.agent_meta.design.oplog.iter().find(|o| o.id() == op_id).cloned() else {
            self.clear_round_gizmo(kind);
            return;
        };
        let Some(mut v) = op_json(&op) else {
            self.clear_round_gizmo(kind);
            return;
        };
        let magnitude = signed.abs();
        let chamfer = signed < 0.0;
        match kind {
            RoundKind::Vertex => {
                v["radius"] = serde_json::json!(magnitude);
                v["chamfer"] = serde_json::json!(chamfer);
            }
            RoundKind::Edge => {
                if let Some(obj) = v.as_object_mut() {
                    obj.remove("radius");
                    obj.remove("distance");
                    obj.remove("radius_end");
                    if chamfer {
                        obj.insert("op".into(), "chamfer".into());
                        obj.insert("distance".into(), magnitude.into());
                    } else {
                        obj.insert("op".into(), "fillet".into());
                        obj.insert("radius".into(), magnitude.into());
                    }
                }
            }
        }
        let label = if chamfer { "Chamfer" } else { "Fillet" };
        let result = serde_json::from_value::<Op>(v)
            .map_err(|e| OpError::invalid(format!("op GUI tidak valid: {e}")))
            .and_then(|op| {
                self.edit_oplog_via_replay(
                    vec![ducad_engine::ReplaceOp { id: op_id.to_string(), op }],
                    Vec::new(),
                    label,
                )
            });
        match result {
            Ok(()) => {
                if self.adopt_after_op(op_id).is_none() {
                    self.model_status = Some(format!(
                        "{label} {magnitude:.1} mm sukses — klik sudutnya lagi utk mengubah/menghapus"
                    ));
                }
            }
            Err(e) => self.report_gui_op_error(label, &e),
        }
        self.clear_round_gizmo(kind);
    }

    /// JSON op rounding baru pada `shape` body `body` (id op `op_id`).
    fn new_round_op_json(
        shape: &KernelShape,
        op_id: &str,
        body: &str,
        kind: RoundKind,
        ray: PickRay,
        anchor: (f64, f64, f64),
        signed: f64,
    ) -> Option<serde_json::Value> {
        let magnitude = signed.abs();
        let chamfer = signed < 0.0;
        match kind {
            RoundKind::Vertex => Some(serde_json::json!({
                "op": "round_vertex", "id": op_id, "body": body,
                "near": [anchor.0, anchor.1, anchor.2], "radius": magnitude, "chamfer": chamfer,
            })),
            RoundKind::Edge => {
                let picked =
                    ducad_kernel::edges_for_rays(shape, &[ray], Self::EDGE_REAPPLY_TOLERANCE_MM).ok()?;
                let sel = ducad_engine::select::suggest_edges(shape, &picked)?.selector;
                Some(if chamfer {
                    serde_json::json!({ "op": "chamfer", "id": op_id, "body": body, "edges": sel, "distance": magnitude })
                } else {
                    serde_json::json!({ "op": "fillet", "id": op_id, "body": body, "edges": sel, "radius": magnitude })
                })
            }
        }
    }

    /// Rounding gizmo BARU sebagai op. `None` = tidak bisa dijadikan op
    /// (pemanggil memakai jalur lama), `Some(ok)` = sudah ditangani.
    pub(crate) fn commit_new_round_op(
        &mut self,
        kind: RoundKind,
        body_id: BodyId,
        ray: PickRay,
        anchor: (f64, f64, f64),
        signed: f64,
    ) -> Option<bool> {
        let shape = &self.model.geometry.get(body_id)?.shape;
        let polyline = match kind {
            RoundKind::Edge => ducad_kernel::pick_edge(shape, ray, Self::EDGE_REAPPLY_TOLERANCE_MM)
                .map(|(_, pl)| pl)
                .unwrap_or_default(),
            RoundKind::Vertex => Vec::new(),
        };
        // Selector dicoba dulu: gagal → jalur lama, sebelum mengubah apa pun.
        Self::new_round_op_json(shape, "_", "_", kind, ray, anchor, signed)?;
        let prefix = match (kind, signed < 0.0) {
            (RoundKind::Vertex, _) => "round",
            (RoundKind::Edge, false) => "fillet",
            (RoundKind::Edge, true) => "chamfer",
        };
        let op_id = self.unique_op_id(prefix);
        let label = if signed < 0.0 { "Chamfer" } else { "Fillet" };
        let id_for_op = op_id.clone();
        let r = self.direct_op_via_oplog(body_id, label, move |app, body| {
            let shape = &app.model.geometry.get(body_id)?.shape;
            Self::new_round_op_json(shape, &id_for_op, body, kind, ray, anchor, signed)
        });
        let ok = self.try_gui_op(label, r)?;
        if ok {
            self.round_markers.push(RoundMarker {
                op_id,
                vertex: kind == RoundKind::Vertex,
                ray_origin: ray.origin,
                ray_dir: ray.dir,
                anchor,
                polyline,
            });
            self.model_status = Some(format!(
                "{label} {:.1} mm sukses — klik sudutnya lagi utk mengubah/menghapus",
                signed.abs()
            ));
        }
        Some(ok)
    }

    /// `id` belum dipakai op, nama body, maupun sketsa.
    pub(crate) fn op_id_free(&self, id: &str) -> bool {
        !self.agent_meta.design.oplog.iter().any(|o| o.id() == id)
            && !self.model.doc.bodies.values().any(|b| b.name == id)
            && !self.agent_meta.sketch_ids.contains_key(id)
    }

    /// Pindahkan riwayat rounding LAMA (`round_history`, dari berkas atau
    /// sesi sebelum rounding berbasis op) ke oplog: `adopt` shape dasar, lalu
    /// tiap rounding dan tarik sisi sebagai op berurutan. Sesudahnya semua
    /// rounding body itu bisa diedit lewat op. `false` = gagal; riwayat lama
    /// tetap dipakai.
    pub(crate) fn migrate_legacy_rounds(&mut self, body_id: BodyId) -> bool {
        use crate::types::RoundPostOp;
        let Some(h) = self.fresh_round_history(body_id) else {
            return false;
        };
        let Ok(base) = ducad_kernel::clone_shape(&h.base) else {
            return false;
        };
        let features = h.features.clone();
        let mut post_ops = h.post_ops.clone();
        post_ops.sort_by_key(|s| s.after);
        self.sync_agent_meta();
        let renamed = self.make_body_name_unique(body_id);
        let Some(body) = self.model.doc.bodies.get(body_id).map(|b| b.name.clone()) else {
            return false;
        };

        let mut taken: Vec<String> = Vec::new();
        let mut fresh_id = |app: &Self, prefix: &str| {
            let id = (1..)
                .map(|n| format!("{prefix}{n}"))
                .find(|id| !taken.contains(id) && app.op_id_free(id))
                .unwrap_or_else(|| prefix.to_string());
            taken.push(id.clone());
            id
        };
        let push_pull = |shape: &KernelShape, id: String, ray: PickRay, distance: f64| {
            let fi = ducad_kernel::face_index_for_ray(shape, ray)?;
            let sel = ducad_engine::select::suggest_faces(shape, &[fi])?.selector;
            let next = ducad_kernel::extrude_face(shape, ray, distance).ok()?;
            let op = serde_json::json!({
                "op": "push_pull", "id": id, "body": body, "face": sel, "distance": distance,
            });
            Some((op, next))
        };

        let built = (|| {
            let mut ops = vec![crate::oplog_direct::adopt_op_json(&fresh_id(self, "adopt"), &body, &base).ok()?];
            let mut markers = Vec::new();
            let mut shape = base;
            let mut post = post_ops.iter().peekable();
            for (i, f) in features.iter().enumerate() {
                while let Some(p) = post.next_if(|p| p.after <= i) {
                    let RoundPostOp::ExtrudeFace { ray, distance } = p.op;
                    let (op, next) = push_pull(&shape, fresh_id(self, "push_pull"), ray, distance)?;
                    ops.push(op);
                    shape = next;
                }
                let signed = match f.style {
                    RoundStyle::Fillet => f.radius,
                    RoundStyle::Chamfer => -f.radius,
                };
                let prefix = if f.kind == RoundKind::Vertex { "round" } else { "fillet" };
                let op_id = fresh_id(self, prefix);
                let mut op = Self::new_round_op_json(&shape, &op_id, &body, f.kind, f.ray, f.anchor, signed)?;
                if let (Some(r_end), RoundStyle::Fillet, RoundKind::Edge) = (f.radius_end, f.style, f.kind) {
                    op["radius_end"] = serde_json::json!(r_end);
                }
                ops.push(op);
                shape = Self::build_rounded_shape(&shape, std::slice::from_ref(f)).ok()?;
                markers.push(RoundMarker {
                    op_id,
                    vertex: f.kind == RoundKind::Vertex,
                    ray_origin: f.ray.origin,
                    ray_dir: f.ray.dir,
                    anchor: f.anchor,
                    polyline: f.polyline.clone(),
                });
            }
            for p in post {
                let RoundPostOp::ExtrudeFace { ray, distance } = p.op;
                let (op, next) = push_pull(&shape, fresh_id(self, "push_pull"), ray, distance)?;
                ops.push(op);
                shape = next;
            }
            let ops = ops
                .into_iter()
                .map(serde_json::from_value::<Op>)
                .collect::<Result<Vec<_>, _>>()
                .ok()?;
            Some((ops, markers))
        })();

        let ok = match built {
            Some((ops, markers)) => match self.run_gui_ops(ops, "Rounding → Feature Tree", &body) {
                Ok(_) => {
                    self.round_history.remove(&body_id);
                    self.round_markers.extend(markers);
                    true
                }
                Err(e) => {
                    log::warn!("migrasi rounding lama gagal: {e}");
                    false
                }
            },
            None => false,
        };
        if let Some(mut cmd) = renamed {
            if ok {
                self.model_undo.attach_to_top(cmd);
            } else {
                ducad_core::Command::revert(cmd.as_mut(), &mut self.model);
            }
        }
        ok
    }
}
