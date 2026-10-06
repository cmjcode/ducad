use glam::{Mat4, Vec3};

/// Preset sudut pandang kamera standar CAD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewPreset {
    Top,
    Bottom,
    Front,
    Back,
    Right,
    Left,
    Isometric,
}

/// Mode kamera: Orbit (3D CAD turntable) atau Ortho2D (proyeksi ortografis 2D sejajar bidang sketsa).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum CameraMode {
    #[default]
    Orbit,
    Ortho2D { plane: crate::plane::SketchPlane },
}

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
    pub mode: CameraMode,
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
            mode: CameraMode::Orbit,
        }
    }
}

impl OrbitCamera {
    /// Atur mode kamera (Orbit 3D atau Ortho2D dengan bidang sketsa tertentu).
    pub fn set_mode(&mut self, mode: CameraMode) {
        if let CameraMode::Ortho2D { ref plane } = mode {
            self.orient_to_plane(plane);
        }
        self.mode = mode;
    }

    /// Dapatkan mode kamera saat ini.
    pub fn mode(&self) -> &CameraMode {
        &self.mode
    }

    /// Hitung rasio piksel per milimeter pada ketinggian viewport `viewport_h_px` piksel.
    pub fn px_per_mm(&self, viewport_h_px: f32) -> f32 {
        let world_h_mm = 2.0 * self.distance * (self.fov_y * 0.5).tan();
        viewport_h_px / world_h_mm.max(1e-4)
    }

    pub fn eye(&self) -> Vec3 {
        match &self.mode {
            CameraMode::Orbit => {
                let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
                let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
                self.target
                    + self.distance * Vec3::new(cos_pitch * cos_yaw, cos_pitch * sin_yaw, sin_pitch)
            }
            CameraMode::Ortho2D { plane } => {
                let n = plane.normal.normalize_or_zero();
                let n = if n.length_squared() > 1e-4 { n } else { Vec3::Z };
                self.target + n * self.distance
            }
        }
    }

    pub fn view(&self) -> Mat4 {
        match &self.mode {
            CameraMode::Orbit => {
                glam::camera::rh::view::look_at_mat4(self.eye(), self.target, Vec3::Z)
            }
            CameraMode::Ortho2D { plane } => {
                let up = plane.v_axis.normalize_or_zero();
                let up = if up.length_squared() > 1e-4 { up } else { Vec3::Y };
                glam::camera::rh::view::look_at_mat4(self.eye(), self.target, up)
            }
        }
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        match &self.mode {
            CameraMode::Orbit => {
                let near = (self.distance * 0.001).max(0.01);
                let far = (self.distance * 100.0).max(10_000.0);
                glam::camera::rh::proj::directx::perspective(self.fov_y, aspect.max(0.01), near, far)
                    * self.view()
            }
            CameraMode::Ortho2D { .. } => {
                let half_h = self.distance * (self.fov_y * 0.5).tan();
                let half_w = half_h * aspect.max(0.01);
                let near = -10_000.0;
                let far = 10_000.0;
                glam::camera::rh::proj::directx::orthographic(
                    -half_w, half_w, -half_h, half_h, near, far,
                ) * self.view()
            }
        }
    }

