//! Seleksi lasso tinta (Contain / Intersect) berbasis poligon sembarang.

use glam::Vec2;

use crate::document::InkDoc;
use crate::stroke::InkPoint;

/// Mode seleksi lasso:
/// - `Contain`: seluruh titik coretan harus berada di dalam poligon.
/// - `Intersect`: sebagian titik di dalam poligon ATAU coretan memotong sisi poligon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LassoMode {
    Contain,
    Intersect,
}

/// Menyeleksi coretan tinta yang dicakup atau dipotong oleh `polygon`.
///
/// Hasilnya adalah daftar ID coretan terurut menaik (deterministik).
pub fn lasso_select(doc: &InkDoc, polygon: &[Vec2], mode: LassoMode) -> Vec<u64> {
    if polygon.len() < 3 || doc.strokes.is_empty() {
        return Vec::new();
    }

    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for p in polygon {
        min_x = min_x.min(p.x);
        max_x = max_x.max(p.x);
        min_y = min_y.min(p.y);
        max_y = max_y.max(p.y);
    }

    let q_min = Vec2::new(min_x, min_y);
    let q_max = Vec2::new(max_x, max_y);

    let candidate_ids = if let Some(idx) = doc.index() {
        idx.query_aabb(q_min, q_max)
    } else {
        let mut ids: Vec<u64> = doc
            .strokes
            .iter()
            .filter(|s| {
                !s.hidden
                    && s.bbox.0.x <= q_max.x
                    && s.bbox.1.x >= q_min.x
                    && s.bbox.0.y <= q_max.y
                    && s.bbox.1.y >= q_min.y
            })
            .map(|s| s.id)
            .collect();
        ids.sort_unstable();
        ids
    };

    let mut selected = Vec::new();
    for id in candidate_ids {
        let stroke = match doc.stroke(id) {
            Some(s) if !s.hidden && !s.points.is_empty() => s,
            _ => continue,
        };

        match mode {
            LassoMode::Contain => {
                if stroke.points.iter().all(|pt| point_in_polygon(pt.pos(), polygon)) {
                    selected.push(id);
                }
            }
            LassoMode::Intersect => {
                if stroke.points.iter().any(|pt| point_in_polygon(pt.pos(), polygon))
                    || stroke_intersects_polygon_edges(&stroke.points, polygon)
                {
                    selected.push(id);
                }
            }
        }
    }

    selected.sort_unstable();
    selected
}

/// Algoritma ray-casting (even-odd) untuk menguji apakah titik `p` berada di dalam `poly`.
pub fn point_in_polygon(p: Vec2, poly: &[Vec2]) -> bool {
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let pi = poly[i];
        let pj = poly[j];
        if ((pi.y > p.y) != (pj.y > p.y))
            && (p.x < (pj.x - pi.x) * (p.y - pi.y) / (pj.y - pi.y) + pi.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn stroke_intersects_polygon_edges(stroke_pts: &[InkPoint], poly: &[Vec2]) -> bool {
    let m = poly.len();
    if m < 2 {
        return false;
    }
    for i in 1..stroke_pts.len() {
        let p0 = stroke_pts[i - 1].pos();
        let p1 = stroke_pts[i].pos();
        let mut j = m - 1;
        for k in 0..m {
            let q0 = poly[j];
            let q1 = poly[k];
            if segments_intersect(p0, p1, q0, q1) {
                return true;
            }
            j = k;
        }
    }
    false
}

fn segments_intersect(p1: Vec2, p2: Vec2, p3: Vec2, p4: Vec2) -> bool {
    fn ccw(a: Vec2, b: Vec2, c: Vec2) -> f32 {
        (c.y - a.y) * (b.x - a.x) - (b.y - a.y) * (c.x - a.x)
    }

    let d1 = ccw(p1, p2, p3);
    let d2 = ccw(p1, p2, p4);
    let d3 = ccw(p3, p4, p1);
    let d4 = ccw(p3, p4, p2);

    ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
}
