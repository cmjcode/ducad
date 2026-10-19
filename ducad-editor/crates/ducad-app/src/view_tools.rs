//! Export PNG dari GUI (Paritas 0.15) dan Compare Bodies (0.10).
//!
//! PNG memakai renderer yang sama dengan `render_view` milik MCP/CLI
//! (`ducad_engine::render`), jadi gambar GUI dan agent identik. Compare:
//! simpan snapshot geometri, lalu bandingkan model sekarang dengannya —
//! volume bertambah tampil hijau, volume hilang merah, lewat jalur ghost
//! yang sama dengan proposal agent.

use std::sync::Arc;

use ducad_engine::diff::{diff_against_snapshot, snapshot_model, BodyDiff, ModelSnapshot};
use ducad_engine::render::{svg_to_png, RenderOptions, View};
use ducad_kernel::KernelMesh;

use crate::app::DuCADApp;

pub const PNG_SIZES: [(u32, u32); 3] = [(1280, 720), (1920, 1080), (3840, 2160)];
pub const VIEWS: [View; 7] = [View::Iso, View::Front, View::Back, View::Left, View::Right, View::Top, View::Bottom];

pub struct ViewToolsState {
    pub png_open: bool,
    pub png_view: View,
    pub png_size: usize,
    pub png_hidden_lines: bool,
    pub compare_open: bool,
    pub snapshot: Option<ModelSnapshot>,
    pub diff_rows: Vec<BodyDiff>,
    pub ghost_added: Vec<Arc<KernelMesh>>,
    pub ghost_removed: Vec<Arc<KernelMesh>>,
}

impl Default for ViewToolsState {
    fn default() -> Self {
        Self {
            png_open: false,
            png_view: View::Iso,
            png_size: 1,
            png_hidden_lines: false,
            compare_open: false,
            snapshot: None,
            diff_rows: Vec::new(),
            ghost_added: Vec::new(),
            ghost_removed: Vec::new(),
        }
    }
}

fn view_label(v: View) -> String {
    let key = match v {
        View::Iso => "view-iso",
        View::Front => "view-front",
        View::Back => "view-back",
        View::Left => "view-left",
        View::Right => "view-right",
        View::Top => "view-top",
        View::Bottom => "view-bottom",
    };
    ducad_i18n::t!(key)
}

impl DuCADApp {
    /// Render PNG model (body terlihat) ke byte.
    pub fn render_png_bytes(&mut self, view: View, width: u32, height: u32, hidden_lines: bool) -> Result<Vec<u8>, String> {
        let opt = RenderOptions { view, width, height, hidden_lines, bodies: None };
        let core = ducad_engine::SessionCore {
            model: &mut self.model,
            model_undo: &mut self.model_undo,
            sketches: &mut self.sketch_set,
            meta: &mut self.agent_meta,
        };
        let svg = ducad_engine::render::render_svg_core(&core, &opt).map_err(|e| e.message)?;
        svg_to_png(&svg.svg, width, height).map_err(|e| e.message)
    }

    fn export_png_dialog_commit(&mut self) {
        if self.model.doc.bodies.is_empty() {
            self.file_status = Some(ducad_i18n::t!("png-no-bodies"));
            return;
        }
        let (w, h) = PNG_SIZES[self.view_tools.png_size.min(PNG_SIZES.len() - 1)];
        let (view, hidden) = (self.view_tools.png_view, self.view_tools.png_hidden_lines);
        let bytes = match self.render_png_bytes(view, w, h, hidden) {
            Ok(b) => b,
            Err(e) => {
                self.file_status = Some(ducad_i18n::t!("png-failed", error = e.as_str()));
                return;
            }
        };
        let filter = ducad_i18n::t!("png-filter");
        let Some(path) = self.pick_save_path(&filter, &["png"], "model.png") else { return };
        self.file_status = Some(match std::fs::write(&path, bytes) {
            Ok(()) => {
                let name = path.display().to_string();
                ducad_i18n::t!("png-saved", name = name.as_str())
            }
            Err(e) => {
                let err = e.to_string();
                ducad_i18n::t!("png-failed", error = err.as_str())
            }
        });
        self.view_tools.png_open = false;
    }

    /// Simpan snapshot geometri untuk Compare.
    pub fn take_compare_snapshot(&mut self) {
        let snap = snapshot_model(&self.model);
        self.model_status = Some(ducad_i18n::t!("compare-snapshot-taken", count = snap.body_count()));
        self.view_tools.snapshot = Some(snap);
        self.clear_compare_overlay();
    }

    /// Bandingkan model sekarang dengan snapshot; isi tabel + ghost.
    pub fn run_compare(&mut self) {
        let Some(snap) = &self.view_tools.snapshot else {
            self.model_status = Some(ducad_i18n::t!("compare-need-snapshot"));
            return;
        };
        let (rows, shapes, warnings) = diff_against_snapshot(snap, &self.model);
        let tess = |v: &[ducad_kernel::KernelShape]| v.iter().map(|s| Arc::new(s.tessellate())).collect();
        self.view_tools.ghost_added = tess(&shapes.added);
        self.view_tools.ghost_removed = tess(&shapes.removed);
        self.view_tools.diff_rows = rows;
        if let Some(w) = warnings.first() {
            self.model_status = Some(w.clone());
        }
    }

