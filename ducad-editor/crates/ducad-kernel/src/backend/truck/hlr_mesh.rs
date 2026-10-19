//! HLR lembar gambar backend truck (PLAN_MIGRASI_TRUCK §4.1, F4.2): hybrid
//! B-rep + mesh, pengganti `HLRBRep_Algo` OCCT.
//!
//! - **Garis kandidat** = edge logis B-rep (`super::topology`), bukan rusuk
//!   segitiga. Seam dan edge mulus (tangen, mis. batas fillet) tidak
//!   digambar, sama seperti keluaran "sharp" HLR OCCT.
//! - **Siluet** permukaan lengkung dicari di mesh tampilan (rusuk dalam satu
//!   face yang memisahkan segitiga menghadap/membelakangi pemirsa), lalu
//!   di-*snap* ke generatris/lingkaran analitik silinder, kerucut, dan bola
//!   sehingga posisinya eksak, bukan sebatas chord mesh.
//! - **Oklusi**: segitiga yang menghadap pemirsa diproyeksikan ke grid 2D;
//!   setiap garis diuji per potongan pendek, lalu titik peralihan
//!   tampak/tersembunyi dicari dengan bisection.
//! - **Lingkaran** yang bidangnya sejajar bidang gambar keluar sebagai
//!   [`HlrArc2D`](crate::hlr::HlrArc2D) dengan pusat & radius dari B-rep —
//!   inilah yang dipakai dimensi otomatis (Ø, R).
//!
//! Konvensi koordinat sama dengan [`crate::hlr_sheet`]: `p ↦ (p·right, p·up)`,
//! pemirsa di arah `right × up`.

use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use glam::{DVec2, DVec3, Vec3};
use solidcraft_kernel::FaceSurface;

use super::bridge::to_dv;
use super::topology::{display_tolerance, Topology};
use crate::hlr::{HlrLineKind, HlrSegment2D, ProjectedView, ProjectedViewKind};
use crate::hlr_exact::{ExactCurve2D, ExactCurveType, ExactHlrView, ExactLineKind};
use crate::hlr_sheet::{
    arc_about, circumcenter3, lines_bounds, push_polyline, seg, to_d, ExactLines, SheetFace,
};
use crate::shape::KernelShape;
use crate::topo::EdgeKind;

/// Basis ortonormal bidang gambar.
struct Frame {
    r: DVec3,
    u: DVec3,
    /// Menuju pemirsa (`r × u`); kedalaman makin besar makin dekat.
    t: DVec3,
}

impl Frame {
    fn new(right: DVec3, up: DVec3) -> Option<Frame> {
        let r = right.normalize_or_zero();
        let u = (up - r * up.dot(r)).normalize_or_zero();
        let t = r.cross(u);
        (r != DVec3::ZERO && t.length_squared() > 0.5).then_some(Frame { r, u, t })
    }
    fn p2(&self, p: DVec3) -> DVec2 {
        DVec2::new(p.dot(self.r), p.dot(self.u))
    }
    fn depth(&self, p: DVec3) -> f64 {
        p.dot(self.t)
    }
}

fn mesh_point(topo: &Topology, i: u32) -> Option<DVec3> {
    topo.mesh
        .positions
        .get(i as usize)
        .map(|q| DVec3::new(q[0] as f64, q[1] as f64, q[2] as f64))
}

// ---------------------------------------------------------------------------
// Oklusi
// ---------------------------------------------------------------------------

struct Tri {
    a: DVec2,
    b: DVec2,
    c: DVec2,
    da: f64,
    db: f64,
    dc: f64,
    lo: DVec2,
    hi: DVec2,
}

/// Segitiga penghalang (menghadap pemirsa) dalam grid 2D seragam.
struct Occluders {
    tris: Vec<Tri>,
    origin: DVec2,
    cell: f64,
    nx: usize,
    ny: usize,
    cells: Vec<Vec<u32>>,
    /// Ambang "benar-benar di depan" (mm): di atas galat chord mesh, supaya
    /// permukaan tidak menghalangi edge/siluetnya sendiri.
    eps: f64,
}

