use glam::Vec3;

use crate::grid::LineVertex;
use crate::sketch::measure::dashed_line_3d;

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

        verts.push(LineVertex {
            position: [p_bot.x, p_bot.y, p_bot.z],
            color,
        });
        verts.push(LineVertex {
            position: [p_top.x, p_top.y, p_top.z],
            color,
        });

        // Ring batang di tengah
        let next_angle = tau * ((i + 1) as f32 / segs as f32);
        let next_radial =
            t1 * (shaft_radius * next_angle.cos()) + t2 * (shaft_radius * next_angle.sin());
        let p_mid1 = c + radial;
        let p_mid2 = c + next_radial;
        verts.push(LineVertex {
            position: [p_mid1.x, p_mid1.y, p_mid1.z],
            color,
        });
        verts.push(LineVertex {
            position: [p_mid2.x, p_mid2.y, p_mid2.z],
            color,
        });
    }

    // Poros utama tengah putih/terang
    const BRIGHT_WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    verts.push(LineVertex {
        position: [bot.x, bot.y, bot.z],
        color: BRIGHT_WHITE,
    });
    verts.push(LineVertex {
        position: [top.x, top.y, top.z],
        color: BRIGHT_WHITE,
    });

    // 2. Kepala panah atas (Kerucut 8 sisi + ring dasar)
    let top_base = top - n * (s * 1.3);
    for i in 0..segs {
        let angle = tau * (i as f32 / segs as f32);
        let next_angle = tau * ((i + 1) as f32 / segs as f32);
        let b1 = top_base + t1 * (s * angle.cos()) + t2 * (s * angle.sin());
        let b2 = top_base + t1 * (s * next_angle.cos()) + t2 * (s * next_angle.sin());

        verts.push(LineVertex {
            position: [top.x, top.y, top.z],
            color,
        });
        verts.push(LineVertex {
            position: [b1.x, b1.y, b1.z],
            color,
        });

        verts.push(LineVertex {
            position: [b1.x, b1.y, b1.z],
            color,
        });
        verts.push(LineVertex {
            position: [b2.x, b2.y, b2.z],
            color,
        });

        verts.push(LineVertex {
            position: [b1.x, b1.y, b1.z],
            color,
        });
        verts.push(LineVertex {
            position: [top_base.x, top_base.y, top_base.z],
            color,
        });
    }

    // 3. Kepala panah bawah (Kerucut 8 sisi + ring dasar)
    let bot_base = bot + n * (s * 1.3);
    for i in 0..segs {
        let angle = tau * (i as f32 / segs as f32);
        let next_angle = tau * ((i + 1) as f32 / segs as f32);
        let b1 = bot_base + t1 * (s * angle.cos()) + t2 * (s * angle.sin());
        let b2 = bot_base + t1 * (s * next_angle.cos()) + t2 * (s * next_angle.sin());

        verts.push(LineVertex {
            position: [bot.x, bot.y, bot.z],
            color,
        });
        verts.push(LineVertex {
            position: [b1.x, b1.y, b1.z],
            color,
        });

        verts.push(LineVertex {
            position: [b1.x, b1.y, b1.z],
            color,
        });
        verts.push(LineVertex {
            position: [b2.x, b2.y, b2.z],
            color,
        });

        verts.push(LineVertex {
            position: [b1.x, b1.y, b1.z],
            color,
        });
        verts.push(LineVertex {
            position: [bot_base.x, bot_base.y, bot_base.z],
            color,
        });
    }

    verts
}

/// Versi SOLID (bukan wireframe) dari `double_arrow_gizmo_lines`.
#[allow(clippy::type_complexity)]
pub fn solid_double_arrow_gizmo_mesh(
    center: [f32; 3],
    height: f32,
    arrow_size: f32,
    color: [f32; 4],
    normal: Vec3,
) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 4]>, Vec<u32>) {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let n = normal.normalize_or_zero();
    if n == Vec3::ZERO {
        return (positions, normals, colors, indices);
    }
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

    let shaft_radius = arrow_size * 0.22;
    let s = arrow_size;
    let segs = 10;
    let tau = std::f32::consts::TAU;

    let push_tri = |positions: &mut Vec<[f32; 3]>,
                        normals: &mut Vec<[f32; 3]>,
                        colors: &mut Vec<[f32; 4]>,
                        indices: &mut Vec<u32>,
                        a: Vec3,
                        b: Vec3,
                        c: Vec3,
                        outward_hint: Vec3| {
        let mut face_n = (b - a).cross(c - a);
        let (vb, vc) = if face_n.dot(outward_hint) < 0.0 {
            face_n = -face_n;
            (c, b)
        } else {
            (b, c)
        };
        let face_n = face_n.normalize_or_zero();
        let base_idx = positions.len() as u32;
        for p in [a, vb, vc] {
            positions.push([p.x, p.y, p.z]);
            normals.push([face_n.x, face_n.y, face_n.z]);
            colors.push(color);
        }
        indices.extend_from_slice(&[base_idx, base_idx + 1, base_idx + 2]);
    };

    // 1. Poros tengah (silinder solid tipis)
    let shaft_top = top - n * (s * 1.0);
    let shaft_bot = bot + n * (s * 1.0);
    for i in 0..segs {
        let a0 = tau * (i as f32 / segs as f32);
        let a1 = tau * ((i + 1) as f32 / segs as f32);
        let r0 = t1 * (shaft_radius * a0.cos()) + t2 * (shaft_radius * a0.sin());
        let r1 = t1 * (shaft_radius * a1.cos()) + t2 * (shaft_radius * a1.sin());
        let hint = r0 + r1;
        push_tri(
            &mut positions,
            &mut normals,
            &mut colors,
            &mut indices,
            shaft_bot + r0,
            shaft_top + r0,
            shaft_top + r1,
            hint,
        );
        push_tri(
            &mut positions,
            &mut normals,
            &mut colors,
            &mut indices,
            shaft_bot + r0,
            shaft_top + r1,
            shaft_bot + r1,
            hint,
        );
    }

    // 2 & 3. Kepala panah atas & bawah: kerucut solid
    let mut push_cone = |apex: Vec3, base_center: Vec3| {
        for i in 0..segs {
            let a0 = tau * (i as f32 / segs as f32);
            let a1 = tau * ((i + 1) as f32 / segs as f32);
            let b0 = base_center + t1 * (s * a0.cos()) + t2 * (s * a0.sin());
            let b1 = base_center + t1 * (s * a1.cos()) + t2 * (s * a1.sin());
            let radial_hint = (b0 - base_center) + (b1 - base_center);
            push_tri(
                &mut positions,
                &mut normals,
                &mut colors,
                &mut indices,
                apex,
                b0,
                b1,
                radial_hint,
            );
            let cap_hint = base_center - apex;
            push_tri(
                &mut positions,
                &mut normals,
                &mut colors,
                &mut indices,
                base_center,
                b0,
                b1,
                cap_hint,
            );
        }
    };
    push_cone(top, top - n * (s * 1.3));
    push_cone(bot, bot - n * (s * 1.3));

    (positions, normals, colors, indices)
}

