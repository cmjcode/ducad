//! Mode Tinta → profil sketsa CAD yang bisa di-extrude.
//!
//! Dua jalur:
//! - **Bentuk Pintar** (opsi kuas): tiap coretan langsung dikenali lewat
//!   pipeline Freehand dan masuk sketch sebagai entitas ber-constraint,
//!   celahnya ditutup otomatis.
//! - **Jadikan Profil**: coretan terpilih (atau semua) dikonversi sekaligus
//!   (`ducad_ink::vectorize`), coretan kasar disembunyikan, lalu aplikasi
//!   pindah ke Mode Sketsa CAD dengan region hasil terpilih — tombol
//!   Ekstrusi langsung muncul di bar konteks.

use std::collections::HashSet;

use ducad_ink::{vectorize_strokes, SetStrokesHidden, Stroke, VectorizeOptions};
use ducad_sketch::constraint::commands::AddConstraint;
use ducad_sketch::{EntityId, UpdateEntity};
use ducad_ui::{InkHud, InkHudEvent, InkHudTool};
use eframe::egui;
use glam::DVec2;

use crate::app::DuCADApp;
use crate::freehand::{retarget, ProfileFeedback};
use crate::ink::tools::InkTool;
use crate::mode::AppMode;
use crate::types::ToolKind;

impl DuCADApp {
    /// "Bentuk Pintar": kenali coretan (posisi pena mentah, bidang sketch)
    /// dan masukkan ke sketch. `true` bila coretan dipakai — pemanggil
    /// TIDAK menambahkannya sebagai tinta.
    pub fn ink_smart_shape_commit(&mut self, raw: &[glam::Vec2]) -> bool {
        let pts: Vec<DVec2> = raw
            .iter()
            .map(|p| DVec2::new(p.x as f64, p.y as f64))
            .collect();
        // Usulan freehand lama (bila ada) diterima dulu agar tidak tertimpa.
        self.freehand_accept();
        if !self.freehand_recognize(pts, Vec::new()) {
            return false;
        }
        self.freehand_accept();
        true
    }

    /// Id coretan yang akan dikonversi: seleksi lasso, atau semua coretan
    /// yang terlihat bila tidak ada seleksi.
    fn ink_profile_targets(&self) -> Vec<u64> {
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

    /// "Jadikan Profil". Mengembalikan jumlah coretan yang dikonversi.
    ///
    /// Undo: dua langkah — pertama menghapus entitas sketch, kedua
    /// menampilkan lagi coretan tinta (tumpukan undo tinta dan sketch
    /// terpisah; urutannya dijaga undo global).
    pub fn ink_to_profile(&mut self) -> usize {
        let targets = self.ink_profile_targets();
        let opt = VectorizeOptions {
            snap_dist: self.freehand_snap_dist(),
            close_gap: self.auto_close_gap_mm(),
        };
        let v = {
            let strokes: Vec<&Stroke> = self
                .ink
                .strokes
                .iter()
                .filter(|s| targets.contains(&s.id))
                .collect();
            vectorize_strokes(self.sketch(), &strokes, &opt)
        };
        if v.is_empty() {
            self.freehand.feedback = Some(ProfileFeedback::Notice(ducad_i18n::t!(
                "ink-to-profile-empty"
            )));
            return 0;
        }

        self.execute_ink_command(
            Box::new(SetStrokesHidden::new(v.converted.clone(), true)),
            "Ink to Profile",
        );
        self.sketch_set.active_mut().undo.begin("Ink to Profile");
        for (id, e) in &v.updated_existing {
            self.execute_sketch_command(Box::new(UpdateEntity::new(
                "Ink to Profile",
                *id,
                e.clone(),
            )));
        }
        let ids = self.insert_entities_tracked("Ink to Profile", v.entities.clone());
        for c in retarget(&v.constraints, &v.trial_ids, &ids) {
            self.execute_sketch_command(Box::new(AddConstraint::new(c)));
        }
        self.sketch_set.active_mut().undo.commit();

        self.ink_state.selected_stroke_ids.clear();
        self.set_app_mode(AppMode::Sketch);
        self.set_tool(ToolKind::Select);
        let scope: HashSet<EntityId> = ids.into_iter().collect();
        self.refresh_profile_feedback(&scope);
        if !matches!(self.freehand.feedback, Some(ProfileFeedback::Closed { .. })) {
            self.freehand.feedback = Some(ProfileFeedback::Notice(format!(
                "{} · {}",
                ducad_i18n::t!("ink-to-profile-done", count = v.converted.len()),
                self.profile_feedback_text().unwrap_or_default()
            )));
        }
        self.record_activity(
            ducad_ui::ActivityKindUi::Sketch2D,
            &ducad_i18n::t!("ink-to-profile"),
            &ducad_i18n::t!("ink-to-profile-done", count = v.converted.len()),
        );
        v.converted.len()
    }

    /// HUD mengambang Mode Tinta (atas tengah kanvas).
    pub fn show_ink_hud(&mut self, ctx: &egui::Context, screen_rect: egui::Rect) {
        if self.app_mode != AppMode::Ink {
            return;
        }
        let active = match self.ink_state.active_tool {
            InkTool::Brush => Some(InkHudTool::Brush),
            InkTool::Eraser => Some(InkHudTool::Eraser),
            InkTool::Lasso => Some(InkHudTool::Lasso),
            _ => None,
        };
        let smart = self.ink_state.smart_shape;
        let selected = self.ink_state.selected_stroke_ids.len();
        let event = egui::Area::new(egui::Id::new("ducad-ink-hud"))
            .fixed_pos(egui::pos2(screen_rect.center().x, screen_rect.min.y + 64.0))
            .pivot(egui::Align2::CENTER_TOP)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| InkHud::show(ui, active, smart, selected))
            .inner;
        if let Some(ev) = event {
            self.apply_ink_hud_event(ev);
        }
    }

