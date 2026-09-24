use ducad_io::native::{
    deserialize_from_json, needs_v3, serialize_detailed_to_json_with_design, FORMAT_VERSION,
    FORMAT_VERSION_DESIGN, FORMAT_VERSION_MINIMAL,
};
use ducad_sketch::entity::{Entity, PathSeg, Subpath};
use ducad_sketch::layer::{Layer, Origin};
use ducad_sketch::style::{BlendMode, FillRule, LineCap, LineJoin, Paint, Rgba, StrokeStyle, Style};
use ducad_sketch::Sketch;
use glam::DVec2;
use proptest::prelude::*;

const V1_MINIMAL_STR: &str = include_str!("fixtures/v1_minimal.ducad");
const V2_DESIGN_STR: &str = include_str!("fixtures/v2_design.ducad");
const V3_VECTOR_STR: &str = include_str!("fixtures/v3_vector.ducad");

#[test]
fn v3_fixture_loads() {
    let doc = deserialize_from_json(V3_VECTOR_STR).expect("v3 fixture harus bisa dimuat");
    assert_eq!(doc.sketch.entities.len(), 1);
    let entity = doc.sketch.entities.values().next().unwrap();
    match entity {
        Entity::Path { subpaths, .. } => {
            assert_eq!(subpaths.len(), 1);
            assert_eq!(subpaths[0].start, DVec2::new(0.0, 0.0));
            assert_eq!(subpaths[0].segs.len(), 3);
            assert!(subpaths[0].closed);
        }
        _ => panic!("Expected Entity::Path"),
    }
    assert_eq!(doc.sketch.styles.len(), 1);
    assert_eq!(doc.sketch.layers.len(), 1);
    assert!(needs_v3(&doc.sketch));
}

#[test]
fn v1_fixture_loads_identically() {
    let doc = deserialize_from_json(V1_MINIMAL_STR).expect("v1 fixture harus bisa dimuat");
    assert_eq!(doc.sketch.entities.len(), 1);
    let entity = doc.sketch.entities.values().next().unwrap();
    match entity {
        Entity::Line { start, end, .. } => {
            assert_eq!(*start, DVec2::new(0.0, 0.0));
            assert_eq!(*end, DVec2::new(10.0, 0.0));
        }
        _ => panic!("Expected Entity::Line"),
    }
    assert!(doc.sketch.styles.is_empty());
    assert!(doc.sketch.layers.is_empty());
    assert!(doc.design.is_none());

    // Memuat ulang dan memastikan tidak butuh v3
    assert!(!needs_v3(&doc.sketch));

    // Bandingkan roundtrip canonical JSON
    let canonical = serialize_detailed_to_json_with_design(&[&doc.sketch], &[], None).unwrap();
    let re_doc = deserialize_from_json(&canonical).unwrap();
    assert_eq!(re_doc.sketch.entities.len(), 1);
}

#[test]
fn v2_fixture_loads() {
    let doc = deserialize_from_json(V2_DESIGN_STR).expect("v2 fixture harus bisa dimuat");
    assert_eq!(doc.sketch.entities.len(), 1);
    let entity = doc.sketch.entities.values().next().unwrap();
    match entity {
        Entity::Circle { center, radius, .. } => {
            assert_eq!(*center, DVec2::new(0.0, 0.0));
            assert_eq!(*radius, 5.0);
        }
        _ => panic!("Expected Entity::Circle"),
    }
    assert!(doc.design.is_some());
    assert!(!needs_v3(&doc.sketch));
}

#[test]
fn document_with_path_saves_as_v3() {
    let mut sketch = Sketch::default();
    let path_ent = Entity::Path {
        subpaths: vec![Subpath {
            start: DVec2::new(0.0, 0.0),
            segs: vec![
                PathSeg::Line {
                    end: DVec2::new(10.0, 0.0),
                },
                PathSeg::Cubic {
                    c1: DVec2::new(10.0, 5.0),
                    c2: DVec2::new(5.0, 10.0),
                    end: DVec2::new(0.0, 10.0),
                },
                PathSeg::Line {
                    end: DVec2::new(0.0, 0.0),
                },
            ],
            closed: true,
        }],
        is_construction: false,
    };
    let id = sketch.entities.insert(path_ent.clone());
    sketch.styles.insert(
        id,
        Style {
            fill: Some(Paint::Solid(Rgba([1.0, 0.0, 0.0, 1.0]))),
            fill_rule: FillRule::NonZero,
            stroke: Some(StrokeStyle {
                paint: Paint::Solid(Rgba::BLACK),
                width_mm: 1.0,
                dash: vec![],
                cap: LineCap::Butt,
                join: LineJoin::Miter,
            }),
            opacity: 1.0,
            blend: BlendMode::Normal,
        },
    );
    let layer_id = sketch
        .layers
        .insert(Layer::new("VectorLayer", Rgba([0.0, 1.0, 0.0, 1.0])));
    sketch.layer_order.push(layer_id);
    sketch.entity_layer.insert(id, layer_id);
    sketch
        .origin
        .insert(id, Origin::Import { source: "svg:test.svg".into() });

    assert!(needs_v3(&sketch));

    let json = serialize_detailed_to_json_with_design(&[&sketch], &[], None).unwrap();
    let val: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(val["format_version"], FORMAT_VERSION);
    assert_eq!(val["format_version"], 3);

    // Pastikan bisa di-deserialize kembali tanpa kehilangan data
    let doc = deserialize_from_json(&json).expect("v3 doc harus bisa dimuat kembali");
    assert_eq!(doc.sketch.entities.len(), 1);
    let loaded_ent = doc.sketch.entities.get(id).unwrap();
    assert_eq!(*loaded_ent, path_ent);
    assert!(doc.sketch.styles.get(id).is_some());
    assert_eq!(doc.sketch.layers.len(), 1);
    assert_eq!(doc.sketch.entity_layer.get(id), Some(&layer_id));
}

