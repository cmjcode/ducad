//! Oplog engine ikut satu langkah undo dengan geometri GUI (Step 1a,
//! `docs/PLAN_DESIGNER_UX.md`).
//!
//! Geometri GUI di-undo lewat `model_undo`, sedangkan oplog/params hidup di
//! `agent_meta`. Tanpa modul ini, undo GUI setelah op agent/panel hanya
//! memulihkan geometri; sidik jari lalu tidak cocok dan `sync_agent_meta`
//! membuang oplog (`oplog_stale`).
//!
//! Caranya: setiap aksi yang mengubah oplog menitipkan satu
//! [`OplogStateCommand`] ke transaksi undo geometri yang sama. Command itu
//! tidak menyentuh `ModelDoc`; ia hanya menaruh `SessionMeta` sebelum/sesudah
//! ke slot bersama, dan aplikasi mengambilnya setelah undo/redo. Karena
//! command ikut di dalam transaksinya, ia aman terhadap restamp, coalescing,
//! dan batas kedalaman tumpukan.

use std::cell::RefCell;
use std::rc::Rc;

use ducad_core::Command;
use ducad_engine::model::ModelDoc;
use ducad_engine::SessionMeta;

use crate::app::DuCADApp;

/// Keadaan yang dipulihkan undo/redo: meta oplog + isi slot sketsa GUI yang
/// terikat ke op `sketch` (berubah saat param menggerakkan dimensi).
#[derive(Clone)]
pub struct OplogRestore {
    pub meta: SessionMeta,
    pub sketches: Vec<(ducad_sketch::SketchId, ducad_sketch::Sketch)>,
}

/// Slot tempat command menaruh keadaan yang harus dipulihkan.
pub type OplogSlot = Rc<RefCell<Option<OplogRestore>>>;

/// Keadaan oplog sebelum/sesudah satu aksi, dititipkan di transaksi undo
/// geometri.
pub struct OplogStateCommand {
    label: String,
    before: Box<OplogRestore>,
    after: Box<OplogRestore>,
    slot: OplogSlot,
}

impl Command<ModelDoc> for OplogStateCommand {
    fn name(&self) -> &str {
        &self.label
    }
    fn apply(&mut self, _: &mut ModelDoc) {
        *self.slot.borrow_mut() = Some((*self.after).clone());
    }
    fn revert(&mut self, _: &mut ModelDoc) {
        *self.slot.borrow_mut() = Some((*self.before).clone());
    }
}

/// Penanda awal aksi yang mungkin mengubah oplog.
pub(crate) struct OplogMark {
    before: OplogRestore,
    stamp: Option<u64>,
}

/// Salinan meta tanpa hasil simulasi (besar, dan divalidasi ulang lewat
/// tanda tangan geometri saat dipakai).
fn snapshot(meta: &mut SessionMeta) -> SessionMeta {
    let sim = std::mem::take(&mut meta.sim_results);
    let analysis = std::mem::take(&mut meta.analysis_results);
    let copy = meta.clone();
    meta.sim_results = sim;
    meta.analysis_results = analysis;
    copy
}

fn same_oplog_state(a: &SessionMeta, b: &SessionMeta) -> bool {
    a.design.params == b.design.params
        && a.design.oplog == b.design.oplog
        && a.design.suppressed == b.design.suppressed
        && a.design.base_bodies.len() == b.design.base_bodies.len()
        && a.design.active_configuration == b.design.active_configuration
        && a.live_sketches == b.live_sketches
}

fn same_sketches(
    a: &[(ducad_sketch::SketchId, ducad_sketch::Sketch)],
    b: &[(ducad_sketch::SketchId, ducad_sketch::Sketch)],
) -> bool {
    let text = |v: &[(ducad_sketch::SketchId, ducad_sketch::Sketch)]| {
        v.iter()
            .map(|(id, s)| format!("{id:?}{}", serde_json::to_string(s).unwrap_or_default()))
            .collect::<Vec<_>>()
    };
    text(a) == text(b)
}

impl DuCADApp {
    /// Isi slot sketsa GUI yang terikat ke op `sketch`.
    fn bound_sketch_snapshot(&self) -> Vec<(ducad_sketch::SketchId, ducad_sketch::Sketch)> {
        let mut ids: Vec<ducad_sketch::SketchId> =
            self.agent_meta.live_sketches.values().copied().collect();
        ids.sort_by_key(|id| format!("{id:?}"));
        ids.dedup();
        ids.into_iter()
            .filter_map(|id| Some((id, self.sketch_set.sketch(id)?.clone())))
            .collect()
    }

