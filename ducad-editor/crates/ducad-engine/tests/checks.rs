//! Checks (P7.1, P7.4, golden P7.5).

use ducad_engine::check::{all_pass, CheckItem, CheckStatus};
use ducad_engine::ops::{Op, OpFile};
use ducad_engine::Session;

fn plate() -> Session {
    let f: OpFile = serde_json::from_str(include_str!("fixtures/plate.ops.json")).unwrap();
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    assert!(s.run(f.ops, false).committed);
    s
}

fn checks(json: &str) -> Vec<CheckItem> {
    serde_json::from_str(json).unwrap()
}

fn ops(json: &str) -> Vec<Op> {
    serde_json::from_str(json).unwrap()
}

#[test]
fn plate_passes_spec_checks() {
    let s = plate();
    let c = checks(
        r#"[{"check":"valid","bodies":"*"},
            {"check":"body_count","expect":1},
            {"id":"tebal","check":"bbox_size","body":"*","expect":[60,40,"$t"],"tol":0.05},
            {"check":"volume","body":"plate","expect":18377.93,"tol_pct":0.5},
            {"check":"volume","body":"*","min":18000,"max":19000},
            {"check":"bbox_max","body":"*","max":[60,40,8]},
            {"check":"hole_count","body":"*","diameter":5.5,"expect":4},
            {"check":"min_wall","body":"*","min":2},
            {"check":"mass","body":"*","density_g_cm3":2.7,"min_g":49,"max_g":50}]"#,
    );
    let summary = s.run_checks(Some(&c));
    for r in &summary.results {
        assert_eq!(r.status, CheckStatus::Pass, "{r:?}");
    }
    assert!(all_pass(&summary.results));
    assert_eq!(summary.pass, 9);
    assert_eq!(summary.results[2].id.as_deref(), Some("tebal"));
}

#[test]
fn failing_and_error_checks() {
    let s = plate();
    let c = checks(
        r#"[{"check":"body_count","expect":2},
            {"check":"hole_count","body":"*","diameter":6.6,"expect":4},
            {"check":"volume","body":"nope","expect":1},
            {"check":"mass","body":"*","max_g":1}]"#,
    );
    let r = s.run_checks(Some(&c)).results;
    assert_eq!(r[0].status, CheckStatus::Fail);
    assert_eq!(r[1].status, CheckStatus::Fail);
    assert_eq!(r[1].measured, 0);
    assert_eq!(r[2].status, CheckStatus::Error);
    // Material default tanpa densitas? Hasilnya pass/fail bila preset dikenal,
    // error bila tidak — keduanya bukan panic.
    assert_ne!(r[3].status, CheckStatus::Pass);
}

#[test]
fn thin_wall_near_hole_fails_with_location() {
    let mut s = plate();
    // Lubang di x = ±20 (r 2.75); w = 47.5 → dinding 1 mm ke tepi x = ±23.75.
    let mut p = s.design().params.clone();
    p.insert("w".into(), 47.5);
    assert!(s.set_params(p).unwrap().committed);
    let r = s
        .run_checks(Some(&checks(
            r#"[{"check":"min_wall","body":"*","min":2}]"#,
        )))
        .results;
    assert_eq!(r[0].status, CheckStatus::Fail);
    let measured = r[0].measured["min"].as_f64().unwrap();
    assert!((measured - 1.0).abs() < 0.15, "{measured}");
    let at = r[0].location.unwrap();
    assert!(at[0].abs() > 20.0 && at[0].abs() < 24.0, "lokasi {at:?}");
}

#[test]
fn batch_report_carries_design_checks() {
    let mut s = plate();
    s.set_checks(checks(r#"[{"check":"body_count","expect":1},{"check":"hole_count","body":"*","diameter":5.5,"expect":4}]"#));
    let report = s.run(ops(r#"[{"op":"chamfer","id":"c1","body":"plate","edges":"of(>Z)[kind=line]","distance":0.5}]"#), false);
    assert!(report.committed, "{:?}", report.error);
    let results = report.checks.expect("checks wajib ada");
    assert_eq!(results.len(), 2);
    assert!(all_pass(&results));
    // Check yang gagal tidak membatalkan batch.
    let report = s.run(
        ops(r#"[{"op":"primitive","id":"extra","shape":{"box":{"size":[5,5,5]}},"at":[100,0,0]}]"#),
        false,
    );
    assert!(report.committed);
    assert_eq!(report.checks.unwrap()[0].status, CheckStatus::Fail);
}

#[test]
fn hole_counter_convex_and_counterbore() {
    let mut s = Session::new();
    let r = s.run(
        ops(
            r#"[{"op":"primitive","id":"pin","shape":{"cylinder":{"r":2.75,"h":10}}},
                {"op":"primitive","id":"blk","shape":{"box":{"size":[30,30,10]}},"at":[20,0,0]},
                {"op":"hole","id":"cb","body":"blk","face":">Z","at_world":[[35,15,10]],
                 "spec":{"iso":"M5","kind":"counterbore"}}]"#,
        ),
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    let c = checks(
        r#"[{"check":"hole_count","body":"pin","diameter":5.5,"expect":0},
            {"check":"hole_count","body":"blk","diameter":5.5,"expect":1},
            {"check":"hole_count","body":"blk","diameter":10.0,"expect":1}]"#,
    );
    for r in s.run_checks(Some(&c)).results {
        assert_eq!(r.status, CheckStatus::Pass, "{r:?}");
    }
}

#[test]
fn clearance_and_interference() {
    let mut s = Session::new();
    let r = s.run(
        ops(
            r#"[{"op":"primitive","id":"box","shape":{"box":{"size":[40,40,20]}}},
                {"op":"primitive","id":"lid","shape":{"box":{"size":[40,40,3]}},"at":[0,0,20.5]}]"#,
        ),
        false,
    );
    assert!(r.committed);
    let c = checks(
        r#"[{"check":"clearance","a":"box","b":"lid","min":0.5},
            {"check":"clearance","a":"box","b":"lid","min":0.6},
            {"check":"no_interference","bodies":"*"},
            {"check":"body_count","expect":2}]"#,
    );
    let r = s.run_checks(Some(&c)).results;
    assert_eq!(r[0].status, CheckStatus::Pass, "{:?}", r[0]);
    assert_eq!(r[1].status, CheckStatus::Fail);
    assert!((r[0].measured.as_f64().unwrap() - 0.5).abs() < 1e-6);
    assert_eq!(r[2].status, CheckStatus::Pass, "{:?}", r[2]);
    assert_eq!(r[3].status, CheckStatus::Pass);
}

#[test]
fn check_items_round_trip_in_design() {
    let c = checks(
        r#"[{"id":"a","note":"catatan","check":"volume","body":"*","expect":"$t","tol_pct":2}]"#,
    );
    let json = serde_json::to_value(&c).unwrap();
    assert_eq!(json[0]["check"], "volume");
    assert_eq!(json[0]["id"], "a");
    let back: Vec<CheckItem> = serde_json::from_value(json).unwrap();
    assert_eq!(back, c);
}
