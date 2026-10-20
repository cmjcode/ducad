//! Proyeksi geometri body ke bidang sketsa (Paritas 1.14, `Op::SketchProject`).
//!
//! Dua mode:
//! - **project**: tepi B-rep terpilih (atau tepi batas face terpilih)
//!   diproyeksikan tegak lurus ke bidang sketsa. Garis → garis; lingkaran/
//!   busur yang sejajar bidang → lingkaran/busur eksak; lingkaran miring →
//!   spline sampel (proyeksinya elips); lingkaran tegak lurus bidang → garis.
//! - **intersect**: kurva irisan body dengan bidang sketsa, dihitung dari
//!   tessellation body (segitiga × bidang), dirangkai menjadi loop, lalu
//!   disederhanakan menjadi garis/lingkaran.
//!
//! Murni geometri: tidak menyentuh `ModelDoc`/undo stack (aturan modul
//! `compute`). Semua masukan rusak (NaN, shape kosong) menghasilkan daftar
//! kosong atau error terstruktur, tidak pernah panik (ADR 0009).

use std::collections::HashMap;

use ducad_kernel::{EdgeInfo, EdgeKind, FaceInfo, KernelMesh, KernelShape, SurfaceKind};
use ducad_sketch::Entity;
use glam::{DVec2, DVec3};

use crate::plane::PlaneFrame;

/// Panjang minimum entitas hasil proyeksi (mm); yang lebih pendek dibuang
/// (mis. tepi tegak lurus bidang yang runtuh menjadi titik).
const MIN_LEN: f64 = 1e-4;
/// Toleransi kesejajaran normal (|cos| di atas ini = sejajar).
const PARALLEL_COS: f64 = 1.0 - 1e-9;
/// |cos| di bawah ini = tegak lurus.
const PERP_COS: f64 = 1e-9;
/// Jumlah sampel per lingkaran penuh untuk proyeksi miring.
const CIRCLE_SAMPLES: usize = 48;
/// Toleransi kolinearitas (sin sudut) saat melebur ruas loop irisan.
const COLLINEAR_SIN: f64 = 1e-4;
/// Toleransi relatif jari-jari untuk mengenali loop irisan sebagai lingkaran
/// (tessellation f32 + tali busur).
const CIRCLE_REL_TOL: f64 = 5e-3;
/// Jarak (mm) titik tessellation dianggap tepat di bidang.
const ON_PLANE_EPS: f64 = 1e-7;

/// Hasil proyeksi: entitas (koordinat lokal bidang) + peringatan.
#[derive(Debug, Default)]
pub struct Projected {
    pub entities: Vec<Entity>,
    pub warnings: Vec<String>,
}

fn v3(a: [f64; 3]) -> DVec3 {
    DVec3::from_array(a)
}

fn finite2(p: DVec2) -> bool {
    p.x.is_finite() && p.y.is_finite()
}

