//! Animated Interactive Tool Tutorials (Panduan Animasi Visual Interaktif Semua Tools).
//!
//! Menampilkan kartu panduan visual animasi di pojok kiri bawah kanvas HUD
//! dengan demonstrasi langkah interaktif, kursor animasi, feedback klik,
//! dan visualisasi hasil secara real-time.

use egui::{
    Align2, Color32, FontId, Pos2, Rect, Stroke, StrokeKind, Ui, Vec2,
};

use crate::left_toolbar::ToolbarTool;
use crate::theme::{
    ACCENT_BLUE, ACCENT_GREEN, ACCENT_ORANGE, BORDER_SUBTLE,
    TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};

pub struct ToolGuides;

impl ToolGuides {
    /// Render kartu tutorial animasi untuk tool yang sedang aktif di pojok kiri bawah kanvas.
    pub fn render_tool_guide(
        ui: &mut Ui,
        canvas_rect: Rect,
        tool: ToolbarTool,
        pending_points_count: usize,
        has_selection: bool,
        time: f64,
    ) {
        // Jangan render jika Select atau History (atau tool yang tidak butuh panduan menggambar)
        if tool == ToolbarTool::Select || tool == ToolbarTool::History {
            return;
        }

        ui.ctx().request_repaint();

        let card_w = 240.0;
        let card_h = 145.0;
        let guide_center = Pos2::new(
            canvas_rect.left() + card_w * 0.5 + 20.0,
            canvas_rect.bottom() - card_h * 0.5 - 20.0,
        );
        let card_rect = Rect::from_center_size(guide_center, Vec2::new(card_w, card_h));

        let painter = ui.painter();

        // 1. Gambar latar belakang kartu glassmorphism
        Self::draw_card_base(painter, card_rect);

        // 2. Render konten diagram animasi sesuai tool
        match tool {
            ToolbarTool::Line => {
                Self::render_line_anim(painter, card_rect, pending_points_count, time);
            }
            ToolbarTool::Rectangle => {
                Self::render_rectangle_anim(painter, card_rect, pending_points_count, time);
            }
            ToolbarTool::Circle => {
                Self::render_circle_anim(painter, card_rect, pending_points_count, time);
            }
            ToolbarTool::Arc => {
                Self::render_arc_anim(painter, card_rect, pending_points_count, time);
            }
            ToolbarTool::Ellipse => {
                Self::render_ellipse_anim(painter, card_rect, pending_points_count, time);
            }
            ToolbarTool::Offset => {
                Self::render_offset_anim(painter, card_rect, has_selection, time);
            }
            ToolbarTool::Mirror => {
                Self::render_mirror_anim(painter, card_rect, has_selection, pending_points_count, time);
            }
            ToolbarTool::Trim => {
                Self::render_trim_anim(painter, card_rect, time);
            }
            ToolbarTool::PointCoincident => {
                Self::render_coincident_anim(painter, card_rect, pending_points_count, time);
            }
            ToolbarTool::PointFixed => {
                Self::render_fixed_anim(painter, card_rect, time);
            }
            ToolbarTool::PointSymmetric => {
                Self::render_symmetric_anim(painter, card_rect, pending_points_count, time);
            }
            ToolbarTool::Extrude => {
                Self::render_extrude_anim(painter, card_rect, has_selection, time);
            }
            ToolbarTool::Revolve => {
                // Revolve ditangani secara khusus dengan kontrol sudut & arah di canvas_hud
            }
            ToolbarTool::Loft => {
                Self::render_loft_anim(painter, card_rect, has_selection, time);
            }
            ToolbarTool::FilletChamfer => {
                Self::render_fillet_anim(painter, card_rect, has_selection, time);
            }
            ToolbarTool::Shell => {
                Self::render_shell_anim(painter, card_rect, has_selection, time);
            }
            ToolbarTool::Boolean => {
                Self::render_boolean_anim(painter, card_rect, time);
            }
            ToolbarTool::SectionView => {
                Self::render_section_anim(painter, card_rect, time);
            }
            ToolbarTool::Measure => {
                Self::render_measure_dist_anim(painter, card_rect, pending_points_count, time);
            }
            ToolbarTool::MeasureAngle => {
                Self::render_measure_angle_anim(painter, card_rect, pending_points_count, time);
            }
            _ => {}
        }
    }

    /// Gambar frame kartu dasar bergaya dark glassmorphic.
    fn draw_card_base(painter: &egui::Painter, card_rect: Rect) {
        painter.rect_filled(
            card_rect,
            10.0,
            Color32::from_rgba_premultiplied(18, 20, 26, 235),
        );
        painter.rect_stroke(
            card_rect,
            10.0,
            Stroke::new(1.0, BORDER_SUBTLE),
            StrokeKind::Inside,
        );
    }

    /// Gambar header teks judul tool dan sub-langkah dinamis.
    fn draw_header(
        painter: &egui::Painter,
        card_rect: Rect,
        tool_name: &str,
        step_title: &str,
        step_color: Color32,
    ) {
        painter.text(
            Pos2::new(card_rect.left() + 10.0, card_rect.top() + 8.0),
            Align2::LEFT_TOP,
            tool_name,
            FontId::proportional(10.0),
            TEXT_SECONDARY,
        );
        painter.text(
            Pos2::new(card_rect.left() + 10.0, card_rect.top() + 21.0),
            Align2::LEFT_TOP,
            step_title,
            FontId::proportional(11.0),
            step_color,
        );
    }

    /// Gambar footer berisi tips shortcut atau panduan singkat.
    fn draw_footer(painter: &egui::Painter, card_rect: Rect, hint: &str) {
        painter.text(
            Pos2::new(card_rect.left() + 10.0, card_rect.bottom() - 10.0),
            Align2::LEFT_BOTTOM,
            hint,
            FontId::proportional(9.0),
            TEXT_MUTED,
        );
    }

    /// Gambar kursor mouse bergaya panah putih tajam beserta efek ripple klik.
    fn draw_cursor(
        painter: &egui::Painter,
        pos: Pos2,
        is_clicking: bool,
        time: f64,
    ) {
        if is_clicking {
            let ripple_radius = 4.0 + (time * 15.0).sin().abs() as f32 * 6.0;
            painter.circle_stroke(
                pos,
                ripple_radius,
                Stroke::new(1.5, ACCENT_BLUE.gamma_multiply(0.85)),
            );
        }

        let arrow_points = [
            pos,
            pos + Vec2::new(11.0, 9.0),
            pos + Vec2::new(5.0, 10.0),
            pos + Vec2::new(8.0, 16.0),
            pos + Vec2::new(5.0, 17.0),
            pos + Vec2::new(2.0, 11.0),
            pos + Vec2::new(-2.0, 14.0),
        ];
        painter.add(egui::Shape::convex_polygon(
            arrow_points.to_vec(),
            Color32::WHITE,
            Stroke::new(1.2, Color32::BLACK),
        ));
    }

    /// Gambar pill badge informasi kecil (misal dimensi/label).
    fn draw_badge(
        painter: &egui::Painter,
        pos: Pos2,
        text: &str,
        bg_color: Color32,
        text_color: Color32,
    ) {
        let badge_rect = Rect::from_center_size(pos, Vec2::new(64.0, 18.0));
        painter.rect_filled(badge_rect, 4.0, bg_color);
        painter.rect_stroke(
            badge_rect,
            4.0,
            Stroke::new(1.0, bg_color.gamma_multiply(1.5)),
            StrokeKind::Inside,
        );
        painter.text(
            pos,
            Align2::CENTER_CENTER,
            text,
            FontId::proportional(10.0),
            text_color,
        );
    }

    // ==========================================
    // 2D SKETCH TOOLS
    // ==========================================

