//! Extrude, revolve, dan boolean.

use std::collections::HashSet;

use ducad_kernel::{ExtrudeExtent, KernelShape, Profile};
use ducad_sketch::{Entity, EntityId, Sketch};
use glam::{DMat3, DQuat, DVec2};

use super::{check_shape, finish, LIN_TOL};
use crate::error::{OpError, OpErrorCode, OpResult};
use crate::model::{BodyGeometry, BooleanKind};
use crate::plane::PlaneFrame;
use crate::profile::{
    arc_endpoints_and_via, build_profile_from_selection, convert_region_to_exact_profile,
    extrude_selection_with_holes_on_plane,
};

/// Cara memilih profil dari sebuah sketch.
pub enum ProfilePick<'a> {
    /// Semua region tertutup non-konstruksi; region bersarang otomatis jadi
    /// lubang (`find_region_hierarchy`).
    AllRegions,
    /// Persis perilaku GUI: `build_profile_from_selection` lalu fallback
    /// `extrude_selection_with_holes_on_plane`.
    Entities(&'a HashSet<EntityId>),
    /// Region terkecil yang memuat titik (koordinat bidang sketch) + lubang
    /// langsungnya.
    AtPoint(DVec2),
}

/// Profil untuk satu `RegionWithHoles`: outer + lubang langsungnya.
fn region_profile(sketch: &Sketch, region: &ducad_sketch::RegionWithHoles) -> Profile {
    let outer = convert_region_to_exact_profile(sketch, &region.outer);
    if region.holes.is_empty() {
        return outer;
    }
    let holes = region
        .holes
        .iter()
        .map(|h| convert_region_to_exact_profile(sketch, h))
        .collect();
    outer.with_holes(holes)
}

/// Titik-titik ujung entitas terbuka (garis/busur/spline) yang tidak punya
/// pasangan dalam `LIN_TOL` — petunjuk kenapa sketch tidak tertutup.
fn dangling_endpoints(sketch: &Sketch) -> Vec<DVec2> {
    let mut ends = Vec::new();
    for (id, e) in sketch.entities.iter() {
        if e.is_construction() || sketch.is_hidden(id) {
            continue;
        }
        match e {
            Entity::Line { start, end, .. } => ends.extend([*start, *end]),
            Entity::Arc {
                center,
                radius,
                start_angle,
                end_angle,
                ..
            } => {
                let (s, _, t) = arc_endpoints_and_via(*center, *radius, *start_angle, *end_angle);
                ends.extend([s, t]);
            }
            Entity::Spline { points, .. } => {
                if let (Some(f), Some(l)) = (points.first(), points.last()) {
                    if (*f - *l).length() > LIN_TOL {
                        ends.extend([*f, *l]);
                    }
                }
            }
            Entity::Path { subpaths, .. } => {
                for sub in subpaths {
                    if !sub.closed && !sub.segs.is_empty() {
                        let first = sub.node(0);
                        let last = sub.node(sub.node_count() - 1);
                        if (first - last).length() > LIN_TOL {
                            ends.extend([first, last]);
                        }
                    }
                }
            }
            Entity::Circle { .. } | Entity::Ellipse { .. } => {}
        }
    }
    ends.iter()
        .enumerate()
        .filter(|(i, p)| {
            !ends
                .iter()
                .enumerate()
                .any(|(j, q)| j != *i && (**p - *q).length() <= LIN_TOL)
        })
        .map(|(_, p)| *p)
        .collect()
}

fn not_closed_error(sketch: &Sketch) -> OpError {
    let count = sketch
        .entities
        .iter()
        .filter(|(id, e)| !e.is_construction() && !sketch.is_hidden(*id))
        .count();
    let dangling = dangling_endpoints(sketch);
    let mut hint =
        format!("sketch memuat {count} entitas non-konstruksi tetapi tidak ada loop tertutup");
    if !dangling.is_empty() {
        let list: Vec<String> = dangling
            .iter()
            .take(6)
            .map(|p| format!("({:.3}, {:.3})", p.x, p.y))
            .collect();
        hint.push_str(&format!(
            "; ujung yang tidak tersambung: {}",
            list.join(", ")
        ));
    }
    OpError::new(
        OpErrorCode::ProfileNotClosed,
        "Tidak ada region tertutup yang bisa dipakai sebagai profil",
    )
    .with_hint(hint)
    .with_context(serde_json::json!({
        "entities": count,
        "dangling": dangling.iter().map(|p| [p.x, p.y]).collect::<Vec<_>>(),
    }))
}

