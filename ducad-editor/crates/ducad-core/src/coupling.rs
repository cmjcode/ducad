//! Mate lanjutan (P20) sebagai KOPLING kinematik, dan langkah urai
//! (*exploded view*) berurutan.
//!
//! Gear/screw/rack-pinion bukan kendala geometri seperti `MateKind`
//! (yang menempelkan face/sumbu), melainkan hubungan GERAK: pose instance
//! yang digerakkan dihitung dari sudut putar instance penggerak. Karena itu
//! mereka disimpan terpisah di `AssemblyTree::couplings` dan dievaluasi
//! SETELAH solver mate — tanpa menambah varian `MateKind`.

use glam::{DQuat, DVec3};
use serde::{Deserialize, Serialize};

use crate::assembly::{AssemblyInstanceId, AssemblyTree};

/// Jenis hubungan gerak.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CouplingKind {
    /// Sudut keluaran = `ratio` × sudut masukan (rasio negatif = arah balik,
    /// seperti pasangan roda gigi luar).
    Gear { ratio: f64 },
    /// Translasi sepanjang sumbu penggerak = `pitch` mm per putaran.
    Screw { pitch: f64 },
    /// Translasi sepanjang sumbu yang digerakkan = `pitch_radius` × sudut (rad).
    RackPinion { pitch_radius: f64 },
}

/// Sumbu di ruang perakitan: titik + arah.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AxisLine {
    pub origin: (f64, f64, f64),
    pub dir: (f64, f64, f64),
}

impl AxisLine {
    fn origin_v(&self) -> DVec3 {
        DVec3::new(self.origin.0, self.origin.1, self.origin.2)
    }
    fn dir_v(&self) -> Option<DVec3> {
        DVec3::new(self.dir.0, self.dir.1, self.dir.2).try_normalize()
    }
}

/// Satu kopling: `driven` mengikuti putaran `driver`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Coupling {
    pub name: String,
    pub kind: CouplingKind,
    pub driver: AssemblyInstanceId,
    /// Sumbu putar penggerak (ruang perakitan).
    pub driver_axis: AxisLine,
    pub driven: AssemblyInstanceId,
    /// Gear: sumbu putar yang digerakkan. Rack-pinion: arah luncur. Screw:
    /// tidak dipakai (translasi mengikuti sumbu penggerak).
    pub driven_axis: AxisLine,
    /// Orientasi penggerak saat kopling dibuat (sudut nol).
    pub driver_reference_quat: (f64, f64, f64, f64),
    /// Pose yang digerakkan saat kopling dibuat.
    pub driven_reference_translation: (f64, f64, f64),
    pub driven_reference_quat: (f64, f64, f64, f64),
}

fn quat(q: (f64, f64, f64, f64)) -> DQuat {
    DQuat::from_xyzw(q.0, q.1, q.2, q.3).normalize()
}

/// Sudut putar bertanda `q_now` relatif `q_ref` terhadap sumbu `axis`
/// (radian, −π..π): komponen twist dari dekomposisi swing-twist.
pub fn twist_angle(q_ref: DQuat, q_now: DQuat, axis: DVec3) -> f64 {
    let rel = q_now * q_ref.inverse();
    let proj = DVec3::new(rel.x, rel.y, rel.z).dot(axis);
    let angle = 2.0 * proj.atan2(rel.w);
    // Normalisasi ke (−π, π].
    let two_pi = std::f64::consts::TAU;
    let wrapped = (angle + std::f64::consts::PI).rem_euclid(two_pi) - std::f64::consts::PI;
    if wrapped <= -std::f64::consts::PI {
        wrapped + two_pi
    } else {
        wrapped
    }
}

/// Satu langkah urai: instance digeser (lalu diputar pada sumbu lewat
/// posisinya) saat tampilan diurai.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExplodeStep {
    pub instance: AssemblyInstanceId,
    /// Pergeseran penuh langkah ini (ruang perakitan, mm).
    pub translation: (f64, f64, f64),
    /// Putaran opsional `(sumbu, sudut°)` pada posisi instance.
    #[serde(default)]
    pub rotation: Option<((f64, f64, f64), f64)>,
}

impl AssemblyTree {
    /// Daftarkan kopling dengan pose saat ini sebagai acuan sudut nol.
    /// `None` bila instance tidak ada, sumbu nol, atau parameter tidak sah.
    pub fn add_coupling(
        &mut self,
        name: impl Into<String>,
        kind: CouplingKind,
        driver: AssemblyInstanceId,
        driver_axis: AxisLine,
        driven: AssemblyInstanceId,
        driven_axis: AxisLine,
    ) -> Option<usize> {
        let valid = match kind {
            CouplingKind::Gear { ratio } => ratio.is_finite() && ratio != 0.0,
            CouplingKind::Screw { pitch } => pitch.is_finite() && pitch != 0.0,
            CouplingKind::RackPinion { pitch_radius } => {
                pitch_radius.is_finite() && pitch_radius != 0.0
            }
        };
        if !valid || driver == driven {
            return None;
        }
        driver_axis.dir_v()?;
        if !matches!(kind, CouplingKind::Screw { .. }) {
            driven_axis.dir_v()?;
        }
        let d = self.instances.get(&driver)?;
        let f = self.instances.get(&driven)?;
        self.couplings.push(Coupling {
            name: name.into(),
            kind,
            driver,
            driver_axis,
            driven,
            driven_axis,
            driver_reference_quat: d.rotation_quat,
            driven_reference_translation: f.translation,
            driven_reference_quat: f.rotation_quat,
        });
        Some(self.couplings.len() - 1)
    }

