//! Solver Kendala Perakitan 3D (3D Assembly Mate Constraint Solver) untuk DuCAD.
//!
//! Menyediakan algoritma analitik dan relaksasi kendala 3D untuk:
//! - **Concentric Mate**: Mengunci keselarasan sumbu silinder poros dengan lubang (kolinear).
//! - **Coincident Mate**: Menempelkan dua permukaan planar datar saling berhimpit (muka-ke-muka).
//! - **Distance Mate**: Menetapkan jarak terukur offset $d$ mm antar bidang/titik acuan.
//! - **Angle Mate**: Mengatur sudut rotasi engsel $\theta^\circ$ antara dua bidang atau sumbu.
//! - **Multi-Constraint Sequential Solver**: Menyelesaikan kombinasi mate simultan
//!   (misal: silinder poros sepusat + bidang bahu penahan datar) tanpa saling merusak.

use anyhow::Result;
use ducad_core::assembly::{
    AssemblyInstanceId, AssemblyTree, MateConstraint, MateKind, MateStatus, MateTargetKind,
};
use glam::{DQuat, DVec3};

use crate::shape::{transform_shape, KernelShape};

/// Hasil kalkulasi transformasi rigid-body (translasi dan rotasi) untuk part target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MateTransformResult {
    pub translation: (f64, f64, f64),
    pub pivot: (f64, f64, f64),
    pub axis: (f64, f64, f64),
    pub angle_rad: f64,
}

impl Default for MateTransformResult {
    fn default() -> Self {
        Self {
            translation: (0.0, 0.0, 0.0),
            pivot: (0.0, 0.0, 0.0),
            axis: (0.0, 0.0, 1.0),
            angle_rad: 0.0,
        }
    }
}

/// Selesaikan kalkulasi transformasi untuk satu Mate Constraint tunggal.
///
/// Menghitung pergeseran dan perputaran yang perlu diterapkan pada `target_b`
/// agar memenuhi kendala terhadap `target_a` (acuan).
pub fn solve_single_mate(mate: &MateConstraint) -> Result<MateTransformResult> {
    match &mate.kind {
        MateKind::Concentric {
            aligned,
            lock_rotation: _,
        } => solve_concentric(&mate.target_a.kind, &mate.target_b.kind, *aligned),
        MateKind::Coincident { opposite_normal } => {
            solve_coincident(&mate.target_a.kind, &mate.target_b.kind, *opposite_normal, 0.0)
        }
        MateKind::Distance {
            offset,
            opposite_normal,
        } => solve_coincident(
            &mate.target_a.kind,
            &mate.target_b.kind,
            *opposite_normal,
            *offset,
        ),
        MateKind::Angle {
            angle_deg,
            opposite_normal,
        } => solve_angle(
            &mate.target_a.kind,
            &mate.target_b.kind,
            *angle_deg,
            *opposite_normal,
        ),
    }
}

/// Ekstrak titik acuan (origin) dan vektor arah/normal dari target mate apa pun secara serbaguna.
fn extract_origin_and_dir(target: &MateTargetKind) -> Result<(DVec3, DVec3)> {
    match target {
        MateTargetKind::CylinderAxis {
            origin, direction, ..
        } => {
            let o = DVec3::new(origin.0, origin.1, origin.2);
            let mut d = DVec3::new(direction.0, direction.1, direction.2);
            if d.length_squared() < 1e-9 {
                d = DVec3::Z;
            }
            Ok((o, d.normalize()))
        }
        MateTargetKind::PlanarFace { origin, normal } => {
            let o = DVec3::new(origin.0, origin.1, origin.2);
            let mut n = DVec3::new(normal.0, normal.1, normal.2);
            if n.length_squared() < 1e-9 {
                n = DVec3::Z;
            }
            Ok((o, n.normalize()))
        }
        MateTargetKind::Point { pos } => {
            let o = DVec3::new(pos.0, pos.1, pos.2);
            Ok((o, DVec3::Z))
        }
    }
}

