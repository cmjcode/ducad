//! Aksi pemodelan GUI yang dijalankan sebagai op oplog (ADR 0010, Step 1a
//! `docs/PLAN_DESIGNER_UX.md`).
//!
//! Sebelumnya fitur GUI memanggil kernel langsung: hasilnya tidak masuk
//! oplog, tidak tersimpan sebagai riwayat, dan diregenerasi dengan
//! referensi indeks atau `fillet_all` (B0-5). Di sini aksi GUI diterjemahkan
//! ke `Op` dengan selector semantik dari pilihan pengguna, lalu dijalankan
//! lewat `SessionCore` di atas state aplikasi — jalur yang sama dengan
//! agent, termasuk satu langkah undo yang membawa oplog (`oplog_undo`).

use ducad_core::BodyId;
use ducad_engine::model::RenameBodyCommand;
use ducad_engine::ops::Op;
use ducad_engine::select::{suggest_edges, Robustness};
use ducad_engine::{BatchReport, OpError, OpErrorCode, SessionCore};
use ducad_kernel::PickRay;

use crate::app::DuCADApp;
use crate::error_card_ui::GuiFix;

/// Fillet atau chamfer dari GUI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RoundKind {
    Fillet {
        radius: f64,
        radius_end: Option<f64>,
    },
    Chamfer {
        distance: f64,
    },
}

impl DuCADApp {
    /// Id op baru `<prefix><n>` yang belum dipakai oplog maupun nama body.
    pub(crate) fn unique_op_id(&self, prefix: &str) -> String {
        let taken = |id: &str| {
            self.agent_meta.design.oplog.iter().any(|o| o.id() == id)
                || self.model.doc.bodies.values().any(|b| b.name == id)
                || self.agent_meta.sketch_ids.contains_key(id)
        };
        (1..)
            .map(|n| format!("{prefix}{n}"))
            .find(|id| !taken(id))
            .unwrap_or_else(|| prefix.to_string())
    }

    /// Pastikan nama body `id` unik (op merujuk body lewat nama). Nama
    /// kembar diganti `"<nama> <n>"`; command rename yang SUDAH diterapkan
    /// dikembalikan agar pemanggil menitipkannya ke transaksi op.
    fn make_body_name_unique(&mut self, id: BodyId) -> Option<Box<RenameBodyCommand>> {
        let name = self.model.doc.bodies.get(id)?.name.clone();
        let clash = self
            .model
            .doc
            .bodies
            .iter()
            .any(|(other, b)| other != id && b.name == name);
        if !clash {
            return None;
        }
        let taken = |n: &str| self.model.doc.bodies.values().any(|b| b.name == n);
        let fresh = (2..).map(|k| format!("{name} {k}")).find(|n| !taken(n))?;
        let mut cmd = Box::new(RenameBodyCommand::new(id, fresh));
        ducad_core::Command::apply(cmd.as_mut(), &mut self.model);
        Some(cmd)
    }

    /// Jalankan `ops` sebagai satu aksi GUI: satu langkah undo (geometri +
    /// oplog), aktivitas tercatat. Gagal → model utuh, `Err` dengan error op.
    #[allow(clippy::result_large_err)]
    pub(crate) fn run_gui_ops(
        &mut self,
        ops: Vec<Op>,
        title: &str,
        details: &str,
    ) -> Result<BatchReport, OpError> {
        self.sync_agent_meta();
        let mark = self.begin_oplog_step();
        let report = {
            let mut core = SessionCore {
                model: &mut self.model,
                model_undo: &mut self.model_undo,
                sketches: &mut self.sketch_set,
                meta: &mut self.agent_meta,
            };
            core.run(ops, false)
        };
        if !report.committed {
            let e = report
                .error
                .clone()
                .unwrap_or_else(|| OpError::new(OpErrorCode::Internal, "op tidak ter-commit"));
            return Err(e);
        }
        self.end_oplog_step(mark, title);
        self.sync_design_after_agent();
        self.clear_redo_except(crate::types::UndoTarget::Model);
        self.onboarding.note_model_command(title);
        self.record_activity(ducad_ui::ActivityKindUi::Solid3D, title, details);
        Ok(report)
    }

    /// Fillet/chamfer tepi terpilih (atau semua tepi bila `rays` kosong) pada
    /// body `id` sebagai op oplog. Tepi yang diklik menjadi selector
    /// semantik; `Ok(robustness)` memberi tahu seberapa tahan selector itu.
    #[allow(clippy::result_large_err)]
    pub(crate) fn round_via_oplog(
        &mut self,
        id: BodyId,
        kind: RoundKind,
        rays: &[PickRay],
        tolerance: f64,
    ) -> Result<Robustness, OpError> {
        let rename = self.make_body_name_unique(id);
        let result = self.round_via_oplog_inner(id, kind, rays, tolerance);
        if let Some(mut cmd) = rename {
            if result.is_ok() {
                // Rename ikut langkah undo yang sama dengan op-nya.
                self.model_undo.attach_to_top(cmd);
            } else {
                ducad_core::Command::revert(cmd.as_mut(), &mut self.model);
            }
        }
        result
    }

