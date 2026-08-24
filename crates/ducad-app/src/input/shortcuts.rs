use ducad_render::PlaneKind;
use ducad_sketch::DeleteEntities;
use eframe::egui;

use crate::app::DuCADApp;
use crate::types::{FileOp, PaletteAction, ToolKind, RADIAL_TOOLS};

impl DuCADApp {
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
                "Fillet 2D".to_string(),
                "F".to_string(),
                PaletteAction::SetTool(ToolKind::Fillet2D),
            ),
            (
                "Chamfer 2D".to_string(),
                String::new(),
                PaletteAction::SetTool(ToolKind::Chamfer2D),
            ),
            (
                "Revolve (Putar 3D)".to_string(),
                "V".to_string(),
                PaletteAction::OpenRevolveDialog,
            ),
            (
                "Sweep 3D (Sapu Sepanjang Jalur)".to_string(),
                String::new(),
                PaletteAction::SetTool(ToolKind::Sweep),
            ),
            (
                "Draft Angle (Kemiringan Cetakan)".to_string(),
                "D".to_string(),
                PaletteAction::SetTool(ToolKind::DraftAngle),
            ),
            (
                "Shell / Hollow (Rongga)".to_string(),
                "S".to_string(),
                PaletteAction::SetTool(ToolKind::Shell),
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
            PaletteAction::OpenRevolveDialog => self.open_revolve_dialog(),
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
                ducad_ui::apply_theme(ctx, self.theme);
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

    pub fn handle_radial_menu(&mut self, ui: &egui::Ui, _response: &egui::Response) {
        if self.radial_menu.is_open() {
            let items: Vec<&str> = RADIAL_TOOLS.iter().map(|(_, label)| *label).collect();
            if let Some(idx) = self.radial_menu.show(ui.ctx(), &items) {
                self.set_tool(RADIAL_TOOLS[idx].0);
            }
        }
        self.radial_press = None;
    }

    pub fn status_text(&self) -> String {
        let hint = match self.tool {
            ToolKind::Select => ducad_i18n::t!("status-prompt-select"),
            ToolKind::Line => match self.pending_points.len() {
                0 => ducad_i18n::t!("status-prompt-line-0"),
                _ if self.line_chain_segments >= 2 => {
                    ducad_i18n::t!("status-prompt-line-close")
                }
                _ => ducad_i18n::t!("status-prompt-line-next"),
            },
            ToolKind::Rectangle => match self.pending_points.len() {
                0 => ducad_i18n::t!("status-prompt-rect-0"),
                _ => ducad_i18n::t!("status-prompt-rect-opp"),
            },
            ToolKind::Circle => match self.pending_points.len() {
                0 => ducad_i18n::t!("status-prompt-circle-0"),
                _ => ducad_i18n::t!("status-prompt-circle-rad"),
            },
            ToolKind::Ellipse => match self.pending_points.len() {
                0 => ducad_i18n::t!("status-prompt-ellipse-0"),
                _ => ducad_i18n::t!("status-prompt-ellipse-box"),
            },
            ToolKind::Spline => match self.pending_points.len() {
                0 => "Klik titik awal kurva Spline".to_string(),
                1 => "Klik titik berikutnya untuk membentuk kurva".to_string(),
                _ => "Klik titik berikutnya, klik titik awal untuk menutup, atau klik ulang titik akhir".to_string(),
            },
            ToolKind::Arc => match self.pending_points.len() {
                0 => ducad_i18n::t!("status-prompt-arc-0"),
                1 => ducad_i18n::t!("status-prompt-arc-1"),
                _ => ducad_i18n::t!("status-prompt-arc-2"),
            },
            ToolKind::Offset => match self.offset_source {
                None => ducad_i18n::t!("status-prompt-offset-none"),
                Some(_) => ducad_i18n::t!("status-prompt-offset-side"),
            },
            ToolKind::Mirror => {
                if self.selected.is_empty() {
                    ducad_i18n::t!("status-prompt-mirror-empty")
                } else {
                    match self.pending_points.len() {
                        0 => ducad_i18n::t!("status-prompt-mirror-p1", count = self.selected.len()),
                        _ => ducad_i18n::t!("status-prompt-mirror-p2"),
                    }
                }
            }
            ToolKind::Trim => ducad_i18n::t!("status-prompt-trim"),
            ToolKind::Fillet2D => ducad_i18n::t!("status-prompt-fillet-2d"),
            ToolKind::Chamfer2D => ducad_i18n::t!("status-prompt-chamfer-2d"),
            ToolKind::Revolve => {
                if self.selected.is_empty() {
                    ducad_i18n::t!("status-prompt-revolve-empty")
                } else {
                    match self.pending_points.len() {
                        0 => ducad_i18n::t!("status-prompt-revolve-p1", count = self.selected.len()),
                        _ => ducad_i18n::t!("status-prompt-revolve-p2"),
                    }
                }
            }
            ToolKind::CoincidentPick => match self.pending_point_refs.len() {
                0 => ducad_i18n::t!("status-prompt-coincident-0"),
                _ => ducad_i18n::t!("status-prompt-coincident-1"),
            },
            ToolKind::FixedPick => ducad_i18n::t!("status-prompt-fixed"),
            ToolKind::SymmetricPick => match self.symmetric_axis() {
                None => ducad_i18n::t!("status-prompt-symmetric-axis"),
                Some(_) => match self.pending_point_refs.len() {
                    0 => ducad_i18n::t!("status-prompt-symmetric-0"),
                    _ => ducad_i18n::t!("status-prompt-symmetric-1"),
                },
            },
            ToolKind::Measure => match self.pending_points.len() {
                0 => ducad_i18n::t!("status-prompt-measure-0"),
                _ => ducad_i18n::t!("status-prompt-measure-1"),
            },
            ToolKind::MeasureAngle => match self.pending_points.len() {
                0 => ducad_i18n::t!("status-prompt-measure-ang-0"),
                1 => ducad_i18n::t!("status-prompt-measure-ang-1"),
                _ => ducad_i18n::t!("status-prompt-measure-ang-2"),
            },
            ToolKind::Extrude => ducad_i18n::t!("status-prompt-extrude"),
            ToolKind::Loft => ducad_i18n::t!("status-prompt-loft"),
            ToolKind::Sweep => ducad_i18n::t!("status-prompt-sweep"),
            ToolKind::Shell => ducad_i18n::t!("status-prompt-shell"),
            ToolKind::DraftAngle => ducad_i18n::t!("status-prompt-draft"),
            ToolKind::Boolean => ducad_i18n::t!("status-prompt-boolean"),
            ToolKind::SectionView => ducad_i18n::t!("status-prompt-section"),
            ToolKind::History => ducad_i18n::t!("status-prompt-history"),
        };
        match &self.last_snap {
            Some(snap) => format!("{hint}  ·  snap: {:?}", snap.kind),
            None => hint,
        }
    }
}
