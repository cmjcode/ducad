//! HLR eksak untuk LEMBAR GAMBAR (P21.3): mengubah keluaran `HLRBRep_Algo`
//! menjadi segmen lurus + busur analitik [`HlrArc2D`], lalu menurunkan fitur
//! geometris (lingkaran, busur, chamfer, sisi silinder) dan garis sumbu
//! langsung dari B-rep — bukan dari mesh.
//!
//! Semua fungsi di sini `pub(crate)` dan TIDAK mengambil `lock_kernel()`:
//! pemanggilnya (`HlrExtractor`, `SectionExtractor`) sudah memegang guard.
//!
//! # Konvensi koordinat
//!
//! Titik 3D `p` jatuh di `(p·right, p·up)` pada bidang gambar, dengan titik
//! asal dunia sebagai titik asal gambar — sama dengan jalur mesh di
//! [`crate::hlr`]. `HLRAlgo_Projector` OCCT memandang dari sumbu +Z kerangka
//! proyeksinya ke arah −Z, dan binding kita mengisi `gp_Ax2(asal, N, Vx)`;
//! jadi `N = right × up` (menuju pemirsa) dan `Vx = right` menghasilkan
//! `X = right`, `Y = N × X = up`. Dikalibrasi lewat tes
//! `exact_sheet_view_matches_projection_convention`.

use glam::{DVec2, DVec3, Vec3};
use opencascade::primitives::{Compound, EdgeType, Face, Shape};

use crate::hlr::{HlrArc2D, HlrGeometricFeature, HlrLineKind, HlrSegment2D};
use crate::picking::face::SurfaceKind;

/// Garis lurus, busur, dan rujukan edge asal untuk satu tampak.
#[derive(Debug, Default)]
pub(crate) struct ExactLines {
    pub segments: Vec<HlrSegment2D>,
    /// Sejajar `segments`: indeks edge topologi asal (lihat [`source_edges`]).
    pub edge_refs: Vec<Option<u32>>,
    pub arcs: Vec<HlrArc2D>,
}

/// Edge sumber B-rep yang sudah diklasifikasi, dalam urutan
/// `topo::unique_edges` per shape (indeks berlanjut antar shape).
pub(crate) enum SourceEdge {
    Line {
        a: DVec3,
        b: DVec3,
    },
    Circle {
        center: DVec3,
        normal: DVec3,
        radius: f64,
    },
    Other,
}

fn to_d(v: Vec3) -> DVec3 {
    DVec3::new(v.x as f64, v.y as f64, v.z as f64)
}

/// Pusat lingkaran luar segitiga 3D `a`, `b`, `c` beserta normal bidangnya.
fn circumcenter3(a: DVec3, b: DVec3, c: DVec3) -> Option<(DVec3, DVec3)> {
    let ab = b - a;
    let ac = c - a;
    let n = ab.cross(ac);
    let n2 = n.length_squared();
    if n2 < 1e-18 {
        return None;
    }
    let center =
        a + (n.cross(ab) * ac.length_squared() + ac.cross(n) * ab.length_squared()) / (2.0 * n2);
    Some((center, n.normalize()))
}

