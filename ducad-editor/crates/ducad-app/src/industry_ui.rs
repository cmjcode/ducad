//! Panel "Fitur Industri" di GUI (P18–P20): menerjemahkan event panel
//! menjadi op di oplog agent (jalur `run_ops` yang sama dengan panel
//! Simulasi), replay konfigurasi, dan perubahan pohon perakitan.
//!
//! Studi lanjutan (frekuensi/buckling/termal) berjalan di thread latar;
//! hasilnya masuk `agent_meta` sehingga check `min_natural_frequency` dkk.
//! ikut hidup.

use std::collections::BTreeMap;
use std::sync::mpsc;

use ducad_core::drawing_annot::Annotation;
use ducad_core::{AxisLine, CouplingKind, ExplodeStep};
use ducad_engine::ops::StudyKind;
use ducad_engine::session::DesignDoc;
use ducad_engine::sim::{AnalysisReport, StudyDef, StudyOutcome};
use ducad_sim::{CancelToken, SimError};
use ducad_ui::{
    AdvStudyDraft, CouplingKindUi, IndConfigRow, IndSheetRow, IndStudyRow, IndustryData,
    IndustryEvent, IndustryPanel, SheetEdgesUi, SheetFeatureUi, StackLinkUi, ThermalBcUi,
};
use serde_json::{json, Value};

use crate::app::DuCADApp;

struct IndustryJob {
    id: String,
    signature: u64,
    cancel: CancelToken,
    rx: mpsc::Receiver<Result<StudyOutcome, SimError>>,
}

/// Keadaan panel Fitur Industri milik `DuCADApp`.
#[derive(Default)]
pub struct IndustryState {
    pub panel_open: bool,
    pub panel: IndustryPanel,
    job: Option<IndustryJob>,
    failures: BTreeMap<String, String>,
    /// Anotasi GD&T lembar gambar (hidup selama sesi).
    pub annotations: Vec<Annotation>,
}

/// JSON op `study` dari draf panel.
pub fn study_op_json(body: &str, draft: &AdvStudyDraft) -> Value {
    let fixtures: Vec<Value> = if draft.kind.uses_fixtures() {
        draft
            .fixtures
            .iter()
            .enumerate()
            .map(|(i, f)| json!({ "id": format!("fix{}", i + 1), "faces": f, "kind": "fixed" }))
            .collect()
    } else {
        Vec::new()
    };
    let loads: Vec<Value> = match (&draft.load, draft.kind) {
        (Some((faces, newton)), ducad_ui::AdvStudyKind::Buckling)
        | (Some((faces, newton)), ducad_ui::AdvStudyKind::ThermalStress) => {
            vec![json!({ "id": "load1", "faces": faces, "kind": "force", "newton": newton })]
        }
        _ => Vec::new(),
    };
    let mut mesh = json!({ "kind": if draft.tet { "tet" } else { "hex" } });
    if draft.cell_mm > 0.0 {
        mesh["cell_mm"] = json!(draft.cell_mm);
    }
    let mut op = json!({
        "op": "study",
        "id": draft.id,
        "kind": draft.kind.key(),
        "setup": { "body": body, "fixtures": fixtures, "loads": loads, "mesh": mesh },
    });
    if draft.kind.uses_modes() {
        op["modes"] = json!(draft.modes.clamp(1, 40));
    }
    if draft.kind.uses_thermal() {
        let boundary: Vec<Value> = draft
            .thermal
            .iter()
            .enumerate()
            .map(|(i, bc)| {
                let id = format!("bc{}", i + 1);
                match bc.kind {
                    ThermalBcUi::Temperature => {
                        json!({ "id": id, "faces": bc.faces, "kind": "temperature", "celsius": bc.value })
                    }
                    ThermalBcUi::HeatFlux => {
                        json!({ "id": id, "faces": bc.faces, "kind": "heat_flux", "w_per_mm2": bc.value })
                    }
                    // Panel memakai W/(m²·K); op memakai W/(mm²·K).
                    ThermalBcUi::Convection => json!({
                        "id": id, "faces": bc.faces, "kind": "convection",
                        "h_w_mm2k": bc.value * 1e-6, "ambient_c": bc.ambient,
                    }),
                }
            })
            .collect();
        op["thermal"] = json!({ "boundary": boundary });
    }
    op
}

/// JSON check `tolerance_stackup` dari rantai panel.
pub fn stack_check_json(id: &str, chain: &[StackLinkUi], max_total: f64, rss: bool) -> Value {
    let links: Vec<Value> = chain
        .iter()
        .map(|l| {
            let mut link = json!({ "nominal": l.nominal, "reverse": l.reverse });
            if l.fit.trim().is_empty() {
                link["plus"] = json!(l.plus);
                link["minus"] = json!(l.minus);
            } else {
                link["fit"] = json!(l.fit.trim());
            }
            link
        })
        .collect();
    json!({
        "id": id,
        "check": "tolerance_stackup",
        "chain": links,
        "max_total": max_total,
        "method": if rss { "rss" } else { "worst_case" },
    })
}

