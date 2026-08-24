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
    push_cone(bot, bot + n * (s * 1.3));

    (positions, normals, colors, indices)
}

/// Mesh solid panah penunjuk arah tunggal (misal: Pull Direction indicator pada Draft Analysis).
#[allow(clippy::type_complexity)]
pub fn solid_directional_arrow_mesh(
    start: [f32; 3],
    length: f32,
    arrow_size: f32,
    color: [f32; 4],
    direction: Vec3,
) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 4]>, Vec<u32>) {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let n = direction.normalize_or_zero();
    if n == Vec3::ZERO {
        return (positions, normals, colors, indices);
    }
    let base_start = Vec3::from(start);
    let tip = base_start + n * length;

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
    let segs = 12;
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

    // 1. Silinder Poros Panah
    let cone_base = tip - n * (s * 1.5);
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
            base_start + r0,
            cone_base + r0,
            cone_base + r1,
            hint,
        );
        push_tri(
            &mut positions,
            &mut normals,
            &mut colors,
            &mut indices,
            base_start + r0,
            cone_base + r1,
            base_start + r1,
            hint,
        );
        // Base cap (tutup bawah poros)
        push_tri(
            &mut positions,
            &mut normals,
            &mut colors,
            &mut indices,
            base_start,
            base_start + r1,
            base_start + r0,
            -n,
        );
    }

    // 2. Kepala Panah Kerucut (Cone)
    for i in 0..segs {
        let a0 = tau * (i as f32 / segs as f32);
        let a1 = tau * ((i + 1) as f32 / segs as f32);
        let b0 = cone_base + t1 * (s * a0.cos()) + t2 * (s * a0.sin());
        let b1 = cone_base + t1 * (s * a1.cos()) + t2 * (s * a1.sin());
        let radial_hint = (b0 - cone_base) + (b1 - cone_base);
        push_tri(
            &mut positions,
            &mut normals,
            &mut colors,
            &mut indices,
            tip,
            b0,
            b1,
            radial_hint,
        );
        // Tutup alas kerucut
        push_tri(
            &mut positions,
            &mut normals,
            &mut colors,
            &mut indices,
            cone_base,
            b1,
            b0,
            -n,
        );
    }

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

/// Bagian komponen Shapr3D 3D Transform Gizmo yang dapat di-hover atau di-drag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformGizmoPart {
    TranslateX,
    TranslateY,
    TranslateZ,
    PlaneXY,
    PlaneYZ,
    PlaneZX,
    RotateX,
    RotateY,
    RotateZ,
    CenterPivot,
}

/// Helper: gambar satu panah 3D linier dengan arrowhead kerucut 8-sisi.
fn push_arrow_3d(
    verts: &mut Vec<LineVertex>,
    start: Vec3,
    end: Vec3,
    color: [f32; 4],
    head_size: f32,
) {
    let dir = (end - start).normalize_or_zero();
    if dir == Vec3::ZERO {
        return;
    }
    let (t1, t2) = if dir.z.abs() < 0.95 {
        let t1 = dir.cross(Vec3::Z).normalize();
        let t2 = dir.cross(t1).normalize();
        (t1, t2)
    } else {
        let t1 = dir.cross(Vec3::Y).normalize();
        let t2 = dir.cross(t1).normalize();
        (t1, t2)
    };

    // Shaft line
    let base_cone = end - dir * head_size;
    verts.push(LineVertex {
        position: [start.x, start.y, start.z],
        color,
    });
    verts.push(LineVertex {
        position: [base_cone.x, base_cone.y, base_cone.z],
        color,
    });

    // Arrowhead cone 8 segments
    let segs = 8;
    let radius = head_size * 0.45;
    let tau = std::f32::consts::TAU;
    for i in 0..segs {
        let a0 = tau * (i as f32 / segs as f32);
        let a1 = tau * ((i + 1) as f32 / segs as f32);
        let p0 = base_cone + t1 * (radius * a0.cos()) + t2 * (radius * a0.sin());
        let p1 = base_cone + t1 * (radius * a1.cos()) + t2 * (radius * a1.sin());

        // Rib to apex
        verts.push(LineVertex {
            position: [p0.x, p0.y, p0.z],
            color,
        });
        verts.push(LineVertex {
            position: [end.x, end.y, end.z],
            color,
        });

        // Base ring segment
        verts.push(LineVertex {
            position: [p0.x, p0.y, p0.z],
            color,
        });
        verts.push(LineVertex {
            position: [p1.x, p1.y, p1.z],
            color,
        });
    }
}

