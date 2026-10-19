//! Backend truck (`kernel-truck`): SolidCraft kernel di atas truck, di-vendor
//! di `vendors/solidcraft` (PLAN_MIGRASI_TRUCK F2).
//!
//! Prinsip:
//! - API publik sama persis dengan backend OCCT; tipe SolidCraft tidak pernah
//!   bocor keluar crate.
//! - Setiap panggilan ke SolidCraft lewat [`kernel`], yang memakai
//!   `solidcraft_kernel::guard` (panic → `Err`) dan membungkus pesan galatnya
//!   dalam bahasa DUCAD.
//! - Operasi yang belum diporting (fase F3/F4) mengembalikan
//!   [`crate::Unsupported`] lewat [`unsupported`], tidak pernah panic.
//! - Tidak ada kunci global: `KernelShape` di sini `Send + Sync`.

pub(crate) mod bridge;
pub mod csg;
pub mod picking_edge;
pub mod picking_face;
pub mod picking_vertex;
pub mod primitives;
pub mod query;
pub mod shape;
pub mod topo;
pub(crate) mod topology;
mod unported;
pub use unported::{advanced, helix, hlr, hlr_exact, mesh, modify, profile, section, step};

/// Pengganti `MutexGuard` backend OCCT: SolidCraft thread-safe, jadi
/// "mengunci" kernel tidak melakukan apa-apa. Ada supaya kode bersama
/// (`let _guard = lock_kernel();`) tetap sama untuk kedua backend.
pub(crate) struct KernelGuard;

/// No-op — lihat [`KernelGuard`].
pub(crate) fn lock_kernel() -> KernelGuard {
    KernelGuard
}

/// Jalankan `f` di bawah `solidcraft_kernel::guard` (panic di truck jadi
/// `Err`) dan ubah galatnya ke `anyhow` dengan konteks operasi DUCAD.
pub(crate) fn kernel<T>(
    what: &str,
    f: impl FnOnce() -> solidcraft_kernel::Result<T>,
) -> anyhow::Result<T> {
    solidcraft_kernel::guard(what, f).map_err(|e| anyhow::anyhow!("{what}: {e}"))
}

/// `Err(Unsupported)` untuk operasi yang belum diporting ke backend truck.
pub(crate) fn unsupported<T>(operation: &'static str) -> anyhow::Result<T> {
    Err(crate::Unsupported {
        operation,
        backend: crate::BACKEND,
    }
    .into())
}

// `KernelShape` WAJIB `Send + Sync` di backend ini (PLAN_MIGRASI_TRUCK 2.7):
// itulah yang memungkinkan `KERNEL_LOCK` dihapus di F5.
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<shape::KernelShape>();
};
