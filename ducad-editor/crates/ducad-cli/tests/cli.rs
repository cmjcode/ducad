//! Tes integrasi `ducad-cli` (binary sungguhan).

use std::f64::consts::PI;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const PLATE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../ducad-engine/tests/fixtures/plate.ops.json"
);

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ducad-cli"))
        .args(args)
        .output()
        .unwrap()
}

fn tmpdir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ducad-cli-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

fn plate_volume(t: f64) -> f64 {
    60.0 * 40.0 * t - 4.0 * (1.0 - PI / 4.0) * 9.0 * t - 4.0 * PI * 2.75 * 2.75 * t
}

fn stdout_json(o: &Output) -> serde_json::Value {
    serde_json::from_slice(&o.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout bukan JSON ({e}): {}",
            String::from_utf8_lossy(&o.stdout)
        )
    })
}

#[test]
fn run_inspect_replay_flow() {
    let d = tmpdir("flow");
    let (part, step, png) = (d.join("t.ducad"), d.join("t.step"), d.join("t.png"));
    let o = cli(&[
        "run",
        PLATE,
        "--out",
        s(&part),
        "--export",
        &format!("step:{}", s(&step)),
        "--export",
        &format!("png:{}", s(&png)),
    ]);
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert_eq!(stdout_json(&o)["committed"], true);
    assert!(part.exists() && step.exists());
    assert_eq!(&std::fs::read(&png).unwrap()[..8], b"\x89PNG\r\n\x1a\n");

    let o = cli(&["inspect", s(&part), "--json"]);
    assert_eq!(o.status.code(), Some(0));
    let v = stdout_json(&o)["bodies"][0]["volume"].as_f64().unwrap();
    assert!(
        (v - plate_volume(8.0)).abs() / plate_volume(8.0) < 1e-3,
        "{v}"
    );

    let o = cli(&["inspect", s(&part), "--mass"]);
    assert_eq!(o.status.code(), Some(0));
    let table = String::from_utf8_lossy(&o.stdout).to_string();
    for label in ["massa", "pusat massa", "inersia @COM", "momen utama 3", "radius girasi"] {
        assert!(table.contains(label), "tabel --mass tanpa '{label}':\n{table}");
    }

    let part2 = d.join("t2.ducad");
    let o = cli(&["replay", s(&part), "--param", "t=10", "--out", s(&part2)]);
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let o = cli(&["inspect", s(&part2), "--json"]);
    let v10 = stdout_json(&o)["bodies"][0]["volume"].as_f64().unwrap();
    assert!(
        (v10 - plate_volume(10.0)).abs() / plate_volume(10.0) < 1e-3,
        "{v10}"
    );

    let o = cli(&[
        "select",
        s(&part),
        "--body",
        "plate",
        "--edges",
        "all[kind=circle]",
        "--json",
    ]);
    assert_eq!(o.status.code(), Some(0));
    // 4 lubang × 2 lingkaran + 4 fillet × 2 busur (atas/bawah).
    assert_eq!(stdout_json(&o)["count"], 16);

    let obj = d.join("t.obj");
    let o = cli(&["export", s(&part), "--format", "obj", "--out", s(&obj)]);
    assert_eq!(o.status.code(), Some(0));
    assert!(stdout_json(&o)["bytes"].as_u64().unwrap() > 0);

    let svg = d.join("top.svg");
    let o = cli(&["render", s(&part), "--view", "top", "--out", s(&svg)]);
    assert_eq!(o.status.code(), Some(0));
    assert!(std::fs::read_to_string(&svg).unwrap().starts_with("<svg"));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn failing_ops_exit_1_with_report() {
    let d = tmpdir("fail");
    let ops = d.join("bad.ops.json");
    std::fs::write(
        &ops,
        r#"{"ops":[{"op":"primitive","id":"b","shape":{"box":{"size":[10,10,10]}}},
                   {"op":"fillet","id":"f","body":"b","edges":"|Z","radius":100}]}"#,
    )
    .unwrap();
    let out = d.join("never.ducad");
    let o = cli(&["run", s(&ops), "--out", s(&out)]);
    assert_eq!(o.status.code(), Some(1));
    let j = stdout_json(&o);
    assert_eq!(j["committed"], false);
    assert!(j["error"]["code"].is_string());
    assert!(!out.exists(), "--out tidak boleh ditulis bila gagal");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn schema_and_usage_errors() {
    let o = cli(&["schema"]);
    assert_eq!(o.status.code(), Some(0));
    let text = String::from_utf8(o.stdout).unwrap();
    serde_json::from_str::<serde_json::Value>(&text).unwrap();
    assert!(text.contains("\"extrude\""));

    assert_eq!(cli(&["run"]).status.code(), Some(2));
    assert_eq!(cli(&["bukan-perintah"]).status.code(), Some(2));
    assert_eq!(cli(&["run", "/tidak/ada.json"]).status.code(), Some(2));
    assert_eq!(cli(&["run", PLATE, "--param", "t"]).status.code(), Some(2));
}

