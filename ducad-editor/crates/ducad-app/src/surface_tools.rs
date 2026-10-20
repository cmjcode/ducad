//! Workspace permukaan (Paritas 6A, 6.8): jendela "Permukaan" yang memakai
//! seleksi GUI — sketsa aktif (extrude/revolve/sweep), tepi terpilih
//! (patch, ruled, extend, boundary), face terpilih (offset, salin, balik
//! normal, fair, untrim, unstitch), body terpilih (thicken, stitch, trim).
//! Geometri dari `ducad_kernel::surface` (sama dengan op `surface` MCP/CLI).

use ducad_core::BodyId;
use ducad_kernel::surface::{self, RuledDir};
use ducad_kernel::KernelShape;
use glam::DVec3;

use crate::app::DuCADApp;
use crate::model::{AddSolidCommand, BodyGeometry, ReplaceGeometryCommand};

/// Status jendela permukaan.
pub struct SurfaceToolsState {
    pub open: bool,
    pub start: f64,
    pub end: f64,
    pub revolve_v: bool,
    pub angle: f64,
    pub distance: f64,
    pub ruled_tangent: bool,
    pub thickness: f64,
    pub symmetric: bool,
    /// Trim: 0 = XY, 1 = XZ, 2 = YZ.
    pub trim_plane: usize,
    pub trim_offset: f64,
    pub trim_keep_positive: bool,
}

impl Default for SurfaceToolsState {
    fn default() -> Self {
        Self {
            open: false,
            start: 0.0,
            end: 20.0,
            revolve_v: true,
            angle: 360.0,
            distance: 5.0,
            ruled_tangent: false,
            thickness: 2.0,
            symmetric: false,
            trim_plane: 0,
            trim_offset: 0.0,
            trim_keep_positive: true,
        }
    }
}

/// Sheet atau solid valid → geometri; selain itu pesan galat.
fn to_geo(r: anyhow::Result<KernelShape>) -> Result<BodyGeometry, String> {
    let s = r.map_err(|e| format!("{e:#}"))?;
    if s.is_valid() || s.is_sheet() {
        Ok(BodyGeometry::from_shape(s))
    } else {
        Err(ducad_i18n::t!("surface-invalid"))
    }
}

type SurfaceOp = fn(&mut DuCADApp);

impl DuCADApp {
    /// Bingkai bidang sketsa aktif (asal, u, v) dalam koordinat dunia.
    fn active_frame(&self) -> surface::SketchFrame {
        let p = self.active_plane;
        let f = |v: glam::Vec3| [v.x as f64, v.y as f64, v.z as f64];
        (p.to_world_f64((0.0, 0.0), 0.0), f(p.u_axis), f(p.v_axis))
    }

    /// Kurva sketsa aktif (terpilih, atau semua non-konstruksi).
    fn active_chains(&self) -> Vec<Vec<ducad_kernel::ProfileSegment>> {
        let mut sk = self.sketch().clone();
        if !self.selected.is_empty() {
            let drop: Vec<_> = sk
                .entities
                .iter()
                .filter(|(id, _)| !self.selected.contains(id))
                .map(|(id, _)| id)
                .collect();
            for id in drop {
                sk.entities.remove(id);
            }
        }
        ducad_engine::compute::direct::sketch_curve_chains(&sk)
    }

    /// Tepi terpilih → (body, indeks tepi logis).
    fn selected_edge_indices(&self) -> Option<(BodyId, Vec<usize>)> {
        for (id, g) in self.model.geometry.iter() {
            let all = ducad_kernel::enumerate_edges(&g.shape);
            let idx: Vec<usize> = self
                .selected_edges
                .iter()
                .filter_map(|e| crate::sketch_project::match_edge_index(&all, &e.polyline))
                .collect();
            if !idx.is_empty() {
                return Some((id, idx));
            }
        }
        None
    }

