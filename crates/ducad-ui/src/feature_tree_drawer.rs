//! Feature Tree & History Drawer — Panel Terpadu Pohon Fitur & Riwayat Desain (DAG & Time-Travel).
//!
//! Menggabungkan pohon langkah-langkah desain parametrik di bagian atas dan log riwayat aktivitas
//! waktu (undo/redo time-travel) di bagian bawah dalam satu antarmuka yang bersih, padat, dan elegan.

use crate::history_drawer::{ActivityItemInfo, ActivityKindUi};
use crate::theme::{
    card_frame, glass_frame, ACCENT_BLUE, ACCENT_ORANGE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};
use ducad_core::parametric::{FeatureId, FeatureNode, FeaturePayload, FeatureStatus};
use ducad_i18n::t;
use egui::{
    Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, ScrollArea, Stroke, Ui, Vec2,
};
use egui_icons::icons::{
    ICON_AUTO_MODE, ICON_CATEGORY, ICON_CHECK_CIRCLE, ICON_CLEAR, ICON_CLOSE, ICON_DELETE,
    ICON_EDIT, ICON_ERROR, ICON_GRID_4X4, ICON_HISTORY, ICON_HORIZONTAL_RULE, ICON_LAYERS,
    ICON_POWER_SETTINGS_NEW, ICON_REFRESH, ICON_SEARCH, ICON_TIMELINE, ICON_VISIBILITY,
    ICON_VISIBILITY_OFF,
};

#[derive(Debug, Clone)]
pub enum FeatureTreeEvent {
    /// Trigger evaluasi ulang dan regenerasi solid body dari DAG.
    Regenerate,
    /// Pilih / fokus ke fitur tertentu.
    SelectFeature(FeatureId),
    /// Aktifkan / nonaktifkan fitur (Suppress).
    ToggleSuppress(FeatureId),
    /// Hapus fitur dari Feature Tree.
    DeleteFeature(FeatureId),
    /// Simpan parameter fitur yang baru diedit dan picu regenerasi.
    SaveFeatureParams {
        id: FeatureId,
        val1: f64,
        val2: Option<f64>,
    },
    /// Lompat ke snapshot riwayat waktu tertentu.
    JumpToHistory {
        id: i64,
        timestamp: String,
        action: String,
    },
    /// Hapus seluruh log riwayat aktivitas.
    ClearHistory,
    /// Tutup panel Feature Tree & Riwayat.
    Close,
}

pub struct FeatureTreeDrawer {
    pub search_query: String,
    pub custom_height: Option<f32>,
    pub editing_feature_id: Option<FeatureId>,
    pub edit_input_val1: String,
    pub edit_input_val2: String,
}

impl Default for FeatureTreeDrawer {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            custom_height: None,
            editing_feature_id: None,
            edit_input_val1: String::new(),
            edit_input_val2: String::new(),
        }
    }
}