fn circumcenter2(a: DVec2, b: DVec2, c: DVec2) -> Option<DVec2> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1e-12 {
        return None;
    }
    let a2 = a.length_squared();
    let b2 = b.length_squared();
    let c2 = c.length_squared();
    Some(DVec2::new(
        (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d,
        (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d,
    ))
}

/// Klasifikasi seluruh edge unik `shapes` — sumber `edge_refs`.
pub(crate) fn source_edges(shapes: &[&Shape]) -> Vec<SourceEdge> {
    let mut out = Vec::new();
    for shape in shapes {
        for edge in crate::topo::unique_edges(shape) {
            let item = match edge.edge_type() {
                EdgeType::Line => SourceEdge::Line {
                    a: edge.start_point(),
                    b: edge.end_point(),
                },
                EdgeType::Circle => {
                    let pts: Vec<DVec3> = edge.approximation_segments().collect();
                    let n = pts.len();
                    if n >= 3 {
                        match circumcenter3(pts[0], pts[n / 3], pts[(2 * n) / 3]) {
                            Some((center, normal)) => SourceEdge::Circle {
                                center,
                                normal,
                                radius: (pts[0] - center).length(),
                            },
                            None => SourceEdge::Other,
                        }
                    } else {
                        SourceEdge::Other
                    }
                }
                _ => SourceEdge::Other,
            };
            out.push(item);
        }
    }
    out
}

/// Jalankan HLR eksak atas `shapes` dan kembalikan garis/busur 2D.
///
/// `None` bila OCCT gagal — pemanggil jatuh ke HLR mesh.
/// `sources` (bila ada) dipakai mengisi rujukan edge asal.
pub(crate) fn exact_lines(
    shapes: &[&Shape],
    right: Vec3,
    up: Vec3,
    keep_hidden: bool,
    sources: Option<&[SourceEdge]>,
) -> Option<ExactLines> {
    if shapes.is_empty() {
        return None;
    }
    let r = to_d(right).normalize_or_zero();
    let u = to_d(up).normalize_or_zero();
    let toward = r.cross(u);
    if toward.length_squared() < 0.5 {
        return None;
    }

    let compound: Shape;
    let target: &Shape = if shapes.len() == 1 {
        shapes[0]
    } else {
        compound = Compound::from_shapes(shapes.iter().copied()).into();
        &compound
    };

    let (vis_sharp, vis_out, hid_sharp, hid_out) = target.hidden_line_removal(toward, r)?;

    // Proyeksi edge sumber, dihitung sekali.
    struct ProjLine {
        idx: u32,
        a: DVec2,
        b: DVec2,
    }
    struct ProjCircle {
        idx: u32,
        c: DVec2,
        r: f64,
        depth: f64,
    }
    let mut proj_lines = Vec::new();
    let mut proj_circles = Vec::new();
    if let Some(src) = sources {
        for (i, e) in src.iter().enumerate() {
            match e {
                SourceEdge::Line { a, b } => {
                    let a2 = DVec2::new(a.dot(r), a.dot(u));
                    let b2 = DVec2::new(b.dot(r), b.dot(u));
                    if (a2 - b2).length() > 1e-4 {
                        proj_lines.push(ProjLine {
                            idx: i as u32,
                            a: a2,
                            b: b2,
                        });
                    }
                }
                SourceEdge::Circle {
                    center,
                    normal,
                    radius,
                } => {
                    if normal.dot(toward).abs() > 0.9999 {
                        proj_circles.push(ProjCircle {
                            idx: i as u32,
                            c: DVec2::new(center.dot(r), center.dot(u)),
                            r: *radius,
                            depth: center.dot(toward),
                        });
                    }
                }
                SourceEdge::Other => {}
            }
        }
    }
    let line_ref = |p: DVec2, q: DVec2| -> Option<u32> {
        for l in &proj_lines {
            let d = l.b - l.a;
            let len2 = d.length_squared();
            let on = |x: DVec2| {
                let t = (x - l.a).dot(d) / len2;
                let foot = l.a + d * t;
                (x - foot).length() < 2e-3 && (-1e-3..=1.0 + 1e-3).contains(&t)
            };
            if on(p) && on(q) {
                return Some(l.idx);
            }
        }
        None
    };
    let circle_ref = |c: DVec2, rad: f64, hidden: bool| -> Option<u32> {
        let tol = 2e-3 * rad.max(1.0);
        let mut best: Option<(&ProjCircle, f64)> = None;
        for pc in &proj_circles {
            if (pc.c - c).length() < tol && (pc.r - rad).abs() < tol {
                // Garis tampak → lingkaran terdekat ke pemirsa; tersembunyi →
                // yang terjauh.
                let score = if hidden { -pc.depth } else { pc.depth };
                if best.map(|(_, s)| score > s).unwrap_or(true) {
                    best = Some((pc, score));
                }
            }
        }
        best.map(|(pc, _)| pc.idx)
    };

    let mut out = ExactLines::default();
    for (group, kind) in [
        (&vis_sharp, HlrLineKind::Visible),
        (&vis_out, HlrLineKind::Silhouette),
        (&hid_sharp, HlrLineKind::Hidden),
        (&hid_out, HlrLineKind::Hidden),
    ] {
        let hidden = kind == HlrLineKind::Hidden;
        if hidden && !keep_hidden {
            continue;
        }
        for edge in group.edges() {
            let pts: Vec<DVec2> = edge
                .approximation_segments()
                .map(|p| DVec2::new(p.x, p.y))
                .collect();
            if pts.len() < 2 {
                continue;
            }
            match edge.edge_type() {
                EdgeType::Line => {
                    let (a, b) = (pts[0], pts[pts.len() - 1]);
                    if (a - b).length() < 1e-3 {
                        continue;
                    }
                    out.edge_refs.push(line_ref(a, b));
                    out.segments.push(seg(a, b, kind));
                }
                EdgeType::Circle => {
                    if let Some(mut arc) = fit_arc(&pts, kind) {
                        arc.edge = circle_ref(
                            DVec2::new(arc.center[0] as f64, arc.center[1] as f64),
                            arc.radius as f64,
                            hidden,
                        );
                        out.arcs.push(arc);
                    } else {
                        push_polyline(&mut out, &pts, kind);
                    }
                }
                _ => push_polyline(&mut out, &pts, kind),
            }
        }
    }
    Some(out)
}

fn seg(a: DVec2, b: DVec2, kind: HlrLineKind) -> HlrSegment2D {
    HlrSegment2D {
        start: [a.x as f32, a.y as f32],
        end: [b.x as f32, b.y as f32],
        kind,
    }
}

fn push_polyline(out: &mut ExactLines, pts: &[DVec2], kind: HlrLineKind) {
    for w in pts.windows(2) {
        if (w[0] - w[1]).length() > 1e-3 {
            out.edge_refs.push(None);
            out.segments.push(seg(w[0], w[1], kind));
        }
    }
}

/// Cocokkan titik sampel sebuah edge lingkaran ke busur 2D. Busur selalu
/// berlawanan arah jarum jam dari `start_deg` ke `end_deg`.
fn fit_arc(pts: &[DVec2], kind: HlrLineKind) -> Option<HlrArc2D> {
    let n = pts.len();
    if n < 3 {
        return None;
    }
    let center = circumcenter2(pts[0], pts[n / 3], pts[(2 * n) / 3])
        .or_else(|| circumcenter2(pts[0], pts[n / 2], pts[n - 1]))?;
    let radius = (pts[0] - center).length();
    if radius < 1e-3 {
        return None;
    }
    // Sapuan bertanda: jumlah selisih sudut antar sampel berurutan — tahan
    // untuk busur > 180°.
    let ang = |p: DVec2| (p.y - center.y).atan2(p.x - center.x);
    let mut total = 0.0f64;
    let mut prev = ang(pts[0]);
    for p in &pts[1..] {
        let a = ang(*p);
        let mut d = a - prev;
        while d > std::f64::consts::PI {
            d -= std::f64::consts::TAU;
        }
        while d < -std::f64::consts::PI {
            d += std::f64::consts::TAU;
        }
        total += d;
        prev = a;
    }
    let sweep = total.abs().to_degrees();
    let (start_deg, end_deg) = if sweep >= 359.9 {
        (0.0, 360.0)
    } else {
        let first = ang(pts[0]).to_degrees();
        let last = ang(pts[n - 1]).to_degrees();
        let start = if total >= 0.0 { first } else { last };
        let start = start.rem_euclid(360.0);
        (start, start + sweep)
    };
    Some(HlrArc2D {
        center: [center.x as f32, center.y as f32],
        radius: radius as f32,
        start_deg: start_deg as f32,
        end_deg: end_deg as f32,
        kind,
        edge: None,
    })
}

/// Cakupan sudut gabungan (derajat, maks 360) dari rentang busur yang boleh
/// tumpang-tindih — dua busur 180° yang berimpit BUKAN lingkaran penuh.
fn coverage_deg(spans: &[(f32, f32)]) -> f32 {
    let mut parts: Vec<(f32, f32)> = Vec::new();
    for (start, end) in spans {
        let sweep = (end - start).clamp(0.0, 360.0);
        if sweep >= 359.9 {
            return 360.0;
        }
        let s = start.rem_euclid(360.0);
        let e = s + sweep;
        if e > 360.0 {
            parts.push((s, 360.0));
            parts.push((0.0, e - 360.0));
        } else {
            parts.push((s, e));
        }
    }
    parts.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut total = 0.0;
    let mut cursor = 0.0f32;
    for (s, e) in parts {
        let s = s.max(cursor);
        if e > s {
            total += e - s;
            cursor = e;
        }
    }
    total
}

fn is_visible(kind: HlrLineKind) -> bool {
    matches!(kind, HlrLineKind::Visible | HlrLineKind::Silhouette)
}

/// Apakah ada segmen tampak yang berimpit dengan garis `a`–`b`.
fn has_visible_line(lines: &ExactLines, a: DVec2, b: DVec2) -> bool {
    let d = b - a;
    let len = d.length();
    if len < 1e-6 {
        return false;
    }
    let dir = d / len;
    let tol = 0.02;
    lines.segments.iter().any(|s| {
        if !is_visible(s.kind) {
            return false;
        }
        let p = DVec2::new(s.start[0] as f64, s.start[1] as f64);
        let q = DVec2::new(s.end[0] as f64, s.end[1] as f64);
        let off = |x: DVec2| ((x - a) - dir * (x - a).dot(dir)).length();
        if off(p) > tol || off(q) > tol {
            return false;
        }
        let (t0, t1) = ((p - a).dot(dir), (q - a).dot(dir));
        let (lo, hi) = (t0.min(t1), t0.max(t1));
        // Tumpang-tindih nyata dengan rentang [0, len].
        hi.min(len) - lo.max(0.0) > 0.05_f64.min(len * 0.5)
    })
}

struct FaceSamples {
    /// (koordinat aksial, jarak radial, sudut keliling) tiap titik tepi face.
    pts: Vec<(f64, f64, f64)>,
}

fn sample_face_about_axis(face: &Face, origin: DVec3, dir: DVec3) -> FaceSamples {
    // Basis tegak lurus sumbu.
    let helper = if dir.x.abs() < 0.9 {
        DVec3::X
    } else {
        DVec3::Y
    };
    let e1 = dir.cross(helper).normalize();
    let e2 = dir.cross(e1);
    let mut pts = Vec::new();
    for edge in face.edges() {
        for p in edge.approximation_segments() {
            let rel = p - origin;
            let a = rel.dot(dir);
            let radial = rel - dir * a;
            pts.push((a, radial.length(), radial.dot(e2).atan2(radial.dot(e1))));
        }
    }
    FaceSamples { pts }
}

impl FaceSamples {
    /// Cakupan sudut keliling (derajat) = 360 − celah terbesar.
    fn coverage_deg(&self) -> f64 {
        let mut angles: Vec<f64> = self.pts.iter().map(|p| p.2.to_degrees()).collect();
        if angles.len() < 2 {
            return 0.0;
        }
        angles.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut gap = angles[0] + 360.0 - angles[angles.len() - 1];
        for w in angles.windows(2) {
            gap = gap.max(w[1] - w[0]);
        }
        360.0 - gap
    }

    fn axial_range(&self) -> Option<(f64, f64)> {
        let lo = self.pts.iter().map(|p| p.0).fold(f64::MAX, f64::min);
        let hi = self.pts.iter().map(|p| p.0).fold(f64::MIN, f64::max);
        (hi - lo > 1e-4).then_some((lo, hi))
    }

    /// Jarak radial rata-rata titik di dekat koordinat aksial `a`.
    fn radius_at(&self, a: f64) -> f64 {
        let near: Vec<f64> = self
            .pts
            .iter()
            .filter(|p| (p.0 - a).abs() < 1e-3)
            .map(|p| p.1)
            .collect();
        if near.is_empty() {
            0.0
        } else {
            near.iter().sum::<f64>() / near.len() as f64
        }
    }
}

/// Fitur geometris + garis sumbu sebuah tampak eksak.
pub(crate) fn exact_features(
    shapes: &[&Shape],
    lines: &ExactLines,
    right: Vec3,
    up: Vec3,
    model_diag: f32,
) -> (Vec<HlrGeometricFeature>, Vec<HlrSegment2D>) {
    let r = to_d(right).normalize_or_zero();
    let u = to_d(up).normalize_or_zero();
    let toward = r.cross(u);
    let p2 = |p: DVec3| DVec2::new(p.dot(r), p.dot(u));
    let f2 = |p: DVec2| [p.x as f32, p.y as f32];

    let mut features: Vec<HlrGeometricFeature> = Vec::new();
    let mut centerlines: Vec<HlrSegment2D> = Vec::new();

    // 1. Lingkaran/busur: kelompokkan busur HLR menurut (pusat, radius).
    struct Group {
        center: [f32; 2],
        radius: f32,
        /// Rentang sudut (derajat) busur tampak / semua busur.
        vis: Vec<(f32, f32)>,
        all: Vec<(f32, f32)>,
        best: Option<HlrArc2D>,
        edge: Option<u32>,
    }
    let mut groups: Vec<Group> = Vec::new();
    for arc in &lines.arcs {
        let tol = 2e-3 * arc.radius.max(1.0);
        let pos = groups.iter().position(|g| {
            (g.center[0] - arc.center[0]).hypot(g.center[1] - arc.center[1]) < tol
                && (g.radius - arc.radius).abs() < tol
        });
        let g = match pos {
            Some(i) => &mut groups[i],
            None => {
                groups.push(Group {
                    center: arc.center,
                    radius: arc.radius,
                    vis: Vec::new(),
                    all: Vec::new(),
                    best: None,
                    edge: None,
                });
                let last = groups.len() - 1;
                &mut groups[last]
            }
        };
        g.all.push((arc.start_deg, arc.end_deg));
        if is_visible(arc.kind) {
            g.vis.push((arc.start_deg, arc.end_deg));
            if g.best
                .as_ref()
                .map(|b| arc.sweep_deg() > b.sweep_deg())
                .unwrap_or(true)
            {
                g.best = Some(arc.clone());
            }
            if g.edge.is_none() {
                g.edge = arc.edge;
            }
        }
    }
    // Tanda pusat: satu silang per pusat, sebesar lingkaran terbesarnya.
    let mut marks: Vec<([f32; 2], f32)> = Vec::new();
    for g in &groups {
        if g.radius < 0.3 {
            continue;
        }
        // Lingkaran penuh yang sebagian tertutup (mis. lubang baut di balik
        // leher) tetap satu fitur lingkaran selama ≥ 90° di antaranya tampak.
        let (vis_sweep, all_sweep) = (coverage_deg(&g.vis), coverage_deg(&g.all));
        if vis_sweep >= 359.0 || (all_sweep >= 359.0 && vis_sweep >= 90.0) {
            features.push(HlrGeometricFeature::Circle {
                center: g.center,
                radius: g.radius,
                edge: g.edge,
            });
        } else if let Some(best) = &g.best {
            if best.sweep_deg() >= 5.0 {
                features.push(HlrGeometricFeature::Arc {
                    center: g.center,
                    radius: g.radius,
                    start_angle: best.start_deg.to_radians(),
                    end_angle: best.end_deg.to_radians(),
                    edge: g.edge,
                });
            }
        }
        if all_sweep >= 359.0 {
            match marks
                .iter_mut()
                .find(|(c, _)| (c[0] - g.center[0]).hypot(c[1] - g.center[1]) < 0.01)
            {
                Some(m) => m.1 = m.1.max(g.radius),
                None => marks.push((g.center, g.radius)),
            }
        }
    }
    for (c, rad) in &marks {
        let h = rad + 3.0;
        centerlines.push(HlrSegment2D {
            start: [c[0] - h, c[1]],
            end: [c[0] + h, c[1]],
            kind: HlrLineKind::Centerline,
        });
        centerlines.push(HlrSegment2D {
            start: [c[0], c[1] - h],
            end: [c[0], c[1] + h],
            kind: HlrLineKind::Centerline,
        });
    }

    // 2. Face silinder/kerucut/bidang yang tampak dari samping.
    let chamfer_limit = (model_diag as f64 * 0.12).max(6.0);
    // (titik pada sumbu 2D, arah 2D kanonik, rentang) untuk menggabung sumbu.
    let mut axes: Vec<(DVec2, DVec2, f64, f64)> = Vec::new();
    let mut chamfer_points: Vec<DVec2> = Vec::new();

    for shape in shapes {
        for face in crate::topo::ordered_faces(shape) {
            let kind = SurfaceKind::from(face.surface_kind().as_str());
            match kind {
                SurfaceKind::Cylinder | SurfaceKind::Cone => {
                    let Some((axis_pt, axis_dir)) = face.cylinder_or_cone_axis() else {
                        continue;
                    };
                    let dir = axis_dir.normalize_or_zero();
                    if dir.dot(toward).abs() > 1e-3 {
                        continue;
                    }
                    let samples = sample_face_about_axis(&face, axis_pt, dir);
                    let Some((a0, a1)) = samples.axial_range() else {
                        continue;
                    };
                    if samples.coverage_deg() < 170.0 {
                        continue; // fillet/busur parsial, bukan poros/lubang
                    }
                    let side = dir.cross(toward).normalize_or_zero();
                    let q0 = axis_pt + dir * a0;
                    let q1 = axis_pt + dir * a1;
                    if kind == SurfaceKind::Cylinder {
                        let Some(radius) = face.cylinder_or_cone_radius() else {
                            continue;
                        };
                        let visible = [1.0, -1.0].iter().any(|s| {
                            has_visible_line(
                                lines,
                                p2(q0 + side * radius * *s),
                                p2(q1 + side * radius * *s),
                            )
                        });
                        let (s2, e2) = (p2(q0), p2(q1));
                        let dup = features.iter().any(|f| match f {
                            HlrGeometricFeature::CylinderSide {
                                axis_start,
                                axis_end,
                                radius: r0,
                                ..
                            } => {
                                (*r0 as f64 - radius).abs() < 1e-3
                                    && (DVec2::new(axis_start[0] as f64, axis_start[1] as f64) - s2)
                                        .length()
                                        < 1e-2
                                    && (DVec2::new(axis_end[0] as f64, axis_end[1] as f64) - e2)
                                        .length()
                                        < 1e-2
                            }
                            _ => false,
                        });
                        if !dup {
                            features.push(HlrGeometricFeature::CylinderSide {
                                axis_start: f2(s2),
                                axis_end: f2(e2),
                                radius: radius as f32,
                                visible,
                            });
                        }
                        add_axis(&mut axes, s2, e2);
                    } else {
                        let (r0, r1) = (samples.radius_at(a0), samples.radius_at(a1));
                        let length = a1 - a0;
                        add_axis(&mut axes, p2(q0), p2(q1));
                        if length > chamfer_limit || (r0 - r1).abs() < 1e-4 {
                            continue;
                        }
                        let angle = (r1 - r0).abs().atan2(length).to_degrees();
                        for s in [1.0, -1.0] {
                            let a = p2(q0 + side * r0 * s);
                            let b = p2(q1 + side * r1 * s);
                            if has_visible_line(lines, a, b) {
                                push_chamfer(
                                    &mut features,
                                    &mut chamfer_points,
                                    a,
                                    b,
                                    length,
                                    angle,
                                );
                                break;
                            }
                        }
                    }
                }
                SurfaceKind::Plane => {
                    // Bidang miring yang tampak sebagai satu garis pendek = chamfer.
                    let n3 = face.normal_at_center().normalize_or_zero();
                    if n3.dot(toward).abs() > 1e-3 {
                        continue;
                    }
                    let n2 = DVec2::new(n3.dot(r), n3.dot(u));
                    if n2.x.abs() < 0.17 || n2.y.abs() < 0.17 {
                        continue; // sejajar sumbu gambar (±10°)
                    }
                    let along = DVec2::new(-n2.y, n2.x).normalize();
                    let mut lo = f64::MAX;
                    let mut hi = f64::MIN;
                    let mut base = DVec2::ZERO;
                    let mut first = true;
                    for edge in face.edges() {
                        for p in edge.approximation_segments() {
                            let q = p2(p);
                            if first {
                                base = q;
                                first = false;
                            }
                            let t = (q - base).dot(along);
                            lo = lo.min(t);
                            hi = hi.max(t);
                        }
                    }
                    if first || hi - lo < 1e-3 {
                        continue;
                    }
                    let (a, b) = (base + along * lo, base + along * hi);
                    let (dx, dy) = ((b.x - a.x).abs(), (b.y - a.y).abs());
                    let (length, angle) = if dx >= dy {
                        (dx, dy.atan2(dx).to_degrees())
                    } else {
                        (dy, dx.atan2(dy).to_degrees())
                    };
                    if length <= chamfer_limit && has_visible_line(lines, a, b) {
                        push_chamfer(&mut features, &mut chamfer_points, a, b, length, angle);
                    }
                }
                _ => {}
            }
        }
    }

    // Garis sumbu poros/lubang, dilebihkan 3 mm di kedua ujung.
    for (pt, dir, lo, hi) in &axes {
        let a = *pt + *dir * (*lo - 3.0);
        let b = *pt + *dir * (*hi + 3.0);
        centerlines.push(HlrSegment2D {
            start: f2(a),
            end: f2(b),
            kind: HlrLineKind::Centerline,
        });
    }

    (features, centerlines)
}

fn push_chamfer(
    features: &mut Vec<HlrGeometricFeature>,
    chamfer_points: &mut Vec<DVec2>,
    a: DVec2,
    b: DVec2,
    length: f64,
    angle: f64,
) {
    chamfer_points.push(a);
    chamfer_points.push(b);
    features.push(HlrGeometricFeature::Chamfer {
        start: [a.x as f32, a.y as f32],
        end: [b.x as f32, b.y as f32],
        length: length as f32,
        angle_deg: angle as f32,
    });
}

/// Tambahkan sumbu ke daftar; sumbu segaris digabung rentangnya.
fn add_axis(axes: &mut Vec<(DVec2, DVec2, f64, f64)>, s: DVec2, e: DVec2) {
    let d = e - s;
    let len = d.length();
    if len < 1e-6 {
        return;
    }
    let mut dir = d / len;
    if dir.x < -1e-9 || (dir.x.abs() <= 1e-9 && dir.y < 0.0) {
        dir = -dir;
    }
    for (pt, adir, lo, hi) in axes.iter_mut() {
        if (adir.x * dir.y - adir.y * dir.x).abs() > 1e-6 {
            continue;
        }
        let rel = s - *pt;
        if (rel - *adir * rel.dot(*adir)).length() > 1e-3 {
            continue;
        }
        let (t0, t1) = ((s - *pt).dot(*adir), (e - *pt).dot(*adir));
        *lo = lo.min(t0.min(t1));
        *hi = hi.max(t0.max(t1));
        return;
    }
    let (t0, t1) = (0.0_f64, (e - s).dot(dir));
    axes.push((s, dir, t0.min(t1), t0.max(t1)));
}

/// Kotak pembatas 2D seluruh garis + busur (tanpa garis sumbu).
pub(crate) fn lines_bounds(lines: &ExactLines) -> Option<([f32; 2], [f32; 2])> {
    let mut lo = [f32::MAX; 2];
    let mut hi = [f32::MIN; 2];
    let mut add = |p: [f32; 2]| {
        lo = [lo[0].min(p[0]), lo[1].min(p[1])];
        hi = [hi[0].max(p[0]), hi[1].max(p[1])];
    };
    for s in &lines.segments {
        add(s.start);
        add(s.end);
    }
    for a in &lines.arcs {
        let (b0, b1) = a.bounds();
        add(b0);
        add(b1);
    }
    (lo[0] <= hi[0]).then_some((lo, hi))
}
