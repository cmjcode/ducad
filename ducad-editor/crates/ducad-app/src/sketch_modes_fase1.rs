//! Logika klik, penyelesaian, dan preview mode gambar Fase 1B (Paritas
//! 1.7–1.12). Geometrinya murni di `ducad_sketch::construct2d`; berkas ini
//! hanya merangkai klik `pending_points` menjadi command sketsa.

use ducad_core::Command;
use ducad_render::{sketch as sketch_render, LineVertex, SketchPlane};
use ducad_sketch::{
    arc_from_center, arc_from_three_points, arc_slot_from_arc, circle_tangent_three, circle_tangent_two,
    circle_three_point, circle_two_point, endpoint_tangent, infinite_line_span, line_from_midpoint,
    rect_three_point, rounded_rect, rounded_rect_constraint_pairs, tangent_arc, tangent_residual,
    ConstraintTemplate, Entity, EntityId, InsertConstructed, InsertEntities, NewRef, Sketch, SlotMode,
    TangentTarget,
};
use glam::DVec2;

use crate::app::DuCADApp;
use crate::sketch_modes::{ArcDrawMode, CircleDrawMode, LineDrawMode, RectDrawMode};
use crate::types::{required_points, ToolKind};

/// Toleransi residual agar constraint Tangent otomatis hanya dipasang bila
/// geometrinya memang sudah menyinggung (solver tidak menggeser apa pun).
const TANGENT_OK: f64 = 1e-6;

/// Setengah panjang tampilan garis tak hingga untuk pratinjau.
const PREVIEW_INFINITE_HALF: f64 = 1.0e4;

impl DuCADApp {
    /// Mode non-bawaan aktif untuk tool sekarang: pratinjau/pill bawaan tool
    /// tidak berlaku dan diganti [`Self::mode_preview_entities`].
    pub fn custom_draw_mode_active(&self) -> bool {
        let m = &self.sketch_modes;
        match self.tool {
            ToolKind::Circle => m.circle != CircleDrawMode::CenterRadius,
            ToolKind::Arc => m.arc == ArcDrawMode::Tangent,
            ToolKind::Rectangle => matches!(m.rect, RectDrawMode::ThreePoint | RectDrawMode::Rounded),
            ToolKind::Line => m.line != LineDrawMode::Chain,
            ToolKind::Slot => matches!(self.slot_mode, SlotMode::ArcCenter | SlotMode::ArcThreePoint),
            ToolKind::Polygon => self.polygon_mode == ducad_sketch::PolygonMode::Edge,
            _ => false,
        }
    }

    /// Tool Garis memakai dua klik (bukan rantai) pada mode titik-tengah/tak hingga.
    pub fn line_uses_two_clicks(&self) -> bool {
        self.sketch_modes.line != LineDrawMode::Chain
    }

    /// Jumlah klik yang dibutuhkan tool aktif dengan mode sekarang.
    pub fn required_points_now(&self) -> usize {
        let m = &self.sketch_modes;
        match self.tool {
            ToolKind::Circle => match m.circle {
                CircleDrawMode::CenterRadius | CircleDrawMode::TwoPoint => 2,
                CircleDrawMode::ThreePoint | CircleDrawMode::TwoTangent | CircleDrawMode::ThreeTangent => 3,
            },
            ToolKind::Arc if m.arc == ArcDrawMode::Tangent => 2,
            ToolKind::Rectangle if m.rect == RectDrawMode::ThreePoint => 3,
            ToolKind::Slot if matches!(self.slot_mode, SlotMode::ArcCenter | SlotMode::ArcThreePoint) => 4,
            ToolKind::Line if self.line_uses_two_clicks() => 2,
            tool => required_points(tool),
        }
    }

    /// Kurva (Line/Circle/Arc) di bawah `p` sebagai target tangen.
    pub fn pick_tangent_curve(&self, p: DVec2) -> Option<(EntityId, TangentTarget)> {
        let sketch = self.sketch();
        let tol = self.sketch_modes.pick_tol.max(1e-6);
        let id = sketch.hit_test(p, tol).or_else(|| sketch.hit_test_linear(p, tol))?;
        let t = TangentTarget::from_entity(sketch.entities.get(id)?)?;
        Some((id, t))
    }

