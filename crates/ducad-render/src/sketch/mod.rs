//! Konversi entitas `ducad-sketch` + indikator snap menjadi `LineVertex`
//! untuk viewport.

use std::collections::HashSet;

use ducad_sketch::{Entity, EntityId, Sketch};
use glam::DVec2;

use crate::grid::LineVertex;
use crate::plane::SketchPlane;

pub mod gizmo;
pub mod glyphs;
pub mod measure;

#[cfg(test)]
mod tests;

pub use gizmo::{
    double_arrow_gizmo_lines, shapr3d_transform_gizmo_lines, solid_directional_arrow_mesh,
    solid_double_arrow_gizmo_mesh, solid_shapr3d_transform_gizmo_mesh, vertex_dot_markers,
    vertex_fillet_marker_lines, TransformGizmoPart,
};
pub use glyphs::{candidate_snap_points_glyphs, picked_point_glyph, removal_preview_lines, snap_glyph};
pub use measure::{
    dashed_line_3d, dimension_leader_lines, measurement_arrowheads, measurement_lines,
};

const Z_OFFSET: f32 = 0.02;
const COLOR_NORMAL: [f32; 4] = [0.86, 0.87, 0.90, 1.0];
const COLOR_HOVER: [f32; 4] = [1.0, 0.82, 0.25, 1.0];
const COLOR_SELECTED: [f32; 4] = [0.30, 0.65, 1.0, 1.0];
const COLOR_PREVIEW: [f32; 4] = [0.55, 0.90, 0.55, 0.85];
const COLOR_CONSTRUCTION: [f32; 4] = [1.0, 0.55, 0.12, 1.0];
const COLOR_SNAP: [f32; 4] = [1.0, 0.55, 0.15, 1.0];
const COLOR_REMOVAL: [f32; 4] = [0.95, 0.25, 0.25, 0.95];
const COLOR_PICKED: [f32; 4] = [0.65, 0.35, 0.95, 1.0];
const COLOR_MEASURE: [f32; 4] = [1.0, 0.95, 0.35, 1.0];
const COLOR_INACTIVE_PLANE: [f32; 4] = [0.65, 0.72, 0.82, 0.85];
const COLOR_INACTIVE_CONSTRUCTION: [f32; 4] = [0.85, 0.55, 0.25, 0.75];
const ARC_SEGMENTS_FULL: usize = 48;

pub(crate) fn to3(plane: &SketchPlane, p: DVec2) -> [f32; 3] {
    let w = plane.to_world(p, Z_OFFSET);
    [w.x, w.y, w.z]
}

