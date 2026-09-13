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
    AssemblyInstanceId, AssemblyTree, MateConstraint, MateConstraintId, MateKind, MateStatus,
    MateTargetKind,
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
/// Transformasi rigid-body satu instance saat ini.
fn instance_pose(inst: &ducad_core::assembly::AssemblyInstance) -> (DVec3, DQuat) {
    let t = DVec3::new(inst.translation.0, inst.translation.1, inst.translation.2);
    let q = DQuat::from_xyzw(
        inst.rotation_quat.0,
        inst.rotation_quat.1,
        inst.rotation_quat.2,
        inst.rotation_quat.3,
    );
    let q = if q.length_squared() < 1e-12 {
        DQuat::IDENTITY
    } else {
        q.normalize()
    };
    (t, q)
}

/// Bawa geometri target mate dari kerangka LOKAL part ke kerangka dunia.
///
/// Geometri di `MateTargetKind` disimpan dalam kerangka lokal part-nya —
/// itulah satu-satunya interpretasi yang membuat mate tetap sahih saat
/// part-nya bergerak. Versi solver sebelumnya memakai nilai itu APA ADANYA
/// sebagai koordinat dunia, sehingga setelah instance digeser sekali,
/// perhitungan mate berikutnya memakai geometri yang sudah basi.
fn target_to_world(kind: &MateTargetKind, pose: (DVec3, DQuat)) -> MateTargetKind {
    let (t, q) = pose;
    let pt = |p: (f64, f64, f64)| {
        let v = q * DVec3::new(p.0, p.1, p.2) + t;
        (v.x, v.y, v.z)
    };
    let dir = |d: (f64, f64, f64)| {
        let v = q * DVec3::new(d.0, d.1, d.2);
        (v.x, v.y, v.z)
    };
    match kind {
        MateTargetKind::PlanarFace { origin, normal } => MateTargetKind::PlanarFace {
            origin: pt(*origin),
            normal: dir(*normal),
        },
        MateTargetKind::CylinderAxis {
            origin,
            direction,
            radius,
        } => MateTargetKind::CylinderAxis {
            origin: pt(*origin),
            direction: dir(*direction),
            radius: *radius,
        },
        MateTargetKind::Point { pos } => MateTargetKind::Point { pos: pt(*pos) },
    }
}

/// Seberapa jauh sebuah mate dari terpenuhi, dalam mm (komponen sudut
/// diskalakan jadi panjang busur pada radius 1 mm supaya dua satuan yang
/// berbeda bisa dijumlahkan jadi satu angka konvergensi).
fn mate_residual(tf: &MateTransformResult) -> f64 {
    let t = DVec3::new(tf.translation.0, tf.translation.1, tf.translation.2).length();
    t + tf.angle_rad.abs()
}

/// Laporan hasil penyelesaian perakitan.
#[derive(Debug, Clone)]
pub struct AssemblySolveReport {
    pub iterations: usize,
    pub converged: bool,
    /// Pelanggaran terbesar yang tersisa di antara semua mate (mm).
    pub max_residual: f64,
    /// Total transformasi yang diterapkan per instance — dipakai pemanggil
    /// untuk menggeser geometri B-rep-nya sekali saja di akhir.
    pub applied: Vec<(AssemblyInstanceId, MateTransformResult)>,
}

const MAX_ASSEMBLY_ITERS: usize = 60;
const ASSEMBLY_TOL_MM: f64 = 1e-6;
/// Faktor relaksasi. < 1 supaya mate yang saling tarik tidak berosilasi:
/// tiap mate hanya menarik sebagian jalan, dan gabungan tarikan seluruh
/// mate-lah yang menentukan posisi akhir.
const RELAXATION: f64 = 0.6;