    fn surface_add(&mut self, name: &str, r: anyhow::Result<KernelShape>) {
        match to_geo(r) {
            Ok(geo) => {
                let msg = ducad_i18n::t!("direct-created", name = name);
                self.execute_model_command(
                    Box::new(AddSolidCommand::new(name.to_string(), geo)),
                    &msg,
                );
                self.model_status = Some(msg);
            }
            Err(e) => self.model_status = Some(ducad_i18n::t!("surface-failed", error = e)),
        }
    }

    fn surface_replace(&mut self, label: &'static str, id: BodyId, r: anyhow::Result<KernelShape>) {
        match to_geo(r) {
            Ok(geo) => {
                let body = self
                    .model
                    .doc
                    .bodies
                    .get(id)
                    .map(|b| b.name.clone())
                    .unwrap_or_default();
                let msg = ducad_i18n::t!("direct-done", tool = label, body = body.as_str());
                self.execute_model_command(
                    Box::new(ReplaceGeometryCommand::new(label, id, geo)),
                    &msg,
                );
                self.model_status = Some(msg);
            }
            Err(e) => self.model_status = Some(ducad_i18n::t!("surface-failed", error = e)),
        }
    }

    fn need_chains(&mut self) -> Option<Vec<Vec<ducad_kernel::ProfileSegment>>> {
        let c = self.active_chains();
        if c.is_empty() {
            self.model_status = Some(ducad_i18n::t!("surface-need-sketch"));
            return None;
        }
        Some(c)
    }

    pub fn surface_extrude(&mut self) {
        let Some(c) = self.need_chains() else { return };
        let (a, b) = (self.surface_tools.start, self.surface_tools.end);
        let r = surface::extrude_sheet(self.active_frame(), &c, a, b);
        self.surface_add("Surface Extrude", r);
    }

    pub fn surface_revolve(&mut self) {
        let Some(c) = self.need_chains() else { return };
        let dir = if self.surface_tools.revolve_v {
            (0.0, 1.0)
        } else {
            (1.0, 0.0)
        };
        let r = surface::revolve_sheet(
            self.active_frame(),
            &c,
            (0.0, 0.0),
            dir,
            self.surface_tools.angle,
        );
        self.surface_add("Surface Revolve", r);
    }

    pub fn surface_sweep(&mut self) {
        let Some(c) = self.need_chains() else { return };
        let polys: Vec<Vec<DVec3>> = self
            .selected_edges
            .iter()
            .map(|e| e.polyline.iter().map(|p| DVec3::new(p.0, p.1, p.2)).collect())
            .collect();
        let path: Vec<[f64; 3]> = crate::direct_tools::chain_edge_polylines(&polys)
            .iter()
            .map(|p| p.to_array())
            .collect();
        if path.len() < 2 {
            self.model_status = Some(ducad_i18n::t!("direct-need-edges"));
            return;
        }
        let r = surface::sweep_sheet(self.active_frame(), &c, &path);
        self.surface_add("Surface Sweep", r);
    }

    fn with_edges(&mut self) -> Option<(BodyId, KernelShape, Vec<usize>)> {
        let Some((id, idx)) = self.selected_edge_indices() else {
            self.model_status = Some(ducad_i18n::t!("direct-need-edges"));
            return None;
        };
        let shape = ducad_kernel::clone_shape(&self.model.geometry.get(id)?.shape).ok()?;
        Some((id, shape, idx))
    }

    pub fn surface_patch(&mut self) {
        let Some((_, s, e)) = self.with_edges() else { return };
        self.surface_add("Patch", surface::patch(&s, &e));
    }

    pub fn surface_ruled(&mut self) {
        let Some((_, s, e)) = self.with_edges() else { return };
        let dir = if self.surface_tools.ruled_tangent {
            RuledDir::Tangent
        } else {
            RuledDir::Normal
        };
        let r = surface::ruled(&s, &e, self.surface_tools.distance, dir, false);
        self.surface_add("Ruled", r);
    }