impl Occluders {
    fn build(topos: &[&Topology], frame: &Frame, eps: f64) -> Occluders {
        let mut tris = Vec::new();
        for topo in topos {
            for t in topo.mesh.indices.chunks_exact(3) {
                let (Some(a), Some(b), Some(c)) = (
                    mesh_point(topo, t[0]),
                    mesh_point(topo, t[1]),
                    mesh_point(topo, t[2]),
                ) else {
                    continue;
                };
                let n = (b - a).cross(c - a);
                let len = n.length();
                // Hanya yang menghadap pemirsa; segitiga hampir sejajar arah
                // pandang (proyeksinya pipih) tidak menghalangi apa pun.
                if len < 1e-14 || n.dot(frame.t) / len <= 1e-3 {
                    continue;
                }
                let (a2, b2, c2) = (frame.p2(a), frame.p2(b), frame.p2(c));
                tris.push(Tri {
                    a: a2,
                    b: b2,
                    c: c2,
                    da: frame.depth(a),
                    db: frame.depth(b),
                    dc: frame.depth(c),
                    lo: a2.min(b2).min(c2),
                    hi: a2.max(b2).max(c2),
                });
            }
        }
        let (mut lo, mut hi) = (DVec2::splat(f64::MAX), DVec2::splat(f64::MIN));
        for t in &tris {
            lo = lo.min(t.lo);
            hi = hi.max(t.hi);
        }
        if tris.is_empty() {
            lo = DVec2::ZERO;
            hi = DVec2::ONE;
        }
        let side = ((tris.len() as f64).sqrt() * 0.5).clamp(1.0, 256.0);
        let cell = (hi - lo).max_element().max(1e-6) / side;
        let nx = (((hi.x - lo.x) / cell).floor() as usize + 1).min(512);
        let ny = (((hi.y - lo.y) / cell).floor() as usize + 1).min(512);
        let mut cells = vec![Vec::new(); nx * ny];
        let idx =
            |v: f64, o: f64, n: usize| (((v - o) / cell).floor().max(0.0) as usize).min(n - 1);
        for (i, t) in tris.iter().enumerate() {
            for gy in idx(t.lo.y, lo.y, ny)..=idx(t.hi.y, lo.y, ny) {
                for gx in idx(t.lo.x, lo.x, nx)..=idx(t.hi.x, lo.x, nx) {
                    cells[gy * nx + gx].push(i as u32);
                }
            }
        }
        Occluders {
            tris,
            origin: lo,
            cell,
            nx,
            ny,
            cells,
            eps,
        }
    }

    /// Apakah titik 3D `p` tertutup segitiga yang lebih dekat ke pemirsa.
    fn hidden(&self, frame: &Frame, p: DVec3) -> bool {
        let q = frame.p2(p);
        let d = frame.depth(p);
        let gx = ((q.x - self.origin.x) / self.cell).floor();
        let gy = ((q.y - self.origin.y) / self.cell).floor();
        if gx < 0.0 || gy < 0.0 || gx as usize >= self.nx || gy as usize >= self.ny {
            return false;
        }
        let cross = |o: DVec2, a: DVec2, b: DVec2| (a - o).perp_dot(b - o);
        for &i in &self.cells[gy as usize * self.nx + gx as usize] {
            let t = &self.tris[i as usize];
            if q.x < t.lo.x || q.x > t.hi.x || q.y < t.lo.y || q.y > t.hi.y {
                continue;
            }
            let area = cross(t.a, t.b, t.c);
            if area.abs() < 1e-14 {
                continue;
            }
            let wa = cross(q, t.b, t.c) / area;
            let wb = cross(t.a, q, t.c) / area;
            let wc = 1.0 - wa - wb;
            let tol = -1e-9;
            if wa < tol || wb < tol || wc < tol {
                continue;
            }
            if wa * t.da + wb * t.db + wc * t.dc > d + self.eps {
                return true;
            }
        }
        false
    }
}

