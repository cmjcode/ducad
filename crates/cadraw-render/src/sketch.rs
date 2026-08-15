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
const COLOR_REMOVAL: [f32; 4] = [0.95, 0.25, 0.25, 0.95];
const COLOR_PICKED: [f32; 4] = [0.65, 0.35, 0.95, 1.0];
const COLOR_MEASURE: [f32; 4] = [1.0, 0.95, 0.35, 1.0];
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

/// Marker silang ungu di titik yang sudah diklik untuk tool pemilihan
/// titik (Coincident/Symmetric) — beda warna dari glyph snap oranye
/// (`snap_glyph`) supaya "titik sudah dipilih" tidak tertukar dengan
/// "kursor sedang di atas titik".
pub fn picked_point_glyph(point: DVec2) -> Vec<LineVertex> {
    const S: f64 = 3.0;
    vec![
        LineVertex {
            position: to3(point + DVec2::new(-S, -S)),
            color: COLOR_PICKED,
        },
        LineVertex {
            position: to3(point + DVec2::new(S, S)),
            color: COLOR_PICKED,
        },
        LineVertex {
            position: to3(point + DVec2::new(-S, S)),
            color: COLOR_PICKED,
        },
        LineVertex {
            position: to3(point + DVec2::new(S, -S)),
            color: COLOR_PICKED,
        },
    ]
}

/// Garis peringatan untuk sub-segmen yang akan dihapus tool Trim — dipakai
/// sebagai preview hover sebelum klik commit.
pub fn removal_preview_lines(start: DVec2, end: DVec2) -> Vec<LineVertex> {
    vec![
        LineVertex {
            position: to3(start),
            color: COLOR_REMOVAL,
        },
        LineVertex {
            position: to3(end),
            color: COLOR_REMOVAL,
        },
    ]
}

