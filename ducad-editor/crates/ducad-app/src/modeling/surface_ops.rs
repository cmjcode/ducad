//! Alat permukaan GUI sebagai op oplog `surface` (ADR 0010).
//!
//! Dulu jendela "Permukaan" memanggil `ducad_kernel::surface` langsung lalu
//! mengganti geometri; pengaman edit langsung mencatat hasilnya sebagai op
//! `adopt` (geometri beku) sehingga tidak bisa diedit atau di-replay. Di sini
//! tiap aksi diterjemahkan ke op `surface` yang sama dengan MCP/CLI:
//!
//! - kurva sketsa → op `sketch` (terikat ke slot GUI bila seluruh sketsa
//!   dipakai, atau op sketsa tersendiri berisi entitas terpilih);
//! - tepi/face terpilih → selector semantik (`suggest_edges`/`suggest_faces`);
//! - body terpilih → nama body.
//!
//! `Ok(false)` = belum bisa diterjemahkan (mis. bidang datum tak dikenal)
//! → pemanggil memakai jalur lama.

#![allow(clippy::result_large_err)]

use ducad_core::BodyId;
use ducad_engine::ops::Op;
use ducad_engine::OpError;
use serde_json::{json, Value};

use crate::app::DuCADApp;

fn op_from(v: Value) -> Result<Op, OpError> {
    serde_json::from_value(v).map_err(|e| OpError::invalid(format!("op surface GUI tidak valid: {e}")))
}

impl DuCADApp {
    /// Op `surface` dengan id baru dan isi `make`.
    fn surface_op(&self, make: Value) -> Result<Op, OpError> {
        let id = self.unique_op_id("surface");
        op_from(json!({ "op": "surface", "id": id, "make": make }))
    }

    fn surface_body_name(&self, id: BodyId) -> Option<String> {
        self.model.doc.bodies.get(id).map(|b| b.name.clone())
    }

    /// Jalankan hasil jalur oplog: `true` = sudah ditangani (sukses, atau
    /// galat sudah ditampilkan); `false` = pakai jalur lama.
    pub(crate) fn surface_handled(&mut self, label: &str, r: Result<bool, OpError>) -> bool {
        match self.try_gui_op(label, r) {
            Some(true) => {
                self.model_status = Some(ducad_i18n::t!("direct-created", name = label));
                true
            }
            Some(false) => true,
            None => false,
        }
    }

    /// Op permukaan dari kurva sketsa aktif. `make(sketch_id)` = isi `make`.
    pub(crate) fn surface_sketch_via_oplog(
        &mut self,
        label: &str,
        make: impl Fn(&str) -> Value,
    ) -> Result<bool, OpError> {
        let sid = self.active_sketch_id();
        let Some(slot) = self.sketch_set.get(sid) else {
            return Ok(false);
        };
        let Some(plane) = self.plane_json_for(slot.plane) else {
            return Ok(false);
        };
        let picked: Vec<ducad_sketch::EntityId> = self
            .selected
            .iter()
            .copied()
            .filter(|id| slot.sketch.entities.contains_key(*id))
            .collect();
        let whole = picked.is_empty() || picked.len() == slot.sketch.entities.len();
        let subset = (!whole).then(|| {
            let mut sk = slot.sketch.clone();
            let drop: Vec<_> = sk.entities.keys().filter(|id| !picked.contains(id)).collect();
            for id in drop {
                sk.entities.remove(id);
            }
            sk
        });
        self.sync_agent_meta();
        let details = format!("{label} dari sketsa");
        match subset {
            None => {
                let bound = self.bound_sketch_id(sid);
                let sketch_id = bound.clone().unwrap_or_else(|| self.unique_op_id("sketch"));
                let sketch_op = self
                    .sketch_op_for_slot(sid, &sketch_id)
                    .ok_or_else(|| OpError::invalid("sketsa aktif tidak bisa diterjemahkan ke op"))?;
                let feature = self.surface_op(make(&sketch_id))?;
                self.run_feature_on_sketch(sid, bound, sketch_id, sketch_op, feature, label, &details)?;
            }
            Some(sk) => {
                // Sebagian entitas: op sketsa sendiri (tidak terikat slot).
                let specs = ducad_engine::ops::from_sketch::sketch_to_specs(&sk);
                let sketch_id = self.unique_op_id("surface_sketch");
                let sketch_op = op_from(json!({
                    "op": "sketch", "id": sketch_id, "plane": plane,
                    "entities": specs.entities, "constraints": specs.constraints,
                }))?;
                let feature = self.surface_op(make(&sketch_id))?;
                self.run_gui_ops(vec![sketch_op, feature], label, &details)?;
            }
        }
        Ok(true)
    }

