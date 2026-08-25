//! Hole Wizard Tool Popup — Pojok Kanan Bawah.
//!
//! Dialog interaktif pembuatan lubang standar ISO (Simple, Counterbore, Countersink, Tapped)
//! dengan tabel ukuran metrik M2-M12 dan penyesuaian dimensi parametrik.

use ducad_core::hole::{HoleKind, HoleSpec, IsoMetricThread};
use ducad_i18n::t;
use egui::{
    Align, Color32, CornerRadius, DragValue, Frame, Layout, Margin, RichText, Stroke, Ui, Vec2,
};
use egui_material_icons::icons::{ICON_ADJUST, ICON_CHECK, ICON_CLOSE};

use super::ToolPopupEvent;
use crate::theme::{
    glass_frame, ACCENT_BLUE, ACCENT_ORANGE, BORDER_SUBTLE, TEXT_PRIMARY, TEXT_SECONDARY,
};

#[derive(Debug, Clone)]
pub struct HolePopupState {
    pub spec: HoleSpec,
    /// Pergeseran posisi lubang dari titik referensi face sepanjang sumbu U (mm).
    pub offset_u: f64,
    /// Pergeseran posisi lubang dari titik referensi face sepanjang sumbu V (mm).
    pub offset_v: f64,
    /// Posisi 3D absolut titik lubang saat ini (x, y, z).
    pub current_pos_3d: Option<(f64, f64, f64)>,
    /// Apakah pengguna sedang men-drag titik lubang secara interaktif.
    pub is_dragging: bool,
}

impl Default for HolePopupState {
    fn default() -> Self {
        Self {
            spec: HoleSpec::default(),
            offset_u: 0.0,
            offset_v: 0.0,
            current_pos_3d: None,
            is_dragging: false,
        }
    }
}

pub struct HolePopup;

