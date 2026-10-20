//! Model joint kinematik (Paritas 4.1–4.4).
//!
//! Sebuah joint menyimpan *frame* di ruang LOKAL instance A (titik asal,
//! sumbu, arah kedua) dan pose relatif B terhadap A saat nilai joint nol.
//! Pose B yang diinginkan = `A · M(nilai) · rel`, dengan `M` gerak lokal
//! jenis joint (putar pada sumbu, geser sepanjang sumbu, …). Model yang sama
//! dipakai untuk joint biasa (B dirapatkan dulu ke A), joint "as-built" (B
//! tidak dipindah; pose sekarang = nilai nol), dan motion study (nilai
//! pertama digerakkan).

use glam::{DAffine3, DQuat, DVec3};
use serde::{Deserialize, Serialize};

use crate::assembly::JointKind;

/// Pose: (translasi, rotasi).
pub type Pose = (DVec3, DQuat);

fn affine(p: Pose) -> DAffine3 {
    DAffine3::from_rotation_translation(p.1, p.0)
}

fn pose(a: DAffine3) -> Pose {
    let (_, q, t) = a.to_scale_rotation_translation();
    (t, q.normalize())
}

fn v3(t: (f64, f64, f64)) -> DVec3 {
    DVec3::new(t.0, t.1, t.2)
}

fn t3(v: DVec3) -> (f64, f64, f64) {
    (v.x, v.y, v.z)
}

/// Frame joint di ruang lokal instance A.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct JointFrame {
    pub origin: (f64, f64, f64),
    /// Sumbu utama (satuan): sumbu putar/geser, normal bidang, sumbu pin.
    pub axis: (f64, f64, f64),
    /// Arah kedua (satuan, tegak lurus `axis`): arah alur pin-slot, sumbu X
    /// bidang planar.
    pub u: (f64, f64, f64),
}

impl JointFrame {
    /// Frame dari titik + sumbu (+ arah kedua opsional) dalam koordinat
    /// dunia; disimpan relatif terhadap pose dunia A.
    pub fn from_world(a_world: Pose, origin: DVec3, axis: DVec3, u: Option<DVec3>) -> Option<Self> {
        if !origin.is_finite() || !axis.is_finite() {
            return None;
        }
        let axis = axis.try_normalize()?;
        let u = u
            .and_then(|u| (u - axis * u.dot(axis)).try_normalize())
            .unwrap_or_else(|| axis.any_orthonormal_vector());
        let inv = affine(a_world).inverse();
        Some(Self {
            origin: t3(inv.transform_point3(origin)),
            axis: t3(inv.transform_vector3(axis).normalize()),
            u: t3(inv.transform_vector3(u).normalize()),
        })
    }

    /// (asal, sumbu, u, w = sumbu × u) di ruang lokal A.
    pub fn vectors(&self) -> (DVec3, DVec3, DVec3, DVec3) {
        let a = v3(self.axis).try_normalize().unwrap_or(DVec3::Z);
        let u = v3(self.u)
            .try_normalize()
            .unwrap_or_else(|| a.any_orthonormal_vector());
        (v3(self.origin), a, u, a.cross(u))
    }
}

/// Keadaan joint: frame + pose relatif B + nilai joint.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct JointState {
    pub frame: JointFrame,
    pub rel_translation: (f64, f64, f64),
    pub rel_rotation: (f64, f64, f64, f64),
    /// Nilai joint: [0] = nilai utama (sudut ° atau jarak mm — yang
    /// digerakkan motion study dan dijepit limit), [1], [2] = nilai lain.
    #[serde(default)]
    pub values: [f64; 3],
}

/// Gerak lokal (ruang A) joint `kind` untuk `values`.
pub fn local_motion(kind: JointKind, frame: &JointFrame, values: [f64; 3]) -> DAffine3 {
    let (o, a, u, w) = frame.vectors();
    let rot_about = |axis: DVec3, deg: f64| {
        let q = DQuat::from_axis_angle(axis, deg.to_radians());
        DAffine3::from_translation(o) * DAffine3::from_quat(q) * DAffine3::from_translation(-o)
    };
    let shift = DAffine3::from_translation;
    let [v0, v1, v2] = values.map(|v| if v.is_finite() { v } else { 0.0 });
    match kind {
        JointKind::Rigid => DAffine3::IDENTITY,
        JointKind::Revolute => rot_about(a, v0),
        JointKind::Slider | JointKind::Cam => shift(a * v0),
        JointKind::Cylindrical => shift(a * v1) * rot_about(a, v0),
        JointKind::Planar => shift(u * v0 + w * v1) * rot_about(a, v2),
        JointKind::Ball => rot_about(a, v0) * rot_about(u, v1) * rot_about(w, v2),
        JointKind::PinSlot => shift(u * v0) * rot_about(a, v1),
    }
}

impl JointState {
    /// Rekam joint dari pose dunia A dan B SAAT INI dengan `values` sebagai
    /// nilai sekarang (B tidak dipindah — dasar joint as-built).
    pub fn capture(
        kind: JointKind,
        frame: JointFrame,
        a_world: Pose,
        b_world: Pose,
        values: [f64; 3],
    ) -> Self {
        let m = local_motion(kind, &frame, values);
        let rel = (affine(a_world) * m).inverse() * affine(b_world);
        let (t, q) = pose(rel);
        Self {
            frame,
            rel_translation: t3(t),
            rel_rotation: (q.x, q.y, q.z, q.w),
            values,
        }
    }

