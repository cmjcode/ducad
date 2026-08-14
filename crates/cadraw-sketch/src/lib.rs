//! Sketching 2D CADRAW: entitas, snapping, dan (Fase 2) constraint solver.
//!
//! Fase 1 mengisi crate ini dengan: Line/Arc/Circle/Spline, snapping engine
//! (endpoint/midpoint/center/intersection/perpendicular/tangent/grid),
//! dan hit-testing dengan toleransi adaptif mouse vs jari.

use glam::DVec2;

slotmap::new_key_type! {
    /// Identitas stabil entitas sketch.
    pub struct EntityId;
}

/// Entitas sketch 2D (koordinat lokal bidang sketch, presisi f64).
#[derive(Debug, Clone, PartialEq)]
pub enum Entity {
    Line { start: DVec2, end: DVec2 },
    Circle { center: DVec2, radius: f64 },
    Arc { center: DVec2, radius: f64, start_angle: f64, end_angle: f64 },
}

/// Satu sketch pada sebuah bidang kerja.
#[derive(Debug, Default)]
pub struct Sketch {
    pub entities: slotmap::SlotMap<EntityId, Entity>,
}
