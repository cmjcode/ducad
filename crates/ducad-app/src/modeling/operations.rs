use ducad_core::BodyId;
use ducad_kernel::PickRay;
use ducad_render::SketchPlane;
use ducad_sketch::constraint::{self, AddConstraint, Constraint};
use glam::Vec3;

use crate::app::DuCADApp;
use crate::model::{
    AddSolidCommand, BodyGeometry, BooleanCommand, BooleanKind, DeleteBodyCommand,
    ReplaceGeometryCommand,
};
use crate::types::{PickMode, ToolKind};

impl DuCADApp {
    /// Terapkan constraint pada entitas terpilih di sketch aktif.
    pub fn apply_constraint(&mut self, new_constraint: Constraint) {
        let mut trial = self.sketch().clone();
        trial.constraints.push(new_constraint.clone());
        let snapshot = trial.constraints.clone();
        let result = constraint::solve(&mut trial, &snapshot);

        if result.converged {
            self.execute_sketch_command(Box::new(AddConstraint::new(new_constraint)));
            self.constraint_status = None;
        } else {
            self.constraint_status = Some(format!(
                "Constraint gagal diselesaikan (sisa residual {:.4}) — dibatalkan, sketch tidak berubah",
                result.final_residual_norm
            ));
        }
    }

    /// Extrude profil dari seleksi entitas sketch saat ini.
    pub fn extrude_selected(&mut self) {
        let distance: f64 = match self.extrude_distance_input.trim().parse() {
            Ok(v) => v,
            Err(_) => {
                self.model_status = Some("Jarak extrude tidak valid".to_string());
                return;
            }
        };
        let profile = match crate::model::build_profile_from_selection(self.sketch(), &self.selected)
        {
            Ok(p) => p,
            Err(msg) => {
                self.model_status = Some(msg);
                return;
            }
        };
        match self.extrude_profile_active_plane(&profile, distance) {
            Ok(shape) => {
                let geo = BodyGeometry::from_shape(shape);
                self.execute_model_command(
                    Box::new(AddSolidCommand::new("Extrude", geo)),
                    &format!("Membuat solid baru setinggi {:.1} mm", distance),
                );
                self.model_status = None;
            }
            Err(e) => self.model_status = Some(format!("Extrude gagal: {e}")),
        }
    }

    /// Revolve profil dari seleksi sketch dengan sumbu dan sudut tertentu.
    pub fn revolve_selected(
        &mut self,
        axis_origin: (f64, f64),
        axis_dir: (f64, f64),
        angle_deg: Option<f64>,
    ) -> bool {
        let profile = match crate::model::build_profile_from_selection(self.sketch(), &self.selected) {
            Ok(p) => p,
            Err(msg) => {
                self.alert_modal.show_error(
                    "Revolve Gagal: Profil Tidak Valid",
                    format!("{msg}"),
                    vec![
                        "Pastikan sketsa membentuk garis atau kurva tertutup sempurna (misal: kotak atau lingkaran).",
                        "Gunakan Tool Pilih (S) lalu drag untuk menyeleksi seluruh entitas yang membentuk profil tertutup.",
                        "Gunakan constraint Coincident pada titik ujung garis yang belum menyatu.",
                    ],
                );
                self.model_status = Some(msg);
                return false;
            }
        };

        match ducad_kernel::revolve_profile(&profile, axis_origin, axis_dir, angle_deg) {
            Ok(shape) => {
                let geo = BodyGeometry::from_shape(shape);
                self.execute_model_command(
                    Box::new(AddSolidCommand::new("Revolve", geo)),
                    &format!("Sudut {:.0}°", angle_deg.unwrap_or(360.0)),
                );
                self.model_status = Some(format!(
                    "Revolve {:.0}° berhasil dibuat",
                    angle_deg.unwrap_or(360.0)
                ));
                true
            }
            Err(e) => {
                self.alert_modal.show_error(
                    "Revolve Gagal: Kesalahan Geometri / Sumbu",
                    format!("{e}"),
                    vec![
                        "Pastikan garis sumbu poros putar TIDAK MEMOTONG bagian dalam profil.",
                        "Letakkan garis sumbu di luar profil atau tepat berhimpit pada salah satu tepi profil.",
                        "Coba gunakan preset 'Sumbu Y' atau 'Tepi Kiri' pada jendela opsi Revolve.",
                    ],
                );
                self.model_status = Some(format!("Revolve gagal: {e}"));
                false
            }
        }
    }

