use anyhow::{Context, Result};
use glam::dvec3;
use opencascade::primitives::Shape;

use crate::lock_kernel;
use crate::mesh::{tessellate_shape, KernelMesh};

/// Solid/shape B-rep OCCT. Field dalam sengaja privat — pemanggil di luar
/// crate ini hanya boleh membangunnya lewat fungsi di modul ini
/// (`extrude_profile`, `union`, `subtract`, dst) dan membacanya lewat
/// `tessellate`/`write_stl`, tidak pernah menyentuh tipe `opencascade`
/// langsung.
pub struct KernelShape(pub(crate) Shape);

impl KernelShape {
    pub(crate) fn from_inner(shape: Shape) -> Self {
        KernelShape(shape)
    }

    pub(crate) fn inner(&self) -> &Shape {
        &self.0
    }

    pub fn tessellate(&self) -> KernelMesh {
        let _guard = lock_kernel();
        tessellate_shape(&self.0)
    }

    pub fn write_stl(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let _guard = lock_kernel();
        self.0.write_stl(path)?;
        Ok(())
    }

    pub fn write_step(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let _guard = lock_kernel();
        self.0.write_step(path)?;
        Ok(())
    }

    /// Baca shape B-rep dari file STEP — kebalikan `write_step`. Dipakai
    /// Import STEP (Fase 5, `cadraw-io`; Fase 7, `cadraw-app::import_worker`)
    /// dan test/`deep_clone` internal.
    pub fn read_step(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let _guard = lock_kernel();
        Ok(KernelShape(
            Shape::read_step(path).context("read_step: gagal membaca STEP")?,
        ))
    }

    /// Serialize B-rep ini jadi teks STEP AP214 (bukan mesh — topologi+
    /// geometri persis, sama presisi dengan file `.step` biasa). Dipakai
    /// `cadraw-io` (Fase 5) untuk menyematkan body ke dalam SATU file
    /// native `.cadraw` tanpa pernah menyentuh tipe `opencascade` — cuma
    /// String, sama seperti `KernelMesh` membungkus mesh sebagai `[f32;3]`
    /// mentah. Roundtrip lewat file sementara (sama trik dengan
    /// `deep_clone`) karena binding ini tidak expose serialisasi in-memory.
    pub fn to_step_string(&self) -> Result<String> {
        let _guard = lock_kernel();
        let path = temp_step_path("to-step-string");
        let result = (|| -> Result<String> {
            self.0
                .write_step(&path)
                .context("to_step_string: gagal menulis STEP sementara")?;
            std::fs::read_to_string(&path)
                .context("to_step_string: gagal membaca balik STEP sementara")
        })();
        let _ = std::fs::remove_file(&path);
        result
    }

    /// Kebalikan `to_step_string`.
    pub fn from_step_string(step: &str) -> Result<Self> {
        let _guard = lock_kernel();
        let path = temp_step_path("from-step-string");
        let result = (|| -> Result<Shape> {
            std::fs::write(&path, step)
                .context("from_step_string: gagal menulis STEP sementara")?;
            Shape::read_step(&path).context("from_step_string: gagal membaca balik STEP sementara")
        })();
        let _ = std::fs::remove_file(&path);
        result.map(KernelShape)
    }
}

/// Path file sementara unik (PID + timestamp nanosecond, sama pola dengan
/// yang dipakai `deep_clone` sebelumnya) — dipusatkan di sini supaya
/// `deep_clone`/`to_step_string`/`from_step_string` tidak menduplikasi
/// logika pembuatan nama file.
pub(crate) fn temp_step_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "cadraw-{tag}-{}-{}.step",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ))
}

/// `opencascade-rs` (0.2.0) tidak menyediakan `Clone` untuk `Shape` — objek
/// C++ di baliknya cuma dibungkus `UniquePtr` tanpa binding copy-constructor.
/// Satu-satunya cara publik untuk menyalin B-rep persis (bukan cuma
/// referensi) adalah roundtrip lewat file STEP (format yang menyimpan
/// topologi+geometri B-rep tepat, bukan mesh tessellation). Dipakai HANYA
/// oleh operasi yang secara internal memutasi shape di tempat (fillet/
/// chamfer) atau mengonsumsi kepemilikan (`hollow`), supaya shape ASLI
/// milik pemanggil tetap utuh untuk keperluan undo — bukan technical debt,
/// keputusan sadar mengingat batasan binding versi ini.
pub(crate) fn deep_clone(shape: &Shape) -> Result<Shape> {
    let path = temp_step_path("deep-clone");
    shape
        .write_step(&path)
        .context("deep_clone: gagal menulis STEP sementara")?;
    let result =
        Shape::read_step(&path).context("deep_clone: gagal membaca balik STEP sementara");
    let _ = std::fs::remove_file(&path);
    result
}

/// Deep-clone publik sebuah shape — dipakai app untuk menyimpan snapshot
/// B-rep (mis. shape dasar SEBELUM rounding parametrik pertama, supaya
/// radius bisa diubah/di-nol-kan lagi dengan rebuild dari dasar) tanpa
/// membuka akses ke `deep_clone` internal maupun detail locking kernel.
pub fn clone_shape(shape: &KernelShape) -> Result<KernelShape> {
    let _guard = lock_kernel();
    Ok(KernelShape(deep_clone(&shape.0)?))
}

/// Geser shape sepanjang X/Y/Z dunia sejauh `(dx, dy, dz)` mm — dipakai
/// gizmo drag axis body 3D. Fungsional (tidak memutasi `shape` pemanggil):
/// `Shape` tidak `Clone`, jadi `deep_clone` dulu sama seperti
/// `fillet_all`/`chamfer_all`, tapi di sini transformasinya jauh lebih
/// murah — `set_global_translation` (API vendor `opencascade-0.2.0`,
/// sudah ada) cuma menggeser `Location` shape, TIDAK merombak B-rep sama
/// sekali (beda dari fillet/chamfer/boolean yang benar-benar membangun
/// ulang geometri). `dx`/`dy`/`dz` adalah delta, bukan posisi absolut —
/// pemanggil (gizmo di `cadraw-app`) selalu menghitung ulang dari shape
/// ASLI sebelum drag dimulai (pola sama dgn gizmo extrude face lain),
/// jadi tidak ada akumulasi error floating-point lintas frame drag.
pub fn translate_shape(shape: &KernelShape, dx: f64, dy: f64, dz: f64) -> Result<KernelShape> {
    let _guard = lock_kernel();
    let mut cloned = deep_clone(&shape.0)?;
    cloned.set_global_translation(dvec3(dx, dy, dz));
    Ok(KernelShape(cloned))
}
