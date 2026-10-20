//! Alat direct editing & solid Fase 2 (PLAN_PARITAS_SOLIDCRAFT §2) di GUI:
//! Offset/Delete/Replace Face, Align, Pipe, Pattern on Path, Web, Emboss
//! sketsa, Boundary Fill, Bounding Solid.
//!
//! Geometri dihitung lewat `ducad_engine::compute::direct` (sumber yang sama
//! dengan op `offset_face`, `delete_face`, … untuk MCP/CLI), lalu masuk undo
//! stack model sebagai `ReplaceGeometryCommand` / `AddSolidCommand`.

use ducad_core::BodyId;
use ducad_engine::compute::direct;
use ducad_kernel::{FaceInfo, PathSegment};
use glam::DVec3;

use crate::app::DuCADApp;
use crate::model::{AddSolidCommand, BodyGeometry, ReplaceGeometryCommand};

/// Alat yang panelnya sedang terbuka.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectTool {
    OffsetFace,
    DeleteFace,
    ReplaceFace,
    Align,
    Pipe,
    PatternPath,
    Web,
    Emboss,
    BoundaryFill,
    BoundingSolid,
}

impl DirectTool {
    pub const ALL: [DirectTool; 10] = [
        DirectTool::OffsetFace,
        DirectTool::DeleteFace,
        DirectTool::ReplaceFace,
        DirectTool::Align,
        DirectTool::Pipe,
        DirectTool::PatternPath,
        DirectTool::Web,
        DirectTool::Emboss,
        DirectTool::BoundaryFill,
        DirectTool::BoundingSolid,
    ];

    pub fn label(self) -> String {
        match self {
            DirectTool::OffsetFace => ducad_i18n::t!("direct-offset-face"),
            DirectTool::DeleteFace => ducad_i18n::t!("direct-delete-face"),
            DirectTool::ReplaceFace => ducad_i18n::t!("direct-replace-face"),
            DirectTool::Align => ducad_i18n::t!("direct-align"),
            DirectTool::Pipe => ducad_i18n::t!("direct-pipe"),
            DirectTool::PatternPath => ducad_i18n::t!("direct-pattern-path"),
            DirectTool::Web => ducad_i18n::t!("direct-web"),
            DirectTool::Emboss => ducad_i18n::t!("direct-emboss"),
            DirectTool::BoundaryFill => ducad_i18n::t!("direct-boundary-fill"),
            DirectTool::BoundingSolid => ducad_i18n::t!("direct-bounding-solid"),
        }
    }

    fn help(self) -> String {
        match self {
            DirectTool::OffsetFace => ducad_i18n::t!("direct-offset-face-help"),
            DirectTool::DeleteFace => ducad_i18n::t!("direct-delete-face-help"),
            DirectTool::ReplaceFace => ducad_i18n::t!("direct-replace-face-help"),
            DirectTool::Align => ducad_i18n::t!("direct-align-help"),
            DirectTool::Pipe => ducad_i18n::t!("direct-pipe-help"),
            DirectTool::PatternPath => ducad_i18n::t!("direct-pattern-path-help"),
            DirectTool::Web => ducad_i18n::t!("direct-web-help"),
            DirectTool::Emboss => ducad_i18n::t!("direct-emboss-help"),
            DirectTool::BoundaryFill => ducad_i18n::t!("direct-boundary-fill-help"),
            DirectTool::BoundingSolid => ducad_i18n::t!("direct-bounding-solid-help"),
        }
    }
}

/// Bidang dunia standar untuk Replace Face (bila tidak ada face kedua).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaneAxis {
    XY,
    XZ,
    YZ,
}

impl PlaneAxis {
    fn normal(self) -> DVec3 {
        match self {
            PlaneAxis::XY => DVec3::Z,
            PlaneAxis::XZ => DVec3::Y,
            PlaneAxis::YZ => DVec3::X,
        }
    }
    fn name(self) -> &'static str {
        match self {
            PlaneAxis::XY => "XY",
            PlaneAxis::XZ => "XZ",
            PlaneAxis::YZ => "YZ",
        }
    }
}

