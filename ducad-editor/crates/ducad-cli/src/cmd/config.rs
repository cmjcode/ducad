use std::path::PathBuf;

use super::{open_part, print_json};
use crate::{CliError, CliResult, Exit};

#[derive(clap::Args)]
pub struct Args {
    part: PathBuf,
    /// Tulis design table CSV (baris = konfigurasi, kolom = parameter).
    #[arg(long)]
    export: Option<PathBuf>,
    /// Ganti konfigurasi part dari design table CSV (butuh --out).
    #[arg(long, conflicts_with = "export")]
    import: Option<PathBuf>,
    /// Aktifkan konfigurasi ini sebelum menyimpan (butuh --out).
    #[arg(long)]
    activate: Option<String>,
    /// Berkas `.ducad` hasil (untuk --import / --activate).
    #[arg(long)]
    out: Option<PathBuf>,
    /// Keluaran JSON (default: teks ringkas).
    #[arg(long)]
    json: bool,
}

pub fn exec(a: Args) -> CliResult {
    let mut s = open_part(&a.part)?;
    let mut changed = false;
    if let Some(path) = &a.import {
        let csv = std::fs::read_to_string(path)
            .map_err(|e| CliError::usage(format!("gagal membaca {}: {e}", path.display())))?;
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
            CliError::usage("--import/--activate mengubah part: isi --out BERKAS.ducad")
        })?;
        s.save(out)?;
    }
    if let Some(path) = &a.export {
        std::fs::write(path, s.design_table_csv())
            .map_err(|e| CliError::usage(format!("gagal menulis {}: {e}", path.display())))?;
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
            format!("  tanpa: {}", c.suppressed_ops.join(", "))
        };
        println!("{mark} {:<20} {}{suppressed}", c.name, params.join(" "));
    }
    Ok(Exit::Ok)
}
