//! Project + Intersect ke sketsa aktif (Paritas 1.14) — sisi GUI.
//!
//! Generalisasi `face_sketch::project_boundary`: tepi/face B-rep terpilih
//! diproyeksikan ke bidang sketsa aktif sebagai entitas *linked*, atau body
//! diiris dengan bidang sketsa (Intersect). Geometrinya dihitung oleh
//! `ducad_engine::compute::project` — sumber yang SAMA dengan
//! `Op::SketchProject` di engine (MCP/CLI) — sehingga GUI dan agent
//! menghasilkan entitas yang identik.
//!
//! Entitas linked dicatat di `Sketch::linked` bersama sidik jari mesh body.
//! Setiap frame [`DuCADApp::refresh_sketch_links`] membandingkan sidik jari
//! itu dengan body sekarang dan menurunkan ulang grup yang body-nya berubah.
//! "Break Link" melepas catatan itu sehingga entitas menjadi entitas biasa.

use ducad_core::BodyId;
use ducad_engine::compute::project::{edges_of_faces, intersect_body, project_edges, Projected};
use ducad_kernel::{EdgeInfo, KernelShape};
use ducad_sketch::{BreakLinks, EntityId, InsertLinked, LinkKind, LinkSource};
use glam::{DVec2, DVec3};

use crate::app::DuCADApp;
use crate::document::plane_frame_from;
use crate::viewport::world_to_screen_pos;

/// Toleransi pencocokan ujung polyline tepi hasil pick ke `EdgeInfo` (mm).
const EDGE_MATCH_TOL: f64 = 1e-3;

/// Mode perintah proyeksi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectMode {
    Project,
    Intersect,
}

/// Indeks tepi `edges` yang ujung-ujungnya cocok dengan polyline pick
/// (arah bebas). Bila beberapa cocok (mis. lingkaran penuh dengan
/// awal = akhir), dipilih yang titik tengahnya terdekat ke polyline.
pub fn match_edge_index(edges: &[EdgeInfo], polyline: &[(f64, f64, f64)]) -> Option<usize> {
    let to_v = |p: &(f64, f64, f64)| DVec3::new(p.0, p.1, p.2);
    let a = to_v(polyline.first()?);
    let b = to_v(polyline.last()?);
    let near = |p: [f64; 3], q: DVec3| (DVec3::from_array(p) - q).length() <= EDGE_MATCH_TOL;
    let dist_to_poly = |p: [f64; 3]| {
        let p = DVec3::from_array(p);
        polyline
            .iter()
            .map(|q| (to_v(q) - p).length())
            .fold(f64::INFINITY, f64::min)
    };
    edges
        .iter()
        .filter(|e| (near(e.start, a) && near(e.end, b)) || (near(e.start, b) && near(e.end, a)))
        .map(|e| (e.index, dist_to_poly(e.mid)))
        .filter(|(_, d)| d.is_finite())
        .min_by(|x, y| x.1.total_cmp(&y.1))
        .map(|(i, _)| i)
}

/// Turunkan entitas untuk `kind` dari `shape` di bidang `frame`.
pub fn derive_link_entities(
    shape: &KernelShape,
    kind: &LinkKind,
    frame: &ducad_engine::PlaneFrame,
) -> Projected {
    match kind {
        LinkKind::Edges { edges } => project_edges(shape, edges, frame),
        LinkKind::Faces { faces } => {
            let all = ducad_kernel::enumerate_faces(shape);
            project_edges(shape, &edges_of_faces(&all, faces), frame)
        }
        LinkKind::Intersect => intersect_body(shape, frame),
    }
}

impl DuCADApp {
    /// Body pertama bernama `name` beserta geometrinya.
    fn body_by_name(&self, name: &str) -> Option<BodyId> {
        self.model
            .doc
            .bodies
            .iter()
            .find(|(_, b)| b.name == name)
            .map(|(id, _)| id)
    }

    /// Body sumber proyeksi: body face aktif, atau satu-satunya body terpilih.
    fn project_source_body(&self) -> Option<BodyId> {
        if let Some((id, _, _)) = &self.active_face {
            return Some(*id);
        }
        (self.selected_bodies.len() == 1).then(|| self.selected_bodies.iter().next().copied()).flatten()
    }

