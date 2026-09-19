//! Penghitung lubang (P7.4) dari `FaceInfo`.

use ducad_kernel::{FaceInfo, SurfaceKind};
use glam::DVec3;

use crate::select::ANG_TOL_DEG;

/// Satu lubang: titik dan arah sumbunya.
#[derive(Debug, Clone, Copy)]
pub struct Hole {
    pub point: [f64; 3],
    pub dir: [f64; 3],
    pub diameter: f64,
}

/// Lubang berdiameter `diameter ± tol`: face `Cylinder` cekung, dikelompokkan
/// menurut sumbu yang berimpit (satu lubang sering terbelah menjadi dua face
/// setengah silinder).
pub fn find_holes(faces: &[FaceInfo], diameter: f64, tol: f64) -> Vec<Hole> {
    let cos_tol = ANG_TOL_DEG.to_radians().cos();
    let mut holes: Vec<Hole> = Vec::new();
    for f in faces {
        let (SurfaceKind::Cylinder, Some(true), Some(r), Some((p, d))) =
            (f.kind, f.concave, f.radius, f.axis)
        else {
            continue;
        };
        if (2.0 * r - diameter).abs() > tol {
            continue;
        }
        let (p, d) = (
            DVec3::from_array(p),
            DVec3::from_array(d).normalize_or_zero(),
        );
        let same_axis = |h: &Hole| {
            let (hp, hd) = (DVec3::from_array(h.point), DVec3::from_array(h.dir));
            hd.dot(d).abs() >= cos_tol && (p - hp).cross(hd).length() <= 1e-3
        };
        if !holes.iter().any(same_axis) {
            holes.push(Hole {
                point: p.to_array(),
                dir: d.to_array(),
                diameter: 2.0 * r,
            });
        }
    }
    holes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compute::{self, PrimitiveShape};

    #[test]
    fn convex_cylinder_is_not_a_hole_and_through_hole_is() {
        let pin =
            compute::primitive(&PrimitiveShape::Cylinder { r: 2.75, h: 10.0 }, [0.0; 3]).unwrap();
        assert!(find_holes(&ducad_kernel::enumerate_faces(&pin.shape), 5.5, 0.02).is_empty());

        let blk = compute::primitive(
            &PrimitiveShape::Box {
                size: [20.0, 20.0, 5.0],
                centered: false,
            },
            [0.0; 3],
        )
        .unwrap();
        let mut spec = ducad_core::hole::HoleSpec::for_iso(
            ducad_core::hole::IsoMetricThread::M5,
            ducad_core::hole::HoleKind::Simple,
            5.0,
        );
        spec.is_through = true;
        let holed = compute::hole(
            &blk.shape,
            &spec,
            &[[10.0, 10.0, 5.0], [4.0, 4.0, 5.0]],
            [0.0, 0.0, 1.0],
        )
        .unwrap();
        let faces = ducad_kernel::enumerate_faces(&holed.shape);
        assert_eq!(find_holes(&faces, 5.5, 0.02).len(), 2);
        assert!(find_holes(&faces, 6.6, 0.02).is_empty());
    }
}
