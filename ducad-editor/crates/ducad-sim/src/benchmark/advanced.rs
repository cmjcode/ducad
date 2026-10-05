//! Benchmark P18: frekuensi balok, tekuk Euler, dan batang termal. Tiap
//! kasus menerima pengaturan mesh sehingga bisa dijalankan pada hex voxel
//! maupun Tet10.

use super::cases::{steel, BenchValue};
use super::{box_surface, FACE_XMAX, FACE_XMIN};
use crate::bc::NodeConstraint;
use crate::setup::{
    FixtureKind, LoadKind, MeshSettings, ResolvedFixture, ResolvedLoad, ResolvedSetup,
    ResolvedThermalBc, ResolvedThermalSetup, ThermalBcKind,
};
use crate::{
    build_mesh, run_frequency, solve_buckling_on, solve_thermal_on, solve_thermal_stress_on,
    BucklingReport, CancelToken, FrequencyReport, SimError, SimMesh, SimReport, ThermalMaterial,
    ThermalReport,
};

/// Sifat termal baja umum: k = 50 W/(m·K), α = 1.2e-5 /K, acuan 20 °C.
pub fn steel_thermal() -> ThermalMaterial {
    ThermalMaterial {
        conductivity_w_mk: 50.0,
        expansion_per_k: 1.2e-5,
        reference_temperature_c: 20.0,
    }
}

/// Hasil benchmark frekuensi.
#[derive(Debug, Clone, PartialEq)]
pub struct FrequencyBeam {
    /// Tiga frekuensi pertama vs Euler–Bernoulli: lentur lemah 1, lentur
    /// kuat 1, lentur lemah 2 (Hz).
    pub modes: Vec<BenchValue>,
    pub report: FrequencyReport,
}

/// Kantilever 150×10×5 mm (panjang sepanjang X) dijepit di −X.
/// Euler–Bernoulli: `f = β²/(2π L²)·√(EI/(ρA))`, β₁ = 1.8751, β₂ = 4.6941.
pub fn frequency_beam(settings: MeshSettings) -> Result<FrequencyBeam, SimError> {
    let (length, width, thickness) = (150.0, 10.0, 5.0);
    let material = steel();
    let setup = ResolvedSetup {
        fixtures: vec![ResolvedFixture {
            id: "wall".into(),
            faces: vec![FACE_XMIN],
            kind: FixtureKind::Fixed,
        }],
        loads: Vec::new(),
        mesh: settings,
        exact_volume_mm3: Some(length * width * thickness),
    };
    let surface = box_surface([length, width, thickness]);
    let report = run_frequency(&surface, &material, &setup, 3, &CancelToken::new())?;
    // √(E/ρ) dalam mm/s: MPa / (t/mm³).
    let wave = (material.young_mpa / (material.density_g_cm3 * 1.0e-9)).sqrt();
    let analytic = |beta: f64, depth: f64| {
        beta * beta / (std::f64::consts::TAU * length * length) * wave * depth / 12.0_f64.sqrt()
    };
    let expected = [
        analytic(1.875_104_068_7, thickness),
        analytic(1.875_104_068_7, width),
        analytic(4.694_091_132_9, thickness),
    ];
    let modes = expected
        .iter()
        .enumerate()
        .map(|(i, &analytic)| BenchValue {
            measured: report.frequencies_hz.get(i).copied().unwrap_or(f64::NAN),
            analytic,
        })
        .collect();
    Ok(FrequencyBeam { modes, report })
}

/// Hasil benchmark tekuk.
#[derive(Debug, Clone, PartialEq)]
pub struct EulerColumn {
    /// Beban tekuk kritis vs `π²EI/L²`, N.
    pub critical_load: BenchValue,
    pub report: BucklingReport,
}

fn nearest_node(mesh: &SimMesh, p: [f64; 3]) -> u32 {
    let mut best = (f64::INFINITY, 0u32);
    for (i, q) in mesh.nodes().iter().enumerate() {
        let d = (q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2);
        if d < best.0 {
            best = (d, i as u32);
        }
    }
    best.1
}