/// Helper: gambar kotak planar (2D tile di ruang 3D) dengan sudut rounded.
fn push_planar_tile(
    verts: &mut Vec<LineVertex>,
    center: Vec3,
    u_axis: Vec3,
    v_axis: Vec3,
    size: f32,
    color: [f32; 4],
) {
    let half = size * 0.5;
    let p0 = center - u_axis * half - v_axis * half;
    let p1 = center + u_axis * half - v_axis * half;
    let p2 = center + u_axis * half + v_axis * half;
    let p3 = center - u_axis * half + v_axis * half;

    let loop_pts = [p0, p1, p2, p3, p0];
    for w in loop_pts.windows(2) {
        verts.push(LineVertex {
            position: [w[0].x, w[0].y, w[0].z],
            color,
        });
        verts.push(LineVertex {
            position: [w[1].x, w[1].y, w[1].z],
            color,
        });
    }
}

/// Helper: gambar busur rotasi melengkung dengan panah dua arah di kedua ujungnya.
fn push_curved_rotation_arc(
    verts: &mut Vec<LineVertex>,
    center: Vec3,
    u_axis: Vec3,
    v_axis: Vec3,
    radius: f32,
    start_rad: f32,
    end_rad: f32,
    color: [f32; 4],
    arrow_size: f32,
) {
    let segs = 16;
    let mut prev = center + u_axis * (radius * start_rad.cos()) + v_axis * (radius * start_rad.sin());
    for i in 1..=segs {
        let t = i as f32 / segs as f32;
        let ang = start_rad + (end_rad - start_rad) * t;
        let cur = center + u_axis * (radius * ang.cos()) + v_axis * (radius * ang.sin());
        verts.push(LineVertex {
            position: [prev.x, prev.y, prev.z],
            color,
        });
        verts.push(LineVertex {
            position: [cur.x, cur.y, cur.z],
            color,
        });
        prev = cur;
    }

    // Arrowhead di start
    let p_start = center + u_axis * (radius * start_rad.cos()) + v_axis * (radius * start_rad.sin());
    let tan_start = (-u_axis * start_rad.sin() + v_axis * start_rad.cos()).normalize();
    let norm_start = (u_axis * start_rad.cos() + v_axis * start_rad.sin()).normalize();
    let h1_a = p_start + tan_start * arrow_size + norm_start * (arrow_size * 0.45);
    let h1_b = p_start + tan_start * arrow_size - norm_start * (arrow_size * 0.45);
    verts.push(LineVertex { position: [p_start.x, p_start.y, p_start.z], color });
    verts.push(LineVertex { position: [h1_a.x, h1_a.y, h1_a.z], color });
    verts.push(LineVertex { position: [p_start.x, p_start.y, p_start.z], color });
    verts.push(LineVertex { position: [h1_b.x, h1_b.y, h1_b.z], color });

    // Arrowhead di end
    let p_end = center + u_axis * (radius * end_rad.cos()) + v_axis * (radius * end_rad.sin());
    let tan_end = (-u_axis * end_rad.sin() + v_axis * end_rad.cos()).normalize();
    let norm_end = (u_axis * end_rad.cos() + v_axis * end_rad.sin()).normalize();
    let h2_a = p_end - tan_end * arrow_size + norm_end * (arrow_size * 0.45);
    let h2_b = p_end - tan_end * arrow_size - norm_end * (arrow_size * 0.45);
    verts.push(LineVertex { position: [p_end.x, p_end.y, p_end.z], color });
    verts.push(LineVertex { position: [h2_a.x, h2_a.y, h2_a.z], color });
    verts.push(LineVertex { position: [p_end.x, p_end.y, p_end.z], color });
    verts.push(LineVertex { position: [h2_b.x, h2_b.y, h2_b.z], color });
}

