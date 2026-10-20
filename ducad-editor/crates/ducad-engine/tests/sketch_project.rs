//! `Op::SketchProject` (Paritas 1.14): proyeksi tepi/face body dan irisan
//! body ke sketch yang sudah ada, plus input hostil dan rollback.

use ducad_engine::ops::{Op, OpFile};
use ducad_engine::{OpErrorCode, Session};
use serde_json::json;

fn op(v: serde_json::Value) -> Op {
    serde_json::from_value(v).unwrap()
}

/// Sesi berisi balok 40×30×10 dan sketch kosong-ish di z = 20.
fn base() -> Session {
    let mut s = Session::new();
    let r = s.run(
        vec![
            op(json!({"op":"primitive","id":"block","shape":{"box":{"size":[40,30,10]}}})),
            op(json!({"op":"sketch","id":"sk","plane":{"base":"XY","offset":20},"entities":[
                {"circle":{"center":[20,15],"r":3,"name":"pin"}}]})),
        ],
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    s
}

fn entity_count(s: &Session, sketch: &str) -> usize {
    s.sketch(sketch).unwrap().0.entities.len()
}

#[test]
fn fixture_replays_and_volumes_match() {
    let f: OpFile =
        serde_json::from_str(include_str!("fixtures/sketch_project.ops.json")).unwrap();
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    let r = s.run(f.ops, false);
    assert!(r.committed, "{:?}", r.error);
    let vol = |name: &str| {
        s.summary()
            .bodies
            .iter()
            .find(|b| b.name == name)
            .map(|b| b.volume)
            .unwrap_or_else(|| panic!("body {name} tidak ada"))
    };
    // Tutup: 40×30×5 dikurangi lubang pin r=3.
    let cap = 40.0 * 30.0 * 5.0 - std::f64::consts::PI * 9.0 * 5.0;
    assert!((vol("cap") - cap).abs() < 1.0, "cap {}", vol("cap"));
    // Irisan di x = 20: 30×10, tebal 2.
    assert!((vol("slice") - 600.0).abs() < 1.0, "slice {}", vol("slice"));
}

#[test]
fn project_face_adds_named_entities() {
    let mut s = base();
    let r = s.run(
        vec![op(json!({"op":"sketch_project","id":"p","sketch":"sk","body":"block","faces":">Z"}))],
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    assert_eq!(r.outcomes[0].detail["entities"], 4);
    let (sk, _) = s.sketch("sk").unwrap();
    let mut names: Vec<&str> = sk.entity_names.values().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(names, ["p.1", "p.2", "p.3", "p.4", "pin"]);
}

#[test]
fn dry_run_and_failed_batch_restore_sketch() {
    let mut s = base();
    let before = entity_count(&s, "sk");
    let r = s.run(
        vec![op(json!({"op":"sketch_project","id":"p","sketch":"sk","body":"block","edges":"of(>Z)"}))],
        true,
    );
    assert!(!r.committed && r.error.is_none(), "{:?}", r.error);
    assert_eq!(entity_count(&s, "sk"), before, "dry_run harus memulihkan sketch");

    // Op kedua gagal → op proyeksi pertama ikut dibatalkan.
    let r = s.run(
        vec![
            op(json!({"op":"sketch_project","id":"p","sketch":"sk","body":"block","faces":">Z"})),
            op(json!({"op":"fillet","id":"f","body":"nope","edges":"|Z","radius":1})),
        ],
        false,
    );
    assert!(!r.committed);
    assert_eq!(entity_count(&s, "sk"), before);
}

#[test]
fn hostile_inputs_are_structured_errors() {
    let mut s = base();
    let cases = [
        (json!({"op":"sketch_project","id":"p","sketch":"sk","body":"block"}), OpErrorCode::InvalidParam),
        (json!({"op":"sketch_project","id":"p","sketch":"sk","body":"nope","faces":">Z"}), OpErrorCode::UnknownRef),
        (json!({"op":"sketch_project","id":"p","sketch":"nope","body":"block","faces":">Z"}), OpErrorCode::UnknownRef),
        (json!({"op":"sketch_project","id":"p","sketch":"sk","body":"block","faces":"all[kind=torus]"}), OpErrorCode::SelectorEmpty),
        (json!({"op":"sketch_project","id":"p","sketch":"sk","body":"block","edges":"(("}), OpErrorCode::SelectorSyntax),
        (json!({"op":"sketch_project","id":"p","sketch":"sk","body":"block","faces":""}), OpErrorCode::SelectorSyntax),
        (json!({"op":"sketch_project","id":"p","sketch":"sk","body":"block","mode":"intersect","faces":">Z"}), OpErrorCode::InvalidParam),
        // Bidang z = 20 tidak memotong balok setinggi 10.
        (json!({"op":"sketch_project","id":"p","sketch":"sk","body":"block","mode":"intersect"}), OpErrorCode::EmptyResult),
        // Tepi tegak lurus bidang runtuh jadi titik → tidak ada entitas.
        (json!({"op":"sketch_project","id":"p","sketch":"sk","body":"block","edges":"|Z"}), OpErrorCode::EmptyResult),
    ];
    for (raw, code) in cases {
        let r = s.run(vec![op(raw.clone())], false);
        assert!(!r.committed, "{raw}");
        let e = r.error.unwrap_or_else(|| panic!("{raw}: tanpa error"));
        assert_eq!(e.code, code, "{raw}: {}", e.message);
        assert!(e.hint.is_some(), "{raw}: tanpa hint ({})", e.message);
    }
    assert_eq!(entity_count(&s, "sk"), 1);
    // Mode tak dikenal ditolak serde, tidak sampai engine.
    assert!(serde_json::from_value::<Op>(
        json!({"op":"sketch_project","id":"p","sketch":"sk","body":"block","mode":"x"})
    )
    .is_err());
}

#[test]
fn duplicate_entity_name_is_rejected() {
    let mut s = Session::new();
    let r = s.run(
        vec![
            op(json!({"op":"primitive","id":"block","shape":{"box":{"size":[40,30,10]}}})),
            op(json!({"op":"sketch","id":"sk","plane":{"base":"XY","offset":20},"entities":[
                {"circle":{"center":[20,15],"r":3,"name":"p.1"}}]})),
            op(json!({"op":"sketch_project","id":"p","sketch":"sk","body":"block","faces":">Z"})),
        ],
        false,
    );
    assert_eq!(r.error.map(|e| e.code), Some(OpErrorCode::DuplicateId));
}
