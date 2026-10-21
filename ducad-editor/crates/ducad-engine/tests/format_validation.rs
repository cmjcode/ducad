//! Step 4 `docs/PLAN_DESIGNER_UX.md` — validasi format berkas.
//!
//! Setiap tes di sini menjadi bukti satu baris di `docs/FORMAT_MATRIX.md`:
//! apa yang bertahan saat round-trip, satuan apa yang dinyatakan berkas, dan
//! bahwa input rusak ditolak dengan `Err` (bukan panic).

use std::path::PathBuf;

use ducad_engine::drawing_auto::{auto_sheet_model, hole_notes, TitleInfo};
use ducad_engine::export::{export, ExportFormat};
use ducad_engine::ops::{Op, OpFile};
use ducad_engine::{DesignDoc, Session};
use ducad_io::drawing::PaperSize;

const BRACKET: &str = include_str!("fixtures/sample_bracket.ops.json");

fn temp(tag: &str, ext: &str) -> PathBuf {
    std::env::temp_dir().join(format!("ducad-format-{tag}-{}.{ext}", std::process::id()))
}

fn bracket() -> Session {
    let f: OpFile = serde_json::from_str(BRACKET).unwrap();
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    let r = s.run(f.ops, false);
    assert!(r.committed, "{:?}", r.error);
    s
}

fn run(s: &mut Session, json: &str) {
    let ops: Vec<Op> = serde_json::from_str(json).unwrap();
    let r = s.run(ops, false);
    assert!(r.committed, "{:?}", r.error);
}

fn total_volume(s: &Session) -> f64 {
    s.summary().bodies.iter().map(|b| b.volume).sum()
}

fn title() -> TitleInfo {
    TitleInfo {
        title: "Uji format".into(),
        part_number: "FMT-001".into(),
        author: "ducad".into(),
        date: "2026-10-21".into(),
        material: "AL6061".into(),
        revision: "B".into(),
    }
}

fn json(v: &impl serde::Serialize) -> serde_json::Value {
    serde_json::to_value(v).unwrap()
}

// ---------------------------------------------------------------- native

/// `.ducad`: param, oplog (termasuk sketsa di bidang `frame`), op suppressed,
/// material, dan lembar gambar bertahan; geometri replay sama.
#[test]
fn native_roundtrip_keeps_design_intent() {
    let mut s = bracket();
    run(
        &mut s,
        r#"[{"op":"sketch","id":"tilted","plane":{"frame":{"origin":[0,0,60],
              "u":[1,0,0],"v":[0,0.70710678,0.70710678],"normal":[0,-0.70710678,0.70710678]}},
             "entities":[{"rect":{"center":[0,0],"w":8,"h":8}}]},
            {"op":"extrude","id":"tab","sketch":"tilted","distance":4},
            {"op":"set_material","id":"mat","body":"bracket","material":"al_6061_t6"}]"#,
    );
    // Suppress fillet lewat jalur yang sama dengan GUI (replay design).
    let mut design: DesignDoc = s.design().clone();
    design.suppressed.push("inner_fillet".into());
    design.fingerprint.clear();
    let mut s = Session::replay(design).unwrap();
    s.save_drawing(
        serde_json::from_value(serde_json::json!({
            "name": "lembar-uji",
            "notes": ["SEMUA UKURAN DALAM MM"]
        }))
        .unwrap(),
    );

    let path = temp("native", "ducad");
    s.save(&path).unwrap();
    let loaded = Session::from_file(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    let (a, b) = (s.design(), loaded.design());
    assert_eq!(b.params, a.params, "param");
    assert_eq!(json(&b.oplog), json(&a.oplog), "oplog (termasuk plane frame)");
    assert_eq!(b.suppressed, vec!["inner_fillet".to_string()], "suppressed");
    assert_eq!(json(&b.drawings), json(&a.drawings), "lembar gambar");
    assert_eq!(b.drawings.len(), 1);

    let mats = |s: &Session| {
        let mut v: Vec<String> = s
            .model()
            .doc
            .bodies
            .values()
            .map(|b| format!("{}={:?}", b.name, b.material))
            .collect();
        v.sort();
        v
    };
    assert_eq!(mats(&loaded), mats(&s), "material body");

    let (va, vb) = (total_volume(&s), total_volume(&loaded));
    assert!((va - vb).abs() / va < 1e-6, "volume {va} vs {vb}");
    assert_eq!(loaded.summary().bodies.len(), s.summary().bodies.len());
}

