//! Deteksi Closed Loop & Planar Region untuk Sketch 2D (Shapr3D Style).
//!
//! Mengidentifikasi profil tertutup (lingkaran, elips, persegi, poligon, loop garis+arc),
//! menghitung titik pusat (centroid), uji titik di dalam region (point-in-region),
//! dan triangulasi 2D untuk rendering highlight biru/cyan aktif.

use std::collections::HashSet;
use glam::DVec2;
use crate::{Entity, EntityId, Sketch};

/// Satu profil/region tertutup pada bidang sketch 2D.
#[derive(Debug, Clone)]
pub struct ClosedRegion {
    /// Seluruh EntityId pembentuk region ini.
    pub entity_ids: HashSet<EntityId>,
    /// Titik-titik poligon pembatas luar (CCW atau CW) yang sudah di-sample.
    pub boundary_points: Vec<DVec2>,
    /// Titik pusat / centroid region (mm).
    pub centroid: DVec2,
    /// Luas area region (mm²).
    pub area: f64,
}

impl ClosedRegion {
    /// Uji apakah sebuah titik `p` berada di dalam region ini.
    pub fn contains_point(&self, p: DVec2) -> bool {
        if self.boundary_points.len() < 3 {
            return false;
        }

        // Ray casting algorithm (even-odd rule)
        let mut inside = false;
        let n = self.boundary_points.len();
        let mut j = n - 1;
        for i in 0..n {
            let pi = self.boundary_points[i];
            let pj = self.boundary_points[j];
            if ((pi.y > p.y) != (pj.y > p.y))
                && (p.x < (pj.x - pi.x) * (p.y - pi.y) / (pj.y - pi.y + 1e-12) + pi.x)
            {
                inside = !inside;
            }
            j = i;
        }
        inside
    }

    /// Triangulasi 2D untuk rendering face (mengembalikan segitiga-segitiga berupa list vertex 2D).
    pub fn triangulate(&self) -> Vec<DVec2> {
        if self.boundary_points.len() < 3 {
            return Vec::new();
        }
        if self.boundary_points.len() == 3 {
            return self.boundary_points.clone();
        }

        // Jika berbentuk poligon convex sederhana (atau lingkaran/elips hasil sampling)
        // ear clipping algorithm untuk poligon 2D umum.
        ear_clip_triangulate(&self.boundary_points)
    }
}

/// Hitung centroid dan luas dari deretan titik poligon 2D.
fn polygon_centroid_and_area(pts: &[DVec2]) -> (DVec2, f64) {
    let n = pts.len();
    if n < 3 {
        let c = if n == 0 { DVec2::ZERO } else { pts.iter().copied().sum::<DVec2>() / (n as f64) };
        return (c, 0.0);
    }

    let mut signed_area = 0.0;
    let mut cx = 0.0;
    let mut cy = 0.0;

    for i in 0..n {
        let p0 = pts[i];
        let p1 = pts[(i + 1) % n];
        let cross = p0.x * p1.y - p1.x * p0.y;
        signed_area += cross;
        cx += (p0.x + p1.x) * cross;
        cy += (p0.y + p1.y) * cross;
    }

    signed_area *= 0.5;
    let area = signed_area.abs();

    if signed_area.abs() < 1e-9 {
        let c = pts.iter().copied().sum::<DVec2>() / (n as f64);
        return (c, 0.0);
    }

    let factor = 1.0 / (6.0 * signed_area);
    (DVec2::new(cx * factor, cy * factor), area)
}

/// Triangulasi poligon sederhana menggunakan metode Ear Clipping.
fn ear_clip_triangulate(poly: &[DVec2]) -> Vec<DVec2> {
    let mut vertices: Vec<(usize, DVec2)> = poly.iter().copied().enumerate().collect();
    let mut triangles = Vec::new();

    // Pastikan orientasi CCW
    let (_, signed_area) = {
        let mut a = 0.0;
        for i in 0..poly.len() {
            let p0 = poly[i];
            let p1 = poly[(i + 1) % poly.len()];
            a += p0.x * p1.y - p1.x * p0.y;
        }
        (a.abs() * 0.5, a * 0.5)
    };

    if signed_area < 0.0 {
        vertices.reverse();
    }

    let mut count = 0;
    let max_iterations = vertices.len() * vertices.len() * 2;

    while vertices.len() > 3 && count < max_iterations {
        count += 1;
        let mut ear_found = false;
        let n = vertices.len();

        for i in 0..n {
            let prev = if i == 0 { n - 1 } else { i - 1 };
            let next = (i + 1) % n;

            let a = vertices[prev].1;
            let b = vertices[i].1;
            let c = vertices[next].1;

            // Cek apakah sudut convex (cross product > 0)
            let cross = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
            if cross <= 1e-9 {
                continue;
            }

            // Cek apakah ada vertex lain di dalam segitiga a-b-c
            let mut has_point_inside = false;
            for j in 0..n {
                if j == prev || j == i || j == next {
                    continue;
                }
                let p = vertices[j].1;
                if is_point_in_triangle(p, a, b, c) {
                    has_point_inside = true;
                    break;
                }
            }

            if !has_point_inside {
                triangles.push(a);
                triangles.push(b);
                triangles.push(c);
                vertices.remove(i);
                ear_found = true;
                break;
            }
        }

        if !ear_found {
            // Fallback jika ada self-intersection/degeneracy: triangle fan
            break;
        }
    }

    if vertices.len() == 3 {
        triangles.push(vertices[0].1);
        triangles.push(vertices[1].1);
        triangles.push(vertices[2].1);
    } else if triangles.is_empty() && poly.len() >= 3 {
        // Fallback fan from center
        let c = poly.iter().copied().sum::<DVec2>() / (poly.len() as f64);
        for i in 0..poly.len() {
            let p0 = poly[i];
            let p1 = poly[(i + 1) % poly.len()];
            triangles.push(c);
            triangles.push(p0);
            triangles.push(p1);
        }
    }

    triangles
}