/// Indeks tepi unik dari tepi-tepi face `faces` (urutan naik).
pub fn edges_of_faces(faces: &[FaceInfo], picked: &[usize]) -> Vec<usize> {
    let mut out: Vec<usize> = faces
        .iter()
        .filter(|f| picked.contains(&f.index))
        .flat_map(|f| f.edge_indices.iter().copied())
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// Proyeksikan tepi `edge_idx` dari `shape` ke bidang `frame`.
pub fn project_edges(shape: &KernelShape, edge_idx: &[usize], frame: &PlaneFrame) -> Projected {
    let edges = ducad_kernel::enumerate_edges(shape);
    let faces = ducad_kernel::enumerate_faces(shape);
    project_edge_infos(&edges, &faces, edge_idx, frame)
}

/// Inti [`project_edges`] di atas hasil enumerasi (bisa diuji tanpa kernel).
pub fn project_edge_infos(
    edges: &[EdgeInfo],
    faces: &[FaceInfo],
    edge_idx: &[usize],
    frame: &PlaneFrame,
) -> Projected {
    let mut out = Projected::default();
    let n = frame.normal_v().normalize_or_zero();
    if n == DVec3::ZERO {
        out.warnings.push("normal bidang sketsa tidak valid".to_string());
        return out;
    }
    for &i in edge_idx {
        let Some(e) = edges.iter().find(|e| e.index == i) else {
            continue;
        };
        let before = out.entities.len();
        match e.kind {
            EdgeKind::Line => {
                let a = frame.to_local(v3(e.start));
                let b = frame.to_local(v3(e.end));
                if finite2(a) && finite2(b) && (b - a).length() > MIN_LEN {
                    out.entities.push(Entity::line(a, b));
                }
            }
            EdgeKind::Circle => match circle_of(e, faces) {
                Some(c) => project_circle(&c, frame, n, &mut out.entities),
                None => out
                    .warnings
                    .push(format!("tepi {i}: lingkaran tanpa pusat/normal yang bisa ditentukan")),
            },
            EdgeKind::Other => {
                // Kurva bebas: hanya tiga titik yang tersedia dari API kernel
                // publik (awal, tengah, akhir) → spline kasar.
                let pts: Vec<DVec2> = [e.start, e.mid, e.end]
                    .iter()
                    .map(|p| frame.to_local(v3(*p)))
                    .collect();
                push_polyline_spline(pts, &mut out.entities);
                out.warnings.push(format!(
                    "tepi {i} (kurva bebas) diaproksimasi spline 3 titik"
                ));
            }
        }
        if out.entities.len() == before && e.kind != EdgeKind::Other {
            out.warnings
                .push(format!("tepi {i} runtuh menjadi titik pada bidang sketsa (dilewati)"));
        }
    }
    out
}

/// Lingkaran/busur 3D: pusat, normal satuan, jari-jari, sudut sapuan dari
/// titik awal (radian, positif mengikuti normal), titik awal.
struct Circle3 {
    center: DVec3,
    normal: DVec3,
    radius: f64,
    start: DVec3,
    sweep: f64,
}

impl Circle3 {
    fn point_at(&self, t: f64) -> DVec3 {
        let x = (self.start - self.center).normalize_or_zero();
        let y = self.normal.cross(x);
        let a = self.sweep * t;
        self.center + (x * a.cos() + y * a.sin()) * self.radius
    }
}

fn circle_of(e: &EdgeInfo, faces: &[FaceInfo]) -> Option<Circle3> {
    let (s, m, en) = (v3(e.start), v3(e.mid), v3(e.end));
    let closed = (s - en).length() < 1e-6;
    if closed {
        // Lingkaran penuh: titik tengah busur = titik seberang awal.
        let center = (s + m) * 0.5;
        let radius = e.radius.unwrap_or((s - center).length());
        // Normal dari face planar yang memuat tepi ini, atau sumbu silinder/kerucut.
        let normal = faces
            .iter()
            .filter(|f| f.edge_indices.contains(&e.index))
            .find_map(|f| match f.kind {
                SurfaceKind::Plane => Some(v3(f.normal)),
                _ => f.axis.map(|(_, d)| v3(d)),
            })?
            .normalize_or_zero();
        if normal == DVec3::ZERO || !radius.is_finite() || radius <= MIN_LEN {
            return None;
        }
        return Some(Circle3 {
            center,
            normal,
            radius,
            start: s,
            sweep: std::f64::consts::TAU,
        });
    }
    // Busur: pusat lingkaran luar tiga titik.
    let (ab, ac) = (m - s, en - s);
    let nrm = ab.cross(ac);
    let nn = nrm.length_squared();
    if !nn.is_finite() || nn <= 1e-18 {
        return None;
    }
    let center = s + (nrm.cross(ab) * ac.length_squared() + ac.cross(nrm) * ab.length_squared())
        / (2.0 * nn);
    let normal = nrm.normalize();
    let radius = (s - center).length();
    let x = (s - center).normalize_or_zero();
    let y = normal.cross(x);
    let ang = |p: DVec3| {
        let d = p - center;
        let a = d.dot(y).atan2(d.dot(x));
        if a < 0.0 {
            a + std::f64::consts::TAU
        } else {
            a
        }
    };
    // Dengan normal = (m−s)×(e−s), arah s→m→e selalu positif.
    let sweep = ang(en);
    if !sweep.is_finite() || !radius.is_finite() {
        return None;
    }
    Some(Circle3 {
        center,
        normal,
        radius,
        start: s,
        sweep,
    })
}

fn project_circle(c: &Circle3, frame: &PlaneFrame, n: DVec3, out: &mut Vec<Entity>) {
    let cos = c.normal.dot(n).abs();
    let full = (c.sweep - std::f64::consts::TAU).abs() < 1e-9;
    if cos >= PARALLEL_COS {
        let center = frame.to_local(c.center);
        if !finite2(center) {
            return;
        }
        if full {
            out.push(Entity::circle(center, c.radius));
            return;
        }
        let a = frame.to_local(c.start) - center;
        let b = frame.to_local(c.point_at(1.0)) - center;
        let mid = frame.to_local(c.point_at(0.5)) - center;
        let (a0, a1) = (a.y.atan2(a.x), b.y.atan2(b.x));
        // Busur sketsa selalu CCW; bila sapuan di bidang ternyata CW, tukar.
        let ccw = a.perp_dot(mid) > 0.0;
        let (s, e) = if ccw { (a0, a1) } else { (a1, a0) };
        out.push(Entity::arc(center, c.radius, s, e));
        return;
    }
    let samples = if full {
        CIRCLE_SAMPLES
    } else {
        ((CIRCLE_SAMPLES as f64 * c.sweep / std::f64::consts::TAU).ceil() as usize).max(4)
    };
    let pts: Vec<DVec2> = (0..=samples)
        .map(|k| frame.to_local(c.point_at(k as f64 / samples as f64)))
        .collect();
    if cos <= PERP_COS {
        // Lingkaran tegak lurus bidang → ruas garis antara dua titik ekstrem.
        if let Some((a, b)) = extreme_pair(&pts) {
            if (b - a).length() > MIN_LEN {
                out.push(Entity::line(a, b));
            }
        }
        return;
    }
    push_polyline_spline(pts, out);
}

/// Dua titik terjauh sepanjang arah sebaran utama.
fn extreme_pair(pts: &[DVec2]) -> Option<(DVec2, DVec2)> {
    let first = *pts.first()?;
    let far = pts
        .iter()
        .copied()
        .max_by(|a, b| (*a - first).length().total_cmp(&(*b - first).length()))?;
    let other = pts
        .iter()
        .copied()
        .max_by(|a, b| (*a - far).length().total_cmp(&(*b - far).length()))?;
    Some((far, other))
}

fn push_polyline_spline(mut pts: Vec<DVec2>, out: &mut Vec<Entity>) {
    pts.retain(|p| finite2(*p));
    pts.dedup_by(|a, b| (*a - *b).length() < MIN_LEN);
    match pts.len() {
        0 | 1 => {}
        2 => out.push(Entity::line(pts[0], pts[1])),
        _ => out.push(Entity::spline(pts)),
    }
}

/// Irisan body dengan bidang `frame` sebagai entitas sketsa.
pub fn intersect_body(shape: &KernelShape, frame: &PlaneFrame) -> Projected {
    let mesh = shape.tessellate();
    intersect_mesh(&mesh, frame)
}

/// Inti [`intersect_body`] di atas mesh (bisa diuji dengan mesh sintetis).
pub fn intersect_mesh(mesh: &KernelMesh, frame: &PlaneFrame) -> Projected {
    let mut out = Projected::default();
    let n = frame.normal_v().normalize_or_zero();
    if n == DVec3::ZERO {
        out.warnings.push("normal bidang sketsa tidak valid".to_string());
        return out;
    }
    let o = frame.origin_v();
    // Bias + dulu (titik di bidang dihitung sisi positif); bila tidak ada
    // irisan sama sekali (bidang berimpit face terluar), coba bias −.
    let mut loops = section_loops(mesh, o, n, 1.0);
    if loops.is_empty() {
        loops = section_loops(mesh, o, n, -1.0);
    }
    for (pts3, closed) in loops {
        let uv: Vec<DVec2> = pts3.iter().map(|p| frame.to_local(*p)).collect();
        loop_to_entities(&uv, closed, &mut out.entities);
    }
    out
}

/// Kunci titik irisan: tepat di vertex, atau di tengah tepi mesh (a < b).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    Vertex(u32),
    Edge(u32, u32),
}

