//! Bagian backend truck dari `crate::advanced` (PLAN_MIGRASI_TRUCK F3.3):
//! loft lintas bidang, cermin, draft & fillet variabel berbasis indeks.

use anyhow::{bail, Context, Result};
use glam::DVec3;

use super::bridge::{loop2, sketch_plane};
use super::kernel;
use crate::advanced::LoftSection;
use crate::picking::face::SurfaceKind;
use crate::profile::Profile;
use crate::shape::KernelShape;

/// Loft solid melewati ≥ 2 penampang berurutan (bidang boleh tidak sejajar).
pub fn loft_sections(sections: &[LoftSection]) -> Result<KernelShape> {
    if sections.len() < 2 {
        bail!(
            "loft butuh minimal 2 penampang (diberikan {})",
            sections.len()
        );
    }
    if let Some(i) = sections
        .iter()
        .position(|s| matches!(s.profile, Profile::WithHoles { .. }))
    {
        bail!("penampang loft #{i} berlubang; loft hanya menerima satu loop luar per penampang");
    }
    let mut secs = Vec::with_capacity(sections.len());
    for (i, s) in sections.iter().enumerate() {
        let (plane, scale) = sketch_plane(s.origin, s.u_axis, s.v_axis)
            .with_context(|| format!("penampang loft #{i} tidak bisa dibentuk"))?;
        let lp = loop2(&s.profile, scale)
            .with_context(|| format!("penampang loft #{i} tidak bisa dibentuk"))?;
        secs.push((plane, lp));
    }
    let body = kernel("Loft", || solidcraft_kernel::loft(&secs))?;
    super::modify::checked(body, "Loft")
}

/// Cerminkan shape terhadap bidang (`point`, `normal`): refleksi
/// Householder `I − 2nnᵀ` (determinan negatif ditangani SolidCraft dengan
/// membalik orientasi face).
pub fn mirror_shape(shape: &KernelShape, point: DVec3, normal: DVec3) -> Result<KernelShape> {
    if normal.length_squared() < 1e-12 || !normal.is_finite() {
        bail!("normal bidang cermin tidak boleh vektor nol");
    }
    let n = normal.normalize();
    let r = |i: usize, j: usize| f64::from(u8::from(i == j)) - 2.0 * n[i] * n[j];
    let t = n * (2.0 * point.dot(n));
    let m = [
        [r(0, 0), r(0, 1), r(0, 2), 0.0],
        [r(1, 0), r(1, 1), r(1, 2), 0.0],
        [r(2, 0), r(2, 1), r(2, 2), 0.0],
        [t.x, t.y, t.z, 1.0],
    ];
    super::shape::apply_matrix(shape, m, "Mirror")
}

/// Draft pada face planar berindeks [`crate::topo::enumerate_faces`].
pub fn draft_faces_by_index(
    shape: &KernelShape,
    faces: &[usize],
    neutral_point: DVec3,
    neutral_normal: DVec3,
    pull_direction: DVec3,
    angle_deg: f64,
) -> Result<KernelShape> {
    if !(angle_deg > 0.0 && angle_deg < 90.0) {
        bail!("sudut draft harus antara 0° dan 90° (eksklusif); diberikan {angle_deg:.3}°");
    }
    if pull_direction.length_squared() < 1e-12 || neutral_normal.length_squared() < 1e-12 {
        bail!("arah tarik dan normal bidang netral tidak boleh vektor nol");
    }
    let topo = shape.topology();
    for (k, &f) in faces.iter().enumerate() {
        let Some(face) = topo.faces.get(f) else {
            bail!(
                "indeks face {f} di luar jangkauan (shape punya {} face)",
                topo.faces.len()
            );
        };
        if face.kind != SurfaceKind::Plane {
            bail!("draft hanya mendukung face planar (face ke-{k} yang dipilih tidak datar)");
        }
    }
    super::modify::draft_logical(
        shape,
        faces,
        neutral_point,
        neutral_normal,
        pull_direction,
        angle_deg,
    )
    .map_err(|e| anyhow::anyhow!("draft gagal: {e:#}"))
}

/// Fillet radius berubah linear `radius_start` → `radius_end` di sepanjang
/// tiap tepi berindeks [`crate::topo::enumerate_edges`].
pub fn fillet_edges_variable_by_index(
    shape: &KernelShape,
    radius_start: f64,
    radius_end: f64,
    edges: &[usize],
) -> Result<KernelShape> {
    super::modify::fillet_variable_logical(shape, radius_start, radius_end, edges)
}
