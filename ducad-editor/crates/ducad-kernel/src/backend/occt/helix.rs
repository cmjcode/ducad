//! Bagian backend OCCT dari `crate::helix` (PLAN_MIGRASI_TRUCK F2.2): kode di
//! sini menyentuh `opencascade`; tipe & logika netral-backend ada di modul induk.

// Tipe & helper netral-backend dari modul induk.
#[allow(unused_imports)]
use crate::helix::*;

use anyhow::{anyhow, bail, Result};
use glam::dvec3;
use opencascade::primitives::{Edge, Face, Wire};
use crate::lock_kernel;
use crate::profile::{build_wire_on_plane, Profile};
use crate::shape::KernelShape;

/// Menghasilkan OpenCASCADE Wire dari kurva spiral / helix 3D.
pub fn create_helix_wire(params: &HelixParams, samples_per_turn: usize) -> Result<Wire> {
    let pts = generate_helix_points(params, samples_per_turn)?;
    if pts.len() < 2 {
        bail!("Titik helix tidak mencukupi untuk membuat Wire");
    }
    let edges: Vec<Edge> = pts
        .windows(2)
        .map(|w| {
            Edge::segment(
                dvec3(w[0][0], w[0][1], w[0][2]),
                dvec3(w[1][0], w[1][1], w[1][2]),
            )
        })
        .collect();

    Ok(Wire::from_edges(edges.iter()))
}

/// Kurva helix sebagai satu edge B-spline yang melewati titik sampel
/// (mulus, cocok sebagai spine sweep).
pub(crate) fn create_helix_spline_wire(params: &HelixParams, samples_per_turn: usize) -> Result<Wire> {
    let pts = generate_helix_points(params, samples_per_turn.max(16))?;
    if pts.len() < 3 {
        bail!("Titik helix tidak mencukupi untuk membuat kurva");
    }
    let edge = Edge::spline_from_points(pts.iter().map(|p| dvec3(p[0], p[1], p[2])), None);
    Ok(Wire::from_edges([&edge]))
}
/// Menghasilkan bentuk solid B-Rep 3D dengan menyapu profil kustom di sepanjang kurva helix.
pub fn create_helix_solid_with_custom_profile(
    params: &HelixParams,
    profile: &Profile,
    samples_per_turn: usize,
) -> Result<KernelShape> {
    let _guard = lock_kernel();
    // Spine = SATU kurva B-spline interpolasi, bukan rangkaian segmen garis:
    // MakePipe di sepanjang polyline menghasilkan solid rusak (volume ≈ 0,
    // lihat tes `helix_spring_has_real_volume`).
    let spine_wire = create_helix_spline_wire(params, samples_per_turn)?;
    let (origin, u_axis, v_axis, normal) = compute_start_profile_plane(params)?;

    let profile_wire = build_wire_on_plane(profile, origin, u_axis, v_axis, normal)?;
    let profile_face = Face::from_wire(&profile_wire);

    let shape = profile_face
        .pipe(&spine_wire)
        .map_err(|e| anyhow!("Operasi Helix Sweep gagal: pastikan profil dan parameter spiral valid ({e})"))?;

    Ok(KernelShape::from_inner(shape))
}