// ------------------------------------------------------------------ STEP

/// STEP: multi-body, volume sama setelah dibuka ulang.
#[test]
fn step_multibody_reimports_with_same_volume() {
    let mut s = Session::new();
    run(
        &mut s,
        r#"[{"op":"primitive","id":"a","shape":{"box":{"size":[10,10,10]}}},
            {"op":"primitive","id":"b","shape":{"box":{"size":[5,5,5]}},"at":[30,0,0]}]"#,
    );
    let path = temp("multi", "step");
    assert!(export(&s, ExportFormat::Step, &path).unwrap() > 0);
    let text = std::fs::read_to_string(&path).unwrap();
    let shape = ducad_io::step_io::import(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    assert!(text.starts_with("ISO-10303-21;"), "header ISO 10303-21");
    assert!(text.contains("AP242"), "skema AP242");
    assert!(text.contains("SI_UNIT(.MILLI.,.METRE.)"), "satuan mm");
    let v = shape.volume().abs();
    assert!((v - 1125.0).abs() / 1125.0 < 1e-4, "volume dua body {v}");
}

// ------------------------------------------------------------------- STL

/// STL biner: jumlah segitiga di header = isi berkas, bbox dalam mm sama
/// dengan model.
#[test]
fn stl_triangle_count_and_bbox_match_model() {
    let s = bracket();
    let path = temp("bracket", "stl");
    let size = export(&s, ExportFormat::Stl, &path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let mesh = ducad_io::mesh_export::read_stl(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    let n = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as u64;
    assert!(n > 0);
    assert_eq!(size, 84 + 50 * n, "ukuran berkas = 84 + 50·n");
    assert_eq!(mesh.indices.len() as u64, 3 * n);

    let bb = s.summary().bodies[0].bbox;
    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    for p in &mesh.positions {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k] as f64);
            hi[k] = hi[k].max(p[k] as f64);
        }
    }
    for k in 0..3 {
        assert!((lo[k] - bb[0][k]).abs() < 1e-3, "bbox min {lo:?} vs {bb:?}");
        assert!((hi[k] - bb[1][k]).abs() < 1e-3, "bbox max {hi:?} vs {bb:?}");
    }
}

// --------------------------------------------------------- drawing PDF/DXF

