//! Alat edit langsung GUI sebagai op oplog (ADR 0010).
//!
//! Dulu alat seperti geser/putar/skala body, tarik sisi, rib, atau fillet
//! gizmo mengganti geometri tanpa mencatat op. `sync_agent_meta` lalu melihat
//! sidik jari model berbeda dari oplog dan MEMBUANG seluruh Feature Tree —
//! semua fitur sebelumnya tidak bisa diedit lagi. Di sini:
//!
//! - tiap alat diterjemahkan ke op parametrik (bisa diedit kapan pun);
//! - edit langsung yang belum punya op dicatat sebagai op `adopt` (geometri
//!   beku) lewat [`DuCADApp::begin_direct_edit_guard`], sehingga Feature Tree
//!   tidak pernah hilang.

#![allow(clippy::result_large_err)]

use ducad_core::BodyId;
use ducad_engine::ops::Op;
use ducad_engine::OpError;
use ducad_kernel::PickRay;
use glam::DVec3;

use crate::app::DuCADApp;
use crate::oplog_undo::OplogMark;

/// Keadaan sebelum edit langsung (lihat [`DuCADApp::begin_direct_edit_guard`]).
pub(crate) struct DirectEditGuard {
    mark: OplogMark,
    /// `(body, nama, mesh_fingerprint)` sebelum command.
    before: Vec<(BodyId, String, u64)>,
}

fn arr(v: DVec3) -> [f64; 3] {
    [v.x, v.y, v.z]
}

/// Op `adopt` sebagai JSON (dipakai migrasi riwayat lama).
pub(crate) fn adopt_op_json(
    id: &str,
    body: &str,
    shape: &ducad_kernel::KernelShape,
) -> Result<serde_json::Value, OpError> {
    let op = ducad_engine::adopt_op(id, body, shape)?;
    serde_json::to_value(op).map_err(|e| OpError::invalid(format!("op adopt tidak valid: {e}")))
}

fn op_from(v: serde_json::Value) -> Result<Op, OpError> {
    serde_json::from_value(v).map_err(|e| OpError::invalid(format!("op GUI tidak valid: {e}")))
}

impl DuCADApp {
    /// Tampilkan error op aksi GUI (status + kartu error dengan saran).
    pub(crate) fn report_gui_op_error(&mut self, label: &str, e: &OpError) {
        self.model_status = Some(crate::modeling::operations::op_status(label, e));
        let fixes = self.fixes_for_op_error(e);
        self.show_op_error(e, fixes);
    }

    /// Jalankan aksi oplog: `Some(true)` sukses, `Some(false)` gagal (error
    /// sudah ditampilkan), `None` = belum didukung → pakai jalur lama.
    pub(crate) fn try_gui_op(&mut self, label: &str, r: Result<bool, OpError>) -> Option<bool> {
        match r {
            Ok(true) => Some(true),
            Ok(false) => None,
            Err(e) => {
                self.report_gui_op_error(label, &e);
                Some(false)
            }
        }
    }

    // --- Pengaman: edit langsung tidak lagi membuang Feature Tree ---------

    /// Dipanggil SEBELUM command geometri langsung. `None` = tidak perlu
    /// dicatat (oplog kosong, atau pemanggil mencatat op-nya sendiri).
    pub(crate) fn begin_direct_edit_guard(&mut self) -> Option<DirectEditGuard> {
        if self.skip_direct_edit_guard || self.agent_meta.design.oplog.is_empty() {
            return None;
        }
        // Oplog yang SUDAH basi dibuang di sini (perilaku lama); yang masih
        // sinkron dipertahankan dan edit ini ditambahkan sebagai op.
        self.sync_agent_meta();
        if self.agent_meta.design.oplog.is_empty() {
            return None;
        }
        let before = self
            .model
            .doc
            .bodies
            .iter()
            .filter_map(|(id, b)| {
                let fp = self.model.geometry.get(id)?.mesh_fingerprint;
                Some((id, b.name.clone(), fp))
            })
            .collect();
        Some(DirectEditGuard { mark: self.begin_oplog_step(), before })
    }

