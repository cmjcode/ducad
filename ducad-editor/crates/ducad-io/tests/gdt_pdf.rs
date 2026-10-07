//! Gate P19 `gdt_pdf`: anotasi GD&T/toleransi pada ekspor PDF dan SVG.

use ducad_core::drawing_annot::{
    Annotation, DatumFeature, DimensionTolerance, FeatureControlFrame, GdtSymbol, HoleTable,
    HoleTableRow, MaterialModifier, RevisionRow, RevisionTable, SurfaceFinish,
};
use ducad_core::IsoFit;
use ducad_io::drawing::gdt::{
    annotation_geometry, datum_triangle_paths, diameter_sign_paths, path_signature,
    surface_finish_paths, symbol_path_hash, symbol_paths, PathCmd,
};
use ducad_io::drawing::{BomItem, DimensionAnnotation, DrawingSheet, PaperSize, TextAnnotation};
use ducad_kernel::{
    HlrDrawing, HlrGeometricFeature, HlrLineKind, HlrSegment2D, ProjectedView, ProjectedViewKind,
};

/// FNV-1a 64-bit: hash stabil lintas platform/versi Rust (tidak seperti
/// `DefaultHasher`).
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn sample_drawing() -> HlrDrawing {
    let view = |kind: ProjectedViewKind, w: f32, h: f32| ProjectedView {
        kind,
        title: kind.title_id().to_string(),
        bounds_min: [0.0, 0.0],
        bounds_max: [w, h],
        segments: vec![
            HlrSegment2D {
                start: [0.0, 0.0],
                end: [w, 0.0],
                kind: HlrLineKind::Visible,
            },
            HlrSegment2D {
                start: [w, 0.0],
                end: [w, h],
                kind: HlrLineKind::Visible,
            },
            HlrSegment2D {
                start: [w, h],
                end: [0.0, h],
                kind: HlrLineKind::Visible,
            },
            HlrSegment2D {
                start: [0.0, h],
                end: [0.0, 0.0],
                kind: HlrLineKind::Visible,
            },
            HlrSegment2D {
                start: [w * 0.2, h * 0.3],
                end: [w * 0.8, h * 0.3],
                kind: HlrLineKind::Hidden,
            },
            HlrSegment2D {
                start: [w * 0.1, h * 0.1],
                end: [w * 0.3, h * 0.3],
                kind: HlrLineKind::Hatch,
            },
        ],
        centerlines: vec![HlrSegment2D {
            start: [w * 0.5, -5.0],
            end: [w * 0.5, h + 5.0],
            kind: HlrLineKind::Centerline,
        }],
        features: vec![HlrGeometricFeature::Circle {
            center: [w * 0.5, h * 0.5],
            radius: 6.0,
            edge: None,
        }],
        width_mm: w,
        height_mm: h,
        depth_mm: 20.0,
        ..ProjectedView::default()
    };

    HlrDrawing {
        front: view(ProjectedViewKind::Front, 50.0, 30.0),
        top: view(ProjectedViewKind::Top, 50.0, 20.0),
        right: view(ProjectedViewKind::Right, 20.0, 30.0),
        isometric: view(ProjectedViewKind::Isometric, 45.0, 40.0),
        sections: Vec::new(),
        detail_views: Vec::new(),
        model_bbox_min: [0.0, 0.0, 0.0],
        model_bbox_max: [50.0, 20.0, 30.0],
        warnings: Vec::new(),
    }
}

