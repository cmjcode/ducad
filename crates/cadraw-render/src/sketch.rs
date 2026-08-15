//! Konversi entitas `cadraw-sketch` + indikator snap menjadi `LineVertex`
//! untuk viewport. Digambar pada bidang `SketchPlane` aktif dengan sedikit offset normal
//! agar tidak z-fighting dengan garis grid yang ada persis di bidang.

use std::collections::HashSet;

use cadraw_sketch::{Entity, EntityId, Sketch, SnapHit, SnapKind};
use glam::{DVec2, Vec3};

use crate::grid::LineVertex;
use crate::plane::SketchPlane;

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

fn to3(plane: &SketchPlane, p: DVec2) -> [f32; 3] {
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

/// Garis rubber-band untuk entitas yang sedang digambar (belum dicommit) pada bidang aktif.
pub fn preview_lines(entity: &Entity, plane: &SketchPlane) -> Vec<LineVertex> {
    let mut verts = Vec::new();
    push_entity(&mut verts, entity, COLOR_PREVIEW, plane);
    verts
}

/// Marker silang ungu di titik yang sudah diklik untuk tool pemilihan titik pada bidang aktif.
pub fn picked_point_glyph(point: DVec2, plane: &SketchPlane) -> Vec<LineVertex> {
    const S: f64 = 3.0;
    vec![
        LineVertex {
            position: to3(plane, point + DVec2::new(-S, -S)),
            color: COLOR_PICKED,
        },
        LineVertex {
            position: to3(plane, point + DVec2::new(S, S)),
            color: COLOR_PICKED,
        },
        LineVertex {
            position: to3(plane, point + DVec2::new(-S, S)),
            color: COLOR_PICKED,
        },
        LineVertex {
            position: to3(plane, point + DVec2::new(S, -S)),
            color: COLOR_PICKED,
        },
    ]
}

/// Garis peringatan untuk sub-segmen yang akan dihapus tool Trim.
pub fn removal_preview_lines(start: DVec2, end: DVec2, plane: &SketchPlane) -> Vec<LineVertex> {
    vec![
        LineVertex {
            position: to3(plane, start),
            color: COLOR_REMOVAL,
        },
        LineVertex {
            position: to3(plane, end),
            color: COLOR_REMOVAL,
        },
    ]
}

/// Garis kuning penghubung titik-titik tool "Ukur" (Fase 7) pada bidang aktif.
pub fn measurement_lines(points: &[DVec2], plane: &SketchPlane) -> Vec<LineVertex> {
    let mut verts = Vec::new();
    for pair in points.windows(2) {
        verts.push(LineVertex {
            position: to3(plane, pair[0]),
            color: COLOR_MEASURE,
        });
        verts.push(LineVertex {
            position: to3(plane, pair[1]),
            color: COLOR_MEASURE,
        });
    }
    verts
}

/// Garis putus-putus 3D untuk proyeksi dimensi dan sumbu extrude.
pub fn dashed_line_3d(p1: [f32; 3], p2: [f32; 3], dash_len: f32, color: [f32; 4]) -> Vec<LineVertex> {
    let mut verts = Vec::new();
    let dx = p2[0] - p1[0];
    let dy = p2[1] - p1[1];
    let dz = p2[2] - p1[2];
    let total_len = (dx * dx + dy * dy + dz * dz).sqrt();
    if total_len < 1e-4 {
        return verts;
    }

    let dir = [dx / total_len, dy / total_len, dz / total_len];
    let mut t = 0.0;
    let mut drawing = true;

    while t < total_len {
        let t_next = (t + dash_len).min(total_len);
        if drawing {
            verts.push(LineVertex {
                position: [
                    p1[0] + dir[0] * t,
                    p1[1] + dir[1] * t,
                    p1[2] + dir[2] * t,
                ],
                color,
            });
            verts.push(LineVertex {
                position: [
                    p1[0] + dir[0] * t_next,
                    p1[1] + dir[1] * t_next,
                    p1[2] + dir[2] * t_next,
                ],
                color,
            });
        }
        t = t_next;
        drawing = !drawing;
    }
    verts
}

/// Gizmo panah dua sisi (`↕`) mengambang di titik tengah profil sketch berorientasi normal untuk Direct Extrude.
pub fn double_arrow_gizmo_lines(
    center: [f32; 3],
    height: f32,
    arrow_size: f32,
    color: [f32; 4],
    normal: Vec3,
) -> Vec<LineVertex> {
    let mut verts = Vec::new();
    let n = normal.normalize_or_zero();
    let c = Vec3::from(center);
    let top = c + n * (height * 0.5);
    let bot = c - n * (height * 0.5);

    let (t1, t2) = if n.z.abs() < 0.95 {
        let t1 = n.cross(Vec3::Z).normalize();
        let t2 = n.cross(t1).normalize();
        (t1, t2)
    } else {
        let t1 = n.cross(Vec3::Y).normalize();
        let t2 = n.cross(t1).normalize();
        (t1, t2)
    };

    let shaft_radius = arrow_size * 0.25;
    let s = arrow_size;
    let segs = 8;
    let tau = std::f32::consts::TAU;

    // 1. Batang silinder multi-rib (tebal)
    for i in 0..segs {
        let angle = tau * (i as f32 / segs as f32);
        let radial = t1 * (shaft_radius * angle.cos()) + t2 * (shaft_radius * angle.sin());
        let p_bot = (bot + n * (s * 1.0)) + radial;
        let p_top = (top - n * (s * 1.0)) + radial;

        verts.push(LineVertex { position: [p_bot.x, p_bot.y, p_bot.z], color });
        verts.push(LineVertex { position: [p_top.x, p_top.y, p_top.z], color });

        // Ring batang di tengah
        let next_angle = tau * ((i + 1) as f32 / segs as f32);
        let next_radial = t1 * (shaft_radius * next_angle.cos()) + t2 * (shaft_radius * next_angle.sin());
        let p_mid1 = c + radial;
        let p_mid2 = c + next_radial;
        verts.push(LineVertex { position: [p_mid1.x, p_mid1.y, p_mid1.z], color });
        verts.push(LineVertex { position: [p_mid2.x, p_mid2.y, p_mid2.z], color });
    }

    // Poros utama tengah putih/terang
    const BRIGHT_WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    verts.push(LineVertex { position: [bot.x, bot.y, bot.z], color: BRIGHT_WHITE });
    verts.push(LineVertex { position: [top.x, top.y, top.z], color: BRIGHT_WHITE });

    // 2. Kepala panah atas (Kerucut 8 sisi + ring dasar)
    let top_base = top - n * (s * 1.3);
    for i in 0..segs {
        let angle = tau * (i as f32 / segs as f32);
        let next_angle = tau * ((i + 1) as f32 / segs as f32);
        let b1 = top_base + t1 * (s * angle.cos()) + t2 * (s * angle.sin());
        let b2 = top_base + t1 * (s * next_angle.cos()) + t2 * (s * next_angle.sin());

        verts.push(LineVertex { position: [top.x, top.y, top.z], color });
        verts.push(LineVertex { position: [b1.x, b1.y, b1.z], color });

        verts.push(LineVertex { position: [b1.x, b1.y, b1.z], color });
        verts.push(LineVertex { position: [b2.x, b2.y, b2.z], color });

        verts.push(LineVertex { position: [b1.x, b1.y, b1.z], color });
        verts.push(LineVertex { position: [top_base.x, top_base.y, top_base.z], color });
    }

    // 3. Kepala panah bawah (Kerucut 8 sisi + ring dasar)
    let bot_base = bot + n * (s * 1.3);
    for i in 0..segs {
        let angle = tau * (i as f32 / segs as f32);
        let next_angle = tau * ((i + 1) as f32 / segs as f32);
        let b1 = bot_base + t1 * (s * angle.cos()) + t2 * (s * angle.sin());
        let b2 = bot_base + t1 * (s * next_angle.cos()) + t2 * (s * next_angle.sin());

        verts.push(LineVertex { position: [bot.x, bot.y, bot.z], color });
        verts.push(LineVertex { position: [b1.x, b1.y, b1.z], color });

        verts.push(LineVertex { position: [b1.x, b1.y, b1.z], color });
        verts.push(LineVertex { position: [b2.x, b2.y, b2.z], color });

        verts.push(LineVertex { position: [b1.x, b1.y, b1.z], color });
        verts.push(LineVertex { position: [bot_base.x, bot_base.y, bot_base.z], color });
    }

    verts
}

/// Garis leader dimensi 2D dengan garis proyeksi putus-putus dan panah pembatas pada bidang aktif.
pub fn dimension_leader_lines(a: DVec2, b: DVec2, offset_dist: f64, plane: &SketchPlane) -> Vec<LineVertex> {
    let mut verts = Vec::new();
    let ab = b - a;
    let len = ab.length();
    if len < 1e-4 {
        return verts;
    }

    let perp = DVec2::new(-ab.y / len, ab.x / len) * offset_dist;
    let a_ext = a + perp;
    let b_ext = b + perp;
    const DIM_COLOR: [f32; 4] = [0.40, 0.45, 0.52, 0.85];

    // Garis proyeksi tegak lurus dari titik asal ke garis dimensi
    verts.extend(dashed_line_3d(to3(plane, a), to3(plane, a_ext + perp.normalize() * 3.0), 3.0, DIM_COLOR));
    verts.extend(dashed_line_3d(to3(plane, b), to3(plane, b_ext + perp.normalize() * 3.0), 3.0, DIM_COLOR));

    // Garis dimensi paralel putus-putus
    verts.extend(dashed_line_3d(to3(plane, a_ext), to3(plane, b_ext), 4.0, DIM_COLOR));

    // Tick panah pada ujung garis dimensi
    let dir = (b_ext - a_ext).normalize();
    let tick_perp = perp.normalize() * 4.0;
    let tick_a1 = a_ext + dir * 4.0 + tick_perp;
    let tick_a2 = a_ext + dir * 4.0 - tick_perp;
    verts.push(LineVertex { position: to3(plane, a_ext), color: DIM_COLOR });
    verts.push(LineVertex { position: to3(plane, tick_a1), color: DIM_COLOR });
    verts.push(LineVertex { position: to3(plane, a_ext), color: DIM_COLOR });
    verts.push(LineVertex { position: to3(plane, tick_a2), color: DIM_COLOR });

    let tick_b1 = b_ext - dir * 4.0 + tick_perp;
    let tick_b2 = b_ext - dir * 4.0 - tick_perp;
    verts.push(LineVertex { position: to3(plane, b_ext), color: DIM_COLOR });
    verts.push(LineVertex { position: to3(plane, tick_b1), color: DIM_COLOR });
    verts.push(LineVertex { position: to3(plane, b_ext), color: DIM_COLOR });
    verts.push(LineVertex { position: to3(plane, tick_b2), color: DIM_COLOR });

    verts
}

fn push_entity(verts: &mut Vec<LineVertex>, entity: &Entity, color: [f32; 4], plane: &SketchPlane) {
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

fn push_ellipse(
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

fn push_arc(
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

/// Glyph indikator snap pada bidang aktif.
pub fn snap_glyph(hit: &SnapHit, plane: &SketchPlane) -> Vec<LineVertex> {
    const S: f64 = 3.0;
    let c = hit.point;
    let mut verts = Vec::new();
    let mut push_loop = |pts: &[DVec2]| {
        for i in 0..pts.len() {
            let a = pts[i];
            let b = pts[(i + 1) % pts.len()];
            verts.push(LineVertex {
                position: to3(plane, a),
                color: COLOR_SNAP,
            });
            verts.push(LineVertex {
                position: to3(plane, b),
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
        SnapKind::Center => push_arc(&mut verts, c, S, 0.0, std::f64::consts::TAU, COLOR_SNAP, plane),
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
                    position: to3(plane, a),
                    color: COLOR_SNAP,
                });
                verts.push(LineVertex {
                    position: to3(plane, b),
                    color: COLOR_SNAP,
                });
            }
        }
    }
    verts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measurement_lines_empty_for_single_point() {
        let plane = SketchPlane::top();
        assert!(measurement_lines(&[DVec2::new(0.0, 0.0)], &plane).is_empty());
    }

    #[test]
    fn measurement_lines_one_segment_for_two_points() {
        let plane = SketchPlane::top();
        let verts = measurement_lines(&[DVec2::new(0.0, 0.0), DVec2::new(5.0, 0.0)], &plane);
        assert_eq!(verts.len(), 2);
    }

    #[test]
    fn measurement_lines_two_segments_for_three_points() {
        let plane = SketchPlane::top();
        let verts = measurement_lines(
            &[
                DVec2::new(0.0, 0.0),
                DVec2::new(1.0, 1.0),
                DVec2::new(2.0, 0.0),
            ],
            &plane,
        );
        assert_eq!(verts.len(), 4);
    }

    #[test]
    fn inactive_entity_lines_front_and_right_planes() {
        let mut sketch = Sketch::default();
        sketch.entities.insert(Entity::Line {
            start: DVec2::new(10.0, 20.0),
            end: DVec2::new(30.0, 40.0),
        });

        let front_plane = SketchPlane::front();
        let front_verts = inactive_entity_lines(&sketch, &front_plane);
        assert_eq!(front_verts.len(), 2);
        // Front plane: x -> x, y -> z, y_world = -Z_OFFSET
        assert!((front_verts[0].position[0] - 10.0).abs() < 1e-4);
        assert!((front_verts[0].position[1] - (-Z_OFFSET)).abs() < 1e-4);
        assert!((front_verts[0].position[2] - 20.0).abs() < 1e-4);

        let right_plane = SketchPlane::right();
        let right_verts = inactive_entity_lines(&sketch, &right_plane);
        assert_eq!(right_verts.len(), 2);
        // Right plane: x_sketch -> y_world, y_sketch -> z_world, x_world = Z_OFFSET
        assert!((right_verts[0].position[0] - Z_OFFSET).abs() < 1e-4);
        assert!((right_verts[0].position[1] - 10.0).abs() < 1e-4);
        assert!((right_verts[0].position[2] - 20.0).abs() < 1e-4);
    }
}
