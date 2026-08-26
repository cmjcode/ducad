use ducad_core::Command;
use ducad_kernel::{KernelMesh, KernelShape};
use ducad_render::{PlaneKind, SketchPlane};
use ducad_sketch::Sketch;

use crate::app::DuCADApp;
use crate::model::ModelDoc;
use crate::types::ToolKind;

impl DuCADApp {
    #[inline]
    pub fn static_plane_for_index(idx: usize) -> SketchPlane {
        match idx {
            0 => SketchPlane::top(),
            1 => SketchPlane::front(),
            2 => SketchPlane::right(),
            _ => SketchPlane::top(),
        }
    }

    #[inline]
    pub fn plane_for_index(&self, idx: usize) -> SketchPlane {
        match idx {
            0 => SketchPlane::top(),
            1 => SketchPlane::front(),
            2 => SketchPlane::right(),
            custom_idx if custom_idx >= 3 && custom_idx - 3 < self.datum_planes.len() => {
                self.datum_planes[custom_idx - 3].plane
            }
            _ => SketchPlane::top(),
        }
    }

    #[inline]
    pub fn active_plane_index(&self) -> usize {
        match self.active_plane.kind {
            PlaneKind::Top => 0,
            PlaneKind::Front => 1,
            PlaneKind::Right => 2,
            PlaneKind::Custom(id) => {
                self.datum_planes
                    .iter()
                    .position(|dp| dp.id == id)
                    .map(|pos| pos + 3)
                    .unwrap_or(0)
            }
        }
    }

    pub fn all_planes(&self) -> Vec<(usize, SketchPlane, String)> {
        let mut list = vec![
            (0, SketchPlane::top(), "Top Plane (XY)".to_string()),
            (1, SketchPlane::front(), "Front Plane (XZ)".to_string()),
            (2, SketchPlane::right(), "Right Plane (YZ)".to_string()),
        ];
        for (i, dp) in self.datum_planes.iter().enumerate() {
            list.push((i + 3, dp.plane, dp.name.clone()));
        }
        list
    }

    pub fn create_datum_plane(&mut self, name: String, mut plane: SketchPlane) -> u32 {
        self.datum_plane_counter += 1;
        let id = self.datum_plane_counter;
        plane.kind = PlaneKind::Custom(id);
        let datum_plane = ducad_render::plane::DatumPlane::new(id, name.clone(), plane);
        self.datum_planes.push(datum_plane);
        self.sketches.push(Sketch::default());
        self.undos.push(ducad_sketch::UndoStack::default());
        self.record_activity(
            ducad_ui::ActivityKindUi::Solid3D,
            "Buat Bidang Referensi (Datum Plane)",
            &format!("Membuat bidang referensi '{}'", name),
        );
        id
    }

    pub fn delete_datum_plane(&mut self, id: u32) {
        if let Some(pos) = self.datum_planes.iter().position(|dp| dp.id == id) {
            let idx = pos + 3;
            let is_active = match self.active_plane.kind {
                PlaneKind::Custom(active_id) => active_id == id,
                _ => false,
            };
            if is_active {
                self.set_sketch_plane(PlaneKind::Top);
            }
            let name = self.datum_planes[pos].name.clone();
            self.datum_planes.remove(pos);
            if idx < self.sketches.len() {
                self.sketches.remove(idx);
                self.undos.remove(idx);
            }
            self.record_activity(
                ducad_ui::ActivityKindUi::Solid3D,
                "Hapus Bidang Referensi",
                &format!("Menghapus bidang referensi '{}'", name),
            );
        }
    }

    pub fn set_sketch_plane_by_index(&mut self, idx: usize) {
        let plane = self.plane_for_index(idx);
        self.active_plane = plane;
        self.selected.clear();
        self.hovered = None;
        self.pending_points.clear();
        self.pending_point_refs.clear();
        self.offset_source = None;
        self.last_snap = None;
        self.is_sketching = true;
        self.left_toolbar.is_sketching = true;
        self.camera.orient_to_plane(&self.active_plane);
    }

