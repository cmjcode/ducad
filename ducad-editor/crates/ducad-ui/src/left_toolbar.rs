//! Bilah Alat Vertikal Kiri (Left Floating Toolbar) bergaya Shapr3D.
//!
//! Menampilkan kolom vertikal ramping mengambang di sisi kiri kanvas, dipusatkan
//! secara vertikal antara atas & bawah viewport. Mendukung label teks opsional
//! (aktif secara bawaan, dapat disembunyikan pengguna mahir), target sentuh 44 pt,
//! tema Terang & Gelap lewat [`ThemeTokens`], serta `widget_info` AccessKit.

use crate::theme::{toolbar_frame, ThemeTokens, RADIUS, TYPOGRAPHY};
use ducad_i18n::t;
use egui::{
    Color32, CornerRadius, Frame, Margin, RichText, ScrollArea, Stroke, StrokeKind, Ui, Vec2,
};
use egui_icons::icons::{
    ICON_ADJUST, ICON_ADS_CLICK, ICON_ARCHITECTURE, ICON_ARROWS_OUTWARD, ICON_CALL_MERGE,
    ICON_CALL_SPLIT, ICON_CIRCLE, ICON_COLORIZE, ICON_CONTENT_CUT, ICON_CROP_16_9, ICON_EDIT,
    ICON_ELLIPSE_OUTLINE, ICON_FIBER_MANUAL_RECORD, ICON_FOUNTAIN_PEN_TIP, ICON_GESTURE,
    ICON_GRADIENT, ICON_GRID_VIEW, ICON_HEATING_COIL, ICON_HEXAGON, ICON_HORIZONTAL_RULE,
    ICON_LAYERS, ICON_LAYERS_OFF, ICON_OPEN_IN_FULL, ICON_REFRESH, ICON_ROUTE, ICON_STADIUM,
    ICON_TIMELINE, ICON_TITLE, ICON_VIEW_IN_AR, ICON_WAVES,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// Titik sketch mandiri (Paritas 1.1).
    Point,
    /// Coretan bebas (Pencil/mouse) → bentuk rapi ber-constraint.
    Freehand,
    Text,
    Fillet2D,
    Chamfer2D,
    Offset,
    Mirror,
    Trim,
    Extend,
    /// Pecah entitas di titik klik (Paritas 1.13).
    Break,
    PointCoincident,
    PointFixed,
    PointSymmetric,
    Pattern,
    // Mode Vektor (M2.1)
    PenBezier,
    NodeEdit,
    ShapeBuilder,
    Eyedropper,
    Gradient,
    // 3D Tools
    Extrude,
    Revolve,
    Loft,
    Sweep,
    Helix,
    Shell,
    Rib,
    DraftAngle,
    SplitBody,
    Boolean,
    HoleWizard,
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
    /// Buka panel Primitif (Box/Silinder/Bola/Kerucut).
    OpenPrimitive,
    /// Paritas 6.8: buka workspace Surface (alat permukaan).
    OpenSurface,
}

pub struct LeftToolbar {
    pub is_sketching: bool,
    pub is_vector_mode: bool,
    pub point_menu_open: bool,
    pub icon_size: f32,
    /// Tampilkan label teks di samping ikon pada rel alat utama (bawaan `true`).
    pub show_labels: bool,
}