    pub fn surface_extend(&mut self) {
        let Some((id, s, e)) = self.with_edges() else { return };
        let r = surface::extend(&s, &e, self.surface_tools.distance);
        self.surface_replace("Extend", id, r);
    }

    /// Boundary: tiap tepi terpilih = satu kurva u.
    pub fn surface_boundary(&mut self) {
        let curves: Vec<Vec<[f64; 3]>> = self
            .selected_edges
            .iter()
            .map(|e| e.polyline.iter().map(|p| [p.0, p.1, p.2]).collect())
            .collect();
        if curves.len() < 2 {
            self.model_status = Some(ducad_i18n::t!("surface-need-two-edges"));
            return;
        }
        self.surface_add("Boundary", surface::boundary(&curves, &[]));
    }

    fn with_faces(&mut self) -> Option<(BodyId, KernelShape, Vec<usize>)> {
        let picks = self.selected_face_picks();
        let Some(first) = picks.first().map(|p| p.body) else {
            self.model_status = Some(ducad_i18n::t!("direct-need-face"));
            return None;
        };
        let idx: Vec<usize> = picks
            .iter()
            .filter(|p| p.body == first)
            .map(|p| p.index)
            .collect();
        let shape = ducad_kernel::clone_shape(&self.model.geometry.get(first)?.shape).ok()?;
        Some((first, shape, idx))
    }

    pub fn surface_offset(&mut self) {
        let Some((_, s, f)) = self.with_faces() else { return };
        let r = surface::offset_surface(&s, &f, self.surface_tools.distance);
        self.surface_add("Offset Surface", r);
    }

    pub fn surface_copy(&mut self) {
        let Some((_, s, f)) = self.with_faces() else { return };
        self.surface_add("Copy Faces", surface::copy_faces(&s, &f));
    }

    pub fn surface_reverse(&mut self) {
        let Some((id, s, f)) = self.with_faces() else { return };
        self.surface_replace("Reverse Normal", id, surface::reverse_normals(&s, &f));
    }

    pub fn surface_fair(&mut self) {
        let Some((id, s, f)) = self.with_faces() else { return };
        self.surface_replace("Fair", id, surface::fair(&s, &f));
    }

    pub fn surface_untrim(&mut self) {
        let Some((id, s, f)) = self.with_faces() else { return };
        let Some(&face) = f.first() else { return };
        self.surface_replace("Untrim", id, surface::untrim(&s, face));
    }

    pub fn surface_unstitch(&mut self) {
        let Some((id, s, f)) = self.with_faces() else { return };
        match surface::unstitch(&s, &f) {
            Ok((pieces, rest)) => {
                if let Some(rest) = rest {
                    self.surface_replace("Unstitch", id, Ok(rest));
                }
                for p in pieces {
                    self.surface_add("Unstitch", Ok(p));
                }
            }
            Err(e) => {
                self.model_status =
                    Some(ducad_i18n::t!("surface-failed", error = format!("{e:#}")))
            }
        }
    }

    fn selected_body_list(&mut self) -> Vec<BodyId> {
        let mut v: Vec<BodyId> = self
            .selected_bodies
            .iter()
            .copied()
            .filter(|b| self.model.geometry.contains_key(*b))
            .collect();
        v.sort();
        if v.is_empty() {
            self.model_status = Some(ducad_i18n::t!("body-tool-need-body"));
        }
        v
    }

    pub fn surface_thicken(&mut self) {
        let Some(id) = self.selected_body_list().first().copied() else {
            return;
        };
        let Some(g) = self.model.geometry.get(id) else {
            return;
        };
        let r = surface::thicken(&g.shape, self.surface_tools.thickness, self.surface_tools.symmetric);
        self.surface_replace("Thicken", id, r);
    }

