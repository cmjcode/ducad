use anyhow::{bail, Result};
use opencascade::primitives::{Compound, Shape};

use crate::lock_kernel;
use crate::shape::KernelShape;

/// Tulis beberapa shape ke SATU file STEP, masing-masing tetap solid
/// terpisah (dibungkus `TopoDS_Compound`, BUKAN di-union jadi satu solid).
/// Dipakai export "semua body" (Fase 5, `ducad-io`) — tool CAD lain yang
/// membuka file ini akan melihat N solid terpisah, sesuai isi dokumen
/// DUCAD aslinya.
pub fn write_step_compound(
    shapes: &[&KernelShape],
    path: impl AsRef<std::path::Path>,
) -> Result<()> {
    if shapes.is_empty() {
        bail!("tidak ada body untuk diekspor");
    }
    let _guard = lock_kernel();
    let refs: Vec<&Shape> = shapes.iter().map(|s| s.inner()).collect();
    let compound = Compound::from_shapes(refs);
    let combined: Shape = compound.into();
    combined.write_step(path)?;
    Ok(())
}
