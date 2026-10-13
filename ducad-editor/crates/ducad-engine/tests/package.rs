//! Paket desain multi-lembar, profil firma, biaya indikatif, redline (W2–W4).

use std::path::PathBuf;

use ducad_engine::check::CheckSummary;
use ducad_engine::firm::FirmProfile;
use ducad_engine::ops::OpFile;
use ducad_engine::package::{
    build_package, cost_md, plan_sheets, traceability_md, PackageInfo, PackageSpec, SheetSpec,
};
use ducad_engine::tooling::{call_core_tool, ToolPaths};
use ducad_engine::{OpResult, Session};
use serde_json::json;

fn load(text: &str) -> Session {
    let f: OpFile = serde_json::from_str(text).unwrap();
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    s.set_checks(f.checks);
    let report = s.run(f.ops, false);
    assert!(report.committed, "{:?}", report.error);
    s
}

fn info() -> PackageInfo {
    PackageInfo {
        title: "Bracket".into(),
        part_number: "BRK-001".into(),
        author: "ana".into(),
        date: "2026-10-13".into(),
    }
}

fn build(
    s: &Session,
    spec: &PackageSpec,
    firm: &FirmProfile,
) -> ducad_engine::package::PackageOutput {
    let checks: CheckSummary = s.run_checks(None);
    build_package(s.model(), s.meta(), spec, &info(), firm, Some(&checks)).unwrap()
}

struct TmpPaths(PathBuf);

impl ToolPaths for TmpPaths {
    fn resolve(&self, p: &str) -> OpResult<PathBuf> {
        Ok(self.0.join(p))
    }
}

fn tmp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ducad-pkg-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn page_count(pdf: &[u8]) -> usize {
    String::from_utf8_lossy(pdf).matches("/Type /Page ").count()
}

