//! Analisis di atas K dan M yang sama dengan studi statik: frekuensi
//! natural, buckling linier, konduksi termal tunak, dan tegangan termal.
//! Semuanya bekerja pada mesh hex voxel maupun Tet10 lewat [`SimMesh`].
//!
//! Satuan: mm, N, s → massa dalam tonne (`g/cm³ × 1e-9 = t/mm³`), sehingga
//! nilai eigen `K φ = λ M φ` adalah `ω²` dalam (rad/s)². Termal: W, mm, K;
//! konduktivitas `W/(m·K) × 1e-3 = W/(mm·K)`.

use serde::{Deserialize, Serialize};

use crate::assemble::assemble_scalar;
use crate::bc::NodeConstraint;
use crate::pipeline::{
    build_mesh, check_cancel, prepare, setup_faces, static_core, MeshRef, SimMesh, ThermalLoad,
};
use crate::report::{MeshStats, ScalarField, SimReport, SolverStats, VectorField};
use crate::setup::{ResolvedSetup, ResolvedThermalSetup, ThermalBcKind};
use crate::solver::{
    lobpcg, solve_pcg, CgOptions, EigenOptions, EigenResult, Jacobi, NodeScalarOp,
};
use crate::{CancelToken, ElasticMaterial, SimError, SurfaceMesh, ThermalMaterial};

/// Jumlah mode bawaan studi frekuensi.
pub const DEFAULT_FREQUENCY_MODES: usize = 10;
/// Jumlah mode bawaan studi buckling.
pub const DEFAULT_BUCKLING_MODES: usize = 3;
/// Toleransi residual relatif solver eigen.
const EIGEN_TOL: f64 = 1.0e-6;
const EIGEN_MAX_ITER: usize = 600;
/// Toleransi PCG untuk konduksi termal dan statik termal (lebih ketat dari
/// statik biasa agar medan suhu linier dan pemuaian bebas tepat).
const THERMAL_TOL: f64 = 1.0e-12;
const THERMAL_STATIC_TOL: f64 = 1.0e-11;

/// Hasil studi frekuensi natural.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrequencyReport {
    /// Frekuensi natural menaik, Hz.
    pub frequencies_hz: Vec<f64>,
    pub mesh_stats: MeshStats,
    pub solver_stats: SolverStats,
    pub warnings: Vec<String>,
    /// Bentuk mode (perpindahan dinormalkan ke maksimum 1), satu per frekuensi.
    #[serde(skip)]
    pub mode_shapes: Vec<VectorField>,
}

/// Hasil studi buckling linier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BucklingReport {
    /// Faktor beban tekuk menaik; yang pertama adalah faktor kritis
    /// (beban tekuk = faktor × beban yang dipasang). Kosong bila beban tidak
    /// menimbulkan tekuk.
    pub load_factors: Vec<f64>,
    /// Von Mises maksimum keadaan pra-tekuk (pada beban yang dipasang), MPa.
    pub prestress_max_von_mises_mpa: f64,
    pub mesh_stats: MeshStats,
    pub solver_stats: SolverStats,
    pub warnings: Vec<String>,
    /// Bentuk mode tekuk (dinormalkan ke maksimum 1), satu per faktor.
    #[serde(skip)]
    pub mode_shapes: Vec<VectorField>,
}

/// Hasil studi konduksi termal tunak.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThermalReport {
    pub max_temperature_c: f64,
    pub min_temperature_c: f64,
    /// Posisi node bersuhu maksimum, mm.
    pub location: [f64; 3],
    pub mesh_stats: MeshStats,
    pub solver_stats: SolverStats,
    pub warnings: Vec<String>,
    /// Suhu nodal, °C.
    #[serde(skip)]
    pub nodal_field: Option<ScalarField>,
}

fn invalid(msg: impl Into<String>) -> SimError {
    SimError::InvalidSetup(msg.into())
}

