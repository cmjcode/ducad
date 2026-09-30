//! Operasi lanjutan untuk agent (P14): loft, sweep, helix, draft, mirror,
//! scale, split, fillet variabel. Fungsi murni: tidak menyentuh undo stack.

use ducad_kernel::{
    HelixHandedness, HelixParams, HelixProfileKind, KernelShape, LoftSection, PathSegment, Profile,
};
use ducad_sketch::{Entity, Sketch};
use glam::{DVec2, DVec3};

use super::{finish, require_positive, LIN_TOL};
use crate::error::{OpError, OpResult};
use crate::model::BodyGeometry;
use crate::plane::PlaneFrame;

fn k(op: &'static str) -> impl Fn(anyhow::Error) -> OpError {
    move |e| OpError::kernel(op, e)
}

fn arr(v: DVec3) -> [f64; 3] {
    [v.x, v.y, v.z]
}

/// Loft melewati penampang berurutan (profil + bidangnya).
pub fn loft(sections: &[(Profile, PlaneFrame)]) -> OpResult<BodyGeometry> {
    if sections.len() < 2 {
        return Err(OpError::invalid(format!(
            "loft butuh minimal 2 sketch penampang (diberikan {})",
            sections.len()
        )));
    }
    let secs: Vec<LoftSection> = sections
        .iter()
        .map(|(p, f)| LoftSection {
            profile: p.clone(),
            origin: f.origin,
            u_axis: f.u_axis,
            v_axis: f.v_axis,
            normal: f.normal,
        })
        .collect();
    let shape = ducad_kernel::loft_sections(&secs).map_err(k("Loft"))?;
    finish("Loft", shape)
}

/// Segmen 2D berarah untuk dirangkai menjadi jalur.
#[derive(Clone)]
enum Seg2 {
    Line(DVec2, DVec2),
    Arc(DVec2, DVec2, DVec2),
    Poly(Vec<DVec2>),
}

impl Seg2 {
    fn start(&self) -> DVec2 {
        match self {
            Seg2::Line(a, _) | Seg2::Arc(a, _, _) => *a,
            Seg2::Poly(p) => p[0],
        }
    }
    fn end(&self) -> DVec2 {
        match self {
            Seg2::Line(_, b) | Seg2::Arc(_, _, b) => *b,
            Seg2::Poly(p) => p[p.len() - 1],
        }
    }
    fn reversed(&self) -> Seg2 {
        match self {
            Seg2::Line(a, b) => Seg2::Line(*b, *a),
            Seg2::Arc(a, v, b) => Seg2::Arc(*b, *v, *a),
            Seg2::Poly(p) => Seg2::Poly(p.iter().rev().copied().collect()),
        }
    }
}

