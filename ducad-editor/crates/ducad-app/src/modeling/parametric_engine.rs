//! Engine Regenerasi & Perekaman Riwayat Parametrik (Feature DAG).
//!
//! Mengatur evaluasi topologis, pembaharuan geometri solid body 3D secara otomatis
//! saat dimensi sketsa atau parameter fitur masa lalu diedit.

use ducad_core::parametric::{FeatureId, FeaturePayload, SketchPlaneRef};
use ducad_core::BodyId;
use ducad_kernel::KernelShape;
use ducad_sketch::Entity;
use std::collections::HashMap;

use crate::app::DuCADApp;
use crate::model::BodyGeometry;

impl DuCADApp {
    /// Catat langkah sketsa 2D baru ke dalam DAG.
    pub fn record_sketch_feature(&mut self, plane_idx: usize, description: impl Into<String>) -> FeatureId {
        let (plane_ref, dep_id) = if plane_idx < 3 {
            let pref = match plane_idx {
                0 => SketchPlaneRef::Top,
                1 => SketchPlaneRef::Front,
                2 => SketchPlaneRef::Right,
                _ => SketchPlaneRef::Top,
            };
            (pref, None)
        } else {
            let custom_idx = plane_idx - 3;
            let datum_id = self.datum_planes.get(custom_idx).map(|dp| dp.id).unwrap_or(0);
            let dep = self.parametric_dag.nodes.iter().find_map(|n| {
                if let FeaturePayload::DatumPlane { datum_id: id, .. } = n.payload {
                    if id == datum_id {
                        return Some(n.id);
                    }
                }
                None
            });
            (SketchPlaneRef::CustomDatum(datum_id), dep)
        };

        let sketch = if plane_idx < self.sketches.len() {
            &self.sketches[plane_idx]
        } else {
            &self.sketches[0]
        };

        let entity_count = sketch.entities.len();
        let (shape_type, dim_w, dim_h) = if entity_count == 1 {
            let ent = sketch.entities.iter().next().map(|(_, e)| e);
            match ent {
                Some(Entity::Circle { radius, .. }) => ("Lingkaran".to_string(), *radius, None),
                Some(Entity::Arc { radius, .. }) => ("Busur".to_string(), *radius, None),
                Some(Entity::Ellipse { radius_x, radius_y, .. }) => ("Elips".to_string(), *radius_x, Some(*radius_y)),
                Some(Entity::Line { start, end, .. }) => ("Garis".to_string(), (*end - *start).length(), None),
                _ => ("Entitas 2D".to_string(), 10.0, None),
            }
        } else if let Some((min, max)) = sketch.bounding_box() {
            let size = max - min;
            let w = if size.x > 1e-4 { size.x } else { 10.0 };
            let h = if size.y > 1e-4 { size.y } else { 10.0 };
            let shape = if entity_count == 4 || entity_count == 5 {
                "Persegi / Kotak".to_string()
            } else {
                "Profil Sketsa".to_string()
            };
            (shape, w, Some(h))
        } else {
            ("Sketsa 2D".to_string(), 10.0, None)
        };

        // Cek apakah sudah ada Sketch feature pada plane ini yang belum diextrude:
        if let Some(existing_node) = self.parametric_dag.nodes.iter_mut().rev().find(|n| {
            if let FeaturePayload::Sketch { plane_index, .. } = n.payload {
                plane_index == plane_idx
            } else {
                false
            }
        }) {
            existing_node.payload = FeaturePayload::Sketch {
                plane_ref,
                plane_index: plane_idx,
                entity_count,
                dim_w,
                dim_h,
                shape_type,
                description: description.into(),
            };
            return existing_node.id;
        }

        let mut deps = Vec::new();
        if let Some(d) = dep_id {
            deps.push(d);
        }

        let name = format!("Sketch {}", self.parametric_dag.nodes.len() + 1);
        self.parametric_dag.add_feature(
            name,
            FeaturePayload::Sketch {
                plane_ref,
                plane_index: plane_idx,
                entity_count,
                dim_w,
                dim_h,
                shape_type,
                description: description.into(),
            },
            deps,
        )
    }

