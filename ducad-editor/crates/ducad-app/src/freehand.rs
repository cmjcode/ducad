//! Alat "Freehand" (P12.3): coretan Pencil/mouse → bentuk rapi
//! ber-constraint. Pengenalan bentuk ada di `ducad_sketch::recognize`,
//! inferensi constraint di `ducad_sketch::infer`; modul ini hanya merangkai
//! keduanya dengan input dan undo GUI.
//!
//! Commit = SATU langkah undo: `InsertEntities` + `AddConstraint` × n
//! dibungkus satu transaksi.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use ducad_sketch::constraint::commands::AddConstraint;
use ducad_sketch::constraint::types::Constraint;
use ducad_sketch::infer::{infer_constraints, InferOptions};
use ducad_sketch::recognize::{recognize_with, to_entities, RecognizeOptions, Recognized, Stroke};
use ducad_sketch::{Entity, EntityId, InsertEntities};
use glam::DVec2;

use crate::app::DuCADApp;

/// Nama grup penanda entitas hasil coretan pensil (Freehand, Bentuk
/// Pintar, sisa tinta). Tombol "Objek Tertutup" HANYA memproses entitas
/// bergrup ini, sehingga objek CAD lain di kanvas tidak pernah tersentuh.
/// Tersimpan di `Sketch::entity_names` dan ikut dipulihkan saat undo.
pub const PENCIL_GROUP: &str = "Coretan Pensil";

/// Jeda sebelum usulan diterima otomatis.
pub const AUTO_ACCEPT: Duration = Duration::from_millis(600);

