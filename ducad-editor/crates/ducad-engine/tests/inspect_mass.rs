//! Properti massa di `inspect` + `Op::SetMaterial` + checks massa (P16).

use std::path::PathBuf;

use ducad_engine::check::CheckStatus;
use ducad_engine::inspect::{summarize, DEFAULT_TOPOLOGY_LIMIT};
use ducad_engine::ops::{Op, OpFile};
use ducad_engine::{OpErrorCode, Session};

fn ops(json: &str) -> Vec<Op> {
    serde_json::from_str(json).unwrap()
}

fn fixture() -> Session {
    let f: OpFile = serde_json::from_str(include_str!("fixtures/mass_bracket.ops.json")).unwrap();
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    s.set_checks(f.checks);
    let r = s.run(f.ops, false);
    assert!(r.committed, "{:?}", r.error);
    s
}

fn close(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * b.abs().max(1e-9)
}

#[test]
fn inspect_mass_bracket_fixture_reports_inertia() {
    let s = fixture();
    let sum = summarize(&s, None, false, DEFAULT_TOPOLOGY_LIMIT).unwrap();
    let b = sum.bodies.iter().find(|b| b.name == "bracket").unwrap();
    // Alas 50×30×5 + dinding 50×5×30, bertumpuk di 50×5×5.
    let volume = 50.0 * 30.0 * 5.0 + 50.0 * 5.0 * 30.0 - 50.0 * 5.0 * 5.0;
    assert!(close(b.volume, volume, 1e-6), "{}", b.volume);
    assert!(close(b.mass_g.unwrap(), volume / 1000.0 * 2.70, 1e-4));
    let mech = b.mechanical.as_ref().unwrap();
    assert_eq!(mech.source, "al_6061_t6");
    assert_eq!(mech.young_modulus_gpa, Some(68.9));
    assert_eq!(mech.density_g_cm3, Some(2.70));

    let i = b.inertia_com.unwrap();
    for (r, c) in [(0, 1), (0, 2), (1, 2)] {
        assert_eq!(i[r][c], i[c][r], "inertia_com harus simetris");
    }
    let m = b.principal_moments.unwrap();
    assert!(m[0] > 0.0 && m[0] <= m[1] && m[1] <= m[2], "{m:?}");
    // Positif-definit: semua minor utama > 0.
    let det2 = i[0][0] * i[1][1] - i[0][1] * i[0][1];
    let det3 = i[0][0] * (i[1][1] * i[2][2] - i[1][2] * i[1][2])
        - i[0][1] * (i[0][1] * i[2][2] - i[1][2] * i[0][2])
        + i[0][2] * (i[0][1] * i[1][2] - i[1][1] * i[0][2]);
    assert!(i[0][0] > 0.0 && det2 > 0.0 && det3 > 0.0);
    // Jejak tensor tidak berubah oleh rotasi ke sumbu utama.
    assert!(close(i[0][0] + i[1][1] + i[2][2], m[0] + m[1] + m[2], 1e-6));
    // Bracket simetris terhadap bidang x = 25.
    assert!(close(b.center_of_mass.unwrap()[0], 25.0, 1e-6));
    let rg = b.radius_of_gyration.unwrap();
    assert!(close(rg[2], (m[2] / b.mass_g.unwrap()).sqrt(), 1e-3));

    let checks = s.run_checks(None);
    assert_eq!(checks.pass, 3, "{:?}", checks.results);
}

