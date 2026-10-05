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
    /// Id op `study`; boleh dikosongkan bila part hanya punya satu studi.
    #[arg(long)]
    study: Option<String>,
    /// Simpan heatmap von Mises ke berkas .png atau .svg.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Keluaran JSON (default: teks ringkas).
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
    std::fs::write(path, bytes)
        .map_err(|e| CliError::usage(format!("gagal menulis {}: {e}", path.display())))
}

/// Cetak hasil studi frekuensi/buckling/termal (tidak punya heatmap).
fn print_analysis(id: &str, report: &AnalysisReport, a: &Args) -> CliResult {
    if a.out.is_some() {
        return Err(CliError::usage(
            "--out hanya untuk studi static/thermal_stress (studi ini tidak punya medan tegangan)",
        ));
    }
    if a.json {
        print_json(report)?;
        return Ok(Exit::Ok);
    }
    let (mesh, solver, warnings) = match report {
        AnalysisReport::Frequency(r) => {
            println!("studi {id} — frekuensi natural");
            for (i, f) in r.frequencies_hz.iter().enumerate() {
                println!("  mode {:<2}          {f:.3} Hz", i + 1);
            }
            (&r.mesh_stats, &r.solver_stats, &r.warnings)
        }
        AnalysisReport::Buckling(r) => {
            println!("studi {id} — buckling linier");
            if r.load_factors.is_empty() {
                println!("  beban tidak menimbulkan tekuk");
            }
            for (i, f) in r.load_factors.iter().enumerate() {
                println!("  faktor beban {:<2}  {f:.4}", i + 1);
            }
            println!(
                "  von Mises pra-tekuk {:.4} MPa",
                r.prestress_max_von_mises_mpa
            );
            (&r.mesh_stats, &r.solver_stats, &r.warnings)
        }
        AnalysisReport::Thermal(r) => {
            println!("studi {id} — konduksi termal tunak");
            println!(
                "  suhu maks        {:.3} °C di [{:.3}, {:.3}, {:.3}]",
                r.max_temperature_c, r.location[0], r.location[1], r.location[2]
            );
            println!("  suhu min         {:.3} °C", r.min_temperature_c);
            (&r.mesh_stats, &r.solver_stats, &r.warnings)
        }
    };
    println!(
        "  mesh             {} elemen, {} node, sel {:.4} mm",
        mesh.elements, mesh.nodes, mesh.cell_mm
    );
    println!(
        "  solver           {} iterasi, residual {:.3e}, {} DOF",
        solver.iterations, solver.residual, solver.dofs
    );
    for w in warnings {
        println!("  peringatan: {w}");
    }
    Ok(Exit::Ok)
}

pub fn exec(a: Args) -> CliResult {
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
    for w in &report.warnings {
        println!("  peringatan: {w}");
    }
    Ok(Exit::Ok)
}
