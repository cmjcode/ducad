//! 3D Helix / Coil / Spring Tool Popup — Pojok Kanan Bawah.
//!
//! Dialog interaktif pengaturan kurva spiral 3D & generator solid pegas / ulir.

use ducad_i18n::t;
use egui::{
    Color32, CornerRadius, DragValue, RichText, Ui, Vec2,
};
use egui_icons::icons::ICON_CHECK;

use super::ToolPopupEvent;
use crate::theme::{
    ACCENT_BLUE, ACCENT_GREEN, ACCENT_ORANGE, TEXT_PRIMARY, TEXT_SECONDARY,
};

/// Preset cepat untuk konfigurasi Helix / Pegas / Ulir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelixPreset {
    CompressionSpring,
    ExtensionSpring,
    AugerBlade,
    BottleThread,
    Custom,
}

/// Tipe penampang kurva spiral 3D.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelixSectionType {
    RoundWire,
    RectangularBlade,
    TriangularThread,
    CurvePathOnly,
}

#[derive(Debug, Clone)]
pub struct HelixPopupState {
    pub preset: HelixPreset,
    pub pitch: f64,
    pub turns: f64,
    pub radius: f64,
    pub is_taper: bool,
    pub end_radius: f64,
    pub handedness: ducad_kernel::HelixHandedness,
    pub section_type: HelixSectionType,
    pub wire_radius: f64,
    pub rect_width: f64,
    pub rect_height: f64,
    pub tri_width: f64,
    pub tri_height: f64,
    pub custom_height: Option<f32>,
}

impl Default for HelixPopupState {
    fn default() -> Self {
        Self {
            preset: HelixPreset::CompressionSpring,
            pitch: 8.0,
            turns: 6.0,
            radius: 15.0,
            is_taper: false,
            end_radius: 10.0,
            handedness: ducad_kernel::HelixHandedness::RightHand,
            section_type: HelixSectionType::RoundWire,
            wire_radius: 1.5,
            rect_width: 12.0,
            rect_height: 2.5,
            tri_width: 3.0,
            tri_height: 2.0,
            custom_height: None,
        }
    }
}

impl HelixPopupState {
    pub fn apply_preset(&mut self, preset: HelixPreset) {
        self.preset = preset;
        match preset {
            HelixPreset::CompressionSpring => {
                self.pitch = 8.0;
                self.turns = 6.0;
                self.radius = 15.0;
                self.is_taper = false;
                self.section_type = HelixSectionType::RoundWire;
                self.wire_radius = 1.5;
            }
            HelixPreset::ExtensionSpring => {
                self.pitch = 4.0;
                self.turns = 10.0;
                self.radius = 12.0;
                self.is_taper = false;
                self.section_type = HelixSectionType::RoundWire;
                self.wire_radius = 1.2;
            }
            HelixPreset::AugerBlade => {
                self.pitch = 25.0;
                self.turns = 3.0;
                self.radius = 25.0;
                self.is_taper = false;
                self.section_type = HelixSectionType::RectangularBlade;
                self.rect_width = 12.0;
                self.rect_height = 2.5;
            }
            HelixPreset::BottleThread => {
                self.pitch = 4.0;
                self.turns = 2.5;
                self.radius = 14.0;
                self.is_taper = false;
                self.section_type = HelixSectionType::TriangularThread;
                self.tri_width = 3.0;
                self.tri_height = 2.0;
            }
            HelixPreset::Custom => {}
        }
    }

    /// Konversi parameter state menjadi struct HelixParams & profile kernel
    pub fn to_kernel_params(&self, origin: [f64; 3], axis: [f64; 3], start_dir: [f64; 3]) -> (ducad_kernel::HelixParams, Option<ducad_kernel::HelixProfileKind>) {
        let params = ducad_kernel::HelixParams {
            radius: self.radius.max(0.1),
            end_radius: if self.is_taper { Some(self.end_radius.max(0.1)) } else { None },
            pitch: self.pitch.max(0.1),
            turns: self.turns.max(0.1),
            handedness: self.handedness,
            origin,
            axis,
            start_dir,
        };

        let profile = match self.section_type {
            HelixSectionType::RoundWire => Some(ducad_kernel::HelixProfileKind::Circle {
                radius: self.wire_radius.max(0.1),
            }),
            HelixSectionType::RectangularBlade => Some(ducad_kernel::HelixProfileKind::Rectangle {
                width: self.rect_width.max(0.1),
                height: self.rect_height.max(0.1),
            }),
            HelixSectionType::TriangularThread => Some(ducad_kernel::HelixProfileKind::Triangle {
                width: self.tri_width.max(0.1),
                height: self.tri_height.max(0.1),
            }),
            HelixSectionType::CurvePathOnly => None,
        };

        (params, profile)
    }
}

pub struct HelixPopup;

