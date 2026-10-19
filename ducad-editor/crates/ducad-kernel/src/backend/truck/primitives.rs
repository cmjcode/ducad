//! Primitif solid (box, silinder, bola, kerucut) backend truck. Semua dibuat
//! di sekitar origin dengan ukuran & pesan galat yang sama dengan backend
//! OCCT; penempatan dilakukan pemanggil lewat `translate_shape`/`transform_shape`.

use anyhow::{bail, Result};
use solidcraft_geom::{Loop2, Plane, Region2, Vec2, Vec3};

use super::kernel;
use crate::shape::KernelShape;

fn ensure_positive(name: &str, value: f64) -> Result<()> {
    if value.is_nan() || value <= 0.0 || !value.is_finite() {
        bail!("{name} harus > 0 (diberikan {value})");
    }
    Ok(())
}

/// Box dengan sudut minimum di origin (`centered = false`) atau berpusat
/// di origin (`true`).
pub fn make_box(width_x: f64, depth_y: f64, height_z: f64, centered: bool) -> Result<KernelShape> {
    ensure_positive("lebar box (X)", width_x)?;
    ensure_positive("kedalaman box (Y)", depth_y)?;
    ensure_positive("tinggi box (Z)", height_z)?;
    let size = Vec3::new(width_x, depth_y, height_z);
    let lo = if centered { size * -0.5 } else { Vec3::ZERO };
    kernel("make_box", || solidcraft_kernel::box_solid(lo, lo + size)).map(KernelShape::from_body)
}

/// Silinder: alas berpusat di origin, sumbu +Z.
pub fn make_cylinder(radius: f64, height: f64) -> Result<KernelShape> {
    ensure_positive("radius silinder", radius)?;
    ensure_positive("tinggi silinder", height)?;
    kernel("make_cylinder", || {
        solidcraft_kernel::cylinder(Vec3::ZERO, Vec3::Z, radius, height)
    })
    .map(KernelShape::from_body)
}

/// Bola berpusat di origin.
pub fn make_sphere(radius: f64) -> Result<KernelShape> {
    ensure_positive("radius bola", radius)?;
    kernel("make_sphere", || {
        solidcraft_kernel::sphere(Vec3::ZERO, radius)
    })
    .map(KernelShape::from_body)
}

/// Kerucut terpancung: alas `r_bottom` di z=0, `r_top` di z=`height`.
/// `r_top` boleh 0 (kerucut lancip). Dibangun sebagai revolusi penuh profil
/// trapesium/segitiga di bidang XZ mengelilingi sumbu Z.
pub fn make_cone(r_bottom: f64, r_top: f64, height: f64) -> Result<KernelShape> {
    ensure_positive("radius alas kerucut", r_bottom)?;
    ensure_positive("tinggi kerucut", height)?;
    if r_top.is_nan() || r_top < 0.0 || !r_top.is_finite() {
        bail!("radius atas kerucut harus >= 0 (diberikan {r_top})");
    }
    // Sketsa x = +X dunia, y = +Z dunia; sumbu revolusi = sumbu y sketsa.
    let plane = Plane {
        origin: Vec3::ZERO,
        x: Vec3::X,
        y: Vec3::Z,
    };
    let mut pts = vec![Vec2::new(0.0, 0.0), Vec2::new(r_bottom, 0.0)];
    if r_top > 1e-9 {
        pts.push(Vec2::new(r_top, height));
    }
    pts.push(Vec2::new(0.0, height));
    let region = Region2 {
        outer: Loop2::polygon(&pts),
        holes: Vec::new(),
    };
    let bodies = kernel("make_cone", || {
        solidcraft_kernel::revolve_whole(
            &plane,
            &[region],
            Vec2::ZERO,
            Vec2::Y,
            std::f64::consts::TAU,
        )
    })?;
    super::csg::one_body(bodies, "make_cone")
}
