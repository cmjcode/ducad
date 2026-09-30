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
    let (p, _) = s
        .propose_edit(Some(params), Vec::new(), Vec::new())
        .unwrap();
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
    assert_eq!(
        err.code,
        OpErrorCode::InvalidParam,
        "id op pengganti harus sama"
    );
}

// ---------------------------------------------------------------------
// P16: edit_oplog (hapus op, dry run) dan propose_oplog_edit.
// ---------------------------------------------------------------------

#[test]
fn remove_op_replays_and_keeps_undo_history_consistent() {
    let mut s = plate();
    let v_holes = volume(&s);
    let r = s
        .edit_oplog(None, Vec::new(), vec!["h1".into()], false)
        .unwrap();
    assert!(r.committed, "{:?}", r.error);
    assert!(volume(&s) > v_holes, "tanpa lubang → volume bertambah");
    let ids: Vec<&str> = s.design().oplog.iter().map(|o| o.id()).collect();
    assert_eq!(ids, ["base", "plate", "f1"]);

    // Batch pertama (run 4 op) tinggal 3 op: undo membuang ketiganya
    // tanpa panik atau memotong oplog di tempat yang salah.
    assert!(s.undo().unwrap());
    assert!(s.design().oplog.is_empty());
    assert!(s.summary().bodies.is_empty());
}

#[test]
fn remove_op_still_referenced_fails_with_hint_and_keeps_session() {
    let mut s = plate();
    let v0 = volume(&s);
    let r = s
        .edit_oplog(None, Vec::new(), vec!["base".into()], false)
        .unwrap();
    assert!(!r.committed);
    let e = r.error.expect("error");
    assert_eq!(e.code, OpErrorCode::UnknownRef);
    let hint = e.hint.unwrap_or_default();
    assert!(
        hint.contains("base") && hint.contains("replace_op"),
        "{hint}"
    );
    assert_eq!(volume(&s), v0, "sesi lama utuh");
    assert_eq!(s.design().oplog.len(), 4);

    let e = s
        .edit_oplog(None, Vec::new(), vec!["nope".into()], false)
        .unwrap_err();
    assert_eq!(e.code, OpErrorCode::UnknownRef);
    assert!(e.context["ops"].as_array().is_some_and(|a| a.len() == 4));
}

#[test]
fn edit_oplog_dry_run_reports_proposed_state_only() {
    let mut s = plate();
    let v0 = volume(&s);
    let r = s
        .edit_oplog(
            None,
            vec![ReplaceOp {
                id: "f1".into(),
                op: fillet(1.0),
            }],
            Vec::new(),
            true,
        )
        .unwrap();
    assert!(!r.committed, "dry run tidak meng-commit");
    assert!(r.error.is_none(), "{:?}", r.error);
    assert!(r.summary.bodies[0].volume > v0, "summary = keadaan usulan");
    assert_eq!(volume(&s), v0, "model tidak berubah");
}

#[test]
fn propose_oplog_edit_with_remove_applies_on_accept() {
    let mut s = plate();
    let v0 = volume(&s);
    let (p, _) = s
        .propose_oplog_edit(None, Vec::new(), vec!["f1".into()], Vec::new())
        .unwrap();
    assert_eq!(p.remove, vec!["f1".to_string()]);
    assert_eq!(volume(&s), v0, "proposal tidak mengubah model");
    let r = s.accept(&p.id);
    assert!(r.committed, "{:?}", r.error);
    assert!(volume(&s) > v0, "tanpa fillet → volume bertambah");
    assert!(s.design().oplog.iter().all(|o| o.id() != "f1"));
}

#[test]
fn preview_edit_does_not_touch_current_model() {
    let s = plate();
    let v0 = volume(&s);
    let p = ducad_engine::preview_edit(s.model(), s.design(), None, &[], &["h1".to_string()], &[])
        .unwrap();
    assert!(!p.report.committed);
    assert!(p.session.summary().bodies[0].volume > v0);
    assert!(!p.diff.is_empty(), "lubang yang hilang muncul di diff");
    assert_eq!(volume(&s), v0);
}

#[test]
fn examples_replay() {
    for (name, text, _) in ducad_engine::ops::EXAMPLES {
        let f: OpFile = serde_json::from_str(text).unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut s = Session::new();
        assert!(s.set_params(f.params).unwrap().committed, "{name}");
        s.set_checks(f.checks);
        let r = s.run(f.ops, false);
        assert!(r.committed, "{name}: {:?}", r.error);
        let checks = s.run_checks(None);
        assert_eq!(
            checks.fail + checks.error,
            0,
            "{name}: {:?}",
            checks.results
        );
    }
}
