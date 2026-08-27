//! Draft Angle Heatmap Inspector Tool Popup — Pojok Kanan Bawah (Menyesuaikan Gaya Drawer & Popups).

use ducad_i18n::t;
use egui::{
    Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, Stroke, Ui, Vec2,
};
use egui_icons::icons::{ICON_ARCHITECTURE, ICON_CLOSE};

use super::ToolPopupEvent;
use crate::theme::{
    glass_frame, ACCENT_BLUE, BORDER_SUBTLE, TEXT_PRIMARY, TEXT_SECONDARY,
};

#[derive(Debug, Clone)]
pub struct DraftPopupState {
    pub pull_dir: [f32; 3],
    pub target_angle_deg: f32,
    pub blend: f32,
}

impl Default for DraftPopupState {
    fn default() -> Self {
        Self {
            pull_dir: [0.0, 0.0, 1.0],
            target_angle_deg: 1.0,
            blend: 1.0,
        }
    }
}

pub struct DraftAnalysisPopup;

impl DraftAnalysisPopup {
    /// Render panel analisis draft di pojok kanan bawah dengan styling konsisten mengikuti History/Items drawer.
    pub fn show(
        ui: &mut Ui,
        state: &mut DraftPopupState,
    ) -> Option<ToolPopupEvent> {
        let mut event = None;
        let mut close_clicked = false;
        let mut changed = false;

        glass_frame().show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                const DRAWER_W: f32 = crate::theme::BOTTOM_RIGHT_PANEL_WIDTH;
                ui.set_min_width(DRAWER_W);
                ui.set_max_width(DRAWER_W);
                ui.set_width(DRAWER_W);
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 5.0);

