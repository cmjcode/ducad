//! Perbaikan kualitas mesh tet linier: kupas sliver pipih di permukaan,
//! flip topologi (2-3, 3-2, 4-4), dan penghalusan verteks interior.
//!
//! Ukuran kualitas = rasio radius `3·r_in / R_circ` (1 untuk tet beraturan,
//! → 0 untuk sliver). Tiap lintasan membangun ulang ketetanggaan dari nol dan
//! hanya menjalankan operasi yang tidak saling tumpang-tindih (tet yang
//! sudah disentuh ditandai "kotor"), sehingga tidak ada pembukuan
//! ketetanggaan inkremental yang rawan salah.

use crate::linalg::{add, cross, dot, norm, scale, sub, V3};
use crate::mesh::delaunay::{TetError, FACE};
use crate::mesh::spatial::SurfaceIndex;
use crate::mesh::tet_points::NO_TAG;
use crate::CancelToken;

pub(crate) const NONE: u32 = u32::MAX;

/// Enam tepi tet sebagai pasangan indeks lokal.
const TET_EDGES: [[usize; 2]; 6] = [[0, 1], [0, 2], [0, 3], [1, 2], [1, 3], [2, 3]];

/// Volume bertanda (positif = tangan kanan).
pub(crate) fn signed_volume(a: V3, b: V3, c: V3, d: V3) -> f64 {
    dot(sub(b, a), cross(sub(c, a), sub(d, a))) / 6.0
}

/// Rasio radius ternormalisasi; ≤ 0 untuk tet terbalik atau nol.
pub(crate) fn radius_ratio(a: V3, b: V3, c: V3, d: V3) -> f64 {
    let volume = signed_volume(a, b, c, d);
    if volume.is_nan() || volume <= 0.0 {
        return if volume < 0.0 { -1.0 } else { 0.0 };
    }
    let (u, v, w) = (sub(b, a), sub(c, a), sub(d, a));
    let area = 0.5
        * (norm(cross(u, v))
            + norm(cross(v, w))
            + norm(cross(w, u))
            + norm(cross(sub(c, b), sub(d, b))));
    let inradius = 3.0 * volume / area;
    let numer = add(
        scale(cross(v, w), dot(u, u)),
        add(scale(cross(w, u), dot(v, v)), scale(cross(u, v), dot(w, w))),
    );
    let circum = norm(numer) / (12.0 * volume);
    if circum > 0.0 && circum.is_finite() {
        3.0 * inradius / circum
    } else {
        0.0
    }
}

/// Ketetanggaan sisi: `adj[t][i]` = tet di seberang verteks lokal `i`.
pub(crate) fn build_adjacency(tets: &[[u32; 4]]) -> Result<Vec<[u32; 4]>, TetError> {
    let mut faces: Vec<([u32; 3], u32, u8)> = Vec::with_capacity(4 * tets.len());
    for (t, v) in tets.iter().enumerate() {
        for (i, f) in FACE.iter().enumerate() {
            let mut key = [v[f[0]], v[f[1]], v[f[2]]];
            key.sort_unstable();
            faces.push((key, t as u32, i as u8));
        }
    }
    faces.sort_unstable();
    let mut adj = vec![[NONE; 4]; tets.len()];
    let mut s = 0;
    while s < faces.len() {
        let mut e = s + 1;
        while e < faces.len() && faces[e].0 == faces[s].0 {
            e += 1;
        }
        match e - s {
            1 => {}
            2 => {
                adj[faces[s].1 as usize][faces[s].2 as usize] = faces[s + 1].1;
                adj[faces[s + 1].1 as usize][faces[s + 1].2 as usize] = faces[s].1;
            }
            _ => {
                return Err(TetError::failed(
                    "tet mesh has a face shared by more than two tets",
                ))
            }
        }
        s = e;
    }
    Ok(adj)
}