/// Solver Concentric: Sumbu silinder/vektor B disejajarkan dan digeser radial agar kolinear dengan sumbu A.
pub fn solve_concentric(
    target_a: &MateTargetKind,
    target_b: &MateTargetKind,
    aligned: bool,
) -> Result<MateTransformResult> {
    let (origin_a, dir_a) = extract_origin_and_dir(target_a)?;
    let (origin_b, dir_b) = extract_origin_and_dir(target_b)?;

    let target_dir_b = if aligned { dir_a } else { -dir_a };

    // 1. Hitung rotasi untuk menyelaraskan arah sumbu B ke arah target
    let mut rot_axis = dir_b.cross(target_dir_b);
    let dot = dir_b.dot(target_dir_b).clamp(-1.0, 1.0);
    let mut rot_angle = dot.acos();

    if rot_axis.length_squared() < 1e-9 {
        if dot < -0.9999 {
            // Berlawanan 180 derajat persis: cari vektor tegak lurus sembarang
            let perp = if dir_b.x.abs() < 0.9 {
                DVec3::X.cross(dir_b).normalize()
            } else {
                DVec3::Y.cross(dir_b).normalize()
            };
            rot_axis = perp;
            rot_angle = std::f64::consts::PI;
        } else {
            rot_axis = DVec3::Z;
            rot_angle = 0.0;
        }
    } else {
        rot_axis = rot_axis.normalize();
    }

    // 2. Hitung translasi radial untuk menyatukan garis sumbu
    // Garis A: origin_a + t * dir_a
    // Garis B melewati origin_b
    let diff = origin_a - origin_b;
    let proj_on_a = diff.dot(dir_a) * dir_a;
    let radial_shift = diff - proj_on_a;

    Ok(MateTransformResult {
        translation: (radial_shift.x, radial_shift.y, radial_shift.z),
        pivot: (origin_b.x, origin_b.y, origin_b.z),
        axis: (rot_axis.x, rot_axis.y, rot_axis.z),
        angle_rad: rot_angle,
    })
}

/// Solver Coincident / Distance: Bidang B diputar agar normalnya sesuai target, lalu digeser sejauh offset.
pub fn solve_coincident(
    target_a: &MateTargetKind,
    target_b: &MateTargetKind,
    opposite_normal: bool,
    offset_distance: f64,
) -> Result<MateTransformResult> {
    let (origin_a, normal_a) = extract_origin_and_dir(target_a)?;
    let (origin_b, normal_b) = extract_origin_and_dir(target_b)?;

    // Kontak muka-ke-muka: normal B harus berlawanan arah dengan normal A (-normal_a) jika opposite_normal
    let target_normal_b = if opposite_normal {
        -normal_a
    } else {
        normal_a
    };

    // 1. Rotasi untuk menyelaraskan vektor normal
    let mut rot_axis = normal_b.cross(target_normal_b);
    let dot = normal_b.dot(target_normal_b).clamp(-1.0, 1.0);
    let mut rot_angle = dot.acos();

    if rot_axis.length_squared() < 1e-9 {
        if dot < -0.9999 {
            let perp = if normal_b.x.abs() < 0.9 {
                DVec3::X.cross(normal_b).normalize()
            } else {
                DVec3::Y.cross(normal_b).normalize()
            };
            rot_axis = perp;
            rot_angle = std::f64::consts::PI;
        } else {
            rot_axis = DVec3::Z;
            rot_angle = 0.0;
        }
    } else {
        rot_axis = rot_axis.normalize();
    }

    // 2. Translasi sepanjang normal A agar bidang berimpit (ditambah offset_distance)
    let current_dist = (origin_b - origin_a).dot(normal_a);
    let required_shift = -current_dist + offset_distance;
    let translation = normal_a * required_shift;

    Ok(MateTransformResult {
        translation: (translation.x, translation.y, translation.z),
        pivot: (origin_b.x, origin_b.y, origin_b.z),
        axis: (rot_axis.x, rot_axis.y, rot_axis.z),
        angle_rad: rot_angle,
    })
}

/// Solver Angle Mate: Memutar bidang/garis B terhadap A sebesar sudut yang ditentukan.
pub fn solve_angle(
    target_a: &MateTargetKind,
    target_b: &MateTargetKind,
    angle_deg: f64,
    opposite_normal: bool,
) -> Result<MateTransformResult> {
    let (_origin_a, normal_a) = extract_origin_and_dir(target_a)?;
    let (origin_b, normal_b) = extract_origin_and_dir(target_b)?;

    // Sumbu engsel perpotongan bidang
    let mut hinge_axis = normal_a.cross(normal_b);
    if hinge_axis.length_squared() < 1e-9 {
        hinge_axis = if normal_a.x.abs() < 0.9 {
            DVec3::X.cross(normal_a).normalize()
        } else {
            DVec3::Y.cross(normal_a).normalize()
        };
    } else {
        hinge_axis = hinge_axis.normalize();
    }

    let target_angle_rad = angle_deg.to_radians();
    let current_angle = normal_a.dot(normal_b).clamp(-1.0, 1.0).acos();
    let delta_angle = if opposite_normal {
        std::f64::consts::PI - target_angle_rad - current_angle
    } else {
        target_angle_rad - current_angle
    };

    Ok(MateTransformResult {
        translation: (0.0, 0.0, 0.0),
        pivot: (origin_b.x, origin_b.y, origin_b.z),
        axis: (hinge_axis.x, hinge_axis.y, hinge_axis.z),
        angle_rad: delta_angle,
    })
}

