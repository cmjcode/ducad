//! P21: lembar gambar dari solid nyata — display-list bersama, skala per
//! tampak, busur eksak, dimensi asosiatif, dan render berbayang.

use std::sync::Mutex;

use ducad_core::drawing_annot::DimensionRef;
use ducad_io::drawing::scene::{build_scene, Item};
use ducad_io::drawing::{
    render_shaded, DimStyle, DimensionPolicy, DrawingSheet, DrawingSpec, ScaleSpec, ShadedBody,
    ShadedCamera, ShadedSpec,
};
use ducad_kernel::{
    make_cylinder, subtract, translate_shape, DrawingOptions, HlrDrawing, HlrExtractor, KernelMesh,
    KernelShape, ProjectedViewKind, SectionAxis, SectionRequest,
};

/// OCCT tidak aman dipakai paralel dari beberapa tes sekaligus.
static LOCK: Mutex<()> = Mutex::new(());

/// Flange Ø100 × 10 dengan bore Ø30 dan `n` lubang Ø`hole_d` pada PCD Ø70.
fn flange(hole_d: f64) -> KernelShape {
    let mut solid = make_cylinder(50.0, 10.0).unwrap();
    let bore = translate_shape(&make_cylinder(15.0, 12.0).unwrap(), 0.0, 0.0, -1.0).unwrap();
    solid = subtract(&solid, &bore).unwrap();
    for k in 0..4 {
        let a = (45.0 + 90.0 * k as f64).to_radians();
        let hole = translate_shape(
            &make_cylinder(hole_d * 0.5, 12.0).unwrap(),
            35.0 * a.cos(),
            35.0 * a.sin(),
            -1.0,
        )
        .unwrap();
        solid = subtract(&solid, &hole).unwrap();
    }
    solid
}

fn drawing_of(shape: &KernelShape, sections: Vec<SectionRequest>) -> (HlrDrawing, KernelMesh) {
    let mesh = shape.tessellate();
    let drawing = HlrExtractor::extract_drawing_with(
        &[shape],
        &[&mesh],
        &[],
        &DrawingOptions {
            sections,
            default_section: false,
            exact: true,
        },
    );
    (drawing, mesh)
}

fn section_y(drawing_bbox: ([f32; 3], [f32; 3])) -> SectionRequest {
    SectionRequest::from_axis("A", SectionAxis::Y, 0.0, false, drawing_bbox)
}

fn flange_sheet(hole_d: f64) -> DrawingSheet {
    let shape = flange(hole_d);
    let bbox = ([-50.0, -50.0, 0.0], [50.0, 50.0, 10.0]);
    let (drawing, _) = drawing_of(&shape, vec![section_y(bbox)]);
    assert!(drawing.warnings.is_empty(), "{:?}", drawing.warnings);
    let spec = DrawingSpec {
        hidden_lines: false,
        ..DrawingSpec::default()
    };
    DrawingSheet::from_spec(drawing, &spec)
}

fn overlap(a: &[f32; 4], b: &[f32; 4]) -> bool {
    a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]
}