/// Selesaikan SELURUH mate secara simultan.
///
/// # Kenapa ditulis ulang
///
/// Versi sebelumnya memproses mate SATU PER SATU dan langsung menerapkan
/// hasilnya ke instance. Tiga akibatnya nyata, bukan teoretis:
///
/// 1. **Mate belakangan merusak mate sebelumnya.** Poros yang sepusat
///    dengan lubang DAN bahunya menempel rata ke permukaan: menyelesaikan
///    Concentric menggeser part, lalu menyelesaikan Coincident
///    menggesernya lagi — dan kesepusatannya hilang. Ini kombinasi paling
///    umum di perakitan mekanik mana pun.
/// 2. **Statusnya berbohong.** `MateStatus::Satisfied` diberikan begitu
///    transformasi satu mate BERHASIL DIHITUNG, tanpa pernah memeriksa
///    apakah mate itu masih terpenuhi setelah semua mate diterapkan.
/// 3. **Hasilnya tidak deterministik.** Iterasi memakai urutan `HashMap`,
///    jadi posisi akhir perakitan bisa berbeda antar jalan.
///
/// Gantinya: relaksasi gaya Jacobi. Tiap iterasi menghitung koreksi SEMUA
/// mate dari keadaan yang SAMA, merata-ratakannya per instance, lalu
/// menerapkannya sebagian (lihat `RELAXATION`). Mate diurutkan berdasarkan
/// id sehingga hasilnya deterministik, dan status ditetapkan dari residual
/// AKHIR — bukan dari apakah satu langkah berhasil dihitung.
pub fn solve_assembly(tree: &mut AssemblyTree) -> AssemblySolveReport {
    // Urutan stabil: `HashMap` tidak menjamin urutan iterasi, dan posisi
    // akhir perakitan tidak boleh bergantung padanya.
    let mut mate_ids: Vec<MateConstraintId> = tree
        .mates
        .iter()
        .filter(|(_, m)| !m.suppressed)
        .map(|(id, _)| *id)
        .collect();
    mate_ids.sort_unstable();

    // Pose awal, untuk menghitung total perpindahan di akhir.
    let initial: std::collections::HashMap<AssemblyInstanceId, (DVec3, DQuat)> = tree
        .instances
        .keys()
        .filter_map(|id| tree.instance_world_transform(*id).map(|p| (*id, p)))
        .collect();

    let mut iterations = 0;
    let mut max_residual = 0.0;

    for iter in 0..MAX_ASSEMBLY_ITERS {
        iterations = iter + 1;
        // Koreksi dikumpulkan dulu untuk SEMUA mate dari keadaan yang sama,
        // baru diterapkan — inilah yang membuatnya simultan, bukan berantai.
        let mut accum: std::collections::HashMap<AssemblyInstanceId, (DVec3, DQuat, usize)> =
            std::collections::HashMap::new();
        let mut worst = 0.0f64;

        for mid in &mate_ids {
            let Some(mate) = tree.mates.get(mid) else {
                continue;
            };
            let id_a = mate.target_a.instance_id;
            let id_b = mate.target_b.instance_id;
            let a_grounded = tree.instances.get(&id_a).is_some_and(|i| i.is_grounded);
            let b_grounded = tree.instances.get(&id_b).is_some_and(|i| i.is_grounded);
            if a_grounded && b_grounded {
                continue;
            }

            // Pose DUNIA, bukan pose lokal: instance yang berada di dalam
            // sub-assembly mewarisi transform induknya, dan mate harus
            // dihitung terhadap posisi nyatanya di ruang perakitan.
            let Some(pose_a) = tree.instance_world_transform(id_a) else {
                continue;
            };
            let Some(pose_b) = tree.instance_world_transform(id_b) else {
                continue;
            };

            // Part yang digerakkan adalah yang TIDAK di-ground; bila
            // keduanya bebas, B yang digerakkan (konvensi lama
            // dipertahankan).
            let (moving_id, world_ref, world_moving) = if !b_grounded {
                (
                    id_b,
                    target_to_world(&mate.target_a.kind, pose_a),
                    target_to_world(&mate.target_b.kind, pose_b),
                )
            } else {
                (
                    id_a,
                    target_to_world(&mate.target_b.kind, pose_b),
                    target_to_world(&mate.target_a.kind, pose_a),
                )
            };

            let mut probe = mate.clone();
            probe.target_a.kind = world_ref;
            probe.target_b.kind = world_moving;
            // Batas gerak dihormati dengan menjepit nilai terkendali mate
            // SEBELUM diselesaikan: engsel pintu tidak berputar 360°, dan
            // tanpa ini solver akan menempatkan part di posisi yang mustahil
            // secara fisik.
            if !mate.limits.is_unbounded() {
                probe.kind = mate.kind.with_value_clamped(&mate.limits);
            }

            let Ok(tf) = solve_single_mate(&probe) else {
                continue;
            };
            worst = worst.max(mate_residual(&tf));

            let delta_t = DVec3::new(tf.translation.0, tf.translation.1, tf.translation.2);
            let delta_q = if tf.angle_rad.abs() > 1e-12 {
                DQuat::from_axis_angle(DVec3::new(tf.axis.0, tf.axis.1, tf.axis.2), tf.angle_rad)
            } else {
                DQuat::IDENTITY
            };
            let entry = accum
                .entry(moving_id)
                .or_insert((DVec3::ZERO, DQuat::IDENTITY, 0));
            entry.0 += delta_t;
            // Rotasi dijumlahkan lewat perkalian quaternion; pembagiannya
            // dilakukan saat penerapan lewat `slerp` dari identitas.
            entry.1 = delta_q * entry.1;
            entry.2 += 1;
        }

        max_residual = worst;
        if worst < ASSEMBLY_TOL_MM {
            break;
        }

        for (inst_id, (sum_t, sum_q, count)) in accum {
            let (_, parent_q) = tree.instance_parent_transform(inst_id);
            let Some(inst) = tree.instances.get_mut(&inst_id) else {
                continue;
            };
            let n = count.max(1) as f64;
            let avg_t = sum_t / n * RELAXATION;
            // Koreksi dihitung di ruang DUNIA, sementara `translation`
            // instance dinyatakan relatif terhadap sub-assembly induknya —
            // jadi koreksinya harus diputar balik ke kerangka induk.
            let local_t = parent_q.inverse() * avg_t;
            inst.translation.0 += local_t.x;
            inst.translation.1 += local_t.y;
            inst.translation.2 += local_t.z;

            if sum_q.length_squared() > 1e-12 {
                // Rata-rata + relaksasi rotasi = interpolasi dari identitas.
                let step_world = DQuat::IDENTITY.slerp(sum_q.normalize(), RELAXATION / n);
                let step_local = parent_q.inverse() * step_world * parent_q;
                let (_, cur_q) = instance_pose(inst);
                let new_q = (step_local * cur_q).normalize();
                inst.rotation_quat = (new_q.x, new_q.y, new_q.z, new_q.w);
            }
        }
    }

    // Status ditetapkan dari keadaan AKHIR, bukan dari keberhasilan satu
    // langkah perhitungan.
    for mid in &mate_ids {
        let Some(mate) = tree.mates.get(mid) else {
            continue;
        };
        let id_a = mate.target_a.instance_id;
        let id_b = mate.target_b.instance_id;
        let (Some(pose_a), Some(pose_b)) = (
            tree.instance_world_transform(id_a),
            tree.instance_world_transform(id_b),
        ) else {
            continue;
        };
        let mut probe = mate.clone();
        probe.target_a.kind = target_to_world(&mate.target_a.kind, pose_a);
        probe.target_b.kind = target_to_world(&mate.target_b.kind, pose_b);
        if !mate.limits.is_unbounded() {
            probe.kind = mate.kind.with_value_clamped(&mate.limits);
        }
        let status = match solve_single_mate(&probe) {
            Ok(tf) if mate_residual(&tf) < 1e-3 => MateStatus::Satisfied,
            Ok(tf) => MateStatus::Conflicted(format!(
                "tidak terpenuhi, sisa pelanggaran {:.3} mm",
                mate_residual(&tf)
            )),
            Err(e) => MateStatus::Conflicted(e.to_string()),
        };
        if let Some(m) = tree.mates.get_mut(mid) {
            m.status = status;
        }
    }

    // Total perpindahan tiap instance dari posisi awalnya, supaya pemanggil
    // menggeser geometri B-rep-nya SEKALI saja — bukan sekali per iterasi.
    let mut applied = Vec::new();
    for id in tree.instances.keys() {
        let Some(&(t0, q0)) = initial.get(id) else {
            continue;
        };
        let Some((t1, q1)) = tree.instance_world_transform(*id) else {
            continue;
        };
        let dq = q1 * q0.inverse();
        let (axis, angle) = dq.to_axis_angle();
        let dt = t1 - t0;
        if dt.length() < 1e-12 && angle.abs() < 1e-12 {
            continue;
        }
        applied.push((
            *id,
            MateTransformResult {
                translation: (dt.x, dt.y, dt.z),
                pivot: (t0.x, t0.y, t0.z),
                axis: (axis.x, axis.y, axis.z),
                angle_rad: angle,
            },
        ));
    }
    applied.sort_by_key(|(id, _)| *id);

    AssemblySolveReport {
        iterations,
        converged: max_residual < ASSEMBLY_TOL_MM,
        max_residual,
        applied,
    }
}

