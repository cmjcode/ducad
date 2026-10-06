//! Tutorial selamat datang: penanda "sudah pernah dibuka" dan deteksi aksi.
//!
//! Widgetnya (`ducad_ui::Onboarding`) murni egui. Modul ini menjawab dua hal
//! yang hanya diketahui aplikasi: apakah ini pertama kali DUCAD dibuka
//! (`$HOME/.ducad/onboarding.json`), dan apakah pengguna benar-benar sudah
//! mencoba aksi pelajaran yang sedang tampil.

use std::path::{Path, PathBuf};

use ducad_ui::{Onboarding, OnboardingEvent, OnboardingGoal, OnboardingState};
use serde::{Deserialize, Serialize};

use crate::app::DuCADApp;

/// Variabel lingkungan untuk mematikan tutorial (CI, sesi agent, demo).
pub const SKIP_ENV: &str = "DUCAD_SKIP_ONBOARDING";

/// Isi `onboarding.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OnboardingPrefs {
    pub version: u32,
    /// Tutorial sudah diselesaikan atau sengaja dilewati.
    pub completed: bool,
}

impl OnboardingPrefs {
    #[cfg(target_os = "ios")]
    pub fn default_path() -> PathBuf {
        crate::file_io::ios_documents_dir().join("onboarding.json")
    }

    /// `$HOME/.ducad/onboarding.json`.
    #[cfg(not(target_os = "ios"))]
    pub fn default_path() -> PathBuf {
        match std::env::var_os("HOME") {
            Some(h) => PathBuf::from(h).join(".ducad").join("onboarding.json"),
            None => PathBuf::from("onboarding.json"),
        }
    }

    /// Berkas tidak ada = pertama kali dibuka. Berkas rusak diperlakukan sama
    /// (tutorial tampil lagi) daripada menggagalkan start aplikasi.
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                log::warn!("onboarding.json rusak, memakai bawaan: {e}");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}

/// Potret keadaan aplikasi; pelajaran lulus bila potret sekarang berbeda dari
/// potret saat pelajaran mulai tampil.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Probe {
    pub entities: usize,
    pub circles: usize,
    pub arcs: usize,
    pub bodies: usize,
    pub camera: [f32; 6],
    pub sketching: bool,
    pub palette_open: bool,
    pub chat_open: bool,
    /// Tool Pilih aktif dan ada entitas, sudut, sisi, atau body yang terpilih.
    pub picked: bool,
}

/// State tutorial milik aplikasi.
pub struct OnboardingCtl {
    pub state: OnboardingState,
    /// `None` = tidak dipersist (tes).
    path: Option<PathBuf>,
    baseline: Option<Probe>,
    /// Nama command model yang dieksekusi sejak pelajaran ini tampil.
    model_commands: Vec<String>,
    saved: bool,
    /// Mode 2D/3D sempat berganti sejak pelajaran ini tampil.
    mode_flipped: bool,
}

impl OnboardingCtl {
    /// Dipakai saat aplikasi start: tutorial terbuka bila belum pernah
    /// diselesaikan dan tidak dimatikan lewat [`SKIP_ENV`].
    pub fn load() -> Self {
        let skip = std::env::var_os(SKIP_ENV).is_some_and(|v| !v.is_empty() && v != "0");
        Self::load_from(OnboardingPrefs::default_path(), skip)
    }

    pub(crate) fn load_from(path: PathBuf, skip: bool) -> Self {
        let prefs = OnboardingPrefs::load(&path);
        let mut ctl = Self::disabled();
        ctl.path = Some(path);
        if !prefs.completed && !skip {
            ctl.state.restart();
        }
        ctl
    }

    /// Tutorial tertutup dan tidak menyentuh disk.
    pub fn disabled() -> Self {
        Self {
            state: OnboardingState::default(),
            path: None,
            baseline: None,
            model_commands: Vec::new(),
            saved: false,
            mode_flipped: false,
        }
    }

    /// Dipanggil `execute_model_command` untuk tiap command model.
    pub fn note_model_command(&mut self, name: &str) {
        // Di luar tutorial tidak ada yang membaca daftar ini.
        if self.state.open {
            self.model_commands.push(name.to_string());
        }
    }

