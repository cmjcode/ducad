use std::path::PathBuf;

use ducad_engine::inspect::summarize;

use super::{apply_params, open_part, parse_param, print_json};
use crate::{CliResult, Exit};

#[derive(clap::Args)]
pub struct Args {
    part: PathBuf,
    #[arg(long = "param", value_parser = parse_param)]
    params: Vec<(String, f64)>,
    #[arg(long)]
    out: Option<PathBuf>,
}

pub fn exec(a: Args) -> CliResult {
    let mut s = open_part(&a.part)?;
    if let Some(report) = apply_params(&mut s, Default::default(), &a.params)? {
        print_json(&report)?;
        if !report.committed {
            return Ok(Exit::OpFailed);
        }
    } else {
        print_json(&summarize(&s, None, false, 0)?)?;
    }
    if let Some(out) = &a.out {
        s.save(out)?;
    }
    Ok(Exit::Ok)
}
