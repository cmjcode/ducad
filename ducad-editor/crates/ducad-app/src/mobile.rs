//! Perilaku tablet yang tidak bergantung UIKit/Android: autosave pemulihan,
//! peringatan memori, dialog berkas asinkron, dan event Apple Pencil.
//!
//! Dipakai di semua platform (desktop hanya lewat `request_open`, yang di
//! sana langsung memanggil dialog `rfd`), supaya jalur kode yang diuji di
//! macOS sama dengan yang berjalan di iPad/Android.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use crate::app::DuCADApp;
use crate::types::ToolKind;

/// Untuk apa berkas diminta; menentukan handler saat picker asinkron
/// (iPad) mengembalikan jalur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenPurpose {
    Native,
    Step,
    Stl,
    Dxf,
    DesignTableCsv,
    ExternalPart,
    Font,
}

/// Event dari jembatan Apple Pencil (`apple_ios.rs`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PencilEvent {
    DoubleTap,
    Hover(egui::Pos2),
    HoverEnded,
}

/// Keadaan tablet yang hidup di `DuCADApp`.
#[derive(Default)]
pub struct MobileState {
    /// Picker berkas yang sedang terbuka (iPad).
    pub pending_open: Option<(OpenPurpose, Receiver<Option<PathBuf>>)>,
    /// Berkas ekspor yang akan ditawarkan ke share sheet begitu tertulis.
    pub pending_share: Option<(PathBuf, Instant)>,
    pub pencil_rx: Option<Receiver<PencilEvent>>,
    /// Posisi hover Pencil (poin egui) sebelum ujung menyentuh layar.
    pub pencil_hover: Option<egui::Pos2>,
    /// Tool sebelum ketuk ganda Pencil memindah ke Pilih.
    pub pencil_prev_tool: Option<ToolKind>,
    /// Stempel undo saat autosave terakhir; menghindari tulis ulang bila
    /// tidak ada perubahan.
    pub last_autosave_stamp: Option<u64>,
    pub restored_from_autosave: bool,
    /// Ada sentuhan layar yang sedang berlangsung (jari atau Pencil).
    pub touch_pointer_active: bool,
    /// Sentuhan yang berlangsung berasal dari Apple Pencil (ada tekanan).
    pub pencil_touch_active: bool,
}

impl MobileState {
    /// Perbarui jenis pointer sentuh dari event egui frame ini. Jari di iPad
    /// tidak melaporkan tekanan (`force == None`), Pencil melaporkan `> 0`
    /// sejak ujungnya menyentuh layar.
    pub fn observe_touch_events(&mut self, events: &[egui::Event], any_touches: bool) {
        for ev in events {
            if let egui::Event::Touch { phase, force, .. } = ev {
                match phase {
                    egui::TouchPhase::Start => {
                        self.touch_pointer_active = true;
                        if force.is_some_and(|f| f > 0.0) {
                            self.pencil_touch_active = true;
                        }
                    }
                    egui::TouchPhase::Move => {
                        if force.is_some_and(|f| f > 0.0) {
                            self.pencil_touch_active = true;
                        }
                    }
                    egui::TouchPhase::End | egui::TouchPhase::Cancel => {}
                }
            }
        }
        if !any_touches {
            self.touch_pointer_active = false;
            self.pencil_touch_active = false;
        }
    }
}

/// Berkas terbaru di `dir` dengan salah satu ekstensi `extensions`
/// (fallback saat tidak ada picker: Android, atau picker iPad gagal tampil).
pub fn newest_matching_file(dir: &Path, extensions: &[&str]) -> Option<PathBuf> {
    let read_dir = std::fs::read_dir(dir).ok()?;
    let mut matching: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)) {
            let modified = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            matching.push((modified, path));
        }
    }
    matching.sort_by_key(|(m, _)| *m);
    matching.pop().map(|(_, p)| p)
}

/// Hapus berkas autosave (dokumen sudah tersimpan eksplisit).
pub fn clear_autosave() {
    let _ = std::fs::remove_file(crate::platform::autosave_path());
    let _ = std::fs::remove_file(crate::platform::autosave_origin_path());
}

impl DuCADApp {
    /// Inisialisasi tablet setelah konstruksi: observer OS, jembatan
    /// Pencil, dan pemulihan autosave.
    pub fn init_mobile(&mut self, cc: &eframe::CreationContext<'_>) {
        #[cfg(target_os = "ios")]
        {
            // Jendela winit dibuat tanpa `windowScene`; di bawah siklus hidup
            // UIScene jendela seperti itu tidak pernah tampil di layar.
            crate::apple_ios::attach_window_to_scene(cc);
            crate::apple_ios::install_memory_warning_observer();
            let (tx, rx) = std::sync::mpsc::channel();
            if crate::apple_ios::install_pencil_bridge(cc, cc.egui_ctx.clone(), tx) {
                self.mobile.pencil_rx = Some(rx);
            }
        }
        #[cfg(not(target_os = "ios"))]
        let _ = cc;
        self.restore_autosave();
    }