    /// Ujung Line/Arc terdekat ke `p`: (id, titik ujung, arah tangen keluar).
    pub fn pick_tangent_endpoint(&self, p: DVec2) -> Option<(EntityId, DVec2, DVec2)> {
        let sketch = self.sketch();
        let tol = self.sketch_modes.pick_tol.max(1e-6);
        sketch
            .entities
            .iter()
            .filter(|(id, _)| !sketch.is_hidden(*id))
            .filter_map(|(id, e)| endpoint_tangent(e, p, tol).map(|(q, d)| (id, q, d)))
            .min_by(|a, b| (a.1 - p).length().total_cmp(&(b.1 - p).length()))
    }

    /// Validasi klik sebelum masuk `pending_points`: klik mode tangen harus
    /// mengenai kurva/ujung. `false` = klik diabaikan (dengan pesan status).
    pub fn accept_mode_click(&mut self, p: DVec2) -> bool {
        let n = self.pending_points.len();
        let ok = match self.tool {
            ToolKind::Circle => match self.sketch_modes.circle {
                CircleDrawMode::TwoTangent if n < 2 => self.pick_tangent_curve(p).is_some(),
                CircleDrawMode::ThreeTangent => self.pick_tangent_curve(p).is_some(),
                _ => true,
            },
            ToolKind::Arc if self.sketch_modes.arc == ArcDrawMode::Tangent && n == 0 => {
                self.pick_tangent_endpoint(p).is_some()
            }
            _ => true,
        };
        if !ok {
            self.model_status = Some(match self.tool {
                ToolKind::Arc => ducad_i18n::t!("draw-mode-pick-endpoint"),
                _ => ducad_i18n::t!("draw-mode-pick-curve"),
            });
        }
        ok
    }

    /// Selesaikan klik untuk mode Fase 1B. `true` bila mode ini menangani
    /// tool aktif (berhasil atau gagal), `false` = jalur bawaan tool.
    pub fn finish_mode_multipoint(&mut self, pts: &[DVec2]) -> bool {
        if !self.custom_draw_mode_active() || pts.len() < self.required_points_now() {
            return false;
        }
        let cmd = self.build_mode_command(pts);
        match cmd {
            Some(c) => {
                self.execute_sketch_command(c);
                self.dynamic_input.clear();
                self.dynamic_focus_pending = false;
            }
            None => self.model_status = Some(ducad_i18n::t!("draw-mode-failed")),
        }
        true
    }

