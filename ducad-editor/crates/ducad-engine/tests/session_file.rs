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
    assert_eq!(json["format_version"], 4);
    assert!(json["bodies"][0].get("step").is_none(), "body turunan tanpa STEP");
    assert!(json["bodies"][0]["brep"]["producer"].is_string());
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
    s.save_legacy(&path).unwrap();
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
        adopted.summary().body_names(),
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
    s.save_legacy(&path).unwrap();

    // STEP box lain dari sesi terpisah.
    let mut other = Session::new();
    let ops: Vec<Op> =
        serde_json::from_str(r#"[{"op":"primitive","id":"x","shape":{"box":{"size":[7,7,7]}}}]"#)
            .unwrap();
    assert!(other.run(ops, false).committed);
    let other_path = temp("stale-other");
    other.save_legacy(&other_path).unwrap();
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

fn read_json(path: &std::path::Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn legacy_save_keeps_step_and_v2() {
    let mut s = plate_session();
    let path = temp("legacy");
    s.save_legacy(&path).unwrap();
    let json = read_json(&path);
    assert_eq!(json["format_version"], 2);
    assert!(json["bodies"][0]["step"].is_string());
    assert!(json["bodies"][0].get("brep").is_none());
    let loaded = Session::from_file(&path).unwrap();
    assert_eq!(loaded.design().oplog.len(), 4);
    let _ = std::fs::remove_file(&path);
}

/// Load tanpa cache == load dengan cache (F1.2): oplog adalah sumber kebenaran.
#[test]
fn v4_load_without_cache_equals_load_with_cache() {
    let mut s = plate_session();
    let path = temp("nocache");
    s.save(&path).unwrap();
    let with_cache = Session::from_file(&path).unwrap();

    let mut json = read_json(&path);
    for b in json["bodies"].as_array_mut().unwrap() {
        b.as_object_mut().unwrap().remove("brep");
    }
    let without = Session::from_json(&json.to_string()).unwrap();

    // Cache dari backend lain = miss, bukan error.
    let mut foreign = read_json(&path);
    foreign["bodies"][0]["brep"]["producer"] = "ducad-truck/1".into();
    let foreign = Session::from_json(&foreign.to_string()).unwrap();

    let v = |s: &Session| s.body("plate").unwrap().1.shape.volume().abs();
    assert!((v(&with_cache) - v(&without)).abs() < 1e-9);
    assert!((v(&with_cache) - v(&foreign)).abs() < 1e-9);
    assert_eq!(
        ducad_engine::session::fingerprint(with_cache.model()),
        ducad_engine::session::fingerprint(without.model())
    );
    assert!(without.summary().warnings.is_empty());
    let _ = std::fs::remove_file(&path);
}

/// Replay gagal → body diambil dari cache (non-strict) dan jadi body dasar
/// yang membawa STEP; strict → error.
#[test]
fn v4_failed_replay_falls_back_to_cache_with_warning() {
    let mut s = plate_session();
    let path = temp("fallback");
    s.save(&path).unwrap();
    let mut json = read_json(&path);
    // Oplog tidak lagi menghasilkan geometri tercatat: sidik jari DAN
    // ringkasan geometri berbeda (sidik jari saja bisa beda antar-backend).
    json["design"]["fingerprint"] = "0000000000000000".into();
    json["design"]["geometry"][0]["volume"] = 1.0.into();
    let text = json.to_string();

    let loaded = Session::from_json(&text).unwrap();
    let w = loaded.summary().warnings;
    assert!(w.contains(&"oplog_stale".to_string()), "{w:?}");
    assert!(loaded.design().oplog.is_empty());
    assert!(loaded.design().base_bodies[0].step.is_some());
    assert!(loaded.body("plate").is_ok());

    assert!(Session::from_json_strict(&text).is_err());

    // Tanpa cache dan replay gagal → error, bukan body hilang diam-diam.
    for b in json["bodies"].as_array_mut().unwrap() {
        b.as_object_mut().unwrap().remove("brep");
    }
    assert!(Session::from_json(&json.to_string()).is_err());
    let _ = std::fs::remove_file(&path);
}

/// Pembaca toleran (F1.4): STEP body berkas lama rusak, oplog masih utuh.
#[test]
fn legacy_file_with_corrupt_step_replays_oplog() {
    let mut s = plate_session();
    let path = temp("corrupt");
    s.save_legacy(&path).unwrap();
    let mut json = read_json(&path);
    json["bodies"][0]["step"] = "ISO-10303-21;\nrusak".into();
    let loaded = Session::from_json(&json.to_string()).unwrap();
    assert_eq!(loaded.design().oplog.len(), 4);
    assert!(loaded
        .summary()
        .warnings
        .iter()
        .any(|w| w.starts_with("step_unreadable")));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn migrate_legacy_v2_to_v4_preserves_geometry_and_metadata() {
    let mut s = plate_session();
    let path = temp("migrate");
    s.save_legacy(&path).unwrap();
    let legacy = std::fs::read_to_string(&path).unwrap();
    let before = Session::from_json(&legacy).unwrap();

    let (out, report) = ducad_engine::migrate::migrate_json(&legacy).unwrap();
    assert!(report.changed, "{report:?}");
    assert_eq!((report.input_version, report.output_version), (2, 4));
    assert_eq!(report.derived, vec!["plate".to_string()]);
    assert!(report.kept_step.is_empty());

    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let old: serde_json::Value = serde_json::from_str(&legacy).unwrap();
    assert!(v["bodies"][0].get("step").is_none());
    assert_eq!(v["bodies"][0]["uuid"], old["bodies"][0]["uuid"]);
    assert_eq!(v["design"], old["design"], "oplog tidak boleh berubah");
    assert_eq!(v["sketch"], old["sketch"]);

    let after = Session::from_json(&out).unwrap();
    assert_eq!(
        ducad_engine::session::fingerprint(before.model()),
        ducad_engine::session::fingerprint(after.model())
    );
    // Idempoten.
    let (again, r2) = ducad_engine::migrate::migrate_json(&out).unwrap();
    assert!(!r2.changed);
    assert_eq!(again, out);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn migrate_leaves_files_without_oplog_or_stale_untouched() {
    let mut s = plate_session();
    let path = temp("migrate-skip");
    s.save_legacy(&path).unwrap();
    let mut json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

    let mut no_design = json.clone();
    no_design.as_object_mut().unwrap().remove("design");
    no_design["format_version"] = 1.into();
    let text = no_design.to_string();
    let (out, r) = ducad_engine::migrate::migrate_json(&text).unwrap();
    assert!(!r.changed);
    assert_eq!(r.skipped.as_deref(), Some("no_oplog"));
    assert_eq!(out, text);

    json["design"]["fingerprint"] = "0000000000000000".into();
    let text = json.to_string();
    let (out, r) = ducad_engine::migrate::migrate_json(&text).unwrap();
    assert!(!r.changed, "{r:?}");
    assert_eq!(out, text);
    assert_eq!(r.kept_step.len(), 1);
    let _ = std::fs::remove_file(&path);
}

/// Berkas dari backend kernel lain (PLAN_MIGRASI_TRUCK F3): sidik jari hash
/// tidak cocok persis, tetapi geometri replay sama dalam toleransi → oplog
/// dipakai, bukan dianggap basi.
#[test]
fn foreign_backend_fingerprint_still_replays_v4_and_v2() {
    for legacy in [false, true] {
        let mut s = plate_session();
        let path = temp(if legacy { "foreign-v2" } else { "foreign-v4" });
        if legacy {
            s.save_legacy(&path).unwrap();
        } else {
            s.save(&path).unwrap();
        }
        let mut json = read_json(&path);
        if !legacy {
            assert!(json["design"]["geometry"].as_array().is_some_and(|g| !g.is_empty()));
        } else {
            // Berkas lama tidak punya ringkasan geometri.
            json["design"].as_object_mut().unwrap().remove("geometry");
        }
        json["design"]["fingerprint"] = "0123456789abcdef".into();
        std::fs::write(&path, serde_json::to_string(&json).unwrap()).unwrap();
        let loaded = Session::from_file(&path).unwrap();
        assert_eq!(loaded.design().oplog.len(), 4, "legacy={legacy}");
        assert!(
            !loaded.warnings().iter().any(|w| w == "oplog_stale"),
            "legacy={legacy}: {:?}",
            loaded.warnings()
        );
        let _ = std::fs::remove_file(&path);
    }
}
