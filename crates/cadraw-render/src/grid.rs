use bytemuck::{Pod, Zeroable};
use glam::Vec3;

use crate::plane::{PlaneKind, SketchPlane};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct LineVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

const MINOR: [f32; 4] = [0.42, 0.44, 0.48, 0.35];
const MAJOR: [f32; 4] = [0.55, 0.57, 0.62, 0.55];
const AXIS_X: [f32; 4] = [0.85, 0.25, 0.25, 1.0];
const AXIS_Y: [f32; 4] = [0.30, 0.72, 0.30, 1.0];
const AXIS_Z: [f32; 4] = [0.30, 0.45, 0.90, 1.0];

/// Grid bidang XY (Z-up) bawaan: garis minor tiap `step`, mayor tiap 10×step.
pub fn generate_grid(half_extent: f32, step: f32) -> Vec<LineVertex> {
    generate_grid_for_plane(&SketchPlane::top(), half_extent, step)
}

/// Grid untuk bidang sketsa tertentu (`Top`, `Front`, atau `Right`),
/// berdiri tegak atau horizontal sesuai orientasi bidang dengan warna sumbu RGB standar CAD.
pub fn generate_grid_for_plane(plane: &SketchPlane, half_extent: f32, step: f32) -> Vec<LineVertex> {
    let mut verts = Vec::new();
    let n = (half_extent / step).round() as i32;
    let mut push_line = |a: Vec3, b: Vec3, color: [f32; 4]| {
        verts.push(LineVertex { position: [a.x, a.y, a.z], color });
        verts.push(LineVertex { position: [b.x, b.y, b.z], color });
    };

    let (u_color, v_color, n_color) = match plane.kind {
        PlaneKind::Top => (AXIS_X, AXIS_Y, AXIS_Z),
        PlaneKind::Front => (AXIS_X, AXIS_Z, AXIS_Y),
        PlaneKind::Right => (AXIS_Y, AXIS_Z, AXIS_X),
    };

    let u = plane.u_axis;
    let v = plane.v_axis;
    let normal = plane.normal;
    let origin = plane.origin;

    for i in -n..=n {
        let d = i as f32 * step;
        if i == 0 {
            continue; // sumbu utama digambar terpisah dengan warna sendiri
        }
        let color = if i % 10 == 0 { MAJOR } else { MINOR };
        // Garis sejajar sumbu V
        push_line(origin + u * d - v * half_extent, origin + u * d + v * half_extent, color);
        // Garis sejajar sumbu U
        push_line(origin - u * half_extent + v * d, origin + u * half_extent + v * d, color);
    }

    // Sumbu U utama
    push_line(origin - u * half_extent, origin + u * half_extent, u_color);
    // Sumbu V utama
    push_line(origin - v * half_extent, origin + v * half_extent, v_color);
    // Stub sumbu normal
    push_line(origin, origin + normal * (half_extent * 0.25), n_color);

    verts
}
