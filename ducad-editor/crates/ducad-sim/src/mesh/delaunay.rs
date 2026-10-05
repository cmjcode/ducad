//! Triangulasi Delaunay 3D inkremental (Bowyer–Watson) dengan predikat eksak.
//!
//! Titik disisipkan menurut urutan Morton (lokalitas ruang → jalan pencarian
//! pendek), ke dalam tetrahedron super yang melingkupi semuanya. Tiap
//! penyisipan: cari tet yang memuat titik (visibility walk), kumpulkan
//! rongga = tet yang bola luarnya memuat titik, lalu hubungkan sisi-sisi
//! rongga ke titik baru. Karena `orient3d`/`insphere` bertanda eksak, rongga
//! selalu berbentuk bintang dan tidak pernah lahir tet bervolume nol.

use crate::mesh::predicates::{insphere, orient3d};
use crate::CancelToken;

const NONE: u32 = u32::MAX;

/// Sisi di seberang verteks lokal `i`, diurutkan sehingga
/// `orient3d(f0, f1, f2, v_i) > 0` untuk tet berorientasi positif.
pub const FACE: [[usize; 3]; 4] = [[1, 3, 2], [0, 2, 3], [0, 3, 1], [0, 1, 2]];

/// Galat mesher tet (internal; diubah menjadi fallback hex oleh pemanggil).
#[derive(Debug, Clone, PartialEq)]
pub enum TetError {
    Cancelled,
    Failed(String),
}

impl TetError {
    pub fn failed(msg: impl Into<String>) -> TetError {
        TetError::Failed(msg.into())
    }
}

/// Hasil triangulasi: tet berorientasi positif (indeks ke titik masukan) dan
/// titik yang dilewati karena kembar.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Triangulation {
    pub tets: Vec<[u32; 4]>,
    pub skipped: Vec<u32>,
}

struct Builder {
    pts: Vec<[f64; 3]>,
    tets: Vec<[u32; 4]>,
    adj: Vec<[u32; 4]>,
    alive: Vec<bool>,
    free: Vec<u32>,
    mark: Vec<u32>,
    epoch: u32,
    last: u32,
    cavity: Vec<u32>,
    boundary: Vec<(u32, u8)>,
    edges: Vec<(u64, u32, u8)>,
}

impl Builder {
    fn alloc(&mut self, verts: [u32; 4], adj: [u32; 4]) -> u32 {
        if let Some(slot) = self.free.pop() {
            let s = slot as usize;
            self.tets[s] = verts;
            self.adj[s] = adj;
            self.alive[s] = true;
            slot
        } else {
            self.tets.push(verts);
            self.adj.push(adj);
            self.alive.push(true);
            self.mark.push(0);
            (self.tets.len() - 1) as u32
        }
    }

    fn p(&self, v: u32) -> [f64; 3] {
        self.pts[v as usize]
    }

    fn contains(&self, t: u32, p: [f64; 3]) -> bool {
        let v = self.tets[t as usize];
        FACE.iter()
            .all(|f| orient3d(self.p(v[f[0]]), self.p(v[f[1]]), self.p(v[f[2]]), p) >= 0.0)
    }

