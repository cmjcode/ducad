//! Voxelisasi mesh permukaan tertutup menjadi [`HexMesh`].
//!
//! Langkah:
//! 1. AABB body → grid sel hampir kubik. Jumlah sel per sumbu dibulatkan dan
//!    ukuran sel diregangkan sedikit agar grid pas dengan AABB, sehingga face
//!    datar sejajar sumbu jatuh tepat pada bidang node.
//! 2. Dalam/luar: parity ray cast sepanjang +X, dikerjakan per baris (y, z)
//!    ("scanline") pada `SUB³` titik sub-sampel tiap sel. Jitter kecil yang
//!    deterministik menghindari sinar tepat mengenai tepi segitiga. Fraksi
//!    sub-sampel di dalam body menjadi bobot elemen.
//! 3. Tiap sisi elemen yang terbuka (quad batas) diberi tag face B-rep dari
//!    segitiga permukaan terdekat, sehingga fixture/beban menempel pada face.
//! 4. Koreksi volume: bobot elemen permukaan diskalakan agar Σ volume voxel
//!    sama dengan volume eksak (massa dan gravitasi benar).

use crate::linalg::{add, cross, dot, norm, scale, sub, V3};
use crate::mesh::hex::{HexMesh, NO_INDEX};
use crate::setup::MeshSettings;
use crate::{CancelToken, SimError, SurfaceMesh};

/// Sub-sampel per sumbu per sel untuk fraksi isi.
const SUB: usize = 4;
/// Sel aktif bila setidaknya sekian sub-sampel (dari `SUB³` = 64) ada di dalam
/// body, yaitu fraksi isi ≥ 0.5. Ambang lebih rendah menambah sel nyaris
/// kosong di tangga permukaan lengkung dan membuat puncak tegangan liar.
const ACTIVE_MIN_COUNT: u8 = 32;
/// Batas jumlah sel grid (termasuk yang kosong).
const MAX_GRID_CELLS: usize = 4_000_000;
/// Rentang elemen aktif untuk ukuran sel otomatis.
const AUTO_MIN_ELEMS: usize = 20_000;
const AUTO_MAX_ELEMS: usize = 200_000;
/// Batas skala koreksi volume pada elemen permukaan.
const VOLUME_SCALE_RANGE: (f64, f64) = (0.2, 3.0);
/// Jitter sinar (fraksi ukuran sub-sel) untuk tiga percobaan.
const JITTERS: [[f64; 2]; 3] = [[1.37e-3, 2.91e-3], [-2.11e-3, 1.73e-3], [3.19e-3, -1.27e-3]];

/// Titik sampel permukaan: potongan kecil segitiga (≤ satu sel) dengan
/// vektor luas dan face pemiliknya. Dipakai untuk mengintegralkan beban.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SurfaceSamples {
    pub pos: Vec<V3>,
    /// Normal keluar × luas potongan, mm².
    pub area_vec: Vec<V3>,
    pub face: Vec<u32>,
    /// Indeks sampel terurut menurut `(face, indeks)`.
    by_face: Vec<u32>,
}

impl SurfaceSamples {
    /// Indeks sampel milik `face`.
    pub fn of_face(&self, face: u32) -> &[u32] {
        let start = self
            .by_face
            .partition_point(|&s| self.face[s as usize] < face);
        let end = self
            .by_face
            .partition_point(|&s| self.face[s as usize] <= face);
        &self.by_face[start..end]
    }
}

/// Mesh voxel + sampel permukaan + catatan pembuatannya.
#[derive(Debug, Clone, PartialEq)]
pub struct VoxelModel {
    pub mesh: HexMesh,
    pub samples: SurfaceSamples,
    /// Volume acuan (eksak dari B-rep, atau volume mesh permukaan), mm³.
    pub target_volume_mm3: f64,
    pub warnings: Vec<String>,
}

struct Grid {
    origin: V3,
    cell: V3,
    dims: [usize; 3],
}

impl Grid {
    fn new(min: V3, extent: V3, h: f64) -> Grid {
        let mut dims = [1usize; 3];
        let mut cell = [0.0; 3];
        for a in 0..3 {
            let n = (extent[a] / h).round();
            dims[a] = if n.is_finite() && n >= 1.0 {
                n.min(1.0e7) as usize
            } else {
                1
            };
            cell[a] = extent[a] / dims[a] as f64;
        }
        Grid {
            origin: min,
            cell,
            dims,
        }
    }

