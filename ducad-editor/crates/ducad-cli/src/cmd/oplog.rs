use std::path::PathBuf;

use super::open_part;
use crate::{CliError, CliResult, Exit};

#[derive(clap::Args)]
pub struct Args {
    part: PathBuf,
    /// Tulis ke berkas (default stdout). Dipakai juga sebagai textconv git.
    #[arg(long)]
    out: Option<PathBuf>,
}

pub fn exec(a: Args) -> CliResult {
    let s = open_part(&a.part)?;
    let text = ducad_engine::oplog::to_git_text(s.design());
    match &a.out {
        Some(p) => std::fs::write(p, text)
            .map_err(|e| CliError::usage(format!("gagal menulis {}: {e}", p.display())))?,
        None => print!("{text}"),
    }
    Ok(Exit::Ok)
}