/// Kolom 100×10×5 mm bertumpu sendi–sendi, ditekan 1000 N dari kedua ujung.
///
/// Sendi: perpindahan melintang (Y, Z) dinolkan di seluruh node kedua face
/// ujung sementara perpindahan aksial bebas, sehingga penampang ujung bebas
/// berotasi. Satu node di tengah menahan gerak tegar aksial. Kekangan ini
/// tidak bisa dinyatakan dengan fixture face (`Roller` mengunci arah
/// normal), jadi kasus ini memakai `NodeConstraint`.
pub fn euler_column(settings: MeshSettings) -> Result<EulerColumn, SimError> {
    let (length, width, thickness, force) = (100.0, 10.0, 5.0, 1000.0);
    let material = steel();
    let push = |id: &str, face: u32, fx: f64| ResolvedLoad {
        id: id.into(),
        faces: vec![face],
        kind: LoadKind::Force {
            newton: [fx, 0.0, 0.0],
        },
    };
    let setup = ResolvedSetup {
        fixtures: Vec::new(),
        loads: vec![
            push("left", FACE_XMIN, force),
            push("right", FACE_XMAX, -force),
        ],
        mesh: settings,
        exact_volume_mm3: Some(length * width * thickness),
    };
    let cancel = CancelToken::new();
    let surface = box_surface([length, width, thickness]);
    let mesh = build_mesh(&surface, &setup.mesh, setup.exact_volume_mm3, &[], &cancel)?;
    let mut extra = Vec::new();
    for face in [FACE_XMIN, FACE_XMAX] {
        for node in mesh.face_nodes(face) {
            extra.push(NodeConstraint { node, axis: 1 });
            extra.push(NodeConstraint { node, axis: 2 });
        }
    }
    extra.push(NodeConstraint {
        node: nearest_node(&mesh, [length / 2.0, width / 2.0, thickness / 2.0]),
        axis: 0,
    });
    let report = solve_buckling_on(&mesh, &material, &setup, &extra, 2, &cancel)?;
    let inertia = width * thickness.powi(3) / 12.0;
    let critical_load = BenchValue {
        measured: report.load_factors.first().copied().unwrap_or(f64::NAN) * force,
        analytic: std::f64::consts::PI.powi(2) * material.young_mpa * inertia / (length * length),
    };
    Ok(EulerColumn {
        critical_load,
        report,
    })
}

/// Hasil benchmark batang termal.
#[derive(Debug, Clone, PartialEq)]
pub struct ThermalRod {
    /// Selisih terbesar suhu nodal terhadap profil linier eksak, K.
    pub profile_error: f64,
    /// Von Mises maksimum pada pemuaian bebas (seharusnya nol), MPa.
    pub free_expansion_stress: f64,
    /// Perpanjangan ujung vs `α·∫ΔT dx`, mm.
    pub elongation: BenchValue,
    pub thermal: ThermalReport,
    pub stress: SimReport,
}

/// Batang 100×10×10 mm, suhu `t_cold` di −X dan `t_hot` di +X, sisi lain
/// terisolasi → profil linier. Lalu pemuaian bebas: hanya enam DOF yang
/// dikunci (tiga node sudut), sehingga tidak boleh timbul tegangan.
pub fn thermal_rod(
    settings: MeshSettings,
    t_cold: f64,
    t_hot: f64,
) -> Result<ThermalRod, SimError> {
    let (length, side) = (100.0, 10.0);
    let material = steel();
    let thermal_material = steel_thermal();
    let setup = ResolvedSetup {
        fixtures: Vec::new(),
        loads: Vec::new(),
        mesh: settings,
        exact_volume_mm3: Some(length * side * side),
    };
    let temperature = |id: &str, face: u32, celsius: f64| ResolvedThermalBc {
        id: id.into(),
        faces: vec![face],
        kind: ThermalBcKind::Temperature { celsius },
    };
    let thermal_setup = ResolvedThermalSetup {
        boundary: vec![
            temperature("cold", FACE_XMIN, t_cold),
            temperature("hot", FACE_XMAX, t_hot),
        ],
    };
    let cancel = CancelToken::new();
    let surface = box_surface([length, side, side]);
    let mesh = build_mesh(
        &surface,
        &setup.mesh,
        setup.exact_volume_mm3,
        &[FACE_XMIN, FACE_XMAX],
        &cancel,
    )?;
    let thermal = solve_thermal_on(&mesh, &thermal_material, &thermal_setup, &cancel)?;
    let field = thermal
        .nodal_field
        .as_ref()
        .ok_or_else(|| SimError::InvalidSetup("thermal report has no nodal field".into()))?;
    let mut profile_error = 0.0_f64;
    for (p, &t) in field.nodes.iter().zip(&field.values) {
        let exact = t_cold + (t_hot - t_cold) * p[0] / length;
        profile_error = profile_error.max((t - exact).abs());
    }
    // Enam kekangan minimum: tidak menghalangi pemuaian.
    let a = nearest_node(&mesh, [0.0, 0.0, 0.0]);
    let b = nearest_node(&mesh, [length, 0.0, 0.0]);
    let c = nearest_node(&mesh, [0.0, side, 0.0]);
    let extra = [
        NodeConstraint { node: a, axis: 0 },
        NodeConstraint { node: a, axis: 1 },
        NodeConstraint { node: a, axis: 2 },
        NodeConstraint { node: b, axis: 1 },
        NodeConstraint { node: b, axis: 2 },
        NodeConstraint { node: c, axis: 2 },
    ];
    let stress = solve_thermal_stress_on(
        &mesh,
        &material,
        &thermal_material,
        &setup,
        &thermal_setup,
        &extra,
        &cancel,
    )?;
    let tip = stress
        .nodal_field
        .as_ref()
        .map(|f| f.displacement[b as usize][0])
        .unwrap_or(f64::NAN);
    let mean_rise = 0.5 * (t_cold + t_hot) - thermal_material.reference_temperature_c;
    Ok(ThermalRod {
        profile_error,
        free_expansion_stress: stress.max_von_mises_mpa,
        elongation: BenchValue {
            measured: tip,
            analytic: thermal_material.expansion_per_k * mean_rise * length,
        },
        thermal,
        stress,
    })
}
