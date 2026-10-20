//! Tes datum (PLAN_PARITAS_SOLIDCRAFT Fase 1 butir 1.2–1.4, 1.6):
//! `construct_axis`, `construct_point`, varian `PlaneSpec` konstruksi,
//! revolve/pattern memakai `"axis:<id>"`, plus input hostil.

use ducad_engine::ops::{Op, OpFile, PlaneSpec};
use ducad_engine::{OpErrorCode, Session};
use serde_json::json;

const FIXTURE: &str = include_str!("fixtures/datum_axis.ops.json");

fn ops(v: serde_json::Value) -> Vec<Op> {
    serde_json::from_value(v).unwrap()
}

/// Sesi dengan kotak 80×80×10 (x, y ∈ ±40, z ∈ 0..10) bernama `b`.
fn boxed() -> Session {
    let mut s = Session::new();
    let r = s.run(
        ops(json!([
            {"op":"primitive","id":"b","shape":{"box":{"size":[80,80,10],"centered":true}},"at":[0,0,5]}
        ])),
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    s
}

fn run_ok(s: &mut Session, v: serde_json::Value) -> ducad_engine::BatchReport {
    let r = s.run(ops(v), false);
    assert!(r.committed, "{:?}", r.error);
    r
}

fn run_err(s: &mut Session, v: serde_json::Value) -> ducad_engine::OpError {
    let r = s.run(ops(v), false);
    assert!(!r.committed, "seharusnya gagal: {:?}", r.outcomes);
    let e = r.error.unwrap();
    assert_ne!(e.code, OpErrorCode::Internal, "{e:?}");
    e
}

fn close(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < 1e-6)
}

fn parallel(a: [f64; 3], b: [f64; 3]) -> bool {
    let c = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    c.iter().all(|x| x.abs() < 1e-6)
}

#[test]
fn fixture_builds_and_passes_checks() {
    let f: OpFile = serde_json::from_str(FIXTURE).unwrap();
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    s.set_checks(f.checks);
    let r = s.run(f.ops, false);
    assert!(r.committed, "{:?}", r.error);
    let checks = s.run_checks(None);
    assert_eq!(checks.fail + checks.error, 0, "{:?}", checks.results);
    let m = s.meta();
    assert!(close(m.datum_points["hole_c"], [30.0, 0.0, 10.0]), "{:?}", m.datum_points);
    assert!(close(m.datum_points["origin_pt"], [0.0, 0.0, 10.0]));
    assert!(parallel(m.datum_axes["hole_ax"].dir, [1.0, 0.0, 0.0]));
    // Ringkasan memuat datum.
    let summary = serde_json::to_value(s.summary()).unwrap();
    assert!(summary["datum_axes"]["center_ax"].is_object());
    assert!(summary["datum_points"]["hole_c"].is_array());
}

#[test]
fn fixture_replays_from_oplog() {
    let f: OpFile = serde_json::from_str(FIXTURE).unwrap();
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    assert!(s.run(f.ops, false).committed);
    let replayed = Session::replay(s.design().clone()).unwrap();
    assert_eq!(replayed.meta().datum_axes, s.meta().datum_axes);
    assert_eq!(replayed.meta().datum_points, s.meta().datum_points);
}

#[test]
fn construct_axis_variants() {
    let mut s = boxed();
    run_ok(
        &mut s,
        json!([
            {"op":"construct_axis","id":"a1","by":{"two_points":{"a":[0,0,0],"b":[0,0,"5*2"]}}},
            {"op":"construct_axis","id":"a2","by":{"edge":{"body":"b","edge":"|Z[x=40][y=40]"}}},
            {"op":"construct_axis","id":"a3","by":{"normal_to_face":{"body":"b","face":">Z","through":[10,20,99]}}},
            {"op":"construct_axis","id":"a4","by":{"two_planes":{"a":{"base":"XY","offset":10},"b":"XZ"}}},
            {"op":"primitive","id":"c","shape":{"cylinder":{"r":5,"h":10}},"at":[100,0,0]},
            {"op":"construct_axis","id":"a5","by":{"through_cylinder":{"body":"c","face":"all[kind=cylinder]"}}},
            {"op":"construct_axis","id":"a6","by":{"edge":{"body":"c","edge":"all[kind=circle][z=10]"}}}
        ]),
    );
    let ax = &s.meta().datum_axes;
    assert!(parallel(ax["a1"].dir, [0.0, 0.0, 1.0]));
    assert!(parallel(ax["a2"].dir, [0.0, 0.0, 1.0]));
    assert!((ax["a2"].origin[0] - 40.0).abs() < 1e-6 && (ax["a2"].origin[1] - 40.0).abs() < 1e-6);
    assert!(close(ax["a3"].origin, [10.0, 20.0, 10.0]));
    assert!(parallel(ax["a4"].dir, [1.0, 0.0, 0.0]));
    assert!((ax["a4"].origin[2] - 10.0).abs() < 1e-6 && ax["a4"].origin[1].abs() < 1e-6);
    for id in ["a5", "a6"] {
        assert!(parallel(ax[id].dir, [0.0, 0.0, 1.0]), "{id}: {:?}", ax[id]);
        assert!((ax[id].origin[0] - 100.0).abs() < 1e-6 && ax[id].origin[1].abs() < 1e-6, "{id}");
    }
}