    /// Revolve profil dari seleksi sketch dengan preset sumbu.
    pub fn revolve_selected_with_preset(
        &mut self,
        preset: ducad_ui::RevolveAxisPreset,
        angle_deg: f64,
    ) -> bool {
        let angle_opt = if (angle_deg - 360.0).abs() < 1e-4 {
            None
        } else {
            Some(angle_deg)
        };

        let bbox = crate::model::compute_profile_bbox(self.sketch(), &self.selected);

        let (axis_origin, axis_dir) = match preset {
            ducad_ui::RevolveAxisPreset::YAxisOrigin => ((0.0, 0.0), (0.0, 1.0)),
            ducad_ui::RevolveAxisPreset::XAxisOrigin => ((0.0, 0.0), (1.0, 0.0)),
            ducad_ui::RevolveAxisPreset::BBoxLeft => {
                if let Some([min_x, min_y, _, _]) = bbox {
                    ((min_x, min_y), (0.0, 1.0))
                } else {
                    ((0.0, 0.0), (0.0, 1.0))
                }
            }
            ducad_ui::RevolveAxisPreset::BBoxRight => {
                if let Some([_, min_y, max_x, _]) = bbox {
                    ((max_x, min_y), (0.0, 1.0))
                } else {
                    ((0.0, 0.0), (0.0, 1.0))
                }
            }
            ducad_ui::RevolveAxisPreset::BBoxBottom => {
                if let Some([min_x, min_y, _, _]) = bbox {
                    ((min_x, min_y), (1.0, 0.0))
                } else {
                    ((0.0, 0.0), (1.0, 0.0))
                }
            }
            ducad_ui::RevolveAxisPreset::BBoxTop => {
                if let Some([min_x, _, _, max_y]) = bbox {
                    ((min_x, max_y), (1.0, 0.0))
                } else {
                    ((0.0, 0.0), (1.0, 0.0))
                }
            }
            ducad_ui::RevolveAxisPreset::CustomTwoPoints => {
                return false;
            }
        };

        let effective_axis_dir = if self.revolve_reverse {
            (-axis_dir.0, -axis_dir.1)
        } else {
            axis_dir
        };

        self.revolve_selected(axis_origin, effective_axis_dir, angle_opt)
    }

    /// Loft antara 2 profil tertutup dari seleksi sketch.
    pub fn loft_selected_regions(&mut self, regions: &[ducad_sketch::region::ClosedRegion]) {
        if regions.len() != 2 {
            self.model_status =
                Some("Pilih persis 2 profil tertutup di kanvas sketsa 2D".to_string());
            return;
        }
        let height: f64 = match self.loft_height_input.trim().parse() {
            Ok(v) => v,
            _ => {
                self.model_status = Some("Tinggi loft tidak valid".to_string());
                return;
            }
        };
        let bottom = match crate::model::build_profile_from_selection(self.sketch(), &regions[0].entity_ids) {
            Ok(p) => p,
            Err(msg) => {
                self.model_status = Some(format!("Profil bawah: {msg}"));
                return;
            }
        };
        let top = match crate::model::build_profile_from_selection(self.sketch(), &regions[1].entity_ids) {
            Ok(p) => p,
            Err(msg) => {
                self.model_status = Some(format!("Profil atas: {msg}"));
                return;
            }
        };

        match ducad_kernel::loft_profiles(&bottom, &top, height) {
            Ok(shape) => {
                let geo = BodyGeometry::from_shape(shape);
                self.execute_model_command(
                    Box::new(AddSolidCommand::new("Loft", geo)),
                    &format!("Menghubungkan 2 profil tertutup (Tinggi {:.1} mm)", height),
                );
                self.set_tool(crate::types::ToolKind::Select);
                self.selected.clear();
                self.selection_box = None;
                self.loft_alignment_dismissed = false;
                self.model_status = Some("✓ Loft 3D berhasil dibuat!".to_string());
            }
            Err(e) => self.model_status = Some(format!("Loft gagal: {e}")),
        }
    }