    pub fn clear_compare_overlay(&mut self) {
        self.view_tools.diff_rows.clear();
        self.view_tools.ghost_added.clear();
        self.view_tools.ghost_removed.clear();
    }

    pub fn show_view_tools(&mut self, ctx: &egui::Context) {
        self.show_png_window(ctx);
        self.show_compare_window(ctx);
    }

    fn show_png_window(&mut self, ctx: &egui::Context) {
        if !self.view_tools.png_open {
            return;
        }
        let mut open = true;
        let mut export = false;
        let st = &mut self.view_tools;
        egui::Window::new(ducad_i18n::t!("png-title"))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(ducad_i18n::t!("png-view"));
                ui.horizontal_wrapped(|ui| {
                    for v in VIEWS {
                        ui.selectable_value(&mut st.png_view, v, view_label(v));
                    }
                });
                ui.label(ducad_i18n::t!("png-size"));
                ui.horizontal(|ui| {
                    for (i, (w, h)) in PNG_SIZES.iter().enumerate() {
                        ui.selectable_value(&mut st.png_size, i, format!("{w} x {h}"));
                    }
                });
                ui.checkbox(&mut st.png_hidden_lines, ducad_i18n::t!("png-hidden-lines"));
                export = ui.button(ducad_i18n::t!("png-export")).clicked();
            });
        if export {
            self.export_png_dialog_commit();
        }
        if !open {
            self.view_tools.png_open = false;
        }
    }

    fn show_compare_window(&mut self, ctx: &egui::Context) {
        if !self.view_tools.compare_open {
            return;
        }
        let mut open = true;
        let (mut snap, mut cmp, mut clear) = (false, false, false);
        let has_snapshot = self.view_tools.snapshot.is_some();
        let rows = &self.view_tools.diff_rows;
        egui::Window::new(ducad_i18n::t!("compare-title"))
            .open(&mut open)
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new(ducad_i18n::t!("compare-help")).small());
                ui.horizontal(|ui| {
                    snap = ui.button(ducad_i18n::t!("compare-take-snapshot")).clicked();
                    cmp = ui.add_enabled(has_snapshot, egui::Button::new(ducad_i18n::t!("compare-run"))).clicked();
                    clear = ui.button(ducad_i18n::t!("compare-clear")).clicked();
                });
                egui::Grid::new("compare-grid").num_columns(4).striped(true).show(ui, |ui| {
                    if !rows.is_empty() {
                        for h in ["inspect-col-body", "compare-col-status", "compare-col-added", "compare-col-removed"] {
                            ui.strong(ducad_i18n::t!(h));
                        }
                        ui.end_row();
                    }
                    for r in rows {
                        ui.label(&r.name);
                        ui.label(r.status);
                        ui.colored_label(
                            egui::Color32::from_rgb(22, 163, 74),
                            r.added_volume.map(|v| format!("{v:.2}")).unwrap_or_default(),
                        );
                        ui.colored_label(
                            egui::Color32::from_rgb(220, 38, 38),
                            r.removed_volume.map(|v| format!("{v:.2}")).unwrap_or_default(),
                        );
                        ui.end_row();
                    }
                });
            });
        if snap {
            self.take_compare_snapshot();
        }
        if cmp {
            self.run_compare();
        }
        if clear {
            self.clear_compare_overlay();
        }
        if !open {
            self.view_tools.compare_open = false;
            self.clear_compare_overlay();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_ui::PrimitiveKind;

    #[test]
    fn compare_reports_added_volume_after_change() {
        let mut app = DuCADApp::new_for_test();
        app.create_primitive(PrimitiveKind::Box, [10.0, 10.0, 10.0], [0.0; 3], false);
        app.take_compare_snapshot();
        let id = app.model.doc.bodies.keys().next().unwrap();
        app.selected_bodies = [id].into_iter().collect();
        app.rotate_selected_body_precise([0.0, 0.0, 1.0], 45.0, true, false);
        app.run_compare();
        assert_eq!(app.view_tools.diff_rows.len(), 1);
        let added = app.view_tools.diff_rows[0].added_volume.unwrap_or(0.0);
        assert!(added > 1.0, "{:?}", app.view_tools.diff_rows);
        assert!(!app.view_tools.ghost_added.is_empty());
        app.clear_compare_overlay();
        assert!(app.view_tools.ghost_added.is_empty());
    }

    #[test]
    fn compare_without_snapshot_reports_status() {
        let mut app = DuCADApp::new_for_test();
        app.run_compare();
        assert!(app.model_status.is_some());
        assert!(app.view_tools.diff_rows.is_empty());
    }

    #[test]
    fn png_render_produces_png_signature() {
        let mut app = DuCADApp::new_for_test();
        app.create_primitive(PrimitiveKind::Cylinder, [5.0, 10.0, 0.0], [0.0; 3], false);
        let bytes = app.render_png_bytes(View::Iso, 320, 240, false).expect("png");
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    }
}