#[test]
fn construct_point_variants() {
    let mut s = boxed();
    run_ok(
        &mut s,
        json!([
            {"op":"construct_point","id":"v1","by":{"vertex":{"body":"b","near":[41,39,11]}}},
            {"op":"construct_point","id":"v2","by":{"vertex":{"body":"b","edge":"|Z[x=40][y=40]","end":"end"}}},
            {"op":"construct_point","id":"p3","by":{"three_planes":{"a":{"base":"XY","offset":3},"b":"XZ","c":{"base":"YZ","offset":7}}}},
            {"op":"construct_point","id":"p4","by":{"edge_plane":{"body":"b","edge":"|Z[x=40][y=40]","plane":{"base":"XY","offset":25}}}},
            {"op":"construct_point","id":"p5","by":{"along_path":{"body":"b","edge":"|Z[x=40][y=40]","fraction":0.5}}},
            {"op":"construct_axis","id":"ax","by":{"two_points":{"a":[0,-50,3],"b":[0,50,3]}}},
            {"op":"construct_point","id":"p6","by":{"two_edges":{"a":"axis:ax","b":{"body":"b","edge":">Z and |X and >Y"}}}},
            {"op":"primitive","id":"sp","shape":{"sphere":{"r":4}},"at":[200,10,5]},
            {"op":"construct_point","id":"p7","by":{"center":{"body":"sp","face":"all[kind=sphere]"}}},
            {"op":"primitive","id":"c","shape":{"cylinder":{"r":5,"h":10}},"at":[100,0,0]},
            {"op":"construct_point","id":"p8","by":{"center":{"body":"c","edge":"all[kind=circle][z=10]"}}},
            {"op":"construct_point","id":"p9","by":{"along_path":{"body":"c","edge":"all[kind=circle][z=0]","fraction":1}}}
        ]),
    );
    let p = &s.meta().datum_points;
    assert!(close(p["v1"], [40.0, 40.0, 10.0]), "{:?}", p["v1"]);
    assert!((p["v2"][0] - 40.0).abs() < 1e-6 && (p["v2"][1] - 40.0).abs() < 1e-6);
    assert!(close(p["p3"], [7.0, 0.0, 3.0]));
    // Garis tepi diperpanjang menembus z = 25.
    assert!(close(p["p4"], [40.0, 40.0, 25.0]));
    assert!(close(p["p5"], [40.0, 40.0, 5.0]));
    // Sumbu y di z=3 vs tepi atas x di (y=40, z=10): titik tengah ruas terdekat.
    assert!(close(p["p6"], [0.0, 40.0, 6.5]), "{:?}", p["p6"]);
    assert!(close(p["p7"], [200.0, 10.0, 5.0]), "{:?}", p["p7"]);
    assert!(close(p["p8"], [100.0, 0.0, 10.0]), "{:?}", p["p8"]);
    // Lingkaran penuh: fraction 1 kembali ke titik awal, di radius 5.
    let d = ((p["p9"][0] - 100.0).powi(2) + p["p9"][1].powi(2)).sqrt();
    assert!((d - 5.0).abs() < 1e-6 && p["p9"][2].abs() < 1e-6, "{:?}", p["p9"]);
}

