//! Layar "Proyek baru dari brief" di GUI: satu layar untuk alur
//! brief → agent bekerja → paket desain → revisi → persetujuan.
//!
//! Layar ini TIDAK punya agent sendiri: ia menyusun instruksi lalu memakai
//! mesin Chat AI yang sama (`chat_send`/`chat_start`), sehingga provider,
//! privasi, CLI agent, dan pagar tool identik dengan panel Chat. Kejadian
//! loop disadap (`ChatState::tap`) untuk menggerakkan daftar tahap. Paket
//! dibangun di dalam proses (`package::build_package`) dan ditampilkan
//! lembar demi lembar dari display-list yang sama dengan PDF.
//!
//! Selama layar ini menjalankan agent, perubahan diterapkan langsung (tanpa
//! kartu Terima/Tolak per batch): pengguna sudah meminta hasilnya secara
//! eksplisit, tiap batch tetap satu langkah undo, dan persetujuan paket tetap
//! milik pengguna.

use ducad_chat::{ChatEvent, ChatModel};
use ducad_engine::check::CheckSummary;
use ducad_engine::package::{PackageInfo, PackagePage};
use ducad_io::drawing::scene::{build_scene, Scene};
use ducad_ui::{
    BriefLogUi, BriefPhase, BriefStage, BriefStepState, BriefStepUi, BriefStudio, BriefStudioData,
    BriefStudioEvent, BriefStudioState, ReviewPanelEvent, ReviewStage,
};

use crate::app::DuCADApp;

/// Tahap kerja agent yang dilacak dari tool yang dipanggilnya.
const STEP_BRIEF: usize = 0;
const STEP_RULES: usize = 1;
const STEP_MODEL: usize = 2;
const STEP_SIMULATE: usize = 3;
const STEP_VERIFY: usize = 4;
const STEP_PACKAGE: usize = 5;
const STEP_COUNT: usize = 6;

/// Batas baris log yang disimpan.
const MAX_LOG: usize = 400;

/// Keadaan layar milik `DuCADApp`.
#[derive(Default)]
pub struct BriefStudioApp {
    pub ui: BriefStudioState,
    phase: BriefPhase,
    /// Giliran agent yang dimulai dari layar ini masih berjalan.
    running: bool,
    /// Giliran yang berjalan adalah revisi (bukan generasi awal).
    revising: bool,
    done: [bool; STEP_COUNT],
    saw_simulate: bool,
    failed: bool,
    log: Vec<BriefLogUi>,
    /// Baris log terakhir masih dialiri teks agent.
    streaming: bool,
    pages: Vec<PackagePage>,
    /// Kunci keadaan (checks) saat paket terakhir dibangun.
    pages_key: Option<(u64, usize, u64)>,
    /// Naik tiap paket dibangun ulang (membatalkan cache display-list).
    generation: u64,
    scene: Option<((u64, usize), Scene)>,
    /// Naik saat status rilis berubah dari layar ini.
    release_epoch: u64,
    error: Option<String>,
    status: Option<String>,
    /// Permintaan terakhir (menjadi deskripsi revisi saat diajukan).
    last_request: String,
    /// Pemeriksaan `DUCAD_START_BRIEF` sudah dilakukan.
    startup_checked: bool,
    /// Tanda tangan checks saat studi terakhir dicoba dijalankan dari sini
    /// (mencegah pengulangan tanpa henti bila studinya memang gagal).
    studies_tried: Option<u64>,
}

/// Instruksi generasi awal. Bahasa Inggris seperti seluruh teks untuk agent;
/// isi brief tetap dalam bahasa pengguna.
pub fn generate_prompt(
    brief: &str,
    process: &str,
    material: &str,
    quantity: &str,
    images: usize,
) -> String {
    let mut facts = format!("Process: {process}");
    if !material.trim().is_empty() {
        facts.push_str(&format!(" | Material: {}", material.trim()));
    }
    if !quantity.trim().is_empty() {
        facts.push_str(&format!(" | Quantity: {}", quantity.trim()));
    }
    let attached = if images > 0 {
        format!("\n{images} image(s) are attached: read the shape and dimensions from them and state which numbers you took from the images.")
    } else {
        String::new()
    };
    format!(
        "Design the part described in the BRIEF and prepare it as a reviewable design package. \
Follow the DESIGN PACKAGE workflow in this order:\n\
1. `set_brief`: function, interfaces, loads, material, process, quantity and numbered requirements R1..Rn taken from the brief.\n\
2. `set_checks`: at least one check per requirement (field `requirement`) plus the `rule_packs` for the process; add `fea-safety` when the brief states a load.\n\
3. Model with params and small `run_ops` batches (dry_run first). Give the body a mechanical material with `set_material`; when a load is stated add a `study` op and call `simulate_static`.\n\
4. `run_checks` until nothing blocks, then `get_brief` to confirm every requirement is verified.\n\
Do NOT call `build_package`, `save_part`, `submit_for_review` or `propose_ops`: the app builds and shows the package itself and the user approves it.\n\
Do not stop to ask questions: when a dimension is missing choose a sensible value and record the assumption in the brief `notes`.\n\
Finish with at most four lines: what was built, the assumptions, and any warning that remains.\n\n\
BRIEF:\n{}\n{facts}{attached}",
        brief.trim()
    )
}

