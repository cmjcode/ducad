//! Wrapper DUCAD di atas kernel OpenCASCADE (via opencascade-rs).
//!
//! Seluruh aplikasi hanya boleh menyentuh tipe dari crate ini, bukan
//! `opencascade` langsung — agar detail FFI terisolasi dan kernel bisa
//! ditambal/diganti tanpa merombak app. `Shape` OCCT sengaja tidak pernah
//! `pub`: [`KernelShape`] membungkusnya sepenuhnya.

pub mod csg;
pub mod helix;
pub mod hlr;
pub mod hole;
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

// Re-exports for public API compatibility
pub use csg::{
    emboss_profiles_on_plane, extrude_profile, extrude_profile_on_plane, intersect, loft_profiles,
    revolve_profile, subtract, sweep_profile_along_path, sweep_profile_along_wire,
    sweep_profile_on_plane_along_path, union,
};
pub use helix::{
    create_helix_path_segments, create_helix_solid, create_helix_solid_with_custom_profile,
    create_helix_wire, generate_helix_points, HelixHandedness, HelixParams, HelixProfileKind,
};
pub use hlr::{
    HlrDrawing, HlrExtractor, HlrGeometricFeature, HlrLineKind, HlrSegment2D, ProjectedView,
    ProjectedViewKind,
};
pub use hole::{apply_hole, create_hole_cutter};
pub use mesh::KernelMesh;
pub use modify::{
    chamfer_all, chamfer_edges, chamfer_vertex, circular_pattern_shape, create_rib,
    create_rib_from_curve, create_rib_solid, draft_angle, extrude_face, fillet_all,
    fillet_edges, fillet_edges_variable, fillet_vertex, linear_pattern_shape, make_filleted_box,
    resize_shape_along_edge, revolve_face, shell_hollow, shell_hollow_faces,
    shell_variable_thickness, split_body, split_body_with_tool, split_face, Direction,
};
pub use picking::{
    edge_dimensions, pick_edge, pick_face, pick_face_details, pick_vertex, point_in_polygon_2d,
    shape_vertices, EdgeDimension, EdgePickHit, FaceHit, PickRay, SurfaceKind,
};
pub use profile::{PathSegment, Profile, ProfileSegment};
pub use shape::{
    clone_shape, rotate_shape, scale_shape, transform_shape, translate_shape, KernelShape,
};
pub use step::write_step_compound;
