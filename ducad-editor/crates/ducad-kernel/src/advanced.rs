//! Operasi lanjutan untuk agent (P14): loft lintas bidang, mirror bidang,
//! draft dan fillet variabel berbasis indeks topologi (tanpa ray picking).

use glam::{dvec3, DVec3};

use crate::profile::Profile;
use crate::shape::KernelShape;

// Implementasi khusus backend (OCCT/truck) — lihat `crate::backend`.
pub use crate::backend::advanced::*;

/// Satu penampang loft: profil 2D pada bidang (origin, u, v, normal).
#[derive(Debug, Clone)]
pub struct LoftSection {
    pub profile: Profile,
    pub origin: [f64; 3],
    pub u_axis: [f64; 3],
    pub v_axis: [f64; 3],
    pub normal: [f64; 3],
}

/// Titik pusat bbox kasar dari pusat massa face (untuk tes dan pivot).
pub fn faces_centroid(shape: &KernelShape) -> DVec3 {
    let faces = crate::topo::enumerate_faces(shape);
    if faces.is_empty() {
        return DVec3::ZERO;
    }
    let sum = faces.iter().fold(DVec3::ZERO, |acc, f| {
        acc + dvec3(f.centroid[0], f.centroid[1], f.centroid[2])
    });
    sum / faces.len() as f64
}
