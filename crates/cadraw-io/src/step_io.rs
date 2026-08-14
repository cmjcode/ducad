//! Interop STEP (AP214) — export/import file `.step` SUNGGUHAN di disk,
//! dibaca tool CAD lain (FreeCAD, SolidWorks, dst). Beda dari
//! `native::save`/`load`, yang menyematkan teks STEP yang sama DI DALAM
//! JSON `.cadraw` — modul ini cuma tipis di atas `cadraw-kernel`, isinya
//! didokumentasikan di sana (`KernelShape::write_step`/`read_step`,
//! `write_step_compound`).

use anyhow::{bail, Result};
use cadraw_kernel::KernelShape;
use std::path::Path;

/// Export beberapa body sekaligus ke SATU file STEP — masing-masing tetap
/// solid terpisah (lewat `Compound`) kalau lebih dari satu, atau ditulis
/// langsung kalau cuma satu (menghindari pembungkus compound yang tak
/// perlu).
pub fn export(shapes: &[&KernelShape], path: impl AsRef<Path>) -> Result<()> {
    match shapes {
        [] => bail!("tidak ada body untuk diekspor ke STEP"),
        [single] => single.write_step(path),
        many => cadraw_kernel::write_step_compound(many, path),
    }
}

/// Import satu file STEP jadi satu `KernelShape` baru — dipakai tool
/// "Import STEP" (hasil ditambahkan sebagai body baru di `ModelDoc`). File
/// STEP yang berisi beberapa solid tetap terbaca sebagai SATU `KernelShape`
/// gabungan — memisahkannya lagi per-solid butuh traversal topologi yang
/// belum ada di lingkup Fase 5 (dicatat sebagai batasan sadar, sama pola
/// dengan fillet/chamfer "semua tepi sekaligus" di Fase 3).
pub fn import(path: impl AsRef<Path>) -> Result<KernelShape> {
    KernelShape::read_step(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::occt_test_lock::LOCK as TEST_LOCK;
    use cadraw_kernel::{extrude_profile, Profile, ProfileSegment};

    fn rect_profile(w: f64, h: f64) -> Profile {
        Profile::Loop(vec![
            ProfileSegment::Line { start: (0.0, 0.0), end: (w, 0.0) },
            ProfileSegment::Line { start: (w, 0.0), end: (w, h) },
            ProfileSegment::Line { start: (w, h), end: (0.0, h) },
            ProfileSegment::Line { start: (0.0, h), end: (0.0, 0.0) },
        ])
    }

    #[test]
    fn export_single_then_import_preserves_mesh() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = extrude_profile(&rect_profile(10.0, 10.0), 5.0).unwrap();
        let path = std::env::temp_dir().join(format!("cadraw-io-test-step-single-{}.step", std::process::id()));
        export(&[&shape], &path).unwrap();
        let restored = import(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(shape.tessellate().positions.len(), restored.tessellate().positions.len());
    }

    #[test]
    fn export_many_combines_bodies() {
        let _guard = TEST_LOCK.lock().unwrap();
        let a = extrude_profile(&rect_profile(10.0, 10.0), 5.0).unwrap();
        let b = extrude_profile(&rect_profile(30.0, 30.0), 5.0).unwrap();
        let path = std::env::temp_dir().join(format!("cadraw-io-test-step-many-{}.step", std::process::id()));
        export(&[&a, &b], &path).unwrap();
        let restored = import(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(restored.tessellate().positions.len() > a.tessellate().positions.len());
    }

    #[test]
    fn export_empty_errors() {
        let path = std::env::temp_dir().join("cadraw-io-test-step-empty.step");
        assert!(export(&[], &path).is_err());
    }
}
