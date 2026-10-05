//! Diagnosis error + fix terverifikasi (P9).

use ducad_engine::ops::Op;
use ducad_engine::{OpErrorCode, Session};

fn ops(json: &str) -> Vec<Op> {
    serde_json::from_str(json).unwrap()
}

fn stepped_plate() -> Session {
    // Plat 60x40x8 dengan tangga: bagian y 0..10 hanya setinggi 2.1 mm.
    let mut s = Session::new();
    let r = s.run(
        ops(r#"[{"op":"primitive","id":"plate","shape":{"box":{"size":[60,40,8]}}},
                {"op":"primitive","id":"cutter","shape":{"box":{"size":[60,10,5.9]}},"at":[0,0,2.1]},
                {"op":"boolean","id":"step","kind":"subtract","a":"plate","b":"cutter"}]"#),
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    s
}

#[test]
fn fillet_too_large_gets_verified_fix() {
    let mut s = stepped_plate();
    let r = s.run(
        ops(r#"[{"op":"fillet","id":"f","body":"step","edges":"|X[y=0][z=2.1]","radius":3}]"#),
        false,
    );
    let e = r.error.expect("fillet r=3 harus gagal");
    assert_eq!(e.code, OpErrorCode::FilletRadiusTooLarge, "{e:?}");
    let limit = e.context["limit"].as_f64().unwrap();
    assert!((limit - 2.1).abs() < 1e-3, "{limit}");
    assert!(!e.fixes.is_empty(), "harus ada fix terverifikasi");
    assert!(e.fixes[0].verified);
    // Model tidak berubah (batch dibatalkan); op hasil patch berhasil.
    let patched = e.fixes[0].patched_op.clone();
    let r = s.run(vec![patched], false);
    assert!(r.committed, "{:?}", r.error);
}

#[test]
fn hole_outside_face_is_blocking() {
    let mut s = Session::new();
    let r = s.run(
        ops(r#"[{"op":"primitive","id":"plate","shape":{"box":{"size":[60,40,8]}}},
                {"op":"hole","id":"h","body":"plate","face":">Z","at":[[100,100]],"spec":{"iso":"M5"}}]"#),
        false,
    );
    let e = r.error.unwrap();
    assert_eq!(e.code, OpErrorCode::HoleOutsideFace, "{e:?}");
    assert!(e.fixes.is_empty());
}

#[test]
fn blind_hole_deeper_than_body_is_a_warning() {
    let mut s = Session::new();
    let r = s.run(
        ops(r#"[{"op":"primitive","id":"plate","shape":{"box":{"size":[60,40,8]}}},
                {"op":"hole","id":"h","body":"plate","face":">Z","at":[[0,0]],"spec":{"iso":"M5","depth":12}}]"#),
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    assert!(
        r.outcomes[1]
            .warnings
            .iter()
            .any(|w| w.starts_with("hole_deeper_than_body")),
        "{:?}",
        r.outcomes[1]
    );
}

#[test]
fn subtract_disjoint_is_boolean_no_overlap() {
    let mut s = Session::new();
    let r = s.run(
        ops(
            r#"[{"op":"primitive","id":"a","shape":{"box":{"size":[10,10,10]}}},
                {"op":"primitive","id":"b","shape":{"box":{"size":[10,10,10]}},"at":[50,0,0]},
                {"op":"boolean","id":"c","kind":"subtract","a":"a","b":"b"}]"#,
        ),
        false,
    );
    assert_eq!(r.error.unwrap().code, OpErrorCode::BooleanNoOverlap);
    let r = s.run(
        ops(
            r#"[{"op":"primitive","id":"a","shape":{"box":{"size":[10,10,10]}}},
                {"op":"primitive","id":"b","shape":{"box":{"size":[10,10,10]}},"at":[50,0,0]},
                {"op":"boolean","id":"c","kind":"union","a":"a","b":"b"}]"#,
        ),
        false,
    );
    assert!(
        r.committed,
        "union tanpa irisan hanya peringatan: {:?}",
        r.error
    );
    assert!(!r.outcomes[2].warnings.is_empty());
}

const GAP_SKETCH: &str = r#"{"op":"sketch","id":"sk","plane":"XY","entities":[
    {"line":{"from":[0,0],"to":[20,0],"name":"a"}},
    {"line":{"from":[20,0],"to":[20,10],"name":"b"}},
    {"line":{"from":[20,10],"to":[0,10],"name":"c"}},
    {"line":{"from":[0,10],"to":[0,0.2],"name":"d"}}]}"#;

