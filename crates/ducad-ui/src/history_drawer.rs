//! History Drawer — Panel Riwayat Kegiatan 2D & 3D bergaya Shapr3D.
//!
//! Menampilkan daftar kegiatan kronologis (aktivitas 2D vs 3D) dalam bentuk
//! list kartu yang elegan, badge warna pembeda, search bar terintegrasi,
//! dan tombol aksi.

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
}

pub struct HistoryDrawer {
    pub search_query: String,
}

impl Default for HistoryDrawer {
    fn default() -> Self {
        Self {
            search_query: String::new(),
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
    ) -> Option<HistoryDrawerEvent> {
        let mut event = None;

        glass_frame().show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_width(270.0);
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 6.0);

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

                // Baris info ringkas & tombol clear
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
                        if !activities.is_empty() {
                            if ui
                                .small_button(
                                    RichText::new(ICON_DELETE.codepoint)
                                        .size(11.0)
                                        .color(Color32::from_rgb(240, 90, 90)),
                                )
                                .on_hover_text("Kosongkan Riwayat")
                                .clicked()
                            {
                                event = Some(HistoryDrawerEvent::ClearAll);
                            }
                        }

                        // Badge counter
                        let (badge_rect, _) = ui.allocate_exact_size(Vec2::new(26.0, 16.0), egui::Sense::hover());
                        ui.painter().rect_filled(
                            badge_rect,
                            CornerRadius::same(8),
                            Color32::from_rgb(46, 50, 62),
                        );
                        ui.painter().text(
                            badge_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("{}", activities.len()),
                            egui::FontId::proportional(9.5),
                            Color32::from_rgb(160, 166, 178),
                        );
                    });
                });

                ui.separator();

                let query = self.search_query.to_lowercase().trim().to_string();

                // =========================================================================
                // 2. SCROLLABLE ACTIVITY LIST
                // =========================================================================
                ScrollArea::vertical()
                    .auto_shrink([false, true])
                    .max_height(max_height)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 5.0);

                    let filtered_activities: Vec<&ActivityItemInfo> = activities
                        .iter()
                        .filter(|a| {
                            query.is_empty()
                                || a.action.to_lowercase().contains(&query)
                                || a.details.to_lowercase().contains(&query)
                                || a.timestamp.contains(&query)
                        })
                        .collect();

                    if filtered_activities.is_empty() {
                        card_frame().show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.vertical_centered(|ui| {
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new(ICON_HISTORY.codepoint)
                                        .size(22.0)
                                        .color(TEXT_MUTED),
                                );
                                ui.label(
                                    RichText::new(if query.is_empty() {
                                        "Belum ada catatan aktivitas"
                                    } else {
                                        "Tidak ditemukan hasil pencarian"
                                    })
                                    .size(11.0)
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
                        for item in filtered_activities {
                            let (badge_color, badge_text, icon_str, border_color) = match item.kind {
                                ActivityKindUi::Sketch2D => (
                                    Color32::from_rgb(0, 180, 150),
                                    "2D",
                                    ICON_HORIZONTAL_RULE.codepoint,
                                    Color32::from_rgba_premultiplied(0, 210, 180, 50),
                                ),
                                ActivityKindUi::Solid3D => (
                                    Color32::from_rgb(175, 82, 222),
                                    "3D",
                                    ICON_CATEGORY.codepoint,
                                    Color32::from_rgba_premultiplied(191, 90, 242, 50),
                                ),
                            };

                            let row_frame = Frame {
                                inner_margin: Margin::symmetric(8, 6),
                                outer_margin: Margin::symmetric(0, 1),
                                corner_radius: CornerRadius::same(6),
                                shadow: egui::Shadow::NONE,
                                fill: Color32::from_rgb(26, 29, 36),
                                stroke: Stroke::new(0.5, border_color),
                            };

                            row_frame.show(ui, |ui| {
                                ui.set_width(ui.available_width());

                                // Baris atas: Icon + Nama Aksi + Badge 2D/3D
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(icon_str)
                                            .size(12.0)
                                            .color(badge_color),
                                    );

                                    ui.label(
                                        RichText::new(&item.action)
                                            .strong()
                                            .size(11.0)
                                            .color(TEXT_PRIMARY),
                                    );

                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        // Badge 2D / 3D
                                        Frame {
                                            inner_margin: Margin::symmetric(5, 1),
                                            outer_margin: Margin::ZERO,
                                            corner_radius: CornerRadius::same(4),
                                            shadow: egui::Shadow::NONE,
                                            fill: badge_color.linear_multiply(0.2),
                                            stroke: Stroke::new(0.5, badge_color),
                                        }
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new(badge_text)
                                                    .size(8.5)
                                                    .strong()
                                                    .color(badge_color),
                                            );
                                        });
                                    });
                                });

                                // Baris bawah: Details ringkas + Timestamp
                                ui.horizontal(|ui| {
                                    if !item.details.is_empty() {
                                        ui.label(
                                            RichText::new(&item.details)
                                                .size(9.5)
                                                .color(TEXT_SECONDARY),
                                        );
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
                        }
                    }
                });
            });
        });

        event
    }
}
