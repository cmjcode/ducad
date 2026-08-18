use glam::DVec2;

use crate::constraint::PointRef;
use crate::entity::{Entity, EntityId};
use crate::sketch::Sketch;

/// Jenis snap — dipakai UI untuk memilih glyph indikator yang sesuai.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapKind {
    Endpoint,
    Midpoint,
    Center,
    Intersection,
    Grid,
}

#[derive(Debug, Clone, Copy)]
pub struct SnapHit {
    pub point: DVec2,
    pub kind: SnapKind,
    /// Rujukan titik sumber (entitas + Start/End/Center) bila snap ini
    /// berasal dari titik yang benar-benar jadi DOF entitas.
    pub source: Option<PointRef>,
}

/// Cari titik snap terbaik di sekitar kursor.
pub fn find_snap(
    sketch: &Sketch,
    cursor: DVec2,
    tolerance: f64,
    grid_step: f64,
    exclude: Option<EntityId>,
) -> Option<SnapHit> {
    let nearest = |kind: SnapKind, pts: Vec<(DVec2, Option<PointRef>)>| -> Option<SnapHit> {
        pts.into_iter()
            .map(|(p, src)| (p, src, (p - cursor).length()))
            .filter(|(_, _, d)| *d <= tolerance)
            .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap())
            .map(|(point, source, _)| SnapHit { point, kind, source })
    };

    let others = || {
        sketch
            .entities
            .iter()
            .filter(move |(id, _)| Some(*id) != exclude)
    };

    if let Some(hit) = nearest(
        SnapKind::Endpoint,
        others()
            .flat_map(|(id, e)| e.endpoint_refs(id))
            .map(|(r, p)| (p, Some(r)))
            .collect(),
    ) {
        return Some(hit);
    }
    if let Some(hit) = nearest(
        SnapKind::Midpoint,
        others()
            .filter_map(|(_, e)| e.midpoint())
            .map(|p| (p, None))
            .collect(),
    ) {
        return Some(hit);
    }
    if let Some(hit) = nearest(
        SnapKind::Center,
        others()
            .filter_map(|(id, e)| e.center_ref(id))
            .map(|(r, p)| (p, Some(r)))
            .collect(),
    ) {
        return Some(hit);
    }
    if let Some(hit) = nearest(
        SnapKind::Intersection,
        find_intersections(sketch, exclude)
            .into_iter()
            .map(|p| (p, None))
            .collect(),
    ) {
        return Some(hit);
    }

    let snapped = DVec2::new(
        (cursor.x / grid_step).round() * grid_step,
        (cursor.y / grid_step).round() * grid_step,
    );
    ((snapped - cursor).length() <= tolerance).then_some(SnapHit {
        point: snapped,
        kind: SnapKind::Grid,
        source: None,
    })
}

/// Titik potong antar entitas Line.
pub(crate) fn find_intersections(sketch: &Sketch, exclude: Option<EntityId>) -> Vec<DVec2> {
    let lines: Vec<(DVec2, DVec2)> = sketch
        .entities
        .iter()
        .filter(|(id, _)| Some(*id) != exclude)
        .filter_map(|(_, e)| match e {
            Entity::Line { start, end } => Some((*start, *end)),
            _ => None,
        })
        .collect();

    let mut pts = Vec::new();
    for i in 0..lines.len() {
        for j in (i + 1)..lines.len() {
            if let Some(p) = line_intersection(lines[i], lines[j]) {
                pts.push(p);
            }
        }
    }
    pts
}

pub(crate) fn line_intersection(a: (DVec2, DVec2), b: (DVec2, DVec2)) -> Option<DVec2> {
    let (t, _u) = line_intersection_params(a, b)?;
    Some(a.0 + (a.1 - a.0) * t)
}

/// Parameter `(t, u)` — posisi perpotongan sepanjang segmen `a` dan `b` masing-masing (0..1 di dalam segmen).
pub(crate) fn line_intersection_params(
    (a1, a2): (DVec2, DVec2),
    (b1, b2): (DVec2, DVec2),
) -> Option<(f64, f64)> {
    let r = a2 - a1;
    let s = b2 - b1;
    let denom = r.x * s.y - r.y * s.x;
    if denom.abs() < 1e-9 {
        return None; // sejajar / berimpit
    }
    let diff = b1 - a1;
    let t = (diff.x * s.y - diff.y * s.x) / denom;
    let u = (diff.x * r.y - diff.y * r.x) / denom;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some((t, u))
}