/// Instruksi revisi: perubahan terkecil, tanpa mulai ulang.
pub fn revision_prompt(
    request: &str,
    marks: &[(usize, String, [f32; 2], String)],
    images: usize,
) -> String {
    let mut text = String::from(
        "Revise the CURRENT design; do not start over and do not rebuild the model.\n",
    );
    if !request.trim().is_empty() {
        text.push_str(&format!("Requested change: {}\n", request.trim()));
    }
    if !marks.is_empty() {
        text.push_str("Marks the user placed on the package sheets (position in sheet mm from the lower-left corner):\n");
        for (i, (page, title, pos, note)) in marks.iter().enumerate() {
            text.push_str(&format!(
                "{}. sheet {} \"{title}\" at ({:.0}, {:.0}): {}\n",
                i + 1,
                page + 1,
                pos[0],
                pos[1],
                note.trim()
            ));
        }
    }
    if images > 0 {
        text.push_str(&format!("{images} image(s) are attached as reference.\n"));
    }
    text.push_str(
        "Use `set_params` first, then `replace_op`; add ops only for new features. Update `set_brief`/`set_checks` when a requirement changes. \
Then `run_checks`: if a change breaks a requirement, say so and do not force it. Read `list_redlines` and `resolve_redline` the ones you handled. \
Do NOT call `build_package`, `save_part`, `submit_for_review` or `propose_ops`. End with one line describing the revision.",
    );
    text
}

fn step_of(tool: &str) -> Option<usize> {
    Some(match tool {
        "set_brief" => STEP_BRIEF,
        "set_checks" => STEP_RULES,
        "run_ops" | "set_params" | "replace_op" | "remove_op" | "import_step" | "import_pdf" => {
            STEP_MODEL
        }
        "simulate_static" => STEP_SIMULATE,
        "run_checks" | "get_brief" | "inspect" | "render_view" | "query_geometry" | "measure"
        | "package_status" => STEP_VERIFY,
        _ => return None,
    })
}

impl BriefStudioApp {
    fn push_log(&mut self, ok: Option<bool>, text: String) {
        self.streaming = false;
        self.log.push(BriefLogUi { ok, text });
        if self.log.len() > MAX_LOG {
            let extra = self.log.len() - MAX_LOG;
            self.log.drain(..extra);
        }
    }

    /// Terapkan satu kejadian loop agent ke tahap dan log.
    fn apply(&mut self, event: ChatEvent) {
        match event {
            ChatEvent::TextDelta { text } => {
                if self.streaming {
                    if let Some(last) = self.log.last_mut() {
                        last.text.push_str(&text);
                        return;
                    }
                }
                self.push_log(None, text);
                self.streaming = true;
            }
            ChatEvent::ToolStart { name, .. } => {
                self.streaming = false;
                if name == "simulate_static" {
                    self.saw_simulate = true;
                }
            }
            ChatEvent::ToolDone {
                name,
                summary,
                is_error,
                ..
            } => {
                if !is_error {
                    if let Some(step) = step_of(&name) {
                        // Verifikasi baru bermakna setelah ada model.
                        if step != STEP_VERIFY || self.done[STEP_MODEL] {
                            self.done[step] = true;
                        }
                    }
                }
                // Nama tool cukup untuk langkah yang berhasil; hasil mentahnya
                // (JSON) hanya berguna saat gagal.
                if name == "ToolSearch" {
                    return;
                }
                let text = if is_error {
                    let short: String = summary.chars().take(220).collect();
                    format!("{name}: {short}")
                } else {
                    name
                };
                self.push_log(Some(!is_error), text);
            }
            ChatEvent::Error { message } => {
                self.failed = true;
                self.error = Some(message.clone());
                self.push_log(Some(false), message);
            }
            ChatEvent::Cancelled => {
                self.status = Some(ducad_i18n::t!("brief-status-stopped"));
            }
            ChatEvent::Done { note, .. } => {
                if let Some(note) = note {
                    self.push_log(None, note);
                }
            }
            ChatEvent::Usage { .. } => {}
        }
    }

