//! Panel "Simulasi" di GUI (P17). Studi disimpan sebagai `Op::Study` di
//! oplog agent (jalur yang sama dengan `run_ops`), solver berjalan di thread
//! latar dengan `CancelToken`, hasilnya masuk `agent_meta.sim_results`
//! (sehingga check `max_stress` dkk. ikut hidup) dan digambar sebagai overlay
//! berwarna di viewport — hasil lama tampil redup sampai yang baru tiba.

use std::collections::BTreeMap;
use std::sync::mpsc;
use std::sync::Arc;

use ducad_core::BodyId;
use ducad_engine::sim::{HeatTriangle, Overlay, StudyResult};
use ducad_engine::SessionCore;
use ducad_sim::{CancelToken, SimError, SimReport, SimSetup};
use ducad_ui::{
    LoadKindUi, SimOverlayUi, SimPanel, SimPanelData, SimPanelEvent, SimResultUi, SimRunStatus,
    SimStudyRow, StudyDraft,
};
use serde_json::json;

use crate::app::DuCADApp;

/// Anggaran segitiga overlay viewport (diurutkan ulang tiap frame).
const OVERLAY_TRIANGLES: usize = 12_000;

struct SimJob {
    id: String,
    signature: u64,
    cancel: CancelToken,
    rx: mpsc::Receiver<Result<SimReport, SimError>>,
}

/// Hasil yang siap digambar.
struct SimView {
    signature: u64,
    report: Arc<SimReport>,
    tris: Vec<HeatTriangle>,
    yield_mpa: f64,
}

/// Kunci cache selector: body + centroid face terkuantisasi (µm).
type PickKey = (BodyId, [i64; 3]);

/// Keadaan simulasi milik `DuCADApp`.
#[derive(Default)]
pub struct SimState {
    pub panel_open: bool,
    pub panel: SimPanel,
    pub selected: Option<usize>,
    job: Option<SimJob>,
    views: BTreeMap<String, SimView>,
    failures: BTreeMap<String, String>,
    /// (body, centroid face terkuantisasi) → selector face terpilih.
    picked_cache: Option<(PickKey, Option<String>)>,
}

fn overlay_of(ui: SimOverlayUi) -> Overlay {
    match ui {
        SimOverlayUi::Stress => Overlay::Stress,
        SimOverlayUi::Displacement => Overlay::Displacement,
        SimOverlayUi::SafetyFactor => Overlay::SafetyFactor,
    }
}

/// Ubah draf panel menjadi JSON `SimSetup`.
pub fn setup_json(body: &str, draft: &StudyDraft) -> serde_json::Value {
    let fixtures: Vec<_> = draft
        .fixtures
        .iter()
        .enumerate()
        .map(|(i, f)| json!({ "id": format!("fix{}", i + 1), "faces": f.faces, "kind": f.kind.key() }))
        .collect();
    let loads: Vec<_> = draft
        .loads
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let id = format!("load{}", i + 1);
            match l.kind {
                LoadKindUi::Force => {
                    json!({ "id": id, "faces": l.faces, "kind": "force", "newton": l.vector })
                }
                LoadKindUi::Pressure => {
                    json!({ "id": id, "faces": l.faces, "kind": "pressure", "mpa": l.scalar })
                }
                LoadKindUi::Gravity => {
                    json!({ "id": id, "kind": "gravity", "g": l.scalar, "dir": l.vector })
                }
            }
        })
        .collect();
    let mut setup = json!({ "body": body, "fixtures": fixtures, "loads": loads });
    if draft.cell_mm > 0.0 {
        setup["mesh"] = json!({ "cell_mm": draft.cell_mm });
    }
    setup
}

impl DuCADApp {
    fn sim_studies(&self) -> Vec<(String, SimSetup)> {
        ducad_engine::sim::studies(&self.agent_meta)
            .into_iter()
            .map(|(id, setup)| (id.to_string(), setup.clone()))
            .collect()
    }

