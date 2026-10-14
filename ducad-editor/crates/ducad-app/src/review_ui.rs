//! Panel "Review & Rilis" di GUI (W0–W5): brief, keterlacakan persyaratan,
//! redline, dan gerbang persetujuan paket desain.
//!
//! Semua perubahan dokumen lewat `agent_meta.design` (pola yang sama dengan
//! `save_native_to`): `sync_agent_meta` → ubah → `sync_design_after_agent`.
//! Redline dan brief memakai tool engine yang sama dengan agent
//! (`agent_call_local`), jadi perilakunya identik dengan MCP. Persetujuan
//! TIDAK punya tool: hanya tombol di panel ini (atau `ducad-cli release`).

use ducad_core::drawing_annot::Annotation;
use ducad_engine::brief::{
    release_signature, review_blockers, review_checklist, today, traceability, ReleaseState,
    TraceStatus,
};
use ducad_engine::DesignDoc;
use ducad_ui::{
    ReviewItemUi, ReviewPanel, ReviewPanelData, ReviewPanelEvent, ReviewRedlineUi,
    ReviewRequirementUi, ReviewRevisionUi, ReviewStage, ReviewTrace,
};
use serde_json::json;

use crate::app::DuCADApp;

/// Keadaan panel review milik `DuCADApp`.
#[derive(Default)]
pub struct ReviewState {
    pub panel_open: bool,
    pub panel: ReviewPanel,
    /// Pesan hasil aksi terakhir.
    pub status: Option<String>,
    /// Titik redline (mm kertas) yang menunggu teks dari panel.
    pub pending_redline: Option<[f64; 2]>,
    /// Data panel + kunci cache-nya (tanda tangan checks, hitungan mutasi).
    cache: Option<((u64, u64, usize), ReviewPanelData)>,
    /// Naik setiap dokumen diubah dari panel ini.
    epoch: u64,
}

impl ReviewState {
    fn touch(&mut self) {
        self.epoch += 1;
        self.cache = None;
    }
}

fn redline_annotations(spec: &ducad_io::drawing::DrawingSpec) -> Vec<Annotation> {
    spec.layout
        .iter()
        .flat_map(|l| l.annotations.iter())
        .filter(|a| matches!(a, Annotation::Redline { .. }))
        .cloned()
        .collect()
}

fn item_label(item: &str) -> String {
    match item {
        "checks" => ducad_i18n::t!("review-item-checks"),
        "requirements" => ducad_i18n::t!("review-item-requirements"),
        "standards" => ducad_i18n::t!("review-item-standards"),
        "drawings" => ducad_i18n::t!("review-item-drawings"),
        "redlines" => ducad_i18n::t!("review-item-redlines"),
        other => other.to_string(),
    }
}

impl DuCADApp {
    /// Dokumen desain apa adanya (tanpa menyinkronkan geometri).
    fn review_design(&self) -> DesignDoc {
        self.design
            .clone()
            .and_then(|d| serde_json::from_value(d).ok())
            .unwrap_or_default()
    }

    /// Tanda tangan desain untuk status rilis. Geometri yang diedit manual
    /// di GUI (belum tersinkron ke `design`) dianggap perubahan desain.
    fn review_signature(&self, design: &DesignDoc) -> String {
        let signature = release_signature(&self.model, design);
        let fp = ducad_engine::session::fingerprint(&self.model);
        if design.fingerprint.is_empty() || design.fingerprint == fp {
            signature
        } else {
            format!("{signature}+{fp}")
        }
    }