    fn steps(&self, busy: bool) -> Vec<BriefStepUi> {
        let labels = [
            (STEP_BRIEF, "brief-step-brief"),
            (STEP_RULES, "brief-step-rules"),
            (STEP_MODEL, "brief-step-model"),
            (STEP_SIMULATE, "brief-step-simulate"),
            (STEP_VERIFY, "brief-step-verify"),
            (STEP_PACKAGE, "brief-step-package"),
        ];
        let visible: Vec<(usize, &str)> = labels
            .into_iter()
            .filter(|(i, _)| *i != STEP_SIMULATE || self.saw_simulate)
            .collect();
        let active = visible
            .iter()
            .find(|(i, _)| !self.done[*i])
            .map(|(i, _)| *i);
        visible
            .into_iter()
            .map(|(i, key)| BriefStepUi {
                label: ducad_i18n::translate(key, None),
                state: if self.done[i] {
                    BriefStepState::Done
                } else if Some(i) == active && self.failed {
                    BriefStepState::Failed
                } else if Some(i) == active && busy {
                    BriefStepState::Active
                } else {
                    BriefStepState::Pending
                },
            })
            .collect()
    }
}

impl DuCADApp {
    /// Buka layar. Panel lain yang menumpang di area kerja ditutup supaya
    /// tidak menimpa layar penuh ini.
    pub fn open_brief_studio(&mut self) {
        self.chat.ensure_loaded();
        self.sync_ai_privacy();
        self.chat.panel.open = false;
        self.review.panel_open = false;
        self.sim.panel_open = false;
        self.industry.panel_open = false;
        self.checks.panel_open = false;
        self.drawing_sheet_state.is_open = false;
        self.brief.ui.open = true;
        // Dokumen yang sudah punya brief langsung menampilkan paketnya.
        if self.brief.phase == BriefPhase::Compose
            && !self.brief.running
            && !self.model.doc.bodies.is_empty()
            && self
                .design
                .as_ref()
                .is_some_and(|d| d.get("brief").is_some_and(|b| !b.is_null()))
        {
            self.brief.phase = BriefPhase::Result;
            self.brief.pages_key = None;
        }
    }

    /// Alasan agent belum bisa dijalankan dari layar ini.
    fn brief_agent_blocked(&self) -> Option<String> {
        if self.chat.is_running() && !self.brief.running {
            return Some(ducad_i18n::t!("brief-blocked-busy"));
        }
        if self.chat.settings.backend == ducad_chat::ChatBackend::Cli {
            return None;
        }
        let cfg = &self.chat.settings.provider;
        if !cfg.is_local() && !self.chat.settings.allow_external {
            return Some(ducad_i18n::t!("brief-blocked-privacy"));
        }
        None
    }

    /// Jalankan agent dengan `prompt`. `model` diisi tes (model terskrip);
    /// `None` = provider/CLI dari pengaturan Chat AI.
    pub(crate) fn brief_start(
        &mut self,
        ctx: &egui::Context,
        prompt: String,
        revising: bool,
        model: Option<Box<dyn ChatModel>>,
    ) {
        if self.chat.is_running() {
            self.brief.error = Some(ducad_i18n::t!("brief-blocked-busy"));
            return;
        }
        self.brief.error = None;
        self.brief.status = None;
        self.brief.failed = false;
        self.brief.revising = revising;
        if !revising {
            self.brief.done = [false; STEP_COUNT];
            self.brief.saw_simulate = false;
            self.brief.log.clear();
        } else {
            // Revisi: model dan verifikasi diulang, brief/aturan sudah ada.
            self.brief.done[STEP_MODEL] = false;
            self.brief.done[STEP_VERIFY] = false;
            self.brief.done[STEP_PACKAGE] = false;
        }
        let errors_before = self.chat.error_count();
        self.chat.tap = Some(Vec::new());
        self.chat.auto_apply = true;
        self.bridge.force_propose = false;
        self.chat.panel.input = prompt;
        match model {
            Some(model) => self.chat_start(ctx, model),
            None => self.chat_send(ctx),
        }
        if self.chat.is_running() {
            self.brief.running = true;
            if !revising {
                self.brief.phase = BriefPhase::Running;
            }
        } else {
            // Ditolak sebelum berjalan (privasi, CLI tidak ditemukan, …).
            self.brief.error = self
                .chat
                .last_error_after(errors_before)
                .or_else(|| Some(ducad_i18n::t!("brief-blocked-busy")));
            self.brief_restore_chat();
        }
    }

    fn brief_restore_chat(&mut self) {
        self.chat.tap = None;
        self.chat.auto_apply = false;
        self.sync_ai_privacy();
    }

