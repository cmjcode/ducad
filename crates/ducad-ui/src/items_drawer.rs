//! Outliner & Scene Properties Panel bergaya Shapr3D dengan Material Icons.
//!
//! Menampilkan panel dock di pojok kanan bawah kanvas untuk navigasi hierarki dokumen:
//! sketsa aktif dan daftar solid body 3D dalam bentuk accordion yang rapi,
//! search bar compact terintegrasi tombol close, badge angka lingkaran sempurna,
//! dan seluruh baris clickable.

use egui::{
    Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, ScrollArea,
    Stroke, Ui, Vec2,
};
use egui_material_icons::icons::{
    ICON_CATEGORY, ICON_CLEAR, ICON_CLOSE, ICON_EDIT, ICON_FOLDER,
    ICON_KEYBOARD_ARROW_DOWN, ICON_KEYBOARD_ARROW_RIGHT, ICON_SEARCH,
    ICON_VISIBILITY, ICON_VISIBILITY_OFF,
};
use crate::theme::{
    card_frame, glass_frame, ACCENT_BLUE, BORDER_SUBTLE,
    TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};

pub struct BodyItemInfo {
    pub id_raw: u64,
    pub name: String,
    pub visible: bool,
    pub selected: bool,
}

pub struct SketchPlaneItemInfo {
    pub index: usize,
    pub name: String,
    pub active: bool,
    pub visible: bool,
}

#[derive(Debug, Clone)]
pub enum ItemsDrawerEvent {
    ToggleBodyVisibility(u64),
    SelectBody { id_raw: u64, extend: bool },
    ToggleSketchVisibility(usize),
    SelectSketchPlane(usize),
    Close,
    Open,
}

pub struct ItemsDrawer {
    pub search_query: String,
    pub sketches_expanded: bool,
    pub bodies_expanded: bool,
}

