//! Generator dan Eksportir Format Vektor 2D SVG (Scalable Vector Graphics) (Fase 11.5).
//!
//! Format SVG mendukung:
//! 1. Ekspor Sketsa 2D (`Sketch`) skala presisi 1:1 (satuan mm) siap kirim ke mesin laser cutting (LightBurn, Glowforge), CNC router, dan software ilustrasi (Inkscape, Illustrator).
//! 2. Ekspor Lembar Kerja Gambar Teknik 2D (`DrawingSheet`) lengkap dengan multi-tampak (Front, Top, Right, Isometric), Section View (pola arsir 45°), Detail View lingkaran, garis dimensi linier/sudut, bingkai kertas ISO, dan Kepala Gambar (Title Block).
//! 3. Ekspor Vector Snapshot (`VectorSnapshot`) — tangkapan vektor 2D dari sudut pandang kamera viewport yang sedang dipakai, dalam koordinat piksel. Berbeda dengan dua yang di atas yang bersatuan milimeter kertas, keluaran ini berskala layar dan ditujukan untuk ilustrasi/presentasi, bukan untuk didimensi.

use anyhow::{Context, Result};
use ducad_kernel::{HlrLineKind, VectorSnapshot};
use ducad_sketch::{Entity, Sketch};
use glam::DVec2;
use std::path::Path;

use crate::drawing::DrawingSheet;

/// Opsi konfigurasi ekspor SVG untuk Sketsa 2D.
#[derive(Debug, Clone)]
pub struct SvgSketchOptions {
    /// Margin keliling di luar batas bounding box sketsa (dalam mm). Default: 10.0 mm.
    pub margin_mm: f64,
    /// Ketebalan garis geometri utama (dalam mm). Default: 0.5 mm (atau 0.1 mm untuk laser hairline).
    pub stroke_width_mm: f64,
    /// Warna garis geometri utama dalam format CSS/Hex (mis. "#000000" atau "#0066cc").
    pub stroke_color: String,
    /// Apakah menyertakan garis konstruksi / referensi sketsa.
    pub include_construction: bool,
    /// Warna garis konstruksi. Default: "#e67e22" (oranye) dengan stroke putus-putus.
    pub construction_stroke_color: String,
    /// Mode potong laser (garis potong tipis 0.1 mm warna merah/hitam murni, background transparan).
    pub laser_cut_mode: bool,
}

impl Default for SvgSketchOptions {
    fn default() -> Self {
        Self {
            margin_mm: 10.0,
            stroke_width_mm: 0.5,
            stroke_color: "#1a1a1a".to_string(),
            include_construction: true,
            construction_stroke_color: "#e67e22".to_string(),
            laser_cut_mode: false,
        }
    }
}

impl SvgSketchOptions {
    /// Preset khusus untuk mesin Laser Cutting / CNC (Hairline 0.1 mm, merah potong murni).
    pub fn laser_cut_preset() -> Self {
        Self {
            margin_mm: 5.0,
            stroke_width_mm: 0.1,
            stroke_color: "#ff0000".to_string(),
            include_construction: false,
            construction_stroke_color: "#0000ff".to_string(),
            laser_cut_mode: true,
        }
    }
}

/// Ekspor sketsa 2D ke berkas `.svg`.
pub fn export_sketch_svg(sketch: &Sketch, path: impl AsRef<Path>) -> Result<()> {
    export_sketch_svg_with_options(sketch, path, &SvgSketchOptions::default())
}

/// Ekspor sketsa 2D dengan opsi kustom ke berkas `.svg`.
pub fn export_sketch_svg_with_options(
    sketch: &Sketch,
    path: impl AsRef<Path>,
    options: &SvgSketchOptions,
) -> Result<()> {
    let svg_content = export_sketch_svg_string(sketch, options)?;
    std::fs::write(path.as_ref(), svg_content).with_context(|| {
        format!(
            "Gagal menulis file SVG sketsa ke {}",
            path.as_ref().display()
        )
    })
}

