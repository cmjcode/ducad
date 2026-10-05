//! Conjugate gradient berprekondisi (PCG) untuk [`CsrSym`].
//!
//! - Berhenti saat `‖r‖/‖b‖ < rel_tol` (bawaan 1e-8).
//! - Iterasi maksimum bawaan `max(20·√n, 2000)`. Angka rencana (20·√n) saja
//!   terlalu kecil untuk model kecil yang berkondisi buruk (pelat tipis satu
//!   lapis sel), jadi diberi lantai 2000.
//! - Prekondisi lewat trait [`Preconditioner`]: [`Jacobi`] (diagonal),
//!   [`BlockJacobi3`], dan `TwoLevel` (`solver::coarse`) yang dipakai studi
//!   statik karena Jacobi saja butuh ribuan iterasi pada struktur tipis.
//! - Token batal dicek setiap `check_every` (50) iterasi.
//! - Sistem singular (kurang tumpuan) dikenali dari: arah pencarian dengan
//!   energi ≈ 0 (`pᵀAp ≤ 1e-13·pᵀ diag(A) p`), nilai tidak hingga, atau
//!   residual yang tidak pernah turun di bawah 1e-3 sampai iterasi habis /
//!   mandek → [`SimError::Underconstrained`]. Residual yang sudah turun tetapi
//!   tidak mencapai toleransi → [`SimError::Diverged`].

use crate::assemble::CsrSym;
use crate::{CancelToken, SimError};

/// Prekondisi: `z = M⁻¹·r`.
pub trait Preconditioner {
    fn apply(&self, r: &[f64], z: &mut [f64]);
}

/// Prekondisi Jacobi (diagonal).
#[derive(Debug, Clone, PartialEq)]
pub struct Jacobi {
    inv_diag: Vec<f64>,
}

impl Jacobi {
    pub fn new(a: &CsrSym) -> Jacobi {
        let inv_diag = a
            .diagonal()
            .iter()
            .map(|&d| {
                if d > 0.0 && d.is_finite() {
                    1.0 / d
                } else {
                    1.0
                }
            })
            .collect();
        Jacobi { inv_diag }
    }
}

impl Preconditioner for Jacobi {
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        for ((z, r), d) in z.iter_mut().zip(r).zip(&self.inv_diag) {
            *z = r * d;
        }
    }
}

/// Jacobi blok 3×3 per node. Sama dengan Jacobi biasa bila blok diagonal
/// node memang diagonal; bedanya blok penalti `k·n·nᵀ` (roller pada face
/// miring) dibalik tepat sehingga penalti 1e6 tidak merusak konvergensi.
///
/// Mengandalkan tata letak [`crate::assemble::assemble_stiffness`]: baris
/// `3a+i` diawali kolom `3a+i..3a+2`.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockJacobi3 {
    inv: Vec<[[f64; 3]; 3]>,
}

impl BlockJacobi3 {
    pub fn new(a: &CsrSym) -> BlockJacobi3 {
        let nodes = a.n / 3;
        let mut inv = Vec::with_capacity(nodes);
        for node in 0..nodes {
            let mut m = [[0.0; 3]; 3];
            let mut layout_ok = true;
            for i in 0..3 {
                let start = a.row_ptr[3 * node + i];
                for j in i..3 {
                    let at = start + (j - i);
                    if at < a.row_ptr[3 * node + i + 1] && a.col[at] as usize == 3 * node + j {
                        m[i][j] = a.val[at];
                        m[j][i] = a.val[at];
                    } else {
                        layout_ok = false;
                    }
                }
            }
            let block = if layout_ok {
                crate::linalg::invert3(&m)
            } else {
                None
            };
            inv.push(block.unwrap_or_else(|| {
                let mut d = [[0.0; 3]; 3];
                for i in 0..3 {
                    d[i][i] = if m[i][i] > 0.0 && m[i][i].is_finite() {
                        1.0 / m[i][i]
                    } else {
                        1.0
                    };
                }
                d
            }));
        }
        BlockJacobi3 { inv }
    }
}

impl Preconditioner for BlockJacobi3 {
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        for ((z, r), m) in z.chunks_exact_mut(3).zip(r.chunks_exact(3)).zip(&self.inv) {
            for i in 0..3 {
                z[i] = m[i][0] * r[0] + m[i][1] * r[1] + m[i][2] * r[2];
            }
        }
    }
}