fn is_point_in_triangle(p: DVec2, a: DVec2, b: DVec2, c: DVec2) -> bool {
    let cross1 = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
    let cross2 = (c.x - b.x) * (p.y - b.y) - (c.y - b.y) * (p.x - b.x);
    let cross3 = (a.x - c.x) * (p.y - c.y) - (a.y - c.y) * (p.x - c.x);

    let has_neg = (cross1 < -1e-9) || (cross2 < -1e-9) || (cross3 < -1e-9);
    let has_pos = (cross1 > 1e-9) || (cross2 > 1e-9) || (cross3 > 1e-9);

    !(has_neg && has_pos)
}

/// Sample titik-titik kurva Arc (CCW)
fn sample_arc_points(center: DVec2, radius: f64, start_angle: f64, end_angle: f64, steps: usize) -> Vec<DVec2> {
    let tau = std::f64::consts::TAU;
    let span = {
        let s = end_angle - start_angle;
        if s <= 0.0 { s + tau } else { s }
    };
    let step_count = steps.max(4);
    let mut pts = Vec::with_capacity(step_count + 1);
    for i in 0..=step_count {
        let t = start_angle + span * (i as f64 / step_count as f64);
        pts.push(center + DVec2::new(radius * t.cos(), radius * t.sin()));
    }
    pts
}

/// Cari seluruh closed region pada `Sketch`.
pub fn find_closed_regions(sketch: &Sketch) -> Vec<ClosedRegion> {
    let mut regions = Vec::new();
    const EPS: f64 = 1e-4;

    // 1. Lingkaran mandiri (Circle)
    for (id, entity) in sketch.entities.iter() {
        if let Entity::Circle { center, radius } = entity {
            if *radius > 1e-6 {
                const SAMPLES: usize = 48;
                let mut pts = Vec::with_capacity(SAMPLES);
                let tau = std::f64::consts::TAU;
                for i in 0..SAMPLES {
                    let t = tau * (i as f64 / SAMPLES as f64);
                    pts.push(*center + DVec2::new(radius * t.cos(), radius * t.sin()));
                }
                let mut ids = HashSet::new();
                ids.insert(id);
                regions.push(ClosedRegion {
                    entity_ids: ids,
                    boundary_points: pts,
                    centroid: *center,
                    area: std::f64::consts::PI * radius * radius,
                });
            }
        } else if let Entity::Ellipse { center, radius_x, radius_y } = entity {
            if *radius_x > 1e-6 && *radius_y > 1e-6 {
                const SAMPLES: usize = 48;
                let mut pts = Vec::with_capacity(SAMPLES);
                let tau = std::f64::consts::TAU;
                for i in 0..SAMPLES {
                    let t = tau * (i as f64 / SAMPLES as f64);
                    pts.push(*center + DVec2::new(radius_x * t.cos(), radius_y * t.sin()));
                }
                let mut ids = HashSet::new();
                ids.insert(id);
                regions.push(ClosedRegion {
                    entity_ids: ids,
                    boundary_points: pts,
                    centroid: *center,
                    area: std::f64::consts::PI * radius_x * radius_y,
                });
            }
        }
    }

    // 2. Loop dari rantai Line & Arc
    struct SegmentInfo {
        id: EntityId,
        start: DVec2,
        end: DVec2,
        sampled_pts: Vec<DVec2>, // Dari start ke end
    }

    let mut segments = Vec::new();
    for (id, entity) in sketch.entities.iter() {
        match entity {
            Entity::Line { start, end } => {
                if (*start - *end).length() > EPS {
                    segments.push(SegmentInfo {
                        id,
                        start: *start,
                        end: *end,
                        sampled_pts: vec![*start, *end],
                    });
                }
            }
            Entity::Arc { center, radius, start_angle, end_angle } => {
                let sampled = sample_arc_points(*center, *radius, *start_angle, *end_angle, 16);
                if let (Some(&s), Some(&e)) = (sampled.first(), sampled.last()) {
                    segments.push(SegmentInfo {
                        id,
                        start: s,
                        end: e,
                        sampled_pts: sampled,
                    });
                }
            }
            _ => {}
        }
    }

    if segments.len() >= 3 {
        // Cari simple cycles
        let mut used_in_region = HashSet::new();

        for start_idx in 0..segments.len() {
            if used_in_region.contains(&segments[start_idx].id) {
                continue;
            }

            let mut chain: Vec<(usize, bool)> = vec![(start_idx, false)]; // (segment_index, reversed)
            let mut visited = HashSet::new();
            visited.insert(start_idx);

            let mut current_tail = segments[start_idx].end;
            let target_head = segments[start_idx].start;

            let mut success = false;

            for _ in 0..segments.len() {
                if (current_tail - target_head).length() < EPS && chain.len() >= 3 {
                    success = true;
                    break;
                }

                // Cari sambungan berikutnya
                let mut found_next = false;
                for (next_idx, seg) in segments.iter().enumerate() {
                    if visited.contains(&next_idx) {
                        continue;
                    }
                    if (seg.start - current_tail).length() < EPS {
                        chain.push((next_idx, false));
                        visited.insert(next_idx);
                        current_tail = seg.end;
                        found_next = true;
                        break;
                    } else if (seg.end - current_tail).length() < EPS {
                        chain.push((next_idx, true));
                        visited.insert(next_idx);
                        current_tail = seg.start;
                        found_next = true;
                        break;
                    }
                }

                if !found_next {
                    break;
                }
            }

            if success {
                let mut entity_ids = HashSet::new();
                let mut boundary_pts = Vec::new();

                for (idx, rev) in &chain {
                    let seg = &segments[*idx];
                    entity_ids.insert(seg.id);
                    used_in_region.insert(seg.id);

                    if !*rev {
                        let pts = &seg.sampled_pts;
                        for p in &pts[..pts.len() - 1] {
                            boundary_pts.push(*p);
                        }
                    } else {
                        let mut pts = seg.sampled_pts.clone();
                        pts.reverse();
                        for p in &pts[..pts.len() - 1] {
                            boundary_pts.push(*p);
                        }
                    }
                }

                let (centroid, area) = polygon_centroid_and_area(&boundary_pts);
                if area > 1e-4 {
                    regions.push(ClosedRegion {
                        entity_ids,
                        boundary_points: boundary_pts,
                        centroid,
                        area,
                    });
                }
            }
        }
    }

    regions
}