    fn restore_snapshot(&mut self) -> OplogRestore {
        OplogRestore {
            meta: snapshot(&mut self.agent_meta),
            sketches: self.bound_sketch_snapshot(),
        }
    }

    /// Catat keadaan oplog sebelum aksi.
    pub(crate) fn begin_oplog_step(&mut self) -> OplogMark {
        OplogMark {
            before: self.restore_snapshot(),
            stamp: self.model_undo.top_undo_stamp(),
        }
    }

    /// Selesaikan aksi: bila oplog (atau slot sketsa terikat) berubah,
    /// titipkan keadaannya ke transaksi geometri yang baru, atau buat
    /// langkah sendiri bila aksi tidak mengubah geometri (mis. op `sketch`).
    pub(crate) fn end_oplog_step(&mut self, mark: OplogMark, label: &str) {
        let mut after = self.restore_snapshot();
        // Slot yang baru terikat/lepas dalam aksi ini ikut tercatat di kedua sisi.
        for (id, _) in &mark.before.sketches {
            if !after.sketches.iter().any(|(a, _)| a == id) {
                if let Some(sk) = self.sketch_set.sketch(*id) {
                    after.sketches.push((*id, sk.clone()));
                }
            }
        }
        let mut before = mark.before;
        for (id, sk) in &after.sketches {
            if !before.sketches.iter().any(|(b, _)| b == id) {
                before.sketches.push((*id, sk.clone()));
            }
        }
        if same_oplog_state(&before.meta, &after.meta)
            && same_sketches(&before.sketches, &after.sketches)
        {
            return;
        }
        let new_geometry_step = self.model_undo.top_undo_stamp() != mark.stamp;
        let cmd = Box::new(OplogStateCommand {
            label: label.to_string(),
            before: Box::new(before),
            after: Box::new(after),
            slot: self.oplog_slot.clone(),
        });
        if new_geometry_step {
            self.model_undo.attach_to_top(cmd);
        } else {
            // `execute` menerapkan command (slot = after); keadaan itu sudah
            // berlaku, jadi slot langsung dikosongkan.
            self.model_undo.execute(cmd, &mut self.model);
            self.oplog_slot.borrow_mut().take();
        }
    }

    /// Pulihkan oplog (dan slot sketsa terikat) yang ditaruh command saat
    /// undo/redo geometri.
    pub(crate) fn take_restored_oplog(&mut self) {
        let Some(restore) = self.oplog_slot.borrow_mut().take() else {
            return;
        };
        let mut meta = restore.meta;
        meta.sim_results = std::mem::take(&mut self.agent_meta.sim_results);
        meta.analysis_results = std::mem::take(&mut self.agent_meta.analysis_results);
        self.agent_meta = meta;
        for (id, sk) in restore.sketches {
            if let Some(slot) = self.sketch_set.sketch_mut(id) {
                *slot = sk;
            }
        }
        self.sync_design_after_agent();
    }

    /// Undo langkah geometri beserta oplog yang menyertainya.
    pub fn undo_model(&mut self) {
        self.model_undo.undo(&mut self.model);
        self.selected_bodies.clear();
        self.take_restored_oplog();
        self.sync_round_history();
    }

    /// Redo langkah geometri beserta oplog yang menyertainya.
    pub fn redo_model(&mut self) {
        self.model_undo.redo(&mut self.model);
        self.selected_bodies.clear();
        self.take_restored_oplog();
        self.sync_round_history();
    }
}

#[cfg(test)]
mod tests {
    use crate::app::DuCADApp;
    use serde_json::json;

    fn run_ops(app: &mut DuCADApp, ops: serde_json::Value) {
        let out = app
            .agent_call_local("run_ops", json!({ "ops": ops }))
            .unwrap();
        assert!(!out.is_error, "{}", out.payload);
        assert_eq!(out.payload["committed"], json!(true), "{}", out.payload);
    }

    fn plate(app: &mut DuCADApp) {
        run_ops(
            app,
            json!([
              {"op":"sketch","id":"base","plane":"XY","entities":[
                {"rect":{"center":[0,0],"w":60,"h":40,"name":"outline"}}]},
              {"op":"extrude","id":"plate","sketch":"base","distance":8}
            ]),
        );
    }

