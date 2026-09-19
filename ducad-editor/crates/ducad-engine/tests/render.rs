//! Render SVG/PNG (P2.3).

use ducad_engine::ops::OpFile;
use ducad_engine::render::{render_svg, RenderOptions, View};
use ducad_engine::{OpErrorCode, Session};

fn plate() -> Session {
    let f: OpFile = serde_json::from_str(include_str!("fixtures/plate.ops.json")).unwrap();
    let mut s = Session::new();
    assert!(s.set_params(f.params).unwrap().committed);
    assert!(s.run(f.ops, false).committed);
    s
}

/// bbox (minx, miny, maxx, maxy) semua `<line>` di SVG.
fn line_bbox(svg: &str) -> [f64; 4] {
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for line in svg.lines().filter(|l| l.trim_start().starts_with("<line")) {
        let attr = |k: &str| -> f64 {
            let pat = format!("{k}=\"");
            let i = line.find(&pat).unwrap() + pat.len();
            line[i..].split('"').next().unwrap().parse().unwrap()
        };
        for (x, y) in [(attr("x1"), attr("y1")), (attr("x2"), attr("y2"))] {
            b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
        }
    }
    b
}

#[test]
fn iso_render_has_visible_lines() {
    let s = plate();
    let r = render_svg(&s, &RenderOptions::default()).unwrap();
    assert!(r.svg.starts_with("<svg"), "{}", &r.svg[..60]);
    assert!(r.visible_segments > 0);
    assert_eq!(r.hidden_segments, 0);
    let with_hidden = render_svg(
        &s,
        &RenderOptions {
            hidden_lines: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(with_hidden.hidden_segments > 0);
}

#[test]
fn top_view_keeps_aspect_ratio_and_fills_canvas() {
    let s = plate();
    let r = render_svg(
        &s,
        &RenderOptions {
            view: View::Top,
            ..Default::default()
        },
    )
    .unwrap();
    let [x0, y0, x1, y1] = line_bbox(&r.svg);
    let (w, h) = (x1 - x0, y1 - y0);
    let ratio = w / h;
    assert!(
        (ratio - 1.5).abs() / 1.5 < 0.05,
        "rasio {ratio} ({w} x {h})"
    );
    let fill = (w / 800.0).max(h / 600.0);
    assert!((0.5..=0.95).contains(&fill), "mengisi {fill}");
}

#[test]
fn empty_session_is_invalid_param() {
    let s = Session::new();
    let err = render_svg(&s, &RenderOptions::default()).unwrap_err();
    assert_eq!(err.code, OpErrorCode::InvalidParam);
}

#[cfg(feature = "raster")]
#[test]
fn png_is_800x600_and_not_blank() {
    let s = plate();
    let r = render_svg(&s, &RenderOptions::default()).unwrap();
    let png = ducad_engine::render::svg_to_png(&r.svg, 800, 600).unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let pix = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
    assert_eq!((pix.width(), pix.height()), (800, 600));
    let dark = pix.pixels().iter().filter(|p| p.red() < 128).count();
    assert!(dark > 100, "hanya {dark} piksel gelap");
}
