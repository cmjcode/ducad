//! Desain yang bisa dipulihkan (Paritas 3.7).
//!
//! Tiap sesi aplikasi menulis autosave-nya sendiri ke
//! `data_dir/recovery/<kunci>.ducad` + metadata `<kunci>.json` (asal berkas,
//! waktu). Saat start, entri yang tersisa (sesi crash / dimatikan OS)
//! ditampilkan di dialog "Pulihkan / Buang" — tidak lagi dibuka otomatis.
//! Berkas lama `autosave.ducad` dari versi sebelumnya dipindah ke sini.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// Metadata satu entri pemulihan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecoveryMeta {
    /// Berkas asal dokumen (kosong = belum pernah disimpan).
    #[serde(default)]
    pub origin: String,
    /// Detik UNIX saat autosave ditulis.
    #[serde(default)]
    pub saved_at: u64,
}

/// Satu entri yang bisa dipulihkan.
#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryEntry {
    pub key: String,
    pub file: PathBuf,
    pub meta: RecoveryMeta,
    pub size: u64,
}

impl RecoveryEntry {
    /// Nama tampilan: nama berkas asal atau "Tanpa judul".
    pub fn title(&self) -> String {
        Path::new(&self.meta.origin)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| ducad_i18n::t!("recovery-untitled"))
    }
}

/// Folder pemulihan.
pub fn dir() -> PathBuf {
    crate::platform::data_dir().join("recovery")
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Kunci sesi ini (unik per proses).
pub fn session_key() -> &'static str {
    static KEY: OnceLock<String> = OnceLock::new();
    KEY.get_or_init(|| format!("{}-{}", now_secs(), std::process::id()))
}

fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 64
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Berkas desain entri `key` di `dir`.
pub fn file_in(dir: &Path, key: &str) -> PathBuf {
    dir.join(format!("{key}.ducad"))
}

fn meta_in(dir: &Path, key: &str) -> PathBuf {
    dir.join(format!("{key}.json"))
}

/// Catat metadata entri `key` (dipanggil setelah berkas desain ditulis).
pub fn write_meta(dir: &Path, key: &str, origin: &str) -> std::io::Result<()> {
    let meta = RecoveryMeta {
        origin: origin.to_string(),
        saved_at: now_secs(),
    };
    let text = serde_json::to_string(&meta).map_err(std::io::Error::other)?;
    std::fs::write(meta_in(dir, key), text)
}

/// Semua entri di `dir`, terbaru dulu. Entri tanpa metadata tetap tampil.
pub fn list_in(dir: &Path) -> Vec<RecoveryEntry> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<RecoveryEntry> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if path.extension()? != "ducad" {
                return None;
            }
            let key = path.file_stem()?.to_str()?.to_string();
            if !valid_key(&key) {
                return None;
            }
            let md = e.metadata().ok();
            let size = md.as_ref().map(|m| m.len()).unwrap_or(0);
            let meta = std::fs::read_to_string(meta_in(dir, &key))
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or_else(|| RecoveryMeta {
                    origin: String::new(),
                    saved_at: md
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0),
                });
            Some(RecoveryEntry {
                key,
                file: path,
                meta,
                size,
            })
        })
        .collect();
    out.sort_by(|a, b| b.meta.saved_at.cmp(&a.meta.saved_at));
    out
}

/// Hapus entri `key` (berkas desain, metadata, dan cadangannya).
pub fn discard_in(dir: &Path, key: &str) {
    if !valid_key(key) {
        return;
    }
    let f = file_in(dir, key);
    let _ = std::fs::remove_file(ducad_io::native::backup_path(&f));
    let _ = std::fs::remove_file(f);
    let _ = std::fs::remove_file(meta_in(dir, key));
}

/// Pindahkan entri `from` menjadi milik sesi `to` (dipakai saat
/// memulihkan, agar entri tidak hilang bila sesi ini juga crash).
pub fn adopt_in(dir: &Path, from: &str, to: &str) -> std::io::Result<PathBuf> {
    if !valid_key(from) || !valid_key(to) {
        return Err(std::io::Error::other("kunci pemulihan tidak valid"));
    }
    let dst = file_in(dir, to);
    if from != to {
        std::fs::rename(file_in(dir, from), &dst)?;
        let _ = std::fs::rename(meta_in(dir, from), meta_in(dir, to));
        let _ = std::fs::remove_file(ducad_io::native::backup_path(&file_in(dir, from)));
    }
    Ok(dst)
}

/// Pindahkan autosave lama (satu berkas global) ke folder pemulihan.
pub fn migrate_legacy(dir: &Path, legacy: &Path, legacy_origin: &Path) {
    if !legacy.is_file() {
        return;
    }
    let _ = std::fs::create_dir_all(dir);
    let key = format!("legacy-{}", now_secs());
    if std::fs::rename(legacy, file_in(dir, &key)).is_ok() {
        let origin = std::fs::read_to_string(legacy_origin).unwrap_or_default();
        let _ = write_meta(dir, &key, origin.trim());
    }
    let _ = std::fs::remove_file(legacy_origin);
}

/// Status dialog pemulihan.
#[derive(Default)]
pub struct RecoveryState {
    pub open: bool,
    pub entries: Vec<RecoveryEntry>,
}

impl crate::app::DuCADApp {
    /// Muat ulang daftar entri (kecuali milik sesi ini); buka dialog bila
    /// ada entri atau `force_open`.
    pub fn refresh_recovery(&mut self, force_open: bool) {
        let me = session_key();
        self.recovery.entries = list_in(&dir())
            .into_iter()
            .filter(|e| e.key != me)
            .collect();
        self.recovery.open = force_open || !self.recovery.entries.is_empty();
    }