    /// Dipanggil SESUDAH command: body yang hilang → op `delete`, body baru
    /// atau berubah → op `adopt`. Oplog ikut langkah undo yang sama.
    pub(crate) fn finish_direct_edit_guard(&mut self, guard: DirectEditGuard, label: &str) {
        let mut ops: Vec<Op> = Vec::new();
        for (id, name, _) in &guard.before {
            if !self.model.doc.bodies.contains_key(*id) {
                let id_op = self.unique_op_id("delete");
                match op_from(serde_json::json!({ "op": "delete", "id": id_op, "body": name })) {
                    Ok(op) => ops.push(op),
                    Err(e) => log::warn!("op delete pengaman gagal: {e}"),
                }
            }
        }
        let changed: Vec<BodyId> = self
            .model
            .doc
            .bodies
            .iter()
            .filter(|(id, _)| {
                let fp = self.model.geometry.get(*id).map(|g| g.mesh_fingerprint);
                !guard
                    .before
                    .iter()
                    .any(|(b, _, old)| b == id && Some(*old) == fp)
            })
            .map(|(id, _)| id)
            .collect();
        for id in changed {
            if let Some(cmd) = self.make_body_name_unique(id) {
                self.model_undo.attach_to_top(cmd);
            }
            let (Some(meta), Some(geo)) = (self.model.doc.bodies.get(id), self.model.geometry.get(id))
            else {
                continue;
            };
            let op_id = self.unique_op_id("adopt");
            match ducad_engine::adopt_op(&op_id, &meta.name, &geo.shape) {
                Ok(op) => ops.push(op),
                Err(e) => log::warn!("op adopt pengaman gagal: {e}"),
            }
        }
        if ops.is_empty() {
            return;
        }
        self.agent_meta.design.oplog.extend(ops);
        self.sync_design_after_agent();
        self.end_oplog_step(guard.mark, label);
    }

    // --- Body utuh ----------------------------------------------------------

    /// Geser dan/atau putar body sebagai op `transform`. `copy` = hasilnya
    /// body baru (bernama id op), body asli tetap.
    #[allow(clippy::result_large_err)]
    pub(crate) fn transform_body_via_oplog(
        &mut self,
        id: BodyId,
        translate: Option<DVec3>,
        rotate: Option<(DVec3, DVec3, f64)>,
        copy: bool,
    ) -> Result<bool, OpError> {
        let title = if copy { "Salin Body" } else if rotate.is_some() { "Putar Body" } else { "Geser Body" };
        self.direct_op_via_oplog(id, title, |app, body| {
            let op_id = app.unique_op_id(if copy { "copy" } else { "move" });
            let mut v = serde_json::json!({ "op": "transform", "id": op_id, "body": body });
            if let Some(t) = translate {
                v["translate"] = serde_json::json!(arr(t));
            }
            if let Some((pivot, axis, angle_deg)) = rotate {
                v["rotate"] = serde_json::json!({
                    "pivot": arr(pivot), "axis": arr(axis), "angle_deg": angle_deg
                });
            }
            if copy {
                v["copy"] = serde_json::json!(true);
            }
            Some(v)
        })
    }

    /// Skala uniform body sebagai op `scale`.
    #[allow(clippy::result_large_err)]
    pub(crate) fn scale_body_via_oplog(
        &mut self,
        id: BodyId,
        factor: f64,
        pivot: DVec3,
        copy: bool,
    ) -> Result<bool, OpError> {
        self.direct_op_via_oplog(id, if copy { "Salin Body" } else { "Resize Body" }, |app, body| {
            let mut v = serde_json::json!({
                "op": "scale", "id": app.unique_op_id(if copy { "copy" } else { "scale" }),
                "body": body, "factor": factor, "pivot": arr(pivot),
            });
            if copy {
                v["copy"] = serde_json::json!(true);
            }
            Some(v)
        })
    }

    /// Cermin body: `copy` = salinan baru, selain itu body diganti bayangannya.
    #[allow(clippy::result_large_err)]
    pub(crate) fn mirror_body_via_oplog(
        &mut self,
        id: BodyId,
        point: DVec3,
        normal: DVec3,
        copy: bool,
    ) -> Result<bool, OpError> {
        self.direct_op_via_oplog(id, "Cermin Body", |app, body| {
            Some(serde_json::json!({
                "op": "mirror", "id": app.unique_op_id("mirror"), "body": body,
                "plane": { "point": arr(point), "normal": arr(normal) }, "copy": copy,
            }))
        })
    }

    /// Hapus body sebagai op `delete`.
    #[allow(clippy::result_large_err)]
    pub(crate) fn delete_body_via_oplog(&mut self, id: BodyId) -> Result<bool, OpError> {
        self.direct_op_via_oplog(id, "Delete Body", |app, body| {
            Some(serde_json::json!({ "op": "delete", "id": app.unique_op_id("delete"), "body": body }))
        })
    }

    /// Body primitif baru sebagai op `primitive` (body bernama id op).
    #[allow(clippy::result_large_err)]
    pub(crate) fn primitive_via_oplog(
        &mut self,
        prefix: &str,
        shape: serde_json::Value,
        at: [f64; 3],
        title: &str,
    ) -> Result<(), OpError> {
        self.sync_agent_meta();
        let id = self.unique_op_id(prefix);
        let op = op_from(serde_json::json!({ "op": "primitive", "id": id, "shape": shape, "at": at }))?;
        let details = format!("{title} {id}");
        self.run_gui_ops(vec![op], title, &details).map(|_| ())
    }

