//! Inspeksi & analisis Fase 5 (PLAN_PARITAS_SOLIDCRAFT §5):
//! clearance (5.1), radius minimum + peta kurvatur + sisir kurvatur sketsa
//! (5.2), kontinuitas G0–G2 antar face (5.3), siluet / isocurve /
//! aksesibilitas (5.4), daftar ulir (5.5). Hasil ditampilkan di jendela
//! "Inspeksi" dan sebagai overlay viewport.

use ducad_core::BodyId;
use ducad_kernel::{FaceInfo, SurfaceKind};
use glam::{DVec2, DVec3, Vec3};

use crate::app::DuCADApp;
use crate::viewport::world_to_screen_pos;

/// Hasil clearance dua body.
#[derive(Debug, Clone)]
pub struct ClearanceHit {
    pub names: (String, String),
    pub distance: f64,
    pub a: DVec3,
    pub b: DVec3,
}

/// Satu baris daftar ulir.
#[derive(Debug, Clone, PartialEq)]
pub struct ThreadRow {
    pub body: String,
    pub designation: String,
    pub detail: String,
}

/// Status jendela inspeksi.
pub struct InspectToolsState {
    pub open: bool,
    pub clearance: Option<ClearanceHit>,
    pub min_radius: Option<(String, f64, DVec3)>,
    pub curvature: ducad_render::CurvatureConfig,
    pub comb: bool,
    pub comb_scale: f64,
    pub continuity: Vec<ducad_kernel::SeamContinuity>,
    /// Segmen siluet dalam piksel viewport.
    pub silhouette: Vec<([f32; 2], [f32; 2])>,
    pub isocurves: Vec<Vec<DVec3>>,
    pub iso_count: usize,
    pub access_dir: [f64; 3],
    /// Titik sampel (posisi, terjangkau?).
    pub access: Vec<(DVec3, bool)>,
    pub access_ratio: Option<f64>,
    pub threads: Vec<ThreadRow>,
}

impl Default for InspectToolsState {
    fn default() -> Self {
        Self {
            open: false,
            clearance: None,
            min_radius: None,
            curvature: ducad_render::CurvatureConfig::default(),
            comb: false,
            comb_scale: 20.0,
            continuity: Vec::new(),
            silhouette: Vec::new(),
            isocurves: Vec::new(),
            iso_count: 8,
            access_dir: [0.0, 0.0, -1.0],
            access: Vec::new(),
            access_ratio: None,
            threads: Vec::new(),
        }
    }
}

/// Kurvatur bertanda di titik tengah tiga titik (1/mm).
fn curvature3(a: DVec2, b: DVec2, c: DVec2) -> f64 {
    let (ab, bc, ca) = (b - a, c - b, a - c);
    let den = ab.length() * bc.length() * ca.length();
    if den < 1e-12 {
        return 0.0;
    }
    2.0 * ab.perp_dot(bc) / den
}

/// Sisir kurvatur (Paritas 5.2) satu rangkaian titik: (titik, ujung sisir).
pub fn comb_for_polyline(pts: &[DVec2], scale: f64) -> Vec<(DVec2, DVec2)> {
    let mut out = Vec::new();
    for w in pts.windows(3) {
        let k = curvature3(w[0], w[1], w[2]);
        let t = (w[2] - w[0]).normalize_or_zero();
        let n = DVec2::new(-t.y, t.x);
        out.push((w[1], w[1] - n * k * scale));
    }
    out
}

impl DuCADApp {
    fn body_name(&self, id: BodyId) -> String {
        self.model
            .doc
            .bodies
            .get(id)
            .map(|b| b.name.clone())
            .unwrap_or_default()
    }

    fn visible_bodies(&self) -> Vec<BodyId> {
        self.model
            .doc
            .bodies
            .iter()
            .filter(|(id, b)| b.visible && self.model.geometry.contains_key(*id))
            .map(|(id, _)| id)
            .collect()
    }

