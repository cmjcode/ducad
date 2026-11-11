use std::path::{Path, PathBuf};

use ducad_engine::render::{svg_to_png, View};
use ducad_engine::sim::{
    render_study_svg, AnalysisReport, Overlay, StudyOutcome, StudyRenderOptions,
};
use ducad_engine::Session;
use ducad_sim::{CancelToken, SimReport};

use super::{open_part, print_json};
use crate::{CliError, CliResult, Exit};

#[derive(clap::Args)]
pub struct Args {
    part: PathBuf,
    /// Study operation ID; may be omitted when the part defines only one study.
    #[arg(long)]
    study: Option<String>,
    /// Save von Mises stress heatmap to a `.png` or `.svg` file.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Output JSON instead of concise human-readable text.
    #[arg(long)]
    json: bool,
}

/// Tulis heatmap hasil studi `id` (PNG atau SVG menurut ekstensi `path`).
pub fn write_heatmap(
    s: &Session,
    id: &str,
    report: &SimReport,
    overlay: Overlay,
    path: &Path,
) -> Result<(), CliError> {
    let setup = ducad_engine::sim::setup_of(s.meta(), id)?;
    let (geo, yield_mpa) = ducad_engine::sim::study_body(s.model(), s.meta(), &setup)?;
    let (width, height) = (1200, 900);
    let svg = render_study_svg(
        geo,
        report,
        &StudyRenderOptions {
            view: View::Iso,
            width,
            height,
            overlay,
            deform_scale: None,
            yield_mpa,
        },
    )?;
    let is_svg = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"));
    let bytes = if is_svg {
        svg.into_bytes()
    } else {
        svg_to_png(&svg, width, height)?
    };
    std::fs::write(path, bytes).map_err(|e| {
        CliError::usage(if crate::is_id() {
            format!("gagal menulis {}: {e}", path.display())
        } else {
            format!("failed to write {}: {e}", path.display())
        })
    })
}

/// Cetak hasil studi frekuensi/buckling/termal (tidak punya heatmap).
fn print_analysis(id: &str, report: &AnalysisReport, a: &Args) -> CliResult {
    let ind = crate::is_id();
    if a.out.is_some() {
        return Err(CliError::usage(crate::tr(
            "--out is only supported for static/thermal_stress studies (this study has no stress field)",
            "--out hanya untuk studi static/thermal_stress (studi ini tidak punya medan tegangan)",
        )));
    }
    if a.json {
        print_json(report)?;
        return Ok(Exit::Ok);
    }
    let (mesh, solver, warnings) = match report {
        AnalysisReport::Frequency(r) => {
            if ind {
                println!("studi {id} — frekuensi natural");
            } else {
                println!("study {id} — natural frequency");
            }
            for (i, f) in r.frequencies_hz.iter().enumerate() {
                println!("  mode {:<2}          {f:.3} Hz", i + 1);
            }
            (&r.mesh_stats, &r.solver_stats, &r.warnings)
        }
        AnalysisReport::Buckling(r) => {
            if ind {
                println!("studi {id} — buckling linier");
            } else {
                println!("study {id} — linear buckling");
            }
            if r.load_factors.is_empty() {
                println!(
                    "  {}",
                    crate::tr(
                        "load does not induce buckling",
                        "beban tidak menimbulkan tekuk"
                    )
                );
            }
            for (i, f) in r.load_factors.iter().enumerate() {
                if ind {
                    println!("  faktor beban {:<2}  {f:.4}", i + 1);
                } else {
                    println!("  load factor {:<2}   {f:.4}", i + 1);
                }
            }
            if ind {
                println!(
                    "  von Mises pra-tekuk {:.4} MPa",
                    r.prestress_max_von_mises_mpa
                );
            } else {
                println!(
                    "  pre-buckling von Mises {:.4} MPa",
                    r.prestress_max_von_mises_mpa
                );
            }
            (&r.mesh_stats, &r.solver_stats, &r.warnings)
        }
        AnalysisReport::Thermal(r) => {
            if ind {
                println!("studi {id} — konduksi termal tunak");
                println!(
                    "  suhu maks        {:.3} °C di [{:.3}, {:.3}, {:.3}]",
                    r.max_temperature_c, r.location[0], r.location[1], r.location[2]
                );
                println!("  suhu min         {:.3} °C", r.min_temperature_c);
            } else {
                println!("study {id} — steady-state thermal conduction");
                println!(
                    "  max temperature  {:.3} °C at [{:.3}, {:.3}, {:.3}]",
                    r.max_temperature_c, r.location[0], r.location[1], r.location[2]
                );
                println!("  min temperature  {:.3} °C", r.min_temperature_c);
            }
            (&r.mesh_stats, &r.solver_stats, &r.warnings)
        }
    };
    if ind {
        println!(
            "  mesh             {} elemen, {} node, sel {:.4} mm",
            mesh.elements, mesh.nodes, mesh.cell_mm
        );
        println!(
            "  solver           {} iterasi, residual {:.3e}, {} DOF",
            solver.iterations, solver.residual, solver.dofs
        );
    } else {
        println!(
            "  mesh             {} elements, {} nodes, cell {:.4} mm",
            mesh.elements, mesh.nodes, mesh.cell_mm
        );
        println!(
            "  solver           {} iterations, residual {:.3e}, {} DOF",
            solver.iterations, solver.residual, solver.dofs
        );
    }
    for w in warnings {
        println!("  {}: {w}", crate::tr("warning", "peringatan"));
    }
    Ok(Exit::Ok)
}

