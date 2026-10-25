//! Asisten AI di GUI (P11.4). Model bekerja di thread latar atas SALINAN
//! `design` (oplog parametrik), bukan atas state GUI: `KernelShape` tidak
//! `Send`, jadi sesi dibangun ulang di dalam thread dari `DesignDoc` dan
//! yang dikirim balik hanya usulan berbentuk data. Usulan baru diterapkan
//! setelah pengguna menekan "Terapkan".

use std::sync::mpsc;

use ducad_assist::{AssistBackend, AssistOutcome, DEFAULT_MAX_ITERS};
use ducad_engine::ops::{Op, Params};
use ducad_engine::{DesignDoc, ReplaceOp, Session};
use ducad_ui::{AssistDialog, AssistDialogEvent, AssistDialogState};

use crate::app::DuCADApp;

/// Kebijakan privasi AI (P11.4). `OfflineOnly` = hanya backend yang
/// `is_on_device()`; jembatan agent eksternal dimatikan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AiPrivacy {
    #[default]
    OfflineOnly,
    AllowExternal,
}

/// Usulan yang bisa dipindahkan antar-thread (tanpa geometri).
#[derive(Debug, Clone, Default)]
pub struct AssistEdit {
    pub params: Option<Params>,
    pub replace: Vec<ReplaceOp>,
    pub append: Vec<Op>,
}

/// Pesan dari thread latar.
pub struct AssistMsg {
    pub rationale: String,
    pub message: String,
    pub changes: Vec<String>,
    pub edit: Option<AssistEdit>,
}

#[derive(Default)]
pub struct AiState {
    pub privacy: AiPrivacy,
    pub dialog: AssistDialogState,
    pub rx: Option<mpsc::Receiver<AssistMsg>>,
    /// Usulan yang menunggu keputusan pengguna.
    pub pending: Option<AssistEdit>,
}

/// Backend di perangkat sesuai fitur build; `None` bila tidak ada.
fn on_device_backend() -> Option<Box<dyn AssistBackend>> {
    #[cfg(feature = "apple-fm")]
    {
        if let Some(b) = ducad_assist::apple::AppleFoundation::detect() {
            return Some(Box::new(b) as Box<dyn AssistBackend>);
        }
    }
    None
}

/// Nama backend aktif untuk chip/dialog (kosong bila tidak ada).
pub fn backend_name() -> String {
    on_device_backend()
        .map(|b| b.name().to_string())
        .unwrap_or_default()
}

/// Ringkasan perubahan per baris untuk dialog.
fn changes_of(edit: &AssistEdit, before: &Params) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(p) = &edit.params {
        for (k, v) in p {
            match before.get(k) {
                Some(old) if (old - v).abs() < 1e-9 => {}
                Some(old) => out.push(format!("{k}: {old} » {v}")),
                None => out.push(format!("{k}: {v} (baru)")),
            }
        }
    }
    for r in &edit.replace {
        out.push(format!("ganti op {} ({})", r.id, r.op.kind()));
    }
    for op in &edit.append {
        out.push(format!("tambah op {} ({})", op.id(), op.kind()));
    }
    out
}

fn edit_from(outcome: &AssistOutcome) -> Option<AssistEdit> {
    let p = outcome.proposal.as_ref()?;
    Some(AssistEdit {
        params: p.params.clone(),
        replace: p.replace.clone(),
        append: p.ops.clone(),
    })
}

impl DuCADApp {
    /// Vault memori, dibuka saat pertama dipakai (P11.5). `None` = build
    /// tanpa fitur `memory`, atau vault gagal dibuka.
    #[cfg(feature = "memory")]
    fn memory(&mut self) -> Option<std::sync::Arc<crate::memory::VaultMemory>> {
        if self.memory.is_none() {
            match crate::memory::VaultMemory::open(crate::memory::default_root()) {
                Ok(m) => self.memory = Some(std::sync::Arc::new(m)),
                Err(e) => {
                    log::warn!("memori: vault tidak bisa dibuka: {e:#}");
                    return None;
                }
            }
        }
        self.memory.clone()
    }

    /// Pelajaran relevan dari vault untuk diumpankan ke model (P11.2).
    fn lessons_for(&mut self, instruction: &str) -> Vec<String> {
        #[cfg(feature = "memory")]
        {
            match self.memory() {
                Some(m) => m.recall(instruction, 5),
                None => Vec::new(),
            }
        }
        #[cfg(not(feature = "memory"))]
        {
            let _ = instruction;
            Vec::new()
        }
    }

