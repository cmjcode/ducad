//! Konversi entitas `cadraw-sketch` + indikator snap menjadi `LineVertex`
//! untuk viewport.

use std::collections::HashSet;

use cadraw_sketch::{Entity, EntityId, Sketch};
use glam::DVec2;

use crate::grid::LineVertex;
use crate::plane::SketchPlane;

pub mod gizmo;
pub mod glyphs;
pub mod measure;

#[cfg(test)]
mod tests;

pub use gizmo::{
    double_arrow_gizmo_lines, shapr3d_transform_gizmo_lines, solid_double_arrow_gizmo_mesh,
    solid_shapr3d_transform_gizmo_mesh, vertex_dot_markers, vertex_fillet_marker_lines,
    TransformGizmoPart,
};
pub use glyphs::{picked_point_glyph, removal_preview_lines, snap_glyph};
pub use measure::{
    dashed_line_3d, dimension_leader_lines, measurement_arrowheads, measurement_lines,
};

const Z_OFFSET: f32 = 0.02;
const COLOR_NORMAL: [f32; 4] = [0.86, 0.87, 0.90, 1.0];
const COLOR_HOVER: [f32; 4] = [1.0, 0.82, 0.25, 1.0];
const COLOR_SELECTED: [f32; 4] = [0.30, 0.65, 1.0, 1.0];
const COLOR_PREVIEW: [f32; 4] = [0.55, 0.90, 0.55, 0.85];
const COLOR_SNAP: [f32; 4] = [1.0, 0.55, 0.15, 1.0];
const COLOR_REMOVAL: [f32; 4] = [0.95, 0.25, 0.25, 0.95];
const COLOR_PICKED: [f32; 4] = [0.65, 0.35, 0.95, 1.0];
const COLOR_MEASURE: [f32; 4] = [1.0, 0.95, 0.35, 1.0];
const COLOR_INACTIVE_PLANE: [f32; 4] = [0.65, 0.72, 0.82, 0.85];
const ARC_SEGMENTS_FULL: usize = 48;

pub(crate) fn to3(plane: &SketchPlane, p: DVec2) -> [f32; 3] {
    let w = plane.to_world(p, Z_OFFSET);
    [w.x, w.y, w.z]
}

/// Garis untuk seluruh entitas sketch pada bidang tertentu, diwarnai menurut status hover/pilih.
pub fn entity_lines(
    sketch: &Sketch,
    hovered: Option<EntityId>,
    selected: &HashSet<EntityId>,
    plane: &SketchPlane,
) -> Vec<LineVertex> {
    let mut verts = Vec::new();
    for (id, entity) in sketch.entities.iter() {
        let color = if Some(id) == hovered {
            COLOR_HOVER
        } else if selected.contains(&id) {
            COLOR_SELECTED
        } else {
            COLOR_NORMAL
        };
        push_entity(&mut verts, entity, color, plane);
    }
    verts
}

/// Garis untuk seluruh entitas sketch pada bidang non-aktif, dirender di koordinat 3D aslinya.
pub fn inactive_entity_lines(sketch: &Sketch, plane: &SketchPlane) -> Vec<LineVertex> {
    let mut verts = Vec::new();
    for (_id, entity) in sketch.entities.iter() {
        push_entity(&mut verts, entity, COLOR_INACTIVE_PLANE, plane);
    }
    verts
}

/// Garis rubber-band untuk entitas yang sedang digambar pada bidang aktif.
pub fn preview_lines(entity: &Entity, plane: &SketchPlane) -> Vec<LineVertex> {
    let mut verts = Vec::new();
    push_entity(&mut verts, entity, COLOR_PREVIEW, plane);
    verts
}

pub(crate) fn push_entity(
    verts: &mut Vec<LineVertex>,
    entity: &Entity,
    color: [f32; 4],
    plane: &SketchPlane,
) {
    match entity {
        Entity::Line { start, end } => {
            verts.push(LineVertex {
                position: to3(plane, *start),
                color,
            });
            verts.push(LineVertex {
                position: to3(plane, *end),
                color,
            });
        }
        Entity::Circle { center, radius } => {
            push_arc(verts, *center, *radius, 0.0, std::f64::consts::TAU, color, plane)
        }
        Entity::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => push_arc(verts, *center, *radius, *start_angle, *end_angle, color, plane),
        Entity::Ellipse {
            center,
            radius_x,
            radius_y,
        } => push_ellipse(verts, *center, *radius_x, *radius_y, color, plane),
    }
}

pub(crate) fn push_ellipse(
    verts: &mut Vec<LineVertex>,
    center: DVec2,
    radius_x: f64,
    radius_y: f64,
    color: [f32; 4],
    plane: &SketchPlane,
) {
    let tau = std::f64::consts::TAU;
    let mut prev = center + DVec2::new(radius_x, 0.0);
    for i in 1..=ARC_SEGMENTS_FULL {
        let t = tau * (i as f64 / ARC_SEGMENTS_FULL as f64);
        let p = center + DVec2::new(radius_x * t.cos(), radius_y * t.sin());
        verts.push(LineVertex {
            position: to3(plane, prev),
            color,
        });
        verts.push(LineVertex {
            position: to3(plane, p),
            color,
        });
        prev = p;
    }
}

pub(crate) fn push_arc(
    verts: &mut Vec<LineVertex>,
    center: DVec2,
    radius: f64,
    start: f64,
    end: f64,
    color: [f32; 4],
    plane: &SketchPlane,
) {
    let tau = std::f64::consts::TAU;
    let span = {
        let s = end - start;
        if s <= 0.0 {
            s + tau
        } else {
            s
        }
    };
    let steps = ((ARC_SEGMENTS_FULL as f64 * span / tau).ceil() as usize).max(4);
    let mut prev = center + DVec2::new(radius * start.cos(), radius * start.sin());
    for i in 1..=steps {
        let t = start + span * (i as f64 / steps as f64);
        let p = center + DVec2::new(radius * t.cos(), radius * t.sin());
        verts.push(LineVertex {
            position: to3(plane, prev),
            color,
        });
        verts.push(LineVertex {
            position: to3(plane, p),
            color,
        });
        prev = p;
    }
}