/// Region terkecil yang memuat `p` beserta lubang langsungnya.
fn profile_at_point(sketch: &Sketch, p: DVec2) -> OpResult<Profile> {
    let Some(region) = ducad_sketch::find_region_at_point(sketch, p) else {
        return Err(OpError::new(
            OpErrorCode::ProfileAmbiguous,
            format!(
                "Titik ({:.3}, {:.3}) tidak berada di dalam region tertutup mana pun",
                p.x, p.y
            ),
        )
        .with_context(serde_json::json!({ "at": [p.x, p.y] })));
    };
    let hierarchy = ducad_sketch::find_region_hierarchy(sketch);
    match hierarchy
        .iter()
        .find(|r| r.outer.entity_ids == region.entity_ids)
    {
        Some(tree) => Ok(region_profile(sketch, tree)),
        // Titik ada di dalam loop yang berperan sebagai lubang: loop itu
        // sendiri dipakai sebagai profil (tanpa lubang).
        None => Ok(convert_region_to_exact_profile(sketch, &region)),
    }
}

/// Resolusi `ProfilePick` menjadi satu atau lebih profil kernel.
pub fn resolve_profiles(sketch: &Sketch, pick: &ProfilePick) -> OpResult<Vec<Profile>> {
    match pick {
        ProfilePick::AllRegions => {
            let hierarchy = ducad_sketch::find_region_hierarchy(sketch);
            if hierarchy.is_empty() {
                return Err(not_closed_error(sketch));
            }
            Ok(hierarchy
                .iter()
                .map(|r| region_profile(sketch, r))
                .collect())
        }
        ProfilePick::Entities(ids) => build_profile_from_selection(sketch, ids)
            .map(|p| vec![p])
            .map_err(|msg| OpError::new(OpErrorCode::ProfileNotClosed, msg)),
        ProfilePick::AtPoint(p) => Ok(vec![profile_at_point(sketch, *p)?]),
    }
}

/// Uraikan extent jadi (pergeseran titik awal sepanjang normal, panjang
/// total) — sama dengan `ExtrudeExtent::resolve` di kernel, dipakai untuk
/// jalur yang hanya menerima jarak tunggal.
fn extent_offset_length(extent: ExtrudeExtent) -> (f64, f64) {
    match extent {
        ExtrudeExtent::Blind(d) => (0.0, d),
        ExtrudeExtent::Symmetric(len) => (-len.abs() * 0.5, len.abs()),
        ExtrudeExtent::TwoSided { forward, backward } => (-backward, forward + backward),
    }
}

fn validate_extent(extent: ExtrudeExtent) -> OpResult<()> {
    let (_, length) = extent_offset_length(extent);
    let bad = match extent {
        ExtrudeExtent::Blind(d) => !d.is_finite() || d.abs() < 1e-9,
        ExtrudeExtent::Symmetric(l) => !l.is_finite() || l.abs() < 1e-9,
        ExtrudeExtent::TwoSided { forward, backward } => {
            !forward.is_finite() || !backward.is_finite() || length.abs() < 1e-9
        }
    };
    if bad {
        return Err(OpError::invalid(format!(
            "Panjang extrude harus tidak nol (diberikan {extent:?})"
        )));
    }
    Ok(())
}

fn extrude_one(
    profile: &Profile,
    plane: &PlaneFrame,
    extent: ExtrudeExtent,
) -> anyhow::Result<KernelShape> {
    match extent {
        // Jalur yang sama persis dengan GUI lama (bukan lewat `_extent`) agar
        // topologi hasilnya identik.
        ExtrudeExtent::Blind(d) => ducad_kernel::extrude_profile_on_plane(
            profile,
            plane.origin,
            plane.u_axis,
            plane.v_axis,
            plane.normal,
            d,
        ),
        other => ducad_kernel::extrude_profile_extent(
            profile,
            plane.origin,
            plane.u_axis,
            plane.v_axis,
            plane.normal,
            other,
        ),
    }
}

/// Heuristik GUI: seleksi yang memuat spline padat (≥ 8 titik) dianggap teks
/// hasil vektorisasi font dan langsung lewat jalur multi-region.
fn is_text_selection(sketch: &Sketch, ids: &HashSet<EntityId>) -> bool {
    ids.iter().any(|id| {
        matches!(sketch.entities.get(*id), Some(Entity::Spline { points, .. }) if points.len() >= 8)
    })
}

/// Extrude profil sketch pada `plane`. Mengembalikan `(nama, geometri)`
/// per solid; nama `"Extrude"` untuk profil tunggal, `"Solid"`/`"Teks 3D"`
/// untuk jalur multi-region (sama dengan GUI).
pub fn extrude(
    sketch: &Sketch,
    pick: &ProfilePick,
    plane: &PlaneFrame,
    extent: ExtrudeExtent,
) -> OpResult<Vec<(String, BodyGeometry)>> {
    validate_extent(extent)?;
    match pick {
        ProfilePick::Entities(ids) => extrude_entities(sketch, ids, plane, extent),
        _ => {
            let profiles = resolve_profiles(sketch, pick)?;
            let mut out = Vec::with_capacity(profiles.len());
            for profile in &profiles {
                let shape = extrude_one(profile, plane, extent)
                    .map_err(|e| OpError::kernel("Extrude", e))?;
                out.push(("Solid".to_string(), finish("Extrude", shape)?));
            }
            Ok(out)
        }
    }
}

