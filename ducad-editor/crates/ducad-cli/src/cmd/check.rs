use std::path::PathBuf;

use ducad_engine::check::CheckItem;

use super::{open_part, print_json};
use crate::{CliError, CliResult, Exit};

#[derive(clap::Args)]
pub struct Args {
    part: PathBuf,
    /// JSON array of CheckItem, or an object with a `"checks"` key (eval task format).
    /// Defaults to the part's `design.checks`.
    #[arg(long)]
    checks: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}

fn read_checks(path: &PathBuf) -> Result<Vec<CheckItem>, CliError> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        CliError::usage(if crate::is_id() {
            format!("gagal membaca {}: {e}", path.display())
        } else {
            format!("failed to read {}: {e}", path.display())
        })
    })?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        CliError::usage(if crate::is_id() {
            format!("{} bukan JSON: {e}", path.display())
        } else {
            format!("{} is not valid JSON: {e}", path.display())
        })
    })?;
    let list = match value {
        serde_json::Value::Object(mut o) => o.remove("checks").ok_or_else(|| {
            CliError::usage(if crate::is_id() {
                format!("{} tidak punya kunci \"checks\"", path.display())
            } else {
                format!("{} is missing key \"checks\"", path.display())
            })
        })?,
        other => other,
    };
    serde_json::from_value(list).map_err(|e| {
        CliError::usage(if crate::is_id() {
            format!("daftar check tidak valid: {e}")
        } else {
            format!("invalid checks list: {e}")
        })
    })
}

pub fn exec(a: Args) -> CliResult {
    let s = open_part(&a.part)?;
    let explicit = a.checks.as_ref().map(read_checks).transpose()?;
    let checks = explicit.as_deref().unwrap_or(&s.design().checks);
    if checks.is_empty() {
        return Err(CliError::usage(crate::tr(
            "no checks defined (pass --checks or populate design.checks)",
            "tidak ada check (isi --checks atau design.checks)",
        )));
    }
    let summary = s.run_checks(Some(checks));
    if a.json {
        print_json(&summary)?;
    } else {
        for r in &summary.results {
            let mark = match (r.status, r.severity) {
                (ducad_engine::check::CheckStatus::Pass, _) => "✓",
                (_, ducad_engine::check::Severity::Warning) => "~",
                (ducad_engine::check::CheckStatus::Fail, _) => "✗",
                (ducad_engine::check::CheckStatus::Error, _) => "!",
            };
            let label = r.id.clone().unwrap_or_else(|| r.kind.to_string());
            println!("{mark} {label}: {}", r.message);
            if let Some(citation) = &r.citation {
                println!("    {}: {citation}", crate::tr("basis", "dasar"));
            }
        }
        if crate::is_id() {
            println!(
                "lulus {} · gagal {} · error {} · peringatan {}",
                summary.pass, summary.fail, summary.error, summary.warn
            );
        } else {
            println!(
                "passed {} · failed {} · error {} · warnings {}",
                summary.pass, summary.fail, summary.error, summary.warn
            );
        }
    }
    Ok(if !summary.is_blocking() {
        Exit::Ok
    } else {
        Exit::ChecksFailed
    })
}
