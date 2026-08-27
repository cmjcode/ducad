use glam::DVec2;
use serde::{Deserialize, Serialize};
use std::f64::consts::{PI, TAU};

use crate::entity::{angle_in_range, distance_point_segment, sample_catmull_rom, Entity, EntityId};
use crate::sketch::Sketch;
use crate::snap::line_intersection_params;

/// Mode penentuan ukuran poligon N-sisi beraturan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolygonMode {
    /// Ukuran diukur dari pusat ke titik sudut / vertex (inscribed / di dalam lingkaran).
    Inscribed,
    /// Ukuran diukur dari pusat ke titik tengah sisi (circumscribed / di luar lingkaran).
    Circumscribed,
}

/// Mode penentuan ukuran slot lonjong (lubang pengait / rel baut).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SlotMode {
    /// Slot diukur dari pusat busur pertama ke pusat busur kedua (Center-to-Center Slot).
    #[default]
    CenterToCenter,
    /// Slot diukur dari ujung luar busur pertama ke ujung luar busur kedua (Overall Slot).
    Overall,
}

/// Bangun Arc yang melalui tiga titik: `p1` jadi salah satu ujung, `p3`
/// ujung lainnya, `p2` menentukan sisi mana yang jadi busur.
pub fn arc_from_three_points(p1: DVec2, p2: DVec2, p3: DVec2) -> Option<Entity> {
    let center = circumcenter(p1, p2, p3)?;
    let radius = (p1 - center).length();
    let angle_of = |p: DVec2| (p - center).y.atan2((p - center).x);
    let (a1, a2, a3) = (angle_of(p1), angle_of(p2), angle_of(p3));

    let ccw_span = |from: f64, to: f64| {
        let d = to - from;
        if d < 0.0 {
            d + TAU
        } else {
            d
        }
    };

    let (start_angle, end_angle) = if ccw_span(a1, a2) <= ccw_span(a1, a3) {
        (a1, a3)
    } else {
        (a3, a1)
    };

    Some(Entity::Arc {
        center,
        radius,
        start_angle,
        end_angle,
        is_construction: false,
    })
}

pub(crate) fn circumcenter(p1: DVec2, p2: DVec2, p3: DVec2) -> Option<DVec2> {
    let (ax, ay) = (p1.x, p1.y);
    let (bx, by) = (p2.x, p2.y);
    let (cx, cy) = (p3.x, p3.y);
    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-9 {
        return None; // kolinear
    }
    let a2 = ax * ax + ay * ay;
    let b2 = bx * bx + by * by;
    let c2 = cx * cx + cy * cy;
    let ux = (a2 * (by - cy) + b2 * (cy - ay) + c2 * (ay - by)) / d;
    let uy = (a2 * (cx - bx) + b2 * (ax - cx) + c2 * (bx - ax)) / d;
    Some(DVec2::new(ux, uy))
}

/// Bangun entitas sejajar `entity` yang melalui (atau mendekati) `reference_point`.
pub fn offset_entity(entity: &Entity, reference_point: DVec2) -> Option<Entity> {
    let is_construction = entity.is_construction();
    match entity {
        Entity::Line { start, end, .. } => {
            let dir = (*end - *start).normalize_or_zero();
            if dir == DVec2::ZERO {
                return None;
            }
            let normal = DVec2::new(-dir.y, dir.x);
            let offset_vec = normal * (reference_point - *start).dot(normal);
            Some(Entity::Line {
                start: *start + offset_vec,
                end: *end + offset_vec,
                is_construction,
            })
        }
        Entity::Circle { center, .. } => {
            let radius = (reference_point - *center).length();
            (radius > 1e-6).then_some(Entity::Circle {
                center: *center,
                radius,
                is_construction,
            })
        }
        Entity::Arc {
            center,
            start_angle,
            end_angle,
            ..
        } => {
            let radius = (reference_point - *center).length();
            (radius > 1e-6).then_some(Entity::Arc {
                center: *center,
                radius,
                start_angle: *start_angle,
                end_angle: *end_angle,
                is_construction,
            })
        }
        Entity::Ellipse {
            center,
            radius_x,
            radius_y,
            ..
        } => {
            let rel = reference_point - *center;
            let rx = *radius_x;
            let ry = *radius_y;
            if rx <= 1e-6 || ry <= 1e-6 {
                return None;
            }
            let norm_dist_sq = (rel.x / rx).powi(2) + (rel.y / ry).powi(2);
            let dist = entity.distance_to(reference_point);
            let signed_d = if norm_dist_sq >= 1.0 { dist } else { -dist };
            let new_rx = rx + signed_d;
            let new_ry = ry + signed_d;
            if new_rx > 1e-4 && new_ry > 1e-4 {
                Some(Entity::Ellipse {
                    center: *center,
                    radius_x: new_rx,
                    radius_y: new_ry,
                    is_construction,
                })
            } else {
                None
            }
        }
        Entity::Spline { points, .. } => {
            if points.len() < 2 {
                return None;
            }
            let dist = entity.distance_to(reference_point);
            if dist < 1e-9 {
                return Some(entity.clone());
            }
            let sampled = sample_catmull_rom(points, 16);
            let mut nearest_seg = (sampled[0], sampled[1]);
            let mut min_seg_dist = f64::INFINITY;
            for w in sampled.windows(2) {
                let d = distance_point_segment(reference_point, w[0], w[1]);
                if d < min_seg_dist {
                    min_seg_dist = d;
                    nearest_seg = (w[0], w[1]);
                }
            }
            let seg_dir = (nearest_seg.1 - nearest_seg.0).normalize_or_zero();
            let seg_normal = DVec2::new(-seg_dir.y, seg_dir.x);
            let mid_seg = (nearest_seg.0 + nearest_seg.1) * 0.5;
            let signed_d = if (reference_point - mid_seg).dot(seg_normal) >= 0.0 {
                dist
            } else {
                -dist
            };

            let n = points.len();
            let mut new_points = Vec::with_capacity(n);
            for i in 0..n {
                let dir = if i == 0 {
                    (points[1] - points[0]).normalize_or_zero()
                } else if i == n - 1 {
                    (points[n - 1] - points[n - 2]).normalize_or_zero()
                } else {
                    (points[i + 1] - points[i - 1]).normalize_or_zero()
                };
                let normal = DVec2::new(-dir.y, dir.x);
                new_points.push(points[i] + normal * signed_d);
            }
            Some(Entity::Spline {
                points: new_points,
                is_construction,
            })
        }
    }
}

/// Pantulkan titik `p` melintasi garis tak-hingga melalui `axis_a`-`axis_b`.
pub fn reflect_point(p: DVec2, axis_a: DVec2, axis_b: DVec2) -> DVec2 {
    let axis_dir = (axis_b - axis_a).normalize_or_zero();
    let rel = p - axis_a;
    let along = axis_dir * rel.dot(axis_dir);
    let perp = rel - along;
    p - perp * 2.0
}

