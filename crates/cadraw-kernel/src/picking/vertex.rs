use glam::DVec3;
use opencascade::primitives::Shape;

use crate::lock_kernel;
use crate::picking::ray::{point_to_ray_distance, PickRay};
use crate::shape::KernelShape;

/// Vertex (sudut/endpoint edge) terdekat ke `ray` (dalam `tolerance` mm).
pub(crate) fn resolve_vertex_along_ray(
    shape: &Shape,
    ray: PickRay,
    tolerance: f64,
) -> Option<DVec3> {
    let origin = ray.origin_vec();
    let dir = ray.dir_vec();
    if dir.length_squared() < 1e-18 {
        return None;
    }

    let mut best: Option<(f64, DVec3)> = None;
    for v in collect_vertices(shape) {
        let dist = point_to_ray_distance(origin, dir, v);
        if dist <= tolerance && best.as_ref().is_none_or(|(d, _)| dist < *d) {
            best = Some((dist, v));
        }
    }
    best.map(|(_, v)| v)
}

/// Semua vertex (sudut) unik pada `shape` — endpoint SEMUA edge, di-dedup
/// lewat jarak epsilon.
pub(crate) fn collect_vertices(shape: &Shape) -> Vec<DVec3> {
    const DEDUP_EPS: f64 = 1e-6;
    let mut vertices: Vec<DVec3> = Vec::new();
    for edge in shape.edges() {
        for p in [edge.start_point(), edge.end_point()] {
            if !vertices.iter().any(|v| (*v - p).length() < DEDUP_EPS) {
                vertices.push(p);
            }
        }
    }
    vertices
}

/// Cast `ray` ke `shape`, kembalikan titik vertex (sudut) terdekat kalau
/// ada dalam `tolerance` mm dari ray.
pub fn pick_vertex(shape: &KernelShape, ray: PickRay, tolerance: f64) -> Option<(f64, f64, f64)> {
    let _guard = lock_kernel();
    resolve_vertex_along_ray(shape.inner(), ray, tolerance).map(|p| (p.x, p.y, p.z))
}

/// Semua vertex (sudut) unik dari `shape`, dedup sama seperti dipakai
/// `pick_vertex`/`fillet_vertex`.
pub fn shape_vertices(shape: &KernelShape) -> Vec<(f64, f64, f64)> {
    let _guard = lock_kernel();
    collect_vertices(shape.inner())
        .into_iter()
        .map(|v| (v.x, v.y, v.z))
        .collect()
}
