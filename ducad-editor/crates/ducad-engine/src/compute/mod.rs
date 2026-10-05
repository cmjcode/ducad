//! Lapisan `compute` — fungsi murni, satu implementasi per operasi, dipakai
//! GUI (`ducad-app`) maupun `Session` (CLI/MCP).
//!
//! Aturan semua fungsi di sini:
//! 1. Validasi parameter dulu → [`OpErrorCode::InvalidParam`].
//! 2. Error `anyhow` kernel → [`OpError::kernel`].
//! 3. Hasil diperiksa: tidak valid → `KernelFailed`; volume ≈ 0 → `EmptyResult`.
//!
//! Fungsi compute TIDAK menyentuh undo stack dan TIDAK memutasi `ModelDoc`:
//! ia menerima referensi dan mengembalikan geometri baru.

pub mod advanced;
pub mod hole;
pub mod pattern;
pub mod primitive;
pub mod round;
pub mod sheet_metal;
pub mod sketch;
pub mod standard;
pub mod solid;

pub use hole::hole;
pub use pattern::{circular_pattern, linear_pattern};
pub use primitive::{primitive, PrimitiveShape};
pub use round::{chamfer, fillet, shell, EdgePick, FacePick};
pub use sketch::solve_with;
pub use solid::{
    boolean, extrude, extrude_single_entity, extrude_vector, resolve_profiles, revolve,
    ProfilePick, VectorExtrudeOptions,
};
pub(crate) use solid::validate_extent;

use ducad_kernel::KernelShape;

use crate::error::{OpError, OpErrorCode, OpResult};
use crate::model::BodyGeometry;

/// Kesamaan posisi (mm) di selector/check — konvensi §4.
pub const LIN_TOL: f64 = 1e-4;

/// Ambang volume (mm³) di bawahnya hasil dianggap kosong.
const EMPTY_VOLUME: f64 = 1e-9;

/// Aturan 3: periksa validitas + volume, lalu bungkus jadi `BodyGeometry`.
pub(crate) fn finish(op: &str, shape: KernelShape) -> OpResult<BodyGeometry> {
    check_shape(op, &shape)?;
    Ok(BodyGeometry::from_shape(shape))
}

pub(crate) fn check_shape(op: &str, shape: &KernelShape) -> OpResult<()> {
    if !shape.is_valid() {
        return Err(OpError::new(
            OpErrorCode::KernelFailed,
            format!("{op} menghasilkan geometri yang tidak valid"),
        ));
    }
    let volume = shape.volume().abs();
    if volume < EMPTY_VOLUME {
        return Err(OpError::new(
            OpErrorCode::EmptyResult,
            format!("{op} menghasilkan solid kosong (volume {volume:.3e} mm³)"),
        )
        .with_context(serde_json::json!({ "volume": volume })));
    }
    Ok(())
}

pub(crate) fn require_positive(name: &str, value: f64) -> OpResult<()> {
    if value.is_nan() || value <= 0.0 || !value.is_finite() {
        return Err(
            OpError::invalid(format!("{name} harus > 0 (diberikan {value})"))
                .with_context(serde_json::json!({ "param": name, "value": value })),
        );
    }
    Ok(())
}
