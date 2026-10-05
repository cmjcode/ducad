//! Konfigurasi varian (P19): penimpaan param, op tersuppress, material.

use ducad_core::Configuration;
use ducad_engine::inspect::{summarize, DEFAULT_TOPOLOGY_LIMIT};
use ducad_engine::ops::OpFile;
use ducad_engine::{OpErrorCode, Session};

fn bracket() -> Session {
    let f: OpFile = serde_json::from_str(include_str!("fixtures/bracket.ops.json")).unwrap();
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    let r = s.run(f.ops, false);
    assert!(r.committed, "{:?}", r.error);
    s
}

fn volume(s: &Session) -> f64 {
    s.body("bracket").unwrap().1.shape.volume().abs()
}

fn cylinders(s: &Session) -> usize {
    let sum = summarize(s, Some("bracket"), false, DEFAULT_TOPOLOGY_LIMIT).unwrap();
    sum.bodies[0]
        .face_kinds
        .get("cylinder")
        .copied()
        .unwrap_or(0)
}

fn config(name: &str, params: &[(&str, f64)], suppressed: &[&str]) -> Configuration {
    let mut c = Configuration::named(name);
    c.params = params.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    c.suppressed_ops = suppressed.iter().map(|s| s.to_string()).collect();
    c
}

#[test]
fn configurations_three_variants_differ_as_specified() {
    let mut s = bracket();
    // Berkas tanpa konfigurasi → satu "Default".
    let names: Vec<String> = s.configurations().into_iter().map(|c| c.name).collect();
    assert_eq!(names, ["Default"]);
    assert_eq!(s.active_configuration(), "Default");
    let base = volume(&s);
    let base_cyl = cylinders(&s);
    assert!(base_cyl >= 2, "bracket dasar punya 2 lubang + fillet");

    let r = s
        .set_configurations(vec![
            config("long", &[("len", 80.0)], &[]),
            config("tall", &[("depth", 40.0), ("wall_h", 40.0)], &[]),
            config("plain", &[], &["holes", "fold"]),
        ])
        .unwrap();
    assert!(r.committed, "{:?}", r.error);
    assert_eq!(s.configurations().len(), 4);
    assert!(
        (volume(&s) - base).abs() < 1e-6,
        "mendaftarkan konfigurasi tidak mengubah model"
    );

    assert!(s.activate_configuration("long").unwrap().committed);
    assert_eq!(s.active_configuration(), "long");
    let long = volume(&s);
    // Semua dimensi sepanjang X berskala len: 80/50.
    assert!(long > base * 1.5 && long < base * 1.7, "{long} vs {base}");
    let sum = summarize(&s, None, false, 10).unwrap();
    assert_eq!(sum.params["len"], 80.0, "inspect melaporkan param efektif");
    assert_eq!(sum.active_configuration.as_deref(), Some("long"));
    assert_eq!(sum.bodies[0].size[0], 80.0);
    // Param dasar tidak tersentuh.
    assert_eq!(s.design().params["len"], 50.0);

    assert!(s.activate_configuration("tall").unwrap().committed);
    let tall = volume(&s);
    assert!(tall > base * 1.2, "{tall} vs {base}");
    assert!((tall - long).abs() > 1.0);

    assert!(s.activate_configuration("plain").unwrap().committed);
    // Lubang dan fillet tersuppress: tidak ada face silinder, volume bertambah
    // sebesar lubang, dan op tetap ada di oplog.
    assert_eq!(cylinders(&s), 0);
    assert!(volume(&s) > base);
    assert_eq!(s.design().oplog.len(), 6);
    let sum = summarize(&s, None, false, 10).unwrap();
    assert!(sum
        .configurations
        .iter()
        .any(|c| c.name == "plain" && c.active));

    assert!(s.activate_configuration("Default").unwrap().committed);
    assert!((volume(&s) - base).abs() < 1e-6);
    assert_eq!(cylinders(&s), base_cyl);
}

