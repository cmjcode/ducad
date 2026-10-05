//! Kasus benchmark dengan solusi analitik. Tiap fungsi menjalankan studi
//! dan mengembalikan pasangan (terukur, analitik); toleransinya ditegakkan
//! di `tests/benchmarks.rs`.

use super::{box_surface, plate_with_hole, FACE_XMAX, FACE_XMIN, FACE_ZMAX};
use crate::bc::NodeConstraint;
use crate::mesh::voxel::voxelize;
use crate::setup::{
    FixtureKind, LoadKind, MeshKind, MeshSettings, ResolvedFixture, ResolvedLoad, ResolvedSetup,
};
use crate::{run_static, solve_static, CancelToken, ElasticMaterial, SimError, SimReport};

/// Nilai terukur vs analitik.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BenchValue {
    pub measured: f64,
    pub analytic: f64,
}

impl BenchValue {
    /// Galat relatif bertanda `(terukur − analitik) / analitik`.
    pub fn rel_error(&self) -> f64 {
        (self.measured - self.analytic) / self.analytic
    }
}

/// Baja struktural umum: E = 210 GPa, ν = 0.3, ρ = 7.85 g/cm³, σ_y = 250 MPa.
pub fn steel() -> ElasticMaterial {
    ElasticMaterial {
        young_mpa: 210_000.0,
        poisson: 0.3,
        density_g_cm3: 7.85,
        yield_mpa: 250.0,
    }
}

/// Mesh hex voxel dengan ukuran sel `cell_mm`.
pub fn mesh(cell_mm: f64) -> MeshSettings {
    MeshSettings {
        cell_mm: Some(cell_mm),
        ..MeshSettings::default()
    }
}

/// Mesh Tet10 dengan ukuran tepi target `size_mm`.
pub fn tet(size_mm: f64) -> MeshSettings {
    MeshSettings {
        kind: MeshKind::Tet,
        cell_mm: Some(size_mm),
        target_elems: None,
    }
}

fn fixed(face: u32) -> ResolvedFixture {
    ResolvedFixture {
        id: "fixed".into(),
        faces: vec![face],
        kind: FixtureKind::Fixed,
    }
}

fn field_of(report: &SimReport) -> Result<&crate::Field, SimError> {
    report
        .nodal_field
        .as_ref()
        .ok_or_else(|| SimError::InvalidSetup("report has no nodal field".into()))
}

/// Hasil batang tarik.
#[derive(Debug, Clone, PartialEq)]
pub struct TensionBar {
    /// Tegangan aksial di tengah bentang, MPa.
    pub stress: BenchValue,
    /// Perpanjangan ujung, mm.
    pub elongation: BenchValue,
    pub report: SimReport,
}

/// Batang 100×10×10 mm (sumbu X), dijepit di −X, 1000 N tarik di +X.
pub fn tension_bar(cell_mm: f64) -> Result<TensionBar, SimError> {
    let (length, side, force) = (100.0, 10.0, 1000.0);
    let material = steel();
    let setup = ResolvedSetup {
        fixtures: vec![fixed(FACE_XMIN)],
        loads: vec![ResolvedLoad {
            id: "pull".into(),
            faces: vec![FACE_XMAX],
            kind: LoadKind::Force {
                newton: [force, 0.0, 0.0],
            },
        }],
        mesh: mesh(cell_mm),
        exact_volume_mm3: Some(length * side * side),
    };
    let report = run_static(
        &box_surface([length, side, side]),
        &material,
        &setup,
        &CancelToken::new(),
    )?;
    let field = field_of(&report)?;
    let area = side * side;
    let stress = BenchValue {
        measured: field
            .sample([length / 2.0, side / 2.0, side / 2.0])
            .von_mises_mpa,
        analytic: force / area,
    };
    // Rata-rata perpindahan aksial di penampang ujung.
    let tip: Vec<f64> = field
        .nodes
        .iter()
        .zip(&field.displacement)
        .filter(|(p, _)| (p[0] - length).abs() < 1e-9)
        .map(|(_, u)| u[0])
        .collect();
    let elongation = BenchValue {
        measured: tip.iter().sum::<f64>() / tip.len().max(1) as f64,
        analytic: force * length / (material.young_mpa * area),
    };
    Ok(TensionBar {
        stress,
        elongation,
        report,
    })
}

