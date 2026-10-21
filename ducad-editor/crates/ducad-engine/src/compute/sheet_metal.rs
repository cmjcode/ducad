//! Sheet metal (P19, Paritas 6C): solid terlipat dan pola datar dari
//! `ducad_core::SheetMetalModel`.
//!
//! Profil dasar boleh berisi busur/Bezier (dicacah jadi poligon) dan lubang
//! (jadi `cutouts`, dibentuk sekali jalan lewat `Profile::WithHoles`). Relief
//! memotong pelat dasar + zona tekuk pertama. Flange di ujung flange lain
//! memperpanjang rantai segmen flange itu. Batasan: flange selalu selebar
//! sisinya; sisi lengkung tidak bisa diberi flange.

use ducad_core::{Cutout, Flange, SectionSegment, SheetMetalModel};
use ducad_kernel::{EdgeInfo, KernelShape, Profile, ProfileSegment};

use super::finish;
use crate::error::{OpError, OpResult};
use crate::model::BodyGeometry;
use crate::plane::PlaneFrame;

type V3 = [f64; 3];

fn axpy(a: V3, x: V3, k: f64) -> V3 {
    [a[0] + x[0] * k, a[1] + x[1] * k, a[2] + x[2] * k]
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Titik bidang `(u, v)` pada ketinggian `h` di atas bidang sketsa.
pub fn to_world(frame: &PlaneFrame, p: [f64; 2], h: f64) -> V3 {
    let on_plane = axpy(axpy(frame.origin, frame.u_axis, p[0]), frame.v_axis, p[1]);
    axpy(on_plane, frame.normal, h)
}

/// Titik per radian saat mencacah busur (≈ 48 per lingkaran penuh).
const ARC_STEPS_PER_RAD: f64 = 48.0 / std::f64::consts::TAU;

/// Titik busur 3-titik dari setelah `start` sampai `end` (inklusif).
fn arc_points(start: [f64; 2], via: [f64; 2], end: [f64; 2]) -> Vec<[f64; 2]> {
    let (ax, ay) = (start[0], start[1]);
    let (bx, by) = (via[0], via[1]);
    let (cx, cy) = (end[0], end[1]);
    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-12 {
        return vec![end];
    }
    let sq = |x: f64, y: f64| x * x + y * y;
    let ux = (sq(ax, ay) * (by - cy) + sq(bx, by) * (cy - ay) + sq(cx, cy) * (ay - by)) / d;
    let uy = (sq(ax, ay) * (cx - bx) + sq(bx, by) * (ax - cx) + sq(cx, cy) * (bx - ax)) / d;
    let r = sq(ax - ux, ay - uy).sqrt();
    let ang = |p: [f64; 2]| (p[1] - uy).atan2(p[0] - ux);
    let tau = std::f64::consts::TAU;
    let ccw = |from: f64, to: f64| (to - from).rem_euclid(tau);
    let (a0, a1, a2) = (ang(start), ang(via), ang(end));
    // Arah yang melewati `via`.
    let sweep = if ccw(a0, a1) <= ccw(a0, a2) {
        ccw(a0, a2)
    } else {
        -ccw(a2, a0)
    };
    let n = ((sweep.abs() * ARC_STEPS_PER_RAD).ceil() as usize).clamp(2, 256);
    (1..=n)
        .map(|i| {
            if i == n {
                end
            } else {
                let t = a0 + sweep * i as f64 / n as f64;
                [ux + r * t.cos(), uy + r * t.sin()]
            }
        })
        .collect()
}

fn bezier_points(p0: [f64; 2], c1: [f64; 2], c2: [f64; 2], p3: [f64; 2]) -> Vec<[f64; 2]> {
    const N: usize = 16;
    (1..=N)
        .map(|i| {
            let t = i as f64 / N as f64;
            let m = 1.0 - t;
            let w = [m * m * m, 3.0 * m * m * t, 3.0 * m * t * t, t * t * t];
            [
                w[0] * p0[0] + w[1] * c1[0] + w[2] * c2[0] + w[3] * p3[0],
                w[0] * p0[1] + w[1] * c1[1] + w[2] * c2[1] + w[3] * p3[1],
            ]
        })
        .collect()
}

/// Hapus titik kembar dan titik segaris (sisi lurus tetap satu sisi).
fn simplify(points: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    let mut pts: Vec<[f64; 2]> = Vec::with_capacity(points.len());
    for p in points {
        if pts
            .last()
            .is_none_or(|q| (p[0] - q[0]).hypot(p[1] - q[1]) > 1e-9)
        {
            pts.push(p);
        }
    }
    while pts.len() > 1 {
        let (f, l) = (pts[0], pts[pts.len() - 1]);
        if (f[0] - l[0]).hypot(f[1] - l[1]) > 1e-9 {
            break;
        }
        pts.pop();
    }
    let mut changed = true;
    while changed && pts.len() > 3 {
        changed = false;
        let n = pts.len();
        for i in 0..n {
            let (a, b, c) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
            let cross = (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]);
            let dot = (b[0] - a[0]) * (c[0] - b[0]) + (b[1] - a[1]) * (c[1] - b[1]);
            let scale = (b[0] - a[0]).hypot(b[1] - a[1]) * (c[0] - b[0]).hypot(c[1] - b[1]);
            if cross.abs() <= 1e-9 * scale.max(1e-12) && dot > 0.0 {
                pts.remove(i);
                changed = true;
                break;
            }
        }
    }
    pts
}