    /// Perintah Project / Intersect dari palette.
    pub fn project_to_sketch(&mut self, mode: ProjectMode) {
        let frame = plane_frame_from(&self.active_plane);
        let bodies: Vec<BodyId> = match mode {
            ProjectMode::Project => self.project_source_body().into_iter().collect(),
            ProjectMode::Intersect => match self.project_source_body() {
                Some(id) => vec![id],
                // Tanpa seleksi: iris semua body yang terlihat.
                None => self
                    .model
                    .doc
                    .bodies
                    .iter()
                    .filter(|(_, b)| b.visible)
                    .map(|(id, _)| id)
                    .collect(),
            },
        };
        if bodies.is_empty() {
            self.model_status = Some(ducad_i18n::t!("project-need-selection"));
            return;
        }
        let mut total = 0usize;
        let mut warnings = Vec::new();
        // Dikumpulkan dulu: geometri body dipinjam selama loop.
        let mut pending: Vec<(Vec<ducad_sketch::Entity>, LinkSource, String)> = Vec::new();
        for body in bodies {
            let Some(name) = self.model.doc.bodies.get(body).map(|b| b.name.clone()) else {
                continue;
            };
            let Some(geo) = self.model.geometry.get(body) else {
                continue;
            };
            let kinds: Vec<LinkKind> = match mode {
                ProjectMode::Intersect => vec![LinkKind::Intersect],
                ProjectMode::Project => {
                    let mut faces: Vec<usize> = self
                        .selected_faces
                        .iter()
                        .chain(self.active_face.as_ref().map(|(_, r, _)| r))
                        .filter_map(|r| ducad_kernel::pick_face_details(&geo.shape, *r))
                        .filter_map(|h| h.face_index)
                        .collect();
                    faces.sort_unstable();
                    faces.dedup();
                    let all_edges = ducad_kernel::enumerate_edges(&geo.shape);
                    let mut edges: Vec<usize> = self
                        .selected_edges
                        .iter()
                        .filter_map(|e| match_edge_index(&all_edges, &e.polyline))
                        .collect();
                    edges.sort_unstable();
                    edges.dedup();
                    let mut k = Vec::new();
                    if !faces.is_empty() {
                        k.push(LinkKind::Faces { faces });
                    }
                    if !edges.is_empty() {
                        k.push(LinkKind::Edges { edges });
                    }
                    k
                }
            };
            if kinds.is_empty() {
                continue;
            }
            for kind in kinds {
                let derived = derive_link_entities(&geo.shape, &kind, &frame);
                warnings.extend(derived.warnings);
                if derived.entities.is_empty() {
                    continue;
                }
                total += derived.entities.len();
                let src = LinkSource {
                    group: 0,
                    body: name.clone(),
                    kind,
                    fingerprint: geo.mesh_fingerprint,
                };
                let label = match mode {
                    ProjectMode::Project => ducad_i18n::t!("project-group-name", body = name.as_str()),
                    ProjectMode::Intersect => ducad_i18n::t!("intersect-group-name", body = name.as_str()),
                };
                pending.push((derived.entities, src, label));
            }
        }
        for (entities, src, label) in pending {
            self.execute_sketch_command(Box::new(InsertLinked::new(entities, src).with_group_name(label)));
        }
        self.model_status = Some(if total == 0 {
            match mode {
                ProjectMode::Project => ducad_i18n::t!("project-need-selection"),
                ProjectMode::Intersect => ducad_i18n::t!("intersect-none"),
            }
        } else if warnings.is_empty() {
            ducad_i18n::t!("project-done", count = total)
        } else {
            ducad_i18n::t!("project-done-warn", count = total, warnings = warnings.len())
        });
    }

    /// Break Link: entitas linked terpilih (atau semua bila seleksi kosong)
    /// menjadi entitas biasa — satu langkah undo.
    pub fn break_sketch_links(&mut self) {
        let sk = self.sketch();
        let ids: Vec<EntityId> = if self.selected.is_empty() {
            sk.linked.keys().copied().collect()
        } else {
            self.selected.iter().copied().filter(|id| sk.is_linked(*id)).collect()
        };
        if ids.is_empty() {
            self.model_status = Some(ducad_i18n::t!("break-link-none"));
            return;
        }
        let n = ids.len();
        self.execute_sketch_command(Box::new(BreakLinks::new(ids)));
        self.model_status = Some(ducad_i18n::t!("break-link-done", count = n));
    }

    /// Turunkan ulang grup link sketsa aktif yang body sumbernya berubah.
    /// Murah bila tidak ada link (dipanggil tiap frame).
    pub fn refresh_sketch_links(&mut self) {
        if self.sketch().linked.is_empty() {
            return;
        }
        let frame = plane_frame_from(&self.active_plane);
        let groups = self.sketch().link_groups();
        let mut updates = Vec::new();
        for (group, src, _) in groups {
            let Some(body) = self.body_by_name(&src.body) else {
                continue;
            };
            let Some(geo) = self.model.geometry.get(body) else {
                continue;
            };
            if geo.mesh_fingerprint == src.fingerprint {
                continue;
            }
            let derived = derive_link_entities(&geo.shape, &src.kind, &frame);
            updates.push((group, derived.entities, geo.mesh_fingerprint));
        }
        if updates.is_empty() {
            return;
        }
        let sk = self.sketch_mut();
        for (group, ents, fp) in updates {
            sk.rederive_link_group(group, ents, fp);
        }
        sk.prune_links();
        let live = self.sketch().entities.keys().collect::<std::collections::HashSet<_>>();
        self.selected.retain(|id| live.contains(id));
    }