#[test]
fn construction_planes_place_sketches() {
    let mut s = boxed();
    run_ok(
        &mut s,
        json!([
            {"op":"primitive","id":"c","shape":{"cylinder":{"r":5,"h":10}},"at":[100,0,0]},
            {"op":"construct_axis","id":"zx","by":{"two_points":{"a":[0,0,0],"b":[0,0,1]}}},
            {"op":"sketch","id":"s_mid","plane":{"midplane":[{"body":"b","face":">Z"},{"body":"b","face":"<Z"}]},"entities":[]},
            {"op":"sketch","id":"s_perp","plane":{"perpendicular_to":"axis:zx","through":[0,0,4]},"entities":[]},
            {"op":"sketch","id":"s_perp_edge","plane":{"perpendicular_to":{"body":"b","edge":"|Z[x=40][y=40]"},"through":[1,2,3]},"entities":[]},
            {"op":"sketch","id":"s_tan","plane":{"tangent_to":{"body":"c","face":"all[kind=cylinder]"},"angle_deg":90},"entities":[]},
            {"op":"sketch","id":"s_tan_pt","plane":{"tangent_to":{"body":"c","face":"all[kind=cylinder]"},"through":[90,0,3]},"entities":[]},
            {"op":"sketch","id":"s_along","plane":{"along":{"body":"b","edge":"|Z[x=40][y=40]"},"fraction":0.25},"entities":[]},
            {"op":"sketch","id":"s_two","plane":{"through_edges":[{"body":"b","edge":"|Z[x=40][y=40]"},{"body":"b","edge":"|Z[x=-40][y=40]"}]},"entities":[]},
            {"op":"sketch","id":"s_one","plane":{"through_edges":["axis:zx"],"through":[5,0,0]},"entities":[]}
        ]),
    );
    let frame = |id: &str| *s.sketch(id).unwrap().1;
    let f = frame("s_mid");
    assert!((f.origin[2] - 5.0).abs() < 1e-6 && parallel(f.normal, [0.0, 0.0, 1.0]));
    let f = frame("s_perp");
    assert!(close(f.origin, [0.0, 0.0, 4.0]) && parallel(f.normal, [0.0, 0.0, 1.0]));
    let f = frame("s_perp_edge");
    assert!(close(f.origin, [1.0, 2.0, 3.0]) && parallel(f.normal, [0.0, 0.0, 1.0]));
    let f = frame("s_tan");
    assert!((f.origin[0] - 100.0).abs() < 1e-6 && (f.origin[1] - 5.0).abs() < 1e-6, "{f:?}");
    assert!(close(f.normal, [0.0, 1.0, 0.0]), "{f:?}");
    let f = frame("s_tan_pt");
    assert!((f.origin[0] - 95.0).abs() < 1e-6 && close(f.normal, [-1.0, 0.0, 0.0]), "{f:?}");
    let f = frame("s_along");
    assert!(close(f.origin, [40.0, 40.0, 2.5]) && parallel(f.normal, [0.0, 0.0, 1.0]), "{f:?}");
    let f = frame("s_two");
    assert!(parallel(f.normal, [0.0, 1.0, 0.0]) && (f.origin[1] - 40.0).abs() < 1e-6, "{f:?}");
    let f = frame("s_one");
    assert!(parallel(f.normal, [0.0, 1.0, 0.0]), "{f:?}");
}

#[test]
fn revolve_and_pattern_accept_datum_axis() {
    let mut s = Session::new();
    run_ok(
        &mut s,
        json!([
            {"op":"construct_axis","id":"ax","by":{"two_points":{"a":[50,0,0],"b":[50,0,10]}}},
            {"op":"sketch","id":"prof","plane":"XZ","entities":[{"rect":{"corner":[55,0],"w":5,"h":10}}]},
            {"op":"revolve","id":"ring","sketch":"prof","axis":"axis:ax"},
            {"op":"primitive","id":"pin","shape":{"cylinder":{"r":1,"h":5}},"at":[58,0,0]},
            {"op":"pattern","id":"pins","body":"pin","kind":{"circular":{"axis":"axis:ax","count":4,"angle_deg":360}}}
        ]),
    );
    let (_, ring) = s.body("ring").unwrap();
    let bb = ring.shape.bounding_box().unwrap();
    // Cincin r 5..10 di sekitar x = 50.
    assert!((bb.0[0] - 40.0).abs() < 0.1 && (bb.1[0] - 60.0).abs() < 0.1, "{bb:?}");
    let (_, p1) = s.body("pins.1").unwrap();
    let bb = p1.shape.bounding_box().unwrap();
    // Salinan pertama diputar 90° di sekitar sumbu x=50 → pusat (50, 8).
    assert!(((bb.0[0] + bb.1[0]) / 2.0 - 50.0).abs() < 0.1, "{bb:?}");
    assert!(((bb.0[1] + bb.1[1]) / 2.0 - 8.0).abs() < 0.1, "{bb:?}");
}

