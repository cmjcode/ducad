//! `ducad-sim` — simulasi statik linier (FEA) di atas mesh heksahedral voxel.
//!
//! Crate ini sengaja berdiri sendiri: tanpa kernel CAD, tanpa GUI. Masukan
//! hanya mesh permukaan tertutup ([`SurfaceMesh`]) dengan tag face B-rep per
//! segitiga, material elastis, dan setup yang selector-nya sudah diselesaikan
//! engine menjadi indeks face ([`ResolvedSetup`]).
//!
//! Satuan: mm, N, MPa (N/mm²), densitas g/cm³, gravitasi m/s².
//! Semua angka FE `f64`; hasil deterministik (tanpa thread, tanpa RNG, tanpa
//! ketergantungan urutan iterasi `HashMap`).
#![forbid(unsafe_code)]
// Kode numerik padat memakai indeks eksplisit agar rumusnya terbaca.
#![allow(clippy::needless_range_loop)]

pub mod assemble;
pub mod bc;
pub mod benchmark;
pub mod element;
pub mod linalg;
pub mod mesh;
pub mod post;
pub mod report;
pub mod setup;
pub mod solver;

mod analysis;
mod pipeline;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub use analysis::{
    run_buckling, run_frequency, run_thermal, run_thermal_stress, solve_buckling_on,
    solve_frequency_on, solve_thermal_on, solve_thermal_stress_on, BucklingReport, FrequencyReport,
    ThermalReport,
};
pub use bc::NodeConstraint;
pub use mesh::tet::{tetrahedralize, TetFace, TetMesh, TetModel};
pub use mesh::voxel::{voxelize, VoxelModel};
pub use pipeline::{
    build_mesh, run_static, solve_static, solve_static_on, SimMesh, FALLBACK_WARNING,
};
pub use report::{
    Field, FieldSample, MeshStats, Reaction, ScalarField, SimReport, SolverStats, VectorField,
};
pub use setup::{
    Fixture, FixtureKind, Load, LoadKind, MeshKind, MeshSettings, ResolvedFixture, ResolvedLoad,
    ResolvedSetup, ResolvedThermalBc, ResolvedThermalSetup, SimSetup, ThermalBc, ThermalBcKind,
    ThermalSetup,
};

/// Sifat termal material (terpisah dari [`ElasticMaterial`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThermalMaterial {
    /// Konduktivitas termal, W/(m·K).
    pub conductivity_w_mk: f64,
    /// Koefisien muai panjang, 1/K.
    pub expansion_per_k: f64,
    /// Suhu acuan tanpa regangan termal, °C.
    pub reference_temperature_c: f64,
}

/// Mesh permukaan tertutup dengan normal mengarah keluar.
/// `tri_face[i]` = indeks face B-rep pemilik segitiga `i`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SurfaceMesh {
    pub positions: Vec<[f64; 3]>,
    pub triangles: Vec<[u32; 3]>,
    pub tri_face: Vec<u32>,
}

/// Material elastis linier isotropik.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElasticMaterial {
    /// Modulus Young, MPa.
    pub young_mpa: f64,
    /// Rasio Poisson.
    pub poisson: f64,
    /// Densitas, g/cm³ (hanya dipakai beban gravitasi).
    pub density_g_cm3: f64,
    /// Tegangan luluh, MPa (untuk faktor keamanan).
    pub yield_mpa: f64,
}

/// Token pembatalan yang bisa dibagi antar-thread; solver memeriksanya
/// setiap 50 iterasi.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Galat simulasi. Pesan dan `hint` berbahasa Inggris karena dibaca agent.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SimError {
    #[error("model is under-constrained: {detail}")]
    Underconstrained {
        /// Number of degrees of freedom that are not held by any fixture.
        free_dofs: usize,
        detail: String,
    },
    #[error("mesh too coarse ({elements} elements, cell {cell_mm:.4} mm): {detail}")]
    MeshTooCoarse {
        cell_mm: f64,
        elements: usize,
        detail: String,
    },
    #[error(
        "solver did not converge after {iterations} iterations (relative residual {residual:.3e})"
    )]
    Diverged { iterations: usize, residual: f64 },
    #[error("simulation cancelled")]
    Cancelled,
    #[error("invalid simulation setup: {0}")]
    InvalidSetup(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
}

impl SimError {
    /// Kode stabil untuk `ERROR_GUIDE`.
    pub fn code(&self) -> &'static str {
        match self {
            SimError::Underconstrained { .. } => "SIM_UNDERCONSTRAINED",
            SimError::MeshTooCoarse { .. } => "SIM_MESH_TOO_COARSE",
            SimError::Diverged { .. } => "SIM_DIVERGED",
            SimError::Cancelled => "SIM_CANCELLED",
            SimError::InvalidSetup(_) => "SIM_INVALID_SETUP",
            SimError::Unsupported(_) => "SIM_UNSUPPORTED",
        }
    }

    /// Saran perbaikan singkat untuk agent.
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            SimError::Underconstrained { .. } => Some(
                "add a Fixed fixture on at least one face; Roller/Symmetry fixtures alone must \
                 block all six rigid-body motions",
            ),
            SimError::MeshTooCoarse { .. } => Some(
                "reduce mesh.cell_mm or raise mesh.target_elems so thin walls span at least one \
                 cell and every loaded or fixed face is captured",
            ),
            SimError::Diverged { .. } => Some(
                "check for nearly disconnected or very thin regions, then retry with a finer \
                 mesh (smaller mesh.cell_mm)",
            ),
            SimError::Cancelled => None,
            SimError::InvalidSetup(_) => Some(
                "check face selectors, load magnitudes and directions, and material values (E > \
                 0, -1 < nu < 0.5)",
            ),
            SimError::Unsupported(_) => Some("check the study kind and mesh.kind (\"hex\" or \"tet\")"),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn sim_has_no_kernel_or_gui_dependency() {
        let manifest = include_str!("../Cargo.toml");
        // Hanya bagian dependensi yang diperiksa; komentar boleh menyebut nama.
        let deps: String = manifest
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n");
        for banned in ["opencascade", "ducad-kernel", "egui", "eframe", "wgpu"] {
            assert!(
                !deps.contains(banned),
                "ducad-sim tidak boleh bergantung pada {banned}"
            );
        }
    }

    #[test]
    fn sim_error_codes_are_ascii_and_stable() {
        use super::SimError;
        let all = [
            SimError::Underconstrained {
                free_dofs: 3,
                detail: "x".into(),
            },
            SimError::MeshTooCoarse {
                cell_mm: 1.0,
                elements: 0,
                detail: "x".into(),
            },
            SimError::Diverged {
                iterations: 1,
                residual: 1.0,
            },
            SimError::Cancelled,
            SimError::InvalidSetup("x".into()),
            SimError::Unsupported("x".into()),
        ];
        let codes: Vec<_> = all.iter().map(SimError::code).collect();
        assert_eq!(
            codes,
            [
                "SIM_UNDERCONSTRAINED",
                "SIM_MESH_TOO_COARSE",
                "SIM_DIVERGED",
                "SIM_CANCELLED",
                "SIM_INVALID_SETUP",
                "SIM_UNSUPPORTED"
            ]
        );
        for e in &all {
            assert!(e.to_string().is_ascii());
            assert!(e.hint().is_none_or(str::is_ascii));
        }
    }
}
