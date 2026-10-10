//! Komponen Kanvas Interaktif Lembar Kerja Gambar Teknik 2D (Engineering Drawing Sheet).
//!
//! Menyediakan kanvas gambar 2D presisi (kertas putih standar A4/A3 dengan bingkai teknik ISO),
//! kontrol skala gambar, tombol toggle garis tampak & tersembunyi, editor kepala gambar (title block),
//! serta tombol ekspor langsung ke PDF Vektor dan DXF CAD.

use ducad_io::drawing::scene::{build_scene, dimension_items, Anchor, Dash, Item};
use ducad_io::drawing::{
    format_scale_ratio, BomItem, DimStyle, DrawingSheet, PaperSize, TextAnnotation, TitleBlockInfo,
};
use ducad_kernel::{HlrLineKind, ProjectedViewKind};
use egui::containers::menu::MenuConfig;
use egui::{
    vec2, Align2, Color32, CornerRadius, FontId, Frame, Margin, Pos2, Rect, RichText, Sense,
    Stroke, Ui, Vec2,
};
use egui_icons::icons::{
    ICON_ADJUST, ICON_AUTORENEW, ICON_CLOSE, ICON_HORIZONTAL_SPLIT, ICON_SWAP_HORIZ, ICON_VIEW_IN_AR,
    ICON_WARNING, ICON_CONTENT_CUT, ICON_DOWNLOAD, ICON_EDIT_NOTE, ICON_FIT_SCREEN,
    ICON_GRID_VIEW, ICON_LAYERS, ICON_OPEN_WITH, ICON_PICTURE_AS_PDF, ICON_REFRESH, ICON_SEARCH, ICON_SQUARE_FOOT, ICON_STRAIGHTEN, ICON_TABLE_CHART,
    ICON_TEXTURE,
};

use crate::theme::{glass_frame, ACCENT_BLUE, BORDER_SUBTLE, TEXT_PRIMARY, TEXT_SECONDARY};

/// Tombol ikon kompak untuk header, dengan kartu tooltip hover berisi
/// title + shortcut opsional + subtitle (sama persis dengan top_bar.rs).
#[allow(clippy::too_many_arguments)]
fn header_icon_btn(
    ui: &mut Ui,
    icon: &str,
    active: bool,
    title: &str,
    shortcut: Option<&str>,
    subtitle: Option<&str>,
    active_bg: Option<Color32>,
    active_fg: Option<Color32>,
) -> egui::Response {
    let (bg, icon_color) = if active {
        (
            active_bg.unwrap_or(ACCENT_BLUE),
            active_fg.unwrap_or(Color32::WHITE),
        )
    } else {
        (Color32::TRANSPARENT, active_fg.unwrap_or(TEXT_PRIMARY))
    };

    let btn = egui::Button::new(RichText::new(icon).size(14.0).color(icon_color))
        .fill(bg)
        .corner_radius(CornerRadius::same(5))
        .min_size(Vec2::new(24.0, 22.0));
    let response = ui.add(btn);

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

/// Aksi / Event yang dihasilkan oleh DrawingSheetView ke aplikasi utama.
#[derive(Debug, Clone)]
pub enum DrawingSheetEvent {
    ExportPdf,
    ExportDxf,
    ExportSvg,
    Close,
    /// Tambah tampak potongan: garis potong (≥ 2 titik) di koordinat model 2D
    /// tampak induk. Aplikasi yang menghitung potongannya (butuh kernel).
    AddSection {
        parent: ProjectedViewKind,
        points: Vec<[f32; 2]>,
    },
    /// Balik arah pandang potongan berlabel ini.
    FlipSection(char),
    /// Sisipkan render 3D berbayang dari kamera viewport saat ini.
    InsertShaded,
    /// Bangun ulang lembar dari geometri terkini (lembar kedaluwarsa).
    Refresh,
}

/// Field teks pada Kepala Gambar (Title Block ISO) yang dapat diedit langsung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleBlockFieldId {
    CompanyName,
    ProjectTitle,
    DrawingNumber,
    Revision,
    DrawnBy,
    Date,
    Scale,
    SheetNumber,
    Material,
    Units,
}

impl TitleBlockFieldId {
    pub const ALL: [TitleBlockFieldId; 10] = [
        TitleBlockFieldId::CompanyName,
        TitleBlockFieldId::ProjectTitle,
        TitleBlockFieldId::DrawingNumber,
        TitleBlockFieldId::Revision,
        TitleBlockFieldId::DrawnBy,
        TitleBlockFieldId::Date,
        TitleBlockFieldId::Scale,
        TitleBlockFieldId::SheetNumber,
        TitleBlockFieldId::Material,
        TitleBlockFieldId::Units,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TitleBlockFieldId::CompanyName => "Perusahaan",
            TitleBlockFieldId::ProjectTitle => "Judul Gambar",
            TitleBlockFieldId::DrawingNumber => "No. Gambar",
            TitleBlockFieldId::Revision => "Revisi",
            TitleBlockFieldId::DrawnBy => "Digambar (Drafter)",
            TitleBlockFieldId::Date => "Tanggal",
            TitleBlockFieldId::Scale => "Skala",
            TitleBlockFieldId::SheetNumber => "Lembar",
            TitleBlockFieldId::Material => "Material",
            TitleBlockFieldId::Units => "Satuan & Toleransi",
        }
    }

    pub fn get_mut_str(self, info: &mut TitleBlockInfo) -> &mut String {
        match self {
            TitleBlockFieldId::CompanyName => &mut info.company_name,
            TitleBlockFieldId::ProjectTitle => &mut info.project_title,
            TitleBlockFieldId::DrawingNumber => &mut info.drawing_number,
            TitleBlockFieldId::Revision => &mut info.revision,
            TitleBlockFieldId::DrawnBy => &mut info.drawn_by,
            TitleBlockFieldId::Date => &mut info.date,
            TitleBlockFieldId::Scale => &mut info.scale,
            TitleBlockFieldId::SheetNumber => &mut info.sheet_number,
            TitleBlockFieldId::Material => &mut info.material,
            TitleBlockFieldId::Units => &mut info.units,
        }
    }
}

/// Field kolom pada baris data Tabel BOM yang dapat diedit secara in-place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BomCellField {
    PartName,
    Quantity,
    Material,
    Description,
}

/// Target elemen teks yang sedang aktif diedit secara live in-place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTextTarget {
    TitleBlock(TitleBlockFieldId),
    CustomText(usize),
    BomTitle,
    BomCell(usize, BomCellField),
}

/// Menghitung koordinat batas persegi (bounding box) field etiket dalam mm pada kertas.
fn title_block_field_rect_mm(tb: [f32; 4], field: TitleBlockFieldId) -> [f32; 4] {
    match field {
        TitleBlockFieldId::CompanyName => [tb[0] + 2.0, tb[1] + 34.0, tb[0] + 93.0, tb[1] + 44.0],
        TitleBlockFieldId::ProjectTitle => [tb[0] + 2.0, tb[1] + 19.0, tb[0] + 83.0, tb[1] + 26.5],
        TitleBlockFieldId::DrawingNumber => [tb[0] + 86.5, tb[1] + 19.0, tb[0] + 122.5, tb[1] + 26.5],
        TitleBlockFieldId::Revision => [tb[0] + 124.5, tb[1] + 19.0, tb[0] + 138.5, tb[1] + 26.5],
        TitleBlockFieldId::DrawnBy => [tb[0] + 2.0, tb[1] + 9.5, tb[0] + 43.5, tb[1] + 14.5],
        TitleBlockFieldId::Date => [tb[0] + 46.5, tb[1] + 9.5, tb[0] + 88.5, tb[1] + 14.5],
        TitleBlockFieldId::Scale => [tb[0] + 91.5, tb[1] + 9.5, tb[0] + 113.5, tb[1] + 14.5],
        TitleBlockFieldId::SheetNumber => [tb[0] + 116.5, tb[1] + 9.5, tb[0] + 138.5, tb[1] + 14.5],
        TitleBlockFieldId::Material => [tb[0] + 2.0, tb[1] + 1.0, tb[0] + 88.5, tb[1] + 6.0],
        TitleBlockFieldId::Units => [tb[0] + 91.5, tb[1] + 1.0, tb[0] + 138.5, tb[1] + 6.0],
    }
}

/// Mode dimensi kustom manual pada lembar kerja gambar teknik.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ManualDimensionMode {
    #[default]
    Linear,
    Diameter,
    Radius,
    Angle,
}

impl ManualDimensionMode {
    pub const ALL: [ManualDimensionMode; 4] = [
        ManualDimensionMode::Linear,
        ManualDimensionMode::Diameter,
        ManualDimensionMode::Radius,
        ManualDimensionMode::Angle,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ManualDimensionMode::Linear => "Linear",
            ManualDimensionMode::Diameter => "Diameter (Ø)",
            ManualDimensionMode::Radius => "Radius (R)",
            ManualDimensionMode::Angle => "Angle (∠)",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            ManualDimensionMode::Linear => ICON_STRAIGHTEN.codepoint,
            ManualDimensionMode::Diameter => "Ø",
            ManualDimensionMode::Radius => "R",
            ManualDimensionMode::Angle => "∠",
        }
    }
}

/// Target anotasi dimensi (otomatis atau manual kustom).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DimensionTarget {
    Auto(usize),
    Manual(usize),
}

/// State persisten untuk tampilan Lembar Kerja Gambar Teknik.
pub struct DrawingSheetViewState {
    pub is_open: bool,
    pub pan_offset: Vec2,
    pub zoom: f32,
    pub text_tool_active: bool,
    pub active_text_edit: Option<ActiveTextTarget>,
    pub selected_text_idx: Option<usize>,
    pub dragging_text_idx: Option<usize>,
    pub hovered_text_idx: Option<usize>,
    pub hovered_text_delete: Option<usize>,
    pub hovered_tb_field: Option<TitleBlockFieldId>,
    pub dragging_view: Option<ProjectedViewKind>,
    pub hovered_view: Option<ProjectedViewKind>,
    pub dragging_dim: Option<DimensionTarget>,
    pub hovered_dim: Option<DimensionTarget>,
    pub selected_dim: Option<DimensionTarget>,
    pub hovered_dim_delete: Option<DimensionTarget>,
    pub measure_tool_active: bool,
    pub dimension_mode: ManualDimensionMode,
    pub measure_points: Vec<[f32; 2]>,
    pub measure_first_pt: Option<[f32; 2]>,
    pub detail_tool_active: bool,
    pub dragging_detail_label: Option<char>,
    pub hovered_detail_label: Option<char>,
    pub hovered_detail_delete: Option<char>,
    pub selected_detail_label: Option<char>,
    pub detail_scale_multiplier: f32,
    pub detail_radius_mm: f32,
    pub balloon_tool_active: bool,
    pub dragging_balloon_id: Option<u32>,
    pub dragging_balloon_target_id: Option<u32>,
    pub hovered_balloon_id: Option<u32>,
    pub hovered_balloon_target_id: Option<u32>,
    pub hovered_balloon_delete: Option<u32>,
    pub selected_balloon_id: Option<u32>,
    pub hovered_bom_row: Option<usize>,
    pub hovered_bom_cell: Option<(usize, BomCellField)>,
    pub hovered_bom_delete_row: Option<usize>,
    pub hovered_bom_add_row: bool,
    pub hovered_bom_title: bool,
    pub dragging_bom_table: bool,
    /// Alat Section: klik titik garis potong pada tampak induk.
    pub section_tool_active: bool,
    pub section_parent: Option<ProjectedViewKind>,
    /// Titik garis potong yang sudah diklik (koordinat model 2D tampak induk).
    pub section_points: Vec<[f32; 2]>,
    pub hovered_section: Option<char>,
    pub hovered_section_delete: Option<char>,
    pub hovered_section_flip: Option<char>,
    pub hovered_shaded: Option<usize>,
    pub dragging_shaded: Option<usize>,
    /// Tampak yang diklik kanan (menu skala).
    pub menu_view: Option<ProjectedViewKind>,
    pub menu_custom_scale: f32,
    /// Tekstur render berbayang: (sidik jari piksel, tekstur).
    pub shaded_textures: Vec<(u64, egui::TextureHandle)>,
}

impl Default for DrawingSheetViewState {
    fn default() -> Self {
        Self {
            is_open: false,
            pan_offset: Vec2::ZERO,
            zoom: 1.0,
            text_tool_active: false,
            active_text_edit: None,
            selected_text_idx: None,
            dragging_text_idx: None,
            hovered_text_idx: None,
            hovered_text_delete: None,
            hovered_tb_field: None,
            dragging_view: None,
            hovered_view: None,
            dragging_dim: None,
            hovered_dim: None,
            selected_dim: None,
            hovered_dim_delete: None,
            measure_tool_active: false,
            dimension_mode: ManualDimensionMode::Linear,
            measure_points: Vec::new(),
            measure_first_pt: None,
            detail_tool_active: false,
            dragging_detail_label: None,
            hovered_detail_label: None,
            hovered_detail_delete: None,
            selected_detail_label: None,
            detail_scale_multiplier: 2.0,
            detail_radius_mm: 15.0,
            balloon_tool_active: false,
            dragging_balloon_id: None,
            dragging_balloon_target_id: None,
            hovered_balloon_id: None,
            hovered_balloon_target_id: None,
            hovered_balloon_delete: None,
            selected_balloon_id: None,
            hovered_bom_row: None,
            hovered_bom_cell: None,
            hovered_bom_delete_row: None,
            hovered_bom_add_row: false,
            hovered_bom_title: false,
            dragging_bom_table: false,
            section_tool_active: false,
            section_parent: None,
            section_points: Vec::new(),
            hovered_section: None,
            hovered_section_delete: None,
            hovered_section_flip: None,
            hovered_shaded: None,
            dragging_shaded: None,
            menu_view: None,
            menu_custom_scale: 0.5,
            shaded_textures: Vec::new(),
        }
    }
}

pub struct DrawingSheetView;