    /// Hitung ulang pose semua instance yang digerakkan dari sudut putar
    /// penggeraknya. Dievaluasi berurutan, jadi rantai gear (A→B→C) bekerja
    /// bila didaftarkan berurutan. Mengembalikan jumlah kopling diterapkan.
    pub fn apply_couplings(&mut self) -> usize {
        let mut applied = 0;
        for i in 0..self.couplings.len() {
            let c = self.couplings[i].clone();
            let (Some(driver), Some(axis)) = (self.instances.get(&c.driver), c.driver_axis.dir_v())
            else {
                continue;
            };
            let theta = twist_angle(
                quat(c.driver_reference_quat),
                quat(driver.rotation_quat),
                axis,
            );
            let ref_t = DVec3::new(
                c.driven_reference_translation.0,
                c.driven_reference_translation.1,
                c.driven_reference_translation.2,
            );
            let ref_q = quat(c.driven_reference_quat);
            let (t, q) = match c.kind {
                CouplingKind::Gear { ratio } => {
                    let Some(out_axis) = c.driven_axis.dir_v() else {
                        continue;
                    };
                    let spin = DQuat::from_axis_angle(out_axis, theta * ratio);
                    let pivot = c.driven_axis.origin_v();
                    (pivot + spin * (ref_t - pivot), spin * ref_q)
                }
                CouplingKind::Screw { pitch } => (
                    ref_t + axis * (pitch * theta / std::f64::consts::TAU),
                    ref_q,
                ),
                CouplingKind::RackPinion { pitch_radius } => {
                    let Some(slide) = c.driven_axis.dir_v() else {
                        continue;
                    };
                    (ref_t + slide * (pitch_radius * theta), ref_q)
                }
            };
            if let Some(inst) = self.instances.get_mut(&c.driven) {
                inst.translation = (t.x, t.y, t.z);
                inst.rotation_quat = (q.x, q.y, q.z, q.w);
                applied += 1;
            }
        }
        applied
    }

    /// Tambah langkah urai di akhir urutan.
    pub fn add_explode_step(&mut self, step: ExplodeStep) -> bool {
        if !self.instances.contains_key(&step.instance) {
            return false;
        }
        self.explode_steps.push(step);
        true
    }