pub struct DirectToolsState {
    pub open: Option<DirectTool>,
    pub distance: f64,
    pub plane: PlaneAxis,
    pub plane_offset: f64,
    pub flip: bool,
    pub gap: f64,
    pub radius: f64,
    pub hollow: bool,
    pub wall: f64,
    pub count: u32,
    pub follow_tangent: bool,
    pub thickness: f64,
    pub depth: f64,
    pub reverse: bool,
    pub deboss: bool,
    pub point: [f64; 3],
    pub cylinder: bool,
    pub margin: f64,
}

impl Default for DirectToolsState {
    fn default() -> Self {
        Self {
            open: None,
            distance: 5.0,
            plane: PlaneAxis::XY,
            plane_offset: 10.0,
            flip: false,
            gap: 0.0,
            radius: 3.0,
            hollow: false,
            wall: 0.5,
            count: 5,
            follow_tangent: false,
            thickness: 2.0,
            depth: 10.0,
            reverse: false,
            deboss: false,
            point: [0.0; 3],
            cylinder: false,
            margin: 0.0,
        }
    }
}

/// Face terpilih pada body tertentu.
#[derive(Debug, Clone)]
pub struct FacePick {
    pub body: BodyId,
    pub index: usize,
    pub info: FaceInfo,
}

/// Rangkai polyline tepi terpilih jadi satu jalur (urutan seleksi; tiap
/// potongan dibalik bila ujungnya yang menyambung).
pub fn chain_edge_polylines(polys: &[Vec<DVec3>]) -> Vec<DVec3> {
    let mut out: Vec<DVec3> = Vec::new();
    for p in polys.iter().filter(|p| p.len() >= 2) {
        let mut seg = p.clone();
        if let Some(&last) = out.last() {
            let (a, b) = (seg[0], seg[seg.len() - 1]);
            if b.distance(last) < a.distance(last) {
                seg.reverse();
            }
        } else if let Some(next) = polys.get(1).filter(|n| n.len() >= 2) {
            // Potongan pertama: arahkan ujungnya ke potongan berikutnya.
            let (n0, n1) = (next[0], next[next.len() - 1]);
            let end = seg[seg.len() - 1];
            let d_end = end.distance(n0).min(end.distance(n1));
            let d_start = seg[0].distance(n0).min(seg[0].distance(n1));
            if d_start < d_end {
                seg.reverse();
            }
        }
        for q in seg {
            if out.last().is_none_or(|l| l.distance(q) > 1e-9) {
                out.push(q);
            }
        }
    }
    out
}

/// Bingkai bidang dari titik + normal (sumbu u/v sembarang yang ortonormal).
fn plane_frame(origin: DVec3, normal: DVec3) -> ducad_engine::PlaneFrame {
    let n = normal.normalize_or_zero();
    let u = n.any_orthonormal_vector();
    let v = n.cross(u);
    ducad_engine::PlaneFrame {
        origin: origin.to_array(),
        u_axis: u.to_array(),
        v_axis: v.to_array(),
        normal: n.to_array(),
    }
}

impl DuCADApp {
    pub fn open_direct_tool(&mut self, tool: DirectTool) {
        self.direct_tools.open = Some(tool);
    }

    /// Face terpilih (urutan seleksi; `active_face` terakhir), dipetakan ke
    /// body terdekat di sepanjang ray.
    pub fn selected_face_picks(&self) -> Vec<FacePick> {
        let rays: Vec<ducad_kernel::PickRay> = self
            .selected_faces
            .iter()
            .copied()
            .chain(self.active_face.as_ref().map(|(_, r, _)| *r))
            .collect();
        let mut out: Vec<FacePick> = Vec::new();
        for ray in rays {
            let mut best: Option<(f64, BodyId, usize)> = None;
            for (id, g) in self.model.geometry.iter() {
                let Some(hit) = ducad_kernel::pick_face_details(&g.shape, ray) else {
                    continue;
                };
                let Some(fi) = hit.face_index else { continue };
                let d = DVec3::from(hit.hit_point).distance(DVec3::from(ray.origin));
                if best.is_none_or(|b| d < b.0) {
                    best = Some((d, id, fi));
                }
            }
            let Some((_, body, index)) = best else { continue };
            if out.iter().any(|p| p.body == body && p.index == index) {
                continue;
            }
            let info = self
                .model
                .geometry
                .get(body)
                .and_then(|g| ducad_kernel::enumerate_faces(&g.shape).into_iter().nth(index));
            if let Some(info) = info {
                out.push(FacePick { body, index, info });
            }
        }
        out
    }

