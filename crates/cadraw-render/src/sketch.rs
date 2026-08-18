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

/// Kepala panah kecil bentuk V di kedua ujung garis pengukuran (bergaya
/// dimension line CAD standar `↔`), TIDAK di vertex tengah untuk kasus
/// Ukur Sudut (3 titik, 2 segmen) — cuma titik pertama & terakhir dari
/// urutan `Measurement::points()` yang dapat kepala panah, supaya vertex
/// sudut tidak kelihatan bercabang tiga. Dipisah dari `measurement_lines`
/// (garis penghubung polos) supaya keduanya bisa dipakai independen.
pub fn measurement_arrowheads(points: &[DVec2], plane: &SketchPlane) -> Vec<LineVertex> {
    const HEAD_LEN: f64 = 4.0;
    const HEAD_ANGLE: f64 = 0.45; // ~26 derajat dari sumbu garis

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
            verts.push(LineVertex { position: to3(plane, tip), color: COLOR_MEASURE });
            verts.push(LineVertex { position: to3(plane, wing), color: COLOR_MEASURE });
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

/// Versi SOLID (bukan wireframe) dari `double_arrow_gizmo_lines` (CADRAW
/// Fase 9 — Icon Gizmo Profesional): silhouette-nya SENGAJA identik
/// (poros tengah + 2 kepala kerucut di ujung, proporsi sama) — cuma sisi
/// kerucut & poros diisi segitiga solid (flat-shaded, tiap segitiga punya
/// vertex-nya sendiri supaya normal-nya tegas per-wajah, hasilnya kesan
/// "gem"/facet yang tajam & profesional, bukan smooth-shaded yang blur)
/// alih-alih rusuk garis kawat. Dipakai lewat pipeline mesh yang SAMA
/// dengan body CAD (`SceneRenderer::set_gizmo_mesh`, shader `fs_mesh`)
/// supaya shading-nya (ambient floor + rim light) konsisten & terasa
/// benar-benar solid, bukan icon UI 2D yang ditempel di atas scene.
/// `cull_mode: None` di `mesh_pipeline`, jadi winding triangle tidak
/// wajib konsisten — tapi `push_tri` tetap membetulkan urutan vertex
/// berdasar `outward_hint` supaya arah NORMAL (dipakai shading) selalu
/// menghadap keluar, bukan cuma soal visibility.
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

    // Segitiga flat-shaded: normal dihitung dari winding a->b->c, lalu
    // ditukar b/c kalau hasilnya berlawanan dgn `outward_hint` — jadi
    // pemanggil cukup kasih perkiraan arah "keluar" kasar (radial cukup,
    // tidak perlu presisi), bukan normal final.
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

    // 1. Poros tengah (silinder solid tipis) — direntang persis sepanjang
    // segmen yang TERLIHAT (antara dasar kedua kerucut); ujungnya yang
    // menembus ke dalam kerucut otomatis tertutup tutup dasar kerucut di
    // bawah, jadi tidak perlu tutup sendiri.
    let shaft_top = top - n * (s * 1.0);
    let shaft_bot = bot + n * (s * 1.0);
    for i in 0..segs {
        let a0 = tau * (i as f32 / segs as f32);
        let a1 = tau * ((i + 1) as f32 / segs as f32);
        let r0 = t1 * (shaft_radius * a0.cos()) + t2 * (shaft_radius * a0.sin());
        let r1 = t1 * (shaft_radius * a1.cos()) + t2 * (shaft_radius * a1.sin());
        let hint = r0 + r1;
        push_tri(&mut positions, &mut normals, &mut colors, &mut indices, shaft_bot + r0, shaft_top + r0, shaft_top + r1, hint);
        push_tri(&mut positions, &mut normals, &mut colors, &mut indices, shaft_bot + r0, shaft_top + r1, shaft_bot + r1, hint);
    }

    // 2 & 3. Kepala panah atas & bawah: kerucut solid (sisi + tutup dasar)
    // menunjuk keluar dari pusat, radius dasar `s` di titik `*_base`
    // (sama persis dgn geometri wireframe lama).
    let mut push_cone = |apex: Vec3, base_center: Vec3| {
        for i in 0..segs {
            let a0 = tau * (i as f32 / segs as f32);
            let a1 = tau * ((i + 1) as f32 / segs as f32);
            let b0 = base_center + t1 * (s * a0.cos()) + t2 * (s * a0.sin());
            let b1 = base_center + t1 * (s * a1.cos()) + t2 * (s * a1.sin());
            let radial_hint = (b0 - base_center) + (b1 - base_center);
            // Sisi kerucut: normal menghadap keluar (radial + sedikit axial).
            push_tri(&mut positions, &mut normals, &mut colors, &mut indices, apex, b0, b1, radial_hint);
            // Tutup dasar: menghadap ke arah BERLAWANAN dgn apex (ke dalam gizmo).
            let cap_hint = base_center - apex;
            push_tri(&mut positions, &mut normals, &mut colors, &mut indices, base_center, b0, b1, cap_hint);
        }
    };
    push_cone(top, top - n * (s * 1.3));
    push_cone(bot, bot + n * (s * 1.3));

    (positions, normals, colors, indices)
}