impl DrawingSheetView {
    /// Render antarmuka lembar kerja gambar teknik 2D lengkap di layar penuh.
    pub fn show(
        ui: &mut Ui,
        state: &mut DrawingSheetViewState,
        sheet: &mut DrawingSheet,
    ) -> Option<DrawingSheetEvent> {
        let mut event = None;

        let total_rect = ui.available_rect_before_wrap();

        // 1. Gambar latar belakang gelap CAD / Drafting Board
        ui.painter().rect_filled(
            total_rect,
            CornerRadius::ZERO,
            Color32::from_rgb(18, 20, 26),
        );

        // 2. Dimensi Top Bar & Floating Controls (Margin simetris: kiri, kanan, dan atas sama 16px)
        let margin_side = 16.0;
        let margin_top = 16.0;
        let topbar_x = total_rect.min.x + margin_side;
        let topbar_w = (total_rect.width() - (margin_side * 2.0)).max(200.0);
        let topbar_rect = Rect::from_min_size(
            Pos2::new(topbar_x, total_rect.min.y + margin_top),
            Vec2::new(topbar_w, 30.0),
        );

        let zoom_controls_size = vec2(112.0, 32.0);
        let zoom_controls_pos = Pos2::new(total_rect.max.x - 128.0, total_rect.max.y - 48.0);
        let zoom_controls_rect = Rect::from_min_size(zoom_controls_pos, zoom_controls_size);

        let canvas_rect = total_rect;

        // 3. Kanvas Interaktif Kertas Gambar Teknik (Background Sensor dialokasikan SEBELUM floating UI)
        let response = ui.allocate_rect(canvas_rect, Sense::click_and_drag());

        if state.zoom <= 0.05 {
            state.zoom = calculate_fit_zoom(canvas_rect, sheet.paper_size);
        }

        let center_pos = canvas_rect.center() + state.pan_offset;
        let (pw_mm, ph_mm) = sheet.paper_size.dimensions_mm();
        let zoom = state.zoom;

        let sheet_w_px = pw_mm * zoom;
        let sheet_h_px = ph_mm * zoom;
        let sheet_min = Pos2::new(
            center_pos.x - sheet_w_px * 0.5,
            center_pos.y - sheet_h_px * 0.5,
        );
        let sheet_max = Pos2::new(
            center_pos.x + sheet_w_px * 0.5,
            center_pos.y + sheet_h_px * 0.5,
        );

        let screen_to_mm = |p: Pos2| -> [f32; 2] {
            [
                (p.x - sheet_min.x) / zoom,
                (sheet_max.y - p.y) / zoom,
            ]
        };

        let mm_to_screen = |x_mm: f32, y_mm: f32| -> Pos2 {
            Pos2::new(
                sheet_min.x + x_mm * zoom,
                sheet_max.y - y_mm * zoom,
            )
        };

        // Kumpulkan titik snap (ujung garis, titik tengah, titik pusat & kuadran lingkaran/busur) dari seluruh tampak
        let mut snap_points_mm: Vec<[f32; 2]> = Vec::new();
        for plc in &sheet.view_placements {
            if !plc.visible {
                continue;
            }
            let view = sheet.drawing.view_by_kind(plc.kind);
            let s = plc.scale;
            let v_center = view.center_2d();
            let cx = plc.center_mm[0];
            let cy = plc.center_mm[1];

            for seg in &view.segments {
                if seg.kind == HlrLineKind::Visible || seg.kind == HlrLineKind::Silhouette {
                    let p_start = [
                        cx + (seg.start[0] - v_center[0]) * s,
                        cy + (seg.start[1] - v_center[1]) * s,
                    ];
                    let p_end = [
                        cx + (seg.end[0] - v_center[0]) * s,
                        cy + (seg.end[1] - v_center[1]) * s,
                    ];
                    let p_mid = [
                        (p_start[0] + p_end[0]) * 0.5,
                        (p_start[1] + p_end[1]) * 0.5,
                    ];
                    snap_points_mm.push(p_start);
                    snap_points_mm.push(p_end);
                    snap_points_mm.push(p_mid);
                }
            }
            for feat in &view.features {
                match feat {
                    ducad_kernel::HlrGeometricFeature::Circle { center, radius, .. }
                    | ducad_kernel::HlrGeometricFeature::Arc { center, radius, .. } => {
                        let c_x = cx + (center[0] - v_center[0]) * s;
                        let c_y = cy + (center[1] - v_center[1]) * s;
                        let r_s = *radius * s;
                        snap_points_mm.push([c_x, c_y]);
                        snap_points_mm.push([c_x + r_s, c_y]);
                        snap_points_mm.push([c_x - r_s, c_y]);
                        snap_points_mm.push([c_x, c_y + r_s]);
                        snap_points_mm.push([c_x, c_y - r_s]);
                    }
                    ducad_kernel::HlrGeometricFeature::Ellipse { center, radius_x, radius_y, .. } => {
                        let c_x = cx + (center[0] - v_center[0]) * s;
                        let c_y = cy + (center[1] - v_center[1]) * s;
                        snap_points_mm.push([c_x, c_y]);
                        snap_points_mm.push([c_x + *radius_x * s, c_y]);
                        snap_points_mm.push([c_x - *radius_x * s, c_y]);
                        snap_points_mm.push([c_x, c_y + *radius_y * s]);
                        snap_points_mm.push([c_x, c_y - *radius_y * s]);
                    }
                    _ => {}
                }
            }
        }

        let cursor_pos = ui.input(|i| i.pointer.hover_pos());
        let is_over_ui = cursor_pos.is_some_and(|p| topbar_rect.contains(p) || zoom_controls_rect.contains(p));

        let mut hovered_view_kind = None;
        let mut hovered_dim = None;
        let mut hovered_dim_delete = None;
        let mut hovered_tb_field = None;
        let mut hovered_text_idx = None;
        let mut hovered_text_delete = None;
        let mut hovered_detail_label = None;
        let mut hovered_detail_delete = None;
        let mut hovered_balloon_id = None;
        let mut hovered_balloon_target_id = None;
        let mut hovered_balloon_delete = None;
        let mut hovered_bom_row = None;
        let mut hovered_bom_cell = None;
        let mut hovered_bom_delete_row = None;
        let mut hovered_bom_add_row = false;
        let mut hovered_bom_title = false;
        let mut active_snap_pt_mm = None;
        let mut hovered_section = None;
        let mut hovered_section_delete = None;
        let mut hovered_section_flip = None;
        let mut hovered_shaded = None;

        let tb = sheet.title_block_rect_mm();

        if let Some(c_pos) = cursor_pos {
            if !is_over_ui && canvas_rect.contains(c_pos) {
                let cursor_mm = screen_to_mm(c_pos);

                // A. Title Block Fields Hit Test (Edit Teks Langsung / In-place)
                for field in TitleBlockFieldId::ALL {
                    let f_rect_mm = title_block_field_rect_mm(tb, field);
                    let p_bl = mm_to_screen(f_rect_mm[0], f_rect_mm[1]);
                    let p_tr = mm_to_screen(f_rect_mm[2], f_rect_mm[3]);
                    let f_rect = Rect::from_two_pos(p_bl, p_tr);
                    if f_rect.contains(c_pos) {
                        hovered_tb_field = Some(field);
                        break;
                    }
                }

                // A2. Tabel BOM (Bill of Materials) Hit Test
                if sheet.show_bom_table && !sheet.bom_table.items.is_empty() && hovered_tb_field.is_none() {
                    let bom_tb = sheet.bom_table_rect_mm();
                    if cursor_mm[0] >= bom_tb[0] && cursor_mm[0] <= bom_tb[2] && cursor_mm[1] >= bom_tb[1] && cursor_mm[1] <= bom_tb[3] {
                        let title_h = sheet.bom_title_height_mm();
                        let header_h = sheet.bom_header_height_mm();
                        let row_h = sheet.bom_row_height_mm();
                        let y_top = bom_tb[3];

                        if cursor_mm[1] >= y_top - title_h {
                            hovered_bom_title = true;
                            if cursor_mm[0] >= bom_tb[2] - 25.0 {
                                hovered_bom_add_row = true;
                            }
                        } else if cursor_mm[1] < y_top - title_h - header_h {
                            let r_idx = ((y_top - title_h - header_h - cursor_mm[1]) / row_h) as usize;
                            if r_idx < sheet.bom_table.items.len() {
                                hovered_bom_row = Some(r_idx);
                                let col_w = sheet.bom_column_widths_mm();
                                let mut cur_col_x = bom_tb[0];
                                for (c_idx, &cw) in col_w.iter().enumerate() {
                                    if cursor_mm[0] >= cur_col_x && cursor_mm[0] < cur_col_x + cw {
                                        match c_idx {
                                            1 => hovered_bom_cell = Some((r_idx, BomCellField::PartName)),
                                            2 => hovered_bom_cell = Some((r_idx, BomCellField::Quantity)),
                                            3 => hovered_bom_cell = Some((r_idx, BomCellField::Material)),
                                            4 => hovered_bom_cell = Some((r_idx, BomCellField::Description)),
                                            _ => {}
                                        }
                                        break;
                                    }
                                    cur_col_x += cw;
                                }
                                if cursor_mm[0] >= bom_tb[2] - 8.0 {
                                    hovered_bom_delete_row = Some(r_idx);
                                }
                            }
                        }
                    }
                }

                // A3. Callout Balloons Hit Test
                if sheet.show_balloons && hovered_tb_field.is_none() && hovered_bom_row.is_none() {
                    for balloon in &sheet.balloons {
                        let d_target = (cursor_mm[0] - balloon.target_point[0]).hypot(cursor_mm[1] - balloon.target_point[1]);
                        if d_target <= 5.0 {
                            hovered_balloon_target_id = Some(balloon.id);
                            hovered_balloon_id = Some(balloon.id);
                            break;
                        }
                        let d_center = (cursor_mm[0] - balloon.balloon_pos[0]).hypot(cursor_mm[1] - balloon.balloon_pos[1]);
                        let d_del = (cursor_mm[0] - (balloon.balloon_pos[0] + balloon.radius_mm + 2.0)).hypot(cursor_mm[1] - (balloon.balloon_pos[1] + balloon.radius_mm + 2.0));
                        if d_del <= 5.0 {
                            hovered_balloon_delete = Some(balloon.id);
                            hovered_balloon_id = Some(balloon.id);
                            break;
                        } else if d_center <= balloon.radius_mm + 2.5 {
                            hovered_balloon_id = Some(balloon.id);
                            break;
                        }
                    }
                }

                // B. Custom Text Annotations Hit Test (Teks Bebas / Catatan)
                if hovered_tb_field.is_none() && hovered_bom_row.is_none() && hovered_balloon_id.is_none() {
                    for (idx, note) in sheet.custom_texts.iter().enumerate() {
                        let p_top_left = mm_to_screen(note.position[0], note.position[1]);
                        let font_sz = (note.font_size * zoom).clamp(7.0, 24.0);
                        let disp_text = if note.text.is_empty() { "Ketik teks..." } else { &note.text };
                        let text_w = ((disp_text.len().max(8) as f32) * font_sz * 0.6 + 12.0).clamp(40.0, 500.0);
                        let text_rect = Rect::from_min_size(p_top_left - vec2(0.0, font_sz * 1.1), vec2(text_w, font_sz * 1.5));
                        let del_btn_rect = Rect::from_center_size(Pos2::new(text_rect.max.x + 10.0, text_rect.center().y), vec2(18.0, 18.0));

                        if del_btn_rect.contains(c_pos) {
                            hovered_text_delete = Some(idx);
                            hovered_text_idx = Some(idx);
                            break;
                        } else if text_rect.contains(c_pos) {
                            hovered_text_idx = Some(idx);
                            break;
                        }
                    }
                }

                // B2. Detail Callouts Hit Test pada Tampak Acuan
                if hovered_tb_field.is_none() && hovered_bom_row.is_none() && hovered_balloon_id.is_none() && hovered_text_idx.is_none() {
                    for det in &sheet.drawing.detail_views {
                        if let Some(plc) = sheet.view_placements.iter().find(|p| p.kind == det.indicator.parent_view && p.visible) {
                            let view = sheet.drawing.view_by_kind(plc.kind);
                            let v_center = view.center_2d();
                            let s = plc.scale;
                            let cx = plc.center_mm[0] + (det.indicator.center_2d[0] - v_center[0]) * s;
                            let cy = plc.center_mm[1] + (det.indicator.center_2d[1] - v_center[1]) * s;
                            let r = det.indicator.radius_mm * s;
                            let dist_to_center = (cursor_mm[0] - cx).hypot(cursor_mm[1] - cy);

                            let l_x_mm = plc.center_mm[0] + (det.indicator.label_pos[0] - v_center[0]) * s;
                            let l_y_mm = plc.center_mm[1] + (det.indicator.label_pos[1] - v_center[1]) * s;
                            let p_lbl = mm_to_screen(l_x_mm, l_y_mm);
                            let p_shoulder = Pos2::new(p_lbl.x + 14.0 * zoom.clamp(0.8, 1.5), p_lbl.y);
                            let del_rect = Rect::from_center_size(Pos2::new(p_shoulder.x + 8.0, p_shoulder.y), vec2(18.0, 18.0));

                            if del_rect.contains(c_pos) {
                                hovered_detail_delete = Some(det.indicator.label);
                                hovered_detail_label = Some(det.indicator.label);
                                break;
                            } else if dist_to_center <= r + 4.0 || (c_pos.x - p_lbl.x).hypot(c_pos.y - p_lbl.y) <= 24.0 {
                                hovered_detail_label = Some(det.indicator.label);
                                break;
                            }
                        }
                    }
                }

                // C. Snap point detection (untuk tambah ukuran baru)
                if state.measure_tool_active || state.section_tool_active {
                    let snap_threshold_mm = 14.0 / zoom;
                    let mut closest_dist = snap_threshold_mm;
                    for sp in &snap_points_mm {
                        let d = (sp[0] - cursor_mm[0]).hypot(sp[1] - cursor_mm[1]);
                        if d < closest_dist {
                            closest_dist = d;
                            active_snap_pt_mm = Some(*sp);
                        }
                    }
                }

                // D. Dimension hit test (untuk geser posisi ukuran dan hapus satu per satu: Otomatis & Manual)
                if hovered_tb_field.is_none() && hovered_bom_row.is_none() && hovered_balloon_id.is_none() && hovered_text_idx.is_none() && hovered_detail_label.is_none() {
                    let mut dim_list: Vec<(DimensionTarget, &ducad_io::drawing::DimensionAnnotation)> = Vec::new();
                    if sheet.show_dimensions {
                        for (idx, dim) in sheet.auto_dimensions.iter().enumerate() {
                            dim_list.push((DimensionTarget::Auto(idx), dim));
                        }
                    }
                    for (idx, dim) in sheet.manual_dimensions.iter().enumerate() {
                        dim_list.push((DimensionTarget::Manual(idx), dim));
                    }

                    for (target, dim) in dim_list {
                        // Kotak teks yang sama dengan yang digambar (drawing::scene).
                        let tb = dim.text_box();
                        let text_hit_rect = Rect::from_two_pos(mm_to_screen(tb[0], tb[1]), mm_to_screen(tb[2], tb[3])).expand(5.0);
                        let line_hit_rect = match dim.effective_style() {
                            DimStyle::Linear if dim.is_vertical => {
                                let x = mm_to_screen(dim.line_pos[0], 0.0).x;
                                let (ya, yb) = (mm_to_screen(0.0, dim.start[1]).y, mm_to_screen(0.0, dim.end[1]).y);
                                Rect::from_min_max(Pos2::new(x - 6.0, ya.min(yb)), Pos2::new(x + 6.0, ya.max(yb)))
                            }
                            DimStyle::Linear => {
                                let y = mm_to_screen(0.0, dim.line_pos[1]).y;
                                let (xa, xb) = (mm_to_screen(dim.start[0], 0.0).x, mm_to_screen(dim.end[0], 0.0).x);
                                Rect::from_min_max(Pos2::new(xa.min(xb), y - 6.0), Pos2::new(xa.max(xb), y + 6.0))
                            }
                            _ => Rect::from_center_size(mm_to_screen(dim.line_pos[0], dim.line_pos[1]), vec2(16.0, 16.0)),
                        };
                        let del_btn_rect = Rect::from_center_size(
                            Pos2::new(text_hit_rect.max.x + 6.0, text_hit_rect.center().y),
                            vec2(18.0, 18.0),
                        );

                        if del_btn_rect.contains(c_pos) {
                            hovered_dim_delete = Some(target);
                            hovered_dim = Some(target);
                            break;
                        } else if text_hit_rect.contains(c_pos) || line_hit_rect.contains(c_pos) {
                            hovered_dim = Some(target);
                            break;
                        }
                    }
                }

                // D2. Label garis potong: balik arah / hapus.
                if hovered_dim.is_none() && hovered_text_idx.is_none() && !state.section_tool_active {
                    'sections: for section in &sheet.drawing.sections {
                        let Some(plc) = sheet.view_placements.iter().find(|p| p.kind == section.parent && p.visible) else {
                            continue;
                        };
                        let Some(label) = section.label.chars().next() else {
                            continue;
                        };
                        for lp in sheet.cutting_label_positions_mm(plc, &section.cutting_line) {
                            let p_lbl = mm_to_screen(lp[0], lp[1]);
                            let flip_c = p_lbl + vec2(20.0, -4.0);
                            let del_c = p_lbl + vec2(40.0, -4.0);
                            let near = (c_pos - p_lbl).length() <= 14.0;
                            let on_flip = (c_pos - flip_c).length() <= 9.0;
                            let on_del = (c_pos - del_c).length() <= 9.0;
                            // Tombol hanya aktif selama label ini sedang disorot.
                            let was = state.hovered_section == Some(label);
                            if near || (was && (on_flip || on_del)) {
                                hovered_section = Some(label);
                                if was && on_flip {
                                    hovered_section_flip = Some(label);
                                } else if was && on_del {
                                    hovered_section_delete = Some(label);
                                }
                                break 'sections;
                            }
                        }
                    }
                }

                // D3. Render berbayang (bisa digeser seperti tampak).
                if hovered_dim.is_none() && hovered_section.is_none() && hovered_text_idx.is_none() {
                    for (i, view) in sheet.shaded.iter().enumerate() {
                        let r = view.rect_mm();
                        if view.visible && cursor_mm[0] >= r[0] && cursor_mm[0] <= r[2] && cursor_mm[1] >= r[1] && cursor_mm[1] <= r[3] {
                            hovered_shaded = Some(i);
                            break;
                        }
                    }
                }

                // E. View hit test (jika tidak sedang hover teks, detail, atau dimensi)
                if hovered_tb_field.is_none() && hovered_bom_row.is_none() && hovered_balloon_id.is_none() && hovered_text_idx.is_none() && hovered_detail_label.is_none() && hovered_dim.is_none() && hovered_section.is_none() && hovered_shaded.is_none() {
                    for plc in &sheet.view_placements {
                        if !plc.visible {
                            continue;
                        }
                        let view = sheet.drawing.view_by_kind(plc.kind);
                        let sz = view.size_2d();
                        let s = plc.scale;
                        let half_w = (sz[0] * s * 0.5 + 6.0).max(12.0);
                        let half_h = (sz[1] * s * 0.5 + 11.5).max(12.0);
                        let cx = plc.center_mm[0];
                        let cy = plc.center_mm[1];
                        if cursor_mm[0] >= cx - half_w
                            && cursor_mm[0] <= cx + half_w
                            && cursor_mm[1] >= cy - half_h
                            && cursor_mm[1] <= cy + half_h
                        {
                            hovered_view_kind = Some(plc.kind);
                            break;
                        }
                    }
                }
            }
        }
        state.hovered_view = hovered_view_kind;
        state.hovered_dim = hovered_dim;
        state.hovered_dim_delete = hovered_dim_delete;
        state.hovered_tb_field = hovered_tb_field;
        state.hovered_text_idx = hovered_text_idx;
        state.hovered_text_delete = hovered_text_delete;
        state.hovered_detail_label = hovered_detail_label;
        state.hovered_detail_delete = hovered_detail_delete;
        state.hovered_balloon_id = hovered_balloon_id;
        state.hovered_balloon_target_id = hovered_balloon_target_id;
        state.hovered_balloon_delete = hovered_balloon_delete;
        state.hovered_bom_row = hovered_bom_row;
        state.hovered_bom_cell = hovered_bom_cell;
        state.hovered_bom_delete_row = hovered_bom_delete_row;
        state.hovered_bom_add_row = hovered_bom_add_row;
        state.hovered_bom_title = hovered_bom_title;
        state.hovered_section = hovered_section;
        state.hovered_section_delete = hovered_section_delete;
        state.hovered_section_flip = hovered_section_flip;
        state.hovered_shaded = hovered_shaded;

        // Interaction Handler
        if !is_over_ui {
            // Pintasan keyboard T (Teks), B (Detail View), M (Dimensi Manual), Escape
            if state.active_text_edit.is_none() {
                if ui.input(|i| i.key_pressed(egui::Key::T)) {
                    state.text_tool_active = !state.text_tool_active;
                    if state.text_tool_active {
                        state.measure_tool_active = false;
                        state.detail_tool_active = false;
                        state.measure_points.clear();
                        state.measure_first_pt = None;
                    }
                }
                if ui.input(|i| i.key_pressed(egui::Key::B)) {
                    state.detail_tool_active = !state.detail_tool_active;
                    if state.detail_tool_active {
                        state.text_tool_active = false;
                        state.measure_tool_active = false;
                        state.measure_points.clear();
                        state.measure_first_pt = None;
                    }
                }
                if ui.input(|i| i.key_pressed(egui::Key::S) && !i.modifiers.command) {
                    state.section_tool_active = !state.section_tool_active;
                    state.section_points.clear();
                    state.section_parent = None;
                    if state.section_tool_active {
                        state.text_tool_active = false;
                        state.detail_tool_active = false;
                        state.measure_tool_active = false;
                        state.measure_points.clear();
                        state.measure_first_pt = None;
                    }
                }
                if ui.input(|i| i.key_pressed(egui::Key::M)) {
                    state.measure_tool_active = !state.measure_tool_active;
                    if state.measure_tool_active {
                        state.text_tool_active = false;
                        state.detail_tool_active = false;
                    }
                    state.measure_points.clear();
                    state.measure_first_pt = None;
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    if state.section_tool_active {
                        if state.section_points.is_empty() {
                            state.section_tool_active = false;
                        }
                        state.section_points.clear();
                        state.section_parent = None;
                    } else if !state.measure_points.is_empty() {
                        state.measure_points.clear();
                        state.measure_first_pt = None;
                    } else if state.measure_tool_active {
                        state.measure_tool_active = false;
                    } else if state.detail_tool_active {
                        state.detail_tool_active = false;
                    } else if state.text_tool_active {
                        state.text_tool_active = false;
                    } else {
                        state.selected_dim = None;
                        state.selected_text_idx = None;
                        state.selected_detail_label = None;
                    }
                }
            }

            // Hapus teks / dimensi / detail view yang sedang dipilih dengan tombol Delete / Backspace
            if state.active_text_edit.is_none() && ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
                if let Some(lbl) = state.selected_detail_label {
                    sheet.remove_detail_view(lbl);
                    state.selected_detail_label = None;
                    state.hovered_detail_label = None;
                    state.hovered_detail_delete = None;
                } else if let Some(t_idx) = state.selected_text_idx {
                    if t_idx < sheet.custom_texts.len() {
                        sheet.custom_texts.remove(t_idx);
                        state.selected_text_idx = None;
                        state.hovered_text_idx = None;
                        state.hovered_text_delete = None;
                    }
                } else if let Some(target) = state.selected_dim {
                    match target {
                        DimensionTarget::Auto(idx) => {
                            if idx < sheet.auto_dimensions.len() {
                                sheet.auto_dimensions.remove(idx);
                            }
                        }
                        DimensionTarget::Manual(idx) => {
                            if idx < sheet.manual_dimensions.len() {
                                sheet.manual_dimensions.remove(idx);
                            }
                        }
                    }
                    state.selected_dim = None;
                    state.hovered_dim = None;
                    state.hovered_dim_delete = None;
                    state.dragging_dim = None;
                }
            }

