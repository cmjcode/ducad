use std::path::PathBuf;

use ducad_engine::export::{export, ExportFormat};

use super::{open_part, print_json};
use crate::{CliResult, Exit};

#[derive(clap::Args)]
pub struct Args {
    part: PathBuf,
    /// step | stl | obj | glb
    #[arg(long)]
    format: String,
    #[arg(long)]
    out: PathBuf,
}

pub fn exec(a: Args) -> CliResult {
    let fmt: ExportFormat = a.format.parse()?;
    let s = open_part(&a.part)?;
    let bytes = export(&s, fmt, &a.out)?;
    print_json(&serde_json::json!({ "path": a.out, "bytes": bytes }))?;
    Ok(Exit::Ok)
}
