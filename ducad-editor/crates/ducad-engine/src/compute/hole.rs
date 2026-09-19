//! Lubang (Hole Wizard) headless.

use ducad_core::hole::HoleSpec;
use ducad_kernel::KernelShape;

use super::{finish, require_positive};
use crate::error::{OpError, OpResult};
use crate::model::BodyGeometry;

/// Terapkan `apply_hole` berurutan di setiap posisi dengan normal keluar
/// face yang sama.
pub fn hole(
    shape: &KernelShape,
    spec: &HoleSpec,
    positions: &[[f64; 3]],
    normal: [f64; 3],
) -> OpResult<BodyGeometry> {
    if positions.is_empty() {
        return Err(OpError::invalid("Posisi lubang tidak boleh kosong"));
    }
    require_positive("diameter lubang", spec.diameter)?;
    if !spec.is_through {
        require_positive("kedalaman lubang", spec.depth)?;
    }
    let n_len = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    if n_len.is_nan() || n_len < 1e-9 {
        return Err(OpError::invalid("Normal lubang tidak boleh vektor nol"));
    }
    let n = (normal[0], normal[1], normal[2]);
    let mut current: Option<KernelShape> = None;
    for p in positions {
        let base = current.as_ref().unwrap_or(shape);
        let next = ducad_kernel::apply_hole(base, spec, (p[0], p[1], p[2]), n)
            .map_err(|e| OpError::kernel("Hole", e))?;
        current = Some(next);
    }
    finish("Hole", current.expect("positions tidak kosong"))
}