#[test]
fn configurations_reject_bad_input_and_keep_session_intact() {
    let mut s = bracket();
    let base = volume(&s);
    assert_eq!(
        s.activate_configuration("ghost").unwrap_err().code,
        OpErrorCode::UnknownRef
    );
    assert_eq!(
        s.set_configurations(vec![config("x", &[], &["no_such_op"])])
            .unwrap_err()
            .code,
        OpErrorCode::UnknownRef
    );
    for bad in ["Default", "a/b", ""] {
        assert_eq!(
            s.set_configurations(vec![config(bad, &[], &[])])
                .unwrap_err()
                .code,
            OpErrorCode::InvalidParam,
            "{bad:?}"
        );
    }
    assert_eq!(
        s.set_configurations(vec![config("a", &[], &[]), config("a", &[], &[])])
            .unwrap_err()
            .code,
        OpErrorCode::DuplicateId
    );

    // Suppress op yang masih dirujuk op berikutnya → aktivasi gagal, sesi utuh.
    assert!(
        s.set_configurations(vec![config("broken", &[], &["wall"])])
            .unwrap()
            .committed
    );
    let r = s.activate_configuration("broken").unwrap();
    assert!(!r.committed);
    assert_eq!(r.error.unwrap().op_id.as_deref(), Some("bracket"));
    assert_eq!(s.active_configuration(), "Default");
    assert!((volume(&s) - base).abs() < 1e-6);
}

#[test]
fn configurations_survive_save_and_load_with_material_override() {
    let mut s = bracket();
    let mut steel = config("steel", &[("len", 60.0)], &[]);
    steel.material_overrides.insert(
        "bracket".into(),
        ducad_core::MaterialSource::Library("s235".into()),
    );
    assert!(s.set_configurations(vec![steel]).unwrap().committed);
    assert!(s.activate_configuration("steel").unwrap().committed);
    let v = volume(&s);
    let mass = |s: &Session| summarize(s, Some("bracket"), false, 10).unwrap().bodies[0].mass_g;
    assert!((mass(&s).unwrap() - v / 1000.0 * 7.85).abs() < 1e-3);

    let path = std::env::temp_dir().join(format!("ducad-engine-cfg-{}.ducad", std::process::id()));
    s.save(&path).unwrap();
    let loaded = Session::from_file(&path).unwrap();
    std::fs::remove_file(&path).ok();
    assert_eq!(loaded.active_configuration(), "steel");
    assert_eq!(loaded.configurations().len(), 2);
    assert!((volume(&loaded) - v).abs() / v < 1e-6);
    assert_eq!(loaded.design().params["len"], 50.0);
    assert!((mass(&loaded).unwrap() - v / 1000.0 * 7.85).abs() < 1e-3);
}

#[test]
fn configurations_design_table_and_param_merge() {
    let mut s = bracket();
    let r = s
        .import_design_table(
            "configuration,len,wall_h,suppressed_ops\nshort,45,,\nheavy,60,40,holes\n",
        )
        .unwrap();
    assert!(r.committed, "{:?}", r.error);
    assert_eq!(
        s.design_table_csv(),
        "configuration,len,wall_h,suppressed_ops\nshort,45,,\nheavy,60,40,holes\n"
    );
    assert_eq!(
        s.import_design_table("configuration,len\nx,abc\n")
            .unwrap_err()
            .code,
        OpErrorCode::InvalidParam
    );

    // `set_params {configuration}`: gabung ke konfigurasi lalu aktifkan.
    let r = s
        .set_configuration_params(
            "short",
            [("wall_h".to_string(), 20.0)].into_iter().collect(),
        )
        .unwrap();
    assert!(r.committed);
    assert_eq!(s.active_configuration(), "short");
    let short = &s.design().configurations[0];
    assert_eq!((short.params["len"], short.params["wall_h"]), (45.0, 20.0));
    // Konfigurasi baru dibuat bila belum ada; "Default" menulis ke param dasar.
    assert!(
        s.set_configuration_params("fresh", [("r".to_string(), 1.0)].into_iter().collect())
            .unwrap()
            .committed
    );
    assert_eq!(s.configurations().len(), 4);
    assert!(
        s.set_configuration_params("Default", [("r".to_string(), 1.5)].into_iter().collect())
            .unwrap()
            .committed
    );
    assert_eq!(s.active_configuration(), "Default");
    assert_eq!(s.design().params["r"], 1.5);

    // OpFile dengan konfigurasi + material kunci pustaka.
    let specs: Vec<ducad_engine::ops::ConfigurationSpec> = serde_json::from_str(
        r#"[{"name":"alu","params":{"len":45},"material_overrides":{"bracket":"al_6061_t6"}}]"#,
    )
    .unwrap();
    let r = s
        .apply_configuration_specs(&specs, Some("alu"))
        .unwrap()
        .unwrap();
    assert!(r.committed, "{:?}", r.error);
    assert_eq!(s.active_configuration(), "alu");
    let bad: Vec<ducad_engine::ops::ConfigurationSpec> =
        serde_json::from_str(r#"[{"name":"x","material_overrides":{"bracket":"unobtainium"}}]"#)
            .unwrap();
    assert_eq!(
        s.apply_configuration_specs(&bad, None).unwrap_err().code,
        OpErrorCode::InvalidParam
    );
}