/// Potongan tampak/tersembunyi sebuah polyline 3D: `(titik, tersembunyi)`.
fn visibility_runs(
    poly: &[DVec3],
    occ: &Occluders,
    frame: &Frame,
    step: f64,
) -> Vec<(Vec<DVec3>, bool)> {
    if poly.len() < 2 {
        return Vec::new();
    }
    let mut cum = Vec::with_capacity(poly.len());
    let mut acc = 0.0;
    cum.push(0.0);
    for w in poly.windows(2) {
        acc += (w[1] - w[0]).length();
        cum.push(acc);
    }
    let total = acc;
    if total < 1e-9 {
        return Vec::new();
    }
    let at = |s: f64| -> DVec3 {
        let s = s.clamp(0.0, total);
        let i = cum.partition_point(|&c| c <= s).clamp(1, poly.len() - 1);
        let (s0, s1) = (cum[i - 1], cum[i]);
        let f = if s1 > s0 { (s - s0) / (s1 - s0) } else { 0.0 };
        poly[i - 1] + (poly[i] - poly[i - 1]) * f
    };
    let hid = |s: f64| occ.hidden(frame, at(s));

    let n = ((total / step).ceil() as usize).clamp(1, 4000);
    let h = total / n as f64;
    // (awal, akhir, tersembunyi) per potongan, digabung yang sejenis.
    let mut runs: Vec<(f64, f64, bool)> = Vec::new();
    for k in 0..n {
        let hidden = hid((k as f64 + 0.5) * h);
        match runs.last_mut() {
            Some(last) if last.2 == hidden => last.1 = (k + 1) as f64 * h,
            _ => runs.push((k as f64 * h, (k + 1) as f64 * h, hidden)),
        }
    }
    // Titik peralihan: bisection di antara pusat dua potongan yang berbeda.
    for i in 1..runs.len() {
        let boundary = runs[i].0;
        let (mut a, mut b) = ((boundary - h * 0.5).max(0.0), (boundary + h * 0.5).min(total));
        let left = runs[i - 1].2;
        for _ in 0..30 {
            if b - a < 1e-5 {
                break;
            }
            let m = 0.5 * (a + b);
            if hid(m) == left {
                a = m;
            } else {
                b = m;
            }
        }
        let s = 0.5 * (a + b);
        runs[i - 1].1 = s;
        runs[i].0 = s;
    }
    let mut out: Vec<(Vec<DVec3>, bool)> = runs
        .into_iter()
        .filter(|(s0, s1, _)| s1 - s0 > 1e-6)
        .map(|(s0, s1, hidden)| {
            let mut pts = vec![at(s0)];
            for (p, &c) in poly.iter().zip(&cum) {
                if c > s0 + 1e-9 && c < s1 - 1e-9 {
                    pts.push(*p);
                }
            }
            pts.push(at(s1));
            (pts, hidden)
        })
        .collect();
    // Polyline tertutup (lingkaran penuh) dimulai di titik sembarang (seam):
    // potongan terakhir dan pertama yang sejenis sebenarnya satu potongan.
    let closed = (poly[0] - poly[poly.len() - 1]).length() < 1e-9 * (1.0 + total);
    if closed && out.len() > 1 && out[0].1 == out[out.len() - 1].1 {
        let (first, _) = out.remove(0);
        if let Some(last) = out.last_mut() {
            last.0.extend(first.into_iter().skip(1));
        }
    }
    out
}

/// Lingkaran yang bidangnya memuat arah pandang tampak sebagai ruas garis
/// `pusat ± r` searah `n × t`. Ruas eksak untuk satu potongan `pts` busur.
fn edge_on_circle_segment(
    frame: &Frame,
    center: DVec3,
    normal: DVec3,
    radius: f64,
    pts: &[DVec3],
) -> Option<(DVec2, DVec2)> {
    let b2 = normal.cross(frame.t).normalize_or_zero();
    let b1 = frame.t;
    if b2 == DVec3::ZERO || pts.len() < 2 {
        return None;
    }
    // Sudut φ tiap titik (p = c + r(cos φ b2 + sin φ b1)), dibuka kontinu.
    let ang = |p: DVec3| {
        let q = p - center;
        q.dot(b1).atan2(q.dot(b2))
    };
    let (mut lo, mut hi) = (ang(pts[0]), ang(pts[0]));
    let (mut prev, mut acc) = (lo, lo);
    for p in &pts[1..] {
        let a = ang(*p);
        let mut d = a - prev;
        while d > std::f64::consts::PI {
            d -= std::f64::consts::TAU;
        }
        while d < -std::f64::consts::PI {
            d += std::f64::consts::TAU;
        }
        acc += d;
        prev = a;
        lo = lo.min(acc);
        hi = hi.max(acc);
    }
    // s = r cos φ di [lo, hi]; ekstrem ±r bila rentang memuat kelipatan π.
    let (mut smin, mut smax) = (lo.cos().min(hi.cos()), lo.cos().max(hi.cos()));
    let pi = std::f64::consts::PI;
    let mut k = (lo / pi).ceil();
    while k * pi <= hi {
        if (k as i64).rem_euclid(2) == 0 {
            smax = 1.0;
        } else {
            smin = -1.0;
        }
        k += 1.0;
    }
    let c2 = frame.p2(center);
    let dir = frame.p2(b2);
    Some((c2 + dir * (smin * radius), c2 + dir * (smax * radius)))
}

