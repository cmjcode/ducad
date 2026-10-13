//! Check DFM/standar, paket aturan, brief, dan keterlacakan (W0, W1, W5).

use ducad_engine::check::{CheckItem, CheckStatus, Severity};
use ducad_engine::ops::{Op, OpFile};
use ducad_engine::tooling::{call_core_tool, call_stateless_tool, NoPaths};
use ducad_engine::Session;
use serde_json::json;

fn plate() -> Session {
    let f: OpFile = serde_json::from_str(include_str!("fixtures/plate.ops.json")).unwrap();
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    assert!(s.run(f.ops, false).committed);
    s
}

fn session(ops: &str) -> Session {
    let ops: Vec<Op> = serde_json::from_str(ops).unwrap();
    let mut s = Session::new();
    let report = s.run(ops, false);
    assert!(report.committed, "{:?}", report.error);
    s
}

fn run(s: &Session, json: &str) -> Vec<ducad_engine::check::CheckResult> {
    let c: Vec<CheckItem> = serde_json::from_str(json).unwrap();
    s.run_checks(Some(&c)).results
}

/// Blok 40×40×10 dengan kantong 20×20×5 di atas: 4 sudut dalam tajam.
const POCKET: &str = r#"[
    {"op":"primitive","id":"blk","shape":{"box":{"size":[40,40,10]}}},
    {"op":"primitive","id":"tool","shape":{"box":{"size":[20,20,5]}},"at":[10,10,5]},
    {"op":"boolean","id":"part","kind":"subtract","a":"blk","b":"tool"}
]"#;

#[test]
fn hole_depth_ratio_measures_depth_over_diameter() {
    let s = plate();
    let r = run(
        &s,
        r#"[{"check":"hole_depth_ratio","body":"*","max_ratio":5},
            {"check":"hole_depth_ratio","body":"*","max_ratio":1},
            {"check":"hole_depth_ratio","body":"*","max_ratio":1,"min_diameter":6}]"#,
    );
    assert_eq!(r[0].status, CheckStatus::Pass, "{:?}", r[0]);
    assert_eq!(r[0].measured["holes"], 4);
    let ratio = r[0].measured["max_ratio"].as_f64().unwrap();
    assert!((ratio - 8.0 / 5.5).abs() < 0.01, "{ratio}");
    assert_eq!(r[1].status, CheckStatus::Fail);
    assert_eq!(r[1].measured["over_limit"], 4);
    // Semua lubang lebih kecil dari min_diameter → tidak ada yang diperiksa.
    assert_eq!(r[2].status, CheckStatus::Pass);
}

#[test]
fn hole_web_is_the_material_between_hole_and_edge() {
    let mut s = plate();
    let r = run(&s, r#"[{"check":"hole_web","body":"*","min_ratio":1.0}]"#);
    assert_eq!(r[0].status, CheckStatus::Pass, "{:?}", r[0]);
    let web = r[0].measured["web"].as_f64().unwrap();
    assert!((web - 7.25).abs() < 0.1, "{web}");

    // w = 47.5 → 1 mm ke tepi x.
    let mut p = s.design().params.clone();
    p.insert("w".into(), 47.5);
    assert!(s.set_params(p).unwrap().committed);
    let r = run(
        &s,
        r#"[{"check":"hole_web","body":"*","min_ratio":1.0},
            {"check":"hole_web","body":"*","min":0.5},
            {"check":"hole_web","body":"*"}]"#,
    );
    assert_eq!(r[0].status, CheckStatus::Fail);
    let web = r[0].measured["web"].as_f64().unwrap();
    assert!((web - 1.0).abs() < 0.1, "{web}");
    assert_eq!(r[0].measured["over_limit"], 4);
    assert!(r[0].location.is_some());
    assert_eq!(r[1].status, CheckStatus::Pass);
    assert_eq!(r[2].status, CheckStatus::Error, "butuh salah satu batas");
}

#[test]
fn draft_angle_fails_on_vertical_walls_and_passes_on_a_cone() {
    let s = plate();
    let r = run(
        &s,
        r#"[{"check":"min_draft_angle","body":"*","min_deg":1}]"#,
    );
    assert_eq!(r[0].status, CheckStatus::Fail);
    assert!(r[0].measured["min_draft_deg"].as_f64().unwrap() < 0.01);

    // Kerucut terpancung: dinding miring terhadap Z.
    let s = session(r#"[{"op":"primitive","id":"c","shape":{"cone":{"r1":20,"r2":15,"h":20}}}]"#);
    let r = run(
        &s,
        r#"[{"check":"min_draft_angle","body":"*","pull":"z","min_deg":5},
            {"check":"min_draft_angle","body":"*","pull":"z","min_deg":20}]"#,
    );
    let draft = r[0].measured["min_draft_deg"].as_f64().unwrap();
    assert!(
        (draft - (5.0f64 / 20.0).atan().to_degrees()).abs() < 0.2,
        "{draft}"
    );
    assert_eq!(r[0].status, CheckStatus::Pass, "{:?}", r[0]);
    assert_eq!(r[1].status, CheckStatus::Fail);
}

