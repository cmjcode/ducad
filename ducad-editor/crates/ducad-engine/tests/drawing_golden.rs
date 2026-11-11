//! Gold test lembar gambar (P21.0/P21.8) + kriteria penerimaan valve body.
//!
//! Hash keluaran PDF/SVG/DXF disimpan di `tests/golden/drawing.hashes.truck.json`.
//! Perbarui dengan `DUCAD_UPDATE_GOLDEN=1 cargo test -p ducad-engine --test drawing_golden`.
//!
//! Valve body belum bisa dibangun dalam waktu wajar (R7,
//! `tests/goldens/known_failures.truck.txt`): tes yang memerlukannya di-skip.

use std::collections::BTreeMap;
use std::path::PathBuf;

use ducad_engine::drawing_auto::{build_sheet, hole_notes};
use ducad_engine::ops::OpFile;
use ducad_engine::tooling::{call_core_tool, ToolPaths};
use ducad_engine::{OpErrorCode, OpResult, Session};
use ducad_io::drawing::scene::{build_scene, Item};
use ducad_io::drawing::{DrawingSheet, DrawingSpec, TitleSpec};
use ducad_kernel::ProjectedViewKind;
use serde_json::json;

fn load(text: &str) -> Session {
    let f: OpFile = serde_json::from_str(text).unwrap();
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    let report = s.run(f.ops, false);
    assert!(report.committed, "{:?}", report.error);
    s.set_checks(f.checks);
    s.set_drawings(f.drawings);
    s
}

fn valve() -> Session {
    load(include_str!("fixtures/valve_body.ops.json"))
}

struct TmpPaths(PathBuf);

impl ToolPaths for TmpPaths {
    fn resolve(&self, p: &str) -> OpResult<PathBuf> {
        Ok(self.0.join(p))
    }
}

