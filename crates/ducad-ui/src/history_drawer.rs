//! History Drawer — Panel Riwayat Kegiatan 2D & 3D bergaya Shapr3D.
//!
//! Menampilkan daftar kegiatan kronologis (aktivitas 2D vs 3D) dalam bentuk
//! list kartu padat/kompak, badge warna pembeda, badge lingkaran sempurna,
//! tombol hapus ikon bersih, dan kemampuan jump (undo/redo) saat item diklik.

use egui::{
    Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, ScrollArea,
    Stroke, Ui, Vec2,
};
use egui_material_icons::icons::{
    ICON_CATEGORY, ICON_CLEAR, ICON_CLOSE, ICON_DELETE, ICON_HISTORY,
    ICON_HORIZONTAL_RULE, ICON_SEARCH,
};
use crate::theme::{
    card_frame, glass_frame, ACCENT_BLUE,
    TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityKindUi {
    Sketch2D,
    Solid3D,
}

#[derive(Debug, Clone)]
pub struct ActivityItemInfo {
    pub id: i64,
    pub timestamp: String,
    pub kind: ActivityKindUi,
    pub action: String,
    pub details: String,
}

#[derive(Debug, Clone)]
pub enum HistoryDrawerEvent {
    Close,
    ClearAll,
    JumpTo {
        id: i64,
        timestamp: String,
        action: String,
    },
}

pub struct HistoryDrawer {
    pub search_query: String,
    pub custom_height: Option<f32>,
}

impl Default for HistoryDrawer {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            custom_height: None,
        }
    }
}