/// Buang titik tengah yang segaris (lipatan bolak-balik tetap dipertahankan).
fn simplify_2d(pts: &[DVec2], tol: f64) -> Vec<DVec2> {
    let mut out: Vec<DVec2> = Vec::with_capacity(pts.len());
    for &q in pts {
        if out.last().is_some_and(|l| (*l - q).length() < 1e-9) {
            continue;
        }
        if out.len() >= 2 {
            let s = out[out.len() - 2];
            let prev = out[out.len() - 1];
            let d = q - s;
            let len = d.length();
            if len > 1e-12 {
                let off = (prev - s).perp_dot(d).abs() / len;
                let forward = (prev - s).dot(d) > 0.0 && (prev - s).length() <= len;
                if off < tol && forward {
                    out.pop();
                }
            }
        }
        out.push(q);
    }
    out
}

// ---------------------------------------------------------------------------
// Edge & siluet
// ---------------------------------------------------------------------------

/// Pusat, normal, dan radius edge lingkaran logis.
fn circle_of(points: &[DVec3], radius: Option<f64>) -> Option<(DVec3, DVec3, f64)> {
    let n = points.len();
    if n < 3 {
        return None;
    }
    let (center, normal) = circumcenter3(points[0], points[n / 3], points[(2 * n) / 3])?;
    Some((center, normal, radius.unwrap_or((points[0] - center).length())))
}

/// Normal face `f` di dekat `p` (bidang: normal eksaknya).
fn normal_at(topo: &Topology, f: usize, p: DVec3) -> Option<DVec3> {
    match topo.faces.get(f)?.surface {
        Some(FaceSurface::Plane { normal, .. }) => Some(to_dv(*normal)),
        _ => topo.normal_near(f, p),
    }
}

/// Edge di antara dua face yang bersinggungan (tangen) — tidak digambar,
/// seperti garis "smooth" yang tidak termasuk keluaran sharp HLR OCCT.
fn is_smooth(topo: &Topology, faces: &[usize], mid: DVec3) -> bool {
    let [f0, f1] = faces else {
        return false;
    };
    match (normal_at(topo, *f0, mid), normal_at(topo, *f1, mid)) {
        (Some(a), Some(b)) => a.dot(b) > 0.995,
        _ => false,
    }
}

/// Siluet analitik sebuah permukaan: titik siluet mesh di-snap ke sini.
enum Snap {
    Cylinder {
        origin: DVec3,
        axis: DVec3,
        radius: f64,
        side: DVec3,
    },
    Cone {
        apex: DVec3,
        axis: DVec3,
        tan: f64,
        e1: DVec3,
        e2: DVec3,
        phis: [f64; 2],
    },
    Sphere {
        center: DVec3,
        radius: f64,
    },
    None,
}

impl Snap {
    fn of(surface: Option<FaceSurface>, frame: &Frame) -> Snap {
        match surface {
            Some(FaceSurface::Cylinder {
                origin,
                axis,
                radius,
            }) => {
                let axis = to_dv(*axis);
                let side = axis.cross(frame.t);
                if side.length() < 1e-6 {
                    return Snap::None;
                }
                Snap::Cylinder {
                    origin: to_dv(origin),
                    axis,
                    radius,
                    side: side.normalize(),
                }
            }
            Some(FaceSurface::Cone {
                apex,
                axis,
                half_angle,
            }) => {
                // Generatris siluet: n̂·t⊥ = tan α · (w·t), n̂ = cos φ e1 + sin φ e2.
                let w = to_dv(*axis);
                let tp = frame.t - w * w.dot(frame.t);
                let tau = tp.length();
                if tau < 1e-9 {
                    return Snap::None;
                }
                let e1 = tp / tau;
                let e2 = w.cross(e1);
                let tan = half_angle.tan();
                let c = tan * w.dot(frame.t) / tau;
                if c.abs() > 1.0 {
                    return Snap::None;
                }
                let phi = c.acos();
                Snap::Cone {
                    apex: to_dv(apex),
                    axis: w,
                    tan,
                    e1,
                    e2,
                    phis: [phi, -phi],
                }
            }
            Some(FaceSurface::Sphere { center, radius }) => Snap::Sphere {
                center: to_dv(center),
                radius,
            },
            _ => Snap::None,
        }
    }

