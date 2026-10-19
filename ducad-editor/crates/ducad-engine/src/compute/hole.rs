//! Lubang (Hole Wizard) headless.

use ducad_core::hole::HoleSpec;
use ducad_kernel::KernelShape;

use super::{finish, require_positive};
use crate::error::{OpError, OpResult};
use crate::model::BodyGeometry;

/// Lubang di setiap posisi dengan normal keluar face yang sama
/// (`ducad_kernel::apply_holes`).
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
    let at: Vec<(f64, f64, f64)> = positions.iter().map(|p| (p[0], p[1], p[2])).collect();
    let current = Some(
        ducad_kernel::apply_holes(shape, spec, &at, n).map_err(|e| OpError::kernel("Hole", e))?,
    );
    match current {
        Some(shape) => finish("Hole", shape),
        None => Err(OpError::invalid("Posisi lubang tidak boleh kosong")),
    }
}