    /// Catat langkah Extrude ke dalam DAG.
    pub fn record_extrude_feature(&mut self, distance: f64, is_cut: bool) -> FeatureId {
        let plane_idx = self.active_plane_index();
        // Cari sketch feature terakhir di plane ini atau buat baru
        let sketch_id = self.parametric_dag.nodes.iter().rev().find_map(|n| {
            if let FeaturePayload::Sketch { plane_index, .. } = n.payload {
                if plane_index == plane_idx {
                    return Some(n.id);
                }
            }
            None
        }).unwrap_or_else(|| self.record_sketch_feature(plane_idx, "Sketch"));

        let name = if is_cut {
            format!("Cut Extrude {}", self.parametric_dag.nodes.len() + 1)
        } else {
            format!("Extrude Boss {}", self.parametric_dag.nodes.len() + 1)
        };

        self.parametric_dag.add_feature(
            name,
            FeaturePayload::Extrude {
                sketch_id,
                distance,
                plane_index: plane_idx,
                is_cut,
            },
            vec![sketch_id],
        )
    }

    /// Catat langkah Revolve ke dalam DAG.
    pub fn record_revolve_feature(
        &mut self,
        angle_deg: f64,
        axis_origin: (f64, f64),
        axis_dir: (f64, f64),
    ) -> FeatureId {
        let plane_idx = self.active_plane_index();
        let sketch_id = self.parametric_dag.nodes.iter().rev().find_map(|n| {
            if let FeaturePayload::Sketch { plane_index, .. } = n.payload {
                if plane_index == plane_idx {
                    return Some(n.id);
                }
            }
            None
        }).unwrap_or_else(|| self.record_sketch_feature(plane_idx, "Sketch"));

        let name = format!("Revolve {}", self.parametric_dag.nodes.len() + 1);
        self.parametric_dag.add_feature(
            name,
            FeaturePayload::Revolve {
                sketch_id,
                angle_deg,
                axis_origin,
                axis_dir,
                plane_index: plane_idx,
            },
            vec![sketch_id],
        )
    }

    /// Catat langkah Fillet ke dalam DAG.
    pub fn record_fillet_feature(&mut self, radius: f64, radius_end: Option<f64>) -> FeatureId {
        let parent_id = self.parametric_dag.nodes.iter().rev().find_map(|n| {
            match n.payload {
                FeaturePayload::Extrude { .. }
                | FeaturePayload::Revolve { .. }
                | FeaturePayload::Fillet { .. }
                | FeaturePayload::Chamfer { .. }
                | FeaturePayload::Shell { .. } => Some(n.id),
                _ => None,
            }
        }).unwrap_or(0);

        let name = format!("Fillet {}", self.parametric_dag.nodes.len() + 1);
        self.parametric_dag.add_feature(
            name,
            FeaturePayload::Fillet {
                target_feature_id: parent_id,
                radius,
                radius_end,
            },
            if parent_id > 0 { vec![parent_id] } else { vec![] },
        )
    }

    /// Catat langkah Chamfer ke dalam DAG.
    pub fn record_chamfer_feature(&mut self, distance: f64) -> FeatureId {
        let parent_id = self.parametric_dag.nodes.iter().rev().find_map(|n| {
            match n.payload {
                FeaturePayload::Extrude { .. }
                | FeaturePayload::Revolve { .. }
                | FeaturePayload::Fillet { .. }
                | FeaturePayload::Chamfer { .. }
                | FeaturePayload::Shell { .. } => Some(n.id),
                _ => None,
            }
        }).unwrap_or(0);

        let name = format!("Chamfer {}", self.parametric_dag.nodes.len() + 1);
        self.parametric_dag.add_feature(
            name,
            FeaturePayload::Chamfer {
                target_feature_id: parent_id,
                distance,
            },
            if parent_id > 0 { vec![parent_id] } else { vec![] },
        )
    }

    /// Catat langkah Hole Wizard ke dalam DAG.
    pub fn record_hole_feature(
        &mut self,
        spec: ducad_core::hole::HoleSpec,
        pos: (f64, f64, f64),
        normal: (f64, f64, f64),
    ) -> FeatureId {
        let parent_id = self.parametric_dag.nodes.iter().rev().find_map(|n| {
            match n.payload {
                FeaturePayload::Extrude { .. }
                | FeaturePayload::Revolve { .. }
                | FeaturePayload::Fillet { .. }
                | FeaturePayload::Chamfer { .. } => Some(n.id),
                _ => None,
            }
        }).unwrap_or(0);

        let name = format!("Hole {}", self.parametric_dag.nodes.len() + 1);
        self.parametric_dag.add_feature(
            name,
            FeaturePayload::Hole {
                target_feature_id: parent_id,
                spec,
                pos,
                normal,
            },
            if parent_id > 0 { vec![parent_id] } else { vec![] },
        )
    }

