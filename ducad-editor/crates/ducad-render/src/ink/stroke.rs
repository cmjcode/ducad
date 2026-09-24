//! Pembangun geometri quad-strip untuk goresan tinta bertekanan (`InkPointRef` -> `InkVertex`).

use glam::{DVec2, Vec2};

use crate::plane::SketchPlane;

/// Titik referensi coretan tinta dengan posisi 2D (mm) dan tekanan stylus (0.0..1.0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InkPointRef {
    pub pos: [f32; 2],
    pub pressure: f32,
}

/// Varian kuas tinta untuk styling goresan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InkBrushKind {
    Pen,
    Pencil,
    Marker,
}

/// Konfigurasi kuas tinta.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InkBrushRef {
    pub color: [f32; 4],
    pub width_min: f32,
    pub width_max: f32,
    pub opacity: f32,
    pub kind: InkBrushKind,
}

impl Default for InkBrushRef {
    fn default() -> Self {
        Self {
            color: [0.0, 0.0, 0.0, 1.0],
            width_min: 0.5,
            width_max: 2.5,
            opacity: 1.0,
            kind: InkBrushKind::Pen,
        }
    }
}

/// Vertex render GPU untuk strip goresan tinta (36 byte).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InkVertex {
    pub pos: [f32; 3],
    /// Posisi lateral melintasi lebar strip: -1.0 (kiri) .. 1.0 (kanan).
    pub side: f32,
    pub color: [f32; 4],
    /// Parameter kelembutan/gaya kuas (0.0 = Pen, 0.5 = Pencil, 1.0 = Marker).
    pub soft: f32,
}

/// Kumpulan vertex tinta yang sudah selesai per-layer.
#[derive(Debug, Clone)]
pub struct InkLayerBatch {
    pub layer: ducad_sketch::LayerId,
    pub vertices: Vec<InkVertex>,
}

/// Offset Z pada bidang sketsa untuk menghindari z-fighting dengan fill vektor.
pub const INK_Z_OFFSET: f32 = 0.005;

/// Hitung vektor arah/tangent untuk setiap titik goresan secara deterministik.
fn compute_tangents(points: &[InkPointRef]) -> Vec<Vec2> {
    let n = points.len();
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![Vec2::new(1.0, 0.0)];
    }

    let mut seg_dirs = Vec::with_capacity(n - 1);
    for i in 0..n - 1 {
        let p0 = Vec2::from(points[i].pos);
        let p1 = Vec2::from(points[i + 1].pos);
        let diff = p1 - p0;
        let len = diff.length();
        let dir = if len > 1e-6 && len.is_finite() {
            diff / len
        } else {
            Vec2::new(1.0, 0.0)
        };
        seg_dirs.push(dir);
    }

    let mut tangents = Vec::with_capacity(n);
    for i in 0..n {
        if i == 0 {
            tangents.push(seg_dirs[0]);
        } else if i == n - 1 {
            tangents.push(seg_dirs[n - 2]);
        } else {
            let t_in = seg_dirs[i - 1];
            let t_out = seg_dirs[i];
            let sum = t_in + t_out;
            let len = sum.length();
            let t = if len > 1e-4 && len.is_finite() {
                sum / len
            } else {
                t_in
            };
            tangents.push(t);
        }
    }

    tangents
}

/// Hitung normal miter pada titik `i`.
fn compute_miter_normal(points: &[InkPointRef], i: usize, tangents: &[Vec2]) -> Vec2 {
    let t = tangents[i];
    let n = Vec2::new(-t.y, t.x);
    let total = points.len();

    if i > 0 && i < total - 1 {
        let p_prev = Vec2::from(points[i - 1].pos);
        let p_curr = Vec2::from(points[i].pos);
        let in_diff = p_curr - p_prev;
        let in_len = in_diff.length();
        let t_in = if in_len > 1e-6 && in_len.is_finite() {
            in_diff / in_len
        } else {
            t
        };
        let n_in = Vec2::new(-t_in.y, t_in.x);
        let cos_half = n.dot(n_in).max(0.25);
        let miter_len = (1.0 / cos_half).min(4.0);
        n * miter_len
    } else {
        n
    }
}

