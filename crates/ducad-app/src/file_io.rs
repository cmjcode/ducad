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
                    .unwrap_or("model.ducad");
                self.file_status = Some(ducad_i18n::t!("file-saved-to", name = name));
                self.current_file_path = Some(path);
            }
            Err(e) => {
                let err_str = e.to_string();
                self.file_status = Some(ducad_i18n::t!("file-save-failed", error = err_str.as_str()));
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
        let filter_name = ducad_i18n::t!("file-doc-ducad");
        if let Some(path) = self.pick_save_path(&filter_name, &["ducad"], "model.ducad") {
            self.save_native_to(path);
        }
    }

    pub fn open_native(&mut self) {
        let filter_name = ducad_i18n::t!("file-doc-ducad");
        let Some(path) = self.pick_open_path(&filter_name, &["ducad"]) else {
            return;
        };
        match ducad_io::native::load(&path) {
            Ok(loaded) => {
                self.sketches = vec![loaded.sketch, loaded.front_sketch, loaded.right_sketch];
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

                let mut new_model = ModelDoc::default();
                for nb in loaded.bodies {
                    let geo = BodyGeometry::from_shape(nb.shape);
                    let id = new_model.doc.add_body_with_material(&nb.name, nb.material);
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
                    .unwrap_or("document")
                    .to_string();
                self.file_status = Some(ducad_i18n::t!("file-opened", name = name.as_str()));
                self.current_file_path = Some(path);
                self.history_db.clear();
                self.activity_cache.clear();
                let act_title = ducad_i18n::t!("file-act-open");
                let act_desc = ducad_i18n::t!("file-act-open-desc", name = name.as_str());
                self.record_activity(
                    ducad_ui::ActivityKindUi::Solid3D,
                    &act_title,
                    &act_desc,
                );
                self.set_tool(ToolKind::Select);
            }
            Err(e) => {
                let err_str = e.to_string();
                self.file_status = Some(ducad_i18n::t!("file-open-failed", error = err_str.as_str()));
            }
        }
    }

    pub fn export_step(&mut self) {
        let filter_name = ducad_i18n::t!("file-step-filter");
        let Some(path) = self.pick_save_path(&filter_name, &["step", "stp"], "model.step") else {
            return;
        };
        let shapes = self.all_body_shapes();
        if shapes.is_empty() {
            self.file_status = Some(ducad_i18n::t!("file-no-bodies-step"));
            return;
        }
        match ducad_io::step_io::export(&shapes, &path) {
            Ok(_) => {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("model.step");
                self.file_status = Some(ducad_i18n::t!("file-exported-step", name = name));
            }
            Err(e) => {
                let err_str = e.to_string();
                self.file_status = Some(ducad_i18n::t!("file-export-step-failed", error = err_str.as_str()));
            }
        }
    }

    pub fn import_step(&mut self) {
        let filter_name = ducad_i18n::t!("file-step-filter");
        let Some(path) = self.pick_open_path(&filter_name, &["step", "stp"]) else {
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
        self.file_status = Some(ducad_i18n::t!("file-importing-step", name = name.as_str()));
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
                            let act_import = ducad_i18n::t!("file-act-import-step", name = res.name.as_str());
                            let cmd = crate::model::AddSolidCommand::new(res.name.clone(), geo);
                            self.execute_model_command(
                                Box::new(cmd),
                                &act_import,
                            );
                            self.file_status =
                                Some(ducad_i18n::t!("file-imported-step", name = res.name.as_str()));
                        }
                        Err(e) => {
                            let err_str = e.to_string();
                            self.file_status =
                                Some(ducad_i18n::t!("file-import-step-build-failed", error = err_str.as_str()));
                        }
                    }
                }
                Err(e) => {
                    let err_str = e.to_string();
                    self.file_status = Some(ducad_i18n::t!("file-import-step-failed", error = err_str.as_str()));
                }
            }
        }
    }

    pub fn export_stl(&mut self) {
        let filter_name = ducad_i18n::t!("file-stl-filter");
        let Some(path) = self.pick_save_path(&filter_name, &["stl"], "model.stl") else {
            return;
        };
        let meshes = self.visible_body_meshes();
        if meshes.is_empty() {
            self.file_status = Some(ducad_i18n::t!("file-no-meshes-stl"));
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
                self.file_status = Some(ducad_i18n::t!("file-exported-stl", name = name));
            }
            Err(e) => {
                let err_str = e.to_string();
                self.file_status = Some(ducad_i18n::t!("file-export-stl-failed", error = err_str.as_str()));
            }
        }
    }

    pub fn export_obj(&mut self) {
        let filter_name = ducad_i18n::t!("file-obj-filter");
        let Some(path) = self.pick_save_path(&filter_name, &["obj"], "model.obj") else {
            return;
        };
        let meshes = self.visible_body_meshes();
        if meshes.is_empty() {
            self.file_status = Some(ducad_i18n::t!("file-no-meshes-obj"));
            return;
        }
        match ducad_io::mesh_export::write_obj(&meshes, &path) {
            Ok(_) => {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("model.obj");
                self.file_status = Some(ducad_i18n::t!("file-exported-obj", name = name));
            }
            Err(e) => {
                let err_str = e.to_string();
                self.file_status = Some(ducad_i18n::t!("file-export-obj-failed", error = err_str.as_str()));
            }
        }
    }

    pub fn export_dxf(&mut self) {
        if self.sketch().entities.is_empty() {
            self.file_status =
                Some(ducad_i18n::t!("file-sketch-empty-dxf"));
            return;
        }
        let filter_name = ducad_i18n::t!("file-dxf-filter");
        let Some(path) = self.pick_save_path(&filter_name, &["dxf"], "sketch.dxf") else {
            return;
        };
        match ducad_io::dxf::export(self.sketch(), &path) {
            Ok(_) => {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("sketch.dxf");
                self.file_status = Some(ducad_i18n::t!("file-exported-dxf", name = name));
            }
            Err(e) => {
                let err_str = e.to_string();
                self.file_status = Some(ducad_i18n::t!("file-export-dxf-failed", error = err_str.as_str()));
            }
        }
    }

    pub fn import_dxf(&mut self) {
        let filter_name = ducad_i18n::t!("file-dxf-filter");
        let Some(path) = self.pick_open_path(&filter_name, &["dxf"]) else {
            return;
        };
        match ducad_io::dxf::import(&path) {
            Ok(res) => {
                let count = res.entities.len();
                if count == 0 {
                    self.file_status = Some(ducad_i18n::t!("file-dxf-no-entities"));
                    return;
                }
                self.execute_sketch_command(Box::new(ducad_sketch::InsertEntities::new(
                    "Import DXF",
                    res.entities,
                )));
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file.dxf");
                self.file_status = Some(ducad_i18n::t!("file-imported-dxf", name = name, count = count));
            }
            Err(e) => {
                let err_str = e.to_string();
                self.file_status = Some(ducad_i18n::t!("file-import-dxf-failed", error = err_str.as_str()));
            }
        }
    }

    /// Membuat dokumen DrawingSheet baru dari seluruh body solid, mesh, dan entitas sketsa dasar.
    pub fn build_current_drawing_sheet(&self) -> ducad_io::drawing::DrawingSheet {
        let shapes = self.all_body_shapes();
        let meshes = self.visible_body_meshes();
        let mesh_refs: Vec<&ducad_kernel::KernelMesh> = meshes.iter().map(|(_, m)| *m).collect();

        // Ekstraksi entitas sketsa profil 2D aktif (garis, lingkaran, busur, spline)
        let mut sketch_segments: Vec<(glam::Vec3, glam::Vec3)> = Vec::new();
        let plane = &self.active_plane;
        for (_, entity) in &self.sketch().entities {
            match entity {
                ducad_sketch::Entity::Line { start, end, .. } => {
                    let p1 = plane.to_world(*start, 0.0);
                    let p2 = plane.to_world(*end, 0.0);
                    sketch_segments.push((p1, p2));
                }
                ducad_sketch::Entity::Circle { center, radius, .. } => {
                    let steps = 36;
                    let mut prev = plane.to_world(
                        *center + glam::DVec2::new(*radius, 0.0),
                        0.0,
                    );
                    for i in 1..=steps {
                        let theta = (i as f64) * std::f64::consts::TAU / (steps as f64);
                        let pt = plane.to_world(
                            *center + glam::DVec2::new(radius * theta.cos(), radius * theta.sin()),
                            0.0,
                        );
                        sketch_segments.push((prev, pt));
                        prev = pt;
                    }
                }
                ducad_sketch::Entity::Arc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                    ..
                } => {
                    let steps = 24;
                    let span = if *end_angle >= *start_angle {
                        *end_angle - *start_angle
                    } else {
                        *end_angle + std::f64::consts::TAU - *start_angle
                    };
                    let mut prev = plane.to_world(
                        *center + glam::DVec2::new(radius * start_angle.cos(), radius * start_angle.sin()),
                        0.0,
                    );
                    for i in 1..=steps {
                        let t = i as f64 / steps as f64;
                        let a = *start_angle + span * t;
                        let pt = plane.to_world(
                            *center + glam::DVec2::new(radius * a.cos(), radius * a.sin()),
                            0.0,
                        );
                        sketch_segments.push((prev, pt));
                        prev = pt;
                    }
                }
                ducad_sketch::Entity::Spline { points, .. } => {
                    if points.len() >= 2 {
                        for w in points.windows(2) {
                            let p1 = plane.to_world(w[0], 0.0);
                            let p2 = plane.to_world(w[1], 0.0);
                            sketch_segments.push((p1, p2));
                        }
                    }
                }
                ducad_sketch::Entity::Ellipse {
                    center,
                    radius_x,
                    radius_y,
                    ..
                } => {
                    let steps = 36;
                    let mut prev = plane.to_world(
                        *center + glam::DVec2::new(*radius_x, 0.0),
                        0.0,
                    );
                    for i in 1..=steps {
                        let theta = (i as f64) * std::f64::consts::TAU / (steps as f64);
                        let pt = plane.to_world(
                            *center
                                + glam::DVec2::new(
                                    radius_x * theta.cos(),
                                    radius_y * theta.sin(),
                                ),
                            0.0,
                        );
                        sketch_segments.push((prev, pt));
                        prev = pt;
                    }
                }
            }
        }

        let drawing = ducad_kernel::HlrExtractor::extract_drawing_with_sketch(
            &shapes,
            &mesh_refs,
            &sketch_segments,
        );

        let mut sheet = ducad_io::drawing::DrawingSheet::new(
            drawing,
            ducad_io::drawing::PaperSize::A4Landscape,
        );

        if self.section_enabled {
            let mut sec_cfg = ducad_kernel::SectionPlaneConfig::from_model_bbox_center_y(
                sheet.drawing.model_bbox_min,
                sheet.drawing.model_bbox_max,
            );
            match self.section_axis {
                crate::types::SectionAxis::Y => {
                    sec_cfg.origin[1] = self.section_offset;
                }
                crate::types::SectionAxis::X => {
                    sec_cfg.origin = [self.section_offset, 0.0, 0.0];
                    sec_cfg.normal = [1.0, 0.0, 0.0];
                    sec_cfg.u_axis = [0.0, 1.0, 0.0];
                    sec_cfg.v_axis = [0.0, 0.0, 1.0];
                }
                crate::types::SectionAxis::Z => {
                    sec_cfg.origin = [0.0, 0.0, self.section_offset];
                    sec_cfg.normal = [0.0, 0.0, 1.0];
                    sec_cfg.u_axis = [1.0, 0.0, 0.0];
                    sec_cfg.v_axis = [0.0, 1.0, 0.0];
                }
            }
            let (sec_view, cut_ind) = ducad_kernel::SectionExtractor::extract_section_view(
                &shapes,
                &mesh_refs,
                &sec_cfg,
                (sheet.drawing.model_bbox_min, sheet.drawing.model_bbox_max),
            );
            sheet.drawing.section_a = Some(sec_view);
            sheet.drawing.cutting_plane = Some(cut_ind);
            sheet.auto_layout();
        }

        // Tambahkan entitas sketsa profil (lingkaran, busur, ellips) secara permanen ke fitur geometris Tampak Atas
        for (_, entity) in &self.sketch().entities {
            match entity {
                ducad_sketch::Entity::Circle { center, radius, .. } => {
                    let feat = ducad_kernel::HlrGeometricFeature::Circle {
                        center: [center.x as f32, center.y as f32],
                        radius: *radius as f32,
                    };
                    if !sheet.drawing.top.features.iter().any(|f| match f {
                        ducad_kernel::HlrGeometricFeature::Circle { center: c, radius: r } => {
                            (c[0] - center.x as f32).hypot(c[1] - center.y as f32) < 1.0 && (r - *radius as f32).abs() < 0.5
                        }
                        _ => false,
                    }) {
                        sheet.drawing.top.features.push(feat);
                    }
                }
                ducad_sketch::Entity::Arc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                    ..
                } => {
                    let feat = ducad_kernel::HlrGeometricFeature::Arc {
                        center: [center.x as f32, center.y as f32],
                        radius: *radius as f32,
                        start_angle: *start_angle as f32,
                        end_angle: *end_angle as f32,
                    };
                    sheet.drawing.top.features.push(feat);
                }
                ducad_sketch::Entity::Ellipse {
                    center,
                    radius_x,
                    radius_y,
                    ..
                } => {
                    let feat = ducad_kernel::HlrGeometricFeature::Ellipse {
                        center: [center.x as f32, center.y as f32],
                        radius_x: *radius_x as f32,
                        radius_y: *radius_y as f32,
                        rotation: 0.0,
                    };
                    sheet.drawing.top.features.push(feat);
                }
                _ => {}
            }
        }

        // Regenerasi dimensi lengkap agar semua fitur sketsa terhitung dan mengikuti skala
        sheet.generate_auto_dimensions();

        if let Some(path) = &self.current_file_path {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                sheet.title_block.project_title = stem.to_uppercase();
                sheet.title_block.drawing_number = format!("DWG-{}", stem.to_uppercase());
            }
        }

        sheet
    }

    /// Membuka tampilan lembar kerja teknik 2D (Drawing Sheet).
    pub fn open_drawing_sheet(&mut self) {
        if self.drawing_sheet_doc.is_none() {
            let sheet = self.build_current_drawing_sheet();
            self.drawing_sheet_doc = Some(sheet);
        }
        self.drawing_sheet_state.is_open = true;
        let act_title = ducad_i18n::t!("menu-drawing-sheet");
        self.record_activity(
            ducad_ui::ActivityKindUi::Solid3D,
            &act_title,
            "Membuka lembar kerja teknik 2D (Drawing Sheet)",
        );
    }

    /// Ekspor lembar kerja gambar teknik ke PDF Vektor.
    pub fn export_drawing_pdf(&mut self) {
        let sheet = self
            .drawing_sheet_doc
            .as_ref()
            .cloned()
            .unwrap_or_else(|| self.build_current_drawing_sheet());

        let filter_name = ducad_i18n::t!("file-pdf-filter");
        let default_name = format!(
            "{}.pdf",
            sheet
                .title_block
                .project_title
                .to_lowercase()
                .replace(' ', "_")
        );

        let Some(path) = self.pick_save_path(&filter_name, &["pdf"], &default_name) else {
            return;
        };

        match ducad_io::export_pdf(&sheet, &path) {
            Ok(_) => {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("drawing.pdf");
                self.file_status = Some(ducad_i18n::t!("file-exported-pdf", name = name));
            }
            Err(e) => {
                let err_str = e.to_string();
                self.file_status =
                    Some(ducad_i18n::t!("file-export-pdf-failed", error = err_str.as_str()));
            }
        }
    }

    /// Ekspor lembar kerja gambar teknik ke file CAD DXF ber-layer.
    pub fn export_drawing_dxf(&mut self) {
        let sheet = self
            .drawing_sheet_doc
            .as_ref()
            .cloned()
            .unwrap_or_else(|| self.build_current_drawing_sheet());

        let filter_name = ducad_i18n::t!("file-drawing-dxf-filter");
        let default_name = format!(
            "{}_drawing.dxf",
            sheet
                .title_block
                .project_title
                .to_lowercase()
                .replace(' ', "_")
        );

        let Some(path) = self.pick_save_path(&filter_name, &["dxf"], &default_name) else {
            return;
        };

        match ducad_io::export_drawing_sheet(&sheet, &path) {
            Ok(_) => {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("drawing.dxf");
                self.file_status = Some(ducad_i18n::t!("file-exported-drawing-dxf", name = name));
            }
            Err(e) => {
                let err_str = e.to_string();
                self.file_status =
                    Some(ducad_i18n::t!("file-export-drawing-dxf-failed", error = err_str.as_str()));
            }
        }
    }
}