    #[allow(clippy::result_large_err)]
    fn round_via_oplog_inner(
        &mut self,
        id: BodyId,
        kind: RoundKind,
        rays: &[PickRay],
        tolerance: f64,
    ) -> Result<Robustness, OpError> {
        let body = self
            .model
            .doc
            .bodies
            .get(id)
            .map(|b| b.name.clone())
            .ok_or_else(|| OpError::invalid("body terpilih tidak ada lagi"))?;
        let shape = &self
            .model
            .geometry
            .get(id)
            .ok_or_else(|| OpError::invalid("body terpilih tidak punya geometri"))?
            .shape;
        let (edges, robustness) = if rays.is_empty() {
            ("all".to_string(), Robustness::Semantic)
        } else {
            let picked = ducad_kernel::edges_for_rays(shape, rays, tolerance)
                .map_err(|e| OpError::invalid(format!("{e:#}")))?;
            let sug = suggest_edges(shape, &picked)
                .ok_or_else(|| OpError::invalid("tepi terpilih tidak ditemukan pada body"))?;
            (sug.selector, sug.robustness)
        };
        let (op, title, details) = match kind {
            RoundKind::Fillet { radius, radius_end } => {
                let id = self.unique_op_id("fillet");
                let mut v = serde_json::json!({
                    "op": "fillet", "id": id, "body": body, "edges": edges, "radius": radius
                });
                if let Some(r) = radius_end {
                    v["radius_end"] = serde_json::json!(r);
                }
                (
                    v,
                    "Fillet",
                    format!("Fillet R{radius:.1} pada {body} ({edges})"),
                )
            }
            RoundKind::Chamfer { distance } => {
                let id = self.unique_op_id("chamfer");
                (
                    serde_json::json!({
                        "op": "chamfer", "id": id, "body": body, "edges": edges, "distance": distance
                    }),
                    "Chamfer",
                    format!("Chamfer {distance:.1} mm pada {body} ({edges})"),
                )
            }
        };
        let op: Op = serde_json::from_value(op)
            .map_err(|e| OpError::invalid(format!("op GUI tidak valid: {e}")))?;
        self.run_gui_ops(vec![op], title, &details)?;
        Ok(robustness)
    }
}

/// Nama bidang op untuk bidang sketsa GUI; `None` = belum didukung jalur
/// oplog (datum bersudut/3 titik, face) → jalur lama.
fn plane_spec(plane: ducad_sketch::PlaneRef) -> Option<&'static str> {
    match plane {
        ducad_sketch::PlaneRef::Top => Some("XY"),
        ducad_sketch::PlaneRef::Front => Some("XZ"),
        ducad_sketch::PlaneRef::Right => Some("YZ"),
        ducad_sketch::PlaneRef::Datum(_) => None,
    }
}

impl DuCADApp {
    /// Op `sketch` dari isi slot GUI `sid` (spec bernama, ADR 0010).
    fn sketch_op_for_slot(&self, sid: ducad_sketch::SketchId, id: &str) -> Option<Op> {
        let slot = self.sketch_set.get(sid)?;
        let plane = plane_spec(slot.plane)?;
        let specs = ducad_engine::ops::from_sketch::sketch_to_specs(&slot.sketch);
        serde_json::from_value(serde_json::json!({
            "op": "sketch",
            "id": id,
            "plane": plane,
            "entities": specs.entities,
            "constraints": specs.constraints,
        }))
        .ok()
    }

    /// Id op `sketch` yang terikat ke slot `sid` dan masih ada di oplog.
    fn bound_sketch_id(&self, sid: ducad_sketch::SketchId) -> Option<String> {
        self.agent_meta
            .live_sketches
            .iter()
            .find(|(id, s)| {
                **s == sid
                    && self
                        .agent_meta
                        .design
                        .oplog
                        .iter()
                        .any(|o| o.id() == id.as_str())
            })
            .map(|(id, _)| id.clone())
    }

    /// Edit oplog (ganti/tambah op) dengan replay penuh di sesi salinan, lalu
    /// adopsi sebagai SATU langkah undo GUI. Gagal → model utuh.
    #[allow(clippy::result_large_err)]
    pub(crate) fn edit_oplog_via_replay(
        &mut self,
        replace: Vec<ducad_engine::ReplaceOp>,
        append: Vec<Op>,
        what: &str,
    ) -> Result<(), OpError> {
        // Replay langsung tanpa diff geometri: `preview_edit` juga menghitung
        // boolean body lama vs baru untuk ghost proposal, yang tidak perlu di
        // sini dan bisa sangat lambat pada body berfillet + berlubang.
        let design =
            ducad_engine::edit_design(&self.agent_meta.design, None, &replace, &[], &append)?;
        let session = ducad_engine::Session::replay(ducad_engine::DesignDoc {
            fingerprint: String::new(),
            ..design
        })?;
        let meta = session.meta().clone();
        self.adopt_agent_model(session.into_model(), meta, what);
        self.clear_redo_except(crate::types::UndoTarget::Model);
        Ok(())
    }

    /// Extrude entitas terpilih di sketsa aktif sebagai op oplog. `Ok(false)`
    /// = kasus yang belum didukung jalur oplog (pemanggil memakai jalur
    /// lama). Sketsa pertama kali di-extrude menjadi op `sketch` yang terikat
    /// ke slot GUI; sketsa terikat yang isinya sudah berubah diganti lebih
    /// dulu (replay penuh), sehingga body hilir ikut diperbarui.
    #[allow(clippy::result_large_err)]
    pub(crate) fn extrude_via_oplog(
        &mut self,
        distance: f64,
        cut_target: Option<BodyId>,
    ) -> Result<bool, OpError> {
        // Slot aktif GUI diturunkan dari bidang aktif (bukan
        // `sketch_set.active_id()`), sama seperti `DuCADApp::sketch()`.
        let sid = self.active_sketch_id();
        let Some(slot) = self.sketch_set.get(sid) else {
            return Ok(false);
        };
        if plane_spec(slot.plane).is_none() || self.selected.is_empty() || distance == 0.0 {
            return Ok(false);
        }
        let names: Vec<String> = self
            .selected
            .iter()
            .filter(|id| slot.sketch.entities.contains_key(**id))
            .map(|id| ducad_engine::ops::from_sketch::entity_spec_name(&slot.sketch, *id))
            .collect();
        if names.is_empty() {
            return Ok(false);
        }
        // Samakan oplog dengan geometri lebih dulu: adopsi mengosongkan
        // ikatan sketsa yang sudah tidak berlaku.
        self.sync_agent_meta();
        let rename = cut_target.and_then(|t| self.make_body_name_unique(t));
        let result = self.extrude_via_oplog_inner(sid, names, distance, cut_target);
        if let Some(mut cmd) = rename {
            if result.is_ok() {
                self.model_undo.attach_to_top(cmd);
            } else {
                ducad_core::Command::revert(cmd.as_mut(), &mut self.model);
            }
        }
        result.map(|()| true)
    }