    fn body_label(&self, id: BodyId) -> String {
        self.model
            .doc
            .bodies
            .get(id)
            .map(|b| b.name.clone())
            .unwrap_or_else(|| "Body".into())
    }

    fn status_err(&mut self, label: &str, e: &ducad_engine::OpError) {
        self.model_status = Some(crate::modeling::operations::op_status(label, e));
    }

    fn replace_geometry(&mut self, label: &'static str, id: BodyId, geo: BodyGeometry) {
        let body = self.body_label(id);
        let msg = ducad_i18n::t!("direct-done", tool = label, body = body.as_str());
        self.execute_model_command(Box::new(ReplaceGeometryCommand::new(label, id, geo)), &msg);
        self.round_history.remove(&id);
        self.active_face = None;
        self.selected_faces.clear();
        self.model_status = Some(msg);
    }

    fn add_body(&mut self, name: String, geo: BodyGeometry) {
        let msg = ducad_i18n::t!("direct-created", name = name.as_str());
        self.execute_model_command(Box::new(AddSolidCommand::new(name, geo)), &msg);
        self.model_status = Some(msg);
    }

    fn first_selected_body(&mut self) -> Option<BodyId> {
        let id = self
            .selected_bodies
            .iter()
            .copied()
            .find(|b| self.model.geometry.contains_key(*b));
        if id.is_none() {
            self.model_status = Some(ducad_i18n::t!("body-tool-need-body"));
        }
        id
    }

    /// Face terpilih dari SATU body (offset/delete).
    fn faces_on_one_body(&mut self) -> Option<(BodyId, Vec<usize>)> {
        let picks = self.selected_face_picks();
        let Some(first) = picks.first().map(|p| p.body) else {
            self.model_status = Some(ducad_i18n::t!("direct-need-face"));
            return None;
        };
        if picks.iter().any(|p| p.body != first) {
            self.model_status = Some(ducad_i18n::t!("direct-one-body"));
            return None;
        }
        Some((first, picks.iter().map(|p| p.index).collect()))
    }

    pub fn direct_offset_faces(&mut self, distance: f64) {
        let Some((id, faces)) = self.faces_on_one_body() else { return };
        let Some(g) = self.model.geometry.get(id) else { return };
        match direct::offset_face(&g.shape, &faces, distance) {
            Ok(geo) => self.replace_geometry("Offset Face", id, geo),
            Err(e) => self.status_err("Offset Face", &e),
        }
    }

    pub fn direct_delete_faces(&mut self) {
        let Some((id, faces)) = self.faces_on_one_body() else { return };
        let Some(g) = self.model.geometry.get(id) else { return };
        match direct::delete_face(&g.shape, &faces) {
            Ok(geo) => self.replace_geometry("Delete Face", id, geo),
            Err(e) => self.status_err("Delete Face", &e),
        }
    }

    /// Face pertama diganti dengan bidang face kedua (bila ada) atau bidang
    /// standar + offset.
    pub fn direct_replace_face(&mut self, plane: PlaneAxis, offset: f64) {
        let picks = self.selected_face_picks();
        let Some(target) = picks.first().cloned() else {
            self.model_status = Some(ducad_i18n::t!("direct-need-face"));
            return;
        };
        let frame = match picks.get(1) {
            Some(p) if p.info.kind == ducad_kernel::SurfaceKind::Plane => {
                plane_frame(DVec3::from(p.info.centroid), DVec3::from(p.info.normal))
            }
            Some(_) => {
                self.model_status = Some(ducad_i18n::t!("direct-replace-need-planar"));
                return;
            }
            None => plane_frame(plane.normal() * offset, plane.normal()),
        };
        let Some(g) = self.model.geometry.get(target.body) else { return };
        match direct::replace_face(&g.shape, target.index, &frame) {
            Ok(geo) => self.replace_geometry("Replace Face", target.body, geo),
            Err(e) => self.status_err("Replace Face", &e),
        }
    }