/// Poligon satu loop profil (tanpa lubang), berlawanan arah jarum jam.
fn loop_polygon(profile: &Profile) -> OpResult<Vec<[f64; 2]>> {
    let mut pts = match profile {
        Profile::Circle { center, radius } => Cutout::Circle {
            center: [center.0, center.1],
            radius: *radius,
        }
        .polygon(),
        Profile::Ellipse {
            center,
            radius_x,
            radius_y,
        } => (0..64)
            .map(|i| {
                let t = std::f64::consts::TAU * i as f64 / 64.0;
                [center.0 + radius_x * t.cos(), center.1 + radius_y * t.sin()]
            })
            .collect(),
        Profile::Loop(segments) => {
            let mut pts = Vec::new();
            for seg in segments {
                match *seg {
                    ProfileSegment::Line { start, end } => {
                        if pts.is_empty() {
                            pts.push([start.0, start.1]);
                        }
                        pts.push([end.0, end.1]);
                    }
                    ProfileSegment::Arc { start, via, end } => {
                        if pts.is_empty() {
                            pts.push([start.0, start.1]);
                        }
                        pts.extend(arc_points(
                            [start.0, start.1],
                            [via.0, via.1],
                            [end.0, end.1],
                        ));
                    }
                    ProfileSegment::Bezier { start, c1, c2, end } => {
                        if pts.is_empty() {
                            pts.push([start.0, start.1]);
                        }
                        pts.extend(bezier_points(
                            [start.0, start.1],
                            [c1.0, c1.1],
                            [c2.0, c2.1],
                            [end.0, end.1],
                        ));
                    }
                }
            }
            pts
        }
        Profile::WithHoles { .. } => {
            return Err(OpError::invalid(
                "profil berlubang bersarang tidak didukung untuk sheet metal",
            ))
        }
    };
    if pts.iter().any(|p| !p[0].is_finite() || !p[1].is_finite()) {
        return Err(OpError::invalid("profil sheet metal berisi koordinat tak hingga/NaN"));
    }
    pts = simplify(pts);
    if pts.len() < 3 {
        return Err(OpError::invalid("profil sheet metal berluas nol"));
    }
    if ducad_core::sheet_metal::signed_area(&pts) < 0.0 {
        pts.reverse();
    }
    Ok(pts)
}

