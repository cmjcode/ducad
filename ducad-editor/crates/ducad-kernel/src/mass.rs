//! Properti massa (P16): volume, pusat massa, dan tensor inersia eksak dari
//! B-rep, plus momen/sumbu utama. Semua nilai di sini GEOMETRIS (densitas
//! 1) — pemanggil mengalikan densitas material sendiri.

use crate::lock_kernel;
use crate::shape::KernelShape;

/// Properti volume geometris sebuah solid (densitas 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MassProperties {
    pub volume_mm3: f64,
    pub centroid: [f64; 3],
    /// Tensor inersia geometris (densitas 1) terhadap origin, mm^5.
    /// Elemen luar-diagonal adalah produk inersia BERTANDA NEGATIF
    /// (`I_xy = -∫xy dV`), sehingga `nᵀ·I·n` = momen terhadap sumbu `n`.
    pub inertia_origin: [[f64; 3]; 3],
}

impl MassProperties {
    /// Tensor inersia terhadap pusat massa (teorema sumbu sejajar), mm^5.
    pub fn inertia_com(&self) -> [[f64; 3]; 3] {
        shift_to_centroid(self.inertia_origin, self.volume_mm3, self.centroid)
    }

    /// Momen utama (naik) dan sumbu utamanya (baris = sumbu satuan)
    /// terhadap pusat massa.
    pub fn principal(&self) -> ([f64; 3], [[f64; 3]; 3]) {
        principal_axes(self.inertia_com())
    }
}

impl KernelShape {
    /// Volume, pusat massa, dan tensor inersia eksak (`BRepGProp`).
    /// Volume dinormalkan positif; solid berorientasi terbalik tetap
    /// menghasilkan tensor positif.
    pub fn mass_properties(&self) -> MassProperties {
        let (volume, centroid, inertia) = {
            let _guard = lock_kernel();
            self.inner().volume_properties()
        };
        let sign = if volume < 0.0 { -1.0 } else { 1.0 };
        MassProperties {
            volume_mm3: volume * sign,
            centroid: [centroid.x, centroid.y, centroid.z],
            inertia_origin: inertia.map(|row| row.map(|v| v * sign)),
        }
    }
}

/// `I_com = I_o − m·(|c|²·E − c·cᵀ)`.
pub fn shift_to_centroid(inertia_origin: [[f64; 3]; 3], mass: f64, c: [f64; 3]) -> [[f64; 3]; 3] {
    let c2 = c[0] * c[0] + c[1] * c[1] + c[2] * c[2];
    let mut out = inertia_origin;
    for (i, row) in out.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            let delta = if i == j { c2 } else { 0.0 };
            *v -= mass * (delta - c[i] * c[j]);
        }
    }
    out
}

/// Kebalikan [`shift_to_centroid`]: pindahkan tensor di pusat massa ke
/// titik yang berjarak `d` dari pusat massa.
pub fn shift_from_centroid(inertia_com: [[f64; 3]; 3], mass: f64, d: [f64; 3]) -> [[f64; 3]; 3] {
    shift_to_centroid(inertia_com, -mass, d)
}

/// Eigen-dekomposisi matriks simetris 3×3 (Jacobi siklik). Mengembalikan
/// nilai eigen terurut naik dan vektor eigen satuan sebagai BARIS, dengan
/// tanda dinormalkan (komponen terbesar positif) supaya hasilnya stabil.
pub fn principal_axes(tensor: [[f64; 3]; 3]) -> ([f64; 3], [[f64; 3]; 3]) {
    let mut a = tensor;
    // Kolom `v` = vektor eigen.
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let scale = a
        .iter()
        .flatten()
        .fold(0.0_f64, |m, x| m.max(x.abs()))
        .max(f64::MIN_POSITIVE);
    for _ in 0..64 {
        let off = a[0][1].abs() + a[0][2].abs() + a[1][2].abs();
        if off <= 1e-15 * scale {
            break;
        }
        for (p, q) in [(0, 1), (0, 2), (1, 2)] {
            if a[p][q].abs() <= 1e-300 {
                continue;
            }
            let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
            let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
            let t = if theta == 0.0 { 1.0 } else { t };
            let c = 1.0 / (t * t + 1.0).sqrt();
            let s = t * c;
            for row in a.iter_mut() {
                let (akp, akq) = (row[p], row[q]);
                row[p] = c * akp - s * akq;
                row[q] = s * akp + c * akq;
            }
            let (rp, rq) = (a[p], a[q]);
            for k in 0..3 {
                a[p][k] = c * rp[k] - s * rq[k];
                a[q][k] = s * rp[k] + c * rq[k];
            }
            for row in v.iter_mut() {
                let (vkp, vkq) = (row[p], row[q]);
                row[p] = c * vkp - s * vkq;
                row[q] = s * vkp + c * vkq;
            }
        }
    }
    let mut order = [0usize, 1, 2];
    order.sort_by(|&i, &j| a[i][i].total_cmp(&a[j][j]));
    let mut moments = [0.0; 3];
    let mut axes = [[0.0; 3]; 3];
    for (slot, &k) in order.iter().enumerate() {
        moments[slot] = a[k][k];
        let mut axis = [v[0][k], v[1][k], v[2][k]];
        let dominant = axis
            .iter()
            .copied()
            .fold(0.0_f64, |m, x| if x.abs() > m.abs() { x } else { m });
        if dominant < 0.0 {
            axis = axis.map(|x| -x);
        }
        axes[slot] = axis;
    }
    (moments, axes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn principal_axes_of_diagonal_is_sorted_identity() {
        let (m, ax) = principal_axes([[3.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 2.0]]);
        assert_eq!(m, [1.0, 2.0, 3.0]);
        assert_eq!(ax[0], [0.0, 1.0, 0.0]);
        assert_eq!(ax[2], [1.0, 0.0, 0.0]);
    }

    #[test]
    fn principal_axes_reconstructs_tensor() {
        let t = [[4.0, 1.0, -0.5], [1.0, 3.0, 0.25], [-0.5, 0.25, 5.0]];
        let (m, ax) = principal_axes(t);
        for i in 0..3 {
            for j in 0..3 {
                let rebuilt: f64 = (0..3).map(|k| m[k] * ax[k][i] * ax[k][j]).sum();
                assert!((rebuilt - t[i][j]).abs() < 1e-12, "({i},{j}) {rebuilt}");
            }
        }
    }

    #[test]
    fn parallel_axis_roundtrip() {
        let com = [[2.0, 0.1, 0.0], [0.1, 3.0, 0.2], [0.0, 0.2, 4.0]];
        let d = [1.0, -2.0, 0.5];
        let back = shift_to_centroid(shift_from_centroid(com, 7.0, d), 7.0, d);
        for i in 0..3 {
            for j in 0..3 {
                assert!((back[i][j] - com[i][j]).abs() < 1e-12);
            }
        }
    }
}
