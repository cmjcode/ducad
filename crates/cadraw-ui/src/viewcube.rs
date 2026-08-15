//! Interactive 3D ViewCube & Orientation Gizmo bergaya Shapr3D.
//!
//! Merender kubus 3D navigasi di pojok kanan atas kanvas yang berputar
//! sinkron dengan orientasi kamera (yaw & pitch). Mengklik salah satu
//! permukaan (TOP, FRONT, RIGHT, BACK, LEFT, BOTTOM) atau sudut isometrik
//! mengembalikan preset orientasi kamera terkait.

use egui::{Color32, FontId, Pos2, Rect, Stroke, Vec2};
use glam::{Mat3, Vec3};

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
        let rect = Rect::from_center_size(center, Vec2::splat(self.size * 1.5));
        let response = ui.allocate_rect(rect, egui::Sense::click());
        let painter = ui.painter_at(rect);

        // Matriks rotasi dari kamera turntable (Z-up)
        // OrbitCamera: eye = target + dist * (cos_pitch*cos_yaw, cos_pitch*sin_yaw, sin_pitch)
        let (sin_y, cos_y) = camera_yaw.sin_cos();
        let (sin_p, cos_p) = camera_pitch.sin_cos();
        
        let forward = Vec3::new(cos_p * cos_y, cos_p * sin_y, sin_p).normalize();
        let right = forward.cross(Vec3::Z).normalize_or_zero();
        let up = right.cross(forward).normalize_or_zero();
        
        // Transformasi dunia ke koordinat kamera ViewCube (X kanan, Y atas, Z kedalaman pandangan)
        let rot = Mat3::from_cols(right, up, forward);

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
            // Front (+Y)
            FaceDef {
                name: "FRONT",
                action: ViewCubeAction::Front,
                normal: Vec3::Y,
                corners: [
                    Vec3::new(-h, h, -h),
                    Vec3::new(h, h, -h),
                    Vec3::new(h, h, h),
                    Vec3::new(-h, h, h),
                ],
            },
            // Back (-Y)
            FaceDef {
                name: "BACK",
                action: ViewCubeAction::Back,
                normal: -Vec3::Y,
                corners: [
                    Vec3::new(h, -h, -h),
                    Vec3::new(-h, -h, -h),
                    Vec3::new(-h, -h, h),
                    Vec3::new(h, -h, h),
                ],
            },
            // Right (+X)
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
            // Left (-X)
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

        // Hitung posisi proyeksi 2D dan kedalaman Z tiap face
        struct ProjectedFace<'a> {
            def: &'a FaceDef,
            pts: [Pos2; 4],
            center_2d: Pos2,
            z_depth: f32,
            is_front: bool,
        }

        let mut projected = Vec::with_capacity(6);
        for face in &faces {
            let view_normal = rot * face.normal;
            let is_front = view_normal.z > 0.05; // Mengarah ke penonton

            let mut pts = [center; 4];
            let mut sum_z = 0.0;
            let mut sum_2d = Vec2::ZERO;

            for (i, corner) in face.corners.iter().enumerate() {
                let v = rot * (*corner);
                // Proyeksi ortografis skala kubus
                let p2d = center + Vec2::new(v.x, -v.y);
                pts[i] = p2d;
                sum_z += v.z;
                sum_2d += Vec2::new(p2d.x, p2d.y);
            }

            let avg_z = sum_z / 4.0;
            let center_2d = Pos2::new(sum_2d.x / 4.0, sum_2d.y / 4.0);

            projected.push(ProjectedFace {
                def: face,
                pts,
                center_2d,
                z_depth: avg_z,
                is_front,
            });
        }

        // Urutkan dari belakang ke depan (painter's algorithm)
        projected.sort_by(|a, b| a.z_depth.partial_cmp(&b.z_depth).unwrap());

        let hover_pos = ui.input(|i| i.pointer.hover_pos());
        let mut clicked_action = None;
        let mut hovered_face_idx = None;

        // Cari face terdepan yang terkena kursor
        if let Some(mouse) = hover_pos {
            if rect.contains(mouse) {
                for (idx, pf) in projected.iter().enumerate().rev() {
                    if pf.is_front && point_in_quad(mouse, pf.pts) {
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

        // Gambar kubus
        for (idx, pf) in projected.iter().enumerate() {
            if !pf.is_front {
                // Sisi belakang digambar redup / outline saja
                painter.add(egui::Shape::convex_polygon(
                    pf.pts.to_vec(),
                    Color32::from_rgba_premultiplied(25, 27, 32, 100),
                    Stroke::new(0.5, Color32::from_rgba_premultiplied(45, 48, 56, 120)),
                ));
            } else {
                // Sisi depan
                let is_hovered = hovered_face_idx == Some(idx);
                let (fill_color, stroke_color, text_color) = if is_hovered {
                    (
                        Color32::from_rgba_premultiplied(10, 132, 255, 220),
                        Stroke::new(1.5, Color32::from_rgb(100, 180, 255)),
                        Color32::WHITE,
                    )
                } else {
                    // Shading ringan berdasarkan orientasi normal
                    let shade = (pf.def.normal.dot(Vec3::new(0.4, 0.6, 0.8)).clamp(0.0, 1.0) * 40.0) as u8;
                    let base_c = 45 + shade;
                    (
                        Color32::from_rgba_premultiplied(base_c, base_c + 2, base_c + 8, 230),
                        Stroke::new(1.0, Color32::from_rgba_premultiplied(90, 95, 110, 220)),
                        Color32::from_rgb(220, 225, 235),
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
        }

        // Gambar sumbu RGB kecil di samping bawah ViewCube
        let axes_center = center + Vec2::new(0.0, self.size * 0.72);
        let axis_len = 16.0;
        let x_2d = axes_center + Vec2::new((rot * Vec3::X).x, -(rot * Vec3::X).y) * axis_len;
        let y_2d = axes_center + Vec2::new((rot * Vec3::Y).x, -(rot * Vec3::Y).y) * axis_len;
        let z_2d = axes_center + Vec2::new((rot * Vec3::Z).x, -(rot * Vec3::Z).y) * axis_len;

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
fn point_in_quad(p: Pos2, pts: [Pos2; 4]) -> bool {
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