/// Konteks gaya dan transformasi bidang untuk pembangunan geometri coretan tinta.
struct StrokeStyleCtx<'a> {
    brush: &'a InkBrushRef,
    plane: &'a SketchPlane,
    z_offset: f32,
    color: [f32; 4],
    soft: f32,
}

/// Hitung sepasang vertex (kiri & kanan) untuk titik `i`.
fn compute_point_pair(
    points: &[InkPointRef],
    i: usize,
    tangents: &[Vec2],
    ctx: &StrokeStyleCtx<'_>,
) -> (InkVertex, InkVertex) {
    let pt = points[i];
    let p = Vec2::from(pt.pos);
    let pressure = if pt.pressure.is_finite() {
        pt.pressure.clamp(0.0, 1.0)
    } else {
        0.5
    };
    let width = ctx.brush.width_min + (ctx.brush.width_max - ctx.brush.width_min) * pressure;
    let hw = width * 0.5;

    let normal = compute_miter_normal(points, i, tangents);
    let left_2d = p + normal * hw;
    let right_2d = p - normal * hw;

    let left_3d = ctx.plane.to_world(DVec2::new(left_2d.x as f64, left_2d.y as f64), ctx.z_offset);
    let right_3d = ctx.plane.to_world(DVec2::new(right_2d.x as f64, right_2d.y as f64), ctx.z_offset);

    (
        InkVertex {
            pos: [left_3d.x, left_3d.y, left_3d.z],
            side: -1.0,
            color: ctx.color,
            soft: ctx.soft,
        },
        InkVertex {
            pos: [right_3d.x, right_3d.y, right_3d.z],
            side: 1.0,
            color: ctx.color,
            soft: ctx.soft,
        },
    )
}

/// Hitung sepasang vertex start cap (2 vertex) pada titik awal goresan.
fn compute_start_cap(
    points: &[InkPointRef],
    tangents: &[Vec2],
    ctx: &StrokeStyleCtx<'_>,
) -> (InkVertex, InkVertex) {
    let pt0 = points[0];
    let p0 = Vec2::from(pt0.pos);
    let pressure = if pt0.pressure.is_finite() {
        pt0.pressure.clamp(0.0, 1.0)
    } else {
        0.5
    };
    let hw = (ctx.brush.width_min + (ctx.brush.width_max - ctx.brush.width_min) * pressure) * 0.5;
    let t0 = tangents[0];
    let n0 = Vec2::new(-t0.y, t0.x);

    let cap_center = p0 - t0 * hw;
    let cap_l = cap_center + n0 * (hw * 0.5);
    let cap_r = cap_center - n0 * (hw * 0.5);

    let l3d = ctx.plane.to_world(DVec2::new(cap_l.x as f64, cap_l.y as f64), ctx.z_offset);
    let r3d = ctx.plane.to_world(DVec2::new(cap_r.x as f64, cap_r.y as f64), ctx.z_offset);

    (
        InkVertex {
            pos: [l3d.x, l3d.y, l3d.z],
            side: -1.0,
            color: ctx.color,
            soft: ctx.soft,
        },
        InkVertex {
            pos: [r3d.x, r3d.y, r3d.z],
            side: 1.0,
            color: ctx.color,
            soft: ctx.soft,
        },
    )
}

/// Hitung sepasang vertex end cap (2 vertex) pada titik akhir goresan.
fn compute_end_cap(
    points: &[InkPointRef],
    tangents: &[Vec2],
    ctx: &StrokeStyleCtx<'_>,
) -> (InkVertex, InkVertex) {
    let last_idx = points.len() - 1;
    let pt_end = points[last_idx];
    let p_end = Vec2::from(pt_end.pos);
    let pressure = if pt_end.pressure.is_finite() {
        pt_end.pressure.clamp(0.0, 1.0)
    } else {
        0.5
    };
    let hw = (ctx.brush.width_min + (ctx.brush.width_max - ctx.brush.width_min) * pressure) * 0.5;
    let tn = tangents[last_idx];
    let nn = Vec2::new(-tn.y, tn.x);

    let cap_center = p_end + tn * hw;
    let cap_l = cap_center + nn * (hw * 0.5);
    let cap_r = cap_center - nn * (hw * 0.5);

    let l3d = ctx.plane.to_world(DVec2::new(cap_l.x as f64, cap_l.y as f64), ctx.z_offset);
    let r3d = ctx.plane.to_world(DVec2::new(cap_r.x as f64, cap_r.y as f64), ctx.z_offset);

    (
        InkVertex {
            pos: [l3d.x, l3d.y, l3d.z],
            side: -1.0,
            color: ctx.color,
            soft: ctx.soft,
        },
        InkVertex {
            pos: [r3d.x, r3d.y, r3d.z],
            side: 1.0,
            color: ctx.color,
            soft: ctx.soft,
        },
    )
}

