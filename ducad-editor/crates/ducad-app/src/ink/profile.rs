//! Mode Tinta: opsi kuas "Bentuk Pintar", dan HUD mode sketsa.
//!
//! **Bentuk Pintar**: tiap coretan langsung dikenali lewat pipeline
//! Freehand dan masuk sketch sebagai entitas ber-constraint. Tidak ada
//! penutupan otomatis — itu tugas tombol HUD "Objek Tertutup"
//! (`crate::closed_objects`) setelah pengguna selesai menggambar.

use ducad_ui::{CanvasHud, InkHudState, InkHudTool, SketchHud, SketchHudEvent};
use eframe::egui;
use glam::DVec2;

use crate::app::DuCADApp;
use crate::ink::tools::InkTool;
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

    /// HUD pensil tampil hanya saat pensil dipakai: alat Freehand, atau
    /// Mode Tinta.
    pub fn pencil_hud_visible(&self) -> bool {
        let pencil = self.app_mode.is_ink() || self.tool == ToolKind::Freehand;
        pencil && self.is_sketching && self.app_mode.is_2d() && !self.drawing_sheet_state.is_open
    }

    /// HUD mengambang pensil, tepat di bawah header: tombol "Objek
    /// Tertutup" (alat Freehand atau Mode Tinta) + alat tinta (Mode Tinta).
    pub fn show_sketch_hud(&mut self, ctx: &egui::Context, screen_rect: egui::Rect) {
        if !self.pencil_hud_visible() {
            return;
        }
        let ink = self.app_mode.is_ink().then_some(InkHudState {
            active: match self.ink_state.active_tool {
                InkTool::Brush => Some(InkHudTool::Brush),
                InkTool::Eraser => Some(InkHudTool::Eraser),
                InkTool::Lasso => Some(InkHudTool::Lasso),
                _ => None,
            },
            smart_shape: self.ink_state.smart_shape,
        });
        // Tepat di bawah pita header (menghormati safe area iPad dan top bar
        // yang disembunyikan), bukan y tetap yang menabrak top bar di tablet.
        let top = CanvasHud::header_band(ctx)
            .map(|b| b.max.y + 8.0)
            .unwrap_or(screen_rect.min.y + 64.0);
        let event = egui::Area::new(egui::Id::new("ducad-sketch-hud"))
            .fixed_pos(egui::pos2(screen_rect.center().x, top))
            .pivot(egui::Align2::CENTER_TOP)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| SketchHud::show(ui, ink))
            .inner;
        if let Some(ev) = event {
            self.apply_sketch_hud_event(ev);
        }
    }

    pub fn apply_sketch_hud_event(&mut self, ev: SketchHudEvent) {
        match ev {
            SketchHudEvent::SetInkTool(t) => {
                self.ink_state.active_tool = match t {
                    InkHudTool::Brush => InkTool::Brush,
                    InkHudTool::Eraser => InkTool::Eraser,
                    InkHudTool::Lasso => InkTool::Lasso,
                };
            }
            SketchHudEvent::ToggleSmartShape => {
                self.ink_state.smart_shape = !self.ink_state.smart_shape;
            }
            SketchHudEvent::ConvertToClosedObjects => {
                self.convert_to_closed_objects();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mode::AppMode;
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
        assert!(!app.sketch().entities.is_empty());
        // Tidak dirapatkan/dipilih otomatis; tombol HUD yang
        // menjadikannya objek tertutup.
        assert!(app.selected.is_empty());
        let r = app.convert_to_closed_objects();
        assert!(
            r.objects + r.kept_clean >= 1,
            "persegi Pencil menjadi objek tertutup: {r:?}"
        );
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
    fn hud_only_visible_while_pencil_is_used() {
        let mut app = DuCADApp::new_for_test();
        app.is_sketching = true;
        app.set_tool(ToolKind::Line);
        assert!(!app.pencil_hud_visible(), "alat CAD lain: tidak tampil");
        app.set_tool(ToolKind::Select);
        assert!(!app.pencil_hud_visible());
        app.set_tool(ToolKind::Freehand);
        assert!(app.pencil_hud_visible(), "alat pensil: tampil");
        app.set_app_mode(AppMode::Ink);
        assert!(app.pencil_hud_visible(), "Mode Tinta: tampil");
    }

    #[test]
    fn hud_events_switch_tool_toggle_smart_shape_and_convert() {
        let mut app = DuCADApp::new_for_test();
        app.set_app_mode(AppMode::Ink);
        app.apply_sketch_hud_event(SketchHudEvent::SetInkTool(InkHudTool::Lasso));
        assert_eq!(app.ink_state.active_tool, InkTool::Lasso);
        app.apply_sketch_hud_event(SketchHudEvent::ToggleSmartShape);
        assert!(app.ink_state.smart_shape);
        app.apply_sketch_hud_event(SketchHudEvent::ConvertToClosedObjects);
        assert!(
            app.closed_objects.notice.is_some(),
            "tombol HUD menjalankan konversi"
        );
    }
}
