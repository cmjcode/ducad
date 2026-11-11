//! Antrean notifikasi bertingkat tunggal untuk DUCAD Studio (Tahap 3).
//!
//! Menyambungkan perubahan pada `model_status`, `file_status`, dan
//! `constraint_status` secara **non-destruktif** (nilai field asli tetap utuh
//! demi kompatibilitas sinkron & tes) ke tumpukan [`NotificationToast`] di
//! bawah pil status serta mengarahkan galat operasi jalur lama ke
//! [`ducad_ui::ErrorCardState`] bila kartu galat belum terbuka.

use ducad_i18n::t;
use ducad_ui::{NotificationLevel, NotificationToast, NotificationToastItem};
use eframe::egui;

use crate::app::DuCADApp;

/// Maksimum notifikasi aktif di tumpukan pada satu waktu.
const MAX_ACTIVE_TOASTS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationSource {
    Model,
    File,
    Constraint,
}

#[derive(Debug, Clone)]
struct QueuedNotification {
    id: u64,
    level: NotificationLevel,
    text: String,
    expires_at: f64,
}

/// Antrean notifikasi aplikasi dengan pelacak nilai terakhir agar perubahan
/// field status lama terdeteksi tanpa menghapus nilainya.
#[derive(Debug, Clone, Default)]
pub struct NotificationQueue {
    next_id: u64,
    entries: Vec<QueuedNotification>,
    last_model: Option<String>,
    last_file: Option<String>,
    last_constraint: Option<String>,
}

impl NotificationQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Dorong pesan baru ke antrean. Mengembalikan tingkat keparahannya.
    pub fn push(&mut self, level: NotificationLevel, text: impl Into<String>, now_secs: f64) -> u64 {
        let text = text.into().trim().to_string();
        if text.is_empty() {
            return 0;
        }
        let ttl = match level {
            NotificationLevel::Info => 5.5,
            NotificationLevel::Warning => 8.0,
            NotificationLevel::Error => 10.0,
        };
        if let Some(existing) = self.entries.iter_mut().find(|e| e.text == text && e.level == level) {
            existing.expires_at = now_secs + ttl;
            return existing.id;
        }
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let id = self.next_id;
        self.entries.push(QueuedNotification {
            id,
            level,
            text,
            expires_at: now_secs + ttl,
        });
        if self.entries.len() > MAX_ACTIVE_TOASTS {
            let drop_count = self.entries.len() - MAX_ACTIVE_TOASTS;
            self.entries.drain(0..drop_count);
        }
        id
    }

    /// Tutup satu notifikasi berdasarkan ID.
    pub fn dismiss(&mut self, id: u64) {
        self.entries.retain(|e| e.id != id);
    }

    /// Bersihkan semua notifikasi yang sudah melewati batas waktunya.
    pub fn prune_expired(&mut self, now_secs: f64) {
        self.entries.retain(|e| e.expires_at > now_secs);
    }

    /// Jumlah notifikasi yang masih aktif.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Salinan siap render untuk [`NotificationToast::show`] (terbaru di atas).
    pub fn toast_items(&self) -> Vec<NotificationToastItem> {
        self.entries
            .iter()
            .rev()
            .map(|e| NotificationToastItem {
                id: e.id,
                level: e.level,
                message: e.text.clone(),
            })
            .collect()
    }
}

/// Klasifikasi heuristik tingkat keparahan pesan status bahasa Indonesia & Inggris.
pub fn classify_status_message(msg: &str, source: NotificationSource) -> NotificationLevel {
    let lower = msg.to_lowercase();

    let is_error = [
        "gagal",
        "galat",
        "error",
        "failed",
        "tidak valid",
        "invalid",
        "tidak bisa",
        "cannot",
        "could not",
        "rusak",
        "corrupt",
        "ditolak",
        "internal error",
        "✖",
        "❌",
    ]
    .iter()
    .any(|kw| lower.contains(kw));
    if is_error {
        return NotificationLevel::Error;
    }

    let is_warning = [
        "peringatan",
        "warning",
        "konflik",
        "conflict",
        "berlebihan",
        "overconstrained",
        "rapuh",
        "kedaluwarsa",
        "tidak ada body",
        "tak ada body",
        "no visible bodies",
        "pilih ",
        "select ",
        "⚠",
    ]
    .iter()
    .any(|kw| lower.contains(kw));
    if is_warning {
        return NotificationLevel::Warning;
    }

    // Constraint status non-kosong hampir selalu menjelaskan penolakan / konflik solver.
    if source == NotificationSource::Constraint {
        return NotificationLevel::Warning;
    }

    NotificationLevel::Info
}