/// Mesh kerja (tet linier).
pub(crate) struct Work {
    pub pts: Vec<V3>,
    pub tets: Vec<[u32; 4]>,
    /// Titik `0..n_surface` adalah sampel permukaan: hanya boleh digeser
    /// di dalam face-nya sendiri (titik tepi fitur diam).
    pub n_surface: usize,
    /// Tag face tiap sampel permukaan (`NO_TAG` = titik tepi fitur).
    pub tags: Vec<u32>,
}

impl Work {
    pub fn quality(&self, t: [u32; 4]) -> f64 {
        radius_ratio(
            self.pts[t[0] as usize],
            self.pts[t[1] as usize],
            self.pts[t[2] as usize],
            self.pts[t[3] as usize],
        )
    }

    fn vol(&self, a: u32, b: u32, c: u32, d: u32) -> f64 {
        signed_volume(
            self.pts[a as usize],
            self.pts[b as usize],
            self.pts[c as usize],
            self.pts[d as usize],
        )
    }

    /// Tet berorientasi positif dari empat verteks, atau `None` bila pipih.
    fn oriented(&self, v: [u32; 4]) -> Option<[u32; 4]> {
        let s = self.vol(v[0], v[1], v[2], v[3]);
        if s > 0.0 {
            Some(v)
        } else if s < 0.0 {
            Some([v[1], v[0], v[2], v[3]])
        } else {
            None
        }
    }
}

/// Cincin tet di sekeliling tepi `(a, b)` mulai dari `start`:
/// `(tet, verteks cincin, tertutup)`. Cincin terbuka (tepi di batas) punya
/// satu verteks lebih banyak daripada tet. `None` bila cincin menyentuh tet
/// mati/kotor atau terlalu panjang.
fn edge_ring(
    tets: &[[u32; 4]],
    adj: &[[u32; 4]],
    blocked: &[bool],
    start: u32,
    a: u32,
    b: u32,
) -> Option<(Vec<u32>, Vec<u32>, bool)> {
    const MAX_RING: usize = 7;
    let first = tets[start as usize];
    let others: Vec<u32> = first
        .iter()
        .copied()
        .filter(|&v| v != a && v != b)
        .collect();
    if others.len() != 2 {
        return None;
    }
    // Berjalan menyeberangi sisi di seberang `drop`; mengembalikan
    // (tet, verteks baru) berurutan dan apakah kembali ke `start`.
    let walk = |mut drop: u32, mut keep: u32| -> Option<(Vec<(u32, u32)>, bool)> {
        let mut out = Vec::new();
        let mut cur = start;
        loop {
            let local = tets[cur as usize].iter().position(|&v| v == drop)?;
            let next = adj[cur as usize][local];
            if next == NONE {
                return Some((out, false));
            }
            if blocked[next as usize] {
                return None;
            }
            if next == start {
                return Some((out, true));
            }
            let fresh = tets[next as usize]
                .iter()
                .copied()
                .find(|&v| v != a && v != b && v != keep)?;
            out.push((next, fresh));
            if out.len() > MAX_RING {
                return None;
            }
            cur = next;
            drop = keep;
            keep = fresh;
        }
    };
    let (forward, closed) = walk(others[0], others[1])?;
    let mut ring_tets = vec![start];
    let mut ring = vec![others[0], others[1]];
    for &(t, v) in &forward {
        ring_tets.push(t);
        ring.push(v);
    }
    if closed {
        // Verteks terakhir sama dengan yang pertama.
        ring.pop();
        return Some((ring_tets, ring, true));
    }
    let (backward, _) = walk(others[1], others[0])?;
    for &(t, v) in &backward {
        ring_tets.insert(0, t);
        ring.insert(0, v);
    }
    (ring.len() <= MAX_RING).then_some((ring_tets, ring, false))
}

/// Kandidat operasi: (kualitas sesudah, tet lama, tet baru, verteks batas
/// yang disentuh).
type Candidate = (f64, Vec<u32>, Vec<[u32; 4]>, Vec<u32>);

