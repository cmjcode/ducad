//! Fase 2 (PLAN_PARITAS_SOLIDCRAFT §2): direct editing face dan alat solid
//! — offset/delete/replace/split face, pipe, web, emboss dari sketsa,
//! align, boundary fill, bounding solid, pattern sepanjang jalur. Fungsi
//! murni: tidak menyentuh undo stack (dipakai `Session` dan GUI).

use ducad_kernel::{KernelShape, PathSegment, Profile, ProfileSegment, SurfaceKind};
use ducad_sketch::{Entity, Sketch};
use glam::{DQuat, DVec2, DVec3};

use super::{finish, require_positive, MAX_LENGTH_MM};
use crate::error::{OpError, OpResult};
use crate::model::BodyGeometry;
use crate::plane::PlaneFrame;

fn k(op: &'static str) -> impl Fn(anyhow::Error) -> OpError {
    move |e| OpError::kernel(op, e)
}

fn tuple(v: DVec3) -> (f64, f64, f64) {
    (v.x, v.y, v.z)
}

fn finite_vec(name: &str, v: DVec3) -> OpResult<()> {
    if v.is_finite() {
        Ok(())
    } else {
        Err(OpError::invalid(format!("{name} tidak valid ({v:?})")))
    }
}

/// Jarak bertanda yang wajib tidak nol dan dalam jangkauan.
fn require_nonzero(name: &str, v: f64) -> OpResult<()> {
    if !v.is_finite() || v.abs() < 1e-6 || v.abs() > MAX_LENGTH_MM {
        return Err(OpError::invalid(format!(
            "{name} harus tidak nol dan |{name}| ≤ {MAX_LENGTH_MM} (diberikan {v})"
        ))
        .with_context(serde_json::json!({ "param": name, "value": v })));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Face: offset / delete / replace / split.
// ---------------------------------------------------------------------------

/// 2.1 Offset Face: geser face searah normal keluar sejauh `distance`.
pub fn offset_face(shape: &KernelShape, faces: &[usize], distance: f64) -> OpResult<BodyGeometry> {
    require_nonzero("distance", distance)?;
    if faces.is_empty() {
        return Err(OpError::invalid("offset_face: tidak ada face terpilih"));
    }
    let out =
        ducad_kernel::offset_faces_by_index(shape, faces, distance).map_err(k("Offset Face"))?;
    finish("Offset Face", out)
}

/// 2.2 Delete Face: hapus face lalu sembuhkan celahnya.
pub fn delete_face(shape: &KernelShape, faces: &[usize]) -> OpResult<BodyGeometry> {
    if faces.is_empty() {
        return Err(OpError::invalid("delete_face: tidak ada face terpilih"));
    }
    let out = ducad_kernel::delete_faces_by_index(shape, faces).map_err(k("Delete Face"))?;
    finish("Delete Face", out)
}

/// 2.3 Replace Face: ganti face planar dengan bidang `plane`.
pub fn replace_face(shape: &KernelShape, face: usize, plane: &PlaneFrame) -> OpResult<BodyGeometry> {
    let p = DVec3::from(plane.origin);
    let n = DVec3::from(plane.normal);
    finite_vec("bidang pengganti", p)?;
    let out =
        ducad_kernel::replace_face_with_plane(shape, face, p, n).map_err(k("Replace Face"))?;
    finish("Replace Face", out)
}

/// 2.11 Split Face dengan bidang (`faces` kosong = semua face yang dilintasi).
pub fn split_face_plane(
    shape: &KernelShape,
    faces: &[usize],
    plane: &PlaneFrame,
) -> OpResult<BodyGeometry> {
    let out = ducad_kernel::split_faces_by_plane(
        shape,
        faces,
        DVec3::from(plane.origin),
        DVec3::from(plane.normal),
    )
    .map_err(k("Split Face"))?;
    finish("Split Face", out)
}

/// Rantai kurva terbuka sketsa (koordinat 2D sketsa) — satu rantai per
/// entitas. Lingkaran jadi dua busur. Konstruksi & tersembunyi dilewati.
pub fn sketch_curve_chains(sketch: &Sketch) -> Vec<Vec<ProfileSegment>> {
    use std::f64::consts::{FRAC_PI_2, PI, TAU};
    let pt = |p: DVec2| (p.x, p.y);
    let mut out = Vec::new();
    for (id, e) in sketch.entities.iter() {
        if e.is_construction() || sketch.is_hidden(id) {
            continue;
        }
        match e {
            Entity::Line { start, end, .. } => out.push(vec![ProfileSegment::Line {
                start: pt(*start),
                end: pt(*end),
            }]),
            Entity::Arc {
                center,
                radius,
                start_angle,
                end_angle,
                ..
            } => {
                let mut a1 = *end_angle;
                if a1 <= *start_angle {
                    a1 += TAU;
                }
                let at = |a: f64| pt(*center + DVec2::new(a.cos(), a.sin()) * *radius);
                out.push(vec![ProfileSegment::Arc {
                    start: at(*start_angle),
                    via: at((*start_angle + a1) * 0.5),
                    end: at(a1),
                }]);
            }
            Entity::Circle { center, radius, .. } => {
                let at = |a: f64| pt(*center + DVec2::new(a.cos(), a.sin()) * *radius);
                out.push(vec![
                    ProfileSegment::Arc {
                        start: at(0.0),
                        via: at(FRAC_PI_2),
                        end: at(PI),
                    },
                    ProfileSegment::Arc {
                        start: at(PI),
                        via: at(PI + FRAC_PI_2),
                        end: at(0.0),
                    },
                ]);
            }
            Entity::Spline { points, .. } if points.len() >= 2 => out.push(
                points
                    .windows(2)
                    .map(|w| ProfileSegment::Line {
                        start: pt(w[0]),
                        end: pt(w[1]),
                    })
                    .collect(),
            ),
            _ => {}
        }
    }
    out
}

/// 2.11 Split Face dengan kurva sketsa (disapu searah normal sketsa).
pub fn split_face_curves(
    shape: &KernelShape,
    faces: &[usize],
    sketch: &Sketch,
    frame: &PlaneFrame,
) -> OpResult<BodyGeometry> {
    let chains = sketch_curve_chains(sketch);
    if chains.is_empty() {
        return Err(OpError::invalid(
            "sketch pemotong split_face tidak berisi garis/busur/lingkaran/spline",
        ));
    }
    let out = ducad_kernel::split_faces_by_curves(
        shape,
        faces,
        frame.origin,
        frame.u_axis,
        frame.v_axis,
        &chains,
    )
    .map_err(k("Split Face"))?;
    finish("Split Face", out)
}

// ---------------------------------------------------------------------------
// Jalur: sampel, pipe, pattern on path.
// ---------------------------------------------------------------------------

/// Pusat lingkaran lewat tiga titik (None bila segaris).
fn circle_center(a: DVec3, b: DVec3, c: DVec3) -> Option<DVec3> {
    let (ab, ac) = (b - a, c - a);
    let n = ab.cross(ac);
    let d = 2.0 * n.length_squared();
    if d < 1e-18 {
        return None;
    }
    Some(a + (n.cross(ab) * ac.length_squared() + ac.cross(n) * ab.length_squared()) / d)
}

/// Polyline rapat dari segmen jalur (busur dipecah 48 bagian).
pub fn path_polyline(segments: &[PathSegment]) -> Vec<DVec3> {
    let mut out: Vec<DVec3> = Vec::new();
    let mut push = |p: DVec3| {
        if out.last().is_none_or(|q| q.distance(p) > 1e-9) {
            out.push(p);
        }
    };
    for s in segments {
        match s {
            PathSegment::Line { start, end } => {
                push(DVec3::from(*start));
                push(DVec3::from(*end));
            }
            PathSegment::Arc { start, via, end } => {
                let (a, m, b) = (DVec3::from(*start), DVec3::from(*via), DVec3::from(*end));
                match circle_center(a, m, b) {
                    Some(c) => {
                        let n = (m - a).cross(b - m).normalize_or_zero();
                        let (ra, rb) = (a - c, b - c);
                        let mut ang = ra.angle_between(rb);
                        if ra.cross(rb).dot(n) < 0.0 {
                            ang = std::f64::consts::TAU - ang;
                        }
                        for i in 0..=48 {
                            let q = DQuat::from_axis_angle(n, ang * i as f64 / 48.0);
                            push(c + q * ra);
                        }
                    }
                    None => {
                        push(a);
                        push(b);
                    }
                }
            }
            PathSegment::Polyline(pts) => pts.iter().for_each(|p| push(DVec3::from(*p))),
        }
    }
    out
}

/// Titik + tangen satuan pada panjang busur `s` sepanjang polyline.
fn at_length(poly: &[DVec3], s: f64) -> (DVec3, DVec3) {
    let mut acc = 0.0;
    for w in poly.windows(2) {
        let seg = w[0].distance(w[1]);
        if seg > 0.0 && acc + seg >= s - 1e-12 {
            let t = ((s - acc) / seg).clamp(0.0, 1.0);
            return (w[0].lerp(w[1], t), (w[1] - w[0]) / seg);
        }
        acc += seg;
    }
    let n = poly.len();
    let dir = if n >= 2 {
        (poly[n - 1] - poly[n - 2]).normalize_or_zero()
    } else {
        DVec3::Z
    };
    (poly.last().copied().unwrap_or(DVec3::ZERO), dir)
}

/// 2.5 Pipe: lingkaran (r, opsional dinding) disapu sepanjang jalur.
pub fn pipe(segments: &[PathSegment], r: f64, wall: Option<f64>) -> OpResult<BodyGeometry> {
    require_positive("r", r)?;
    if let Some(w) = wall {
        require_positive("wall", w)?;
        if w >= r {
            return Err(OpError::invalid(format!(
                "wall ({w}) harus lebih kecil dari r ({r})"
            ))
            .with_hint("hapus 'wall' untuk batang pejal, atau perkecil tebal dinding"));
        }
    }
    let poly = path_polyline(segments);
    if poly.len() < 2 {
        return Err(OpError::invalid("jalur pipe terlalu pendek"));
    }
    let (start, t) = at_length(&poly, 0.0);
    let t = t.normalize_or_zero();
    if t == DVec3::ZERO {
        return Err(OpError::invalid("arah awal jalur pipe tidak terdefinisi"));
    }
    let u = t.any_orthonormal_vector();
    let v = t.cross(u);
    let outer = Profile::Circle {
        center: (0.0, 0.0),
        radius: r,
    };
    let profile = match wall {
        Some(w) => outer.with_holes(vec![Profile::Circle {
            center: (0.0, 0.0),
            radius: r - w,
        }]),
        None => outer,
    };
    let shape = ducad_kernel::sweep_profile_on_plane_along_path(
        &profile,
        start.to_array(),
        u.to_array(),
        v.to_array(),
        t.to_array(),
        segments,
    )
    .map_err(k("Pipe"))?;
    finish("Pipe", shape)
}

/// 2.4 Pattern on Path: `count` instance (termasuk asli) berjarak sama
/// sepanjang jalur. Mengembalikan HANYA salinan baru.
pub fn path_pattern(
    shape: &KernelShape,
    segments: &[PathSegment],
    count: usize,
    align: bool,
) -> OpResult<Vec<BodyGeometry>> {
    if count < 2 {
        return Err(OpError::invalid(format!(
            "pattern path butuh count ≥ 2 (diberikan {count})"
        )));
    }
    if count > 10_000 {
        return Err(OpError::invalid(format!(
            "count terlalu besar: {count} (maks 10000)"
        )));
    }
    let poly = path_polyline(segments);
    let total: f64 = poly.windows(2).map(|w| w[0].distance(w[1])).sum();
    if poly.len() < 2 || total < 1e-6 {
        return Err(OpError::invalid("jalur pattern terlalu pendek"));
    }
    let closed = poly
        .first()
        .zip(poly.last())
        .is_some_and(|(a, b)| a.distance(*b) < 1e-6);
    let step = if closed {
        total / count as f64
    } else {
        total / (count - 1) as f64
    };
    let (p0, t0) = at_length(&poly, 0.0);
    let mut out = Vec::with_capacity(count - 1);
    for i in 1..count {
        let (p, t) = at_length(&poly, step * i as f64);
        let d = p - p0;
        let mut s =
            ducad_kernel::translate_shape(shape, d.x, d.y, d.z).map_err(k("Pattern Path"))?;
        if align {
            let q = DQuat::from_rotation_arc(t0.normalize_or_zero(), t.normalize_or_zero());
            let (axis, angle) = q.to_axis_angle();
            if angle.abs() > 1e-9 && axis.is_finite() {
                s = ducad_kernel::rotate_shape(&s, tuple(p), tuple(axis), angle)
                    .map_err(k("Pattern Path"))?;
            }
        }
        out.push(finish("Pattern Path", s)?);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Web, emboss.
// ---------------------------------------------------------------------------

/// Garis-garis sketsa (dunia 3D) untuk web.
pub fn sketch_lines_3d(sketch: &Sketch, frame: &PlaneFrame) -> Vec<(DVec3, DVec3)> {
    let to3 = |p: DVec2| {
        DVec3::from(frame.origin) + DVec3::from(frame.u_axis) * p.x + DVec3::from(frame.v_axis) * p.y
    };
    sketch
        .entities
        .iter()
        .filter(|(id, e)| !e.is_construction() && !sketch.is_hidden(*id))
        .filter_map(|(_, e)| match e {
            Entity::Line { start, end, .. } => Some((to3(*start), to3(*end))),
            _ => None,
        })
        .collect()
}

/// 2.6 Web: dinding tipis simetris di sepanjang tiap garis, tinggi `depth`
/// searah `up`, di-union ke `base`.
pub fn web(
    base: &KernelShape,
    lines: &[(DVec3, DVec3)],
    up: DVec3,
    thickness: f64,
    depth: f64,
) -> OpResult<BodyGeometry> {
    require_positive("thickness", thickness)?;
    require_positive("depth", depth)?;
    if lines.is_empty() {
        return Err(OpError::invalid("sketch web tidak berisi garis").with_hint(
            "gambar satu atau lebih garis (line) di sketch; tiap garis = satu dinding",
        ));
    }
    let mut acc = ducad_kernel::clone_shape(base).map_err(k("Web"))?;
    for (a, b) in lines {
        let wall = ducad_kernel::create_rib_solid(*a, *b, up, thickness, depth, None)
            .map_err(k("Web"))?;
        acc = ducad_kernel::union(&acc, &wall).map_err(k("Web"))?;
    }
    finish("Web", acc)
}

/// 2.8 Emboss/deboss region sketsa ke `base`.
pub fn emboss(
    base: &KernelShape,
    profiles: &[Profile],
    frame: &PlaneFrame,
    depth: f64,
    deboss: bool,
) -> OpResult<BodyGeometry> {
    require_positive("depth", depth)?;
    if profiles.is_empty() {
        return Err(OpError::invalid("sketch emboss tidak punya region tertutup"));
    }
    let out = ducad_kernel::emboss_profiles_on_plane(
        Some(base),
        profiles,
        frame.origin,
        frame.u_axis,
        frame.v_axis,
        frame.normal,
        depth,
        deboss,
    )
    .map_err(k("Emboss"))?;
    finish("Emboss", out)
}

// ---------------------------------------------------------------------------
// Align.
// ---------------------------------------------------------------------------

/// Face acuan align: datar (titik, normal keluar) atau silinder (titik di
/// sumbu, arah sumbu).
#[derive(Debug, Clone, Copy)]
pub enum AlignFace {
    Plane { point: DVec3, normal: DVec3 },
    Axis { point: DVec3, dir: DVec3 },
}

impl AlignFace {
    /// Dari info face kernel; `None` untuk jenis selain plane/silinder/kerucut.
    pub fn from_info(f: &ducad_kernel::FaceInfo) -> Option<AlignFace> {
        match f.kind {
            SurfaceKind::Plane => Some(AlignFace::Plane {
                point: DVec3::from(f.centroid),
                normal: DVec3::from(f.normal).normalize_or_zero(),
            }),
            SurfaceKind::Cylinder | SurfaceKind::Cone => f.axis.map(|(p, d)| AlignFace::Axis {
                point: DVec3::from(p),
                dir: DVec3::from(d).normalize_or_zero(),
            }),
            _ => None,
        }
    }
}

/// Rotasi, pivot rotasi, dan translasi (diterapkan setelah rotasi) yang
/// membawa `from` ke `to`.
pub fn align_motion(
    from: AlignFace,
    to: AlignFace,
    flip: bool,
    offset: f64,
) -> OpResult<(DQuat, DVec3, DVec3)> {
    match (from, to) {
        (
            AlignFace::Plane {
                point: p1,
                normal: n1,
            },
            AlignFace::Plane {
                point: p2,
                normal: n2,
            },
        ) => {
            let want = if flip { n2 } else { -n2 };
            let q = DQuat::from_rotation_arc(n1, want);
            // p1 tetap saat diputar di sekitarnya; geser sepanjang n2 saja.
            let t = n2 * ((p2 - p1).dot(n2) + offset);
            Ok((q, p1, t))
        }
        (AlignFace::Axis { point: p1, dir: d1 }, AlignFace::Axis { point: p2, dir: d2 }) => {
            let target = if d1.dot(d2) >= 0.0 { d2 } else { -d2 };
            let q = DQuat::from_rotation_arc(d1, target);
            // p1 di sumbu asal → kaki tegak lurusnya di sumbu tujuan.
            let foot = p2 + d2 * (p1 - p2).dot(d2);
            Ok((q, p1, foot - p1 + d2 * offset))
        }
        _ => Err(OpError::invalid(
            "align butuh dua face sejenis: datar–datar atau silinder–silinder",
        )
        .with_hint("pilih face planar (mis. \"<Z\") di kedua body, atau dua face silinder")),
    }
}

/// 2.9 Align: terapkan gerak ke shape.
pub fn align(
    shape: &KernelShape,
    from: AlignFace,
    to: AlignFace,
    flip: bool,
    offset: f64,
) -> OpResult<BodyGeometry> {
    if !offset.is_finite() || offset.abs() > MAX_LENGTH_MM {
        return Err(OpError::invalid(format!(
            "offset align tidak valid ({offset})"
        )));
    }
    let (q, pivot, t) = align_motion(from, to, flip, offset)?;
    let (axis, angle) = q.to_axis_angle();
    let mut s = ducad_kernel::clone_shape(shape).map_err(k("Align"))?;
    if angle.abs() > 1e-12 && axis.is_finite() {
        s = ducad_kernel::rotate_shape(&s, tuple(pivot), tuple(axis), angle)
            .map_err(k("Align"))?;
    }
    if t.length() > 1e-12 {
        s = ducad_kernel::translate_shape(&s, t.x, t.y, t.z).map_err(k("Align"))?;
    }
    finish("Align", s)
}

// ---------------------------------------------------------------------------
// Boundary fill, bounding solid.
// ---------------------------------------------------------------------------

/// 2.10 Boundary Fill.
pub fn boundary_fill(tools: &[&KernelShape], points: &[DVec3]) -> OpResult<BodyGeometry> {
    if tools.is_empty() {
        return Err(OpError::invalid(
            "boundary_fill butuh minimal 1 body pembatas",
        ));
    }
    if points.is_empty() {
        return Err(OpError::invalid("boundary_fill butuh minimal 1 titik"));
    }
    for p in points {
        finite_vec("titik boundary_fill", *p)?;
    }
    let out = ducad_kernel::boundary_fill(tools, points).map_err(|e| {
        OpError::kernel("Boundary Fill", e).with_hint(
            "tiap titik harus berada di dalam salah satu body pembatas atau di rongga tertutup di antaranya",
        )
    })?;
    finish("Boundary Fill", out)
}

/// 2.10 Bounding solid (box / silinder).
pub fn bounding_solid(
    shape: &KernelShape,
    cylinder: bool,
    axis: DVec3,
    margin: f64,
) -> OpResult<BodyGeometry> {
    if !margin.is_finite() || !(0.0..=MAX_LENGTH_MM).contains(&margin) {
        return Err(OpError::invalid(format!(
            "margin harus 0..{MAX_LENGTH_MM} (diberikan {margin})"
        )));
    }
    let out = if cylinder {
        finite_vec("axis", axis)?;
        ducad_kernel::bounding_cylinder_solid(shape, axis, margin)
    } else {
        ducad_kernel::bounding_box_solid(shape, margin)
    }
    .map_err(k("Bounding Solid"))?;
    finish("Bounding Solid", out)
}

// ---------------------------------------------------------------------------
// Alat langsung GUI sebagai op oplog (ADR 0010): push/pull, revolve face,
// round vertex, rib, resize edge, shell variable, adopt.
// ---------------------------------------------------------------------------

/// Toleransi (mm) pencarian sudut terdekat `round_vertex` — sama dengan
/// `EDGE_REAPPLY_TOLERANCE_MM` di GUI.
pub const VERTEX_TOLERANCE_MM: f64 = 5.0;

/// Push/pull satu face (indeks `enumerate_faces`) sejauh `distance`:
/// `> 0` menambah, `< 0` memotong (kernel `extrude_face`).
pub fn push_pull(shape: &KernelShape, face: usize, distance: f64) -> OpResult<BodyGeometry> {
    require_nonzero("distance", distance)?;
    let out = ducad_kernel::extrude_face_by_index(shape, face, distance).map_err(k("Push/Pull"))?;
    finish("Push/Pull", out)
}

/// Revolve satu face planar mengelilingi sumbu di bidangnya, digabung ke body.
pub fn revolve_face(
    shape: &KernelShape,
    face: usize,
    axis_origin: DVec3,
    axis_dir: DVec3,
    angle_deg: Option<f64>,
) -> OpResult<BodyGeometry> {
    finite_vec("axis.origin", axis_origin)?;
    finite_vec("axis.dir", axis_dir)?;
    if axis_dir.length() < 1e-9 {
        return Err(OpError::invalid("axis.dir tidak boleh vektor nol"));
    }
    if let Some(a) = angle_deg {
        if !a.is_finite() || a <= 0.0 || a > 360.0 {
            return Err(OpError::invalid(format!(
                "angle_deg harus 0 < sudut ≤ 360 (diberikan {a})"
            )));
        }
    }
    let out = ducad_kernel::revolve_face_by_index(shape, face, axis_origin, axis_dir, angle_deg)
        .map_err(k("Revolve Face"))?;
    finish("Revolve Face", out)
}

/// Fillet (atau chamfer) semua tepi di sudut terdekat `near`.
pub fn round_vertex(
    shape: &KernelShape,
    near: DVec3,
    radius: f64,
    chamfer: bool,
) -> OpResult<BodyGeometry> {
    require_positive("radius", radius)?;
    finite_vec("near", near)?;
    if ducad_kernel::nearest_vertex(shape, near, VERTEX_TOLERANCE_MM).is_none() {
        return Err(OpError::invalid(format!(
            "tidak ada sudut body dalam {VERTEX_TOLERANCE_MM} mm dari near [{:.3}, {:.3}, {:.3}]",
            near.x, near.y, near.z
        ))
        .with_hint("isi 'near' dengan koordinat sudut body (lihat bbox di inspect)"));
    }
    let (label, out) = if chamfer {
        let r = ducad_kernel::chamfer_vertex_at(shape, radius, near, VERTEX_TOLERANCE_MM);
        ("Chamfer Vertex", r)
    } else {
        let r = ducad_kernel::fillet_vertex_at(shape, radius, near, VERTEX_TOLERANCE_MM);
        ("Fillet Vertex", r)
    };
    finish(label, out.map_err(k(label))?)
}

/// Tulang penguat `start`–`end` digabung ke body.
pub fn rib(
    shape: &KernelShape,
    start: DVec3,
    end: DVec3,
    normal: DVec3,
    thickness: f64,
    depth: f64,
    draft_deg: Option<f64>,
) -> OpResult<BodyGeometry> {
    require_positive("thickness", thickness)?;
    require_positive("depth", depth)?;
    finite_vec("start", start)?;
    finite_vec("end", end)?;
    finite_vec("normal", normal)?;
    if normal.length() < 1e-9 {
        return Err(OpError::invalid("normal rib tidak boleh vektor nol"));
    }
    if let Some(a) = draft_deg {
        if !a.is_finite() || !(0.0..89.0).contains(&a) {
            return Err(OpError::invalid(format!(
                "draft_deg harus 0 ≤ sudut < 89 (diberikan {a})"
            )));
        }
    }
    let out = ducad_kernel::create_rib(shape, start, end, normal, thickness, depth, draft_deg)
        .map_err(k("Rib"))?;
    finish("Rib", out)
}

/// Ubah panjang body sepanjang rusuk `start`–`end` menjadi `length`.
pub fn resize_edge(
    shape: &KernelShape,
    start: DVec3,
    end: DVec3,
    length: f64,
) -> OpResult<BodyGeometry> {
    require_positive("length", length)?;
    finite_vec("start", start)?;
    finite_vec("end", end)?;
    let out = ducad_kernel::resize_shape_along_edge(shape, tuple(start), tuple(end), length)
        .map_err(k("Resize Edge"))?;
    finish("Resize Edge", out)
}

/// Shell dengan tebal per face (indeks `enumerate_faces`).
pub fn shell_variable(
    shape: &KernelShape,
    thickness: f64,
    remove: &[usize],
    custom: &[(usize, f64)],
) -> OpResult<BodyGeometry> {
    require_positive("thickness", thickness)?;
    for (_, t) in custom {
        require_positive("custom.thickness", *t)?;
    }
    let out = ducad_kernel::shell_variable_by_index(shape, thickness, remove, custom)
        .map_err(k("Shell Variable"))?;
    finish("Shell Variable", out)
}

/// Encode shape untuk op `adopt`: `"<producer>:<base64 zlib>"` (cache B-rep
/// native, lossless).
pub fn encode_brep(shape: &KernelShape) -> OpResult<String> {
    let cache = ducad_io::native::NativeBrepCache::of(shape).map_err(k("Adopt"))?;
    Ok(format!("{}:{}", cache.producer, cache.data))
}

/// Kebalikan [`encode_brep`]; teks STEP (`ISO-10303-21;…`) juga diterima.
pub fn decode_brep(brep: &str) -> OpResult<KernelShape> {
    let text = brep.trim_start();
    if text.starts_with("ISO-10303-21") {
        return KernelShape::from_step_string(text).map_err(k("Adopt"));
    }
    let Some((producer, data)) = text.split_once(':') else {
        return Err(OpError::invalid(
            "brep adopt tidak dikenali (harus \"<producer>:<data>\" atau teks STEP)",
        ));
    };
    let cache = ducad_io::native::NativeBrepCache {
        producer: producer.to_string(),
        data: data.to_string(),
    };
    match cache.decode().map_err(k("Adopt"))? {
        Some(shape) => Ok(shape),
        None => Err(OpError::invalid(format!(
            "brep adopt ditulis kernel '{producer}', bukan '{}'",
            KernelShape::NATIVE_CACHE_PRODUCER
        ))),
    }
}

/// Op `adopt` yang membekukan `shape` ke body `body` — dipakai GUI untuk
/// edit langsung yang belum punya op parametrik.
pub fn adopt_op(id: &str, body: &str, shape: &KernelShape) -> OpResult<crate::ops::Op> {
    Ok(crate::ops::Op::Adopt {
        id: id.to_string(),
        body: body.to_string(),
        brep: encode_brep(shape)?,
    })
}
