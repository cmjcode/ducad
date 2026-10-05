//! Elemen hingga. [`ElementModel`] adalah antarmuka yang dipakai
//! `assemble`/`post`, sehingga tipe elemen kedua (Tet10 di P18) dan matriks
//! massa konsisten bisa ditambahkan tanpa mengubah modul-modul itu.

pub mod hex8;
pub mod tet10;

pub use hex8::Hex8;

use crate::mesh::{HexMesh, TetMesh};

/// Model elemen: konektivitas + matriks elemen. Tiga DOF per node
/// (`dof = 3·node + sumbu`).
pub trait ElementModel {
    fn num_nodes(&self) -> usize;
    fn num_elems(&self) -> usize;
    fn nodes_per_elem(&self) -> usize;
    fn elem_nodes(&self, e: usize) -> &[u32];
    /// Matriks kekakuan elemen `e`, row-major `(3·npe)²`, sudah termasuk bobot.
    fn stiffness(&self, e: usize, out: &mut [f64]);
    /// Tegangan (Voigt xx, yy, zz, xy, yz, zx; MPa) di tiap node lokal elemen
    /// dari perpindahan elemen `u_e` (panjang `3·npe`).
    fn nodal_stress(&self, e: usize, u_e: &[f64], out: &mut [[f64; 6]]);
    /// Bobot elemen saat merata-ratakan tegangan ke node.
    fn averaging_weight(&self, e: usize) -> f64;
    /// Massa konsisten skalar `npe²` untuk densitas 1 (sama di tiap komponen).
    fn mass(&self, e: usize, out: &mut [f64]);
    /// Konduksi skalar `npe²` untuk konduktivitas 1.
    fn conductivity(&self, e: usize, out: &mut [f64]);
    /// Kekakuan geometri skalar `npe²` dari tegangan di node lokal.
    fn geometric(&self, e: usize, stress: &[[f64; 6]], out: &mut [f64]);
    /// Beban nodal ekuivalen (`3·npe`) regangan termal `α·ΔT`; `dt` = ΔT di
    /// node lokal.
    fn thermal_load(&self, e: usize, alpha: f64, dt: &[f64], out: &mut [f64]);
    /// Faktor `E·α/(1 − 2ν)` per satuan α: tegangan termal yang dikurangkan
    /// dari komponen normal adalah `thermal_modulus()·α·ΔT`.
    fn thermal_modulus(&self) -> f64;
}

/// Model hex voxel: satu matriks elemen untuk semua sel, diskalakan bobot.
pub struct HexModel<'a> {
    pub mesh: &'a HexMesh,
    pub hex8: Hex8,
    /// `E/(1 − 2ν)`; hanya dipakai beban termal.
    pub thermal_modulus: f64,
}

impl<'a> HexModel<'a> {
    pub fn new(
        mesh: &'a HexMesh,
        young: f64,
        poisson: f64,
    ) -> Result<HexModel<'a>, crate::SimError> {
        Ok(HexModel {
            mesh,
            hex8: Hex8::new(mesh.cell, young, poisson)?,
            thermal_modulus: young / (1.0 - 2.0 * poisson),
        })
    }
}

/// Model Tet10: matriks elemen dihitung per elemen (isoparametrik).
pub struct TetElements<'a> {
    pub mesh: &'a TetMesh,
    lambda: f64,
    mu: f64,
    thermal_modulus: f64,
}

impl<'a> TetElements<'a> {
    pub fn new(
        mesh: &'a TetMesh,
        young: f64,
        poisson: f64,
    ) -> Result<TetElements<'a>, crate::SimError> {
        // Validasi material yang sama dengan elemen hex.
        Hex8::new([1.0; 3], young, poisson)?;
        Ok(TetElements {
            mesh,
            lambda: young * poisson / ((1.0 + poisson) * (1.0 - 2.0 * poisson)),
            mu: young / (2.0 * (1.0 + poisson)),
            thermal_modulus: young / (1.0 - 2.0 * poisson),
        })
    }
}

impl ElementModel for TetElements<'_> {
    fn num_nodes(&self) -> usize {
        self.mesh.nodes.len()
    }

    fn num_elems(&self) -> usize {
        self.mesh.elems.len()
    }

    fn nodes_per_elem(&self) -> usize {
        10
    }

    fn elem_nodes(&self, e: usize) -> &[u32] {
        &self.mesh.elems[e]
    }

    fn stiffness(&self, e: usize, out: &mut [f64]) {
        tet10::stiffness(&self.mesh.elem_coords(e), self.lambda, self.mu, out);
    }

    fn nodal_stress(&self, e: usize, u_e: &[f64], out: &mut [[f64; 6]]) {
        tet10::nodal_stress(&self.mesh.elem_coords(e), self.lambda, self.mu, u_e, out);
    }

    fn averaging_weight(&self, _e: usize) -> f64 {
        1.0
    }

    fn mass(&self, e: usize, out: &mut [f64]) {
        tet10::mass(&self.mesh.elem_coords(e), out);
    }

    fn conductivity(&self, e: usize, out: &mut [f64]) {
        tet10::conductivity(&self.mesh.elem_coords(e), out);
    }

    fn geometric(&self, e: usize, stress: &[[f64; 6]], out: &mut [f64]) {
        tet10::geometric(&self.mesh.elem_coords(e), stress, out);
    }

    fn thermal_load(&self, e: usize, alpha: f64, dt: &[f64], out: &mut [f64]) {
        tet10::thermal_load(
            &self.mesh.elem_coords(e),
            self.thermal_modulus * alpha,
            dt,
            out,
        );
    }

    fn thermal_modulus(&self) -> f64 {
        self.thermal_modulus
    }
}

impl ElementModel for HexModel<'_> {
    fn num_nodes(&self) -> usize {
        self.mesh.nodes.len()
    }

    fn num_elems(&self) -> usize {
        self.mesh.elems.len()
    }

    fn nodes_per_elem(&self) -> usize {
        8
    }

    fn elem_nodes(&self, e: usize) -> &[u32] {
        &self.mesh.elems[e]
    }

    fn stiffness(&self, e: usize, out: &mut [f64]) {
        let w = self.mesh.weight[e];
        for (o, k) in out.iter_mut().zip(self.hex8.stiffness()) {
            *o = w * k;
        }
    }

    fn nodal_stress(&self, _e: usize, u_e: &[f64], out: &mut [[f64; 6]]) {
        self.hex8.nodal_stress(u_e, out);
    }

    fn averaging_weight(&self, e: usize) -> f64 {
        self.mesh.weight[e]
    }

    fn mass(&self, e: usize, out: &mut [f64]) {
        let w = self.mesh.weight[e];
        for (o, m) in out.iter_mut().zip(self.hex8.mass()) {
            *o = w * m;
        }
    }

    fn conductivity(&self, e: usize, out: &mut [f64]) {
        let w = self.mesh.weight[e];
        for (o, c) in out.iter_mut().zip(self.hex8.conductivity()) {
            *o = w * c;
        }
    }

    fn geometric(&self, e: usize, stress: &[[f64; 6]], out: &mut [f64]) {
        self.hex8.geometric(stress, out);
        let w = self.mesh.weight[e];
        for o in out.iter_mut() {
            *o *= w;
        }
    }

    fn thermal_load(&self, e: usize, alpha: f64, dt: &[f64], out: &mut [f64]) {
        self.hex8
            .thermal_load(self.thermal_modulus * alpha * self.mesh.weight[e], dt, out);
    }

    fn thermal_modulus(&self) -> f64 {
        self.thermal_modulus
    }
}
