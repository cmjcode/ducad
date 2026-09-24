//! Modul integrasi mode Sketsa Tinta (Concepts-like) di DuCAD.

pub mod canvas;
pub mod input;
pub mod tools;

pub use canvas::{build_ink_layer_batches, INK_CHUNK_STROKE_SIZE, INK_MAX_STROKE_WARNING};
pub use tools::{InkTool, InkToolState};

#[cfg(test)]
mod tests {
    use super::build_ink_layer_batches;
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

    #[test]
    fn test_ink_chunking_partitions_large_strokes() {
        let mut app = DuCADApp::new_for_test();
        let brush_id = app.get_or_init_active_brush();
        let layer_id = app.get_or_create_ink_layer();

        // Buat 2.500 coretan
        for i in 0..2500 {
            let p0 = ducad_ink::stroke::InkPoint::new(i as f32, 0.0, 0.5, 0.0, 0);
            let p1 = ducad_ink::stroke::InkPoint::new(i as f32 + 0.5, 0.5, 0.5, 0.0, 10);
            let s = ducad_ink::stroke::Stroke::new(
                i as u64,
                vec![p0, p1],
                brush_id,
                ducad_sketch::style::Rgba::BLACK,
                layer_id,
            );
            app.ink.strokes.push(s);
        }

        let batches = build_ink_layer_batches(&app.ink, None, &app.active_plane, &|_| true);
        assert_eq!(batches.len(), 1);
        assert!(!batches[0].vertices.is_empty());
    }

    #[test]
    fn test_ink_viewport_culling_skips_outside_chunks() {
        let mut app = DuCADApp::new_for_test();
        let brush_id = app.get_or_init_active_brush();
        let layer_id = app.get_or_create_ink_layer();

        // Stroke 1: di dalam viewport (0..10)
        let p0 = ducad_ink::stroke::InkPoint::new(5.0, 5.0, 0.5, 0.0, 0);
        let p1 = ducad_ink::stroke::InkPoint::new(6.0, 6.0, 0.5, 0.0, 10);
        let mut s1 = ducad_ink::stroke::Stroke::new(
            1,
            vec![p0, p1],
            brush_id,
            ducad_sketch::style::Rgba::BLACK,
            layer_id,
        );
        s1.recompute_bbox();
        app.ink.strokes.push(s1);

        // Stroke 2: jauh di luar viewport (10000..10010)
        let p2 = ducad_ink::stroke::InkPoint::new(10005.0, 10005.0, 0.5, 0.0, 0);
        let p3 = ducad_ink::stroke::InkPoint::new(10006.0, 10006.0, 0.5, 0.0, 10);
        let mut s2 = ducad_ink::stroke::Stroke::new(
            2,
            vec![p2, p3],
            brush_id,
            ducad_sketch::style::Rgba::BLACK,
            layer_id,
        );
        s2.recompute_bbox();
        app.ink.strokes.push(s2);

        let v_min = Vec2::new(0.0, 0.0);
        let v_max = Vec2::new(50.0, 50.0);

        let batches =
            build_ink_layer_batches(&app.ink, Some((v_min, v_max)), &app.active_plane, &|_| true);
        assert_eq!(batches.len(), 1);
        // Pastikan hanya stroke 1 yang diproses, vertex count proporsional ke 1 stroke
        assert!(batches[0].vertices.len() <= 20);
    }

    #[test]
    fn test_ink_100k_stroke_warning() {
        let mut app = DuCADApp::new_for_test();
        assert_eq!(app.ink_stroke_count_warning(), None);

        // Simulasikan 100k stroke
        let brush_id = app.get_or_init_active_brush();
        let layer_id = app.get_or_create_ink_layer();
        let dummy = ducad_ink::stroke::Stroke::new(
            0,
            vec![ducad_ink::stroke::InkPoint::new(0.0, 0.0, 0.5, 0.0, 0)],
            brush_id,
            ducad_sketch::style::Rgba::BLACK,
            layer_id,
        );
        app.ink.strokes.resize(100_000, dummy);
        assert!(app.ink_stroke_count_warning().is_some());
    }

    #[test]
    fn test_ink_memory_bytes_and_formatted() {
        let app = DuCADApp::new_for_test();
        assert!(app.ink_memory_bytes() > 0);
        let formatted = app.ink_memory_formatted();
        assert!(formatted.ends_with(" B") || formatted.ends_with(" KB"));
    }

    #[test]
    fn test_fit_to_ink_content() {
        let mut app = DuCADApp::new_for_test();
        app.set_app_mode(AppMode::Ink);

        let brush_id = app.get_or_init_active_brush();
        let layer_id = app.get_or_create_ink_layer();

        let p0 = ducad_ink::stroke::InkPoint::new(10.0, 20.0, 0.5, 0.0, 0);
        let p1 = ducad_ink::stroke::InkPoint::new(110.0, 120.0, 0.5, 0.0, 10);
        let mut s = ducad_ink::stroke::Stroke::new(
            1,
            vec![p0, p1],
            brush_id,
            ducad_sketch::style::Rgba::BLACK,
            layer_id,
        );
        s.recompute_bbox();
        app.ink.strokes.push(s);

        app.fit_to_ink_content();

        // Target kamera harus di sekitar tengah (60, 70)
        assert!((app.camera.target.x - 60.0).abs() < 1.0);
        assert!((app.camera.target.y - 70.0).abs() < 1.0);
    }
}