    fn apply(&self, p: DVec3, frame: &Frame) -> DVec3 {
        match *self {
            Snap::Cylinder {
                origin,
                axis,
                radius,
                side,
            } => {
                let rel = p - origin;
                let s = if rel.dot(side) >= 0.0 { 1.0 } else { -1.0 };
                origin + axis * rel.dot(axis) + side * (radius * s)
            }
            Snap::Cone {
                apex,
                axis,
                tan,
                e1,
                e2,
                phis,
            } => {
                let rel = p - apex;
                let a = rel.dot(axis);
                let radial = rel - axis * a;
                let ang = radial.dot(e2).atan2(radial.dot(e1));
                let dist = |x: f64| {
                    let d = (x - ang).rem_euclid(std::f64::consts::TAU);
                    d.min(std::f64::consts::TAU - d)
                };
                let phi = if dist(phis[0]) <= dist(phis[1]) {
                    phis[0]
                } else {
                    phis[1]
                };
                let n = e1 * phi.cos() + e2 * phi.sin();
                apex + axis * a + n * (a * tan)
            }
            Snap::Sphere { center, radius } => {
                let rel = p - center;
                let flat = rel - frame.t * rel.dot(frame.t);
                if flat.length() < 1e-12 {
                    return p;
                }
                center + flat.normalize() * radius
            }
            Snap::None => p,
        }
    }
}

type Key = (i64, i64, i64);

/// Rantai siluet mesh tiap face lengkung, sudah di-snap ke permukaannya.
fn silhouettes(topo: &Topology, frame: &Frame, quantum: f64) -> Vec<(Snap, Vec<Vec<DVec3>>)> {
    let key = |p: DVec3| -> Key {
        (
            (p.x / quantum).round() as i64,
            (p.y / quantum).round() as i64,
            (p.z / quantum).round() as i64,
        )
    };
    let mut out = Vec::new();
    for (f, face) in topo.faces.iter().enumerate() {
        if matches!(face.surface, Some(FaceSurface::Plane { .. })) {
            continue;
        }
        let range = topo.mesh.face_ranges.get(f).cloned().unwrap_or(0..0);
        let idx = topo
            .mesh
            .indices
            .get(range.start as usize..range.end as usize)
            .unwrap_or(&[]);
        // rusuk (kunci terurut) → (titik di kedua kunci, orientasi tiap segitiga)
        let mut edges: HashMap<(Key, Key), (DVec3, DVec3, Vec<f64>)> = HashMap::new();
        for t in idx.chunks_exact(3) {
            let (Some(a), Some(b), Some(c)) = (
                mesh_point(topo, t[0]),
                mesh_point(topo, t[1]),
                mesh_point(topo, t[2]),
            ) else {
                continue;
            };
            let n = (b - a).cross(c - a);
            let len = n.length();
            if len < 1e-14 {
                continue;
            }
            let facing = n.dot(frame.t) / len;
            for (p, q) in [(a, b), (b, c), (c, a)] {
                let (kp, kq) = (key(p), key(q));
                let (k, pa, pb) = if kp < kq { ((kp, kq), p, q) } else { ((kq, kp), q, p) };
                edges.entry(k).or_insert((pa, pb, Vec::new())).2.push(facing);
            }
        }
        let mut segs: Vec<(Key, Key, DVec3, DVec3)> = edges
            .into_iter()
            .filter(|(_, (_, _, f))| {
                // Satu menghadap, satu membelakangi; face yang seluruhnya
                // sejajar arah pandang (mis. sisi silinder dari atas) tidak
                // punya siluet — garis luarnya adalah edge batasnya.
                f.len() == 2 && f[0] * f[1] < 0.0 && f[0].abs().max(f[1].abs()) > 1e-6
            })
            .map(|((ka, kb), (pa, pb, _))| (ka, kb, pa, pb))
            .collect();
        if segs.is_empty() {
            continue;
        }
        // Urutan deterministik sebelum dirangkai.
        segs.sort_by_key(|s| (s.0, s.1));
        let snap = Snap::of(face.surface, frame);
        let chains = chain_segments(&segs)
            .into_iter()
            .map(|c| c.into_iter().map(|p| snap.apply(p, frame)).collect())
            .collect();
        out.push((snap, chains));
    }
    out
}

