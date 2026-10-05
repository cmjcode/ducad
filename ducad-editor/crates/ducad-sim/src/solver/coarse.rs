//! Prekondisi dua-tingkat: Jacobi blok + koreksi ruang kasar.
//!
//! Jacobi saja butuh ribuan iterasi pada struktur tipis (kondisi K tumbuh
//! seperti `(L/h)²·(L/t)²`). Koreksi kasar menghapus mode berfrekuensi rendah
//! itu: node dikelompokkan menjadi agregat (kotak `B×B×B` sel, dipecah per
//! komponen terhubung), tiap agregat menyumbang enam mode benda tegar
//! (3 translasi + 3 rotasi) sebagai basis kasar `P`. Prekondisi aditif:
//!
//! `M⁻¹ = J⁻¹ + P·(PᵀKP)⁻¹·Pᵀ`
//!
//! yang simetris positif-definit sehingga sah untuk CG. Matriks kasar
//! (≤ `6·MAX_AGGREGATES`) difaktorkan Cholesky padat sekali; mode kasar yang
//! tidak punya kekakuan (agregat terkunci penuh atau node segaris) dibuang.
//! Semuanya deterministik.

use crate::assemble::CsrSym;
use crate::solver::cg::{BlockJacobi3, Preconditioner};

/// Batas jumlah agregat (ukuran matriks kasar = 6 × ini).
const MAX_AGGREGATES: usize = 120;
/// Pivot Cholesky di bawah fraksi ini dari diagonal aslinya dianggap nol.
const DEAD_PIVOT: f64 = 1.0e-10;

/// Baris `i` matriks mode tegar node pada posisi relatif `x`:
/// `u = t + ω × x`, sebagai tiga pasangan (kolom, nilai).
fn rigid_row(i: usize, x: [f64; 3]) -> [(usize, f64); 3] {
    match i {
        0 => [(0, 1.0), (4, x[2]), (5, -x[1])],
        1 => [(1, 1.0), (5, x[0]), (3, -x[2])],
        _ => [(2, 1.0), (3, x[1]), (4, -x[0])],
    }
}

/// Prekondisi dua-tingkat untuk K bertata letak blok node 3×3.
#[derive(Debug, Clone, PartialEq)]
pub struct TwoLevel {
    smoother: BlockJacobi3,
    /// Agregat tiap node.
    aggregate: Vec<u32>,
    /// Posisi node relatif terhadap titik berat agregatnya.
    rel: Vec<[f64; 3]>,
    fixed: Vec<bool>,
    /// Ukuran matriks kasar.
    m: usize,
    /// Faktor Cholesky padat `L` (segitiga bawah, row-major m×m).
    chol: Vec<f64>,
    dead: Vec<bool>,
}

fn find(parent: &mut [u32], mut x: u32) -> u32 {
    while parent[x as usize] != x {
        parent[x as usize] = parent[parent[x as usize] as usize];
        x = parent[x as usize];
    }
    x
}

/// Mengelompokkan node: kotak berukuran `size`, dipecah per komponen
/// terhubung menurut pola K. Mengembalikan (agregat per node, jumlah agregat).
fn aggregate_nodes(
    k: &CsrSym,
    positions: &[[f64; 3]],
    min: [f64; 3],
    size: f64,
) -> (Vec<u32>, usize) {
    let nn = positions.len();
    let bin = |p: [f64; 3]| -> [i64; 3] {
        [
            ((p[0] - min[0]) / size).floor() as i64,
            ((p[1] - min[1]) / size).floor() as i64,
            ((p[2] - min[2]) / size).floor() as i64,
        ]
    };
    let bins: Vec<[i64; 3]> = positions.iter().map(|&p| bin(p)).collect();
    let mut parent: Vec<u32> = (0..nn as u32).collect();
    for a in 0..nn {
        // Baris DOF pertama node memuat semua tetangga node (kolom kelipatan 3).
        let row = 3 * a;
        for &c in &k.col[k.row_ptr[row]..k.row_ptr[row + 1]] {
            let b = (c / 3) as usize;
            if b != a && b < nn && bins[a] == bins[b] {
                let ra = find(&mut parent, a as u32);
                let rb = find(&mut parent, b as u32);
                if ra != rb {
                    // Akar terkecil menang agar penomoran deterministik.
                    let (lo, hi) = if ra < rb { (ra, rb) } else { (rb, ra) };
                    parent[hi as usize] = lo;
                }
            }
        }
    }
    let mut id = vec![u32::MAX; nn];
    let mut aggregate = vec![0u32; nn];
    let mut count = 0usize;
    for a in 0..nn {
        let r = find(&mut parent, a as u32) as usize;
        if id[r] == u32::MAX {
            id[r] = count as u32;
            count += 1;
        }
        aggregate[a] = id[r];
    }
    (aggregate, count)
}

