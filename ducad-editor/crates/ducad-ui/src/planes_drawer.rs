//! Reference Planes Drawer — Panel Daftar Bidang Kerja 3D bergaya Shapr3D.
//!
//! Menampilkan daftar bidang referensi (Top, Front, Right, serta Datum Plane kustom)
//! di pojok kanan bawah kanvas, lengkap dengan badge status aktif, tombol ganti bidang instan,
//! tombol pembuatan bidang baru, dan penghapusan bidang kustom.

use crate::theme::{
    card_frame, glass_frame, ACCENT_BLUE, ACCENT_ORANGE, BORDER_SUBTLE, TEXT_MUTED, TEXT_PRIMARY,
    TEXT_SECONDARY,
};
use ducad_i18n::t;
use egui::{
    Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, ScrollArea, Stroke, Ui, Vec2,
};
use egui_icons::icons::{
    ICON_ADD, ICON_CLEAR, ICON_CLOSE, ICON_DELETE, ICON_GRID_4X4, ICON_LAYERS_OFF, ICON_SEARCH,
};

#[derive(Debug, Clone)]
pub struct PlaneItemInfo {
    pub index: usize,
    pub name: String,
    pub is_active: bool,
    pub is_custom: bool,
    pub custom_id: Option<u32>,
}

#[derive(Debug, Clone)]
pub enum PlanesDrawerEvent {
    SelectPlane(usize),
    DeletePlane(u32),
    CreateNewPlane,
    Close,
}

#[derive(Default)]
pub struct PlanesDrawer {
    pub search_query: String,
    pub custom_height: Option<f32>,
}