    /// Ubah panjang body sepanjang rusuk sebagai op `resize_edge`.
    #[allow(clippy::result_large_err)]
    pub(crate) fn resize_edge_via_oplog(
        &mut self,
        id: BodyId,
        start: [f64; 3],
        end: [f64; 3],
        length: f64,
    ) -> Result<bool, OpError> {
        self.direct_op_via_oplog(id, "Ubah Ukuran Rusuk", |app, body| {
            Some(serde_json::json!({
                "op": "resize_edge", "id": app.unique_op_id("resize"), "body": body,
                "start": start, "end": end, "length": length,
            }))
        })
    }

    // --- Sisi (face) -------------------------------------------------------

    /// Selector 1 face body `id` yang kena `ray`.
    fn face_selector_for_ray(&self, id: BodyId, ray: PickRay) -> Option<String> {
        let shape = &self.model.geometry.get(id)?.shape;
        let face = ducad_kernel::face_index_for_ray(shape, ray)?;
        self.face_selector(id, &[face])
    }

    /// Tarik/potong sisi (push/pull) sebagai op `push_pull`.
    #[allow(clippy::result_large_err)]
    pub(crate) fn push_pull_via_oplog(
        &mut self,
        id: BodyId,
        ray: PickRay,
        distance: f64,
    ) -> Result<bool, OpError> {
        let Some(face) = self.face_selector_for_ray(id, ray) else {
            return Ok(false);
        };
        let title = if distance >= 0.0 { "Extrude Face" } else { "Cut Face" };
        self.direct_op_via_oplog(id, title, |app, body| {
            Some(serde_json::json!({
                "op": "push_pull", "id": app.unique_op_id("push_pull"), "body": body,
                "face": face, "distance": distance,
            }))
        })
    }

    /// Putar sisi datar mengelilingi sumbu sebagai op `revolve_face`.
    #[allow(clippy::result_large_err)]
    pub(crate) fn revolve_face_via_oplog(
        &mut self,
        id: BodyId,
        ray: PickRay,
        origin: DVec3,
        dir: DVec3,
        angle_deg: Option<f64>,
    ) -> Result<bool, OpError> {
        let Some(face) = self.face_selector_for_ray(id, ray) else {
            return Ok(false);
        };
        self.direct_op_via_oplog(id, "Revolve Face", |app, body| {
            let mut v = serde_json::json!({
                "op": "revolve_face", "id": app.unique_op_id("revolve_face"), "body": body,
                "face": face, "axis": { "origin": arr(origin), "dir": arr(dir) },
            });
            if let Some(a) = angle_deg {
                v["angle_deg"] = serde_json::json!(a);
            }
            Some(v)
        })
    }

    /// Tulang penguat sebagai op `rib`.
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    pub(crate) fn rib_via_oplog(
        &mut self,
        id: BodyId,
        start: DVec3,
        end: DVec3,
        normal: DVec3,
        thickness: f64,
        depth: f64,
        draft_deg: Option<f64>,
    ) -> Result<bool, OpError> {
        self.direct_op_via_oplog(id, "Add Rib", |app, body| {
            let mut v = serde_json::json!({
                "op": "rib", "id": app.unique_op_id("rib"), "body": body,
                "start": arr(start), "end": arr(end), "normal": arr(normal),
                "thickness": thickness, "depth": depth,
            });
            if let Some(d) = draft_deg {
                v["draft_deg"] = serde_json::json!(d);
            }
            Some(v)
        })
    }

    /// Shell bertebal variabel sebagai op `shell_variable`. Face yang tidak
    /// bisa dijadikan selector → `Ok(false)` (jalur lama).
    #[allow(clippy::result_large_err)]
    pub(crate) fn shell_variable_via_oplog(
        &mut self,
        id: BodyId,
        thickness: f64,
        remove_rays: &[PickRay],
        custom: &[(PickRay, f64)],
    ) -> Result<bool, OpError> {
        let Some(shape) = self.model.geometry.get(id).map(|g| &g.shape) else {
            return Ok(false);
        };
        let mut remove = Vec::new();
        for r in remove_rays {
            let Some(f) = ducad_kernel::face_index_for_ray(shape, *r) else {
                return Ok(false);
            };
            remove.push(f);
        }
        let mut custom_faces = Vec::new();
        for (r, t) in custom {
            let Some(f) = ducad_kernel::face_index_for_ray(shape, *r) else {
                return Ok(false);
            };
            custom_faces.push((f, *t));
        }
        let mut custom_json = Vec::new();
        for (f, t) in custom_faces {
            let Some(sel) = self.face_selector(id, &[f]) else {
                return Ok(false);
            };
            custom_json.push(serde_json::json!({ "faces": sel, "thickness": t }));
        }
        let remove_sel = if remove.is_empty() {
            None
        } else {
            match self.face_selector(id, &remove) {
                Some(s) => Some(s),
                None => return Ok(false),
            }
        };
        self.direct_op_via_oplog(id, "Shell Variable", |app, body| {
            let mut v = serde_json::json!({
                "op": "shell_variable", "id": app.unique_op_id("shell"), "body": body,
                "thickness": thickness,
            });
            if let Some(s) = remove_sel {
                v["remove_faces"] = serde_json::json!(s);
            }
            if !custom_json.is_empty() {
                v["custom"] = serde_json::json!(custom_json);
            }
            Some(v)
        })
    }

