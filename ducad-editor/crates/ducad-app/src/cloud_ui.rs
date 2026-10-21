//! Lisensi (tier → fitur) dan Cloud Sync di aplikasi.
//!
//! - [`DuCADApp::license_tier`] / [`DuCADApp::require_feature`]: satu pintu
//!   pembatasan fitur Pro untuk GUI dan jembatan agent.
//! - [`CloudCtl`]: permintaan ke server dijalankan di thread latar; hasilnya
//!   diambil setiap frame oleh [`DuCADApp::poll_cloud`].
//! - [`PrefsState`]: preferensi pengguna (bahasa, satuan tampilan, ukuran
//!   ikon) disimpan lokal di `prefs.json` dan disinkronkan ke akun.
//!
//! Konflik berkas tidak pernah diselesaikan diam-diam: unggahan memakai
//! revisi terakhir yang dilihat perangkat ini, dan server menolak (409) bila
//! perangkat lain sudah mengubahnya. Unduhan tidak menimpa berkas lokal
//! yang isinya berbeda.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

use ducad_cloud::{CloudFile, CloudSession, DucadAccount, FileList, RemotePrefs, SyncError};
use ducad_core::{Feature, LengthUnit, LicenseTier};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::app::DuCADApp;

// ---------------------------------------------------------------------------
// Preferensi
// ---------------------------------------------------------------------------

/// Preferensi yang ikut berpindah antar perangkat. `None` = belum pernah
/// diatur, sehingga tidak menimpa nilai perangkat lain.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserPrefs {
    pub language: Option<ducad_i18n::Language>,
    pub unit: Option<LengthUnit>,
    pub icon_size: Option<f32>,
}

/// Isi `prefs.json`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrefsState {
    pub prefs: UserPrefs,
    /// Revisi server yang terakhir diterapkan/diunggah perangkat ini.
    pub synced_revision: i64,
    /// Ada perubahan lokal yang belum terunggah.
    pub dirty: bool,
}

impl PrefsState {
    pub fn default_path() -> PathBuf {
        crate::platform::data_dir().join("prefs.json")
    }

