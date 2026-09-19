//! Pola linear & melingkar 3D. Hanya SALINAN baru yang dikembalikan (body
//! asli tidak termasuk), sama dengan kernel.

use ducad_kernel::KernelShape;

use super::finish;
use crate::error::{OpError, OpResult};
use crate::model::BodyGeometry;

pub fn linear_pattern(
    shape: &KernelShape,
    count: [usize; 3],
    pitch: [f64; 3],
) -> OpResult<Vec<BodyGeometry>> {
    if count.contains(&0) {
        return Err(OpError::invalid(format!(
            "Jumlah pattern per sumbu harus >= 1 (diberikan {count:?})"
        )));
    }
    if pitch.iter().any(|p| !p.is_finite()) {
        return Err(OpError::invalid(format!(
            "Jarak pattern tidak valid ({pitch:?})"
        )));
    }
    let shapes = ducad_kernel::linear_pattern_shape(
        shape, count[0], pitch[0], count[1], pitch[1], count[2], pitch[2],
    )
    .map_err(|e| OpError::kernel("Linear Pattern", e))?;
    shapes
        .into_iter()
        .map(|s| finish("Linear Pattern", s))
        .collect()
}

pub fn circular_pattern(
    shape: &KernelShape,
    pivot: [f64; 3],
    axis: [f64; 3],
    count: usize,
    total_angle_deg: f64,
) -> OpResult<Vec<BodyGeometry>> {
    if count == 0 {
        return Err(OpError::invalid("Jumlah pattern melingkar harus >= 1"));
    }
    let len = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    if len.is_nan() || len < 1e-9 {
        return Err(OpError::invalid(
            "Sumbu pattern melingkar tidak boleh vektor nol",
        ));
    }
    if !total_angle_deg.is_finite() || total_angle_deg.abs() < 1e-9 {
        return Err(OpError::invalid(format!(
            "Sudut total pattern harus tidak nol (diberikan {total_angle_deg})"
        )));
    }
    let shapes = ducad_kernel::circular_pattern_shape(
        shape,
        (pivot[0], pivot[1], pivot[2]),
        (axis[0], axis[1], axis[2]),
        count,
        total_angle_deg.to_radians(),
    )
    .map_err(|e| OpError::kernel("Circular Pattern", e))?;
    shapes
        .into_iter()
        .map(|s| finish("Circular Pattern", s))
        .collect()
}