/// Rangkai entitas terbuka sketch (garis, busur, spline) menjadi satu
/// jalur 3D berurutan di bidang `frame`.
pub fn sketch_path(sketch: &Sketch, frame: &PlaneFrame) -> OpResult<Vec<PathSegment>> {
    let mut segs: Vec<Seg2> = Vec::new();
    for (id, e) in sketch.entities.iter() {
        if e.is_construction() || sketch.is_hidden(id) {
            continue;
        }
        match e {
            Entity::Line { start, end, .. } => segs.push(Seg2::Line(*start, *end)),
            Entity::Arc {
                center,
                radius,
                start_angle,
                end_angle,
                ..
            } => {
                let mut a1 = *end_angle;
                if a1 <= *start_angle {
                    a1 += std::f64::consts::TAU;
                }
                let at = |a: f64| *center + DVec2::new(a.cos(), a.sin()) * *radius;
                segs.push(Seg2::Arc(
                    at(*start_angle),
                    at((*start_angle + a1) * 0.5),
                    at(a1),
                ));
            }
            Entity::Spline { points, .. } if points.len() >= 2 => {
                segs.push(Seg2::Poly(points.clone()))
            }
            other => {
                return Err(OpError::invalid(format!(
                    "entitas jalur sweep harus garis/busur/spline terbuka (ditemukan {})",
                    entity_kind(other)
                )))
            }
        }
    }
    if segs.is_empty() {
        return Err(OpError::invalid(
            "sketch jalur sweep tidak berisi garis/busur/spline",
        ));
    }
    let close = |a: DVec2, b: DVec2| a.distance(b) <= LIN_TOL * 10.0;
    // Mulai dari ujung yang tidak bersambung dengan segmen lain (jalur
    // terbuka); jalur tertutup mulai dari segmen pertama.
    let touches_other = |segs: &[Seg2], i: usize, p: DVec2| {
        segs.iter()
            .enumerate()
            .any(|(j, o)| j != i && (close(o.start(), p) || close(o.end(), p)))
    };
    let first = (0..segs.len()).find_map(|i| {
        if !touches_other(&segs, i, segs[i].start()) {
            Some((i, false))
        } else if !touches_other(&segs, i, segs[i].end()) {
            Some((i, true))
        } else {
            None
        }
    });
    let mut chain = vec![match first {
        Some((i, true)) => segs.remove(i).reversed(),
        Some((i, false)) => segs.remove(i),
        None => segs.remove(0),
    }];
    while !segs.is_empty() {
        let tail = chain[chain.len() - 1].end();
        let Some(i) = segs
            .iter()
            .position(|s| close(s.start(), tail) || close(s.end(), tail))
        else {
            return Err(OpError::invalid(format!(
                "jalur sweep terputus di ({:.3}, {:.3}); entitas harus bersambung ujung ke ujung",
                tail.x, tail.y
            )));
        };
        let s = segs.remove(i);
        chain.push(if close(s.start(), tail) {
            s
        } else {
            s.reversed()
        });
    }
    let w = |p: DVec2| arr(frame.to_world(p));
    Ok(chain
        .into_iter()
        .map(|s| match s {
            Seg2::Line(a, b) => PathSegment::Line {
                start: w(a),
                end: w(b),
            },
            Seg2::Arc(a, v, b) => PathSegment::Arc {
                start: w(a),
                via: w(v),
                end: w(b),
            },
            Seg2::Poly(p) => PathSegment::Polyline(p.into_iter().map(w).collect()),
        })
        .collect())
}

fn entity_kind(e: &Entity) -> &'static str {
    match e {
        Entity::Line { .. } => "garis",
        Entity::Circle { .. } => "lingkaran",
        Entity::Arc { .. } => "busur",
        Entity::Ellipse { .. } => "elips",
        Entity::Spline { .. } => "spline",
        _ => "path/bentuk tertutup",
    }
}

/// Jalur polyline dari titik 3D.
pub fn points_path(points: &[[f64; 3]]) -> OpResult<Vec<PathSegment>> {
    if points.len() < 2 {
        return Err(OpError::invalid("jalur sweep butuh minimal 2 titik"));
    }
    Ok(points
        .windows(2)
        .map(|w| PathSegment::Line {
            start: w[0],
            end: w[1],
        })
        .collect())
}

/// Sapu `profile` (di bidang `frame`) sepanjang `path`.
pub fn sweep(
    profile: &Profile,
    frame: &PlaneFrame,
    path: &[PathSegment],
) -> OpResult<BodyGeometry> {
    let shape = ducad_kernel::sweep_profile_on_plane_along_path(
        profile,
        frame.origin,
        frame.u_axis,
        frame.v_axis,
        frame.normal,
        path,
    )
    .map_err(k("Sweep"))?;
    finish("Sweep", shape)
}

/// Penampang helix dalam mm.
pub enum HelixShape {
    Circle(f64),
    Rect(f64, f64),
    Triangle(f64, f64),
}

/// Pegas/ulir.
#[allow(clippy::too_many_arguments)]
pub fn helix(
    r: f64,
    end_r: Option<f64>,
    pitch: f64,
    turns: f64,
    section: HelixShape,
    at: [f64; 3],
    axis: [f64; 3],
    left_hand: bool,
) -> OpResult<BodyGeometry> {
    require_positive("r", r)?;
    require_positive("pitch", pitch)?;
    require_positive("turns", turns)?;
    if let Some(e) = end_r {
        require_positive("end_r", e)?;
    }
    let ax = DVec3::from(axis);
    if ax.length() < 1e-9 || !ax.is_finite() {
        return Err(OpError::invalid("sumbu helix tidak boleh vektor nol"));
    }
    let ax = ax.normalize();
    // Arah radial awal: tegak lurus sumbu.
    let helper = if ax.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
    let start_dir = (helper - ax * helper.dot(ax)).normalize();
    let kind = match section {
        HelixShape::Circle(rr) => {
            require_positive("section.r", rr)?;
            HelixProfileKind::Circle { radius: rr }
        }
        HelixShape::Rect(w, h) => {
            require_positive("section.w", w)?;
            require_positive("section.h", h)?;
            HelixProfileKind::Rectangle {
                width: w,
                height: h,
            }
        }
        HelixShape::Triangle(w, h) => {
            require_positive("section.w", w)?;
            require_positive("section.h", h)?;
            HelixProfileKind::Triangle {
                width: w,
                height: h,
            }
        }
    };
    let params = HelixParams {
        radius: r,
        end_radius: end_r,
        pitch,
        turns,
        handedness: if left_hand {
            HelixHandedness::LeftHand
        } else {
            HelixHandedness::RightHand
        },
        origin: at,
        axis: arr(ax),
        start_dir: arr(start_dir),
    };
    let shape = ducad_kernel::create_helix_solid(&params, kind, 36).map_err(k("Helix"))?;
    finish("Helix", shape)
}

