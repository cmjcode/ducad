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

/// Setengah-lebar "kartu" batas bidang non-aktif (bukan grid rapat 500 unit
/// milik bidang aktif) — dipakai SEKALIGUS untuk menggambar outline
/// ([`plane_outline`]) dan untuk hit-test klik/tap yang mengaktifkannya,
/// supaya area yang digambar & area yang bisa diklik selalu sama persis.
pub const INACTIVE_PLANE_HALF_EXTENT: f32 = 120.0;

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
        PlaneKind::Custom(_) => ([0.20, 0.80, 1.00, 1.0], [0.95, 0.70, 0.20, 1.0], [0.85, 0.25, 0.90, 1.0]),
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

/// Kerangka batas (bukan grid rapat) untuk bidang sketsa yang TIDAK aktif —
/// cuma persegi tipis + silang sumbu kecil di tengah, supaya ketiga bidang
/// tetap bisa dibedakan & diklik di viewport tanpa menumpuk garis grid penuh
/// di atas satu sama lain (lihat `INACTIVE_PLANE_HALF_EXTENT`).
pub fn plane_outline(plane: &SketchPlane, half_extent: f32, color: [f32; 4]) -> Vec<LineVertex> {
    let u = plane.u_axis;
    let v = plane.v_axis;
    let origin = plane.origin;

    let corner = |su: f32, sv: f32| origin + u * (su * half_extent) + v * (sv * half_extent);
    let corners = [
        corner(-1.0, -1.0),
        corner(1.0, -1.0),
        corner(1.0, 1.0),
        corner(-1.0, 1.0),
    ];

    let mut verts = Vec::new();
    let mut push_line = |a: Vec3, b: Vec3| {
        verts.push(LineVertex { position: [a.x, a.y, a.z], color });
        verts.push(LineVertex { position: [b.x, b.y, b.z], color });
    };

    for i in 0..4 {
        push_line(corners[i], corners[(i + 1) % 4]);
    }

    // Silang sumbu kecil di tengah, penanda orientasi u/v bidang.
    let cross = half_extent * 0.12;
    push_line(origin - u * cross, origin + u * cross);
    push_line(origin - v * cross, origin + v * cross);

    verts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_outline_vertex_count_is_boundary_plus_cross() {
        // 4 sisi persegi + 2 garis silang tengah = 6 segmen = 12 vertex.
        let verts = plane_outline(&SketchPlane::front(), 120.0, [1.0, 0.0, 0.0, 0.5]);
        assert_eq!(verts.len(), 12);
    }

    #[test]
    fn plane_outline_corners_stay_within_half_extent_on_plane() {
        let half_extent = 120.0;
        let plane = SketchPlane::right();
        let verts = plane_outline(&plane, half_extent, [1.0, 1.0, 1.0, 1.0]);
        for v in &verts {
            let p = Vec3::from(v.position);
            let diff = p - plane.origin;
            let u = diff.dot(plane.u_axis);
            let val = diff.dot(plane.v_axis);
            assert!(u.abs() <= half_extent + 1e-4, "u di luar batas: {u}");
            assert!(val.abs() <= half_extent + 1e-4, "v di luar batas: {val}");
        }
    }

    #[test]
    fn plane_outline_uses_given_color() {
        let color = [0.10, 0.55, 0.95, 0.30];
        let verts = plane_outline(&SketchPlane::top(), 120.0, color);
        assert!(verts.iter().all(|v| v.color == color));
    }
}
