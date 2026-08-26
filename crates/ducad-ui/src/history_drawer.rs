//! History Drawer — Panel Riwayat Kegiatan 2D & 3D bergaya Shapr3D.
//!
//! Menampilkan daftar kegiatan kronologis (aktivitas 2D vs 3D) dalam bentuk
//! list kartu padat/kompak, badge warna pembeda, badge lingkaran sempurna,
//! tombol hapus ikon bersih, dan kemampuan jump (undo/redo) saat item diklik.

use crate::theme::{
    card_frame, glass_frame, ACCENT_BLUE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};
use ducad_i18n::t;
use egui::{
    Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, ScrollArea, Stroke, Ui, Vec2,
};
use egui_icons::icons::{
    ICON_CATEGORY, ICON_CLEAR, ICON_CLOSE, ICON_DELETE, ICON_HISTORY, ICON_HORIZONTAL_RULE,
    ICON_SEARCH,
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
        _anchor_bottom_y: f32,
    ) -> Option<HistoryDrawerEvent> {
        let mut event = None;

        glass_frame().show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                const DRAWER_W: f32 = 230.0;
                ui.set_min_width(DRAWER_W);
                ui.set_max_width(DRAWER_W);
                ui.set_width(DRAWER_W);
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);

                let query = self.search_query.trim().to_lowercase();
                let filtered: Vec<&ActivityItemInfo> = activities
                    .iter()
                    .filter(|item| {
                        if query.is_empty() {
                            true
                        } else {
                            item.action.to_lowercase().contains(&query)
                                || item.details.to_lowercase().contains(&query)
                                || item.timestamp.to_lowercase().contains(&query)
                        }
                    })
                    .collect();

                let estimated_h =
                    (90.0 + (filtered.len().max(1) as f32 * 56.0)).clamp(140.0, max_height);
                let panel_h = self.custom_height.unwrap_or(estimated_h);

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
                let pill_rect =
                    egui::Rect::from_center_size(handle_rect.center(), Vec2::new(36.0, 4.0));
                let pill_color = if handle_resp.dragged() {
                    ACCENT_BLUE
                } else if handle_resp.hovered() {
                    TEXT_SECONDARY
                } else {
                    Color32::from_rgb(70, 75, 90)
                };
                ui.painter()
                    .rect_filled(pill_rect, CornerRadius::same(2), pill_color);

                if handle_resp.dragged() {
                    let delta_y = handle_resp.drag_delta().y;
                    let cur_h = self.custom_height.unwrap_or(estimated_h);
                    let new_h = (cur_h - delta_y).clamp(120.0, max_height);
                    self.custom_height = Some(new_h);
                    ui.ctx().request_repaint();
                }

                if handle_resp.double_clicked() {
                    self.custom_height = None;
                    ui.ctx().request_repaint();
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
                    let clear_btn_w = if has_query { 20.0 } else { 0.0 };
                    let close_btn_w = 22.0;
                    let text_width =
                        (ui.available_width() - clear_btn_w - close_btn_w - 6.0).max(60.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.search_query)
                            .hint_text(t!("history-search-placeholder"))
                            .clip_text(true)
                            .desired_width(text_width),
                    );

                    if has_query {
                        if ui
                            .small_button(
                                RichText::new(ICON_CLEAR.codepoint)
                                    .size(11.0)
                                    .color(TEXT_SECONDARY),
                            )
                            .on_hover_text(t!("history-clear-search"))
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
                        .on_hover_text(t!("history-close"));
                    if close_btn.clicked() {
                        event = Some(HistoryDrawerEvent::Close);
                    }
                });

                // Baris info ringkas & tombol clear (ikon bersih + badge lingkaran sempurna)
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(ICON_HISTORY.codepoint)
                            .size(11.5)
                            .color(ACCENT_BLUE),
                    );
                    ui.label(
                        RichText::new(t!("drawer-history-title"))
                            .size(10.0)
                            .strong()
                            .color(TEXT_PRIMARY),
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // Tombol Hapus (cukup icon murni dengan hover merah)
                        if !activities.is_empty() {
                            let (del_rect, del_resp) =
                                ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::click());
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
                            if del_resp.on_hover_text(t!("drawer-clear-history")).clicked() {
                                event = Some(HistoryDrawerEvent::ClearAll);
                            }
                        }

                        // Badge counter (lingkaran penuh sempurna radius 9.0)
                        let (badge_rect, _) =
                            ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::hover());
                        ui.painter().circle_filled(
                            badge_rect.center(),
                            9.0,
                            Color32::from_rgba_premultiplied(10, 132, 255, 45),
                        );
                        ui.painter().circle_stroke(
                            badge_rect.center(),
                            9.0,
                            Stroke::new(0.5, ACCENT_BLUE),
                        );
                        ui.painter().text(
                            badge_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("{}", activities.len()),
                            egui::FontId::proportional(9.0),
                            ACCENT_BLUE,
                        );
                    });
                });

                ui.add_space(2.0);

                // =========================================================================
                // 2. LIST AKTIVITAS DENGAN SCROLL AREA
                // =========================================================================
                let scroll_height = (panel_h - 90.0).max(60.0);

                ScrollArea::vertical()
                    .id_salt("history_drawer_scroll")
                    .min_scrolled_height(scroll_height)
                    .max_height(scroll_height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_min_height(scroll_height);
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 3.0);

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
                                            t!("drawer-history-empty")
                                        } else {
                                            t!("history-no-match")
                                        })
                                        .size(10.5)
                                        .strong()
                                        .color(TEXT_SECONDARY),
                                    );
                                    ui.label(
                                        RichText::new(t!("history-auto-record"))
                                            .size(9.0)
                                            .color(TEXT_MUTED),
                                    );
                                    ui.add_space(8.0);
                                });
                            });
                        } else {
                            for item in filtered {
                                let (badge_color, badge_text, icon_str, border_color) =
                                    match item.kind {
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
                                            RichText::new(icon_str).size(11.0).color(badge_color),
                                        );

                                        ui.label(
                                            RichText::new(&item.action)
                                                .strong()
                                                .size(10.5)
                                                .color(TEXT_PRIMARY),
                                        );

                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
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
                                            },
                                        );
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

                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                ui.label(
                                                    RichText::new(&item.timestamp)
                                                        .size(8.5)
                                                        .color(TEXT_MUTED),
                                                );
                                            },
                                        );
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
                                    .on_hover_text(t!(
                                        "history-jump-tooltip",
                                        time = item.timestamp.as_str()
                                    ))
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