    pub fn apply_create_datum_plane(&mut self) {
        let offset_val = self.datum_offset_input.trim().parse::<f64>().unwrap_or(20.0);
        let angle_val = self.datum_angle_input.trim().parse::<f64>().unwrap_or(45.0);

        let (name, plane) = match self.datum_mode {
            ducad_ui::DatumPlaneMode::Offset => {
                let dist = if self.datum_flip { -offset_val } else { offset_val };
                if let Some((_, _, hit)) = &self.active_face {
                    let origin = glam::vec3(hit.hit_point.0 as f32, hit.hit_point.1 as f32, hit.hit_point.2 as f32);
                    let norm = glam::vec3(hit.normal.0 as f32, hit.normal.1 as f32, hit.normal.2 as f32);
                    let plane = SketchPlane::from_face_offset(origin, norm, dist as f32);
                    let name = format!("Plane {} (Face {:+0.0}mm)", self.datum_plane_counter + 1, dist);
                    (name, plane)
                } else {
                    let base = self.plane_for_index(self.datum_base_plane_idx);
                    let plane = base.offset(dist as f32);
                    let name = format!("Plane {} (Offset {:+0.0}mm)", self.datum_plane_counter + 1, dist);
                    (name, plane)
                }
            }
            ducad_ui::DatumPlaneMode::Angled => {
                let ang = if self.datum_flip { -angle_val } else { angle_val };
                if let Some(edge) = self.selected_edges.first() {
                    let p1 = edge.polyline.first().map(|&(x, y, z)| glam::vec3(x as f32, y as f32, z as f32)).unwrap_or(glam::Vec3::ZERO);
                    let p2 = edge.polyline.last().map(|&(x, y, z)| glam::vec3(x as f32, y as f32, z as f32)).unwrap_or(glam::Vec3::new(50.0, 0.0, 0.0));
                    let ref_norm = glam::Vec3::Z;
                    let plane = SketchPlane::from_angle_and_edge(p1, p2, ref_norm, ang as f32);
                    let name = format!("Plane {} (Angled {:0.0}°)", self.datum_plane_counter + 1, ang);
                    (name, plane)
                } else {
                    let plane = SketchPlane::from_angle_and_edge(
                        glam::Vec3::ZERO,
                        glam::Vec3::new(50.0, 0.0, 0.0),
                        glam::Vec3::Z,
                        ang as f32,
                    );
                    let name = format!("Plane {} (Angled {:0.0}°)", self.datum_plane_counter + 1, ang);
                    (name, plane)
                }
            }
            ducad_ui::DatumPlaneMode::ThreePoints => {
                if self.datum_selected_points.len() >= 3 {
                    let p1 = self.datum_selected_points[0];
                    let p2 = self.datum_selected_points[1];
                    let p3 = self.datum_selected_points[2];
                    if let Some(plane) = SketchPlane::from_3_points(p1, p2, p3) {
                        let name = format!("Plane {} (3-Point)", self.datum_plane_counter + 1);
                        (name, plane)
                    } else {
                        self.model_status = Some("Gagal membuat bidang: 3 titik kolinear".to_string());
                        return;
                    }
                } else {
                    self.model_status = Some("Pilih 3 titik non-kolinear terlebih dahulu".to_string());
                    return;
                }
            }
        };

        let new_id = self.create_datum_plane(name.clone(), plane);
        self.set_sketch_plane(PlaneKind::Custom(new_id));
        self.datum_selected_points.clear();
        self.planes_drawer_open = true;
        self.model_status = Some(format!("Bidang referensi '{}' berhasil dibuat", name));
        self.set_tool(ToolKind::Select);
    }

    #[inline]
    pub fn sketch(&self) -> &Sketch {
        let idx = self.active_plane_index();
        if idx < self.sketches.len() {
            &self.sketches[idx]
        } else {
            &self.sketches[0]
        }
    }