    /// Garis ungu putus-putus di atas entitas linked (penanda read-only).
    pub fn paint_linked_entities(&self, ui: &egui::Ui, rect: egui::Rect) {
        let sk = self.sketch();
        if sk.linked.is_empty() || !self.is_sketching {
            return;
        }
        let painter = ui.painter_at(rect);
        let color = egui::Color32::from_rgb(175, 82, 222);
        let to_px = |p: DVec2| world_to_screen_pos(&self.camera, rect, self.active_plane.to_world(p, 0.0));
        for id in sk.linked.keys() {
            let Some(e) = sk.entities.get(*id) else { continue };
            for poly in e.flatten_all(0.05) {
                let px: Vec<egui::Pos2> = poly.iter().filter_map(|p| to_px(*p)).collect();
                if px.len() >= 2 {
                    painter.extend(egui::Shape::dashed_line(&px, egui::Stroke::new(1.5, color), 6.0, 4.0));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_kernel::EdgeKind;

    fn edge(index: usize, s: [f64; 3], e: [f64; 3]) -> EdgeInfo {
        EdgeInfo {
            index,
            kind: EdgeKind::Line,
            start: s,
            end: e,
            mid: [(s[0] + e[0]) / 2.0, (s[1] + e[1]) / 2.0, (s[2] + e[2]) / 2.0],
            length: 1.0,
            dir: None,
            radius: None,
        }
    }

    #[test]
    fn picked_polyline_matches_edge_in_either_direction() {
        let edges = vec![edge(0, [0.0; 3], [10.0, 0.0, 0.0]), edge(5, [0.0; 3], [0.0, 10.0, 0.0])];
        let poly = vec![(0.0, 10.0, 0.0), (0.0, 5.0, 0.0), (0.0, 0.0, 0.0)];
        assert_eq!(match_edge_index(&edges, &poly), Some(5));
        assert_eq!(match_edge_index(&edges, &[]), None);
        assert_eq!(match_edge_index(&edges, &[(f64::NAN, 0.0, 0.0)]), None);
    }

    fn app_with_box(dims: [f64; 3]) -> (DuCADApp, BodyId) {
        let mut app = DuCADApp::new_for_test();
        app.create_primitive(ducad_ui::PrimitiveKind::Box, dims, [5.0, 0.0, 0.0], false);
        let id = app.model.doc.bodies.keys().next().expect("box dibuat");
        app.selected_bodies = [id].into_iter().collect();
        (app, id)
    }

    fn sketch_width(app: &DuCADApp) -> f64 {
        let (min, max) = app
            .sketch()
            .entities
            .values()
            .filter_map(|e| e.bounding_box())
            .reduce(|(a0, a1), (b0, b1)| (a0.min(b0), a1.max(b1)))
            .expect("ada entitas");
        max.x - min.x
    }

    #[test]
    fn project_top_face_then_body_change_rederives_and_break_link() {
        let (mut app, id) = app_with_box([10.0, 20.0, 30.0]);
        app.selected_faces = vec![ducad_kernel::PickRay { origin: (10.0, 10.0, 100.0), dir: (0.0, 0.0, -1.0) }];
        app.project_to_sketch(ProjectMode::Project);
        assert_eq!(app.sketch().entities.len(), 4, "{:?}", app.model_status);
        assert_eq!(app.sketch().linked.len(), 4);
        assert!((sketch_width(&app) - 10.0).abs() < 1e-6);

        // Body berubah → entitas linked ikut berubah pada refresh berikutnya.
        let bigger = ducad_engine::compute::primitive(
            &crate::body_tools::primitive_shape(ducad_ui::PrimitiveKind::Box, [25.0, 20.0, 30.0], false),
            [5.0, 0.0, 0.0],
        )
        .expect("box besar");
        app.execute_model_command(
            Box::new(crate::model::ReplaceGeometryCommand::new("uji", id, bigger)),
            "uji",
        );
        app.refresh_sketch_links();
        assert_eq!(app.sketch().entities.len(), 4);
        assert!((sketch_width(&app) - 25.0).abs() < 1e-6, "lebar {}", sketch_width(&app));

        // Break Link (tanpa seleksi = semua) → entitas biasa; Undo memulihkan.
        app.break_sketch_links();
        assert!(app.sketch().linked.is_empty());
        app.undo();
        assert_eq!(app.sketch().linked.len(), 4);
    }

    #[test]
    fn intersect_box_with_front_plane() {
        let (mut app, _) = app_with_box([10.0, 20.0, 30.0]);
        // Bidang Front (XZ, y = 0) berimpit sisi depan kotak: irisan = persegi 10 × 30.
        app.active_plane = ducad_render::SketchPlane::front();
        app.project_to_sketch(ProjectMode::Intersect);
        assert!(!app.sketch().entities.is_empty(), "{:?}", app.model_status);
        assert!(app.sketch().entities.keys().all(|id| app.sketch().is_linked(id)));
    }

    #[test]
    fn project_without_selection_reports_and_does_not_panic() {
        let mut app = DuCADApp::new_for_test();
        app.project_to_sketch(ProjectMode::Project);
        assert!(app.sketch().entities.is_empty());
        app.project_to_sketch(ProjectMode::Intersect);
        assert!(app.sketch().entities.is_empty());
        app.break_sketch_links();
        app.refresh_sketch_links();
    }
}
