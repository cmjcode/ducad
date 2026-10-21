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

/// Spec bidang op (JSON) untuk bidang sketsa GUI: nama standar bila di
/// titik asal, selain itu bidang eksplisit `frame` (datum bersudut/3 titik/
/// offset; ADR 0010 urutan 7).
pub(crate) fn sketch_plane_json(plane: &ducad_render::SketchPlane) -> serde_json::Value {
    let named = match plane.kind {
        ducad_render::PlaneKind::Top => Some("XY"),
        ducad_render::PlaneKind::Front => Some("XZ"),
        ducad_render::PlaneKind::Right => Some("YZ"),
        ducad_render::PlaneKind::Custom(_) => None,
    };
    if let (Some(n), true) = (named, plane.origin.length() < 1e-6) {
        return serde_json::json!(n);
    }
    let a = |v: glam::Vec3| [v.x as f64, v.y as f64, v.z as f64];
    serde_json::json!({ "frame": {
        "origin": a(plane.origin), "u": a(plane.u_axis), "v": a(plane.v_axis), "normal": a(plane.normal),
    }})
}

/// Seperti [`sketch_plane_json`], digeser `offset` di sepanjang normal.
fn offset_plane_json(plane: &ducad_render::SketchPlane, offset: f64) -> serde_json::Value {
    let mut moved = *plane;
    moved.origin += plane.normal * offset as f32;
    match sketch_plane_json(plane) {
        serde_json::Value::String(n) => serde_json::json!({ "base": n, "offset": offset }),
        _ => sketch_plane_json(&moved),
    }
}

impl DuCADApp {
    /// Spec bidang op untuk `PlaneRef` slot sketsa GUI; datum dicari di
    /// `datum_planes` lewat id stabilnya. `None` = datum tidak ditemukan.
    pub(crate) fn plane_json_for(
        &self,
        plane: ducad_sketch::PlaneRef,
    ) -> Option<serde_json::Value> {
        Some(match plane {
            ducad_sketch::PlaneRef::Top => serde_json::json!("XY"),
            ducad_sketch::PlaneRef::Front => serde_json::json!("XZ"),
            ducad_sketch::PlaneRef::Right => serde_json::json!("YZ"),
            ducad_sketch::PlaneRef::Datum(id) => {
                sketch_plane_json(&self.datum_planes.iter().find(|d| d.id == id)?.plane)
            }
        })
    }

