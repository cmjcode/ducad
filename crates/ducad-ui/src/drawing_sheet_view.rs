//! Komponen Kanvas Interaktif Lembar Kerja Gambar Teknik 2D (Engineering Drawing Sheet).
//!
//! Menyediakan kanvas gambar 2D presisi (kertas putih standar A4/A3 dengan bingkai teknik ISO),
//! kontrol skala gambar, tombol toggle garis tampak & tersembunyi, editor kepala gambar (title block),
//! serta tombol ekspor langsung ke PDF Vektor dan DXF CAD.

use ducad_io::drawing::{
    format_scale_ratio, DrawingSheet, PaperSize, TextAnnotation, TitleBlockInfo,
};
use ducad_kernel::{HlrLineKind, ProjectedViewKind};
use egui::{
    vec2, Align2, Color32, CornerRadius, FontId, Frame, Margin, Pos2, Rect, RichText, Sense,
    Stroke, Ui, Vec2,
};
use egui_material_icons::icons::{
    ICON_CLOSE, ICON_CONTENT_CUT, ICON_DOWNLOAD, ICON_EDIT_NOTE, ICON_FIT_SCREEN, ICON_GRID_VIEW,
    ICON_LAYERS, ICON_PICTURE_AS_PDF, ICON_REFRESH, ICON_SEARCH, ICON_STRAIGHTEN, ICON_TEXTURE,
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
    Close,
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

    pub fn get_mut_str<'a>(self, info: &'a mut TitleBlockInfo) -> &'a mut String {
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