    /// Face pertama (body yang dipindah) ke face kedua (body lain).
    pub fn direct_align(&mut self, flip: bool, gap: f64) {
        let picks = self.selected_face_picks();
        let (Some(from), Some(to)) = (picks.first(), picks.get(1)) else {
            self.model_status = Some(ducad_i18n::t!("direct-align-need-two"));
            return;
        };
        if from.body == to.body {
            self.model_status = Some(ducad_i18n::t!("direct-align-need-two"));
            return;
        }
        let (Some(a), Some(b)) = (
            direct::AlignFace::from_info(&from.info),
            direct::AlignFace::from_info(&to.info),
        ) else {
            self.model_status = Some(ducad_i18n::t!("direct-align-kind"));
            return;
        };
        let id = from.body;
        let Some(g) = self.model.geometry.get(id) else { return };
        match direct::align(&g.shape, a, b, flip, gap) {
            Ok(geo) => self.replace_geometry("Align", id, geo),
            Err(e) => self.status_err("Align", &e),
        }
    }

    /// Jalur dari tepi terpilih (dirangkai).
    fn selected_edge_path(&mut self) -> Option<Vec<PathSegment>> {
        let polys: Vec<Vec<DVec3>> = self
            .selected_edges
            .iter()
            .map(|e| e.polyline.iter().map(|p| DVec3::new(p.0, p.1, p.2)).collect())
            .collect();
        let pts = chain_edge_polylines(&polys);
        if pts.len() < 2 {
            self.model_status = Some(ducad_i18n::t!("direct-need-edges"));
            return None;
        }
        Some(vec![PathSegment::Polyline(
            pts.iter().map(|p| p.to_array()).collect(),
        )])
    }

    pub fn direct_pipe(&mut self, r: f64, wall: Option<f64>) {
        let Some(path) = self.selected_edge_path() else { return };
        match direct::pipe(&path, r, wall) {
            Ok(geo) => self.add_body("Pipe".into(), geo),
            Err(e) => self.status_err("Pipe", &e),
        }
    }

    pub fn direct_pattern_path(&mut self, count: u32, follow: bool) {
        let Some(path) = self.selected_edge_path() else { return };
        let Some(id) = self.first_selected_body() else { return };
        let Some(g) = self.model.geometry.get(id) else { return };
        match direct::path_pattern(&g.shape, &path, count as usize, follow) {
            Ok(copies) => {
                let base = self.body_label(id);
                let named: Vec<(String, BodyGeometry)> = copies
                    .into_iter()
                    .enumerate()
                    .map(|(k, g)| (format!("{base} ({})", k + 2), g))
                    .collect();
                let msg = ducad_i18n::t!("direct-pattern-done", count = named.len() + 1);
                self.execute_model_command(
                    Box::new(crate::model::AddMultipleSolidsCommand::new(
                        "Pattern on Path",
                        named,
                    )),
                    &msg,
                );
                self.model_status = Some(msg);
            }
            Err(e) => self.status_err("Pattern on Path", &e),
        }
    }