    /// Tet yang memuat `p`.
    fn locate(&self, p: [f64; 3]) -> Result<u32, TetError> {
        let mut t = self.last;
        if !self.alive.get(t as usize).copied().unwrap_or(false) {
            t = (0..self.tets.len() as u32)
                .find(|&i| self.alive[i as usize])
                .ok_or_else(|| TetError::failed("delaunay: empty triangulation"))?;
        }
        let limit = self.tets.len() * 2 + 64;
        'walk: for step in 0..limit {
            let v = self.tets[t as usize];
            for k in 0..4 {
                let i = (k + step) % 4;
                let f = FACE[i];
                if orient3d(self.p(v[f[0]]), self.p(v[f[1]]), self.p(v[f[2]]), p) < 0.0 {
                    let n = self.adj[t as usize][i];
                    if n == NONE {
                        break 'walk;
                    }
                    t = n;
                    continue 'walk;
                }
            }
            return Ok(t);
        }
        // Jalan berputar atau keluar: cari menyeluruh.
        (0..self.tets.len() as u32)
            .find(|&i| self.alive[i as usize] && self.contains(i, p))
            .ok_or_else(|| TetError::failed("delaunay: point location failed"))
    }

    fn in_sphere(&self, t: u32, p: [f64; 3]) -> bool {
        let v = self.tets[t as usize];
        insphere(self.p(v[0]), self.p(v[1]), self.p(v[2]), self.p(v[3]), p) > 0.0
    }

    /// Menyisipkan titik `index`; `Ok(false)` bila titik kembar dan dilewati.
    fn insert(&mut self, index: u32) -> Result<bool, TetError> {
        let p = self.p(index);
        let start = self.locate(p)?;
        if !self.in_sphere(start, p) {
            return Ok(false);
        }
        self.epoch += 2;
        let inside = self.epoch;
        let outside = self.epoch + 1;
        self.cavity.clear();
        self.cavity.push(start);
        self.mark[start as usize] = inside;
        let mut head = 0;
        while head < self.cavity.len() {
            let t = self.cavity[head];
            head += 1;
            for i in 0..4 {
                let n = self.adj[t as usize][i];
                if n == NONE || self.mark[n as usize] == inside || self.mark[n as usize] == outside
                {
                    continue;
                }
                if self.in_sphere(n, p) {
                    self.mark[n as usize] = inside;
                    self.cavity.push(n);
                } else {
                    self.mark[n as usize] = outside;
                }
            }
        }
        // Sisi batas rongga; perluas rongga bila ada sisi yang tidak terlihat
        // dari `p` (tidak terjadi dengan predikat eksak, tetapi dijaga).
        for _ in 0..64 {
            self.boundary.clear();
            let mut grow: Option<u32> = None;
            for &t in &self.cavity {
                let v = self.tets[t as usize];
                for i in 0..4 {
                    let n = self.adj[t as usize][i];
                    if n != NONE && self.mark[n as usize] == inside {
                        continue;
                    }
                    let f = FACE[i];
                    if orient3d(self.p(v[f[0]]), self.p(v[f[1]]), self.p(v[f[2]]), p) <= 0.0 {
                        if n == NONE {
                            return Err(TetError::failed(
                                "delaunay: cavity is not star-shaped at the hull",
                            ));
                        }
                        grow = Some(n);
                    }
                    self.boundary.push((t, i as u8));
                }
            }
            match grow {
                None => break,
                Some(n) => {
                    self.mark[n as usize] = inside;
                    self.cavity.push(n);
                }
            }
        }
        // Tet baru.
        self.edges.clear();
        let boundary = std::mem::take(&mut self.boundary);
        let mut first_new = NONE;
        for &(t, i) in &boundary {
            let v = self.tets[t as usize];
            let f = FACE[i as usize];
            let verts = [v[f[0]], v[f[1]], v[f[2]], index];
            if orient3d(self.p(verts[0]), self.p(verts[1]), self.p(verts[2]), p) <= 0.0 {
                self.boundary = boundary;
                return Err(TetError::failed("delaunay: cavity is not star-shaped"));
            }
            let n = self.adj[t as usize][i as usize];
            let new = self.alloc_fresh(verts, [NONE, NONE, NONE, n]);
            if n != NONE {
                for k in 0..4 {
                    if self.adj[n as usize][k] == t {
                        self.adj[n as usize][k] = new;
                    }
                }
            }
            for j in 0..3 {
                let a = verts[(j + 1) % 3];
                let b = verts[(j + 2) % 3];
                let key = (u64::from(a.min(b)) << 32) | u64::from(a.max(b));
                self.edges.push((key, new, j as u8));
            }
            first_new = new;
        }
        self.boundary = boundary;
        self.edges.sort_unstable();
        if !self.edges.len().is_multiple_of(2) {
            return Err(TetError::failed("delaunay: cavity boundary is not closed"));
        }
        for pair in self.edges.chunks_exact(2) {
            if pair[0].0 != pair[1].0 {
                return Err(TetError::failed(
                    "delaunay: cavity boundary is not a manifold",
                ));
            }
            self.adj[pair[0].1 as usize][pair[0].2 as usize] = pair[1].1;
            self.adj[pair[1].1 as usize][pair[1].2 as usize] = pair[0].1;
        }
        // Baru sekarang slot rongga dilepas.
        for k in 0..self.cavity.len() {
            let t = self.cavity[k];
            self.alive[t as usize] = false;
            self.free.push(t);
        }
        self.last = first_new;
        Ok(true)
    }

    /// Alokasi yang tidak memakai ulang slot rongga yang sedang dibongkar
    /// (slot itu baru masuk `free` setelah penyisipan selesai).
    fn alloc_fresh(&mut self, verts: [u32; 4], adj: [u32; 4]) -> u32 {
        self.alloc(verts, adj)
    }
}