#[test]
fn revolve_datum_axis_off_plane_has_hint() {
    let mut s = Session::new();
    run_ok(
        &mut s,
        json!([
            {"op":"construct_axis","id":"tilted","by":{"two_points":{"a":[0,0,0],"b":[0,1,1]}}},
            {"op":"construct_axis","id":"shifted","by":{"two_points":{"a":[0,5,0],"b":[0,5,1]}}},
            {"op":"sketch","id":"prof","plane":"XZ","entities":[{"rect":{"corner":[5,0],"w":5,"h":10}}]}
        ]),
    );
    for ax in ["axis:tilted", "axis:shifted"] {
        let e = run_err(&mut s, json!([{"op":"revolve","id":"r","sketch":"prof","axis":ax}]));
        assert_eq!(e.code, OpErrorCode::InvalidParam, "{e:?}");
        assert!(e.hint.as_deref().unwrap_or_default().contains("sketch"), "{e:?}");
    }
    let e = run_err(&mut s, json!([{"op":"revolve","id":"r","sketch":"prof","axis":"axis:nope"}]));
    assert_eq!(e.code, OpErrorCode::UnknownRef);
    assert!(e.hint.is_some());
    let e = run_err(
        &mut s,
        json!([{"op":"primitive","id":"pin","shape":{"cylinder":{"r":1,"h":5}}},
               {"op":"pattern","id":"p","body":"pin","kind":{"circular":{"pivot":[1,0,0],"axis":"axis:shifted","count":3,"angle_deg":360}}}]),
    );
    assert!(e.message.contains("pivot"), "{e:?}");
}

#[test]
fn hostile_datum_inputs_fail_cleanly() {
    let cases = [
        // Selector kosong / tidak cocok / sintaks rusak.
        json!({"op":"construct_axis","id":"x","by":{"edge":{"body":"b","edge":""}}}),
        json!({"op":"construct_axis","id":"x","by":{"edge":{"body":"b","edge":"all[kind=circle]"}}}),
        json!({"op":"construct_axis","id":"x","by":{"edge":{"body":"b","edge":"(("}}}),
        json!({"op":"construct_axis","id":"x","by":{"edge":{"body":"b","edge":"axis:ax"}}}),
        json!({"op":"construct_axis","id":"x","by":{"edge":{"body":"nope","edge":"|Z"}}}),
        // Degenerat.
        json!({"op":"construct_axis","id":"x","by":{"two_points":{"a":[1,1,1],"b":[1,1,1]}}}),
        json!({"op":"construct_axis","id":"x","by":{"two_points":{"a":"point:nope","b":[1,1,1]}}}),
        json!({"op":"construct_axis","id":"x","by":{"two_points":{"a":"axis:ax","b":[1,1,1]}}}),
        json!({"op":"construct_axis","id":"x","by":{"two_points":{"a":"garbage","b":[1,1,1]}}}),
        json!({"op":"construct_axis","id":"x","by":{"two_points":{"a":["$nope",0,0],"b":[1,1,1]}}}),
        json!({"op":"construct_axis","id":"x","by":{"two_planes":{"a":"XY","b":{"base":"XY","offset":5}}}}),
        json!({"op":"construct_axis","id":"x","by":{"normal_to_face":{"body":"b","face":"#Z"}}}),
        json!({"op":"construct_axis","id":"x","by":{"through_cylinder":{"body":"b","face":">Z"}}}),
        json!({"op":"construct_point","id":"x","by":{"three_planes":{"a":"XY","b":"XY","c":"YZ"}}}),
        json!({"op":"construct_point","id":"x","by":{"vertex":{"body":"b"}}}),
        json!({"op":"construct_point","id":"x","by":{"vertex":{"body":"b","edge":"|Z","near":[0,0,0]}}}),
        json!({"op":"construct_point","id":"x","by":{"center":{"body":"b","edge":"|Z[x=40][y=40]"}}}),
        json!({"op":"construct_point","id":"x","by":{"center":{"body":"b","face":">Z"}}}),
        json!({"op":"construct_point","id":"x","by":{"along_path":{"body":"b","edge":"|Z[x=40][y=40]","fraction":2}}}),
        json!({"op":"construct_point","id":"x","by":{"along_path":{"body":"b","edge":"|Z[x=40][y=40]","fraction":"$nan"}}}),
        json!({"op":"construct_point","id":"x","by":{"edge_plane":{"body":"b","edge":"|Z[x=40][y=40]","plane":{"base":"XY","offset":"0/0"}}}}),
        json!({"op":"construct_point","id":"x","by":{"edge_plane":{"body":"b","edge":">Z and |X and >Y","plane":"XY"}}}),
        json!({"op":"construct_point","id":"x","by":{"two_edges":{"a":"axis:ax","b":"axis:ax"}}}),
        // Bidang konstruksi.
        json!({"op":"sketch","id":"x","plane":{"midplane":[]},"entities":[]}),
        json!({"op":"sketch","id":"x","plane":{"midplane":["XY","XZ"]},"entities":[]}),
        json!({"op":"sketch","id":"x","plane":{"perpendicular_to":"axis:","through":[0,0,0]},"entities":[]}),
        json!({"op":"sketch","id":"x","plane":{"tangent_to":{"body":"b","face":">Z"}},"entities":[]}),
        json!({"op":"sketch","id":"x","plane":{"along":{"body":"b","edge":"|Z[x=40][y=40]"},"fraction":-1},"entities":[]}),
        json!({"op":"sketch","id":"x","plane":{"through_edges":[]},"entities":[]}),
        json!({"op":"sketch","id":"x","plane":{"through_edges":["axis:ax","axis:ax"]},"entities":[]}),
        json!({"op":"sketch","id":"x","plane":{"through_edges":["axis:ax"],"through":[0,3,0]},"entities":[]}),
    ];
    for case in cases {
        let mut s = boxed();
        run_ok(
            &mut s,
            json!([{"op":"construct_axis","id":"ax","by":{"two_points":{"a":[0,0,0],"b":[0,1,0]}}}]),
        );
        let op: Op = serde_json::from_value(case.clone()).unwrap_or_else(|e| panic!("{case}: {e}"));
        let r = s.run(vec![op], false);
        assert!(!r.committed, "{case} seharusnya gagal");
        let e = r.error.unwrap();
        assert_ne!(e.code, OpErrorCode::Internal, "{case}: {e:?}");
        if case["op"] != "sketch" {
            assert!(e.hint.is_some(), "{case}: tanpa hint: {e:?}");
        }
    }
}

