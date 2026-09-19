//! Primitif solid ditempatkan di titik `at`.

use super::finish;
use crate::error::{OpError, OpResult};
use crate::model::BodyGeometry;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PrimitiveShape {
    Box { size: [f64; 3], centered: bool },
    Cylinder { r: f64, h: f64 },
    Sphere { r: f64 },
    Cone { r1: f64, r2: f64, h: f64 },
}

/// Buat primitif di sekitar origin lalu geser sejauh `at`.
pub fn primitive(shape: &PrimitiveShape, at: [f64; 3]) -> OpResult<BodyGeometry> {
    if at.iter().any(|c| !c.is_finite()) {
        return Err(OpError::invalid(format!(
            "Posisi primitif tidak valid ({at:?})"
        )));
    }
    let made = match *shape {
        PrimitiveShape::Box { size, centered } => {
            ducad_kernel::make_box(size[0], size[1], size[2], centered)
        }
        PrimitiveShape::Cylinder { r, h } => ducad_kernel::make_cylinder(r, h),
        PrimitiveShape::Sphere { r } => ducad_kernel::make_sphere(r),
        PrimitiveShape::Cone { r1, r2, h } => ducad_kernel::make_cone(r1, r2, h),
    }
    // Kernel hanya gagal di sini karena dimensi di luar domain.
    .map_err(|e| OpError::invalid(format!("{e:#}")))?;
    let placed = if at.iter().all(|c| c.abs() < 1e-12) {
        made
    } else {
        ducad_kernel::translate_shape(&made, at[0], at[1], at[2])
            .map_err(|e| OpError::kernel("Primitif", e))?
    };
    finish("Primitif", placed)
}
