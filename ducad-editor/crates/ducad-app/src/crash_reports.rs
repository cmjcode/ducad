//! Laporan crash + replay (Paritas 3.14).
//!
//! Saat command model GUI panik (ditangkap `catch_unwind` di
//! `execute_model_command`), DUCAD menulis ke `log_dir()/crashes/`:
//! - `crash-<ts>.json` — waktu, versi, backend kernel, aksi, pesan panik;
//! - `crash-<ts>.ducad` — salinan dokumen saat itu (sketsa, body, oplog),
//!   yang bisa diputar ulang: `ducad-cli replay crash-<ts>.ducad`.
//!
//! Hanya lokal — tidak dikirim ke mana pun. Menu: palette "Laporan crash".

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Isi `crash-<ts>.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrashReport {
    pub ts: String,
    pub version: String,
    pub backend: String,
    /// Nama command/aksi GUI yang panik.
    pub action: String,
    pub message: String,
    /// Nama berkas `.ducad` salinan dokumen (kosong bila gagal ditulis).
    #[serde(default)]
    pub document: String,
}

/// Folder laporan crash.
pub fn dir() -> PathBuf {
    crate::logging::log_dir().join("crashes")
}

/// Batas jumlah laporan yang disimpan (yang tertua dibuang).
pub const MAX_REPORTS: usize = 50;

/// Stempel waktu aman untuk nama berkas.
fn stamp() -> String {
    chrono::Utc::now().format("%Y%m%dT%H%M%S%.3fZ").to_string()
}

/// Tulis metadata laporan; `write_doc` diberi path `.ducad` tujuan dan
/// mengembalikan `true` bila salinan dokumen berhasil ditulis.
pub fn write_report_in(
    dir: &Path,
    action: &str,
    message: &str,
    write_doc: impl FnOnce(&Path) -> bool,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let ts = stamp();
    let base = format!("crash-{ts}");
    let doc_path = dir.join(format!("{base}.ducad"));
    let document = if write_doc(&doc_path) {
        format!("{base}.ducad")
    } else {
        String::new()
    };
    let report = CrashReport {
        ts,
        version: env!("CARGO_PKG_VERSION").to_string(),
        backend: ducad_kernel::BACKEND.to_string(),
        action: action.to_string(),
        message: message.to_string(),
        document,
    };
    let path = dir.join(format!("{base}.json"));
    let text = serde_json::to_string_pretty(&report).map_err(std::io::Error::other)?;
    std::fs::write(&path, text)?;
    prune_in(dir, MAX_REPORTS);
    Ok(path)
}

/// Laporan di `dir`, terbaru dulu: (path json, isi).
pub fn list_in(dir: &Path) -> Vec<(PathBuf, CrashReport)> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<(PathBuf, CrashReport)> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|x| x == "json")
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("crash-"))
        })
        .filter_map(|p| {
            let text = std::fs::read_to_string(&p).ok()?;
            let r: CrashReport = serde_json::from_str(&text).ok()?;
            Some((p, r))
        })
        .collect();
    out.sort_by(|a, b| b.1.ts.cmp(&a.1.ts));
    out
}

/// Hapus satu laporan (json + salinan dokumen).
pub fn delete_report(json: &Path) {
    let _ = std::fs::remove_file(json.with_extension("ducad"));
    let _ = std::fs::remove_file(json);
}

fn prune_in(dir: &Path, keep: usize) {
    for (p, _) in list_in(dir).into_iter().skip(keep) {
        delete_report(&p);
    }
}

/// Status jendela laporan crash.
#[derive(Default)]
pub struct CrashReportsState {
    pub open: bool,
}

impl crate::app::DuCADApp {
    /// Catat crash command GUI `action` (dipanggil dari pagar panik).
    pub fn record_crash(&mut self, action: &str, message: &str) {
        let bodies = self.native_export_bodies();
        let sketches = self.plane_ordered_sketches();
        let datums = self.native_datums();
        let res = write_report_in(&dir(), action, message, |doc| {
            ducad_io::native::save_full(
                doc,
                &sketches,
                &bodies,
                self.design.as_ref(),
                Some(&self.ink),
                Some(&datums),
            )
            .is_ok()
        });
        match res {
            Ok(p) => log::error!("laporan crash ditulis: {}", p.display()),
            Err(e) => log::warn!("gagal menulis laporan crash: {e}"),
        }
    }

    /// Jendela "Laporan crash".
    pub fn show_crash_reports(&mut self, ctx: &egui::Context) {
        if !self.crash_reports.open {
            return;
        }
        let reports = list_in(&dir());
        let mut open = true;
        let mut delete: Option<PathBuf> = None;
        let mut copy: Option<String> = None;
        egui::Window::new(ducad_i18n::t!("crash-title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(520.0)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new(ducad_i18n::t!("crash-help")).small());
                if reports.is_empty() {
                    ui.label(ducad_i18n::t!("crash-empty"));
                }
                egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                    for (path, r) in &reports {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.strong(&r.action);
                                ui.label(egui::RichText::new(&r.ts).small().weak());
                            });
                            ui.label(&r.message);
                            ui.horizontal(|ui| {
                                if !r.document.is_empty() {
                                    let doc = path.with_extension("ducad");
                                    if ui.button(ducad_i18n::t!("crash-copy-replay")).clicked() {
                                        copy = Some(format!(
                                            "ducad-cli replay \"{}\"",
                                            doc.display()
                                        ));
                                    }
                                }
                                if ui.button(ducad_i18n::t!("crash-copy-path")).clicked() {
                                    copy = Some(path.display().to_string());
                                }
                                if ui
                                    .button(egui_icons::icons::ICON_DELETE.codepoint)
                                    .on_hover_text(ducad_i18n::t!("crash-delete"))
                                    .clicked()
                                {
                                    delete = Some(path.clone());
                                }
                            });
                        });
                    }
                });
            });
        if let Some(text) = copy {
            ctx.copy_text(text);
            self.file_status = Some(ducad_i18n::t!("crash-copied"));
        }
        if let Some(p) = delete {
            delete_report(&p);
        }
        if !open {
            self.crash_reports.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_list_prune_delete() {
        let d = std::env::temp_dir().join(format!("ducad-crash-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let p = write_report_in(&d, "Fillet", "index out of bounds", |doc| {
            std::fs::write(doc, b"{}").is_ok()
        })
        .unwrap();
        let list = list_in(&d);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].1.action, "Fillet");
        assert!(p.with_extension("ducad").is_file());
        // Dokumen gagal ditulis → laporan tetap ada, tanpa nama dokumen.
        std::thread::sleep(std::time::Duration::from_millis(5));
        write_report_in(&d, "Shell", "x", |_| false).unwrap();
        assert!(list_in(&d)[0].1.document.is_empty());
        prune_in(&d, 1);
        assert_eq!(list_in(&d).len(), 1);
        delete_report(&list_in(&d)[0].0);
        assert!(list_in(&d).is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn panicking_model_command_writes_report_and_keeps_app() {
        struct Boom;
        impl ducad_core::Command<crate::model::ModelDoc> for Boom {
            fn name(&self) -> &str {
                "Boom"
            }
            fn apply(&mut self, _: &mut crate::model::ModelDoc) {
                panic!("sengaja");
            }
            fn revert(&mut self, _: &mut crate::model::ModelDoc) {}
        }
        let before = list_in(&dir()).len();
        let mut app = crate::app::DuCADApp::new_for_test();
        app.execute_model_command(Box::new(Boom), "uji");
        assert!(app.file_status.is_some());
        let after = list_in(&dir());
        assert!(after.len() > before || after.len() == MAX_REPORTS);
        if let Some((p, r)) = after.iter().find(|(_, r)| r.action == "Boom") {
            assert!(r.message.contains("sengaja"));
            delete_report(p);
        }
    }
}
