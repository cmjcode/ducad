//! Alat "Freehand" (P12.3): coretan Pencil/mouse → bentuk rapi
//! ber-constraint. Pengenalan bentuk ada di `ducad_sketch::recognize`,
//! inferensi constraint di `ducad_sketch::infer`; modul ini hanya merangkai
//! keduanya dengan input dan undo GUI.
//!
//! Commit = SATU langkah undo: `InsertEntities` + `AddConstraint` × n
//! dibungkus satu transaksi.

use std::time::{Duration, Instant};

use ducad_sketch::constraint::commands::AddConstraint;
use ducad_sketch::constraint::types::Constraint;
use ducad_sketch::infer::{infer_constraints, InferOptions};
use ducad_sketch::recognize::{recognize, to_entities, Recognized, Stroke};
use ducad_sketch::{Entity, EntityId, InsertEntities};
use glam::DVec2;

use crate::app::DuCADApp;

/// Jeda sebelum usulan diterima otomatis.
pub const AUTO_ACCEPT: Duration = Duration::from_millis(600);

/// Usulan hasil satu coretan, menunggu diterima/ditolak.
pub struct FreehandPreview {
    /// Coretan mentah (koordinat bidang sketch).
    pub raw: Vec<DVec2>,
    pub shape: Recognized,
    pub entities: Vec<Entity>,
    /// Constraint antar entitas baru saja.
    pub constraints: Vec<Constraint>,
    /// Id entitas di sketch PERCOBAAN, sejajar dengan `entities`; dipakai
    /// memetakan id di `constraints` ke id sungguhan saat commit.
    pub trial_ids: Vec<EntityId>,
    pub created: Instant,
}

#[derive(Default)]
pub struct FreehandState {
    /// Coretan yang sedang digambar.
    pub stroke: Vec<DVec2>,
    pub pressure: Vec<f32>,
    pub preview: Option<FreehandPreview>,
    /// Terima otomatis setelah [`AUTO_ACCEPT`].
    pub auto_accept: bool,
}

impl FreehandState {
    pub fn new() -> Self {
        Self {
            auto_accept: true,
            ..Default::default()
        }
    }
}

impl DuCADApp {
    /// Titik baru pada coretan yang sedang berjalan.
    pub fn freehand_push(&mut self, p: DVec2, force: Option<f32>) {
        if self
            .freehand
            .stroke
            .last()
            .is_none_or(|q| (*q - p).length() > 1e-6)
        {
            self.freehand.stroke.push(p);
            if let Some(f) = force {
                self.freehand.pressure.push(f);
            }
        }
    }

    /// Pointer naik: kenali bentuk lalu infer constraint (tanpa mengubah
    /// sketch). Coretan yang tidak dikenali dibuang diam-diam.
    pub fn freehand_finish(&mut self) {
        let points = std::mem::take(&mut self.freehand.stroke);
        let pressure = std::mem::take(&mut self.freehand.pressure);
        if points.len() < 2 {
            return;
        }
        let stroke = Stroke {
            points: points.clone(),
            pressure,
        };
        let Some(shape) = recognize(&stroke) else {
            return;
        };
        let entities = to_entities(&shape);
        if entities.is_empty() {
            return;
        }
        // Infer pada SALINAN sketch: entitas asli baru masuk saat commit.
        let mut trial = self.sketch().clone();
        let new_ids: Vec<EntityId> = entities
            .iter()
            .map(|e| trial.entities.insert(e.clone()))
            .collect();
        let opt = InferOptions {
            snap_dist: self.freehand_snap_dist(),
            ..InferOptions::default()
        };
        let inferred = infer_constraints(&trial, &new_ids, &opt);
        // Geometri hasil solve dipakai sebagai bentuk final.
        let solved: Vec<Entity> = new_ids
            .iter()
            .filter_map(|id| inferred.sketch.entities.get(*id).cloned())
            .collect();
        self.freehand.preview = Some(FreehandPreview {
            raw: points,
            shape,
            entities: if solved.len() == entities.len() {
                solved
            } else {
                entities
            },
            // Hanya constraint antar entitas BARU yang dibawa: id entitas
            // lama tetap sah, tetapi id percobaan tidak, jadi yang menyentuh
            // entitas percobaan lain dibuang.
            constraints: keep_new_only(&inferred.accepted, &new_ids),
            trial_ids: new_ids,
            created: Instant::now(),
        });
    }