    /// Data panel; dihitung ulang hanya bila checks/dokumen berubah.
    pub fn review_panel_data(&mut self) -> ReviewPanelData {
        // Jumlah check yang masih dihitung di latar ikut menjadi kunci:
        // hasil `min_wall` tiba tanpa mengubah tanda tangan checks.
        let key = (
            self.checks.signature,
            self.review.epoch,
            self.checks.stale.iter().filter(|s| **s).count(),
        );
        if let Some((k, data)) = &self.review.cache {
            if *k == key {
                let mut data = data.clone();
                data.status = self.review.status.clone();
                data.pending_redline = self.review.pending_redline.is_some();
                return data;
            }
        }
        let design = self.review_design();
        let signature = self.review_signature(&design);
        let release = design.release.clone().unwrap_or_default();
        let state = release.effective_state(&signature);
        let results = &self.checks.results;
        let brief = design.brief.clone().unwrap_or_default();
        let trace = traceability(&brief, results);
        let checklist = review_checklist(&design, results);
        let blockers: Vec<String> = checklist
            .iter()
            .filter(|i| !i.ok)
            .filter_map(|i| match i.item {
                "checks" if !results.is_empty() => Some(ducad_i18n::t!("review-blocker-checks")),
                "redlines" => Some(ducad_i18n::t!("review-blocker-redlines")),
                _ => None,
            })
            .collect();
        let mut redlines = Vec::new();
        for spec in &design.drawings {
            for r in ducad_io::drawing::redlines(spec) {
                if !r.resolved {
                    redlines.push(ReviewRedlineUi {
                        sheet: spec.name.clone(),
                        id: r.id.clone(),
                        text: r.text.clone(),
                        author: r.author.clone(),
                    });
                }
            }
        }
        let data = ReviewPanelData {
            has_brief: design.brief.is_some(),
            title: brief.title.clone(),
            function: brief.function.clone(),
            process: brief
                .process
                .and_then(|p| serde_json::to_value(p).ok())
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default(),
            requirements: trace
                .rows
                .iter()
                .map(|row| ReviewRequirementUi {
                    id: row.id.clone(),
                    text: row.text.clone(),
                    status: match row.status {
                        TraceStatus::Verified => ReviewTrace::Verified,
                        TraceStatus::Failed => ReviewTrace::Failed,
                        TraceStatus::Unverified => ReviewTrace::Unverified,
                    },
                    checks: row
                        .checks
                        .iter()
                        .map(|c| c.check.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                })
                .collect(),
            stage: match state {
                ReleaseState::Draft => ReviewStage::Draft,
                ReleaseState::InReview => ReviewStage::InReview,
                ReleaseState::Approved => ReviewStage::Approved,
            },
            revision: release.revision.clone(),
            approved_by: release
                .approval
                .as_ref()
                .filter(|_| state == ReleaseState::Approved)
                .map(|a| format!("{} ({})", a.by, a.date)),
            approval_stale: release.approval_is_stale(&signature),
            checklist: checklist
                .iter()
                .map(|i| ReviewItemUi {
                    label: item_label(i.item),
                    ok: i.ok,
                    detail: i.detail.clone(),
                })
                .collect(),
            blockers,
            redlines,
            history: release
                .history
                .iter()
                .map(|h| ReviewRevisionUi {
                    revision: h.revision.clone(),
                    date: h.date.clone(),
                    by: h.by.clone(),
                    description: h.description.clone(),
                })
                .collect(),
            pending_redline: self.review.pending_redline.is_some(),
            status: self.review.status.clone(),
        };
        self.review.cache = Some((key, data.clone()));
        data
    }

    /// Ubah `agent_meta.design` lalu tulis balik ke `self.design`.
    fn review_mutate(&mut self, f: impl FnOnce(&mut DesignDoc, &str) -> Result<String, String>) {
        self.sync_agent_meta();
        let signature = release_signature(&self.model, &self.agent_meta.design);
        let outcome = f(&mut self.agent_meta.design, &signature);
        self.sync_design_after_agent();
        self.review.touch();
        self.review.status = Some(match outcome {
            Ok(message) => message,
            Err(e) => ducad_i18n::t!("review-status-failed", error = e.as_str()),
        });
    }

    /// Nama lembar yang sedang disunting di editor lembar.
    fn review_sheet_name(&self) -> String {
        self.drawing_spec
            .as_ref()
            .map(|s| s.name.clone())
            .unwrap_or_else(|| "sheet1".to_string())
    }

    /// Redline terbuka maupun selesai dari dokumen → lembar yang sedang
    /// tampil dan `drawing_spec` GUI. Dipanggil setelah tool redline
    /// (dari panel ini maupun dari agent) mengubah dokumen.
    pub(crate) fn pull_redlines_from_design(&mut self) {
        let name = self.review_sheet_name();
        let stored: Vec<Annotation> = self
            .agent_meta
            .design
            .drawing(&name)
            .map(redline_annotations)
            .unwrap_or_default();
        // Lembar belum pernah dibuka di GUI: pakai lembar tersimpan dokumen
        // (dibuat tool redline) supaya catatannya tampil saat dibuka.
        if self.drawing_spec.is_none() {
            self.drawing_spec = self.agent_meta.design.drawing(&name).cloned();
        }
        if let Some(spec) = &mut self.drawing_spec {
            let layout = spec.layout.get_or_insert_with(Default::default);
            layout
                .annotations
                .retain(|a| !matches!(a, Annotation::Redline { .. }));
            layout.annotations.extend(stored.iter().cloned());
        }
        if let Some(sheet) = &mut self.drawing_sheet_doc {
            sheet
                .annotations
                .retain(|a| !matches!(a, Annotation::Redline { .. }));
            sheet.annotations.extend(stored);
        }
        self.review.touch();
    }

    /// Redline lembar aktif, untuk digabung ke anotasi lembar saat dibangun.
    pub(crate) fn sheet_redlines(&self) -> Vec<Annotation> {
        self.drawing_spec
            .as_ref()
            .map(redline_annotations)
            .unwrap_or_default()
    }

    /// Simpan lembar yang sedang disunting ke dokumen supaya tool redline
    /// bekerja pada tata letak terkini (bukan lembar bawaan baru).
    fn review_store_sheet(&mut self) {
        self.capture_drawing_spec();
        if self.drawing_spec.is_none() {
            return;
        }
        self.sync_agent_meta();
        // Redline di dokumen adalah sumber kebenaran: tata letak hasil
        // tangkapan tidak boleh menimpa redline yang baru ditutup agent.
        let name = self.review_sheet_name();
        let stored: Option<Vec<Annotation>> = self
            .agent_meta
            .design
            .drawing(&name)
            .map(redline_annotations);
        if let Some(spec) = &mut self.drawing_spec {
            if let Some(stored) = stored {
                let layout = spec.layout.get_or_insert_with(Default::default);
                layout
                    .annotations
                    .retain(|a| !matches!(a, Annotation::Redline { .. }));
                layout.annotations.extend(stored);
            }
            self.agent_meta.design.upsert_drawing(spec.clone());
        }
        self.sync_design_after_agent();
    }

    fn review_tool(
        &mut self,
        tool: &str,
        args: serde_json::Value,
        ok: impl FnOnce(&serde_json::Value) -> String,
    ) {
        let out = self.agent_call_local(tool, args);
        self.review.touch();
        self.review.status = Some(match out {
            Some(out) if !out.is_error => ok(&out.payload),
            Some(out) => {
                let message = out.payload["error"]["message"]
                    .as_str()
                    .unwrap_or("?")
                    .to_string();
                ducad_i18n::t!("review-status-failed", error = message.as_str())
            }
            None => ducad_i18n::t!("review-status-failed", error = "no reply"),
        });
    }

    /// Buang cache data panel (dipanggil setelah agent mengubah dokumen).
    pub(crate) fn review_touch(&mut self) {
        self.review.touch();
    }

    /// Klik alat Redline di lembar: simpan titiknya dan minta teks di panel.
    pub(crate) fn review_begin_redline(&mut self, position: [f32; 2]) {
        self.review.pending_redline = Some([f64::from(position[0]), f64::from(position[1])]);
        self.review.panel_open = true;
        self.review.status = None;
    }

    pub fn handle_review_event(&mut self, event: ReviewPanelEvent) {
        match event {
            ReviewPanelEvent::Close => self.review.panel_open = false,
            ReviewPanelEvent::SaveBrief { title, function } => {
                let mut brief = self.review_design().brief.unwrap_or_default();
                brief.title = title;
                brief.function = function;
                self.review_tool("set_brief", json!({ "brief": brief }), |_| {
                    ducad_i18n::t!("review-status-brief-saved")
                });
            }
            ReviewPanelEvent::AddRequirement { text } => {
                let mut brief = self.review_design().brief.unwrap_or_default();
                let id = (1..)
                    .map(|n| format!("R{n}"))
                    .find(|id| !brief.requirements.iter().any(|r| &r.id == id))
                    .unwrap_or_else(|| "R1".to_string());
                brief.requirements.push(ducad_engine::brief::Requirement {
                    id: id.clone(),
                    text,
                    source: Some("user".to_string()),
                });
                self.review_tool("set_brief", json!({ "brief": brief }), move |_| {
                    ducad_i18n::t!("review-status-requirement-added", id = id.as_str())
                });
            }
            ReviewPanelEvent::Submit { description, by } => {
                self.review_tool(
                    "submit_for_review",
                    json!({ "description": description, "by": by }),
                    |p| {
                        let rev = p["revision"].as_str().unwrap_or("?").to_string();
                        ducad_i18n::t!("review-status-submitted", rev = rev.as_str())
                    },
                );
            }
            ReviewPanelEvent::Approve { by } => {
                // Penghalang dihitung dari hasil check GUI terkini.
                let results = self.checks.results.clone();
                self.review_mutate(|design, signature| {
                    let review = review_checklist(design, &results);
                    let blockers: Vec<String> = review_blockers(&review)
                        .into_iter()
                        .filter(|b| !(b.starts_with("checks") && results.is_empty()))
                        .collect();
                    if !blockers.is_empty() {
                        return Err(blockers.join("; "));
                    }
                    let mut release = design.release.clone().unwrap_or_default();
                    release
                        .approve(signature, &by, &today())
                        .map_err(|e| e.message)?;
                    let rev = release.revision.clone();
                    design.release = Some(release);
                    Ok(ducad_i18n::t!("review-status-approved", rev = rev.as_str()))
                });
            }
            ReviewPanelEvent::Reopen => {
                self.review_mutate(|design, _| {
                    let mut release = design.release.clone().unwrap_or_default();
                    release.reopen();
                    design.release = Some(release);
                    Ok(ducad_i18n::t!("review-status-reopened"))
                });
            }
            ReviewPanelEvent::BuildPackage => self.review_build_package(),
            ReviewPanelEvent::StartRedline => {
                self.open_drawing_sheet();
                self.drawing_sheet_state.redline_tool_active = true;
                self.drawing_sheet_state.text_tool_active = false;
                self.review.status = Some(ducad_i18n::t!("review-redline-pick"));
            }
            ReviewPanelEvent::CancelRedline => self.review.pending_redline = None,
            ReviewPanelEvent::AddRedline { text } => {
                let Some(position) = self.review.pending_redline.take() else {
                    return;
                };
                self.review_store_sheet();
                let sheet = self.review_sheet_name();
                self.review_tool(
                    "add_redline",
                    json!({ "sheet": sheet, "text": text, "position": position }),
                    |p| {
                        let id = p["added"]["id"].as_str().unwrap_or("?").to_string();
                        ducad_i18n::t!("review-redline-placed", id = id.as_str())
                    },
                );
                self.pull_redlines_from_design();
            }
            ReviewPanelEvent::ResolveRedline {
                sheet,
                id,
                resolution,
            } => {
                self.review_store_sheet();
                let shown = id.clone();
                self.review_tool(
                    "resolve_redline",
                    json!({ "sheet": sheet, "id": id, "resolution": resolution }),
                    move |_| ducad_i18n::t!("review-redline-resolved", id = shown.as_str()),
                );
                self.pull_redlines_from_design();
            }
        }
    }

    /// Bangun paket desain multi-lembar ke PDF pilihan pengguna.
    fn review_build_package(&mut self) {
        let design = self.review_design();
        let stem = self
            .current_file_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("part")
            .to_string();
        let title = design
            .brief
            .as_ref()
            .map(|b| b.title.clone())
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| stem.clone());
        let filter = ducad_i18n::t!("file-pdf-filter");
        let Some(path) = self.pick_save_path(&filter, &["pdf"], &format!("{stem}-package.pdf"))
        else {
            return;
        };
        self.review_store_sheet();
        self.sync_agent_meta();
        let firm_path = ducad_engine::firm::find_firm_profile(
            None,
            self.current_file_path.as_ref().and_then(|p| p.parent()),
        );
        let firm = firm_path
            .as_ref()
            .and_then(|p| ducad_engine::firm::FirmProfile::load(p).ok())
            .unwrap_or_default();
        let summary = ducad_engine::check::CheckSummary::from_results(self.checks.results.clone());
        let info = ducad_engine::package::PackageInfo {
            title,
            part_number: stem,
            author: String::new(),
            date: today(),
        };
        let spec = self.agent_meta.design.package.clone().unwrap_or_default();
        let built = ducad_engine::package::build_package(
            &self.model,
            &self.agent_meta,
            &spec,
            &info,
            &firm,
            Some(&summary),
        )
        .map_err(|e| e.message)
        .and_then(|out| {
            std::fs::write(&path, out.pdf_bytes()).map_err(|e| e.to_string())?;
            Ok(out.pages.len())
        });
        self.review.status = Some(match built {
            Ok(sheets) => {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("package.pdf")
                    .to_string();
                let sheets = sheets.to_string();
                ducad_i18n::t!(
                    "review-status-package-written",
                    name = name.as_str(),
                    sheets = sheets.as_str()
                )
            }
            Err(e) => ducad_i18n::t!("review-status-failed", error = e.as_str()),
        });
    }