    fn cells(&self) -> usize {
        self.dims[0]
            .saturating_mul(self.dims[1])
            .saturating_mul(self.dims[2])
    }
}

fn invalid(msg: impl Into<String>) -> SimError {
    SimError::InvalidSetup(msg.into())
}

pub(crate) fn tri_points(surface: &SurfaceMesh, t: usize) -> [V3; 3] {
    // Indeks sudah divalidasi di `validate_surface`.
    let [a, b, c] = surface.triangles[t];
    [
        surface.positions[a as usize],
        surface.positions[b as usize],
        surface.positions[c as usize],
    ]
}

/// Memeriksa mesh permukaan dan mengembalikan (min, extent, volume bertanda).
pub(crate) fn validate_surface(surface: &SurfaceMesh) -> Result<(V3, V3, f64), SimError> {
    if surface.triangles.is_empty() || surface.positions.is_empty() {
        return Err(invalid("surface mesh is empty"));
    }
    if surface.tri_face.len() != surface.triangles.len() {
        return Err(invalid(
            "surface mesh tri_face length does not match triangles",
        ));
    }
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for p in &surface.positions {
        for a in 0..3 {
            if !p[a].is_finite() {
                return Err(invalid("surface mesh has a non-finite vertex"));
            }
            min[a] = min[a].min(p[a]);
            max[a] = max[a].max(p[a]);
        }
    }
    let n = surface.positions.len();
    for tri in &surface.triangles {
        if tri.iter().any(|&i| i as usize >= n) {
            return Err(invalid("surface mesh triangle index out of range"));
        }
    }
    let extent = sub(max, min);
    let largest = extent[0].max(extent[1]).max(extent[2]);
    if extent.iter().any(|&e| e <= largest * 1e-9 || e <= 0.0) {
        return Err(invalid("surface mesh has zero thickness along an axis"));
    }
    // Volume bertanda (teorema divergensi), relatif ke `min` agar stabil.
    let mut vol6 = 0.0;
    for t in 0..surface.triangles.len() {
        let [a, b, c] = tri_points(surface, t);
        vol6 += dot(sub(a, min), cross(sub(b, min), sub(c, min)));
    }
    let volume = vol6 / 6.0;
    if volume.is_nan() || volume <= 0.0 {
        return Err(invalid(
            "surface mesh is not a closed, outward-oriented solid (non-positive volume)",
        ));
    }
    Ok((min, extent, volume))
}