/// Rangkai segmen irisan segitiga × bidang menjadi polyline. `bias` = sisi
/// tempat titik yang tepat di bidang dimasukkan (perturbasi simbolik agar
/// topologi irisan konsisten). Hasil: `(titik, tertutup)`.
fn section_loops(mesh: &KernelMesh, o: DVec3, n: DVec3, bias: f64) -> Vec<(Vec<DVec3>, bool)> {
    // Lebur vertex kembar (mesh per-face menduplikasi vertex di tepi face).
    let mut canon: HashMap<(i64, i64, i64), u32> = HashMap::new();
    let mut remap: Vec<u32> = Vec::with_capacity(mesh.positions.len());
    let mut pos: Vec<DVec3> = Vec::new();
    for p in &mesh.positions {
        let v = DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
        if !v.is_finite() {
            remap.push(u32::MAX);
            continue;
        }
        let q = ((v.x * 1e4).round() as i64, (v.y * 1e4).round() as i64, (v.z * 1e4).round() as i64);
        let id = *canon.entry(q).or_insert_with(|| {
            pos.push(v);
            (pos.len() - 1) as u32
        });
        remap.push(id);
    }
    let dist: Vec<f64> = pos.iter().map(|p| (*p - o).dot(n)).collect();
    let side = |d: f64| -> bool {
        // true = sisi positif (termasuk titik di bidang bila bias +).
        if d.abs() <= ON_PLANE_EPS {
            bias > 0.0
        } else {
            d > 0.0
        }
    };
    let point_of = |k: Key| -> Option<DVec3> {
        match k {
            Key::Vertex(v) => pos.get(v as usize).copied(),
            Key::Edge(a, b) => {
                let (pa, pb) = (*pos.get(a as usize)?, *pos.get(b as usize)?);
                let (da, db) = (*dist.get(a as usize)?, *dist.get(b as usize)?);
                let t = da / (da - db);
                t.is_finite().then(|| pa + (pb - pa) * t)
            }
        }
    };
    let crossing = |a: u32, b: u32| -> Key {
        let (da, db) = (dist[a as usize], dist[b as usize]);
        if da.abs() <= ON_PLANE_EPS {
            Key::Vertex(a)
        } else if db.abs() <= ON_PLANE_EPS {
            Key::Vertex(b)
        } else {
            Key::Edge(a.min(b), a.max(b))
        }
    };

    let mut adj: HashMap<Key, Vec<Key>> = HashMap::new();
    for tri in mesh.indices.chunks_exact(3) {
        let ids: Vec<u32> = tri
            .iter()
            .filter_map(|&i| remap.get(i as usize).copied())
            .filter(|&i| (i as usize) < pos.len())
            .collect();
        let [a, b, c] = ids[..] else { continue };
        if a == b || b == c || a == c {
            continue;
        }
        let s = [side(dist[a as usize]), side(dist[b as usize]), side(dist[c as usize])];
        let v = [a, b, c];
        let mut keys = Vec::with_capacity(2);
        for k in 0..3 {
            let (i, j) = (k, (k + 1) % 3);
            if s[i] != s[j] {
                keys.push(crossing(v[i], v[j]));
            }
        }
        if let [k0, k1] = keys[..] {
            if k0 != k1 {
                let e0 = adj.entry(k0).or_default();
                if !e0.contains(&k1) {
                    e0.push(k1);
                }
                let e1 = adj.entry(k1).or_default();
                if !e1.contains(&k0) {
                    e1.push(k0);
                }
            }
        }
    }

    // Jalan rantai: mulai dari ujung (derajat 1) dulu agar rantai terbuka
    // utuh, lalu sisa (loop tertutup). Urutan kunci dibuat deterministik.
    let mut keys: Vec<Key> = adj.keys().copied().collect();
    keys.sort_by_key(|k| match k {
        Key::Vertex(v) => (0, *v, 0),
        Key::Edge(a, b) => (1, *a, *b),
    });
    let mut used: std::collections::HashSet<(Key, Key)> = std::collections::HashSet::new();
    let edge_key = |a: Key, b: Key| {
        let ord = |k: Key| match k {
            Key::Vertex(v) => (0, v, 0),
            Key::Edge(x, y) => (1, x, y),
        };
        if ord(a) <= ord(b) {
            (a, b)
        } else {
            (b, a)
        }
    };
    let mut loops = Vec::new();
    let starts: Vec<Key> = keys
        .iter()
        .copied()
        .filter(|k| adj.get(k).is_some_and(|v| v.len() == 1))
        .chain(keys.iter().copied())
        .collect();
    for start in starts {
        while let Some(next) = adj
            .get(&start)
            .and_then(|v| v.iter().copied().find(|nb| !used.contains(&edge_key(start, *nb))))
        {
            let mut chain = vec![start];
            let (mut prev, mut cur) = (start, next);
            used.insert(edge_key(prev, cur));
            let mut closed = false;
            // Batas langkah = jumlah kunci: menjamin berhenti meski mesh rusak.
            for _ in 0..=keys.len() {
                if cur == start {
                    closed = true;
                    break;
                }
                chain.push(cur);
                let Some(nb) = adj.get(&cur).and_then(|v| {
                    v.iter()
                        .copied()
                        .find(|nb| *nb != prev && !used.contains(&edge_key(cur, *nb)))
                        .or_else(|| {
                            v.iter().copied().find(|nb| !used.contains(&edge_key(cur, *nb)))
                        })
                }) else {
                    break;
                };
                used.insert(edge_key(cur, nb));
                prev = cur;
                cur = nb;
            }
            let pts: Vec<DVec3> = chain.iter().filter_map(|k| point_of(*k)).collect();
            if pts.len() >= 2 {
                loops.push((pts, closed));
            }
        }
    }
    loops
}

