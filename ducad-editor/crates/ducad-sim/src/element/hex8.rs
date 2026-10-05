//! Elemen heksahedral 8-node trilinear untuk sel balok sejajar sumbu.
//!
//! Integrasi Gauss 2×2×2. Untuk menghilangkan *shear locking* pada lentur
//! (dinding tipis yang hanya 1–3 sel tebalnya), elemen diperkaya sembilan
//! mode inkompatibel Wilson–Taylor (`1 − ξ²`, `1 − η²`, `1 − ζ²` per komponen
//! perpindahan) yang dikondensasi statik di tingkat elemen. Karena Jacobian
//! sel konstan, elemen ini lolos patch test tanpa modifikasi dan tetap punya
//! tepat enam mode benda tegar.
//!
//! Node lokal `n` berada di `ξ = (±1, ±1, ±1)` dengan tanda dari bit `n`.

use crate::linalg::solve_dense;
use crate::SimError;

const GAUSS: f64 = 0.577_350_269_189_625_8;
const NDOF: usize = 24;
const NENH: usize = 9;

/// Tanda koordinat lokal node `n` pada sumbu `a`.
fn sign(n: usize, a: usize) -> f64 {
    if (n >> a) & 1 == 1 {
        1.0
    } else {
        -1.0
    }
}

/// Matriks elastisitas isotropik 6×6 (Voigt xx, yy, zz, xy, yz, zx; geser teknik).
pub fn elasticity_matrix(young: f64, poisson: f64) -> [[f64; 6]; 6] {
    let lambda = young * poisson / ((1.0 + poisson) * (1.0 - 2.0 * poisson));
    let mu = young / (2.0 * (1.0 + poisson));
    let mut d = [[0.0; 6]; 6];
    for i in 0..3 {
        for j in 0..3 {
            d[i][j] = lambda;
        }
        d[i][i] = lambda + 2.0 * mu;
        d[i + 3][i + 3] = mu;
    }
    d
}

/// Mengisi kolom regangan untuk satu "fungsi bentuk" dengan gradien `grad`,
/// mulai kolom `col` (tiga kolom: komponen u, v, w).
fn fill_strain_columns<const N: usize>(m: &mut [[f64; N]; 6], col: usize, grad: [f64; 3]) {
    m[0][col] = grad[0];
    m[1][col + 1] = grad[1];
    m[2][col + 2] = grad[2];
    m[3][col] = grad[1];
    m[3][col + 1] = grad[0];
    m[4][col + 1] = grad[2];
    m[4][col + 2] = grad[1];
    m[5][col] = grad[2];
    m[5][col + 2] = grad[0];
}

/// Gradien fisik delapan fungsi bentuk trilinear di titik lokal `xi`.
fn shape_gradients(cell: [f64; 3], xi: [f64; 3]) -> [[f64; 3]; 8] {
    let mut out = [[0.0; 3]; 8];
    for n in 0..8 {
        let f = [
            1.0 + sign(n, 0) * xi[0],
            1.0 + sign(n, 1) * xi[1],
            1.0 + sign(n, 2) * xi[2],
        ];
        out[n] = [
            2.0 / cell[0] * sign(n, 0) * f[1] * f[2] / 8.0,
            2.0 / cell[1] * sign(n, 1) * f[0] * f[2] / 8.0,
            2.0 / cell[2] * sign(n, 2) * f[0] * f[1] / 8.0,
        ];
    }
    out
}

/// Matriks regangan–perpindahan B (6×24) di titik lokal `xi`.
fn b_matrix(cell: [f64; 3], xi: [f64; 3]) -> [[f64; NDOF]; 6] {
    let mut b = [[0.0; NDOF]; 6];
    for n in 0..8 {
        let f = [
            1.0 + sign(n, 0) * xi[0],
            1.0 + sign(n, 1) * xi[1],
            1.0 + sign(n, 2) * xi[2],
        ];
        let grad = [
            2.0 / cell[0] * sign(n, 0) * f[1] * f[2] / 8.0,
            2.0 / cell[1] * sign(n, 1) * f[0] * f[2] / 8.0,
            2.0 / cell[2] * sign(n, 2) * f[0] * f[1] / 8.0,
        ];
        fill_strain_columns(&mut b, 3 * n, grad);
    }
    b
}