#[test]
fn check_command_exit_codes() {
    let d = tmpdir("check");
    let part = d.join("p.ducad");
    assert_eq!(
        cli(&["run", PLATE, "--out", s(&part)]).status.code(),
        Some(0)
    );
    let good = d.join("good.json");
    std::fs::write(
        &good,
        r#"{"id":"t","checks":[{"check":"body_count","expect":1},{"check":"hole_count","body":"*","diameter":5.5,"expect":4}]}"#,
    )
    .unwrap();
    let o = cli(&["check", s(&part), "--checks", s(&good), "--json"]);
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert_eq!(stdout_json(&o)["pass"], 2);
    let bad = d.join("bad.json");
    std::fs::write(&bad, r#"[{"check":"body_count","expect":3}]"#).unwrap();
    assert_eq!(
        cli(&["check", s(&part), "--checks", s(&bad)]).status.code(),
        Some(3)
    );
    assert_eq!(
        cli(&["check", s(&part)]).status.code(),
        Some(2),
        "tanpa check sama sekali"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn oplog_is_git_friendly() {
    let d = tmpdir("oplog");
    let part = d.join("p.ducad");
    assert_eq!(
        cli(&["run", PLATE, "--out", s(&part)]).status.code(),
        Some(0)
    );
    let o = cli(&["oplog", s(&part)]);
    assert_eq!(o.status.code(), Some(0));
    let text = String::from_utf8(o.stdout).unwrap();
    assert_eq!(
        text.lines()
            .filter(|l| l.trim_start().starts_with("{\"op\":"))
            .count(),
        4
    );
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["params"]["t"], 8.0);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn diff_exit_codes_and_json() {
    let d = tmpdir("diff");
    let (a, b) = (d.join("a.ducad"), d.join("b.ducad"));
    assert_eq!(cli(&["run", PLATE, "--out", s(&a)]).status.code(), Some(0));
    assert_eq!(
        cli(&["replay", s(&a), "--param", "t=10", "--out", s(&b)])
            .status
            .code(),
        Some(0)
    );
    assert_eq!(cli(&["diff", s(&a), s(&a)]).status.code(), Some(0));
    let o = cli(&["diff", s(&a), s(&b), "--json", "--no-geometry"]);
    assert_eq!(o.status.code(), Some(1));
    let j = stdout_json(&o);
    assert_eq!(j["params"][0]["name"], "t");
    assert_eq!(j["bodies"][0]["status"], "changed");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn diff_svg_output() {
    let d = tmpdir("diffsvg");
    let (a, b, svg) = (d.join("a.ducad"), d.join("b.ducad"), d.join("d.svg"));
    assert_eq!(cli(&["run", PLATE, "--out", s(&a)]).status.code(), Some(0));
    assert_eq!(
        cli(&["replay", s(&a), "--param", "t=10", "--out", s(&b)])
            .status
            .code(),
        Some(0)
    );
    let o = cli(&["diff", s(&a), s(&b), "--svg", s(&svg)]);
    assert_eq!(
        o.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let text = std::fs::read_to_string(&svg).unwrap();
    assert!(
        text.contains("#16a34a"),
        "lapisan volume bertambah berwarna hijau"
    );
    let _ = std::fs::remove_dir_all(&d);
}

fn fnv(path: &Path) -> u64 {
    std::fs::read(path)
        .unwrap()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
        })
}

#[test]
fn build_default_artifacts_deterministic() {
    let d = tmpdir("build");
    let (a, b) = (d.join("a"), d.join("b"));
    for out in [&a, &b] {
        let o = cli(&["build", PLATE, "--out", s(out), "--date", "2026-01-02"]);
        assert_eq!(
            o.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&o.stderr)
        );
    }
    for f in [
        "plate.step",
        "plate.stl",
        "plate-drawing.pdf",
        "plate-iso.png",
        "plate-bom.csv",
        "report.json",
        "report.md",
    ] {
        assert!(a.join(f).is_file(), "{f} tidak ada");
    }
    for f in ["plate.step", "plate.stl", "plate-drawing.pdf", "plate-bom.csv"] {
        assert_eq!(fnv(&a.join(f)), fnv(&b.join(f)), "{f} tidak deterministik");
    }
    let bom = std::fs::read_to_string(a.join("plate-bom.csv")).unwrap();
    assert_eq!(
        bom.lines().next(),
        Some("item,part,qty,material,volume_mm3,mass_g,file,standard")
    );
    assert_eq!(bom.lines().count(), 2, "{bom}");
    let step = std::fs::read_to_string(a.join("plate.step")).unwrap();
    assert!(step.contains("'2026-01-02T00:00:00'"));
    let md = std::fs::read_to_string(a.join("report.md")).unwrap();
    assert!(md.lines().count() <= 60, "{md}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn build_failed_check_exits_3_without_artifacts() {
    let d = tmpdir("buildfail");
    let mut file: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(PLATE).unwrap()).unwrap();
    file["checks"] = serde_json::json!([{"check":"body_count","expect":2}]);
    let ops = d.join("bad.ops.json");
    std::fs::write(&ops, file.to_string()).unwrap();
    let out = d.join("out");
    let o = cli(&["build", s(&ops), "--out", s(&out)]);
    assert_eq!(
        o.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(!out.join("bad.step").exists());
    assert!(out.join("report.json").is_file());
    assert!(out.join("report.md").is_file());
    let _ = std::fs::remove_dir_all(&d);
}

/// P17: `build` menjalankan studi, menulis `sim/*`, dan gagal (kode 3)
/// bila check tegangan dilanggar; `sim` mencetak ringkasan studi.
#[test]
fn build_with_study_writes_sim_artifacts() {
    const SIM: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ducad-engine/tests/fixtures/sim_bracket.ops.json"
    );
    let d = tmpdir("buildsim");
    let (a, b) = (d.join("a"), d.join("b"));
    for out in [&a, &b] {
        let o = cli(&["build", SIM, "--out", s(out), "--formats", "step", "--date", "2026-01-02"]);
        assert_eq!(
            o.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&o.stderr)
        );
    }
    let r: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(a.join("report.json")).unwrap()).unwrap();
    assert_eq!(r["sim"][0]["id"], "tip_load");
    assert_eq!(r["sim"][0]["status"], "ok");
    assert!(r["sim"][0]["max_von_mises_mpa"].as_f64().unwrap() > 1.0);
    assert_eq!(r["checks"]["pass"], 3, "{r}");
    let listed: Vec<&str> = r["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|x| x["file"].as_str())
        .collect();
    assert!(listed.contains(&"sim/tip_load.json") && listed.contains(&"sim/tip_load-stress.png"));
    assert_eq!(
        &std::fs::read(a.join("sim/tip_load-stress.png")).unwrap()[..8],
        b"\x89PNG\r\n\x1a\n"
    );
    for f in ["sim/tip_load.json", "sim/tip_load-stress.png"] {
        assert_eq!(fnv(&a.join(f)), fnv(&b.join(f)), "{f} tidak deterministik");
    }
    let md = std::fs::read_to_string(a.join("report.md")).unwrap();
    assert!(md.contains("## Simulasi") && md.contains("sim/tip_load-stress.png"), "{md}");

    // Check tegangan yang mustahil dipenuhi → kode 3, laporan tetap ditulis.
    let mut file: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(SIM).unwrap()).unwrap();
    file["checks"] = serde_json::json!([{"check":"max_stress","study":"tip_load","max_mpa":0.5}]);
    let strict = d.join("strict.ops.json");
    std::fs::write(&strict, file.to_string()).unwrap();
    let out = d.join("strict");
    let o = cli(&["build", s(&strict), "--out", s(&out)]);
    assert_eq!(o.status.code(), Some(3), "{}", String::from_utf8_lossy(&o.stderr));
    let r: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("report.json")).unwrap()).unwrap();
    assert_eq!(r["status"], "checks_failed");
    assert_eq!(r["sim"][0]["status"], "ok");
    assert!(!out.join("strict.step").exists());

    // `sim` pada berkas .ducad.
    let part = d.join("p.ducad");
    assert_eq!(cli(&["run", SIM, "--out", s(&part)]).status.code(), Some(0));
    let heat = d.join("heat.svg");
    let o = cli(&["sim", s(&part), "--out", s(&heat)]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let text = String::from_utf8_lossy(&o.stdout).to_string();
    assert!(text.contains("von Mises maks") && text.contains("faktor keamanan"), "{text}");
    assert!(std::fs::read_to_string(&heat).unwrap().starts_with("<svg"));
    let o = cli(&["sim", s(&part), "--study", "tip_load", "--json"]);
    assert!(stdout_json(&o)["safety_factor"].as_f64().unwrap() > 2.0);
    assert_ne!(cli(&["sim", s(&part), "--study", "ghost"]).status.code(), Some(0));
    let _ = std::fs::remove_dir_all(&d);
}

