//! Modul backend truck yang di F2 belum (sepenuhnya) diporting.
//!
//! Signature-nya sama persis dengan backend OCCT; yang belum ada
//! mengembalikan [`crate::Unsupported`] (engine memetakannya ke kode
//! `unsupported` + hint), tidak pernah panic. Jadwal porting: fillet,
//! chamfer, shell, draft, split, rib, helix → F3; HLR eksak & tampak
//! potongan B-rep → F4 (sampai itu, gambar kerja memakai jalur mesh).

/// `crate::modify` — semua op F3.
pub mod modify {
    use anyhow::Result;
    use glam::DVec3;

    use super::super::unsupported;
    use crate::modify::{Direction, ShellOpening};
    use crate::picking::ray::PickRay;
    use crate::shape::KernelShape;

    pub fn fillet_all(_shape: &KernelShape, _radius: f64) -> Result<KernelShape> {
        unsupported("fillet_all")
    }
    pub fn chamfer_all(_shape: &KernelShape, _distance: f64) -> Result<KernelShape> {
        unsupported("chamfer_all")
    }
    pub fn fillet_edges(
        _shape: &KernelShape,
        _radius: f64,
        _rays: &[PickRay],
        _tolerance: f64,
    ) -> Result<KernelShape> {
        unsupported("fillet_edges")
    }
    pub fn fillet_edges_variable(
        _shape: &KernelShape,
        _radius_start: f64,
        _radius_end: f64,
        _rays: &[PickRay],
        _tolerance: f64,
    ) -> Result<KernelShape> {
        unsupported("fillet_edges_variable")
    }
    pub fn fillet_vertex(
        _shape: &KernelShape,
        _radius: f64,
        _ray: PickRay,
        _tolerance: f64,
    ) -> Result<KernelShape> {
        unsupported("fillet_vertex")
    }
    pub fn chamfer_vertex(
        _shape: &KernelShape,
        _distance: f64,
        _ray: PickRay,
        _tolerance: f64,
    ) -> Result<KernelShape> {
        unsupported("chamfer_vertex")
    }
    pub fn chamfer_edges(
        _shape: &KernelShape,
        _distance: f64,
        _rays: &[PickRay],
        _tolerance: f64,
    ) -> Result<KernelShape> {
        unsupported("chamfer_edges")
    }
    pub fn fillet_edges_by_index(
        _shape: &KernelShape,
        _radius: f64,
        _edges: &[usize],
    ) -> Result<KernelShape> {
        unsupported("fillet_edges_by_index")
    }
    pub fn chamfer_edges_by_index(
        _shape: &KernelShape,
        _distance: f64,
        _edges: &[usize],
    ) -> Result<KernelShape> {
        unsupported("chamfer_edges_by_index")
    }
    pub fn shell_open(
        _shape: &KernelShape,
        thickness: f64,
        _opening: ShellOpening,
        _depth: f64,
    ) -> Result<KernelShape> {
        crate::modify::check_shell_thickness(thickness)?;
        unsupported("shell_open")
    }
    pub fn shell_faces_by_index(
        _shape: &KernelShape,
        thickness: f64,
        _remove_faces: &[usize],
    ) -> Result<KernelShape> {
        crate::modify::check_shell_thickness(thickness)?;
        unsupported("shell_faces_by_index")
    }
    pub fn shell_hollow(
        _shape: &KernelShape,
        thickness: f64,
        _remove_face_dir: Direction,
    ) -> Result<KernelShape> {
        crate::modify::check_shell_thickness(thickness)?;
        unsupported("shell_hollow")
    }
    pub fn shell_hollow_faces(
        _shape: &KernelShape,
        thickness: f64,
        _rays: &[PickRay],
    ) -> Result<KernelShape> {
        crate::modify::check_shell_thickness(thickness)?;
        unsupported("shell_hollow_faces")
    }
    pub fn extrude_face(
        _shape: &KernelShape,
        _ray: PickRay,
        _distance: f64,
    ) -> Result<KernelShape> {
        unsupported("extrude_face")
    }
    pub fn revolve_face(
        _shape: &KernelShape,
        _ray: PickRay,
        _axis_origin: DVec3,
        _axis_dir: DVec3,
        _angle_degrees: Option<f64>,
    ) -> Result<KernelShape> {
        unsupported("revolve_face")
    }
    pub fn draft_angle(
        _shape: &KernelShape,
        _neutral_plane_point: DVec3,
        _neutral_plane_normal: DVec3,
        _pull_direction: DVec3,
        _angle_deg: f64,
        _rays: &[PickRay],
    ) -> Result<KernelShape> {
        unsupported("draft_angle")
    }
    pub fn make_filleted_box(
        _width: f64,
        _depth: f64,
        _height: f64,
        _fillet: f64,
    ) -> Result<KernelShape> {
        unsupported("make_filleted_box")
    }
    pub fn resize_shape_along_edge(
        _shape: &KernelShape,
        _edge_start: (f64, f64, f64),
        _edge_end: (f64, f64, f64),
        _new_length: f64,
    ) -> Result<KernelShape> {
        unsupported("resize_shape_along_edge")
    }
    pub fn split_body(
        _shape: &KernelShape,
        _plane_point: DVec3,
        _plane_normal: DVec3,
    ) -> Result<Vec<KernelShape>> {
        unsupported("split_body")
    }
    pub fn split_body_with_tool(
        _shape: &KernelShape,
        _tool: &KernelShape,
    ) -> Result<Vec<KernelShape>> {
        unsupported("split_body_with_tool")
    }
    pub fn split_face(
        _shape: &KernelShape,
        _plane_point: DVec3,
        _plane_normal: DVec3,
    ) -> Result<KernelShape> {
        unsupported("split_face")
    }
    /// Tidak ada padanan di SolidCraft (PLAN_MIGRASI_TRUCK §4.3): gunakan
    /// shell seragam lalu offset per face.
    pub fn shell_variable_thickness(
        _shape: &KernelShape,
        _default_thickness: f64,
        _remove_face_rays: &[PickRay],
        _variable_thickness: &[(PickRay, f64)],
    ) -> Result<KernelShape> {
        unsupported("shell_variable_thickness")
    }
    pub fn create_rib_solid(
        _start_pt: DVec3,
        _end_pt: DVec3,
        _normal_dir: DVec3,
        _thickness: f64,
        _depth: f64,
        _draft_angle_deg: Option<f64>,
    ) -> Result<KernelShape> {
        unsupported("create_rib_solid")
    }
    pub fn create_rib(
        _shape: &KernelShape,
        _start_pt: DVec3,
        _end_pt: DVec3,
        _normal_dir: DVec3,
        _thickness: f64,
        _depth: f64,
        _draft_angle_deg: Option<f64>,
    ) -> Result<KernelShape> {
        unsupported("create_rib")
    }
}

