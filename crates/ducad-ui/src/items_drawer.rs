//! Outliner & Scene Properties Panel bergaya Shapr3D dengan Material Icons.
//!
//! Menampilkan panel dock di pojok kanan bawah kanvas untuk navigasi hierarki dokumen:
//! objek sketsa 2D aktif (garis, lingkaran, busur, elips) dan daftar solid body 3D (BODIES)
//! dalam bentuk accordion yang rapi, search bar compact terintegrasi tombol close,
//! badge angka lingkaran sempurna, dan seluruh baris clickable.

use crate::theme::{
    card_frame, glass_frame, ACCENT_BLUE, BORDER_SUBTLE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};
use ducad_i18n::t;
use egui::{
    Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, ScrollArea, Stroke, Ui, Vec2,
};
use egui_material_icons::icons::{
    ICON_CATEGORY, ICON_CLEAR, ICON_CLOSE, ICON_CUBE_OUTLINE, ICON_FOLDER,
    ICON_HORIZONTAL_RULE, ICON_KEYBOARD_ARROW_DOWN, ICON_KEYBOARD_ARROW_RIGHT, ICON_SEARCH,
    ICON_VISIBILITY, ICON_VISIBILITY_OFF,
};

pub struct BodyItemInfo {
    pub id_raw: u64,
    pub name: String,
    pub visible: bool,
    pub selected: bool,
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

#[derive(Debug, Clone)]
pub enum ItemsDrawerEvent {
    ToggleBodyVisibility(u64),
    ToggleEntity2dVisibility(u64),
    ToggleGroupVisibility(String),
    SelectBody { id_raw: u64, extend: bool },
    SelectEntity2d { id_raw: u64, extend: bool },
    Close,
    Open,
    ToggleGroup(String),
}

pub struct ItemsDrawer {
    pub search_query: String,
    pub objects_2d_expanded: bool,
    pub bodies_expanded: bool,
    pub custom_height: Option<f32>,
    /// Expanded state per nama grup 2D.
    pub expanded_groups: std::collections::HashMap<String, bool>,
}

impl Default for ItemsDrawer {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            objects_2d_expanded: true,
            bodies_expanded: true,
            custom_height: None,
            expanded_groups: std::collections::HashMap::new(),
        }
    }
}

