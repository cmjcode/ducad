use glam::DVec2;
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

use crate::constraint::PointRef;

slotmap::new_key_type! {
    /// Identitas stabil entitas sketch.
    pub struct EntityId;
}

/// Entitas sketch 2D (koordinat lokal bidang sketch, presisi f64).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Entity {
    Line {
        start: DVec2,
        end: DVec2,
    },
    Circle {
        center: DVec2,
        radius: f64,
    },
    Arc {
        center: DVec2,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    },
    /// Ellips axis-aligned (sumbu sejajar X/Y).
    Ellipse {
        center: DVec2,
        radius_x: f64,
        radius_y: f64,
    },
    /// Kurva Spline halus yang melalui deretan titik kontrol/fit (Catmull-Rom).
    Spline {
        points: Vec<DVec2>,
    },
}

impl Entity {
    /// Titik-titik endpoint sebagai kandidat snap "endpoint".
    pub fn endpoints(&self) -> Vec<DVec2> {
        match self {
            Entity::Line { start, end } => vec![*start, *end],
            Entity::Circle { center, radius } => vec![
                *center + DVec2::new(*radius, 0.0),
                *center + DVec2::new(0.0, *radius),
                *center + DVec2::new(-*radius, 0.0),
                *center + DVec2::new(0.0, -*radius),
            ],
            Entity::Ellipse {
                center,
                radius_x,
                radius_y,
            } => vec![
                *center + DVec2::new(*radius_x, 0.0),
                *center + DVec2::new(0.0, *radius_y),
                *center + DVec2::new(-*radius_x, 0.0),
                *center + DVec2::new(0.0, -*radius_y),
            ],
            Entity::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => vec![
                *center + DVec2::new(radius * start_angle.cos(), radius * start_angle.sin()),
                *center + DVec2::new(radius * end_angle.cos(), radius * end_angle.sin()),
            ],
            Entity::Spline { points } => {
                if points.is_empty() {
                    vec![]
                } else if points.len() == 1 {
                    vec![points[0]]
                } else {
                    let mut pts = vec![points[0], *points.last().unwrap()];
                    for pt in &points[1..points.len() - 1] {
                        pts.push(*pt);
                    }
                    pts
                }
            }
        }
    }

