//! Kartu proposal + ghost preview (P8.4 di GUI).
//!
//! Agent mengirim `propose_ops` lewat jembatan live (P5); batch dijalankan
//! lalu DIBATALKAN, dan yang tersisa hanyalah geometri selisihnya: mesh
//! `added` digambar hijau tembus pandang, `removed` merah tembus pandang,
//! lewat jalur mesh scene yang sudah ada. Balasan ke agent baru dikirim
//! setelah pengguna menekan Terima/Tolak — atau setelah batas waktu
//! (`rejected: "timeout"`). Ini pagar keselamatan: pada sesi live, agent
//! TIDAK punya `accept_proposal`.

use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use ducad_engine::ops::Op;
use ducad_kernel::KernelMesh;
use serde_json::{json, Value};

use crate::agent_bridge::{AgentBridge, REPLY_TIMEOUT_SECS};
use crate::app::DuCADApp;

/// Warna ghost (sama dengan diff SVG P8.3).
pub const GHOST_ADDED: [f32; 4] = [0.086, 0.639, 0.290, 0.45];
pub const GHOST_REMOVED: [f32; 4] = [0.863, 0.149, 0.149, 0.45];

/// Proposal yang sedang ditampilkan dan menunggu keputusan pengguna.
pub struct ProposalView {
    pub id: String,
    pub added: Vec<std::sync::Arc<KernelMesh>>,
    pub removed: Vec<std::sync::Arc<KernelMesh>>,
    /// Ringkasan volume untuk kartu.
    pub summary: String,
    /// Satu baris per op (`id (jenis)`).
    pub ops_labels: Vec<String>,
    /// Op yang dijalankan bila diterima.
    pub ops: Vec<Op>,
    /// Edit oplog (params/ganti/hapus): sesi hasil replay + laporannya,
    /// diadopsi utuh bila diterima. `None` = hanya menambah `ops`.
    pub edited: Option<Box<(ducad_engine::Session, ducad_engine::BatchReport)>>,
    /// Sidik jari model saat proposal dibuat.
    pub base_fingerprint: String,
    /// Tujuan balasan agent: `(id permintaan, pengirim)`.
    pub reply: Option<(u64, Sender<Value>)>,
    pub deadline: Instant,
}

impl ProposalView {
    fn expired(&self) -> bool {
        Instant::now() >= self.deadline
    }

    /// Kirim balasan penolakan sekali; `reason` = `"user"` atau `"timeout"`.
    fn reply_rejected(&mut self, reason: &str) {
        if let Some((id, tx)) = self.reply.take() {
            let payload = json!({ "proposal_id": self.id, "rejected": reason });
            let _ = tx.send(AgentBridge::ok_reply(id, payload, None, false));
        }
    }
}

impl DuCADApp {
    /// `propose_ops` dari jembatan: jalankan batch lalu batalkan, simpan
    /// ghost + pengirim balasan. Balasan menyusul setelah pengguna memilih.
    #[allow(clippy::result_large_err)]
    pub(crate) fn agent_propose(
        &mut self,
        params: Value,
        req_id: u64,
        reply: &Sender<Value>,
    ) -> ducad_engine::OpResult<()> {
        let e = ducad_engine::tooling::parse_edit_args("propose_ops", params)?;
        self.agent_propose_edit(e, req_id, reply)
    }