impl Default for ItemsDrawer {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            sketches_expanded: true,
            bodies_expanded: true,
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
            .on_hover_text("Buka Properties (Sketsa & Solid Body)")
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
        sketch_planes: &[SketchPlaneItemInfo],
        bodies: &[BodyItemInfo],
        max_height: f32,
    ) -> Option<ItemsDrawerEvent> {
        let mut event = None;

        glass_frame().show(ui, |ui| {
            ui.set_width(260.0);
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 6.0);

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
                        .hint_text("Cari sketsa, body 3D…")
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
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height(max_height)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(0.0, 6.0);

                    // -----------------------------------------------------------------
                    // ACCORDION A: SKETCHES
                    // -----------------------------------------------------------------
                    let sketches_matching: Vec<&SketchPlaneItemInfo> = sketch_planes
                        .iter()
                        .filter(|sp| query.is_empty() || sp.name.to_lowercase().contains(&query))
                        .collect();

                    let sketch_chevron = if self.sketches_expanded {
                        ICON_KEYBOARD_ARROW_DOWN.codepoint
                    } else {
                        ICON_KEYBOARD_ARROW_RIGHT.codepoint
                    };

                    // Accordion Header
                    let header_frame = Frame {
                        inner_margin: Margin::symmetric(8, 6),
                        outer_margin: Margin::ZERO,
                        corner_radius: CornerRadius::same(6),
                        shadow: egui::Shadow::NONE,
                        fill: Color32::from_rgb(30, 33, 42),
                        stroke: Stroke::new(0.5, BORDER_SUBTLE),
                    };

                    let header_resp = header_frame.show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{} {}", sketch_chevron, ICON_EDIT.codepoint))
                                    .size(11.5)
                                    .color(ACCENT_BLUE),
                            );
                            ui.label(
                                RichText::new("SKETCHES")
                                    .size(11.0)
                                    .strong()
                                    .color(TEXT_PRIMARY),
                            );

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                // Badge lingkaran sempurna (Perfect circle)
                                let (badge_rect, _) = ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::hover());
                                ui.painter().circle_filled(
                                    badge_rect.center(),
                                    9.0,
                                    Color32::from_rgb(46, 50, 62),
                                );
                                ui.painter().text(
                                    badge_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    format!("{}", sketch_planes.len()),
                                    egui::FontId::proportional(10.0),
                                    Color32::from_rgb(160, 166, 178),
                                );
                            });
                        });
                    }).response;

                    if header_resp.interact(egui::Sense::click()).clicked() {
                        self.sketches_expanded = !self.sketches_expanded;
                    }

                    if self.sketches_expanded {
                        ui.add_space(2.0);
                        for sp in sketches_matching {
                            let is_active = sp.active;
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

                            let row_frame = Frame {
                                inner_margin: Margin::symmetric(8, 6),
                                outer_margin: Margin::symmetric(0, 1),
                                corner_radius: CornerRadius::same(6),
                                shadow: egui::Shadow::NONE,
                                fill: card_bg,
                                stroke: card_stroke,
                            };

                            let mut eye_toggled = false;
                            let card_output = row_frame.show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    // Eye icon
                                    let eye_icon = if sp.visible {
                                        ICON_VISIBILITY.codepoint
                                    } else {
                                        ICON_VISIBILITY_OFF.codepoint
                                    };
                                    let eye_color = if sp.visible {
                                        if is_active { Color32::WHITE } else { TEXT_PRIMARY }
                                    } else {
                                        TEXT_MUTED
                                    };

                                    if ui
                                        .small_button(
                                            RichText::new(eye_icon).size(12.0).color(eye_color),
                                        )
                                        .on_hover_text(if sp.visible {
                                            "Sembunyikan Sketsa"
                                        } else {
                                            "Tampilkan Sketsa"
                                        })
                                        .clicked()
                                    {
                                        eye_toggled = true;
                                    }

                                    // Name with high contrast
                                    let name_color = if is_active {
                                        Color32::WHITE
                                    } else {
                                        TEXT_PRIMARY
                                    };
                                    let name_text = RichText::new(&sp.name)
                                        .size(11.5)
                                        .strong()
                                        .color(name_color);

                                    ui.label(name_text);

                                    // Right-aligned status pill
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if is_active {
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
                                                    RichText::new("Aktif")
                                                        .size(9.5)
                                                        .strong()
                                                        .color(Color32::WHITE),
                                                );
                                            });
                                        }
                                    });
                                });
                            });

                            if eye_toggled {
                                event = Some(ItemsDrawerEvent::ToggleSketchVisibility(sp.index));
                            } else if card_output.response.interact(egui::Sense::click()).clicked() {
                                event = Some(ItemsDrawerEvent::SelectSketchPlane(sp.index));
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

                    let bodies_header_resp = bodies_header_frame.show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{} {}", bodies_chevron, ICON_CATEGORY.codepoint))
                                    .size(11.5)
                                    .color(ACCENT_BLUE),
                            );
                            ui.label(
                                RichText::new("BODIES")
                                    .size(11.0)
                                    .strong()
                                    .color(TEXT_PRIMARY),
                            );

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                // Badge lingkaran sempurna (Perfect circle)
                                let (badge_rect, _) = ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::hover());
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
                    }).response;

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
                                        RichText::new(ICON_CATEGORY.codepoint)
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

                                let mut eye_toggled = false;
                                let card_output = row_frame.show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        let eye = if b.visible {
                                            ICON_VISIBILITY.codepoint
                                        } else {
                                            ICON_VISIBILITY_OFF.codepoint
                                        };
                                        let eye_color = if b.visible {
                                            if is_selected { Color32::WHITE } else { TEXT_PRIMARY }
                                        } else {
                                            TEXT_MUTED
                                        };

                                        if ui
                                            .small_button(
                                                RichText::new(eye).size(12.0).color(eye_color),
                                            )
                                            .on_hover_text(if b.visible {
                                                "Sembunyikan Body"
                                            } else {
                                                "Tampilkan Body"
                                            })
                                            .clicked()
                                        {
                                            eye_toggled = true;
                                        }

                                        let icon_shape = ICON_CATEGORY.codepoint;
                                        ui.label(
                                            RichText::new(icon_shape)
                                                .size(13.0)
                                                .color(if is_selected { ACCENT_BLUE } else { TEXT_SECONDARY }),
                                        );

                                        let name_color = if is_selected {
                                            Color32::WHITE
                                        } else {
                                            TEXT_PRIMARY
                                        };
                                        let text = RichText::new(&b.name)
                                            .strong()
                                            .size(11.5)
                                            .color(name_color);

                                        ui.label(text);

                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
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
                                        });
                                    });
                                });

                                if eye_toggled {
                                    event = Some(
                                        ItemsDrawerEvent::ToggleBodyVisibility(b.id_raw),
                                    );
                                } else if card_output.response.interact(egui::Sense::click()).clicked() {
                                    let extend = ui.input(|i| {
                                        i.modifiers.command || i.modifiers.shift
                                    });
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

        event
    }
}
