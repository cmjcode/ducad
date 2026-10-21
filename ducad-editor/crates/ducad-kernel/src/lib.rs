//! Kernel geometri DUCAD: API B-rep di atas SolidCraft/truck (Rust murni).
//!
//! Seluruh aplikasi hanya boleh menyentuh tipe dari crate ini, bukan crate
//! kernel pihak ketiga langsung — agar detail backend terisolasi dan kernel
//! bisa ditambal/diganti tanpa merombak app. Tipe backend sengaja tidak
//! pernah `pub`: [`KernelShape`] membungkusnya sepenuhnya. Lihat
//! `src/backend/mod.rs` dan docs/PLAN_MIGRASI_TRUCK.md.

// Prinsip "jangan pernah crash" (ADR 0009): tidak ada unwrap/expect/panic di
// kode produksi; modul tes dikecualikan.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::todo, clippy::unimplemented)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod backend;

pub mod advanced;
pub mod assembly_solver;
pub mod csg;
pub use backend::direct;
pub use backend::iges;
pub use backend::surface;
pub mod detail;
pub mod diagnostics;
pub mod helix;
pub mod hlr;
pub mod hlr_exact;
mod hlr_sheet;
pub mod hole;
pub mod interference;
pub mod mass;
pub mod mesh;
pub mod modify;
pub mod picking;
pub use backend::primitives;
pub mod profile;
pub mod projection;
pub mod section;
pub use backend::shape;
pub use backend::step;
pub mod thickness;
pub mod thread;
pub mod topo;
pub mod vector_snapshot;

// Tes modul kernel.
#[cfg(test)]
mod tests;
// Tes kontrak: hanya API publik.
#[cfg(test)]
mod contract_tests;

/// Nama backend kernel (`"truck"`) — dicatat golden corpus, jurnal panik,
/// dan berkas cache lintas-versi.
pub const BACKEND: &str = "truck";

/// Toleransi relatif [`KernelShape::volume`] (PLAN_MIGRASI_TRUCK §3.1, F5.1):
/// `measure()` SolidCraft berbasis tesselasi halus, ±1e-4 — pembanding
/// volume/massa memakai 5e-4.
pub const VOLUME_REL_TOL: f64 = 5e-4;

/// [`KernelShape`] boleh dipakai dari banyak thread sekaligus (`Send + Sync`,
/// tanpa kunci global).
pub const THREAD_SAFE: bool = true;

/// Error bertipe: operasi ini belum didukung backend kernel yang aktif
/// (PLAN_MIGRASI_TRUCK §0 butir 4 — op yang tidak didukung mengembalikan
/// `Err`, tidak pernah panic). Engine memetakannya ke kode `unsupported`
/// lewat `downcast_ref`, bukan mencocokkan teks pesan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unsupported {
    /// Nama operasi kernel, mis. `"fillet_edges"`.
    pub operation: &'static str,
    /// Backend yang menolak (`BACKEND`).
    pub backend: &'static str,
}

impl std::fmt::Display for Unsupported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} belum didukung backend kernel {}", self.operation, self.backend)
    }
}

impl std::error::Error for Unsupported {}

pub use diagnostics::capture_kernel_warnings;

// Re-exports for public API compatibility
pub use advanced::{
    draft_faces_by_index, fillet_edges_variable_by_index, loft_sections, mirror_shape, LoftSection,
};
pub use assembly_solver::{
    apply_mate_transform_to_shape, evaluate_motion, solve_angle, solve_assembly,
    solve_assembly_with_couplings, solve_assembly_with_drag, solve_coincident, solve_concentric,
    AssemblySolveReport,
    solve_single_mate, MateTransformResult,
};
pub use csg::{
    emboss_profiles_on_plane, extrude_profile, extrude_profile_extent, extrude_profile_on_plane,
    intersect, loft_profiles, EmptyIntersection, ExtrudeExtent,
    revolve_profile, subtract, sweep_profile_along_path, union_all,
    sweep_profile_on_plane_along_path, union,
};
pub use detail::{clip_segment_to_circle, DetailExtractor, DetailIndicator, DetailViewData};
pub use helix::{
    create_helix_path_segments, create_helix_solid, create_helix_solid_with_custom_profile,
    generate_helix_points, HelixHandedness, HelixParams, HelixProfileKind,
};
pub use hlr::{
    DrawingOptions, HlrArc2D, HlrDrawing, HlrExtractor, HlrGeometricFeature, HlrLineKind,
    HlrSegment2D, ProjectedView, ProjectedViewKind,
};
pub use hlr_exact::{extract_exact_hlr, ExactCurve2D, ExactCurveType, ExactHlrView, ExactLineKind};
pub use projection::{
    clip_segment_to_halfspace, clip_segment_to_rect, Projector, DEFAULT_DEPTH_TOLERANCE_MM,
};
pub use vector_snapshot::{
    extract_vector_snapshot, SnapshotBody, SnapshotCamera, SnapshotOptions, VectorSnapshot,
};
pub use hole::{apply_hole, apply_holes, create_hole_cutter};
pub use interference::{
    check_clearance, compute_mesh_centroid, compute_mesh_volume, compute_pair_interference,
    detect_interference, BodyClash, ClearanceResult,
};
pub use mass::{principal_axes, shift_from_centroid, shift_to_centroid, MassProperties};
pub use mesh::KernelMesh;
pub use modify::{
    chamfer_all, chamfer_edges, chamfer_vertex, circular_pattern_shape, create_rib,
    create_rib_from_curve, create_rib_solid, draft_angle, edges_for_rays, extrude_face, fillet_all,
    fillet_edges, fillet_edges_by_index, chamfer_edges_by_index, shell_faces_by_index, fillet_edges_variable, fillet_vertex, linear_pattern_shape, make_filleted_box,
    resize_shape_along_edge, revolve_face, shell_hollow, shell_hollow_faces, shell_open,
    shell_variable_thickness, split_body, split_body_with_tool, split_face, Direction,
    ShellDepthTooDeep, ShellOpening,
};
pub use picking::{
    edge_dimensions, edge_outward_normal, face_index_for_ray, pick_edge, pick_face, pick_face_details, pick_vertex,
    point_in_polygon_2d, shape_vertices, vertex_outward_normal, EdgeDimension, EdgeNormalHit,
    EdgePickHit, FaceHit, PickRay, RadialAxis, SurfaceKind, Vec3Raw,
};
pub use primitives::{make_box, make_cone, make_cylinder, make_sphere};
pub use direct::{
    bounding_box_solid, bounding_cylinder_solid, boundary_fill, delete_faces_by_index, make_torus,
    seam_continuity, Continuity, SeamContinuity,
    offset_faces_by_index, point_inside, replace_face_with_plane, split_faces_by_curves,
    split_faces_by_plane,
};
pub use profile::{PathSegment, Profile, ProfileSegment};
pub use section::{
    generate_iso_hatch_pattern, CuttingLineIndicator, SectionAxis, SectionExtractor, SectionPath,
    SectionPlaneConfig, SectionRequest, SectionView,
};
pub use shape::{
    clone_shape, extract_shape_edges, make_compound, rotate_shape, scale_shape, transform_shape,
    translate_shape, KernelShape,
};
pub use step::write_step_compound;
pub use thread::{cut_iso_thread, IsoThreadProfile};
pub use thickness::{
    min_wall_thickness, ray_hit_distance, MeshRaycaster, WallReport, DEFAULT_WALL_SAMPLES,
};
pub use topo::{enumerate_edges, enumerate_faces, EdgeInfo, EdgeKind, FaceInfo};
