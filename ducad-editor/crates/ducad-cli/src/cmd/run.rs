use std::path::PathBuf;

use ducad_engine::export::{export, ExportFormat};
use ducad_engine::ops::OpFile;
use ducad_engine::Session;

use super::{apply_params, parse_param, print_json, write_json};
use crate::{CliError, CliResult, Exit};

#[derive(clap::Args)]
pub struct Args {
    /// File OpFile JSON (`{ "params": {...}, "ops": [...] }`).
    ops: PathBuf,
    /// Part awal; tanpa ini sesi dimulai kosong.
    #[arg(long)]
    part: Option<PathBuf>,
    /// Simpan hasil ke `.ducad`.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Timpa param (`K=V`), boleh diulang.
    #[arg(long = "param", value_parser = parse_param)]
    params: Vec<(String, f64)>,
    /// Validasi tanpa mengubah apa pun.
    #[arg(long)]
    dry_run: bool,
    /// Ekspor `FMT:PATH` (step|stl|obj|glb|svg|png; svg/png tampak iso), boleh diulang.
    #[arg(long = "export", value_parser = parse_export)]
    exports: Vec<(String, PathBuf)>,
    /// Tulis BatchReport juga ke berkas ini.
    #[arg(long)]
    report: Option<PathBuf>,
}

fn parse_export(s: &str) -> Result<(String, PathBuf), String> {
    let (fmt, path) = s
        .split_once(':')
        .ok_or_else(|| format!("ekspor '{s}' harus berbentuk FMT:PATH"))?;
    let fmt = fmt.to_ascii_lowercase();
    if !["step", "stl", "obj", "glb", "svg", "png"].contains(&fmt.as_str()) {
        return Err(format!("format ekspor '{fmt}' tidak dikenal"));
    }
    Ok((fmt, PathBuf::from(path)))
}

pub fn exec(a: Args) -> CliResult {
    let text = std::fs::read_to_string(&a.ops)
        .map_err(|e| CliError::usage(format!("gagal membaca {}: {e}", a.ops.display())))?;
    let file: OpFile = serde_json::from_str(&text).map_err(|e| {
        CliError::usage(format!("{} bukan OpFile yang valid: {e}", a.ops.display()))
    })?;
    let mut session = match &a.part {
        Some(p) => super::open_part(p)?,
        None => Session::new(),
    };
    if let Some(report) = apply_params(&mut session, file.params, &a.params)? {
        if !report.committed {
            print_json(&report)?;
            if let Some(p) = &a.report {
                write_json(p, &report)?;
            }
            return Ok(Exit::OpFailed);
        }
    }
    if !file.checks.is_empty() {
        session.set_checks(file.checks);
    }
    if !file.drawings.is_empty() {
        session.set_drawings(file.drawings);
    }
    let mut report = session.run(file.ops, a.dry_run);
    if report.committed {
        // Varian dari berkas ops dipasang setelah op-nya ada di oplog.
        if let Some(r) = session
            .apply_configuration_specs(&file.configurations, file.active_configuration.as_deref())?
        {
            if !r.committed {
                report = r;
            }
        }
    }
    print_json(&report)?;
    if let Some(p) = &a.report {
        write_json(p, &report)?;
    }
    if report.error.is_some() {
        return Ok(Exit::OpFailed);
    }
    if a.dry_run {
        return Ok(Exit::Ok);
    }
    if let Some(out) = &a.out {
        session.save(out)?;
    }
    for (fmt, path) in &a.exports {
        match fmt.as_str() {
            "svg" | "png" => super::render::write_view(
                &session,
                ducad_engine::render::View::Iso,
                fmt == "png",
                path,
                800,
                600,
                false,
                None,
            )?,
            _ => {
                let f: ExportFormat = fmt.parse()?;
                export(&session, f, path)?;
            }
        }
    }
    Ok(Exit::Ok)
}