    #[allow(clippy::result_large_err)]
    fn extrude_via_oplog_inner(
        &mut self,
        sid: ducad_sketch::SketchId,
        names: Vec<String>,
        distance: f64,
        cut_target: Option<BodyId>,
    ) -> Result<(), OpError> {
        let bound = self.bound_sketch_id(sid);
        let sketch_id = bound.clone().unwrap_or_else(|| self.unique_op_id("sketch"));
        let sketch_op = self
            .sketch_op_for_slot(sid, &sketch_id)
            .ok_or_else(|| OpError::invalid("sketsa aktif tidak bisa diterjemahkan ke op"))?;
        let extrude_id = self.unique_op_id("extrude");
        let mut ext = serde_json::json!({
            "op": "extrude",
            "id": extrude_id,
            "sketch": sketch_id,
            "profile": { "names": names },
            "distance": distance.abs(),
        });
        if distance < 0.0 {
            ext["direction"] = serde_json::json!("reverse");
        }
        if let Some(t) = cut_target {
            let target = self
                .model
                .doc
                .bodies
                .get(t)
                .map(|b| b.name.clone())
                .ok_or_else(|| OpError::invalid("body sasaran cut tidak ada lagi"))?;
            ext["mode"] = serde_json::json!("cut");
            ext["target"] = serde_json::json!(target);
        }
        let extrude: Op = serde_json::from_value(ext)
            .map_err(|e| OpError::invalid(format!("op extrude GUI tidak valid: {e}")))?;
        let details = format!("Extrude {:.1} mm dari {sketch_id}", distance.abs());

        match bound {
            None => {
                self.agent_meta.live_sketches.insert(sketch_id.clone(), sid);
                let r = self.run_gui_ops(vec![sketch_op, extrude], "Extrude", &details);
                if r.is_err() {
                    self.agent_meta.live_sketches.remove(&sketch_id);
                }
                r.map(|_| ())
            }
            Some(id) => {
                let current = self.agent_meta.design.oplog.iter().find(|o| o.id() == id);
                if current == Some(&sketch_op) {
                    self.run_gui_ops(vec![extrude], "Extrude", &details)
                        .map(|_| ())
                } else {
                    let replace = vec![ducad_engine::ReplaceOp { id, op: sketch_op }];
                    self.edit_oplog_via_replay(replace, vec![extrude], "Extrude")
                }
            }
        }
    }

    /// Sketsa terikat yang isinya berubah sejak op-nya dicatat → ganti op
    /// `sketch` dan replay, agar body hilir mengikuti (dipanggil saat keluar
    /// dari mode sketsa). Gagal → oplog lama tetap, error ditampilkan.
    pub(crate) fn sync_bound_sketches(&mut self) {
        let bound: Vec<(String, ducad_sketch::SketchId)> = self
            .agent_meta
            .live_sketches
            .iter()
            .map(|(id, sid)| (id.clone(), *sid))
            .collect();
        let mut replace = Vec::new();
        for (id, sid) in bound {
            let Some(current) = self.agent_meta.design.oplog.iter().find(|o| o.id() == id) else {
                continue;
            };
            let Some(op) = self.sketch_op_for_slot(sid, &id) else {
                continue;
            };
            if *current != op {
                replace.push(ducad_engine::ReplaceOp { id, op });
            }
        }
        if replace.is_empty() {
            return;
        }
        if let Err(e) = self.edit_oplog_via_replay(replace, Vec::new(), "Perbarui sketsa") {
            self.model_status = Some(crate::modeling::operations::op_status(
                "Perbarui sketsa",
                &e,
            ));
            let fixes = self.fixes_for_op_error(&e);
            self.show_op_error(&e, fixes);
        }
    }
}

impl DuCADApp {
    /// Baris Feature Tree dari oplog dokumen (Step 2).
    pub fn oplog_row_infos(&self) -> Vec<ducad_ui::OplogRowInfo> {
        ducad_engine::oplog_view::oplog_rows(&self.agent_meta.design)
            .into_iter()
            .map(|r| ducad_ui::OplogRowInfo {
                id: r.id,
                kind: r.kind,
                summary: r.summary,
                value: r.main_value,
                suppressed: r.suppressed,
                depends_on: r.depends_on,
                dependents: r.dependents,
            })
            .collect()
    }

    /// Tampilkan error op dari aksi Feature Tree (status + kartu error).
    fn report_tree_error(&mut self, label: &str, e: &OpError) {
        self.model_status = Some(crate::modeling::operations::op_status(label, e));
        let fixes = self.fixes_for_op_error(e);
        self.show_op_error(e, fixes);
    }

    /// Ganti nilai utama op `id` lalu replay (satu langkah undo).
    pub fn set_op_value(&mut self, id: &str, key: &str, value: f64) {
        let Some(op) = self.agent_meta.design.oplog.iter().find(|o| o.id() == id) else {
            return;
        };
        let Some(patched) = ducad_engine::oplog_view::with_value(op, key, value) else {
            return;
        };
        let replace = vec![ducad_engine::ReplaceOp {
            id: id.to_string(),
            op: patched,
        }];
        if let Err(e) = self.edit_oplog_via_replay(replace, Vec::new(), "Ubah fitur") {
            self.report_tree_error("Ubah fitur", &e);
        }
    }

    /// Aktifkan/nonaktifkan op `id` lalu replay (satu langkah undo). Op yang
    /// masih dipakai op lain gagal dengan error yang menyebut op hilirnya.
    pub fn toggle_op_suppressed(&mut self, id: &str) {
        let mut design = self.agent_meta.design.clone();
        if let Some(pos) = design.suppressed.iter().position(|s| s == id) {
            design.suppressed.remove(pos);
        } else {
            design.suppressed.push(id.to_string());
        }
        let result = ducad_engine::Session::replay(ducad_engine::DesignDoc {
            fingerprint: String::new(),
            ..design
        });
        match result {
            Ok(session) => {
                let meta = session.meta().clone();
                self.adopt_agent_model(session.into_model(), meta, "Suppress fitur");
                self.clear_redo_except(crate::types::UndoTarget::Model);
            }
            Err(e) => self.report_tree_error("Suppress fitur", &e),
        }
    }