    /// Catat pembuatan Datum Plane ke dalam DAG.
    pub fn record_datum_plane_feature(
        &mut self,
        datum_id: u32,
        offset: f64,
        angle: f64,
        mode_desc: String,
    ) -> FeatureId {
        let name = format!("Datum Plane {datum_id}");
        self.parametric_dag.add_feature(
            name,
            FeaturePayload::DatumPlane {
                datum_id,
                offset,
                angle,
                mode_desc,
            },
            vec![],
        )
    }

    /// Update parameter sebuah fitur dan jalankan regenerasi topologis downstream.
    pub fn save_feature_params_and_regenerate(
        &mut self,
        id: FeatureId,
        val1: f64,
        val2: Option<f64>,
    ) -> Result<(), String> {
        let Some(existing) = self.parametric_dag.get_feature(id).cloned() else {
            return Err("Fitur tidak ditemukan".to_string());
        };

        let new_payload = match existing.payload {
            FeaturePayload::Extrude { sketch_id, plane_index, is_cut, .. } => {
                FeaturePayload::Extrude {
                    sketch_id,
                    distance: val1,
                    plane_index,
                    is_cut,
                }
            }
            FeaturePayload::Revolve { sketch_id, axis_origin, axis_dir, plane_index, .. } => {
                FeaturePayload::Revolve {
                    sketch_id,
                    angle_deg: val1,
                    axis_origin,
                    axis_dir,
                    plane_index,
                }
            }
            FeaturePayload::Fillet { target_feature_id, .. } => {
                FeaturePayload::Fillet {
                    target_feature_id,
                    radius: val1,
                    radius_end: val2,
                }
            }
            FeaturePayload::Chamfer { target_feature_id, .. } => {
                FeaturePayload::Chamfer {
                    target_feature_id,
                    distance: val1,
                }
            }
            FeaturePayload::Shell { target_feature_id, .. } => {
                FeaturePayload::Shell {
                    target_feature_id,
                    thickness: val1,
                }
            }
            FeaturePayload::Sketch { plane_ref, plane_index, entity_count, shape_type, description, .. } => {
                // Perbarui entitas sketsa aktif secara proporsional sesuai dimensi baru
                self.apply_sketch_dimension_update(plane_index, val1, val2);
                FeaturePayload::Sketch {
                    plane_ref,
                    plane_index,
                    entity_count,
                    dim_w: val1,
                    dim_h: val2,
                    shape_type,
                    description,
                }
            }
            FeaturePayload::DatumPlane { datum_id, mode_desc, .. } => {
                FeaturePayload::DatumPlane {
                    datum_id,
                    offset: val1,
                    angle: val2.unwrap_or(0.0),
                    mode_desc,
                }
            }
            FeaturePayload::Helix { wire_radius, turns, .. } => {
                FeaturePayload::Helix {
                    radius: val1,
                    pitch: val2.unwrap_or(10.0),
                    turns,
                    wire_radius,
                }
            }
            other => other,
        };

        self.parametric_dag.update_feature_payload(id, new_payload);
        self.regenerate_parametric_model()
    }

