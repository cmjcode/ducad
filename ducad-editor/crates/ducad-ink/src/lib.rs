//! Model coretan tinta bebas, kuas, filter input, dan manipulasi spasial DUCAD.

pub mod brush;
pub mod document;
pub mod stroke;

pub use brush::{Brush, BrushId, BrushKind, PressureCurve};
pub use document::InkDoc;
pub use stroke::{InkPoint, Stroke};

#[cfg(test)]
mod tests {
    use super::*;
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
