//! Generator dan Eksportir Format Vektor 2D SVG (Scalable Vector Graphics) (Fase 11.5).
//!
//! Format SVG mendukung:
//! 1. Ekspor Sketsa 2D (`Sketch`) skala presisi 1:1 (satuan mm) siap kirim ke mesin laser cutting (LightBurn, Glowforge), CNC router, dan software ilustrasi (Inkscape, Illustrator).
//! 2. Ekspor Lembar Kerja Gambar Teknik 2D (`DrawingSheet`) lengkap dengan multi-tampak (Front, Top, Right, Isometric), Section View (pola arsir 45°), Detail View lingkaran, garis dimensi linier/sudut, bingkai kertas ISO, dan Kepala Gambar (Title Block).

use anyhow::{Context, Result};
use ducad_kernel::{HlrLineKind, ProjectedViewKind};
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
        let is_const = match entity {
            Entity::Line { is_construction, .. } => *is_construction,
            Entity::Circle { is_construction, .. } => *is_construction,
            Entity::Arc { is_construction, .. } => *is_construction,
            Entity::Ellipse { is_construction, .. } => *is_construction,
            Entity::Spline { is_construction, .. } => *is_construction,
        };

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
            let is_const = match entity {
                Entity::Line { is_construction, .. } => *is_construction,
                Entity::Circle { is_construction, .. } => *is_construction,
                Entity::Arc { is_construction, .. } => *is_construction,
                Entity::Ellipse { is_construction, .. } => *is_construction,
                Entity::Spline { is_construction, .. } => *is_construction,
            };
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
        let is_const = match entity {
            Entity::Line { is_construction, .. } => *is_construction,
            Entity::Circle { is_construction, .. } => *is_construction,
            Entity::Arc { is_construction, .. } => *is_construction,
            Entity::Ellipse { is_construction, .. } => *is_construction,
            Entity::Spline { is_construction, .. } => *is_construction,
        };
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
pub fn export_drawing_sheet_svg_string(sheet: &DrawingSheet) -> Result<String> {
    let (pw, ph) = sheet.paper_size.dimensions_mm();

    let mut out = String::with_capacity(32 * 1024);

    out.push_str(&format!(
        r##"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<svg xmlns="http://www.w3.org/2000/svg" width="{pw:.1}mm" height="{ph:.1}mm" viewBox="0 0 {pw:.1} {ph:.1}" version="1.1" style="background:#ffffff; font-family:'Segoe UI', Roboto, Helvetica, Arial, sans-serif;">
  <defs>
    <!-- Marker Panah Dimensi -->
    <marker id="dim_arrow_start" viewBox="0 0 10 10" refX="0" refY="5" markerWidth="6" markerHeight="6" orient="auto">
      <path d="M 10 2 L 0 5 L 10 8 Z" fill="#0055aa" />
    </marker>
    <marker id="dim_arrow_end" viewBox="0 0 10 10" refX="10" refY="5" markerWidth="6" markerHeight="6" orient="auto">
      <path d="M 0 2 L 10 5 L 0 8 Z" fill="#0055aa" />
    </marker>
    <!-- Marker Panah Potongan Section A-A -->
    <marker id="section_arrow" viewBox="0 0 10 10" refX="5" refY="5" markerWidth="8" markerHeight="8" orient="auto">
      <path d="M 0 1 L 10 5 L 0 9 Z" fill="#d9534f" />
    </marker>
  </defs>

  <!-- Latar Belakang Kertas -->
  <rect width="{pw:.1}" height="{ph:.1}" fill="#ffffff" />
"##
    ));

    // 1. Bingkai Gambar (Border)
    out.push_str(r##"  <!-- Bingkai Gambar Standar ISO -->"##);
    out.push('\n');
    out.push_str(r##"  <g id="sheet_border" fill="none">"##);
    out.push('\n');

    let (outer, inner) = sheet.border_rects_mm();
    let (ox0, oy0, ox1, oy1) = (outer[0], outer[1], outer[2], outer[3]);
    let (ix0, iy0, ix1, iy1) = (inner[0], inner[1], inner[2], inner[3]);

    out.push_str(&format!(
        r##"    <rect x="{ox0:.2}" y="{oy0:.2}" width="{ow:.2}" height="{oh:.2}" stroke="#111827" stroke-width="0.7" />
    <rect x="{ix0:.2}" y="{iy0:.2}" width="{iw:.2}" height="{ih:.2}" stroke="#111827" stroke-width="0.4" />
"##,
        ow = ox1 - ox0,
        oh = oy1 - oy0,
        iw = ix1 - ix0,
        ih = iy1 - iy0
    ));
    out.push_str("  </g>\n");

    // 2. Kepala Gambar (Title Block)
    let tb = sheet.title_block_rect_mm();
    let (tbx, tby, tbw, tbh) = (tb[0], tb[1], tb[2] - tb[0], tb[3] - tb[1]);
    let info = &sheet.title_block;

    out.push_str(r##"  <!-- Kepala Gambar (Title Block) -->"##);
    out.push('\n');
    out.push_str(&format!(
        r##"  <g id="title_block">
    <rect x="{tbx:.2}" y="{tby:.2}" width="{tbw:.2}" height="{tbh:.2}" fill="#f9fafb" stroke="#111827" stroke-width="0.5" />
    <line x1="{tbx:.2}" y1="{tby_div1:.2}" x2="{tbx_end:.2}" y2="{tby_div1:.2}" stroke="#111827" stroke-width="0.3" />
    <line x1="{tbx:.2}" y1="{tby_div2:.2}" x2="{tbx_end:.2}" y2="{tby_div2:.2}" stroke="#111827" stroke-width="0.3" />
    <line x1="{tbx_mid:.2}" y1="{tby_div2:.2}" x2="{tbx_mid:.2}" y2="{tby_end:.2}" stroke="#111827" stroke-width="0.3" />

    <text x="{tx1:.2}" y="{ty1:.2}" font-size="3.2" font-weight="bold" fill="#1f2937">{comp}</text>
    <text x="{tx1:.2}" y="{ty2:.2}" font-size="3.8" font-weight="bold" fill="#0055aa">{proj}</text>
    <text x="{tx1:.2}" y="{ty3:.2}" font-size="2.4" fill="#4b5563">DIGAMBAR: {drawn} ({date})</text>
    <text x="{tx1:.2}" y="{ty4:.2}" font-size="2.4" fill="#4b5563">MATERIAL: {mat}</text>

    <text x="{tx2:.2}" y="{ty2:.2}" font-size="2.8" font-weight="bold" fill="#111827">NO: {dwg_no}</text>
    <text x="{tx2:.2}" y="{ty3:.2}" font-size="2.4" fill="#4b5563">SKALA: {scale}</text>
    <text x="{tx2:.2}" y="{ty4:.2}" font-size="2.4" fill="#4b5563">SATUAN: {unit} | LBR: {sheet_no}</text>
  </g>
"##,
        tbx = tbx,
        tby = tby,
        tbw = tbw,
        tbh = tbh,
        tbx_end = tbx + tbw,
        tby_end = tby + tbh,
        tby_div1 = tby + 14.0,
        tby_div2 = tby + 28.0,
        tbx_mid = tbx + 85.0,
        tx1 = tbx + 4.0,
        tx2 = tbx + 88.0,
        ty1 = tby + 8.0,
        ty2 = tby + 22.0,
        ty3 = tby + 34.0,
        ty4 = tby + 41.0,
        comp = escape_xml(&info.company_name),
        proj = escape_xml(if info.project_title.is_empty() { "KOMPONEN UTAMA" } else { &info.project_title }),
        drawn = escape_xml(&info.drawn_by),
        date = escape_xml(&info.date),
        mat = escape_xml(&info.material),
        dwg_no = escape_xml(&info.drawing_number),
        scale = escape_xml(&info.scale),
        unit = escape_xml(&info.units),
        sheet_no = escape_xml(&info.sheet_number),
    ));

    // 3. Tampak Proyeksi Geometri dari View Placements
    for plc in &sheet.view_placements {
        if !plc.visible {
            continue;
        }

        let view = sheet.drawing.view_by_kind(plc.kind);
        let center = plc.center_mm;
        let scale = plc.scale;
        let v_center = view.center_2d();
        let view_sz = view.size_2d();

        let view_id = match plc.kind {
            ProjectedViewKind::Top => "view_top".to_string(),
            ProjectedViewKind::Front => "view_front".to_string(),
            ProjectedViewKind::Right => "view_right".to_string(),
            ProjectedViewKind::Isometric => "view_isometric".to_string(),
            ProjectedViewKind::SectionAA => "view_section_a".to_string(),
            ProjectedViewKind::Detail(id) => format!("view_detail_{id}"),
        };

        out.push_str(&format!(
            r##"  <g id="{view_id}">
    <!-- Label Tampak -->
    <text x="{tx:.2}" y="{ty:.2}" font-size="3.0" font-weight="bold" fill="#374151" text-anchor="middle">{title}</text>
"##,
            tx = center[0],
            ty = center[1] + (view_sz[1] * scale * 0.5) + 6.0,
            title = escape_xml(&view.title)
        ));

        // Render Garis HLR (Visible solid, Hidden dashed, Hatch, Centerlines)
        for seg in &view.segments {
            let x1 = center[0] + (seg.start[0] - v_center[0]) * scale;
            let y1 = center[1] + (seg.start[1] - v_center[1]) * scale;
            let x2 = center[0] + (seg.end[0] - v_center[0]) * scale;
            let y2 = center[1] + (seg.end[1] - v_center[1]) * scale;

            match seg.kind {
                HlrLineKind::Visible | HlrLineKind::Silhouette => {
                    out.push_str(&format!(
                        r##"    <line x1="{x1:.3}" y1="{y1:.3}" x2="{x2:.3}" y2="{y2:.3}" stroke="#111827" stroke-width="0.45" stroke-linecap="round" />
"##
                    ));
                }
                HlrLineKind::Hidden => {
                    if sheet.show_hidden_lines {
                        out.push_str(&format!(
                            r##"    <line x1="{x1:.3}" y1="{y1:.3}" x2="{x2:.3}" y2="{y2:.3}" stroke="#9ca3af" stroke-width="0.25" stroke-dasharray="2 1.5" />
"##
                        ));
                    }
                }
                HlrLineKind::Centerline => {
                    if sheet.show_centerlines {
                        out.push_str(&format!(
                            r##"    <line x1="{x1:.3}" y1="{y1:.3}" x2="{x2:.3}" y2="{y2:.3}" stroke="#0088cc" stroke-width="0.25" stroke-dasharray="6 1.5 1.5 1.5" />
"##
                        ));
                    }
                }
                HlrLineKind::Hatch => {
                    if sheet.show_hatch {
                        out.push_str(&format!(
                            r##"    <line x1="{x1:.3}" y1="{y1:.3}" x2="{x2:.3}" y2="{y2:.3}" stroke="#d9534f" stroke-width="0.25" opacity="0.85" />
"##
                        ));
                    }
                }
                HlrLineKind::CuttingPlane => {
                    out.push_str(&format!(
                        r##"    <line x1="{x1:.3}" y1="{y1:.3}" x2="{x2:.3}" y2="{y2:.3}" stroke="#d9534f" stroke-width="0.6" marker-start="url(#section_arrow)" marker-end="url(#section_arrow)" />
"##
                    ));
                }
            }
        }

        // Centerlines tambahan
        if sheet.show_centerlines {
            for cl in &view.centerlines {
                let x1 = center[0] + (cl.start[0] - v_center[0]) * scale;
                let y1 = center[1] + (cl.start[1] - v_center[1]) * scale;
                let x2 = center[0] + (cl.end[0] - v_center[0]) * scale;
                let y2 = center[1] + (cl.end[1] - v_center[1]) * scale;
                out.push_str(&format!(
                    r##"    <line x1="{x1:.3}" y1="{y1:.3}" x2="{x2:.3}" y2="{y2:.3}" stroke="#0088cc" stroke-width="0.2" stroke-dasharray="4 1 1 1" />
"##
                ));
            }
        }

        // Garis potong panah pada Tampak Atas
        if plc.kind == ProjectedViewKind::Top {
            if let Some(ind) = &sheet.drawing.cutting_plane {
                let p1_x = center[0] + (ind.start[0] - v_center[0]) * scale;
                let p1_y = center[1] + (ind.start[1] - v_center[1]) * scale;
                let p2_x = center[0] + (ind.end[0] - v_center[0]) * scale;
                let p2_y = center[1] + (ind.end[1] - v_center[1]) * scale;

                out.push_str(&format!(
                    r##"    <line x1="{p1_x:.3}" y1="{p1_y:.3}" x2="{p2_x:.3}" y2="{p2_y:.3}" stroke="#d9534f" stroke-width="0.7" stroke-dasharray="8 2 2 2" marker-start="url(#section_arrow)" marker-end="url(#section_arrow)" />
"##
                ));

                let lbl1_x = center[0] + (ind.label1_pos[0] - v_center[0]) * scale;
                let lbl1_y = center[1] + (ind.label1_pos[1] - v_center[1]) * scale;
                let lbl2_x = center[0] + (ind.label2_pos[0] - v_center[0]) * scale;
                let lbl2_y = center[1] + (ind.label2_pos[1] - v_center[1]) * scale;

                out.push_str(&format!(
                    r##"    <text x="{lbl1_x:.2}" y="{lbl1_y:.2}" font-size="3.5" font-weight="bold" fill="#d9534f">{lbl}</text>
    <text x="{lbl2_x:.2}" y="{lbl2_y:.2}" font-size="3.5" font-weight="bold" fill="#d9534f">{lbl}</text>
"##,
                    lbl = escape_xml(&ind.label)
                ));
            }
        }

        // Bingkai Lingkaran Detail View
        if let ProjectedViewKind::Detail(_) = plc.kind {
            let r = view_sz[0] * 0.5 * scale;
            out.push_str(&format!(
                r##"    <circle cx="{cx:.3}" cy="{cy:.3}" r="{r:.3}" fill="none" stroke="#2563eb" stroke-width="0.4" stroke-dasharray="3 1.5" />
"##,
                cx = center[0],
                cy = center[1]
            ));
        }

        // Indikator Lingkaran Detail pada Tampak Acuan
        for det in &sheet.drawing.detail_views {
            if det.indicator.parent_view == plc.kind {
                let ind = &det.indicator;
                let cx = center[0] + (ind.center_2d[0] - v_center[0]) * scale;
                let cy = center[1] + (ind.center_2d[1] - v_center[1]) * scale;
                let r = ind.radius_mm * scale;

                out.push_str(&format!(
                    r##"    <circle cx="{cx:.3}" cy="{cy:.3}" r="{r:.3}" fill="none" stroke="#2563eb" stroke-width="0.35" stroke-dasharray="3 1.5" />
"##
                ));

                let lbl_x = center[0] + (ind.label_pos[0] - v_center[0]) * scale;
                let lbl_y = center[1] + (ind.label_pos[1] - v_center[1]) * scale;
                let det_lbl = ind.label.to_string();
                out.push_str(&format!(
                    r##"    <text x="{lbl_x:.2}" y="{lbl_y:.2}" font-size="3.0" font-weight="bold" fill="#2563eb">DETAIL {lbl}</text>
"##,
                    lbl = escape_xml(&det_lbl)
                ));
            }
        }

        out.push_str("  </g>\n");
    }

    // 4. Garis Dimensi dan Anotasi Teks
    out.push_str(r##"  <!-- Anotasi Dimensi Linier & Manual -->"##);
    out.push('\n');
    out.push_str(r##"  <g id="dimensions_layer">"##);
    out.push('\n');

    let dims_to_export: Vec<&crate::drawing::DimensionAnnotation> = if sheet.show_dimensions {
        sheet.auto_dimensions.iter().chain(sheet.manual_dimensions.iter()).collect()
    } else {
        sheet.manual_dimensions.iter().collect()
    };

    for dim in dims_to_export {
        let (x1, y1) = (dim.start[0], dim.start[1]);
        let (x2, y2) = (dim.end[0], dim.end[1]);
        let (lx, ly) = (dim.line_pos[0], dim.line_pos[1]);

        if dim.is_vertical {
            out.push_str(&format!(
                r##"    <line x1="{x1:.2}" y1="{y1:.2}" x2="{lx:.2}" y2="{y1:.2}" stroke="#0055aa" stroke-width="0.25" stroke-dasharray="1 1" />
    <line x1="{x2:.2}" y1="{y2:.2}" x2="{lx:.2}" y2="{y2:.2}" stroke="#0055aa" stroke-width="0.25" stroke-dasharray="1 1" />
    <line x1="{lx:.2}" y1="{y1:.2}" x2="{lx:.2}" y2="{y2:.2}" stroke="#0055aa" stroke-width="0.35" marker-start="url(#dim_arrow_start)" marker-end="url(#dim_arrow_end)" />
    <text x="{tx:.2}" y="{ty:.2}" font-size="2.6" font-weight="bold" fill="#0055aa" text-anchor="middle" transform="rotate(-90 {tx:.2} {ty:.2})">{val}</text>
"##,
                tx = lx - 2.5,
                ty = (y1 + y2) * 0.5,
                val = escape_xml(&dim.text)
            ));
        } else {
            out.push_str(&format!(
                r##"    <line x1="{x1:.2}" y1="{y1:.2}" x2="{x1:.2}" y2="{ly:.2}" stroke="#0055aa" stroke-width="0.25" stroke-dasharray="1 1" />
    <line x1="{x2:.2}" y1="{y2:.2}" x2="{x2:.2}" y2="{ly:.2}" stroke="#0055aa" stroke-width="0.25" stroke-dasharray="1 1" />
    <line x1="{x1:.2}" y1="{ly:.2}" x2="{x2:.2}" y2="{ly:.2}" stroke="#0055aa" stroke-width="0.35" marker-start="url(#dim_arrow_start)" marker-end="url(#dim_arrow_end)" />
    <text x="{tx:.2}" y="{ty:.2}" font-size="2.6" font-weight="bold" fill="#0055aa" text-anchor="middle">{val}</text>
"##,
                tx = (x1 + x2) * 0.5,
                ty = ly - 1.5,
                val = escape_xml(&dim.text)
            ));
        }
    }

    out.push_str("  </g>\n");

    // 5. Tabel BOM (Bill of Materials)
    if sheet.show_bom_table && !sheet.bom_table.items.is_empty() {
        out.push_str(r##"  <!-- Tabel BOM (Bill of Materials ISO 7573) -->"##);
        out.push('\n');
        out.push_str(r##"  <g id="bom_table">"##);
        out.push('\n');

        let tb = sheet.bom_table_rect_mm();
        let (bx, by, bw, bh) = (tb[0], tb[1], tb[2] - tb[0], tb[3] - tb[1]);
        let col_w = sheet.bom_column_widths_mm();
        let title_h = sheet.bom_title_height_mm();
        let header_h = sheet.bom_header_height_mm();
        let row_h = sheet.bom_row_height_mm();

        // Background & Border Luar
        out.push_str(&format!(
            r##"    <rect x="{bx:.2}" y="{by:.2}" width="{bw:.2}" height="{bh:.2}" fill="#ffffff" stroke="#111827" stroke-width="0.5" />
    <rect x="{bx:.2}" y="{by:.2}" width="{bw:.2}" height="{title_h:.2}" fill="#f3f4f6" stroke="#111827" stroke-width="0.35" />
    <text x="{tx_title:.2}" y="{ty_title:.2}" font-size="3.0" font-weight="bold" fill="#111827">{title_str}</text>
    <rect x="{bx:.2}" y="{by_hdr:.2}" width="{bw:.2}" height="{header_h:.2}" fill="#e5e7eb" stroke="#111827" stroke-width="0.35" />
"##,
            tx_title = bx + 4.0,
            ty_title = by + title_h * 0.65,
            title_str = escape_xml(if sheet.bom_table.title.is_empty() { "BILL OF MATERIALS" } else { &sheet.bom_table.title }),
            by_hdr = by + title_h
        ));

        // Header Kolom
        let col_names = ["ITEM", "PART NAME", "QTY", "MATERIAL", "DESCRIPTION"];
        let mut cur_col_x = bx;
        let ty_hdr_text = by + title_h + header_h * 0.65;
        for (i, &name) in col_names.iter().enumerate() {
            let cw = col_w[i];
            let is_center = i == 0 || i == 2;
            let tx = if is_center { cur_col_x + cw * 0.5 } else { cur_col_x + 2.0 };
            let anchor = if is_center { "middle" } else { "start" };
            out.push_str(&format!(
                r##"    <text x="{tx:.2}" y="{ty_hdr_text:.2}" font-size="2.4" font-weight="bold" fill="#1f2937" text-anchor="{anchor}">{name}</text>
"##
            ));
            if i > 0 {
                out.push_str(&format!(
                    r##"    <line x1="{cur_col_x:.2}" y1="{y_div_start:.2}" x2="{cur_col_x:.2}" y2="{y_div_end:.2}" stroke="#111827" stroke-width="0.25" />
"##,
                    y_div_start = by + title_h,
                    y_div_end = by + bh
                ));
            }
            cur_col_x += cw;
        }

        // Baris Item Data
        for (row_idx, item) in sheet.bom_table.items.iter().enumerate() {
            let y_row = by + title_h + header_h + (row_idx as f32 * row_h);
            out.push_str(&format!(
                r##"    <line x1="{bx:.2}" y1="{y_row:.2}" x2="{bx_end:.2}" y2="{y_row:.2}" stroke="#111827" stroke-width="0.2" />
"##,
                bx_end = bx + bw
            ));

            let ty_row_text = y_row + row_h * 0.68;
            let mut cell_x = bx;
            let vals = [
                format!("{}", item.item_number),
                item.part_name.clone(),
                format!("{}", item.quantity),
                item.material.clone(),
                item.description.clone(),
            ];

            for (c_idx, val) in vals.iter().enumerate() {
                let cw = col_w[c_idx];
                let is_center = c_idx == 0 || c_idx == 2;
                let tx = if is_center { cell_x + cw * 0.5 } else { cell_x + 2.0 };
                let anchor = if is_center { "middle" } else { "start" };
                let weight = if is_center { "font-weight=\"bold\" " } else { "" };
                out.push_str(&format!(
                    r##"    <text x="{tx:.2}" y="{ty_row_text:.2}" font-size="2.3" {weight}fill="#111827" text-anchor="{anchor}">{val_esc}</text>
"##,
                    val_esc = escape_xml(val)
                ));
                cell_x += cw;
            }
        }

        out.push_str("  </g>\n");
    }

    // 6. Part Callout Balloons
    if sheet.show_balloons && !sheet.balloons.is_empty() {
        out.push_str(r##"  <!-- Lingkaran Nomor Penunjuk Part (Callout Balloons) -->"##);
        out.push('\n');
        out.push_str(r##"  <g id="callout_balloons">"##);
        out.push('\n');

        for balloon in &sheet.balloons {
            let (tx, ty) = (balloon.target_point[0], balloon.target_point[1]);
            let (bx, by) = (balloon.balloon_pos[0], balloon.balloon_pos[1]);
            let r = balloon.radius_mm;

            let dx = tx - bx;
            let dy = ty - by;
            let len = (dx * dx + dy * dy).sqrt().max(0.1);

            let ex = bx + (dx / len) * r;
            let ey = by + (dy / len) * r;

            // Garis Leader
            out.push_str(&format!(
                r##"    <line x1="{tx:.2}" y1="{ty:.2}" x2="{ex:.2}" y2="{ey:.2}" stroke="#111827" stroke-width="0.35" marker-start="url(#dim_arrow_start)" />
    <circle cx="{bx:.2}" cy="{by:.2}" r="{r:.2}" fill="#ffffff" stroke="#111827" stroke-width="0.4" />
    <text x="{bx:.2}" y="{ty_num:.2}" font-size="3.2" font-weight="bold" fill="#111827" text-anchor="middle">{num}</text>
"##,
                ty_num = by + 1.1,
                num = balloon.item_number
            ));
        }

        out.push_str("  </g>\n");
    }

    // 7. Anotasi Teks Bebas
    if !sheet.custom_texts.is_empty() {
        out.push_str(r##"  <!-- Anotasi Teks Bebas & Catatan Teknis -->"##);
        out.push('\n');
        out.push_str(r##"  <g id="custom_texts">"##);
        out.push('\n');
        for note in &sheet.custom_texts {
            if note.text.trim().is_empty() {
                continue;
            }
            out.push_str(&format!(
                r##"    <text x="{x:.2}" y="{y:.2}" font-size="{fs:.2}" font-weight="bold" fill="#111827">{txt}</text>
"##,
                x = note.position[0],
                y = note.position[1],
                fs = note.font_size,
                txt = escape_xml(&note.text)
            ));
        }
        out.push_str("  </g>\n");
    }

    out.push_str("</svg>\n");

    Ok(out)
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
    use ducad_kernel::HlrExtractor;

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
}

