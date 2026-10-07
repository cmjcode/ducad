//! Tutorial selamat datang: penanda "sudah pernah dibuka", deteksi hasil tiap
//! langkah, dan geometri awal tiap bab.
//!
//! Widgetnya (`ducad_ui::Onboarding`) murni egui. Modul ini menjawab apa yang
//! hanya diketahui aplikasi: apakah ini pertama kali DUCAD dibuka
//! (`$HOME/.ducad/onboarding.json`), dan apakah geometri benar-benar berubah
//! seperti yang diminta langkah yang sedang tampil (body bertambah, volume
//! berkurang, lingkaran bertambah, dan seterusnya).

use std::path::{Path, PathBuf};

use ducad_ui::{Onboarding, OnboardingChapter, OnboardingEvent, OnboardingGoal, OnboardingState};
use serde::{Deserialize, Serialize};
use serde_json::json;

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
    /// `<data_dir>/onboarding.json` (lihat `platform::data_dir`).
    pub fn default_path() -> PathBuf {
        crate::platform::data_dir().join("onboarding.json")
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

/// Potret keadaan aplikasi. Sebuah langkah lulus bila potret sekarang berbeda
/// dari potret saat langkah itu mulai tampil, sesuai `OnboardingGoal`-nya.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Probe {
    pub circles: usize,
    pub arcs: usize,
    pub bodies: usize,
    /// Jumlah volume semua body (mm³); dihitung hanya saat model berubah.
    pub volume: f64,
    /// Nama semua body, terurut; berubah = ada yang diganti nama.
    pub names: Vec<String>,
    pub materials: usize,
    pub camera: [f32; 6],
    pub sketching: bool,
    pub plane_z: f32,
    pub measurements: usize,
    pub section: bool,
    pub sim_results: bool,
    pub chat_open: bool,
}

