//! 2D Text & Emboss/Deboss Tool Popup — Pojok Kanan Bawah.
//!
//! Dialog interaktif pengaturan teks 2D (string, tinggi font, perataan, spasi, custom TTF/OTF)
//! serta aksi langsung pembuatan profil sketsa dan ekstrusi Emboss (timbul) / Deboss (ukir).

use ducad_i18n::t;
use egui::{
    Align, Color32, CornerRadius, DragValue, Layout, RichText, Ui, Vec2,
};
use egui_material_icons::icons::{
    ICON_CHECK, ICON_FOLDER_OPEN, ICON_FORMAT_ALIGN_CENTER, ICON_FORMAT_ALIGN_LEFT,
    ICON_FORMAT_ALIGN_RIGHT, ICON_TITLE,
};

use super::ToolPopupEvent;
use crate::theme::{
    ACCENT_BLUE, ACCENT_ORANGE, TEXT_PRIMARY, TEXT_SECONDARY,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextPopupMode {
    SketchOnly,
    Emboss,
    Deboss,
}

#[derive(Debug, Clone)]
pub struct TextPopupState {
    pub text: String,
    pub font_height_mm: f64,
    pub letter_spacing: f64,
    pub line_spacing: f64,
    pub align: ducad_sketch::TextAlign,
    pub is_construction: bool,
    pub custom_font_name: Option<String>,
    pub mode: TextPopupMode,
    pub emboss_depth: f64,
}

impl Default for TextPopupState {
    fn default() -> Self {
        Self {
            text: "DUCAD".to_string(),
            font_height_mm: 12.0,
            letter_spacing: 1.0,
            line_spacing: 1.2,
            align: ducad_sketch::TextAlign::Left,
            is_construction: false,
            custom_font_name: None,
            mode: TextPopupMode::SketchOnly,
            emboss_depth: 2.0,
        }
    }
}

pub struct TextPopup;

impl TextPopup {
    pub fn show(ui: &mut Ui, state: &mut TextPopupState) -> Option<ToolPopupEvent> {
        let mut event = None;
        let mut apply_clicked = false;
        let mut pick_font_clicked = false;

        const DRAWER_W: f32 = 260.0;
        ui.set_min_width(DRAWER_W);
        ui.set_max_width(DRAWER_W);
        ui.set_width(DRAWER_W);
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 6.0);

        // =========================================================================
        // 1. INPUT STRING TEKS
        // =========================================================================
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{} {}", ICON_TITLE.codepoint, t!("text-input-label")))
                    .strong()
                    .size(11.0)
                    .color(TEXT_PRIMARY),
            );
        });

        ui.add(
            egui::TextEdit::singleline(&mut state.text)
                .hint_text(t!("text-input-hint"))
                .desired_width(DRAWER_W),
        );

        ui.add_space(2.0);

        // =========================================================================
        // 2. DIMENSI TEKS (TINGGI FONT & SPASI)
        // =========================================================================
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(t!("text-font-height"))
                    .size(10.5)
                    .color(TEXT_SECONDARY),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new("mm").size(10.0).color(TEXT_SECONDARY));
                ui.add(
                    DragValue::new(&mut state.font_height_mm)
                        .range(0.5..=500.0)
                        .speed(0.5)
                        .max_decimals(2),
                );
            });
        });

        ui.horizontal(|ui| {
            ui.label(
                RichText::new(t!("text-letter-spacing"))
                    .size(10.5)
                    .color(TEXT_SECONDARY),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new("x").size(10.0).color(TEXT_SECONDARY));
                ui.add(
                    DragValue::new(&mut state.letter_spacing)
                        .range(0.5..=5.0)
                        .speed(0.05)
                        .max_decimals(2),
                );
            });
        });

        // =========================================================================
        // 3. PERATAAN (ALIGNMENT) & FONT PICKER
        // =========================================================================
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(t!("text-align-label"))
                    .size(10.5)
                    .color(TEXT_SECONDARY),
            );

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let is_right = state.align == ducad_sketch::TextAlign::Right;
                if ui
                    .add(
                        egui::Button::new(RichText::new(ICON_FORMAT_ALIGN_RIGHT.codepoint).size(12.0))
                            .fill(if is_right { ACCENT_BLUE } else { Color32::TRANSPARENT }),
                    )
                    .clicked()
                {
                    state.align = ducad_sketch::TextAlign::Right;
                }

                let is_center = state.align == ducad_sketch::TextAlign::Center;
                if ui
                    .add(
                        egui::Button::new(RichText::new(ICON_FORMAT_ALIGN_CENTER.codepoint).size(12.0))
                            .fill(if is_center { ACCENT_BLUE } else { Color32::TRANSPARENT }),
                    )
                    .clicked()
                {
                    state.align = ducad_sketch::TextAlign::Center;
                }

                let is_left = state.align == ducad_sketch::TextAlign::Left;
                if ui
                    .add(
                        egui::Button::new(RichText::new(ICON_FORMAT_ALIGN_LEFT.codepoint).size(12.0))
                            .fill(if is_left { ACCENT_BLUE } else { Color32::TRANSPARENT }),
                    )
                    .clicked()
                {
                    state.align = ducad_sketch::TextAlign::Left;
                }
            });
        });

        // Pilihan Berkas Font Kustom
        ui.horizontal(|ui| {
            let default_font_label = t!("text-font-default");
            let font_label = state
                .custom_font_name
                .as_deref()
                .unwrap_or(&default_font_label);
            ui.label(
                RichText::new(format!("🔤 {}", font_label))
                    .size(10.0)
                    .color(TEXT_SECONDARY),
            );

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(format!("{} {}", ICON_FOLDER_OPEN.codepoint, t!("text-font-browse")))
                                .size(10.0)
                                .color(TEXT_PRIMARY),
                        )
                        .corner_radius(CornerRadius::same(4)),
                    )
                    .clicked()
                {
                    pick_font_clicked = true;
                }
            });
        });

        ui.add_space(2.0);
        ui.separator();
        ui.add_space(2.0);

        // =========================================================================
        // 4. PILIHAN OUTPUT (SKETCH / EMBOSS / DEBOSS)
        // =========================================================================
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(t!("text-output-mode"))
                    .strong()
                    .size(10.5)
                    .color(TEXT_PRIMARY),
            );
        });

        ui.horizontal(|ui| {
            let modes = [
                (TextPopupMode::SketchOnly, t!("text-mode-sketch")),
                (TextPopupMode::Emboss, t!("text-mode-emboss")),
                (TextPopupMode::Deboss, t!("text-mode-deboss")),
            ];

            for (m, label) in modes {
                let is_sel = state.mode == m;
                let bg = if is_sel {
                    if m == TextPopupMode::Deboss {
                        ACCENT_ORANGE
                    } else {
                        ACCENT_BLUE
                    }
                } else {
                    Color32::from_rgba_premultiplied(35, 38, 48, 180)
                };
                let text_color = if is_sel { Color32::WHITE } else { TEXT_SECONDARY };

                if ui
                    .add(
                        egui::Button::new(RichText::new(label).size(10.5).color(text_color))
                            .fill(bg)
                            .corner_radius(CornerRadius::same(5)),
                    )
                    .clicked()
                {
                    state.mode = m;
                }
            }
        });

        if state.mode != TextPopupMode::SketchOnly {
            ui.horizontal(|ui| {
                let depth_label = if state.mode == TextPopupMode::Emboss {
                    t!("text-emboss-height")
                } else {
                    t!("text-deboss-depth")
                };
                ui.label(RichText::new(depth_label).size(10.5).color(TEXT_SECONDARY));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new("mm").size(10.0).color(TEXT_SECONDARY));
                    ui.add(
                        DragValue::new(&mut state.emboss_depth)
                            .range(0.1..=100.0)
                            .speed(0.1)
                            .max_decimals(2),
                    );
                });
            });
        }

        ui.add_space(4.0);

        // =========================================================================
        // 5. TOMBOL AKSI UTAMA (APPLY)
        // =========================================================================
        let (apply_label, apply_color) = match state.mode {
            TextPopupMode::SketchOnly => (t!("text-apply-sketch"), ACCENT_BLUE),
            TextPopupMode::Emboss => (t!("text-apply-emboss"), ACCENT_BLUE),
            TextPopupMode::Deboss => (t!("text-apply-deboss"), ACCENT_ORANGE),
        };

        let apply_btn = ui.add(
            egui::Button::new(
                RichText::new(format!("{} {}", ICON_CHECK.codepoint, apply_label))
                    .size(11.5)
                    .strong()
                    .color(Color32::WHITE),
            )
            .fill(apply_color)
            .corner_radius(CornerRadius::same(6))
            .min_size(Vec2::new(DRAWER_W, 28.0)),
        );

        if apply_btn.clicked() {
            apply_clicked = true;
        }

        if pick_font_clicked {
            event = Some(ToolPopupEvent::PickCustomFont);
        } else if apply_clicked {
            event = Some(ToolPopupEvent::ApplyText {
                text: state.text.clone(),
                font_height_mm: state.font_height_mm,
                letter_spacing: state.letter_spacing,
                align: state.align,
                is_construction: state.is_construction,
                mode: state.mode,
                depth: state.emboss_depth,
            });
        }

        event
    }
}