/// Poligon dasar (CCW) dan lubang dari profil sketsa. Busur/Bezier/elips
/// dicacah jadi poligon; lubang lingkaran tetap lingkaran sejati.
pub fn base_from_profile(profile: &Profile) -> OpResult<(Vec<[f64; 2]>, Vec<Cutout>)> {
    let (outer, holes): (&Profile, &[Profile]) = match profile {
        Profile::WithHoles { outer, holes } => (outer, holes),
        p => (p, &[]),
    };
    let outline = loop_polygon(outer)?;
    let cutouts = holes
        .iter()
        .map(|h| match h {
            Profile::Circle { center, radius } => Ok(Cutout::Circle {
                center: [center.0, center.1],
                radius: *radius,
            }),
            other => loop_polygon(other).map(|points| Cutout::Polygon { points }),
        })
        .collect::<OpResult<Vec<_>>>()?;
    Ok((outline, cutouts))
}

fn polygon_profile(points: &[[f64; 2]]) -> Profile {
    let n = points.len();
    Profile::Loop(
        (0..n)
            .map(|i| ProfileSegment::Line {
                start: (points[i][0], points[i][1]),
                end: (points[(i + 1) % n][0], points[(i + 1) % n][1]),
            })
            .collect(),
    )
}

fn cutout_profile(c: &Cutout) -> Profile {
    match c {
        Cutout::Circle { center, radius } => Profile::Circle {
            center: (center[0], center[1]),
            radius: *radius,
        },
        Cutout::Polygon { points } => polygon_profile(points),
    }
}

/// Pelat (poligon + lubang) setebal `thickness` di atas bidang `frame`.
fn plate(
    points: &[[f64; 2]],
    cutouts: &[Cutout],
    frame: &PlaneFrame,
    thickness: f64,
) -> OpResult<KernelShape> {
    let outer = polygon_profile(points);
    let profile = if cutouts.is_empty() {
        outer
    } else {
        Profile::WithHoles {
            outer: Box::new(outer),
            holes: cutouts.iter().map(cutout_profile).collect(),
        }
    };
    ducad_kernel::extrude_profile_on_plane(
        &profile,
        frame.origin,
        frame.u_axis,
        frame.v_axis,
        frame.normal,
        thickness,
    )
    .map_err(|e| OpError::kernel("SheetMetal", e))
}

/// Celah kecil supaya alat potong tidak berbagi bidang dengan body.
fn eps(model: &SheetMetalModel) -> f64 {
    (model.thickness * 0.01).max(1e-3)
}

/// Daerah 2D `subj ∪ … − clip` (aturan non-zero) → (batas luar CCW, lubang
/// tambahan). Titik hasil di-snap ke titik masukan terdekat supaya sisi
/// pelat tetap tepat sama dengan sisi model (flange menempel tanpa celah).
fn region_2d(
    subj: &[Vec<[f64; 2]>],
    clip: &[Vec<[f64; 2]>],
) -> OpResult<(Vec<[f64; 2]>, Vec<Cutout>)> {
    use i_overlay::core::fill_rule::FillRule;
    use i_overlay::core::overlay_rule::OverlayRule;
    use i_overlay::float::single::SingleFloatOverlay;
    let subj: Vec<Vec<[f64; 2]>> = subj.to_vec();
    let clip: Vec<Vec<[f64; 2]>> = clip.to_vec();
    let rule = if clip.is_empty() {
        OverlayRule::Union
    } else {
        OverlayRule::Difference
    };
    let shapes = subj.overlay(&clip, rule, FillRule::NonZero);
    let [shape] = shapes.as_slice() else {
        return Err(OpError::invalid(format!(
            "pelat sheet metal terpecah jadi {} bagian (relief/strip memotong pelat)",
            shapes.len()
        )));
    };
    let known: Vec<[f64; 2]> = subj.iter().chain(clip.iter()).flatten().copied().collect();
    let snap = |p: [f64; 2]| {
        known
            .iter()
            .copied()
            .find(|q| (q[0] - p[0]).hypot(q[1] - p[1]) < 1e-5)
            .unwrap_or(p)
    };
    let mut contours = shape.iter().map(|c| {
        let mut pts = simplify(c.iter().map(|p| snap(*p)).collect());
        if ducad_core::sheet_metal::signed_area(&pts) < 0.0 {
            pts.reverse();
        }
        pts
    });
    let outer = contours
        .next()
        .filter(|c| c.len() >= 3)
        .ok_or_else(|| OpError::invalid("pelat sheet metal kosong"))?;
    let holes = contours
        .filter(|c| c.len() >= 3)
        .map(|points| Cutout::Polygon { points })
        .collect();
    Ok((outer, holes))
}

