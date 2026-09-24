//! Modul integrasi mode Sketsa Tinta (Concepts-like) di DuCAD.

pub mod input;
pub mod tools;

pub use tools::{InkTool, InkToolState};

#[cfg(test)]
mod tests {
    use crate::app::DuCADApp;
    use crate::mode::AppMode;
    use ducad_ui::TouchDesignMode;
    use eframe::egui;
    use glam::Vec2;

    #[test]
    fn touch_sequence_produces_one_stroke_one_undo() {
        let mut app = DuCADApp::new_for_test();
        app.set_app_mode(AppMode::Ink);

        let touch_id = egui::TouchId(1);
        app.handle_ink_touch(
            egui::TouchPhase::Start,
            Vec2::new(10.0, 10.0),
            Some(0.6),
            touch_id,
        );
        app.handle_ink_touch(
            egui::TouchPhase::Move,
            Vec2::new(20.0, 15.0),
            Some(0.7),
            touch_id,
        );
        app.handle_ink_touch(
            egui::TouchPhase::Move,
            Vec2::new(30.0, 20.0),
            Some(0.8),
            touch_id,
        );
        app.handle_ink_touch(
            egui::TouchPhase::End,
            Vec2::new(30.0, 20.0),
            Some(0.8),
            touch_id,
        );

        assert_eq!(app.ink.strokes.len(), 1);
        assert!(app.can_undo());

        app.undo();
        assert_eq!(app.ink.strokes.len(), 0);

        app.redo();
        assert_eq!(app.ink.strokes.len(), 1);
    }

    #[test]
    fn pencil_only_mode_ignores_finger_draw() {
        let mut app = DuCADApp::new_for_test();
        app.set_app_mode(AppMode::Ink);
        app.touch_config.mode = TouchDesignMode::PencilOnly;

        // Sentuhan jari tanpa force (None) harus diabaikan untuk menggambar
        let finger_id = egui::TouchId(2);
        app.handle_ink_touch(
            egui::TouchPhase::Start,
            Vec2::new(10.0, 10.0),
            None,
            finger_id,
        );
        app.handle_ink_touch(
            egui::TouchPhase::Move,
            Vec2::new(20.0, 20.0),
            None,
            finger_id,
        );
        app.handle_ink_touch(
            egui::TouchPhase::End,
            Vec2::new(20.0, 20.0),
            None,
            finger_id,
        );

        assert_eq!(app.ink.strokes.len(), 0);
        assert!(!app.can_undo());

        // Sentuhan Apple Pencil dengan nilai force harus diterima
        let pencil_id = egui::TouchId(3);
        app.handle_ink_touch(
            egui::TouchPhase::Start,
            Vec2::new(10.0, 10.0),
            Some(0.8),
            pencil_id,
        );
        app.handle_ink_touch(
            egui::TouchPhase::Move,
            Vec2::new(20.0, 20.0),
            Some(0.8),
            pencil_id,
        );
        app.handle_ink_touch(
            egui::TouchPhase::End,
            Vec2::new(20.0, 20.0),
            Some(0.8),
            pencil_id,
        );

        assert_eq!(app.ink.strokes.len(), 1);
        assert!(app.can_undo());
    }

    #[test]
    fn tap_without_move_makes_dot_stroke() {
        let mut app = DuCADApp::new_for_test();
        app.set_app_mode(AppMode::Ink);

        let tap_id = egui::TouchId(4);
        app.handle_ink_touch(
            egui::TouchPhase::Start,
            Vec2::new(50.0, 50.0),
            Some(0.5),
            tap_id,
        );
        app.handle_ink_touch(
            egui::TouchPhase::End,
            Vec2::new(50.0, 50.0),
            Some(0.5),
            tap_id,
        );

        assert_eq!(app.ink.strokes.len(), 1);
        let stroke = &app.ink.strokes[0];
        assert!(!stroke.points.is_empty());
        assert!((stroke.points[0].x - 50.0).abs() < 1e-3);
        assert!((stroke.points[0].y - 50.0).abs() < 1e-3);

        assert!(app.can_undo());
        app.undo();
        assert_eq!(app.ink.strokes.len(), 0);
    }
}