/// Lembar tanpa anotasi GD&T yang memakai semua entitas lama (dimensi,
/// teks, BOM, balon) — dasar perbandingan "sebelum perubahan".
fn baseline_sheet() -> DrawingSheet {
    let mut sheet = DrawingSheet::new(sample_drawing(), PaperSize::A3Landscape);
    sheet.manual_dimensions.push(DimensionAnnotation {
        start: [40.0, 60.0],
        end: [90.0, 60.0],
        line_pos: [65.0, 52.0],
        is_vertical: false,
        text: "50.00 mm".to_string(),
        ..Default::default()
    });
    sheet.custom_texts.push(TextAnnotation {
        position: [30.0, 250.0],
        text: "CATATAN (UJI)".to_string(),
        font_size: 3.5,
    });
    sheet.bom_table.items.push(BomItem {
        item_number: 1,
        part_name: "Base".to_string(),
        quantity: 2,
        material: "Steel 1045".to_string(),
        description: "Uji".to_string(),
    });
    sheet.add_balloon(
        1,
        [150.0, 120.0],
        [170.0, 140.0],
        ProjectedViewKind::Isometric,
    );
    sheet
}

/// Hash ekspor `baseline_sheet()` TANPA anotasi. Direkam ulang pada P21.0:
/// lembar kini digambar lewat display-list bersama (`drawing::scene`),
/// operator `arc` yang tak sah di PDF diganti kurva Bézier, dan SVG tidak
/// lagi tercermin sumbu-Y. Yang dijaga tes ini tetap sama: menambah anotasi
/// tidak boleh mengubah satu byte pun dari lembar tanpa anotasi.
const BASELINE_PDF: (u64, usize) = (0xb4a7_0fc8_8a66_ee23, 11174);
const BASELINE_SVG: (u64, usize) = (0xde5b_5eaf_aab1_3e60, 18902);

const MM_TO_PT: f32 = 72.0 / 25.4;

fn pdf_text(sheet: &DrawingSheet) -> String {
    String::from_utf8_lossy(&ducad_io::pdf::generate_pdf_bytes(sheet)).into_owned()
}

fn svg_text(sheet: &DrawingSheet) -> String {
    ducad_io::svg::export_drawing_sheet_svg_string(sheet).unwrap_or_else(|e| panic!("{e}"))
}

/// Isi content stream halaman.
fn content_stream(pdf: &str) -> &str {
    let start = pdf.find("stream\n").map(|i| i + "stream\n".len());
    let end = pdf.find("\nendstream");
    match (start, end) {
        (Some(start), Some(end)) if start <= end => &pdf[start..end],
        _ => panic!("PDF tidak memuat content stream"),
    }
}

/// Blok operator yang ditambahkan anotasi = stream beranotasi dikurangi
/// stream lembar yang sama tanpa anotasi.
fn annotation_block(sheet: &DrawingSheet) -> String {
    let mut plain = sheet.clone();
    plain.annotations.clear();
    let plain_pdf = pdf_text(&plain);
    let annotated_pdf = pdf_text(sheet);
    let base = content_stream(&plain_pdf);
    let full = content_stream(&annotated_pdf);
    assert!(
        full.starts_with(base),
        "anotasi harus ditambahkan di akhir stream tanpa mengubah isi lama"
    );
    full[base.len()..].to_string()
}

/// Jumlah path yang dicat (`S` garis, `B` isi+garis) di sebuah blok.
fn painted_paths(block: &str) -> usize {
    block
        .lines()
        .filter(|l| l.ends_with(" S") || l.ends_with(" B"))
        .count()
}

/// String yang ditulis lewat font (`(...) Tj`).
fn shown_strings(block: &str) -> Vec<String> {
    block
        .lines()
        .filter(|l| l.starts_with("BT ") && l.ends_with(" Tj ET"))
        .filter_map(|l| {
            let open = l.find('(')?;
            let close = l.rfind(')')?;
            Some(l[open + 1..close].to_string())
        })
        .collect()
}

fn fit(class: &str) -> IsoFit {
    IsoFit::parse(class).unwrap_or_else(|e| panic!("{e}"))
}

fn position_frame() -> Annotation {
    Annotation::FeatureControlFrame {
        position: [100.0, 200.0],
        frame: FeatureControlFrame {
            symbol: GdtSymbol::Position,
            value: 0.1,
            diameter_zone: true,
            modifiers: vec![MaterialModifier::Mmc],
            datums: vec!["A".to_string(), "B".to_string()],
        },
    }
}

