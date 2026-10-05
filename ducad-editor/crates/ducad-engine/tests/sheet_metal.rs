//! Op sheet metal (P19): base_flange, edge_flange, hem, jog, unfold/fold,
//! flat_pattern.

use ducad_engine::inspect::{summarize, DEFAULT_TOPOLOGY_LIMIT};
use ducad_engine::ops::{Op, OpFile};
use ducad_engine::{OpErrorCode, Session};

fn ops(json: &str) -> Vec<Op> {
    serde_json::from_str(json).unwrap()
}

fn tray() -> Session {
    let f: OpFile = serde_json::from_str(include_str!("fixtures/sheet_box.ops.json")).unwrap();
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    s.set_checks(f.checks);
    let r = s.run(f.ops, false);
    assert!(r.committed, "{:?}", r.error);
    s
}

fn volume(s: &Session, body: &str) -> f64 {
    s.body(body).unwrap().1.shape.volume().abs()
}

fn plate(extra: &str) -> Session {
    let mut s = Session::new();
    let r = s.run(
        ops(&format!(
            r#"[{{"op":"sketch","id":"sk","plane":"XY","entities":[{{"rect":{{"corner":[0,0],"w":80,"h":40}}}}]}},
                {{"op":"base_flange","id":"p","sketch":"sk","thickness":1.5}}{extra}]"#
        )),
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    s
}

#[test]
fn sheet_metal_box_with_four_flanges_matches_allowance() {
    let s = tray();
    let state = &s.meta().sheet_metal["tray"];
    let model = &state.model;
    assert_eq!(model.flanges.len(), 4);
    // BA = π/2 · (R + k·t) = π/2 · 2.88.
    let ba = std::f64::consts::FRAC_PI_2 * 2.88;
    let sum = summarize(&s, None, false, DEFAULT_TOPOLOGY_LIMIT).unwrap();
    let report = sum.bodies.iter().find(|b| b.name == "tray").unwrap();
    let sheet = report.sheet_metal.as_ref().unwrap();
    for f in &sheet.flanges {
        assert!(
            (f.developed_length - (20.0 + ba)).abs() < 0.01,
            "{}",
            f.developed_length
        );
    }
    // Bentangan = datar + Σ BA + Σ panjang flange, di kedua arah (± 0.01 mm).
    let blank = sum.bodies.iter().find(|b| b.name == "blank").unwrap();
    assert!(
        (blank.size[0] - (100.0 + 2.0 * (20.0 + ba))).abs() < 0.01,
        "{:?}",
        blank.size
    );
    assert!(
        (blank.size[1] - (60.0 + 2.0 * (20.0 + ba))).abs() < 0.01,
        "{:?}",
        blank.size
    );
    assert!((blank.size[2] - 2.0).abs() < 1e-6);
    assert!(blank.valid && report.valid);
    // Solid terlipat = rumus analitik; pola datar = luas × tebal.
    assert!((volume(&s, "tray") - model.folded_volume()).abs() / model.folded_volume() < 1e-6);
    assert!(
        (volume(&s, "blank") - model.flat_area() * 2.0).abs() / (model.flat_area() * 2.0) < 1e-6
    );
    // Dinding berdiri setinggi R + t + panjang lurus.
    assert!((report.size[2] - 24.0).abs() < 1e-6, "{:?}", report.size);
    assert!((report.size[0] - 108.0).abs() < 1e-6, "{:?}", report.size);
    // Tekukan = face silinder sungguhan (dalam + luar per flange).
    assert_eq!(report.face_kinds["cylinder"], 8);
    let checks = s.run_checks(None);
    assert_eq!(
        (checks.pass, checks.fail, checks.error),
        (3, 0, 0),
        "{:?}",
        checks.results
    );
}