    /// Body yang dianalisis: terpilih, atau body face aktif, atau satu-satunya.
    fn inspect_target(&self) -> Option<BodyId> {
        self.selected_bodies
            .iter()
            .copied()
            .next()
            .or_else(|| self.active_face.as_ref().map(|(b, _, _)| *b))
            .or_else(|| {
                let v = self.visible_bodies();
                (v.len() == 1).then(|| v[0])
            })
    }

    /// 5.1 Clearance dua body terpilih.
    pub fn measure_clearance(&mut self) {
        let mut ids: Vec<BodyId> = self.selected_bodies.iter().copied().collect();
        ids.sort();
        let (Some(a), Some(b)) = (ids.first().copied(), ids.get(1).copied()) else {
            self.model_status = Some(ducad_i18n::t!("inspect-need-two"));
            return;
        };
        let (Some(ga), Some(gb)) = (self.model.geometry.get(a), self.model.geometry.get(b)) else {
            return;
        };
        match ducad_kernel::check_clearance(&ga.shape, &gb.shape, 0.0) {
            Ok(r) => {
                let hit = ClearanceHit {
                    names: (self.body_name(a), self.body_name(b)),
                    distance: r.distance,
                    a: DVec3::new(r.point_a.0, r.point_a.1, r.point_a.2),
                    b: DVec3::new(r.point_b.0, r.point_b.1, r.point_b.2),
                };
                self.model_status = Some(ducad_i18n::t!(
                    "inspect-clearance-result",
                    a = hit.names.0.as_str(),
                    b = hit.names.1.as_str(),
                    d = format!("{:.3}", hit.distance)
                ));
                self.inspect_tools.clearance = Some(hit);
            }
            Err(e) => self.model_status = Some(e.to_string()),
        }
    }

    /// 5.2 Radius terkecil (silinder/kerucut/bola) di body terlihat.
    pub fn find_min_radius(&mut self) {
        let mut best: Option<(String, f64, DVec3)> = None;
        for id in self.visible_bodies() {
            let Some(g) = self.model.geometry.get(id) else {
                continue;
            };
            for f in ducad_kernel::enumerate_faces(&g.shape) {
                let Some(r) = f.radius.filter(|r| r.is_finite() && *r > 1e-9) else {
                    continue;
                };
                if best.as_ref().is_none_or(|b| r < b.1) {
                    let at = f
                        .boundary
                        .first()
                        .map(|p| DVec3::from(*p))
                        .unwrap_or_else(|| DVec3::from(f.centroid));
                    best = Some((self.body_name(id), r, at));
                }
            }
        }
        self.model_status = Some(match &best {
            Some((n, r, _)) => {
                ducad_i18n::t!("inspect-min-radius", body = n.as_str(), r = format!("{r:.3}"))
            }
            None => ducad_i18n::t!("inspect-no-radius"),
        });
        self.inspect_tools.min_radius = best;
    }

    /// 5.3 Kontinuitas sambungan body target.
    pub fn run_continuity(&mut self) {
        let Some(id) = self.inspect_target() else {
            self.model_status = Some(ducad_i18n::t!("body-tool-need-body"));
            return;
        };
        let Some(g) = self.model.geometry.get(id) else {
            return;
        };
        match ducad_kernel::seam_continuity(&g.shape, 1e-3, 1.0, 0.01) {
            Ok(seams) => {
                let count = |c| seams.iter().filter(|s| s.class == c).count();
                self.model_status = Some(ducad_i18n::t!(
                    "inspect-continuity-result",
                    g0 = count(ducad_kernel::Continuity::G0),
                    g1 = count(ducad_kernel::Continuity::G1),
                    g2 = count(ducad_kernel::Continuity::G2),
                    gap = count(ducad_kernel::Continuity::Gap)
                ));
                self.inspect_tools.continuity = seams;
            }
            Err(e) => self.model_status = Some(e.to_string()),
        }
    }

