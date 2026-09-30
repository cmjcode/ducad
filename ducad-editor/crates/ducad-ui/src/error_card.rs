//! Kartu error operasi (P9.3): judul dari kode error (lewat i18n), satu
//! kalimat penyebab, dan tombol per perbaikan terverifikasi.

use crate::theme::{glass_frame, ACCENT_BLUE, TEXT_PRIMARY, TEXT_SECONDARY};
use ducad_i18n::t;
use egui::{Align2, Color32, RichText, Vec2};
use egui_icons::icons::ICON_CLOSE;

#[derive(Debug, Clone, Default)]
pub struct ErrorCardState {
    pub open: bool,
    pub title: String,
    pub cause: String,
    /// Label tombol per fix ("Pakai radius 1.8 mm").
    pub fixes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCardEvent {
    ApplyFix(usize),
    Close,
}

pub struct ErrorCard;

impl ErrorCard {
    /// Kartu melayang di tengah bawah layar.
    pub fn show(ctx: &egui::Context, state: &ErrorCardState) -> Option<ErrorCardEvent> {
        if !state.open {
            return None;
        }
        let mut event = None;
        egui::Area::new(egui::Id::new("ducad-error-card"))
            .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -96.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                glass_frame().show(ui, |ui| {
                    ui.set_max_width(420.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("⚠").color(Color32::from_rgb(255, 159, 10)));
                        ui.label(RichText::new(&state.title).strong().color(TEXT_PRIMARY));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button(ICON_CLOSE.codepoint).on_hover_text(t!("error-card-close")).clicked() {
                                event = Some(ErrorCardEvent::Close);
                            }
                        });
                    });
                    ui.label(RichText::new(&state.cause).color(TEXT_SECONDARY));
                    if !state.fixes.is_empty() {
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            for (i, label) in state.fixes.iter().enumerate() {
                                let btn = egui::Button::new(RichText::new(label).color(Color32::WHITE))
                                    .fill(ACCENT_BLUE);
                                if ui.add(btn).clicked() {
                                    event = Some(ErrorCardEvent::ApplyFix(i));
                                }
                            }
                        });
                    }
                });
            });
        event
    }
}
