//! Titik-titik untuk mesher tet: deteksi tepi fitur pada mesh permukaan,
//! ukuran lokal per face, lalu pengambilan sampel
//! (sudut → rantai tepi fitur → interior face → kisi BCC interior).
//!
//! Permukaan "di-remesh" dengan cara ini: titik baru ditaruh berjarak ≈ `h`
//! pada tepi fitur dan face, dan triangulasi Delaunay terbatas (tet yang
//! pusatnya di dalam body) membentuk segitiga permukaan baru yang mendekati
//! sama sisi. Tepi fitur terjaga karena titik-titiknya rapat di sepanjang tepi.

use std::collections::BTreeMap;

use crate::linalg::{add, cross, dot, norm, normalize, scale, sub, V3};
use crate::mesh::delaunay::TetError;
use crate::mesh::spatial::{PointHash, SurfaceIndex};
use crate::mesh::voxel::tri_points;
use crate::{CancelToken, SurfaceMesh};

/// Gradasi ukuran: ukuran boleh tumbuh sekian per satuan jarak dari sumber halus.
const GRADE: f64 = 0.5;
/// Ukuran pada face lengkung: fraksi jari-jari kelengkungan (~18 segmen per lingkaran).
const CURVATURE_FACTOR: f64 = 0.35;
/// Ukuran pada face yang dibebani/dikunci relatif terhadap ukuran dasar.
const LOADED_FACTOR: f64 = 0.7;
/// Ukuran lokal paling kecil relatif terhadap ukuran dasar.
const MIN_SIZE_FACTOR: f64 = 0.25;
/// Sudut belok (rad) yang menjadikan verteks tepi fitur sebuah sudut.
const CORNER_ANGLE: f64 = 0.61;
/// Tetapan kisi BCC relatif terhadap ukuran lokal.
const BCC_FACTOR: f64 = 1.12;
/// Batas jumlah titik.
const MAX_POINTS: usize = 600_000;

/// Tepi fitur dan verteks yang sudah dilas.
pub(crate) struct Features {
    /// Posisi verteks las.
    pub wpos: Vec<V3>,
    /// Semua tepi batas tag (pasangan verteks las, unik).
    pub segments: Vec<[u32; 2]>,
    /// Rantai tepi fitur: (verteks las, tertutup, ukuran).
    pub chains: Vec<(Vec<u32>, bool, f64)>,
    pub corners: Vec<u32>,
    /// Ukuran target per tag face.
    pub sizes: BTreeMap<u32, f64>,
}

fn weld(surface: &SurfaceMesh, tol: f64) -> (Vec<u32>, Vec<V3>) {
    let mut keyed: Vec<([i64; 3], u32)> = surface
        .positions
        .iter()
        .enumerate()
        .map(|(i, p)| {
            (
                [
                    (p[0] / tol).round() as i64,
                    (p[1] / tol).round() as i64,
                    (p[2] / tol).round() as i64,
                ],
                i as u32,
            )
        })
        .collect();
    keyed.sort_unstable();
    let mut welded = vec![0u32; surface.positions.len()];
    let mut wpos = Vec::new();
    let mut last: Option<[i64; 3]> = None;
    for (key, i) in keyed {
        if last != Some(key) {
            wpos.push(surface.positions[i as usize]);
            last = Some(key);
        }
        welded[i as usize] = (wpos.len() - 1) as u32;
    }
    (welded, wpos)
}