    /// Dipanggil setelah dokumen berhasil disimpan.
    pub fn note_saved(&mut self) {
        self.saved = true;
    }

    fn reset_step_tracking(&mut self) {
        self.baseline = None;
        self.model_commands.clear();
        self.saved = false;
        self.mode_flipped = false;
    }

    fn ran(&self, names: &[&str]) -> bool {
        self.model_commands
            .iter()
            .any(|c| names.contains(&c.as_str()))
    }

    fn goal_met(&self, goal: OnboardingGoal, base: &Probe, now: &Probe) -> bool {
        match goal {
            OnboardingGoal::None => true,
            OnboardingGoal::DrawRectangle => now.entities > base.entities,
            OnboardingGoal::UseSelect => now.picked,
            OnboardingGoal::DrawCircle => now.circles > base.circles,
            OnboardingGoal::Extrude => self.ran(&["Extrude"]) || now.bodies > base.bodies,
            OnboardingGoal::Navigate => camera_moved(&base.camera, &now.camera),
            OnboardingGoal::PushPull => self.ran(&["Extrude Face", "Cut Face", "Cut Extrude"]),
            // Fillet sudut sketsa menambah satu busur; fillet tepi solid juga diterima.
            OnboardingGoal::Fillet => now.arcs > base.arcs || self.ran(&["Fillet", "Chamfer"]),
            // Harus berakhir di mode Sketsa: pelajaran berikutnya menggambar.
            OnboardingGoal::ToggleMode => self.mode_flipped && now.sketching,
            OnboardingGoal::OpenPalette => now.palette_open,
            OnboardingGoal::OpenChat => now.chat_open,
            OnboardingGoal::Save => self.saved,
        }
    }

    fn persist_completed(&self) {
        let Some(path) = &self.path else {
            return;
        };
        let prefs = OnboardingPrefs {
            version: 1,
            completed: true,
        };
        if let Err(e) = prefs.save(path) {
            log::warn!("gagal menyimpan {}: {e}", path.display());
        }
    }
}

/// Orbit beberapa derajat, zoom beberapa persen, atau geser beberapa mm.
fn camera_moved(base: &[f32; 6], now: &[f32; 6]) -> bool {
    let rotated = (now[0] - base[0]).abs() > 0.05 || (now[1] - base[1]).abs() > 0.05;
    let zoomed = (now[2] - base[2]).abs() > base[2].abs() * 0.03;
    let panned = (3..6).any(|i| (now[i] - base[i]).abs() > base[2].abs().max(1.0) * 0.02);
    rotated || zoomed || panned
}

impl DuCADApp {
    pub(crate) fn onboarding_probe(&self) -> Probe {
        let sketch = self.sketch();
        Probe {
            entities: sketch.entities.iter().count(),
            circles: sketch
                .entities
                .iter()
                .filter(|(_, e)| matches!(e, ducad_sketch::Entity::Circle { .. }))
                .count(),
            arcs: sketch
                .entities
                .iter()
                .filter(|(_, e)| matches!(e, ducad_sketch::Entity::Arc { .. }))
                .count(),
            bodies: self.model.doc.bodies.len(),
            camera: [
                self.camera.yaw,
                self.camera.pitch,
                self.camera.distance,
                self.camera.target.x,
                self.camera.target.y,
                self.camera.target.z,
            ],
            sketching: self.is_sketching,
            palette_open: self.palette.is_open(),
            chat_open: self.chat.panel.open,
            picked: self.tool == crate::types::ToolKind::Select
                && (!self.selected.is_empty()
                    || self.active_sketch_corner.is_some()
                    || self.active_face.is_some()
                    || !self.selected_bodies.is_empty()),
        }
    }

    /// Buka tutorial dari awal (palet perintah / burger menu).
    pub fn start_onboarding(&mut self) {
        self.onboarding.state.restart();
        self.onboarding.reset_step_tracking();
    }