    pub fn surface_stitch(&mut self) {
        let ids = self.selected_body_list();
        if ids.len() < 2 {
            self.model_status = Some(ducad_i18n::t!("surface-need-two-bodies"));
            return;
        }
        let shapes: Vec<&KernelShape> = ids
            .iter()
            .filter_map(|b| self.model.geometry.get(*b).map(|g| &g.shape))
            .collect();
        match to_geo(surface::stitch(&shapes, 0.01)) {
            Ok(geo) => {
                let msg = ducad_i18n::t!("direct-created", name = "Stitch");
                self.execute_model_command(
                    Box::new(AddSolidCommand::new("Stitch".to_string(), geo)),
                    &msg,
                );
                for id in ids {
                    self.execute_model_command(
                        Box::new(crate::model::DeleteBodyCommand::new(id)),
                        &msg,
                    );
                }
                self.selected_bodies.clear();
                self.model_status = Some(msg);
            }
            Err(e) => self.model_status = Some(ducad_i18n::t!("surface-failed", error = e)),
        }
    }

    pub fn surface_trim(&mut self) {
        let Some(id) = self.selected_body_list().first().copied() else {
            return;
        };
        let Some(g) = self.model.geometry.get(id) else {
            return;
        };
        let n = match self.surface_tools.trim_plane {
            1 => DVec3::Y,
            2 => DVec3::X,
            _ => DVec3::Z,
        };
        let p = n * self.surface_tools.trim_offset;
        let side = if self.surface_tools.trim_keep_positive { 1.0 } else { -1.0 };
        let keep = p + n * side * 1e6;
        let r = surface::trim_by_plane(&g.shape, p, n, keep);
        self.surface_replace("Trim", id, r);
    }

