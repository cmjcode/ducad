//! `ducad-cli assist` (P11.4): minta asisten AI di perangkat mengusulkan
//! perubahan. Hasilnya proposal; `--accept` menerapkannya lalu menyimpan.

use std::path::PathBuf;

use ducad_assist::{assist, AssistBackend, DEFAULT_MAX_ITERS};
use ducad_engine::ops::OpFile;
use ducad_engine::Session;

use super::print_json;
use crate::{CliError, CliResult, Exit};

#[derive(clap::Args)]
pub struct Args {
    /// Part `.ducad` atau OpFile `.json`.
    part: PathBuf,
    /// `apple` (Foundation Models, macOS 26+) atau `local` (GGUF).
    #[arg(long, default_value = "apple")]
    backend: String,
    #[arg(long)]
    instruction: String,
    /// Terapkan proposal lalu simpan ke `--out`.
    #[arg(long)]
    accept: bool,
    #[arg(long)]
    out: Option<PathBuf>,
    /// Berkas teks, satu pelajaran per baris (maks 5 dipakai).
    #[arg(long)]
    lessons: Option<PathBuf>,
    #[arg(long, default_value_t = DEFAULT_MAX_ITERS)]
    max_iters: usize,
    /// Backend `local`: berkas GGUF.
    #[arg(long)]
    model: Option<PathBuf>,
    /// Backend `local`: `tokenizer.json`.
    #[arg(long)]
    tokenizer: Option<PathBuf>,
    /// Sertakan transkrip prompt/balasan di keluaran JSON.
    #[arg(long)]
    transcript: bool,
}

fn load(path: &PathBuf) -> Result<Session, CliError> {
    let is_json = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("json"));
    if !is_json {
        return super::open_part(path);
    }
    let text = std::fs::read_to_string(path)
        .map_err(|e| CliError::usage(format!("gagal membaca {}: {e}", path.display())))?;
    let file: OpFile = serde_json::from_str(&text)
        .map_err(|e| CliError::usage(format!("{} bukan OpFile yang valid: {e}", path.display())))?;
    let mut s = Session::new();
    let r = s.set_params(file.params)?;
    if let Some(e) = r.error {
        return Err(e.into());
    }
    if !file.checks.is_empty() {
        s.set_checks(file.checks);
    }
    let r = s.run(file.ops, false);
    match r.error {
        Some(e) => Err(e.into()),
        None => Ok(s),
    }
}

#[allow(unused_variables)]
fn backend(a: &Args) -> Result<Box<dyn AssistBackend>, CliError> {
    match a.backend.as_str() {
        #[cfg(feature = "apple-fm")]
        "apple" => ducad_assist::apple::AppleFoundation::detect()
            .map(|b| Box::new(b) as Box<dyn AssistBackend>)
            .ok_or_else(|| {
                CliError::usage(format!(
                    "Apple Foundation Models tidak tersedia di perangkat ini ({})",
                    ducad_assist::apple::status()
                ))
            }),
        #[cfg(feature = "local-gguf")]
        "local" => {
            let model = a
                .model
                .clone()
                .ok_or_else(|| CliError::usage("backend local butuh --model <GGUF>"))?;
            let tokenizer = a
                .tokenizer
                .clone()
                .ok_or_else(|| CliError::usage("backend local butuh --tokenizer <tokenizer.json>"))?;
            ducad_assist::gguf::LocalGguf::load(&model, &tokenizer)
                .map(|b| Box::new(b) as Box<dyn AssistBackend>)
                .map_err(|e| CliError::usage(format!("gagal memuat model: {e:#}")))
        }
        other => Err(CliError::usage(format!(
            "backend '{other}' tidak tersedia di build ini (bangun ducad-cli dengan fitur apple-fm / local-gguf)"
        ))),
    }
}

pub fn exec(a: Args) -> CliResult {
    if a.accept && a.out.is_none() {
        return Err(CliError::usage("--accept butuh --out OUT.ducad"));
    }
    let lessons: Vec<String> = match &a.lessons {
        Some(p) => std::fs::read_to_string(p)
            .map_err(|e| CliError::usage(format!("gagal membaca {}: {e}", p.display())))?
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect(),
        None => Vec::new(),
    };
    let mut session = load(&a.part)?;
    let mut b = backend(&a)?;
    let started = std::time::Instant::now();
    let outcome = assist(&mut session, b.as_mut(), &a.instruction, &lessons, a.max_iters)?;
    let secs = started.elapsed().as_secs_f64();

    let mut json = serde_json::to_value(&outcome)
        .map_err(|e| CliError::usage(format!("gagal serialisasi: {e}")))?;
    if let Some(obj) = json.as_object_mut() {
        if !a.transcript {
            obj.remove("transcript");
        }
        obj.insert("backend".into(), b.name().into());
        obj.insert("seconds".into(), serde_json::json!((secs * 100.0).round() / 100.0));
    }

    let mut exit = if outcome.proposal.is_some() || outcome.reply.message().is_some() {
        Exit::Ok
    } else {
        Exit::OpFailed
    };
    if a.accept {
        match &outcome.proposal {
            Some(p) => {
                let report = session.accept(&p.id);
                if let Some(obj) = json.as_object_mut() {
                    obj.insert("accepted".into(), report.committed.into());
                }
                if report.committed {
                    if let Some(out) = &a.out {
                        session.save(out)?;
                    }
                } else {
                    exit = Exit::OpFailed;
                }
            }
            None => exit = Exit::OpFailed,
        }
    }
    print_json(&json)?;
    Ok(exit)
}
