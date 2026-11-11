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
    /// Skip boolean kernel calculation for added/removed volume.
    #[arg(long)]
    no_geometry: bool,
    /// Write color-coded isometric diff render to SVG.
    #[arg(long)]
    svg: Option<PathBuf>,
    /// Write color-coded isometric diff render to PNG (800x600).
    #[arg(long)]
    png: Option<PathBuf>,
}

pub fn exec(a: Args) -> CliResult {
    let ind = crate::is_id();
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
            std::fs::write(p, bytes).map_err(|e| {
                crate::CliError::usage(if ind {
                    format!("gagal menulis {}: {e}", p.display())
                } else {
                    format!("failed to write {}: {e}", p.display())
                })
            })
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
        if let Some(impact) = &d.impact {
            for sheet in &impact.sheets {
                println!(
                    "{} {} {}: {}",
                    crate::tr("sheet", "lembar"),
                    sheet.kind,
                    sheet.name,
                    sheet.reason
                );
            }
            println!(
                "{}: {}",
                crate::tr("unchanged sheets", "lembar tidak terdampak"),
                impact.unchanged_sheets
            );
            for c in &impact.checks {
                println!("check {}: {:?} → {:?} — {}", c.check, c.old, c.new, c.message);
            }
            if impact.checks_remeasured > 0 {
                println!(
                    "{}: {}",
                    crate::tr(
                        "checks remeasured without status change",
                        "check terukur ulang tanpa ganti status"
                    ),
                    impact.checks_remeasured
                );
            }
            for r in &impact.requirements {
                println!(
                    "{} {}: {:?} → {:?}",
                    crate::tr("requirement", "persyaratan"),
                    r.id,
                    r.old,
                    r.new
                );
            }
        }
        for w in &d.warnings {
            println!("{}: {w}", crate::tr("warning", "peringatan"));
        }
    }
    Ok(if d.is_empty() {
        Exit::Ok
    } else {
        Exit::OpFailed
    })
}
