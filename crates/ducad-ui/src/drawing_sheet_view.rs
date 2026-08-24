//! Komponen Kanvas Interaktif Lembar Kerja Gambar Teknik 2D (Engineering Drawing Sheet).
//!
//! Menyediakan kanvas gambar 2D presisi (kertas putih standar A4/A3 dengan bingkai teknik ISO),
//! kontrol skala gambar, tombol toggle garis tampak & tersembunyi, editor kepala gambar (title block),
//! serta tombol ekspor langsung ke PDF Vektor dan DXF CAD.

use ducad_io::drawing::{DrawingSheet, PaperSize};
use ducad_kernel::HlrLineKind;
use egui::{
    vec2, Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2,
};
use egui_material_icons::icons::{
    ICON_CHECK, ICON_CLOSE, ICON_DOWNLOAD, ICON_EDIT_NOTE, ICON_FIT_SCREEN, ICON_PICTURE_AS_PDF,
    ICON_REFRESH,
};

use crate::theme::{card_frame, glass_frame, ACCENT_BLUE, BORDER_SUBTLE, TEXT_PRIMARY, TEXT_SECONDARY};

/// Helper tombol toggle kustom untuk toolbar Drawing Sheet dengan kontras tinggi saat aktif.
fn sheet_toggle_btn(ui: &mut Ui, label: impl AsRef<str>, is_active: bool) -> egui::Response {
    let (bg_color, text_color, stroke) = if is_active {
        (
            ACCENT_BLUE,
            Color32::WHITE,
            Stroke::new(1.0, Color32::from_rgb(80, 170, 255)),
        )
    } else {
        (
            Color32::from_rgba_premultiplied(32, 36, 46, 160),
            TEXT_PRIMARY,
            Stroke::new(0.5, BORDER_SUBTLE),
        )
    };

    let btn = egui::Button::new(
        RichText::new(label.as_ref())
            .size(11.0)
            .strong()
            .color(text_color),
    )
    .corner_radius(CornerRadius::same(6))
    .fill(bg_color)
    .stroke(stroke);

    ui.add(btn)
}

/// Aksi / Event yang dihasilkan oleh DrawingSheetView ke aplikasi utama.
#[derive(Debug, Clone)]
pub enum DrawingSheetEvent {
    ExportPdf,
    ExportDxf,
    Close,
}

/// State persisten untuk tampilan Lembar Kerja Gambar Teknik.
pub struct DrawingSheetViewState {
    pub is_open: bool,
    pub pan_offset: Vec2,
    pub zoom: f32,
    pub title_block_editor_open: bool,
}

