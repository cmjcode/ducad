//! Animated Interactive Tool Tutorials (Panduan Animasi Visual Interaktif Semua Tools).
//!
//! Menampilkan kartu panduan visual animasi di pojok kiri bawah kanvas HUD
//! dengan demonstrasi langkah interaktif, kursor animasi, feedback klik,
//! dan visualisasi hasil secara real-time.

use ducad_i18n::t;
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

        // Posisi kartu: pojok kiri bawah kanvas, di atas status bar
        let card_width = 310.0;
        let card_height = 148.0;
        let margin_bottom = 44.0;
        let margin_left = 68.0; // Di sebelah kanan left toolbar

        let card_rect = Rect::from_min_size(
            Pos2::new(
                canvas_rect.left() + margin_left,
                canvas_rect.bottom() - card_height - margin_bottom,
            ),
            Vec2::new(card_width, card_height),
        );

        let painter = ui.painter_at(card_rect);

        // Latar belakang kartu semi-transparan modern dengan border halus
        painter.rect_filled(
            card_rect,
            8.0,
            Color32::from_rgba_premultiplied(12, 14, 18, 230),
        );
        painter.rect_stroke(
            card_rect,
            8.0,
            Stroke::new(1.0, BORDER_SUBTLE),
            StrokeKind::Inside,
        );

        // 2. Render konten diagram animasi sesuai tool
        match tool {
            ToolbarTool::Line => Self::render_line_anim(&painter, card_rect, pending_points_count, time),
            ToolbarTool::Rectangle => Self::render_rectangle_anim(&painter, card_rect, pending_points_count, time),
            ToolbarTool::Circle => Self::render_circle_anim(&painter, card_rect, pending_points_count, time),
            ToolbarTool::Arc => Self::render_arc_anim(&painter, card_rect, pending_points_count, time),
            ToolbarTool::Ellipse => Self::render_ellipse_anim(&painter, card_rect, pending_points_count, time),
            ToolbarTool::Spline => Self::render_spline_anim(&painter, card_rect, pending_points_count, time),
            ToolbarTool::Offset => Self::render_offset_anim(&painter, card_rect, has_selection, time),
            ToolbarTool::Mirror => Self::render_mirror_anim(&painter, card_rect, has_selection, pending_points_count, time),
            ToolbarTool::Trim => Self::render_trim_anim(&painter, card_rect, time),
            ToolbarTool::PointCoincident => Self::render_coincident_anim(&painter, card_rect, pending_points_count, time),
            ToolbarTool::PointFixed => Self::render_fixed_anim(&painter, card_rect, time),
            ToolbarTool::PointSymmetric => Self::render_symmetric_anim(&painter, card_rect, pending_points_count, time),
            ToolbarTool::Extrude => Self::render_extrude_anim(&painter, card_rect, has_selection, time),
            ToolbarTool::Loft => Self::render_loft_anim(&painter, card_rect, has_selection, time),
            ToolbarTool::Sweep => Self::render_sweep_anim(&painter, card_rect, time),
            ToolbarTool::Shell => Self::render_shell_anim(&painter, card_rect, has_selection, time),
            ToolbarTool::Boolean => Self::render_boolean_anim(&painter, card_rect, time),
            ToolbarTool::SectionView => Self::render_section_anim(&painter, card_rect, time),
            ToolbarTool::Measure => Self::render_measure_dist_anim(&painter, card_rect, pending_points_count, time),
            ToolbarTool::MeasureAngle => Self::render_measure_angle_anim(&painter, card_rect, pending_points_count, time),
            _ => {}
        }
    }

    // ==========================================
    // HELPER RENDERING (KOMPONEN VISUAL)
    // ==========================================

    /// Header judul dan langkah aktif
    fn draw_header(
        painter: &egui::Painter,
        card_rect: Rect,
        title: &str,
        step_text: &str,
        step_color: Color32,
    ) {
        let pos_title = Pos2::new(card_rect.left() + 10.0, card_rect.top() + 10.0);
        painter.text(
            pos_title,
            Align2::LEFT_TOP,
            title,
            FontId::proportional(11.0),
            TEXT_SECONDARY,
        );

        let pos_step = Pos2::new(card_rect.left() + 10.0, card_rect.top() + 24.0);
        painter.text(
            pos_step,
            Align2::LEFT_TOP,
            step_text,
            FontId::proportional(12.0),
            step_color,
        );
    }

    /// Footer tips navigasi & pintasan
    fn draw_footer(painter: &egui::Painter, card_rect: Rect, tip_text: &str) {
        let pos_tip = Pos2::new(card_rect.left() + 10.0, card_rect.bottom() - 14.0);
        painter.text(
            pos_tip,
            Align2::LEFT_BOTTOM,
            tip_text,
            FontId::proportional(10.0),
            TEXT_MUTED,
        );
    }

    /// Kursor panah interaktif dengan efek klik (gelombang lingkaran)
    fn draw_cursor(painter: &egui::Painter, pos: Pos2, is_clicking: bool, time: f64) {
        // Efek ripple klik
        if is_clicking {
            let click_wave = ((time * 4.0).sin() * 0.5 + 0.5) as f32;
            let radius = 6.0 + click_wave * 8.0;
            let alpha = ((1.0 - click_wave) * 200.0) as u8;
            painter.circle_stroke(
                pos,
                radius,
                Stroke::new(1.5, Color32::from_rgba_premultiplied(50, 150, 255, alpha)),
            );
        }

        // Gambar panah kursor mouse (vektor)
        let pts = [
            pos,
            Pos2::new(pos.x + 9.0, pos.y + 11.0),
            Pos2::new(pos.x + 4.5, pos.y + 10.5),
            Pos2::new(pos.x + 6.5, pos.y + 15.0),
            Pos2::new(pos.x + 4.0, pos.y + 16.0),
            Pos2::new(pos.x + 2.0, pos.y + 11.5),
            Pos2::new(pos.x - 2.0, pos.y + 13.5),
        ];

        painter.add(egui::Shape::convex_polygon(
            pts.to_vec(),
            if is_clicking { ACCENT_ORANGE } else { Color32::WHITE },
            Stroke::new(1.0, Color32::BLACK),
        ));
    }

    /// Badge status hasil / dimensi
    fn draw_badge(
        painter: &egui::Painter,
        pos: Pos2,
        text: &str,
        bg_color: Color32,
        text_color: Color32,
    ) {
        let galley = painter.layout_no_wrap(
            text.to_string(),
            FontId::proportional(10.5),
            text_color,
        );
        let badge_rect = Rect::from_center_size(
            pos,
            Vec2::new(galley.size().x + 12.0, galley.size().y + 6.0),
        );
        painter.rect_filled(badge_rect, 4.0, bg_color);
        painter.rect_stroke(
            badge_rect,
            4.0,
            Stroke::new(1.0, bg_color.gamma_multiply(1.5)),
            StrokeKind::Inside,
        );
        painter.galley(
            Pos2::new(
                badge_rect.center().x - galley.size().x * 0.5,
                badge_rect.center().y - galley.size().y * 0.5,
            ),
            galley,
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
            (t!("guide-line-step-2-active"), ACCENT_GREEN)
        } else if phase < 0.35 {
            (t!("guide-line-step-1"), ACCENT_ORANGE)
        } else {
            (t!("guide-line-step-2"), ACCENT_ORANGE)
        };

        Self::draw_header(
            painter,
            card_rect,
            &t!("guide-line-header"),
            &step_title,
            step_color,
        );
        Self::draw_footer(painter, card_rect, &t!("guide-line-tip"));

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
            (t!("guide-rect-step-2-active"), ACCENT_GREEN)
        } else if phase < 0.35 {
            (t!("guide-rect-step-1"), ACCENT_ORANGE)
        } else {
            (t!("guide-rect-step-2"), ACCENT_ORANGE)
        };

        Self::draw_header(
            painter,
            card_rect,
            &t!("guide-rect-header"),
            &step_title,
            step_color,
        );
        Self::draw_footer(painter, card_rect, &t!("guide-rect-tip"));

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
            (t!("guide-circle-step-2-active"), ACCENT_GREEN)
        } else if phase < 0.35 {
            (t!("guide-circle-step-1"), ACCENT_ORANGE)
        } else {
            (t!("guide-circle-step-2"), ACCENT_ORANGE)
        };

        Self::draw_header(
            painter,
            card_rect,
            &t!("guide-circle-header"),
            &step_title,
            step_color,
        );
        Self::draw_footer(painter, card_rect, &t!("guide-circle-tip"));

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
            (t!("guide-arc-step-2-active"), ACCENT_GREEN)
        } else if pending_points >= 2 {
            (t!("guide-arc-step-3-active"), ACCENT_GREEN)
        } else if phase < 0.28 {
            (t!("guide-arc-step-1"), ACCENT_ORANGE)
        } else if phase < 0.62 {
            (t!("guide-arc-step-2"), ACCENT_ORANGE)
        } else if phase < 0.90 {
            (t!("guide-arc-step-3"), ACCENT_ORANGE)
        } else {
            (t!("guide-arc-step-done"), ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, &t!("guide-arc-header"), &step_title, step_color);
        Self::draw_footer(painter, card_rect, &t!("guide-arc-tip"));

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
            (t!("guide-ellipse-step-1"), ACCENT_ORANGE)
        } else if phase < 0.66 {
            (t!("guide-ellipse-step-2"), ACCENT_ORANGE)
        } else {
            (t!("guide-ellipse-step-3"), ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, &t!("guide-ellipse-header"), &step_title, step_color);
        Self::draw_footer(painter, card_rect, &t!("guide-ellipse-tip"));

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

    /// Spline Tool (Kurva Organik Multi-Titik Catmull-Rom)
    fn render_spline_anim(
        painter: &egui::Painter,
        card_rect: Rect,
        pending_points: usize,
        time: f64,
    ) {
        let cycle = 4.2;
        let phase = ((time % cycle) / cycle) as f32;

        let (step_title, step_color) = if pending_points > 0 {
            (t!("guide-spline-step-active"), ACCENT_GREEN)
        } else if phase < 0.25 {
            (t!("guide-spline-step-1"), ACCENT_ORANGE)
        } else if phase < 0.70 {
            (t!("guide-spline-step-2"), ACCENT_ORANGE)
        } else {
            (t!("guide-spline-step-3"), ACCENT_GREEN)
        };

        Self::draw_header(
            painter,
            card_rect,
            &t!("guide-spline-header"),
            &step_title,
            step_color,
        );
        Self::draw_footer(painter, card_rect, &t!("guide-spline-tip"));

        // 4 titik kontrol untuk kurva S halus yang indah
        let p0 = Pos2::new(card_rect.left() + 45.0, card_rect.bottom() - 38.0);
        let p1 = Pos2::new(card_rect.left() + 105.0, card_rect.top() + 48.0);
        let p2 = Pos2::new(card_rect.right() - 110.0, card_rect.bottom() - 42.0);
        let p3 = Pos2::new(card_rect.right() - 45.0, card_rect.top() + 46.0);

        let (cursor_pos, is_clicking, pts_count, progress) = if phase < 0.25 {
            let t = (phase / 0.25).clamp(0.0, 1.0);
            let pos = Pos2::new(p0.x - (1.0 - t) * 15.0, p0.y + (1.0 - t) * 10.0);
            (pos, t > 0.8, 1, 0.0)
        } else if phase < 0.50 {
            let t = ((phase - 0.25) / 0.25).clamp(0.0, 1.0);
            let pos = Pos2::new(p0.x + (p1.x - p0.x) * t, p0.y + (p1.y - p0.y) * t);
            (pos, t > 0.85, 2, t)
        } else if phase < 0.75 {
            let t = ((phase - 0.50) / 0.25).clamp(0.0, 1.0);
            let pos = Pos2::new(p1.x + (p2.x - p1.x) * t, p1.y + (p2.y - p1.y) * t);
            (pos, t > 0.85, 3, t)
        } else {
            let t = ((phase - 0.75) / 0.25).clamp(0.0, 1.0);
            let pos = Pos2::new(p2.x + (p3.x - p2.x) * t, p2.y + (p3.y - p2.y) * t);
            (pos, t > 0.85, 4, t)
        };

        // Render titik-titik jangkar
        let all_pts = [p0, p1, p2, p3];
        for i in 0..pts_count {
            painter.circle_filled(all_pts[i], 3.0, ACCENT_BLUE);
            painter.circle_stroke(all_pts[i], 3.0, Stroke::new(1.0, Color32::WHITE));
        }

        // Gambar kurva spline bertahap
        if pts_count >= 2 {
            let active_pts: Vec<Pos2> = if pts_count == 2 {
                vec![p0, Pos2::new(p0.x + (p1.x - p0.x) * progress, p0.y + (p1.y - p0.y) * progress)]
            } else if pts_count == 3 {
                vec![p0, p1, Pos2::new(p1.x + (p2.x - p1.x) * progress, p1.y + (p2.y - p1.y) * progress)]
            } else {
                vec![p0, p1, p2, Pos2::new(p2.x + (p3.x - p2.x) * progress, p2.y + (p3.y - p2.y) * progress)]
            };

            let num_samples = 40;
            let mut curve_pts = Vec::with_capacity(num_samples + 1);
            let n = active_pts.len();
            for s in 0..=num_samples {
                let t_curve = s as f32 / num_samples as f32;
                if n == 2 {
                    curve_pts.push(Pos2::new(
                        active_pts[0].x + (active_pts[1].x - active_pts[0].x) * t_curve,
                        active_pts[0].y + (active_pts[1].y - active_pts[0].y) * t_curve,
                    ));
                } else if n == 3 {
                    let u = 1.0 - t_curve;
                    let x = u * u * active_pts[0].x + 2.0 * u * t_curve * active_pts[1].x + t_curve * t_curve * active_pts[2].x;
                    let y = u * u * active_pts[0].y + 2.0 * u * t_curve * active_pts[1].y + t_curve * t_curve * active_pts[2].y;
                    curve_pts.push(Pos2::new(x, y));
                } else {
                    let u = 1.0 - t_curve;
                    let x = u * u * u * active_pts[0].x + 3.0 * u * u * t_curve * active_pts[1].x + 3.0 * u * t_curve * t_curve * active_pts[2].x + t_curve * t_curve * t_curve * active_pts[3].x;
                    let y = u * u * u * active_pts[0].y + 3.0 * u * u * t_curve * active_pts[1].y + 3.0 * u * t_curve * t_curve * active_pts[2].y + t_curve * t_curve * t_curve * active_pts[3].y;
                    curve_pts.push(Pos2::new(x, y));
                }
            }

            for w in curve_pts.windows(2) {
                painter.line_segment([w[0], w[1]], Stroke::new(2.2, ACCENT_BLUE));
            }
        }

        if phase > 0.82 {
            Self::draw_badge(
                painter,
                Pos2::new(card_rect.center().x, card_rect.top() + 40.0),
                "Enter: Selesai",
                Color32::from_rgba_premultiplied(10, 132, 255, 60),
                Color32::from_rgb(120, 200, 255),
            );
        }

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
            (t!("guide-offset-step-1"), ACCENT_ORANGE)
        } else {
            (t!("guide-offset-step-2"), ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, &t!("guide-offset-header"), &step_title, step_color);
        Self::draw_footer(painter, card_rect, &t!("guide-offset-tip"));

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
            (t!("guide-mirror-step-1"), ACCENT_ORANGE)
        } else if phase < 0.65 {
            (t!("guide-mirror-step-2"), ACCENT_ORANGE)
        } else {
            (t!("guide-mirror-step-3"), ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, &t!("guide-mirror-header"), &step_title, step_color);
        Self::draw_footer(painter, card_rect, &t!("guide-mirror-tip"));

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

            let badge_pos = Pos2::new(card_rect.right() - 44.0, card_rect.center().y + 4.0);
            Self::draw_badge(
                painter,
                badge_pos,
                &t!("guide-mirror-symmetric"),
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
            (t!("guide-trim-step-1"), ACCENT_ORANGE)
        } else {
            (t!("guide-trim-step-2"), ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, &t!("guide-trim-header"), &step_title, step_color);
        Self::draw_footer(painter, card_rect, &t!("guide-trim-tip"));

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
                &t!("guide-trim-badge"),
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
            (t!("guide-coincident-step-1"), ACCENT_ORANGE)
        } else if phase < 0.70 {
            (t!("guide-coincident-step-2"), ACCENT_ORANGE)
        } else {
            (t!("guide-coincident-step-done"), ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, &t!("guide-coincident-header"), &step_title, step_color);
        Self::draw_footer(painter, card_rect, &t!("guide-coincident-tip"));

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
                &t!("guide-coincident-badge"),
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
            (t!("guide-fixed-step-1"), ACCENT_ORANGE)
        } else {
            (t!("guide-fixed-step-done"), ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, &t!("guide-fixed-header"), &step_title, step_color);
        Self::draw_footer(painter, card_rect, &t!("guide-fixed-tip"));

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
                &t!("guide-fixed-badge"),
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
            (t!("guide-symmetric-step-1"), ACCENT_ORANGE)
        } else if phase < 0.66 {
            (t!("guide-symmetric-step-2"), ACCENT_ORANGE)
        } else {
            (t!("guide-symmetric-step-done"), ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, &t!("guide-symmetric-header"), &step_title, step_color);
        Self::draw_footer(painter, card_rect, &t!("guide-symmetric-tip"));

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
            (Pos2::new(axis_x, card_rect.center().y + (1.0 - t) * 15.0), t > 0.85)
        } else {
            (p_right, false)
        };

        if phase >= 0.66 {
            painter.line_segment([p_left, p_right], Stroke::new(1.0, ACCENT_GREEN));
            let badge_pos = Pos2::new(card_rect.right() - 48.0, card_rect.center().y + 4.0);
            Self::draw_badge(
                painter,
                badge_pos,
                &t!("guide-symmetric-badge"),
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
            (t!("guide-extrude-step-1"), ACCENT_ORANGE)
        } else {
            (t!("guide-extrude-step-2"), ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, &t!("guide-extrude-header"), &step_title, step_color);
        Self::draw_footer(painter, card_rect, &t!("guide-extrude-tip"));

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
            (t!("guide-loft-step-1"), ACCENT_ORANGE)
        } else if phase < 0.66 {
            (t!("guide-loft-step-2"), ACCENT_BLUE)
        } else {
            (t!("guide-loft-step-done"), ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, &t!("guide-loft-header"), &step_title, step_color);
        Self::draw_footer(painter, card_rect, &t!("guide-loft-tip"));

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
                &t!("guide-loft-badge"),
                Color32::from_rgba_premultiplied(15, 80, 40, 220),
                Color32::WHITE,
            );
        }

        Self::draw_cursor(painter, cursor_pos, is_clicking, time);
    }

    /// Sweep Tool
    fn render_sweep_anim(
        painter: &egui::Painter,
        card_rect: Rect,
        time: f64,
    ) {
        let cycle = 4.8;
        let phase = ((time % cycle) / cycle) as f32;

        let (step_title, step_color) = if phase < 0.25 {
            (t!("guide-sweep-step-1"), ACCENT_BLUE)
        } else if phase < 0.50 {
            (t!("guide-sweep-step-2"), ACCENT_ORANGE)
        } else if phase < 0.75 {
            (t!("guide-sweep-step-3"), ACCENT_BLUE)
        } else {
            (t!("guide-sweep-step-done"), ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, &t!("guide-sweep-header"), &step_title, step_color);
        Self::draw_footer(painter, card_rect, &t!("guide-sweep-tip"));

        // 3D Isometric View Center
        let origin = Pos2::new(card_rect.left() + 65.0, card_rect.bottom() - 36.0);

        // 1. Draw 3D Isometric Reference Planes (XY horizontal & XZ vertical)
        let xy_p1 = Pos2::new(origin.x - 35.0, origin.y + 10.0);
        let xy_p2 = Pos2::new(origin.x + 20.0, origin.y + 10.0);
        let xy_p3 = Pos2::new(origin.x + 55.0, origin.y - 12.0);
        let xy_p4 = Pos2::new(origin.x, origin.y - 12.0);
        painter.line_segment([xy_p1, xy_p2], Stroke::new(0.8, TEXT_MUTED.gamma_multiply(0.35)));
        painter.line_segment([xy_p2, xy_p3], Stroke::new(0.8, TEXT_MUTED.gamma_multiply(0.35)));
        painter.line_segment([xy_p3, xy_p4], Stroke::new(0.8, TEXT_MUTED.gamma_multiply(0.35)));
        painter.line_segment([xy_p4, xy_p1], Stroke::new(0.8, TEXT_MUTED.gamma_multiply(0.35)));

        // Profile Center & Geometry (Ellipse on XY plane)
        let prof_center = Pos2::new(origin.x - 10.0, origin.y - 2.0);
        let profile_color = if phase < 0.25 {
            TEXT_PRIMARY
        } else if phase < 0.50 {
            ACCENT_ORANGE
        } else {
            ACCENT_GREEN
        };
        // Draw profile ellipse on horizontal plane
        let rx = 9.0;
        let ry = 4.5;
        let prof_pts: Vec<Pos2> = (0..=24)
            .map(|i| {
                let ang = (i as f32) * std::f32::consts::TAU / 24.0;
                Pos2::new(prof_center.x + ang.cos() * rx, prof_center.y + ang.sin() * ry)
            })
            .collect();
        for w in prof_pts.windows(2) {
            painter.line_segment([w[0], w[1]], Stroke::new(1.8, profile_color));
        }

        // Guide Path Curve (Spline rising into XZ vertical plane)
        let path_start = prof_center;
        let path_ctrl = Pos2::new(origin.x + 15.0, origin.y - 48.0);
        let path_end = Pos2::new(origin.x + 65.0, origin.y - 52.0);

        let segments = 24;
        let mut path_pts = Vec::new();
        for i in 0..=segments {
            let t = i as f32 / segments as f32;
            let x = (1.0 - t).powi(2) * path_start.x + 2.0 * (1.0 - t) * t * path_ctrl.x + t.powi(2) * path_end.x;
            let y = (1.0 - t).powi(2) * path_start.y + 2.0 * (1.0 - t) * t * path_ctrl.y + t.powi(2) * path_end.y;
            path_pts.push(Pos2::new(x, y));
        }

        let path_color = if phase < 0.50 {
            TEXT_MUTED
        } else if phase < 0.75 {
            ACCENT_BLUE
        } else {
            ACCENT_GREEN
        };
        for w in path_pts.windows(2) {
            painter.line_segment([w[0], w[1]], Stroke::new(1.8, path_color));
        }

        // 3D Swept Volume Animation (When Phase >= 0.75)
        if phase >= 0.75 {
            let sweep_progress = ((phase - 0.75) / 0.25).clamp(0.0, 1.0);
            let active_count = ((segments as f32 * sweep_progress).ceil() as usize).max(1);

            for i in 1..=active_count.min(path_pts.len() - 1) {
                let p_curr = path_pts[i];
                let p_prev = path_pts[i - 1];
                let tangent = (p_curr - p_prev).normalized();
                let normal = Vec2::new(-tangent.y, tangent.x) * 4.0;

                // Draw swept tube hull lines
                let top1 = p_prev + normal;
                let top2 = p_curr + normal;
                let bot1 = p_prev - normal;
                let bot2 = p_curr - normal;

                painter.line_segment([top1, top2], Stroke::new(1.4, ACCENT_GREEN));
                painter.line_segment([bot1, bot2], Stroke::new(1.4, ACCENT_GREEN));
                painter.line_segment([top2, bot2], Stroke::new(0.8, ACCENT_GREEN.gamma_multiply(0.4)));
            }

            let badge_pos = Pos2::new(card_rect.right() - 48.0, card_rect.center().y + 4.0);
            Self::draw_badge(
                painter,
                badge_pos,
                &t!("guide-sweep-badge"),
                Color32::from_rgba_premultiplied(15, 80, 40, 220),
                Color32::WHITE,
            );
        }

        // Bottom Menu mini pill (Step 2)
        if (0.25..0.50).contains(&phase) {
            let bottom_pill_rect = Rect::from_center_size(
                Pos2::new(card_rect.center().x + 25.0, card_rect.bottom() - 16.0),
                Vec2::new(76.0, 16.0),
            );
            painter.rect_filled(bottom_pill_rect, 8.0, Color32::from_rgba_premultiplied(20, 24, 32, 230));
            painter.rect_stroke(bottom_pill_rect, 8.0, Stroke::new(1.0, ACCENT_ORANGE), StrokeKind::Inside);
            painter.text(
                bottom_pill_rect.center(),
                Align2::CENTER_CENTER,
                "⚡ Sweep",
                FontId::proportional(9.5),
                ACCENT_ORANGE,
            );
        }

        // Top HUD mini pill (Step 4)
        if phase >= 0.75 {
            let top_hud_rect = Rect::from_center_size(
                Pos2::new(card_rect.center().x + 10.0, card_rect.top() + 32.0),
                Vec2::new(108.0, 15.0),
            );
            painter.rect_filled(top_hud_rect, 7.5, Color32::from_rgba_premultiplied(15, 60, 30, 230));
            painter.rect_stroke(top_hud_rect, 7.5, Stroke::new(1.0, ACCENT_GREEN), StrokeKind::Inside);
            painter.text(
                top_hud_rect.center(),
                Align2::CENTER_CENTER,
                "🚀 Buat Sweep 3D",
                FontId::proportional(9.0),
                Color32::WHITE,
            );
        }

        // Cursor Movement
        let (cursor_pos, is_clicking) = if phase < 0.25 {
            (Pos2::new(prof_center.x - 20.0, prof_center.y - 15.0), false)
        } else if phase < 0.50 {
            let t = ((phase - 0.25) / 0.25).clamp(0.0, 1.0);
            if t < 0.5 {
                (prof_center, t > 0.35)
            } else {
                let pill_target = Pos2::new(card_rect.center().x + 25.0, card_rect.bottom() - 16.0);
                (pill_target, t > 0.85)
            }
        } else if phase < 0.75 {
            let t = ((phase - 0.50) / 0.25).clamp(0.0, 1.0);
            (Pos2::new(path_ctrl.x + (1.0 - t) * 12.0, path_ctrl.y), t > 0.75)
        } else {
            let top_hud_target = Pos2::new(card_rect.center().x + 30.0, card_rect.top() + 32.0);
            (top_hud_target, false)
        };

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
            (t!("guide-shell-step-1"), ACCENT_ORANGE)
        } else {
            (t!("guide-shell-step-2"), ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, &t!("guide-shell-header"), &step_title, step_color);
        Self::draw_footer(painter, card_rect, &t!("guide-shell-tip"));

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
        let cycle = 4.2;
        let phase = ((time % cycle) / cycle) as f32;

        let (step_title, step_color, op_name) = if phase < 0.33 {
            (t!("boolean-union-desc"), ACCENT_BLUE, t!("boolean-union-badge"))
        } else if phase < 0.66 {
            (t!("boolean-subtract-desc"), ACCENT_ORANGE, t!("boolean-subtract-badge"))
        } else {
            (t!("boolean-intersect-desc"), ACCENT_GREEN, t!("boolean-intersect-badge"))
        };

        Self::draw_header(painter, card_rect, &t!("guide-boolean-header"), &step_title, step_color);
        Self::draw_footer(painter, card_rect, &t!("guide-boolean-tip"));

        let c1 = Pos2::new(card_rect.left() + 65.0, card_rect.center().y + 4.0);
        let c2 = Pos2::new(card_rect.left() + 85.0, card_rect.center().y + 4.0);

        let r1 = Rect::from_center_size(c1, Vec2::new(36.0, 36.0));
        let r2 = Rect::from_center_size(c2, Vec2::new(36.0, 36.0));

        if phase < 0.33 {
            // Union: kedua kubus digabung utuh
            let union_r = r1.union(r2);
            painter.rect_filled(union_r, 3.0, ACCENT_BLUE.gamma_multiply(0.35));
            painter.rect_stroke(union_r, 3.0, Stroke::new(1.4, ACCENT_BLUE), StrokeKind::Inside);

            // Garis batas antar bodi
            painter.rect_stroke(r1, 3.0, Stroke::new(1.0, ACCENT_BLUE.gamma_multiply(0.5)), StrokeKind::Inside);
            painter.rect_stroke(r2, 3.0, Stroke::new(1.0, ACCENT_BLUE.gamma_multiply(0.5)), StrokeKind::Inside);
        } else if phase < 0.66 {
            // Subtract: bodi 1 dipotong bodi 2
            painter.rect_filled(r1, 3.0, ACCENT_ORANGE.gamma_multiply(0.35));
            painter.rect_stroke(r1, 3.0, Stroke::new(1.4, ACCENT_ORANGE), StrokeKind::Inside);

            // Bodi 2 digambar putus-putus sebagai pemotong
            painter.rect_filled(r2, 3.0, Color32::from_rgba_premultiplied(40, 44, 52, 100));
            painter.rect_stroke(r2, 3.0, Stroke::new(1.0, TEXT_MUTED), StrokeKind::Inside);
        } else {
            // Intersect: hanya bagian irisan yang dipertahankan
            let intersect_r = r1.intersect(r2);
            painter.rect_filled(intersect_r, 2.0, ACCENT_GREEN.gamma_multiply(0.50));
            painter.rect_stroke(intersect_r, 2.0, Stroke::new(1.5, ACCENT_GREEN), StrokeKind::Inside);

            // Garis luar kedua bodi samar
            painter.rect_stroke(r1, 3.0, Stroke::new(1.0, TEXT_MUTED.gamma_multiply(0.6)), StrokeKind::Inside);
            painter.rect_stroke(r2, 3.0, Stroke::new(1.0, TEXT_MUTED.gamma_multiply(0.6)), StrokeKind::Inside);
        }

        let badge_pos = Pos2::new(card_rect.right() - 48.0, card_rect.center().y + 4.0);
        Self::draw_badge(
            painter,
            badge_pos,
            &op_name,
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
            (t!("guide-section-step-1"), ACCENT_ORANGE)
        } else {
            (t!("guide-section-step-2"), ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, &t!("guide-section-header"), &step_title, step_color);
        Self::draw_footer(painter, card_rect, &t!("guide-section-tip"));

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
            &t!("guide-section-badge"),
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
            (t!("guide-measure-step-2-active"), ACCENT_GREEN)
        } else if phase < 0.40 {
            (t!("guide-measure-step-1"), ACCENT_ORANGE)
        } else {
            (t!("guide-measure-step-2"), ACCENT_ORANGE)
        };

        Self::draw_header(painter, card_rect, &t!("guide-measure-header"), &step_title, step_color);
        Self::draw_footer(painter, card_rect, &t!("guide-measure-tip"));

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
            (t!("guide-measure-angle-step-1"), ACCENT_ORANGE)
        } else {
            (t!("guide-measure-angle-step-2"), ACCENT_GREEN)
        };

        Self::draw_header(painter, card_rect, &t!("guide-measure-angle-header"), &step_title, step_color);
        Self::draw_footer(painter, card_rect, &t!("guide-measure-angle-tip"));

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
