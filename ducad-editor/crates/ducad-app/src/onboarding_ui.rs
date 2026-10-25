//! Tutorial selamat datang: penanda "sudah pernah dibuka", deteksi hasil tiap
//! langkah, dan geometri awal tiap bab.
//!
//! Widgetnya (`ducad_ui::Onboarding`) murni egui. Modul ini menjawab apa yang
//! hanya diketahui aplikasi: apakah ini pertama kali DUCAD dibuka
//! (`$HOME/.ducad/onboarding.json`), dan apakah geometri benar-benar berubah
//! seperti yang diminta langkah yang sedang tampil (body bertambah, volume
//! berkurang, lingkaran bertambah, dan seterusnya).
//!
//! Tutorial tidak mengubah dokumen kerja (Step 5 `PLAN_DESIGNER_UX`): saat
//! bab pertama dimulai di atas dokumen yang berisi, dokumen itu ditulis ke
//! `<data_dir>/tutorial-stash.ducad` lalu tutorial memakai dokumen kosong.
//! Saat tutorial ditutup (atau aplikasi dibuka lagi setelah crash), dokumen
//! itu dibuka kembali dengan path aslinya.

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
    /// Jumlah op di oplog desain.
    pub ops: usize,
    /// Sidik `design.params`.
    pub params_sig: u64,
    /// Sidik `design.oplog`.
    pub oplog_sig: u64,
}

/// Dokumen kerja yang disisihkan selama tutorial.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct StashInfo {
    /// Path berkas dokumen sebelum tutorial (`None` = belum pernah disimpan).
    pub original_path: Option<PathBuf>,
    /// Dokumen punya perubahan yang belum disimpan.
    pub dirty: bool,
}

