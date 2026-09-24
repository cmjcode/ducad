//! Model coretan tinta bebas, kuas, filter input, dan manipulasi spasial DUCAD.

pub mod brush;
pub mod commands;
pub mod document;
pub mod filter;
pub mod predict;
pub mod stroke;

pub use brush::{Brush, BrushId, BrushKind, PressureCurve};
pub use commands::{
    AddStroke, DeleteStrokes, MoveStrokesToLayer, ReplacePoints, SetStrokeColor,
    SetStrokesHidden, SplitStroke, TransformStrokes,
};
pub use document::InkDoc;
pub use filter::OneEuro;
pub use predict::StrokeBuilder;
pub use stroke::{InkPoint, Stroke};

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_core::Command;
    use ducad_sketch::layer::LayerId;
    use ducad_sketch::style::Rgba;
    use glam::Vec2;
    use slotmap::SlotMap;

    #[test]
    fn brush_width_at_pressure_respects_curve() {
        let b_linear = Brush {
            name: "Test Linear".to_string(),
            kind: BrushKind::Pen,
            width_min_mm: 1.0,
            width_max_mm: 5.0,
            pressure_curve: PressureCurve::Linear,
            opacity: 1.0,
            smoothing: 0.0,
        };
        assert_eq!(b_linear.width_at(0.0), 1.0);
        assert_eq!(b_linear.width_at(1.0), 5.0);
        assert!((b_linear.width_at(0.5) - 3.0).abs() < 1e-5);

        let b_ease = Brush {
            name: "Test Ease".to_string(),
            kind: BrushKind::Pencil,
            width_min_mm: 1.0,
            width_max_mm: 5.0,
            pressure_curve: PressureCurve::Ease(2.0),
            opacity: 1.0,
            smoothing: 0.0,
        };
        // p = 0.5 -> p^2 = 0.25 -> 1.0 + 4.0 * 0.25 = 2.0
        assert!((b_ease.width_at(0.5) - 2.0).abs() < 1e-5);
    }

    #[test]
    fn stroke_bbox_matches_points() {
        let pts = vec![
            InkPoint::new(10.0, 5.0, 0.5, 0.0, 0),
            InkPoint::new(-2.0, 20.0, 0.7, 0.0, 10),
            InkPoint::new(15.0, 8.0, 0.4, 0.0, 20),
        ];
        let mut sm: SlotMap<BrushId, Brush> = SlotMap::with_key();
        let bid = sm.insert(Brush::presets().pop().unwrap());
        let mut lm: SlotMap<LayerId, ()> = SlotMap::with_key();
        let lid = lm.insert(());

        let stroke = Stroke::new(1, pts, bid, Rgba([0.0, 0.0, 0.0, 1.0]), lid);
        assert_eq!(stroke.bbox.0, Vec2::new(-2.0, 5.0));
        assert_eq!(stroke.bbox.1, Vec2::new(15.0, 20.0));
        assert!(stroke.length() > 0.0);
    }

    #[test]
    fn hit_returns_topmost_stroke() {
        let mut doc = InkDoc::default();
        let bid = doc.brushes.keys().next().unwrap();
        let mut lm: SlotMap<LayerId, ()> = SlotMap::with_key();
        let lid = lm.insert(());

        // Stroke 1 (bawah): garis horizontal y = 10, x = 0..20
        let s1 = Stroke::new(
            0,
            vec![
                InkPoint::new(0.0, 10.0, 0.5, 0.0, 0),
                InkPoint::new(20.0, 10.0, 0.5, 0.0, 10),
            ],
            bid,
            Rgba([0.0, 0.0, 0.0, 1.0]),
            lid,
        );
        let id1 = doc.add_stroke(s1);

        // Stroke 2 (atas, overlap): garis vertikal x = 10, y = 0..20
        let s2 = Stroke::new(
            0,
            vec![
                InkPoint::new(10.0, 0.0, 0.5, 0.0, 0),
                InkPoint::new(10.0, 20.0, 0.5, 0.0, 10),
            ],
            bid,
            Rgba([1.0, 0.0, 0.0, 1.0]),
            lid,
        );
        let id2 = doc.add_stroke(s2);

        // Klik tepat di titik potong (10, 10) harus mengembalikan stroke teratas (id2)
        assert_eq!(doc.hit(Vec2::new(10.0, 10.0), 0.5), Some(id2));
        // Klik di (5, 10) hanya kena stroke 1
        assert_eq!(doc.hit(Vec2::new(5.0, 10.0), 0.5), Some(id1));
        // Klik jauh (50, 50) tidak kena apa-apa
        assert_eq!(doc.hit(Vec2::new(50.0, 50.0), 0.5), None);
    }

    #[test]
    fn visible_in_filters_by_bbox_and_layer() {
        let mut doc = InkDoc::default();
        let bid = doc.brushes.keys().next().unwrap();
        let mut lm: SlotMap<LayerId, ()> = SlotMap::with_key();
        let l1 = lm.insert(());
        let l2 = lm.insert(());

        let s1 = Stroke::new(
            0,
            vec![InkPoint::new(0.0, 0.0, 0.5, 0.0, 0), InkPoint::new(10.0, 10.0, 0.5, 0.0, 10)],
            bid,
            Rgba([0.0, 0.0, 0.0, 1.0]),
            l1,
        );
        let id1 = doc.add_stroke(s1);

        let s2 = Stroke::new(
            0,
            vec![InkPoint::new(50.0, 50.0, 0.5, 0.0, 0), InkPoint::new(60.0, 60.0, 0.5, 0.0, 10)],
            bid,
            Rgba([0.0, 0.0, 0.0, 1.0]),
            l2,
        );
        let id2 = doc.add_stroke(s2);

        // Viewport mencakup keduanya, tapi layer l2 disembunyikan
        let visible = doc.visible_in(Vec2::new(-10.0, -10.0), Vec2::new(100.0, 100.0), &|lid| lid == l1);
        assert_eq!(visible, vec![id1]);

        // Viewport hanya di area stroke 2
        let visible2 = doc.visible_in(Vec2::new(40.0, 40.0), Vec2::new(70.0, 70.0), &|_| true);
        assert_eq!(visible2, vec![id2]);
    }

    #[test]
    fn serde_roundtrip_stroke() {
        let mut lm: SlotMap<LayerId, ()> = SlotMap::with_key();
        let lid = lm.insert(());
        let mut sm: SlotMap<BrushId, ()> = SlotMap::with_key();
        let bid = sm.insert(());

        let stroke = Stroke::new(
            42,
            vec![
                InkPoint::new(1.0, 2.0, 0.8, 0.1, 100),
                InkPoint::new(3.0, 4.0, 0.6, 0.2, 120),
            ],
            bid,
            Rgba([0.2, 0.4, 0.6, 1.0]),
            lid,
        );

        let json = serde_json::to_string(&stroke).unwrap();
        let mut deserialized: Stroke = serde_json::from_str(&json).unwrap();
        deserialized.recompute_bbox();

        assert_eq!(stroke, deserialized);
    }

    #[test]
    fn memory_bytes_grows_with_points() {
        let mut doc = InkDoc::default();
        let initial_bytes = doc.memory_bytes();

        let bid = doc.brushes.keys().next().unwrap();
        let mut lm: SlotMap<LayerId, ()> = SlotMap::with_key();
        let lid = lm.insert(());

        let pts: Vec<InkPoint> = (0..500)
            .map(|i| InkPoint::new(i as f32, i as f32, 0.5, 0.0, i))
            .collect();
        doc.add_stroke(Stroke::new(0, pts, bid, Rgba([1.0, 1.0, 1.0, 1.0]), lid));

        assert!(doc.memory_bytes() > initial_bytes);
    }

    #[test]
    fn one_euro_passes_constant_signal() {
        let mut filter = OneEuro::new(1.0, 0.01, 1.0);
        let constant = 42.5;
        for i in 0..100 {
            let t = i as f32 * 0.01;
            let out = filter.filter(constant, t);
            assert!((out - constant).abs() < 1e-4);
        }
    }

    #[test]
    fn one_euro_reduces_jitter_variance() {
        let mut filter = OneEuro::from_smoothing(0.5);
        let n = 200;
        let mut raw_samples = Vec::with_capacity(n);
        let mut filtered_samples = Vec::with_capacity(n);

        // Pseudo-random noise with fixed seed: LCG
        let mut state: u64 = 123456789;
        let base_signal = 10.0;

        for i in 0..n {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let noise = ((state >> 33) as f32 / (1u64 << 31) as f32 - 0.5) * 2.0; // [-1.0, 1.0]
            let val = base_signal + noise;
            let t = i as f32 * 0.01; // 100 Hz
            let out = filter.filter(val, t);

            if i >= 20 {
                raw_samples.push(val);
                filtered_samples.push(out);
            }
        }

        let mean_raw = raw_samples.iter().sum::<f32>() / raw_samples.len() as f32;
        let var_raw = raw_samples.iter().map(|x| (x - mean_raw).powi(2)).sum::<f32>() / raw_samples.len() as f32;

        let mean_filt = filtered_samples.iter().sum::<f32>() / filtered_samples.len() as f32;
        let var_filt = filtered_samples.iter().map(|x| (x - mean_filt).powi(2)).sum::<f32>() / filtered_samples.len() as f32;

        assert!(
            var_filt < 0.30 * var_raw,
            "variance ratio was {}, expected < 0.30",
            var_filt / var_raw
        );
    }

    #[test]
    fn builder_drops_points_closer_than_min_dist() {
        let brush = Brush::presets().pop().unwrap();
        let mut builder = StrokeBuilder::new(&brush);

        let p1 = builder.push(InkPoint::new(10.0, 10.0, 0.5, 0.0, 0));
        assert!(p1.is_some());

        // Point closer than 0.05 mm and delta_pressure <= 0.1 should be dropped
        let p2 = builder.push(InkPoint::new(10.02, 10.02, 0.52, 0.0, 10)); // dist ≈ 0.028 mm < 0.05
        assert!(p2.is_none());

        // Point further than 0.05 mm should be accepted
        let p3 = builder.push(InkPoint::new(10.1, 10.0, 0.5, 0.0, 20)); // dist = 0.08 mm > 0.05
        assert!(p3.is_some());

        let pts = builder.finish();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn predict_extrapolates_along_velocity() {
        let brush = Brush::presets().pop().unwrap();
        let mut builder = StrokeBuilder::new(&brush);

        // Moving horizontally at 10 mm per 100 ms = 0.1 mm/ms
        builder.push(InkPoint::new(0.0, 5.0, 0.5, 0.0, 0));
        builder.push(InkPoint::new(1.0, 5.0, 0.5, 0.0, 10));
        let p3 = builder
            .push(InkPoint::new(2.0, 5.0, 0.5, 0.0, 20))
            .expect("Third point pushed");

        let pred = builder.predict(25).expect("Prediction should be available for >= 3 points");
        assert!(pred.x > p3.x, "expected pred.x ({}) > p3.x ({})", pred.x, p3.x);
        assert!((pred.y - p3.y).abs() < 1e-3);
        assert_eq!(pred.t_ms, 25);
    }

    #[test]
    fn predict_is_bounded() {
        let brush = Brush::presets().pop().unwrap();
        let mut builder = StrokeBuilder::new(&brush);

        // Ultra high speed points
        builder.push(InkPoint::new(0.0, 0.0, 0.5, 0.0, 0));
        builder.push(InkPoint::new(100.0, 0.0, 0.5, 0.0, 10));
        let p3 = builder
            .push(InkPoint::new(200.0, 0.0, 0.5, 0.0, 20))
            .expect("Point pushed");

        let pred = builder.predict(1000).expect("Prediction available");
        // Time bounded to at most +8 ms
        assert!(pred.t_ms <= p3.t_ms + 8);
        // Distance displacement bounded to at most 2.0 mm
        let disp = Vec2::new(pred.x - p3.x, pred.y - p3.y);
        assert!(disp.length() <= 2.0001);
    }

    #[test]
    fn builder_is_deterministic() {
        let brush = Brush::presets().pop().unwrap();
        let mut b1 = StrokeBuilder::new(&brush);
        let mut b2 = StrokeBuilder::new(&brush);

        let inputs = vec![
            InkPoint::new(0.0, 0.0, 0.2, 0.0, 0),
            InkPoint::new(0.02, 0.02, 0.21, 0.0, 5),
            InkPoint::new(1.0, 2.0, 0.5, 0.1, 15),
            InkPoint::new(2.5, 4.0, 0.8, 0.2, 30),
            InkPoint::new(2.52, 4.01, 0.81, 0.2, 35),
            InkPoint::new(5.0, 8.0, 0.9, 0.3, 50),
        ];

        for &pt in &inputs {
            b1.push(pt);
            b2.push(pt);
        }

        let res1 = b1.finish();
        let res2 = b2.finish();

        assert_eq!(res1, res2);
    }

    #[test]
    fn apply_then_revert_restores_state() {
        let mut lm: SlotMap<LayerId, ()> = SlotMap::with_key();
        let l1 = lm.insert(());
        let l2 = lm.insert(());

        let mut doc = InkDoc::default();
        let bid = doc.brushes.keys().next().unwrap();

        // 1. AddStroke
        let s = Stroke::new(
            0,
            vec![
                InkPoint::new(0.0, 0.0, 0.5, 0.0, 0),
                InkPoint::new(10.0, 10.0, 0.5, 0.0, 10),
            ],
            bid,
            Rgba([0.0, 0.0, 0.0, 1.0]),
            l1,
        );
        let mut add_cmd = AddStroke::new(s);
        let base_doc = doc.clone();
        add_cmd.apply(&mut doc);
        assert_eq!(doc.strokes.len(), 1);
        add_cmd.revert(&mut doc);
        assert_eq!(doc, base_doc);

        // Apply it again so we have a stroke to work with
        add_cmd.apply(&mut doc);
        let stroke_id = add_cmd.stroke_id().unwrap();

        // 2. SetStrokesHidden
        let before_hidden = doc.clone();
        let mut hide_cmd = SetStrokesHidden::new(vec![stroke_id], true);
        hide_cmd.apply(&mut doc);
        assert!(doc.stroke(stroke_id).unwrap().hidden);
        hide_cmd.revert(&mut doc);
        assert_eq!(doc, before_hidden);

        // 3. SetStrokeColor
        let before_color = doc.clone();
        let mut color_cmd = SetStrokeColor::new(vec![stroke_id], Rgba([1.0, 0.0, 0.0, 1.0]));
        color_cmd.apply(&mut doc);
        assert_eq!(doc.stroke(stroke_id).unwrap().color, Rgba([1.0, 0.0, 0.0, 1.0]));
        color_cmd.revert(&mut doc);
        assert_eq!(doc, before_color);

        // 4. MoveStrokesToLayer
        let before_layer = doc.clone();
        let mut layer_cmd = MoveStrokesToLayer::new(vec![stroke_id], l2);
        layer_cmd.apply(&mut doc);
        assert_eq!(doc.stroke(stroke_id).unwrap().layer, l2);
        layer_cmd.revert(&mut doc);
        assert_eq!(doc, before_layer);

        // 5. TransformStrokes
        let before_xform = doc.clone();
        let mut xform_cmd =
            TransformStrokes::new(vec![stroke_id], kurbo::Affine::translate((5.0, 5.0)));
        xform_cmd.apply(&mut doc);
        assert!((doc.stroke(stroke_id).unwrap().points[0].x - 5.0).abs() < 1e-4);
        xform_cmd.revert(&mut doc);
        assert_eq!(doc, before_xform);

        // 6. DeleteStrokes
        let before_del = doc.clone();
        let mut del_cmd = DeleteStrokes::new(vec![stroke_id]);
        del_cmd.apply(&mut doc);
        assert_eq!(doc.strokes.len(), 0);
        del_cmd.revert(&mut doc);
        assert_eq!(doc, before_del);
    }

    #[test]
    fn split_stroke_preserves_all_points() {
        let mut doc = InkDoc::default();
        let bid = doc.brushes.keys().next().unwrap();
        let mut lm: SlotMap<LayerId, ()> = SlotMap::with_key();
        let lid = lm.insert(());

        let pts: Vec<InkPoint> = (0..10)
            .map(|i| InkPoint::new(i as f32, i as f32 * 2.0, 0.5, 0.0, i * 10))
            .collect();
        let stroke = Stroke::new(0, pts.clone(), bid, Rgba([0.0, 0.0, 0.0, 1.0]), lid);
        let id = doc.add_stroke(stroke);
        let before_split = doc.clone();

        let mut cmd = SplitStroke::new(id, vec![3, 7]);
        cmd.apply(&mut doc);

        assert_eq!(doc.strokes.len(), 3);
        assert_eq!(doc.strokes[0].points.len(), 3); // 0..3
        assert_eq!(doc.strokes[1].points.len(), 4); // 3..7
        assert_eq!(doc.strokes[2].points.len(), 3); // 7..10

        let mut combined_pts = Vec::new();
        for s in &doc.strokes {
            combined_pts.extend_from_slice(&s.points);
        }
        assert_eq!(combined_pts, pts);

        // Revert restores original state
        cmd.revert(&mut doc);
        assert_eq!(doc, before_split);
    }

    #[test]
    fn replace_points_revert_restores_original() {
        let mut doc = InkDoc::default();
        let bid = doc.brushes.keys().next().unwrap();
        let mut lm: SlotMap<LayerId, ()> = SlotMap::with_key();
        let lid = lm.insert(());

        let orig_pts: Vec<InkPoint> = (0..5)
            .map(|i| InkPoint::new(i as f32, 0.0, 0.5, 0.0, i * 10))
            .collect();
        let id = doc.add_stroke(Stroke::new(
            0,
            orig_pts.clone(),
            bid,
            Rgba([0.0, 0.0, 0.0, 1.0]),
            lid,
        ));
        let before_replace = doc.clone();

        let new_pts: Vec<InkPoint> = (0..8)
            .map(|i| InkPoint::new(i as f32 * 3.0, 10.0, 0.8, 0.1, i * 15))
            .collect();
        let mut cmd = ReplacePoints::new(id, new_pts.clone());
        cmd.apply(&mut doc);

        assert_eq!(doc.stroke(id).unwrap().points, new_pts);

        cmd.revert(&mut doc);
        assert_eq!(doc, before_replace);
        assert_eq!(doc.stroke(id).unwrap().points, orig_pts);
    }

    #[test]
    fn ink_has_no_gui_dependency() {
        let manifest = include_str!("../Cargo.toml");
        for banned in [
            "egui",
            "eframe",
            "wgpu",
            "ducad-app",
            "ducad-ui",
            "ducad-engine",
            "ducad-kernel",
            "opencascade",
            "occt-sys",
            "rfd",
        ] {
            assert!(
                !manifest.contains(banned),
                "ducad-ink tidak boleh bergantung pada {banned}"
            );
        }
    }
}
