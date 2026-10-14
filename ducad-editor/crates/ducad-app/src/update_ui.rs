//! Pembaruan otomatis dari GitHub Releases (desktop): pemeriksaan latar
//! saat start, jendela "Pembaruan DUCAD", dan mulai ulang ke versi baru.
//!
//! Jaringan dan pemasangan berjalan di thread latar lewat `ducad_update`;
//! frame GUI hanya memeriksa kanal. Mulai ulang membawa dokumen yang terbuka
//! lewat autosave pemulihan (`DuCADApp::on_exit` menulisnya alih-alih
//! menghapusnya), dan `pending_notice` di `update.json` memberi tahu
//! instans baru bahwa ia hasil pembaruan.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::Duration;

use ducad_i18n::t;
use ducad_ui::{card_frame, glass_frame, ACCENT_BLUE, ACCENT_GREEN, ACCENT_ORANGE, TEXT_PRIMARY, TEXT_SECONDARY};
use ducad_update::{Installed, Progress, Release, Unsupported, UpdateError, UpdateSettings};
use egui::{Align2, RichText, Vec2, Window};
use egui_icons::icons::{
    ICON_CHECK_CIRCLE, ICON_DOWNLOAD, ICON_ERROR, ICON_OPEN_IN_NEW, ICON_RESTART_ALT, ICON_SYSTEM_UPDATE,
    ICON_WARNING,
};

/// Versi yang sedang berjalan.
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

enum Msg {
    Checked(Result<Option<Release>, UpdateError>),
    Installed(Result<Installed, UpdateError>),
}

enum Phase {
    Idle,
    Checking,
    UpToDate,
    /// `blocker`: alasan (sudah diterjemahkan) pembaruan tidak bisa dipasang
    /// otomatis; pengguna diarahkan ke halaman unduhan.
    Available { release: Release, blocker: Option<String> },
    Installing(Release),
    Ready(Installed),
    Failed { message: String, page_url: Option<String> },
}

/// Permintaan ke aplikasi dari jendela pembaruan.
pub enum UpdateRequest {
    /// Tutup aplikasi lalu jalankan versi yang baru dipasang.
    Restart,
}

pub struct UpdateCtl {
    /// `None` = updater mati (tablet, tes).
    dir: Option<PathBuf>,
    ctx: Option<egui::Context>,
    settings: UpdateSettings,
    phase: Phase,
    rx: Option<Receiver<Msg>>,
    progress: Arc<Progress>,
    open: bool,
    /// Pemeriksaan dipicu pengguna: hasil "sudah terbaru"/galat ikut tampil,
    /// dan versi yang pernah dilewati tetap ditawarkan.
    manual: bool,
    restart_requested: bool,
}

impl UpdateCtl {
    pub fn disabled() -> Self {
        Self {
            dir: None,
            ctx: None,
            settings: UpdateSettings::default(),
            phase: Phase::Idle,
            rx: None,
            progress: Arc::default(),
            open: false,
            manual: false,
            restart_requested: false,
        }
    }

    /// Updater desktop; mati di tablet (pembaruan lewat App Store/Play).
    pub fn load(ctx: &egui::Context) -> Self {
        if crate::platform::is_mobile() {
            return Self::disabled();
        }
        ducad_update::cleanup_leftovers();
        let dir = crate::platform::data_dir();
        Self {
            settings: UpdateSettings::load(&dir),
            dir: Some(dir),
            ctx: Some(ctx.clone()),
            ..Self::disabled()
        }
    }

    pub fn enabled(&self) -> bool {
        self.dir.is_some()
    }

    /// Versi hasil pembaruan sebelum mulai ulang; dikembalikan sekali.
    pub fn take_notice(&mut self) -> Option<String> {
        let notice = self.settings.pending_notice.take()?;
        self.save();
        Some(notice)
    }

    /// Pemeriksaan latar saat start, bila diaktifkan dan sudah jatuh tempo.
    pub fn check_on_startup(&mut self) {
        if self.enabled() && self.settings.check_due(ducad_update::now_unix()) {
            self.spawn_check(false);
        }
    }