    /// Jalankan studi simulasi yang hasilnya belum ada/kedaluwarsa, supaya
    /// check tegangan bisa dinilai (mis. berkas buatan CLI yang baru dibuka).
    /// Mengembalikan `true` bila ada studi yang dijalankan.
    fn brief_refresh_studies(&mut self) -> bool {
        // Definisi studi dibaca dari dokumen (`self.design`, lengkap dengan
        // oplog): `agent_meta` berkas yang dibuka dari disk tidak membawa
        // oplog, jadi studinya tidak terlihat di sana.
        let Some(design) = self
            .design
            .clone()
            .and_then(|d| serde_json::from_value::<ducad_engine::DesignDoc>(d).ok())
        else {
            return false;
        };
        let mut meta = ducad_engine::SessionMeta {
            design,
            sim_results: self.agent_meta.sim_results.clone(),
            analysis_results: self.agent_meta.analysis_results.clone(),
            ..Default::default()
        };
        let stale = ducad_engine::sim::study_defs(&meta)
            .into_iter()
            .any(|(id, def)| {
                if def.kind.has_stress() {
                    ducad_engine::sim::fresh_result(&self.model, &meta, id).is_err()
                } else {
                    ducad_engine::sim::fresh_analysis(&self.model, &meta, id).is_err()
                }
            });
        if !stale || self.brief.studies_tried == Some(self.checks.signature) {
            return false;
        }
        self.brief.studies_tried = Some(self.checks.signature);
        {
            let mut core = ducad_engine::SessionCore {
                model: &mut self.model,
                model_undo: &mut self.model_undo,
                sketches: &mut self.sketch_set,
                meta: &mut meta,
            };
            let _ = ducad_engine::sim::run_all_studies(&mut core, &ducad_sim::CancelToken::new());
        }
        self.agent_meta.sim_results = meta.sim_results;
        self.agent_meta.analysis_results = meta.analysis_results;
        true
    }

    /// Bangun paket dari model + dokumen saat ini.
    fn brief_rebuild_package(&mut self) {
        self.sync_agent_meta();
        let design = &self.agent_meta.design;
        let stem = self
            .current_file_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .map(str::to_string);
        let title = design
            .brief
            .as_ref()
            .map(|b| b.title.clone())
            .filter(|t| !t.trim().is_empty())
            .or_else(|| stem.clone())
            .unwrap_or_else(|| "Design".to_string());
        // Nomor gambar dari judul: huruf/angka saja, maks. 20 karakter supaya
        // muat di kolom kepala gambar.
        let part_number = stem.unwrap_or_else(|| {
            let slug: String = title
                .to_lowercase()
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                .collect();
            let mut out = String::new();
            for part in slug.split('-').filter(|p| !p.is_empty()) {
                if out.len() + part.len() + 1 > 20 {
                    break;
                }
                if !out.is_empty() {
                    out.push('-');
                }
                out.push_str(part);
            }
            if out.is_empty() {
                "design".to_string()
            } else {
                out
            }
        });
        let firm = ducad_engine::firm::find_firm_profile(
            None,
            self.current_file_path.as_ref().and_then(|p| p.parent()),
        )
        .and_then(|p| ducad_engine::firm::FirmProfile::load(&p).ok())
        .unwrap_or_default();
        let summary = CheckSummary::from_results(self.checks.results.clone());
        let info = PackageInfo {
            title,
            part_number,
            author: String::new(),
            date: ducad_engine::brief::today(),
        };
        let spec = design.package.clone().unwrap_or_default();
        match ducad_engine::package::build_package(
            &self.model,
            &self.agent_meta,
            &spec,
            &info,
            &firm,
            Some(&summary),
        ) {
            Ok(out) => {
                self.brief.pages = out.pages;
                self.brief.done[STEP_PACKAGE] = true;
            }
            Err(e) => {
                self.brief.pages.clear();
                self.brief.error = Some(ducad_i18n::t!(
                    "brief-error-package",
                    error = e.message.as_str()
                ));
            }
        }
        self.brief.generation += 1;
        self.brief.scene = None;
    }

    /// Kunci keadaan yang menentukan isi paket: tanda tangan checks, jumlah
    /// check yang masih dihitung di latar, dan status rilis.
    fn brief_pages_key(&self) -> (u64, usize, u64) {
        (
            self.checks.signature,
            self.checks.stale.iter().filter(|s| **s).count(),
            self.brief.release_epoch,
        )
    }

    /// Giliran agent selesai: pindah ke paket, atau kembali ke brief bila
    /// tidak ada model yang dihasilkan.
    fn brief_finished(&mut self, ctx: &egui::Context) {
        self.brief.running = false;
        self.brief.streaming = false;
        self.brief_restore_chat();
        if self.model.doc.bodies.is_empty() {
            self.brief.phase = BriefPhase::Compose;
            if self.brief.error.is_none() {
                self.brief.error = Some(ducad_i18n::t!("brief-error-no-model"));
            }
            return;
        }
        self.refresh_checks(ctx);
        self.review_touch();
        self.brief.phase = BriefPhase::Result;
        self.brief.pages_key = None;
        if self.brief.revising {
            self.brief.ui.marks.clear();
            self.brief.ui.revision.clear();
            if self.brief.error.is_none() {
                self.brief.status = Some(ducad_i18n::t!("brief-status-revised"));
            }
        }
        self.brief.revising = false;
    }

