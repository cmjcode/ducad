//! Insert SVG ke sketsa aktif (Paritas 1.19) — sisi GUI.
//!
//! Popup kecil: skala (pengali di atas konversi px→mm) dan tombol pilih
//! berkas. Parsing ada di `ducad_io::svg_import` (parser `usvg` yang sama
//! dengan render SVG di aplikasi). Hasil masuk sebagai satu langkah undo
//! dengan nama grup `SVG: <berkas>`, pojok kiri-bawahnya di titik asal
//! bidang sketsa.

use std::path::Path;

use ducad_io::{import_svg_entities, SvgImportOptions};
use ducad_sketch::InsertEntities;

use crate::app::DuCADApp;

/// Batas ukuran berkas SVG yang mau dibaca (byte).
const MAX_SVG_BYTES: u64 = 32 * 1024 * 1024;

pub struct SvgInsertState {
    pub open: bool,
    pub scale: f64,
}

impl Default for SvgInsertState {
    fn default() -> Self {
        Self { open: false, scale: 1.0 }
    }
}

impl DuCADApp {
    /// Perintah palette "Insert SVG…": buka popup opsi.
    pub fn open_svg_insert(&mut self) {
        self.svg_insert.open = true;
    }

    /// Impor `bytes` SVG ke sketsa aktif; `name` hanya untuk label grup.
    /// Mengembalikan jumlah entitas yang disisipkan.
    pub fn insert_svg_bytes(&mut self, bytes: &[u8], name: &str, scale: f64) -> usize {
        let opts = SvgImportOptions { scale, ..Default::default() };
        match import_svg_entities(bytes, &opts) {
            Ok(res) => {
                let n = res.entities.len();
                self.execute_sketch_command(Box::new(InsertEntities::with_group(
                    "Insert SVG",
                    res.entities,
                    format!("SVG: {name}"),
                )));
                self.file_status = Some(ducad_i18n::t!("svg-insert-done", count = n, name = name));
                n
            }
            Err(e) => {
                let err = e.to_string();
                self.file_status = Some(ducad_i18n::t!("svg-insert-failed", error = err.as_str()));
                0
            }
        }
    }

    /// Baca berkas lalu impor (dengan batas ukuran).
    pub fn insert_svg_path(&mut self, path: &Path, scale: f64) -> usize {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("berkas.svg").to_string();
        let too_big = std::fs::metadata(path).map(|m| m.len() > MAX_SVG_BYTES).unwrap_or(false);
        if too_big {
            self.file_status = Some(ducad_i18n::t!("svg-insert-failed", error = "> 32 MB"));
            return 0;
        }
        match std::fs::read(path) {
            Ok(bytes) => self.insert_svg_bytes(&bytes, &name, scale),
            Err(e) => {
                let err = e.to_string();
                self.file_status = Some(ducad_i18n::t!("svg-insert-failed", error = err.as_str()));
                0
            }
        }
    }

    /// Popup Insert SVG (dipanggil tiap frame).
    pub fn show_svg_insert(&mut self, ctx: &egui::Context) {
        if !self.svg_insert.open {
            return;
        }
        let mut open = true;
        let mut pick = false;
        egui::Window::new(ducad_i18n::t!("svg-insert-title"))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 80.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(ducad_i18n::t!("svg-insert-scale"));
                    ui.add(
                        egui::DragValue::new(&mut self.svg_insert.scale)
                            .speed(0.01)
                            .range(0.001..=1000.0),
                    );
                });
                ui.label(egui::RichText::new(ducad_i18n::t!("svg-insert-hint")).small());
                pick = ui.button(ducad_i18n::t!("svg-insert-pick")).clicked();
            });
        if pick {
            let filter = ducad_i18n::t!("svg-insert-filter");
            if let Some(path) = self.pick_open_path(&filter, &["svg"]) {
                let scale = self.svg_insert.scale;
                if self.insert_svg_path(&path, scale) > 0 {
                    self.svg_insert.open = false;
                }
            }
        } else if !open {
            self.svg_insert.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_svg_as_one_undo_step_and_rejects_garbage() {
        let mut app = DuCADApp::new_for_test();
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="5"/><circle cx="5" cy="5" r="2"/></svg>"#;
        assert_eq!(app.insert_svg_bytes(svg, "a.svg", 1.0), 2);
        assert_eq!(app.sketch().entities.len(), 2);
        app.undo();
        assert!(app.sketch().entities.is_empty());
        assert_eq!(app.insert_svg_bytes(b"<svg", "rusak.svg", 1.0), 0);
        assert_eq!(app.insert_svg_bytes(svg, "a.svg", f64::NAN), 0);
        assert!(app.sketch().entities.is_empty());
        assert_eq!(app.insert_svg_path(Path::new("/tidak/ada.svg"), 1.0), 0);
    }
}