/// `crate::advanced` — cermin sudah ada (transformasi), sisanya F3.
pub mod advanced {
    use anyhow::{bail, Result};
    use glam::DVec3;

    use super::super::unsupported;
    use crate::advanced::LoftSection;
    use crate::shape::KernelShape;

    pub fn loft_sections(_sections: &[LoftSection]) -> Result<KernelShape> {
        unsupported("loft_sections")
    }

    /// Cerminkan shape terhadap bidang (`point`, `normal`): refleksi
    /// Householder `I − 2nnᵀ` (determinan negatif ditangani SolidCraft
    /// dengan membalik orientasi face).
    pub fn mirror_shape(shape: &KernelShape, point: DVec3, normal: DVec3) -> Result<KernelShape> {
        let n = normal.normalize_or_zero();
        if n == DVec3::ZERO {
            bail!("normal bidang cermin tidak boleh nol");
        }
        let r = |i: usize, j: usize| f64::from(u8::from(i == j)) - 2.0 * n[i] * n[j];
        let t = n * (2.0 * point.dot(n));
        let m = [
            [r(0, 0), r(0, 1), r(0, 2), 0.0],
            [r(1, 0), r(1, 1), r(1, 2), 0.0],
            [r(2, 0), r(2, 1), r(2, 2), 0.0],
            [t.x, t.y, t.z, 1.0],
        ];
        super::super::shape::apply_matrix(shape, m, "Mirror")
    }

    pub fn draft_faces_by_index(
        _shape: &KernelShape,
        _faces: &[usize],
        _neutral_point: DVec3,
        _neutral_normal: DVec3,
        _pull_direction: DVec3,
        _angle_deg: f64,
    ) -> Result<KernelShape> {
        unsupported("draft_faces_by_index")
    }

    pub fn fillet_edges_variable_by_index(
        _shape: &KernelShape,
        _radius_start: f64,
        _radius_end: f64,
        _edges: &[usize],
    ) -> Result<KernelShape> {
        unsupported("fillet_edges_variable_by_index")
    }
}

/// `crate::helix` — solid helix F3 (`sweep_helix` SolidCraft).
pub mod helix {
    use anyhow::Result;

    use super::super::unsupported;
    use crate::helix::HelixParams;
    use crate::profile::Profile;
    use crate::shape::KernelShape;

    pub fn create_helix_solid_with_custom_profile(
        _params: &HelixParams,
        _profile: &Profile,
        _samples_per_turn: usize,
    ) -> Result<KernelShape> {
        unsupported("create_helix_solid_with_custom_profile")
    }
}

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
