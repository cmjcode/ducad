//! Backend OpenCASCADE (`kernel-occt`). Kode di sini dipindah apa adanya dari
//! modul-modul lama `ducad-kernel` (PLAN_MIGRASI_TRUCK F2.2); dihapus di F6.

pub mod advanced;
pub mod csg;
pub mod helix;
pub mod hlr;
pub mod hlr_exact;
pub(crate) mod hlr_sheet;
pub mod mesh;
pub mod modify;
pub mod picking_edge;
pub mod picking_face;
pub mod picking_vertex;
pub mod primitives;
pub mod profile;
pub mod query;
pub mod section;
pub mod shape;
pub mod step;
pub mod topo;

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
/// `ducad-app` cuma pernah memanggil kernel dari UI thread tunggal.
/// Fase 7 menambah `import_worker` di `ducad-app` (thread latar belakang
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
