use glam::{DVec2, Vec3};
use crate::camera::ViewPreset;

/// Jenis bidang referensi datum standar CAD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum PlaneKind {
    #[default]
    Top,   // XY (Horizontal)
    Front, // XZ (Vertikal Depan)
    Right, // YZ (Vertikal Samping)
}

impl PlaneKind {
    pub fn name(self) -> &'static str {
        match self {
            PlaneKind::Top => "Top (XY)",
            PlaneKind::Front => "Front (XZ)",
            PlaneKind::Right => "Right (YZ)",
        }
    }

    pub fn display_label(self) -> &'static str {
        match self {
            PlaneKind::Top => "Top Plane (XY)",
            PlaneKind::Front => "Front Plane (XZ)",
            PlaneKind::Right => "Right Plane (YZ)",
        }
    }

    pub fn all() -> [PlaneKind; 3] {
        [PlaneKind::Top, PlaneKind::Front, PlaneKind::Right]
    }
}

/// Definisi bidang kerja sketsa (workplane / datum plane) dalam ruang 3D dunia.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SketchPlane {
    pub kind: PlaneKind,
    pub origin: Vec3,
    pub u_axis: Vec3,
    pub v_axis: Vec3,
    pub normal: Vec3,
}

impl Default for SketchPlane {
    fn default() -> Self {
        Self::top()
    }
}

impl SketchPlane {
    /// Bidang Horizontal Top (XY plane, normal +Z).
    pub fn top() -> Self {
        Self {
            kind: PlaneKind::Top,
            origin: Vec3::ZERO,
            u_axis: Vec3::X,
            v_axis: Vec3::Y,
            normal: Vec3::Z,
        }
    }

    /// Bidang Vertikal Front (XZ plane, normal -Y).
    pub fn front() -> Self {
        Self {
            kind: PlaneKind::Front,
            origin: Vec3::ZERO,
            u_axis: Vec3::X,
            v_axis: Vec3::Z,
            normal: Vec3::new(0.0, -1.0, 0.0),
        }
    }

    /// Bidang Vertikal Right (YZ plane, normal +X).
    pub fn right() -> Self {
        Self {
            kind: PlaneKind::Right,
            origin: Vec3::ZERO,
            u_axis: Vec3::Y,
            v_axis: Vec3::Z,
            normal: Vec3::X,
        }
    }

    /// Bangun `SketchPlane` dari `PlaneKind`.
    pub fn from_kind(kind: PlaneKind) -> Self {
        match kind {
            PlaneKind::Top => Self::top(),
            PlaneKind::Front => Self::front(),
            PlaneKind::Right => Self::right(),
        }
    }

