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
use crate::model::ModelDoc;

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
                Some(old) => out.push(format!("{k}: {old} → {v}")),
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
                    &[],
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
        let design = serde_json::to_value(session.design()).ok();
        self.adopt_session(session.into_model(), design);
        self.ai.dialog.has_proposal = false;
        self.ai.dialog.message = ducad_i18n::t!("assist-applied");
    }

    /// Ganti model GUI dengan hasil sesi engine (body + `design`).
    fn adopt_session(&mut self, model: ModelDoc, design: Option<serde_json::Value>) {
        self.model = model;
        self.model_undo = ducad_core::UndoStack::default();
        self.selected_bodies.clear();
        self.design = design;
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
