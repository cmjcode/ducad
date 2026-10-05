//! Engine Regenerasi & Perekaman Riwayat Parametrik (Feature DAG).
//!
//! Mengatur evaluasi topologis, pembaharuan geometri solid body 3D secara otomatis
//! saat dimensi sketsa atau parameter fitur masa lalu diedit.

use ducad_core::parametric::{FeatureId, FeaturePayload, FeatureStatus, SketchPlaneRef};
use ducad_core::BodyId;
use ducad_engine::compute::{self, ProfilePick};
use ducad_engine::{resolve_material, MaterialSel};
use ducad_kernel::{ExtrudeExtent, KernelShape};
use ducad_sketch::Entity;
use std::collections::HashMap;

use crate::app::DuCADApp;
use crate::model::BodyGeometry;

impl DuCADApp {
    /// Catat langkah sketsa 2D baru ke dalam DAG.
    pub fn record_sketch_feature(
        &mut self,
        plane_idx: usize,
        description: impl Into<String>,
    ) -> FeatureId {
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
            let datum_id = self
                .datum_planes
                .get(custom_idx)
                .map(|dp| dp.id)
                .unwrap_or(0);
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

        let sketch = self.sketch_at_index(plane_idx);

        let entity_count = sketch.entities.len();
        let (shape_type, dim_w, dim_h) = if entity_count == 1 {
            let ent = sketch.entities.iter().next().map(|(_, e)| e);
            match ent {
                Some(Entity::Circle { radius, .. }) => ("Lingkaran".to_string(), *radius, None),
                Some(Entity::Arc { radius, .. }) => ("Busur".to_string(), *radius, None),
                Some(Entity::Ellipse {
                    radius_x, radius_y, ..
                }) => ("Elips".to_string(), *radius_x, Some(*radius_y)),
                Some(Entity::Line { start, end, .. }) => {
                    ("Garis".to_string(), (*end - *start).length(), None)
                }
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

    /// Catat langkah Extrude ke dalam DAG dengan sumber entitas spesifik dan opsi warna dari style.
    pub fn record_extrude_feature_with_sources(
        &mut self,
        distance: f64,
        is_cut: bool,
        source_entities: Vec<String>,
        material_from_style: bool,
    ) -> FeatureId {
        let plane_idx = self.active_plane_index();
        // Cari sketch feature terakhir di plane ini atau buat baru
        let sketch_id = self
            .parametric_dag
            .nodes
            .iter()
            .rev()
            .find_map(|n| {
                if let FeaturePayload::Sketch { plane_index, .. } = n.payload {
                    if plane_index == plane_idx {
                        return Some(n.id);
                    }
                }
                None
            })
            .unwrap_or_else(|| self.record_sketch_feature(plane_idx, "Sketch"));

        let name = if is_cut {
            format!("Cut Extrude {}", self.parametric_dag.nodes.len() + 1)
        } else {
            format!("Extrude Boss {}", self.parametric_dag.nodes.len() + 1)
        };

        let feature_id = self.parametric_dag.add_feature(
            name,
            FeaturePayload::Extrude {
                sketch_id,
                distance,
                plane_index: plane_idx,
                is_cut,
                source_entities: source_entities.clone(),
                material_from_style,
            },
            vec![sketch_id],
        );

        // Rekam revisi awal untuk entitas sumber
        let sk = self.sketch_at_index(plane_idx);
        let mut rev_map = HashMap::new();
        if !source_entities.is_empty() {
            for (eid, _) in &sk.entities {
                if let Some(name) = sk.entity_names.get(&eid) {
                    if source_entities.contains(name) {
                        let r = sk.rev.get(eid).copied().unwrap_or(0);
                        rev_map.insert(eid, r);
                    }
                }
            }
        }
        self.feature_source_revs.insert(feature_id, rev_map);

        feature_id
    }

    /// Catat langkah Extrude ke dalam DAG.
    pub fn record_extrude_feature(&mut self, distance: f64, is_cut: bool) -> FeatureId {
        self.record_extrude_feature_with_sources(distance, is_cut, Vec::new(), false)
    }

    /// Periksa apakah ada entitas sumber sketsa yang berubah (stale) untuk fitur parametrik.
    pub fn check_stale_features(&mut self) {
        let total_global_rev: u64 = self
            .sketch_set
            .iter()
            .map(|(_, slot)| slot.sketch.global_rev)
            .fold(0u64, |acc, r| acc.wrapping_add(r));

        if total_global_rev == self.last_checked_global_rev {
            return;
        }
        self.last_checked_global_rev = total_global_rev;
        self.check_stale_features_force();
    }

    /// Periksa status stale untuk semua fitur berbasis entitas sumber tanpa membandingkan global_rev.
    pub fn check_stale_features_force(&mut self) {
        let mut stale_node_ids = Vec::new();
        for node in &self.parametric_dag.nodes {
            if let FeaturePayload::Extrude {
                plane_index,
                ref source_entities,
                ..
            } = node.payload
            {
                if source_entities.is_empty() || node.is_suppressed {
                    continue;
                }
                let sk = self.sketch_at_index(plane_index);
                let recorded_revs = self.feature_source_revs.get(&node.id);

                let mut is_stale = false;
                if let Some(rev_map) = recorded_revs {
                    let mut matching_count = 0;
                    for (eid, _) in &sk.entities {
                        if let Some(name) = sk.entity_names.get(&eid) {
                            if source_entities.contains(name) {
                                matching_count += 1;
                                let curr_rev = sk.rev.get(eid).copied().unwrap_or(0);
                                if rev_map.get(&eid).copied() != Some(curr_rev) {
                                    is_stale = true;
                                    break;
                                }
                            }
                        }
                    }
                    if matching_count != rev_map.len() {
                        is_stale = true;
                    }
                } else {
                    is_stale = true;
                }

                if is_stale {
                    stale_node_ids.push(node.id);
                }
            }
        }

        for node_id in stale_node_ids {
            if let Some(node) = self.parametric_dag.get_feature_mut(node_id) {
                node.status = FeatureStatus::Stale;
            }
        }
    }

    /// Catat langkah Revolve ke dalam DAG.
    pub fn record_revolve_feature(
        &mut self,
        angle_deg: f64,
        axis_origin: (f64, f64),
        axis_dir: (f64, f64),
    ) -> FeatureId {
        let plane_idx = self.active_plane_index();
        let sketch_id = self
            .parametric_dag
            .nodes
            .iter()
            .rev()
            .find_map(|n| {
                if let FeaturePayload::Sketch { plane_index, .. } = n.payload {
                    if plane_index == plane_idx {
                        return Some(n.id);
                    }
                }
                None
            })
            .unwrap_or_else(|| self.record_sketch_feature(plane_idx, "Sketch"));

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
        let parent_id = self
            .parametric_dag
            .nodes
            .iter()
            .rev()
            .find_map(|n| match n.payload {
                FeaturePayload::Extrude { .. }
                | FeaturePayload::Revolve { .. }
                | FeaturePayload::Fillet { .. }
                | FeaturePayload::Chamfer { .. }
                | FeaturePayload::Shell { .. } => Some(n.id),
                _ => None,
            })
            .unwrap_or(0);

        let name = format!("Fillet {}", self.parametric_dag.nodes.len() + 1);
        self.parametric_dag.add_feature(
            name,
            FeaturePayload::Fillet {
                target_feature_id: parent_id,
                radius,
                radius_end,
            },
            if parent_id > 0 {
                vec![parent_id]
            } else {
                vec![]
            },
        )
    }

    /// Catat langkah Shell ke dalam DAG, lengkap dengan sisi yang dibuka dan
    /// kedalaman rongga supaya regenerasi membuka sisi yang sama (bukan
    /// selalu sisi atas).
    pub fn record_shell_feature(
        &mut self,
        thickness: f64,
        open_rays: &[ducad_kernel::PickRay],
        open_direction: Option<ducad_kernel::Direction>,
        depth: f64,
    ) -> FeatureId {
        let parent_id = self
            .parametric_dag
            .nodes
            .iter()
            .rev()
            .find_map(|n| match n.payload {
                FeaturePayload::Extrude { .. }
                | FeaturePayload::Revolve { .. }
                | FeaturePayload::Fillet { .. }
                | FeaturePayload::Chamfer { .. }
                | FeaturePayload::Shell { .. } => Some(n.id),
                _ => None,
            })
            .unwrap_or(0);

        let name = format!("Shell {}", self.parametric_dag.nodes.len() + 1);
        self.parametric_dag.add_feature(
            name,
            FeaturePayload::Shell {
                target_feature_id: parent_id,
                thickness,
                open_rays: open_rays
                    .iter()
                    .map(|r| {
                        (
                            [r.origin.0, r.origin.1, r.origin.2],
                            [r.dir.0, r.dir.1, r.dir.2],
                        )
                    })
                    .collect(),
                open_direction: open_direction.map(|d| shell_direction_label(d).to_string()),
                depth,
            },
            if parent_id > 0 {
                vec![parent_id]
            } else {
                vec![]
            },
        )
    }

    /// Catat langkah Chamfer ke dalam DAG.
    pub fn record_chamfer_feature(&mut self, distance: f64) -> FeatureId {
        let parent_id = self
            .parametric_dag
            .nodes
            .iter()
            .rev()
            .find_map(|n| match n.payload {
                FeaturePayload::Extrude { .. }
                | FeaturePayload::Revolve { .. }
                | FeaturePayload::Fillet { .. }
                | FeaturePayload::Chamfer { .. }
                | FeaturePayload::Shell { .. } => Some(n.id),
                _ => None,
            })
            .unwrap_or(0);

        let name = format!("Chamfer {}", self.parametric_dag.nodes.len() + 1);
        self.parametric_dag.add_feature(
            name,
            FeaturePayload::Chamfer {
                target_feature_id: parent_id,
                distance,
            },
            if parent_id > 0 {
                vec![parent_id]
            } else {
                vec![]
            },
        )
    }

    /// Catat langkah Hole Wizard ke dalam DAG.
    pub fn record_hole_feature(
        &mut self,
        spec: ducad_core::hole::HoleSpec,
        pos: (f64, f64, f64),
        normal: (f64, f64, f64),
    ) -> FeatureId {
        let parent_id = self
            .parametric_dag
            .nodes
            .iter()
            .rev()
            .find_map(|n| match n.payload {
                FeaturePayload::Extrude { .. }
                | FeaturePayload::Revolve { .. }
                | FeaturePayload::Fillet { .. }
                | FeaturePayload::Chamfer { .. } => Some(n.id),
                _ => None,
            })
            .unwrap_or(0);

        let name = format!("Hole {}", self.parametric_dag.nodes.len() + 1);
        self.parametric_dag.add_feature(
            name,
            FeaturePayload::Hole {
                target_feature_id: parent_id,
                spec,
                pos,
                normal,
            },
            if parent_id > 0 {
                vec![parent_id]
            } else {
                vec![]
            },
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
            FeaturePayload::Extrude {
                sketch_id,
                plane_index,
                is_cut,
                source_entities,
                material_from_style,
                ..
            } => FeaturePayload::Extrude {
                sketch_id,
                distance: val1,
                plane_index,
                is_cut,
                source_entities,
                material_from_style,
            },
            FeaturePayload::Revolve {
                sketch_id,
                axis_origin,
                axis_dir,
                plane_index,
                ..
            } => FeaturePayload::Revolve {
                sketch_id,
                angle_deg: val1,
                axis_origin,
                axis_dir,
                plane_index,
            },
            FeaturePayload::Fillet {
                target_feature_id, ..
            } => FeaturePayload::Fillet {
                target_feature_id,
                radius: val1,
                radius_end: val2,
            },
            FeaturePayload::Chamfer {
                target_feature_id, ..
            } => FeaturePayload::Chamfer {
                target_feature_id,
                distance: val1,
            },
            FeaturePayload::Shell {
                target_feature_id,
                open_rays,
                open_direction,
                depth,
                ..
            } => FeaturePayload::Shell {
                target_feature_id,
                thickness: val1,
                open_rays,
                open_direction,
                depth,
            },
            FeaturePayload::Sketch {
                plane_ref,
                plane_index,
                entity_count,
                shape_type,
                description,
                ..
            } => {
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
            FeaturePayload::DatumPlane {
                datum_id,
                mode_desc,
                ..
            } => FeaturePayload::DatumPlane {
                datum_id,
                offset: val1,
                angle: val2.unwrap_or(0.0),
                mode_desc,
            },
            FeaturePayload::Helix {
                wire_radius, turns, ..
            } => FeaturePayload::Helix {
                radius: val1,
                pitch: val2.unwrap_or(10.0),
                turns,
                wire_radius,
            },
            other => other,
        };

        self.parametric_dag.update_feature_payload(id, new_payload);
        self.regenerate_parametric_model()
    }

    /// Helper untuk mengubah ukuran entitas sketsa saat parameter dimensi di Feature Tree diedit.
    ///
    /// Perubahan dijalankan sebagai `UpdateEntity` dalam satu transaksi di
    /// tumpukan undo sketsa itu: bisa di-undo, dan `touch` menaikkan `rev`
    /// sehingga deteksi fitur basi ikut melihatnya.
    fn apply_sketch_dimension_update(
        &mut self,
        plane_index: usize,
        new_w: f64,
        new_h: Option<f64>,
    ) {
        if new_w <= 0.0 {
            return;
        }
        let plane = self.plane_ref_for_index(plane_index);
        let Some(sketch_id) = self.sketch_set.first_on_plane(plane) else {
            return;
        };
        let Some(sketch) = self.sketch_set.sketch(sketch_id) else {
            return;
        };
        let updates = dimension_updates(sketch, new_w, new_h);
        if updates.is_empty() {
            return;
        }
        let Some(slot) = self.sketch_set.get_mut(sketch_id) else {
            return;
        };
        slot.undo.begin("Ubah Dimensi");
        for (id, entity) in updates {
            slot.undo.execute(
                Box::new(ducad_sketch::commands::UpdateEntity::new(
                    "Ubah Dimensi",
                    id,
                    entity,
                )),
                &mut slot.sketch,
            );
        }
        slot.undo.commit();
        self.clear_redo_except(crate::types::UndoTarget::Sketch);
    }

    /// Eksekusi seluruh Feature Tree DAG secara topologis dan rekonstruksi bodi solid 3D.
    ///
    /// Fitur yang gagal TIDAK menghentikan regenerasi: fitur independen tetap
    /// dievaluasi, turunan fitur gagal ditandai error karena dependensinya, dan
    /// status diset per fitur (tidak ada "tandai semua valid"). Mengembalikan
    /// `Err` berisi seluruh pesan bila ada fitur yang gagal.
    pub fn regenerate_parametric_model(&mut self) -> Result<(), String> {
        let order = self.parametric_dag.topological_order()?;
        let mut ctx = RegenCtx {
            existing_bodies: self.model.doc.bodies.iter().map(|(id, _)| id).collect(),
            ..RegenCtx::default()
        };
        let mut failed: std::collections::HashSet<FeatureId> = std::collections::HashSet::new();
        let mut errors: Vec<String> = Vec::new();

        for id in order {
            let Some(node) = self.parametric_dag.get_feature(id).cloned() else {
                continue;
            };
            if node.is_suppressed {
                continue;
            }

            let outcome = match node.dependencies.iter().find(|d| failed.contains(d)) {
                Some(dep) => FeatureRegen::Failed(format!("fitur induk #{dep} gagal")),
                None => self.regenerate_feature(id, &node, &mut ctx),
            };
            let status = match outcome {
                FeatureRegen::Built => FeatureStatus::Valid,
                FeatureRegen::Failed(msg) => {
                    failed.insert(id);
                    errors.push(format!("{}: {msg}", node.name));
                    FeatureStatus::Error(msg)
                }
                FeatureRegen::Unsupported => {
                    failed.insert(id);
                    let msg =
                        "regenerasi fitur ini belum didukung — geometrinya tidak diterapkan ulang"
                            .to_string();
                    errors.push(format!("{}: {msg}", node.name));
                    FeatureStatus::Error(msg)
                }
            };
            if let Some(n) = self.parametric_dag.get_feature_mut(id) {
                n.status = status;
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

        if !has_solid_features && !ctx.existing_bodies.is_empty() {
            let active_idx = self.active_plane_index();
            let sketch = self.sketch_at_index(active_idx);

            let all_ids: std::collections::HashSet<_> = sketch
                .entities
                .iter()
                .filter(|(_, e)| !e.is_construction())
                .map(|(eid, _)| eid)
                .collect();

            if let Ok(profile) = crate::model::build_profile_from_selection(sketch, &all_ids) {
                let plane = self.plane_for_index(active_idx);
                let origin = [
                    plane.origin.x as f64,
                    plane.origin.y as f64,
                    plane.origin.z as f64,
                ];
                let u_axis = [
                    plane.u_axis.x as f64,
                    plane.u_axis.y as f64,
                    plane.u_axis.z as f64,
                ];
                let v_axis = [
                    plane.v_axis.x as f64,
                    plane.v_axis.y as f64,
                    plane.v_axis.z as f64,
                ];
                let normal = [
                    plane.normal.x as f64,
                    plane.normal.y as f64,
                    plane.normal.z as f64,
                ];

                if let Ok(shape) = ducad_kernel::extrude_profile_on_plane(
                    &profile, origin, u_axis, v_axis, normal, 25.0,
                ) {
                    let target_body_id = ctx.existing_bodies[0];
                    let geo = BodyGeometry::from_shape(shape);
                    self.model.geometry.insert(target_body_id, geo);
                }
            }
        }

        self.model.doc.dirty = true;
        if errors.is_empty() {
            self.model_status = Some("Model parametrik berhasil diregenerasi".to_string());
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }

    /// Evaluasi satu fitur. Pemilihan per varian payload; tiap varian punya
    /// fungsinya sendiri.
    fn regenerate_feature(
        &mut self,
        id: FeatureId,
        node: &ducad_core::parametric::FeatureNode,
        ctx: &mut RegenCtx,
    ) -> FeatureRegen {
        match &node.payload {
            FeaturePayload::DatumPlane {
                datum_id, offset, ..
            } => {
                if let Some(dp) = self.datum_planes.iter_mut().find(|dp| dp.id == *datum_id) {
                    dp.plane = dp.plane.offset(*offset as f32);
                }
                FeatureRegen::Built
            }
            // Sketsa dievaluasi lewat fitur yang memakainya.
            FeaturePayload::Sketch { .. } => FeatureRegen::Built,
            FeaturePayload::Extrude {
                distance,
                plane_index,
                source_entities,
                material_from_style,
                ..
            } => self.regen_extrude(
                id,
                &node.name,
                *distance,
                *plane_index,
                source_entities,
                *material_from_style,
                ctx,
            ),
            FeaturePayload::Revolve {
                angle_deg,
                axis_origin,
                axis_dir,
                plane_index,
                ..
            } => self.regen_revolve(
                id,
                &node.name,
                *angle_deg,
                *axis_origin,
                *axis_dir,
                *plane_index,
                ctx,
            ),
            FeaturePayload::Fillet {
                target_feature_id,
                radius,
                ..
            } => {
                let r = *radius;
                self.regen_finishing(id, *target_feature_id, ctx, |s| {
                    ducad_kernel::fillet_all(s, r)
                })
            }
            FeaturePayload::Chamfer {
                target_feature_id,
                distance,
            } => {
                let d = *distance;
                self.regen_finishing(id, *target_feature_id, ctx, |s| {
                    ducad_kernel::chamfer_all(s, d)
                })
            }
            FeaturePayload::Shell {
                target_feature_id,
                thickness,
                open_rays,
                open_direction,
                depth,
            } => {
                let (t, d) = (*thickness, *depth);
                let rays: Vec<ducad_kernel::PickRay> = open_rays
                    .iter()
                    .map(|(o, dir)| ducad_kernel::PickRay {
                        origin: (o[0], o[1], o[2]),
                        dir: (dir[0], dir[1], dir[2]),
                    })
                    .collect();
                let dir = shell_direction_from_label(open_direction.as_deref());
                self.regen_finishing(id, *target_feature_id, ctx, |s| {
                    let opening = if rays.is_empty() {
                        ducad_kernel::ShellOpening::Farthest(dir)
                    } else {
                        ducad_kernel::ShellOpening::Rays(&rays)
                    };
                    ducad_kernel::shell_open(s, t, opening, d)
                })
            }
            FeaturePayload::Hole { .. }
            | FeaturePayload::Helix { .. }
            | FeaturePayload::Boolean { .. }
            | FeaturePayload::Custom { .. } => FeatureRegen::Unsupported,
        }
    }

    /// Body tujuan fitur pembentuk solid ke-n: pakai ulang body yang ada
    /// sesuai urutan, atau buat baru.
    fn next_regen_body(&mut self, name: &str, ctx: &mut RegenCtx) -> BodyId {
        let body = match ctx.existing_bodies.get(ctx.body_idx) {
            Some(&b) => b,
            None => self.model.doc.add_body(name.to_string()),
        };
        ctx.body_idx += 1;
        body
    }

    #[allow(clippy::too_many_arguments)]
    fn regen_extrude(
        &mut self,
        id: FeatureId,
        name: &str,
        distance: f64,
        plane_index: usize,
        source_entities: &[String],
        material_from_style: bool,
        ctx: &mut RegenCtx,
    ) -> FeatureRegen {
        let plane = self.plane_for_index(plane_index);
        let sketch = self.sketch_at_index(plane_index);
        let target_ids: std::collections::HashSet<_> = sketch
            .entities
            .iter()
            .filter(|(eid, e)| {
                !e.is_construction()
                    && (source_entities.is_empty()
                        || sketch
                            .entity_names
                            .get(eid)
                            .is_some_and(|name| source_entities.contains(name)))
            })
            .map(|(eid, _)| eid)
            .collect();
        if !source_entities.is_empty() && target_ids.is_empty() {
            return FeatureRegen::Failed("Entitas sumber tidak ditemukan dalam sketsa".to_string());
        }

        let plane_frame = crate::document::plane_frame_from(&plane);
        let pick = ProfilePick::Entities(&target_ids);
        let solids =
            match compute::extrude(sketch, &pick, &plane_frame, ExtrudeExtent::Blind(distance)) {
                Ok(s) => s,
                Err(e) => return FeatureRegen::Failed(format!("Ekstrusi gagal: {}", e.message)),
            };
        let style = material_from_style
            .then(|| ducad_engine::compute::solid::selection_style(sketch, &target_ids));
        let rev_map: HashMap<_, _> = target_ids
            .iter()
            .map(|eid| (*eid, sketch.rev.get(*eid).copied().unwrap_or(0)))
            .collect();

        let Some((_, geo)) = solids.into_iter().next() else {
            return FeatureRegen::Failed(
                "Tidak ada geometri yang dihasilkan dari ekstrusi".to_string(),
            );
        };
        let body = self.next_regen_body(name, ctx);
        ctx.body_map.insert(id, body);
        if let Ok(shape) = ducad_kernel::clone_shape(&geo.shape) {
            ctx.feature_shapes.insert(id, shape);
        }
        self.model.geometry.insert(body, geo);
        if let Some(style) = style {
            let mat = resolve_material(&MaterialSel::FromStyle, &style);
            if let Some(meta) = self.model.doc.bodies.get_mut(body) {
                meta.material = mat;
            }
        }
        self.feature_source_revs.insert(id, rev_map);
        FeatureRegen::Built
    }

    #[allow(clippy::too_many_arguments)]
    fn regen_revolve(
        &mut self,
        id: FeatureId,
        name: &str,
        angle_deg: f64,
        axis_origin: (f64, f64),
        axis_dir: (f64, f64),
        plane_index: usize,
        ctx: &mut RegenCtx,
    ) -> FeatureRegen {
        let sketch = self.sketch_at_index(plane_index);
        let all_ids: std::collections::HashSet<_> = sketch
            .entities
            .iter()
            .filter(|(_, e)| !e.is_construction())
            .map(|(eid, _)| eid)
            .collect();
        let angle = ((angle_deg - 360.0).abs() >= 1e-4).then_some(angle_deg);
        let profile = match crate::model::build_profile_from_selection(sketch, &all_ids) {
            Ok(p) => p,
            Err(e) => return FeatureRegen::Failed(format!("Profil revolve tidak valid: {e}")),
        };
        let shape = match ducad_kernel::revolve_profile(&profile, axis_origin, axis_dir, angle) {
            Ok(s) => s,
            Err(e) => return FeatureRegen::Failed(format!("Revolve gagal: {e}")),
        };
        if let Ok(cloned) = ducad_kernel::clone_shape(&shape) {
            ctx.feature_shapes.insert(id, cloned);
        }
        let body = self.next_regen_body(name, ctx);
        ctx.body_map.insert(id, body);
        self.model
            .geometry
            .insert(body, BodyGeometry::from_shape(shape));
        FeatureRegen::Built
    }

    /// Fillet/Chamfer/Shell: operasi atas bentuk fitur induk, hasilnya
    /// menggantikan geometri body induk.
    fn regen_finishing<E: std::fmt::Display>(
        &mut self,
        id: FeatureId,
        parent: FeatureId,
        ctx: &mut RegenCtx,
        op: impl FnOnce(&KernelShape) -> Result<KernelShape, E>,
    ) -> FeatureRegen {
        let Some(parent_shape) = ctx.feature_shapes.get(&parent) else {
            return FeatureRegen::Failed(format!("bentuk fitur induk #{parent} tidak tersedia"));
        };
        let shape = match op(parent_shape) {
            Ok(s) => s,
            Err(e) => return FeatureRegen::Failed(format!("Operasi kernel gagal: {e}")),
        };
        if let Ok(cloned) = ducad_kernel::clone_shape(&shape) {
            ctx.feature_shapes.insert(id, cloned);
        }
        if let Some(&body) = ctx.body_map.get(&parent) {
            ctx.body_map.insert(id, body);
            self.model
                .geometry
                .insert(body, BodyGeometry::from_shape(shape));
        }
        FeatureRegen::Built
    }
}

/// Label tersimpan untuk arah sisi shell yang dibuka (`"+X"` … `"-Z"`).
pub(crate) fn shell_direction_label(dir: ducad_kernel::Direction) -> &'static str {
    use ducad_kernel::Direction;
    match dir {
        Direction::PosX => "+X",
        Direction::NegX => "-X",
        Direction::PosY => "+Y",
        Direction::NegY => "-Y",
        Direction::PosZ => "+Z",
        Direction::NegZ => "-Z",
    }
}

/// Kebalikan `shell_direction_label`; label tak dikenal / kosong (berkas
/// lama) = sisi atas, perilaku sebelum arah disimpan.
pub(crate) fn shell_direction_from_label(label: Option<&str>) -> ducad_kernel::Direction {
    use ducad_kernel::Direction;
    match label {
        Some("+X") => Direction::PosX,
        Some("-X") => Direction::NegX,
        Some("+Y") => Direction::PosY,
        Some("-Y") => Direction::NegY,
        Some("-Z") => Direction::NegZ,
        _ => Direction::PosZ,
    }
}

/// Hasil regenerasi satu fitur.
enum FeatureRegen {
    Built,
    Failed(String),
    /// Varian yang belum punya jalur regenerasi (Hole, Helix, Boolean, Custom).
    Unsupported,
}

/// State bersama selama satu regenerasi.
#[derive(Default)]
struct RegenCtx {
    feature_shapes: HashMap<FeatureId, KernelShape>,
    body_map: HashMap<FeatureId, BodyId>,
    existing_bodies: Vec<BodyId>,
    body_idx: usize,
}

/// Entitas `sketch` setelah diskalakan ke dimensi baru (lebar `new_w`,
/// tinggi opsional `new_h`). Hanya entitas yang benar-benar berubah yang
/// dikembalikan, terurut sesuai iterasi slotmap (deterministik).
fn dimension_updates(
    sketch: &ducad_sketch::Sketch,
    new_w: f64,
    new_h: Option<f64>,
) -> Vec<(ducad_sketch::EntityId, Entity)> {
    if sketch.entities.is_empty() {
        return Vec::new();
    }

    if sketch.entities.len() == 1 {
        return sketch
            .entities
            .iter()
            .filter_map(|(id, ent)| {
                let mut ent = ent.clone();
                match &mut ent {
                    Entity::Circle { radius, .. } | Entity::Arc { radius, .. } => *radius = new_w,
                    Entity::Ellipse {
                        radius_x, radius_y, ..
                    } => {
                        *radius_x = new_w;
                        if let Some(h) = new_h {
                            *radius_y = h;
                        }
                    }
                    Entity::Line { start, end, .. } => {
                        let dir = (*end - *start).normalize_or_zero();
                        if dir.length_squared() == 0.0 {
                            return None;
                        }
                        *end = *start + dir * new_w;
                    }
                    _ => return None,
                }
                Some((id, ent))
            })
            .collect();
    }

    let Some((min, max)) = sketch.bounding_box() else {
        return Vec::new();
    };
    let size = max - min;
    let old_w = if size.x > 1e-4 { size.x } else { 1.0 };
    let old_h = if size.y > 1e-4 { size.y } else { 1.0 };
    let scale_x = new_w / old_w;
    let scale_y = new_h.map(|h| h / old_h).unwrap_or(scale_x);
    let center = (min + max) * 0.5;
    let scale_point = |p: glam::DVec2| -> glam::DVec2 {
        glam::DVec2::new(
            center.x + (p.x - center.x) * scale_x,
            center.y + (p.y - center.y) * scale_y,
        )
    };

    sketch
        .entities
        .iter()
        .map(|(id, ent)| {
            let mut ent = ent.clone();
            match &mut ent {
                Entity::Line { start, end, .. } => {
                    *start = scale_point(*start);
                    *end = scale_point(*end);
                }
                Entity::Circle {
                    center: c, radius, ..
                }
                | Entity::Arc {
                    center: c, radius, ..
                } => {
                    *c = scale_point(*c);
                    *radius *= scale_x.max(scale_y);
                }
                Entity::Ellipse {
                    center: c,
                    radius_x,
                    radius_y,
                    ..
                } => {
                    *c = scale_point(*c);
                    *radius_x *= scale_x;
                    *radius_y *= scale_y;
                }
                Entity::Spline { points, .. } => {
                    for pt in points.iter_mut() {
                        *pt = scale_point(*pt);
                    }
                }
                Entity::Path { subpaths, .. } => {
                    for sub in subpaths.iter_mut() {
                        *sub = sub.map_points(scale_point);
                    }
                }
            }
            (id, ent)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;

    #[test]
    fn test_parametric_dag_recording_and_regeneration() {
        let mut app = DuCADApp::new_for_test();

        // 1. Gambar sebuah Circle di Sketch Top Plane
        app.sketch_at_index_mut(0)
            .unwrap()
            .entities
            .insert(Entity::Circle {
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
        let circle_r = match app.sketch_at_index(0).entities.iter().next().unwrap().1 {
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
        app.sketch_at_index_mut(0)
            .unwrap()
            .entities
            .insert(Entity::Line {
                start: DVec2::new(0.0, 0.0),
                end: DVec2::new(40.0, 0.0),
                is_construction: false,
            });
        app.sketch_at_index_mut(0)
            .unwrap()
            .entities
            .insert(Entity::Line {
                start: DVec2::new(40.0, 0.0),
                end: DVec2::new(40.0, 20.0),
                is_construction: false,
            });
        app.sketch_at_index_mut(0)
            .unwrap()
            .entities
            .insert(Entity::Line {
                start: DVec2::new(40.0, 20.0),
                end: DVec2::new(0.0, 20.0),
                is_construction: false,
            });
        app.sketch_at_index_mut(0)
            .unwrap()
            .entities
            .insert(Entity::Line {
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
        let (min, max) = app.sketch_at_index(0).bounding_box().unwrap();
        let size = max - min;
        assert!((size.x - 80.0).abs() < 1e-3);
        assert!((size.y - 60.0).abs() < 1e-3);
    }

    /// Regresi REVIEW-2026-09-24 #6: edit dimensi di Feature Tree harus lewat
    /// command — bisa di-undo dan menaikkan `rev` sketsa.
    #[test]
    fn sketch_dimension_edit_is_undoable_and_bumps_rev() {
        let mut app = DuCADApp::new_for_test();
        app.sketch_at_index_mut(0)
            .unwrap()
            .entities
            .insert(Entity::Circle {
                center: DVec2::ZERO,
                radius: 15.0,
                is_construction: false,
            });
        let f_sketch = app.record_sketch_feature(0, "Circle R15");
        let rev_before = app.sketch_at_index(0).global_rev;

        assert!(app
            .save_feature_params_and_regenerate(f_sketch, 30.0, None)
            .is_ok());
        let radius = |app: &DuCADApp| match app.sketch_at_index(0).entities.iter().next() {
            Some((_, Entity::Circle { radius, .. })) => *radius,
            _ => f64::NAN,
        };
        assert_eq!(radius(&app), 30.0);
        assert!(
            app.sketch_at_index(0).global_rev > rev_before,
            "rev harus naik"
        );

        app.undo();
        assert_eq!(radius(&app), 15.0, "edit dimensi bisa di-undo");
    }
}
