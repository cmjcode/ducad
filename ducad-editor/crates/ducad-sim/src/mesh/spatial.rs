//! Indeks ruang untuk mesher tet: grid sel seragam (CSR), hash titik
//! dinamis untuk penolakan Poisson, dan indeks permukaan (titik terdekat
//! pada segitiga + uji dalam/luar dengan sinar).

use crate::linalg::{cross, dot, norm, normalize, sub, V3};
use crate::mesh::voxel::{closest_on_triangle, tri_points};
use crate::SurfaceMesh;

/// Grid sel kubik dengan daftar item per sel (CSR).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CellGrid {
    pub min: V3,
    pub cell: f64,
    pub dims: [usize; 3],
    ptr: Vec<u32>,
    items: Vec<u32>,
}

impl CellGrid {
    /// Grid yang menutup `[min, max]` dengan sel ≥ `cell`, dibesarkan bila
    /// jumlah sel melebihi `max_cells`.
    pub fn new(min: V3, max: V3, cell: f64, max_cells: usize) -> CellGrid {
        let extent = sub(max, min);
        let mut cell = cell.max(1e-12 * norm(extent)).max(f64::MIN_POSITIVE);
        let mut dims = [1usize; 3];
        for _ in 0..64 {
            let mut total = 1.0_f64;
            for a in 0..3 {
                let n = (extent[a] / cell).floor() + 1.0;
                dims[a] = if n.is_finite() {
                    n.clamp(1.0, 4096.0) as usize
                } else {
                    1
                };
                total *= dims[a] as f64;
            }
            if total <= max_cells as f64 {
                break;
            }
            cell *= 1.26;
        }
        let cells = dims[0] * dims[1] * dims[2];
        CellGrid {
            min,
            cell,
            dims,
            ptr: vec![0; cells + 1],
            items: Vec::new(),
        }
    }

    pub fn coord(&self, v: f64, a: usize) -> usize {
        let t = ((v - self.min[a]) / self.cell).floor();
        if t.is_finite() {
            t.clamp(0.0, (self.dims[a] - 1) as f64) as usize
        } else {
            0
        }
    }

    pub fn cell_of(&self, p: V3) -> [usize; 3] {
        [
            self.coord(p[0], 0),
            self.coord(p[1], 1),
            self.coord(p[2], 2),
        ]
    }

    pub fn index(&self, c: [usize; 3]) -> usize {
        c[0] + self.dims[0] * (c[1] + self.dims[1] * c[2])
    }

    /// Mengisi grid dari pasangan `(sel, item)`.
    pub fn fill(&mut self, mut pairs: Vec<(u32, u32)>) {
        pairs.sort_unstable();
        pairs.dedup();
        self.ptr.iter_mut().for_each(|v| *v = 0);
        for &(c, _) in &pairs {
            self.ptr[c as usize + 1] += 1;
        }
        for c in 0..self.ptr.len() - 1 {
            self.ptr[c + 1] += self.ptr[c];
        }
        self.items = pairs.into_iter().map(|p| p.1).collect();
    }

    pub fn items(&self, c: [usize; 3]) -> &[u32] {
        let i = self.index(c);
        &self.items[self.ptr[i] as usize..self.ptr[i + 1] as usize]
    }

    /// Menambahkan pasangan untuk semua sel yang disentuh kotak `[lo, hi]`.
    pub fn push_box(&self, lo: V3, hi: V3, item: u32, pairs: &mut Vec<(u32, u32)>) {
        let a = self.cell_of(lo);
        let b = self.cell_of(hi);
        for k in a[2]..=b[2] {
            for j in a[1]..=b[1] {
                for i in a[0]..=b[0] {
                    pairs.push((self.index([i, j, k]) as u32, item));
                }
            }
        }
    }
}

/// Hash titik dinamis (daftar berantai per sel) untuk kueri "ada titik dalam
/// radius r?".
pub(crate) struct PointHash {
    grid: CellGrid,
    head: Vec<u32>,
    next: Vec<u32>,
    pub pts: Vec<V3>,
}

const NIL: u32 = u32::MAX;