            if state.section_tool_active {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                if response.clicked() {
                    let click_mm = active_snap_pt_mm.or_else(|| cursor_pos.map(screen_to_mm));
                    if let Some(pt) = click_mm {
                        // Titik pertama menentukan tampak induk (Depan/Atas/Kanan).
                        let parent = state.section_parent.or_else(|| {
                            sheet.view_at(pt).filter(|k| {
                                matches!(k, ProjectedViewKind::Front | ProjectedViewKind::Top | ProjectedViewKind::Right)
                            })
                        });
                        if let Some(parent) = parent {
                            if let Some(mut model) = sheet.paper_to_model(parent, pt) {
                                // Ruas harus mendatar/tegak terhadap titik sebelumnya.
                                if let Some(last) = state.section_points.last() {
                                    if (model[0] - last[0]).abs() >= (model[1] - last[1]).abs() {
                                        model[1] = last[1];
                                    } else {
                                        model[0] = last[0];
                                    }
                                }
                                state.section_parent = Some(parent);
                                state.section_points.push(model);
                                // Shift = lanjutkan (potongan bertingkat); tanpa Shift selesai.
                                let more = ui.input(|i| i.modifiers.shift);
                                if state.section_points.len() >= 2 && !more {
                                    event = Some(DrawingSheetEvent::AddSection {
                                        parent,
                                        points: std::mem::take(&mut state.section_points),
                                    });
                                    state.section_parent = None;
                                    state.section_tool_active = false;
                                }
                            }
                        }
                    }
                }
            } else if state.detail_tool_active {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                if response.clicked() {
                    if let Some(c_pos) = cursor_pos {
                        let click_mm = screen_to_mm(c_pos);
                        for plc in &sheet.view_placements {
                            if !plc.visible {
                                continue;
                            }
                            let view = sheet.drawing.view_by_kind(plc.kind);
                            let sz = view.size_2d();
                            let s = plc.scale;
                            let half_w = sz[0] * s * 0.5;
                            let half_h = sz[1] * s * 0.5;
                            let cx = plc.center_mm[0];
                            let cy = plc.center_mm[1];
                            if click_mm[0] >= cx - half_w
                                && click_mm[0] <= cx + half_w
                                && click_mm[1] >= cy - half_h
                                && click_mm[1] <= cy + half_h
                            {
                                let v_center = view.center_2d();
                                let u0 = (click_mm[0] - cx) / s + v_center[0];
                                let v0 = (click_mm[1] - cy) / s + v_center[1];

                                let mut next_letter = 'B';
                                while sheet.drawing.detail_views.iter().any(|d| d.indicator.label == next_letter) {
                                    next_letter = ((next_letter as u8) + 1) as char;
                                }

                                sheet.add_or_update_detail_view(
                                    plc.kind,
                                    [u0, v0],
                                    state.detail_radius_mm,
                                    state.detail_scale_multiplier,
                                    next_letter,
                                );
                                state.selected_detail_label = Some(next_letter);
                                state.detail_tool_active = false;
                                break;
                            }
                        }
                    }
                }
            } else if state.measure_tool_active {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                if response.clicked() {
                    let click_pt_mm = active_snap_pt_mm.or_else(|| cursor_pos.map(screen_to_mm));
                    if let Some(pt) = click_pt_mm {
                        match state.dimension_mode {
                            ManualDimensionMode::Linear => {
                                if let Some(first_pt) = state.measure_points.first().copied() {
                                    // Asosiatif bila kedua titik di tampak yang sama.
                                    if let Some(dim) = sheet.make_linear_dimension(first_pt, pt) {
                                        sheet.manual_dimensions.push(dim);
                                    }
                                    state.measure_points.clear();
                                    state.measure_first_pt = None;
                                } else {
                                    state.measure_points = vec![pt];
                                    state.measure_first_pt = Some(pt);
                                }
                            }
                            ManualDimensionMode::Diameter | ManualDimensionMode::Radius => {
                                if let Some(center_pt) = state.measure_points.first().copied() {
                                    let radius = state.dimension_mode == ManualDimensionMode::Radius;
                                    if let Some(dim) = sheet.make_radial_dimension(center_pt, pt, radius) {
                                        sheet.manual_dimensions.push(dim);
                                    }
                                    state.measure_points.clear();
                                    state.measure_first_pt = None;
                                } else {
                                    state.measure_points = vec![pt];
                                    state.measure_first_pt = Some(pt);
                                }
                            }
                            ManualDimensionMode::Angle => {
                                if state.measure_points.is_empty() {
                                    state.measure_points.push(pt); // Vertex
                                    state.measure_first_pt = Some(pt);
                                } else if state.measure_points.len() == 1 {
                                    state.measure_points.push(pt); // Leg 1
                                } else if state.measure_points.len() >= 2 {
                                    let p_v = state.measure_points[0];
                                    let p_a1 = state.measure_points[1];
                                    let p_a2 = pt; // Leg 2

                                    let v1 = [p_a1[0] - p_v[0], p_a1[1] - p_v[1]];
                                    let v2 = [p_a2[0] - p_v[0], p_a2[1] - p_v[1]];
                                    let len1 = (v1[0] * v1[0] + v1[1] * v1[1]).sqrt();
                                    let len2 = (v2[0] * v2[0] + v2[1] * v2[1]).sqrt();

                                    if len1 > 1e-3 && len2 > 1e-3 {
                                        let dot = v1[0] * v2[0] + v1[1] * v2[1];
                                        let cos_val = (dot / (len1 * len2)).clamp(-1.0, 1.0);
                                        let deg = cos_val.acos().to_degrees();
                                        sheet.manual_dimensions.push(ducad_io::drawing::DimensionAnnotation {
                                            start: p_v,
                                            end: p_a1,
                                            line_pos: p_a2,
                                            is_vertical: false,
                                            text: format!("{:.1}°", deg),
                                            style: DimStyle::Angle,
                                            ..Default::default()
                                        });
                                    }
                                    state.measure_points.clear();
                                    state.measure_first_pt = None;
                                }
                            }
                        }
                    }
                }
            } else if state.balloon_tool_active {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                if response.clicked() {
                    if let Some(c_pos) = cursor_pos {
                        let click_mm = screen_to_mm(c_pos);
                        let next_item = if sheet.bom_table.items.is_empty() {
                            sheet.balloons.len() + 1
                        } else {
                            (sheet.balloons.len() % sheet.bom_table.items.len()) + 1
                        };
                        let target_pt = click_mm;
                        let balloon_pos = [click_mm[0] + 16.0, click_mm[1] + 12.0];
                        sheet.add_balloon(next_item, target_pt, balloon_pos, ProjectedViewKind::Isometric);
                        state.selected_balloon_id = sheet.balloons.last().map(|b| b.id);
                        state.balloon_tool_active = false;
                    }
                }
            } else {
                // Klik untuk pilih/hapus teks, detail view, balon, BOM, atau dimensi, atau tambah teks baru
                if response.clicked() {
                    if let Some(label) = state.hovered_section_delete {
                        // Hapus potongan beserta tampak dan dimensinya.
                        let kind = ProjectedViewKind::Section(label);
                        sheet.drawing.sections.retain(|s| !s.label.starts_with(label));
                        sheet.view_placements.retain(|p| p.kind != kind);
                        sheet.manual_dimensions.retain(|d| d.view != Some(kind));
                        sheet.generate_auto_dimensions();
                        state.hovered_section = None;
                        state.hovered_section_delete = None;
                    } else if let Some(label) = state.hovered_section_flip {
                        event = Some(DrawingSheetEvent::FlipSection(label));
                    } else if let Some(del_b_id) = state.hovered_balloon_delete {
                        sheet.remove_balloon(del_b_id);
                        state.selected_balloon_id = None;
                        state.hovered_balloon_id = None;
                        state.hovered_balloon_delete = None;
                    } else if let Some(del_r_idx) = state.hovered_bom_delete_row {
                        if del_r_idx < sheet.bom_table.items.len() {
                            sheet.bom_table.items.remove(del_r_idx);
                            state.hovered_bom_delete_row = None;
                            state.hovered_bom_row = None;
                            state.active_text_edit = None;
                        }
                    } else if state.hovered_bom_add_row {
                        let new_no = sheet.bom_table.items.len() + 1;
                        sheet.bom_table.items.push(BomItem {
                            item_number: new_no,
                            part_name: format!("Part {}", new_no),
                            quantity: 1,
                            material: "Aluminium 6061-T6".to_string(),
                            description: String::new(),
                        });
                        let new_idx = sheet.bom_table.items.len() - 1;
                        state.active_text_edit = Some(ActiveTextTarget::BomCell(new_idx, BomCellField::PartName));
                    } else if let Some((r_idx, field)) = state.hovered_bom_cell {
                        state.active_text_edit = Some(ActiveTextTarget::BomCell(r_idx, field));
                        state.hovered_bom_row = Some(r_idx);
                    } else if state.hovered_bom_title {
                        state.active_text_edit = Some(ActiveTextTarget::BomTitle);
                    } else if let Some(b_id) = state.hovered_balloon_id {
                        state.selected_balloon_id = Some(b_id);
                        state.selected_text_idx = None;
                        state.selected_dim = None;
                        state.selected_detail_label = None;
                    } else if let Some(del_det) = state.hovered_detail_delete {
                        sheet.remove_detail_view(del_det);
                        state.selected_detail_label = None;
                        state.hovered_detail_label = None;
                        state.hovered_detail_delete = None;
                    } else if let Some(del_t) = state.hovered_text_delete {
                        if del_t < sheet.custom_texts.len() {
                            sheet.custom_texts.remove(del_t);
                            state.selected_text_idx = None;
                            state.hovered_text_idx = None;
                            state.hovered_text_delete = None;
                            state.active_text_edit = None;
                        }
                    } else if let Some(target) = state.hovered_dim_delete {
                        match target {
                            DimensionTarget::Auto(idx) => {
                                if idx < sheet.auto_dimensions.len() {
                                    sheet.auto_dimensions.remove(idx);
                                }
                            }
                            DimensionTarget::Manual(idx) => {
                                if idx < sheet.manual_dimensions.len() {
                                    sheet.manual_dimensions.remove(idx);
                                }
                            }
                        }
                        state.selected_dim = None;
                        state.hovered_dim = None;
                        state.hovered_dim_delete = None;
                        state.dragging_dim = None;
                    } else if let Some(lbl) = state.hovered_detail_label {
                        state.selected_detail_label = Some(lbl);
                        state.selected_text_idx = None;
                        state.selected_dim = None;
                        state.active_text_edit = None;
                    } else if let Some(field) = state.hovered_tb_field {
                        state.active_text_edit = Some(ActiveTextTarget::TitleBlock(field));
                        state.selected_text_idx = None;
                        state.selected_dim = None;
                        state.selected_detail_label = None;
                    } else if let Some(t_idx) = state.hovered_text_idx {
                        state.active_text_edit = Some(ActiveTextTarget::CustomText(t_idx));
                        state.selected_text_idx = Some(t_idx);
                        state.selected_dim = None;
                        state.selected_detail_label = None;
                    } else if let Some(dim_target) = state.hovered_dim {
                        state.selected_dim = Some(dim_target);
                        state.selected_text_idx = None;
                        state.selected_detail_label = None;
                        state.active_text_edit = None;
                    } else if state.text_tool_active {
                        // Tambah teks anotasi baru pada kertas di posisi klik
                        if let Some(c_pos) = cursor_pos {
                            let click_mm = screen_to_mm(c_pos);
                            sheet.custom_texts.push(TextAnnotation {
                                position: click_mm,
                                text: String::new(),
                                font_size: 3.5,
                            });
                            let new_idx = sheet.custom_texts.len() - 1;
                            state.active_text_edit = Some(ActiveTextTarget::CustomText(new_idx));
                            state.selected_text_idx = Some(new_idx);
                            state.selected_dim = None;
                            state.selected_detail_label = None;
                        }
                    } else {
                        state.selected_dim = None;
                        state.selected_text_idx = None;
                        state.selected_detail_label = None;
                        state.selected_balloon_id = None;
                        state.active_text_edit = None;
                    }
                }

                // Drag and drop geser teks, balon, BOM, detail circle, ukuran, atau tampak
                if response.drag_started_by(egui::PointerButton::Primary)
                    && !ui.input(|i| i.modifiers.alt)
                    && state.hovered_dim_delete.is_none()
                    && state.hovered_text_delete.is_none()
                    && state.hovered_detail_delete.is_none()
                    && state.hovered_balloon_delete.is_none()
                    && state.hovered_bom_delete_row.is_none()
                {
                    if state.hovered_balloon_target_id.is_some() {
                        state.dragging_balloon_target_id = state.hovered_balloon_target_id;
                        state.selected_balloon_id = state.hovered_balloon_id;
                    } else if state.hovered_balloon_id.is_some() {
                        state.dragging_balloon_id = state.hovered_balloon_id;
                        state.selected_balloon_id = state.hovered_balloon_id;
                    } else if (state.hovered_bom_row.is_some() || state.hovered_bom_title) && state.active_text_edit.is_none() {
                        state.dragging_bom_table = true;
                    } else if state.hovered_detail_label.is_some() {
                        state.dragging_detail_label = state.hovered_detail_label;
                        state.selected_detail_label = state.hovered_detail_label;
                    } else if state.hovered_text_idx.is_some() && state.active_text_edit.is_none() {
                        state.dragging_text_idx = state.hovered_text_idx;
                        state.selected_text_idx = state.hovered_text_idx;
                    } else if state.hovered_dim.is_some() {
                        state.dragging_dim = state.hovered_dim;
                        state.selected_dim = state.hovered_dim;
                    } else if state.hovered_shaded.is_some() {
                        state.dragging_shaded = state.hovered_shaded;
                    } else if state.hovered_tb_field.is_none() {
                        state.dragging_view = state.hovered_view;
                    }
                }

                if response.dragged_by(egui::PointerButton::Primary) && !ui.input(|i| i.modifiers.alt) {
                    if let Some(b_id) = state.dragging_balloon_target_id {
                        if let Some(b) = sheet.balloons.iter_mut().find(|b| b.id == b_id) {
                            let delta_x = response.drag_delta().x / zoom;
                            let delta_y = -response.drag_delta().y / zoom;
                            b.target_point[0] += delta_x;
                            b.target_point[1] += delta_y;
                        }
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    } else if let Some(b_id) = state.dragging_balloon_id {
                        if let Some(b) = sheet.balloons.iter_mut().find(|b| b.id == b_id) {
                            let delta_x = response.drag_delta().x / zoom;
                            let delta_y = -response.drag_delta().y / zoom;
                            b.balloon_pos[0] += delta_x;
                            b.balloon_pos[1] += delta_y;
                        }
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    } else if state.dragging_bom_table {
                        let cur_pos = sheet.bom_table.custom_pos_mm.unwrap_or([tb[0], tb[1] + 48.0]);
                        let delta_x = response.drag_delta().x / zoom;
                        let delta_y = -response.drag_delta().y / zoom;
                        sheet.bom_table.custom_pos_mm = Some([cur_pos[0] + delta_x, cur_pos[1] + delta_y]);
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    } else if let Some(lbl) = state.dragging_detail_label {
                        if let Some(det) = sheet.drawing.detail_views.iter().find(|d| d.indicator.label == lbl).cloned() {
                            if let Some(plc) = sheet.view_placements.iter().find(|p| p.kind == det.indicator.parent_view) {
                                let s = plc.scale;
                                let delta_u = response.drag_delta().x / (zoom * s);
                                let delta_v = -response.drag_delta().y / (zoom * s);
                                let new_center = [det.indicator.center_2d[0] + delta_u, det.indicator.center_2d[1] + delta_v];
                                sheet.add_or_update_detail_view(
                                    det.indicator.parent_view,
                                    new_center,
                                    det.indicator.radius_mm,
                                    det.scale_multiplier,
                                    lbl,
                                );
                            }
                        }
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    } else if let Some(t_idx) = state.dragging_text_idx {
                        if let Some(note) = sheet.custom_texts.get_mut(t_idx) {
                            let delta_x = response.drag_delta().x / zoom;
                            let delta_y = -response.drag_delta().y / zoom;
                            note.position[0] += delta_x;
                            note.position[1] += delta_y;
                        }
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    } else if let Some(target) = state.dragging_dim {
                        // Dimensi asosiatif menyimpan hasil geser sebagai
                        // offset/sudut (ikut bertahan saat geometri berubah).
                        if let Some(c_pos) = cursor_pos {
                            let target_mm = screen_to_mm(c_pos);
                            match target {
                                DimensionTarget::Auto(idx) => sheet.move_dimension(true, idx, target_mm),
                                DimensionTarget::Manual(idx) => sheet.move_dimension(false, idx, target_mm),
                            }
                        }
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    } else if let Some(i) = state.dragging_shaded {
                        if let Some(view) = sheet.shaded.get_mut(i) {
                            view.center_mm[0] += response.drag_delta().x / zoom;
                            view.center_mm[1] -= response.drag_delta().y / zoom;
                        }
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    } else if let Some(kind) = state.dragging_view {
                        if let Some(plc) = sheet.view_placements.iter_mut().find(|p| p.kind == kind) {
                            let delta_x = response.drag_delta().x / zoom;
                            let delta_y = -response.drag_delta().y / zoom;
                            plc.center_mm[0] += delta_x;
                            plc.center_mm[1] += delta_y;
                            // Dimensi ikut tampaknya; offset pengguna dipertahankan.
                            sheet.refresh_associative_dimensions();
                        }
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    }
                }

                if response.drag_stopped() {
                    state.dragging_detail_label = None;
                    state.dragging_text_idx = None;
                    state.dragging_dim = None;
                    state.dragging_view = None;
                    state.dragging_shaded = None;
                    state.dragging_balloon_id = None;
                    state.dragging_balloon_target_id = None;
                    state.dragging_bom_table = false;
                }

                // Pan canvas. Saat dua jari di layar, seretan jari pertama
                // (pointer Primary egui) diabaikan: pan diambil dari
                // `translation_delta` gesture multi-touch di bawah.
                let multi_touch = ui.input(|i| i.multi_touch());
                if multi_touch.is_none()
                    && (response.dragged_by(egui::PointerButton::Middle)
                    || (response.dragged_by(egui::PointerButton::Primary) && ui.input(|i| i.modifiers.alt))
                    || (response.dragged_by(egui::PointerButton::Primary)
                        && state.dragging_detail_label.is_none()
                        && state.hovered_detail_label.is_none()
                        && state.dragging_view.is_none()
                        && state.hovered_view.is_none()
                        && state.dragging_dim.is_none()
                        && state.hovered_dim.is_none()
                        && state.dragging_text_idx.is_none()
                        && state.hovered_text_idx.is_none()
                        && state.dragging_balloon_id.is_none()
                        && state.hovered_balloon_id.is_none()
                        && !state.dragging_bom_table
                        && state.hovered_bom_row.is_none()
                        && state.hovered_tb_field.is_none()))
                {
                    state.pan_offset += response.drag_delta();
                }
                if let Some(touch) = multi_touch {
                    state.pan_offset += touch.translation_delta;
                    zoom_about(state, canvas_rect, touch.center_pos, touch.zoom_delta);
                }

                if state.hovered_section_delete.is_some() || state.hovered_section_flip.is_some() || state.hovered_dim_delete.is_some() || state.hovered_text_delete.is_some() || state.hovered_detail_delete.is_some() || state.hovered_balloon_delete.is_some() || state.hovered_bom_delete_row.is_some() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                } else if state.text_tool_active || state.hovered_tb_field.is_some() || state.hovered_text_idx.is_some() || state.hovered_bom_cell.is_some() || state.hovered_bom_title {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
                } else if (state.hovered_detail_label.is_some() && state.dragging_detail_label.is_none())
                    || (state.hovered_dim.is_some() && state.dragging_dim.is_none())
                    || (state.hovered_balloon_id.is_some() && state.dragging_balloon_id.is_none())
                    || (state.hovered_view.is_some() && state.dragging_view.is_none())
                {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                }
            }

            // Zoom di sekitar kursor: pinch trackpad (`zoom_delta`, ctrl+scroll)
            // dan roda mouse. Pinch dua jari di layar sentuh sudah ditangani
            // lewat `multi_touch` di atas.
            if response.hovered() && ui.input(|i| i.multi_touch()).is_none() {
                let anchor = cursor_pos.unwrap_or_else(|| canvas_rect.center());
                let pinch = ui.input(|i| i.zoom_delta());
                if pinch != 1.0 {
                    zoom_about(state, canvas_rect, anchor, pinch);
                } else {
                    let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
                    if scroll_delta.abs() > 0.0 {
                        let zoom_factor = if scroll_delta > 0.0 { 1.1 } else { 0.9 };
                        zoom_about(state, canvas_rect, anchor, zoom_factor);
                    }
                }
            }
        }

        // Menu konteks tampak: skala per tampak (P21.2).
        if response.secondary_clicked() {
            state.menu_view = state.hovered_view;
            if let Some(plc) = state.menu_view.and_then(|k| sheet.view_placements.iter().find(|p| p.kind == k)) {
                state.menu_custom_scale = plc.scale;
            }
        }
        if let Some(kind) = state.menu_view {
            response.context_menu(|ui| {
                ui.label(RichText::new(format!("Skala {}", kind.title_id())).strong().size(11.5));
                ui.separator();
                for (label, value) in [("1:1", 1.0), ("1:2", 0.5), ("1:5", 0.2), ("2:1", 2.0)] {
                    if ui.button(label).clicked() {
                        sheet.set_view_scale(kind, Some(value));
                        ui.close();
                    }
                }
                ui.horizontal(|ui| {
                    ui.label("Kustom");
                    ui.add(egui::DragValue::new(&mut state.menu_custom_scale).speed(0.01).range(0.01..=10.0));
                    if ui.button("Terapkan").clicked() {
                        sheet.set_view_scale(kind, Some(state.menu_custom_scale));
                        ui.close();
                    }
                });
                ui.separator();
                if ui.button("Ikuti skala lembar").clicked() {
                    sheet.set_view_scale(kind, None);
                    ui.close();
                }
            });
        }

        // Tekstur render berbayang (dibuat ulang hanya bila pikselnya berubah).
        state.shaded_textures.truncate(sheet.shaded.len());
        for (i, view) in sheet.shaded.iter().enumerate() {
            if view.image.is_empty() {
                continue;
            }
            let mut key = (view.image.width as u64) << 32 | view.image.height as u64;
            for b in view.image.rgb.iter().step_by(997) {
                key = key.wrapping_mul(0x0000_0100_0000_01b3) ^ *b as u64;
            }
            let fresh = state.shaded_textures.get(i).is_some_and(|(k, _)| *k == key);
            if !fresh {
                let image = egui::ColorImage::from_rgb(
                    [view.image.width as usize, view.image.height as usize],
                    &view.image.rgb,
                );
                let tex = ui.ctx().load_texture(format!("sheet-shaded-{i}"), image, egui::TextureOptions::LINEAR);
                if i < state.shaded_textures.len() {
                    state.shaded_textures[i] = (key, tex);
                } else if i == state.shaded_textures.len() {
                    state.shaded_textures.push((key, tex));
                }
            }
        }

        // Render Lembar Kertas & Konten Gambar 2D
        render_sheet_canvas(ui, canvas_rect, state, sheet, active_snap_pt_mm, cursor_pos);

        // Panduan alat (pojok kiri bawah) untuk Section dan Dimensi asosiatif.
        let guide_time = ui.input(|i| i.time);
        if state.section_tool_active {
            crate::tool_guides::ToolGuides::render_sheet_guide(
                ui,
                canvas_rect,
                crate::tool_guides::SheetGuide::Section,
                state.section_points.len(),
                guide_time,
            );
        } else if state.measure_tool_active {
            crate::tool_guides::ToolGuides::render_sheet_guide(
                ui,
                canvas_rect,
                crate::tool_guides::SheetGuide::Dimension,
                state.measure_points.len(),
                guide_time,
            );
        }

        // Indikator lembar kedaluwarsa: geometri berubah setelah lembar dibuat.
        if sheet.stale {
            let badge = Rect::from_center_size(
                Pos2::new(canvas_rect.center().x, topbar_rect.max.y + 26.0),
                vec2(330.0, 26.0),
            );
            ui.painter().rect_filled(badge, CornerRadius::same(6), Color32::from_rgb(150, 92, 10));
            let stale_resp = ui.put(
                badge,
                egui::Button::new(
                    RichText::new(format!(
                        "{}  Lembar kedaluwarsa — klik untuk memperbarui",
                        ICON_WARNING.codepoint
                    ))
                    .size(11.5)
                    .color(Color32::WHITE),
                )
                .fill(Color32::TRANSPARENT),
            );
            if stale_resp.clicked() {
                event = Some(DrawingSheetEvent::Refresh);
            }
        }

        // 4. Inline Live Text Edit Box (Mengedit langsung di tempat pada etiket atau teks bebas)
        let mut finish_text_edit = false;
        if let Some(target) = state.active_text_edit {
            match target {
                ActiveTextTarget::TitleBlock(field) => {
                    let tb = sheet.title_block_rect_mm();
                    let f_rect_mm = title_block_field_rect_mm(tb, field);
                    let p_bl = mm_to_screen(f_rect_mm[0], f_rect_mm[1]);
                    let p_tr = mm_to_screen(f_rect_mm[2], f_rect_mm[3]);
                    let field_screen_rect = Rect::from_two_pos(p_bl, p_tr);

                    let font_sz = match field {
                        TitleBlockFieldId::ProjectTitle => (4.8 * zoom).clamp(9.0, 16.0),
                        TitleBlockFieldId::CompanyName => (4.0 * zoom).clamp(8.5, 14.0),
                        _ => (3.2 * zoom).clamp(7.5, 12.0),
                    };

                    let val_mut = field.get_mut_str(&mut sheet.title_block);
                    let mut edit_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(field_screen_rect)
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );

                    Frame::NONE
                        .fill(Color32::WHITE)
                        .stroke(Stroke::new(1.5, Color32::from_rgb(0, 130, 250)))
                        .corner_radius(CornerRadius::same(2))
                        .inner_margin(Margin::symmetric(3, 1))
                        .show(&mut edit_ui, |ui| {
                            let res = ui.add(
                                egui::TextEdit::singleline(val_mut)
                                    .font(FontId::proportional(font_sz))
                                    .text_color(Color32::BLACK)
                                    .frame(egui::Frame::NONE)
                                    .hint_text("Ketik...")
                                    .desired_width(field_screen_rect.width() - 6.0),
                            );
                            // Hanya minta fokus bila belum fokus (hindari restart IME tiap frame di iPad).
                            if !res.has_focus() {
                                res.request_focus();
                            }
                            if res.lost_focus()
                                || ui.input(|i| {
                                    i.key_pressed(egui::Key::Enter)
                                        || i.key_pressed(egui::Key::Escape)
                                })
                            {
                                finish_text_edit = true;
                            }
                        });
                }
                ActiveTextTarget::CustomText(idx) => {
                    if idx < sheet.custom_texts.len() {
                        let pos_mm = sheet.custom_texts[idx].position;
                        let font_size = sheet.custom_texts[idx].font_size;
                        let p_top_left = mm_to_screen(pos_mm[0], pos_mm[1]);
                        let font_sz = (font_size * zoom).clamp(8.0, 22.0);
                        let text_w = ((sheet.custom_texts[idx].text.len().max(12) as f32)
                            * font_sz
                            * 0.65
                            + 30.0)
                            .clamp(120.0, 450.0);
                        let edit_rect = Rect::from_min_size(
                            Pos2::new(p_top_left.x, p_top_left.y - font_sz * 1.2),
                            vec2(text_w, font_sz * 1.8),
                        );

                        let val_mut = &mut sheet.custom_texts[idx].text;
                        let mut edit_ui = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(edit_rect)
                                .layout(egui::Layout::left_to_right(egui::Align::Center)),
                        );
                        Frame::NONE
                            .fill(Color32::WHITE)
                            .stroke(Stroke::new(1.5, Color32::from_rgb(0, 130, 250)))
                            .corner_radius(CornerRadius::same(3))
                            .inner_margin(Margin::symmetric(4, 2))
                            .show(&mut edit_ui, |ui| {
                                let res = ui.add(
                                    egui::TextEdit::singleline(val_mut)
                                        .font(FontId::proportional(font_sz))
                                        .text_color(Color32::BLACK)
                                        .frame(egui::Frame::NONE)
                                        .hint_text("Ketik catatan...")
                                        .desired_width(edit_rect.width() - 8.0),
                                );
                                // Hanya minta fokus bila belum fokus (hindari restart IME tiap frame di iPad).
                                if !res.has_focus() {
                                    res.request_focus();
                                }
                                if res.lost_focus()
                                    || ui.input(|i| {
                                        i.key_pressed(egui::Key::Enter)
                                            || i.key_pressed(egui::Key::Escape)
                                    })
                                {
                                    finish_text_edit = true;
                                }
                            });
                    } else {
                        finish_text_edit = true;
                    }
                }
                ActiveTextTarget::BomTitle => {
                    let bom_tb = sheet.bom_table_rect_mm();
                    let title_h = sheet.bom_title_height_mm();
                    let p_bl = mm_to_screen(bom_tb[0] + 2.0, bom_tb[3] - title_h + 1.0);
                    let p_tr = mm_to_screen(bom_tb[2] - 30.0, bom_tb[3] - 1.0);
                    let edit_rect = Rect::from_two_pos(p_bl, p_tr);
                    let font_sz = (3.6 * zoom).clamp(8.0, 14.0);

                    let val_mut = &mut sheet.bom_table.title;
                    let mut edit_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(edit_rect)
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );
                    Frame::NONE
                        .fill(Color32::WHITE)
                        .stroke(Stroke::new(1.5, Color32::from_rgb(0, 130, 250)))
                        .corner_radius(CornerRadius::same(2))
                        .inner_margin(Margin::symmetric(3, 1))
                        .show(&mut edit_ui, |ui| {
                            let res = ui.add(
                                egui::TextEdit::singleline(val_mut)
                                    .font(FontId::proportional(font_sz))
                                    .text_color(Color32::BLACK)
                                    .frame(egui::Frame::NONE)
                                    .hint_text("BILL OF MATERIALS")
                                    .desired_width(edit_rect.width() - 6.0),
                            );
                            // Hanya minta fokus bila belum fokus (hindari restart IME tiap frame di iPad).
                            if !res.has_focus() {
                                res.request_focus();
                            }
                            if res.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Escape)) {
                                finish_text_edit = true;
                            }
                        });
                }
                ActiveTextTarget::BomCell(row_idx, col_field) => {
                    if row_idx < sheet.bom_table.items.len() {
                        let bom_tb = sheet.bom_table_rect_mm();
                        let title_h = sheet.bom_title_height_mm();
                        let header_h = sheet.bom_header_height_mm();
                        let row_h = sheet.bom_row_height_mm();
                        let col_w = sheet.bom_column_widths_mm();

                        let y_top = bom_tb[3];
                        let y_row_top = y_top - title_h - header_h - (row_idx as f32 * row_h);
                        let y_row_bot = y_row_top - row_h;

                        let (c_start_x, c_w) = match col_field {
                            BomCellField::PartName => (bom_tb[0] + col_w[0], col_w[1]),
                            BomCellField::Quantity => (bom_tb[0] + col_w[0] + col_w[1], col_w[2]),
                            BomCellField::Material => (bom_tb[0] + col_w[0] + col_w[1] + col_w[2], col_w[3]),
                            BomCellField::Description => (bom_tb[0] + col_w[0] + col_w[1] + col_w[2] + col_w[3], col_w[4]),
                        };

                        let p_bl = mm_to_screen(c_start_x + 0.5, y_row_bot + 0.5);
                        let p_tr = mm_to_screen(c_start_x + c_w - 0.5, y_row_top - 0.5);
                        let edit_rect = Rect::from_two_pos(p_bl, p_tr);
                        let font_sz = (2.6 * zoom).clamp(7.5, 12.0);

                        let mut qty_str = format!("{}", sheet.bom_table.items[row_idx].quantity);
                        let mut edit_ui = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(edit_rect)
                                .layout(egui::Layout::left_to_right(egui::Align::Center)),
                        );

                        Frame::NONE
                            .fill(Color32::WHITE)
                            .stroke(Stroke::new(1.5, Color32::from_rgb(0, 130, 250)))
                            .corner_radius(CornerRadius::same(2))
                            .inner_margin(Margin::symmetric(3, 1))
                            .show(&mut edit_ui, |ui| {
                                let (res, changed_qty) = match col_field {
                                    BomCellField::PartName => {
                                        let val_mut = &mut sheet.bom_table.items[row_idx].part_name;
                                        (ui.add(egui::TextEdit::singleline(val_mut).font(FontId::proportional(font_sz)).text_color(Color32::BLACK).frame(egui::Frame::NONE).desired_width(edit_rect.width() - 4.0)), false)
                                    }
                                    BomCellField::Quantity => {
                                        let r = ui.add(egui::TextEdit::singleline(&mut qty_str).font(FontId::proportional(font_sz)).text_color(Color32::BLACK).frame(egui::Frame::NONE).desired_width(edit_rect.width() - 4.0));
                                        (r, true)
                                    }
                                    BomCellField::Material => {
                                        let val_mut = &mut sheet.bom_table.items[row_idx].material;
                                        (ui.add(egui::TextEdit::singleline(val_mut).font(FontId::proportional(font_sz)).text_color(Color32::BLACK).frame(egui::Frame::NONE).desired_width(edit_rect.width() - 4.0)), false)
                                    }
                                    BomCellField::Description => {
                                        let val_mut = &mut sheet.bom_table.items[row_idx].description;
                                        (ui.add(egui::TextEdit::singleline(val_mut).font(FontId::proportional(font_sz)).text_color(Color32::BLACK).frame(egui::Frame::NONE).desired_width(edit_rect.width() - 4.0)), false)
                                    }
                                };
                                if changed_qty {
                                    if let Ok(num) = qty_str.trim().parse::<u32>() {
                                        sheet.bom_table.items[row_idx].quantity = num;
                                    }
                                }
                                // Hanya minta fokus bila belum fokus (hindari restart IME tiap frame di iPad).
                                if !res.has_focus() {
                                    res.request_focus();
                                }
                                if res.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Escape)) {
                                    finish_text_edit = true;
                                }
                            });
                    } else {
                        finish_text_edit = true;
                    }
                }
            }
        }
        if finish_text_edit {
            if let Some(ActiveTextTarget::CustomText(idx)) = state.active_text_edit {
                if idx < sheet.custom_texts.len() && sheet.custom_texts[idx].text.trim().is_empty() {
                    sheet.custom_texts.remove(idx);
                    state.selected_text_idx = None;
                }
            }
            state.active_text_edit = None;
        }

        // 5. Render Header Controls (Floating Top Bar Glassmorphism di Atas Kanvas)
        let mut header_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(topbar_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );

        glass_frame().show(&mut header_ui, |ui| {
            ui.set_height(30.0);
            ui.horizontal(|ui| {
                // A. Icon Drawing Sheet (Minimalis tanpa teks judul)
                ui.label(
                    RichText::new(ICON_PICTURE_AS_PDF.codepoint)
                        .size(14.0)
                        .color(ACCENT_BLUE),
                )
                .on_hover_text("Lembar Kerja Gambar Teknik 2D");

                ui.add_space(2.0);
                ui.separator();
                ui.add_space(2.0);

                // B. Pemilih Ukuran Kertas (A4/A3)
                ui.label(RichText::new("Kertas:").size(11.0).color(TEXT_SECONDARY));
                egui::ComboBox::from_id_salt("paper_size_combo")
                    .selected_text(sheet.paper_size.label())
                    .show_ui(ui, |ui| crate::theme::glass_menu(ui, |ui| {
                        if ui.selectable_label(sheet.paper_size == PaperSize::A4Landscape, PaperSize::A4Landscape.label()).clicked() {
                            sheet.paper_size = PaperSize::A4Landscape;
                            sheet.auto_layout();
                        }
                        if ui.selectable_label(sheet.paper_size == PaperSize::A4Portrait, PaperSize::A4Portrait.label()).clicked() {
                            sheet.paper_size = PaperSize::A4Portrait;
                            sheet.auto_layout();
                        }
                        if ui.selectable_label(sheet.paper_size == PaperSize::A3Landscape, PaperSize::A3Landscape.label()).clicked() {
                            sheet.paper_size = PaperSize::A3Landscape;
                            sheet.auto_layout();
                        }
                        if ui.selectable_label(sheet.paper_size == PaperSize::A3Portrait, PaperSize::A3Portrait.label()).clicked() {
                            sheet.paper_size = PaperSize::A3Portrait;
                            sheet.auto_layout();
                        }
                    }));

                // C. Skala Gambar Mode Sliding Panjang & Halus (Bisa langsung diklik untuk ketik angka)
                ui.label(RichText::new("Skala:").size(11.0).color(TEXT_SECONDARY));
                let mut cur_scale = sheet.scale as f64;
                let scale_slider = egui::Slider::new(&mut cur_scale, 0.01..=3.0)
                    .logarithmic(true)
                    .custom_formatter(|n, _| format_scale_ratio(n as f32))
                    .custom_parser(|s| {
                        let s = s.trim();
                        if let Some((a, b)) = s.split_once(':') {
                            if let (Ok(num), Ok(den)) = (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
                                if den > 0.0 {
                                    return Some(num / den);
                                }
                            }
                        }
                        s.parse::<f64>().ok()
                    })
                    .show_value(true);

                let slider_resp = ui
                    .add_sized([260.0, 20.0], scale_slider)
                    .on_hover_text("Geser untuk mengubah skala secara sangat halus, atau klik angka untuk mengetik rasio skala");
                if slider_resp.changed() {
                    sheet.layout_with_scale(cur_scale as f32);
                }

                let auto_btn = header_icon_btn(
                    ui,
                    ICON_REFRESH.codepoint,
                    false,
                    "Auto Layout",
                    Some("R"),
                    Some("Atur ulang posisi tampak proyeksi & skala optimal otomatis"),
                    None,
                    None,
                );
                if auto_btn.clicked() {
                    sheet.auto_layout();
                }

                ui.add_space(2.0);
                ui.separator();
                ui.add_space(2.0);

                // D. Toggles Visibilitas Gambar
                let hlr_btn = header_icon_btn(
                    ui,
                    ICON_LAYERS.codepoint,
                    sheet.show_hidden_lines,
                    "Garis Tersembunyi (Hidden Lines)",
                    Some("H"),
                    Some("Tampilkan tepi garis tersembunyi bergaris putus-putus"),
                    Some(Color32::from_rgba_premultiplied(18, 42, 85, 100)),
                    Some(ACCENT_BLUE),
                );
                if hlr_btn.clicked() {
                    sheet.show_hidden_lines = !sheet.show_hidden_lines;
                }

                let dim_btn = header_icon_btn(
                    ui,
                    ICON_STRAIGHTEN.codepoint,
                    sheet.show_dimensions,
                    "Dimensi Otomatis",
                    Some("D"),
                    Some("Tampilkan anotasi ukuran dimensi proyeksi"),
                    Some(Color32::from_rgba_premultiplied(18, 42, 85, 100)),
                    Some(ACCENT_BLUE),
                );
                if dim_btn.clicked() {
                    sheet.show_dimensions = !sheet.show_dimensions;
                }

                let measure_btn = header_icon_btn(
                    ui,
                    ICON_SQUARE_FOOT.codepoint,
                    state.measure_tool_active,
                    "Manual Dimension Tool (Ukur)",
                    Some("M"),
                    Some("Klik titik pada gambar untuk menambah dimensi linier, diameter, radius, atau sudut secara kustom"),
                    Some(Color32::from_rgba_premultiplied(18, 42, 85, 100)),
                    Some(Color32::from_rgb(255, 140, 0)),
                );
                if measure_btn.clicked() {
                    state.measure_tool_active = !state.measure_tool_active;
                    if state.measure_tool_active {
                        state.text_tool_active = false;
                        state.detail_tool_active = false;
                    }
                    state.measure_points.clear();
                    state.measure_first_pt = None;
                }

                if state.measure_tool_active {
                    ui.add_space(2.0);
                    ui.label(RichText::new("Dim:").size(10.5).color(Color32::from_rgb(255, 140, 0)));
                    for mode in ManualDimensionMode::ALL {
                        let is_sel = state.dimension_mode == mode;
                        let btn = ui.add(
                            egui::Button::new(
                                RichText::new(format!("{} {}", mode.icon(), mode.label()))
                                    .size(10.0)
                                    .strong()
                                    .color(if is_sel { Color32::WHITE } else { TEXT_SECONDARY }),
                            )
                            .fill(if is_sel {
                                Color32::from_rgb(200, 100, 0)
                            } else {
                                Color32::from_rgba_premultiplied(35, 40, 50, 180)
                            })
                            .corner_radius(CornerRadius::same(3))
                            .min_size(Vec2::new(26.0, 18.0)),
                        );
                        if btn.clicked() {
                            state.dimension_mode = mode;
                            state.measure_points.clear();
                            state.measure_first_pt = None;
                        }
                    }

                    // Prompt hint
                    let prompt_text = match state.dimension_mode {
                        ManualDimensionMode::Linear => {
                            if state.measure_points.is_empty() {
                                "Pilih Titik 1"
                            } else {
                                "Pilih Titik 2 (Selesai)"
                            }
                        }
                        ManualDimensionMode::Diameter => {
                            if state.measure_points.is_empty() {
                                "Pilih Titik Pusat"
                            } else {
                                "Pilih Tepi Lingkaran (Ø)"
                            }
                        }
                        ManualDimensionMode::Radius => {
                            if state.measure_points.is_empty() {
                                "Pilih Titik Pusat"
                            } else {
                                "Pilih Tepi Busur (R)"
                            }
                        }
                        ManualDimensionMode::Angle => match state.measure_points.len() {
                            0 => "Pilih Titik Sudut/Puncak",
                            1 => "Pilih Kaki Garis 1",
                            _ => "Pilih Kaki Garis 2 (∠)",
                        },
                    };
                    ui.label(RichText::new(format!("({})", prompt_text)).size(10.0).color(Color32::from_rgb(255, 200, 100)));
                }

                let cl_btn = header_icon_btn(
                    ui,
                    ICON_TEXTURE.codepoint,
                    sheet.show_centerlines,
                    "Garis Sumbu (Centerlines)",
                    Some("C"),
                    Some("Tampilkan garis sumbu simetri hijau"),
                    Some(Color32::from_rgba_premultiplied(18, 42, 85, 100)),
                    Some(ACCENT_BLUE),
                );
                if cl_btn.clicked() {
                    sheet.show_centerlines = !sheet.show_centerlines;
                }

                let sec_btn = header_icon_btn(
                    ui,
                    ICON_CONTENT_CUT.codepoint,
                    sheet.show_section_view,
                    "Section View A-A (Tampak Potongan)",
                    Some("P"),
                    Some("Tampilkan tampak potongan melintang A-A lengkap dengan arsir 45° ISO/ANSI"),
                    Some(Color32::from_rgba_premultiplied(18, 42, 85, 100)),
                    Some(ACCENT_BLUE),
                );
                if sec_btn.clicked() {
                    sheet.show_section_view = !sheet.show_section_view;
                    sheet.auto_layout();
                }

                let section_tool_btn = header_icon_btn(
                    ui,
                    ICON_HORIZONTAL_SPLIT.codepoint,
                    state.section_tool_active,
                    "Alat Section (Garis Potong)",
                    Some("S"),
                    Some("Klik dua titik pada Tampak Depan/Atas/Kanan untuk membuat potongan baru. Tahan Shift untuk potongan bertingkat."),
                    Some(Color32::from_rgba_premultiplied(18, 42, 85, 100)),
                    Some(Color32::from_rgb(255, 140, 0)),
                );
                if section_tool_btn.clicked() {
                    state.section_tool_active = !state.section_tool_active;
                    state.section_points.clear();
                    state.section_parent = None;
                    if state.section_tool_active {
                        state.measure_tool_active = false;
                        state.detail_tool_active = false;
                        state.text_tool_active = false;
                        state.balloon_tool_active = false;
                    }
                }

                let shaded_btn = header_icon_btn(
                    ui,
                    ICON_VIEW_IN_AR.codepoint,
                    false,
                    "Sisipkan render 3D",
                    None,
                    Some("Tambahkan render berbayang dari arah kamera viewport saat ini ke lembar"),
                    None,
                    None,
                );
                if shaded_btn.clicked() {
                    event = Some(DrawingSheetEvent::InsertShaded);
                }

                let regen_btn = header_icon_btn(
                    ui,
                    ICON_AUTORENEW.codepoint,
                    false,
                    "Regenerasi dimensi otomatis",
                    None,
                    Some("Buat ulang seluruh dimensi otomatis dan kembalikan posisinya ke tata letak bawaan"),
                    None,
                    None,
                );
                if regen_btn.clicked() {
                    for dim in &mut sheet.auto_dimensions {
                        dim.pinned = false;
                    }
                    sheet.generate_auto_dimensions();
                    state.selected_dim = None;
                }

                let hatch_btn = header_icon_btn(
                    ui,
                    ICON_GRID_VIEW.codepoint,
                    sheet.show_hatch,
                    "Arsir ISO 45° (Hatch Pattern)",
                    Some("A"),
                    Some("Tampilkan pola arsir miring 45° standar ISO pada penampang potongan solid"),
                    Some(Color32::from_rgba_premultiplied(18, 42, 85, 100)),
                    Some(ACCENT_BLUE),
                );
                if hatch_btn.clicked() {
                    sheet.show_hatch = !sheet.show_hatch;
                }

                let detail_btn = header_icon_btn(
                    ui,
                    ICON_SEARCH.codepoint,
                    state.detail_tool_active || !sheet.drawing.detail_views.is_empty(),
                    "Detail View (Lingkaran Pembesar Skala Detail)",
                    Some("B"),
                    Some("Klik pada tampak gambar untuk membuat area pembesar independen mikro (2:1, 5:1, 10:1)"),
                    Some(Color32::from_rgba_premultiplied(18, 42, 85, 100)),
                    Some(Color32::from_rgb(0, 210, 160)),
                );
                if detail_btn.clicked() {
                    state.detail_tool_active = !state.detail_tool_active;
                    if state.detail_tool_active {
                        state.text_tool_active = false;
                        state.measure_tool_active = false;
                    }
                }

                let text_btn = header_icon_btn(
                    ui,
                    ICON_EDIT_NOTE.codepoint,
                    state.text_tool_active,
                    "Tool Input Teks & Edit Etiket (Live Text)",
                    Some("T"),
                    Some("Klik teks/etiket untuk edit langsung, atau klik kertas untuk menambah teks baru"),
                    Some(Color32::from_rgba_premultiplied(18, 42, 85, 100)),
                    Some(Color32::from_rgb(0, 180, 255)),
                );
                if text_btn.clicked() {
                    state.text_tool_active = !state.text_tool_active;
                    if state.text_tool_active {
                        state.measure_tool_active = false;
                        state.detail_tool_active = false;
                        state.balloon_tool_active = false;
                    }
                }

                let bom_btn = header_icon_btn(
                    ui,
                    ICON_TABLE_CHART.codepoint,
                    sheet.show_bom_table,
                    "Tabel BOM (Bill of Materials)",
                    Some("O"),
                    Some("Tampilkan tabel daftar komponen/part dan material (ISO 7573)"),
                    Some(Color32::from_rgba_premultiplied(18, 42, 85, 100)),
                    Some(ACCENT_BLUE),
                );
                if bom_btn.clicked() {
                    sheet.show_bom_table = !sheet.show_bom_table;
                }

                let balloon_btn = header_icon_btn(
                    ui,
                    ICON_ADJUST.codepoint,
                    state.balloon_tool_active || sheet.show_balloons,
                    "Part Callout Balloons (Nomor Penunjuk)",
                    Some("U"),
                    Some("Klik untuk aktifkan tool penunjuk nomor bagian pada gambar isometrik"),
                    Some(Color32::from_rgba_premultiplied(18, 42, 85, 100)),
                    Some(Color32::from_rgb(255, 160, 40)),
                );
                if balloon_btn.clicked() {
                    state.balloon_tool_active = !state.balloon_tool_active;
                    if state.balloon_tool_active {
                        sheet.show_balloons = true;
                        state.text_tool_active = false;
                        state.measure_tool_active = false;
                        state.detail_tool_active = false;
                    }
                }
                if state.balloon_tool_active {
                    ui.label(RichText::new("(Klik gambar untuk menaruh balon)").size(10.0).color(Color32::from_rgb(255, 180, 50)));
                }

                if state.detail_tool_active || state.selected_detail_label.is_some() || !sheet.drawing.detail_views.is_empty() {
                    ui.add_space(2.0);
                    ui.label(RichText::new("Detail:").size(10.5).color(Color32::from_rgb(0, 210, 160)));
                    let scale_presets = [(2.0, "2:1"), (4.0, "4:1"), (5.0, "5:1"), (10.0, "10:1")];
                    for (mult, lbl) in scale_presets {
                        let is_sel = (state.detail_scale_multiplier - mult).abs() < 1e-3;
                        let btn = ui.add(
                            egui::Button::new(RichText::new(lbl).size(10.0).strong().color(if is_sel { Color32::WHITE } else { TEXT_SECONDARY }))
                                .fill(if is_sel { Color32::from_rgb(0, 150, 110) } else { Color32::from_rgba_premultiplied(35, 40, 50, 180) })
                                .corner_radius(CornerRadius::same(3))
                                .min_size(Vec2::new(26.0, 18.0)),
                        );
                        if btn.clicked() {
                            state.detail_scale_multiplier = mult;
                            if let Some(target_lbl) = state.selected_detail_label {
                                if let Some(det) = sheet.drawing.detail_views.iter().find(|d| d.indicator.label == target_lbl).cloned() {
                                    sheet.add_or_update_detail_view(
                                        det.indicator.parent_view,
                                        det.indicator.center_2d,
                                        det.indicator.radius_mm,
                                        mult,
                                        target_lbl,
                                    );
                                }
                            }
                        }
                    }
                }

                ui.add_space(2.0);
                ui.separator();
                ui.add_space(2.0);

                // E. Tombol Zoom In & Zoom Out di Top Bar
                let zoom_out_header = header_icon_btn(
                    ui,
                    "-",
                    false,
                    "Zoom Out",
                    Some("-"),
                    Some("Perkecil tampilan kanvas kertas"),
                    None,
                    None,
                );
                if zoom_out_header.clicked() {
                    state.zoom = (state.zoom / 1.2).clamp(0.15, 8.0);
                }

                let fit_zoom = calculate_fit_zoom(canvas_rect, sheet.paper_size);
                let zoom_percent = (state.zoom / fit_zoom * 100.0).round() as i32;
                if ui
                    .add(egui::Button::new(RichText::new(format!("{}%", zoom_percent)).size(10.5).color(TEXT_SECONDARY)).frame(false))
                    .on_hover_text("Pusatkan Kertas ke Layar (Fit / 100%)")
                    .clicked()
                {
                    state.pan_offset = Vec2::ZERO;
                    state.zoom = fit_zoom;
                }

                let zoom_in_header = header_icon_btn(
                    ui,
                    "+",
                    false,
                    "Zoom In",
                    Some("+"),
                    Some("Perbesar tampilan kanvas kertas"),
                    None,
                    None,
                );
                if zoom_in_header.clicked() {
                    state.zoom = (state.zoom * 1.2).clamp(0.15, 8.0);
                }

                // F. Sisi Kanan: Close (X) dan Ekspor
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Tombol Tutup (X) di paling kanan
                    let close_btn = header_icon_btn(
                        ui,
                        ICON_CLOSE.codepoint,
                        false,
                        "Tutup Lembar Kerja",
                        Some("Esc"),
                        Some("Kembali ke viewport 3D"),
                        None,
                        None,
                    );
                    if close_btn.clicked() {
                        event = Some(DrawingSheetEvent::Close);
                    }

                    ui.add_space(4.0);

                    // Satu tombol Ekspor: menu pilihan format (PDF / DXF / SVG)
                    let export_btn = header_icon_btn(
                        ui,
                        ICON_DOWNLOAD.codepoint,
                        false,
                        "Ekspor Gambar Kerja",
                        None,
                        Some("Pilih format: PDF vektor, DXF CAD, atau SVG"),
                        None,
                        Some(ACCENT_BLUE),
                    );
                    egui::Popup::menu(&export_btn)
                        .info(
                            egui::UiStackInfo::new(egui::UiKind::Menu)
                                .with_tag_value(MenuConfig::MENU_CONFIG_TAG, MenuConfig::new()),
                        )
                        .show(|ui| {
                            crate::theme::glass_menu(ui, |ui| {
                                let items: [(&str, &str, &str, DrawingSheetEvent); 3] = [
                                    (
                                        ICON_PICTURE_AS_PDF.codepoint,
                                        "PDF Vektor",
                                        "Cetak dokumen gambar teknik presisi ke file PDF",
                                        DrawingSheetEvent::ExportPdf,
                                    ),
                                    (
                                        ICON_DOWNLOAD.codepoint,
                                        "DXF CAD",
                                        "Ekspor vektor 2D ke format CAD DXF",
                                        DrawingSheetEvent::ExportDxf,
                                    ),
                                    (
                                        ICON_DOWNLOAD.codepoint,
                                        "SVG Vektor 2D",
                                        "Ekspor gambar kerja ke format vektor SVG",
                                        DrawingSheetEvent::ExportSvg,
                                    ),
                                ];
                                for (icon, label, hint, ev) in items {
                                    if ui
                                        .button(format!("{icon} {label}"))
                                        .on_hover_text(hint)
                                        .clicked()
                                    {
                                        event = Some(ev);
                                        ui.close();
                                    }
                                }
                            })
                        });
                });
            });
        });

        // 6. Floating Zoom In / Zoom Out / Fit Toolbar di Pojok Kanan Bawah
        let mut zoom_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(zoom_controls_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );

        glass_frame().show(&mut zoom_ui, |ui| {
            ui.set_height(32.0);
            ui.horizontal(|ui| {
                let z_out = ui
                    .add(
                        egui::Button::new(RichText::new("-").size(15.0).color(TEXT_PRIMARY))
                            .frame(false)
                            .min_size(vec2(26.0, 24.0)),
                    )
                    .on_hover_text("Perkecil Tampilan (Zoom Out)");
                if z_out.clicked() {
                    state.zoom = (state.zoom / 1.2).clamp(0.15, 8.0);
                }

                ui.separator();

                let z_fit = ui
                    .add(
                        egui::Button::new(
                            RichText::new(ICON_FIT_SCREEN.codepoint.to_string())
                                .size(14.0)
                                .color(TEXT_PRIMARY),
                        )
                        .frame(false)
                        .min_size(vec2(28.0, 24.0)),
                    )
                    .on_hover_text("Pusatkan Kertas ke Layar (Fit)");
                if z_fit.clicked() {
                    state.pan_offset = Vec2::ZERO;
                    state.zoom = calculate_fit_zoom(canvas_rect, sheet.paper_size);
                }

                ui.separator();

                let z_in = ui
                    .add(
                        egui::Button::new(RichText::new("+").size(15.0).color(TEXT_PRIMARY))
                            .frame(false)
                            .min_size(vec2(26.0, 24.0)),
                    )
                    .on_hover_text("Perbesar Tampilan (Zoom In)");
                if z_in.clicked() {
                    state.zoom = (state.zoom * 1.2).clamp(0.15, 8.0);
                }
            });
        });

        event
    }
}