/// Shapr3D-Style 3D Transform Gizmo:
/// Menghasilkan garis geometri 3D lengkap untuk widget transformasi solid body:
/// - 3 panah sumbu translasi linier (X, Y, Z)
/// - 3 kotak handle bidang translasi planar (XY, YZ, ZX)
/// - 3 busur rotasi berpanah ganda (sekeliling sumbu X, Y, Z)
/// - Cincin tengah pivot handle
pub fn shapr3d_transform_gizmo_lines(
    center_pos: [f32; 3],
    gizmo_scale: f32,
    active_part: Option<TransformGizmoPart>,
) -> Vec<LineVertex> {
    let mut verts = Vec::new();
    let c = Vec3::from(center_pos);
    let s = gizmo_scale.max(10.0);

    const COLOR_BASE: [f32; 4] = [0.95, 0.95, 0.98, 0.95];
    const COLOR_ACTIVE: [f32; 4] = [1.0, 0.85, 0.20, 1.0];
    const COLOR_X: [f32; 4] = [0.96, 0.30, 0.30, 0.95];
    const COLOR_Y: [f32; 4] = [0.30, 0.85, 0.35, 0.95];
    const COLOR_Z: [f32; 4] = [0.20, 0.68, 1.0, 0.95];

    let color_for = |part: TransformGizmoPart, fallback: [f32; 4]| -> [f32; 4] {
        if active_part == Some(part) {
            COLOR_ACTIVE
        } else {
            fallback
        }
    };

    let arrow_len = s * 1.5;
    let head_size = s * 0.38;

    // 1. Tiga panah translasi linier
    push_arrow_3d(&mut verts, c, c + Vec3::X * arrow_len, color_for(TransformGizmoPart::TranslateX, COLOR_X), head_size);
    push_arrow_3d(&mut verts, c, c + Vec3::Y * arrow_len, color_for(TransformGizmoPart::TranslateY, COLOR_Y), head_size);
    push_arrow_3d(&mut verts, c, c + Vec3::Z * arrow_len, color_for(TransformGizmoPart::TranslateZ, COLOR_Z), head_size);

    // 2. Tiga kotak translasi bidang (Planar Tiles)
    let plane_offset = s * 0.65;
    let plane_size = s * 0.40;
    push_planar_tile(
        &mut verts,
        c + (Vec3::X + Vec3::Y) * plane_offset,
        Vec3::X,
        Vec3::Y,
        plane_size,
        color_for(TransformGizmoPart::PlaneXY, COLOR_BASE),
    );
    push_planar_tile(
        &mut verts,
        c + (Vec3::Y + Vec3::Z) * plane_offset,
        Vec3::Y,
        Vec3::Z,
        plane_size,
        color_for(TransformGizmoPart::PlaneYZ, COLOR_BASE),
    );
    push_planar_tile(
        &mut verts,
        c + (Vec3::Z + Vec3::X) * plane_offset,
        Vec3::Z,
        Vec3::X,
        plane_size,
        color_for(TransformGizmoPart::PlaneZX, COLOR_BASE),
    );

    // 3. Tiga busur rotasi melengkung (Rotation Arcs)
    let rot_radius = s * 1.05;
    let rot_arrow_size = s * 0.22;
    let (ang_start, ang_end) = (0.26, 1.31); // ~15 deg s.d. 75 deg di kuadran positif
    // Rotasi sekeliling Z (di bidang XY)
    push_curved_rotation_arc(
        &mut verts,
        c,
        Vec3::X,
        Vec3::Y,
        rot_radius,
        ang_start,
        ang_end,
        color_for(TransformGizmoPart::RotateZ, COLOR_Z),
        rot_arrow_size,
    );
    // Rotasi sekeliling X (di bidang YZ)
    push_curved_rotation_arc(
        &mut verts,
        c,
        Vec3::Y,
        Vec3::Z,
        rot_radius,
        ang_start,
        ang_end,
        color_for(TransformGizmoPart::RotateX, COLOR_X),
        rot_arrow_size,
    );
    // Rotasi sekeliling Y (di bidang ZX)
    push_curved_rotation_arc(
        &mut verts,
        c,
        Vec3::Z,
        Vec3::X,
        rot_radius,
        ang_start,
        ang_end,
        color_for(TransformGizmoPart::RotateY, COLOR_Y),
        rot_arrow_size,
    );

    // 4. Center Pivot Ring
    let pivot_color = color_for(TransformGizmoPart::CenterPivot, [1.0, 1.0, 1.0, 0.9]);
    let pivot_r = s * 0.12;
    let segs = 12;
    let tau = std::f32::consts::TAU;
    for i in 0..segs {
        let a0 = tau * (i as f32 / segs as f32);
        let a1 = tau * ((i + 1) as f32 / segs as f32);
        for (u, v) in [(Vec3::X, Vec3::Y), (Vec3::Y, Vec3::Z), (Vec3::Z, Vec3::X)] {
            let p0 = c + u * (pivot_r * a0.cos()) + v * (pivot_r * a0.sin());
            let p1 = c + u * (pivot_r * a1.cos()) + v * (pivot_r * a1.sin());
            verts.push(LineVertex { position: [p0.x, p0.y, p0.z], color: pivot_color });
            verts.push(LineVertex { position: [p1.x, p1.y, p1.z], color: pivot_color });
        }
    }

    verts
}