/// Matriks regangan mode inkompatibel G (6×9) di titik lokal `xi`.
fn g_matrix(cell: [f64; 3], xi: [f64; 3]) -> [[f64; NENH]; 6] {
    let mut g = [[0.0; NENH]; 6];
    for m in 0..3 {
        let mut grad = [0.0; 3];
        grad[m] = -2.0 * xi[m] * 2.0 / cell[m];
        fill_strain_columns(&mut g, 3 * m, grad);
    }
    g
}

/// Matriks elemen hex8 untuk satu ukuran sel dan satu material.
#[derive(Debug, Clone, PartialEq)]
pub struct Hex8 {
    /// Kekakuan terkondensasi 24×24, row-major.
    ke: Vec<f64>,
    /// Operator tegangan di delapan node: `8 × 6 × 24`, row-major.
    stress: Vec<f64>,
    /// Ukuran sel.
    cell: [f64; 3],
    /// Massa konsisten skalar 8×8 untuk densitas 1.
    mass: Vec<f64>,
    /// Konduksi skalar 8×8 untuk konduktivitas 1.
    conduct: Vec<f64>,
    /// Operator beban termal `8 titik Gauss × 24`: jumlah tiga baris normal
    /// regangan (termasuk mode inkompatibel) × volume titik Gauss.
    thermal: Vec<f64>,
}

impl Hex8 {
    pub fn new(cell: [f64; 3], young: f64, poisson: f64) -> Result<Hex8, SimError> {
        if cell.iter().any(|&h| !(h.is_finite() && h > 0.0)) {
            return Err(SimError::InvalidSetup(
                "element size must be positive".into(),
            ));
        }
        if !(young.is_finite() && young > 0.0) {
            return Err(SimError::InvalidSetup(
                "material Young's modulus must be positive".into(),
            ));
        }
        if !(poisson.is_finite() && poisson > -0.999 && poisson < 0.4999) {
            return Err(SimError::InvalidSetup(
                "material Poisson ratio must be between -1 and 0.5 (exclusive)".into(),
            ));
        }
        let d = elasticity_matrix(young, poisson);
        let dv = cell[0] * cell[1] * cell[2] / 8.0;
        let mut kuu = vec![0.0; NDOF * NDOF];
        let mut kau = vec![0.0; NENH * NDOF];
        let mut kaa = vec![0.0; NENH * NENH];
        for gp in 0..8 {
            let xi = [
                sign(gp, 0) * GAUSS,
                sign(gp, 1) * GAUSS,
                sign(gp, 2) * GAUSS,
            ];
            let b = b_matrix(cell, xi);
            let g = g_matrix(cell, xi);
            let db = mat6_mul(&d, &b);
            let dg = mat6_mul(&d, &g);
            for i in 0..NDOF {
                for j in 0..NDOF {
                    let mut s = 0.0;
                    for r in 0..6 {
                        s += b[r][i] * db[r][j];
                    }
                    kuu[i * NDOF + j] += s * dv;
                }
            }
            for i in 0..NENH {
                for j in 0..NDOF {
                    let mut s = 0.0;
                    for r in 0..6 {
                        s += g[r][i] * db[r][j];
                    }
                    kau[i * NDOF + j] += s * dv;
                }
                for j in 0..NENH {
                    let mut s = 0.0;
                    for r in 0..6 {
                        s += g[r][i] * dg[r][j];
                    }
                    kaa[i * NENH + j] += s * dv;
                }
            }
        }
        // X = Kaa⁻¹·Kau, lalu K = Kuu − Kauᵀ·X.
        let mut x = kau.clone();
        if !solve_dense(&mut kaa, NENH, &mut x, NDOF) {
            return Err(SimError::InvalidSetup(
                "element stiffness is singular for this material".into(),
            ));
        }
        let mut ke = kuu;
        for i in 0..NDOF {
            for j in 0..NDOF {
                let mut s = 0.0;
                for a in 0..NENH {
                    s += kau[a * NDOF + i] * x[a * NDOF + j];
                }
                ke[i * NDOF + j] -= s;
            }
        }
        // Simetri tepat (menghapus galat pembulatan).
        for i in 0..NDOF {
            for j in (i + 1)..NDOF {
                let avg = 0.5 * (ke[i * NDOF + j] + ke[j * NDOF + i]);
                ke[i * NDOF + j] = avg;
                ke[j * NDOF + i] = avg;
            }
        }
        // Operator tegangan di node: S = D·(B − G·X). Evaluasi langsung di
        // node sama dengan ekstrapolasi trilinear dari titik Gauss.
        let mut stress = vec![0.0; 8 * 6 * NDOF];
        for n in 0..8 {
            let xi = [sign(n, 0), sign(n, 1), sign(n, 2)];
            let b = b_matrix(cell, xi);
            let g = g_matrix(cell, xi);
            let mut strain = b;
            for r in 0..6 {
                for j in 0..NDOF {
                    let mut s = 0.0;
                    for a in 0..NENH {
                        s += g[r][a] * x[a * NDOF + j];
                    }
                    strain[r][j] -= s;
                }
            }
            let ds = mat6_mul(&d, &strain);
            for r in 0..6 {
                stress[(n * 6 + r) * NDOF..(n * 6 + r + 1) * NDOF].copy_from_slice(&ds[r]);
            }
        }
        // Besaran tambahan (frekuensi, termal, buckling); tidak memengaruhi `ke`.
        let volume = cell[0] * cell[1] * cell[2];
        let mut mass = vec![0.0; 64];
        for a in 0..8 {
            for b in 0..8 {
                // Per sumbu: ∫ N_a N_b = (h/4)·(1 + s_a·s_b/3).
                let mut m = volume / 64.0;
                for k in 0..3 {
                    m *= 1.0 + sign(a, k) * sign(b, k) / 3.0;
                }
                mass[a * 8 + b] = m;
            }
        }
        let mut conduct = vec![0.0; 64];
        let mut thermal = vec![0.0; 8 * NDOF];
        for gp in 0..8 {
            let xi = [
                sign(gp, 0) * GAUSS,
                sign(gp, 1) * GAUSS,
                sign(gp, 2) * GAUSS,
            ];
            let grads = shape_gradients(cell, xi);
            for a in 0..8 {
                for b in 0..8 {
                    conduct[a * 8 + b] += dv
                        * (grads[a][0] * grads[b][0]
                            + grads[a][1] * grads[b][1]
                            + grads[a][2] * grads[b][2]);
                }
            }
            let b = b_matrix(cell, xi);
            let g = g_matrix(cell, xi);
            for j in 0..NDOF {
                let mut sum = 0.0;
                for r in 0..3 {
                    let mut enhanced = b[r][j];
                    for a in 0..NENH {
                        enhanced -= g[r][a] * x[a * NDOF + j];
                    }
                    sum += enhanced;
                }
                thermal[gp * NDOF + j] = sum * dv;
            }
        }
        Ok(Hex8 {
            ke,
            stress,
            cell,
            mass,
            conduct,
            thermal,
        })
    }