#[test]
fn plane_spec_json_is_unambiguous_and_round_trips() {
    let forms = [
        json!("XY"),
        json!({"base":"XZ","offset":"$t"}),
        json!({"body":"b","face":">Z"}),
        json!({"midplane":[{"body":"b","face":">Z"},"XY"]}),
        json!({"perpendicular_to":"axis:a","through":"point:p"}),
        json!({"perpendicular_to":{"body":"b","edge":"|Z"},"through":[1,2,3]}),
        json!({"tangent_to":{"body":"b","face":"all[kind=cylinder]"},"angle_deg":45}),
        json!({"tangent_to":{"body":"b","face":"all[kind=cylinder]"}}),
        json!({"along":{"body":"b","edge":"|Z"},"fraction":0.5}),
        json!({"through_edges":["axis:a",{"body":"b","edge":"|X"}]}),
        json!({"through_edges":["axis:a"],"through":[0,0,1]}),
    ];
    let kinds = [
        "Named", "Offset", "OnFace", "Midplane", "Perpendicular", "Perpendicular", "Tangent",
        "Tangent", "AlongPath", "TwoEdges", "TwoEdges",
    ];
    for (v, kind) in forms.iter().zip(kinds) {
        let p: PlaneSpec = serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{v}: {e}"));
        assert!(format!("{p:?}").starts_with(kind), "{v} → {p:?}");
        let back: PlaneSpec = serde_json::from_value(serde_json::to_value(&p).unwrap()).unwrap();
        assert_eq!(back, p, "round trip {v}");
    }
    // Campuran field antar varian ditolak, bukan diam-diam dipetakan.
    for bad in [
        json!({"base":"XY","offset":5,"face":">Z"}),
        json!({"body":"b","face":">Z","fraction":1}),
        json!({"along":{"body":"b","edge":"|Z"}}),
        json!({"perpendicular_to":"axis:a"}),
    ] {
        assert!(serde_json::from_value::<PlaneSpec>(bad.clone()).is_err(), "{bad}");
    }
}

#[test]
fn existing_fixtures_parse_identically() {
    for text in [
        include_str!("fixtures/flange.ops.json"),
        include_str!("fixtures/loft_helix.ops.json"),
        include_str!("fixtures/enclosure.ops.json"),
    ] {
        let f: OpFile = serde_json::from_str(text).unwrap();
        let again: OpFile = serde_json::from_value(serde_json::to_value(&f).unwrap()).unwrap();
        assert_eq!(f, again);
    }
    // Pola melingkar lama: `pivot` + vektor tetap varian `Dir`.
    let f: OpFile = serde_json::from_str(include_str!("fixtures/flange.ops.json")).unwrap();
    let pat = serde_json::to_value(&f.ops[3]).unwrap();
    assert_eq!(pat["kind"]["circular"]["axis"], json!([0.0, 0.0, 1.0]));
    assert_eq!(pat["kind"]["circular"]["pivot"], json!([0.0, 0.0, 0.0]));
}