impl Default for DrawingSheetViewState {
    fn default() -> Self {
        Self {
            is_open: false,
            pan_offset: Vec2::ZERO,
            zoom: 1.0,
            title_block_editor_open: false,
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

        // 2. Toolbar Header Atas Lembar Kerja
        let header_height = 42.0;
        let header_rect = Rect::from_min_size(
            total_rect.min,
            Vec2::new(total_rect.width(), header_height),
        );

        let canvas_rect = Rect::from_min_size(
            Pos2::new(total_rect.min.x, total_rect.min.y + header_height),
            Vec2::new(total_rect.width(), total_rect.height() - header_height),
        );

        // Render Header Controls
        let mut header_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(header_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );

        glass_frame().show(&mut header_ui, |ui| {
            ui.set_height(header_height - 6.0);
            ui.spacing_mut().item_spacing = vec2(8.0, 0.0);

            // A. Pemilih Ukuran Kertas (A4/A3)
            ui.add_space(4.0);
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

            // B. Skala Gambar
            ui.label(RichText::new("Skala:").size(11.0).color(TEXT_SECONDARY));
            let current_scale_label = sheet.title_block.scale.clone();
            egui::ComboBox::from_id_salt("scale_combo")
                .selected_text(current_scale_label)
                .show_ui(ui, |ui| {
                    let scales = [
                        (0.2, "1:5"),
                        (0.5, "1:2"),
                        (1.0, "1:1"),
                        (2.0, "2:1"),
                        (5.0, "5:1"),
                    ];
                    for (val, lbl) in scales {
                        let is_sel = (sheet.scale - val).abs() < 1e-4;
                        if ui.selectable_label(is_sel, lbl).clicked() {
                            sheet.scale = val;
                            sheet.title_block.scale = lbl.to_string();
                            for plc in &mut sheet.view_placements {
                                plc.scale = val;
                            }
                            sheet.generate_auto_dimensions();
                        }
                    }
                });

            if ui.button(format!("{} Auto Layout", ICON_REFRESH.codepoint)).clicked() {
                sheet.auto_layout();
            }

            ui.separator();

            // C. Toggles Visibilitas dengan Kontras Jelas
            let hidden_btn_txt = if sheet.show_hidden_lines {
                format!("{} Garis Tersembunyi", ICON_CHECK.codepoint)
            } else {
                "Garis Tersembunyi".to_string()
            };
            if sheet_toggle_btn(ui, hidden_btn_txt, sheet.show_hidden_lines).clicked() {
                sheet.show_hidden_lines = !sheet.show_hidden_lines;
            }

            let dim_btn_txt = if sheet.show_dimensions {
                format!("{} Dimensi", ICON_CHECK.codepoint)
            } else {
                "Dimensi".to_string()
            };
            if sheet_toggle_btn(ui, dim_btn_txt, sheet.show_dimensions).clicked() {
                sheet.show_dimensions = !sheet.show_dimensions;
            }

            let cl_btn_txt = if sheet.show_centerlines {
                format!("{} Sumbu", ICON_CHECK.codepoint)
            } else {
                "Sumbu".to_string()
            };
            if sheet_toggle_btn(ui, cl_btn_txt, sheet.show_centerlines).clicked() {
                sheet.show_centerlines = !sheet.show_centerlines;
            }

            // D. Tombol Edit Title Block
            if sheet_toggle_btn(
                ui,
                format!("{} Kepala Gambar", ICON_EDIT_NOTE.codepoint),
                state.title_block_editor_open,
            )
            .clicked()
            {
                state.title_block_editor_open = !state.title_block_editor_open;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Tombol Tutup (X)
                if ui
                    .button(
                        RichText::new(ICON_CLOSE.codepoint.to_string())
                            .size(13.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .on_hover_text("Tutup Lembar Kerja")
                    .clicked()
                {
                    event = Some(DrawingSheetEvent::Close);
                }

                // Tombol Ekspor PDF Vektor (Biru Utama)
                if ui
                    .button(
                        RichText::new(format!("{} Ekspor PDF", ICON_PICTURE_AS_PDF.codepoint))
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .clicked()
                {
                    event = Some(DrawingSheetEvent::ExportPdf);
                }

                // Tombol Ekspor DXF CAD
                if ui
                    .button(
                        RichText::new(format!("{} Ekspor DXF", ICON_DOWNLOAD.codepoint))
                            .color(TEXT_PRIMARY),
                    )
                    .clicked()
                {
                    event = Some(DrawingSheetEvent::ExportDxf);
                }
            });
        });

        // 3. Kanvas Interaktif Kertas Gambar Teknik
        let response = ui.allocate_rect(canvas_rect, Sense::click_and_drag());

        // Handle Pan & Zoom
        if response.dragged_by(egui::PointerButton::Middle)
            || (response.dragged_by(egui::PointerButton::Primary) && ui.input(|i| i.modifiers.alt))
        {
            state.pan_offset += response.drag_delta();
        }

        let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll_delta.abs() > 0.0 && response.hovered() {
            let zoom_factor = if scroll_delta > 0.0 { 1.1 } else { 0.9 };
            state.zoom = (state.zoom * zoom_factor).clamp(0.2, 5.0);
        }

        if state.zoom <= 0.05 {
            state.zoom = calculate_fit_zoom(canvas_rect, sheet.paper_size);
        }

        // Render Lembar Kertas & Konten Gambar 2D
        render_sheet_canvas(ui, canvas_rect, state, sheet);

        // 4. Tombol Fit Mengambang di Pojok Kanan Bawah (Ikon Saja, Tanpa Frame Pembungkus Ganda)
        let fit_size = vec2(36.0, 36.0);
        let fit_pos = Pos2::new(canvas_rect.max.x - 52.0, canvas_rect.max.y - 52.0);
        let fit_rect = Rect::from_min_size(fit_pos, fit_size);
        let mut fit_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(fit_rect)
                .layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight)),
        );
        let fit_btn = egui::Button::new(
            RichText::new(ICON_FIT_SCREEN.codepoint.to_string())
                .size(16.0)
                .color(TEXT_PRIMARY),
        )
        .corner_radius(CornerRadius::same(8))
        .fill(Color32::from_rgba_premultiplied(28, 32, 42, 200))
        .stroke(Stroke::new(0.5, BORDER_SUBTLE))
        .min_size(fit_size);

        if fit_ui
            .add(fit_btn)
            .on_hover_text("Pusatkan Kertas ke Layar (Fit)")
            .clicked()
        {
            state.pan_offset = Vec2::ZERO;
            state.zoom = calculate_fit_zoom(canvas_rect, sheet.paper_size);
        }

        // 5. Panel Form Floating: Edit Kepala Gambar (Title Block)
        if state.title_block_editor_open {
            render_title_block_editor(ui, canvas_rect, sheet, &mut state.title_block_editor_open);
        }

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

    // E. Kepala Gambar (Title Block)
    render_title_block_screen(&painter, sheet, zoom, mm_to_screen);

    // F. Gambar Tampak-tampak Proyeksi (Front, Top, Right, Isometric)
    for plc in &sheet.view_placements {
        if !plc.visible {
            continue;
        }
        let view = sheet.drawing.view_by_kind(plc.kind);
        let center_mm = plc.center_mm;
        let scale = plc.scale;
        let v_center = view.center_2d();

        // Judul Tampak
        let title_pos = mm_to_screen(
            center_mm[0] - (view.size_2d()[0] * scale * 0.5),
            center_mm[1] - (view.size_2d()[1] * scale * 0.5) - 6.0,
        );
        let font_title = FontId::proportional((8.5 * zoom).clamp(9.0, 16.0));
        painter.text(
            title_pos,
            Align2::LEFT_TOP,
            &view.title,
            font_title,
            Color32::BLACK,
        );

        // 1. Centerlines (Garis Sumbu)
        if sheet.show_centerlines {
            let cl_stroke = Stroke::new((0.4 * zoom).max(1.0), Color32::from_rgb(0, 140, 50));
            for cl in &view.centerlines {
                let p1 = mm_to_screen(
                    center_mm[0] + (cl.start[0] - v_center[0]) * scale,
                    center_mm[1] + (cl.start[1] - v_center[1]) * scale,
                );
                let p2 = mm_to_screen(
                    center_mm[0] + (cl.end[0] - v_center[0]) * scale,
                    center_mm[1] + (cl.end[1] - v_center[1]) * scale,
                );
                draw_dashed_line(&painter, p1, p2, cl_stroke, 6.0 * zoom, 3.0 * zoom);
            }
        }

        // 2. Hidden Lines (Garis Tersembunyi Putus-putus)
        if sheet.show_hidden_lines {
            let hidden_stroke = Stroke::new((0.4 * zoom).max(0.8), Color32::from_rgb(130, 130, 145));
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

        // 3. Visible Lines & Silhouettes (Garis Tampak Solid)
        let visible_stroke = Stroke::new((0.7 * zoom).max(1.2), Color32::BLACK);
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
    }

    // G. Anotasi Dimensi Otomatis
    if sheet.show_dimensions {
        let dim_stroke = Stroke::new((0.4 * zoom).max(1.0), Color32::from_rgb(10, 80, 200));
        let font_dim = FontId::proportional((7.5 * zoom).clamp(8.0, 13.0));

        for dim in &sheet.auto_dimensions {
            let p1 = mm_to_screen(dim.start[0], dim.start[1]);
            let p2 = mm_to_screen(dim.end[0], dim.end[1]);

            if dim.is_vertical {
                let dim_x_px = mm_to_screen(dim.line_pos[0], 0.0).x;
                let ext1_end = Pos2::new(dim_x_px, p1.y);
                let ext2_end = Pos2::new(dim_x_px, p2.y);

                painter.line_segment([p1, ext1_end], dim_stroke);
                painter.line_segment([p2, ext2_end], dim_stroke);
                painter.line_segment([ext1_end, ext2_end], dim_stroke);

                let mid_y = (p1.y + p2.y) * 0.5;
                painter.text(
                    Pos2::new(dim_x_px - 4.0 * zoom, mid_y),
                    Align2::RIGHT_CENTER,
                    &dim.text,
                    font_dim.clone(),
                    Color32::from_rgb(10, 80, 200),
                );
            } else {
                let dim_y_px = mm_to_screen(0.0, dim.line_pos[1]).y;
                let ext1_end = Pos2::new(p1.x, dim_y_px);
                let ext2_end = Pos2::new(p2.x, dim_y_px);

                painter.line_segment([p1, ext1_end], dim_stroke);
                painter.line_segment([p2, ext2_end], dim_stroke);
                painter.line_segment([ext1_end, ext2_end], dim_stroke);

                let mid_x = (p1.x + p2.x) * 0.5;
                painter.text(
                    Pos2::new(mid_x, dim_y_px - 3.0 * zoom),
                    Align2::CENTER_BOTTOM,
                    &dim.text,
                    font_dim.clone(),
                    Color32::from_rgb(10, 80, 200),
                );
            }
        }
    }
}

/// Render Kepala Gambar (Title Block) pada layar.
fn render_title_block_screen<F>(
    painter: &egui::Painter,
    sheet: &DrawingSheet,
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

    // Kotak luar title block
    painter.rect_stroke(
        tb_rect,
        CornerRadius::ZERO,
        Stroke::new((0.6 * zoom).max(1.0), Color32::BLACK),
        egui::StrokeKind::Inside,
    );

    // Garis pembagi horizontal
    let y1 = mm_to_screen(0.0, tb[1] + 15.0).y;
    let y2 = mm_to_screen(0.0, tb[1] + 30.0).y;
    painter.line_segment(
        [Pos2::new(tb_rect.min.x, y1), Pos2::new(tb_rect.max.x, y1)],
        Stroke::new(1.0, Color32::BLACK),
    );
    painter.line_segment(
        [Pos2::new(tb_rect.min.x, y2), Pos2::new(tb_rect.max.x, y2)],
        Stroke::new(1.0, Color32::BLACK),
    );

    // Garis pembagi vertikal
    let x_mid = mm_to_screen(tb[0] + 75.0, 0.0).x;
    painter.line_segment(
        [Pos2::new(x_mid, y2), Pos2::new(x_mid, tb_rect.max.y)],
        Stroke::new(1.0, Color32::BLACK),
    );

    let x_qtr = mm_to_screen(tb[0] + 110.0, 0.0).x;
    painter.line_segment(
        [Pos2::new(x_qtr, y1), Pos2::new(x_qtr, tb_rect.max.y)],
        Stroke::new(1.0, Color32::BLACK),
    );

    // Teks dalam Title Block
    let font_lg = FontId::proportional((9.5 * zoom).clamp(10.0, 18.0));
    let font_md = FontId::proportional((8.0 * zoom).clamp(8.5, 14.0));
    let font_sm = FontId::proportional((6.5 * zoom).clamp(7.0, 11.0));

    // Perusahaan
    painter.text(
        mm_to_screen(tb[0] + 4.0, tb[1] + 36.0),
        Align2::LEFT_CENTER,
        &info.company_name,
        font_md.clone(),
        Color32::BLACK,
    );

    // Judul Proyek
    painter.text(
        mm_to_screen(tb[0] + 4.0, tb[1] + 32.0),
        Align2::LEFT_CENTER,
        "JUDUL GAMBAR:",
        font_sm.clone(),
        Color32::from_rgb(100, 100, 100),
    );
    painter.text(
        mm_to_screen(tb[0] + 4.0, tb[1] + 20.0),
        Align2::LEFT_CENTER,
        &info.project_title,
        font_lg,
        Color32::BLACK,
    );

    // Drafter & Tanggal
    painter.text(
        mm_to_screen(tb[0] + 4.0, tb[1] + 10.0),
        Align2::LEFT_CENTER,
        format!("DIGAMBAR: {} | TGL: {}", info.drawn_by, info.date),
        font_sm.clone(),
        Color32::BLACK,
    );
    painter.text(
        mm_to_screen(tb[0] + 4.0, tb[1] + 4.0),
        Align2::LEFT_CENTER,
        format!("MATERIAL: {}", info.material),
        font_sm.clone(),
        Color32::BLACK,
    );

    // Nomor Gambar & Skala
    painter.text(
        mm_to_screen(tb[0] + 78.0, tb[1] + 24.0),
        Align2::LEFT_CENTER,
        "NO. GAMBAR:",
        font_sm.clone(),
        Color32::from_rgb(100, 100, 100),
    );
    painter.text(
        mm_to_screen(tb[0] + 78.0, tb[1] + 17.5),
        Align2::LEFT_CENTER,
        &info.drawing_number,
        font_md,
        Color32::BLACK,
    );
    painter.text(
        mm_to_screen(tb[0] + 78.0, tb[1] + 9.0),
        Align2::LEFT_CENTER,
        format!("SKALA: {}", info.scale),
        font_sm.clone(),
        Color32::BLACK,
    );
    painter.text(
        mm_to_screen(tb[0] + 78.0, tb[1] + 3.5),
        Align2::LEFT_CENTER,
        format!("SATUAN: {} | LBR: {}", info.units, info.sheet_number),
        font_sm,
        Color32::BLACK,
    );
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

/// Form Editor Floating untuk mengubah isi Kepala Gambar (Title Block).
fn render_title_block_editor(
    ui: &mut Ui,
    canvas_rect: Rect,
    sheet: &mut DrawingSheet,
    is_open: &mut bool,
) {
    let panel_w = 320.0;
    let panel_h = 360.0;
    let panel_rect = Rect::from_min_size(
        Pos2::new(
            canvas_rect.max.x - panel_w - 20.0,
            canvas_rect.min.y + 20.0,
        ),
        Vec2::new(panel_w, panel_h),
    );

    let mut panel_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(panel_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );

    card_frame().show(&mut panel_ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{} Editor Kepala Gambar", ICON_EDIT_NOTE.codepoint))
                    .strong()
                    .size(13.0)
                    .color(Color32::WHITE),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(ICON_CLOSE.codepoint).clicked() {
                    *is_open = false;
                }
            });
        });

