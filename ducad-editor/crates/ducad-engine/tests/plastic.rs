//! Op `plastic` (Paritas 6B) lewat `Session`.

use ducad_engine::ops::{Op, OpFile};
use ducad_engine::Session;
use serde_json::json;

fn op(v: serde_json::Value) -> Op {
    serde_json::from_value(v).unwrap()
}

#[test]
fn case_fixture_replays_and_reports_strain() {
    let f: OpFile = serde_json::from_str(include_str!("fixtures/plastic_case.ops.json")).unwrap();
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    let r = s.run(f.ops, false);
    assert!(r.committed, "{:?}", r.error);
    let clip = r.outcomes.iter().find(|o| o.op_id == "clip").unwrap();
    assert!(clip.detail["strain_pct"].as_f64().unwrap() > 0.0);
    let posts = r.outcomes.iter().find(|o| o.op_id == "posts").unwrap();
    // Default rule: draft 1°, outer = 2.5 + 4 × 1.0.
    assert_eq!(posts.detail["draft_deg"].as_f64(), Some(1.0));
    assert_eq!(posts.detail["outer_d"].as_f64(), Some(6.5));
}

#[test]
fn rule_overrides_change_defaults() {
    let mut s = Session::new();
    let r = s.run(vec![op(json!({"op":"primitive","id":"p","shape":{"box":{"size":[40,40,3]}}}))], false);
    assert!(r.committed);
    s.set_checks(
        serde_json::from_value(json!([{"check":"min_draft_angle","body":"p","pull":"z","min_deg":2}])).unwrap(),
    );
    let r = s.run(
        vec![op(json!({"op":"plastic","id":"b","body":"p","make":{"boss":{"face":">Z","at":[[0,0]],"height":5}}}))],
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    assert_eq!(r.outcomes[0].detail["draft_deg"].as_f64(), Some(2.0));
}

#[test]
fn hostile_plastic_inputs_rejected() {
    for c in [
        json!({"op":"plastic","id":"x","body":"p","make":{"boss":{"face":">Z","height":5}}}),
        json!({"op":"plastic","id":"x","body":"p","make":{"boss":{"face":">Z","at":[[0,0]],"height":5,"hole_d":50,"outer_d":10}}}),
        json!({"op":"plastic","id":"x","body":"p","make":{"lip":{"face":">Z","height":0}}}),
        json!({"op":"plastic","id":"x","body":"p","make":{"snap_fit":{"face":">Z","at":[[0,0],[1,1]],"length":10,"width":5,"hook":1}}}),
        json!({"op":"plastic","id":"x","body":"p","make":{"rest":{"at":[0,0,0],"diameter":5,"direction":[0,0,0]}}}),
        json!({"op":"plastic","id":"x","body":"ghost","make":{"rest":{"at":[0,0,0],"diameter":5}}}),
    ] {
        let mut s = Session::new();
        assert!(s.run(vec![op(json!({"op":"primitive","id":"p","shape":{"box":{"size":[40,40,3]}}}))], false).committed);
        let r = s.run(vec![op(c.clone())], false);
        assert!(!r.committed, "seharusnya ditolak: {c}");
    }
}
