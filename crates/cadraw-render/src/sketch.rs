//! Konversi entitas `cadraw-sketch` + indikator snap menjadi `LineVertex`
//! untuk viewport. Digambar di bidang XY dengan sedikit offset Z agar
//! tidak z-fighting dengan garis grid yang ada persis di Z=0.

use std::collections::HashSet;

use cadraw_sketch::{Entity, EntityId, Sketch, SnapHit, SnapKind};
use glam::DVec2;

use crate::grid::LineVertex;

const Z_OFFSET: f32 = 0.02;
const COLOR_NORMAL: [f32; 4] = [0.86, 0.87, 0.90, 1.0];
const COLOR_HOVER: [f32; 4] = [1.0, 0.82, 0.25, 1.0];
const COLOR_SELECTED: [f32; 4] = [0.30, 0.65, 1.0, 1.0];
const COLOR_PREVIEW: [f32; 4] = [0.55, 0.90, 0.55, 0.85];
const COLOR_SNAP: [f32; 4] = [1.0, 0.55, 0.15, 1.0];
const ARC_SEGMENTS_FULL: usize = 48;

fn to3(p: DVec2) -> [f32; 3] {
    [p.x as f32, p.y as f32, Z_OFFSET]
}

/// Garis untuk seluruh entitas sketch, diwarnai menurut status hover/pilih.
pub fn entity_lines(
    sketch: &Sketch,
    hovered: Option<EntityId>,
    selected: &HashSet<EntityId>,
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
        push_entity(&mut verts, entity, color);
    }
    verts
}

/// Garis rubber-band untuk entitas yang sedang digambar (belum dicommit).
pub fn preview_lines(entity: &Entity) -> Vec<LineVertex> {
    let mut verts = Vec::new();
    push_entity(&mut verts, entity, COLOR_PREVIEW);
    verts
}

fn push_entity(verts: &mut Vec<LineVertex>, entity: &Entity, color: [f32; 4]) {
    match entity {
        Entity::Line { start, end } => {
            verts.push(LineVertex {
                position: to3(*start),
                color,
            });
            verts.push(LineVertex {
                position: to3(*end),
                color,
            });
        }
        Entity::Circle { center, radius } => {
            push_arc(verts, *center, *radius, 0.0, std::f64::consts::TAU, color)
        }
        Entity::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => push_arc(verts, *center, *radius, *start_angle, *end_angle, color),
    }
}

fn push_arc(
    verts: &mut Vec<LineVertex>,
    center: DVec2,
    radius: f64,
    start: f64,
    end: f64,
    color: [f32; 4],
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
            position: to3(prev),
            color,
        });
        verts.push(LineVertex {
            position: to3(p),
            color,
        });
        prev = p;
    }
}

/// Glyph indikator snap: bentuk berbeda per jenis, ukuran tetap dalam unit
/// dunia (cukup untuk skala sketch Fase 1; disempurnakan jadi ukuran
/// konstan-piksel di Fase 4).
pub fn snap_glyph(hit: &SnapHit) -> Vec<LineVertex> {
    const S: f64 = 3.0;
    let c = hit.point;
    let mut verts = Vec::new();
    let mut push_loop = |pts: &[DVec2]| {
        for i in 0..pts.len() {
            let a = pts[i];
            let b = pts[(i + 1) % pts.len()];
            verts.push(LineVertex {
                position: to3(a),
                color: COLOR_SNAP,
            });
            verts.push(LineVertex {
                position: to3(b),
                color: COLOR_SNAP,
            });
        }
    };

    match hit.kind {
        SnapKind::Endpoint => push_loop(&[
            c + DVec2::new(-S, -S),
            c + DVec2::new(S, -S),
            c + DVec2::new(S, S),
            c + DVec2::new(-S, S),
        ]),
        SnapKind::Midpoint => push_loop(&[
            c + DVec2::new(0.0, S),
            c + DVec2::new(S, -S),
            c + DVec2::new(-S, -S),
        ]),
        SnapKind::Center => push_arc(&mut verts, c, S, 0.0, std::f64::consts::TAU, COLOR_SNAP),
        SnapKind::Intersection => push_loop(&[
            c + DVec2::new(-S, 0.0),
            c + DVec2::new(0.0, -S),
            c + DVec2::new(S, 0.0),
            c + DVec2::new(0.0, S),
        ]),
        SnapKind::Grid => {
            let cross = [
                (c + DVec2::new(-S, 0.0), c + DVec2::new(S, 0.0)),
                (c + DVec2::new(0.0, -S), c + DVec2::new(0.0, S)),
            ];
            for (a, b) in cross {
                verts.push(LineVertex {
                    position: to3(a),
                    color: COLOR_SNAP,
                });
                verts.push(LineVertex {
                    position: to3(b),
                    color: COLOR_SNAP,
                });
            }
        }
    }
    verts
}
