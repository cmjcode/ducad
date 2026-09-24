use glam::DVec2;
use serde::{Deserialize, Serialize};

use crate::entity::{Entity, EntityId};
use crate::sketch::Sketch;

/// Rujukan ke satu titik pada entitas — dipakai constraint yang butuh
/// titik spesifik (Coincident, Fixed, Distance), bukan seluruh entitas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PointRef {
    LineStart(EntityId),
    LineEnd(EntityId),
    /// Center Circle/Arc/Ellipse.
    Center(EntityId),
    /// Node ke-`node` pada subpath ke-`sub` dari `Entity::Path`.
    PathNode {
        id: EntityId,
        sub: u16,
        node: u32,
    },
}

impl PointRef {
    pub fn entity_id(&self) -> EntityId {
        match self {
            PointRef::LineStart(id) | PointRef::LineEnd(id) | PointRef::Center(id) => *id,
            PointRef::PathNode { id, .. } => *id,
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
        (Entity::Spline { points, .. }, PointRef::LineStart(_)) => points.first().copied(),
        (Entity::Spline { points, .. }, PointRef::LineEnd(_)) => points.last().copied(),
        (Entity::Spline { points, .. }, PointRef::Center(_)) => {
            if points.is_empty() {
                None
            } else {
                Some(points.iter().copied().sum::<DVec2>() / (points.len() as f64))
            }
        }
        (Entity::Path { subpaths, .. }, PointRef::PathNode { sub, node, .. }) => {
            subpaths.get(*sub as usize)?.node(*node as usize)
        }
        _ => None,
    }
}

/// Satu constraint geometris/dimensional.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Constraint {
    Coincident {
        a: PointRef,
        b: PointRef,
    },
    Horizontal {
        line: EntityId,
    },
    Vertical {
        line: EntityId,
    },
    HorizontalPoints {
        a: PointRef,
        b: PointRef,
    },
    VerticalPoints {
        a: PointRef,
        b: PointRef,
    },
    Parallel {
        a: EntityId,
        b: EntityId,
    },
    Perpendicular {
        a: EntityId,
        b: EntityId,
    },
    EqualLength {
        a: EntityId,
        b: EntityId,
    },
    EqualRadius {
        a: EntityId,
        b: EntityId,
    },
    Fixed {
        point: PointRef,
        target: DVec2,
    },
    Distance {
        a: PointRef,
        b: PointRef,
        value: f64,
    },
    Radius {
        entity: EntityId,
        value: f64,
    },
    /// Sudut CCW dari arah `a` ke arah `b`, radian, kontinu di (-π, π].
    Angle {
        a: EntityId,
        b: EntityId,
        value: f64,
    },
    Tangent {
        a: EntityId,
        b: EntityId,
    },
    /// Titik `a` dan `b` saling cermin melintasi garis `axis`.
    Symmetric {
        a: PointRef,
        b: PointRef,
        axis: EntityId,
    },

    // ---- P1.2: constraint yang sebelumnya tidak ada ----
    /// Titik `point` menempel pada kurva `curve` (garis tak hingga untuk
    /// Line, lingkaran untuk Circle/Arc). Berbeda dari `Coincident` yang
    /// hanya mengikat titik-ke-TITIK, ini mengikat titik-ke-KURVA sehingga
    /// titiknya masih bebas meluncur sepanjang kurva — kebutuhan sehari-hari
    /// yang sebelumnya tidak bisa dinyatakan sama sekali.
    PointOnCurve {
        point: PointRef,
        curve: EntityId,
    },
    /// `point` berada tepat di tengah `line`.
    Midpoint {
        point: PointRef,
        line: EntityId,
    },
    /// Pusat dua entitas radial berimpit.
    Concentric {
        a: EntityId,
        b: EntityId,
    },
    /// Dua garis terletak pada satu garis lurus yang sama (sejajar DAN
    /// segaris) — lebih kuat dari `Parallel`.
    Collinear {
        a: EntityId,
        b: EntityId,
    },
}

impl PointRef {
    fn map_entity_id(&mut self, f: &impl Fn(EntityId) -> EntityId) {
        match self {
            PointRef::LineStart(id) | PointRef::LineEnd(id) | PointRef::Center(id) => *id = f(*id),
            PointRef::PathNode { id, .. } => *id = f(*id),
        }
    }
}

impl Constraint {
    /// Ganti setiap `EntityId` yang dirujuk lewat `f` — dipakai saat undo
    /// memulihkan entitas terhapus dengan id baru.
    pub fn map_entity_ids(&mut self, f: impl Fn(EntityId) -> EntityId) {
        match self {
            Constraint::Coincident { a, b }
            | Constraint::Distance { a, b, .. }
            | Constraint::HorizontalPoints { a, b }
            | Constraint::VerticalPoints { a, b } => {
                a.map_entity_id(&f);
                b.map_entity_id(&f);
            }
            Constraint::Horizontal { line } | Constraint::Vertical { line } => *line = f(*line),
            Constraint::Parallel { a, b }
            | Constraint::Perpendicular { a, b }
            | Constraint::EqualLength { a, b }
            | Constraint::EqualRadius { a, b }
            | Constraint::Angle { a, b, .. }
            | Constraint::Tangent { a, b }
            | Constraint::Concentric { a, b }
            | Constraint::Collinear { a, b } => {
                *a = f(*a);
                *b = f(*b);
            }
            Constraint::Fixed { point, .. } => point.map_entity_id(&f),
            Constraint::Radius { entity, .. } => *entity = f(*entity),
            Constraint::Symmetric { a, b, axis } => {
                a.map_entity_id(&f);
                b.map_entity_id(&f);
                *axis = f(*axis);
            }
            Constraint::PointOnCurve { point, curve } => {
                point.map_entity_id(&f);
                *curve = f(*curve);
            }
            Constraint::Midpoint { point, line } => {
                point.map_entity_id(&f);
                *line = f(*line);
            }
        }
    }
}
