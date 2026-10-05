//! Perakitan kekakuan global ke CSR simetris + penerapan syarat batas.
//!
//! - Hanya segitiga atas (termasuk diagonal) yang disimpan; diagonal selalu
//!   entri pertama tiap baris.
//! - DOF `Fixed` dieliminasi: baris/kolomnya dikosongkan dan diagonalnya 1,
//!   sehingga penomoran DOF tetap `3·node + sumbu`.
//! - `Roller`/`Symmetry`: penalti `PENALTY_FACTOR × maks diagonal` pada arah
//!   normal (`k·n·nᵀ` ditambahkan ke blok diagonal node).
//!
//! Modul ini hanya bergantung pada [`ElementModel`], bukan pada tipe elemen.

use crate::element::ElementModel;
use crate::SimError;

/// Skala penalti untuk kekangan arah normal: `1e6 × maks diagonal K`.
pub const PENALTY_FACTOR: f64 = 1.0e6;

/// Matriks simetris jarang, segitiga atas dalam CSR.
#[derive(Debug, Clone, PartialEq)]
pub struct CsrSym {
    pub n: usize,
    pub row_ptr: Vec<usize>,
    pub col: Vec<u32>,
    pub val: Vec<f64>,
}

impl CsrSym {
    /// Membangun dari triplet `(baris, kolom, nilai)`; duplikat dijumlahkan,
    /// entri segitiga bawah dicerminkan ke atas (cukup berikan salah satu
    /// sisi tiap pasangan), diagonal yang hilang diisi nol.
    pub fn from_triplets(n: usize, entries: &[(u32, u32, f64)]) -> Result<CsrSym, SimError> {
        let mut sorted: Vec<(u32, u32, f64)> = Vec::with_capacity(entries.len() + n);
        for &(r, c, v) in entries {
            if r as usize >= n || c as usize >= n {
                return Err(SimError::InvalidSetup(
                    "matrix entry index out of range".into(),
                ));
            }
            sorted.push((r.min(c), r.max(c), v));
        }
        for d in 0..n as u32 {
            sorted.push((d, d, 0.0));
        }
        sorted.sort_by_key(|a| (a.0, a.1));
        let mut row_ptr = vec![0usize; n + 1];
        let mut col = Vec::new();
        let mut val: Vec<f64> = Vec::new();
        let mut last: Option<(u32, u32)> = None;
        for (r, c, v) in sorted {
            if last == Some((r, c)) {
                if let Some(slot) = val.last_mut() {
                    *slot += v;
                }
            } else {
                col.push(c);
                val.push(v);
                row_ptr[r as usize + 1] += 1;
                last = Some((r, c));
            }
        }
        for r in 0..n {
            row_ptr[r + 1] += row_ptr[r];
        }
        Ok(CsrSym {
            n,
            row_ptr,
            col,
            val,
        })
    }

    /// `y = A·x`.
    pub fn mul(&self, x: &[f64], y: &mut [f64]) {
        y.fill(0.0);
        for r in 0..self.n {
            let start = self.row_ptr[r];
            let end = self.row_ptr[r + 1];
            if start == end {
                continue;
            }
            let xr = x[r];
            let cols = &self.col[start + 1..end];
            let vals = &self.val[start + 1..end];
            let mut s = self.val[start] * xr;
            for (&c, &v) in cols.iter().zip(vals) {
                s += v * x[c as usize];
                y[c as usize] += v * xr;
            }
            y[r] += s;
        }
    }

    pub fn diagonal(&self) -> Vec<f64> {
        (0..self.n)
            .map(|r| {
                if self.row_ptr[r] < self.row_ptr[r + 1] {
                    self.val[self.row_ptr[r]]
                } else {
                    0.0
                }
            })
            .collect()
    }
}

impl CsrSym {
    /// Mengeliminasi baris/kolom `fixed`: entri dinolkan, diagonal menjadi 1.
    pub fn eliminate(&mut self, fixed: &[bool]) {
        for r in 0..self.n {
            for at in self.row_ptr[r]..self.row_ptr[r + 1] {
                if fixed[r] || fixed[self.col[at] as usize] {
                    self.val[at] = 0.0;
                }
            }
            if fixed[r] && self.row_ptr[r] < self.row_ptr[r + 1] {
                self.val[self.row_ptr[r]] = 1.0;
            }
        }
    }
}