/// Mengubah vektor eigen menjadi medan bentuk mode bernorma maksimum 1
/// dengan tanda deterministik (komponen terbesar positif).
fn mode_fields(mesh: MeshRef<'_>, result: &EigenResult, keep: &[usize]) -> Vec<VectorField> {
    let locator = mesh.locator();
    keep.iter()
        .map(|&k| {
            let v = &result.vectors[k];
            let mut peak = 0.0_f64;
            let mut signed = 0.0_f64;
            for c in v.chunks_exact(3) {
                let m = (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt();
                peak = peak.max(m);
            }
            for &x in v {
                if x.abs() > signed.abs() {
                    signed = x;
                }
            }
            let scale = if peak > 0.0 {
                signed.signum() / peak
            } else {
                0.0
            };
            let vectors = v
                .chunks_exact(3)
                .map(|c| [c[0] * scale, c[1] * scale, c[2] * scale])
                .collect();
            VectorField::new(mesh.nodes().to_vec(), locator.clone(), vectors)
        })
        .collect()
}

fn without_loads(setup: &ResolvedSetup) -> ResolvedSetup {
    ResolvedSetup {
        fixtures: setup.fixtures.clone(),
        loads: Vec::new(),
        mesh: setup.mesh.clone(),
        exact_volume_mm3: setup.exact_volume_mm3,
    }
}

/// Frekuensi natural dari mesh permukaan: `K φ = ω² M φ` dengan massa
/// konsisten. Fixture berlaku, beban diabaikan. `modes = 0` berarti
/// [`DEFAULT_FREQUENCY_MODES`].
pub fn run_frequency(
    surface: &SurfaceMesh,
    material: &ElasticMaterial,
    setup: &ResolvedSetup,
    modes: usize,
    cancel: &CancelToken,
) -> Result<FrequencyReport, SimError> {
    let supports = without_loads(setup);
    let mesh = build_mesh(
        surface,
        &setup.mesh,
        setup.exact_volume_mm3,
        &setup_faces(&supports),
        cancel,
    )?;
    solve_frequency_on(&mesh, material, setup, &[], modes, cancel)
}

/// Frekuensi natural pada mesh yang sudah ada.
pub fn solve_frequency_on(
    mesh: &SimMesh,
    material: &ElasticMaterial,
    setup: &ResolvedSetup,
    extra: &[NodeConstraint],
    modes: usize,
    cancel: &CancelToken,
) -> Result<FrequencyReport, SimError> {
    let mesh = mesh.as_ref();
    if !(material.density_g_cm3.is_finite() && material.density_g_cm3 > 0.0) {
        return Err(invalid(
            "frequency analysis needs a positive material density",
        ));
    }
    let count = if modes == 0 {
        DEFAULT_FREQUENCY_MODES
    } else {
        modes
    };
    let supports = without_loads(setup);
    let prepared = prepare(mesh, material, &supports, extra, false, cancel)?;
    let elems = prepared.elems.as_ref();
    let fixed = &prepared.conditions.fixed;
    let mass = assemble_scalar(elems, |e, out| elems.mass(e, out))?;
    check_cancel(cancel)?;
    let mass_op = NodeScalarOp {
        matrix: &mass,
        scale: material.density_g_cm3 * 1.0e-9,
        fixed,
    };
    let options = EigenOptions {
        count,
        tol: EIGEN_TOL,
        max_iter: EIGEN_MAX_ITER,
    };
    let result = lobpcg(
        &prepared.assembled.k,
        &mass_op,
        prepared.pre.as_ref(),
        fixed,
        &options,
        cancel,
    )?;
    let mut warnings = mesh.warnings().to_vec();
    warnings.extend(prepared.conditions.warnings.iter().cloned());
    if !result.converged {
        warnings.push(format!(
            "eigen solver stopped at relative residual {:.2e}; the highest modes may be inaccurate",
            result.residual
        ));
    }
    let supported = fixed.iter().any(|&f| f) || !prepared.conditions.penalties.is_empty();
    if !supported {
        warnings.push("no fixtures: the first six modes are rigid-body modes near 0 Hz".into());
    }
    let keep: Vec<usize> = (0..result.values.len()).collect();
    let frequencies_hz = result
        .values
        .iter()
        .map(|&v| v.max(0.0).sqrt() / std::f64::consts::TAU)
        .collect();
    Ok(FrequencyReport {
        frequencies_hz,
        mesh_stats: mesh.stats(),
        solver_stats: SolverStats {
            iterations: result.iterations,
            residual: result.residual,
            dofs: prepared.free_dofs,
        },
        warnings,
        mode_shapes: mode_fields(mesh, &result, &keep),
    })
}

/// Buckling linier dari mesh permukaan: keadaan tegangan statik akibat
/// beban → kekakuan geometri `K_σ` → `(K + λ K_σ) φ = 0`. `modes = 0`
/// berarti [`DEFAULT_BUCKLING_MODES`].
pub fn run_buckling(
    surface: &SurfaceMesh,
    material: &ElasticMaterial,
    setup: &ResolvedSetup,
    modes: usize,
    cancel: &CancelToken,
) -> Result<BucklingReport, SimError> {
    let mesh = build_mesh(
        surface,
        &setup.mesh,
        setup.exact_volume_mm3,
        &setup_faces(setup),
        cancel,
    )?;
    solve_buckling_on(&mesh, material, setup, &[], modes, cancel)
}

/// Buckling linier pada mesh yang sudah ada.
pub fn solve_buckling_on(
    mesh: &SimMesh,
    material: &ElasticMaterial,
    setup: &ResolvedSetup,
    extra: &[NodeConstraint],
    modes: usize,
    cancel: &CancelToken,
) -> Result<BucklingReport, SimError> {
    let mesh = mesh.as_ref();
    let count = if modes == 0 {
        DEFAULT_BUCKLING_MODES
    } else {
        modes
    };
    let outcome = static_core(mesh, material, setup, extra, None, None, cancel)?;
    let prepared = &outcome.prepared;
    let elems = prepared.elems.as_ref();
    let fixed = &prepared.conditions.fixed;
    let mut warnings = outcome.report.warnings.clone();
    let stats = |iterations: usize, residual: f64| SolverStats {
        iterations,
        residual,
        dofs: prepared.free_dofs,
    };
    if outcome.report.max_von_mises_mpa <= 0.0 {
        warnings.push("the study has no loads; no buckling load factor exists".into());
        return Ok(BucklingReport {
            load_factors: Vec::new(),
            prestress_max_von_mises_mpa: 0.0,
            mesh_stats: mesh.stats(),
            solver_stats: stats(0, 0.0),
            warnings,
            mode_shapes: Vec::new(),
        });
    }
    // Kekakuan geometri dari tegangan nodal rata-rata.
    let npe = elems.nodes_per_elem();
    let mut local = vec![[0.0; 6]; npe];
    let geometric = assemble_scalar(elems, |e, out| {
        for (slot, &n) in local.iter_mut().zip(elems.elem_nodes(e)) {
            *slot = outcome.stress[n as usize];
        }
        elems.geometric(e, &local, out);
    })?;
    check_cancel(cancel)?;
    let geometric_op = NodeScalarOp {
        matrix: &geometric,
        scale: 1.0,
        fixed,
    };
    // K_σ φ = θ K φ; θ paling negatif ↔ faktor beban terkecil λ = −1/θ.
    let options = EigenOptions {
        count,
        tol: EIGEN_TOL,
        max_iter: EIGEN_MAX_ITER,
    };
    let result = lobpcg(
        &geometric_op,
        &prepared.assembled.k,
        prepared.pre.as_ref(),
        fixed,
        &options,
        cancel,
    )?;
    if !result.converged {
        warnings.push(format!(
            "eigen solver stopped at relative residual {:.2e}; load factors may be inaccurate",
            result.residual
        ));
    }
    let keep: Vec<usize> = (0..result.values.len())
        .filter(|&k| result.values[k] < 0.0)
        .collect();
    let load_factors: Vec<f64> = keep.iter().map(|&k| -1.0 / result.values[k]).collect();
    if load_factors.is_empty() {
        warnings.push(
            "the applied loads do not cause buckling (no compressive instability found)".into(),
        );
    }
    Ok(BucklingReport {
        load_factors,
        prestress_max_von_mises_mpa: outcome.report.max_von_mises_mpa,
        mesh_stats: mesh.stats(),
        solver_stats: stats(result.iterations, result.residual),
        warnings,
        mode_shapes: mode_fields(mesh, &result, &keep),
    })
}

fn thermal_faces(thermal: &ResolvedThermalSetup) -> Vec<u32> {
    let mut faces: Vec<u32> = thermal
        .boundary
        .iter()
        .flat_map(|b| b.faces.iter().copied())
        .collect();
    faces.sort_unstable();
    faces.dedup();
    faces
}

fn validate_thermal_material(material: &ThermalMaterial) -> Result<(), SimError> {
    if !(material.conductivity_w_mk.is_finite() && material.conductivity_w_mk > 0.0) {
        return Err(invalid("thermal conductivity must be positive"));
    }
    if !(material.expansion_per_k.is_finite() && material.reference_temperature_c.is_finite()) {
        return Err(invalid(
            "thermal expansion and reference temperature must be finite",
        ));
    }
    Ok(())
}

/// Menyelesaikan konduksi tunak; mengembalikan suhu nodal + statistik.
fn thermal_core(
    mesh: MeshRef<'_>,
    material: &ThermalMaterial,
    thermal: &ResolvedThermalSetup,
    cancel: &CancelToken,
) -> Result<(Vec<f64>, SolverStats, Vec<String>), SimError> {
    validate_thermal_material(material)?;
    // Model elemen hanya dipakai untuk matriks konduksi; sifat elastis tidak berpengaruh.
    let dummy = ElasticMaterial {
        young_mpa: 1.0,
        poisson: 0.3,
        density_g_cm3: 0.0,
        yield_mpa: 0.0,
    };
    let elems = mesh.element_model(&dummy)?;
    let elems = elems.as_ref();
    let boundary = mesh.boundary();
    let nn = mesh.nodes().len();
    let mut warnings = Vec::new();
    let mut fixed = vec![false; nn];
    let mut prescribed = vec![0.0; nn];
    let mut source = vec![0.0; nn];
    let mut film = vec![0.0; nn];
    let mut anchored = false;
    for bc in &thermal.boundary {
        let what = format!("thermal boundary '{}'", bc.id);
        if bc.faces.is_empty() {
            return Err(invalid(format!("{what}: no faces selected")));
        }
        let nodal = boundary.face_nodal(&bc.faces, &what, &mut warnings)?;
        match bc.kind {
            ThermalBcKind::Temperature { celsius } => {
                if !celsius.is_finite() {
                    return Err(invalid(format!("{what}: temperature must be finite")));
                }
                let mut faces = bc.faces.clone();
                faces.sort_unstable();
                faces.dedup();
                let mut nodes: Vec<u32> = boundary.fixture_nodes(&faces).keys().copied().collect();
                nodes.extend(&nodal.nodes);
                for n in nodes {
                    fixed[n as usize] = true;
                    prescribed[n as usize] = celsius;
                }
                anchored = true;
            }
            ThermalBcKind::HeatFlux { w_per_mm2 } => {
                if !w_per_mm2.is_finite() {
                    return Err(invalid(format!("{what}: heat flux must be finite")));
                }
                for (i, &n) in nodal.nodes.iter().enumerate() {
                    source[n as usize] += w_per_mm2 * nodal.area[i];
                }
            }
            ThermalBcKind::Convection {
                h_w_mm2k,
                ambient_c,
            } => {
                if !(h_w_mm2k.is_finite() && h_w_mm2k >= 0.0 && ambient_c.is_finite()) {
                    return Err(invalid(format!(
                        "{what}: film coefficient must be non-negative and ambient temperature finite"
                    )));
                }
                // Konveksi dilumpkan ke diagonal (luas nodal tak-negatif).
                for (i, &n) in nodal.nodes.iter().enumerate() {
                    let area = nodal.area[i].max(0.0);
                    film[n as usize] += h_w_mm2k * area;
                    source[n as usize] += h_w_mm2k * ambient_c * area;
                }
                anchored |= h_w_mm2k > 0.0;
            }
        }
    }
    if !anchored {
        return Err(invalid(
            "a thermal study needs at least one temperature or convection boundary (otherwise the temperature level is undefined)",
        ));
    }
    let conductivity = material.conductivity_w_mk * 1.0e-3;
    let mut matrix = assemble_scalar(elems, |e, out| elems.conductivity(e, out))?;
    for v in matrix.val.iter_mut() {
        *v *= conductivity;
    }
    for n in 0..nn {
        if matrix.row_ptr[n] < matrix.row_ptr[n + 1] {
            let at = matrix.row_ptr[n];
            matrix.val[at] += film[n];
        }
    }
    check_cancel(cancel)?;
    // Suhu terpasang dipindah ke ruas kanan: b = q − K·T_d.
    let mut lifted = vec![0.0; nn];
    matrix.mul(&prescribed, &mut lifted);
    let mut rhs: Vec<f64> = source.iter().zip(&lifted).map(|(q, g)| q - g).collect();
    for n in 0..nn {
        if fixed[n] {
            rhs[n] = 0.0;
        }
    }
    matrix.eliminate(&fixed);
    let free = fixed.iter().filter(|&&f| !f).count();
    let mut options = CgOptions::for_size(free);
    options.rel_tol = THERMAL_TOL;
    options.max_iter = options.max_iter.max(20_000);
    let (delta, stats) = solve_pcg(&matrix, &rhs, &Jacobi::new(&matrix), &options, cancel).map_err(|e| match e {
        SimError::Underconstrained { .. } => invalid(
            "the thermal problem is singular: part of the body is not connected to any temperature or convection boundary",
        ),
        other => other,
    })?;
    let temperature: Vec<f64> = delta.iter().zip(&prescribed).map(|(d, t)| d + t).collect();
    Ok((
        temperature,
        SolverStats {
            iterations: stats.iterations,
            residual: stats.residual,
            dofs: free,
        },
        warnings,
    ))
}

/// Konduksi termal tunak dari mesh permukaan. `setup` hanya dipakai untuk
/// pengaturan mesh dan volume eksak; fixture dan beban mekanik diabaikan.
pub fn run_thermal(
    surface: &SurfaceMesh,
    material: &ThermalMaterial,
    setup: &ResolvedSetup,
    thermal: &ResolvedThermalSetup,
    cancel: &CancelToken,
) -> Result<ThermalReport, SimError> {
    validate_thermal_material(material)?;
    let mesh = build_mesh(
        surface,
        &setup.mesh,
        setup.exact_volume_mm3,
        &thermal_faces(thermal),
        cancel,
    )?;
    solve_thermal_on(&mesh, material, thermal, cancel)
}

/// Konduksi termal tunak pada mesh yang sudah ada.
pub fn solve_thermal_on(
    mesh: &SimMesh,
    material: &ThermalMaterial,
    thermal: &ResolvedThermalSetup,
    cancel: &CancelToken,
) -> Result<ThermalReport, SimError> {
    let mesh = mesh.as_ref();
    let (temperature, solver_stats, extra_warnings) =
        thermal_core(mesh, material, thermal, cancel)?;
    let mut max = f64::NEG_INFINITY;
    let mut min = f64::INFINITY;
    let mut at = 0usize;
    for (n, &t) in temperature.iter().enumerate() {
        if t > max {
            max = t;
            at = n;
        }
        min = min.min(t);
    }
    let mut warnings = mesh.warnings().to_vec();
    warnings.extend(extra_warnings);
    Ok(ThermalReport {
        max_temperature_c: max,
        min_temperature_c: min,
        location: mesh.nodes().get(at).copied().unwrap_or([0.0; 3]),
        mesh_stats: mesh.stats(),
        solver_stats,
        warnings,
        nodal_field: Some(ScalarField::new(
            mesh.nodes().to_vec(),
            mesh.locator(),
            temperature,
        )),
    })
}

/// Tegangan termal (coupling satu arah) dari mesh permukaan: konduksi tunak
/// → `ΔT = T − T_acuan` → regangan termal `α·ΔT` sebagai beban ekuivalen
/// pada studi statik. Fixture dan beban mekanik di `setup` ikut bekerja.
pub fn run_thermal_stress(
    surface: &SurfaceMesh,
    material: &ElasticMaterial,
    thermal_material: &ThermalMaterial,
    setup: &ResolvedSetup,
    thermal: &ResolvedThermalSetup,
    cancel: &CancelToken,
) -> Result<SimReport, SimError> {
    validate_thermal_material(thermal_material)?;
    let mut faces = setup_faces(setup);
    faces.extend(thermal_faces(thermal));
    faces.sort_unstable();
    faces.dedup();
    let mesh = build_mesh(surface, &setup.mesh, setup.exact_volume_mm3, &faces, cancel)?;
    solve_thermal_stress_on(
        &mesh,
        material,
        thermal_material,
        setup,
        thermal,
        &[],
        cancel,
    )
}

/// Tegangan termal pada mesh yang sudah ada.
pub fn solve_thermal_stress_on(
    mesh: &SimMesh,
    material: &ElasticMaterial,
    thermal_material: &ThermalMaterial,
    setup: &ResolvedSetup,
    thermal: &ResolvedThermalSetup,
    extra: &[NodeConstraint],
    cancel: &CancelToken,
) -> Result<SimReport, SimError> {
    let mesh = mesh.as_ref();
    let (temperature, _, thermal_warnings) = thermal_core(mesh, thermal_material, thermal, cancel)?;
    let delta_t: Vec<f64> = temperature
        .iter()
        .map(|t| t - thermal_material.reference_temperature_c)
        .collect();
    let load = ThermalLoad {
        delta_t: &delta_t,
        alpha: thermal_material.expansion_per_k,
    };
    let mut report = static_core(
        mesh,
        material,
        setup,
        extra,
        Some(load),
        Some(THERMAL_STATIC_TOL),
        cancel,
    )?
    .report;
    report.warnings.extend(thermal_warnings);
    Ok(report)
}
