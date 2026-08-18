//! In-Canvas HUD & Floating Dimension Pills bergaya Shapr3D dengan Material Icons.
//!
//! Menampilkan widget HUD mengambang langsung di atas kanvas 3D:
//! tombol kapsul "Normal to Sketch", banner peringatan Section View,
//! badge dimensi in-situ, dan status seleksi mengambang di bawah tengah kanvas.

use egui::{Color32, FontId, Pos2, RichText, Stroke, StrokeKind, Ui, Vec2};
use egui_material_icons::icons::{ICON_3D_ROTATION, ICON_LOCK, ICON_STRAIGHTEN};
use crate::theme::{pill_frame, ACCENT_BLUE, ACCENT_ORANGE, TEXT_PRIMARY, TEXT_SECONDARY};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasHudEvent {
    OrientNormalToSketch,
    TurnOffSectionView,
    OpenMeasurements,
}

/// Titik-titik poligon konveks rounded-rect berpusat di origin (belum
/// diputar/digeser) — dipakai `render_dimension_pill_aligned` karena
/// `Painter::rect_filled` (rounded-rect bawaan egui) tidak punya varian
/// berotasi. `segments_per_corner` mengatur kehalusan lengkungan tiap sudut
/// (6 sudah cukup mulus untuk ukuran pill sekecil ini).
fn rounded_rect_local_points(half: Vec2, radius: f32, segments_per_corner: usize) -> Vec<Pos2> {
    let r = radius.min(half.x).min(half.y).max(0.0);
    let centers_and_angles = [
        (Vec2::new(-half.x + r, -half.y + r), std::f32::consts::PI, 1.5 * std::f32::consts::PI), // kiri-atas
        (Vec2::new(half.x - r, -half.y + r), 1.5 * std::f32::consts::PI, 2.0 * std::f32::consts::PI), // kanan-atas
        (Vec2::new(half.x - r, half.y - r), 0.0, 0.5 * std::f32::consts::PI), // kanan-bawah
        (Vec2::new(-half.x + r, half.y - r), 0.5 * std::f32::consts::PI, std::f32::consts::PI), // kiri-bawah
    ];

    let mut points = Vec::with_capacity((segments_per_corner + 1) * 4);
    for (center, start_angle, end_angle) in centers_and_angles {
        for i in 0..=segments_per_corner {
            let t = i as f32 / segments_per_corner as f32;
            let angle = start_angle + (end_angle - start_angle) * t;
            let p = center + Vec2::new(angle.cos(), angle.sin()) * r;
            points.push(Pos2::new(p.x, p.y));
        }
    }
    points
}

pub struct CanvasHud;

impl CanvasHud {
    /// Render tombol kapsul mengambang "Normal to Sketch" di dalam container UI yang diberikan.
    pub fn show_normal_to_sketch_btn(ui: &mut Ui) -> Option<CanvasHudEvent> {
        let mut event = None;
        pill_frame().show(ui, |ui| {
            let btn = ui.button(
                RichText::new(format!("{} Normal to Sketch", ICON_3D_ROTATION.codepoint))
                    .size(12.0)
                    .strong()
                    .color(TEXT_PRIMARY),
            );
            if btn.clicked() {
                event = Some(CanvasHudEvent::OrientNormalToSketch);
            }
        });
        event
    }

