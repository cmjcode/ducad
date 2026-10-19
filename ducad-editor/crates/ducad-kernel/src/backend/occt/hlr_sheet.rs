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
use opencascade::primitives::{Compound, EdgeType, Shape};

pub(crate) use crate::hlr_sheet::{lines_bounds, ExactLines};
use crate::hlr::{HlrGeometricFeature, HlrLineKind, HlrSegment2D};
use crate::hlr_sheet::{circumcenter3, fit_arc, push_polyline, seg, to_d, SheetFace};
use crate::picking::face::SurfaceKind;

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

/// Fitur geometris + garis sumbu sebuah tampak eksak (logika bersama di
/// [`crate::hlr_sheet::exact_features`]; di sini hanya face OCCT → [`SheetFace`]).
pub(crate) fn exact_features(
    shapes: &[&Shape],
    lines: &ExactLines,
    right: Vec3,
    up: Vec3,
    model_diag: f32,
) -> (Vec<HlrGeometricFeature>, Vec<HlrSegment2D>) {
    let mut faces = Vec::new();
    for shape in shapes {
        for face in crate::topo::ordered_faces(shape) {
            let boundary = || -> Vec<DVec3> {
                face.edges()
                    .flat_map(|e| e.approximation_segments().collect::<Vec<_>>())
                    .collect()
            };
            match SurfaceKind::from(face.surface_kind().as_str()) {
                SurfaceKind::Cylinder => {
                    let Some((axis_pt, axis_dir)) = face.cylinder_or_cone_axis() else {
                        continue;
                    };
                    let Some(radius) = face.cylinder_or_cone_radius() else {
                        continue;
                    };
                    faces.push(SheetFace::Cylinder {
                        axis_pt,
                        axis_dir,
                        radius,
                        boundary: boundary(),
                    });
                }
                SurfaceKind::Cone => {
                    let Some((axis_pt, axis_dir)) = face.cylinder_or_cone_axis() else {
                        continue;
                    };
                    faces.push(SheetFace::Cone {
                        axis_pt,
                        axis_dir,
                        boundary: boundary(),
                    });
                }
                SurfaceKind::Plane => faces.push(SheetFace::Plane {
                    normal: face.normal_at_center(),
                    boundary: boundary(),
                }),
                _ => {}
            }
        }
    }
    crate::hlr_sheet::exact_features(&faces, lines, right, up, model_diag)
}