fn extrude_entities(
    sketch: &Sketch,
    ids: &HashSet<EntityId>,
    plane: &PlaneFrame,
    extent: ExtrudeExtent,
) -> OpResult<Vec<(String, BodyGeometry)>> {
    // 1. Jalur objek geometris parametrik murni (Circle, Ellipse, loop Line/Arc/Spline).
    if !is_text_selection(sketch, ids) {
        if let Ok(profile) = build_profile_from_selection(sketch, ids) {
            if let Ok(shape) = extrude_one(&profile, plane, extent) {
                return Ok(vec![("Extrude".to_string(), finish("Extrude", shape)?)]);
            }
        }
    }

    // 2. Jalur teks 3D & multi-region / lubang boolean — hanya menerima
    // jarak tunggal, jadi extent non-Blind dijalankan dari bidang yang digeser.
    let (offset, length) = extent_offset_length(extent);
    let shifted = if matches!(extent, ExtrudeExtent::Blind(_)) {
        *plane
    } else {
        let mut p = *plane;
        p.origin = (plane.origin_v() + plane.normal_v().normalize_or_zero() * offset).to_array();
        p
    };
    let solids =
        extrude_selection_with_holes_on_plane(sketch, ids, &shifted, length).map_err(|msg| {
            let code = if ids.is_empty() {
                OpErrorCode::InvalidParam
            } else {
                OpErrorCode::ProfileNotClosed
            };
            OpError::new(code, msg)
        })?;
    for (_, geo) in &solids {
        check_shape("Extrude", &geo.shape)?;
    }
    Ok(solids)
}

/// Transformasi rigid yang memetakan bidang XY (tempat kernel membangun
/// profil revolve) ke `plane`: rotasi [u v u×v] lalu translasi ke origin.
fn place_on_plane(shape: KernelShape, plane: &PlaneFrame) -> anyhow::Result<KernelShape> {
    let u = plane.u_v().normalize_or_zero();
    let v = plane.v_v().normalize_or_zero();
    let rot = DQuat::from_mat3(&DMat3::from_cols(u, v, u.cross(v)));
    let (axis, angle) = rot.to_axis_angle();
    let o = plane.origin;
    if angle.abs() < 1e-12 && o.iter().all(|c| c.abs() < 1e-12) {
        return Ok(shape);
    }
    ducad_kernel::transform_shape(
        &shape,
        (o[0], o[1], o[2]),
        (0.0, 0.0, 0.0),
        (axis.x, axis.y, axis.z),
        angle,
    )
}

/// Revolve profil mengelilingi sumbu 2D (koordinat bidang sketch).
/// `angle_deg: None` = 360°.
pub fn revolve(
    sketch: &Sketch,
    pick: &ProfilePick,
    plane: &PlaneFrame,
    axis_origin: DVec2,
    axis_dir: DVec2,
    angle_deg: Option<f64>,
) -> OpResult<BodyGeometry> {
    if axis_dir.length() < 1e-9 || !axis_dir.is_finite() {
        return Err(OpError::invalid("Arah sumbu revolve tidak boleh nol"));
    }
    if let Some(a) = angle_deg {
        if a.is_nan() || a <= 0.0 || a > 360.0 {
            return Err(OpError::invalid(format!(
                "Sudut revolve harus di (0, 360] derajat (diberikan {a})"
            )));
        }
    }
    let profiles = resolve_profiles(sketch, pick)?;
    let mut shapes = Vec::with_capacity(profiles.len());
    for profile in &profiles {
        let s = ducad_kernel::revolve_profile(
            profile,
            (axis_origin.x, axis_origin.y),
            (axis_dir.x, axis_dir.y),
            angle_deg,
        )
        .map_err(|e| OpError::kernel("Revolve", e))?;
        shapes.push(s);
    }
    let shape = match shapes.len() {
        1 => shapes.pop().expect("len == 1"),
        _ => {
            let refs: Vec<&KernelShape> = shapes.iter().collect();
            ducad_kernel::make_compound(&refs).map_err(|e| OpError::kernel("Revolve", e))?
        }
    };
    let placed = place_on_plane(shape, plane).map_err(|e| OpError::kernel("Revolve", e))?;
    finish("Revolve", placed)
}

/// Union/Subtract/Intersect dua shape.
pub fn boolean(a: &KernelShape, b: &KernelShape, kind: BooleanKind) -> OpResult<BodyGeometry> {
    let (label, result) = match kind {
        BooleanKind::Union => ("Union", ducad_kernel::union(a, b)),
        BooleanKind::Subtract => ("Subtract", ducad_kernel::subtract(a, b)),
        BooleanKind::Intersect => ("Intersect", ducad_kernel::intersect(a, b)),
    };
    let shape = result.map_err(|e| {
        if e.downcast_ref::<ducad_kernel::EmptyIntersection>()
            .is_some()
        {
            OpError::new(
                OpErrorCode::EmptyResult,
                format!("{label} menghasilkan solid kosong: kedua body tidak bersinggungan"),
            )
        } else {
            OpError::kernel(label, e)
        }
    })?;
    finish(label, shape)
}
