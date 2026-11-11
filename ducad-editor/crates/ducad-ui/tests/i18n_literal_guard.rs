//! Penjaga regresi lokalisasi (Tahap 3):
//! 1. Seluruh kunci `t!("…")` dan `translate("…")` di `ducad-ui/src` dan
//!    `ducad-app/src` wajib terdefinisi di `en-US/main.ftl` maupun `id-ID/main.ftl`.
//! 2. Modul-modul permukaan utama `ducad-ui` yang sudah dilokalkan tidak boleh
//!    kembali menyisipkan literal bahasa Indonesia secara hardcoded pada widget
//!    (`ui.label("…")`, `ui.button("…")`, `on_hover_text("…")`, `RichText::new("…")`).

use std::path::{Path, PathBuf};

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn keys_in(src: &str) -> Vec<String> {
    let mut keys = Vec::new();
    for marker in ["t!(\"", "translate(\""] {
        let mut rest = src;
        while let Some(at) = rest.find(marker) {
            let tail = &rest[at + marker.len()..];
            if let Some(end) = tail.find('"') {
                let candidate = &tail[..end];
                if !candidate.is_empty() && !candidate.contains('{') {
                    keys.push(candidate.to_string());
                }
            }
            rest = tail;
        }
    }
    keys
}

#[test]
fn all_ui_and_app_i18n_keys_exist_in_both_locales() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    collect_rs(&root.join("ducad-ui/src"), &mut files);
    collect_rs(&root.join("ducad-app/src"), &mut files);

    let locales = ["en-US", "id-ID"].map(|l| {
        std::fs::read_to_string(root.join(format!("ducad-i18n/locales/{l}/main.ftl"))).unwrap()
    });
    let defined = |text: &str, key: &str| {
        text.lines()
            .any(|l| l.starts_with(&format!("{key} =")) || l.starts_with(&format!("{key}=")))
    };

    let mut checked = 0usize;
    let mut missing = Vec::new();
    for file in &files {
        let src = std::fs::read_to_string(file).unwrap();
        // Abaikan kode di bawah #[cfg(test)] (mis. tes fallback kunci tidak dikenal).
        let prod_src = src.split("#[cfg(test)]").next().unwrap_or(&src);
        for key in keys_in(prod_src) {
            checked += 1;
            for (lang, text) in ["en-US", "id-ID"].iter().zip(&locales) {
                if !defined(text, &key) {
                    let rel = file.strip_prefix(&root).unwrap_or(file);
                    missing.push(format!("{key} ({lang}) dipakai di {}", rel.display()));
                }
            }
        }
    }
    assert!(checked >= 200, "pemindai kunci terlalu sedikit ({checked})");
    missing.sort();
    missing.dedup();
    assert!(
        missing.is_empty(),
        "kunci i18n belum ada di berkas Fluent:\n{}",
        missing.join("\n")
    );
}

#[test]
fn localized_ui_surfaces_have_no_hardcoded_indonesian_widget_literals() {
    let ui_src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let guarded_files = [
        "top_bar.rs",
        "left_toolbar.rs",
        "context_bar.rs",
        "account_drawer.rs",
        "notification_toast.rs",
        "unit_input.rs",
        "theme.rs",
        "touch.rs",
        "command_palette.rs",
        "error_card.rs",
        "checks_panel.rs",
        "review_panel.rs",
        "brief_studio.rs",
        "proposal_card.rs",
    ];
    let widget_calls = [
        "ui.label(\"",
        "ui.button(\"",
        "ui.small_button(\"",
        "on_hover_text(\"",
        "RichText::new(\"",
    ];
    let forbidden_words = [
        "gagal",
        "simpan",
        "buka",
        "tutup",
        "pengaturan",
        "impor",
        "ekspor",
        "hapus",
        "batal",
        "keluar",
        "masuk",
        "perbarui",
        "tampilkan",
        "sembunyikan",
        "kurangi",
        "ukuran",
        "bahasa",
        "satuan",
    ];

    let mut violations = Vec::new();
    for rel in guarded_files {
        let path = ui_src.join(rel);
        let src = std::fs::read_to_string(&path).unwrap();
        let prod_src = src.split("#[cfg(test)]").next().unwrap_or(&src);
        for (idx, line) in prod_src.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            for marker in widget_calls {
                let mut rest = trimmed;
                while let Some(pos) = rest.find(marker) {
                    let after = &rest[pos + marker.len()..];
                    if let Some(end) = after.find('"') {
                        let lit = &after[..end];
                        let lower = lit.to_lowercase();
                        for word in forbidden_words {
                            if lower
                                .split(|c: char| !c.is_alphabetic())
                                .any(|tok| tok == word)
                            {
                                violations.push(format!(
                                    "{rel}:{} literal Indonesia `{lit}` (kata `{word}`)",
                                    idx + 1
                                ));
                                break;
                            }
                        }
                    }
                    rest = after;
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "ditemukan literal UI bahasa Indonesia tanpa t!():\n{}",
        violations.join("\n")
    );
}
