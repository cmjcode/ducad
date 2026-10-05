//! Tampak terurai: balon BOM otomatis (P20).

use ducad_core::assembly::AssemblyTree;
use ducad_io::drawing::{DrawingSheet, PaperSize};
use ducad_kernel::{
    HlrDrawing, HlrGeometricFeature, HlrLineKind, HlrSegment2D, ProjectedView, ProjectedViewKind,
};

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
        }],
        width_mm: w,
        height_mm: h,
        depth_mm: 20.0,
    };

    HlrDrawing {
        front: view(ProjectedViewKind::Front, 50.0, 30.0),
        top: view(ProjectedViewKind::Top, 50.0, 20.0),
        right: view(ProjectedViewKind::Right, 20.0, 30.0),
        isometric: view(ProjectedViewKind::Isometric, 45.0, 40.0),
        section_a: Some(view(ProjectedViewKind::SectionAA, 50.0, 30.0)),
        cutting_plane: None,
        detail_views: Vec::new(),
        model_bbox_min: [0.0, 0.0, 0.0],
        model_bbox_max: [50.0, 20.0, 30.0],
    }
}

fn assembly() -> AssemblyTree {
    let mut tree = AssemblyTree::default();
    for (name, body, number) in [
        ("Rangka", 1, "FR-001"),
        ("Baut A", 2, "ISO 4762 - M6 x 20"),
        ("Baut B", 2, "ISO 4762 - M6 x 20"),
        ("Tutup", 3, "CV-002"),
    ] {
        let id = tree.add_instance(name, body);
        tree.instances.get_mut(&id).unwrap().part_number = Some(number.to_string());
    }
    tree
}

#[test]
fn exploded_drawing_has_one_balloon_per_bom_row_and_is_deterministic() {
    let rows = assembly().build_bom(true);
    // Dua baut bernomor sama digabung: 3 baris.
    assert_eq!(rows.len(), 3);
    let targets = [[80.0_f32, 120.0], [95.0, 90.0], [70.0, 150.0]];

    let export = || {
        let mut sheet = DrawingSheet::new(sample_drawing(), PaperSize::A3Landscape);
        let made = sheet.set_bom_with_balloons(&rows, &targets);
        assert_eq!(made, rows.len(), "balon = jumlah baris BOM");
        assert_eq!(sheet.bom_table.items.len(), rows.len());
        let path = std::env::temp_dir().join(format!(
            "ducad-exploded-{}-{:?}.pdf",
            std::process::id(),
            std::thread::current().id()
        ));
        ducad_io::pdf::export_pdf(&sheet, &path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).ok();
        (sheet, bytes)
    };
    let (sheet, first) = export();
    let (_, second) = export();
    assert_eq!(first, second, "PDF tampak terurai harus deterministik");
    assert!(first.starts_with(b"%PDF"));

    // Nomor balon = nomor item BOM, masing-masing tepat sekali.
    let mut items: Vec<usize> = sheet.balloons.iter().map(|b| b.item_number).collect();
    items.sort_unstable();
    assert_eq!(
        items,
        rows.iter().map(|r| r.item as usize).collect::<Vec<_>>()
    );
    // Satu kolom di kanan semua sasaran, urut menurun mengikuti tinggi sasaran
    // (garis penunjuk tidak bersilangan).
    let x = sheet.balloons[0].balloon_pos[0];
    assert!(sheet
        .balloons
        .iter()
        .all(|b| b.balloon_pos[0] == x && x > 95.0));
    for pair in sheet.balloons.windows(2) {
        assert!(pair[0].target_point[1] >= pair[1].target_point[1]);
        assert!(pair[0].balloon_pos[1] > pair[1].balloon_pos[1]);
    }

    // Sasaran lebih sedikit dari baris → tabel lengkap, balon seadanya.
    let mut partial = DrawingSheet::new(sample_drawing(), PaperSize::A3Landscape);
    assert_eq!(partial.set_bom_with_balloons(&rows, &targets[..1]), 1);
    assert_eq!(partial.bom_table.items.len(), 3);
    assert_eq!(partial.set_bom_with_balloons(&rows, &[]), 0);
}