/// Rangkai segmen yang berbagi ujung menjadi polyline.
fn chain_segments(segs: &[(Key, Key, DVec3, DVec3)]) -> Vec<Vec<DVec3>> {
    let mut at: HashMap<Key, Vec<usize>> = HashMap::new();
    for (i, s) in segs.iter().enumerate() {
        at.entry(s.0).or_default().push(i);
        at.entry(s.1).or_default().push(i);
    }
    let mut used = vec![false; segs.len()];
    let other = |i: usize, k: Key| {
        if segs[i].0 == k {
            (segs[i].1, segs[i].3)
        } else {
            (segs[i].0, segs[i].2)
        }
    };
    let next = |used: &[bool], k: Key| {
        at.get(&k)
            .and_then(|v| v.iter().copied().find(|&j| !used[j]))
    };
    // Mulai dari ujung bebas lebih dulu, lalu sisanya (loop tertutup).
    let mut order: Vec<usize> = (0..segs.len())
        .filter(|&i| at[&segs[i].0].len() != 2 || at[&segs[i].1].len() != 2)
        .collect();
    order.extend(0..segs.len());
    let mut out = Vec::new();
    for start in order {
        if used[start] {
            continue;
        }
        used[start] = true;
        let s = &segs[start];
        let (mut head, mut tail) = (s.0, s.1);
        let mut pts = VecDeque::from(vec![s.2, s.3]);
        while let Some(j) = next(&used, tail) {
            used[j] = true;
            let (k, p) = other(j, tail);
            pts.push_back(p);
            tail = k;
        }
        while let Some(j) = next(&used, head) {
            used[j] = true;
            let (k, p) = other(j, head);
            pts.push_front(p);
            head = k;
        }
        out.push(pts.into_iter().collect());
    }
    out
}

/// Diagonal bbox gabungan seluruh shape.
fn model_size(shapes: &[&KernelShape]) -> f64 {
    let (mut lo, mut hi) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
    for s in shapes {
        if let Some((a, b)) = s.bounding_box() {
            lo = lo.min(DVec3::from_array(a));
            hi = hi.max(DVec3::from_array(b));
        }
    }
    if lo.x > hi.x {
        0.0
    } else {
        (hi - lo).length()
    }
}