/// Seret satu instance ke `target_world` lalu selesaikan ulang seluruh
/// perakitan.
///
/// Inilah perilaku "drag part" di CAD perakitan: part yang diseret TIDAK
/// dipaku di titik seretan. Ia dipindahkan ke sana, lalu mate-nya menarik
/// balik — sehingga part hanya benar-benar berpindah sepanjang derajat
/// kebebasan yang masih bebas. Poros pada mate silinder yang diseret
/// menyamping akan kembali ke sumbunya, tapi mempertahankan pergeserannya
/// sepanjang sumbu itu. Part yang di-ground tidak bisa diseret sama sekali.
pub fn solve_assembly_with_drag(
    tree: &mut AssemblyTree,
    dragged: AssemblyInstanceId,
    target_world: (f64, f64, f64),
) -> AssemblySolveReport {
    let grounded = tree.instances.get(&dragged).is_some_and(|i| i.is_grounded);
    if !grounded {
        // Target diberikan di ruang dunia; `translation` instance relatif
        // terhadap induknya, jadi diputar balik lebih dulu.
        let (parent_t, parent_q) = tree.instance_parent_transform(dragged);
        let world = DVec3::new(target_world.0, target_world.1, target_world.2);
        let local = parent_q.inverse() * (world - parent_t);
        if let Some(inst) = tree.instances.get_mut(&dragged) {
            inst.translation = (local.x, local.y, local.z);
        }
    }
    solve_assembly(tree)
}

