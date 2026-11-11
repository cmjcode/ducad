//! Tumpukan notifikasi bertingkat (Info, Peringatan, Galat) di kanvas.
//!
//! Menyatukan pesan `model_status`, `file_status`, dan `constraint_status`
//! ke satu komponen antarmuka yang konsisten, dapat dibaca pembaca layar
//! (`widget_info`), dan dapat ditutup satu per satu.

use crate::theme::{pill_frame, ThemeTokens, TYPOGRAPHY};
use ducad_i18n::t;
use egui::{Align2, Pos2, RichText, Stroke, Vec2};
use egui_icons::icons::{ICON_CLOSE, ICON_ERROR, ICON_INFO, ICON_WARNING};

/// Tingkat keparahan pesan notifikasi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    Info,
    Warning,
    Error,
}

/// Satu baris notifikasi aktif untuk dirender oleh [`NotificationToast`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationToastItem {
    pub id: u64,
    pub level: NotificationLevel,
    pub message: String,
}

pub struct NotificationToast;

impl NotificationToast {
    /// Render hingga 3 notifikasi terbaru di bawah pil status (`anchor`).
    /// Mengembalikan `Some(id)` bila tombol tutup pada notifikasi diklik.
    pub fn show(
        ctx: &egui::Context,
        anchor: Pos2,
        items: &[NotificationToastItem],
    ) -> Option<u64> {
        if items.is_empty() {
            return None;
        }
        let mut dismissed = None;
        egui::Area::new(egui::Id::new("ducad-notification-toast-stack"))
            .fixed_pos(anchor)
            .pivot(Align2::LEFT_TOP)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let tok = ThemeTokens::from_ui(ui);
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 6.0);
                for item in items.iter().rev().take(3) {
                    let (icon, accent) = match item.level {
                        NotificationLevel::Info => (ICON_INFO.codepoint, tok.accent_primary),
                        NotificationLevel::Warning => (ICON_WARNING.codepoint, tok.accent_warning),
                        NotificationLevel::Error => (ICON_ERROR.codepoint, tok.accent_danger),
                    };
                    pill_frame()
                        .stroke(Stroke::new(1.0, accent))
                        .show(ui, |ui| {
                            ui.set_max_width(420.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(icon)
                                        .size(TYPOGRAPHY.subheading)
                                        .color(accent),
                                );
                                let msg_resp = ui.label(
                                    RichText::new(&item.message)
                                        .size(TYPOGRAPHY.caption)
                                        .color(tok.text_primary),
                                );
                                let msg_for_a11y = item.message.clone();
                                msg_resp.widget_info(move || {
                                    egui::WidgetInfo::labeled(
                                        egui::WidgetType::Label,
                                        true,
                                        msg_for_a11y.clone(),
                                    )
                                });
                                let close_tip = t!("notif-dismiss");
                                let close_resp = ui
                                    .small_button(
                                        RichText::new(ICON_CLOSE.codepoint)
                                            .size(TYPOGRAPHY.caption)
                                            .color(tok.text_secondary),
                                    )
                                    .on_hover_text(&close_tip);
                                close_resp.widget_info(move || {
                                    egui::WidgetInfo::labeled(
                                        egui::WidgetType::Button,
                                        true,
                                        close_tip.clone(),
                                    )
                                });
                                if close_resp.clicked() {
                                    dismissed = Some(item.id);
                                }
                            });
                        });
                }
            });
        dismissed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{apply, ThemeMode};

    #[test]
    fn notification_toast_renders_and_handles_empty_gracefully() {
        let ctx = egui::Context::default();
        apply(&ctx, ThemeMode::Dark);
        let mut out = ctx.run_ui(Default::default(), |ctx| {
            assert_eq!(NotificationToast::show(ctx, Pos2::new(16.0, 64.0), &[]), None);
            let items = vec![
                NotificationToastItem {
                    id: 1,
                    level: NotificationLevel::Info,
                    message: "STEP exported".to_string(),
                },
                NotificationToastItem {
                    id: 2,
                    level: NotificationLevel::Warning,
                    message: "Numeric selector warning".to_string(),
                },
            ];
            let _ = NotificationToast::show(ctx, Pos2::new(16.0, 64.0), &items);
        });
        out.textures_delta.clear();
    }
}