    /// 5.4 Garis siluet sudut pandang sekarang.
    pub fn compute_silhouette(&mut self) {
        match self.capture_view_snapshot() {
            Ok(snap) => {
                self.inspect_tools.silhouette = snap
                    .segments
                    .iter()
                    .filter(|s| s.kind == ducad_kernel::HlrLineKind::Silhouette)
                    .map(|s| (s.start, s.end))
                    .collect();
                self.model_status = Some(ducad_i18n::t!(
                    "inspect-silhouette-result",
                    count = self.inspect_tools.silhouette.len()
                ));
            }
            Err(msg) => self.model_status = Some(msg),
        }
    }

    /// Info face aktif + titik mesh face-nya.
    fn active_face_info(&self) -> Option<(FaceInfo, Vec<DVec3>)> {
        let (body, ray, _) = self.active_face.as_ref()?;
        let g = self.model.geometry.get(*body)?;
        let hit = ducad_kernel::pick_face_details(&g.shape, *ray)?;
        let fi = hit.face_index?;
        let info = ducad_kernel::enumerate_faces(&g.shape).into_iter().nth(fi)?;
        let mesh = &g.mesh;
        let pts: Vec<DVec3> = mesh
            .face_ranges
            .get(fi)
            .map(|r| {
                (r.start as usize..r.end as usize)
                    .filter_map(|k| mesh.indices.get(k))
                    .filter_map(|i| mesh.positions.get(*i as usize))
                    .map(|p| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64))
                    .collect()
            })
            .unwrap_or_default();
        Some((info, pts))
    }

    /// 5.4 Isocurve face aktif (datar: grid; silinder: garis sejajar sumbu
    /// + lingkaran), dibatasi bentuk face.
    pub fn compute_isocurves(&mut self) {
        let Some((info, pts)) = self.active_face_info() else {
            self.model_status = Some(ducad_i18n::t!("direct-need-face"));
            return;
        };
        let n = self.inspect_tools.iso_count.clamp(2, 64);
        let curves = match info.kind {
            SurfaceKind::Plane => iso_plane(&info, n),
            SurfaceKind::Cylinder => iso_cylinder(&info, &pts, n),
            _ => Vec::new(),
        };
        self.model_status = Some(if curves.is_empty() {
            ducad_i18n::t!("inspect-iso-unsupported")
        } else {
            ducad_i18n::t!("inspect-iso-result", count = curves.len())
        });
        self.inspect_tools.isocurves = curves;
    }

    /// 5.4 Aksesibilitas body target dari arah alat (arah gerak alat, mis.
    /// −Z = dari atas): titik face terjangkau bila menghadap alat dan sinar ke
    /// arah datangnya alat tidak terhalang body terlihat mana pun.
    pub fn compute_accessibility(&mut self) {
        let Some(id) = self.inspect_target() else {
            self.model_status = Some(ducad_i18n::t!("body-tool-need-body"));
            return;
        };
        let dir = DVec3::from(self.inspect_tools.access_dir).normalize_or_zero();
        if dir == DVec3::ZERO || !dir.is_finite() {
            return;
        }
        let meshes: Vec<&ducad_kernel::KernelMesh> = self
            .visible_bodies()
            .into_iter()
            .filter_map(|b| self.model.geometry.get(b).map(|g| g.mesh.as_ref()))
            .collect();
        let merged = ducad_kernel::KernelMesh::merge(&meshes);
        let Some(rc) = ducad_kernel::MeshRaycaster::new(&merged) else {
            return;
        };
        let Some(g) = self.model.geometry.get(id) else {
            return;
        };
        let mesh = &g.mesh;
        let tri_n = mesh.indices.len() / 3;
        let step = (tri_n / 3000).max(1);
        let back = -dir;
        let (mut ok_area, mut total_area) = (0.0, 0.0);
        let mut out = Vec::new();
        for t in (0..tri_n).step_by(step) {
            let p = |k: usize| {
                let i = *mesh.indices.get(t * 3 + k)? as usize;
                mesh.positions
                    .get(i)
                    .map(|v| DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64))
            };
            let (Some(a), Some(b), Some(c)) = (p(0), p(1), p(2)) else {
                continue;
            };
            let cr = (b - a).cross(c - a);
            let area = cr.length() * 0.5;
            if area < 1e-12 {
                continue;
            }
            let n = cr / (2.0 * area);
            let ctr = (a + b + c) / 3.0;
            let facing = n.dot(back) > 1e-3;
            let origin = ctr + n * 1e-3 + back * 1e-3;
            let blocked = facing
                && rc
                    .closest(origin.as_vec3().to_array(), back.as_vec3().to_array())
                    .is_some();
            let ok = facing && !blocked;
            total_area += area;
            if ok {
                ok_area += area;
            }
            out.push((ctr, ok));
        }
        let ratio = if total_area > 0.0 { ok_area / total_area } else { 0.0 };
        self.model_status = Some(ducad_i18n::t!(
            "inspect-access-result",
            pct = format!("{:.1}", ratio * 100.0)
        ));
        self.inspect_tools.access = out;
        self.inspect_tools.access_ratio = Some(ratio);
    }

    /// 5.5 Daftar ulir (op thread) dan lubang tap di desain.
    pub fn list_threads(&mut self) {
        self.sync_agent_meta();
        let mut rows: Vec<ThreadRow> = Vec::new();
        for (body, notes) in &self.agent_meta.threads {
            for n in notes {
                let kind = if n.cosmetic {
                    ducad_i18n::t!("inspect-thread-cosmetic")
                } else {
                    ducad_i18n::t!("inspect-thread-modeled")
                };
                rows.push(ThreadRow {
                    body: body.clone(),
                    designation: n.designation.clone(),
                    detail: ducad_i18n::t!(
                        "inspect-thread-detail",
                        len = format!("{:.1}", n.length),
                        hand = if n.left_handed { "LH" } else { "RH" },
                        kind = kind.as_str()
                    ),
                });
            }
        }
        for op in &self.agent_meta.design.oplog {
            if let ducad_engine::ops::Op::Hole {
                body,
                spec:
                    ducad_engine::ops::HoleSpecRef::Iso {
                        iso,
                        kind: ducad_engine::ops::HoleKindSpec::Tapped,
                        ..
                    },
                at,
                at_world,
                ..
            } = op
            {
                rows.push(ThreadRow {
                    body: body.clone(),
                    designation: iso.clone(),
                    detail: ducad_i18n::t!(
                        "inspect-thread-tapped",
                        count = at.len() + at_world.len()
                    ),
                });
            }
        }
        self.model_status = Some(ducad_i18n::t!("inspect-threads-result", count = rows.len()));
        self.inspect_tools.threads = rows;
    }

    /// Overlay hasil inspeksi di viewport.
    pub fn paint_inspect_overlay(&self, ui: &egui::Ui, rect: egui::Rect) {
        let st = &self.inspect_tools;
        if !st.open {
            return;
        }
        let painter = ui.painter_at(rect);
        let px = |p: DVec3| world_to_screen_pos(&self.camera, rect, p.as_vec3());
        let font = egui::FontId::proportional(11.0);
        if let Some(c) = &st.clearance {
            if let (Some(a), Some(b)) = (px(c.a), px(c.b)) {
                let col = egui::Color32::from_rgb(10, 132, 255);
                painter.line_segment([a, b], egui::Stroke::new(2.0, col));
                painter.circle_filled(a, 3.0, col);
                painter.circle_filled(b, 3.0, col);
                painter.text(
                    a.lerp(b, 0.5),
                    egui::Align2::LEFT_BOTTOM,
                    format!("{:.3} mm", c.distance),
                    font.clone(),
                    col,
                );
            }
        }
        if let Some((_, r, at)) = &st.min_radius {
            if let Some(p) = px(*at) {
                let col = egui::Color32::from_rgb(255, 69, 58);
                painter.circle_stroke(p, 8.0, egui::Stroke::new(2.0, col));
                painter.text(
                    p + egui::vec2(10.0, -10.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("R {r:.2}"),
                    font.clone(),
                    col,
                );
            }
        }
        for s in &st.continuity {
            let col = match s.class {
                ducad_kernel::Continuity::G2 => egui::Color32::from_rgb(48, 209, 88),
                ducad_kernel::Continuity::G1 => egui::Color32::from_rgb(255, 214, 10),
                ducad_kernel::Continuity::G0 => egui::Color32::from_rgb(255, 149, 0),
                ducad_kernel::Continuity::Gap => egui::Color32::from_rgb(255, 69, 58),
            };
            let line: Vec<egui::Pos2> = s.points.iter().filter_map(|p| px(DVec3::from(*p))).collect();
            painter.add(egui::Shape::line(line, egui::Stroke::new(2.0, col)));
            if let Some(m) = px(DVec3::from(s.mid)) {
                painter.text(m, egui::Align2::CENTER_BOTTOM, s.class.label(), font.clone(), col);
            }
        }
        let sil = egui::Color32::from_rgb(191, 90, 242);
        for (a, b) in &st.silhouette {
            let p = |q: &[f32; 2]| rect.min + egui::vec2(q[0], q[1]);
            painter.line_segment([p(a), p(b)], egui::Stroke::new(2.0, sil));
        }
        let iso = egui::Color32::from_rgb(100, 210, 255);
        for c in &st.isocurves {
            let line: Vec<egui::Pos2> = c.iter().filter_map(|p| px(*p)).collect();
            painter.add(egui::Shape::line(line, egui::Stroke::new(1.0, iso)));
        }
        for (p, ok) in &st.access {
            if let Some(s) = px(*p) {
                let col = if *ok {
                    egui::Color32::from_rgb(48, 209, 88)
                } else {
                    egui::Color32::from_rgb(255, 69, 58)
                };
                painter.circle_filled(s, 1.6, col);
            }
        }
        if st.comb {
            let plane = self.active_plane;
            let w = |p: DVec2| -> Vec3 { plane.to_world(p, 0.0) };
            let comb_col = egui::Color32::from_rgb(255, 149, 0);
            for (eid, e) in self.sketch().entities.iter() {
                if !self.selected.is_empty() && !self.selected.contains(&eid) {
                    continue;
                }
                let pts: Vec<DVec2> = match e {
                    ducad_sketch::Entity::Spline { points, .. } if points.len() >= 2 => {
                        ducad_sketch::entity::sample_catmull_rom(points, 12)
                    }
                    ducad_sketch::Entity::Arc {
                        center,
                        radius,
                        start_angle,
                        end_angle,
                        ..
                    } => {
                        let mut a1 = *end_angle;
                        if a1 <= *start_angle {
                            a1 += std::f64::consts::TAU;
                        }
                        (0..=24)
                            .map(|i| {
                                let a = start_angle + (a1 - start_angle) * i as f64 / 24.0;
                                *center + DVec2::new(a.cos(), a.sin()) * *radius
                            })
                            .collect()
                    }
                    ducad_sketch::Entity::Circle { center, radius, .. } => (0..=48)
                        .map(|i| {
                            let a = std::f64::consts::TAU * i as f64 / 48.0;
                            *center + DVec2::new(a.cos(), a.sin()) * *radius
                        })
                        .collect(),
                    _ => continue,
                };
                let mut tips = Vec::new();
                for (p, tip) in comb_for_polyline(&pts, st.comb_scale) {
                    if let (Some(a), Some(b)) = (
                        world_to_screen_pos(&self.camera, rect, w(p)),
                        world_to_screen_pos(&self.camera, rect, w(tip)),
                    ) {
                        painter.line_segment([a, b], egui::Stroke::new(1.0, comb_col));
                        tips.push(b);
                    }
                }
                painter.add(egui::Shape::line(tips, egui::Stroke::new(1.5, comb_col)));
            }
        }
    }

    /// Jendela "Inspeksi" (Fase 5).
    pub fn show_inspect_tools(&mut self, ctx: &egui::Context) {
        if !self.inspect_tools.open {
            return;
        }
        #[derive(Clone, Copy)]
        enum Act {
            Clearance,
            MinRadius,
            Continuity,
            Silhouette,
            Iso,
            Access,
            Threads,
            Clear,
        }
        let mut act = None;
        let mut open = true;
        let st = &mut self.inspect_tools;
        egui::Window::new(ducad_i18n::t!("inspect-tools-title"))
            .open(&mut open)
            .collapsible(true)
            .resizable(true)
            .default_width(340.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .button(ducad_i18n::t!("inspect-clearance"))
                        .on_hover_text(ducad_i18n::t!("inspect-clearance-hint"))
                        .clicked()
                    {
                        act = Some(Act::Clearance);
                    }
                    if ui.button(ducad_i18n::t!("inspect-min-radius-btn")).clicked() {
                        act = Some(Act::MinRadius);
                    }
                });
                if let Some(c) = &st.clearance {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} - {}: {:.3} mm",
                            c.names.0, c.names.1, c.distance
                        ))
                        .small(),
                    );
                }
                ui.separator();
                ui.checkbox(&mut st.curvature.enabled, ducad_i18n::t!("inspect-curvature-map"));
                ui.horizontal(|ui| {
                    ui.label(ducad_i18n::t!("inspect-curvature-max"));
                    ui.add(
                        egui::DragValue::new(&mut st.curvature.max_curvature)
                            .speed(0.005)
                            .range(0.001..=10.0)
                            .suffix(" 1/mm"),
                    );
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut st.comb, ducad_i18n::t!("inspect-comb"));
                    ui.add(
                        egui::DragValue::new(&mut st.comb_scale)
                            .speed(1.0)
                            .range(0.1..=10_000.0),
                    );
                });
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    if ui.button(ducad_i18n::t!("inspect-continuity")).clicked() {
                        act = Some(Act::Continuity);
                    }
                    if ui.button(ducad_i18n::t!("inspect-silhouette")).clicked() {
                        act = Some(Act::Silhouette);
                    }
                    if ui.button(ducad_i18n::t!("inspect-isocurve")).clicked() {
                        act = Some(Act::Iso);
                    }
                    ui.add(egui::DragValue::new(&mut st.iso_count).range(2..=64));
                });
                ui.horizontal(|ui| {
                    ui.label(ducad_i18n::t!("inspect-access-dir"));
                    for v in st.access_dir.iter_mut() {
                        ui.add(egui::DragValue::new(v).speed(0.05).range(-1.0..=1.0));
                    }
                    if ui.button(ducad_i18n::t!("inspect-access")).clicked() {
                        act = Some(Act::Access);
                    }
                });
                if let Some(r) = st.access_ratio {
                    ui.label(
                        egui::RichText::new(ducad_i18n::t!(
                            "inspect-access-result",
                            pct = format!("{:.1}", r * 100.0)
                        ))
                        .small(),
                    );
                }
                ui.separator();
                if ui.button(ducad_i18n::t!("inspect-threads")).clicked() {
                    act = Some(Act::Threads);
                }
                for t in &st.threads {
                    ui.label(
                        egui::RichText::new(format!("{}  {}  {}", t.body, t.designation, t.detail))
                            .small(),
                    );
                }
                ui.separator();
                if ui.button(ducad_i18n::t!("inspect-clear")).clicked() {
                    act = Some(Act::Clear);
                }
            });
        match act {
            Some(Act::Clearance) => self.measure_clearance(),
            Some(Act::MinRadius) => self.find_min_radius(),
            Some(Act::Continuity) => self.run_continuity(),
            Some(Act::Silhouette) => self.compute_silhouette(),
            Some(Act::Iso) => self.compute_isocurves(),
            Some(Act::Access) => self.compute_accessibility(),
            Some(Act::Threads) => self.list_threads(),
            Some(Act::Clear) => {
                let st = &self.inspect_tools;
                let keep = (st.curvature, st.comb_scale, st.access_dir);
                self.inspect_tools = InspectToolsState {
                    open: true,
                    curvature: keep.0,
                    comb_scale: keep.1,
                    access_dir: keep.2,
                    ..Default::default()
                };
            }
            None => {}
        }
        if !open {
            self.inspect_tools.open = false;
            self.inspect_tools.curvature.enabled = false;
        }
    }
}