/// Mendeteksi tepi fitur, sudut, rantai, dan ukuran per face.
pub(crate) fn extract_features(
    surface: &SurfaceMesh,
    diag: f64,
    base: f64,
    fine_faces: &[u32],
) -> Features {
    let (welded, wpos) = weld(surface, 1e-7 * diag);
    // Rekaman tepi (tag, a, b, segitiga).
    let mut edges: Vec<(u32, u32, u32, u32)> = Vec::with_capacity(3 * surface.triangles.len());
    for (t, tri) in surface.triangles.iter().enumerate() {
        let w = [
            welded[tri[0] as usize],
            welded[tri[1] as usize],
            welded[tri[2] as usize],
        ];
        if w[0] == w[1] || w[1] == w[2] || w[0] == w[2] {
            continue;
        }
        for k in 0..3 {
            let (a, b) = (w[k], w[(k + 1) % 3]);
            edges.push((surface.tri_face[t], a.min(b), a.max(b), t as u32));
        }
    }
    edges.sort_unstable();
    let normal = |t: u32| {
        let [a, b, c] = tri_points(surface, t as usize);
        normalize(cross(sub(b, a), sub(c, a)))
    };
    let centroid = |t: u32| {
        let [a, b, c] = tri_points(surface, t as usize);
        scale(add(a, add(b, c)), 1.0 / 3.0)
    };
    let mut boundary: Vec<(u32, u32, u32)> = Vec::new();
    let mut curvature: BTreeMap<u32, Vec<f64>> = BTreeMap::new();
    let mut s = 0;
    while s < edges.len() {
        let mut e = s;
        while e < edges.len()
            && (edges[e].0, edges[e].1, edges[e].2) == (edges[s].0, edges[s].1, edges[s].2)
        {
            e += 1;
        }
        let (tag, a, b, t1) = edges[s];
        if e - s == 2 {
            let t2 = edges[s + 1].3;
            if let (Some(n1), Some(n2)) = (normal(t1), normal(t2)) {
                let theta = dot(n1, n2).clamp(-1.0, 1.0).acos();
                if theta > 0.02 {
                    if let Some(dir) = normalize(sub(wpos[b as usize], wpos[a as usize])) {
                        let d = sub(centroid(t2), centroid(t1));
                        let perp = norm(sub(d, scale(dir, dot(d, dir))));
                        if perp > 1e-9 * diag {
                            curvature.entry(tag).or_default().push(theta / perp);
                        }
                    }
                }
            }
        } else {
            boundary.push((tag, a, b));
        }
        s = e;
    }
    // Ukuran per tag.
    let mut sizes: BTreeMap<u32, f64> = BTreeMap::new();
    for &tag in &surface.tri_face {
        sizes.entry(tag).or_insert(base);
    }
    for (tag, mut list) in curvature {
        list.sort_by(f64::total_cmp);
        let kappa = list[list.len() / 2];
        if kappa > 0.0 {
            let slot = sizes.entry(tag).or_insert(base);
            *slot = slot.min(CURVATURE_FACTOR / kappa);
        }
    }
    for tag in fine_faces {
        if let Some(slot) = sizes.get_mut(tag) {
            *slot = slot.min(LOADED_FACTOR * base);
        }
    }
    for v in sizes.values_mut() {
        *v = v.clamp(MIN_SIZE_FACTOR * base, base);
    }

    // Setengah-tepi batas per tag.
    let mut half: Vec<(u32, u32, u32, u32)> = Vec::with_capacity(2 * boundary.len());
    for (id, &(tag, a, b)) in boundary.iter().enumerate() {
        half.push((tag, a, b, id as u32));
        half.push((tag, b, a, id as u32));
    }
    half.sort_unstable();
    let range = |tag: u32, v: u32| {
        let lo = half.partition_point(|h| (h.0, h.1) < (tag, v));
        let hi = half.partition_point(|h| (h.0, h.1) <= (tag, v));
        lo..hi
    };
    let mut corners: Vec<u32> = Vec::new();
    let mut i = 0;
    while i < half.len() {
        let (tag, v) = (half[i].0, half[i].1);
        let r = range(tag, v);
        if r.len() != 2 {
            corners.push(v);
        } else {
            let p = wpos[v as usize];
            let d1 = sub(p, wpos[half[r.start].2 as usize]);
            let d2 = sub(wpos[half[r.start + 1].2 as usize], p);
            let turn = match (normalize(d1), normalize(d2)) {
                (Some(u), Some(w)) => dot(u, w).clamp(-1.0, 1.0).acos(),
                _ => 0.0,
            };
            if turn > CORNER_ANGLE {
                corners.push(v);
            }
        }
        i = r.end;
    }
    corners.sort_unstable();
    corners.dedup();
    let is_corner = |v: u32| corners.binary_search(&v).is_ok();

    // Rantai: mulai dari sudut dulu, lalu lingkar tanpa sudut.
    let mut visited = vec![false; boundary.len()];
    let mut found: BTreeMap<Vec<u32>, (bool, f64)> = BTreeMap::new();
    for pass in 0..2 {
        for start in 0..half.len() {
            let (tag, from, to, id) = half[start];
            if visited[id as usize] || (pass == 0 && !is_corner(from)) {
                continue;
            }
            visited[id as usize] = true;
            let mut chain = vec![from, to];
            let mut cur = to;
            let mut closed = false;
            loop {
                if cur == from {
                    closed = true;
                    break;
                }
                if is_corner(cur) {
                    break;
                }
                let next = half[range(tag, cur)]
                    .iter()
                    .find(|h| !visited[h.3 as usize])
                    .copied();
                match next {
                    Some(h) => {
                        visited[h.3 as usize] = true;
                        chain.push(h.2);
                        cur = h.2;
                    }
                    None => break,
                }
            }
            let size = sizes.get(&tag).copied().unwrap_or(base);
            // Bentuk kanonik agar rantai yang sama dari dua face menyatu.
            let key = if closed {
                chain.pop();
                let n = chain.len();
                let at = (0..n).min_by_key(|&k| chain[k]).unwrap_or(0);
                let forward: Vec<u32> = (0..n).map(|k| chain[(at + k) % n]).collect();
                let backward: Vec<u32> = (0..n).map(|k| chain[(at + n - k) % n]).collect();
                forward.min(backward)
            } else {
                let reversed: Vec<u32> = chain.iter().rev().copied().collect();
                chain.min(reversed)
            };
            let slot = found.entry(key).or_insert((closed, size));
            slot.1 = slot.1.min(size);
        }
    }
    let mut chains: Vec<(Vec<u32>, bool, f64)> =
        found.into_iter().map(|(k, (c, s))| (k, c, s)).collect();
    chains.sort_by(|a, b| a.2.total_cmp(&b.2).then_with(|| a.0.cmp(&b.0)));
    let mut segments: Vec<[u32; 2]> = boundary.iter().map(|&(_, a, b)| [a, b]).collect();
    segments.sort_unstable();
    segments.dedup();
    Features {
        wpos,
        segments,
        chains,
        corners,
        sizes,
    }
}

