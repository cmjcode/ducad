use glam::DVec3;
use opencascade::primitives::{Edge, Shape};

use crate::lock_kernel;
use crate::picking::ray::{closest_point_ray_segment, PickRay};
use crate::shape::KernelShape;

/// (titik hit terdekat di edge, polyline approksimasi edge itu utk
/// highlight render) — hasil `pick_edge`.
pub type EdgePickHit = ((f64, f64, f64), Vec<(f64, f64, f64)>);

/// (titik tengah dunia edge di arc-length setengah panjangnya, titik AWAL
/// dan AKHIR edge, panjang total edge) — satu entri per edge topologi
/// shape. `start`/`end` disertakan (bukan cuma titik tengah) supaya
/// pemanggil (ducad-app) bisa menghitung sudut layar rusuknya SETELAH
/// diproyeksikan kamera — label dimensi dengan begitu bisa disejajarkan ke
/// arah rusuknya sendiri, ikut berubah sudut saat kamera diputar, sama
/// seperti pill dimensi entitas sketsa 2D.
pub type EdgeDimension = ((f64, f64, f64), (f64, f64, f64), (f64, f64, f64), f64);

/// Edge terdekat ke `ray` (dalam `tolerance` mm), plus titik terdekatnya
/// dan polyline approksimasi (buat highlight render).
pub(crate) fn resolve_edge_along_ray(
    shape: &Shape,
    ray: PickRay,
    tolerance: f64,
) -> Option<(Edge, DVec3, Vec<DVec3>)> {
    let origin = ray.origin_vec();
    let dir = ray.dir_vec();
    let dir_len_sq = dir.length_squared();
    if dir_len_sq < 1e-18 {
        return None;
    }

    let mut best: Option<(f64, f64, Edge, DVec3, Vec<DVec3>)> = None; // (t, dist, edge, point, polyline)
    for edge in shape.edges() {
        let polyline: Vec<DVec3> = edge.approximation_segments().collect();
        if polyline.len() < 2 {
            continue;
        }
        let mut edge_best: Option<(f64, f64, DVec3)> = None; // (t, dist, point)
        for pair in polyline.windows(2) {
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
        if dist <= tolerance {
            let is_better = match &best {
                None => true,
                Some((best_t, best_dist, ..)) => {
                    // Jika kandidat jauh lebih presisi (mis. tepat di ray) dibanding foreground yang glancing
                    if dist < *best_dist * 0.4 && *best_dist > 1.0 {
                        true
                    } else if *best_dist < dist * 0.4 && dist > 1.0 {
                        false
                    } else {
                        const DEPTH_EPS: f64 = 2.0;
                        if t < best_t - DEPTH_EPS {
                            true // Lebih dekat ke kamera (foreground prioritas)
                        } else if t > best_t + DEPTH_EPS {
                            false // Di belakang edge foreground
                        } else {
                            dist < *best_dist // Kedalaman sama, pilih yang paling presisi ke ray
                        }
                    }
                }
            };
            if is_better {
                best = Some((t, dist, edge, point, polyline));
            }
        }
    }
    best.map(|(_, _, edge, point, polyline)| (edge, point, polyline))
}

/// Cast `ray` ke `shape`, kembalikan (titik hit terdekat di edge, polyline
/// approksimasi edge itu utk highlight) kalau ada edge dalam `tolerance`
/// mm dari ray. Dipakai UI utk edge-picking interaktif (Fillet/Chamfer
/// per-tepi).
pub fn pick_edge(shape: &KernelShape, ray: PickRay, tolerance: f64) -> Option<EdgePickHit> {
    let _guard = lock_kernel();
    resolve_edge_along_ray(shape.inner(), ray, tolerance).map(|(_, point, polyline)| {
        (
            (point.x, point.y, point.z),
            polyline.into_iter().map(|p| (p.x, p.y, p.z)).collect(),
        )
    })
}

/// Hitung vektor normal keluar (outward radial normal) dari rusuk pada shape.
/// Mengambil rata-rata normal keluar dari face-face yang bertemu di rusuk ini,
/// lalu memproyeksikannya tegak lurus terhadap garis singgung rusuk (edge tangent).
/// Bekerja konsisten untuk sudut luar (convex) maupun sudut dalam (concave).
pub fn edge_outward_normal(
    shape: &KernelShape,
    ray: PickRay,
    tolerance: f64,
) -> Option<((f64, f64, f64), (f64, f64, f64))> {
    let _guard = lock_kernel();
    let (edge, point, _) = resolve_edge_along_ray(shape.inner(), ray, tolerance)?;
    let edge_start = edge.start_point();
    let edge_end = edge.end_point();
    let edge_tan = (edge_end - edge_start).normalize_or_zero();

    let mut normal_sum = DVec3::ZERO;
    const TOUCH_EPS: f64 = 1e-3;

    for face in shape.inner().faces() {
        let touches = face.edges().any(|e| {
            let s = e.start_point();
            let end = e.end_point();
            ((s - edge_start).length() < TOUCH_EPS && (end - edge_end).length() < TOUCH_EPS)
                || ((s - edge_end).length() < TOUCH_EPS && (end - edge_start).length() < TOUCH_EPS)
        });
        if touches {
            let n = face.normal_at(point);
            if n.length_squared() > 1e-6 {
                normal_sum += n.normalize_or_zero();
            }
        }
    }

    let radial = if edge_tan != DVec3::ZERO && normal_sum != DVec3::ZERO {
        let r = normal_sum - edge_tan * normal_sum.dot(edge_tan);
        r.normalize_or_zero()
    } else {
        normal_sum.normalize_or_zero()
    };

    let dir = if radial != DVec3::ZERO {
        radial
    } else {
        DVec3::Z
    };

    Some(((point.x, point.y, point.z), (dir.x, dir.y, dir.z)))
}

/// Panjang + titik tengah SEMUA edge shape, dipakai fitur "Tampilkan
/// Semua Ukuran" (checkbox ruler properties, ducad-app) untuk melabeli
/// tiap rusuk 3D tanpa perlu ray picking satu-satu.
pub fn edge_dimensions(shape: &KernelShape) -> Vec<EdgeDimension> {
    let _guard = lock_kernel();
    let mut seen_pairs = std::collections::HashSet::new();
    let mut out = Vec::new();

    let quantize = |p: DVec3| -> (i64, i64, i64) {
        (
            (p.x * 1000.0).round() as i64,
            (p.y * 1000.0).round() as i64,
            (p.z * 1000.0).round() as i64,
        )
    };

    const MAX_EDGES_TO_EVALUATE: usize = 2000;

    for (idx, edge) in shape.inner().edges().enumerate() {
        if idx >= MAX_EDGES_TO_EVALUATE {
            break;
        }
        let start = edge.start_point();
        let end = edge.end_point();
        let q_start = quantize(start);
        let q_end = quantize(end);
        let key = if q_start <= q_end {
            (q_start, q_end)
        } else {
            (q_end, q_start)
        };

        if !seen_pairs.insert(key) {
            continue;
        }

        let polyline: Vec<DVec3> = edge.approximation_segments().collect();
        if polyline.len() < 2 {
            continue;
        }
        let seg_lens: Vec<f64> = polyline.windows(2).map(|w| (w[1] - w[0]).length()).collect();
        let length: f64 = seg_lens.iter().sum();
        if length <= 1e-9 {
            continue;
        }
        let half = length * 0.5;
        let mut acc = 0.0;
        let mut mid = polyline[0];
        for (seg_len, w) in seg_lens.iter().zip(polyline.windows(2)) {
            if acc + seg_len >= half {
                let t = if *seg_len > 1e-9 {
                    (half - acc) / seg_len
                } else {
                    0.0
                };
                mid = w[0] + (w[1] - w[0]) * t;
                break;
            }
            acc += seg_len;
            mid = w[1];
        }
        out.push((
            (mid.x, mid.y, mid.z),
            (start.x, start.y, start.z),
            (end.x, end.y, end.z),
            length,
        ));
    }
    out
}
