//! Jenis studi P18 lewat `Op::Study`: frekuensi, buckling, termal, tegangan
//! termal — dibandingkan dengan rumus balok, plus checks dan cache.

use ducad_engine::ops::{Op, OpFile};
use ducad_engine::sim::{AnalysisReport, StudyOutcome};
use ducad_engine::{OpErrorCode, Session};
use ducad_sim::CancelToken;

const LEN: f64 = 100.0;
const W: f64 = 10.0;
const T: f64 = 5.0;

fn fixture() -> Session {
    let f: OpFile = serde_json::from_str(include_str!("fixtures/sim_modes.ops.json")).unwrap();
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    s.set_checks(f.checks);
    let r = s.run(f.ops, false);
    assert!(r.committed, "{:?}", r.error);
    s
}

fn analysis(s: &mut Session, id: &str) -> AnalysisReport {
    match s.run_study_any(id, &CancelToken::new()).unwrap() {
        StudyOutcome::Analysis(r) => r.as_ref().clone(),
        StudyOutcome::Stress(_) => panic!("{id}: diharapkan hasil analisis"),
    }
}

fn rel(actual: f64, expected: f64) -> f64 {
    (actual - expected).abs() / expected.abs()
}

#[test]
fn study_kinds_match_beam_theory() {
    let mut s = fixture();
    let m = ducad_core::library_material("al_6061_t6").unwrap().props;
    let e_mpa = m.young_modulus_gpa * 1000.0;
    // Sumbu lemah: I = w t³ / 12.
    let inertia = W * T.powi(3) / 12.0;

    // Kantilever: f1 = (1.8751² / 2π) · sqrt(E I / (ρ A L⁴)), satuan mm–N–tonne.
    let rho = m.density_g_cm3 * 1e-9;
    let f1 = 1.8751_f64.powi(2) / (2.0 * std::f64::consts::PI)
        * (e_mpa * inertia / (rho * W * T * LEN.powi(4))).sqrt();
    let AnalysisReport::Frequency(freq) = analysis(&mut s, "modes") else {
        panic!("bukan frekuensi")
    };
    assert_eq!(freq.frequencies_hz.len(), 3);
    assert!(
        rel(freq.frequencies_hz[0], f1) < 0.05,
        "f1 {} vs {f1}",
        freq.frequencies_hz[0]
    );
    assert!(freq.frequencies_hz.windows(2).all(|p| p[0] <= p[1]));

    // Kolom jepit-bebas: Pcr = π² E I / (4 L²); beban terpasang 500 N.
    let pcr = std::f64::consts::PI.powi(2) * e_mpa * inertia / (4.0 * LEN * LEN);
    let AnalysisReport::Buckling(buck) = analysis(&mut s, "column") else {
        panic!("bukan buckling")
    };
    assert!(
        rel(buck.load_factors[0], pcr / 500.0) < 0.05,
        "faktor {} vs {}",
        buck.load_factors[0],
        pcr / 500.0
    );

    // Batang dengan dua ujung bersuhu tetap: profil linier 20..100 °C.
    let AnalysisReport::Thermal(heat) = analysis(&mut s, "heat") else {
        panic!("bukan termal")
    };
    assert!((heat.max_temperature_c - 100.0).abs() < 1e-6);
    assert!((heat.min_temperature_c - 20.0).abs() < 1e-6);

    // Batang terjepit dua ujung, ΔT seragam 40 K: σ ≈ E α ΔT (efek Poisson
    // di jepitan menaikkan puncaknya).
    let sigma = e_mpa * m.thermal_expansion_per_k * 40.0;
    let StudyOutcome::Stress(hot) = s.run_study_any("clamped_hot", &CancelToken::new()).unwrap()
    else {
        panic!("bukan tegangan")
    };
    assert!(
        hot.max_von_mises_mpa > 0.9 * sigma && hot.max_von_mises_mpa < 2.5 * sigma,
        "{} vs {sigma}",
        hot.max_von_mises_mpa
    );

    let checks = s.run_checks(None);
    assert_eq!(
        (checks.pass, checks.fail, checks.error),
        (4, 0, 0),
        "{:?}",
        checks.results
    );
}