/// Helper untuk merender kartu item dengan tombol eye visibility di sisi kiri
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

        let card_output = ui.horizontal(|ui| {
            if indent > 0.0 {
                ui.add_space(indent);
            }

            let row_resp = row_frame.show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    // Reserve space untuk eye button
                    let (eye_rect, _) = ui.allocate_exact_size(
                        Vec2::new(24.0, 20.0),
                        egui::Sense::focusable_noninteractive(),
                    );

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

                    // Draw background dan icon placeholder via painter
                    ui.painter().rect_filled(
                        eye_rect,
                        CornerRadius::same(4),
                        Color32::from_rgb(38, 43, 56),
                    );
                    ui.painter().text(
                        eye_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        eye_icon,
                        egui::FontId::proportional(13.0),
                        eye_color,
                    );

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
                    ui.label(
                        RichText::new(name)
                            .strong()
                            .size(11.5)
                            .color(name_color),
                    );

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

                    eye_rect
                })
                .inner
            });

            (row_resp.response, row_resp.inner)
        });

        let (row_response, eye_rect_screen) = card_output.inner;

        // Overlay layer Foreground untuk tombol mata interaktif
        let eye_layer = egui::LayerId::new(
            egui::Order::Foreground,
            ui.id().with("eye_overlay").with(id_raw),
        );
        let painter = ui.ctx().layer_painter(eye_layer);
        let ptr = ui.ctx().pointer_interact_pos();
        let is_hovered = ptr.map(|p| eye_rect_screen.contains(p)).unwrap_or(false);

        let eye_bg = if is_hovered {
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

        painter.rect_filled(
            eye_rect_screen,
            CornerRadius::same(4),
            eye_bg,
        );
        painter.text(
            eye_rect_screen.center(),
            egui::Align2::CENTER_CENTER,
            eye_icon,
            egui::FontId::proportional(13.0),
            eye_color,
        );

        if is_hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        let eye_clicked = ui.ctx().input(|i| {
            is_hovered
                && i.pointer.primary_clicked()
                && i.pointer
                    .interact_pos()
                    .map(|p| eye_rect_screen.contains(p))
                    .unwrap_or(false)
        });

        let card_clicked = visible
            && !is_hovered
            && row_response
                .interact(egui::Sense::click())
                .clicked();

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

    /// Render panel pohon objek (accordion 2D Entities + 3D Bodies).
    pub fn show(
        &mut self,
        ui: &mut Ui,
        entities_2d: &[Entity2dItemInfo],
        bodies: &[BodyItemInfo],
        max_height: f32,
        _anchor_bottom_y: f32,
    ) -> Option<ItemsDrawerEvent> {
        let mut event = None;

        // Hitung perkiraan tinggi konten
        let query = self.search_query.trim().to_lowercase();
        let entities_count = entities_2d
            .iter()
            .filter(|e| query.is_empty() || e.name.to_lowercase().contains(&query))
            .count();
        let bodies_count = bodies
            .iter()
            .filter(|b| query.is_empty() || b.name.to_lowercase().contains(&query))
            .count();

        // 56px header search + padding + 36px accordion header + item heights
        let mut estimated_h: f32 = 72.0;
        if self.objects_2d_expanded {
            estimated_h += 38.0 + (entities_count.max(1) as f32 * 36.0);
        } else {
            estimated_h += 38.0;
        }
        if self.bodies_expanded {
            estimated_h += 38.0 + (bodies_count.max(1) as f32 * 36.0);
        } else {
            estimated_h += 38.0;
        }

        let panel_h = estimated_h.clamp(140.0, max_height);
        let auto_shrink_v = estimated_h < max_height;

        let frame = glass_frame()
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::same(10));

        frame.show(ui, |ui| {
            ui.set_min_width(232.0);
            ui.set_max_width(232.0);
            ui.set_width(232.0);
            if !auto_shrink_v {
                ui.set_height(panel_h);
            }

            ui.vertical(|ui| {
                // -------------------------------------------------------------
                // SEARCH BAR COMPACT DENGAN TOMBOL CLOSE
                // -------------------------------------------------------------
                let search_box_frame = Frame {
                    inner_margin: Margin::symmetric(8, 5),
                    outer_margin: Margin::ZERO,
                    corner_radius: CornerRadius::same(6),
                    shadow: egui::Shadow::NONE,
                    fill: Color32::from_rgb(22, 25, 33),
                    stroke: Stroke::new(0.5, BORDER_SUBTLE),
                };

                search_box_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(ICON_SEARCH.codepoint)
                                .size(13.0)
                                .color(TEXT_MUTED),
                        );
                        ui.add_space(2.0);

                        let text_edit = egui::TextEdit::singleline(&mut self.search_query)
                            .hint_text(RichText::new(t!("drawer-search-placeholder")).size(11.0).color(TEXT_MUTED))
                            .text_color(TEXT_PRIMARY)
                            .font(egui::TextStyle::Body)
                            .desired_width(ui.available_width() - 44.0);

                        ui.add(text_edit);

                        if !self.search_query.is_empty() {
                            let clear_resp = ui.add(
                                egui::Button::new(
                                    RichText::new(ICON_CLEAR.codepoint)
                                        .size(12.0)
                                        .color(TEXT_MUTED),
                                )
                                .frame(false),
                            );
                            if clear_resp.clicked() {
                                self.search_query.clear();
                            }
                        }

                        // Tombol close terintegrasi di pojok kanan search bar
                        let close_btn = ui.add(
                            egui::Button::new(
                                RichText::new(ICON_CLOSE.codepoint)
                                    .size(13.0)
                                    .color(TEXT_SECONDARY),
                            )
                            .frame(false),
                        );
                        if close_btn.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if close_btn.clicked() {
                            event = Some(ItemsDrawerEvent::Close);
                        }
                    });
                });

                ui.add_space(6.0);

                // -------------------------------------------------------------
                // SCROLL AREA KONTEN
                // -------------------------------------------------------------
                let scroll_h = (panel_h - 60.0).max(80.0);
                let mut scroll_area = ScrollArea::vertical()
                    .id_salt("items_drawer_scroll")
                    .auto_shrink([false, auto_shrink_v]);

                if !auto_shrink_v {
                    scroll_area = scroll_area.max_height(scroll_h);
                }

                scroll_area.show(ui, |ui| {
                    if !auto_shrink_v {
                        ui.set_min_height(scroll_h);
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
                                    RichText::new(obj_chevron)
                                        .size(13.0)
                                        .color(TEXT_SECONDARY),
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
                            let mut groups: BTreeMap<String, Vec<&Entity2dItemInfo>> = BTreeMap::new();
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
                                    let header_output = group_frame.show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            // Chevron
                                            ui.label(
                                                RichText::new(chevron)
                                                    .size(13.0)
                                                    .color(TEXT_SECONDARY),
                                            );
                                            ui.add_space(4.0);

                                            // Reserve space untuk eye button grup
                                            let (eye_rect, _) = ui.allocate_exact_size(
                                                Vec2::new(24.0, 20.0),
                                                egui::Sense::focusable_noninteractive(),
                                            );

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
                                                Color32::from_rgb(38, 43, 56),
                                            );
                                            ui.painter().text(
                                                eye_rect.center(),
                                                egui::Align2::CENTER_CENTER,
                                                eye_icon,
                                                egui::FontId::proportional(13.0),
                                                eye_color,
                                            );

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
                                                    format!("{}", members.len()),
                                                    egui::FontId::proportional(10.0),
                                                    Color32::from_rgb(160, 166, 178),
                                                );
                                            });

                                            eye_rect
                                        })
                                        .inner
                                    });

                                    let group_resp = header_output.response;
                                    let eye_rect_screen = header_output.inner;

                                    let eye_layer = egui::LayerId::new(
                                        egui::Order::Foreground,
                                        ui.id().with("group_eye_overlay").with(group_name.as_str()),
                                    );
                                    let painter = ui.ctx().layer_painter(eye_layer);
                                    let ptr = ui.ctx().pointer_interact_pos();
                                    let is_hovered = ptr
                                        .map(|p| eye_rect_screen.contains(p))
                                        .unwrap_or(false);

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

                                    painter.rect_filled(
                                        eye_rect_screen,
                                        CornerRadius::same(4),
                                        eye_bg,
                                    );
                                    painter.text(
                                        eye_rect_screen.center(),
                                        egui::Align2::CENTER_CENTER,
                                        eye_icon,
                                        egui::FontId::proportional(13.0),
                                        eye_color,
                                    );

                                    if is_hovered {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }

                                    let eye_clicked = ui.ctx().input(|i| {
                                        is_hovered
                                            && i.pointer.primary_clicked()
                                            && i.pointer
                                                .interact_pos()
                                                .map(|p| eye_rect_screen.contains(p))
                                                .unwrap_or(false)
                                    });

                                    let card_clicked = !is_hovered
                                        && group_resp
                                            .interact(egui::Sense::click())
                                            .clicked();

                                    (eye_clicked, card_clicked)
                                });

                                let (group_eye_clicked, group_card_clicked) = group_push_resp.inner;

                                if group_eye_clicked {
                                    event = Some(ItemsDrawerEvent::ToggleGroupVisibility(
                                        group_name.clone(),
                                    ));
                                } else if group_card_clicked {
                                    event = Some(ItemsDrawerEvent::ToggleGroup(group_name.clone()));
                                }

                                if is_expanded {
                                    // Render anggota grup dengan indentasi dan tombol mata
                                    for e in members {
                                        let (eye_clicked, card_clicked) = render_item_card(
                                            ui,
                                            e.id_raw,
                                            e.icon,
                                            &e.name,
                                            e.visible,
                                            e.selected,
                                            16.0,
                                            false,
                                        );
                                        if eye_clicked {
                                            event = Some(
                                                ItemsDrawerEvent::ToggleEntity2dVisibility(e.id_raw),
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
                                    ui,
                                    e.id_raw,
                                    e.icon,
                                    &e.name,
                                    e.visible,
                                    e.selected,
                                    0.0,
                                    true,
                                );
                                if eye_clicked {
                                    event =
                                        Some(ItemsDrawerEvent::ToggleEntity2dVisibility(e.id_raw));
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
                                if !query.is_empty() && !b.name.to_lowercase().contains(&query) {
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
                                    event = Some(ItemsDrawerEvent::ToggleBodyVisibility(b.id_raw));
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
            });
        });

        event
    }
}