    /// Sumbangan langkah urai untuk `id` pada `explode_factor` saat ini:
    /// `(pergeseran, putaran)`. Langkah dimainkan BERURUTAN — faktor 0..1
    /// dibagi rata ke semua langkah, jadi part terluar lepas lebih dulu.
    pub fn explode_step_pose(&self, id: AssemblyInstanceId) -> (DVec3, DQuat) {
        let n = self.explode_steps.len();
        let progress = self.explode_factor.clamp(0.0, 1.0) * n as f64;
        let mut shift = DVec3::ZERO;
        let mut spin = DQuat::IDENTITY;
        for (i, step) in self.explode_steps.iter().enumerate() {
            if step.instance != id {
                continue;
            }
            let k = (progress - i as f64).clamp(0.0, 1.0);
            shift += DVec3::new(step.translation.0, step.translation.1, step.translation.2) * k;
            if let Some((axis, angle_deg)) = step.rotation {
                if let Some(a) = DVec3::new(axis.0, axis.1, axis.2).try_normalize() {
                    spin = DQuat::from_axis_angle(a, angle_deg.to_radians() * k) * spin;
                }
            }
        }
        (shift, spin)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const Z: AxisLine = AxisLine {
        origin: (0.0, 0.0, 0.0),
        dir: (0.0, 0.0, 1.0),
    };

    fn tree() -> (AssemblyTree, AssemblyInstanceId, AssemblyInstanceId) {
        let mut t = AssemblyTree::default();
        let a = t.add_instance("driver", 1);
        let b = t.add_instance("driven", 2);
        (t, a, b)
    }

    fn spin(t: &mut AssemblyTree, id: AssemblyInstanceId, deg: f64) {
        let q = DQuat::from_axis_angle(DVec3::Z, deg.to_radians());
        t.instances.get_mut(&id).unwrap().rotation_quat = (q.x, q.y, q.z, q.w);
    }

    #[test]
    fn coupling_twist_angle_is_signed_and_axis_specific() {
        let q = DQuat::from_axis_angle(DVec3::Z, 0.7);
        assert!((twist_angle(DQuat::IDENTITY, q, DVec3::Z) - 0.7).abs() < 1e-12);
        assert!((twist_angle(DQuat::IDENTITY, q, -DVec3::Z) + 0.7).abs() < 1e-12);
        assert!(twist_angle(DQuat::IDENTITY, q, DVec3::X).abs() < 1e-12);
        let back = DQuat::from_axis_angle(DVec3::Z, -2.5);
        assert!((twist_angle(DQuat::IDENTITY, back, DVec3::Z) + 2.5).abs() < 1e-12);
    }

    #[test]
    fn coupling_rejects_invalid_definitions() {
        let (mut t, a, b) = tree();
        assert!(t
            .add_coupling("x", CouplingKind::Gear { ratio: 0.0 }, a, Z, b, Z)
            .is_none());
        assert!(t
            .add_coupling("x", CouplingKind::Gear { ratio: 2.0 }, a, Z, a, Z)
            .is_none());
        assert!(t
            .add_coupling("x", CouplingKind::Gear { ratio: 2.0 }, a, Z, 99, Z)
            .is_none());
        let zero = AxisLine {
            origin: (0.0, 0.0, 0.0),
            dir: (0.0, 0.0, 0.0),
        };
        assert!(t
            .add_coupling("x", CouplingKind::Gear { ratio: 2.0 }, a, zero, b, Z)
            .is_none());
        // Screw tidak butuh sumbu yang digerakkan.
        assert!(t
            .add_coupling("x", CouplingKind::Screw { pitch: 1.5 }, a, Z, b, zero)
            .is_some());
    }

    #[test]
    fn explode_steps_play_in_sequence() {
        let (mut t, a, b) = tree();
        assert!(t.add_explode_step(ExplodeStep {
            instance: a,
            translation: (0.0, 0.0, 40.0),
            rotation: None,
        }));
        assert!(t.add_explode_step(ExplodeStep {
            instance: b,
            translation: (20.0, 0.0, 0.0),
            rotation: Some(((0.0, 0.0, 1.0), 90.0)),
        }));
        assert!(!t.add_explode_step(ExplodeStep {
            instance: 99,
            translation: (1.0, 0.0, 0.0),
            rotation: None,
        }));
        // Setengah jalan: langkah 1 selesai, langkah 2 belum mulai.
        t.explode_factor = 0.5;
        assert_eq!(t.explode_step_pose(a).0, DVec3::new(0.0, 0.0, 40.0));
        assert_eq!(t.explode_step_pose(b).0, DVec3::ZERO);
        t.explode_factor = 0.75;
        let (shift, spin) = t.explode_step_pose(b);
        assert!((shift.x - 10.0).abs() < 1e-12);
        assert!(
            (twist_angle(DQuat::IDENTITY, spin, DVec3::Z) - 45.0_f64.to_radians()).abs() < 1e-12
        );
        // Tampilan mengikuti; solver (pose dunia) tidak berubah.
        let (display, _) = t.instance_display_transform(a).unwrap();
        assert_eq!(display.z, 40.0);
        assert_eq!(t.instances[&a].translation, (0.0, 0.0, 0.0));
        t.explode_factor = 0.0;
        assert_eq!(t.instance_display_transform(a).unwrap().0, DVec3::ZERO);
        let _ = spin;
    }

    #[test]
    fn coupling_serde_defaults_keep_old_assemblies_readable() {
        let (t, _, _) = tree();
        let mut json = serde_json::to_value(&t).unwrap();
        let obj = json.as_object_mut().unwrap();
        obj.remove("couplings");
        obj.remove("explode_steps");
        let back: AssemblyTree = serde_json::from_value(json).unwrap();
        assert!(back.couplings.is_empty() && back.explode_steps.is_empty());
    }

    #[test]
    fn coupling_gear_chain_and_rack() {
        let (mut t, a, b) = tree();
        let c = t.add_instance("rack", 3);
        t.instances.get_mut(&b).unwrap().translation = (30.0, 0.0, 0.0);
        let b_axis = AxisLine {
            origin: (30.0, 0.0, 0.0),
            dir: (0.0, 0.0, 1.0),
        };
        let x_dir = AxisLine {
            origin: (0.0, 0.0, 0.0),
            dir: (1.0, 0.0, 0.0),
        };
        t.add_coupling("g", CouplingKind::Gear { ratio: -2.0 }, a, Z, b, b_axis)
            .unwrap();
        t.add_coupling(
            "r",
            CouplingKind::RackPinion { pitch_radius: 10.0 },
            b,
            b_axis,
            c,
            x_dir,
        )
        .unwrap();
        spin(&mut t, a, 30.0);
        assert_eq!(t.apply_couplings(), 2);
        let qb = quat(t.instances[&b].rotation_quat);
        assert!((twist_angle(DQuat::IDENTITY, qb, DVec3::Z) + 60.0_f64.to_radians()).abs() < 1e-12);
        // Gear berputar di tempat (sumbunya lewat posisinya sendiri).
        assert!((t.instances[&b].translation.0 - 30.0).abs() < 1e-12);
        // Rack: r·θ = 10 · (−60°).
        let expect = 10.0 * (-60.0_f64).to_radians();
        assert!((t.instances[&c].translation.0 - expect).abs() < 1e-9);
    }
}
