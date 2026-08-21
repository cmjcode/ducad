//! Bilah Alat Vertikal Kiri (Left Floating Toolbar) bergaya Shapr3D.
//!
//! Menampilkan kolom vertikal ramping mengambang di sisi kiri kanvas, DIPUSATKAN
//! SECARA VERTIKAL antara atas & bawah viewport.
//! Isinya lengkap untuk kedua mode (Sketch Mode 2D dan Solid Mode 3D) serta
//! menu utilitas seperti History (riwayat & undo/redo) dan Pengukuran.

use egui::{Color32, CornerRadius, Frame, Margin, RichText, Stroke, StrokeKind, Ui, Vec2};
use egui_material_icons::icons::{
    ICON_ADS_CLICK, ICON_ARCHITECTURE, ICON_CALL_MERGE, ICON_CIRCLE, ICON_CONTENT_CUT,
    ICON_CROP_16_9, ICON_HISTORY, ICON_HOME_MINI, ICON_HORIZONTAL_RULE,
    ICON_LAYERS,
};
use crate::theme::{glass_frame, ACCENT_BLUE, BG_HOVER_DARK, BORDER_SUBTLE, TEXT_PRIMARY, TEXT_SECONDARY};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarTool {
    Select,
    // 2D Tools
    Line,
    Arc,
    Rectangle,
    Circle,
    Ellipse,
    Offset,
    Mirror,
    Trim,
    PointCoincident,
    PointFixed,
    PointSymmetric,
    // 3D Tools
    Extrude,
    Revolve,
    Loft,
    FilletChamfer,
    Shell,
    Boolean,
    SectionView,
    // Shared Tools
    Measure,
    MeasureAngle,
    History,
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

    /// Render bilah alat vertikal kiri bergaya Shapr3D.
    #[allow(clippy::type_complexity)]
    pub fn show(&mut self, ui: &mut Ui, current_tool: ToolbarTool) -> Option<ToolbarEvent> {
        let mut event = None;

        glass_frame().show(ui, |ui| {
            ui.set_width(36.0);
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 3.0);

            // 1. Tool List: Selalu ada "Pilih"
            let select_active = current_tool == ToolbarTool::Select;
            let sel_btn = square_btn(
                ui,
                ICON_ADS_CLICK.codepoint,
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

            ui.add_space(1.0);
            ui.separator();
            ui.add_space(1.0);

            // 2. Mode-Specific Tools
            if self.is_sketching {
                // ==================== MODE 2D SKETCH (CREATE OBJECTS) ====================
                let sketch_tools: &[(ToolbarTool, &str, &str, Option<&str>, Option<&str>)] = &[
                    (ToolbarTool::Line, ICON_HORIZONTAL_RULE.codepoint, "Line", Some("L"), Some("Garis lurus 2 titik")),
                    (ToolbarTool::Arc, ICON_ARCHITECTURE.codepoint, "Arc", Some("A"), Some("Busur lengkung 3 titik")),
                    (ToolbarTool::Rectangle, ICON_CROP_16_9.codepoint, "Rectangle", Some("R"), Some("Persegi panjang 2 titik")),
                    (ToolbarTool::Circle, ICON_CIRCLE.codepoint, "Circle", Some("C"), Some("Lingkaran pusat & radius")),
                    (ToolbarTool::Ellipse, ICON_HOME_MINI.codepoint, "Ellipse", Some("E"), Some("Elips pusat & sumbu")),
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
            } else {
                // ==================== MODE 3D SOLID ====================
                let tools_3d: &[(ToolbarTool, &str, &str, Option<&str>, Option<&str>)] = &[
                    (ToolbarTool::Loft, ICON_LAYERS.codepoint, "Loft 3D", None, Some("Bentuk transisi antara 2 profil profil/tinggi")),
                    (ToolbarTool::FilletChamfer, "⤹", "Fillet & Chamfer", Some("F"), Some("Lengkung atau serongkan tepi rusuk solid")),
                    (ToolbarTool::Shell, "⧉", "Shell / Hollow", Some("S"), Some("Ronggakan benda 3D dengan ketebalan dinding")),
                    (ToolbarTool::Boolean, ICON_CALL_MERGE.codepoint, "Boolean", Some("B"), Some("Gabung (Union), Potong (Subtract), Irisan")),
                    (ToolbarTool::SectionView, ICON_CONTENT_CUT.codepoint, "Section View", None, Some("Tampilan potongan bidang X/Y/Z")),
                ];

                for (tool, icon, title, shortcut, subtitle) in tools_3d {
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
            }

            // 3. Shared Utilities (Pengukuran & Riwayat/History)
            ui.add_space(1.0);
            ui.separator();
            ui.add_space(1.0);


            // History & Undo/Redo
            let history_active = current_tool == ToolbarTool::History;
            let hist_btn = square_btn(
                ui,
                ICON_HISTORY.codepoint,
                history_active,
                "Riwayat & Undo/Redo",
                Some("H"),
                Some("Lacak riwayat langkah & undo / redo model"),
                None,
                None,
            );
            if hist_btn.clicked() {
                event = Some(ToolbarEvent::SelectTool(ToolbarTool::History));
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