#[test]
fn flange_auto_dimensions_have_pattern_pcd_and_no_overlap() {
    let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let sheet = flange_sheet(10.0);
    let texts: Vec<&str> = sheet
        .auto_dimensions
        .iter()
        .map(|d| d.text.as_str())
        .collect();
    assert!(texts.contains(&"4×Ø10 PCD Ø70"), "{texts:?}");
    assert!(texts.contains(&"Ø100"), "{texts:?}");
    assert!(texts.contains(&"Ø30"), "{texts:?}");
    assert!(texts.contains(&"10"), "tebal flange: {texts:?}");
    // Tiap ukuran hanya sekali di seluruh lembar.
    let mut sorted = texts.clone();
    sorted.sort();
    let before = sorted.len();
    sorted.dedup();
    assert_eq!(before, sorted.len(), "dimensi berulang: {texts:?}");
    // Semua dimensi otomatis punya sumber geometri.
    assert!(sheet.auto_dimensions.iter().all(|d| d.source.is_some()));
    // Tidak ada dua kotak teks dimensi yang beririsan.
    let boxes: Vec<[f32; 4]> = sheet.auto_dimensions.iter().map(|d| d.text_box()).collect();
    for i in 0..boxes.len() {
        for j in (i + 1)..boxes.len() {
            assert!(
                !overlap(&boxes[i], &boxes[j]),
                "teks '{}' menimpa '{}': {:?} vs {:?}",
                texts[i],
                texts[j],
                boxes[i],
                boxes[j]
            );
        }
    }
    // Callout pola lubang membawa lingkaran PCD.
    let pattern = sheet
        .auto_dimensions
        .iter()
        .find(|d| matches!(d.source, Some(DimensionRef::HolePattern { .. })))
        .unwrap();
    assert_eq!(pattern.style, DimStyle::Leader);
    let pcd = pattern.aux_circle.expect("lingkaran PCD");
    assert!((pcd[2] - 35.0 * sheet.scale).abs() < 1e-3);
}

#[test]
fn changing_hole_diameter_updates_text_but_keeps_offsets() {
    let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let mut sheet = flange_sheet(10.0);
    // Pengguna menggeser dua dimensi.
    let pattern_i = sheet
        .auto_dimensions
        .iter()
        .position(|d| matches!(d.source, Some(DimensionRef::HolePattern { .. })))
        .unwrap();
    let outer_i = sheet
        .auto_dimensions
        .iter()
        .position(|d| d.text == "Ø100")
        .unwrap();
    for (i, offset) in [(pattern_i, 31.0f32), (outer_i, 19.0f32)] {
        let d = sheet.auto_dimensions[i].clone();
        let mut moved = sheet
            .resolve_dimension(
                d.source.as_ref().unwrap(),
                offset.copysign(d.offset_mm),
                d.angle_deg,
            )
            .unwrap();
        moved.pinned = true;
        sheet.auto_dimensions[i] = moved;
    }
    let outer_before = sheet.auto_dimensions[outer_i].clone();
    let layout = sheet.to_layout();
    assert_eq!(layout.dimensions.len(), 2);

    // Geometri baru (lubang Ø12), lembar dibangun ulang dari spec + layout.
    let shape = flange(12.0);
    let bbox = ([-50.0, -50.0, 0.0], [50.0, 50.0, 10.0]);
    let (drawing, _) = drawing_of(&shape, vec![section_y(bbox)]);
    let spec = DrawingSpec {
        hidden_lines: false,
        layout: Some(layout),
        ..DrawingSpec::default()
    };
    let rebuilt = DrawingSheet::from_spec(drawing, &spec);
    let pattern = rebuilt
        .auto_dimensions
        .iter()
        .find(|d| matches!(d.source, Some(DimensionRef::HolePattern { .. })))
        .unwrap();
    assert_eq!(pattern.text, "4×Ø12 PCD Ø70");
    assert!((pattern.offset_mm.abs() - 31.0).abs() < 1e-4 && pattern.pinned);
    let outer = rebuilt
        .auto_dimensions
        .iter()
        .find(|d| d.text == "Ø100")
        .unwrap();
    assert_eq!(outer.offset_mm, outer_before.offset_mm);
    assert_eq!(
        outer.line_pos, outer_before.line_pos,
        "dimensi lain tidak boleh bergeser"
    );

    // Jalur langsung: ganti gambar di lembar yang sama lalu segarkan.
    let mut live = sheet.clone();
    live.drawing = rebuilt.drawing.clone();
    assert_eq!(live.refresh_associative_dimensions(), 0);
    assert!(live
        .auto_dimensions
        .iter()
        .any(|d| d.text == "4×Ø12 PCD Ø70"));
    assert_eq!(
        live.auto_dimensions[outer_i].line_pos,
        outer_before.line_pos
    );
}