/// State tutorial milik aplikasi.
pub struct OnboardingCtl {
    pub state: OnboardingState,
    /// `None` = tidak dipersist (tes).
    path: Option<PathBuf>,
    baseline: Option<Probe>,
    /// Volume terakhir yang dihitung; dihitung ulang bila `volume_dirty`.
    volume: f64,
    volume_dirty: bool,
    /// Nama command model yang dieksekusi sejak langkah ini tampil.
    model_commands: Vec<String>,
    saved: bool,
    exported: Vec<&'static str>,
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
            volume: 0.0,
            volume_dirty: true,
            model_commands: Vec::new(),
            saved: false,
            exported: Vec::new(),
        }
    }

    /// Dipanggil `execute_model_command` untuk tiap command model.
    pub fn note_model_command(&mut self, name: &str) {
        self.volume_dirty = true;
        if self.state.open {
            self.model_commands.push(name.to_string());
        }
    }

    /// Dipanggil setelah dokumen berhasil disimpan.
    pub fn note_saved(&mut self) {
        self.saved = true;
    }

    /// Dipanggil setelah ekspor berhasil (`"step"`, `"pdf"`).
    pub fn note_exported(&mut self, format: &str) {
        match format {
            "step" => self.exported.push("step"),
            "pdf" => self.exported.push("pdf"),
            _ => {}
        }
    }

    fn reset_step_tracking(&mut self) {
        self.baseline = None;
        self.model_commands.clear();
        self.saved = false;
        self.exported.clear();
    }

    fn ran(&self, names: &[&str]) -> bool {
        self.model_commands
            .iter()
            .any(|c| names.contains(&c.as_str()))
    }

    fn goal_met(&self, goal: OnboardingGoal, base: &Probe, now: &Probe) -> bool {
        // Toleransi volume: perubahan nyata selalu jauh di atas 1 mm³.
        const EPS: f64 = 1.0;
        match goal {
            OnboardingGoal::None => true,
            OnboardingGoal::Circle => now.circles > base.circles,
            OnboardingGoal::ExtrudeBody => now.bodies > base.bodies,
            OnboardingGoal::Navigate => camera_moved(&base.camera, &now.camera),
            // Bidang sketsa di atas alas (sisi atas solid) dan lingkaran baru di sana.
            OnboardingGoal::CircleOnFace => {
                now.sketching && now.plane_z > 0.5 && now.circles > base.circles
            }
            OnboardingGoal::Cut => now.volume < base.volume - EPS,
            OnboardingGoal::Chamfer => self.ran(&["Chamfer"]),
            OnboardingGoal::Save => self.saved,
            OnboardingGoal::MoreCircles(n) => now.circles >= base.circles + n,
            OnboardingGoal::Slot => now.arcs >= base.arcs + 2,
            OnboardingGoal::MoreArcs(n) => now.arcs >= base.arcs + n,
            OnboardingGoal::Fillet => self.ran(&["Fillet"]),
            OnboardingGoal::Measure => now.measurements > base.measurements,
            OnboardingGoal::Rename => now.names != base.names && now.bodies == base.bodies,
            OnboardingGoal::AddVolume => now.volume > base.volume + EPS,
            OnboardingGoal::Shell => self.ran(&["Shell", "Shell Face"]),
            OnboardingGoal::Hole => self.ran(&["Hole Wizard"]),
            OnboardingGoal::Section => now.section,
            OnboardingGoal::Material => now.materials > base.materials,
            OnboardingGoal::SimResult => now.sim_results && !base.sim_results,
            OnboardingGoal::ExportPdf => self.exported.contains(&"pdf"),
            OnboardingGoal::ExportStep => self.exported.contains(&"step"),
            OnboardingGoal::OpenChat => now.chat_open,
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

/// Geometri awal Bab 2: cakram Ø240×6 dengan lubang poros Ø60 dan chamfer
/// tepi atas (hasil Bab 1). Dijalankan lewat oplog engine.
const CHAPTER2_START_OPS: &str = r#"[
  {"op": "primitive", "id": "disc", "shape": {"cylinder": {"r": 120, "h": 6}}, "at": [0, 0, 0]},
  {"op": "primitive", "id": "bore", "shape": {"cylinder": {"r": 30, "h": 6}}, "at": [0, 0, 0]},
  {"op": "boolean", "id": "cakram", "kind": "subtract", "a": "disc", "b": "bore"},
  {"op": "chamfer", "id": "rim", "body": "cakram", "edges": "of(>Z)", "distance": 1}
]"#;

/// Geometri awal Bab 3: Bab 2 ditambah lima lubang baut dan dua belas
/// ventilasi. `pattern` dengan `merge` menyatukan salinan ke body asalnya,
/// jadi yang dikurangkan tetap `bolt` dan `vent`.
const CHAPTER3_START_OPS: &str = r#"[
  {"op": "primitive", "id": "disc", "shape": {"cylinder": {"r": 120, "h": 6}}, "at": [0, 0, 0]},
  {"op": "primitive", "id": "bore", "shape": {"cylinder": {"r": 30, "h": 6}}, "at": [0, 0, 0]},
  {"op": "boolean", "id": "cakram", "kind": "subtract", "a": "disc", "b": "bore"},
  {"op": "primitive", "id": "bolt", "shape": {"cylinder": {"r": 5, "h": 6}}, "at": [45, 0, 0]},
  {"op": "pattern", "id": "bolts", "body": "bolt", "merge": true,
   "kind": {"circular": {"pivot": [0, 0, 0], "axis": [0, 0, 1], "count": 5, "angle_deg": 360}}},
  {"op": "boolean", "id": "cakram2", "kind": "subtract", "a": "cakram", "b": "bolt"},
  {"op": "primitive", "id": "vent", "shape": {"cylinder": {"r": 4, "h": 6}}, "at": [95, 0, 0]},
  {"op": "pattern", "id": "vents", "body": "vent", "merge": true,
   "kind": {"circular": {"pivot": [0, 0, 0], "axis": [0, 0, 1], "count": 12, "angle_deg": 360}}},
  {"op": "boolean", "id": "cakram3", "kind": "subtract", "a": "cakram2", "b": "vent"},
  {"op": "chamfer", "id": "rim", "body": "cakram3", "edges": "of(>Z)", "distance": 1}
]"#;

