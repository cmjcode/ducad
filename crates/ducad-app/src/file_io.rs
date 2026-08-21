use std::path::PathBuf;

use ducad_kernel::KernelShape;

use crate::app::DuCADApp;
use crate::model::{BodyGeometry, ModelDoc};
use crate::types::ToolKind;

#[cfg(target_os = "ios")]
pub fn ios_documents_dir() -> PathBuf {
    crate::apple::apple_documents_directory()
}

impl DuCADApp {
    #[cfg(not(target_os = "ios"))]
    pub fn pick_open_path(&mut self, filter_name: &str, extensions: &[&str]) -> Option<PathBuf> {
        let mut dialog = rfd::FileDialog::new().add_filter(filter_name, extensions);
        if let Some(p) = &self.current_file_path {
            if let Some(dir) = p.parent() {
                dialog = dialog.set_directory(dir);
            }
        }
        dialog.pick_file()
    }

    #[cfg(target_os = "ios")]
    pub fn pick_open_path(&mut self, _filter_name: &str, extensions: &[&str]) -> Option<PathBuf> {
        let docs = ios_documents_dir();
        let read_dir = std::fs::read_dir(&docs).ok()?;
        let mut matching_files: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if extensions.iter().any(|&e| e.eq_ignore_ascii_case(ext)) {
                        let modified = entry
                            .metadata()
                            .and_then(|m| m.modified())
                            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                        matching_files.push((modified, path));
                    }
                }
            }
        }
        matching_files.sort_by_key(|(m, _)| *m);
        matching_files.pop().map(|(_, p)| p)
    }

    #[cfg(not(target_os = "ios"))]
    pub fn pick_save_path(
        &mut self,
        filter_name: &str,
        extensions: &[&str],
        default_name: &str,
    ) -> Option<PathBuf> {
        let mut dialog = rfd::FileDialog::new()
            .set_file_name(default_name)
            .add_filter(filter_name, extensions);
        if let Some(p) = &self.current_file_path {
            if let Some(dir) = p.parent() {
                dialog = dialog.set_directory(dir);
            }
        }
        dialog.save_file()
    }

    #[cfg(target_os = "ios")]
    pub fn pick_save_path(
        &mut self,
        _filter_name: &str,
        _extensions: &[&str],
        default_name: &str,
    ) -> Option<PathBuf> {
        let docs = ios_documents_dir();
        let _ = std::fs::create_dir_all(&docs);
        Some(docs.join(default_name))
    }

    pub fn save_native_to(&mut self, path: PathBuf) {
        let body_refs = self.native_body_refs();
        match ducad_io::native::save_multi_plane(&path, &self.sketches, &body_refs) {
            Ok(_) => {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("dokumen");
                self.file_status = Some(format!("Tersimpan ke {name}"));
                self.current_file_path = Some(path);
            }
            Err(e) => {
                self.file_status = Some(format!("Gagal menyimpan: {e}"));
            }
        }
    }

    pub fn save_native(&mut self) {
        if let Some(p) = self.current_file_path.clone() {
            self.save_native_to(p);
        } else {
            self.save_native_as();
        }
    }

    pub fn save_native_as(&mut self) {
        if let Some(path) = self.pick_save_path("Dokumen DUCAD", &["ducad"], "model.ducad") {
            self.save_native_to(path);
        }
    }

    pub fn open_native(&mut self) {
        let Some(path) = self.pick_open_path("Dokumen DUCAD", &["ducad"]) else {
            return;
        };
        match ducad_io::native::load(&path) {
            Ok(loaded) => {
                self.sketches = [loaded.sketch, loaded.front_sketch, loaded.right_sketch];
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

                let mut new_model = ModelDoc::default();
                for nb in loaded.bodies {
                    let geo = BodyGeometry::from_shape(nb.shape);
                    let id = new_model.doc.add_body(&nb.name);
                    new_model.geometry.insert(id, geo);
                    if let Some(meta) = new_model.doc.bodies.get_mut(id) {
                        meta.visible = nb.visible;
                    }
                }
                self.model = new_model;
                self.model_undo = ducad_core::UndoStack::default();
                self.selected_bodies.clear();

                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("dokumen");
                self.file_status = Some(format!("Dibuka: {name}"));
                self.current_file_path = Some(path);
                self.set_tool(ToolKind::Select);
            }
            Err(e) => {
                self.file_status = Some(format!("Gagal membuka: {e}"));
            }
        }
    }

    pub fn export_step(&mut self) {
        let Some(path) = self.pick_save_path("STEP 3D CAD", &["step", "stp"], "model.step") else {
            return;
        };
        let shapes = self.all_body_shapes();
        if shapes.is_empty() {
            self.file_status = Some("Tak ada body 3D untuk diekspor ke STEP".to_string());
            return;
        }
        match ducad_io::step_io::export(&shapes, &path) {
            Ok(_) => {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("model.step");
                self.file_status = Some(format!("Diekspor ke STEP: {name}"));
            }
            Err(e) => {
                self.file_status = Some(format!("Gagal ekspor STEP: {e}"));
            }
        }
    }

    pub fn import_step(&mut self) {
        let Some(path) = self.pick_open_path("STEP 3D CAD", &["step", "stp"]) else {
            return;
        };
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string();
        self.import_worker.submit(crate::import_worker::ImportJob {
            name: name.clone(),
            path: path.clone(),
        });
        self.file_status = Some(format!("Mengimpor STEP di latar belakang: {name}…"));
        self.pending_imports += 1;
    }

    pub fn poll_import_worker(&mut self) {
        for res in self.import_worker.poll() {
            if self.pending_imports > 0 {
                self.pending_imports -= 1;
            }
            match res.outcome {
                Ok((step_str, mesh)) => {
                    match KernelShape::from_step_string(&step_str) {
                        Ok(shape) => {
                            let geo = BodyGeometry::from_shape_with_mesh(shape, mesh);
                            let cmd = crate::model::AddSolidCommand::new(res.name.clone(), geo);
                            self.execute_model_command(
                                Box::new(cmd),
                                &format!("Impor {}", res.name),
                            );
                            self.file_status =
                                Some(format!("Sukses mengimpor STEP: {}", res.name));
                        }
                        Err(e) => {
                            self.file_status =
                                Some(format!("Gagal membangun solid dari STEP: {e}"));
                        }
                    }
                }
                Err(e) => {
                    self.file_status = Some(format!("Gagal mengimpor STEP: {e}"));
                }
            }
        }
    }

    pub fn export_stl(&mut self) {
        let Some(path) = self.pick_save_path("STL Mesh", &["stl"], "model.stl") else {
            return;
        };
        let meshes = self.visible_body_meshes();
        if meshes.is_empty() {
            self.file_status = Some("Tak ada mesh 3D tampak untuk diekspor ke STL".to_string());
            return;
        }
        let mesh_refs: Vec<&ducad_kernel::KernelMesh> =
            meshes.iter().map(|(_, m)| *m).collect();
        let merged = ducad_kernel::KernelMesh::merge(&mesh_refs);
        match ducad_io::mesh_export::write_stl_binary(&merged, &path) {
            Ok(_) => {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("model.stl");
                self.file_status = Some(format!("Diekspor ke STL: {name}"));
            }
            Err(e) => self.file_status = Some(format!("Gagal ekspor STL: {e}")),
        }
    }

    pub fn export_obj(&mut self) {
        let Some(path) = self.pick_save_path("Wavefront OBJ", &["obj"], "model.obj") else {
            return;
        };
        let meshes = self.visible_body_meshes();
        if meshes.is_empty() {
            self.file_status = Some("Tak ada mesh 3D tampak untuk diekspor ke OBJ".to_string());
            return;
        }
        match ducad_io::mesh_export::write_obj(&meshes, &path) {
            Ok(_) => {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("model.obj");
                self.file_status = Some(format!("Diekspor ke OBJ: {name}"));
            }
            Err(e) => self.file_status = Some(format!("Gagal ekspor OBJ: {e}")),
        }
    }

    pub fn export_dxf(&mut self) {
        if self.sketch().entities.is_empty() {
            self.file_status =
                Some("Sketsa aktif kosong — tak ada entitas untuk diekspor".to_string());
            return;
        }
        let Some(path) = self.pick_save_path("AutoCAD DXF R12", &["dxf"], "sketch.dxf") else {
            return;
        };
        match ducad_io::dxf::export(self.sketch(), &path) {
            Ok(_) => {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("sketch.dxf");
                self.file_status = Some(format!("Diekspor ke DXF: {name}"));
            }
            Err(e) => self.file_status = Some(format!("Gagal ekspor DXF: {e}")),
        }
    }

    pub fn import_dxf(&mut self) {
        let Some(path) = self.pick_open_path("AutoCAD DXF", &["dxf"]) else {
            return;
        };
        match ducad_io::dxf::import(&path) {
            Ok(res) => {
                let count = res.entities.len();
                if count == 0 {
                    self.file_status = Some(
                        "File DXF terbaca tapi tidak memuat entitas 2D yang didukung"
                            .to_string(),
                    );
                    return;
                }
                self.execute_sketch_command(Box::new(ducad_sketch::InsertEntities::new(
                    "Impor DXF",
                    res.entities,
                )));
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file.dxf");
                self.file_status = Some(format!("Diimpor dari {name}: {count} entitas"));
            }
            Err(e) => self.file_status = Some(format!("Gagal impor DXF: {e}")),
        }
    }
}
