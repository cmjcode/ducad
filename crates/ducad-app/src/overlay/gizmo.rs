use eframe::egui;
use glam::{DVec2, Vec3};

use crate::app::DuCADApp;
use crate::model::{
    AddSolidCommand, BodyGeometry, ReplaceGeometryCommand,
};
use crate::viewport::{pixel_tolerance_to_world, world_to_screen_pos};

impl DuCADApp {
    /// Hitung delta pergeseran dari pergeseran mouse layar diproyeksikan ke sumbu normal 3D.
    pub fn project_screen_drag_to_world_axis(
        &self,
        rect: egui::Rect,
        origin_3d: Vec3,
        normal_3d: Vec3,
        drag_delta: egui::Vec2,
    ) -> (f64, Option<egui::Vec2>) {
        let normal = normal_3d.normalize_or_zero();
        let p_base = origin_3d;
        let p_ref = p_base + normal * 10.0;
        let s_base = world_to_screen_pos(&self.camera, rect, p_base);
        let s_ref = world_to_screen_pos(&self.camera, rect, p_ref);

        if let (Some(sb), Some(sr)) = (s_base, s_ref) {
            let arrow_vec = sr - sb;
            let len_sq = arrow_vec.length_sq();
            if len_sq > 1e-4 {
                let dot = drag_delta.x * arrow_vec.x + drag_delta.y * arrow_vec.y;
                let delta_mm = (dot / len_sq) * 10.0;
                return (delta_mm as f64, Some(arrow_vec));
            }
        }

        let world_scale = pixel_tolerance_to_world(&self.camera, rect);
        ((-drag_delta.y as f64) * world_scale * 1.6, None)
    }

    /// Hitung delta pergeseran diproyeksikan ke sumbu normal bidang sketsa aktif.
    pub fn project_screen_drag_to_extrude_axis(
        &self,
        rect: egui::Rect,
        centroid: DVec2,
        drag_delta: egui::Vec2,
    ) -> (f64, Option<egui::Vec2>) {
        let p_base = self.active_plane.to_world(centroid, 0.0);
        self.project_screen_drag_to_world_axis(rect, p_base, self.active_plane.normal, drag_delta)
    }

    /// Deteksi live apakah extrude saat ini memotong solid yang ada (Smart Boolean Cut).
    pub fn update_gizmo_boolean_detection(&mut self) {
        if let Ok(profile) =
            crate::model::build_profile_from_selection(self.sketch(), &self.selected)
        {
            if let Ok(swept) =
                self.extrude_profile_active_plane(&profile, self.gizmo_distance)
            {
                let mut is_cutting = false;
                for (b_id, b_geo) in self.model.geometry.iter() {
                    if let Some(body) = self.model.doc.bodies.get(b_id) {
                        if body.visible {
                            if let Ok(intersect_shape) =
                                ducad_kernel::intersect(&b_geo.shape, &swept)
                            {
                                let tri_count = intersect_shape.tessellate().triangle_count();
                                if tri_count > 0 {
                                    if let Ok(_cut_res) =
                                        ducad_kernel::subtract(&b_geo.shape, &swept)
                                    {
                                        is_cutting = true;
                                        self.gizmo_is_cutting = true;
                                        self.gizmo_target_body = Some(b_id);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                if !is_cutting {
                    self.gizmo_is_cutting = false;
                    self.gizmo_target_body = None;
                }
            }
        }
    }

    /// Eksekusi commit extrude/cut saat drag gizmo selesai atau nilai presisi di-enter.
    pub fn commit_gizmo_extrusion(&mut self) {
        if self.gizmo_distance.abs() > 0.1 {
            if let Ok(profile) =
                crate::model::build_profile_from_selection(self.sketch(), &self.selected)
            {
                if let Ok(swept) =
                    self.extrude_profile_active_plane(&profile, self.gizmo_distance)
                {
                    if self.gizmo_is_cutting {
                        if let Some(target_id) = self.gizmo_target_body {
                            if let Some(target_geo) = self.model.geometry.get(target_id) {
                                if let Ok(cut_res) =
                                    ducad_kernel::subtract(&target_geo.shape, &swept)
                                {
                                    let new_geo = BodyGeometry::from_shape(cut_res);
                                    self.execute_model_command(
                                        Box::new(ReplaceGeometryCommand::new(
                                            "Cut Extrude",
                                            target_id,
                                            new_geo,
                                        )),
                                        &format!("Jarak {:.1} mm", self.gizmo_distance),
                                    );
                                    self.round_history.remove(&target_id);
                                }
                            }
                        }
                    } else {
                        let geo = BodyGeometry::from_shape(swept);
                        let cmd = AddSolidCommand::new("Extrude", geo);
                        self.execute_model_command(
                            Box::new(cmd),
                            &format!("Tinggi {:.1} mm", self.gizmo_distance),
                        );
                    }
                    self.selected.clear();
                }
            }
        }
        self.extruding_from_gizmo = false;
        self.gizmo_is_cutting = false;
        self.gizmo_target_body = None;
        self.gizmo_distance = 20.0;
        self.gizmo_edit_input = format!(
            "{:.0}",
            self.unit.to_display_val(self.gizmo_distance)
        );
    }

    /// Cek apakah posisi mouse saat ini berada dekat dengan gizmo panah atau dasar profil.
    pub fn check_near_gizmo(&self, rect: egui::Rect, hover_pos: Option<egui::Pos2>) -> bool {
        let Some(pos) = hover_pos else {
            return false;
        };

        if let Some(c) = self.selected_closed_region_centroid() {
            let z_top = if self.extruding_from_gizmo {
                self.gizmo_distance as f32
            } else {
                16.0
            };
            let top_3d = self.active_plane.to_world(c, z_top);
            let bot_3d = self.active_plane.to_world(c, 0.0);
            let near_top = world_to_screen_pos(&self.camera, rect, top_3d)
                .is_some_and(|s| s.distance(pos) < 36.0);
            let near_bot = world_to_screen_pos(&self.camera, rect, bot_3d)
                .is_some_and(|s| s.distance(pos) < 36.0);
            if near_top || near_bot {
                return true;
            }
        }

        if let Some((_, _, hit)) = &self.active_face {
            let anchor = hit.gizmo_anchor();
            let c_base = Vec3::new(anchor.0 as f32, anchor.1 as f32, anchor.2 as f32);
            let pull_dir = Vec3::new(
                hit.pull_dir.0 as f32,
                hit.pull_dir.1 as f32,
                hit.pull_dir.2 as f32,
            );
            let dist = if self.extruding_face_from_gizmo {
                self.face_gizmo_distance as f32
            } else {
                18.0
            };
            let top_3d = c_base + pull_dir * dist;
            let mid_3d = (c_base + top_3d) * 0.5;
            let near_top = world_to_screen_pos(&self.camera, rect, top_3d)
                .is_some_and(|s| s.distance(pos) < 40.0);
            let near_bot = world_to_screen_pos(&self.camera, rect, c_base)
                .is_some_and(|s| s.distance(pos) < 40.0);
            let near_mid = world_to_screen_pos(&self.camera, rect, mid_3d)
                .is_some_and(|s| s.distance(pos) < 40.0);
            if near_top || near_bot || near_mid {
                return true;
            }
        }

        if !self.feature_pick_active() {
            if let Some((_, center)) = self.selected_single_body_center() {
                if let Some(s_center) = world_to_screen_pos(&self.camera, rect, center) {
                    if s_center.distance(pos) < 95.0 {
                        return true;
                    }
                }
            }
        }

        false
    }
}
