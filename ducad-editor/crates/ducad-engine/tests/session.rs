//! Tes `Session` (P1.5).

use std::f64::consts::PI;

use ducad_engine::ops::{Op, OpFile, Params};
use ducad_engine::{OpErrorCode, Session};

fn ops(json: &str) -> Vec<Op> {
    serde_json::from_str(json).unwrap()
}

fn plate_file() -> OpFile {
    serde_json::from_str(include_str!("fixtures/plate.ops.json")).unwrap()
}

fn plate_volume(t: f64) -> f64 {
    60.0 * 40.0 * t - 4.0 * (1.0 - PI / 4.0) * 9.0 * t - 4.0 * PI * 2.75 * 2.75 * t
}

fn session_with_plate() -> Session {
    let f = plate_file();
    let mut s = Session::new();
    let report = s.set_params(f.params).unwrap();
    assert!(report.committed, "{:?}", report.error);
    let report = s.run(f.ops, false);
    assert!(report.committed, "{:?}", report.error);
    s
}

fn volume(s: &Session, name: &str) -> f64 {
    s.body(name).unwrap().1.shape.volume().abs()
}

#[test]
fn plate_ops_build_expected_volume() {
    let s = session_with_plate();
    assert_eq!(s.summary().body_names(), vec!["plate".to_string()]);
    let v = volume(&s, "plate");
    assert!(
        (v - plate_volume(8.0)).abs() / plate_volume(8.0) < 1e-3,
        "{v}"
    );
    assert_eq!(s.design().oplog.len(), 4);
    assert!(!s.design().fingerprint.is_empty());
}

#[test]
fn failing_batch_rolls_back_everything() {
    let mut s = session_with_plate();
    let before_bodies = s.summary().body_names();
    let before_len = s.design().oplog.len();
    let before_v = volume(&s, "plate");
    let report = s.run(
        ops(r#"[
          {"op":"primitive","id":"b1","shape":{"box":{"size":[5,5,5]}},"at":[100,0,0]},
          {"op":"chamfer","id":"c1","body":"plate","edges":"of(>Z)","distance":0.5},
          {"op":"fillet","id":"f9","body":"plate","edges":"|Z","radius":100}
        ]"#),
        false,
    );
    assert!(!report.committed);
    let err = report.error.expect("error wajib ada");
    assert_eq!(err.op_index, Some(2));
    assert_eq!(err.op_id.as_deref(), Some("f9"));
    assert_eq!(
        report.outcomes.len(),
        2,
        "outcome op yang sempat berhasil tetap dilaporkan"
    );
    assert_eq!(s.summary().body_names(), before_bodies);
    assert_eq!(s.design().oplog.len(), before_len);
    assert!((volume(&s, "plate") - before_v).abs() < 1e-9);
}