#[test]
fn overhang_ignores_the_bed_and_finds_a_ledge() {
    let s = plate();
    let r = run(&s, r#"[{"check":"max_overhang","body":"*"}]"#);
    assert_eq!(r[0].status, CheckStatus::Pass, "{:?}", r[0]);

    // Tiang 10×10×20 dengan topi 30×30×5: bawah topi menggantung 800 mm².
    let s = session(
        r#"[{"op":"primitive","id":"post","shape":{"box":{"size":[10,10,20]}},"at":[10,10,0]},
            {"op":"primitive","id":"cap","shape":{"box":{"size":[30,30,5]}},"at":[0,0,20]},
            {"op":"boolean","id":"tee","kind":"union","a":"post","b":"cap"}]"#,
    );
    let r = run(
        &s,
        r#"[{"check":"max_overhang","body":"*"},
            {"check":"max_overhang","body":"*","max_area":1000},
            {"check":"max_overhang","body":"*","up":"x","max_area":100000}]"#,
    );
    assert_eq!(r[0].status, CheckStatus::Fail);
    let area = r[0].measured["overhang_area"].as_f64().unwrap();
    assert!((area - 800.0).abs() < 1.0, "{area}");
    assert_eq!(r[1].status, CheckStatus::Pass);
    assert_eq!(r[2].status, CheckStatus::Pass);
}

#[test]
fn sharp_pocket_corners_cannot_be_milled_until_filleted() {
    let mut s = session(POCKET);
    let check = r#"[{"check":"min_internal_radius","body":"*","min":2},
                    {"check":"min_internal_radius","body":"*","min":4}]"#;
    let r = run(&s, check);
    assert_eq!(r[0].status, CheckStatus::Fail, "{:?}", r[0]);
    assert_eq!(r[0].measured["sharp_corners"], 4);
    assert!(r[0].location.is_some());

    let fillet: Vec<Op> = serde_json::from_str(
        r#"[{"op":"fillet","id":"corners","body":"part","edges":"|Z[len=5]","radius":3}]"#,
    )
    .unwrap();
    let report = s.run(fillet, false);
    assert!(report.committed, "{:?}", report.error);
    let r = run(&s, check);
    assert_eq!(r[0].status, CheckStatus::Pass, "{:?}", r[0]);
    assert_eq!(r[0].measured["sharp_corners"], 0);
    assert_eq!(r[0].measured["min_radius"], 3.0);
    assert_eq!(r[1].status, CheckStatus::Fail, "R3 < R4");

    // Plat polos: sudut luar membulat bukan sudut dalam.
    let r = run(
        &plate(),
        r#"[{"check":"min_internal_radius","body":"*","min":2}]"#,
    );
    assert_eq!(r[0].status, CheckStatus::Pass, "{:?}", r[0]);
}

#[test]
fn thread_engagement_uses_the_measured_depth_of_through_taps() {
    let s = session(
        r#"[{"op":"primitive","id":"blk","shape":{"box":{"size":[30,30,8]}}},
            {"op":"hole","id":"tap","body":"blk","face":">Z","at":[[0,0]],
             "spec":{"iso":"M6","kind":"tapped"}}]"#,
    );
    let r = run(
        &s,
        r#"[{"check":"thread_engagement","body":"*","min_ratio":1.0},
            {"check":"thread_engagement","body":"*","min_ratio":1.5}]"#,
    );
    assert_eq!(r[0].status, CheckStatus::Pass, "{:?}", r[0]);
    // Dinding silinder terukur sedikit lebih pendek dari tebal 8 mm karena
    // champer masuk lubang tap: panjang ulir efektif, bukan tebal pelat.
    let ratio = r[0].measured["min_ratio"].as_f64().unwrap();
    assert!((1.2..=8.0 / 6.0 + 0.01).contains(&ratio), "{ratio}");
    assert_eq!(r[1].status, CheckStatus::Fail);
    // Body tanpa ulir lulus.
    let r = run(
        &plate(),
        r#"[{"check":"thread_engagement","body":"*","min_ratio":1.5}]"#,
    );
    assert_eq!(r[0].status, CheckStatus::Pass);
}