    /// Catat satu baris ke log sesi vault; diam saja bila memori mati.
    fn note_session(&mut self, text: &str) {
        #[cfg(feature = "memory")]
        {
            if let Some(m) = self.memory() {
                if let Err(e) = m.append_session(text) {
                    log::warn!("memori: log sesi gagal ditulis: {e:#}");
                }
            }
        }
        #[cfg(not(feature = "memory"))]
        {
            let _ = text;
        }
    }

    fn design_for_assist(&self) -> Option<DesignDoc> {
        serde_json::from_value(self.design.clone()?).ok()
    }

    /// Buka dialog "Tanya AI…".
    pub fn open_assist_dialog(&mut self) {
        self.ai.dialog.open = true;
        self.ai.dialog.backend = backend_name();
        self.ai.dialog.on_device = true;
        if self.design_for_assist().is_none() {
            self.ai.dialog.message = ducad_i18n::t!("assist-needs-design");
        }
    }

    /// Mulai kerja model di thread latar.
    fn start_assist(&mut self) {
        let Some(design) = self.design_for_assist() else {
            self.ai.dialog.message = ducad_i18n::t!("assist-needs-design");
            return;
        };
        let Some(mut backend) = on_device_backend() else {
            self.ai.dialog.message = ducad_i18n::t!("assist-no-backend");
            return;
        };
        let instruction = self.ai.dialog.instruction.trim().to_string();
        let lessons = self.lessons_for(&instruction);
        let before = design.params.clone();
        let (tx, rx) = mpsc::channel();
        self.ai.rx = Some(rx);
        self.ai.pending = None;
        self.ai.dialog.busy = true;
        self.ai.dialog.rationale.clear();
        self.ai.dialog.changes.clear();
        self.ai.dialog.message.clear();
        self.ai.dialog.has_proposal = false;
        std::thread::spawn(move || {
            let fail = |message: String| AssistMsg {
                rationale: String::new(),
                message,
                changes: Vec::new(),
                edit: None,
            };
            let msg = match Session::replay(design) {
                Err(e) => fail(e.message),
                Ok(mut session) => match ducad_assist::assist(
                    &mut session,
                    backend.as_mut(),
                    &instruction,
                    &lessons,
                    DEFAULT_MAX_ITERS,
                ) {
                    Err(e) => fail(e.message),
                    Ok(outcome) => {
                        let edit = edit_from(&outcome);
                        let changes = edit
                            .as_ref()
                            .map(|e| changes_of(e, &before))
                            .unwrap_or_default();
                        AssistMsg {
                            rationale: outcome.reply.rationale.clone(),
                            message: outcome
                                .reply
                                .message()
                                .map(str::to_string)
                                .or_else(|| outcome.last_error.as_ref().map(|e| e.message.clone()))
                                .unwrap_or_default(),
                            changes,
                            edit,
                        }
                    }
                },
            };
            let _ = tx.send(msg);
        });
    }

    /// Terapkan usulan: replay design + edit, lalu ganti model GUI.
    fn apply_assist(&mut self) {
        let (Some(edit), Some(design)) = (self.ai.pending.take(), self.design_for_assist()) else {
            return;
        };
        let mut session = match Session::replay(design) {
            Ok(s) => s,
            Err(e) => {
                self.ai.dialog.message = e.message;
                return;
            }
        };
        let proposal = match session.propose_edit(edit.params, edit.replace, edit.append) {
            Ok((p, _)) => p,
            Err(e) => {
                self.ai.dialog.message = e.message;
                return;
            }
        };
        let report = session.accept(&proposal.id);
        if !report.committed {
            self.ai.dialog.message = report
                .error
                .map(|e| e.message)
                .unwrap_or_else(|| "usulan gagal diterapkan".into());
            return;
        }
        self.adopt_session(session);
        self.ai.dialog.has_proposal = false;
        self.ai.dialog.message = ducad_i18n::t!("assist-applied");
        let instruction = self.ai.dialog.instruction.trim().to_string();
        self.note_session(&format!("Usulan AI diterapkan: {instruction}"));
    }

