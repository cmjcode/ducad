use glam::DVec2;
use std::f64::consts::TAU;

use crate::entity::{Entity, EntityId};
use crate::sketch::Sketch;
use crate::snap::line_intersection_params;

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
    match entity {
        Entity::Line { start, end } => {
            let dir = (*end - *start).normalize_or_zero();
            if dir == DVec2::ZERO {
                return None;
            }
            let normal = DVec2::new(-dir.y, dir.x);
            let offset_vec = normal * (reference_point - *start).dot(normal);
            Some(Entity::Line {
                start: *start + offset_vec,
                end: *end + offset_vec,
            })
        }
        Entity::Circle { center, .. } => {
            let radius = (reference_point - *center).length();
            (radius > 1e-6).then_some(Entity::Circle {
                center: *center,
                radius,
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
            })
        }
        Entity::Ellipse { .. } => None,
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

    Some(match entity {
        Entity::Line { start, end } => Entity::Line {
            start: reflect(*start),
            end: reflect(*end),
        },
        Entity::Circle { center, radius } => Entity::Circle {
            center: reflect(*center),
            radius: *radius,
        },
        Entity::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let axis_angle = axis_dir.y.atan2(axis_dir.x);
            let reflect_angle = |a: f64| 2.0 * axis_angle - a;
            Entity::Arc {
                center: reflect(*center),
                radius: *radius,
                start_angle: reflect_angle(*end_angle),
                end_angle: reflect_angle(*start_angle),
            }
        }
        Entity::Ellipse {
            center,
            radius_x,
            radius_y,
        } => Entity::Ellipse {
            center: reflect(*center),
            radius_x: *radius_x,
            radius_y: *radius_y,
        },
    })
}

/// Geser entitas sepanjang bidang sketsa-nya (u,v lokal) sejauh `delta`.
pub fn translate_entity(entity: &Entity, delta: DVec2) -> Entity {
    match entity {
        Entity::Line { start, end } => Entity::Line {
            start: *start + delta,
            end: *end + delta,
        },
        Entity::Circle { center, radius } => Entity::Circle {
            center: *center + delta,
            radius: *radius,
        },
        Entity::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => Entity::Arc {
            center: *center + delta,
            radius: *radius,
            start_angle: *start_angle,
            end_angle: *end_angle,
        },
        Entity::Ellipse {
            center,
            radius_x,
            radius_y,
        } => Entity::Ellipse {
            center: *center + delta,
            radius_x: *radius_x,
            radius_y: *radius_y,
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
            Entity::Line { start, end } => line_intersection_params(line, (*start, *end)),
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