    /// Pemeriksaan dari perintah "Periksa pembaruan…".
    pub fn check_now(&mut self) {
        if !self.enabled() {
            return;
        }
        self.open = true;
        match self.phase {
            // Sudah berjalan / sudah ada hasil yang lebih maju: cukup tampilkan.
            Phase::Checking | Phase::Installing(_) | Phase::Ready(_) => {}
            _ => self.spawn_check(true),
        }
    }

    /// Versi baru yang sudah terpasang dan diminta dijalankan; dipanggil
    /// `on_exit`. Menandai `pending_notice` untuk instans berikutnya.
    pub fn take_restart(&mut self) -> Option<Installed> {
        if !self.restart_requested {
            return None;
        }
        let Phase::Ready(installed) = &self.phase else {
            return None;
        };
        let installed = installed.clone();
        self.settings.pending_notice = Some(installed.version.to_string());
        self.save();
        Some(installed)
    }

    fn save(&self) {
        if let Some(dir) = &self.dir {
            if let Err(e) = self.settings.save(dir) {
                log::warn!("update.json tidak tersimpan: {e}");
            }
        }
    }

    fn spawn<F>(&mut self, name: &str, job: F)
    where
        F: FnOnce() -> Msg + Send + 'static,
    {
        let (tx, rx) = mpsc::channel();
        let ctx = self.ctx.clone();
        let spawned = std::thread::Builder::new().name(name.into()).spawn(move || {
            let _ = tx.send(job());
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
        match spawned {
            Ok(_) => self.rx = Some(rx),
            Err(e) => {
                self.phase = Phase::Failed { message: t!("update-err-io", detail = e.to_string()), page_url: None };
            }
        }
    }

    fn spawn_check(&mut self, manual: bool) {
        self.manual = manual;
        self.phase = Phase::Checking;
        self.settings.last_check_unix = Some(ducad_update::now_unix());
        self.save();
        self.spawn("ducad-update-check", || Msg::Checked(ducad_update::check_latest(CURRENT_VERSION)));
    }

    fn start_install(&mut self, release: Release) {
        self.progress = Arc::default();
        let progress = Arc::clone(&self.progress);
        let job_release = release.clone();
        self.phase = Phase::Installing(release);
        self.spawn("ducad-update-install", move || {
            Msg::Installed(ducad_update::install(&job_release, &progress))
        });
    }

    fn poll(&mut self) {
        let Some(rx) = &self.rx else {
            return;
        };
        let msg = match rx.try_recv() {
            Ok(msg) => msg,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                self.rx = None;
                if matches!(self.phase, Phase::Checking | Phase::Installing(_)) {
                    self.phase = Phase::Idle;
                }
                return;
            }
        };
        self.rx = None;
        match msg {
            Msg::Checked(Ok(Some(release))) => {
                let skipped = self.settings.skipped_version.as_deref() == Some(&release.version.to_string());
                if skipped && !self.manual {
                    self.phase = Phase::Idle;
                    return;
                }
                let blocker = install_blocker(&release);
                self.phase = Phase::Available { release, blocker };
                self.open = true;
            }
            Msg::Checked(Ok(None)) => {
                self.phase = if self.manual { Phase::UpToDate } else { Phase::Idle };
            }
            Msg::Checked(Err(e)) => {
                log::info!("Pemeriksaan pembaruan gagal: {e}");
                self.phase = if self.manual {
                    Phase::Failed { message: error_text(&e), page_url: None }
                } else {
                    Phase::Idle
                };
            }
            Msg::Installed(result) => {
                let page_url = match &self.phase {
                    Phase::Installing(release) => Some(release.page_url.clone()),
                    _ => None,
                };
                self.phase = match result {
                    Ok(installed) => Phase::Ready(installed),
                    Err(e) => {
                        log::warn!("Pembaruan gagal dipasang: {e}");
                        Phase::Failed { message: error_text(&e), page_url }
                    }
                };
                self.open = true;
            }
        }
    }

    /// Dipanggil sekali per frame: ambil hasil thread latar, lalu gambar
    /// jendela pembaruan bila terbuka.
    pub fn frame(&mut self, ctx: &egui::Context) -> Option<UpdateRequest> {
        self.poll();
        if !self.open {
            return None;
        }
        let mut request = None;
        let mut is_open = true;
        let title = t!("update-window-title");
        let area_id = egui::Id::new(title.as_str());
        Window::new(title.as_str())
            .open(&mut is_open)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .resizable(false)
            .collapsible(false)
            .frame(glass_frame().transparent_flat())
            .default_width(420.0)
            .show(ctx, |ui| {
                ducad_ui::glass_window(ui, area_id, |ui| {
                    ui.set_width(420.0);
                    request = self.contents(ui);
                })
            });
        if !is_open {
            self.close();
        }
        request
    }

    fn close(&mut self) {
        self.open = false;
        if matches!(self.phase, Phase::UpToDate | Phase::Failed { .. }) {
            self.phase = Phase::Idle;
        }
    }

    fn contents(&mut self, ui: &mut egui::Ui) -> Option<UpdateRequest> {
        let mut request = None;
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(ICON_SYSTEM_UPDATE.codepoint).size(22.0).color(ACCENT_BLUE));
            ui.vertical(|ui| {
                ui.label(RichText::new(t!("update-window-title")).size(14.0).strong().color(TEXT_PRIMARY));
                ui.label(
                    RichText::new(t!("update-current", version = CURRENT_VERSION)).size(11.0).color(TEXT_SECONDARY),
                );
            });
        });
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);

        // Aksi tombol dikumpulkan dulu: `self.phase` sedang dipinjam.
        enum Action {
            Install(Release),
            OpenPage(String),
            Skip(String),
            Later,
            Restart,
        }
        let mut action = None;

        match &self.phase {
            Phase::Idle | Phase::Checking => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new(t!("update-checking")).size(12.0).color(TEXT_PRIMARY));
                });
            }
            Phase::UpToDate => {
                status_line(ui, ICON_CHECK_CIRCLE.codepoint, ACCENT_GREEN, &t!("update-up-to-date", version = CURRENT_VERSION));
            }
            Phase::Available { release, blocker } => {
                let version = release.version.to_string();
                ui.label(
                    RichText::new(t!("update-available", version = version.as_str()))
                        .size(13.0)
                        .strong()
                        .color(ACCENT_GREEN),
                );
                if !release.notes.trim().is_empty() {
                    ui.add_space(4.0);
                    ui.label(RichText::new(t!("update-notes")).size(11.0).strong().color(TEXT_SECONDARY));
                    card_frame().show(ui, |ui| {
                        egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(RichText::new(release.notes.trim()).size(11.0).color(TEXT_PRIMARY));
                        });
                    });
                }
                if let Some(reason) = blocker {
                    ui.add_space(4.0);
                    status_line(ui, ICON_WARNING.codepoint, ACCENT_ORANGE, reason);
                }
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    if blocker.is_none() {
                        if ui.button(format!("{} {}", ICON_DOWNLOAD.codepoint, t!("update-install"))).clicked() {
                            action = Some(Action::Install(release.clone()));
                        }
                    } else if ui.button(format!("{} {}", ICON_OPEN_IN_NEW.codepoint, t!("update-open-page"))).clicked() {
                        action = Some(Action::OpenPage(release.page_url.clone()));
                    }
                    if ui.button(t!("update-skip")).clicked() {
                        action = Some(Action::Skip(version.clone()));
                    }
                    if ui.button(t!("update-later")).clicked() {
                        action = Some(Action::Later);
                    }
                });
            }
            Phase::Installing(release) => {
                let version = release.version.to_string();
                ui.label(
                    RichText::new(t!("update-downloading", version = version.as_str())).size(12.0).color(TEXT_PRIMARY),
                );
                ui.add_space(4.0);
                let mb = |bytes: u64| bytes as f64 / (1024.0 * 1024.0);
                let text = match self.progress.total() {
                    0 => format!("{:.1} MB", mb(self.progress.downloaded())),
                    total => format!("{:.1} / {:.1} MB", mb(self.progress.downloaded()), mb(total)),
                };
                let bar = match self.progress.fraction() {
                    Some(f) => egui::ProgressBar::new(f),
                    None => egui::ProgressBar::new(0.0).animate(true),
                };
                ui.add(bar.text(text));
                ui.ctx().request_repaint_after(Duration::from_millis(100));
            }
            Phase::Ready(installed) => {
                let version = installed.version.to_string();
                status_line(ui, ICON_CHECK_CIRCLE.codepoint, ACCENT_GREEN, &t!("update-ready", version = version.as_str()));
                ui.label(RichText::new(t!("update-ready-note")).size(11.0).color(TEXT_SECONDARY));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(format!("{} {}", ICON_RESTART_ALT.codepoint, t!("update-restart"))).clicked() {
                        action = Some(Action::Restart);
                    }
                    if ui.button(t!("update-later")).clicked() {
                        action = Some(Action::Later);
                    }
                });
            }
            Phase::Failed { message, page_url } => {
                status_line(ui, ICON_ERROR.codepoint, ACCENT_ORANGE, message);
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if let Some(url) = page_url {
                        if ui.button(format!("{} {}", ICON_OPEN_IN_NEW.codepoint, t!("update-open-page"))).clicked() {
                            action = Some(Action::OpenPage(url.clone()));
                        }
                    }
                    if ui.button(t!("update-close")).clicked() {
                        action = Some(Action::Later);
                    }
                });
            }
        }

        ui.add_space(6.0);
        ui.separator();
        if ui.checkbox(&mut self.settings.auto_check, t!("update-auto-check")).changed() {
            self.save();
        }

        match action {
            Some(Action::Install(release)) => self.start_install(release),
            Some(Action::OpenPage(url)) => {
                if let Err(e) = ducad_cloud::open_url(&url) {
                    log::warn!("Halaman rilis tidak bisa dibuka: {e}");
                }
            }
            Some(Action::Skip(version)) => {
                self.settings.skipped_version = Some(version);
                self.save();
                self.phase = Phase::Idle;
                self.open = false;
            }
            Some(Action::Later) => self.close(),
            Some(Action::Restart) => {
                self.restart_requested = true;
                request = Some(UpdateRequest::Restart);
            }
            None => {}
        }
        request
    }
}

