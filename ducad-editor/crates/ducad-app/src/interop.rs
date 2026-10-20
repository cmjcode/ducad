//! Ekspor/impor Fase 3 (PLAN_PARITAS_SOLIDCRAFT §3): 3MF (3.9), impor mesh
//! 3MF/OBJ/glTF (3.10), IGES (3.11), dan DXF garis tampak/tersembunyi dari
//! sudut pandang aktif (3.15). Dipanggil dari command palette.

use std::path::{Path, PathBuf};

use crate::app::DuCADApp;

fn file_name(path: &Path, fallback: &str) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(fallback)
        .to_string()
}

impl DuCADApp {
    fn default_export_name(&self, ext: &str) -> String {
        self.current_file_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .and_then(|n| n.to_str())
            .map(|stem| format!("{stem}.{ext}"))
            .unwrap_or_else(|| format!("model.{ext}"))
    }

    /// Body terlihat (nama, mesh, warna material) untuk ekspor mesh.
    fn visible_mesh_exports(&self) -> Vec<(String, ducad_kernel::KernelMesh, Option<[f32; 3]>)> {
        self.model
            .doc
            .bodies
            .iter()
            .filter(|(_, b)| b.visible)
            .filter_map(|(id, b)| {
                let g = self.model.geometry.get(id)?;
                let c = b.material.base_color;
                Some((b.name.clone(), g.mesh.as_ref().clone(), Some([c[0], c[1], c[2]])))
            })
            .collect()
    }

    /// Tulis 3MF body terlihat ke `path`.
    pub fn export_3mf_to(&mut self, path: &Path) -> bool {
        let bodies = self.visible_mesh_exports();
        if bodies.is_empty() {
            self.file_status = Some(ducad_i18n::t!("file-no-meshes-stl"));
            return false;
        }
        let refs: Vec<(&str, &ducad_kernel::KernelMesh, Option<[f32; 3]>)> =
            bodies.iter().map(|(n, m, c)| (n.as_str(), m, *c)).collect();
        match ducad_io::mesh_formats::write_3mf(&refs, path) {
            Ok(()) => {
                let name = file_name(path, "model.3mf");
                self.file_status = Some(ducad_i18n::t!("file-exported-3mf", name = name.as_str()));
                true
            }
            Err(e) => {
                self.file_status =
                    Some(ducad_i18n::t!("file-export-failed", error = e.to_string()));
                false
            }
        }
    }

    pub fn export_3mf(&mut self) {
        let filter = ducad_i18n::t!("file-3mf-filter");
        let default = self.default_export_name("3mf");
        if let Some(path) = self.pick_save_path(&filter, &["3mf"], &default) {
            self.export_3mf_to(&path);
        }
    }

    /// Tulis IGES semua body B-rep ke `path`.
    pub fn export_iges_to(&mut self, path: &Path) -> bool {
        let named: Vec<(String, ducad_kernel::KernelShape)> = self
            .model
            .doc
            .bodies
            .iter()
            .filter_map(|(id, b)| {
                let g = self.model.geometry.get(id)?;
                ducad_kernel::clone_shape(&g.shape)
                    .ok()
                    .map(|s| (b.name.clone(), s))
            })
            .collect();
        let refs: Vec<(&str, &ducad_kernel::KernelShape)> =
            named.iter().map(|(n, s)| (n.as_str(), s)).collect();
        match ducad_kernel::iges::write_iges(&refs, path) {
            Ok(()) => {
                let name = file_name(path, "model.igs");
                self.file_status = Some(ducad_i18n::t!("file-exported-iges", name = name.as_str()));
                true
            }
            Err(e) => {
                self.file_status =
                    Some(ducad_i18n::t!("file-export-failed", error = e.to_string()));
                false
            }
        }
    }

    pub fn export_iges(&mut self) {
        let filter = ducad_i18n::t!("file-iges-filter");
        let default = self.default_export_name("igs");
        if let Some(path) = self.pick_save_path(&filter, &["igs", "iges"], &default) {
            self.export_iges_to(&path);
        }
    }

    /// Kirim berkas ke import worker (STEP/STL/IGES/3MF/OBJ/glTF).
    pub fn import_any_path(&mut self, path: PathBuf) {
        let name = file_name(&path, "file");
        self.import_worker.submit(crate::import_worker::ImportJob {
            name: name.clone(),
            path,
        });
        self.file_status = Some(ducad_i18n::t!("file-importing", name = name.as_str()));
        self.pending_imports += 1;
    }

    pub fn import_mesh_file(&mut self) {
        let filter = ducad_i18n::t!("file-mesh-filter");
        if let Some(path) = self.request_open(
            crate::mobile::OpenPurpose::Stl,
            &filter,
            &["3mf", "obj", "gltf", "glb", "stl"],
        ) {
            self.import_any_path(path);
        }
    }

    pub fn import_iges(&mut self) {
        let filter = ducad_i18n::t!("file-iges-filter");
        if let Some(path) =
            self.request_open(crate::mobile::OpenPurpose::Step, &filter, &["igs", "iges"])
        {
            self.import_any_path(path);
        }
    }

    /// DXF garis tampak + tersembunyi dari sudut pandang viewport saat ini.
    pub fn export_view_dxf(&mut self) {
        let snapshot = match self.capture_view_snapshot() {
            Ok(s) => s,
            Err(msg) => {
                self.file_status = Some(msg);
                return;
            }
        };
        let filter = ducad_i18n::t!("file-dxf-filter");
        let default = self
            .default_export_name("dxf")
            .replace(".dxf", "_view.dxf");
        let Some(path) = self.pick_save_path(&filter, &["dxf"], &default) else {
            return;
        };
        match ducad_io::dxf::export_vector_snapshot_dxf(&snapshot, &path) {
            Ok(()) => {
                let name = file_name(&path, "view.dxf");
                self.file_status = Some(ducad_i18n::t!(
                    "file-exported-view-dxf",
                    name = name.as_str(),
                    count = snapshot.segments.len()
                ));
            }
            Err(e) => {
                self.file_status =
                    Some(ducad_i18n::t!("file-export-failed", error = e.to_string()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_3mf_and_iges_then_reimport() {
        let mut app = DuCADApp::new_for_test();
        app.create_primitive(ducad_ui::PrimitiveKind::Box, [10.0, 10.0, 10.0], [0.0; 3], false);
        let dir = std::env::temp_dir().join(format!("ducad-interop-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f3 = dir.join("a.3mf");
        assert!(app.export_3mf_to(&f3), "{:?}", app.file_status);
        let back = ducad_io::mesh_formats::read_mesh_file(&f3).unwrap();
        assert_eq!(back.len(), 1);
        let fi = dir.join("a.igs");
        assert!(app.export_iges_to(&fi), "{:?}", app.file_status);
        let (shapes, _) = ducad_kernel::iges::read_iges(&fi).unwrap();
        assert!((shapes[0].1.volume().abs() - 1000.0).abs() < 1.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_without_bodies_reports_status() {
        let mut app = DuCADApp::new_for_test();
        let p = std::env::temp_dir().join("ducad-empty.3mf");
        assert!(!app.export_3mf_to(&p));
        assert!(app.file_status.is_some());
    }
}
