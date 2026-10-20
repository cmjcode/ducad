//! Workspace plastik (Paritas 6B): boss, lip/groove, snap-fit, rest pada
//! face/titik yang diklik. Ukuran bawaan dari aturan `dfm-injection` aktif
//! (draft, tebal dinding, radius dalam) — sama dengan op `plastic`.

use ducad_engine::compute::plastic::{self, PlasticDefaults};
use glam::DVec3;

use crate::app::DuCADApp;
use crate::model::ReplaceGeometryCommand;

/// Status jendela plastik.
pub struct PlasticToolsState {
    pub open: bool,
    pub boss_height: f64,
    pub boss_hole: f64,
    pub lip_height: f64,
    pub clearance: f64,
    pub snap_length: f64,
    pub snap_width: f64,
    pub snap_hook: f64,
    pub rest_d: f64,
    pub rest_h: f64,
}

impl Default for PlasticToolsState {
    fn default() -> Self {
        Self {
            open: false,
            boss_height: 8.0,
            boss_hole: 2.5,
            lip_height: 1.5,
            clearance: 0.1,
            snap_length: 12.0,
            snap_width: 6.0,
            snap_hook: 0.8,
            rest_d: 8.0,
            rest_h: 1.0,
        }
    }
}

impl DuCADApp {
    /// Nilai bawaan dari check desain aktif (rule pack).
    pub fn plastic_defaults(&self) -> PlasticDefaults {
        PlasticDefaults::from_checks(&self.agent_meta.design.checks)
    }

    /// Face aktif: (body, indeks face, titik klik, normal).
    fn plastic_target(&mut self) -> Option<(ducad_core::BodyId, usize, DVec3, DVec3)> {
        let Some((body, ray, hit)) = self.active_face.clone() else {
            self.model_status = Some(ducad_i18n::t!("direct-need-face"));
            return None;
        };
        let g = self.model.geometry.get(body)?;
        let fi = ducad_kernel::pick_face_details(&g.shape, ray)?.face_index?;
        Some((body, fi, DVec3::from(hit.hit_point), DVec3::from(hit.normal)))
    }

    fn plastic_apply(
        &mut self,
        label: &'static str,
        body: ducad_core::BodyId,
        r: ducad_engine::OpResult<crate::model::BodyGeometry>,
    ) {
        match r {
            Ok(geo) => {
                let name = self
                    .model
                    .doc
                    .bodies
                    .get(body)
                    .map(|b| b.name.clone())
                    .unwrap_or_default();
                let msg = ducad_i18n::t!("direct-done", tool = label, body = name.as_str());
                self.execute_model_command(
                    Box::new(ReplaceGeometryCommand::new(label, body, geo)),
                    &msg,
                );
                if self.model_status.is_none() {
                    self.model_status = Some(msg);
                }
            }
            Err(e) => {
                self.model_status = Some(crate::modeling::operations::op_status(label, &e))
            }
        }
    }

    pub fn plastic_boss(&mut self) {
        let d = self.plastic_defaults();
        let Some((body, _, p, n)) = self.plastic_target() else {
            return;
        };
        let Some(g) = self.model.geometry.get(body) else {
            return;
        };
        let st = &self.plastic_tools;
        let hole = st.boss_hole.max(0.0);
        let r = plastic::boss(
            &g.shape,
            p,
            n,
            st.boss_height,
            d.boss_outer(hole),
            hole,
            d.draft_deg,
            d.min_radius,
        );
        self.model_status = None;
        let r = r.map(|(geo, w)| {
            if !w.is_empty() {
                self.model_status = Some(w.join("; "));
            }
            geo
        });
        self.plastic_apply("Boss", body, r);
    }

    pub fn plastic_lip(&mut self, groove: bool) {
        let d = self.plastic_defaults();
        let Some((body, fi, _, _)) = self.plastic_target() else {
            return;
        };
        let Some(g) = self.model.geometry.get(body) else {
            return;
        };
        let st = &self.plastic_tools;
        let r = plastic::lip(&g.shape, fi, d.min_wall, st.lip_height, groove, st.clearance);
        self.model_status = None;
        self.plastic_apply(if groove { "Groove" } else { "Lip" }, body, r);
    }

    pub fn plastic_snap(&mut self) {
        let d = self.plastic_defaults();
        let Some((body, _, p, n)) = self.plastic_target() else {
            return;
        };
        let Some(g) = self.model.geometry.get(body) else {
            return;
        };
        let st = &self.plastic_tools;
        let r = plastic::snap_fit(
            &g.shape,
            p,
            n,
            DVec3::X,
            st.snap_length,
            1.5 * d.min_wall,
            st.snap_width,
            st.snap_hook,
            30.0,
        );
        self.model_status = None;
        let r = r.map(|s| {
            self.model_status = Some(ducad_i18n::t!(
                "plastic-strain",
                strain = format!("{:.2}", s.strain_pct),
                max = format!("{:.1}", plastic::SNAP_FIT_ALLOWABLE_STRAIN_PCT)
            ));
            s.geo
        });
        self.plastic_apply("Snap Fit", body, r);
    }

