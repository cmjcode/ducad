use glam::{DVec2, Quat, Vec3};
use crate::camera::ViewPreset;

/// Jenis bidang referensi datum standar atau kustom CAD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum PlaneKind {
    #[default]
    Top,   // XY (Horizontal)
    Front, // XZ (Vertikal Depan)
    Right, // YZ (Vertikal Samping)
    Custom(u32), // Bidang referensi kustom (Datum Plane ID)
}

impl PlaneKind {
    pub fn name(self) -> &'static str {
        match self {
            PlaneKind::Top => "Top (XY)",
            PlaneKind::Front => "Front (XZ)",
            PlaneKind::Right => "Right (YZ)",
            PlaneKind::Custom(_) => "Datum Plane",
        }
    }

    pub fn display_label(self) -> &'static str {
        match self {
            PlaneKind::Top => "Top Plane (XY)",
            PlaneKind::Front => "Front Plane (XZ)",
            PlaneKind::Right => "Right Plane (YZ)",
            PlaneKind::Custom(_) => "Datum Plane",
        }
    }

    pub fn all() -> [PlaneKind; 3] {
        [PlaneKind::Top, PlaneKind::Front, PlaneKind::Right]
    }
}

/// Struktur data bidang referensi kustom (Datum Plane).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DatumPlane {
    pub id: u32,
    pub name: String,
    pub plane: SketchPlane,
    pub visible: bool,
}

