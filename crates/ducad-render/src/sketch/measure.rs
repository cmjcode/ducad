use glam::DVec2;

use crate::grid::LineVertex;
use crate::plane::SketchPlane;
use crate::sketch::{to3, COLOR_MEASURE};

/// Garis kuning penghubung titik-titik tool "Ukur" pada bidang aktif.
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

/// Kepala panah kecil bentuk V di kedua ujung garis pengukuran.
pub fn measurement_arrowheads(points: &[DVec2], plane: &SketchPlane) -> Vec<LineVertex> {
    const HEAD_LEN: f64 = 4.0;
    const HEAD_ANGLE: f64 = 0.45;

    let mut verts = Vec::new();
    if points.len() < 2 {
        return verts;
    }

    let push_head = |verts: &mut Vec<LineVertex>, tip: DVec2, dir_out: DVec2| {
        for sign in [-1.0_f64, 1.0] {
            let angle = HEAD_ANGLE * sign;
            let (s, c) = angle.sin_cos();
            let rotated = DVec2::new(dir_out.x * c - dir_out.y * s, dir_out.x * s + dir_out.y * c);
            let wing = tip - rotated * HEAD_LEN;
            verts.push(LineVertex {
                position: to3(plane, tip),
                color: COLOR_MEASURE,
            });
            verts.push(LineVertex {
                position: to3(plane, wing),
                color: COLOR_MEASURE,
            });
        }
    };

    let first = points[0];
    let second = points[1];
    let dir_start = (first - second).normalize_or_zero();
    if dir_start != DVec2::ZERO {
        push_head(&mut verts, first, dir_start);
    }

    let last = points[points.len() - 1];
    let before_last = points[points.len() - 2];
    let dir_end = (last - before_last).normalize_or_zero();
    if dir_end != DVec2::ZERO {
        push_head(&mut verts, last, dir_end);
    }

    verts
}

/// Garis putus-putus 3D untuk proyeksi dimensi dan sumbu extrude.
pub fn dashed_line_3d(
    p1: [f32; 3],
    p2: [f32; 3],
    dash_len: f32,
    color: [f32; 4],
) -> Vec<LineVertex> {
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

/// Garis leader dimensi 2D dengan garis proyeksi putus-putus dan panah pembatas pada bidang aktif.
pub fn dimension_leader_lines(
    a: DVec2,
    b: DVec2,
    offset_dist: f64,
    plane: &SketchPlane,
) -> Vec<LineVertex> {
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
    verts.extend(dashed_line_3d(
        to3(plane, a),
        to3(plane, a_ext + perp.normalize() * 3.0),
        3.0,
        DIM_COLOR,
    ));
    verts.extend(dashed_line_3d(
        to3(plane, b),
        to3(plane, b_ext + perp.normalize() * 3.0),
        3.0,
        DIM_COLOR,
    ));

    // Garis dimensi paralel putus-putus
    verts.extend(dashed_line_3d(to3(plane, a_ext), to3(plane, b_ext), 4.0, DIM_COLOR));

    // Tick panah pada ujung garis dimensi
    let dir = (b_ext - a_ext).normalize();
    let tick_perp = perp.normalize() * 4.0;
    let tick_a1 = a_ext + dir * 4.0 + tick_perp;
    let tick_a2 = a_ext + dir * 4.0 - tick_perp;
    verts.push(LineVertex {
        position: to3(plane, a_ext),
        color: DIM_COLOR,
    });
    verts.push(LineVertex {
        position: to3(plane, tick_a1),
        color: DIM_COLOR,
    });
    verts.push(LineVertex {
        position: to3(plane, a_ext),
        color: DIM_COLOR,
    });
    verts.push(LineVertex {
        position: to3(plane, tick_a2),
        color: DIM_COLOR,
    });

    let tick_b1 = b_ext - dir * 4.0 + tick_perp;
    let tick_b2 = b_ext - dir * 4.0 - tick_perp;
    verts.push(LineVertex {
        position: to3(plane, b_ext),
        color: DIM_COLOR,
    });
    verts.push(LineVertex {
        position: to3(plane, tick_b1),
        color: DIM_COLOR,
    });
    verts.push(LineVertex {
        position: to3(plane, b_ext),
        color: DIM_COLOR,
    });
    verts.push(LineVertex {
        position: to3(plane, tick_b2),
        color: DIM_COLOR,
    });

    verts
}