    /// Loft antara `pending_loft_bottom` dan profil dari seleksi sketch saat ini (fallback).
    pub fn loft_selected(&mut self) {
        let all_regions = ducad_sketch::region::find_closed_regions(self.sketch());
        let selected_regions: Vec<ducad_sketch::region::ClosedRegion> = all_regions
            .into_iter()
            .filter(|r| {
                !r.entity_ids.is_empty()
                    && r.entity_ids.iter().all(|id| self.selected.contains(id))
            })
            .collect();
        if selected_regions.len() == 2 {
            self.loft_selected_regions(&selected_regions);
            return;
        }

        let Some(bottom) = self.pending_loft_bottom.clone() else {
            self.model_status =
                Some("Pilih 2 profil tertutup di kanvas sketsa 2D".to_string());
            return;
        };
        let height: f64 = match self.loft_height_input.trim().parse() {
            Ok(v) => v,
            _ => {
                self.model_status = Some("Tinggi loft tidak valid".to_string());
                return;
            }
        };
        let top = match crate::model::build_profile_from_selection(self.sketch(), &self.selected) {
            Ok(p) => p,
            Err(msg) => {
                self.model_status = Some(format!("Profil atas: {msg}"));
                return;
            }
        };
        match ducad_kernel::loft_profiles(&bottom, &top, height) {
            Ok(shape) => {
                let geo = BodyGeometry::from_shape(shape);
                self.execute_model_command(
                    Box::new(AddSolidCommand::new("Loft", geo)),
                    &format!("Menghubungkan 2 profil tertutup (Tinggi {:.1} mm)", height),
                );
                self.pending_loft_bottom = None;
                self.model_status = None;
            }
            Err(e) => self.model_status = Some(format!("Loft gagal: {e}")),
        }
    }

    /// Sweep profil di sepanjang jalur kurva yang telah ditentukan.
    pub fn sweep_selected(&mut self) {
        let Some((profile, profile_plane)) = self.pending_sweep_profile.clone().or_else(|| {
            crate::model::build_profile_from_selection(self.sketch(), &self.selected)
                .ok()
                .map(|p| (p, self.active_plane))
        }) else {
            self.model_status = Some("Pilih profil tertutup untuk operasi sweep".to_string());
            return;
        };

        let Some(path_segments) = self.pending_sweep_path.clone().or_else(|| {
            let path_plane_idx = self.sweep_path_plane_idx.unwrap_or_else(|| self.active_plane_index());
            let path_plane = Self::plane_for_index(path_plane_idx);
            crate::model::build_path_from_selection_on_plane(
                &self.sketches[path_plane_idx],
                &self.selected,
                &path_plane,
            )
            .ok()
        }) else {
            self.model_status = Some("Pilih garis/busur/spline sebagai kurva jalur sweep".to_string());
            return;
        };

        let origin = [
            profile_plane.origin.x as f64,
            profile_plane.origin.y as f64,
            profile_plane.origin.z as f64,
        ];
        let u_axis = [
            profile_plane.u_axis.x as f64,
            profile_plane.u_axis.y as f64,
            profile_plane.u_axis.z as f64,
        ];
        let v_axis = [
            profile_plane.v_axis.x as f64,
            profile_plane.v_axis.y as f64,
            profile_plane.v_axis.z as f64,
        ];
        let normal = [
            profile_plane.normal.x as f64,
            profile_plane.normal.y as f64,
            profile_plane.normal.z as f64,
        ];

        match ducad_kernel::sweep_profile_on_plane_along_path(
            &profile,
            origin,
            u_axis,
            v_axis,
            normal,
            &path_segments,
        ) {
            Ok(shape) => {
                let geo = BodyGeometry::from_shape(shape);
                self.execute_model_command(
                    Box::new(AddSolidCommand::new("Sweep", geo)),
                    "Menyapu profil 2D di sepanjang kurva jalur 3D",
                );
                self.pending_sweep_profile = None;
                self.pending_sweep_path = None;
                self.sweep_path_plane_idx = None;
                self.hovered_plane_idx = None;
                self.selected.clear();
                self.model_status = Some("✓ Solid Sweep 3D berhasil dibuat".to_string());
                self.set_tool(ToolKind::Select);
            }
            Err(e) => self.model_status = Some(format!("Sweep gagal: {e}")),
        }
    }