    /// Jam logis perubahan dokumen: stempel undo tertinggi dari tiga tumpukan.
    pub fn edit_stamp(&self) -> u64 {
        [
            self.sketch_set.active().undo.top_undo_stamp(),
            self.model_undo.top_undo_stamp(),
            self.ink_undo.top_undo_stamp(),
        ]
        .into_iter()
        .flatten()
        .max()
        .unwrap_or(0)
    }

    /// Tulis `autosave.ducad` bila ada perubahan sejak autosave terakhir.
    /// Dipanggil eframe saat OS menidurkan aplikasi dan berkala
    /// (`App::save`). Mengembalikan `true` bila berkas ditulis.
    pub fn write_autosave(&mut self) -> bool {
        let stamp = self.edit_stamp();
        let dirty = self.model.doc.dirty;
        if stamp == 0 && !dirty && self.current_file_path.is_none() {
            return false;
        }
        if self.mobile.last_autosave_stamp == Some(stamp) && !dirty {
            return false;
        }
        let path = crate::platform::autosave_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let body_exports = self.native_export_bodies();
        let ordered = self.plane_ordered_sketches();
        match ducad_io::native::save_multi_plane_detailed_with_design_and_ink(
            &path,
            &ordered,
            &body_exports,
            self.design.as_ref(),
            Some(&self.ink),
        ) {
            Ok(_) => {
                let origin = self
                    .current_file_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let _ = std::fs::write(crate::platform::autosave_origin_path(), origin);
                self.mobile.last_autosave_stamp = Some(stamp);
                true
            }
            Err(e) => {
                log::warn!("autosave gagal: {e}");
                false
            }
        }
    }

    /// Pulihkan dokumen dari autosave (tablet saja: di sana OS mematikan
    /// aplikasi tanpa pemberitahuan).
    pub fn restore_autosave(&mut self) {
        if !crate::platform::is_mobile() {
            return;
        }
        let path = crate::platform::autosave_path();
        if !path.is_file() {
            return;
        }
        self.open_native_path(path);
        let origin = std::fs::read_to_string(crate::platform::autosave_origin_path())
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .filter(|p| p.is_file());
        self.current_file_path = origin;
        self.file_status = Some(ducad_i18n::t!("file-restored-autosave"));
        self.mobile.restored_from_autosave = true;
    }

    /// Dipanggil di awal tiap frame.
    pub fn poll_mobile(&mut self, ctx: &egui::Context) {
        if crate::platform::take_memory_warning() {
            self.trim_memory(ctx);
        }
        ctx.input(|i| self.mobile.observe_touch_events(&i.events, i.any_touches()));
        self.poll_file_picker();
        self.poll_pencil();
        self.poll_share();
    }

    /// Bebaskan cache yang bisa dibangun ulang; dipanggil saat OS memberi
    /// peringatan memori.
    pub fn trim_memory(&mut self, ctx: &egui::Context) {
        self.ink_render = Default::default();
        self.round_preview_cache = None;
        self.liquid_glass = false;
        ctx.forget_all_images();
        self.file_status = Some(ducad_i18n::t!("mobile-memory-trimmed"));
        log::warn!("peringatan memori OS: cache dibebaskan");
    }

    /// Minta berkas untuk dibuka. Desktop: dialog sinkron. iPad: picker
    /// Files.app asinkron (hasil diproses `poll_file_picker`), fallback
    /// berkas terbaru di folder Dokumen. Android: berkas terbaru di folder
    /// Dokumen (SAF belum dijembatani).
    pub fn request_open(
        &mut self,
        purpose: OpenPurpose,
        filter_name: &str,
        extensions: &[&str],
    ) -> Option<PathBuf> {
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        {
            let _ = purpose;
            self.pick_open_path(filter_name, extensions)
        }
        #[cfg(target_os = "ios")]
        {
            let _ = filter_name;
            let (tx, rx) = std::sync::mpsc::channel();
            if crate::apple_ios::present_document_picker(extensions, tx) {
                self.mobile.pending_open = Some((purpose, rx));
                None
            } else {
                newest_matching_file(&crate::platform::documents_dir(), extensions)
            }
        }
        #[cfg(target_os = "android")]
        {
            let _ = (purpose, filter_name);
            newest_matching_file(&crate::platform::documents_dir(), extensions)
        }
    }

