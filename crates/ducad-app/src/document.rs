use ducad_core::Command;
use ducad_kernel::{KernelMesh, KernelShape};
use ducad_render::{PlaneKind, SketchPlane};
use ducad_sketch::Sketch;

use crate::app::DuCADApp;
use crate::model::ModelDoc;
use crate::types::ToolKind;

impl DuCADApp {
    #[inline]
    pub fn plane_for_index(idx: usize) -> SketchPlane {
        match idx {
            0 => SketchPlane::top(),
            1 => SketchPlane::front(),
            2 => SketchPlane::right(),
            _ => SketchPlane::top(),
        }
    }

    #[inline]
    pub fn active_plane_index(&self) -> usize {
        match self.active_plane.kind {
            PlaneKind::Top => 0,
            PlaneKind::Front => 1,
            PlaneKind::Right => 2,
        }
    }

    #[inline]
    pub fn sketch(&self) -> &Sketch {
        &self.sketches[self.active_plane_index()]
    }

    #[inline]
    #[allow(dead_code)]
    pub fn sketch_mut(&mut self) -> &mut Sketch {
        let idx = self.active_plane_index();
        &mut self.sketches[idx]
    }

    #[inline]
    pub fn execute_sketch_command(&mut self, cmd: Box<dyn Command<Sketch>>) {
        let name = cmd.name().to_string();
        let idx = self.active_plane_index();
        let plane_label = self.active_plane.kind.display_label().to_string();
        self.undos[idx].execute(cmd, &mut self.sketches[idx]);

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
        self.undos[idx].undo(&mut self.sketches[idx]);
    }

    #[inline]
    pub fn redo_active_sketch(&mut self) {
        let idx = self.active_plane_index();
        self.undos[idx].redo(&mut self.sketches[idx]);
    }

    #[inline]
    pub fn can_undo_active_sketch(&self) -> bool {
        let idx = self.active_plane_index();
        self.undos[idx].can_undo()
    }

    #[inline]
    pub fn can_redo_active_sketch(&self) -> bool {
        let idx = self.active_plane_index();
        self.undos[idx].can_redo()
    }

    #[inline]
    pub fn sketch_undo_count(&self) -> usize {
        let idx = self.active_plane_index();
        self.undos[idx].undo_count()
    }

    #[inline]
    pub fn sketch_redo_count(&self) -> usize {
        let idx = self.active_plane_index();
        self.undos[idx].redo_count()
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
            self.active_plane = SketchPlane::from_kind(kind);
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
        self.sketches = [Sketch::default(), Sketch::default(), Sketch::default()];
        self.undos = [
            ducad_sketch::UndoStack::default(),
            ducad_sketch::UndoStack::default(),
            ducad_sketch::UndoStack::default(),
        ];
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

    pub fn native_body_refs(&self) -> Vec<(&str, bool, &KernelShape)> {
        self.model
            .doc
            .bodies
            .iter()
            .map(|(id, meta)| {
                (
                    meta.name.as_str(),
                    meta.visible,
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
}