/// Kotak pemotong zona tekuk pertama di kedua ujung flange ber-relief.
fn bend_relief_boxes(
    model: &SheetMetalModel,
    frame: &PlaneFrame,
    flange: &Flange,
) -> OpResult<Vec<KernelShape>> {
    let (Some((w, _, _)), Some(bb), Some((a, b, out))) = (
        model.relief_size(flange),
        model.first_bend_box(flange),
        model.edge_frame(flange.edge),
    ) else {
        return Ok(Vec::new());
    };
    let e = eps(model);
    let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
    let dir2 = [(b[0] - a[0]) / len, (b[1] - a[1]) / len];
    let out3 = axpy(axpy([0.0; 3], frame.u_axis, out[0]), frame.v_axis, out[1]);
    let dir3 = axpy(axpy([0.0; 3], frame.u_axis, dir2[0]), frame.v_axis, dir2[1]);
    // v = n × out → out × v = n: profil tangan-kanan terhadap normal.
    let v3 = cross(frame.normal, out3);
    let sign = if dot(dir3, v3) >= 0.0 { 1.0 } else { -1.0 };
    let origin = to_world(frame, a, bb[1] - e);
    let mut tools = Vec::new();
    for start in [0.0, len - w] {
        // Sisi dalam tepat di `w` (sebidang dengan takik pelat); sisi luar
        // melewati ujung flange.
        let (lo, hi) = if start == 0.0 {
            (-e, w)
        } else {
            (start, start + w + e)
        };
        let (l0, l1) = (lo * sign, hi * sign);
        let (v0, v1) = (l0.min(l1), l0.max(l1));
        let (s0, s1) = (-e, bb[2] + e);
        let rect = polygon_profile(&[[s0, v0], [s1, v0], [s1, v1], [s0, v1]]);
        tools.push(
            ducad_kernel::extrude_profile_on_plane(
                &rect,
                origin,
                out3,
                v3,
                frame.normal,
                bb[3] - bb[1] + 2.0 * e,
            )
            .map_err(|err| OpError::kernel("SheetMetal", err))?,
        );
    }
    Ok(tools)
}

/// Solid satu flange: penampang `(s, h)` di-extrude sepanjang sisi pelat.
fn flange_solid(
    model: &SheetMetalModel,
    frame: &PlaneFrame,
    flange: &Flange,
) -> OpResult<KernelShape> {
    let (a, b, out) = model.edge_frame(flange.edge).ok_or_else(|| {
        OpError::invalid(format!("sisi {} pelat dasar berpanjang nol", flange.edge))
    })?;
    let (a3, b3) = (to_world(frame, a, 0.0), to_world(frame, b, 0.0));
    let length = dot(axpy(b3, a3, -1.0), axpy(b3, a3, -1.0)).sqrt();
    let edge_dir = {
        let d = axpy(b3, a3, -1.0);
        [d[0] / length, d[1] / length, d[2] / length]
    };
    // Arah keluar di dunia (tanpa origin).
    let out3 = axpy(axpy([0.0; 3], frame.u_axis, out[0]), frame.v_axis, out[1]);
    let segments: Vec<ProfileSegment> = model
        .cross_section(flange)
        .into_iter()
        .map(|s| match s {
            SectionSegment::Line { start, end } => ProfileSegment::Line {
                start: (start[0], start[1]),
                end: (end[0], end[1]),
            },
            SectionSegment::Arc { start, via, end } => ProfileSegment::Arc {
                start: (start[0], start[1]),
                via: (via[0], via[1]),
                end: (end[0], end[1]),
            },
        })
        .collect();
    // Bidang penampang (s = keluar, h = normal pelat) di-extrude searah
    // `u × v` supaya orientasinya tangan-kanan: mulai dari ujung sisi yang
    // membuat arah itu menunjuk ke sepanjang sisi.
    let sweep = cross(out3, frame.normal);
    let (origin, normal) = if dot(sweep, edge_dir) > 0.0 {
        (a3, edge_dir)
    } else {
        (b3, [-edge_dir[0], -edge_dir[1], -edge_dir[2]])
    };
    ducad_kernel::extrude_profile_on_plane(
        &Profile::Loop(segments),
        origin,
        out3,
        frame.normal,
        normal,
        length,
    )
    .map_err(|e| OpError::kernel("SheetMetal", e))
}