    /// Hapus op `id` lalu replay (satu langkah undo).
    #[allow(clippy::result_large_err)]
    pub fn delete_op(&mut self, id: &str) {
        let result =
            ducad_engine::edit_design(&self.agent_meta.design, None, &[], &[id.to_string()], &[])
                .and_then(|design| {
                    ducad_engine::Session::replay(ducad_engine::DesignDoc {
                        fingerprint: String::new(),
                        ..design
                    })
                });
        match result {
            Ok(session) => {
                let meta = session.meta().clone();
                self.adopt_agent_model(session.into_model(), meta, "Hapus fitur");
                self.clear_redo_except(crate::types::UndoTarget::Model);
            }
            Err(e) => self.report_tree_error("Hapus fitur", &e),
        }
    }
}

impl DuCADApp {
    /// Setelah membuka berkas: replay `design.oplog` dan pasang hasilnya
    /// sebagai oplog live bila geometrinya sama dengan body di berkas; op
    /// `sketch` diikat kembali ke slot GUI yang isinya identik. Bila replay
    /// gagal atau geometri berbeda, perilaku lama berlaku (adopsi).
    pub(crate) fn restore_oplog_from_design(&mut self) {
        let Some(design) = self
            .design
            .as_ref()
            .and_then(|v| serde_json::from_value::<ducad_engine::DesignDoc>(v.clone()).ok())
        else {
            return;
        };
        if design.oplog.is_empty() {
            return;
        }
        let Ok(session) = ducad_engine::Session::replay(ducad_engine::DesignDoc {
            fingerprint: String::new(),
            ..design
        }) else {
            return;
        };
        // Body dari berkas bisa berasal dari cache B-rep: bandingkan dengan
        // toleransi (sama dengan `Session::replay`), bukan sidik jari hash.
        let current = ducad_engine::session::fingerprint(&self.model);
        if !ducad_engine::session::same_geometry(
            &ducad_engine::session::body_summary(session.model()),
            &ducad_engine::session::body_summary(&self.model),
        ) {
            log::warn!("oplog berkas tidak menghasilkan geometri yang sama; riwayat diadopsi");
            return;
        }
        let mut meta = session.meta().clone();
        let sketch_ops: Vec<(String, Op)> = meta
            .design
            .oplog
            .iter()
            .filter(|o| o.kind() == "sketch")
            .map(|o| (o.id().to_string(), o.clone()))
            .collect();
        let slots: Vec<ducad_sketch::SketchId> = self.sketch_set.ids().to_vec();
        for (id, op) in sketch_ops {
            let bound = slots
                .iter()
                .copied()
                .filter(|sid| !meta.live_sketches.values().any(|s| s == sid))
                .find(|sid| self.sketch_op_for_slot(*sid, &id).as_ref() == Some(&op));
            if let Some(sid) = bound {
                meta.sketch_ids.insert(id.clone(), sid);
                meta.live_sketches.insert(id, sid);
            }
        }
        meta.design.fingerprint = current;
        meta.sim_results = std::mem::take(&mut self.agent_meta.sim_results);
        meta.analysis_results = std::mem::take(&mut self.agent_meta.analysis_results);
        self.agent_meta = meta;
        self.sync_design_after_agent();
    }
}

/// Spec op lubang (`{"iso":..}` / `{"diameter":..}`) yang menghasilkan
/// `HoleSpec` sama persis dengan pilihan wizard; `None` = wizard memakai
/// ukuran yang tidak bisa dinyatakan op (mis. counterbore custom).
pub(crate) fn hole_spec_json(spec: &ducad_core::hole::HoleSpec) -> Option<serde_json::Value> {
    use ducad_core::hole::{HoleKind, IsoMetricThread};
    let depth = (!spec.is_through).then_some(spec.depth);
    let mut cands = Vec::new();
    if spec.thread_size != IsoMetricThread::Custom {
        let kind = match spec.kind {
            HoleKind::Simple => "clearance",
            HoleKind::Tapped => "tapped",
            HoleKind::Counterbore => "counterbore",
            HoleKind::Countersink => "countersink",
        };
        let mut v = serde_json::json!({ "iso": spec.thread_size.label(), "kind": kind });
        if let Some(d) = depth {
            v["depth"] = serde_json::json!(d);
        }
        cands.push(v);
    }
    if spec.kind == HoleKind::Simple {
        let mut v = serde_json::json!({ "diameter": spec.diameter });
        if let Some(d) = depth {
            v["depth"] = serde_json::json!(d);
        }
        cands.push(v);
    }
    let same = |a: &ducad_core::hole::HoleSpec| {
        let near = |x: f64, y: f64| (x - y).abs() < 1e-9;
        a.kind == spec.kind
            && a.is_through == spec.is_through
            && (spec.is_through || near(a.depth, spec.depth))
            && near(a.diameter, spec.diameter)
            && near(a.counterbore_diameter, spec.counterbore_diameter)
            && near(a.counterbore_depth, spec.counterbore_depth)
            && near(a.countersink_diameter, spec.countersink_diameter)
            && near(a.countersink_angle_deg, spec.countersink_angle_deg)
    };
    cands.into_iter().find(|v| {
        serde_json::from_value::<ducad_engine::ops::HoleSpecRef>(v.clone())
            .ok()
            .and_then(|r| {
                ducad_engine::session::hole_spec(&r, &ducad_engine::ops::Params::new()).ok()
            })
            .is_some_and(|built| same(&built))
    })
}