    /// Garis sketsa terpilih (atau semua garis sketsa aktif) → dinding web
    /// pada body face aktif / body terpilih.
    pub fn direct_web(&mut self, thickness: f64, depth: f64, reverse: bool) {
        let Some(id) = self
            .active_face
            .as_ref()
            .map(|(b, _, _)| *b)
            .or_else(|| self.selected_bodies.iter().copied().next())
        else {
            self.model_status = Some(ducad_i18n::t!("body-tool-need-body"));
            return;
        };
        let plane = self.active_plane;
        let to3 = |p: glam::DVec2| DVec3::from(plane.to_world_f64((p.x, p.y), 0.0));
        let lines: Vec<(DVec3, DVec3)> = self
            .sketch()
            .entities
            .iter()
            .filter(|(eid, e)| {
                !e.is_construction() && (self.selected.is_empty() || self.selected.contains(eid))
            })
            .filter_map(|(_, e)| match e {
                ducad_sketch::Entity::Line { start, end, .. } => Some((to3(*start), to3(*end))),
                _ => None,
            })
            .collect();
        let n = DVec3::new(
            plane.normal.x as f64,
            plane.normal.y as f64,
            plane.normal.z as f64,
        );
        let up = if reverse { -n } else { n };
        let Some(g) = self.model.geometry.get(id) else { return };
        match direct::web(&g.shape, &lines, up, thickness, depth) {
            Ok(geo) => self.replace_geometry("Web", id, geo),
            Err(e) => self.status_err("Web", &e),
        }
    }

    pub fn direct_boundary_fill(&mut self, point: [f64; 3]) {
        let ids: Vec<BodyId> = self
            .selected_bodies
            .iter()
            .copied()
            .filter(|b| self.model.geometry.contains_key(*b))
            .collect();
        if ids.is_empty() {
            self.model_status = Some(ducad_i18n::t!("body-tool-need-body"));
            return;
        }
        let shapes: Vec<&ducad_kernel::KernelShape> = ids
            .iter()
            .filter_map(|b| self.model.geometry.get(*b).map(|g| &g.shape))
            .collect();
        match direct::boundary_fill(&shapes, &[DVec3::from(point)]) {
            Ok(geo) => self.add_body("Boundary Fill".into(), geo),
            Err(e) => self.status_err("Boundary Fill", &e),
        }
    }

    pub fn direct_bounding_solid(&mut self, cylinder: bool, margin: f64) {
        let Some(id) = self.first_selected_body() else { return };
        let Some(g) = self.model.geometry.get(id) else { return };
        match direct::bounding_solid(&g.shape, cylinder, DVec3::Z, margin) {
            Ok(geo) => {
                let name = format!("{} (Bounding)", self.body_label(id));
                self.add_body(name, geo)
            }
            Err(e) => self.status_err("Bounding Solid", &e),
        }
    }

