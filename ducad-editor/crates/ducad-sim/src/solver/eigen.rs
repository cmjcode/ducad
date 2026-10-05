//! Solver eigen umum `A·x = θ·B·x` untuk nilai eigen terkecil: LOBPCG
//! (Locally Optimal Block Preconditioned Conjugate Gradient).
//!
//! Dipakai untuk frekuensi (`A = K`, `B = M`) dan buckling (`A = K_σ`,
//! `B = K`). Prekondisi `T ≈ K⁻¹` membuat tiap iterasi setara satu langkah
//! shift-invert tak-eksak, tanpa penyelesaian linier penuh di dalamnya —
//! jauh lebih murah daripada Lanczos shift-invert yang butuh satu PCG penuh
//! per vektor Lanczos.
//!
//! Kestabilan: basis `[X, W, P]` di-B-ortonormalkan dengan Gram–Schmidt
//! termodifikasi (dua lintasan) sebelum Rayleigh–Ritz, vektor yang nyaris
//! bergantung linier dibuang. Vektor awal dari LCG deterministik.

use crate::assemble::CsrSym;
use crate::linalg::jacobi_eigen;
use crate::solver::cg::Preconditioner;
use crate::{CancelToken, SimError};

/// Operator linier simetris.
pub trait LinOp {
    fn apply(&self, x: &[f64], y: &mut [f64]);
}

impl LinOp for CsrSym {
    fn apply(&self, x: &[f64], y: &mut [f64]) {
        self.mul(x, y);
    }
}

/// Matriks skalar tingkat node yang bekerja sama pada tiga komponen
/// (`dof = 3·node + sumbu`), dikalikan `scale`, dengan DOF `fixed` dimatikan.
pub struct NodeScalarOp<'a> {
    pub matrix: &'a CsrSym,
    pub scale: f64,
    pub fixed: &'a [bool],
}

impl LinOp for NodeScalarOp<'_> {
    fn apply(&self, x: &[f64], y: &mut [f64]) {
        y.fill(0.0);
        let m = self.matrix;
        let get = |dof: usize| if self.fixed[dof] { 0.0 } else { x[dof] };
        for r in 0..m.n {
            let start = m.row_ptr[r];
            let end = m.row_ptr[r + 1];
            if start == end {
                continue;
            }
            let xr = [get(3 * r), get(3 * r + 1), get(3 * r + 2)];
            let d = m.val[start];
            let mut s = [d * xr[0], d * xr[1], d * xr[2]];
            for at in start + 1..end {
                let c = m.col[at] as usize;
                let v = m.val[at];
                for a in 0..3 {
                    s[a] += v * get(3 * c + a);
                    y[3 * c + a] += v * xr[a];
                }
            }
            for a in 0..3 {
                y[3 * r + a] += s[a];
            }
        }
        for (dof, v) in y.iter_mut().enumerate() {
            *v = if self.fixed[dof] {
                0.0
            } else {
                *v * self.scale
            };
        }
    }
}

/// Opsi LOBPCG.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EigenOptions {
    /// Jumlah pasangan eigen yang diminta.
    pub count: usize,
    /// Toleransi residual relatif `‖Ax − θBx‖ / (‖Ax‖ + |θ|·‖Bx‖)`.
    pub tol: f64,
    pub max_iter: usize,
}

/// Hasil LOBPCG.
#[derive(Debug, Clone, PartialEq)]
pub struct EigenResult {
    /// Nilai eigen menaik.
    pub values: Vec<f64>,
    /// Vektor eigen B-ortonormal.
    pub vectors: Vec<Vec<f64>>,
    pub iterations: usize,
    /// Residual relatif terbesar di antara pasangan yang diminta.
    pub residual: f64,
    pub converged: bool,
}