/// Pantulkan `entity` melintasi garis tak-hingga melalui `axis_a`-`axis_b`.
pub fn mirror_entity(entity: &Entity, axis_a: DVec2, axis_b: DVec2) -> Option<Entity> {
    let axis_dir = (axis_b - axis_a).normalize_or_zero();
    if axis_dir == DVec2::ZERO {
        return None;
    }
    let reflect = |p: DVec2| reflect_point(p, axis_a, axis_b);
    let is_construction = entity.is_construction();

    Some(match entity {
        Entity::Line { start, end, .. } => Entity::Line {
            start: reflect(*start),
            end: reflect(*end),
            is_construction,
        },
        Entity::Circle { center, radius, .. } => Entity::Circle {
            center: reflect(*center),
            radius: *radius,
            is_construction,
        },
        Entity::Arc {
            center,
            radius,
            start_angle,
            end_angle,
            ..
        } => {
            let axis_angle = axis_dir.y.atan2(axis_dir.x);
            let reflect_angle = |a: f64| 2.0 * axis_angle - a;
            Entity::Arc {
                center: reflect(*center),
                radius: *radius,
                start_angle: reflect_angle(*end_angle),
                end_angle: reflect_angle(*start_angle),
                is_construction,
            }
        }
        Entity::Ellipse {
            center,
            radius_x,
            radius_y,
            ..
        } => Entity::Ellipse {
            center: reflect(*center),
            radius_x: *radius_x,
            radius_y: *radius_y,
            is_construction,
        },
        Entity::Spline { points, .. } => Entity::Spline {
            points: points.iter().map(|p| reflect(*p)).collect(),
            is_construction,
        },
    })
}

/// Geser entitas sepanjang bidang sketsa-nya (u,v lokal) sejauh `delta`.
pub fn translate_entity(entity: &Entity, delta: DVec2) -> Entity {
    let is_construction = entity.is_construction();
    match entity {
        Entity::Line { start, end, .. } => Entity::Line {
            start: *start + delta,
            end: *end + delta,
            is_construction,
        },
        Entity::Circle { center, radius, .. } => Entity::Circle {
            center: *center + delta,
            radius: *radius,
            is_construction,
        },
        Entity::Arc {
            center,
            radius,
            start_angle,
            end_angle,
            ..
        } => Entity::Arc {
            center: *center + delta,
            radius: *radius,
            start_angle: *start_angle,
            end_angle: *end_angle,
            is_construction,
        },
        Entity::Ellipse {
            center,
            radius_x,
            radius_y,
            ..
        } => Entity::Ellipse {
            center: *center + delta,
            radius_x: *radius_x,
            radius_y: *radius_y,
            is_construction,
        },
        Entity::Spline { points, .. } => Entity::Spline {
            points: points.iter().map(|p| *p + delta).collect(),
            is_construction,
        },
    }
}

/// Titik potong (parameter `t`, 0..1) `line` dengan entitas Line lain di `sketch`.
pub fn line_intersection_params_in_sketch(
    sketch: &Sketch,
    line: (DVec2, DVec2),
    exclude: EntityId,
) -> Vec<f64> {
    sketch
        .entities
        .iter()
        .filter(|(id, _)| *id != exclude)
        .filter_map(|(_, e)| match e {
            Entity::Line { start, end, .. } => line_intersection_params(line, (*start, *end)),
            _ => None,
        })
        .map(|(t, _u)| t)
        .collect()
}

/// Parameter proyeksi (tidak diklem) titik `p` pada segmen `start..end`.
pub fn project_t(start: DVec2, end: DVec2, p: DVec2) -> f64 {
    let ab = end - start;
    let len_sq = ab.length_squared();
    if len_sq < f64::EPSILON {
        return 0.0;
    }
    (p - start).dot(ab) / len_sq
}

/// Sisa segmen (pasangan titik) setelah menghapus interval `[a,b]` yang memuat `click_t`.
pub fn trim_segments(
    start: DVec2,
    end: DVec2,
    cut_ts: &[f64],
    click_t: f64,
) -> Vec<(DVec2, DVec2)> {
    let mut ts: Vec<f64> = cut_ts
        .iter()
        .copied()
        .filter(|t| *t > 1e-6 && *t < 1.0 - 1e-6)
        .collect();
    ts.push(0.0);
    ts.push(1.0);
    ts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ts.dedup_by(|a, b| (*a - *b).abs() < 1e-9);

    ts.windows(2)
        .filter(|w| !(click_t >= w[0] && click_t <= w[1]))
        .map(|w| (start + (end - start) * w[0], start + (end - start) * w[1]))
        .collect()
}

/// Hasil kalkulasi 2D Fillet.
#[derive(Debug, Clone, PartialEq)]
pub struct Fillet2DResult {
    pub trimmed_line1: Entity,
    pub trimmed_line2: Entity,
    pub arc: Entity,
    pub tangent1: DVec2,
    pub tangent2: DVec2,
    pub center: DVec2,
}

/// Hasil kalkulasi 2D Chamfer.
#[derive(Debug, Clone, PartialEq)]
pub struct Chamfer2DResult {
    pub trimmed_line1: Entity,
    pub trimmed_line2: Entity,
    pub bevel_line: Entity,
    pub tangent1: DVec2,
    pub tangent2: DVec2,
}

/// Helper untuk mencari titik sudut perpotongan/pertemuan dua segmen garis dan ujung terjauhnya.
fn resolve_corner_and_far_ends(
    line1: (DVec2, DVec2),
    line2: (DVec2, DVec2),
) -> Option<(DVec2, DVec2, DVec2, bool, bool)> {
    let (a, b) = line1;
    let (c, d) = line2;
    const ENDPOINT_TOL: f64 = 1e-4;

    // 1. Cek apakah langsung berbagi ujung yang sama
    if (a - c).length() < ENDPOINT_TOL {
        return Some((a, b, d, true, true));
    }
    if (a - d).length() < ENDPOINT_TOL {
        return Some((a, b, c, true, false));
    }
    if (b - c).length() < ENDPOINT_TOL {
        return Some((b, a, d, false, true));
    }
    if (b - d).length() < ENDPOINT_TOL {
        return Some((b, a, c, false, false));
    }

    // 2. Jika tidak persis berbagi titik ujung, hitung titik potong garis tak hingga
    let d1 = b - a;
    let d2 = d - c;
    let det = d1.x * d2.y - d1.y * d2.x;
    if det.abs() < 1e-9 {
        return None; // Garis sejajar / kolinear
    }

    let t = ((c - a).x * d2.y - (c - a).y * d2.x) / det;
    let u = ((c - a).x * d1.y - (c - a).y * d1.x) / det;
    let v = a + d1 * t;

    let (p1, l1_start_is_v) = if t <= 0.5 { (b, true) } else { (a, false) };
    let (p2, l2_start_is_v) = if u <= 0.5 { (d, true) } else { (c, false) };

    Some((v, p1, p2, l1_start_is_v, l2_start_is_v))
}