    /// Union/Subtract/Intersect dua body terpilih.
    pub fn boolean_selected(&mut self, kind: BooleanKind, label: &'static str, result_name: &str) {
        let ids: Vec<BodyId> = self.selected_bodies.iter().copied().collect();
        let [a, b] = ids.as_slice() else {
            self.model_status =
                Some("Pilih persis 2 body di daftar untuk operasi ini".to_string());
            return;
        };
        let (a_id, b_id) = (*a, *b);
        let detail_desc = match kind {
            BooleanKind::Union => "Menggabungkan 2 solid 3D menjadi satu",
            BooleanKind::Subtract => "Memotong solid 3D utama dengan solid pemotong",
            BooleanKind::Intersect => "Mengambil irisan perpotongan 2 solid 3D",
        };
        match BooleanCommand::try_new(&self.model, kind, label, result_name, a_id, b_id) {
            Ok(cmd) => {
                self.execute_model_command(Box::new(cmd), detail_desc);
                self.selected_bodies.clear();
                self.round_history.remove(&a_id);
                self.round_history.remove(&b_id);
                self.model_status = None;
            }
            Err(e) => self.model_status = Some(format!("Boolean gagal: {e}")),
        }
    }

    /// Eksekusi operasi boolean aktif (Union, Subtract, Intersect) dari Top HUD.
    pub fn apply_current_boolean_op(&mut self) {
        match self.boolean_op {
            ducad_ui::BooleanOpKind::Union => {
                self.boolean_selected(BooleanKind::Union, "Union", "Union");
            }
            ducad_ui::BooleanOpKind::Subtract => {
                self.boolean_selected(BooleanKind::Subtract, "Subtract", "Subtract");
            }
            ducad_ui::BooleanOpKind::Intersect => {
                self.boolean_selected(BooleanKind::Intersect, "Intersect", "Intersect");
            }
        }
        self.set_tool(crate::types::ToolKind::Select);
    }

    /// Fillet SEMUA tepi atau tepi terpilih pada 1 body.
    pub fn fillet_selected_body(&mut self) {
        let Some(&id) = self
            .selected_bodies
            .iter()
            .next()
            .filter(|_| self.selected_bodies.len() == 1)
        else {
            self.model_status = Some("Pilih persis 1 body untuk Fillet".to_string());
            return;
        };
        let Ok(radius) = self.fillet_radius_input.trim().parse::<f64>() else {
            self.model_status = Some("Radius fillet tidak valid".to_string());
            return;
        };
        let Some(geo) = self.model.geometry.get(id) else {
            return;
        };
        let rays: Vec<PickRay> = self.selected_edges.iter().map(|e| e.ray).collect();
        let result = if rays.is_empty() {
            ducad_kernel::fillet_all(&geo.shape, radius)
        } else {
            ducad_kernel::fillet_edges(
                &geo.shape,
                radius,
                &rays,
                Self::EDGE_REAPPLY_TOLERANCE_MM,
            )
        };
        match result {
            Ok(shape) => {
                let new_geo = BodyGeometry::from_shape(shape);
                self.execute_model_command(
                    Box::new(ReplaceGeometryCommand::new("Fillet", id, new_geo)),
                    &format!("Melengkungkan sudut rusuk body (Radius {:.1} mm)", radius),
                );
                self.round_history.remove(&id);
                self.selected_edges.clear();
                self.picking_mode = PickMode::None;
                self.model_status = None;
            }
            Err(e) => self.model_status = Some(format!("Fillet gagal: {e}")),
        }
    }

    /// Chamfer SEMUA tepi atau tepi terpilih pada 1 body.
    pub fn chamfer_selected_body(&mut self) {
        let Some(&id) = self
            .selected_bodies
            .iter()
            .next()
            .filter(|_| self.selected_bodies.len() == 1)
        else {
            self.model_status = Some("Pilih persis 1 body untuk Chamfer".to_string());
            return;
        };
        let Ok(distance) = self.chamfer_distance_input.trim().parse::<f64>() else {
            self.model_status = Some("Jarak chamfer tidak valid".to_string());
            return;
        };
        let Some(geo) = self.model.geometry.get(id) else {
            return;
        };
        let rays: Vec<PickRay> = self.selected_edges.iter().map(|e| e.ray).collect();
        let result = if rays.is_empty() {
            ducad_kernel::chamfer_all(&geo.shape, distance)
        } else {
            ducad_kernel::chamfer_edges(
                &geo.shape,
                distance,
                &rays,
                Self::EDGE_REAPPLY_TOLERANCE_MM,
            )
        };
        match result {
            Ok(shape) => {
                let new_geo = BodyGeometry::from_shape(shape);
                self.execute_model_command(
                    Box::new(ReplaceGeometryCommand::new("Chamfer", id, new_geo)),
                    &format!("Meniruskan sudut siku rusuk body ({:.1} mm)", distance),
                );
                self.round_history.remove(&id);
                self.selected_edges.clear();
                self.picking_mode = PickMode::None;
                self.model_status = None;
            }
            Err(e) => self.model_status = Some(format!("Chamfer gagal: {e}")),
        }
    }