    fn rel(&self) -> DAffine3 {
        let q = DQuat::from_xyzw(
            self.rel_rotation.0,
            self.rel_rotation.1,
            self.rel_rotation.2,
            self.rel_rotation.3,
        );
        let q = if q.length_squared() > 1e-18 && q.is_finite() { q.normalize() } else { DQuat::IDENTITY };
        DAffine3::from_rotation_translation(q, v3(self.rel_translation))
    }

    /// Pose dunia B yang diminta joint, diberi pose dunia A.
    pub fn target_b(&self, kind: JointKind, a_world: Pose) -> Pose {
        pose(affine(a_world) * local_motion(kind, &self.frame, self.values) * self.rel())
    }

    /// Pose dunia A yang diminta joint, diberi pose dunia B (A digerakkan
    /// bila B di-ground).
    pub fn target_a(&self, kind: JointKind, b_world: Pose) -> Pose {
        pose(affine(b_world) * (local_motion(kind, &self.frame, self.values) * self.rel()).inverse())
    }

    /// Asal + sumbu frame di ruang dunia (untuk penanda joint di viewport).
    pub fn world_axis(&self, a_world: Pose) -> (DVec3, DVec3) {
        let (o, a, _, _) = self.frame.vectors();
        let m = affine(a_world);
        (m.transform_point3(o), m.transform_vector3(a).normalize_or_zero())
    }
}

/// Pelanggaran pose (mm + radian) antara dua pose.
pub fn pose_error(cur: Pose, want: Pose) -> f64 {
    let dq = want.1 * cur.1.inverse();
    let (_, ang) = dq.to_axis_angle();
    let ang = if ang > std::f64::consts::PI {
        std::f64::consts::TAU - ang
    } else {
        ang
    };
    (want.0 - cur.0).length() + ang.abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Pose, b: Pose) -> bool {
        pose_error(a, b) < 1e-9
    }

    #[test]
    fn capture_then_zero_motion_is_identity() {
        let a = (DVec3::new(1.0, 2.0, 3.0), DQuat::from_rotation_z(0.3));
        let b = (DVec3::new(10.0, 0.0, 5.0), DQuat::from_rotation_x(0.7));
        let f = JointFrame::from_world(a, DVec3::new(1.0, 2.0, 3.0), DVec3::Z, None).unwrap();
        for kind in [
            JointKind::Rigid,
            JointKind::Revolute,
            JointKind::Slider,
            JointKind::Cylindrical,
            JointKind::Planar,
            JointKind::Ball,
            JointKind::PinSlot,
            JointKind::Cam,
        ] {
            let s = JointState::capture(kind, f, a, b, [0.0; 3]);
            assert!(close(s.target_b(kind, a), b), "{kind:?}");
            assert!(close(s.target_a(kind, b), a), "{kind:?}");
        }
    }

    #[test]
    fn revolute_rotates_about_axis_and_slider_translates() {
        let a = (DVec3::ZERO, DQuat::IDENTITY);
        let b = (DVec3::new(10.0, 0.0, 0.0), DQuat::IDENTITY);
        let f = JointFrame::from_world(a, DVec3::ZERO, DVec3::Z, None).unwrap();
        let mut s = JointState::capture(JointKind::Revolute, f, a, b, [0.0; 3]);
        s.values[0] = 90.0;
        let (t, _) = s.target_b(JointKind::Revolute, a);
        assert!((t - DVec3::new(0.0, 10.0, 0.0)).length() < 1e-9, "{t}");
        let mut s = JointState::capture(JointKind::Slider, f, a, b, [0.0; 3]);
        s.values[0] = 5.0;
        let (t, _) = s.target_b(JointKind::Slider, a);
        assert!((t - DVec3::new(10.0, 0.0, 5.0)).length() < 1e-9, "{t}");
        // A berputar → B ikut (frame melekat ke A).
        let a2 = (DVec3::ZERO, DQuat::from_rotation_z(std::f64::consts::FRAC_PI_2));
        let s = JointState::capture(JointKind::Rigid, f, a, b, [0.0; 3]);
        let (t, _) = s.target_b(JointKind::Rigid, a2);
        assert!((t - DVec3::new(0.0, 10.0, 0.0)).length() < 1e-9, "{t}");
    }

    #[test]
    fn non_finite_values_are_ignored() {
        let a = (DVec3::ZERO, DQuat::IDENTITY);
        let f = JointFrame::from_world(a, DVec3::ZERO, DVec3::Z, None).unwrap();
        let s = JointState::capture(JointKind::Revolute, f, a, a, [f64::NAN, 0.0, 0.0]);
        assert!(s.target_b(JointKind::Revolute, a).0.is_finite());
        assert!(JointFrame::from_world(a, DVec3::ZERO, DVec3::ZERO, None).is_none());
        assert!(JointFrame::from_world(a, DVec3::NAN, DVec3::Z, None).is_none());
    }
}
