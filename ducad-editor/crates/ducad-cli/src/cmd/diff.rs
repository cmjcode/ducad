use std::path::PathBuf;

use ducad_engine::diff::{diff, OpChange};

use super::{open_part, print_json};
use crate::{CliResult, Exit};

#[derive(clap::Args)]
pub struct Args {
    a: PathBuf,
    b: PathBuf,
    #[arg(long)]
    json: bool,
    /// Jangan hitung volume tambah/hilang dengan boolean kernel.
    #[arg(long)]
    no_geometry: bool,
    /// Tulis render diff berwarna (tampak iso) ke SVG.
    #[arg(long)]
    svg: Option<PathBuf>,
    /// Tulis render diff berwarna (tampak iso) ke PNG 800x600.
    #[arg(long)]
    png: Option<PathBuf>,
}

pub fn exec(a: Args) -> CliResult {
    let sa = open_part(&a.a)?;
    let sb = open_part(&a.b)?;
    let (d, shapes) = diff(&sa, &sb, !a.no_geometry);
    if a.svg.is_some() || a.png.is_some() {
        let r = ducad_engine::render::render_diff_svg(
            &sb,
            &shapes,
            ducad_engine::render::View::Iso,
            800,
            600,
        )?;
        let write = |p: &PathBuf, bytes: &[u8]| {
            std::fs::write(p, bytes)
                .map_err(|e| crate::CliError::usage(format!("gagal menulis {}: {e}", p.display())))
        };
        if let Some(p) = &a.svg {
            write(p, r.svg.as_bytes())?;
        }
        if let Some(p) = &a.png {
            write(p, &ducad_engine::render::svg_to_png(&r.svg, 800, 600)?)?;
        }
    }
    if a.json {
        print_json(&d)?;
    } else {
        for p in &d.params {
            println!("param {}: {:?} → {:?}", p.name, p.old, p.new);
        }
        for c in &d.ops {
            match c {
                OpChange::Added { index, op } => {
                    println!("+ op #{index} {} ({})", op.id(), op.kind())
                }
                OpChange::Removed { index, op } => {
                    println!("- op #{index} {} ({})", op.id(), op.kind())
                }
                OpChange::Changed { id, fields } => {
                    for f in fields {
                        println!("~ op {id}{}: {} → {}", f.path, f.old, f.new);
                    }
                }
                OpChange::Reordered { id, from, to } => println!("↕ op {id}: #{from} → #{to}"),
            }
        }
        for b in d.bodies.iter().filter(|b| b.status != "unchanged") {
            println!(
                "body {} {}: volume {:?} → {:?} (+{:?} / −{:?} mm³)",
                b.name, b.status, b.volume_old, b.volume_new, b.added_volume, b.removed_volume
            );
        }
        for w in &d.warnings {
            println!("peringatan: {w}");
        }
    }
    Ok(if d.is_empty() {
        Exit::Ok
    } else {
        Exit::OpFailed
    })
}