    /// `snap_dist` = 8 px dalam mm pada zoom saat ini.
    fn freehand_snap_dist(&self) -> f64 {
        let rect = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(self.last_viewport_size[0], self.last_viewport_size[1]),
        );
        (crate::viewport::pixel_tolerance_to_world(&self.camera, rect) * 8.0).max(0.2)
    }

    /// Terima usulan: satu langkah undo berisi entitas + constraint.
    pub fn freehand_accept(&mut self) {
        let Some(preview) = self.freehand.preview.take() else {
            return;
        };
        let before: Vec<EntityId> = self.sketch().entities.keys().collect();
        {
            let slot = self.sketch_set.active_mut();
            slot.undo.begin("Freehand");
        }
        self.execute_sketch_command(Box::new(InsertEntities::new(
            "Freehand",
            preview.entities.clone(),
        )));
        let new_ids: Vec<EntityId> = self
            .sketch()
            .entities
            .keys()
            .filter(|id| !before.contains(id))
            .collect();
        for c in retarget(&preview.constraints, &preview.trial_ids, &new_ids) {
            self.execute_sketch_command(Box::new(AddConstraint::new(c)));
        }
        {
            let slot = self.sketch_set.active_mut();
            slot.undo.commit();
        }
        self.record_activity(
            ducad_ui::ActivityKindUi::Sketch2D,
            &ducad_i18n::t!("tool-freehand"),
            &ducad_i18n::t!("freehand-committed"),
        );
    }

    pub fn freehand_reject(&mut self) {
        self.freehand.preview = None;
        self.freehand.stroke.clear();
        self.freehand.pressure.clear();
    }

    /// Dipanggil tiap frame: terima otomatis setelah [`AUTO_ACCEPT`].
    pub fn freehand_tick(&mut self) {
        let due = self
            .freehand
            .preview
            .as_ref()
            .is_some_and(|p| p.created.elapsed() >= AUTO_ACCEPT);
        if due && self.freehand.auto_accept {
            self.freehand_accept();
        }
    }
}

/// Buang constraint yang menyentuh entitas di luar `new_ids` — id entitas
/// percobaan tidak ada di sketch sungguhan.
fn keep_new_only(constraints: &[Constraint], new_ids: &[EntityId]) -> Vec<Constraint> {
    constraints
        .iter()
        .filter(|c| constraint_entities(c).iter().all(|id| new_ids.contains(id)))
        .cloned()
        .collect()
}

/// Petakan id sketch percobaan ke id sungguhan lewat POSISI entitas:
/// `InsertEntities` menyisipkan dengan urutan yang sama seperti `trial`.
fn retarget(constraints: &[Constraint], trial: &[EntityId], real: &[EntityId]) -> Vec<Constraint> {
    let map = |id: EntityId| -> Option<EntityId> {
        trial.iter().position(|x| *x == id).and_then(|i| real.get(i)).copied()
    };
    constraints.iter().filter_map(|c| remap(c, &map)).collect()
}

fn remap(c: &Constraint, map: &impl Fn(EntityId) -> Option<EntityId>) -> Option<Constraint> {
    use ducad_sketch::constraint::types::PointRef;
    let pt = |p: &PointRef| -> Option<PointRef> {
        let id = map(p.entity_id())?;
        Some(match p {
            PointRef::LineStart(_) => PointRef::LineStart(id),
            PointRef::LineEnd(_) => PointRef::LineEnd(id),
            PointRef::Center(_) => PointRef::Center(id),
            PointRef::PathNode { sub, node, .. } => PointRef::PathNode {
                id,
                sub: *sub,
                node: *node,
            },
        })
    };
    Some(match c {
        Constraint::Coincident { a, b } => Constraint::Coincident {
            a: pt(a)?,
            b: pt(b)?,
        },
        Constraint::Horizontal { line } => Constraint::Horizontal { line: map(*line)? },
        Constraint::Vertical { line } => Constraint::Vertical { line: map(*line)? },
        Constraint::HorizontalPoints { a, b } => Constraint::HorizontalPoints {
            a: pt(a)?,
            b: pt(b)?,
        },
        Constraint::VerticalPoints { a, b } => Constraint::VerticalPoints {
            a: pt(a)?,
            b: pt(b)?,
        },
        Constraint::Parallel { a, b } => Constraint::Parallel {
            a: map(*a)?,
            b: map(*b)?,
        },
        Constraint::Perpendicular { a, b } => Constraint::Perpendicular {
            a: map(*a)?,
            b: map(*b)?,
        },
        Constraint::EqualLength { a, b } => Constraint::EqualLength {
            a: map(*a)?,
            b: map(*b)?,
        },
        Constraint::EqualRadius { a, b } => Constraint::EqualRadius {
            a: map(*a)?,
            b: map(*b)?,
        },
        Constraint::Tangent { a, b } => Constraint::Tangent {
            a: map(*a)?,
            b: map(*b)?,
        },
        Constraint::Concentric { a, b } => Constraint::Concentric {
            a: map(*a)?,
            b: map(*b)?,
        },
        Constraint::PointOnCurve { point, curve } => Constraint::PointOnCurve {
            point: pt(point)?,
            curve: map(*curve)?,
        },
        other => other.clone(),
    })
}