/// Ubah zoom dengan `factor` sambil mempertahankan titik layar `anchor`
/// menunjuk ke titik kertas yang sama (zoom di kursor / pusat pinch).
pub fn zoom_about(state: &mut DrawingSheetViewState, canvas_rect: Rect, anchor: Pos2, factor: f32) {
    let old_zoom = state.zoom;
    let new_zoom = (old_zoom * factor).clamp(0.15, 8.0);
    if new_zoom == old_zoom {
        return;
    }
    let ratio = new_zoom / old_zoom;
    let center = canvas_rect.center();
    // Kertas digambar di `center + pan_offset`; titik kertas yang ada di
    // `anchor` punya offset `(anchor - center - pan)` yang harus diskalakan.
    let rel = anchor - center - state.pan_offset;
    state.pan_offset = anchor - center - rel * ratio;
    state.zoom = new_zoom;
}

fn calculate_fit_zoom(canvas_rect: Rect, paper_size: PaperSize) -> f32 {
    let (pw, ph) = paper_size.dimensions_mm();
    let margin = 60.0;
    let avail_w = (canvas_rect.width() - margin).max(100.0);
    let avail_h = (canvas_rect.height() - margin).max(100.0);

    let scale_w = avail_w / pw;
    let scale_h = avail_h / ph;
    (scale_w.min(scale_h) * 0.95).clamp(0.5, 4.0)
}

