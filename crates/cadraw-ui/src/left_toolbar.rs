//! Bilah Alat Vertikal Kiri (Left Floating Toolbar) bergaya Shapr3D.
//!
//! Menampilkan kolom vertikal ramping mengambang di sisi kiri kanvas berisi tombol ikon
//! bujur sangkar (square icon buttons) dengan tooltip melayang (hover cards) untuk
//! pemilihan tool sketsa, navigasi dokumen, aksi modeling, serta utilitas cepat.

use egui::{Color32, CornerRadius, Frame, Margin, RichText, Stroke, StrokeKind, Ui, Vec2};
use egui_material_icons::icons::{
    ICON_ADS_CLICK, ICON_CATEGORY, ICON_CHANGE_HISTORY, ICON_CIRCLE, ICON_CLOSE, ICON_CONTENT_CUT,
    ICON_CROP_16_9, ICON_DELETE, ICON_EDIT, ICON_FLIP, ICON_FOLDER, ICON_FOLDER_OPEN,
    ICON_HORIZONTAL_RULE, ICON_LAYERS, ICON_OPEN_IN_FULL, ICON_REFRESH, ICON_SEARCH, ICON_STRAIGHTEN,
};
use crate::theme::{glass_frame, ACCENT_BLUE, ACCENT_ORANGE, BORDER_SUBTLE, BG_HOVER_DARK, TEXT_PRIMARY, TEXT_SECONDARY};

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
    EnterSketching,
    ExitSketching,
    SelectSketchPlane(usize),
    ToggleSectionView,
    ToggleMeasurements,
    DeleteSelection,
}

pub struct LeftToolbar {
    pub is_sketching: bool,
    pub items_drawer_open: bool,
    pub section_view_active: bool,
    pub measurement_active: bool,
    pub point_menu_open: bool,
    pub plane_menu_open: bool,
}

impl Default for LeftToolbar {
    fn default() -> Self {
        Self {
            is_sketching: true,
            items_drawer_open: false,
            section_view_active: false,
            measurement_active: false,
            point_menu_open: false,
            plane_menu_open: false,
        }
    }
}

impl LeftToolbar {
    pub fn new() -> Self {
        Self::default()
    }

