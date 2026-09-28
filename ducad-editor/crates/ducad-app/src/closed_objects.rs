//! Tombol HUD "Objek Tertutup": sketsa bebas → objek tertutup siap
//! extrude, dijalankan SETELAH pengguna selesai menggambar.
//!
//! HANYA coretan pensil (grup [`PENCIL_GROUP`]) yang diproses — objek CAD
//! lain di kanvas (garis/lingkaran CAD, teks, profil fitur 3D) tidak pernah
//! diubah.
//!
//! - Mode Sketsa CAD (alat Freehand): coretan pensil terpilih (atau semua)
//!   dipecah oleh `ducad_sketch::build_closed_objects`. Coretan yang ikut
//!   membentuk wilayah diganti objek baru; bentuk yang sudah rapi dibiarkan.
//! - Mode Tinta: coretan terpilih (atau semua) lebih dulu dikenali menjadi
//!   entitas (`ducad_ink::vectorize`), lalu dipecah dengan cara yang sama.
//!   Tinta kasar disembunyikan dan aplikasi pindah ke Mode Sketsa CAD.
//!
//! Wilayah yang saling memotong menjadi objek TERPISAH — tidak ada bagian
//! sketsa yang dibuang kecuali ekor yang tidak membatasi wilayah apa pun.

use std::collections::HashSet;

use ducad_ink::{vectorize_strokes, SetStrokesHidden, Stroke, VectorizeOptions};
use ducad_sketch::constraint::commands::AddConstraint;
use ducad_sketch::{build_closed_objects, DeleteEntities, Entity, EntityId, FaceOptions};

use crate::app::DuCADApp;
use crate::freehand::{constraint_entities, retarget, PENCIL_GROUP};
use crate::mode::AppMode;
use crate::types::ToolKind;

/// Celah (piksel layar) antar ujung yang masih disambung.
pub const SNAP_PX: f64 = 14.0;

/// State tombol "Objek Tertutup".
#[derive(Debug, Default)]
pub struct ClosedObjectsState {
    /// Pesan hasil konversi terakhir untuk status bar.
    pub notice: Option<String>,
}

/// Ringkasan satu konversi.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ConvertReport {
    /// Objek tertutup baru.
    pub objects: usize,
    /// Wilayah yang sudah rapi dan dibiarkan.
    pub kept_clean: usize,
    /// Coretan tinta yang dikonversi (Mode Tinta).
    pub strokes: usize,
    /// Id objek baru di sketch.
    pub new_ids: Vec<EntityId>,
}

impl DuCADApp {
    /// Coretan pensil di sketch aktif; bila ada seleksi, hanya yang terpilih.
    fn pencil_scope(&self) -> Vec<EntityId> {
        let sketch = self.sketch();
        let mut v: Vec<EntityId> = sketch
            .entities
            .keys()
            .filter(|id| sketch.entity_names.get(id).map(String::as_str) == Some(PENCIL_GROUP))
            .filter(|id| self.selected.is_empty() || self.selected.contains(id))
            .collect();
        v.sort();
        v
    }

    fn closed_object_options(&self) -> FaceOptions {
        FaceOptions {
            snap: (self.mm_per_px() * SNAP_PX).clamp(0.5, 25.0),
            ..FaceOptions::default()
        }
    }

    /// Aksi tombol HUD "Objek Tertutup".
    pub fn convert_to_closed_objects(&mut self) -> ConvertReport {
        // Usulan Freehand yang belum diterima ikut dikonversi.
        self.freehand_accept();
        let report = if self.app_mode == AppMode::Ink {
            self.convert_ink_to_closed_objects()
        } else {
            self.convert_sketch_to_closed_objects()
        };
        self.closed_objects.notice = Some(if report.objects > 0 {
            ducad_i18n::t!("close-objects-done", count = report.objects)
        } else if report.kept_clean > 0 {
            ducad_i18n::t!("close-objects-kept")
        } else if self.app_mode == AppMode::Ink {
            ducad_i18n::t!("ink-to-profile-empty")
        } else {
            ducad_i18n::t!("close-objects-none")
        });
        if report.objects > 0 {
            self.record_activity(
                ducad_ui::ActivityKindUi::Sketch2D,
                &ducad_i18n::t!("hud-close-objects"),
                &ducad_i18n::t!("close-objects-done", count = report.objects),
            );
        }
        report
    }

