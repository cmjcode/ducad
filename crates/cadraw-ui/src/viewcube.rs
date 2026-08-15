//! Interactive 3D ViewCube & Orientation Gizmo bergaya Shapr3D / Fusion 360.
//!
//! Merender kubus 3D navigasi di pojok kanan atas kanvas yang berputar
//! sinkron dengan orientasi kamera (yaw & pitch). Mengklik salah satu
//! permukaan (TOP, FRONT, RIGHT, BACK, LEFT, BOTTOM) mengembalikan
//! preset orientasi kamera terkait.

use egui::{Color32, FontId, Pos2, Rect, Stroke, Vec2};
use glam::Vec3;

/// Preset sudut pandang kamera yang dihasilkan saat mengklik ViewCube.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewCubeAction {
    Top,
    Bottom,
    Front,
    Back,
    Right,
    Left,
    Isometric,
}

pub struct ViewCube {
    size: f32,
}

impl Default for ViewCube {
    fn default() -> Self {
        Self { size: 54.0 }
    }
}

struct FaceDef {
    name: &'static str,
    action: ViewCubeAction,
    normal: Vec3,
    corners: [Vec3; 4],
}

impl ViewCube {
    pub fn new() -> Self {
        Self::default()
    }

    /// Render ViewCube di area tertentu. Mengembalikan `Some(action)` jika pengguna mengklik permukaan kubus.
    pub fn show(
        &self,
        ui: &mut egui::Ui,
        center: Pos2,
        camera_yaw: f32,
        camera_pitch: f32,
    ) -> Option<ViewCubeAction> {
        let rect = Rect::from_center_size(center, Vec2::splat(self.size * 1.6));
        let response = ui.allocate_rect(rect, egui::Sense::click());
        let painter = ui.painter_at(rect);

        // Vektor basis kamera turntable (Z-up, target di origin ViewCube):
        // eye_dir = arah dari target ke mata kamera
        let (sin_y, cos_y) = camera_yaw.sin_cos();
        let (sin_p, cos_p) = camera_pitch.sin_cos();

        let eye_dir = Vec3::new(cos_p * cos_y, cos_p * sin_y, sin_p).normalize();
        let cam_right = Vec3::new(-sin_y, cos_y, 0.0);
        let cam_up = Vec3::new(-sin_p * cos_y, -sin_p * sin_y, cos_p);

        // Proyeksi ortografis koordinat 3D ke 2D canvas UI (egui Y ke bawah)
        let project = |p: Vec3| -> (Pos2, f32) {
            let sx = p.dot(cam_right);
            let sy = -p.dot(cam_up);
            let sz = p.dot(eye_dir);
            (center + Vec2::new(sx, sy), sz)
        };

        let h = self.size * 0.5;
        let faces = [
            // Top (+Z)
            FaceDef {
                name: "TOP",
                action: ViewCubeAction::Top,
                normal: Vec3::Z,
                corners: [
                    Vec3::new(-h, -h, h),
                    Vec3::new(h, -h, h),
                    Vec3::new(h, h, h),
                    Vec3::new(-h, h, h),
                ],
            },
            // Bottom (-Z)
            FaceDef {
                name: "BTM",
                action: ViewCubeAction::Bottom,
                normal: -Vec3::Z,
                corners: [
                    Vec3::new(-h, h, -h),
                    Vec3::new(h, h, -h),
                    Vec3::new(h, -h, -h),
                    Vec3::new(-h, -h, -h),
                ],
            },
            // Front (-Y) -> Pada yaw=-90°, mata kamera ada di -Y melihat ke +Y
            FaceDef {
                name: "FRONT",
                action: ViewCubeAction::Front,
                normal: -Vec3::Y,
                corners: [
                    Vec3::new(-h, -h, -h),
                    Vec3::new(h, -h, -h),
                    Vec3::new(h, -h, h),
                    Vec3::new(-h, -h, h),
                ],
            },
            // Back (+Y) -> Pada yaw=+90°, mata kamera ada di +Y melihat ke -Y
            FaceDef {
                name: "BACK",
                action: ViewCubeAction::Back,
                normal: Vec3::Y,
                corners: [
                    Vec3::new(h, h, -h),
                    Vec3::new(-h, h, -h),
                    Vec3::new(-h, h, h),
                    Vec3::new(h, h, h),
                ],
            },
            // Right (+X) -> Pada yaw=0°, mata kamera ada di +X melihat ke origin
            FaceDef {
                name: "RIGHT",
                action: ViewCubeAction::Right,
                normal: Vec3::X,
                corners: [
                    Vec3::new(h, -h, -h),
                    Vec3::new(h, h, -h),
                    Vec3::new(h, h, h),
                    Vec3::new(h, -h, h),
                ],
            },
            // Left (-X) -> Pada yaw=180°, mata kamera ada di -X melihat ke origin
            FaceDef {
                name: "LEFT",
                action: ViewCubeAction::Left,
                normal: -Vec3::X,
                corners: [
                    Vec3::new(-h, h, -h),
                    Vec3::new(-h, -h, -h),
                    Vec3::new(-h, -h, h),
                    Vec3::new(-h, h, h),
                ],
            },
        ];

        // Struktur data proyeksi 2D untuk permukaan yang menghadap kamera
        struct ProjectedFace<'a> {
            def: &'a FaceDef,
            pts: [Pos2; 4],
            center_2d: Pos2,
            z_depth: f32,
        }

