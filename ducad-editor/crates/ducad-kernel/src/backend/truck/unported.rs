//! Modul backend truck yang belum (sepenuhnya) diporting.
//!
//! Signature-nya sama persis dengan backend OCCT; yang belum ada
//! mengembalikan [`crate::Unsupported`] (engine memetakannya ke kode
//! `unsupported` + hint), tidak pernah panic. HLR eksak & tampak potongan
//! B-rep → F4 (sampai itu, gambar kerja memakai jalur mesh).

/// `crate::hlr_exact` — HLR eksak F4 (tidak ada di SolidCraft; PLAN §4.1).
pub mod hlr_exact {
    use anyhow::Result;

    use super::super::unsupported;
    use crate::hlr_exact::ExactHlrView;
    use crate::shape::KernelShape;

    pub fn extract_exact_hlr(
        _shape: &KernelShape,
        _view_dir: (f64, f64, f64),
        _up: (f64, f64, f64),
    ) -> Result<ExactHlrView> {
        unsupported("extract_exact_hlr")
    }
}

/// Hook HLR eksak untuk `crate::hlr`: belum ada (F4), pemanggil memakai HLR
/// mesh dan mencatat `HLR_EXACT_FALLBACK`.
pub mod hlr {
    use glam::Vec3;

    use crate::hlr::{ProjectedView, ProjectedViewKind};
    use crate::shape::KernelShape;

    pub(crate) fn exact_projected_view(
        _shapes: &[&KernelShape],
        _sketch_segments: &[(Vec3, Vec3)],
        _view_kind: ProjectedViewKind,
        _model_bbox: ([f32; 3], [f32; 3]),
        _warnings: &mut Vec<String>,
    ) -> Option<ProjectedView> {
        None
    }
}

/// Hook tampak potongan B-rep untuk `crate::section`: belum ada (F4),
/// pemanggil mengiris mesh.
pub mod section {
    use crate::section::{BrepCut, BrepCutInput};

    pub(crate) fn cut_bodies(_input: &BrepCutInput<'_>) -> Result<Option<BrepCut>, String> {
        Ok(None)
    }
}

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