impl DuCADApp {
    /// Sinkronkan perubahan `model_status`, `file_status`, dan `constraint_status`
    /// ke antrean notifikasi tanpa mengosongkan ketiga field tersebut.
    pub fn sync_notifications(&mut self, now_secs: f64) {
        self.notifications.prune_expired(now_secs);

        let mut newly_added: Vec<(NotificationSource, String)> = Vec::new();

        if self.model_status != self.notifications.last_model {
            self.notifications.last_model = self.model_status.clone();
            if let Some(msg) = self.model_status.as_ref().filter(|s| !s.trim().is_empty()) {
                newly_added.push((NotificationSource::Model, msg.clone()));
            }
        }
        if self.file_status != self.notifications.last_file {
            self.notifications.last_file = self.file_status.clone();
            if let Some(msg) = self.file_status.as_ref().filter(|s| !s.trim().is_empty()) {
                newly_added.push((NotificationSource::File, msg.clone()));
            }
        }
        if self.constraint_status != self.notifications.last_constraint {
            self.notifications.last_constraint = self.constraint_status.clone();
            if let Some(msg) = self.constraint_status.as_ref().filter(|s| !s.trim().is_empty()) {
                newly_added.push((NotificationSource::Constraint, msg.clone()));
            }
        }

        for (src, msg) in newly_added {
            let level = classify_status_message(&msg, src);
            self.notifications.push(level, msg.clone(), now_secs);
            // Arahkan galat ke kartu galat bila belum ada kartu OpError terstruktur yang terbuka.
            if level == NotificationLevel::Error && !self.error_card.open {
                self.error_card.open = true;
                self.error_card.title = t!("notif-error-title");
                self.error_card.cause = msg;
                self.error_card.recovery.clear();
                self.error_card.fixes.clear();
                self.error_fixes.clear();
            }
        }
    }

    /// Render tumpukan toast notifikasi di `anchor` (kiri atas di bawah pil status).
    pub fn show_notifications(&mut self, ctx: &egui::Context, anchor: egui::Pos2) {
        let now = ctx.input(|i| i.time);
        self.sync_notifications(now);
        if self.notifications.is_empty() {
            return;
        }
        // Jadwalkan repaint berkala agar toast otomatis menghilang tepat waktu.
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
        let items = self.notifications.toast_items();
        if let Some(dismissed_id) = NotificationToast::show(ctx, anchor, &items) {
            self.notifications.dismiss(dismissed_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_fields_feed_queue_non_destructively_and_route_errors_to_card() {
        let mut app = DuCADApp::new_for_test();
        app.file_status = Some("STEP saved: bracket.step".to_string());
        app.sync_notifications(1.0);
        // Field asli tidak dihapus.
        assert_eq!(app.file_status.as_deref(), Some("STEP saved: bracket.step"));
        assert_eq!(app.notifications.len(), 1);
        assert!(!app.error_card.open);

        // Pesan galat membuka kartu error bila sebelumnya tertutup.
        app.model_status = Some("Gagal operasi boolean".to_string());
        app.sync_notifications(2.0);
        assert_eq!(app.model_status.as_deref(), Some("Gagal operasi boolean"));
        assert_eq!(app.notifications.len(), 2);
        assert!(app.error_card.open);
        assert_eq!(app.error_card.cause, "Gagal operasi boolean");

        // Setelah kadaluarsa, toast dibersihkan.
        app.sync_notifications(20.0);
        assert!(app.notifications.is_empty());
    }

    #[test]
    fn structured_error_card_is_not_clobbered_by_status_sync() {
        let mut app = DuCADApp::new_for_test();
        app.error_card.open = true;
        app.error_card.title = "Fillet Too Large".to_string();
        app.error_card.cause = "Radius 10 mm exceeds edge".to_string();
        app.model_status = Some("Fillet gagal: radius terlalu besar".to_string());

        app.sync_notifications(1.0);
        assert_eq!(app.error_card.title, "Fillet Too Large");
        assert_eq!(app.error_card.cause, "Radius 10 mm exceeds edge");
    }
}
