use glam::{Mat4, Vec3};

/// Kamera orbit gaya turntable (CAD): berputar mengelilingi `target`,
/// sumbu Z dunia selalu "atas" — tidak pernah roll, sesuai ekspektasi
/// pengguna AutoCAD/Shapr3D.
#[derive(Debug, Clone)]
pub struct OrbitCamera {
    pub target: Vec3,
    /// Rotasi sekitar sumbu Z (radian).
    pub yaw: f32,
    /// Elevasi dari bidang XY (radian), dibatasi < ±90° agar tidak gimbal-flip.
    pub pitch: f32,
    pub distance: f32,
    pub fov_y: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        // Pose isometrik-ish awal, enak untuk melihat grid.
        Self {
            target: Vec3::ZERO,
            yaw: -45f32.to_radians(),
            pitch: 30f32.to_radians(),
            distance: 250.0,
            fov_y: 45f32.to_radians(),
        }
    }
}

impl OrbitCamera {
    pub fn eye(&self) -> Vec3 {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        self.target
            + self.distance * Vec3::new(cos_pitch * cos_yaw, cos_pitch * sin_yaw, sin_pitch)
    }

    pub fn view(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye(), self.target, Vec3::Z)
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let near = (self.distance * 0.001).max(0.01);
        let far = (self.distance * 100.0).max(10_000.0);
        Mat4::perspective_rh(self.fov_y, aspect.max(0.01), near, far) * self.view()
    }

    /// Putar kamera; delta dalam piksel layar.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        const SENSITIVITY: f32 = 0.008;
        self.yaw -= dx * SENSITIVITY;
        self.pitch = (self.pitch + dy * SENSITIVITY)
            .clamp(-89f32.to_radians(), 89f32.to_radians());
    }

    /// Geser target sejajar bidang layar; delta dalam piksel, diskalakan
    /// agar titik pada depth target mengikuti kursor 1:1.
    pub fn pan(&mut self, dx: f32, dy: f32, viewport_height_px: f32) {
        let world_per_pixel =
            2.0 * self.distance * (self.fov_y * 0.5).tan() / viewport_height_px.max(1.0);
        let forward = (self.target - self.eye()).normalize();
        let right = forward.cross(Vec3::Z).normalize();
        let up = right.cross(forward);
        self.target -= right * dx * world_per_pixel;
        self.target += up * dy * world_per_pixel;
    }

    /// `factor` > 1 mendekat (zoom in). Zoom dengan clamp agar tidak
    /// menembus target atau hilang ke tak-hingga.
    pub fn zoom(&mut self, factor: f32) {
        if factor.is_finite() && factor > 0.0 {
            self.distance = (self.distance / factor).clamp(1.0, 100_000.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eye_respects_distance() {
        let cam = OrbitCamera::default();
        assert!((cam.eye().distance(cam.target) - cam.distance).abs() < 1e-3);
    }

    #[test]
    fn pitch_is_clamped() {
        let mut cam = OrbitCamera::default();
        cam.orbit(0.0, 1e6);
        assert!(cam.pitch <= 89f32.to_radians() + 1e-6);
    }

    #[test]
    fn zoom_never_reaches_zero() {
        let mut cam = OrbitCamera::default();
        for _ in 0..100 {
            cam.zoom(10.0);
        }
        assert!(cam.distance >= 1.0);
    }
}
