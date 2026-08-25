//! Bilah Alat Vertikal Kiri (Left Floating Toolbar) bergaya Shapr3D.
//!
//! Menampilkan kolom vertikal ramping mengambang di sisi kiri kanvas, DIPUSATKAN
//! SECARA VERTIKAL antara atas & bawah viewport.
//! Isinya lengkap untuk kedua mode (Sketch Mode 2D dan Solid Mode 3D) serta
//! menu utilitas seperti History (riwayat & undo/redo) dan Pengukuran.

use crate::theme::{
    glass_frame, ACCENT_BLUE, BG_HOVER_DARK, BORDER_SUBTLE, TEXT_PRIMARY, TEXT_SECONDARY,
};
use ducad_i18n::t;
use egui::{Color32, CornerRadius, Frame, Margin, RichText, Stroke, StrokeKind, Ui, Vec2};
use egui_material_icons::icons::{
    ICON_ADS_CLICK, ICON_ARCHITECTURE, ICON_CIRCLE, ICON_CROP_16_9, ICON_ELLIPSE_OUTLINE,
    ICON_HEXAGON, ICON_HORIZONTAL_RULE, ICON_LAYERS, ICON_ROUTE, ICON_STADIUM, ICON_TIMELINE,
    ICON_TITLE,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarTool {
    Select,
    // 2D Tools
    Line,
    Arc,
    Rectangle,
    Circle,
    Ellipse,
    Polygon,
    Slot,
    Spline,
    Text,
    Fillet2D,
    Chamfer2D,
    Offset,
    Mirror,
    Trim,
    PointCoincident,
    PointFixed,
    PointSymmetric,
    Pattern,
    // 3D Tools
    Extrude,
    Revolve,
    Loft,
    Sweep,
    Shell,
    Rib,
    DraftAngle,
    SplitBody,
    Boolean,
    DatumPlane,
    SectionView,
    ZebraInspection,
    DraftAnalysis,
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

        glass_frame()
            .inner_margin(Margin::same(4))
            .corner_radius(CornerRadius::same(8))
            .show(ui, |ui| {
                ui.set_width(30.0);
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.5);

                // 1. Tool List: Selalu ada "Pilih"
                let select_active = current_tool == ToolbarTool::Select;
                let sel_title = t!("tool-select");
                let sel_desc = t!("tool-select-desc");
                let sel_btn = square_btn(
                    ui,
                    ICON_ADS_CLICK.codepoint,
                    select_active,
                    &sel_title,
                    Some("Esc"),
                    Some(&sel_desc),
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
                let line_title = t!("tool-line");
                let line_desc = t!("tool-line-desc");
                let arc_title = t!("tool-arc");
                let arc_desc = t!("tool-arc-desc");
                let rect_title = t!("tool-rectangle");
                let rect_desc = t!("tool-rectangle-desc");
                let circle_title = t!("tool-circle");
                let circle_desc = t!("tool-circle-desc");
                let ellipse_title = t!("tool-ellipse");
                let ellipse_desc = t!("tool-ellipse-desc");
                let polygon_title = t!("tool-polygon");
                let polygon_desc = t!("tool-polygon-desc");
                let slot_title = t!("tool-slot");
                let slot_desc = t!("tool-slot-desc");
                let spline_title = t!("tool-spline");
                let spline_desc = t!("tool-spline-desc");
                let text_title = t!("tool-text");
                let text_desc = t!("tool-text-desc");
                let loft_title = t!("tool-loft");
                let loft_desc = t!("tool-loft-desc");

                let sketch_tools: &[(ToolbarTool, &str, &str, Option<&str>, Option<&str>)] = &[
                    (
                        ToolbarTool::Line,
                        ICON_HORIZONTAL_RULE.codepoint,
                        &line_title,
                        Some("L"),
                        Some(&line_desc),
                    ),
                    (
                        ToolbarTool::Arc,
                        ICON_ARCHITECTURE.codepoint,
                        &arc_title,
                        Some("A"),
                        Some(&arc_desc),
                    ),
                    (
                        ToolbarTool::Rectangle,
                        ICON_CROP_16_9.codepoint,
                        &rect_title,
                        Some("R"),
                        Some(&rect_desc),
                    ),
                    (
                        ToolbarTool::Circle,
                        ICON_CIRCLE.codepoint,
                        &circle_title,
                        Some("C"),
                        Some(&circle_desc),
                    ),
                    (
                        ToolbarTool::Ellipse,
                        ICON_ELLIPSE_OUTLINE.codepoint,
                        &ellipse_title,
                        Some("E"),
                        Some(&ellipse_desc),
                    ),
                    (
                        ToolbarTool::Polygon,
                        ICON_HEXAGON.codepoint,
                        &polygon_title,
                        Some("Y"),
                        Some(&polygon_desc),
                    ),
                    (
                        ToolbarTool::Slot,
                        ICON_STADIUM.codepoint,
                        &slot_title,
                        None,
                        Some(&slot_desc),
                    ),
                    (
                        ToolbarTool::Spline,
                        ICON_TIMELINE.codepoint,
                        &spline_title,
                        Some("S"),
                        Some(&spline_desc),
                    ),
                    (
                        ToolbarTool::Text,
                        ICON_TITLE.codepoint,
                        &text_title,
                        Some("T"),
                        Some(&text_desc),
                    ),
                    (
                        ToolbarTool::Loft,
                        ICON_LAYERS.codepoint,
                        &loft_title,
                        None,
                        Some(&loft_desc),
                    ),
                ];

                for (tool, icon, title, shortcut, subtitle) in sketch_tools {
                    let is_active = current_tool == *tool;
                    let btn =
                        square_btn(ui, icon, is_active, title, *shortcut, *subtitle, None, None);
                    if btn.clicked() {
                        event = Some(ToolbarEvent::SelectTool(*tool));
                    }
                }
            } else {
                // ==================== MODE 3D SOLID ====================
                let datum_title = t!("tool-datum-plane");
                let datum_desc = t!("tool-datum-plane-desc");
                let section_title = t!("tool-section");
                let section_desc = t!("tool-section-desc");
                let sweep_title = t!("tool-sweep");
                let sweep_desc = t!("tool-sweep-desc");
                let draft_title = t!("tool-draft-angle");
                let draft_desc = t!("tool-draft-angle-desc");

                let tools_3d: &[(ToolbarTool, &str, &str, Option<&str>, Option<&str>)] = &[
                    (
                        ToolbarTool::DatumPlane,
                        ICON_LAYERS.codepoint,
                        &datum_title,
                        None,
                        Some(&datum_desc),
                    ),
                    (
                        ToolbarTool::DraftAngle,
                        ICON_ARCHITECTURE.codepoint,
                        &draft_title,
                        Some("D"),
                        Some(&draft_desc),
                    ),
                    (
                        ToolbarTool::Sweep,
                        ICON_ROUTE.codepoint,
                        &sweep_title,
                        None,
                        Some(&sweep_desc),
                    ),
                    (
                        ToolbarTool::SectionView,
                        ICON_LAYERS.codepoint,
                        &section_title,
                        None,
                        Some(&section_desc),
                    ),
                ];

                for (tool, icon, title, shortcut, subtitle) in tools_3d {
                    let is_active = current_tool == *tool;
                    let btn =
                        square_btn(ui, icon, is_active, title, *shortcut, *subtitle, None, None);
                    if btn.clicked() {
                        event = Some(ToolbarEvent::SelectTool(*tool));
                    }
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
    let size = Vec2::splat(30.0);
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

        ui.painter().rect(
            rect,
            CornerRadius::same(6),
            bg_fill,
            stroke,
            StrokeKind::Inside,
        );

        let font_id = egui::FontId::proportional(14.0);
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
            ui.label(
                RichText::new(title)
                    .strong()
                    .size(12.0)
                    .color(Color32::WHITE),
            );
            if let Some(sc) = shortcut {
                if !sc.is_empty() {
                    Frame::NONE
                        .fill(Color32::from_rgba_premultiplied(50, 54, 65, 230))
                        .corner_radius(CornerRadius::same(4))
                        .inner_margin(Margin::symmetric(4, 1))
                        .stroke(Stroke::new(0.5, BORDER_SUBTLE))
                        .show(ui, |ui| {
                            ui.label(RichText::new(sc).size(9.5).strong().color(TEXT_PRIMARY));
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