impl DuCADApp {
    pub(crate) fn onboarding_probe(&mut self) -> Probe {
        if self.onboarding.volume_dirty {
            self.onboarding.volume = self.model.geometry.values().map(|g| g.shape.volume()).sum();
            self.onboarding.volume_dirty = false;
        }
        let sketch = self.sketch();
        let mut names: Vec<String> = self
            .model
            .doc
            .bodies
            .values()
            .map(|b| b.name.clone())
            .collect();
        names.sort_unstable();
        Probe {
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
            volume: self.onboarding.volume,
            names,
            materials: self
                .model
                .doc
                .bodies
                .values()
                .filter(|b| b.mechanical.is_some())
                .count(),
            camera: [
                self.camera.yaw,
                self.camera.pitch,
                self.camera.distance,
                self.camera.target.x,
                self.camera.target.y,
                self.camera.target.z,
            ],
            sketching: self.is_sketching,
            plane_z: self.active_plane.origin.z,
            measurements: self.measurements.len(),
            section: self.section_enabled,
            sim_results: self.sim.has_results(),
            chat_open: self.chat.panel.open,
        }
    }

    /// Buka tutorial dari awal (tombol bantuan, palet perintah, burger menu).
    pub fn start_onboarding(&mut self) {
        self.onboarding.state.restart();
        self.onboarding.reset_step_tracking();
    }

    /// Bab 2 dan 3 melanjutkan hasil bab sebelumnya. Bila kanvas kosong saat
    /// bab itu dimulai (pengguna melompat dari kartu sambutan), geometri
    /// awalnya dibangun lewat oplog engine supaya langkah-langkahnya tetap
    /// bisa dikerjakan.
    pub(crate) fn onboarding_bootstrap_chapter(&mut self) {
        let step = self.onboarding.state.step;
        let chapter = self.onboarding.state.current().chapter;
        if step != chapter.first_step() || !self.model.doc.bodies.is_empty() {
            return;
        }
        let ops = match chapter {
            OnboardingChapter::Beginner => return,
            OnboardingChapter::Intermediate => CHAPTER2_START_OPS,
            OnboardingChapter::Advanced => CHAPTER3_START_OPS,
        };
        let ops: serde_json::Value = match serde_json::from_str(ops) {
            Ok(v) => v,
            Err(e) => {
                log::error!("oplog awal bab tutorial rusak: {e}");
                return;
            }
        };
        // Mode "selalu usulkan" menahan run_ops sampai disetujui; geometri
        // awal harus langsung ada, jadi dilewati dalam mode itu.
        if self.bridge.force_propose {
            return;
        }
        match self.agent_call_local("run_ops", json!({ "ops": ops })) {
            Some(out) if !out.is_error => {
                self.onboarding.volume_dirty = true;
                self.is_sketching = false;
                self.left_toolbar.is_sketching = false;
                self.model_status = Some(ducad_i18n::t!("onboard-bootstrap-done"));
            }
            Some(out) => {
                log::warn!("geometri awal bab tutorial gagal: {}", out.payload);
                self.model_status = Some(ducad_i18n::t!("onboard-bootstrap-failed"));
            }
            None => {}
        }
    }