                // =========================================================================
                // 1. HEADER & CLOSE BUTTON
                // =========================================================================
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "{} {}",
                            ICON_ARCHITECTURE.codepoint,
                            t!("tool-draft-analysis")
                        ))
                        .strong()
                        .size(12.5)
                        .color(ACCENT_BLUE),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button(
                                RichText::new(ICON_CLOSE.codepoint)
                                    .size(12.0)
                                    .color(TEXT_SECONDARY),
                            )
                            .on_hover_text(t!("guide-cancel"))
                            .clicked()
                        {
                            close_clicked = true;
                        }
                    });
                });

                ui.separator();

                // =========================================================================
                // 2. ARAH BUKA CETAKAN (PULL DIRECTION)
                // =========================================================================
                ui.label(
                    RichText::new(t!("draft-pull-dir"))
                        .strong()
                        .size(10.5)
                        .color(TEXT_PRIMARY),
                );

                let dirs = [
                    ("+Z", [0.0, 0.0, 1.0]),
                    ("-Z", [0.0, 0.0, -1.0]),
                    ("+Y", [0.0, 1.0, 0.0]),
                    ("-Y", [0.0, -1.0, 0.0]),
                    ("+X", [1.0, 0.0, 0.0]),
                    ("-X", [-1.0, 0.0, 0.0]),
                ];

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(2.5, 2.5);
                    for (lbl, dir) in dirs {
                        let is_active = (state.pull_dir[0] - dir[0]).abs() < 1e-3
                            && (state.pull_dir[1] - dir[1]).abs() < 1e-3
                            && (state.pull_dir[2] - dir[2]).abs() < 1e-3;

                        let btn_color = if is_active {
                            ACCENT_BLUE
                        } else {
                            Color32::from_rgba_premultiplied(35, 38, 48, 180)
                        };
                        let text_color = if is_active { Color32::WHITE } else { TEXT_SECONDARY };

                        let btn = egui::Button::new(
                            RichText::new(lbl).size(10.0).strong().color(text_color),
                        )
                        .fill(btn_color)
                        .corner_radius(CornerRadius::same(4))
                        .min_size(Vec2::new(31.0, 22.0));

                        if ui.add(btn).clicked() {
                            state.pull_dir = dir;
                            changed = true;
                        }
                    }
                });

                ui.add_space(1.0);
                ui.separator();

                // =========================================================================
                // 3. SUDUT DRAFT TARGET (THRESHOLD) & PRESET
                // =========================================================================
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(t!("draft-target-angle"))
                            .strong()
                            .size(10.5)
                            .color(TEXT_PRIMARY),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let drag = ui.add(
                            egui::DragValue::new(&mut state.target_angle_deg)
                                .range(0.5..=10.0)
                                .speed(0.1)
                                .suffix("°"),
                        );
                        if drag.changed() {
                            changed = true;
                        }
                    });
                });

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 2.0);
                    let presets = [0.5, 1.0, 2.0, 3.0];
                    for &p in &presets {
                        let is_preset_active = (state.target_angle_deg - p).abs() < 1e-2;
                        let bg = if is_preset_active {
                            ACCENT_BLUE
                        } else {
                            Color32::from_rgba_premultiplied(35, 38, 48, 140)
                        };
                        let text_color = if is_preset_active {
                            Color32::WHITE
                        } else {
                            TEXT_SECONDARY
                        };
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(format!("{p:.1}°"))
                                        .size(9.5)
                                        .color(text_color),
                                )
                                .fill(bg)
                                .corner_radius(CornerRadius::same(4))
                                .min_size(Vec2::new(32.0, 18.0)),
                            )
                            .clicked()
                        {
                            state.target_angle_deg = p;
                            changed = true;
                        }
                    }
                });

                ui.add_space(1.0);
                ui.separator();

                // =========================================================================
                // 4. LEGENDA WARNA DFM (HEATMAP ZONES)
                // =========================================================================
                let angle_str = format!("{:.1}", state.target_angle_deg);
                Frame::NONE
                    .fill(Color32::from_rgba_premultiplied(18, 21, 28, 180))
                    .corner_radius(CornerRadius::same(5))
                    .inner_margin(Margin::symmetric(7, 5))
                    .stroke(Stroke::new(0.5, BORDER_SUBTLE))
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 3.5);

                        // 🟢 Aman (Green)
                        ui.horizontal(|ui| {
                            let (r, _) =
                                ui.allocate_exact_size(Vec2::splat(9.0), egui::Sense::hover());
                            ui.painter()
                                .circle_filled(r.center(), 3.5, Color32::from_rgb(46, 204, 113));
                            ui.label(
                                RichText::new(t!("draft-safe-legend", angle = &angle_str))
                                    .size(9.5)
                                    .color(Color32::from_rgb(180, 240, 200)),
                            );
                        });

                        // 🟡 Kritis / Rendah (Yellow)
                        ui.horizontal(|ui| {
                            let (r, _) =
                                ui.allocate_exact_size(Vec2::splat(9.0), egui::Sense::hover());
                            ui.painter()
                                .circle_filled(r.center(), 3.5, Color32::from_rgb(241, 196, 15));
                            ui.label(
                                RichText::new(t!("draft-warning-legend", angle = &angle_str))
                                    .size(9.5)
                                    .color(Color32::from_rgb(255, 235, 160)),
                            );
                        });

                        // 🔴 Undercut / Terjebak (Red)
                        ui.horizontal(|ui| {
                            let (r, _) =
                                ui.allocate_exact_size(Vec2::splat(9.0), egui::Sense::hover());
                            ui.painter()
                                .circle_filled(r.center(), 3.5, Color32::from_rgb(231, 76, 60));
                            ui.label(
                                RichText::new(t!("draft-undercut-legend"))
                                    .size(9.5)
                                    .color(Color32::from_rgb(255, 180, 180)),
                            );
                        });
                    });

                ui.add_space(1.0);
                ui.separator();

                // =========================================================================
                // 5. INTENSITAS BLEND / SHADING
                // =========================================================================
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(t!("draft-blend"))
                            .size(10.0)
                            .color(TEXT_SECONDARY),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let mut blend_pct = (state.blend * 100.0).round();
                        let drag = ui.add(
                            egui::DragValue::new(&mut blend_pct)
                                .range(10.0..=100.0)
                                .speed(1.0)
                                .suffix("%"),
                        );
                        if drag.changed() {
                            state.blend = (blend_pct / 100.0).clamp(0.1, 1.0);
                            changed = true;
                        }
                    });
                });
            });
        });

        if close_clicked {
            event = Some(ToolPopupEvent::CloseDraftInspection);
        } else if changed {
            event = Some(ToolPopupEvent::UpdateDraftInspection {
                pull_dir: state.pull_dir,
                target_angle_deg: state.target_angle_deg,
                blend: state.blend,
            });
        }

        event
    }
}
