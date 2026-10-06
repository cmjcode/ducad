//! Geometri murni untuk "Sketsa di Face": menurunkan orientasi bidang sketsa
//! dari batas face dan memproyeksikan batas itu menjadi geometri konstruksi.
//!
//! Tanpa kernel dan tanpa GUI supaya bisa diuji dari poligon batas sederhana.

use ducad_render::SketchPlane;
use ducad_sketch::Entity;
use glam::{DVec2, DVec3, Vec3};

/// Toleransi kolinearitas dua segmen berurutan (sin sudut antar arah).
const COLLINEAR_SIN: f64 = 1e-3;
/// Panjang segmen minimum yang dianggap tepi (mm).
const MIN_EDGE_LEN: f64 = 1e-3;
/// Toleransi relatif jari-jari untuk mengenali loop sebagai lingkaran.
const CIRCLE_REL_TOL: f64 = 1e-3;

/// Gabungkan titik-titik polyline tertutup menjadi deretan ruas lurus:
/// titik berurutan yang segaris dilebur jadi satu ruas. Mengembalikan
/// pasangan `(awal, akhir)` tiap ruas; polyline dianggap tertutup.
fn straight_runs(points: &[DVec3]) -> Vec<(DVec3, DVec3)> {
    let n = points.len();
    if n < 2 {
        return Vec::new();
    }
    let mut runs: Vec<(DVec3, DVec3)> = Vec::new();
    for i in 0..n {
        let a = points[i];
        let b = points[(i + 1) % n];
        let seg = b - a;
        if seg.length() < MIN_EDGE_LEN {
            continue;
        }
        if let Some(last) = runs.last_mut() {
            let prev = last.1 - last.0;
            let sin = prev.normalize().cross(seg.normalize()).length();
            if sin < COLLINEAR_SIN && (last.1 - a).length() < MIN_EDGE_LEN {
                last.1 = b;
                continue;
            }
        }
        runs.push((a, b));
    }
    // Ruas terakhir dan pertama bisa segaris (loop tertutup).
    if runs.len() > 1 {
        let (fa, fb) = runs[0];
        let (la, lb) = runs[runs.len() - 1];
        let sin = (fb - fa).normalize().cross((lb - la).normalize()).length();
        if sin < COLLINEAR_SIN && (lb - fa).length() < MIN_EDGE_LEN {
            runs[0].0 = la;
            runs.pop();
        }
    }
    runs
}

/// Arah tepi lurus terpanjang dari batas face, dipakai sebagai petunjuk sumbu U
/// bidang sketsa. `None` bila batas tidak punya ruas lurus berarti (mis. lingkaran).
pub fn longest_straight_edge_dir(points: &[DVec3]) -> Option<Vec3> {
    let runs = straight_runs(points);
    // Tepi hasil aproksimasi busur berupa ruas-ruas pendek; ruas lurus asli
    // jauh lebih panjang. Tolak bila ruas terpanjang tidak dominan.
    let (best, best_len) = runs
        .iter()
        .map(|(a, b)| (*b - *a, (*b - *a).length()))
        .max_by(|x, y| x.1.partial_cmp(&y.1).unwrap_or(std::cmp::Ordering::Equal))?;
    let total: f64 = runs.iter().map(|(a, b)| (*b - *a).length()).sum();
    if best_len < MIN_EDGE_LEN || runs.len() > 4 && best_len < total * 0.1 {
        return None;
    }
    Some(best.as_vec3())
}

/// Proyeksikan batas face ke bidang sketsa sebagai geometri konstruksi:
/// lingkaran bila semua titik sejauh jari-jari yang sama dari pusatnya,
/// selain itu garis konstruksi per ruas lurus hasil [`straight_runs`].
pub fn project_boundary(plane: &SketchPlane, points: &[DVec3]) -> Vec<Entity> {
    if points.len() < 2 {
        return Vec::new();
    }
    let uv: Vec<DVec2> = points
        .iter()
        .map(|p| plane.project_point_to_uv(p.as_vec3()))
        .collect();

    if let Some(circle) = recognize_circle(&uv) {
        return vec![circle];
    }

    straight_runs(points)
        .into_iter()
        .map(|(a, b)| {
            Entity::line(
                plane.project_point_to_uv(a.as_vec3()),
                plane.project_point_to_uv(b.as_vec3()),
            )
            .with_construction(true)
        })
        .collect()
}

