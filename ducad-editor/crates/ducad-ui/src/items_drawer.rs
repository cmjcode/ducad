//! Outliner & Scene Properties Panel bergaya Shapr3D dengan Material Icons.
//!
//! Menampilkan panel dock di pojok kanan bawah kanvas untuk navigasi hierarki dokumen:
//! objek sketsa 2D aktif (garis, lingkaran, busur, elips) dan daftar solid body 3D (BODIES)
//! dalam bentuk accordion yang rapi, search bar compact terintegrasi tombol close,
//! badge angka lingkaran sempurna, dan seluruh baris clickable.

use crate::theme::{
    card_frame, glass_frame, ACCENT_BLUE, BORDER_SUBTLE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};
use crate::vector::{
    LayersPanelEvent, LayersPanelState, PropertiesPanelEvent, PropertiesPanelState, StyleDiff,
    SwatchManager,
};
use ducad_i18n::t;
use ducad_sketch::layer::{Layer, LayerId};
use ducad_sketch::Rgba;
use egui::{
    Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, ScrollArea, Stroke, Ui, Vec2,
};
use egui_icons::icons::{
    ICON_CATEGORY, ICON_CLEAR, ICON_CLOSE, ICON_CUBE_OUTLINE, ICON_FOLDER, ICON_HORIZONTAL_RULE,
    ICON_KEYBOARD_ARROW_DOWN, ICON_KEYBOARD_ARROW_RIGHT, ICON_LAYERS, ICON_SEARCH, ICON_TUNE,
    ICON_VISIBILITY, ICON_VISIBILITY_OFF,
};

pub struct BodyItemInfo {
    pub id_raw: u64,
    pub name: String,
    pub visible: bool,
    pub selected: bool,
    pub material: ducad_core::Material,
}

pub struct Entity2dItemInfo {
    pub id_raw: u64,
    pub name: String,
    pub icon: &'static str,
    pub visible: bool,
    pub selected: bool,
    /// Nama grup yang ditetapkan user. `None` = entitas flat tanpa grup.
    pub group_name: Option<String>,
}

/// Tab aktif pada panel dock dokumen di pojok kanan bawah.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ItemsDrawerTab {
    #[default]
    Objects,
    Properties,
    Layers,
}

/// Konteks pengeditan vektor untuk tab Properti & Layer pada ItemsDrawer.
pub struct VectorDrawerContext<'a> {
    pub diff: &'a StyleDiff,
    pub properties_panel: &'a mut PropertiesPanelState,
    pub swatches: &'a mut SwatchManager,
    pub doc_swatches: &'a [Rgba],
    pub layers_panel: &'a mut LayersPanelState,
    pub layer_order_reversed: &'a [(LayerId, Layer)],
    pub active_layer: Option<LayerId>,
}

#[derive(Debug, Clone)]
pub enum ItemsDrawerEvent {
    ToggleBodyVisibility(u64),
    ToggleEntity2dVisibility(u64),
    ToggleGroupVisibility(String),
    SelectBody { id_raw: u64, extend: bool },
    SelectEntity2d { id_raw: u64, extend: bool },
    SelectGroup { name: String, extend: bool },
    Close,
    Open,
    ToggleGroup(String),
    TabChanged(ItemsDrawerTab),
    Property(PropertiesPanelEvent),
    Layer(LayersPanelEvent),
}

pub struct ItemsDrawer {
    pub search_query: String,
    pub objects_2d_expanded: bool,
    pub bodies_expanded: bool,
    pub custom_height: Option<f32>,
    pub last_rendered_height: f32,
    /// Expanded state per nama grup 2D.
    pub expanded_groups: std::collections::HashMap<String, bool>,
    /// Tab aktif saat ini (Objek, Properti, atau Layer).
    pub active_tab: ItemsDrawerTab,
}

impl Default for ItemsDrawer {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            objects_2d_expanded: true,
            bodies_expanded: true,
            custom_height: None,
            last_rendered_height: 200.0,
            expanded_groups: std::collections::HashMap::new(),
            active_tab: ItemsDrawerTab::Objects,
        }
    }
}