#[test]
fn pdf_and_svg_share_geometry_and_use_real_curves() {
    let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let sheet = flange_sheet(10.0);
    let scene = build_scene(&sheet);
    let top = scene.group("view_top").expect("grup tampak atas");
    // Lingkaran tampak atas = primitif lingkaran, bukan rantai garis.
    let circles = top
        .items
        .iter()
        .filter(|i| matches!(i, Item::Circle { .. }))
        .count();
    assert!(circles >= 6, "Ø100, Ø30, 4 lubang: {circles}");
    assert!(
        !top.items
            .iter()
            .any(|i| matches!(i, Item::Line { pen, .. } if pen.layer == "VISIBLE")),
        "tampak atas flange tidak punya garis lurus tampak"
    );

    let pdf = String::from_utf8_lossy(&ducad_io::pdf::generate_pdf_bytes(&sheet)).into_owned();
    let svg = ducad_io::svg::export_drawing_sheet_svg_string(&sheet).unwrap();
    // PDF hanya memakai operator PDF 1.4 — `arc` (PostScript) tidak sah.
    assert!(!pdf.contains(" arc"), "operator arc tak sah masih ada");
    assert!(pdf.contains(" c "), "busur harus kurva Bezier");
    assert!(pdf.contains("/WinAnsiEncoding"));
    assert!(
        pdf.contains("(4\\327\\33010 PCD \\33070)"),
        "Ø dan × ditulis sebagai oktal WinAnsi"
    );
    assert!(svg.contains("<circle "));
    assert!(!svg.contains("<polyline"));
    assert!(svg.contains(">4×Ø10 PCD Ø70</text>"));

    // Satu segmen Tampak Depan: koordinat PDF (pt, Y-atas) dan SVG (mm,
    // Y-bawah) harus menunjuk titik kertas yang sama.
    let front = scene.group("view_front").unwrap();
    let (a, b) = front
        .items
        .iter()
        .find_map(|i| match i {
            Item::Line { a, b, pen } if pen.layer == "VISIBLE" => Some((*a, *b)),
            _ => None,
        })
        .expect("segmen tampak depan");
    let pt = 72.0 / 25.4;
    let pdf_op = format!(
        "{:.2} {:.2} m {:.2} {:.2} l S",
        a[0] * pt,
        a[1] * pt,
        b[0] * pt,
        b[1] * pt
    );
    assert!(pdf.contains(&pdf_op), "{pdf_op}");
    let ph = sheet.paper_size.height_mm();
    let svg_el = format!(
        "<line x1=\"{:.3}\" y1=\"{:.3}\" x2=\"{:.3}\" y2=\"{:.3}\"",
        a[0],
        ph - a[1],
        b[0],
        ph - b[1]
    );
    assert!(svg.contains(&svg_el), "{svg_el}");

    // Deterministik.
    assert_eq!(
        ducad_io::pdf::generate_pdf_bytes(&sheet),
        ducad_io::pdf::generate_pdf_bytes(&sheet.clone())
    );
}