    /// Shell/Hollow 1 body terpilih.
    pub fn shell_selected_body(&mut self) {
        let Some(&id) = self
            .selected_bodies
            .iter()
            .next()
            .filter(|_| self.selected_bodies.len() == 1)
        else {
            self.model_status = Some("Pilih persis 1 body untuk Shell/Hollow".to_string());
            return;
        };
        let Ok(thickness) = self.shell_thickness_input.trim().parse::<f64>() else {
            self.model_status = Some("Tebal shell tidak valid".to_string());
            return;
        };
        let Some(geo) = self.model.geometry.get(id) else {
            return;
        };
        let result = if self.selected_faces.is_empty() {
            ducad_kernel::shell_hollow(&geo.shape, thickness, self.shell_direction)
        } else {
            ducad_kernel::shell_hollow_faces(&geo.shape, thickness, &self.selected_faces)
        };
        match result {
            Ok(shape) => {
                let new_geo = BodyGeometry::from_shape(shape);
                self.execute_model_command(
                    Box::new(ReplaceGeometryCommand::new("Shell", id, new_geo)),
                    &format!("Membuat rongga hollow dinding tebal {:.1} mm", thickness),
                );
                self.round_history.remove(&id);
                self.selected_faces.clear();
                self.picking_mode = PickMode::None;
                self.model_status = None;
            }
            Err(e) => self.model_status = Some(format!("Shell gagal: {e}")),
        }
    }

    /// Shell/Hollow sisi/face 3D yang sedang aktif dengan ketebalan dinding yang ditentukan.
    pub fn shell_active_face(&mut self) {
        let Some((target_id, ray, _hit)) = self.active_face else {
            self.model_status =
                Some("Pilih salah satu sisi (face) objek terlebih dahulu untuk Shell/Hollow".to_string());
            return;
        };
        let Ok(thickness) = self.shell_thickness_input.trim().parse::<f64>() else {
            self.model_status = Some("Tebal shell tidak valid".to_string());
            return;
        };
        let Some(geo) = self.model.geometry.get(target_id) else {
            return;
        };

        match ducad_kernel::shell_hollow_faces(&geo.shape, thickness, &[ray]) {
            Ok(shape) => {
                let new_geo = BodyGeometry::from_shape(shape);
                self.execute_model_command(
                    Box::new(ReplaceGeometryCommand::new("Shell Face", target_id, new_geo)),
                    &format!("Membuat rongga berlubang pada sisi (Tebal {:.1} mm)", thickness),
                );
                self.round_history.remove(&target_id);
                self.active_face = None;
                self.model_status = Some(format!("Shell face {:.1} mm sukses", thickness));
            }
            Err(e) => {
                self.model_status = Some(format!("Shell face gagal: {e}"));
            }
        }
    }

    /// Extrude (push-pull) sisi/face 3D yang sedang aktif dengan jarak tertentu.
    pub fn extrude_active_face(&mut self, distance: f64) {
        let Some((target_id, ray, _hit)) = self.active_face else {
            self.model_status =
                Some("Pilih salah satu sisi (face) objek terlebih dahulu".to_string());
            return;
        };
        let Some(target_geo) = self.model.geometry.get(target_id) else {
            self.model_status = Some("Body terpilih tidak ditemukan".to_string());
            return;
        };

        match ducad_kernel::extrude_face(&target_geo.shape, ray, distance) {
            Ok(new_shape) => {
                let new_geo = BodyGeometry::from_shape(new_shape);
                let (label, desc) = if distance >= 0.0 {
                    ("Extrude Face", format!("Menarik sisi solid sejauh {:.1} mm", distance))
                } else {
                    ("Cut Face", format!("Memotong/mencekungkan sisi solid sedalam {:.1} mm", distance.abs()))
                };
                self.execute_model_command(
                    Box::new(ReplaceGeometryCommand::new(label, target_id, new_geo)),
                    &desc,
                );
                self.round_history.remove(&target_id);
                self.active_face = None;
                self.model_status = Some(format!("Extrude face {:.1} mm sukses", distance));
            }
            Err(e) => {
                self.model_status = Some(format!("Extrude face gagal: {e}"));
            }
        }
    }