    /// Render bilah alat vertikal kiri bergaya Shapr3D.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        current_tool: ToolbarTool,
        active_plane_name: &str,
    ) -> Option<ToolbarEvent> {
        let mut event = None;

        glass_frame().show(ui, |ui| {
            ui.set_width(36.0);
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 3.0);

            // 1. Mode Badge / Switcher (Shapr3D Style)
            let mode_icon = if self.is_sketching { ICON_EDIT } else { ICON_CATEGORY };
            let mode_title = if self.is_sketching { "Sketch" } else { "3D Mode" };
            let mode_sub = if self.is_sketching { "Mode Menggambar 2D" } else { "Mode Desain 3D" };
            let mod_btn = square_btn(
                ui,
                mode_icon,
                true,
                mode_title,
                None,
                Some(mode_sub),
                Some(Color32::from_rgba_premultiplied(18, 42, 85, 220)),
                Some(ACCENT_BLUE),
            );
            if mod_btn.clicked() {
                if self.is_sketching {
                    event = Some(ToolbarEvent::ExitSketching);
                } else {
                    event = Some(ToolbarEvent::EnterSketching);
                }
            }

            // 2. Items Drawer Toggle
            let folder_icon = if self.items_drawer_open {
                ICON_FOLDER_OPEN
            } else {
                ICON_FOLDER
            };
            let items_btn = square_btn(
                ui,
                folder_icon,
                self.items_drawer_open,
                "Items",
                None,
                Some("Daftar sketch & solid body"),
                None,
                None,
            );
            if items_btn.clicked() {
                self.items_drawer_open = !self.items_drawer_open;
                event = Some(ToolbarEvent::ToggleItemsDrawer);
            }

            // 3. Search Palette Button
            let search_btn = square_btn(
                ui,
                ICON_SEARCH,
                false,
                "Search",
                Some("⌘K"),
                Some("Cari tool & perintah"),
                None,
                None,
            );
            if search_btn.clicked() {
                event = Some(ToolbarEvent::OpenSearch);
            }

            // 4. Exit / Enter Sketching & Plane Selector Button
            if self.is_sketching {
                ui.add_space(2.0);
                let exit_btn = square_btn(
                    ui,
                    ICON_CLOSE,
                    false,
                    "Exit Sketching",
                    Some("Esc"),
                    Some("Keluar dari mode sketch"),
                    Some(Color32::from_rgba_premultiplied(65, 22, 22, 220)),
                    Some(Color32::from_rgb(255, 130, 130)),
                );
                if exit_btn.clicked() {
                    event = Some(ToolbarEvent::ExitSketching);
                }

                // Tombol Pemilih Bidang Sketsa (Top / Front / Right)
                let plane_btn = square_btn(
                    ui,
                    ICON_LAYERS,
                    self.plane_menu_open,
                    "Sketch Plane",
                    None,
                    Some(active_plane_name),
                    Some(Color32::from_rgba_premultiplied(18, 42, 85, 220)),
                    Some(ACCENT_BLUE),
                );
                if plane_btn.clicked() {
                    self.plane_menu_open = !self.plane_menu_open;
                }

                if self.plane_menu_open {
                    let p_rect = plane_btn.rect;
                    let menu_pos = egui::pos2(p_rect.right() + 6.0, p_rect.top() - 4.0);
                    egui::Area::new(egui::Id::new("cadraw-plane-select-popup"))
                        .fixed_pos(menu_pos)
                        .order(egui::Order::Tooltip)
                        .show(ui.ctx(), |ui| {
                            glass_frame().show(ui, |ui| {
                                ui.set_width(170.0);
                                ui.spacing_mut().item_spacing = Vec2::new(2.0, 4.0);
                                ui.label(RichText::new("PILIH BIDANG SKETSA").strong().size(10.0).color(TEXT_SECONDARY));
                                ui.separator();

                                let planes = [
                                    (0, "Top Plane (XY)", "Bidang Horizontal"),
                                    (1, "Front Plane (XZ)", "Bidang Vertikal Depan"),
                                    (2, "Right Plane (YZ)", "Bidang Vertikal Samping"),
                                ];

                                for (idx, label, sub) in planes {
                                    let active = active_plane_name.contains(&label[..5]);
                                    let btn = ui.selectable_label(active, RichText::new(format!("{} {}", ICON_LAYERS, label)).size(11.5));
                                    if btn.on_hover_text(sub).clicked() {
                                        event = Some(ToolbarEvent::SelectSketchPlane(idx));
                                        self.plane_menu_open = false;
                                    }
                                }
                            });
                        });
                }
                ui.add_space(2.0);
            } else {
                ui.add_space(2.0);
                let sketch_btn = square_btn(
                    ui,
                    ICON_EDIT,
                    false,
                    "Start Sketch",
                    Some("S"),
                    Some("Mulai menggambar 2D di bidang"),
                    Some(Color32::from_rgba_premultiplied(18, 55, 35, 220)),
                    Some(Color32::from_rgb(120, 240, 150)),
                );
                if sketch_btn.clicked() {
                    event = Some(ToolbarEvent::EnterSketching);
                }
                ui.add_space(2.0);
            }

            ui.add_space(1.0);
            ui.separator();
            ui.add_space(1.0);

            // 5. Tool List: Selalu ada "Pilih"
            let select_active = current_tool == ToolbarTool::Select;
            let sel_btn = square_btn(
                ui,
                ICON_ADS_CLICK,
                select_active,
                "Pilih",
                Some("Esc"),
                Some("Seleksi entitas atau elemen"),
                None,
                None,
            );
            if sel_btn.clicked() {
                event = Some(ToolbarEvent::SelectTool(ToolbarTool::Select));
            }

            // 6. Tools 2D (Hanya muncul saat MODE SKETCH aktif)
            if self.is_sketching {
                let sketch_tools: &[(ToolbarTool, &str, &str, Option<&str>, Option<&str>)] = &[
                    (ToolbarTool::Line, ICON_HORIZONTAL_RULE, "Line", Some("L"), Some("Garis lurus 2 titik")),
                    (ToolbarTool::Arc, ICON_CHANGE_HISTORY, "Arc", Some("A"), Some("Busur lengkung 3 titik")),
                    (ToolbarTool::Rectangle, ICON_CROP_16_9, "Rectangle", Some("R"), Some("Persegi panjang 2 titik")),
                    (ToolbarTool::Circle, ICON_CIRCLE, "Circle", Some("C"), Some("Lingkaran pusat & radius")),
                    (ToolbarTool::Ellipse, ICON_EDIT, "Ellipse", Some("E"), Some("Elips pusat & sumbu")),
                    (ToolbarTool::Offset, ICON_OPEN_IN_FULL, "Offset", Some("O"), Some("Geser paralel profil kurva")),
                    (ToolbarTool::Mirror, ICON_FLIP, "Mirror", Some("M"), Some("Cermin terhadap garis sumbu")),
                    (ToolbarTool::Trim, ICON_CONTENT_CUT, "Trim", Some("T"), Some("Potong segmen berpotongan")),
                    (ToolbarTool::Revolve, ICON_REFRESH, "Revolve", Some("V"), Some("Putar profil 360° terhadap sumbu")),
                ];

                for (tool, icon, title, shortcut, subtitle) in sketch_tools {
                    let is_active = current_tool == *tool;
                    let btn = square_btn(
                        ui,
                        icon,
                        is_active,
                        title,
                        *shortcut,
                        *subtitle,
                        None,
                        None,
                    );
                    if btn.clicked() {
                        event = Some(ToolbarEvent::SelectTool(*tool));
                    }
                }

                // Point Constraint Tools
                let is_point_active = matches!(
                    current_tool,
                    ToolbarTool::PointCoincident | ToolbarTool::PointFixed | ToolbarTool::PointSymmetric
                );
                let point_title = match current_tool {
                    ToolbarTool::PointCoincident => "Titik (Coincident)",
                    ToolbarTool::PointFixed => "Titik (Fixed)",
                    ToolbarTool::PointSymmetric => "Titik (Symmetric)",
                    _ => "Titik Constraint",
                };
                let pt_btn = square_btn(
                    ui,
                    "●",
                    is_point_active,
                    point_title,
                    None,
                    Some("Coincident / Fixed / Symmetric"),
                    None,
                    None,
                );
                if pt_btn.clicked() {
                    self.point_menu_open = !self.point_menu_open;
                }

                if self.point_menu_open {
                    let pt_rect = pt_btn.rect;
                    let menu_pos = egui::pos2(pt_rect.right() + 6.0, pt_rect.top() - 4.0);
                    egui::Area::new(egui::Id::new("cadraw-point-tools-popup"))
                        .fixed_pos(menu_pos)
                        .order(egui::Order::Tooltip)
                        .show(ui.ctx(), |ui| {
                            glass_frame().show(ui, |ui| {
                                ui.set_width(130.0);
                                ui.spacing_mut().item_spacing = Vec2::new(2.0, 3.0);
                                ui.label(RichText::new("Titik Constraint").strong().size(10.5).color(TEXT_SECONDARY));
                                ui.separator();

                                let pt_options = [
                                    (ToolbarTool::PointCoincident, "● Coincident", "Berimpit (2 pt)"),
                                    (ToolbarTool::PointFixed, "🔒 Fixed", "Terkunci (1 pt)"),
                                    (ToolbarTool::PointSymmetric, "⫿ Symmetric", "Simetris (2 pt)"),
                                ];

                                for (t, label, sub) in pt_options {
                                    let selected = current_tool == t;
                                    let btn = ui.selectable_label(selected, RichText::new(label).size(11.5));
                                    if btn.on_hover_text(sub).clicked() {
                                        event = Some(ToolbarEvent::SelectTool(t));
                                        self.point_menu_open = false;
                                    }
                                }
                            });
                        });
                }
            }

            ui.add_space(1.0);
            ui.separator();
            ui.add_space(1.0);

            // 7. Utilities (Section View, Measure, Delete)
            let sec_btn = square_btn(
                ui,
                ICON_CONTENT_CUT,
                self.section_view_active,
                "Section View",
                None,
                Some("Potong tampilan visual 3D"),
                if self.section_view_active {
                    Some(Color32::from_rgba_premultiplied(190, 100, 15, 230))
                } else {
                    None
                },
                if self.section_view_active {
                    Some(Color32::WHITE)
                } else {
                    Some(if self.section_view_active { ACCENT_ORANGE } else { TEXT_PRIMARY })
                },
            );
            if sec_btn.clicked() {
                event = Some(ToolbarEvent::ToggleSectionView);
            }

            let is_meas_active = current_tool == ToolbarTool::Measure || current_tool == ToolbarTool::MeasureAngle;
            let meas_btn = square_btn(
                ui,
                ICON_STRAIGHTEN,
                is_meas_active,
                "Measurements",
                None,
                Some("Ukur jarak & sudut"),
                None,
                None,
            );
            if meas_btn.clicked() {
                event = Some(ToolbarEvent::ToggleMeasurements);
            }

            let del_btn = square_btn(
                ui,
                ICON_DELETE,
                false,
                "Delete",
                Some("⌫"),
                Some("Hapus entitas atau body terpilih"),
                None,
                Some(Color32::from_rgb(255, 110, 110)),
            );
            if del_btn.clicked() {
                event = Some(ToolbarEvent::DeleteSelection);
            }
        });

        event
    }
}