fn result_lines(outcome: &StudyOutcome) -> Vec<String> {
    let mesh_line = |kind: ducad_sim::MeshKind, elements: usize| {
        let (kind, elements) = (format!("{kind:?}").to_lowercase(), elements.to_string());
        ducad_i18n::t!(
            "ind-result-mesh",
            kind = kind.as_str(),
            elements = elements.as_str()
        )
    };
    match outcome {
        StudyOutcome::Stress(r) => {
            let (stress, disp, sf) = (
                format!("{:.3}", r.max_von_mises_mpa),
                format!("{:.5}", r.max_displacement_mm),
                format!("{:.2}", r.safety_factor),
            );
            vec![
                ducad_i18n::t!(
                    "ind-result-stress",
                    stress = stress.as_str(),
                    disp = disp.as_str(),
                    sf = sf.as_str()
                ),
                mesh_line(r.mesh_stats.kind, r.mesh_stats.elements),
            ]
        }
        StudyOutcome::Analysis(a) => match a.as_ref() {
            AnalysisReport::Frequency(r) => {
                let mut lines: Vec<String> = r
                    .frequencies_hz
                    .iter()
                    .enumerate()
                    .map(|(i, hz)| {
                        let (n, hz) = ((i + 1).to_string(), format!("{hz:.2}"));
                        ducad_i18n::t!("ind-result-frequency", n = n.as_str(), hz = hz.as_str())
                    })
                    .collect();
                lines.push(mesh_line(r.mesh_stats.kind, r.mesh_stats.elements));
                lines
            }
            AnalysisReport::Buckling(r) => {
                let mut lines: Vec<String> = r
                    .load_factors
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        let (n, factor) = ((i + 1).to_string(), format!("{f:.3}"));
                        ducad_i18n::t!(
                            "ind-result-buckling",
                            n = n.as_str(),
                            factor = factor.as_str()
                        )
                    })
                    .collect();
                if lines.is_empty() {
                    lines.push(ducad_i18n::t!("ind-result-no-buckling"));
                }
                lines.push(mesh_line(r.mesh_stats.kind, r.mesh_stats.elements));
                lines
            }
            AnalysisReport::Thermal(r) => {
                let (max, min) = (
                    format!("{:.2}", r.max_temperature_c),
                    format!("{:.2}", r.min_temperature_c),
                );
                vec![
                    ducad_i18n::t!("ind-result-thermal", max = max.as_str(), min = min.as_str()),
                    mesh_line(r.mesh_stats.kind, r.mesh_stats.elements),
                ]
            }
        },
    }
}

fn annotation_summary(a: &Annotation) -> String {
    let p = a.position();
    let detail = match a {
        Annotation::FeatureControlFrame { frame, .. } => {
            format!(
                "{} {} {}",
                frame.symbol.name(),
                frame.value,
                frame.datums.join("|")
            )
        }
        Annotation::DatumFeature { datum, .. } => datum.label.clone(),
        Annotation::DimensionTolerance { dimension, .. } => format!("{}", dimension.nominal),
        Annotation::SurfaceFinish { finish, .. } => format!("Ra {}", finish.ra_um),
        _ => String::new(),
    };
    format!("{} {detail} @ [{:.0}, {:.0}]", a.kind_name(), p[0], p[1])
}

impl DuCADApp {
    /// Studi non-statik di oplog.
    fn ind_studies(&self) -> Vec<(String, StudyDef)> {
        ducad_engine::sim::study_defs(&self.agent_meta)
            .into_iter()
            .filter(|(_, d)| d.kind != StudyKind::Static)
            .map(|(id, d)| (id.to_string(), d))
            .collect()
    }

    fn ind_cached(&self, id: &str, def: &StudyDef) -> Option<(u64, StudyOutcome)> {
        if def.kind.has_stress() {
            let c = self.agent_meta.sim_results.get(id)?;
            Some((c.signature, StudyOutcome::Stress(c.report.clone())))
        } else {
            let c = self.agent_meta.analysis_results.get(id)?;
            Some((c.signature, StudyOutcome::Analysis(c.report.clone())))
        }
    }

