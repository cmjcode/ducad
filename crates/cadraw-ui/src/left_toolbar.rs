//! Bilah Alat Vertikal Kiri (Left Floating Toolbar) bergaya Shapr3D.
//!
//! Menampilkan kolom vertikal mengambang di sisi kiri kanvas berisi ikon Material + label
//! rapi untuk pemilihan tool sketsa, aksi direct modeling, mode badge,
//! toggle drawer Items, serta quick utilities di bagian bawah.

use egui::{Color32, RichText, Stroke, Ui, Vec2};
use egui_material_icons::icons::{
    ICON_ADS_CLICK, ICON_CHANGE_HISTORY, ICON_CIRCLE, ICON_CLOSE, ICON_CONTENT_CUT,
    ICON_CROP_16_9, ICON_DELETE, ICON_EDIT, ICON_FLIP, ICON_FOLDER, ICON_FOLDER_OPEN,
    ICON_HORIZONTAL_RULE, ICON_OPEN_IN_FULL, ICON_REFRESH, ICON_SEARCH,
    ICON_STRAIGHTEN, ICON_CATEGORY,
};
use crate::theme::{glass_frame, ACCENT_BLUE, TEXT_PRIMARY, TEXT_SECONDARY};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarTool {
    Select,
    Line,
    Arc,
    Rectangle,
    Circle,
    Ellipse,
    Offset,
    Mirror,
    Trim,
    Revolve,
    PointCoincident,
    PointFixed,
    PointSymmetric,
    Measure,
    MeasureAngle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarEvent {
    SelectTool(ToolbarTool),
    ToggleItemsDrawer,
    OpenSearch,
    ExitSketching,
    ToggleSectionView,
    ToggleMeasurements,
    DeleteSelection,
}

pub struct LeftToolbar {
    pub is_sketching: bool,
    pub items_drawer_open: bool,
    pub section_view_active: bool,
    pub measurement_active: bool,
}

impl Default for LeftToolbar {
    fn default() -> Self {
        Self {
            is_sketching: true,
            items_drawer_open: false,
            section_view_active: false,
            measurement_active: false,
        }
    }
}

impl LeftToolbar {
    pub fn new() -> Self {
        Self::default()
    }

    /// Render bilah alat vertikal kiri. Mengembalikan `Some(ToolbarEvent)` jika ada interaksi.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        current_tool: ToolbarTool,
        active_plane_name: &str,
    ) -> Option<ToolbarEvent> {
        let mut event = None;

        glass_frame().show(ui, |ui| {
            ui.set_width(132.0);
            ui.spacing_mut().item_spacing = Vec2::new(2.0, 2.0);

            // 1. Mode Badge / Switcher (Shapr3D Style)
            ui.horizontal(|ui| {
                ui.label(RichText::new(ICON_CATEGORY).size(14.0).color(ACCENT_BLUE));
                ui.label(RichText::new("Modeling").strong().size(12.0).color(TEXT_PRIMARY));
            });

            ui.add_space(2.0);
            ui.separator();
            ui.add_space(2.0);

            // 2. Navigation & Drawer Controls
            let folder_icon = if self.items_drawer_open { ICON_FOLDER_OPEN } else { ICON_FOLDER };
            let items_text = format!("{} Items", folder_icon);
            let items_btn = ui.selectable_label(
                self.items_drawer_open,
                RichText::new(items_text).size(11.5).color(TEXT_PRIMARY),
            );
            if items_btn.clicked() {
                self.items_drawer_open = !self.items_drawer_open;
                event = Some(ToolbarEvent::ToggleItemsDrawer);
            }

            let search_btn = ui.button(
                RichText::new(format!("{} Search", ICON_SEARCH))
                    .size(11.5)
                    .color(TEXT_PRIMARY),
            );
            if search_btn.clicked() {
                event = Some(ToolbarEvent::OpenSearch);
            }

            // 3. Exit Sketching Pill (hanya tampil jika sedang dalam mode sketch)
            if self.is_sketching {
                ui.add_space(2.0);
                let exit_btn = ui.add_sized(
                    Vec2::new(ui.available_width(), 28.0),
                    egui::Button::new(
                        RichText::new(format!("{} Exit Sketching", ICON_CLOSE))
                            .size(10.5)
                            .strong()
                            .color(Color32::from_rgb(255, 140, 140)),
                    )
                    .fill(Color32::from_rgba_premultiplied(55, 25, 25, 220))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(180, 50, 50))),
                );
                if exit_btn.on_hover_text(format!("Bidang: {}", active_plane_name)).clicked() {
                    event = Some(ToolbarEvent::ExitSketching);
                }
                ui.add_space(2.0);
            }

            ui.separator();
            ui.add_space(2.0);

            // 4. Sketch & Modeling Tool List dengan Icon Material
            let tools = [
                (ToolbarTool::Select, ICON_ADS_CLICK, "Pilih", ""),
                (ToolbarTool::Line, ICON_HORIZONTAL_RULE, "Line", "L"),
                (ToolbarTool::Arc, ICON_CHANGE_HISTORY, "Arc", "A"),
                (ToolbarTool::Rectangle, ICON_CROP_16_9, "Rectangle", "R"),
                (ToolbarTool::Circle, ICON_CIRCLE, "Circle", "C"),
                (ToolbarTool::Ellipse, ICON_EDIT, "Ellipse", "E"),
                (ToolbarTool::Offset, ICON_OPEN_IN_FULL, "Offset", "O"),
                (ToolbarTool::Mirror, ICON_FLIP, "Mirror", "M"),
                (ToolbarTool::Trim, ICON_CONTENT_CUT, "Trim", "T"),
                (ToolbarTool::Revolve, ICON_REFRESH, "Revolve", "V"),
            ];

            for (tool, icon, label, shortcut) in tools {
                let is_active = current_tool == tool;
                let row_resp = ui.horizontal(|ui| {
                    let formatted = format!("{} {}", icon, label);
                    let text = if is_active {
                        RichText::new(formatted).strong().size(11.5).color(Color32::WHITE)
                    } else {
                        RichText::new(formatted).size(11.5).color(TEXT_PRIMARY)
                    };
                    let btn = ui.selectable_label(is_active, text);
                    if btn.clicked() {
                        event = Some(ToolbarEvent::SelectTool(tool));
                    }
                    if !shortcut.is_empty() {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let sc_color = if is_active { Color32::WHITE } else { TEXT_SECONDARY };
                            ui.label(RichText::new(shortcut).size(9.5).color(sc_color));
                        });
                    }
                });
                if row_resp.response.clicked() {
                    event = Some(ToolbarEvent::SelectTool(tool));
                }
            }

            ui.separator();

            // 5. Tool Titik Constraint Pick
            let point_tools = [
                (ToolbarTool::PointCoincident, "Coincident"),
                (ToolbarTool::PointFixed, "Fixed"),
                (ToolbarTool::PointSymmetric, "Symmetric"),
            ];
            let active_point_label = point_tools
                .iter()
                .find(|(t, _)| *t == current_tool)
                .map(|(_, l)| format!("● {}", l))
                .unwrap_or_else(|| "Titik ▾".to_string());

            ui.menu_button(RichText::new(active_point_label).size(11.0).color(TEXT_PRIMARY), |ui| {
                for (tool, label) in point_tools {
                    if ui.selectable_label(current_tool == tool, label).clicked() {
                        event = Some(ToolbarEvent::SelectTool(tool));
                        ui.close();
                    }
                }
            });

            ui.separator();

            // 6. Utilities Cepat
            let sec_label = if self.section_view_active {
                format!("{} Section (ON)", ICON_CONTENT_CUT)
            } else {
                format!("{} Section", ICON_CONTENT_CUT)
            };
            if ui.selectable_label(self.section_view_active, RichText::new(sec_label).size(11.0)).clicked() {
                event = Some(ToolbarEvent::ToggleSectionView);
            }

            let meas_label = format!("{} Measure", ICON_STRAIGHTEN);
            if ui.button(RichText::new(meas_label).size(11.0)).clicked() {
                event = Some(ToolbarEvent::ToggleMeasurements);
            }

            let del_label = format!("{} Delete", ICON_DELETE);
            if ui.button(RichText::new(del_label).size(11.0).color(Color32::from_rgb(240, 100, 100))).clicked() {
                event = Some(ToolbarEvent::DeleteSelection);
            }
        });

        event
    }
}