/// Evaluasi studi gerak pada posisi `t` dalam [0, 1]: nilai mate yang
/// digerakkan diatur, lalu seluruh perakitan diselesaikan ulang.
///
/// Mengembalikan `None` bila mate-nya tidak ada atau bukan mate bernilai
/// numerik — menggerakkan `Concentric` tidak punya arti.
pub fn evaluate_motion(
    tree: &mut AssemblyTree,
    study: &ducad_core::assembly::MotionStudy,
    t: f64,
) -> Option<AssemblySolveReport> {
    let value = study.value_at(t);
    let mate = tree.mates.get_mut(&study.driven_mate)?;
    mate.kind = match &mate.kind {
        MateKind::Distance {
            opposite_normal, ..
        } => MateKind::Distance {
            offset: value,
            opposite_normal: *opposite_normal,
        },
        MateKind::Angle {
            opposite_normal, ..
        } => MateKind::Angle {
            angle_deg: value,
            opposite_normal: *opposite_normal,
        },
        _ => return None,
    };
    Some(solve_assembly(tree))
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

#[cfg(test)]
mod simultaneous_tests {
    use super::*;
    use ducad_core::assembly::{
        AssemblyTree, MateConstraint, MateStatus, MateTarget,
    };

    fn tree_with_two_parts() -> (AssemblyTree, AssemblyInstanceId, AssemblyInstanceId) {
        let mut tree = AssemblyTree::default();
        let base = tree.add_instance("Base".to_string(), 1);
        let shaft = tree.add_instance("Shaft".to_string(), 2);
        if let Some(b) = tree.instances.get_mut(&base) {
            b.is_grounded = true;
        }
        (tree, base, shaft)
    }

    fn add_mate(tree: &mut AssemblyTree, mate: MateConstraint) {
        tree.mates.insert(mate.id, mate);
    }

    fn mate(
        id: u32,
        kind: MateKind,
        a: (AssemblyInstanceId, MateTargetKind),
        b: (AssemblyInstanceId, MateTargetKind),
    ) -> MateConstraint {
        MateConstraint {
            id,
            name: format!("Mate {id}"),
            kind,
            target_a: MateTarget {
                instance_id: a.0,
                kind: a.1,
            },
            target_b: MateTarget {
                instance_id: b.0,
                kind: b.1,
            },
            status: MateStatus::UnderConstrained,
            suppressed: false,
            limits: Default::default(),
            joint: None,
        }
    }

    #[test]
    fn concentric_and_coincident_are_satisfied_together() {
        // KASUS YANG DULU RUSAK. Poros sepusat dengan lubang DAN bahunya
        // menempel rata ke permukaan — kombinasi paling umum di perakitan
        // mekanik. Solver sekuensial lama menyelesaikan Concentric,
        // menggeser part, lalu menyelesaikan Coincident yang menggesernya
        // lagi sehingga kesepusatannya HILANG, tapi kedua mate tetap
        // dilaporkan "Satisfied".
        let (mut tree, base, shaft) = tree_with_two_parts();

        add_mate(
            &mut tree,
            mate(
                1,
                MateKind::Concentric {
                    lock_rotation: false,
                    aligned: true,
                },
                (
                    base,
                    MateTargetKind::CylinderAxis {
                        origin: (0.0, 0.0, 0.0),
                        direction: (0.0, 0.0, 1.0),
                        radius: 5.0,
                    },
                ),
                (
                    shaft,
                    MateTargetKind::CylinderAxis {
                        origin: (30.0, 20.0, 0.0),
                        direction: (0.0, 0.0, 1.0),
                        radius: 5.0,
                    },
                ),
            ),
        );
        add_mate(
            &mut tree,
            mate(
                2,
                MateKind::Coincident {
                    opposite_normal: false,
                },
                (
                    base,
                    MateTargetKind::PlanarFace {
                        origin: (0.0, 0.0, 10.0),
                        normal: (0.0, 0.0, 1.0),
                    },
                ),
                (
                    shaft,
                    MateTargetKind::PlanarFace {
                        origin: (30.0, 20.0, 40.0),
                        normal: (0.0, 0.0, 1.0),
                    },
                ),
            ),
        );

        let report = solve_assembly(&mut tree);
        assert!(
            report.converged,
            "harus konvergen; sisa pelanggaran {} mm setelah {} iterasi",
            report.max_residual, report.iterations
        );

        // KEDUANYA harus terpenuhi, bukan cuma yang terakhir diproses.
        for id in [1u32, 2u32] {
            assert_eq!(
                tree.mates.get(&id).map(|m| m.status.clone()),
                Some(MateStatus::Satisfied),
                "mate {id} harus terpenuhi di akhir"
            );
        }

        // Verifikasi geometris langsung: sumbu poros harus berimpit dengan
        // sumbu lubang (x,y = 0), dan bahunya di Z = 10.
        let inst = tree.instances.get(&shaft).unwrap();
        let pose = instance_pose(inst);
        let axis_world = target_to_world(
            &MateTargetKind::CylinderAxis {
                origin: (30.0, 20.0, 0.0),
                direction: (0.0, 0.0, 1.0),
                radius: 5.0,
            },
            pose,
        );
        let face_world = target_to_world(
            &MateTargetKind::PlanarFace {
                origin: (30.0, 20.0, 40.0),
                normal: (0.0, 0.0, 1.0),
            },
            pose,
        );
        let (ax, _) = extract_origin_and_dir(&axis_world).unwrap();
        let (fo, _) = extract_origin_and_dir(&face_world).unwrap();
        assert!(ax.x.abs() < 1e-3 && ax.y.abs() < 1e-3, "sumbu poros {ax:?}");
        assert!((fo.z - 10.0).abs() < 1e-3, "bidang bahu di z = {}", fo.z);
    }

    #[test]
    fn result_is_deterministic_regardless_of_mate_insertion_order() {
        // Solver lama memakai urutan iterasi HashMap, jadi posisi akhir
        // perakitan bisa berbeda antar jalan. Di sini mate disisipkan dalam
        // dua urutan berlawanan dan hasilnya harus identik.
        let build = |reverse: bool| {
            let (mut tree, base, shaft) = tree_with_two_parts();
            let m1 = mate(
                1,
                MateKind::Coincident {
                    opposite_normal: false,
                },
                (
                    base,
                    MateTargetKind::PlanarFace {
                        origin: (0.0, 0.0, 0.0),
                        normal: (0.0, 0.0, 1.0),
                    },
                ),
                (
                    shaft,
                    MateTargetKind::PlanarFace {
                        origin: (0.0, 0.0, 25.0),
                        normal: (0.0, 0.0, 1.0),
                    },
                ),
            );
            let m2 = mate(
                2,
                MateKind::Concentric {
                    lock_rotation: false,
                    aligned: true,
                },
                (
                    base,
                    MateTargetKind::CylinderAxis {
                        origin: (0.0, 0.0, 0.0),
                        direction: (0.0, 0.0, 1.0),
                        radius: 4.0,
                    },
                ),
                (
                    shaft,
                    MateTargetKind::CylinderAxis {
                        origin: (12.0, -7.0, 0.0),
                        direction: (0.0, 0.0, 1.0),
                        radius: 4.0,
                    },
                ),
            );
            if reverse {
                add_mate(&mut tree, m2);
                add_mate(&mut tree, m1);
            } else {
                add_mate(&mut tree, m1);
                add_mate(&mut tree, m2);
            }
            solve_assembly(&mut tree);
            tree.instances.get(&shaft).unwrap().translation
        };

        let a = build(false);
        let b = build(true);
        assert!(
            (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9 && (a.2 - b.2).abs() < 1e-9,
            "posisi akhir harus sama: {a:?} vs {b:?}"
        );
    }

    #[test]
    fn conflicting_mates_are_reported_not_silently_accepted() {
        // Dua mate jarak yang saling bertentangan pada sumbu yang sama.
        // Yang penting: statusnya TIDAK boleh "Satisfied".
        let (mut tree, base, shaft) = tree_with_two_parts();
        for (id, offset) in [(1u32, 10.0), (2u32, 40.0)] {
            add_mate(
                &mut tree,
                mate(
                    id,
                    MateKind::Distance {
                        offset,
                        opposite_normal: false,
                    },
                    (
                        base,
                        MateTargetKind::PlanarFace {
                            origin: (0.0, 0.0, 0.0),
                            normal: (0.0, 0.0, 1.0),
                        },
                    ),
                    (
                        shaft,
                        MateTargetKind::PlanarFace {
                            origin: (0.0, 0.0, 0.0),
                            normal: (0.0, 0.0, 1.0),
                        },
                    ),
                ),
            );
        }

        let report = solve_assembly(&mut tree);
        assert!(!report.converged, "kendala bertentangan tidak boleh konvergen");
        assert!(
            tree.mates
                .values()
                .any(|m| matches!(m.status, MateStatus::Conflicted(_))),
            "minimal satu mate harus dilaporkan Conflicted, bukan Satisfied"
        );
    }

    #[test]
    fn grounded_parts_never_move() {
        let (mut tree, base, shaft) = tree_with_two_parts();
        add_mate(
            &mut tree,
            mate(
                1,
                MateKind::Coincident {
                    opposite_normal: false,
                },
                (
                    base,
                    MateTargetKind::PlanarFace {
                        origin: (0.0, 0.0, 0.0),
                        normal: (0.0, 0.0, 1.0),
                    },
                ),
                (
                    shaft,
                    MateTargetKind::PlanarFace {
                        origin: (0.0, 0.0, 50.0),
                        normal: (0.0, 0.0, 1.0),
                    },
                ),
            ),
        );
        solve_assembly(&mut tree);
        let b = tree.instances.get(&base).unwrap();
        assert_eq!(b.translation, (0.0, 0.0, 0.0), "part grounded tidak boleh bergeser");
    }
}

#[cfg(test)]
mod drag_motion_explode_tests {
    use super::*;
    use ducad_core::assembly::{AssemblyTree, MateConstraint, MateStatus, MateTarget, MotionStudy};

    fn tree_with_two_parts() -> (AssemblyTree, AssemblyInstanceId, AssemblyInstanceId) {
        let mut tree = AssemblyTree::default();
        let base = tree.add_instance("Base".to_string(), 1);
        let part = tree.add_instance("Part".to_string(), 2);
        tree.instances.get_mut(&base).unwrap().is_grounded = true;
        (tree, base, part)
    }

    fn mate(
        id: u32,
        kind: MateKind,
        a: (AssemblyInstanceId, MateTargetKind),
        b: (AssemblyInstanceId, MateTargetKind),
    ) -> MateConstraint {
        MateConstraint {
            id,
            name: format!("Mate {id}"),
            kind,
            target_a: MateTarget { instance_id: a.0, kind: a.1 },
            target_b: MateTarget { instance_id: b.0, kind: b.1 },
            status: MateStatus::UnderConstrained,
            suppressed: false,
            limits: Default::default(),
            joint: None,
        }
    }

    fn z_axis(origin: (f64, f64, f64)) -> MateTargetKind {
        MateTargetKind::CylinderAxis {
            origin,
            direction: (0.0, 0.0, 1.0),
            radius: 5.0,
        }
    }

    #[test]
    fn dragging_a_slider_only_moves_it_along_its_free_axis() {
        // INTI drag-dengan-solver. Part pada mate silinder sumbu Z bebas
        // bergeser sepanjang Z dan berputar, tapi TIDAK bebas menyamping.
        // Diseret ke (5, 7, 30): X dan Y harus ditarik kembali ke sumbu,
        // sementara Z = 30 dipertahankan.
        let (mut tree, base, part) = tree_with_two_parts();
        tree.mates.insert(
            1,
            mate(
                1,
                MateKind::Concentric { lock_rotation: false, aligned: true },
                (base, z_axis((0.0, 0.0, 0.0))),
                (part, z_axis((0.0, 0.0, 0.0))),
            ),
        );

        let report = solve_assembly_with_drag(&mut tree, part, (5.0, 7.0, 30.0));
        assert!(report.converged, "residual {}", report.max_residual);

        let t = tree.instances.get(&part).unwrap().translation;
        assert!(t.0.abs() < 1e-3 && t.1.abs() < 1e-3, "harus kembali ke sumbu: {t:?}");
        assert!((t.2 - 30.0).abs() < 1e-3, "pergeseran sepanjang sumbu harus dipertahankan: {t:?}");
    }

    #[test]
    fn grounded_part_cannot_be_dragged() {
        let (mut tree, base, _part) = tree_with_two_parts();
        solve_assembly_with_drag(&mut tree, base, (100.0, 100.0, 100.0));
        assert_eq!(tree.instances.get(&base).unwrap().translation, (0.0, 0.0, 0.0));
    }

    #[test]
    fn motion_study_drives_a_distance_mate_through_intermediate_positions() {
        let (mut tree, base, part) = tree_with_two_parts();
        let plane = |z: f64| MateTargetKind::PlanarFace {
            origin: (0.0, 0.0, z),
            normal: (0.0, 0.0, 1.0),
        };
        tree.mates.insert(
            1,
            mate(
                1,
                MateKind::Distance { offset: 0.0, opposite_normal: false },
                (base, plane(0.0)),
                (part, plane(0.0)),
            ),
        );
        let study = MotionStudy {
            name: "Buka".to_string(),
            driven_mate: 1,
            from: 10.0,
            to: 50.0,
            steps: 4,
        };

        // t = 0.5 -> jarak 30 mm. Yang diverifikasi adalah POSISI part,
        // bukan cuma nilai mate-nya: solver benar-benar harus menggerakkan.
        let report = evaluate_motion(&mut tree, &study, 0.5).expect("mate numerik");
        assert!(report.converged, "residual {}", report.max_residual);
        let z = tree.instances.get(&part).unwrap().translation.2;
        assert!((z - 30.0).abs() < 1e-3, "z = {z}, seharusnya 30");

        let _ = evaluate_motion(&mut tree, &study, 1.0).unwrap();
        let z = tree.instances.get(&part).unwrap().translation.2;
        assert!((z - 50.0).abs() < 1e-3, "z = {z}, seharusnya 50");
    }

    #[test]
    fn motion_study_refuses_non_numeric_mates() {
        // Menggerakkan `Concentric` tidak punya arti; harus None, bukan
        // diam-diam tidak melakukan apa-apa lalu melaporkan sukses.
        let (mut tree, base, part) = tree_with_two_parts();
        tree.mates.insert(
            1,
            mate(
                1,
                MateKind::Concentric { lock_rotation: false, aligned: true },
                (base, z_axis((0.0, 0.0, 0.0))),
                (part, z_axis((0.0, 0.0, 0.0))),
            ),
        );
        let study = MotionStudy { name: "x".into(), driven_mate: 1, from: 0.0, to: 1.0, steps: 1 };
        assert!(evaluate_motion(&mut tree, &study, 0.5).is_none());
    }

    #[test]
    fn explode_factor_moves_display_but_never_the_solver() {
        // Jebakan exploded view: kalau solver melihat pergeseran urai, ia
        // menganggap semua mate terlanggar dan menarik part kembali —
        // membatalkan urai atau, lebih buruk, merusak posisi terakit.
        let (mut tree, base, part) = tree_with_two_parts();
        tree.mates.insert(
            1,
            mate(
                1,
                MateKind::Concentric { lock_rotation: false, aligned: true },
                (base, z_axis((0.0, 0.0, 0.0))),
                (part, z_axis((0.0, 0.0, 0.0))),
            ),
        );
        solve_assembly(&mut tree);
        let assembled = tree.instance_world_transform(part).unwrap().0;

        tree.instances.get_mut(&part).unwrap().explode_offset = (0.0, 0.0, 100.0);
        tree.explode_factor = 0.5;

        // Tampilan bergeser 50 mm...
        let shown = tree.instance_display_transform(part).unwrap().0;
        assert!((shown - assembled - DVec3::new(0.0, 0.0, 50.0)).length() < 1e-9);

        // ...tapi solver masih melihat keadaan terakit dan tetap puas.
        let report = solve_assembly(&mut tree);
        assert!(report.converged);
        assert_eq!(tree.mates[&1].status, MateStatus::Satisfied);
        assert!((tree.instance_world_transform(part).unwrap().0 - assembled).length() < 1e-9);
    }

    #[test]
    fn auto_explode_pushes_parts_outward_and_leaves_ground_anchored() {
        let mut tree = AssemblyTree::default();
        let base = tree.add_instance("Base".to_string(), 1);
        tree.instances.get_mut(&base).unwrap().is_grounded = true;
        let left = tree.add_instance("L".to_string(), 2);
        let right = tree.add_instance("R".to_string(), 3);
        tree.instances.get_mut(&left).unwrap().translation = (-10.0, 0.0, 0.0);
        tree.instances.get_mut(&right).unwrap().translation = (10.0, 0.0, 0.0);

        tree.auto_explode_radial(40.0);

        assert_eq!(tree.instances[&base].explode_offset, (0.0, 0.0, 0.0), "jangkar tidak bergeser");
        assert!(tree.instances[&left].explode_offset.0 < -30.0, "kiri terdorong ke kiri");
        assert!(tree.instances[&right].explode_offset.0 > 30.0, "kanan terdorong ke kanan");
    }
}
