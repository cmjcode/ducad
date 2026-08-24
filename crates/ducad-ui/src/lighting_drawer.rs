//! Studio Lighting & SSAO Presentation Drawer.
//!
//! Menampilkan drawer pengaturan pencahayaan studio 3-titik, SSAO, dan
//! bayangan kontak lantai (Floor Contact Shadow) di pojok kanan bawah kanvas.

use ducad_i18n::t;
use egui::{
    Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, ScrollArea, Slider, Stroke, Ui,
    Vec2,
};
use egui_material_icons::icons::{
    ICON_CLOSE, ICON_LIGHTBULB_ON, ICON_TUNE,
};

use crate::canvas_hud::StudioLightingPresetUi;
use crate::theme::{
    card_frame, glass_frame, ACCENT_BLUE, BORDER_SUBTLE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};

#[derive(Debug, Clone, PartialEq)]
pub enum LightingDrawerEvent {
    SetPreset(StudioLightingPresetUi),
    SetKeyIntensity(f32),
    SetFillIntensity(f32),
    SetRimIntensity(f32),
    SetSsaoIntensity(f32),
    ToggleFloorShadow,
    SetFloorShadowIntensity(f32),
    Close,
}

#[derive(Debug, Clone)]
pub struct LightingDrawer {
    pub custom_height: Option<f32>,
    pub fine_tune_expanded: bool,
}

impl Default for LightingDrawer {
    fn default() -> Self {
        Self {
            custom_height: None,
            fine_tune_expanded: false,
        }
    }
}