#[test]
fn fit_and_general_tolerance_follow_the_iso_tables() {
    let s = plate();
    let r = run(
        &s,
        r#"[{"check":"fit","nominal":25,"hole":"H7","shaft":"g6","kind":"clearance"},
            {"check":"fit","nominal":25,"hole":"H7","shaft":"g6","max_clearance":0.02},
            {"check":"fit","nominal":25,"hole":"H7","shaft":"p6","kind":"interference"},
            {"check":"general_tolerance","body":"*","class":"m","process":"cnc"},
            {"check":"general_tolerance","body":"*","class":"f","process":"fdm"}]"#,
    );
    // Ø25 H7 = +0.021/0, g6 = −0.007/−0.020 → kelonggaran 0.007 … 0.041.
    assert_eq!(r[0].status, CheckStatus::Pass, "{:?}", r[0]);
    assert_eq!(r[0].measured["min_clearance"], 0.007);
    assert_eq!(r[0].measured["max_clearance"], 0.041);
    assert_eq!(r[1].status, CheckStatus::Fail);
    assert_eq!(r[2].status, CheckStatus::Pass, "{:?}", r[2]);
    assert_eq!(r[3].status, CheckStatus::Pass);
    // 60 → ±0.3, 40 → ±0.3, 8 → ±0.2 (kelas m).
    assert_eq!(r[3].measured["dimensions"][0]["tol"], 0.3);
    assert_eq!(r[3].measured["dimensions"][2]["tol"], 0.2);
    assert_eq!(r[4].status, CheckStatus::Fail);
}

#[test]
fn warnings_are_reported_but_do_not_block() {
    let s = plate();
    let c: Vec<CheckItem> = serde_json::from_str(
        r#"[{"check":"body_count","expect":1},
            {"check":"hole_depth_ratio","body":"*","max_ratio":1,"severity":"warning",
             "citation":"Guideline: test"},
            {"check":"body_count","expect":7}]"#,
    )
    .unwrap();
    let summary = s.run_checks(Some(&c[..2]));
    assert_eq!(
        (summary.pass, summary.fail, summary.error, summary.warn),
        (1, 0, 0, 1)
    );
    assert!(!summary.is_blocking());
    assert_eq!(summary.results[1].severity, Severity::Warning);
    assert_eq!(
        summary.results[1].citation.as_deref(),
        Some("Guideline: test")
    );
    assert!(s.run_checks(Some(&c)).is_blocking());
    // Severity bawaan tidak ditulis ke berkas (kompatibel dengan berkas lama).
    let text = serde_json::to_string(&c[0]).unwrap();
    assert!(!text.contains("severity"), "{text}");
}

#[test]
fn rule_pack_expands_into_cited_checks_through_set_checks() {
    let mut s = session(POCKET);
    let out = call_core_tool(
        &mut s.core(),
        "set_checks",
        json!({ "checks": [{"id": "satu", "check": "body_count", "expect": 1}],
                "rule_packs": [{"pack": "dfm-cnc", "vars": {"min_internal_radius": 2}}] }),
        &NoPaths,
    )
    .unwrap();
    let p = &out.payload;
    assert!(p["disclaimer"]
        .as_str()
        .unwrap()
        .contains("qualified engineer"));
    let results = p["results"].as_array().unwrap();
    assert_eq!(results.len(), 7, "{p}");
    let corner = results
        .iter()
        .find(|r| r["id"] == "dfm-cnc/internal-radius")
        .unwrap();
    assert_eq!(corner["status"], "fail");
    assert_eq!(corner["rule"], "dfm-cnc@1/internal-radius");
    assert!(corner["citation"]
        .as_str()
        .unwrap()
        .starts_with("Guideline:"));
    let tol = results
        .iter()
        .find(|r| r["id"] == "dfm-cnc/general-tolerance")
        .unwrap();
    assert!(tol["citation"]
        .as_str()
        .unwrap()
        .starts_with("ISO 2768-1:1989"));
    // Sudut tajam ber-severity error → menghalangi.
    assert_eq!(p["fail"], 1, "{p}");
    assert_eq!(s.design().checks.len(), 7);

    let bad = call_core_tool(
        &mut s.core(),
        "set_checks",
        json!({ "rule_packs": ["dfm-plasma"] }),
        &NoPaths,
    );
    assert!(bad.is_err());

    let catalog = call_stateless_tool("get_schema", json!({ "rule_packs": true })).unwrap();
    let packs = catalog.payload["rule_packs"].as_array().unwrap();
    assert!(packs.iter().any(|p| p["id"] == "dfm-sheet-metal"));
    assert!(packs[0]["rules"][0]["citation"].is_string());
}