        // Strict Back-Face Culling: Hanya permukaan yang mengarah ke kamera yang diproses & digambar
        let mut projected = Vec::with_capacity(3);
        for face in &faces {
            let facing = face.normal.dot(eye_dir);
            if facing > 0.001 {
                let mut pts = [center; 4];
                let mut sum_z = 0.0;
                let mut sum_2d = Vec2::ZERO;

                for (i, corner) in face.corners.iter().enumerate() {
                    let (p2d, z) = project(*corner);
                    pts[i] = p2d;
                    sum_z += z;
                    sum_2d += Vec2::new(p2d.x, p2d.y);
                }

                let avg_z = sum_z / 4.0;
                let center_2d = Pos2::new(sum_2d.x / 4.0, sum_2d.y / 4.0);

                projected.push(ProjectedFace {
                    def: face,
                    pts,
                    center_2d,
                    z_depth: avg_z,
                });
            }
        }

        // Urutkan dari belakang ke depan (painter's algorithm untuk permukaan depan yang terlihat)
        projected.sort_by(|a, b| {
            a.z_depth
                .partial_cmp(&b.z_depth)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let hover_pos = ui.input(|i| i.pointer.hover_pos());
        let mut clicked_action = None;
        let mut hovered_face_idx = None;

        // Cari face terdepan yang terkena kursor
        if let Some(mouse) = hover_pos {
            if rect.contains(mouse) {
                for (idx, pf) in projected.iter().enumerate().rev() {
                    if point_in_quad(mouse, pf.pts) {
                        hovered_face_idx = Some(idx);
                        break;
                    }
                }
            }
        }

        if response.clicked() {
            if let Some(idx) = hovered_face_idx {
                clicked_action = Some(projected[idx].def.action);
            }
        }

        // Directional lighting untuk nuansa solid 3D (arah cahaya dari kanan atas depan)
        let light_dir = Vec3::new(0.4, -0.5, 0.75).normalize();

        // Gambar permukaan kubus (Solid Opaque, tanpa garis belakang bocor)
        for (idx, pf) in projected.iter().enumerate() {
            let is_hovered = hovered_face_idx == Some(idx);

            let (fill_color, stroke_color, text_color) = if is_hovered {
                (
                    Color32::from_rgb(26, 115, 232), // Modern CAD Active Blue
                    Stroke::new(1.5, Color32::from_rgb(130, 195, 255)),
                    Color32::WHITE,
                )
            } else {
                let diffuse = pf.def.normal.dot(light_dir).max(0.0);
                let base_val = (46.0 + diffuse * 36.0).clamp(42.0, 96.0) as u8;
                (
                    Color32::from_rgb(base_val, base_val + 2, base_val + 7),
                    Stroke::new(1.0, Color32::from_rgb(82, 88, 102)),
                    Color32::from_rgb(225, 230, 240),
                )
            };

            painter.add(egui::Shape::convex_polygon(
                pf.pts.to_vec(),
                fill_color,
                stroke_color,
            ));

            // Label teks permukaan
            painter.text(
                pf.center_2d,
                egui::Align2::CENTER_CENTER,
                pf.def.name,
                FontId::proportional(10.0),
                text_color,
            );
        }

        // Gambar sumbu RGB kecil di bawah ViewCube
        let axes_center = center + Vec2::new(0.0, self.size * 0.72);
        let axis_len = 16.0;
        let x_2d = axes_center + Vec2::new(Vec3::X.dot(cam_right), -Vec3::X.dot(cam_up)) * axis_len;
        let y_2d = axes_center + Vec2::new(Vec3::Y.dot(cam_right), -Vec3::Y.dot(cam_up)) * axis_len;
        let z_2d = axes_center + Vec2::new(Vec3::Z.dot(cam_right), -Vec3::Z.dot(cam_up)) * axis_len;

        painter.line_segment([axes_center, x_2d], Stroke::new(1.5, Color32::from_rgb(255, 69, 58)));
        painter.line_segment([axes_center, y_2d], Stroke::new(1.5, Color32::from_rgb(48, 209, 88)));
        painter.line_segment([axes_center, z_2d], Stroke::new(1.5, Color32::from_rgb(10, 132, 255)));

        painter.text(x_2d, egui::Align2::CENTER_CENTER, "x", FontId::proportional(9.0), Color32::from_rgb(255, 69, 58));
        painter.text(y_2d, egui::Align2::CENTER_CENTER, "y", FontId::proportional(9.0), Color32::from_rgb(48, 209, 88));
        painter.text(z_2d, egui::Align2::CENTER_CENTER, "z", FontId::proportional(9.0), Color32::from_rgb(10, 132, 255));

        clicked_action
    }
}

/// Point in polygon (quad) test
pub fn point_in_quad(p: Pos2, pts: [Pos2; 4]) -> bool {
    let mut inside = false;
    let mut j = 3;
    for i in 0..4 {
        let (pi, pj) = (pts[i], pts[j]);
        if ((pi.y > p.y) != (pj.y > p.y)) && (p.x < (pj.x - pi.x) * (p.y - pi.y) / (pj.y - pi.y) + pi.x) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_in_quad() {
        let quad = [
            Pos2::new(0.0, 0.0),
            Pos2::new(10.0, 0.0),
            Pos2::new(10.0, 10.0),
            Pos2::new(0.0, 10.0),
        ];
        assert!(point_in_quad(Pos2::new(5.0, 5.0), quad));
        assert!(!point_in_quad(Pos2::new(15.0, 5.0), quad));
        assert!(!point_in_quad(Pos2::new(-1.0, 5.0), quad));
    }

    #[test]
    fn test_camera_orthonormal_basis() {
        let yaw = -45f32.to_radians();
        let pitch = 35.264f32.to_radians();

        let (sin_y, cos_y) = yaw.sin_cos();
        let (sin_p, cos_p) = pitch.sin_cos();

        let eye_dir = Vec3::new(cos_p * cos_y, cos_p * sin_y, sin_p).normalize();
        let cam_right = Vec3::new(-sin_y, cos_y, 0.0);
        let cam_up = Vec3::new(-sin_p * cos_y, -sin_p * sin_y, cos_p);

        assert!((eye_dir.length() - 1.0).abs() < 1e-4);
        assert!((cam_right.length() - 1.0).abs() < 1e-4);
        assert!((cam_up.length() - 1.0).abs() < 1e-4);

        assert!(cam_right.dot(eye_dir).abs() < 1e-4);
        assert!(cam_up.dot(eye_dir).abs() < 1e-4);
        assert!(cam_right.dot(cam_up).abs() < 1e-4);
    }

    #[test]
    fn test_preset_face_visibility() {
        // Top view: yaw = -90°, pitch = 89°
        let (sin_y, cos_y) = (-90f32.to_radians()).sin_cos();
        let (sin_p, cos_p) = (89f32.to_radians()).sin_cos();
        let eye_dir = Vec3::new(cos_p * cos_y, cos_p * sin_y, sin_p);
        assert!(Vec3::Z.dot(eye_dir) > 0.0); // TOP is visible
        assert!((-Vec3::Z).dot(eye_dir) < 0.0); // BTM is culled

        // Front view: yaw = -90°, pitch = 0°
        let (sin_y, cos_y) = (-90f32.to_radians()).sin_cos();
        let (sin_p, cos_p) = (0f32.to_radians()).sin_cos();
        let eye_dir = Vec3::new(cos_p * cos_y, cos_p * sin_y, sin_p);
        assert!((-Vec3::Y).dot(eye_dir) > 0.99); // FRONT is visible
        assert!(Vec3::Y.dot(eye_dir) < -0.99); // BACK is culled

        // Right view: yaw = 0°, pitch = 0°
        let (sin_y, cos_y) = (0f32.to_radians()).sin_cos();
        let (sin_p, cos_p) = (0f32.to_radians()).sin_cos();
        let eye_dir = Vec3::new(cos_p * cos_y, cos_p * sin_y, sin_p);
        assert!(Vec3::X.dot(eye_dir) > 0.99); // RIGHT is visible
        assert!((-Vec3::X).dot(eye_dir) < -0.99); // LEFT is culled

        // Isometric view: yaw = -45°, pitch = 35.264°
        let (sin_y, cos_y) = (-45f32.to_radians()).sin_cos();
        let (sin_p, cos_p) = (35.264f32.to_radians()).sin_cos();
        let eye_dir = Vec3::new(cos_p * cos_y, cos_p * sin_y, sin_p);
        assert!(Vec3::Z.dot(eye_dir) > 0.0); // TOP visible
        assert!((-Vec3::Y).dot(eye_dir) > 0.0); // FRONT visible
        assert!(Vec3::X.dot(eye_dir) > 0.0); // RIGHT visible
    }
}