    /// Jendela "Permukaan".
    pub fn show_surface_tools(&mut self, ctx: &egui::Context) {
        if !self.surface_tools.open {
            return;
        }
        let mut run: Option<SurfaceOp> = None;
        let mut open = true;
        let st = &mut self.surface_tools;
        egui::Window::new(ducad_i18n::t!("surface-title"))
            .open(&mut open)
            .collapsible(true)
            .resizable(false)
            .default_width(330.0)
            .show(ctx, |ui| {
                let mm = |ui: &mut egui::Ui, v: &mut f64| {
                    ui.add(egui::DragValue::new(v).speed(0.1).suffix(" mm"));
                };
                ui.strong(ducad_i18n::t!("surface-from-sketch"));
                ui.horizontal(|ui| {
                    mm(ui, &mut st.start);
                    mm(ui, &mut st.end);
                    if ui.button(ducad_i18n::t!("surface-extrude")).clicked() {
                        run = Some(DuCADApp::surface_extrude);
                    }
                });
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut st.revolve_v, false, "u");
                    ui.selectable_value(&mut st.revolve_v, true, "v");
                    ui.add(
                        egui::DragValue::new(&mut st.angle)
                            .speed(1.0)
                            .range(-360.0..=360.0)
                            .suffix(" deg"),
                    );
                    if ui.button(ducad_i18n::t!("surface-revolve")).clicked() {
                        run = Some(DuCADApp::surface_revolve);
                    }
                    if ui
                        .button(ducad_i18n::t!("surface-sweep"))
                        .on_hover_text(ducad_i18n::t!("surface-sweep-hint"))
                        .clicked()
                    {
                        run = Some(DuCADApp::surface_sweep);
                    }
                });
                ui.separator();
                ui.strong(ducad_i18n::t!("surface-from-edges"));
                ui.horizontal(|ui| {
                    ui.label(ducad_i18n::t!("direct-distance"));
                    mm(ui, &mut st.distance);
                    ui.checkbox(&mut st.ruled_tangent, ducad_i18n::t!("surface-tangent"));
                });
                ui.horizontal_wrapped(|ui| {
                    let ops: [(&str, SurfaceOp); 4] = [
                        ("surface-patch", DuCADApp::surface_patch),
                        ("surface-ruled", DuCADApp::surface_ruled),
                        ("surface-extend", DuCADApp::surface_extend),
                        ("surface-boundary", DuCADApp::surface_boundary),
                    ];
                    for (key, f) in ops {
                        if ui.button(ducad_i18n::t!(key)).clicked() {
                            run = Some(f);
                        }
                    }
                });
                ui.separator();
                ui.strong(ducad_i18n::t!("surface-from-faces"));
                ui.horizontal_wrapped(|ui| {
                    let ops: [(&str, SurfaceOp); 6] = [
                        ("surface-offset", DuCADApp::surface_offset),
                        ("surface-copy", DuCADApp::surface_copy),
                        ("surface-reverse", DuCADApp::surface_reverse),
                        ("surface-fair", DuCADApp::surface_fair),
                        ("surface-untrim", DuCADApp::surface_untrim),
                        ("surface-unstitch", DuCADApp::surface_unstitch),
                    ];
                    for (key, f) in ops {
                        if ui.button(ducad_i18n::t!(key)).clicked() {
                            run = Some(f);
                        }
                    }
                });
                ui.separator();
                ui.strong(ducad_i18n::t!("surface-from-bodies"));
                ui.horizontal(|ui| {
                    mm(ui, &mut st.thickness);
                    ui.checkbox(&mut st.symmetric, ducad_i18n::t!("surface-symmetric"));
                    if ui.button(ducad_i18n::t!("surface-thicken")).clicked() {
                        run = Some(DuCADApp::surface_thicken);
                    }
                    if ui.button(ducad_i18n::t!("surface-stitch")).clicked() {
                        run = Some(DuCADApp::surface_stitch);
                    }
                });
                ui.horizontal(|ui| {
                    for (i, n) in ["XY", "XZ", "YZ"].iter().enumerate() {
                        ui.selectable_value(&mut st.trim_plane, i, *n);
                    }
                    mm(ui, &mut st.trim_offset);
                    ui.checkbox(&mut st.trim_keep_positive, "+");
                    if ui.button(ducad_i18n::t!("surface-trim")).clicked() {
                        run = Some(DuCADApp::surface_trim);
                    }
                });
            });
        if let Some(f) = run {
            f(self);
        }
        if !open {
            self.surface_tools.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_sketch::Entity;
    use glam::DVec2;

    #[test]
    fn sketch_line_to_sheet_then_trim_and_thicken() {
        let mut app = DuCADApp::new_for_test();
        app.sketch_mut()
            .entities
            .insert(Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0)));
        app.surface_tools.end = 10.0;
        app.surface_extrude();
        let id = app
            .model
            .doc
            .bodies
            .iter()
            .map(|(id, _)| id)
            .next()
            .expect("sheet dibuat");
        assert!(app.model.geometry.get(id).unwrap().shape.is_sheet());
        app.selected_bodies = [id].into_iter().collect();
        app.surface_tools.trim_offset = 4.0;
        app.surface_tools.trim_keep_positive = false;
        app.surface_trim();
        let a = app.model.geometry.get(id).unwrap().shape.surface_area();
        assert!((a - 40.0).abs() < 0.5, "{a} {:?}", app.model_status);
        app.surface_thicken();
        let v = app.model.geometry.get(id).unwrap().shape.volume().abs();
        assert!((v - 80.0).abs() < 1.0, "{v} {:?}", app.model_status);
    }

    #[test]
    fn tools_without_selection_only_report() {
        let mut app = DuCADApp::new_for_test();
        let ops: [SurfaceOp; 6] = [
            DuCADApp::surface_extrude,
            DuCADApp::surface_patch,
            DuCADApp::surface_offset,
            DuCADApp::surface_thicken,
            DuCADApp::surface_stitch,
            DuCADApp::surface_boundary,
        ];
        for f in ops {
            app.model_status = None;
            f(&mut app);
            assert!(app.model_status.is_some());
        }
        assert!(app.model.doc.bodies.is_empty());
    }
}
