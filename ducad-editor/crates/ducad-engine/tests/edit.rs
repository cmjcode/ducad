//! `replace_op` dan `propose_edit` (P11).

use ducad_engine::ops::{Op, OpFile};
use ducad_engine::{OpErrorCode, ReplaceOp, Session};

fn plate() -> Session {
    let f: OpFile = serde_json::from_str(include_str!("fixtures/plate.ops.json")).unwrap();
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    assert!(s.run(f.ops, false).committed);
    s
}

fn volume(s: &Session) -> f64 {
    s.summary().bodies[0].volume
}

fn fillet(r: f64) -> Op {
    serde_json::from_value(serde_json::json!(
        {"op":"fillet","id":"f1","body":"plate","edges":"|Z","radius":r}
    ))
    .unwrap()
}

#[test]
fn replace_op_replays_and_failure_keeps_session() {
    let mut s = plate();
    let v0 = volume(&s);
    let r = s.replace_op("f1", fillet(1.0)).unwrap();
    assert!(r.committed, "{:?}", r.error);
    assert!(volume(&s) > v0, "fillet lebih kecil → volume bertambah");
    assert_eq!(s.design().oplog.len(), 4);

    let v1 = volume(&s);
    let r = s.replace_op("f1", fillet(50.0)).unwrap();
    assert!(!r.committed);
    assert_eq!(volume(&s), v1, "sesi lama utuh");

    let e = s.replace_op("nope", fillet(1.0)).unwrap_err();
    assert_eq!(e.code, OpErrorCode::UnknownRef);
}

#[test]
fn propose_edit_params_is_not_applied_until_accept() {
    let mut s = plate();
    let v0 = volume(&s);
    let mut params = s.design().params.clone();
    params.insert("t".into(), 10.0);
    let (p, _) = s.propose_edit(Some(params), Vec::new(), Vec::new()).unwrap();
    assert!(!p.report.committed);
    assert_eq!(volume(&s), v0, "proposal tidak mengubah model");
    assert!(!p.diff.is_empty());
    let r = s.accept(&p.id);
    assert!(r.committed, "{:?}", r.error);
    assert!((volume(&s) - v0 * 10.0 / 8.0).abs() < 1.0);
    assert_eq!(s.design().params["t"], 10.0);
}

#[test]
fn propose_edit_replace_with_bad_op_is_error() {
    let mut s = plate();
    let err = s
        .propose_edit(
            None,
            vec![ReplaceOp {
                id: "f1".into(),
                op: fillet(50.0),
            }],
            Vec::new(),
        )
        .err()
        .unwrap();
    assert_ne!(err.code, OpErrorCode::UnknownRef);
    let err = s
        .propose_edit(
            None,
            vec![ReplaceOp {
                id: "h1".into(),
                op: fillet(1.0),
            }],
            Vec::new(),
        )
        .err()
        .unwrap();
    assert_eq!(err.code, OpErrorCode::InvalidParam, "id op pengganti harus sama");
}