/// Nilai eigen dianggap mapan bila berubah relatif kurang dari
/// `STALL_FACTOR × tol` selama [`STALL_SPAN`] iterasi.
const STALL_FACTOR: f64 = 1.0e-2;
const STALL_SPAN: usize = 3;
/// Selang iterasi untuk menghitung ulang `A·x`, `B·x`.
const REFRESH_EVERY: usize = 25;

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn axpy(alpha: f64, x: &[f64], y: &mut [f64]) {
    for (yi, xi) in y.iter_mut().zip(x) {
        *yi += alpha * xi;
    }
}

/// Vektor beserta citranya di bawah A dan B.
struct Triple {
    v: Vec<f64>,
    av: Vec<f64>,
    bv: Vec<f64>,
}

impl Triple {
    fn new(v: Vec<f64>, a: &dyn LinOp, b: &dyn LinOp) -> Triple {
        let mut av = vec![0.0; v.len()];
        let mut bv = vec![0.0; v.len()];
        a.apply(&v, &mut av);
        b.apply(&v, &mut bv);
        Triple { v, av, bv }
    }

    fn add_scaled(&mut self, alpha: f64, other: &Triple) {
        axpy(alpha, &other.v, &mut self.v);
        axpy(alpha, &other.av, &mut self.av);
        axpy(alpha, &other.bv, &mut self.bv);
    }

    fn scale(&mut self, alpha: f64) {
        for arr in [&mut self.v, &mut self.av, &mut self.bv] {
            for x in arr.iter_mut() {
                *x *= alpha;
            }
        }
    }
}

/// B-ortonormalkan `cand` terhadap `basis` lalu tambahkan; `false` bila
/// vektor nyaris bergantung linier.
fn push_orthonormal(basis: &mut Vec<Triple>, mut cand: Triple) -> bool {
    let start = dot(&cand.v, &cand.bv);
    if start.is_nan() || start <= 0.0 || !start.is_finite() {
        return false;
    }
    // Gram–Schmidt termodifikasi; lintasan kedua hanya bila lintasan pertama
    // membuang sebagian besar vektor ("dua kali sudah cukup").
    let mut norm2 = start;
    for _ in 0..2 {
        let before = norm2;
        for q in basis.iter() {
            let c = dot(&q.v, &cand.bv);
            if c != 0.0 {
                cand.add_scaled(-c, q);
            }
        }
        norm2 = dot(&cand.v, &cand.bv);
        if norm2 > 0.25 * before {
            break;
        }
    }
    if norm2.is_nan() || norm2 <= 1e-14 * start || !norm2.is_finite() {
        return false;
    }
    cand.scale(1.0 / norm2.sqrt());
    basis.push(cand);
    true
}

/// Kombinasi linier `Σ c_i·basis[i]`.
fn combine(basis: &[Triple], coeff: &[f64], range: std::ops::Range<usize>, n: usize) -> Triple {
    let mut out = Triple {
        v: vec![0.0; n],
        av: vec![0.0; n],
        bv: vec![0.0; n],
    };
    for i in range {
        if coeff[i] != 0.0 {
            out.add_scaled(coeff[i], &basis[i]);
        }
    }
    out
}

/// Rayleigh–Ritz pada basis B-ortonormal: `keep` pasangan terkecil sebagai
/// X baru, dan bagian X baru yang berasal dari `basis[old..]` sebagai P baru.
fn rayleigh_ritz(
    basis: &[Triple],
    keep: usize,
    old: usize,
    n: usize,
) -> (Vec<f64>, Vec<Triple>, Vec<Triple>) {
    let s = basis.len();
    let mut g = vec![0.0; s * s];
    for i in 0..s {
        for j in i..s {
            let v = dot(&basis[i].v, &basis[j].av);
            g[i * s + j] = v;
            g[j * s + i] = v;
        }
    }
    let (values, vectors) = jacobi_eigen(&g, s);
    let keep = keep.min(s);
    let new_x: Vec<Triple> = (0..keep)
        .map(|k| combine(basis, &vectors[k], 0..s, n))
        .collect();
    let new_p: Vec<Triple> = if old < s {
        (0..keep)
            .map(|k| combine(basis, &vectors[k], old..s, n))
            .collect()
    } else {
        Vec::new()
    };
    (values[..keep].to_vec(), new_x, new_p)
}

