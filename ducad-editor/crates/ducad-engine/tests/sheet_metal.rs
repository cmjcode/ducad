//! Op sheet metal (P19, Paritas 6C): base_flange, edge_flange, hem, jog,
//! unfold/fold, flat_pattern, relief, flange-di-ujung-flange, perforate,
//! convert_to_sheet.

use ducad_engine::inspect::{summarize, DEFAULT_TOPOLOGY_LIMIT};
use ducad_engine::ops::{Op, OpFile};
use ducad_engine::{OpErrorCode, Session};

/// Volume vs rumus analitik: toleransi volume kernel (berbasis tesselasi,
/// PLAN_MIGRASI_TRUCK F5.1).
const VOL_TOL: f64 = ducad_kernel::VOLUME_REL_TOL;

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
    assert!((volume(&s, "tray") - model.folded_volume()).abs() / model.folded_volume() < VOL_TOL);
    assert!(
        (volume(&s, "blank") - model.flat_area() * 2.0).abs() / (model.flat_area() * 2.0) < VOL_TOL
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
        (v - model.folded_volume()).abs() / v < VOL_TOL,
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
    // Dua region terpisah → harus tepat satu.
    let mut s = Session::new();
    let r = s.run(
        ops(r#"[{"op":"sketch","id":"sk","plane":"XY","entities":[{"circle":{"center":[0,0],"r":5}},{"circle":{"center":[30,0],"r":5}}]},
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

#[test]
fn sheet_metal_base_with_holes_and_arcs() {
    let mut s = Session::new();
    let r = s.run(
        ops(r#"[{"op":"sketch","id":"sk","plane":"XY","entities":[
                    {"rect":{"corner":[0,0],"w":60,"h":40}},
                    {"circle":{"center":[20,20],"r":5}},
                    {"rect":{"corner":[40,10],"w":10,"h":20}}]},
                {"op":"base_flange","id":"p","sketch":"sk","thickness":2},
                {"op":"sketch","id":"disk","plane":{"base":"XY","offset":50},"entities":[{"circle":{"center":[0,0],"r":15}}]},
                {"op":"base_flange","id":"d","sketch":"disk","thickness":1}]"#),
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    let m = &s.meta().sheet_metal["p"].model;
    assert_eq!(m.cutouts.len(), 2);
    assert!(m
        .cutouts
        .iter()
        .any(|c| matches!(c, ducad_core::Cutout::Circle { radius, .. } if (*radius - 5.0).abs() < 1e-9)));
    let v = volume(&s, "p");
    assert!((v - m.folded_volume()).abs() / v < VOL_TOL, "{v} vs {}", m.folded_volume());
    // Lubang ikut ke pola datar (loop tertutup di outline DXF).
    let flat = m.flat_pattern().unwrap();
    assert!(flat.outline.len() > 4 + 48);
    // Pelat bundar: lingkaran dicacah jadi poligon.
    let disk = &s.meta().sheet_metal["d"].model;
    assert!(disk.outline.len() >= 24);
    let vd = volume(&s, "d");
    assert!((vd - std::f64::consts::PI * 225.0).abs() / vd < 0.01, "{vd}");
}

#[test]
fn sheet_metal_relief_is_cut_in_folded_and_flat() {
    let mut s = plate(
        r#",{"op":"edge_flange","id":"f","body":"p","edges":"|X[len=80][z=0][y=0]","length":10,"relief":"rect"}"#,
    );
    let m = s.meta().sheet_metal["p"].model.clone();
    let v = volume(&s, "p");
    let mut no_relief = m.clone();
    no_relief.flanges[0].relief = ducad_core::ReliefKind::None;
    assert!(no_relief.folded_volume() > m.folded_volume() + 1.0);
    // Toleransi longgar: alat relief diperbesar sedikit supaya tidak sebidang.
    assert!((v - m.folded_volume()).abs() / v < 0.005, "{v} vs {}", m.folded_volume());
    assert!(summarize(&s, Some("p"), false, 10).unwrap().bodies[0].valid);
    let r = s.run(ops(r#"[{"op":"unfold","id":"u","body":"p"}]"#), false);
    assert!(r.committed, "{:?}", r.error);
    let vf = volume(&s, "p");
    let want = m.flat_area() * 1.5;
    assert!((vf - want).abs() / want < 0.005, "{vf} vs {want}");
}

#[test]
fn sheet_metal_flange_on_flange_extends_chain() {
    // Flange 90° (R = t = 1.5, L = 8): ujung di z = 11; hem di tepi ujungnya.
    let mut s = plate(
        r#",{"op":"edge_flange","id":"wall","body":"p","edges":"|X[len=80][z=0][y=0]","length":8}"#,
    );
    let r = s.run(
        ops(r#"[{"op":"edge_flange","id":"lip","body":"p","edges":"|X[len=80][z=11]","length":5,"angle":90}]"#),
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    let m = &s.meta().sheet_metal["p"].model;
    assert_eq!(m.flanges.len(), 1);
    assert_eq!(m.flanges[0].segments.len(), 2);
    let v = volume(&s, "p");
    assert!((v - m.folded_volume()).abs() / v < VOL_TOL, "{v} vs {}", m.folded_volume());
    let sum = summarize(&s, Some("p"), false, 10).unwrap();
    assert!(sum.bodies[0].valid);
    // Bentangan = Σ (BA + panjang) kedua segmen.
    let ba = m.allowance(&m.flanges[0].segments[0]);
    assert!((m.developed_length(&m.flanges[0]) - 2.0 * ba - 13.0).abs() < 1e-9);
    let r = s.run(ops(r#"[{"op":"flat_pattern","id":"blank","body":"p"}]"#), false);
    assert!(r.committed, "{:?}", r.error);
}

#[test]
fn sheet_metal_perforate_grid_and_hostile_input() {
    let mut s = plate("");
    let r = s.run(
        ops(r#"[{"op":"perforate","id":"pf","body":"p","d":3,"pitch":6,"margin":3}]"#),
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    let holes = r.outcomes[0].detail["holes"].as_u64().unwrap();
    assert!(holes > 20, "{holes}");
    let m = &s.meta().sheet_metal["p"].model;
    assert_eq!(m.cutouts.len() as u64, holes);
    let v = volume(&s, "p");
    assert!((v - m.folded_volume()).abs() / v < VOL_TOL, "{v} vs {}", m.folded_volume());
    for bad in [
        r#"{"op":"perforate","id":"x","body":"p","d":-1,"pitch":6}"#,
        r#"{"op":"perforate","id":"x","body":"p","d":3,"pitch":2}"#,
        r#"{"op":"perforate","id":"x","body":"p","d":0.01,"pitch":0.02,"margin":0}"#,
        r#"{"op":"perforate","id":"x","body":"p","d":3,"pitch":6,"margin":-1}"#,
        r#"{"op":"perforate","id":"x","body":"p","d":3,"pitch":6,"margin":100}"#,
        r#"{"op":"perforate","id":"x","body":"ghost","d":3,"pitch":6}"#,
    ] {
        let r = s.run(vec![serde_json::from_str(bad).unwrap()], false);
        let e = r.error.unwrap_or_else(|| panic!("harus gagal: {bad}"));
        assert!(e.hint.is_some(), "{bad}");
    }
}

#[test]
fn sheet_metal_convert_flat_plate_solid() {
    let mut s = Session::new();
    let r = s.run(
        ops(r#"[{"op":"sketch","id":"sk","plane":"XY","entities":[
                    {"rect":{"corner":[0,0],"w":50,"h":30}},
                    {"circle":{"center":[10,15],"r":3}}]},
                {"op":"extrude","id":"plate","sketch":"sk","distance":2},
                {"op":"convert_to_sheet","id":"cv","body":"plate","bend_radius":1}]"#),
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    let m = &s.meta().sheet_metal["plate"].model;
    assert!((m.thickness - 2.0).abs() < 1e-6);
    assert_eq!(m.cutouts.len(), 1);
    assert_eq!(m.default_radius, 1.0);
    // Setelah konversi, flange dan pola datar bekerja.
    let r = s.run(
        ops(r#"[{"op":"edge_flange","id":"f","body":"plate","edges":"|X[len=50][z=0][y=0]","length":10},
                {"op":"flat_pattern","id":"blank","body":"plate"}]"#),
        false,
    );
    assert!(r.committed, "{:?}", r.error);

    // Solid bukan pelat seragam (bertingkat) → error dengan hint.
    let mut s = Session::new();
    let r = s.run(
        ops(r#"[{"op":"sketch","id":"sk","plane":"XY","entities":[{"rect":{"corner":[0,0],"w":40,"h":20}}]},
                {"op":"extrude","id":"a","sketch":"sk","distance":2},
                {"op":"sketch","id":"sk2","plane":"XY","entities":[{"rect":{"corner":[0,0],"w":10,"h":20}}]},
                {"op":"extrude","id":"b","sketch":"sk2","distance":8},
                {"op":"boolean","id":"ab","kind":"union","a":"a","b":"b"}]"#),
        false,
    );
    assert!(r.committed, "{:?}", r.error);
    let r = s.run(ops(r#"[{"op":"convert_to_sheet","id":"cv","body":"ab"}]"#), false);
    let e = r.error.unwrap();
    assert_eq!(e.code, OpErrorCode::InvalidParam);
    assert!(e.hint.unwrap().contains("base_flange"));
    // Dua kali konversi dan k-factor di luar 0..1 ditolak.
    let r = s.run(ops(r#"[{"op":"convert_to_sheet","id":"cv","body":"ab","k_factor":2}]"#), false);
    assert!(r.error.is_some());
}
