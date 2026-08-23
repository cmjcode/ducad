//! Outliner & Scene Properties Panel bergaya Shapr3D dengan Material Icons.
//!
//! Menampilkan panel dock di pojok kanan bawah kanvas untuk navigasi hierarki dokumen:
//! objek sketsa 2D aktif (garis, lingkaran, busur, elips) dan daftar solid body 3D (BODIES)
//! dalam bentuk accordion yang rapi, search bar compact terintegrasi tombol close,
//! badge angka lingkaran sempurna, dan seluruh baris clickable.

use crate::theme::{
    card_frame, glass_frame, ACCENT_BLUE, BORDER_SUBTLE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};
use egui::{
    Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, ScrollArea, Stroke, Ui, Vec2,
};
use egui_material_icons::icons::{
    ICON_CATEGORY, ICON_CIRCLE, ICON_CLEAR, ICON_CLOSE, ICON_CUBE_OUTLINE, ICON_FOLDER,
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
    pub selected: bool,
    /// Nama grup yang ditetapkan user. `None` = entitas flat tanpa grup.
    pub group_name: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ItemsDrawerEvent {
    ToggleBodyVisibility(u64),
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

        let icon_color = if is_hovered {
            Color32::WHITE
        } else {
            ACCENT_BLUE
        };

        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            ICON_FOLDER.codepoint,
            egui::FontId::proportional(19.0),
            icon_color,
        );

        if resp
            .on_hover_text("Buka Properties (Objek 2D & Solid Body 3D)")
            .clicked()
        {
            event = Some(ItemsDrawerEvent::Open);
        }

        event
    }

    /// Render panel accordion properties di pojok kanan bawah kanvas.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        entities_2d: &[Entity2dItemInfo],
        bodies: &[BodyItemInfo],
        max_height: f32,
        anchor_bottom_y: f32,
    ) -> Option<ItemsDrawerEvent> {
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
                    if let Some(ptr_pos) = ui.input(|i| i.pointer.hover_pos()) {
                        let desired_h = anchor_bottom_y - ptr_pos.y;
                        self.custom_height = Some(desired_h.clamp(120.0, max_height));
                    }
                }

                // =========================================================================
                // 1. SEARCH BAR & CLOSE BUTTON (Compact Single Row, No Folder Icon)
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
                            .hint_text("Cari objek 2D, body 3D…")
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
                        .on_hover_text("Tutup Panel");
                    if close_btn.clicked() {
                        event = Some(ItemsDrawerEvent::Close);
                    }
                });

                ui.separator();

                let query = self.search_query.to_lowercase().trim().to_string();

                // =========================================================================
                // 2. SCROLLABLE ACCORDION SECTIONS
                // =========================================================================
                let (scroll_h, auto_shrink_v) = if let Some(ch) = self.custom_height {
                    ((ch - 55.0).clamp(60.0, max_height - 55.0), false)
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
                                    RichText::new("2D OBJECTS")
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
                                        RichText::new("Belum ada objek 2D")
                                            .size(11.5)
                                            .strong()
                                            .color(TEXT_SECONDARY),
                                    );
                                    ui.label(
                                        RichText::new("Gunakan Line, Circle, atau Arc di toolbar")
                                            .size(9.5)
                                            .color(TEXT_MUTED),
                                    );
                                    ui.add_space(6.0);
                                });
                            });
                        } else {
                            // Pisahkan entitas menjadi: bernama grup vs flat
                            // Kumpulkan grup unik (urutan stabil via BTreeMap)
                            use std::collections::BTreeMap;
                            let mut groups: BTreeMap<String, Vec<&Entity2dItemInfo>> = BTreeMap::new();
                            let mut ungrouped: Vec<&Entity2dItemInfo> = Vec::new();

                            for e in &entities_matching {
                                if query.is_empty() || e.name.to_lowercase().contains(&query)
                                    || e.group_name.as_ref().map(|g| g.to_lowercase().contains(&query)).unwrap_or(false)
                                {
                                    match &e.group_name {
                                        Some(g) => groups.entry(g.clone()).or_default().push(e),
                                        None => ungrouped.push(e),
                                    }
                                }
                            }

                            // Render grup-grup terlebih dahulu
                            for (group_name, members) in &groups {
                                let is_expanded = *self.expanded_groups.get(group_name).unwrap_or(&true);
                                let chevron = if is_expanded {
                                    ICON_KEYBOARD_ARROW_DOWN.codepoint
                                } else {
                                    ICON_KEYBOARD_ARROW_RIGHT.codepoint
                                };
                                let any_selected = members.iter().any(|e| e.selected);

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
                                        if any_selected { ACCENT_BLUE } else { BORDER_SUBTLE },
                                    ),
                                };

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
                                        // 2D group icon (ICON_CATEGORY)
                                        ui.label(
                                            RichText::new(ICON_CATEGORY.codepoint)
                                                .size(13.0)
                                                .color(if any_selected { ACCENT_BLUE } else { TEXT_SECONDARY }),
                                        );
                                        ui.add_space(6.0);
                                        // Nama grup
                                        ui.label(
                                            RichText::new(group_name.as_str())
                                                .size(11.5)
                                                .strong()
                                                .color(if any_selected { Color32::WHITE } else { TEXT_PRIMARY }),
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
                                    });
                                });

                                if header_output.response.interact(egui::Sense::click()).clicked() {
                                    event = Some(ItemsDrawerEvent::ToggleGroup(group_name.clone()));
                                }

                                if is_expanded {
                                    // Render anggota grup dengan indentasi
                                    for e in members {
                                        ui.push_id(e.id_raw, |ui| {
                                            ui.horizontal(|ui| {
                                                // Indentasi
                                                ui.add_space(16.0);
                                                // Render kartu entitas
                                                let is_selected = e.selected;
                                                let card_bg = if is_selected {
                                                    Color32::from_rgb(18, 38, 68)
                                                } else {
                                                    Color32::from_rgb(22, 25, 33)
                                                };
                                                let card_stroke = if is_selected {
                                                    Stroke::new(1.0, ACCENT_BLUE)
                                                } else {
                                                    Stroke::new(0.5, BORDER_SUBTLE)
                                                };

                                                let row_frame = Frame {
                                                    inner_margin: Margin::symmetric(6, 5),
                                                    outer_margin: Margin::symmetric(0, 1),
                                                    corner_radius: CornerRadius::same(5),
                                                    shadow: egui::Shadow::NONE,
                                                    fill: card_bg,
                                                    stroke: card_stroke,
                                                };

                                                let card_output = row_frame.show(ui, |ui| {
                                                    ui.set_width(ui.available_width());
                                                    ui.horizontal(|ui| {
                                                        ui.label(RichText::new(e.icon).size(12.0).color(
                                                            if is_selected { ACCENT_BLUE } else { TEXT_SECONDARY },
                                                        ));
                                                        ui.add_space(6.0);
                                                        ui.label(
                                                            RichText::new(&e.name)
                                                                .size(11.0)
                                                                .color(if is_selected { Color32::WHITE } else { TEXT_PRIMARY }),
                                                        );
                                                    });
                                                });

                                                if card_output.response.interact(egui::Sense::click()).clicked() {
                                                    let extend = ui.input(|i| i.modifiers.command || i.modifiers.shift);
                                                    event = Some(ItemsDrawerEvent::SelectEntity2d {
                                                        id_raw: e.id_raw,
                                                        extend,
                                                    });
                                                }
                                            });
                                        });
                                    }
                                }
                            }

                            // Render entitas flat (tanpa grup)
                            for e in ungrouped {
                                ui.push_id(e.id_raw, |ui| {
                                    let is_selected = e.selected;
                                    let card_bg = if is_selected {
                                        Color32::from_rgb(18, 38, 68)
                                    } else {
                                        Color32::from_rgb(26, 29, 36)
                                    };
                                    let card_stroke = if is_selected {
                                        Stroke::new(1.0, ACCENT_BLUE)
                                    } else {
                                        Stroke::new(0.5, BORDER_SUBTLE)
                                    };

                                    let row_frame = Frame {
                                        inner_margin: Margin::symmetric(8, 6),
                                        outer_margin: Margin::symmetric(0, 1),
                                        corner_radius: CornerRadius::same(6),
                                        shadow: egui::Shadow::NONE,
                                        fill: card_bg,
                                        stroke: card_stroke,
                                    };

                                    let card_output = row_frame.show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(e.icon).size(13.0).color(
                                                if is_selected {
                                                    ACCENT_BLUE
                                                } else {
                                                    TEXT_SECONDARY
                                                },
                                            ));
                                            ui.add_space(6.0);

                                            let name_color = if is_selected {
                                                Color32::WHITE
                                            } else {
                                                TEXT_PRIMARY
                                            };
                                            let label_text = RichText::new(&e.name)
                                                .strong()
                                                .size(11.5)
                                                .color(name_color);

                                            ui.label(label_text);

                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    if is_selected {
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
                                                    }
                                                },
                                            );
                                        });
                                    });

                                    if card_output
                                        .response
                                        .interact(egui::Sense::click())
                                        .clicked()
                                    {
                                        let extend =
                                            ui.input(|i| i.modifiers.command || i.modifiers.shift);
                                        event = Some(ItemsDrawerEvent::SelectEntity2d {
                                            id_raw: e.id_raw,
                                            extend,
                                        });
                                    }
                                });
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
                                        RichText::new("Belum ada body 3D")
                                            .size(11.5)
                                            .strong()
                                            .color(TEXT_SECONDARY),
                                    );
                                    ui.label(
                                        RichText::new("Gunakan Extrude, Revolve, atau Loft")
                                            .size(9.5)
                                            .color(TEXT_MUTED),
                                    );
                                    ui.add_space(6.0);
                                });
                            });
                        } else {
                            for b in bodies {
                                if !query.is_empty() && !b.name.to_lowercase().contains(&query) {
                                    continue;
                                }

                                // Render kartu item body
                                let push_id_resp = ui.push_id(b.id_raw, |ui| {
                                    let is_selected = b.selected;
                                    let card_bg = if is_selected {
                                        Color32::from_rgb(18, 38, 68)
                                    } else {
                                        Color32::from_rgb(26, 29, 36)
                                    };
                                    let card_stroke = if is_selected {
                                        Stroke::new(1.0, ACCENT_BLUE)
                                    } else {
                                        Stroke::new(0.5, BORDER_SUBTLE)
                                    };

                                    let row_frame = Frame {
                                        inner_margin: Margin::symmetric(8, 6),
                                        outer_margin: Margin::symmetric(0, 1),
                                        corner_radius: CornerRadius::same(6),
                                        shadow: egui::Shadow::NONE,
                                        fill: card_bg,
                                        stroke: card_stroke,
                                    };

                                    // Render card body — eye button di luar row_frame agar tidak tertutup
                                    let card_resp = row_frame.show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            // Reserve space untuk eye button
                                            let (eye_rect, _) = ui.allocate_exact_size(
                                                Vec2::new(24.0, 20.0),
                                                egui::Sense::focusable_noninteractive(),
                                            );

                                            let eye_icon = if b.visible {
                                                ICON_VISIBILITY.codepoint
                                            } else {
                                                ICON_VISIBILITY_OFF.codepoint
                                            };
                                            let eye_color = if b.visible {
                                                if is_selected {
                                                    Color32::WHITE
                                                } else {
                                                    TEXT_PRIMARY
                                                }
                                            } else {
                                                TEXT_MUTED
                                            };

                                            // Draw background dan icon via painter (painter dari layer ini)
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

                                            let icon_shape = ICON_CUBE_OUTLINE.codepoint;
                                            ui.label(RichText::new(icon_shape).size(13.0).color(
                                                if is_selected {
                                                    ACCENT_BLUE
                                                } else {
                                                    TEXT_SECONDARY
                                                },
                                            ));
                                            ui.add_space(6.0);

                                            let name_color = if is_selected {
                                                Color32::WHITE
                                            } else {
                                                TEXT_PRIMARY
                                            };
                                            ui.label(
                                                RichText::new(&b.name)
                                                    .strong()
                                                    .size(11.5)
                                                    .color(name_color),
                                            );

                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    if is_selected {
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
                                                    }
                                                },
                                            );

                                            eye_rect
                                        })
                                        .inner
                                    });

                                    // Eye button — pakai ctx.layer_painter di layer Foreground terpisah
                                    // agar tidak terhalang frame atau widget lain
                                    let eye_rect_screen = card_resp.inner;

                                    // Gambar ulang eye di atas dengan Layer Debug untuk memastikan posisi
                                    let eye_layer = egui::LayerId::new(
                                        egui::Order::Foreground,
                                        ui.id().with("eye_overlay").with(b.id_raw),
                                    );
                                    let painter = ui.ctx().layer_painter(eye_layer);
                                    let ptr = ui.ctx().pointer_interact_pos();
                                    let is_hovered =
                                        ptr.map(|p| eye_rect_screen.contains(p)).unwrap_or(false);

                                    let eye_bg = if is_hovered {
                                        Color32::from_rgb(60, 75, 100)
                                    } else if b.visible {
                                        Color32::from_rgb(38, 43, 56)
                                    } else {
                                        Color32::from_rgb(24, 26, 32)
                                    };

                                    let eye_icon = if b.visible {
                                        ICON_VISIBILITY.codepoint
                                    } else {
                                        ICON_VISIBILITY_OFF.codepoint
                                    };
                                    let eye_color = if b.visible {
                                        if is_selected {
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

                                    // Detect klik pada eye rect — pakai input pointer events langsung
                                    let eye_clicked = ui.ctx().input(|i| {
                                        is_hovered
                                            && i.pointer.primary_clicked()
                                            && i.pointer
                                                .interact_pos()
                                                .map(|p| eye_rect_screen.contains(p))
                                                .unwrap_or(false)
                                    });

                                    if eye_clicked {
                                        println!(
                                            "[DUCAD UI] KLIK TERDETEKSI pada icon mata body='{}'",
                                            b.name
                                        );
                                    }

                                    let card_clicked = !is_hovered
                                        && card_resp
                                            .response
                                            .interact(egui::Sense::click())
                                            .clicked();

                                    (eye_clicked, card_clicked)
                                });

                                let (eye_clicked, card_clicked) = push_id_resp.inner;

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
