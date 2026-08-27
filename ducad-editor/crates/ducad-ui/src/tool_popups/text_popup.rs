//! 2D Text Tool Popup — Pojok Kanan Bawah.
//!
//! Dialog interaktif pengaturan teks 2D (string, font family, tinggi font, perataan, spasi, custom TTF/OTF).

use ducad_i18n::t;
use egui::{
    Align, Color32, CornerRadius, DragValue, Layout, RichText, Ui, Vec2,
};
use egui_icons::icons::{
    ICON_CHECK, ICON_FOLDER_OPEN, ICON_FORMAT_ALIGN_CENTER, ICON_FORMAT_ALIGN_LEFT,
    ICON_FORMAT_ALIGN_RIGHT, ICON_TITLE,
};

use super::ToolPopupEvent;
use crate::theme::{
    ACCENT_BLUE, TEXT_PRIMARY, TEXT_SECONDARY,
};

#[derive(Debug, Clone)]
pub struct TextPopupState {
    pub text: String,
    pub font_height_mm: f64,
    pub letter_spacing: f64,
    pub line_spacing: f64,
    pub align: ducad_sketch::TextAlign,
    pub font_preset: ducad_sketch::FontPreset,
    pub is_construction: bool,
    pub custom_font_name: Option<String>,
}

impl Default for TextPopupState {
    fn default() -> Self {
        Self {
            text: "DUCAD".to_string(),
            font_height_mm: 12.0,
            letter_spacing: 1.0,
            line_spacing: 1.2,
            align: ducad_sketch::TextAlign::Left,
            font_preset: ducad_sketch::FontPreset::Arial,
            is_construction: false,
            custom_font_name: None,
        }
    }
}

pub struct TextPopup;

impl TextPopup {
    pub fn show(ui: &mut Ui, state: &mut TextPopupState) -> Option<ToolPopupEvent> {
        let mut event = None;
        let mut apply_clicked = false;
        let mut pick_font_clicked = false;

        const DRAWER_W: f32 = crate::theme::BOTTOM_RIGHT_PANEL_WIDTH;
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
        // 2. PILIHAN FONT FAMILY (COMBOBOX LIVE)
        // =========================================================================
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("🔤 Font:")
                    .size(10.5)
                    .color(TEXT_SECONDARY),
            );

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add(
                        egui::Button::new(RichText::new(ICON_FOLDER_OPEN.codepoint).size(12.0))
                            .corner_radius(CornerRadius::same(4)),
                    )
                    .on_hover_text(t!("text-font-browse"))
                    .clicked()
                {
                    pick_font_clicked = true;
                }

                let current_label = if let Some(custom) = &state.custom_font_name {
                    custom.as_str()
                } else {
                    state.font_preset.display_name()
                };

                egui::ComboBox::from_id_salt("text_font_family_combo")
                    .selected_text(RichText::new(current_label).size(11.0))
                    .width(140.0)
                    .show_ui(ui, |ui| {
                        for preset in ducad_sketch::FontPreset::all() {
                            if ui
                                .selectable_value(&mut state.font_preset, *preset, preset.display_name())
                                .clicked()
                            {
                                state.custom_font_name = None;
                            }
                        }
                    });
            });
        });

        ui.add_space(2.0);

        // =========================================================================
        // 3. DIMENSI TEKS (TINGGI FONT & SPASI)
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
        // 4. PERATAAN (ALIGNMENT)
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

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        // =========================================================================
        // 5. TOMBOL AKSI UTAMA (PLACE ON SKETCH)
        // =========================================================================
        let apply_btn = ui.add(
            egui::Button::new(
                RichText::new(format!("{} {}", ICON_CHECK.codepoint, t!("text-apply-sketch")))
                    .size(11.5)
                    .strong()
                    .color(Color32::WHITE),
            )
            .fill(ACCENT_BLUE)
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
                font_preset: state.font_preset,
                is_construction: state.is_construction,
            });
        }

        event
    }
}