    /// Revolve sisi/face 3D yang sedang aktif mengelilingi sumbu dalam koordinat bidang aktif atau 3D.
    pub fn revolve_active_face(
        &mut self,
        axis_origin_2d: (f64, f64),
        axis_dir_2d: (f64, f64),
        angle_deg: Option<f64>,
    ) -> bool {
        let Some((target_id, ray, _hit)) = self.active_face else {
            self.model_status =
                Some("Pilih salah satu sisi (face) objek terlebih dahulu".to_string());
            return false;
        };
        let Some(target_geo) = self.model.geometry.get(target_id) else {
            self.model_status = Some("Body terpilih tidak ditemukan".to_string());
            return false;
        };

        let p0_3d = self.active_plane.to_world_f64(axis_origin_2d, 0.0);
        let p1_2d = (axis_origin_2d.0 + axis_dir_2d.0, axis_origin_2d.1 + axis_dir_2d.1);
        let p1_3d = self.active_plane.to_world_f64(p1_2d, 0.0);

        let axis_origin = glam::dvec3(p0_3d[0], p0_3d[1], p0_3d[2]);
        let axis_dir = glam::dvec3(p1_3d[0] - p0_3d[0], p1_3d[1] - p0_3d[1], p1_3d[2] - p0_3d[2]);

        match ducad_kernel::revolve_face(&target_geo.shape, ray, axis_origin, axis_dir, angle_deg) {
            Ok(new_shape) => {
                let new_geo = BodyGeometry::from_shape(new_shape);
                self.execute_model_command(
                    Box::new(ReplaceGeometryCommand::new("Revolve Face", target_id, new_geo)),
                    &format!("Memutar permukaan sisi solid sebesar {:.0}°", angle_deg.unwrap_or(360.0)),
                );
                self.round_history.remove(&target_id);
                self.active_face = None;
                self.model_status = Some(format!(
                    "Revolve Face {:.0}° sukses",
                    angle_deg.unwrap_or(360.0)
                ));
                true
            }
            Err(e) => {
                self.alert_modal.show_error(
                    "Revolve Face Gagal",
                    format!("{e}"),
                    vec![
                        "Pastikan garis sumbu poros putar TIDAK MEMOTONG bagian dalam sisi (face).",
                        "Letakkan garis sumbu di luar face atau tepat berhimpit pada salah satu tepi rusuk.",
                    ],
                );
                self.model_status = Some(format!("Revolve face gagal: {e}"));
                false
            }
        }
    }

    /// Jadikan permukaan sisi 3D yang aktif sebagai bidang sketsa baru.
    pub fn sketch_on_active_face(&mut self) {
        let Some((_target_id, _ray, hit)) = self.active_face else {
            self.model_status =
                Some("Pilih salah satu sisi (face) objek terlebih dahulu".to_string());
            return;
        };
        let origin = Vec3::new(
            hit.centroid.0 as f32,
            hit.centroid.1 as f32,
            hit.centroid.2 as f32,
        );
        let normal = Vec3::new(
            hit.normal.0 as f32,
            hit.normal.1 as f32,
            hit.normal.2 as f32,
        );
        self.active_plane = SketchPlane::from_origin_normal(origin, normal);
        self.is_sketching = true;
        self.left_toolbar.is_sketching = true;
        self.camera.orient_to_plane(&self.active_plane);
        self.active_face = None;
        self.model_status = Some("Sketsa aktif pada permukaan sisi objek".to_string());
    }

    /// Hapus semua body terpilih.
    pub fn delete_selected_bodies(&mut self) {
        for id in std::mem::take(&mut self.selected_bodies) {
            let body_name = self.model.doc.bodies.get(id).map(|b| b.name.clone()).unwrap_or_else(|| "Solid".to_string());
            self.execute_model_command(
                Box::new(DeleteBodyCommand::new(id)),
                &format!("Menghapus objek solid '{}' dari dokumen", body_name),
            );
            self.round_history.remove(&id);
        }
        self.body_move_armed = false;
        self.body_move_target = None;
    }
}