/// Satu lintasan klasifikasi: jumlah sub-sampel di dalam body per sel grid,
/// plus jumlah baris dengan paritas ganjil (indikasi sinar kena tepi/celah).
fn classify_once(surface: &SurfaceMesh, grid: &Grid, jitter: [f64; 2]) -> (Vec<u8>, usize) {
    let m = [grid.dims[0] * SUB, grid.dims[1] * SUB, grid.dims[2] * SUB];
    let d = [
        grid.cell[0] / SUB as f64,
        grid.cell[1] / SUB as f64,
        grid.cell[2] / SUB as f64,
    ];
    let oy = grid.origin[1] + jitter[0] * d[1];
    let oz = grid.origin[2] + jitter[1] * d[2];
    let mut hits: Vec<(u32, f64)> = Vec::new();
    for t in 0..surface.triangles.len() {
        let [a, b, c] = tri_points(surface, t);
        let ymin = a[1].min(b[1]).min(c[1]);
        let ymax = a[1].max(b[1]).max(c[1]);
        let zmin = a[2].min(b[2]).min(c[2]);
        let zmax = a[2].max(b[2]).max(c[2]);
        let j0 = ((ymin - oy) / d[1] - 0.5).ceil().max(0.0);
        let j1 = ((ymax - oy) / d[1] - 0.5).floor().min(m[1] as f64 - 1.0);
        let k0 = ((zmin - oz) / d[2] - 0.5).ceil().max(0.0);
        let k1 = ((zmax - oz) / d[2] - 0.5).floor().min(m[2] as f64 - 1.0);
        if j1 < j0 || k1 < k0 {
            continue;
        }
        let e1 = [b[1] - a[1], b[2] - a[2]];
        let e2 = [c[1] - a[1], c[2] - a[2]];
        let den = e1[0] * e2[1] - e2[0] * e1[1];
        let span = (ymax - ymin).max(zmax - zmin);
        if den.abs() <= 1e-14 * span * span {
            // Segitiga sejajar sinar: tidak menyumbang perpotongan.
            continue;
        }
        let inv = 1.0 / den;
        for k in (k0 as usize)..=(k1 as usize) {
            let pz = oz + (k as f64 + 0.5) * d[2] - a[2];
            for j in (j0 as usize)..=(j1 as usize) {
                let py = oy + (j as f64 + 0.5) * d[1] - a[1];
                let w1 = (py * e2[1] - e2[0] * pz) * inv;
                let w2 = (e1[0] * pz - py * e1[1]) * inv;
                let w0 = 1.0 - w1 - w2;
                if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                    let x = w0 * a[0] + w1 * b[0] + w2 * c[0];
                    hits.push(((j + m[1] * k) as u32, x));
                }
            }
        }
    }
    hits.sort_unstable_by(|p, q| p.0.cmp(&q.0).then(p.1.total_cmp(&q.1)));

    let mut counts = vec![0u8; grid.cells()];
    let mut bad_rows = 0usize;
    let mut s = 0usize;
    while s < hits.len() {
        let row = hits[s].0;
        let mut e = s;
        while e < hits.len() && hits[e].0 == row {
            e += 1;
        }
        if (e - s) % 2 == 1 {
            bad_rows += 1;
        }
        let j = row as usize % m[1];
        let k = row as usize / m[1];
        let base = grid.dims[0] * ((j / SUB) + grid.dims[1] * (k / SUB));
        let mut h = s;
        let mut inside = false;
        for i in 0..m[0] {
            let x = grid.origin[0] + (i as f64 + 0.5) * d[0];
            while h < e && hits[h].1 < x {
                inside = !inside;
                h += 1;
            }
            if inside {
                counts[base + i / SUB] += 1;
            }
        }
        s = e;
    }
    (counts, bad_rows)
}

/// Klasifikasi dengan hingga tiga jitter; mengambil percobaan dengan baris
/// bermasalah paling sedikit.
fn classify(surface: &SurfaceMesh, grid: &Grid) -> (Vec<u8>, usize) {
    let mut best: Option<(Vec<u8>, usize)> = None;
    for jitter in JITTERS {
        let (counts, bad) = classify_once(surface, grid, jitter);
        let better = best.as_ref().is_none_or(|b| bad < b.1);
        if better {
            best = Some((counts, bad));
        }
        if bad == 0 {
            break;
        }
    }
    best.unwrap_or_default()
}

fn active_count(counts: &[u8]) -> usize {
    counts.iter().filter(|&&c| c >= ACTIVE_MIN_COUNT).count()
}

fn check_cancel(cancel: &CancelToken) -> Result<(), SimError> {
    if cancel.is_cancelled() {
        Err(SimError::Cancelled)
    } else {
        Ok(())
    }
}