    // --- Hole lama (jalur kernel langsung) ---------------------------------

    /// Indeks face lubang di `pos` sepanjang sumbu `normal`: dinding
    /// silinder/kerucut cekung yang sesumbu, plus lantai planar kecil
    /// (dasar blind/counterbore) yang terpusat di sumbu.
    fn hole_face_indices(shape: &ducad_kernel::KernelShape, pos: DVec3, normal: DVec3) -> Vec<usize> {
        let n = normal.normalize_or_zero();
        let off_axis = |c: DVec3| {
            let d = c - pos;
            (d - n * d.dot(n)).length()
        };
        let parallel = |d: DVec3| d.normalize_or_zero().cross(n).length() < 1e-3;
        let faces = ducad_kernel::enumerate_faces(shape);
        let mut picked = Vec::new();
        let mut r_max: f64 = 0.0;
        for f in &faces {
            let curved = matches!(f.kind, ducad_kernel::SurfaceKind::Cylinder | ducad_kernel::SurfaceKind::Cone);
            if !curved || f.concave != Some(true) {
                continue;
            }
            let Some((o, dir)) = f.axis else { continue };
            if parallel(DVec3::from(dir)) && off_axis(DVec3::from(o)) < 0.05 {
                picked.push(f.index);
                r_max = r_max.max(f.radius.unwrap_or(0.0));
            }
        }
        if picked.is_empty() {
            return picked;
        }
        let max_diag = 2.0 * r_max * std::f64::consts::SQRT_2 * 1.05 + 0.1;
        for f in &faces {
            if f.kind != ducad_kernel::SurfaceKind::Plane || !parallel(DVec3::from(f.normal)) {
                continue;
            }
            let diag = (DVec3::from(f.bbox.1) - DVec3::from(f.bbox.0)).length();
            if off_axis(DVec3::from(f.centroid)) < 0.05 && diag <= max_diag {
                picked.push(f.index);
            }
        }
        picked
    }

    /// Edit lubang LAMA yang body-nya sudah diubah sesudahnya. Membangun ulang
    /// dari bentuk dasar lama akan membuang perubahan itu, jadi lubang lama
    /// dihapus (`delete_face`) lalu dibuat ulang sebagai op `hole`.
    /// `None` = bukan kasus ini (pemanggil lanjut seperti biasa).
    pub(crate) fn rehole_stale_legacy(
        &mut self,
        body_id: BodyId,
        ray: PickRay,
        pos: (f64, f64, f64),
        spec: &ducad_core::hole::HoleSpec,
    ) -> Option<bool> {
        let (b, idx) = self.editing_hole_idx?;
        if b != body_id {
            return None;
        }
        let fp = self.model.geometry.get(body_id)?.mesh_fingerprint;
        let hist = self.hole_history.get(&body_id)?;
        if hist.result_fingerprint == fp {
            return None;
        }
        let old = hist.features.get(idx)?.clone();
        let faces = Self::hole_face_indices(
            &self.model.geometry.get(body_id)?.shape,
            DVec3::from(old.pos),
            DVec3::from(old.normal),
        );
        let Some(sel) = (!faces.is_empty()).then(|| self.face_selector(body_id, &faces)).flatten() else {
            self.model_status = Some(
                "Lubang lama tidak ditemukan lagi pada body (bentuknya sudah berubah) — hapus sisinya dengan Delete Face lalu buat lubang baru".to_string(),
            );
            return Some(false);
        };
        let r = self.direct_op_via_oplog(body_id, "Hapus lubang lama", |app, body| {
            Some(serde_json::json!({
                "op": "delete_face", "id": app.unique_op_id("delete_face"), "body": body, "faces": sel,
            }))
        });
        if self.try_gui_op("Edit Hole", r) != Some(true) {
            return Some(false);
        }
        match self.hole_via_oplog(body_id, ray, pos, spec, false) {
            Ok(true) => {}
            Ok(false) => {
                self.undo_model();
                self.model_status = Some("Ukuran lubang ini belum bisa dibuat ulang sebagai fitur".to_string());
                return Some(false);
            }
            Err(e) => {
                self.undo_model();
                self.report_gui_op_error("Edit Hole", &e);
                return Some(false);
            }
        }
        if let Some(h) = self.hole_history.get_mut(&body_id) {
            if idx < h.features.len() {
                h.features.remove(idx);
            }
            if h.features.is_empty() {
                self.hole_history.remove(&body_id);
            }
        }
        Some(true)
    }