/// Usulan hasil satu coretan, menunggu diterima/ditolak.
pub struct FreehandPreview {
    /// Coretan mentah (koordinat bidang sketch).
    pub raw: Vec<DVec2>,
    pub shape: Recognized,
    pub entities: Vec<Entity>,
    /// Constraint antar entitas baru, dan antara entitas baru dengan
    /// entitas lama (snap ke ujung yang sudah ada).
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
        self.freehand_recognize(points, pressure);
    }

    /// Kenali satu coretan utuh dan pasang usulan. `true` bila ada usulan.
    /// Dipakai tool Freehand dan opsi kuas "Bentuk Pintar" mode Tinta.
    pub fn freehand_recognize(&mut self, points: Vec<DVec2>, pressure: Vec<f32>) -> bool {
        if points.len() < 2 {
            return false;
        }
        let stroke = Stroke {
            points: points.clone(),
            pressure,
        };
        // Saat menggambar coretan TIDAK ditutup otomatis; penutupan hanya
        // lewat tombol HUD "Objek Tertutup".
        let opt = RecognizeOptions {
            close_shapes: false,
        };
        let Some(shape) = recognize_with(&stroke, &opt) else {
            return false;
        };
        let entities = to_entities(&shape);
        if entities.is_empty() {
            return false;
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
        let existing: HashSet<EntityId> = self.sketch().entities.keys().collect();
        self.freehand.preview = Some(FreehandPreview {
            raw: points,
            shape,
            entities: if solved.len() == entities.len() {
                solved
            } else {
                entities
            },
            constraints: keep_resolvable(&inferred.accepted, &new_ids, &existing),
            trial_ids: new_ids,
            created: Instant::now(),
        });
        true
    }

    /// mm per piksel layar pada zoom saat ini (nilai aman bila viewport
    /// belum pernah digambar).
    pub(crate) fn mm_per_px(&self) -> f64 {
        let rect = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(self.last_viewport_size[0], self.last_viewport_size[1]),
        );
        let v = crate::viewport::pixel_tolerance_to_world(&self.camera, rect);
        if v.is_finite() && v > 0.0 {
            v
        } else {
            0.25
        }
    }

    /// `snap_dist` = 8 px dalam mm pada zoom saat ini.
    pub(crate) fn freehand_snap_dist(&self) -> f64 {
        (self.mm_per_px() * 8.0).max(0.2)
    }

    /// Sisipkan entitas lewat command dan kembalikan id-nya SEJAJAR dengan
    /// `entities`. Id dicocokkan lewat kesamaan geometri, bukan urutan kunci
    /// slotmap — slot bekas entitas yang dihapus bisa dipakai ulang sehingga
    /// urutan kunci tidak sama dengan urutan sisip.
    pub fn insert_entities_tracked(
        &mut self,
        label: &'static str,
        entities: Vec<Entity>,
    ) -> Vec<EntityId> {
        self.insert_entities_tracked_in(label, entities, None)
    }

    /// Seperti [`Self::insert_entities_tracked`], dengan nama grup opsional.
    pub fn insert_entities_tracked_in(
        &mut self,
        label: &'static str,
        entities: Vec<Entity>,
        group: Option<&str>,
    ) -> Vec<EntityId> {
        if entities.is_empty() {
            return Vec::new();
        }
        let before: HashSet<EntityId> = self.sketch().entities.keys().collect();
        let cmd = match group {
            Some(g) => InsertEntities::with_group(label, entities.clone(), g),
            None => InsertEntities::new(label, entities.clone()),
        };
        self.execute_sketch_command(Box::new(cmd));
        let mut fresh: Vec<EntityId> = self
            .sketch()
            .entities
            .keys()
            .filter(|id| !before.contains(id))
            .collect();
        let mut out = Vec::with_capacity(entities.len());
        for e in &entities {
            let pos = fresh
                .iter()
                .position(|id| self.sketch().entities.get(*id) == Some(e))
                .unwrap_or(0);
            if pos < fresh.len() {
                out.push(fresh.remove(pos));
            }
        }
        out
    }

    /// Terima usulan: SATU langkah undo berisi entitas + constraint, ditandai
    /// grup [`PENCIL_GROUP`]. Sengaja tidak menutup celah atau memilih apa
    /// pun: pengguna menggambar sampai selesai, lalu menekan "Objek
    /// Tertutup" di HUD.
    pub fn freehand_accept(&mut self) {
        let Some(preview) = self.freehand.preview.take() else {
            return;
        };
        self.sketch_set.active_mut().undo.begin("Freehand");
        let new_ids = self.insert_entities_tracked_in(
            "Freehand",
            preview.entities.clone(),
            Some(PENCIL_GROUP),
        );
        for c in retarget(&preview.constraints, &preview.trial_ids, &new_ids) {
            self.execute_sketch_command(Box::new(AddConstraint::new(c)));
        }
        self.sketch_set.active_mut().undo.commit();
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

/// Pertahankan constraint yang semua entitasnya adalah entitas baru ATAU
/// entitas lama yang ada di sketch sungguhan (mis. Coincident ke ujung garis
/// yang sudah digambar) — tanpa ikatan ini profil multi-coretan terbuka lagi
/// begitu salah satu entitasnya digeser.
fn keep_resolvable(
    constraints: &[Constraint],
    new_ids: &[EntityId],
    existing: &HashSet<EntityId>,
) -> Vec<Constraint> {
    constraints
        .iter()
        .filter(|c| {
            constraint_entities(c)
                .iter()
                .all(|id| new_ids.contains(id) || existing.contains(id))
        })
        .cloned()
        .collect()
}

/// Petakan id sketch percobaan ke id sungguhan (posisi dalam `trial` →
/// posisi dalam `real`); id entitas lama tidak berubah. Constraint yang
/// menyentuh id percobaan tanpa pasangan dibuang.
pub(crate) fn retarget(
    constraints: &[Constraint],
    trial: &[EntityId],
    real: &[EntityId],
) -> Vec<Constraint> {
    constraints
        .iter()
        .filter(|c| {
            constraint_entities(c).iter().all(|id| {
                trial
                    .iter()
                    .position(|x| x == id)
                    .is_none_or(|i| i < real.len())
            })
        })
        .map(|c| {
            let mut c = c.clone();
            c.map_entity_ids(|id| {
                trial
                    .iter()
                    .position(|x| *x == id)
                    .and_then(|i| real.get(i).copied())
                    .unwrap_or(id)
            });
            c
        })
        .collect()
}

/// Id entitas yang dirujuk sebuah constraint.
pub(crate) fn constraint_entities(c: &Constraint) -> Vec<EntityId> {
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

    fn draw(app: &mut DuCADApp, pts: &[DVec2]) {
        for p in pts {
            app.freehand_push(*p, None);
        }
        app.freehand_finish();
        assert!(app.freehand.preview.is_some(), "coretan dikenali");
        app.freehand_accept();
    }

    #[test]
    fn stroke_keeps_coincident_with_existing_entity() {
        let mut app = DuCADApp::new_for_test();
        app.set_tool(ToolKind::Freehand);
        let first: Vec<DVec2> = (0..=40).map(|i| DVec2::new(i as f64, 0.0)).collect();
        draw(&mut app, &first);
        let old: Vec<EntityId> = app.sketch().entities.keys().collect();
        assert_eq!(old.len(), 1);
        // Garis kedua mulai 0,6 mm dari ujung garis pertama.
        let second: Vec<DVec2> = (0..=40)
            .map(|i| DVec2::new(40.4, 0.4 + i as f64 * 0.6))
            .collect();
        draw(&mut app, &second);
        let touches_old = app.sketch().constraints.iter().any(|c| {
            let ids = constraint_entities(c);
            matches!(c, Constraint::Coincident { .. })
                && ids.contains(&old[0])
                && ids.iter().any(|id| *id != old[0])
        });
        assert!(touches_old, "{:?}", app.sketch().constraints);
    }

    #[test]
    fn tracked_insert_survives_reused_slots() {
        let mut app = DuCADApp::new_for_test();
        let a = app.insert_entities_tracked(
            "Line",
            (0..4)
                .map(|i| Entity::line(DVec2::new(i as f64, 0.0), DVec2::new(i as f64, 5.0)))
                .collect(),
        );
        // Hapus dua entitas di tengah → slot bebas dipakai ulang.
        app.execute_sketch_command(Box::new(ducad_sketch::DeleteEntities::new(vec![a[1], a[2]])));
        let ents: Vec<Entity> = (0..3)
            .map(|i| Entity::line(DVec2::new(100.0 + i as f64, 0.0), DVec2::new(100.0, 9.0)))
            .collect();
        let b = app.insert_entities_tracked("Line", ents.clone());
        assert_eq!(b.len(), 3);
        for (id, e) in b.iter().zip(&ents) {
            assert_eq!(app.sketch().entities.get(*id), Some(e));
        }
    }

    #[test]
    fn invalid_profile_shows_error_instead_of_wireframe_body() {
        let mut app = DuCADApp::new_for_test();
        // Spline tertutup "pita" yang memotong dirinya, disisipkan langsung
        // (melewati pengenal bentuk).
        let ids = app.insert_entities_tracked(
            "Spline",
            vec![Entity::spline(vec![
                DVec2::new(0.0, 0.0),
                DVec2::new(30.0, 20.0),
                DVec2::new(30.0, 0.0),
                DVec2::new(0.0, 20.0),
                DVec2::new(0.0, 0.0),
            ])],
        );
        app.selected = ids.into_iter().collect();
        app.gizmo_distance = 10.0;
        app.commit_gizmo_extrusion();
        let bad_bodies = app
            .model
            .geometry
            .iter()
            .filter(|(_, g)| !g.shape.is_valid() || g.mesh.triangle_count() == 0)
            .count();
        assert_eq!(bad_bodies, 0, "tidak boleh ada body kerangka");
        if app.model.doc.bodies.is_empty() {
            assert!(app.error_card.open, "kegagalan harus terlihat");
        }
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