impl DuCADApp {
    /// Lubang wizard sebagai op `hole` (face → selector semantik, posisi →
    /// `at_world`). `edit = true` mengganti op lubang terdekat pada body yang
    /// sama. `Ok(false)` = belum didukung jalur oplog → jalur lama.
    #[allow(clippy::result_large_err)]
    pub(crate) fn hole_via_oplog(
        &mut self,
        body_id: BodyId,
        ray: PickRay,
        pos: (f64, f64, f64),
        spec: &ducad_core::hole::HoleSpec,
        edit: bool,
    ) -> Result<bool, OpError> {
        let Some(spec_json) = hole_spec_json(spec) else {
            return Ok(false);
        };
        if edit
            && self
                .hole_history
                .get(&body_id)
                .is_some_and(|h| !h.features.is_empty())
        {
            // Lubang lama (jalur kernel langsung) diedit di jalur lama.
            return Ok(false);
        }
        let Some(geo) = self.model.geometry.get(body_id) else {
            return Ok(false);
        };
        let Some(face) = ducad_kernel::face_index_for_ray(&geo.shape, ray) else {
            return Ok(false);
        };
        let Some(sug) = ducad_engine::select::suggest_faces(&geo.shape, &[face]) else {
            return Ok(false);
        };
        self.sync_agent_meta();
        let rename = self.make_body_name_unique(body_id);
        let body = self.model.doc.bodies[body_id].name.clone();
        let at = [pos.0, pos.1, pos.2];
        let target = edit.then(|| self.nearest_hole_op(&body, at)).flatten();
        let id = target.clone().unwrap_or_else(|| self.unique_op_id("hole"));
        let op: Op = serde_json::from_value(serde_json::json!({
            "op": "hole", "id": id, "body": body, "face": sug.selector,
            "at_world": [at], "spec": spec_json,
        }))
        .map_err(|e| OpError::invalid(format!("op hole GUI tidak valid: {e}")))?;
        let details = format!("Hole {} pada {body}", spec.technical_callout());
        let result = match target {
            Some(id) => self.edit_oplog_via_replay(
                vec![ducad_engine::ReplaceOp { id, op }],
                Vec::new(),
                "Edit Hole",
            ),
            None => self
                .run_gui_ops(vec![op], "Hole Wizard", &details)
                .map(|_| ()),
        };
        if let Some(mut cmd) = rename {
            if result.is_ok() {
                self.model_undo.attach_to_top(cmd);
            } else {
                ducad_core::Command::revert(cmd.as_mut(), &mut self.model);
            }
        }
        result.map(|()| true)
    }