    // --- Perubahan geometri di luar command ---------------------------------

    /// Perubahan geometri yang tidak lewat command (part eksternal dimuat
    /// ulang, regenerasi timeline lama): catat sebagai op `adopt` supaya
    /// oplog tidak dianggap basi lalu dibuang. Tanpa langkah undo (sama
    /// seperti perubahannya sendiri).
    pub(crate) fn record_raw_geometry_change(&mut self, bodies: &[BodyId]) {
        if self.agent_meta.design.oplog.is_empty() {
            return;
        }
        for &id in bodies {
            // Nama unik langsung berlaku (perubahan ini juga tanpa undo).
            let _ = self.make_body_name_unique(id);
            let (Some(meta), Some(geo)) = (self.model.doc.bodies.get(id), self.model.geometry.get(id)) else {
                continue;
            };
            let op_id = self.unique_op_id("adopt");
            match ducad_engine::adopt_op(&op_id, &meta.name, &geo.shape) {
                Ok(op) => self.agent_meta.design.oplog.push(op),
                Err(e) => log::warn!("op adopt gagal: {e}"),
            }
        }
        self.oplog_fp_dirty = true;
    }

    /// `(body, mesh_fingerprint)` semua body — pasangan
    /// [`Self::record_changed_since`].
    pub(crate) fn body_fingerprints(&self) -> Vec<(BodyId, u64)> {
        self.model.geometry.iter().map(|(id, g)| (id, g.mesh_fingerprint)).collect()
    }

    /// Catat body yang berubah/baru sejak `before` lewat
    /// [`Self::record_raw_geometry_change`].
    pub(crate) fn record_changed_since(&mut self, before: &[(BodyId, u64)]) {
        let changed: Vec<BodyId> = self
            .model
            .geometry
            .iter()
            .filter(|(id, g)| !before.iter().any(|(b, fp)| b == id && *fp == g.mesh_fingerprint))
            .map(|(id, _)| id)
            .collect();
        if !changed.is_empty() {
            self.record_raw_geometry_change(&changed);
        }
    }

    /// Body dipindah solver perakitan (putar `angle_rad` di sekitar `pivot`
    /// lalu geser `translation`). Dicatat sebagai op `transform` `place<n>`;
    /// gerakan beruntun body yang sama digabung ke op terakhir supaya Feature
    /// Tree tidak bertambah tiap frame.
    pub(crate) fn note_rigid_move(&mut self, body: BodyId, tf: &ducad_kernel::MateTransformResult) {
        use glam::DQuat;
        if self.agent_meta.design.oplog.is_empty() {
            return;
        }
        let Some(name) = self.model.doc.bodies.get(body).map(|b| b.name.clone()) else {
            return;
        };
        let p = DVec3::from(tf.pivot);
        let t = DVec3::from(tf.translation);
        let axis = DVec3::from(tf.axis).normalize_or_zero();
        let rd = if axis == DVec3::ZERO || tf.angle_rad.abs() < 1e-12 {
            DQuat::IDENTITY
        } else {
            DQuat::from_axis_angle(axis, tf.angle_rad)
        };
        // Δ: x → rd·x + cd.
        let cd = p + t - rd * p;
        // Op terakhir = `place` body ini → gabungkan.
        let last = self.agent_meta.design.oplog.last().and_then(|o| {
            let v = serde_json::to_value(o).ok()?;
            let is_place = o.id().starts_with("place")
                && v["op"] == "transform"
                && v["body"] == name.as_str();
            is_place.then_some(v)
        });
        let vec3 = |v: &serde_json::Value| -> Option<DVec3> {
            let a = v.as_array()?;
            Some(DVec3::new(a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?))
        };
        let (r_new, c_new, replace) = match &last {
            Some(v) => {
                // Op `place` selalu berpivot 0: x → R·(x + t) = R·x + R·t.
                let t_old = vec3(&v["translate"]).unwrap_or(DVec3::ZERO);
                let r_old = match (vec3(&v["rotate"]["axis"]), v["rotate"]["angle_deg"].as_f64()) {
                    (Some(a), Some(deg)) if a.length() > 1e-12 => {
                        DQuat::from_axis_angle(a.normalize(), deg.to_radians())
                    }
                    _ => DQuat::IDENTITY,
                };
                let c_old = r_old * t_old;
                (rd * r_old, rd * c_old + cd, true)
            }
            None => (rd, cd, false),
        };
        let translate = r_new.inverse() * c_new;
        let (ax, ang) = r_new.to_axis_angle();
        let op_id = if replace {
            self.agent_meta.design.oplog.last().map(|o| o.id().to_string()).unwrap_or_default()
        } else {
            self.unique_op_id("place")
        };
        let mut v = serde_json::json!({
            "op": "transform", "id": op_id, "body": name, "translate": arr(translate),
        });
        if ang.abs() > 1e-12 {
            v["rotate"] = serde_json::json!({ "pivot": [0.0, 0.0, 0.0], "axis": arr(ax), "angle_deg": ang.to_degrees() });
        }
        let Ok(op) = op_from(v) else { return };
        if replace {
            self.agent_meta.design.oplog.pop();
        }
        self.agent_meta.design.oplog.push(op);
        self.oplog_fp_dirty = true;
    }