/// Garis & busur tampak (dan tersembunyi bila `keep_hidden`) seluruh
/// `shapes` pada bidang gambar `right`/`up`. `None` bila tidak ada B-rep.
pub(crate) fn exact_lines(
    shapes: &[&KernelShape],
    right: Vec3,
    up: Vec3,
    keep_hidden: bool,
) -> Option<ExactLines> {
    let frame = Frame::new(to_d(right), to_d(up))?;
    let solids: Vec<&KernelShape> = shapes
        .iter()
        .copied()
        .filter(|s| s.body().is_some())
        .collect();
    if solids.is_empty() {
        return None;
    }
    let size = model_size(&solids).max(1e-3);
    let chord = display_tolerance(size);
    let topos: Vec<&Topology> = solids.iter().map(|s| s.topology()).collect();
    let occ = Occluders::build(&topos, &frame, (chord * 3.0).max(size * 2e-5).max(2e-3));
    let step = (size / 400.0).clamp(0.02, 2.0);
    let kind_of = |hidden: bool, visible: HlrLineKind| {
        if hidden {
            HlrLineKind::Hidden
        } else {
            visible
        }
    };

    let mut out = ExactLines::default();
    // Indeks edge berlanjut antar body dalam urutan body gambar.
    let mut offset = 0u32;
    for shape in shapes {
        let topo = shape.topology();
        let base = offset;
        offset += topo.edges.len() as u32;
        if shape.body().is_none() {
            continue;
        }
        for (ei, e) in topo.edges.iter().enumerate() {
            // Seam (face periodik yang sama di kedua sisi) dan edge tangen.
            if e.faces.len() < 2 || e.points.len() < 2 || is_smooth(topo, &e.faces, e.mid()) {
                continue;
            }
            let idx = base + ei as u32;
            let geom = (e.kind == EdgeKind::Circle)
                .then(|| circle_of(&e.points, e.radius))
                .flatten();
            // Lingkaran sejajar bidang gambar → busur; tegak lurus → ruas garis.
            let circle = geom.filter(|(_, n, _)| n.dot(frame.t).abs() > 0.9999);
            let edge_on = geom.filter(|(_, n, _)| n.dot(frame.t).abs() < 1e-6);
            for (pts, hidden) in visibility_runs(&e.points, &occ, &frame, step) {
                if hidden && !keep_hidden {
                    continue;
                }
                let kind = kind_of(hidden, HlrLineKind::Visible);
                let p2: Vec<DVec2> = pts.iter().map(|p| frame.p2(*p)).collect();
                match (e.kind, circle) {
                    (EdgeKind::Line, _) => {
                        let (a, b) = (p2[0], p2[p2.len() - 1]);
                        if (a - b).length() >= 1e-3 {
                            out.edge_refs.push(Some(idx));
                            out.segments.push(seg(a, b, kind));
                        }
                    }
                    (EdgeKind::Circle, Some((center, _, radius))) => {
                        if let Some(mut arc) = arc_about(frame.p2(center), radius, &p2, kind) {
                            if arc.sweep_deg() > 1e-3 {
                                arc.edge = Some(idx);
                                out.arcs.push(arc);
                            }
                        }
                    }
                    _ => {
                        if let Some((center, normal, radius)) = edge_on {
                            if let Some((a, b)) =
                                edge_on_circle_segment(&frame, center, normal, radius, &pts)
                            {
                                if (a - b).length() >= 1e-3 {
                                    out.edge_refs.push(None);
                                    out.segments.push(seg(a, b, kind));
                                }
                                continue;
                            }
                        }
                        push_polyline(&mut out, &simplify_2d(&p2, 1e-4), kind)
                    }
                }
            }
        }
        for (snap, chains) in silhouettes(topo, &frame, (size * 1e-7).max(1e-7)) {
            for chain in chains {
                for (pts, hidden) in visibility_runs(&chain, &occ, &frame, step) {
                    if hidden && !keep_hidden {
                        continue;
                    }
                    let kind = kind_of(hidden, HlrLineKind::Silhouette);
                    let p2: Vec<DVec2> = pts.iter().map(|p| frame.p2(*p)).collect();
                    match snap {
                        Snap::Cylinder { .. } | Snap::Cone { .. } => {
                            let (a, b) = (p2[0], p2[p2.len() - 1]);
                            if (a - b).length() >= 1e-3 {
                                out.edge_refs.push(None);
                                out.segments.push(seg(a, b, kind));
                            }
                        }
                        Snap::Sphere { center, radius } => {
                            if let Some(arc) = arc_about(frame.p2(center), radius, &p2, kind) {
                                out.arcs.push(arc);
                            }
                        }
                        Snap::None => push_polyline(&mut out, &simplify_2d(&p2, 1e-4), kind),
                    }
                }
            }
        }
    }
    Some(out)
}

/// Face silinder/kerucut/bidang untuk fitur gambar ([`crate::hlr_sheet`]).
pub(crate) fn sheet_faces(shapes: &[&KernelShape]) -> Vec<SheetFace> {
    let mut out = Vec::new();
    for shape in shapes {
        if shape.body().is_none() {
            continue;
        }
        let topo = shape.topology();
        for face in &topo.faces {
            let boundary = || -> Vec<DVec3> {
                face.edges
                    .iter()
                    .filter_map(|&e| topo.edges.get(e))
                    .flat_map(|e| e.points.iter().copied())
                    .collect()
            };
            match face.surface {
                Some(FaceSurface::Cylinder {
                    origin,
                    axis,
                    radius,
                }) => out.push(SheetFace::Cylinder {
                    axis_pt: to_dv(origin),
                    axis_dir: to_dv(*axis),
                    radius,
                    boundary: boundary(),
                }),
                Some(FaceSurface::Cone { apex, axis, .. }) => out.push(SheetFace::Cone {
                    axis_pt: to_dv(apex),
                    axis_dir: to_dv(*axis),
                    boundary: boundary(),
                }),
                Some(FaceSurface::Plane { normal, .. }) => out.push(SheetFace::Plane {
                    normal: to_dv(*normal),
                    boundary: boundary(),
                }),
                _ => {}
            }
        }
    }
    out
}

