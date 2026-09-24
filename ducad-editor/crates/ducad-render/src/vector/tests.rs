use ducad_sketch::{
    Entity, FillRule, LineCap, LineJoin, Paint, PathSeg, Rgba, StrokeStyle, Style, Subpath,
};
use glam::DVec2;

use super::tessellate::{
    subpaths_to_stroked_polylines, tessellate_entity, tessellate_fill, tessellate_stroke,
    TessError, TessOptions, Tessellated,
};
use crate::plane::SketchPlane;

fn total_area(tess: &Tessellated) -> f64 {
    let mut area = 0.0;
    for chunk in tess.indices.chunks_exact(3) {
        let v0 = tess.vertices[chunk[0] as usize].uv;
        let v1 = tess.vertices[chunk[1] as usize].uv;
        let v2 = tess.vertices[chunk[2] as usize].uv;
        let tri_area = 0.5
            * ((v1[0] - v0[0]) * (v2[1] - v0[1]) - (v2[0] - v0[0]) * (v1[1] - v0[1])).abs()
                as f64;
        area += tri_area;
    }
    area
}

#[test]
fn unit_square_fill_area_is_one() {
    let sub = Subpath {
        start: DVec2::new(0.0, 0.0),
        segs: vec![
            PathSeg::Line {
                end: DVec2::new(1.0, 0.0),
            },
            PathSeg::Line {
                end: DVec2::new(1.0, 1.0),
            },
            PathSeg::Line {
                end: DVec2::new(0.0, 1.0),
            },
        ],
        closed: true,
    };
    let fill = Paint::Solid(Rgba::BLACK);
    let plane = SketchPlane::top();
    let opts = TessOptions::default();

    let tess = tessellate_fill(&[sub], &fill, FillRule::NonZero, &plane, &opts).unwrap();
    let area = total_area(&tess);
    assert!(
        (area - 1.0).abs() < 1e-4,
        "Luas unit square ({area}) harus mendekati 1.0"
    );
}

#[test]
fn even_odd_ring_has_hole() {
    // Persegi luar 10×10 (luas 100)
    let outer = Subpath {
        start: DVec2::new(0.0, 0.0),
        segs: vec![
            PathSeg::Line {
                end: DVec2::new(10.0, 0.0),
            },
            PathSeg::Line {
                end: DVec2::new(10.0, 10.0),
            },
            PathSeg::Line {
                end: DVec2::new(0.0, 10.0),
            },
        ],
        closed: true,
    };
    // Persegi dalam 4×4 di posisi (3,3) (luas 16)
    let inner = Subpath {
        start: DVec2::new(3.0, 3.0),
        segs: vec![
            PathSeg::Line {
                end: DVec2::new(7.0, 3.0),
            },
            PathSeg::Line {
                end: DVec2::new(7.0, 7.0),
            },
            PathSeg::Line {
                end: DVec2::new(3.0, 7.0),
            },
        ],
        closed: true,
    };
    let fill = Paint::Solid(Rgba::BLACK);
    let plane = SketchPlane::top();
    let opts = TessOptions::default();

    let tess =
        tessellate_fill(&[outer, inner], &fill, FillRule::EvenOdd, &plane, &opts).unwrap();
    let area = total_area(&tess);
    assert!(
        (area - 84.0).abs() < 1e-3,
        "Luas ring even-odd ({area}) harus mendekati 84.0 (100 - 16)"
    );
}

#[test]
fn nonzero_same_winding_ring_is_filled() {
    // Persegi luar 10×10 searah jarum jam / CCW
    let outer = Subpath {
        start: DVec2::new(0.0, 0.0),
        segs: vec![
            PathSeg::Line {
                end: DVec2::new(10.0, 0.0),
            },
            PathSeg::Line {
                end: DVec2::new(10.0, 10.0),
            },
            PathSeg::Line {
                end: DVec2::new(0.0, 10.0),
            },
        ],
        closed: true,
    };
    // Persegi dalam 4×4 dengan winding yang SAMA (keduanya CCW)
    let inner = Subpath {
        start: DVec2::new(3.0, 3.0),
        segs: vec![
            PathSeg::Line {
                end: DVec2::new(7.0, 3.0),
            },
            PathSeg::Line {
                end: DVec2::new(7.0, 7.0),
            },
            PathSeg::Line {
                end: DVec2::new(3.0, 7.0),
            },
        ],
        closed: true,
    };
    let fill = Paint::Solid(Rgba::BLACK);
    let plane = SketchPlane::top();
    let opts = TessOptions::default();

    let tess =
        tessellate_fill(&[outer, inner], &fill, FillRule::NonZero, &plane, &opts).unwrap();
    let area = total_area(&tess);
    assert!(
        (area - 100.0).abs() < 1e-3,
        "Luas non-zero same winding ({area}) harus 100.0 (seluruh area terisi)"
    );
}