/// Solid terlipat: pelat dasar (takik relief dipotong di 2D) ∪ flange
/// (zona tekuk relief dipotong sebelum digabung). Pengurangan 3D tidak
/// pernah menyentuh pelat berlubang — boolean truck rapuh di sana.
pub fn folded(model: &SheetMetalModel, frame: &PlaneFrame) -> OpResult<BodyGeometry> {
    model.validate().map_err(OpError::invalid)?;
    let notches: Vec<Vec<[f64; 2]>> =
        model.relief_base_notches().into_iter().map(|(_, p)| p).collect();
    let (outline, extra) = if notches.is_empty() {
        (model.outline.clone(), Vec::new())
    } else {
        region_2d(std::slice::from_ref(&model.outline), &notches)?
    };
    let mut cutouts = model.cutouts.clone();
    cutouts.extend(extra);
    let mut shape = plate(&outline, &cutouts, frame, model.thickness)?;
    for flange in &model.flanges {
        let mut solid = flange_solid(model, frame, flange)?;
        for tool in bend_relief_boxes(model, frame, flange)? {
            solid = ducad_kernel::subtract(&solid, &tool)
                .map_err(|e| OpError::kernel("SheetMetal", e))?;
        }
        shape =
            ducad_kernel::union(&shape, &solid).map_err(|e| OpError::kernel("SheetMetal", e))?;
    }
    finish("SheetMetal", shape)
}

/// Solid pola datar: daerah 2D (pelat dasar ∪ strip flange − relief)
/// di-extrude sekali beserta lubangnya, di bidang sketsa yang sama.
pub fn flat(model: &SheetMetalModel, frame: &PlaneFrame) -> OpResult<BodyGeometry> {
    let pattern = model.flat_pattern().map_err(OpError::invalid)?;
    if pattern.polygons.is_empty() {
        return Err(OpError::invalid("pola datar kosong"));
    }
    let polys: Vec<Vec<[f64; 2]>> = pattern
        .polygons
        .iter()
        .map(|p| {
            let mut p = p.clone();
            if ducad_core::sheet_metal::signed_area(&p) < 0.0 {
                p.reverse();
            }
            p
        })
        .collect();
    let (outline, extra) = if polys.len() == 1 && model.relief_polygons().is_empty() {
        (polys[0].clone(), Vec::new())
    } else {
        region_2d(&polys, &model.relief_polygons())?
    };
    let mut cutouts = model.cutouts.clone();
    cutouts.extend(extra);
    finish("FlatPattern", plate(&outline, &cutouts, frame, model.thickness)?)
}

/// Sasaran flange baru: sisi pelat dasar, atau ujung flange yang sudah ada
/// (indeks di `model.flanges`) — rantai segmennya diperpanjang.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeTarget {
    Base(usize),
    Tip(usize),
}

