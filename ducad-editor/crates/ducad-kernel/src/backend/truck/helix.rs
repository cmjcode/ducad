//! Bagian backend truck dari `crate::helix` (PLAN_MIGRASI_TRUCK F3.4):
//! solid helix lewat `sweep_helix` SolidCraft (gerak sekrup, dinding
//! B-spline), atau sapuan jalur 3D untuk helix kerucut.

use anyhow::{anyhow, bail, Result};
use solidcraft_geom::Vec3;

use super::bridge::{region, sketch_plane};
use super::kernel;
use crate::helix::{
    compute_start_profile_plane, generate_helix_points, HelixHandedness, HelixParams,
};
use crate::profile::Profile;
use crate::shape::KernelShape;

/// Solid hasil menyapu `profile` (di bidang tegak lurus kurva di titik
/// awalnya, sumbu-u radial keluar) di sepanjang helix `params`.
pub fn create_helix_solid_with_custom_profile(
    params: &HelixParams,
    profile: &Profile,
    samples_per_turn: usize,
) -> Result<KernelShape> {
    // Validasi parameter sama dengan backend lama (≤ 0.5.x) (lewat titik helix).
    let pts = generate_helix_points(params, samples_per_turn)?;
    let (origin, u_axis, v_axis, _normal) = compute_start_profile_plane(params)?;
    let (plane, scale) = sketch_plane(origin, u_axis, v_axis)?;
    let region = region(profile, scale)?;
    let conical = (params.resolved_end_radius() - params.radius).abs() > 1e-9;
    let body = if conical {
        let path: Vec<Vec3> = pts.iter().map(|p| Vec3::new(p[0], p[1], p[2])).collect();
        kernel("Helix", || solidcraft_kernel::sweep_path(&plane, &region, &path))
    } else {
        let a = params.axis;
        let len = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
        if len < 1e-12 {
            bail!("sumbu helix tidak boleh vektor nol");
        }
        let mut axis = Vec3::new(a[0] / len, a[1] / len, a[2] / len);
        let mut pitch = params.pitch;
        // `sweep_helix` selalu tangan kanan terhadap `axis`. Ulir kiri =
        // tangan kanan terhadap −axis dengan pitch negatif (tetap naik
        // searah +axis).
        if params.handedness == HelixHandedness::LeftHand {
            axis = axis * -1.0;
            pitch = -pitch;
        }
        let o = params.origin;
        let turns = params.turns.abs();
        kernel("Helix", || {
            solidcraft_kernel::sweep_helix(
                &plane,
                &region,
                Vec3::new(o[0], o[1], o[2]),
                axis,
                pitch,
                turns,
            )
        })
    }
    .map_err(|e| {
        anyhow!("Operasi Helix Sweep gagal: pastikan profil dan parameter spiral valid ({e:#})")
    })?;
    Ok(KernelShape::from_body(body))
}