/// Garis kuning penghubung titik-titik tool "Ukur" (Fase 7) — 2 titik untuk
/// jarak, 3 titik (dengan `vertex` di tengah) untuk sudut. Sengaja terima
/// `&[DVec2]` generik (bukan `Measurement` dari `cadraw-app`) supaya crate
/// ini tetap tidak bergantung pada tipe app-level, sama pola dengan seluruh
/// modul render lain.
pub fn measurement_lines(points: &[DVec2]) -> Vec<LineVertex> {
    let mut verts = Vec::new();
    for pair in points.windows(2) {
        verts.push(LineVertex {
            position: to3(pair[0]),
            color: COLOR_MEASURE,
        });
        verts.push(LineVertex {
            position: to3(pair[1]),
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

/// Gizmo panah dua sisi (`↕`) mengambang di titik tengah profil sketch (Z-up/down) untuk Direct Extrude.
pub fn double_arrow_gizmo_lines(center: [f32; 3], height: f32, arrow_size: f32, color: [f32; 4]) -> Vec<LineVertex> {
    let mut verts = Vec::new();
    let top = [center[0], center[1], center[2] + height * 0.5];
    let bot = [center[0], center[1], center[2] - height * 0.5];

    // Batang poros panah
    verts.push(LineVertex { position: bot, color });
    verts.push(LineVertex { position: top, color });

    // Kepala panah atas
    let s = arrow_size;
    verts.push(LineVertex { position: top, color });
    verts.push(LineVertex { position: [top[0] - s, top[1], top[2] - s * 1.4], color });

    verts.push(LineVertex { position: top, color });
    verts.push(LineVertex { position: [top[0] + s, top[1], top[2] - s * 1.4], color });

    verts.push(LineVertex { position: top, color });
    verts.push(LineVertex { position: [top[0], top[1] - s, top[2] - s * 1.4], color });

    verts.push(LineVertex { position: top, color });
    verts.push(LineVertex { position: [top[0], top[1] + s, top[2] - s * 1.4], color });

    // Kepala panah bawah
    verts.push(LineVertex { position: bot, color });
    verts.push(LineVertex { position: [bot[0] - s, bot[1], bot[2] + s * 1.4], color });

    verts.push(LineVertex { position: bot, color });
    verts.push(LineVertex { position: [bot[0] + s, bot[1], bot[2] + s * 1.4], color });

    verts.push(LineVertex { position: bot, color });
    verts.push(LineVertex { position: [bot[0], bot[1] - s, bot[2] + s * 1.4], color });

    verts.push(LineVertex { position: bot, color });
    verts.push(LineVertex { position: [bot[0], bot[1] + s, bot[2] + s * 1.4], color });

    verts
}

/// Garis leader dimensi 2D dengan garis proyeksi putus-putus dan panah pembatas (seperti Screenshot 1).
pub fn dimension_leader_lines(a: DVec2, b: DVec2, offset_dist: f64) -> Vec<LineVertex> {
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
    verts.extend(dashed_line_3d(to3(a), to3(a_ext + perp.normalize() * 3.0), 3.0, DIM_COLOR));
    verts.extend(dashed_line_3d(to3(b), to3(b_ext + perp.normalize() * 3.0), 3.0, DIM_COLOR));

    // Garis dimensi paralel putus-putus
    verts.extend(dashed_line_3d(to3(a_ext), to3(b_ext), 4.0, DIM_COLOR));

    // Tick panah pada ujung garis dimensi
    let dir = (b_ext - a_ext).normalize();
    let tick_perp = perp.normalize() * 4.0;
    let tick_a1 = a_ext + dir * 4.0 + tick_perp;
    let tick_a2 = a_ext + dir * 4.0 - tick_perp;
    verts.push(LineVertex { position: to3(a_ext), color: DIM_COLOR });
    verts.push(LineVertex { position: to3(tick_a1), color: DIM_COLOR });
    verts.push(LineVertex { position: to3(a_ext), color: DIM_COLOR });
    verts.push(LineVertex { position: to3(tick_a2), color: DIM_COLOR });

    let tick_b1 = b_ext - dir * 4.0 + tick_perp;
    let tick_b2 = b_ext - dir * 4.0 - tick_perp;
    verts.push(LineVertex { position: to3(b_ext), color: DIM_COLOR });
    verts.push(LineVertex { position: to3(tick_b1), color: DIM_COLOR });
    verts.push(LineVertex { position: to3(b_ext), color: DIM_COLOR });
    verts.push(LineVertex { position: to3(tick_b2), color: DIM_COLOR });

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
        Entity::Ellipse {
            center,
            radius_x,
            radius_y,
        } => push_ellipse(verts, *center, *radius_x, *radius_y, color),
    }
}

fn push_ellipse(
    verts: &mut Vec<LineVertex>,
    center: DVec2,
    radius_x: f64,
    radius_y: f64,
    color: [f32; 4],
) {
    let tau = std::f64::consts::TAU;
    let mut prev = center + DVec2::new(radius_x, 0.0);
    for i in 1..=ARC_SEGMENTS_FULL {
        let t = tau * (i as f64 / ARC_SEGMENTS_FULL as f64);
        let p = center + DVec2::new(radius_x * t.cos(), radius_y * t.sin());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measurement_lines_empty_for_single_point() {
        // 1 titik belum ada segmen untuk digambar (dipakai saat tool Ukur
        // baru dapat titik pertama, sebelum titik kedua diklik).
        assert!(measurement_lines(&[DVec2::new(0.0, 0.0)]).is_empty());
    }

    #[test]
    fn measurement_lines_one_segment_for_two_points() {
        let verts = measurement_lines(&[DVec2::new(0.0, 0.0), DVec2::new(5.0, 0.0)]);
        assert_eq!(verts.len(), 2);
    }

    #[test]
    fn measurement_lines_two_segments_for_three_points() {
        // Tool Ukur Sudut: 2 segmen (a→vertex, vertex→b) dari 3 titik.
        let verts = measurement_lines(&[
            DVec2::new(0.0, 0.0),
            DVec2::new(1.0, 1.0),
            DVec2::new(2.0, 0.0),
        ]);
        assert_eq!(verts.len(), 4);
    }
}