/// Memilih ukuran sel lalu mengklasifikasi grid akhirnya.
fn choose_grid(
    surface: &SurfaceMesh,
    min: V3,
    extent: V3,
    volume: f64,
    settings: &MeshSettings,
    cancel: &CancelToken,
    warnings: &mut Vec<String>,
) -> Result<(Grid, Vec<u8>, usize), SimError> {
    let largest = extent[0].max(extent[1]).max(extent[2]);
    if let Some(h) = settings.cell_mm {
        if !(h.is_finite() && h > 0.0) {
            return Err(invalid("mesh.cell_mm must be a positive number"));
        }
        let grid = Grid::new(min, extent, h);
        if grid.cells() > MAX_GRID_CELLS {
            return Err(invalid(format!(
                "mesh.cell_mm = {h} gives a grid of {} cells (limit {MAX_GRID_CELLS}); use a larger cell",
                grid.cells()
            )));
        }
        let (counts, bad) = classify(surface, &grid);
        return Ok((grid, counts, bad));
    }

    // Mode target: eksplisit (`target_elems`) atau otomatis 20k–200k.
    let (mut h, explicit_target) = match settings.target_elems {
        Some(0) => return Err(invalid("mesh.target_elems must be at least 1")),
        Some(t) => ((volume / t as f64).cbrt(), Some(t)),
        None => (largest / 40.0, None),
    };
    let mut last: Option<(Grid, Vec<u8>, usize)> = None;
    for _ in 0..6 {
        check_cancel(cancel)?;
        let grid = Grid::new(min, extent, h);
        if grid.cells() > MAX_GRID_CELLS {
            h *= (grid.cells() as f64 / MAX_GRID_CELLS as f64).cbrt() * 1.02;
            continue;
        }
        let (counts, bad) = classify(surface, &grid);
        let n = active_count(&counts);
        let goal = match explicit_target {
            Some(t) => {
                let ratio = n as f64 / t as f64;
                if (0.85..=1.15).contains(&ratio) {
                    None
                } else {
                    Some(t)
                }
            }
            None => {
                if n < AUTO_MIN_ELEMS {
                    Some(30_000)
                } else if n > AUTO_MAX_ELEMS {
                    Some(150_000)
                } else {
                    None
                }
            }
        };
        last = Some((grid, counts, bad));
        match goal {
            None => break,
            Some(_) if n == 0 => h *= 0.5,
            Some(t) => h *= (n as f64 / t as f64).cbrt(),
        }
    }
    let result = last.ok_or_else(|| invalid("could not choose a mesh cell size"))?;
    if explicit_target.is_none() {
        let n = active_count(&result.1);
        if !(AUTO_MIN_ELEMS..=AUTO_MAX_ELEMS).contains(&n) {
            warnings.push(format!(
                "automatic cell size ended at {n} elements, outside the 20000-200000 target range"
            ));
        }
    }
    Ok(result)
}

/// Titik terdekat pada segitiga (Ericson, *Real-Time Collision Detection*).
pub(crate) fn closest_on_triangle(p: V3, a: V3, b: V3, c: V3) -> V3 {
    let ab = sub(b, a);
    let ac = sub(c, a);
    let ap = sub(p, a);
    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = sub(p, b);
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return add(a, scale(ab, d1 / (d1 - d3)));
    }
    let cp = sub(p, c);
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return add(a, scale(ac, d2 / (d2 - d6)));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return add(b, scale(sub(c, b), w));
    }
    let denom = va + vb + vc;
    if denom.abs() <= f64::MIN_POSITIVE {
        return a;
    }
    let v = vb / denom;
    let w = vc / denom;
    add(a, add(scale(ab, v), scale(ac, w)))
}

/// Memotong setiap segitiga (bisection tepi terpanjang) sampai tepinya ≤ `limit`.
/// Mengembalikan sampel dan pasangan `(sel, segitiga)` untuk indeks spasial.
fn sample_surface(
    surface: &SurfaceMesh,
    mesh: &HexMesh,
    limit: f64,
) -> (SurfaceSamples, Vec<(u32, u32)>) {
    let mut samples = SurfaceSamples::default();
    let mut pairs: Vec<(u32, u32)> = Vec::new();
    let mut stack: Vec<[V3; 3]> = Vec::new();
    for t in 0..surface.triangles.len() {
        stack.clear();
        stack.push(tri_points(surface, t));
        while let Some([a, b, c]) = stack.pop() {
            let lab = norm(sub(b, a));
            let lbc = norm(sub(c, b));
            let lca = norm(sub(a, c));
            let longest = lab.max(lbc).max(lca);
            if longest > limit {
                // Orientasi anak sama dengan induknya.
                if lab >= lbc && lab >= lca {
                    let m = scale(add(a, b), 0.5);
                    stack.push([a, m, c]);
                    stack.push([m, b, c]);
                } else if lbc >= lca {
                    let m = scale(add(b, c), 0.5);
                    stack.push([a, b, m]);
                    stack.push([a, m, c]);
                } else {
                    let m = scale(add(c, a), 0.5);
                    stack.push([a, b, m]);
                    stack.push([m, b, c]);
                }
                continue;
            }
            let area_vec = scale(cross(sub(b, a), sub(c, a)), 0.5);
            if norm(area_vec) <= 0.0 {
                continue;
            }
            let centroid = scale(add(a, add(b, c)), 1.0 / 3.0);
            let cell = mesh.locate_cell(centroid);
            pairs.push((mesh.cell_index(cell[0], cell[1], cell[2]) as u32, t as u32));
            samples.pos.push(centroid);
            samples.area_vec.push(area_vec);
            samples.face.push(surface.tri_face[t]);
        }
    }
    let mut order: Vec<u32> = (0..samples.pos.len() as u32).collect();
    order.sort_by_key(|&s| (samples.face[s as usize], s));
    samples.by_face = order;
    pairs.sort_unstable();
    pairs.dedup();
    (samples, pairs)
}

