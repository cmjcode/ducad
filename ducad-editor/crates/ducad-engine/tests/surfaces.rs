//! Op `surface` (Paritas 6A) lewat `Session`: replay fixture + input hostil.

use ducad_engine::ops::{Op, OpFile};
use ducad_engine::Session;
use serde_json::json;

fn op(v: serde_json::Value) -> Op {
    serde_json::from_value(v).unwrap()
}

fn body(s: &Session, name: &str) -> ducad_engine::inspect::BodyReport {
    s.summary()
        .bodies
        .into_iter()
        .find(|b| b.name == name)
        .unwrap_or_else(|| panic!("body {name} tidak ada"))
}

#[test]
fn fixture_replays() {
    let f: OpFile = serde_json::from_str(include_str!("fixtures/surfaces.ops.json")).unwrap();
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    let r = s.run(f.ops, false);
    assert!(r.committed, "{:?}", r.error);
    assert!(body(&s, "cup").sheet, "revolve profil terbuka = permukaan");
    assert!(body(&s, "net").sheet && body(&s, "net").area > 80.0);
    assert!(!body(&s, "wall").sheet, "wall sudah ditebalkan");
}

#[test]
fn thicken_makes_solid_and_extend_grows() {
    let mut s = Session::new();
    let r = s.run(
        vec![
            op(json!({"op":"sketch","id":"k","plane":"XY","entities":[{"line":{"from":[0,0],"to":[10,0]}}]})),
            op(json!({"op":"surface","id":"w","make":{"extrude":{"sketch":"k","end":10}}})),
        ],
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    let a0 = body(&s, "w").area;
    assert!((a0 - 100.0).abs() < 1.0, "{a0}");
    assert!(body(&s, "w").sheet);
    let r = s.run(vec![op(json!({"op":"surface","id":"e","make":{"extend":{"body":"w","edges":">Z","distance":5}}}))], false);
    assert!(r.committed, "{:?}", r.error);
    assert!((body(&s, "w").area - 150.0).abs() < 1.0);
    let r = s.run(vec![op(json!({"op":"surface","id":"t","make":{"thicken":{"body":"w","thickness":2}}}))], false);
    assert!(r.committed, "{:?}", r.error);
    let w = body(&s, "w");
    assert!(!w.sheet && (w.volume - 300.0).abs() < 2.0, "{} {}", w.sheet, w.volume);
}

#[test]
fn stitch_consumes_inputs() {
    let mut s = Session::new();
    let r = s.run(
        vec![
            op(json!({"op":"sketch","id":"k1","plane":"XY","entities":[{"line":{"from":[0,0],"to":[10,0]}}]})),
            op(json!({"op":"sketch","id":"k2","plane":"XY","entities":[{"line":{"from":[10,0],"to":[10,10]}}]})),
            op(json!({"op":"surface","id":"a","make":{"extrude":{"sketch":"k1","end":10}}})),
            op(json!({"op":"surface","id":"b","make":{"extrude":{"sketch":"k2","end":10}}})),
            op(json!({"op":"surface","id":"ab","make":{"stitch":{"bodies":["a","b"]}}})),
        ],
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    let names: Vec<String> = s.summary().bodies.into_iter().map(|b| b.name).collect();
    assert_eq!(names, vec!["ab".to_string()]);
}

#[test]
fn hostile_surface_inputs_rejected() {
    let base = || {
        let mut s = Session::new();
        let r = s.run(
            vec![op(json!({"op":"sketch","id":"k","plane":"XY","entities":[{"line":{"from":[0,0],"to":[10,0]}}]}))],
            false,
        );
        assert!(r.committed);
        s
    };
    for c in [
        json!({"op":"surface","id":"x","make":{"extrude":{"sketch":"k","end":0}}}),
        json!({"op":"surface","id":"x","make":{"extrude":{"sketch":"missing","end":5}}}),
        json!({"op":"surface","id":"x","make":{"revolve":{"sketch":"k","axis":"axis:none","angle_deg":90}}}),
        json!({"op":"surface","id":"x","make":{"revolve":{"sketch":"k","axis":"v","angle_deg":0}}}),
        json!({"op":"surface","id":"x","make":{"loft":{"sections":["k"]}}}),
        json!({"op":"surface","id":"x","make":{"boundary":{"u":[[[0,0,0]]]}}}),
        json!({"op":"surface","id":"x","make":{"thicken":{"body":"nope","thickness":1}}}),
        json!({"op":"surface","id":"x","make":{"stitch":{"bodies":[]}}}),
        json!({"op":"surface","id":"x","make":{"sweep":{"sketch":"k","path":{"points":[[0,0,0]]}}}}),
    ] {
        let mut s = base();
        let r = s.run(vec![op(c.clone())], false);
        assert!(!r.committed, "seharusnya ditolak: {c}");
        assert!(r.error.is_some_and(|e| !e.message.is_empty()));
    }
}
