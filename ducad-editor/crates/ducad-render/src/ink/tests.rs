use crate::plane::SketchPlane;
use super::stroke::{
    append_stroke_vertices, build_stroke_vertices, InkBrushKind, InkBrushRef, InkPointRef,
};

#[test]
fn stroke_vertex_count_is_two_per_point_plus_caps() {
    let plane = SketchPlane::top();
    let brush = InkBrushRef::default();

    // 1 titik -> 2 * 1 + 4 = 6
    let points1 = vec![InkPointRef {
        pos: [0.0, 0.0],
        pressure: 0.5,
    }];
    let mut out1 = Vec::new();
    build_stroke_vertices(&points1, &brush, &plane, &mut out1);
    assert_eq!(out1.len(), 6);

    // 2 titik -> 2 * 2 + 4 = 8
    let points2 = vec![
        InkPointRef {
            pos: [0.0, 0.0],
            pressure: 0.5,
        },
        InkPointRef {
            pos: [10.0, 0.0],
            pressure: 0.5,
        },
    ];
    let mut out2 = Vec::new();
    build_stroke_vertices(&points2, &brush, &plane, &mut out2);
    assert_eq!(out2.len(), 8);

    // 10 titik -> 2 * 10 + 4 = 24
    let points10: Vec<InkPointRef> = (0..10)
        .map(|i| InkPointRef {
            pos: [i as f32 * 2.0, (i as f32).sin() * 5.0],
            pressure: 0.3 + 0.05 * (i as f32),
        })
        .collect();
    let mut out10 = Vec::new();
    build_stroke_vertices(&points10, &brush, &plane, &mut out10);
    assert_eq!(out10.len(), 24);
}

#[test]
fn append_equals_full_rebuild() {
    let plane = SketchPlane::top();
    let brush = InkBrushRef {
        color: [0.2, 0.4, 0.8, 1.0],
        width_min: 1.0,
        width_max: 5.0,
        opacity: 0.9,
        kind: InkBrushKind::Pencil,
    };

    let points: Vec<InkPointRef> = (0..15)
        .map(|i| InkPointRef {
            pos: [i as f32 * 3.0, (i as f32 * 0.5).cos() * 8.0],
            pressure: (i as f32 / 15.0).clamp(0.1, 0.9),
        })
        .collect();

    // 1. Full rebuild
    let mut full = Vec::new();
    build_stroke_vertices(&points, &brush, &plane, &mut full);

    // 2. Parsial k=7 lalu append
    let k = 7;
    let mut incremental = Vec::new();
    build_stroke_vertices(&points[..k], &brush, &plane, &mut incremental);
    append_stroke_vertices(&points, k, &brush, &plane, &mut incremental);

    assert_eq!(full.len(), incremental.len());
    for (i, (vf, vi)) in full.iter().zip(incremental.iter()).enumerate() {
        assert!(
            (vf.pos[0] - vi.pos[0]).abs() < 1e-4
                && (vf.pos[1] - vi.pos[1]).abs() < 1e-4
                && (vf.pos[2] - vi.pos[2]).abs() < 1e-4,
            "Vertex pos mismatch pada index {i}: full={:?}, inc={:?}",
            vf.pos,
            vi.pos
        );
        assert_eq!(vf.side, vi.side, "Side mismatch pada index {i}");
        assert_eq!(vf.color, vi.color, "Color mismatch pada index {i}");
        assert_eq!(vf.soft, vi.soft, "Soft mismatch pada index {i}");
    }
}

#[test]
fn pressure_scales_width() {
    let plane = SketchPlane::top();
    let brush = InkBrushRef {
        color: [0.0, 0.0, 0.0, 1.0],
        width_min: 1.0,
        width_max: 10.0,
        opacity: 1.0,
        kind: InkBrushKind::Pen,
    };

    let points_low = vec![
        InkPointRef {
            pos: [0.0, 0.0],
            pressure: 0.1,
        },
        InkPointRef {
            pos: [10.0, 0.0],
            pressure: 0.1,
        },
    ];
    let mut out_low = Vec::new();
    build_stroke_vertices(&points_low, &brush, &plane, &mut out_low);

    let points_high = vec![
        InkPointRef {
            pos: [0.0, 0.0],
            pressure: 0.9,
        },
        InkPointRef {
            pos: [10.0, 0.0],
            pressure: 0.9,
        },
    ];
    let mut out_high = Vec::new();
    build_stroke_vertices(&points_high, &brush, &plane, &mut out_high);

    // Titik 0 adalah index 2 (left) dan 3 (right)
    let left_low = out_low[2].pos;
    let right_low = out_low[3].pos;
    let width_low = ((left_low[0] - right_low[0]).powi(2)
        + (left_low[1] - right_low[1]).powi(2)
        + (left_low[2] - right_low[2]).powi(2))
    .sqrt();

    let left_high = out_high[2].pos;
    let right_high = out_high[3].pos;
    let width_high = ((left_high[0] - right_high[0]).powi(2)
        + (left_high[1] - right_high[1]).powi(2)
        + (left_high[2] - right_high[2]).powi(2))
    .sqrt();

    assert!(
        width_high > width_low * 2.0,
        "Tekanan tinggi (w={width_high}) harus menghasilkan lebar jauh lebih besar dari tekanan rendah (w={width_low})"
    );
}

#[test]
fn single_point_stroke_makes_dot() {
    let plane = SketchPlane::top();
    let brush = InkBrushRef {
        color: [1.0, 0.0, 0.0, 1.0],
        width_min: 2.0,
        width_max: 8.0,
        opacity: 1.0,
        kind: InkBrushKind::Marker,
    };

    let pt = vec![InkPointRef {
        pos: [5.0, 5.0],
        pressure: 0.5,
    }];
    let mut out = Vec::new();
    build_stroke_vertices(&pt, &brush, &plane, &mut out);

    assert_eq!(out.len(), 6, "Satu titik harus menghasilkan 6 vertex");
    for v in &out {
        assert!(v.pos[0].is_finite());
        assert!(v.pos[1].is_finite());
        assert!(v.pos[2].is_finite());
        assert!(v.side.abs() <= 1.0);
    }
}

#[test]
fn no_nan_for_duplicate_points() {
    let plane = SketchPlane::top();
    let brush = InkBrushRef::default();

    let duplicate_points = vec![
        InkPointRef {
            pos: [10.0, 10.0],
            pressure: 0.5,
        },
        InkPointRef {
            pos: [10.0, 10.0],
            pressure: 0.5,
        },
        InkPointRef {
            pos: [10.0, 10.0],
            pressure: 0.5,
        },
    ];

    let mut out = Vec::new();
    build_stroke_vertices(&duplicate_points, &brush, &plane, &mut out);

    assert_eq!(out.len(), 2 * 3 + 4);
    for (i, v) in out.iter().enumerate() {
        assert!(!v.pos[0].is_nan(), "pos[0] NaN at index {i}");
        assert!(!v.pos[1].is_nan(), "pos[1] NaN at index {i}");
        assert!(!v.pos[2].is_nan(), "pos[2] NaN at index {i}");
        assert!(!v.side.is_nan(), "side NaN at index {i}");
    }
}

#[test]
fn wgsl_ink_shader_compiles() {
    let shader_str = include_str!("shader_ink.wgsl");
    let module = egui_wgpu::wgpu::naga::front::wgsl::parse_str(shader_str);
    assert!(module.is_ok(), "WGSL parse error: {:?}", module.err());
}