/// Hook `crate::hlr`: satu tampak lembar gambar dari HLR hybrid.
pub(crate) fn exact_projected_view(
    shapes: &[&KernelShape],
    sketch_segments: &[(Vec3, Vec3)],
    view_kind: ProjectedViewKind,
    model_bbox: ([f32; 3], [f32; 3]),
    warnings: &mut Vec<String>,
) -> Option<ProjectedView> {
    let (_, right_vec, up_vec) = view_kind.camera_vectors();
    let started = Instant::now();
    let mut lines = exact_lines(shapes, right_vec, up_vec, true)?;
    if started.elapsed().as_secs_f32() > 10.0 {
        warnings.push(format!(
            "HLR_EXACT_SLOW: {} took {:.1} s",
            view_kind.title_en(),
            started.elapsed().as_secs_f32()
        ));
    }
    for (p1, p2) in sketch_segments {
        let a = [p1.dot(right_vec), p1.dot(up_vec)];
        let b = [p2.dot(right_vec), p2.dot(up_vec)];
        if (a[0] - b[0]).hypot(a[1] - b[1]) > 0.02 {
            lines.segments.push(HlrSegment2D {
                start: a,
                end: b,
                kind: HlrLineKind::Visible,
            });
            lines.edge_refs.push(None);
        }
    }
    let diag = (Vec3::from_array(model_bbox.1) - Vec3::from_array(model_bbox.0)).length();
    let (features, centerlines) =
        crate::hlr_sheet::exact_features(&sheet_faces(shapes), &lines, right_vec, up_vec, diag);
    let (bounds_min, bounds_max) = lines_bounds(&lines).unwrap_or(([0.0, 0.0], [100.0, 100.0]));
    Some(ProjectedView {
        kind: view_kind,
        title: view_kind.title_id(),
        bounds_min,
        bounds_max,
        segments: lines.segments,
        centerlines,
        features,
        width_mm: (model_bbox.1[0] - model_bbox.0[0]).abs(),
        height_mm: (model_bbox.1[2] - model_bbox.0[2]).abs(),
        depth_mm: (model_bbox.1[1] - model_bbox.0[1]).abs(),
        arcs: lines.arcs,
        edge_refs: lines.edge_refs,
        exact: true,
    })
}

/// `crate::hlr_exact::extract_exact_hlr` — konvensi sama dengan OCCT
/// (`gp_Ax2(asal, N, Vx)`): `view_dir` = normal bidang proyeksi (menuju
/// pemirsa), `up` = sumbu X gambar.
pub fn extract_exact_hlr(
    shape: &KernelShape,
    view_dir: (f64, f64, f64),
    up: (f64, f64, f64),
) -> anyhow::Result<ExactHlrView> {
    let dir = DVec3::new(view_dir.0, view_dir.1, view_dir.2);
    let x = DVec3::new(up.0, up.1, up.2);
    if dir.length() < 1e-9 || x.length() < 1e-9 {
        anyhow::bail!("arah pandang dan vektor atas HLR tidak boleh nol");
    }
    let n = dir.normalize();
    if n.cross(x.normalize()).length() < 1e-6 {
        anyhow::bail!("vektor atas HLR tidak boleh sejajar arah pandang");
    }
    let r = (x - n * x.dot(n)).normalize();
    let u = n.cross(r);
    let f = |v: DVec3| Vec3::new(v.x as f32, v.y as f32, v.z as f32);
    let Some(lines) = exact_lines(&[shape], f(r), f(u), true) else {
        anyhow::bail!("HLR gagal untuk geometri ini");
    };
    let kind = |k: HlrLineKind| match k {
        HlrLineKind::Hidden => ExactLineKind::HiddenSharp,
        HlrLineKind::Silhouette => ExactLineKind::VisibleOutline,
        _ => ExactLineKind::VisibleSharp,
    };
    let mut curves: Vec<ExactCurve2D> = lines
        .segments
        .iter()
        .map(|s| ExactCurve2D {
            kind: kind(s.kind),
            curve: ExactCurveType::Line,
            points: vec![
                (s.start[0] as f64, s.start[1] as f64),
                (s.end[0] as f64, s.end[1] as f64),
            ],
        })
        .collect();
    curves.extend(lines.arcs.iter().map(|a| ExactCurve2D {
        kind: kind(a.kind),
        curve: ExactCurveType::Circle,
        points: a
            .polyline(3.0)
            .into_iter()
            .map(|p| (p[0] as f64, p[1] as f64))
            .collect(),
    }));
    Ok(ExactHlrView { curves })
}