/// P19: `build --all-configs` → satu direktori per konfigurasi + matriks,
/// kode 3 bila salah satu konfigurasi gagal check.
#[test]
fn build_all_configs_writes_one_directory_each() {
    const BRACKET: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ducad-engine/tests/fixtures/bracket.ops.json"
    );
    let d = tmpdir("buildcfg");
    let mut file: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(BRACKET).unwrap()).unwrap();
    file["configurations"] = serde_json::json!([
        { "name": "long", "params": { "len": 80 } },
        { "name": "plain", "suppressed_ops": ["holes", "fold"] }
    ]);
    file["checks"] = serde_json::json!([
        { "id": "one_body", "check": "body_count", "expect": 1 },
        { "id": "fits", "check": "bbox_max", "body": "*", "max": [100, 40, 40] }
    ]);
    let ops = d.join("bracket.ops.json");
    std::fs::write(&ops, file.to_string()).unwrap();

    let out = d.join("out");
    let o = cli(&["build", s(&ops), "--out", s(&out), "--all-configs", "--formats", "step,bom"]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let size = |name: &str| std::fs::metadata(out.join(name).join("bracket.step")).unwrap().len();
    for name in ["Default", "long", "plain"] {
        assert!(out.join(name).join("report.json").is_file(), "{name}");
        assert!(out.join(name).join("bracket-bom.csv").is_file(), "{name}");
        assert!(size(name) > 1000);
    }
    assert_ne!(size("Default"), size("plain"), "varian tanpa lubang harus berbeda");
    let top: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("report.json")).unwrap()).unwrap();
    let rows = top["configurations"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|r| r["status"] == "ok"), "{top}");
    let md = std::fs::read_to_string(out.join("report.md")).unwrap();
    assert!(md.contains("| Konfigurasi | Status | one_body | fits |"), "{md}");
    assert!(md.contains("[long](long/report.md)"), "{md}");

    // Satu varian melanggar check (len 120 > 100) → kode 3, varian lain tetap dibangun.
    file["configurations"][0]["params"]["len"] = serde_json::json!(120);
    std::fs::write(&ops, file.to_string()).unwrap();
    let out2 = d.join("out2");
    let o = cli(&["build", s(&ops), "--out", s(&out2), "--all-configs", "--formats", "step"]);
    assert_eq!(o.status.code(), Some(3), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(out2.join("Default/bracket.step").is_file());
    assert!(!out2.join("long/bracket.step").exists());
    let top: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out2.join("report.json")).unwrap()).unwrap();
    assert_eq!(top["configurations"][1]["status"], "checks_failed");

    // `--config` tunggal, nama tak dikenal → salah pakai (kode 2).
    let o = cli(&["build", s(&ops), "--out", s(&d.join("one")), "--config", "plain", "--formats", "step"]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let o = cli(&["build", s(&ops), "--out", s(&d.join("bad")), "--config", "ghost"]);
    assert_eq!(o.status.code(), Some(2));

    // `config`: run → ekspor design table → impor → aktifkan.
    let part = d.join("b.ducad");
    assert_eq!(cli(&["run", s(&ops), "--out", s(&part)]).status.code(), Some(0));
    let csv = d.join("table.csv");
    let o = cli(&["config", s(&part), "--export", s(&csv), "--json"]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(stdout_json(&o)["configurations"].as_array().unwrap().len(), 3);
    let table = std::fs::read_to_string(&csv).unwrap();
    assert!(table.starts_with("configuration,len,suppressed_ops\n"), "{table}");
    std::fs::write(&csv, "configuration,len\nshort,45\n").unwrap();
    let part2 = d.join("b2.ducad");
    let o = cli(&["config", s(&part), "--import", s(&csv), "--activate", "short", "--out", s(&part2)]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let o = cli(&["inspect", s(&part2), "--json"]);
    let sum = stdout_json(&o);
    assert_eq!(sum["active_configuration"], "short");
    assert_eq!(sum["bodies"][0]["size"][0], 45.0);
    assert_eq!(cli(&["config", s(&part), "--import", s(&csv)]).status.code(), Some(2));
    let _ = std::fs::remove_dir_all(&d);
}

/// P19: `build --formats flat` menulis DXF pola bentangan berlayer.
#[test]
fn build_flat_pattern_dxf_for_sheet_metal() {
    const SHEET: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ducad-engine/tests/fixtures/sheet_box.ops.json"
    );
    let d = tmpdir("buildflat");
    let o = cli(&["build", SHEET, "--out", s(&d), "--formats", "flat,step"]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let dxf = std::fs::read_to_string(d.join("sheet_box-tray-flat.dxf")).unwrap();
    assert!(dxf.contains("BEND_UP") && dxf.contains("OUTLINE"), "{}", &dxf[..200.min(dxf.len())]);
    // 4 strip × 3 sisi luar + 4 garis tekuk.
    assert_eq!(dxf.matches("\n0\nLINE\n").count(), 16);
    let r: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(d.join("report.json")).unwrap()).unwrap();
    assert_eq!(r["checks"]["pass"], 3, "{r}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn build_ducad_replays() {
    let d = tmpdir("buildpart");
    let part = d.join("p.ducad");
    assert_eq!(cli(&["run", PLATE, "--out", s(&part)]).status.code(), Some(0));
    let out = d.join("out");
    let o = cli(&["build", s(&part), "--out", s(&out), "--formats", "step,bom"]);
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let r: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("report.json")).unwrap()).unwrap();
    assert_eq!(r["load"], "replay");
    assert_eq!(r["artifacts"].as_array().unwrap().len(), 2);
    let _ = std::fs::remove_dir_all(&d);
}