/// Indeks spasial sel → segitiga (CSR) + normal satuan tiap segitiga.
struct TriIndex {
    ptr: Vec<u32>,
    tris: Vec<u32>,
    normals: Vec<V3>,
}

impl TriIndex {
    fn build(surface: &SurfaceMesh, cells: usize, pairs: &[(u32, u32)]) -> TriIndex {
        let mut ptr = vec![0u32; cells + 1];
        for &(c, _) in pairs {
            ptr[c as usize + 1] += 1;
        }
        for c in 0..cells {
            ptr[c + 1] += ptr[c];
        }
        let tris = pairs.iter().map(|p| p.1).collect();
        let normals = (0..surface.triangles.len())
            .map(|t| {
                let [a, b, c] = tri_points(surface, t);
                crate::linalg::normalize(cross(sub(b, a), sub(c, a))).unwrap_or([0.0; 3])
            })
            .collect();
        TriIndex { ptr, tris, normals }
    }

    fn cell_tris(&self, cell: usize) -> &[u32] {
        &self.tris[self.ptr[cell] as usize..self.ptr[cell + 1] as usize]
    }
}

/// Memberi tag face B-rep ke semua quad batas.
///
/// Skor kandidat = jarak titik tengah quad ke segitiga + penalti kecil bila
/// normal segitiga tidak searah normal quad; penalti itu memecah seri di
/// dekat tepi antara dua face.
fn tag_quads(mesh: &mut HexMesh, surface: &SurfaceMesh, index: &TriIndex) {
    let hmin = mesh.cell[0].min(mesh.cell[1]).min(mesh.cell[2]);
    let align_penalty = 0.125 * hmin;
    let score_tri = |t: u32, center: V3, outward: V3| -> (f64, f64) {
        let [a, b, c] = tri_points(surface, t as usize);
        let q = closest_on_triangle(center, a, b, c);
        let dist = norm(sub(center, q));
        let misalign = 1.0 - dot(index.normals[t as usize], outward);
        (dist + align_penalty * misalign, dist)
    };
    let mut seen: Vec<u32> = Vec::new();
    for e in 0..mesh.elems.len() {
        let ijk = mesh.cell_ijk(mesh.elem_cell[e] as usize);
        for dir in 0..6 {
            let axis = dir / 2;
            let exposed = if dir % 2 == 0 {
                ijk[axis] == 0 || {
                    let mut n = ijk;
                    n[axis] -= 1;
                    mesh.elem_of_cell[mesh.cell_index(n[0], n[1], n[2])] == NO_INDEX
                }
            } else {
                ijk[axis] + 1 == mesh.dims[axis] || {
                    let mut n = ijk;
                    n[axis] += 1;
                    mesh.elem_of_cell[mesh.cell_index(n[0], n[1], n[2])] == NO_INDEX
                }
            };
            if !exposed {
                continue;
            }
            let center = mesh.quad_center(e, dir);
            let outward = HexMesh::quad_normal(dir);
            // (skor, jarak, segitiga)
            let mut best: Option<(f64, f64, u32)> = None;
            for radius in [1usize, 2, 4] {
                seen.clear();
                for k in ijk[2].saturating_sub(radius)..=(ijk[2] + radius).min(mesh.dims[2] - 1) {
                    for j in ijk[1].saturating_sub(radius)..=(ijk[1] + radius).min(mesh.dims[1] - 1)
                    {
                        for i in
                            ijk[0].saturating_sub(radius)..=(ijk[0] + radius).min(mesh.dims[0] - 1)
                        {
                            seen.extend_from_slice(index.cell_tris(mesh.cell_index(i, j, k)));
                        }
                    }
                }
                seen.sort_unstable();
                seen.dedup();
                best = None;
                for &t in &seen {
                    let (score, dist) = score_tri(t, center, outward);
                    if best.is_none_or(|b| score < b.0) {
                        best = Some((score, dist, t));
                    }
                }
                if let Some(b) = best {
                    if radius > 1 || b.1 <= 0.35 * hmin {
                        break;
                    }
                }
            }
            if best.is_none() {
                for t in 0..surface.triangles.len() as u32 {
                    let (score, dist) = score_tri(t, center, outward);
                    if best.is_none_or(|b| score < b.0) {
                        best = Some((score, dist, t));
                    }
                }
            }
            if let Some((_, _, t)) = best {
                mesh.quad_tag[e][dir] = surface.tri_face[t as usize];
                mesh.face_quads
                    .push((surface.tri_face[t as usize], e as u32, dir as u8));
            }
        }
    }
    mesh.face_quads.sort_unstable();
}