    fn oplog_ids(app: &DuCADApp) -> Vec<String> {
        app.agent_meta
            .design
            .oplog
            .iter()
            .map(|o| o.id().to_string())
            .collect()
    }

    fn stale(app: &DuCADApp) -> bool {
        app.agent_meta.warnings.iter().any(|w| w == "oplog_stale")
    }

    #[test]
    fn gui_undo_after_ops_restores_oplog_not_stale() {
        let mut app = DuCADApp::new_for_test();
        plate(&mut app);
        run_ops(
            &mut app,
            json!([{"op":"fillet","id":"f1","body":"plate","edges":"|Z","radius":3}]),
        );
        assert_eq!(oplog_ids(&app), ["base", "plate", "f1"]);

        app.undo_model();
        assert_eq!(oplog_ids(&app), ["base", "plate"], "undo mencabut f1");
        app.sync_agent_meta();
        assert!(!stale(&app), "oplog tidak boleh dibuang setelah undo");
        assert_eq!(oplog_ids(&app), ["base", "plate"]);

        app.redo_model();
        assert_eq!(
            oplog_ids(&app),
            ["base", "plate", "f1"],
            "redo mengembalikan f1"
        );
        app.sync_agent_meta();
        assert!(!stale(&app));

        // Op berikutnya setelah undo/redo tetap menambah oplog yang sama.
        run_ops(
            &mut app,
            json!([{"op":"chamfer","id":"c1","body":"plate","edges":"of(<Z)","distance":0.5}]),
        );
        assert_eq!(oplog_ids(&app), ["base", "plate", "f1", "c1"]);
    }

    #[test]
    fn global_undo_follows_oplog_too() {
        let mut app = DuCADApp::new_for_test();
        plate(&mut app);
        run_ops(
            &mut app,
            json!([{"op":"fillet","id":"f1","body":"plate","edges":"|Z","radius":3}]),
        );
        app.undo();
        assert_eq!(oplog_ids(&app), ["base", "plate"]);
        app.redo();
        assert_eq!(oplog_ids(&app), ["base", "plate", "f1"]);
    }

    #[test]
    fn param_edit_is_one_gui_undo_step_with_oplog() {
        let mut app = DuCADApp::new_for_test();
        let out = app
            .agent_call_local("set_params", json!({ "params": { "t": 5 } }))
            .unwrap();
        assert!(!out.is_error, "{}", out.payload);
        run_ops(
            &mut app,
            json!([{"op":"primitive","id":"b","shape":{"box":{"size":[10,10,"$t"]}}}]),
        );
        let out = app
            .agent_call_local("set_params", json!({ "params": { "t": 9 } }))
            .unwrap();
        assert!(!out.is_error, "{}", out.payload);
        assert_eq!(app.agent_meta.design.params["t"], 9.0);
        app.undo_model();
        assert_eq!(
            app.agent_meta.design.params["t"], 5.0,
            "undo memulihkan param"
        );
        app.sync_agent_meta();
        assert!(!stale(&app));
        app.redo_model();
        assert_eq!(app.agent_meta.design.params["t"], 9.0);
    }

    #[test]
    fn sketch_only_op_gets_its_own_undo_step() {
        let mut app = DuCADApp::new_for_test();
        plate(&mut app);
        let steps = app.model_undo.undo_count();
        run_ops(
            &mut app,
            json!([{"op":"sketch","id":"s2","plane":"XY","entities":[
              {"circle":{"center":[0,0],"r":5,"name":"c"}}]}]),
        );
        assert_eq!(app.model_undo.undo_count(), steps + 1);
        app.undo_model();
        assert_eq!(oplog_ids(&app), ["base", "plate"]);
    }

    /// Undo saat loft masih di-stage: pratinjau dibatalkan dulu (body
    /// pratinjau dibuang), bukan ikut terseret ke keadaan undo.
    #[test]
    fn undo_cancels_staged_loft_preview_first() {
        let mut app = DuCADApp::new_for_test();
        plate(&mut app);
        let preview = app.model.doc.add_body("loft-preview");
        app.loft_staged_body_id = Some(preview);
        app.undo();
        assert!(app.loft_staged_body_id.is_none());
        assert!(app.model.doc.bodies.get(preview).is_none(), "body pratinjau dibuang");
        // `plate` = satu batch `run_ops` = satu langkah undo.
        assert!(oplog_ids(&app).is_empty(), "undo tetap berjalan");
    }
}
