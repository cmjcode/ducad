//! Tes wajib P11 dengan `MockBackend`.

use ducad_assist::{assist, AssistAction, MockBackend};
use ducad_engine::ops::OpFile;
use ducad_engine::{OpErrorCode, Session};

fn plate() -> Session {
    let f: OpFile = serde_json::from_str(include_str!(
        "../../ducad-engine/tests/fixtures/plate.ops.json"
    ))
    .unwrap();
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    assert!(s.run(f.ops, false).committed);
    s
}

fn volume(s: &Session) -> f64 {
    s.summary().bodies[0].volume
}

const SET_T10: &str = r#"Berikut usulan:
```json
{"rationale":"param t mengatur tebal","actions":[{"set_params":{"t":10}}]}
```"#;

#[test]
fn set_params_becomes_proposal_not_commit() {
    let mut s = plate();
    let v0 = volume(&s);
    let mut b = MockBackend::new([SET_T10]);
    let out = assist(&mut s, &mut b, "tebal jadi 10", &[], 3).unwrap();
    assert_eq!(out.iterations, 1);
    assert_eq!(
        out.reply.actions,
        vec![AssistAction::SetParams(
            [("t".to_string(), 10.0)].into_iter().collect()
        )]
    );
    let p = out.proposal.expect("proposal");
    assert_eq!(p.params.as_ref().unwrap()["t"], 10.0);
    assert_eq!(volume(&s), v0, "tidak ada commit otomatis");
    assert!(b.calls[0].contains("INSTRUKSI\ntebal jadi 10"));
    assert!(b.calls[0].contains("\"t\":8"));
    let r = s.accept(&p.id);
    assert!(r.committed);
    assert!((volume(&s) - v0 * 10.0 / 8.0).abs() < 1.0);
}

#[test]
fn non_json_retries_once_then_errors() {
    let mut s = plate();
    let mut b = MockBackend::new(["tentu, saya ubah tebalnya", "masih bukan json"]);
    let err = assist(&mut s, &mut b, "tebal jadi 10", &[], 3).unwrap_err();
    assert_eq!(err.code, OpErrorCode::InvalidParam);
    assert!(err.message.contains("bukan JSON"), "{}", err.message);
    assert_eq!(b.calls.len(), 2, "tepat satu kali ulang");
    assert!(b.calls[1].contains("bukan JSON valid"));

    let mut b = MockBackend::new(["halo", SET_T10]);
    let out = assist(&mut s, &mut b, "tebal jadi 10", &[], 3).unwrap();
    assert!(out.proposal.is_some());
    assert_eq!(out.transcript.len(), 2);
}

#[test]
fn invalid_op_error_is_fed_back_and_second_iteration_succeeds() {
    let mut s = plate();
    let bad = r#"{"rationale":"fillet","actions":[{"append_ops":[{"op":"fillet","id":"f2","body":"plate","edges":"of(>Z)","radius":50}]}]}"#;
    let good = r#"{"rationale":"fillet kecil","actions":[{"append_ops":[{"op":"fillet","id":"f2","body":"plate","edges":"of(>Z)","radius":1}]}]}"#;
    let mut b = MockBackend::new([bad, good]);
    let out = assist(&mut s, &mut b, "bulatkan tepi atas", &[], 3).unwrap();
    assert_eq!(out.iterations, 2);
    assert!(out.proposal.is_some());
    assert!(out.last_error.is_none());
    assert!(
        b.calls[1].contains("ERROR TERAKHIR"),
        "error diumpankan: {}",
        b.calls[1]
    );
    assert_eq!(s.design().oplog.len(), 4, "tidak ada commit");
}

#[test]
fn bad_shape_is_fed_back() {
    let mut s = plate();
    let mut b = MockBackend::new([r#"{"rationale":"x","actions":[{"set_param":{"t":10}}]}"#, SET_T10]);
    let out = assist(&mut s, &mut b, "tebal jadi 10", &[], 3).unwrap();
    assert_eq!(out.iterations, 2);
    assert!(b.calls[1].contains("tidak sesuai kontrak"));
    assert!(out.proposal.is_some());
}

#[test]
fn max_iters_is_respected() {
    let mut s = plate();
    let bad = r#"{"rationale":"x","actions":[{"append_ops":[{"op":"fillet","id":"f2","body":"nope","edges":"|Z","radius":1}]}]}"#;
    let mut b = MockBackend::new([bad; 10]);
    let out = assist(&mut s, &mut b, "fillet", &[], 2).unwrap();
    assert_eq!(out.iterations, 2);
    assert_eq!(b.calls.len(), 2);
    assert!(out.proposal.is_none());
    assert_eq!(out.last_error.unwrap().code, OpErrorCode::UnknownRef);
}

#[test]
fn explain_ends_without_proposal() {
    let mut s = plate();
    let mut b = MockBackend::new([r#"{"rationale":"r","actions":[{"explain":"Tebal diatur param t."}]}"#]);
    let out = assist(&mut s, &mut b, "apa itu t?", &[], 3).unwrap();
    assert!(out.proposal.is_none());
    assert_eq!(out.reply.message(), Some("Tebal diatur param t."));
}

#[test]
fn failed_checks_are_fed_back_once() {
    let mut s = plate();
    s.set_checks(
        serde_json::from_value(serde_json::json!([
            {"id":"tebal","check":"bbox_size","body":"*","expect":[60,40,12],"tol":0.05}
        ]))
        .unwrap(),
    );
    let t11 = r#"{"rationale":"r","actions":[{"set_params":{"t":11}}]}"#;
    let t12 = r#"{"rationale":"r","actions":[{"set_params":{"t":12}}]}"#;
    let mut b = MockBackend::new([t11, t12, t12]);
    let out = assist(&mut s, &mut b, "tebal jadi 12", &[], 3).unwrap();
    assert_eq!(b.calls.len(), 2);
    assert!(b.calls[1].contains("CHECK GAGAL"));
    let p = out.proposal.unwrap();
    assert_eq!(p.params.as_ref().unwrap()["t"], 12.0);
}

#[test]
fn lessons_are_capped_and_dropped_when_too_long() {
    let s = plate();
    let lessons: Vec<String> = (0..8).map(|i| format!("pelajaran {i}")).collect();
    let ctx = ducad_assist::prompt::Context {
        instruction: "x",
        last_error: None,
        failed_checks: &[],
        lessons: &lessons,
    };
    let m = ducad_assist::prompt::user_message(&s, &ctx);
    assert!(m.contains("pelajaran 4") && !m.contains("pelajaran 5"));
    let long: Vec<String> = vec!["x".repeat(20_000)];
    let ctx = ducad_assist::prompt::Context {
        lessons: &long,
        ..ctx
    };
    let m = ducad_assist::prompt::user_message(&s, &ctx);
    assert!(!m.contains("PELAJARAN"));
}

/// Tidak ada jalur kode crate ini yang meng-commit perubahan model.
#[test]
fn crate_never_commits() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        for forbidden in [".run(", ".accept(", ".set_params(", ".replace_op(", ".save("] {
            assert!(
                !text.contains(forbidden),
                "{} memanggil {forbidden}",
                path.display()
            );
        }
    }
}