/// Membangun elemen dan node dari hitungan sub-sampel.
fn build_mesh(grid: Grid, counts: &[u8]) -> HexMesh {
    let [nx, ny, nz] = grid.dims;
    let node_index = |i: usize, j: usize, k: usize| i + (nx + 1) * (j + (ny + 1) * k);
    let mut node_id = vec![NO_INDEX; (nx + 1) * (ny + 1) * (nz + 1)];
    let mut elem_of_cell = vec![NO_INDEX; counts.len()];
    let mut elem_cell = Vec::new();
    for (c, &count) in counts.iter().enumerate() {
        if count >= ACTIVE_MIN_COUNT {
            elem_of_cell[c] = elem_cell.len() as u32;
            elem_cell.push(c as u32);
            let (i, j, k) = (c % nx, (c / nx) % ny, c / (nx * ny));
            for n in 0..8 {
                node_id[node_index(i + (n & 1), j + ((n >> 1) & 1), k + (n >> 2))] = 0;
            }
        }
    }
    // Node dinomori menurut urutan grid agar pola CSR rapat dan deterministik.
    let mut nodes = Vec::new();
    for k in 0..=nz {
        for j in 0..=ny {
            for i in 0..=nx {
                let slot = &mut node_id[node_index(i, j, k)];
                if *slot != NO_INDEX {
                    *slot = nodes.len() as u32;
                    nodes.push([
                        grid.origin[0] + i as f64 * grid.cell[0],
                        grid.origin[1] + j as f64 * grid.cell[1],
                        grid.origin[2] + k as f64 * grid.cell[2],
                    ]);
                }
            }
        }
    }
    let full = (SUB * SUB * SUB) as f64;
    let mut elems = Vec::with_capacity(elem_cell.len());
    let mut weight = Vec::with_capacity(elem_cell.len());
    for &c in &elem_cell {
        let c = c as usize;
        let (i, j, k) = (c % nx, (c / nx) % ny, c / (nx * ny));
        let mut conn = [0u32; 8];
        for n in 0..8 {
            conn[n] = node_id[node_index(i + (n & 1), j + ((n >> 1) & 1), k + (n >> 2))];
        }
        elems.push(conn);
        weight.push(f64::from(counts[c]) / full);
    }
    let quad_tag = vec![[NO_INDEX; 6]; elems.len()];
    HexMesh {
        origin: grid.origin,
        cell: grid.cell,
        dims: grid.dims,
        nodes,
        elems,
        elem_cell,
        weight,
        elem_of_cell,
        quad_tag,
        face_quads: Vec::new(),
    }
}

/// Menskalakan bobot elemen permukaan agar Σ volume voxel = `target`.
fn correct_volume(mesh: &mut HexMesh, target: f64, warnings: &mut Vec<String>) {
    let mut interior = 0.0;
    let mut surface_sum = 0.0;
    let is_surface: Vec<bool> = mesh
        .quad_tag
        .iter()
        .map(|t| t.iter().any(|&f| f != NO_INDEX))
        .collect();
    for e in 0..mesh.elems.len() {
        if is_surface[e] {
            surface_sum += mesh.weight[e];
        } else {
            interior += mesh.weight[e];
        }
    }
    if surface_sum <= 0.0 {
        return;
    }
    let wanted = (target / mesh.cell_volume() - interior) / surface_sum;
    let factor = wanted.clamp(VOLUME_SCALE_RANGE.0, VOLUME_SCALE_RANGE.1);
    if !wanted.is_finite() {
        return;
    }
    if factor != wanted {
        warnings.push(format!(
            "voxel volume differs from the exact volume by more than the correction range \
             (wanted surface scale {wanted:.3}); mass and stiffness are approximate, use a finer mesh"
        ));
    }
    for e in 0..mesh.elems.len() {
        if is_surface[e] {
            mesh.weight[e] *= factor;
        }
    }
}