    // --- Peringatan: edit tertimpa langkah `adopt` -------------------------

    /// Op `adopt` sesudah op `op_id` pada body yang sama: edit pada `op_id`
    /// tidak terlihat karena geometri body ditimpa versi beku itu.
    pub(crate) fn adopt_after_op(&self, op_id: &str) -> Option<String> {
        let oplog = &self.agent_meta.design.oplog;
        let idx = oplog.iter().position(|o| o.id() == op_id)?;
        let v = serde_json::to_value(&oplog[idx]).ok()?;
        let mut bodies: Vec<String> = vec![op_id.to_string()];
        for key in ["body", "target"] {
            if let Some(b) = v.get(key).and_then(|b| b.as_str()) {
                bodies.push(b.to_string());
            }
        }
        // Op sketsa: body yang dibuat fitur pemakainya.
        for o in oplog.iter().skip(idx + 1) {
            let Ok(w) = serde_json::to_value(o) else { continue };
            if w.get("sketch").and_then(|x| x.as_str()) == Some(op_id) {
                bodies.push(o.id().to_string());
                if let Some(t) = w.get("target").and_then(|x| x.as_str()) {
                    bodies.push(t.to_string());
                }
            }
        }
        oplog.iter().skip(idx + 1).find_map(|o| {
            let w = serde_json::to_value(o).ok()?;
            let body = w.get("body")?.as_str()?;
            (w.get("op")? == "adopt" && bodies.iter().any(|b| b == body)).then(|| o.id().to_string())
        })
    }

    /// Tampilkan peringatan bila salah satu `op_ids` tertimpa `adopt`.
    /// `true` = ada peringatan.
    pub(crate) fn warn_if_frozen(&mut self, op_ids: &[String]) -> bool {
        let Some((op, adopt)) = op_ids
            .iter()
            .find_map(|id| self.adopt_after_op(id).map(|a| (id.clone(), a)))
        else {
            return false;
        };
        self.model_status = Some(format!(
            "Perubahan '{op}' tersimpan, tapi body-nya masih ditimpa edit langsung '{adopt}' (geometri beku) sesudahnya. Hapus/nonaktifkan '{adopt}' di Feature Tree agar perubahan terlihat."
        ));
        true
    }

}

#[cfg(test)]
mod tests {
    //! Alat edit langsung tidak lagi membuang Feature Tree (oplog), dan fitur
    //! sebelumnya tetap bisa diedit sesudahnya.

    use super::*;
    use crate::model::{BodyGeometry, ReplaceGeometryCommand};
    use crate::types::RoundKind;
    use glam::Vec3;

    /// Kotak 20 mm di titik asal sebagai op `primitive`.
    fn app_with_box() -> (DuCADApp, BodyId) {
        let mut app = DuCADApp::new_for_test();
        app.create_primitive(ducad_ui::PrimitiveKind::Box, [20.0, 20.0, 20.0], [0.0; 3], false);
        let id = app.model.doc.bodies.keys().next().expect("box dibuat");
        assert_eq!(app.agent_meta.design.oplog.len(), 1, "primitif harus jadi op");
        (app, id)
    }

    fn kinds(app: &DuCADApp) -> Vec<String> {
        app.agent_meta
            .design
            .oplog
            .iter()
            .map(|o| serde_json::to_value(o).unwrap()["op"].as_str().unwrap().to_string())
            .collect()
    }

    fn bbox(app: &DuCADApp, id: BodyId) -> ([f64; 3], [f64; 3]) {
        app.model.geometry.get(id).unwrap().shape.bounding_box().unwrap()
    }

    fn select(app: &mut DuCADApp, id: BodyId) {
        app.tool = crate::types::ToolKind::Select;
        app.selected_bodies.clear();
        app.selected_bodies.insert(id);
    }