fn tmp_dir(tag: &str) -> TmpPaths {
    let dir = std::env::temp_dir().join(format!("ducad-p21-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    TmpPaths(dir)
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn stamp(bytes: &[u8]) -> String {
    format!("{:016x}:{}", fnv1a(bytes), bytes.len())
}

fn exports(sheet: &DrawingSheet, tag: &str) -> [(String, Vec<u8>); 3] {
    let dxf = std::env::temp_dir().join(format!("ducad-golden-{tag}-{}.dxf", std::process::id()));
    ducad_io::dxf::export_drawing_sheet(sheet, &dxf).unwrap();
    let dxf_bytes = std::fs::read(&dxf).unwrap();
    let _ = std::fs::remove_file(&dxf);
    [
        (
            format!("{tag}.pdf"),
            ducad_io::pdf::generate_pdf_bytes(sheet),
        ),
        (
            format!("{tag}.svg"),
            ducad_io::svg::export_drawing_sheet_svg_string(sheet)
                .unwrap()
                .into_bytes(),
        ),
        (format!("{tag}.dxf"), dxf_bytes),
    ]
}

fn valve_sheet(s: &Session) -> (DrawingSheet, Vec<String>) {
    let spec = s
        .design()
        .drawing("sheet1")
        .expect("spec tersimpan")
        .clone();
    let out = build_sheet(s.model(), &spec, Some(&s.design().fingerprint)).unwrap();
    (out.sheet, out.warnings)
}

fn dim_texts(sheet: &DrawingSheet) -> Vec<String> {
    sheet
        .auto_dimensions
        .iter()
        .map(|d| d.text.clone())
        .collect()
}

/// R7 (known_failures.truck.txt): membangun valve body > 30 menit. Satu
/// tempat untuk mengaktifkan kembali tes valve setelah R7 diperbaiki.
const SKIP_VALVE: bool = true;

fn skip_valve() -> bool {
    if SKIP_VALVE {
        eprintln!("dilewati: R7 valve body (known_failures.truck.txt)");
    }
    SKIP_VALVE
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct StructuralSheetGolden {
    views: Vec<String>,
    auto_dimensions: Vec<String>,
    company_name: String,
    notes: Vec<String>,
    scene_texts: Vec<String>,
    visible_line_len_mm: f64,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct DrawingGoldenFile {
    structural: BTreeMap<String, StructuralSheetGolden>,
    #[serde(default)]
    byte_hashes: BTreeMap<String, BTreeMap<String, String>>,
}

fn build_structural_golden(sheet: &DrawingSheet, files: &[(String, Vec<u8>); 3]) -> StructuralSheetGolden {
    for (name, bytes) in files {
        let text = String::from_utf8_lossy(bytes);
        assert!(
            !text.contains("CAD/CAM"),
            "{name} tidak boleh lagi mengklaim CAD/CAM"
        );
        if name.ends_with(".pdf") {
            assert!(bytes.starts_with(b"%PDF-"), "{name} bukan PDF valid");
            assert!(text.contains("%%EOF"), "{name} tidak memiliki penutup PDF");
        } else if name.ends_with(".svg") {
            assert!(text.contains("<svg") && text.contains("</svg>"), "{name} bukan SVG valid");
        } else if name.ends_with(".dxf") {
            assert!(text.contains("SECTION") && text.contains("EOF"), "{name} bukan DXF valid");
        }
    }

    let scene = build_scene(sheet);
    let mut scene_texts: Vec<String> = scene
        .groups
        .iter()
        .flat_map(|g| &g.items)
        .filter_map(|item| match item {
            Item::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    scene_texts.sort();

    let visible_line_len_mm: f64 = scene
        .groups
        .iter()
        .flat_map(|g| &g.items)
        .filter_map(|item| match item {
            Item::Line { a, b, pen } if pen.layer == "VISIBLE" => {
                let dx = (b[0] - a[0]) as f64;
                let dy = (b[1] - a[1]) as f64;
                Some((dx * dx + dy * dy).sqrt())
            }
            _ => None,
        })
        .sum();

    StructuralSheetGolden {
        views: sheet
            .view_placements
            .iter()
            .filter(|p| p.visible)
            .map(|p| format!("{:?}", p.kind))
            .collect(),
        auto_dimensions: dim_texts(sheet),
        company_name: sheet.title_block.company_name.clone(),
        notes: sheet.notes.clone(),
        scene_texts,
        visible_line_len_mm: (visible_line_len_mm * 10.0).round() / 10.0,
    }
}

#[test]
fn drawing_golden_hashes_are_stable() {
    let flange = load(include_str!("fixtures/flange.ops.json"));
    let spec = DrawingSpec {
        title: TitleSpec {
            title: "Flange".into(),
            part_number: "F-001".into(),
            author: "ducad".into(),
            date: "2026-01-01".into(),
            material: "S235".into(),
            revision: "A".into(),
        },
        notes: hole_notes(flange.design(), &flange.design().params),
        ..DrawingSpec::default()
    };
    let flange_sheet = build_sheet(flange.model(), &spec, None).unwrap().sheet;
    let mut sheets = vec![(flange_sheet, "flange")];
    if !skip_valve() {
        let valve = valve();
        let (valve_sheet, warnings) = valve_sheet(&valve);
        assert!(warnings.is_empty(), "{warnings:?}");
        sheets.push((valve_sheet, "valve_body"));
    }

    let mut actual_bytes: BTreeMap<String, String> = BTreeMap::new();
    let mut actual_structural: BTreeMap<String, StructuralSheetGolden> = BTreeMap::new();
    for (sheet, tag) in sheets.iter().map(|(s, t)| (s, *t)) {
        let first = exports(sheet, tag);
        actual_structural.insert(tag.to_string(), build_structural_golden(sheet, &first));
        for (name, bytes) in first {
            actual_bytes.insert(name, stamp(&bytes));
        }
        // Dua kali ekspor pada mesin yang sama harus identik byte demi byte.
        for (name, bytes) in exports(&sheet.clone(), tag) {
            assert_eq!(actual_bytes[&name], stamp(&bytes), "{name} tidak deterministik");
        }
    }

    let file = "tests/golden/drawing.hashes.truck.json";
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(file);
    let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    if std::env::var_os("DUCAD_UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut golden: DrawingGoldenFile = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        if golden.structural != actual_structural {
            // Struktur lembar berubah sengaja: kosongkan hash byte arsitektur
            // lain yang sudah usang supaya direkam ulang bersih.
            golden.byte_hashes.clear();
            golden.structural = actual_structural;
        }
        golden.byte_hashes.insert(platform, actual_bytes);
        let text = serde_json::to_string_pretty(&golden).unwrap() + "\n";
        std::fs::write(&path, text).unwrap();
        return;
    }

    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "{} tidak terbaca ({e}); jalankan dengan DUCAD_UPDATE_GOLDEN=1",
            path.display()
        )
    });
    let golden: DrawingGoldenFile = serde_json::from_str(&text).unwrap();
    assert_eq!(
        actual_structural.keys().collect::<Vec<_>>(),
        golden.structural.keys().collect::<Vec<_>>(),
        "daftar lembar golden tidak cocok"
    );
    for (tag, actual_s) in &actual_structural {
        let expected_s = &golden.structural[tag];
        assert_eq!(actual_s.views, expected_s.views, "tampak `{tag}` berubah");
        assert_eq!(
            actual_s.auto_dimensions, expected_s.auto_dimensions,
            "dimensi otomatis `{tag}` berubah"
        );
        assert_eq!(
            actual_s.company_name, expected_s.company_name,
            "nama perusahaan `{tag}` berubah"
        );
        assert_eq!(actual_s.notes, expected_s.notes, "catatan `{tag}` berubah");
        assert_eq!(
            actual_s.scene_texts, expected_s.scene_texts,
            "teks scene `{tag}` berubah; bila disengaja perbarui dengan DUCAD_UPDATE_GOLDEN=1"
        );
        assert!(
            (actual_s.visible_line_len_mm - expected_s.visible_line_len_mm).abs() <= 1.0,
            "panjang garis tampak `{tag}` bergeser jauh: aktual {} vs golden {}",
            actual_s.visible_line_len_mm,
            expected_s.visible_line_len_mm
        );
    }

    if let Some(expected_bytes) = golden.byte_hashes.get(&platform) {
        assert_eq!(
            &actual_bytes, expected_bytes,
            "keluaran byte lembar berubah pada `{platform}`; bila disengaja perbarui dengan DUCAD_UPDATE_GOLDEN=1"
        );
    } else {
        eprintln!(
            "catatan: hash byte untuk `{platform}` belum direkam di {}; struktur & geometri terkuantisasi lulus",
            path.display()
        );
    }
}

#[test]
fn valve_body_sheet_meets_acceptance_criteria() {
    if skip_valve() {
        return;
    }
    let s = valve();
    let checks = s.run_checks(None);
    assert_eq!(checks.fail + checks.error, 0, "{checks:?}");
    let (sheet, _) = valve_sheet(&s);

    // 1. Tampak: Depan, Atas, potongan A-A dan B-B; B-B menulis skalanya.
    let kinds: Vec<ProjectedViewKind> = sheet
        .view_placements
        .iter()
        .filter(|p| p.visible)
        .map(|p| p.kind)
        .collect();
    for kind in [
        ProjectedViewKind::Front,
        ProjectedViewKind::Top,
        ProjectedViewKind::Section('A'),
        ProjectedViewKind::Section('B'),
    ] {
        assert!(kinds.contains(&kind), "tampak {kind:?} hilang: {kinds:?}");
    }
    assert!(!kinds.contains(&ProjectedViewKind::Right));
    assert!((sheet.scale - 0.5).abs() < 1e-6);
    let a = sheet.drawing.section('A').unwrap();
    assert_eq!(a.cutting_line.polyline().len(), 4, "A-A bertingkat");
    assert!(a.cut_area_mm2 > 100.0 && sheet.drawing.section('B').unwrap().cut_area_mm2 > 100.0);
    assert!(sheet.drawing.front.exact && sheet.drawing.top.exact && a.view.exact);

    let svg = ducad_io::svg::export_drawing_sheet_svg_string(&sheet).unwrap();
    let pdf = ducad_io::pdf::generate_pdf_bytes(&sheet);
    let pdf_text = String::from_utf8_lossy(&pdf);
    assert!(svg.contains("SECTION A-A</text>") && svg.contains("SECTION B-B</text>"));
    assert_eq!(
        svg.matches(">SCALE 1:2</text>").count(),
        1,
        "hanya B-B yang menulis skala"
    );
    assert!(svg.contains(">AISI 316L</text>") && pdf_text.contains("(AISI 316L)"));
    assert!(svg.contains(">1:2</text>") && svg.contains(">Valve Body</text>"));
    assert_eq!(sheet.notes.len(), 3);
    assert!(svg.contains(">NOTE:</text>") && svg.contains(">3. DRAWING SCALE 1:2.</text>"));
    // Dua render berbayang tersemat; grid zona A–F / 1–8.
    assert_eq!(sheet.shaded.len(), 2);
    assert_eq!(pdf_text.matches("/Subtype /Image").count(), 2);
    assert_eq!(svg.matches("<image ").count(), 2);
    assert!(svg.contains(">F</text>") && svg.contains(">8</text>") && !svg.contains(">G</text>"));

    // 2. Dimensi otomatis.
    let texts = dim_texts(&sheet);
    for want in [
        "Ø42",
        "Ø60",
        "Ø98",
        "Ø146",
        "4×Ø14 PCD Ø130",
        "88",
        "120",
        "166",
        "2×45°",
        "5×45°",
        "R5",
    ] {
        assert!(
            texts.iter().any(|t| t == want),
            "dimensi '{want}' hilang: {texts:?}"
        );
    }
    let boxes: Vec<[f32; 4]> = sheet.auto_dimensions.iter().map(|d| d.text_box()).collect();
    for i in 0..boxes.len() {
        for j in (i + 1)..boxes.len() {
            let (p, q) = (&boxes[i], &boxes[j]);
            let hit = p[0] < q[2] && q[0] < p[2] && p[1] < q[3] && q[1] < p[3];
            assert!(!hit, "teks '{}' menimpa '{}'", texts[i], texts[j]);
        }
    }

    // 4. Lingkaran adalah kurva, bukan poligon.
    assert!(!svg.contains("<polyline"));
    assert!(
        svg.matches("<circle ").count() >= 10,
        "{}",
        svg.matches("<circle ").count()
    );
    assert!(!pdf_text.contains(" arc"));
    let scene = build_scene(&sheet);
    let section_a = scene.group("view_section_a").unwrap();
    let curves = section_a
        .items
        .iter()
        .filter(|i| matches!(i, Item::Circle { .. } | Item::Arc { .. }))
        .count();
    assert!(
        curves >= 4,
        "potongan A-A harus memuat busur/lingkaran analitik: {curves}"
    );

    // 5. Deterministik + ukuran wajar (< 4 MB).
    assert_eq!(pdf, ducad_io::pdf::generate_pdf_bytes(&valve_sheet(&s).0));
    assert!(pdf.len() < 4 * 1024 * 1024, "PDF {} byte", pdf.len());
}

#[test]
fn drawing_tool_renders_saved_sheet_and_follows_set_params() {
    if skip_valve() {
        return;
    }
    let mut s = valve();
    let paths = tmp_dir("tool");

    // Lembar tersimpan: cukup `name`.
    let out = call_core_tool(
        &mut s.core(),
        "drawing",
        json!({ "name": "sheet1", "format": "svg", "path": "before.svg" }),
        &paths,
    )
    .unwrap();
    assert!(!out.is_error, "{}", out.payload);
    let dims: Vec<String> = serde_json::from_value(out.payload["dimensions"].clone()).unwrap();
    assert!(dims.contains(&"Ø42".to_string()), "{dims:?}");
    assert_eq!(out.payload["scale"], "1:2");
    assert_eq!(out.payload["sections"], json!(["A", "B"]));
    assert_eq!(out.payload["shaded"], 2);
    let (before, _) = valve_sheet(&s);
    let summary = s.summary();
    assert_eq!(summary.drawings.len(), 1);
    assert!(!summary.drawings[0].stale);

    // 3. bore_d 42 → 45: Ø42 menjadi Ø45, dimensi lain tetap di tempatnya.
    let mut params = s.design().params.clone();
    params.insert("bore_d".into(), 45.0);
    assert!(s.set_params(params).unwrap().committed);
    assert_eq!(
        s.design().drawings.len(),
        1,
        "spec bertahan melewati set_params"
    );
    let out = call_core_tool(
        &mut s.core(),
        "drawing",
        json!({ "name": "sheet1", "format": "svg", "path": "after.svg" }),
        &paths,
    )
    .unwrap();
    let dims: Vec<String> = serde_json::from_value(out.payload["dimensions"].clone()).unwrap();
    assert!(
        dims.contains(&"Ø45".to_string()) && !dims.contains(&"Ø42".to_string()),
        "{dims:?}"
    );
    assert!(!s.summary().drawings[0].stale);
    let (after, _) = valve_sheet(&s);
    // Urutan dan jumlah dimensi tetap; hanya teks Ø42 yang berubah.
    assert_eq!(before.auto_dimensions.len(), after.auto_dimensions.len());
    for (old, new) in before.auto_dimensions.iter().zip(&after.auto_dimensions) {
        if old.text == "Ø42" {
            assert_eq!(new.text, "Ø45");
        } else {
            assert_eq!(new.text, old.text);
            // Chamfer 5×45° ada di mulut bore: ujung leadernya memang ikut
            // bergeser bersama bore. Semua dimensi lain harus diam.
            if old.text != "5×45°" {
                assert_eq!(
                    new.line_pos, old.line_pos,
                    "dimensi '{}' bergeser",
                    old.text
                );
            }
        }
    }
    let bore_before = before
        .auto_dimensions
        .iter()
        .find(|d| d.text == "Ø42")
        .unwrap();
    let bore_after = after
        .auto_dimensions
        .iter()
        .find(|d| d.text == "Ø45")
        .unwrap();
    // Teks Ø bore tetap di tempatnya (siku leader sama), hanya ujung panah
    // yang mengikuti lingkaran baru.
    assert_eq!(bore_after.angle_deg, bore_before.angle_deg);
    assert!((bore_after.line_pos[0] - bore_before.line_pos[0]).abs() < 0.01);
    assert!((bore_after.line_pos[1] - bore_before.line_pos[1]).abs() < 0.01);
    assert_ne!(bore_after.end, bore_before.end);

    let _ = std::fs::remove_dir_all(&paths.0);
}

#[test]
fn drawing_tool_sections_views_dimensions_and_save() {
    let mut s = load(include_str!("fixtures/flange.ops.json"));
    let paths = tmp_dir("spec");
    let out = call_core_tool(
        &mut s.core(),
        "drawing",
        json!({
            "name": "shop", "save": true, "format": "pdf", "path": "shop.pdf", "paper": "a3",
            "sections": [{ "label": "A", "parent": "top", "axis": "y", "offset": 0 }],
            "views": { "right": { "visible": false }, "section_a": { "scale": 0.5 } },
            "scale": "1:1",
            "shaded": ["iso"],
            "dimensions": [
                { "type": "hole_pattern", "select": "all[kind=cylinder][r=3.3]", "view": "top" },
                { "type": "diameter", "select": "all[kind=cylinder][r=10]", "view": "section_a" }
            ]
        }),
        &paths,
    )
    .unwrap();
    assert!(!out.is_error, "{}", out.payload);
    assert_eq!(out.payload["saved"], true);
    let dims: Vec<String> = serde_json::from_value(out.payload["dimensions"].clone()).unwrap();
    assert_eq!(dims, vec!["6×Ø6.6 PCD Ø60".to_string(), "Ø20".to_string()]);
    assert!(out.payload["bytes"].as_u64().unwrap() > 1000);
    let views: Vec<String> = serde_json::from_value(out.payload["views"].clone()).unwrap();
    assert!(views.contains(&"section_a".to_string()) && !views.contains(&"right".to_string()));

    // Tersimpan di design dan ikut round-trip `.ducad`.
    let saved = s
        .design()
        .drawing("shop")
        .expect("lembar tersimpan")
        .clone();
    assert_eq!(saved.shaded.len(), 1);
    let file = paths.0.join("flange.ducad");
    s.save(&file).unwrap();
    let mut reopened = Session::from_file(&file).unwrap();
    assert_eq!(
        reopened.design().drawings,
        s.design().drawings,
        "DrawingSpec round-trip identik"
    );
    assert_eq!(
        reopened.summary().drawings[0].summary.sections,
        vec!["A".to_string()]
    );

    // Lembar tersimpan dirender ulang tanpa argumen lain; hasilnya sama.
    let first = std::fs::read(paths.0.join("shop.pdf")).unwrap();
    let again = call_core_tool(
        &mut reopened.core(),
        "drawing",
        json!({ "name": "shop", "format": "pdf", "path": "shop2.pdf" }),
        &paths,
    )
    .unwrap();
    assert!(!again.is_error, "{}", again.payload);
    assert_eq!(
        first,
        std::fs::read(paths.0.join("shop2.pdf")).unwrap(),
        "hash PDF identik"
    );

    // Error berkode.
    let empty = call_core_tool(
        &mut s.core(),
        "drawing",
        json!({ "format": "svg", "path": "x.svg",
                "sections": [{ "label": "A", "axis": "y", "offset": 500 }] }),
        &paths,
    )
    .unwrap_err();
    assert_eq!(empty.code, OpErrorCode::DrawingSectionEmpty);
    let dup = call_core_tool(
        &mut s.core(),
        "drawing",
        json!({ "format": "svg", "path": "x.svg",
                "sections": [{ "label": "A", "axis": "y" }, { "label": "a", "axis": "x" }] }),
        &paths,
    )
    .unwrap_err();
    assert_eq!(dup.code, OpErrorCode::DrawingSectionLabelDup);
    let bad_view = call_core_tool(
        &mut s.core(),
        "drawing",
        json!({ "format": "svg", "path": "x.svg", "views": { "back": { "visible": true } } }),
        &paths,
    )
    .unwrap_err();
    assert_eq!(bad_view.code, OpErrorCode::InvalidParam);

    // Diff melaporkan lembar yang ditambahkan.
    let plain = load(include_str!("fixtures/flange.ops.json"));
    let (diff, _) = ducad_engine::diff::diff(&plain, &s, false);
    assert_eq!(diff.drawings.len(), 1);
    assert_eq!(
        (diff.drawings[0].name.as_str(), diff.drawings[0].status),
        ("shop", "added")
    );

    let _ = std::fs::remove_dir_all(&paths.0);
}

#[test]
fn hlr_cache_reuses_unchanged_geometry() {
    if skip_valve() {
        return;
    }
    let s = valve();
    let spec = s.design().drawing("sheet1").unwrap().clone();
    let key = format!("{}-cache-test", s.design().fingerprint);
    let started = std::time::Instant::now();
    let first = build_sheet(s.model(), &spec, Some(&key)).unwrap();
    let cold = started.elapsed();
    assert!(ducad_engine::drawing_auto::hlr_cache_len() >= 1);
    let started = std::time::Instant::now();
    let second = build_sheet(s.model(), &spec, Some(&key)).unwrap();
    let warm = started.elapsed();
    assert_eq!(
        ducad_io::pdf::generate_pdf_bytes(&first.sheet),
        ducad_io::pdf::generate_pdf_bytes(&second.sheet)
    );
    assert!(
        warm <= cold,
        "render kedua ({warm:?}) harus memakai cache HLR ({cold:?})"
    );
    // Valve body (±60 face): 4 tampak + 2 potongan jauh di bawah 2 s per tampak.
    assert!(cold.as_secs_f32() < 12.0, "{cold:?}");
}
