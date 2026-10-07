//! Uji asap editor lembar gambar (P21.7): `DrawingSheetView::show` dijalankan
//! tanpa jendela pada lembar nyata (potongan + render berbayang + dimensi
//! asosiatif) dan alat Section menghasilkan event `AddSection`.
//!
//! Ini BUKAN pengganti ceklis manual A15 — hanya menjaga agar jalur gambar
//! dan interaksi dasar tidak panik dan tetap tersambung.

use ducad_io::drawing::{
    render_shaded, DrawingSheet, DrawingSpec, ShadedBody, ShadedCamera, ShadedSpec,
};
use ducad_kernel::{
    make_cylinder, subtract, translate_shape, DrawingOptions, HlrExtractor, ProjectedViewKind,
    SectionAxis, SectionRequest,
};
use ducad_ui::{DrawingSheetEvent, DrawingSheetView, DrawingSheetViewState};
use egui::{pos2, Event, Modifiers, PointerButton, Pos2, RawInput, Rect};

const SCREEN: Rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(1400.0, 900.0));

fn sample_sheet() -> DrawingSheet {
    let disc = make_cylinder(40.0, 12.0).unwrap();
    let bore = translate_shape(&make_cylinder(10.0, 14.0).unwrap(), 0.0, 0.0, -1.0).unwrap();
    let shape = subtract(&disc, &bore).unwrap();
    let mesh = shape.tessellate();
    let bbox = ([-40.0, -40.0, 0.0], [40.0, 40.0, 12.0]);
    let drawing = HlrExtractor::extract_drawing_with(
        &[&shape],
        &[&mesh],
        &[],
        &DrawingOptions {
            sections: vec![SectionRequest::from_axis(
                "A",
                SectionAxis::Y,
                0.0,
                false,
                bbox,
            )],
            default_section: false,
            exact: true,
        },
    );
    let spec = DrawingSpec {
        shaded: vec![ShadedSpec {
            camera: ShadedCamera::Iso,
            px_per_mm: 1,
        }],
        notes: vec!["ALL DIMENSIONS ARE IN MILLIMETERS.".to_string()],
        ..DrawingSpec::default()
    };
    let mut sheet = DrawingSheet::from_spec(drawing, &spec);
    for view in &mut sheet.shaded {
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
    }
    sheet
}

fn input(events: Vec<Event>) -> RawInput {
    RawInput {
        screen_rect: Some(SCREEN),
        events,
        ..Default::default()
    }
}

/// Satu frame editor; mengembalikan event, jumlah bentuk yang dilukis, dan
/// kotak kanvas yang dipakai editor.
fn frame(
    ctx: &egui::Context,
    state: &mut DrawingSheetViewState,
    sheet: &mut DrawingSheet,
    events: Vec<Event>,
) -> (Option<DrawingSheetEvent>, usize, Rect) {
    let mut out = None;
    let mut canvas = SCREEN;
    let full = ctx.run_ui(input(events), |ui| {
        canvas = ui.available_rect_before_wrap();
        out = DrawingSheetView::show(ui, state, sheet);
    });
    let shapes = full.shapes.len();
    // Tidak ada backend: delta tekstur sengaja tidak diterapkan.
    full.drop_without_applying_deltas();
    (out, shapes, canvas)
}

/// Posisi layar untuk titik kertas (mm), sama dengan rumus kanvas editor.
fn paper_to_screen(
    state: &DrawingSheetViewState,
    sheet: &DrawingSheet,
    canvas: Rect,
    p: [f32; 2],
) -> Pos2 {
    let (pw, ph) = sheet.paper_size.dimensions_mm();
    let center = canvas.center() + state.pan_offset;
    let min_x = center.x - pw * state.zoom * 0.5;
    let max_y = center.y + ph * state.zoom * 0.5;
    pos2(min_x + p[0] * state.zoom, max_y - p[1] * state.zoom)
}

fn click(at: Pos2) -> Vec<Vec<Event>> {
    let button = |pressed| Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    vec![
        vec![Event::PointerMoved(at)],
        vec![button(true)],
        vec![button(false)],
        vec![],
    ]
}

#[test]
fn sheet_editor_paints_real_sheet_without_panicking() {
    let ctx = egui::Context::default();
    let mut state = DrawingSheetViewState {
        is_open: true,
        ..Default::default()
    };
    let mut sheet = sample_sheet();
    assert!(!sheet.auto_dimensions.is_empty());

    let mut shapes = 0;
    for _ in 0..3 {
        let (event, n, _) = frame(&ctx, &mut state, &mut sheet, Vec::new());
        assert!(event.is_none());
        shapes = n;
    }
    // Bingkai, tampak (garis + busur), dimensi, catatan, render, bilah alat.
    assert!(shapes > 150, "terlalu sedikit bentuk dilukis: {shapes}");
    assert_eq!(
        state.shaded_textures.len(),
        1,
        "render berbayang menjadi tekstur"
    );

    // Lencana kedaluwarsa dan kartu panduan alat ikut tergambar tanpa panik.
    sheet.stale = true;
    state.measure_tool_active = true;
    let (_, with_overlays, _) = frame(&ctx, &mut state, &mut sheet, Vec::new());
    assert!(with_overlays > shapes);
    state.measure_tool_active = false;
    sheet.stale = false;

    // Skala per tampak lewat API yang dipakai menu konteks.
    sheet.set_view_scale(ProjectedViewKind::Section('A'), Some(0.5));
    let (event, _, _) = frame(&ctx, &mut state, &mut sheet, Vec::new());
    assert!(event.is_none());
}

#[test]
fn section_tool_emits_add_section_with_model_coordinates() {
    let ctx = egui::Context::default();
    let mut state = DrawingSheetViewState {
        is_open: true,
        ..Default::default()
    };
    let mut sheet = sample_sheet();
    frame(&ctx, &mut state, &mut sheet, Vec::new());
    let (_, _, canvas) = frame(&ctx, &mut state, &mut sheet, Vec::new());

    let top = sheet
        .view_placements
        .iter()
        .find(|p| p.kind == ProjectedViewKind::Top)
        .expect("tampak atas")
        .clone();
    state.section_tool_active = true;

    // Dua klik di dalam Tampak Atas: kiri lalu kanan (sedikit miring — alat
    // mengunci ruas menjadi mendatar).
    let a = paper_to_screen(
        &state,
        &sheet,
        canvas,
        [top.center_mm[0] - 20.0 * top.scale, top.center_mm[1] + 3.0],
    );
    let b = paper_to_screen(
        &state,
        &sheet,
        canvas,
        [top.center_mm[0] + 22.0 * top.scale, top.center_mm[1] + 4.0],
    );
    let mut emitted = None;
    for events in click(a).into_iter().chain(click(b)) {
        if let (Some(event), _, _) = frame(&ctx, &mut state, &mut sheet, events) {
            emitted = Some(event);
        }
    }
    let Some(DrawingSheetEvent::AddSection { parent, points }) = emitted else {
        panic!("alat Section tidak menghasilkan AddSection: {emitted:?}");
    };
    assert_eq!(parent, ProjectedViewKind::Top);
    assert_eq!(points.len(), 2);
    assert!(
        (points[0][1] - points[1][1]).abs() < 1e-4,
        "ruas dikunci mendatar: {points:?}"
    );
    assert!(points[0][0] < -5.0 && points[1][0] > 5.0, "{points:?}");
    assert!(!state.section_tool_active && state.section_points.is_empty());
}