impl HolePopup {
    /// Render panel Hole Wizard di pojok kanan bawah.
    pub fn show(ui: &mut Ui, state: &mut HolePopupState) -> Option<ToolPopupEvent> {
        let mut event = None;
        let mut close_clicked = false;
        let mut apply_clicked = false;

        glass_frame().show(ui, |ui| {
            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                const DRAWER_W: f32 = 270.0;
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
                            ICON_ADJUST.codepoint,
                            t!("tool-hole-wizard")
                        ))
                        .strong()
                        .size(12.5)
                        .color(ACCENT_BLUE),
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
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
                // 2. TIPE LUBANG (HOLE TYPE TABS)
                // =========================================================================
                ui.label(
                    RichText::new(t!("hole-type"))
                        .strong()
                        .size(10.5)
                        .color(TEXT_PRIMARY),
                );

                let kinds = [
                    (HoleKind::Simple, "Simple"),
                    (HoleKind::Counterbore, "C-Bore"),
                    (HoleKind::Countersink, "C-Sink"),
                    (HoleKind::Tapped, "Tapped"),
                ];

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(3.0, 2.0);
                    for (k, lbl) in kinds {
                        let is_active = state.spec.kind == k;
                        let bg = if is_active {
                            ACCENT_BLUE
                        } else {
                            Color32::from_rgba_premultiplied(35, 38, 48, 180)
                        };
                        let text_color = if is_active {
                            Color32::WHITE
                        } else {
                            TEXT_SECONDARY
                        };

                        let btn = egui::Button::new(
                            RichText::new(lbl).size(10.0).strong().color(text_color),
                        )
                        .fill(bg)
                        .corner_radius(CornerRadius::same(4))
                        .min_size(Vec2::new(60.0, 22.0));

                        if ui.add(btn).clicked() {
                            let curr_depth = state.spec.depth;
                            state.spec = HoleSpec::for_iso(state.spec.thread_size, k, curr_depth);
                        }
                    }
                });

                ui.add_space(2.0);
                ui.separator();

                // =========================================================================
                // 3. STANDAR BAUT METRIK ISO (PRESET SELECTOR)
                // =========================================================================
                ui.label(
                    RichText::new(t!("hole-iso-standard"))
                        .strong()
                        .size(10.5)
                        .color(TEXT_PRIMARY),
                );

                let threads = [
                    IsoMetricThread::M2,
                    IsoMetricThread::M2_5,
                    IsoMetricThread::M3,
                    IsoMetricThread::M4,
                    IsoMetricThread::M5,
                    IsoMetricThread::M6,
                    IsoMetricThread::M8,
                    IsoMetricThread::M10,
                    IsoMetricThread::M12,
                ];

                // Baris 1: M2, M2.5, M3, M4, M5
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(2.5, 2.0);
                    for &th in &threads[..5] {
                        let is_active = state.spec.thread_size == th;
                        let bg = if is_active {
                            ACCENT_ORANGE
                        } else {
                            Color32::from_rgba_premultiplied(35, 38, 48, 140)
                        };
                        let text_color = if is_active {
                            Color32::WHITE
                        } else {
                            TEXT_SECONDARY
                        };

                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(th.label())
                                        .size(9.5)
                                        .strong()
                                        .color(text_color),
                                )
                                .fill(bg)
                                .corner_radius(CornerRadius::same(4))
                                .min_size(Vec2::new(48.0, 20.0)),
                            )
                            .clicked()
                        {
                            let curr_depth = state.spec.depth;
                            let is_through = state.spec.is_through;
                            let has_drill_tip = state.spec.has_drill_tip;
                            state.spec = HoleSpec::for_iso(th, state.spec.kind, curr_depth);
                            state.spec.is_through = is_through;
                            state.spec.has_drill_tip = has_drill_tip;
                        }
                    }
                });

                // Baris 2: M6, M8, M10, M12
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(2.5, 2.0);
                    for &th in &threads[5..] {
                        let is_active = state.spec.thread_size == th;
                        let bg = if is_active {
                            ACCENT_ORANGE
                        } else {
                            Color32::from_rgba_premultiplied(35, 38, 48, 140)
                        };
                        let text_color = if is_active {
                            Color32::WHITE
                        } else {
                            TEXT_SECONDARY
                        };

                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(th.label())
                                        .size(9.5)
                                        .strong()
                                        .color(text_color),
                                )
                                .fill(bg)
                                .corner_radius(CornerRadius::same(4))
                                .min_size(Vec2::new(61.0, 20.0)),
                            )
                            .clicked()
                        {
                            let curr_depth = state.spec.depth;
                            let is_through = state.spec.is_through;
                            let has_drill_tip = state.spec.has_drill_tip;
                            state.spec = HoleSpec::for_iso(th, state.spec.kind, curr_depth);
                            state.spec.is_through = is_through;
                            state.spec.has_drill_tip = has_drill_tip;
                        }
                    }
                });

                ui.add_space(2.0);

                // Info Callout Box Standar ISO
                Frame::NONE
                    .fill(Color32::from_rgba_premultiplied(18, 22, 30, 200))
                    .corner_radius(CornerRadius::same(5))
                    .inner_margin(Margin::symmetric(7, 5))
                    .stroke(Stroke::new(0.5, BORDER_SUBTLE))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("🏷")
                                    .size(10.0),
                            );
                            ui.label(
                                RichText::new(state.spec.technical_callout())
                                    .size(9.5)
                                    .strong()
                                    .color(Color32::from_rgb(180, 225, 255)),
                            );
                        });
                    });

                ui.add_space(2.0);
                ui.separator();

                // =========================================================================
                // 4. PARAMETER DIMENSI GEOMETRI
                // =========================================================================
                // Diameter Lubang
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(t!("hole-dia"))
                            .size(10.5)
                            .color(TEXT_PRIMARY),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add(
                            DragValue::new(&mut state.spec.diameter)
                                .range(0.5..=100.0)
                                .speed(0.1)
                                .suffix(" mm"),
                        );
                    });
                });

                // Kedalaman & Opsi Through All
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(t!("hole-depth"))
                            .size(10.5)
                            .color(TEXT_PRIMARY),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if !state.spec.is_through {
                            ui.add(
                                DragValue::new(&mut state.spec.depth)
                                    .range(1.0..=500.0)
                                    .speed(0.5)
                                    .suffix(" mm"),
                            );
                        } else {
                            ui.label(
                                RichText::new(t!("hole-through-all"))
                                    .size(10.0)
                                    .color(ACCENT_BLUE),
                            );
                        }
                    });
                });

                // Tembus / Through All Toggle
                ui.horizontal(|ui| {
                    ui.checkbox(&mut state.spec.is_through, RichText::new(t!("hole-through-all")).size(10.0).color(TEXT_SECONDARY));
                });

                // Parameter khusus Counterbore
                if state.spec.kind == HoleKind::Counterbore {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(t!("hole-cbore-dia"))
                                .size(10.0)
                                .color(TEXT_SECONDARY),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.add(
                                DragValue::new(&mut state.spec.counterbore_diameter)
                                    .range(1.0..=120.0)
                                    .speed(0.1)
                                    .suffix(" mm"),
                            );
                        });
                    });

                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(t!("hole-cbore-depth"))
                                .size(10.0)
                                .color(TEXT_SECONDARY),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.add(
                                DragValue::new(&mut state.spec.counterbore_depth)
                                    .range(0.5..=100.0)
                                    .speed(0.1)
                                    .suffix(" mm"),
                            );
                        });
                    });
                }

                // Parameter khusus Countersink
                if state.spec.kind == HoleKind::Countersink {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(t!("hole-csink-dia"))
                                .size(10.0)
                                .color(TEXT_SECONDARY),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.add(
                                DragValue::new(&mut state.spec.countersink_diameter)
                                    .range(1.0..=120.0)
                                    .speed(0.1)
                                    .suffix(" mm"),
                            );
                        });
                    });

                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(t!("hole-csink-angle"))
                                .size(10.0)
                                .color(TEXT_SECONDARY),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.add(
                                DragValue::new(&mut state.spec.countersink_angle_deg)
                                    .range(30.0..=150.0)
                                    .speed(1.0)
                                    .suffix("°"),
                            );
                        });
                    });
                }

                // Parameter khusus Tapped Thread
                if state.spec.kind == HoleKind::Tapped {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(t!("hole-thread-pitch"))
                                .size(10.0)
                                .color(TEXT_SECONDARY),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.add(
                                DragValue::new(&mut state.spec.thread_pitch)
                                    .range(0.2..=5.0)
                                    .speed(0.05)
                                    .suffix(" mm"),
                            );
                        });
                    });

                    if !state.spec.is_through {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(t!("hole-thread-depth"))
                                    .size(10.0)
                                    .color(TEXT_SECONDARY),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.add(
                                    DragValue::new(&mut state.spec.thread_depth)
                                        .range(1.0..=500.0)
                                        .speed(0.5)
                                        .suffix(" mm"),
                                );
                            });
                        });
                    }
                }

                // Opsi Ujung Bor 118° (hanya jika blind)
                if !state.spec.is_through {
                    ui.horizontal(|ui| {
                        ui.checkbox(
                            &mut state.spec.has_drill_tip,
                            RichText::new(t!("hole-drill-tip"))
                                .size(10.0)
                                .color(TEXT_SECONDARY),
                        );
                    });
                }

                ui.add_space(2.0);
                ui.separator();

                // =========================================================================
                // 5. POSISI & GESER LUBANG (PLACEMENT & OFFSET)
                // =========================================================================
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Posisi & Geser (Offset):")
                            .strong()
                            .size(10.5)
                            .color(TEXT_PRIMARY),
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .button(
                                RichText::new("Pusat (Center)")
                                    .size(9.0)
                                    .color(Color32::from_rgb(180, 225, 255)),
                            )
                            .on_hover_text("Kembalikan titik lubang ke tengah permukaan (centroid)")
                            .clicked()
                        {
                            state.offset_u = 0.0;
                            state.offset_v = 0.0;
                            state.current_pos_3d = None;
                        }
                    });
                });

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Offset U (X):")
                            .size(10.0)
                            .color(TEXT_SECONDARY),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add(
                            DragValue::new(&mut state.offset_u)
                                .speed(0.2)
                                .suffix(" mm"),
                        );
                    });
                });

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Offset V (Y):")
                            .size(10.0)
                            .color(TEXT_SECONDARY),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add(
                            DragValue::new(&mut state.offset_v)
                                .speed(0.2)
                                .suffix(" mm"),
                        );
                    });
                });

                ui.label(
                    RichText::new("💡 Klik atau drag titik target di viewport untuk menggeser.")
                        .italics()
                        .size(9.0)
                        .color(Color32::from_rgb(150, 190, 220)),
                );

                ui.add_space(3.0);
                ui.separator();
                ui.add_space(2.0);

                // =========================================================================
                // 6. TOMBOL TERAPKAN LUBANG (APPLY BUTTON)
                // =========================================================================
                let apply_btn = egui::Button::new(
                    RichText::new(format!(
                        "{} {}",
                        ICON_CHECK.codepoint,
                        t!("hole-apply")
                    ))
                    .size(11.5)
                    .strong()
                    .color(Color32::WHITE),
                )
                .fill(ACCENT_BLUE)
                .corner_radius(CornerRadius::same(5))
                .min_size(Vec2::new(DRAWER_W, 26.0));

                if ui.add(apply_btn).clicked() {
                    apply_clicked = true;
                }
            });
        });

        if close_clicked {
            event = Some(ToolPopupEvent::Close);
        } else if apply_clicked {
            event = Some(ToolPopupEvent::ApplyHole(state.spec));
        }

        event
    }
}