#[test]
fn open_gap_profile_gets_sketch_fix() {
    let mut s = Session::new();
    let batch =
        format!(r#"[{GAP_SKETCH}, {{"op":"extrude","id":"blk","sketch":"sk","distance":5}}]"#);
    let r = s.run(ops(&batch), false);
    let e = r.error.expect("profil terbuka");
    assert_eq!(e.code, OpErrorCode::ProfileOpenGap, "{e:?}");
    assert!((e.context["distance"].as_f64().unwrap() - 0.2).abs() < 1e-9);
    let fix = e.fixes.first().expect("fix terverifikasi");
    assert_eq!(fix.patch.op_id, "sk");
    // Terapkan fix ke op sketch lalu ulangi batch.
    let mut retry = ops(&batch);
    retry[0] = fix.patched_op.clone();
    let r = s.run(retry, false);
    assert!(r.committed, "{:?}", r.error);
    let v = s.body("blk").unwrap().1.shape.volume().abs();
    assert!((v - 1000.0).abs() < 1e-6, "{v}");
}

#[test]
fn over_constrained_sketch_gets_remove_fix() {
    let mut s = Session::new();
    let r = s.run(
        ops(r#"[{"op":"sketch","id":"sk","plane":"XY",
                 "entities":[{"line":{"from":[0,0],"to":[10,3],"name":"l"}}],
                 "constraints":[{"horizontal":"l"},{"vertical":"l"}]}]"#),
        false,
    );
    let e = r.error.unwrap();
    assert_eq!(e.code, OpErrorCode::OverConstrained);
    let fix = e.fixes.first().expect("fix hapus constraint");
    assert!(fix.patch.remove[0].starts_with("/constraints/"));
    let r = s.run(vec![fix.patched_op.clone()], false);
    assert!(r.committed, "{:?}", r.error);
}

#[test]
fn undiagnosed_kernel_failure_has_no_fake_fix() {
    let mut s = Session::new();
    let r = s.run(
        ops(r#"[{"op":"sketch","id":"sk","plane":"XY","entities":[{"rect":{"center":[0,0],"w":10,"h":10}}]},
                {"op":"revolve","id":"r","sketch":"sk","axis":"v"}]"#),
        false,
    );
    let e = r.error.expect("sumbu memotong profil");
    assert_eq!(e.code, OpErrorCode::KernelFailed, "{e:?}");
    assert!(e.fixes.is_empty());
}

#[test]
fn shell_opens_any_side_and_limits_depth() {
    let mut s = Session::new();
    let r = s.run(
        ops(r#"[{"op":"primitive","id":"a","shape":{"box":{"size":[20,20,20]}}},
                {"op":"shell","id":"sa","body":"a","remove_faces":"<X","thickness":2},
                {"op":"primitive","id":"b","shape":{"box":{"size":[20,20,20]}},"at":[40,0,0]},
                {"op":"shell","id":"sb","body":"b","remove_faces":"<Z","thickness":2,"depth":10}]"#),
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    let full = r.outcomes[1].detail["volume"].as_f64().unwrap();
    let limited = r.outcomes[3].detail["volume"].as_f64().unwrap();
    assert!((full - (8000.0 - 16.0 * 16.0 * 18.0)).abs() < 40.0, "{full}");
    assert!((limited - (8000.0 - 16.0 * 16.0 * 10.0)).abs() < 40.0, "{limited}");
}

#[test]
fn shell_depth_too_deep_gets_verified_fixes() {
    let mut s = Session::new();
    let r = s.run(
        ops(r#"[{"op":"primitive","id":"a","shape":{"box":{"size":[20,20,20]}}},
                {"op":"shell","id":"sa","body":"a","remove_faces":">Z","thickness":2,"depth":19}]"#),
        false,
    );
    let e = r.error.expect("depth 19 harus gagal");
    assert_eq!(e.code, OpErrorCode::ShellDepthTooDeep, "{e:?}");
    assert!((e.context["max_depth"].as_f64().unwrap() - 18.0).abs() < 1e-3);
    assert_eq!(e.fixes.len(), 2, "{:?}", e.fixes);
    assert!(e.fixes.iter().all(|f| f.verified));
    let r = s.run(
        vec![
            ops(r#"[{"op":"primitive","id":"a","shape":{"box":{"size":[20,20,20]}}}]"#).remove(0),
            e.fixes[1].patched_op.clone(),
        ],
        false,
    );
    assert!(r.committed, "{:?}", r.error);
}

#[test]
fn shell_depth_with_two_open_faces_is_invalid_param() {
    let mut s = Session::new();
    let r = s.run(
        ops(r#"[{"op":"primitive","id":"a","shape":{"box":{"size":[20,20,20]}}},
                {"op":"shell","id":"sa","body":"a","remove_faces":"|Z","thickness":2,"depth":5}]"#),
        false,
    );
    assert_eq!(r.error.expect("harus gagal").code, OpErrorCode::InvalidParam);
}