/// Grid isocurve face datar: garis u/v dipotong ke poligon batas face.
fn iso_plane(info: &FaceInfo, n: usize) -> Vec<Vec<DVec3>> {
    let nrm = DVec3::from(info.normal).normalize_or_zero();
    let poly: Vec<DVec3> = info.boundary.iter().map(|p| DVec3::from(*p)).collect();
    if poly.len() < 3 || nrm == DVec3::ZERO {
        return Vec::new();
    }
    let u = nrm.any_orthonormal_vector();
    let v = nrm.cross(u);
    let o = poly[0];
    let p2: Vec<DVec2> = poly
        .iter()
        .map(|p| DVec2::new((*p - o).dot(u), (*p - o).dot(v)))
        .collect();
    let (lo, hi) = p2.iter().fold(
        (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)),
        |(a, b), p| (a.min(*p), b.max(*p)),
    );
    let to3 = |q: DVec2| o + u * q.x + v * q.y;
    let mut out = Vec::new();
    for axis in 0..2 {
        for k in 1..n {
            let s = k as f64 / n as f64;
            let c = if axis == 0 {
                lo.x + (hi.x - lo.x) * s
            } else {
                lo.y + (hi.y - lo.y) * s
            };
            let mut hits: Vec<f64> = Vec::new();
            for (i, a) in p2.iter().enumerate() {
                let b = p2[(i + 1) % p2.len()];
                let (ac, bc, aw, bw) = if axis == 0 {
                    (a.x, b.x, a.y, b.y)
                } else {
                    (a.y, b.y, a.x, b.x)
                };
                if (ac <= c && bc > c) || (bc <= c && ac > c) {
                    hits.push(aw + (c - ac) / (bc - ac) * (bw - aw));
                }
            }
            hits.sort_by(|a, b| a.total_cmp(b));
            for pair in hits.chunks_exact(2) {
                let (p, q) = if axis == 0 {
                    (DVec2::new(c, pair[0]), DVec2::new(c, pair[1]))
                } else {
                    (DVec2::new(pair[0], c), DVec2::new(pair[1], c))
                };
                out.push(vec![to3(p), to3(q)]);
            }
        }
    }
    out
}