#[test]
fn inspect_mass_assembly_of_two_materials_matches_formula() {
    let mut s = Session::new();
    let r = s.run(
        ops(r#"[
            {"op":"primitive","id":"a","shape":{"box":{"size":[10,10,10]}}},
            {"op":"set_material","id":"a_mat","body":"a","material":"al_6061_t6"},
            {"op":"primitive","id":"b","shape":{"box":{"size":[10,10,10]}}},
            {"op":"transform","id":"b_pos","body":"b","translate":[30,0,0]},
            {"op":"set_material","id":"b_mat","body":"b","material":"s235"}
        ]"#),
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    let sum = summarize(&s, None, false, DEFAULT_TOPOLOGY_LIMIT).unwrap();
    let asm = sum
        .assembly
        .expect("dua body bermaterial → laporan gabungan");
    let (ma, mb) = (2.70, 7.85); // 1000 mm³ = 1 cm³
    assert!(close(asm.total_mass_g, ma + mb, 1e-6));
    let cx = (ma * 5.0 + mb * 35.0) / (ma + mb);
    assert!(
        close(asm.center_of_mass[0], cx, 1e-4),
        "{:?}",
        asm.center_of_mass
    );
    assert!(close(asm.center_of_mass[1], 5.0, 1e-6));
    // Kubus: I = m·s²/6 di pusatnya; sumbu z bergeser sejauh jarak x ke COM.
    let own = |m: f64| m * 100.0 / 6.0;
    let izz = own(ma) + ma * (5.0 - cx).powi(2) + own(mb) + mb * (35.0 - cx).powi(2);
    assert!(
        close(asm.inertia_com[2][2], izz, 1e-4),
        "{} vs {izz}",
        asm.inertia_com[2][2]
    );
    assert!(close(asm.inertia_com[0][0], own(ma) + own(mb), 1e-4));
    assert!(asm.skipped.is_empty());
    // Satu body saja → tanpa laporan gabungan.
    assert!(summarize(&s, Some("a"), false, 10)
        .unwrap()
        .assembly
        .is_none());
}

#[test]
fn inspect_mass_set_material_validates_and_undoes() {
    let mut s = Session::new();
    assert!(
        s.run(
            ops(r#"[{"op":"primitive","id":"a","shape":{"box":{"size":[10,10,10]}}}]"#),
            false
        )
        .committed
    );
    let bad = s.run(
        ops(r#"[{"op":"set_material","id":"m","body":"a","material":"unobtainium"}]"#),
        false,
    );
    assert!(!bad.committed);
    let e = bad.error.unwrap();
    assert_eq!(e.code, OpErrorCode::InvalidParam);
    assert!(e.message.contains("al_6061_t6"), "{}", e.message);
    let unphysical = s.run(
        ops(
            r#"[{"op":"set_material","id":"m","body":"a","material":{"custom":{
            "density_g_cm3":1,"young_modulus_gpa":2,"poisson_ratio":0.7,
            "yield_strength_mpa":10,"ultimate_strength_mpa":20}}}]"#,
        ),
        false,
    );
    assert_eq!(unphysical.error.unwrap().code, OpErrorCode::InvalidParam);

    let mass = |s: &Session| summarize(s, Some("a"), false, 10).unwrap().bodies[0].mass_g;
    assert_eq!(mass(&s), Some(1.2)); // preset visual bawaan
    assert!(
        s.run(
            ops(r#"[{"op":"set_material","id":"m","body":"a","material":"TI_6AL_4V"}]"#),
            false
        )
        .committed
    );
    assert_eq!(mass(&s), Some(4.43));
    assert!(s.undo().unwrap());
    assert_eq!(mass(&s), Some(1.2));
}

#[test]
fn inspect_mass_material_survives_boolean_and_file_roundtrip() {
    let mut s = Session::new();
    let r = s.run(
        ops(r#"[
            {"op":"primitive","id":"a","shape":{"box":{"size":[10,10,10]}}},
            {"op":"set_material","id":"m","body":"a","material":"copper"},
            {"op":"primitive","id":"b","shape":{"box":{"size":[10,10,20]}}},
            {"op":"boolean","id":"c","kind":"union","a":"a","b":"b"}
        ]"#),
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    let source = |s: &Session| {
        summarize(s, Some("c"), false, 10).unwrap().bodies[0]
            .mechanical
            .as_ref()
            .map(|m| m.source.clone())
    };
    assert_eq!(source(&s).as_deref(), Some("copper"));

    let path: PathBuf =
        std::env::temp_dir().join(format!("ducad-engine-mass-{}.ducad", std::process::id()));
    s.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.contains("\"library\": \"copper\""),
        "material mekanik harus tersimpan"
    );
    let loaded = Session::from_file(&path).unwrap();
    assert_eq!(source(&loaded).as_deref(), Some("copper"));
    std::fs::remove_file(&path).ok();
}

#[test]
fn inspect_mass_checks_fail_with_numbers() {
    let mut s = Session::new();
    assert!(
        s.run(
            ops(r#"[{"op":"primitive","id":"a","shape":{"box":{"size":[10,20,30]}}}]"#),
            false
        )
        .committed
    );
    let checks: Vec<ducad_engine::check::CheckItem> = serde_json::from_str(
        r#"[
        {"check":"center_of_mass","body":"a","expect":[5,10,15]},
        {"check":"center_of_mass","body":"a","expect":[5,10,16]},
        {"check":"moment_of_inertia","body":"a","axis":"z","density_g_cm3":1.0,"min":249.9,"max":250.1},
        {"check":"moment_of_inertia","body":"a","axis":"principal_max","density_g_cm3":1.0,"max":100}
    ]"#,
    )
    .unwrap();
    let r = s.run_checks(Some(&checks));
    let status: Vec<CheckStatus> = r.results.iter().map(|c| c.status).collect();
    // Izz = m(a²+b²)/12 = 6 g × 500 / 12 = 250 g·mm².
    assert_eq!(
        status,
        [
            CheckStatus::Pass,
            CheckStatus::Fail,
            CheckStatus::Pass,
            CheckStatus::Fail
        ],
        "{:?}",
        r.results
    );
    assert!(r.results[1].message.contains("pusat massa"));
}