impl DatumPlane {
    pub fn new(id: u32, name: impl Into<String>, plane: SketchPlane) -> Self {
        Self {
            id,
            name: name.into(),
            plane,
            visible: true,
        }
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
            PlaneKind::Custom(_) => Self::top(),
        }
    }

    /// Bangun bidang kustom dengan parameter lengkap `(origin, u_axis, v_axis, normal)`.
    pub fn custom(id: u32, origin: Vec3, u_axis: Vec3, v_axis: Vec3, normal: Vec3) -> Self {
        let u = u_axis.normalize_or_zero();
        let n = normal.normalize_or_zero();
        let v = if v_axis.length_squared() > 1e-6 {
            v_axis.normalize()
        } else {
            n.cross(u).normalize_or_zero()
        };
        Self {
            kind: PlaneKind::Custom(id),
            origin,
            u_axis: u,
            v_axis: v,
            normal: n,
        }
    }

    /// 1. Offset Plane: Buat bidang baru sejajar berjarak `distance` (mm) dari bidang ini.
    pub fn offset(&self, distance: f32) -> Self {
        let origin = self.origin + self.normal * distance;
        Self {
            kind: self.kind,
            origin,
            u_axis: self.u_axis,
            v_axis: self.v_axis,
            normal: self.normal,
        }
    }

    /// 1b. Offset Plane dari permukaan datar (face planar).
    pub fn from_face_offset(face_origin: Vec3, face_normal: Vec3, distance: f32) -> Self {
        let norm = face_normal.normalize_or_zero();
        let normal = if norm.length_squared() > 1e-6 { norm } else { Vec3::Z };
        let mut plane = Self::from_origin_normal(face_origin, normal);
        if distance.abs() > 1e-6 {
            plane.origin += normal * distance;
        }
        plane
    }

    /// 2. Angled Plane: Memutar `angle_deg` derajat terhadap garis linier edge (p1 -> p2).
    pub fn from_angle_and_edge(
        p1: Vec3,
        p2: Vec3,
        ref_normal: Vec3,
        angle_deg: f32,
    ) -> Self {
        let edge_vec = p2 - p1;
        let u_axis = if edge_vec.length_squared() > 1e-6 {
            edge_vec.normalize()
        } else {
            Vec3::X
        };

        let ref_norm = ref_normal.normalize_or_zero();
        let ref_norm = if ref_norm.length_squared() > 1e-6 && ref_norm.dot(u_axis).abs() < 0.99 {
            (ref_norm - u_axis * ref_norm.dot(u_axis)).normalize()
        } else {
            let alt = if u_axis.z.abs() < 0.9 { Vec3::Z } else { Vec3::Y };
            u_axis.cross(alt).normalize()
        };

        // Rotasi mengelilingi sumbu edge (u_axis)
        let quat = Quat::from_axis_angle(u_axis, angle_deg.to_radians());
        let normal = (quat * ref_norm).normalize();
        let v_axis = normal.cross(u_axis).normalize();

        Self {
            kind: PlaneKind::Custom(0),
            origin: p1,
            u_axis,
            v_axis,
            normal,
        }
    }

    /// 3. 3-Point Plane: Membentuk bidang datar yang melalui 3 titik vertex 3D non-kolinear.
    pub fn from_3_points(p1: Vec3, p2: Vec3, p3: Vec3) -> Option<Self> {
        let v12 = p2 - p1;
        let v13 = p3 - p1;
        let normal_raw = v12.cross(v13);
        if normal_raw.length_squared() < 1e-8 {
            return None; // 3 titik kolinear atau tumpang tindih
        }
        let normal = normal_raw.normalize();
        let u_axis = v12.normalize();
        let v_axis = normal.cross(u_axis).normalize();

        Some(Self {
            kind: PlaneKind::Custom(0),
            origin: p1,
            u_axis,
            v_axis,
            normal,
        })
    }

    /// Bangun `SketchPlane` ortogonal dari titik awal dan dua sumbu lokal U dan V.
    pub fn from_origin_u_v(origin: Vec3, u_axis: Vec3, v_axis: Vec3) -> Option<Self> {
        let u = u_axis.normalize_or_zero();
        if u.length_squared() < 1e-6 {
            return None;
        }
        let normal_raw = u.cross(v_axis);
        if normal_raw.length_squared() < 1e-6 {
            return None;
        }
        let normal = normal_raw.normalize();
        let v = normal.cross(u).normalize();
        Some(Self {
            kind: PlaneKind::Custom(0),
            origin,
            u_axis: u,
            v_axis: v,
            normal,
        })
    }

    /// Bangun `SketchPlane` pada posisi sembarang dari `origin` dan `normal`.
    pub fn from_origin_normal(origin: Vec3, normal: Vec3) -> Self {
        let norm = normal.normalize_or_zero();
        let normal = if norm.length_squared() > 1e-6 { norm } else { Vec3::Z };

        let (kind, arbitrary) = if normal.z.abs() > 0.8 {
            (PlaneKind::Top, Vec3::Y)
        } else if normal.y.abs() > 0.8 {
            (PlaneKind::Front, Vec3::Z)
        } else {
            (PlaneKind::Right, Vec3::Z)
        };

        let mut u_axis = arbitrary.cross(normal).normalize_or_zero();
        if u_axis.length_squared() < 1e-6 {
            let alt_arbitrary = if normal.x.abs() > 0.8 { Vec3::Y } else { Vec3::X };
            u_axis = alt_arbitrary.cross(normal).normalize_or_zero();
        }
        let v_axis = normal.cross(u_axis).normalize();

        Self {
            kind,
            origin,
            u_axis,
            v_axis,
            normal,
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

    /// Proyeksikan titik 3D dunia ke titik 2D lokal (u, v) pada bidang ini.
    pub fn project_point_to_uv(&self, world_pt: Vec3) -> DVec2 {
        let diff = world_pt - self.origin;
        let u = diff.dot(self.u_axis) as f64;
        let v = diff.dot(self.v_axis) as f64;
        DVec2::new(u, v)
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
        if self.normal.z > 0.5 {
            ViewPreset::Top
        } else if self.normal.z < -0.5 {
            ViewPreset::Bottom
        } else if self.normal.y < -0.5 {
            ViewPreset::Front
        } else if self.normal.y > 0.5 {
            ViewPreset::Back
        } else if self.normal.x > 0.5 {
            ViewPreset::Right
        } else if self.normal.x < -0.5 {
            ViewPreset::Left
        } else {
            match self.kind {
                PlaneKind::Top => ViewPreset::Top,
                PlaneKind::Front => ViewPreset::Front,
                PlaneKind::Right => ViewPreset::Right,
                PlaneKind::Custom(_) => ViewPreset::Top,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offset_plane() {
        let top = SketchPlane::top();
        let offset_plane = top.offset(25.0);
        assert_eq!(offset_plane.origin, Vec3::new(0.0, 0.0, 25.0));
        assert_eq!(offset_plane.normal, Vec3::Z);
        assert_eq!(offset_plane.u_axis, Vec3::X);
        assert_eq!(offset_plane.v_axis, Vec3::Y);
    }

    #[test]
    fn test_angled_plane() {
        let p1 = Vec3::ZERO;
        let p2 = Vec3::new(10.0, 0.0, 0.0);
        let ref_normal = Vec3::Z;
        let angled = SketchPlane::from_angle_and_edge(p1, p2, ref_normal, 45.0);

        assert!((angled.u_axis - Vec3::X).length() < 1e-5);
        assert!(angled.u_axis.dot(angled.normal).abs() < 1e-5);
        assert!(angled.u_axis.dot(angled.v_axis).abs() < 1e-5);
        assert!(angled.v_axis.dot(angled.normal).abs() < 1e-5);

        // 45 degrees rotated around X axis from +Z
        let expected_normal = Vec3::new(0.0, -45f32.to_radians().sin(), 45f32.to_radians().cos()).normalize();
        assert!((angled.normal - expected_normal).length() < 1e-4);
    }

    #[test]
    fn test_three_point_plane() {
        let p1 = Vec3::new(0.0, 0.0, 0.0);
        let p2 = Vec3::new(10.0, 0.0, 0.0);
        let p3 = Vec3::new(0.0, 10.0, 10.0);

        let plane = SketchPlane::from_3_points(p1, p2, p3).expect("valid plane");
        assert_eq!(plane.origin, p1);
        assert!(plane.u_axis.dot(plane.normal).abs() < 1e-5);
        assert!(plane.v_axis.dot(plane.normal).abs() < 1e-5);

        // Collinear test
        let col_p3 = Vec3::new(20.0, 0.0, 0.0);
        assert!(SketchPlane::from_3_points(p1, p2, col_p3).is_none());
    }

    #[test]
    fn test_project_point_to_uv() {
        let top = SketchPlane::top();
        let uv = top.project_point_to_uv(Vec3::new(12.5, -7.0, 50.0));
        assert!((uv.x - 12.5).abs() < 1e-5);
        assert!((uv.y - (-7.0)).abs() < 1e-5);
    }

    #[test]
    fn test_from_origin_normal_valid_axes() {
        let origins = [Vec3::ZERO, Vec3::new(10.0, 20.0, 30.0)];
        let normals = [
            Vec3::Z,
            -Vec3::Z,
            Vec3::Y,
            -Vec3::Y,
            Vec3::X,
            -Vec3::X,
            Vec3::new(1.0, 1.0, 1.0).normalize(),
        ];

        for origin in origins {
            for normal in normals {
                let plane = SketchPlane::from_origin_normal(origin, normal);
                assert!(!plane.u_axis.is_nan(), "u_axis is NaN for normal {:?}", normal);
                assert!(!plane.v_axis.is_nan(), "v_axis is NaN for normal {:?}", normal);
                assert!(plane.u_axis.length_squared() > 0.99, "u_axis not unit length for normal {:?}", normal);
                assert!(plane.v_axis.length_squared() > 0.99, "v_axis not unit length for normal {:?}", normal);
                assert!(
                    plane.u_axis.dot(plane.v_axis).abs() < 1e-4,
                    "u and v not orthogonal for normal {:?}",
                    normal
                );
                assert!(
                    plane.u_axis.dot(plane.normal).abs() < 1e-4,
                    "u and normal not orthogonal for normal {:?}",
                    normal
                );
                assert!(
                    plane.v_axis.dot(plane.normal).abs() < 1e-4,
                    "v and normal not orthogonal for normal {:?}",
                    normal
                );
            }
        }
    }

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

