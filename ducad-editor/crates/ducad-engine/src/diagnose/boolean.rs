//! Boolean: bbox kedua body tidak beririsan.

use serde_json::json;

use crate::error::{OpError, OpErrorCode};
use crate::model::BodyGeometry;

/// `Ok(None)` = beririsan; `Ok(Some(warning))` = union tanpa irisan
/// (peringatan); `Err` = subtract/intersect tanpa irisan (memblokir).
pub(crate) fn precheck_boolean(
    a_name: &str,
    b_name: &str,
    a: &BodyGeometry,
    b: &BodyGeometry,
    blocking: bool,
) -> Result<Option<String>, OpError> {
    let (Some((a0, a1)), Some((b0, b1))) = (a.mesh.bounding_box(), b.mesh.bounding_box()) else {
        return Ok(None);
    };
    let mut gap2 = 0.0f64;
    let mut overlap = true;
    for i in 0..3 {
        let d = (b0[i] - a1[i]).max(a0[i] - b1[i]) as f64;
        if d > 1e-6 {
            overlap = false;
            gap2 += d * d;
        }
    }
    if overlap {
        return Ok(None);
    }
    let gap = gap2.sqrt();
    if !blocking {
        return Ok(Some(format!(
            "boolean_no_overlap: body '{a_name}' dan '{b_name}' tidak beririsan (jarak {gap:.3} mm); hasil union berupa dua solid terpisah"
        )));
    }
    Err(OpError::new(
        OpErrorCode::BooleanNoOverlap,
        format!("body '{a_name}' dan '{b_name}' tidak beririsan (jarak bbox {gap:.3} mm)"),
    )
    .with_hint("geser body agar saling menembus, atau periksa urutan a/b")
    .with_context(json!({
        "bbox_a": [a0, a1],
        "bbox_b": [b0, b1],
        "separation": gap,
    })))
}
