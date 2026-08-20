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
        let idx = self.active_plane_index();
        self.undos[idx].execute(cmd, &mut self.sketches[idx]);
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

