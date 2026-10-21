//! IGES 5.3 (Paritas 3.11): baca/tulis lewat penerjemah IGES SolidCraft
//! (IGES ↔ STEP di dalam kernel, satuan mm).

use anyhow::{anyhow, bail, Context, Result};
use solidcraft_kernel::{ExportBody, StepHeader};

use super::kernel;
use crate::shape::KernelShape;

/// Teks IGES untuk beberapa shape (masing-masing tetap solid terpisah).
pub fn iges_string(shapes: &[(&str, &KernelShape)]) -> Result<String> {
    let bodies: Vec<ExportBody> = shapes
        .iter()
        .filter_map(|(name, s)| {
            s.body().map(|body| ExportBody {
                name: (*name).to_string(),
                body,
                color: None,
            })
        })
        .collect();
    if bodies.is_empty() {
        bail!("tidak ada body untuk diekspor ke IGES");
    }
    let header = StepHeader {
        description: "DUCAD model".into(),
        ..Default::default()
    };
    kernel("IGES export", || solidcraft_kernel::iges_export_bodies(&bodies, &header))
}

/// Tulis shape ke berkas `.igs`/`.iges`.
pub fn write_iges(shapes: &[(&str, &KernelShape)], path: impl AsRef<std::path::Path>) -> Result<()> {
    let text = iges_string(shapes)?;
    std::fs::write(path, text).context("write_iges: gagal menulis berkas")
}

/// Batas ukuran berkas IGES yang dibaca (sama dengan SolidCraft).
pub const MAX_IGES_BYTES: usize = solidcraft_kernel::MAX_IGES_BYTES;

/// Hasil impor IGES: shape bernama dan peringatan entitas yang tidak terbaca.
pub type IgesImport = (Vec<(String, KernelShape)>, Vec<String>);

/// Baca teks IGES: tiap solid (atau shell hasil jahitan permukaan) jadi satu
/// shape bernama, plus peringatan entitas yang tidak terbaca.
pub fn read_iges_string(text: &str) -> Result<IgesImport> {
    if text.len() > MAX_IGES_BYTES {
        bail!("berkas IGES terlalu besar ({} byte, maks {MAX_IGES_BYTES})", text.len());
    }
    let import = kernel("IGES import", || solidcraft_kernel::iges_import(text))?;
    let shapes: Vec<(String, KernelShape)> = import
        .bodies
        .into_iter()
        .enumerate()
        .map(|(i, b)| {
            let name = if b.name.trim().is_empty() { format!("IGES {}", i + 1) } else { b.name };
            (name, KernelShape::from_body(b.body))
        })
        .collect();
    if shapes.is_empty() {
        return Err(anyhow!("IGES tidak berisi solid/permukaan yang terbaca"));
    }
    Ok((shapes, import.warnings))
}

/// Baca berkas `.igs`/`.iges`.
pub fn read_iges(path: impl AsRef<std::path::Path>) -> Result<IgesImport> {
    let bytes = std::fs::read(path).context("read_iges: gagal membaca berkas")?;
    let text = String::from_utf8_lossy(&bytes);
    read_iges_string(&text)
}