/// Hasil kantilever.
#[derive(Debug, Clone, PartialEq)]
pub struct Cantilever {
    /// Defleksi ujung vs Euler–Bernoulli `FL³/(3EI)`, mm.
    pub deflection: BenchValue,
    /// Von Mises maksimum model vs tegangan lentur akar `FLc/I`, MPa.
    pub max_stress: BenchValue,
    /// Tegangan serat atas di penampang `x = section_x` vs `F(L−x)c/I`, MPa.
    pub section_stress: BenchValue,
    pub section_x: f64,
    pub report: SimReport,
}

/// Balok 100×10×10 mm dijepit di −X, 100 N ke −Z di ujung +X.
pub fn cantilever(cell_mm: f64, poisson: f64) -> Result<Cantilever, SimError> {
    cantilever_with(mesh(cell_mm), poisson)
}

/// Kantilever yang sama dengan pengaturan mesh sembarang (hex atau tet).
pub fn cantilever_with(settings: MeshSettings, poisson: f64) -> Result<Cantilever, SimError> {
    let (length, side, force) = (100.0, 10.0, 100.0);
    let material = ElasticMaterial { poisson, ..steel() };
    let setup = ResolvedSetup {
        fixtures: vec![fixed(FACE_XMIN)],
        loads: vec![ResolvedLoad {
            id: "tip".into(),
            faces: vec![FACE_XMAX],
            kind: LoadKind::Force {
                newton: [0.0, 0.0, -force],
            },
        }],
        mesh: settings,
        exact_volume_mm3: Some(length * side * side),
    };
    let report = run_static(
        &box_surface([length, side, side]),
        &material,
        &setup,
        &CancelToken::new(),
    )?;
    let field = field_of(&report)?;
    let inertia = side.powi(4) / 12.0;
    let c = side / 2.0;
    let deflection = BenchValue {
        measured: -field.sample([length, side / 2.0, side / 2.0]).displacement[2],
        analytic: force * length.powi(3) / (3.0 * material.young_mpa * inertia),
    };
    let max_stress = BenchValue {
        measured: report.max_von_mises_mpa,
        analytic: force * length * c / inertia,
    };
    let section_x = side;
    let section_stress = BenchValue {
        measured: field.sample([section_x, side / 2.0, side]).von_mises_mpa,
        analytic: force * (length - section_x) * c / inertia,
    };
    Ok(Cantilever {
        deflection,
        max_stress,
        section_stress,
        section_x,
        report,
    })
}

/// Hasil pelat bertekanan.
#[derive(Debug, Clone, PartialEq)]
pub struct PressurePlate {
    /// Defleksi tengah vs Timoshenko `0.00406·q·a⁴/D`, mm.
    pub deflection: BenchValue,
    pub report: SimReport,
}