/// Marker gizmo vertex fillet 3D (CADRAW Fase 3 — Rounded Sudut): kotak
/// kawat kecil TEPAT di `vertex`, garis putus-putus dari `vertex` ke posisi
/// handle sejauh `handle_dist` di sepanjang `out_dir` (arah "keluar" body,
/// lihat `App::active_vertex_gizmo_dir`), dan ikon kuadran lingkaran kecil
/// (melambangkan "rounding") digambar pada bidang tangent thd `out_dir` di
/// dekat handle. `out_dir` TIDAK perlu sudah ternormalisasi. Warna sengaja
/// dibedakan dari `FACE_GIZMO_COLOR` (cyan, dipakai gizmo extrude/push-pull
/// face) supaya kedua gizmo tidak tertukar secara visual saat sudut & sisi
/// body berdekatan di layar.
pub fn vertex_fillet_marker_lines(
    vertex: [f32; 3],
    out_dir: Vec3,
    handle_dist: f32,
    color: [f32; 4],
) -> Vec<LineVertex> {
    let mut verts = Vec::new();
    let v = Vec3::from(vertex);
    let n = out_dir.normalize_or_zero();

    // 1. Marker kotak kawat kecil persis di titik vertex.
    const S: f32 = 1.2;
    let corners = [
        Vec3::new(-S, -S, -S), Vec3::new(S, -S, -S), Vec3::new(S, S, -S), Vec3::new(-S, S, -S),
        Vec3::new(-S, -S, S), Vec3::new(S, -S, S), Vec3::new(S, S, S), Vec3::new(-S, S, S),
    ];
    const EDGES: [(usize, usize); 12] = [
        (0, 1), (1, 2), (2, 3), (3, 0),
        (4, 5), (5, 6), (6, 7), (7, 4),
        (0, 4), (1, 5), (2, 6), (3, 7),
    ];
    for (a, b) in EDGES {
        let pa = v + corners[a];
        let pb = v + corners[b];
        verts.push(LineVertex { position: [pa.x, pa.y, pa.z], color });
        verts.push(LineVertex { position: [pb.x, pb.y, pb.z], color });
    }

    if n == Vec3::ZERO {
        return verts;
    }

    // 2. Garis putus-putus dari vertex ke posisi handle di arah `out_dir`.
    let handle = v + n * handle_dist;
    let handle_arr = [handle.x, handle.y, handle.z];
    verts.extend(dashed_line_3d(vertex, handle_arr, 2.0, color));

    // 3. Ikon kuadran lingkaran kecil di dekat handle, pada bidang tangent
    // thd `out_dir` (basis sama seperti `double_arrow_gizmo_lines` pakai
    // utk kepala panah, supaya orientasinya konsisten scr visual).
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
        verts.push(LineVertex { position: [prev.x, prev.y, prev.z], color });
        verts.push(LineVertex { position: [p.x, p.y, p.z], color });
        prev = p;
    }

    verts
}