    /// Perbarui status "sudah dicoba" pelajaran yang sedang tampil.
    pub(crate) fn onboarding_track(&mut self) {
        if !self.onboarding.state.open {
            return;
        }
        let now = self.onboarding_probe();
        let animating = self.camera_animation.is_some();
        let base = self.onboarding.baseline.get_or_insert(now);
        if animating {
            // Kamera yang bergerak sendiri (mis. ke isometrik setelah extrude)
            // bukan aksi pengguna.
            base.camera = now.camera;
        }
        let base = *base;
        self.onboarding.mode_flipped |= now.sketching != base.sketching;
        let goal = self.onboarding.state.current().goal;
        if !self.onboarding.state.done
            && goal != OnboardingGoal::None
            && self.onboarding.goal_met(goal, &base, &now)
        {
            self.onboarding.state.done = true;
        }
    }

    /// Dipanggil sekali per frame setelah seluruh chrome dirender.
    pub fn onboarding_frame(&mut self, ctx: &egui::Context, bounds: egui::Rect) {
        // Lembar gambar 2D menutupi kanvas; tutorial menunggu sampai ditutup.
        if !self.onboarding.state.open || self.drawing_sheet_state.is_open {
            return;
        }
        self.onboarding_track();
        match Onboarding::show(ctx, bounds, &mut self.onboarding.state) {
            Some(OnboardingEvent::StepChanged) => self.onboarding.reset_step_tracking(),
            Some(OnboardingEvent::Dismissed | OnboardingEvent::Finished) => {
                self.onboarding.reset_step_tracking();
                self.onboarding.persist_completed();
            }
            Some(OnboardingEvent::SetLanguage(lang)) => {
                self.language = lang;
                ducad_i18n::set_language(lang);
            }
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_ui::{OnboardingStepKind, ONBOARDING_STEPS};

    fn temp_path(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ducad-onboarding-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("onboarding.json")
    }

    fn step_of(goal: OnboardingGoal) -> usize {
        ONBOARDING_STEPS
            .iter()
            .position(|s| s.goal == goal && s.kind == OnboardingStepKind::Lesson)
            .expect("pelajaran ada")
    }

    fn app_at(goal: OnboardingGoal) -> DuCADApp {
        let mut app = DuCADApp::new_for_test();
        app.start_onboarding();
        app.onboarding.state.go_to(step_of(goal));
        // Frame pertama pelajaran hanya mengambil potret awal.
        app.onboarding_track();
        assert!(!app.onboarding.state.done);
        app
    }

    #[test]
    fn first_run_opens_and_completion_is_remembered() {
        let path = temp_path("first-run");
        let ctl = OnboardingCtl::load_from(path.clone(), false);
        assert!(ctl.state.open, "berkas belum ada = pertama kali dibuka");
        assert_eq!(ctl.state.step, 0);

        ctl.persist_completed();
        assert_eq!(
            OnboardingPrefs::load(&path),
            OnboardingPrefs {
                version: 1,
                completed: true
            }
        );
        assert!(!OnboardingCtl::load_from(path.clone(), false).state.open);
        let _ = std::fs::remove_dir_all(path.parent().expect("punya induk"));
    }

    #[test]
    fn skip_env_and_corrupt_file() {
        let path = temp_path("skip");
        assert!(!OnboardingCtl::load_from(path.clone(), true).state.open);

        std::fs::create_dir_all(path.parent().expect("punya induk")).expect("buat dir");
        std::fs::write(&path, "{ bukan json").expect("tulis");
        assert!(OnboardingCtl::load_from(path.clone(), false).state.open);
        let _ = std::fs::remove_dir_all(path.parent().expect("punya induk"));
    }

    #[test]
    fn test_app_starts_without_tutorial() {
        let app = DuCADApp::new_for_test();
        assert!(!app.onboarding.state.open);
    }

    #[test]
    fn model_command_passes_matching_lesson_only() {
        let mut app = app_at(OnboardingGoal::Fillet);
        app.onboarding.note_model_command("Extrude");
        app.onboarding_track();
        assert!(!app.onboarding.state.done, "extrude bukan fillet");
        app.onboarding.note_model_command("Fillet");
        app.onboarding_track();
        assert!(app.onboarding.state.done);
    }

    #[test]
    fn commands_before_the_lesson_do_not_count() {
        let mut app = app_at(OnboardingGoal::Extrude);
        app.onboarding.note_model_command("Extrude");
        app.onboarding_track();
        assert!(app.onboarding.state.done);

        // Pindah langkah menghapus jejak: extrude tadi tidak meluluskan push-pull.
        app.onboarding
            .state
            .go_to(step_of(OnboardingGoal::PushPull));
        app.onboarding.reset_step_tracking();
        app.onboarding_track();
        assert!(!app.onboarding.state.done);
        app.onboarding.note_model_command("Extrude Face");
        app.onboarding_track();
        assert!(app.onboarding.state.done);
    }

    #[test]
    fn state_changes_pass_their_lessons() {
        let mut app = app_at(OnboardingGoal::Navigate);
        app.camera.yaw += 0.3;
        app.onboarding_track();
        assert!(app.onboarding.state.done, "orbit");

        let mut app = app_at(OnboardingGoal::OpenPalette);
        app.palette.open();
        app.onboarding_track();
        assert!(app.onboarding.state.done, "palet");

        let mut app = app_at(OnboardingGoal::Save);
        app.onboarding.note_saved();
        app.onboarding_track();
        assert!(app.onboarding.state.done, "simpan");
    }

    #[test]
    fn basics_come_first_and_fillet_follows_the_rectangle() {
        assert_eq!(step_of(OnboardingGoal::ToggleMode), 1);
        assert_eq!(step_of(OnboardingGoal::DrawRectangle), 2);
        assert_eq!(step_of(OnboardingGoal::UseSelect), 3);
        assert_eq!(step_of(OnboardingGoal::Fillet), 4);
    }

    #[test]
    fn select_lesson_needs_select_tool_and_a_pick() {
        let mut app = app_at(OnboardingGoal::UseSelect);
        app.tool = crate::types::ToolKind::Rectangle;
        app.active_sketch_corner =
            Some((Default::default(), Default::default(), glam::DVec2::ZERO));
        app.onboarding_track();
        assert!(!app.onboarding.state.done, "tool lain masih aktif");
        app.tool = crate::types::ToolKind::Select;
        app.onboarding_track();
        assert!(app.onboarding.state.done);
    }

    #[test]
    fn mode_lesson_must_end_in_sketch_mode() {
        let mut app = app_at(OnboardingGoal::ToggleMode);
        assert!(app.is_sketching);
        app.is_sketching = false;
        app.onboarding_track();
        assert!(!app.onboarding.state.done, "masih di 3D");
        app.is_sketching = true;
        app.onboarding_track();
        assert!(app.onboarding.state.done);
    }

    #[test]
    fn palette_demo_command_exists_in_the_real_palette() {
        let mut app = DuCADApp::new_for_test();
        let actions = app.palette_actions();
        let hit = actions
            .iter()
            .find(|(label, _, _)| label == ducad_ui::PALETTE_DEMO_COMMAND)
            .expect("perintah demo ada di palet");
        assert!(matches!(
            hit.2,
            crate::types::PaletteAction::ExtrudeSelection
        ));
        // Hanya satu perintah yang cocok dengan kata yang diketik di animasi.
        assert_eq!(
            actions
                .iter()
                .filter(|(label, _, _)| label.to_lowercase().contains("extrude"))
                .count(),
            1
        );
        // Tanpa seleksi: tidak panik, hanya memberi petunjuk.
        app.extrude_from_palette();
        assert!(app.model_status.is_some());
    }

    #[test]
    fn camera_animation_is_not_user_navigation() {
        let mut app = app_at(OnboardingGoal::Navigate);
        app.start_camera_animation_to_isometric(10_000);
        app.camera.yaw += 0.5;
        app.onboarding_track();
        assert!(!app.onboarding.state.done);
    }

    #[test]
    fn frame_renders_and_restart_reopens() {
        let mut app = DuCADApp::new_for_test();
        let ctx = egui::Context::default();
        let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 860.0));
        app.start_onboarding();
        for _ in 0..3 {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(bounds),
                    ..Default::default()
                },
                |ui| app.onboarding_frame(ui.ctx(), bounds),
            );
            out.textures_delta.clear();
        }
        assert!(app.onboarding.state.open);
        assert_eq!(app.onboarding.state.step, 0);
    }
}
