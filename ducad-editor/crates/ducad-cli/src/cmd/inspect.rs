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
    /// Cetak tabel properti massa (massa, pusat massa, inersia, sumbu utama).
    #[arg(long)]
    mass: bool,
    /// Keluaran JSON (default: teks ringkas).
    #[arg(long)]
    json: bool,
}

fn vec3(v: [f64; 3]) -> String {
    format!("[{:>14.4}, {:>14.4}, {:>14.4}]", v[0], v[1], v[2])
}

fn print_mass(b: &ducad_engine::inspect::BodyReport) {
    println!("  properti massa");
    let material = b
        .mechanical
        .as_ref()
        .map(|m| m.source.clone())
        .or_else(|| b.material.as_ref().map(|m| format!("preset:{}", m.preset)))
        .unwrap_or_else(|| "-".into());
    println!("    material        {material}");
    match b.mass_g {
        Some(m) => println!("    massa           {m:.4} g"),
        None => println!("    massa           - (densitas tidak diketahui)"),
    }
    println!("    volume          {:.4} mm³", b.volume);
    println!("    luas            {:.4} mm²", b.area);
    if let Some(c) = b.center_of_mass {
        println!("    pusat massa     {} mm", vec3(c));
    }
    if let Some(i) = b.inertia_com {
        println!("    inersia @COM    {} g·mm²", vec3(i[0]));
        println!("                    {}", vec3(i[1]));
        println!("                    {}", vec3(i[2]));
    }
    if let (Some(m), Some(axes)) = (b.principal_moments, b.principal_axes) {
        for k in 0..3 {
            println!(
                "    momen utama {}   {:>14.4} g·mm²  sumbu [{:.4}, {:.4}, {:.4}]",
                k + 1,
                m[k],
                axes[k][0],
                axes[k][1],
                axes[k][2]
            );
        }
    }
    if let Some(r) = b.radius_of_gyration {
        println!("    radius girasi   {} mm", vec3(r));
    }
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