/// Target elemen teks yang sedang aktif diedit secara live in-place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTextTarget {
    TitleBlock(TitleBlockFieldId),
    CustomText(usize),
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
    pub dragging_dim_idx: Option<usize>,
    pub hovered_dim_idx: Option<usize>,
    pub selected_dim_idx: Option<usize>,
    pub hovered_dim_delete: Option<usize>,
    pub measure_tool_active: bool,
    pub measure_first_pt: Option<[f32; 2]>,
    pub detail_tool_active: bool,
    pub dragging_detail_label: Option<char>,
    pub hovered_detail_label: Option<char>,
    pub hovered_detail_delete: Option<char>,
    pub selected_detail_label: Option<char>,
    pub detail_scale_multiplier: f32,
    pub detail_radius_mm: f32,
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
            dragging_dim_idx: None,
            hovered_dim_idx: None,
            selected_dim_idx: None,
            hovered_dim_delete: None,
            measure_tool_active: false,
            measure_first_pt: None,
            detail_tool_active: false,
            dragging_detail_label: None,
            hovered_detail_label: None,
            hovered_detail_delete: None,
            selected_detail_label: None,
            detail_scale_multiplier: 2.0,
            detail_radius_mm: 15.0,
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

        // Kumpulkan titik snap (ujung garis, titik pusat lingkaran/busur) dari seluruh tampak
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
                    snap_points_mm.push([
                        cx + (seg.start[0] - v_center[0]) * s,
                        cy + (seg.start[1] - v_center[1]) * s,
                    ]);
                    snap_points_mm.push([
                        cx + (seg.end[0] - v_center[0]) * s,
                        cy + (seg.end[1] - v_center[1]) * s,
                    ]);
                }
            }
            for feat in &view.features {
                match feat {
                    ducad_kernel::HlrGeometricFeature::Circle { center, .. }
                    | ducad_kernel::HlrGeometricFeature::Arc { center, .. }
                    | ducad_kernel::HlrGeometricFeature::Ellipse { center, .. } => {
                        snap_points_mm.push([
                            cx + (center[0] - v_center[0]) * s,
                            cy + (center[1] - v_center[1]) * s,
                        ]);
                    }
                    _ => {}
                }
            }
        }

        let cursor_pos = ui.input(|i| i.pointer.hover_pos());
        let is_over_ui = cursor_pos.map_or(false, |p| topbar_rect.contains(p) || zoom_controls_rect.contains(p));

        let mut hovered_view_kind = None;
        let mut hovered_dim_idx = None;
        let mut hovered_dim_delete = None;
        let mut hovered_tb_field = None;
        let mut hovered_text_idx = None;
        let mut hovered_text_delete = None;
        let mut hovered_detail_label = None;
        let mut hovered_detail_delete = None;
        let mut active_snap_pt_mm = None;

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

                // B. Custom Text Annotations Hit Test (Teks Bebas / Catatan)
                if hovered_tb_field.is_none() {
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
                if hovered_tb_field.is_none() && hovered_text_idx.is_none() {
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
                if state.measure_tool_active {
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

                // D. Dimension hit test (untuk geser posisi ukuran dan hapus satu per satu)
                if hovered_tb_field.is_none() && hovered_text_idx.is_none() && hovered_detail_label.is_none() && sheet.show_dimensions {
                    for (idx, dim) in sheet.auto_dimensions.iter().enumerate() {
                        let p1 = mm_to_screen(dim.start[0], dim.start[1]);
                        let p2 = mm_to_screen(dim.end[0], dim.end[1]);
                        let is_leader = dim.text.starts_with('R')
                            || dim.text.starts_with('Ø')
                            || dim.text.starts_with("Rx");
                        let is_angle = dim.text.ends_with('°');

                        let (text_center, line_hit_rect) = if is_angle {
                            let p_txt = mm_to_screen(dim.line_pos[0], dim.line_pos[1]);
                            (p_txt + vec2(20.0, 0.0), Rect::from_center_size(p_txt, vec2(24.0, 24.0)))
                        } else if is_leader {
                            let p_bend = mm_to_screen(dim.line_pos[0], dim.line_pos[1]);
                            let tc = Pos2::new(p_bend.x + 20.0, p_bend.y - 3.0 * zoom);
                            (tc, Rect::from_center_size(p_bend, vec2(28.0, 28.0)))
                        } else if dim.is_vertical {
                            let dim_x_px = mm_to_screen(dim.line_pos[0], 0.0).x;
                            let mid_y = (p1.y + p2.y) * 0.5;
                            let tc = Pos2::new(dim_x_px - 4.0 * zoom, mid_y);
                            let lr = Rect::from_min_max(
                                Pos2::new(dim_x_px - 8.0, p1.y.min(p2.y) - 4.0),
                                Pos2::new(dim_x_px + 8.0, p1.y.max(p2.y) + 4.0),
                            );
                            (tc, lr)
                        } else {
                            let dim_y_px = mm_to_screen(0.0, dim.line_pos[1]).y;
                            let mid_x = (p1.x + p2.x) * 0.5;
                            let tc = Pos2::new(mid_x, dim_y_px - 3.0 * zoom);
                            let lr = Rect::from_min_max(
                                Pos2::new(p1.x.min(p2.x) - 4.0, dim_y_px - 8.0),
                                Pos2::new(p1.x.max(p2.x) + 4.0, dim_y_px + 8.0),
                            );
                            (tc, lr)
                        };

                        let text_hit_rect = Rect::from_center_size(text_center, vec2(60.0, 24.0));
                        let del_btn_rect = Rect::from_center_size(text_center + vec2(35.0, 0.0), vec2(18.0, 18.0));

                        if del_btn_rect.contains(c_pos) {
                            hovered_dim_delete = Some(idx);
                            hovered_dim_idx = Some(idx);
                            break;
                        } else if text_hit_rect.contains(c_pos) || line_hit_rect.contains(c_pos) {
                            hovered_dim_idx = Some(idx);
                            break;
                        }
                    }
                }

                // E. View hit test (jika tidak sedang hover teks, detail, atau dimensi)
                if hovered_tb_field.is_none() && hovered_text_idx.is_none() && hovered_detail_label.is_none() && hovered_dim_idx.is_none() {
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
        state.hovered_dim_idx = hovered_dim_idx;
        state.hovered_dim_delete = hovered_dim_delete;
        state.hovered_tb_field = hovered_tb_field;
        state.hovered_text_idx = hovered_text_idx;
        state.hovered_text_delete = hovered_text_delete;
        state.hovered_detail_label = hovered_detail_label;
        state.hovered_detail_delete = hovered_detail_delete;

        // Interaction Handler
        if !is_over_ui {
            // Pintasan keyboard T untuk Tool Teks dan B untuk Detail View
            if state.active_text_edit.is_none() {
                if ui.input(|i| i.key_pressed(egui::Key::T)) {
                    state.text_tool_active = !state.text_tool_active;
                    if state.text_tool_active {
                        state.measure_tool_active = false;
                        state.detail_tool_active = false;
                    }
                }
                if ui.input(|i| i.key_pressed(egui::Key::B)) {
                    state.detail_tool_active = !state.detail_tool_active;
                    if state.detail_tool_active {
                        state.text_tool_active = false;
                        state.measure_tool_active = false;
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
                } else if let Some(sel_idx) = state.selected_dim_idx {
                    if sel_idx < sheet.auto_dimensions.len() {
                        sheet.auto_dimensions.remove(sel_idx);
                        state.selected_dim_idx = None;
                        state.hovered_dim_idx = None;
                        state.hovered_dim_delete = None;
                        state.dragging_dim_idx = None;
                    }
                }
            }

            if state.detail_tool_active {
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
                        if let Some(first_pt) = state.measure_first_pt {
                            let p1 = first_pt;
                            let p2 = pt;
                            let raw_dist_mm = (p2[0] - p1[0]).hypot(p2[1] - p1[1]) / sheet.scale;
                            if raw_dist_mm > 0.05 {
                                let is_vert = (p2[0] - p1[0]).abs() < (p2[1] - p1[1]).abs();
                                let mid = [(p1[0] + p2[0]) * 0.5, (p1[1] + p2[1]) * 0.5];
                                sheet.auto_dimensions.push(ducad_io::drawing::DimensionAnnotation {
                                    start: p1,
                                    end: p2,
                                    line_pos: mid,
                                    is_vertical: is_vert,
                                    text: format!("{:.2} mm", raw_dist_mm),
                                });
                            }
                            state.measure_first_pt = None;
                        } else {
                            state.measure_first_pt = Some(pt);
                        }
                    }
                }
            } else {
                // Klik untuk pilih/hapus teks, detail view, atau dimensi, atau tambah teks baru
                if response.clicked() {
                    if let Some(del_det) = state.hovered_detail_delete {
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
                    } else if let Some(del_idx) = state.hovered_dim_delete {
                        if del_idx < sheet.auto_dimensions.len() {
                            sheet.auto_dimensions.remove(del_idx);
                            state.selected_dim_idx = None;
                            state.hovered_dim_idx = None;
                            state.hovered_dim_delete = None;
                            state.dragging_dim_idx = None;
                        }
                    } else if let Some(lbl) = state.hovered_detail_label {
                        state.selected_detail_label = Some(lbl);
                        state.selected_text_idx = None;
                        state.selected_dim_idx = None;
                        state.active_text_edit = None;
                    } else if let Some(field) = state.hovered_tb_field {
                        state.active_text_edit = Some(ActiveTextTarget::TitleBlock(field));
                        state.selected_text_idx = None;
                        state.selected_dim_idx = None;
                        state.selected_detail_label = None;
                    } else if let Some(t_idx) = state.hovered_text_idx {
                        state.active_text_edit = Some(ActiveTextTarget::CustomText(t_idx));
                        state.selected_text_idx = Some(t_idx);
                        state.selected_dim_idx = None;
                        state.selected_detail_label = None;
                    } else if let Some(d_idx) = state.hovered_dim_idx {
                        state.selected_dim_idx = Some(d_idx);
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
                            state.selected_dim_idx = None;
                            state.selected_detail_label = None;
                        }
                    } else {
                        state.selected_dim_idx = None;
                        state.selected_text_idx = None;
                        state.selected_detail_label = None;
                        state.active_text_edit = None;
                    }
                }

                // Drag and drop geser teks, detail circle, ukuran, atau tampak
                if response.drag_started_by(egui::PointerButton::Primary) && !ui.input(|i| i.modifiers.alt) {
                    if state.hovered_dim_delete.is_none() && state.hovered_text_delete.is_none() && state.hovered_detail_delete.is_none() {
                        if state.hovered_detail_label.is_some() {
                            state.dragging_detail_label = state.hovered_detail_label;
                            state.selected_detail_label = state.hovered_detail_label;
                        } else if state.hovered_text_idx.is_some() && state.active_text_edit.is_none() {
                            state.dragging_text_idx = state.hovered_text_idx;
                            state.selected_text_idx = state.hovered_text_idx;
                        } else if state.hovered_dim_idx.is_some() {
                            state.dragging_dim_idx = state.hovered_dim_idx;
                            state.selected_dim_idx = state.hovered_dim_idx;
                        } else if state.hovered_tb_field.is_none() {
                            state.dragging_view = state.hovered_view;
                        }
                    }
                }

                if response.dragged_by(egui::PointerButton::Primary) && !ui.input(|i| i.modifiers.alt) {
                    if let Some(lbl) = state.dragging_detail_label {
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
                    } else if let Some(d_idx) = state.dragging_dim_idx {
                        if let Some(dim) = sheet.auto_dimensions.get_mut(d_idx) {
                            let delta_x = response.drag_delta().x / zoom;
                            let delta_y = -response.drag_delta().y / zoom;
                            let is_radial_leader = dim.text.starts_with('R')
                                || dim.text.starts_with('Ø')
                                || dim.text.starts_with("Rx")
                                || dim.text.ends_with('°');
                            if is_radial_leader {
                                dim.line_pos[0] += delta_x;
                                dim.line_pos[1] += delta_y;
                            } else if dim.is_vertical {
                                dim.line_pos[0] += delta_x;
                            } else {
                                dim.line_pos[1] += delta_y;
                            }
                        }
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    } else if let Some(kind) = state.dragging_view {
                        if let Some(plc) = sheet.view_placements.iter_mut().find(|p| p.kind == kind) {
                            let delta_x = response.drag_delta().x / zoom;
                            let delta_y = -response.drag_delta().y / zoom;
                            plc.center_mm[0] += delta_x;
                            plc.center_mm[1] += delta_y;
                            sheet.generate_auto_dimensions();
                        }
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    }
                }

                if response.drag_stopped() {
                    state.dragging_detail_label = None;
                    state.dragging_text_idx = None;
                    state.dragging_dim_idx = None;
                    state.dragging_view = None;
                }

                // Pan canvas
                if response.dragged_by(egui::PointerButton::Middle)
                    || (response.dragged_by(egui::PointerButton::Primary) && ui.input(|i| i.modifiers.alt))
                    || (response.dragged_by(egui::PointerButton::Primary)
                        && state.dragging_detail_label.is_none()
                        && state.hovered_detail_label.is_none()
                        && state.dragging_view.is_none()
                        && state.hovered_view.is_none()
                        && state.dragging_dim_idx.is_none()
                        && state.hovered_dim_idx.is_none()
                        && state.dragging_text_idx.is_none()
                        && state.hovered_text_idx.is_none()
                        && state.hovered_tb_field.is_none())
                {
                    state.pan_offset += response.drag_delta();
                }

                if state.hovered_dim_delete.is_some() || state.hovered_text_delete.is_some() || state.hovered_detail_delete.is_some() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                } else if state.text_tool_active || state.hovered_tb_field.is_some() || state.hovered_text_idx.is_some() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
                } else if (state.hovered_detail_label.is_some() && state.dragging_detail_label.is_none())
                    || (state.hovered_dim_idx.is_some() && state.dragging_dim_idx.is_none())
                    || (state.hovered_view.is_some() && state.dragging_view.is_none())
                {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                }
            }

            let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll_delta.abs() > 0.0 && response.hovered() {
                let zoom_factor = if scroll_delta > 0.0 { 1.1 } else { 0.9 };
                state.zoom = (state.zoom * zoom_factor).clamp(0.15, 8.0);
            }
        }

        // Render Lembar Kertas & Konten Gambar 2D
        render_sheet_canvas(ui, canvas_rect, state, sheet, active_snap_pt_mm, cursor_pos);

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
                            res.request_focus();
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
                                res.request_focus();
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
                    .show_ui(ui, |ui| {
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
                    });

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
                    ICON_STRAIGHTEN.codepoint,
                    state.measure_tool_active,
                    "Tambah Ukuran Baru (Ukur)",
                    Some("M"),
                    Some("Klik 2 titik pada gambar untuk menambah dimensi ukuran baru secara manual"),
                    Some(Color32::from_rgba_premultiplied(18, 42, 85, 100)),
                    Some(Color32::from_rgb(255, 140, 0)),
                );
                if measure_btn.clicked() {
                    state.measure_tool_active = !state.measure_tool_active;
                    if state.measure_tool_active {
                        state.text_tool_active = false;
                    }
                    state.measure_first_pt = None;
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
                    }
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

                    // Tombol Ekspor PDF Vektor
                    let pdf_btn = header_icon_btn(
                        ui,
                        ICON_PICTURE_AS_PDF.codepoint,
                        false,
                        "Ekspor PDF Vektor",
                        None,
                        Some("Cetak dokumen gambar teknik presisi ke file PDF"),
                        None,
                        Some(ACCENT_BLUE),
                    );
                    if pdf_btn.clicked() {
                        event = Some(DrawingSheetEvent::ExportPdf);
                    }

                    // Tombol Ekspor DXF CAD
                    let dxf_btn = header_icon_btn(
                        ui,
                        ICON_DOWNLOAD.codepoint,
                        false,
                        "Ekspor DXF CAD",
                        None,
                        Some("Ekspor vektor 2D ke format CAD DXF"),
                        None,
                        None,
                    );
                    if dxf_btn.clicked() {
                        event = Some(DrawingSheetEvent::ExportDxf);
                    }
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

    // C. Bingkai Gambar Dalam & Luar
    let (_outer, inner) = sheet.border_rects_mm();

    let p_inner_bl = mm_to_screen(inner[0], inner[1]);
    let p_inner_tr = mm_to_screen(inner[2], inner[3]);
    let inner_rect = Rect::from_two_pos(p_inner_bl, p_inner_tr);

    // Bingkai dalam tebal (0.7mm)
    painter.rect_stroke(
        inner_rect,
        CornerRadius::ZERO,
        Stroke::new((0.7 * zoom).max(1.5), Color32::BLACK),
        egui::StrokeKind::Inside,
    );

    // Bingkai luar tipis
    painter.rect_stroke(
        sheet_rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, Color32::from_rgb(180, 180, 180)),
        egui::StrokeKind::Inside,
    );

    // D. Grid Zona Referensi (1..6 dan A..D)
    let cols = 6;
    let rows = 4;
    let col_w = (inner[2] - inner[0]) / cols as f32;
    let row_h = (inner[3] - inner[1]) / rows as f32;

    let font_zone = FontId::monospace((7.0 * zoom).clamp(8.0, 14.0));

    for c in 0..cols {
        let x_mm = inner[0] + (c as f32 + 0.5) * col_w;
        let pt_top = mm_to_screen(x_mm, inner[3] + 2.0);
        let pt_bot = mm_to_screen(x_mm, inner[1] - 4.5);
        let txt = format!("{}", c + 1);
        painter.text(pt_top, Align2::CENTER_BOTTOM, &txt, font_zone.clone(), Color32::BLACK);
        painter.text(pt_bot, Align2::CENTER_TOP, &txt, font_zone.clone(), Color32::BLACK);
    }

    let row_chars = ["A", "B", "C", "D"];
    for r in 0..rows {
        let y_mm = inner[1] + (r as f32 + 0.5) * row_h;
        let pt_left = mm_to_screen(inner[0] - 5.0, y_mm);
        let pt_right = mm_to_screen(inner[2] + 4.0, y_mm);
        let txt = row_chars[r.min(3)];
        painter.text(pt_left, Align2::RIGHT_CENTER, txt, font_zone.clone(), Color32::BLACK);
        painter.text(pt_right, Align2::LEFT_CENTER, txt, font_zone.clone(), Color32::BLACK);
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
            let badge_text = if is_dragging { "✥ Menggeser..." } else { "✥ Tahan & Geser Tata Letak" };
            let badge_galley = painter.layout_no_wrap(
                badge_text.to_string(),
                FontId::proportional((3.5 * zoom).clamp(8.0, 11.0)),
                Color32::WHITE,
            );
            let badge_rect = Rect::from_min_size(badge_pos, badge_galley.size() + vec2(8.0, 4.0));
            painter.rect_filled(badge_rect, CornerRadius::same(3), Color32::from_rgb(25, 95, 210));
            painter.galley(badge_pos + vec2(4.0, 2.0), badge_galley, Color32::WHITE);
        }

        // 1. Centerlines (Garis Sumbu Simetri)
        if sheet.show_centerlines {
            let cl_stroke = Stroke::new((0.35 * zoom).clamp(0.8, 1.5), Color32::from_rgb(0, 130, 45));
            for cl in &view.centerlines {
                let p1 = mm_to_screen(
                    center_mm[0] + (cl.start[0] - v_center[0]) * scale,
                    center_mm[1] + (cl.start[1] - v_center[1]) * scale,
                );
                let p2 = mm_to_screen(
                    center_mm[0] + (cl.end[0] - v_center[0]) * scale,
                    center_mm[1] + (cl.end[1] - v_center[1]) * scale,
                );
                draw_centerline(&painter, p1, p2, cl_stroke, 8.0 * zoom, 2.0 * zoom);
            }
        }

        // 2. Hidden Lines (Garis Tersembunyi Putus-putus)
        if sheet.show_hidden_lines {
            let hidden_stroke = Stroke::new((0.35 * zoom).clamp(0.8, 1.2), Color32::from_rgb(110, 115, 130));
            for seg in &view.segments {
                if seg.kind == HlrLineKind::Hidden {
                    let p1 = mm_to_screen(
                        center_mm[0] + (seg.start[0] - v_center[0]) * scale,
                        center_mm[1] + (seg.start[1] - v_center[1]) * scale,
                    );
                    let p2 = mm_to_screen(
                        center_mm[0] + (seg.end[0] - v_center[0]) * scale,
                        center_mm[1] + (seg.end[1] - v_center[1]) * scale,
                    );
                    draw_dashed_line(&painter, p1, p2, hidden_stroke, 4.0 * zoom, 2.5 * zoom);
                }
            }
        }

        // 3. Garis Arsir Potongan 45° ISO/ANSI (Hatch Pattern)
        if sheet.show_hatch {
            let hatch_stroke = Stroke::new((0.35 * zoom).clamp(0.6, 1.2), Color32::from_rgb(70, 95, 125));
            for seg in &view.segments {
                if seg.kind == HlrLineKind::Hatch {
                    let p1 = mm_to_screen(
                        center_mm[0] + (seg.start[0] - v_center[0]) * scale,
                        center_mm[1] + (seg.start[1] - v_center[1]) * scale,
                    );
                    let p2 = mm_to_screen(
                        center_mm[0] + (seg.end[0] - v_center[0]) * scale,
                        center_mm[1] + (seg.end[1] - v_center[1]) * scale,
                    );
                    painter.line_segment([p1, p2], hatch_stroke);
                }
            }
        }

        // 4. Visible Lines & Silhouettes (Garis Tampak Tebal Solid ISO 128)
        let visible_stroke = Stroke::new((0.60 * zoom).clamp(1.2, 2.4), Color32::BLACK);
        for seg in &view.segments {
            if seg.kind == HlrLineKind::Visible || seg.kind == HlrLineKind::Silhouette {
                let p1 = mm_to_screen(
                    center_mm[0] + (seg.start[0] - v_center[0]) * scale,
                    center_mm[1] + (seg.start[1] - v_center[1]) * scale,
                );
                let p2 = mm_to_screen(
                    center_mm[0] + (seg.end[0] - v_center[0]) * scale,
                    center_mm[1] + (seg.end[1] - v_center[1]) * scale,
                );
                painter.line_segment([p1, p2], visible_stroke);
            }
        }

        // 4b. Bingkai Lingkaran Viewport untuk Tampak Detail (Detail View Circle Viewport)
        if let ProjectedViewKind::Detail(_) = plc.kind {
            let r_px = view_sz[0] * 0.5 * scale * zoom;
            let p_center = mm_to_screen(center_mm[0], center_mm[1]);
            painter.circle_stroke(
                p_center,
                r_px,
                Stroke::new((0.8 * zoom).clamp(1.4, 2.8), Color32::from_rgb(20, 24, 35)),
            );
        }

        // 5. Indikator Garis Potong Panah A-A pada Tampak Acuan (Top View)
        if plc.kind == ProjectedViewKind::Top {
            if let Some(ind) = &sheet.drawing.cutting_plane {
                let p1 = mm_to_screen(
                    center_mm[0] + (ind.start[0] - v_center[0]) * scale,
                    center_mm[1] + (ind.start[1] - v_center[1]) * scale,
                );
                let p2 = mm_to_screen(
                    center_mm[0] + (ind.end[0] - v_center[0]) * scale,
                    center_mm[1] + (ind.end[1] - v_center[1]) * scale,
                );

                // Garis putus-putus tengah
                let cut_dash_stroke = Stroke::new((0.4 * zoom).clamp(0.8, 1.4), Color32::from_rgb(50, 50, 60));
                draw_dashed_line(&painter, p1, p2, cut_dash_stroke, 6.0 * zoom, 3.0 * zoom);

                // Ujung garis tebal ISO
                let thick_stroke = Stroke::new((1.5 * zoom).clamp(2.0, 3.5), Color32::from_rgb(20, 20, 30));
                let end_len_px = (6.0 * scale * zoom).clamp(10.0, 30.0);
                painter.line_segment([p1, Pos2::new(p1.x + end_len_px, p1.y)], thick_stroke);
                painter.line_segment([p2, Pos2::new(p2.x - end_len_px, p2.y)], thick_stroke);

                // Panah pandangan potong A-A
                let arr_len_px = (6.0 * scale * zoom).clamp(12.0, 26.0);
                let arr1_top = Pos2::new(p1.x, p1.y - arr_len_px);
                let arr2_top = Pos2::new(p2.x, p2.y - arr_len_px);
                painter.line_segment([p1, arr1_top], thick_stroke);
                painter.line_segment([p2, arr2_top], thick_stroke);

                let arr_sz = (2.6 * zoom).clamp(4.0, 8.5);
                draw_arrowhead(&painter, arr1_top, Vec2::new(0.0, -1.0), arr_sz, Color32::from_rgb(20, 20, 30));
                draw_arrowhead(&painter, arr2_top, Vec2::new(0.0, -1.0), arr_sz, Color32::from_rgb(20, 20, 30));

                // Huruf teks label 'A'
                let font_lbl = FontId::proportional((5.5 * zoom).clamp(9.0, 16.0));
                painter.text(Pos2::new(p1.x - 10.0, arr1_top.y - 2.0), Align2::RIGHT_CENTER, &ind.label, font_lbl.clone(), Color32::from_rgb(20, 20, 30));
                painter.text(Pos2::new(p2.x + 10.0, arr2_top.y - 2.0), Align2::LEFT_CENTER, &ind.label, font_lbl, Color32::from_rgb(20, 20, 30));
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
                let rim_pt = Pos2::new(p_center.x + r_px * 0.7071, p_center.y - r_px * 0.7071);

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
                    painter.text(del_pos, Align2::CENTER_CENTER, "×", FontId::proportional(11.0 * zoom.clamp(0.8, 1.2)), Color32::WHITE);
                }
            }
        }

        // Judul Tampak Profesional di bawah view (proporsional & elegan)
        let title_y_mm = center_mm[1] - (view_sz[1] * scale * 0.5) - 7.5;
        let title_pos = mm_to_screen(center_mm[0], title_y_mm);
        let title_sub_pos = mm_to_screen(center_mm[0], title_y_mm - 3.8);

        let font_title = FontId::proportional((4.2 * zoom).clamp(5.5, 12.0));
        let font_sub = FontId::proportional((3.5 * zoom).clamp(4.5, 9.5));

        let (sub_label, scale_label) = match plc.kind {
            ProjectedViewKind::Front => ("FRONT VIEW", format!("SKALA {}", sheet.title_block.scale)),
            ProjectedViewKind::Top => ("TOP VIEW", format!("SKALA {}", sheet.title_block.scale)),
            ProjectedViewKind::Right => ("RIGHT SIDE VIEW", format!("SKALA {}", sheet.title_block.scale)),
            ProjectedViewKind::Isometric => ("ISOMETRIC 3D", format!("SKALA {}", sheet.title_block.scale)),
            ProjectedViewKind::SectionAA => ("SECTION A-A", format!("SKALA {}", sheet.title_block.scale)),
            ProjectedViewKind::Detail(_) => ("DETAIL VIEW", format!("SKALA {}", format_scale_ratio(plc.scale))),
        };

        painter.text(
            title_pos,
            Align2::CENTER_TOP,
            &format!("{} | {}", view.title, sub_label),
            font_title,
            Color32::from_rgb(20, 24, 35),
        );
        painter.text(
            title_sub_pos,
            Align2::CENTER_TOP,
            &scale_label,
            font_sub,
            Color32::from_rgb(100, 105, 120),
        );
    }

    // G. Anotasi Dimensi Presisi dengan Panah Terisi (Filled Arrowheads) & Extension Lines
    if sheet.show_dimensions {
        let font_dim = FontId::monospace((4.2 * zoom).clamp(5.5, 11.5));
        let arrow_sz = (2.2 * zoom).clamp(3.5, 7.5);

        for (idx, dim) in sheet.auto_dimensions.iter().enumerate() {
            let p1 = mm_to_screen(dim.start[0], dim.start[1]);
            let p2 = mm_to_screen(dim.end[0], dim.end[1]);

            let is_leader = dim.text.starts_with('R')
                || dim.text.starts_with('Ø')
                || dim.text.starts_with("Rx");
            let is_angle = dim.text.ends_with('°');

            let is_dim_hovered = state.hovered_dim_idx == Some(idx);
            let is_dim_selected = state.selected_dim_idx == Some(idx);
            let is_dim_dragging = state.dragging_dim_idx == Some(idx);
            let is_del_hovered = state.hovered_dim_delete == Some(idx);

            let active_dim_color = if is_dim_selected || is_dim_dragging {
                Color32::from_rgb(255, 135, 15)
            } else if is_dim_hovered {
                Color32::from_rgb(0, 110, 230)
            } else {
                Color32::from_rgb(12, 70, 175)
            };

            let cur_dim_stroke = Stroke::new(
                if is_dim_selected || is_dim_dragging || is_dim_hovered {
                    (0.55 * zoom).clamp(1.2, 2.0)
                } else {
                    (0.35 * zoom).clamp(0.8, 1.5)
                },
                active_dim_color,
            );

            let label_bg_rect: Rect;

            if is_angle {
                let p_v = p1;
                let p_a1 = p2;
                let p_txt = mm_to_screen(dim.line_pos[0], dim.line_pos[1]);

                painter.line_segment([p_v, p_a1], cur_dim_stroke);
                painter.line_segment([p_v, p_txt], cur_dim_stroke);

                let dir_vec = (p_txt - p_v).normalized();
                draw_arrowhead(&painter, p_txt, dir_vec, arrow_sz, active_dim_color);

                let galley = painter.layout_no_wrap(dim.text.clone(), font_dim.clone(), active_dim_color);
                label_bg_rect = Rect::from_center_size(p_txt + vec2(galley.size().x * 0.5 + 4.0, 0.0), galley.size() + vec2(4.0, 2.0));
                painter.rect_filled(label_bg_rect, CornerRadius::same(2), Color32::from_rgba_premultiplied(252, 252, 252, 240));
                painter.galley(Pos2::new(label_bg_rect.min.x + 2.0, label_bg_rect.min.y + 1.0), galley, active_dim_color);
            } else if is_leader {
                let p_start = p1;
                let p_end = p2;
                let p_bend = mm_to_screen(dim.line_pos[0], dim.line_pos[1]);
                let p_shoulder = Pos2::new(p_bend.x + 12.0 * zoom.clamp(0.8, 1.5), p_bend.y);

                painter.line_segment([p_start, p_end], cur_dim_stroke);
                painter.line_segment([p_end, p_bend], cur_dim_stroke);
                painter.line_segment([p_bend, p_shoulder], cur_dim_stroke);

                let dir_vec = p_end - p_start;
                let dir_norm = if dir_vec.length_sq() > 1e-4 {
                    dir_vec.normalized()
                } else {
                    Vec2::new(1.0, 0.0)
                };
                draw_arrowhead(&painter, p_end, dir_norm, arrow_sz, active_dim_color);

                let txt_pos = Pos2::new(p_bend.x + 2.0, p_bend.y - 3.0 * zoom);
                let galley = painter.layout_no_wrap(dim.text.clone(), font_dim.clone(), active_dim_color);
                label_bg_rect = Rect::from_center_size(txt_pos + Vec2::new(galley.size().x * 0.5, 0.0), galley.size() + vec2(4.0, 2.0));
                painter.rect_filled(label_bg_rect, CornerRadius::same(2), Color32::from_rgba_premultiplied(252, 252, 252, 240));
                painter.galley(Pos2::new(label_bg_rect.min.x + 2.0, label_bg_rect.min.y + 1.0), galley, active_dim_color);
            } else if dim.is_vertical {
                let dim_x_px = mm_to_screen(dim.line_pos[0], 0.0).x;
                let ext_overshoot = 2.0 * zoom;
                let ext_dir = if dim_x_px < p1.x { -1.0 } else { 1.0 };

                let ext1_start = Pos2::new(p1.x + ext_dir * 1.5 * zoom, p1.y);
                let ext1_end = Pos2::new(dim_x_px + ext_dir * ext_overshoot, p1.y);
                let ext2_start = Pos2::new(p2.x + ext_dir * 1.5 * zoom, p2.y);
                let ext2_end = Pos2::new(dim_x_px + ext_dir * ext_overshoot, p2.y);

                painter.line_segment([ext1_start, ext1_end], cur_dim_stroke);
                painter.line_segment([ext2_start, ext2_end], cur_dim_stroke);

                let line_top = Pos2::new(dim_x_px, p1.y.min(p2.y));
                let line_bot = Pos2::new(dim_x_px, p1.y.max(p2.y));
                painter.line_segment([line_top, line_bot], cur_dim_stroke);

                draw_arrowhead(&painter, line_top, Vec2::new(0.0, -1.0), arrow_sz, active_dim_color);
                draw_arrowhead(&painter, line_bot, Vec2::new(0.0, 1.0), arrow_sz, active_dim_color);

                let mid_y = (p1.y + p2.y) * 0.5;
                let txt_pos = Pos2::new(dim_x_px - 4.0 * zoom, mid_y);

                let galley = painter.layout_no_wrap(dim.text.clone(), font_dim.clone(), active_dim_color);
                label_bg_rect = Rect::from_center_size(txt_pos - Vec2::new(galley.size().x * 0.5, 0.0), galley.size() + vec2(4.0, 2.0));
                painter.rect_filled(label_bg_rect, CornerRadius::same(2), Color32::from_rgba_premultiplied(252, 252, 252, 240));
                painter.galley(Pos2::new(label_bg_rect.min.x + 2.0, label_bg_rect.min.y + 1.0), galley, active_dim_color);
            } else {
                let dim_y_px = mm_to_screen(0.0, dim.line_pos[1]).y;
                let ext_overshoot = 2.0 * zoom;
                let ext_dir = if dim_y_px > p1.y { 1.0 } else { -1.0 };

                let ext1_start = Pos2::new(p1.x, p1.y + ext_dir * 1.5 * zoom);
                let ext1_end = Pos2::new(p1.x, dim_y_px + ext_dir * ext_overshoot);
                let ext2_start = Pos2::new(p2.x, p2.y + ext_dir * 1.5 * zoom);
                let ext2_end = Pos2::new(p2.x, dim_y_px + ext_dir * ext_overshoot);

                painter.line_segment([ext1_start, ext1_end], cur_dim_stroke);
                painter.line_segment([ext2_start, ext2_end], cur_dim_stroke);

                let line_left = Pos2::new(p1.x.min(p2.x), dim_y_px);
                let line_right = Pos2::new(p1.x.max(p2.x), dim_y_px);
                painter.line_segment([line_left, line_right], cur_dim_stroke);

                draw_arrowhead(&painter, line_left, Vec2::new(-1.0, 0.0), arrow_sz, active_dim_color);
                draw_arrowhead(&painter, line_right, Vec2::new(1.0, 0.0), arrow_sz, active_dim_color);

                let mid_x = (p1.x + p2.x) * 0.5;
                let txt_pos = Pos2::new(mid_x, dim_y_px - 3.0 * zoom);

                let galley = painter.layout_no_wrap(dim.text.clone(), font_dim.clone(), active_dim_color);
                label_bg_rect = Rect::from_center_size(txt_pos - Vec2::new(0.0, galley.size().y * 0.5), galley.size() + vec2(4.0, 2.0));
                painter.rect_filled(label_bg_rect, CornerRadius::same(2), Color32::from_rgba_premultiplied(252, 252, 252, 240));
                painter.galley(Pos2::new(label_bg_rect.min.x + 2.0, label_bg_rect.min.y + 1.0), galley, active_dim_color);
            }

            // Highlight border dan tombol hapus [ ✕ ] saat dimensi dihover / dipilih
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
                    "×",
                    FontId::monospace(11.0),
                    Color32::WHITE,
                );
            }
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

                // Tombol Hapus [ ✕ ]
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
                    "×",
                    FontId::monospace(11.0),
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

    // I. Indikator Snap Point & Pengukuran Live (Tambah Data Ukuran Manual)
    if let Some(snap_mm) = active_snap_pt_mm {
        let p_snap = mm_to_screen(snap_mm[0], snap_mm[1]);
        painter.circle_stroke(p_snap, 5.0, Stroke::new(1.5, Color32::from_rgb(255, 140, 0)));
        painter.circle_filled(p_snap, 2.5, Color32::from_rgb(255, 140, 0));
    }

    if state.measure_tool_active {
        if let Some(p1_mm) = state.measure_first_pt {
            let p1 = mm_to_screen(p1_mm[0], p1_mm[1]);
            let p2 = if let Some(snap_mm) = active_snap_pt_mm {
                mm_to_screen(snap_mm[0], snap_mm[1])
            } else if let Some(c_pos) = cursor_pos {
                c_pos
            } else {
                p1
            };
            let p2_mm = [
                (p2.x - sheet_min.x) / zoom,
                (sheet_max.y - p2.y) / zoom,
            ];
            let live_dist_mm = (p2_mm[0] - p1_mm[0]).hypot(p2_mm[1] - p1_mm[1]) / sheet.scale;

            let measure_stroke = Stroke::new(1.5, Color32::from_rgb(255, 140, 0));
            draw_dashed_line(&painter, p1, p2, measure_stroke, 4.0, 2.5);
            painter.circle_filled(p1, 3.5, Color32::from_rgb(255, 140, 0));
            painter.circle_filled(p2, 3.5, Color32::from_rgb(255, 140, 0));

            let font_meas = FontId::monospace((4.5 * zoom).clamp(7.0, 13.0));
            let mid_p = Pos2::new((p1.x + p2.x) * 0.5, (p1.y + p2.y) * 0.5 - 12.0);
            let galley = painter.layout_no_wrap(format!("{:.2} mm", live_dist_mm), font_meas, Color32::from_rgb(255, 180, 50));
            let bg_rect = Rect::from_center_size(mid_p, galley.size() + vec2(6.0, 4.0));
            painter.rect_filled(bg_rect, CornerRadius::same(3), Color32::from_rgba_premultiplied(30, 30, 30, 230));
            painter.galley(bg_rect.min + vec2(3.0, 2.0), galley, Color32::from_rgb(255, 180, 50));
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
            &format!("ISO 2768-m | {}", unit_str),
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