    /// Massa konsisten skalar 8×8 (densitas 1, bobot 1).
    pub fn mass(&self) -> &[f64] {
        &self.mass
    }

    /// Konduksi skalar 8×8 (konduktivitas 1, bobot 1).
    pub fn conductivity(&self) -> &[f64] {
        &self.conduct
    }

    /// Kekakuan geometri skalar 8×8 dari tegangan di delapan node.
    pub fn geometric(&self, stress: &[[f64; 6]], out: &mut [f64]) {
        out.fill(0.0);
        let dv = self.cell[0] * self.cell[1] * self.cell[2] / 8.0;
        for gp in 0..8 {
            let xi = [
                sign(gp, 0) * GAUSS,
                sign(gp, 1) * GAUSS,
                sign(gp, 2) * GAUSS,
            ];
            let grads = shape_gradients(self.cell, xi);
            let mut s = [0.0; 6];
            for n in 0..8 {
                let w = (1.0 + sign(n, 0) * xi[0])
                    * (1.0 + sign(n, 1) * xi[1])
                    * (1.0 + sign(n, 2) * xi[2])
                    / 8.0;
                for r in 0..6 {
                    s[r] += w * stress[n][r];
                }
            }
            for a in 0..8 {
                let g = grads[a];
                let sg = [
                    s[0] * g[0] + s[3] * g[1] + s[5] * g[2],
                    s[3] * g[0] + s[1] * g[1] + s[4] * g[2],
                    s[5] * g[0] + s[4] * g[1] + s[2] * g[2],
                ];
                for b in 0..8 {
                    out[a * 8 + b] +=
                        dv * (sg[0] * grads[b][0] + sg[1] * grads[b][1] + sg[2] * grads[b][2]);
                }
            }
        }
    }