/// Cari region yang mencakup titik `p`, jika ada.
pub fn find_region_at_point(sketch: &Sketch, p: DVec2) -> Option<ClosedRegion> {
    let regions = find_closed_regions(sketch);
    // Prioritaskan region dengan luas terkecil jika ada nesting
    regions
        .into_iter()
        .filter(|r| r.contains_point(p))
        .min_by(|a, b| a.area.partial_cmp(&b.area).unwrap_or(std::cmp::Ordering::Equal))
}

/// Cari region yang mengandung sebuah `EntityId`.
pub fn find_region_containing_entity(sketch: &Sketch, id: EntityId) -> Option<ClosedRegion> {
    let regions = find_closed_regions(sketch);
    regions.into_iter().find(|r| r.entity_ids.contains(&id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rectangle_region_detection() {
        let mut sketch = Sketch::default();
        let p0 = DVec2::new(0.0, 0.0);
        let p1 = DVec2::new(10.0, 0.0);
        let p2 = DVec2::new(10.0, 5.0);
        let p3 = DVec2::new(0.0, 5.0);

        sketch.entities.insert(Entity::Line { start: p0, end: p1 });
        sketch.entities.insert(Entity::Line { start: p1, end: p2 });
        sketch.entities.insert(Entity::Line { start: p2, end: p3 });
        sketch.entities.insert(Entity::Line { start: p3, end: p0 });

        let regions = find_closed_regions(&sketch);
        assert_eq!(regions.len(), 1);
        let reg = &regions[0];
        assert_eq!(reg.entity_ids.len(), 4);
        assert!((reg.area - 50.0).abs() < 1e-3);
        assert!((reg.centroid.x - 5.0).abs() < 1e-3);
        assert!((reg.centroid.y - 2.5).abs() < 1e-3);

        // Point inside
        assert!(reg.contains_point(DVec2::new(5.0, 2.5)));
        // Point outside
        assert!(!reg.contains_point(DVec2::new(15.0, 2.5)));
    }

    #[test]
    fn test_circle_region_detection() {
        let mut sketch = Sketch::default();
        sketch.entities.insert(Entity::Circle {
            center: DVec2::new(20.0, 20.0),
            radius: 10.0,
        });

        let regions = find_closed_regions(&sketch);
        assert_eq!(regions.len(), 1);
        let reg = &regions[0];
        assert!(reg.contains_point(DVec2::new(20.0, 20.0)));
        assert!(reg.contains_point(DVec2::new(25.0, 20.0)));
        assert!(!reg.contains_point(DVec2::new(35.0, 20.0)));
    }
}
