use ducad_core::BodyId;
use ducad_kernel::PickRay;
use ducad_render::SketchPlane;
use ducad_sketch::constraint::{self, AddConstraint, Constraint};
use glam::{DVec2, Vec3};

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
        let Some((target_id, ray, _)) = self.active_face.as_ref().map(|(id, r, _)| (*id, *r, ())) else {
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

    /// Shell dengan ketebalan bervariasi (Variable Thickness Shell).
    pub fn shell_variable_selected_body(&mut self) {
        let target_id = self
            .selected_bodies
            .iter()
            .next()
            .copied()
            .or_else(|| self.active_face.as_ref().map(|(id, _, _)| *id));

        let Some(id) = target_id else {
            self.model_status = Some("Pilih persis 1 body untuk Shell Variable Thickness".to_string());
            return;
        };
        let Ok(thickness) = self.shell_thickness_input.trim().parse::<f64>() else {
            self.model_status = Some("Tebal default shell tidak valid".to_string());
            return;
        };
        let Some(geo) = self.model.geometry.get(id) else {
            return;
        };

        let mut remove_faces = self.selected_faces.clone();
        if remove_faces.is_empty() {
            if let Some((_, r, _)) = &self.active_face {
                remove_faces.push(*r);
            }
        }

        let result = if self.shell_variable_faces.is_empty() {
            if remove_faces.is_empty() {
                ducad_kernel::shell_hollow(&geo.shape, thickness, self.shell_direction)
            } else {
                ducad_kernel::shell_hollow_faces(&geo.shape, thickness, &remove_faces)
            }
        } else {
            ducad_kernel::shell_variable_thickness(
                &geo.shape,
                thickness,
                &remove_faces,
                &self.shell_variable_faces,
            )
        };

        match result {
            Ok(shape) => {
                let new_geo = BodyGeometry::from_shape(shape);
                let orig_name = self
                    .model
                    .doc
                    .bodies
                    .get(id)
                    .map(|b| b.name.clone())
                    .unwrap_or_else(|| "Solid".to_string());
                self.execute_model_command(
                    Box::new(ReplaceGeometryCommand::new("Shell Variable", id, new_geo)),
                    &format!("Membuat shell berongga dinding tebal {:.1} mm pada '{}'", thickness, orig_name),
                );
                self.round_history.remove(&id);
                self.selected_faces.clear();
                self.shell_variable_faces.clear();
                self.active_face = None;
                self.picking_mode = crate::types::PickMode::None;
                self.model_status = Some(format!("Shell dinding bervariasi pada '{}' berhasil ✓", orig_name));
                self.set_tool(crate::types::ToolKind::Select);
            }
            Err(e) => self.model_status = Some(format!("Shell Variable gagal: {e}")),
        }
    }

    /// Terapkan Tulang Penguat (Rib / Stiffener Support) pada body 3D terpilih.
    pub fn apply_rib_to_body(&mut self) {
        let target_id = self
            .selected_bodies
            .iter()
            .next()
            .copied()
            .or_else(|| self.active_face.as_ref().map(|(id, _, _)| *id));

        let Some(id) = target_id else {
            self.model_status = Some("Pilih 1 body 3D untuk menambahkan tulang penguat (Rib)".to_string());
            return;
        };
        let Ok(thickness) = self.rib_thickness_input.trim().parse::<f64>() else {
            self.model_status = Some("Tebal tulang penguat (rib thickness) tidak valid".to_string());
            return;
        };
        let Ok(depth) = self.rib_depth_input.trim().parse::<f64>() else {
            self.model_status = Some("Kedalaman tulang penguat (rib depth) tidak valid".to_string());
            return;
        };
        let draft_deg = self.rib_draft_input.trim().parse::<f64>().ok().filter(|&d| d > 0.0);

        let Some(geo) = self.model.geometry.get(id) else {
            return;
        };

        // Dapatkan titik awal & akhir rib serta vektor normal arah kedalaman
        let (p_start, p_end, normal) = if let (Some(s), Some(e)) = (self.rib_start_pt, self.rib_end_pt) {
            (s, e, self.rib_normal_dir)
        } else if let Some(first_sel) = self.selected.iter().next() {
            if let Some(ducad_sketch::Entity::Line { start, end, .. }) = self.sketch().entities.get(*first_sel) {
                let plane = self.active_plane;
                let s3 = plane.to_world(*start, 0.0);
                let e3 = plane.to_world(*end, 0.0);
                let n3 = -plane.normal;
                (
                    glam::dvec3(s3.x as f64, s3.y as f64, s3.z as f64),
                    glam::dvec3(e3.x as f64, e3.y as f64, e3.z as f64),
                    glam::dvec3(n3.x as f64, n3.y as f64, n3.z as f64),
                )
            } else {
                let (min, max) = geo.mesh.bounding_box().unwrap_or(([-30.0, -30.0, -30.0], [30.0, 30.0, 30.0]));
                let mid_y = (min[1] + max[1]) * 0.5;
                let top_z = max[2];
                (
                    glam::dvec3(min[0] as f64, mid_y as f64, top_z as f64),
                    glam::dvec3(max[0] as f64, mid_y as f64, top_z as f64),
                    glam::dvec3(0.0, 0.0, -1.0),
                )
            }
        } else if let Some((_, _, hit)) = &self.active_face {
            // FACE TERPILIH LANGSUNG PADA SOLID CASING
            let face_normal = glam::dvec3(hit.normal.0, hit.normal.1, hit.normal.2).normalize();
            let centroid = glam::dvec3(hit.centroid.0, hit.centroid.1, hit.centroid.2);
            let inward_normal = -face_normal;

            // Hitung basis sumbu orthogonal yang sejajar pada bidang face:
            // U0: Sumbu Horizontal 0° (sejajar bidang XY atau tegak lurus sumbu vertikal Z)
            // V0: Sumbu Vertikal 90° (tegak lurus terhadap U0 dan face_normal)
            let (dir_u0, dir_v0) = if face_normal.z.abs() > 0.95 {
                // Face horizontal (misal alas / pelat bawah atau atas)
                (glam::DVec3::X, glam::DVec3::Y)
            } else {
                // Face vertikal atau miring (misal dinding samping atau belakang casing)
                let u = glam::DVec3::Z.cross(face_normal).normalize();
                let v = face_normal.cross(u).normalize();
                (u, v)
            };

            let angle_deg = self.rib_angle_input.trim().parse::<f64>().unwrap_or(0.0);
            let angle_rad = angle_deg.to_radians();

            // Sumbu arah rib berputar tepat sesuai sudut yang dimasukkan pengguna
            let chosen_dir = (dir_u0 * angle_rad.cos() + dir_v0 * angle_rad.sin()).normalize();

            if hit.boundary_points.len() >= 2 {
                let pts: Vec<glam::DVec3> = hit
                    .boundary_points
                    .iter()
                    .map(|p| glam::dvec3(p.0, p.1, p.2))
                    .collect();

                // Proyeksikan seluruh titik batas face ke sumbu yang dipilih
                let mut min_proj = f64::INFINITY;
                let mut max_proj = f64::NEG_INFINITY;
                for p in &pts {
                    let proj = (*p - centroid).dot(chosen_dir);
                    if proj < min_proj {
                        min_proj = proj;
                    }
                    if proj > max_proj {
                        max_proj = proj;
                    }
                }

                let p_a = centroid + chosen_dir * min_proj;
                let p_b = centroid + chosen_dir * max_proj;

                (p_a, p_b, inward_normal)
            } else {
                (
                    centroid - chosen_dir * 20.0,
                    centroid + chosen_dir * 20.0,
                    inward_normal,
                )
            }
        } else {
            let (min, max) = geo.mesh.bounding_box().unwrap_or(([-30.0, -30.0, -30.0], [30.0, 30.0, 30.0]));
            let mid_y = (min[1] + max[1]) * 0.5;
            let top_z = max[2];
            (
                glam::dvec3(min[0] as f64, mid_y as f64, top_z as f64),
                glam::dvec3(max[0] as f64, mid_y as f64, top_z as f64),
                glam::dvec3(0.0, 0.0, -1.0),
            )
        };

        // Coba fusion dengan arah normal yang dihitung; jika gagal (misal arah keluar vs masuk), coba arah sebaliknya
        let res = ducad_kernel::create_rib(&geo.shape, p_start, p_end, normal, thickness, depth, draft_deg)
            .or_else(|_| ducad_kernel::create_rib(&geo.shape, p_start, p_end, -normal, thickness, depth, draft_deg));

        match res {
            Ok(shape) => {
                let new_geo = BodyGeometry::from_shape(shape);
                let orig_name = self
                    .model
                    .doc
                    .bodies
                    .get(id)
                    .map(|b| b.name.clone())
                    .unwrap_or_else(|| "Solid".to_string());
                self.execute_model_command(
                    Box::new(ReplaceGeometryCommand::new("Add Rib", id, new_geo)),
                    &format!("Menambahkan tulang penguat (Rib t={:.1} mm) pada '{}'", thickness, orig_name),
                );
                self.round_history.remove(&id);
                self.rib_start_pt = None;
                self.rib_end_pt = None;
                self.active_face = None;
                self.picking_mode = crate::types::PickMode::None;
                self.model_status = Some(format!("Tulang penguat (Rib) pada '{}' berhasil ditambahkan ✓", orig_name));
                self.set_tool(crate::types::ToolKind::Select);
            }
            Err(e) => {
                self.model_status = Some(format!("Gagal menambahkan Rib: {e}"));
            }
        }
    }

    /// Extrude (push-pull) sisi/face 3D yang sedang aktif dengan jarak tertentu.
    pub fn extrude_active_face(&mut self, distance: f64) {
        let Some((target_id, ray, _)) = self.active_face.as_ref().map(|(id, r, _)| (*id, *r, ())) else {
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
        let Some((target_id, ray, _)) = self.active_face.as_ref().map(|(id, r, _)| (*id, *r, ())) else {
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
                let deg_label = angle_deg.map(|d| format!("{:.0}°", d)).unwrap_or_else(|| "360°".to_string());
                self.execute_model_command(
                    Box::new(ReplaceGeometryCommand::new("Revolve Face", target_id, new_geo)),
                    &format!("Memutar sisi solid {} mengelilingi sumbu", deg_label),
                );
                self.round_history.remove(&target_id);
                self.active_face = None;
                self.model_status = Some(format!("Revolve face {} sukses", deg_label));
                true
            }
            Err(e) => {
                self.model_status = Some(format!("Revolve face gagal: {e}"));
                false
            }
        }
    }

    /// Jadikan permukaan sisi 3D yang aktif sebagai bidang sketsa baru.
    pub fn sketch_on_active_face(&mut self) {
        let Some((_target_id, _ray, hit)) = &self.active_face else {
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

    /// Terapkan kemiringan cetakan (draft angle) ke face planar yang sedang aktif
    /// atau kumpulan face terpilih. Neutral plane diambil dari posisi Y=0 dengan
    /// arah normal sejajar `pull_dir` (menghasilkan garis netral di bidang dasar
    /// benda — perilaku umum untuk benda cetakan plastik dengan bidang pisah horizontal).
    pub fn apply_draft_angle(
        &mut self,
        angle_deg: f64,
        pull_dir: (f64, f64, f64),
    ) {
        use glam::DVec3;

        // Kumpulkan rays — prioritas: selected_faces (dari picking), fallback active_face.
        let (target_id, rays): (ducad_core::BodyId, Vec<ducad_kernel::PickRay>) = {
            if !self.selected_faces.is_empty() {
                // Mode: beberapa face dipilih lewat PickMode::Face
                let Some(&id) = self.selected_bodies.iter().next() else {
                    self.model_status = Some(
                        "Pilih body 3D dan minimal 1 face planar untuk Draft Angle".to_string(),
                    );
                    return;
                };
                (id, self.selected_faces.clone())
            } else if let Some((id, ray, _)) = self.active_face {
                // Mode: satu face di-hover/klik di viewport
                (id, vec![ray])
            } else {
                self.model_status = Some(
                    "Pilih minimal 1 face planar pada objek untuk Draft Angle".to_string(),
                );
                return;
            }
        };

        let Some(geo) = self.model.geometry.get(target_id) else {
            self.model_status = Some("Geometri body tidak ditemukan".to_string());
            return;
        };

        // Neutral plane: Z=0 (bidang dasar) dengan normal = pull_dir
        // (pengguna bisa mengatur ini nanti via dialog jika diperlukan).
        let neutral_plane_point = DVec3::ZERO;
        let neutral_plane_normal = DVec3::new(pull_dir.0, pull_dir.1, pull_dir.2);
        let pull_direction = DVec3::new(pull_dir.0, pull_dir.1, pull_dir.2);

        match ducad_kernel::draft_angle(
            &geo.shape,
            neutral_plane_point,
            neutral_plane_normal,
            pull_direction,
            angle_deg,
            &rays,
        ) {
            Ok(shape) => {
                let new_geo = BodyGeometry::from_shape(shape);
                self.execute_model_command(
                    Box::new(ReplaceGeometryCommand::new("Draft Angle", target_id, new_geo)),
                    &format!(
                        "Memberi kemiringan cetakan {:.1}° ke face (arah pull {:?})",
                        angle_deg, pull_dir
                    ),
                );
                self.round_history.remove(&target_id);
                self.selected_faces.clear();
                self.active_face = None;
                self.picking_mode = crate::types::PickMode::None;
                self.model_status = Some(format!(
                    "Draft Angle {:.1}° diterapkan ✓",
                    angle_deg
                ));
                self.set_tool(crate::types::ToolKind::Select);
            }
            Err(e) => {
                self.model_status = Some(format!("Draft Angle gagal: {e}"));
            }
        }
    }

    /// Terapkan operasi Split Body atau Split Face.
    pub fn apply_split(
        &mut self,
        mode: ducad_ui::SplitMode,
        plane_kind: ducad_ui::SplitPlaneKind,
        offset_mm: f64,
    ) {
        use glam::DVec3;

        // Tentukan target body:
        let target_id = if let Some(&id) = self.selected_bodies.iter().next() {
            id
        } else if let Some((id, _, _)) = self.active_face {
            id
        } else {
            self.model_status = Some("Pilih body 3D yang ingin dipotong".to_string());
            return;
        };

        let Some(geo) = self.model.geometry.get(target_id) else {
            self.model_status = Some("Geometri body tidak ditemukan".to_string());
            return;
        };

        let orig_name = self.model.doc.bodies
            .get(target_id)
            .map(|b| b.name.clone())
            .unwrap_or_else(|| "Body".to_string());

        let center = geo.mesh.center();

        // Hitung titik dan normal bidang pemotong (default di tengah body)
        let (plane_point, plane_normal) = match plane_kind {
            ducad_ui::SplitPlaneKind::XY => {
                (
                    DVec3::new(center[0] as f64, center[1] as f64, center[2] as f64 + offset_mm),
                    DVec3::new(0.0, 0.0, 1.0),
                )
            }
            ducad_ui::SplitPlaneKind::XZ => {
                (
                    DVec3::new(center[0] as f64, center[1] as f64 + offset_mm, center[2] as f64),
                    DVec3::new(0.0, 1.0, 0.0),
                )
            }
            ducad_ui::SplitPlaneKind::YZ => {
                (
                    DVec3::new(center[0] as f64 + offset_mm, center[1] as f64, center[2] as f64),
                    DVec3::new(1.0, 0.0, 0.0),
                )
            }
            ducad_ui::SplitPlaneKind::PickedFace => {
                if let Some((_, _, hit)) = &self.active_face {
                    let normal = DVec3::new(hit.normal.0, hit.normal.1, hit.normal.2).normalize();
                    let origin = DVec3::new(center[0] as f64, center[1] as f64, center[2] as f64) + normal * offset_mm;
                    (origin, normal)
                } else {
                    (
                        DVec3::new(center[0] as f64, center[1] as f64, center[2] as f64 + offset_mm),
                        DVec3::new(0.0, 0.0, 1.0),
                    )
                }
            }
        };

        match mode {
            ducad_ui::SplitMode::SplitBody => {
                match ducad_kernel::split_body(&geo.shape, plane_point, plane_normal) {
                    Ok(parts) => {
                        if parts.len() < 2 {
                            self.model_status = Some("Bidang pemotong tidak memotong body menjadi bagian terpisah (coba geser offset)".to_string());
                            return;
                        }

                        let total = parts.len();
                        let result_bodies = parts
                            .into_iter()
                            .enumerate()
                            .map(|(i, shape)| {
                                let name = format!("{} (Bagian {})", orig_name, i + 1);
                                (name, BodyGeometry::from_shape(shape))
                            })
                            .collect::<Vec<_>>();

                        let cmd = crate::model::SplitBodyCommand::new(target_id, result_bodies);
                        self.execute_model_command(
                            Box::new(cmd),
                            &format!("Memotong body '{}' menjadi {} bagian terpisah", orig_name, total),
                        );

                        self.round_history.remove(&target_id);
                        self.selected_bodies.clear();
                        self.active_face = None;
                        self.selected_faces.clear();
                        self.picking_mode = crate::types::PickMode::None;
                        self.model_status = Some(format!(
                            "Body '{}' berhasil dipotong menjadi {} body terpisah ✓",
                            orig_name, total
                        ));
                        self.set_tool(crate::types::ToolKind::Select);
                    }
                    Err(e) => {
                        self.model_status = Some(format!("Split Body gagal: {e}"));
                    }
                }
            }
            ducad_ui::SplitMode::SplitFace => {
                match ducad_kernel::split_face(&geo.shape, plane_point, plane_normal) {
                    Ok(new_shape) => {
                        let new_geo = BodyGeometry::from_shape(new_shape);
                        self.execute_model_command(
                            Box::new(ReplaceGeometryCommand::new("Split Face", target_id, new_geo)),
                            &format!("Membagi face pada body '{}'", orig_name),
                        );

                        self.round_history.remove(&target_id);
                        self.active_face = None;
                        self.selected_faces.clear();
                        self.picking_mode = crate::types::PickMode::None;
                        self.model_status = Some(format!(
                            "Face pada body '{}' berhasil dibagi ✓",
                            orig_name
                        ));
                        self.set_tool(crate::types::ToolKind::Select);
                    }
                    Err(e) => {
                        self.model_status = Some(format!("Split Face gagal: {e}"));
                    }
                }
            }
        }
    }

    /// Terapkan Linear atau Circular Pattern pada entitas sketsa 2D terpilih.
    pub fn apply_pattern_2d(&mut self) {
        if self.selected.is_empty() {
            self.model_status = Some("Pilih minimal 1 entitas sketsa untuk membuat pattern".to_string());
            return;
        }

        let entities: Vec<ducad_sketch::Entity> = self
            .selected
            .iter()
            .filter_map(|id| self.sketch().entities.get(*id).cloned())
            .collect();

        if entities.is_empty() {
            return;
        }

        let new_entities = match self.pattern_kind {
            ducad_ui::PatternKind::Linear => {
                ducad_sketch::linear_pattern_entities(
                    &entities,
                    self.pattern_count_x,
                    self.pattern_pitch_x,
                    self.pattern_count_y,
                    self.pattern_pitch_y,
                )
            }
            ducad_ui::PatternKind::Circular => {
                let pivot = self.pattern_custom_pivot_2d.unwrap_or(DVec2::ZERO);
                let total_angle_rad = self.pattern_circ_angle_deg.to_radians();
                ducad_sketch::circular_pattern_entities_with_radius(
                    &entities,
                    pivot,
                    self.pattern_circ_count,
                    total_angle_rad,
                    Some(self.pattern_circ_radius),
                )
            }
        };

        if new_entities.is_empty() {
            self.model_status = Some("Pattern tidak menghasilkan duplikat baru (jumlah minimal 2)".to_string());
            return;
        }

        let count = new_entities.len();
        let cmd = ducad_sketch::InsertEntities::new(
            match self.pattern_kind {
                ducad_ui::PatternKind::Linear => "Linear Pattern",
                ducad_ui::PatternKind::Circular => "Circular Pattern",
            },
            new_entities,
        );
        self.execute_sketch_command(Box::new(cmd));
        self.selected.clear();
        self.active_sketch_corner = None;
        self.active_sketch_fillet_arc = None;
        self.sketch_corner_gizmo_active = false;
        self.hovered_corner_2d = None;
        self.pattern_custom_pivot_2d = None;
        self.pattern_custom_pivot_3d = None;
        self.model_status = Some(format!("{} entitas baru ditambahkan via Pattern ✓", count));
        self.set_tool(crate::types::ToolKind::Select);
    }

    /// Terapkan Linear atau Circular Pattern pada body 3D terpilih.
    pub fn apply_pattern_3d(&mut self) {
        if self.selected_bodies.is_empty() {
            self.model_status = Some("Pilih minimal 1 body 3D untuk membuat pattern".to_string());
            return;
        }

        let selected_ids: Vec<BodyId> = self.selected_bodies.iter().copied().collect();
        let mut new_bodies = Vec::new();

        for body_id in selected_ids {
            let orig_name = self.model.doc.bodies.get(body_id).map(|b| b.name.clone()).unwrap_or_else(|| "Solid".to_string());
            let Some(geo) = self.model.geometry.get(body_id) else {
                continue;
            };

            let duplicated_shapes = match self.pattern_kind {
                ducad_ui::PatternKind::Linear => {
                    ducad_kernel::linear_pattern_shape(
                        &geo.shape,
                        self.pattern_count_x,
                        self.pattern_pitch_x,
                        self.pattern_count_y,
                        self.pattern_pitch_y,
                        self.pattern_count_z,
                        self.pattern_pitch_z,
                    )
                }
                ducad_ui::PatternKind::Circular => {
                    let pivot = self.pattern_custom_pivot_3d
                        .map(|v| (v.x as f64, v.y as f64, v.z as f64))
                        .unwrap_or((0.0, 0.0, 0.0));
                    let axis = self.pattern_circ_axis.to_dir();
                    let total_angle_rad = self.pattern_circ_angle_deg.to_radians();

                    ducad_kernel::circular_pattern_shape(
                        &geo.shape,
                        pivot,
                        axis,
                        self.pattern_circ_count,
                        total_angle_rad,
                    )
                }
            };

            match duplicated_shapes {
                Ok(shapes) => {
                    for (idx, shape) in shapes.into_iter().enumerate() {
                        let name = format!("{} (Array {})", orig_name, idx + 1);
                        new_bodies.push((name, BodyGeometry::from_shape(shape)));
                    }
                }
                Err(e) => {
                    self.model_status = Some(format!("Pattern 3D gagal: {e}"));
                    return;
                }
            }
        }

        if new_bodies.is_empty() {
            self.model_status = Some("Pattern tidak menghasilkan duplikat baru (jumlah minimal 2)".to_string());
            return;
        }

        let total_new = new_bodies.len();
        let cmd = crate::model::AddMultipleSolidsCommand::new(
            match self.pattern_kind {
                ducad_ui::PatternKind::Linear => "Linear Pattern 3D",
                ducad_ui::PatternKind::Circular => "Circular Pattern 3D",
            },
            new_bodies,
        );

        self.execute_model_command(
            Box::new(cmd),
            &format!("Membuat {} duplikat solid melalui Pattern", total_new),
        );

        self.selected_bodies.clear();
        self.pattern_custom_pivot_2d = None;
        self.pattern_custom_pivot_3d = None;
        self.model_status = Some(format!("{} solid baru ditambahkan via Pattern 3D ✓", total_new));
        self.set_tool(crate::types::ToolKind::Select);
    }

    /// Terapkan operasi pembuatan lubang (*Hole Wizard*) pada face bodi 3D yang aktif.
    pub fn apply_hole_wizard(&mut self, spec: ducad_core::hole::HoleSpec) {
        let (body_id, hit) = match &self.active_face {
            Some((id, _, hit)) => (*id, hit.clone()),
            None => {
                self.alert_modal.show_error(
                    "Hole Wizard Gagal",
                    "Tidak ada face yang dipilih. Klik salah satu permukaan datar pada objek 3D terlebih dahulu.",
                    vec![
                        "Gunakan Tool Pilih (S) dan klik permukaan datar (face).",
                        "Buka kembali Hole Wizard dari menu aksi di bagian bawah layar.",
                    ],
                );
                self.model_status = Some("Pilih permukaan face untuk membuat lubang".to_string());
                return;
            }
        };

        let geo = match self.model.geometry.get(body_id) {
            Some(g) => g,
            None => {
                self.model_status = Some("Geometri solid tidak ditemukan".to_string());
                return;
            }
        };

        let (hole_pos, _, _, _) = self.compute_active_hole_position_and_basis(&hit);
        let pos = (hole_pos.x as f64, hole_pos.y as f64, hole_pos.z as f64);
        let normal = hit.normal;

        let new_feature = crate::types::HoleFeature {
            spec: spec.clone(),
            pos,
            normal,
            face_hit: hit.clone(),
            offset_u: self.hole_popup_state.offset_u,
            offset_v: self.hole_popup_state.offset_v,
        };

        let wants_edit = self.hole_popup_state.mode == ducad_ui::HoleOperationMode::EditHole;

        let (new_shape_res, is_reedit) = if wants_edit {
            if let Some(hist) = self.hole_history.get_mut(&body_id) {
                if !hist.features.is_empty() {
                    // Tentukan indeks fitur yang diedit
                    let target_idx = if let Some((edit_body, edit_idx)) = self.editing_hole_idx {
                        if edit_body == body_id && edit_idx < hist.features.len() {
                            edit_idx
                        } else {
                            hist.features.len() - 1
                        }
                    } else {
                        // Cari fitur lubang yang paling dekat dengan titik target
                        let mut best_idx = hist.features.len() - 1;
                        let mut best_dist_sq = f64::MAX;
                        for (i, f) in hist.features.iter().enumerate() {
                            let d2 = (f.pos.0 - pos.0).powi(2)
                                + (f.pos.1 - pos.1).powi(2)
                                + (f.pos.2 - pos.2).powi(2);
                            if d2 < best_dist_sq {
                                best_dist_sq = d2;
                                best_idx = i;
                            }
                        }
                        best_idx
                    };

                    hist.features[target_idx] = new_feature.clone();

                    // Rebuild all holes from base shape
                    match ducad_kernel::clone_shape(&hist.base) {
                        Ok(mut curr_shape) => {
                            let mut err = None;
                            for feat in &hist.features {
                                match ducad_kernel::apply_hole(&curr_shape, &feat.spec, feat.pos, feat.normal) {
                                    Ok(sh) => curr_shape = sh,
                                    Err(e) => {
                                        err = Some(e);
                                        break;
                                    }
                                }
                            }
                            if let Some(e) = err {
                                (Err(e), true)
                            } else {
                                (Ok(curr_shape), true)
                            }
                        }
                        Err(e) => (Err(e), true),
                    }
                } else {
                    (ducad_kernel::apply_hole(&geo.shape, &spec, pos, normal), false)
                }
            } else {
                (ducad_kernel::apply_hole(&geo.shape, &spec, pos, normal), false)
            }
        } else {
            // Lubang baru pada solid body (New Hole)
            match ducad_kernel::apply_hole(&geo.shape, &spec, pos, normal) {
                Ok(sh) => {
                    if !self.hole_history.contains_key(&body_id) {
                        if let Ok(base_sh) = ducad_kernel::clone_shape(&geo.shape) {
                            self.hole_history.insert(
                                body_id,
                                crate::types::HoleHistory {
                                    base: base_sh,
                                    features: Vec::new(),
                                },
                            );
                        }
                    }
                    if let Some(hist) = self.hole_history.get_mut(&body_id) {
                        hist.features.push(new_feature);
                    }
                    (Ok(sh), false)
                }
                Err(e) => (Err(e), false),
            }
        };

        match new_shape_res {
            Ok(new_shape) => {
                let new_geo = BodyGeometry::from_shape(new_shape);
                let callout = spec.technical_callout();
                let history_msg = if is_reedit {
                    format!("Edit Hole: {callout}")
                } else {
                    format!("Hole Wizard: {callout}")
                };

                self.execute_model_command(
                    Box::new(ReplaceGeometryCommand::new(
                        "Hole Wizard",
                        body_id,
                        new_geo,
                    )),
                    &history_msg,
                );

                self.active_face = None;
                self.editing_hole_idx = None;
                self.hole_popup_state.offset_u = 0.0;
                self.hole_popup_state.offset_v = 0.0;
                self.hole_popup_state.current_pos_3d = None;
                self.model_status = Some(ducad_i18n::t!("hole-applied", callout = &callout));
            }
            Err(e) => {
                self.alert_modal.show_error(
                    "Operasi Hole Wizard Gagal",
                    format!("{e}"),
                    vec![
                        "Pastikan diameter lubang tidak melebihi dimensi permukaan benda.",
                        "Untuk lubang berkedalaman (blind), pastikan kedalaman tidak melebihi ketebalan benda atau gunakan opsi Tembus (Through All).",
                        "Untuk lubang bertingkat (Counterbore/Countersink), pastikan diameter kepala lebih besar dari diameter lubang utama.",
                    ],
                );
                self.model_status = Some(format!("Hole Wizard gagal: {e}"));
            }
        }
    }
}