/// Loop/rantai titik bidang → lingkaran (bila semua titik sejauh jari-jari
/// yang sama dari pusatnya) atau ruas-ruas garis dengan titik segaris dilebur.
fn loop_to_entities(uv: &[DVec2], closed: bool, out: &mut Vec<Entity>) {
    let mut pts: Vec<DVec2> = uv.iter().copied().filter(|p| finite2(*p)).collect();
    pts.dedup_by(|a, b| (*a - *b).length() < MIN_LEN);
    if closed && pts.len() > 1 && (pts[0] - pts[pts.len() - 1]).length() < MIN_LEN {
        pts.pop();
    }
    if pts.len() < 2 {
        return;
    }
    if closed && pts.len() >= 8 {
        let center = pts.iter().copied().sum::<DVec2>() / pts.len() as f64;
        let r = pts.iter().map(|p| (*p - center).length()).sum::<f64>() / pts.len() as f64;
        if r > MIN_LEN && pts.iter().all(|p| ((*p - center).length() - r).abs() <= r * CIRCLE_REL_TOL) {
            out.push(Entity::circle(center, r));
            return;
        }
    }
    // Lebur titik tengah yang segaris dengan tetangganya.
    let n = pts.len();
    let is_corner = |i: usize| -> bool {
        if !closed && (i == 0 || i == n - 1) {
            return true;
        }
        let prev = pts[(i + n - 1) % n];
        let next = pts[(i + 1) % n];
        let (d0, d1) = ((pts[i] - prev).normalize_or_zero(), (next - pts[i]).normalize_or_zero());
        d0.perp_dot(d1).abs() > COLLINEAR_SIN || d0.dot(d1) < 0.0
    };
    let corners: Vec<DVec2> = (0..n).filter(|&i| is_corner(i)).map(|i| pts[i]).collect();
    if corners.len() < 2 {
        return;
    }
    let segs = if closed { corners.len() } else { corners.len() - 1 };
    for i in 0..segs {
        let a = corners[i];
        let b = corners[(i + 1) % corners.len()];
        if (b - a).length() > MIN_LEN {
            out.push(Entity::line(a, b));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mesh kubus [0,s]^3 dengan vertex per-face terduplikasi (seperti
    /// tessellation B-rep) supaya peleburan vertex ikut teruji.
    fn cube_mesh(s: f32) -> KernelMesh {
        let quads: [[[f32; 3]; 4]; 6] = [
            [[0., 0., 0.], [s, 0., 0.], [s, s, 0.], [0., s, 0.]],
            [[0., 0., s], [s, 0., s], [s, s, s], [0., s, s]],
            [[0., 0., 0.], [s, 0., 0.], [s, 0., s], [0., 0., s]],
            [[0., s, 0.], [s, s, 0.], [s, s, s], [0., s, s]],
            [[0., 0., 0.], [0., s, 0.], [0., s, s], [0., 0., s]],
            [[s, 0., 0.], [s, s, 0.], [s, s, s], [s, 0., s]],
        ];
        let mut m = KernelMesh::default();
        for q in quads {
            let b = m.positions.len() as u32;
            m.positions.extend_from_slice(&q);
            m.indices.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
        }
        m
    }

    fn frame_at_z(z: f64) -> PlaneFrame {
        PlaneFrame {
            origin: [0.0, 0.0, z],
            ..PlaneFrame::top()
        }
    }

    #[test]
    fn intersect_cube_midplane_gives_square() {
        let r = intersect_mesh(&cube_mesh(10.0), &frame_at_z(5.0));
        assert_eq!(r.entities.len(), 4, "{:?}", r.entities);
        let total: f64 = r
            .entities
            .iter()
            .map(|e| match e {
                Entity::Line { start, end, .. } => (*end - *start).length(),
                _ => 0.0,
            })
            .sum();
        assert!((total - 40.0).abs() < 1e-3, "keliling {total}");
    }

    #[test]
    fn intersect_at_face_plane_falls_back_to_other_bias() {
        // Bidang berimpit face bawah: bias + tidak memberi irisan, bias − memberi.
        let r = intersect_mesh(&cube_mesh(10.0), &frame_at_z(0.0));
        assert_eq!(r.entities.len(), 4, "{:?}", r.entities);
    }

    #[test]
    fn intersect_miss_and_garbage_is_empty() {
        assert!(intersect_mesh(&cube_mesh(10.0), &frame_at_z(50.0)).entities.is_empty());
        let mut m = cube_mesh(10.0);
        m.positions[0] = [f32::NAN, 0.0, 0.0];
        m.indices.push(9999);
        let _ = intersect_mesh(&m, &frame_at_z(5.0));
        let bad = PlaneFrame {
            normal: [0.0, 0.0, 0.0],
            ..PlaneFrame::top()
        };
        assert!(intersect_mesh(&cube_mesh(10.0), &bad).entities.is_empty());
        assert!(intersect_mesh(&KernelMesh::default(), &frame_at_z(0.0)).entities.is_empty());
    }

    #[test]
    fn closed_polygon_on_circle_becomes_circle() {
        let pts: Vec<DVec2> = (0..32)
            .map(|i| {
                let a = i as f64 / 32.0 * std::f64::consts::TAU;
                DVec2::new(3.0 + 5.0 * a.cos(), 5.0 * a.sin())
            })
            .collect();
        let mut out = Vec::new();
        loop_to_entities(&pts, true, &mut out);
        match out.as_slice() {
            [Entity::Circle { center, radius, .. }] => {
                assert!((center.x - 3.0).abs() < 1e-6 && (radius - 5.0).abs() < 1e-6);
            }
            other => panic!("{other:?}"),
        }
    }

    fn line_edge(index: usize, a: [f64; 3], b: [f64; 3]) -> EdgeInfo {
        EdgeInfo {
            index,
            kind: EdgeKind::Line,
            start: a,
            end: b,
            mid: [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, (a[2] + b[2]) / 2.0],
            length: 0.0,
            dir: None,
            radius: None,
        }
    }

    #[test]
    fn project_lines_drops_perpendicular_edges() {
        let edges = vec![
            line_edge(0, [0.0, 0.0, 7.0], [10.0, 0.0, 7.0]),
            line_edge(1, [0.0, 0.0, 0.0], [0.0, 0.0, 7.0]),
        ];
        let r = project_edge_infos(&edges, &[], &[0, 1, 99], &PlaneFrame::top());
        assert_eq!(r.entities.len(), 1);
        assert!(matches!(r.entities[0], Entity::Line { end, .. } if (end.x - 10.0).abs() < 1e-9));
        assert_eq!(r.warnings.len(), 1);
    }

    #[test]
    fn project_parallel_arc_is_exact_and_tilted_is_spline() {
        // Busur seperempat lingkaran r=5 di z=3 dari (5,0) ke (0,5).
        let h = 5.0 / 2f64.sqrt();
        let arc = EdgeInfo {
            index: 0,
            kind: EdgeKind::Circle,
            start: [5.0, 0.0, 3.0],
            end: [0.0, 5.0, 3.0],
            mid: [h, h, 3.0],
            length: 0.0,
            dir: None,
            radius: Some(5.0),
        };
        let r = project_edge_infos(std::slice::from_ref(&arc), &[], &[0], &PlaneFrame::top());
        match r.entities.as_slice() {
            [Entity::Arc { radius, start_angle, end_angle, .. }] => {
                assert!((radius - 5.0).abs() < 1e-9);
                assert!(start_angle.abs() < 1e-9);
                assert!((end_angle - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
            }
            other => panic!("{other:?}"),
        }
        // Busur yang sama diproyeksikan ke bidang Front (tegak lurus) → garis/spline.
        let r = project_edge_infos(&[arc], &[], &[0], &PlaneFrame::front());
        assert_eq!(r.entities.len(), 1, "{:?}", r.entities);
    }
}
