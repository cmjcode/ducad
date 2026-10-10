//! Penjaga regresi: `Response::request_focus()` TIDAK boleh dipanggil tanpa
//! syarat pada setiap frame.
//!
//! `Memory::request_focus` menyetel `requested_interrupt_ime`, lalu egui-winit
//! menjalankan `set_ime_allowed(false)` + `set_ime_allowed(true)`. Di iOS
//! winit memetakan itu ke `resignFirstResponder`/`becomeFirstResponder`,
//! sehingga keyboard virtual iPad muncul-hilang berulang (terjadi pada kotak
//! angka presisi extrude). Setiap pemanggilan harus dibungkus syarat, mis.
//! `if !resp.has_focus() { resp.request_focus(); }` atau flag sekali-jalan.

use std::path::{Path, PathBuf};

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn request_focus_is_always_guarded() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates dir");
    let mut files = Vec::new();
    for crate_name in ["ducad-app", "ducad-ui"] {
        rust_files(&crates.join(crate_name).join("src"), &mut files);
    }
    assert!(!files.is_empty(), "tidak menemukan berkas sumber");

    let mut violations = Vec::new();
    for file in files {
        let src = std::fs::read_to_string(&file).expect("baca berkas sumber");
        let lines: Vec<&str> = src.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if !trimmed.ends_with(".request_focus();") || trimmed.starts_with("//") {
                continue;
            }
            // Salah satu dari 3 baris kode sebelumnya (lewati komentar/kosong)
            // harus pembuka `if … {` — blok sekali-jalan atau cek `has_focus()`.
            let guarded = lines[..idx]
                .iter()
                .rev()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty() && !l.starts_with("//"))
                .take(3)
                .any(|g| g.starts_with("if ") && g.ends_with('{'));
            if !guarded {
                violations.push(format!("{}:{}", file.display(), idx + 1));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "request_focus() tanpa syarat (memicu restart IME tiap frame):\n{}",
        violations.join("\n")
    );
}
