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
}

impl Entity {
    /// Titik-titik endpoint sebagai kandidat snap "endpoint".
    pub fn endpoints(&self) -> Vec<DVec2> {
        match self {
            Entity::Line { start, end } => vec![*start, *end],
            Entity::Circle { .. } | Entity::Ellipse { .. } => vec![],
            Entity::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => vec![
                *center + DVec2::new(radius * start_angle.cos(), radius * start_angle.sin()),
                *center + DVec2::new(radius * end_angle.cos(), radius * end_angle.sin()),
            ],
        }
    }

    pub fn midpoint(&self) -> Option<DVec2> {
        match self {
            Entity::Line { start, end } => Some((*start + *end) * 0.5),
            _ => None,
        }
    }

    pub fn center(&self) -> Option<DVec2> {
        match self {
            Entity::Circle { center, .. }
            | Entity::Arc { center, .. }
            | Entity::Ellipse { center, .. } => Some(*center),
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
            _ => vec![],
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
        }
    }
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
