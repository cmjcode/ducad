//! Komponen Kanvas Interaktif Lembar Kerja Gambar Teknik 2D (Engineering Drawing Sheet).
//!
//! Menyediakan kanvas gambar 2D presisi (kertas putih standar A4/A3 dengan bingkai teknik ISO),
//! kontrol skala gambar, tombol toggle garis tampak & tersembunyi, editor kepala gambar (title block),
//! serta tombol ekspor langsung ke PDF Vektor dan DXF CAD.

use ducad_io::drawing::{DrawingSheet, PaperSize};
use ducad_kernel::{HlrLineKind, ProjectedViewKind};
use egui::{
    vec2, Align2, Color32, CornerRadius, FontId, Frame, Margin, Pos2, Rect, RichText, Sense,
    Stroke, Ui, Vec2,
};
use egui_material_icons::icons::{
    ICON_CHECK, ICON_CLOSE, ICON_DOWNLOAD, ICON_EDIT_NOTE, ICON_FIT_SCREEN, ICON_LAYERS,
    ICON_PICTURE_AS_PDF, ICON_REFRESH, ICON_STRAIGHTEN, ICON_TEXTURE,
};

use crate::theme::{card_frame, glass_frame, ACCENT_BLUE, BORDER_SUBTLE, TEXT_PRIMARY, TEXT_SECONDARY};

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

        // 2. Dimensi Top Bar & Floating Controls (Margin simetris: kiri, kanan, dan atas sama 16px)
        let margin_side = 16.0;
        let margin_top = 16.0;
        let topbar_x = total_rect.min.x + margin_side;
        let topbar_w = (total_rect.width() - (margin_side * 2.0)).max(200.0);
        let topbar_rect = Rect::from_min_size(
            Pos2::new(topbar_x, total_rect.min.y + margin_top),
            Vec2::new(topbar_w, 30.0),
        );

        let fit_size = vec2(36.0, 36.0);
        let fit_pos = Pos2::new(total_rect.max.x - 52.0, total_rect.max.y - 52.0);
        let fit_rect = Rect::from_min_size(fit_pos, fit_size);

        let canvas_rect = total_rect;

        // 3. Kanvas Interaktif Kertas Gambar Teknik (Background Sensor dialokasikan SEBELUM floating UI)
        let response = ui.allocate_rect(canvas_rect, Sense::click_and_drag());

        // Handle Pan & Zoom (Hanya aktif jika kursor tidak sedang di atas top bar / floating buttons)
        let cursor_pos = ui.input(|i| i.pointer.hover_pos());
        let is_over_ui = cursor_pos.map_or(false, |p| topbar_rect.contains(p) || fit_rect.contains(p));

        if !is_over_ui {
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
        }

        if state.zoom <= 0.05 {
            state.zoom = calculate_fit_zoom(canvas_rect, sheet.paper_size);
        }

        // Render Lembar Kertas & Konten Gambar 2D
        render_sheet_canvas(ui, canvas_rect, state, sheet);

        // 4. Render Header Controls (Floating Top Bar Glassmorphism di Atas Kanvas)
        let mut header_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(topbar_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );

        glass_frame().show(&mut header_ui, |ui| {
            ui.set_height(30.0);
            ui.horizontal(|ui| {
                // A. Icon Drawing Sheet & Title
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

                // C. Skala Gambar
                ui.label(RichText::new("Skala:").size(11.0).color(TEXT_SECONDARY));
                let current_scale_label = sheet.title_block.scale.clone();
                egui::ComboBox::from_id_salt("scale_combo")
                    .selected_text(current_scale_label)
                    .show_ui(ui, |ui| {
                        let scales = [
                            (0.05, "1:20"),
                            (0.1, "1:10"),
                            (0.125, "1:8"),
                            (0.2, "1:5"),
                            (0.25, "1:4"),
                            (0.333, "1:3"),
                            (0.4, "1:2.5"),
                            (0.5, "1:2"),
                            (0.667, "1:1.5"),
                            (1.0, "1:1"),
                            (2.0, "2:1"),
                            (5.0, "5:1"),
                        ];
                        for (val, lbl) in scales {
                            let is_sel = (sheet.scale - val).abs() < 1e-3;
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

                let auto_btn = header_icon_btn(
                    ui,
                    ICON_REFRESH.codepoint,
                    false,
                    "Auto Layout",
                    Some("R"),
                    Some("Atur ulang posisi tampak proyeksi secara otomatis"),
                    None,
                    None,
                );
                if auto_btn.clicked() {
                    sheet.auto_layout();
                }

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

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

                let tb_btn = header_icon_btn(
                    ui,
                    ICON_EDIT_NOTE.codepoint,
                    state.title_block_editor_open,
                    "Editor Kepala Gambar",
                    Some("T"),
                    Some("Buka panel informasi formulir Title Block ISO"),
                    Some(Color32::from_rgba_premultiplied(18, 42, 85, 100)),
                    Some(ACCENT_BLUE),
                );
                if tb_btn.clicked() {
                    state.title_block_editor_open = !state.title_block_editor_open;
                }

                // E. Sisi Kanan: Close (X) dan Ekspor
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

        // 5. Tombol Fit Mengambang di Pojok Kanan Bawah
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

        // 6. Panel Form Floating: Edit Kepala Gambar (Title Block)
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

    // E. Kepala Gambar (Title Block ISO 7200)
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
        let view_sz = view.size_2d();

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

        // 3. Visible Lines & Silhouettes (Garis Tampak Tebal Solid ISO 128)
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

        // Judul Tampak Profesional di bawah view (bebas dari tabrakan garis dimensi)
        let title_y_mm = center_mm[1] - (view_sz[1] * scale * 0.5) - 13.0;
        let title_pos = mm_to_screen(center_mm[0], title_y_mm);
        let title_sub_pos = mm_to_screen(center_mm[0], title_y_mm - 4.5);

        let font_title = FontId::proportional((4.2 * zoom).clamp(5.5, 12.0));
        let font_sub = FontId::proportional((3.5 * zoom).clamp(4.5, 9.5));

        let (sub_label, scale_label) = match plc.kind {
            ProjectedViewKind::Front => ("FRONT VIEW", format!("SKALA {}", sheet.title_block.scale)),
            ProjectedViewKind::Top => ("TOP VIEW", format!("SKALA {}", sheet.title_block.scale)),
            ProjectedViewKind::Right => ("RIGHT SIDE VIEW", format!("SKALA {}", sheet.title_block.scale)),
            ProjectedViewKind::Isometric => ("ISOMETRIC 3D", format!("SKALA {}", sheet.title_block.scale)),
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
        let dim_stroke = Stroke::new((0.35 * zoom).clamp(0.8, 1.5), Color32::from_rgb(12, 70, 175));
        let font_dim = FontId::monospace((4.2 * zoom).clamp(5.5, 11.5));
        let arrow_sz = (2.2 * zoom).clamp(3.5, 7.5);

        for dim in &sheet.auto_dimensions {
            let p1 = mm_to_screen(dim.start[0], dim.start[1]);
            let p2 = mm_to_screen(dim.end[0], dim.end[1]);

            let is_leader = dim.text.starts_with('R')
                || dim.text.starts_with('Ø')
                || dim.text.starts_with("Rx");
            let is_angle = dim.text.ends_with('°');

            if is_angle {
                let p_v = p1;
                let p_a1 = p2;
                let p_txt = mm_to_screen(dim.line_pos[0], dim.line_pos[1]);

                // Gambar garis bantu sudut dari vertex ke titik ukur & posisi teks
                painter.line_segment([p_v, p_a1], dim_stroke);
                painter.line_segment([p_v, p_txt], dim_stroke);

                let dir_vec = (p_txt - p_v).normalized();
                draw_arrowhead(&painter, p_txt, dir_vec, arrow_sz, Color32::from_rgb(12, 70, 175));

                let galley = painter.layout_no_wrap(dim.text.clone(), font_dim.clone(), Color32::from_rgb(12, 70, 175));
                let bg_rect = Rect::from_center_size(p_txt + vec2(galley.size().x * 0.5 + 4.0, 0.0), galley.size() + vec2(4.0, 2.0));
                painter.rect_filled(bg_rect, CornerRadius::same(2), Color32::from_rgba_premultiplied(252, 252, 252, 240));
                painter.galley(Pos2::new(bg_rect.min.x + 2.0, bg_rect.min.y + 1.0), galley, Color32::from_rgb(12, 70, 175));
            } else if is_leader {
                let p_start = p1;
                let p_end = p2;
                let p_bend = mm_to_screen(dim.line_pos[0], dim.line_pos[1]);
                let p_shoulder = Pos2::new(p_bend.x + 12.0 * zoom.clamp(0.8, 1.5), p_bend.y);

                // Garis radial leader dari pusat ke tepi keliling dan bahu horizontal
                painter.line_segment([p_start, p_end], dim_stroke);
                painter.line_segment([p_end, p_bend], dim_stroke);
                painter.line_segment([p_bend, p_shoulder], dim_stroke);

                let dir_vec = p_end - p_start;
                let dir_norm = if dir_vec.length_sq() > 1e-4 {
                    dir_vec.normalized()
                } else {
                    Vec2::new(1.0, 0.0)
                };
                draw_arrowhead(&painter, p_end, dir_norm, arrow_sz, Color32::from_rgb(12, 70, 175));

                let txt_pos = Pos2::new(p_bend.x + 2.0, p_bend.y - 3.0 * zoom);
                let galley = painter.layout_no_wrap(dim.text.clone(), font_dim.clone(), Color32::from_rgb(12, 70, 175));
                let bg_rect = Rect::from_center_size(txt_pos + Vec2::new(galley.size().x * 0.5, 0.0), galley.size() + vec2(4.0, 2.0));
                painter.rect_filled(bg_rect, CornerRadius::same(2), Color32::from_rgba_premultiplied(252, 252, 252, 240));
                painter.galley(Pos2::new(bg_rect.min.x + 2.0, bg_rect.min.y + 1.0), galley, Color32::from_rgb(12, 70, 175));
            } else if dim.is_vertical {
                let dim_x_px = mm_to_screen(dim.line_pos[0], 0.0).x;
                let ext_overshoot = 2.0 * zoom;
                let ext_dir = if dim_x_px < p1.x { -1.0 } else { 1.0 };

                let ext1_start = Pos2::new(p1.x + ext_dir * 1.5 * zoom, p1.y);
                let ext1_end = Pos2::new(dim_x_px + ext_dir * ext_overshoot, p1.y);
                let ext2_start = Pos2::new(p2.x + ext_dir * 1.5 * zoom, p2.y);
                let ext2_end = Pos2::new(dim_x_px + ext_dir * ext_overshoot, p2.y);

                painter.line_segment([ext1_start, ext1_end], dim_stroke);
                painter.line_segment([ext2_start, ext2_end], dim_stroke);

                let line_top = Pos2::new(dim_x_px, p1.y.min(p2.y));
                let line_bot = Pos2::new(dim_x_px, p1.y.max(p2.y));
                painter.line_segment([line_top, line_bot], dim_stroke);

                // Panah vertikal di kedua ujung
                draw_arrowhead(&painter, line_top, Vec2::new(0.0, -1.0), arrow_sz, Color32::from_rgb(12, 70, 175));
                draw_arrowhead(&painter, line_bot, Vec2::new(0.0, 1.0), arrow_sz, Color32::from_rgb(12, 70, 175));

                let mid_y = (p1.y + p2.y) * 0.5;
                let txt_pos = Pos2::new(dim_x_px - 4.0 * zoom, mid_y);

                // Knockout background putih tipis agar angka tidak tertimpa garis
                let galley = painter.layout_no_wrap(dim.text.clone(), font_dim.clone(), Color32::from_rgb(12, 70, 175));
                let bg_rect = Rect::from_center_size(txt_pos - Vec2::new(galley.size().x * 0.5, 0.0), galley.size() + vec2(4.0, 2.0));
                painter.rect_filled(bg_rect, CornerRadius::same(2), Color32::from_rgba_premultiplied(252, 252, 252, 240));
                painter.galley(Pos2::new(bg_rect.min.x + 2.0, bg_rect.min.y + 1.0), galley, Color32::from_rgb(12, 70, 175));
            } else {
                let dim_y_px = mm_to_screen(0.0, dim.line_pos[1]).y;
                let ext_overshoot = 2.0 * zoom;
                let ext_dir = if dim_y_px > p1.y { 1.0 } else { -1.0 };

                let ext1_start = Pos2::new(p1.x, p1.y + ext_dir * 1.5 * zoom);
                let ext1_end = Pos2::new(p1.x, dim_y_px + ext_dir * ext_overshoot);
                let ext2_start = Pos2::new(p2.x, p2.y + ext_dir * 1.5 * zoom);
                let ext2_end = Pos2::new(p2.x, dim_y_px + ext_dir * ext_overshoot);

                painter.line_segment([ext1_start, ext1_end], dim_stroke);
                painter.line_segment([ext2_start, ext2_end], dim_stroke);

                let line_left = Pos2::new(p1.x.min(p2.x), dim_y_px);
                let line_right = Pos2::new(p1.x.max(p2.x), dim_y_px);
                painter.line_segment([line_left, line_right], dim_stroke);

                // Panah horizontal di kedua ujung
                draw_arrowhead(&painter, line_left, Vec2::new(-1.0, 0.0), arrow_sz, Color32::from_rgb(12, 70, 175));
                draw_arrowhead(&painter, line_right, Vec2::new(1.0, 0.0), arrow_sz, Color32::from_rgb(12, 70, 175));

                let mid_x = (p1.x + p2.x) * 0.5;
                let txt_pos = Pos2::new(mid_x, dim_y_px - 3.0 * zoom);

                let galley = painter.layout_no_wrap(dim.text.clone(), font_dim.clone(), Color32::from_rgb(12, 70, 175));
                let bg_rect = Rect::from_center_size(txt_pos - Vec2::new(0.0, galley.size().y * 0.5), galley.size() + vec2(4.0, 2.0));
                painter.rect_filled(bg_rect, CornerRadius::same(2), Color32::from_rgba_premultiplied(252, 252, 252, 240));
                painter.galley(Pos2::new(bg_rect.min.x + 2.0, bg_rect.min.y + 1.0), galley, Color32::from_rgb(12, 70, 175));
            }
        }
    }
}

/// Render Kepala Gambar (Title Block) Standar ISO 7200 / DIN 6771 yang Presisi.
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

    let x_b1 = mm_to_screen(tb[0] + 45.0, 0.0).x;
    let x_b2 = mm_to_screen(tb[0] + 90.0, 0.0).x;
    let x_b3 = mm_to_screen(tb[0] + 115.0, 0.0).x;

    painter.line_segment([Pos2::new(x_b1, y_row2), Pos2::new(x_b1, y_row1)], stroke_thin);
    painter.line_segment([Pos2::new(x_b2, y_row2), Pos2::new(x_b2, tb_rect.max.y)], stroke_thin);
    painter.line_segment([Pos2::new(x_b3, y_row2), Pos2::new(x_b3, y_row1)], stroke_thin);

    // 4. Tipografi & Konten Teks Proporsional (Skala Kertas)
    let font_caption = FontId::proportional((2.6 * zoom).clamp(4.0, 8.5));
    let font_val_sm = FontId::proportional((3.2 * zoom).clamp(4.8, 10.0));
    let font_val_md = FontId::proportional((4.0 * zoom).clamp(5.8, 12.0));
    let font_val_lg = FontId::proportional((5.2 * zoom).clamp(7.0, 14.5));

    let col_caption = Color32::from_rgb(110, 115, 130);
    let col_val = Color32::BLACK;

    // A. Row 1: Perusahaan & Proyeksi
    let p_comp = mm_to_screen(tb[0] + 3.0, tb[1] + 39.5);
    painter.text(p_comp, Align2::LEFT_CENTER, &info.company_name, font_val_md.clone(), col_val);
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

    // B. Row 2: Judul Komponen & Nomor Gambar
    painter.text(
        mm_to_screen(tb[0] + 3.0, tb[1] + 27.5),
        Align2::LEFT_CENTER,
        "JUDUL GAMBAR / PART TITLE:",
        font_caption.clone(),
        col_caption,
    );
    let proj_title = if info.project_title.is_empty() { "KOMPONEN UTAMA" } else { &info.project_title };
    painter.text(
        mm_to_screen(tb[0] + 3.0, tb[1] + 22.0),
        Align2::LEFT_CENTER,
        proj_title,
        font_val_lg,
        col_val,
    );

    painter.text(
        mm_to_screen(tb[0] + 88.0, tb[1] + 27.5),
        Align2::LEFT_CENTER,
        "NO. GAMBAR / DWG NO:",
        font_caption.clone(),
        col_caption,
    );
    painter.text(
        mm_to_screen(tb[0] + 88.0, tb[1] + 22.0),
        Align2::LEFT_CENTER,
        &info.drawing_number,
        font_val_md.clone(),
        col_val,
    );

    // C. Row 3: Drafter, Tanggal, Skala, Lembar
    painter.text(mm_to_screen(tb[0] + 3.0, tb[1] + 15.0), Align2::LEFT_CENTER, "DIGAMBAR:", font_caption.clone(), col_caption);
    painter.text(mm_to_screen(tb[0] + 3.0, tb[1] + 11.5), Align2::LEFT_CENTER, &info.drawn_by, font_val_sm.clone(), col_val);

    painter.text(mm_to_screen(tb[0] + 48.0, tb[1] + 15.0), Align2::LEFT_CENTER, "TANGGAL:", font_caption.clone(), col_caption);
    painter.text(mm_to_screen(tb[0] + 48.0, tb[1] + 11.5), Align2::LEFT_CENTER, &info.date, font_val_sm.clone(), col_val);

    painter.text(mm_to_screen(tb[0] + 93.0, tb[1] + 15.0), Align2::LEFT_CENTER, "SKALA:", font_caption.clone(), col_caption);
    painter.text(mm_to_screen(tb[0] + 93.0, tb[1] + 11.5), Align2::LEFT_CENTER, &info.scale, font_val_sm.clone(), col_val);

    painter.text(mm_to_screen(tb[0] + 118.0, tb[1] + 15.0), Align2::LEFT_CENTER, "LEMBAR:", font_caption.clone(), col_caption);
    painter.text(mm_to_screen(tb[0] + 118.0, tb[1] + 11.5), Align2::LEFT_CENTER, &info.sheet_number, font_val_sm.clone(), col_val);

    // D. Row 4: Material & Toleransi
    painter.text(mm_to_screen(tb[0] + 3.0, tb[1] + 6.5), Align2::LEFT_CENTER, "MATERIAL:", font_caption.clone(), col_caption);
    painter.text(mm_to_screen(tb[0] + 3.0, tb[1] + 2.8), Align2::LEFT_CENTER, &info.material, font_val_sm.clone(), col_val);

    painter.text(mm_to_screen(tb[0] + 93.0, tb[1] + 6.5), Align2::LEFT_CENTER, "TOLERANSI & SATUAN:", font_caption.clone(), col_caption);
    painter.text(
        mm_to_screen(tb[0] + 93.0, tb[1] + 2.8),
        Align2::LEFT_CENTER,
        &format!("ISO 2768-m | {}", info.units),
        font_val_sm,
        col_val,
    );
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
