use std::path::PathBuf;

use ducad_engine::select::{select_edges, select_faces};

use super::{open_part, print_json};
use crate::{CliError, CliResult, Exit};

#[derive(clap::Args)]
pub struct Args {
    part: PathBuf,
    #[arg(long)]
    body: String,
    #[arg(long, conflicts_with = "edges", required_unless_present = "edges")]
    faces: Option<String>,
    #[arg(long)]
    edges: Option<String>,
    #[arg(long)]
    json: bool,
}

pub fn exec(a: Args) -> CliResult {
    let s = open_part(&a.part)?;
    let (_, geo) = s.body(&a.body)?;
    let out = match (&a.faces, &a.edges) {
        (Some(sel), None) => {
            let idx = select_faces(&geo.shape, sel)?;
            let all = ducad_kernel::enumerate_faces(&geo.shape);
            let items: Vec<_> = idx.iter().map(|&i| all[i].clone()).collect();
            serde_json::json!({ "count": idx.len(), "indices": idx, "items": items })
        }
        (None, Some(sel)) => {
            let idx = select_edges(&geo.shape, sel)?;
            let all = ducad_kernel::enumerate_edges(&geo.shape);
            let items: Vec<_> = idx.iter().map(|&i| all[i].clone()).collect();
            serde_json::json!({ "count": idx.len(), "indices": idx, "items": items })
        }
        _ => {
            return Err(CliError::usage(
                "pakai tepat satu dari --faces atau --edges",
            ))
        }
    };
    if a.json {
        print_json(&out)?;
    } else {
        println!("{} cocok: {}", out["count"], out["indices"]);
    }
    Ok(Exit::Ok)
}
