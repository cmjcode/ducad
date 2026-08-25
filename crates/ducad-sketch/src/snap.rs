use std::collections::HashSet;
use glam::DVec2;

use crate::constraint::PointRef;
use crate::entity::{Entity, EntityId};
use crate::region::find_closed_regions;
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
    find_snap_with_extra(sketch, cursor, tolerance, grid_step, exclude, &[])
}

/// Cari titik snap terbaik di sekitar kursor dengan tambahan titik-titik kandidat (mis. titik awal/pending saat menggambar).
pub fn find_snap_with_extra(
    sketch: &Sketch,
    cursor: DVec2,
    tolerance: f64,
    grid_step: f64,
    exclude: Option<EntityId>,
    extra_points: &[DVec2],
) -> Option<SnapHit> {
    let exclude_set: Option<HashSet<EntityId>> = exclude.map(|id| {
        let mut s = HashSet::new();
        s.insert(id);
        s
    });
    find_snap_with_exclude_set(
        sketch,
        cursor,
        tolerance,
        grid_step,
        exclude_set.as_ref(),
        extra_points,
    )
}

/// Cari titik snap terbaik di sekitar kursor dengan dukungan exclude set entitas.
pub fn find_snap_with_exclude_set(
    sketch: &Sketch,
    cursor: DVec2,
    tolerance: f64,
    grid_step: f64,
    exclude_set: Option<&HashSet<EntityId>>,
    extra_points: &[DVec2],
) -> Option<SnapHit> {
    let nearest = |kind: SnapKind, pts: Vec<(DVec2, Option<PointRef>)>| -> Option<SnapHit> {
        pts.into_iter()
            .map(|(p, src)| (p, src, (p - cursor).length()))
            .filter(|(_, _, d)| *d <= tolerance)
            .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap())
            .map(|(point, source, _)| SnapHit { point, kind, source })
    };

    let is_excluded = |id: EntityId| -> bool {
        if let Some(set) = exclude_set {
            set.contains(&id)
        } else {
            false
        }
    };

    let others = || {
        sketch
            .entities
            .iter()
            .filter(move |(id, _)| !is_excluded(*id) && !sketch.is_hidden(*id))
    };

    let mut endpoints: Vec<(DVec2, Option<PointRef>)> = others()
        .flat_map(|(id, e)| e.endpoint_refs(id))
        .map(|(r, p)| (p, Some(r)))
        .collect();

    for ep in extra_points {
        endpoints.push((*ep, None));
    }

    if let Some(hit) = nearest(SnapKind::Endpoint, endpoints) {
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

    // Titik Pusat: Circle, Arc, Ellipse, Spline center
    let mut center_pts: Vec<(DVec2, Option<PointRef>)> = others()
        .filter_map(|(id, e)| e.center_ref(id))
        .map(|(r, p)| (p, Some(r)))
        .collect();

    // Centroid dari semua closed region (rectangle, polygon, closed chain, dll.)
    for reg in find_closed_regions(sketch) {
        if let Some(set) = exclude_set {
            if !set.is_empty() && reg.entity_ids.is_subset(set) {
                continue;
            }
        }
        center_pts.push((reg.centroid, None));
    }

    if let Some(hit) = nearest(SnapKind::Center, center_pts) {
        return Some(hit);
    }

    if let Some(hit) = nearest(
        SnapKind::Intersection,
        find_intersections_with_exclude_set(sketch, exclude_set)
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
pub fn find_intersections(sketch: &Sketch, exclude: Option<EntityId>) -> Vec<DVec2> {
    let exclude_set: Option<HashSet<EntityId>> = exclude.map(|id| {
        let mut s = HashSet::new();
        s.insert(id);
        s
    });
    find_intersections_with_exclude_set(sketch, exclude_set.as_ref())
}


/// Titik potong antar entitas Line dengan exclude set.
pub(crate) fn find_intersections_with_exclude_set(
    sketch: &Sketch,
    exclude_set: Option<&HashSet<EntityId>>,
) -> Vec<DVec2> {
    let is_excluded = |id: EntityId| -> bool {
        if let Some(set) = exclude_set {
            set.contains(&id)
        } else {
            false
        }
    };

    let lines: Vec<(DVec2, DVec2)> = sketch
        .entities
        .iter()
        .filter(|(id, _)| !is_excluded(*id) && !sketch.is_hidden(*id))
        .filter_map(|(_, e)| match e {
            Entity::Line { start, end, .. } => Some((*start, *end)),
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

/// Kumpulkan semua titik kandidat snap (Center, Midpoint, Endpoint, Intersection) dari sketch.
pub fn all_snap_candidate_points(
    sketch: &Sketch,
    exclude: Option<EntityId>,
) -> Vec<(DVec2, SnapKind)> {
    let exclude_set: Option<HashSet<EntityId>> = exclude.map(|id| {
        let mut s = HashSet::new();
        s.insert(id);
        s
    });
    all_snap_candidate_points_with_exclude_set(sketch, exclude_set.as_ref())
}

/// Kumpulkan semua titik kandidat snap dari sketch dengan dukungan exclude set entitas.
pub fn all_snap_candidate_points_with_exclude_set(
    sketch: &Sketch,
    exclude_set: Option<&HashSet<EntityId>>,
) -> Vec<(DVec2, SnapKind)> {
    let mut results = Vec::new();

    let is_excluded = |id: EntityId| -> bool {
        if let Some(set) = exclude_set {
            set.contains(&id)
        } else {
            false
        }
    };

    let others = || {
        sketch
            .entities
            .iter()
            .filter(move |(id, _)| !is_excluded(*id) && !sketch.is_hidden(*id))
    };

    // 1. Center points (Circle, Arc, Ellipse, Spline)
    for (_, e) in others() {
        if let Some(c) = e.center() {
            results.push((c, SnapKind::Center));
        }
    }

    // Centroid dari semua closed region
    for reg in find_closed_regions(sketch) {
        if let Some(set) = exclude_set {
            if !set.is_empty() && reg.entity_ids.is_subset(set) {
                continue;
            }
        }
        if !results
            .iter()
            .any(|(p, k)| *k == SnapKind::Center && (p.x - reg.centroid.x).hypot(p.y - reg.centroid.y) < 1e-4)
        {
            results.push((reg.centroid, SnapKind::Center));
        }
    }

    // 2. Midpoints
    for (_, e) in others() {
        if let Some(m) = e.midpoint() {
            results.push((m, SnapKind::Midpoint));
        }
    }

    // 3. Endpoints
    for (id, e) in others() {
        for (_, ep) in e.endpoint_refs(id) {
            results.push((ep, SnapKind::Endpoint));
        }
    }

    // 4. Intersections
    for inter in find_intersections_with_exclude_set(sketch, exclude_set) {
        results.push((inter, SnapKind::Intersection));
    }

    results
}