    pub fn midpoint(&self) -> Option<DVec2> {
        match self {
            Entity::Line { start, end } => Some((*start + *end) * 0.5),
            Entity::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                let tau = std::f64::consts::TAU;
                let span = {
                    let s = end_angle - start_angle;
                    if s <= 0.0 {
                        s + tau
                    } else {
                        s
                    }
                };
                let mid_angle = start_angle + span * 0.5;
                Some(*center + DVec2::new(radius * mid_angle.cos(), radius * mid_angle.sin()))
            }
            Entity::Spline { points } => {
                if points.len() >= 2 {
                    let sampled = sample_catmull_rom(points, 8);
                    if !sampled.is_empty() {
                        return Some(sampled[sampled.len() / 2]);
                    }
                }
                None
            }
            _ => None,
        }
    }

    pub fn center(&self) -> Option<DVec2> {
        match self {
            Entity::Circle { center, .. }
            | Entity::Arc { center, .. }
            | Entity::Ellipse { center, .. } => Some(*center),
            Entity::Spline { points } => {
                if points.is_empty() {
                    None
                } else {
                    let sum = points.iter().copied().sum::<DVec2>();
                    Some(sum / (points.len() as f64))
                }
            }
            Entity::Line { .. } => None,
        }
    }

    /// Sama seperti `endpoints()`, tapi berpasangan dengan `PointRef` sumbernya.
    pub fn endpoint_refs(&self, id: EntityId) -> Vec<(PointRef, DVec2)> {
        match self {
            Entity::Line { start, end } => vec![
                (PointRef::LineStart(id), *start),
                (PointRef::LineEnd(id), *end),
            ],
            Entity::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => vec![
                (
                    PointRef::LineStart(id),
                    *center + DVec2::new(radius * start_angle.cos(), radius * start_angle.sin()),
                ),
                (
                    PointRef::LineEnd(id),
                    *center + DVec2::new(radius * end_angle.cos(), radius * end_angle.sin()),
                ),
            ],
            Entity::Circle { center, radius } => vec![
                (PointRef::Center(id), *center + DVec2::new(*radius, 0.0)),
                (PointRef::Center(id), *center + DVec2::new(0.0, *radius)),
                (PointRef::Center(id), *center + DVec2::new(-*radius, 0.0)),
                (PointRef::Center(id), *center + DVec2::new(0.0, -*radius)),
            ],
            Entity::Ellipse {
                center,
                radius_x,
                radius_y,
            } => vec![
                (PointRef::Center(id), *center + DVec2::new(*radius_x, 0.0)),
                (PointRef::Center(id), *center + DVec2::new(0.0, *radius_y)),
                (PointRef::Center(id), *center + DVec2::new(-*radius_x, 0.0)),
                (PointRef::Center(id), *center + DVec2::new(0.0, -*radius_y)),
            ],
            Entity::Spline { points } => {
                if points.len() >= 2 {
                    let mut refs = vec![
                        (PointRef::LineStart(id), points[0]),
                        (PointRef::LineEnd(id), *points.last().unwrap()),
                    ];
                    for pt in &points[1..points.len() - 1] {
                        refs.push((PointRef::Center(id), *pt));
                    }
                    refs
                } else if points.len() == 1 {
                    vec![(PointRef::LineStart(id), points[0])]
                } else {
                    vec![]
                }
            }
        }
    }

    /// Sama seperti `center()`, berpasangan dengan `PointRef::Center`.
    pub fn center_ref(&self, id: EntityId) -> Option<(PointRef, DVec2)> {
        self.center().map(|c| (PointRef::Center(id), c))
    }

    /// Jarak titik ke entitas — dipakai hit-testing seleksi & snap.
    pub fn distance_to(&self, p: DVec2) -> f64 {
        match self {
            Entity::Line { start, end } => distance_point_segment(p, *start, *end),
            Entity::Circle { center, radius } => ((p - *center).length() - radius).abs(),
            Entity::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                let to_p = p - *center;
                let angle = to_p.y.atan2(to_p.x);
                if angle_in_range(angle, *start_angle, *end_angle) {
                    (to_p.length() - radius).abs()
                } else {
                    self.endpoints()
                        .into_iter()
                        .map(|e| (p - e).length())
                        .fold(f64::INFINITY, f64::min)
                }
            }
            Entity::Ellipse {
                center,
                radius_x,
                radius_y,
            } => {
                const SAMPLES: usize = 64;
                (0..SAMPLES)
                    .map(|i| {
                        let t = TAU * (i as f64) / (SAMPLES as f64);
                        let boundary =
                            *center + DVec2::new(radius_x * t.cos(), radius_y * t.sin());
                        (p - boundary).length()
                    })
                    .fold(f64::INFINITY, f64::min)
            }
            Entity::Spline { points } => {
                if points.is_empty() {
                    return f64::INFINITY;
                }
                if points.len() == 1 {
                    return (p - points[0]).length();
                }
                let sampled = sample_catmull_rom(points, 16);
                let mut min_d = f64::INFINITY;
                for w in sampled.windows(2) {
                    let d = distance_point_segment(p, w[0], w[1]);
                    if d < min_d {
                        min_d = d;
                    }
                }
                min_d
            }
        }
    }
}

/// Evaluasi titik pada kurva Catmull-Rom spline yang melewati `points`.
pub fn sample_catmull_rom(points: &[DVec2], samples_per_span: usize) -> Vec<DVec2> {
    let n = points.len();
    if n < 2 {
        return points.to_vec();
    }
    if n == 2 {
        return vec![points[0], points[1]];
    }

    let samples = samples_per_span.max(2);
    let mut result = Vec::with_capacity((n - 1) * samples + 1);

    for i in 0..(n - 1) {
        let p0 = if i == 0 { points[0] } else { points[i - 1] };
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = if i + 2 < n { points[i + 2] } else { points[n - 1] };

        for s in 0..samples {
            let t = s as f64 / samples as f64;
            let t2 = t * t;
            let t3 = t2 * t;

            let pt = 0.5
                * (2.0 * p1
                    + (-p0 + p2) * t
                    + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                    + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3);
            result.push(pt);
        }
    }
    result.push(*points.last().unwrap());
    result
}

pub(crate) fn distance_point_segment(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq < f64::EPSILON {
        return (p - a).length();
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

pub(crate) fn angle_in_range(angle: f64, start: f64, end: f64) -> bool {
    let norm = |a: f64| ((a % TAU) + TAU) % TAU;
    let (a, s, e) = (norm(angle), norm(start), norm(end));
    if s <= e {
        a >= s && a <= e
    } else {
        a >= s || a <= e
    }
}
