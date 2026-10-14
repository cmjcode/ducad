//! Penjaga pelapisan crate workspace (padanan `cargo xtask layers` di
//! CADCraft). Peta crate di `CLAUDE.md` adalah kontrak: setiap crate hanya
//! boleh bergantung pada crate DUCAD yang tercantum di tabel ini, crate UI
//! (egui/eframe/wgpu/winit/rfd) hanya boleh muncul di lapisan GUI, dan
//! `opencascade` hanya di `ducad-kernel`. Menambah panah baru = ubah tabel
//! ini DAN peta di `CLAUDE.md`.
//!
//! Dijalankan dari `ducad-core` karena crate ini paling bawah dan paling
//! cepat dikompilasi; ia hanya membaca `Cargo.toml` crate lain sebagai teks.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// (crate, crate DUCAD yang boleh menjadi dependensinya — normal, dev, build).
const ALLOWED: &[(&str, &[&str])] = &[
    ("ducad-core", &[]),
    ("ducad-sim", &[]),
    ("ducad-i18n", &[]),
    ("ducad-cloud", &[]),
    ("ducad-chat", &[]),
    ("ducad-glass", &[]),
    ("ducad-sketch", &["ducad-core"]),
    ("ducad-kernel", &["ducad-core"]),
    ("ducad-ink", &["ducad-core", "ducad-sketch"]),
    ("ducad-render", &["ducad-core", "ducad-sketch"]),
    (
        "ducad-io",
        &["ducad-core", "ducad-sketch", "ducad-kernel", "ducad-ink"],
    ),
    (
        "ducad-engine",
        &[
            "ducad-core",
            "ducad-sketch",
            "ducad-kernel",
            "ducad-io",
            "ducad-sim",
        ],
    ),
    ("ducad-assist", &["ducad-engine"]),
    ("ducad-mcp", &["ducad-engine", "ducad-kernel"]),
    (
        "ducad-ui",
        &[
            "ducad-core",
            "ducad-sketch",
            "ducad-kernel",
            "ducad-io",
            "ducad-glass",
            "ducad-i18n",
            "ducad-cloud",
        ],
    ),
    (
        "ducad-cli",
        &[
            "ducad-core",
            "ducad-sketch",
            "ducad-kernel",
            "ducad-io",
            "ducad-sim",
            "ducad-engine",
            "ducad-assist",
            "ducad-chat",
            "ducad-mcp",
        ],
    ),
    (
        "ducad-app",
        &[
            "ducad-core",
            "ducad-sketch",
            "ducad-kernel",
            "ducad-ink",
            "ducad-render",
            "ducad-io",
            "ducad-sim",
            "ducad-engine",
            "ducad-assist",
            "ducad-chat",
            "ducad-mcp",
            "ducad-glass",
            "ducad-ui",
            "ducad-i18n",
            "ducad-cloud",
        ],
    ),
    ("ducad-android", &["ducad-app"]),
];

/// Crate yang boleh menyentuh toolkit UI / jendela / dialog native.
const UI_ALLOWED: &[&str] = &[
    "ducad-glass",
    "ducad-render",
    "ducad-ui",
    "ducad-app",
    "ducad-android",
];
const UI_CRATES: &[&str] = &[
    "egui",
    "eframe",
    "egui-wgpu",
    "egui_wgpu",
    "egui_extras",
    "egui_kittest",
    "egui_icons",
    "wgpu",
    "winit",
    "rfd",
];

fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// Nama dependensi (kunci tabel, atau `package = "…"` bila di-rename) dari
/// semua bagian `[dependencies]`, `[dev-dependencies]`,
/// `[build-dependencies]`, dan `[target.*.dependencies]`.
fn dependency_names(manifest: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut in_deps = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_deps = line.ends_with("dependencies]");
            // `[dependencies.foo]` / `[target.'…'.dependencies.foo]` juga
            // bentuk tabel dependensi.
            if let Some(idx) = line.find("dependencies.") {
                names.insert(
                    line[idx + "dependencies.".len()..]
                        .trim_end_matches(']')
                        .to_string(),
                );
                in_deps = true;
            }
            continue;
        }
        if !in_deps || line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim().trim_matches('"');
            if key == "package" {
                // Baris `package = "…"` di dalam `[dependencies.foo]`.
                if let Some(name) = value.trim().trim_matches('"').split('"').next() {
                    names.insert(name.to_string());
                }
                continue;
            }
            // `foo.workspace = true` → kunci `foo`.
            let key = key.split('.').next().unwrap_or(key);
            names.insert(key.to_string());
            if let Some(pos) = value.find("package") {
                let tail = &value[pos..];
                if let Some(q) = tail.find('"') {
                    let after = &tail[q + 1..];
                    if let Some(end) = after.find('"') {
                        names.insert(after[..end].to_string());
                    }
                }
            }
        }
    }
    names
}

fn manifests() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(crates_dir()).unwrap().flatten() {
        let path = entry.path().join("Cargo.toml");
        if let Ok(text) = std::fs::read_to_string(&path) {
            let name = entry.file_name().to_string_lossy().into_owned();
            out.push((name, text));
        }
    }
    out.sort();
    assert!(out.len() >= 10, "crate ditemukan: {}", out.len());
    out
}

#[test]
fn every_crate_is_in_the_layer_table() {
    for (name, _) in manifests() {
        assert!(
            ALLOWED.iter().any(|(n, _)| *n == name),
            "crate `{name}` belum ada di tabel layering (tests/layering.rs) dan peta CLAUDE.md"
        );
    }
}

#[test]
fn ducad_dependencies_only_point_downward() {
    for (name, text) in manifests() {
        let allowed = ALLOWED
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, a)| *a)
            .unwrap_or(&[]);
        for dep in dependency_names(&text)
            .iter()
            .filter(|d| d.starts_with("ducad-"))
        {
            assert!(
                allowed.contains(&dep.as_str()),
                "`{name}` bergantung pada `{dep}`, yang tidak diizinkan tabel layering \
                 (tambahkan panahnya di tests/layering.rs dan CLAUDE.md bila memang disengaja)"
            );
        }
    }
}

#[test]
fn ui_toolkits_stay_in_the_gui_layer() {
    for (name, text) in manifests() {
        if UI_ALLOWED.contains(&name.as_str()) {
            continue;
        }
        for dep in dependency_names(&text) {
            assert!(
                !UI_CRATES.contains(&dep.as_str()),
                "`{name}` bergantung pada crate UI `{dep}`; hanya {UI_ALLOWED:?} yang boleh"
            );
        }
    }
}

#[test]
fn only_the_kernel_links_opencascade() {
    for (name, text) in manifests() {
        let uses = dependency_names(&text)
            .iter()
            .any(|d| d.starts_with("opencascade"));
        assert_eq!(
            uses,
            name == "ducad-kernel",
            "`{name}`: hanya ducad-kernel yang boleh bergantung pada opencascade"
        );
    }
}

#[test]
fn manifest_parser_reads_renames_and_target_tables() {
    let text = r#"
[package]
name = "x"

[dependencies]
ducad-core = { workspace = true }
egui_wgpu = { package = "egui-wgpu", version = "0.36" }
serde.workspace = true

[target.'cfg(target_os = "ios")'.dependencies]
objc2 = "0.6"

[dependencies.rfd]
version = "0.17"

[dev-dependencies]
proptest = "1"
"#;
    let names = dependency_names(text);
    for expected in [
        "ducad-core",
        "egui_wgpu",
        "egui-wgpu",
        "serde",
        "objc2",
        "rfd",
        "proptest",
    ] {
        assert!(names.contains(expected), "{expected} hilang dari {names:?}");
    }
    assert!(!names.contains("name"));
}