/// Hitung 2D Fillet (busur tangensial) antara dua garis dengan radius `radius`.
pub fn compute_fillet_2d(
    line1: (DVec2, DVec2),
    line2: (DVec2, DVec2),
    radius: f64,
) -> Option<Fillet2DResult> {
    if radius <= 1e-6 {
        return None;
    }

    let (v, p1, p2, l1_start_is_v, l2_start_is_v) = resolve_corner_and_far_ends(line1, line2)?;

    let len1 = (p1 - v).length();
    let len2 = (p2 - v).length();
    if len1 < 1e-6 || len2 < 1e-6 {
        return None;
    }

    let u1 = (p1 - v) / len1;
    let u2 = (p2 - v) / len2;

    let dot = u1.dot(u2).clamp(-1.0, 1.0);
    // Tolak sudut yang terlalu kolinear (0 atau 180 derajat)
    if !(-0.9999..=0.9999).contains(&dot) {
        return None;
    }

    let alpha = dot.acos();
    let tan_half = (alpha * 0.5).tan();
    if tan_half.abs() < 1e-6 {
        return None;
    }

    let d_t = radius / tan_half;
    if d_t > len1 + 1e-5 || d_t > len2 + 1e-5 {
        return None; // Radius terlalu besar untuk panjang segmen
    }

    let t1 = v + u1 * d_t;
    let t2 = v + u2 * d_t;

    let sin_half = (alpha * 0.5).sin();
    let d_c = radius / sin_half;
    let bisector = (u1 + u2).normalize();
    let center = v + bisector * d_c;

    // Hitung sudut awal dan akhir busur
    let phi1 = (t1 - center).y.atan2((t1 - center).x);
    let phi2 = (t2 - center).y.atan2((t2 - center).x);

    let cross = (t1.x - center.x) * (t2.y - center.y) - (t1.y - center.y) * (t2.x - center.x);
    let (start_angle, end_angle) = if cross > 0.0 {
        (phi1, phi2)
    } else {
        (phi2, phi1)
    };

    let arc = Entity::Arc {
        center,
        radius,
        start_angle,
        end_angle,
        is_construction: false,
    };

    let trimmed_line1 = if l1_start_is_v {
        Entity::Line {
            start: t1,
            end: p1,
            is_construction: false,
        }
    } else {
        Entity::Line {
            start: p1,
            end: t1,
            is_construction: false,
        }
    };

    let trimmed_line2 = if l2_start_is_v {
        Entity::Line {
            start: t2,
            end: p2,
            is_construction: false,
        }
    } else {
        Entity::Line {
            start: p2,
            end: t2,
            is_construction: false,
        }
    };

    Some(Fillet2DResult {
        trimmed_line1,
        trimmed_line2,
        arc,
        tangent1: t1,
        tangent2: t2,
        center,
    })
}

/// Hitung 2D Chamfer (garis miring/bevel) antara dua garis dengan jarak pemotongan `dist1` dan `dist2`.
pub fn compute_chamfer_2d(
    line1: (DVec2, DVec2),
    line2: (DVec2, DVec2),
    dist1: f64,
    dist2: f64,
) -> Option<Chamfer2DResult> {
    if dist1 <= 1e-6 || dist2 <= 1e-6 {
        return None;
    }

    let (v, p1, p2, l1_start_is_v, l2_start_is_v) = resolve_corner_and_far_ends(line1, line2)?;

    let len1 = (p1 - v).length();
    let len2 = (p2 - v).length();
    if len1 < 1e-6 || len2 < 1e-6 {
        return None;
    }

    let u1 = (p1 - v) / len1;
    let u2 = (p2 - v) / len2;

    let dot = u1.dot(u2).clamp(-1.0, 1.0);
    if !(-0.9999..=0.9999).contains(&dot) {
        return None;
    }

    if dist1 > len1 + 1e-5 || dist2 > len2 + 1e-5 {
        return None; // Jarak pemotongan melebihi panjang garis
    }

    let t1 = v + u1 * dist1;
    let t2 = v + u2 * dist2;

    let bevel_line = Entity::Line {
        start: t1,
        end: t2,
        is_construction: false,
    };

    let trimmed_line1 = if l1_start_is_v {
        Entity::Line {
            start: t1,
            end: p1,
            is_construction: false,
        }
    } else {
        Entity::Line {
            start: p1,
            end: t1,
            is_construction: false,
        }
    };

    let trimmed_line2 = if l2_start_is_v {
        Entity::Line {
            start: t2,
            end: p2,
            is_construction: false,
        }
    } else {
        Entity::Line {
            start: p2,
            end: t2,
            is_construction: false,
        }
    };

    Some(Chamfer2DResult {
        trimmed_line1,
        trimmed_line2,
        bevel_line,
        tangent1: t1,
        tangent2: t2,
    })
}

/// Cari pasangan garis yang bertemu pada atau dekat titik `point` dalam toleransi `tolerance`.
pub fn find_corner_lines_at_point(
    sketch: &Sketch,
    point: DVec2,
    tolerance: f64,
) -> Option<(EntityId, EntityId, DVec2)> {
    let lines: Vec<(EntityId, DVec2, DVec2)> = sketch
        .entities
        .iter()
        .filter(|(id, _)| !sketch.is_hidden(*id))
        .filter_map(|(id, entity)| match *entity {
            Entity::Line { start, end, .. } => Some((id, start, end)),
            _ => None,
        })
        .collect();

    let mut candidate_corners: Vec<(EntityId, EntityId, DVec2, f64)> = Vec::new();

    for i in 0..lines.len() {
        for j in (i + 1)..lines.len() {
            let (id1, s1, e1) = lines[i];
            let (id2, s2, e2) = lines[j];

            let pairs = [
                (s1, s2, (s1 + s2) * 0.5),
                (s1, e2, (s1 + e2) * 0.5),
                (e1, s2, (e1 + s2) * 0.5),
                (e1, e2, (e1 + e2) * 0.5),
            ];

            for (p1, p2, corner) in pairs {
                if (p1 - p2).length() <= (tolerance * 0.5).max(1e-2) {
                    let d = (corner - point).length();
                    if d <= tolerance {
                        candidate_corners.push((id1, id2, corner, d));
                    }
                }
            }
        }
    }

    candidate_corners.sort_by(|a, b| a.3.partial_cmp(&b.3).unwrap_or(std::cmp::Ordering::Equal));
    candidate_corners.first().map(|(id1, id2, corner, _)| (*id1, *id2, *corner))
}


