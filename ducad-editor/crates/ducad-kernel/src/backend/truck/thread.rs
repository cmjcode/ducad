//! Ulir ISO di backend truck (PLAN_MIGRASI_TRUCK §4.2 pilihan A): alur
//! disapu sebagai MESH helix tertutup (`sweep_helix_mesh`, 48 stasiun per
//! putaran) lalu dipotong dengan `faceted_boolean`. Hasilnya B-rep berface
//! planar (permukaan lengkung di-facet) — tetap solid valid dan bisa
//! diekspor STEP. Boolean B-rep eksak dari alur helix B-spline terlalu
//! lambat di truck (risiko R4).

use anyhow::{anyhow, bail, Context, Result};
use solidcraft_geom::{Plane, Vec2, Vec3};
use solidcraft_kernel::BoolOp;

use super::kernel;
use crate::shape::KernelShape;

/// Stasiun penampang per putaran (angka SolidCraft untuk ulir bermodel).
const STATIONS_PER_TURN: usize = 48;

/// Potong alur ulir. `profile` = titik penampang alur (u radial keluar, 0 di
/// diameter mayor; v aksial), cembung, di bidang aksial pada sudut awal.
#[allow(clippy::too_many_arguments)]
pub(crate) fn cut_faceted(
    shape: &KernelShape,
    axis_origin: [f64; 3],
    axis_dir: [f64; 3],
    major_d: f64,
    pitch: f64,
    length: f64,
    left_handed: bool,
    profile: &[(f64, f64)],
) -> Result<KernelShape> {
    let body = shape.require_body("Thread")?;
    let a = Vec3::new(axis_dir[0], axis_dir[1], axis_dir[2]);
    let k = a.normalized().ok_or_else(|| anyhow!("arah sumbu ulir nol"))?;
    let o = Vec3::new(axis_origin[0], axis_origin[1], axis_origin[2]);
    let radial = k.any_perp();
    // Bidang aksial: x = radial keluar, y = sumbu; origin di diameter mayor.
    let plane = Plane::new(o + radial * (major_d / 2.0), radial, k)
        .ok_or_else(|| anyhow!("bidang profil ulir tidak valid"))?;
    let polygon: Vec<Vec2> = profile.iter().map(|&(u, v)| Vec2::new(u, v)).collect();
    let turns = length / pitch;
    if turns > 200.0 {
        bail!("ulir {turns:.0} putaran melebihi batas 200 putaran backend truck");
    }
    // `sweep_helix_mesh` selalu tangan kanan terhadap sumbunya; ulir kiri =
    // tangan kanan terhadap −k dengan pitch negatif (tetap maju searah +k).
    let (axis, p) = if left_handed { (k * -1.0, -pitch) } else { (k, pitch) };
    let groove = kernel("Thread", || {
        solidcraft_kernel::sweep_helix_mesh(&plane, &polygon, o, axis, p, turns, STATIONS_PER_TURN)
    })
    .context("gagal membentuk alur ulir")?;
    let tol = (major_d * 1e-3).clamp(1e-3, 0.02);
    let out = kernel("Thread", || {
        solidcraft_kernel::faceted_boolean(body, &groove, BoolOp::Cut, tol)
    })
    .context("gagal memotong alur ulir")?
    .ok_or_else(|| anyhow!("pemotongan ulir menghabiskan seluruh body"))?;
    log::info!("ulir dipotong sebagai facet ({STATIONS_PER_TURN} stasiun/putaran)");
    Ok(KernelShape::from_body(out))
}