/// Cocokkan tepi terpilih dengan sisi bebas pelat dasar (muka bawah `h = 0`
/// atau atas `h = t`) atau dengan tepi ujung flange yang sudah ada.
pub fn match_edges(
    model: &SheetMetalModel,
    frame: &PlaneFrame,
    edges: &[EdgeInfo],
) -> OpResult<Vec<EdgeTarget>> {
    let n = model.outline.len();
    let near = |p: V3, q: V3| {
        let d = axpy(p, q, -1.0);
        dot(d, d).sqrt() < 1e-4
    };
    let same = |edge: &EdgeInfo, a: V3, b: V3| {
        (near(edge.start, a) && near(edge.end, b)) || (near(edge.start, b) && near(edge.end, a))
    };
    let mut out = Vec::new();
    for edge in edges {
        let base = (0..n).find(|&i| {
            [0.0, model.thickness].into_iter().any(|h| {
                let a = to_world(frame, model.outline[i], h);
                let b = to_world(frame, model.outline[(i + 1) % n], h);
                same(edge, a, b)
            })
        });
        let tip = || {
            model.flanges.iter().position(|f| {
                let Some((a, b, o)) = model.edge_frame(f.edge) else {
                    return false;
                };
                let (bottom, top) = model.section_tip(f);
                [bottom, top].into_iter().any(|p| {
                    let pa = to_world(frame, [a[0] + o[0] * p[0], a[1] + o[1] * p[0]], p[1]);
                    let pb = to_world(frame, [b[0] + o[0] * p[0], b[1] + o[1] * p[0]], p[1]);
                    same(edge, pa, pb)
                })
            })
        };
        let target = match base {
            Some(i) => {
                if let Some(f) = model.flanges.iter().find(|f| f.edge == i) {
                    return Err(OpError::invalid(format!(
                        "sisi {i} pelat dasar sudah punya flange '{}'",
                        f.id
                    ))
                    .with_hint("to continue that flange pick the edge at its free tip instead"));
                }
                EdgeTarget::Base(i)
            }
            None => match tip() {
                Some(k) => EdgeTarget::Tip(k),
                None => {
                    return Err(OpError::invalid(format!(
                        "tepi #{} bukan sisi lurus pelat dasar yang bebas maupun tepi ujung flange",
                        edge.index
                    ))
                    .with_hint("pick an outer straight edge of the base flange or the free tip edge of a flange; test the selector with query_geometry"));
                }
            },
        };
        if !out.contains(&target) {
            out.push(target);
        }
    }
    Ok(out)
}

/// Batas jumlah lubang satu op `perforate` (geometri tetap ringan).
pub const MAX_PERFORATION_HOLES: usize = 2000;

fn seg_dist(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy)
}

fn inside(p: [f64; 2], poly: &[[f64; 2]]) -> bool {
    let n = poly.len();
    let mut c = false;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            c = !c;
        }
    }
    c
}

fn clear_of(p: [f64; 2], poly: &[[f64; 2]], gap: f64) -> bool {
    let n = poly.len();
    !inside(p, poly) && (0..n).all(|i| seg_dist(p, poly[i], poly[(i + 1) % n]) >= gap)
}