/// Render kanvas kertas putih dengan bayangan drop shadow dan gambar vektor presisi.
fn render_sheet_canvas(
    ui: &mut Ui,
    canvas_rect: Rect,
    state: &DrawingSheetViewState,
    sheet: &DrawingSheet,
    active_snap_pt_mm: Option<[f32; 2]>,
    cursor_pos: Option<Pos2>,
) {
    let painter = ui.painter_at(canvas_rect);
    let center_pos = canvas_rect.center() + state.pan_offset;
    let (pw_mm, ph_mm) = sheet.paper_size.dimensions_mm();
    let zoom = state.zoom;

    let sheet_w_px = pw_mm * zoom;
    let sheet_h_px = ph_mm * zoom;

    let sheet_min = Pos2::new(
        center_pos.x - sheet_w_px * 0.5,
        center_pos.y - sheet_h_px * 0.5,
    );
    let sheet_max = Pos2::new(
        center_pos.x + sheet_w_px * 0.5,
        center_pos.y + sheet_h_px * 0.5,
    );
    let sheet_rect = Rect::from_min_max(sheet_min, sheet_max);

    // Transformasi dari mm lembar kerja (asal 0,0 di pojok kiri bawah kertas) ke koordinat layar pixel
    let mm_to_screen = |x_mm: f32, y_mm: f32| -> Pos2 {
        Pos2::new(
            sheet_min.x + x_mm * zoom,
            sheet_max.y - y_mm * zoom, // Balik sumbu Y (Y mm naik ke atas)
        )
    };

    let screen_to_mm = |p: Pos2| -> [f32; 2] {
        [
            (p.x - sheet_min.x) / zoom,
            (sheet_max.y - p.y) / zoom,
        ]
    };

    // A. Drop shadow kertas
    let shadow_rect = sheet_rect.translate(vec2(0.0, 6.0)).expand(6.0);
    painter.rect_filled(
        shadow_rect,
        CornerRadius::same(4),
        Color32::from_black_alpha(80),
    );

    // B. Kertas Putih Dasar
    painter.rect_filled(
        sheet_rect,
        CornerRadius::ZERO,
        Color32::from_rgb(252, 252, 252),
    );

    // Display-list bersama dengan ekspor PDF/SVG: tampilan editor = hasil cetak.
    let scene = build_scene(sheet);
    let to_screen = |p: [f32; 2]| mm_to_screen(p[0], p[1]);

    // C–D. Bingkai gambar + grid zona.
    painter.rect_stroke(
        sheet_rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, Color32::from_rgb(180, 180, 180)),
        egui::StrokeKind::Inside,
    );
    if let Some(group) = scene.group("sheet_border") {
        paint_items(&painter, &group.items, zoom, &to_screen, None, &state.shaded_textures);
    }

    // E. Kepala Gambar (Title Block ISO 7200)
    render_title_block_screen(&painter, sheet, state, zoom, mm_to_screen);

    // F. Gambar Tampak-tampak Proyeksi (Front, Top, Right, Isometric)
    for plc in &sheet.view_placements {
        if !plc.visible {
            continue;
        }
        let view = sheet.drawing.view_by_kind(plc.kind);
        let center_mm = plc.center_mm;
        let scale = plc.scale;
        let v_center = view.center_2d();
        let view_sz = view.size_2d();

        // Highlight box saat tampak di-hover / sedang digeser (Drag-and-Drop)
        let is_hovered = state.hovered_view == Some(plc.kind);
        let is_dragging = state.dragging_view == Some(plc.kind);
        if is_hovered || is_dragging {
            let half_w = (view_sz[0] * scale * 0.5 + 6.0) * zoom;
            let half_h = (view_sz[1] * scale * 0.5 + 11.5) * zoom;
            let p_center = mm_to_screen(center_mm[0], center_mm[1]);
            let v_box = Rect::from_center_size(p_center, vec2(half_w * 2.0, half_h * 2.0));

            painter.rect_stroke(
                v_box,
                CornerRadius::same(6),
                Stroke::new(1.5, if is_dragging { ACCENT_BLUE } else { Color32::from_rgb(60, 130, 240) }),
                egui::StrokeKind::Outside,
            );

            let badge_pos = Pos2::new(v_box.min.x + 6.0, v_box.min.y + 4.0);
            let badge_text = if is_dragging { format!("{} Menggeser...", ICON_OPEN_WITH.codepoint) } else { format!("{} Tahan & Geser Tata Letak", ICON_OPEN_WITH.codepoint) };
            let badge_galley = painter.layout_no_wrap(
                badge_text.to_string(),
                FontId::proportional((3.5 * zoom).clamp(8.0, 11.0)),
                Color32::WHITE,
            );
            let badge_rect = Rect::from_min_size(badge_pos, badge_galley.size() + vec2(8.0, 4.0));
            painter.rect_filled(badge_rect, CornerRadius::same(3), Color32::from_rgb(25, 95, 210));
            painter.galley(badge_pos + vec2(4.0, 2.0), badge_galley, Color32::WHITE);
        }

        // 1–5. Garis tampak, busur, arsir, sumbu, garis potong, dan judul.
        let group_id = format!("view_{}", ducad_io::drawing::view_key(plc.kind));
        if let Some(group) = scene.group(&group_id) {
            paint_items(&painter, &group.items, zoom, &to_screen, None, &state.shaded_textures);
        }

        // 5b. Tombol balik arah / hapus pada label garis potong yang disorot.
        for section in &sheet.drawing.sections {
            let Some(label) = section.label.chars().next() else {
                continue;
            };
            if section.parent != plc.kind || state.hovered_section != Some(label) {
                continue;
            }
            for lp in sheet.cutting_label_positions_mm(plc, &section.cutting_line) {
                let p_lbl = mm_to_screen(lp[0], lp[1]);
                let buttons = [
                    (p_lbl + vec2(20.0, -4.0), ICON_SWAP_HORIZ.codepoint, state.hovered_section_flip == Some(label), Color32::from_rgb(25, 95, 210)),
                    (p_lbl + vec2(40.0, -4.0), ICON_CLOSE.codepoint, state.hovered_section_delete == Some(label), Color32::from_rgb(185, 45, 45)),
                ];
                for (center, icon, hot, color) in buttons {
                    painter.circle_filled(center, if hot { 9.0 } else { 8.0 }, color);
                    painter.text(center, Align2::CENTER_CENTER, icon, FontId::proportional(11.0), Color32::WHITE);
                }
            }
        }

        // 6. Indikator Lingkaran Detail pada Tampak Acuan (Detail Callout Circle)
        for det in &sheet.drawing.detail_views {
            if det.indicator.parent_view == plc.kind {
                let ind = &det.indicator;
                let c_x_mm = center_mm[0] + (ind.center_2d[0] - v_center[0]) * scale;
                let c_y_mm = center_mm[1] + (ind.center_2d[1] - v_center[1]) * scale;
                let p_center = mm_to_screen(c_x_mm, c_y_mm);
                let r_px = ind.radius_mm * scale * zoom;

                let is_det_hovered = state.hovered_detail_label == Some(ind.label);
                let is_det_selected = state.selected_detail_label == Some(ind.label);

                let callout_color = if is_det_selected {
                    Color32::from_rgb(255, 140, 0)
                } else if is_det_hovered {
                    Color32::from_rgb(0, 150, 255)
                } else {
                    Color32::from_rgb(40, 45, 60)
                };

                let callout_stroke = Stroke::new((0.55 * zoom).clamp(1.0, 2.2), callout_color);
                draw_dashed_circle(&painter, p_center, r_px, callout_stroke, 28);

                // Titik silang pusat (Center crosshair)
                let ch_sz = 3.5 * zoom;
                painter.line_segment(
                    [Pos2::new(p_center.x - ch_sz, p_center.y), Pos2::new(p_center.x + ch_sz, p_center.y)],
                    Stroke::new(0.6 * zoom, callout_color),
                );
                painter.line_segment(
                    [Pos2::new(p_center.x, p_center.y - ch_sz), Pos2::new(p_center.x, p_center.y + ch_sz)],
                    Stroke::new(0.6 * zoom, callout_color),
                );

                // Garis penunjuk (Leader line) & Badge huruf label
                let l_x_mm = center_mm[0] + (ind.label_pos[0] - v_center[0]) * scale;
                let l_y_mm = center_mm[1] + (ind.label_pos[1] - v_center[1]) * scale;
                let p_lbl = mm_to_screen(l_x_mm, l_y_mm);
                let rim_pt = Pos2::new(
                    p_center.x + r_px * std::f32::consts::FRAC_1_SQRT_2,
                    p_center.y - r_px * std::f32::consts::FRAC_1_SQRT_2,
                );

                painter.line_segment([rim_pt, p_lbl], callout_stroke);
                let p_shoulder = Pos2::new(p_lbl.x + 14.0 * zoom.clamp(0.8, 1.5), p_lbl.y);
                painter.line_segment([p_lbl, p_shoulder], callout_stroke);

                let font_badge = FontId::proportional((4.8 * zoom).clamp(9.0, 16.0));
                let badge_text = format!("DETAIL {}", ind.label);
                let badge_pos = Pos2::new(p_lbl.x + 2.0, p_lbl.y - 2.0);
                painter.text(badge_pos, Align2::LEFT_BOTTOM, &badge_text, font_badge, callout_color);

                // Tombol hapus jika di-hover / dipilih
                if is_det_hovered || is_det_selected {
                    let del_pos = Pos2::new(p_shoulder.x + 8.0, p_shoulder.y);
                    let is_del_h = state.hovered_detail_delete == Some(ind.label);
                    let del_bg = if is_del_h { Color32::from_rgb(220, 40, 40) } else { Color32::from_rgb(180, 50, 50) };
                    painter.circle_filled(del_pos, 7.0 * zoom.clamp(0.8, 1.3), del_bg);
                    painter.text(del_pos, Align2::CENTER_CENTER, ICON_CLOSE.codepoint, FontId::proportional(10.0 * zoom.clamp(0.8, 1.2)), Color32::WHITE);
                }
            }
        }

    }

    // F2. Render berbayang + catatan umum.
    for id in ["shaded_views", "notes"] {
        if let Some(group) = scene.group(id) {
            paint_items(&painter, &group.items, zoom, &to_screen, None, &state.shaded_textures);
        }
    }
    if let Some(i) = state.hovered_shaded.or(state.dragging_shaded) {
        if let Some(view) = sheet.shaded.get(i) {
            let r = view.rect_mm();
            painter.rect_stroke(
                Rect::from_two_pos(mm_to_screen(r[0], r[1]), mm_to_screen(r[2], r[3])),
                CornerRadius::same(4),
                Stroke::new(1.5, ACCENT_BLUE),
                egui::StrokeKind::Outside,
            );
        }
    }

    // F3. Pratinjau garis potong yang sedang dibuat (alat Section).
    if state.section_tool_active {
        if let Some(parent) = state.section_parent {
            if let Some(plc) = sheet.view_placements.iter().find(|p| p.kind == parent) {
                let vc = sheet.drawing.view_by_kind(parent).center_2d();
                let model_to_screen = |m: [f32; 2]| {
                    mm_to_screen(
                        plc.center_mm[0] + (m[0] - vc[0]) * plc.scale,
                        plc.center_mm[1] + (m[1] - vc[1]) * plc.scale,
                    )
                };
                let stroke = Stroke::new(1.8, Color32::from_rgb(255, 140, 0));
                let mut pts: Vec<Pos2> = state.section_points.iter().map(|m| model_to_screen(*m)).collect();
                if let (Some(last), Some(cur)) = (pts.last().copied(), cursor_pos) {
                    // Ruas berikutnya dikunci mendatar/tegak.
                    let d = cur - last;
                    pts.push(if d.x.abs() >= d.y.abs() { Pos2::new(cur.x, last.y) } else { Pos2::new(last.x, cur.y) });
                }
                for w in pts.windows(2) {
                    draw_dashed_line(&painter, w[0], w[1], stroke, 8.0, 4.0);
                }
                for p in &pts {
                    painter.circle_filled(*p, 3.5, Color32::from_rgb(255, 140, 0));
                }
            }
        }
    }

    // G. Anotasi Dimensi Presisi (Otomatis & Manual) dengan Panah Terisi & Extension Lines
    let mut dims_to_render: Vec<(DimensionTarget, &ducad_io::drawing::DimensionAnnotation, bool)> = Vec::new();
    if sheet.show_dimensions {
        for (idx, dim) in sheet.auto_dimensions.iter().enumerate() {
            dims_to_render.push((DimensionTarget::Auto(idx), dim, false));
        }
    }
    for (idx, dim) in sheet.manual_dimensions.iter().enumerate() {
        dims_to_render.push((DimensionTarget::Manual(idx), dim, true));
    }

    if !dims_to_render.is_empty() {
        for (target, dim, is_manual) in dims_to_render {
            let is_dim_hovered = state.hovered_dim == Some(target);
            let is_dim_selected = state.selected_dim == Some(target);
            let is_dim_dragging = state.dragging_dim == Some(target);
            let is_del_hovered = state.hovered_dim_delete == Some(target);

            let active_dim_color = if is_dim_selected || is_dim_dragging {
                Color32::from_rgb(255, 135, 15)
            } else if is_dim_hovered {
                Color32::from_rgb(0, 110, 230)
            } else if is_manual {
                Color32::from_rgb(18, 90, 190)
            } else {
                Color32::from_rgb(12, 70, 175)
            };

            // Geometri dimensi dari display-list bersama (sama dengan PDF/SVG).
            let tb = dim.text_box();
            let label_bg_rect = Rect::from_two_pos(mm_to_screen(tb[0], tb[1]), mm_to_screen(tb[2], tb[3])).expand(1.5);
            painter.rect_filled(label_bg_rect, CornerRadius::same(2), Color32::from_rgba_premultiplied(252, 252, 252, 235));
            let tint = (is_dim_selected || is_dim_dragging || is_dim_hovered || is_manual).then_some(active_dim_color);
            paint_items(&painter, &dimension_items(dim), zoom, &to_screen, tint, &state.shaded_textures);

            // Highlight border dan tombol hapus [ ✖ ] saat dimensi dihover / dipilih
            if is_dim_hovered || is_dim_selected || is_dim_dragging {
                painter.rect_stroke(
                    label_bg_rect.expand(2.5),
                    CornerRadius::same(3),
                    Stroke::new(1.2, if is_dim_selected || is_dim_dragging { Color32::from_rgb(255, 140, 0) } else { Color32::from_rgb(0, 140, 255) }),
                    egui::StrokeKind::Outside,
                );

                let del_center = Pos2::new(label_bg_rect.max.x + 9.5, label_bg_rect.center().y);
                let del_color = if is_del_hovered {
                    Color32::from_rgb(230, 40, 40)
                } else {
                    Color32::from_rgba_premultiplied(185, 45, 45, 235)
                };
                painter.circle_filled(del_center, 7.0, del_color);
                painter.text(
                    del_center,
                    Align2::CENTER_CENTER,
                    ICON_CLOSE.codepoint,
                    FontId::proportional(10.0),
                    Color32::WHITE,
                );
            }
        }
    }

    // G2. Anotasi GD&T / toleransi (geometri yang sama dengan ekspor PDF/SVG).
    let annot_stroke = Stroke::new((0.25 * zoom).clamp(0.8, 2.0), Color32::BLACK);
    for annotation in &sheet.annotations {
        let Some(geometry) = ducad_io::drawing::gdt::annotation_geometry(annotation) else {
            continue;
        };
        let [ax, ay] = geometry.anchor;
        let at = |p: &[f32; 2]| mm_to_screen(ax + p[0], ay + p[1]);
        for path in &geometry.paths {
            let mut points: Vec<Pos2> = Vec::new();
            let mut closed = false;
            for cmd in &path.cmds {
                match cmd {
                    ducad_io::drawing::gdt::PathCmd::Move(a)
                    | ducad_io::drawing::gdt::PathCmd::Line(a) => points.push(at(a)),
                    ducad_io::drawing::gdt::PathCmd::Cubic(c1, c2, end) => {
                        let Some(start) = points.last().copied() else {
                            continue;
                        };
                        let (c1, c2, end) = (at(c1), at(c2), at(end));
                        for k in 1..=8 {
                            let t = k as f32 / 8.0;
                            let u = 1.0 - t;
                            let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
                            points.push(Pos2::new(
                                w[0] * start.x + w[1] * c1.x + w[2] * c2.x + w[3] * end.x,
                                w[0] * start.y + w[1] * c1.y + w[2] * c2.y + w[3] * end.y,
                            ));
                        }
                    }
                    ducad_io::drawing::gdt::PathCmd::Close => closed = true,
                }
            }
            if points.len() < 2 {
                continue;
            }
            if path.filled {
                painter.add(egui::Shape::convex_polygon(
                    points,
                    Color32::BLACK,
                    annot_stroke,
                ));
            } else if closed {
                painter.add(egui::Shape::closed_line(points, annot_stroke));
            } else {
                painter.add(egui::Shape::line(points, annot_stroke));
            }
        }
        for run in &geometry.texts {
            let align = if run.centered {
                egui::Align2::CENTER_BOTTOM
            } else {
                egui::Align2::LEFT_BOTTOM
            };
            painter.text(
                at(&run.pos),
                align,
                &run.text,
                FontId::proportional((run.size_mm * zoom).clamp(5.0, 40.0)),
                Color32::BLACK,
            );
        }
    }

    // H. Anotasi Teks Bebas (Custom Text Notes)
    for (idx, note) in sheet.custom_texts.iter().enumerate() {
        let is_editing = state.active_text_edit == Some(ActiveTextTarget::CustomText(idx));
        let is_hovered = state.hovered_text_idx == Some(idx);
        let is_selected = state.selected_text_idx == Some(idx);
        let is_del_hovered = state.hovered_text_delete == Some(idx);

        let p_top_left = mm_to_screen(note.position[0], note.position[1]);
        let font_sz = (note.font_size * zoom).clamp(7.0, 24.0);
        let font_text = FontId::proportional(font_sz);

        if !is_editing {
            let display_text = if note.text.is_empty() { "Ketik teks..." } else { &note.text };
            let galley = painter.layout_no_wrap(display_text.to_string(), font_text.clone(), Color32::BLACK);
            let text_rect = Rect::from_min_size(p_top_left - vec2(0.0, galley.size().y), galley.size() + vec2(8.0, 4.0));

            // Background highlight jika dipilih / di-hover
            if is_hovered || is_selected {
                painter.rect_filled(text_rect, CornerRadius::same(3), Color32::from_rgba_premultiplied(0, 140, 255, 25));
                painter.rect_stroke(
                    text_rect,
                    CornerRadius::same(3),
                    Stroke::new(1.2, if is_selected { Color32::from_rgb(255, 140, 0) } else { Color32::from_rgb(0, 140, 255) }),
                    egui::StrokeKind::Outside,
                );

                // Tombol Hapus [ ✖ ]
                let del_center = Pos2::new(text_rect.max.x + 9.5, text_rect.center().y);
                let del_color = if is_del_hovered {
                    Color32::from_rgb(230, 40, 40)
                } else {
                    Color32::from_rgba_premultiplied(185, 45, 45, 235)
                };
                painter.circle_filled(del_center, 7.0, del_color);
                painter.text(
                    del_center,
                    Align2::CENTER_CENTER,
                    ICON_CLOSE.codepoint,
                    FontId::proportional(10.0),
                    Color32::WHITE,
                );
            }

            let text_color = if note.text.is_empty() {
                Color32::from_rgb(140, 145, 160)
            } else {
                Color32::BLACK
            };
            painter.galley(p_top_left - vec2(-4.0, galley.size().y - 2.0), galley, text_color);
        }
    }

    // I. Tabel BOM (Bill of Materials ISO 7573)
    render_bom_table_screen(&painter, sheet, state, zoom, mm_to_screen);

    // J. Part Callout Balloons
    render_callout_balloons_screen(&painter, sheet, state, zoom, mm_to_screen);

    // K. Indikator Snap Point & Pengukuran Live (Tambah Data Ukuran Manual)
    if let Some(snap_mm) = active_snap_pt_mm {
        let p_snap = mm_to_screen(snap_mm[0], snap_mm[1]);
        painter.circle_stroke(p_snap, 5.0, Stroke::new(1.5, Color32::from_rgb(255, 140, 0)));
        painter.circle_filled(p_snap, 2.5, Color32::from_rgb(255, 140, 0));
    }

    if state.measure_tool_active {
        let p_cur = if let Some(snap_mm) = active_snap_pt_mm {
            mm_to_screen(snap_mm[0], snap_mm[1])
        } else if let Some(c_pos) = cursor_pos {
            c_pos
        } else {
            sheet_min
        };
        let p_cur_mm = screen_to_mm(p_cur);

        let font_meas = FontId::monospace((4.5 * zoom).clamp(7.0, 13.0));
        let measure_stroke = Stroke::new(1.5, Color32::from_rgb(255, 140, 0));
        let guide_stroke = Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 160, 40, 160));

        match state.dimension_mode {
            ManualDimensionMode::Linear => {
                if let Some(p1_mm) = state.measure_points.first().copied() {
                    let p1 = mm_to_screen(p1_mm[0], p1_mm[1]);
                    let p2 = p_cur;
                    let live_dist_mm = (p_cur_mm[0] - p1_mm[0]).hypot(p_cur_mm[1] - p1_mm[1]) / sheet.scale;

                    draw_dashed_line(&painter, p1, p2, measure_stroke, 4.0, 2.5);
                    painter.circle_filled(p1, 3.5, Color32::from_rgb(255, 140, 0));
                    painter.circle_filled(p2, 3.5, Color32::from_rgb(255, 140, 0));

                    let mid_p = Pos2::new((p1.x + p2.x) * 0.5, (p1.y + p2.y) * 0.5 - 12.0);
                    let galley = painter.layout_no_wrap(format!("{:.2} mm", live_dist_mm), font_meas, Color32::from_rgb(255, 180, 50));
                    let bg_rect = Rect::from_center_size(mid_p, galley.size() + vec2(6.0, 4.0));
                    painter.rect_filled(bg_rect, CornerRadius::same(3), Color32::from_rgba_premultiplied(30, 30, 30, 230));
                    painter.galley(bg_rect.min + vec2(3.0, 2.0), galley, Color32::from_rgb(255, 180, 50));
                }
            }
            ManualDimensionMode::Diameter => {
                if let Some(p1_mm) = state.measure_points.first().copied() {
                    let p1 = mm_to_screen(p1_mm[0], p1_mm[1]);
                    let p2 = p_cur;
                    let r_px = (p2 - p1).length();
                    let live_diam_mm = (p_cur_mm[0] - p1_mm[0]).hypot(p_cur_mm[1] - p1_mm[1]) * 2.0 / sheet.scale;

                    painter.circle_stroke(p1, r_px, guide_stroke);
                    painter.circle_filled(p1, 3.5, Color32::from_rgb(255, 140, 0));
                    painter.circle_filled(p2, 3.5, Color32::from_rgb(255, 140, 0));
                    draw_dashed_line(&painter, p1, p2, measure_stroke, 4.0, 2.5);

                    let mid_p = Pos2::new(p2.x + 14.0, p2.y - 12.0);
                    let galley = painter.layout_no_wrap(format!("Ø {:.2} mm", live_diam_mm), font_meas, Color32::from_rgb(255, 180, 50));
                    let bg_rect = Rect::from_center_size(mid_p, galley.size() + vec2(6.0, 4.0));
                    painter.rect_filled(bg_rect, CornerRadius::same(3), Color32::from_rgba_premultiplied(30, 30, 30, 230));
                    painter.galley(bg_rect.min + vec2(3.0, 2.0), galley, Color32::from_rgb(255, 180, 50));
                }
            }
            ManualDimensionMode::Radius => {
                if let Some(p1_mm) = state.measure_points.first().copied() {
                    let p1 = mm_to_screen(p1_mm[0], p1_mm[1]);
                    let p2 = p_cur;
                    let live_rad_mm = (p_cur_mm[0] - p1_mm[0]).hypot(p_cur_mm[1] - p1_mm[1]) / sheet.scale;

                    painter.circle_filled(p1, 3.5, Color32::from_rgb(255, 140, 0));
                    painter.circle_filled(p2, 3.5, Color32::from_rgb(255, 140, 0));
                    draw_dashed_line(&painter, p1, p2, measure_stroke, 4.0, 2.5);

                    let dir_vec = if (p2 - p1).length_sq() > 1e-4 {
                        (p2 - p1).normalized()
                    } else {
                        Vec2::new(1.0, 0.0)
                    };
                    let arrow_sz = (2.2 * zoom).clamp(3.5, 7.5);
                    draw_arrowhead(&painter, p2, dir_vec, arrow_sz, Color32::from_rgb(255, 140, 0));

                    let mid_p = Pos2::new(p2.x + 14.0, p2.y - 12.0);
                    let galley = painter.layout_no_wrap(format!("R {:.2} mm", live_rad_mm), font_meas, Color32::from_rgb(255, 180, 50));
                    let bg_rect = Rect::from_center_size(mid_p, galley.size() + vec2(6.0, 4.0));
                    painter.rect_filled(bg_rect, CornerRadius::same(3), Color32::from_rgba_premultiplied(30, 30, 30, 230));
                    painter.galley(bg_rect.min + vec2(3.0, 2.0), galley, Color32::from_rgb(255, 180, 50));
                }
            }
            ManualDimensionMode::Angle => {
                if state.measure_points.len() == 1 {
                    let p_v_mm = state.measure_points[0];
                    let p_v = mm_to_screen(p_v_mm[0], p_v_mm[1]);
                    painter.circle_filled(p_v, 3.5, Color32::from_rgb(255, 140, 0));
                    draw_dashed_line(&painter, p_v, p_cur, guide_stroke, 4.0, 2.5);
                } else if state.measure_points.len() >= 2 {
                    let p_v_mm = state.measure_points[0];
                    let p_a1_mm = state.measure_points[1];
                    let p_v = mm_to_screen(p_v_mm[0], p_v_mm[1]);
                    let p_a1 = mm_to_screen(p_a1_mm[0], p_a1_mm[1]);
                    let p_a2 = p_cur;

                    painter.circle_filled(p_v, 4.0, Color32::from_rgb(255, 140, 0));
                    painter.circle_filled(p_a1, 3.0, Color32::from_rgb(255, 140, 0));
                    painter.circle_filled(p_a2, 3.0, Color32::from_rgb(255, 140, 0));
                    painter.line_segment([p_v, p_a1], measure_stroke);
                    painter.line_segment([p_v, p_a2], measure_stroke);

                    let v1 = [p_a1_mm[0] - p_v_mm[0], p_a1_mm[1] - p_v_mm[1]];
                    let v2 = [p_cur_mm[0] - p_v_mm[0], p_cur_mm[1] - p_v_mm[1]];
                    let len1 = (v1[0] * v1[0] + v1[1] * v1[1]).sqrt();
                    let len2 = (v2[0] * v2[0] + v2[1] * v2[1]).sqrt();
                    let live_angle_deg = if len1 > 1e-3 && len2 > 1e-3 {
                        let dot = v1[0] * v2[0] + v1[1] * v2[1];
                        let cos_val = (dot / (len1 * len2)).clamp(-1.0, 1.0);
                        cos_val.acos().to_degrees()
                    } else {
                        0.0
                    };

                    let mid_p = Pos2::new((p_v.x + p_a2.x) * 0.5 + 14.0, (p_v.y + p_a2.y) * 0.5 - 12.0);
                    let galley = painter.layout_no_wrap(format!("{:.1}°", live_angle_deg), font_meas, Color32::from_rgb(255, 180, 50));
                    let bg_rect = Rect::from_center_size(mid_p, galley.size() + vec2(6.0, 4.0));
                    painter.rect_filled(bg_rect, CornerRadius::same(3), Color32::from_rgba_premultiplied(30, 30, 30, 230));
                    painter.galley(bg_rect.min + vec2(3.0, 2.0), galley, Color32::from_rgb(255, 180, 50));
                }
            }
        }
    }
}