    /// Jendela alat direct editing (satu jendela; alat dipilih di atas).
    pub fn show_direct_tools(&mut self, ctx: &egui::Context) {
        let Some(mut tool) = self.direct_tools.open else { return };
        let mut open = true;
        let mut apply = false;
        let face_count = self.selected_face_picks().len();
        let edge_count = self.selected_edges.len();
        let body_count = self.selected_bodies.len();
        let st = &mut self.direct_tools;
        egui::Window::new(ducad_i18n::t!("direct-title"))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_width(320.0)
            .show(ctx, |ui| {
                egui::ComboBox::from_id_salt("direct-tool")
                    .selected_text(tool.label())
                    .show_ui(ui, |ui| {
                        for t in DirectTool::ALL {
                            ui.selectable_value(&mut tool, t, t.label());
                        }
                    });
                ui.label(egui::RichText::new(tool.help()).small());
                ui.label(
                    egui::RichText::new(ducad_i18n::t!(
                        "direct-selection",
                        faces = face_count,
                        edges = edge_count,
                        bodies = body_count
                    ))
                    .small()
                    .weak(),
                );
                ui.separator();
                let mm = |ui: &mut egui::Ui, v: &mut f64, min: f64| {
                    ui.add(
                        egui::DragValue::new(v)
                            .speed(0.1)
                            .range(min..=1.0e6)
                            .suffix(" mm"),
                    );
                };
                egui::Grid::new("direct-grid").num_columns(2).show(ui, |ui| match tool {
                    DirectTool::OffsetFace => {
                        ui.label(ducad_i18n::t!("direct-distance"));
                        ui.add(
                            egui::DragValue::new(&mut st.distance)
                                .speed(0.1)
                                .range(-1.0e6..=1.0e6)
                                .suffix(" mm"),
                        );
                        ui.end_row();
                    }
                    DirectTool::DeleteFace => {}
                    DirectTool::ReplaceFace => {
                        ui.label(ducad_i18n::t!("direct-plane"));
                        ui.horizontal(|ui| {
                            for p in [PlaneAxis::XY, PlaneAxis::XZ, PlaneAxis::YZ] {
                                ui.selectable_value(&mut st.plane, p, p.name());
                            }
                        });
                        ui.end_row();
                        ui.label(ducad_i18n::t!("direct-plane-offset"));
                        ui.add(
                            egui::DragValue::new(&mut st.plane_offset)
                                .speed(0.1)
                                .suffix(" mm"),
                        );
                        ui.end_row();
                    }
                    DirectTool::Align => {
                        ui.label(ducad_i18n::t!("direct-flip"));
                        ui.checkbox(&mut st.flip, "");
                        ui.end_row();
                        ui.label(ducad_i18n::t!("direct-gap"));
                        ui.add(egui::DragValue::new(&mut st.gap).speed(0.1).suffix(" mm"));
                        ui.end_row();
                    }
                    DirectTool::Pipe => {
                        ui.label(ducad_i18n::t!("prim-radius"));
                        mm(ui, &mut st.radius, 0.001);
                        ui.end_row();
                        ui.label(ducad_i18n::t!("direct-hollow"));
                        ui.checkbox(&mut st.hollow, "");
                        ui.end_row();
                        if st.hollow {
                            ui.label(ducad_i18n::t!("direct-wall"));
                            mm(ui, &mut st.wall, 0.001);
                            ui.end_row();
                        }
                    }
                    DirectTool::PatternPath => {
                        ui.label(ducad_i18n::t!("direct-count"));
                        ui.add(egui::DragValue::new(&mut st.count).range(2..=1000));
                        ui.end_row();
                        ui.label(ducad_i18n::t!("direct-follow-tangent"));
                        ui.checkbox(&mut st.follow_tangent, "");
                        ui.end_row();
                    }
                    DirectTool::Web => {
                        ui.label(ducad_i18n::t!("direct-thickness"));
                        mm(ui, &mut st.thickness, 0.001);
                        ui.end_row();
                        ui.label(ducad_i18n::t!("direct-depth"));
                        mm(ui, &mut st.depth, 0.001);
                        ui.end_row();
                        ui.label(ducad_i18n::t!("direct-reverse"));
                        ui.checkbox(&mut st.reverse, "");
                        ui.end_row();
                    }
                    DirectTool::Emboss => {
                        ui.label(ducad_i18n::t!("direct-depth"));
                        mm(ui, &mut st.depth, 0.001);
                        ui.end_row();
                        ui.label(ducad_i18n::t!("direct-deboss"));
                        ui.checkbox(&mut st.deboss, "");
                        ui.end_row();
                    }
                    DirectTool::BoundaryFill => {
                        ui.label(ducad_i18n::t!("direct-point"));
                        ui.horizontal(|ui| {
                            for v in st.point.iter_mut() {
                                ui.add(egui::DragValue::new(v).speed(0.5));
                            }
                        });
                        ui.end_row();
                    }
                    DirectTool::BoundingSolid => {
                        ui.label(ducad_i18n::t!("direct-cylinder"));
                        ui.checkbox(&mut st.cylinder, "");
                        ui.end_row();
                        ui.label(ducad_i18n::t!("direct-margin"));
                        mm(ui, &mut st.margin, 0.0);
                        ui.end_row();
                    }
                });
                ui.add_space(4.0);
                apply = ui.button(ducad_i18n::t!("direct-apply")).clicked();
            });
        self.direct_tools.open = open.then_some(tool);
        if !apply {
            return;
        }
        let s = &self.direct_tools;
        let (distance, plane, plane_offset, flip, gap) =
            (s.distance, s.plane, s.plane_offset, s.flip, s.gap);
        let (radius, wall) = (s.radius, s.hollow.then_some(s.wall));
        let (count, follow, thickness, depth, reverse, deboss) = (
            s.count,
            s.follow_tangent,
            s.thickness,
            s.depth,
            s.reverse,
            s.deboss,
        );
        let (point, cylinder, margin) = (s.point, s.cylinder, s.margin);
        match tool {
            DirectTool::OffsetFace => self.direct_offset_faces(distance),
            DirectTool::DeleteFace => self.direct_delete_faces(),
            DirectTool::ReplaceFace => self.direct_replace_face(plane, plane_offset),
            DirectTool::Align => self.direct_align(flip, gap),
            DirectTool::Pipe => self.direct_pipe(radius, wall),
            DirectTool::PatternPath => self.direct_pattern_path(count, follow),
            DirectTool::Web => self.direct_web(thickness, depth, reverse),
            DirectTool::Emboss => self.apply_emboss_deboss(depth, deboss),
            DirectTool::BoundaryFill => self.direct_boundary_fill(point),
            DirectTool::BoundingSolid => self.direct_bounding_solid(cylinder, margin),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PickedEdge;

    fn app_with_box() -> DuCADApp {
        let mut app = DuCADApp::new_for_test();
        app.create_primitive(ducad_ui::PrimitiveKind::Box, [10.0, 10.0, 10.0], [0.0; 3], false);
        app
    }

    fn first_body(app: &DuCADApp) -> BodyId {
        app.model.geometry.iter().next().map(|(id, _)| id).unwrap()
    }

    fn top_ray() -> ducad_kernel::PickRay {
        ducad_kernel::PickRay {
            origin: (5.0, 5.0, 100.0),
            dir: (0.0, 0.0, -1.0),
        }
    }

    fn volume(app: &DuCADApp, id: BodyId) -> f64 {
        app.model.geometry.get(id).unwrap().shape.volume()
    }

    #[test]
    fn chain_reverses_pieces_to_connect() {
        let a = vec![DVec3::new(10.0, 0.0, 0.0), DVec3::ZERO];
        let b = vec![DVec3::new(10.0, 10.0, 0.0), DVec3::new(10.0, 0.0, 0.0)];
        let out = chain_edge_polylines(&[a, b]);
        assert_eq!(
            out,
            vec![DVec3::ZERO, DVec3::new(10.0, 0.0, 0.0), DVec3::new(10.0, 10.0, 0.0)]
        );
    }

    #[test]
    fn offset_selected_top_face() {
        let mut app = app_with_box();
        let id = first_body(&app);
        app.selected_faces = vec![top_ray()];
        app.direct_offset_faces(5.0);
        assert!((volume(&app, id) - 1500.0).abs() < 1.0, "{} {:?}", volume(&app, id), app.model_status);
        assert!(app.selected_faces.is_empty());
    }

    #[test]
    fn replace_face_with_world_plane() {
        let mut app = app_with_box();
        let id = first_body(&app);
        app.selected_faces = vec![top_ray()];
        app.direct_replace_face(PlaneAxis::XY, 20.0);
        assert!((volume(&app, id) - 2000.0).abs() < 2.0, "{} {:?}", volume(&app, id), app.model_status);
    }

    #[test]
    fn tools_without_selection_only_set_status() {
        let mut app = app_with_box();
        let id = first_body(&app);
        app.direct_offset_faces(5.0);
        app.direct_delete_faces();
        app.direct_align(false, 0.0);
        app.direct_pipe(2.0, None);
        app.direct_boundary_fill([5.0; 3]);
        assert!(app.model_status.is_some());
        assert!((volume(&app, id) - 1000.0).abs() < 1e-6);
    }

    #[test]
    fn pipe_from_selected_edge_and_bounding_box() {
        let mut app = app_with_box();
        let before = app.model.geometry.len();
        app.selected_edges = vec![PickedEdge {
            ray: top_ray(),
            polyline: vec![(0.0, 20.0, 0.0), (0.0, 20.0, 30.0)],
        }];
        app.direct_pipe(2.0, Some(0.5));
        assert_eq!(app.model.geometry.len(), before + 1, "{:?}", app.model_status);
        let id = first_body(&app);
        app.selected_bodies.insert(id);
        app.direct_bounding_solid(false, 1.0);
        assert_eq!(app.model.geometry.len(), before + 2, "{:?}", app.model_status);
    }
}