/// Drawing DXF: menyatakan mm ($INSUNITS 4), memakai layer standar, dan
/// berisi jenis entitas yang dijamin (LINE, CIRCLE/ARC, TEXT).
#[test]
fn drawing_dxf_declares_mm_and_layers() {
    let s = bracket();
    let notes = hole_notes(s.design(), &s.design().params);
    let sheet = auto_sheet_model(s.model(), PaperSize::A3Landscape, &title(), &notes).unwrap();
    let path = temp("drawing", "dxf");
    ducad_io::dxf::export_drawing_sheet(&sheet, &path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    assert!(text.contains("$INSUNITS\n70\n4\n"), "satuan mm");
    for layer in ["BORDER", "TITLEBLOCK", "VISIBLE", "DIMENSIONS"] {
        assert!(text.contains(&format!("LAYER\n2\n{layer}\n")), "layer {layer}");
    }
    for kind in ["LINE", "TEXT"] {
        assert!(text.contains(&format!("0\n{kind}\n")), "entitas {kind}");
    }
    assert!(
        text.contains("0\nCIRCLE\n") || text.contains("0\nARC\n"),
        "lubang tergambar sebagai lingkaran/busur"
    );
    // Dibaca kembali oleh importer DXF sendiri tanpa error.
    let back = ducad_io::dxf::import_str(&text).unwrap();
    assert!(!back.entities.is_empty());
    assert_eq!(back.source_unit, Some("mm"));
}

/// Drawing PDF: satu halaman berukuran kertas yang dipilih, teks judul dan
/// nomor part ada di stream.
#[test]
fn drawing_pdf_has_paper_size_and_title() {
    let s = bracket();
    for (paper, w_mm, h_mm) in [
        (PaperSize::A3Landscape, 420.0, 297.0),
        (PaperSize::A4Landscape, 297.0, 210.0),
    ] {
        let sheet = auto_sheet_model(s.model(), paper, &title(), &[]).unwrap();
        let pdf = ducad_io::pdf::generate_pdf_bytes(&sheet);
        let text = String::from_utf8_lossy(&pdf);
        assert!(pdf.starts_with(b"%PDF"));
        assert!(text.trim_end().ends_with("%%EOF"), "PDF ditutup %%EOF");
        assert_eq!(text.matches("/Type /Page ").count(), 1, "{paper:?}: satu halaman");
        let pt = |mm: f64| mm / 25.4 * 72.0;
        let media = format!("/MediaBox [0 0 {:.2} {:.2}]", pt(w_mm), pt(h_mm));
        assert!(text.contains(&media), "{paper:?}: {media}");
        assert!(text.contains("FMT-001"), "{paper:?}: nomor part");
        assert!(text.contains("Uji format"), "{paper:?}: judul");
    }
}

// ------------------------------------------------------------ input rusak

/// Berkas rusak/terpotong ditolak dengan `Err`, tidak panic.
#[test]
fn corrupt_inputs_are_rejected_not_panicking() {
    // Native: JSON rusak, JSON valid tapi bukan dokumen, dan berkas terpotong.
    let mut s = bracket();
    let good = temp("good", "ducad");
    s.save(&good).unwrap();
    let full = std::fs::read_to_string(&good).unwrap();
    let _ = std::fs::remove_file(&good);
    for (tag, body) in [
        ("garbage", "bukan json".to_string()),
        ("wrongshape", "{\"hello\":1}".to_string()),
        ("truncated", full[..full.len() / 2].to_string()),
    ] {
        let p = temp(tag, "ducad");
        std::fs::write(&p, body).unwrap();
        let r = std::panic::catch_unwind(|| Session::from_file(&p).is_err());
        let _ = std::fs::remove_file(&p);
        assert_eq!(r.ok(), Some(true), "native {tag}: harus Err tanpa panic");
    }

    // STEP: teks acak, STEP terpotong, berkas kosong.
    let step = temp("ok", "step");
    export(&s, ExportFormat::Step, &step).unwrap();
    let step_text = std::fs::read_to_string(&step).unwrap();
    let _ = std::fs::remove_file(&step);
    for (tag, body) in [
        ("garbage", "ISO-10303-21;\nHEADER;\nsampah".to_string()),
        ("truncated", step_text[..step_text.len() / 3].to_string()),
        ("empty", String::new()),
    ] {
        let p = temp(tag, "step");
        std::fs::write(&p, body).unwrap();
        let r = std::panic::catch_unwind(|| ducad_io::step_io::import(&p).is_err());
        let _ = std::fs::remove_file(&p);
        assert_eq!(r.ok(), Some(true), "STEP {tag}: harus Err tanpa panic");
    }

    // STL: biner terpotong (header menjanjikan 10 segitiga, isi kosong).
    let r = std::panic::catch_unwind(|| {
        let mut b = vec![0u8; 84];
        b[80..84].copy_from_slice(&10u32.to_le_bytes());
        ducad_io::mesh_export::read_stl_from_bytes(&b).is_err()
    });
    assert_eq!(r.ok(), Some(true), "STL terpotong");

    // DXF: angka rusak / berkas terpotong tidak panic dan tidak menghasilkan
    // geometri palsu.
    for body in ["0\nSECTION\n2\nENTITIES\n0\nLINE\n10\nabc\n", "0\nSECTION\n2\nENT", "x"] {
        let r = std::panic::catch_unwind(|| match ducad_io::dxf::import_str(body) {
            Ok(res) => res.entities.is_empty(),
            Err(_) => true,
        });
        assert_eq!(r.ok(), Some(true), "DXF rusak: {body:?}");
    }
    let _ = &mut s;
}
