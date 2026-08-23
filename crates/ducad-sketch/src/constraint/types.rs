use glam::DVec2;
use serde::{Deserialize, Serialize};

use crate::entity::{Entity, EntityId};
use crate::sketch::Sketch;

/// Rujukan ke satu titik pada entitas — dipakai constraint yang butuh
/// titik spesifik (Coincident, Fixed, Distance), bukan seluruh entitas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointRef {
    LineStart(EntityId),
    LineEnd(EntityId),
    /// Center Circle/Arc/Ellipse.
    Center(EntityId),
}

impl PointRef {
    pub fn entity_id(&self) -> EntityId {
        match self {
            PointRef::LineStart(id) | PointRef::LineEnd(id) | PointRef::Center(id) => *id,
        }
    }
}

/// Posisi `pr` saat ini di `sketch`.
pub fn point_ref_position(sketch: &Sketch, pr: &PointRef) -> Option<DVec2> {
    let entity = sketch.entities.get(pr.entity_id())?;
    match (entity, pr) {
        (Entity::Line { start, .. }, PointRef::LineStart(_)) => Some(*start),
        (Entity::Line { end, .. }, PointRef::LineEnd(_)) => Some(*end),
        (
            Entity::Circle { center, .. }
            | Entity::Arc { center, .. }
            | Entity::Ellipse { center, .. },
            PointRef::Center(_),
        ) => Some(*center),
        (Entity::Spline { points }, PointRef::LineStart(_)) => points.first().copied(),
        (Entity::Spline { points }, PointRef::LineEnd(_)) => points.last().copied(),
        (Entity::Spline { points }, PointRef::Center(_)) => {
            if points.is_empty() {
                None
            } else {
                Some(points.iter().copied().sum::<DVec2>() / (points.len() as f64))
            }
        }
        _ => None,
    }
}

/// Satu constraint geometris/dimensional.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Constraint {
    Coincident { a: PointRef, b: PointRef },
    Horizontal { line: EntityId },
    Vertical { line: EntityId },
    Parallel { a: EntityId, b: EntityId },
    Perpendicular { a: EntityId, b: EntityId },
    EqualLength { a: EntityId, b: EntityId },
    EqualRadius { a: EntityId, b: EntityId },
    Fixed { point: PointRef, target: DVec2 },
    Distance { a: PointRef, b: PointRef, value: f64 },
    Radius { entity: EntityId, value: f64 },
    /// Sudut CCW dari arah `a` ke arah `b`, radian, kontinu di (-π, π].
    Angle { a: EntityId, b: EntityId, value: f64 },
    Tangent { a: EntityId, b: EntityId },
    /// Titik `a` dan `b` saling cermin melintasi garis `axis`.
    Symmetric { a: PointRef, b: PointRef, axis: EntityId },
}