/// Medan ukuran: ukuran dasar, diperkecil di dekat titik sumber yang halus.
pub(crate) struct SizeField {
    pub base: f64,
    radius: f64,
    hash: Option<PointHash>,
    sizes: Vec<f64>,
}

impl SizeField {
    fn new(min: V3, max: V3, base: f64, smallest: f64) -> SizeField {
        let radius = (base - smallest) / GRADE;
        let hash =
            (smallest < 0.999 * base).then(|| PointHash::new(min, max, radius.max(0.5 * base)));
        SizeField {
            base,
            radius,
            hash,
            sizes: Vec::new(),
        }
    }

    fn add_source(&mut self, p: V3, size: f64) {
        if size < 0.999 * self.base {
            if let Some(hash) = &mut self.hash {
                hash.insert(p);
                self.sizes.push(size);
            }
        }
    }

    pub fn at(&self, p: V3) -> f64 {
        let mut h = self.base;
        if let Some(hash) = &self.hash {
            hash.for_each_within(p, self.radius, |i, d| {
                h = h.min(self.sizes[i as usize] + GRADE * d);
            });
        }
        h
    }
}

/// Awan titik hasil pengambilan sampel.
pub(crate) struct PointCloud {
    pub pts: Vec<V3>,
    /// Titik `0..n_surface` berada di permukaan; sisanya interior.
    pub n_surface: usize,
    /// Tag face tiap titik permukaan; [`NO_TAG`] untuk titik tepi fitur.
    pub tags: Vec<u32>,
}

fn check_cancel(cancel: &CancelToken) -> Result<(), TetError> {
    if cancel.is_cancelled() {
        Err(TetError::Cancelled)
    } else {
        Ok(())
    }
}

pub(crate) const NO_TAG: u32 = u32::MAX;
/// Jumlah iterasi relaksasi titik permukaan.
const RELAX_ITERATIONS: usize = 30;

