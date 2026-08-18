use eframe::egui;

use crate::app::CadrawApp;
use crate::types::{ToolKind, KEYBOARD_SHORTCUTS};

impl CadrawApp {
    #[allow(dead_code)]
    pub fn tool_buttons(&mut self, ui: &mut egui::Ui) {
        for (kind, label) in [
            (ToolKind::Select, "Pilih"),
            (ToolKind::Line, "Garis (L)"),
            (ToolKind::Rectangle, "Persegi (R)"),
            (ToolKind::Circle, "Lingkaran (C)"),
            (ToolKind::Ellipse, "Ellips (E)"),
            (ToolKind::Arc, "Arc (A)"),
            (ToolKind::Offset, "Offset (O)"),
            (ToolKind::Mirror, "Mirror (M)"),
            (ToolKind::Trim, "Trim (T)"),
            (ToolKind::Revolve, "Revolve (V)"),
        ] {
            if ui.selectable_label(self.tool == kind, label).clicked() {
                self.set_tool(kind);
            }
        }
        ui.separator();

        let point_tools = [
            (ToolKind::CoincidentPick, "Coincident (titik)"),
            (ToolKind::FixedPick, "Fixed (titik)"),
            (ToolKind::SymmetricPick, "Symmetric (titik)"),
        ];
        let active_label = point_tools
            .iter()
            .find(|(kind, _)| *kind == self.tool)
            .map(|(_, label)| format!("● {label}"))
            .unwrap_or_else(|| "Titik ▾".to_string());
        ui.menu_button(active_label, |ui| {
            for (kind, label) in point_tools {
                if ui.selectable_label(self.tool == kind, label).clicked() {
                    self.set_tool(kind);
                    ui.close();
                }
            }
        });

        let measure_tools = [
            (ToolKind::Measure, "Ukur Jarak"),
            (ToolKind::MeasureAngle, "Ukur Sudut"),
        ];
        let measure_active_label = measure_tools
            .iter()
            .find(|(kind, _)| *kind == self.tool)
            .map(|(_, label)| format!("● {label}"))
            .unwrap_or_else(|| "📏 Ukur ▾".to_string());
        ui.menu_button(measure_active_label, |ui| {
            for (kind, label) in measure_tools {
                if ui.selectable_label(self.tool == kind, label).clicked() {
                    self.set_tool(kind);
                    ui.close();
                }
            }
        });
    }

    #[allow(dead_code)]
    pub fn file_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("📄 File", |ui| {
            if ui.button("Baru").clicked() {
                self.new_document();
                ui.close();
            }
            ui.separator();
            if ui.button("Buka… (⌘O)").clicked() {
                self.open_native();
                ui.close();
            }
            if ui.button("Simpan (⌘S)").clicked() {
                self.save_native();
                ui.close();
            }
            if ui.button("Simpan Sebagai… (⌘⇧S)").clicked() {
                self.save_native_as();
                ui.close();
            }
            ui.separator();
            ui.menu_button("Import", |ui| {
                if ui.button("STEP…").clicked() {
                    self.import_step();
                    ui.close();
                }
                if ui.button("DXF…").clicked() {
                    self.import_dxf();
                    ui.close();
                }
            });
            ui.menu_button("Export", |ui| {
                if ui.button("STEP… (semua body)").clicked() {
                    self.export_step();
                    ui.close();
                }
                if ui.button("STL… (body visible)").clicked() {
                    self.export_stl();
                    ui.close();
                }
                if ui.button("OBJ… (body visible)").clicked() {
                    self.export_obj();
                    ui.close();
                }
                if ui.button("DXF… (sketch)").clicked() {
                    self.export_dxf();
                    ui.close();
                }
            });
        });
    }

    #[allow(dead_code)]
    pub fn settings_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("⚙ Pengaturan", |ui| {
            ui.label("Tema");
            if ui.button(self.theme.label()).clicked() {
                self.theme = self.theme.toggled();
                cadraw_ui::apply_theme(ui.ctx(), self.theme);
            }

            ui.separator();
            if ui.button("⌘K Buka Command Palette").clicked() {
                self.palette.open();
                ui.close();
            }

            ui.separator();
            ui.collapsing("Pintasan Keyboard", |ui| {
                egui::Grid::new("settings-keyboard-shortcuts")
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| {
                        for (key, desc) in KEYBOARD_SHORTCUTS {
                            ui.strong(key);
                            ui.label(desc);
                            ui.end_row();
                        }
                    });
            });
        });
    }
}