impl LightingDrawer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Render panel Studio Lighting di pojok kanan bawah kanvas.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        preset: StudioLightingPresetUi,
        ssao_intensity: f32,
        floor_shadow_enabled: bool,
        floor_shadow_intensity: f32,
        key_intensity: f32,
        fill_intensity: f32,
        rim_intensity: f32,
        max_height: f32,
    ) -> Option<LightingDrawerEvent> {
        let mut event = None;

        glass_frame().show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                const DRAWER_W: f32 = 246.0;
                ui.set_min_width(DRAWER_W);
                ui.set_max_width(DRAWER_W);
                ui.set_width(DRAWER_W);
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);

                let estimated_h = (360.0_f32).clamp(240.0, max_height);
                let panel_h = self.custom_height.unwrap_or(estimated_h);

                // =========================================================================
                // 0. TOP RESIZE HANDLE
                // =========================================================================
                let (handle_rect, handle_resp) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 8.0),
                    egui::Sense::click_and_drag(),
                );
                if handle_resp.hovered() || handle_resp.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                }
                let pill_rect =
                    egui::Rect::from_center_size(handle_rect.center(), Vec2::new(32.0, 3.5));
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
                    let new_h = (cur_h - delta_y).clamp(200.0, max_height);
                    self.custom_height = Some(new_h);
                    ui.ctx().request_repaint();
                }

                if handle_resp.double_clicked() {
                    self.custom_height = None;
                    ui.ctx().request_repaint();
                }

                // =========================================================================
                // 1. HEADER & CLOSE BUTTON
                // =========================================================================
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "{} {}",
                            ICON_LIGHTBULB_ON.codepoint,
                            t!("topbar-studio-lighting")
                        ))
                        .strong()
                        .size(12.5)
                        .color(ACCENT_BLUE),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let close_resp = ui
                            .small_button(
                                RichText::new(ICON_CLOSE.codepoint)
                                    .size(12.0)
                                    .color(TEXT_SECONDARY),
                            )
                            .on_hover_text(t!("guide-cancel"));
                        if close_resp.clicked() {
                            event = Some(LightingDrawerEvent::Close);
                        }
                    });
                });

                ui.separator();

                // =========================================================================
                // 2. SCROLLABLE CONTENT
                // =========================================================================
                ScrollArea::vertical()
                    .max_height((panel_h - 40.0).max(160.0))
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 6.0);

                        // --- Studio Lighting Presets Section ---
                        ui.label(
                            RichText::new(t!("hud-studio-preset"))
                                .size(10.5)
                                .strong()
                                .color(TEXT_PRIMARY),
                        );

                        let presets = [
                            (
                                StudioLightingPresetUi::CleanStudio,
                                t!("hud-studio-clean"),
                                "Daylight",
                            ),
                            (
                                StudioLightingPresetUi::WarmShowcase,
                                t!("hud-studio-warm"),
                                "Warm Gold",
                            ),
                            (
                                StudioLightingPresetUi::CoolTech,
                                t!("hud-studio-cool"),
                                "Cool Cyan",
                            ),
                            (
                                StudioLightingPresetUi::DramaticDark,
                                t!("hud-studio-dramatic"),
                                "Silhouette",
                            ),
                        ];

                        egui::Grid::new("studio_lighting_preset_grid")
                            .num_columns(2)
                            .spacing(Vec2::new(6.0, 6.0))
                            .show(ui, |ui| {
                                for (i, (p_kind, p_label, p_sub)) in presets.iter().enumerate() {
                                    let is_active = preset == *p_kind;
                                    let bg_color = if is_active {
                                        Color32::from_rgba_premultiplied(22, 60, 120, 220)
                                    } else {
                                        Color32::from_rgba_premultiplied(28, 32, 44, 180)
                                    };
                                    let stroke_color = if is_active {
                                        ACCENT_BLUE
                                    } else {
                                        BORDER_SUBTLE
                                    };

                                    let card = Frame {
                                        inner_margin: Margin::symmetric(8, 6),
                                        outer_margin: Margin::ZERO,
                                        corner_radius: CornerRadius::same(6),
                                        shadow: egui::Shadow::NONE,
                                        fill: bg_color,
                                        stroke: Stroke::new(if is_active { 1.2 } else { 0.6 }, stroke_color),
                                    };

                                    let resp = card.show(ui, |ui| {
                                        ui.set_width(108.0);
                                        ui.vertical(|ui| {
                                            ui.horizontal(|ui| {
                                                let dot_color = if is_active {
                                                    ACCENT_BLUE
                                                } else {
                                                    Color32::from_rgb(100, 110, 130)
                                                };
                                                let (r, _) = ui.allocate_exact_size(
                                                    Vec2::splat(6.0),
                                                    egui::Sense::hover(),
                                                );
                                                ui.painter().circle_filled(r.center(), 2.5, dot_color);

                                                ui.label(
                                                    RichText::new(p_label)
                                                        .size(10.0)
                                                        .strong()
                                                        .color(if is_active {
                                                            Color32::WHITE
                                                        } else {
                                                            TEXT_PRIMARY
                                                        }),
                                                );
                                            });
                                            ui.label(
                                                RichText::new(*p_sub)
                                                    .size(8.5)
                                                    .color(if is_active {
                                                        Color32::from_rgb(160, 200, 255)
                                                    } else {
                                                        TEXT_MUTED
                                                    }),
                                            );
                                        });
                                    });

                                    if resp.response.interact(egui::Sense::click()).clicked() {
                                        event = Some(LightingDrawerEvent::SetPreset(*p_kind));
                                    }

                                    if i % 2 == 1 {
                                        ui.end_row();
                                    }
                                }
                            });

                        ui.add_space(2.0);
                        ui.separator();

                        // --- SSAO (Screen Space Ambient Occlusion) Section ---
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(t!("hud-studio-ssao"))
                                    .size(10.0)
                                    .strong()
                                    .color(TEXT_PRIMARY),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let mut ssao_pct = (ssao_intensity * 100.0).round();
                                let drag = ui.add(
                                    egui::DragValue::new(&mut ssao_pct)
                                        .range(0.0..=150.0)
                                        .speed(1.0)
                                        .suffix("%"),
                                );
                                if drag.changed() {
                                    event = Some(LightingDrawerEvent::SetSsaoIntensity(
                                        (ssao_pct / 100.0).clamp(0.0, 1.5),
                                    ));
                                }
                            });
                        });

                        let mut ssao_val = ssao_intensity;
                        let ssao_slider = ui.add(
                            Slider::new(&mut ssao_val, 0.0..=1.5)
                                .show_value(false)
                                .text(""),
                        );
                        if ssao_slider.changed() {
                            event = Some(LightingDrawerEvent::SetSsaoIntensity(ssao_val));
                        }

                        ui.label(
                            RichText::new(t!("hud-studio-ssao-desc"))
                                .size(8.5)
                                .color(TEXT_MUTED),
                        );

                        ui.add_space(2.0);
                        ui.separator();

                        // --- Floor Soft Contact Shadow Section ---
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("⏥ {}", t!("hud-studio-floor-shadow")))
                                    .size(10.0)
                                    .strong()
                                    .color(TEXT_PRIMARY),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let shadow_btn = egui::Button::new(
                                    RichText::new(if floor_shadow_enabled { "ON" } else { "OFF" })
                                        .size(9.5)
                                        .strong()
                                        .color(if floor_shadow_enabled {
                                            Color32::WHITE
                                        } else {
                                            TEXT_SECONDARY
                                        }),
                                )
                                .fill(if floor_shadow_enabled {
                                    ACCENT_BLUE
                                } else {
                                    Color32::from_rgba_premultiplied(35, 38, 48, 180)
                                })
                                .corner_radius(CornerRadius::same(4))
                                .min_size(Vec2::new(38.0, 18.0));

                                if ui.add(shadow_btn).clicked() {
                                    event = Some(LightingDrawerEvent::ToggleFloorShadow);
                                }
                            });
                        });

                        if floor_shadow_enabled {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(t!("hud-studio-shadow-intensity"))
                                        .size(9.5)
                                        .color(TEXT_SECONDARY),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let mut shadow_pct = (floor_shadow_intensity * 100.0).round();
                                    let drag = ui.add(
                                        egui::DragValue::new(&mut shadow_pct)
                                            .range(10.0..=100.0)
                                            .speed(1.0)
                                            .suffix("%"),
                                    );
                                    if drag.changed() {
                                        event = Some(
                                            LightingDrawerEvent::SetFloorShadowIntensity(
                                                (shadow_pct / 100.0).clamp(0.1, 1.0),
                                            ),
                                        );
                                    }
                                });
                            });

                            let mut shadow_val = floor_shadow_intensity;
                            let shadow_slider = ui.add(
                                Slider::new(&mut shadow_val, 0.1..=1.0)
                                    .show_value(false)
                                    .text(""),
                            );
                            if shadow_slider.changed() {
                                event = Some(LightingDrawerEvent::SetFloorShadowIntensity(
                                    shadow_val,
                                ));
                            }
                        }

                        ui.add_space(2.0);
                        ui.separator();

                        // --- 3-Point Light Balance Fine-Tuning ---
                        ui.horizontal(|ui| {
                            let arrow = if self.fine_tune_expanded { "▼" } else { "▶" };
                            let label = ui.selectable_label(
                                self.fine_tune_expanded,
                                RichText::new(format!(
                                    "{} {} {}",
                                    arrow,
                                    ICON_TUNE.codepoint,
                                    t!("hud-studio-lights")
                                ))
                                .size(10.0)
                                .color(if self.fine_tune_expanded {
                                    ACCENT_BLUE
                                } else {
                                    TEXT_SECONDARY
                                }),
                            );
                            if label.clicked() {
                                self.fine_tune_expanded = !self.fine_tune_expanded;
                            }
                        });

                        if self.fine_tune_expanded {
                            card_frame().show(ui, |ui| {
                                ui.spacing_mut().item_spacing = Vec2::new(3.0, 4.0);

                                // Key Light
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(t!("hud-studio-key"))
                                            .size(9.5)
                                            .color(TEXT_SECONDARY),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        let mut k_val = (key_intensity * 100.0).round();
                                        let drag = ui.add(
                                            egui::DragValue::new(&mut k_val)
                                                .range(0.0..=200.0)
                                                .speed(1.0)
                                                .suffix("%"),
                                        );
                                        if drag.changed() {
                                            event = Some(LightingDrawerEvent::SetKeyIntensity(
                                                (k_val / 100.0).clamp(0.0, 2.0),
                                            ));
                                        }
                                    });
                                });

                                // Fill Light
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(t!("hud-studio-fill"))
                                            .size(9.5)
                                            .color(TEXT_SECONDARY),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        let mut f_val = (fill_intensity * 100.0).round();
                                        let drag = ui.add(
                                            egui::DragValue::new(&mut f_val)
                                                .range(0.0..=200.0)
                                                .speed(1.0)
                                                .suffix("%"),
                                        );
                                        if drag.changed() {
                                            event = Some(LightingDrawerEvent::SetFillIntensity(
                                                (f_val / 100.0).clamp(0.0, 2.0),
                                            ));
                                        }
                                    });
                                });

                                // Rim Light
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(t!("hud-studio-rim"))
                                            .size(9.5)
                                            .color(TEXT_SECONDARY),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        let mut r_val = (rim_intensity * 100.0).round();
                                        let drag = ui.add(
                                            egui::DragValue::new(&mut r_val)
                                                .range(0.0..=200.0)
                                                .speed(1.0)
                                                .suffix("%"),
                                        );
                                        if drag.changed() {
                                            event = Some(LightingDrawerEvent::SetRimIntensity(
                                                (r_val / 100.0).clamp(0.0, 2.0),
                                            ));
                                        }
                                    });
                                });
                            });
                        }
                    });
            });
        });

        event
    }
}
