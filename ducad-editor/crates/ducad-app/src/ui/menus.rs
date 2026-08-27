use ducad_i18n::{current_language, set_language, t, Language};
use eframe::egui;

use crate::app::DuCADApp;
use crate::types::{ToolKind, KEYBOARD_SHORTCUTS};

impl DuCADApp {
    #[allow(dead_code)]
    pub fn tool_buttons(&mut self, ui: &mut egui::Ui) {
        let select_label = t!("tool-select");
        let line_label = format!("{} (L)", t!("tool-line"));
        let rect_label = format!("{} (R)", t!("tool-rectangle"));
        let circle_label = format!("{} (C)", t!("tool-circle"));
        let ellipse_label = format!("{} (E)", t!("tool-ellipse"));
        let polygon_label = format!("{} (Y)", t!("tool-polygon"));
        let text_label = format!("{} (T)", t!("tool-text"));
        let spline_label = format!("{} (S)", t!("tool-spline"));
        let fillet_label = format!("{} (F)", t!("tool-fillet-2d"));
        let chamfer_label = t!("tool-chamfer-2d");
        let arc_label = format!("{} (A)", t!("tool-arc"));
        let offset_label = format!("{} (O)", t!("tool-offset"));
        let mirror_label = format!("{} (M)", t!("tool-mirror"));
        let trim_label = format!("{} (T)", t!("tool-trim"));
        let revolve_label = format!("{} (V)", t!("tool-revolve"));

        for (kind, label) in [
            (ToolKind::Select, select_label.as_str()),
            (ToolKind::Line, line_label.as_str()),
            (ToolKind::Rectangle, rect_label.as_str()),
            (ToolKind::Circle, circle_label.as_str()),
            (ToolKind::Ellipse, ellipse_label.as_str()),
            (ToolKind::Polygon, polygon_label.as_str()),
            (ToolKind::Text, text_label.as_str()),
            (ToolKind::Spline, spline_label.as_str()),
            (ToolKind::Fillet2D, fillet_label.as_str()),
            (ToolKind::Chamfer2D, chamfer_label.as_str()),
            (ToolKind::Arc, arc_label.as_str()),
            (ToolKind::Offset, offset_label.as_str()),
            (ToolKind::Mirror, mirror_label.as_str()),
            (ToolKind::Trim, trim_label.as_str()),
            (ToolKind::Revolve, revolve_label.as_str()),
        ] {
            if ui.selectable_label(self.tool == kind, label).clicked() {
                if kind == ToolKind::Revolve {
                    self.open_revolve_dialog();
                } else {
                    self.set_tool(kind);
                }
            }
        }
        ui.separator();

        let pt_coincident = format!("{} ({})", t!("tool-coincident"), t!("param-axis"));
        let pt_fixed = format!("{} ({})", t!("tool-fixed"), t!("param-axis"));
        let pt_symmetric = format!("{} ({})", t!("tool-symmetric"), t!("param-axis"));
        let point_tools = [
            (ToolKind::CoincidentPick, pt_coincident.as_str()),
            (ToolKind::FixedPick, pt_fixed.as_str()),
            (ToolKind::SymmetricPick, pt_symmetric.as_str()),
        ];
        let active_label = point_tools
            .iter()
            .find(|(kind, _)| *kind == self.tool)
            .map(|(_, label)| format!("● {label}"))
            .unwrap_or_else(|| format!("{} ▾", t!("tool-coincident")));
        ui.menu_button(active_label, |ui| {
            for (kind, label) in point_tools {
                if ui.selectable_label(self.tool == kind, label).clicked() {
                    self.set_tool(kind);
                    ui.close();
                }
            }
        });

        let measure_dist = t!("tool-measure");
        let measure_ang = t!("tool-measure-angle");
        let measure_tools = [
            (ToolKind::Measure, measure_dist.as_str()),
            (ToolKind::MeasureAngle, measure_ang.as_str()),
        ];
        let measure_active_label = measure_tools
            .iter()
            .find(|(kind, _)| *kind == self.tool)
            .map(|(_, label)| format!("● {label}"))
            .unwrap_or_else(|| format!("📏 {} ▾", t!("tool-measure")));
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
        ui.menu_button(format!("📄 {}", t!("menu-file")), |ui| {
            if ui.button(t!("menu-new")).clicked() {
                self.new_document();
                ui.close();
            }
            ui.separator();
            if ui.button(format!("{} (⌘O)", t!("menu-open"))).clicked() {
                self.open_native();
                ui.close();
            }
            if ui.button(format!("{} (⌘S)", t!("menu-save"))).clicked() {
                self.save_native();
                ui.close();
            }
            if ui.button(format!("{} (⌘+Shift+S)", t!("menu-save-as"))).clicked() {
                self.save_native_as();
                ui.close();
            }
            ui.separator();
            ui.menu_button(t!("menu-import"), |ui| {
                if ui.button(t!("menu-import-step")).clicked() {
                    self.import_step();
                    ui.close();
                }
                if ui.button(t!("menu-import-dxf")).clicked() {
                    self.import_dxf();
                    ui.close();
                }
            });
            ui.menu_button(t!("menu-export"), |ui| {
                if ui.button(t!("menu-export-step")).clicked() {
                    self.export_step();
                    ui.close();
                }
                if ui.button(t!("menu-export-stl")).clicked() {
                    self.export_stl();
                    ui.close();
                }
                if ui.button(t!("menu-export-obj")).clicked() {
                    self.export_obj();
                    ui.close();
                }
                if ui.button(t!("menu-export-glb")).clicked() {
                    self.export_glb();
                    ui.close();
                }
                if ui.button(t!("menu-export-dxf")).clicked() {
                    self.export_dxf();
                    ui.close();
                }
                if ui.button(t!("menu-export-svg")).clicked() {
                    self.export_sketch_svg();
                    ui.close();
                }
            });
        });
    }

    #[allow(dead_code)]
    pub fn settings_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button(format!("⚙ {}", t!("menu-settings")), |ui| {
            ui.label(t!("menu-theme"));
            if ui.button(self.theme.label()).clicked() {
                self.theme = self.theme.toggled();
                ducad_ui::apply_theme(ui.ctx(), self.theme);
            }

            ui.separator();
            ui.menu_button(
                format!("🌐 {} ({})", t!("lang-current"), current_language().display_name()),
                |ui| {
                    for lang in Language::all() {
                        let is_sel = current_language() == *lang;
                        let prefix = if is_sel { "✓ " } else { "   " };
                        if ui.button(format!("{}{}", prefix, lang.display_name())).clicked() {
                            self.language = *lang;
                            set_language(*lang);
                            ui.close();
                        }
                    }
                },
            );

            ui.separator();
            if ui.button(format!("⌘K / ⌘⇧P {}", t!("menu-command-palette"))).clicked() {
                self.palette.open();
                ui.close();
            }

            ui.separator();
            ui.collapsing(t!("menu-shortcuts"), |ui| {
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