#[derive(Debug, Clone, PartialEq)]
pub enum FilletTarget {
    /// Sudut tajam pertemuan 2 garis lurus (belum difillet)
    SharpCorner {
        line1: EntityId,
        line2: EntityId,
        corner: DVec2,
        bisector: DVec2,
    },
    /// Fillet arc yang sudah ada menghubungkan 2 garis
    ExistingFillet {
        arc_id: EntityId,
        line1: EntityId,
        line2: EntityId,
        apex: DVec2,
        bisector: DVec2,
        radius: f64,
        far1: DVec2,
        far2: DVec2,
    },
}

/// Cari semua target fillet pada sketch:
/// 1. Sudut tajam pertemuan 2 garis lurus.
/// 2. Arc yang merupakan fillet dari 2 garis lurus (untuk revisi radius langsung).
pub fn find_all_fillet_targets(sketch: &Sketch) -> Vec<FilletTarget> {
    const SNAP_TOL: f64 = 0.5;

    let lines: Vec<(EntityId, DVec2, DVec2)> = sketch
        .entities
        .iter()
        .filter(|(id, _)| !sketch.is_hidden(*id))
        .filter_map(|(id, e)| match e {
            Entity::Line { start, end, .. } => Some((id, *start, *end)),
            _ => None,
        })
        .collect();

    let arcs: Vec<(EntityId, DVec2, f64, f64, f64, DVec2, DVec2)> = sketch
        .entities
        .iter()
        .filter(|(id, _)| !sketch.is_hidden(*id))
        .filter_map(|(id, e)| match e {
            Entity::Arc {
                center,
                radius,
                start_angle,
                end_angle,
                ..
            } => {
                let p1 = *center + DVec2::new(*radius * start_angle.cos(), *radius * start_angle.sin());
                let p2 = *center + DVec2::new(*radius * end_angle.cos(), *radius * end_angle.sin());
                Some((id, *center, *radius, *start_angle, *end_angle, p1, p2))
            }
            _ => None,
        })
        .collect();

    let mut targets = Vec::new();
    let mut consumed_line_endpoints: Vec<(EntityId, DVec2)> = Vec::new();

    // 1. Deteksi Arc yang menghubungkan 2 garis lurus (Existing Fillet)
    for (arc_id, center, radius, _, _, ap1, ap2) in arcs {
        let mut conn1: Option<(EntityId, DVec2, DVec2)> = None; // (line_id, near_pt, far_pt)
        let mut conn2: Option<(EntityId, DVec2, DVec2)> = None;

        for &(lid, ls, le) in &lines {
            if (ls - ap1).length() <= SNAP_TOL {
                conn1 = Some((lid, ls, le));
            } else if (le - ap1).length() <= SNAP_TOL {
                conn1 = Some((lid, le, ls));
            }
            if (ls - ap2).length() <= SNAP_TOL {
                conn2 = Some((lid, ls, le));
            } else if (le - ap2).length() <= SNAP_TOL {
                conn2 = Some((lid, le, ls));
            }
        }

        if let (Some((l1_id, n1, f1)), Some((l2_id, n2, f2))) = (conn1, conn2) {
            if l1_id != l2_id {
                // Hitung perpotongan garis tak hingga (f1 -> n1) dan (f2 -> n2) untuk cari apex
                let d1 = n1 - f1;
                let d2 = n2 - f2;
                let det = d1.x * d2.y - d1.y * d2.x;
                if det.abs() > 1e-6 {
                    let t = ((f2 - f1).x * d2.y - (f2 - f1).y * d2.x) / det;
                    let apex = f1 + d1 * t;
                    let bisector = (center - apex).normalize_or_zero();
                    if bisector.length() > 0.01 {
                        consumed_line_endpoints.push((l1_id, n1));
                        consumed_line_endpoints.push((l2_id, n2));
                        targets.push(FilletTarget::ExistingFillet {
                            arc_id,
                            line1: l1_id,
                            line2: l2_id,
                            apex,
                            bisector,
                            radius,
                            far1: f1,
                            far2: f2,
                        });
                    }
                }
            }
        }
    }

    // 2. Deteksi sudut tajam pertemuan 2 garis lurus (Sharp Line Corner)
    for i in 0..lines.len() {
        for j in (i + 1)..lines.len() {
            let (id1, s1, e1) = lines[i];
            let (id2, s2, e2) = lines[j];

            let pairs: [(DVec2, DVec2, DVec2, DVec2); 4] = [
                (s1, s2, e1, e2),
                (s1, e2, e1, s2),
                (e1, s2, s1, e2),
                (e1, e2, s1, s2),
            ];

            for (p1, p2, far1, far2) in pairs {
                if (p1 - p2).length() <= SNAP_TOL {
                    let corner = (p1 + p2) * 0.5;
                    // Abaikan jika endpoint ini adalah bagian dari existing fillet
                    if consumed_line_endpoints
                        .iter()
                        .any(|(id, pt)| (*id == id1 || *id == id2) && (*pt - corner).length() <= SNAP_TOL)
                    {
                        continue;
                    }

                    let u1 = (far1 - corner).normalize_or_zero();
                    let u2 = (far2 - corner).normalize_or_zero();
                    let dot = u1.dot(u2).clamp(-1.0, 1.0);
                    // Hindari garis lurus (dot ~ -1) atau segmen bertumpuk (dot ~ 1)
                    if dot.abs() < 0.999 {
                        let b = (u1 + u2).normalize_or_zero();
                        let bisector = if b.length() > 0.01 {
                            b
                        } else {
                            DVec2::new(-u1.y, u1.x)
                        };

                        if !targets.iter().any(|t| match t {
                            FilletTarget::SharpCorner { corner: c, .. } => (*c - corner).length() < SNAP_TOL,
                            FilletTarget::ExistingFillet { apex, .. } => (*apex - corner).length() < SNAP_TOL,
                        }) {
                            targets.push(FilletTarget::SharpCorner {
                                line1: id1,
                                line2: id2,
                                corner,
                                bisector,
                            });
                        }
                    }
                }
            }
        }
    }

    targets
}

/// Temukan semua sudut pertemuan antar garis pada sketch.
/// Khusus untuk sudut tajam antar 2 garis lurus (bukan sambungan tangen busur).
pub fn find_all_corners(sketch: &Sketch) -> Vec<(EntityId, EntityId, DVec2, DVec2)> {
    find_all_fillet_targets(sketch)
        .into_iter()
        .filter_map(|t| match t {
            FilletTarget::SharpCorner {
                line1,
                line2,
                corner,
                bisector,
            } => Some((line1, line2, corner, bisector)),
            _ => None,
        })
        .collect()
}

/// Putar satu titik 2D mengelilingi pusat `pivot` sebesar `angle_rad` radian.
pub fn rotate_point(p: DVec2, pivot: DVec2, angle_rad: f64) -> DVec2 {
    let rel = p - pivot;
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();
    pivot + DVec2::new(rel.x * cos_a - rel.y * sin_a, rel.x * sin_a + rel.y * cos_a)
}

