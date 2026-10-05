//! Gate P17: benchmark analitik pada mesh hex voxel.
//! Jalankan dengan `--nocapture` untuk melihat angka terukur.

use ducad_sim::benchmark::cases::{self, BenchValue};

fn show(name: &str, v: BenchValue) {
    println!(
        "{name}: terukur {:.6}, analitik {:.6}, galat {:+.2} %",
        v.measured,
        v.analytic,
        100.0 * v.rel_error()
    );
}

#[test]
fn bench_tension_bar() {
    // Batang 10×10×100 mm, 1000 N aksial: σ = 10 MPa ± 2 %, δ = FL/(EA) ± 3 %.
    let result = cases::tension_bar(2.5).unwrap();
    show("tension_bar stress [MPa]", result.stress);
    show("tension_bar elongation [mm]", result.elongation);
    assert!(result.stress.rel_error().abs() < 0.02);
    assert!(result.elongation.rel_error().abs() < 0.03);
    // Reaksi jepitan menyeimbangkan beban.
    let r = result.report.reactions[0].force_n;
    assert!(
        (r[0] + 1000.0).abs() < 1e-3 && r[1].abs() < 1e-3 && r[2].abs() < 1e-3,
        "{r:?}"
    );
    assert!((result.report.safety_factor - 250.0 / result.report.max_von_mises_mpa).abs() < 1e-9);
}

#[test]
fn bench_cantilever() {
    // Balok 10×10×100 mm, 100 N di ujung: δ Euler–Bernoulli ± 10 %,
    // tegangan lentur maksimum (FLc/I = 60 MPa) ± 10 %.
    let result = cases::cantilever(2.5, 0.3).unwrap();
    show("cantilever deflection [mm]", result.deflection);
    show("cantilever max von Mises vs FLc/I [MPa]", result.max_stress);
    show(
        "cantilever top-fibre stress at x = 10 mm [MPa]",
        result.section_stress,
    );
    assert!(result.deflection.rel_error().abs() < 0.10);
    assert!(result.max_stress.rel_error().abs() < 0.10);
    assert!(result.section_stress.rel_error().abs() < 0.10);
    // Puncak tegangan berada di akar balok.
    assert!(
        result.report.location[0] < 5.0,
        "{:?}",
        result.report.location
    );
    let r = result.report.reactions[0].force_n;
    assert!((r[2] - 100.0).abs() < 1e-3, "{r:?}");
}

#[test]
fn bench_pressure_plate() {
    // Pelat 80×80×2 mm tumpuan sederhana empat tepi, tekanan merata 0.01 MPa:
    // defleksi tengah vs Timoshenko (0.00406·q·a⁴/D) ± 10 %.
    let result = cases::pressure_plate(80.0, 2.0, 2.0, 0.01).unwrap();
    show("pressure_plate centre deflection [mm]", result.deflection);
    assert!(result.deflection.rel_error().abs() < 0.10);
}

#[test]
fn bench_hole_plate() {
    // Pelat 100×50×2 mm berlubang Ø10 ditarik: faktor konsentrasi 2.5–3.3.
    let result = cases::hole_plate([100.0, 50.0, 2.0], 10.0, 1.0).unwrap();
    show(
        "hole_plate stress concentration (gross)",
        result.concentration,
    );
    let kt = result.concentration.measured;
    assert!((2.5..=3.3).contains(&kt), "Kt = {kt}");
}

/// Waktu dinding untuk ~50k elemen; jalankan dengan
/// `cargo test --release -p ducad-sim --test benchmarks -- --ignored --nocapture`.
#[test]
#[ignore = "pengukuran performa, jalankan manual di profil release"]
fn perf_50k_elements() {
    let start = std::time::Instant::now();
    let bar = cases::cantilever(0.585, 0.3).unwrap();
    println!(
        "perf cantilever: {} elemen, {} DOF, {} iterasi, {:.2} s",
        bar.report.mesh_stats.elements,
        bar.report.solver_stats.dofs,
        bar.report.solver_stats.iterations,
        start.elapsed().as_secs_f64()
    );
    let start = std::time::Instant::now();
    let plate = cases::hole_plate([100.0, 50.0, 10.0], 20.0, 0.97).unwrap();
    println!(
        "perf hole plate: {} elemen, {} DOF, {} iterasi, {:.2} s",
        plate.report.mesh_stats.elements,
        plate.report.solver_stats.dofs,
        plate.report.solver_stats.iterations,
        start.elapsed().as_secs_f64()
    );
    let start = std::time::Instant::now();
    let thin = cases::hole_plate([100.0, 50.0, 1.0], 10.0, 0.45).unwrap();
    println!(
        "perf thin hole plate: {} elemen, {} DOF, {} iterasi, {:.2} s",
        thin.report.mesh_stats.elements,
        thin.report.solver_stats.dofs,
        thin.report.solver_stats.iterations,
        start.elapsed().as_secs_f64()
    );
}

/// Waktu dinding braket ~100k Tet10: mesh, statik, dan 10 mode frekuensi.
/// `cargo test --release -p ducad-sim --test benchmarks -- --ignored --nocapture`.
#[test]
#[ignore = "pengukuran performa, jalankan manual di profil release"]
fn perf_100k_tet() {
    use ducad_sim::benchmark::l_bracket;
    use ducad_sim::{
        build_mesh, solve_frequency_on, solve_static_on, CancelToken, FixtureKind, LoadKind,
        ResolvedFixture, ResolvedLoad, ResolvedSetup,
    };
    let surface = l_bracket(80.0, 60.0, 10.0, 40.0);
    let size = std::env::var("PERF_TET_SIZE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.64);
    let setup = ResolvedSetup {
        fixtures: vec![ResolvedFixture {
            id: "back".into(),
            faces: vec![5],
            kind: FixtureKind::Fixed,
        }],
        loads: vec![ResolvedLoad {
            id: "tip".into(),
            faces: vec![1],
            kind: LoadKind::Force {
                newton: [0.0, 0.0, -500.0],
            },
        }],
        mesh: cases::tet(size),
        exact_volume_mm3: Some(52_000.0),
    };
    let cancel = CancelToken::new();
    let material = cases::steel();
    let start = std::time::Instant::now();
    let mesh = build_mesh(
        &surface,
        &setup.mesh,
        setup.exact_volume_mm3,
        &[1, 5],
        &cancel,
    )
    .unwrap();
    let mesh_time = start.elapsed().as_secs_f64();
    let start = std::time::Instant::now();
    let report = solve_static_on(&mesh, &material, &setup, &[], &cancel).unwrap();
    let static_time = start.elapsed().as_secs_f64();
    println!(
        "perf tet bracket: {:?} {} elemen, {} node, {} DOF; mesh {:.2} s; statik {} iterasi {:.2} s (total {:.2} s); max vM {:.2} MPa; peringatan {:?}",
        report.mesh_stats.kind,
        report.mesh_stats.elements,
        report.mesh_stats.nodes,
        report.solver_stats.dofs,
        mesh_time,
        report.solver_stats.iterations,
        static_time,
        mesh_time + static_time,
        report.max_von_mises_mpa,
        report.warnings
    );
    let start = std::time::Instant::now();
    let modes = solve_frequency_on(&mesh, &material, &setup, &[], 10, &cancel).unwrap();
    println!(
        "perf tet bracket frekuensi 10 mode: {} iterasi, {:.2} s (+ mesh {:.2} s); f1..f3 = {:.1?} Hz; peringatan {:?}",
        modes.solver_stats.iterations,
        start.elapsed().as_secs_f64(),
        mesh_time,
        &modes.frequencies_hz[..3],
        modes.warnings
    );
}