    pub fn apply_ink_hud_event(&mut self, ev: InkHudEvent) {
        match ev {
            InkHudEvent::SetTool(t) => {
                self.ink_state.active_tool = match t {
                    InkHudTool::Brush => InkTool::Brush,
                    InkHudTool::Eraser => InkTool::Eraser,
                    InkHudTool::Lasso => InkTool::Lasso,
                };
            }
            InkHudEvent::ToggleSmartShape => {
                self.ink_state.smart_shape = !self.ink_state.smart_shape;
            }
            InkHudEvent::MakeProfile => {
                self.ink_to_profile();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_ink::stroke::InkPoint;
    use eframe::egui::{TouchId, TouchPhase};
    use glam::Vec2;

    fn segment(a: Vec2, b: Vec2) -> Vec<Vec2> {
        (0..=30).map(|i| a + (b - a) * (i as f32 / 30.0)).collect()
    }

    /// Persegi ±40×25 mm digambar dalam SATU coretan Pencil.
    fn square_path() -> Vec<Vec2> {
        let c = [
            Vec2::new(0.0, 0.0),
            Vec2::new(40.0, 0.3),
            Vec2::new(40.2, 25.0),
            Vec2::new(0.1, 25.2),
        ];
        let mut pts = Vec::new();
        for i in 0..4 {
            pts.extend(segment(c[i], c[(i + 1) % 4]));
        }
        pts
    }

    fn touch_stroke(app: &mut DuCADApp, pts: &[Vec2], id: u64) {
        let tid = TouchId(id);
        app.handle_ink_touch(TouchPhase::Start, pts[0], Some(0.6), tid);
        for p in &pts[1..] {
            app.handle_ink_touch(TouchPhase::Move, *p, Some(0.6), tid);
        }
        app.handle_ink_touch(TouchPhase::End, pts[pts.len() - 1], Some(0.6), tid);
    }

    fn add_ink(app: &mut DuCADApp, pts: &[Vec2]) {
        let points = pts
            .iter()
            .map(|p| InkPoint::new(p.x, p.y, 0.5, 0.0, 0))
            .collect();
        app.commit_ink_stroke(points);
    }

    #[test]
    fn smart_shape_turns_pencil_square_into_closed_profile() {
        let mut app = DuCADApp::new_for_test();
        app.set_app_mode(AppMode::Ink);
        app.ink_state.smart_shape = true;
        touch_stroke(&mut app, &square_path(), 1);

        assert!(
            app.ink.strokes.is_empty(),
            "coretan tidak disimpan sebagai tinta"
        );
        // Bentuk persisnya (persegi atau spline tertutup) bergantung pada
        // filter 1€ yang memakai stempel waktu; di tes semua sentuhan
        // serentak. Yang dijamin: hasilnya profil tertutup.
        assert!(!app.sketch().entities.is_empty());
        assert_eq!(ducad_sketch::find_closed_regions(app.sketch()).len(), 1);
        assert!(!app.selected.is_empty(), "region terpilih");

        app.extrude_distance_input = "6".to_string();
        app.extrude_selected();
        assert_eq!(app.model.doc.bodies.len(), 1, "{:?}", app.model_status);
    }

    #[test]
    fn smart_shape_off_keeps_plain_ink() {
        let mut app = DuCADApp::new_for_test();
        app.set_app_mode(AppMode::Ink);
        touch_stroke(&mut app, &square_path(), 2);
        assert_eq!(app.ink.strokes.len(), 1);
        assert!(app.sketch().entities.is_empty());
    }

    #[test]
    fn make_profile_converts_sloppy_ink_and_extrudes() {
        let mut app = DuCADApp::new_for_test();
        app.set_app_mode(AppMode::Ink);
        // Empat coretan terpisah, ujungnya meleset ±1 mm.
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
        assert!(ducad_sketch::find_closed_regions(app.sketch()).is_empty());

        assert_eq!(app.ink_to_profile(), 4);
        assert_eq!(app.app_mode, AppMode::Sketch, "pindah ke Mode Sketsa CAD");
        assert_eq!(app.tool, ToolKind::Select);
        assert!(
            app.ink.strokes.iter().all(|s| s.hidden),
            "tinta kasar disembunyikan"
        );
        assert!(
            matches!(app.freehand.feedback, Some(ProfileFeedback::Closed { .. })),
            "{:?}",
            app.freehand.feedback
        );

        app.extrude_distance_input = "5".to_string();
        app.extrude_selected();
        assert_eq!(app.model.doc.bodies.len(), 1, "{:?}", app.model_status);

        app.undo(); // solid
        app.undo(); // entitas sketch
        assert!(app.sketch().entities.is_empty());
        app.undo(); // tinta tampil lagi
        assert!(app.ink.strokes.iter().all(|s| !s.hidden));
        assert_eq!(app.ink.strokes.len(), 4);
    }

    #[test]
    fn make_profile_uses_lasso_selection_only() {
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
        assert_eq!(app.ink_to_profile(), 1);
        assert_eq!(app.sketch().entities.len(), 1);
        assert_eq!(app.ink.strokes.iter().filter(|s| s.hidden).count(), 1);
    }

    #[test]
    fn make_profile_with_no_ink_reports_notice() {
        let mut app = DuCADApp::new_for_test();
        app.set_app_mode(AppMode::Ink);
        assert_eq!(app.ink_to_profile(), 0);
        assert_eq!(app.app_mode, AppMode::Ink, "tetap di mode tinta");
        assert!(matches!(
            app.freehand.feedback,
            Some(ProfileFeedback::Notice(_))
        ));
    }

    #[test]
    fn hud_events_switch_tool_and_toggle_smart_shape() {
        let mut app = DuCADApp::new_for_test();
        app.set_app_mode(AppMode::Ink);
        app.apply_ink_hud_event(InkHudEvent::SetTool(InkHudTool::Lasso));
        assert_eq!(app.ink_state.active_tool, InkTool::Lasso);
        app.apply_ink_hud_event(InkHudEvent::ToggleSmartShape);
        assert!(app.ink_state.smart_shape);
    }
}