    #[inline]
    #[allow(dead_code)]
    pub fn sketch_mut(&mut self) -> &mut Sketch {
        let idx = self.active_plane_index();
        if idx < self.sketches.len() {
            &mut self.sketches[idx]
        } else {
            &mut self.sketches[0]
        }
    }

    #[inline]
    pub fn execute_sketch_command(&mut self, cmd: Box<dyn Command<Sketch>>) {
        let name = cmd.name().to_string();
        let idx = self.active_plane_index();
        let plane_label = match self.active_plane.kind {
            PlaneKind::Custom(id) => self
                .datum_planes
                .iter()
                .find(|dp| dp.id == id)
                .map(|dp| dp.name.clone())
                .unwrap_or_else(|| self.active_plane.kind.display_label().to_string()),
            _ => self.active_plane.kind.display_label().to_string(),
        };
        if idx < self.sketches.len() {
            self.undos[idx].execute(cmd, &mut self.sketches[idx]);
        }

        let (action_title, detail_desc) = match name.as_str() {
            "Line" => ("Sketsa Garis 2D", format!("Menggambar segmen garis di Bidang {}", plane_label)),
            "Circle" => ("Sketsa Lingkaran 2D", format!("Menggambar lingkaran di Bidang {}", plane_label)),
            "Arc" => ("Sketsa Busur 2D", format!("Menggambar busur 3-titik di Bidang {}", plane_label)),
            "Rectangle" => ("Sketsa Persegi 2D", format!("Menggambar kotak/persegi di Bidang {}", plane_label)),
            "Ellipse" => ("Sketsa Elips 2D", format!("Menggambar elips di Bidang {}", plane_label)),
            "Trim" => ("Potong Garis (Trim)", format!("Memotong segmen garis di Bidang {}", plane_label)),
            "Offset" => ("Offset Garis / Kurva", format!("Menduplikasi garis sejajar di Bidang {}", plane_label)),
            "Mirror" => ("Cermin Sketsa (Mirror)", format!("Mencerminkan entitas sketsa di Bidang {}", plane_label)),
            "Delete" => ("Hapus Entitas Sketsa", format!("Menghapus elemen 2D di Bidang {}", plane_label)),
            "Move" => ("Geser Sketsa 2D", format!("Memindahkan posisi elemen di Bidang {}", plane_label)),
            _ => ("Aktivitas Sketsa 2D", format!("{} di Bidang {}", name, plane_label)),
        };

        self.record_activity(
            ducad_ui::ActivityKindUi::Sketch2D,
            action_title,
            &detail_desc,
        );
    }

    #[inline]
    pub fn execute_model_command(&mut self, cmd: Box<dyn Command<ModelDoc>>, details: &str) {
        let name = cmd.name().to_string();
        let action_title = match name.as_str() {
            "Extrude" => "Extrude Solid 3D",
            "Cut Extrude" => "Cut Extrude (Potong Solid)",
            "Extrude Face" => "Tarik Sisi Solid (Push-Pull)",
            "Revolve" => "Revolve Solid 3D",
            "Revolve Face" => "Putar Sisi Solid 3D",
            "Loft" => "Loft Solid 3D",
            "Fillet" => "Fillet Sudut Lengkung",
            "Chamfer" => "Chamfer Sudut Bevel",
            "Shell" => "Shell / Hollow Berongga",
            "Shell Face" => "Shell Berlubang Sisi",
            "Boolean Union" => "Boolean Gabung (Union)",
            "Boolean Subtract" => "Boolean Potong (Subtract)",
            "Boolean Intersect" => "Boolean Irisan (Intersect)",
            "Delete Body" => "Hapus Objek Solid 3D",
            "Translate Body" => "Geser Objek Solid 3D",
            "Rotate Body" => "Putar Objek Solid 3D",
            "Scale Body" => "Ubah Skala / Resize 3D",
            _ => &name,
        };

        self.model_undo.execute(cmd, &mut self.model);
        self.record_activity(
            ducad_ui::ActivityKindUi::Solid3D,
            action_title,
            details,
        );
    }

