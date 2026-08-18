use cadraw_render::PlaneKind;
use cadraw_sketch::DeleteEntities;
use eframe::egui;

use crate::app::CadrawApp;
use crate::types::{FileOp, PaletteAction, ToolKind, RADIAL_TOOLS};

impl CadrawApp {
    pub fn palette_actions(&self) -> Vec<(String, String, PaletteAction)> {
        let mut actions = vec![
            (
                "Dokumen Baru".to_string(),
                String::new(),
                PaletteAction::File(FileOp::New),
            ),
            (
                "Buka…".to_string(),
                "⌘O".to_string(),
                PaletteAction::File(FileOp::Open),
            ),
            (
                "Simpan".to_string(),
                "⌘S".to_string(),
                PaletteAction::File(FileOp::Save),
            ),
            (
                "Simpan Sebagai…".to_string(),
                "⌘⇧S".to_string(),
                PaletteAction::File(FileOp::SaveAs),
            ),
            (
                "Import STEP…".to_string(),
                String::new(),
                PaletteAction::File(FileOp::ImportStep),
            ),
            (
                "Import DXF…".to_string(),
                String::new(),
                PaletteAction::File(FileOp::ImportDxf),
            ),
            (
                "Export STEP… (semua body)".to_string(),
                String::new(),
                PaletteAction::File(FileOp::ExportStep),
            ),
            (
                "Export STL… (body visible)".to_string(),
                String::new(),
                PaletteAction::File(FileOp::ExportStl),
            ),
            (
                "Export OBJ… (body visible)".to_string(),
                String::new(),
                PaletteAction::File(FileOp::ExportObj),
            ),
            (
                "Export DXF… (sketch)".to_string(),
                String::new(),
                PaletteAction::File(FileOp::ExportDxf),
            ),
            (
                "Pilih".to_string(),
                String::new(),
                PaletteAction::SetTool(ToolKind::Select),
            ),
            (
                "Garis".to_string(),
                "L".to_string(),
                PaletteAction::SetTool(ToolKind::Line),
            ),
            (
                "Persegi".to_string(),
                "R".to_string(),
                PaletteAction::SetTool(ToolKind::Rectangle),
            ),
            (
                "Lingkaran".to_string(),
                "C".to_string(),
                PaletteAction::SetTool(ToolKind::Circle),
            ),
            (
                "Ellips".to_string(),
                "E".to_string(),
                PaletteAction::SetTool(ToolKind::Ellipse),
            ),
            (
                "Arc".to_string(),
                "A".to_string(),
                PaletteAction::SetTool(ToolKind::Arc),
            ),
            (
                "Offset".to_string(),
                "O".to_string(),
                PaletteAction::SetTool(ToolKind::Offset),
            ),
            (
                "Mirror".to_string(),
                "M".to_string(),
                PaletteAction::SetTool(ToolKind::Mirror),
            ),
            (
                "Trim".to_string(),
                "T".to_string(),
                PaletteAction::SetTool(ToolKind::Trim),
            ),
            (
                "Revolve".to_string(),
                "V".to_string(),
                PaletteAction::SetTool(ToolKind::Revolve),
            ),
            (
                "Coincident (titik)".to_string(),
                String::new(),
                PaletteAction::SetTool(ToolKind::CoincidentPick),
            ),
            (
                "Fixed (titik)".to_string(),
                String::new(),
                PaletteAction::SetTool(ToolKind::FixedPick),
            ),
            (
                "Symmetric (titik)".to_string(),
                String::new(),
                PaletteAction::SetTool(ToolKind::SymmetricPick),
            ),
            (
                "Ukur Jarak".to_string(),
                String::new(),
                PaletteAction::SetTool(ToolKind::Measure),
            ),
            (
                "Ukur Sudut".to_string(),
                String::new(),
                PaletteAction::SetTool(ToolKind::MeasureAngle),
            ),
            (
                "Undo Sketch".to_string(),
                "⌘Z".to_string(),
                PaletteAction::Undo,
            ),
            (
                "Redo Sketch".to_string(),
                "⌘⇧Z".to_string(),
                PaletteAction::Redo,
            ),
            (
                "Undo Model".to_string(),
                String::new(),
                PaletteAction::ModelUndo,
            ),
            (
                "Redo Model".to_string(),
                String::new(),
                PaletteAction::ModelRedo,
            ),
            (
                "Sketch: Bidang Top (XY)".to_string(),
                String::new(),
                PaletteAction::SetSketchPlane(PlaneKind::Top),
            ),
            (
                "Sketch: Bidang Vertikal Front (XZ)".to_string(),
                String::new(),
                PaletteAction::SetSketchPlane(PlaneKind::Front),
            ),
            (
                "Sketch: Bidang Vertikal Right (YZ)".to_string(),
                String::new(),
                PaletteAction::SetSketchPlane(PlaneKind::Right),
            ),
            (
                "Mode Sketch (2D)".to_string(),
                "⌘⇧2".to_string(),
                PaletteAction::EnterSketching,
            ),
            (
                "Mode 3D".to_string(),
                "⌘⇧3".to_string(),
                PaletteAction::ExitSketching,
            ),
            (
                format!("Ganti Tema ({})", self.theme.toggled().label()),
                String::new(),
                PaletteAction::ToggleTheme,
            ),
        ];
        if !self.selected.is_empty() {
            actions.push((
                format!("Hapus Seleksi ({} entitas)", self.selected.len()),
                "Del".to_string(),
                PaletteAction::DeleteSelection,
            ));
        }
        if !self.measurements.is_empty() {
            actions.push((
                format!("Hapus Semua Pengukuran ({})", self.measurements.len()),
                String::new(),
                PaletteAction::ClearMeasurements,
            ));
        }
        actions
    }