    /// Beban nodal ekuivalen regangan termal; `beta = E·α/(1 − 2ν)`,
    /// `dt` = ΔT di delapan node.
    pub fn thermal_load(&self, beta: f64, dt: &[f64], out: &mut [f64]) {
        out.fill(0.0);
        for gp in 0..8 {
            let xi = [
                sign(gp, 0) * GAUSS,
                sign(gp, 1) * GAUSS,
                sign(gp, 2) * GAUSS,
            ];
            let mut t = 0.0;
            for n in 0..8 {
                t += (1.0 + sign(n, 0) * xi[0])
                    * (1.0 + sign(n, 1) * xi[1])
                    * (1.0 + sign(n, 2) * xi[2])
                    / 8.0
                    * dt[n];
            }
            let row = &self.thermal[gp * NDOF..(gp + 1) * NDOF];
            for (o, r) in out.iter_mut().zip(row) {
                *o += beta * t * r;
            }
        }
    }

    /// Kekakuan elemen 24×24 (row-major) untuk bobot 1.
    pub fn stiffness(&self) -> &[f64] {
        &self.ke
    }

    /// Tegangan di delapan node dari perpindahan elemen (24 nilai).
    pub fn nodal_stress(&self, u_e: &[f64], out: &mut [[f64; 6]]) {
        for (n, slot) in out.iter_mut().enumerate().take(8) {
            for r in 0..6 {
                let row = &self.stress[(n * 6 + r) * NDOF..(n * 6 + r + 1) * NDOF];
                slot[r] = row.iter().zip(u_e).map(|(s, u)| s * u).sum();
            }
        }
    }
}