    /// Proposal dari argumen edit yang sudah divalidasi: tambah op di akhir
    /// (batch lalu batalkan), atau edit oplog (params/ganti/hapus) yang
    /// di-replay pada sesi salinan. Model GUI tidak berubah sampai diterima.
    #[allow(clippy::result_large_err)]
    pub(crate) fn agent_propose_edit(
        &mut self,
        e: ducad_engine::tooling::EditArgs,
        req_id: u64,
        reply: &Sender<Value>,
    ) -> ducad_engine::OpResult<()> {
        let mut labels: Vec<String> = Vec::new();
        if let Some(p) = &e.params {
            let mut keys: Vec<String> = p.iter().map(|(k, v)| format!("{k} = {v}")).collect();
            keys.sort();
            labels.push(format!("params: {}", keys.join(", ")));
        }
        labels.extend(
            e.replace
                .iter()
                .map(|r| format!("~ {} ({})", r.id, r.op.kind())),
        );
        labels.extend(e.remove.iter().map(|id| format!("− {id}")));
        labels.extend(
            e.append
                .iter()
                .map(|o| format!("{} ({})", o.id(), o.kind())),
        );

        let (diff, shapes, base_fingerprint, edited) = if e.is_append_only() {
            let mut core = ducad_engine::SessionCore {
                model: &mut self.model,
                model_undo: &mut self.model_undo,
                sketches: &mut self.sketch_set,
                meta: &mut self.agent_meta,
            };
            let (_, diff, shapes, fp) = core.propose(e.append.clone())?;
            (diff, shapes, fp, None)
        } else {
            let params = e.params.map(|p| {
                let mut merged = self.agent_meta.design.params.clone();
                merged.extend(p);
                merged
            });
            let preview = ducad_engine::preview_edit(
                &self.model,
                &self.agent_meta.design,
                params.as_ref(),
                &e.replace,
                &e.remove,
                &e.append,
            )?;
            let fp = ducad_engine::session::fingerprint(&self.model);
            let mut report = preview.report;
            report.committed = true;
            (
                preview.diff,
                preview.shapes,
                fp,
                Some(Box::new((preview.session, report))),
            )
        };
        let ops = e.append;

        let tess = |v: &[ducad_kernel::KernelShape]| -> Vec<std::sync::Arc<KernelMesh>> {
            v.iter()
                .map(|s| std::sync::Arc::new(s.tessellate()))
                .collect()
        };
        let added_volume: f64 = diff.iter().filter_map(|d| d.added_volume).sum();
        let removed_volume: f64 = diff.iter().filter_map(|d| d.removed_volume).sum();
        let (added_txt, removed_txt) =
            (format!("{added_volume:.1}"), format!("{removed_volume:.1}"));
        let view = ProposalView {
            id: format!("live-{req_id}"),
            added: tess(&shapes.added),
            removed: tess(&shapes.removed),
            summary: ducad_i18n::t!(
                "proposal-volume",
                added = added_txt.as_str(),
                removed = removed_txt.as_str()
            ),
            ops_labels: labels,
            ops,
            edited,
            base_fingerprint,
            reply: Some((req_id, reply.clone())),
            deadline: Instant::now() + Duration::from_secs(REPLY_TIMEOUT_SECS),
        };
        // Proposal lama yang belum dijawab ditolak lebih dulu supaya klien
        // tidak menggantung.
        if let Some(mut old) = self.pending_proposal.take() {
            old.reply_rejected("superseded");
        }
        self.pending_proposal = Some(view);
        Ok(())
    }

    /// Kartu melayang + batas waktu. Dipanggil tiap frame.
    pub fn proposal_frame(&mut self, ctx: &egui::Context) {
        let Some(p) = &self.pending_proposal else {
            return;
        };
        if p.expired() {
            if let Some(mut p) = self.pending_proposal.take() {
                p.reply_rejected("timeout");
            }
            self.model_status = Some(ducad_i18n::t!("proposal-timeout"));
            return;
        }
        ctx.request_repaint_after(Duration::from_millis(500));
        let state = ducad_ui::ProposalCardState {
            title: ducad_i18n::t!("proposal-title"),
            summary: p.summary.clone(),
            ops: p.ops_labels.clone(),
            accept: ducad_i18n::t!("proposal-accept"),
            reject: ducad_i18n::t!("proposal-reject"),
        };
        match ducad_ui::ProposalCard::show(ctx, &state) {
            Some(ducad_ui::ProposalCardEvent::Accept) => self.accept_pending_proposal(),
            Some(ducad_ui::ProposalCardEvent::Reject) => {
                if let Some(mut p) = self.pending_proposal.take() {
                    p.reply_rejected("user");
                }
                self.model_status = Some(ducad_i18n::t!("proposal-rejected"));
            }
            None => {}
        }
    }

