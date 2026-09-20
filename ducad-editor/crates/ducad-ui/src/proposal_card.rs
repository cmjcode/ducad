//! Kartu proposal agent (P8.4): ringkasan perubahan + tombol Terima/Tolak.
//!
//! Ghost geometrinya digambar di viewport (hijau = ditambah, merah =
//! dihapus); kartu ini hanya keputusan pengguna. Agent tidak bisa menerima
//! proposalnya sendiri pada sesi live.

use crate::theme::{glass_frame, ACCENT_GREEN, TEXT_PRIMARY, TEXT_SECONDARY};
use egui::{Align2, Color32, RichText, Vec2};

#[derive(Debug, Clone, Default)]
pub struct ProposalCardState {
    pub title: String,
    /// Mis. "+1200.0 mm³ / −340.0 mm³".
    pub summary: String,
    /// Satu baris per op.
    pub ops: Vec<String>,
    pub accept: String,
    pub reject: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalCardEvent {
    Accept,
    Reject,
}

pub struct ProposalCard;

impl ProposalCard {
    pub fn show(ctx: &egui::Context, state: &ProposalCardState) -> Option<ProposalCardEvent> {
        let mut event = None;
        egui::Area::new(egui::Id::new("ducad-proposal-card"))
            .anchor(Align2::RIGHT_BOTTOM, Vec2::new(-16.0, -96.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                glass_frame().show(ui, |ui| {
                    ui.set_max_width(320.0);
                    ui.label(RichText::new(&state.title).strong().color(TEXT_PRIMARY));
                    ui.label(RichText::new(&state.summary).color(TEXT_SECONDARY));
                    for line in state.ops.iter().take(8) {
                        ui.label(RichText::new(line).size(11.0).color(TEXT_SECONDARY));
                    }
                    if state.ops.len() > 8 {
                        ui.label(
                            RichText::new(format!("… +{}", state.ops.len() - 8))
                                .size(11.0)
                                .color(TEXT_SECONDARY),
                        );
                    }
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let accept =
                            egui::Button::new(RichText::new(&state.accept).color(Color32::WHITE))
                                .fill(ACCENT_GREEN);
                        if ui.add(accept).clicked() {
                            event = Some(ProposalCardEvent::Accept);
                        }
                        if ui.button(&state.reject).clicked() {
                            event = Some(ProposalCardEvent::Reject);
                        }
                    });
                });
            });
        event
    }
}