    /// Replay oplog dari nol harus menghasilkan geometri yang sama.
    fn assert_replays(app: &DuCADApp) {
        let design = ducad_engine::DesignDoc {
            fingerprint: String::new(),
            ..app.agent_meta.design.clone()
        };
        let s = ducad_engine::Session::replay(design).expect("oplog bisa di-replay");
        for (id, b) in app.model.doc.bodies.iter() {
            let live = app.model.geometry.get(id).unwrap();
            let (_, re) = s.body(&b.name).expect("body ada di replay");
            assert!(
                (live.shape.volume() - re.shape.volume()).abs() < 1e-3,
                "volume '{}' beda: live {} vs replay {}",
                b.name,
                live.shape.volume(),
                re.shape.volume()
            );
        }
    }

    fn pull_top(app: &mut DuCADApp, id: BodyId, d: f64) {
        let ray = PickRay { origin: (15.0, 15.0, 80.0), dir: (0.0, 0.0, -1.0) };
        let hit = ducad_kernel::pick_face_details(&app.model.geometry.get(id).unwrap().shape, ray).unwrap();
        app.active_face = Some((id, ray, hit));
        app.extrude_active_face(d);
    }

    #[test]
    fn move_rotate_scale_body_become_ops() {
        let (mut app, id) = app_with_box();
        select(&mut app, id);
        app.translate_selected_body(Vec3::new(40.0, 0.0, 0.0));
        select(&mut app, id);
        app.rotate_selected_body(Vec3::Z, 90.0);
        select(&mut app, id);
        app.scale_selected_body_by_axis(0, 40.0);
        assert_eq!(kinds(&app), ["primitive", "transform", "transform", "scale"]);
        let (min, max) = bbox(&app, id);
        assert!((max[0] - min[0] - 40.0).abs() < 1e-3);
        app.sync_agent_meta();
        assert_eq!(app.agent_meta.design.oplog.len(), 4, "oplog tidak boleh dibuang");
        assert_replays(&app);
    }

    #[test]
    fn push_pull_becomes_op_and_earlier_feature_stays_editable() {
        let (mut app, id) = app_with_box();
        pull_top(&mut app, id, 10.0);
        assert_eq!(kinds(&app), ["primitive", "push_pull"]);
        assert!((bbox(&app, id).1[2] - 30.0).abs() < 1e-3);
        // Ukuran primitif (fitur sebelumnya) masih bisa diedit; tarikan ikut.
        let prim = app.agent_meta.design.oplog[0].clone();
        let mut v = serde_json::to_value(&prim).unwrap();
        v["shape"]["box"]["size"] = serde_json::json!([20.0, 20.0, 40.0]);
        let op = serde_json::from_value(v).unwrap();
        app.edit_oplog_via_replay(
            vec![ducad_engine::ReplaceOp { id: prim.id().to_string(), op }],
            Vec::new(),
            "t",
        )
        .unwrap();
        assert!((bbox(&app, id).1[2] - 50.0).abs() < 1e-3, "tarikan sisi harus ikut");
    }

    /// Laporan pengguna: fillet gizmo → tarik sisi → fillet tetap bisa diedit.
    #[test]
    fn gizmo_fillet_stays_editable_after_push_pull_move_and_undo() {
        let (mut app, id) = app_with_box();
        let ray = PickRay { origin: (-5.0, -5.0, 10.0), dir: (1.0, 1.0, 0.0) };
        app.active_edge = Some((id, ray, (0.0, 0.0, 10.0)));
        app.edge_gizmo_radius = 3.0;
        app.commit_round(RoundKind::Edge);
        assert_eq!(kinds(&app), ["primitive", "fillet"]);
        let op_id = app.round_markers[0].op_id.clone();

        pull_top(&mut app, id, 10.0);
        select(&mut app, id);
        app.translate_selected_body(Vec3::new(40.0, 0.0, 0.0));
        let v_r3 = app.model.geometry.get(id).unwrap().shape.volume();

        // Penanda mengikuti geseran body.
        let now = app.round_marker_now(&app.round_markers[0].clone()).expect("penanda hidup");
        assert!((now.anchor.0 - 40.0).abs() < 1e-9);

        assert!(app.begin_round_op_edit(&op_id));
        assert!(app.round_op_preview_shape(6.0).is_some(), "pratinjau edit harus ada");
        app.edge_gizmo_radius = 6.0;
        app.commit_round(RoundKind::Edge);
        assert!((app.round_op_info(&op_id).unwrap().radius - 6.0).abs() < 1e-9);
        assert_eq!(kinds(&app), ["primitive", "fillet", "push_pull", "transform"]);
        assert!((bbox(&app, id).1[2] - 30.0).abs() < 1e-3, "tarikan sisi tetap ada");
        assert!((bbox(&app, id).0[0] - 40.0).abs() < 1e-3, "geseran tetap ada");
        assert!(app.model.geometry.get(id).unwrap().shape.volume() < v_r3 - 1.0);
        assert_replays(&app);

        // Undo edit → radius 3 kembali; redo → 6.
        app.undo_model();
        assert!((app.round_op_info(&op_id).unwrap().radius - 3.0).abs() < 1e-9);
        app.redo_model();
        assert!((app.round_op_info(&op_id).unwrap().radius - 6.0).abs() < 1e-9);

        // Radius 0 = hapus fillet.
        assert!(app.begin_round_op_edit(&op_id));
        app.edge_gizmo_radius = 0.0;
        app.commit_round(RoundKind::Edge);
        assert!(app.round_op_info(&op_id).is_none());
        assert_eq!(kinds(&app), ["primitive", "push_pull", "transform"]);
    }