impl TwoLevel {
    /// Membangun prekondisi. `spacing` = ukuran sel khas mesh (mm).
    /// Mengembalikan `None` bila ruang kasar tidak bisa dibentuk (model
    /// terlalu kecil); pemanggil lalu memakai Jacobi blok saja.
    pub fn build(
        k: &CsrSym,
        fixed: &[bool],
        positions: &[[f64; 3]],
        spacing: f64,
    ) -> Option<TwoLevel> {
        let nn = positions.len();
        if nn < 64
            || k.n != 3 * nn
            || fixed.len() != 3 * nn
            || !(spacing.is_finite() && spacing > 0.0)
        {
            return None;
        }
        let mut min = [f64::INFINITY; 3];
        for p in positions {
            for a in 0..3 {
                min[a] = min[a].min(p[a]);
            }
        }
        // Geser setengah sel agar node di bidang grid tidak jatuh tepat di batas kotak.
        for a in 0..3 {
            min[a] -= 0.5 * spacing;
        }
        let mut blocks = ((nn as f64 / MAX_AGGREGATES as f64).cbrt().ceil()).max(2.0);
        let (mut aggregate, mut count) = aggregate_nodes(k, positions, min, blocks * spacing);
        for _ in 0..8 {
            if count <= MAX_AGGREGATES {
                break;
            }
            blocks = (blocks * (count as f64 / MAX_AGGREGATES as f64).sqrt())
                .ceil()
                .max(blocks + 1.0);
            (aggregate, count) = aggregate_nodes(k, positions, min, blocks * spacing);
        }
        if count > 4 * MAX_AGGREGATES || count == 0 {
            return None;
        }
        // Titik berat agregat → posisi relatif.
        let mut centroid = vec![[0.0; 3]; count];
        let mut members = vec![0usize; count];
        for (a, p) in positions.iter().enumerate() {
            let g = aggregate[a] as usize;
            members[g] += 1;
            for c in 0..3 {
                centroid[g][c] += p[c];
            }
        }
        let rel: Vec<[f64; 3]> = positions
            .iter()
            .enumerate()
            .map(|(a, p)| {
                let g = aggregate[a] as usize;
                let inv = 1.0 / members[g] as f64;
                [
                    p[0] - centroid[g][0] * inv,
                    p[1] - centroid[g][1] * inv,
                    p[2] - centroid[g][2] * inv,
                ]
            })
            .collect();
        // Matriks kasar A = PᵀKP (padat, simetris penuh).
        let m = 6 * count;
        let mut coarse = vec![0.0; m * m];
        for r in 0..k.n {
            if fixed[r] {
                continue;
            }
            let a = r / 3;
            let pa = rigid_row(r % 3, rel[a]);
            let base_a = 6 * aggregate[a] as usize;
            for at in k.row_ptr[r]..k.row_ptr[r + 1] {
                let c = k.col[at] as usize;
                let v = k.val[at];
                if v == 0.0 || fixed[c] {
                    continue;
                }
                let b = c / 3;
                let pb = rigid_row(c % 3, rel[b]);
                let base_b = 6 * aggregate[b] as usize;
                for &(p, vp) in &pa {
                    for &(q, vq) in &pb {
                        let t = vp * v * vq;
                        coarse[(base_a + p) * m + base_b + q] += t;
                        if r != c {
                            coarse[(base_b + q) * m + base_a + p] += t;
                        }
                    }
                }
            }
        }
        // Cholesky padat di tempat dengan pembuangan pivot mati.
        let mut dead = vec![false; m];
        for j in 0..m {
            let original = coarse[j * m + j];
            let mut d = original;
            for t in 0..j {
                d -= coarse[j * m + t] * coarse[j * m + t];
            }
            if !(original > 0.0 && d.is_finite() && d > DEAD_PIVOT * original) {
                dead[j] = true;
                for t in 0..j {
                    coarse[j * m + t] = 0.0;
                }
                coarse[j * m + j] = 1.0;
                for i in (j + 1)..m {
                    coarse[i * m + j] = 0.0;
                }
                continue;
            }
            let ljj = d.sqrt();
            coarse[j * m + j] = ljj;
            let (head, tail) = coarse.split_at_mut((j + 1) * m);
            let row_j = &head[j * m..j * m + j];
            for row_i in tail.chunks_exact_mut(m) {
                let mut s = row_i[j];
                for (x, y) in row_i[..j].iter().zip(row_j) {
                    s -= x * y;
                }
                row_i[j] = s / ljj;
            }
        }
        Some(TwoLevel {
            smoother: BlockJacobi3::new(k),
            aggregate,
            rel,
            fixed: fixed.to_vec(),
            m,
            chol: coarse,
            dead,
        })
    }

    /// Jumlah agregat.
    pub fn aggregates(&self) -> usize {
        self.m / 6
    }
}

impl Preconditioner for TwoLevel {
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        self.smoother.apply(r, z);
        let m = self.m;
        // Restriksi: y = Pᵀ·r.
        let mut y = vec![0.0; m];
        for (dof, &rv) in r.iter().enumerate() {
            if rv == 0.0 || self.fixed[dof] {
                continue;
            }
            let a = dof / 3;
            let base = 6 * self.aggregate[a] as usize;
            for (p, vp) in rigid_row(dof % 3, self.rel[a]) {
                y[base + p] += vp * rv;
            }
        }
        for j in 0..m {
            if self.dead[j] {
                y[j] = 0.0;
            }
        }
        // L·Lᵀ·y = rhs.
        for i in 0..m {
            let row = &self.chol[i * m..i * m + i];
            let s: f64 = row.iter().zip(&y[..i]).map(|(l, v)| l * v).sum();
            y[i] = (y[i] - s) / self.chol[i * m + i];
        }
        for i in (0..m).rev() {
            let yi = y[i] / self.chol[i * m + i];
            y[i] = yi;
            let row = &self.chol[i * m..i * m + i];
            for (v, l) in y[..i].iter_mut().zip(row) {
                *v -= l * yi;
            }
        }
        // Prolongasi: z += P·y.
        for (dof, zv) in z.iter_mut().enumerate() {
            if self.fixed[dof] {
                continue;
            }
            let a = dof / 3;
            let base = 6 * self.aggregate[a] as usize;
            for (p, vp) in rigid_row(dof % 3, self.rel[a]) {
                *zv += vp * y[base + p];
            }
        }
    }
}
