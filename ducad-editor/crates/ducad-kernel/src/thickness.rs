//! Tebal dinding minimum berbasis mesh (P7.3).
//!
//! Murni geometri atas [`KernelMesh`] — TANPA OCCT dan tanpa
//! `lock_kernel`, sehingga aman dijalankan di thread latar (GUI menghitung
//! check `min_wall` di belakang layar dengan `Arc<KernelMesh>`).
//!
//! Metode: untuk tiap sampel (centroid segitiga) tembakkan sinar dari sedikit
//! di dalam permukaan ke arah −normal; jarak ke permukaan berikutnya adalah
//! tebal material di titik itu. Pencarian perpotongan memakai BVH segitiga.

use crate::mesh::KernelMesh;

/// Hasil pengukuran tebal dinding.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallReport {
    /// Tebal terkecil (mm).
    pub min: f32,
    /// Lokasi sampel dengan tebal terkecil.
    pub at: [f32; 3],
    /// Persentil ke-5 — pembanding apakah `min` hanya artefak lokal.
    pub p05: f32,
    /// Jumlah sampel yang menghasilkan pengukuran.
    pub samples: usize,
}

/// Jumlah sampel default.
pub const DEFAULT_WALL_SAMPLES: usize = 4000;

const LEAF_SIZE: usize = 8;
const START_OFFSET: f32 = 1e-3;
const MIN_T: f32 = 1e-4;
const MIN_AREA: f32 = 1e-6;

type V3 = [f32; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: V3, s: f32) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn len(a: V3) -> f32 {
    dot(a, a).sqrt()
}

#[derive(Clone, Copy)]
struct Aabb {
    min: V3,
    max: V3,
}

impl Aabb {
    fn empty() -> Self {
        Self {
            min: [f32::MAX; 3],
            max: [f32::MIN; 3],
        }
    }

    fn grow(&mut self, p: V3) {
        for (i, v) in p.into_iter().enumerate() {
            self.min[i] = self.min[i].min(v);
            self.max[i] = self.max[i].max(v);
        }
    }

    fn union(mut self, o: &Aabb) -> Self {
        self.grow(o.min);
        self.grow(o.max);
        self
    }

    /// Uji slab: apakah sinar memotong kotak sebelum `t_max`.
    fn hit(&self, origin: V3, inv_dir: V3, t_max: f32) -> bool {
        let mut t0 = 0.0f32;
        let mut t1 = t_max;
        for i in 0..3 {
            let a = (self.min[i] - origin[i]) * inv_dir[i];
            let b = (self.max[i] - origin[i]) * inv_dir[i];
            let (lo, hi) = if a < b { (a, b) } else { (b, a) };
            // NaN (0 · ∞) gagal kedua perbandingan: tidak membatasi.
            if lo > t0 {
                t0 = lo;
            }
            if hi < t1 {
                t1 = hi;
            }
            if t0 > t1 {
                return false;
            }
        }
        true
    }
}

enum Node {
    Leaf {
        bounds: Aabb,
        tris: Vec<u32>,
    },
    Inner {
        bounds: Aabb,
        left: Box<Node>,
        right: Box<Node>,
    },
}

impl Node {
    fn bounds(&self) -> &Aabb {
        match self {
            Node::Leaf { bounds, .. } | Node::Inner { bounds, .. } => bounds,
        }
    }
}

struct Bvh<'a> {
    tris: &'a [[V3; 3]],
    root: Node,
}

impl<'a> Bvh<'a> {
    fn build(tris: &'a [[V3; 3]]) -> Self {
        let ids: Vec<u32> = (0..tris.len() as u32).collect();
        let root = Self::node(tris, ids);
        Self { tris, root }
    }

    fn node(tris: &[[V3; 3]], mut ids: Vec<u32>) -> Node {
        let mut bounds = Aabb::empty();
        let mut cb = Aabb::empty();
        for &i in &ids {
            let t = &tris[i as usize];
            for p in t {
                bounds.grow(*p);
            }
            cb.grow(centroid(t));
        }
        if ids.len() <= LEAF_SIZE {
            return Node::Leaf { bounds, tris: ids };
        }
        // Belah median pada sumbu terpanjang sebaran centroid.
        let ext = sub(cb.max, cb.min);
        let axis = if ext[0] >= ext[1] && ext[0] >= ext[2] {
            0
        } else if ext[1] >= ext[2] {
            1
        } else {
            2
        };
        if ext[axis] <= 0.0 {
            return Node::Leaf { bounds, tris: ids };
        }
        let mid = ids.len() / 2;
        ids.select_nth_unstable_by(mid, |a, b| {
            let ca = centroid(&tris[*a as usize])[axis];
            let cb = centroid(&tris[*b as usize])[axis];
            ca.partial_cmp(&cb).unwrap_or(std::cmp::Ordering::Equal)
        });
        let right = ids.split_off(mid);
        let l = Self::node(tris, ids);
        let r = Self::node(tris, right);
        Node::Inner {
            bounds: l.bounds().union(r.bounds()),
            left: Box::new(l),
            right: Box::new(r),
        }
    }