    /// Putar kamera; delta dalam piksel layar.
    /// Diabaikan bila dalam mode Ortho2D (orbit dikunci).
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        if matches!(self.mode, CameraMode::Ortho2D { .. }) {
            return;
        }
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
        match &self.mode {
            CameraMode::Orbit => {
                let forward = (self.target - self.eye()).normalize_or_zero();
                let right = forward.cross(Vec3::Z).normalize_or_zero();
                let up = right.cross(forward).normalize_or_zero();
                self.target -= right * dx * world_per_pixel;
                self.target += up * dy * world_per_pixel;
            }
            CameraMode::Ortho2D { plane } => {
                let right = plane.u_axis.normalize_or_zero();
                let up = plane.v_axis.normalize_or_zero();
                self.target -= right * dx * world_per_pixel;
                self.target += up * dy * world_per_pixel;
            }
        }
    }

    /// `factor` > 1 mendekat (zoom in).
    pub fn zoom(&mut self, factor: f32) {
        self.zoom_at(factor, glam::Vec2::ZERO, 1.0);
    }

    /// Zoom mengelilingi titik kursor dalam koordinat NDC layar (-1.0..1.0).
    /// Menjaga titik di bawah kursor tetap berada di posisi yang sama sebelum dan sesudah zoom.
    /// Batas zoom: jarak dijaga pada ekuivalen 0.01 - 1000 px/mm.
    pub fn zoom_at(&mut self, factor: f32, cursor_ndc: glam::Vec2, aspect: f32) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }

        let old_distance = self.distance;
        // Rentang jarak 0.5 mm .. 150,000 mm (mencakup 0.01 - 1000 px/mm pada viewport standar)
        let new_distance = (self.distance / factor).clamp(0.5, 150_000.0);
        if (new_distance - old_distance).abs() < 1e-6 {
            return;
        }

        let half_h_old = old_distance * (self.fov_y * 0.5).tan();
        let half_w_old = half_h_old * aspect.max(0.01);

        let half_h_new = new_distance * (self.fov_y * 0.5).tan();
        let half_w_new = half_h_new * aspect.max(0.01);

        let delta_w = half_w_old - half_w_new;
        let delta_h = half_h_old - half_h_new;

        match &self.mode {
            CameraMode::Orbit => {
                let forward = (self.target - self.eye()).normalize_or_zero();
                let right = forward.cross(Vec3::Z).normalize_or_zero();
                let up = right.cross(forward).normalize_or_zero();
                self.target += right * cursor_ndc.x * delta_w + up * cursor_ndc.y * delta_h;
            }
            CameraMode::Ortho2D { plane } => {
                let right = plane.u_axis.normalize_or_zero();
                let up = plane.v_axis.normalize_or_zero();
                self.target += right * cursor_ndc.x * delta_w + up * cursor_ndc.y * delta_h;
            }
        }

        self.distance = new_distance;
    }

    /// Terapkan preset orientasi kamera standar CAD.
    pub fn set_preset(&mut self, preset: ViewPreset) {
        match preset {
            ViewPreset::Top => {
                self.yaw = -90f32.to_radians();
                self.pitch = 89f32.to_radians();
            }
            ViewPreset::Bottom => {
                self.yaw = -90f32.to_radians();
                self.pitch = -89f32.to_radians();
            }
            ViewPreset::Front => {
                self.yaw = -90f32.to_radians();
                self.pitch = 0.0;
            }
            ViewPreset::Back => {
                self.yaw = 90f32.to_radians();
                self.pitch = 0.0;
            }
            ViewPreset::Right => {
                self.yaw = 0.0;
                self.pitch = 0.0;
            }
            ViewPreset::Left => {
                self.yaw = 180f32.to_radians();
                self.pitch = 0.0;
            }
            ViewPreset::Isometric => {
                self.yaw = -45f32.to_radians();
                self.pitch = 35.264f32.to_radians();
            }
        }
    }

    /// Selaraskan pandangan tegak lurus ke bidang gambar sketch (XY Z-up).
    pub fn orient_to_sketch(&mut self) {
        self.set_preset(ViewPreset::Top);
    }

    /// Selaraskan pandangan tegak lurus ke bidang sketsa tertentu (`Top`, `Front`, `Right`, atau `Custom`).
    pub fn orient_to_plane(&mut self, plane: &crate::plane::SketchPlane) {
        let norm = plane.normal.normalize_or_zero();
        if norm.length_squared() > 1e-6 {
            self.target = plane.origin;
            let pitch = norm.z.clamp(-0.9999, 0.9999).asin();
            let yaw = norm.x.atan2(-norm.y);
            self.pitch = pitch;
            self.yaw = yaw;
        } else {
            self.set_preset(plane.camera_preset());
        }
    }

    /// Persentase zoom relatif terhadap "ukuran sebenarnya" (100% = 1 mm model
    /// tampil 1 mm di layar, asumsi 96 titik/inci) untuk tinggi viewport
    /// `viewport_h_px`. Dipakai indikator % zoom di header.
    pub fn zoom_percent(&self, viewport_h_px: f32) -> f32 {
        self.px_per_mm(viewport_h_px.max(1.0)) / PX_PER_MM_ACTUAL_SIZE * 100.0
    }

    /// Atur zoom ke `percent` (lihat `zoom_percent`) sambil mempertahankan
    /// `target` di tengah layar. Nilai tidak finit atau <= 0 diabaikan; jarak
    /// tetap dibatasi oleh `zoom_at`.
    pub fn set_zoom_percent(&mut self, percent: f32, viewport_h_px: f32) {
        if !percent.is_finite() || percent <= 0.0 {
            return;
        }
        let current = self.zoom_percent(viewport_h_px);
        if current <= 0.0 || !current.is_finite() {
            return;
        }
        self.zoom(percent / current);
    }

    /// Geser `target` ke pusat kotak `(min, max)` dan atur jarak supaya seluruh
    /// kotak muat di viewport beraspek `aspect` dengan sedikit margin. Orientasi
    /// (yaw/pitch/mode) tidak berubah. Kotak degenerate (titik) diberi radius
    /// minimum supaya tidak zoom tak hingga.
    pub fn fit_bounds(&mut self, min: Vec3, max: Vec3, aspect: f32) {
        if !(min.is_finite() && max.is_finite()) {
            return;
        }
        let lo = min.min(max);
        let hi = min.max(max);
        self.target = (lo + hi) * 0.5;
        // Bola pembungkus: aman untuk orientasi kamera apa pun.
        let radius = ((hi - lo).length() * 0.5).max(1.0);
        // Sumbu layar tersempit yang membatasi: bila aspect < 1 (portrait),
        // lebar setengah layar = half_h * aspect harus >= radius.
        let half_h = radius / aspect.clamp(0.01, 1.0) * FIT_MARGIN;
        let distance = half_h / (self.fov_y * 0.5).tan();
        self.distance = distance.clamp(0.5, 150_000.0);
    }
}

