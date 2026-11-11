use std::path::PathBuf;

use ducad_engine::inspect::{summarize, DEFAULT_TOPOLOGY_LIMIT};

use super::{open_part, print_json};
use crate::{CliResult, Exit};

#[derive(clap::Args)]
pub struct Args {
    part: PathBuf,
    #[arg(long)]
    body: Option<String>,
    /// Include face and edge topology lists.
    #[arg(long)]
    topology: bool,
    /// Print mass properties table (mass, center of mass, inertia, principal axes).
    #[arg(long)]
    mass: bool,
    /// Output JSON instead of concise human-readable text.
    #[arg(long)]
    json: bool,
}

fn vec3(v: [f64; 3]) -> String {
    format!("[{:>14.4}, {:>14.4}, {:>14.4}]", v[0], v[1], v[2])
}

fn print_mass(b: &ducad_engine::inspect::BodyReport) {
    let id = crate::is_id();
    println!("{}", if id { "  properti massa" } else { "  mass properties" });
    let material = b
        .mechanical
        .as_ref()
        .map(|m| m.source.clone())
        .or_else(|| b.material.as_ref().map(|m| format!("preset:{}", m.preset)))
        .unwrap_or_else(|| "-".into());
    println!("    material        {material}");
    match b.mass_g {
        Some(m) => {
            if id {
                println!("    massa           {m:.4} g");
            } else {
                println!("    mass            {m:.4} g");
            }
        }
        None => {
            if id {
                println!("    massa           - (densitas tidak diketahui)");
            } else {
                println!("    mass            - (unknown density)");
            }
        }
    }
    println!("    volume          {:.4} mm³", b.volume);
    if id {
        println!("    luas            {:.4} mm²", b.area);
    } else {
        println!("    area            {:.4} mm²", b.area);
    }
    if let Some(c) = b.center_of_mass {
        if id {
            println!("    pusat massa     {} mm", vec3(c));
        } else {
            println!("    center of mass  {} mm", vec3(c));
        }
    }
    if let Some(i) = b.inertia_com {
        if id {
            println!("    inersia @COM    {} g·mm²", vec3(i[0]));
        } else {
            println!("    inertia @COM    {} g·mm²", vec3(i[0]));
        }
        println!("                    {}", vec3(i[1]));
        println!("                    {}", vec3(i[2]));
    }
    if let (Some(m), Some(axes)) = (b.principal_moments, b.principal_axes) {
        for k in 0..3 {
            if id {
                println!(
                    "    momen utama {}   {:>14.4} g·mm²  sumbu [{:.4}, {:.4}, {:.4}]",
                    k + 1,
                    m[k],
                    axes[k][0],
                    axes[k][1],
                    axes[k][2]
                );
            } else {
                println!(
                    "    principal moment {} {:>11.4} g·mm²  axis [{:.4}, {:.4}, {:.4}]",
                    k + 1,
                    m[k],
                    axes[k][0],
                    axes[k][1],
                    axes[k][2]
                );
            }
        }
    }
    if let Some(r) = b.radius_of_gyration {
        if id {
            println!("    radius girasi   {} mm", vec3(r));
        } else {
            println!("    radius of gyration {} mm", vec3(r));
        }
    }
}

pub fn exec(a: Args) -> CliResult {
    let id = crate::is_id();
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
        let color_info = b
            .material
            .as_ref()
            .map(|m| format!("  color {:?}", m.base_color))
            .unwrap_or_default();
        println!(
            "body {:<16} volume {:>12.4}  size {:?}{}  faces {}  edges {}  valid {}",
            b.name, b.volume, b.size, color_info, b.faces, b.edges, b.valid
        );
        if a.mass {
            print_mass(b);
        }
    }
    if let (true, Some(asm)) = (a.mass, &sum.assembly) {
        if id {
            println!(
                "gabungan: massa {:.4} g  pusat massa {} mm",
                asm.total_mass_g,
                vec3(asm.center_of_mass)
            );
            for row in asm.inertia_com {
                println!("  inersia @COM {} g·mm²", vec3(row));
            }
            if !asm.skipped.is_empty() {
                println!("  tanpa densitas: {}", asm.skipped.join(", "));
            }
        } else {
            println!(
                "assembly: mass {:.4} g  center of mass {} mm",
                asm.total_mass_g,
                vec3(asm.center_of_mass)
            );
            for row in asm.inertia_com {
                println!("  inertia @COM {} g·mm²", vec3(row));
            }
            if !asm.skipped.is_empty() {
                println!("  missing density: {}", asm.skipped.join(", "));
            }
        }
    }
    for sk in &sum.sketches {
        if id {
            println!(
                "sketch {:<14} bidang {:<8} entitas {}  region {}  dof {}",
                sk.id, sk.plane, sk.entities, sk.closed_regions, sk.dof
            );
        } else {
            println!(
                "sketch {:<14} plane {:<8} entities {}  regions {}  dof {}",
                sk.id, sk.plane, sk.entities, sk.closed_regions, sk.dof
            );
        }
        if !sk.entity_kinds.is_empty() {
            let kinds = sk
                .entity_kinds
                .iter()
                .map(|(k, v)| format!("{k}: {v}"))
                .collect::<Vec<_>>()
                .join(", ");
            println!("  {}: {kinds}", crate::tr("entities", "entitas"));
        }
        if !sk.names.is_empty() {
            println!("  {}: {}", crate::tr("names", "nama"), sk.names.join(", "));
        }
    }
    for w in &sum.warnings {
        println!("{}: {w}", crate::tr("warning", "peringatan"));
    }
    Ok(Exit::Ok)
}