/// Serialisasi sketsa 2D menjadi teks XML SVG utuh.
pub fn export_sketch_svg_string(
    sketch: &Sketch,
    options: &SvgSketchOptions,
) -> Result<String> {
    // 1. Hitung Bounding Box Sketsa
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    let mut has_entities = false;

    for (_, entity) in &sketch.entities {
        let is_const = entity.is_construction();

        if is_const && !options.include_construction {
            continue;
        }

        has_entities = true;

        match entity {
            Entity::Line { start, end, .. } => {
                min_x = min_x.min(start.x).min(end.x);
                min_y = min_y.min(start.y).min(end.y);
                max_x = max_x.max(start.x).max(end.x);
                max_y = max_y.max(start.y).max(end.y);
            }
            Entity::Circle { center, radius, .. } => {
                min_x = min_x.min(center.x - radius);
                min_y = min_y.min(center.y - radius);
                max_x = max_x.max(center.x + radius);
                max_y = max_y.max(center.y + radius);
            }
            Entity::Arc { center, radius, .. } => {
                min_x = min_x.min(center.x - radius);
                min_y = min_y.min(center.y - radius);
                max_x = max_x.max(center.x + radius);
                max_y = max_y.max(center.y + radius);
            }
            Entity::Ellipse { center, radius_x, radius_y, .. } => {
                min_x = min_x.min(center.x - radius_x);
                min_y = min_y.min(center.y - radius_y);
                max_x = max_x.max(center.x + radius_x);
                max_y = max_y.max(center.y + radius_y);
            }
            Entity::Spline { points, .. } => {
                for p in points {
                    min_x = min_x.min(p.x);
                    min_y = min_y.min(p.y);
                    max_x = max_x.max(p.x);
                    max_y = max_y.max(p.y);
                }
            }
            Entity::Path { subpaths, .. } => {
                for sub in subpaths {
                    let (b_min, b_max) = sub.bbox();
                    min_x = min_x.min(b_min.x);
                    min_y = min_y.min(b_min.y);
                    max_x = max_x.max(b_max.x);
                    max_y = max_y.max(b_max.y);
                }
            }
        }
    }

    if !has_entities {
        min_x = 0.0;
        min_y = 0.0;
        max_x = 100.0;
        max_y = 100.0;
    }

    let margin = options.margin_mm;
    let width_mm = (max_x - min_x) + margin * 2.0;
    let height_mm = (max_y - min_y) + margin * 2.0;

    let width_mm = width_mm.max(10.0);
    let height_mm = height_mm.max(10.0);

    // Transformasi koordinat CAD (Y-up) ke SVG (Y-down)
    let to_svg = |pt: DVec2| -> (f64, f64) {
        let sx = pt.x - min_x + margin;
        let sy = max_y - pt.y + margin;
        (sx, sy)
    };

    let mut out = String::with_capacity(4096);
    out.push_str(&format!(
        r##"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<svg xmlns="http://www.w3.org/2000/svg" width="{width_mm:.3}mm" height="{height_mm:.3}mm" viewBox="0 0 {width_mm:.3} {height_mm:.3}" version="1.1">
  <title>DUCAD 2D Vector Sketch</title>
  <desc>Generated by DuCAD CAD/CAM Engine</desc>
"##
    ));

    if !options.laser_cut_mode {
        out.push_str(&format!(
            r##"  <rect width="{width_mm:.3}" height="{height_mm:.3}" fill="#ffffff" />
"##
        ));
    }

    // Layer Geometri Konstruksi
    if options.include_construction {
        out.push_str(r##"  <g id="construction_layer" fill="none" stroke-dasharray="2 1">"##);
        out.push('\n');
        for (_, entity) in &sketch.entities {
            let is_const = entity.is_construction();
            if is_const {
                render_sketch_entity(
                    &mut out,
                    entity,
                    &to_svg,
                    &options.construction_stroke_color,
                    (options.stroke_width_mm * 0.6).max(0.15),
                );
            }
        }
        out.push_str("  </g>\n");
    }

    // Layer Geometri Utama (Solid / Laser Cut)
    out.push_str(r##"  <g id="geometry_layer" fill="none">"##);
    out.push('\n');
    for (_, entity) in &sketch.entities {
        let is_const = entity.is_construction();
        if !is_const {
            render_sketch_entity(
                &mut out,
                entity,
                &to_svg,
                &options.stroke_color,
                options.stroke_width_mm,
            );
        }
    }
    out.push_str("  </g>\n");

    out.push_str("</svg>\n");
    Ok(out)
}

fn render_sketch_entity<F>(
    out: &mut String,
    entity: &Entity,
    to_svg: &F,
    color: &str,
    stroke_w: f64,
) where
    F: Fn(DVec2) -> (f64, f64),
{
    match entity {
        Entity::Line { start, end, .. } => {
            let (x1, y1) = to_svg(*start);
            let (x2, y2) = to_svg(*end);
            out.push_str(&format!(
                r##"    <line x1="{x1:.4}" y1="{y1:.4}" x2="{x2:.4}" y2="{y2:.4}" stroke="{color}" stroke-width="{stroke_w:.3}" stroke-linecap="round" />
"##
            ));
        }
        Entity::Circle { center, radius, .. } => {
            let (cx, cy) = to_svg(*center);
            out.push_str(&format!(
                r##"    <circle cx="{cx:.4}" cy="{cy:.4}" r="{radius:.4}" stroke="{color}" stroke-width="{stroke_w:.3}" />
"##
            ));
        }
        Entity::Arc {
            center,
            radius,
            start_angle,
            end_angle,
            ..
        } => {
            let span = if *end_angle >= *start_angle {
                *end_angle - *start_angle
            } else {
                *end_angle + std::f64::consts::TAU - *start_angle
            };

            let p1 = *center + DVec2::new(radius * start_angle.cos(), radius * start_angle.sin());
            let p2 = *center + DVec2::new(radius * end_angle.cos(), radius * end_angle.sin());
            let (x1, y1) = to_svg(p1);
            let (x2, y2) = to_svg(p2);

            let large_arc = if span > std::f64::consts::PI { 1 } else { 0 };
            out.push_str(&format!(
                r##"    <path d="M {x1:.4} {y1:.4} A {radius:.4} {radius:.4} 0 {large_arc} 0 {x2:.4} {y2:.4}" stroke="{color}" stroke-width="{stroke_w:.3}" stroke-linecap="round" fill="none" />
"##
            ));
        }
        Entity::Ellipse {
            center,
            radius_x,
            radius_y,
            ..
        } => {
            let (cx, cy) = to_svg(*center);
            out.push_str(&format!(
                r##"    <ellipse cx="{cx:.4}" cy="{cy:.4}" rx="{radius_x:.4}" ry="{radius_y:.4}" stroke="{color}" stroke-width="{stroke_w:.3}" />
"##
            ));
        }
        Entity::Spline { points, .. } => {
            if points.len() >= 2 {
                let (first_x, first_y) = to_svg(points[0]);
                let mut path_d = format!("M {first_x:.4} {first_y:.4}");
                for pt in &points[1..] {
                    let (px, py) = to_svg(*pt);
                    path_d.push_str(&format!(" L {px:.4} {py:.4}"));
                }
                out.push_str(&format!(
                    r##"    <path d="{path_d}" stroke="{color}" stroke-width="{stroke_w:.3}" stroke-linecap="round" stroke-linejoin="round" fill="none" />
"##
                ));
            }
        }
        Entity::Path { subpaths, .. } => {
            for sub in subpaths {
                let pts = sub.flatten(0.05);
                if pts.len() >= 2 {
                    let (first_x, first_y) = to_svg(pts[0]);
                    let mut path_d = format!("M {first_x:.4} {first_y:.4}");
                    for pt in &pts[1..] {
                        let (px, py) = to_svg(*pt);
                        path_d.push_str(&format!(" L {px:.4} {py:.4}"));
                    }
                    if sub.closed {
                        path_d.push_str(" Z");
                    }
                    out.push_str(&format!(
                        r##"    <path d="{path_d}" stroke="{color}" stroke-width="{stroke_w:.3}" stroke-linecap="round" stroke-linejoin="round" fill="none" />
"##
                    ));
                }
            }
        }
    }
}

/// Ekspor Dokumen Lembar Kerja 2D (Drawing Sheet) ke file SVG vektor murni.
pub fn export_drawing_sheet_svg(sheet: &DrawingSheet, path: impl AsRef<Path>) -> Result<()> {
    let svg_content = export_drawing_sheet_svg_string(sheet)?;
    std::fs::write(path.as_ref(), svg_content).with_context(|| {
        format!(
            "Gagal menulis file SVG gambar kerja ke {}",
            path.as_ref().display()
        )
    })
}

/// Serialisasi Dokumen Lembar Kerja 2D (Drawing Sheet) menjadi teks SVG lengkap.
///
/// Geometri berasal dari display-list bersama (`drawing::scene`, Y ke atas)
/// dan dibalik sekali di sini (`y_svg = tinggi − y`), sehingga SVG identik
/// dengan PDF — tidak lagi tercermin sumbu-Y (P21.0).
pub fn export_drawing_sheet_svg_string(sheet: &DrawingSheet) -> Result<String> {
    use crate::drawing::scene::{build_scene, Anchor, Item, Pen};

    let scene = build_scene(sheet);
    let (pw, ph) = (scene.width_mm, scene.height_mm);
    let mut out = String::with_capacity(64 * 1024);
    out.push_str(&format!(
        r##"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<svg xmlns="http://www.w3.org/2000/svg" width="{pw:.1}mm" height="{ph:.1}mm" viewBox="0 0 {pw:.1} {ph:.1}" version="1.1" style="background:#ffffff; font-family:Helvetica, Arial, sans-serif;">
  <rect width="{pw:.1}" height="{ph:.1}" fill="#ffffff" />
"##
    ));

    let hex = |c: [u8; 3]| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
    let stroke = |pen: &Pen| {
        let dash: Vec<String> = pen.dash.pattern_mm().iter().map(|d| format!("{d}")).collect();
        let dash = if dash.is_empty() {
            String::new()
        } else {
            format!(r#" stroke-dasharray="{}""#, dash.join(" "))
        };
        format!(
            r#"stroke="{}" stroke-width="{}"{dash}"#,
            hex(pen.color),
            pen.width_mm
        )
    };
    let y = |v: f32| ph - v;

    for group in &scene.groups {
        out.push_str(&format!(
            "  <g id=\"{}\" fill=\"none\" stroke-linecap=\"round\" stroke-linejoin=\"round\">\n",
            escape_xml(&group.id)
        ));
        for item in &group.items {
            match item {
                Item::Line { a, b, pen } => out.push_str(&format!(
                    "    <line x1=\"{:.3}\" y1=\"{:.3}\" x2=\"{:.3}\" y2=\"{:.3}\" {} />\n",
                    a[0],
                    y(a[1]),
                    b[0],
                    y(b[1]),
                    stroke(pen)
                )),
                Item::Arc {
                    center,
                    radius,
                    start_deg,
                    end_deg,
                    pen,
                } => {
                    let (a0, a1) = (start_deg.to_radians(), end_deg.to_radians());
                    let large = if end_deg - start_deg > 180.0 { 1 } else { 0 };
                    // Berlawanan jarum jam di Y-atas = sweep-flag 0 di SVG.
                    out.push_str(&format!(
                        "    <path d=\"M {:.3} {:.3} A {r:.3} {r:.3} 0 {large} 0 {:.3} {:.3}\" {} />\n",
                        center[0] + radius * a0.cos(),
                        y(center[1] + radius * a0.sin()),
                        center[0] + radius * a1.cos(),
                        y(center[1] + radius * a1.sin()),
                        stroke(pen),
                        r = radius
                    ));
                }
                Item::Circle {
                    center,
                    radius,
                    pen,
                    fill,
                } => {
                    let fill = fill.map(|f| format!(" fill=\"{}\"", hex(f))).unwrap_or_default();
                    out.push_str(&format!(
                        "    <circle cx=\"{:.3}\" cy=\"{:.3}\" r=\"{:.3}\"{fill} {} />\n",
                        center[0],
                        y(center[1]),
                        radius,
                        stroke(pen)
                    ));
                }
                Item::Rect { min, max, pen, fill } => {
                    let fill = fill.map(|f| format!(" fill=\"{}\"", hex(f))).unwrap_or_default();
                    let pen = pen.as_ref().map(|p| format!(" {}", stroke(p))).unwrap_or_default();
                    out.push_str(&format!(
                        "    <rect x=\"{:.3}\" y=\"{:.3}\" width=\"{:.3}\" height=\"{:.3}\"{fill}{pen} />\n",
                        min[0],
                        y(max[1]),
                        max[0] - min[0],
                        max[1] - min[1]
                    ));
                }
                Item::Fill { points, color, .. } => {
                    let pts: Vec<String> = points.iter().map(|p| format!("{:.3},{:.3}", p[0], y(p[1]))).collect();
                    out.push_str(&format!(
                        "    <polygon points=\"{}\" fill=\"{}\" />\n",
                        pts.join(" "),
                        hex(*color)
                    ));
                }
                Item::Text {
                    pos,
                    text,
                    size_mm,
                    bold,
                    anchor,
                    angle_deg,
                    ..
                } => {
                    let anchor = match anchor {
                        Anchor::Start => "",
                        Anchor::Middle => " text-anchor=\"middle\"",
                        Anchor::End => " text-anchor=\"end\"",
                    };
                    let weight = if *bold { " font-weight=\"bold\"" } else { "" };
                    let (px, py) = (pos[0], y(pos[1]));
                    let rotate = if angle_deg.abs() < 1e-3 {
                        String::new()
                    } else {
                        format!(" transform=\"rotate({:.2} {px:.3} {py:.3})\"", -angle_deg)
                    };
                    out.push_str(&format!(
                        "    <text x=\"{px:.3}\" y=\"{py:.3}\" font-size=\"{size_mm}\"{weight}{anchor}{rotate} fill=\"#000000\">{}</text>\n",
                        escape_xml(text)
                    ));
                }
                Item::Image { min, max, index } => {
                    if let Some(view) = sheet.shaded.get(*index) {
                        out.push_str(&format!(
                            "    <image x=\"{:.3}\" y=\"{:.3}\" width=\"{:.3}\" height=\"{:.3}\" preserveAspectRatio=\"none\" href=\"{}\" />\n",
                            min[0],
                            y(max[1]),
                            max[0] - min[0],
                            max[1] - min[1],
                            view.image.png_data_uri()
                        ));
                    }
                }
            }
        }
        out.push_str("  </g>\n");
    }

    // Anotasi GD&T & Toleransi (tidak menulis apa pun bila kosong)
    write_sheet_annotations(&mut out, sheet);

    out.push_str("</svg>\n");

    Ok(out)
}

/// Menulis anotasi GD&T/toleransi (P19) sebagai path vektor dari
/// `drawing::gdt` — geometri yang sama dengan ekspor PDF.
///
/// Jangkar anotasi dalam mm dari pojok kiri-BAWAH kertas (sama dengan PDF);
/// di sini dibalik ke koordinat SVG (Y ke bawah).
fn write_sheet_annotations(out: &mut String, sheet: &DrawingSheet) {
    use crate::drawing::gdt::{annotation_geometry, PathCmd, STROKE_MM};

    if sheet.annotations.is_empty() {
        return;
    }

    out.push_str(r##"  <!-- Anotasi GDT dan Toleransi -->"##);
    out.push('\n');
    out.push_str(&format!(
        r##"  <g id="annotations" fill="none" stroke="#111827" stroke-width="{STROKE_MM:.2}" stroke-linecap="round" stroke-linejoin="round">"##
    ));
    out.push('\n');

    for annotation in &sheet.annotations {
        let Some(geometry) = annotation_geometry(annotation) else {
            continue;
        };
        let [ax, ay] = geometry.anchor;
        let ay = sheet.paper_size.height_mm() - ay;
        let pt = |p: &[f32; 2]| format!("{:.3} {:.3}", ax + p[0], ay - p[1]);

        for path in &geometry.paths {
            let mut d = String::new();
            for cmd in &path.cmds {
                if !d.is_empty() {
                    d.push(' ');
                }
                match cmd {
                    PathCmd::Move(a) => d.push_str(&format!("M {}", pt(a))),
                    PathCmd::Line(a) => d.push_str(&format!("L {}", pt(a))),
                    PathCmd::Cubic(a, b, c) => {
                        d.push_str(&format!("C {} {} {}", pt(a), pt(b), pt(c)))
                    }
                    PathCmd::Close => d.push('Z'),
                }
            }
            let fill = if path.filled { r##" fill="#111827""## } else { "" };
            out.push_str(&format!("    <path d=\"{d}\"{fill} />\n"));
        }

        for run in &geometry.texts {
            let anchor = if run.centered { r#" text-anchor="middle""# } else { "" };
            let weight = if run.bold { r#" font-weight="bold""# } else { "" };
            out.push_str(&format!(
                r##"    <text x="{x:.2}" y="{y:.2}" font-size="{fs:.2}"{weight}{anchor} fill="#111827" stroke="none">{txt}</text>
"##,
                x = ax + run.pos[0],
                y = ay - run.pos[1],
                fs = run.size_mm,
                txt = escape_xml(&run.text)
            ));
        }
    }

    out.push_str("  </g>\n");
}

// ---------------------------------------------------------------------------
// Vector Snapshot (tangkapan vektor dari kamera viewport)
// ---------------------------------------------------------------------------

/// Opsi gaya untuk ekspor [`VectorSnapshot`].
///
/// Semua ketebalan dalam PIKSEL, bukan milimeter: koordinat tangkapan adalah
/// piksel viewport, jadi lebar garis harus satu sistem dengannya supaya
/// tampilannya tidak berubah saat berkas diperbesar.
#[derive(Debug, Clone)]
pub struct SvgSnapshotOptions {
    /// Ketebalan garis tampak.
    pub visible_stroke_px: f64,
    /// Ketebalan garis siluet permukaan lengkung.
    pub silhouette_stroke_px: f64,
    /// Ketebalan garis tersembunyi.
    pub hidden_stroke_px: f64,
    /// Warna garis tampak.
    pub visible_color: String,
    /// Warna garis siluet.
    pub silhouette_color: String,
    /// Warna garis tersembunyi.
    pub hidden_color: String,
    /// Pola putus-putus garis tersembunyi (nilai atribut `stroke-dasharray`).
    pub hidden_dasharray: String,
    /// Warna latar. `None` menghasilkan latar transparan — yang biasanya
    /// diinginkan saat gambar ditempel ke dokumen lain.
    pub background: Option<String>,
    /// Tuliskan lapisan garis tersembunyi. Mematikan ini hanya menyembunyikan
    /// lapisannya; untuk tidak menghitungnya sama sekali, matikan
    /// `include_hidden` pada opsi tangkapan di kernel.
    pub include_hidden: bool,
    /// Pola putus-putus untuk garis tampak & siluet (`None` = garis utuh) —
    /// dipakai lapisan "volume hilang" pada render diff.
    pub visible_dasharray: Option<String>,
}

impl Default for SvgSnapshotOptions {
    fn default() -> Self {
        Self {
            visible_stroke_px: 1.4,
            silhouette_stroke_px: 1.1,
            hidden_stroke_px: 0.7,
            visible_color: "#111827".to_string(),
            silhouette_color: "#374151".to_string(),
            hidden_color: "#9ca3af".to_string(),
            hidden_dasharray: "6 4".to_string(),
            background: None,
            include_hidden: true,
            visible_dasharray: None,
        }
    }
}

impl SvgSnapshotOptions {
    /// Preset garis tunggal hitam rata tanpa garis tersembunyi — cocok untuk
    /// gambar garis (line art) yang akan diwarnai ulang di Illustrator/Inkscape.
    pub fn line_art_preset() -> Self {
        Self {
            visible_stroke_px: 1.6,
            silhouette_stroke_px: 1.6,
            hidden_stroke_px: 1.6,
            visible_color: "#000000".to_string(),
            silhouette_color: "#000000".to_string(),
            hidden_color: "#000000".to_string(),
            hidden_dasharray: "6 4".to_string(),
            background: None,
            include_hidden: false,
            visible_dasharray: None,
        }
    }
}

/// Ekspor tangkapan vektor viewport ke berkas `.svg`.
pub fn export_vector_snapshot_svg(snapshot: &VectorSnapshot, path: impl AsRef<Path>) -> Result<()> {
    export_vector_snapshot_svg_with_options(snapshot, path, &SvgSnapshotOptions::default())
}

/// Ekspor tangkapan vektor viewport dengan opsi gaya kustom.
pub fn export_vector_snapshot_svg_with_options(
    snapshot: &VectorSnapshot,
    path: impl AsRef<Path>,
    options: &SvgSnapshotOptions,
) -> Result<()> {
    let svg_content = export_vector_snapshot_svg_string(snapshot, options)?;
    std::fs::write(path.as_ref(), svg_content).with_context(|| {
        format!(
            "Gagal menulis file SVG Vector Snapshot ke {}",
            path.as_ref().display()
        )
    })
}

/// Serialisasi tangkapan vektor viewport menjadi teks XML SVG utuh.
///
/// Garis dikelompokkan per jenis ke dalam tiga `<g>` berlabel, dengan atribut
/// goresan dipasang di grup alih-alih diulang pada tiap `<line>`. Hasilnya
/// jauh lebih ringkas, dan di editor vektor setiap lapisan bisa dipilih atau
/// disembunyikan sekaligus.
pub fn export_vector_snapshot_svg_string(
    snapshot: &VectorSnapshot,
    options: &SvgSnapshotOptions,
) -> Result<String> {
    let w = snapshot.width_px.max(1.0);
    let h = snapshot.height_px.max(1.0);

    let mut out = String::with_capacity(16 * 1024 + snapshot.segments.len() * 64);
    out.push_str(&format!(
        r##"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<svg xmlns="http://www.w3.org/2000/svg" width="{w:.0}" height="{h:.0}" viewBox="0 0 {w:.2} {h:.2}" version="1.1">
  <title>DUCAD Vector Snapshot</title>
"##
    ));

    if let Some(bg) = &options.background {
        out.push_str(&format!(
            r##"  <rect width="{w:.2}" height="{h:.2}" fill="{bg}" />
"##,
            bg = escape_xml(bg)
        ));
    }

    write_snapshot_kinds(&mut out, snapshot, options, "snapshot");
    out.push_str("</svg>\n");
    Ok(out)
}

/// Tulis lapisan tersembunyi (opsional), siluet, lalu tampak dengan id
/// berawalan `prefix`.
fn write_snapshot_kinds(
    out: &mut String,
    snapshot: &VectorSnapshot,
    options: &SvgSnapshotOptions,
    prefix: &str,
) {
    let dash = options.visible_dasharray.as_deref();
    // Garis tersembunyi digambar LEBIH DULU supaya garis tampak menimpanya di
    // titik-titik persilangan, sesuai kelaziman gambar teknik.
    if options.include_hidden {
        write_snapshot_layer(
            out,
            snapshot,
            HlrLineKind::Hidden,
            &format!("{prefix}_hidden"),
            &options.hidden_color,
            options.hidden_stroke_px,
            Some(&options.hidden_dasharray),
        );
    }
    write_snapshot_layer(
        out,
        snapshot,
        HlrLineKind::Silhouette,
        &format!("{prefix}_silhouette"),
        &options.silhouette_color,
        options.silhouette_stroke_px,
        dash,
    );
    write_snapshot_layer(
        out,
        snapshot,
        HlrLineKind::Visible,
        &format!("{prefix}_visible"),
        &options.visible_color,
        options.visible_stroke_px,
        dash,
    );
}

/// Beberapa tangkapan (satu kamera, `viewBox` sama) dalam SATU `<svg>`, satu
/// `<g id="layer_i">` per lapisan — dipakai render diff berwarna. Ukuran dan
/// latar diambil dari lapisan pertama.
pub fn export_vector_snapshot_svg_layers(
    layers: &[(&VectorSnapshot, &SvgSnapshotOptions)],
) -> Result<String> {
    let Some((first, first_opts)) = layers.first() else {
        anyhow::bail!("tidak ada lapisan untuk diekspor");
    };
    let w = first.width_px.max(1.0);
    let h = first.height_px.max(1.0);
    let mut out = String::with_capacity(16 * 1024);
    out.push_str(&format!(
        r##"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<svg xmlns="http://www.w3.org/2000/svg" width="{w:.0}" height="{h:.0}" viewBox="0 0 {w:.2} {h:.2}" version="1.1">
  <title>DUCAD Vector Snapshot</title>
"##
    ));
    if let Some(bg) = &first_opts.background {
        out.push_str(&format!(
            r##"  <rect width="{w:.2}" height="{h:.2}" fill="{bg}" />
"##,
            bg = escape_xml(bg)
        ));
    }
    for (i, (snap, opts)) in layers.iter().enumerate() {
        out.push_str(&format!("  <g id=\"layer_{i}\">\n"));
        write_snapshot_kinds(&mut out, snap, opts, &format!("layer_{i}"));
        out.push_str("  </g>\n");
    }
    out.push_str("</svg>\n");
    Ok(out)
}

/// Tulis satu lapisan `<g>` berisi seluruh segmen berjenis `kind`.
/// Grup dilewati sama sekali bila tidak ada segmen yang cocok, supaya tidak
/// ada lapisan kosong yang mengotori panel objek di editor vektor.
fn write_snapshot_layer(
    out: &mut String,
    snapshot: &VectorSnapshot,
    kind: HlrLineKind,
    id: &str,
    color: &str,
    stroke_px: f64,
    dasharray: Option<&str>,
) {
    let mut segs = snapshot.segments.iter().filter(|s| s.kind == kind).peekable();
    if segs.peek().is_none() {
        return;
    }

    let dash = match dasharray {
        Some(d) if !d.is_empty() => format!(r#" stroke-dasharray="{}""#, escape_xml(d)),
        _ => String::new(),
    };
    out.push_str(&format!(
        r##"  <g id="{id}" fill="none" stroke="{color}" stroke-width="{stroke_px:.2}" stroke-linecap="round"{dash}>
"##,
        color = escape_xml(color)
    ));
    for seg in segs {
        out.push_str(&format!(
            r##"    <line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" />
"##,
            seg.start[0], seg.start[1], seg.end[0], seg.end[1]
        ));
    }
    out.push_str("  </g>\n");
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_kernel::{HlrExtractor, HlrSegment2D};
    use glam::Vec2;

    fn sample_sketch() -> Sketch {
        let mut sk = Sketch::default();
        sk.entities.insert(Entity::line(DVec2::new(0.0, 0.0), DVec2::new(50.0, 0.0)));
        sk.entities.insert(Entity::circle(DVec2::new(25.0, 25.0), 10.0));
        sk.entities.insert(Entity::Arc {
            center: DVec2::new(50.0, 50.0),
            radius: 15.0,
            start_angle: 0.0,
            end_angle: std::f64::consts::FRAC_PI_2,
            is_construction: false,
        });
        sk
    }

    #[test]
    fn test_export_sketch_svg_xml_structure() {
        let sk = sample_sketch();
        let svg = export_sketch_svg_string(&sk, &SvgSketchOptions::default()).unwrap();

        assert!(svg.starts_with(r#"<?xml version="1.0""#));
        assert!(svg.contains("<svg "));
        assert!(svg.contains("<line "));
        assert!(svg.contains("<circle "));
        assert!(svg.contains("<path "));
        assert!(svg.ends_with("</svg>\n"));
    }

    #[test]
    fn test_export_sketch_svg_laser_cut_preset() {
        let sk = sample_sketch();
        let svg = export_sketch_svg_string(&sk, &SvgSketchOptions::laser_cut_preset()).unwrap();

        assert!(svg.contains(r##"stroke="#ff0000""##));
        assert!(svg.contains(r##"stroke-width="0.100""##));
    }

    #[test]
    fn test_export_drawing_sheet_svg() {
        let drawing = HlrExtractor::extract_drawing(&[], &[]);
        let sheet = DrawingSheet::new(drawing, crate::drawing::PaperSize::A4Landscape);
        let svg = export_drawing_sheet_svg_string(&sheet).unwrap();

        assert!(svg.contains(r#"width="297.0mm""#));
        assert!(svg.contains(r#"height="210.0mm""#));
        assert!(svg.contains(r#"id="title_block""#));
        assert!(svg.contains(r#"id="sheet_border""#));
    }

    #[test]
    fn test_export_drawing_sheet_svg_bom_and_balloons() {
        let drawing = HlrExtractor::extract_drawing(&[], &[]);
        let mut sheet = DrawingSheet::new(drawing, crate::drawing::PaperSize::A4Landscape);

        sheet.bom_table.items.push(crate::drawing::BomItem {
            item_number: 1,
            part_name: "Mounting Bracket".to_string(),
            quantity: 2,
            material: "Aluminium 6061-T6".to_string(),
            description: "Front support".to_string(),
        });
        sheet.add_balloon(1, [150.0, 100.0], [170.0, 120.0], ducad_kernel::ProjectedViewKind::Isometric);

        let svg = export_drawing_sheet_svg_string(&sheet).unwrap();
        assert!(svg.contains(r#"id="bom_table""#));
        assert!(svg.contains("Mounting Bracket"));
        assert!(svg.contains("Aluminium 6061-T6"));
        assert!(svg.contains(r#"id="callout_balloons""#));
    }

    fn sample_snapshot() -> VectorSnapshot {
        VectorSnapshot {
            segments: vec![
                HlrSegment2D::new(Vec2::new(10.0, 10.0), Vec2::new(90.0, 10.0), HlrLineKind::Visible),
                HlrSegment2D::new(Vec2::new(90.0, 10.0), Vec2::new(90.0, 60.0), HlrLineKind::Visible),
                HlrSegment2D::new(Vec2::new(10.0, 10.0), Vec2::new(40.0, 30.0), HlrLineKind::Hidden),
                HlrSegment2D::new(Vec2::new(40.0, 30.0), Vec2::new(70.0, 55.0), HlrLineKind::Silhouette),
            ],
            width_px: 800.0,
            height_px: 600.0,
        }
    }

    #[test]
    fn test_export_vector_snapshot_svg_structure() {
        let svg =
            export_vector_snapshot_svg_string(&sample_snapshot(), &SvgSnapshotOptions::default())
                .unwrap();

        assert!(svg.starts_with(r#"<?xml version="1.0""#));
        assert!(svg.ends_with("</svg>\n"));
        // viewBox harus memakai ukuran viewport, bukan kotak pembatas garis.
        assert!(svg.contains(r#"viewBox="0 0 800.00 600.00""#), "{svg}");
        assert!(svg.contains(r#"id="snapshot_visible""#));
        assert!(svg.contains(r#"id="snapshot_hidden""#));
        assert!(svg.contains(r#"id="snapshot_silhouette""#));
        assert_eq!(svg.matches("<line ").count(), 4);
    }

    #[test]
    fn test_vector_snapshot_hidden_layer_is_dashed_and_drawn_first() {
        let svg =
            export_vector_snapshot_svg_string(&sample_snapshot(), &SvgSnapshotOptions::default())
                .unwrap();
        assert!(svg.contains(r#"stroke-dasharray="6 4""#));
        // Garis tampak harus menimpa garis tersembunyi, jadi ditulis belakangan.
        let hidden = svg.find(r#"id="snapshot_hidden""#).unwrap();
        let visible = svg.find(r#"id="snapshot_visible""#).unwrap();
        assert!(hidden < visible);
    }

    #[test]
    fn test_vector_snapshot_can_omit_the_hidden_layer() {
        let opts = SvgSnapshotOptions {
            include_hidden: false,
            ..Default::default()
        };
        let svg = export_vector_snapshot_svg_string(&sample_snapshot(), &opts).unwrap();
        assert!(!svg.contains(r#"id="snapshot_hidden""#));
        assert_eq!(svg.matches("<line ").count(), 3);
    }

    #[test]
    fn test_vector_snapshot_background_is_transparent_by_default() {
        let default_svg =
            export_vector_snapshot_svg_string(&sample_snapshot(), &SvgSnapshotOptions::default())
                .unwrap();
        assert!(!default_svg.contains("<rect "));

        let opts = SvgSnapshotOptions {
            background: Some("#ffffff".to_string()),
            ..Default::default()
        };
        let putih = export_vector_snapshot_svg_string(&sample_snapshot(), &opts).unwrap();
        assert!(putih.contains(r##"<rect width="800.00" height="600.00" fill="#ffffff" />"##));
    }

    #[test]
    fn test_vector_snapshot_skips_empty_layers() {
        let snap = VectorSnapshot {
            segments: vec![HlrSegment2D::new(
                Vec2::new(0.0, 0.0),
                Vec2::new(10.0, 10.0),
                HlrLineKind::Visible,
            )],
            width_px: 100.0,
            height_px: 100.0,
        };
        let svg = export_vector_snapshot_svg_string(&snap, &SvgSnapshotOptions::default()).unwrap();
        assert!(svg.contains(r#"id="snapshot_visible""#));
        assert!(!svg.contains(r#"id="snapshot_hidden""#));
        assert!(!svg.contains(r#"id="snapshot_silhouette""#));
    }

    #[test]
    fn test_vector_snapshot_line_art_preset_is_pure_black() {
        let svg =
            export_vector_snapshot_svg_string(&sample_snapshot(), &SvgSnapshotOptions::line_art_preset())
                .unwrap();
        assert!(svg.contains(r##"stroke="#000000""##));
        assert!(!svg.contains(r#"id="snapshot_hidden""#));
    }

    /// Jalur penuh: solid B-rep asli dari kernel OCCT, berlubang, ditangkap
    /// dari kamera serong, lalu ditulis sebagai SVG.
    ///
    /// Tes unit di `ducad_kernel::vector_snapshot` memakai mesh buatan tangan;
    /// yang ini membuktikan rangkaiannya utuh di atas geometri sungguhan —
    /// termasuk bahwa lubang menghasilkan garis tersembunyi.
    #[test]
    fn test_vector_snapshot_end_to_end_from_real_solid() {
        let _guard = crate::occt_test_lock::LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());

        use ducad_kernel::{Profile, ProfileSegment};
        let sudut = [(0.0, 0.0), (60.0, 0.0), (60.0, 40.0), (0.0, 40.0)];
        let mut tepi = Vec::new();
        for i in 0..4 {
            tepi.push(ProfileSegment::Line {
                start: sudut[i],
                end: sudut[(i + 1) % 4],
            });
        }
        // Plat 60x40 dengan satu lubang tembus 20 mm, dibentuk sekali jalan
        // sebagai face berlubang alih-alih lewat boolean.
        let profil = Profile::WithHoles {
            outer: Box::new(Profile::Loop(tepi)),
            holes: vec![Profile::Circle {
                center: (30.0, 20.0),
                radius: 10.0,
            }],
        };
        let shape = ducad_kernel::extrude_profile(&profil, 30.0)
            .expect("extrude plat berlubang harus berhasil");

        let mesh = shape.tessellate();
        let edges = ducad_kernel::extract_shape_edges(&shape, Some(&mesh));
        assert!(!edges.is_empty(), "solid harus punya rusuk");

        let camera = ducad_kernel::SnapshotCamera {
            eye: glam::Vec3::new(140.0, -150.0, 120.0),
            target: glam::Vec3::new(30.0, 20.0, 15.0),
            up: glam::Vec3::Z,
            fov_y: 45f32.to_radians(),
            width_px: 1200.0,
            height_px: 800.0,
            near: 0.25,
            orthographic: false,
        };
        let snapshot = ducad_kernel::extract_vector_snapshot(
            &camera,
            &[ducad_kernel::SnapshotBody::new(&edges, &mesh)],
            &[],
            &ducad_kernel::SnapshotOptions::default(),
        );

        assert!(
            snapshot.count(HlrLineKind::Visible) > 0,
            "balok berlubang harus punya garis tampak"
        );
        assert!(
            snapshot.count(HlrLineKind::Hidden) > 0,
            "dinding lubang di sisi jauh harus tersembunyi"
        );

        let svg =
            export_vector_snapshot_svg_string(&snapshot, &SvgSnapshotOptions::default()).unwrap();
        assert!(svg.contains(r#"viewBox="0 0 1200.00 800.00""#));
        assert!(svg.contains(r#"id="snapshot_visible""#));
        assert!(svg.contains(r#"id="snapshot_hidden""#));

        // Berkas benar-benar bisa ditulis ke disk.
        let path = std::env::temp_dir().join("ducad_snapshot_e2e_test.svg");
        export_vector_snapshot_svg(&snapshot, &path).unwrap();
        let ditulis = std::fs::read_to_string(&path).unwrap();
        assert_eq!(ditulis, svg);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_empty_vector_snapshot_still_produces_valid_svg() {
        let snap = VectorSnapshot {
            segments: Vec::new(),
            width_px: 640.0,
            height_px: 480.0,
        };
        let svg = export_vector_snapshot_svg_string(&snap, &SvgSnapshotOptions::default()).unwrap();
        assert!(svg.contains(r#"viewBox="0 0 640.00 480.00""#));
        assert!(svg.ends_with("</svg>\n"));
        assert!(!svg.contains("<line "));
    }
}