/// Pola tetangga tingkat node (CSR): untuk tiap node, tetangga ≥ dirinya
/// yang berbagi elemen, terurut (dirinya selalu pertama).
fn node_pattern(model: &dyn ElementModel) -> Result<(Vec<usize>, Vec<u32>), SimError> {
    let nn = model.num_nodes();
    let ne = model.num_elems();
    let mut adj_ptr = vec![0usize; nn + 1];
    for e in 0..ne {
        for &n in model.elem_nodes(e) {
            if n as usize >= nn {
                return Err(SimError::InvalidSetup(
                    "element node index out of range".into(),
                ));
            }
            adj_ptr[n as usize + 1] += 1;
        }
    }
    for n in 0..nn {
        adj_ptr[n + 1] += adj_ptr[n];
    }
    let mut adj = vec![0u32; adj_ptr[nn]];
    let mut cursor = adj_ptr.clone();
    for e in 0..ne {
        for &n in model.elem_nodes(e) {
            adj[cursor[n as usize]] = e as u32;
            cursor[n as usize] += 1;
        }
    }
    let mut nbr_ptr = vec![0usize; nn + 1];
    let mut nbr: Vec<u32> = Vec::new();
    let mut scratch: Vec<u32> = Vec::new();
    for a in 0..nn {
        scratch.clear();
        scratch.push(a as u32);
        for &e in &adj[adj_ptr[a]..adj_ptr[a + 1]] {
            for &b in model.elem_nodes(e as usize) {
                if b as usize > a {
                    scratch.push(b);
                }
            }
        }
        scratch.sort_unstable();
        scratch.dedup();
        nbr.extend_from_slice(&scratch);
        nbr_ptr[a + 1] = nbr.len();
    }
    Ok((nbr_ptr, nbr))
}

/// Merakit matriks skalar tingkat node (satu DOF per node) dari matriks
/// elemen `npe²` yang diisi `fill(e, out)`: massa, konduksi, atau kekakuan
/// geometri. Tanpa eliminasi.
pub fn assemble_scalar(
    model: &dyn ElementModel,
    mut fill: impl FnMut(usize, &mut [f64]),
) -> Result<CsrSym, SimError> {
    let nn = model.num_nodes();
    let npe = model.nodes_per_elem();
    let (nbr_ptr, nbr) = node_pattern(model)?;
    let mut val = vec![0.0; nbr.len()];
    let mut me = vec![0.0; npe * npe];
    for e in 0..model.num_elems() {
        fill(e, &mut me);
        let conn = model.elem_nodes(e);
        for la in 0..npe {
            let na = conn[la] as usize;
            let list = &nbr[nbr_ptr[na]..nbr_ptr[na + 1]];
            for lb in 0..npe {
                let nb = conn[lb];
                if (nb as usize) < na {
                    continue;
                }
                if let Ok(p) = list.binary_search(&nb) {
                    val[nbr_ptr[na] + p] += me[la * npe + lb];
                }
            }
        }
    }
    Ok(CsrSym {
        n: nn,
        row_ptr: nbr_ptr,
        col: nbr,
        val,
    })
}

/// Kekangan penalti arah normal pada satu node.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Penalty {
    pub node: u32,
    /// Arah satuan yang dikekang.
    pub dir: [f64; 3],
    /// Indeks fixture pemilik (untuk gaya reaksi).
    pub owner: u32,
}

/// Hasil perakitan.
#[derive(Debug, Clone, PartialEq)]
pub struct Assembled {
    pub k: CsrSym,
    /// Nilai penalti yang dipakai (N/mm).
    pub penalty: f64,
}