/// Shapr3D-Style 3D Transform Gizmo Solid Mesh:
/// Menghasilkan mesh solid 3D shaded (triangles, normals, colors, indices)
/// dengan kualitas dan pencahayaan konsisten seperti gizmo extrude:
/// - 3 Panah translasi solid (silinder + kerucut solid)
/// - 3 Kotak translasi planar solid double-sided
/// - 3 Busur rotasi tabung melengkung solid (torus arc) dengan panah kerucut solid
/// - 1 Bola pivot pusat solid
///
/// `eye_pos`: posisi kamera opsional — jika diberikan, setiap panah sumbu akan
/// di-flip agar selalu **mengarah menjauhi kamera** (camera-facing), sehingga
/// gizmo tidak pernah terlihat terbalik dari sudut pandang manapun.
#[allow(clippy::type_complexity)]
pub fn solid_shapr3d_transform_gizmo_mesh(
    center_pos: [f32; 3],
    gizmo_scale: f32,
    active_part: Option<TransformGizmoPart>,
    eye_pos: Option<Vec3>,
) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 4]>, Vec<u32>) {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();

    let c = Vec3::from(center_pos);
    let s = gizmo_scale.max(10.0);
    let tau = std::f32::consts::TAU;

    // Modern premium color palette — lebih vibrant, alpha tinggi
    const COLOR_ACTIVE: [f32; 4] = [1.0, 0.88, 0.15, 1.0];  // highlight kuning emas
    const COLOR_X: [f32; 4] = [0.98, 0.22, 0.22, 1.0];       // merah coral vibrant
    const COLOR_Y: [f32; 4] = [0.15, 0.92, 0.35, 1.0];       // hijau neon mint
    const COLOR_Z: [f32; 4] = [0.20, 0.60, 1.0, 1.0];        // biru langit cerah

    // Hitung per-sumbu apakah harus di-flip agar panah menghadap ke kamera.
    // dot(eye - center, axis) < 0 artinya kamera berada di sisi negatif sumbu →
    // flip sehingga panah selalu menunjuk ke arah kamera (camera-facing).
    let (dir_x, dir_y, dir_z) = if let Some(eye) = eye_pos {
        let to_eye = eye - c;
        let dx = if to_eye.dot(Vec3::X) >= 0.0 { Vec3::X } else { Vec3::NEG_X };
        let dy = if to_eye.dot(Vec3::Y) >= 0.0 { Vec3::Y } else { Vec3::NEG_Y };
        let dz = if to_eye.dot(Vec3::Z) >= 0.0 { Vec3::Z } else { Vec3::NEG_Z };
        (dx, dy, dz)
    } else {
        (Vec3::X, Vec3::Y, Vec3::Z)
    };

    let color_for = |part: TransformGizmoPart, fallback: [f32; 4]| -> [f32; 4] {
        if active_part == Some(part) {
            COLOR_ACTIVE
        } else {
            fallback
        }
    };

    let _push_tri = |pos: &mut Vec<[f32; 3]>,
                    norm: &mut Vec<[f32; 3]>,
                    col: &mut Vec<[f32; 4]>,
                    ind: &mut Vec<u32>,
                    a: Vec3,
                    b: Vec3,
                    c: Vec3,
                    color: [f32; 4],
                    outward_hint: Vec3| {
        let mut face_n = (b - a).cross(c - a);
        let (vb, vc) = if outward_hint != Vec3::ZERO && face_n.dot(outward_hint) < 0.0 {
            face_n = -face_n;
            (c, b)
        } else {
            (b, c)
        };
        let face_n = face_n.normalize_or_zero();
        let base_idx = pos.len() as u32;
        for p in [a, vb, vc] {
            pos.push([p.x, p.y, p.z]);
            norm.push([face_n.x, face_n.y, face_n.z]);
            col.push(color);
        }
        ind.extend_from_slice(&[base_idx, base_idx + 1, base_idx + 2]);
    };

    // push_cone: kerucut solid dengan smooth vertex normals (lebih halus dari flat-shading).
    let push_cone = |pos: &mut Vec<[f32; 3]>,
                     norm: &mut Vec<[f32; 3]>,
                     col: &mut Vec<[f32; 4]>,
                     ind: &mut Vec<u32>,
                     apex: Vec3,
                     base_center: Vec3,
                     radius: f32,
                     color: [f32; 4]| {
        let dir = (apex - base_center).normalize_or_zero();
        if dir == Vec3::ZERO { return; }
        let (t1, t2) = if dir.z.abs() < 0.9 {
            let t1 = dir.cross(Vec3::Z).normalize();
            let t2 = dir.cross(t1).normalize();
            (t1, t2)
        } else {
            let t1 = dir.cross(Vec3::Y).normalize();
            let t2 = dir.cross(t1).normalize();
            (t1, t2)
        };
        let height = (apex - base_center).length();
        // sin/cos dari sudut semi-angle kerucut untuk normal miring
        let cone_sin = radius / (radius * radius + height * height).sqrt();
        let cone_cos = height / (radius * radius + height * height).sqrt();
        let segs: u32 = 24; // sangat halus
        for i in 0..segs {
            let a0 = tau * (i as f32 / segs as f32);
            let a1 = tau * ((i + 1) as f32 / segs as f32);
            let rad0 = t1 * a0.cos() + t2 * a0.sin();
            let rad1 = t1 * a1.cos() + t2 * a1.sin();
            let b0 = base_center + rad0 * radius;
            let b1 = base_center + rad1 * radius;
            // Normal miring (smooth) untuk kerucut: campuran radial + axial
            let sn_apex = dir; // di apex, normal ≈ axial
            let sn0 = (rad0 * cone_cos + dir * cone_sin).normalize_or_zero();
            let sn1 = (rad1 * cone_cos + dir * cone_sin).normalize_or_zero();
            // Sisi kerucut — per-vertex normal
            let base_idx = pos.len() as u32;
            pos.push([apex.x, apex.y, apex.z]); norm.push([sn_apex.x, sn_apex.y, sn_apex.z]); col.push(color);
            pos.push([b0.x, b0.y, b0.z]);       norm.push([sn0.x, sn0.y, sn0.z]);             col.push(color);
            pos.push([b1.x, b1.y, b1.z]);       norm.push([sn1.x, sn1.y, sn1.z]);             col.push(color);
            ind.extend_from_slice(&[base_idx, base_idx + 1, base_idx + 2]);
            // Cap bawah (flat)
            let cap_n = -dir;
            let bi = pos.len() as u32;
            pos.push([base_center.x, base_center.y, base_center.z]); norm.push([cap_n.x, cap_n.y, cap_n.z]); col.push(color);
            pos.push([b0.x, b0.y, b0.z]);                           norm.push([cap_n.x, cap_n.y, cap_n.z]); col.push(color);
            pos.push([b1.x, b1.y, b1.z]);                           norm.push([cap_n.x, cap_n.y, cap_n.z]); col.push(color);
            ind.extend_from_slice(&[bi, bi + 2, bi + 1]); // winding berlawanan untuk cap
        }
    };


    // push_cylinder: silinder solid dengan smooth vertex normals + end caps.
    let push_cylinder = |pos: &mut Vec<[f32; 3]>,
                         norm: &mut Vec<[f32; 3]>,
                         col: &mut Vec<[f32; 4]>,
                         ind: &mut Vec<u32>,
                         start: Vec3,
                         end: Vec3,
                         radius: f32,
                         color: [f32; 4]| {
        let dir = (end - start).normalize_or_zero();
        if dir == Vec3::ZERO { return; }
        let (t1, t2) = if dir.z.abs() < 0.9 {
            let t1 = dir.cross(Vec3::Z).normalize();
            let t2 = dir.cross(t1).normalize();
            (t1, t2)
        } else {
            let t1 = dir.cross(Vec3::Y).normalize();
            let t2 = dir.cross(t1).normalize();
            (t1, t2)
        };
        let segs: u32 = 20; // sangat halus
        for i in 0..segs {
            let a0 = tau * (i as f32 / segs as f32);
            let a1 = tau * ((i + 1) as f32 / segs as f32);
            let rad0 = t1 * a0.cos() + t2 * a0.sin();
            let rad1 = t1 * a1.cos() + t2 * a1.sin();
            let ps0 = start + rad0 * radius;
            let pe0 = end   + rad0 * radius;
            let ps1 = start + rad1 * radius;
            let pe1 = end   + rad1 * radius;
            // Sisi silinder: smooth radial normals per-vertex
            let bi = pos.len() as u32;
            pos.extend([[ps0.x,ps0.y,ps0.z],[pe0.x,pe0.y,pe0.z],[pe1.x,pe1.y,pe1.z],[ps1.x,ps1.y,ps1.z]]);
            norm.extend([[rad0.x,rad0.y,rad0.z],[rad0.x,rad0.y,rad0.z],[rad1.x,rad1.y,rad1.z],[rad1.x,rad1.y,rad1.z]]);
            col.extend([color; 4]);
            ind.extend_from_slice(&[bi,bi+1,bi+2, bi,bi+2,bi+3]);
            // Cap start (flat)
            let ns = [-dir.x,-dir.y,-dir.z];
            let ci = pos.len() as u32;
            pos.extend([[start.x,start.y,start.z],[ps0.x,ps0.y,ps0.z],[ps1.x,ps1.y,ps1.z]]);
            norm.extend([ns,ns,ns]); col.extend([color;3]);
            ind.extend_from_slice(&[ci,ci+2,ci+1]);
            // Cap end (flat)
            let ne = [dir.x,dir.y,dir.z];
            let ei = pos.len() as u32;
            pos.extend([[end.x,end.y,end.z],[pe0.x,pe0.y,pe0.z],[pe1.x,pe1.y,pe1.z]]);
            norm.extend([ne,ne,ne]); col.extend([color;3]);
            ind.extend_from_slice(&[ei,ei+1,ei+2]);
        }
    };



    let push_torus_arc = |pos: &mut Vec<[f32; 3]>,
                          norm: &mut Vec<[f32; 3]>,
                          col: &mut Vec<[f32; 4]>,
                          ind: &mut Vec<u32>,
                          center: Vec3,
                          u_axis: Vec3,
                          v_axis: Vec3,
                          radius: f32,
                          tube_r: f32,
                          ang_start: f32,
                          ang_end: f32,
                          arrow_size: f32,
                          color: [f32; 4]| {
        let arc_segs = 28;  // busur sangat halus
        let ring_segs = 12; // penampang lingkaran sangat halus
        let axis_n = u_axis.cross(v_axis).normalize_or_zero();

        for i in 0..arc_segs {
            let t0 = ang_start + (ang_end - ang_start) * (i as f32 / arc_segs as f32);
            let t1 = ang_start + (ang_end - ang_start) * ((i + 1) as f32 / arc_segs as f32);

            let c0 = center + u_axis * (radius * t0.cos()) + v_axis * (radius * t0.sin());
            let c1 = center + u_axis * (radius * t1.cos()) + v_axis * (radius * t1.sin());

            let rad_dir0 = (u_axis * t0.cos() + v_axis * t0.sin()).normalize();
            let rad_dir1 = (u_axis * t1.cos() + v_axis * t1.sin()).normalize();

            for j in 0..ring_segs {
                let phi0 = tau * (j as f32 / ring_segs as f32);
                let phi1 = tau * ((j + 1) as f32 / ring_segs as f32);

                // Posisi titik di permukaan torus
                let r0_0 = rad_dir0 * (tube_r * phi0.cos()) + axis_n * (tube_r * phi0.sin());
                let r0_1 = rad_dir0 * (tube_r * phi1.cos()) + axis_n * (tube_r * phi1.sin());
                let r1_0 = rad_dir1 * (tube_r * phi0.cos()) + axis_n * (tube_r * phi0.sin());
                let r1_1 = rad_dir1 * (tube_r * phi1.cos()) + axis_n * (tube_r * phi1.sin());

                // Normal outward per vertex (smooth shading)
                let n00 = r0_0.normalize_or_zero();
                let n01 = r0_1.normalize_or_zero();
                let n10 = r1_0.normalize_or_zero();
                let n11 = r1_1.normalize_or_zero();

                let bi = pos.len() as u32;
                let p00 = c0 + r0_0; let p01 = c0 + r0_1;
                let p10 = c1 + r1_0; let p11 = c1 + r1_1;
                pos.extend([[p00.x,p00.y,p00.z],[p10.x,p10.y,p10.z],[p11.x,p11.y,p11.z],[p01.x,p01.y,p01.z]]);
                norm.extend([[n00.x,n00.y,n00.z],[n10.x,n10.y,n10.z],[n11.x,n11.y,n11.z],[n01.x,n01.y,n01.z]]);
                col.extend([color; 4]);
                ind.extend_from_slice(&[bi,bi+1,bi+2, bi,bi+2,bi+3]);
            }
        }

        // Arrowhead di ujung awal dan akhir busur rotasi
        let p_start = center + u_axis * (radius * ang_start.cos()) + v_axis * (radius * ang_start.sin());
        let tan_start = (-u_axis * ang_start.sin() + v_axis * ang_start.cos()).normalize();
        push_cone(pos, norm, col, ind, p_start - tan_start * (arrow_size * 1.1), p_start, arrow_size * 0.45, color);

        let p_end = center + u_axis * (radius * ang_end.cos()) + v_axis * (radius * ang_end.sin());
        let tan_end = (-u_axis * ang_end.sin() + v_axis * ang_end.cos()).normalize();
        push_cone(pos, norm, col, ind, p_end + tan_end * (arrow_size * 1.1), p_end, arrow_size * 0.45, color);
    };


    // 1. Tiga panah translasi linier solid (X, Y, Z)
    // Gunakan dir_x/dir_y/dir_z yang sudah di-flip agar selalu camera-facing.
    let arrow_len = s * 1.60;
    let head_size = s * 0.42;
    let cone_r = s * 0.15;   // kepala lebih besar — lebih mudah diklik
    let shaft_r = s * 0.048; // batang lebih tebal — lebih visible

    for (part, dir, col) in [
        (TransformGizmoPart::TranslateX, dir_x, COLOR_X),
        (TransformGizmoPart::TranslateY, dir_y, COLOR_Y),
        (TransformGizmoPart::TranslateZ, dir_z, COLOR_Z),
    ] {
        let color = color_for(part, col);
        let shaft_start = c + dir * (s * 0.14);
        let shaft_end = c + dir * (arrow_len - head_size);
        let apex = c + dir * arrow_len;
        push_cylinder(&mut positions, &mut normals, &mut colors, &mut indices, shaft_start, shaft_end, shaft_r, color);
        push_cone(&mut positions, &mut normals, &mut colors, &mut indices, apex, shaft_end, cone_r, color);
    }

    // 3. Tiga busur rotasi solid melengkung (Rotate X, Y, Z)
    // Busur juga menggunakan dir_* yang camera-facing agar tampil di kuadran yang visible.
    let rot_radius = s * 1.08;
    let rot_tube_r = s * 0.042;  // tabung lebih tebal
    let rot_arrow_size = s * 0.22;
    let (ang_start, ang_end) = (0.22, 1.35);

    // Rotate Z di bidang camera-facing XY
    push_torus_arc(
        &mut positions, &mut normals, &mut colors, &mut indices,
        c, dir_x, dir_y, rot_radius, rot_tube_r, ang_start, ang_end, rot_arrow_size,
        color_for(TransformGizmoPart::RotateZ, COLOR_Z),
    );
    // Rotate X di bidang camera-facing YZ
    push_torus_arc(
        &mut positions, &mut normals, &mut colors, &mut indices,
        c, dir_y, dir_z, rot_radius, rot_tube_r, ang_start, ang_end, rot_arrow_size,
        color_for(TransformGizmoPart::RotateX, COLOR_X),
    );
    // Rotate Y di bidang camera-facing ZX
    push_torus_arc(
        &mut positions, &mut normals, &mut colors, &mut indices,
        c, dir_z, dir_x, rot_radius, rot_tube_r, ang_start, ang_end, rot_arrow_size,
        color_for(TransformGizmoPart::RotateY, COLOR_Y),
    );

    // 4. Center Pivot Solid Sphere — smooth dengan 16×24 segmen
    let pivot_color = color_for(TransformGizmoPart::CenterPivot, [0.96, 0.96, 1.0, 1.0]);
    let pivot_r = s * 0.13;
    let lat_segs = 16; // dari 6 → 16
    let lon_segs = 24; // dari 10 → 24
    for i in 0..lat_segs {
        let lat0 = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * (i as f32 / lat_segs as f32);
        let lat1 = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * ((i + 1) as f32 / lat_segs as f32);
        for j in 0..lon_segs {
            let lon0 = tau * (j as f32 / lon_segs as f32);
            let lon1 = tau * ((j + 1) as f32 / lon_segs as f32);

            let get_p = |lat: f32, lon: f32| -> Vec3 {
                c + Vec3::new(lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()) * pivot_r
            };
            let get_n = |lat: f32, lon: f32| -> [f32; 3] {
                // Normal = arah radial (outward) untuk smooth sphere
                [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
            };

            let p00 = get_p(lat0, lon0); let n00 = get_n(lat0, lon0);
            let p10 = get_p(lat1, lon0); let n10 = get_n(lat1, lon0);
            let p11 = get_p(lat1, lon1); let n11 = get_n(lat1, lon1);
            let p01 = get_p(lat0, lon1); let n01 = get_n(lat0, lon1);

            let bi = positions.len() as u32;
            positions.extend([[p00.x,p00.y,p00.z],[p10.x,p10.y,p10.z],[p11.x,p11.y,p11.z],[p01.x,p01.y,p01.z]]);
            normals.extend([n00, n10, n11, n01]);
            colors.extend([pivot_color; 4]);
            indices.extend_from_slice(&[bi,bi+1,bi+2, bi,bi+2,bi+3]);
        }
    }

    (positions, normals, colors, indices)
}