    fn convert_sketch_to_closed_objects(&mut self) -> ConvertReport {
        let ids = self.pencil_scope();
        let r = build_closed_objects(self.sketch(), &ids, &self.closed_object_options());
        let mut report = ConvertReport {
            kept_clean: r.kept_clean,
            ..Default::default()
        };
        if r.is_empty() {
            return report;
        }
        self.sketch_set.active_mut().undo.begin("Close Objects");
        if !r.consumed.is_empty() {
            self.execute_sketch_command(Box::new(DeleteEntities::new(r.consumed.clone())));
        }
        report.new_ids = self.insert_entities_tracked("Close Objects", r.objects);
        self.sketch_set.active_mut().undo.commit();
        report.objects = report.new_ids.len();
        self.selected.clear();
        report
    }

    /// Coretan tinta yang dikonversi: seleksi lasso, atau semua yang terlihat.
    fn ink_targets(&self) -> Vec<u64> {
        let visible = self.ink.strokes.iter().filter(|s| !s.hidden);
        if self.ink_state.selected_stroke_ids.is_empty() {
            visible.map(|s| s.id).collect()
        } else {
            visible
                .filter(|s| self.ink_state.selected_stroke_ids.contains(&s.id))
                .map(|s| s.id)
                .collect()
        }
    }