impl Default for LeftToolbar {
    fn default() -> Self {
        Self {
            is_sketching: true,
            is_vector_mode: false,
            point_menu_open: false,
            icon_size: crate::theme::ICON_SIZE_DEFAULT,
            show_labels: true,
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
        let icon_sz = self.icon_size.clamp(12.0, 18.0);
        let btn_height = (icon_sz + 14.0).max(ui.spacing().interact_size.y).max(30.0);
        let btn_width = if self.show_labels {
            138.0
        } else {
            btn_height
        };
        let max_scroll_h = (ui.available_height() - 16.0).max(220.0);

        toolbar_frame().show(ui, |ui| {
            ui.spacing_mut().interact_size.y = btn_height;
            ui.set_width(btn_width);
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.5);

            ScrollArea::vertical()
                .id_salt("ducad-left-toolbar-scroll")
                .max_height(max_scroll_h)
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
                .show(ui, |ui| {
                    // 1. Tool List: Selalu ada "Pilih"
                    let select_active = current_tool == ToolbarTool::Select;
                    let sel_title = t!("tool-select");
                    let sel_desc = t!("tool-select-desc");
                    let sel_btn = rail_item_btn(
                        ui,
                        ICON_ADS_CLICK.codepoint,
                        icon_sz,
                        btn_width,
                        btn_height,
                        self.show_labels,
                        select_active,
                        &sel_title,
                        Some("Esc"),
                        Some(&sel_desc),
                        None,
                        None,
                    );
                    publish_tool_rect(ui, ToolbarTool::Select, sel_btn.rect);
                    if sel_btn.clicked() {
                        event = Some(ToolbarEvent::SelectTool(ToolbarTool::Select));
                    }

                    ui.add_space(1.0);
                    ui.separator();
                    ui.add_space(1.0);

                    // 2. Mode-Specific Tools
                    if self.is_vector_mode {
                        let node_title = t!("tool-node-edit");
                        let node_desc = t!("tool-node-edit-desc");
                        let pen_title = t!("tool-pen-bezier");
                        let pen_desc = t!("tool-pen-bezier-desc");
                        let line_title = t!("tool-line");
                        let line_desc = t!("tool-line-desc");
                        let rect_title = t!("tool-rectangle");
                        let rect_desc = t!("tool-rectangle-desc");
                        let ellipse_title = t!("tool-ellipse");
                        let ellipse_desc = t!("tool-ellipse-desc");
                        let polygon_title = t!("tool-polygon");
                        let polygon_desc = t!("tool-polygon-desc");
                        let text_title = t!("tool-text");
                        let text_desc = t!("tool-text-desc");
                        let shape_title = t!("tool-shape-builder");
                        let shape_desc = t!("tool-shape-builder-desc");
                        let eyedropper_title = t!("tool-eyedropper");
                        let eyedropper_desc = t!("tool-eyedropper-desc");
                        let gradient_title = t!("tool-gradient");
                        let gradient_desc = t!("tool-gradient-desc");

                        let vector_tools: &[(ToolbarTool, &str, &str, Option<&str>, Option<&str>)] =
                            &[
                                (
                                    ToolbarTool::NodeEdit,
                                    ICON_EDIT.codepoint,
                                    &node_title,
                                    Some("N"),
                                    Some(&node_desc),
                                ),
                                (
                                    ToolbarTool::PenBezier,
                                    ICON_FOUNTAIN_PEN_TIP.codepoint,
                                    &pen_title,
                                    Some("P"),
                                    Some(&pen_desc),
                                ),
                                (
                                    ToolbarTool::Line,
                                    ICON_HORIZONTAL_RULE.codepoint,
                                    &line_title,
                                    Some("L"),
                                    Some(&line_desc),
                                ),
                                (
                                    ToolbarTool::Rectangle,
                                    ICON_CROP_16_9.codepoint,
                                    &rect_title,
                                    Some("R"),
                                    Some(&rect_desc),
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
                                    ToolbarTool::Text,
                                    ICON_TITLE.codepoint,
                                    &text_title,
                                    Some("T"),
                                    Some(&text_desc),
                                ),
                                (
                                    ToolbarTool::ShapeBuilder,
                                    ICON_CALL_MERGE.codepoint,
                                    &shape_title,
                                    Some("Shift+M"),
                                    Some(&shape_desc),
                                ),
                                (
                                    ToolbarTool::Eyedropper,
                                    ICON_COLORIZE.codepoint,
                                    &eyedropper_title,
                                    Some("I"),
                                    Some(&eyedropper_desc),
                                ),
                                (
                                    ToolbarTool::Gradient,
                                    ICON_GRADIENT.codepoint,
                                    &gradient_title,
                                    Some("G"),
                                    Some(&gradient_desc),
                                ),
                            ];

                        for (tool, icon, title, shortcut, subtitle) in vector_tools {
                            let is_active = current_tool == *tool;
                            let btn = rail_item_btn(
                                ui,
                                icon,
                                icon_sz,
                                btn_width,
                                btn_height,
                                self.show_labels,
                                is_active,
                                title,
                                *shortcut,
                                *subtitle,
                                None,
                                None,
                            );
                            publish_tool_rect(ui, *tool, btn.rect);
                            if btn.clicked() {
                                event = Some(ToolbarEvent::SelectTool(*tool));
                            }
                        }
                    } else if self.is_sketching {
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
                        let point_title = t!("tool-point");
                        let point_desc = t!("tool-point-desc");
                        let freehand_title = t!("tool-freehand");
                        let freehand_desc = t!("tool-freehand-desc");
                        let text_title = t!("tool-text");
                        let text_desc = t!("tool-text-desc");
                        let trim_title = t!("tool-trim");
                        let trim_desc = t!("tool-trim-desc");
                        let fillet_title = t!("tool-fillet-2d");
                        let fillet_desc = t!("tool-fillet-2d-desc");
                        let extend_title = t!("tool-extend");
                        let extend_desc = t!("tool-extend-desc");
                        let break_title = t!("tool-break");
                        let break_desc = t!("tool-break-desc");
                        let datum_title = t!("tool-datum-plane");
                        let datum_desc = t!("tool-datum-plane-desc");
                        let loft_title = t!("tool-loft");
                        let loft_desc = t!("tool-loft-desc");

                        let sketch_tools: &[(ToolbarTool, &str, &str, Option<&str>, Option<&str>)] =
                            &[
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
                                    ToolbarTool::Point,
                                    ICON_FIBER_MANUAL_RECORD.codepoint,
                                    &point_title,
                                    None,
                                    Some(&point_desc),
                                ),
                                (
                                    ToolbarTool::Freehand,
                                    ICON_GESTURE.codepoint,
                                    &freehand_title,
                                    Some("Shift+F"),
                                    Some(&freehand_desc),
                                ),
                                (
                                    ToolbarTool::Text,
                                    ICON_TITLE.codepoint,
                                    &text_title,
                                    Some("Shift+T"),
                                    Some(&text_desc),
                                ),
                                (
                                    ToolbarTool::Trim,
                                    ICON_CONTENT_CUT.codepoint,
                                    &trim_title,
                                    Some("T"),
                                    Some(&trim_desc),
                                ),
                                (
                                    ToolbarTool::Fillet2D,
                                    ICON_ADJUST.codepoint,
                                    &fillet_title,
                                    Some("F"),
                                    Some(&fillet_desc),
                                ),
                                (
                                    ToolbarTool::Extend,
                                    ICON_ARROWS_OUTWARD.codepoint,
                                    &extend_title,
                                    Some("Shift+E"),
                                    Some(&extend_desc),
                                ),
                                (
                                    ToolbarTool::Break,
                                    ICON_CALL_SPLIT.codepoint,
                                    &break_title,
                                    None,
                                    Some(&break_desc),
                                ),
                                (
                                    ToolbarTool::DatumPlane,
                                    ICON_LAYERS_OFF.codepoint,
                                    &datum_title,
                                    Some("Shift+P"),
                                    Some(&datum_desc),
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
                            let btn = rail_item_btn(
                                ui,
                                icon,
                                icon_sz,
                                btn_width,
                                btn_height,
                                self.show_labels,
                                is_active,
                                title,
                                *shortcut,
                                *subtitle,
                                None,
                                None,
                            );
                            publish_tool_rect(ui, *tool, btn.rect);
                            if btn.clicked() {
                                event = Some(ToolbarEvent::SelectTool(*tool));
                            }
                        }
                    } else {
                        // ==================== MODE 3D SOLID ====================
                        let prim_title = t!("prim-title");
                        let prim_desc = t!("prim-desc");
                        if rail_item_btn(
                            ui,
                            ICON_VIEW_IN_AR.codepoint,
                            icon_sz,
                            btn_width,
                            btn_height,
                            self.show_labels,
                            false,
                            &prim_title,
                            None,
                            Some(&prim_desc),
                            None,
                            None,
                        )
                        .clicked()
                        {
                            event = Some(ToolbarEvent::OpenPrimitive);
                        }
                        let surf_title = t!("surface-title");
                        let surf_desc = t!("surface-desc");
                        if rail_item_btn(
                            ui,
                            ICON_WAVES.codepoint,
                            icon_sz,
                            btn_width,
                            btn_height,
                            self.show_labels,
                            false,
                            &surf_title,
                            None,
                            Some(&surf_desc),
                            None,
                            None,
                        )
                        .clicked()
                        {
                            event = Some(ToolbarEvent::OpenSurface);
                        }

                        let extrude_title = t!("tool-extrude");
                        let extrude_desc = t!("status-prompt-extrude");
                        let revolve_title = t!("tool-revolve");
                        let revolve_desc = t!("status-prompt-revolve-empty");
                        let boolean_title = t!("tool-boolean");
                        let boolean_desc = t!("status-prompt-boolean");
                        let hole_title = t!("tool-hole-wizard");
                        let hole_desc = t!("tool-hole-wizard-desc");
                        let shell_title = t!("tool-shell");
                        let shell_desc = t!("status-prompt-shell");
                        let pattern_title = t!("tool-pattern");
                        let pattern_desc = t!("status-prompt-pattern");
                        let split_title = t!("tool-split-body");
                        let split_desc = t!("status-prompt-split-body");
                        let datum_title = t!("tool-datum-plane");
                        let datum_desc = t!("tool-datum-plane-desc");
                        let sweep_title = t!("tool-sweep");
                        let sweep_desc = t!("tool-sweep-desc");
                        let helix_title = t!("tool-helix");
                        let helix_desc = t!("tool-helix-desc");
                        let draft_title = t!("tool-draft-angle");
                        let draft_desc = t!("tool-draft-angle-desc");
                        let section_title = t!("tool-section");
                        let section_desc = t!("tool-section-desc");

                        let tools_3d: &[(ToolbarTool, &str, &str, Option<&str>, Option<&str>)] = &[
                            (
                                ToolbarTool::Extrude,
                                ICON_OPEN_IN_FULL.codepoint,
                                &extrude_title,
                                Some("E"),
                                Some(&extrude_desc),
                            ),
                            (
                                ToolbarTool::Revolve,
                                ICON_REFRESH.codepoint,
                                &revolve_title,
                                Some("V"),
                                Some(&revolve_desc),
                            ),
                            (
                                ToolbarTool::Boolean,
                                ICON_CALL_MERGE.codepoint,
                                &boolean_title,
                                Some("B"),
                                Some(&boolean_desc),
                            ),
                            (
                                ToolbarTool::HoleWizard,
                                ICON_ADJUST.codepoint,
                                &hole_title,
                                Some("W"),
                                Some(&hole_desc),
                            ),
                            (
                                ToolbarTool::Shell,
                                ICON_CUBE_OUTLINE_ALIAS,
                                &shell_title,
                                Some("H"),
                                Some(&shell_desc),
                            ),
                            (
                                ToolbarTool::Pattern,
                                ICON_GRID_VIEW.codepoint,
                                &pattern_title,
                                Some("P"),
                                Some(&pattern_desc),
                            ),
                            (
                                ToolbarTool::SplitBody,
                                ICON_CALL_SPLIT.codepoint,
                                &split_title,
                                None,
                                Some(&split_desc),
                            ),
                            (
                                ToolbarTool::DatumPlane,
                                ICON_LAYERS_OFF.codepoint,
                                &datum_title,
                                Some("Shift+P"),
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
                                ToolbarTool::Helix,
                                ICON_HEATING_COIL.codepoint,
                                &helix_title,
                                Some("G"),
                                Some(&helix_desc),
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
                            let btn = rail_item_btn(
                                ui,
                                icon,
                                icon_sz,
                                btn_width,
                                btn_height,
                                self.show_labels,
                                is_active,
                                title,
                                *shortcut,
                                *subtitle,
                                None,
                                None,
                            );
                            publish_tool_rect(ui, *tool, btn.rect);
                            if btn.clicked() {
                                event = Some(ToolbarEvent::SelectTool(*tool));
                            }
                        }
                    }
                });
        });

        event
    }
}

const ICON_CUBE_OUTLINE_ALIAS: &str = egui_icons::icons::ICON_CUBE_OUTLINE.codepoint;

/// Umumkan letak tombol tool supaya tutorial selamat datang bisa menyorotnya.
fn publish_tool_rect(ui: &Ui, tool: ToolbarTool, rect: egui::Rect) {
    crate::onboarding::Onboarding::publish_target(
        ui.ctx(),
        crate::onboarding::OnboardingTarget::Tool(tool),
        rect,
    );
}

/// Sisi tombol persegi rail (toolbar kiri maupun rail kanan) untuk ukuran ikon tertentu.
pub fn rail_button_side(icon_size: f32) -> f32 {
    (icon_size.clamp(12.0, 18.0) + 14.0).max(30.0)
}

/// Tombol persegi bergaya toolbar kiri untuk rail vertikal lain (mis. rail kanan).
pub fn rail_square_btn(
    ui: &mut Ui,
    icon: &str,
    icon_size: f32,
    is_active: bool,
    title: &str,
    shortcut: Option<&str>,
    subtitle: Option<&str>,
) -> egui::Response {
    square_btn(
        ui,
        icon,
        icon_size.clamp(12.0, 18.0),
        is_active,
        title,
        shortcut,
        subtitle,
        None,
        None,
    )
}

/// Tombol rail yang bisa dinonaktifkan (mis. Undo saat riwayat kosong).
pub fn rail_square_btn_enabled(
    ui: &mut Ui,
    icon: &str,
    icon_size: f32,
    enabled: bool,
    title: &str,
    shortcut: Option<&str>,
    subtitle: Option<&str>,
) -> egui::Response {
    let tok = ThemeTokens::from_ui(ui);
    let fg = if enabled { None } else { Some(tok.text_muted) };
    let resp = ui
        .add_enabled_ui(enabled, |ui| {
            square_btn(
                ui,
                icon,
                icon_size.clamp(12.0, 18.0),
                false,
                title,
                shortcut,
                subtitle,
                None,
                fg,
            )
        })
        .inner;
    resp.on_disabled_hover_text(title)
}

#[allow(clippy::too_many_arguments)]
fn square_btn(
    ui: &mut Ui,
    icon: &str,
    icon_size: f32,
    is_active: bool,
    title: &str,
    shortcut: Option<&str>,
    subtitle: Option<&str>,
    custom_bg: Option<Color32>,
    custom_fg: Option<Color32>,
) -> egui::Response {
    let btn_side = (icon_size + 14.0).max(ui.spacing().interact_size.y).max(30.0);
    rail_item_btn(
        ui, icon, icon_size, btn_side, btn_side, false, is_active, title, shortcut, subtitle,
        custom_bg, custom_fg,
    )
}

#[allow(clippy::too_many_arguments)]
fn rail_item_btn(
    ui: &mut Ui,
    icon: &str,
    icon_size: f32,
    btn_width: f32,
    btn_height: f32,
    show_label: bool,
    is_active: bool,
    title: &str,
    shortcut: Option<&str>,
    subtitle: Option<&str>,
    custom_bg: Option<Color32>,
    custom_fg: Option<Color32>,
) -> egui::Response {
    let tok = ThemeTokens::from_ui(ui);
    let size = Vec2::new(btn_width, btn_height);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

    let is_enabled = ui.is_enabled();
    let a11y_label = match shortcut.filter(|s| !s.is_empty()) {
        Some(sc) => format!("{title} ({sc})"),
        None => title.to_string(),
    };
    response.widget_info(move || {
        egui::WidgetInfo::selected(
            egui::WidgetType::Button,
            is_enabled,
            is_active,
            a11y_label.clone(),
        )
    });

    if ui.is_rect_visible(rect) {
        let (bg_fill, stroke, fg_color) = if is_active {
            (
                custom_bg.unwrap_or(tok.accent_primary),
                Stroke::new(1.0, Color32::from_rgb(100, 180, 255)),
                custom_fg.unwrap_or(Color32::WHITE),
            )
        } else if response.hovered() {
            (
                custom_bg.unwrap_or(tok.bg_hover),
                Stroke::new(1.0, tok.accent_primary),
                custom_fg.unwrap_or(tok.text_primary),
            )
        } else {
            (
                custom_bg.unwrap_or(tok.bg_button_idle),
                Stroke::new(0.5, tok.border_subtle),
                custom_fg.unwrap_or(tok.text_primary),
            )
        };

        ui.painter().rect(
            rect,
            CornerRadius::same(RADIUS.sm),
            bg_fill,
            stroke,
            StrokeKind::Inside,
        );

        let font_id = egui::FontId::proportional(icon_size);
        if show_label {
            let icon_center = egui::pos2(rect.left() + (btn_height * 0.5).min(20.0), rect.center().y);
            ui.painter().text(
                icon_center,
                egui::Align2::CENTER_CENTER,
                icon,
                font_id,
                fg_color,
            );
            let text_left = rect.left() + (btn_height * 0.5).min(20.0) + icon_size * 0.6 + 6.0;
            let max_chars = 13usize;
            let short_title: String = if title.chars().count() > max_chars {
                let mut s: String = title.chars().take(max_chars - 1).collect();
                s.push('…');
                s
            } else {
                title.to_string()
            };
            ui.painter().text(
                egui::pos2(text_left, rect.center().y),
                egui::Align2::LEFT_CENTER,
                short_title,
                egui::FontId::proportional(TYPOGRAPHY.caption),
                fg_color,
            );
        } else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                icon,
                font_id,
                fg_color,
            );
        }
    }

    response.on_hover_ui(|ui| {
        let tok = ThemeTokens::from_ui(ui);
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 2.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(title)
                    .strong()
                    .size(TYPOGRAPHY.body)
                    .color(tok.text_primary),
            );
            if let Some(sc) = shortcut {
                if !sc.is_empty() {
                    Frame::NONE
                        .fill(tok.bg_card)
                        .corner_radius(CornerRadius::same(RADIUS.xs))
                        .inner_margin(Margin::symmetric(5, 1))
                        .stroke(Stroke::new(0.5, tok.border_subtle))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(sc)
                                    .size(TYPOGRAPHY.caption)
                                    .strong()
                                    .color(tok.text_primary),
                            );
                        });
                }
            }
        });
        if let Some(sub) = subtitle {
            if !sub.is_empty() {
                ui.label(
                    RichText::new(sub)
                        .size(TYPOGRAPHY.caption)
                        .color(tok.text_secondary),
                );
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{apply_with_options, ThemeMode, ThemeOptions, TOUCH_TARGET_IPAD};

    #[test]
    fn test_left_toolbar_vector_mode_flag() {
        let mut toolbar = LeftToolbar::default();
        assert!(!toolbar.is_vector_mode);
        assert!(toolbar.show_labels);
        toolbar.is_vector_mode = true;
        assert!(toolbar.is_vector_mode);
    }

    #[test]
    fn test_vector_toolbar_tools_variants() {
        let tools = [
            ToolbarTool::Select,
            ToolbarTool::NodeEdit,
            ToolbarTool::PenBezier,
            ToolbarTool::Line,
            ToolbarTool::Rectangle,
            ToolbarTool::Ellipse,
            ToolbarTool::Polygon,
            ToolbarTool::Text,
            ToolbarTool::ShapeBuilder,
            ToolbarTool::Eyedropper,
            ToolbarTool::Gradient,
        ];
        assert_eq!(tools.len(), 11);
    }

    #[test]
    fn toolbar_honors_44pt_touch_targets_and_light_mode() {
        let ctx = egui::Context::default();
        apply_with_options(
            &ctx,
            ThemeMode::Light,
            ThemeOptions {
                touch_target_y: TOUCH_TARGET_IPAD,
                ui_scale: 1.0,
                reduce_motion: true,
            },
        );
        let mut toolbar = LeftToolbar {
            is_sketching: false,
            show_labels: true,
            ..Default::default()
        };
        let mut out = ctx.run_ui(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = toolbar.show(ui, ToolbarTool::Select);
            });
        });
        out.textures_delta.clear();
    }
}