/// Pelat persegi `side × side × thickness`, tumpuan sederhana di empat tepi
/// bawah (w = 0), tekanan merata `pressure_mpa` di face atas.
///
/// Tumpuan sederhana adalah kekangan *tepi*, yang tidak bisa dinyatakan
/// lewat fixture berbasis face; karena itu kasus ini memakai
/// [`solve_static`] dengan [`NodeConstraint`]. Gerak benda tegar di bidang
/// pelat dikunci dengan syarat simetri di dua bidang tengah.
pub fn pressure_plate(
    side: f64,
    thickness: f64,
    cell_mm: f64,
    pressure_mpa: f64,
) -> Result<PressurePlate, SimError> {
    let material = steel();
    let setup = ResolvedSetup {
        fixtures: Vec::new(),
        loads: vec![ResolvedLoad {
            id: "pressure".into(),
            faces: vec![FACE_ZMAX],
            kind: LoadKind::Pressure { mpa: pressure_mpa },
        }],
        mesh: mesh(cell_mm),
        exact_volume_mm3: Some(side * side * thickness),
    };
    let cancel = CancelToken::new();
    let model = voxelize(
        &box_surface([side, side, thickness]),
        &setup.mesh,
        setup.exact_volume_mm3,
        &cancel,
    )?;
    let tol = 1e-6 * side;
    let mut extra = Vec::new();
    for (n, p) in model.mesh.nodes.iter().enumerate() {
        let on_edge =
            p[0] < tol || p[1] < tol || (p[0] - side).abs() < tol || (p[1] - side).abs() < tol;
        if p[2] < tol && on_edge {
            extra.push(NodeConstraint {
                node: n as u32,
                axis: 2,
            });
        }
        if (p[0] - side / 2.0).abs() < tol {
            extra.push(NodeConstraint {
                node: n as u32,
                axis: 0,
            });
        }
        if (p[1] - side / 2.0).abs() < tol {
            extra.push(NodeConstraint {
                node: n as u32,
                axis: 1,
            });
        }
    }
    let report = solve_static(&model, &material, &setup, &extra, &cancel)?;
    let field = field_of(&report)?;
    let rigidity = material.young_mpa * thickness.powi(3)
        / (12.0 * (1.0 - material.poisson * material.poisson));
    let deflection = BenchValue {
        measured: -field
            .sample([side / 2.0, side / 2.0, thickness / 2.0])
            .displacement[2],
        analytic: 0.00406 * pressure_mpa * side.powi(4) / rigidity,
    };
    Ok(PressurePlate { deflection, report })
}

/// Hasil pelat berlubang.
#[derive(Debug, Clone, PartialEq)]
pub struct HolePlate {
    /// Faktor konsentrasi tegangan terhadap tegangan bruto `F/(W·t)`.
    /// Analitik: rumus Heywood/Howland untuk lebar hingga (≈ 3 untuk d/W kecil).
    pub concentration: BenchValue,
    pub report: SimReport,
}

/// Pelat `length × width × thickness` berlubang tengah diameter `hole`,
/// dijepit di −X dan ditarik di +X.
pub fn hole_plate(size: [f64; 3], hole: f64, cell_mm: f64) -> Result<HolePlate, SimError> {
    hole_plate_with(size, hole, mesh(cell_mm), 128)
}

/// Pelat berlubang dengan pengaturan mesh sembarang; `segments` = jumlah
/// sisi poligon lubang pada mesh permukaan.
pub fn hole_plate_with(
    size: [f64; 3],
    hole: f64,
    settings: MeshSettings,
    segments: usize,
) -> Result<HolePlate, SimError> {
    let force = 1000.0;
    let material = steel();
    let exact = (size[0] * size[1] - std::f64::consts::PI * hole * hole / 4.0) * size[2];
    let setup = ResolvedSetup {
        fixtures: vec![fixed(FACE_XMIN)],
        loads: vec![ResolvedLoad {
            id: "pull".into(),
            faces: vec![FACE_XMAX],
            kind: LoadKind::Force {
                newton: [force, 0.0, 0.0],
            },
        }],
        mesh: settings,
        exact_volume_mm3: Some(exact),
    };
    let surface = plate_with_hole(size, hole, segments);
    let report = run_static(&surface, &material, &setup, &CancelToken::new())?;
    let field = field_of(&report)?;
    let nominal = force / (size[1] * size[2]);
    // Puncak di sekitar lubang saja (menjauhi singularitas jepitan).
    let peak = field
        .nodes
        .iter()
        .zip(&field.von_mises)
        .filter(|(p, _)| (p[0] - size[0] / 2.0).abs() < hole)
        .map(|(_, &v)| v)
        .fold(0.0_f64, f64::max);
    let q = 1.0 - hole / size[1];
    let kt_net = 2.0 + 0.284 * q - 0.6 * q * q + 1.32 * q * q * q;
    Ok(HolePlate {
        concentration: BenchValue {
            measured: peak / nominal,
            analytic: kt_net / q,
        },
        report,
    })
}