    /// Berkas tidak ada atau rusak → bawaan (tidak menggagalkan start).
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) {
        let write = || -> anyhow::Result<()> {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(path, serde_json::to_string_pretty(self)?)?;
            Ok(())
        };
        if let Err(e) = write() {
            log::warn!("prefs.json tidak bisa ditulis: {e}");
        }
    }

    /// Apa yang dilakukan dengan preferensi dari server.
    pub fn reconcile(&self, remote: &RemotePrefs) -> PrefsAction {
        if self.dirty {
            // Perubahan lokal terakhir menang (last-write-wins).
            return PrefsAction::Push;
        }
        match &remote.prefs {
            Some(v) if remote.revision > self.synced_revision => {
                match serde_json::from_value::<UserPrefs>(v.clone()) {
                    Ok(p) => PrefsAction::Apply(p, remote.revision),
                    Err(_) => PrefsAction::Nothing,
                }
            }
            // Akun belum punya preferensi: unggah milik perangkat ini.
            None if self.prefs != UserPrefs::default() => PrefsAction::Push,
            _ => PrefsAction::Nothing,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PrefsAction {
    Apply(UserPrefs, i64),
    Push,
    Nothing,
}

// ---------------------------------------------------------------------------
// Revisi berkas yang diketahui perangkat ini
// ---------------------------------------------------------------------------

/// Isi `cloud_sync.json`: revisi cloud terakhir per nama berkas, per akun.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KnownRevisions {
    pub user_id: String,
    /// Kunci: nama berkas huruf kecil (server membandingkan tanpa peka huruf).
    pub revisions: HashMap<String, i64>,
}

impl KnownRevisions {
    fn path() -> PathBuf {
        crate::platform::data_dir().join("cloud_sync.json")
    }

    fn load_for(user_id: &str) -> Self {
        let loaded: Self = std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        if loaded.user_id == user_id {
            loaded
        } else {
            Self { user_id: user_id.to_string(), revisions: HashMap::new() }
        }
    }

    fn save(&self) {
        let path = Self::path();
        let write = || -> anyhow::Result<()> {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(&path, serde_json::to_string_pretty(self)?)?;
            Ok(())
        };
        if let Err(e) = write() {
            log::warn!("cloud_sync.json tidak bisa ditulis: {e}");
        }
    }

    pub fn get(&self, name: &str) -> i64 {
        self.revisions.get(&name.to_lowercase()).copied().unwrap_or(0)
    }

    pub fn set(&mut self, name: &str, revision: i64) {
        self.revisions.insert(name.to_lowercase(), revision);
    }

    pub fn remove(&mut self, name: &str) {
        self.revisions.remove(&name.to_lowercase());
    }
}

// ---------------------------------------------------------------------------
// Pekerjaan latar
// ---------------------------------------------------------------------------

enum Job {
    Refresh,
    Upload { name: String, bytes: Vec<u8>, base_revision: i64, force: bool },
    Download { file: CloudFile },
    Delete { file: CloudFile },
    PullPrefs,
    PushPrefs(Value),
}

enum Outcome {
    Refreshed(FileList),
    Uploaded(CloudFile),
    Downloaded { file: CloudFile, bytes: Vec<u8> },
    Deleted(CloudFile),
    PrefsPulled(RemotePrefs),
    PrefsPushed(RemotePrefs),
}

struct Done {
    /// Akun baru bila token diperbarui atau profil/tier berubah.
    account: Option<DucadAccount>,
    result: Result<Outcome, SyncError>,
    /// Unggahan yang ditolak karena konflik disimpan untuk dipilih pengguna.
    upload: Option<(String, Vec<u8>)>,
}

fn run_job(server_url: &str, account: DucadAccount, job: Job) -> Done {
    let mut s = CloudSession::new(server_url, account);
    let mut upload = None;
    let result = match job {
        Job::Refresh => s
            .refresh_profile()
            .and_then(|_| s.list_files())
            .map(Outcome::Refreshed),
        Job::Upload { name, bytes, base_revision, force } => {
            let r = s.upload_file(&name, &bytes, base_revision, force).map(Outcome::Uploaded);
            if matches!(r, Err(SyncError::Conflict { .. })) {
                upload = Some((name, bytes));
            }
            r
        }
        Job::Download { file } => s.download_file(&file.id).map(|bytes| Outcome::Downloaded { file, bytes }),
        Job::Delete { file } => s.delete_file(&file.id).map(|_| Outcome::Deleted(file)),
        Job::PullPrefs => s
            .refresh_profile()
            .and_then(|_| s.get_prefs())
            .map(Outcome::PrefsPulled),
        Job::PushPrefs(v) => s.put_prefs(&v).map(Outcome::PrefsPushed),
    };
    let account = s.account_changed().then(|| s.into_account());
    Done { account, result, upload }
}

/// Keadaan Cloud Sync di aplikasi.
#[derive(Default)]
pub struct CloudCtl {
    pub files: Option<FileList>,
    pub status: Option<String>,
    pub status_is_error: bool,
    /// Unggahan yang ditolak karena konflik: (nama, isi).
    pub conflict: Option<(String, Vec<u8>)>,
    pub confirm_delete: Option<String>,
    known: KnownRevisions,
    /// Pekerjaan berkas (satu per waktu, supaya tombol tidak saling balap).
    files_job: Option<Receiver<Done>>,
    /// Pekerjaan preferensi (terpisah dari berkas).
    prefs_job: Option<Receiver<Done>>,
    /// Ada perubahan preferensi yang menunggu pekerjaan sebelumnya selesai.
    prefs_push_queued: bool,
}

impl CloudCtl {
    pub fn busy(&self) -> bool {
        self.files_job.is_some()
    }

    fn set_status(&mut self, msg: impl Into<String>, is_error: bool) {
        self.status = Some(msg.into());
        self.status_is_error = is_error;
    }

    /// Lupakan semua keadaan akun (logout).
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

fn spawn(server_url: &str, account: &DucadAccount, job: Job) -> Receiver<Done> {
    let (tx, rx) = channel();
    let url = server_url.to_string();
    let account = account.clone();
    std::thread::spawn(move || {
        let _ = tx.send(run_job(&url, account, job));
    });
    rx
}

/// Nama salinan saat konflik: `braket.ducad` → `braket (salinan 2026-10-20 1030).ducad`.
pub fn conflict_copy_name(name: &str, stamp: &str) -> String {
    let stem = name
        .strip_suffix(".ducad")
        .or_else(|| name.strip_suffix(".DUCAD"))
        .unwrap_or(name);
    format!("{stem} (salinan {stamp}).ducad")
}

/// Path tujuan unduhan di `dir`. Berkas yang sudah ada dengan isi sama
/// dipakai ulang; isi berbeda tidak ditimpa — namanya diberi revisi cloud.
pub fn download_target(dir: &Path, file: &CloudFile, bytes: &[u8]) -> PathBuf {
    let safe: String = file
        .name
        .chars()
        .map(|c| if matches!(c, '/' | '\\' | ':') || c.is_control() { '_' } else { c })
        .collect();
    let primary = dir.join(&safe);
    match std::fs::read(&primary) {
        Err(_) => primary,
        Ok(existing) if existing == bytes => primary,
        Ok(_) => {
            let stem = safe.strip_suffix(".ducad").unwrap_or(&safe);
            dir.join(format!("{stem} (cloud rev {}).ducad", file.revision))
        }
    }
}

// ---------------------------------------------------------------------------
// Integrasi DuCADApp
// ---------------------------------------------------------------------------

impl DuCADApp {
    /// Tier efektif: akun login, atau `Free`. Tes boleh memaksa tier lewat
    /// `license_override`.
    pub fn license_tier(&self) -> LicenseTier {
        self.license_override
            .unwrap_or_else(|| self.account.as_ref().map(|a| a.tier()).unwrap_or_default())
    }

    /// `true` bila fitur boleh dipakai. Bila tidak: pesan di status berkas
    /// dan popup akun dibuka supaya pengguna tahu harus ke mana.
    pub fn require_feature(&mut self, feature: Feature) -> bool {
        match self.license_tier().check(feature) {
            Ok(()) => true,
            Err(msg) => {
                self.file_status = Some(msg);
                self.account_drawer_open = true;
                false
            }
        }
    }

    /// Dipanggil saat start (akun tersimpan) dan setelah login berhasil:
    /// segarkan tier + daftar berkas, lalu sinkronkan preferensi.
    pub fn cloud_on_login(&mut self) {
        let Some(acc) = self.account.clone() else {
            return;
        };
        self.cloud.known = KnownRevisions::load_for(&acc.user_id);
        self.cloud.files_job = Some(spawn(&self.server_url, &acc, Job::Refresh));
        self.cloud.prefs_job = Some(spawn(&self.server_url, &acc, Job::PullPrefs));
    }

    pub fn cloud_on_logout(&mut self) {
        self.cloud.reset();
    }

    pub fn cloud_refresh(&mut self) {
        let Some(acc) = self.account.clone() else {
            return;
        };
        if self.cloud.busy() {
            return;
        }
        self.cloud.files_job = Some(spawn(&self.server_url, &acc, Job::Refresh));
    }

    /// Simpan dokumen aktif lalu unggah isinya.
    pub fn cloud_upload_current(&mut self) {
        let (Some(acc), Some(path)) = (self.account.clone(), self.current_file_path.clone()) else {
            return;
        };
        if self.cloud.busy() {
            return;
        }
        self.save_native_to(path.clone());
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                self.cloud.set_status(format!("Gagal membaca {}: {e}", path.display()), true);
                return;
            }
        };
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("model.ducad")
            .to_string();
        let base_revision = self.cloud.known.get(&name);
        self.cloud.conflict = None;
        self.cloud.set_status(format!("Mengunggah {name}..."), false);
        self.cloud.files_job = Some(spawn(
            &self.server_url,
            &acc,
            Job::Upload { name, bytes, base_revision, force: false },
        ));
    }

    fn cloud_resolve_conflict(&mut self, overwrite: bool) {
        let (Some(acc), Some((name, bytes))) = (self.account.clone(), self.cloud.conflict.take()) else {
            return;
        };
        let (name, base_revision, force) = if overwrite {
            (name, 0, true)
        } else {
            let stamp = chrono::Local::now().format("%Y-%m-%d %H%M").to_string();
            (conflict_copy_name(&name, &stamp), 0, false)
        };
        self.cloud.set_status(format!("Mengunggah {name}..."), false);
        self.cloud.files_job = Some(spawn(
            &self.server_url,
            &acc,
            Job::Upload { name, bytes, base_revision, force },
        ));
    }

    fn cloud_file(&self, id: &str) -> Option<CloudFile> {
        self.cloud.files.as_ref()?.files.iter().find(|f| f.id == id).cloned()
    }

    /// Tangani event Cloud dari popup akun.
    pub fn handle_cloud_event(&mut self, ev: &ducad_ui::AccountDrawerEvent) {
        use ducad_ui::AccountDrawerEvent as E;
        let Some(acc) = self.account.clone() else {
            return;
        };
        match ev {
            E::CloudRefresh => self.cloud_refresh(),
            E::CloudUploadCurrent => self.cloud_upload_current(),
            E::CloudOpen(id) => {
                if let (Some(file), false) = (self.cloud_file(id), self.cloud.busy()) {
                    self.cloud.set_status(format!("Mengunduh {}...", file.name), false);
                    self.cloud.files_job = Some(spawn(&self.server_url, &acc, Job::Download { file }));
                }
            }
            E::CloudDelete(id) => self.cloud.confirm_delete = Some(id.clone()),
            E::CloudDeleteCancel => self.cloud.confirm_delete = None,
            E::CloudDeleteConfirm => {
                let file = self.cloud.confirm_delete.take().and_then(|id| self.cloud_file(&id));
                if let (Some(file), false) = (file, self.cloud.busy()) {
                    self.cloud.files_job = Some(spawn(&self.server_url, &acc, Job::Delete { file }));
                }
            }
            E::CloudConflictOverwrite => self.cloud_resolve_conflict(true),
            E::CloudConflictSaveCopy => self.cloud_resolve_conflict(false),
            E::CloudConflictDismiss => {
                self.cloud.conflict = None;
                self.cloud.status = None;
            }
            _ => {}
        }
    }

    // ---- preferensi -------------------------------------------------------

    /// Potret preferensi aplikasi saat ini.
    fn current_user_prefs(&self) -> UserPrefs {
        UserPrefs {
            language: Some(self.language),
            unit: Some(self.unit),
            icon_size: Some(self.icon_size),
        }
    }

    /// Terapkan preferensi (dari `prefs.json` saat start, atau dari akun).
    /// Hanya satuan TAMPILAN yang diubah — satuan dokumen terbuka tidak.
    pub fn apply_user_prefs(&mut self, p: &UserPrefs) {
        if let Some(lang) = p.language {
            self.language = lang;
            ducad_i18n::set_language(lang);
        }
        if let Some(unit) = p.unit {
            self.unit = unit;
        }
        if let Some(sz) = p.icon_size.filter(|s| s.is_finite() && *s > 0.0) {
            self.icon_size = sz;
            self.left_toolbar.icon_size = sz;
        }
    }

    /// Panggil setelah pengguna mengubah bahasa/satuan/ukuran ikon.
    pub fn user_prefs_changed(&mut self) {
        let now = self.current_user_prefs();
        if now == self.prefs.prefs {
            return;
        }
        self.prefs.prefs = now;
        self.prefs.dirty = true;
        self.prefs.save(&self.prefs_path);
        self.queue_prefs_push();
    }

    fn queue_prefs_push(&mut self) {
        let Some(acc) = self.account.clone() else {
            return;
        };
        if self.cloud.prefs_job.is_some() {
            self.cloud.prefs_push_queued = true;
            return;
        }
        let value = serde_json::to_value(&self.prefs.prefs).unwrap_or(Value::Null);
        self.cloud.prefs_job = Some(spawn(&self.server_url, &acc, Job::PushPrefs(value)));
    }

    // ---- poll -------------------------------------------------------------

    /// Ambil hasil pekerjaan latar (dipanggil setiap frame).
    pub fn poll_cloud(&mut self, ctx: &egui::Context) {
        if self.cloud.files_job.is_some() || self.cloud.prefs_job.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if let Some(done) = self.cloud.files_job.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.cloud.files_job = None;
            self.cloud_finish(done);
        }
        if let Some(done) = self.cloud.prefs_job.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.cloud.prefs_job = None;
            self.cloud_finish(done);
            if std::mem::take(&mut self.cloud.prefs_push_queued) {
                self.queue_prefs_push();
            }
        }
    }

    fn cloud_finish(&mut self, done: Done) {
        if let Some(acc) = done.account {
            // Logout di tengah jalan: jangan hidupkan lagi sesi yang dibuang.
            if self.account.is_some() {
                let _ = ducad_cloud::save_account(&acc);
                self.account = Some(acc);
            }
        }
        if self.account.is_none() {
            return;
        }
        match done.result {
            Ok(Outcome::Refreshed(list)) => {
                self.cloud.files = Some(list);
                if !self.cloud.status_is_error {
                    self.cloud.status = None;
                }
            }
            Ok(Outcome::Uploaded(file)) => {
                self.cloud.known.set(&file.name, file.revision);
                self.cloud.known.save();
                self.cloud.set_status(format!("{} terunggah (revisi {}).", file.name, file.revision), false);
                self.cloud_refresh();
            }
            Ok(Outcome::Downloaded { file, bytes }) => self.cloud_open_downloaded(file, bytes),
            Ok(Outcome::Deleted(file)) => {
                self.cloud.known.remove(&file.name);
                self.cloud.known.save();
                self.cloud.set_status(format!("{} dihapus dari cloud.", file.name), false);
                self.cloud_refresh();
            }
            Ok(Outcome::PrefsPulled(remote)) => match self.prefs.reconcile(&remote) {
                PrefsAction::Apply(p, revision) => {
                    self.apply_user_prefs(&p);
                    self.prefs.prefs = self.current_user_prefs();
                    self.prefs.synced_revision = revision;
                    self.prefs.save(&self.prefs_path);
                }
                PrefsAction::Push => self.queue_prefs_push(),
                PrefsAction::Nothing => {}
            },
            Ok(Outcome::PrefsPushed(remote)) => {
                self.prefs.synced_revision = remote.revision;
                self.prefs.dirty = false;
                self.prefs.save(&self.prefs_path);
            }
            Err(SyncError::Conflict { message, .. }) => {
                self.cloud.conflict = done.upload;
                self.cloud.set_status(message, true);
            }
            Err(e) => self.cloud.set_status(e.message(), true),
        }
    }

    fn cloud_open_downloaded(&mut self, file: CloudFile, bytes: Vec<u8>) {
        let dir = crate::platform::documents_dir().join("DUCAD Cloud");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.cloud.set_status(format!("Folder {} tidak bisa dibuat: {e}", dir.display()), true);
            return;
        }
        let target = download_target(&dir, &file, &bytes);
        if let Err(e) = std::fs::write(&target, &bytes) {
            self.cloud.set_status(format!("Gagal menyimpan {}: {e}", target.display()), true);
            return;
        }
        // Revisi hanya dicatat bila berkas tersimpan dengan nama aslinya;
        // salinan "(cloud rev N)" adalah berkas lain.
        if target.file_name().and_then(|n| n.to_str()) == Some(file.name.as_str()) {
            self.cloud.known.set(&file.name, file.revision);
            self.cloud.known.save();
        }
        self.cloud.set_status(format!("{} diunduh ke {}.", file.name, dir.display()), false);
        self.open_native_path(target);
    }

    /// Data popup akun.
    pub fn cloud_view(&self) -> ducad_ui::CloudView<'_> {
        ducad_ui::CloudView {
            tier: self.license_tier(),
            files: self.cloud.files.as_ref(),
            busy: self.cloud.busy(),
            status: self.cloud.status.as_deref(),
            status_is_error: self.cloud.status_is_error,
            current_document: self
                .current_file_path
                .as_ref()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str()),
            conflict: self.cloud.conflict.as_ref().map(|(n, _)| n.as_str()),
            confirm_delete: self.cloud.confirm_delete.as_deref(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote(prefs: Option<Value>, revision: i64) -> RemotePrefs {
        RemotePrefs { prefs, revision, updated_at: None }
    }

    #[test]
    fn newer_remote_prefs_are_applied_when_local_is_clean() {
        let local = PrefsState { synced_revision: 1, ..Default::default() };
        let r = remote(Some(serde_json::json!({ "icon_size": 30.0 })), 2);
        assert_eq!(
            local.reconcile(&r),
            PrefsAction::Apply(UserPrefs { icon_size: Some(30.0), ..Default::default() }, 2)
        );
    }

    #[test]
    fn local_changes_win_and_stale_remote_is_ignored() {
        let dirty = PrefsState { dirty: true, ..Default::default() };
        assert_eq!(dirty.reconcile(&remote(Some(serde_json::json!({})), 9)), PrefsAction::Push);

        let clean = PrefsState { synced_revision: 5, ..Default::default() };
        assert_eq!(clean.reconcile(&remote(Some(serde_json::json!({})), 5)), PrefsAction::Nothing);
    }

    #[test]
    fn empty_account_receives_this_devices_prefs() {
        let local = PrefsState {
            prefs: UserPrefs { icon_size: Some(20.0), ..Default::default() },
            ..Default::default()
        };
        assert_eq!(local.reconcile(&remote(None, 0)), PrefsAction::Push);
        assert_eq!(PrefsState::default().reconcile(&remote(None, 0)), PrefsAction::Nothing);
    }

    #[test]
    fn prefs_state_round_trips_through_disk() {
        let dir = std::env::temp_dir().join(format!("ducad-prefs-{}", std::process::id()));
        let path = dir.join("prefs.json");
        let s = PrefsState {
            prefs: UserPrefs {
                language: Some(ducad_i18n::Language::default()),
                unit: Some(LengthUnit::Millimeters),
                icon_size: Some(24.0),
            },
            synced_revision: 3,
            dirty: true,
        };
        s.save(&path);
        assert_eq!(PrefsState::load(&path), s);
        assert_eq!(PrefsState::load(&dir.join("tidak-ada.json")), PrefsState::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn conflict_copy_keeps_extension() {
        assert_eq!(
            conflict_copy_name("braket.ducad", "2026-10-20 1030"),
            "braket (salinan 2026-10-20 1030).ducad"
        );
    }

    #[test]
    fn download_never_overwrites_different_local_content() {
        let dir = std::env::temp_dir().join(format!("ducad-dl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = CloudFile {
            id: "f".into(),
            name: "braket.ducad".into(),
            size: 3,
            sha256: String::new(),
            revision: 4,
            updated_at: String::new(),
        };
        assert_eq!(download_target(&dir, &file, b"abc"), dir.join("braket.ducad"));
        std::fs::write(dir.join("braket.ducad"), b"abc").unwrap();
        assert_eq!(download_target(&dir, &file, b"abc"), dir.join("braket.ducad"));
        assert_eq!(download_target(&dir, &file, b"xyz"), dir.join("braket (cloud rev 4).ducad"));
        let evil = CloudFile { name: "../luar.ducad".into(), ..file };
        assert!(download_target(&dir, &evil, b"x").starts_with(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn known_revisions_are_case_insensitive() {
        let mut k = KnownRevisions::default();
        k.set("Braket.ducad", 3);
        assert_eq!(k.get("braket.DUCAD"), 3);
        k.remove("BRAKET.ducad");
        assert_eq!(k.get("braket.ducad"), 0);
    }

    #[test]
    fn free_tier_blocks_pro_features_in_app() {
        let mut app = DuCADApp::new_for_test();
        app.license_override = Some(LicenseTier::Free);
        assert!(!app.require_feature(Feature::ExportStep));
        assert!(app.file_status.as_deref().unwrap_or_default().contains("Pro"));
        assert!(app.account_drawer_open, "popup akun dibuka agar pengguna tahu harus ke mana");

        app.license_override = Some(LicenseTier::Pro);
        app.file_status = None;
        assert!(app.require_feature(Feature::ExportStep));
        assert!(app.file_status.is_none());
    }

    #[test]
    fn logged_out_app_is_free() {
        let mut app = DuCADApp::new_for_test();
        app.license_override = None;
        app.account = None;
        assert_eq!(app.license_tier(), LicenseTier::Free);
    }
}