    /// Perbarui status "sudah dikerjakan" langkah yang sedang tampil.
    pub(crate) fn onboarding_track(&mut self) {
        if !self.onboarding.state.open {
            return;
        }
        let now = self.onboarding_probe();
        let animating = self.camera_animation.is_some();
        let base = self.onboarding.baseline.get_or_insert_with(|| now.clone());
        if animating {
            // Kamera yang bergerak sendiri (mis. ke isometrik setelah extrude)
            // bukan aksi pengguna.
            base.camera = now.camera;
        }
        let base = base.clone();
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
        if !self.onboarding.state.open {
            return;
        }
        // Deteksi tetap jalan saat lembar gambar 2D terbuka (mengekspor PDF
        // dari sana adalah salah satu langkah); kartunya menunggu sampai
        // lembar ditutup.
        self.onboarding_track();
        if self.drawing_sheet_state.is_open {
            return;
        }
        match Onboarding::show(ctx, bounds, &mut self.onboarding.state) {
            Some(OnboardingEvent::StepChanged) => {
                self.onboarding.reset_step_tracking();
                self.onboarding_bootstrap_chapter();
            }
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
        // Frame pertama langkah hanya mengambil potret awal.
        app.onboarding_track();
        assert!(!app.onboarding.state.done);
        app
    }

    fn add_box(app: &mut DuCADApp, id: &str, size: [f64; 3]) {
        let out = app
            .agent_call_local(
                "run_ops",
                json!({ "ops": [{ "op": "primitive", "id": id, "shape": { "box": { "size": size } } }] }),
            )
            .expect("balasan langsung");
        assert!(!out.is_error, "{}", out.payload);
        app.onboarding.volume_dirty = true;
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
    fn chapter_one_is_overview_then_build_steps() {
        let goals: Vec<_> = ONBOARDING_STEPS[1..]
            .iter()
            .take_while(|s| s.kind != OnboardingStepKind::ChapterEnd)
            .map(|s| (s.kind, s.goal))
            .collect();
        assert_eq!(
            goals[0],
            (OnboardingStepKind::Overview, OnboardingGoal::None)
        );
        assert_eq!(
            goals[1..].iter().map(|g| g.1).collect::<Vec<_>>(),
            [
                OnboardingGoal::Circle,
                OnboardingGoal::ExtrudeBody,
                OnboardingGoal::Navigate,
                OnboardingGoal::CircleOnFace,
                OnboardingGoal::Cut,
                OnboardingGoal::Chamfer,
                OnboardingGoal::Save,
            ]
        );
    }

    #[test]
    fn geometry_goals_follow_real_changes() {
        // Extrude = body bertambah.
        let mut app = app_at(OnboardingGoal::ExtrudeBody);
        add_box(&mut app, "a", [10.0, 10.0, 10.0]);
        app.onboarding_track();
        assert!(app.onboarding.state.done, "body bertambah");

        // Potong = volume berkurang; menambah body bukan potongan.
        let mut app = DuCADApp::new_for_test();
        add_box(&mut app, "a", [10.0, 10.0, 10.0]);
        app.start_onboarding();
        app.onboarding.state.go_to(step_of(OnboardingGoal::Cut));
        app.onboarding_track();
        add_box(&mut app, "b", [5.0, 5.0, 5.0]);
        app.onboarding_track();
        assert!(
            !app.onboarding.state.done,
            "volume bertambah bukan potongan"
        );
        let out = app
            .agent_call_local(
                "run_ops",
                json!({ "ops": [{ "op": "boolean", "id": "c", "kind": "subtract", "a": "a", "b": "b" }] }),
            )
            .expect("balasan langsung");
        assert!(!out.is_error, "{}", out.payload);
        app.onboarding.volume_dirty = true;
        app.onboarding_track();
        assert!(app.onboarding.state.done, "volume berkurang");

        // Hub = volume bertambah.
        let mut app = DuCADApp::new_for_test();
        add_box(&mut app, "a", [10.0, 10.0, 10.0]);
        app.start_onboarding();
        app.onboarding
            .state
            .go_to(step_of(OnboardingGoal::AddVolume));
        app.onboarding_track();
        add_box(&mut app, "b", [5.0, 5.0, 5.0]);
        app.onboarding_track();
        assert!(app.onboarding.state.done, "volume bertambah");
    }

    #[test]
    fn model_commands_pass_matching_lessons_only() {
        let mut app = app_at(OnboardingGoal::Chamfer);
        app.onboarding.note_model_command("Fillet");
        app.onboarding_track();
        assert!(!app.onboarding.state.done, "fillet bukan chamfer");
        app.onboarding.note_model_command("Chamfer");
        app.onboarding_track();
        assert!(app.onboarding.state.done);

        for (goal, cmd) in [
            (OnboardingGoal::Fillet, "Fillet"),
            (OnboardingGoal::Shell, "Shell"),
            (OnboardingGoal::Hole, "Hole Wizard"),
        ] {
            let mut app = app_at(goal);
            app.onboarding.note_model_command(cmd);
            app.onboarding_track();
            assert!(app.onboarding.state.done, "{cmd}");
        }
    }

    #[test]
    fn commands_before_the_lesson_do_not_count() {
        let mut app = app_at(OnboardingGoal::Chamfer);
        app.onboarding.note_model_command("Chamfer");
        app.onboarding_track();
        assert!(app.onboarding.state.done);
        app.onboarding.state.go_to(step_of(OnboardingGoal::Fillet));
        app.onboarding.reset_step_tracking();
        app.onboarding_track();
        assert!(!app.onboarding.state.done);
    }

    #[test]
    fn sketch_goals_need_the_right_plane_and_entities() {
        let mut app = app_at(OnboardingGoal::CircleOnFace);
        let circle = || ducad_sketch::Entity::circle(glam::DVec2::ZERO, 5.0);
        app.execute_sketch_command(Box::new(ducad_sketch::commands::InsertEntities::new(
            "Circle",
            vec![circle()],
        )));
        app.onboarding_track();
        assert!(!app.onboarding.state.done, "masih di bidang alas");
        app.active_plane = ducad_render::SketchPlane::from_origin_normal(
            glam::Vec3::new(0.0, 0.0, 6.0),
            glam::Vec3::Z,
        );
        app.is_sketching = true;
        app.onboarding_track();
        assert!(app.onboarding.state.done, "lingkaran di sisi atas");

        let mut app = app_at(OnboardingGoal::MoreCircles(4));
        app.execute_sketch_command(Box::new(ducad_sketch::commands::InsertEntities::new(
            "Pattern",
            vec![circle(), circle(), circle()],
        )));
        app.onboarding_track();
        assert!(!app.onboarding.state.done, "baru tiga salinan");
        app.execute_sketch_command(Box::new(ducad_sketch::commands::InsertEntities::new(
            "Pattern",
            vec![circle()],
        )));
        app.onboarding_track();
        assert!(app.onboarding.state.done);
    }

    #[test]
    fn state_goals_pass_on_their_actions() {
        let mut app = app_at(OnboardingGoal::Navigate);
        app.camera.yaw += 0.3;
        app.onboarding_track();
        assert!(app.onboarding.state.done, "orbit");

        let mut app = app_at(OnboardingGoal::Navigate);
        app.start_camera_animation_to_isometric(10_000);
        app.camera.yaw += 0.5;
        app.onboarding_track();
        assert!(!app.onboarding.state.done, "animasi kamera bukan navigasi");

        let mut app = app_at(OnboardingGoal::Section);
        app.section_enabled = true;
        app.onboarding_track();
        assert!(app.onboarding.state.done, "irisan");

        let mut app = app_at(OnboardingGoal::Save);
        app.onboarding.note_saved();
        app.onboarding_track();
        assert!(app.onboarding.state.done, "simpan");

        let mut app = app_at(OnboardingGoal::ExportStep);
        app.onboarding.note_exported("pdf");
        app.onboarding_track();
        assert!(!app.onboarding.state.done, "pdf bukan step");
        app.onboarding.note_exported("step");
        app.onboarding_track();
        assert!(app.onboarding.state.done, "step");

        let mut app = DuCADApp::new_for_test();
        add_box(&mut app, "a", [10.0, 10.0, 10.0]);
        app.start_onboarding();
        app.onboarding.state.go_to(step_of(OnboardingGoal::Rename));
        app.onboarding_track();
        for b in app.model.doc.bodies.values_mut() {
            b.name = "Cakram".to_string();
        }
        app.onboarding_track();
        assert!(app.onboarding.state.done, "ganti nama");
    }

    #[test]
    fn later_chapters_bootstrap_their_geometry() {
        for chapter in [OnboardingChapter::Intermediate, OnboardingChapter::Advanced] {
            let mut app = DuCADApp::new_for_test();
            app.start_onboarding();
            app.onboarding.state.go_to(chapter.first_step());
            app.onboarding_bootstrap_chapter();
            assert!(
                !app.model.doc.bodies.is_empty(),
                "{}: geometri awal dibuat",
                chapter.key()
            );
            assert!(!app.is_sketching, "mulai di mode 3D");
            let volume: f64 = app.model.geometry.values().map(|g| g.shape.volume()).sum();
            // Cakram Ø240×6 pejal ≈ 271.000 mm³; lubang-lubang menguranginya.
            assert!(
                volume > 150_000.0 && volume < 271_500.0,
                "{}: {volume}",
                chapter.key()
            );
        }
        // Bab 1 dan kanvas yang sudah berisi tidak disentuh.
        let mut app = DuCADApp::new_for_test();
        app.start_onboarding();
        app.onboarding
            .state
            .go_to(OnboardingChapter::Beginner.first_step());
        app.onboarding_bootstrap_chapter();
        assert!(app.model.doc.bodies.is_empty());
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
        assert_eq!(
            actions
                .iter()
                .filter(|(label, _, _)| label.to_lowercase().contains("extrude"))
                .count(),
            1
        );
        app.extrude_from_palette();
        assert!(app.model_status.is_some());
    }
}