impl HelixPopup {
    pub fn show(ui: &mut Ui, state: &mut HelixPopupState) -> Option<ToolPopupEvent> {
        let mut event = None;
        let mut apply_clicked = false;

        const DRAWER_W: f32 = 230.0;
        ui.set_min_width(DRAWER_W);
        ui.set_max_width(DRAWER_W);
        ui.set_width(DRAWER_W);
        ui.spacing_mut().item_spacing = Vec2::new(3.0, 4.0);

        // =========================================================================
        // 1. PRESET BUTTONS
        // =========================================================================
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(t!("helix-preset-label"))
                    .size(10.0)
                    .color(TEXT_SECONDARY),
            );
        });

        ui.horizontal_wrapped(|ui| {
            let presets = [
                (HelixPreset::CompressionSpring, t!("helix-preset-compression-spring")),
                (HelixPreset::ExtensionSpring, t!("helix-preset-extension-spring")),
                (HelixPreset::AugerBlade, t!("helix-preset-auger")),
                (HelixPreset::BottleThread, t!("helix-preset-thread")),
            ];

            for (p, label) in presets {
                let is_active = state.preset == p;
                let bg = if is_active {
                    ACCENT_BLUE
                } else {
                    Color32::from_rgba_premultiplied(35, 40, 50, 190)
                };
                let fg = if is_active {
                    Color32::WHITE
                } else {
                    TEXT_PRIMARY
                };

                let btn = ui.add(
                    egui::Button::new(RichText::new(label).size(9.5).color(fg))
                        .fill(bg)
                        .corner_radius(CornerRadius::same(4)),
                );
                if btn.clicked() {
                    state.apply_preset(p);
                }
            }
        });

        ui.add_space(2.0);
        ui.separator();

        // =========================================================================
        // 2. PARAMETER KURVA SPIRAL (PITCH, TURNS, RADIUS, TAPER)
        // =========================================================================
        ui.label(
            RichText::new("🌀 Parameter Spiral")
                .strong()
                .size(10.5)
                .color(ACCENT_BLUE),
        );

        egui::Grid::new("helix_params_grid")
            .num_columns(2)
            .spacing([6.0, 4.0])
            .show(ui, |ui| {
                // Pitch
                ui.label(RichText::new(t!("helix-param-pitch")).size(10.0).color(TEXT_SECONDARY));
                ui.horizontal(|ui| {
                    if ui.add(
                        DragValue::new(&mut state.pitch)
                            .speed(0.2)
                            .range(0.5..=200.0)
                            .suffix(" mm"),
                    ).changed() {
                        state.preset = HelixPreset::Custom;
                    }
                });
                ui.end_row();

                // Turns (Jumlah putaran)
                ui.label(RichText::new(t!("helix-param-turns")).size(10.0).color(TEXT_SECONDARY));
                ui.horizontal(|ui| {
                    if ui.add(
                        DragValue::new(&mut state.turns)
                            .speed(0.25)
                            .range(0.25..=100.0)
                            .suffix(" rev"),
                    ).changed() {
                        state.preset = HelixPreset::Custom;
                    }
                });
                ui.end_row();

                // Base Radius
                ui.label(RichText::new(t!("helix-param-radius")).size(10.0).color(TEXT_SECONDARY));
                ui.horizontal(|ui| {
                    if ui.add(
                        DragValue::new(&mut state.radius)
                            .speed(0.5)
                            .range(1.0..=500.0)
                            .suffix(" mm"),
                    ).changed() {
                        state.preset = HelixPreset::Custom;
                    }
                });
                ui.end_row();
            });

        // Taper / Conical Helix Toggle
        ui.horizontal(|ui| {
            let mut taper_chk = state.is_taper;
            if ui.checkbox(&mut taper_chk, RichText::new(t!("helix-param-taper")).size(10.0).color(TEXT_PRIMARY)).changed() {
                state.is_taper = taper_chk;
                state.preset = HelixPreset::Custom;
            }
            if state.is_taper {
                ui.add(
                    DragValue::new(&mut state.end_radius)
                        .speed(0.5)
                        .range(1.0..=500.0)
                        .suffix(" mm"),
                );
            }
        });

        ui.add_space(2.0);
        ui.separator();

        // =========================================================================
        // 3. BENTUK PENAMPANG (PROFILE)
        // =========================================================================
        ui.label(
            RichText::new(format!("📐 {}", t!("helix-param-profile-type")))
                .strong()
                .size(10.5)
                .color(ACCENT_ORANGE),
        );

        ui.horizontal_wrapped(|ui| {
            let profile_types = [
                (HelixSectionType::RoundWire, "● Bulat"),
                (HelixSectionType::RectangularBlade, "■ Bilah"),
                (HelixSectionType::TriangularThread, "▲ Ulir"),
                (HelixSectionType::CurvePathOnly, "〰 Jalur"),
            ];

            for (st, lbl) in profile_types {
                let is_active = state.section_type == st;
                let bg = if is_active {
                    ACCENT_ORANGE
                } else {
                    Color32::from_rgba_premultiplied(35, 40, 50, 190)
                };
                let fg = if is_active {
                    Color32::WHITE
                } else {
                    TEXT_PRIMARY
                };

                let btn = ui.add(
                    egui::Button::new(RichText::new(lbl).size(9.5).color(fg))
                        .fill(bg)
                        .corner_radius(CornerRadius::same(4)),
                );
                if btn.clicked() {
                    state.section_type = st;
                    state.preset = HelixPreset::Custom;
                }
            }
        });

        // Profile dimensions
        match state.section_type {
            HelixSectionType::RoundWire => {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t!("helix-param-wire-radius")).size(10.0).color(TEXT_SECONDARY));
                    ui.add(
                        DragValue::new(&mut state.wire_radius)
                            .speed(0.1)
                            .range(0.2..=(state.pitch * 0.48).max(0.5))
                            .suffix(" mm"),
                    );
                });
            }
            HelixSectionType::RectangularBlade => {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t!("helix-param-width")).size(9.5).color(TEXT_SECONDARY));
                    ui.add(
                        DragValue::new(&mut state.rect_width)
                            .speed(0.2)
                            .range(0.5..=100.0)
                            .suffix(" mm"),
                    );
                    ui.label(RichText::new(t!("helix-param-height")).size(9.5).color(TEXT_SECONDARY));
                    ui.add(
                        DragValue::new(&mut state.rect_height)
                            .speed(0.1)
                            .range(0.5..=(state.pitch * 0.95).max(1.0))
                            .suffix(" mm"),
                    );
                });
            }
            HelixSectionType::TriangularThread => {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t!("helix-param-width")).size(9.5).color(TEXT_SECONDARY));
                    ui.add(
                        DragValue::new(&mut state.tri_width)
                            .speed(0.1)
                            .range(0.5..=50.0)
                            .suffix(" mm"),
                    );
                    ui.label(RichText::new(t!("helix-param-height")).size(9.5).color(TEXT_SECONDARY));
                    ui.add(
                        DragValue::new(&mut state.tri_height)
                            .speed(0.1)
                            .range(0.5..=(state.pitch * 0.95).max(1.0))
                            .suffix(" mm"),
                    );
                });
            }
            HelixSectionType::CurvePathOnly => {
                ui.label(
                    RichText::new("Kurva jalur 3D (spine path) siap dipakai pada tool Sweep")
                        .size(9.0)
                        .color(TEXT_SECONDARY),
                );
            }
        }

        ui.add_space(2.0);
        ui.separator();

        // =========================================================================
        // 4. ARAH PUTARAN (HANDEDNESS) & TOTAL TINGGI
        // =========================================================================
        ui.horizontal(|ui| {
            ui.label(RichText::new(t!("helix-param-handedness")).size(10.0).color(TEXT_SECONDARY));

            let is_rh = state.handedness == ducad_kernel::HelixHandedness::RightHand;
            let (bg_rh, fg_rh) = if is_rh { (ACCENT_BLUE, Color32::WHITE) } else { (Color32::from_rgba_premultiplied(35, 40, 50, 190), TEXT_PRIMARY) };
            let (bg_lh, fg_lh) = if !is_rh { (ACCENT_BLUE, Color32::WHITE) } else { (Color32::from_rgba_premultiplied(35, 40, 50, 190), TEXT_PRIMARY) };

            if ui.add(egui::Button::new(RichText::new("CW Kanan").size(9.0).color(fg_rh)).fill(bg_rh).corner_radius(CornerRadius::same(3))).clicked() {
                state.handedness = ducad_kernel::HelixHandedness::RightHand;
            }
            if ui.add(egui::Button::new(RichText::new("CCW Kiri").size(9.0).color(fg_lh)).fill(bg_lh).corner_radius(CornerRadius::same(3))).clicked() {
                state.handedness = ducad_kernel::HelixHandedness::LeftHand;
            }
        });

        let total_h = state.pitch * state.turns;
        let height_str = format!("{:.1}", total_h);
        ui.label(
            RichText::new(t!("helix-total-height", height = height_str.as_str()))
                .strong()
                .size(10.0)
                .color(ACCENT_GREEN),
        );

        ui.add_space(2.0);

        // =========================================================================
        // 5. TOMBOL EKSEKUSI (APPLY / COMMIT)
        // =========================================================================
        ui.horizontal(|ui| {
            let commit_label = if state.section_type == HelixSectionType::CurvePathOnly {
                format!("{} {}", ICON_CHECK.codepoint, t!("hud-helix-path-btn"))
            } else {
                format!("{} {}", ICON_CHECK.codepoint, t!("hud-helix-exec-btn"))
            };

            let btn = ui.add(
                egui::Button::new(RichText::new(commit_label).size(11.0).strong().color(Color32::WHITE))
                    .fill(ACCENT_GREEN)
                    .corner_radius(CornerRadius::same(5))
                    .min_size(Vec2::new(DRAWER_W - 8.0, 24.0)),
            );
            if btn.clicked() {
                apply_clicked = true;
            }
        });

        // Trigger commit on Enter or click
        if apply_clicked || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let (params, profile) = state.to_kernel_params([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]);
            event = Some(ToolPopupEvent::ApplyHelix { params, profile });
        }

        event
    }
}