    /// Cakupan: coretan tinta (seleksi lasso, atau semua) ditambah — bila
    /// tidak ada seleksi lasso — coretan pensil yang sudah jadi entitas
    /// sketsa (mis. hasil "Bentuk Pintar").
    ///
    /// Undo: dua langkah bila ada tinta yang dikonversi — entitas sketch
    /// dulu, lalu tinta tampil lagi (tumpukan undo tinta dan sketch
    /// terpisah; urutan dijaga undo global).
    fn convert_ink_to_closed_objects(&mut self) -> ConvertReport {
        let lasso = !self.ink_state.selected_stroke_ids.is_empty();
        let targets = self.ink_targets();
        let v = {
            let strokes: Vec<&Stroke> = self
                .ink
                .strokes
                .iter()
                .filter(|s| targets.contains(&s.id))
                .collect();
            vectorize_strokes(
                self.sketch(),
                &strokes,
                &VectorizeOptions {
                    snap_dist: self.freehand_snap_dist(),
                },
            )
        };
        let existing: Vec<EntityId> = if lasso {
            Vec::new()
        } else {
            self.pencil_scope()
        };

        // Pecah wilayah di atas salinan sketch + entitas hasil pengenalan.
        let mut trial = self.sketch().clone();
        let trial_ids: Vec<EntityId> = v
            .entities
            .iter()
            .map(|e| trial.entities.insert(e.clone()))
            .collect();
        let scope: Vec<EntityId> = trial_ids.iter().chain(&existing).copied().collect();
        let r = build_closed_objects(&trial, &scope, &self.closed_object_options());
        if v.is_empty() && r.is_empty() {
            return ConvertReport {
                kept_clean: r.kept_clean,
                ..Default::default()
            };
        }
        let consumed: HashSet<EntityId> = r.consumed.iter().copied().collect();
        let consumed_existing: Vec<EntityId> = existing
            .iter()
            .copied()
            .filter(|id| consumed.contains(id))
            .collect();
        let kept: Vec<usize> = (0..trial_ids.len())
            .filter(|i| !consumed.contains(&trial_ids[*i]))
            .collect();
        let gone: HashSet<EntityId> = (0..trial_ids.len())
            .filter(|i| consumed.contains(&trial_ids[*i]))
            .map(|i| v.trial_ids[i])
            .chain(consumed_existing.iter().copied())
            .collect();
        let constraints: Vec<_> = v
            .constraints
            .iter()
            .filter(|c| constraint_entities(c).iter().all(|id| !gone.contains(id)))
            .cloned()
            .collect();

        if !v.converted.is_empty() {
            self.execute_ink_command(
                Box::new(SetStrokesHidden::new(v.converted.clone(), true)),
                "Ink to Closed Objects",
            );
        }
        self.sketch_set.active_mut().undo.begin("Close Objects");
        if !consumed_existing.is_empty() {
            self.execute_sketch_command(Box::new(DeleteEntities::new(consumed_existing)));
        }
        let kept_entities: Vec<Entity> = kept.iter().map(|i| v.entities[*i].clone()).collect();
        let kept_ids =
            self.insert_entities_tracked_in("Close Objects", kept_entities, Some(PENCIL_GROUP));
        let kept_trial: Vec<EntityId> = kept.iter().map(|i| v.trial_ids[*i]).collect();
        for c in retarget(&constraints, &kept_trial, &kept_ids) {
            self.execute_sketch_command(Box::new(AddConstraint::new(c)));
        }
        let new_ids = self.insert_entities_tracked("Close Objects", r.objects);
        self.sketch_set.active_mut().undo.commit();

        self.ink_state.selected_stroke_ids.clear();
        self.set_app_mode(AppMode::Sketch);
        self.set_tool(ToolKind::Select);
        self.selected.clear();
        ConvertReport {
            objects: new_ids.len(),
            kept_clean: r.kept_clean,
            strokes: v.converted.len(),
            new_ids,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_ink::stroke::InkPoint;
    use glam::{DVec2, Vec2};
    use std::f64::consts::{PI, TAU};

    /// Coretan angka "6": lengkung besar, lalu loop yang memotong
    /// lintasannya sendiri sebelum kembali ke titik awal.
    fn six_stroke() -> Vec<DVec2> {
        let mut pts = Vec::new();
        for i in 0..=80 {
            let a = PI * (1.0 - i as f64 / 80.0);
            pts.push(DVec2::new(40.0 * a.cos(), 20.0 * a.sin()));
        }
        for i in 0..=80 {
            let a = TAU * i as f64 / 80.0;
            pts.push(DVec2::new(15.0 + 25.0 * a.cos(), -12.0 * a.sin()));
        }
        for i in 0..=40 {
            let t = i as f64 / 40.0;
            pts.push(DVec2::new(40.0 - 80.0 * t, -15.0 * (PI * t).sin()));
        }
        pts
    }

    fn draw(app: &mut DuCADApp, pts: &[DVec2]) {
        for p in pts {
            app.freehand_push(*p, None);
        }
        app.freehand_finish();
        app.freehand_accept();
    }

    fn arc_stroke(from: f64, to: f64) -> Vec<DVec2> {
        (0..=60)
            .map(|i| DVec2::from_angle(from + (to - from) * i as f64 / 60.0) * 15.0)
            .collect()
    }

    #[test]
    fn sketching_does_not_change_strokes_until_button() {
        let mut app = DuCADApp::new_for_test();
        app.set_tool(ToolKind::Freehand);
        draw(&mut app, &arc_stroke(0.07, PI - 0.07));
        draw(&mut app, &arc_stroke(PI + 0.07, TAU - 0.07));
        assert_eq!(
            app.sketch().entities.len(),
            2,
            "tidak ada jembatan otomatis"
        );
        assert!(app.selected.is_empty(), "tidak ada seleksi otomatis");
        assert!(ducad_sketch::find_closed_regions(app.sketch()).is_empty());

        let r = app.convert_to_closed_objects();
        assert_eq!(r.objects, 1);
        assert_eq!(ducad_sketch::find_closed_regions(app.sketch()).len(), 1);
        app.selected = r.new_ids.iter().copied().collect();
        app.extrude_distance_input = "4".to_string();
        app.extrude_selected();
        assert_eq!(app.model.doc.bodies.len(), 1, "{:?}", app.model_status);
    }

    #[test]
    fn cad_objects_on_crowded_canvas_are_never_touched() {
        let mut app = DuCADApp::new_for_test();
        // Objek CAD: persegi garis, dua lingkaran BERSINGGUNGAN, teks.
        let c = [
            DVec2::new(60.0, 0.0),
            DVec2::new(90.0, 0.0),
            DVec2::new(90.0, 20.0),
            DVec2::new(60.0, 20.0),
        ];
        app.insert_entities_tracked(
            "Line",
            (0..4).map(|i| Entity::line(c[i], c[(i + 1) % 4])).collect(),
        );
        app.insert_entities_tracked(
            "Circle",
            vec![
                Entity::circle(DVec2::new(75.0, 40.0), 8.0),
                Entity::circle(DVec2::new(82.0, 40.0), 6.0),
            ],
        );
        let text = ducad_sketch::text_to_entities(
            "AB",
            DVec2::new(-60.0, 40.0),
            &Default::default(),
            None,
        )
        .unwrap_or_default();
        app.insert_entities_tracked("Text", text);
        let cad: Vec<(EntityId, Entity)> = app
            .sketch()
            .entities
            .iter()
            .map(|(id, e)| (id, e.clone()))
            .collect();

        // Coretan pensil yang saling tumpang, satu memotong persegi CAD.
        app.set_tool(ToolKind::Freehand);
        for k in 0..4 {
            let o = DVec2::new(k as f64 * 18.0 - 30.0, -30.0);
            let pts: Vec<DVec2> = (0..=90)
                .map(|i| o + DVec2::from_angle(TAU * 0.97 * i as f64 / 90.0) * 14.0)
                .collect();
            draw(&mut app, &pts);
        }
        draw(&mut app, &six_stroke());

        let r = app.convert_to_closed_objects();
        assert!(r.objects >= 6, "{r:?}");
        for (id, e) in &cad {
            assert_eq!(
                app.sketch().entities.get(*id),
                Some(e),
                "objek CAD tidak berubah"
            );
        }
        for id in &r.new_ids {
            let n0 = app.model.doc.bodies.len();
            app.selected = [*id].into_iter().collect();
            app.gizmo_distance = 8.0;
            app.commit_gizmo_extrusion();
            assert_eq!(app.model.doc.bodies.len(), n0 + 1, "{:?}", app.model_status);
        }
        assert!(app
            .model
            .geometry
            .iter()
            .all(|(_, g)| g.shape.is_valid() && g.mesh.triangle_count() > 0));
    }

    #[test]
    fn nearly_closed_stroke_stays_open_until_button() {
        let mut app = DuCADApp::new_for_test();
        app.set_tool(ToolKind::Freehand);
        // Lingkaran yang berhenti ±2 mm sebelum titik awal.
        let pts: Vec<DVec2> = (0..=90)
            .map(|i| DVec2::from_angle(TAU * 0.98 * i as f64 / 90.0) * 20.0)
            .collect();
        draw(&mut app, &pts);
        assert!(
            ducad_sketch::find_closed_regions(app.sketch()).is_empty(),
            "coretan tidak ditutup otomatis saat menggambar"
        );
        let r = app.convert_to_closed_objects();
        assert_eq!(r.objects, 1, "{r:?}");
        assert_eq!(ducad_sketch::find_closed_regions(app.sketch()).len(), 1);
    }

    #[test]
    fn self_intersecting_stroke_becomes_several_extrudable_objects() {
        let mut app = DuCADApp::new_for_test();
        app.set_tool(ToolKind::Freehand);
        draw(&mut app, &six_stroke());
        let r = app.convert_to_closed_objects();
        assert!(
            r.objects >= 2,
            "wilayah yang dipotong jadi objek terpisah: {r:?}"
        );

        // Setiap objek bisa di-extrude sendiri lewat gizmo, dan hasilnya
        // solid berpermukaan (bukan kerangka).
        for id in &r.new_ids {
            app.selected = [*id].into_iter().collect();
            app.gizmo_distance = 8.0;
            app.commit_gizmo_extrusion();
        }
        assert_eq!(
            app.model.doc.bodies.len(),
            r.objects,
            "{:?}",
            app.model_status
        );
        for (_, g) in app.model.geometry.iter() {
            assert!(g.shape.is_valid());
            assert!(g.mesh.triangle_count() > 0);
        }
    }

    #[test]
    fn conversion_is_one_undo_and_idempotent() {
        let mut app = DuCADApp::new_for_test();
        app.set_tool(ToolKind::Freehand);
        draw(&mut app, &six_stroke());
        let before = app.sketch().entities.len();
        let r = app.convert_to_closed_objects();
        assert!(r.objects >= 2);
        let after = app.sketch().entities.len();

        let again = app.convert_to_closed_objects();
        assert_eq!(again.objects, 0, "tekan dua kali tidak mengubah apa pun");
        assert_eq!(app.sketch().entities.len(), after);

        app.undo();
        assert_eq!(app.sketch().entities.len(), before, "satu langkah undo");
    }

    #[test]
    fn clean_pencil_rectangle_keeps_its_lines_and_constraints() {
        let mut app = DuCADApp::new_for_test();
        app.set_tool(ToolKind::Freehand);
        // Persegi pensil yang ujungnya kembali tepat ke titik awal.
        let c = [
            DVec2::new(0.0, 0.0),
            DVec2::new(40.0, 0.0),
            DVec2::new(40.0, 20.0),
            DVec2::new(0.0, 20.0),
        ];
        let mut pts = Vec::new();
        for e in 0..4 {
            for i in 0..40 {
                pts.push(c[e] + (c[(e + 1) % 4] - c[e]) * (i as f64 / 40.0));
            }
        }
        pts.push(c[0]);
        draw(&mut app, &pts);
        let before: Vec<Entity> = app.sketch().entities.values().cloned().collect();
        let constraints = app.sketch().constraints.len();
        let r = app.convert_to_closed_objects();
        assert_eq!(r.objects, 0, "{r:?}");
        assert_eq!(r.kept_clean, 1);
        let after: Vec<Entity> = app.sketch().entities.values().cloned().collect();
        assert_eq!(before, after);
        assert_eq!(app.sketch().constraints.len(), constraints);
        assert!(app.closed_objects.notice.is_some());
    }

    fn segment(a: Vec2, b: Vec2) -> Vec<Vec2> {
        (0..=30).map(|i| a + (b - a) * (i as f32 / 30.0)).collect()
    }

    fn add_ink(app: &mut DuCADApp, pts: &[Vec2]) {
        let points = pts
            .iter()
            .map(|p| InkPoint::new(p.x, p.y, 0.5, 0.0, 0))
            .collect();
        app.commit_ink_stroke(points);
    }

    #[test]
    fn ink_strokes_convert_and_extrude() {
        let mut app = DuCADApp::new_for_test();
        app.set_app_mode(AppMode::Ink);
        // Empat coretan terpisah, ujungnya meleset ±1 mm, plus satu garis
        // yang membelah bentuk → dua objek.
        add_ink(
            &mut app,
            &segment(Vec2::new(0.0, 0.0), Vec2::new(40.0, 0.4)),
        );
        add_ink(
            &mut app,
            &segment(Vec2::new(40.8, 1.0), Vec2::new(40.3, 25.0)),
        );
        add_ink(
            &mut app,
            &segment(Vec2::new(39.2, 25.6), Vec2::new(0.5, 25.2)),
        );
        add_ink(
            &mut app,
            &segment(Vec2::new(-0.6, 24.3), Vec2::new(-0.4, 1.1)),
        );
        add_ink(
            &mut app,
            &segment(Vec2::new(20.0, -4.0), Vec2::new(20.0, 30.0)),
        );

        let r = app.convert_to_closed_objects();
        assert_eq!(r.strokes, 5);
        assert_eq!(r.objects, 2, "{r:?}");
        assert_eq!(app.app_mode, AppMode::Sketch);
        assert!(app.ink.strokes.iter().all(|s| s.hidden));

        app.selected = [r.new_ids[0]].into_iter().collect();
        app.extrude_distance_input = "5".to_string();
        app.extrude_selected();
        assert_eq!(app.model.doc.bodies.len(), 1, "{:?}", app.model_status);

        app.undo(); // solid
        app.undo(); // entitas sketch
        assert!(app.sketch().entities.is_empty());
        app.undo(); // tinta tampil lagi
        assert!(app.ink.strokes.iter().all(|s| !s.hidden));
    }

    #[test]
    fn ink_lasso_selection_limits_conversion() {
        let mut app = DuCADApp::new_for_test();
        app.set_app_mode(AppMode::Ink);
        add_ink(
            &mut app,
            &segment(Vec2::new(0.0, 0.0), Vec2::new(30.0, 0.0)),
        );
        add_ink(
            &mut app,
            &segment(Vec2::new(0.0, 50.0), Vec2::new(30.0, 50.0)),
        );
        let first = app.ink.strokes[0].id;
        app.ink_state.selected_stroke_ids = vec![first];
        let r = app.convert_to_closed_objects();
        assert_eq!(r.strokes, 1);
        assert_eq!(app.sketch().entities.len(), 1, "garis lepas tetap garis");
        assert_eq!(app.ink.strokes.iter().filter(|s| s.hidden).count(), 1);
    }

    #[test]
    fn empty_ink_reports_notice_and_stays_in_ink_mode() {
        let mut app = DuCADApp::new_for_test();
        app.set_app_mode(AppMode::Ink);
        let r = app.convert_to_closed_objects();
        assert_eq!(r, ConvertReport::default());
        assert_eq!(app.app_mode, AppMode::Ink);
        assert!(app.closed_objects.notice.is_some());
    }
}