    pub fn plastic_rest(&mut self) {
        let Some((body, _, p, n)) = self.plastic_target() else {
            return;
        };
        let Some(g) = self.model.geometry.get(body) else {
            return;
        };
        let st = &self.plastic_tools;
        let r = plastic::rest(&g.shape, p, n, st.rest_d, st.rest_h);
        self.model_status = None;
        self.plastic_apply("Rest", body, r);
    }

    /// Jendela "Plastik".
    pub fn show_plastic_tools(&mut self, ctx: &egui::Context) {
        if !self.plastic_tools.open {
            return;
        }
        let d = self.plastic_defaults();
        #[derive(Clone, Copy)]
        enum Act {
            Boss,
            Lip,
            Groove,
            Snap,
            Rest,
        }
        let mut act = None;
        let mut open = true;
        let st = &mut self.plastic_tools;
        egui::Window::new(ducad_i18n::t!("plastic-title"))
            .open(&mut open)
            .collapsible(true)
            .resizable(false)
            .default_width(320.0)
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(ducad_i18n::t!(
                        "plastic-rules",
                        draft = format!("{:.1}", d.draft_deg),
                        wall = format!("{:.2}", d.min_wall),
                        radius = format!("{:.2}", d.min_radius)
                    ))
                    .small(),
                );
                ui.label(egui::RichText::new(ducad_i18n::t!("plastic-help")).small().weak());
                let mm = |ui: &mut egui::Ui, v: &mut f64| {
                    ui.add(
                        egui::DragValue::new(v)
                            .speed(0.1)
                            .range(0.0..=1e4)
                            .suffix(" mm"),
                    );
                };
                egui::Grid::new("plastic-grid").num_columns(3).show(ui, |ui| {
                    ui.label(ducad_i18n::t!("plastic-boss"));
                    ui.horizontal(|ui| {
                        mm(ui, &mut st.boss_height);
                        mm(ui, &mut st.boss_hole);
                    });
                    if ui.button(ducad_i18n::t!("direct-apply")).clicked() {
                        act = Some(Act::Boss);
                    }
                    ui.end_row();
                    ui.label(ducad_i18n::t!("plastic-lip"));
                    ui.horizontal(|ui| {
                        mm(ui, &mut st.lip_height);
                        mm(ui, &mut st.clearance);
                    });
                    ui.horizontal(|ui| {
                        if ui.button(ducad_i18n::t!("plastic-lip-btn")).clicked() {
                            act = Some(Act::Lip);
                        }
                        if ui.button(ducad_i18n::t!("plastic-groove-btn")).clicked() {
                            act = Some(Act::Groove);
                        }
                    });
                    ui.end_row();
                    ui.label(ducad_i18n::t!("plastic-snap"));
                    ui.horizontal(|ui| {
                        mm(ui, &mut st.snap_length);
                        mm(ui, &mut st.snap_width);
                        mm(ui, &mut st.snap_hook);
                    });
                    if ui.button(ducad_i18n::t!("direct-apply")).clicked() {
                        act = Some(Act::Snap);
                    }
                    ui.end_row();
                    ui.label(ducad_i18n::t!("plastic-rest"));
                    ui.horizontal(|ui| {
                        mm(ui, &mut st.rest_d);
                        mm(ui, &mut st.rest_h);
                    });
                    if ui.button(ducad_i18n::t!("direct-apply")).clicked() {
                        act = Some(Act::Rest);
                    }
                    ui.end_row();
                });
            });
        match act {
            Some(Act::Boss) => self.plastic_boss(),
            Some(Act::Lip) => self.plastic_lip(false),
            Some(Act::Groove) => self.plastic_lip(true),
            Some(Act::Snap) => self.plastic_snap(),
            Some(Act::Rest) => self.plastic_rest(),
            None => {}
        }
        if !open {
            self.plastic_tools.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boss_on_clicked_face_and_no_face_reports() {
        let mut app = DuCADApp::new_for_test();
        app.plastic_boss();
        assert!(app.model_status.is_some());
        app.create_primitive(ducad_ui::PrimitiveKind::Box, [40.0, 40.0, 3.0], [0.0; 3], false);
        let id = app.model.doc.bodies.iter().map(|(id, _)| id).next().unwrap();
        let ray = ducad_kernel::PickRay {
            origin: (20.0, 20.0, 50.0),
            dir: (0.0, 0.0, -1.0),
        };
        let hit = ducad_kernel::pick_face_details(&app.model.geometry.get(id).unwrap().shape, ray)
            .unwrap();
        app.active_face = Some((id, ray, hit));
        let v0 = app.model.geometry.get(id).unwrap().shape.volume();
        app.plastic_boss();
        let v1 = app.model.geometry.get(id).unwrap().shape.volume();
        assert!(v1 > v0 + 100.0, "{v0} -> {v1} {:?}", app.model_status);
        assert_eq!(app.plastic_defaults().draft_deg, 1.0);
    }
}