    #[inline]
    pub fn undo_active_sketch(&mut self) {
        let idx = self.active_plane_index();
        if idx < self.sketches.len() {
            self.undos[idx].undo(&mut self.sketches[idx]);
        }
    }

    #[inline]
    pub fn redo_active_sketch(&mut self) {
        let idx = self.active_plane_index();
        if idx < self.sketches.len() {
            self.undos[idx].redo(&mut self.sketches[idx]);
        }
    }

    #[inline]
    pub fn can_undo_active_sketch(&self) -> bool {
        let idx = self.active_plane_index();
        if idx < self.undos.len() {
            self.undos[idx].can_undo()
        } else {
            false
        }
    }

    #[inline]
    pub fn can_redo_active_sketch(&self) -> bool {
        let idx = self.active_plane_index();
        if idx < self.undos.len() {
            self.undos[idx].can_redo()
        } else {
            false
        }
    }

    #[inline]
    pub fn sketch_undo_count(&self) -> usize {
        let idx = self.active_plane_index();
        if idx < self.undos.len() {
            self.undos[idx].undo_count()
        } else {
            0
        }
    }

    #[inline]
    pub fn sketch_redo_count(&self) -> usize {
        let idx = self.active_plane_index();
        if idx < self.undos.len() {
            self.undos[idx].redo_count()
        } else {
            0
        }
    }

    /// Ubah bidang kerja sketsa aktif dan selaraskan kamera.
    pub fn set_sketch_plane(&mut self, kind: PlaneKind) {
        if self.active_plane.kind != kind {
            self.selected.clear();
            self.hovered = None;
            self.pending_points.clear();
            self.pending_point_refs.clear();
            self.offset_source = None;
            self.last_snap = None;
            self.active_plane = match kind {
                PlaneKind::Top => SketchPlane::top(),
                PlaneKind::Front => SketchPlane::front(),
                PlaneKind::Right => SketchPlane::right(),
                PlaneKind::Custom(id) => self
                    .datum_planes
                    .iter()
                    .find(|dp| dp.id == id)
                    .map(|dp| dp.plane)
                    .unwrap_or_else(SketchPlane::top),
            };
        }
        self.is_sketching = true;
        self.left_toolbar.is_sketching = true;
        self.camera.orient_to_plane(&self.active_plane);
    }

    /// Aktifkan `kind` sebagai bidang sketsa lewat gestur langsung di viewport 3D.
    pub fn activate_plane_from_viewport(&mut self, kind: PlaneKind) {
        self.set_sketch_plane(kind);
        self.model_status = Some(format!(
            "Bidang '{}' kini aktif untuk sketsa",
            kind.display_label()
        ));
    }

    pub fn new_document(&mut self) {
        self.sketches = vec![Sketch::default(), Sketch::default(), Sketch::default()];
        self.undos = vec![
            ducad_sketch::UndoStack::default(),
            ducad_sketch::UndoStack::default(),
            ducad_sketch::UndoStack::default(),
        ];
        self.datum_planes.clear();
        self.datum_plane_counter = 0;
        self.selected.clear();
        self.hovered = None;
        self.pending_points.clear();
        self.pending_point_refs.clear();
        self.offset_source = None;
        self.line_chain_start = None;
        self.line_chain_segments = 0;
        self.model = ModelDoc::default();
        self.model_undo = ducad_core::UndoStack::default();
        self.selected_bodies.clear();
        self.current_file_path = None;
        self.history_db.clear();
        self.activity_cache.clear();
        self.file_status = Some("Dokumen baru".to_string());
        self.measurements.clear();
        self.set_tool(ToolKind::Select);
    }

