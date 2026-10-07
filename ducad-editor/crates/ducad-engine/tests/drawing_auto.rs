//! Gambar kerja otomatis (P10.1).

use ducad_engine::drawing_auto::{auto_sheet_model, hole_notes, TitleInfo};
use ducad_engine::ops::OpFile;
use ducad_engine::{OpErrorCode, Session};
use ducad_io::drawing::PaperSize;
use ducad_kernel::ProjectedViewKind;

fn plate() -> Session {
    let f: OpFile = serde_json::from_str(include_str!("fixtures/plate.ops.json")).unwrap();
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    assert!(s.run(f.ops, false).committed);
    s
}

#[test]
fn plate_sheet_has_views_dims_notes_and_pdf() {
    let s = plate();
    let notes = hole_notes(s.design(), &s.design().params);
    assert!(notes.iter().any(|n| n.starts_with("4×")), "{notes:?}");
    let title = TitleInfo {
        title: "Plate".into(),
        part_number: "P-001".into(),
        author: "ducad".into(),
        date: "2026-01-01".into(),
        material: "AL6061".into(),
        revision: "A".into(),
    };
    let sheet = auto_sheet_model(s.model(), PaperSize::A3Landscape, &title, &notes).unwrap();
    for kind in [
        ProjectedViewKind::Front,
        ProjectedViewKind::Top,
        ProjectedViewKind::Right,
        ProjectedViewKind::Isometric,
    ] {
        assert!(
            sheet.view_placements.iter().any(|v| v.kind == kind),
            "tampak {kind:?} hilang"
        );
    }
    assert!(!sheet.auto_dimensions.is_empty());
    // P21: catatan disimpan di `sheet.notes` dan digambar di atas kepala gambar.
    assert!(sheet.notes.iter().any(|t| t.contains("4×")));
    assert!(sheet.auto_dimensions.iter().all(|d| d.source.is_some()));
    assert_eq!(sheet.title_block.drawing_number, "P-001");
    assert!(sheet.title_block.scale.contains(':'), "{}", sheet.title_block.scale);
    let pdf = ducad_io::pdf::generate_pdf_bytes(&sheet);
    assert!(pdf.starts_with(b"%PDF"));
}

#[test]
fn empty_session_is_invalid_param() {
    let s = Session::new();
    let err = auto_sheet_model(s.model(), PaperSize::A4Landscape, &TitleInfo::default(), &[])
        .unwrap_err();
    assert_eq!(err.code, OpErrorCode::InvalidParam);
}