/// Helper untuk merender kartu item dengan tombol eye visibility di sisi kiri
#[allow(clippy::too_many_arguments)]
fn render_item_card(
    ui: &mut Ui,
    id_raw: u64,
    icon: &str,
    name: &str,
    visible: bool,
    selected: bool,
    indent: f32,
    show_selected_badge: bool,
) -> (bool, bool) {
    ui.push_id(id_raw, |ui| {
        let card_bg = if selected {
            Color32::from_rgb(18, 38, 68)
        } else if visible {
            if indent > 0.0 {
                Color32::from_rgb(22, 25, 33)
            } else {
                Color32::from_rgb(26, 29, 36)
            }
        } else {
            Color32::from_rgb(20, 22, 28)
        };
        let card_stroke = if selected {
            Stroke::new(1.0, ACCENT_BLUE)
        } else {
            Stroke::new(0.5, BORDER_SUBTLE)
        };

        let row_frame = Frame {
            inner_margin: Margin::symmetric(
                if indent > 0.0 { 6 } else { 8 },
                if indent > 0.0 { 5 } else { 6 },
            ),
            outer_margin: Margin::symmetric(0, 1),
            corner_radius: CornerRadius::same(if indent > 0.0 { 5 } else { 6 }),
            shadow: egui::Shadow::NONE,
            fill: card_bg,
            stroke: card_stroke,
        };

        let mut eye_clicked = false;

        let card_output = ui.horizontal(|ui| {
            if indent > 0.0 {
                ui.add_space(indent);
            }

            let row_resp = row_frame.show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    // Tombol eye visibility
                    let (eye_rect, eye_resp) =
                        ui.allocate_exact_size(Vec2::new(24.0, 20.0), egui::Sense::click());
                    let is_eye_hovered = eye_resp.hovered();
                    if is_eye_hovered {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    let eye_bg = if is_eye_hovered {
                        Color32::from_rgb(60, 75, 100)
                    } else if visible {
                        Color32::from_rgb(38, 43, 56)
                    } else {
                        Color32::from_rgb(24, 26, 32)
                    };

                    let eye_icon = if visible {
                        ICON_VISIBILITY.codepoint
                    } else {
                        ICON_VISIBILITY_OFF.codepoint
                    };
                    let eye_color = if visible {
                        if selected {
                            Color32::WHITE
                        } else {
                            TEXT_PRIMARY
                        }
                    } else {
                        TEXT_MUTED
                    };

                    ui.painter()
                        .rect_filled(eye_rect, CornerRadius::same(4), eye_bg);
                    ui.painter().text(
                        eye_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        eye_icon,
                        egui::FontId::proportional(13.0),
                        eye_color,
                    );

                    if eye_resp.clicked() {
                        eye_clicked = true;
                    }

                    ui.add_space(4.0);

                    let icon_color = if selected {
                        ACCENT_BLUE
                    } else if visible {
                        TEXT_SECONDARY
                    } else {
                        TEXT_MUTED
                    };
                    ui.label(RichText::new(icon).size(13.0).color(icon_color));
                    ui.add_space(6.0);

                    let name_color = if selected {
                        Color32::WHITE
                    } else if visible {
                        TEXT_PRIMARY
                    } else {
                        TEXT_MUTED
                    };
                    ui.label(RichText::new(name).strong().size(11.5).color(name_color));

                    if show_selected_badge && selected {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            Frame {
                                inner_margin: Margin::symmetric(6, 1),
                                outer_margin: Margin::ZERO,
                                corner_radius: CornerRadius::same(4),
                                shadow: egui::Shadow::NONE,
                                fill: Color32::from_rgb(10, 132, 255),
                                stroke: Stroke::NONE,
                            }
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new("Terpilih")
                                        .size(9.5)
                                        .strong()
                                        .color(Color32::WHITE),
                                );
                            });
                        });
                    }
                });
            });

            row_resp.response
        });

        let row_response = card_output.inner;
        let card_clicked =
            visible && !eye_clicked && row_response.interact(egui::Sense::click()).clicked();

        (eye_clicked, card_clicked)
    })
    .inner
}

