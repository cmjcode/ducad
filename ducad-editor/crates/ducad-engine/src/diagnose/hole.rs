//! Lubang: titik di luar face (memblokir) dan lubang buta terlalu dalam
//! (peringatan).

use ducad_kernel::FaceInfo;
use glam::{DVec2, DVec3};
use serde_json::json;

use crate::error::{OpError, OpErrorCode};
use crate::model::BodyGeometry;
use crate::plane::PlaneFrame;

/// Pre-check memblokir: setiap posisi (dunia, sudah di bidang face) harus
/// berada di dalam poligon batas luar face.
pub(crate) fn precheck_hole(
    face: &FaceInfo,
    frame: &PlaneFrame,
    positions: &[[f64; 3]],
) -> Result<(), OpError> {
    if face.boundary.len() < 3 {
        return Ok(());
    }
    let poly: Vec<(f64, f64)> = face
        .boundary
        .iter()
        .map(|p| {
            let l = frame.to_local(DVec3::from_array(*p));
            (l.x, l.y)
        })
        .collect();
    let (mut lo, mut hi) = (DVec2::splat(f64::MAX), DVec2::splat(f64::MIN));
    for (x, y) in &poly {
        lo = lo.min(DVec2::new(*x, *y));
        hi = hi.max(DVec2::new(*x, *y));
    }
    for (i, p) in positions.iter().enumerate() {
        let l = frame.to_local(DVec3::from_array(*p));
        if !ducad_kernel::point_in_polygon_2d((l.x, l.y), &poly) {
            return Err(OpError::new(
                OpErrorCode::HoleOutsideFace,
                format!(
                    "posisi lubang #{i} ({:.3}, {:.3}) berada di luar face (batas lokal u {:.3}..{:.3}, v {:.3}..{:.3})",
                    l.x, l.y, lo.x, hi.x, lo.y, hi.y
                ),
            )
            .with_hint("pindahkan titik ke dalam face; koordinat 'at' relatif terhadap pusat face")
            .with_context(json!({
                "index": i,
                "point_local": [l.x, l.y],
                "point_world": p,
                "face_bbox_local": [[lo.x, lo.y], [hi.x, hi.y]],
            })));
        }
    }
    Ok(())
}

/// Peringatan (bukan error): kedalaman lubang buta melebihi tebal body di
/// titik masuk, diukur dengan sinar ke arah −normal.
pub(crate) fn hole_depth_warning(
    geo: &BodyGeometry,
    positions: &[[f64; 3]],
    normal: [f64; 3],
    depth: f64,
) -> Option<String> {
    let n = DVec3::from_array(normal).normalize_or_zero();
    for (i, p) in positions.iter().enumerate() {
        let origin = DVec3::from_array(*p) - n * 1e-3;
        let dir = -n;
        let t = ducad_kernel::ray_hit_distance(
            &geo.mesh,
            origin.as_vec3().to_array(),
            dir.as_vec3().to_array(),
        )?;
        let thickness = t as f64 + 1e-3;
        if depth > thickness + 1e-6 {
            return Some(format!(
                "hole_deeper_than_body: lubang #{i} sedalam {depth} mm melebihi tebal body {thickness:.3} mm (menjadi tembus)"
            ));
        }
    }
    None
}