/// Lubang perforasi (grid lurus atau selang-seling 60°) berdiameter `d`
/// berjarak pusat `pitch`, minimal `margin` dari tepi pelat, lubang lain,
/// dan takik relief. Error bila tak ada lubang muat atau melebihi batas.
pub fn perforation(
    model: &SheetMetalModel,
    d: f64,
    pitch: f64,
    margin: f64,
    stagger: bool,
) -> OpResult<Vec<Cutout>> {
    if !(d.is_finite() && d > 0.0) {
        return Err(OpError::invalid(format!("diameter perforasi harus > 0 (dapat {d})")));
    }
    if !(pitch.is_finite() && pitch > d) {
        return Err(OpError::invalid(format!(
            "pitch perforasi harus > diameter {d} (dapat {pitch})"
        )));
    }
    if !(margin.is_finite() && margin >= 0.0) {
        return Err(OpError::invalid(format!("margin perforasi harus >= 0 (dapat {margin})")));
    }
    let r = d / 2.0;
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for p in &model.outline {
        lo = [lo[0].min(p[0]), lo[1].min(p[1])];
        hi = [hi[0].max(p[0]), hi[1].max(p[1])];
    }
    let dy = if stagger { pitch * 3f64.sqrt() / 2.0 } else { pitch };
    let (w, h) = (hi[0] - lo[0] - 2.0 * (r + margin), hi[1] - lo[1] - 2.0 * (r + margin));
    if w < 0.0 || h < 0.0 {
        return Err(OpError::invalid("pelat terlalu kecil untuk perforasi dengan margin ini"));
    }
    let (nx, ny) = ((w / pitch).floor() as usize + 1, (h / dy).floor() as usize + 1);
    if nx.saturating_mul(ny) > MAX_PERFORATION_HOLES * 8 {
        return Err(OpError::invalid(format!(
            "perforasi terlalu rapat: ±{} lubang (batas {MAX_PERFORATION_HOLES})",
            nx.saturating_mul(ny)
        ))
        .with_hint("increase pitch or perforate a smaller sheet"));
    }
    // Pusatkan grid di kotak pembatas.
    let x0 = lo[0] + r + margin + (w - (nx - 1) as f64 * pitch) / 2.0;
    let y0 = lo[1] + r + margin + (h - (ny - 1) as f64 * dy) / 2.0;
    let avoid: Vec<Vec<[f64; 2]>> = model
        .cutouts
        .iter()
        .map(Cutout::polygon)
        .chain(model.relief_polygons())
        .collect();
    let n = model.outline.len();
    let mut holes = Vec::new();
    for j in 0..ny {
        let shift = if stagger && j % 2 == 1 { pitch / 2.0 } else { 0.0 };
        for i in 0..nx {
            let c = [x0 + shift + i as f64 * pitch, y0 + j as f64 * dy];
            let in_plate = inside(c, &model.outline)
                && (0..n).all(|k| {
                    seg_dist(c, model.outline[k], model.outline[(k + 1) % n]) >= r + margin
                });
            if in_plate && avoid.iter().all(|p| clear_of(c, p, r + margin)) {
                holes.push(Cutout::Circle {
                    center: c,
                    radius: r,
                });
            }
        }
    }
    if holes.is_empty() {
        return Err(OpError::invalid("tidak ada lubang perforasi yang muat di pelat")
            .with_hint("reduce margin / diameter or pitch"));
    }
    if holes.len() > MAX_PERFORATION_HOLES {
        return Err(OpError::invalid(format!(
            "perforasi menghasilkan {} lubang (batas {MAX_PERFORATION_HOLES})",
            holes.len()
        ))
        .with_hint("increase pitch or perforate a smaller sheet"));
    }
    Ok(holes)
}

fn norm(a: V3) -> f64 {
    dot(a, a).sqrt()
}

fn unit(a: V3) -> V3 {
    let l = norm(a).max(1e-300);
    [a[0] / l, a[1] / l, a[2] / l]
}

