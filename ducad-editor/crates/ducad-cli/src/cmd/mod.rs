pub mod assist;
pub mod build;
pub mod check;
pub mod diff;
pub mod export;
pub mod inspect;
pub mod oplog;
pub mod render;
pub mod replay;
pub mod run;
pub mod select;

use std::path::Path;

use ducad_engine::ops::Params;
use ducad_engine::Session;
use serde::Serialize;

use crate::CliError;

/// Cetak `value` sebagai JSON rapi ke stdout.
pub fn print_json(value: &impl Serialize) -> Result<(), CliError> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|e| CliError::usage(format!("gagal serialisasi JSON: {e}")))?;
    println!("{text}");
    Ok(())
}

/// Tulis `value` sebagai JSON rapi ke berkas.
pub fn write_json(path: &Path, value: &impl Serialize) -> Result<(), CliError> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|e| CliError::usage(format!("gagal serialisasi JSON: {e}")))?;
    std::fs::write(path, text + "\n")
        .map_err(|e| CliError::usage(format!("gagal menulis {}: {e}", path.display())))
}

/// Urai `--param K=V`.
pub fn parse_param(s: &str) -> Result<(String, f64), String> {
    let (k, v) = s
        .split_once('=')
        .ok_or_else(|| format!("param '{s}' harus berbentuk K=V"))?;
    let v: f64 = v
        .trim()
        .parse()
        .map_err(|_| format!("nilai param '{k}' bukan angka: '{v}'"))?;
    if k.trim().is_empty() {
        return Err(format!("nama param kosong di '{s}'"));
    }
    Ok((k.trim().to_string(), v))
}

/// Gabungkan `overrides` ke params sesi lalu replay. Laporan gagal → kode 1.
pub fn apply_params(
    s: &mut Session,
    base: Params,
    overrides: &[(String, f64)],
) -> Result<Option<ducad_engine::BatchReport>, CliError> {
    if overrides.is_empty() && base == s.design().params {
        return Ok(None);
    }
    let mut params = s.design().params.clone();
    params.extend(base);
    params.extend(overrides.iter().cloned());
    let report = s.set_params(params)?;
    Ok(Some(report))
}

/// Muat part `.ducad`.
pub fn open_part(path: &Path) -> Result<Session, CliError> {
    Ok(Session::from_file(path)?)
}