/// Piksel (titik egui) per milimeter pada zoom 100%: 96 titik/inci ÷ 25,4 mm/inci.
pub const PX_PER_MM_ACTUAL_SIZE: f32 = 96.0 / 25.4;

/// Margin tambahan saat "pas ke layar" supaya objek tidak menempel tepi.
const FIT_MARGIN: f32 = 1.15;

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
        assert!(cam.distance >= 0.5);
    }

    #[test]
    fn zoom_percent_roundtrips_through_set_zoom_percent() {
        let mut cam = OrbitCamera::default();
        let vp_h = 900.0;
        cam.set_zoom_percent(100.0, vp_h);
        assert!((cam.zoom_percent(vp_h) - 100.0).abs() < 1e-2);
        // 100% = 1 mm model tampil ~3,78 px.
        assert!((cam.px_per_mm(vp_h) - PX_PER_MM_ACTUAL_SIZE).abs() < 1e-3);

        cam.set_zoom_percent(250.0, vp_h);
        assert!((cam.zoom_percent(vp_h) - 250.0).abs() < 1e-2);

        // Target tidak bergeser: zoom dipusatkan di tengah layar.
        assert!(cam.target.length() < 1e-6);
    }

    #[test]
    fn set_zoom_percent_ignores_invalid_values() {
        let mut cam = OrbitCamera::default();
        let before = cam.distance;
        cam.set_zoom_percent(0.0, 900.0);
        cam.set_zoom_percent(-5.0, 900.0);
        cam.set_zoom_percent(f32::NAN, 900.0);
        assert_eq!(cam.distance, before);
    }

    #[test]
    fn fit_bounds_centers_target_and_contains_box() {
        let mut cam = OrbitCamera::default();
        let min = Vec3::new(-10.0, -20.0, 0.0);
        let max = Vec3::new(30.0, 20.0, 50.0);
        cam.fit_bounds(min, max, 16.0 / 9.0);
        assert!((cam.target - Vec3::new(10.0, 0.0, 25.0)).length() < 1e-4);
        // Setiap sudut kotak harus berada di dalam frustum (NDC dalam [-1, 1]).
        let vp = cam.view_proj(16.0 / 9.0);
        for &x in &[min.x, max.x] {
            for &y in &[min.y, max.y] {
                for &z in &[min.z, max.z] {
                    let clip = vp * Vec3::new(x, y, z).extend(1.0);
                    let ndc = clip.truncate() / clip.w;
                    assert!(ndc.x.abs() <= 1.0 && ndc.y.abs() <= 1.0, "sudut di luar layar: {ndc:?}");
                }
            }
        }
    }

    #[test]
    fn fit_bounds_handles_degenerate_box() {
        let mut cam = OrbitCamera::default();
        cam.fit_bounds(Vec3::ONE, Vec3::ONE, 1.0);
        assert!(cam.distance.is_finite() && cam.distance >= 0.5);
        let d = cam.distance;
        cam.fit_bounds(Vec3::NAN, Vec3::ONE, 1.0);
        assert_eq!(cam.distance, d);
    }

    #[test]
    fn presets_apply_correctly() {
        let mut cam = OrbitCamera::default();
        cam.set_preset(ViewPreset::Top);
        assert!((cam.pitch - 89f32.to_radians()).abs() < 1e-4);

        cam.set_preset(ViewPreset::Front);
        assert!(cam.pitch.abs() < 1e-4);
        assert!((cam.yaw - (-90f32.to_radians())).abs() < 1e-4);

        cam.orient_to_sketch();
        assert!((cam.pitch - 89f32.to_radians()).abs() < 1e-4);
    }

    #[test]
    fn ortho2d_ignores_orbit_input() {
        let mut cam = OrbitCamera::default();
        let plane = crate::plane::SketchPlane::top();
        cam.set_mode(CameraMode::Ortho2D { plane });

        let initial_pitch = cam.pitch;
        let initial_yaw = cam.yaw;

        cam.orbit(50.0, 50.0);

        assert_eq!(cam.pitch, initial_pitch);
        assert_eq!(cam.yaw, initial_yaw);
    }

    #[test]
    fn zoom_keeps_cursor_point_fixed() {
        let mut cam = OrbitCamera::default();
        let plane = crate::plane::SketchPlane::top();
        cam.set_mode(CameraMode::Ortho2D { plane });

        let aspect = 1.6;
        let cursor_ndc = glam::Vec2::new(0.4, -0.6);

        let half_h = cam.distance * (cam.fov_y * 0.5).tan();
        let half_w = half_h * aspect;
        let p_world = cam.target
            + glam::Vec3::X * (cursor_ndc.x * half_w)
            + glam::Vec3::Y * (cursor_ndc.y * half_h);

        cam.zoom_at(2.5, cursor_ndc, aspect);

        let vp = cam.view_proj(aspect);
        let clip = vp * p_world.extend(1.0);
        let ndc_after = clip.truncate() / clip.w;

        assert!(
            (ndc_after.x - cursor_ndc.x).abs() < 1e-4,
            "x mismatch: {} vs {}",
            ndc_after.x,
            cursor_ndc.x
        );
        assert!(
            (ndc_after.y - cursor_ndc.y).abs() < 1e-4,
            "y mismatch: {} vs {}",
            ndc_after.y,
            cursor_ndc.y
        );
    }

    #[test]
    fn px_per_mm_matches_projection() {
        let mut cam = OrbitCamera::default();
        let plane = crate::plane::SketchPlane::top();
        cam.set_mode(CameraMode::Ortho2D { plane });

        let viewport_h_px = 800.0;
        let aspect = 1.5;
        let px_mm = cam.px_per_mm(viewport_h_px);

        let vp = cam.view_proj(aspect);
        let p0 = vp * glam::Vec4::new(0.0, 0.0, 0.0, 1.0);
        let p1 = vp * glam::Vec4::new(0.0, 1.0, 0.0, 1.0);
        let dy_ndc = (p1.y / p1.w) - (p0.y / p0.w);
        let dy_px = (dy_ndc.abs() * 0.5) * viewport_h_px;

        assert!(
            (dy_px - px_mm).abs() < 1e-3,
            "dy_px ({dy_px}) vs px_mm ({px_mm})"
        );
    }
}
