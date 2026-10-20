//! Fase 2 PLAN_PARITAS_SOLIDCRAFT: op direct editing & alat solid lewat
//! `Session` (jalur yang sama dengan CLI/MCP): replay fixture, volume, dan
//! input hostil (tidak boleh panic, batch di-rollback).

use ducad_engine::ops::{Op, OpFile};
use ducad_engine::{OpErrorCode, Session};
use serde_json::json;

fn op(v: serde_json::Value) -> Op {
    serde_json::from_value(v).unwrap()
}

fn vol(s: &Session, name: &str) -> f64 {
    s.summary()
        .bodies
        .iter()
        .find(|b| b.name == name)
        .map(|b| b.volume)
        .unwrap_or_else(|| panic!("body {name} tidak ada"))
}

fn bbox(s: &Session, name: &str) -> [[f64; 3]; 2] {
    s.summary()
        .bodies
        .iter()
        .find(|b| b.name == name)
        .map(|b| b.bbox)
        .unwrap_or_else(|| panic!("body {name} tidak ada"))
}

fn near(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= b.abs() * rel + 1e-6
}

fn block() -> Session {
    let mut s = Session::new();
    let r = s.run(
        vec![op(json!({"op":"primitive","id":"b","shape":{"box":{"size":[10,10,10]}}}))],
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    s
}

fn run_one(s: &mut Session, v: serde_json::Value) -> ducad_engine::BatchReport {
    s.run(vec![op(v)], false)
}

#[test]
fn fixture_replays_with_expected_geometry() {
    let f: OpFile = serde_json::from_str(include_str!("fixtures/direct_edit.ops.json")).unwrap();
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    let r = s.run(f.ops, false);
    assert!(r.committed, "{:?}", r.error);

    // 45×30×15 + 2 dinding web (30×2×4) − deboss lingkaran r3 sedalam 1.
    let expect = 45.0 * 30.0 * 15.0 + 2.0 * 30.0 * 2.0 * 4.0 - std::f64::consts::PI * 9.0;
    assert!(near(vol(&s, "block"), expect, 2e-3), "block {}", vol(&s, "block"));
    let torus = 2.0 * std::f64::consts::PI.powi(2) * 10.0 * 4.0;
    assert!(near(vol(&s, "ring"), torus, 2e-3), "ring {}", vol(&s, "ring"));
    assert!(near(vol(&s, "ring_box"), 26.0 * 26.0 * 6.0, 1e-3), "box {}", vol(&s, "ring_box"));
    // Tabung r2/r1.5 sepanjang ±70 mm (sudut tajam memakan sedikit).
    let tube = std::f64::consts::PI * (4.0 - 2.25) * 70.0;
    assert!(near(vol(&s, "rail"), tube, 0.08), "rail {} vs {tube}", vol(&s, "rail"));
    for k in 1..=4 {
        let name = format!("studs.{k}");
        let bb = bbox(&s, &name);
        assert!(near((bb[0][0] + bb[1][0]) * 0.5, 10.0 * k as f64, 1e-3), "{name} {bb:?}");
    }
    // Cap diputar 180° lalu duduk di bawah bidang z = 0.
    let cap = bbox(&s, "cap");
    assert!(cap[1][2].abs() < 1e-3 && near(cap[0][2], -4.0, 1e-3), "cap {cap:?}");
    assert!(near(vol(&s, "overlap"), 500.0, 1e-3), "overlap {}", vol(&s, "overlap"));
}

#[test]
fn offset_and_delete_face() {
    let mut s = block();
    let r = run_one(&mut s, json!({"op":"offset_face","id":"o","body":"b","faces":">Z","distance":5}));
    assert!(r.committed, "{:?}", r.error);
    assert!(near(vol(&s, "b"), 1500.0, 1e-3));
    let r = run_one(&mut s, json!({"op":"fillet","id":"f","body":"b","edges":"|Z","radius":1}));
    assert!(r.committed, "{:?}", r.error);
    let r = run_one(&mut s, json!({"op":"delete_face","id":"d","body":"b","faces":"all[kind=cylinder]"}));
    assert!(r.committed, "{:?}", r.error);
    assert!(near(vol(&s, "b"), 1500.0, 1e-3), "{}", vol(&s, "b"));
}

#[test]
fn replace_face_with_face_of_other_body() {
    let mut s = block();
    let r = s.run(
        vec![
            op(json!({"op":"primitive","id":"wall","shape":{"box":{"size":[2,2,2]}},"at":[30,0,20]})),
            op(json!({"op":"replace_face","id":"rf","body":"b","face":">Z","with":{"body":"wall","face":">Z"}})),
        ],
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    assert!(near(vol(&s, "b"), 2200.0, 1e-3), "{}", vol(&s, "b"));
}

#[test]
fn split_face_by_sketch_then_offset_piece() {
    let mut s = block();
    let r = s.run(
        vec![
            op(json!({"op":"sketch","id":"cut","plane":{"base":"XY","offset":10},"entities":[
                {"line":{"from":[5,-1],"to":[5,11]}}]})),
            op(json!({"op":"split_face","id":"sp","body":"b","faces":">Z","tool":{"sketch":"cut"}})),
        ],
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    let faces = s.summary().bodies[0].faces;
    assert_eq!(faces, 7, "top terbelah dua");
    let r = run_one(&mut s, json!({"op":"offset_face","id":"o","body":"b","faces":">Z[x>5]","distance":2}));
    assert!(r.committed, "{:?}", r.error);
    assert!(near(vol(&s, "b"), 1100.0, 2e-3), "{}", vol(&s, "b"));
}

#[test]
fn pipe_solid_and_pattern_align() {
    let mut s = Session::new();
    let r = s.run(
        vec![
            op(json!({"op":"pipe","id":"p","path":{"points":[[0,0,0],[0,0,20]]},"r":3})),
            op(json!({"op":"primitive","id":"k","shape":{"box":{"size":[1,1,1]}}})),
            op(json!({"op":"pattern","id":"pp","body":"k","kind":{"path":{"path":{"points":[[0,0,0],[10,0,0],[10,10,0]]},"count":3,"align":true}}})),
        ],
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    assert!(near(vol(&s, "p"), std::f64::consts::PI * 9.0 * 20.0, 2e-3));
    // Instance terakhir: sudut minimum asal pindah ke ujung jalur (10,10,0)
    // lalu diputar 90° mengelilingi Z mengikuti tangen (+X → +Y).
    let bb = bbox(&s, "pp.2");
    let want = [[9.0, 10.0, 0.0], [10.0, 11.0, 1.0]];
    for (a, b) in bb.iter().flatten().zip(want.iter().flatten()) {
        assert!((a - b).abs() < 1e-3, "{bb:?}");
    }
}

#[test]
fn hostile_inputs_are_rejected_without_panic() {
    let cases = [
        json!({"op":"offset_face","id":"x","body":"b","faces":">Z","distance":0}),
        json!({"op":"offset_face","id":"x","body":"b","faces":">Z","distance":1e300}),
        json!({"op":"offset_face","id":"x","body":"b","faces":"all[kind=cylinder]","distance":1}),
        json!({"op":"offset_face","id":"x","body":"nope","faces":">Z","distance":1}),
        json!({"op":"delete_face","id":"x","body":"b","faces":"all"}),
        json!({"op":"replace_face","id":"x","body":"b","face":"all","with":"XY"}),
        json!({"op":"replace_face","id":"x","body":"b","face":">Z","with":"YZ"}),
        json!({"op":"split_face","id":"x","body":"b","tool":{"plane":{"base":"XY","offset":100}}}),
        json!({"op":"split_face","id":"x","body":"b","tool":{"sketch":"missing"}}),
        json!({"op":"pipe","id":"x","path":{"points":[[0,0,0],[0,0,10]]},"r":2,"wall":3}),
        json!({"op":"pipe","id":"x","path":{"points":[[0,0,0]]},"r":2}),
        json!({"op":"web","id":"x","body":"b","sketch":"missing","thickness":1,"depth":1}),
        json!({"op":"emboss","id":"x","sketch":"missing","body":"b","depth":1}),
        json!({"op":"align","id":"x","body":"b","from":">Z","to":{"body":"b","face":"<Z"}}),
        json!({"op":"boundary_fill","id":"x","bodies":[],"points":[[0,0,0]]}),
        json!({"op":"boundary_fill","id":"x","bodies":["b"],"points":[[500,500,500]]}),
        json!({"op":"bounding_solid","id":"x","body":"b","margin":-1}),
        json!({"op":"bounding_solid","id":"x","body":"b","shape":"cylinder","axis":[0,0,0]}),
        json!({"op":"primitive","id":"x","shape":{"torus":{"major":1,"minor":2}}}),
        json!({"op":"pattern","id":"x","body":"b","kind":{"path":{"path":{"points":[[0,0,0],[1,0,0]]},"count":1}}}),
    ];
    for c in cases {
        let mut s = block();
        let before = vol(&s, "b");
        let r = s.run(vec![op(c.clone())], false);
        assert!(!r.committed, "seharusnya ditolak: {c}");
        let e = r.error.unwrap_or_else(|| panic!("tanpa error: {c}"));
        assert!(!e.message.is_empty());
        assert!(near(vol(&s, "b"), before, 1e-9), "rollback gagal: {c}");
    }
}

#[test]
fn unknown_body_maps_to_unknown_ref() {
    let mut s = block();
    let r = run_one(&mut s, json!({"op":"delete_face","id":"x","body":"ghost","faces":">Z"}));
    assert_eq!(r.error.map(|e| e.code), Some(OpErrorCode::UnknownRef));
}