#[test]
fn document_without_new_features_still_saves_as_v1_or_v2() {
    let mut sketch = Sketch::default();
    sketch
        .entities
        .insert(Entity::line(DVec2::ZERO, DVec2::new(5.0, 5.0)));
    assert!(!needs_v3(&sketch));

    // Tanpa design -> v1
    let json_v1 = serialize_detailed_to_json_with_design(&[&sketch], &[], None).unwrap();
    let val_v1: serde_json::Value = serde_json::from_str(&json_v1).unwrap();
    assert_eq!(val_v1["format_version"], FORMAT_VERSION_MINIMAL);
    assert_eq!(val_v1["format_version"], 1);

    // Dengan design -> v2
    let design_val = serde_json::json!({
        "version": 1,
        "ops": []
    });
    let json_v2 =
        serialize_detailed_to_json_with_design(&[&sketch], &[], Some(&design_val)).unwrap();
    let val_v2: serde_json::Value = serde_json::from_str(&json_v2).unwrap();
    assert_eq!(val_v2["format_version"], FORMAT_VERSION_DESIGN);
    assert_eq!(val_v2["format_version"], 2);
}

// Proptest untuk Style dan Subpath roundtrip
fn arb_dvec2() -> impl Strategy<Value = DVec2> {
    (-100000..=100000i64, -100000..=100000i64)
        .prop_map(|(x, y)| DVec2::new(x as f64 * 0.01, y as f64 * 0.01))
}

fn arb_rgba() -> impl Strategy<Value = Rgba> {
    (0.0f32..=1.0, 0.0f32..=1.0, 0.0f32..=1.0, 0.0f32..=1.0)
        .prop_map(|(r, g, b, a)| Rgba([r, g, b, a]))
}

fn arb_paint() -> impl Strategy<Value = Paint> {
    prop_oneof![
        arb_rgba().prop_map(Paint::Solid),
        (
            arb_dvec2(),
            arb_dvec2(),
            prop::collection::vec((0.0f64..=1.0, arb_rgba()), 2..5)
        )
            .prop_map(|(from, to, stops)| { Paint::Linear { from, to, stops } }),
        (
            arb_dvec2(),
            0.1f64..100.0,
            prop::collection::vec((0.0f64..=1.0, arb_rgba()), 2..5)
        )
            .prop_map(|(center, radius, stops)| { Paint::Radial { center, radius, stops } }),
    ]
}

fn arb_fill_rule() -> impl Strategy<Value = FillRule> {
    prop_oneof![Just(FillRule::NonZero), Just(FillRule::EvenOdd)]
}

fn arb_line_cap() -> impl Strategy<Value = LineCap> {
    prop_oneof![Just(LineCap::Butt), Just(LineCap::Round), Just(LineCap::Square)]
}

fn arb_line_join() -> impl Strategy<Value = LineJoin> {
    prop_oneof![Just(LineJoin::Miter), Just(LineJoin::Round), Just(LineJoin::Bevel)]
}

fn arb_blend_mode() -> impl Strategy<Value = BlendMode> {
    prop_oneof![
        Just(BlendMode::Normal),
        Just(BlendMode::Multiply),
        Just(BlendMode::Screen),
    ]
}

fn arb_stroke_style() -> impl Strategy<Value = StrokeStyle> {
    (
        arb_paint(),
        0.0f64..50.0,
        prop::collection::vec(0.1f64..10.0, 0..4),
        arb_line_cap(),
        arb_line_join(),
    )
        .prop_map(|(paint, width_mm, dash, cap, join)| StrokeStyle {
            paint,
            width_mm,
            dash,
            cap,
            join,
        })
}

fn arb_style() -> impl Strategy<Value = Style> {
    (
        prop::option::of(arb_paint()),
        arb_fill_rule(),
        prop::option::of(arb_stroke_style()),
        0.0f32..=1.0,
        arb_blend_mode(),
    )
        .prop_map(
            |(fill, fill_rule, stroke, opacity, blend)| Style {
                fill,
                fill_rule,
                stroke,
                opacity,
                blend,
            },
        )
}