    /// Op `hole` pada `body` dengan satu titik `at_world` literal terdekat ke
    /// `at`.
    fn nearest_hole_op(&self, body: &str, at: [f64; 3]) -> Option<String> {
        self.agent_meta
            .design
            .oplog
            .iter()
            .filter_map(|o| {
                let v = serde_json::to_value(o).ok()?;
                if v["op"] != "hole" || v["body"] != body {
                    return None;
                }
                let p = v["at_world"].as_array()?.first()?.as_array()?;
                let p: Vec<f64> = p.iter().filter_map(|c| c.as_f64()).collect();
                (p.len() == 3).then(|| {
                    let d =
                        (p[0] - at[0]).powi(2) + (p[1] - at[1]).powi(2) + (p[2] - at[2]).powi(2);
                    (d, o.id().to_string())
                })
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id)
    }
}

impl DuCADApp {
    /// Fix terverifikasi dari error op → tombol kartu error, termasuk
    /// "Tutup celah" untuk profil yang hampir tertutup (dipetakan ke entitas
    /// sketsa aktif lewat nama spec-nya).
    pub(crate) fn fixes_for_op_error(&self, e: &OpError) -> Vec<(String, GuiFix)> {
        let mut out = gui_fixes_from_op_error(e);
        if e.code == OpErrorCode::ProfileOpenGap {
            let ctx = &e.context;
            let name = ctx["b"]["entity"].as_str();
            let at_end = ctx["b"]["end"].as_str() == Some("end");
            let to = ctx["a"]["point"]
                .as_array()
                .and_then(|p| Some([p.first()?.as_f64()?, p.get(1)?.as_f64()?]));
            let sketch = self.sketch();
            let entity = name.and_then(|n| {
                sketch
                    .entities
                    .keys()
                    .find(|id| ducad_engine::ops::from_sketch::entity_spec_name(sketch, *id) == n)
            });
            if let (Some(entity), Some(to)) = (entity, to) {
                let gap = ctx["distance"].as_f64().unwrap_or(0.0);
                out.push((
                    ducad_i18n::t!("fix-close-gap-mm", value = format!("{gap:.3}")),
                    GuiFix::CloseSketchGap { entity, at_end, to },
                ));
            }
        }
        out
    }
}

/// Fix terverifikasi dari error op → tombol kartu error GUI.
pub(crate) fn gui_fixes_from_op_error(e: &OpError) -> Vec<(String, GuiFix)> {
    e.fixes
        .iter()
        .filter(|f| f.verified)
        .filter_map(|f| {
            let num = |key: &str| f.patch.set.get(key).and_then(|v| v.as_f64());
            let fix = if let Some(r) = num("/radius") {
                GuiFix::FilletRadius(r)
            } else if let Some(d) = num("/distance") {
                GuiFix::ChamferDistance(d)
            } else {
                return None;
            };
            Some((f.label.clone(), fix))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_engine::compute;

    fn app_with_box(name: &str, at: [f64; 3]) -> (DuCADApp, BodyId) {
        let mut app = DuCADApp::new_for_test();
        let id = add_box(&mut app, name, at);
        (app, id)
    }

    fn add_box(app: &mut DuCADApp, name: &str, at: [f64; 3]) -> BodyId {
        let geo = compute::primitive(
            &compute::PrimitiveShape::Box {
                size: [20.0, 20.0, 20.0],
                centered: false,
            },
            at,
        )
        .unwrap();
        let id = app.model.doc.add_body(name);
        app.model.geometry.insert(id, geo);
        id
    }

    /// Ray tegak ke bawah tepat di atas tepi atas kubus pada x=20, y=10.
    fn ray_top_edge_x20() -> PickRay {
        PickRay {
            origin: (20.0, 10.0, 50.0),
            dir: (0.0, 0.0, -1.0),
        }
    }

    #[test]
    fn gui_fillet_becomes_oplog_op_with_semantic_selector() {
        let (mut app, id) = app_with_box("Box", [0.0; 3]);
        let v0 = app.model.geometry[id].shape.volume();
        let rob = app
            .round_via_oplog(
                id,
                RoundKind::Fillet {
                    radius: 2.0,
                    radius_end: None,
                },
                &[ray_top_edge_x20()],
                1.0,
            )
            .unwrap();
        assert_eq!(rob, Robustness::Semantic);
        let op = app.agent_meta.design.oplog.last().expect("op masuk oplog");
        let v = serde_json::to_value(op).unwrap();
        assert_eq!(v["op"], "fillet");
        assert_eq!(v["body"], "Box");
        let sel = v["edges"].as_str().unwrap().to_string();
        assert!(!sel.starts_with("idx:"), "{sel}");
        assert!(app.model.geometry[id].shape.volume() < v0);

        // Satu langkah undo memulihkan geometri dan oplog.
        app.undo_model();
        assert!((app.model.geometry[id].shape.volume() - v0).abs() < 1e-6);
        assert!(app.agent_meta.design.oplog.is_empty());
    }

    #[test]
    fn gui_fillet_survives_replay_from_saved_design() {
        let (mut app, id) = app_with_box("Box", [0.0; 3]);
        app.round_via_oplog(
            id,
            RoundKind::Fillet {
                radius: 2.0,
                radius_end: None,
            },
            &[ray_top_edge_x20()],
            1.0,
        )
        .unwrap();
        let v_gui = app.model.geometry[id].shape.volume();
        let design = app.agent_meta.design.clone();
        let s = ducad_engine::Session::replay(design).expect("design GUI bisa di-replay");
        let v_replay = s.body("Box").unwrap().1.shape.volume();
        assert!((v_gui - v_replay).abs() < 1e-6, "{v_gui} vs {v_replay}");
    }

    #[test]
    fn duplicate_body_names_are_made_unique_in_the_same_undo_step() {
        let (mut app, a) = app_with_box("Extrude", [0.0; 3]);
        let b = add_box(&mut app, "Extrude", [50.0, 0.0, 0.0]);
        let steps = app.model_undo.undo_count();
        app.round_via_oplog(b, RoundKind::Chamfer { distance: 1.0 }, &[], 1.0)
            .unwrap();
        assert_ne!(app.model.doc.bodies[a].name, app.model.doc.bodies[b].name);
        let v = serde_json::to_value(app.agent_meta.design.oplog.last().unwrap()).unwrap();
        assert_eq!(v["body"], app.model.doc.bodies[b].name.as_str());
        assert_eq!(
            app.model_undo.undo_count(),
            steps + 1,
            "rename + chamfer = 1 langkah"
        );
        app.undo_model();
        assert_eq!(
            app.model.doc.bodies[b].name, "Extrude",
            "undo memulihkan nama"
        );
    }

    use glam::DVec2;

    /// Profil L di bidang Front tanpa constraint (alas `base`, tinggi 35,
    /// tebal 5); mengembalikan id entitas berurutan.
    fn draw_l_profile(app: &mut DuCADApp, base: f64) -> Vec<ducad_sketch::entity::EntityId> {
        app.set_sketch_plane(ducad_render::PlaneKind::Front);
        app.enter_sketching();
        let pts = [
            DVec2::new(0.0, 0.0),
            DVec2::new(base, 0.0),
            DVec2::new(base, 5.0),
            DVec2::new(5.0, 5.0),
            DVec2::new(5.0, 35.0),
            DVec2::new(0.0, 35.0),
        ];
        let sk = app.sketch_mut();
        (0..6)
            .map(|i| {
                sk.entities
                    .insert(ducad_sketch::Entity::line(pts[i], pts[(i + 1) % 6]))
            })
            .collect()
    }

    fn set_base(app: &mut DuCADApp, ids: &[ducad_sketch::entity::EntityId], base: f64) {
        use ducad_sketch::Entity;
        let sk = app.sketch_mut();
        if let Some(Entity::Line { end, .. }) = sk.entities.get_mut(ids[0]) {
            end.x = base;
        }
        if let Some(Entity::Line { start, end, .. }) = sk.entities.get_mut(ids[1]) {
            start.x = base;
            end.x = base;
        }
        if let Some(Entity::Line { start, .. }) = sk.entities.get_mut(ids[2]) {
            start.x = base;
        }
    }

    fn only_body(app: &DuCADApp) -> BodyId {
        assert_eq!(app.model.doc.bodies.len(), 1, "harus satu body");
        app.model.doc.bodies.keys().next().unwrap()
    }

    fn volume(app: &DuCADApp) -> f64 {
        app.model.geometry[only_body(app)].shape.volume().abs()
    }

    /// Volume L + fillet sudut dalam r, panjang 60.
    fn l_volume(base: f64, r: f64) -> f64 {
        (base * 5.0 + 30.0 * 5.0) * 60.0 + (1.0 - std::f64::consts::FRAC_PI_4) * r * r * 60.0
    }

    /// Toleransi relatif yang sama dengan tes proyek contoh engine.
    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() / b < 1e-4
    }

    fn oplog_kinds(app: &DuCADApp) -> Vec<String> {
        app.agent_meta
            .design
            .oplog
            .iter()
            .map(|o| o.kind().to_string())
            .collect()
    }

    /// T1–T2 lewat GUI: sketsa → extrude → fillet sudut dalam → ubah alas
    /// di sketsa → keluar mode sketsa → body dan fillet mengikuti.
    #[test]
    fn gui_sketch_extrude_fillet_then_edit_sketch_updates_downstream() {
        let mut app = DuCADApp::new_for_test();
        let ids = draw_l_profile(&mut app, 40.0);
        app.selected = ids.iter().copied().collect();
        app.extrude_distance_input = "60".into();
        app.extrude_selected();
        assert!(app.model_status.is_none(), "{:?}", app.model_status);
        assert_eq!(oplog_kinds(&app), ["sketch", "extrude"]);
        assert_eq!(app.agent_meta.live_sketches.len(), 1, "slot GUI terikat");
        assert_eq!(
            app.sketch_set.len(),
            app.sketch_set.ids().len(),
            "tidak ada slot ganda"
        );
        let slots_after_extrude = app.sketch_set.len();
        assert!(close(volume(&app), l_volume(40.0, 0.0)));

        // Fillet sudut dalam lewat ray miring tepat ke tepi (5, y, 5).
        let id = only_body(&app);
        let s = std::f64::consts::FRAC_1_SQRT_2;
        let ray = PickRay {
            origin: (20.0, -30.0, 20.0),
            dir: (-s, 0.0, -s),
        };
        let rob = app
            .round_via_oplog(
                id,
                RoundKind::Fillet {
                    radius: 3.0,
                    radius_end: None,
                },
                &[ray],
                1.0,
            )
            .unwrap();
        assert_eq!(rob, Robustness::Semantic);
        assert_eq!(oplog_kinds(&app), ["sketch", "extrude", "fillet"]);
        let fillet_json =
            serde_json::to_value(app.agent_meta.design.oplog.last().unwrap()).unwrap();
        assert!(
            close(volume(&app), l_volume(40.0, 3.0)),
            "volume {} vs {} ({fillet_json})",
            volume(&app),
            l_volume(40.0, 3.0)
        );

        // Lubang M5 tembus di alas lewat hole wizard (ray tegak ke bawah).
        let mut spec = ducad_core::hole::HoleSpec::for_iso(
            ducad_core::hole::IsoMetricThread::M5,
            ducad_core::hole::HoleKind::Simple,
            20.0,
        );
        spec.is_through = true;
        let down = PickRay {
            origin: (22.5, -15.0, 50.0),
            dir: (0.0, 0.0, -1.0),
        };
        assert!(app
            .hole_via_oplog(id, down, (22.5, -15.0, 5.0), &spec, false)
            .unwrap());
        assert_eq!(oplog_kinds(&app), ["sketch", "extrude", "fillet", "hole"]);
        let hole = std::f64::consts::PI * 2.75 * 2.75 * 5.0;
        assert!(close(volume(&app), l_volume(40.0, 3.0) - hole));

        // T2: ubah alas 40 → 55 di sketsa lalu keluar dari mode sketsa.
        app.enter_sketching();
        set_base(&mut app, &ids, 55.0);
        app.exit_sketching();
        assert!(!app.error_card.open, "{:?}", app.error_card);
        assert_eq!(oplog_kinds(&app), ["sketch", "extrude", "fillet", "hole"]);
        assert!(
            close(volume(&app), l_volume(55.0, 3.0) - hole),
            "fillet & extrude mengikuti sketsa: {}",
            volume(&app)
        );
        assert_eq!(
            app.sketch_set.len(),
            slots_after_extrude,
            "tidak ada slot ganda"
        );
        app.sync_agent_meta();
        assert!(!app.agent_meta.warnings.iter().any(|w| w == "oplog_stale"));

        // Design tersimpan bisa di-replay headless dengan hasil sama.
        let s = ducad_engine::Session::replay(app.agent_meta.design.clone()).unwrap();
        let v_replay: f64 = s.summary().bodies.iter().map(|b| b.volume).sum();
        assert!(close(v_replay, volume(&app)));

        // Undo edit sketsa → model kembali ke alas 40.
        app.undo_model();
        assert!(close(volume(&app), l_volume(40.0, 3.0) - hole));
    }

    #[test]
    fn hole_spec_json_maps_iso_and_custom_and_rejects_custom_counterbore() {
        use ducad_core::hole::{HoleKind, HoleSpec, IsoMetricThread};
        let mut iso = HoleSpec::for_iso(IsoMetricThread::M6, HoleKind::Counterbore, 12.0);
        iso.is_through = false;
        let v = hole_spec_json(&iso).expect("ISO counterbore bisa dinyatakan op");
        assert_eq!(v["iso"], "M6");
        assert_eq!(v["kind"], "counterbore");
        assert_eq!(v["depth"], 12.0);

        let mut custom = HoleSpec::for_iso(IsoMetricThread::Custom, HoleKind::Simple, 7.0);
        custom.diameter = 4.2;
        custom.is_through = false;
        assert_eq!(hole_spec_json(&custom).unwrap()["diameter"], 4.2);

        let mut odd = iso;
        odd.counterbore_diameter += 1.5;
        assert!(
            hole_spec_json(&odd).is_none(),
            "counterbore custom → jalur lama"
        );
    }

    #[test]
    fn open_profile_offers_close_gap_fix_that_extrudes() {
        let mut app = DuCADApp::new_for_test();
        app.set_sketch_plane(ducad_render::PlaneKind::Top);
        app.enter_sketching();
        let pts = [
            DVec2::new(0.0, 0.0),
            DVec2::new(40.0, 0.0),
            DVec2::new(40.0, 30.0),
            DVec2::new(0.0, 30.0),
            DVec2::new(0.0, 0.3),
        ];
        let ids: Vec<_> = (0..4)
            .map(|i| {
                app.sketch_mut()
                    .entities
                    .insert(ducad_sketch::Entity::line(pts[i], pts[i + 1]))
            })
            .collect();
        app.selected = ids.iter().copied().collect();
        app.extrude_distance_input = "5".into();
        app.extrude_selected();
        assert!(app.error_card.open, "profil terbuka harus gagal");
        assert!(app.model.doc.bodies.is_empty());
        let i = app
            .error_fixes
            .iter()
            .position(|f| matches!(f, GuiFix::CloseSketchGap { .. }))
            .unwrap_or_else(|| panic!("tombol Tutup celah: {:?}", app.error_card));
        app.apply_gui_fix(i);
        assert_eq!(app.model.doc.bodies.len(), 1, "{:?}", app.error_card);
        assert!(close(volume(&app), 40.0 * 30.0 * 5.0));
    }

    #[test]
    fn feature_tree_edits_value_suppresses_and_refuses_unsafe_delete() {
        let mut app = DuCADApp::new_for_test();
        let out = app
            .agent_call_local(
                "run_ops",
                serde_json::json!({ "ops": [
                    {"op":"primitive","id":"b","shape":{"box":{"size":[20,20,20]}}},
                    {"op":"fillet","id":"f1","body":"b","edges":"|Z","radius":2}
                ]}),
            )
            .unwrap();
        assert!(!out.is_error, "{}", out.payload);
        let rows = app.oplog_row_infos();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].value, Some(("radius".to_string(), 2.0)));
        assert_eq!(rows[0].dependents, ["f1"]);
        let v2 = volume(&app);

        app.set_op_value("f1", "radius", 4.0);
        assert!(
            volume(&app) < v2,
            "radius lebih besar membuang lebih banyak material"
        );
        assert_eq!(
            app.oplog_row_infos()[1].value,
            Some(("radius".to_string(), 4.0))
        );
        app.undo_model();
        assert!(close(volume(&app), v2), "edit nilai = satu langkah undo");

        app.toggle_op_suppressed("f1");
        assert!(close(volume(&app), 8000.0), "fillet nonaktif → kubus penuh");
        assert!(app.oplog_row_infos()[1].suppressed);
        app.toggle_op_suppressed("f1");
        assert!(close(volume(&app), v2));

        app.delete_op("b");
        assert!(app.error_card.open, "body masih dipakai fillet");
        assert_eq!(app.agent_meta.design.oplog.len(), 2, "oplog utuh");
        assert!(close(volume(&app), v2));
    }