    /// Ganti model GUI dengan hasil sesi engine lewat jalur oplog bersama
    /// (`adopt_replayed_session`): satu langkah undo yang ikut memulihkan
    /// oplog/params, `agent_meta` dan `design` tetap sinkron sehingga
    /// `sync_agent_meta` berikutnya tidak membuang oplog.
    fn adopt_session(&mut self, session: Session) {
        self.adopt_replayed_session(session, "assist");
        self.selected_bodies.clear();
        let rationale = self.ai.dialog.rationale.clone();
        self.record_activity(
            ducad_ui::ActivityKindUi::Solid3D,
            &ducad_i18n::t!("assist-title"),
            &rationale,
        );
    }

    /// Dipanggil tiap frame: terima hasil latar lalu render dialog.
    pub fn assist_frame(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.ai.rx {
            if let Ok(msg) = rx.try_recv() {
                self.ai.dialog.busy = false;
                self.ai.dialog.rationale = msg.rationale;
                self.ai.dialog.changes = msg.changes;
                self.ai.dialog.message = msg.message;
                self.ai.dialog.has_proposal = msg.edit.is_some();
                self.ai.pending = msg.edit;
                self.ai.rx = None;
            }
        }
        if self.ai.dialog.busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
        match AssistDialog::show(ctx, &mut self.ai.dialog) {
            Some(AssistDialogEvent::Ask) => self.start_assist(),
            Some(AssistDialogEvent::Apply) => self.apply_assist(),
            Some(AssistDialogEvent::Reject) => {
                self.ai.pending = None;
                self.ai.dialog.has_proposal = false;
                self.ai.dialog.message = ducad_i18n::t!("assist-rejected");
            }
            Some(AssistDialogEvent::Cancel) => {
                // Thread latar dibiarkan selesai sendiri; hasilnya diabaikan.
                self.ai.rx = None;
                self.ai.dialog.busy = false;
            }
            Some(AssistDialogEvent::Close) | None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn stale(app: &DuCADApp) -> bool {
        app.agent_meta.warnings.iter().any(|w| w == "oplog_stale")
    }

    /// Usulan AI yang diterapkan harus bisa di-undo (geometri + oplog) dan
    /// tidak membuat oplog dibuang pada `sync_agent_meta` berikutnya.
    #[test]
    fn applied_assist_edit_is_undoable_and_keeps_oplog() {
        let mut app = DuCADApp::new_for_test();
        let out = app
            .agent_call_local("set_params", json!({ "params": { "t": 5 } }))
            .unwrap();
        assert!(!out.is_error, "{}", out.payload);
        let out = app
            .agent_call_local(
                "run_ops",
                json!({ "ops": [{"op":"primitive","id":"b","shape":{"box":{"size":[10,10,"$t"]}}}] }),
            )
            .unwrap();
        assert!(!out.is_error, "{}", out.payload);
        let id = app.model.doc.bodies.keys().next().unwrap();
        let v0 = app.model.geometry[id].shape.volume();
        let steps = app.model_undo.undo_count();

        let mut params = Params::default();
        params.insert("t".into(), 9.0);
        app.ai.pending = Some(AssistEdit { params: Some(params), replace: Vec::new(), append: Vec::new() });
        app.apply_assist();
        assert_eq!(app.agent_meta.design.params["t"], 9.0);
        let id = app.model.doc.bodies.keys().next().unwrap();
        assert!((app.model.geometry[id].shape.volume() - 900.0).abs() < 1e-6);
        assert_eq!(app.model_undo.undo_count(), steps + 1, "satu langkah undo");
        let design: DesignDoc = serde_json::from_value(app.design.clone().unwrap()).unwrap();
        assert_eq!(design.params["t"], 9.0, "`design` ikut diperbarui");
        app.sync_agent_meta();
        assert!(!stale(&app));

        app.undo_model();
        assert_eq!(app.agent_meta.design.params["t"], 5.0, "undo memulihkan params");
        let id = app.model.doc.bodies.keys().next().unwrap();
        assert!((app.model.geometry[id].shape.volume() - v0).abs() < 1e-6, "undo memulihkan geometri");
        assert_eq!(app.agent_meta.design.oplog.len(), 1);
        app.sync_agent_meta();
        assert!(!stale(&app), "oplog tidak boleh dibuang setelah undo usulan AI");
    }
}