    /// Pulihkan entri `key`: buka desainnya dan jadikan milik sesi ini.
    pub fn recover_design(&mut self, key: &str) {
        let Some(entry) = self.recovery.entries.iter().find(|e| e.key == key).cloned() else {
            return;
        };
        let d = dir();
        discard_in(&d, session_key());
        let path = match adopt_in(&d, &entry.key, session_key()) {
            Ok(p) => p,
            Err(e) => {
                self.file_status = Some(ducad_i18n::t!("recovery-failed", error = e.to_string()));
                return;
            }
        };
        self.open_native_path(path);
        let origin = PathBuf::from(&entry.meta.origin);
        self.current_file_path =
            (!entry.meta.origin.is_empty() && origin.is_file()).then_some(origin);
        // Tandai kotor supaya autosave berikutnya tetap menulis entri ini.
        self.model.doc.dirty = true;
        let title = entry.title();
        self.file_status = Some(ducad_i18n::t!("recovery-restored", name = title.as_str()));
        self.mobile.restored_from_autosave = true;
        self.refresh_recovery(false);
    }

    /// Buang entri `key` (atau semua bila `None`).
    pub fn discard_recovery(&mut self, key: Option<&str>) {
        let d = dir();
        let keys: Vec<String> = match key {
            Some(k) => vec![k.to_string()],
            None => self.recovery.entries.iter().map(|e| e.key.clone()).collect(),
        };
        for k in keys {
            discard_in(&d, &k);
        }
        let was_open = self.recovery.open;
        self.refresh_recovery(false);
        self.recovery.open = was_open && !self.recovery.entries.is_empty();
    }

    /// Dialog "Desain yang bisa dipulihkan".
    pub fn show_recovery(&mut self, ctx: &egui::Context) {
        if !self.recovery.open {
            return;
        }
        let mut open = true;
        let mut restore: Option<String> = None;
        let mut discard: Option<Option<String>> = None;
        egui::Window::new(ducad_i18n::t!("recovery-title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new(ducad_i18n::t!("recovery-help")).small());
                if self.recovery.entries.is_empty() {
                    ui.label(ducad_i18n::t!("recovery-empty"));
                }
                egui::Grid::new("recovery-grid")
                    .num_columns(4)
                    .striped(true)
                    .show(ui, |ui| {
                        for e in &self.recovery.entries {
                            ui.label(e.title()).on_hover_text(&e.meta.origin);
                            ui.label(format_age(e.meta.saved_at));
                            ui.label(format!("{:.1} KB", e.size as f64 / 1024.0));
                            ui.horizontal(|ui| {
                                if ui.button(ducad_i18n::t!("recovery-restore")).clicked() {
                                    restore = Some(e.key.clone());
                                }
                                if ui
                                    .button(egui_icons::icons::ICON_DELETE.codepoint)
                                    .on_hover_text(ducad_i18n::t!("recovery-discard"))
                                    .clicked()
                                {
                                    discard = Some(Some(e.key.clone()));
                                }
                            });
                            ui.end_row();
                        }
                    });
                if !self.recovery.entries.is_empty()
                    && ui.button(ducad_i18n::t!("recovery-discard-all")).clicked()
                {
                    discard = Some(None);
                }
            });
        if let Some(k) = restore {
            self.recover_design(&k);
            self.recovery.open = false;
        } else if let Some(k) = discard {
            self.discard_recovery(k.as_deref());
        }
        if !open {
            self.recovery.open = false;
        }
    }
}

/// "5 menit lalu" dsb.
fn format_age(saved_at: u64) -> String {
    let secs = now_secs().saturating_sub(saved_at);
    if secs < 90 {
        ducad_i18n::t!("recovery-age-now")
    } else if secs < 90 * 60 {
        ducad_i18n::t!("recovery-age-min", n = secs / 60)
    } else if secs < 36 * 3600 {
        ducad_i18n::t!("recovery-age-hour", n = secs / 3600)
    } else {
        ducad_i18n::t!("recovery-age-day", n = secs / 86400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ducad-recovery-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn list_adopt_and_discard() {
        let d = tmp("list");
        std::fs::write(file_in(&d, "100-1"), b"{}").unwrap();
        write_meta(&d, "100-1", "/x/plat.ducad").unwrap();
        std::fs::write(file_in(&d, "200-2"), b"{}").unwrap();
        std::fs::write(d.join("bad key!.ducad"), b"").unwrap();
        let list = list_in(&d);
        assert_eq!(list.len(), 2, "{list:?}");
        let plat = list.iter().find(|e| e.key == "100-1").unwrap();
        assert_eq!(plat.title(), "plat.ducad");
        let moved = adopt_in(&d, "100-1", "300-3").unwrap();
        assert!(moved.is_file() && !file_in(&d, "100-1").exists());
        let adopted = list_in(&d).into_iter().find(|e| e.key == "300-3").unwrap();
        assert_eq!(adopted.meta.origin, "/x/plat.ducad");
        discard_in(&d, "300-3");
        discard_in(&d, "200-2");
        discard_in(&d, "../evil");
        assert_eq!(list_in(&d).len(), 0);
        assert!(adopt_in(&d, "../a", "b").is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn legacy_autosave_is_migrated() {
        let d = tmp("legacy");
        let legacy = d.join("autosave.ducad");
        let origin = d.join("autosave.origin");
        std::fs::write(&legacy, b"{}").unwrap();
        std::fs::write(&origin, "/home/a.ducad\n").unwrap();
        let rd = d.join("recovery");
        migrate_legacy(&rd, &legacy, &origin);
        let list = list_in(&rd);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].meta.origin, "/home/a.ducad");
        assert!(!legacy.exists() && !origin.exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn missing_dir_is_empty_not_error() {
        assert!(list_in(Path::new("/nonexistent/ducad/recovery")).is_empty());
    }
}
