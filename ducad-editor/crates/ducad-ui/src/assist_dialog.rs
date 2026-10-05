//! Dialog "Tanya AI…" (P11.4): satu kotak instruksi, spinner saat model
//! bekerja, lalu alasan + ringkasan usulan dengan tombol Terapkan/Tolak.
//! Usulan TIDAK pernah diterapkan tanpa klik pengguna.

use crate::theme::{glass_frame, ACCENT_BLUE, TEXT_PRIMARY, TEXT_SECONDARY};
use ducad_i18n::t;
use egui::{Color32, RichText};

#[derive(Debug, Clone, Default)]
pub struct AssistDialogState {
    pub open: bool,
    /// Kotak teks instruksi (diubah oleh dialog).
    pub instruction: String,
    /// Model sedang bekerja di thread latar.
    pub busy: bool,
    /// Nama backend aktif, mis. "apple-fm"; kosong = tidak ada backend.
    pub backend: String,
    /// `true` bila backend berjalan di perangkat.
    pub on_device: bool,
    /// Alasan dari model (`rationale`).
    pub rationale: String,
    /// Ringkasan usulan per baris, mis. "t: 8 → 10".
    pub changes: Vec<String>,
    /// Ada usulan yang bisa diterapkan.
    pub has_proposal: bool,
    /// Jawaban teks (`explain`/`ask_user`) atau pesan error.
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssistDialogEvent {
    Ask,
    Cancel,
    Apply,
    Reject,
    Close,
}

pub struct AssistDialog;

impl AssistDialog {
    pub fn show(ctx: &egui::Context, state: &mut AssistDialogState) -> Option<AssistDialogEvent> {
        if !state.open {
            return None;
        }
        let mut event = None;
        let mut open = true;
        let assist_title = t!("assist-title");
        let assist_area_id = egui::Id::new(assist_title.as_str());
        egui::Window::new(assist_title.as_str())
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(420.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .frame(glass_frame().transparent_flat())
            .show(ctx, |ui| crate::theme::glass_window(ui, assist_area_id, |ui| {
                ui.set_max_width(420.0);
                let badge = if state.backend.is_empty() {
                    t!("assist-no-backend")
                } else if state.on_device {
                    t!("ai-chip-on-device")
                } else {
                    t!("ai-chip-external")
                };
                ui.label(RichText::new(badge).size(11.0).color(TEXT_SECONDARY));
                ui.add_enabled_ui(!state.busy && !state.backend.is_empty(), |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut state.instruction)
                            .hint_text(t!("assist-hint"))
                            .desired_rows(2)
                            .desired_width(f32::INFINITY),
                    );
                });
                ui.horizontal(|ui| {
                    if state.busy {
                        ui.spinner();
                        ui.label(RichText::new(t!("assist-working")).color(TEXT_SECONDARY));
                        if ui.button(t!("assist-cancel")).clicked() {
                            event = Some(AssistDialogEvent::Cancel);
                        }
                    } else {
                        let can_ask = !state.instruction.trim().is_empty() && !state.backend.is_empty();
                        let btn = egui::Button::new(RichText::new(t!("assist-ask")).color(Color32::WHITE))
                            .fill(ACCENT_BLUE);
                        if ui.add_enabled(can_ask, btn).clicked() {
                            event = Some(AssistDialogEvent::Ask);
                        }
                    }
                });
                if !state.rationale.is_empty() {
                    ui.add_space(4.0);
                    ui.label(RichText::new(&state.rationale).color(TEXT_PRIMARY));
                }
                for c in &state.changes {
                    ui.label(RichText::new(format!("• {c}")).size(11.0).color(TEXT_SECONDARY));
                }
                if !state.message.is_empty() {
                    ui.add_space(4.0);
                    ui.label(RichText::new(&state.message).color(TEXT_SECONDARY));
                }
                if state.has_proposal && !state.busy {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        let apply = egui::Button::new(RichText::new(t!("assist-apply")).color(Color32::WHITE))
                            .fill(ACCENT_BLUE);
                        if ui.add(apply).clicked() {
                            event = Some(AssistDialogEvent::Apply);
                        }
                        if ui.button(t!("assist-reject")).clicked() {
                            event = Some(AssistDialogEvent::Reject);
                        }
                    });
                }
                ui.add_space(4.0);
                ui.label(
                    RichText::new(t!("assist-capability-note"))
                        .size(10.0)
                        .color(TEXT_SECONDARY),
                );
            }));
        if !open {
            state.open = false;
            return Some(AssistDialogEvent::Close);
        }
        event
    }
}