/// Ubah solid pelat datar setebal seragam (mis. hasil impor STEP) jadi
/// model sheet metal: face datar terbesar + face sejajar berlawanan →
/// tebal; batas face bawah → outline; silinder cekung tegak lurus pelat →
/// lubang. Solid yang sudah bertekuk ditolak dengan daftar radius tekuk
/// yang terdeteksi.
pub fn plate_from_solid(
    shape: &KernelShape,
    bend_radius: Option<f64>,
    k_factor: f64,
) -> OpResult<(SheetMetalModel, PlaneFrame)> {
    use ducad_kernel::SurfaceKind;
    let faces = ducad_kernel::enumerate_faces(shape);
    let planar = || faces.iter().filter(|f| f.kind == SurfaceKind::Plane);
    let top = planar()
        .max_by(|a, b| a.area.total_cmp(&b.area))
        .ok_or_else(|| OpError::invalid("solid tidak punya face datar"))?;
    let n = unit(top.normal);
    let bottom = planar()
        .filter(|f| dot(unit(f.normal), n) < -0.9999 && (f.area - top.area).abs() <= 0.02 * top.area)
        .map(|f| (f, dot(axpy(top.centroid, f.centroid, -1.0), n)))
        .filter(|(_, t)| *t > 1e-6)
        .min_by(|a, b| a.1.total_cmp(&b.1));
    let bends: Vec<f64> = {
        let mut r: Vec<f64> = faces
            .iter()
            .filter(|f| f.kind == SurfaceKind::Cylinder && f.concave == Some(false))
            .filter(|f| f.axis.is_some_and(|(_, ax)| dot(unit(ax), n).abs() < 1e-3))
            .filter_map(|f| f.radius)
            .collect();
        r.sort_by(f64::total_cmp);
        r.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        r
    };
    let not_plate = |why: &str| {
        OpError::invalid(format!(
            "solid bukan pelat datar setebal seragam: {why}"
        ))
        .with_hint("convert_to_sheet takes a flat plate; for a bent part rebuild it with base_flange + edge_flange using the detected bend radii")
        .with_context(serde_json::json!({ "detected_bend_radii": bends }))
    };
    let Some((mut bottom, t)) = bottom else {
        return Err(not_plate("tidak ada face datar sejajar berlawanan seluas face terbesar"));
    };
    // Deterministik: normal pelat condong ke +Z (lalu +Y, +X).
    let mut n = n;
    let first = [n[2], n[1], n[0]]
        .into_iter()
        .find(|c| c.abs() > 1e-9)
        .unwrap_or(1.0);
    if first < 0.0 {
        bottom = top;
        n = [-n[0], -n[1], -n[2]];
    }
    let origin = bottom.centroid;
    let pts3 = &bottom.boundary;
    if pts3.len() < 3 {
        return Err(not_plate("batas face bawah kosong"));
    }
    // u searah sisi terpanjang supaya outline rapi.
    let m = pts3.len();
    let longest = (0..m)
        .map(|i| axpy(pts3[(i + 1) % m], pts3[i], -1.0))
        .max_by(|a, b| norm(*a).total_cmp(&norm(*b)))
        .unwrap_or([1.0, 0.0, 0.0]);
    let along = axpy(longest, n, -dot(longest, n));
    if norm(along) < 1e-9 {
        return Err(not_plate("batas face bawah degenerate"));
    }
    let u = unit(along);
    let v = cross(n, u);
    let frame = PlaneFrame {
        origin,
        u_axis: u,
        v_axis: v,
        normal: n,
    };
    let project = |p: V3| {
        let d = axpy(p, origin, -1.0);
        [dot(d, u), dot(d, v)]
    };
    let mut outline = simplify(pts3.iter().map(|p| project(*p)).collect());
    if outline.len() < 3 {
        return Err(not_plate("outline berluas nol"));
    }
    if ducad_core::sheet_metal::signed_area(&outline) < 0.0 {
        outline.reverse();
    }
    let mut cutouts: Vec<Cutout> = Vec::new();
    for f in faces.iter().filter(|f| {
        f.kind == SurfaceKind::Cylinder
            && f.concave == Some(true)
            && f.axis.is_some_and(|(_, ax)| dot(unit(ax), n).abs() > 0.9999)
    }) {
        let (Some((p, _)), Some(r)) = (f.axis, f.radius) else {
            continue;
        };
        let c = project(p);
        let dup = cutouts.iter().any(|h| {
            matches!(h, Cutout::Circle { center, radius }
                if (center[0] - c[0]).hypot(center[1] - c[1]) < 1e-4 && (radius - r).abs() < 1e-6)
        });
        if !dup {
            cutouts.push(Cutout::Circle { center: c, radius: r });
        }
    }
    let model = SheetMetalModel {
        outline,
        thickness: t,
        default_radius: bend_radius.unwrap_or(t),
        k_factor,
        flanges: Vec::new(),
        bend_table: ducad_core::BendTable::default(),
        cutouts,
    };
    model.validate().map_err(|e| not_plate(&e))?;
    let (want, got) = (shape.volume(), model.folded_volume());
    if want.is_nan() || want <= 0.0 || ((got - want) / want).abs() > 0.01 {
        return Err(not_plate(&format!(
            "volume pelat hasil deteksi {got:.1} mm³ ≠ volume solid {want:.1} mm³ (ada tekukan, fitur, atau lubang non-lingkaran)"
        )));
    }
    Ok((model, frame))
}
