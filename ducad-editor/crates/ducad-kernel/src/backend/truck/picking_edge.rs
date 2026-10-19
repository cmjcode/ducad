//! Bagian backend truck dari `crate::picking::edge` (PLAN_MIGRASI_TRUCK 2.6):
//! picking di atas polyline edge logis, dengan aturan pemilihan kandidat
//! (prioritas depan, presisi ke ray) yang sama dengan backend lama (≤ 0.5.x).

use glam::DVec3;

use super::topology::Topology;
use crate::picking::edge::{EdgeDimension, EdgeNormalHit, EdgePickHit};
use crate::picking::ray::{closest_point_ray_segment, PickRay};
use crate::shape::KernelShape;

/// Edge logis terdekat ke `ray` (dalam `tolerance` mm) dan titik terdekatnya.
pub(crate) fn resolve_edge(
    topo: &Topology,
    ray: PickRay,
    tolerance: f64,
) -> Option<(usize, DVec3)> {
    let origin = ray.origin_vec();
    let dir = ray.dir_vec();
    let dir_len_sq = dir.length_squared();
    if dir_len_sq < 1e-18 {
        return None;
    }
    let mut best: Option<(f64, f64, usize, DVec3)> = None; // (t, dist, edge, point)
    for (i, edge) in topo.edges.iter().enumerate() {
        if edge.points.len() < 2 {
            continue;
        }
        let mut edge_best: Option<(f64, f64, DVec3)> = None;
        for pair in edge.points.windows(2) {
            let (dist, point) = closest_point_ray_segment(origin, dir, pair[0], pair[1]);
            let t = dir.dot(point - origin) / dir_len_sq;
            if t < 0.0 {
                continue;
            }
            if edge_best.as_ref().is_none_or(|(_, d, _)| dist < *d) {
                edge_best = Some((t, dist, point));
            }
        }
        let Some((t, dist, point)) = edge_best else {
            continue;
        };
        if dist > tolerance {
            continue;
        }
        let is_better = match &best {
            None => true,
            Some((best_t, best_dist, ..)) => {
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
            best = Some((t, dist, i, point));
        }
    }
    best.map(|(_, _, i, p)| (i, p))
}

/// Cast `ray` ke `shape`: titik hit terdekat di edge + polyline edge itu.
pub fn pick_edge(shape: &KernelShape, ray: PickRay, tolerance: f64) -> Option<EdgePickHit> {
    let topo = shape.topology();
    let (i, point) = resolve_edge(topo, ray, tolerance)?;
    let edge = topo.edges.get(i)?;
    Some((
        (point.x, point.y, point.z),
        edge.points.iter().map(|p| (p.x, p.y, p.z)).collect(),
    ))
}

/// Titik hit di edge + arah keluar (rata-rata normal face tetangga, tanpa
/// komponen sepanjang edge).
pub fn edge_outward_normal(
    shape: &KernelShape,
    ray: PickRay,
    tolerance: f64,
) -> Option<EdgeNormalHit> {
    let topo = shape.topology();
    let (i, point) = resolve_edge(topo, ray, tolerance)?;
    let edge = topo.edges.get(i)?;
    let tan = (edge.end() - edge.start()).normalize_or_zero();
    let mut sum = DVec3::ZERO;
    for &f in &edge.faces {
        if let Some(n) = topo.normal_near(f, point) {
            sum += n;
        }
    }
    let radial = if tan != DVec3::ZERO && sum != DVec3::ZERO {
        (sum - tan * sum.dot(tan)).normalize_or_zero()
    } else {
        sum.normalize_or_zero()
    };
    let dir = if radial != DVec3::ZERO {
        radial
    } else {
        DVec3::Z
    };
    Some(((point.x, point.y, point.z), (dir.x, dir.y, dir.z)))
}

/// Panjang + titik tengah semua edge (fitur "Tampilkan Semua Ukuran").
/// Edge berujung sama dengan edge sebelumnya dilewati, seperti di backend lama (≤ 0.5.x).
pub fn edge_dimensions(shape: &KernelShape) -> Vec<EdgeDimension> {
    const MAX_EDGES_TO_EVALUATE: usize = 2000;
    let quantize = |p: DVec3| -> (i64, i64, i64) {
        (
            (p.x * 1000.0).round() as i64,
            (p.y * 1000.0).round() as i64,
            (p.z * 1000.0).round() as i64,
        )
    };
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for edge in shape.topology().edges.iter().take(MAX_EDGES_TO_EVALUATE) {
        let (start, end) = (edge.start(), edge.end());
        let (qs, qe) = (quantize(start), quantize(end));
        if !seen.insert(if qs <= qe { (qs, qe) } else { (qe, qs) }) {
            continue;
        }
        let length = edge.length();
        if edge.points.len() < 2 || length <= 1e-9 {
            continue;
        }
        let mid = edge.mid();
        out.push((
            (mid.x, mid.y, mid.z),
            (start.x, start.y, start.z),
            (end.x, end.y, end.z),
            length,
        ));
    }
    out
}