/// Garis untuk seluruh entitas sketch pada bidang tertentu, diwarnai menurut status hover/pilih/konstruksi.
pub fn entity_lines(
    sketch: &Sketch,
    hovered: Option<EntityId>,
    selected: &HashSet<EntityId>,
    plane: &SketchPlane,
) -> Vec<LineVertex> {
    let mut verts = Vec::new();
    for (id, entity) in sketch.entities.iter() {
        if sketch.is_hidden(id) {
            continue;
        }
        let color = if Some(id) == hovered {
            COLOR_HOVER
        } else if selected.contains(&id) {
            COLOR_SELECTED
        } else if entity.is_construction() {
            COLOR_CONSTRUCTION
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
    for (id, entity) in sketch.entities.iter() {
        if sketch.is_hidden(id) {
            continue;
        }
        let color = if entity.is_construction() {
            COLOR_INACTIVE_CONSTRUCTION
        } else {
            COLOR_INACTIVE_PLANE
        };
        push_entity(&mut verts, entity, color, plane);
    }
    verts
}

/// Garis rubber-band untuk entitas yang sedang digambar pada bidang aktif.
pub fn preview_lines(entity: &Entity, plane: &SketchPlane) -> Vec<LineVertex> {
    let mut verts = Vec::new();
    let color = if entity.is_construction() {
        COLOR_CONSTRUCTION
    } else {
        COLOR_PREVIEW
    };
    push_entity(&mut verts, entity, color, plane);
    verts
}

pub(crate) fn push_entity(
    verts: &mut Vec<LineVertex>,
    entity: &Entity,
    color: [f32; 4],
    plane: &SketchPlane,
) {
    if entity.is_construction() {
        push_construction_entity(verts, entity, color, plane);
        return;
    }

    match entity {
        Entity::Line { start, end, .. } => {
            verts.push(LineVertex {
                position: to3(plane, *start),
                color,
            });
            verts.push(LineVertex {
                position: to3(plane, *end),
                color,
            });
        }
        Entity::Circle { center, radius, .. } => {
            push_arc(verts, *center, *radius, 0.0, std::f64::consts::TAU, color, plane)
        }
        Entity::Arc {
            center,
            radius,
            start_angle,
            end_angle,
            ..
        } => push_arc(verts, *center, *radius, *start_angle, *end_angle, color, plane),
        Entity::Ellipse {
            center,
            radius_x,
            radius_y,
            ..
        } => push_ellipse(verts, *center, *radius_x, *radius_y, color, plane),
        Entity::Spline { points, .. } => push_spline(verts, points, color, plane),
    }
}

pub(crate) fn push_construction_entity(
    verts: &mut Vec<LineVertex>,
    entity: &Entity,
    color: [f32; 4],
    plane: &SketchPlane,
) {
    const DASH_LEN: f64 = 2.5;
    match entity {
        Entity::Line { start, end, .. } => {
            verts.extend(dashed_line_3d(
                to3(plane, *start),
                to3(plane, *end),
                DASH_LEN as f32,
                color,
            ));
        }
        Entity::Circle { center, radius, .. } => {
            let tau = std::f64::consts::TAU;
            let mut pts = Vec::with_capacity(ARC_SEGMENTS_FULL + 1);
            for i in 0..=ARC_SEGMENTS_FULL {
                let t = tau * (i as f64 / ARC_SEGMENTS_FULL as f64);
                pts.push(*center + DVec2::new(radius * t.cos(), radius * t.sin()));
            }
            push_dashed_polyline(verts, &pts, DASH_LEN, color, plane);
        }
        Entity::Arc {
            center,
            radius,
            start_angle,
            end_angle,
            ..
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
            let steps = ((ARC_SEGMENTS_FULL as f64 * span / tau).ceil() as usize).max(4);
            let mut pts = Vec::with_capacity(steps + 1);
            for i in 0..=steps {
                let t = start_angle + span * (i as f64 / steps as f64);
                pts.push(*center + DVec2::new(radius * t.cos(), radius * t.sin()));
            }
            push_dashed_polyline(verts, &pts, DASH_LEN, color, plane);
        }
        Entity::Ellipse {
            center,
            radius_x,
            radius_y,
            ..
        } => {
            let tau = std::f64::consts::TAU;
            let mut pts = Vec::with_capacity(ARC_SEGMENTS_FULL + 1);
            for i in 0..=ARC_SEGMENTS_FULL {
                let t = tau * (i as f64 / ARC_SEGMENTS_FULL as f64);
                pts.push(*center + DVec2::new(radius_x * t.cos(), radius_y * t.sin()));
            }
            push_dashed_polyline(verts, &pts, DASH_LEN, color, plane);
        }
        Entity::Spline { points, .. } => {
            if points.len() >= 2 {
                let sampled = ducad_sketch::entity::sample_catmull_rom(points, 16);
                push_dashed_polyline(verts, &sampled, DASH_LEN, color, plane);
            }
        }
    }
}

pub(crate) fn push_dashed_polyline(
    verts: &mut Vec<LineVertex>,
    points: &[DVec2],
    dash_len: f64,
    color: [f32; 4],
    plane: &SketchPlane,
) {
    if points.len() < 2 {
        return;
    }
    let mut dist_accum = 0.0;
    let mut drawing = true;

    for w in points.windows(2) {
        let p0 = w[0];
        let p1 = w[1];
        let seg_vec = p1 - p0;
        let seg_len = seg_vec.length();
        if seg_len < 1e-6 {
            continue;
        }
        let dir = seg_vec / seg_len;
        let mut curr_t = 0.0;

        while curr_t < seg_len {
            let remain_in_phase = dash_len - (dist_accum % dash_len);
            let next_t = (curr_t + remain_in_phase).min(seg_len);
            let advance = next_t - curr_t;

            if drawing && advance > 1e-6 {
                verts.push(LineVertex {
                    position: to3(plane, p0 + dir * curr_t),
                    color,
                });
                verts.push(LineVertex {
                    position: to3(plane, p0 + dir * next_t),
                    color,
                });
            }

            dist_accum += advance;
            if (dist_accum % dash_len).abs() < 1e-4 || (dist_accum % dash_len) == 0.0 {
                drawing = !drawing;
            }
            curr_t = next_t;
        }
    }
}

pub(crate) fn push_spline(
    verts: &mut Vec<LineVertex>,
    points: &[DVec2],
    color: [f32; 4],
    plane: &SketchPlane,
) {
    if points.len() < 2 {
        return;
    }
    let sampled = ducad_sketch::entity::sample_catmull_rom(points, 16);
    for w in sampled.windows(2) {
        verts.push(LineVertex {
            position: to3(plane, w[0]),
            color,
        });
        verts.push(LineVertex {
            position: to3(plane, w[1]),
            color,
        });
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