    #[test]
    fn gizmo_vertex_round_becomes_op() {
        let (mut app, id) = app_with_box();
        let ray = PickRay { origin: (30.0, 30.0, 30.0), dir: (-1.0, -1.0, -1.0) };
        app.active_vertex = Some((id, ray, (20.0, 20.0, 20.0)));
        app.vertex_gizmo_radius = 2.0;
        app.commit_round(RoundKind::Vertex);
        assert_eq!(kinds(&app), ["primitive", "round_vertex"]);
        assert_replays(&app);
    }

    /// Edit langsung tanpa op (jalur lama) → dicatat `adopt`, oplog utuh.
    #[test]
    fn unconverted_direct_edit_is_adopted_not_wiped() {
        let (mut app, id) = app_with_box();
        let shape = ducad_kernel::make_box(5.0, 5.0, 5.0, false).unwrap();
        app.execute_model_command(
            Box::new(ReplaceGeometryCommand::new("Legacy", id, BodyGeometry::from_shape(shape))),
            "legacy",
        );
        assert_eq!(kinds(&app), ["primitive", "adopt"]);
        app.sync_agent_meta();
        assert_eq!(app.agent_meta.design.oplog.len(), 2, "oplog tidak boleh dibuang");
        assert_replays(&app);
        // Primitif tertimpa adopt → ada peringatan.
        let first = app.agent_meta.design.oplog[0].id().to_string();
        assert!(app.adopt_after_op(&first).is_some());
        // Undo: geometri dan oplog kembali bersama.
        app.undo_model();
        assert_eq!(kinds(&app), ["primitive"]);
    }

    /// Solver perakitan memindah body: satu op `place`, gerakan beruntun digabung.
    #[test]
    fn assembly_moves_merge_into_one_place_op() {
        let (mut app, id) = app_with_box();
        for _ in 0..3 {
            let tf = ducad_kernel::MateTransformResult {
                translation: (5.0, 0.0, 0.0),
                pivot: (10.0, 10.0, 10.0),
                axis: (0.0, 0.0, 1.0),
                angle_rad: 0.3,
            };
            let geo = app.model.geometry.get(id).unwrap();
            let moved = ducad_kernel::apply_mate_transform_to_shape(&geo.shape, &tf).unwrap();
            app.model.geometry.insert(id, BodyGeometry::from_shape(moved));
            app.note_rigid_move(id, &tf);
        }
        assert_eq!(kinds(&app), ["primitive", "transform"]);
        app.sync_agent_meta();
        assert_eq!(app.agent_meta.design.oplog.len(), 2, "oplog tidak boleh dibuang");
        let design = ducad_engine::DesignDoc {
            fingerprint: String::new(),
            ..app.agent_meta.design.clone()
        };
        let s = ducad_engine::Session::replay(design).unwrap();
        let name = app.model.doc.bodies[id].name.clone();
        let (min, max) = s.body(&name).unwrap().1.shape.bounding_box().unwrap();
        let (lmin, lmax) = bbox(&app, id);
        for k in 0..3 {
            assert!((min[k] - lmin[k]).abs() < 1e-3 && (max[k] - lmax[k]).abs() < 1e-3, "posisi replay beda");
        }
    }

    #[test]
    fn hole_faces_found_for_through_hole() {
        let shape = ducad_kernel::make_box(20.0, 20.0, 10.0, false).unwrap();
        let tool = ducad_kernel::make_cylinder(3.0, 30.0).unwrap();
        let tool = ducad_kernel::translate_shape(&tool, 10.0, 10.0, -10.0).unwrap();
        let holed = ducad_kernel::subtract(&shape, &tool).unwrap();
        let faces = DuCADApp::hole_face_indices(&holed, DVec3::new(10.0, 10.0, 10.0), DVec3::Z);
        assert_eq!(faces.len(), 1, "dinding lubang saja, bukan sisi atas/bawah");
    }
}
