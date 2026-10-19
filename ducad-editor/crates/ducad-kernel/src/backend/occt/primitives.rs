//! Primitif solid (box, silinder, bola, kerucut) — pembungkus tipis fungsi
//! `opencascade-rs` yang sudah ada (`Shape::box_*`, `cylinder_*`,
//! `sphere`, `cone`). Semua dibuat di sekitar origin; penempatan di posisi
//! lain dilakukan pemanggil lewat `translate_shape`/`transform_shape`.

use anyhow::{bail, Result};
use opencascade::primitives::Shape;

use crate::lock_kernel;
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
    let _guard = lock_kernel();
    let shape = if centered {
        Shape::box_centered(width_x, depth_y, height_z)
    } else {
        Shape::box_with_dimensions(width_x, depth_y, height_z)
    };
    Ok(KernelShape::from_inner(shape))
}

/// Silinder: alas berpusat di origin, sumbu +Z.
pub fn make_cylinder(radius: f64, height: f64) -> Result<KernelShape> {
    ensure_positive("radius silinder", radius)?;
    ensure_positive("tinggi silinder", height)?;
    let _guard = lock_kernel();
    Ok(KernelShape::from_inner(Shape::cylinder_radius_height(
        radius, height,
    )))
}

/// Bola berpusat di origin.
pub fn make_sphere(radius: f64) -> Result<KernelShape> {
    ensure_positive("radius bola", radius)?;
    let _guard = lock_kernel();
    Ok(KernelShape::from_inner(Shape::sphere(radius).build()))
}

/// Kerucut terpancung: alas `r_bottom` di z=0, `r_top` di z=`height`.
/// `r_top` boleh 0 (kerucut lancip).
pub fn make_cone(r_bottom: f64, r_top: f64, height: f64) -> Result<KernelShape> {
    ensure_positive("radius alas kerucut", r_bottom)?;
    ensure_positive("tinggi kerucut", height)?;
    if r_top.is_nan() || r_top < 0.0 || !r_top.is_finite() {
        bail!("radius atas kerucut harus >= 0 (diberikan {r_top})");
    }
    let _guard = lock_kernel();
    let shape = Shape::cone()
        .bottom_radius(r_bottom)
        .top_radius(r_top)
        .height(height)
        .build();
    Ok(KernelShape::from_inner(shape))
}