    /// Terima proposal: jalankan op-nya sungguhan di atas state GUI.
    /// Model berubah sejak proposal dibuat → `proposal_stale`.
    pub fn accept_pending_proposal(&mut self) {
        let Some(mut p) = self.pending_proposal.take() else {
            return;
        };
        let fp = ducad_engine::session::fingerprint(&self.model);
        if fp != p.base_fingerprint {
            if let Some((id, tx)) = p.reply.take() {
                let e = ducad_engine::OpError::new(
                    ducad_engine::OpErrorCode::ProposalStale,
                    format!("model berubah sejak proposal '{}' dibuat", p.id),
                )
                .with_hint("buat proposal baru dengan propose_ops");
                let _ = tx.send(AgentBridge::err_reply(id, &e));
            }
            self.model_status = Some(ducad_i18n::t!("proposal-rejected"));
            return;
        }
        if let Some(edited) = p.edited.take() {
            // Edit oplog: adopsi sesi hasil replay sebagai satu langkah undo.
            let (session, report) = *edited;
            let payload = ducad_engine::tooling::to_value(report).unwrap_or(Value::Null);
            let meta = session.meta().clone();
            self.adopt_agent_model(session.into_model(), meta, &p.id);
            if let Some((id, tx)) = p.reply.take() {
                let _ = tx.send(AgentBridge::ok_reply(id, payload, None, false));
            }
            self.model_status = Some(ducad_i18n::t!("proposal-accepted"));
            return;
        }
        let report = {
            let mut core = ducad_engine::SessionCore {
                model: &mut self.model,
                model_undo: &mut self.model_undo,
                sketches: &mut self.sketch_set,
                meta: &mut self.agent_meta,
            };
            core.run(p.ops.clone(), false)
        };
        let committed = report.committed;
        let payload = ducad_engine::tooling::to_value(report).unwrap_or(Value::Null);
        if let Some((id, tx)) = p.reply.take() {
            let _ = tx.send(AgentBridge::ok_reply(id, payload, None, !committed));
        }
        if committed {
            let count = p.ops.len().to_string();
            self.sync_design_after_agent();
            let title = ducad_i18n::t!("bridge-activity", count = count.as_str());
            self.after_model_changed(&title, &p.id);
            self.model_status = Some(ducad_i18n::t!("proposal-accepted"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::mpsc;

    /// Bangun app dengan satu balok lewat jembatan, tanpa soket.
    fn app_with_box() -> DuCADApp {
        let mut app = DuCADApp::new_for_test();
        let _tx = app.bridge.test_channel();
        let (reply, _rx) = mpsc::channel();
        let ops = json!({ "ops": [
            { "op": "primitive", "id": "blok", "shape": { "box": { "size": [20, 20, 10] } } }
        ] });
        let out = app
            .agent_call_for_test("run_ops", ops, 1, &reply)
            .expect("balasan langsung");
        assert!(!out.is_error, "{}", out.payload);
        app
    }

    #[test]
    fn propose_does_not_change_model_until_accepted() {
        let mut app = app_with_box();
        let before = app.model.doc.bodies.len();
        let (reply, rx) = mpsc::channel();
        let ops = json!({ "ops": [
            { "op": "primitive", "id": "blok2", "shape": { "box": { "size": [5, 5, 5] } }, "at": [50, 0, 0] }
        ] });
        app.agent_propose(ops, 9, &reply).expect("proposal");

        // Model belum berubah; ghost tersimpan dan balasan belum dikirim.
        assert_eq!(app.model.doc.bodies.len(), before);
        let p = app.pending_proposal.as_ref().expect("ada proposal");
        assert_eq!(p.added.len(), 1);
        assert!(rx.try_recv().is_err(), "balasan menunggu keputusan user");

        app.accept_pending_proposal();
        assert_eq!(app.model.doc.bodies.len(), before + 1);
        let r = rx.try_recv().expect("balasan setelah diterima");
        assert_eq!(r["is_error"], false, "{r}");
        assert_eq!(r["payload"]["committed"], true, "{r}");
        assert!(app.pending_proposal.is_none());
    }

    #[test]
    fn reject_replies_without_touching_model() {
        let mut app = app_with_box();
        let before = app.model.doc.bodies.len();
        let (reply, rx) = mpsc::channel();
        let ops = json!({ "ops": [
            { "op": "primitive", "id": "blok3", "shape": { "sphere": { "r": 3 } }, "at": [50, 0, 0] }
        ] });
        app.agent_propose(ops, 10, &reply).expect("proposal");
        let mut p = app.pending_proposal.take().expect("ada proposal");
        p.reply_rejected("user");
        assert_eq!(app.model.doc.bodies.len(), before);
        let r = rx.try_recv().expect("balasan penolakan");
        assert_eq!(r["payload"]["rejected"], "user");
    }

    #[test]
    fn accept_after_model_changed_is_stale() {
        let mut app = app_with_box();
        let (reply, rx) = mpsc::channel();
        let ops = json!({ "ops": [
            { "op": "primitive", "id": "blok4", "shape": { "sphere": { "r": 3 } }, "at": [50, 0, 0] }
        ] });
        app.agent_propose(ops, 11, &reply).expect("proposal");

        // Pengguna meng-undo sementara proposal menunggu.
        app.model_undo.undo(&mut app.model);
        app.accept_pending_proposal();
        let r = rx.try_recv().expect("balasan stale");
        assert_eq!(r["is_error"], true, "{r}");
        assert_eq!(r["payload"]["error"]["code"], "proposal_stale", "{r}");
    }

    /// App dengan contoh plate (params + 4 op) lewat jembatan.
    fn app_with_plate() -> DuCADApp {
        let mut app = DuCADApp::new_for_test();
        let _tx = app.bridge.test_channel();
        let (reply, _rx) = mpsc::channel();
        let plate: Value = serde_json::from_str(ducad_engine::ops::EXAMPLE_PLATE).unwrap();
        for (method, params) in [
            ("set_params", json!({ "params": plate["params"] })),
            ("run_ops", json!({ "ops": plate["ops"] })),
        ] {
            let out = app
                .agent_call_for_test(method, params, 1, &reply)
                .expect("balasan langsung");
            assert!(!out.is_error, "{method}: {}", out.payload);
        }
        app
    }

    fn live_volume(app: &mut DuCADApp) -> f64 {
        let (reply, _rx) = mpsc::channel();
        let out = app
            .agent_call_for_test("inspect", json!({}), 2, &reply)
            .expect("inspect");
        out.payload["bodies"][0]["volume"]
            .as_f64()
            .unwrap_or_default()
    }

    #[test]
    fn replace_op_on_live_document_is_one_undo_step() {
        let mut app = app_with_plate();
        let v0 = live_volume(&mut app);
        let (reply, _rx) = mpsc::channel();
        let fillet = json!({"op":"fillet","id":"f1","body":"plate","edges":"|Z","radius":1});
        let out = app
            .agent_call_for_test(
                "replace_op",
                json!({ "id": "f1", "op": fillet.clone(), "dry_run": true }),
                3,
                &reply,
            )
            .expect("balasan");
        assert_eq!(out.payload["committed"], false, "{}", out.payload);
        assert_eq!(live_volume(&mut app), v0, "dry run tidak mengubah dokumen");

        let out = app
            .agent_call_for_test("replace_op", json!({ "id": "f1", "op": fillet }), 4, &reply)
            .expect("balasan");
        assert!(!out.is_error, "{}", out.payload);
        let v1 = live_volume(&mut app);
        assert!(v1 > v0, "fillet lebih kecil → volume bertambah");

        app.model_undo.undo(&mut app.model);
        assert!(
            (live_volume(&mut app) - v0).abs() < 1e-6,
            "satu undo GUI mengembalikan"
        );

        let out = app
            .agent_call_for_test("remove_op", json!({ "ids": ["nope"] }), 5, &reply)
            .expect("balasan");
        assert_eq!(
            out.payload["error"]["code"], "unknown_ref",
            "{}",
            out.payload
        );
    }

    #[test]
    fn force_propose_turns_remove_op_into_edit_proposal() {
        let mut app = app_with_plate();
        app.bridge.force_propose = true;
        let v0 = live_volume(&mut app);
        let (reply, rx) = mpsc::channel();
        let out = app.agent_call_for_test("remove_op", json!({ "ids": ["h1"] }), 6, &reply);
        assert!(out.is_none(), "balasan menunggu keputusan pengguna");
        let p = app.pending_proposal.as_ref().expect("ada proposal");
        assert!(p.edited.is_some());
        assert!(
            p.ops_labels.iter().any(|l| l.contains("h1")),
            "{:?}",
            p.ops_labels
        );
        assert_eq!(live_volume(&mut app), v0, "model belum berubah");

        app.accept_pending_proposal();
        let r = rx.try_recv().expect("balasan setelah diterima");
        assert_eq!(r["payload"]["committed"], true, "{r}");
        assert!(
            live_volume(&mut app) > v0,
            "lubang hilang → volume bertambah"
        );
        assert!(app.agent_meta.design.oplog.iter().all(|o| o.id() != "h1"));
    }

    #[test]
    fn set_checks_is_saved_into_document_design() {
        let mut app = app_with_plate();
        let (reply, _rx) = mpsc::channel();
        let out = app
            .agent_call_for_test(
                "set_checks",
                json!({ "checks": [{"check": "hole_count", "body": "*", "diameter": 5.5, "expect": 4}] }),
                7,
                &reply,
            )
            .expect("balasan");
        assert_eq!(out.payload["pass"], 1, "{}", out.payload);
        let design = app.design.clone().expect("design tersimpan");
        assert_eq!(
            design["checks"].as_array().map(|a| a.len()),
            Some(1),
            "{design}"
        );
    }
}