    pub fn name(&self) -> &'static str {
        self.kind.name()
    }

    pub fn display_label(&self) -> &'static str {
        self.kind.display_label()
    }

    /// Konversi titik 2D lokal sketsa (u, v) ke koordinat 3D dunia dengan offset tebal `z_offset`.
    pub fn to_world(&self, p_2d: DVec2, z_offset: f32) -> Vec3 {
        self.origin
            + self.u_axis * (p_2d.x as f32)
            + self.v_axis * (p_2d.y as f32)
            + self.normal * z_offset
    }

    /// Konversi titik 2D lokal sketsa (u, v) ke `[f64; 3]` presisi tinggi untuk kernel.
    pub fn to_world_f64(&self, p_2d: (f64, f64), offset: f64) -> [f64; 3] {
        let ox = self.origin.x as f64;
        let oy = self.origin.y as f64;
        let oz = self.origin.z as f64;
        let ux = self.u_axis.x as f64;
        let uy = self.u_axis.y as f64;
        let uz = self.u_axis.z as f64;
        let vx = self.v_axis.x as f64;
        let vy = self.v_axis.y as f64;
        let vz = self.v_axis.z as f64;
        let nx = self.normal.x as f64;
        let ny = self.normal.y as f64;
        let nz = self.normal.z as f64;

        [
            ox + ux * p_2d.0 + vx * p_2d.1 + nx * offset,
            oy + uy * p_2d.0 + vy * p_2d.1 + ny * offset,
            oz + uz * p_2d.0 + vz * p_2d.1 + nz * offset,
        ]
    }

    /// Interseksi ray unprojection kamera (p_near + t * dir) ke bidang ini.
    /// Mengembalikan titik 2D lokal (u, v) pada bidang jika ada perpotongan.
    pub fn ray_intersection(&self, p_near: Vec3, dir: Vec3) -> Option<DVec2> {
        let denom = dir.dot(self.normal);
        if denom.abs() < 1e-6 {
            return None; // Ray sejajar dengan bidang
        }
        let t = (self.origin - p_near).dot(self.normal) / denom;
        let hit = p_near + dir * t;
        let diff = hit - self.origin;
        let u = diff.dot(self.u_axis);
        let v = diff.dot(self.v_axis);
        Some(DVec2::new(u as f64, v as f64))
    }

    /// Preset orientasi kamera standar CAD untuk memandang tegak lurus ke bidang ini.
    pub fn camera_preset(&self) -> ViewPreset {
        match self.kind {
            PlaneKind::Top => ViewPreset::Top,
            PlaneKind::Front => ViewPreset::Front,
            PlaneKind::Right => ViewPreset::Right,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_top_plane_projection() {
        let plane = SketchPlane::top();
        let pt = plane.to_world(DVec2::new(10.0, 20.0), 0.0);
        assert!((pt.x - 10.0).abs() < 1e-5);
        assert!((pt.y - 20.0).abs() < 1e-5);
        assert!((pt.z - 0.0).abs() < 1e-5);

        let pt64 = plane.to_world_f64((10.0, 20.0), 5.0);
        assert_eq!(pt64, [10.0, 20.0, 5.0]);

        let ray_near = Vec3::new(10.0, 20.0, 100.0);
        let ray_dir = Vec3::new(0.0, 0.0, -1.0);
        let hit = plane.ray_intersection(ray_near, ray_dir).unwrap();
        assert!((hit.x - 10.0).abs() < 1e-5);
        assert!((hit.y - 20.0).abs() < 1e-5);

        assert_eq!(plane.camera_preset(), ViewPreset::Top);
    }

    #[test]
    fn test_front_plane_projection() {
        let plane = SketchPlane::front();
        let pt = plane.to_world(DVec2::new(15.0, 25.0), 0.0);
        assert!((pt.x - 15.0).abs() < 1e-5);
        assert!((pt.y - 0.0).abs() < 1e-5);
        assert!((pt.z - 25.0).abs() < 1e-5);

        let pt64 = plane.to_world_f64((15.0, 25.0), 5.0);
        assert_eq!(pt64, [15.0, -5.0, 25.0]);

        let ray_near = Vec3::new(15.0, -100.0, 25.0);
        let ray_dir = Vec3::new(0.0, 1.0, 0.0);
        let hit = plane.ray_intersection(ray_near, ray_dir).unwrap();
        assert!((hit.x - 15.0).abs() < 1e-5);
        assert!((hit.y - 25.0).abs() < 1e-5);

        assert_eq!(plane.camera_preset(), ViewPreset::Front);
    }

    #[test]
    fn test_right_plane_projection() {
        let plane = SketchPlane::right();
        let pt = plane.to_world(DVec2::new(30.0, 40.0), 0.0);
        assert!((pt.x - 0.0).abs() < 1e-5);
        assert!((pt.y - 30.0).abs() < 1e-5);
        assert!((pt.z - 40.0).abs() < 1e-5);

        let pt64 = plane.to_world_f64((30.0, 40.0), 5.0);
        assert_eq!(pt64, [5.0, 30.0, 40.0]);

        let ray_near = Vec3::new(100.0, 30.0, 40.0);
        let ray_dir = Vec3::new(-1.0, 0.0, 0.0);
        let hit = plane.ray_intersection(ray_near, ray_dir).unwrap();
        assert!((hit.x - 30.0).abs() < 1e-5);
        assert!((hit.y - 40.0).abs() < 1e-5);

        assert_eq!(plane.camera_preset(), ViewPreset::Right);
    }

    #[test]
    fn test_parallel_ray_returns_none() {
        let plane = SketchPlane::front();
        let ray_near = Vec3::new(0.0, -10.0, 0.0);
        let ray_dir = Vec3::new(1.0, 0.0, 0.0); // Sejajar sumbu X / dalam bidang XZ
        assert!(plane.ray_intersection(ray_near, ray_dir).is_none());
    }
}
