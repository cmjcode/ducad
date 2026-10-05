//! Elemen tetrahedral kuadratik 10-node (isoparametrik).
//!
//! Node 0–3 di sudut; node 4–9 di tengah tepi menurut [`EDGES`].
//! Koordinat barisentrik `λ0 = 1 − ξ − η − ζ`, `λ1 = ξ`, `λ2 = η`, `λ3 = ζ`.
//! Kekakuan memakai Gauss 4 titik (eksak untuk tet bersisi lurus); massa,
//! volume, dan beban badan memakai aturan Keast 11 titik (derajat 4, eksak
//! untuk `N_a·N_b` pada tet bersisi lurus).

use crate::linalg::V3;

/// Pasangan sudut tiap node tengah tepi (node `4 + i`).
pub const EDGES: [[usize; 2]; 6] = [[0, 1], [1, 2], [0, 2], [0, 3], [1, 3], [2, 3]];

/// Koordinat barisentrik ke-10 node.
pub const NODE_COORDS: [[f64; 4]; 10] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
    [0.5, 0.5, 0.0, 0.0],
    [0.0, 0.5, 0.5, 0.0],
    [0.5, 0.0, 0.5, 0.0],
    [0.5, 0.0, 0.0, 0.5],
    [0.0, 0.5, 0.0, 0.5],
    [0.0, 0.0, 0.5, 0.5],
];

const G4A: f64 = 0.138_196_601_125_010_5;
const G4B: f64 = 0.585_410_196_624_968_5;

/// Gauss 4 titik (derajat 2); bobot berjumlah 1/6.
pub const GAUSS4: [([f64; 4], f64); 4] = [
    ([G4B, G4A, G4A, G4A], 1.0 / 24.0),
    ([G4A, G4B, G4A, G4A], 1.0 / 24.0),
    ([G4A, G4A, G4B, G4A], 1.0 / 24.0),
    ([G4A, G4A, G4A, G4B], 1.0 / 24.0),
];

/// Aturan Keast 11 titik (derajat 4); bobot berjumlah 1/6.
pub fn keast11() -> [([f64; 4], f64); 11] {
    let w0 = -74.0 / 5625.0;
    let w1 = 343.0 / 45000.0;
    let w2 = 56.0 / 2250.0;
    let (a, b) = (1.0 / 14.0, 11.0 / 14.0);
    let root = (5.0_f64 / 14.0).sqrt();
    let (c, d) = (0.25 * (1.0 + root), 0.25 * (1.0 - root));
    [
        ([0.25, 0.25, 0.25, 0.25], w0),
        ([b, a, a, a], w1),
        ([a, b, a, a], w1),
        ([a, a, b, a], w1),
        ([a, a, a, b], w1),
        ([c, c, d, d], w2),
        ([c, d, c, d], w2),
        ([c, d, d, c], w2),
        ([d, c, c, d], w2),
        ([d, c, d, c], w2),
        ([d, d, c, c], w2),
    ]
}

/// Fungsi bentuk di titik barisentrik `l`.
pub fn shape(l: [f64; 4]) -> [f64; 10] {
    let mut n = [0.0; 10];
    for i in 0..4 {
        n[i] = l[i] * (2.0 * l[i] - 1.0);
    }
    for (k, e) in EDGES.iter().enumerate() {
        n[4 + k] = 4.0 * l[e[0]] * l[e[1]];
    }
    n
}