    pub fn native_body_refs(&self) -> Vec<(&str, bool, ducad_core::Material, &KernelShape)> {
        self.model
            .doc
            .bodies
            .iter()
            .map(|(id, meta)| {
                (
                    meta.name.as_str(),
                    meta.visible,
                    meta.material,
                    &self
                        .model
                        .geometry
                        .get(id)
                        .expect("body hilang dari storage")
                        .shape,
                )
            })
            .collect()
    }

    pub fn all_body_shapes(&self) -> Vec<&KernelShape> {
        self.model
            .doc
            .bodies
            .iter()
            .map(|(id, _)| {
                &self
                    .model
                    .geometry
                    .get(id)
                    .expect("body hilang dari storage")
                    .shape
            })
            .collect()
    }

    pub fn visible_body_meshes(&self) -> Vec<(&str, &KernelMesh)> {
        self.model
            .doc
            .bodies
            .iter()
            .filter(|(_, meta)| meta.visible)
            .map(|(id, meta)| {
                (
                    meta.name.as_str(),
                    &self
                        .model
                        .geometry
                        .get(id)
                        .expect("body hilang dari storage")
                        .mesh,
                )
            })
            .collect()
    }

    pub fn visible_bodies_with_material(&self) -> Vec<(&str, ducad_core::Material, &KernelMesh)> {
        self.model
            .doc
            .bodies
            .iter()
            .filter(|(_, meta)| meta.visible)
            .map(|(id, meta)| {
                (
                    meta.name.as_str(),
                    meta.material,
                    &self
                        .model
                        .geometry
                        .get(id)
                        .expect("body hilang dari storage")
                        .mesh,
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_datum_plane_creation_and_indexing() {
        let top_plane = SketchPlane::top();
        let offset_plane = top_plane.offset(25.0);
        assert_eq!(offset_plane.origin, glam::Vec3::new(0.0, 0.0, 25.0));

        let angled_plane = SketchPlane::from_angle_and_edge(
            glam::Vec3::ZERO,
            glam::Vec3::new(10.0, 0.0, 0.0),
            glam::Vec3::Z,
            45.0,
        );
        let normal = angled_plane.normal;
        assert!((normal.y.abs() - normal.z.abs()).abs() < 1e-4);

        let p1 = glam::Vec3::new(0.0, 0.0, 0.0);
        let p2 = glam::Vec3::new(10.0, 0.0, 0.0);
        let p3 = glam::Vec3::new(0.0, 10.0, 5.0);
        let three_pt_plane = SketchPlane::from_3_points(p1, p2, p3).unwrap();
        assert_eq!(three_pt_plane.origin, p1);

        let planes = vec![
            (0, SketchPlane::top(), "Top".to_string()),
            (1, SketchPlane::front(), "Front".to_string()),
            (2, SketchPlane::right(), "Right".to_string()),
            (3, offset_plane, "Offset Z+25".to_string()),
        ];
        // Ray from (0, 0, 100) pointing downwards
        let hit = crate::viewport::pick_plane_index_for_ray(
            glam::Vec3::new(0.0, 0.0, 100.0),
            glam::Vec3::new(0.0, 0.0, -1.0),
            &planes,
            0,
        );
        assert_eq!(hit, Some(3));
    }

    #[test]
    fn test_delete_datum_plane_logic() {
        let mut datum_planes = vec![
            ducad_render::plane::DatumPlane::new(1, "Plane 1".to_string(), SketchPlane::top().offset(10.0)),
            ducad_render::plane::DatumPlane::new(2, "Plane 2".to_string(), SketchPlane::top().offset(20.0)),
        ];
        let mut sketches = vec![Sketch::default(), Sketch::default(), Sketch::default(), Sketch::default(), Sketch::default()];

        // Delete Plane 1 (id: 1)
        if let Some(pos) = datum_planes.iter().position(|dp| dp.id == 1) {
            let idx = pos + 3;
            datum_planes.remove(pos);
            sketches.remove(idx);
        }

        assert_eq!(datum_planes.len(), 1);
        assert_eq!(datum_planes[0].id, 2);
        assert_eq!(sketches.len(), 4);
    }
}