/// Lukis primitif display-list (`drawing::scene`) ke kanvas egui.
///
/// `tint` menimpa warna (sorot/pilih). Busur dicacah halus; garis putus
/// memakai pola yang sama dengan ekspor.
fn paint_items(
    painter: &egui::Painter,
    items: &[Item],
    zoom: f32,
    to_screen: &dyn Fn([f32; 2]) -> Pos2,
    tint: Option<Color32>,
    textures: &[(u64, egui::TextureHandle)],
) {
    let color_of = |c: [u8; 3]| tint.unwrap_or(Color32::from_rgb(c[0], c[1], c[2]));
    let stroke_of = |pen: &ducad_io::drawing::scene::Pen| {
        Stroke::new((pen.width_mm * zoom).clamp(0.7, 6.0), color_of(pen.color))
    };
    let dashed = |painter: &egui::Painter, a: Pos2, b: Pos2, pen: &ducad_io::drawing::scene::Pen| {
        let stroke = stroke_of(pen);
        match pen.dash {
            Dash::Solid => {
                painter.line_segment([a, b], stroke);
            }
            Dash::Hidden => draw_dashed_line(painter, a, b, stroke, 2.0 * zoom, 1.0 * zoom),
            Dash::Center => draw_centerline(painter, a, b, stroke, 6.0 * zoom, 1.2 * zoom),
        }
    };
    for item in items {
        match item {
            Item::Line { a, b, pen } => dashed(painter, to_screen(*a), to_screen(*b), pen),
            Item::Arc {
                center,
                radius,
                start_deg,
                end_deg,
                pen,
            } => {
                let sweep = (end_deg - start_deg).clamp(0.0, 360.0);
                let n = ((sweep / 4.0).ceil() as usize).max(2);
                let pts: Vec<Pos2> = (0..=n)
                    .map(|i| {
                        let a = (start_deg + sweep * i as f32 / n as f32).to_radians();
                        to_screen([center[0] + radius * a.cos(), center[1] + radius * a.sin()])
                    })
                    .collect();
                if pen.dash == Dash::Solid {
                    painter.add(egui::Shape::line(pts, stroke_of(pen)));
                } else {
                    for w in pts.chunks(2) {
                        if w.len() == 2 {
                            painter.line_segment([w[0], w[1]], stroke_of(pen));
                        }
                    }
                }
            }
            Item::Circle {
                center,
                radius,
                pen,
                fill,
            } => {
                let c = to_screen(*center);
                let r = radius * zoom;
                if let Some(f) = fill {
                    painter.circle_filled(c, r, Color32::from_rgb(f[0], f[1], f[2]));
                }
                if pen.dash == Dash::Solid {
                    painter.circle_stroke(c, r, stroke_of(pen));
                } else {
                    let dashes = ((r * 0.5) as usize).clamp(12, 120);
                    draw_dashed_circle(painter, c, r, stroke_of(pen), dashes);
                }
            }
            Item::Rect { min, max, pen, fill } => {
                let rect = Rect::from_two_pos(to_screen(*min), to_screen(*max));
                if let Some(f) = fill {
                    painter.rect_filled(rect, CornerRadius::ZERO, Color32::from_rgb(f[0], f[1], f[2]));
                }
                if let Some(pen) = pen {
                    painter.rect_stroke(rect, CornerRadius::ZERO, stroke_of(pen), egui::StrokeKind::Middle);
                }
            }
            Item::Fill { points, color, .. } => {
                let pts: Vec<Pos2> = points.iter().map(|p| to_screen(*p)).collect();
                painter.add(egui::Shape::convex_polygon(pts, color_of(*color), Stroke::NONE));
            }
            Item::Text {
                pos,
                text,
                size_mm,
                bold: _,
                anchor,
                angle_deg,
                ..
            } => {
                let font = FontId::proportional((size_mm * 1.35 * zoom).clamp(4.0, 64.0));
                let galley = painter.layout_no_wrap(text.clone(), font, color_of([0, 0, 0]));
                let (w, h) = (galley.size().x, galley.size().y);
                let ax = match anchor {
                    Anchor::Start => 0.0,
                    Anchor::Middle => w * 0.5,
                    Anchor::End => w,
                };
                // Titik jangkar galley = (ax, garis dasar ≈ 82 % tinggi).
                let (sin, cos) = angle_deg.to_radians().sin_cos();
                let (vx, vy) = (ax, h * 0.82);
                let offset = vec2(vx * cos + vy * sin, -vx * sin + vy * cos);
                let origin = to_screen(*pos) - offset;
                let mut shape = egui::epaint::TextShape::new(origin, galley, color_of([0, 0, 0]));
                shape.angle = -angle_deg.to_radians();
                painter.add(shape);
            }
            Item::Image { min, max, index } => {
                let rect = Rect::from_two_pos(to_screen(*min), to_screen(*max));
                match textures.get(*index) {
                    Some((_, tex)) => {
                        painter.image(
                            tex.id(),
                            rect,
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    }
                    None => {
                        painter.rect_stroke(
                            rect,
                            CornerRadius::ZERO,
                            Stroke::new(1.0, Color32::from_rgb(150, 150, 150)),
                            egui::StrokeKind::Inside,
                        );
                    }
                }
            }
        }
    }
}

/// Render Kepala Gambar (Title Block) Standar ISO 7200 / DIN 6771 yang Presisi dan Interaktif Langsung.
fn render_title_block_screen<F>(
    painter: &egui::Painter,
    sheet: &DrawingSheet,
    state: &DrawingSheetViewState,
    zoom: f32,
    mm_to_screen: F,
) where
    F: Fn(f32, f32) -> Pos2,
{
    let tb = sheet.title_block_rect_mm();
    let info = &sheet.title_block;

    let p_bl = mm_to_screen(tb[0], tb[1]);
    let p_tr = mm_to_screen(tb[2], tb[3]);
    let tb_rect = Rect::from_two_pos(p_bl, p_tr);

    let stroke_thick = Stroke::new((0.5 * zoom).clamp(1.0, 1.8), Color32::BLACK);
    let stroke_thin = Stroke::new((0.3 * zoom).clamp(0.6, 1.0), Color32::BLACK);

    // 1. Kotak Luar Tebal
    painter.rect_stroke(tb_rect, CornerRadius::ZERO, stroke_thick, egui::StrokeKind::Inside);

    // 2. Garis Pembagi Horizontal
    let y_row1 = mm_to_screen(0.0, tb[1] + 9.0).y;
    let y_row2 = mm_to_screen(0.0, tb[1] + 18.0).y;
    let y_row3 = mm_to_screen(0.0, tb[1] + 32.0).y;

    painter.line_segment([Pos2::new(tb_rect.min.x, y_row1), Pos2::new(tb_rect.max.x, y_row1)], stroke_thin);
    painter.line_segment([Pos2::new(tb_rect.min.x, y_row2), Pos2::new(tb_rect.max.x, y_row2)], stroke_thick);
    painter.line_segment([Pos2::new(tb_rect.min.x, y_row3), Pos2::new(tb_rect.max.x, y_row3)], stroke_thin);

    // 3. Garis Pembagi Vertikal
    let x_col_top = mm_to_screen(tb[0] + 95.0, 0.0).x;
    painter.line_segment([Pos2::new(x_col_top, tb_rect.min.y), Pos2::new(x_col_top, y_row3)], stroke_thin);

    let x_col_mid = mm_to_screen(tb[0] + 85.0, 0.0).x;
    painter.line_segment([Pos2::new(x_col_mid, y_row3), Pos2::new(x_col_mid, y_row2)], stroke_thick);

    let x_rev_mid = mm_to_screen(tb[0] + 124.0, 0.0).x;
    painter.line_segment([Pos2::new(x_rev_mid, y_row3), Pos2::new(x_rev_mid, y_row2)], stroke_thin);

    let x_b1 = mm_to_screen(tb[0] + 45.0, 0.0).x;
    let x_b2 = mm_to_screen(tb[0] + 90.0, 0.0).x;
    let x_b3 = mm_to_screen(tb[0] + 115.0, 0.0).x;

    painter.line_segment([Pos2::new(x_b1, y_row2), Pos2::new(x_b1, y_row1)], stroke_thin);
    painter.line_segment([Pos2::new(x_b2, y_row2), Pos2::new(x_b2, tb_rect.max.y)], stroke_thin);
    painter.line_segment([Pos2::new(x_b3, y_row2), Pos2::new(x_b3, y_row1)], stroke_thin);

    // Highlight hovered field pada Title Block
    for field in TitleBlockFieldId::ALL {
        let is_hovered = state.hovered_tb_field == Some(field);
        let is_editing = state.active_text_edit == Some(ActiveTextTarget::TitleBlock(field));
        if is_hovered && !is_editing {
            let f_rect_mm = title_block_field_rect_mm(tb, field);
            let p1 = mm_to_screen(f_rect_mm[0], f_rect_mm[1]);
            let p2 = mm_to_screen(f_rect_mm[2], f_rect_mm[3]);
            let f_rect = Rect::from_two_pos(p1, p2);
            painter.rect_filled(f_rect, CornerRadius::same(2), Color32::from_rgba_premultiplied(0, 140, 255, 30));
            painter.rect_stroke(f_rect, CornerRadius::same(2), Stroke::new(1.0, Color32::from_rgb(0, 140, 255)), egui::StrokeKind::Outside);
        }
    }

    // 4. Tipografi & Konten Teks Proporsional (Skala Kertas)
    let font_caption = FontId::proportional((2.6 * zoom).clamp(4.0, 8.5));
    let font_val_sm = FontId::proportional((3.2 * zoom).clamp(4.8, 10.0));
    let font_val_md = FontId::proportional((4.0 * zoom).clamp(5.8, 12.0));
    let font_val_lg = FontId::proportional((5.2 * zoom).clamp(7.0, 14.5));

    let col_caption = Color32::from_rgb(110, 115, 130);
    let col_val = Color32::BLACK;

    let is_editing_field = |f: TitleBlockFieldId| -> bool {
        state.active_text_edit == Some(ActiveTextTarget::TitleBlock(f))
    };

    // A. Row 1: Perusahaan & Proyeksi
    if !is_editing_field(TitleBlockFieldId::CompanyName) {
        let p_comp = mm_to_screen(tb[0] + 3.0, tb[1] + 39.5);
        let comp_name = if info.company_name.is_empty() { "DUCAD Studio CAD/CAM" } else { &info.company_name };
        painter.text(p_comp, Align2::LEFT_CENTER, comp_name, font_val_md.clone(), col_val);
    }
    let p_comp_sub = mm_to_screen(tb[0] + 3.0, tb[1] + 35.0);
    painter.text(
        p_comp_sub,
        Align2::LEFT_CENTER,
        "LEMBAR KERJA GAMBAR TEKNIK (ISO 5457)",
        font_caption.clone(),
        col_caption,
    );

    // Simbol Proyeksi Sudut Ketiga (3rd Angle Projection Cone ISO)
    let p_proj = mm_to_screen(tb[0] + 117.0, tb[1] + 38.5);
    draw_3rd_angle_projection_symbol(painter, p_proj, zoom);

    // B. Row 2: Judul Komponen, Nomor Gambar, dan Revisi
    painter.text(
        mm_to_screen(tb[0] + 3.0, tb[1] + 27.5),
        Align2::LEFT_CENTER,
        "JUDUL GAMBAR / PART TITLE:",
        font_caption.clone(),
        col_caption,
    );
    if !is_editing_field(TitleBlockFieldId::ProjectTitle) {
        let proj_title = if info.project_title.is_empty() { "KOMPONEN UTAMA" } else { &info.project_title };
        painter.text(
            mm_to_screen(tb[0] + 3.0, tb[1] + 22.0),
            Align2::LEFT_CENTER,
            proj_title,
            font_val_lg,
            col_val,
        );
    }

    painter.text(
        mm_to_screen(tb[0] + 88.0, tb[1] + 27.5),
        Align2::LEFT_CENTER,
        "NO. GAMBAR / DWG NO:",
        font_caption.clone(),
        col_caption,
    );
    if !is_editing_field(TitleBlockFieldId::DrawingNumber) {
        let dwg_num = if info.drawing_number.is_empty() { "DWG-MODEL" } else { &info.drawing_number };
        painter.text(
            mm_to_screen(tb[0] + 88.0, tb[1] + 22.0),
            Align2::LEFT_CENTER,
            dwg_num,
            font_val_md.clone(),
            col_val,
        );
    }

    painter.text(
        mm_to_screen(tb[0] + 126.0, tb[1] + 27.5),
        Align2::LEFT_CENTER,
        "REV:",
        font_caption.clone(),
        col_caption,
    );
    if !is_editing_field(TitleBlockFieldId::Revision) {
        let rev = if info.revision.is_empty() { "A" } else { &info.revision };
        painter.text(
            mm_to_screen(tb[0] + 126.0, tb[1] + 22.0),
            Align2::LEFT_CENTER,
            rev,
            font_val_md.clone(),
            col_val,
        );
    }

    // C. Row 3: Drafter, Tanggal, Skala, Lembar
    painter.text(mm_to_screen(tb[0] + 3.0, tb[1] + 15.0), Align2::LEFT_CENTER, "DIGAMBAR:", font_caption.clone(), col_caption);
    if !is_editing_field(TitleBlockFieldId::DrawnBy) {
        let drafter = if info.drawn_by.is_empty() { "DUCAD Designer" } else { &info.drawn_by };
        painter.text(mm_to_screen(tb[0] + 3.0, tb[1] + 11.5), Align2::LEFT_CENTER, drafter, font_val_sm.clone(), col_val);
    }

    painter.text(mm_to_screen(tb[0] + 48.0, tb[1] + 15.0), Align2::LEFT_CENTER, "TANGGAL:", font_caption.clone(), col_caption);
    if !is_editing_field(TitleBlockFieldId::Date) {
        let date_str = if info.date.is_empty() { "2026-08-25" } else { &info.date };
        painter.text(mm_to_screen(tb[0] + 48.0, tb[1] + 11.5), Align2::LEFT_CENTER, date_str, font_val_sm.clone(), col_val);
    }

    painter.text(mm_to_screen(tb[0] + 93.0, tb[1] + 15.0), Align2::LEFT_CENTER, "SKALA:", font_caption.clone(), col_caption);
    if !is_editing_field(TitleBlockFieldId::Scale) {
        painter.text(mm_to_screen(tb[0] + 93.0, tb[1] + 11.5), Align2::LEFT_CENTER, &info.scale, font_val_sm.clone(), col_val);
    }

    painter.text(mm_to_screen(tb[0] + 118.0, tb[1] + 15.0), Align2::LEFT_CENTER, "LEMBAR:", font_caption.clone(), col_caption);
    if !is_editing_field(TitleBlockFieldId::SheetNumber) {
        let sheet_num = if info.sheet_number.is_empty() { "1 / 1" } else { &info.sheet_number };
        painter.text(mm_to_screen(tb[0] + 118.0, tb[1] + 11.5), Align2::LEFT_CENTER, sheet_num, font_val_sm.clone(), col_val);
    }

    // D. Row 4: Material & Toleransi
    painter.text(mm_to_screen(tb[0] + 3.0, tb[1] + 6.5), Align2::LEFT_CENTER, "MATERIAL:", font_caption.clone(), col_caption);
    if !is_editing_field(TitleBlockFieldId::Material) {
        let mat = if info.material.is_empty() { "Aluminium 6061-T6" } else { &info.material };
        painter.text(mm_to_screen(tb[0] + 3.0, tb[1] + 2.8), Align2::LEFT_CENTER, mat, font_val_sm.clone(), col_val);
    }

    painter.text(mm_to_screen(tb[0] + 93.0, tb[1] + 6.5), Align2::LEFT_CENTER, "TOLERANSI & SATUAN:", font_caption.clone(), col_caption);
    if !is_editing_field(TitleBlockFieldId::Units) {
        let unit_str = if info.units.is_empty() { "mm" } else { &info.units };
        painter.text(
            mm_to_screen(tb[0] + 93.0, tb[1] + 2.8),
            Align2::LEFT_CENTER,
            format!("ISO 2768-m | {}", unit_str),
            font_val_sm,
            col_val,
        );
    }
}

/// Gambar simbol proyeksi sudut ketiga (3rd Angle Projection Cone ISO standard).
fn draw_3rd_angle_projection_symbol(painter: &egui::Painter, center: Pos2, zoom: f32) {
    let s = (zoom * 0.85).clamp(0.8, 1.8);
    let stroke = Stroke::new(1.0 * s, Color32::BLACK);
    let cl_stroke = Stroke::new(0.6 * s, Color32::from_rgb(120, 120, 120));

    // Garis sumbu horizontal
    painter.line_segment([center - vec2(16.0 * s, 0.0), center + vec2(16.0 * s, 0.0)], cl_stroke);

    // Kerucut terpancung (trapezoid) di sebelah kiri
    let trap_cx = center.x - 7.0 * s;
    let p_tl = Pos2::new(trap_cx - 5.0 * s, center.y - 2.5 * s);
    let p_bl = Pos2::new(trap_cx - 5.0 * s, center.y + 2.5 * s);
    let p_tr = Pos2::new(trap_cx + 5.0 * s, center.y - 5.0 * s);
    let p_br = Pos2::new(trap_cx + 5.0 * s, center.y + 5.0 * s);

    painter.line_segment([p_tl, p_tr], stroke);
    painter.line_segment([p_tr, p_br], stroke);
    painter.line_segment([p_br, p_bl], stroke);
    painter.line_segment([p_bl, p_tl], stroke);

    // Dua lingkaran konsentris di sebelah kanan
    let circ_cx = center.x + 8.0 * s;
    let circ_c = Pos2::new(circ_cx, center.y);
    painter.circle_stroke(circ_c, 2.5 * s, stroke);
    painter.circle_stroke(circ_c, 5.0 * s, stroke);
}

/// Gambar panah terisi tajam untuk ujung garis dimensi.
fn draw_arrowhead(painter: &egui::Painter, tip: Pos2, dir: Vec2, size: f32, color: Color32) {
    let norm = dir.normalized();
    let perp = Vec2::new(-norm.y, norm.x);
    let back = tip - norm * size;
    let p_l = back + perp * (size * 0.32);
    let p_r = back - perp * (size * 0.32);

    let shape = egui::epaint::PathShape::convex_polygon(
        vec![tip, p_l, p_r],
        color,
        Stroke::NONE,
    );
    painter.add(shape);
}

/// Gambar garis sumbu simetri titik-strip panjang presisi (Centerline dash-dot `— · —`).
fn draw_centerline(
    painter: &egui::Painter,
    p1: Pos2,
    p2: Pos2,
    stroke: Stroke,
    dash_len: f32,
    gap_len: f32,
) {
    let dir = p2 - p1;
    let total_len = dir.length();
    if total_len < 1.0 {
        return;
    }
    let norm = dir / total_len;

    let dot_len = 1.0;

    let mut traveled = 0.0;
    while traveled < total_len {
        // 1. Long Dash
        let d_end = (traveled + dash_len).min(total_len);
        painter.line_segment([p1 + norm * traveled, p1 + norm * d_end], stroke);
        traveled += dash_len + gap_len;
        if traveled >= total_len {
            break;
        }

        // 2. Center Dot
        let dot_end = (traveled + dot_len).min(total_len);
        painter.line_segment([p1 + norm * traveled, p1 + norm * dot_end], stroke);
        traveled += dot_len + gap_len;
    }
}

/// Gambar garis putus-putus (dashed line) di egui.
fn draw_dashed_line(
    painter: &egui::Painter,
    p1: Pos2,
    p2: Pos2,
    stroke: Stroke,
    dash_len: f32,
    gap_len: f32,
) {
    let dir = p2 - p1;
    let total_len = dir.length();
    if total_len < 1.0 {
        return;
    }
    let norm = dir / total_len;

    let mut traveled = 0.0;
    while traveled < total_len {
        let start = p1 + norm * traveled;
        let end = p1 + norm * (traveled + dash_len).min(total_len);
        painter.line_segment([start, end], stroke);
        traveled += dash_len + gap_len;
    }
}

/// Gambar lingkaran putus-putus (dashed circle) di egui.
fn draw_dashed_circle(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    stroke: Stroke,
    num_dashes: usize,
) {
    if radius < 1.0 {
        return;
    }
    let n = num_dashes.max(8);
    let step = std::f32::consts::TAU / (n as f32);
    let dash_fraction = 0.65;
    let segments_per_dash = 4;

    for i in 0..n {
        let base_angle = (i as f32) * step;
        let dash_angle = step * dash_fraction;
        let sub_step = dash_angle / (segments_per_dash as f32);

        for j in 0..segments_per_dash {
            let a1 = base_angle + (j as f32) * sub_step;
            let a2 = base_angle + ((j + 1) as f32) * sub_step;
            let pt1 = Pos2::new(center.x + radius * a1.cos(), center.y + radius * a1.sin());
            let pt2 = Pos2::new(center.x + radius * a2.cos(), center.y + radius * a2.sin());
            painter.line_segment([pt1, pt2], stroke);
        }
    }
}

/// Render Tabel BOM (Bill of Materials ISO 7573) di atas Etiket Lembar Kerja 2D.
fn render_bom_table_screen<F>(
    painter: &egui::Painter,
    sheet: &DrawingSheet,
    state: &DrawingSheetViewState,
    zoom: f32,
    mm_to_screen: F,
) where
    F: Fn(f32, f32) -> Pos2,
{
    if !sheet.show_bom_table || sheet.bom_table.items.is_empty() {
        return;
    }

    let tb = sheet.bom_table_rect_mm();
    let col_w = sheet.bom_column_widths_mm();
    let title_h = sheet.bom_title_height_mm();
    let header_h = sheet.bom_header_height_mm();
    let row_h = sheet.bom_row_height_mm();

    let p_bl = mm_to_screen(tb[0], tb[1]);
    let p_tr = mm_to_screen(tb[2], tb[3]);
    let bom_rect = Rect::from_two_pos(p_bl, p_tr);

    let stroke_thick = Stroke::new((0.5 * zoom).clamp(1.0, 1.8), Color32::BLACK);
    let stroke_thin = Stroke::new((0.25 * zoom).clamp(0.6, 1.0), Color32::from_rgb(40, 45, 55));
    let stroke_highlight = Stroke::new(1.5, Color32::from_rgb(0, 140, 255));

    // Latar belakang putih tabel
    painter.rect_filled(bom_rect, CornerRadius::ZERO, Color32::WHITE);

    // 1. Kotak Luar Tebal
    painter.rect_stroke(bom_rect, CornerRadius::ZERO, stroke_thick, egui::StrokeKind::Inside);

    // 2. Baris Judul (Title Bar)
    let y_title_bot = tb[3] - title_h;
    let p_t1 = mm_to_screen(tb[0], y_title_bot);
    let p_t2 = mm_to_screen(tb[2], y_title_bot);
    painter.line_segment([p_t1, p_t2], stroke_thick);

    let title_rect = Rect::from_two_pos(mm_to_screen(tb[0], y_title_bot), mm_to_screen(tb[2], tb[3]));
    painter.rect_filled(title_rect, CornerRadius::ZERO, Color32::from_rgb(238, 242, 248));

    let title_text = if sheet.bom_table.title.is_empty() { "BILL OF MATERIALS" } else { &sheet.bom_table.title };
    let font_title = FontId::proportional((3.5 * zoom).clamp(8.5, 14.0));
    painter.text(
        Pos2::new(title_rect.min.x + 8.0, title_rect.center().y),
        Align2::LEFT_CENTER,
        title_text,
        font_title,
        Color32::BLACK,
    );

    // Tombol [ + Baris ] di pojok kanan baris judul
    let add_btn_rect = Rect::from_min_size(
        Pos2::new(title_rect.max.x - 55.0 * zoom.clamp(0.8, 1.2), title_rect.min.y + 2.0),
        vec2(50.0 * zoom.clamp(0.8, 1.2), title_rect.height() - 4.0),
    );
    let is_add_hover = state.hovered_bom_add_row;
    painter.rect_filled(
        add_btn_rect,
        CornerRadius::same(2),
        if is_add_hover { Color32::from_rgb(0, 150, 110) } else { Color32::from_rgba_premultiplied(40, 45, 55, 40) },
    );
    painter.text(
        add_btn_rect.center(),
        Align2::CENTER_CENTER,
        "+ Baris",
        FontId::proportional((2.6 * zoom).clamp(7.5, 11.0)),
        if is_add_hover { Color32::WHITE } else { Color32::from_rgb(30, 35, 45) },
    );

    // 3. Baris Header Kolom
    let y_header_bot = y_title_bot - header_h;
    let p_h1 = mm_to_screen(tb[0], y_header_bot);
    let p_h2 = mm_to_screen(tb[2], y_header_bot);
    painter.line_segment([p_h1, p_h2], stroke_thick);

    let header_rect = Rect::from_two_pos(mm_to_screen(tb[0], y_header_bot), mm_to_screen(tb[2], y_title_bot));
    painter.rect_filled(header_rect, CornerRadius::ZERO, Color32::from_rgb(224, 230, 240));

    let col_headers = ["ITEM", "PART NAME", "QTY", "MATERIAL", "DESCRIPTION"];
    let font_header = FontId::proportional((2.7 * zoom).clamp(7.0, 11.5));

    let mut cur_x = tb[0];
    for (i, &col_name) in col_headers.iter().enumerate() {
        let cw = col_w[i];
        let p_c_bl = mm_to_screen(cur_x, y_header_bot);
        let p_c_tr = mm_to_screen(cur_x + cw, y_title_bot);
        let c_rect = Rect::from_two_pos(p_c_bl, p_c_tr);

        if i == 0 || i == 2 {
            painter.text(c_rect.center(), Align2::CENTER_CENTER, col_name, font_header.clone(), Color32::BLACK);
        } else {
            painter.text(
                Pos2::new(c_rect.min.x + 3.0, c_rect.center().y),
                Align2::LEFT_CENTER,
                col_name,
                font_header.clone(),
                Color32::BLACK,
            );
        }

        if i > 0 {
            let p_vl_top = mm_to_screen(cur_x, y_title_bot);
            let p_vl_bot = mm_to_screen(cur_x, tb[1]);
            painter.line_segment([p_vl_top, p_vl_bot], stroke_thin);
        }
        cur_x += cw;
    }

    // 4. Data Rows
    let font_row = FontId::proportional((2.6 * zoom).clamp(7.0, 11.5));
    for (r_idx, item) in sheet.bom_table.items.iter().enumerate() {
        let y_row_top = y_header_bot - (r_idx as f32 * row_h);
        let y_row_bot = y_row_top - row_h;

        let row_p1 = mm_to_screen(tb[0], y_row_bot);
        let row_p2 = mm_to_screen(tb[2], y_row_bot);
        painter.line_segment([row_p1, row_p2], stroke_thin);

        let row_screen_rect = Rect::from_two_pos(row_p1, mm_to_screen(tb[2], y_row_top));

        // Highlight saat baris di-hover atau balon terkait di-hover/dipilih
        let is_row_hover = state.hovered_bom_row == Some(r_idx);
        let is_balloon_matched = state.hovered_balloon_id.or(state.selected_balloon_id).is_some_and(|b_id| {
            sheet.balloons.iter().find(|b| b.id == b_id).is_some_and(|b| b.item_number == item.item_number)
        });

        if is_row_hover || is_balloon_matched {
            let bg_col = if is_balloon_matched {
                Color32::from_rgba_premultiplied(255, 140, 0, 35)
            } else {
                Color32::from_rgba_premultiplied(0, 140, 255, 30)
            };
            painter.rect_filled(row_screen_rect, CornerRadius::ZERO, bg_col);
            painter.rect_stroke(row_screen_rect, CornerRadius::ZERO, stroke_highlight, egui::StrokeKind::Inside);

            // Tombol Hapus [ ✖ ] di tepi kanan baris
            let del_btn_center = Pos2::new(row_screen_rect.max.x - 7.0, row_screen_rect.center().y);
            let is_del_hover = state.hovered_bom_delete_row == Some(r_idx);
            painter.circle_filled(
                del_btn_center,
                6.0,
                if is_del_hover { Color32::from_rgb(230, 40, 40) } else { Color32::from_rgba_premultiplied(180, 40, 40, 200) },
            );
            painter.text(del_btn_center, Align2::CENTER_CENTER, "×", FontId::monospace(10.0), Color32::WHITE);
        }

        let vals = [
            format!("{}", item.item_number),
            item.part_name.clone(),
            format!("{}", item.quantity),
            item.material.clone(),
            item.description.clone(),
        ];

        let mut cell_x = tb[0];
        for (c_idx, val) in vals.iter().enumerate() {
            let cw = col_w[c_idx];
            let cell_bl = mm_to_screen(cell_x, y_row_bot);
            let cell_tr = mm_to_screen(cell_x + cw, y_row_top);
            let cell_rect = Rect::from_two_pos(cell_bl, cell_tr);

            if c_idx == 0 {
                // Item number: Tebal & Center
                painter.text(
                    cell_rect.center(),
                    Align2::CENTER_CENTER,
                    val,
                    FontId::proportional((2.8 * zoom).clamp(7.5, 12.0)),
                    if is_balloon_matched { Color32::from_rgb(220, 100, 0) } else { Color32::BLACK },
                );
            } else if c_idx == 2 {
                // Qty: Center
                painter.text(cell_rect.center(), Align2::CENTER_CENTER, val, font_row.clone(), Color32::BLACK);
            } else {
                // Teks: Left aligned
                let txt_pos = Pos2::new(cell_rect.min.x + 3.0, cell_rect.center().y);
                painter.text(txt_pos, Align2::LEFT_CENTER, val, font_row.clone(), Color32::BLACK);
            }
            cell_x += cw;
        }
    }
}

/// Render Part Callout Balloons yang terhubung ke bagian komponen pada tampak 2D/3D.
fn render_callout_balloons_screen<F>(
    painter: &egui::Painter,
    sheet: &DrawingSheet,
    state: &DrawingSheetViewState,
    zoom: f32,
    mm_to_screen: F,
) where
    F: Fn(f32, f32) -> Pos2,
{
    if !sheet.show_balloons || sheet.balloons.is_empty() {
        return;
    }

    for balloon in &sheet.balloons {
        let is_hovered = state.hovered_balloon_id == Some(balloon.id);
        let is_selected = state.selected_balloon_id == Some(balloon.id);
        let is_dragging = state.dragging_balloon_id == Some(balloon.id) || state.dragging_balloon_target_id == Some(balloon.id);

        // Cek apakah baris BOM yang dihover sesuai dengan nomor item balon ini
        let is_bom_row_matched = state.hovered_bom_row.is_some_and(|r_idx| {
            sheet.bom_table.items.get(r_idx).is_some_and(|it| it.item_number == balloon.item_number)
        });

        let p_target = mm_to_screen(balloon.target_point[0], balloon.target_point[1]);
        let p_center = mm_to_screen(balloon.balloon_pos[0], balloon.balloon_pos[1]);
        let r_px = (balloon.radius_mm * zoom).clamp(8.0, 40.0);

        let active_color = if is_hovered || is_selected || is_dragging || is_bom_row_matched {
            Color32::from_rgb(255, 140, 0)
        } else {
            Color32::BLACK
        };

        // 1. Leader Line dari p_target ke lingkar balon p_center
        let dir = p_target - p_center;
        let len = dir.length().max(1.0);
        let p_rim = p_center + (dir / len) * r_px;

        let stroke_line = Stroke::new(
            if is_hovered || is_selected || is_bom_row_matched { 1.5 } else { 1.0 },
            active_color,
        );
        painter.line_segment([p_target, p_rim], stroke_line);

        // 2. Target Arrowhead (panah penunjuk part)
        let arrow_dir = (p_target - p_rim).normalized();
        draw_arrowhead(painter, p_target, arrow_dir, (2.8 * zoom).clamp(5.0, 11.0), active_color);

        // 3. Titik target handle saat dipilih/dihover
        if is_hovered || is_selected {
            painter.circle_filled(p_target, 3.5, Color32::from_rgb(255, 140, 0));
        }

        // 4. Glowing halo saat dihover atau dipilih
        if is_hovered || is_selected || is_bom_row_matched {
            painter.circle_stroke(
                p_center,
                r_px + 3.0,
                Stroke::new(1.8, Color32::from_rgba_premultiplied(255, 140, 0, 150)),
            );
        }

        // 5. Lingkaran Balon Putih Masking
        painter.circle_filled(p_center, r_px, Color32::WHITE);
        painter.circle_stroke(p_center, r_px, stroke_line);

        // 6. Nomor Item di Tengah Balon
        let num_str = format!("{}", balloon.item_number);
        let font_num = FontId::proportional((balloon.radius_mm * 1.05 * zoom).clamp(9.0, 22.0));
        painter.text(p_center, Align2::CENTER_CENTER, num_str, font_num, active_color);

        // 7. Tombol Hapus [ ✖ ] saat dihover
        if is_hovered || is_selected {
            let del_center = Pos2::new(p_center.x + r_px * 0.7 + 6.0, p_center.y - r_px * 0.7 - 6.0);
            let is_del_hover = state.hovered_balloon_delete == Some(balloon.id);
            painter.circle_filled(
                del_center,
                6.5,
                if is_del_hover { Color32::from_rgb(230, 40, 40) } else { Color32::from_rgba_premultiplied(180, 40, 40, 220) },
            );
            painter.text(del_center, Align2::CENTER_CENTER, "×", FontId::monospace(10.0), Color32::WHITE);
        }
    }
}

#[cfg(test)]
mod zoom_tests {
    use super::*;

    #[test]
    fn zoom_about_keeps_anchor_fixed_on_paper() {
        let canvas = Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 800.0));
        let mut state = DrawingSheetViewState { zoom: 2.0, pan_offset: vec2(30.0, -20.0), ..Default::default() };
        let anchor = Pos2::new(700.0, 150.0);
        // Titik kertas (mm) di bawah anchor sebelum zoom.
        let paper_before = (anchor - canvas.center() - state.pan_offset) / state.zoom;

        zoom_about(&mut state, canvas, anchor, 1.5);

        assert!((state.zoom - 3.0).abs() < 1e-5);
        let paper_after = (anchor - canvas.center() - state.pan_offset) / state.zoom;
        assert!((paper_before - paper_after).length() < 1e-3);
    }

    #[test]
    fn zoom_about_clamps_and_leaves_pan_untouched_at_limit() {
        let canvas = Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 800.0));
        let mut state = DrawingSheetViewState { zoom: 8.0, pan_offset: vec2(5.0, 5.0), ..Default::default() };
        zoom_about(&mut state, canvas, canvas.center(), 2.0);
        assert_eq!(state.zoom, 8.0);
        assert_eq!(state.pan_offset, vec2(5.0, 5.0));
    }
}
