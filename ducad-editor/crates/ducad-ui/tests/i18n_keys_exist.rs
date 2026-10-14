//! Penjaga: setiap kunci `t!("…")` yang dipakai layar paket desain harus ada
//! di KEDUA berkas bahasa. Kunci yang hilang tampil sebagai nama kuncinya
//! (mis. "brief-approve") — pernah lolos karena tes render ikut memakai
//! `t!()` sehingga tetap "cocok".

use std::path::{Path, PathBuf};

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Kunci literal di `t!("kunci"…)` dan `translate("kunci"…)`.
fn keys_in(src: &str) -> Vec<String> {
    let mut keys = Vec::new();
    for marker in ["t!(\"", "translate(\""] {
        let mut rest = src;
        while let Some(at) = rest.find(marker) {
            let tail = &rest[at + marker.len()..];
            if let Some(end) = tail.find('"') {
                keys.push(tail[..end].to_string());
            }
            rest = tail;
        }
    }
    keys
}

#[test]
fn design_package_keys_exist_in_both_languages() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    collect(&root.join("ducad-ui/src"), &mut files);
    collect(&root.join("ducad-app/src"), &mut files);
    let locales = ["en-US", "id-ID"].map(|l| {
        std::fs::read_to_string(root.join(format!("ducad-i18n/locales/{l}/main.ftl"))).unwrap()
    });
    let defined = |text: &str, key: &str| text.lines().any(|l| l.starts_with(&format!("{key} =")));
    let mut used = 0;
    let mut missing = Vec::new();
    for file in &files {
        let src = std::fs::read_to_string(file).unwrap();
        for key in keys_in(&src) {
            // Lingkup penjaga: layar yang ditambahkan bersama paket desain.
            if ![
                "brief-",
                "review-",
                "chat-attach",
                "sheet-redline",
                "palette-review",
                "palette-brief",
                "menu-new-from-brief",
            ]
            .iter()
            .any(|p| key.starts_with(p))
            {
                continue;
            }
            used += 1;
            for (lang, text) in ["en-US", "id-ID"].iter().zip(&locales) {
                if !defined(text, &key) {
                    missing.push(format!("{key} ({lang}) dipakai di {}", file.display()));
                }
            }
        }
    }
    assert!(used > 80, "pemindai tidak menemukan kunci ({used})");
    missing.sort();
    missing.dedup();
    assert!(
        missing.is_empty(),
        "kunci terjemahan hilang:\n{}",
        missing.join("\n")
    );
}