fn arb_path_seg() -> impl Strategy<Value = PathSeg> {
    prop_oneof![
        arb_dvec2().prop_map(|end| PathSeg::Line { end }),
        (arb_dvec2(), arb_dvec2(), arb_dvec2())
            .prop_map(|(c1, c2, end)| PathSeg::Cubic { c1, c2, end }),
    ]
}

fn arb_subpath() -> impl Strategy<Value = Subpath> {
    (
        arb_dvec2(),
        prop::collection::vec(arb_path_seg(), 0..8),
        any::<bool>(),
    )
        .prop_map(|(start, segs, closed)| Subpath {
            start,
            segs,
            closed,
        })
}

fn subpath_approx_eq(a: &Subpath, b: &Subpath) -> bool {
    const TOL: f64 = 1e-9;
    if (a.start - b.start).length() > TOL || a.closed != b.closed || a.segs.len() != b.segs.len() {
        return false;
    }
    for (s1, s2) in a.segs.iter().zip(b.segs.iter()) {
        match (s1, s2) {
            (PathSeg::Line { end: e1 }, PathSeg::Line { end: e2 }) => {
                if (*e1 - *e2).length() > TOL {
                    return false;
                }
            }
            (
                PathSeg::Cubic { c1: a1, c2: a2, end: ae },
                PathSeg::Cubic { c1: b1, c2: b2, end: be },
            ) => {
                if (*a1 - *b1).length() > TOL
                    || (*a2 - *b2).length() > TOL
                    || (*ae - *be).length() > TOL
                {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

fn rgba_approx_eq(a: Rgba, b: Rgba) -> bool {
    const TOL: f32 = 1e-5;
    (a.0[0] - b.0[0]).abs() < TOL
        && (a.0[1] - b.0[1]).abs() < TOL
        && (a.0[2] - b.0[2]).abs() < TOL
        && (a.0[3] - b.0[3]).abs() < TOL
}

fn paint_approx_eq(a: &Paint, b: &Paint) -> bool {
    const TOL: f64 = 1e-9;
    match (a, b) {
        (Paint::Solid(c1), Paint::Solid(c2)) => rgba_approx_eq(*c1, *c2),
        (
            Paint::Linear {
                from: f1,
                to: t1,
                stops: s1,
            },
            Paint::Linear {
                from: f2,
                to: t2,
                stops: s2,
            },
        ) => {
            (*f1 - *f2).length() < TOL
                && (*t1 - *t2).length() < TOL
                && s1.len() == s2.len()
                && s1.iter().zip(s2.iter()).all(|((p1, c1), (p2, c2))| {
                    (p1 - p2).abs() < TOL && rgba_approx_eq(*c1, *c2)
                })
        }
        (
            Paint::Radial {
                center: c1,
                radius: r1,
                stops: s1,
            },
            Paint::Radial {
                center: c2,
                radius: r2,
                stops: s2,
            },
        ) => {
            (*c1 - *c2).length() < TOL
                && (r1 - r2).abs() < TOL
                && s1.len() == s2.len()
                && s1.iter().zip(s2.iter()).all(|((p1, c1), (p2, c2))| {
                    (p1 - p2).abs() < TOL && rgba_approx_eq(*c1, *c2)
                })
        }
        _ => false,
    }
}

fn stroke_approx_eq(a: &StrokeStyle, b: &StrokeStyle) -> bool {
    const TOL: f64 = 1e-9;
    paint_approx_eq(&a.paint, &b.paint)
        && (a.width_mm - b.width_mm).abs() < TOL
        && a.cap == b.cap
        && a.join == b.join
        && a.dash.len() == b.dash.len()
        && a
            .dash
            .iter()
            .zip(b.dash.iter())
            .all(|(d1, d2)| (d1 - d2).abs() < TOL)
}

fn style_approx_eq(a: &Style, b: &Style) -> bool {
    const TOL: f32 = 1e-5;
    let fill_eq = match (&a.fill, &b.fill) {
        (None, None) => true,
        (Some(p1), Some(p2)) => paint_approx_eq(p1, p2),
        _ => false,
    };
    let stroke_eq = match (&a.stroke, &b.stroke) {
        (None, None) => true,
        (Some(s1), Some(s2)) => stroke_approx_eq(s1, s2),
        _ => false,
    };
    fill_eq
        && stroke_eq
        && a.fill_rule == b.fill_rule
        && a.blend == b.blend
        && (a.opacity - b.opacity).abs() < TOL
}

proptest! {
    #[test]
    fn v3_roundtrip_is_lossless(
        style in arb_style(),
        subpath in arb_subpath(),
    ) {
        // Roundtrip Style
        let json_style = serde_json::to_string(&style).unwrap();
        let de_style: Style = serde_json::from_str(&json_style).unwrap();
        prop_assert!(style_approx_eq(&style, &de_style));

        // Roundtrip Subpath
        let json_subpath = serde_json::to_string(&subpath).unwrap();
        let de_subpath: Subpath = serde_json::from_str(&json_subpath).unwrap();
        prop_assert!(subpath_approx_eq(&subpath, &de_subpath));
    }
}