    /// Render banner informasi Section View di dalam container UI yang diberikan.
    pub fn show_section_view_banner(ui: &mut Ui) -> Option<CanvasHudEvent> {
        let mut event = None;
        pill_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Turn off Section View to show hidden parts")
                        .size(11.0)
                        .color(ACCENT_ORANGE),
                );
                if ui.small_button(RichText::new("Turn off").size(10.0)).clicked() {
                    event = Some(CanvasHudEvent::TurnOffSectionView);
                }
            });
        });
        event
    }

    /// Render status badge seleksi & pengukuran mengambang di dalam container UI yang diberikan.
    pub fn show_bottom_status_pill(
        ui: &mut Ui,
        selection_summary: &str,
        measurement_summary: Option<&str>,
    ) -> Option<CanvasHudEvent> {
        let mut event = None;
        pill_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                // Ringkasan seleksi
                ui.label(
                    RichText::new(selection_summary)
                        .size(11.0)
                        .strong()
                        .color(ACCENT_BLUE),
                );

                // Ringkasan pengukuran jika ada
                if let Some(m) = measurement_summary {
                    ui.label(RichText::new("|").color(TEXT_SECONDARY));
                    let resp = ui.selectable_label(
                        false,
                        RichText::new(format!("{} {}", ICON_STRAIGHTEN.codepoint, m))
                            .size(11.0)
                            .color(TEXT_PRIMARY),
                    );
                    if resp.clicked() {
                        event = Some(CanvasHudEvent::OpenMeasurements);
                    }
                }
            });
        });
        event
    }

    /// Render badge dimensi mengambang putih langsung pada posisi 2D di kanvas.
    pub fn render_dimension_pill(
        ui: &mut Ui,
        pos_2d: Pos2,
        value_text: &str,
        locked: bool,
    ) {
        let painter = ui.painter();
        let lock_icon = if locked { format!("{} ", ICON_LOCK.codepoint) } else { "".to_string() };
        let full_text = format!("{}{}", lock_icon, value_text);

        let font = FontId::proportional(11.0);
        let galley = painter.layout_no_wrap(full_text, font, Color32::from_rgb(20, 20, 22));
        let rect = egui::Rect::from_center_size(
            pos_2d,
            galley.size() + Vec2::new(16.0, 8.0),
        );

        // Gambar latar belakang pill putih/terang dengan bayangan
        painter.rect_filled(rect, 10.0, Color32::from_rgba_premultiplied(245, 246, 250, 245));
        painter.rect_stroke(rect, 10.0, Stroke::new(1.0, Color32::from_gray(180)), StrokeKind::Inside);
        painter.galley(rect.min + Vec2::new(8.0, 4.0), galley, Color32::from_rgb(20, 20, 22));
    }

    /// Render badge dimensi mengambang yang DIPUTAR sejajar arah garis pengukuran
    /// (bukan selalu horizontal seperti `render_dimension_pill`) — dipakai label
    /// nominal jarak/sudut supaya menempel rapi di garisnya sendiri dan tidak
    /// numpuk visual dengan garis pengukuran lain yang miring. `angle_rad` harus
    /// SUDAH dinormalisasi pemanggil ke rentang -90°..90° (mis. lewat
    /// `Vec2::angle()` lalu di-`±π` kalau di luar rentang itu) supaya teksnya
    /// tidak pernah kebalik/terbaca dari bawah ke atas.
    pub fn render_dimension_pill_aligned(ui: &mut Ui, center_2d: Pos2, angle_rad: f32, value_text: &str) {
        let painter = ui.painter();
        let font = FontId::proportional(11.0);
        let galley = painter.layout_no_wrap(value_text.to_string(), font, Color32::from_rgb(20, 20, 22));
        let half = (galley.size() + Vec2::new(16.0, 8.0)) * 0.5;
        let rot = egui::emath::Rot2::from_angle(angle_rad);

        // Sudut membulat (radius 10, sama seperti `render_dimension_pill` yang
        // tidak diputar) diaproksimasi jadi polygon konveks — rounded-rect
        // bawaan egui (`rect_filled`) tidak punya varian berotasi — lalu semua
        // titiknya diputar & digeser ke `center_2d`.
        let corners: Vec<Pos2> = rounded_rect_local_points(half, 10.0, 6)
            .into_iter()
            .map(|c| center_2d + rot * c.to_vec2())
            .collect();

        // Agak transparan (dari solid 245 -> 210) supaya garis kuning di
        // baliknya tetap samar kelihatan, bukan ketutup penuh sama pill-nya.
        painter.add(egui::epaint::PathShape::convex_polygon(
            corners,
            Color32::from_rgba_premultiplied(245, 246, 250, 210),
            Stroke::new(1.0, Color32::from_rgba_premultiplied(170, 170, 175, 210)),
        ));

        // `TextShape::pos` adalah pojok kiri-atas galley SEBELUM rotasi, dengan
        // pivot rotasi di titik itu juga — jadi dihitung mundur dari pusat
        // supaya, setelah diputar `rot`, teksnya jatuh center di `center_2d`.
        let text_pos = center_2d + rot * (Vec2::new(-half.x, -half.y) + Vec2::new(8.0, 4.0));
        let mut text_shape = egui::epaint::TextShape::new(text_pos, galley, Color32::from_rgb(20, 20, 22));
        text_shape.angle = angle_rad;
        painter.add(text_shape);
    }

    /// Render badge dimensi interaktif putih (dengan border biru bila aktif / hover)
    /// yang dapat diklik untuk memasukkan angka presisi.
    pub fn render_interactive_dimension_pill(
        ui: &mut Ui,
        pos_2d: Pos2,
        value_text: &str,
        is_active: bool,
    ) -> egui::Response {
        let font = FontId::proportional(11.5);
        let galley = ui.painter().layout_no_wrap(value_text.to_string(), font, Color32::from_rgb(20, 20, 25));
        let size = galley.size() + Vec2::new(18.0, 10.0);
        let rect = egui::Rect::from_center_size(pos_2d, size);

        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        let is_hovered = response.hovered();

        // Background & border
        let bg_color = if is_active {
            Color32::from_rgba_premultiplied(235, 245, 255, 250)
        } else {
            Color32::from_rgba_premultiplied(250, 250, 252, 245)
        };
        let border_color = if is_active || is_hovered {
            ACCENT_BLUE
        } else {
            Color32::from_gray(185)
        };
        let stroke_width = if is_active { 1.8 } else { 1.0 };

        let painter = ui.painter();
        painter.rect_filled(rect, 8.0, bg_color);
        painter.rect_stroke(rect, 8.0, Stroke::new(stroke_width, border_color), StrokeKind::Inside);
        painter.galley(rect.min + Vec2::new(9.0, 5.0), galley, Color32::from_rgb(20, 20, 25));

        response
    }

    /// Render badge dimensi interaktif yang DIPUTAR sejajar arah garis pengukuran
    /// (dengan border biru bila aktif / hover) yang dapat diklik untuk memasukkan angka presisi.
    pub fn render_interactive_dimension_pill_aligned(
        ui: &mut Ui,
        center_2d: Pos2,
        angle_rad: f32,
        value_text: &str,
        is_active: bool,
    ) -> egui::Response {
        let font = FontId::proportional(11.5);
        let galley = ui.painter().layout_no_wrap(value_text.to_string(), font, Color32::from_rgb(20, 20, 25));
        let half = (galley.size() + Vec2::new(18.0, 10.0)) * 0.5;
        let size = half * 2.0;
        let max_dim = size.x.max(size.y);
        let rect = egui::Rect::from_center_size(center_2d, Vec2::splat(max_dim));

        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        let is_hovered = response.hovered();

        let bg_color = if is_active {
            Color32::from_rgba_premultiplied(235, 245, 255, 250)
        } else if is_hovered {
            Color32::from_rgba_premultiplied(255, 255, 255, 250)
        } else {
            Color32::from_rgba_premultiplied(245, 246, 250, 220)
        };
        let border_color = if is_active || is_hovered {
            ACCENT_BLUE
        } else {
            Color32::from_rgba_premultiplied(170, 170, 175, 220)
        };
        let stroke_width = if is_active || is_hovered { 1.8 } else { 1.0 };

        let rot = egui::emath::Rot2::from_angle(angle_rad);
        let corners: Vec<Pos2> = rounded_rect_local_points(half, 8.0, 6)
            .into_iter()
            .map(|c| center_2d + rot * c.to_vec2())
            .collect();

        let painter = ui.painter();
        painter.add(egui::epaint::PathShape::convex_polygon(
            corners,
            bg_color,
            Stroke::new(stroke_width, border_color),
        ));

        let text_pos = center_2d + rot * (Vec2::new(-half.x, -half.y) + Vec2::new(9.0, 5.0));
        let mut text_shape = egui::epaint::TextShape::new(text_pos, galley, Color32::from_rgb(20, 20, 25));
        text_shape.angle = angle_rad;
        painter.add(text_shape);

        response
    }

    /// Render badge tombol "Copy" floating (gaya Shapr3D) di bawah Transform Gizmo.
    pub fn render_copy_toggle_badge(
        ui: &mut Ui,
        pos_2d: Pos2,
        is_copy_active: bool,
    ) -> egui::Response {
        let font = FontId::proportional(12.0);
        let text = "Copy";
        let galley = ui.painter().layout_no_wrap(text.to_string(), font.clone(), Color32::from_rgb(20, 20, 25));
        let size = galley.size() + Vec2::new(20.0, 10.0);
        let rect = egui::Rect::from_center_size(pos_2d, size);

        let response = ui.allocate_rect(rect, egui::Sense::click());
        let is_hovered = response.hovered();

        let bg_color = if is_copy_active {
            Color32::from_rgb(40, 130, 250)
        } else if is_hovered {
            Color32::from_rgba_premultiplied(240, 245, 255, 250)
        } else {
            Color32::from_rgba_premultiplied(255, 255, 255, 245)
        };
        let text_color = if is_copy_active {
            Color32::WHITE
        } else {
            Color32::from_rgb(20, 20, 25)
        };
        let border_color = if is_copy_active {
            ACCENT_BLUE
        } else if is_hovered {
            ACCENT_BLUE
        } else {
            Color32::from_gray(180)
        };

        let painter = ui.painter();
        painter.rect_filled(rect, 8.0, bg_color);
        painter.rect_stroke(rect, 8.0, Stroke::new(1.2, border_color), StrokeKind::Inside);
        let text_galley = painter.layout_no_wrap(text.to_string(), font, text_color);
        painter.galley(rect.min + Vec2::new(10.0, 5.0), text_galley, text_color);

        if is_hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        response
    }

    /// Render badge sudut interaktif (mis. "45.0°") yang dapat diklik untuk mengetik angka sudut rotasi.
    pub fn render_interactive_angle_pill(
        ui: &mut Ui,
        pos_2d: Pos2,
        value_text: &str,
        is_active: bool,
    ) -> egui::Response {
        let font = FontId::proportional(11.5);
        let galley = ui.painter().layout_no_wrap(value_text.to_string(), font, Color32::from_rgb(20, 20, 25));
        let size = galley.size() + Vec2::new(18.0, 10.0);
        let rect = egui::Rect::from_center_size(pos_2d, size);

        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        let is_hovered = response.hovered();

        let bg_color = if is_active {
            Color32::from_rgba_premultiplied(235, 245, 255, 250)
        } else {
            Color32::from_rgba_premultiplied(250, 250, 252, 245)
        };
        let border_color = if is_active || is_hovered {
            ACCENT_BLUE
        } else {
            Color32::from_gray(185)
        };
        let stroke_width = if is_active { 1.8 } else { 1.0 };

        let painter = ui.painter();
        painter.rect_filled(rect, 8.0, bg_color);
        painter.rect_stroke(rect, 8.0, Stroke::new(stroke_width, border_color), StrokeKind::Inside);
        painter.galley(rect.min + Vec2::new(9.0, 5.0), galley, Color32::from_rgb(20, 20, 25));

        response
    }

    /// Area sense-drag utk gizmo panah dua-sisi push/pull/extrude/rounding
    /// (Fase 9 — Icon Gizmo Profesional). Dulu fungsi ini SEKALIGUS
    /// menggambar badge lingkaran biru flat + icon panah `▲-▼` di atasnya
    /// — badge itu sekarang DIHAPUS (terasa seperti tombol UI 2D yang
    /// nempel aneh di tengah scene 3D, dobel dgn icon panah kerucut solid
    /// yang juga digambar di scene). Icon visualnya sekarang TUNGGAL: mesh
    /// solid ter-shading dari `sketch_render::solid_double_arrow_gizmo_mesh`
    /// (lihat `CadrawApp::build_gizmo_mesh`), digambar sungguhan di scene
    /// 3D lewat `SceneRenderer::set_gizmo_mesh`. Fungsi ini HANYA
    /// menyediakan hit-area drag (posisinya PERSIS di titik yg sama dgn
    /// pusat mesh solid itu) + ubah cursor icon sesuai arah proyeksi 3D —
    /// area sense-nya sengaja dibiarkan sama besarnya dgn handle lama
    /// (bukan cuma seukuran mesh solid yg lebih kecil) supaya tetap gampang
    /// di-grab.
    pub fn render_draggable_double_arrow_handle(
        ui: &mut Ui,
        pos_2d: Pos2,
        is_dragging: bool,
        dir_2d: Option<Vec2>,
    ) -> egui::Response {
        let handle_radius = if is_dragging { 18.0 } else { 16.0 };
        let rect = egui::Rect::from_center_size(pos_2d, Vec2::splat(handle_radius * 2.0 + 8.0));
        let response = ui.allocate_rect(rect, egui::Sense::drag());
        let is_hovered = response.hovered();

        let dir_u = match dir_2d {
            Some(d) if d.length() > 1e-4 => d.normalized(),
            _ => Vec2::new(0.0, -1.0),
        };

        if is_hovered || is_dragging {
            let cursor = if dir_u.x.abs() > dir_u.y.abs() * 2.0 {
                egui::CursorIcon::ResizeHorizontal
            } else if dir_u.y.abs() > dir_u.x.abs() * 2.0 {
                egui::CursorIcon::ResizeVertical
            } else if dir_u.x * dir_u.y < 0.0 {
                egui::CursorIcon::ResizeNeSw
            } else {
                egui::CursorIcon::ResizeNwSe
            };
            ui.ctx().set_cursor_icon(cursor);
        }

        response
    }

    /// Crosshair "+" draggable — dipakai gizmo geser sketch OMNIDIRECTIONAL
    /// (beda dari `render_draggable_double_arrow_handle` yang menyiratkan 1
    /// sumbu): drag bebas ke segala arah dalam bidang sketsa sekaligus (u
    /// DAN v, bukan satu-satu). Titik "+" ini SEKALIGUS jadi acuan visual
    /// "titik tengah" objek 2D — makanya sengaja digambar MENYATU dengan
    /// gaya garis sketch (biru selaras `COLOR_SELECTED` di
    /// `cadraw-render/sketch.rs`, tanpa badge lingkaran solid warna
    /// mencolok saat idle) alih-alih ikon tombol UI yang berdiri sendiri.
    /// Interaksinya dua jalur (lihat pemanggil di `dynamic_input_ui`):
    /// (1) klik-drag langsung menggeser bebas, dgn snap ke titik entitas
    /// LAIN atau ke titik tengah region tertutup LAIN — cara menyatukan
    /// pusat 2 sketch/profil; (2) klik SINGKAT (tanpa gerak) meng-arm
    /// "mode geser" (`is_armed`) supaya bisa lanjut digeser pakai tombol
    /// panah keyboard tanpa pegang mouse — makanya `Sense::click_and_drag`
    /// (bukan `drag()` saja) supaya `Response::clicked()` kebaca terpisah
    /// dari `dragged()`.
    pub fn render_draggable_move_handle(
        ui: &mut Ui,
        pos_2d: Pos2,
        is_dragging: bool,
        is_armed: bool,
    ) -> egui::Response {
        let active = is_dragging || is_armed;
        let handle_radius = if active { 9.0 } else { 7.0 };
        // Hit-area tetap lega (lebih besar dari radius visual) supaya tetap
        // gampang di-grab walau tampilan idle-nya sengaja dikecilkan/dibuat
        // halus biar blend dengan sketch.
        let rect = egui::Rect::from_center_size(pos_2d, Vec2::splat(handle_radius * 2.0 + 20.0));
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        let is_hovered = response.hovered();

        if is_hovered || active {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Move);
        }

        let painter = ui.painter();
        // Biru selaras warna entitas terpilih (`COLOR_SELECTED` di
        // cadraw-render/sketch.rs) — bikin "+" terasa bagian dari sketch,
        // bukan elemen UI terpisah.
        let blend_color = Color32::from_rgb(77, 166, 255);

        if is_hovered || is_dragging {
            painter.circle_filled(pos_2d, handle_radius + 6.0, blend_color.gamma_multiply(0.22));
        } else if is_armed {
            // Denyut visual halus (cincin putus-putus) menandai "mode
            // geser" aktif menunggu tombol panah — beda dari sekadar hover.
            painter.circle_stroke(pos_2d, handle_radius + 6.0, Stroke::new(1.2, blend_color.gamma_multiply(0.8)));
        }

        // Titik kecil di pusat (representasi "titik tengah objek").
        let dot_radius = if active { 3.0 } else { 2.0 };
        painter.circle_filled(pos_2d, dot_radius, blend_color);
        if active {
            painter.circle_stroke(pos_2d, dot_radius, Stroke::new(1.0, Color32::WHITE));
        }

        // Crosshair "+" tipis, dua garis tegak lurus lewat pusat — TIDAK
        // ada badge lingkaran solid di baliknya (beda dari versi lama)
        // supaya menyatu dgn garis sketch, bukan menutupinya.
        let arm = handle_radius + (if active { 6.0 } else { 4.0 });
        let gap = dot_radius + 1.5;
        let stroke_w = if active { 2.0 } else { 1.4 };
        let stroke = Stroke::new(stroke_w, blend_color);
        painter.line_segment([pos_2d - Vec2::new(arm, 0.0), pos_2d - Vec2::new(gap, 0.0)], stroke);
        painter.line_segment([pos_2d + Vec2::new(gap, 0.0), pos_2d + Vec2::new(arm, 0.0)], stroke);
        painter.line_segment([pos_2d - Vec2::new(0.0, arm), pos_2d - Vec2::new(0.0, gap)], stroke);
        painter.line_segment([pos_2d + Vec2::new(0.0, gap), pos_2d + Vec2::new(0.0, arm)], stroke);

        response
    }
}