impl PlanesDrawer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Render panel daftar bidang referensi di pojok kanan bawah kanvas.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        planes: &[PlaneItemInfo],
        max_height: f32,
        _anchor_bottom_y: f32,
    ) -> Option<PlanesDrawerEvent> {
        let mut event = None;

        glass_frame().show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                const DRAWER_W: f32 = crate::theme::BOTTOM_RIGHT_PANEL_WIDTH;
                ui.set_min_width(DRAWER_W);
                ui.set_max_width(DRAWER_W);
                ui.set_width(DRAWER_W);
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);

                let query = self.search_query.trim().to_lowercase();
                let filtered: Vec<&PlaneItemInfo> = planes
                    .iter()
                    .filter(|item| {
                        if query.is_empty() {
                            true
                        } else {
                            item.name.to_lowercase().contains(&query)
                        }
                    })
                    .collect();

                let estimated_h =
                    (100.0 + (filtered.len().max(1) as f32 * 46.0)).clamp(140.0, max_height);
                let panel_h = self.custom_height.unwrap_or(estimated_h);

                // =========================================================================
                // 1. HEADER DRAWER (Judul, Badge Jumlah, Tombol + New, Tombol Close)
                // =========================================================================
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);

                    // Ikon Layers Off
                    ui.label(
                        RichText::new(ICON_LAYERS_OFF.codepoint)
                            .size(14.0)
                            .color(ACCENT_BLUE),
                    );

                    // Judul Panel
                    ui.label(
                        RichText::new(t!("planes-drawer-title"))
                            .size(12.0)
                            .strong()
                            .color(TEXT_PRIMARY),
                    );

                    // Badge Angka Lingkaran Total Bidang
                    let count_str = planes.len().to_string();
                    let font_id = egui::FontId::proportional(9.0);
                    let galley = ui.painter().layout_no_wrap(
                        count_str.clone(),
                        font_id,
                        Color32::from_rgb(180, 205, 235),
                    );
                    let diameter = (galley.size().x.max(galley.size().y) + 6.0).max(15.0);
                    let (rect, _) = ui.allocate_exact_size(
                        Vec2::new(diameter, diameter),
                        egui::Sense::hover(),
                    );
                    ui.painter().circle_filled(
                        rect.center(),
                        diameter * 0.5,
                        Color32::from_rgb(25, 45, 80),
                    );
                    ui.painter().circle_stroke(
                        rect.center(),
                        diameter * 0.5,
                        Stroke::new(0.5, Color32::from_rgb(60, 110, 190)),
                    );
                    ui.painter().galley(
                        rect.center() - galley.size() * 0.5,
                        galley,
                        Color32::from_rgb(180, 205, 235),
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // Tombol Close (X)
                        let close_btn = ui.add(
                            egui::Button::new(
                                RichText::new(ICON_CLOSE.codepoint)
                                    .size(12.0)
                                    .color(TEXT_SECONDARY),
                            )
                            .frame(false)
                            .min_size(Vec2::new(16.0, 16.0)),
                        );
                        if close_btn
                            .on_hover_text(t!("param-close"))
                            .clicked()
                        {
                            event = Some(PlanesDrawerEvent::Close);
                        }

                        // Tombol + New Plane
                        let add_btn = ui.add(
                            egui::Button::new(
                                RichText::new(ICON_ADD.codepoint)
                                    .size(14.0)
                                    .color(ACCENT_BLUE),
                            )
                            .frame(false)
                            .min_size(Vec2::new(18.0, 18.0)),
                        );
                        if add_btn
                            .on_hover_text(t!("planes-drawer-new"))
                            .clicked()
                        {
                            event = Some(PlanesDrawerEvent::CreateNewPlane);
                        }
                    });
                });

                ui.add_space(1.0);

                // =========================================================================
                // 2. SEARCH BAR COMPACT
                // =========================================================================
                Frame::NONE
                    .fill(Color32::from_rgb(20, 23, 30))
                    .stroke(Stroke::new(0.5, BORDER_SUBTLE))
                    .corner_radius(CornerRadius::same(5))
                    .inner_margin(Margin::symmetric(6, 3))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                            ui.label(
                                RichText::new(ICON_SEARCH.codepoint)
                                    .size(11.0)
                                    .color(TEXT_MUTED),
                            );

                            let edit_resp = ui.add(
                                egui::TextEdit::singleline(&mut self.search_query)
                                    .hint_text(
                                        RichText::new(t!("planes-drawer-search"))
                                            .size(10.0)
                                            .color(TEXT_MUTED),
                                    )
                                    .font(egui::FontId::proportional(10.5))
                                    .text_color(TEXT_PRIMARY)
                                    .frame(Frame::NONE)
                                    .desired_width(DRAWER_W - 60.0),
                            );

                            if !self.search_query.is_empty() {
                                let clear_btn = ui.add(
                                    egui::Button::new(
                                        RichText::new(ICON_CLEAR.codepoint)
                                            .size(10.0)
                                            .color(TEXT_SECONDARY),
                                    )
                                    .frame(false)
                                    .min_size(Vec2::new(12.0, 12.0)),
                                );
                                if clear_btn.clicked() {
                                    self.search_query.clear();
                                    edit_resp.request_focus();
                                }
                            }
                        });
                    });

                ui.add_space(2.0);

                // =========================================================================
                // 3. DAFTAR KARTU BIDANG (SCROLL AREA)
                // =========================================================================
                ScrollArea::vertical()
                    .id_salt("planes_drawer_scroll")
                    .max_height(panel_h - 60.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 3.0);

                        if filtered.is_empty() {
                            ui.add_space(10.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new(ICON_GRID_4X4.codepoint)
                                        .size(24.0)
                                        .color(TEXT_MUTED),
                                );
                                ui.add_space(3.0);
                                ui.label(
                                    RichText::new(t!("planes-drawer-empty"))
                                        .size(10.5)
                                        .color(TEXT_MUTED),
                                );
                            });
                            ui.add_space(10.0);
                        } else {
                            for plane in filtered {
                                ui.push_id(format!("plane_item_{}", plane.index), |ui| {
                                    let is_active = plane.is_active;

                                    let card_bg = if is_active {
                                        Color32::from_rgb(18, 38, 68)
                                    } else {
                                        Color32::from_rgb(26, 29, 36)
                                    };

                                    let card_stroke = if is_active {
                                        Stroke::new(1.0, ACCENT_BLUE)
                                    } else {
                                        Stroke::new(0.5, BORDER_SUBTLE)
                                    };

                                    card_frame()
                                        .fill(card_bg)
                                        .stroke(card_stroke)
                                        .corner_radius(CornerRadius::same(5))
                                        .inner_margin(Margin::symmetric(7, 5))
                                        .show(ui, |ui| {
                                            ui.set_width(ui.available_width());
                                            ui.horizontal(|ui| {
                                                ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);

                                                // Ikon Bidang
                                                let icon = if plane.is_custom {
                                                    ICON_GRID_4X4.codepoint
                                                } else {
                                                    ICON_LAYERS_OFF.codepoint
                                                };
                                                let icon_color = if is_active {
                                                    ACCENT_BLUE
                                                } else if plane.is_custom {
                                                    ACCENT_ORANGE
                                                } else {
                                                    TEXT_SECONDARY
                                                };

                                                ui.label(
                                                    RichText::new(icon)
                                                        .size(12.5)
                                                        .color(icon_color),
                                                );

                                                // Nama Bidang (Area klik untuk mengaktifkan bidang)
                                                let text_color = if is_active {
                                                    Color32::WHITE
                                                } else {
                                                    TEXT_PRIMARY
                                                };
                                                let name_resp = ui.add(
                                                    egui::Label::new(
                                                        RichText::new(&plane.name)
                                                            .size(11.0)
                                                            .strong()
                                                            .color(text_color),
                                                    )
                                                    .sense(egui::Sense::click()),
                                                );

                                                if name_resp.hovered() {
                                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                                }
                                                if name_resp.clicked() {
                                                    event = Some(PlanesDrawerEvent::SelectPlane(plane.index));
                                                }

                                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                    // Tombol Hapus (khusus custom plane)
                                                    if let Some(custom_id) = plane.custom_id {
                                                        let del_btn = ui.add(
                                                            egui::Button::new(
                                                                RichText::new(ICON_DELETE.codepoint)
                                                                    .size(13.0)
                                                                    .color(Color32::from_rgb(240, 90, 90)),
                                                            )
                                                            .frame(false)
                                                            .min_size(Vec2::new(18.0, 18.0)),
                                                        );
                                                        if del_btn
                                                            .on_hover_text(t!("planes-drawer-delete-tooltip"))
                                                            .clicked()
                                                        {
                                                            event = Some(PlanesDrawerEvent::DeletePlane(custom_id));
                                                        }
                                                    }

                                                    // Badge Aktif
                                                    if is_active {
                                                        let badge_text = t!("planes-drawer-active");
                                                        let badge_bg = Color32::from_rgb(14, 80, 160);
                                                        let badge_fg = Color32::WHITE;

                                                        Frame::NONE
                                                            .fill(badge_bg)
                                                            .corner_radius(CornerRadius::same(3))
                                                            .inner_margin(Margin::symmetric(4, 1))
                                                            .show(ui, |ui| {
                                                                ui.label(
                                                                    RichText::new(badge_text)
                                                                        .size(9.0)
                                                                        .strong()
                                                                        .color(badge_fg),
                                                                );
                                                            });
                                                    }
                                                });
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