fn morton_key(q: [u32; 3]) -> u64 {
    fn spread(mut x: u64) -> u64 {
        x &= 0x1f_ffff;
        x = (x | (x << 32)) & 0x1f_0000_0000_ffff;
        x = (x | (x << 16)) & 0x1f_0000_ff00_00ff;
        x = (x | (x << 8)) & 0x100f_00f0_0f00_f00f;
        x = (x | (x << 4)) & 0x10c3_0c30_c30c_30c3;
        x = (x | (x << 2)) & 0x1249_2492_4924_9249;
        x
    }
    spread(u64::from(q[0])) | (spread(u64::from(q[1])) << 1) | (spread(u64::from(q[2])) << 2)
}

/// Triangulasi Delaunay titik-titik `points`.
pub fn delaunay(points: &[[f64; 3]], cancel: &CancelToken) -> Result<Triangulation, TetError> {
    let n = points.len();
    if n < 4 {
        return Err(TetError::failed("delaunay: fewer than four points"));
    }
    if n > (u32::MAX / 8) as usize {
        return Err(TetError::failed("delaunay: too many points"));
    }
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for p in points {
        for a in 0..3 {
            if !p[a].is_finite() {
                return Err(TetError::failed("delaunay: non-finite point"));
            }
            min[a] = min[a].min(p[a]);
            max[a] = max[a].max(p[a]);
        }
    }
    let diag =
        ((max[0] - min[0]).powi(2) + (max[1] - min[1]).powi(2) + (max[2] - min[2]).powi(2)).sqrt();
    if diag.is_nan() || diag <= 0.0 {
        return Err(TetError::failed("delaunay: all points coincide"));
    }
    let center = [
        0.5 * (min[0] + max[0]),
        0.5 * (min[1] + max[1]),
        0.5 * (min[2] + max[2]),
    ];
    let s = 100.0 * diag;
    let mut pts = points.to_vec();
    for d in [
        [1.0, 1.0, 1.0],
        [1.0, -1.0, -1.0],
        [-1.0, 1.0, -1.0],
        [-1.0, -1.0, 1.0],
    ] {
        pts.push([
            center[0] + s * d[0],
            center[1] + s * d[1],
            center[2] + s * d[2],
        ]);
    }
    let base = n as u32;
    let mut root = [base, base + 1, base + 2, base + 3];
    if orient3d(pts[n], pts[n + 1], pts[n + 2], pts[n + 3]) < 0.0 {
        root.swap(0, 1);
    }
    let mut builder = Builder {
        pts,
        tets: Vec::with_capacity(7 * n),
        adj: Vec::with_capacity(7 * n),
        alive: Vec::with_capacity(7 * n),
        free: Vec::new(),
        mark: Vec::with_capacity(7 * n),
        epoch: 0,
        last: 0,
        cavity: Vec::new(),
        boundary: Vec::new(),
        edges: Vec::new(),
    };
    builder.alloc(root, [NONE; 4]);

    let mut order: Vec<(u64, u32)> = (0..n as u32)
        .map(|i| {
            let p = points[i as usize];
            let mut q = [0u32; 3];
            for a in 0..3 {
                let span = (max[a] - min[a]).max(diag * 1e-12);
                q[a] = (((p[a] - min[a]) / span) * 1023.0).clamp(0.0, 1023.0) as u32;
            }
            (morton_key(q), i)
        })
        .collect();
    order.sort_unstable();

    let mut skipped = Vec::new();
    for (count, &(_, index)) in order.iter().enumerate() {
        if count % 1024 == 0 && cancel.is_cancelled() {
            return Err(TetError::Cancelled);
        }
        if !builder.insert(index)? {
            skipped.push(index);
        }
    }
    skipped.sort_unstable();
    let tets = (0..builder.tets.len())
        .filter(|&t| builder.alive[t] && builder.tets[t].iter().all(|&v| v < base))
        .map(|t| builder.tets[t])
        .collect();
    Ok(Triangulation { tets, skipped })
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn volume(points: &[[f64; 3]], t: [u32; 4]) -> f64 {
        let p = |i: usize| points[t[i] as usize];
        let (a, b, c, d) = (p(0), p(1), p(2), p(3));
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let w = [d[0] - a[0], d[1] - a[1], d[2] - a[2]];
        (u[0] * (v[1] * w[2] - v[2] * w[1]) - u[1] * (v[0] * w[2] - v[2] * w[0])
            + u[2] * (v[0] * w[1] - v[1] * w[0]))
            / 6.0
    }

    #[test]
    fn delaunay_bowyer_watson_random_points_have_empty_spheres() {
        let mut rng = Lcg(2024);
        let points: Vec<[f64; 3]> = (0..300)
            .map(|_| [rng.next(), rng.next() * 2.0, rng.next() * 0.5])
            .collect();
        let tri = delaunay(&points, &CancelToken::new()).unwrap();
        assert!(tri.skipped.is_empty());
        assert!(tri.tets.len() > 1000);
        for &t in &tri.tets {
            assert!(volume(&points, t) > 0.0);
            let v = |i: usize| points[t[i] as usize];
            for (k, &q) in points.iter().enumerate() {
                if t.contains(&(k as u32)) {
                    continue;
                }
                assert!(
                    insphere(v(0), v(1), v(2), v(3), q) <= 0.0,
                    "titik {k} di dalam bola tet {t:?}"
                );
            }
        }
    }

    #[test]
    fn delaunay_bowyer_watson_handles_cubic_lattice_and_duplicates() {
        // Kisi kubik: kosferis dan koplanar di mana-mana. Tidak boleh panik,
        // tidak boleh ada tet bervolume ≤ 0, dan volume total = volume kubus.
        let n = 6;
        let mut points = Vec::new();
        for k in 0..n {
            for j in 0..n {
                for i in 0..n {
                    points.push([i as f64 * 0.7, j as f64 * 0.7, k as f64 * 0.7]);
                }
            }
        }
        // Titik kembar dilewati.
        points.push(points[17]);
        points.push(points[100]);
        let tri = delaunay(&points, &CancelToken::new()).unwrap();
        assert_eq!(tri.skipped.len(), 2);
        let mut total = 0.0;
        for &t in &tri.tets {
            let v = volume(&points, t);
            assert!(v > 0.0, "tet terbalik/nol: {t:?}");
            total += v;
        }
        let side = 0.7 * (n - 1) as f64;
        assert!(
            (total - side.powi(3)).abs() < 1e-9 * side.powi(3),
            "{total}"
        );
    }

    #[test]
    fn delaunay_bowyer_watson_rejects_degenerate_sets() {
        let cancel = CancelToken::new();
        assert!(delaunay(&[[0.0; 3]; 3], &cancel).is_err());
        assert!(delaunay(&[[1.0; 3]; 8], &cancel).is_err());
        assert!(delaunay(&[[f64::NAN; 3]; 8], &cancel).is_err());
        // Semua titik koplanar: tidak ada tet, tidak panik.
        let flat: Vec<[f64; 3]> = (0..25)
            .map(|i| [(i % 5) as f64, (i / 5) as f64, 0.0])
            .collect();
        let tri = delaunay(&flat, &cancel).unwrap();
        assert!(tri.tets.is_empty());
        cancel.cancel();
        let pts: Vec<[f64; 3]> = (0..10)
            .map(|i| [i as f64, (i * i) as f64, (i * 7 % 5) as f64])
            .collect();
        assert_eq!(delaunay(&pts, &cancel).unwrap_err(), TetError::Cancelled);
    }
}