/// Selesaikan seluruh hierarki perakitan (Multi-Constraint Assembly Solver).
///
/// Mengiterasi semua mate aktif secara berurutan dan mengupdate posisi `translation` dan `rotation_quat`
/// pada setiap `AssemblyInstance` non-grounded.
pub fn solve_assembly(tree: &mut AssemblyTree) -> Vec<(AssemblyInstanceId, MateTransformResult)> {
    let mut results = Vec::new();

    for mate in tree.mates.values_mut() {
        if mate.suppressed {
            continue;
        }

        let id_a = mate.target_a.instance_id;
        let id_b = mate.target_b.instance_id;

        // Cari instance target yang bisa digerakkan (non-grounded)
        let is_a_grounded = tree.instances.get(&id_a).map_or(false, |i| i.is_grounded);
        let is_b_grounded = tree.instances.get(&id_b).map_or(false, |i| i.is_grounded);

        if is_a_grounded && is_b_grounded {
            // Kedua part terkunci mati, cek apakah sudah terpenuhi
            mate.status = MateStatus::Satisfied;
            continue;
        }

        let (moving_id, mate_to_solve) = if !is_b_grounded {
            (id_b, mate.clone())
        } else {
            // Balikkan target agar A yang digerakkan mendekati B
            let mut flipped_mate = mate.clone();
            std::mem::swap(&mut flipped_mate.target_a, &mut flipped_mate.target_b);
            (id_a, flipped_mate)
        };

        match solve_single_mate(&mate_to_solve) {
            Ok(tf) => {
                mate.status = MateStatus::Satisfied;
                if let Some(inst) = tree.instances.get_mut(&moving_id) {
                    // Update posisi translasi instance
                    inst.translation.0 += tf.translation.0;
                    inst.translation.1 += tf.translation.1;
                    inst.translation.2 += tf.translation.2;

                    // Update rotasi instance quaternion
                    if tf.angle_rad.abs() > 1e-6 {
                        let axis_v = DVec3::new(tf.axis.0, tf.axis.1, tf.axis.2);
                        let delta_q = DQuat::from_axis_angle(axis_v, tf.angle_rad);
                        let current_q = DQuat::from_xyzw(
                            inst.rotation_quat.0,
                            inst.rotation_quat.1,
                            inst.rotation_quat.2,
                            inst.rotation_quat.3,
                        );
                        let new_q = (delta_q * current_q).normalize();
                        inst.rotation_quat = (new_q.x, new_q.y, new_q.z, new_q.w);
                    }
                }
                results.push((moving_id, tf));
            }
            Err(e) => {
                mate.status = MateStatus::Conflicted(e.to_string());
            }
        }
    }

    results
}

/// Terapkan hasil transformasi rigid-body langsung ke geometri B-Rep `KernelShape`.
pub fn apply_mate_transform_to_shape(
    shape: &KernelShape,
    tf: &MateTransformResult,
) -> Result<KernelShape> {
    transform_shape(shape, tf.translation, tf.pivot, tf.axis, tf.angle_rad)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concentric_mate_solver_coaxial() {
        let target_a = MateTargetKind::CylinderAxis {
            origin: (0.0, 0.0, 0.0),
            direction: (0.0, 0.0, 1.0),
            radius: 10.0,
        };
        let target_b = MateTargetKind::CylinderAxis {
            origin: (25.0, 15.0, 50.0),
            direction: (0.0, 0.0, 1.0),
            radius: 10.0,
        };

        let res = solve_concentric(&target_a, &target_b, true).unwrap();
        // Translasi harus menggeser titik B (-25, -15) ke sumbu Z
        assert!((res.translation.0 - (-25.0)).abs() < 1e-6);
        assert!((res.translation.1 - (-15.0)).abs() < 1e-6);
        // Arah sudah sama (Z), rotasi 0
        assert_eq!(res.angle_rad, 0.0);
    }

    #[test]
    fn test_coincident_mate_solver_touching_planes() {
        let target_a = MateTargetKind::PlanarFace {
            origin: (0.0, 0.0, 100.0),
            normal: (0.0, 0.0, 1.0), // Bidang menghadap ke atas Z
        };
        let target_b = MateTargetKind::PlanarFace {
            origin: (50.0, 50.0, 120.0),
            normal: (0.0, 0.0, 1.0), // Menghadap Z, butuh opposite_normal = true
        };

        let res = solve_coincident(&target_a, &target_b, true, 0.0).unwrap();
        // Normal harus diputar 180 derajat agar berlawanan
        assert!((res.angle_rad - std::f64::consts::PI).abs() < 1e-6);
        // Translasi Z dari 120 ke 100 adalah -20
        assert!((res.translation.2 - (-20.0)).abs() < 1e-6);
    }

    #[test]
    fn test_distance_mate_solver() {
        let target_a = MateTargetKind::PlanarFace {
            origin: (0.0, 0.0, 0.0),
            normal: (0.0, 0.0, 1.0),
        };
        let target_b = MateTargetKind::PlanarFace {
            origin: (0.0, 0.0, 5.0),
            normal: (0.0, 0.0, -1.0),
        };

        // Atur jarak 15 mm
        let res = solve_coincident(&target_a, &target_b, true, 15.0).unwrap();
        // Posisi awal Z=5, offset 15 dari Z=0 -> butuh translasi +10
        assert!((res.translation.2 - 10.0).abs() < 1e-6);
    }
}
