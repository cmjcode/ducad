use bytemuck::{Pod, Zeroable};

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

/// Grid bidang XY (Z-up): garis minor tiap `step`, mayor tiap 10×step,
/// plus sumbu X/Y penuh dan stub sumbu Z. Cukup untuk Fase 0; nanti
/// diganti grid shader tak-hingga dengan fade jarak.
pub fn generate_grid(half_extent: f32, step: f32) -> Vec<LineVertex> {
    let mut verts = Vec::new();
    let n = (half_extent / step).round() as i32;
    let mut push_line = |a: [f32; 3], b: [f32; 3], color: [f32; 4]| {
        verts.push(LineVertex { position: a, color });
        verts.push(LineVertex { position: b, color });
    };

    for i in -n..=n {
        let d = i as f32 * step;
        if i == 0 {
            continue; // sumbu digambar terpisah dengan warna sendiri
        }
        let color = if i % 10 == 0 { MAJOR } else { MINOR };
        push_line([d, -half_extent, 0.0], [d, half_extent, 0.0], color);
        push_line([-half_extent, d, 0.0], [half_extent, d, 0.0], color);
    }

    push_line([-half_extent, 0.0, 0.0], [half_extent, 0.0, 0.0], AXIS_X);
    push_line([0.0, -half_extent, 0.0], [0.0, half_extent, 0.0], AXIS_Y);
    push_line([0.0, 0.0, 0.0], [0.0, 0.0, half_extent * 0.25], AXIS_Z);

    verts
}