#[test]
fn single_part_package_has_cover_part_sheet_and_compliance() {
    let mut s = load(include_str!("fixtures/bracket.ops.json"));
    let core = &mut s.core();
    call_core_tool(
        core,
        "set_brief",
        json!({ "brief": { "title": "Bracket L", "function": "Menahan sensor pada rangka.",
            "process": "cnc", "quantity": 50,
            "requirements": [{ "id": "R1", "text": "Dua lubang baut M5" },
                             { "id": "R2", "text": "Bisa dibuat CNC 3 sumbu" }] } }),
        &ducad_engine::tooling::NoPaths,
    )
    .unwrap();
    call_core_tool(
        core,
        "set_checks",
        json!({ "checks": [{ "id": "lubang", "requirement": "R1", "check": "hole_count",
                             "body": "*", "diameter": 5.5, "expect": 2 }],
                "rule_packs": [{ "pack": "dfm-cnc", "requirement": "R2" }] }),
        &ducad_engine::tooling::NoPaths,
    )
    .unwrap();

    assert_eq!(
        plan_sheets(s.model(), s.meta()),
        vec![
            SheetSpec::Cover,
            SheetSpec::Part {
                body: "bracket".into()
            },
            SheetSpec::Compliance
        ]
    );
    let out = build(&s, &PackageSpec::default(), &FirmProfile::default());
    let kinds: Vec<&str> = out.pages.iter().map(|p| p.kind).collect();
    assert_eq!(&kinds[..2], ["cover", "part"]);
    assert!(kinds[2..].iter().all(|k| *k == "compliance"));
    let total = out.pages.len();
    for (i, page) in out.pages.iter().enumerate() {
        assert_eq!(
            page.sheet.title_block.sheet_number,
            format!("{} / {total}", i + 1)
        );
        assert_eq!(page.sheet.title_block.revision, "A");
        assert_eq!(page.sheet.title_block.drawing_number, "BRK-001");
    }
    // Lembar part memuat tampak dan catatan lubang; sampul memuat indeks.
    assert!(!out.pages[1].sheet.view_placements.is_empty());
    assert!(
        out.pages[1].sheet.notes.iter().any(|n| n.starts_with("2×")),
        "{:?}",
        out.pages[1].sheet.notes
    );
    let cover_text: Vec<String> = out.pages[0].groups[0]
        .items
        .iter()
        .filter_map(|i| match i {
            ducad_io::drawing::scene::Item::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    for needle in [
        "BRACKET",
        "SHEET INDEX",
        "Part - bracket",
        "DESIGN BRIEF",
        "REVISIONS",
        "DRAFT - not released",
    ] {
        assert!(
            cover_text.iter().any(|t| t.contains(needle)),
            "sampul tanpa '{needle}': {cover_text:?}"
        );
    }
    let t = out.traceability.as_ref().unwrap();
    assert_eq!(t.rows.len(), 2);
    assert!(traceability_md(t).contains("| R1 | Dua lubang baut M5 | verified |"));

    let pdf = out.pdf_bytes();
    assert_eq!(page_count(&pdf), total);
    assert!(String::from_utf8_lossy(&pdf).contains("/Title (Part - bracket)"));
    assert_eq!(
        pdf,
        build(&s, &PackageSpec::default(), &FirmProfile::default()).pdf_bytes(),
        "deterministik"
    );
}

#[test]
fn assembly_package_has_balloons_bom_and_skips_part_sheets_of_standard_parts() {
    let s = load(include_str!("fixtures/toolbox_joint.ops.json"));
    let plan = plan_sheets(s.model(), s.meta());
    assert_eq!(
        plan,
        vec![
            SheetSpec::Cover,
            SheetSpec::Assembly,
            SheetSpec::Part {
                body: "plate".into()
            },
            SheetSpec::Bom,
            SheetSpec::Compliance
        ]
    );
    let out = build(&s, &PackageSpec::default(), &FirmProfile::default());
    assert_eq!(out.bom.len(), 4);
    assert!(
        out.bom.iter().any(|l| l.standard.contains("4762")),
        "{:?}",
        out.bom
    );
    let assembly = out.pages.iter().find(|p| p.kind == "assembly").unwrap();
    assert_eq!(assembly.sheet.bom_table.items.len(), 4);
    assert_eq!(assembly.sheet.balloons.len(), 4, "satu balon per baris BOM");
    let (pw, ph) = assembly.sheet.paper_size.dimensions_mm();
    for b in &assembly.sheet.balloons {
        assert!(
            (0.0..pw).contains(&b.target_point[0]) && (0.0..ph).contains(&b.target_point[1]),
            "{b:?}"
        );
    }
    // Lembar part hanya menggambar body-nya.
    let part = out.pages.iter().find(|p| p.kind == "part").unwrap();
    assert!(part.sheet.balloons.is_empty());
    assert!(part.sheet.drawing.top.width_mm <= 40.5);
}

#[test]
fn sheet_metal_package_adds_a_flat_pattern_page_and_explicit_sheets_are_validated() {
    let s = load(include_str!("fixtures/sheet_box.ops.json"));
    let out = build(&s, &PackageSpec::default(), &FirmProfile::default());
    let flat = out
        .pages
        .iter()
        .find(|p| p.kind == "flat_pattern")
        .expect("lembar pola bentangan");
    let lines = flat.groups[0]
        .items
        .iter()
        .filter(|i| matches!(i, ducad_io::drawing::scene::Item::Line { .. }))
        .count();
    assert!(lines >= 6, "{lines}");

    let checks = s.run_checks(None);
    let bad = |sheets: Vec<SheetSpec>| {
        build_package(
            s.model(),
            s.meta(),
            &PackageSpec {
                sheets,
                paper: None,
            },
            &info(),
            &FirmProfile::default(),
            Some(&checks),
        )
        .unwrap_err()
    };
    assert!(bad(vec![SheetSpec::Drawing {
        name: "nope".into()
    }])
    .message
    .contains("nope"));
    assert!(bad(vec![SheetSpec::Part {
        body: "ghost".into()
    }])
    .message
    .contains("ghost"));
    assert!(bad(vec![SheetSpec::FlatPattern {
        body: "blank".into()
    }])
    .message
    .contains("sheet metal"));
    let err = build_package(
        s.model(),
        s.meta(),
        &PackageSpec {
            sheets: vec![SheetSpec::Cover],
            paper: Some("a2".into()),
        },
        &info(),
        &FirmProfile::default(),
        None,
    )
    .unwrap_err();
    assert!(err.message.contains("a2"));
}

#[test]
fn firm_profile_drives_title_block_and_indicative_cost() {
    let mut s = load(include_str!("fixtures/plate.ops.json"));
    let ops: Vec<ducad_engine::ops::Op> = serde_json::from_str(
        r#"[{"op":"set_material","id":"mat","body":"plate","material":"al_6061_t6"}]"#,
    )
    .unwrap();
    let report = s.run(ops, false);
    assert!(report.committed, "{:?}", report.error);
    call_core_tool(
        &mut s.core(),
        "set_brief",
        json!({ "brief": { "process": "cnc", "quantity": 10 } }),
        &ducad_engine::tooling::NoPaths,
    )
    .unwrap();
    let firm = FirmProfile::from_json(
        r#"{ "company": "PT Presisi Nusantara", "drawn_by": "tim desain",
             "general_tolerance": "ISO 2768-f", "paper": "a4", "part_number_prefix": "PN-",
             "currency": "IDR", "notes": ["DEBURR ALL EDGES."],
             "materials": [{ "name": "Al 6061-T6", "price_per_kg": 100000 }],
             "processes": [{ "process": "cnc", "setup_cost": 500000, "rate_per_hour": 300000,
                             "rate_cm3_per_hour": 60 }] }"#,
    )
    .unwrap();
    let out = build(&s, &PackageSpec::default(), &firm);
    let part = out.pages.iter().find(|p| p.kind == "part").unwrap();
    let tb = &part.sheet.title_block;
    assert_eq!(tb.company_name, "PT Presisi Nusantara");
    assert_eq!(tb.general_tolerance, "ISO 2768-f");
    assert_eq!(tb.drawing_number, "PN-BRK-001");
    assert_eq!(
        part.sheet.paper_size,
        ducad_io::drawing::PaperSize::A4Landscape
    );
    assert!(part.sheet.notes.iter().any(|n| n == "DEBURR ALL EDGES."));

    // Balok stok 60×40×8 = 19,2 cm³; Al 2,70 g/cm³ → 51,84 g → 5184 IDR.
    let line = &out.cost.lines[0];
    assert_eq!(line.stock_cm3, 19.2);
    let material = line.material_cost.unwrap();
    assert!((material - 5184.0).abs() < 60.0, "{material}");
    let hours = line.process_hours.unwrap();
    let removed = 19.2 - s.model().geometry.values().next().unwrap().shape.volume() / 1000.0;
    assert!((hours - removed / 60.0).abs() < 1e-3, "{hours}");
    assert_eq!(out.cost.setup_per_unit, Some(50000.0));
    let total = out.cost.unit_total.unwrap();
    // `process_hours` dibulatkan 3 desimal di laporan; total dihitung dari nilai penuh.
    assert!(
        (total - (material + removed / 60.0 * 300000.0 + 50000.0)).abs() < 1.0,
        "{total}"
    );
    let md = cost_md(&out.cost);
    assert!(md.contains("Indicative only, not a quotation"));
    assert!(md.contains("**Unit total:"));

    // Tanpa profil: tidak ada angka biaya, hanya peringatan.
    let plain = build(&s, &PackageSpec::default(), &FirmProfile::default());
    assert!(plain.cost.unit_total.is_none());
    assert!(!plain.cost.warnings.is_empty());
    assert!(cost_md(&plain.cost).contains("not available"));
}

#[test]
fn build_package_tool_writes_pdf_and_reports() {
    let mut s = load(include_str!("fixtures/toolbox_joint.ops.json"));
    let dir = tmp("tool");
    let out = call_core_tool(
        &mut s.core(),
        "build_package",
        json!({ "path": "joint.pdf", "title": "Sambungan baut", "date": "2026-10-13",
                "reports_dir": "reports", "save": true,
                "sheets": [{ "kind": "cover" }, { "kind": "assembly" }, { "kind": "bom" }] }),
        &TmpPaths(dir.clone()),
    )
    .unwrap();
    let p = &out.payload;
    assert_eq!(p["sheets"].as_array().unwrap().len(), 3, "{p}");
    assert_eq!(p["sheets"][1]["kind"], "assembly");
    assert_eq!(p["state"], "draft");
    let pdf = std::fs::read(dir.join("joint.pdf")).unwrap();
    assert_eq!(page_count(&pdf), 3);
    assert!(dir.join("reports/compliance.md").is_file());
    assert!(dir.join("reports/cost.md").is_file());
    assert_eq!(s.design().package.as_ref().unwrap().sheets.len(), 3);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn redlines_are_listed_resolved_and_block_review() {
    let mut s = load(include_str!("fixtures/plate.ops.json"));
    let none = ducad_engine::tooling::NoPaths;
    let added = call_core_tool(
        &mut s.core(),
        "add_redline",
        json!({ "text": "Tebalkan plat jadi 10 mm", "position": [40, 250], "target": [120, 160],
                "author": "budi" }),
        &none,
    )
    .unwrap();
    assert_eq!(
        added.payload["added"],
        json!({ "sheet": "sheet1", "id": "RL1" })
    );
    call_core_tool(
        &mut s.core(),
        "add_redline",
        json!({ "sheet": "sheet1", "text": "Tambah champer", "position": [40, 240] }),
        &none,
    )
    .unwrap();
    let list = call_core_tool(&mut s.core(), "list_redlines", json!({}), &none).unwrap();
    assert_eq!(list.payload["open"], 2);
    assert_eq!(list.payload["redlines"][1]["id"], "RL2");

    // Redline terbuka tampil merah di PDF lembar dan masuk ceklis review.
    let spec = s.design().drawing("sheet1").unwrap().clone();
    let sheet = ducad_engine::drawing_auto::build_sheet(s.model(), &spec, None)
        .unwrap()
        .sheet;
    let pdf = String::from_utf8_lossy(&ducad_io::pdf::generate_pdf_bytes(&sheet)).to_string();
    assert!(
        pdf.contains("(RL1: Tebalkan plat jadi 10 mm)"),
        "redline tidak tergambar"
    );
    assert!(pdf.contains("0.784 0.118 0.118 RG"), "redline harus merah");
    let status = call_core_tool(&mut s.core(), "package_status", json!({}), &none).unwrap();
    let redlines = status.payload["review"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["item"] == "redlines")
        .unwrap()
        .clone();
    assert_eq!(redlines["ok"], false);

    assert!(call_core_tool(
        &mut s.core(),
        "resolve_redline",
        json!({ "id": "RL1", "resolution": " " }),
        &none
    )
    .is_err());
    assert!(call_core_tool(
        &mut s.core(),
        "resolve_redline",
        json!({ "id": "RL9", "resolution": "x" }),
        &none
    )
    .is_err());
    let done = call_core_tool(
        &mut s.core(),
        "resolve_redline",
        json!({ "id": "RL1", "resolution": "set_params t = 10" }),
        &none,
    )
    .unwrap();
    assert_eq!(done.payload["open"], 1);
    let all = call_core_tool(
        &mut s.core(),
        "list_redlines",
        json!({ "all": true }),
        &none,
    )
    .unwrap();
    assert_eq!(
        all.payload["redlines"][0]["resolution"],
        "set_params t = 10"
    );
    // Yang sudah ditangani tidak lagi digambar.
    let spec = s.design().drawing("sheet1").unwrap().clone();
    let sheet = ducad_engine::drawing_auto::build_sheet(s.model(), &spec, None)
        .unwrap()
        .sheet;
    let pdf = String::from_utf8_lossy(&ducad_io::pdf::generate_pdf_bytes(&sheet)).to_string();
    assert!(!pdf.contains("RL1:") && pdf.contains("(RL2: Tambah champer)"));
}

#[test]
fn diff_reports_which_sheets_checks_and_requirements_a_revision_touches() {
    let source = r#"{ "params": { "t": 8, "h": 20 },
        "ops": [
            {"op":"primitive","id":"plate","shape":{"box":{"size":[60,40,"$t"]}}},
            {"op":"primitive","id":"post","shape":{"box":{"size":[10,10,"$h"]}},"at":[100,0,0]}
        ],
        "checks": [
            {"id":"plate_mass","requirement":"R1","check":"mass","body":"plate","density_g_cm3":2.7,"max_g":60},
            {"id":"post_height","requirement":"R2","check":"bbox_size","body":"post","expect":[10,10,"$h"]}
        ] }"#;
    let brief = json!({ "brief": { "requirements": [
        { "id": "R1", "text": "Plate below 60 g" }, { "id": "R2", "text": "Post height h" }] } });
    let mut a = load(source);
    let mut b = load(source);
    for s in [&mut a, &mut b] {
        call_core_tool(
            &mut s.core(),
            "set_brief",
            brief.clone(),
            &ducad_engine::tooling::NoPaths,
        )
        .unwrap();
    }
    assert!(
        ducad_engine::diff::diff(&a, &b, false).0.impact.is_none(),
        "tanpa perubahan, tanpa dampak"
    );

    // Revisi: plat ditebalkan → massa 64,8 g melewati batas 60 g.
    let mut p = b.design().params.clone();
    p.insert("t".into(), 10.0);
    assert!(b.set_params(p).unwrap().committed);
    let (d, _) = ducad_engine::diff::diff(&a, &b, false);
    let impact = d.impact.expect("revisi berdampak");
    let touched: Vec<(&str, &str)> = impact
        .sheets
        .iter()
        .map(|s| (s.kind, s.name.as_str()))
        .collect();
    assert_eq!(
        touched,
        vec![
            ("cover", ""),
            ("assembly", ""),
            ("part", "plate"),
            ("bom", ""),
            ("compliance", "")
        ],
        "{impact:?}"
    );
    assert_eq!(
        impact.unchanged_sheets, 1,
        "lembar part 'post' tidak terdampak"
    );
    assert_eq!(impact.checks.len(), 1);
    assert_eq!(impact.checks[0].check, "plate_mass");
    assert_eq!(
        impact.checks[0].new,
        Some(ducad_engine::check::CheckStatus::Fail)
    );
    assert_eq!(impact.requirements.len(), 1);
    assert_eq!(impact.requirements[0].id, "R1");

    // Lembar yang tidak terdampak identik byte demi byte di kedua revisi.
    let page = |s: &Session| {
        let out = build(s, &PackageSpec::default(), &FirmProfile::default());
        let p = out
            .pages
            .into_iter()
            .find(|p| p.title == "Part - post")
            .unwrap();
        ducad_io::pdf::generate_pdf_document(&[ducad_io::pdf::PdfPage {
            sheet: &p.sheet,
            extra: &p.groups,
            bookmark: "",
        }])
    };
    assert_eq!(page(&a), page(&b));
    let other = |s: &Session| {
        let out = build(s, &PackageSpec::default(), &FirmProfile::default());
        let p = out
            .pages
            .into_iter()
            .find(|p| p.title == "Part - plate")
            .unwrap();
        ducad_io::pdf::generate_pdf_document(&[ducad_io::pdf::PdfPage {
            sheet: &p.sheet,
            extra: &p.groups,
            bookmark: "",
        }])
    };
    assert_ne!(other(&a), other(&b));
}

#[test]
fn import_pdf_turns_a_drawing_into_a_reference_sketch() {
    // PDF gambar buatan DUCAD sendiri → diimpor kembali sebagai sketsa.
    let mut source = load(include_str!("fixtures/plate.ops.json"));
    let dir = tmp("pdfimport");
    let out = call_core_tool(
        &mut source.core(),
        "drawing",
        json!({ "format": "pdf", "path": "plate.pdf", "sections": [], "dimensions": "none",
                "hidden_lines": false }),
        &TmpPaths(dir.clone()),
    )
    .unwrap();
    assert!(!out.is_error, "{}", out.payload);

    let mut s = Session::new();
    let out = call_core_tool(
        &mut s.core(),
        "import_pdf",
        json!({ "path": "plate.pdf", "sketch": "old_drawing" }),
        &TmpPaths(dir.clone()),
    )
    .unwrap();
    assert!(!out.is_error, "{}", out.payload);
    let p = &out.payload;
    assert_eq!(p["committed"], true);
    assert!(p["polylines"].as_u64().unwrap() > 20, "{p}");
    // Lembar A3 lanskap 420 × 297 mm: bingkai luar menjadi ukuran sketsa.
    let size = p["size_mm"].as_array().unwrap();
    assert!((size[0].as_f64().unwrap() - 420.0).abs() < 0.5, "{p}");
    assert!((size[1].as_f64().unwrap() - 297.0).abs() < 0.5, "{p}");
    assert_eq!(s.design().oplog.len(), 1, "satu op sketch, ikut replay");
    let (sketch, _) = s.sketch("old_drawing").unwrap();
    assert!(sketch.entities.len() > 50);

    // Batas titik, skala, dan berkas yang salah ditolak dengan pesan jelas.
    let err = call_core_tool(
        &mut s.core(),
        "import_pdf",
        json!({ "path": "plate.pdf", "sketch": "too_big", "max_points": 10 }),
        &TmpPaths(dir.clone()),
    )
    .unwrap_err();
    assert!(err.message.contains("max_points"), "{}", err.message);
    assert!(call_core_tool(
        &mut s.core(),
        "import_pdf",
        json!({ "path": "plate.pdf", "sketch": "x", "scale": 0 }),
        &TmpPaths(dir.clone()),
    )
    .is_err());
    std::fs::write(dir.join("scan.pdf"), b"%PDF-1.4\n%%EOF\n").unwrap();
    let err = call_core_tool(
        &mut s.core(),
        "import_pdf",
        json!({ "path": "scan.pdf", "sketch": "x" }),
        &TmpPaths(dir.clone()),
    )
    .unwrap_err();
    assert!(err.message.contains("no vector paths"), "{}", err.message);
    std::fs::remove_dir_all(dir).ok();
}