    /// Dipanggil tiap frame dari `update`; menggambar layar bila terbuka.
    pub fn brief_studio_frame(&mut self, ctx: &egui::Context, rect: egui::Rect) {
        // `DUCAD_START_BRIEF=1`: aplikasi langsung membuka layar ini.
        if !self.brief.startup_checked {
            self.brief.startup_checked = true;
            // `DUCAD_OPEN_FILE=<path.ducad>`: buka berkas saat mulai (demo/uji).
            if let Some(path) = std::env::var_os("DUCAD_OPEN_FILE").filter(|p| !p.is_empty()) {
                self.open_native_path(std::path::PathBuf::from(path));
            }
            if std::env::var_os("DUCAD_START_BRIEF").is_some_and(|v| v != "0" && !v.is_empty()) {
                self.open_brief_studio();
                // `DUCAD_BRIEF_PAGE=<n>`: lembar yang ditampilkan (mulai 1).
                if let Some(n) = std::env::var("DUCAD_BRIEF_PAGE")
                    .ok()
                    .and_then(|v| v.parse::<usize>().ok())
                {
                    self.brief.ui.page = n.saturating_sub(1);
                }
            }
            // `DUCAD_BRIEF_AUTORUN="<brief>"`: buka layar, isi brief, dan
            // langsung jalankan agent (demo/uji ujung ke ujung tanpa klik).
            if let Some(text) = std::env::var("DUCAD_BRIEF_AUTORUN")
                .ok()
                .filter(|t| !t.trim().is_empty())
            {
                self.open_brief_studio();
                self.brief.ui.brief = text;
                self.handle_brief_event(ctx, BriefStudioEvent::Generate);
            }
        }
        // Sadapan diproses walau layar ditutup, supaya giliran yang masih
        // berjalan tetap berakhir rapi.
        self.chat_poll(ctx);
        if let Some(events) = self.chat.tap.as_mut().map(std::mem::take) {
            for e in events {
                self.brief.apply(e);
            }
        }
        if self.brief.running && !self.chat.is_running() {
            self.brief_finished(ctx);
        }
        if !self.brief.ui.open {
            return;
        }
        if self.brief.phase == BriefPhase::Result && !self.brief.running {
            let key = self.brief_pages_key();
            if self.brief.pages_key != Some(key) && !ctx.input(|i| i.pointer.any_down()) {
                self.sync_agent_meta();
                if self.brief_refresh_studies() {
                    // Hasil studi baru: checks dihitung ulang dulu, paket
                    // dibangun pada frame berikutnya.
                    self.refresh_checks(ctx);
                    ctx.request_repaint();
                } else {
                    self.brief_rebuild_package();
                    self.brief.pages_key = Some(self.brief_pages_key());
                }
            }
        }
        let data = self.brief_studio_data();
        let mut event = None;
        egui::Area::new(egui::Id::new("ducad-brief-studio-overlay"))
            .fixed_pos(rect.min)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.set_width(rect.width());
                ui.set_height(rect.height());
                event = BriefStudio::show(ui, &mut self.brief.ui, &data);
            });
        if let Some(event) = event {
            self.handle_brief_event(ctx, event);
        }
        if self.brief.running {
            ctx.request_repaint_after(std::time::Duration::from_millis(60));
        }
    }

    pub(crate) fn brief_studio_data(&mut self) -> BriefStudioData {
        let busy = self.brief.running;
        let page = self
            .brief
            .ui
            .page
            .min(self.brief.pages.len().saturating_sub(1));
        let scene_key = (self.brief.generation, page);
        if self.brief.scene.as_ref().map(|(k, _)| *k) != Some(scene_key) {
            self.brief.scene = self.brief.pages.get(page).map(|p| {
                let mut scene = build_scene(&p.sheet);
                scene.groups.extend(p.groups.iter().cloned());
                (scene_key, scene)
            });
        }
        let review = self.review_panel_data();
        let summary = CheckSummary::from_results(self.checks.results.clone());
        let count =
            |s: ducad_ui::ReviewTrace| review.requirements.iter().filter(|r| r.status == s).count();
        BriefStudioData {
            phase: self.brief.phase,
            busy,
            agent_label: self.chat.panel.provider_label.clone(),
            agent_blocked: self.brief_agent_blocked(),
            document_has_model: !self.model.doc.bodies.is_empty(),
            steps: self.brief.steps(busy),
            log: self.brief.log.clone(),
            pages: self.brief.pages.iter().map(|p| p.title.clone()).collect(),
            scene: self.brief.scene.as_ref().map(|(_, s)| s.clone()),
            stage: match review.stage {
                ReviewStage::Draft => BriefStage::Draft,
                ReviewStage::InReview => BriefStage::InReview,
                ReviewStage::Approved => BriefStage::Approved,
            },
            revision: review.revision.clone(),
            approved_by: review.approved_by.clone(),
            // Check yang masih dihitung di latar belum menjadi penghalang
            // sungguhan: tampilkan sebagai "sedang dihitung".
            blockers: if self.checks.stale.iter().any(|s| *s) {
                vec![ducad_i18n::t!("checks-stale")]
            } else {
                review.blockers.clone()
            },
            checks: (summary.pass, summary.fail + summary.error, summary.warn),
            requirements: (
                count(ducad_ui::ReviewTrace::Verified),
                count(ducad_ui::ReviewTrace::Failed),
                count(ducad_ui::ReviewTrace::Unverified),
            ),
            title: if review.title.trim().is_empty() {
                ducad_i18n::t!("brief-title")
            } else {
                review.title.clone()
            },
            status: self.brief.status.clone(),
            error: self.brief.error.clone(),
            attachments: self.chat.pending_images.len(),
        }
    }

    pub(crate) fn handle_brief_event(&mut self, ctx: &egui::Context, event: BriefStudioEvent) {
        match event {
            BriefStudioEvent::Close => self.brief.ui.open = false,
            BriefStudioEvent::OpenChatSettings => {
                self.brief.ui.open = false;
                self.open_chat();
                self.chat.panel.settings_open = true;
            }
            BriefStudioEvent::AttachImage => self.chat_attach_image(),
            BriefStudioEvent::ClearAttachments => {
                self.chat.pending_images.clear();
                self.chat.panel.attachments = 0;
            }
            BriefStudioEvent::Stop => self.chat_stop(),
            BriefStudioEvent::NewBrief => {
                if !self.brief.running {
                    self.brief.phase = BriefPhase::Compose;
                    self.brief.error = None;
                    self.brief.status = None;
                }
            }
            BriefStudioEvent::Generate => {
                let ui = &self.brief.ui;
                let prompt = generate_prompt(
                    &ui.brief,
                    ui.process_key(),
                    &ui.material,
                    &ui.quantity,
                    self.chat.pending_images.len(),
                );
                self.brief.last_request = ui.brief.trim().chars().take(80).collect();
                self.brief_start(ctx, prompt, false, None);
            }
            BriefStudioEvent::SendRevision => {
                let marks: Vec<(usize, String, [f32; 2], String)> = self
                    .brief
                    .ui
                    .marks
                    .iter()
                    .filter(|m| !m.text.trim().is_empty())
                    .map(|m| {
                        let title = self
                            .brief
                            .pages
                            .get(m.page)
                            .map(|p| p.title.clone())
                            .unwrap_or_default();
                        (m.page, title, m.position, m.text.clone())
                    })
                    .collect();
                let request = self.brief.ui.revision.trim().to_string();
                let prompt = revision_prompt(&request, &marks, self.chat.pending_images.len());
                // Deskripsi revisi untuk tabel revisi: teks permintaan, atau
                // catatan tanda pertama.
                self.brief.last_request = if request.is_empty() {
                    marks.first().map(|m| m.3.clone()).unwrap_or_default()
                } else {
                    request
                }
                .chars()
                .take(80)
                .collect();
                self.brief_start(ctx, prompt, true, None);
            }
            BriefStudioEvent::Approve => {
                let by = self.brief.ui.approver.trim().to_string();
                let description = if self.brief.last_request.is_empty() {
                    "Initial release".to_string()
                } else {
                    self.brief.last_request.clone()
                };
                self.handle_review_event(ReviewPanelEvent::Submit {
                    description,
                    by: by.clone(),
                });
                self.handle_review_event(ReviewPanelEvent::Approve { by });
                self.brief.status = self.review.status.clone();
                self.brief.release_epoch += 1;
            }
            BriefStudioEvent::ExportPdf => {
                let name = format!(
                    "{}-package.pdf",
                    self.current_file_path
                        .as_ref()
                        .and_then(|p| p.file_stem())
                        .and_then(|s| s.to_str())
                        .unwrap_or("design")
                );
                let filter = ducad_i18n::t!("file-pdf-filter");
                let Some(path) = self.pick_save_path(&filter, &["pdf"], &name) else {
                    return;
                };
                let pages: Vec<ducad_io::pdf::PdfPage<'_>> = self
                    .brief
                    .pages
                    .iter()
                    .map(|p| ducad_io::pdf::PdfPage {
                        sheet: &p.sheet,
                        extra: &p.groups,
                        bookmark: &p.title,
                    })
                    .collect();
                let written = std::fs::write(&path, ducad_io::pdf::generate_pdf_document(&pages));
                self.brief.status = Some(match written {
                    Ok(()) => {
                        let shown = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("package.pdf")
                            .to_string();
                        let sheets = self.brief.pages.len().to_string();
                        ducad_i18n::t!(
                            "brief-status-exported",
                            name = shown.as_str(),
                            sheets = sheets.as_str()
                        )
                    }
                    Err(e) => {
                        let e = e.to_string();
                        ducad_i18n::t!("review-status-failed", error = e.as_str())
                    }
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_chat::{ModelTurn, ScriptedModel, StopReason, ToolCall, Usage};
    use serde_json::json;
    use std::time::{Duration, Instant};

    fn call(id: &str, name: &str, input: serde_json::Value) -> ModelTurn {
        ModelTurn {
            text: String::new(),
            calls: vec![ToolCall {
                id: id.into(),
                name: name.into(),
                input,
                input_error: None,
            }],
            raw: None,
            stop: StopReason::ToolUse,
            usage: Usage::default(),
        }
    }

    fn frame(app: &mut DuCADApp, ctx: &egui::Context) {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0));
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(rect),
                ..Default::default()
            },
            |ui| {
                let ctx = ui.ctx().clone();
                // Urutan yang sama dengan `update` aplikasi.
                app.refresh_checks(&ctx);
                app.brief_studio_frame(&ctx, rect);
            },
        );
        out.textures_delta.clear();
    }

    fn pump(app: &mut DuCADApp, ctx: &egui::Context) {
        let deadline = Instant::now() + Duration::from_secs(120);
        while app.brief.running && Instant::now() < deadline {
            app.poll_agent_bridge(ctx);
            frame(app, ctx);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(!app.brief.running, "agent tidak selesai");
        // Check `min_wall` dihitung di thread latar; paket dibangun ulang
        // begitu hasilnya tiba.
        while app.checks.stale.iter().any(|s| *s) && Instant::now() < deadline {
            frame(app, ctx);
            std::thread::sleep(Duration::from_millis(5));
        }
        frame(app, ctx);
        frame(app, ctx);
    }

    fn app() -> (DuCADApp, egui::Context) {
        let mut app = DuCADApp::new_for_test();
        app.chat.loaded_for_test();
        app.open_brief_studio();
        (app, egui::Context::default())
    }

    /// Agent terskrip yang mengikuti alur paket desain.
    fn design_script() -> ScriptedModel {
        ScriptedModel::new([
            call(
                "c1",
                "set_brief",
                json!({ "brief": {
                "title": "Mounting plate", "function": "Carries a motor.", "process": "cnc",
                "requirements": [{ "id": "R1", "text": "Plate 60 x 40 x 8 mm" },
                                 { "id": "R2", "text": "Millable on 3 axes" }] } }),
            ),
            call(
                "c2",
                "run_ops",
                json!({ "ops": [
                { "op": "primitive", "id": "plate", "shape": { "box": { "size": [60, 40, 8] } } }] }),
            ),
            call(
                "c3",
                "set_checks",
                json!({
                "checks": [{ "id": "size", "requirement": "R1", "check": "bbox_size",
                             "body": "plate", "expect": [60, 40, 8] }],
                "rule_packs": [{ "pack": "dfm-cnc", "requirement": "R2" }] }),
            ),
            call("c4", "run_checks", json!({})),
            ModelTurn {
                text: "Plat 60 x 40 x 8 selesai.".into(),
                ..Default::default()
            },
        ])
    }

    #[test]
    fn brief_becomes_a_package_shown_sheet_by_sheet() {
        let (mut app, ctx) = app();
        // Pengaturan chat bawaan meminta konfirmasi tulis; layar ini tidak.
        app.chat.settings.confirm_writes = true;
        app.brief.ui.brief = "Plat dudukan motor 60 x 40 x 8".into();
        let prompt = generate_prompt(&app.brief.ui.brief, "cnc", "Al 6061-T6", "10", 0);
        assert!(prompt.contains("BRIEF:\nPlat dudukan motor 60 x 40 x 8"));
        assert!(prompt.contains("Process: cnc | Material: Al 6061-T6 | Quantity: 10"));
        app.brief_start(&ctx, prompt, false, Some(Box::new(design_script())));
        assert!(app.brief.running);
        assert_eq!(app.brief_studio_data().phase, BriefPhase::Running);
        pump(&mut app, &ctx);

        let data = app.brief_studio_data();
        assert_eq!(data.phase, BriefPhase::Result, "{:?}", data.error);
        assert!(data.error.is_none(), "{:?}", data.error);
        assert_eq!(data.title, "Mounting plate");
        assert_eq!(data.pages, vec!["Cover", "Part - plate", "Compliance"]);
        assert!(
            data.steps.iter().all(|s| s.state == BriefStepState::Done),
            "{:?}",
            data.steps
        );
        assert_eq!(data.steps.len(), 5, "tanpa tahap simulasi");
        assert_eq!(data.requirements, (2, 0, 0));
        assert_eq!(data.checks.1, 0);
        assert!(data
            .log
            .iter()
            .any(|l| l.text == "run_ops" && l.ok == Some(true)));
        assert!(data
            .log
            .iter()
            .any(|l| l.ok.is_none() && l.text.contains("selesai")));
        // Lembar pertama tergambar sebagai display-list; pindah lembar berganti isi.
        let cover = data.scene.expect("display-list lembar");
        assert!(cover.groups.iter().any(|g| g.id == "document"));
        app.brief.ui.page = 1;
        let part = app.brief_studio_data().scene.unwrap();
        assert!(!part.groups.iter().any(|g| g.id == "document"));
        // Perubahan diterapkan langsung, dan pengaturan chat dipulihkan.
        assert_eq!(app.native_body_refs().len(), 1);
        assert!(
            app.bridge.force_propose,
            "konfirmasi tulis kembali aktif setelah selesai"
        );
        assert!(app.chat.tap.is_none());
    }

    #[test]
    fn revision_keeps_the_design_and_approval_needs_the_user() {
        let (mut app, ctx) = app();
        app.brief_start(
            &ctx,
            "buat plat".into(),
            false,
            Some(Box::new(design_script())),
        );
        pump(&mut app, &ctx);
        assert_eq!(app.brief_studio_data().stage, BriefStage::Draft);

        // Setujui dari layar ini.
        app.brief.ui.approver = "budi".into();
        app.handle_brief_event(&ctx, BriefStudioEvent::Approve);
        let data = app.brief_studio_data();
        assert_eq!(data.stage, BriefStage::Approved, "{:?}", data.status);
        assert!(data.approved_by.is_some());

        // Revisi: tanda di lembar + teks → agent mengubah, bukan mulai ulang.
        app.brief.ui.marks.push(ducad_ui::BriefMark {
            page: 1,
            position: [120.0, 150.0],
            text: "tambah boss di sini".into(),
        });
        let prompt = revision_prompt(
            "tambah boss",
            &[(
                1,
                "Part - plate".into(),
                [120.0, 150.0],
                "tambah boss di sini".into(),
            )],
            0,
        );
        assert!(prompt.contains("sheet 2 \"Part - plate\" at (120, 150): tambah boss di sini"));
        assert!(prompt.contains("do not start over"));
        let revise = ScriptedModel::new([
            call(
                "r1",
                "run_ops",
                json!({ "ops": [
                { "op": "primitive", "id": "boss", "shape": { "cylinder": { "r": 5, "h": 20 } } }] }),
            ),
            ModelTurn {
                text: "Boss ditambahkan.".into(),
                ..Default::default()
            },
        ]);
        app.brief_start(&ctx, prompt, true, Some(Box::new(revise)));
        assert_eq!(
            app.brief_studio_data().phase,
            BriefPhase::Result,
            "revisi tetap di layar paket"
        );
        pump(&mut app, &ctx);

        let data = app.brief_studio_data();
        assert_eq!(app.native_body_refs().len(), 2, "model lama dipertahankan");
        assert_eq!(
            data.stage,
            BriefStage::Draft,
            "persetujuan gugur setelah revisi"
        );
        assert!(
            app.brief.ui.marks.is_empty(),
            "tanda dibersihkan setelah revisi"
        );
        assert!(
            data.pages.contains(&"Assembly".to_string()),
            "{:?}",
            data.pages
        );
        // Pengajuan berikutnya menjadi revisi B.
        app.handle_brief_event(&ctx, BriefStudioEvent::Approve);
        assert_eq!(app.brief_studio_data().revision, "B");
    }

    #[test]
    fn a_run_without_a_model_returns_to_the_brief_with_an_error() {
        let (mut app, ctx) = app();
        let model = ScriptedModel::new([ModelTurn {
            text: "Maaf, saya butuh ukuran.".into(),
            ..Default::default()
        }]);
        app.brief_start(&ctx, "buat sesuatu".into(), false, Some(Box::new(model)));
        pump(&mut app, &ctx);
        let data = app.brief_studio_data();
        assert_eq!(data.phase, BriefPhase::Compose);
        assert!(data.error.is_some());
        assert!(data.pages.is_empty());
    }

    #[test]
    fn external_ai_must_be_allowed_before_generating() {
        let (mut app, ctx) = app();
        // Bawaan: provider jaringan, AI eksternal belum diizinkan.
        let data = app.brief_studio_data();
        assert!(data.agent_blocked.is_some());
        app.brief.ui.brief = "buat plat".into();
        app.handle_brief_event(&ctx, BriefStudioEvent::Generate);
        assert!(!app.brief.running);
        assert!(app.brief_studio_data().error.is_some());
        assert_eq!(app.native_body_refs().len(), 0);
    }
}
