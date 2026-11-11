use std::path::PathBuf;

use super::{open_part, print_json};
use crate::{CliError, CliResult, Exit};

#[derive(clap::Args)]
pub struct Args {
    part: PathBuf,
    /// Write CSV design table (rows = configurations, columns = parameters).
    #[arg(long)]
    export: Option<PathBuf>,
    /// Replace part configurations from a CSV design table (requires `--out`).
    #[arg(long, conflicts_with = "export")]
    import: Option<PathBuf>,
    /// Activate this configuration before saving (requires `--out`).
    #[arg(long)]
    activate: Option<String>,
    /// Output `.ducad` file (required for `--import` / `--activate`).
    #[arg(long)]
    out: Option<PathBuf>,
    /// Output JSON instead of concise human-readable text.
    #[arg(long)]
    json: bool,
}

pub fn exec(a: Args) -> CliResult {
    let mut s = open_part(&a.part)?;
    let mut changed = false;
    if let Some(path) = &a.import {
        let csv = std::fs::read_to_string(path).map_err(|e| {
            CliError::usage(if crate::is_id() {
                format!("gagal membaca {}: {e}", path.display())
            } else {
                format!("failed to read {}: {e}", path.display())
            })
        })?;
        let report = s.import_design_table(&csv)?;
        if let Some(e) = report.error {
            return Err(e.into());
        }
        changed = true;
    }
    if let Some(name) = &a.activate {
        let report = s.activate_configuration(name)?;
        if let Some(e) = report.error {
            eprintln!("ducad-cli: {}", e.message);
            return Ok(Exit::OpFailed);
        }
        changed = true;
    }
    if changed {
        let out = a.out.as_ref().ok_or_else(|| {
            CliError::usage(crate::tr(
                "--import/--activate modifies the part: pass --out FILE.ducad",
                "--import/--activate mengubah part: isi --out BERKAS.ducad",
            ))
        })?;
        s.save(out)?;
    }
    if let Some(path) = &a.export {
        std::fs::write(path, s.design_table_csv()).map_err(|e| {
            CliError::usage(if crate::is_id() {
                format!("gagal menulis {}: {e}", path.display())
            } else {
                format!("failed to write {}: {e}", path.display())
            })
        })?;
    }
    let active = s.active_configuration().to_string();
    let configs = s.configurations();
    if a.json {
        print_json(&serde_json::json!({ "active": active, "configurations": configs }))?;
        return Ok(Exit::Ok);
    }
    for c in &configs {
        let mark = if c.name == active { "*" } else { " " };
        let params: Vec<String> = c.params.iter().map(|(k, v)| format!("{k}={v}")).collect();
        let suppressed = if c.suppressed_ops.is_empty() {
            String::new()
        } else {
            format!(
                "  {}: {}",
                crate::tr("suppressed", "tanpa"),
                c.suppressed_ops.join(", ")
            )
        };
        println!("{mark} {:<20} {}{suppressed}", c.name, params.join(" "));
    }
    Ok(Exit::Ok)
}