    pub fn run_palette_action(&mut self, ctx: &egui::Context, action: PaletteAction) {
        match action {
            PaletteAction::SetTool(kind) => self.set_tool(kind),
            PaletteAction::SetSketchPlane(kind) => self.set_sketch_plane(kind),
            PaletteAction::EnterSketching => {
                self.is_sketching = true;
                self.left_toolbar.is_sketching = true;
                self.camera.orient_to_plane(&self.active_plane);
            }
            PaletteAction::ExitSketching => {
                self.is_sketching = false;
                self.left_toolbar.is_sketching = false;
                self.set_tool(ToolKind::Select);
            }
            PaletteAction::Undo => {
                self.undo_active_sketch();
            }
            PaletteAction::Redo => {
                self.redo_active_sketch();
            }
            PaletteAction::ModelUndo => {
                self.model_undo.undo(&mut self.model);
                self.selected_bodies.clear();
            }
            PaletteAction::ModelRedo => {
                self.model_undo.redo(&mut self.model);
                self.selected_bodies.clear();
            }
            PaletteAction::DeleteSelection => {
                if !self.selected.is_empty() {
                    let ids: Vec<_> = self.selected.drain().collect();
                    self.execute_sketch_command(Box::new(DeleteEntities::new(ids)));
                }
            }
            PaletteAction::ToggleTheme => {
                self.theme = self.theme.toggled();
                cadraw_ui::apply_theme(ctx, self.theme);
            }
            PaletteAction::ClearMeasurements => {
                self.measurements.clear();
            }
            PaletteAction::File(op) => match op {
                FileOp::New => self.new_document(),
                FileOp::Open => self.open_native(),
                FileOp::Save => self.save_native(),
                FileOp::SaveAs => self.save_native_as(),
                FileOp::ImportStep => self.import_step(),
                FileOp::ImportDxf => self.import_dxf(),
                FileOp::ExportStep => self.export_step(),
                FileOp::ExportStl => self.export_stl(),
                FileOp::ExportObj => self.export_obj(),
                FileOp::ExportDxf => self.export_dxf(),
            },
        }
    }