/// Relaksasi tolak-menolak titik interior face (titik fitur diam): tiap
/// titik didorong menjauhi tetangga yang lebih dekat dari ~1.3 ukuran lokal
/// lalu diproyeksikan kembali ke face-nya. Hasilnya sebaran mendekati
/// heksagonal, sehingga segitiga permukaan mendekati sama sisi.
fn relax_surface(
    index: &SurfaceIndex,
    start: &[V3],
    meta: &[(u32, f64)],
    min: V3,
    max: V3,
    smallest: f64,
    cancel: &CancelToken,
) -> Result<Vec<V3>, TetError> {
    let mut pts = start.to_vec();
    for _ in 0..RELAX_ITERATIONS {
        check_cancel(cancel)?;
        let mut hash = PointHash::new(min, max, 1.3 * smallest);
        for &p in &pts {
            hash.insert(p);
        }
        let mut next = pts.clone();
        for (i, &(tag, size)) in meta.iter().enumerate() {
            if tag == NO_TAG {
                continue;
            }
            let p = pts[i];
            let reach = 1.3 * size;
            let mut push = [0.0; 3];
            hash.for_each_within(p, reach, |j, d| {
                if j as usize != i && d > 0.0 {
                    let w = (1.0 - d / reach) / d;
                    push = add(push, scale(sub(p, pts[j as usize]), w));
                }
            });
            let magnitude = norm(push);
            if magnitude <= 0.0 {
                continue;
            }
            let step = (0.12 * size * magnitude).min(0.08 * size);
            let moved = add(p, scale(push, step / magnitude));
            if let Some(c) = index.closest(moved, Some(tag), None) {
                // Tekanan dari interior tidak boleh mendorong titik mendekati
                // titik fitur (yang diam) lebih dari jarak aman.
                let mut crowding = false;
                hash.for_each_within(c.point, 0.85 * size, |j, d| {
                    if meta[j as usize].0 == NO_TAG && d < norm(sub(p, pts[j as usize])) {
                        crowding = true;
                    }
                });
                if !crowding {
                    next[i] = c.point;
                }
            }
        }
        pts = next;
    }
    Ok(pts)
}