    fn build_mode_command(&self, pts: &[DVec2]) -> Option<Box<dyn Command<Sketch>>> {
        let cons = self.construction_mode;
        let insert = |label: &'static str, ents: Vec<Entity>| -> Box<dyn Command<Sketch>> {
            Box::new(InsertEntities::new(label, ents.into_iter().map(|e| e.with_construction(cons)).collect()))
        };
        let circle = |c: DVec2, r: f64| Entity::circle(c, r).with_construction(cons);
        let m = &self.sketch_modes;
        match self.tool {
            ToolKind::Circle => match m.circle {
                CircleDrawMode::CenterRadius => None,
                CircleDrawMode::TwoPoint => {
                    circle_two_point(pts[0], pts[1]).map(|(c, r)| insert("Circle", vec![circle(c, r)]))
                }
                CircleDrawMode::ThreePoint => {
                    circle_three_point(pts[0], pts[1], pts[2]).map(|(c, r)| insert("Circle", vec![circle(c, r)]))
                }
                CircleDrawMode::TwoTangent => {
                    let (id1, t1) = self.pick_tangent_curve(pts[0])?;
                    let (id2, t2) = self.pick_tangent_curve(pts[1])?;
                    let (c, r) = circle_tangent_two(&t1, &t2, pts[2])?;
                    Some(self.tangent_circle_command(circle(c, r), &[id1, id2]))
                }
                CircleDrawMode::ThreeTangent => {
                    let picks: Vec<(EntityId, TangentTarget)> =
                        pts.iter().take(3).filter_map(|p| self.pick_tangent_curve(*p)).collect();
                    let [a, b, c] = picks.as_slice() else { return None };
                    let guess = pts.iter().take(3).copied().sum::<DVec2>() / 3.0;
                    let (center, r) = circle_tangent_three([&a.1, &b.1, &c.1], guess)?;
                    Some(self.tangent_circle_command(circle(center, r), &[a.0, b.0, c.0]))
                }
            },
            ToolKind::Arc => {
                let (id, start, dir) = self.pick_tangent_endpoint(pts[0])?;
                let arc = tangent_arc(start, dir, pts[1])?.with_construction(cons);
                let mut tpl = Vec::new();
                let existing = self.sketch().entities.get(id)?;
                if tangent_residual(existing, &arc).is_some_and(|r| r.abs() < TANGENT_OK) {
                    tpl.push(ConstraintTemplate::Tangent(NewRef::Existing(id), NewRef::New(0)));
                }
                // Ujung sumber berimpit dengan ujung busur baru yang berada
                // di `start` (busur bisa CW → titik itu ujung akhirnya).
                let nearest_end = |e: &Entity| -> Option<bool> {
                    let ends = e.endpoints();
                    let (s, t) = match e {
                        Entity::Line { .. } | Entity::Arc { .. } => (*ends.first()?, *ends.get(1)?),
                        _ => return None,
                    };
                    Some((t - start).length() < (s - start).length())
                };
                if let (Some(src_end), Some(arc_end)) = (nearest_end(existing), nearest_end(&arc)) {
                    tpl.push(ConstraintTemplate::CoincidentEnds(
                        NewRef::Existing(id),
                        src_end,
                        NewRef::New(0),
                        arc_end,
                    ));
                }
                Some(Box::new(InsertConstructed::new("Arc", vec![arc]).with_constraints(tpl)))
            }
            ToolKind::Rectangle => match m.rect {
                RectDrawMode::ThreePoint => {
                    let c = rect_three_point(pts[0], pts[1], pts[2])?;
                    Some(insert("Rectangle", (0..4).map(|i| Entity::line(c[i], c[(i + 1) % 4])).collect()))
                }
                RectDrawMode::Rounded => {
                    let ents = rounded_rect(pts[0], pts[1], m.corner_radius, cons)?;
                    let (tan, eq) = rounded_rect_constraint_pairs();
                    let tpl = tan
                        .iter()
                        .map(|&(l, a)| ConstraintTemplate::Tangent(NewRef::New(l), NewRef::New(a)))
                        .chain(eq.iter().map(|&(a, b)| ConstraintTemplate::EqualRadius(NewRef::New(a), NewRef::New(b))))
                        .collect();
                    Some(Box::new(InsertConstructed::new("Rectangle", ents).with_constraints(tpl)))
                }
                _ => None,
            },
            ToolKind::Slot => {
                let centerline = match self.slot_mode {
                    SlotMode::ArcCenter => arc_from_center(pts[0], pts[1], pts[2]),
                    SlotMode::ArcThreePoint => arc_from_three_points(pts[0], pts[1], pts[2]),
                    _ => None,
                }?;
                arc_slot_from_arc(&centerline, pts[3], cons).map(|e| insert("Slot", e))
            }
            ToolKind::Polygon => ducad_sketch::regular_polygon_entities(
                pts[0],
                pts[1],
                self.polygon_sides,
                self.polygon_mode,
                cons,
            )
            .map(|e| insert("Segi-N", e)),
            ToolKind::Line => match m.line {
                LineDrawMode::Midpoint => {
                    let (a, b) = line_from_midpoint(pts[0], pts[1])?;
                    Some(insert("Line", vec![Entity::line(a, b)]))
                }
                LineDrawMode::Infinite => {
                    if (pts[1] - pts[0]).length() < 1e-9 || !(pts[0].is_finite() && pts[1].is_finite()) {
                        return None;
                    }
                    let line = Entity::line(pts[0], pts[1]).with_construction(true);
                    Some(Box::new(InsertConstructed::new("Line", vec![line]).with_infinite(vec![0])))
                }
                LineDrawMode::Chain => None,
            },
            _ => None,
        }
    }

    /// Lingkaran baru + constraint Tangent ke setiap kurva acuan (hanya yang
    /// sudah terpenuhi geometrinya), lalu solve — satu langkah undo.
    fn tangent_circle_command(&self, circle: Entity, refs: &[EntityId]) -> Box<dyn Command<Sketch>> {
        let sketch = self.sketch();
        let tpl = refs
            .iter()
            .filter(|id| {
                sketch
                    .entities
                    .get(**id)
                    .and_then(|e| tangent_residual(e, &circle))
                    .is_some_and(|r| r.abs() < TANGENT_OK)
            })
            .map(|id| ConstraintTemplate::Tangent(NewRef::Existing(*id), NewRef::New(0)))
            .collect();
        Box::new(InsertConstructed::new("Circle", vec![circle]).with_constraints(tpl))
    }

    /// Entitas pratinjau mode Fase 1B untuk posisi kursor. `None` = mode
    /// bawaan tool (pratinjau lama yang berlaku); `Some(vec![])` = mode
    /// khusus aktif tetapi belum ada yang bisa dipratinjau.
    pub fn mode_preview_entities(&self, cursor: DVec2) -> Option<Vec<Entity>> {
        if !self.custom_draw_mode_active() {
            return None;
        }
        let pts = &self.pending_points;
        let cons = self.construction_mode;
        let guide = |a: DVec2, b: DVec2| Entity::line(a, b).with_construction(true);
        let mut out: Vec<Entity> = Vec::new();
        let m = &self.sketch_modes;
        match (self.tool, pts.len()) {
            (ToolKind::Circle, n) => match (m.circle, n) {
                (CircleDrawMode::TwoPoint, 1) => {
                    out.extend(circle_two_point(pts[0], cursor).map(|(c, r)| Entity::circle(c, r)));
                }
                (CircleDrawMode::ThreePoint, 1) => out.push(guide(pts[0], cursor)),
                (CircleDrawMode::ThreePoint, 2) => {
                    out.extend(circle_three_point(pts[0], pts[1], cursor).map(|(c, r)| Entity::circle(c, r)));
                }
                (CircleDrawMode::TwoTangent, 2) => {
                    if let (Some((_, t1)), Some((_, t2))) =
                        (self.pick_tangent_curve(pts[0]), self.pick_tangent_curve(pts[1]))
                    {
                        out.extend(circle_tangent_two(&t1, &t2, cursor).map(|(c, r)| Entity::circle(c, r)));
                    }
                }
                _ => {}
            },
            (ToolKind::Arc, 1) => {
                if let Some((_, start, dir)) = self.pick_tangent_endpoint(pts[0]) {
                    out.extend(tangent_arc(start, dir, cursor));
                }
            }
            (ToolKind::Rectangle, 1) if m.rect == RectDrawMode::Rounded => {
                match rounded_rect(pts[0], cursor, m.corner_radius, cons) {
                    Some(ents) => out.extend(ents),
                    None => out.push(guide(pts[0], cursor)),
                }
            }
            (ToolKind::Rectangle, 1) => out.push(guide(pts[0], cursor)),
            (ToolKind::Rectangle, 2) => {
                if let Some(c) = rect_three_point(pts[0], pts[1], cursor) {
                    out.extend((0..4).map(|i| Entity::line(c[i], c[(i + 1) % 4])));
                }
            }
            (ToolKind::Slot, 1) => out.push(guide(pts[0], cursor)),
            (ToolKind::Slot, 2) => {
                let arc = match self.slot_mode {
                    SlotMode::ArcCenter => arc_from_center(pts[0], pts[1], cursor),
                    _ => arc_from_three_points(pts[0], pts[1], cursor),
                };
                out.extend(arc.map(|a| a.with_construction(true)));
            }
            (ToolKind::Slot, 3) => {
                let arc = match self.slot_mode {
                    SlotMode::ArcCenter => arc_from_center(pts[0], pts[1], pts[2]),
                    _ => arc_from_three_points(pts[0], pts[1], pts[2]),
                };
                if let Some(arc) = arc {
                    out.extend(arc_slot_from_arc(&arc, cursor, cons).unwrap_or_default());
                    out.push(arc.with_construction(true));
                }
            }
            (ToolKind::Polygon, 1) => {
                out.extend(
                    ducad_sketch::regular_polygon_entities(pts[0], cursor, self.polygon_sides, self.polygon_mode, cons)
                        .unwrap_or_default(),
                );
            }
            (ToolKind::Line, 1) => match m.line {
                LineDrawMode::Midpoint => {
                    out.extend(line_from_midpoint(pts[0], cursor).map(|(a, b)| Entity::line(a, b)));
                }
                LineDrawMode::Infinite => {
                    out.extend(
                        infinite_line_span(pts[0], cursor, pts[0], PREVIEW_INFINITE_HALF).map(|(a, b)| guide(a, b)),
                    );
                }
                LineDrawMode::Chain => {}
            },
            _ => {}
        }
        Some(
            out.into_iter()
                .map(|e| if e.is_construction() { e } else { e.with_construction(cons) })
                .collect(),
        )
    }

    /// Garis render untuk garis tak hingga di `sketch`: perpanjangan ke kedua
    /// arah di sekitar pusat pandangan kamera (Paritas 1.12).
    pub fn infinite_line_overlay(&self, sketch: &Sketch, plane: &SketchPlane, world_scale: f64) -> Vec<LineVertex> {
        if sketch.infinite_lines.is_empty() {
            return Vec::new();
        }
        let around = plane.project_point_to_uv(self.camera.target);
        let half = (world_scale * 4000.0).max(1000.0);
        let mut verts = Vec::new();
        for id in &sketch.infinite_lines {
            if sketch.is_hidden(*id) {
                continue;
            }
            let Some(Entity::Line { start, end, .. }) = sketch.entities.get(*id) else {
                continue;
            };
            if let Some((a, b)) = infinite_line_span(*start, *end, around, half) {
                verts.extend(sketch_render::preview_lines(&Entity::line(a, b).with_construction(true), plane));
            }
        }
        verts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_sketch::constraint::Constraint;

    fn click_all(app: &mut DuCADApp, pts: &[DVec2]) {
        for p in pts {
            app.on_click_point(*p);
        }
    }

    fn entities(app: &DuCADApp) -> Vec<Entity> {
        app.sketch().entities.values().cloned().collect()
    }

    #[test]
    fn circle_two_and_three_point_modes() {
        let mut app = DuCADApp::new_for_test();
        app.set_tool(ToolKind::Circle);
        app.set_draw_mode_index(1);
        assert_eq!(app.required_points_now(), 2);
        click_all(&mut app, &[DVec2::ZERO, DVec2::new(10.0, 0.0)]);
        let ents = entities(&app);
        assert!(matches!(ents.as_slice(), [Entity::Circle { radius, .. }] if (*radius - 5.0).abs() < 1e-9));

        app.set_draw_mode_index(2);
        click_all(&mut app, &[DVec2::new(1.0, 0.0), DVec2::new(0.0, 1.0)]);
        assert_eq!(app.pending_points.len(), 2, "3-titik menunggu klik ketiga");
        app.on_click_point(DVec2::new(-1.0, 0.0));
        assert_eq!(app.sketch().entities.len(), 2);
        assert!(app.pending_points.is_empty());
        // Kolinear: tidak ada entitas, tidak panic.
        click_all(&mut app, &[DVec2::ZERO, DVec2::ONE, DVec2::splat(2.0)]);
        assert_eq!(app.sketch().entities.len(), 2);
    }

    #[test]
    fn two_tangent_circle_adds_tangent_constraints() {
        let mut app = DuCADApp::new_for_test();
        app.set_tool(ToolKind::Line);
        app.set_draw_mode_index(1);
        // Dua garis lewat mode titik-tengah: sumbu X dan Y.
        click_all(&mut app, &[DVec2::new(10.0, 0.0), DVec2::new(20.0, 0.0)]);
        click_all(&mut app, &[DVec2::new(0.0, 10.0), DVec2::new(0.0, 20.0)]);
        assert_eq!(app.sketch().entities.len(), 2);
        app.set_tool(ToolKind::Circle);
        app.set_draw_mode_index(3);
        app.sketch_modes.pick_tol = 0.5;
        // Klik di ruang kosong ditolak.
        app.on_click_point(DVec2::new(7.0, 7.0));
        assert!(app.pending_points.is_empty());
        click_all(&mut app, &[DVec2::new(5.0, 0.0), DVec2::new(0.0, 5.0), DVec2::new(3.0, 5.0)]);
        let circle = entities(&app).into_iter().find(|e| matches!(e, Entity::Circle { .. }));
        let Some(Entity::Circle { center, radius, .. }) = circle else { panic!("lingkaran tidak dibuat") };
        assert!((radius - 4.0).abs() < 1e-6 && (center - DVec2::splat(4.0)).length() < 1e-6, "{center:?} {radius}");
        let tangents = app.sketch().constraints.iter().filter(|c| matches!(c, Constraint::Tangent { .. })).count();
        assert_eq!(tangents, 2);
    }

    #[test]
    fn three_tangent_circle_is_incircle_and_constrained() {
        let mut app = DuCADApp::new_for_test();
        let tri = vec![
            Entity::line(DVec2::ZERO, DVec2::new(4.0, 0.0)),
            Entity::line(DVec2::new(4.0, 0.0), DVec2::new(0.0, 3.0)),
            Entity::line(DVec2::new(0.0, 3.0), DVec2::ZERO),
        ];
        app.execute_sketch_command(Box::new(InsertEntities::new("Segitiga", tri)));
        app.set_tool(ToolKind::Circle);
        app.set_draw_mode_index(4);
        app.sketch_modes.pick_tol = 0.2;
        click_all(&mut app, &[DVec2::new(2.0, 0.0), DVec2::new(2.0, 1.5), DVec2::new(0.0, 1.5)]);
        let Some(Entity::Circle { center, radius, .. }) =
            entities(&app).into_iter().find(|e| matches!(e, Entity::Circle { .. }))
        else {
            panic!("lingkaran tidak dibuat")
        };
        assert!((radius - 1.0).abs() < 1e-6 && (center - DVec2::ONE).length() < 1e-6);
        assert_eq!(app.sketch().constraints.len(), 3);
    }

    #[test]
    fn tangent_arc_snaps_to_line_end_and_adds_tangent() {
        let mut app = DuCADApp::new_for_test();
        app.execute_sketch_command(Box::new(InsertEntities::new(
            "Garis",
            vec![Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0))],
        )));
        app.set_tool(ToolKind::Arc);
        app.set_draw_mode_index(2);
        assert_eq!(app.required_points_now(), 2);
        app.on_click_point(DVec2::new(5.0, 3.0));
        assert!(app.pending_points.is_empty(), "awal busur harus di ujung entitas");
        click_all(&mut app, &[DVec2::new(10.1, 0.0), DVec2::new(15.0, 5.0)]);
        let Some(Entity::Arc { center, radius, .. }) =
            entities(&app).into_iter().find(|e| matches!(e, Entity::Arc { .. }))
        else {
            panic!("busur tidak dibuat")
        };
        assert!((center - DVec2::new(10.0, 5.0)).length() < 1e-6 && (radius - 5.0).abs() < 1e-6);
        // Tangent + Coincident (ujung akhir garis ↔ ujung awal busur).
        assert!(matches!(
            app.sketch().constraints.as_slice(),
            [
                Constraint::Tangent { .. },
                Constraint::Coincident {
                    a: ducad_sketch::constraint::PointRef::LineEnd(_),
                    b: ducad_sketch::constraint::PointRef::LineStart(_),
                }
            ]
        ));
    }

    #[test]
    fn three_point_and_rounded_rect_modes() {
        let mut app = DuCADApp::new_for_test();
        app.set_tool(ToolKind::Rectangle);
        app.set_draw_mode_index(2);
        click_all(&mut app, &[DVec2::ZERO, DVec2::new(3.0, 3.0), DVec2::new(-1.0, 1.0)]);
        assert_eq!(app.sketch().entities.len(), 4);
        app.set_draw_mode_index(3);
        app.sketch_modes.corner_radius = 1.0;
        click_all(&mut app, &[DVec2::new(20.0, 0.0), DVec2::new(30.0, 6.0)]);
        assert_eq!(app.sketch().entities.len(), 12);
        let c = &app.sketch().constraints;
        assert_eq!(c.iter().filter(|c| matches!(c, Constraint::Tangent { .. })).count(), 8);
        assert_eq!(c.iter().filter(|c| matches!(c, Constraint::EqualRadius { .. })).count(), 3);
        // Radius terlalu besar: gagal dengan pesan, tidak panic.
        app.sketch_modes.corner_radius = 100.0;
        app.model_status = None;
        click_all(&mut app, &[DVec2::new(40.0, 0.0), DVec2::new(50.0, 6.0)]);
        assert_eq!(app.sketch().entities.len(), 12);
        assert!(app.model_status.is_some());
    }

    #[test]
    fn center_point_and_arc_slot_modes() {
        let mut app = DuCADApp::new_for_test();
        app.set_tool(ToolKind::Slot);
        app.slot_mode = SlotMode::CenterPoint;
        click_all(&mut app, &[DVec2::ZERO, DVec2::new(5.0, 0.0), DVec2::new(0.0, 1.0)]);
        let arcs: Vec<DVec2> = entities(&app)
            .into_iter()
            .filter_map(|e| match e {
                Entity::Arc { center, .. } => Some(center),
                _ => None,
            })
            .collect();
        assert!(arcs.contains(&DVec2::new(-5.0, 0.0)) && arcs.contains(&DVec2::new(5.0, 0.0)));

        let mut app = DuCADApp::new_for_test();
        app.set_tool(ToolKind::Slot);
        app.slot_mode = SlotMode::ArcCenter;
        assert_eq!(app.required_points_now(), 4);
        click_all(&mut app, &[DVec2::ZERO, DVec2::new(10.0, 0.0), DVec2::new(0.0, 10.0)]);
        assert_eq!(app.pending_points.len(), 3);
        app.on_click_point(DVec2::new(11.0, 0.0));
        assert_eq!(app.sketch().entities.len(), 4);

        app.slot_mode = SlotMode::ArcThreePoint;
        let r = 10.0 / 2f64.sqrt();
        click_all(&mut app, &[DVec2::new(10.0, 0.0), DVec2::new(r, r), DVec2::new(0.0, 10.0), DVec2::new(0.0, 9.0)]);
        assert_eq!(app.sketch().entities.len(), 8);
    }

    #[test]
    fn edge_polygon_mode_builds_from_one_side() {
        let mut app = DuCADApp::new_for_test();
        app.set_tool(ToolKind::Polygon);
        app.polygon_mode = ducad_sketch::PolygonMode::Edge;
        app.polygon_sides = 4;
        click_all(&mut app, &[DVec2::ZERO, DVec2::new(2.0, 0.0)]);
        let ents = entities(&app);
        assert_eq!(ents.len(), 4);
        assert!(ents.iter().flat_map(|e| e.endpoints()).any(|p| (p - DVec2::new(2.0, 2.0)).length() < 1e-9));
    }

    #[test]
    fn midpoint_and_infinite_line_modes() {
        let mut app = DuCADApp::new_for_test();
        app.set_tool(ToolKind::Line);
        app.set_draw_mode_index(1);
        assert!(app.line_uses_two_clicks());
        click_all(&mut app, &[DVec2::ZERO, DVec2::new(3.0, 0.0)]);
        assert!(matches!(entities(&app).as_slice(), [Entity::Line { start, .. }] if *start == DVec2::new(-3.0, 0.0)));

        app.set_draw_mode_index(2);
        click_all(&mut app, &[DVec2::new(0.0, 5.0), DVec2::new(1.0, 6.0)]);
        let sk = app.sketch();
        assert_eq!(sk.infinite_lines.len(), 1);
        let id = sk.infinite_lines.iter().next().copied().unwrap();
        assert!(sk.entities.get(id).unwrap().is_construction(), "garis tak hingga wajib konstruksi");
        // Tidak menyumbang region: hanya garis konstruksi + satu garis biasa.
        assert!(ducad_sketch::find_closed_regions(sk).is_empty());
        let plane = SketchPlane::top();
        assert!(!app.infinite_line_overlay(sk, &plane, 1.0).is_empty());
        // Ganti mode membuang titik tertunda.
        app.on_click_point(DVec2::ONE);
        app.set_draw_mode_index(0);
        assert!(app.pending_points.is_empty());
    }

    #[test]
    fn previews_exist_for_custom_modes_only() {
        let mut app = DuCADApp::new_for_test();
        app.set_tool(ToolKind::Circle);
        assert!(app.mode_preview_entities(DVec2::ONE).is_none());
        app.set_draw_mode_index(1);
        app.on_click_point(DVec2::ZERO);
        assert_eq!(app.mode_preview_entities(DVec2::new(2.0, 0.0)).map(|v| v.len()), Some(1));
        // NaN dari kursor tidak panic.
        let _ = app.mode_preview_entities(DVec2::new(f64::NAN, 0.0));
    }
}