/// Statistik perbaikan.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ImproveStats {
    pub passes: usize,
    pub peeled: usize,
    pub flips: usize,
    pub moves: usize,
}

/// Memperbaiki mesh sampai rasio radius minimum ≥ `target` atau tidak ada
/// lagi operasi yang membantu.
pub(crate) fn improve(
    work: &mut Work,
    index: &SurfaceIndex,
    target: f64,
    max_passes: usize,
    cancel: &CancelToken,
) -> Result<ImproveStats, TetError> {
    let mut stats = ImproveStats::default();
    for _ in 0..max_passes {
        if cancel.is_cancelled() {
            return Err(TetError::Cancelled);
        }
        stats.passes += 1;
        let adj = build_adjacency(&work.tets)?;
        let nt = work.tets.len();
        let q: Vec<f64> = work.tets.iter().map(|&t| work.quality(t)).collect();
        let mut order: Vec<u32> = (0..nt as u32).filter(|&t| q[t as usize] < target).collect();
        if order.is_empty() {
            break;
        }
        order.sort_by(|&x, &y| q[x as usize].total_cmp(&q[y as usize]).then(x.cmp(&y)));
        // Bintang verteks (CSR) dan penanda batas.
        let np = work.pts.len();
        let mut star_ptr = vec![0u32; np + 1];
        for t in &work.tets {
            for &v in t {
                star_ptr[v as usize + 1] += 1;
            }
        }
        for v in 0..np {
            star_ptr[v + 1] += star_ptr[v];
        }
        let mut star = vec![0u32; star_ptr[np] as usize];
        let mut cursor = star_ptr.clone();
        for (t, verts) in work.tets.iter().enumerate() {
            for &v in verts {
                star[cursor[v as usize] as usize] = t as u32;
                cursor[v as usize] += 1;
            }
        }
        let mut on_boundary = vec![false; np];
        let mut boundary_edges: Vec<(u32, u32)> = Vec::new();
        for (t, verts) in work.tets.iter().enumerate() {
            for (i, f) in FACE.iter().enumerate() {
                if adj[t][i] == NONE {
                    let tri = [verts[f[0]], verts[f[1]], verts[f[2]]];
                    for k in 0..3 {
                        on_boundary[tri[k] as usize] = true;
                        let (a, b) = (tri[k], tri[(k + 1) % 3]);
                        boundary_edges.push((a.min(b), a.max(b)));
                    }
                }
            }
        }
        boundary_edges.sort_unstable();
        boundary_edges.dedup();

        let mut blocked = vec![false; nt]; // mati atau kotor
        let mut dead = vec![false; nt];
        let mut touched = vec![false; np];
        let mut fresh: Vec<[u32; 4]> = Vec::new();
        let mut ops = 0usize;

        for &t in &order {
            let ti = t as usize;
            if blocked[ti] {
                continue;
            }
            let verts = work.tets[ti];
            // 1. Kupas sliver pipih di permukaan.
            let open: Vec<usize> = (0..4).filter(|&i| adj[ti][i] == NONE).collect();
            if !open.is_empty()
                && verts
                    .iter()
                    .all(|&v| (v as usize) < work.n_surface && !touched[v as usize])
            {
                // Pipih: tinggi terkecil jauh di bawah tepi terpanjang, dan
                // sisi-sisi batasnya hampir sebidang (bukan baji di tepi fitur).
                let p = [
                    work.pts[verts[0] as usize],
                    work.pts[verts[1] as usize],
                    work.pts[verts[2] as usize],
                    work.pts[verts[3] as usize],
                ];
                let mut longest = 0.0_f64;
                for e in TET_EDGES {
                    longest = longest.max(norm(sub(p[e[0]], p[e[1]])));
                }
                let mut largest_area = 0.0_f64;
                let mut normals: Vec<V3> = Vec::new();
                for (i, f) in FACE.iter().enumerate() {
                    let n = cross(sub(p[f[1]], p[f[0]]), sub(p[f[2]], p[f[0]]));
                    let area = 0.5 * norm(n);
                    largest_area = largest_area.max(area);
                    if open.contains(&i) && area > 1e-3 * longest * longest {
                        normals.push(scale(n, 0.5 / area));
                    }
                }
                let volume = signed_volume(p[0], p[1], p[2], p[3]);
                let thin = largest_area > 0.0 && 3.0 * volume / largest_area <= 0.05 * longest;
                let coplanar = normals.iter().all(|n| dot(*n, normals[0]) > 0.7);
                let flat = thin && coplanar;
                let valid = match open.len() {
                    1 => !on_boundary[verts[open[0]] as usize],
                    2 => {
                        let (a, b) = (verts[open[0]], verts[open[1]]);
                        boundary_edges.binary_search(&(a.min(b), a.max(b))).is_err()
                    }
                    3 => q[ti] < 1e-6,
                    _ => false,
                };
                if flat && valid {
                    dead[ti] = true;
                    blocked[ti] = true;
                    for &v in &verts {
                        touched[v as usize] = true;
                    }
                    stats.peeled += 1;
                    ops += 1;
                    continue;
                }
            }
            // 2. Flip terbaik.
            // (kualitas sesudah, tet lama, tet baru, verteks batas yang disentuh)
            let mut best: Option<Candidate> = None;
            let mut consider =
                |old: Vec<u32>, new: Vec<[u32; 4]>, surface: Vec<u32>, work: &Work| {
                    let before = old
                        .iter()
                        .map(|&o| q[o as usize])
                        .fold(f64::INFINITY, f64::min);
                    let after = new
                        .iter()
                        .map(|&n| work.quality(n))
                        .fold(f64::INFINITY, f64::min);
                    if after > before + 1e-3 && best.as_ref().is_none_or(|b| after > b.0) {
                        best = Some((after, old, new, surface));
                    }
                };
            for i in 0..4 {
                let n = adj[ti][i];
                if n == NONE || blocked[n as usize] {
                    continue;
                }
                let f = FACE[i];
                let (a, b, c, d) = (verts[f[0]], verts[f[1]], verts[f[2]], verts[i]);
                let Some(e) = work.tets[n as usize]
                    .iter()
                    .copied()
                    .find(|&v| v != a && v != b && v != c)
                else {
                    continue;
                };
                let vols = [
                    work.vol(a, b, d, e),
                    work.vol(b, c, d, e),
                    work.vol(c, a, d, e),
                ];
                let same = vols.iter().all(|&v| v > 0.0) || vols.iter().all(|&v| v < 0.0);
                if !same {
                    continue;
                }
                let new: Option<Vec<[u32; 4]>> = [[a, b, d, e], [b, c, d, e], [c, a, d, e]]
                    .iter()
                    .map(|&v| work.oriented(v))
                    .collect();
                if let Some(new) = new {
                    consider(vec![t, n], new, Vec::new(), work);
                }
            }
            for e in TET_EDGES {
                let (a, b) = (verts[e[0]], verts[e[1]]);
                let Some((ring_tets, ring, closed)) =
                    edge_ring(&work.tets, &adj, &blocked, t, a, b)
                else {
                    continue;
                };
                let n = ring.len();
                if n < 3 {
                    continue;
                }
                if !closed {
                    // Menghapus tepi batas mengganti dua sisi batas dengan dua
                    // sisi baru di atas tepi (r0, r_akhir): hanya boleh bila
                    // daerah itu pipih dan bukan tepi fitur.
                    let (r0, rm) = (ring[0], ring[n - 1]);
                    let quad = [a, b, r0, rm];
                    if quad
                        .iter()
                        .any(|&v| v as usize >= work.n_surface || touched[v as usize])
                    {
                        continue;
                    }
                    if boundary_edges
                        .binary_search(&(r0.min(rm), r0.max(rm)))
                        .is_ok()
                    {
                        continue;
                    }
                    let p = quad.map(|v| work.pts[v as usize]);
                    let mut longest = 0.0_f64;
                    for ed in TET_EDGES {
                        longest = longest.max(norm(sub(p[ed[0]], p[ed[1]])));
                    }
                    let mut largest_area = 0.0_f64;
                    for f in FACE {
                        largest_area = largest_area
                            .max(0.5 * norm(cross(sub(p[f[1]], p[f[0]]), sub(p[f[2]], p[f[0]]))));
                    }
                    let volume = signed_volume(p[0], p[1], p[2], p[3]).abs();
                    if !(largest_area > 0.0 && 3.0 * volume / largest_area <= 0.15 * longest) {
                        continue;
                    }
                    let n1 = cross(sub(p[1], p[0]), sub(p[2], p[0]));
                    let n2 = cross(sub(p[3], p[0]), sub(p[1], p[0]));
                    let (l1, l2) = (norm(n1), norm(n2));
                    let degenerate =
                        l1 <= 2e-3 * longest * longest || l2 <= 2e-3 * longest * longest;
                    if !degenerate && dot(n1, n2) < 0.7 * l1 * l2 {
                        continue;
                    }
                }
                for apex in 0..n {
                    let mut new: Vec<[u32; 4]> = Vec::with_capacity(2 * (n - 2));
                    let mut sign = 0.0;
                    let mut ok = true;
                    for i in 1..n - 1 {
                        let tri = [ring[apex], ring[(apex + i) % n], ring[(apex + i + 1) % n]];
                        let sa = work.vol(tri[0], tri[1], tri[2], a);
                        let sb = work.vol(tri[0], tri[1], tri[2], b);
                        if sa.is_nan()
                            || sb.is_nan()
                            || sa * sb >= 0.0
                            || (sign != 0.0 && sa * sign < 0.0)
                        {
                            ok = false;
                            break;
                        }
                        sign = sa;
                        match (
                            work.oriented([tri[0], tri[1], tri[2], a]),
                            work.oriented([tri[0], tri[1], tri[2], b]),
                        ) {
                            (Some(x), Some(y)) => {
                                new.push(x);
                                new.push(y);
                            }
                            _ => {
                                ok = false;
                                break;
                            }
                        }
                    }
                    if ok {
                        let tag = if closed {
                            Vec::new()
                        } else {
                            vec![a, b, ring[0], ring[n - 1]]
                        };
                        consider(ring_tets.clone(), new, tag, work);
                    }
                    if n == 3 {
                        break;
                    }
                }
            }
            if let Some((_, old, new, surface)) = best {
                for o in old {
                    dead[o as usize] = true;
                    blocked[o as usize] = true;
                }
                for v in surface {
                    touched[v as usize] = true;
                }
                fresh.extend(new);
                stats.flips += 1;
                ops += 1;
                continue;
            }
            // 3. Geser satu verteks interior.
            for local in 0..4 {
                let v = verts[local];
                let on_surface = (v as usize) < work.n_surface;
                if on_surface && (work.tags[v as usize] == NO_TAG || touched[v as usize]) {
                    continue;
                }
                let members =
                    &star[star_ptr[v as usize] as usize..star_ptr[v as usize + 1] as usize];
                if members.iter().any(|&m| blocked[m as usize]) {
                    continue;
                }
                let p = work.pts[v as usize];
                let star_quality = |work: &Work| {
                    members
                        .iter()
                        .map(|&m| work.quality(work.tets[m as usize]))
                        .fold(f64::INFINITY, f64::min)
                };
                let current = star_quality(work);
                let mut centroid = [0.0; 3];
                let mut count = 0.0;
                for &m in members {
                    for &w in &work.tets[m as usize] {
                        if w != v {
                            centroid = add(centroid, work.pts[w as usize]);
                            count += 1.0;
                        }
                    }
                }
                let centroid = scale(centroid, 1.0 / count);
                // Arah yang menaikkan tinggi tet ini di atas sisi seberang `v`.
                let f = FACE[local];
                let (a, b, c) = (
                    work.pts[verts[f[0]] as usize],
                    work.pts[verts[f[1]] as usize],
                    work.pts[verts[f[2]] as usize],
                );
                let n = cross(sub(b, a), sub(c, a));
                let length = norm(n).sqrt();
                let up = if norm(n) > 0.0 {
                    scale(n, length / norm(n))
                } else {
                    [0.0; 3]
                };
                let mut candidates: Vec<V3> = Vec::new();
                if on_surface {
                    // Geser menyinggung permukaan lalu proyeksikan ke face-nya.
                    let tag = work.tags[v as usize];
                    let mut reach = 0.0_f64;
                    for &m in members {
                        for &w in &work.tets[m as usize] {
                            if w != v {
                                reach = reach.max(norm(sub(work.pts[w as usize], p)));
                            }
                        }
                    }
                    let Some(here) = index.closest(p, Some(tag), None) else {
                        continue;
                    };
                    let normal = index.normals[here.tri as usize];
                    let seed = if normal[0].abs() < 0.8 {
                        [1.0, 0.0, 0.0]
                    } else {
                        [0.0, 1.0, 0.0]
                    };
                    let t1 = cross(normal, seed);
                    let t1 = scale(t1, 1.0 / norm(t1).max(f64::MIN_POSITIVE));
                    let t2 = cross(normal, t1);
                    for radius in [0.08, 0.16] {
                        for k in 0..8 {
                            let angle = k as f64 * std::f64::consts::FRAC_PI_4;
                            let offset = add(scale(t1, angle.cos()), scale(t2, angle.sin()));
                            let moved = add(p, scale(offset, radius * reach));
                            if let Some(c) = index.closest(moved, Some(tag), None) {
                                candidates.push(c.point);
                            }
                        }
                    }
                } else {
                    candidates.extend([
                        centroid,
                        scale(add(p, centroid), 0.5),
                        add(p, scale(up, 0.4)),
                        add(p, scale(up, 0.2)),
                        add(scale(add(p, centroid), 0.5), scale(up, 0.2)),
                    ]);
                }
                let mut chosen: Option<(f64, V3)> = None;
                for cand in candidates {
                    work.pts[v as usize] = cand;
                    let value = star_quality(work);
                    if value > current + 1e-3 && chosen.is_none_or(|c| value > c.0) {
                        chosen = Some((value, cand));
                    }
                }
                match chosen {
                    Some((_, cand)) => {
                        work.pts[v as usize] = cand;
                        for &m in members {
                            blocked[m as usize] = true;
                        }
                        touched[v as usize] = true;
                        stats.moves += 1;
                        ops += 1;
                        break;
                    }
                    None => work.pts[v as usize] = p,
                }
            }
        }
        if ops == 0 {
            break;
        }
        let mut next: Vec<[u32; 4]> = Vec::with_capacity(nt + fresh.len());
        for (t, verts) in work.tets.iter().enumerate() {
            if !dead[t] {
                next.push(*verts);
            }
        }
        next.extend(fresh);
        work.tets = next;
    }
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_tet_radius_ratio_reference_values() {
        // Tet beraturan → 1.
        let s = 1.0 / 2.0_f64.sqrt();
        let regular = [
            [1.0, 0.0, -s],
            [-1.0, 0.0, -s],
            [0.0, 1.0, s],
            [0.0, -1.0, s],
        ];
        let q = radius_ratio(regular[0], regular[1], regular[2], regular[3])
            .max(radius_ratio(regular[1], regular[0], regular[2], regular[3]));
        assert!((q - 1.0).abs() < 1e-12, "{q}");
        // Sliver: empat titik nyaris koplanar.
        let sliver = radius_ratio(
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 1e-4],
        );
        assert!(sliver > 0.0 && sliver < 1e-3);
        // Terbalik.
        assert!(
            radius_ratio(
                [0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0]
            ) < 0.0
        );
    }
}
