//! Wrapper CADRAW di atas kernel OpenCASCADE (via opencascade-rs).
//!
//! Seluruh aplikasi hanya boleh menyentuh tipe dari crate ini, bukan
//! `opencascade` langsung — agar detail FFI terisolasi dan kernel bisa
//! ditambal/diganti tanpa merombak app. `Shape` OCCT sengaja tidak pernah
//! `pub`: [`KernelShape`] membungkusnya sepenuhnya.

pub mod csg;
pub mod mesh;
pub mod modify;
pub mod picking;
pub mod profile;
pub mod shape;
pub mod step;

#[cfg(test)]
mod tests;

use std::sync::Mutex;

/// Kunci global memaksa SEMUA pemanggilan ke kernel OCCT serial lintas
/// thread mana pun. OCCT (setidaknya jalur transfer STEP — dibuktikan
/// lewat crash `SIGABRT`/`Interface_InterfaceError` di test suite Fase 3,
/// lihat `tests::TEST_LOCK`) TIDAK thread-safe untuk dipanggil BERSAMAAN
/// dari >1 thread; juga dikonfirmasi lewat compile-time check bahwa
/// `KernelShape` (dan `opencascade::Shape` di baliknya) TIDAK `Send` —
/// `UniquePtr<TopoDS_Shape>` milik `cxx` tidak pernah diberi `unsafe impl
/// Send` di binding ini, konsisten dengan OCCT yang memang tidak
/// thread-safe. Sampai Fase 6, ini bukan masalah nyata karena
/// `cadraw-app` cuma pernah memanggil kernel dari UI thread tunggal.
/// Fase 7 menambah `import_worker` di `cadraw-app` (thread latar belakang
/// untuk Import STEP, supaya UI tidak beku selama tessellation shape
/// besar) — begitu ADA thread kedua yang bisa memanggil kernel, lock ini
/// jadi wajib di setiap fungsi publik supaya tidak pernah ada 2
/// panggilan OCCT jalan bersamaan, apa pun urutan aksi user (mis. klik
/// Extrude persis saat import STEP di latar belakang masih berjalan).
/// Dipegang HANYA di fungsi publik, bukan di helper privat seperti
/// `deep_clone`/`tessellate_shape` yang selalu dipanggil dari dalam
/// fungsi publik yang sudah memegang lock — `Mutex` std TIDAK reentrant,
/// mengunci dua kali dari thread yang sama akan deadlock.
static KERNEL_LOCK: Mutex<()> = Mutex::new(());

/// Kunci `KERNEL_LOCK`, pulih dari poisoning (panic sebelumnya sambil
/// memegang lock) alih-alih ikut panic — satu operasi kernel yang gagal
/// tidak seharusnya mengunci permanen SEMUA operasi kernel berikutnya di
/// aplikasi UI yang harus tetap responsif.
pub(crate) fn lock_kernel() -> std::sync::MutexGuard<'static, ()> {
    KERNEL_LOCK.lock().unwrap_or_else(|poison| poison.into_inner())
}

// Re-exports for public API compatibility
pub use csg::{
    extrude_profile, extrude_profile_on_plane, intersect, loft_profiles, revolve_profile,
    subtract, union,
};
pub use mesh::KernelMesh;
pub use modify::{
    chamfer_all, chamfer_edges, extrude_face, fillet_all, fillet_edges, fillet_vertex,
    make_filleted_box, shell_hollow, shell_hollow_faces, Direction,
};
pub use picking::{
    edge_dimensions, pick_edge, pick_face, pick_face_details, pick_vertex, shape_vertices,
    EdgeDimension, EdgePickHit, FaceHit, PickRay, SurfaceKind,
};
pub use profile::{Profile, ProfileSegment};
pub use shape::{clone_shape, rotate_shape, transform_shape, translate_shape, KernelShape};
pub use step::write_step_compound;