impl HistoryDrawer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Render panel history aktivitas di pojok kanan bawah kanvas.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        activities: &[ActivityItemInfo],
        max_height: f32,
        anchor_bottom_y: f32,
    ) -> Option<HistoryDrawerEvent> {
        let mut event = None;

        glass_frame().show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_width(280.0);
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);

                // =========================================================================
                // 0. TOP RESIZE HANDLE (Tarik ke atas / bawah untuk mengubah tinggi panel)
                // =========================================================================
                let (handle_rect, handle_resp) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 10.0),
                    egui::Sense::click_and_drag(),
                );
                if handle_resp.hovered() || handle_resp.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                }
                let pill_rect = egui::Rect::from_center_size(
                    handle_rect.center(),
                    Vec2::new(36.0, 4.0),
                );
                let pill_color = if handle_resp.dragged() {
                    ACCENT_BLUE
                } else if handle_resp.hovered() {
                    TEXT_SECONDARY
                } else {
                    Color32::from_rgb(70, 75, 90)
                };
                ui.painter().rect_filled(pill_rect, CornerRadius::same(2), pill_color);

                if handle_resp.dragged() {
                    if let Some(ptr_pos) = ui.input(|i| i.pointer.hover_pos()) {
                        let desired_h = anchor_bottom_y - ptr_pos.y;
                        self.custom_height = Some(desired_h.clamp(120.0, max_height));
                    }
                }

                // =========================================================================
                // 1. SEARCH BAR, HEADER & CLOSE BUTTON
                // =========================================================================
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(ICON_SEARCH.codepoint)
                            .size(13.0)
                            .color(TEXT_SECONDARY),
                    );
                    let has_query = !self.search_query.is_empty();
                    let clear_btn_w = if has_query { 22.0 } else { 0.0 };
                    let close_btn_w = 26.0;
                    let text_width = (ui.available_width() - clear_btn_w - close_btn_w).max(80.0);

                    ui.add(
                        egui::TextEdit::singleline(&mut self.search_query)
                            .hint_text("Cari riwayat aktivitas…")
                            .desired_width(text_width),
                    );

                    if has_query {
                        if ui
                            .small_button(
                                RichText::new(ICON_CLEAR.codepoint)
                                    .size(11.0)
                                    .color(TEXT_SECONDARY),
                            )
                            .on_hover_text("Hapus pencarian")
                            .clicked()
                        {
                            self.search_query.clear();
                        }
                    }

                    let close_btn = ui
                        .small_button(
                            RichText::new(ICON_CLOSE.codepoint)
                                .size(12.0)
                                .color(TEXT_SECONDARY),
                        )
                        .on_hover_text("Tutup Riwayat");
                    if close_btn.clicked() {
                        event = Some(HistoryDrawerEvent::Close);
                    }
                });

                // Baris info ringkas & tombol clear (ikon bersih + badge lingkaran sempurna)
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(ICON_HISTORY.codepoint)
                            .size(12.0)
                            .color(ACCENT_BLUE),
                    );
                    ui.label(
                        RichText::new("RIWAYAT AKTIVITAS")
                            .size(10.5)
                            .strong()
                            .color(TEXT_PRIMARY),
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // Tombol Hapus (cukup icon murni dengan hover merah)
                        if !activities.is_empty() {
                            let (del_rect, del_resp) = ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::click());
                            let del_hovered = del_resp.hovered();
                            let del_color = if del_hovered {
                                Color32::from_rgb(255, 69, 58)
                            } else {
                                TEXT_MUTED
                            };
                            ui.painter().text(
                                del_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                ICON_DELETE.codepoint,
                                egui::FontId::proportional(12.5),
                                del_color,
                            );
                            if del_resp.on_hover_text("Kosongkan Riwayat").clicked() {
                                event = Some(HistoryDrawerEvent::ClearAll);
                            }
                        }

                        // Badge counter (lingkaran penuh sempurna radius 9.0)
                        let (badge_rect, _) = ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::hover());
                        ui.painter().circle_filled(
                            badge_rect.center(),
                            9.0,
                            Color32::from_rgb(46, 50, 62),
                        );
                        ui.painter().text(
                            badge_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("{}", activities.len()),
                            egui::FontId::proportional(10.0),
                            Color32::from_rgb(160, 166, 178),
                        );
                    });
                });

                ui.separator();

                let query = self.search_query.to_lowercase().trim().to_string();

                // =========================================================================
                // 2. SCROLLABLE ACTIVITY LIST (Padat & Kompak)
                // =========================================================================
                let (scroll_h, auto_shrink_v) = if let Some(ch) = self.custom_height {
                    ((ch - 60.0).clamp(60.0, max_height - 60.0), false)
                } else {
                    (max_height, true)
                };

                let mut scroll_area = ScrollArea::vertical()
                    .auto_shrink([false, auto_shrink_v])
                    .max_height(scroll_h);

                if !auto_shrink_v {
                    scroll_area = scroll_area.min_scrolled_height(scroll_h);
                }

                scroll_area.show(ui, |ui| {
                    if !auto_shrink_v {
                        ui.set_min_height(scroll_h);
                    }
                    ui.spacing_mut().item_spacing = Vec2::new(0.0, 3.0);

                        let filtered: Vec<&ActivityItemInfo> = activities
                            .iter()
                            .filter(|a| {
                                query.is_empty()
                                    || a.action.to_lowercase().contains(&query)
                                    || a.details.to_lowercase().contains(&query)
                                    || a.timestamp.contains(&query)
                            })
                            .collect();

                        if filtered.is_empty() {
                            card_frame().show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.vertical_centered(|ui| {
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new(ICON_HISTORY.codepoint)
                                            .size(20.0)
                                            .color(TEXT_MUTED),
                                    );
                                    ui.label(
                                        RichText::new(if query.is_empty() {
                                            "Belum ada catatan aktivitas"
                                        } else {
                                            "Tidak ditemukan hasil pencarian"
                                        })
                                        .size(10.5)
                                        .strong()
                                        .color(TEXT_SECONDARY),
                                    );
                                    ui.label(
                                        RichText::new("Aktivitas 2D & 3D akan tercatat otomatis")
                                            .size(9.0)
                                            .color(TEXT_MUTED),
                                    );
                                    ui.add_space(8.0);
                                });
                            });
                        } else {
                            for item in filtered {
                                let (badge_color, badge_text, icon_str, border_color) = match item.kind {
                                    ActivityKindUi::Sketch2D => (
                                        Color32::from_rgb(0, 210, 180),
                                        "2D",
                                        ICON_HORIZONTAL_RULE.codepoint,
                                        Color32::from_rgba_premultiplied(0, 210, 180, 40),
                                    ),
                                    ActivityKindUi::Solid3D => (
                                        Color32::from_rgb(191, 90, 242),
                                        "3D",
                                        ICON_CATEGORY.codepoint,
                                        Color32::from_rgba_premultiplied(191, 90, 242, 40),
                                    ),
                                };

                                let row_frame = Frame {
                                    inner_margin: Margin::symmetric(6, 4),
                                    outer_margin: Margin::symmetric(0, 0),
                                    corner_radius: CornerRadius::same(5),
                                    shadow: egui::Shadow::NONE,
                                    fill: Color32::from_rgb(24, 27, 34),
                                    stroke: Stroke::new(0.5, border_color),
                                };

                                let card_resp = row_frame.show(ui, |ui| {
                                    ui.set_width(ui.available_width());

                                    // Baris atas: Icon + Aksi + Badge 2D/3D
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);

                                        ui.label(
                                            RichText::new(icon_str)
                                                .size(11.0)
                                                .color(badge_color),
                                        );

                                        ui.label(
                                            RichText::new(&item.action)
                                                .strong()
                                                .size(10.5)
                                                .color(TEXT_PRIMARY),
                                        );

                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            // Badge 2D / 3D kompak
                                            Frame {
                                                inner_margin: Margin::symmetric(4, 0),
                                                outer_margin: Margin::ZERO,
                                                corner_radius: CornerRadius::same(3),
                                                shadow: egui::Shadow::NONE,
                                                fill: badge_color.linear_multiply(0.18),
                                                stroke: Stroke::new(0.5, badge_color),
                                            }
                                            .show(ui, |ui| {
                                                ui.label(
                                                    RichText::new(badge_text)
                                                        .size(8.0)
                                                        .strong()
                                                        .color(badge_color),
                                                );
                                            });
                                        });
                                    });

                                    // Baris bawah (kompak): Detail deskripsi informatif + Timestamp
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);

                                        if !item.details.is_empty() {
                                            ui.add(
                                                egui::Label::new(
                                                    RichText::new(&item.details)
                                                        .size(9.0)
                                                        .color(TEXT_SECONDARY),
                                                )
                                                .truncate(),
                                            )
                                            .on_hover_text(&item.details);
                                        }

                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            ui.label(
                                                RichText::new(&item.timestamp)
                                                    .size(8.5)
                                                    .color(TEXT_MUTED),
                                            );
                                        });
                                    });
                                });

                                let interact = card_resp.response.interact(egui::Sense::click());
                                if interact.hovered() {
                                    ui.painter().rect_stroke(
                                        card_resp.response.rect,
                                        CornerRadius::same(5),
                                        Stroke::new(1.0, ACCENT_BLUE),
                                        egui::StrokeKind::Inside,
                                    );
                                }

                                if interact
                                    .on_hover_text(format!("Klik untuk memulihkan keadaan pada {}", item.timestamp))
                                    .clicked()
                                {
                                    event = Some(HistoryDrawerEvent::JumpTo {
                                        id: item.id,
                                        timestamp: item.timestamp.clone(),
                                        action: item.action.clone(),
                                    });
                                }
                            }
                        }
                    });
            });
        });

        event
    }
}