/// Membangkitkan titik permukaan dan interior.
pub(crate) fn generate_points(
    surface: &SurfaceMesh,
    index: &SurfaceIndex,
    features: &Features,
    min: V3,
    max: V3,
    base: f64,
    cancel: &CancelToken,
) -> Result<PointCloud, TetError> {
    let smallest = features.sizes.values().copied().fold(base, f64::min);
    let mut field = SizeField::new(min, max, base, smallest);
    let mut hash = PointHash::new(min, max, smallest);
    // Per titik permukaan: (tag face atau NO_TAG untuk titik fitur, ukuran).
    let mut meta: Vec<(u32, f64)> = Vec::new();
    let too_many = || TetError::failed("the requested mesh size needs too many points");

    // 1. Sudut.
    for &c in &features.corners {
        hash.insert(features.wpos[c as usize]);
        meta.push((NO_TAG, base));
    }
    // 2. Rantai tepi fitur (yang halus lebih dulu).
    let corner_count = hash.pts.len();
    let mut chain_sizes: Vec<f64> = Vec::new();
    for (verts, closed, size) in &features.chains {
        let mut line: Vec<V3> = verts.iter().map(|&v| features.wpos[v as usize]).collect();
        if *closed {
            line.push(line[0]);
        }
        let mut cum = vec![0.0];
        for k in 1..line.len() {
            cum.push(cum[k - 1] + norm(sub(line[k], line[k - 1])));
        }
        let length = cum[cum.len() - 1];
        if length <= 0.0 {
            continue;
        }
        let n = ((length / size).round() as usize).max(if *closed { 3 } else { 1 });
        let mut seg = 1;
        let first = if *closed { 0 } else { 1 };
        for i in first..n {
            let target = length * i as f64 / n as f64;
            while seg + 1 < cum.len() && cum[seg] < target {
                seg += 1;
            }
            let span = cum[seg] - cum[seg - 1];
            let t = if span > 0.0 {
                (target - cum[seg - 1]) / span
            } else {
                0.0
            };
            let p = add(line[seg - 1], scale(sub(line[seg], line[seg - 1]), t));
            if !hash.any_within(p, 0.45 * size) {
                hash.insert(p);
                chain_sizes.push(*size);
                meta.push((NO_TAG, *size));
            }
        }
        if hash.pts.len() > MAX_POINTS {
            return Err(too_many());
        }
    }
    for (k, &size) in chain_sizes.iter().enumerate() {
        field.add_source(hash.pts[corner_count + k], size);
    }
    // Sudut mewarisi ukuran titik rantai terdekat (lewat medan ukuran).
    check_cancel(cancel)?;

    // 3. Interior face: kandidat kisi barisentrik per segitiga, seleksi serakah.
    let mut by_tag: Vec<(u32, u32)> = surface
        .tri_face
        .iter()
        .enumerate()
        .map(|(t, &f)| (f, t as u32))
        .collect();
    by_tag.sort_by(|a, b| {
        let sa = features.sizes.get(&a.0).copied().unwrap_or(base);
        let sb = features.sizes.get(&b.0).copied().unwrap_or(base);
        sa.total_cmp(&sb).then(a.cmp(b))
    });
    for (count, &(tag, t)) in by_tag.iter().enumerate() {
        if count % 256 == 0 {
            check_cancel(cancel)?;
        }
        let tag_size = features.sizes.get(&tag).copied().unwrap_or(base);
        let [a, b, c] = tri_points(surface, t as usize);
        let longest = norm(sub(b, a)).max(norm(sub(c, b))).max(norm(sub(a, c)));
        let n = ((longest / (0.3 * tag_size)).ceil() as usize).clamp(1, 3000);
        for i in 0..=n {
            for j in 0..=(n - i) {
                let (u, v) = (i as f64 / n as f64, j as f64 / n as f64);
                let p = add(scale(a, 1.0 - u - v), add(scale(b, u), scale(c, v)));
                let size = tag_size.min(field.at(p));
                if !hash.any_within(p, 0.85 * size) {
                    hash.insert(p);
                    field.add_source(p, size);
                    meta.push((tag, size));
                }
            }
        }
        if hash.pts.len() > MAX_POINTS {
            return Err(too_many());
        }
    }
    let n_surface = hash.pts.len();
    check_cancel(cancel)?;
    let relaxed = relax_surface(index, &hash.pts, &meta, min, max, smallest, cancel)?;
    let mut hash = PointHash::new(min, max, smallest);
    for p in relaxed {
        hash.insert(p);
    }

    // 4. Interior: kisi BCC bertingkat (tiap tingkat setengah jarak kisi).
    let levels = if smallest < 0.999 * base {
        ((base / smallest).log2().round() as usize).min(2)
    } else {
        0
    };
    let a0 = BCC_FACTOR * base;
    for level in 0..=levels {
        let step = a0 / (1u32 << level) as f64;
        for sub_lattice in 0..2usize {
            let off = 0.5 * step * sub_lattice as f64;
            let count =
                |a: usize| (((max[a] - min[a] - off) / step).floor() as i64 + 1).max(0) as usize;
            let (nx, ny, nz) = (count(0), count(1), count(2));
            if (nx as f64) * (ny as f64) * (nz as f64) > 2.0e8 {
                return Err(too_many());
            }
            for k in 0..nz {
                check_cancel(cancel)?;
                for j in 0..ny {
                    for i in 0..nx {
                        // Titik ini sudah ditawarkan di tingkat sebelumnya?
                        if level > 0 && sub_lattice == 0 && i % 2 == j % 2 && j % 2 == k % 2 {
                            continue;
                        }
                        let p = [
                            min[0] + off + step * i as f64,
                            min[1] + off + step * j as f64,
                            min[2] + off + step * k as f64,
                        ];
                        let h = field.at(p);
                        let wanted = ((base / h).log2().round().max(0.0) as usize).min(levels);
                        if wanted < level {
                            continue;
                        }
                        if hash.any_within(p, 0.7 * h) || !index.inside(p) {
                            continue;
                        }
                        if index
                            .closest(p, None, None)
                            .is_none_or(|c| c.dist < 0.55 * h)
                        {
                            continue;
                        }
                        hash.insert(p);
                    }
                }
            }
            if hash.pts.len() > MAX_POINTS {
                return Err(too_many());
            }
        }
    }
    Ok(PointCloud {
        pts: hash.pts,
        n_surface,
        tags: meta.iter().map(|m| m.0).collect(),
    })
}