/// Mencari `opt.count` nilai eigen terkecil `A·x = θ·B·x`. `fixed` menandai
/// DOF yang dimatikan (vektor dijaga nol di sana). `B` harus positif-definit
/// pada DOF bebas.
pub fn lobpcg(
    a: &dyn LinOp,
    b: &dyn LinOp,
    pre: &dyn Preconditioner,
    fixed: &[bool],
    opt: &EigenOptions,
    cancel: &CancelToken,
) -> Result<EigenResult, SimError> {
    let n = fixed.len();
    let free = fixed.iter().filter(|&&f| !f).count();
    let count = opt.count.min(free);
    if count == 0 {
        return Err(SimError::InvalidSetup(
            "no free degrees of freedom for the eigen solve".into(),
        ));
    }
    let block = (count + (count / 4).max(3)).min(free);
    // Vektor awal deterministik.
    let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut next = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((seed >> 11) as f64) / ((1u64 << 53) as f64) - 0.5
    };
    let mut x: Vec<Triple> = Vec::with_capacity(block);
    let mut attempts = 0;
    while x.len() < block && attempts < 4 * block {
        attempts += 1;
        let mut raw: Vec<f64> = (0..n)
            .map(|i| if fixed[i] { 0.0 } else { next() })
            .collect();
        // Satu langkah prekondisi menghaluskan vektor acak.
        let mut smooth = vec![0.0; n];
        pre.apply(&raw, &mut smooth);
        for (i, v) in smooth.iter().enumerate() {
            raw[i] = if fixed[i] { 0.0 } else { *v };
        }
        push_orthonormal(&mut x, Triple::new(raw, a, b));
    }
    if x.len() < count {
        return Err(SimError::InvalidSetup(
            "could not build a starting basis for the eigen solve (mass or stiffness is singular)"
                .into(),
        ));
    }
    let (mut theta, start_x, _) = rayleigh_ritz(&x, block, x.len(), n);
    x = start_x;
    let mut p: Vec<Triple> = Vec::new();
    let mut residual = f64::INFINITY;
    let mut iterations = 0;
    let mut converged = false;
    let mut work = vec![0.0; n];
    let mut history: Vec<Vec<f64>> = Vec::new();
    let stall_tol = STALL_FACTOR * opt.tol;
    for it in 0..opt.max_iter {
        if cancel.is_cancelled() {
            return Err(SimError::Cancelled);
        }
        iterations = it;
        // Residual.
        let mut w: Vec<Vec<f64>> = Vec::new();
        residual = 0.0;
        for (k, t) in x.iter().enumerate() {
            let mut r: Vec<f64> =
                t.av.iter()
                    .zip(&t.bv)
                    .map(|(av, bv)| av - theta[k] * bv)
                    .collect();
            for (i, v) in r.iter_mut().enumerate() {
                if fixed[i] {
                    *v = 0.0;
                }
            }
            let scale = dot(&t.av, &t.av).sqrt() + theta[k].abs() * dot(&t.bv, &t.bv).sqrt();
            let rel = if scale > 0.0 {
                dot(&r, &r).sqrt() / scale
            } else {
                0.0
            };
            if !rel.is_finite() {
                return Err(SimError::Diverged {
                    iterations: it,
                    residual: f64::NAN,
                });
            }
            if k < count {
                residual = residual.max(rel);
            }
            // Kunci lunak: vektor yang nilai eigennya sudah mapan tidak lagi
            // menyumbang arah pencarian baru.
            let locked = history.len() >= STALL_SPAN && {
                let past = &history[history.len() - STALL_SPAN];
                past.get(k).is_some_and(|old| {
                    (old - theta[k]).abs() <= stall_tol * theta[k].abs().max(f64::MIN_POSITIVE)
                })
            };
            if rel > 0.1 * opt.tol && !locked {
                pre.apply(&r, &mut work);
                for (i, v) in work.iter_mut().enumerate() {
                    if fixed[i] {
                        *v = 0.0;
                    }
                }
                w.push(work.clone());
            }
        }
        // Konvergen bila residual kecil, atau nilai eigen yang diminta sudah
        // tidak berubah (galat nilai eigen kuadratik terhadap galat vektor,
        // jadi nilai eigen mapan jauh sebelum residual Euclid mencapai tol).
        history.push(theta.clone());
        let settled = history.len() > STALL_SPAN && {
            let past = &history[history.len() - 1 - STALL_SPAN];
            past.iter()
                .zip(&theta)
                .take(count)
                .all(|(old, new)| (old - new).abs() <= stall_tol * new.abs().max(f64::MIN_POSITIVE))
        };
        if residual < opt.tol || settled {
            converged = true;
            break;
        }
        // Secara berkala hitung ulang A·x dan B·x dari x agar galat
        // pembulatan kombinasi linier tidak menumpuk, dan buang P.
        if it > 0 && it % REFRESH_EVERY == 0 {
            let mut fresh: Vec<Triple> = Vec::with_capacity(x.len());
            for t in std::mem::take(&mut x) {
                push_orthonormal(&mut fresh, Triple::new(t.v, a, b));
            }
            x = fresh;
            p.clear();
            if x.len() < count {
                return Err(SimError::Diverged {
                    iterations: it,
                    residual,
                });
            }
        }
        // Basis [X, W, P] yang B-ortonormal.
        let old = x.len();
        let mut basis: Vec<Triple> = std::mem::take(&mut x);
        for v in w {
            push_orthonormal(&mut basis, Triple::new(v, a, b));
        }
        for t in std::mem::take(&mut p) {
            push_orthonormal(&mut basis, t);
        }
        let (values, new_x, new_p) = rayleigh_ritz(&basis, block, old, n);
        theta = values;
        x = new_x;
        p = new_p;
        iterations = it + 1;
    }
    x.truncate(count);
    theta.truncate(count);
    Ok(EigenResult {
        values: theta,
        vectors: x.into_iter().map(|t| t.v).collect(),
        iterations,
        residual,
        converged,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solver::cg::Jacobi;

    #[test]
    fn solver_eigen_lobpcg_matches_analytic_chain() {
        // Rantai pegas terjepit di kedua ujung: K = tridiag(−1, 2, −1), M = I.
        // λ_k = 2 − 2 cos(kπ/(n+1)).
        let n = 60;
        let mut entries = Vec::new();
        for i in 0..n {
            entries.push((i as u32, i as u32, 2.0));
            if i + 1 < n {
                entries.push((i as u32, (i + 1) as u32, -1.0));
            }
        }
        let k = CsrSym::from_triplets(n, &entries).unwrap();
        let identity: Vec<(u32, u32, f64)> = (0..n as u32).map(|i| (i, i, 1.0)).collect();
        let m = CsrSym::from_triplets(n, &identity).unwrap();
        let fixed = vec![false; n];
        let opt = EigenOptions {
            count: 4,
            tol: 1e-9,
            max_iter: 500,
        };
        let result = lobpcg(&k, &m, &Jacobi::new(&k), &fixed, &opt, &CancelToken::new()).unwrap();
        assert!(result.converged, "residual {}", result.residual);
        for (i, value) in result.values.iter().enumerate() {
            let exact = 2.0 - 2.0 * ((i + 1) as f64 * std::f64::consts::PI / (n + 1) as f64).cos();
            assert!(
                (value - exact).abs() < 1e-9 * exact.max(1e-3),
                "mode {i}: {value} vs {exact}"
            );
        }
        // Vektor B-ortonormal.
        for i in 0..4 {
            for j in 0..4 {
                let d = dot(&result.vectors[i], &result.vectors[j]);
                assert!((d - if i == j { 1.0 } else { 0.0 }).abs() < 1e-8);
            }
        }
        // Deterministik.
        let again = lobpcg(&k, &m, &Jacobi::new(&k), &fixed, &opt, &CancelToken::new()).unwrap();
        assert_eq!(result, again);
    }
}