fn mat6_mul<const N: usize>(d: &[[f64; 6]; 6], m: &[[f64; N]; 6]) -> [[f64; N]; 6] {
    let mut out = [[0.0; N]; 6];
    for r in 0..6 {
        for c in 0..N {
            let mut s = 0.0;
            for k in 0..6 {
                s += d[r][k] * m[k][c];
            }
            out[r][c] = s;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linalg::jacobi_eigenvalues;

    const E: f64 = 210_000.0;
    const NU: f64 = 0.3;

    fn node_position(cell: [f64; 3], n: usize) -> [f64; 3] {
        [
            0.5 * cell[0] * (1.0 + sign(n, 0)),
            0.5 * cell[1] * (1.0 + sign(n, 1)),
            0.5 * cell[2] * (1.0 + sign(n, 2)),
        ]
    }

    #[test]
    fn element_hex8_symmetric_psd_six_rigid_modes() {
        for cell in [[1.0, 1.0, 1.0], [2.0, 0.7, 1.3]] {
            let hex = Hex8::new(cell, E, NU).unwrap();
            let k = hex.stiffness();
            let scale = k.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
            for i in 0..NDOF {
                for j in 0..NDOF {
                    assert!((k[i * NDOF + j] - k[j * NDOF + i]).abs() <= 1e-12 * scale);
                }
            }
            let eig = jacobi_eigenvalues(k, NDOF);
            let max = eig[NDOF - 1];
            let zero: Vec<_> = eig.iter().filter(|v| v.abs() < 1e-9 * max).collect();
            assert_eq!(zero.len(), 6, "nilai eigen: {eig:?}");
            // Positif-semidefinit: sisanya positif tegas.
            assert!(eig.iter().all(|&v| v > -1e-9 * max));
            assert!(eig[6] > 1e-4 * max, "mode energi-nol palsu: {eig:?}");
        }
    }

    #[test]
    fn element_hex8_rigid_motion_gives_no_force() {
        let cell = [1.5, 1.0, 0.5];
        let hex = Hex8::new(cell, E, NU).unwrap();
        let k = hex.stiffness();
        let scale = k.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
        // Tiga translasi + tiga rotasi infinitesimal.
        for mode in 0..6 {
            let mut u = [0.0; NDOF];
            for n in 0..8 {
                let p = node_position(cell, n);
                let d = match mode {
                    0 => [1.0, 0.0, 0.0],
                    1 => [0.0, 1.0, 0.0],
                    2 => [0.0, 0.0, 1.0],
                    3 => [0.0, -p[2], p[1]],
                    4 => [p[2], 0.0, -p[0]],
                    _ => [-p[1], p[0], 0.0],
                };
                u[3 * n..3 * n + 3].copy_from_slice(&d);
            }
            for i in 0..NDOF {
                let f: f64 = (0..NDOF).map(|j| k[i * NDOF + j] * u[j]).sum();
                assert!(f.abs() < 1e-10 * scale, "mode {mode}: gaya {f}");
            }
            let mut s = [[0.0; 6]; 8];
            hex.nodal_stress(&u, &mut s);
            assert!(s.iter().flatten().all(|v| v.abs() < 1e-9 * E));
        }
    }

    #[test]
    fn element_hex8_constant_strain_patch_single_element() {
        let cell = [2.0, 1.0, 0.5];
        let hex = Hex8::new(cell, E, NU).unwrap();
        // Medan linier u = A·x dengan gradien sembarang.
        let a = [
            [1.0e-3, 2.0e-4, -3.0e-4],
            [4.0e-4, -5.0e-4, 1.0e-4],
            [-2.0e-4, 6.0e-4, 7.0e-4],
        ];
        let mut u = [0.0; NDOF];
        for n in 0..8 {
            let p = node_position(cell, n);
            for c in 0..3 {
                u[3 * n + c] = a[c][0] * p[0] + a[c][1] * p[1] + a[c][2] * p[2];
            }
        }
        let strain = [
            a[0][0],
            a[1][1],
            a[2][2],
            a[0][1] + a[1][0],
            a[1][2] + a[2][1],
            a[0][2] + a[2][0],
        ];
        let d = elasticity_matrix(E, NU);
        let mut expect = [0.0; 6];
        for r in 0..6 {
            expect[r] = (0..6).map(|c| d[r][c] * strain[c]).sum();
        }
        let mut s = [[0.0; 6]; 8];
        hex.nodal_stress(&u, &mut s);
        for n in 0..8 {
            for r in 0..6 {
                assert!(
                    (s[n][r] - expect[r]).abs() < 1e-9 * E * 1e-3,
                    "node {n} komponen {r}"
                );
            }
        }
        // Gaya nodal = traksi tegangan konstan pada sisi: energi regangan
        // uᵀKu harus sama dengan V·σ:ε.
        let k = hex.stiffness();
        let mut energy = 0.0;
        for i in 0..NDOF {
            for j in 0..NDOF {
                energy += u[i] * k[i * NDOF + j] * u[j];
            }
        }
        let exact: f64 =
            (0..6).map(|r| expect[r] * strain[r]).sum::<f64>() * cell[0] * cell[1] * cell[2];
        assert!((energy - exact).abs() < 1e-10 * exact.abs());
    }

    #[test]
    fn element_hex8_pure_bending_is_exact() {
        // Mode inkompatibel membuat satu elemen mewakili lentur murni tepat:
        // energi = M²L/(2EI) untuk kelengkungan konstan.
        let cell = [4.0, 1.0, 1.0];
        let hex = Hex8::new(cell, E, 0.0).unwrap();
        let kappa = 1.0e-3;
        let mut u = [0.0; NDOF];
        for n in 0..8 {
            let p = node_position(cell, n);
            let (x, z) = (p[0] - 2.0, p[2] - 0.5);
            u[3 * n] = -kappa * x * z;
            // w = κ·x²/2 tidak diwakili medan trilinear; mode inkompatibel
            // yang menggantikannya.
            u[3 * n + 2] = 0.0;
        }
        let k = hex.stiffness();
        let mut energy = 0.0;
        for i in 0..NDOF {
            for j in 0..NDOF {
                energy += u[i] * k[i * NDOF + j] * u[j];
            }
        }
        let inertia = 1.0 / 12.0;
        let exact = E * inertia * kappa * kappa * cell[0];
        assert!((energy - exact).abs() < 1e-9 * exact, "{energy} vs {exact}");
    }

    #[test]
    fn element_hex8_rejects_bad_material() {
        assert!(Hex8::new([1.0; 3], -1.0, 0.3).is_err());
        assert!(Hex8::new([1.0; 3], 1.0, 0.5).is_err());
        assert!(Hex8::new([0.0, 1.0, 1.0], 1.0, 0.3).is_err());
        assert!(Hex8::new([1.0; 3], f64::NAN, 0.3).is_err());
    }
}
