use std::path::PathBuf;

use ducad_engine::check::CheckItem;

use super::{open_part, print_json};
use crate::{CliError, CliResult, Exit};

#[derive(clap::Args)]
pub struct Args {
    part: PathBuf,
    /// Larik CheckItem, atau objek dengan kunci "checks" (format tugas eval).
    /// Tanpa ini dipakai `design.checks` part.
    #[arg(long)]
    checks: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}

fn read_checks(path: &PathBuf) -> Result<Vec<CheckItem>, CliError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| CliError::usage(format!("gagal membaca {}: {e}", path.display())))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| CliError::usage(format!("{} bukan JSON: {e}", path.display())))?;
    let list = match value {
        serde_json::Value::Object(mut o) => o.remove("checks").ok_or_else(|| {
            CliError::usage(format!("{} tidak punya kunci \"checks\"", path.display()))
        })?,
        other => other,
    };
    serde_json::from_value(list)
        .map_err(|e| CliError::usage(format!("daftar check tidak valid: {e}")))
}

pub fn exec(a: Args) -> CliResult {
    let s = open_part(&a.part)?;
    let explicit = a.checks.as_ref().map(read_checks).transpose()?;
    let checks = explicit.as_deref().unwrap_or(&s.design().checks);
    if checks.is_empty() {
        return Err(CliError::usage(
            "tidak ada check (isi --checks atau design.checks)",
        ));
    }
    let summary = s.run_checks(Some(checks));
    if a.json {
        print_json(&summary)?;
    } else {
        for r in &summary.results {
            let mark = match r.status {
                ducad_engine::check::CheckStatus::Pass => "✓",
                ducad_engine::check::CheckStatus::Fail => "✗",
                ducad_engine::check::CheckStatus::Error => "!",
            };
            let label = r.id.clone().unwrap_or_else(|| r.kind.to_string());
            println!("{mark} {label}: {}", r.message);
        }
        println!(
            "lulus {} · gagal {} · error {}",
            summary.pass, summary.fail, summary.error
        );
    }
    Ok(if summary.fail + summary.error == 0 {
        Exit::Ok
    } else {
        Exit::ChecksFailed
    })
}