#[test]
fn dry_run_reports_but_changes_nothing() {
    let mut s = session_with_plate();
    let report = s.run(
        ops(r#"[{"op":"primitive","id":"b1","shape":{"box":{"size":[5,5,5]}}}]"#),
        true,
    );
    assert!(!report.committed);
    assert!(report.error.is_none());
    assert_eq!(report.outcomes[0].created, vec!["b1".to_string()]);
    assert_eq!(s.summary().body_names(), vec!["plate".to_string()]);
    assert_eq!(s.design().oplog.len(), 4);
}

#[test]
fn consumed_body_names_its_consumer() {
    let mut s = Session::new();
    let r = s.run(
        ops(r#"[
          {"op":"primitive","id":"a","shape":{"box":{"size":[10,10,10]}}},
          {"op":"primitive","id":"b","shape":{"box":{"size":[10,10,10]}},"at":[5,0,0]},
          {"op":"boolean","id":"u","kind":"union","a":"a","b":"b"}
        ]"#),
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    assert_eq!(
        r.outcomes[2].removed,
        vec!["a".to_string(), "b".to_string()]
    );
    let r = s.run(
        ops(r#"[{"op":"fillet","id":"f","body":"a","edges":"all","radius":1}]"#),
        false,
    );
    let e = r.error.unwrap();
    assert_eq!(e.code, OpErrorCode::BodyConsumed);
    assert!(e.message.contains("'u'"), "{}", e.message);
    let r = s.run(
        ops(r#"[{"op":"fillet","id":"f","body":"zz","edges":"all","radius":1}]"#),
        false,
    );
    assert_eq!(r.error.unwrap().code, OpErrorCode::UnknownRef);
}

#[test]
fn ids_are_validated_at_apply_time() {
    let mut s = Session::new();
    let r = s.run(
        ops(r#"[{"op":"primitive","id":"Bad-Id","shape":{"sphere":{"r":1}}}]"#),
        false,
    );
    assert_eq!(r.error.unwrap().code, OpErrorCode::InvalidParam);
    let r = s.run(
        ops(r#"[{"op":"primitive","id":"x","shape":{"sphere":{"r":1}}},
                {"op":"primitive","id":"x","shape":{"sphere":{"r":2}}}]"#),
        false,
    );
    assert_eq!(r.error.unwrap().code, OpErrorCode::DuplicateId);
}

#[test]
fn undo_redo_reproduces_volume() {
    let mut s = session_with_plate();
    let v_full = volume(&s, "plate");
    let r = s.run(
        ops(r#"[{"op":"shell","id":"sh","body":"plate","remove_faces":"<Z","thickness":1}]"#),
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    let v_shell = volume(&s, "plate");
    assert!(v_shell < v_full);
    assert!(s.undo().unwrap());
    assert!((volume(&s, "plate") - v_full).abs() / v_full < 1e-9);
    assert!(s.redo().unwrap());
    assert!((volume(&s, "plate") - v_shell).abs() / v_shell < 1e-9);
    assert!(!s.redo().unwrap());
}

#[test]
fn set_params_failure_keeps_old_session() {
    let mut s = session_with_plate();
    let v = volume(&s, "plate");
    let mut p: Params = s.design().params.clone();
    p.insert("r".into(), 50.0);
    let report = s.set_params(p).unwrap();
    assert!(!report.committed);
    assert!(report.error.is_some());
    assert_eq!(s.design().params["r"], 3.0);
    assert!((volume(&s, "plate") - v).abs() < 1e-9);

    let mut p: Params = s.design().params.clone();
    p.insert("t".into(), 10.0);
    let report = s.set_params(p).unwrap();
    assert!(report.committed, "{:?}", report.error);
    let v10 = volume(&s, "plate");
    assert!(
        (v10 - plate_volume(10.0)).abs() / plate_volume(10.0) < 1e-3,
        "{v10}"
    );
}

#[test]
fn extrude_modes_and_multi_solid_naming() {
    let mut s = Session::new();
    let r = s.run(
        ops(r#"[
          {"op":"sketch","id":"sk","plane":"XY","entities":[
             {"rect":{"corner":[0,0],"w":10,"h":10,"name":"a"}},
             {"rect":{"corner":[20,0],"w":10,"h":10,"name":"b"}}]},
          {"op":"extrude","id":"two","sketch":"sk","distance":5},
          {"op":"sketch","id":"cut","plane":{"body":"two","face":">Z"},"entities":[
             {"circle":{"center":[0,0],"r":2}}]},
          {"op":"extrude","id":"hole","sketch":"cut","distance":5,"direction":"reverse","mode":"cut","target":"two"}
        ]"#),
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    assert_eq!(
        r.outcomes[1].created,
        vec!["two".to_string(), "two.2".to_string()]
    );
    assert_eq!(r.outcomes[3].modified, vec!["two".to_string()]);
    let v = volume(&s, "two");
    let expected = 500.0 - PI * 4.0 * 5.0;
    assert!((v - expected).abs() / expected < 1e-3, "{v} vs {expected}");
    // Face atas "two" (0..10) di z=5; lingkaran dipusatkan di centroid face.
    let (_, frame) = s.sketch("cut").unwrap();
    assert!((frame.origin[2] - 5.0).abs() < 1e-9);
}

#[test]
fn per_object_creates_named_bodies_in_draw_order() {
    let mut s = Session::new();
    let r = s.run(
        ops(r#"[
          {"op":"sketch","id":"sk","plane":"XY","entities":[
             {"circle":{"center":[0,0],"r":5,"name":"logo"}},
             {"circle":{"center":[30,0],"r":5}}
          ]},
          {"op":"extrude","id":"logo3d","sketch":"sk","distance":3,"per_object":true}
        ]"#),
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    assert_eq!(
        r.outcomes[1].created,
        vec!["logo3d.logo".to_string(), "logo3d.p2".to_string()]
    );
    assert!(s.body("logo3d.logo").is_ok());
    assert!(s.body("logo3d.p2").is_ok());
}

#[test]
fn from_style_sets_material_color() {
    let mut s = Session::new();
    let r = s.run(
        ops(r##"[
          {"op":"sketch","id":"sk","plane":"XY","entities":[
             {"path":{
                "subpaths":[{
                   "start":[0,0],
                   "segs":[
                     {"line":{"to":[10,0]}},
                     {"line":{"to":[10,10]}},
                     {"line":{"to":[0,10]}},
                     {"line":{"to":[0,0]}}
                   ],
                   "closed":true
                }],
                "style":{"fill":"#FF0000","opacity":1.0},
                "name":"red_box"
             }}
          ]},
          {"op":"extrude","id":"solid","sketch":"sk","distance":2,"material":"from_style"}
        ]"##),
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    let (bid, _) = s.body("solid").unwrap();
    let body = s.model().doc.bodies.get(bid).unwrap();
    assert_eq!(
        body.material.preset,
        ducad_core::MaterialPreset::MattePlastic
    );
    assert!((body.material.base_color[0] - 1.0).abs() < 1e-2);
    assert!(body.material.base_color[1] < 1e-2);
    assert!(body.material.base_color[2] < 1e-2);
}

/// Regresi REVIEW-2026-09-24 #12: dua entitas berstyle beda → warna dari
/// entitas pertama menurut draw order, sama di setiap run.
#[test]
fn from_style_multi_entity_is_deterministic() {
    let square = |x0: i32, color: &str, name: &str| {
        format!(
            r##"{{"path":{{"subpaths":[{{"start":[{x0},0],"segs":[
                {{"line":{{"to":[{x1},0]}}}},{{"line":{{"to":[{x1},10]}}}},
                {{"line":{{"to":[{x0},10]}}}},{{"line":{{"to":[{x0},0]}}}}],
                "closed":true}}],"style":{{"fill":"{color}","opacity":1.0}},"name":"{name}"}}}}"##,
            x1 = x0 + 10
        )
    };
    let json = format!(
        r#"[{{"op":"sketch","id":"sk","plane":"XY","entities":[{},{}]}},
            {{"op":"extrude","id":"solid","sketch":"sk","distance":2,"material":"from_style"}}]"#,
        square(0, "#FF0000", "red"),
        square(20, "#0000FF", "blue")
    );
    for _ in 0..8 {
        let mut s = Session::new();
        let r = s.run(ops(&json), false);
        assert!(r.committed, "{:?}", r.error);
        let (bid, _) = s.body("solid").unwrap();
        let c = s.model().doc.bodies.get(bid).unwrap().material.base_color;
        assert!(
            (c[0] - 1.0).abs() < 1e-2 && c[2] < 1e-2,
            "warna harus merah: {c:?}"
        );
    }
}

#[test]
fn translucent_fill_becomes_glass() {
    let mut s = Session::new();
    let r = s.run(
        ops(r##"[
          {"op":"sketch","id":"sk","plane":"XY","entities":[
             {"path":{
                "subpaths":[{
                   "start":[0,0],
                   "segs":[
                     {"line":{"to":[10,0]}},
                     {"line":{"to":[10,10]}},
                     {"line":{"to":[0,10]}},
                     {"line":{"to":[0,0]}}
                   ],
                   "closed":true
                }],
                "style":{"fill":"#0000FF","opacity":0.5},
                "name":"glass_box"
             }}
          ]},
          {"op":"extrude","id":"glass","sketch":"sk","distance":2,"material":"from_style"}
        ]"##),
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    let (bid, _) = s.body("glass").unwrap();
    let body = s.model().doc.bodies.get(bid).unwrap();
    assert_eq!(
        body.material.preset,
        ducad_core::MaterialPreset::TranslucentGlass
    );
    assert!(body.material.base_color[3] < 0.99);
}

#[test]
fn per_object_with_cut_is_invalid_param() {
    let mut s = session_with_plate();
    let r = s.run(
        ops(r#"[
          {"op":"sketch","id":"sk","plane":"XY","entities":[
             {"circle":{"center":[0,0],"r":2,"name":"c1"}}
          ]},
          {"op":"extrude","id":"cut_obj","sketch":"sk","distance":5,"per_object":true,"mode":"cut","target":"plate"}
        ]"#),
        false,
    );
    assert!(!r.committed);
    let err = r.error.unwrap();
    assert_eq!(err.code, OpErrorCode::InvalidParam);
    assert!(
        err.message.contains("per_object hanya untuk body baru"),
        "{}",
        err.message
    );
}

#[test]
fn extrude_without_new_fields_is_unchanged() {
    let s = session_with_plate();
    let v = volume(&s, "plate");
    let expected = plate_volume(8.0);
    assert!((v - expected).abs() / expected < 1e-3, "{v} vs {expected}");
}

#[test]
fn outline_extrude_uses_stroke_width() {
    let mut s = Session::new();
    let r = s.run(
        ops(r#"[
          {"op":"sketch","id":"sk","plane":"XY","entities":[
             {"circle":{"center":[0,0],"r":10,"name":"c1"}}
          ]},
          {"op":"extrude","id":"ring","sketch":"sk","distance":5,"outline":{"width":2.0}}
        ]"#),
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    let v = volume(&s, "ring");
    // Outer r = 11, inner r = 9 => Area = PI * (121 - 81) = 40 * PI
    // Height = 5 => Volume = 200 * PI
    let expected = 200.0 * PI;
    assert!(
        (v - expected).abs() / expected < 0.02,
        "Volume outline extrude {v} vs {expected}"
    );
}

/// Regresi REVIEW-2026-09-24 #25: preset tak dikenal → `InvalidParam`.
#[test]
fn unknown_material_preset_is_invalid_param() {
    let mut s = Session::new();
    let r = s.run(
        ops(r#"[
          {"op":"sketch","id":"sk","plane":"XY","entities":[{"circle":{"center":[0,0],"r":5}}]},
          {"op":"extrude","id":"x","sketch":"sk","distance":2,"material":"unobtainium"}
        ]"#),
        false,
    );
    assert!(!r.committed);
    let err = r.error.expect("harus gagal");
    assert_eq!(err.code, OpErrorCode::InvalidParam);
}

/// Regresi REVIEW-2026-09-24 #25: nama eksplisit berpola `e<n>` tetap
/// dipakai sebagai nama body per-objek.
#[test]
fn explicit_e_digit_name_is_kept_for_per_object_body() {
    let mut s = Session::new();
    let r = s.run(
        ops(r#"[
          {"op":"sketch","id":"sk","plane":"XY","entities":[
             {"circle":{"center":[0,0],"r":5,"name":"e5"}},
             {"circle":{"center":[20,0],"r":5}}
          ]},
          {"op":"extrude","id":"x","sketch":"sk","distance":2,"per_object":true}
        ]"#),
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    assert_eq!(
        r.outcomes[1].created,
        vec!["x.e5".to_string(), "x.p2".to_string()]
    );
}
