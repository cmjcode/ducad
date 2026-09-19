//! Checks desain di GUI (P7.5): dievaluasi ulang setiap kali geometri body
//! berubah (dan tidak ada drag aktif); `min_wall` dihitung di thread latar
//! dari `Arc<KernelMesh>` — hasil lama ditampilkan redup sampai hasil baru
//! tiba.

use std::hash::{Hash, Hasher};
use std::sync::mpsc;

use ducad_engine::check::{
    evaluate_min_wall, resolve_min_wall, run_checks_on, Check, CheckResult, CheckStatus,
};
use ducad_engine::{DesignDoc, SessionMeta};
use ducad_ui::{CheckRowStatus, CheckRowUi};

use crate::app::DuCADApp;

/// Hasil `min_wall` dari thread latar: (tanda tangan model, indeks check, hasil).
pub type MinWallMsg = (u64, usize, CheckResult);

/// Keadaan checks milik `DuCADApp`.
#[derive(Default)]
pub struct ChecksState {
    pub panel_open: bool,
    pub results: Vec<CheckResult>,
    /// Paralel dengan `results`: hasil lama yang sedang dihitung ulang.
    pub stale: Vec<bool>,
    /// Tanda tangan model + design saat terakhir dievaluasi.
    pub signature: u64,
    pub rx: Option<mpsc::Receiver<MinWallMsg>>,
}

impl DuCADApp {
    /// Tanda tangan murah keadaan model: body (id, nama, visibilitas,
    /// alamat `Arc` mesh — berganti setiap geometri diganti) + design.
    fn checks_signature(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for (id, b) in self.model.doc.bodies.iter() {
            format!("{id:?}").hash(&mut h);
            b.name.hash(&mut h);
            b.visible.hash(&mut h);
            if let Some(g) = self.model.geometry.get(id) {
                (std::sync::Arc::as_ptr(&g.mesh) as usize).hash(&mut h);
            }
        }
        self.design.as_ref().map(|d| d.to_string()).hash(&mut h);
        h.finish()
    }

    fn design_doc(&self) -> Option<DesignDoc> {
        serde_json::from_value(self.design.clone()?).ok()
    }

    /// Dipanggil tiap frame: terima hasil `min_wall` dari latar, lalu
    /// evaluasi ulang bila model/design berubah dan pointer tidak ditekan.
    pub fn refresh_checks(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.checks.rx {
            while let Ok((sig, i, result)) = rx.try_recv() {
                if sig == self.checks.signature && i < self.checks.results.len() {
                    self.checks.results[i] = result;
                    self.checks.stale[i] = false;
                }
            }
        }
        if ctx.input(|i| i.pointer.any_down()) {
            return;
        }
        let sig = self.checks_signature();
        if sig == self.checks.signature {
            return;
        }
        self.checks.signature = sig;
        let Some(design) = self.design_doc().filter(|d| !d.checks.is_empty()) else {
            self.checks.results.clear();
            self.checks.stale.clear();
            return;
        };
        let checks = design.checks.clone();
        let meta = SessionMeta {
            design,
            ..SessionMeta::default()
        };

        // Semua check non-min_wall langsung; min_wall diberi placeholder.
        let mut results: Vec<CheckResult> = Vec::with_capacity(checks.len());
        let mut stale = vec![false; checks.len()];
        let (tx, rx) = mpsc::channel::<MinWallMsg>();
        for (i, item) in checks.iter().enumerate() {
            if let Check::MinWall { .. } = item.check {
                let previous = self
                    .checks
                    .results
                    .get(i)
                    .filter(|r| r.kind == "min_wall")
                    .cloned();
                let placeholder = previous.unwrap_or_else(|| CheckResult {
                    index: i,
                    id: item.id.clone(),
                    kind: "min_wall",
                    status: CheckStatus::Error,
                    measured: serde_json::Value::Null,
                    expected: serde_json::Value::Null,
                    message: ducad_i18n::t!("checks-stale"),
                    location: None,
                    body: None,
                });
                results.push(placeholder);
                stale[i] = true;
                let resolved = resolve_min_wall(&self.model, &meta, item);
                let mesh = resolved.as_ref().and_then(|(name, _)| {
                    let id = self
                        .model
                        .doc
                        .bodies
                        .iter()
                        .find(|(_, b)| &b.name == name)
                        .map(|(id, _)| id)?;
                    self.model.geometry.get(id).map(|g| g.mesh.clone())
                });
                match (resolved, mesh) {
                    (Some((name, min)), Some(mesh)) => {
                        let tx = tx.clone();
                        let id = item.id.clone();
                        let repaint = ctx.clone();
                        std::thread::spawn(move || {
                            let mut r = evaluate_min_wall(&mesh, &name, min);
                            r.index = i;
                            r.id = id;
                            let _ = tx.send((sig, i, r));
                            repaint.request_repaint();
                        });
                    }
                    _ => {
                        // Body tak dikenal dsb.: evaluasi sinkron untuk pesan error.
                        let mut r =
                            run_checks_on(&self.model, &meta, std::slice::from_ref(item)).remove(0);
                        r.index = i;
                        results[i] = r;
                        stale[i] = false;
                    }
                }
            } else {
                let mut r = run_checks_on(&self.model, &meta, std::slice::from_ref(item)).remove(0);
                r.index = i;
                results.push(r);
            }
        }
        self.checks.results = results;
        self.checks.stale = stale;
        self.checks.rx = Some(rx);
    }

