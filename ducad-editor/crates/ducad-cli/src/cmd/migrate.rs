//! `ducad-cli migrate-file` — migrasi berkas `.ducad` ke format v4
//! (rencana migrasi kernel F1.3, lihat `docs/FORMAT.md`).
//!
//! Tanpa `--out`/`--in-place` hanya melapor (dry run). Body yang terbukti
//! direproduksi oplog kehilangan STEP-nya dan mendapat cache B-rep; sisanya
//! tidak disentuh. Setiap hasil diverifikasi ulang sebelum ditulis.

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::print_json;
use crate::{CliError, CliResult, Exit};

#[derive(clap::Args)]
pub struct Args {
    /// Berkas `.ducad` yang dimigrasi.
    #[arg(required = true)]
    files: Vec<PathBuf>,
    /// Tulis hasil ke berkas ini (hanya untuk satu masukan).
    #[arg(long, conflicts_with = "in_place")]
    out: Option<PathBuf>,
    /// Timpa berkas asal; salinan lama disimpan sebagai `<nama>.ducad.bak`.
    #[arg(long)]
    in_place: bool,
}

#[derive(Serialize)]
struct FileReport {
    file: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    written: Option<PathBuf>,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    report: Option<ducad_engine::migrate::MigrateReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

pub fn exec(a: Args) -> CliResult {
    if a.out.is_some() && a.files.len() != 1 {
        return Err(CliError::usage("--out hanya untuk satu berkas masukan"));
    }
    let mut failed = false;
    let mut out = Vec::with_capacity(a.files.len());
    for file in &a.files {
        let r = migrate_one(file, a.out.as_deref(), a.in_place);
        failed |= r.error.is_some();
        out.push(r);
    }
    print_json(&out)?;
    Ok(if failed { Exit::OpFailed } else { Exit::Ok })
}

fn migrate_one(file: &Path, out: Option<&Path>, in_place: bool) -> FileReport {
    let mut fr = FileReport {
        file: file.to_path_buf(),
        written: None,
        report: None,
        error: None,
    };
    let result = (|| -> Result<(), String> {
        let json = std::fs::read_to_string(file)
            .map_err(|e| format!("gagal membaca {}: {e}", file.display()))?;
        let (text, report) =
            ducad_engine::migrate::migrate_json(&json).map_err(|e| e.message)?;
        let changed = report.changed;
        fr.report = Some(report);
        let target = match (out, in_place) {
            (Some(p), _) => Some(p.to_path_buf()),
            (None, true) if changed => {
                let bak = PathBuf::from(format!("{}.bak", file.display()));
                std::fs::copy(file, &bak)
                    .map_err(|e| format!("gagal membuat cadangan {}: {e}", bak.display()))?;
                Some(file.to_path_buf())
            }
            _ => None,
        };
        if let Some(target) = target {
            std::fs::write(&target, text)
                .map_err(|e| format!("gagal menulis {}: {e}", target.display()))?;
            fr.written = Some(target);
        }
        Ok(())
    })();
    fr.error = result.err();
    fr
}
