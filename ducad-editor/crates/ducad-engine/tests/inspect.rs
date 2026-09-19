//! `inspect::summarize` (P2.2).

use ducad_engine::inspect::{round4, summarize, DEFAULT_TOPOLOGY_LIMIT};
use ducad_engine::ops::OpFile;
use ducad_engine::{OpErrorCode, Session};

fn plate() -> Session {
    let f: OpFile = serde_json::from_str(include_str!("fixtures/plate.ops.json")).unwrap();
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    assert!(s.run(f.ops, false).committed);
    s
}

#[test]
fn plate_summary() {
    let s = plate();
    let sum = summarize(&s, None, false, DEFAULT_TOPOLOGY_LIMIT).unwrap();
    assert_eq!(sum.unit, "mm");
    assert_eq!(sum.oplog_len, 4);
    assert_eq!(sum.bodies.len(), 1);
    let b = &sum.bodies[0];
    assert_eq!(b.name, "plate");
    assert!(b.valid);
    // 6 face datar + 4 fillet r=3 + 4 dinding lubang M5.
    assert_eq!(b.faces, 14);
    assert_eq!(b.face_kinds["cylinder"], 8);
    assert_eq!(b.face_kinds["plane"], 6);
    assert_eq!(b.size, [60.0, 40.0, 8.0]);
    assert_eq!(sum.sketches.len(), 1);
    assert_eq!(sum.sketches[0].plane, "XY");
    assert_eq!(sum.sketches[0].closed_regions, 1);
    assert!(sum.sketches[0].names.contains(&"outline.top".to_string()));
}

#[test]
fn topology_is_optional_and_numbers_are_rounded() {
    let s = plate();
    let json = serde_json::to_value(summarize(&s, None, false, 200).unwrap()).unwrap();
    assert!(json["bodies"][0].get("topology").is_none());
    let vol = json["bodies"][0]["volume"].as_f64().unwrap();
    assert_eq!(vol, round4(vol));
    let text = serde_json::to_string(&json).unwrap();
    for num in text.split(|c: char| !(c.is_ascii_digit() || c == '.')) {
        if let Some((_, frac)) = num.split_once('.') {
            assert!(frac.len() <= 4, "angka {num} belum dibulatkan 4 desimal");
        }
    }

    let with = summarize(&s, Some("plate"), true, 5).unwrap();
    let topo = with.bodies[0].topology.as_ref().unwrap();
    assert_eq!(topo.faces.len(), 5);
    assert!(topo.truncated);
    assert_eq!(
        summarize(&s, Some("nope"), false, 200).unwrap_err().code,
        OpErrorCode::UnknownRef
    );
    assert_eq!(round4(1.23456), 1.2346);
    assert_eq!(round4(-0.00001), 0.0);
}