impl ItemsDrawer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Render floating icon button folder di pojok kanan bawah saat panel tertutup.
    pub fn show_floating_button(
        &self,
        ui: &mut Ui,
        _total_items: usize,
    ) -> Option<ItemsDrawerEvent> {
        let mut event = None;

        let size = Vec2::splat(38.0);
        let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());

        let is_hovered = resp.hovered();
        let bg_color = if is_hovered {
            Color32::from_rgb(38, 44, 58)
        } else {
            Color32::from_rgb(24, 27, 34)
        };
        let stroke_color = if is_hovered {
            ACCENT_BLUE
        } else {
            BORDER_SUBTLE
        };

        ui.painter().rect(
            rect,
            CornerRadius::same(19),
            bg_color,
            Stroke::new(if is_hovered { 1.5 } else { 1.0 }, stroke_color),
            egui::StrokeKind::Inside,
        );

        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            ICON_FOLDER.codepoint,
            egui::FontId::proportional(19.0),
            if is_hovered {
                Color32::WHITE
            } else {
                TEXT_PRIMARY
            },
        );

        if resp.clicked() {
            event = Some(ItemsDrawerEvent::Open);
        }

        event
    }

    /// Render panel dock di pojok kanan bawah (tab Objek, Properti gaya vektor, dan Layer).
    pub fn show(
        &mut self,
        ui: &mut Ui,
        entities_2d: &[Entity2dItemInfo],
        bodies: &[BodyItemInfo],
        max_height: f32,
        _anchor_bottom_y: f32,
        mut vector_ctx: Option<VectorDrawerContext<'_>>,
    ) -> Option<ItemsDrawerEvent> {
        let mut event = None;

        let query = self.search_query.trim().to_lowercase();

        let frame_resp = glass_frame().show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                const DRAWER_W: f32 = crate::theme::BOTTOM_RIGHT_PANEL_WIDTH - 4.0;
                ui.set_min_width(DRAWER_W);
                ui.set_max_width(DRAWER_W);
                ui.set_width(DRAWER_W);
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
                    let cur_h = self
                        .custom_height
                        .unwrap_or(self.last_rendered_height)
                        .clamp(120.0, max_height);
                    let new_h = (cur_h - delta_y).clamp(120.0, max_height);
                    self.custom_height = Some(new_h);
                    ui.ctx().request_repaint();
                }

                if handle_resp.double_clicked() {
                    self.custom_height = None;
                    ui.ctx().request_repaint();
                }

                // =========================================================================
                // 1. TAB BAR (Objek | Properti | Layer) DENGAN TOMBOL CLOSE
                // =========================================================================
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);

                    let is_obj = self.active_tab == ItemsDrawerTab::Objects;
                    if ui
                        .selectable_label(
                            is_obj,
                            RichText::new(t!("drawer-tab-objects"))
                                .size(11.5)
                                .color(if is_obj { Color32::WHITE } else { TEXT_SECONDARY })
                                .strong(),
                        )
                        .clicked()
                    {
                        self.active_tab = ItemsDrawerTab::Objects;
                        event = Some(ItemsDrawerEvent::TabChanged(ItemsDrawerTab::Objects));
                    }

                    let is_prop = self.active_tab == ItemsDrawerTab::Properties;
                    if ui
                        .selectable_label(
                            is_prop,
                            RichText::new(t!("drawer-tab-properties"))
                                .size(11.5)
                                .color(if is_prop { Color32::WHITE } else { TEXT_SECONDARY })
                                .strong(),
                        )
                        .clicked()
                    {
                        self.active_tab = ItemsDrawerTab::Properties;
                        event = Some(ItemsDrawerEvent::TabChanged(ItemsDrawerTab::Properties));
                    }

                    let is_layer = self.active_tab == ItemsDrawerTab::Layers;
                    if ui
                        .selectable_label(
                            is_layer,
                            RichText::new(t!("drawer-tab-layers"))
                                .size(11.5)
                                .color(if is_layer { Color32::WHITE } else { TEXT_SECONDARY })
                                .strong(),
                        )
                        .clicked()
                    {
                        self.active_tab = ItemsDrawerTab::Layers;
                        event = Some(ItemsDrawerEvent::TabChanged(ItemsDrawerTab::Layers));
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let close_btn = ui
                            .small_button(
                                RichText::new(ICON_CLOSE.codepoint)
                                    .size(12.0)
                                    .color(TEXT_SECONDARY),
                            )
                            .on_hover_text(t!("history-close"));
                        if close_btn.clicked() {
                            event = Some(ItemsDrawerEvent::Close);
                        }
                    });
                });

                ui.separator();

                match self.active_tab {
                    ItemsDrawerTab::Objects => {
                        // Search bar compact
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(ICON_SEARCH.codepoint)
                                    .size(13.0)
                                    .color(TEXT_SECONDARY),
                            );
                            let has_query = !self.search_query.is_empty();
                            let clear_btn_w = if has_query { 20.0 } else { 0.0 };
                            let text_width =
                                (ui.available_width() - clear_btn_w - 4.0).max(60.0);
                            ui.add(
                                egui::TextEdit::singleline(&mut self.search_query)
                                    .hint_text(t!("drawer-search-placeholder"))
                                    .clip_text(true)
                                    .desired_width(text_width),
                            );

                            if has_query
                                && ui
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
                        });

                        ui.add_space(2.0);

                        let is_custom = self.custom_height.is_some();
                        let header_overhead = 88.0;
                        let max_scroll_h = match self.custom_height {
                            Some(custom_h) => {
                                (custom_h - header_overhead).clamp(50.0, (max_height - header_overhead).max(50.0))
                            }
                            None => (max_height - header_overhead).max(60.0),
                        };

                        let mut scroll_area = ScrollArea::vertical()
                            .id_salt("items_drawer_scroll")
                            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
                            .max_height(max_scroll_h)
                            .auto_shrink([false, !is_custom]);

                        if is_custom {
                            scroll_area = scroll_area.min_scrolled_height(max_scroll_h);
                        }

                        scroll_area.show(ui, |ui| {
                            if is_custom {
                                ui.set_min_height(max_scroll_h);
                            }
                            ui.spacing_mut().item_spacing = Vec2::new(0.0, 6.0);

                        // -----------------------------------------------------------------
                        // ACCORDION A: 2D OBJECTS
                        // -----------------------------------------------------------------
                        let entities_matching: Vec<&Entity2dItemInfo> = entities_2d
                            .iter()
                            .filter(|e| query.is_empty() || e.name.to_lowercase().contains(&query))
                            .collect();

                        let obj_chevron = if self.objects_2d_expanded {
                            ICON_KEYBOARD_ARROW_DOWN.codepoint
                        } else {
                            ICON_KEYBOARD_ARROW_RIGHT.codepoint
                        };

                        let header_frame = Frame {
                            inner_margin: Margin::symmetric(8, 6),
                            outer_margin: Margin::ZERO,
                            corner_radius: CornerRadius::same(6),
                            shadow: egui::Shadow::NONE,
                            fill: Color32::from_rgb(30, 33, 42),
                            stroke: Stroke::new(0.5, BORDER_SUBTLE),
                        };

                        let header_resp = header_frame
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(obj_chevron).size(13.0).color(TEXT_SECONDARY),
                                    );
                                    ui.add_space(2.0);
                                    ui.label(
                                        RichText::new("2D OBJECTS")
                                            .size(11.0)
                                            .strong()
                                            .color(TEXT_PRIMARY),
                                    );

                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        let (badge_rect, _) = ui.allocate_exact_size(
                                            Vec2::splat(18.0),
                                            egui::Sense::hover(),
                                        );
                                        ui.painter().circle_filled(
                                            badge_rect.center(),
                                            9.0,
                                            Color32::from_rgb(46, 50, 62),
                                        );
                                        ui.painter().text(
                                            badge_rect.center(),
                                            egui::Align2::CENTER_CENTER,
                                            format!("{}", entities_2d.len()),
                                            egui::FontId::proportional(10.0),
                                            Color32::from_rgb(160, 166, 178),
                                        );
                                    });
                                });
                            })
                            .response;

                        if header_resp.interact(egui::Sense::click()).clicked() {
                            self.objects_2d_expanded = !self.objects_2d_expanded;
                        }

                        if self.objects_2d_expanded {
                            ui.add_space(2.0);
                            if entities_2d.is_empty() {
                                card_frame().show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.vertical_centered(|ui| {
                                        ui.add_space(6.0);
                                        ui.label(
                                            RichText::new(ICON_HORIZONTAL_RULE.codepoint)
                                                .size(20.0)
                                                .color(TEXT_MUTED),
                                        );
                                        ui.label(
                                            RichText::new(t!("drawer-empty-sketches"))
                                                .size(11.5)
                                                .strong()
                                                .color(TEXT_SECONDARY),
                                        );
                                        ui.add_space(6.0);
                                    });
                                });
                            } else {
                                use std::collections::BTreeMap;
                                let mut groups: BTreeMap<String, Vec<&Entity2dItemInfo>> =
                                    BTreeMap::new();
                                let mut ungrouped: Vec<&Entity2dItemInfo> = Vec::new();

                                for e in &entities_matching {
                                    if query.is_empty()
                                        || e.name.to_lowercase().contains(&query)
                                        || e.group_name
                                            .as_ref()
                                            .map(|g| g.to_lowercase().contains(&query))
                                            .unwrap_or(false)
                                    {
                                        match &e.group_name {
                                            Some(g) => groups.entry(g.clone()).or_default().push(e),
                                            None => ungrouped.push(e),
                                        }
                                    }
                                }

                                // Render grup-grup terlebih dahulu
                                for (group_name, members) in &groups {
                                    let is_expanded =
                                        *self.expanded_groups.get(group_name).unwrap_or(&true);
                                    let chevron = if is_expanded {
                                        ICON_KEYBOARD_ARROW_DOWN.codepoint
                                    } else {
                                        ICON_KEYBOARD_ARROW_RIGHT.codepoint
                                    };
                                    let any_selected = members.iter().any(|e| e.selected);
                                    let any_visible = members.iter().any(|e| e.visible);

                                    let group_frame = Frame {
                                        inner_margin: Margin::symmetric(8, 5),
                                        outer_margin: Margin::symmetric(0, 1),
                                        corner_radius: CornerRadius::same(6),
                                        shadow: egui::Shadow::NONE,
                                        fill: if any_selected {
                                            Color32::from_rgb(18, 35, 60)
                                        } else {
                                            Color32::from_rgb(28, 32, 42)
                                        },
                                        stroke: Stroke::new(
                                            if any_selected { 1.0 } else { 0.5 },
                                            if any_selected {
                                                ACCENT_BLUE
                                            } else {
                                                BORDER_SUBTLE
                                            },
                                        ),
                                    };

                                    let group_push_resp = ui.push_id(group_name.as_str(), |ui| {
                                        let mut group_eye_clicked = false;
                                        let mut group_chevron_clicked = false;
                                        let header_output = group_frame.show(ui, |ui| {
                                            ui.set_width(ui.available_width());
                                            ui.horizontal(|ui| {
                                                // Chevron clickable
                                                let (chev_rect, chev_resp) = ui
                                                    .allocate_exact_size(
                                                        Vec2::new(16.0, 20.0),
                                                        egui::Sense::click(),
                                                    );
                                                if chev_resp.hovered() {
                                                    ui.ctx().set_cursor_icon(
                                                        egui::CursorIcon::PointingHand,
                                                    );
                                                }
                                                ui.painter().text(
                                                    chev_rect.center(),
                                                    egui::Align2::CENTER_CENTER,
                                                    chevron,
                                                    egui::FontId::proportional(13.0),
                                                    TEXT_SECONDARY,
                                                );
                                                if chev_resp.clicked() {
                                                    group_chevron_clicked = true;
                                                }
                                                ui.add_space(2.0);

                                                // Tombol eye visibility grup
                                                let (eye_rect, eye_resp) = ui.allocate_exact_size(
                                                    Vec2::new(24.0, 20.0),
                                                    egui::Sense::click(),
                                                );
                                                let is_hovered = eye_resp.hovered();
                                                if is_hovered {
                                                    ui.ctx().set_cursor_icon(
                                                        egui::CursorIcon::PointingHand,
                                                    );
                                                }

                                                let eye_bg = if is_hovered {
                                                    Color32::from_rgb(60, 75, 100)
                                                } else if any_visible {
                                                    Color32::from_rgb(38, 43, 56)
                                                } else {
                                                    Color32::from_rgb(24, 26, 32)
                                                };

                                                let eye_icon = if any_visible {
                                                    ICON_VISIBILITY.codepoint
                                                } else {
                                                    ICON_VISIBILITY_OFF.codepoint
                                                };
                                                let eye_color = if any_visible {
                                                    if any_selected {
                                                        Color32::WHITE
                                                    } else {
                                                        TEXT_PRIMARY
                                                    }
                                                } else {
                                                    TEXT_MUTED
                                                };

                                                ui.painter().rect_filled(
                                                    eye_rect,
                                                    CornerRadius::same(4),
                                                    eye_bg,
                                                );
                                                ui.painter().text(
                                                    eye_rect.center(),
                                                    egui::Align2::CENTER_CENTER,
                                                    eye_icon,
                                                    egui::FontId::proportional(13.0),
                                                    eye_color,
                                                );

                                                if eye_resp.clicked() {
                                                    group_eye_clicked = true;
                                                }

                                                ui.add_space(4.0);

                                                // 2D group icon (ICON_CATEGORY)
                                                ui.label(
                                                    RichText::new(ICON_CATEGORY.codepoint)
                                                        .size(13.0)
                                                        .color(if any_selected {
                                                            ACCENT_BLUE
                                                        } else {
                                                            TEXT_SECONDARY
                                                        }),
                                                );
                                                ui.add_space(6.0);
                                                // Nama grup
                                                ui.label(
                                                    RichText::new(group_name.as_str())
                                                        .size(11.5)
                                                        .strong()
                                                        .color(if any_selected {
                                                            Color32::WHITE
                                                        } else {
                                                            TEXT_PRIMARY
                                                        }),
                                                );
                                                // Badge jumlah anggota (kanan)
                                                ui.with_layout(
                                                    Layout::right_to_left(Align::Center),
                                                    |ui| {
                                                        let (badge_rect, _) = ui
                                                            .allocate_exact_size(
                                                                Vec2::splat(18.0),
                                                                egui::Sense::hover(),
                                                            );
                                                        ui.painter().circle_filled(
                                                            badge_rect.center(),
                                                            9.0,
                                                            Color32::from_rgb(46, 50, 62),
                                                        );
                                                        ui.painter().text(
                                                            badge_rect.center(),
                                                            egui::Align2::CENTER_CENTER,
                                                            format!("{}", members.len()),
                                                            egui::FontId::proportional(10.0),
                                                            Color32::from_rgb(160, 166, 178),
                                                        );
                                                    },
                                                );
                                            });
                                        });

                                        let group_resp = header_output.response;
                                        let group_card_clicked = !group_eye_clicked
                                            && !group_chevron_clicked
                                            && group_resp.interact(egui::Sense::click()).clicked();

                                        (
                                            group_eye_clicked,
                                            group_chevron_clicked,
                                            group_card_clicked,
                                        )
                                    });

                                    let (
                                        group_eye_clicked,
                                        group_chevron_clicked,
                                        group_card_clicked,
                                    ) = group_push_resp.inner;

                                    if group_eye_clicked {
                                        event = Some(ItemsDrawerEvent::ToggleGroupVisibility(
                                            group_name.clone(),
                                        ));
                                    } else if group_chevron_clicked {
                                        event =
                                            Some(ItemsDrawerEvent::ToggleGroup(group_name.clone()));
                                    } else if group_card_clicked {
                                        let extend =
                                            ui.input(|i| i.modifiers.command || i.modifiers.shift);
                                        event = Some(ItemsDrawerEvent::SelectGroup {
                                            name: group_name.clone(),
                                            extend,
                                        });
                                    }

                                    if is_expanded {
                                        // Render anggota grup dengan indentasi dan tombol mata
                                        for e in members {
                                            let (eye_clicked, card_clicked) = render_item_card(
                                                ui, e.id_raw, e.icon, &e.name, e.visible,
                                                e.selected, 16.0, false,
                                            );
                                            if eye_clicked {
                                                event = Some(
                                                    ItemsDrawerEvent::ToggleEntity2dVisibility(
                                                        e.id_raw,
                                                    ),
                                                );
                                            } else if card_clicked {
                                                let extend = ui.input(|i| {
                                                    i.modifiers.command || i.modifiers.shift
                                                });
                                                event = Some(ItemsDrawerEvent::SelectEntity2d {
                                                    id_raw: e.id_raw,
                                                    extend,
                                                });
                                            }
                                        }
                                    }
                                }

                                // Render entitas flat (tanpa grup) dengan tombol mata
                                for e in ungrouped {
                                    let (eye_clicked, card_clicked) = render_item_card(
                                        ui, e.id_raw, e.icon, &e.name, e.visible, e.selected, 0.0,
                                        true,
                                    );
                                    if eye_clicked {
                                        event = Some(ItemsDrawerEvent::ToggleEntity2dVisibility(
                                            e.id_raw,
                                        ));
                                    } else if card_clicked {
                                        let extend =
                                            ui.input(|i| i.modifiers.command || i.modifiers.shift);
                                        event = Some(ItemsDrawerEvent::SelectEntity2d {
                                            id_raw: e.id_raw,
                                            extend,
                                        });
                                    }
                                }
                            }
                        }

                        ui.add_space(4.0);

                        // -----------------------------------------------------------------
                        // ACCORDION B: 3D SOLID BODIES
                        // -----------------------------------------------------------------
                        let bodies_chevron = if self.bodies_expanded {
                            ICON_KEYBOARD_ARROW_DOWN.codepoint
                        } else {
                            ICON_KEYBOARD_ARROW_RIGHT.codepoint
                        };

                        let bodies_header_frame = Frame {
                            inner_margin: Margin::symmetric(8, 6),
                            outer_margin: Margin::ZERO,
                            corner_radius: CornerRadius::same(6),
                            shadow: egui::Shadow::NONE,
                            fill: Color32::from_rgb(30, 33, 42),
                            stroke: Stroke::new(0.5, BORDER_SUBTLE),
                        };

                        let bodies_header_resp = bodies_header_frame
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(bodies_chevron)
                                            .size(13.0)
                                            .color(TEXT_SECONDARY),
                                    );
                                    ui.add_space(2.0);
                                    ui.label(
                                        RichText::new("BODIES")
                                            .size(11.0)
                                            .strong()
                                            .color(TEXT_PRIMARY),
                                    );

                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        // Badge lingkaran sempurna
                                        let (badge_rect, _) = ui.allocate_exact_size(
                                            Vec2::splat(18.0),
                                            egui::Sense::hover(),
                                        );
                                        ui.painter().circle_filled(
                                            badge_rect.center(),
                                            9.0,
                                            Color32::from_rgb(46, 50, 62),
                                        );
                                        ui.painter().text(
                                            badge_rect.center(),
                                            egui::Align2::CENTER_CENTER,
                                            format!("{}", bodies.len()),
                                            egui::FontId::proportional(10.0),
                                            Color32::from_rgb(160, 166, 178),
                                        );
                                    });
                                });
                            })
                            .response;

                        if bodies_header_resp.interact(egui::Sense::click()).clicked() {
                            self.bodies_expanded = !self.bodies_expanded;
                        }

                        if self.bodies_expanded {
                            ui.add_space(2.0);
                            if bodies.is_empty() {
                                card_frame().show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.vertical_centered(|ui| {
                                        ui.add_space(6.0);
                                        ui.label(
                                            RichText::new(ICON_CUBE_OUTLINE.codepoint)
                                                .size(22.0)
                                                .color(TEXT_MUTED),
                                        );
                                        ui.label(
                                            RichText::new(t!("drawer-empty-bodies"))
                                                .size(11.5)
                                                .strong()
                                                .color(TEXT_SECONDARY),
                                        );
                                        ui.add_space(6.0);
                                    });
                                });
                            } else {
                                for b in bodies {
                                    if !query.is_empty() && !b.name.to_lowercase().contains(&query)
                                    {
                                        continue;
                                    }

                                    let (eye_clicked, card_clicked) = render_item_card(
                                        ui,
                                        b.id_raw,
                                        ICON_CUBE_OUTLINE.codepoint,
                                        &b.name,
                                        b.visible,
                                        b.selected,
                                        0.0,
                                        true,
                                    );

                                    if eye_clicked {
                                        event =
                                            Some(ItemsDrawerEvent::ToggleBodyVisibility(b.id_raw));
                                    } else if card_clicked {
                                        let extend =
                                            ui.input(|i| i.modifiers.command || i.modifiers.shift);
                                        event = Some(ItemsDrawerEvent::SelectBody {
                                            id_raw: b.id_raw,
                                            extend,
                                        });
                                    }
                                }
                            }
                        }
                    });
            }
            ItemsDrawerTab::Properties => {
                let is_custom = self.custom_height.is_some();
                let header_overhead = 58.0;
                let max_scroll_h = match self.custom_height {
                    Some(custom_h) => {
                        (custom_h - header_overhead).clamp(50.0, (max_height - header_overhead).max(50.0))
                    }
                    None => (max_height - header_overhead).max(60.0),
                };

                let mut scroll_area = ScrollArea::vertical()
                    .id_salt("items_drawer_props_scroll")
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
                    .max_height(max_scroll_h)
                    .auto_shrink([false, !is_custom]);

                if is_custom {
                    scroll_area = scroll_area.min_scrolled_height(max_scroll_h);
                }

                scroll_area.show(ui, |ui| {
                    if is_custom {
                        ui.set_min_height(max_scroll_h);
                    }
                    if let Some(ref mut vctx) = vector_ctx {
                        if let Some(prop_ev) = vctx.properties_panel.show(
                            ui,
                            vctx.diff,
                            vctx.swatches,
                            vctx.doc_swatches,
                        ) {
                            event = Some(ItemsDrawerEvent::Property(prop_ev));
                        }
                    } else {
                        ui.vertical_centered(|ui| {
                            ui.add_space(28.0);
                            ui.label(
                                RichText::new(ICON_TUNE.codepoint)
                                    .size(24.0)
                                    .color(TEXT_MUTED),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(t!("drawer-vector-sketch-inactive"))
                                    .size(12.0)
                                    .strong()
                                    .color(TEXT_SECONDARY),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(t!("drawer-vector-sketch-inactive-desc"))
                                    .size(10.5)
                                    .color(TEXT_MUTED),
                            );
                        });
                    }
                });
            }
            ItemsDrawerTab::Layers => {
                let is_custom = self.custom_height.is_some();
                let header_overhead = 58.0;
                let max_scroll_h = match self.custom_height {
                    Some(custom_h) => {
                        (custom_h - header_overhead).clamp(50.0, (max_height - header_overhead).max(50.0))
                    }
                    None => (max_height - header_overhead).max(60.0),
                };

                let mut scroll_area = ScrollArea::vertical()
                    .id_salt("items_drawer_layers_scroll")
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
                    .max_height(max_scroll_h)
                    .auto_shrink([false, !is_custom]);

                if is_custom {
                    scroll_area = scroll_area.min_scrolled_height(max_scroll_h);
                }

                scroll_area.show(ui, |ui| {
                    if is_custom {
                        ui.set_min_height(max_scroll_h);
                    }
                    if let Some(ref mut vctx) = vector_ctx {
                        if let Some(layer_ev) = vctx.layers_panel.show(
                            ui,
                            vctx.layer_order_reversed,
                            vctx.active_layer,
                        ) {
                            event = Some(ItemsDrawerEvent::Layer(layer_ev));
                        }
                    } else {
                        ui.vertical_centered(|ui| {
                            ui.add_space(28.0);
                            ui.label(
                                RichText::new(ICON_LAYERS.codepoint)
                                    .size(24.0)
                                    .color(TEXT_MUTED),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(t!("drawer-vector-sketch-inactive"))
                                    .size(12.0)
                                    .strong()
                                    .color(TEXT_SECONDARY),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(t!("drawer-vector-layers-inactive-desc"))
                                    .size(10.5)
                                    .color(TEXT_MUTED),
                            );
                        });
                    }
                });
            }
        }
    });
});

self.last_rendered_height = frame_resp.response.rect.height();

        event
    }
}