impl PointHash {
    pub fn new(min: V3, max: V3, cell: f64) -> PointHash {
        let grid = CellGrid::new(min, max, cell, 2_000_000);
        let cells = grid.dims[0] * grid.dims[1] * grid.dims[2];
        PointHash {
            grid,
            head: vec![NIL; cells],
            next: Vec::new(),
            pts: Vec::new(),
        }
    }

    pub fn insert(&mut self, p: V3) -> u32 {
        let id = self.pts.len() as u32;
        let c = self.grid.index(self.grid.cell_of(p));
        self.pts.push(p);
        self.next.push(self.head[c]);
        self.head[c] = id;
        id
    }

    /// Memanggil `f(indeks, jarak)` untuk tiap titik dalam radius `r`.
    pub fn for_each_within(&self, p: V3, r: f64, mut f: impl FnMut(u32, f64)) {
        let lo = self.grid.cell_of([p[0] - r, p[1] - r, p[2] - r]);
        let hi = self.grid.cell_of([p[0] + r, p[1] + r, p[2] + r]);
        let r2 = r * r;
        for k in lo[2]..=hi[2] {
            for j in lo[1]..=hi[1] {
                for i in lo[0]..=hi[0] {
                    let mut at = self.head[self.grid.index([i, j, k])];
                    while at != NIL {
                        let q = self.pts[at as usize];
                        let d2 =
                            (q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2);
                        if d2 <= r2 {
                            f(at, d2.sqrt());
                        }
                        at = self.next[at as usize];
                    }
                }
            }
        }
    }

    pub fn any_within(&self, p: V3, r: f64) -> bool {
        let mut found = false;
        self.for_each_within(p, r, |_, _| found = true);
        found
    }
}

/// Titik terdekat pada permukaan.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Closest {
    pub dist: f64,
    pub point: V3,
    pub tri: u32,
}

/// Indeks permukaan: segitiga per sel + ember (y, z) untuk sinar +X.
pub(crate) struct SurfaceIndex<'a> {
    pub surface: &'a SurfaceMesh,
    grid: CellGrid,
    ray: CellGrid,
    pub normals: Vec<V3>,
    jitter: f64,
}