/// Isocurve silinder: garis sejajar sumbu (sudut) + lingkaran (tinggi),
/// dibatasi rentang sudut & tinggi titik mesh face.
fn iso_cylinder(info: &FaceInfo, pts: &[DVec3], n: usize) -> Vec<Vec<DVec3>> {
    let (Some((o, d)), Some(r)) = (info.axis, info.radius) else {
        return Vec::new();
    };
    let (o, a) = (DVec3::from(o), DVec3::from(d).normalize_or_zero());
    if a == DVec3::ZERO || pts.is_empty() {
        return Vec::new();
    }
    const BINS: usize = 72;
    let tau = std::f64::consts::TAU;
    let x = a.any_orthonormal_vector();
    let y = a.cross(x);
    let (mut h0, mut h1) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut bins = [false; BINS];
    let bin_of = |ang: f64| (((ang.rem_euclid(tau)) / tau * BINS as f64) as usize).min(BINS - 1);
    for p in pts {
        let q = *p - o;
        let h = q.dot(a);
        h0 = h0.min(h);
        h1 = h1.max(h);
        bins[bin_of(q.dot(y).atan2(q.dot(x)))] = true;
    }
    let at = |ang: f64, h: f64| o + a * h + (x * ang.cos() + y * ang.sin()) * r;
    let covered = |ang: f64| bins[bin_of(ang)];
    let mut out = Vec::new();
    for k in 0..n {
        let ang = tau * k as f64 / n as f64;
        if covered(ang) {
            out.push(vec![at(ang, h0), at(ang, h1)]);
        }
    }
    for k in 1..n {
        let h = h0 + (h1 - h0) * k as f64 / n as f64;
        let mut ring = Vec::new();
        for i in 0..=BINS {
            let ang = tau * i as f64 / BINS as f64;
            if covered(ang) {
                ring.push(at(ang, h));
            } else if ring.len() >= 2 {
                out.push(std::mem::take(&mut ring));
            } else {
                ring.clear();
            }
        }
        if ring.len() >= 2 {
            out.push(ring);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comb_of_circle_has_constant_curvature() {
        let pts: Vec<DVec2> = (0..=36)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / 36.0;
                DVec2::new(a.cos(), a.sin()) * 5.0
            })
            .collect();
        for (p, tip) in comb_for_polyline(&pts, 10.0) {
            assert!(((tip - p).length() - 2.0).abs() < 0.02, "k=1/5 × 10 = 2");
        }
        let line = [DVec2::ZERO, DVec2::new(1.0, 0.0), DVec2::new(2.0, 0.0)];
        assert!(comb_for_polyline(&line, 10.0)
            .iter()
            .all(|(p, t)| p.distance(*t) < 1e-12));
    }

    #[test]
    fn clearance_radius_continuity_access_threads() {
        let mut app = DuCADApp::new_for_test();
        app.create_primitive(ducad_ui::PrimitiveKind::Box, [10.0; 3], [0.0; 3], false);
        app.create_primitive(
            ducad_ui::PrimitiveKind::Cylinder,
            [3.0, 10.0, 0.0],
            [20.0, 5.0, 0.0],
            false,
        );
        let ids: Vec<BodyId> = app.model.doc.bodies.iter().map(|(id, _)| id).collect();
        app.selected_bodies = ids.iter().copied().collect();
        app.measure_clearance();
        let c = app.inspect_tools.clearance.clone().expect("clearance");
        assert!((c.distance - 7.0).abs() < 0.05, "{}", c.distance);
        app.find_min_radius();
        assert!((app.inspect_tools.min_radius.as_ref().unwrap().1 - 3.0).abs() < 1e-6);
        app.selected_bodies = [ids[0]].into_iter().collect();
        app.run_continuity();
        assert!(!app.inspect_tools.continuity.is_empty());
        // Kotak dari atas (−Z): face atas terjangkau, sisanya tidak.
        app.compute_accessibility();
        let r = app.inspect_tools.access_ratio.unwrap();
        assert!(r > 0.05 && r < 0.5, "{r}");
        app.list_threads();
        assert!(app.inspect_tools.threads.is_empty());
    }

    #[test]
    fn plane_isocurves_are_clipped_to_face() {
        let mut info = ducad_kernel::enumerate_faces(
            &ducad_kernel::make_box(10.0, 10.0, 10.0, false).unwrap(),
        )
        .into_iter()
        .find(|f| f.normal[2] > 0.9)
        .unwrap();
        info.boundary = vec![
            [0.0, 0.0, 10.0],
            [10.0, 0.0, 10.0],
            [10.0, 10.0, 10.0],
            [0.0, 10.0, 10.0],
        ];
        let c = iso_plane(&info, 4);
        assert_eq!(c.len(), 6);
        for seg in &c {
            for p in seg {
                assert!(p.x > -1e-6 && p.x < 10.0 + 1e-6 && p.y > -1e-6 && p.y < 10.0 + 1e-6);
            }
        }
    }
}
