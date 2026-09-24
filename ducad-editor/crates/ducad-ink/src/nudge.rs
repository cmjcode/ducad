//! Deformasi lokal titik tinta (Nudge ala Concepts) dengan falloff smoothstep.

use glam::Vec2;

use crate::stroke::InkPoint;

/// Melakukan deformasi lokal pada sekumpulan titik coretan di dalam `radius` dari `center`.
///
/// Titik di luar `radius` tidak berubah sama sekali (identitas).
/// Titik tepat di `center` bergeser sebesar `delta`.
/// Titik di antara 0 dan `radius` bergeser secara halus dengan falloff `smoothstep(1 - r / R)`.
pub fn nudge(points: &[InkPoint], center: Vec2, radius: f32, delta: Vec2) -> Vec<InkPoint> {
    if radius <= 0.0 || delta == Vec2::ZERO {
        return points.to_vec();
    }

    points
        .iter()
        .map(|pt| {
            let dist = (pt.pos() - center).length();
            if dist >= radius {
                *pt
            } else {
                let t = 1.0 - (dist / radius);
                // Hermite smoothstep: 3*t^2 - 2*t^3
                let factor = t * t * (3.0 - 2.0 * t);
                let new_pos = pt.pos() + delta * factor;
                InkPoint::new(new_pos.x, new_pos.y, pt.pressure, pt.tilt, pt.t_ms)
            }
        })
        .collect()
}
