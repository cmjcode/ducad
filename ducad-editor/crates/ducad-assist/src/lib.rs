//! Asisten AI lokal/offline DUCAD (P11).
//!
//! Model bahasa (di perangkat) membaca ringkasan part lalu mengusulkan
//! aksi (`set_params`, `append_ops`, `replace_op`, `explain`, `ask_user`).
//! Aksi perubahan SELALU menjadi proposal (`Session::propose_edit`) —
//! crate ini tidak pernah meng-commit perubahan; pengguna/pemanggil yang
//! memutuskan `accept`. Error op dan check gagal diumpankan balik ke model
//! (maks `max_iters` putaran).

// `OpError` sengaja kaya konteks (sama dengan ducad-engine).
#![allow(clippy::result_large_err)]

pub mod backend;
pub mod prompt;

#[cfg(feature = "apple-fm")]
pub mod apple;
#[cfg(feature = "local-gguf")]
pub mod gguf;

use ducad_engine::check::{CheckResult, CheckStatus};
use ducad_engine::ops::{Op, Params};
use ducad_engine::{OpError, OpErrorCode, OpResult, Proposal, ReplaceOp, Session};
use serde::{Deserialize, Serialize};

pub use backend::{AssistBackend, MockBackend};

/// Parameter generasi backend lokal (P11.2).
pub const TEMPERATURE: f64 = 0.2;
pub const TOP_P: f64 = 0.9;
pub const SEED: u64 = 299_792_458;
pub const MAX_TOKENS: usize = 768;
/// Putaran default loop `assist`.
pub const DEFAULT_MAX_ITERS: usize = 3;

/// Batas kemampuan yang dinyatakan di UI (P11.4).
pub const CAPABILITY_NOTE: &str = "Model di perangkat cocok untuk mengubah ukuran dan menambah fitur sederhana. Untuk membuat part baru yang rumit, pakai agent eksternal.";

/// Satu aksi usulan model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssistAction {
    SetParams(Params),
    AppendOps(Vec<Op>),
    ReplaceOp { id: String, op: Box<Op> },
    Explain(String),
    AskUser(String),
}

/// Balasan model: `{rationale, actions}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistReply {
    #[serde(default)]
    pub rationale: String,
    #[serde(default)]
    pub actions: Vec<AssistAction>,
}

impl AssistReply {
    /// Teks `explain`/`ask_user` pertama (mengakhiri loop tanpa proposal).
    pub fn message(&self) -> Option<&str> {
        self.actions.iter().find_map(|a| match a {
            AssistAction::Explain(t) | AssistAction::AskUser(t) => Some(t.as_str()),
            _ => None,
        })
    }
}

/// Hasil `assist`.
#[derive(Debug, Clone, Serialize)]
pub struct AssistOutcome {
    /// Balasan terakhir yang terurai.
    pub reply: AssistReply,
    /// Proposal terakhir yang berhasil (belum diterapkan).
    pub proposal: Option<Proposal>,
    pub iterations: usize,
    /// Pasangan (pesan user, balasan mentah) per panggilan backend.
    pub transcript: Vec<(String, String)>,
    /// Error terakhir bila tidak ada proposal yang berhasil.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<OpError>,
}

/// Kegagalan mengurai balasan model.
#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    /// Bukan JSON sama sekali → satu kali permintaan ulang.
    NotJson(String),
    /// JSON valid tapi aksi/op tidak sesuai kontrak → diumpankan sebagai error.
    BadShape(String),
}

/// Ambil substring dari `{` pertama sampai `}` terakhir lalu urai.
pub fn parse_reply(text: &str) -> Result<AssistReply, ParseError> {
    let (Some(start), Some(end)) = (text.find('{'), text.rfind('}')) else {
        return Err(ParseError::NotJson("tidak ada objek JSON".into()));
    };
    if end < start {
        return Err(ParseError::NotJson("tidak ada objek JSON".into()));
    }
    // Aturan P11.2: `{` pertama s.d. `}` terakhir. Model kecil kadang
    // menulis DUA objek berturut-turut; bila rentang itu bukan JSON, pakai
    // objek utuh pertama.
    let value: serde_json::Value = match serde_json::from_str(&text[start..=end]) {
        Ok(v) => v,
        Err(e) => serde_json::Deserializer::from_str(&text[start..])
            .into_iter::<serde_json::Value>()
            .next()
            .and_then(Result::ok)
            .filter(serde_json::Value::is_object)
            .ok_or_else(|| ParseError::NotJson(e.to_string()))?,
    };
    serde_json::from_value(value.clone()).map_err(|e| ParseError::BadShape(shape_error(&value, e)))
}

/// Pesan bentuk-salah yang menunjuk aksi pertama yang tidak valid — pesan
/// serde mentah ("invalid type: sequence") terlalu kabur bagi model kecil.
fn shape_error(value: &serde_json::Value, whole: serde_json::Error) -> String {
    let Some(actions) = value.get("actions").and_then(|a| a.as_array()) else {
        return format!("objek harus punya larik \"actions\": {whole}");
    };
    for (i, a) in actions.iter().enumerate() {
        if let Err(e) = serde_json::from_value::<AssistAction>(a.clone()) {
            let mut snippet = a.to_string();
            if snippet.len() > 160 {
                let cut = (0..=160).rev().find(|k| snippet.is_char_boundary(*k)).unwrap_or(0);
                snippet.truncate(cut);
                snippet.push('…');
            }
            return format!(
                "aksi #{i} {snippet} tidak valid: {e}. Kunci aksi yang sah: set_params, append_ops, replace_op, explain, ask_user; op ditulis di dalam append_ops"
            );
        }
    }
    whole.to_string()
}