/// Voxelisasi mesh permukaan. `exact_volume_mm3` (bila ada) dipakai untuk
/// koreksi volume; kalau tidak, volume mesh permukaan sendiri yang dipakai.
pub fn voxelize(
    surface: &SurfaceMesh,
    settings: &MeshSettings,
    exact_volume_mm3: Option<f64>,
    cancel: &CancelToken,
) -> Result<VoxelModel, SimError> {
    let (min, extent, surface_volume) = validate_surface(surface)?;
    let target_volume = match exact_volume_mm3 {
        Some(v) if v.is_finite() && v > 0.0 => v,
        Some(_) => return Err(invalid("exact volume must be a positive number")),
        None => surface_volume,
    };
    let mut warnings = Vec::new();
    let (grid, counts, bad_rows) = choose_grid(
        surface,
        min,
        extent,
        surface_volume,
        settings,
        cancel,
        &mut warnings,
    )?;
    check_cancel(cancel)?;
    if bad_rows > 0 {
        warnings.push(format!(
            "{bad_rows} ray rows hit the surface an odd number of times; the surface mesh may not be watertight"
        ));
    }
    let cell_mm = (grid.cell[0] * grid.cell[1] * grid.cell[2]).cbrt();
    let elements = active_count(&counts);
    if elements == 0 {
        return Err(SimError::MeshTooCoarse {
            cell_mm,
            elements,
            detail: "no cell is at least half inside the body".into(),
        });
    }
    let mut mesh = build_mesh(grid, &counts);
    let hmin = mesh.cell[0].min(mesh.cell[1]).min(mesh.cell[2]);
    // Batasi jumlah sampel untuk permukaan yang sangat luas terhadap sel.
    let area: f64 = (0..surface.triangles.len())
        .map(|t| {
            let [a, b, c] = tri_points(surface, t);
            0.5 * norm(cross(sub(b, a), sub(c, a)))
        })
        .sum();
    let limit = hmin.max((area / 1.0e6).sqrt());
    let (samples, pairs) = sample_surface(surface, &mesh, limit);
    check_cancel(cancel)?;
    let index = TriIndex::build(surface, mesh.elem_of_cell.len(), &pairs);
    tag_quads(&mut mesh, surface, &index);
    correct_volume(&mut mesh, target_volume, &mut warnings);
    Ok(VoxelModel {
        mesh,
        samples,
        target_volume_mm3: target_volume,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::benchmark::{box_surface, plate_with_hole, FACE_HOLE};

    fn settings(cell: f64) -> MeshSettings {
        MeshSettings {
            cell_mm: Some(cell),
            ..MeshSettings::default()
        }
    }

    #[test]
    fn mesh_voxel_box_is_exact() {
        let surface = box_surface([10.0, 10.0, 100.0]);
        let model = voxelize(
            &surface,
            &settings(2.5),
            Some(10_000.0),
            &CancelToken::new(),
        )
        .unwrap();
        let mesh = &model.mesh;
        assert_eq!(mesh.dims, [4, 4, 40]);
        assert_eq!(mesh.elems.len(), 640);
        assert_eq!(mesh.nodes.len(), 5 * 5 * 41);
        assert!(mesh.weight.iter().all(|&w| (w - 1.0).abs() < 1e-12));
        assert!((mesh.volume() - 10_000.0).abs() < 1e-6);
        assert!(model.warnings.is_empty(), "{:?}", model.warnings);
        // Setiap face kotak mendapat tepat quad-quad sisinya.
        assert_eq!(mesh.tagged_faces(), vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(mesh.quads_of_face(0).len(), 4 * 40);
        assert_eq!(mesh.quads_of_face(5).len(), 16);
        assert_eq!(mesh.face_nodes(5).len(), 25);
        for &n in &mesh.face_nodes(4) {
            assert!(mesh.nodes[n as usize][2].abs() < 1e-12);
        }
    }

    #[test]
    fn mesh_voxel_volume_and_face_tags() {
        let size = [60.0, 40.0, 6.0];
        let dia = 16.0;
        let surface = plate_with_hole(size, dia, 96);
        let exact = (size[0] * size[1] - std::f64::consts::PI * dia * dia / 4.0) * size[2];
        for cell in [2.0, 1.3, 0.77] {
            let model =
                voxelize(&surface, &settings(cell), Some(exact), &CancelToken::new()).unwrap();
            let rel = (model.mesh.volume() - exact).abs() / exact;
            assert!(rel < 1e-3, "cell {cell}: selisih volume relatif {rel}");
            assert!(model.warnings.is_empty(), "{:?}", model.warnings);
            // Setiap face B-rep memetakan ≥ 1 node.
            for face in 0..=FACE_HOLE {
                assert!(
                    !model.mesh.face_nodes(face).is_empty(),
                    "cell {cell}: face {face} tanpa node"
                );
                assert!(!model.samples.of_face(face).is_empty());
            }
            // Node bertag lubang berada di sekitar permukaan silinder.
            let c = [size[0] / 2.0, size[1] / 2.0];
            for &n in &model.mesh.face_nodes(FACE_HOLE) {
                let p = model.mesh.nodes[n as usize];
                let r = ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2)).sqrt();
                assert!((r - dia / 2.0).abs() < 2.0 * cell, "r = {r}");
            }
        }
        // Tanpa volume eksak: volume mesh permukaan menjadi acuan.
        let model = voxelize(&surface, &settings(2.0), None, &CancelToken::new()).unwrap();
        assert!((model.mesh.volume() - model.target_volume_mm3).abs() / exact < 1e-9);
        assert!((model.target_volume_mm3 - exact).abs() / exact < 2e-3);
    }

    #[test]
    fn mesh_voxel_timing_and_auto_range() {
        let surface = plate_with_hole([120.0, 80.0, 20.0], 30.0, 128);
        let start = std::time::Instant::now();
        let model = voxelize(
            &surface,
            &MeshSettings::default(),
            None,
            &CancelToken::new(),
        )
        .unwrap();
        let elapsed = start.elapsed().as_secs_f64();
        let n = model.mesh.elems.len();
        assert!((20_000..=200_000).contains(&n), "{n} elemen");
        let limit = if cfg!(debug_assertions) { 10.0 } else { 2.0 };
        assert!(elapsed < limit, "voxelisasi {n} sel butuh {elapsed:.2} s");
        // Target eksplisit dihormati walau di bawah 20k.
        let small = MeshSettings {
            target_elems: Some(3_000),
            ..MeshSettings::default()
        };
        let model = voxelize(&surface, &small, None, &CancelToken::new()).unwrap();
        let n = model.mesh.elems.len() as f64;
        assert!((n / 3000.0 - 1.0).abs() < 0.3, "{n} elemen");
    }

    #[test]
    fn mesh_voxel_rejects_bad_surfaces() {
        let cancel = CancelToken::new();
        let empty = SurfaceMesh::default();
        assert!(matches!(
            voxelize(&empty, &MeshSettings::default(), None, &cancel),
            Err(SimError::InvalidSetup(_))
        ));
        let mut bad_index = box_surface([1.0, 1.0, 1.0]);
        bad_index.triangles[0][0] = 99;
        assert!(matches!(
            voxelize(&bad_index, &MeshSettings::default(), None, &cancel),
            Err(SimError::InvalidSetup(_))
        ));
        let mut inside_out = box_surface([1.0, 1.0, 1.0]);
        for t in &mut inside_out.triangles {
            t.swap(1, 2);
        }
        assert!(matches!(
            voxelize(&inside_out, &MeshSettings::default(), None, &cancel),
            Err(SimError::InvalidSetup(_))
        ));
        let good = box_surface([1.0, 1.0, 1.0]);
        assert!(matches!(
            voxelize(&good, &settings(-1.0), None, &cancel),
            Err(SimError::InvalidSetup(_))
        ));
        cancel.cancel();
        assert_eq!(
            voxelize(&good, &MeshSettings::default(), None, &cancel).unwrap_err(),
            SimError::Cancelled
        );
    }
}