    /// Line Tool
    fn render_line_anim(
        painter: &egui::Painter,
        card_rect: Rect,
        pending_points: usize,
        time: f64,
    ) {
        let cycle = 2.8;
        let phase = ((time % cycle) / cycle) as f32;

        let (step_title, step_color) = if pending_points > 0 {
            ("2. Tarik & Klik Titik Akhir (Langkah Aktif)", ACCENT_GREEN)
        } else if phase < 0.35 {
            ("1. Klik Titik Awal", ACCENT_ORANGE)
        } else {
            ("2. Tarik & Klik Titik Akhir", ACCENT_ORANGE)
        };

        Self::draw_header(
            painter,
            card_rect,
            "Panduan Line (Garis):",
            step_title,
            step_color,
        );
        Self::draw_footer(painter, card_rect, "💡 Tahan Shift untuk snap garis lurus 0°/45°/90°");

        let p1 = Pos2::new(card_rect.left() + 45.0, card_rect.bottom() - 40.0);
        let p2 = Pos2::new(card_rect.right() - 55.0, card_rect.top() + 48.0);

        let (cursor_pos, is_clicking, line_progress) = if phase < 0.35 {
            let t = (phase / 0.35).clamp(0.0, 1.0);
            let pos = Pos2::new(p1.x + (1.0 - t) * 20.0, p1.y + (1.0 - t) * 15.0);
            (pos, t > 0.75, 0.0)
        } else if phase < 0.80 {
            let t = ((phase - 0.35) / 0.45).clamp(0.0, 1.0);
            let pos = Pos2::new(p1.x + (p2.x - p1.x) * t, p1.y + (p2.y - p1.y) * t);
            (pos, t > 0.9, t)
        } else {
            (p2, false, 1.0)
        };

        // Gambar garis
        if line_progress > 0.0 {
            let cur_end = Pos2::new(p1.x + (p2.x - p1.x) * line_progress, p1.y + (p2.y - p1.y) * line_progress);
            painter.line_segment([p1, cur_end], Stroke::new(2.0, ACCENT_BLUE));

            // Badge panjang garis
            let mid = Pos2::new((p1.x + cur_end.x) * 0.5 - 10.0, (p1.y + cur_end.y) * 0.5 - 12.0);
            Self::draw_badge(
                painter,
                mid,
                "50.0 mm",
                Color32::from_rgba_premultiplied(20, 60, 110, 200),
                Color32::WHITE,
            );
        }

        // Titik awal & akhir
        painter.circle_filled(p1, 3.5, if phase >= 0.28 { ACCENT_ORANGE } else { TEXT_MUTED });
        if line_progress >= 0.95 {
            painter.circle_filled(p2, 3.5, ACCENT_ORANGE);
        }

        Self::draw_cursor(painter, cursor_pos, is_clicking, time);
    }

    /// Rectangle Tool
    fn render_rectangle_anim(
        painter: &egui::Painter,
        card_rect: Rect,
        pending_points: usize,
        time: f64,
    ) {
        let cycle = 3.0;
        let phase = ((time % cycle) / cycle) as f32;

        let (step_title, step_color) = if pending_points > 0 {
            ("2. Tarik ke Sudut Lawan (Langkah Aktif)", ACCENT_GREEN)
        } else if phase < 0.35 {
            ("1. Klik Sudut Pertama", ACCENT_ORANGE)
        } else {
            ("2. Tarik ke Sudut Diagonal", ACCENT_ORANGE)
        };

        Self::draw_header(
            painter,
            card_rect,
            "Panduan Rectangle (Kotak):",
            step_title,
            step_color,
        );
        Self::draw_footer(painter, card_rect, "💡 Sudut awal menjadi jangkar posisi kotak");

        let p1 = Pos2::new(card_rect.left() + 45.0, card_rect.top() + 45.0);
        let p2 = Pos2::new(card_rect.right() - 55.0, card_rect.bottom() - 38.0);

        let (cursor_pos, is_clicking, rect_progress) = if phase < 0.35 {
            let t = (phase / 0.35).clamp(0.0, 1.0);
            let pos = Pos2::new(p1.x - (1.0 - t) * 15.0, p1.y - (1.0 - t) * 15.0);
            (pos, t > 0.75, 0.0)
        } else if phase < 0.80 {
            let t = ((phase - 0.35) / 0.45).clamp(0.0, 1.0);
            let pos = Pos2::new(p1.x + (p2.x - p1.x) * t, p1.y + (p2.y - p1.y) * t);
            (pos, t > 0.9, t)
        } else {
            (p2, false, 1.0)
        };

        if rect_progress > 0.0 {
            let cur_max = Pos2::new(p1.x + (p2.x - p1.x) * rect_progress, p1.y + (p2.y - p1.y) * rect_progress);
            let r = Rect::from_min_max(p1, cur_max);
            painter.rect_filled(r, 2.0, ACCENT_BLUE.gamma_multiply(0.20));
            painter.rect_stroke(r, 2.0, Stroke::new(1.5, ACCENT_BLUE), StrokeKind::Inside);

            let badge_pos = Pos2::new(r.center().x, r.center().y);
            Self::draw_badge(
                painter,
                badge_pos,
                "60×40 mm",
                Color32::from_rgba_premultiplied(20, 60, 110, 220),
                Color32::WHITE,
            );
        }

        painter.circle_filled(p1, 3.5, if phase >= 0.28 { ACCENT_ORANGE } else { TEXT_MUTED });
        if rect_progress >= 0.95 {
            painter.circle_filled(p2, 3.5, ACCENT_ORANGE);
        }

        Self::draw_cursor(painter, cursor_pos, is_clicking, time);
    }

    /// Circle Tool
    fn render_circle_anim(
        painter: &egui::Painter,
        card_rect: Rect,
        pending_points: usize,
        time: f64,
    ) {
        let cycle = 2.8;
        let phase = ((time % cycle) / cycle) as f32;

        let (step_title, step_color) = if pending_points > 0 {
            ("2. Tarik Radius Jari-Jari (Langkah Aktif)", ACCENT_GREEN)
        } else if phase < 0.35 {
            ("1. Klik Titik Pusat Lingkaran", ACCENT_ORANGE)
        } else {
            ("2. Tarik & Tentukan Radius (R)", ACCENT_ORANGE)
        };

        Self::draw_header(
            painter,
            card_rect,
            "Panduan Circle (Lingkaran):",
            step_title,
            step_color,
        );
        Self::draw_footer(painter, card_rect, "💡 Ukuran radius dapat disesuaikan di popup");

        let center = Pos2::new(card_rect.left() + 75.0, card_rect.center().y + 4.0);
        let max_r = 28.0;

        let (cursor_pos, is_clicking, radius_progress) = if phase < 0.35 {
            let t = (phase / 0.35).clamp(0.0, 1.0);
            let pos = Pos2::new(center.x - (1.0 - t) * 15.0, center.y + (1.0 - t) * 10.0);
            (pos, t > 0.75, 0.0)
        } else if phase < 0.80 {
            let t = ((phase - 0.35) / 0.45).clamp(0.0, 1.0);
            let pos = Pos2::new(center.x + max_r * t, center.y);
            (pos, t > 0.9, t)
        } else {
            (Pos2::new(center.x + max_r, center.y), false, 1.0)
        };

        let current_r = max_r * radius_progress;
        if current_r > 0.0 {
            painter.circle_filled(center, current_r, ACCENT_BLUE.gamma_multiply(0.20));
            painter.circle_stroke(center, current_r, Stroke::new(1.5, ACCENT_BLUE));
            painter.line_segment(
                [center, Pos2::new(center.x + current_r, center.y)],
                Stroke::new(1.2, ACCENT_ORANGE),
            );

            let badge_pos = Pos2::new(card_rect.right() - 48.0, card_rect.center().y + 4.0);
            Self::draw_badge(
                painter,
                badge_pos,
                "R = 25 mm",
                Color32::from_rgba_premultiplied(20, 60, 110, 220),
                Color32::WHITE,
            );
        }

        painter.circle_filled(center, 3.5, if phase >= 0.28 { ACCENT_ORANGE } else { TEXT_MUTED });
        Self::draw_cursor(painter, cursor_pos, is_clicking, time);
    }