/// Satu anotasi dari tiap jenis.
fn all_kinds() -> Vec<Annotation> {
    let mut diameter = DimensionTolerance::with_limits(12.0, 0.05, 0.05);
    diameter.diameter = true;
    vec![
        position_frame(),
        Annotation::DimensionTolerance {
            position: [60.0, 230.0],
            dimension: DimensionTolerance::with_fit(25.0, fit("H7")),
        },
        Annotation::DimensionTolerance {
            position: [60.0, 240.0],
            dimension: DimensionTolerance::with_limits(10.0, 0.1, 0.2),
        },
        Annotation::DimensionTolerance {
            position: [60.0, 250.0],
            dimension: diameter,
        },
        Annotation::DatumFeature {
            position: [150.0, 230.0],
            datum: DatumFeature {
                label: "A".to_string(),
            },
        },
        Annotation::SurfaceFinish {
            position: [170.0, 230.0],
            finish: SurfaceFinish { ra_um: 3.2 },
        },
        Annotation::HoleTable {
            position: [200.0, 220.0],
            table: HoleTable {
                title: String::new(),
                rows: vec![HoleTableRow {
                    tag: "A1".to_string(),
                    x: 10.0,
                    y: 12.5,
                    diameter: 5.5,
                    depth: None,
                    note: "M6 (uji)".to_string(),
                }],
            },
        },
        Annotation::RevisionTable {
            position: [200.0, 250.0],
            table: RevisionTable {
                rows: vec![RevisionRow {
                    rev: "B".to_string(),
                    description: "Ubah toleransi".to_string(),
                    date: "2026-10-05".to_string(),
                    by: "YJ".to_string(),
                }],
            },
        },
    ]
}

#[test]
fn gdt_pdf_sheet_without_annotations_is_byte_identical_to_before() {
    let sheet = baseline_sheet();
    assert!(sheet.annotations.is_empty());
    let pdf = ducad_io::pdf::generate_pdf_bytes(&sheet);
    assert_eq!(
        (fnv1a(&pdf), pdf.len()),
        BASELINE_PDF,
        "PDF tanpa anotasi berubah: {:#018x}",
        fnv1a(&pdf)
    );
    let svg = svg_text(&sheet);
    assert_eq!(
        (fnv1a(svg.as_bytes()), svg.len()),
        BASELINE_SVG,
        "SVG tanpa anotasi berubah: {:#018x}",
        fnv1a(svg.as_bytes())
    );
}

#[test]
fn gdt_pdf_contains_feature_control_frame() {
    let mut sheet = baseline_sheet();
    sheet.annotations.push(position_frame());
    assert!(sheet.validate_annotations().is_ok());
    let block = annotation_block(&sheet);

    // Blok anotasi berdiri sendiri: state grafik disimpan dan dipulihkan.
    assert!(
        block.starts_with("q 0 0 0 RG 0 0 0 rg 0.99 w [] 0 d 1 j 1 J\n"),
        "{block}"
    );
    assert!(block.ends_with("Q\n"));

    // Angka yang dihitung manual: jangkar (100, 200) mm = (283.46, 566.93) pt;
    // bingkai setinggi 7 mm (586.77 pt), pembatas pertama di x = 107 mm.
    assert!(block.contains("283.46 566.93 m "), "pojok bingkai: {block}");
    assert!(
        block.contains("303.31 566.93 m 303.31 586.77 l S\n"),
        "pembatas setelah kompartemen simbol: {block}"
    );

    // Primitif cocok satu-satu dengan geometri anotasi.
    let geometry = annotation_geometry(&sheet.annotations[0]).unwrap_or_default();
    assert_eq!(painted_paths(&block), geometry.paths.len());
    assert_eq!(geometry.paths.len(), 10);
    let cubics = geometry
        .paths
        .iter()
        .flat_map(|p| &p.cmds)
        .filter(|c| matches!(c, PathCmd::Cubic(..)))
        .count();
    // Lingkaran simbol posisi + tanda diameter + lingkaran M = 3 x 4 kurva.
    assert_eq!(cubics, 12);
    assert_eq!(block.matches(" c ").count(), cubics);
    for path in &geometry.paths {
        if let Some(PathCmd::Move(p)) = path.cmds.first() {
            let start = format!(
                "{:.2} {:.2} m ",
                (100.0 + p[0]) * MM_TO_PT,
                (200.0 + p[1]) * MM_TO_PT
            );
            assert!(block.contains(&start), "path mulai {start} tidak ada");
        }
    }

    // Bingkai luar: persegi tertutup selebar seluruh kompartemen.
    let outer = &geometry.paths[6];
    let PathCmd::Line(corner) = outer.cmds[2] else {
        panic!("bingkai luar bukan persegi");
    };
    assert_eq!(corner[1], 7.0);
    let frame_ops = format!(
        "283.46 566.93 m {x1:.2} 566.93 l {x1:.2} 586.77 l 283.46 586.77 l h S\n",
        x1 = (100.0 + corner[0]) * MM_TO_PT
    );
    assert!(block.contains(&frame_ops), "bingkai luar: {block}");

    // Hanya angka dan huruf yang lewat font; simbol tidak pernah.
    assert_eq!(shown_strings(&block), ["0.1", "M", "A", "B"]);
    assert!(block.is_ascii());
}

