//! Panel Checks — hasil pemeriksaan desain (P7.5).
//!
//! Satu baris per check: ikon status · id/jenis · terukur vs harapan. Klik
//! baris → caller memilih body terkait dan mengarahkan kamera ke lokasinya.
//! Hasil yang sedang dihitung ulang di latar belakang digambar redup.

use crate::theme::{
    glass_frame, ACCENT_GREEN, ACCENT_ORANGE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};
use ducad_i18n::t;
use egui::{Color32, RichText, ScrollArea, Sense, Ui, Vec2};
use egui_icons::icons::ICON_CLOSE;

/// Status satu baris.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckRowStatus {
    Pass,
    Fail,
    Error,
}

/// Data tampilan satu check (dibangun caller dari `CheckResult`).
#[derive(Debug, Clone)]
pub struct CheckRowUi {
    pub status: CheckRowStatus,
    /// `id` check, atau jenisnya bila tanpa id.
    pub label: String,
    /// Pesan terukur vs harapan.
    pub detail: String,
    /// Baris punya lokasi 3D / body yang bisa disorot.
    pub focusable: bool,
    /// Hasil lama yang sedang dihitung ulang.
    pub stale: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecksPanelEvent {
    RowClicked(usize),
    Close,
}

const FAIL_RED: Color32 = Color32::from_rgb(255, 69, 58);
const PANEL_W: f32 = crate::theme::BOTTOM_RIGHT_PANEL_WIDTH + 60.0;

/// Ringkasan `(lulus, tidak lulus)` untuk top bar.
pub fn checks_summary(rows: &[CheckRowUi]) -> (usize, usize) {
    let pass = rows
        .iter()
        .filter(|r| r.status == CheckRowStatus::Pass)
        .count();
    (pass, rows.len() - pass)
}

pub struct ChecksPanel;

impl ChecksPanel {
    pub fn show(ui: &mut Ui, rows: &[CheckRowUi]) -> Option<ChecksPanelEvent> {
        let mut event = None;
        glass_frame().show(ui, |ui| {
            ui.set_width(PANEL_W);
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
            ui.horizontal(|ui| {
                let (pass, not_pass) = checks_summary(rows);
                ui.label(
                    RichText::new(t!("checks-title"))
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.label(RichText::new(format!("✔ {pass}  ✖ {not_pass}")).color(TEXT_SECONDARY));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button(ICON_CLOSE.codepoint)
                        .on_hover_text(t!("checks-close"))
                        .clicked()
                    {
                        event = Some(ChecksPanelEvent::Close);
                    }
                });
            });
            ui.separator();
            if rows.is_empty() {
                ui.label(RichText::new(t!("checks-empty")).color(TEXT_MUTED));
                return;
            }
            ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                for (i, row) in rows.iter().enumerate() {
                    let (icon, color) = match row.status {
                        CheckRowStatus::Pass => ("✔", ACCENT_GREEN),
                        CheckRowStatus::Fail => ("✖", FAIL_RED),
                        CheckRowStatus::Error => ("!", ACCENT_ORANGE),
                    };
                    let dim = |c: Color32| if row.stale { c.gamma_multiply(0.45) } else { c };
                    let resp = ui
                        .horizontal(|ui| {
                            ui.label(RichText::new(icon).strong().color(dim(color)));
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&row.label).color(dim(TEXT_PRIMARY)));
                                ui.label(
                                    RichText::new(&row.detail)
                                        .small()
                                        .color(dim(TEXT_SECONDARY)),
                                );
                            });
                        })
                        .response
                        .interact(Sense::click());
                    let resp = if row.stale {
                        resp.on_hover_text(t!("checks-stale"))
                    } else {
                        resp
                    };
                    if row.focusable && resp.clicked() {
                        event = Some(ChecksPanelEvent::RowClicked(i));
                    }
                }
            });
        });
        event
    }
}