    /// Op permukaan dari tepi `edges` (indeks `enumerate_edges`) body `body`.
    /// `make(nama_body, selector)` = isi `make`.
    pub(crate) fn surface_edges_via_oplog(
        &mut self,
        label: &str,
        body: BodyId,
        edges: &[usize],
        make: impl Fn(&str, &str) -> Value,
    ) -> Result<bool, OpError> {
        self.sync_agent_meta();
        let (Some(name), Some(geo)) = (self.surface_body_name(body), self.model.geometry.get(body)) else {
            return Ok(false);
        };
        let Some(sel) = ducad_engine::select::suggest_edges(&geo.shape, edges) else {
            return Ok(false);
        };
        let op = self.surface_op(make(&name, &sel.selector))?;
        self.run_gui_ops(vec![op], label, label).map(|_| true)
    }

    /// Op permukaan dari face `faces` (indeks `enumerate_faces`) body `body`.
    pub(crate) fn surface_faces_via_oplog(
        &mut self,
        label: &str,
        body: BodyId,
        faces: &[usize],
        make: impl Fn(&str, &str) -> Value,
    ) -> Result<bool, OpError> {
        self.sync_agent_meta();
        let Some(name) = self.surface_body_name(body) else {
            return Ok(false);
        };
        let Some(sel) = self.face_selector(body, faces) else {
            return Ok(false);
        };
        let op = self.surface_op(make(&name, &sel))?;
        self.run_gui_ops(vec![op], label, label).map(|_| true)
    }

    /// Op permukaan dari body `bodies`. `make(nama_body)` = isi `make`.
    pub(crate) fn surface_bodies_via_oplog(
        &mut self,
        label: &str,
        bodies: &[BodyId],
        make: impl Fn(&[String]) -> Value,
    ) -> Result<bool, OpError> {
        self.sync_agent_meta();
        let names: Option<Vec<String>> = bodies.iter().map(|b| self.surface_body_name(*b)).collect();
        let Some(names) = names else {
            return Ok(false);
        };
        let op = self.surface_op(make(&names))?;
        self.run_gui_ops(vec![op], label, label).map(|_| true)
    }

    /// Op permukaan tanpa rujukan (mis. boundary dari titik 3D).
    pub(crate) fn surface_plain_via_oplog(&mut self, label: &str, make: Value) -> Result<bool, OpError> {
        self.sync_agent_meta();
        let op = self.surface_op(make)?;
        self.run_gui_ops(vec![op], label, label).map(|_| true)
    }
}

#[cfg(test)]
mod tests {
    use crate::app::DuCADApp;
    use ducad_sketch::{Entity, InsertEntities};
    use glam::DVec2;

    fn kinds(app: &DuCADApp) -> Vec<String> {
        app.agent_meta.design.oplog.iter().map(|o| o.kind().to_string()).collect()
    }

    fn draw_line(app: &mut DuCADApp) {
        app.execute_sketch_command(Box::new(InsertEntities::new(
            "Line",
            vec![Entity::line(DVec2::new(0.0, 0.0), DVec2::new(20.0, 0.0))],
        )));
    }

    /// Extrude permukaan dari sketsa → op `sketch` + `surface`, bukan
    /// `adopt`; replay penuh berhasil.
    #[test]
    fn surface_extrude_records_surface_op() {
        let mut app = DuCADApp::new_for_test();
        draw_line(&mut app);
        app.surface_tools.end = 10.0;
        app.surface_extrude();
        assert!(!app.error_card.open, "{:?}", app.error_card);
        assert_eq!(kinds(&app), ["sketch", "surface"], "{:?}", app.model_status);
        assert_eq!(app.model.doc.bodies.len(), 1);
        let session = ducad_engine::Session::replay(app.agent_meta.design.clone());
        assert!(session.is_ok(), "{:?}", session.err());
    }

    /// Body permukaan lalu thicken → op `surface` kedua (di tempat); undo
    /// mencabut op itu.
    #[test]
    fn surface_thicken_records_surface_op() {
        let mut app = DuCADApp::new_for_test();
        draw_line(&mut app);
        app.surface_tools.end = 20.0;
        app.surface_extrude();
        let id = app.model.doc.bodies.keys().next().unwrap();
        app.selected_bodies.insert(id);
        app.surface_tools.thickness = 2.0;
        app.surface_thicken();
        assert!(!app.error_card.open, "{:?}", app.error_card);
        assert_eq!(kinds(&app), ["sketch", "surface", "surface"], "{:?}", app.model_status);
        let v = app.model.geometry.values().next().unwrap().shape.volume().abs();
        assert!((v - 20.0 * 20.0 * 2.0).abs() < 1.0, "volume {v}");
        app.undo();
        assert_eq!(kinds(&app), ["sketch", "surface"]);
    }
}
