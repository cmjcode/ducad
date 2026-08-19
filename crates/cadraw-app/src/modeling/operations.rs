use cadraw_core::BodyId;
use cadraw_kernel::PickRay;
use cadraw_render::SketchPlane;
use cadraw_sketch::constraint::{self, AddConstraint, Constraint};
use glam::Vec3;

use crate::app::CadrawApp;
use crate::model::{
    AddSolidCommand, BodyGeometry, BooleanCommand, BooleanKind, DeleteBodyCommand,
    ReplaceGeometryCommand,
};
use crate::types::PickMode;

impl CadrawApp {
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
                self.model_undo.execute(
                    Box::new(AddSolidCommand::new("Extrude", geo)),
                    &mut self.model,
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

        match cadraw_kernel::revolve_profile(&profile, axis_origin, axis_dir, angle_deg) {
            Ok(shape) => {
                let geo = BodyGeometry::from_shape(shape);
                self.model_undo.execute(
                    Box::new(AddSolidCommand::new("Revolve", geo)),
                    &mut self.model,
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
        preset: cadraw_ui::RevolveAxisPreset,
        angle_deg: f64,
    ) -> bool {
        let angle_opt = if (angle_deg - 360.0).abs() < 1e-4 {
            None
        } else {
            Some(angle_deg)
        };

        let bbox = crate::model::compute_profile_bbox(self.sketch(), &self.selected);

        let (axis_origin, axis_dir) = match preset {
            cadraw_ui::RevolveAxisPreset::YAxisOrigin => ((0.0, 0.0), (0.0, 1.0)),
            cadraw_ui::RevolveAxisPreset::XAxisOrigin => ((0.0, 0.0), (1.0, 0.0)),
            cadraw_ui::RevolveAxisPreset::BBoxLeft => {
                if let Some([min_x, min_y, _, _]) = bbox {
                    ((min_x, min_y), (0.0, 1.0))
                } else {
                    ((0.0, 0.0), (0.0, 1.0))
                }
            }
            cadraw_ui::RevolveAxisPreset::BBoxRight => {
                if let Some([_, min_y, max_x, _]) = bbox {
                    ((max_x, min_y), (0.0, 1.0))
                } else {
                    ((0.0, 0.0), (0.0, 1.0))
                }
            }
            cadraw_ui::RevolveAxisPreset::BBoxBottom => {
                if let Some([min_x, min_y, _, _]) = bbox {
                    ((min_x, min_y), (1.0, 0.0))
                } else {
                    ((0.0, 0.0), (1.0, 0.0))
                }
            }
            cadraw_ui::RevolveAxisPreset::BBoxTop => {
                if let Some([min_x, _, _, max_y]) = bbox {
                    ((min_x, max_y), (1.0, 0.0))
                } else {
                    ((0.0, 0.0), (1.0, 0.0))
                }
            }
            cadraw_ui::RevolveAxisPreset::CustomTwoPoints => {
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

    /// Loft antara `pending_loft_bottom` dan profil dari seleksi sketch saat ini.
    pub fn loft_selected(&mut self) {
        let Some(bottom) = self.pending_loft_bottom.clone() else {
            self.model_status =
                Some("Set Profil Bawah dari Seleksi dulu sebelum Loft".to_string());
            return;
        };
        let height: f64 = match self.loft_height_input.trim().parse() {
            Ok(v) => v,
            Err(_) => {
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
        match cadraw_kernel::loft_profiles(&bottom, &top, height) {
            Ok(shape) => {
                let geo = BodyGeometry::from_shape(shape);
                self.model_undo
                    .execute(Box::new(AddSolidCommand::new("Loft", geo)), &mut self.model);
                self.pending_loft_bottom = None;
                self.model_status = None;
            }
            Err(e) => self.model_status = Some(format!("Loft gagal: {e}")),
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
        match BooleanCommand::try_new(&self.model, kind, label, result_name, a_id, b_id) {
            Ok(cmd) => {
                self.model_undo.execute(Box::new(cmd), &mut self.model);
                self.selected_bodies.clear();
                self.round_history.remove(&a_id);
                self.round_history.remove(&b_id);
                self.model_status = None;
            }
            Err(msg) => self.model_status = Some(msg),
        }
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
            cadraw_kernel::fillet_all(&geo.shape, radius)
        } else {
            cadraw_kernel::fillet_edges(
                &geo.shape,
                radius,
                &rays,
                Self::EDGE_REAPPLY_TOLERANCE_MM,
            )
        };
        match result {
            Ok(shape) => {
                let new_geo = BodyGeometry::from_shape(shape);
                self.model_undo.execute(
                    Box::new(ReplaceGeometryCommand::new("Fillet", id, new_geo)),
                    &mut self.model,
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
            cadraw_kernel::chamfer_all(&geo.shape, distance)
        } else {
            cadraw_kernel::chamfer_edges(
                &geo.shape,
                distance,
                &rays,
                Self::EDGE_REAPPLY_TOLERANCE_MM,
            )
        };
        match result {
            Ok(shape) => {
                let new_geo = BodyGeometry::from_shape(shape);
                self.model_undo.execute(
                    Box::new(ReplaceGeometryCommand::new("Chamfer", id, new_geo)),
                    &mut self.model,
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
            cadraw_kernel::shell_hollow(&geo.shape, thickness, self.shell_direction)
        } else {
            cadraw_kernel::shell_hollow_faces(&geo.shape, thickness, &self.selected_faces)
        };
        match result {
            Ok(shape) => {
                let new_geo = BodyGeometry::from_shape(shape);
                self.model_undo.execute(
                    Box::new(ReplaceGeometryCommand::new("Shell", id, new_geo)),
                    &mut self.model,
                );
                self.round_history.remove(&id);
                self.selected_faces.clear();
                self.picking_mode = PickMode::None;
                self.model_status = None;
            }
            Err(e) => self.model_status = Some(format!("Shell gagal: {e}")),
        }
    }

    /// Extrude sisi/face 3D yang sedang aktif sepanjang `distance` mm.
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
        match cadraw_kernel::extrude_face(&target_geo.shape, ray, distance) {
            Ok(new_shape) => {
                let new_geo = BodyGeometry::from_shape(new_shape);
                let label = if distance > 0.0 {
                    "Extrude Face"
                } else {
                    "Cut Face"
                };
                self.model_undo.execute(
                    Box::new(ReplaceGeometryCommand::new(label, target_id, new_geo)),
                    &mut self.model,
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
            self.model_undo
                .execute(Box::new(DeleteBodyCommand::new(id)), &mut self.model);
            self.round_history.remove(&id);
        }
        self.body_move_armed = false;
        self.body_move_target = None;
    }
}