/// Bangun quad-strip untuk seluruh goresan: 2 vertex per titik (kiri/kanan),
/// lebar = lerp(min, max, pressure), join miter terbatas, ujung round cap 4 vertex (2 awal, 2 akhir).
///
/// Menghasilkan tepat `2 * points.len() + 4` vertex untuk `points.len() >= 1`.
pub fn build_stroke_vertices(
    points: &[InkPointRef],
    brush: &InkBrushRef,
    plane: &SketchPlane,
    out: &mut Vec<InkVertex>,
) {
    if points.is_empty() {
        return;
    }

    let tangents = compute_tangents(points);
    let c = brush.color;
    let alpha = (c[3] * brush.opacity).clamp(0.0, 1.0);
    let color = [c[0], c[1], c[2], alpha];
    let soft = match brush.kind {
        InkBrushKind::Pen => 0.0,
        InkBrushKind::Pencil => 0.5,
        InkBrushKind::Marker => 1.0,
    };

    let ctx = StrokeStyleCtx {
        brush,
        plane,
        z_offset: INK_Z_OFFSET,
        color,
        soft,
    };

    out.reserve(2 * points.len() + 4);

    // 1. Start cap (2 vertex)
    let (cap_sl, cap_sr) = compute_start_cap(points, &tangents, &ctx);
    out.push(cap_sl);
    out.push(cap_sr);

    // 2. Interior points (2 vertex per titik)
    for i in 0..points.len() {
        let (vl, vr) = compute_point_pair(points, i, &tangents, &ctx);
        out.push(vl);
        out.push(vr);
    }

    // 3. End cap (2 vertex)
    let (cap_el, cap_er) = compute_end_cap(points, &tangents, &ctx);
    out.push(cap_el);
    out.push(cap_er);
}

/// Hanya tambah vertex untuk titik `[from..]` — `out` sudah berisi hasil titik sebelumnya.
/// Menghasilkan buffer identik dengan pemanggilan `build_stroke_vertices(points, ...)`.
pub fn append_stroke_vertices(
    points: &[InkPointRef],
    from: usize,
    brush: &InkBrushRef,
    plane: &SketchPlane,
    out: &mut Vec<InkVertex>,
) {
    if points.is_empty() {
        return;
    }

    if from == 0 || out.is_empty() || out.len() < 2 + 2 * from {
        out.clear();
        build_stroke_vertices(points, brush, plane, out);
        return;
    }

    // out berisi start cap (2) + previous points (2 * from) + previous end cap (2).
    // Buang end cap lama dari titik sebelumnya
    out.truncate(2 + 2 * from);

    let tangents = compute_tangents(points);
    let c = brush.color;
    let alpha = (c[3] * brush.opacity).clamp(0.0, 1.0);
    let color = [c[0], c[1], c[2], alpha];
    let soft = match brush.kind {
        InkBrushKind::Pen => 0.0,
        InkBrushKind::Pencil => 0.5,
        InkBrushKind::Marker => 1.0,
    };

    let ctx = StrokeStyleCtx {
        brush,
        plane,
        z_offset: INK_Z_OFFSET,
        color,
        soft,
    };

    // Perbarui titik `from - 1` karena sekarang ia menghubungkan ke titik baru
    let (vl_prev, vr_prev) = compute_point_pair(points, from - 1, &tangents, &ctx);
    let idx = 2 + 2 * (from - 1);
    out[idx] = vl_prev;
    out[idx + 1] = vr_prev;

    // Tambahkan titik-titik baru dari `from..points.len()`
    for i in from..points.len() {
        let (vl, vr) = compute_point_pair(points, i, &tangents, &ctx);
        out.push(vl);
        out.push(vr);
    }

    // Tambahkan end cap baru (2 vertex)
    let (cap_el, cap_er) = compute_end_cap(points, &tangents, &ctx);
    out.push(cap_el);
    out.push(cap_er);
}