fn recognize_circle(uv: &[DVec2]) -> Option<Entity> {
    if uv.len() < 8 {
        return None;
    }
    let center = uv.iter().copied().sum::<DVec2>() / uv.len() as f64;
    let radii: Vec<f64> = uv.iter().map(|p| (*p - center).length()).collect();
    let r = radii.iter().sum::<f64>() / radii.len() as f64;
    if r < MIN_EDGE_LEN {
        return None;
    }
    let ok = radii.iter().all(|x| (x - r).abs() <= r * CIRCLE_REL_TOL);
    ok.then(|| Entity::circle(center, r).with_construction(true))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(w: f64, h: f64, z: f64) -> Vec<DVec3> {
        vec![
            DVec3::new(0.0, 0.0, z),
            DVec3::new(w, 0.0, z),
            DVec3::new(w, h, z),
            DVec3::new(0.0, h, z),
        ]
    }

    #[test]
    fn longest_edge_of_rectangle_is_its_long_side() {
        let dir = longest_straight_edge_dir(&rect(30.0, 10.0, 5.0)).expect("ada tepi");
        assert!(dir.normalize().x.abs() > 0.999, "{dir:?}");
    }

    #[test]
    fn collinear_subdivided_edge_is_merged() {
        // Sisi panjang dipecah jadi tiga titik segaris; sisi pendek 10 utuh.
        let pts = vec![
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(4.0, 0.0, 0.0),
            DVec3::new(8.0, 0.0, 0.0),
            DVec3::new(12.0, 0.0, 0.0),
            DVec3::new(12.0, 10.0, 0.0),
            DVec3::new(0.0, 10.0, 0.0),
        ];
        let dir = longest_straight_edge_dir(&pts).expect("ada tepi");
        assert!(dir.normalize().x.abs() > 0.999, "{dir:?}");
        let plane = SketchPlane::top();
        let ents = project_boundary(&plane, &pts);
        assert_eq!(ents.len(), 4, "{ents:?}");
        assert!(ents.iter().all(|e| e.is_construction()));
    }

    fn circle_pts(r: f64, z: f64, n: usize) -> Vec<DVec3> {
        (0..n)
            .map(|i| {
                let t = i as f64 / n as f64 * std::f64::consts::TAU;
                DVec3::new(3.0 + r * t.cos(), -2.0 + r * t.sin(), z)
            })
            .collect()
    }

    #[test]
    fn circular_face_has_no_edge_hint_and_projects_as_circle() {
        let pts = circle_pts(12.5, 7.0, 64);
        assert!(longest_straight_edge_dir(&pts).is_none());
        let plane = SketchPlane::from_origin_normal(Vec3::new(3.0, -2.0, 7.0), Vec3::Z);
        let ents = project_boundary(&plane, &pts);
        assert_eq!(ents.len(), 1);
        match &ents[0] {
            Entity::Circle {
                center,
                radius,
                is_construction,
            } => {
                assert!(center.length() < 1e-6, "{center:?}");
                assert!((radius - 12.5).abs() < 1e-6);
                assert!(is_construction);
            }
            other => panic!("bukan lingkaran: {other:?}"),
        }
    }

    #[test]
    fn projection_lands_on_face_plane_coordinates() {
        // Face atas balok 30×10 di Z=5 → di bidang face, sudutnya relatif centroid.
        let pts = rect(30.0, 10.0, 5.0);
        let plane = SketchPlane::from_origin_normal(Vec3::new(15.0, 5.0, 5.0), Vec3::Z);
        let ents = project_boundary(&plane, &pts);
        assert_eq!(ents.len(), 4);
        let max_abs = ents
            .iter()
            .flat_map(|e| match e {
                Entity::Line { start, end, .. } => vec![*start, *end],
                _ => vec![],
            })
            .map(|p| p.abs().max_element())
            .fold(0.0, f64::max);
        assert!((max_abs - 15.0).abs() < 1e-6, "{max_abs}");
    }
}