/// Helper fungsi untuk menggambar tombol bujur sangkar (square button) dengan tooltip hover yang elegan.
#[allow(clippy::too_many_arguments)]
fn square_btn(
    ui: &mut Ui,
    icon: &'static str,
    is_active: bool,
    title: &str,
    shortcut: Option<&str>,
    subtitle: Option<&str>,
    custom_bg: Option<Color32>,
    custom_fg: Option<Color32>,
) -> egui::Response {
    let size = Vec2::splat(34.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let (bg_fill, stroke, icon_color) = if is_active {
            (
                custom_bg.unwrap_or(ACCENT_BLUE),
                Stroke::new(1.0, Color32::from_rgb(100, 180, 255)),
                custom_fg.unwrap_or(Color32::WHITE),
            )
        } else if response.hovered() {
            (
                custom_bg.unwrap_or(BG_HOVER_DARK),
                Stroke::new(1.0, Color32::from_rgba_premultiplied(10, 132, 255, 180)),
                custom_fg.unwrap_or(Color32::WHITE),
            )
        } else {
            (
                custom_bg.unwrap_or(Color32::from_rgba_premultiplied(30, 33, 40, 140)),
                Stroke::new(0.5, BORDER_SUBTLE),
                custom_fg.unwrap_or(TEXT_PRIMARY),
            )
        };

        ui.painter().rect(rect, CornerRadius::same(7), bg_fill, stroke, StrokeKind::Inside);

        let font_id = egui::FontId::proportional(15.5);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            icon,
            font_id,
            icon_color,
        );
    }

    // Kartu Tooltip Melayang saat Di-Hover
    response.on_hover_ui(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 2.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(title).strong().size(12.0).color(Color32::WHITE));
            if let Some(sc) = shortcut {
                if !sc.is_empty() {
                    Frame::NONE
                        .fill(Color32::from_rgba_premultiplied(50, 54, 65, 230))
                        .corner_radius(CornerRadius::same(4))
                        .inner_margin(Margin::symmetric(4, 1))
                        .stroke(Stroke::new(0.5, BORDER_SUBTLE))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(sc)
                                    .size(9.5)
                                    .strong()
                                    .color(TEXT_PRIMARY),
                            );
                        });
                }
            }
        });
        if let Some(sub) = subtitle {
            if !sub.is_empty() {
                ui.label(RichText::new(sub).size(10.0).color(TEXT_SECONDARY));
            }
        }
    })
}