    /// T4 lewat GUI: simpan → buka di aplikasi baru → oplog dan ikatan
    /// sketsa hidup lagi; edit sketsa setelah dibuka tetap memperbarui hilir.
    #[test]
    fn gui_oplog_survives_save_and_reopen() {
        let mut app = DuCADApp::new_for_test();
        let ids = draw_l_profile(&mut app, 40.0);
        app.selected = ids.iter().copied().collect();
        app.extrude_distance_input = "60".into();
        app.extrude_selected();
        app.exit_sketching();
        let id = only_body(&app);
        let s = std::f64::consts::FRAC_1_SQRT_2;
        let ray = PickRay {
            origin: (20.0, -30.0, 20.0),
            dir: (-s, 0.0, -s),
        };
        app.round_via_oplog(
            id,
            RoundKind::Fillet {
                radius: 3.0,
                radius_end: None,
            },
            &[ray],
            1.0,
        )
        .unwrap();
        let path =
            std::env::temp_dir().join(format!("ducad-gui-reopen-{}.ducad", std::process::id()));
        app.save_native_to(path.clone());

        let mut other = DuCADApp::new_for_test();
        other.open_native_path(path.clone());
        let _ = std::fs::remove_file(&path);
        assert_eq!(oplog_kinds(&other), ["sketch", "extrude", "fillet"]);
        assert_eq!(
            other.agent_meta.live_sketches.len(),
            1,
            "ikatan sketsa pulih"
        );
        assert!(close(volume(&other), l_volume(40.0, 3.0)));
        other.sync_agent_meta();
        assert!(!other.agent_meta.warnings.iter().any(|w| w == "oplog_stale"));

        // Edit sketsa di aplikasi yang baru membuka berkas.
        other.set_sketch_plane(ducad_render::PlaneKind::Front);
        other.enter_sketching();
        use ducad_sketch::Entity;
        for (_, e) in other.sketch_mut().entities.iter_mut() {
            if let Entity::Line { start, end, .. } = e {
                if start.x == 40.0 {
                    start.x = 55.0;
                }
                if end.x == 40.0 {
                    end.x = 55.0;
                }
            }
        }
        other.exit_sketching();
        assert!(!other.error_card.open, "{:?}", other.error_card);
        assert!(
            close(volume(&other), l_volume(55.0, 3.0)),
            "{}",
            volume(&other)
        );
    }