/// Id entitas yang dirujuk sebuah constraint.
fn constraint_entities(c: &Constraint) -> Vec<EntityId> {
    match c {
        Constraint::Coincident { a, b }
        | Constraint::HorizontalPoints { a, b }
        | Constraint::VerticalPoints { a, b } => vec![a.entity_id(), b.entity_id()],
        Constraint::Horizontal { line } | Constraint::Vertical { line } => vec![*line],
        Constraint::Parallel { a, b }
        | Constraint::Perpendicular { a, b }
        | Constraint::EqualLength { a, b }
        | Constraint::EqualRadius { a, b }
        | Constraint::Tangent { a, b }
        | Constraint::Concentric { a, b }
        | Constraint::Collinear { a, b }
        | Constraint::Angle { a, b, .. } => vec![*a, *b],
        Constraint::Fixed { point, .. } => vec![point.entity_id()],
        Constraint::Distance { a, b, .. } => vec![a.entity_id(), b.entity_id()],
        Constraint::Radius { entity, .. } => vec![*entity],
        Constraint::Symmetric { a, b, axis } => vec![a.entity_id(), b.entity_id(), *axis],
        Constraint::PointOnCurve { point, curve } => vec![point.entity_id(), *curve],
        Constraint::Midpoint { point, line } => vec![point.entity_id(), *line],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ToolKind;

    fn square_stroke() -> Vec<DVec2> {
        let corners = [
            DVec2::new(-15.0, -10.0),
            DVec2::new(15.0, -10.2),
            DVec2::new(15.1, 10.0),
            DVec2::new(-14.9, 10.1),
        ];
        let mut pts = Vec::new();
        for e in 0..4 {
            let (a, b) = (corners[e], corners[(e + 1) % 4]);
            for i in 0..40 {
                pts.push(a + (b - a) * (i as f64 / 40.0));
            }
        }
        pts.push(corners[0]);
        pts
    }

    #[test]
    fn stroke_becomes_lines_with_constraints_in_one_undo() {
        let mut app = DuCADApp::new_for_test();
        app.set_tool(ToolKind::Freehand);
        for p in square_stroke() {
            app.freehand_push(p, None);
        }
        app.freehand_finish();
        let preview = app.freehand.preview.as_ref().expect("ada usulan");
        assert_eq!(preview.entities.len(), 4);
        assert!(!preview.constraints.is_empty());

        app.freehand_accept();
        assert_eq!(app.sketch().entities.len(), 4);
        assert!(!app.sketch().constraints.is_empty());

        assert!(app.sketch_set.undo().is_some(), "satu undo");
        assert_eq!(app.sketch().entities.len(), 0, "sketch kosong lagi");
    }

    #[test]
    fn rejected_stroke_leaves_sketch_untouched() {
        let mut app = DuCADApp::new_for_test();
        app.set_tool(ToolKind::Freehand);
        for p in square_stroke() {
            app.freehand_push(p, None);
        }
        app.freehand_finish();
        assert!(app.freehand.preview.is_some());
        app.freehand_reject();
        assert!(app.freehand.preview.is_none());
        assert_eq!(app.sketch().entities.len(), 0);
    }

    #[test]
    fn tap_is_ignored() {
        let mut app = DuCADApp::new_for_test();
        app.set_tool(ToolKind::Freehand);
        for p in [DVec2::ZERO, DVec2::new(0.05, 0.02), DVec2::new(0.03, 0.04)] {
            app.freehand_push(p, None);
        }
        app.freehand_finish();
        assert!(app.freehand.preview.is_none());
        assert_eq!(app.sketch().entities.len(), 0);
    }
}