/// Putar entitas 2D mengelilingi titik pusat `pivot` sebesar `angle_rad` radian.
pub fn rotate_entity(entity: &Entity, pivot: DVec2, angle_rad: f64) -> Entity {
    let is_construction = entity.is_construction();
    match entity {
        Entity::Line { start, end, .. } => Entity::Line {
            start: rotate_point(*start, pivot, angle_rad),
            end: rotate_point(*end, pivot, angle_rad),
            is_construction,
        },
        Entity::Circle { center, radius, .. } => Entity::Circle {
            center: rotate_point(*center, pivot, angle_rad),
            radius: *radius,
            is_construction,
        },
        Entity::Arc {
            center,
            radius,
            start_angle,
            end_angle,
            ..
        } => Entity::Arc {
            center: rotate_point(*center, pivot, angle_rad),
            radius: *radius,
            start_angle: start_angle + angle_rad,
            end_angle: end_angle + angle_rad,
            is_construction,
        },
        Entity::Ellipse {
            center,
            radius_x,
            radius_y,
            ..
        } => {
            let norm_angle = ((angle_rad % std::f64::consts::TAU) + std::f64::consts::TAU) % std::f64::consts::TAU;
            let is_perpendicular = (norm_angle - std::f64::consts::FRAC_PI_2).abs() < 0.01
                || (norm_angle - 3.0 * std::f64::consts::FRAC_PI_2).abs() < 0.01;
            let (rx, ry) = if is_perpendicular {
                (*radius_y, *radius_x)
            } else {
                (*radius_x, *radius_y)
            };
            Entity::Ellipse {
                center: rotate_point(*center, pivot, angle_rad),
                radius_x: rx,
                radius_y: ry,
                is_construction,
            }
        }
        Entity::Spline { points, .. } => Entity::Spline {
            points: points
                .iter()
                .map(|p| rotate_point(*p, pivot, angle_rad))
                .collect(),
            is_construction,
        },
    }
}

/// Buat salinan entitas dalam susunan grid linier 2D (Count X, Pitch X, Count Y, Pitch Y).
/// Mengembalikan HANYA entitas salinan baru (tidak termasuk entitas asli pada indeks (0, 0)).
pub fn linear_pattern_entities(
    entities: &[Entity],
    count_x: usize,
    pitch_x: f64,
    count_y: usize,
    pitch_y: f64,
) -> Vec<Entity> {
    let mut new_entities = Vec::new();
    let cx = count_x.max(1);
    let cy = count_y.max(1);

    for iy in 0..cy {
        for ix in 0..cx {
            if ix == 0 && iy == 0 {
                continue; // Lewati entitas asli
            }
            let delta = DVec2::new(ix as f64 * pitch_x, iy as f64 * pitch_y);
            for e in entities {
                new_entities.push(translate_entity(e, delta));
            }
        }
    }

    new_entities
}

/// Transformasi sebuah entitas dengan rotasi terhadap centroid aslinya, lalu translasi ke pusat target baru.
pub fn transform_entity_to_target(
    entity: &Entity,
    orig_center: DVec2,
    target_center: DVec2,
    angle_rad: f64,
) -> Entity {
    let rotated = rotate_entity(entity, orig_center, angle_rad);
    let delta = target_center - orig_center;
    translate_entity(&rotated, delta)
}

/// Buat salinan entitas dalam susunan melingkar (Circular Pattern 2D) mengelilingi titik pusat `pivot`.
/// `count`: jumlah TOTAL item (termasuk item awal). Jika count <= 1, mengembalikan kosong.
/// `total_angle_rad`: rentang sudut rotasi total (mis. 2*PI untuk 360° penuh).
/// Mengembalikan HANYA entitas salinan baru (tidak termasuk entitas asli pada k=0).
pub fn circular_pattern_entities(
    entities: &[Entity],
    pivot: DVec2,
    count: usize,
    total_angle_rad: f64,
) -> Vec<Entity> {
    circular_pattern_entities_with_radius(entities, pivot, count, total_angle_rad, None)
}

/// Buat salinan entitas dalam susunan melingkar (Circular Pattern 2D) dengan radius orbit eksplisit.
pub fn circular_pattern_entities_with_radius(
    entities: &[Entity],
    pivot: DVec2,
    count: usize,
    total_angle_rad: f64,
    custom_radius: Option<f64>,
) -> Vec<Entity> {
    if count <= 1 || entities.is_empty() {
        return Vec::new();
    }

    let orig_centroid = compute_entities_centroid(entities).unwrap_or(pivot);
    let base_vec = orig_centroid - pivot;
    let base_dist = base_vec.length();
    let r = custom_radius.unwrap_or(if base_dist > 1e-4 { base_dist } else { 30.0 });
    let base_dir = if base_dist > 1e-4 {
        base_vec / base_dist
    } else {
        DVec2::new(1.0, 0.0)
    };

    let mut new_entities = Vec::new();
    let is_full_circle = (total_angle_rad.abs() - std::f64::consts::TAU).abs() < 1e-4;
    let step_angle = if is_full_circle {
        total_angle_rad / count as f64
    } else {
        total_angle_rad / (count - 1) as f64
    };

    // Jika custom_radius berbeda dari jarak asli objek ke pivot,
    // maka objek asli berada di luar orbit radius sehingga slot k=0
    // di lingkaran harus ikut dibuatkan salinan agar lingkaran terisi penuh.
    let start_k = if let Some(cr) = custom_radius {
        if (cr - base_dist).abs() > 1e-3 {
            0
        } else {
            1
        }
    } else {
        1
    };

    for k in start_k..count {
        let angle = k as f64 * step_angle;
        let (sin_a, cos_a) = angle.sin_cos();
        let rot_dir = DVec2::new(
            base_dir.x * cos_a - base_dir.y * sin_a,
            base_dir.x * sin_a + base_dir.y * cos_a,
        );
        let target_center = pivot + rot_dir * r;

        for e in entities {
            new_entities.push(transform_entity_to_target(e, orig_centroid, target_center, angle));
        }
    }

    new_entities
}

/// Hitung titik pusat bounding box gabungan dari sekumpulan entitas sketsa.
pub fn compute_entities_centroid(entities: &[Entity]) -> Option<DVec2> {
    if entities.is_empty() {
        return None;
    }
    let mut sum = DVec2::ZERO;
    let mut count = 0.0;

    for e in entities {
        for pt in e.endpoints() {
            sum += pt;
            count += 1.0;
        }
    }

    if count > 0.0 {
        Some(sum / count)
    } else {
        None
    }
}