impl<'a> SurfaceIndex<'a> {
    pub fn new(surface: &'a SurfaceMesh, min: V3, max: V3, cell: f64) -> SurfaceIndex<'a> {
        let diag = norm(sub(max, min));
        let pad = 1e-6 * diag;
        let lo = [min[0] - pad, min[1] - pad, min[2] - pad];
        let hi = [max[0] + pad, max[1] + pad, max[2] + pad];
        let mut grid = CellGrid::new(lo, hi, cell, 250_000);
        let side = ((surface.triangles.len() as f64).sqrt() * 2.0).clamp(4.0, 256.0);
        let ray_cell = ((hi[1] - lo[1]).max(hi[2] - lo[2]) / side).max(1e-9 * diag);
        let mut ray = CellGrid::new(
            [lo[0], lo[1], lo[2]],
            [lo[0], hi[1], hi[2]],
            ray_cell,
            70_000,
        );
        let mut pairs = Vec::new();
        let mut ray_pairs = Vec::new();
        let mut normals = Vec::with_capacity(surface.triangles.len());
        for t in 0..surface.triangles.len() {
            let [a, b, c] = tri_points(surface, t);
            let mut tlo = a;
            let mut thi = a;
            for p in [b, c] {
                for k in 0..3 {
                    tlo[k] = tlo[k].min(p[k]);
                    thi[k] = thi[k].max(p[k]);
                }
            }
            grid.push_box(tlo, thi, t as u32, &mut pairs);
            ray.push_box(
                [lo[0], tlo[1], tlo[2]],
                [lo[0], thi[1], thi[2]],
                t as u32,
                &mut ray_pairs,
            );
            normals.push(normalize(cross(sub(b, a), sub(c, a))).unwrap_or([0.0; 3]));
        }
        grid.fill(pairs);
        ray.fill(ray_pairs);
        SurfaceIndex {
            surface,
            grid,
            ray,
            normals,
            jitter: 1e-7 * diag,
        }
    }

    /// Titik permukaan terdekat dari `p`. `face` membatasi ke satu tag;
    /// `align = (normal, penalti)` menambah penalti bila normal segitiga
    /// tidak searah `normal` (memecah seri di dekat tepi antar-face).
    pub fn closest(&self, p: V3, face: Option<u32>, align: Option<(V3, f64)>) -> Option<Closest> {
        let c = self.grid.cell_of(p);
        let max_r = self.grid.dims[0]
            .max(self.grid.dims[1])
            .max(self.grid.dims[2]);
        let mut best: Option<(f64, Closest)> = None;
        for r in 0..=max_r {
            let lo = [
                c[0].saturating_sub(r),
                c[1].saturating_sub(r),
                c[2].saturating_sub(r),
            ];
            let hi = [
                (c[0] + r).min(self.grid.dims[0] - 1),
                (c[1] + r).min(self.grid.dims[1] - 1),
                (c[2] + r).min(self.grid.dims[2] - 1),
            ];
            for k in lo[2]..=hi[2] {
                for j in lo[1]..=hi[1] {
                    for i in lo[0]..=hi[0] {
                        let ring = i.abs_diff(c[0]).max(j.abs_diff(c[1])).max(k.abs_diff(c[2]));
                        if ring != r {
                            continue;
                        }
                        for &t in self.grid.items([i, j, k]) {
                            if face.is_some_and(|f| self.surface.tri_face[t as usize] != f) {
                                continue;
                            }
                            let [a, b, cc] = tri_points(self.surface, t as usize);
                            let q = closest_on_triangle(p, a, b, cc);
                            let dist = norm(sub(p, q));
                            let score = match align {
                                Some((n, pen)) => {
                                    dist + pen * (1.0 - dot(self.normals[t as usize], n))
                                }
                                None => dist,
                            };
                            if best.as_ref().is_none_or(|b| score < b.0) {
                                best = Some((
                                    score,
                                    Closest {
                                        dist,
                                        point: q,
                                        tri: t,
                                    },
                                ));
                            }
                        }
                    }
                }
            }
            if let Some((score, _)) = best {
                if score <= r as f64 * self.grid.cell {
                    break;
                }
            }
        }
        best.map(|b| b.1)
    }

    /// Satu sinar +X dari `p`; `None` bila sinar terlalu dekat tepi segitiga.
    fn ray_parity(&self, p: V3) -> Option<bool> {
        let c = self.ray.cell_of([self.ray.min[0], p[1], p[2]]);
        let mut inside = false;
        for &t in self.ray.items([0, c[1], c[2]]) {
            let [a, b, cc] = tri_points(self.surface, t as usize);
            let e1 = [b[1] - a[1], b[2] - a[2]];
            let e2 = [cc[1] - a[1], cc[2] - a[2]];
            let den = e1[0] * e2[1] - e2[0] * e1[1];
            if den == 0.0 {
                continue;
            }
            let (py, pz) = (p[1] - a[1], p[2] - a[2]);
            let w1 = (py * e2[1] - e2[0] * pz) / den;
            let w2 = (e1[0] * pz - py * e1[1]) / den;
            let w0 = 1.0 - w1 - w2;
            let tol = 1e-9;
            if w0 < -tol || w1 < -tol || w2 < -tol {
                continue;
            }
            if w0 < tol || w1 < tol || w2 < tol {
                return None;
            }
            let x = w0 * a[0] + w1 * b[0] + w2 * cc[0];
            if (x - p[0]).abs() <= self.jitter * 1e-3 {
                return None;
            }
            if x > p[0] {
                inside = !inside;
            }
        }
        Some(inside)
    }

    /// Uji dalam/luar dengan paritas sinar; sinar digeser sedikit bila
    /// mengenai tepi segitiga.
    pub fn inside(&self, p: V3) -> bool {
        const SHIFTS: [[f64; 2]; 6] = [
            [0.0, 0.0],
            [1.0, 0.618],
            [-0.731, 1.0],
            [0.377, -1.0],
            [-1.0, -0.283],
            [2.3, 1.7],
        ];
        for s in SHIFTS {
            let q = [p[0], p[1] + s[0] * self.jitter, p[2] + s[1] * self.jitter];
            if let Some(result) = self.ray_parity(q) {
                return result;
            }
        }
        false
    }
}