    /// Op `sketch` dari isi slot GUI `sid` (spec bernama, ADR 0010).
    pub(crate) fn sketch_op_for_slot(&self, sid: ducad_sketch::SketchId, id: &str) -> Option<Op> {
        let slot = self.sketch_set.get(sid)?;
        let plane = self.plane_json_for(slot.plane)?;
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
        self.adopt_replayed_session(session, what);
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
        if self.plane_json_for(slot.plane).is_none() || self.selected.is_empty() || distance == 0.0
        {
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

        self.run_feature_on_sketch(
            sid, bound, sketch_id, sketch_op, extrude, "Extrude", &details,
        )
    }

    /// Jalankan op fitur `feature` yang merujuk op `sketch` slot `sid`:
    /// sketsa belum terikat → batch `[sketch, feature]` + ikat slot; sudah
    /// terikat dan sama → `[feature]`; sudah terikat tapi berubah → ganti op
    /// `sketch` + tambah fitur dengan replay penuh. Satu langkah undo.
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    fn run_feature_on_sketch(
        &mut self,
        sid: ducad_sketch::SketchId,
        bound: Option<String>,
        sketch_id: String,
        sketch_op: Op,
        feature: Op,
        title: &str,
        details: &str,
    ) -> Result<(), OpError> {
        match bound {
            None => {
                self.agent_meta.live_sketches.insert(sketch_id.clone(), sid);
                let r = self.run_gui_ops(vec![sketch_op, feature], title, details);
                if r.is_err() {
                    self.agent_meta.live_sketches.remove(&sketch_id);
                }
                r.map(|_| ())
            }
            Some(id) => {
                let current = self.agent_meta.design.oplog.iter().find(|o| o.id() == id);
                if current == Some(&sketch_op) {
                    self.run_gui_ops(vec![feature], title, details).map(|_| ())
                } else {
                    let replace = vec![ducad_engine::ReplaceOp { id, op: sketch_op }];
                    self.edit_oplog_via_replay(replace, vec![feature], title)
                }
            }
        }
    }

    /// Emboss/deboss profil sketsa aktif ke `body` sebagai op oplog (sketsa
    /// terikat + op `emboss`). Tanpa seleksi = semua region. `Ok(false)` =
    /// belum didukung (bidang datum tak dikenal) → jalur lama.
    #[allow(clippy::result_large_err)]
    pub(crate) fn emboss_via_oplog(
        &mut self,
        body: BodyId,
        depth: f64,
        deboss: bool,
    ) -> Result<bool, OpError> {
        let sid = self.active_sketch_id();
        let Some(slot) = self.sketch_set.get(sid) else {
            return Ok(false);
        };
        if self.plane_json_for(slot.plane).is_none() || !self.model.doc.bodies.contains_key(body) {
            return Ok(false);
        }
        let names: Vec<String> = self
            .selected
            .iter()
            .filter(|id| slot.sketch.entities.contains_key(**id))
            .map(|id| ducad_engine::ops::from_sketch::entity_spec_name(&slot.sketch, *id))
            .collect();
        self.sync_agent_meta();
        let rename = self.make_body_name_unique(body);
        let body_name = self.model.doc.bodies[body].name.clone();
        let bound = self.bound_sketch_id(sid);
        let sketch_id = bound.clone().unwrap_or_else(|| self.unique_op_id("sketch"));
        let result = (|| {
            let sketch_op = self
                .sketch_op_for_slot(sid, &sketch_id)
                .ok_or_else(|| OpError::invalid("sketsa aktif tidak bisa diterjemahkan ke op"))?;
            let profile = if names.is_empty() {
                serde_json::json!("all")
            } else {
                serde_json::json!({ "names": names })
            };
            let label = if deboss { "deboss" } else { "emboss" };
            let feature: Op = serde_json::from_value(serde_json::json!({
                "op": "emboss", "id": self.unique_op_id(label), "sketch": sketch_id,
                "profile": profile, "body": body_name, "depth": depth, "deboss": deboss,
            }))
            .map_err(|e| OpError::invalid(format!("op emboss GUI tidak valid: {e}")))?;
            let title = if deboss { "Deboss" } else { "Emboss" };
            let details = format!("{title} {depth:.1} mm pada {body_name}");
            self.run_feature_on_sketch(
                sid,
                bound.clone(),
                sketch_id.clone(),
                sketch_op,
                feature,
                title,
                &details,
            )
        })();
        if let Some(mut cmd) = rename {
            if result.is_ok() {
                self.model_undo.attach_to_top(cmd);
            } else {
                ducad_core::Command::revert(cmd.as_mut(), &mut self.model);
            }
        }
        result.map(|()| true)
    }

    /// Op edit langsung pada satu body (ADR 0010 urutan 9). `make(body_name)`
    /// membangun JSON op; nama body dibuat unik dalam langkah undo yang sama.
    #[allow(clippy::result_large_err)]
    pub(crate) fn direct_op_via_oplog(
        &mut self,
        body: BodyId,
        title: &str,
        make: impl FnOnce(&Self, &str) -> Option<serde_json::Value>,
    ) -> Result<bool, OpError> {
        self.sync_agent_meta();
        let (names, renames) = self
            .unique_body_names(&[body])
            .ok_or_else(|| OpError::invalid("body terpilih tidak ada lagi"))?;
        let Some(op) = make(self, &names[0]) else {
            for mut cmd in renames.into_iter().rev() {
                ducad_core::Command::revert(&mut cmd, &mut self.model);
            }
            return Ok(false);
        };
        let details = format!("{title} pada {}", names[0]);
        self.run_with_renames(op, renames, title, &details)
            .map(|()| true)
    }

    /// Selector semantik untuk face `indices` body `body`.
    pub(crate) fn face_selector(&self, body: BodyId, indices: &[usize]) -> Option<String> {
        let shape = &self.model.geometry.get(body)?.shape;
        ducad_engine::select::suggest_faces(shape, indices).map(|s| s.selector)
    }

    /// Revolve entitas terpilih di sketsa aktif sebagai op oplog; sumbu
    /// dalam koordinat bidang sketsa. `Ok(false)` = belum didukung jalur
    /// oplog (bidang datum) → jalur lama.
    #[allow(clippy::result_large_err)]
    pub(crate) fn revolve_via_oplog(
        &mut self,
        axis_origin: (f64, f64),
        axis_dir: (f64, f64),
        angle_deg: Option<f64>,
    ) -> Result<bool, OpError> {
        let sid = self.active_sketch_id();
        let Some(slot) = self.sketch_set.get(sid) else {
            return Ok(false);
        };
        if self.plane_json_for(slot.plane).is_none() || self.selected.is_empty() {
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
        self.sync_agent_meta();
        let bound = self.bound_sketch_id(sid);
        let sketch_id = bound.clone().unwrap_or_else(|| self.unique_op_id("sketch"));
        let sketch_op = self
            .sketch_op_for_slot(sid, &sketch_id)
            .ok_or_else(|| OpError::invalid("sketsa aktif tidak bisa diterjemahkan ke op"))?;
        let mut v = serde_json::json!({
            "op": "revolve",
            "id": self.unique_op_id("revolve"),
            "sketch": sketch_id,
            "profile": { "names": names },
            "axis": { "origin": [axis_origin.0, axis_origin.1], "dir": [axis_dir.0, axis_dir.1] },
        });
        if let Some(a) = angle_deg {
            v["angle_deg"] = serde_json::json!(a);
        }
        let feature: Op = serde_json::from_value(v)
            .map_err(|e| OpError::invalid(format!("op revolve GUI tidak valid: {e}")))?;
        let details = format!(
            "Revolve {:.0} deg dari {sketch_id}",
            angle_deg.unwrap_or(360.0)
        );
        self.run_feature_on_sketch(
            sid, bound, sketch_id, sketch_op, feature, "Revolve", &details,
        )
        .map(|()| true)
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
                self.adopt_replayed_session(session, "Suppress fitur");
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
                self.adopt_replayed_session(session, "Hapus fitur");
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

impl DuCADApp {
    /// Nama-nama body (dibuat unik dulu) untuk dirujuk op. Command rename
    /// yang sudah diterapkan dikembalikan untuk dititipkan ke transaksi op.
    fn unique_body_names(
        &mut self,
        ids: &[BodyId],
    ) -> Option<(Vec<String>, Vec<RenameBodyCommand>)> {
        let mut renames = Vec::new();
        let mut names = Vec::new();
        for id in ids {
            if let Some(cmd) = self.make_body_name_unique(*id) {
                renames.push(*cmd);
            }
            names.push(self.model.doc.bodies.get(*id)?.name.clone());
        }
        Some((names, renames))
    }

    /// Jalankan `op` sebagai aksi GUI, dengan rename body yang menyertainya
    /// ikut satu langkah undo (atau dibatalkan bila op gagal).
    #[allow(clippy::result_large_err)]
    fn run_with_renames(
        &mut self,
        op: serde_json::Value,
        renames: Vec<RenameBodyCommand>,
        title: &str,
        details: &str,
    ) -> Result<(), OpError> {
        let result = serde_json::from_value::<Op>(op)
            .map_err(|e| OpError::invalid(format!("op GUI tidak valid: {e}")))
            .and_then(|op| self.run_gui_ops(vec![op], title, details).map(|_| ()));
        for mut cmd in renames.into_iter().rev() {
            if result.is_ok() {
                self.model_undo.attach_to_top(Box::new(cmd));
            } else {
                ducad_core::Command::revert(&mut cmd, &mut self.model);
            }
        }
        result
    }

    /// Boolean dua body sebagai op oplog. `a` = sasaran (dipilih pertama),
    /// `b` = alat/pemotong. Hasilnya body baru bernama id op.
    #[allow(clippy::result_large_err)]
    pub(crate) fn boolean_via_oplog(
        &mut self,
        a: BodyId,
        b: BodyId,
        kind: ducad_engine::model::BooleanKind,
    ) -> Result<(), OpError> {
        use ducad_engine::model::BooleanKind;
        self.sync_agent_meta();
        let (names, renames) = self
            .unique_body_names(&[a, b])
            .ok_or_else(|| OpError::invalid("body terpilih tidak ada lagi"))?;
        let (kind_s, title) = match kind {
            BooleanKind::Union => ("union", "Boolean Union"),
            BooleanKind::Subtract => ("subtract", "Boolean Subtract"),
            BooleanKind::Intersect => ("intersect", "Boolean Intersect"),
        };
        let id = self.unique_op_id(kind_s);
        let op = serde_json::json!({
            "op": "boolean", "id": id, "kind": kind_s, "a": names[0], "b": names[1]
        });
        let details = format!("{title}: {} dan {}", names[0], names[1]);
        self.run_with_renames(op, renames, title, &details)
    }

    /// Pattern 3D body-body terpilih sebagai op oplog dalam SATU batch
    /// (satu langkah undo); salinan baru bernama `<id op>.<n>`.
    #[allow(clippy::result_large_err)]
    pub(crate) fn pattern_via_oplog(
        &mut self,
        bodies: &[BodyId],
        kind: serde_json::Value,
    ) -> Result<(), OpError> {
        self.sync_agent_meta();
        let (names, renames) = self
            .unique_body_names(bodies)
            .ok_or_else(|| OpError::invalid("body terpilih tidak ada lagi"))?;
        let mut ops = Vec::with_capacity(names.len());
        let mut used: Vec<String> = Vec::new();
        for name in &names {
            let mut id = self.unique_op_id("pattern");
            let mut n = 1;
            while used.contains(&id) {
                n += 1;
                id = format!("{}_{n}", self.unique_op_id("pattern"));
            }
            used.push(id.clone());
            let op = serde_json::json!({
                "op": "pattern", "id": id, "body": name, "kind": kind.clone(), "merge": false
            });
            ops.push(
                serde_json::from_value::<Op>(op)
                    .map_err(|e| OpError::invalid(format!("op pattern GUI tidak valid: {e}")))?,
            );
        }
        let details = format!("Pattern 3D dari {}", names.join(", "));
        let result = self.run_gui_ops(ops, "Pattern 3D", &details).map(|_| ());
        for mut cmd in renames.into_iter().rev() {
            if result.is_ok() {
                self.model_undo.attach_to_top(Box::new(cmd));
            } else {
                ducad_core::Command::revert(&mut cmd, &mut self.model);
            }
        }
        result
    }
}

/// Command sketsa: pasang/hapus ekspresi param sebuah dimensi
/// (`Sketch::dim_exprs`), satu langkah undo bersama perubahan nilainya.
pub struct SetDimExpr {
    key: String,
    pending: Option<String>,
}

impl ducad_core::Command<ducad_sketch::Sketch> for SetDimExpr {
    fn name(&self) -> &str {
        "Ikat dimensi ke parameter"
    }
    fn apply(&mut self, sk: &mut ducad_sketch::Sketch) {
        self.swap(sk);
    }
    fn revert(&mut self, sk: &mut ducad_sketch::Sketch) {
        self.swap(sk);
    }
}

impl SetDimExpr {
    fn swap(&mut self, sk: &mut ducad_sketch::Sketch) {
        let prev = match self.pending.take() {
            Some(e) => sk.dim_exprs.insert(self.key.clone(), e),
            None => sk.dim_exprs.remove(&self.key),
        };
        self.pending = prev;
    }
}

impl DuCADApp {
    /// Nilai input dimensi: `$param`/ekspresi param dievaluasi terhadap
    /// parameter dokumen (mm), selain itu angka/satuan biasa. `Ok((mm,
    /// ekspresi))`; ekspresi `Some` berarti dimensi diikat ke param.
    pub(crate) fn eval_dim_input(&self, text: &str) -> Result<(f64, Option<String>), String> {
        let t = text.trim();
        if !t.contains('$') {
            return self.unit.parse_input_mm(t).map(|v| (v, None));
        }
        let params = self.agent_meta.design.effective_params();
        ducad_engine::ops::eval(&ducad_engine::ops::Num::Expr(t.to_string()), &params)
            .map(|v| (v, Some(t.to_string())))
            .map_err(|e| e.message)
    }

    /// Perbarui dimensi `index` ke `new_constraint` dan pasang/hapus ikatan
    /// param-nya dalam satu langkah undo sketsa.
    pub(crate) fn update_dimension(
        &mut self,
        index: usize,
        new_constraint: ducad_sketch::constraint::Constraint,
        expr: Option<String>,
    ) {
        let Some(key) = ducad_sketch::constraint::dim_key(&new_constraint) else {
            self.update_constraint(index, new_constraint);
            return;
        };
        let sid = self.active_sketch_id();
        self.sketch_set.set_active(sid);
        self.sketch_set.active_mut().undo.begin("Dimensi");
        self.constraint_status = None;
        self.update_constraint(index, new_constraint);
        // `update_constraint` mengisi `constraint_status` bila gagal.
        let applied = self.constraint_status.is_none();
        if applied && self.sketch().dim_exprs.get(&key) != expr.as_ref() {
            self.execute_sketch_command(Box::new(SetDimExpr { key, pending: expr }));
        }
        self.sketch_set.active_mut().undo.commit();
    }

    /// Salin hasil replay ke slot GUI yang terikat: entitas dicocokkan lewat
    /// nama spec (EntityId GUI tetap), nilai dimensi terikat param dihitung
    /// ulang. Dipanggil di antara penanda awal/akhir langkah undo.
    pub(crate) fn update_live_slots_from(&mut self, session: &ducad_engine::Session) {
        let params = session.meta().design.effective_params();
        let live: Vec<(String, ducad_sketch::SketchId)> = self
            .agent_meta
            .live_sketches
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        for (id, sid) in live {
            let Ok((replayed, _)) = session.sketch(&id) else {
                continue;
            };
            let Some(gui) = self.sketch_set.sketch(sid) else {
                continue;
            };
            let by_name: std::collections::HashMap<&str, &ducad_sketch::Entity> = replayed
                .entity_names
                .iter()
                .filter_map(|(rid, n)| Some((n.as_str(), replayed.entities.get(*rid)?)))
                .collect();
            let mut updated = gui.clone();
            for (gid, ent) in updated.entities.iter_mut() {
                let name = ducad_engine::ops::from_sketch::entity_spec_name(gui, gid);
                if let Some(r) = by_name.get(name.as_str()) {
                    let construction = ent.is_construction();
                    *ent = (*r).clone().with_construction(construction);
                }
            }
            for c in updated.constraints.iter_mut() {
                use ducad_sketch::constraint::Constraint;
                let Some(expr) = ducad_sketch::constraint::dim_key(c)
                    .and_then(|k| gui.dim_exprs.get(&k).cloned())
                else {
                    continue;
                };
                let Ok(v) = ducad_engine::ops::eval(&ducad_engine::ops::Num::Expr(expr), &params)
                else {
                    continue;
                };
                match c {
                    Constraint::Distance { value, .. } | Constraint::Radius { value, .. } => {
                        *value = v
                    }
                    Constraint::Angle { value, .. } => *value = v.to_radians(),
                    _ => {}
                }
            }
            if let Some(slot) = self.sketch_set.sketch_mut(sid) {
                *slot = updated;
            }
        }
    }
}

/// Entitas spec (JSON) yang menggambar `profile` persis: garis, busur
/// (`arc3`), Bezier (`path`), lingkaran, elips, dan lubang di dalamnya.
fn profile_entity_specs(profile: &ducad_kernel::Profile, out: &mut Vec<serde_json::Value>) {
    use ducad_kernel::{Profile, ProfileSegment};
    let p = |v: (f64, f64)| serde_json::json!([v.0, v.1]);
    match profile {
        Profile::Circle { center, radius } => {
            out.push(serde_json::json!({ "circle": { "center": p(*center), "r": radius } }))
        }
        Profile::Ellipse {
            center,
            radius_x,
            radius_y,
        } => out.push(serde_json::json!({ "ellipse": {
            "center": p(*center), "rx": radius_x, "ry": radius_y } })),
        Profile::Loop(segs) => {
            for seg in segs {
                out.push(match *seg {
                    ProfileSegment::Line { start, end } => {
                        serde_json::json!({ "line": { "from": p(start), "to": p(end) } })
                    }
                    ProfileSegment::Arc { start, via, end } => {
                        serde_json::json!({ "arc3": { "p1": p(start), "p2": p(via), "p3": p(end) } })
                    }
                    ProfileSegment::Bezier { start, c1, c2, end } => {
                        serde_json::json!({ "path": { "subpaths": [{
                            "start": p(start),
                            "segs": [{ "cubic": { "c1": p(c1), "c2": p(c2), "to": p(end) } }],
                        }] } })
                    }
                });
            }
        }
        Profile::WithHoles { outer, holes } => {
            profile_entity_specs(outer, out);
            for h in holes {
                profile_entity_specs(h, out);
            }
        }
    }
}

/// Selector sumbu (`"X"`, `"Y"`, `"Z"`) dan tanda untuk arah sejajar sumbu.
fn axis_of(dir: (f64, f64, f64)) -> Option<(&'static str, bool)> {
    let v = [dir.0, dir.1, dir.2];
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len < 1e-12 {
        return None;
    }
    ["X", "Y", "Z"]
        .iter()
        .enumerate()
        .find(|(i, _)| (v[*i] / len).abs() > 1.0 - 1e-9)
        .map(|(i, a)| (*a, v[i] > 0.0))
}

impl DuCADApp {
    /// Op `sketch` turunan yang menggambar `profile` di `plane` (JSON spec).
    #[allow(clippy::result_large_err)]
    fn profile_sketch_op(
        &self,
        id: &str,
        plane: serde_json::Value,
        profile: &ducad_kernel::Profile,
    ) -> Result<Op, OpError> {
        let mut entities = Vec::new();
        profile_entity_specs(profile, &mut entities);
        serde_json::from_value(serde_json::json!({
            "op": "sketch", "id": id, "plane": plane, "entities": entities,
        }))
        .map_err(|e| OpError::invalid(format!("profil tidak bisa dijadikan op sketch: {e}")))
    }

    /// Loft dua profil (`bottom` di `plane`, `top` setinggi `height` di
    /// sepanjang normal bidang) sebagai op oplog. `Ok(false)` = bidang datum
    /// → jalur lama.
    #[allow(clippy::result_large_err)]
    pub(crate) fn loft_via_oplog(
        &mut self,
        bottom: &ducad_kernel::Profile,
        top: &ducad_kernel::Profile,
        plane: &ducad_render::SketchPlane,
        height: f64,
    ) -> Result<bool, OpError> {
        self.sync_agent_meta();
        let a = self.unique_op_id("loft_bottom");
        let b = self.unique_op_id("loft_top");
        let id = self.unique_op_id("loft");
        let ops = vec![
            self.profile_sketch_op(&a, sketch_plane_json(plane), bottom)?,
            self.profile_sketch_op(&b, offset_plane_json(plane, height), top)?,
            serde_json::from_value(
                serde_json::json!({ "op": "loft", "id": id, "sections": [a, b] }),
            )
            .map_err(|e| OpError::invalid(format!("op loft GUI tidak valid: {e}")))?,
        ];
        let details = format!("Loft 2 profil, tinggi {height:.1} mm");
        self.run_gui_ops(ops, "Loft", &details).map(|_| true)
    }

    /// Sweep `profile` (di `profile_plane`) sepanjang jalur lurus/polyline
    /// sebagai op oplog. Jalur berbusur → `Ok(false)` (jalur lama): titik
    /// polyline akan menghilangkan lengkungnya.
    #[allow(clippy::result_large_err)]
    pub(crate) fn sweep_via_oplog(
        &mut self,
        profile: &ducad_kernel::Profile,
        profile_plane: &ducad_render::SketchPlane,
        path: &[ducad_kernel::PathSegment],
    ) -> Result<bool, OpError> {
        use ducad_kernel::PathSegment;
        let plane = sketch_plane_json(profile_plane);
        let mut points: Vec<[f64; 3]> = Vec::new();
        let mut push = |p: [f64; 3]| {
            let same = points
                .last()
                .is_some_and(|q: &[f64; 3]| (0..3).all(|k| (q[k] - p[k]).abs() < 1e-9));
            if !same {
                points.push(p);
            }
        };
        for seg in path {
            match seg {
                PathSegment::Line { start, end } => {
                    push(*start);
                    push(*end);
                }
                PathSegment::Polyline(pts) => pts.iter().for_each(|p| push(*p)),
                PathSegment::Arc { .. } => return Ok(false),
            }
        }
        if points.len() < 2 {
            return Ok(false);
        }
        self.sync_agent_meta();
        let section = self.unique_op_id("sweep_section");
        let id = self.unique_op_id("sweep");
        let ops = vec![
            self.profile_sketch_op(&section, plane, profile)?,
            serde_json::from_value(serde_json::json!({
                "op": "sweep", "id": id, "sketch": section, "path": { "points": points },
            }))
            .map_err(|e| OpError::invalid(format!("op sweep GUI tidak valid: {e}")))?,
        ];
        self.run_gui_ops(ops, "Sweep", "Sweep profil sepanjang jalur")
            .map(|_| true)
    }

    /// Draft face yang dipilih (ray) sebagai op oplog. Bidang netral = face
    /// terbawah searah tarik (`<Z` untuk tarik +Z). Arah tarik tidak sejajar
    /// sumbu → `Ok(false)` (jalur lama).
    #[allow(clippy::result_large_err)]
    pub(crate) fn draft_via_oplog(
        &mut self,
        body: BodyId,
        rays: &[PickRay],
        angle_deg: f64,
        pull: (f64, f64, f64),
    ) -> Result<bool, OpError> {
        let Some((axis, positive)) = axis_of(pull) else {
            return Ok(false);
        };
        let Some(geo) = self.model.geometry.get(body) else {
            return Ok(false);
        };
        let mut picked: Vec<usize> = rays
            .iter()
            .filter_map(|r| ducad_kernel::face_index_for_ray(&geo.shape, *r))
            .collect();
        picked.sort_unstable();
        picked.dedup();
        let Some(faces) = ducad_engine::select::suggest_faces(&geo.shape, &picked) else {
            return Ok(false);
        };
        let neutral = if positive {
            format!("<{axis}")
        } else {
            format!(">{axis}")
        };
        self.sync_agent_meta();
        let (names, renames) = self
            .unique_body_names(&[body])
            .ok_or_else(|| OpError::invalid("body terpilih tidak ada lagi"))?;
        let id = self.unique_op_id("draft");
        let op = serde_json::json!({
            "op": "draft", "id": id, "body": names[0], "faces": faces.selector,
            "angle_deg": angle_deg, "neutral": neutral, "pull": [pull.0, pull.1, pull.2],
        });
        let details = format!("Draft {angle_deg:.1} deg pada {}", names[0]);
        self.run_with_renames(op, renames, "Draft Angle", &details)
            .map(|()| true)
    }

    /// Op `shell` setara dengan pilihan shell GUI pada `shape` (SEBELUM
    /// shell): face yang dibuka → selector semantik. `None` bila pilihan
    /// tidak bisa dinyatakan (shell variabel, face tak ditemukan).
    pub(crate) fn shell_op_for(
        &self,
        body: &str,
        shape: &ducad_kernel::KernelShape,
        pick: &crate::modeling::operations::ShellPick,
        thickness: f64,
        depth: f64,
    ) -> Option<Op> {
        use crate::modeling::operations::ShellPick;
        use ducad_kernel::Direction;
        let remove = match pick {
            ShellPick::Direction(d) => match d {
                Direction::PosX => ">X",
                Direction::NegX => "<X",
                Direction::PosY => ">Y",
                Direction::NegY => "<Y",
                Direction::PosZ => ">Z",
                Direction::NegZ => "<Z",
            }
            .to_string(),
            ShellPick::Rays(rays) => {
                let mut idx: Vec<usize> = rays
                    .iter()
                    .filter_map(|r| ducad_kernel::face_index_for_ray(shape, *r))
                    .collect();
                idx.sort_unstable();
                idx.dedup();
                if idx.len() != rays.len() {
                    return None;
                }
                ducad_engine::select::suggest_faces(shape, &idx)?.selector
            }
        };
        let mut v = serde_json::json!({
            "op": "shell", "id": self.unique_op_id("shell"), "body": body,
            "remove_faces": remove, "thickness": thickness,
        });
        if depth > 0.0 {
            v["depth"] = serde_json::json!(depth);
        }
        serde_json::from_value(v).ok()
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

    /// Subtract memakai urutan pilih: pertama = sasaran, kedua = pemotong.
    #[test]
    fn gui_subtract_respects_selection_order_and_is_an_op() {
        for (first_big, expected) in [(true, 8000.0 - 2000.0), (false, 3000.0 - 2000.0)] {
            let (mut app, big) = app_with_box("Big", [0.0; 3]);
            let geo = compute::primitive(
                &compute::PrimitiveShape::Box {
                    size: [10.0, 10.0, 30.0],
                    centered: false,
                },
                [5.0, 5.0, -5.0],
            )
            .unwrap();
            let small = app.model.doc.add_body("Tool");
            app.model.geometry.insert(small, geo);
            app.selected_bodies.clear();
            if first_big {
                app.selected_bodies.insert(big);
                app.selected_bodies.insert(small);
            } else {
                app.selected_bodies.insert(small);
                app.selected_bodies.insert(big);
            }
            app.boolean_selected(
                ducad_engine::model::BooleanKind::Subtract,
                "Subtract",
                "Subtract",
            );
            if first_big {
                assert_eq!(oplog_kinds(&app), ["boolean"]);
                let v = serde_json::to_value(&app.agent_meta.design.oplog[0]).unwrap();
                assert_eq!(
                    (v["a"].as_str(), v["b"].as_str()),
                    (Some("Big"), Some("Tool"))
                );
                assert!(close(volume(&app), expected), "{}", volume(&app));
            } else {
                // Tool (10x10x30) dikurangi kubus: sisa dua ujung 10x10x5.
                let total: f64 = app
                    .model
                    .geometry
                    .values()
                    .map(|g| g.shape.volume().abs())
                    .sum();
                assert!(close(total, expected), "{total}");
            }
        }
    }

    #[test]
    fn gui_linear_pattern_becomes_op_and_replays() {
        let (mut app, id) = app_with_box("Box", [0.0; 3]);
        app.selected_bodies.insert(id);
        app.pattern_kind = ducad_ui::PatternKind::Linear;
        app.pattern_count_x = 3;
        app.pattern_count_y = 1;
        app.pattern_count_z = 1;
        app.pattern_pitch_x = 30.0;
        app.apply_pattern_3d();
        assert_eq!(oplog_kinds(&app), ["pattern"]);
        assert_eq!(app.model.doc.bodies.len(), 3, "asli + 2 salinan");
        let s = ducad_engine::Session::replay(app.agent_meta.design.clone()).unwrap();
        assert_eq!(s.summary().bodies.len(), 3);
        app.undo_model();
        assert_eq!(app.model.doc.bodies.len(), 1);
        assert!(app.agent_meta.design.oplog.is_empty());
    }

    /// Step 2 butir 3: dimensi sketsa GUI diikat ke `$base`; mengubah param
    /// menggerakkan body DAN garis sketsa GUI; undo memulihkan keduanya.
    #[test]
    fn gui_dimension_bound_to_param_drives_sketch_and_body() {
        use ducad_sketch::constraint::{Constraint, PointRef};
        use ducad_sketch::Entity;
        let mut app = DuCADApp::new_for_test();
        let row = |n: &str, v: &str| crate::design_tools::ParamRow {
            name: n.into(),
            orig_name: None,
            value: v.into(),
        };
        app.apply_param_rows(&[row("base", "40")]).unwrap();
        let ids = draw_l_profile(&mut app, 40.0);
        {
            let sk = app.sketch_mut();
            for i in 0..6 {
                sk.constraints.push(Constraint::Coincident {
                    a: PointRef::LineEnd(ids[i]),
                    b: PointRef::LineStart(ids[(i + 1) % 6]),
                });
            }
            for i in [0, 2, 4] {
                sk.constraints.push(Constraint::Horizontal { line: ids[i] });
            }
            for i in [1, 3, 5] {
                sk.constraints.push(Constraint::Vertical { line: ids[i] });
            }
            sk.constraints.push(Constraint::Fixed {
                point: PointRef::LineStart(ids[0]),
                target: DVec2::ZERO,
            });
            sk.constraints.push(Constraint::Distance {
                a: PointRef::LineStart(ids[0]),
                b: PointRef::LineEnd(ids[0]),
                value: 40.0,
                driven: false,
            });
        }
        let idx = app.sketch().constraints.len() - 1;
        let (v, expr) = app.eval_dim_input("$base").unwrap();
        assert_eq!((v, expr.as_deref()), (40.0, Some("$base")));
        app.update_dimension(
            idx,
            Constraint::Distance {
                a: PointRef::LineStart(ids[0]),
                b: PointRef::LineEnd(ids[0]),
                value: v,
                driven: false,
            },
            expr,
        );
        assert_eq!(
            app.sketch().dim_exprs.len(),
            1,
            "{:?}",
            app.constraint_status
        );

        app.selected = ids.iter().copied().collect();
        app.extrude_distance_input = "60".into();
        app.extrude_selected();
        app.exit_sketching();
        let sketch_json = serde_json::to_string(&app.agent_meta.design.oplog[0]).unwrap();
        assert!(sketch_json.contains("\"$base\""), "{sketch_json}");
        assert!(close(volume(&app), l_volume(40.0, 0.0)));

        let base_len = |app: &DuCADApp| {
            let sid = app
                .agent_meta
                .live_sketches
                .values()
                .next()
                .copied()
                .unwrap();
            match app.sketch_set.sketch(sid).unwrap().entities[ids[0]] {
                Entity::Line { start, end, .. } => (end - start).length(),
                _ => f64::NAN,
            }
        };
        app.apply_param_rows(&[crate::design_tools::ParamRow {
            name: "base".into(),
            orig_name: Some("base".into()),
            value: "55".into(),
        }])
        .unwrap();
        assert!(close(volume(&app), l_volume(55.0, 0.0)), "{}", volume(&app));
        assert!(
            (base_len(&app) - 55.0).abs() < 1e-6,
            "sketsa GUI ikut param"
        );
        app.sync_agent_meta();
        assert!(!app.agent_meta.warnings.iter().any(|w| w == "oplog_stale"));
        // Keluar dari mode sketsa tidak boleh "mengembalikan" sketsa lama.
        app.enter_sketching();
        app.exit_sketching();
        assert!(close(volume(&app), l_volume(55.0, 0.0)));

        app.undo_model();
        assert!(close(volume(&app), l_volume(40.0, 0.0)), "{}", volume(&app));
        assert!(
            (base_len(&app) - 40.0).abs() < 1e-6,
            "undo memulihkan sketsa GUI"
        );
    }

    /// Revolve sketsa di bidang Front berputar di sumbu world Z (bukan
    /// selalu XY seperti jalur lama) dan menjadi op oplog.
    #[test]
    fn gui_revolve_on_front_plane_uses_real_plane() {
        let mut app = DuCADApp::new_for_test();
        app.set_sketch_plane(ducad_render::PlaneKind::Front);
        app.enter_sketching();
        let pts = [
            DVec2::new(5.0, 0.0),
            DVec2::new(15.0, 0.0),
            DVec2::new(15.0, 20.0),
            DVec2::new(5.0, 20.0),
        ];
        let ids: Vec<_> = (0..4)
            .map(|i| {
                app.sketch_mut()
                    .entities
                    .insert(ducad_sketch::Entity::line(pts[i], pts[(i + 1) % 4]))
            })
            .collect();
        app.selected = ids.iter().copied().collect();
        assert!(app.revolve_selected((0.0, 0.0), (0.0, 1.0), None));
        assert_eq!(oplog_kinds(&app), ["sketch", "revolve"]);
        let expected = std::f64::consts::PI * (15.0 * 15.0 - 5.0 * 5.0) * 20.0;
        assert!(close(volume(&app), expected), "{}", volume(&app));
        let bbox = app.model.geometry[only_body(&app)]
            .mesh
            .bounding_box()
            .unwrap();
        assert!(
            bbox.0[2].abs() < 1e-3 && (bbox.1[2] - 20.0).abs() < 1e-3,
            "{bbox:?}"
        );
    }

    /// Volume total hasil replay design GUI (pembuktian op setara).
    fn replay_volume(app: &DuCADApp) -> f64 {
        let s = ducad_engine::Session::replay(app.agent_meta.design.clone()).expect("replay");
        s.summary().bodies.iter().map(|b| b.volume).sum()
    }

    fn total_volume(app: &DuCADApp) -> f64 {
        app.model
            .geometry
            .values()
            .map(|g| g.shape.volume().abs())
            .sum()
    }

    #[test]
    fn gui_shell_records_equivalent_op() {
        let (mut app, id) = app_with_box("Box", [0.0; 3]);
        app.selected_bodies.insert(id);
        app.shell_thickness_input = "2".into();
        app.shell_direction = ducad_kernel::Direction::PosZ;
        app.selected_faces.clear();
        app.active_face = None;
        app.shell_selected_body();
        assert_eq!(oplog_kinds(&app), ["shell"]);
        let v = serde_json::to_value(&app.agent_meta.design.oplog[0]).unwrap();
        assert_eq!(v["remove_faces"], ">Z");
        let expected = 8000.0 - 16.0 * 16.0 * 18.0;
        assert!(
            close(total_volume(&app), expected),
            "{}",
            total_volume(&app)
        );
        assert!(close(replay_volume(&app), total_volume(&app)));
        app.undo_model();
        assert!(close(total_volume(&app), 8000.0));
        assert!(app.agent_meta.design.oplog.is_empty());
    }

    #[test]
    fn gui_draft_records_op_with_bottom_neutral_plane() {
        let (mut app, id) = app_with_box("Box", [0.0; 3]);
        app.selected_bodies.insert(id);
        // Ray menembak dinding +X (x=20) di tengahnya.
        let ray = PickRay {
            origin: (50.0, 10.0, 10.0),
            dir: (-1.0, 0.0, 0.0),
        };
        app.selected_faces = vec![ray];
        app.apply_draft_angle(5.0, (0.0, 0.0, 1.0));
        assert_eq!(oplog_kinds(&app), ["draft"], "{:?}", app.model_status);
        let v = serde_json::to_value(&app.agent_meta.design.oplog[0]).unwrap();
        assert_eq!(v["neutral"], "<Z");
        assert!(total_volume(&app) < 8000.0, "dinding miring ke dalam");
        assert!(close(replay_volume(&app), total_volume(&app)));
    }

    #[test]
    fn gui_loft_records_ops_on_the_real_plane() {
        let mut app = DuCADApp::new_for_test();
        app.set_sketch_plane(ducad_render::PlaneKind::Front);
        app.enter_sketching();
        let sq = [
            DVec2::new(-10.0, -10.0),
            DVec2::new(10.0, -10.0),
            DVec2::new(10.0, 10.0),
            DVec2::new(-10.0, 10.0),
        ];
        let mut ids: Vec<_> = (0..4)
            .map(|i| {
                app.sketch_mut()
                    .entities
                    .insert(ducad_sketch::Entity::line(sq[i], sq[(i + 1) % 4]))
            })
            .collect();
        ids.push(
            app.sketch_mut()
                .entities
                .insert(ducad_sketch::Entity::circle(DVec2::new(40.0, 0.0), 5.0)),
        );
        app.selected = ids.iter().copied().collect();
        app.loft_height_input = "30".into();
        app.loft_selected();
        assert_eq!(
            oplog_kinds(&app),
            ["sketch", "sketch", "loft"],
            "{:?}",
            app.model_status
        );
        assert!(close(replay_volume(&app), total_volume(&app)));
        // Bidang Front: loft berjalan sepanjang -Y (normal bidang), bukan Z.
        let bbox = app.model.geometry[only_body(&app)]
            .mesh
            .bounding_box()
            .unwrap();
        assert!(
            (bbox.0[1] + 30.0).abs() < 1e-3 && bbox.1[1].abs() < 1e-3,
            "{bbox:?}"
        );
    }

    #[test]
    fn gui_sweep_along_line_records_ops() {
        let mut app = DuCADApp::new_for_test();
        app.pending_sweep_profile = Some((
            ducad_kernel::Profile::Circle {
                center: (0.0, 0.0),
                radius: 4.0,
            },
            ducad_render::SketchPlane::top(),
        ));
        app.pending_sweep_path = Some(vec![ducad_kernel::PathSegment::Line {
            start: [0.0, 0.0, 0.0],
            end: [0.0, 0.0, 30.0],
        }]);
        app.sweep_selected();
        assert_eq!(
            oplog_kinds(&app),
            ["sketch", "sweep"],
            "{:?}",
            app.model_status
        );
        // Sapuan lingkaran diaproksimasi kernel (selisih relatif ~1.6e-4).
        let expected = std::f64::consts::PI * 16.0 * 30.0;
        assert!(
            (total_volume(&app) - expected).abs() / expected < 1e-3,
            "{}",
            total_volume(&app)
        );
        assert!(close(replay_volume(&app), total_volume(&app)));
    }

    /// Urutan 7: sketsa di datum bersudut / 3 titik di-extrude lewat oplog
    /// (bidang `frame`), replay-nya identik.
    #[test]
    fn gui_extrude_on_angled_and_three_point_datums_is_an_op() {
        let mut app = DuCADApp::new_for_test();
        let angled = ducad_render::SketchPlane::from_angle_and_edge(
            glam::Vec3::ZERO,
            glam::Vec3::new(50.0, 0.0, 0.0),
            glam::Vec3::Z,
            30.0,
        );
        let three = ducad_render::SketchPlane::from_3_points(
            glam::Vec3::new(100.0, 0.0, 0.0),
            glam::Vec3::new(110.0, 0.0, 5.0),
            glam::Vec3::new(100.0, 10.0, 0.0),
        )
        .unwrap();
        for (k, plane) in [angled, three].into_iter().enumerate() {
            let id = app.create_datum_plane(format!("D{k}"), plane);
            app.set_sketch_plane(ducad_render::PlaneKind::Custom(id));
            app.enter_sketching();
            let pts = [
                DVec2::new(0.0, 0.0),
                DVec2::new(10.0, 0.0),
                DVec2::new(10.0, 6.0),
                DVec2::new(0.0, 6.0),
            ];
            let ids: Vec<_> = (0..4)
                .map(|i| {
                    app.sketch_mut()
                        .entities
                        .insert(ducad_sketch::Entity::line(pts[i], pts[(i + 1) % 4]))
                })
                .collect();
            app.selected = ids.iter().copied().collect();
            app.extrude_distance_input = "5".into();
            app.extrude_selected();
            app.exit_sketching();
        }
        assert_eq!(
            oplog_kinds(&app),
            ["sketch", "extrude", "sketch", "extrude"],
            "{:?}",
            app.model_status
        );
        let v = serde_json::to_value(&app.agent_meta.design.oplog[0]).unwrap();
        assert!(v["plane"].get("frame").is_some(), "{}", v["plane"]);
        assert!(
            close(total_volume(&app), 2.0 * 300.0),
            "{}",
            total_volume(&app)
        );
        assert!(close(replay_volume(&app), total_volume(&app)));
    }

    #[test]
    fn gui_emboss_and_deboss_are_ops() {
        for (deboss, sign) in [(false, 1.0), (true, -1.0)] {
            let (mut app, id) = app_with_box("Box", [0.0; 3]);
            let top = ducad_render::SketchPlane::top().offset(20.0);
            let d = app.create_datum_plane("Atas".into(), top);
            app.set_sketch_plane(ducad_render::PlaneKind::Custom(d));
            app.enter_sketching();
            let pts = [
                DVec2::new(8.0, 8.0),
                DVec2::new(12.0, 8.0),
                DVec2::new(12.0, 12.0),
                DVec2::new(8.0, 12.0),
            ];
            for i in 0..4 {
                app.sketch_mut()
                    .entities
                    .insert(ducad_sketch::Entity::line(pts[i], pts[(i + 1) % 4]));
            }
            app.selected.clear();
            app.selected_bodies.insert(id);
            app.apply_emboss_deboss(2.0, deboss);
            assert_eq!(
                oplog_kinds(&app),
                ["sketch", "emboss"],
                "{:?}",
                app.model_status
            );
            assert!(
                close(total_volume(&app), 8000.0 + sign * 32.0),
                "{}",
                total_volume(&app)
            );
            assert!(close(replay_volume(&app), total_volume(&app)));
        }
    }

    /// Urutan 8: teks sketsa (spline glyph eksak + lubang huruf) masuk op
    /// `sketch` lewat ikatan slot; extrude-nya di-replay identik.
    #[test]
    fn gui_text_extrude_round_trips_through_oplog() {
        let mut app = DuCADApp::new_for_test();
        app.set_sketch_plane(ducad_render::PlaneKind::Top);
        app.enter_sketching();
        app.apply_text_to_sketch("AO", DVec2::ZERO, &ducad_sketch::TextOptions::default());
        let ids: Vec<_> = app.sketch().entities.keys().collect();
        assert!(!ids.is_empty(), "{:?}", app.model_status);
        app.selected = ids.into_iter().collect();
        app.extrude_distance_input = "3".into();
        app.extrude_selected();
        assert_eq!(
            oplog_kinds(&app),
            ["sketch", "extrude"],
            "{:?}",
            app.model_status
        );
        assert!(total_volume(&app) > 0.0);
        assert!(close(replay_volume(&app), total_volume(&app)));
    }

    /// Urutan 9: offset face dan replace face (bidang XZ GUI ber-offset)
    /// menjadi op dengan replay identik.
    #[test]
    fn gui_direct_offset_and_replace_face_are_ops() {
        let (mut app, _) = app_with_box("Box", [0.0; 3]);
        app.selected_faces = vec![PickRay {
            origin: (10.0, 10.0, 50.0),
            dir: (0.0, 0.0, -1.0),
        }];
        app.direct_offset_faces(5.0);
        assert_eq!(oplog_kinds(&app), ["offset_face"], "{:?}", app.model_status);
        assert!(close(total_volume(&app), 10000.0), "{}", total_volume(&app));
        assert!(close(replay_volume(&app), total_volume(&app)));

        // Face +Y (y=20) diganti bidang XZ GUI ber-offset 30 (normal +Y).
        app.selected_faces = vec![PickRay {
            origin: (10.0, 50.0, 10.0),
            dir: (0.0, -1.0, 0.0),
        }];
        app.direct_replace_face(crate::direct_tools::PlaneAxis::XZ, 30.0);
        assert_eq!(
            oplog_kinds(&app),
            ["offset_face", "replace_face"],
            "{:?}",
            app.model_status
        );
        assert!(
            close(total_volume(&app), 20.0 * 30.0 * 25.0),
            "{}",
            total_volume(&app)
        );
        assert!(close(replay_volume(&app), total_volume(&app)));
    }

    /// Assembly merujuk body lewat `BodyId`/UUID: replay karena edit
    /// parameter tidak boleh memutus referensinya (ADR 0011).
    #[test]
    fn assembly_instances_survive_param_replay() {
        let mut app = DuCADApp::new_for_test();
        let row = |v: &str| crate::design_tools::ParamRow {
            name: "w".into(),
            orig_name: Some("w".into()),
            value: v.into(),
        };
        app.apply_param_rows(&[crate::design_tools::ParamRow {
            name: "w".into(),
            orig_name: None,
            value: "10".into(),
        }])
        .unwrap();
        let out = app
            .agent_call_local(
                "run_ops",
                serde_json::json!({ "ops": [
                    {"op":"primitive","id":"part","shape":{"box":{"size":["$w",10,10]}}}
                ]}),
            )
            .unwrap();
        assert!(!out.is_error, "{}", out.payload);
        app.sync_assembly_instances();
        let id = only_body(&app);
        let raw = slotmap::Key::data(&id).as_ffi();
        let uuid = app.model.doc.bodies[id].uuid.clone();
        assert!(app
            .assembly_tree
            .instances
            .values()
            .any(|i| i.body_id_raw == raw));

        app.apply_param_rows(&[row("25")]).unwrap();
        let id2 = only_body(&app);
        assert_eq!(id2, id, "BodyId tetap setelah replay");
        assert_eq!(app.model.doc.bodies[id2].uuid, uuid, "UUID tetap");
        assert!(close(volume(&app), 2500.0));
        app.undo_model();
        assert_eq!(only_body(&app), id);
        assert!(close(volume(&app), 1000.0));
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
