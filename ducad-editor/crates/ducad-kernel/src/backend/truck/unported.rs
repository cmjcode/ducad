//! Modul backend truck yang belum (sepenuhnya) diporting.
//!
//! Signature-nya sama persis dengan backend OCCT; yang belum ada
//! mengembalikan [`crate::Unsupported`] (engine memetakannya ke kode
//! `unsupported` + hint), tidak pernah panic.

/// `crate::step` — ekspor beberapa body ke satu berkas STEP.
pub mod step {
    use anyhow::{bail, Context, Result};
    use solidcraft_kernel::Body;

    use super::super::kernel;
    use crate::shape::KernelShape;

    /// Tulis beberapa shape ke SATU file STEP, masing-masing tetap solid
    /// terpisah (`step_export` SolidCraft, satu produk per body).
    pub fn write_step_compound(
        shapes: &[&KernelShape],
        path: impl AsRef<std::path::Path>,
    ) -> Result<()> {
        if shapes.is_empty() {
            bail!("tidak ada body untuk diekspor");
        }
        let bodies: Vec<&Body> = shapes.iter().filter_map(|s| s.body()).collect();
        if bodies.is_empty() {
            bail!("tidak ada body untuk diekspor");
        }
        let text = kernel("write_step_compound", || {
            solidcraft_kernel::step_export(&bodies, "DUCAD")
        })?;
        std::fs::write(path, text).context("write_step_compound: gagal menulis berkas")
    }
}

/// `crate::mesh` — tidak ada helper khusus backend (tesselasi lewat
/// `KernelShape::tessellate`).
pub mod mesh {}

/// `crate::profile` — konversi profil ada di `super::bridge`.
pub mod profile {}