    pub fn industry_panel_data(&mut self) -> IndustryData {
        let target = self.sim_target();
        let target_body = target.and_then(|id| self.model.doc.bodies.get(id));
        let target_name = target_body.map(|b| b.name.clone());
        let target_has_material =
            target_body.is_some_and(|b| self.model.doc.mechanical_of(b).is_some());
        let picked_face = self.sim_picked_selector();
        let picked_point = self.active_face.as_ref().map(|(_, _, hit)| {
            let c = hit.centroid;
            [c.0, c.1, c.2]
        });

        let studies = self
            .ind_studies()
            .into_iter()
            .map(|(id, def)| {
                let cached = self.ind_cached(&id, &def);
                let signature = ducad_engine::sim::def_signature(&self.model, &def);
                IndStudyRow {
                    running: self.industry.job.as_ref().is_some_and(|j| j.id == id),
                    stale: cached
                        .as_ref()
                        .is_some_and(|(sig, _)| Some(*sig) != signature),
                    lines: cached.map(|(_, o)| result_lines(&o)).unwrap_or_default(),
                    error: self.industry.failures.get(&id).cloned(),
                    kind: def.kind.name().to_string(),
                    body: def.setup.body.clone(),
                    id,
                }
            })
            .collect();

        let design = &self.agent_meta.design;
        let active = design.active_configuration.clone();
        let mut configs = vec![IndConfigRow {
            name: ducad_core::DEFAULT_CONFIGURATION.to_string(),
            active: active.is_none(),
            ..IndConfigRow::default()
        }];
        configs.extend(design.configurations.iter().map(|c| IndConfigRow {
            name: c.name.clone(),
            active: active.as_deref() == Some(c.name.as_str()),
            params: c.params.iter().map(|(k, v)| (k.clone(), *v)).collect(),
            suppressed: c.suppressed_ops.clone(),
        }));

        let sheets = self
            .agent_meta
            .sheet_metal
            .iter()
            .filter(|(name, _)| self.model.doc.bodies.values().any(|b| &b.name == *name))
            .map(|(name, state)| IndSheetRow {
                name: name.clone(),
                thickness: state.model.thickness,
                flanges: state.model.flanges.len(),
                unfolded: state.unfolded,
            })
            .collect();

        let mut instances: Vec<(u64, String)> = self
            .assembly_tree
            .instances
            .values()
            .map(|i| (i.id as u64, i.name.clone()))
            .collect();
        instances.sort();
        let name_of = |id: u32| {
            self.assembly_tree
                .instances
                .get(&id)
                .map(|i| i.name.clone())
                .unwrap_or_else(|| format!("#{id}"))
        };
        let couplings = self
            .assembly_tree
            .couplings
            .iter()
            .map(|c| {
                let kind = match c.kind {
                    CouplingKind::Gear { ratio } => format!("gear {ratio}"),
                    CouplingKind::Screw { pitch } => format!("screw {pitch} mm"),
                    CouplingKind::RackPinion { pitch_radius } => format!("rack r={pitch_radius}"),
                };
                format!(
                    "{}: {} -> {} ({kind})",
                    c.name,
                    name_of(c.driver),
                    name_of(c.driven)
                )
            })
            .collect();
        let explode_steps = self
            .assembly_tree
            .explode_steps
            .iter()
            .map(|s| {
                let t = s.translation;
                format!(
                    "{} [{:.1}, {:.1}, {:.1}]",
                    name_of(s.instance),
                    t.0,
                    t.1,
                    t.2
                )
            })
            .collect();
        let threads = self
            .agent_meta
            .threads
            .iter()
            .flat_map(|(body, notes)| {
                notes
                    .iter()
                    .map(move |n| format!("{body}: {} x {:.1} mm", n.designation, n.length))
            })
            .collect();

        IndustryData {
            target_body: target_name,
            target_has_material,
            picked_face,
            picked_point,
            studies,
            configs,
            base_params: design.params.iter().map(|(k, v)| (k.clone(), *v)).collect(),
            op_ids: design.oplog.iter().map(|o| o.id().to_string()).collect(),
            sheets,
            annotations: self
                .industry
                .annotations
                .iter()
                .map(annotation_summary)
                .collect(),
            instances,
            couplings,
            explode_steps,
            explode_factor: self.assembly_tree.explode_factor,
            threads,
        }
    }

    /// Jalankan batch op; `ok` menjadi status bila berhasil.
    fn ind_run_ops(&mut self, ops: Vec<Value>, ok: String) -> bool {
        match self.sim_core_tool("run_ops", json!({ "ops": ops })) {
            Ok(()) => {
                self.after_model_changed(&ducad_i18n::t!("ind-title"), &ok);
                self.model_status = Some(ok);
                true
            }
            Err(why) => {
                self.ind_fail(why);
                false
            }
        }
    }

    /// Replay design yang diedit (konfigurasi) dan adopsi sebagai satu
    /// langkah undo; gagal → model utuh.
    fn ind_adopt_design(&mut self, design: DesignDoc, ok: String) {
        let replayed = ducad_engine::Session::replay(DesignDoc {
            fingerprint: String::new(),
            ..design
        });
        match replayed {
            Ok(session) => {
                let meta = session.meta().clone();
                self.adopt_agent_model(session.into_model(), meta, "configuration");
                self.model_status = Some(ok);
            }
            Err(e) => self.ind_fail(e.message),
        }
    }

    fn ind_fail(&mut self, why: impl AsRef<str>) {
        self.model_status = Some(ducad_i18n::t!("ind-failed", why = why.as_ref()));
    }

    fn ind_create_study(&mut self, draft: AdvStudyDraft) {
        let Some(body) = self
            .sim_target()
            .and_then(|id| self.model.doc.bodies.get(id))
            .map(|b| b.name.clone())
        else {
            return;
        };
        let op = study_op_json(&body, &draft);
        match self.sim_core_tool("run_ops", json!({ "ops": [op] })) {
            Ok(()) => self.model_status = Some(ducad_i18n::t!("ind-study-created")),
            Err(why) => self.ind_fail(why),
        }
    }

