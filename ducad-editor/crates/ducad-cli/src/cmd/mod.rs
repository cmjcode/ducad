pub mod assist;
pub mod build;
pub mod chat;
pub mod check;
pub mod config;
pub mod diff;
pub mod export;
pub mod golden;
pub mod inspect;
pub mod migrate;
pub mod oplog;
pub mod release;
pub mod render;
pub mod replay;
pub mod run;
pub mod select;
pub mod sim;

use std::path::Path;

use ducad_engine::ops::Params;
use ducad_engine::Session;
use serde::Serialize;

use crate::CliError;

/// Print `value` as pretty-formatted JSON to stdout.
pub fn print_json(value: &impl Serialize) -> Result<(), CliError> {
    let text = serde_json::to_string_pretty(value).map_err(|e| {
        CliError::usage(if crate::is_id() {
            format!("gagal serialisasi JSON: {e}")
        } else {
            format!("failed to serialize JSON: {e}")
        })
    })?;
    println!("{text}");
    Ok(())
}

/// Write `value` as pretty-formatted JSON to a file.
pub fn write_json(path: &Path, value: &impl Serialize) -> Result<(), CliError> {
    let text = serde_json::to_string_pretty(value).map_err(|e| {
        CliError::usage(if crate::is_id() {
            format!("gagal serialisasi JSON: {e}")
        } else {
            format!("failed to serialize JSON: {e}")
        })
    })?;
    std::fs::write(path, text + "\n").map_err(|e| {
        CliError::usage(if crate::is_id() {
            format!("gagal menulis {}: {e}", path.display())
        } else {
            format!("failed to write {}: {e}", path.display())
        })
    })
}

/// Parse `--param K=V`.
pub fn parse_param(s: &str) -> Result<(String, f64), String> {
    let (k, v) = s.split_once('=').ok_or_else(|| {
        if crate::is_id() {
            format!("param '{s}' harus berbentuk K=V")
        } else {
            format!("param '{s}' must be K=V")
        }
    })?;
    let v: f64 = v.trim().parse().map_err(|_| {
        if crate::is_id() {
            format!("nilai param '{k}' bukan angka: '{v}'")
        } else {
            format!("param '{k}' value is not a number: '{v}'")
        }
    })?;
    if k.trim().is_empty() {
        return Err(if crate::is_id() {
            format!("nama param kosong di '{s}'")
        } else {
            format!("empty param name in '{s}'")
        });
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
