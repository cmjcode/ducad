//! Logika alat penghapus tinta (Eraser) — WholeStroke dan Partial (radius).

use glam::Vec2;

use crate::document::InkDoc;
use crate::stroke::InkPoint;

/// Mode penghapus tinta:
/// - `WholeStroke`: menghapus seluruh coretan bila tersentuh oleh lintasan penghapus.
/// - `Partial`: memotong dan menghapus titik-titik yang berada dalam radius lintasan penghapus.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EraseMode {
    WholeStroke,
    Partial { radius_mm: f32 },
}

/// Hasil operasi penghapus yang berisi ID coretan yang dihapus total
/// dan coretan yang digantikan oleh pecahan-pecahan baru.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EraseResult {
    /// ID coretan yang dihapus seluruhnya.
    pub removed: Vec<u64>,
    /// ID coretan lama beserta pecahan-pecahan coretan baru penggantinya.
    pub replaced: Vec<(u64, Vec<Vec<InkPoint>>)>,
}

/// Menghapus coretan tinta sepanjang lintasan `path` berdasarkan mode penghapus.
///
/// Logika ini murni (pure) dan tidak memutasi dokumen; mengembalikan `EraseResult`
/// yang dapat diterapkan lewat command undo/redo.
pub fn erase(doc: &InkDoc, path: &[Vec2], mode: EraseMode) -> EraseResult {
    let mut result = EraseResult::default();
    if path.is_empty() || doc.strokes.is_empty() {
        return result;
    }

    let radius = match mode {
        EraseMode::WholeStroke => 0.5,
        EraseMode::Partial { radius_mm } => radius_mm.max(1e-4),
    };

    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for p in path {
        min_x = min_x.min(p.x);
        max_x = max_x.max(p.x);
        min_y = min_y.min(p.y);
        max_y = max_y.max(p.y);
    }

    let q_min = Vec2::new(min_x - radius, min_y - radius);
    let q_max = Vec2::new(max_x + radius, max_y + radius);

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

    for id in candidate_ids {
        let stroke = match doc.stroke(id) {
            Some(s) if !s.hidden => s,
            _ => continue,
        };

        match mode {
            EraseMode::WholeStroke => {
                if stroke_hits_path(&stroke.points, path, radius) {
                    result.removed.push(id);
                }
            }
            EraseMode::Partial { radius_mm } => {
                let mut chunks: Vec<Vec<InkPoint>> = Vec::new();
                let mut current_chunk: Vec<InkPoint> = Vec::new();
                let mut any_deleted = false;

                for pt in &stroke.points {
                    let d = dist_point_to_path(pt.pos(), path);
                    if d <= radius_mm {
                        any_deleted = true;
                        if !current_chunk.is_empty() {
                            chunks.push(std::mem::take(&mut current_chunk));
                        }
                    } else {
                        current_chunk.push(*pt);
                    }
                }
                if !current_chunk.is_empty() {
                    chunks.push(current_chunk);
                }

                if any_deleted {
                    if chunks.is_empty() {
                        result.removed.push(id);
                    } else {
                        result.replaced.push((id, chunks));
                    }
                }
            }
        }
    }

    result.removed.sort_unstable();
    result.replaced.sort_by_key(|(id, _)| *id);
    result
}

fn dist_point_to_path(p: Vec2, path: &[Vec2]) -> f32 {
    if path.is_empty() {
        return f32::INFINITY;
    }
    if path.len() == 1 {
        return (p - path[0]).length();
    }
    let mut min_d = f32::INFINITY;
    for i in 1..path.len() {
        min_d = min_d.min(dist_to_segment(p, path[i - 1], path[i]));
    }
    min_d
}

fn stroke_hits_path(stroke_pts: &[InkPoint], path: &[Vec2], tol: f32) -> bool {
    if stroke_pts.is_empty() || path.is_empty() {
        return false;
    }
    if stroke_pts.len() == 1 {
        return dist_point_to_path(stroke_pts[0].pos(), path) <= tol;
    }

    for i in 1..stroke_pts.len() {
        let p0 = stroke_pts[i - 1].pos();
        let p1 = stroke_pts[i].pos();

        if path.len() == 1 {
            if dist_to_segment(path[0], p0, p1) <= tol {
                return true;
            }
        } else {
            for j in 1..path.len() {
                let q0 = path[j - 1];
                let q1 = path[j];
                if segments_intersect_or_close(p0, p1, q0, q1, tol) {
                    return true;
                }
            }
        }
    }
    false
}

fn dist_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let ap = p - a;
    let ab_len_sq = ab.length_squared();
    if ab_len_sq <= 1e-9 {
        return ap.length();
    }
    let t = (ap.dot(ab) / ab_len_sq).clamp(0.0, 1.0);
    let proj = a + ab * t;
    (p - proj).length()
}

fn segments_intersect_or_close(a: Vec2, b: Vec2, c: Vec2, d: Vec2, tol: f32) -> bool {
    if segments_intersect(a, b, c, d) {
        return true;
    }
    dist_to_segment(a, c, d) <= tol
        || dist_to_segment(b, c, d) <= tol
        || dist_to_segment(c, a, b) <= tol
        || dist_to_segment(d, a, b) <= tol
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
