//! Operasi lanjutan untuk agent (P14): loft lintas bidang, mirror bidang,
//! draft dan fillet variabel berbasis indeks topologi (tanpa ray picking).
//!
//! Aturan kernel: setiap fungsi publik memegang `lock_kernel()` dan tidak
//! memanggil fungsi publik kernel lain selagi memegangnya.

use anyhow::{anyhow, bail, Context, Result};
use glam::{dvec3, DVec3};
use opencascade::primitives::{IntoShape, Solid};

use crate::lock_kernel;
use crate::profile::{build_wire_on_plane, Profile};
use crate::shape::{deep_clone, validate_or_heal, KernelShape};
use crate::topo::{ordered_faces, take_by_index, unique_edges};
use crate::SurfaceKind;

/// Satu penampang loft: profil 2D pada bidang (origin, u, v, normal).
#[derive(Debug, Clone)]
pub struct LoftSection {
    pub profile: Profile,
    pub origin: [f64; 3],
    pub u_axis: [f64; 3],
    pub v_axis: [f64; 3],
    pub normal: [f64; 3],
}

/// Loft solid melewati ≥ 2 penampang berurutan (BRepOffsetAPI_ThruSections).
/// Penampang boleh di bidang mana pun, tidak harus sejajar.
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
    let _guard = lock_kernel();
    let mut wires = Vec::with_capacity(sections.len());
    for (i, s) in sections.iter().enumerate() {
        let w = build_wire_on_plane(&s.profile, s.origin, s.u_axis, s.v_axis, s.normal)
            .with_context(|| format!("penampang loft #{i} tidak bisa dibentuk"))?;
        wires.push(w);
    }
    let solid = Solid::loft(wires.iter());
    validate_or_heal(KernelShape::from_inner(solid.into_shape()), "Loft")
}

/// Cermin shape terhadap bidang (titik `point`, normal `normal`).
///
/// Binding OCCT hanya menyediakan cermin SUMBU (= rotasi 180°), jadi
/// refleksi bidang disusun dari inversi titik (skala −1 di `point`) lalu
/// rotasi 180° mengelilingi `normal`: (2nnᵀ − I)(−I) = I − 2nnᵀ.
pub fn mirror_shape(shape: &KernelShape, point: DVec3, normal: DVec3) -> Result<KernelShape> {
    if normal.length_squared() < 1e-12 || !normal.is_finite() {
        bail!("normal bidang cermin tidak boleh vektor nol");
    }
    let _guard = lock_kernel();
    let mut cloned = deep_clone(shape.inner())?;
    cloned.scale(point, -1.0);
    cloned.rotate(point, normal.normalize(), std::f64::consts::PI);
    validate_or_heal(KernelShape::from_inner(cloned), "Mirror")
}

/// Draft (kemiringan cetakan) pada face planar berindeks
/// [`crate::topo::enumerate_faces`].
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
    let _guard = lock_kernel();
    let cloned = deep_clone(shape.inner())?;
    let picked = take_by_index(ordered_faces(&cloned), faces, "face")?;
    for (k, f) in picked.iter().enumerate() {
        if SurfaceKind::from(f.surface_kind().as_str()) != SurfaceKind::Plane {
            bail!("draft hanya mendukung face planar (face ke-{k} yang dipilih tidak datar)");
        }
    }
    let refs: Vec<&opencascade::primitives::Face> = picked.iter().collect();
    let result = cloned
        .draft_angle(
            neutral_point,
            neutral_normal.normalize(),
            pull_direction.normalize(),
            angle_deg,
            &refs,
        )
        .map_err(|e| anyhow!("draft gagal: {e}"))?;
    validate_or_heal(KernelShape::from_inner(result), "Draft")
}

/// Fillet dengan radius berubah linear dari `radius_start` ke `radius_end`
/// di sepanjang tiap tepi berindeks [`crate::topo::enumerate_edges`].
pub fn fillet_edges_variable_by_index(
    shape: &KernelShape,
    radius_start: f64,
    radius_end: f64,
    edges: &[usize],
) -> Result<KernelShape> {
    if radius_start <= 0.0 || radius_end <= 0.0 {
        bail!("radius fillet variabel (awal dan akhir) harus > 0");
    }
    let _guard = lock_kernel();
    let mut cloned = deep_clone(shape.inner())?;
    let picked = take_by_index(unique_edges(&cloned), edges, "tepi")?;
    cloned
        .fillet_edges_variable(radius_start, radius_end, &picked)
        .context("radius fillet variabel terlalu besar untuk tepi terpilih")?;
    validate_or_heal(KernelShape::from_inner(cloned), "Fillet")
}

/// Titik pusat bbox kasar dari pusat massa face (untuk tes dan pivot).
pub fn faces_centroid(shape: &KernelShape) -> DVec3 {
    let faces = crate::topo::enumerate_faces(shape);
    if faces.is_empty() {
        return DVec3::ZERO;
    }
    let sum = faces.iter().fold(DVec3::ZERO, |acc, f| {
        acc + dvec3(f.centroid[0], f.centroid[1], f.centroid[2])
    });
    sum / faces.len() as f64
}