#[test]
fn brief_links_requirements_to_checks() {
    let mut s = plate();
    let brief = json!({
        "title": "Plat dudukan",
        "function": "Menahan motor NEMA 17 pada rangka.",
        "material": "Al 6061-T6",
        "process": "cnc",
        "requirements": [
            { "id": "R1", "text": "Ukuran luar 60 x 40 x 8 mm", "source": "user" },
            { "id": "R2", "text": "4 lubang baut M5" },
            { "id": "R3", "text": "Massa di bawah 40 g" },
            { "id": "R4", "text": "Bisa dibuat dengan CNC 3 sumbu" }
        ]
    });
    let out = call_core_tool(
        &mut s.core(),
        "set_brief",
        json!({ "brief": brief }),
        &NoPaths,
    )
    .unwrap();
    assert_eq!(out.payload["traceability"]["unverified"], 4);
    assert_eq!(out.payload["suggested_rule_pack"], "dfm-cnc");

    let out = call_core_tool(
        &mut s.core(),
        "set_checks",
        json!({ "checks": [
            { "id": "ukuran", "requirement": "R1", "check": "bbox_size", "body": "*", "expect": [60, 40, 8] },
            { "id": "lubang", "requirement": "R2", "check": "hole_count", "body": "*", "diameter": 5.5, "expect": 4 },
            { "id": "massa", "requirement": "R3", "check": "mass", "body": "*", "density_g_cm3": 2.7, "max_g": 40 },
            { "id": "yatim", "requirement": "R9", "check": "body_count", "expect": 1 }
        ], "rule_packs": [{ "pack": "dfm-cnc", "requirement": "R4" }] }),
        &NoPaths,
    )
    .unwrap();
    let t = &out.payload["traceability"];
    assert_eq!(t["verified"], 3, "{t}");
    assert_eq!(t["failed"], 1, "massa 49,6 g > 40 g");
    assert_eq!(t["unverified"], 0);
    assert_eq!(t["rows"][2]["status"], "failed");
    assert_eq!(t["rows"][3]["checks"].as_array().unwrap().len(), 6);
    assert_eq!(t["unknown_requirements"], json!(["R9"]));

    // Brief ikut tersimpan di design dan paket aturan tak disarankan lagi.
    let got = call_core_tool(&mut s.core(), "get_brief", json!({}), &NoPaths).unwrap();
    assert_eq!(got.payload["brief"]["requirements"][1]["id"], "R2");
    assert!(got.payload.get("suggested_rule_pack").is_none());
    let text = serde_json::to_string(s.design()).unwrap();
    let back: ducad_engine::DesignDoc = serde_json::from_str(&text).unwrap();
    assert_eq!(back.brief, s.design().brief);

    let dup = json!({ "requirements": [{ "id": "R1", "text": "a" }, { "id": "R1", "text": "b" }] });
    assert!(call_core_tool(
        &mut s.core(),
        "set_brief",
        json!({ "brief": dup }),
        &NoPaths
    )
    .is_err());
}

#[test]
fn package_status_tracks_review_and_stale_approval() {
    let mut s = plate();
    let status = |s: &mut Session| {
        call_core_tool(&mut s.core(), "package_status", json!({}), &NoPaths)
            .unwrap()
            .payload
    };
    assert_eq!(status(&mut s)["state"], "draft");
    assert_eq!(status(&mut s)["revision"], "A");

    let out = call_core_tool(
        &mut s.core(),
        "submit_for_review",
        json!({ "description": "Rilis awal", "by": "ana", "date": "2026-10-13" }),
        &NoPaths,
    )
    .unwrap();
    assert_eq!(out.payload["state"], "in_review");
    assert_eq!(out.payload["history"][0]["description"], "Rilis awal");

    // Persetujuan hanya lewat API sesi (bukan tool core yang bisa dipanggil agent live).
    let (mut release, signature) = s.release();
    release.approve(&signature, "budi", "2026-10-13").unwrap();
    s.set_release(release);
    assert_eq!(status(&mut s)["state"], "approved");
    assert!(call_core_tool(&mut s.core(), "submit_for_review", json!({}), &NoPaths).is_err());

    // Desain berubah → persetujuan basi → kembali draf; pengajuan berikutnya = revisi B.
    let mut p = s.design().params.clone();
    p.insert("t".into(), 10.0);
    assert!(s.set_params(p).unwrap().committed);
    let st = status(&mut s);
    assert_eq!(st["state"], "draft");
    assert_eq!(st["approval_stale"], true);
    let out = call_core_tool(
        &mut s.core(),
        "submit_for_review",
        json!({ "description": "Plat ditebalkan jadi 10 mm", "date": "2026-10-14" }),
        &NoPaths,
    )
    .unwrap();
    assert_eq!(out.payload["revision"], "B");
    assert_eq!(out.payload["history"].as_array().unwrap().len(), 2);
}