/// Marker gizmo vertex fillet 3D.
pub fn vertex_fillet_marker_lines(
    vertex: [f32; 3],
    out_dir: Vec3,
    handle_dist: f32,
    color: [f32; 4],
) -> Vec<LineVertex> {
    let mut verts = Vec::new();
    let v = Vec3::from(vertex);
    let n = out_dir.normalize_or_zero();

    const S: f32 = 1.2;
    let corners = [
        Vec3::new(-S, -S, -S),
        Vec3::new(S, -S, -S),
        Vec3::new(S, S, -S),
        Vec3::new(-S, S, -S),
        Vec3::new(-S, -S, S),
        Vec3::new(S, -S, S),
        Vec3::new(S, S, S),
        Vec3::new(-S, S, S),
    ];
    const EDGES: [(usize, usize); 12] = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    for (a, b) in EDGES {
        let pa = v + corners[a];
        let pb = v + corners[b];
        verts.push(LineVertex {
            position: [pa.x, pa.y, pa.z],
            color,
        });
        verts.push(LineVertex {
            position: [pb.x, pb.y, pb.z],
            color,
        });
    }

    if n == Vec3::ZERO {
        return verts;
    }

    let handle = v + n * handle_dist;
    let handle_arr = [handle.x, handle.y, handle.z];
    verts.extend(dashed_line_3d(vertex, handle_arr, 2.0, color));

    let (t1, t2) = if n.z.abs() < 0.95 {
        let t1 = n.cross(Vec3::Z).normalize();
        let t2 = n.cross(t1).normalize();
        (t1, t2)
    } else {
        let t1 = n.cross(Vec3::Y).normalize();
        let t2 = n.cross(t1).normalize();
        (t1, t2)
    };
    const ARC_R: f32 = 2.5;
    let arc_center = handle - n * (ARC_R * 0.5);
    let segs = 8;
    let mut prev = arc_center + t1 * ARC_R;
    for i in 1..=segs {
        let angle = std::f32::consts::FRAC_PI_2 * (i as f32 / segs as f32);
        let p = arc_center + t1 * (ARC_R * angle.cos()) + t2 * (ARC_R * angle.sin());
        verts.push(LineVertex {
            position: [prev.x, prev.y, prev.z],
            color,
        });
        verts.push(LineVertex {
            position: [p.x, p.y, p.z],
            color,
        });
        prev = p;
    }

    verts
}

/// Marker kecil di tiap titik `vertices`.
pub fn vertex_dot_markers(
    vertices: &[[f32; 3]],
    hover_point: Option<[f32; 3]>,
    color: [f32; 4],
    hover_color: [f32; 4],
) -> Vec<LineVertex> {
    const HOVER_EPS: f32 = 1e-3;
    let mut verts = Vec::new();
    for p in vertices {
        let is_hover = hover_point.is_some_and(|h| {
            (h[0] - p[0]).abs() < HOVER_EPS
                && (h[1] - p[1]).abs() < HOVER_EPS
                && (h[2] - p[2]).abs() < HOVER_EPS
        });
        let c = if is_hover { hover_color } else { color };
        let s = if is_hover { 2.2 } else { 1.0 };
        let v = Vec3::from(*p);
        for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
            let a = v - axis * s;
            let b = v + axis * s;
            verts.push(LineVertex {
                position: [a.x, a.y, a.z],
                color: c,
            });
            verts.push(LineVertex {
                position: [b.x, b.y, b.z],
                color: c,
            });
        }
    }
    verts
}
