//! Preset Material Industri & CMF (Color, Material, Finish) Drawer.
//!
//! Menampilkan drawer properti material interaktif di pojok kanan bawah
//! yang mengubah tampilan visual objek 3D seketika secara real-time.

use ducad_core::{Material, MaterialPreset};
use ducad_i18n::t;
use egui::{
    Color32, CornerRadius, Frame, Margin, RichText, ScrollArea, Slider, Stroke, Ui, Vec2,
};
use egui_icons::icons::{
    ICON_AUTO_AWESOME, ICON_CLOSE, ICON_PALETTE, ICON_SHIELD, ICON_TEXTURE, ICON_WATER_DROP,
};

use crate::theme::{
    glass_frame, ACCENT_BLUE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};

#[derive(Debug, Clone, PartialEq)]
pub enum CmfDrawerEvent {
    SetMaterial(Material),
    Close,
}

#[derive(Debug, Clone)]
pub struct CmfDrawer {
    pub custom_height: Option<f32>,
    pub fine_tune_expanded: bool,
}

impl Default for CmfDrawer {
    fn default() -> Self {
        Self {
            custom_height: None,
            fine_tune_expanded: false,
        }
    }
}

impl CmfDrawer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Render panel CMF di pojok kanan bawah kanvas.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        current_material: &Material,
        selected_body_name: Option<&str>,
        selected_count: usize,
        max_height: f32,
    ) -> Option<CmfDrawerEvent> {
        let mut event = None;
        let mut mat = *current_material;
        let mut mat_changed = false;

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
                            ICON_PALETTE.codepoint,
                            t!("inspector-cmf-title")
                        ))
                        .strong()
                        .size(12.0)
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
                            event = Some(CmfDrawerEvent::Close);
                        }

                        // Badge nama objek / jumlah seleksi
                        let badge_text = if let Some(name) = selected_body_name {
                            name.to_string()
                        } else if selected_count > 1 {
                            format!("{} Bodies", selected_count)
                        } else {
                            "1 Body".to_string()
                        };
                        let badge_frame = Frame {
                            inner_margin: Margin::symmetric(5, 2),
                            outer_margin: Margin::ZERO,
                            corner_radius: CornerRadius::same(4),
                            shadow: egui::Shadow::NONE,
                            fill: Color32::from_rgb(22, 38, 64),
                            stroke: Stroke::new(0.5, ACCENT_BLUE),
                        };
                        badge_frame.show(ui, |ui| {
                            ui.label(
                                RichText::new(badge_text)
                                    .size(9.0)
                                    .color(Color32::from_rgb(180, 220, 255)),
                            );
                        });
                    });
                });

                ui.add_space(2.0);
                ui.separator();

                // =========================================================================
                // 2. SCROLL CONTENT (Presets, Swatches, Fine-Tuning)
                // =========================================================================
                let scroll_height = (panel_h - 48.0).max(120.0);
                ScrollArea::vertical()
                    .id_salt("cmf_drawer_scroll")
                    .max_height(scroll_height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 5.0);

                        // -------------------------------------------------------------
                        // 1. INDUSTRIAL PRESET BUTTONS
                        // -------------------------------------------------------------
                        ui.label(
                            RichText::new(t!("inspector-cmf-presets"))
                                .size(10.0)
                                .color(TEXT_SECONDARY),
                        );

                        let presets = [
                            (
                                MaterialPreset::MattePlastic,
                                t!("material-matte-plastic"),
                                "ABS / PC",
                                ICON_TEXTURE.codepoint,
                            ),
                            (
                                MaterialPreset::GlossyPlastic,
                                t!("material-glossy-plastic"),
                                "High-Gloss",
                                ICON_AUTO_AWESOME.codepoint,
                            ),
                            (
                                MaterialPreset::AnodizedAluminum,
                                t!("material-anodized-aluminum"),
                                "Satin / Brushed",
                                ICON_SHIELD.codepoint,
                            ),
                            (
                                MaterialPreset::PolishedChrome,
                                t!("material-polished-chrome"),
                                "Mirror Finish",
                                ICON_AUTO_AWESOME.codepoint,
                            ),
                            (
                                MaterialPreset::TranslucentGlass,
                                t!("material-translucent-glass"),
                                "Clear Acrylic",
                                ICON_WATER_DROP.codepoint,
                            ),
                        ];

                        for (preset, name, desc, icon) in presets {
                            let is_selected = mat.preset == preset;
                            let bg_color = if is_selected {
                                Color32::from_rgb(26, 52, 92)
                            } else {
                                Color32::from_rgb(22, 26, 34)
                            };
                            let stroke_color = if is_selected {
                                ACCENT_BLUE
                            } else {
                                Color32::from_rgb(42, 48, 60)
                            };

                            let card_frame = Frame {
                                inner_margin: Margin::symmetric(8, 5),
                                outer_margin: Margin::symmetric(0, 1),
                                corner_radius: CornerRadius::same(5),
                                shadow: egui::Shadow::NONE,
                                fill: bg_color,
                                stroke: Stroke::new(if is_selected { 1.2 } else { 0.5 }, stroke_color),
                            };

                            let resp = card_frame
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(icon)
                                                .size(13.0)
                                                .color(if is_selected {
                                                    ACCENT_BLUE
                                                } else {
                                                    TEXT_SECONDARY
                                                }),
                                        );
                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(name)
                                                    .size(10.5)
                                                    .strong()
                                                    .color(if is_selected {
                                                        TEXT_PRIMARY
                                                    } else {
                                                        TEXT_SECONDARY
                                                    }),
                                            );
                                            ui.label(
                                                RichText::new(desc)
                                                    .size(8.5)
                                                    .color(TEXT_MUTED),
                                            );
                                        });
                                    });
                                })
                                .response;

                            if resp.interact(egui::Sense::click()).clicked() {
                                mat = match preset {
                                    MaterialPreset::MattePlastic => {
                                        Material::matte_plastic(Some(mat.base_color))
                                    }
                                    MaterialPreset::GlossyPlastic => {
                                        Material::glossy_plastic(Some(mat.base_color))
                                    }
                                    MaterialPreset::AnodizedAluminum => {
                                        Material::anodized_aluminum(Some(mat.base_color))
                                    }
                                    MaterialPreset::PolishedChrome => {
                                        Material::polished_chrome(Some(mat.base_color))
                                    }
                                    MaterialPreset::TranslucentGlass => {
                                        Material::translucent_glass(Some(mat.base_color))
                                    }
                                    _ => mat,
                                };
                                mat_changed = true;
                            }
                        }

                        ui.add_space(3.0);
                        ui.separator();

                        // -------------------------------------------------------------
                        // 2. INDUSTRIAL COLOR PALETTE SWATCHES & CUSTOM TINT
                        // -------------------------------------------------------------
                        ui.label(
                            RichText::new(t!("inspector-cmf-color"))
                                .size(10.0)
                                .color(TEXT_SECONDARY),
                        );

                        let swatches: &[([f32; 4], &str)] = match mat.preset {
                            MaterialPreset::MattePlastic => &[
                                ([0.22, 0.24, 0.27, 1.0], "Stealth Charcoal"),
                                ([0.55, 0.58, 0.62, 1.0], "Industrial Slate"),
                                ([0.88, 0.90, 0.92, 1.0], "Pure White"),
                                ([0.18, 0.32, 0.48, 1.0], "Nordic Blue"),
                                ([0.32, 0.40, 0.28, 1.0], "Olive Green"),
                            ],
                            MaterialPreset::GlossyPlastic => &[
                                ([0.08, 0.08, 0.10, 1.0], "Piano Black"),
                                ([0.96, 0.38, 0.12, 1.0], "Signal Orange"),
                                ([0.92, 0.18, 0.18, 1.0], "Racing Red"),
                                ([0.96, 0.82, 0.10, 1.0], "Cyber Yellow"),
                                ([0.98, 0.98, 0.98, 1.0], "Ceramic White"),
                            ],
                            MaterialPreset::AnodizedAluminum => &[
                                ([0.72, 0.75, 0.80, 1.0], "Space Gray"),
                                ([0.88, 0.90, 0.92, 1.0], "Satin Silver"),
                                ([0.22, 0.35, 0.52, 1.0], "Midnight Blue"),
                                ([0.85, 0.78, 0.65, 1.0], "Champagne Gold"),
                                ([0.82, 0.65, 0.68, 1.0], "Rose Titanium"),
                            ],
                            MaterialPreset::PolishedChrome => &[
                                ([0.92, 0.94, 0.96, 1.0], "Mirror Chrome"),
                                ([0.75, 0.78, 0.82, 1.0], "Stainless Steel"),
                                ([0.45, 0.48, 0.52, 1.0], "Gunmetal"),
                                ([0.82, 0.68, 0.48, 1.0], "Polished Bronze"),
                            ],
                            MaterialPreset::TranslucentGlass => &[
                                ([0.75, 0.88, 0.96, 0.38], "Clear Ice Glass"),
                                ([0.28, 0.30, 0.35, 0.45], "Smoky Gray Glass"),
                                ([0.25, 0.82, 0.88, 0.38], "Cyan Tint Glass"),
                                ([0.28, 0.78, 0.48, 0.38], "Emerald Glass"),
                                ([0.88, 0.22, 0.32, 0.38], "Ruby Glass"),
                            ],
                            _ => &[
                                ([0.62, 0.68, 0.76, 1.0], "CAD Grey"),
                                ([0.20, 0.65, 0.95, 1.0], "Accent Blue"),
                                ([0.95, 0.40, 0.15, 1.0], "Accent Orange"),
                                ([0.22, 0.24, 0.27, 1.0], "Stealth Charcoal"),
                                ([0.88, 0.90, 0.92, 1.0], "Pure White"),
                            ],
                        };

                        ui.horizontal(|ui| {
                            for &(col, label) in swatches {
                                let c32 = Color32::from_rgba_unmultiplied(
                                    (col[0] * 255.0) as u8,
                                    (col[1] * 255.0) as u8,
                                    (col[2] * 255.0) as u8,
                                    (col[3] * 255.0) as u8,
                                );
                                let (rect, resp) =
                                    ui.allocate_exact_size(Vec2::new(20.0, 20.0), egui::Sense::click());
                                let is_active = (mat.base_color[0] - col[0]).abs() < 0.05
                                    && (mat.base_color[1] - col[1]).abs() < 0.05
                                    && (mat.base_color[2] - col[2]).abs() < 0.05;

                                ui.painter().circle_filled(rect.center(), 8.5, c32);
                                ui.painter().circle_stroke(
                                    rect.center(),
                                    8.5,
                                    Stroke::new(
                                        if is_active { 1.8 } else { 0.8 },
                                        if is_active {
                                            ACCENT_BLUE
                                        } else {
                                            Color32::from_rgb(65, 72, 85)
                                        },
                                    ),
                                );

                                if resp.on_hover_text(label).clicked() {
                                    mat.base_color = col;
                                    mat_changed = true;
                                }
                            }

                            // Custom Color Picker Button
                            if ui.color_edit_button_rgba_unmultiplied(&mut mat.base_color).changed()
                            {
                                mat.preset = MaterialPreset::Custom;
                                mat_changed = true;
                            }
                        });

                        ui.add_space(3.0);
                        ui.separator();

                        // -------------------------------------------------------------
                        // 3. FINE-TUNING SLIDERS (Roughness, Metallic, Clearcoat, Opacity)
                        // -------------------------------------------------------------
                        let chevron = if self.fine_tune_expanded { "▼" } else { "▶" };
                        let ft_header_resp = ui.button(
                            RichText::new(format!("{} ⚙ {}", chevron, t!("inspector-cmf-fine-tune")))
                                .size(10.0)
                                .color(TEXT_SECONDARY),
                        );
                        if ft_header_resp.clicked() {
                            self.fine_tune_expanded = !self.fine_tune_expanded;
                        }

                        if self.fine_tune_expanded {
                            ui.indent("cmf_fine_tune_indent", |ui| {
                                ui.spacing_mut().item_spacing = Vec2::new(0.0, 4.0);

                                // Roughness
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(t!("material-roughness"))
                                            .size(9.5)
                                            .color(TEXT_SECONDARY),
                                    );
                                    let r_slider = ui.add(
                                        Slider::new(&mut mat.roughness, 0.02..=1.0)
                                            .show_value(true)
                                            .fixed_decimals(2),
                                    );
                                    if r_slider.changed() {
                                        mat.preset = MaterialPreset::Custom;
                                        mat_changed = true;
                                    }
                                });

                                // Metallic
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(t!("material-metallic"))
                                            .size(9.5)
                                            .color(TEXT_SECONDARY),
                                    );
                                    let m_slider = ui.add(
                                        Slider::new(&mut mat.metallic, 0.0..=1.0)
                                            .show_value(true)
                                            .fixed_decimals(2),
                                    );
                                    if m_slider.changed() {
                                        mat.preset = MaterialPreset::Custom;
                                        mat_changed = true;
                                    }
                                });

                                // Clearcoat
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(t!("material-clearcoat"))
                                            .size(9.5)
                                            .color(TEXT_SECONDARY),
                                    );
                                    let c_slider = ui.add(
                                        Slider::new(&mut mat.clearcoat, 0.0..=1.0)
                                            .show_value(true)
                                            .fixed_decimals(2),
                                    );
                                    if c_slider.changed() {
                                        mat.preset = MaterialPreset::Custom;
                                        mat_changed = true;
                                    }
                                });

                                // Opacity / Alpha
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(t!("material-opacity"))
                                            .size(9.5)
                                            .color(TEXT_SECONDARY),
                                    );
                                    let mut alpha = mat.base_color[3];
                                    let a_slider = ui.add(
                                        Slider::new(&mut alpha, 0.05..=1.0)
                                            .show_value(true)
                                            .fixed_decimals(2),
                                    );
                                    if a_slider.changed() {
                                        mat.base_color[3] = alpha;
                                        mat.preset = MaterialPreset::Custom;
                                        mat_changed = true;
                                    }
                                });
                            });
                        }
                    });
            });
        });

        if mat_changed {
            event = Some(CmfDrawerEvent::SetMaterial(mat));
        }

        event
    }
}
