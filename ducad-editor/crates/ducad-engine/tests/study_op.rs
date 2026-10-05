//! `Op::Study` + checks tegangan + render hasil (P17).

use ducad_engine::check::{CheckItem, CheckStatus};
use ducad_engine::ops::{Op, OpFile};
use ducad_engine::sim::{render_study_svg, Overlay, StudyRenderOptions};
use ducad_engine::{OpErrorCode, Session};
use ducad_sim::CancelToken;

fn ops(json: &str) -> Vec<Op> {
    serde_json::from_str(json).unwrap()
}

fn fixture() -> Session {
    let f: OpFile = serde_json::from_str(include_str!("fixtures/sim_bracket.ops.json")).unwrap();
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    s.set_checks(f.checks);
    let r = s.run(f.ops, false);
    assert!(r.committed, "{:?}", r.error);
    s
}

#[test]
fn study_op_replays_and_is_deterministic() {
    let mut s = fixture();
    // Studi tidak membuat body.
    assert_eq!(s.summary().body_names(), ["bracket"]);
    let first = s.run_study("tip_load", &CancelToken::new()).unwrap();
    // Kantilever 25 mm, F = 200 N, penampang 50×5: σ ≈ 6FL/(bh²) = 24 MPa.
    assert!(
        first.max_von_mises_mpa > 8.0 && first.max_von_mises_mpa < 80.0,
        "{}",
        first.max_von_mises_mpa
    );
    assert!(first.max_displacement_mm > 0.0 && first.max_displacement_mm < 0.5);
    // Gaya reaksi mengimbangi beban.
    let rz: f64 = first.reactions.iter().map(|r| r.force_n[2]).sum();
    assert!((rz - 200.0).abs() < 1e-3, "reaksi z {rz}");

    // Replay dari nol → laporan identik bit demi bit.
    let second = fixture()
        .run_study("tip_load", &CancelToken::new())
        .unwrap();
    assert_eq!(
        serde_json::to_string(first.as_ref()).unwrap(),
        serde_json::to_string(second.as_ref()).unwrap()
    );
    assert_eq!(first.as_ref(), second.as_ref());
}

#[test]
fn study_op_checks_need_a_fresh_result() {
    let mut s = fixture();
    let before = s.run_checks(None);
    assert_eq!(
        before.error, 3,
        "belum dijalankan → error: {:?}",
        before.results
    );
    assert!(before.results[0].message.contains("belum dijalankan"));

    let ran = s.run_all_studies(&CancelToken::new());
    assert_eq!(ran.len(), 1);
    assert!(ran[0].1.is_ok(), "{:?}", ran[0].1);
    let after = s.run_checks(None);
    assert_eq!(
        (after.pass, after.fail, after.error),
        (3, 0, 0),
        "{:?}",
        after.results
    );

    // Batas yang sengaja mustahil → Fail dengan angka terukur.
    let strict: Vec<CheckItem> = serde_json::from_str(
        r#"[{"check":"max_stress","max_mpa":0.5},{"check":"min_safety_factor","min":1000}]"#,
    )
    .unwrap();
    let r = s.run_checks(Some(&strict));
    assert_eq!(r.fail, 2, "{:?}", r.results);
    assert!(r.results[0].measured.as_f64().unwrap() > 0.5);

    // Model berubah → hasil basi → error lagi sampai dijalankan ulang.
    assert!(s
        .run(ops(r#"[{"op":"fillet","id":"soften","body":"bracket","edges":"|X[y=5][z=5]","radius":2}]"#), false)
        .committed);
    let stale = s.run_checks(None);
    assert_eq!(stale.error, 3, "{:?}", stale.results);
    assert!(stale.results[0].message.contains("basi"));
    assert_eq!(stale.results[0].status, CheckStatus::Error);
}

#[test]
fn study_op_reports_setup_errors() {
    let mut s = Session::new();
    assert!(
        s.run(
            ops(r#"[{"op":"primitive","id":"bar","shape":{"box":{"size":[10,10,40]}}}]"#),
            false
        )
        .committed
    );
    let study = |fixtures: &str, body: &str| {
        ops(&format!(
            r#"[{{"op":"study","id":"st","setup":{{"body":"{body}","fixtures":{fixtures},
            "loads":[{{"id":"f","faces":">Z","kind":"force","newton":[0,0,-10]}}],
            "mesh":{{"cell_mm":2.5}}}}}}]"#
        ))
    };
    // Body tak dikenal dan selector kosong ditolak saat op diterapkan.
    let r = s.run(
        study(r#"[{"id":"fx","faces":"<Z","kind":"fixed"}]"#, "nope"),
        false,
    );
    assert_eq!(r.error.unwrap().code, OpErrorCode::UnknownRef);
    let r = s.run(
        study(
            r#"[{"id":"fx","faces":"all[kind=cylinder]","kind":"fixed"}]"#,
            "bar",
        ),
        false,
    );
    assert_eq!(r.error.unwrap().code, OpErrorCode::SelectorEmpty);

    // Tanpa material mekanik → sim_no_material dengan hint berbahasa Inggris.
    assert!(
        s.run(
            study(r#"[{"id":"fx","faces":"<Z","kind":"fixed"}]"#, "bar"),
            false
        )
        .committed
    );
    let e = s.run_study("st", &CancelToken::new()).unwrap_err();
    assert_eq!(e.code, OpErrorCode::SimNoMaterial);
    assert!(e.hint.unwrap().contains("set_material"));

    // Tanpa fixture → sim_underconstrained.
    let mut free = Session::new();
    assert!(
        free.run(
            ops(
                r#"[{"op":"primitive","id":"bar","shape":{"box":{"size":[10,10,40]}}},
                    {"op":"set_material","id":"m","body":"bar","material":"s235"}]"#
            ),
            false
        )
        .committed
    );
    assert!(free.run(study("[]", "bar"), false).committed);
    let e = free.run_study("st", &CancelToken::new()).unwrap_err();
    assert_eq!(e.code, OpErrorCode::SimUnderconstrained);
    assert!(e.hint.unwrap().contains("fixture"));
    assert!(e.message.is_ascii());
    // Studi tak dikenal.
    assert_eq!(
        free.run_study("ghost", &CancelToken::new())
            .unwrap_err()
            .code,
        OpErrorCode::UnknownRef
    );
}

#[test]
fn study_op_renders_heatmap_svg() {
    let mut s = fixture();
    let report = s.run_study("tip_load", &CancelToken::new()).unwrap();
    let geo = s.body("bracket").unwrap().1;
    for overlay in [
        Overlay::Stress,
        Overlay::Displacement,
        Overlay::SafetyFactor,
    ] {
        let svg = render_study_svg(
            geo,
            &report,
            &StudyRenderOptions {
                view: ducad_engine::render::View::Iso,
                width: 640,
                height: 480,
                overlay,
                deform_scale: None,
                yield_mpa: 276.0,
            },
        )
        .unwrap();
        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
        assert!(svg.matches("<path").count() > 200, "{overlay:?}");
        assert!(svg.len() < 4 * 1024 * 1024);
    }
    let bad = render_study_svg(
        geo,
        &report,
        &StudyRenderOptions {
            view: ducad_engine::render::View::Top,
            width: 0,
            height: 480,
            overlay: Overlay::Stress,
            deform_scale: Some(1.0),
            yield_mpa: 276.0,
        },
    );
    assert_eq!(bad.unwrap_err().code, OpErrorCode::InvalidParam);
}