    /// `t` terkecil > `MIN_T` sepanjang sinar, mengabaikan segitiga `skip`.
    fn closest(&self, origin: V3, dir: V3, skip: u32) -> Option<f32> {
        let inv = [1.0 / dir[0], 1.0 / dir[1], 1.0 / dir[2]];
        let mut best = f32::MAX;
        let mut stack: Vec<&Node> = vec![&self.root];
        while let Some(n) = stack.pop() {
            if !n.bounds().hit(origin, inv, best) {
                continue;
            }
            match n {
                Node::Leaf { tris, .. } => {
                    for &i in tris {
                        if i == skip {
                            continue;
                        }
                        if let Some(t) = ray_triangle(origin, dir, &self.tris[i as usize]) {
                            if t > MIN_T && t < best {
                                best = t;
                            }
                        }
                    }
                }
                Node::Inner { left, right, .. } => {
                    stack.push(left);
                    stack.push(right);
                }
            }
        }
        (best < f32::MAX).then_some(best)
    }
}

fn centroid(t: &[V3; 3]) -> V3 {
    scale(add(add(t[0], t[1]), t[2]), 1.0 / 3.0)
}

/// Möller–Trumbore, dua sisi.
fn ray_triangle(origin: V3, dir: V3, t: &[V3; 3]) -> Option<f32> {
    const EPS: f32 = 1e-9;
    let e1 = sub(t[1], t[0]);
    let e2 = sub(t[2], t[0]);
    let p = cross(dir, e2);
    let det = dot(e1, p);
    if det.abs() < EPS {
        return None;
    }
    let inv = 1.0 / det;
    let s = sub(origin, t[0]);
    let u = dot(s, p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = cross(s, e1);
    let v = dot(dir, q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    Some(dot(e2, q) * inv)
}

/// Tebal dinding minimum `mesh`. Sampel = centroid segitiga; bila segitiga
/// lebih banyak dari `max_samples`, diambil setiap segitiga ke-`k`
/// (deterministik, tanpa RNG). `None` bila tidak ada sampel yang
/// menghasilkan pengukuran.
pub fn min_wall_thickness(mesh: &KernelMesh, max_samples: usize) -> Option<WallReport> {
    let n = mesh.indices.len() / 3;
    if n == 0 {
        return None;
    }
    let vertex = |i: u32| mesh.positions.get(i as usize).copied();
    let mut tris: Vec<[V3; 3]> = Vec::with_capacity(n);
    for c in mesh.indices.chunks_exact(3) {
        let (Some(a), Some(b), Some(d)) = (vertex(c[0]), vertex(c[1]), vertex(c[2])) else {
            return None;
        };
        tris.push([a, b, d]);
    }
    let bvh = Bvh::build(&tris);
    let step = n.div_ceil(max_samples.max(1)).max(1);

    let mut values: Vec<f32> = Vec::new();
    let mut best: Option<(f32, V3)> = None;
    for i in (0..n).step_by(step) {
        let t = &tris[i];
        let geo = cross(sub(t[1], t[0]), sub(t[2], t[0]));
        let area2 = len(geo);
        if area2 * 0.5 < MIN_AREA {
            continue;
        }
        // Normal keluar = normal geometri dari urutan (winding) segitiga.
        // Mesher `opencascade-rs` membalik urutan segitiga untuk face
        // `Reversed`, tetapi TIDAK membalik normal vertex hasil
        // `ComputeNormals` — pada dinding lubang normal vertex menunjuk ke
        // dalam material. Aturan awal rencana ("balik bila berlawanan dengan
        // rata-rata normal vertex") karenanya salah; dibuktikan tes plat
        // berlubang (sinar menyeberangi lubang, terukur 5,5 alih-alih 2,25).
        let normal = scale(geo, 1.0 / area2);
        let c = centroid(t);
        let origin = sub(c, scale(normal, START_OFFSET));
        let dir = scale(normal, -1.0);
        let Some(hit) = bvh.closest(origin, dir, i as u32) else {
            continue;
        };
        let thickness = hit + START_OFFSET;
        values.push(thickness);
        if best.is_none_or(|(m, _)| thickness < m) {
            best = Some((thickness, c));
        }
    }
    let (min, at) = best?;
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p05 = values[((values.len() - 1) as f32 * 0.05).round() as usize];
    Some(WallReport {
        min,
        at,
        p05,
        samples: values.len(),
    })
}
