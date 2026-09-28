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
use ducad_sketch::gaps::{open_endpoints, plan_gap_closure, GapOptions, JOIN_EPS};
use ducad_sketch::infer::{infer_constraints, InferOptions};
use ducad_sketch::recognize::{recognize, to_entities, Recognized, Stroke};
use ducad_sketch::{
    find_region_containing_entity, Entity, EntityId, InsertEntities, UpdateEntity,
};
use glam::DVec2;

use crate::app::DuCADApp;

/// Jeda sebelum usulan diterima otomatis.
pub const AUTO_ACCEPT: Duration = Duration::from_millis(600);

/// Celah (piksel layar) yang ditutup otomatis sesudah coretan diterima.
pub const AUTO_CLOSE_PX: f64 = 14.0;
/// Celah (piksel layar) maksimum untuk aksi manual "Tutup Profil".
pub const MANUAL_CLOSE_PX: f64 = 60.0;

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

/// Status profil sesudah coretan terakhir diterima.
#[derive(Debug, Clone, PartialEq)]
pub enum ProfileFeedback {
    /// Coretan menutup region; `area` dalam mm².
    Closed { area: f64 },
    /// Masih terbuka; `gap` = celah terkecil yang tersisa (mm), bila ada.
    Open { gap: Option<f64> },
    /// Pesan satu kali (hasil aksi Tutup Profil / Jadikan Profil).
    Notice(String),
}

#[derive(Default)]
pub struct FreehandState {
    /// Coretan yang sedang digambar.
    pub stroke: Vec<DVec2>,
    pub pressure: Vec<f32>,
    pub preview: Option<FreehandPreview>,
    /// Terima otomatis setelah [`AUTO_ACCEPT`].
    pub auto_accept: bool,
    /// Tutup celah kecil otomatis sesudah coretan diterima.
    pub auto_close: bool,
    /// Ujung menggantung dari coretan terakhir — digambar sebagai penanda
    /// merah agar pengguna tahu di mana profil masih bocor.
    pub open_ends: Vec<DVec2>,
    pub feedback: Option<ProfileFeedback>,
}

impl FreehandState {
    pub fn new() -> Self {
        Self {
            auto_accept: true,
            auto_close: true,
            ..Default::default()
        }
    }
}