fn sig<T: Serialize>(v: &T) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_string(v).unwrap_or_default().hash(&mut h);
    h.finish()
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
    undone: bool,
    /// Tutorial sudah memakai dokumennya sendiri (kosong atau hasil bab).
    owns_doc: bool,
    /// Dokumen kerja sedang disisihkan di [`Self::stash_file`].
    stashed: Option<StashInfo>,
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
        // Sisa sesi yang crash di tengah tutorial: dipulihkan di frame pertama.
        if let (Some(file), Some(info)) = (ctl.stash_file(), ctl.stash_info_file()) {
            if file.exists() {
                let info = std::fs::read_to_string(info)
                    .ok()
                    .and_then(|t| serde_json::from_str(&t).ok())
                    .unwrap_or(StashInfo {
                        original_path: None,
                        dirty: true,
                    });
                ctl.stashed = Some(info);
            }
        }
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
            undone: false,
            owns_doc: false,
            stashed: None,
        }
    }

    /// `<dir onboarding.json>/tutorial-stash.ducad`; `None` tanpa persistensi.
    pub(crate) fn stash_file(&self) -> Option<PathBuf> {
        Some(self.path.as_ref()?.with_file_name("tutorial-stash.ducad"))
    }

    fn stash_info_file(&self) -> Option<PathBuf> {
        Some(self.path.as_ref()?.with_file_name("tutorial-stash.json"))
    }

    /// Dipanggil `DuCADApp::undo` saat ada yang di-undo.
    pub fn note_undo(&mut self) {
        if self.state.open {
            self.undone = true;
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
        self.undone = false;
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
            OnboardingGoal::ParamEdit => now.params_sig != base.params_sig && now.ops == base.ops,
            OnboardingGoal::FeatureEdit => {
                now.oplog_sig != base.oplog_sig
                    && now.ops == base.ops
                    && now.params_sig == base.params_sig
            }
            OnboardingGoal::Undo => self.undone,
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
            ops: self.agent_meta.design.oplog.len(),
            params_sig: sig(&self.agent_meta.design.params),
            oplog_sig: sig(&self.agent_meta.design.oplog),
        }
    }

    /// Dokumen berisi sesuatu yang layak dijaga.
    fn document_has_content(&self) -> bool {
        !self.model.doc.bodies.is_empty()
            || !self.agent_meta.design.oplog.is_empty()
            || self.current_file_path.is_some()
            || self
                .sketch_set
                .iter()
                .any(|(_, sk)| !sk.sketch.entities.is_empty())
    }

    /// Sisihkan dokumen kerja sebelum tutorial memakai kanvas. Gagal
    /// menyisihkan = tutorial tetap jalan di dokumen ini, dengan pemberitahuan.
    pub(crate) fn onboarding_take_document(&mut self) {
        if self.onboarding.owns_doc {
            return;
        }
        self.onboarding.owns_doc = true;
        if self.onboarding.stashed.is_some() || !self.document_has_content() {
            return;
        }
        let (Some(file), Some(info_file)) =
            (self.onboarding.stash_file(), self.onboarding.stash_info_file())
        else {
            return;
        };
        let info = StashInfo {
            original_path: self.current_file_path.clone(),
            dirty: self.model.doc.dirty,
        };
        let written = file
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .map_err(anyhow::Error::from)
            .and_then(|_| self.write_native_file(&file))
            .and_then(|_| {
                std::fs::write(&info_file, serde_json::to_string_pretty(&info)?)?;
                Ok(())
            });
        match written {
            Ok(()) => {
                self.onboarding.stashed = Some(info);
                self.new_document();
            }
            Err(e) => {
                let err = e.to_string();
                self.file_status =
                    Some(ducad_i18n::t!("onboard-stash-failed", error = err.as_str()));
            }
        }
    }

    /// Buka kembali dokumen yang disisihkan. Berkas sisihan baru dihapus
    /// setelah berhasil dibuka.
    pub(crate) fn onboarding_restore_document(&mut self) {
        self.onboarding.owns_doc = false;
        let Some(info) = self.onboarding.stashed.take() else {
            return;
        };
        let (Some(file), Some(info_file)) =
            (self.onboarding.stash_file(), self.onboarding.stash_info_file())
        else {
            return;
        };
        self.open_native_path(file.clone());
        if self.current_file_path.as_deref() != Some(file.as_path()) {
            // Gagal dibuka: status sudah berisi error; sisihan disimpan untuk
            // dicoba lagi saat aplikasi dibuka berikutnya.
            self.onboarding.stashed = Some(info);
            return;
        }
        self.current_file_path = info.original_path;
        self.model.doc.dirty = info.dirty;
        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_file(&info_file);
        self.file_status = Some(ducad_i18n::t!("onboard-restored-doc"));
    }

    /// Buka tutorial dari awal (tombol bantuan, palet perintah, burger menu).
    pub fn start_onboarding(&mut self) {
        self.onboarding.state.restart();
        self.onboarding.reset_step_tracking();
    }

    /// Bab proyek contoh: dokumen berisi bracket contoh, tanpa riwayat undo,
    /// supaya undo pertama pengguna membatalkan revisinya sendiri.
    pub(crate) fn load_sample_project(&mut self) -> Result<(), String> {
        let session = ducad_engine::samples::bracket_design()
            .and_then(ducad_engine::Session::replay)
            .map_err(|e| e.message.clone())?;
        self.new_document();
        self.adopt_replayed_session(session, "sample");
        self.model_undo = ducad_core::UndoStack::default();
        self.model.doc.dirty = false;
        self.onboarding.volume_dirty = true;
        self.leave_sketch_mode();
        Ok(())
    }

    /// Bab 2 dan 3 melanjutkan hasil bab sebelumnya. Bila kanvas kosong saat
    /// bab itu dimulai (pengguna melompat dari kartu sambutan), geometri
    /// awalnya dibangun lewat oplog engine supaya langkah-langkahnya tetap
    /// bisa dikerjakan.
    pub(crate) fn onboarding_bootstrap_chapter(&mut self) {
        let step = self.onboarding.state.step;
        let chapter = self.onboarding.state.current().chapter;
        if step == 0 {
            return;
        }
        self.onboarding_take_document();
        if step != chapter.first_step() {
            return;
        }
        if chapter == OnboardingChapter::Sample {
            if ducad_engine::samples::is_bracket(&self.agent_meta.design) {
                return;
            }
            self.model_status = Some(match self.load_sample_project() {
                Ok(()) => ducad_i18n::t!("onboard-bootstrap-done"),
                Err(e) => {
                    log::warn!("proyek contoh gagal dibuka: {e}");
                    ducad_i18n::t!("onboard-bootstrap-failed")
                }
            });
            return;
        }
        if !self.model.doc.bodies.is_empty() {
            return;
        }
        let ops = match chapter {
            OnboardingChapter::Sample | OnboardingChapter::Beginner => return,
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
                self.leave_sketch_mode();
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
            if self.onboarding.stashed.is_some() {
                self.onboarding_restore_document();
            }
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
                self.onboarding_restore_document();
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
        let goals: Vec<_> = ONBOARDING_STEPS[OnboardingChapter::Beginner.first_step()..]
            .iter()
            .take_while(|s| s.kind != OnboardingStepKind::ChapterEnd)
            .map(|s| (s.kind, s.goal))
            .collect();
        assert_eq!(
            goals[0],
            (OnboardingStepKind::Overview, OnboardingGoal::None)
        );
        assert_eq!(goals[1..].len(), 7);
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


    fn total_volume(app: &DuCADApp) -> f64 {
        app.model.geometry.values().map(|g| g.shape.volume().abs()).sum()
    }

    /// Step 5: bab proyek contoh membuka bracket yang diuji CI, lalu
    /// mengajarkan revisi — ubah param, ubah fitur, undo — dengan deteksi
    /// berdasarkan perubahan desain yang nyata.
    #[test]
    fn sample_chapter_revises_the_tested_bracket() {
        let mut app = DuCADApp::new_for_test();
        app.start_onboarding();
        app.onboarding
            .state
            .go_to(OnboardingChapter::Sample.first_step());
        app.onboarding_bootstrap_chapter();
        assert!(ducad_engine::samples::is_bracket(&app.agent_meta.design));
        assert_eq!(app.model.doc.bodies.len(), 1);
        assert!(!app.can_undo(), "membuka contoh bukan langkah undo");
        let v0 = total_volume(&app);

        // 1. Parameter: len 60 -> 80.
        assert_eq!(app.onboarding.state.current().goal, OnboardingGoal::ParamEdit);
        app.onboarding_track();
        let mut rows: Vec<crate::design_tools::ParamRow> = app
            .agent_meta
            .design
            .params
            .iter()
            .map(|(k, v)| crate::design_tools::ParamRow {
                name: k.clone(),
                orig_name: Some(k.clone()),
                value: if k == "len" { "80".into() } else { format!("{v}") },
            })
            .collect();
        rows.sort_by(|a, b| a.name.cmp(&b.name));
        app.apply_param_rows(&rows).expect("param diterapkan");
        app.onboarding.volume_dirty = true;
        app.onboarding_track();
        assert!(app.onboarding.state.done, "param berubah");
        let v1 = total_volume(&app);
        assert!(v1 > v0 * 1.2, "bracket lebih panjang: {v0} -> {v1}");

        // 2. Fitur: radius fillet lewat Feature Tree. Mengubah param lagi
        //    bukan edit fitur.
        app.onboarding.state.go_to(app.onboarding.state.step + 1);
        app.onboarding.reset_step_tracking();
        assert_eq!(app.onboarding.state.current().goal, OnboardingGoal::FeatureEdit);
        app.onboarding_track();
        app.set_op_value("inner_fillet", "radius", 4.0);
        app.onboarding_track();
        assert!(app.onboarding.state.done, "op fillet berubah");

        // 3. Undo: membatalkan edit fitur saja.
        app.onboarding.state.go_to(app.onboarding.state.step + 1);
        app.onboarding.reset_step_tracking();
        assert_eq!(app.onboarding.state.current().goal, OnboardingGoal::Undo);
        app.onboarding_track();
        assert!(!app.onboarding.state.done);
        app.undo();
        app.onboarding_track();
        assert!(app.onboarding.state.done, "undo dijalankan");
        let fillet = app
            .agent_meta
            .design
            .oplog
            .iter()
            .find(|o| o.id() == "inner_fillet")
            .map(|o| serde_json::to_value(o).unwrap()["radius"].clone());
        assert_eq!(fillet, Some(serde_json::json!("$r")), "fillet kembali ke $r");
        assert_eq!(app.agent_meta.design.params["len"], 80.0, "param tetap");
    }

    /// Step 5: tutorial tidak mengubah dokumen kerja. Dokumen yang terbuka
    /// disisihkan saat bab dimulai dan dibuka kembali saat tutorial ditutup.
    #[test]
    fn tutorial_sets_aside_and_restores_the_working_document() {
        let path = temp_path("stash");
        let mut app = DuCADApp::new_for_test();
        app.onboarding = OnboardingCtl::load_from(path.clone(), true);
        add_box(&mut app, "kerja", [10.0, 20.0, 30.0]);
        let original = path.with_file_name("bracket-saya.ducad");
        app.current_file_path = Some(original.clone());
        app.model.doc.dirty = true;

        app.start_onboarding();
        app.onboarding
            .state
            .go_to(OnboardingChapter::Sample.first_step());
        app.onboarding_bootstrap_chapter();
        let stash = app.onboarding.stash_file().expect("ada path sisihan");
        assert!(stash.exists(), "dokumen kerja ditulis ke sisihan");
        assert!(ducad_engine::samples::is_bracket(&app.agent_meta.design));
        assert_eq!(app.current_file_path, None, "tutorial di dokumen sendiri");

        // Pindah langkah di dalam tutorial tidak menyisihkan ulang.
        app.onboarding.state.go_to(app.onboarding.state.step + 1);
        app.onboarding_bootstrap_chapter();
        assert!(ducad_engine::samples::is_bracket(&app.agent_meta.design));

        app.onboarding.state.open = false;
        app.onboarding_restore_document();
        assert_eq!(app.current_file_path, Some(original));
        assert!(app.model.doc.dirty, "status belum disimpan ikut kembali");
        let names: Vec<_> = app.model.doc.bodies.values().map(|b| b.name.clone()).collect();
        assert_eq!(names, ["kerja"]);
        assert!((total_volume(&app) - 6000.0).abs() < 1e-6);
        assert!(!stash.exists(), "sisihan dihapus setelah dibuka");
        let _ = std::fs::remove_dir_all(path.parent().expect("punya induk"));
    }

    /// Crash di tengah tutorial: sisihan dipulihkan saat aplikasi dibuka lagi.
    #[test]
    fn stash_left_by_a_crash_is_restored_on_next_start() {
        let path = temp_path("crash");
        let mut app = DuCADApp::new_for_test();
        app.onboarding = OnboardingCtl::load_from(path.clone(), true);
        add_box(&mut app, "kerja", [10.0, 10.0, 10.0]);
        app.start_onboarding();
        app.onboarding
            .state
            .go_to(OnboardingChapter::Beginner.first_step());
        app.onboarding_bootstrap_chapter();
        assert!(app.model.doc.bodies.is_empty(), "tutorial mulai kosong");
        drop(app); // "crash"

        let mut app = DuCADApp::new_for_test();
        app.onboarding = OnboardingCtl::load_from(path.clone(), true);
        assert!(app.onboarding.stashed.is_some());
        let ctx = egui::Context::default();
        let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(bounds),
                ..Default::default()
            },
            |ui| app.onboarding_frame(ui.ctx(), bounds),
        );
        out.textures_delta.clear();
        assert_eq!(app.model.doc.bodies.len(), 1, "dokumen kerja kembali");
        assert!(app.onboarding.stashed.is_none());
        let _ = std::fs::remove_dir_all(path.parent().expect("punya induk"));
    }

    /// Dokumen kosong tidak perlu disisihkan; tutorial jalan seperti biasa.
    #[test]
    fn empty_document_is_not_stashed() {
        let path = temp_path("empty");
        let mut app = DuCADApp::new_for_test();
        app.onboarding = OnboardingCtl::load_from(path.clone(), true);
        app.start_onboarding();
        app.onboarding
            .state
            .go_to(OnboardingChapter::Beginner.first_step());
        app.onboarding_bootstrap_chapter();
        assert!(!app.onboarding.stash_file().expect("path").exists());
        let _ = std::fs::remove_dir_all(path.parent().expect("punya induk"));
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