#[test]
fn stroke_width_produces_expected_area() {
    // Garis lurus sepanjang 10 mm
    let poly = vec![DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0)];
    let stroke = StrokeStyle {
        paint: Paint::Solid(Rgba::BLACK),
        width_mm: 2.0,
        dash: vec![],
        cap: LineCap::Butt,
        join: LineJoin::Miter,
    };
    let plane = SketchPlane::top();
    let opts = TessOptions::default();

    let tess = tessellate_stroke(&[poly], &stroke, &plane, &opts);
    let area = total_area(&tess);
    assert!(
        (area - 20.0).abs() < 0.05,
        "Luas stroke butt cap ({area}) harus mendekati 20.0 mm² (10 mm × 2 mm)"
    );
}

#[test]
fn dash_pattern_splits_into_segments() {
    let sub = Subpath {
        start: DVec2::new(0.0, 0.0),
        segs: vec![PathSeg::Line {
            end: DVec2::new(10.0, 0.0),
        }],
        closed: false,
    };
    // Garis 10 mm dengan pola [2.0, 1.0] -> segmen [0..2], [3..5], [6..8], [9..10] = 4 segmen
    let segments = subpaths_to_stroked_polylines(&[sub], &[2.0, 1.0], 0.01);
    assert_eq!(
        segments.len(),
        4,
        "Pola dash [2.0, 1.0] sepanjang 10 mm harus menghasilkan 4 segmen terpisah"
    );
}

#[test]
fn tessellate_is_deterministic() {
    let sub = Subpath {
        start: DVec2::new(0.0, 0.0),
        segs: vec![
            PathSeg::Cubic {
                c1: DVec2::new(20.0, 0.0),
                c2: DVec2::new(20.0, 30.0),
                end: DVec2::new(0.0, 30.0),
            },
            PathSeg::Line {
                end: DVec2::new(0.0, 0.0),
            },
        ],
        closed: true,
    };
    let entity = Entity::path(vec![sub]);
    let style = Style {
        fill: Some(Paint::Solid(Rgba([1.0, 0.5, 0.2, 1.0]))),
        fill_rule: FillRule::NonZero,
        stroke: Some(StrokeStyle {
            paint: Paint::Solid(Rgba::BLACK),
            width_mm: 1.0,
            dash: vec![2.0, 2.0],
            cap: LineCap::Round,
            join: LineJoin::Round,
        }),
        opacity: 0.8,
        blend: ducad_sketch::BlendMode::Normal,
    };
    let plane = SketchPlane::top();
    let opts = TessOptions::default();

    let t1 = tessellate_entity(&entity, &style, &plane, &opts).unwrap();
    let t2 = tessellate_entity(&entity, &style, &plane, &opts).unwrap();

    assert_eq!(
        t1.vertices, t2.vertices,
        "Hasil vertex tesselasi harus deterministik dan identik"
    );
    assert_eq!(
        t1.indices, t2.indices,
        "Hasil indeks tesselasi harus deterministik dan identik"
    );
}

#[test]
fn degenerate_path_does_not_panic() {
    let fill = Paint::Solid(Rgba::BLACK);
    let plane = SketchPlane::top();
    let opts = TessOptions::default();

    // 1. Path kosong (0 subpaths)
    let res0 = tessellate_fill(&[], &fill, FillRule::NonZero, &plane, &opts);
    assert!(
        matches!(res0, Err(TessError::Degenerate(_))),
        "0 titik harus mengembalikan TessError::Degenerate"
    );

    // 2. 1 titik (hanya start, 0 segmen)
    let sub1 = Subpath {
        start: DVec2::new(5.0, 5.0),
        segs: vec![],
        closed: false,
    };
    let res1 = tessellate_fill(&[sub1], &fill, FillRule::NonZero, &plane, &opts);
    assert!(
        matches!(res1, Err(TessError::Degenerate(_))),
        "1 titik harus mengembalikan TessError::Degenerate"
    );

    // 3. 2 titik (1 segmen garis)
    let sub2 = Subpath {
        start: DVec2::new(0.0, 0.0),
        segs: vec![PathSeg::Line {
            end: DVec2::new(5.0, 5.0),
        }],
        closed: false,
    };
    let res2 = tessellate_fill(&[sub2], &fill, FillRule::NonZero, &plane, &opts);
    assert!(
        matches!(res2, Err(TessError::Degenerate(_))),
        "2 titik harus mengembalikan TessError::Degenerate"
    );

    // 4. Koordinat NaN
    let sub_nan = Subpath {
        start: DVec2::new(f64::NAN, 0.0),
        segs: vec![
            PathSeg::Line {
                end: DVec2::new(10.0, 0.0),
            },
            PathSeg::Line {
                end: DVec2::new(10.0, 10.0),
            },
        ],
        closed: true,
    };
    let res_nan = tessellate_fill(&[sub_nan], &fill, FillRule::NonZero, &plane, &opts);
    assert!(
        matches!(res_nan, Err(TessError::NonFiniteCoordinate)),
        "Koordinat NaN harus mengembalikan TessError::NonFiniteCoordinate"
    );
}