#[test]
fn per_view_scale_scales_geometry_but_not_dimension_values() {
    let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let shape = flange(10.0);
    let bbox = ([-50.0, -50.0, 0.0], [50.0, 50.0, 10.0]);
    let (drawing, _) = drawing_of(&shape, vec![section_y(bbox)]);
    let mut spec = DrawingSpec {
        scale: ScaleSpec::Fixed(1.0),
        hidden_lines: false,
        ..DrawingSpec::default()
    };
    spec.views.entry(ProjectedViewKind::Section('A')).scale = Some(0.5);
    spec.views.entry(ProjectedViewKind::Right).visible = Some(false);
    let sheet = DrawingSheet::from_spec(drawing, &spec);
    assert_eq!(sheet.scale, 1.0);
    assert!(!sheet
        .view_placements
        .iter()
        .any(|p| p.kind == ProjectedViewKind::Right));

    let widest = |id: &str| -> f32 {
        build_scene(&sheet)
            .group(id)
            .unwrap()
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Line { a, b, pen } if pen.layer == "VISIBLE" => Some((a[0] - b[0]).abs()),
                _ => None,
            })
            .fold(0.0, f32::max)
    };
    // Sisi atas flange: 100 mm di Depan (1:1); di potongan (1:2) garis
    // terpanjang adalah dinding di antara bore dan tepi: (100−30)/2·0.5 = 17.5.
    assert!(
        (widest("view_front") - 100.0).abs() < 1e-2,
        "{}",
        widest("view_front")
    );
    assert!(
        (widest("view_section_a") - 17.5).abs() < 1e-2,
        "{}",
        widest("view_section_a")
    );

    let svg = ducad_io::svg::export_drawing_sheet_svg_string(&sheet).unwrap();
    assert!(
        svg.contains(">SCALE 1:2</text>"),
        "judul potongan harus menyebut skalanya"
    );
    assert_eq!(
        svg.matches(">SCALE ").count(),
        1,
        "tampak berskala lembar tidak diberi label"
    );

    // Nilai dimensi tetap nilai model walau digambar di tampak 1:2.
    let section_dims: Vec<&str> = sheet
        .auto_dimensions
        .iter()
        .filter(|d| d.view == Some(ProjectedViewKind::Section('A')))
        .map(|d| d.text.as_str())
        .collect();
    assert!(
        !section_dims.iter().any(|t| *t == "50" || *t == "Ø50"),
        "{section_dims:?}"
    );
    let all: Vec<&str> = sheet
        .auto_dimensions
        .iter()
        .map(|d| d.text.as_str())
        .collect();
    assert!(all.contains(&"Ø100") && all.contains(&"Ø30"), "{all:?}");

    // Mengubah skala satu tampak lewat API lembar.
    let mut sheet = sheet;
    sheet.set_view_scale(ProjectedViewKind::Section('A'), None);
    let plc = sheet
        .view_placements
        .iter()
        .find(|p| p.kind == ProjectedViewKind::Section('A'))
        .unwrap();
    assert_eq!(plc.scale, 1.0);
}

#[test]
fn shaded_views_are_embedded_as_images() {
    let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let shape = flange(10.0);
    let (drawing, mesh) = drawing_of(&shape, Vec::new());
    let spec = DrawingSpec {
        shaded: vec![
            ShadedSpec {
                camera: ShadedCamera::Iso,
                px_per_mm: 2,
            },
            ShadedSpec {
                camera: ShadedCamera::IsoBack,
                px_per_mm: 2,
            },
        ],
        dimensions: DimensionPolicy::None,
        ..DrawingSpec::default()
    };
    let mut sheet = DrawingSheet::from_spec(drawing, &spec);
    assert!(sheet.auto_dimensions.is_empty());
    assert_eq!(sheet.shaded.len(), 2);
    let plain = ducad_io::pdf::generate_pdf_bytes(&sheet);
    assert!(!String::from_utf8_lossy(&plain).contains("/Subtype /Image"));

    let (_, inner) = sheet.border_rects_mm();
    for view in &mut sheet.shaded {
        let r = view.rect_mm();
        assert!(
            r[0] >= inner[0] && r[2] <= inner[2] && r[1] >= inner[1] && r[3] <= inner[3],
            "{r:?}"
        );
        let (w, h) = view.pixel_size();
        view.image = render_shaded(
            &[ShadedBody {
                mesh: &mesh,
                color: [0.7, 0.72, 0.76],
            }],
            view.spec.camera,
            w,
            h,
        );
        assert_eq!((view.image.width, view.image.height), (w, h));
    }
    assert!(!overlap(
        &sheet.shaded[0].rect_mm(),
        &sheet.shaded[1].rect_mm()
    ));

    let pdf = ducad_io::pdf::generate_pdf_bytes(&sheet);
    let text = String::from_utf8_lossy(&pdf);
    assert!(pdf.len() > plain.len());
    assert_eq!(
        text.matches("/Subtype /Image").count(),
        2,
        "satu XObject per spec"
    );
    assert!(text.contains("/Im0 Do") && text.contains("/Im1 Do"));
    assert!(text.contains("/Filter /FlateDecode"));
    assert_eq!(
        pdf,
        ducad_io::pdf::generate_pdf_bytes(&sheet.clone()),
        "PDF deterministik"
    );

    let svg = ducad_io::svg::export_drawing_sheet_svg_string(&sheet).unwrap();
    assert_eq!(svg.matches("<image ").count(), 2);
    assert!(svg.contains("href=\"data:image/png;base64,"));
}