/// Hasil eksekusi rencana penutupan celah.
#[derive(Debug, Default)]
pub struct CloseOutcome {
    pub closed: usize,
    pub bridge_ids: Vec<EntityId>,
    pub smallest_remaining_gap: Option<f64>,
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
        let Some(shape) = recognize(&stroke) else {
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

    /// Celah otomatis dalam mm (lihat [`AUTO_CLOSE_PX`]).
    pub fn auto_close_gap_mm(&self) -> f64 {
        (self.mm_per_px() * AUTO_CLOSE_PX).clamp(0.5, 25.0)
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
        if entities.is_empty() {
            return Vec::new();
        }
        let before: HashSet<EntityId> = self.sketch().entities.keys().collect();
        self.execute_sketch_command(Box::new(InsertEntities::new(label, entities.clone())));
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

    /// Terima usulan: SATU langkah undo berisi entitas, constraint, dan
    /// penutupan celah otomatis.
    pub fn freehand_accept(&mut self) {
        let Some(preview) = self.freehand.preview.take() else {
            return;
        };
        self.sketch_set.active_mut().undo.begin("Freehand");
        let new_ids = self.insert_entities_tracked("Freehand", preview.entities.clone());
        for c in retarget(&preview.constraints, &preview.trial_ids, &new_ids) {
            self.execute_sketch_command(Box::new(AddConstraint::new(c)));
        }
        let mut scope: HashSet<EntityId> = new_ids.iter().copied().collect();
        if self.freehand.auto_close {
            let gap = self.auto_close_gap_mm();
            let outcome = self.execute_gap_closure(
                &scope,
                &GapOptions {
                    max_gap: gap,
                    move_max: gap,
                },
            );
            scope.extend(outcome.bridge_ids);
        }
        self.sketch_set.active_mut().undo.commit();
        self.refresh_profile_feedback(&scope);
        self.record_activity(
            ducad_ui::ActivityKindUi::Sketch2D,
            &ducad_i18n::t!("tool-freehand"),
            &ducad_i18n::t!("freehand-committed"),
        );
    }

    /// Jalankan rencana penutupan celah untuk `scope` lewat command (tanpa
    /// membuka transaksi undo — pemanggil yang membungkus).
    pub fn execute_gap_closure(
        &mut self,
        scope: &HashSet<EntityId>,
        opt: &GapOptions,
    ) -> CloseOutcome {
        let fix = plan_gap_closure(self.sketch(), Some(scope), opt);
        let mut outcome = CloseOutcome {
            closed: fix.closed,
            bridge_ids: Vec::new(),
            smallest_remaining_gap: fix.smallest_remaining_gap,
        };
        if fix.is_empty() {
            return outcome;
        }
        for (id, e) in fix.updates {
            self.execute_sketch_command(Box::new(UpdateEntity::new("Close Profile", id, e)));
        }
        outcome.bridge_ids = self.insert_entities_tracked("Close Profile", fix.bridges);
        for c in fix.constraints {
            self.execute_sketch_command(Box::new(AddConstraint::new(c)));
        }
        for link in fix.bridge_links {
            if let Some(id) = outcome.bridge_ids.get(link.bridge) {
                self.execute_sketch_command(Box::new(AddConstraint::new(link.constraint(*id))));
            }
        }
        outcome
    }

    /// Aksi "Tutup Profil": tutup celah pada seleksi (atau seluruh sketch
    /// bila seleksi kosong) dengan batas lebih longgar; celah besar
    /// dijembatani garis agar coretan asli tidak terdistorsi.
    pub fn close_selected_profile(&mut self) -> CloseOutcome {
        let scope: HashSet<EntityId> = if self.selected.is_empty() {
            self.sketch().entities.keys().collect()
        } else {
            self.selected.clone()
        };
        let opt = GapOptions {
            max_gap: (self.mm_per_px() * MANUAL_CLOSE_PX).clamp(1.0, 100.0),
            move_max: self.auto_close_gap_mm(),
        };
        // Tanpa celah yang bisa ditutup: jangan buat langkah undo kosong
        // dan jangan ubah seleksi.
        let preview = plan_gap_closure(self.sketch(), Some(&scope), &opt);
        if preview.is_empty() {
            return CloseOutcome {
                closed: 0,
                bridge_ids: Vec::new(),
                smallest_remaining_gap: preview.smallest_remaining_gap,
            };
        }
        self.sketch_set.active_mut().undo.begin("Close Profile");
        let outcome = self.execute_gap_closure(&scope, &opt);
        self.sketch_set.active_mut().undo.commit();
        let mut touched = scope;
        touched.extend(outcome.bridge_ids.iter().copied());
        self.refresh_profile_feedback(&touched);
        outcome
    }

    /// Hitung ulang umpan balik profil untuk entitas `ids`: bila membentuk
    /// region, region itu DIPILIH (bar konteks langsung menawarkan
    /// Ekstrusi); bila tidak, entitasnya dipilih dan ujung menggantung
    /// ditandai.
    pub fn refresh_profile_feedback(&mut self, ids: &HashSet<EntityId>) {
        let mut sorted: Vec<EntityId> = ids.iter().copied().collect();
        sorted.sort();
        let mut region_ids: HashSet<EntityId> = HashSet::new();
        let mut area = 0.0;
        for id in &sorted {
            if region_ids.contains(id) {
                continue;
            }
            if let Some(r) = find_region_containing_entity(self.sketch(), *id) {
                area += r.area.abs();
                region_ids.extend(r.entity_ids);
            }
        }
        if !region_ids.is_empty() {
            self.selected = region_ids;
            self.freehand.open_ends.clear();
            self.freehand.feedback = Some(ProfileFeedback::Closed { area });
            return;
        }
        let live: HashSet<EntityId> = sorted
            .iter()
            .copied()
            .filter(|id| self.sketch().entities.contains_key(*id))
            .collect();
        let all_ends = open_endpoints(self.sketch(), JOIN_EPS);
        let joined = |i: usize| {
            all_ends.iter().enumerate().any(|(j, q)| {
                j != i && (q.pos - all_ends[i].pos).length() <= JOIN_EPS
            })
        };
        let dangling: Vec<usize> = (0..all_ends.len()).filter(|i| !joined(*i)).collect();
        let open: Vec<DVec2> = dangling
            .iter()
            .filter(|i| live.contains(&all_ends[**i].entity))
            .map(|i| all_ends[*i].pos)
            .collect();
        // Celah = jarak antar ujung menggantung yang bisa disambung: beda
        // entitas, atau ujung-ujung spline/path yang sama (menutup diri).
        let sketch = self.sketch();
        let can_self_close = |id: EntityId| {
            matches!(
                sketch.entities.get(id),
                Some(Entity::Spline { .. } | Entity::Path { .. })
            )
        };
        let mut gap: Option<f64> = None;
        for (k, &i) in dangling.iter().enumerate() {
            for &j in &dangling[k + 1..] {
                let (a, b) = (&all_ends[i], &all_ends[j]);
                if !(live.contains(&a.entity) || live.contains(&b.entity)) {
                    continue;
                }
                if a.entity == b.entity && !can_self_close(a.entity) {
                    continue;
                }
                let d = (a.pos - b.pos).length();
                gap = Some(gap.map_or(d, |g: f64| g.min(d)));
            }
        }
        self.freehand.feedback = if open.is_empty() {
            None
        } else {
            Some(ProfileFeedback::Open { gap })
        };
        self.freehand.open_ends = open;
        self.selected = live;
    }

    /// Teks umpan balik untuk status bar, bila ada.
    pub fn profile_feedback_text(&self) -> Option<String> {
        match self.freehand.feedback.as_ref()? {
            ProfileFeedback::Closed { area } => Some(ducad_i18n::t!(
                "freehand-profile-closed",
                area = format!("{area:.0}")
            )),
            ProfileFeedback::Open { gap: Some(g) } => Some(ducad_i18n::t!(
                "freehand-profile-open-gap",
                gap = format!("{g:.1}")
            )),
            ProfileFeedback::Open { gap: None } => {
                Some(ducad_i18n::t!("freehand-profile-open"))
            }
            ProfileFeedback::Notice(msg) => Some(msg.clone()),
        }
    }

    /// Aksi "Tutup Profil" dari bar konteks/palette, dengan pesan hasil.
    pub fn run_close_profile(&mut self) {
        let outcome = self.close_selected_profile();
        if outcome.closed > 0 {
            // `refresh_profile_feedback` sudah mengisi Closed/Open; bila
            // masih terbuka, jumlah celah yang ditutup tetap diberitahukan.
            if !matches!(self.freehand.feedback, Some(ProfileFeedback::Closed { .. })) {
                self.freehand.feedback = Some(ProfileFeedback::Notice(ducad_i18n::t!(
                    "close-profile-done",
                    count = outcome.closed
                )));
            }
            return;
        }
        let msg = match outcome.smallest_remaining_gap {
            Some(g) => ducad_i18n::t!("close-profile-too-far", gap = format!("{g:.1}")),
            None => ducad_i18n::t!("close-profile-none"),
        };
        self.freehand.feedback = Some(ProfileFeedback::Notice(msg));
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

    /// Coretan bunga tertutup yang melewati titik awal sedikit.
    fn flower_stroke(center: DVec2) -> Vec<DVec2> {
        let n = 200;
        (0..n)
            .map(|i| {
                let a = std::f64::consts::TAU * 1.04 * i as f64 / (n - 1) as f64;
                let r = 16.0 + 4.0 * (5.0 * a).sin();
                center + DVec2::new(a.cos(), a.sin()) * r
            })
            .collect()
    }

    fn arc_stroke(from: f64, to: f64) -> Vec<DVec2> {
        (0..=60)
            .map(|i| DVec2::from_angle(from + (to - from) * i as f64 / 60.0) * 15.0)
            .collect()
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
    fn closed_freeform_stroke_extrudes_without_manual_selection() {
        let mut app = DuCADApp::new_for_test();
        app.set_tool(ToolKind::Freehand);
        draw(&mut app, &flower_stroke(DVec2::ZERO));
        assert!(
            matches!(app.freehand.feedback, Some(ProfileFeedback::Closed { .. })),
            "{:?}",
            app.freehand.feedback
        );
        assert!(!app.selected.is_empty(), "region hasil coretan terpilih");
        assert!(app.freehand.open_ends.is_empty());

        app.extrude_distance_input = "5".to_string();
        app.extrude_selected();
        assert_eq!(app.model.doc.bodies.len(), 1, "{:?}", app.model_status);
    }

    #[test]
    fn two_strokes_with_gap_close_in_one_undo() {
        use std::f64::consts::PI;
        let mut app = DuCADApp::new_for_test();
        app.set_tool(ToolKind::Freehand);
        draw(&mut app, &arc_stroke(0.07, PI - 0.07));
        assert!(
            matches!(app.freehand.feedback, Some(ProfileFeedback::Open { .. })),
            "satu busur masih terbuka: {:?}",
            app.freehand.feedback
        );
        assert_eq!(app.freehand.open_ends.len(), 2, "dua ujung ditandai");
        let after_first = app.sketch().entities.len();

        draw(&mut app, &arc_stroke(PI + 0.07, 2.0 * PI - 0.07));
        assert!(
            matches!(app.freehand.feedback, Some(ProfileFeedback::Closed { .. })),
            "{:?}",
            app.freehand.feedback
        );
        assert_eq!(ducad_sketch::find_closed_regions(app.sketch()).len(), 1);

        app.extrude_distance_input = "4".to_string();
        app.extrude_selected();
        assert_eq!(app.model.doc.bodies.len(), 1, "{:?}", app.model_status);
        app.undo();
        assert_eq!(app.model.doc.bodies.len(), 0);

        app.undo();
        assert_eq!(
            app.sketch().entities.len(),
            after_first,
            "coretan kedua + jembatan celah hilang dalam satu undo"
        );
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
    fn close_profile_action_bridges_gap_beyond_auto_limit() {
        let mut app = DuCADApp::new_for_test();
        let g = app.auto_close_gap_mm() * 1.5;
        let lines = vec![
            Entity::line(DVec2::new(0.0, 0.0), DVec2::new(40.0, 0.0)),
            Entity::line(DVec2::new(40.0, g), DVec2::new(40.0, 30.0)),
            Entity::line(DVec2::new(40.0, 30.0), DVec2::new(0.0, 30.0)),
            Entity::line(DVec2::new(0.0, 30.0), DVec2::new(0.0, 0.0)),
        ];
        let ids = app.insert_entities_tracked("Line", lines);
        app.selected = ids.into_iter().collect();
        assert!(ducad_sketch::find_closed_regions(app.sketch()).is_empty());

        app.run_close_profile();
        assert!(
            matches!(app.freehand.feedback, Some(ProfileFeedback::Closed { .. })),
            "{:?}",
            app.freehand.feedback
        );
        assert_eq!(app.sketch().entities.len(), 5, "celah besar dijembatani garis");
        app.extrude_distance_input = "3".to_string();
        app.extrude_selected();
        assert_eq!(app.model.doc.bodies.len(), 1, "{:?}", app.model_status);
    }

    #[test]
    fn close_profile_without_gap_changes_nothing() {
        let mut app = DuCADApp::new_for_test();
        let ids = app.insert_entities_tracked(
            "Line",
            vec![Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0))],
        );
        app.selected = ids.iter().copied().collect();
        let undo_before = app.sketch_set.active().undo.top_undo_stamp();
        app.run_close_profile();
        assert_eq!(app.sketch_set.active().undo.top_undo_stamp(), undo_before);
        assert_eq!(app.selected.len(), 1);
        assert!(matches!(
            app.freehand.feedback,
            Some(ProfileFeedback::Notice(_))
        ));
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
    fn zz_debug_self_intersecting_extrude() {
        // Bentuk seperti gambar: lengkung besar lalu loop kecil yang memotong
        // lintasannya sendiri sebelum kembali ke titik awal.
        let mut pts = Vec::new();
        for i in 0..=80 { let a = std::f64::consts::PI * (1.0 - i as f64 / 80.0); pts.push(DVec2::new(40.0 * a.cos(), 20.0 * a.sin())); }
        for i in 0..=80 { let a = std::f64::consts::TAU * i as f64 / 80.0; pts.push(DVec2::new(40.0 - 25.0 + 25.0 * a.cos(), -12.0 * a.sin())); }
        for i in 0..=40 { let t = i as f64 / 40.0; pts.push(DVec2::new(40.0 + (-40.0 - 40.0) * t, 0.0 - 15.0 * (std::f64::consts::PI * t).sin())); }
        let mut app = DuCADApp::new_for_test();
        app.set_tool(ToolKind::Freehand);
        for p in &pts { app.freehand_push(*p, None); }
        app.freehand_finish();
        eprintln!("SHAPE {:?}", app.freehand.preview.as_ref().map(|p| std::mem::discriminant(&p.shape)));
        app.freehand_accept();
        eprintln!("FEEDBACK {:?} sel={} ents={}", app.freehand.feedback, app.selected.len(), app.sketch().entities.len());
        app.extrude_distance_input = "10".to_string();
        app.extrude_selected();
        eprintln!("STATUS {:?} bodies={}", app.model_status, app.model.doc.bodies.len());
        for (_, g) in app.model.geometry.iter() {
            eprintln!("VALID {} VOL {} TRIS {} EDGES {}", g.shape.is_valid(), g.shape.volume(), g.mesh.triangle_count(), g.edge_lines.len());
        }
        let mut app2 = DuCADApp::new_for_test();
        app2.set_tool(ToolKind::Freehand);
        draw(&mut app2, &flower_stroke(DVec2::ZERO));
        app2.extrude_distance_input = "10".to_string();
        app2.extrude_selected();
        for (_, g) in app2.model.geometry.iter() {
            eprintln!("FLOWER VALID {} VOL {} TRIS {}", g.shape.is_valid(), g.shape.volume(), g.mesh.triangle_count());
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