pub fn exec(a: Args) -> CliResult {
    let ind = crate::is_id();
    let mut s = open_part(&a.part)?;
    let id = ducad_engine::sim::pick_study(s.meta(), a.study.as_deref())?;
    let report = match s.run_study_any(&id, &CancelToken::new())? {
        StudyOutcome::Stress(r) => r,
        StudyOutcome::Analysis(r) => return print_analysis(&id, &r, &a),
    };
    if let Some(out) = &a.out {
        write_heatmap(&s, &id, &report, Overlay::Stress, out)?;
    }
    if a.json {
        print_json(report.as_ref())?;
        return Ok(Exit::Ok);
    }
    if ind {
        println!("studi {id} — estimasi teknik (mesh {:?})", report.mesh_stats.kind);
        println!(
            "  von Mises maks   {:.4} MPa di [{:.3}, {:.3}, {:.3}]",
            report.max_von_mises_mpa, report.location[0], report.location[1], report.location[2]
        );
        println!("  deformasi maks   {:.6} mm", report.max_displacement_mm);
        println!("  faktor keamanan  {:.3}", report.safety_factor);
        for r in &report.reactions {
            println!(
                "  reaksi {:<10} [{:.3}, {:.3}, {:.3}] N",
                r.fixture_id, r.force_n[0], r.force_n[1], r.force_n[2]
            );
        }
        println!(
            "  mesh             {} elemen, {} node, sel {:.4} mm",
            report.mesh_stats.elements, report.mesh_stats.nodes, report.mesh_stats.cell_mm
        );
        println!(
            "  solver           {} iterasi, residual {:.3e}, {} DOF",
            report.solver_stats.iterations, report.solver_stats.residual, report.solver_stats.dofs
        );
    } else {
        println!(
            "study {id} — engineering estimate ({:?} mesh)",
            report.mesh_stats.kind
        );
        println!(
            "  max von Mises    {:.4} MPa at [{:.3}, {:.3}, {:.3}]",
            report.max_von_mises_mpa, report.location[0], report.location[1], report.location[2]
        );
        println!("  max displacement {:.6} mm", report.max_displacement_mm);
        println!("  safety factor    {:.3}", report.safety_factor);
        for r in &report.reactions {
            println!(
                "  reaction {:<8} [{:.3}, {:.3}, {:.3}] N",
                r.fixture_id, r.force_n[0], r.force_n[1], r.force_n[2]
            );
        }
        println!(
            "  mesh             {} elements, {} nodes, cell {:.4} mm",
            report.mesh_stats.elements, report.mesh_stats.nodes, report.mesh_stats.cell_mm
        );
        println!(
            "  solver           {} iterations, residual {:.3e}, {} DOF",
            report.solver_stats.iterations, report.solver_stats.residual, report.solver_stats.dofs
        );
    }
    for w in &report.warnings {
        println!("  {}: {w}", crate::tr("warning", "peringatan"));
    }
    Ok(Exit::Ok)
}