/// Hitung daftar titik sudut (vertices) untuk poligon $N$-sisi beraturan.
///
/// - `center`: Titik pusat poligon $C$.
/// - `point2`: Titik kedua (kursor) yang menentukan radius dan sudut orientasi.
/// - `sides`: Jumlah sisi $N$ ($N \ge 3$).
/// - `mode`: `Inscribed` (radius ke titik sudut) atau `Circumscribed` (radius ke titik tengah sisi).
pub fn regular_polygon_vertices(
    center: DVec2,
    point2: DVec2,
    sides: usize,
    mode: PolygonMode,
) -> Option<Vec<DVec2>> {
    let sides = sides.max(3);
    let delta = point2 - center;
    let r = delta.length();
    if r < 1e-6 {
        return None;
    }
    let base_angle = delta.y.atan2(delta.x);
    let step_angle = TAU / sides as f64;

    let (r_vertex, start_angle) = match mode {
        PolygonMode::Inscribed => (r, base_angle),
        PolygonMode::Circumscribed => {
            let half_step = PI / sides as f64;
            let r_v = r / half_step.cos();
            (r_v, base_angle - half_step)
        }
    };

    let vertices: Vec<DVec2> = (0..sides)
        .map(|i| {
            let angle = start_angle + i as f64 * step_angle;
            center + DVec2::new(r_vertex * angle.cos(), r_vertex * angle.sin())
        })
        .collect();

    Some(vertices)
}

/// Bangun entitas `Line` tersambung membentuk loop tertutup poligon $N$-sisi beraturan.
pub fn regular_polygon_entities(
    center: DVec2,
    point2: DVec2,
    sides: usize,
    mode: PolygonMode,
    is_construction: bool,
) -> Option<Vec<Entity>> {
    let verts = regular_polygon_vertices(center, point2, sides, mode)?;
    let n = verts.len();
    let lines = (0..n)
        .map(|i| Entity::line(verts[i], verts[(i + 1) % n]).with_construction(is_construction))
        .collect();
    Some(lines)
}

/// Bangun entitas pembentuk slot lonjong (2 Line dan 2 Arc) berdasarkan 2 titik aksis dan 1 titik penentu radius/lebar.
pub fn slot_from_points(
    p1: DVec2,
    p2: DVec2,
    p3: DVec2,
    mode: SlotMode,
    is_construction: bool,
) -> Option<Vec<Entity>> {
    let delta = p2 - p1;
    let len = delta.length();
    if len < 1e-6 {
        return None;
    }
    let u = delta / len;
    let normal = DVec2::new(-u.y, u.x);
    let radius = ((p3 - p1).dot(normal)).abs();
    if radius < 1e-6 {
        return None;
    }
    slot_from_radius(p1, p2, radius, mode, is_construction)
}

/// Bangun entitas pembentuk slot lonjong (2 Line dan 2 Arc) berdasarkan 2 titik aksis dan nilai radius/setengah-lebar $R$.
pub fn slot_from_radius(
    p1: DVec2,
    p2: DVec2,
    radius: f64,
    mode: SlotMode,
    is_construction: bool,
) -> Option<Vec<Entity>> {
    let radius = radius.abs();
    if radius < 1e-6 {
        return None;
    }
    let delta = p2 - p1;
    let len = delta.length();
    if len < 1e-6 {
        return None;
    }
    let u = delta / len;
    let v = DVec2::new(-u.y, u.x);
    let theta = u.y.atan2(u.x);

    let (c1, c2) = match mode {
        SlotMode::CenterToCenter => (p1, p2),
        SlotMode::Overall => {
            if len <= 2.0 * radius {
                let mid = (p1 + p2) * 0.5;
                (mid, mid)
            } else {
                (p1 + u * radius, p2 - u * radius)
            }
        }
    };

    // 4 Corner / Tangent Points:
    // a: top-left (c1 + v * R)
    // b: top-right (c2 + v * R)
    // c: bottom-right (c2 - v * R)
    // d: bottom-left (c1 - v * R)
    let a = c1 + v * radius;
    let b = c2 + v * radius;
    let c = c2 - v * radius;
    let d = c1 - v * radius;

    // Arc at c2 (right cap): sweeps CCW from -v (theta - PI/2) through +u (theta) to +v (theta + PI/2)
    // Start at c, End at b
    let arc2_start = theta - PI * 0.5;
    let arc2_end = theta + PI * 0.5;

    // Arc at c1 (left cap): sweeps CCW from +v (theta + PI/2) through -u (theta + PI) to -v (theta + 3*PI/2)
    // Start at a, End at d
    let arc1_start = theta + PI * 0.5;
    let arc1_end = theta + PI * 1.5;

    // Closed CCW Loop:
    // 1. Arc c2: c -> b
    // 2. Line: b -> a
    // 3. Arc c1: a -> d
    // 4. Line: d -> c
    let entities = vec![
        Entity::Arc {
            center: c2,
            radius,
            start_angle: arc2_start,
            end_angle: arc2_end,
            is_construction,
        },
        Entity::Line {
            start: b,
            end: a,
            is_construction,
        },
        Entity::Arc {
            center: c1,
            radius,
            start_angle: arc1_start,
            end_angle: arc1_end,
            is_construction,
        },
        Entity::Line {
            start: d,
            end: c,
            is_construction,
        },
    ];

    Some(entities)
}

/// Sepasang Arc lingkaran tangensial (G1 continuous Bi-Arc) yang menghubungkan titik `p0` (vektor singgung `t0`)
/// ke titik `p1` (vektor singgung `t1`).
pub fn biarc_fit(
    p0: DVec2,
    t0: DVec2,
    p1: DVec2,
    t1: DVec2,
    is_construction: bool,
) -> Option<(Entity, Entity)> {
    let chord = p1 - p0;
    let chord_len = chord.length();
    if chord_len < 1e-6 {
        return None;
    }
    let t0 = t0.normalize_or_zero();
    let t1 = t1.normalize_or_zero();
    if t0 == DVec2::ZERO || t1 == DVec2::ZERO {
        return None;
    }

    let n0 = DVec2::new(-t0.y, t0.x);
    let n1 = DVec2::new(-t1.y, t1.x);

    let k = 2.0 * (1.0 - t0.dot(t1));
    let pm = if k.abs() < 1e-6 {
        (p0 + p1) * 0.5
    } else {
        let b = -2.0 * chord.dot(t0 + t1);
        let c = 2.0 * chord.length_squared();
        let discr = (b * b - 4.0 * k * (-c)).max(0.0);
        let d = (-b + discr.sqrt()) / (2.0 * k);
        let d = if d.is_finite() && d > 0.0 {
            d
        } else {
            chord_len * 0.5
        };
        let q0 = p0 + t0 * (d * 0.5);
        let q1 = p1 - t1 * (d * 0.5);
        (q0 + q1) * 0.5
    };

    let mid1 = (p0 + pm) * 0.5;
    let chord1 = pm - p0;
    let bisector1_dir = DVec2::new(-chord1.y, chord1.x);
    let c1 = line_intersection_2d(p0, n0, mid1, bisector1_dir).unwrap_or(mid1);
    let r1 = (p0 - c1).length().max(1e-4);
    let a0 = (p0 - c1).y.atan2((p0 - c1).x);
    let am1 = (pm - c1).y.atan2((pm - c1).x);
    let (s1, e1) = orient_arc_angles(a0, am1, t0, p0 - c1);

    let mid2 = (pm + p1) * 0.5;
    let chord2 = p1 - pm;
    let bisector2_dir = DVec2::new(-chord2.y, chord2.x);
    let c2 = line_intersection_2d(p1, n1, mid2, bisector2_dir).unwrap_or(mid2);
    let r2 = (p1 - c2).length().max(1e-4);
    let am2 = (pm - c2).y.atan2((pm - c2).x);
    let a1 = (p1 - c2).y.atan2((p1 - c2).x);
    let (s2, e2) = orient_arc_angles(am2, a1, t1, p1 - c2);

    let arc1 = Entity::Arc {
        center: c1,
        radius: r1,
        start_angle: s1,
        end_angle: e1,
        is_construction,
    };
    let arc2 = Entity::Arc {
        center: c2,
        radius: r2,
        start_angle: s2,
        end_angle: e2,
        is_construction,
    };
    Some((arc1, arc2))
}