#[test]
fn gdt_pdf_symbol_path_hashes_are_stable() {
    // Snapshot geometri tiap simbol ISO 1101. Bila sengaja mengubah bentuk
    // simbol, perbarui angka di sini setelah memeriksa hasilnya secara visual.
    let expected: [(GdtSymbol, u64); 14] = [
        (GdtSymbol::Straightness, 0xaabb_87f3_52e6_a417),
        (GdtSymbol::Flatness, 0x45a2_8dfe_4d93_21e0),
        (GdtSymbol::Circularity, 0xde3c_dfd8_fb5a_4d82),
        (GdtSymbol::Cylindricity, 0x2ebd_b326_47f0_f14e),
        (GdtSymbol::ProfileOfLine, 0x6acc_cbe9_574f_d633),
        (GdtSymbol::ProfileOfSurface, 0x3fc4_6931_cbba_d55d),
        (GdtSymbol::Perpendicularity, 0x4f81_9752_9dbf_ed59),
        (GdtSymbol::Angularity, 0x2b07_cf63_8933_ba20),
        (GdtSymbol::Parallelism, 0xeab5_9f7a_08d6_9b30),
        (GdtSymbol::Position, 0xeaa0_e54b_bc2b_acba),
        (GdtSymbol::Concentricity, 0xec56_9773_3772_9511),
        (GdtSymbol::Symmetry, 0xdddb_e947_7ab6_c77b),
        (GdtSymbol::CircularRunout, 0x5db1_4646_5395_f6ee),
        (GdtSymbol::TotalRunout, 0x9ee2_5659_8780_1505),
    ];
    assert_eq!(expected.map(|(s, _)| s), GdtSymbol::ALL);
    let mut mismatches = Vec::new();
    for (symbol, hash) in expected {
        let actual = symbol_path_hash(symbol);
        if actual != hash {
            mismatches.push(format!("(GdtSymbol::{symbol:?}, {actual:#018x}),"));
        }
    }
    assert!(
        mismatches.is_empty(),
        "hash path simbol berubah:\n{}",
        mismatches.join("\n")
    );

    // Simbol lain yang juga wajib berupa path vektor.
    let others = [
        (
            "diameter",
            path_signature(&diameter_sign_paths([0.0, 0.0], 1.3)),
        ),
        ("datum", path_signature(&datum_triangle_paths())),
        ("finish", path_signature(&surface_finish_paths(10.0))),
    ];
    let expected_others: [(&str, u64); 3] = [
        ("diameter", 0x2932_9026_3364_9db3),
        ("datum", 0x09b2_248a_1396_7e7f),
        ("finish", 0xb929_1cb6_7a2b_6c14),
    ];
    let mut mismatches = Vec::new();
    for ((name, signature), (_, hash)) in others.iter().zip(expected_others) {
        let actual = fnv1a(signature.as_bytes());
        if actual != hash {
            mismatches.push(format!("(\"{name}\", {actual:#018x}),"));
        }
    }
    assert!(
        mismatches.is_empty(),
        "hash path simbol berubah:\n{}",
        mismatches.join("\n")
    );
}