    /// Baris untuk panel & ringkasan top bar.
    pub fn check_rows(&self) -> Vec<CheckRowUi> {
        self.checks
            .results
            .iter()
            .zip(
                self.checks
                    .stale
                    .iter()
                    .copied()
                    .chain(std::iter::repeat(false)),
            )
            .map(|(r, stale)| CheckRowUi {
                status: match r.status {
                    CheckStatus::Pass => CheckRowStatus::Pass,
                    CheckStatus::Fail => CheckRowStatus::Fail,
                    CheckStatus::Error => CheckRowStatus::Error,
                },
                label: r.id.clone().unwrap_or_else(|| r.kind.to_string()),
                detail: r.message.clone(),
                focusable: r.location.is_some() || r.body.is_some(),
                stale,
            })
            .collect()
    }

    /// Klik baris: pilih body terkait dan arahkan kamera ke lokasi check.
    pub fn focus_check(&mut self, i: usize) {
        let Some(r) = self.checks.results.get(i) else {
            return;
        };
        if let Some(name) = &r.body {
            if let Some((id, _)) = self.model.doc.bodies.iter().find(|(_, b)| &b.name == name) {
                self.selected_bodies.clear();
                self.selected_bodies.insert(id);
            }
        }
        if let Some(p) = r.location {
            self.camera.target = glam::Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_plate_and_checks() -> DuCADApp {
        let mut app = DuCADApp::new_for_test();
        let f: ducad_engine::ops::OpFile =
            serde_json::from_str(ducad_engine::ops::EXAMPLE_PLATE).unwrap();
        let mut s = ducad_engine::Session::new();
        assert!(s.set_params(f.params).unwrap().committed);
        assert!(s.run(f.ops, false).committed);
        s.set_checks(
            serde_json::from_str(
                r#"[{"check":"body_count","expect":1},
                    {"id":"dinding","check":"min_wall","body":"*","min":2},
                    {"check":"hole_count","body":"*","diameter":5.5,"expect":3}]"#,
            )
            .unwrap(),
        );
        let (_, geo) = s.body("plate").unwrap();
        let id = app.model.doc.add_body("plate");
        let shape = ducad_kernel::clone_shape(&geo.shape).unwrap();
        app.model
            .geometry
            .insert(id, crate::model::BodyGeometry::from_shape(shape));
        app.design = Some(serde_json::to_value(s.design()).unwrap());
        app
    }

    #[test]
    fn checks_refresh_with_background_min_wall() {
        let mut app = app_with_plate_and_checks();
        let ctx = egui::Context::default();
        app.refresh_checks(&ctx);
        assert_eq!(app.checks.results.len(), 3);
        assert_eq!(app.checks.results[0].status, CheckStatus::Pass);
        assert_eq!(app.checks.results[2].status, CheckStatus::Fail);
        assert!(app.checks.stale[1], "min_wall dihitung di latar");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while app.checks.stale[1] && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
            app.refresh_checks(&ctx);
        }
        assert!(!app.checks.stale[1]);
        assert_eq!(app.checks.results[1].status, CheckStatus::Pass);
        assert_eq!(app.checks.results[1].id.as_deref(), Some("dinding"));

        let rows = app.check_rows();
        assert_eq!(ducad_ui::checks_summary(&rows), (2, 1));
        app.focus_check(1);
        assert_eq!(app.selected_bodies.len(), 1);
    }

    #[test]
    fn no_design_means_no_rows() {
        let mut app = DuCADApp::new_for_test();
        app.refresh_checks(&egui::Context::default());
        assert!(app.check_rows().is_empty());
    }
}