fn line_intersection_2d(p1: DVec2, d1: DVec2, p2: DVec2, d2: DVec2) -> Option<DVec2> {
    let denom = d1.x * d2.y - d1.y * d2.x;
    if denom.abs() < 1e-9 {
        return None;
    }
    let diff = p2 - p1;
    let t = (diff.x * d2.y - diff.y * d2.x) / denom;
    Some(p1 + d1 * t)
}

fn orient_arc_angles(from: f64, to: f64, tangent: DVec2, radial: DVec2) -> (f64, f64) {
    let cross = radial.x * tangent.y - radial.y * tangent.x;
    if cross >= 0.0 {
        (from, to)
    } else {
        (to, from)
    }
}

/// Aproksimasi offset kurva elips menjadi deretan Arc lingkaran tangensial (Bi-Arcs).
pub fn multi_arc_parallel_offset_ellipse(
    center: DVec2,
    rx: f64,
    ry: f64,
    offset_dist: f64,
    num_spans: usize,
    is_construction: bool,
) -> Vec<Entity> {
    let spans = num_spans.max(4);
    let mut entities = Vec::new();
    let sample_ellipse_offset = |t: f64| -> (DVec2, DVec2) {
        let p = center + DVec2::new(rx * t.cos(), ry * t.sin());
        let unnorm_normal = DVec2::new(ry * t.cos(), rx * t.sin());
        let normal = unnorm_normal.normalize_or_zero();
        let offset_p = p + normal * offset_dist;
        let tangent = DVec2::new(-normal.y, normal.x);
        (offset_p, tangent)
    };

    for i in 0..spans {
        let t0 = TAU * (i as f64) / (spans as f64);
        let t1 = TAU * ((i + 1) as f64) / (spans as f64);
        let (p0, v0) = sample_ellipse_offset(t0);
        let (p1, v1) = sample_ellipse_offset(t1);
        if let Some((arc1, arc2)) = biarc_fit(p0, v0, p1, v1, is_construction) {
            entities.push(arc1);
            entities.push(arc2);
        }
    }
    entities
}

/// Aproksimasi offset kurva spline menjadi deretan Arc lingkaran tangensial (Bi-Arcs).
pub fn multi_arc_parallel_offset_spline(
    points: &[DVec2],
    offset_dist: f64,
    num_spans: usize,
    is_construction: bool,
) -> Vec<Entity> {
    if points.len() < 2 {
        return Vec::new();
    }
    let dense = sample_catmull_rom(points, num_spans.max(4));
    if dense.len() < 2 {
        return Vec::new();
    }
    let n = dense.len();
    let mut offset_pts = Vec::with_capacity(n);
    let mut tangents = Vec::with_capacity(n);

    for i in 0..n {
        let t = if i == 0 {
            (dense[1] - dense[0]).normalize_or_zero()
        } else if i == n - 1 {
            (dense[n - 1] - dense[n - 2]).normalize_or_zero()
        } else {
            (dense[i + 1] - dense[i - 1]).normalize_or_zero()
        };
        let norm = DVec2::new(-t.y, t.x);
        offset_pts.push(dense[i] + norm * offset_dist);
        tangents.push(t);
    }

    let mut entities = Vec::new();
    for i in 0..(n - 1) {
        if let Some((arc1, arc2)) = biarc_fit(
            offset_pts[i],
            tangents[i],
            offset_pts[i + 1],
            tangents[i + 1],
            is_construction,
        ) {
            entities.push(arc1);
            entities.push(arc2);
        }
    }
    entities
}

/// Konversi entitas menjadi multi-arc tangensial offset jika berupa Ellipse atau Spline, atau entitas offset tunggal untuk Line/Circle/Arc.
pub fn offset_entity_multi_arc(
    entity: &Entity,
    reference_point: DVec2,
    num_spans: usize,
) -> Option<Vec<Entity>> {
    let is_construction = entity.is_construction();
    match entity {
        Entity::Ellipse {
            center,
            radius_x,
            radius_y,
            ..
        } => {
            let rel = reference_point - *center;
            let rx = *radius_x;
            let ry = *radius_y;
            if rx <= 1e-6 || ry <= 1e-6 {
                return None;
            }
            let norm_dist_sq = (rel.x / rx).powi(2) + (rel.y / ry).powi(2);
            let dist = entity.distance_to(reference_point);
            let signed_d = if norm_dist_sq >= 1.0 { dist } else { -dist };
            let arcs = multi_arc_parallel_offset_ellipse(
                *center,
                rx,
                ry,
                signed_d,
                num_spans,
                is_construction,
            );
            if arcs.is_empty() {
                None
            } else {
                Some(arcs)
            }
        }
        Entity::Spline { points, .. } => {
            if points.len() < 2 {
                return None;
            }
            let dist = entity.distance_to(reference_point);
            let sampled = sample_catmull_rom(points, 16);
            let mut nearest_seg = (sampled[0], sampled[1]);
            let mut min_seg_dist = f64::INFINITY;
            for w in sampled.windows(2) {
                let d = distance_point_segment(reference_point, w[0], w[1]);
                if d < min_seg_dist {
                    min_seg_dist = d;
                    nearest_seg = (w[0], w[1]);
                }
            }
            let seg_dir = (nearest_seg.1 - nearest_seg.0).normalize_or_zero();
            let seg_normal = DVec2::new(-seg_dir.y, seg_dir.x);
            let mid_seg = (nearest_seg.0 + nearest_seg.1) * 0.5;
            let signed_d = if (reference_point - mid_seg).dot(seg_normal) >= 0.0 {
                dist
            } else {
                -dist
            };

            let arcs = multi_arc_parallel_offset_spline(
                points,
                signed_d,
                num_spans,
                is_construction,
            );
            if arcs.is_empty() {
                None
            } else {
                Some(arcs)
            }
        }
        _ => offset_entity(entity, reference_point).map(|e| vec![e]),
    }
}