    fn poll_file_picker(&mut self) {
        let Some((purpose, rx)) = self.mobile.pending_open.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(Some(path)) => self.open_picked(purpose, path),
            Ok(None) | Err(TryRecvError::Disconnected) => {}
            Err(TryRecvError::Empty) => self.mobile.pending_open = Some((purpose, rx)),
        }
    }

    /// Jalankan aksi yang tertunda begitu picker memberi jalur.
    pub fn open_picked(&mut self, purpose: OpenPurpose, path: PathBuf) {
        match purpose {
            OpenPurpose::Native => self.open_native_path(path),
            OpenPurpose::Step => self.import_step_path(path),
            OpenPurpose::Stl => self.import_stl_path(path),
            OpenPurpose::Dxf => self.import_dxf_path(path),
            OpenPurpose::DesignTableCsv => self.import_design_table_csv_path(path),
            OpenPurpose::ExternalPart => self.add_external_part_path(path),
            OpenPurpose::Font => self.load_custom_font_path(path),
        }
    }

    fn poll_pencil(&mut self) {
        let Some(rx) = self.mobile.pencil_rx.as_ref() else {
            return;
        };
        let mut events = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            events.push(ev);
        }
        for ev in events {
            match ev {
                PencilEvent::Hover(pos) => self.mobile.pencil_hover = Some(pos),
                PencilEvent::HoverEnded => self.mobile.pencil_hover = None,
                PencilEvent::DoubleTap => self.pencil_double_tap(),
            }
        }
    }

    /// Benar bila sentuhan 1 jari saat ini harus dialihkan ke navigasi kanvas
    /// (mode PencilOnly) — seretan Pencil dan mouse tidak termasuk.
    pub fn finger_navigation_active(&self) -> bool {
        self.touch_config.single_finger_navigates()
            && self.mobile.touch_pointer_active
            && !self.mobile.pencil_touch_active
    }

    /// Ketuk ganda Pencil: bolak-balik antara tool aktif dan Pilih.
    pub fn pencil_double_tap(&mut self) {
        if self.tool == ToolKind::Select {
            if let Some(prev) = self.mobile.pencil_prev_tool.take() {
                self.set_tool(prev);
            }
        } else {
            self.mobile.pencil_prev_tool = Some(self.tool);
            self.set_tool(ToolKind::Select);
        }
        #[cfg(target_os = "ios")]
        crate::apple_ios::haptic_selection_changed();
    }

    /// Tawarkan share sheet begitu berkas ekspor selesai ditulis (iPad).
    fn poll_share(&mut self) {
        let Some((path, requested_at)) = self.mobile.pending_share.take() else {
            return;
        };
        let written = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .map(|m| {
                m.elapsed()
                    .map(|age| age < requested_at.elapsed())
                    .unwrap_or(true)
            })
            .unwrap_or(false);
        if written {
            #[cfg(target_os = "ios")]
            crate::apple_ios::share_file(&path, None);
            #[cfg(not(target_os = "ios"))]
            let _ = &path;
        } else if requested_at.elapsed() < Duration::from_secs(30) {
            self.mobile.pending_share = Some((path, requested_at));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_matching_file_picks_latest_by_extension() {
        let dir = std::env::temp_dir().join(format!("ducad-mobile-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("a.ducad"), b"a").unwrap();
        std::thread::sleep(Duration::from_millis(20));
        std::fs::write(dir.join("b.ducad"), b"b").unwrap();
        std::fs::write(dir.join("c.step"), b"c").unwrap();
        let newest = newest_matching_file(&dir, &["ducad"]).unwrap();
        assert_eq!(newest.file_name().unwrap(), "b.ducad");
        assert!(newest_matching_file(&dir, &["dxf"]).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pencil_double_tap_toggles_between_tool_and_select() {
        let mut app = DuCADApp::new_for_test();
        app.set_tool(ToolKind::Line);
        app.pencil_double_tap();
        assert_eq!(app.tool, ToolKind::Select);
        app.pencil_double_tap();
        assert_eq!(app.tool, ToolKind::Line);
    }

    #[test]
    fn finger_navigation_only_for_finger_in_pencil_only_mode() {
        let touch = |phase, force| egui::Event::Touch {
            device_id: egui::TouchDeviceId(0),
            id: egui::TouchId(1),
            phase,
            pos: egui::pos2(10.0, 10.0),
            force,
        };
        let mut app = DuCADApp::new_for_test();
        app.touch_config.set_mode(ducad_ui::TouchDesignMode::PencilOnly);

        app.mobile.observe_touch_events(&[touch(egui::TouchPhase::Start, None)], true);
        assert!(app.finger_navigation_active(), "jari harus menavigasi");
        app.mobile.observe_touch_events(&[touch(egui::TouchPhase::End, None)], false);
        assert!(!app.finger_navigation_active());

        app.mobile.observe_touch_events(&[touch(egui::TouchPhase::Start, Some(0.4))], true);
        assert!(!app.finger_navigation_active(), "Pencil harus tetap menggambar");
        app.mobile.observe_touch_events(&[], false);

        app.touch_config.set_mode(ducad_ui::TouchDesignMode::PencilAndFinger);
        app.mobile.observe_touch_events(&[touch(egui::TouchPhase::Start, None)], true);
        assert!(!app.finger_navigation_active(), "mode hibrida: jari mendesain");
    }

    #[test]
    fn autosave_skips_untouched_document() {
        let mut app = DuCADApp::new_for_test();
        assert!(!app.write_autosave());
    }
}