#[test]
fn sheet_metal_unfold_then_fold_restores_volume() {
    let mut s = tray();
    let folded = volume(&s, "tray");
    assert!(
        s.run(ops(r#"[{"op":"unfold","id":"open","body":"tray"}]"#), false)
            .committed
    );
    let flat = volume(&s, "tray");
    let model = s.meta().sheet_metal["tray"].model.clone();
    assert!(s.meta().sheet_metal["tray"].unfolded);
    assert!((flat - model.flat_area() * 2.0).abs() / flat < 1e-6);
    // Flange tidak bisa ditambah saat terbentang; unfold dua kali ditolak.
    let r = s.run(
        ops(r#"[{"op":"unfold","id":"again","body":"tray"}]"#),
        false,
    );
    assert_eq!(r.error.unwrap().code, OpErrorCode::InvalidParam);
    assert!(
        s.run(ops(r#"[{"op":"fold","id":"close","body":"tray"}]"#), false)
            .committed
    );
    let back = volume(&s, "tray");
    assert!((back - folded).abs() / folded < 1e-3, "{back} vs {folded}");
    // Undo sesi mengembalikan keadaan terbentang.
    assert!(s.undo().unwrap());
    assert!(s.meta().sheet_metal["tray"].unfolded);
}

#[test]
fn sheet_metal_hem_and_jog_build_valid_solids() {
    let s = plate(
        r#",{"op":"hem","id":"lip","body":"p","edges":"|X[len=80][z=0][y=0]","length":6,"gap":1.5},
           {"op":"jog","id":"step","body":"p","edges":"|X[len=80][z=0][y=40]","offset":5,"length":10,"angle":45}"#,
    );
    let model = &s.meta().sheet_metal["p"].model;
    assert_eq!(model.flanges.len(), 2);
    assert_eq!(model.flanges[0].segments[0].angle_deg, 180.0);
    assert_eq!(model.flanges[0].segments[0].radius, 0.75);
    assert_eq!(model.flanges[1].segments.len(), 2);
    let sum = summarize(&s, Some("p"), false, 10).unwrap();
    assert!(sum.bodies[0].valid);
    let v = volume(&s, "p");
    assert!(
        (v - model.folded_volume()).abs() / v < 1e-6,
        "{v} vs {}",
        model.folded_volume()
    );
    // Jog 5 mm ke atas: tinggi total = offset + tebal; hem menambah di bawah nol? tidak —
    // hem ke atas menumpuk di atas pelat: gap + 2·tebal.
    assert!(
        (sum.bodies[0].size[2] - 6.5).abs() < 1e-6,
        "{:?}",
        sum.bodies[0].size
    );

    // Offset jog lebih kecil dari yang bisa dibuat dua tekukan → error berangka.
    let mut s2 = plate("");
    let r = s2.run(
        ops(r#"[{"op":"jog","id":"j","body":"p","edges":"|X[len=80][z=0][y=0]","offset":0.5,"length":5}]"#),
        false,
    );
    let e = r.error.unwrap();
    assert_eq!(e.code, OpErrorCode::InvalidParam);
    assert!(e.context["min_offset"].as_f64().unwrap() > 0.5);
}

#[test]
fn sheet_metal_rejects_unsupported_input() {
    // Sketsa lingkaran bukan poligon.
    let mut s = Session::new();
    let r = s.run(
        ops(r#"[{"op":"sketch","id":"sk","plane":"XY","entities":[{"circle":{"center":[0,0],"r":20}}]},
                {"op":"base_flange","id":"p","sketch":"sk","thickness":1}]"#),
        false,
    );
    assert_eq!(r.error.unwrap().code, OpErrorCode::InvalidParam);

    // Body biasa bukan sheet metal.
    let mut s = plate("");
    assert!(
        s.run(
            ops(r#"[{"op":"primitive","id":"blk","shape":{"box":{"size":[5,5,5]}}}]"#),
            false
        )
        .committed
    );
    let r = s.run(
        ops(r#"[{"op":"edge_flange","id":"f","body":"blk","edges":"|X","length":5}]"#),
        false,
    );
    let e = r.error.unwrap();
    assert_eq!(e.code, OpErrorCode::InvalidParam);
    assert!(e.hint.unwrap().contains("base_flange"));

    // Sisi yang sama dua kali, dan tepi yang bukan sisi pelat dasar.
    let flange =
        r#"[{"op":"edge_flange","id":"f1","body":"p","edges":"|X[len=80][z=0][y=0]","length":8}]"#;
    assert!(s.run(ops(flange), false).committed);
    let again = flange.replace("f1", "f2").replace("[z=0]", "[z=1.5]");
    // Rim atas sisi itu sudah termakan tekukan → selector kosong atau ditolak.
    let r = s.run(ops(&again), false);
    assert!(matches!(
        r.error.unwrap().code,
        OpErrorCode::InvalidParam | OpErrorCode::SelectorEmpty
    ));
    let r = s.run(
        ops(r#"[{"op":"edge_flange","id":"f3","body":"p","edges":"|Z","length":8}]"#),
        false,
    );
    assert_eq!(r.error.unwrap().code, OpErrorCode::InvalidParam);
    // Sudut di luar domain.
    let r = s.run(
        ops(r#"[{"op":"edge_flange","id":"f4","body":"p","edges":"|X[len=80][z=0][y=40]","length":8,"angle":200}]"#),
        false,
    );
    assert_eq!(r.error.unwrap().code, OpErrorCode::InvalidParam);

    // Check pada body non-pelat → error; radius terlalu kecil → fail.
    let checks: Vec<ducad_engine::check::CheckItem> = serde_json::from_str(
        r#"[{"check":"min_bend_radius","body":"blk","min_ratio_to_t":1},
            {"check":"min_bend_radius","body":"p","min_ratio_to_t":2},
            {"check":"min_flange_length","body":"p","min":4}]"#,
    )
    .unwrap();
    let r = s.run_checks(Some(&checks));
    assert_eq!((r.pass, r.fail, r.error), (1, 1, 1), "{:?}", r.results);
}

#[test]
fn sheet_metal_survives_save_load_and_downward_flange() {
    let mut s = plate(
        r#",{"op":"edge_flange","id":"down","body":"p","edges":"|Y[len=40][z=0][x=0]","length":12,"angle":-90,"radius":3,"relief":"obround"}"#,
    );
    let sum = summarize(&s, Some("p"), false, 10).unwrap();
    // Flange ke bawah: bbox turun sejauh R + panjang di bawah muka bawah.
    assert!(
        (sum.bodies[0].bbox[0][2] + 15.0).abs() < 1e-6,
        "{:?}",
        sum.bodies[0].bbox
    );
    let v = volume(&s, "p");
    let path =
        std::env::temp_dir().join(format!("ducad-engine-sheet-{}.ducad", std::process::id()));
    s.save(&path).unwrap();
    let loaded = Session::from_file(&path).unwrap();
    std::fs::remove_file(&path).ok();
    assert!((volume(&loaded, "p") - v).abs() / v < 1e-6);
    let m = &loaded.meta().sheet_metal["p"].model;
    assert_eq!(m.flanges[0].relief, ducad_core::ReliefKind::Obround);
    let flat = m.flat_pattern().unwrap();
    assert!(!flat.bend_lines[0].up);
    let dxf = ducad_io::flat_dxf::flat_pattern_dxf(&flat);
    assert!(dxf.contains("BEND_DOWN"));
}
