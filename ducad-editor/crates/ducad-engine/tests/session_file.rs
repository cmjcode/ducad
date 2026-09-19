//! `Session::save` / `Session::from_file` (P1.6).

use std::path::PathBuf;

use ducad_engine::ops::{Op, OpFile};
use ducad_engine::Session;

fn temp(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("ducad-engine-{tag}-{}.ducad", std::process::id()))
}

fn plate_session() -> Session {
    let f: OpFile = serde_json::from_str(include_str!("fixtures/plate.ops.json")).unwrap();
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    let r = s.run(f.ops, false);
    assert!(r.committed, "{:?}", r.error);
    s
}

#[test]
fn save_then_load_replays_same_geometry_and_keeps_uuid() {
    let mut s = plate_session();
    let path = temp("roundtrip");
    s.save(&path).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(json["format_version"], 2);
    assert_eq!(json["design"]["oplog"].as_array().unwrap().len(), 4);
    let uuid = json["bodies"][0]["uuid"].as_str().unwrap().to_string();

    let mut loaded = Session::from_file(&path).unwrap();
    let v0 = s.body("plate").unwrap().1.shape.volume().abs();
    let v1 = loaded.body("plate").unwrap().1.shape.volume().abs();
    assert!((v0 - v1).abs() / v0 < 1e-6, "{v0} vs {v1}");
    assert_eq!(loaded.design().oplog.len(), 4);
    assert_eq!(
        loaded.model().doc.bodies.values().next().unwrap().uuid,
        uuid
    );

    loaded.save(&path).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(json["bodies"][0]["uuid"], uuid.as_str());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn file_without_design_is_adopted_with_unique_names() {
    // Berkas gaya GUI: dua body bernama sama, tanpa `design`.
    let mut s = Session::new();
    let ops: Vec<Op> = serde_json::from_str(
        r#"[{"op":"primitive","id":"a","shape":{"box":{"size":[10,10,10]}}},
            {"op":"primitive","id":"b","shape":{"box":{"size":[5,5,5]}},"at":[20,0,0]}]"#,
    )
    .unwrap();
    assert!(s.run(ops, false).committed);
    let path = temp("adopt");
    s.save(&path).unwrap();
    let mut json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    json.as_object_mut().unwrap().remove("design");
    json["format_version"] = 1.into();
    for b in json["bodies"].as_array_mut().unwrap() {
        b["name"] = "Solid".into();
    }
    std::fs::write(&path, json.to_string()).unwrap();

    let adopted = Session::from_file(&path).unwrap();
    assert_eq!(
        adopted.summary().bodies,
        vec!["Solid".to_string(), "Solid#2".to_string()]
    );
    assert!(adopted.design().oplog.is_empty());
    assert_eq!(adopted.design().base_bodies.len(), 2);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn edited_file_is_adopted_with_oplog_stale_warning() {
    let mut s = plate_session();
    let path = temp("stale");
    s.save(&path).unwrap();

    // STEP box lain dari sesi terpisah.
    let mut other = Session::new();
    let ops: Vec<Op> =
        serde_json::from_str(r#"[{"op":"primitive","id":"x","shape":{"box":{"size":[7,7,7]}}}]"#)
            .unwrap();
    assert!(other.run(ops, false).committed);
    let other_path = temp("stale-other");
    other.save(&other_path).unwrap();
    let other_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&other_path).unwrap()).unwrap();

    let mut json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    json["bodies"][0]["step"] = other_json["bodies"][0]["step"].clone();
    std::fs::write(&path, json.to_string()).unwrap();

    let loaded = Session::from_file(&path).unwrap();
    assert!(loaded
        .summary()
        .warnings
        .contains(&"oplog_stale".to_string()));
    assert!(
        loaded.design().oplog.is_empty(),
        "mode adopsi: oplog kosong"
    );
    assert_eq!(
        loaded.design().params["t"],
        8.0,
        "params lama dipertahankan"
    );
    let v = loaded.body("plate").unwrap().1.shape.volume().abs();
    assert!(
        (v - 343.0).abs() < 1e-6,
        "body dari berkas, bukan hasil replay: {v}"
    );
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&other_path);
}

#[test]
fn explicit_replay_of_design_with_wrong_fingerprint_is_oplog_stale() {
    let s = plate_session();
    let mut design = s.design().clone();
    design.fingerprint = "0000000000000000".into();
    let err = match Session::replay(design) {
        Ok(_) => panic!("harus OplogStale"),
        Err(e) => e,
    };
    assert_eq!(err.code, ducad_engine::OpErrorCode::OplogStale);
}