    #[test]
    fn second_extrude_from_same_sketch_reuses_bound_op() {
        let mut app = DuCADApp::new_for_test();
        app.set_sketch_plane(ducad_render::PlaneKind::Top);
        app.enter_sketching();
        let sk = app.sketch_mut();
        let a = sk
            .entities
            .insert(ducad_sketch::Entity::circle(DVec2::new(0.0, 0.0), 5.0));
        let b = sk
            .entities
            .insert(ducad_sketch::Entity::circle(DVec2::new(30.0, 0.0), 5.0));
        app.extrude_distance_input = "10".into();
        app.selected = [a].into_iter().collect();
        app.extrude_selected();
        app.selected = [b].into_iter().collect();
        app.extrude_selected();
        assert_eq!(oplog_kinds(&app), ["sketch", "extrude", "extrude"]);
        assert_eq!(app.model.doc.bodies.len(), 2);
    }

    #[test]
    fn failed_round_keeps_model_and_maps_verified_fixes() {
        let (mut app, id) = app_with_box("Box", [0.0; 3]);
        let v0 = app.model.geometry[id].shape.volume();
        let e = app
            .round_via_oplog(
                id,
                RoundKind::Fillet {
                    radius: 15.0,
                    radius_end: None,
                },
                &[],
                1.0,
            )
            .unwrap_err();
        assert_eq!(e.code, OpErrorCode::FilletRadiusTooLarge);
        assert!((app.model.geometry[id].shape.volume() - v0).abs() < 1e-9);
        assert!(app.agent_meta.design.oplog.is_empty());
        let fixes = gui_fixes_from_op_error(&e);
        assert!(!fixes.is_empty(), "{e:?}");
        assert!(matches!(fixes[0].1, GuiFix::FilletRadius(r) if r < 15.0));
    }
}
