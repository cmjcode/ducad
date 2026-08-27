//! Generator fitur lubang (Hole Wizard) dan operasi Boolean pemotongan lubang pada solid 3D.
//!
//! Menghasilkan geometri solid pemotong berpenampang silinder bertingkat/tirus/kerucut
//! sesuai standar ISO baut metrik (Counterbore, Countersink, Tapped, Simple) dan memotongkannya
//! ke dalam solid target via Boolean Subtraction.

use anyhow::{bail, Result};
use ducad_core::hole::{HoleKind, HoleSpec};
use glam::dvec3;

use crate::csg::{revolve_profile, subtract};
use crate::profile::{Profile, ProfileSegment};
use crate::shape::{transform_shape, KernelShape};

/// Buat solid B-rep pemotong lubang (*hole cutter tool*) yang sudah diposisikan
/// dan diorientasikan pada titik `position` (x, y, z) dengan vektor normal keluar `normal` (nx, ny, nz).
pub fn create_hole_cutter(
    spec: &HoleSpec,
    position: (f64, f64, f64),
    normal: (f64, f64, f64),
) -> Result<KernelShape> {
    let norm_len = (normal.0 * normal.0 + normal.1 * normal.1 + normal.2 * normal.2).sqrt();
    if norm_len < 1e-9 {
        bail!("vektor normal face tidak valid (panjang mendekati nol)");
    }
    let norm = (
        normal.0 / norm_len,
        normal.1 / norm_len,
        normal.2 / norm_len,
    );

    let r_main = (spec.diameter / 2.0).max(0.05);
    let depth = if spec.is_through {
        spec.depth.max(500.0)
    } else {
        spec.depth.max(0.1)
    };

    // Ekstensi aman ke atas permukaan (z_top > 0) agar Boolean cut tidak coplanar pada face masuk
    let z_top = 0.5_f64;

    let mut pts: Vec<(f64, f64)> = Vec::new();

    match spec.kind {
        HoleKind::Simple => {
            pts.push((0.0, z_top));
            pts.push((r_main, z_top));
            pts.push((r_main, -depth));
            if spec.has_drill_tip && !spec.is_through {
                let tip_h = r_main / 59.0_f64.to_radians().tan();
                pts.push((0.0, -depth - tip_h));
            } else {
                pts.push((0.0, -depth));
            }
        }
        HoleKind::Counterbore => {
            let r_cb = (spec.counterbore_diameter / 2.0).max(r_main + 0.05);
            let t_cb = spec.counterbore_depth.clamp(0.01, (depth - 0.01).max(0.02));
            pts.push((0.0, z_top));
            pts.push((r_cb, z_top));
            pts.push((r_cb, -t_cb));
            pts.push((r_main, -t_cb));
            pts.push((r_main, -depth));
            if spec.has_drill_tip && !spec.is_through {
                let tip_h = r_main / 59.0_f64.to_radians().tan();
                pts.push((0.0, -depth - tip_h));
            } else {
                pts.push((0.0, -depth));
            }
        }
        HoleKind::Countersink => {
            let r_cs = (spec.countersink_diameter / 2.0).max(r_main + 0.05);
            let angle_rad = (spec.countersink_angle_deg / 2.0)
                .to_radians()
                .clamp(10.0_f64.to_radians(), 85.0_f64.to_radians());
            let h_cs = (r_cs - r_main) / angle_rad.tan();
            let h_cs = h_cs.min((depth - 0.01).max(0.02));
            let r_top = r_cs + z_top * angle_rad.tan();

            pts.push((0.0, z_top));
            pts.push((r_top, z_top));
            pts.push((r_cs, 0.0));
            pts.push((r_main, -h_cs));
            pts.push((r_main, -depth));
            if spec.has_drill_tip && !spec.is_through {
                let tip_h = r_main / 59.0_f64.to_radians().tan();
                pts.push((0.0, -depth - tip_h));
            } else {
                pts.push((0.0, -depth));
            }
        }
        HoleKind::Tapped => {
            let (d_nom, _, _, _, _, _, _) = spec.thread_size.standard_params();
            let r_nom = (d_nom / 2.0).max(r_main);
            let chamfer_h = (r_nom - r_main).max(0.0);
            let r_top = r_nom + z_top;

            pts.push((0.0, z_top));
            pts.push((r_top, z_top));
            pts.push((r_nom, 0.0));
            if chamfer_h > 1e-4 {
                pts.push((r_main, -chamfer_h));
            }
            pts.push((r_main, -depth));
            if spec.has_drill_tip && !spec.is_through {
                let tip_h = r_main / 59.0_f64.to_radians().tan();
                pts.push((0.0, -depth - tip_h));
            } else {
                pts.push((0.0, -depth));
            }
        }
    }

    if pts.len() < 3 {
        bail!("titik profil lubang tidak mencukupi");
    }

    let mut segments = Vec::with_capacity(pts.len());
    for i in 0..pts.len() {
        let next_i = (i + 1) % pts.len();
        segments.push(ProfileSegment::Line {
            start: pts[i],
            end: pts[next_i],
        });
    }

    let profile = Profile::Loop(segments);

    // Revolve profil sekeliling sumbu Y (X=0) 360°
    let raw_cutter = revolve_profile(&profile, (0.0, 0.0), (0.0, 1.0), None)?;

    // Arah default cutter di ruang lokal adalah sepanjang sumbu -Y: (0, -1, 0)
    let u_local = dvec3(0.0, -1.0, 0.0);
    // Arah target pengeboran adalah masuk ke dalam bodi: -norm
    let v_target = dvec3(-norm.0, -norm.1, -norm.2);

    let dot = u_local.dot(v_target).clamp(-1.0, 1.0);
    let cross = u_local.cross(v_target);
    let angle_rad = dot.acos();

    let (rot_axis, rot_angle) = if angle_rad.abs() < 1e-6 {
        ((0.0, 1.0, 0.0), 0.0)
    } else if (angle_rad - std::f64::consts::PI).abs() < 1e-6 {
        ((1.0, 0.0, 0.0), std::f64::consts::PI)
    } else {
        let norm_cross = cross.normalize();
        ((norm_cross.x, norm_cross.y, norm_cross.z), angle_rad)
    };

    let transformed_cutter = transform_shape(
        &raw_cutter,
        position,
        (0.0, 0.0, 0.0),
        rot_axis,
        rot_angle,
    )?;

    Ok(transformed_cutter)
}

/// Terapkan operasi pembuatan lubang (*Hole Wizard*) pada solid `target_shape`.
/// Memotong geometri solid menggunakan Boolean Subtraction.
pub fn apply_hole(
    target_shape: &KernelShape,
    spec: &HoleSpec,
    position: (f64, f64, f64),
    normal: (f64, f64, f64),
) -> Result<KernelShape> {
    let cutter = create_hole_cutter(spec, position, normal)?;
    subtract(target_shape, &cutter)
}