#[test]
fn study_kinds_are_cached_and_deterministic() {
    let mut s = fixture();
    let before = s.run_checks(None);
    assert_eq!(before.error, 4, "belum dijalankan: {:?}", before.results);
    let ran = s.run_all_studies(&CancelToken::new());
    assert_eq!(ran.len(), 4);
    for (id, r) in &ran {
        assert!(r.is_ok(), "{id}: {:?}", r.as_ref().err());
    }
    let first = serde_json::to_string(&analysis(&mut s, "modes")).unwrap();
    let again = serde_json::to_string(&analysis(&mut fixture(), "modes")).unwrap();
    assert_eq!(first, again, "replay dari nol harus identik");

    // Mengubah model membuat hasil basi.
    assert!(
        s.set_params(
            [("len".to_string(), 120.0)]
                .into_iter()
                .chain([("w".to_string(), 10.0), ("t".to_string(), 5.0)])
                .collect()
        )
        .unwrap()
        .committed
    );
    let stale = s.run_checks(None);
    assert_eq!(stale.error, 4, "{:?}", stale.results);
    // Replay bisa membuang cache ("belum dijalankan") atau menandainya basi.
    let msg = &stale.results[0].message;
    assert!(
        msg.contains("basi") || msg.contains("belum dijalankan"),
        "{msg}"
    );
}

#[test]
fn study_kind_fields_are_validated() {
    let base = r#"{"op":"primitive","id":"b","shape":{"box":{"size":[20,10,5]}}}"#;
    let run = |study: &str| {
        let mut s = Session::new();
        let ops: Vec<Op> = serde_json::from_str(&format!("[{base},{study}]")).unwrap();
        s.run(ops, false)
    };
    let setup =
        r#""setup":{"body":"b","fixtures":[{"id":"f","faces":"<X","kind":"fixed"}],"loads":[]}"#;
    // Termal tanpa syarat batas.
    let r = run(&format!(
        r#"{{"op":"study","id":"s","kind":"thermal",{setup}}}"#
    ));
    let e = r.error.unwrap();
    assert_eq!(e.code, OpErrorCode::InvalidParam);
    assert!(e.message.contains("thermal"), "{}", e.message);
    // `thermal` pada studi statik.
    let th =
        r#""thermal":{"boundary":[{"id":"h","faces":">X","kind":"temperature","celsius":50}]}"#;
    let r = run(&format!(r#"{{"op":"study","id":"s",{setup},{th}}}"#));
    assert_eq!(r.error.unwrap().code, OpErrorCode::InvalidParam);
    // Hanya fluks panas → tidak ada keadaan tunak.
    let flux =
        r#""thermal":{"boundary":[{"id":"h","faces":">X","kind":"heat_flux","w_per_mm2":0.01}]}"#;
    let r = run(&format!(
        r#"{{"op":"study","id":"s","kind":"thermal",{setup},{flux}}}"#
    ));
    assert!(r.error.unwrap().message.contains("steady state"));
    // `modes` di luar rentang dan pada jenis yang salah.
    let r = run(&format!(
        r#"{{"op":"study","id":"s","kind":"frequency","modes":0,{setup}}}"#
    ));
    assert_eq!(r.error.unwrap().code, OpErrorCode::InvalidParam);
    let r = run(&format!(r#"{{"op":"study","id":"s","modes":3,{setup}}}"#));
    assert_eq!(r.error.unwrap().code, OpErrorCode::InvalidParam);
    // Studi frekuensi tidak punya medan tegangan.
    let mut s = Session::new();
    let ops: Vec<Op> = serde_json::from_str(&format!(
        r#"[{base},{{"op":"set_material","id":"m","body":"b","material":"s235"}},{{"op":"study","id":"s","kind":"frequency",{setup}}}]"#
    ))
    .unwrap();
    assert!(s.run(ops, false).committed);
    let e = s.run_study("s", &CancelToken::new()).unwrap_err();
    assert!(e.message.contains("frequency"), "{}", e.message);
}