    /// Helper untuk mengubah ukuran entitas sketsa saat parameter dimensi di Feature Tree diedit.
    fn apply_sketch_dimension_update(&mut self, plane_index: usize, new_w: f64, new_h: Option<f64>) {
        if plane_index >= self.sketches.len() || new_w <= 0.0 {
            return;
        }

        let sketch = &mut self.sketches[plane_index];
        if sketch.entities.is_empty() {
            return;
        }

        if sketch.entities.len() == 1 {
            for (_, ent) in sketch.entities.iter_mut() {
                match ent {
                    Entity::Circle { radius, .. } => {
                        *radius = new_w;
                    }
                    Entity::Arc { radius, .. } => {
                        *radius = new_w;
                    }
                    Entity::Ellipse { radius_x, radius_y, .. } => {
                        *radius_x = new_w;
                        if let Some(h) = new_h {
                            *radius_y = h;
                        }
                    }
                    Entity::Line { start, end, .. } => {
                        let dir = (*end - *start).normalize_or_zero();
                        if dir.length_squared() > 0.0 {
                            *end = *start + dir * new_w;
                        }
                    }
                    _ => {}
                }
            }
            return;
        }

        if let Some((min, max)) = sketch.bounding_box() {
            let size = max - min;
            let old_w = if size.x > 1e-4 { size.x } else { 1.0 };
            let old_h = if size.y > 1e-4 { size.y } else { 1.0 };

            let scale_x = new_w / old_w;
            let scale_y = if let Some(h) = new_h {
                h / old_h
            } else {
                scale_x
            };

            let center = (min + max) * 0.5;

            for (_, ent) in sketch.entities.iter_mut() {
                match ent {
                    Entity::Line { start, end, .. } => {
                        start.x = center.x + (start.x - center.x) * scale_x;
                        start.y = center.y + (start.y - center.y) * scale_y;
                        end.x = center.x + (end.x - center.x) * scale_x;
                        end.y = center.y + (end.y - center.y) * scale_y;
                    }
                    Entity::Circle { center: c, radius, .. } => {
                        c.x = center.x + (c.x - center.x) * scale_x;
                        c.y = center.y + (c.y - center.y) * scale_y;
                        *radius *= scale_x.max(scale_y);
                    }
                    Entity::Arc { center: c, radius, .. } => {
                        c.x = center.x + (c.x - center.x) * scale_x;
                        c.y = center.y + (c.y - center.y) * scale_y;
                        *radius *= scale_x.max(scale_y);
                    }
                    Entity::Ellipse { center: c, radius_x, radius_y, .. } => {
                        c.x = center.x + (c.x - center.x) * scale_x;
                        c.y = center.y + (c.y - center.y) * scale_y;
                        *radius_x *= scale_x;
                        *radius_y *= scale_y;
                    }
                    Entity::Spline { points, .. } => {
                        for pt in points.iter_mut() {
                            pt.x = center.x + (pt.x - center.x) * scale_x;
                            pt.y = center.y + (pt.y - center.y) * scale_y;
                        }
                    }
                }
            }
        }
    }