/// Marker kecil (silang 3 sumbu) di tiap titik `vertices` — dipakai
/// menggambar SEMUA sudut (vertex) body 3D saat mode 3D supaya target
/// picking vertex/gizmo rounding (`vertex_fillet_marker_lines`, gizmo edge
/// fillet) TERLIHAT sebelum diklik, bukan cuma target invisible ±piksel
/// (keluhan awal fitur ini: klik "sudut kubus" sering meleset ke rusuk
/// karena vertex-nya sendiri tidak pernah digambar). `hover_point`, kalau
/// ada, dicocokkan lewat jarak epsilon ke salah satu `vertices` dan
/// digambar lebih besar + `hover_color` supaya user tahu sudut mana yang
/// bakal kena kalau diklik sekarang — pola sama dgn highlight hover
/// entitas sketch 2D di `entity_lines`.
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
            (h[0] - p[0]).abs() < HOVER_EPS && (h[1] - p[1]).abs() < HOVER_EPS && (h[2] - p[2]).abs() < HOVER_EPS
        });
        let c = if is_hover { hover_color } else { color };
        let s = if is_hover { 2.2 } else { 1.0 };
        let v = Vec3::from(*p);
        for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
            let a = v - axis * s;
            let b = v + axis * s;
            verts.push(LineVertex { position: [a.x, a.y, a.z], color: c });
            verts.push(LineVertex { position: [b.x, b.y, b.z], color: c });
        }
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
    fn measurement_arrowheads_empty_for_single_point() {
        let plane = SketchPlane::top();
        assert!(measurement_arrowheads(&[DVec2::new(0.0, 0.0)], &plane).is_empty());
    }

    #[test]
    fn measurement_arrowheads_both_ends_for_two_points() {
        // 2 titik -> 1 segmen -> kepala panah di KEDUA ujung (kiri & kanan),
        // masing-masing 2 wing (V shape) x 2 vertex/garis = 4 vertex per ujung.
        let plane = SketchPlane::top();
        let verts = measurement_arrowheads(&[DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0)], &plane);
        assert_eq!(verts.len(), 8);
    }

    #[test]
    fn measurement_arrowheads_skip_shared_vertex_for_three_points() {
        // 3 titik (Ukur Sudut: a, vertex, b) -> tetap cuma 2 ujung yang dapat
        // panah (titik pertama & terakhir), vertex tengah TIDAK dapat panah.
        let plane = SketchPlane::top();
        let verts = measurement_arrowheads(
            &[DVec2::new(0.0, 0.0), DVec2::new(1.0, 1.0), DVec2::new(2.0, 0.0)],
            &plane,
        );
        assert_eq!(verts.len(), 8);
    }

    #[test]
    fn measurement_arrowheads_degenerate_coincident_points_no_panic() {
        // Titik awal berimpit dengan titik kedua -> arah kepala panah nol ->
        // tidak boleh panic (division oleh nol dsb), cukup skip kepala itu.
        let plane = SketchPlane::top();
        let verts = measurement_arrowheads(&[DVec2::new(3.0, 3.0), DVec2::new(3.0, 3.0)], &plane);
        assert!(verts.is_empty());
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

    #[test]
    fn solid_double_arrow_gizmo_mesh_produces_valid_triangle_soup() {
        let (positions, normals, colors, indices) =
            solid_double_arrow_gizmo_mesh([0.0, 0.0, 0.0], 22.0, 5.0, [0.0, 0.78, 1.0, 1.0], Vec3::Z);

        assert!(!positions.is_empty());
        assert_eq!(positions.len(), normals.len());
        assert_eq!(positions.len(), colors.len());
        // Non-indexed triangle soup (flat-shaded): 1 vertex unik per sudut segitiga.
        assert_eq!(indices.len(), positions.len());
        assert_eq!(indices.len() % 3, 0);

        for idx in &indices {
            assert!((*idx as usize) < positions.len());
        }
        for p in &positions {
            assert!(p.iter().all(|v| v.is_finite()));
        }
        for n in &normals {
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            // Semua normal harus satuan (flat-shaded per segitiga) — kecuali
            // segitiga degenerate (tidak terjadi di geometri kerucut/silinder ini).
            assert!((len - 1.0).abs() < 1e-3, "normal length = {len}");
        }
        for c in &colors {
            assert_eq!(*c, [0.0, 0.78, 1.0, 1.0]);
        }
    }

    #[test]
    fn solid_double_arrow_gizmo_mesh_empty_for_zero_normal() {
        let (positions, normals, colors, indices) =
            solid_double_arrow_gizmo_mesh([0.0, 0.0, 0.0], 22.0, 5.0, [0.0, 0.78, 1.0, 1.0], Vec3::ZERO);
        assert!(positions.is_empty());
        assert!(normals.is_empty());
        assert!(colors.is_empty());
        assert!(indices.is_empty());
    }

    #[test]
    fn solid_double_arrow_gizmo_mesh_scales_with_arrow_size() {
        // Semua vertex kerucut/poros harus tetap berada dalam bounding radius
        // proporsional thd `arrow_size` (radius dasar kerucut) — jaminan dasar
        // supaya gizmo yang di-skala kecil (Fase 9, skala berbasis piksel layar)
        // benar-benar mengecil, bukan diam di ukuran tetap.
        let (small, ..) = solid_double_arrow_gizmo_mesh([0.0, 0.0, 0.0], 4.0, 1.0, [1.0, 1.0, 1.0, 1.0], Vec3::Z);
        let (big, ..) = solid_double_arrow_gizmo_mesh([0.0, 0.0, 0.0], 40.0, 10.0, [1.0, 1.0, 1.0, 1.0], Vec3::Z);

        let max_radial = |verts: &[[f32; 3]]| -> f32 {
            verts.iter().map(|p| (p[0] * p[0] + p[1] * p[1]).sqrt()).fold(0.0, f32::max)
        };
        assert!(max_radial(&big) > max_radial(&small) * 5.0);
    }
}