/// Parameter jarak sinar `origin + t * dir` (dengan `t > 1e-5`) terhadap suatu `Entity`.
pub fn ray_intersect_entity(origin: DVec2, dir: DVec2, entity: &Entity) -> Vec<f64> {
    let dir = dir.normalize_or_zero();
    if dir == DVec2::ZERO {
        return Vec::new();
    }
    match entity {
        Entity::Line { start, end, .. } => {
            let v = *end - *start;
            let denom = dir.x * v.y - dir.y * v.x;
            if denom.abs() < 1e-9 {
                return Vec::new();
            }
            let diff = *start - origin;
            let t = (diff.x * v.y - diff.y * v.x) / denom;
            let u = (diff.x * dir.y - diff.y * dir.x) / denom;
            if t > 1e-5 && (-1e-5..=1.0 + 1e-5).contains(&u) {
                vec![t]
            } else {
                Vec::new()
            }
        }
        Entity::Circle { center, radius, .. } => {
            let r = *radius;
            let m = origin - *center;
            let b = m.dot(dir);
            let c = m.length_squared() - r * r;
            let discr = b * b - c;
            if discr < 0.0 {
                return Vec::new();
            }
            let sqrt_discr = discr.sqrt();
            let mut ts = Vec::new();
            let t1 = -b - sqrt_discr;
            let t2 = -b + sqrt_discr;
            if t1 > 1e-5 {
                ts.push(t1);
            }
            if t2 > 1e-5 && (t2 - t1).abs() > 1e-6 {
                ts.push(t2);
            }
            ts
        }
        Entity::Arc {
            center,
            radius,
            start_angle,
            end_angle,
            ..
        } => {
            let r = *radius;
            let m = origin - *center;
            let b = m.dot(dir);
            let c = m.length_squared() - r * r;
            let discr = b * b - c;
            if discr < 0.0 {
                return Vec::new();
            }
            let sqrt_discr = discr.sqrt();
            let mut ts = Vec::new();
            for &t in &[-b - sqrt_discr, -b + sqrt_discr] {
                if t > 1e-5 {
                    let q = origin + dir * t;
                    let to_q = q - *center;
                    let angle = to_q.y.atan2(to_q.x);
                    if angle_in_range(angle, *start_angle, *end_angle) {
                        ts.push(t);
                    }
                }
            }
            ts
        }
        Entity::Ellipse {
            center,
            radius_x,
            radius_y,
            ..
        } => {
            let rx = *radius_x;
            let ry = *radius_y;
            if rx <= 1e-6 || ry <= 1e-6 {
                return Vec::new();
            }
            let m = origin - *center;
            let mx = m.x / rx;
            let my = m.y / ry;
            let dx = dir.x / rx;
            let dy = dir.y / ry;
            let a = dx * dx + dy * dy;
            if a < 1e-12 {
                return Vec::new();
            }
            let b = 2.0 * (mx * dx + my * dy);
            let c = mx * mx + my * my - 1.0;
            let discr = b * b - 4.0 * a * c;
            if discr < 0.0 {
                return Vec::new();
            }
            let sqrt_discr = discr.sqrt();
            let mut ts = Vec::new();
            let t1 = (-b - sqrt_discr) / (2.0 * a);
            let t2 = (-b + sqrt_discr) / (2.0 * a);
            if t1 > 1e-5 {
                ts.push(t1);
            }
            if t2 > 1e-5 && (t2 - t1).abs() > 1e-6 {
                ts.push(t2);
            }
            ts
        }
        Entity::Spline { points, .. } => {
            let sampled = sample_catmull_rom(points, 24);
            let mut ts = Vec::new();
            for w in sampled.windows(2) {
                let seg = Entity::line(w[0], w[1]);
                let seg_ts = ray_intersect_entity(origin, dir, &seg);
                ts.extend(seg_ts);
            }
            ts.sort_by(|a, b| a.partial_cmp(b).unwrap());
            ts.dedup_by(|a, b| (*a - *b).abs() < 1e-5);
            ts
        }
    }
}

/// Memperpanjang segmen garis `line_id` pada `sketch` sampai menyentuh kurva pembatas terdekat.
/// `click_pos` menentukan ujung garis mana yang akan diperpanjang.
pub fn extend_segment(
    sketch: &Sketch,
    line_id: EntityId,
    click_pos: DVec2,
) -> Option<Entity> {
    let Entity::Line { start, end, is_construction } = sketch.entities.get(line_id)?.clone() else {
        return None;
    };
    let t_click = project_t(start, end, click_pos);
    let extend_end = t_click >= 0.5;

    let (origin, dir) = if extend_end {
        (end, (end - start).normalize_or_zero())
    } else {
        (start, (start - end).normalize_or_zero())
    };

    if dir == DVec2::ZERO {
        return None;
    }

    let mut min_t = f64::INFINITY;
    for (id, entity) in &sketch.entities {
        if id == line_id || sketch.is_hidden(id) {
            continue;
        }
        for t in ray_intersect_entity(origin, dir, entity) {
            if t > 1e-5 && t < min_t {
                min_t = t;
            }
        }
    }

    if min_t.is_finite() {
        let target_pt = origin + dir * min_t;
        if extend_end {
            Some(Entity::Line {
                start,
                end: target_pt,
                is_construction,
            })
        } else {
            Some(Entity::Line {
                start: target_pt,
                end,
                is_construction,
            })
        }
    } else {
        None
    }
}

/// Helper preview perpanjangan: mengembalikan pasangan titik segmen tambahan (ujung garis -> titik potong target).
pub fn extend_preview(
    sketch: &Sketch,
    line_id: EntityId,
    click_pos: DVec2,
) -> Option<(DVec2, DVec2)> {
    let Entity::Line { start, end, .. } = sketch.entities.get(line_id)?.clone() else {
        return None;
    };
    let t_click = project_t(start, end, click_pos);
    let extend_end = t_click >= 0.5;

    let (origin, dir) = if extend_end {
        (end, (end - start).normalize_or_zero())
    } else {
        (start, (start - end).normalize_or_zero())
    };

    if dir == DVec2::ZERO {
        return None;
    }

    let mut min_t = f64::INFINITY;
    for (id, entity) in &sketch.entities {
        if id == line_id || sketch.is_hidden(id) {
            continue;
        }
        for t in ray_intersect_entity(origin, dir, entity) {
            if t > 1e-5 && t < min_t {
                min_t = t;
            }
        }
    }

    if min_t.is_finite() {
        Some((origin, origin + dir * min_t))
    } else {
        None
    }
}