#[test]
fn editor_dimensions_are_associative_and_draggable() {
    let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let mut sheet = flange_sheet(10.0);
    let top = sheet
        .view_placements
        .iter()
        .find(|p| p.kind == ProjectedViewKind::Top)
        .unwrap()
        .clone();
    let s = top.scale;
    // Pusat dua lubang bersebelahan di kertas (pusat tampak atas = sumbu flange).
    let k = 35.0 * std::f32::consts::FRAC_1_SQRT_2 * s;
    let (h1, h2) = (
        [top.center_mm[0] - k, top.center_mm[1] + k],
        [top.center_mm[0] + k, top.center_mm[1] + k],
    );
    assert_eq!(sheet.view_at(h1), Some(ProjectedViewKind::Top));

    // Linear antar pusat lubang: asosiatif, nilai model (bukan nilai kertas).
    let linear = sheet.make_linear_dimension(h1, h2).unwrap();
    assert!(matches!(linear.source, Some(DimensionRef::Linear { .. })));
    assert_eq!(linear.text, "49.5");
    assert_eq!(linear.view, Some(ProjectedViewKind::Top));

    // Ø dari klik pusat lalu tepi: menempel ke lingkaran lubang.
    let rim = [h1[0] + 5.0 * s, h1[1]];
    let dia = sheet.make_radial_dimension(h1, rim, false).unwrap();
    assert!(matches!(dia.source, Some(DimensionRef::Diameter { .. })));
    assert_eq!(dia.text, "Ø10");
    let rad = sheet.make_radial_dimension(h1, rim, true).unwrap();
    assert_eq!(rad.text, "R5");

    // Di luar tampak mana pun: dimensi mutlak tanpa sumber, skala lembar.
    let free = sheet
        .make_linear_dimension([25.0, 12.0], [45.0, 12.0])
        .unwrap();
    assert!(free.source.is_none());

    // Menggeser dimensi asosiatif menyimpan offset dan menandainya `pinned`.
    sheet.manual_dimensions.push(linear);
    let before = sheet.manual_dimensions[0].clone();
    sheet.move_dimension(false, 0, [before.line_pos[0], before.line_pos[1] + 9.0]);
    let after = &sheet.manual_dimensions[0];
    assert!(after.pinned && (after.offset_mm - (before.offset_mm + 9.0)).abs() < 1e-3);
    assert_eq!(after.text, before.text);

    // Menggeser tampak: dimensi ikut, offset tetap.
    let offset = after.offset_mm;
    for plc in &mut sheet.view_placements {
        if plc.kind == ProjectedViewKind::Top {
            plc.center_mm[0] += 12.0;
        }
    }
    assert_eq!(sheet.refresh_associative_dimensions(), 0);
    let moved = &sheet.manual_dimensions[0];
    assert!((moved.start[0] - (before.start[0] + 12.0)).abs() < 1e-3);
    assert_eq!(moved.offset_mm, offset);

    // Tata letak (termasuk dimensi manual) bertahan lewat spec → JSON → lembar.
    let layout = sheet.to_layout();
    let json = serde_json::to_string(&layout).unwrap();
    let back: ducad_io::drawing::SheetLayout = serde_json::from_str(&json).unwrap();
    assert_eq!(back, layout);
    assert_eq!(back.manual_dimensions.len(), 1);
}