    /// Setelah berkas dibuka: bawa bagian non-op dokumen (params, checks,
    /// lembar, brief, status rilis, susunan paket) ke `agent_meta`, supaya
    /// tool agent dan panel ini tidak menimpanya dengan keadaan dokumen
    /// sebelumnya. Oplog sengaja TIDAK dibawa: geometri GUI diadopsi lewat
    /// `sync_agent_meta` seperti biasa.
    pub(crate) fn adopt_loaded_design(&mut self) {
        let loaded = self.review_design();
        self.agent_meta = ducad_engine::SessionMeta {
            design: DesignDoc {
                params: loaded.params,
                checks: loaded.checks,
                drawings: loaded.drawings,
                brief: loaded.brief,
                release: loaded.release,
                package: loaded.package,
                ..DesignDoc::default()
            },
            ..Default::default()
        };
        self.review = ReviewState {
            panel_open: self.review.panel_open,
            ..ReviewState::default()
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_plate() -> DuCADApp {
        let mut app = DuCADApp::new_for_test();
        let out = app
            .agent_call_local(
                "run_ops",
                json!({ "ops": [{ "op": "primitive", "id": "plate",
                                  "shape": { "box": { "size": [60, 40, 8] } } }] }),
            )
            .unwrap();
        assert!(!out.is_error, "{}", out.payload);
        app
    }

    fn refresh(app: &mut DuCADApp) -> ReviewPanelData {
        app.refresh_checks(&egui::Context::default());
        app.review_panel_data()
    }

    #[test]
    fn brief_requirements_and_release_flow_through_the_panel() {
        let mut app = app_with_plate();
        app.handle_review_event(ReviewPanelEvent::SaveBrief {
            title: "Plat dudukan".into(),
            function: "Menahan motor".into(),
        });
        app.handle_review_event(ReviewPanelEvent::AddRequirement {
            text: "Tebal 8 mm".into(),
        });
        let out = app
            .agent_call_local(
                "set_checks",
                json!({ "checks": [{ "id": "tebal", "requirement": "R1", "check": "bbox_size",
                                     "body": "plate", "expect": [60, 40, 8] }] }),
            )
            .unwrap();
        assert!(!out.is_error, "{}", out.payload);
        let data = refresh(&mut app);
        assert!(data.has_brief);
        assert_eq!(data.title, "Plat dudukan");
        assert_eq!(data.requirements.len(), 1);
        assert_eq!(data.requirements[0].status, ReviewTrace::Verified);
        assert_eq!(data.stage, ReviewStage::Draft);
        assert!(data.blockers.is_empty(), "{:?}", data.blockers);

        app.handle_review_event(ReviewPanelEvent::Submit {
            description: "Rilis awal".into(),
            by: "ana".into(),
        });
        assert_eq!(refresh(&mut app).stage, ReviewStage::InReview);
        app.handle_review_event(ReviewPanelEvent::Approve { by: "budi".into() });
        let data = refresh(&mut app);
        assert_eq!(data.stage, ReviewStage::Approved, "{:?}", data.status);
        assert!(data.approved_by.as_deref().unwrap().starts_with("budi"));
        assert_eq!(data.history.len(), 1);

        // Mengubah desain membatalkan persetujuan; pengajuan berikutnya revisi B.
        let out = app
            .agent_call_local(
                "run_ops",
                json!({ "ops": [{ "op": "primitive", "id": "boss",
                                  "shape": { "cylinder": { "r": 5, "h": 20 } } }] }),
            )
            .unwrap();
        assert!(!out.is_error, "{}", out.payload);
        let data = refresh(&mut app);
        assert_eq!(data.stage, ReviewStage::Draft);
        assert!(data.approval_stale);
        app.handle_review_event(ReviewPanelEvent::Submit {
            description: "Tambah boss".into(),
            by: "ana".into(),
        });
        let data = refresh(&mut app);
        assert_eq!(data.revision, "B");
        assert_eq!(data.history.len(), 2);
        // Semuanya ikut tersimpan di `design` (yang ditulis ke .ducad).
        let saved: DesignDoc = serde_json::from_value(app.design.clone().unwrap()).unwrap();
        assert_eq!(saved.brief.unwrap().requirements[0].id, "R1");
        assert_eq!(saved.release.unwrap().revision, "B");
    }

    #[test]
    fn failing_check_and_open_redline_block_approval() {
        let mut app = app_with_plate();
        app.agent_call_local(
            "set_checks",
            json!({ "checks": [{ "id": "tipis", "check": "bbox_max", "body": "plate", "max": [60, 40, 5] }] }),
        )
        .unwrap();
        let data = refresh(&mut app);
        assert_eq!(data.blockers.len(), 1);
        app.handle_review_event(ReviewPanelEvent::Approve { by: "budi".into() });
        assert_eq!(refresh(&mut app).stage, ReviewStage::Draft);

        app.agent_call_local("set_checks", json!({ "checks": [] }))
            .unwrap();
        app.review_begin_redline([40.0, 120.0]);
        assert!(app.review.panel_open && refresh(&mut app).pending_redline);
        app.handle_review_event(ReviewPanelEvent::AddRedline {
            text: "Tebalkan jadi 10".into(),
        });
        let data = refresh(&mut app);
        assert!(!data.pending_redline);
        assert_eq!(data.redlines.len(), 1, "{:?}", data.status);
        assert_eq!(data.redlines[0].id, "RL1");
        assert!(!data.blockers.is_empty());
        // Redline ikut digambar di lembar.
        let sheet = app.build_annotated_drawing_sheet();
        assert!(sheet
            .annotations
            .iter()
            .any(|a| matches!(a, Annotation::Redline { redline, .. } if redline.id == "RL1")));

        app.handle_review_event(ReviewPanelEvent::ResolveRedline {
            sheet: data.redlines[0].sheet.clone(),
            id: "RL1".into(),
            resolution: "t = 10".into(),
        });
        let data = refresh(&mut app);
        assert!(data.redlines.is_empty(), "{:?}", data.status);
        assert!(data.blockers.is_empty());
        app.handle_review_event(ReviewPanelEvent::Approve { by: "budi".into() });
        assert_eq!(refresh(&mut app).stage, ReviewStage::Approved);
    }

    #[test]
    fn loaded_design_keeps_brief_release_and_checks_through_agent_sync() {
        let mut app = app_with_plate();
        // Dokumen "dibuka": design memuat brief + release + checks.
        let design = json!({
            "schema": 1,
            "checks": [{ "id": "satu", "check": "body_count", "expect": 1 }],
            "brief": { "title": "Dari berkas", "requirements": [{ "id": "R1", "text": "x" }] },
            "release": { "state": "in_review", "revision": "C" }
        });
        app.design = Some(design);
        app.adopt_loaded_design();
        // Tool agent apa pun menyinkronkan agent_meta → design ditulis ulang.
        let out = app.agent_call_local("get_brief", json!({})).unwrap();
        assert_eq!(
            out.payload["brief"]["title"], "Dari berkas",
            "{}",
            out.payload
        );
        app.sync_agent_meta();
        app.sync_design_after_agent();
        let saved: DesignDoc = serde_json::from_value(app.design.clone().unwrap()).unwrap();
        assert_eq!(saved.checks.len(), 1);
        assert_eq!(saved.brief.unwrap().title, "Dari berkas");
        assert_eq!(saved.release.unwrap().revision, "C");
    }
}