impl FeatureTreeDrawer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Mulai mode edit parameter untuk fitur tertentu.
    pub fn start_editing(&mut self, node: &FeatureNode) {
        self.editing_feature_id = Some(node.id);
        match &node.payload {
            FeaturePayload::Extrude { distance, .. } => {
                self.edit_input_val1 = format!("{:.2}", distance);
                self.edit_input_val2.clear();
            }
            FeaturePayload::Revolve { angle_deg, .. } => {
                self.edit_input_val1 = format!("{:.1}", angle_deg);
                self.edit_input_val2.clear();
            }
            FeaturePayload::Fillet {
                radius, radius_end, ..
            } => {
                self.edit_input_val1 = format!("{:.2}", radius);
                if let Some(r_end) = radius_end {
                    self.edit_input_val2 = format!("{:.2}", r_end);
                } else {
                    self.edit_input_val2.clear();
                }
            }
            FeaturePayload::Chamfer { distance, .. } => {
                self.edit_input_val1 = format!("{:.2}", distance);
                self.edit_input_val2.clear();
            }
            FeaturePayload::Shell { thickness, .. } => {
                self.edit_input_val1 = format!("{:.2}", thickness);
                self.edit_input_val2.clear();
            }
            FeaturePayload::Sketch { dim_w, dim_h, .. } => {
                self.edit_input_val1 = format!("{:.2}", dim_w);
                if let Some(h) = dim_h {
                    self.edit_input_val2 = format!("{:.2}", h);
                } else {
                    self.edit_input_val2.clear();
                }
            }
            FeaturePayload::DatumPlane { offset, angle, .. } => {
                self.edit_input_val1 = format!("{:.2}", offset);
                self.edit_input_val2 = format!("{:.1}", angle);
            }
            FeaturePayload::Helix { radius, pitch, .. } => {
                self.edit_input_val1 = format!("{:.2}", radius);
                self.edit_input_val2 = format!("{:.2}", pitch);
            }
            _ => {
                self.edit_input_val1 = "10.0".to_string();
                self.edit_input_val2.clear();
            }
        }
    }

    /// Render panel Feature Tree & Riwayat Desain di pojok kanan bawah kanvas.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        nodes: &[FeatureNode],
        activities: &[ActivityItemInfo],
        needs_regen: bool,
        max_height: f32,
        _anchor_bottom_y: f32,
    ) -> Option<FeatureTreeEvent> {
        let mut event = None;

        glass_frame().show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                const DRAWER_W: f32 = crate::theme::BOTTOM_RIGHT_PANEL_WIDTH - 4.0;
                ui.set_min_width(DRAWER_W);
                ui.set_max_width(DRAWER_W);
                ui.set_width(DRAWER_W);
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);

                let query = self.search_query.trim().to_lowercase();

                let filtered_nodes: Vec<&FeatureNode> = nodes
                    .iter()
                    .filter(|item| {
                        if query.is_empty() {
                            true
                        } else {
                            item.name.to_lowercase().contains(&query)
                                || item.payload.type_label().to_lowercase().contains(&query)
                                || item.payload.summary_text().to_lowercase().contains(&query)
                        }
                    })
                    .collect();

                let filtered_activities: Vec<&ActivityItemInfo> = activities
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

                let estimated_h = (130.0
                    + (filtered_nodes.len().max(1) as f32 * 54.0)
                    + (filtered_activities.len().min(4) as f32 * 46.0))
                    .clamp(200.0, max_height);
                let panel_h = self.custom_height.unwrap_or(estimated_h);

                // =========================================================================
                // 0. TOP RESIZE HANDLE
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
                    let new_h = (cur_h - delta_y).clamp(160.0, max_height);
                    self.custom_height = Some(new_h);
                    ui.ctx().request_repaint();
                }

                if handle_resp.double_clicked() {
                    self.custom_height = None;
                    ui.ctx().request_repaint();
                }

                // =========================================================================
                // 1. SEARCH BAR & CLOSE BUTTON
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
                            .hint_text("Cari fitur atau riwayat...")
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
                        .on_hover_text(t!("feature-tree-close"));
                    if close_btn.clicked() {
                        event = Some(FeatureTreeEvent::Close);
                    }
                });

                // =========================================================================
                // 2. HEADER: Title & Badges
                // =========================================================================
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(ICON_TIMELINE.codepoint)
                            .size(12.5)
                            .color(ACCENT_BLUE),
                    );
                    ui.label(
                        RichText::new("Feature Tree & Riwayat")
                            .size(10.5)
                            .strong()
                            .color(TEXT_PRIMARY),
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // Badge jumlah total
                        let total_cnt = nodes.len() + activities.len();
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
                            format!("{}", total_cnt),
                            egui::FontId::proportional(8.5),
                            ACCENT_BLUE,
                        );
                    });
                });

                // Tombol aksi Regenerate Model jika dirty / ada perubahan
                if needs_regen {
                    let regen_btn = Frame {
                        inner_margin: Margin::symmetric(8, 4),
                        outer_margin: Margin::symmetric(0, 1),
                        corner_radius: CornerRadius::same(5),
                        shadow: egui::Shadow::NONE,
                        fill: Color32::from_rgb(255, 149, 0),
                        stroke: Stroke::new(0.5, ACCENT_ORANGE),
                    }
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(ICON_REFRESH.codepoint)
                                    .size(12.0)
                                    .color(Color32::BLACK),
                            );
                            ui.label(
                                RichText::new(t!("feature-tree-regen-needed"))
                                    .size(9.5)
                                    .strong()
                                    .color(Color32::BLACK),
                            );
                        });
                    });

                    if regen_btn.response.interact(egui::Sense::click()).clicked() {
                        event = Some(FeatureTreeEvent::Regenerate);
                    }
                }

                ui.add_space(2.0);

                // =========================================================================
                // 3. SCROLL AREA: SEKSI ATAS (FEATURE TREE) & SEKSI BAWAH (HISTORY)
                // =========================================================================
                let scroll_height = (panel_h - (if needs_regen { 110.0 } else { 82.0 })).max(80.0);

                ScrollArea::vertical()
                    .id_salt("unified_feature_history_scroll")
                    .min_scrolled_height(scroll_height)
                    .max_height(scroll_height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_min_height(scroll_height);
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 4.0);

                        // -------------------------------------------------------------
                        // SEKSI ATAS: POHON FITUR PARAMETRIK
                        // -------------------------------------------------------------
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("POHON FITUR (PARAMETRIK)")
                                    .size(8.5)
                                    .strong()
                                    .color(ACCENT_BLUE),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new(format!("{} fitur", nodes.len()))
                                        .size(8.0)
                                        .color(TEXT_MUTED),
                                );
                            });
                        });

                        if filtered_nodes.is_empty() {
                            card_frame().show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.vertical_centered(|ui| {
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new(if query.is_empty() {
                                            "Belum ada fitur 2D/3D"
                                        } else {
                                            "Fitur tidak ditemukan"
                                        })
                                        .size(9.5)
                                        .color(TEXT_SECONDARY),
                                    );
                                    ui.add_space(4.0);
                                });
                            });
                        } else {
                            for node in &filtered_nodes {
                                ui.push_id(node.id, |ui| {
                                    let mut edit_clicked = false;
                                    let mut suppress_clicked = false;
                                    let mut delete_clicked = false;

                                    let is_editing = self.editing_feature_id == Some(node.id);
                                    let is_dirty = node.status == FeatureStatus::NeedsRegeneration;
                                    let is_err = matches!(node.status, FeatureStatus::Error(_));

                                    let border_color = if is_editing {
                                        ACCENT_BLUE
                                    } else if is_err {
                                        Color32::from_rgb(255, 69, 58)
                                    } else if is_dirty {
                                        ACCENT_ORANGE
                                    } else if node.is_suppressed {
                                        Color32::from_rgba_premultiplied(120, 120, 130, 40)
                                    } else {
                                        Color32::from_rgba_premultiplied(10, 132, 255, 40)
                                    };

                                    let bg_color = if is_editing {
                                        Color32::from_rgb(20, 30, 48)
                                    } else if node.is_suppressed {
                                        Color32::from_rgb(20, 22, 28)
                                    } else {
                                        Color32::from_rgb(24, 27, 34)
                                    };

                                    let row_frame = Frame {
                                        inner_margin: Margin::symmetric(6, 4),
                                        outer_margin: Margin::symmetric(0, 0),
                                        corner_radius: CornerRadius::same(5),
                                        shadow: egui::Shadow::NONE,
                                        fill: bg_color,
                                        stroke: Stroke::new(0.5, border_color),
                                    };

                                    let _card_resp = row_frame.show(ui, |ui| {
                                        ui.set_width(ui.available_width());

                                        let icon_str = match node.payload.icon_name() {
                                            "plane" => ICON_LAYERS.codepoint,
                                            "sketch" => ICON_GRID_4X4.codepoint,
                                            "extrude" | "revolve" | "fillet" | "chamfer"
                                            | "shell" => ICON_CATEGORY.codepoint,
                                            "cut_extrude" => ICON_CLEAR.codepoint,
                                            "hole" => ICON_AUTO_MODE.codepoint,
                                            "helix" => ICON_TIMELINE.codepoint,
                                            _ => ICON_CATEGORY.codepoint,
                                        };

                                        let (status_color, status_icon, status_desc) =
                                            match &node.status {
                                                FeatureStatus::Valid => (
                                                    Color32::from_rgb(48, 209, 88),
                                                    ICON_CHECK_CIRCLE.codepoint,
                                                    "Fitur valid & up-to-date".to_string(),
                                                ),
                                                FeatureStatus::NeedsRegeneration => (
                                                    Color32::from_rgb(255, 149, 0),
                                                    ICON_REFRESH.codepoint,
                                                    "Perlu regenerasi".to_string(),
                                                ),
                                                FeatureStatus::Error(err_msg) => (
                                                    Color32::from_rgb(255, 69, 58),
                                                    ICON_ERROR.codepoint,
                                                    format!("Error: {err_msg}"),
                                                ),
                                                FeatureStatus::Suppressed => (
                                                    Color32::from_rgb(142, 142, 147),
                                                    ICON_VISIBILITY_OFF.codepoint,
                                                    "Fitur dinonaktifkan (suppressed)".to_string(),
                                                ),
                                            };

                                        // Baris atas: Icon + Nama Fitur + Type Badge + Status
                                        let title_resp = ui
                                            .horizontal(|ui| {
                                                ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);

                                                ui.label(RichText::new(icon_str).size(11.0).color(
                                                    if node.is_suppressed {
                                                        TEXT_MUTED
                                                    } else {
                                                        ACCENT_BLUE
                                                    },
                                                ));

                                                ui.label(
                                                    RichText::new(&node.name)
                                                        .strong()
                                                        .size(10.5)
                                                        .color(if node.is_suppressed {
                                                            TEXT_MUTED
                                                        } else {
                                                            TEXT_PRIMARY
                                                        }),
                                                );

                                                Frame {
                                                    inner_margin: Margin::symmetric(3, 0),
                                                    outer_margin: Margin::ZERO,
                                                    corner_radius: CornerRadius::same(3),
                                                    shadow: egui::Shadow::NONE,
                                                    fill: Color32::from_rgba_premultiplied(
                                                        10, 132, 255, 20,
                                                    ),
                                                    stroke: Stroke::new(
                                                        0.5,
                                                        Color32::from_rgba_premultiplied(
                                                            10, 132, 255, 60,
                                                        ),
                                                    ),
                                                }
                                                .show(ui, |ui| {
                                                    ui.label(
                                                        RichText::new(node.payload.type_label())
                                                            .size(7.5)
                                                            .color(ACCENT_BLUE),
                                                    );
                                                });

                                                ui.with_layout(
                                                    Layout::right_to_left(Align::Center),
                                                    |ui| {
                                                        ui.label(
                                                            RichText::new(status_icon)
                                                                .size(10.0)
                                                                .color(status_color),
                                                        )
                                                        .on_hover_text(status_desc);
                                                    },
                                                );
                                            })
                                            .response;

                                        if title_resp.interact(egui::Sense::click()).clicked() {
                                            event = Some(FeatureTreeEvent::SelectFeature(node.id));
                                        }

                                        // Baris bawah: Ringkasan parameter + Tombol Aksi (Edit, Suppress, Delete)
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);

                                            let summary = node.payload.summary_text();
                                            let buttons_w = 62.0;
                                            let text_w =
                                                (ui.available_width() - buttons_w - 6.0).max(30.0);

                                            ui.add_sized(
                                                Vec2::new(text_w, 18.0),
                                                egui::Label::new(
                                                    RichText::new(&summary).size(9.0).color(
                                                        if node.is_suppressed {
                                                            TEXT_MUTED
                                                        } else {
                                                            TEXT_SECONDARY
                                                        },
                                                    ),
                                                )
                                                .truncate(),
                                            )
                                            .on_hover_text(&summary);

                                            // 1. Tombol Edit Parameter
                                            let (edit_rect, edit_resp) = ui.allocate_exact_size(
                                                Vec2::splat(18.0),
                                                egui::Sense::click(),
                                            );
                                            let is_edit_hov = edit_resp.hovered();
                                            if is_edit_hov {
                                                ui.ctx().set_cursor_icon(
                                                    egui::CursorIcon::PointingHand,
                                                );
                                                ui.painter().rect_filled(
                                                    edit_rect,
                                                    CornerRadius::same(3),
                                                    Color32::from_rgba_premultiplied(
                                                        10, 132, 255, 60,
                                                    ),
                                                );
                                            }
                                            ui.painter().text(
                                                edit_rect.center(),
                                                egui::Align2::CENTER_CENTER,
                                                ICON_EDIT.codepoint,
                                                egui::FontId::proportional(11.0),
                                                if is_edit_hov { ACCENT_BLUE } else { TEXT_MUTED },
                                            );
                                            if edit_resp
                                                .on_hover_text(t!("feature-tree-edit-params"))
                                                .clicked()
                                            {
                                                edit_clicked = true;
                                            }

                                            // 2. Tombol Suppress / Unsuppress
                                            let (sup_rect, sup_resp) = ui.allocate_exact_size(
                                                Vec2::splat(18.0),
                                                egui::Sense::click(),
                                            );
                                            let is_sup_hov = sup_resp.hovered();
                                            if is_sup_hov {
                                                ui.ctx().set_cursor_icon(
                                                    egui::CursorIcon::PointingHand,
                                                );
                                                ui.painter().rect_filled(
                                                    sup_rect,
                                                    CornerRadius::same(3),
                                                    Color32::from_rgba_premultiplied(
                                                        142, 142, 147, 60,
                                                    ),
                                                );
                                            }
                                            let sup_icon = if node.is_suppressed {
                                                ICON_POWER_SETTINGS_NEW.codepoint
                                            } else {
                                                ICON_VISIBILITY.codepoint
                                            };
                                            ui.painter().text(
                                                sup_rect.center(),
                                                egui::Align2::CENTER_CENTER,
                                                sup_icon,
                                                egui::FontId::proportional(11.0),
                                                if is_sup_hov { ACCENT_BLUE } else { TEXT_MUTED },
                                            );
                                            if sup_resp
                                                .on_hover_text(if node.is_suppressed {
                                                    t!("feature-tree-unsuppress")
                                                } else {
                                                    t!("feature-tree-suppress")
                                                })
                                                .clicked()
                                            {
                                                suppress_clicked = true;
                                            }

                                            // 3. Tombol Hapus Fitur
                                            let (del_rect, del_resp) = ui.allocate_exact_size(
                                                Vec2::splat(18.0),
                                                egui::Sense::click(),
                                            );
                                            let is_del_hov = del_resp.hovered();
                                            if is_del_hov {
                                                ui.ctx().set_cursor_icon(
                                                    egui::CursorIcon::PointingHand,
                                                );
                                                ui.painter().rect_filled(
                                                    del_rect,
                                                    CornerRadius::same(3),
                                                    Color32::from_rgba_premultiplied(
                                                        255, 69, 58, 60,
                                                    ),
                                                );
                                            }
                                            let del_color = if is_del_hov {
                                                Color32::from_rgb(255, 69, 58)
                                            } else {
                                                TEXT_MUTED
                                            };
                                            ui.painter().text(
                                                del_rect.center(),
                                                egui::Align2::CENTER_CENTER,
                                                ICON_DELETE.codepoint,
                                                egui::FontId::proportional(11.0),
                                                del_color,
                                            );
                                            if del_resp
                                                .on_hover_text(t!("feature-tree-delete"))
                                                .clicked()
                                            {
                                                delete_clicked = true;
                                            }
                                        });
                                    });

                                    // Mode Edit Parameter In-line
                                    if is_editing {
                                        let edit_id = node.id;
                                        ui.add_space(2.0);
                                        Frame {
                                            inner_margin: Margin::symmetric(8, 6),
                                            outer_margin: Margin::symmetric(2, 0),
                                            corner_radius: CornerRadius::same(5),
                                            shadow: egui::Shadow::NONE,
                                            fill: Color32::from_rgb(18, 25, 38),
                                            stroke: Stroke::new(1.0, ACCENT_BLUE),
                                        }
                                        .show(ui, |ui| {
                                            ui.set_width(ui.available_width());
                                            ui.vertical(|ui| {
                                                ui.label(
                                                    RichText::new("Edit Parameter Fitur")
                                                        .strong()
                                                        .size(9.5)
                                                        .color(ACCENT_BLUE),
                                                );

                                                let (lbl1, lbl2_opt): (&str, Option<String>) =
                                                    match &node.payload {
                                                        FeaturePayload::Extrude { .. } => {
                                                            ("Tinggi Extrude (mm):", None)
                                                        }
                                                        FeaturePayload::Revolve { .. } => {
                                                            ("Sudut Putar (°):", None)
                                                        }
                                                        FeaturePayload::Fillet {
                                                            radius_end,
                                                            ..
                                                        } => {
                                                            if radius_end.is_some() {
                                                                (
                                                                    "Radius Fillet (mm):",
                                                                    Some(
                                                                        "End Radius (mm):"
                                                                            .to_string(),
                                                                    ),
                                                                )
                                                            } else {
                                                                ("Radius Fillet (mm):", None)
                                                            }
                                                        }
                                                        FeaturePayload::Chamfer { .. } => {
                                                            ("Jarak Chamfer (mm):", None)
                                                        }
                                                        FeaturePayload::Shell { .. } => {
                                                            ("Tebal Dinding (mm):", None)
                                                        }
                                                        FeaturePayload::Sketch {
                                                            dim_h,
                                                            shape_type,
                                                            ..
                                                        } => {
                                                            if dim_h.is_some() {
                                                                (
                                                                    "Panjang (X) [mm]:",
                                                                    Some(
                                                                        "Lebar (Y) [mm]:"
                                                                            .to_string(),
                                                                    ),
                                                                )
                                                            } else if shape_type == "Lingkaran"
                                                                || shape_type == "Busur"
                                                            {
                                                                ("Radius (mm):", None)
                                                            } else if shape_type == "Elips" {
                                                                (
                                                                    "Radius X (mm):",
                                                                    Some(
                                                                        "Radius Y (mm):"
                                                                            .to_string(),
                                                                    ),
                                                                )
                                                            } else {
                                                                ("Ukuran Dimensi (mm):", None)
                                                            }
                                                        }
                                                        FeaturePayload::DatumPlane { .. } => (
                                                            "Jarak Offset (mm):",
                                                            Some("Sudut Putar (°):".to_string()),
                                                        ),
                                                        FeaturePayload::Helix { .. } => (
                                                            "Radius Spiral (mm):",
                                                            Some("Pitch Ulir (mm):".to_string()),
                                                        ),
                                                        _ => ("Parameter (mm):", None),
                                                    };

                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        RichText::new(lbl1)
                                                            .size(9.0)
                                                            .color(TEXT_SECONDARY),
                                                    );
                                                    ui.add(
                                                        egui::TextEdit::singleline(
                                                            &mut self.edit_input_val1,
                                                        )
                                                        .desired_width(70.0),
                                                    );
                                                });

                                                if let Some(lbl2) = lbl2_opt {
                                                    ui.horizontal(|ui| {
                                                        ui.label(
                                                            RichText::new(lbl2)
                                                                .size(9.0)
                                                                .color(TEXT_SECONDARY),
                                                        );
                                                        ui.add(
                                                            egui::TextEdit::singleline(
                                                                &mut self.edit_input_val2,
                                                            )
                                                            .desired_width(70.0),
                                                        );
                                                    });
                                                }

                                                ui.horizontal(|ui| {
                                                    if ui
                                                        .small_button(
                                                            RichText::new(t!("feature-edit-save"))
                                                                .size(9.5)
                                                                .strong()
                                                                .color(Color32::WHITE),
                                                        )
                                                        .clicked()
                                                    {
                                                        let v1 = self
                                                            .edit_input_val1
                                                            .trim()
                                                            .parse::<f64>()
                                                            .unwrap_or(10.0);
                                                        let v2 = self
                                                            .edit_input_val2
                                                            .trim()
                                                            .parse::<f64>()
                                                            .ok();
                                                        event = Some(
                                                            FeatureTreeEvent::SaveFeatureParams {
                                                                id: edit_id,
                                                                val1: v1,
                                                                val2: v2,
                                                            },
                                                        );
                                                        self.editing_feature_id = None;
                                                    }

                                                    if ui
                                                        .small_button(
                                                            RichText::new(t!(
                                                                "feature-edit-cancel"
                                                            ))
                                                            .size(9.5)
                                                            .color(TEXT_MUTED),
                                                        )
                                                        .clicked()
                                                    {
                                                        self.editing_feature_id = None;
                                                    }
                                                });
                                            });
                                        });
                                        ui.add_space(2.0);
                                    }

                                    if edit_clicked {
                                        self.start_editing(node);
                                    } else if suppress_clicked {
                                        event = Some(FeatureTreeEvent::ToggleSuppress(node.id));
                                    } else if delete_clicked {
                                        event = Some(FeatureTreeEvent::DeleteFeature(node.id));
                                    }
                                });
                            }
                        }

                        // -------------------------------------------------------------
                        // SEKSI BAWAH: RIWAYAT AKTIVITAS & SNAPSHOTS (TIME-TRAVEL)
                        // -------------------------------------------------------------
                        ui.add_space(6.0);
                        ui.painter().line_segment(
                            [
                                egui::pos2(ui.min_rect().min.x, ui.cursor().top()),
                                egui::pos2(ui.min_rect().max.x, ui.cursor().top()),
                            ],
                            Stroke::new(0.5, Color32::from_rgb(45, 50, 65)),
                        );
                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(ICON_HISTORY.codepoint)
                                    .size(10.0)
                                    .color(Color32::from_rgb(0, 210, 180)),
                            );
                            ui.label(
                                RichText::new("RIWAYAT AKTIVITAS")
                                    .size(8.5)
                                    .strong()
                                    .color(Color32::from_rgb(0, 210, 180)),
                            );

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if !activities.is_empty() {
                                    let clear_btn = ui
                                        .small_button(
                                            RichText::new(ICON_DELETE.codepoint)
                                                .size(10.0)
                                                .color(TEXT_MUTED),
                                        )
                                        .on_hover_text("Hapus semua riwayat aktivitas");
                                    if clear_btn.clicked() {
                                        event = Some(FeatureTreeEvent::ClearHistory);
                                    }
                                }
                                ui.label(
                                    RichText::new(format!("{} entri", activities.len()))
                                        .size(8.0)
                                        .color(TEXT_MUTED),
                                );
                            });
                        });

                        if filtered_activities.is_empty() {
                            card_frame().show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.vertical_centered(|ui| {
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new(if query.is_empty() {
                                            t!("drawer-history-empty")
                                        } else {
                                            t!("history-no-match")
                                        })
                                        .size(9.0)
                                        .color(TEXT_SECONDARY),
                                    );
                                    ui.add_space(4.0);
                                });
                            });
                        } else {
                            for item in filtered_activities {
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
                                    fill: Color32::from_rgb(22, 25, 32),
                                    stroke: Stroke::new(0.5, border_color),
                                };

                                let card_resp = row_frame.show(ui, |ui| {
                                    ui.set_width(ui.available_width());

                                    // Baris atas: Icon + Aksi + Badge 2D/3D
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);

                                        ui.label(
                                            RichText::new(icon_str).size(10.0).color(badge_color),
                                        );

                                        ui.label(
                                            RichText::new(&item.action)
                                                .strong()
                                                .size(10.0)
                                                .color(TEXT_PRIMARY),
                                        );

                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
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
                                                            .size(7.5)
                                                            .strong()
                                                            .color(badge_color),
                                                    );
                                                });
                                            },
                                        );
                                    });

                                    // Baris bawah: Detail + Timestamp
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);

                                        if !item.details.is_empty() {
                                            ui.add(
                                                egui::Label::new(
                                                    RichText::new(&item.details)
                                                        .size(8.5)
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
                                                        .size(8.0)
                                                        .color(TEXT_MUTED),
                                                );
                                            },
                                        );
                                    });
                                });

                                let interact = card_resp.response.interact(egui::Sense::click());
                                if interact.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    ui.painter().rect_stroke(
                                        card_resp.response.rect,
                                        CornerRadius::same(5),
                                        Stroke::new(1.0, ACCENT_BLUE),
                                        egui::StrokeKind::Inside,
                                    );
                                }
                                if interact
                                    .on_hover_text("Klik untuk melompat / restore ke snapshot ini")
                                    .clicked()
                                {
                                    event = Some(FeatureTreeEvent::JumpToHistory {
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