        ui.separator();
        ui.add_space(4.0);

        let info = &mut sheet.title_block;

        egui::Grid::new("tb_editor_grid")
            .num_columns(2)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                ui.label(RichText::new("Judul:").size(11.0).color(TEXT_SECONDARY));
                ui.text_edit_singleline(&mut info.project_title);
                ui.end_row();

                ui.label(RichText::new("No. Gambar:").size(11.0).color(TEXT_SECONDARY));
                ui.text_edit_singleline(&mut info.drawing_number);
                ui.end_row();

                ui.label(RichText::new("Drafter:").size(11.0).color(TEXT_SECONDARY));
                ui.text_edit_singleline(&mut info.drawn_by);
                ui.end_row();

                ui.label(RichText::new("Tanggal:").size(11.0).color(TEXT_SECONDARY));
                ui.text_edit_singleline(&mut info.date);
                ui.end_row();

                ui.label(RichText::new("Material:").size(11.0).color(TEXT_SECONDARY));
                ui.text_edit_singleline(&mut info.material);
                ui.end_row();

                ui.label(RichText::new("Perusahaan:").size(11.0).color(TEXT_SECONDARY));
                ui.text_edit_singleline(&mut info.company_name);
                ui.end_row();

                ui.label(RichText::new("Revisi:").size(11.0).color(TEXT_SECONDARY));
                ui.text_edit_singleline(&mut info.revision);
                ui.end_row();
            });

        ui.add_space(8.0);
        if ui
            .button(
                RichText::new(format!("{} Terapkan Perubahan", ICON_CHECK.codepoint))
                    .strong()
                    .color(Color32::WHITE),
            )
            .clicked()
        {
            *is_open = false;
        }
    });
}