/// Opsi PCG.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CgOptions {
    pub rel_tol: f64,
    pub max_iter: usize,
    pub check_every: usize,
}

impl CgOptions {
    /// Opsi bawaan untuk sistem berukuran `n`.
    pub fn for_size(n: usize) -> CgOptions {
        CgOptions {
            rel_tol: 1.0e-8,
            max_iter: ((20.0 * (n as f64).sqrt()).ceil() as usize).max(2000),
            check_every: 50,
        }
    }
}

/// Statistik penyelesaian.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CgStats {
    pub iterations: usize,
    /// Residual relatif akhir `‖r‖/‖b‖`.
    pub residual: f64,
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn underconstrained(n: usize, detail: &str) -> SimError {
    SimError::Underconstrained {
        free_dofs: n,
        detail: detail.into(),
    }
}

/// Menyelesaikan `A·x = b` dengan PCG, mulai dari `x = 0`.
pub fn solve_pcg(
    a: &CsrSym,
    b: &[f64],
    pre: &dyn Preconditioner,
    opt: &CgOptions,
    cancel: &CancelToken,
) -> Result<(Vec<f64>, CgStats), SimError> {
    let n = a.n;
    if b.len() != n {
        return Err(SimError::InvalidSetup(
            "right-hand side length mismatch".into(),
        ));
    }
    let bnorm = dot(b, b).sqrt();
    if !bnorm.is_finite() {
        return Err(SimError::InvalidSetup("load vector is not finite".into()));
    }
    let mut x = vec![0.0; n];
    if bnorm == 0.0 {
        return Ok((
            x,
            CgStats {
                iterations: 0,
                residual: 0.0,
            },
        ));
    }
    if cancel.is_cancelled() {
        return Err(SimError::Cancelled);
    }
    let diag = a.diagonal();
    let mut r = b.to_vec();
    let mut z = vec![0.0; n];
    pre.apply(&r, &mut z);
    let mut p = z.clone();
    let mut ap = vec![0.0; n];
    let mut rz = dot(&r, &z);
    if !(rz.is_finite() && rz > 0.0) {
        return Err(underconstrained(
            n,
            "the stiffness matrix is not positive definite",
        ));
    }
    let stall_window = (opt.max_iter / 4).max(1000);
    let check_every = opt.check_every.max(1);
    let mut best = 1.0_f64;
    let mut best_iter = 0usize;
    let mut iterations = 0usize;
    for it in 1..=opt.max_iter {
        if it % check_every == 0 && cancel.is_cancelled() {
            return Err(SimError::Cancelled);
        }
        iterations = it;
        a.mul(&p, &mut ap);
        let pap = dot(&p, &ap);
        let pdp: f64 = p.iter().zip(&diag).map(|(p, d)| p * p * d).sum();
        if !pap.is_finite() || pap <= 1.0e-13 * pdp {
            return Err(underconstrained(
                n,
                "the stiffness matrix is singular (a rigid-body motion is not blocked)",
            ));
        }
        let alpha = rz / pap;
        for i in 0..n {
            x[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
        }
        let rel = dot(&r, &r).sqrt() / bnorm;
        if !rel.is_finite() {
            return Err(underconstrained(
                n,
                "the solution diverged to non-finite values",
            ));
        }
        if rel < opt.rel_tol {
            return Ok((
                x,
                CgStats {
                    iterations: it,
                    residual: rel,
                },
            ));
        }
        if rel < 0.999 * best {
            best = rel;
            best_iter = it;
        } else if it - best_iter > stall_window {
            break;
        }
        pre.apply(&r, &mut z);
        let rz_new = dot(&r, &z);
        if !(rz_new.is_finite() && rz_new > 0.0) {
            return Err(underconstrained(
                n,
                "the stiffness matrix is not positive definite",
            ));
        }
        let beta = rz_new / rz;
        rz = rz_new;
        for i in 0..n {
            p[i] = z[i] + beta * p[i];
        }
    }
    if best > 1.0e-3 {
        Err(underconstrained(
            n,
            "the solver made no progress (a rigid-body motion is not blocked, or the model is \
             nearly disconnected)",
        ))
    } else {
        Err(SimError::Diverged {
            iterations,
            residual: best,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LCG deterministik untuk data uji.
    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
        }
    }

    fn random_spd(n: usize, seed: u64) -> (CsrSym, Vec<f64>) {
        let mut rng = Lcg(seed);
        let mut entries = Vec::new();
        let mut row_sum = vec![0.0; n];
        for i in 0..n {
            for _ in 0..6 {
                let j = (rng.next() * n as f64) as usize % n;
                if j == i {
                    continue;
                }
                let v = rng.next() * 2.0 - 1.0;
                entries.push((i as u32, j as u32, v));
                row_sum[i] += v.abs();
                row_sum[j] += v.abs();
            }
        }
        for i in 0..n {
            // Dominan diagonal dengan margin kecil dan skala beragam.
            let margin = 0.01 + rng.next() * 0.5;
            entries.push((i as u32, i as u32, row_sum[i] + margin));
        }
        let b = (0..n).map(|_| rng.next() * 2.0 - 1.0).collect();
        (CsrSym::from_triplets(n, &entries).unwrap(), b)
    }

    fn true_residual(a: &CsrSym, x: &[f64], b: &[f64]) -> f64 {
        let mut ax = vec![0.0; a.n];
        a.mul(x, &mut ax);
        let num: f64 = ax.iter().zip(b).map(|(p, q)| (p - q) * (p - q)).sum();
        num.sqrt() / dot(b, b).sqrt()
    }

    #[test]
    fn solver_cg_random_spd_converges() {
        let n = 1000;
        let (a, b) = random_spd(n, 42);
        let opt = CgOptions::for_size(n);
        let (x, stats) = solve_pcg(&a, &b, &Jacobi::new(&a), &opt, &CancelToken::new()).unwrap();
        assert!(stats.residual < 1e-8);
        assert!(stats.iterations <= opt.max_iter);
        let res = true_residual(&a, &x, &b);
        assert!(res < 1e-8, "residual sejati {res}");
    }

    #[test]
    fn solver_cg_block_jacobi_matches_layout_fallback() {
        // Tata letak bukan blok node: BlockJacobi3 jatuh ke diagonal dan tetap konvergen.
        let n = 300;
        let (a, b) = random_spd(n, 7);
        let opt = CgOptions::for_size(n);
        let (x, _) = solve_pcg(&a, &b, &BlockJacobi3::new(&a), &opt, &CancelToken::new()).unwrap();
        assert!(true_residual(&a, &x, &b) < 1e-8);
    }

    /// Rantai pegas bebas-bebas: singular (translasi benda tegar).
    fn free_chain(n: usize) -> CsrSym {
        let mut entries = Vec::new();
        for i in 0..n - 1 {
            entries.push((i as u32, i as u32, 1.0));
            entries.push(((i + 1) as u32, (i + 1) as u32, 1.0));
            entries.push((i as u32, (i + 1) as u32, -1.0));
        }
        CsrSym::from_triplets(n, &entries).unwrap()
    }

    #[test]
    fn solver_cg_singular_system_is_underconstrained() {
        let n = 1000;
        let a = free_chain(n);
        let opt = CgOptions::for_size(n);
        for b in [
            vec![1.0; n],
            (0..n).map(|i| (i % 7) as f64 - 2.0).collect::<Vec<_>>(),
        ] {
            let err = solve_pcg(&a, &b, &Jacobi::new(&a), &opt, &CancelToken::new()).unwrap_err();
            assert_eq!(err.code(), "SIM_UNDERCONSTRAINED", "{err}");
            assert!(err.hint().is_some());
        }
    }

    #[test]
    fn solver_cg_zero_rhs_and_cancel() {
        let n = 1000;
        let (a, b) = random_spd(n, 3);
        let opt = CgOptions::for_size(n);
        let pre = Jacobi::new(&a);
        let (x, stats) = solve_pcg(&a, &vec![0.0; n], &pre, &opt, &CancelToken::new()).unwrap();
        assert!(x.iter().all(|&v| v == 0.0) && stats.iterations == 0);
        let cancel = CancelToken::new();
        cancel.cancel();
        assert_eq!(
            solve_pcg(&a, &b, &pre, &opt, &cancel).unwrap_err(),
            SimError::Cancelled
        );
        // Toleransi mustahil → iterasi habis → Diverged (residual sudah kecil).
        let tight = CgOptions {
            rel_tol: 0.0,
            max_iter: 30,
            check_every: 50,
        };
        let err = solve_pcg(&a, &b, &pre, &tight, &CancelToken::new()).unwrap_err();
        assert!(matches!(
            err,
            SimError::Diverged { .. } | SimError::Underconstrained { .. }
        ));
    }
}
