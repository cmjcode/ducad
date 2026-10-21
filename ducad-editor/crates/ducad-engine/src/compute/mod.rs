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
pub mod budget;
pub mod datum;
pub mod direct;
pub mod hole;
pub mod pattern;
pub mod plastic;
pub mod primitive;
pub mod project;
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

/// Panjang terbesar yang diterima (1 km). Ukuran absurd membuat kernel
/// kehilangan presisi (toleransi relatif) atau bekerja tanpa batas, dan di
/// wasm panic = abort, jadi angka seperti itu harus berhenti di sini
/// (ADR 0009).
pub(crate) const MAX_LENGTH_MM: f64 = 1.0e6;
/// Nilai absolut terbesar untuk angka apa pun di dalam op (koordinat,
/// jarak, gaya, …): di atasnya kernel/solver kehilangan presisi atau abort.
pub(crate) const MAX_ABS_VALUE: f64 = 1.0e9;
/// Panjang terkecil yang diterima (1 nm): di bawah toleransi titik kernel
/// (1e-7 mm) edge/face runtuh menjadi degenerate dan pembuat shape gagal.
pub(crate) const MIN_LENGTH_MM: f64 = 1.0e-6;

pub(crate) fn require_positive(name: &str, value: f64) -> OpResult<()> {
    if value.is_nan() || value <= 0.0 || !value.is_finite() {
        return Err(
            OpError::invalid(format!("{name} harus > 0 (diberikan {value})"))
                .with_context(serde_json::json!({ "param": name, "value": value })),
        );
    }
    if value < MIN_LENGTH_MM {
        return Err(OpError::invalid(format!(
            "{name} terlalu kecil: {value} (minimum {MIN_LENGTH_MM})"
        ))
        .with_context(serde_json::json!({ "param": name, "value": value, "min": MIN_LENGTH_MM })));
    }
    if value > MAX_LENGTH_MM {
        return Err(OpError::invalid(format!(
            "{name} terlalu besar: {value} (maksimum {MAX_LENGTH_MM})"
        ))
        .with_context(serde_json::json!({ "param": name, "value": value, "max": MAX_LENGTH_MM })));
    }
    Ok(())
}

/// Tolak op yang memuat angka di luar ±[`MAX_ABS_VALUE`] (dicek sebelum
/// op menyentuh kernel). `pointer` di context menunjuk field-nya.
pub(crate) fn check_op_magnitudes(op: &crate::ops::Op) -> OpResult<()> {
    fn walk(v: &serde_json::Value, path: &mut String) -> Option<(String, f64)> {
        match v {
            serde_json::Value::Number(n) => n
                .as_f64()
                .filter(|x| !x.is_finite() || x.abs() > MAX_ABS_VALUE)
                .map(|x| (path.clone(), x)),
            serde_json::Value::Array(a) => a.iter().enumerate().find_map(|(i, c)| {
                let len = path.len();
                path.push_str(&format!("/{i}"));
                let hit = walk(c, path);
                path.truncate(len);
                hit
            }),
            serde_json::Value::Object(m) => m.iter().find_map(|(k, c)| {
                let len = path.len();
                path.push('/');
                path.push_str(k);
                let hit = walk(c, path);
                path.truncate(len);
                hit
            }),
            _ => None,
        }
    }
    let Ok(v) = serde_json::to_value(op) else {
        return Ok(());
    };
    match walk(&v, &mut String::new()) {
        Some((pointer, value)) => Err(OpError::invalid(format!(
            "nilai {value} di {pointer} di luar jangkauan model (±{MAX_ABS_VALUE})"
        ))
        .with_context(serde_json::json!({ "pointer": pointer, "value": value, "max_abs": MAX_ABS_VALUE }))),
        None => Ok(()),
    }
}