fn backend_err(name: &str, e: anyhow::Error) -> OpError {
    OpError::new(OpErrorCode::Io, format!("backend AI '{name}' gagal: {e:#}"))
}

/// Satu panggilan model + satu kali ulang bila balasan bukan JSON. Hasil
/// dalam `Ok(Err(_))` = JSON dengan bentuk salah (diumpankan ke model).
fn ask(
    backend: &mut dyn AssistBackend,
    system: &str,
    user: &str,
    transcript: &mut Vec<(String, String)>,
) -> OpResult<Result<AssistReply, String>> {
    let name = backend.name().to_string();
    let raw = backend
        .complete(system, user, MAX_TOKENS)
        .map_err(|e| backend_err(&name, e))?;
    log::debug!("balasan {name}: {raw}");
    transcript.push((user.to_string(), raw.clone()));
    let first = match parse_reply(&raw) {
        Ok(r) => return Ok(Ok(r)),
        Err(ParseError::BadShape(e)) => return Ok(Err(e)),
        Err(ParseError::NotJson(e)) => e,
    };
    let retry = format!(
        "{user}\n\nBalasan sebelumnya bukan JSON valid ({first}). Balas HANYA satu objek JSON lengkap berkunci rationale dan actions, seperti CONTOH di prompt sistem."
    );
    let raw = backend
        .complete(system, &retry, MAX_TOKENS)
        .map_err(|e| backend_err(&name, e))?;
    log::debug!("balasan ulang {name}: {raw}");
    transcript.push((retry, raw.clone()));
    match parse_reply(&raw) {
        Ok(r) => Ok(Ok(r)),
        Err(ParseError::BadShape(e)) => Ok(Err(e)),
        Err(ParseError::NotJson(e)) => Err(OpError::new(
            OpErrorCode::InvalidParam,
            format!("balasan model bukan JSON valid setelah 1 kali ulang: {e}"),
        )
        .with_hint("coba ulang dengan instruksi yang lebih singkat, atau pakai model lain")),
    }
}

/// Gabungkan aksi perubahan menjadi (params, penggantian, op tambahan).
fn collect_edit(
    session: &Session,
    actions: &[AssistAction],
) -> (Option<Params>, Vec<ReplaceOp>, Vec<Op>) {
    let mut params: Option<Params> = None;
    let mut replace = Vec::new();
    let mut append = Vec::new();
    for a in actions {
        match a {
            AssistAction::SetParams(p) => {
                let base = params.get_or_insert_with(|| session.design().params.clone());
                base.extend(p.iter().map(|(k, v)| (k.clone(), *v)));
            }
            AssistAction::AppendOps(ops) => append.extend(ops.iter().cloned()),
            AssistAction::ReplaceOp { id, op } => replace.push(ReplaceOp {
                id: id.clone(),
                op: (**op).clone(),
            }),
            AssistAction::Explain(_) | AssistAction::AskUser(_) => {}
        }
    }
    (params, replace, append)
}

fn failed_checks(p: &Proposal) -> Vec<CheckResult> {
    p.report
        .checks
        .iter()
        .flatten()
        .filter(|c| c.status != CheckStatus::Pass)
        .cloned()
        .collect()
}

/// Jalankan asisten: minta aksi ke `backend`, ubah menjadi proposal, umpankan
/// error/check gagal, maks `max_iters` putaran. Tidak pernah meng-commit.
pub fn assist(
    session: &mut Session,
    backend: &mut dyn AssistBackend,
    instruction: &str,
    lessons: &[String],
    max_iters: usize,
) -> OpResult<AssistOutcome> {
    let system = prompt::SYSTEM_PROMPT;
    let mut transcript = Vec::new();
    let mut last_error: Option<OpError> = None;
    let mut checks_feedback: Vec<CheckResult> = Vec::new();
    let mut checks_retry_used = false;
    let mut best: Option<Proposal> = None;
    let mut reply = AssistReply {
        rationale: String::new(),
        actions: Vec::new(),
    };
    let mut iterations = 0;

    while iterations < max_iters.max(1) {
        iterations += 1;
        let user = prompt::user_message(
            session,
            &prompt::Context {
                instruction,
                last_error: last_error.as_ref(),
                failed_checks: &checks_feedback,
                lessons,
            },
        );
        reply = match ask(backend, system, &user, &mut transcript)? {
            Ok(r) => r,
            Err(shape) => {
                last_error = Some(OpError::invalid(format!(
                    "aksi balasan tidak sesuai kontrak: {shape}"
                )));
                continue;
            }
        };
        if reply.message().is_some() {
            break;
        }
        let (params, replace, append) = collect_edit(session, &reply.actions);
        if params.is_none() && replace.is_empty() && append.is_empty() {
            last_error = Some(OpError::invalid(
                "balasan tidak berisi aksi; kirim minimal satu aksi",
            ));
            continue;
        }
        match session.propose_edit(params, replace, append) {
            Err(e) => last_error = Some(e),
            Ok((p, _)) => {
                last_error = None;
                let failed = failed_checks(&p);
                if let Some(old) = best.replace(p) {
                    session.reject(&old.id);
                }
                if failed.is_empty() || checks_retry_used {
                    break;
                }
                checks_retry_used = true;
                checks_feedback = failed;
            }
        }
    }

    // Error hanya relevan bila tidak ada proposal sama sekali.
    if best.is_some() {
        last_error = None;
    }
    Ok(AssistOutcome {
        reply,
        proposal: best,
        iterations,
        transcript,
        last_error,
    })
}
