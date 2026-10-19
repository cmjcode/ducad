//! Bagian backend truck dari `crate::picking::vertex` (PLAN_MIGRASI_TRUCK
//! 2.6): vertex = ujung edge logis (titik sambung potongan busur yang
//! digabung tidak ikut, sama seperti backend lama (≤ 0.5.x) yang hanya punya satu vertex
//! per lingkaran).

use glam::DVec3;

use crate::picking::ray::{point_to_ray_distance, PickRay};
use crate::shape::KernelShape;

/// Cast `ray` ke `shape`, kembalikan vertex terdekat dalam `tolerance` mm.
pub fn pick_vertex(shape: &KernelShape, ray: PickRay, tolerance: f64) -> Option<(f64, f64, f64)> {
    let origin = ray.origin_vec();
    let dir = ray.dir_vec();
    let dir_len_sq = dir.length_squared();
    if dir_len_sq < 1e-18 {
        return None;
    }
    let mut best: Option<(f64, f64, DVec3)> = None; // (t, dist, vertex)
    for &v in &shape.topology().vertices {
        let t = dir.dot(v - origin) / dir_len_sq;
        if t < 0.0 {
            continue;
        }
        let dist = point_to_ray_distance(origin, dir, v);
        if dist > tolerance {
            continue;
        }
        let is_better = match &best {
            None => true,
            Some((best_t, best_dist, _)) => {
                if dist < *best_dist * 0.4 && *best_dist > 1.0 {
                    true
                } else if *best_dist < dist * 0.4 && dist > 1.0 {
                    false
                } else {
                    const DEPTH_EPS: f64 = 2.0;
                    if t < best_t - DEPTH_EPS {
                        true
                    } else if t > best_t + DEPTH_EPS {
                        false
                    } else {
                        dist < *best_dist
                    }
                }
            }
        };
        if is_better {
            best = Some((t, dist, v));
        }
    }
    best.map(|(_, _, v)| (v.x, v.y, v.z))
}

/// Semua vertex unik dari `shape`.
pub fn shape_vertices(shape: &KernelShape) -> Vec<(f64, f64, f64)> {
    shape
        .topology()
        .vertices
        .iter()
        .map(|v| (v.x, v.y, v.z))
        .collect()
}

/// Normal keluar di sudut: rata-rata normal semua face yang bertemu di
/// vertex ini.
pub fn vertex_outward_normal(shape: &KernelShape, vertex_pos: (f64, f64, f64)) -> (f64, f64, f64) {
    const TOUCH_EPS: f64 = 1e-3;
    let topo = shape.topology();
    let v = DVec3::new(vertex_pos.0, vertex_pos.1, vertex_pos.2);
    let mut sum = DVec3::ZERO;
    for (f, face) in topo.faces.iter().enumerate() {
        let touches = face
            .edges
            .iter()
            .filter_map(|&e| topo.edges.get(e))
            .any(|e| (e.start() - v).length() < TOUCH_EPS || (e.end() - v).length() < TOUCH_EPS);
        if touches {
            if let Some(n) = topo.normal_near(f, v) {
                sum += n;
            }
        }
    }
    let dir = sum.normalize_or_zero();
    if dir != DVec3::ZERO {
        (dir.x, dir.y, dir.z)
    } else {
        (0.0, 0.0, 1.0)
    }
}