    /// Mulai studi di thread latar; selector dan material diselesaikan di
    /// sini (butuh kernel), thread hanya menjalankan solver.
    fn ind_run_study(&mut self, id: String, ctx: &egui::Context) {
        if self.industry.job.is_some() {
            return;
        }
        self.sync_agent_meta();
        self.industry.failures.remove(&id);
        let def = match ducad_engine::sim::def_of(&self.agent_meta, &id) {
            Ok(d) => d,
            Err(e) => {
                self.industry.failures.insert(id, e.message);
                return;
            }
        };
        let prepared = ducad_engine::sim::prepare_def(&self.model, &self.agent_meta, &def);
        let signature = ducad_engine::sim::def_signature(&self.model, &def);
        let (prepared, signature) = match (prepared, signature) {
            (Ok(p), Some(sig)) => (p, sig),
            (Err(e), _) => {
                self.industry.failures.insert(id, e.message);
                return;
            }
            (_, None) => return,
        };
        let cancel = CancelToken::new();
        let (tx, rx) = mpsc::channel();
        let (thread_cancel, repaint) = (cancel.clone(), ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(prepared.run(&thread_cancel));
            repaint.request_repaint();
        });
        self.industry.job = Some(IndustryJob {
            id,
            signature,
            cancel,
            rx,
        });
    }

    /// Dipanggil tiap frame: ambil hasil studi latar bila sudah selesai.
    pub fn refresh_industry(&mut self) {
        let Some(job) = &self.industry.job else {
            return;
        };
        let result = match job.rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Err(SimError::Cancelled),
        };
        let Some(job) = self.industry.job.take() else {
            return;
        };
        match result {
            Ok(outcome) => ducad_engine::sim::store_outcome(
                &mut self.agent_meta,
                &job.id,
                job.signature,
                &outcome,
            ),
            Err(SimError::Cancelled) => {
                self.model_status = Some(ducad_i18n::t!("ind-study-cancelled"));
            }
            Err(e) => {
                let why = match e.hint() {
                    Some(hint) => format!("{}: {e} ({hint})", e.code()),
                    None => format!("{}: {e}", e.code()),
                };
                self.industry.failures.insert(job.id, why);
            }
        }
    }

    fn ind_remove_op(&mut self, id: &str) {
        self.sync_agent_meta();
        match self.agent_edit_oplog("remove_op", json!({ "ids": [id] })) {
            Ok(out) if !out.is_error => {
                self.industry.failures.remove(id);
            }
            Ok(out) => {
                let why = out.payload["error"]["message"]
                    .as_str()
                    .unwrap_or("remove_op")
                    .to_string();
                self.ind_fail(why);
            }
            Err(e) => self.ind_fail(e.message),
        }
    }

    /// Selector tepi untuk sisi pelat dasar sejajar X/Y (pelat di bidang XY).
    fn ind_sheet_edges(&self, body: &str, edges: &SheetEdgesUi) -> Result<String, String> {
        let along_x = match edges {
            SheetEdgesUi::Custom(s) => return Ok(s.trim().to_string()),
            SheetEdgesUi::AlongX => true,
            SheetEdgesUi::AlongY => false,
        };
        let state = self
            .agent_meta
            .sheet_metal
            .get(body)
            .ok_or_else(|| format!("'{body}'"))?;
        let f = &state.frame;
        let near = |a: [f64; 3], b: [f64; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-6);
        if !(near(f.u_axis, [1.0, 0.0, 0.0]) && near(f.v_axis, [0.0, 1.0, 0.0])) {
            return Err(ducad_i18n::t!("ind-sheet-need-xy"));
        }
        let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
        for p in &state.model.outline {
            for k in 0..2 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let len = if along_x {
            hi[0] - lo[0]
        } else {
            hi[1] - lo[1]
        };
        let axis = if along_x { "X" } else { "Y" };
        Ok(format!("|{axis}[len={len}][z={}]", f.origin[2]))
    }

    fn ind_design_edit(&mut self, edit: impl FnOnce(&mut DesignDoc), ok: String) {
        self.sync_agent_meta();
        let mut design = self.agent_meta.design.clone();
        edit(&mut design);
        self.ind_adopt_design(design, ok);
    }

    /// Id op yang belum dipakai, berawalan `base`.
    fn ind_free_id(&self, base: &str) -> String {
        let used = |id: &str| self.agent_meta.design.oplog.iter().any(|o| o.id() == id);
        let base: String = base.chars().take(26).collect();
        if !used(&base) {
            return base;
        }
        (2..1000)
            .map(|n| format!("{base}{n}"))
            .find(|id| !used(id))
            .unwrap_or(base)
    }

    /// Salin anotasi panel ke lembar gambar yang sedang terbuka.
    pub(crate) fn ind_sync_annotations(&mut self) {
        if let Some(sheet) = &mut self.drawing_sheet_doc {
            sheet.annotations = self.industry.annotations.clone();
        }
    }

    fn ind_sheet_simple(&mut self, op: &str, body: String, ok: String) {
        let id = self.ind_free_id(&format!("{body}_{op}"));
        self.ind_run_ops(vec![json!({ "op": op, "id": id, "body": body })], ok);
    }

    pub fn handle_industry_event(&mut self, event: IndustryEvent, ctx: &egui::Context) {
        match event {
            IndustryEvent::Close => self.industry.panel_open = false,
            IndustryEvent::CreateStudy(draft) => self.ind_create_study(draft),
            IndustryEvent::RunStudy(id) => self.ind_run_study(id, ctx),
            IndustryEvent::CancelStudy => {
                if let Some(job) = &self.industry.job {
                    job.cancel.cancel();
                }
            }
            IndustryEvent::DeleteStudy(id) => self.ind_remove_op(&id),
            IndustryEvent::ActivateConfig(name) => {
                let ok = ducad_i18n::t!("ind-cfg-activated", name = name.as_str());
                self.ind_design_edit(
                    |d| {
                        d.active_configuration =
                            (name != ducad_core::DEFAULT_CONFIGURATION).then_some(name);
                    },
                    ok,
                );
            }
            IndustryEvent::SaveConfig {
                name,
                params,
                suppressed,
            } => {
                if !ducad_core::valid_configuration_name(&name) {
                    self.ind_fail(name);
                    return;
                }
                self.ind_design_edit(
                    |d| {
                        let params: BTreeMap<String, f64> = params.into_iter().collect();
                        match d.configurations.iter_mut().find(|c| c.name == name) {
                            Some(c) => {
                                c.params = params;
                                c.suppressed_ops = suppressed;
                            }
                            None => {
                                let mut c = ducad_core::Configuration::named(name.clone());
                                c.params = params;
                                c.suppressed_ops = suppressed;
                                d.configurations.push(c);
                            }
                        }
                        d.active_configuration = Some(name);
                    },
                    ducad_i18n::t!("ind-cfg-saved"),
                );
            }
            IndustryEvent::DeleteConfig(name) => self.ind_design_edit(
                |d| {
                    d.configurations.retain(|c| c.name != name);
                    if d.active_configuration.as_deref() == Some(name.as_str()) {
                        d.active_configuration = None;
                    }
                },
                ducad_i18n::t!("ind-cfg-deleted"),
            ),
            IndustryEvent::ExportConfigCsv => {
                let csv = ducad_core::design_table_to_csv(&self.agent_meta.design.configurations);
                if let Some(path) = self.pick_save_path("CSV", &["csv"], "design-table.csv") {
                    match std::fs::write(&path, csv) {
                        Ok(()) => self.model_status = Some(ducad_i18n::t!("ind-cfg-exported")),
                        Err(e) => self.ind_fail(e.to_string()),
                    }
                }
            }
            IndustryEvent::ImportConfigCsv => {
                let Some(path) = self.pick_open_path("CSV", &["csv"]) else {
                    return;
                };
                let parsed = std::fs::read_to_string(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|text| ducad_core::design_table_from_csv(&text));
                match parsed {
                    Ok(configs) => self.ind_design_edit(
                        |d| {
                            if !configs
                                .iter()
                                .any(|c| Some(c.name.as_str()) == d.active_configuration.as_deref())
                            {
                                d.active_configuration = None;
                            }
                            d.configurations = configs;
                        },
                        ducad_i18n::t!("ind-cfg-imported"),
                    ),
                    Err(why) => self.ind_fail(why),
                }
            }
            IndustryEvent::BaseFlange {
                id,
                width,
                height,
                thickness,
                radius,
                k_factor,
            } => {
                let sketch = format!("{id}_sk");
                let ops = vec![
                    json!({ "op": "sketch", "id": sketch, "plane": "XY", "entities": [
                        { "rect": { "corner": [0, 0], "w": width, "h": height, "name": "outline" } }
                    ] }),
                    json!({ "op": "base_flange", "id": id, "sketch": sketch,
                            "thickness": thickness, "bend_radius": radius, "k_factor": k_factor }),
                ];
                self.ind_run_ops(ops, ducad_i18n::t!("ind-sheet-base"));
            }
            IndustryEvent::SheetFeature {
                id,
                body,
                kind,
                edges,
                length,
                angle,
                extra,
            } => {
                let edges = match self.ind_sheet_edges(&body, &edges) {
                    Ok(e) => e,
                    Err(why) => {
                        self.ind_fail(why);
                        return;
                    }
                };
                let op = match kind {
                    SheetFeatureUi::EdgeFlange => json!({
                        "op": "edge_flange", "id": id, "body": body, "edges": edges,
                        "length": length, "angle": angle }),
                    SheetFeatureUi::Hem => json!({
                        "op": "hem", "id": id, "body": body, "edges": edges,
                        "length": length, "gap": extra }),
                    SheetFeatureUi::Jog => json!({
                        "op": "jog", "id": id, "body": body, "edges": edges,
                        "offset": extra, "length": length, "angle": angle }),
                };
                self.ind_run_ops(vec![op], ducad_i18n::t!("ind-sheet-add-feature"));
            }
            IndustryEvent::Unfold(body) => {
                self.ind_sheet_simple("unfold", body, ducad_i18n::t!("ind-sheet-unfold"))
            }
            IndustryEvent::Fold(body) => {
                self.ind_sheet_simple("fold", body, ducad_i18n::t!("ind-sheet-fold"))
            }
            IndustryEvent::FlatPattern(body) => {
                self.ind_sheet_simple("flat_pattern", body, ducad_i18n::t!("ind-sheet-flat"))
            }
            IndustryEvent::ExportFlatDxf(body) => {
                let pattern = self
                    .agent_meta
                    .sheet_metal
                    .get(&body)
                    .ok_or_else(|| body.clone())
                    .and_then(|s| s.model.flat_pattern());
                match pattern {
                    Ok(p) => {
                        let name = format!("{body}-flat.dxf");
                        if let Some(path) = self.pick_save_path("DXF", &["dxf"], &name) {
                            let dxf = ducad_io::flat_dxf::flat_pattern_dxf(&p);
                            match std::fs::write(&path, dxf) {
                                Ok(()) => {
                                    self.model_status = Some(ducad_i18n::t!("ind-sheet-exported"))
                                }
                                Err(e) => self.ind_fail(e.to_string()),
                            }
                        }
                    }
                    Err(why) => self.ind_fail(why),
                }
            }
            IndustryEvent::AddStackCheck {
                chain,
                max_total,
                rss,
            } => {
                self.sync_agent_meta();
                let n = self.agent_meta.design.checks.len() + 1;
                let value = stack_check_json(&format!("stackup{n}"), &chain, max_total, rss);
                match serde_json::from_value(value) {
                    Ok(item) => {
                        self.agent_meta.design.checks.push(item);
                        self.sync_design_after_agent();
                        self.model_status = Some(ducad_i18n::t!("ind-tol-check-added"));
                    }
                    Err(e) => self.ind_fail(e.to_string()),
                }
            }
            IndustryEvent::AddAnnotation(annotation) => match annotation.validate() {
                Ok(()) => {
                    self.industry.annotations.push(annotation);
                    self.ind_sync_annotations();
                    self.model_status = Some(ducad_i18n::t!("ind-gdt-added"));
                }
                Err(why) => self.ind_fail(why),
            },
            IndustryEvent::RemoveAnnotation(i) => {
                if i < self.industry.annotations.len() {
                    self.industry.annotations.remove(i);
                    self.ind_sync_annotations();
                }
            }
            IndustryEvent::OpenDrawing => self.open_drawing_sheet(),
            IndustryEvent::InsertStandard {
                id,
                standard,
                size,
                length,
                at,
            } => {
                let mut op = json!({ "op": "standard_part", "id": id, "standard": standard,
                                     "size": size, "at": at });
                if let Some(length) = length {
                    op["length"] = json!(length);
                }
                self.ind_run_ops(vec![op], ducad_i18n::t!("ind-parts-inserted"));
            }
            IndustryEvent::AddThread {
                id,
                pitch,
                length,
                cosmetic,
            } => {
                let body = self
                    .sim_target()
                    .and_then(|b| self.model.doc.bodies.get(b))
                    .map(|b| b.name.clone());
                let (Some(body), Some(face)) = (body, self.sim_picked_selector()) else {
                    return;
                };
                let mut op = json!({ "op": "thread", "id": id, "body": body, "face": face,
                                     "cosmetic": cosmetic });
                if pitch > 0.0 {
                    op["pitch"] = json!(pitch);
                }
                if length > 0.0 {
                    op["length"] = json!(length);
                }
                self.ind_run_ops(vec![op], ducad_i18n::t!("ind-thread-added"));
            }
            IndustryEvent::AddCoupling {
                kind,
                value,
                driver,
                driven,
                driver_axis,
                driven_axis,
            } => {
                self.sync_assembly_instances();
                let kind = match kind {
                    CouplingKindUi::Gear => CouplingKind::Gear { ratio: value },
                    CouplingKindUi::Screw => CouplingKind::Screw { pitch: value },
                    CouplingKindUi::RackPinion => CouplingKind::RackPinion {
                        pitch_radius: value,
                    },
                };
                let axis = |instance: u32, axis: usize| {
                    let origin = self
                        .assembly_tree
                        .instances
                        .get(&instance)
                        .map(|i| i.translation)
                        .unwrap_or((0.0, 0.0, 0.0));
                    let dir = [(1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0)][axis.min(2)];
                    AxisLine { origin, dir }
                };
                let (driver, driven) = (driver as u32, driven as u32);
                let (a, b) = (axis(driver, driver_axis), axis(driven, driven_axis));
                let name = format!("coupling{}", self.assembly_tree.couplings.len() + 1);
                let added = self
                    .assembly_tree
                    .add_coupling(name, kind, driver, a, driven, b);
                self.model_status = Some(if added.is_some() {
                    ducad_i18n::t!("ind-asm-coupling-added")
                } else {
                    ducad_i18n::t!("ind-asm-coupling-failed")
                });
            }
            IndustryEvent::RemoveCoupling(i) => {
                if i < self.assembly_tree.couplings.len() {
                    self.assembly_tree.couplings.remove(i);
                }
            }
            IndustryEvent::AddExplodeStep {
                instance,
                translation,
            } => {
                self.sync_assembly_instances();
                let t = translation;
                if self.assembly_tree.add_explode_step(ExplodeStep {
                    instance: instance as u32,
                    translation: (t[0], t[1], t[2]),
                    rotation: None,
                }) && self.assembly_tree.explode_factor <= 0.0
                {
                    self.set_explode_factor(1.0);
                }
            }
            IndustryEvent::RemoveExplodeStep(i) => {
                if i < self.assembly_tree.explode_steps.len() {
                    self.assembly_tree.explode_steps.remove(i);
                }
            }
            IndustryEvent::SetExplode(f) => self.set_explode_factor(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_ui::{AdvStudyKind, ThermalBcDraft};

    #[test]
    fn study_op_json_parses_as_study_op() {
        let draft = AdvStudyDraft {
            id: "heat".into(),
            kind: AdvStudyKind::ThermalStress,
            fixtures: vec!["<X".into()],
            load: Some((">X".into(), [0.0, 0.0, -50.0])),
            thermal: vec![
                ThermalBcDraft {
                    faces: "<X".into(),
                    kind: ThermalBcUi::Temperature,
                    value: 80.0,
                    ambient: 25.0,
                },
                ThermalBcDraft {
                    faces: ">Z".into(),
                    kind: ThermalBcUi::Convection,
                    value: 20.0,
                    ambient: 25.0,
                },
            ],
            modes: 5,
            tet: true,
            cell_mm: 3.0,
        };
        let value = study_op_json("beam", &draft);
        assert!(
            value.get("modes").is_none(),
            "modes hanya untuk frekuensi/buckling"
        );
        let h = value["thermal"]["boundary"][1]["h_w_mm2k"]
            .as_f64()
            .unwrap();
        assert!((h - 20.0e-6).abs() < 1e-12);
        let op: ducad_engine::ops::Op = serde_json::from_value(value).unwrap();
        assert_eq!(op.id(), "heat");

        let freq = AdvStudyDraft {
            id: "modes".into(),
            kind: AdvStudyKind::Frequency,
            fixtures: vec!["<X".into()],
            load: Some((">X".into(), [1.0, 0.0, 0.0])),
            modes: 4,
            ..AdvStudyDraft::default()
        };
        let value = study_op_json("beam", &freq);
        assert_eq!(value["modes"], 4);
        assert!(value.get("thermal").is_none());
        assert_eq!(value["setup"]["loads"].as_array().unwrap().len(), 0);
        assert!(serde_json::from_value::<ducad_engine::ops::Op>(value).is_ok());
    }

    fn app_with_bar() -> DuCADApp {
        let mut app = DuCADApp::new_for_test();
        let shape = ducad_kernel::make_box(10.0, 10.0, 40.0, false).unwrap();
        let id = app.model.doc.add_body("bar");
        app.model
            .geometry
            .insert(id, crate::model::BodyGeometry::from_shape(shape));
        app.model.doc.bodies[id].mechanical =
            Some(ducad_core::MaterialSource::Library("s235".into()));
        app
    }

    fn wait_for_job(app: &mut DuCADApp) {
        for _ in 0..4000 {
            app.refresh_industry();
            if app.industry.job.is_none() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("studi tidak selesai dalam 20 detik");
    }

    fn total_volume(app: &DuCADApp) -> f64 {
        app.model
            .geometry
            .values()
            .map(|g| g.shape.volume().abs())
            .sum()
    }

    #[test]
    fn industry_studies_run_in_background() {
        let mut app = app_with_bar();
        let ctx = egui::Context::default();
        let freq = AdvStudyDraft {
            id: "modes".into(),
            kind: AdvStudyKind::Frequency,
            fixtures: vec!["<Z".into()],
            modes: 2,
            cell_mm: 2.5,
            ..AdvStudyDraft::default()
        };
        app.handle_industry_event(IndustryEvent::CreateStudy(freq), &ctx);
        let heat = AdvStudyDraft {
            id: "heat".into(),
            kind: AdvStudyKind::Thermal,
            thermal: vec![
                ThermalBcDraft {
                    faces: "<Z".into(),
                    kind: ThermalBcUi::Temperature,
                    value: 90.0,
                    ambient: 25.0,
                },
                ThermalBcDraft {
                    faces: ">Z".into(),
                    kind: ThermalBcUi::Temperature,
                    value: 30.0,
                    ambient: 25.0,
                },
            ],
            cell_mm: 2.5,
            ..AdvStudyDraft::default()
        };
        app.handle_industry_event(IndustryEvent::CreateStudy(heat), &ctx);
        let data = app.industry_panel_data();
        assert_eq!(data.studies.len(), 2, "{:?}", app.model_status);
        assert!(data.studies.iter().all(|s| s.lines.is_empty()));
        // Studi non-statik tidak muncul di panel Simulasi.
        assert!(app.sim_panel_data().studies.is_empty());

        for id in ["modes", "heat"] {
            app.handle_industry_event(IndustryEvent::RunStudy(id.into()), &ctx);
            assert!(app
                .industry_panel_data()
                .studies
                .iter()
                .any(|s| s.id == id && s.running));
            wait_for_job(&mut app);
        }
        let data = app.industry_panel_data();
        for row in &data.studies {
            assert!(row.error.is_none(), "{row:?}");
            assert!(!row.lines.is_empty() && !row.stale, "{row:?}");
        }
        assert!(ducad_engine::sim::fresh_analysis(&app.model, &app.agent_meta, "heat").is_ok());

        app.handle_industry_event(IndustryEvent::DeleteStudy("modes".into()), &ctx);
        assert_eq!(app.industry_panel_data().studies.len(), 1);
    }

    #[test]
    fn industry_sheet_metal_and_configurations() {
        let mut app = DuCADApp::new_for_test();
        let ctx = egui::Context::default();
        app.handle_industry_event(
            IndustryEvent::BaseFlange {
                id: "tray".into(),
                width: 100.0,
                height: 60.0,
                thickness: 2.0,
                radius: 2.0,
                k_factor: 0.44,
            },
            &ctx,
        );
        let data = app.industry_panel_data();
        assert_eq!(data.sheets.len(), 1, "{:?}", app.model_status);
        let flat = total_volume(&app);
        assert!((flat - 100.0 * 60.0 * 2.0).abs() < 1.0, "{flat}");

        app.handle_industry_event(
            IndustryEvent::SheetFeature {
                id: "wall_x".into(),
                body: "tray".into(),
                kind: SheetFeatureUi::EdgeFlange,
                edges: SheetEdgesUi::AlongX,
                length: 20.0,
                angle: 90.0,
                extra: 0.0,
            },
            &ctx,
        );
        let data = app.industry_panel_data();
        assert_eq!(data.sheets[0].flanges, 2, "{:?}", app.model_status);
        let folded = total_volume(&app);
        assert!(folded > flat + 1000.0, "{folded} vs {flat}");

        // Konfigurasi yang melewati flange → kembali ke pelat datar.
        app.handle_industry_event(
            IndustryEvent::SaveConfig {
                name: "plain".into(),
                params: Vec::new(),
                suppressed: vec!["wall_x".into()],
            },
            &ctx,
        );
        let data = app.industry_panel_data();
        assert!(
            data.configs.iter().any(|c| c.name == "plain" && c.active),
            "{:?}",
            app.model_status
        );
        assert!((total_volume(&app) - flat).abs() < 1.0);
        app.handle_industry_event(IndustryEvent::ActivateConfig("Default".into()), &ctx);
        assert!((total_volume(&app) - folded).abs() < 1.0);
        app.handle_industry_event(IndustryEvent::DeleteConfig("plain".into()), &ctx);
        assert_eq!(app.industry_panel_data().configs.len(), 1);

        // Pola datar = body baru; bentang/lipat bolak-balik.
        let bodies = app.model.doc.bodies.len();
        app.handle_industry_event(IndustryEvent::FlatPattern("tray".into()), &ctx);
        assert_eq!(
            app.model.doc.bodies.len(),
            bodies + 1,
            "{:?}",
            app.model_status
        );
        app.handle_industry_event(IndustryEvent::Unfold("tray".into()), &ctx);
        assert!(app.industry_panel_data().sheets.iter().any(|s| s.unfolded));
        app.handle_industry_event(IndustryEvent::Fold("tray".into()), &ctx);
        assert!(app.industry_panel_data().sheets.iter().all(|s| !s.unfolded));
    }

    #[test]
    fn industry_parts_tolerances_and_assembly() {
        let mut app = app_with_bar();
        let ctx = egui::Context::default();
        app.handle_industry_event(
            IndustryEvent::InsertStandard {
                id: "bolt1".into(),
                standard: "iso4762".into(),
                size: "M6".into(),
                length: Some(20.0),
                at: [30.0, 0.0, 0.0],
            },
            &ctx,
        );
        assert_eq!(app.model.doc.bodies.len(), 2, "{:?}", app.model_status);

        app.handle_industry_event(
            IndustryEvent::AddStackCheck {
                chain: vec![StackLinkUi::default()],
                max_total: 0.5,
                rss: false,
            },
            &ctx,
        );
        assert_eq!(
            app.agent_meta.design.checks.len(),
            1,
            "{:?}",
            app.model_status
        );

        let datum = |label: &str| Annotation::DatumFeature {
            position: [40.0, 40.0],
            datum: ducad_core::drawing_annot::DatumFeature {
                label: label.into(),
            },
        };
        app.handle_industry_event(IndustryEvent::AddAnnotation(datum("A")), &ctx);
        app.handle_industry_event(IndustryEvent::AddAnnotation(datum("bad label")), &ctx);
        assert_eq!(app.industry.annotations.len(), 1, "label tidak sah ditolak");
        assert_eq!(app.build_annotated_drawing_sheet().annotations.len(), 1);
        app.handle_industry_event(IndustryEvent::RemoveAnnotation(0), &ctx);
        assert!(app.industry.annotations.is_empty());

        app.sync_assembly_instances();
        let data = app.industry_panel_data();
        assert_eq!(data.instances.len(), 2);
        let (a, b) = (data.instances[0].0, data.instances[1].0);
        app.handle_industry_event(
            IndustryEvent::AddCoupling {
                kind: CouplingKindUi::Gear,
                value: -2.0,
                driver: a,
                driven: b,
                driver_axis: 2,
                driven_axis: 2,
            },
            &ctx,
        );
        app.handle_industry_event(
            IndustryEvent::AddExplodeStep {
                instance: b,
                translation: [0.0, 0.0, 30.0],
            },
            &ctx,
        );
        let data = app.industry_panel_data();
        assert_eq!(data.couplings.len(), 1, "{:?}", app.model_status);
        assert_eq!(data.explode_steps.len(), 1);
        assert!(data.explode_factor > 0.0);
        app.handle_industry_event(IndustryEvent::RemoveCoupling(0), &ctx);
        app.handle_industry_event(IndustryEvent::RemoveExplodeStep(0), &ctx);
        let data = app.industry_panel_data();
        assert!(data.couplings.is_empty() && data.explode_steps.is_empty());
    }

    #[test]
    fn stack_check_json_parses_as_check() {
        let chain = vec![
            StackLinkUi {
                nominal: 25.0,
                fit: "H7".into(),
                ..StackLinkUi::default()
            },
            StackLinkUi {
                nominal: 25.0,
                reverse: true,
                ..StackLinkUi::default()
            },
        ];
        let value = stack_check_json("stackup1", &chain, 0.3, true);
        assert!(
            value["chain"][0].get("plus").is_none(),
            "fit menggantikan plus/minus"
        );
        let item: ducad_engine::check::CheckItem = serde_json::from_value(value).unwrap();
        assert_eq!(item.id.as_deref(), Some("stackup1"));
    }
}