    pub fn handle_radial_menu(&mut self, ui: &egui::Ui, response: &egui::Response) {
        const LONG_PRESS_SECS: f64 = 0.42;
        const MOVE_TOLERANCE: f32 = 6.0;

        if self.radial_menu.is_open() {
            let items: Vec<&str> = RADIAL_TOOLS.iter().map(|(_, label)| *label).collect();
            if let Some(idx) = self.radial_menu.show(ui.ctx(), &items) {
                self.set_tool(RADIAL_TOOLS[idx].0);
            }
            return;
        }

        if self.tool != ToolKind::Select {
            self.radial_press = None;
            return;
        }

        let now = ui.input(|i| i.time);
        if response.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_down()) {
            let pos = response
                .interact_pointer_pos()
                .unwrap_or_else(|| ui.input(|i| i.pointer.hover_pos()).unwrap_or_default());
            match self.radial_press {
                None => self.radial_press = Some((pos, now)),
                Some((start_pos, start_time)) => {
                    if pos.distance(start_pos) > MOVE_TOLERANCE {
                        self.radial_press = None;
                    } else if now - start_time >= LONG_PRESS_SECS {
                        self.radial_menu.open_at(start_pos);
                        self.radial_suppress_click = true;
                        self.radial_press = None;
                    }
                }
            }
        } else {
            self.radial_press = None;
        }
    }

    pub fn status_text(&self) -> String {
        let hint = match self.tool {
            ToolKind::Select => {
                "Pilih: klik entitas, Shift+klik multi-pilih, Delete hapus".to_string()
            }
            ToolKind::Line => match self.pending_points.len() {
                0 => "Garis: klik titik awal (L)".to_string(),
                _ if self.line_chain_segments >= 2 => {
                    "Garis: klik titik berikutnya, klik titik awal untuk tutup loop, atau ESC untuk selesai".to_string()
                }
                _ => "Garis: klik titik berikutnya, atau ESC untuk selesai".to_string(),
            },
            ToolKind::Rectangle => match self.pending_points.len() {
                0 => "Persegi: klik sudut pertama (R)".to_string(),
                _ => "Persegi: klik sudut berlawanan".to_string(),
            },
            ToolKind::Circle => match self.pending_points.len() {
                0 => "Lingkaran: klik titik pusat (C)".to_string(),
                _ => "Lingkaran: klik untuk radius, atau ketik radius lalu Enter".to_string(),
            },
            ToolKind::Ellipse => match self.pending_points.len() {
                0 => "Ellips: klik titik pusat (E)".to_string(),
                _ => "Ellips: klik sudut kotak pembatas".to_string(),
            },
            ToolKind::Arc => match self.pending_points.len() {
                0 => "Arc: klik titik awal (A)".to_string(),
                1 => "Arc: klik titik akhir".to_string(),
                _ => "Arc: klik titik di busur (menentukan sisi)".to_string(),
            },
            ToolKind::Offset => match self.offset_source {
                None => "Offset: klik entitas sumber (O)".to_string(),
                Some(_) => "Offset: klik sisi & jarak hasil offset".to_string(),
            },
            ToolKind::Mirror => {
                if self.selected.is_empty() {
                    "Mirror: pilih entitas di tool Pilih dulu, lalu tekan M".to_string()
                } else {
                    match self.pending_points.len() {
                        0 => format!(
                            "Mirror: klik titik 1 sumbu cermin ({} entitas terpilih)",
                            self.selected.len()
                        ),
                        _ => "Mirror: klik titik 2 sumbu cermin".to_string(),
                    }
                }
            }
            ToolKind::Trim => "Trim: klik segmen garis yang mau dipotong (T)".to_string(),
            ToolKind::Revolve => {
                if self.selected.is_empty() {
                    "Revolve: pilih profil di tool Pilih dulu, lalu tekan V".to_string()
                } else {
                    match self.pending_points.len() {
                        0 => format!(
                            "Revolve: klik titik 1 sumbu ({} entitas terpilih, 360°)",
                            self.selected.len()
                        ),
                        _ => "Revolve: klik titik 2 sumbu".to_string(),
                    }
                }
            }
            ToolKind::CoincidentPick => match self.pending_point_refs.len() {
                0 => "Coincident: klik titik pertama (endpoint/center)".to_string(),
                _ => "Coincident: klik titik kedua".to_string(),
            },
            ToolKind::FixedPick => {
                "Fixed: klik titik (endpoint/center) untuk menahannya di posisi sekarang".to_string()
            }
            ToolKind::SymmetricPick => match self.symmetric_axis() {
                None => "Symmetric: pilih 1 Line jadi sumbu di tool Pilih dulu".to_string(),
                Some(_) => match self.pending_point_refs.len() {
                    0 => "Symmetric: klik titik pertama (endpoint/center)".to_string(),
                    _ => "Symmetric: klik titik kedua".to_string(),
                },
            },
            ToolKind::Measure => match self.pending_points.len() {
                0 => "Ukur: klik titik pertama".to_string(),
                _ => "Ukur: klik titik kedua".to_string(),
            },
            ToolKind::MeasureAngle => match self.pending_points.len() {
                0 => "Ukur Sudut: klik titik awal".to_string(),
                1 => "Ukur Sudut: klik titik sudut (vertex)".to_string(),
                _ => "Ukur Sudut: klik titik akhir".to_string(),
            },
        };
        match &self.last_snap {
            Some(snap) => format!("{hint}  ·  snap: {:?}", snap.kind),
            None => hint,
        }
    }
}