/// Merakit K global dengan DOF `fixed` dieliminasi dan penalti ditambahkan.
pub fn assemble_stiffness(
    model: &dyn ElementModel,
    fixed: &[bool],
    penalties: &[Penalty],
) -> Result<Assembled, SimError> {
    let nn = model.num_nodes();
    let ne = model.num_elems();
    let npe = model.nodes_per_elem();
    let ndof = 3 * nn;
    if fixed.len() != ndof {
        return Err(SimError::InvalidSetup(
            "constraint vector length mismatch".into(),
        ));
    }
    // Node → elemen (CSR).
    let mut adj_ptr = vec![0usize; nn + 1];
    for e in 0..ne {
        for &n in model.elem_nodes(e) {
            if n as usize >= nn {
                return Err(SimError::InvalidSetup(
                    "element node index out of range".into(),
                ));
            }
            adj_ptr[n as usize + 1] += 1;
        }
    }
    for n in 0..nn {
        adj_ptr[n + 1] += adj_ptr[n];
    }
    let mut adj = vec![0u32; adj_ptr[nn]];
    let mut cursor = adj_ptr.clone();
    for e in 0..ne {
        for &n in model.elem_nodes(e) {
            adj[cursor[n as usize]] = e as u32;
            cursor[n as usize] += 1;
        }
    }
    // Pola tingkat node: tetangga ≥ dirinya, terurut (dirinya selalu pertama).
    let mut nbr_ptr = vec![0usize; nn + 1];
    let mut nbr: Vec<u32> = Vec::new();
    let mut scratch: Vec<u32> = Vec::new();
    for a in 0..nn {
        scratch.clear();
        scratch.push(a as u32);
        for &e in &adj[adj_ptr[a]..adj_ptr[a + 1]] {
            for &b in model.elem_nodes(e as usize) {
                if b as usize > a {
                    scratch.push(b);
                }
            }
        }
        scratch.sort_unstable();
        scratch.dedup();
        nbr.extend_from_slice(&scratch);
        nbr_ptr[a + 1] = nbr.len();
    }
    // Pola tingkat DOF.
    let mut row_ptr = vec![0usize; ndof + 1];
    for a in 0..nn {
        let others = nbr_ptr[a + 1] - nbr_ptr[a] - 1;
        for i in 0..3 {
            row_ptr[3 * a + i + 1] = row_ptr[3 * a + i] + (3 - i) + 3 * others;
        }
    }
    let nnz = row_ptr[ndof];
    let mut col = vec![0u32; nnz];
    for a in 0..nn {
        let list = &nbr[nbr_ptr[a]..nbr_ptr[a + 1]];
        for i in 0..3 {
            let mut at = row_ptr[3 * a + i];
            for j in i..3 {
                col[at] = (3 * a + j) as u32;
                at += 1;
            }
            for &b in &list[1..] {
                for j in 0..3 {
                    col[at] = 3 * b + j as u32;
                    at += 1;
                }
            }
        }
    }
    // Nilai.
    let nd = 3 * npe;
    let mut val = vec![0.0; nnz];
    let mut ke = vec![0.0; nd * nd];
    for e in 0..ne {
        model.stiffness(e, &mut ke);
        let conn = model.elem_nodes(e);
        for la in 0..npe {
            let na = conn[la] as usize;
            let list = &nbr[nbr_ptr[na]..nbr_ptr[na + 1]];
            for lb in 0..npe {
                let nb = conn[lb] as usize;
                if nb < na {
                    continue;
                }
                let Ok(p) = list.binary_search(&(nb as u32)) else {
                    continue;
                };
                for i in 0..3 {
                    let r = 3 * na + i;
                    if fixed[r] {
                        continue;
                    }
                    let base = row_ptr[r];
                    for j in 0..3 {
                        if fixed[3 * nb + j] {
                            continue;
                        }
                        let idx = if p == 0 {
                            if j < i {
                                continue;
                            }
                            base + (j - i)
                        } else {
                            base + (3 - i) + 3 * (p - 1) + j
                        };
                        val[idx] += ke[(3 * la + i) * nd + 3 * lb + j];
                    }
                }
            }
        }
    }
    let mut max_diag = 0.0_f64;
    for r in 0..ndof {
        if fixed[r] {
            val[row_ptr[r]] = 1.0;
        } else {
            max_diag = max_diag.max(val[row_ptr[r]]);
        }
    }
    if !max_diag.is_finite() {
        return Err(SimError::InvalidSetup(
            "stiffness matrix is not finite".into(),
        ));
    }
    let penalty = PENALTY_FACTOR * max_diag;
    for pen in penalties {
        let a = pen.node as usize;
        if a >= nn {
            return Err(SimError::InvalidSetup(
                "constraint node index out of range".into(),
            ));
        }
        for i in 0..3 {
            if fixed[3 * a + i] {
                continue;
            }
            for j in i..3 {
                if fixed[3 * a + j] {
                    continue;
                }
                val[row_ptr[3 * a + i] + (j - i)] += penalty * pen.dir[i] * pen.dir[j];
            }
        }
    }
    Ok(Assembled {
        k: CsrSym {
            n: ndof,
            row_ptr,
            col,
            val,
        },
        penalty,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assemble_csr_from_triplets_and_mul() {
        // [[4,1,0],[1,3,2],[0,2,5]]
        let a = CsrSym::from_triplets(
            3,
            &[
                (0, 0, 4.0),
                (1, 0, 1.0),
                (1, 1, 3.0),
                (1, 2, 1.0),
                (1, 2, 1.0),
                (2, 2, 5.0),
            ],
        )
        .unwrap();
        let mut y = [0.0; 3];
        a.mul(&[1.0, 2.0, 3.0], &mut y);
        assert_eq!(y, [6.0, 13.0, 19.0]);
        assert_eq!(a.diagonal(), vec![4.0, 3.0, 5.0]);
        assert!(CsrSym::from_triplets(2, &[(0, 5, 1.0)]).is_err());
    }
}