#[test]
fn gdt_pdf_every_symbol_is_drawn_as_vector_paths() {
    for (i, symbol) in GdtSymbol::ALL.into_iter().enumerate() {
        let mut sheet = baseline_sheet();
        let annotation = Annotation::FeatureControlFrame {
            position: [40.0 + 5.0 * i as f64, 150.0],
            frame: FeatureControlFrame {
                symbol,
                value: 0.05,
                diameter_zone: false,
                modifiers: Vec::new(),
                datums: Vec::new(),
            },
        };
        let symbol_path_count = symbol_paths(symbol, [0.0, 0.0], 2.2).len();
        assert!(symbol_path_count >= 1);
        sheet.annotations.push(annotation);
        let block = annotation_block(&sheet);
        // Simbol + bingkai luar + satu pembatas; satu-satunya teks = nilainya.
        assert_eq!(
            painted_paths(&block),
            symbol_path_count + 2,
            "{symbol:?}: {block}"
        );
        assert_eq!(shown_strings(&block), ["0.05"], "{symbol:?}");
        assert!(block.is_ascii(), "{symbol:?}");
    }
}

#[test]
fn gdt_pdf_all_annotation_kinds_render_in_pdf_and_svg() {
    let mut sheet = baseline_sheet();
    sheet.annotations = all_kinds();
    assert!(sheet.validate_annotations().is_ok());

    let block = annotation_block(&sheet);
    let shown = shown_strings(&block);
    for expected in [
        "25 H7 \\(+0.021/0\\)",
        "+0.1",
        "-0.2",
        "0.05",
        "Ra 3.2",
        "HOLE TABLE",
        "THRU",
        "M6 \\(uji\\)",
        "REVISIONS",
        "Ubah toleransi",
    ] {
        assert!(
            shown.iter().any(|s| s == expected),
            "'{expected}' tidak ada di {shown:?}"
        );
    }
    // Segitiga datum dan panah terisi memakai operator isi+garis.
    assert!(block.contains(" h B\n"));

    let total_paths: usize = sheet
        .annotations
        .iter()
        .filter_map(annotation_geometry)
        .map(|g| g.paths.len())
        .sum();
    let total_texts: usize = sheet
        .annotations
        .iter()
        .filter_map(annotation_geometry)
        .map(|g| g.texts.len())
        .sum();
    assert_eq!(painted_paths(&block), total_paths);
    assert_eq!(shown.len(), total_texts);

    // SVG memuat geometri yang sama: jumlah path dan teks identik.
    let mut plain = sheet.clone();
    plain.annotations.clear();
    let plain_svg = svg_text(&plain);
    let svg = svg_text(&sheet);
    assert!(svg.len() > plain_svg.len());
    let group_start = svg
        .find("<g id=\"annotations\"")
        .unwrap_or_else(|| panic!("grup anotasi SVG tidak ada"));
    let group = &svg[group_start..];
    assert_eq!(group.matches("<path d=\"M ").count(), total_paths);
    assert_eq!(group.matches("<text ").count(), total_texts);
    assert!(group.contains(">25 H7 (+0.021/0)</text>"));
    assert!(
        group.contains("fill=\"#111827\" />"),
        "segitiga datum terisi"
    );
    assert!(group.trim_end().ends_with("</g>\n</svg>"));
    assert!(!plain_svg.contains("id=\"annotations\""));
    // Jangkar bingkai (100, 200) mm dari pojok kiri-BAWAH kertas A3 (tinggi
    // 297): di SVG y = 297 − 200 = 97, dan tinggi bingkai 7 mm -> y = 90.
    // Sebelum P21.0 SVG lembar tercermin sumbu-Y terhadap PDF.
    assert!(group.contains("M 100.000 97.000 L "));
    assert!(group.contains(" 90.000 Z"));
}