pub fn draft(
    shape: &KernelShape,
    faces: &[usize],
    neutral_point: DVec3,
    neutral_normal: DVec3,
    pull: DVec3,
    angle_deg: f64,
) -> OpResult<BodyGeometry> {
    if !(angle_deg > 0.0 && angle_deg < 90.0) {
        return Err(OpError::invalid(format!(
            "angle_deg draft harus di (0, 90) derajat (diberikan {angle_deg})"
        )));
    }
    let s = ducad_kernel::draft_faces_by_index(
        shape,
        faces,
        neutral_point,
        neutral_normal,
        pull,
        angle_deg,
    )
    .map_err(k("Draft"))?;
    finish("Draft", s)
}

pub fn mirror(shape: &KernelShape, point: DVec3, normal: DVec3) -> OpResult<BodyGeometry> {
    if normal.length() < 1e-9 || !normal.is_finite() {
        return Err(OpError::invalid(
            "normal bidang cermin tidak boleh vektor nol",
        ));
    }
    let s = ducad_kernel::mirror_shape(shape, point, normal).map_err(k("Mirror"))?;
    finish("Mirror", s)
}

pub fn scale(shape: &KernelShape, pivot: [f64; 3], factor: f64) -> OpResult<BodyGeometry> {
    require_positive("factor", factor)?;
    let s = ducad_kernel::scale_shape(shape, (pivot[0], pivot[1], pivot[2]), factor)
        .map_err(k("Scale"))?;
    finish("Scale", s)
}

pub fn fillet_variable(
    shape: &KernelShape,
    edges: &[usize],
    r0: f64,
    r1: f64,
) -> OpResult<BodyGeometry> {
    require_positive("radius", r0)?;
    require_positive("radius_end", r1)?;
    let s =
        ducad_kernel::fillet_edges_variable_by_index(shape, r0, r1, edges).map_err(k("Fillet"))?;
    finish("Fillet", s)
}

/// Potong dengan bidang. Hasil: (sisi searah normal, sisi berlawanan);
/// beberapa potongan di satu sisi digabung menjadi compound.
pub fn split(
    shape: &KernelShape,
    point: DVec3,
    normal: DVec3,
) -> OpResult<(Option<BodyGeometry>, Option<BodyGeometry>)> {
    if normal.length() < 1e-9 || !normal.is_finite() {
        return Err(OpError::invalid(
            "normal bidang potong tidak boleh vektor nol",
        ));
    }
    let n = normal.normalize();
    let pieces = ducad_kernel::split_body(shape, point, n).map_err(k("Split"))?;
    let (mut pos, mut neg) = (Vec::new(), Vec::new());
    for p in pieces {
        if p.volume().abs() < 1e-9 {
            continue;
        }
        let c = ducad_kernel::advanced::faces_centroid(&p);
        if (c - point).dot(n) >= 0.0 {
            pos.push(p);
        } else {
            neg.push(p);
        }
    }
    let join = |v: Vec<KernelShape>| -> OpResult<Option<BodyGeometry>> {
        match v.len() {
            0 => Ok(None),
            1 => finish("Split", v.into_iter().next().expect("len 1")).map(Some),
            _ => {
                let refs: Vec<&KernelShape> = v.iter().collect();
                let c = ducad_kernel::make_compound(&refs).map_err(k("Split"))?;
                finish("Split", c).map(Some)
            }
        }
    };
    Ok((join(pos)?, join(neg)?))
}
