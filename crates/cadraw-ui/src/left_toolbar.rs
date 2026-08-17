//! Bilah Alat Vertikal Kiri (Left Floating Toolbar) bergaya Shapr3D.
//!
//! Menampilkan kolom vertikal ramping mengambang di sisi kiri kanvas, DIPUSATKAN
//! SECARA VERTIKAL antara atas & bawah viewport (lihat pemosisian `Area`-nya di
//! `main.rs`, meniru pola `FeatureInspectorState`). Isinya HANYA tool-tool yang
//! spesifik per mode: tombol "Pilih" (selalu ada) dan tool-tool sketsa 2D
//! (Line, Arc, Rectangle, dst — cuma muncul saat Sketch Mode). Kontrol yang
//! selalu sama di kedua mode (mode switcher, Items, Search, Sketch Plane,
//! Section View, Measurements, Delete) sudah dipindah ke `top_bar.rs`.

use egui::{Color32, CornerRadius, Frame, Margin, RichText, Stroke, StrokeKind, Ui, Vec2};
use egui_material_icons::icons::{
    ICON_ADS_CLICK, ICON_CHANGE_HISTORY, ICON_CIRCLE, ICON_CONTENT_CUT, ICON_CROP_16_9,
    ICON_EDIT, ICON_FLIP, ICON_HORIZONTAL_RULE, ICON_OPEN_IN_FULL, ICON_REFRESH,
};
use crate::theme::{glass_frame, ACCENT_BLUE, BORDER_SUBTLE, BG_HOVER_DARK, TEXT_PRIMARY, TEXT_SECONDARY};

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
}

pub struct LeftToolbar {
    pub is_sketching: bool,
    pub point_menu_open: bool,
}

impl Default for LeftToolbar {
    fn default() -> Self {
        Self {
            is_sketching: true,
            point_menu_open: false,
        }
    }
}

impl LeftToolbar {
    pub fn new() -> Self {
        Self::default()
    }

    /// Render bilah alat vertikal kiri bergaya Shapr3D — hanya tool "Pilih"
    /// & tool-tool sketsa 2D (muncul saat Sketch Mode).
    pub fn show(&mut self, ui: &mut Ui, current_tool: ToolbarTool) -> Option<ToolbarEvent> {
        let mut event = None;

        glass_frame().show(ui, |ui| {
            ui.set_width(36.0);
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 3.0);

            // 1. Tool List: Selalu ada "Pilih"
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

            // 2. Tools 2D (Hanya muncul saat MODE SKETCH aktif)
            if self.is_sketching {
                ui.add_space(1.0);
                ui.separator();
                ui.add_space(1.0);

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