    /// Eksekusi seluruh Feature Tree DAG secara topologis dan rekonstruksi bodi solid 3D.
    pub fn regenerate_parametric_model(&mut self) -> Result<(), String> {
        let order = self.parametric_dag.topological_order()?;
        let mut feature_shapes: HashMap<FeatureId, KernelShape> = HashMap::new();
        let mut body_map: HashMap<FeatureId, BodyId> = HashMap::new();

        // Cari body yang ada di model saat ini
        let existing_bodies: Vec<BodyId> = self.model.doc.bodies.iter().map(|(id, _)| id).collect();
        let mut body_idx = 0;

        for id in order {
            let Some(node) = self.parametric_dag.get_feature(id).cloned() else {
                continue;
            };

            if node.is_suppressed {
                continue;
            }

            match node.payload {
                FeaturePayload::DatumPlane { datum_id, offset, .. } => {
                    if let Some(dp) = self.datum_planes.iter_mut().find(|dp| dp.id == datum_id) {
                        dp.plane = dp.plane.offset(offset as f32);
                    }
                }
                FeaturePayload::Sketch { plane_index, .. } => {
                    // Pastikan sketsa siap dievaluasi
                    let _ = plane_index;
                }
                FeaturePayload::Extrude { distance, plane_index, .. } => {
                    let plane = self.plane_for_index(plane_index);
                    let sketch = if plane_index < self.sketches.len() {
                        &self.sketches[plane_index]
                    } else {
                        &self.sketches[0]
                    };

                    let all_ids: std::collections::HashSet<_> = sketch
                        .entities
                        .iter()
                        .filter(|(_, e)| !e.is_construction())
                        .map(|(eid, _)| eid)
                        .collect();

                    if let Ok(profile) = crate::model::build_profile_from_selection(sketch, &all_ids) {
                        let origin = [plane.origin.x as f64, plane.origin.y as f64, plane.origin.z as f64];
                        let u_axis = [plane.u_axis.x as f64, plane.u_axis.y as f64, plane.u_axis.z as f64];
                        let v_axis = [plane.v_axis.x as f64, plane.v_axis.y as f64, plane.v_axis.z as f64];
                        let normal = [plane.normal.x as f64, plane.normal.y as f64, plane.normal.z as f64];

                        if let Ok(shape) = ducad_kernel::extrude_profile_on_plane(
                            &profile, origin, u_axis, v_axis, normal, distance,
                        ) {
                            if let Ok(cloned_shape) = ducad_kernel::clone_shape(&shape) {
                                feature_shapes.insert(id, cloned_shape);
                            }

                            // Pasangkan ke BodyId
                            let target_body_id = if body_idx < existing_bodies.len() {
                                existing_bodies[body_idx]
                            } else {
                                self.model.doc.add_body(node.name.clone())
                            };
                            body_idx += 1;
                            body_map.insert(id, target_body_id);

                            let geo = BodyGeometry::from_shape(shape);
                            self.model.geometry.insert(target_body_id, geo);
                        }
                    }
                }
                FeaturePayload::Revolve { angle_deg, axis_origin, axis_dir, plane_index, .. } => {
                    let sketch = if plane_index < self.sketches.len() {
                        &self.sketches[plane_index]
                    } else {
                        &self.sketches[0]
                    };

                    let all_ids: std::collections::HashSet<_> = sketch
                        .entities
                        .iter()
                        .filter(|(_, e)| !e.is_construction())
                        .map(|(eid, _)| eid)
                        .collect();

                    let angle_opt = if (angle_deg - 360.0).abs() < 1e-4 { None } else { Some(angle_deg) };

                    if let Ok(profile) = crate::model::build_profile_from_selection(sketch, &all_ids) {
                        if let Ok(shape) = ducad_kernel::revolve_profile(&profile, axis_origin, axis_dir, angle_opt) {
                            if let Ok(cloned_shape) = ducad_kernel::clone_shape(&shape) {
                                feature_shapes.insert(id, cloned_shape);
                            }

                            let target_body_id = if body_idx < existing_bodies.len() {
                                existing_bodies[body_idx]
                            } else {
                                self.model.doc.add_body(node.name.clone())
                            };
                            body_idx += 1;
                            body_map.insert(id, target_body_id);

                            let geo = BodyGeometry::from_shape(shape);
                            self.model.geometry.insert(target_body_id, geo);
                        }
                    }
                }
                FeaturePayload::Fillet { target_feature_id, radius, .. } => {
                    if let Some(parent_shape) = feature_shapes.get(&target_feature_id) {
                        if let Ok(res_shape) = ducad_kernel::fillet_all(parent_shape, radius) {
                            if let Ok(cloned_res) = ducad_kernel::clone_shape(&res_shape) {
                                feature_shapes.insert(id, cloned_res);
                            }
                            if let Some(&b_id) = body_map.get(&target_feature_id) {
                                let geo = BodyGeometry::from_shape(res_shape);
                                self.model.geometry.insert(b_id, geo);
                            }
                        }
                    }
                }
                FeaturePayload::Chamfer { target_feature_id, distance } => {
                    if let Some(parent_shape) = feature_shapes.get(&target_feature_id) {
                        if let Ok(res_shape) = ducad_kernel::chamfer_all(parent_shape, distance) {
                            if let Ok(cloned_res) = ducad_kernel::clone_shape(&res_shape) {
                                feature_shapes.insert(id, cloned_res);
                            }
                            if let Some(&b_id) = body_map.get(&target_feature_id) {
                                let geo = BodyGeometry::from_shape(res_shape);
                                self.model.geometry.insert(b_id, geo);
                            }
                        }
                    }
                }
                FeaturePayload::Shell { target_feature_id, thickness } => {
                    if let Some(parent_shape) = feature_shapes.get(&target_feature_id) {
                        if let Ok(res_shape) = ducad_kernel::shell_hollow(parent_shape, thickness, ducad_kernel::Direction::PosZ) {
                            if let Ok(cloned_res) = ducad_kernel::clone_shape(&res_shape) {
                                feature_shapes.insert(id, cloned_res);
                            }
                            if let Some(&b_id) = body_map.get(&target_feature_id) {
                                let geo = BodyGeometry::from_shape(res_shape);
                                self.model.geometry.insert(b_id, geo);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Jika dokumen sudah memiliki bodi 3D namun di DAG belum tercatat Extrude/Revolve,
        // rekonstruksi bodi 3D yang ada langsung dari sketsa bidang aktif:
        let has_solid_features = self.parametric_dag.nodes.iter().any(|n| {
            matches!(
                n.payload,
                FeaturePayload::Extrude { .. }
                    | FeaturePayload::Revolve { .. }
                    | FeaturePayload::Fillet { .. }
                    | FeaturePayload::Chamfer { .. }
                    | FeaturePayload::Shell { .. }
            )
        });

        if !has_solid_features && !existing_bodies.is_empty() {
            let active_idx = self.active_plane_index();
            let sketch = if active_idx < self.sketches.len() {
                &self.sketches[active_idx]
            } else {
                &self.sketches[0]
            };

            let all_ids: std::collections::HashSet<_> = sketch
                .entities
                .iter()
                .filter(|(_, e)| !e.is_construction())
                .map(|(eid, _)| eid)
                .collect();

            if let Ok(profile) = crate::model::build_profile_from_selection(sketch, &all_ids) {
                let plane = self.plane_for_index(active_idx);
                let origin = [plane.origin.x as f64, plane.origin.y as f64, plane.origin.z as f64];
                let u_axis = [plane.u_axis.x as f64, plane.u_axis.y as f64, plane.u_axis.z as f64];
                let v_axis = [plane.v_axis.x as f64, plane.v_axis.y as f64, plane.v_axis.z as f64];
                let normal = [plane.normal.x as f64, plane.normal.y as f64, plane.normal.z as f64];

                if let Ok(shape) = ducad_kernel::extrude_profile_on_plane(
                    &profile, origin, u_axis, v_axis, normal, 25.0,
                ) {
                    let target_body_id = existing_bodies[0];
                    let geo = BodyGeometry::from_shape(shape);
                    self.model.geometry.insert(target_body_id, geo);
                }
            }
        }

        self.parametric_dag.mark_all_valid();
        self.model.doc.dirty = true;
        self.model_status = Some("Model parametrik berhasil diregenerasi".to_string());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;

    #[test]
    fn test_parametric_dag_recording_and_regeneration() {
        let mut app = DuCADApp::new_for_test();

        // 1. Gambar sebuah Circle di Sketch Top Plane
        app.sketches[0].entities.insert(Entity::Circle {
            center: DVec2::ZERO,
            radius: 15.0,
            is_construction: false,
        });

        // 2. Catat sketch & extrude feature
        let f_sketch = app.record_sketch_feature(0, "Circle R15");
        let f_extrude = app.record_extrude_feature(25.0, false);

        assert_eq!(app.parametric_dag.nodes.len(), 2);

        // 3. Eksekusi regenerasi
        let res = app.regenerate_parametric_model();
        assert!(res.is_ok());
        assert_eq!(app.model.doc.bodies.len(), 1);

        // 4. Ubah parameter sketsa masa lalu menjadi R = 30.0 & Extrude menjadi 50.0
        let update_res = app.save_feature_params_and_regenerate(f_sketch, 30.0, None);
        assert!(update_res.is_ok());

        // Validasi bahwa entitas sketsa terupdate menjadi radius 30.0
        let circle_r = match app.sketches[0].entities.iter().next().unwrap().1 {
            Entity::Circle { radius, .. } => *radius,
            _ => 0.0,
        };
        assert_eq!(circle_r, 30.0);

        // Update extrude depth
        let extrude_res = app.save_feature_params_and_regenerate(f_extrude, 50.0, None);
        assert!(extrude_res.is_ok());
        assert_eq!(app.model.doc.bodies.len(), 1);
    }

    #[test]
    fn test_parametric_rectangle_width_height_regeneration() {
        let mut app = DuCADApp::new_for_test();

        // Gambar 4 garis persegi 40x20
        app.sketches[0].entities.insert(Entity::Line {
            start: DVec2::new(0.0, 0.0),
            end: DVec2::new(40.0, 0.0),
            is_construction: false,
        });
        app.sketches[0].entities.insert(Entity::Line {
            start: DVec2::new(40.0, 0.0),
            end: DVec2::new(40.0, 20.0),
            is_construction: false,
        });
        app.sketches[0].entities.insert(Entity::Line {
            start: DVec2::new(40.0, 20.0),
            end: DVec2::new(0.0, 20.0),
            is_construction: false,
        });
        app.sketches[0].entities.insert(Entity::Line {
            start: DVec2::new(0.0, 20.0),
            end: DVec2::new(0.0, 0.0),
            is_construction: false,
        });

        let f_sketch = app.record_sketch_feature(0, "Rectangle 40x20");
        let _f_extrude = app.record_extrude_feature(10.0, false);

        assert_eq!(app.parametric_dag.nodes.len(), 2);
        assert!(app.regenerate_parametric_model().is_ok());

        // Update sketch parameter menjadi 80x60 (Panjang 80, Lebar 60)
        let update_res = app.save_feature_params_and_regenerate(f_sketch, 80.0, Some(60.0));
        assert!(update_res.is_ok());

        // Periksa bounding box sketch baru
        let (min, max) = app.sketches[0].bounding_box().unwrap();
        let size = max - min;
        assert!((size.x - 80.0).abs() < 1e-3);
        assert!((size.y - 60.0).abs() < 1e-3);
    }
}

