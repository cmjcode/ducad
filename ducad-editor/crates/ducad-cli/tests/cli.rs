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