/// Turunan fungsi bentuk terhadap `(ξ, η, ζ)`.
pub fn shape_grad_ref(l: [f64; 4]) -> [[f64; 3]; 10] {
    // dλ_k/dξ_a
    const DL: [[f64; 3]; 4] = [
        [-1.0, -1.0, -1.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    ];
    let mut g = [[0.0; 3]; 10];
    for i in 0..4 {
        let d = 4.0 * l[i] - 1.0;
        for a in 0..3 {
            g[i][a] = d * DL[i][a];
        }
    }
    for (k, e) in EDGES.iter().enumerate() {
        for a in 0..3 {
            g[4 + k][a] = 4.0 * (l[e[1]] * DL[e[0]][a] + l[e[0]] * DL[e[1]][a]);
        }
    }
    g
}

/// Gradien fisik fungsi bentuk dan `det J` di titik `l`. `det J = 6·V` untuk
/// tet lurus berorientasi positif.
pub fn phys_grad(x: &[V3; 10], l: [f64; 4]) -> ([[f64; 3]; 10], f64) {
    let gr = shape_grad_ref(l);
    // J[a][b] = ∂x_b/∂ξ_a
    let mut j = [[0.0; 3]; 3];
    for n in 0..10 {
        for a in 0..3 {
            for b in 0..3 {
                j[a][b] += gr[n][a] * x[n][b];
            }
        }
    }
    let c00 = j[1][1] * j[2][2] - j[1][2] * j[2][1];
    let c01 = j[1][2] * j[2][0] - j[1][0] * j[2][2];
    let c02 = j[1][0] * j[2][1] - j[1][1] * j[2][0];
    let det = j[0][0] * c00 + j[0][1] * c01 + j[0][2] * c02;
    let inv_det = if det != 0.0 { 1.0 / det } else { 0.0 };
    let inv = [
        [
            c00 * inv_det,
            (j[0][2] * j[2][1] - j[0][1] * j[2][2]) * inv_det,
            (j[0][1] * j[1][2] - j[0][2] * j[1][1]) * inv_det,
        ],
        [
            c01 * inv_det,
            (j[0][0] * j[2][2] - j[0][2] * j[2][0]) * inv_det,
            (j[0][2] * j[1][0] - j[0][0] * j[1][2]) * inv_det,
        ],
        [
            c02 * inv_det,
            (j[0][1] * j[2][0] - j[0][0] * j[2][1]) * inv_det,
            (j[0][0] * j[1][1] - j[0][1] * j[1][0]) * inv_det,
        ],
    ];
    // ∂N/∂x_b = Σ_a (J⁻¹)[b][a] ∂N/∂ξ_a
    let mut g = [[0.0; 3]; 10];
    for n in 0..10 {
        for b in 0..3 {
            g[n][b] = inv[b][0] * gr[n][0] + inv[b][1] * gr[n][1] + inv[b][2] * gr[n][2];
        }
    }
    (g, det)
}

/// `det J` minimum di titik Gauss dan node (untuk memeriksa elemen lengkung).
pub fn min_det(x: &[V3; 10]) -> f64 {
    let mut min = f64::INFINITY;
    for (l, _) in GAUSS4 {
        min = min.min(phys_grad(x, l).1);
    }
    for l in NODE_COORDS {
        min = min.min(phys_grad(x, l).1);
    }
    min
}

/// Volume elemen (Keast 11 titik).
pub fn volume(x: &[V3; 10]) -> f64 {
    keast11().iter().map(|&(l, w)| w * phys_grad(x, l).1).sum()
}

/// Kekakuan 30×30 (row-major) untuk tetapan Lamé `lambda`, `mu`.
pub fn stiffness(x: &[V3; 10], lambda: f64, mu: f64, out: &mut [f64]) {
    out.fill(0.0);
    for (l, w) in GAUSS4 {
        let (g, det) = phys_grad(x, l);
        let dv = w * det;
        for a in 0..10 {
            for b in 0..10 {
                let gg = g[a][0] * g[b][0] + g[a][1] * g[b][1] + g[a][2] * g[b][2];
                for i in 0..3 {
                    let row = (3 * a + i) * 30 + 3 * b;
                    for j in 0..3 {
                        let mut v = lambda * g[a][i] * g[b][j] + mu * g[a][j] * g[b][i];
                        if i == j {
                            v += mu * gg;
                        }
                        out[row + j] += dv * v;
                    }
                }
            }
        }
    }
}

/// Tegangan (Voigt) di 10 node dari perpindahan elemen `u_e` (30 nilai).
pub fn nodal_stress(x: &[V3; 10], lambda: f64, mu: f64, u_e: &[f64], out: &mut [[f64; 6]]) {
    for (n, slot) in out.iter_mut().enumerate().take(10) {
        let (g, _) = phys_grad(x, NODE_COORDS[n]);
        // Gradien perpindahan h[i][j] = ∂u_i/∂x_j.
        let mut h = [[0.0; 3]; 3];
        for a in 0..10 {
            for i in 0..3 {
                let u = u_e[3 * a + i];
                for j in 0..3 {
                    h[i][j] += u * g[a][j];
                }
            }
        }
        let trace = h[0][0] + h[1][1] + h[2][2];
        *slot = [
            lambda * trace + 2.0 * mu * h[0][0],
            lambda * trace + 2.0 * mu * h[1][1],
            lambda * trace + 2.0 * mu * h[2][2],
            mu * (h[0][1] + h[1][0]),
            mu * (h[1][2] + h[2][1]),
            mu * (h[0][2] + h[2][0]),
        ];
    }
}

/// Matriks massa konsisten skalar 10×10 untuk densitas 1 (`∫ N_a N_b dV`).
pub fn mass(x: &[V3; 10], out: &mut [f64]) {
    out.fill(0.0);
    for (l, w) in keast11() {
        let n = shape(l);
        let dv = w * phys_grad(x, l).1;
        for a in 0..10 {
            for b in 0..10 {
                out[a * 10 + b] += dv * n[a] * n[b];
            }
        }
    }
}

/// Volume nodal konsisten `∫ N_a dV` (untuk beban badan).
pub fn nodal_volume(x: &[V3; 10], out: &mut [f64]) {
    out.fill(0.0);
    for (l, w) in keast11() {
        let n = shape(l);
        let dv = w * phys_grad(x, l).1;
        for a in 0..10 {
            out[a] += dv * n[a];
        }
    }
}

/// Matriks konduksi skalar 10×10 untuk konduktivitas 1 (`∫ ∇N_a·∇N_b dV`).
pub fn conductivity(x: &[V3; 10], out: &mut [f64]) {
    out.fill(0.0);
    for (l, w) in GAUSS4 {
        let (g, det) = phys_grad(x, l);
        let dv = w * det;
        for a in 0..10 {
            for b in 0..10 {
                out[a * 10 + b] += dv * (g[a][0] * g[b][0] + g[a][1] * g[b][1] + g[a][2] * g[b][2]);
            }
        }
    }
}

/// Kekakuan geometri skalar 10×10 (`∫ ∇N_aᵀ σ ∇N_b dV`) dari tegangan nodal.
pub fn geometric(x: &[V3; 10], stress: &[[f64; 6]], out: &mut [f64]) {
    out.fill(0.0);
    for (l, w) in keast11() {
        let (g, det) = phys_grad(x, l);
        let n = shape(l);
        let mut s = [0.0; 6];
        for a in 0..10 {
            for r in 0..6 {
                s[r] += n[a] * stress[a][r];
            }
        }
        let dv = w * det;
        for a in 0..10 {
            let sg = [
                s[0] * g[a][0] + s[3] * g[a][1] + s[5] * g[a][2],
                s[3] * g[a][0] + s[1] * g[a][1] + s[4] * g[a][2],
                s[5] * g[a][0] + s[4] * g[a][1] + s[2] * g[a][2],
            ];
            for b in 0..10 {
                out[a * 10 + b] += dv * (sg[0] * g[b][0] + sg[1] * g[b][1] + sg[2] * g[b][2]);
            }
        }
    }
}

/// Beban nodal ekuivalen regangan termal isotropik: `∫ Bᵀ D ε_th dV` dengan
/// `ε_th = α·ΔT·I`; `beta = E·α/(1 − 2ν)`, `dt` = ΔT di 10 node.
pub fn thermal_load(x: &[V3; 10], beta: f64, dt: &[f64], out: &mut [f64]) {
    out.fill(0.0);
    for (l, w) in GAUSS4 {
        let (g, det) = phys_grad(x, l);
        let n = shape(l);
        let t: f64 = (0..10).map(|a| n[a] * dt[a]).sum();
        let s = w * det * beta * t;
        for a in 0..10 {
            for i in 0..3 {
                out[3 * a + i] += s * g[a][i];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linalg::jacobi_eigenvalues;

    fn straight(corners: [V3; 4]) -> [V3; 10] {
        let mut x = [[0.0; 3]; 10];
        x[..4].copy_from_slice(&corners);
        for (k, e) in EDGES.iter().enumerate() {
            for a in 0..3 {
                x[4 + k][a] = 0.5 * (corners[e[0]][a] + corners[e[1]][a]);
            }
        }
        x
    }

    fn sample() -> [V3; 10] {
        straight([
            [0.1, 0.0, 0.2],
            [2.0, 0.3, 0.0],
            [0.4, 1.7, 0.1],
            [0.3, 0.5, 1.4],
        ])
    }

    fn factorial(n: u32) -> f64 {
        (1..=n).map(f64::from).product()
    }

    #[test]
    fn element_tet10_quadrature_rules_are_exact() {
        // ∫ λ0^a λ1^b λ2^c λ3^d dV = a! b! c! d! / (a+b+c+d+3)! pada tet acuan.
        for total in 0..=4u32 {
            for a in 0..=total {
                for b in 0..=(total - a) {
                    for c in 0..=(total - a - b) {
                        let d = total - a - b - c;
                        let exact = factorial(a) * factorial(b) * factorial(c) * factorial(d)
                            / factorial(total + 3);
                        let f = |l: [f64; 4]| {
                            l[0].powi(a as i32)
                                * l[1].powi(b as i32)
                                * l[2].powi(c as i32)
                                * l[3].powi(d as i32)
                        };
                        let k11: f64 = keast11().iter().map(|&(l, w)| w * f(l)).sum();
                        assert!(
                            (k11 - exact).abs() < 1e-14,
                            "Keast derajat {total}: {k11} vs {exact}"
                        );
                        if total <= 2 {
                            let g4: f64 = GAUSS4.iter().map(|&(l, w)| w * f(l)).sum();
                            assert!((g4 - exact).abs() < 1e-15, "Gauss derajat {total}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn element_tet10_shape_functions_are_consistent() {
        let l = [0.1, 0.2, 0.3, 0.4];
        assert!((shape(l).iter().sum::<f64>() - 1.0).abs() < 1e-14);
        for (n, c) in NODE_COORDS.iter().enumerate() {
            let s = shape(*c);
            for m in 0..10 {
                assert!((s[m] - if m == n { 1.0 } else { 0.0 }).abs() < 1e-14);
            }
        }
        let x = sample();
        let (g, det) = phys_grad(&x, l);
        assert!(det > 0.0);
        // Σ ∇N = 0 dan Σ ∇N x = I.
        for b in 0..3 {
            assert!(g.iter().map(|v| v[b]).sum::<f64>().abs() < 1e-12);
            for a in 0..3 {
                let v: f64 = (0..10).map(|n| g[n][b] * x[n][a]).sum();
                assert!((v - if a == b { 1.0 } else { 0.0 }).abs() < 1e-12);
            }
        }
        assert!((volume(&x) - det / 6.0).abs() < 1e-13);
    }

    #[test]
    fn element_tet10_stiffness_symmetric_psd_six_rigid_modes() {
        let x = sample();
        let (lambda, mu) = (121_153.8, 80_769.2);
        let mut k = vec![0.0; 900];
        stiffness(&x, lambda, mu, &mut k);
        let scale = k.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
        for i in 0..30 {
            for j in 0..30 {
                assert!((k[i * 30 + j] - k[j * 30 + i]).abs() < 1e-10 * scale);
            }
        }
        let eig = jacobi_eigenvalues(&k, 30);
        let max = eig[29];
        assert_eq!(
            eig.iter().filter(|v| v.abs() < 1e-9 * max).count(),
            6,
            "{eig:?}"
        );
        assert!(eig.iter().all(|&v| v > -1e-9 * max));
    }

    #[test]
    fn element_tet10_patch_linear_and_quadratic_fields() {
        let x = sample();
        let (young, nu) = (210_000.0, 0.3);
        let lambda = young * nu / ((1.0 + nu) * (1.0 - 2.0 * nu));
        let mu = young / (2.0 * (1.0 + nu));
        // Medan kuadratik u_x = a x² + b y z, u_y = c x, u_z = d z²: regangan linier, eksak di node.
        let field = |p: V3| {
            [
                1e-4 * p[0] * p[0] + 2e-4 * p[1] * p[2],
                3e-4 * p[0],
                -1e-4 * p[2] * p[2],
            ]
        };
        let mut u = [0.0; 30];
        for n in 0..10 {
            u[3 * n..3 * n + 3].copy_from_slice(&field(x[n]));
        }
        let mut s = [[0.0; 6]; 10];
        nodal_stress(&x, lambda, mu, &u, &mut s);
        for n in 0..10 {
            let p = x[n];
            let (exx, eyy, ezz) = (2e-4 * p[0], 0.0, -2e-4 * p[2]);
            let (gxy, gyz, gzx) = (2e-4 * p[2] + 3e-4, 0.0, 2e-4 * p[1]);
            let tr = exx + eyy + ezz;
            let expect = [
                lambda * tr + 2.0 * mu * exx,
                lambda * tr + 2.0 * mu * eyy,
                lambda * tr + 2.0 * mu * ezz,
                mu * gxy,
                mu * gyz,
                mu * gzx,
            ];
            for r in 0..6 {
                assert!(
                    (s[n][r] - expect[r]).abs() < 1e-8,
                    "node {n} komponen {r}: {} vs {}",
                    s[n][r],
                    expect[r]
                );
            }
        }
    }

    #[test]
    fn element_tet10_mass_and_conductivity_properties() {
        let x = sample();
        let v = volume(&x);
        let mut m = vec![0.0; 100];
        mass(&x, &mut m);
        assert!((m.iter().sum::<f64>() - v).abs() < 1e-13);
        // Bentuk tertutup tet lurus: M_sudut,sudut = 6V/420·... → cek lewat nilai eigen positif.
        let eig = jacobi_eigenvalues(&m, 10);
        assert!(
            eig[0] > 1e-6 * eig[9],
            "massa konsisten harus positif-definit: {eig:?}"
        );
        assert!(
            (m[0] - v / 70.0).abs() < 1e-13,
            "M_00 = V/70, dapat {}",
            m[0]
        );
        assert!(
            (m[4 * 10 + 4] - 8.0 * v / 105.0).abs() < 1e-13,
            "M_44 = 8V/105"
        );
        let mut nv = [0.0; 10];
        nodal_volume(&x, &mut nv);
        assert!((nv[0] + v / 20.0).abs() < 1e-13 && (nv[5] - v / 5.0).abs() < 1e-13);
        let mut c = vec![0.0; 100];
        conductivity(&x, &mut c);
        // Suhu seragam tidak menghasilkan fluks; suhu linier T = x memberi energi V.
        for a in 0..10 {
            assert!((0..10).map(|b| c[a * 10 + b]).sum::<f64>().abs() < 1e-12);
        }
        let energy: f64 = (0..10)
            .flat_map(|a| (0..10).map(move |b| (a, b)))
            .map(|(a, b)| x[a][0] * c[a * 10 + b] * x[b][0])
            .sum();
        assert!((energy - v).abs() < 1e-12);
        // Kekakuan geometri dengan σ_xx = 1 seragam sama dengan energi (∂/∂x)².
        let stress = [[1.0, 0.0, 0.0, 0.0, 0.0, 0.0]; 10];
        let mut g = vec![0.0; 100];
        geometric(&x, &stress, &mut g);
        let e2: f64 = (0..10)
            .flat_map(|a| (0..10).map(move |b| (a, b)))
            .map(|(a, b)| x[a][0] * g[a * 10 + b] * x[b][0])
            .sum();
        assert!((e2 - v).abs() < 1e-12);
    }
}