    /// Body sasaran studi baru: pemilik face aktif, body terpilih, atau
    /// body pertama.
    fn sim_target(&self) -> Option<BodyId> {
        self.active_face
            .as_ref()
            .map(|(id, _, _)| *id)
            .filter(|id| self.model.doc.bodies.contains_key(*id))
            .or_else(|| {
                self.selected_bodies
                    .iter()
                    .copied()
                    .find(|id| self.model.doc.bodies.contains_key(*id))
            })
            .or_else(|| self.model.doc.bodies.keys().next())
    }

    /// Selector untuk face yang sedang aktif di viewport: arah ekstrem
    /// (`>Z`, `<X`, …) bila itu menunjuk tepat face tersebut, selain itu
    /// `idx:<n>`.
    fn sim_picked_selector(&mut self) -> Option<String> {
        let (body, _, hit) = self.active_face.as_ref()?;
        let c = hit.centroid;
        let key = (*body, [c.0, c.1, c.2].map(|v| (v * 1000.0).round() as i64));
        if let Some((cached, sel)) = &self.sim.picked_cache {
            if *cached == key {
                return sel.clone();
            }
        }
        let selector = self.model.geometry.get(*body).and_then(|geo| {
            let faces = ducad_kernel::enumerate_faces(&geo.shape);
            let dist2 = |f: &ducad_kernel::FaceInfo| {
                (f.centroid[0] - c.0).powi(2)
                    + (f.centroid[1] - c.1).powi(2)
                    + (f.centroid[2] - c.2).powi(2)
            };
            let face = faces.iter().min_by(|a, b| dist2(a).total_cmp(&dist2(b)))?;
            let semantic = [">X", "<X", ">Y", "<Y", ">Z", "<Z"]
                .into_iter()
                .find(|sel| {
                    ducad_engine::select::select_faces(&geo.shape, sel)
                        .is_ok_and(|hits| hits == [face.index])
                });
            Some(
                semantic
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("idx:{}", face.index)),
            )
        });
        self.sim.picked_cache = Some((key, selector.clone()));
        selector
    }

    fn sim_status(&self, id: &str, setup: &SimSetup) -> SimRunStatus {
        if self.sim.job.as_ref().is_some_and(|j| j.id == id) {
            return SimRunStatus::Running;
        }
        if let Some(why) = self.sim.failures.get(id) {
            return SimRunStatus::Failed(why.clone());
        }
        match self.sim.views.get(id) {
            None => SimRunStatus::NotRun,
            Some(v)
                if ducad_engine::sim::study_signature(&self.model, setup) == Some(v.signature) =>
            {
                SimRunStatus::Done
            }
            Some(_) => SimRunStatus::Stale,
        }
    }

    /// Rentang nilai overlay aktif pada hasil `view`.
    fn sim_legend(&self, view: &SimView) -> Option<(f64, f64)> {
        let overlay = overlay_of(self.sim.panel.overlay);
        let mut range: Option<(f64, f64)> = None;
        for t in &view.tris {
            for k in 0..3 {
                let v = ducad_engine::sim::overlay_value(
                    overlay,
                    t.von_mises[k],
                    t.displacement[k],
                    view.yield_mpa,
                );
                range = Some(range.map_or((v, v), |(lo, hi)| (lo.min(v), hi.max(v))));
            }
        }
        range
    }

    pub fn sim_panel_data(&mut self) -> SimPanelData {
        let studies = self.sim_studies();
        if self.sim.selected.is_some_and(|i| i >= studies.len()) {
            self.sim.selected = None;
        }
        if self.sim.selected.is_none() && !studies.is_empty() {
            self.sim.selected = Some(0);
        }
        let rows: Vec<SimStudyRow> = studies
            .iter()
            .map(|(id, setup)| SimStudyRow {
                id: id.clone(),
                body: setup.body.clone(),
                fixtures: setup.fixtures.len(),
                loads: setup.loads.len(),
                status: self.sim_status(id, setup),
            })
            .collect();
        let result = self.sim.selected.and_then(|i| {
            let view = self.sim.views.get(&studies.get(i)?.0)?;
            let r = &view.report;
            Some(SimResultUi {
                max_von_mises_mpa: r.max_von_mises_mpa,
                max_displacement_mm: r.max_displacement_mm,
                safety_factor: r.safety_factor,
                reactions: r
                    .reactions
                    .iter()
                    .map(|x| (x.fixture_id.clone(), x.force_n))
                    .collect(),
                elements: r.mesh_stats.elements,
                nodes: r.mesh_stats.nodes,
                cell_mm: r.mesh_stats.cell_mm,
                iterations: r.solver_stats.iterations,
                warnings: r.warnings.clone(),
                legend: self.sim_legend(view),
                stale: rows[i].status != SimRunStatus::Done,
            })
        });
        let target = self.sim_target();
        let target_body = target.and_then(|id| self.model.doc.bodies.get(id));
        SimPanelData {
            selected: self.sim.selected,
            result,
            target_body: target_body.map(|b| b.name.clone()),
            target_has_material: target_body
                .is_some_and(|b| self.model.doc.mechanical_of(b).is_some()),
            picked_face: self.sim_picked_selector(),
            studies: rows,
        }
    }

    /// Jalankan tool core langsung di atas state GUI (tanpa mode usul).
    fn sim_core_tool(&mut self, tool: &str, args: serde_json::Value) -> Result<(), String> {
        self.sync_agent_meta();
        let out = {
            let mut core = SessionCore {
                model: &mut self.model,
                model_undo: &mut self.model_undo,
                sketches: &mut self.sketch_set,
                meta: &mut self.agent_meta,
            };
            ducad_engine::tooling::call_core_tool(
                &mut core,
                tool,
                args,
                &ducad_engine::tooling::NoPaths,
            )
        };
        match out {
            Ok(o) if !o.is_error && o.payload["committed"] != json!(false) => {
                self.sync_design_after_agent();
                Ok(())
            }
            Ok(o) => Err(o.payload["error"]["message"]
                .as_str()
                .unwrap_or("operasi ditolak")
                .to_string()),
            Err(e) => Err(e.message),
        }
    }

    fn sim_create(&mut self, draft: StudyDraft) {
        let Some(body) = self
            .sim_target()
            .and_then(|id| self.model.doc.bodies.get(id))
            .map(|b| b.name.clone())
        else {
            return;
        };
        let op = json!({ "op": "study", "id": draft.id, "setup": setup_json(&body, &draft) });
        match self.sim_core_tool("run_ops", json!({ "ops": [op] })) {
            Ok(()) => {
                self.sim.selected = Some(self.sim_studies().len().saturating_sub(1));
                self.model_status = Some(ducad_i18n::t!("sim-created"));
            }
            Err(why) => self.model_status = Some(why),
        }
    }

    fn sim_delete(&mut self, index: usize) {
        let Some((id, _)) = self.sim_studies().into_iter().nth(index) else {
            return;
        };
        self.sync_agent_meta();
        match self.agent_edit_oplog("remove_op", json!({ "ids": [id] })) {
            Ok(out) if !out.is_error => {
                self.sim.views.remove(&id);
                self.sim.failures.remove(&id);
                self.model_status = Some(ducad_i18n::t!("sim-deleted"));
            }
            Ok(out) => {
                self.model_status = out.payload["error"]["message"].as_str().map(str::to_string)
            }
            Err(e) => self.model_status = Some(e.message),
        }
    }

    /// Mulai studi `index` di thread latar. Selector dan material
    /// diselesaikan DI SINI (butuh kernel); thread hanya menjalankan solver.
    fn sim_run(&mut self, index: usize, ctx: &egui::Context) {
        if self.sim.job.is_some() {
            return;
        }
        self.sync_agent_meta();
        let Some((id, setup)) = self.sim_studies().into_iter().nth(index) else {
            return;
        };
        self.sim.failures.remove(&id);
        let prepared = ducad_engine::sim::prepare_static(&self.model, &self.agent_meta, &setup);
        let signature = ducad_engine::sim::study_signature(&self.model, &setup);
        let ((surface, material, resolved), signature) = match (prepared, signature) {
            (Ok(p), Some(sig)) => (p, sig),
            (Err(e), _) => {
                self.sim.failures.insert(id, e.message);
                return;
            }
            (_, None) => return,
        };
        let cancel = CancelToken::new();
        let (tx, rx) = mpsc::channel();
        let (thread_cancel, repaint) = (cancel.clone(), ctx.clone());
        std::thread::spawn(move || {
            let result = ducad_sim::run_static(&surface, &material, &resolved, &thread_cancel);
            let _ = tx.send(result);
            repaint.request_repaint();
        });
        self.sim.job = Some(SimJob {
            id,
            signature,
            cancel,
            rx,
        });
    }

    /// Dipanggil tiap frame: terima hasil solver dari thread latar.
    pub fn refresh_sim(&mut self) {
        let Some(job) = &self.sim.job else {
            return;
        };
        let result = match job.rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Err(SimError::Cancelled),
        };
        let Some(job) = self.sim.job.take() else {
            return;
        };
        match result {
            Ok(report) => self.sim_store(job.id, job.signature, report),
            Err(SimError::Cancelled) => {
                self.model_status = Some(ducad_i18n::t!("sim-cancelled"));
            }
            Err(e) => {
                let why = match e.hint() {
                    Some(hint) => format!("{}: {e} ({hint})", e.code()),
                    None => format!("{}: {e}", e.code()),
                };
                self.sim.failures.insert(job.id, why);
            }
        }
    }

    fn sim_store(&mut self, id: String, signature: u64, report: SimReport) {
        let report = Arc::new(report);
        let setup = self
            .sim_studies()
            .into_iter()
            .find(|(sid, _)| *sid == id)
            .map(|(_, s)| s);
        let body = setup
            .as_ref()
            .and_then(|s| ducad_engine::sim::study_body(&self.model, &self.agent_meta, s).ok());
        let Some((geo, yield_mpa)) = body else {
            return;
        };
        let tris = ducad_engine::sim::study_triangles(geo, &report, OVERLAY_TRIANGLES);
        self.agent_meta.sim_results.insert(
            id.clone(),
            StudyResult {
                signature,
                report: report.clone(),
            },
        );
        self.sim.views.insert(
            id,
            SimView {
                signature,
                report,
                tris,
                yield_mpa,
            },
        );
        // Check `max_stress` dkk. dievaluasi ulang dengan hasil baru.
        self.checks.signature = 0;
    }

    pub fn handle_sim_panel_event(&mut self, event: SimPanelEvent, ctx: &egui::Context) {
        match event {
            SimPanelEvent::Close => self.sim.panel_open = false,
            SimPanelEvent::Select(i) => self.sim.selected = Some(i),
            SimPanelEvent::Run(i) => {
                self.sim.selected = Some(i);
                self.sim_run(i, ctx);
            }
            SimPanelEvent::Cancel => {
                if let Some(job) = &self.sim.job {
                    job.cancel.cancel();
                }
            }
            SimPanelEvent::Delete(i) => self.sim_delete(i),
            SimPanelEvent::Create(draft) => self.sim_create(draft),
        }
    }

    /// Overlay hasil studi terpilih: segitiga berwarna per-sudut, digeser
    /// deformasi × skala, digambar jauh → dekat.
    pub fn paint_sim_overlay(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        if !(self.sim.panel_open && self.sim.panel.show_overlay) {
            return;
        }
        let studies = self.sim_studies();
        let Some((id, setup)) = self.sim.selected.and_then(|i| studies.get(i)) else {
            return;
        };
        let Some(view) = self.sim.views.get(id) else {
            return;
        };
        let Some((lo, hi)) = self.sim_legend(view) else {
            return;
        };
        let stale = self.sim_status(id, setup) != SimRunStatus::Done;
        let overlay = overlay_of(self.sim.panel.overlay);
        let span = (hi - lo).max(1e-12);
        let deform = if self.sim.panel.auto_deform {
            let diag = view.tris.iter().flat_map(|t| t.position).fold(
                ([f64::MAX; 3], [f64::MIN; 3]),
                |(mut a, mut b), p| {
                    for k in 0..3 {
                        a[k] = a[k].min(p[k]);
                        b[k] = b[k].max(p[k]);
                    }
                    (a, b)
                },
            );
            let size = (0..3)
                .map(|k| (diag.1[k] - diag.0[k]).powi(2))
                .sum::<f64>()
                .sqrt();
            if view.report.max_displacement_mm > 1e-12 {
                0.05 * size / view.report.max_displacement_mm
            } else {
                0.0
            }
        } else {
            self.sim.panel.deform_scale as f64
        };

        let aspect = rect.width() / rect.height().max(1.0);
        let vp = self.camera.view_proj(aspect);
        let alpha = if stale { 90 } else { 255 };
        let mut drawn: Vec<(f32, [egui::Pos2; 3], [egui::Color32; 3])> =
            Vec::with_capacity(view.tris.len());
        for t in &view.tris {
            let mut px = [egui::Pos2::ZERO; 3];
            let mut colors = [egui::Color32::WHITE; 3];
            let mut depth = 0.0;
            let mut visible = true;
            for k in 0..3 {
                let (p, d) = (t.position[k], t.displacement[k]);
                let world = glam::Vec3::new(
                    (p[0] + d[0] * deform) as f32,
                    (p[1] + d[1] * deform) as f32,
                    (p[2] + d[2] * deform) as f32,
                );
                let clip = vp.project_point3(world);
                if !(0.0..=1.0).contains(&clip.z) {
                    visible = false;
                    break;
                }
                px[k] = egui::pos2(
                    rect.min.x + (clip.x + 1.0) * 0.5 * rect.width(),
                    rect.min.y + (1.0 - clip.y) * 0.5 * rect.height(),
                );
                depth += clip.z;
                let v =
                    ducad_engine::sim::overlay_value(overlay, t.von_mises[k], d, view.yield_mpa);
                let mut x = ((v - lo) / span) as f32;
                if overlay == Overlay::SafetyFactor {
                    x = 1.0 - x;
                }
                let c = ducad_ui::turbo_color(x);
                colors[k] = egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha);
            }
            if visible {
                drawn.push((depth, px, colors));
            }
        }
        // Jauh (z besar) → dekat, supaya sisi depan menimpa sisi belakang.
        drawn.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut mesh = egui::Mesh::default();
        for (_, px, colors) in drawn {
            let base = mesh.vertices.len() as u32;
            for k in 0..3 {
                mesh.colored_vertex(px[k], colors[k]);
            }
            mesh.add_triangle(base, base + 1, base + 2);
        }
        ui.painter_at(rect).add(egui::Shape::mesh(mesh));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_ui::{FixtureDraft, FixtureKindUi, LoadDraft};

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

    fn draft() -> StudyDraft {
        StudyDraft {
            id: "pull".into(),
            fixtures: vec![FixtureDraft {
                faces: "<Z".into(),
                kind: FixtureKindUi::Fixed,
            }],
            loads: vec![LoadDraft {
                faces: ">Z".into(),
                kind: LoadKindUi::Force,
                vector: [0.0, 0.0, 1000.0],
                scalar: 0.0,
            }],
            cell_mm: 2.5,
        }
    }

    fn wait_for_job(app: &mut DuCADApp) {
        for _ in 0..2000 {
            app.refresh_sim();
            if app.sim.job.is_none() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("studi tidak selesai dalam 10 detik");
    }

    #[test]
    fn sim_setup_json_matches_engine_schema() {
        let mut d = draft();
        d.loads.push(LoadDraft {
            faces: String::new(),
            kind: LoadKindUi::Gravity,
            vector: [0.0, 0.0, -1.0],
            scalar: 9.80665,
        });
        d.loads.push(LoadDraft {
            faces: ">X".into(),
            kind: LoadKindUi::Pressure,
            vector: [0.0; 3],
            scalar: 0.2,
        });
        let setup: SimSetup = serde_json::from_value(setup_json("bar", &d)).unwrap();
        assert_eq!(setup.body, "bar");
        assert_eq!(setup.loads.len(), 3);
        assert_eq!(setup.loads[1].faces, None);
        assert_eq!(setup.mesh.cell_mm, Some(2.5));
    }

    #[test]
    fn sim_study_runs_in_background_and_feeds_checks() {
        let mut app = app_with_bar();
        let ctx = egui::Context::default();
        app.handle_sim_panel_event(SimPanelEvent::Create(draft()), &ctx);
        let data = app.sim_panel_data();
        assert_eq!(data.studies.len(), 1, "{:?}", app.model_status);
        assert_eq!(data.studies[0].status, SimRunStatus::NotRun);
        assert!(data.target_has_material);

        app.handle_sim_panel_event(SimPanelEvent::Run(0), &ctx);
        assert_eq!(
            app.sim_panel_data().studies[0].status,
            SimRunStatus::Running
        );
        wait_for_job(&mut app);
        let data = app.sim_panel_data();
        assert_eq!(
            data.studies[0].status,
            SimRunStatus::Done,
            "{:?}",
            data.studies
        );
        let r = data.result.unwrap();
        // 1000 N pada 100 mm² → 10 MPa nominal.
        assert!(r.max_von_mises_mpa > 8.0 && r.max_von_mises_mpa < 20.0);
        assert!(r.legend.is_some_and(|(lo, hi)| hi > lo));
        assert!(!r.stale);
        // Hasil masuk cache yang dibaca check engine.
        assert!(ducad_engine::sim::fresh_result(&app.model, &app.agent_meta, "pull").is_ok());

        app.handle_sim_panel_event(SimPanelEvent::Delete(0), &ctx);
        assert!(
            app.sim_panel_data().studies.is_empty(),
            "{:?}",
            app.model_status
        );
    }

    #[test]
    fn sim_failure_and_cancel_are_reported() {
        let mut app = app_with_bar();
        let ctx = egui::Context::default();
        // Tanpa material → gagal saat persiapan, dengan pesan.
        let id = app.model.doc.bodies.keys().next().unwrap();
        app.model.doc.bodies[id].mechanical = None;
        app.handle_sim_panel_event(SimPanelEvent::Create(draft()), &ctx);
        app.handle_sim_panel_event(SimPanelEvent::Run(0), &ctx);
        let status = app.sim_panel_data().studies[0].status.clone();
        assert!(matches!(status, SimRunStatus::Failed(_)), "{status:?}");

        // Dengan material: batalkan segera → tidak ada hasil, tidak beku.
        app.model.doc.bodies[id].mechanical =
            Some(ducad_core::MaterialSource::Library("s235".into()));
        app.handle_sim_panel_event(SimPanelEvent::Run(0), &ctx);
        app.handle_sim_panel_event(SimPanelEvent::Cancel, &ctx);
        wait_for_job(&mut app);
        let status = app.sim_panel_data().studies[0].status.clone();
        assert!(
            matches!(status, SimRunStatus::NotRun | SimRunStatus::Done),
            "{status:?}"
        );
    }
}