    /// Arc Tool (3-Points: Titik Awal -> Titik Lengkungan -> Titik Akhir)
    fn render_arc_anim(
        painter: &egui::Painter,
        card_rect: Rect,
        pending_points: usize,
        time: f64,
    ) {
        let cycle = 3.8;
        let phase = ((time % cycle) / cycle) as f32;

        let (step_title, step_color) = if pending_points == 1 {
            ("2. Klik Titik Lengkungan (Langkah Aktif)", ACCENT_GREEN)
        } else if pending_points >= 2 {
            ("3. Klik Titik Akhir Busur (Langkah Aktif)", ACCENT_GREEN)
        } else if phase < 0.28 {
            ("1. Klik Titik Awal Busur", ACCENT_ORANGE)
        } else if phase < 0.62 {
            ("2. Klik Titik Lengkungan (Kurva)", ACCENT_ORANGE)
        } else if phase < 0.90 {
            ("3. Klik Titik Akhir Busur", ACCENT_ORANGE)
        } else {
            ("Busur Terbentuk (3 Titik)", ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, "Panduan Arc (Busur 3-Titik):", step_title, step_color);
        Self::draw_footer(painter, card_rect, "💡 Urutan: Titik Awal → Lengkungan → Titik Akhir");

        // Tiga titik kunci busur
        let p1 = Pos2::new(card_rect.left() + 45.0, card_rect.bottom() - 36.0);
        let p2 = Pos2::new(card_rect.center().x - 5.0, card_rect.top() + 46.0);
        let p3 = Pos2::new(card_rect.right() - 45.0, card_rect.bottom() - 36.0);

        let (cursor_pos, is_clicking, step_phase) = if phase < 0.28 {
            let t = (phase / 0.28).clamp(0.0, 1.0);
            let pos = Pos2::new(p1.x - (1.0 - t) * 14.0, p1.y + (1.0 - t) * 10.0);
            (pos, t > 0.78, 1)
        } else if phase < 0.62 {
            let t = ((phase - 0.28) / 0.34).clamp(0.0, 1.0);
            let pos = Pos2::new(p1.x + (p2.x - p1.x) * t, p1.y + (p2.y - p1.y) * t);
            (pos, t > 0.85, 2)
        } else if phase < 0.90 {
            let t = ((phase - 0.62) / 0.28).clamp(0.0, 1.0);
            let pos = Pos2::new(p2.x + (p3.x - p2.x) * t, p2.y + (p3.y - p2.y) * t);
            (pos, t > 0.85, 3)
        } else {
            (Pos2::new(p3.x + 8.0, p3.y + 6.0), false, 4)
        };

        // Render preview grafis sesuai step
        match step_phase {
            1 => {
                // Langkah 1: Kursor menuju & klik titik awal
                painter.circle_filled(p1, 3.5, if phase >= 0.22 { ACCENT_ORANGE } else { TEXT_MUTED });
            }
            2 => {
                // Langkah 2: Garis lurus preview dari p1 ke kursor
                painter.circle_filled(p1, 3.5, ACCENT_ORANGE);
                painter.line_segment([p1, cursor_pos], Stroke::new(1.3, ACCENT_BLUE.gamma_multiply(0.75)));
                if phase >= 0.55 {
                    painter.circle_filled(p2, 3.5, ACCENT_ORANGE);
                }
            }
            3 => {
                // Langkah 3: Kurva busur sejati yang terbentuk dari p1, p2, ke posisi kursor saat ini
                painter.circle_filled(p1, 3.5, ACCENT_ORANGE);
                painter.circle_filled(p2, 3.5, ACCENT_ORANGE);
                Self::draw_arc_three_points(painter, p1, p2, cursor_pos, Stroke::new(2.0, ACCENT_BLUE));
                if phase >= 0.85 {
                    painter.circle_filled(p3, 3.5, ACCENT_ORANGE);
                }
            }
            _ => {
                // Langkah Selesai: Tampilkan busur penuh dan badge ukuran
                painter.circle_filled(p1, 3.5, ACCENT_ORANGE);
                painter.circle_filled(p2, 3.5, ACCENT_ORANGE);
                painter.circle_filled(p3, 3.5, ACCENT_ORANGE);
                Self::draw_arc_three_points(painter, p1, p2, p3, Stroke::new(2.0, ACCENT_BLUE));

                let badge_pos = Pos2::new(card_rect.center().x - 5.0, card_rect.center().y + 8.0);
                Self::draw_badge(
                    painter,
                    badge_pos,
                    "R = 35 mm",
                    Color32::from_rgba_premultiplied(20, 60, 110, 220),
                    Color32::WHITE,
                );
            }
        }

        // Tampilkan nomor urutan langkah di tiap titik
        let draw_point_label = |p: Pos2, num: &str, is_active: bool| {
            if is_active {
                painter.text(
                    Pos2::new(p.x, p.y - 9.0),
                    Align2::CENTER_CENTER,
                    num,
                    FontId::proportional(10.0),
                    TEXT_PRIMARY,
                );
            }
        };

        draw_point_label(p1, "1", phase >= 0.22);
        draw_point_label(p2, "2", phase >= 0.55);
        draw_point_label(p3, "3", phase >= 0.85);

        Self::draw_cursor(painter, cursor_pos, is_clicking, time);
    }

    /// Helper untuk menggambar kurva busur lingkaran 3 titik sejati (p1 -> p2 -> p3)
    fn draw_arc_three_points(
        painter: &egui::Painter,
        p1: Pos2,
        p2: Pos2,
        p3: Pos2,
        stroke: Stroke,
    ) {
        let (ax, ay) = (p1.x, p1.y);
        let (bx, by) = (p2.x, p2.y);
        let (cx, cy) = (p3.x, p3.y);
        let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
        if d.abs() < 1e-2 {
            painter.line_segment([p1, p2], stroke);
            painter.line_segment([p2, p3], stroke);
            return;
        }
        let a2 = ax * ax + ay * ay;
        let b2 = bx * bx + by * by;
        let c2 = cx * cx + cy * cy;
        let ux = (a2 * (by - cy) + b2 * (cy - ay) + c2 * (ay - by)) / d;
        let uy = (a2 * (cx - bx) + b2 * (ax - cx) + c2 * (bx - ax)) / d;
        let center = Pos2::new(ux, uy);
        let radius = (p1 - center).length();

        let a1 = (p1.y - center.y).atan2(p1.x - center.x);
        let a2_angle = (p2.y - center.y).atan2(p2.x - center.x);
        let a3 = (p3.y - center.y).atan2(p3.x - center.x);

        let ccw_span = |from: f32, to: f32| {
            let diff = to - from;
            if diff < 0.0 {
                diff + std::f32::consts::TAU
            } else {
                diff
            }
        };

        let span12 = ccw_span(a1, a2_angle);
        let span13 = ccw_span(a1, a3);

        let (start_angle, span) = if span12 <= span13 {
            (a1, span13)
        } else {
            (a3, ccw_span(a3, a1))
        };

        let segments = 24;
        let mut pts = Vec::with_capacity(segments + 1);
        for i in 0..=segments {
            let t = i as f32 / segments as f32;
            let ang = start_angle + t * span;
            pts.push(Pos2::new(center.x + radius * ang.cos(), center.y + radius * ang.sin()));
        }
        for w in pts.windows(2) {
            painter.line_segment([w[0], w[1]], stroke);
        }
    }

    /// Ellipse Tool
    fn render_ellipse_anim(
        painter: &egui::Painter,
        card_rect: Rect,
        _pending_points: usize,
        time: f64,
    ) {
        let cycle = 3.4;
        let phase = ((time % cycle) / cycle) as f32;

        let (step_title, step_color) = if phase < 0.33 {
            ("1. Klik Titik Pusat", ACCENT_ORANGE)
        } else if phase < 0.66 {
            ("2. Tarik Radius Mayor (Rx)", ACCENT_ORANGE)
        } else {
            ("3. Tarik Radius Minor (Ry)", ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, "Panduan Ellipse (Elips):", step_title, step_color);
        Self::draw_footer(painter, card_rect, "💡 Rx & Ry mengatur kelonjongan elips");

        let center = Pos2::new(card_rect.left() + 75.0, card_rect.center().y + 4.0);
        let max_rx = 34.0;
        let max_ry = 20.0;

        let (cursor_pos, is_clicking, rx_t, ry_t) = if phase < 0.33 {
            let t = (phase / 0.33).clamp(0.0, 1.0);
            let pos = Pos2::new(center.x - (1.0 - t) * 15.0, center.y);
            (pos, t > 0.8, 0.0, 0.0)
        } else if phase < 0.66 {
            let t = ((phase - 0.33) / 0.33).clamp(0.0, 1.0);
            let pos = Pos2::new(center.x + max_rx * t, center.y);
            (pos, t > 0.85, t, 0.2)
        } else {
            let t = ((phase - 0.66) / 0.34).clamp(0.0, 1.0);
            let pos = Pos2::new(center.x, center.y - max_ry * t);
            (pos, t > 0.85, 1.0, t)
        };

        let current_rx = max_rx * rx_t;
        let current_ry = max_ry * ry_t;

        if current_rx > 2.0 && current_ry > 2.0 {
            // Gambar elips bertahap
            let segments = 24;
            let mut ellipse_pts = Vec::with_capacity(segments + 1);
            for i in 0..=segments {
                let rad = i as f32 * std::f32::consts::TAU / segments as f32;
                ellipse_pts.push(Pos2::new(
                    center.x + current_rx * rad.cos(),
                    center.y + current_ry * rad.sin(),
                ));
            }
            for w in ellipse_pts.windows(2) {
                painter.line_segment([w[0], w[1]], Stroke::new(1.8, ACCENT_BLUE));
            }

            let badge_pos = Pos2::new(card_rect.right() - 48.0, card_rect.center().y + 4.0);
            Self::draw_badge(
                painter,
                badge_pos,
                "35×20 mm",
                Color32::from_rgba_premultiplied(20, 60, 110, 220),
                Color32::WHITE,
            );
        }

        painter.circle_filled(center, 3.5, if phase >= 0.28 { ACCENT_ORANGE } else { TEXT_MUTED });
        Self::draw_cursor(painter, cursor_pos, is_clicking, time);
    }

    // ==========================================
    // MODIFIERS & CONSTRAINTS
    // ==========================================

    /// Offset Tool
    fn render_offset_anim(
        painter: &egui::Painter,
        card_rect: Rect,
        _has_selection: bool,
        time: f64,
    ) {
        let cycle = 3.0;
        let phase = ((time % cycle) / cycle) as f32;

        let (step_title, step_color) = if phase < 0.40 {
            ("1. Klik Kurva Sumber", ACCENT_ORANGE)
        } else {
            ("2. Geser Jarak & Sisi Offset", ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, "Panduan Offset Sketsa:", step_title, step_color);
        Self::draw_footer(painter, card_rect, "💡 Arah geser mouse menentukan sisi luar/dalam");

        let p1 = Pos2::new(card_rect.left() + 40.0, card_rect.center().y + 12.0);
        let p2 = Pos2::new(card_rect.right() - 60.0, card_rect.center().y + 12.0);

        // Garis dasar
        painter.line_segment([p1, p2], Stroke::new(2.0, if phase >= 0.35 { ACCENT_ORANGE } else { TEXT_PRIMARY }));

        let (cursor_pos, is_clicking, offset_t) = if phase < 0.40 {
            let t = (phase / 0.40).clamp(0.0, 1.0);
            let pos = Pos2::new((p1.x + p2.x) * 0.5, p1.y + (1.0 - t) * 15.0);
            (pos, t > 0.8, 0.0)
        } else {
            let t = ((phase - 0.40) / 0.50).clamp(0.0, 1.0);
            let dist = 22.0 * t;
            let pos = Pos2::new((p1.x + p2.x) * 0.5, p1.y - dist);
            (pos, false, t)
        };

        if offset_t > 0.0 {
            let off_y = p1.y - 22.0 * offset_t;
            let op1 = Pos2::new(p1.x, off_y);
            let op2 = Pos2::new(p2.x, off_y);
            painter.line_segment([op1, op2], Stroke::new(1.8, ACCENT_BLUE));

            // Panah jarak offset
            let mid_x = (p1.x + p2.x) * 0.5;
            painter.line_segment(
                [Pos2::new(mid_x, p1.y), Pos2::new(mid_x, off_y)],
                Stroke::new(1.0, ACCENT_GREEN),
            );

            let badge_pos = Pos2::new(card_rect.right() - 44.0, card_rect.center().y - 6.0);
            Self::draw_badge(
                painter,
                badge_pos,
                "+10.0 mm",
                Color32::from_rgba_premultiplied(15, 80, 40, 220),
                Color32::WHITE,
            );
        }

        Self::draw_cursor(painter, cursor_pos, is_clicking, time);
    }

    /// Mirror Tool
    fn render_mirror_anim(
        painter: &egui::Painter,
        card_rect: Rect,
        _has_selection: bool,
        _pending_points: usize,
        time: f64,
    ) {
        let cycle = 3.4;
        let phase = ((time % cycle) / cycle) as f32;

        let (step_title, step_color) = if phase < 0.30 {
            ("1. Pilih Sketsa Sumber", ACCENT_ORANGE)
        } else if phase < 0.65 {
            ("2. Klik 2 Titik Sumbu Cermin", ACCENT_ORANGE)
        } else {
            ("3. Hasil Cermin Terduplikasi", ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, "Panduan Mirror (Cermin):", step_title, step_color);
        Self::draw_footer(painter, card_rect, "💡 Garis sumbu mendefinisikan bidang simetri");

        let axis_x = card_rect.left() + 110.0;
        let a1 = Pos2::new(axis_x, card_rect.top() + 38.0);
        let a2 = Pos2::new(axis_x, card_rect.bottom() - 36.0);

        // Garis sumbu putus-putus
        let segments = 6;
        for i in 0..segments {
            let u1 = i as f32 / segments as f32;
            let u2 = (i as f32 + 0.6) / segments as f32;
            painter.line_segment(
                [Pos2::new(axis_x, a1.y + (a2.y - a1.y) * u1), Pos2::new(axis_x, a1.y + (a2.y - a1.y) * u2)],
                Stroke::new(1.2, ACCENT_ORANGE),
            );
        }

        // Sketsa sumber (segitiga di kiri)
        let s1 = Pos2::new(axis_x - 40.0, card_rect.center().y + 12.0);
        let s2 = Pos2::new(axis_x - 15.0, card_rect.center().y + 12.0);
        let s3 = Pos2::new(axis_x - 15.0, card_rect.center().y - 15.0);
        painter.line_segment([s1, s2], Stroke::new(1.5, ACCENT_BLUE));
        painter.line_segment([s2, s3], Stroke::new(1.5, ACCENT_BLUE));
        painter.line_segment([s3, s1], Stroke::new(1.5, ACCENT_BLUE));

        let (cursor_pos, is_clicking) = if phase < 0.30 {
            let t = (phase / 0.30).clamp(0.0, 1.0);
            (Pos2::new(s3.x - (1.0 - t) * 15.0, s3.y), t > 0.8)
        } else if phase < 0.65 {
            let t = ((phase - 0.30) / 0.35).clamp(0.0, 1.0);
            let pos = Pos2::new(axis_x, a1.y + (a2.y - a1.y) * t);
            (pos, t > 0.85)
        } else {
            (a2, false)
        };

        // Sketsa hasil cermin di kanan
        if phase >= 0.65 {
            let m1 = Pos2::new(axis_x + 40.0, card_rect.center().y + 12.0);
            let m2 = Pos2::new(axis_x + 15.0, card_rect.center().y + 12.0);
            let m3 = Pos2::new(axis_x + 15.0, card_rect.center().y - 15.0);
            painter.line_segment([m1, m2], Stroke::new(1.5, ACCENT_GREEN));
            painter.line_segment([m2, m3], Stroke::new(1.5, ACCENT_GREEN));
            painter.line_segment([m3, m1], Stroke::new(1.5, ACCENT_GREEN));

            let badge_pos = Pos2::new(card_rect.right() - 36.0, card_rect.top() + 45.0);
            Self::draw_badge(
                painter,
                badge_pos,
                "⇄ Simetris",
                Color32::from_rgba_premultiplied(15, 80, 40, 220),
                Color32::WHITE,
            );
        }

        Self::draw_cursor(painter, cursor_pos, is_clicking, time);
    }

    /// Trim Tool
    fn render_trim_anim(
        painter: &egui::Painter,
        card_rect: Rect,
        time: f64,
    ) {
        let cycle = 3.0;
        let phase = ((time % cycle) / cycle) as f32;

        let (step_title, step_color) = if phase < 0.45 {
            ("1. Arahkan ke Garis Berpotongan", ACCENT_ORANGE)
        } else {
            ("2. Klik Segmen yang Mau Dipotong", ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, "Panduan Trim (Gunting):", step_title, step_color);
        Self::draw_footer(painter, card_rect, "💡 Memotong segmen garis hingga titik potong terdekat");

        let inter_x = card_rect.left() + 90.0;
        let inter_y = card_rect.center().y + 4.0;

        // Garis vertikal utama
        painter.line_segment(
            [Pos2::new(inter_x, card_rect.top() + 38.0), Pos2::new(inter_x, card_rect.bottom() - 36.0)],
            Stroke::new(2.0, ACCENT_BLUE),
        );

        // Garis horizontal kiri (tetap ada)
        painter.line_segment(
            [Pos2::new(card_rect.left() + 40.0, inter_y), Pos2::new(inter_x, inter_y)],
            Stroke::new(2.0, ACCENT_BLUE),
        );

        // Segmen kanan yang mau di-trim
        let seg_end = Pos2::new(card_rect.right() - 50.0, inter_y);

        let (cursor_pos, is_clicking) = if phase < 0.45 {
            let t = (phase / 0.45).clamp(0.0, 1.0);
            let pos = Pos2::new(seg_end.x - (1.0 - t) * 20.0, inter_y + (1.0 - t) * 15.0);
            (pos, false)
        } else {
            (Pos2::new((inter_x + seg_end.x) * 0.5, inter_y), phase < 0.70)
        };

        if phase < 0.65 {
            let col = if phase >= 0.45 { Color32::from_rgb(255, 69, 58) } else { ACCENT_BLUE };
            painter.line_segment([Pos2::new(inter_x, inter_y), seg_end], Stroke::new(2.0, col));
        } else {
            // Efek potongan gunting selesai
            let badge_pos = Pos2::new(card_rect.right() - 44.0, card_rect.center().y + 4.0);
            Self::draw_badge(
                painter,
                badge_pos,
                "✂ Terpotong",
                Color32::from_rgba_premultiplied(100, 30, 30, 220),
                Color32::WHITE,
            );
        }

        Self::draw_cursor(painter, cursor_pos, is_clicking, time);
    }

    /// Coincident Constraint
    fn render_coincident_anim(
        painter: &egui::Painter,
        card_rect: Rect,
        _pending: usize,
        time: f64,
    ) {
        let cycle = 3.0;
        let phase = ((time % cycle) / cycle) as f32;

        let (step_title, step_color) = if phase < 0.35 {
            ("1. Klik Titik Pertama", ACCENT_ORANGE)
        } else if phase < 0.70 {
            ("2. Klik Titik Target / Garis", ACCENT_ORANGE)
        } else {
            ("3. Titik Menyatu (Coincident)", ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, "Panduan Coincident (Penyatuan Titik):", step_title, step_color);
        Self::draw_footer(painter, card_rect, "💡 Menempelkan 2 titik atau titik ke garis secara permanen");

        let p1_orig = Pos2::new(card_rect.left() + 50.0, card_rect.center().y - 10.0);
        let p2 = Pos2::new(card_rect.right() - 70.0, card_rect.center().y + 10.0);

        let (cursor_pos, is_clicking, merge_t) = if phase < 0.35 {
            let t = (phase / 0.35).clamp(0.0, 1.0);
            (Pos2::new(p1_orig.x + (1.0 - t) * 15.0, p1_orig.y), t > 0.8, 0.0)
        } else if phase < 0.70 {
            let t = ((phase - 0.35) / 0.35).clamp(0.0, 1.0);
            (Pos2::new(p1_orig.x + (p2.x - p1_orig.x) * t, p1_orig.y + (p2.y - p1_orig.y) * t), t > 0.85, 0.0)
        } else {
            let t = ((phase - 0.70) / 0.20).clamp(0.0, 1.0);
            (p2, false, t)
        };

        let current_p1 = Pos2::new(
            p1_orig.x + (p2.x - p1_orig.x) * merge_t,
            p1_orig.y + (p2.y - p1_orig.y) * merge_t,
        );

        painter.circle_filled(current_p1, 4.0, ACCENT_BLUE);
        painter.circle_filled(p2, 4.0, ACCENT_GREEN);

        if merge_t > 0.8 {
            let badge_pos = Pos2::new(card_rect.right() - 44.0, card_rect.top() + 45.0);
            Self::draw_badge(
                painter,
                badge_pos,
                "🔗 Menyatu",
                Color32::from_rgba_premultiplied(15, 80, 40, 220),
                Color32::WHITE,
            );
        }

        Self::draw_cursor(painter, cursor_pos, is_clicking, time);
    }

    /// Fixed Constraint
    fn render_fixed_anim(
        painter: &egui::Painter,
        card_rect: Rect,
        time: f64,
    ) {
        let cycle = 2.6;
        let phase = ((time % cycle) / cycle) as f32;

        let (step_title, step_color) = if phase < 0.45 {
            ("1. Klik Titik yang Mau Dikunci", ACCENT_ORANGE)
        } else {
            ("2. Posisi Terkunci (Anchor)", ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, "Panduan Fixed (Kunci Posisi):", step_title, step_color);
        Self::draw_footer(painter, card_rect, "💡 Titik fixed tidak akan bergeser oleh solver sketsa");

        let p = Pos2::new(card_rect.center().x - 20.0, card_rect.center().y + 4.0);

        let (cursor_pos, is_clicking) = if phase < 0.45 {
            let t = (phase / 0.45).clamp(0.0, 1.0);
            (Pos2::new(p.x + (1.0 - t) * 20.0, p.y + (1.0 - t) * 15.0), t > 0.8)
        } else {
            (p, false)
        };

        let is_locked = phase >= 0.45;
        painter.circle_filled(p, 4.5, if is_locked { ACCENT_GREEN } else { ACCENT_BLUE });

        if is_locked {
            // Ikon gembok / jangkar
            painter.circle_stroke(p, 8.0, Stroke::new(1.5, ACCENT_GREEN));
            let badge_pos = Pos2::new(card_rect.right() - 48.0, card_rect.center().y + 4.0);
            Self::draw_badge(
                painter,
                badge_pos,
                "⚓ Terkunci",
                Color32::from_rgba_premultiplied(15, 80, 40, 220),
                Color32::WHITE,
            );
        }

        Self::draw_cursor(painter, cursor_pos, is_clicking, time);
    }

    /// Symmetric Constraint
    fn render_symmetric_anim(
        painter: &egui::Painter,
        card_rect: Rect,
        _pending: usize,
        time: f64,
    ) {
        let cycle = 3.4;
        let phase = ((time % cycle) / cycle) as f32;

        let (step_title, step_color) = if phase < 0.33 {
            ("1. Klik Titik 1 & Titik 2", ACCENT_ORANGE)
        } else if phase < 0.66 {
            ("2. Klik Garis Sumbu Simetri", ACCENT_ORANGE)
        } else {
            ("3. Jarak Terikat Simetris", ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, "Panduan Symmetric (Simetris):", step_title, step_color);
        Self::draw_footer(painter, card_rect, "💡 Menjaga jarak kedua titik seimbang terhadap sumbu");

        let axis_x = card_rect.left() + 105.0;
        let p_left = Pos2::new(axis_x - 40.0, card_rect.center().y + 4.0);
        let p_right = Pos2::new(axis_x + 40.0, card_rect.center().y + 4.0);

        // Garis sumbu tengah
        painter.line_segment(
            [Pos2::new(axis_x, card_rect.top() + 38.0), Pos2::new(axis_x, card_rect.bottom() - 36.0)],
            Stroke::new(1.2, ACCENT_ORANGE),
        );

        painter.circle_filled(p_left, 4.0, ACCENT_BLUE);
        painter.circle_filled(p_right, 4.0, ACCENT_BLUE);

        let (cursor_pos, is_clicking) = if phase < 0.33 {
            let t = (phase / 0.33).clamp(0.0, 1.0);
            (Pos2::new(p_left.x + (1.0 - t) * 15.0, p_left.y), t > 0.8)
        } else if phase < 0.66 {
            let t = ((phase - 0.33) / 0.33).clamp(0.0, 1.0);
            (Pos2::new(axis_x, card_rect.top() + 45.0 + t * 25.0), t > 0.8)
        } else {
            (Pos2::new(axis_x, card_rect.center().y + 4.0), false)
        };

        if phase >= 0.66 {
            painter.line_segment([p_left, p_right], Stroke::new(1.0, ACCENT_GREEN.gamma_multiply(0.6)));
            let badge_pos = Pos2::new(card_rect.right() - 40.0, card_rect.top() + 45.0);
            Self::draw_badge(
                painter,
                badge_pos,
                "↔ Simetris",
                Color32::from_rgba_premultiplied(15, 80, 40, 220),
                Color32::WHITE,
            );
        }

        Self::draw_cursor(painter, cursor_pos, is_clicking, time);
    }

    // ==========================================
    // 3D SOLID MODELING TOOLS
    // ==========================================

    /// Extrude Tool
    fn render_extrude_anim(
        painter: &egui::Painter,
        card_rect: Rect,
        _has_selection: bool,
        time: f64,
    ) {
        let cycle = 3.2;
        let phase = ((time % cycle) / cycle) as f32;

        let (step_title, step_color) = if phase < 0.35 {
            ("1. Pilih Profil 2D Tertutup", ACCENT_ORANGE)
        } else {
            ("2. Tarik Panah Ketinggian 3D", ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, "Panduan Extrude (Tarik Padat 3D):", step_title, step_color);
        Self::draw_footer(painter, card_rect, "💡 Tarik panah gizmo atau klik dimensi ruler");

        // Gambar prisma isometrik 3D
        let base_c = Pos2::new(card_rect.left() + 75.0, card_rect.bottom() - 40.0);
        let w = 32.0;
        let d = 16.0;
        let max_h = 30.0;

        let (cursor_pos, is_clicking, h_t) = if phase < 0.35 {
            let t = (phase / 0.35).clamp(0.0, 1.0);
            (Pos2::new(base_c.x + (1.0 - t) * 15.0, base_c.y), t > 0.8, 0.0)
        } else {
            let t = ((phase - 0.35) / 0.50).clamp(0.0, 1.0);
            let pos = Pos2::new(base_c.x, base_c.y - max_h * t - 10.0);
            (pos, false, t)
        };

        let current_h = max_h * h_t;

        // Alas bawah
        let b1 = Pos2::new(base_c.x, base_c.y + d * 0.5);
        let b2 = Pos2::new(base_c.x + w * 0.5, base_c.y);
        let b3 = Pos2::new(base_c.x, base_c.y - d * 0.5);
        let b4 = Pos2::new(base_c.x - w * 0.5, base_c.y);

        let t1 = Pos2::new(b1.x, b1.y - current_h);
        let t2 = Pos2::new(b2.x, b2.y - current_h);
        let t3 = Pos2::new(b3.x, b3.y - current_h);
        let t4 = Pos2::new(b4.x, b4.y - current_h);

        // Sisi padat 3D
        if current_h > 2.0 {
            // Sisi depan kiri
            painter.add(egui::Shape::convex_polygon(
                vec![b1, b4, t4, t1],
                ACCENT_BLUE.gamma_multiply(0.25),
                Stroke::new(1.0, ACCENT_BLUE),
            ));
            // Sisi depan kanan
            painter.add(egui::Shape::convex_polygon(
                vec![b1, b2, t2, t1],
                ACCENT_BLUE.gamma_multiply(0.40),
                Stroke::new(1.0, ACCENT_BLUE),
            ));
        }

        // Tutup atas
        painter.add(egui::Shape::convex_polygon(
            vec![t1, t2, t3, t4],
            ACCENT_BLUE.gamma_multiply(0.55),
            Stroke::new(1.2, ACCENT_BLUE),
        ));

        // Panah ketinggian 3D
        if h_t > 0.2 {
            painter.line_segment([t1, Pos2::new(t1.x, t1.y - 12.0)], Stroke::new(2.0, ACCENT_GREEN));
            let badge_pos = Pos2::new(card_rect.right() - 48.0, card_rect.center().y + 4.0);
            Self::draw_badge(
                painter,
                badge_pos,
                "H = 20 mm",
                Color32::from_rgba_premultiplied(15, 80, 40, 220),
                Color32::WHITE,
            );
        }

        Self::draw_cursor(painter, cursor_pos, is_clicking, time);
    }

    /// Loft Tool
    fn render_loft_anim(
        painter: &egui::Painter,
        card_rect: Rect,
        _has_selection: bool,
        time: f64,
    ) {
        let cycle = 4.2;
        let phase = ((time % cycle) / cycle) as f32;

        let (step_title, step_color) = if phase < 0.33 {
            ("1. Drag Kotak / Klik 2 Profil 2D", ACCENT_ORANGE)
        } else if phase < 0.66 {
            ("2. Opsi: Satukan Titik Tengah", ACCENT_BLUE)
        } else {
            ("3. Atur Tinggi di Top Bar & Buat 3D", ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, "Panduan Loft 3D (Mode 2D):", step_title, step_color);
        Self::draw_footer(painter, card_rect, "💡 Pilih 2 profil di kanvas -> atur tinggi di Top Bar -> Enter");

        let p1_orig = Pos2::new(card_rect.left() + 45.0, card_rect.bottom() - 36.0);
        let p2_orig = Pos2::new(card_rect.left() + 85.0, card_rect.bottom() - 36.0);

        // Jika phase >= 0.33, lingkaran bergeser ke tengah persegi (alignment)
        let circ_center = if phase < 0.33 {
            p2_orig
        } else if phase < 0.66 {
            let t = ((phase - 0.33) / 0.33).clamp(0.0, 1.0);
            Pos2::new(p2_orig.x + (p1_orig.x - p2_orig.x) * t, p2_orig.y)
        } else {
            p1_orig
        };

        let r_shape = Rect::from_center_size(p1_orig, Vec2::new(26.0, 24.0));

        // Gambar kotak seleksi drag di fase 1
        if phase < 0.33 {
            let t = (phase / 0.33).clamp(0.0, 1.0);
            let sel_min = Pos2::new(card_rect.left() + 25.0, card_rect.bottom() - 55.0);
            let sel_max = Pos2::new(
                sel_min.x + (card_rect.left() + 105.0 - sel_min.x) * t,
                sel_min.y + (card_rect.bottom() - 20.0 - sel_min.y) * t,
            );
            let sel_rect = Rect::from_min_max(sel_min, sel_max);
            painter.rect_filled(sel_rect, 2.0, ACCENT_BLUE.gamma_multiply(0.15));
            painter.rect_stroke(sel_rect, 2.0, Stroke::new(1.0, ACCENT_BLUE), StrokeKind::Inside);
        }

        // Bentuk 1 (Persegi)
        painter.rect_stroke(
            r_shape,
            2.0,
            Stroke::new(1.8, if phase >= 0.25 { ACCENT_ORANGE } else { ACCENT_BLUE }),
            StrokeKind::Inside,
        );

        // Bentuk 2 (Lingkaran)
        painter.circle_stroke(
            circ_center,
            10.0,
            Stroke::new(1.8, if phase >= 0.25 { ACCENT_ORANGE } else { ACCENT_GREEN }),
        );

        let (cursor_pos, is_clicking) = if phase < 0.33 {
            let t = (phase / 0.33).clamp(0.0, 1.0);
            (
                Pos2::new(card_rect.left() + 25.0 + t * 80.0, card_rect.bottom() - 55.0 + t * 35.0),
                true,
            )
        } else if phase < 0.66 {
            (Pos2::new(card_rect.center().x, card_rect.top() + 48.0), false)
        } else {
            (Pos2::new(card_rect.right() - 30.0, card_rect.top() + 20.0), true)
        };

        if phase >= 0.66 {
            // Representasi 3D Loft terangkat ke atas
            let top_c = Pos2::new(card_rect.right() - 48.0, card_rect.top() + 42.0);
            let b_base = Pos2::new(card_rect.right() - 48.0, card_rect.bottom() - 30.0);

            // Garis transisi 3D
            painter.line_segment([Pos2::new(b_base.x - 13.0, b_base.y), Pos2::new(top_c.x - 10.0, top_c.y)], Stroke::new(1.5, ACCENT_GREEN));
            painter.line_segment([Pos2::new(b_base.x + 13.0, b_base.y), Pos2::new(top_c.x + 10.0, top_c.y)], Stroke::new(1.5, ACCENT_GREEN));
            painter.circle_stroke(top_c, 10.0, Stroke::new(1.5, ACCENT_GREEN));

            let badge_pos = Pos2::new(card_rect.right() - 48.0, card_rect.center().y + 4.0);
            Self::draw_badge(
                painter,
                badge_pos,
                "✓ Loft 3D",
                Color32::from_rgba_premultiplied(15, 80, 40, 220),
                Color32::WHITE,
            );
        }

        Self::draw_cursor(painter, cursor_pos, is_clicking, time);
    }

    /// Fillet & Chamfer Tool
    fn render_fillet_anim(
        painter: &egui::Painter,
        card_rect: Rect,
        _has_selection: bool,
        time: f64,
    ) {
        let cycle = 3.2;
        let phase = ((time % cycle) / cycle) as f32;

        let (step_title, step_color) = if phase < 0.40 {
            ("1. Pilih Tepi (Edge) 3D", ACCENT_ORANGE)
        } else {
            ("2. Geser Radius Lengkung (Fillet)", ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, "Panduan Fillet & Chamfer:", step_title, step_color);
        Self::draw_footer(painter, card_rect, "💡 Fillet untuk lengkung halus, Chamfer untuk sudut miring");

        let corner = Pos2::new(card_rect.left() + 65.0, card_rect.top() + 45.0);
        let p_down = Pos2::new(corner.x, card_rect.bottom() - 36.0);
        let p_right = Pos2::new(card_rect.left() + 130.0, corner.y);

        let (cursor_pos, is_clicking, r_t) = if phase < 0.40 {
            let t = (phase / 0.40).clamp(0.0, 1.0);
            (Pos2::new(corner.x + (1.0 - t) * 15.0, corner.y + (1.0 - t) * 15.0), t > 0.8, 0.0)
        } else {
            let t = ((phase - 0.40) / 0.50).clamp(0.0, 1.0);
            (Pos2::new(corner.x + 18.0 * t, corner.y + 18.0 * t), false, t)
        };

        let radius = 18.0 * r_t;

        if radius < 2.0 {
            // Sudut tajam
            painter.line_segment([p_down, corner], Stroke::new(2.0, ACCENT_BLUE));
            painter.line_segment([corner, p_right], Stroke::new(2.0, ACCENT_BLUE));
        } else {
            // Sudut membulat (Fillet)
            let f_down = Pos2::new(corner.x, corner.y + radius);
            let f_right = Pos2::new(corner.x + radius, corner.y);

            painter.line_segment([p_down, f_down], Stroke::new(2.0, ACCENT_BLUE));
            painter.line_segment([f_right, p_right], Stroke::new(2.0, ACCENT_BLUE));

            // Busur fillet
            painter.line_segment([f_down, f_right], Stroke::new(2.0, ACCENT_GREEN));

            let badge_pos = Pos2::new(card_rect.right() - 48.0, card_rect.center().y + 4.0);
            Self::draw_badge(
                painter,
                badge_pos,
                "R = 5.0 mm",
                Color32::from_rgba_premultiplied(15, 80, 40, 220),
                Color32::WHITE,
            );
        }

        Self::draw_cursor(painter, cursor_pos, is_clicking, time);
    }

    /// Shell Tool
    fn render_shell_anim(
        painter: &egui::Painter,
        card_rect: Rect,
        _has_selection: bool,
        time: f64,
    ) {
        let cycle = 3.2;
        let phase = ((time % cycle) / cycle) as f32;

        let (step_title, step_color) = if phase < 0.40 {
            ("1. Pilih Face Terbuka (Open Face)", ACCENT_ORANGE)
        } else {
            ("2. Bentuk Dinding Tipis (Hollow)", ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, "Panduan Shell (Bodi Berongga):", step_title, step_color);
        Self::draw_footer(painter, card_rect, "💡 Mengosongkan bagian dalam benda padat dengan ketebalan t");

        let box_rect = Rect::from_center_size(
            Pos2::new(card_rect.left() + 75.0, card_rect.center().y + 4.0),
            Vec2::new(48.0, 42.0),
        );

        painter.rect_filled(box_rect, 3.0, ACCENT_BLUE.gamma_multiply(0.25));
        painter.rect_stroke(box_rect, 3.0, Stroke::new(1.5, ACCENT_BLUE), StrokeKind::Inside);

        let (cursor_pos, is_clicking) = if phase < 0.40 {
            let t = (phase / 0.40).clamp(0.0, 1.0);
            (Pos2::new(box_rect.center().x + (1.0 - t) * 15.0, box_rect.top() + (1.0 - t) * 10.0), t > 0.8)
        } else {
            (Pos2::new(box_rect.center().x, box_rect.top()), false)
        };

        if phase >= 0.40 {
            // Rongga dalam
            let inner_rect = box_rect.shrink(6.0);
            painter.rect_filled(inner_rect, 2.0, Color32::from_rgba_premultiplied(10, 12, 16, 240));
            painter.rect_stroke(inner_rect, 2.0, Stroke::new(1.2, ACCENT_GREEN), StrokeKind::Inside);

            let badge_pos = Pos2::new(card_rect.right() - 48.0, card_rect.center().y + 4.0);
            Self::draw_badge(
                painter,
                badge_pos,
                "t = 2.0 mm",
                Color32::from_rgba_premultiplied(15, 80, 40, 220),
                Color32::WHITE,
            );
        }

        Self::draw_cursor(painter, cursor_pos, is_clicking, time);
    }

    /// Boolean Tool (Union, Subtract, Intersect)
    fn render_boolean_anim(
        painter: &egui::Painter,
        card_rect: Rect,
        time: f64,
    ) {
        let cycle = 3.8;
        let phase = ((time % cycle) / cycle) as f32;

        let (step_title, step_color, op_name) = if phase < 0.33 {
            ("1. Mode: Union ∪ (Gabung Bodi)", ACCENT_BLUE, "∪ Gabung")
        } else if phase < 0.66 {
            ("2. Mode: Subtract - (Potong Bodi)", ACCENT_ORANGE, "- Potong")
        } else {
            ("3. Mode: Intersect ∩ (Irisan)", ACCENT_GREEN, "∩ Irisan")
        };

        Self::draw_header(painter, card_rect, "Panduan Boolean 3D:", step_title, step_color);
        Self::draw_footer(painter, card_rect, "💡 Pilih 2 bodi padat untuk menggabungkan atau memotong");

        let c1 = Pos2::new(card_rect.left() + 65.0, card_rect.center().y + 4.0);
        let c2 = Pos2::new(card_rect.left() + 85.0, card_rect.center().y + 4.0);

        let r1 = Rect::from_center_size(c1, Vec2::new(36.0, 36.0));
        let r2 = Rect::from_center_size(c2, Vec2::new(36.0, 36.0));

        painter.rect_filled(r1, 3.0, ACCENT_BLUE.gamma_multiply(0.30));
        painter.rect_stroke(r1, 3.0, Stroke::new(1.2, ACCENT_BLUE), StrokeKind::Inside);

        painter.rect_filled(r2, 3.0, step_color.gamma_multiply(0.30));
        painter.rect_stroke(r2, 3.0, Stroke::new(1.2, step_color), StrokeKind::Inside);

        let badge_pos = Pos2::new(card_rect.right() - 48.0, card_rect.center().y + 4.0);
        Self::draw_badge(
            painter,
            badge_pos,
            op_name,
            step_color.gamma_multiply(0.3),
            Color32::WHITE,
        );
    }

    /// Section View Tool
    fn render_section_anim(
        painter: &egui::Painter,
        card_rect: Rect,
        time: f64,
    ) {
        let cycle = 3.0;
        let phase = ((time % cycle) / cycle) as f32;

        let (step_title, step_color) = if phase < 0.40 {
            ("1. Pilih Bidang Potong (XY/XZ/YZ)", ACCENT_ORANGE)
        } else {
            ("2. Geser Jarak Penampang Dalam", ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, "Panduan Section View (Irisan Dalam):", step_title, step_color);
        Self::draw_footer(painter, card_rect, "💡 Menginspeksi rongga internal tanpa merusak 3D");

        let box_rect = Rect::from_center_size(
            Pos2::new(card_rect.left() + 75.0, card_rect.center().y + 4.0),
            Vec2::new(50.0, 38.0),
        );

        painter.rect_filled(box_rect, 2.0, ACCENT_BLUE.gamma_multiply(0.20));
        painter.rect_stroke(box_rect, 2.0, Stroke::new(1.2, ACCENT_BLUE), StrokeKind::Inside);

        // Bidang potong bergerak
        let plane_x = box_rect.left() + box_rect.width() * ((phase * 1.5).min(1.0));
        painter.line_segment(
            [Pos2::new(plane_x, box_rect.top() - 6.0), Pos2::new(plane_x, box_rect.bottom() + 6.0)],
            Stroke::new(2.0, ACCENT_ORANGE),
        );

        let badge_pos = Pos2::new(card_rect.right() - 48.0, card_rect.center().y + 4.0);
        Self::draw_badge(
            painter,
            badge_pos,
            "🔍 Potongan",
            Color32::from_rgba_premultiplied(70, 45, 15, 220),
            Color32::WHITE,
        );
    }

    // ==========================================
    // MEASUREMENT & UTILITIES
    // ==========================================

    /// Measure Tool
    fn render_measure_dist_anim(
        painter: &egui::Painter,
        card_rect: Rect,
        pending: usize,
        time: f64,
    ) {
        let cycle = 2.8;
        let phase = ((time % cycle) / cycle) as f32;

        let (step_title, step_color) = if pending > 0 {
            ("2. Klik Titik Kedua (Langkah Aktif)", ACCENT_GREEN)
        } else if phase < 0.40 {
            ("1. Klik Titik Pertama", ACCENT_ORANGE)
        } else {
            ("2. Klik Titik Kedua", ACCENT_ORANGE)
        };

        Self::draw_header(painter, card_rect, "Panduan Measure (Ukur Jarak):", step_title, step_color);
        Self::draw_footer(painter, card_rect, "💡 Pengukuran non-destruktif untuk inspeksi dimensi");

        let p1 = Pos2::new(card_rect.left() + 45.0, card_rect.bottom() - 38.0);
        let p2 = Pos2::new(card_rect.right() - 65.0, card_rect.top() + 45.0);

        let (cursor_pos, is_clicking, dist_t) = if phase < 0.40 {
            let t = (phase / 0.40).clamp(0.0, 1.0);
            (Pos2::new(p1.x + (1.0 - t) * 15.0, p1.y), t > 0.8, 0.0)
        } else {
            let t = ((phase - 0.40) / 0.50).clamp(0.0, 1.0);
            (Pos2::new(p1.x + (p2.x - p1.x) * t, p1.y + (p2.y - p1.y) * t), t > 0.85, t)
        };

        painter.circle_filled(p1, 3.5, ACCENT_ORANGE);
        if dist_t > 0.0 {
            let cur_end = Pos2::new(p1.x + (p2.x - p1.x) * dist_t, p1.y + (p2.y - p1.y) * dist_t);
            painter.line_segment([p1, cur_end], Stroke::new(1.8, ACCENT_GREEN));

            let badge_pos = Pos2::new(card_rect.right() - 48.0, card_rect.center().y + 4.0);
            Self::draw_badge(
                painter,
                badge_pos,
                "📏 42.5 mm",
                Color32::from_rgba_premultiplied(15, 80, 40, 220),
                Color32::WHITE,
            );
        }

        Self::draw_cursor(painter, cursor_pos, is_clicking, time);
    }

    /// Measure Angle Tool
    fn render_measure_angle_anim(
        painter: &egui::Painter,
        card_rect: Rect,
        _pending: usize,
        time: f64,
    ) {
        let cycle = 3.0;
        let phase = ((time % cycle) / cycle) as f32;

        let (step_title, step_color) = if phase < 0.40 {
            ("1. Klik Garis Pertama", ACCENT_ORANGE)
        } else {
            ("2. Klik Garis Kedua", ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, "Panduan Measure Angle (Ukur Sudut):", step_title, step_color);
        Self::draw_footer(painter, card_rect, "💡 Mengukur sudut presisi dalam satuan derajat (°)");

        let vertex = Pos2::new(card_rect.left() + 45.0, card_rect.bottom() - 36.0);
        let l1_end = Pos2::new(card_rect.left() + 110.0, vertex.y);
        let l2_end = Pos2::new(card_rect.left() + 95.0, card_rect.top() + 42.0);

        painter.line_segment([vertex, l1_end], Stroke::new(2.0, if phase >= 0.35 { ACCENT_ORANGE } else { TEXT_PRIMARY }));
        painter.line_segment([vertex, l2_end], Stroke::new(2.0, if phase >= 0.70 { ACCENT_GREEN } else { TEXT_PRIMARY }));

        let (cursor_pos, is_clicking) = if phase < 0.40 {
            let t = (phase / 0.40).clamp(0.0, 1.0);
            (Pos2::new((vertex.x + l1_end.x) * 0.5, vertex.y + (1.0 - t) * 10.0), t > 0.8)
        } else {
            let t = ((phase - 0.40) / 0.50).clamp(0.0, 1.0);
            (Pos2::new((vertex.x + l2_end.x) * 0.5, (vertex.y + l2_end.y) * 0.5 + (1.0 - t) * 10.0), t > 0.85)
        };

        if phase >= 0.70 {
            // Busur sudut
            painter.circle_stroke(vertex, 16.0, Stroke::new(1.2, ACCENT_GREEN));
            let badge_pos = Pos2::new(card_rect.right() - 48.0, card_rect.center().y + 4.0);
            Self::draw_badge(
                painter,
                badge_pos,
                "📐 45.0°",
                Color32::from_rgba_premultiplied(15, 80, 40, 220),
                Color32::WHITE,
            );
        }

        Self::draw_cursor(painter, cursor_pos, is_clicking, time);
    }
}