fn status_line(ui: &mut egui::Ui, icon: &str, color: egui::Color32, text: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(icon).size(16.0).color(color));
        ui.label(RichText::new(text).size(12.0).color(TEXT_PRIMARY));
    });
}

/// Alasan pembaruan `release` tidak bisa dipasang otomatis di sini, atau
/// `None` bila bisa.
fn install_blocker(release: &Release) -> Option<String> {
    if let Err(reason) = ducad_update::Installation::current() {
        return Some(unsupported_text(&reason));
    }
    let has_package = ducad_update::Target::current().is_some_and(|t| release.package_for(&t).is_some());
    (!has_package).then(|| t!("update-err-no-package"))
}

fn unsupported_text(reason: &Unsupported) -> String {
    match reason {
        Unsupported::Platform => t!("update-manual-platform"),
        Unsupported::NotPackaged => t!("update-manual-dev"),
        Unsupported::AppStore => t!("update-manual-appstore"),
        Unsupported::ReadOnly(path) => t!("update-manual-readonly", path = path.display().to_string()),
    }
}

fn error_text(e: &UpdateError) -> String {
    match e {
        UpdateError::Network(detail) => t!("update-err-network", detail = detail.as_str()),
        UpdateError::NoRelease => t!("update-err-no-release"),
        UpdateError::NoPackage => t!("update-err-no-package"),
        UpdateError::ChecksumMissing => t!("update-err-checksum-missing"),
        UpdateError::ChecksumMismatch => t!("update-err-checksum"),
        UpdateError::Signature(detail) => t!("update-err-signature", detail = detail.as_str()),
        UpdateError::Unsupported(reason) => unsupported_text(reason),
        UpdateError::Io(detail) => t!("update-err-io", detail = detail.as_str()),
    }
}