#[test]
fn gdt_pdf_export_is_deterministic_and_well_formed() {
    let mut sheet = baseline_sheet();
    sheet.annotations = all_kinds();

    let first = ducad_io::pdf::generate_pdf_bytes(&sheet);
    let second = ducad_io::pdf::generate_pdf_bytes(&sheet.clone());
    assert_eq!(first, second, "ekspor PDF dua kali harus identik");
    assert_eq!(svg_text(&sheet), svg_text(&sheet.clone()));

    // /Length sesuai panjang stream sebenarnya dan xref menunjuk objek.
    // (Dikerjakan pada byte: header PDF memuat penanda biner non-UTF-8.)
    let find = |needle: &[u8]| {
        first
            .windows(needle.len())
            .position(|w| w == needle)
            .unwrap_or_else(|| panic!("{:?} tidak ada", String::from_utf8_lossy(needle)))
    };
    let stream_len = find(b"\nendstream") - (find(b"stream\n") + 7);
    find(format!("<< /Length {stream_len} >>").as_bytes());
    let tail = String::from_utf8_lossy(&first[find(b"startxref\n") + 10..]).into_owned();
    let xref_at = tail
        .lines()
        .next()
        .and_then(|l| l.trim().parse::<usize>().ok())
        .unwrap_or_else(|| panic!("startxref tidak terbaca"));
    assert!(first[xref_at..].starts_with(b"xref\n"));
    assert!(first.ends_with(b"%%EOF\n"));
}

#[test]
fn gdt_pdf_invalid_annotations_do_not_break_export() {
    let mut sheet = baseline_sheet();
    sheet.annotations = vec![
        // Jangkar tak hingga: dilewati.
        Annotation::SurfaceFinish {
            position: [f64::NAN, 10.0],
            finish: SurfaceFinish { ra_um: 3.2 },
        },
        // Kelas di luar tabel ISO 286: kelas tampil tanpa batas.
        Annotation::DimensionTolerance {
            position: [60.0, 230.0],
            dimension: DimensionTolerance::with_fit(800.0, fit("H7")),
        },
        // Toleransi bentuk dengan datum: tidak sah tetapi tetap tergambar.
        Annotation::FeatureControlFrame {
            position: [60.0, 240.0],
            frame: FeatureControlFrame {
                symbol: GdtSymbol::Flatness,
                value: -1.0,
                diameter_zone: false,
                modifiers: Vec::new(),
                datums: vec!["A".to_string()],
            },
        },
    ];
    let err = sheet.validate_annotations().unwrap_err();
    assert!(err.contains("#0"), "{err}");

    let block = annotation_block(&sheet);
    let shown = shown_strings(&block);
    assert_eq!(shown, ["800 H7", "-1", "A"]);
    assert!(!block.contains("NaN") && !block.contains("inf"));
    assert!(!svg_text(&sheet).contains("NaN"));
}

#[test]
fn gdt_pdf_sheet_json_keeps_annotations_and_loads_old_files() {
    let mut sheet = baseline_sheet();
    sheet.annotations = all_kinds();
    let json = serde_json::to_value(&sheet).unwrap_or_else(|e| panic!("{e}"));
    let back: DrawingSheet = serde_json::from_value(json.clone()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(back.annotations, sheet.annotations);
    assert_eq!(
        ducad_io::pdf::generate_pdf_bytes(&back),
        ducad_io::pdf::generate_pdf_bytes(&sheet)
    );

    // Berkas lama: tanpa kunci `annotations`.
    let mut old = json;
    let removed = old.as_object_mut().and_then(|o| o.remove("annotations"));
    assert!(removed.is_some());
    let loaded: DrawingSheet = serde_json::from_value(old).unwrap_or_else(|e| panic!("{e}"));
    assert!(loaded.annotations.is_empty());
}
