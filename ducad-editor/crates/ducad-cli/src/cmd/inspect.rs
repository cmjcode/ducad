use std::path::PathBuf;

use ducad_engine::inspect::{summarize, DEFAULT_TOPOLOGY_LIMIT};

use super::{open_part, print_json};
use crate::{CliResult, Exit};

#[derive(clap::Args)]
pub struct Args {
    part: PathBuf,
    #[arg(long)]
    body: Option<String>,
    /// Sertakan daftar face/tepi.
    #[arg(long)]
    topology: bool,
    /// Keluaran JSON (default: teks ringkas).
    #[arg(long)]
    json: bool,
}

pub fn exec(a: Args) -> CliResult {
    let s = open_part(&a.part)?;
    let sum = summarize(&s, a.body.as_deref(), a.topology, DEFAULT_TOPOLOGY_LIMIT)?;
    if a.json {
        print_json(&sum)?;
        return Ok(Exit::Ok);
    }
    println!(
        "unit: {}   oplog: {} op   params: {:?}",
        sum.unit, sum.oplog_len, sum.params
    );
    for b in &sum.bodies {
        println!(
            "body {:<16} volume {:>12.4}  size {:?}  faces {}  edges {}  valid {}",
            b.name, b.volume, b.size, b.faces, b.edges, b.valid
        );
    }
    for sk in &sum.sketches {
        println!(
            "sketch {:<14} bidang {:<8} entitas {}  region {}  dof {}",
            sk.id, sk.plane, sk.entities, sk.closed_regions, sk.dof
        );
        if !sk.entity_kinds.is_empty() {
            let kinds = sk
                .entity_kinds
                .iter()
                .map(|(k, v)| format!("{k}: {v}"))
                .collect::<Vec<_>>()
                .join(", ");
            println!("  entitas: {kinds}");
        }
        if !sk.names.is_empty() {
            println!("  nama: {}", sk.names.join(", "));
        }
    }
    for w in &sum.warnings {
        println!("peringatan: {w}");
    }
    Ok(Exit::Ok)
}
