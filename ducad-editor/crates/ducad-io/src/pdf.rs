//! Generator PDF Vektor Resolusi Tinggi untuk Lembar Kerja Gambar Teknik 2D (Engineering Drawing Sheets).
//!
//! Menghasilkan file PDF standar (PDF 1.4 compliant) murni tanpa dependensi eksternal.
//! Mendukung gambar garis tampak tebal (0.5mm solid), garis tersembunyi (0.25mm dashed),
//! garis sumbu simetri (0.25mm dash-dot `— · —`), bingkai gambar ISO dengan grid zona,
//! kepala gambar (title block), panah dan teks dimensi, serta simbol proyeksi sudut ketiga.

use anyhow::{Context, Result};
use std::io::Write;
use std::path::Path;

use crate::drawing::scene::{build_scene, text_width, Anchor, Item, Pen, Scene};
use crate::drawing::DrawingSheet;

const MM_TO_PT: f32 = 72.0 / 25.4; // 1 mm = ~2.83465 points

pub struct PdfWriter {
    buffer: Vec<u8>,
    offsets: Vec<usize>,
}

impl Default for PdfWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl PdfWriter {
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(32 * 1024),
            offsets: Vec::new(),
        }
    }

    fn write_header(&mut self) {
        self.buffer.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
    }

    fn add_object(&mut self, content: &str) -> usize {
        let obj_num = self.offsets.len() + 1;
        self.offsets.push(self.buffer.len());
        writeln!(self.buffer, "{obj_num} 0 obj").unwrap();
        self.buffer.extend_from_slice(content.as_bytes());
        writeln!(self.buffer, "\nendobj").unwrap();
        obj_num
    }

    fn add_stream_object(&mut self, stream_data: &[u8]) -> usize {
        let obj_num = self.offsets.len() + 1;
        self.offsets.push(self.buffer.len());
        let len = stream_data.len();
        writeln!(self.buffer, "{obj_num} 0 obj\n<< /Length {len} >>\nstream").unwrap();
        self.buffer.extend_from_slice(stream_data);
        writeln!(self.buffer, "\nendstream\nendobj").unwrap();
        obj_num
    }

    /// Stream biner dengan entri kamus tambahan (mis. XObject gambar).
    fn add_raw_stream(&mut self, dict: &str, data: &[u8]) -> usize {
        let obj_num = self.offsets.len() + 1;
        self.offsets.push(self.buffer.len());
        let len = data.len();
        writeln!(self.buffer, "{obj_num} 0 obj\n<< {dict} /Length {len} >>\nstream").unwrap();
        self.buffer.extend_from_slice(data);
        writeln!(self.buffer, "\nendstream\nendobj").unwrap();
        obj_num
    }

    /// Nomor objek berikutnya yang akan dialokasikan.
    fn next_object_number(&self) -> usize {
        self.offsets.len() + 1
    }

    fn finalize(mut self, root_obj: usize) -> Vec<u8> {
        let xref_offset = self.buffer.len();
        let total_objs = self.offsets.len() + 1;

        writeln!(self.buffer, "xref\n0 {total_objs}").unwrap();
        writeln!(self.buffer, "0000000000 65535 f ").unwrap();
        for offset in &self.offsets {
            writeln!(self.buffer, "{offset:010} 00000 n ").unwrap();
        }

        writeln!(
            self.buffer,
            "trailer\n<< /Size {total_objs} /Root {root_obj} 0 R >>\nstartxref\n{xref_offset}\n%%EOF"
        )
        .unwrap();

        self.buffer
    }
}

/// Ekspor Dokumen Drawing Sheet ke file PDF vektor murni.
pub fn export_pdf(sheet: &DrawingSheet, path: impl AsRef<Path>) -> Result<()> {
    let pdf_bytes = generate_pdf_bytes(sheet);
    std::fs::write(path, pdf_bytes).context("gagal menulis file PDF gambar teknik")?;
    Ok(())
}

/// Menghasilkan raw bytes PDF vektor dari sebuah DrawingSheet.
///
/// Geometri berasal dari display-list bersama (`drawing::scene`), sehingga
/// PDF dan SVG selalu identik. Gambar raster (render berbayang) disematkan
/// sebagai XObject `/Image` ber-`/FlateDecode`.
pub fn generate_pdf_bytes(sheet: &DrawingSheet) -> Vec<u8> {
    let mut writer = PdfWriter::new();
    writer.write_header();

    let (pw_mm, ph_mm) = sheet.paper_size.dimensions_mm();
    let pw_pt = pw_mm * MM_TO_PT;
    let ph_pt = ph_mm * MM_TO_PT;

    let scene = build_scene(sheet);
    let mut stream = String::with_capacity(64 * 1024);
    stream.push_str(&format!("q 1 1 1 rg 0 0 {pw_pt:.2} {ph_pt:.2} re f Q\n"));
    let mut images: Vec<usize> = Vec::new();
    render_scene(&mut stream, &scene, &mut images);

    // Anotasi GD&T & Toleransi (tidak menulis apa pun bila kosong) — selalu
    // paling akhir di content stream.
    render_annotations(&mut stream, sheet);

    let encoding = "/Encoding /WinAnsiEncoding";
    let font1_obj = writer.add_object(&format!("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica {encoding} >>"));
    let font2_obj = writer.add_object(&format!("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold {encoding} >>"));
    let font3_obj = writer.add_object(&format!("<< /Type /Font /Subtype /Type1 /BaseFont /Courier {encoding} >>"));

    // Content stream harus menjadi stream PERTAMA di berkas.
    let stream_obj = writer.add_stream_object(stream.as_bytes());

    let mut xobjects = String::new();
    for index in &images {
        let Some(view) = sheet.shaded.get(*index) else {
            continue;
        };
        let data = view.image.zlib_rgb();
        let obj = writer.add_raw_stream(
            &format!(
                "/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode",
                view.image.width, view.image.height
            ),
            &data,
        );
        xobjects.push_str(&format!("/Im{index} {obj} 0 R "));
    }
    let xobject_res = if xobjects.is_empty() {
        String::new()
    } else {
        format!(" /XObject << {}>>", xobjects)
    };

    let pages_num = writer.next_object_number() + 1;
    let page_obj = writer.add_object(&format!(
        "<< /Type /Page /Parent {pages_num} 0 R /MediaBox [0 0 {pw_pt:.2} {ph_pt:.2}] /Contents {stream_obj} 0 R /Resources << /Font << /F1 {font1_obj} 0 R /F2 {font2_obj} 0 R /F3 {font3_obj} 0 R >>{xobject_res} >> >>"
    ));
    let pages_obj = writer.add_object(&format!("<< /Type /Pages /Kids [{page_obj} 0 R] /Count 1 >>"));
    let catalog_obj = writer.add_object(&format!("<< /Type /Catalog /Pages {pages_obj} 0 R >>"));

    writer.finalize(catalog_obj)
}

fn mm_to_pt(val_mm: f32) -> f32 {
    val_mm * MM_TO_PT
}

fn rgb(c: [u8; 3]) -> String {
    let f = |v: u8| {
        let t = format!("{:.3}", v as f32 / 255.0);
        t.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    format!("{} {} {}", f(c[0]), f(c[1]), f(c[2]))
}

fn set_pen(s: &mut String, current: &mut Option<Pen>, pen: Pen) {
    if *current == Some(pen) {
        return;
    }
    let dash: Vec<String> = pen
        .dash
        .pattern_mm()
        .iter()
        .map(|d| format!("{:.2}", mm_to_pt(*d)))
        .collect();
    s.push_str(&format!(
        "{} RG {:.2} w [{}] 0 d\n",
        rgb(pen.color),
        mm_to_pt(pen.width_mm),
        dash.join(" ")
    ));
    *current = Some(pen);
}

/// Lintasan busur lingkaran sebagai kurva Bézier kubik (≤ 90° per ruas).
/// PDF tidak punya operator busur; `arc` adalah PostScript, bukan PDF.
fn arc_path(s: &mut String, cx: f32, cy: f32, r: f32, start_deg: f32, end_deg: f32) {
    let sweep = (end_deg - start_deg).clamp(0.0, 360.0);
    let n = ((sweep / 90.0).ceil() as usize).max(1);
    let step = (sweep / n as f32).to_radians();
    let k = 4.0 / 3.0 * (step / 4.0).tan() * r;
    let mut a = start_deg.to_radians();
    s.push_str(&format!("{:.2} {:.2} m ", cx + r * a.cos(), cy + r * a.sin()));
    for _ in 0..n {
        let b = a + step;
        let (p0x, p0y) = (cx + r * a.cos(), cy + r * a.sin());
        let (p3x, p3y) = (cx + r * b.cos(), cy + r * b.sin());
        s.push_str(&format!(
            "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c ",
            p0x - k * a.sin(),
            p0y + k * a.cos(),
            p3x + k * b.sin(),
            p3y - k * b.cos(),
            p3x,
            p3y
        ));
        a = b;
    }
}

fn render_scene(s: &mut String, scene: &Scene, images: &mut Vec<usize>) {
    for group in &scene.groups {
        if group.items.is_empty() {
            continue;
        }
        s.push_str("q 1 j 1 J\n");
        let mut pen_state: Option<Pen> = None;
        for item in &group.items {
            match item {
                Item::Line { a, b, pen } => {
                    set_pen(s, &mut pen_state, *pen);
                    s.push_str(&format!(
                        "{:.2} {:.2} m {:.2} {:.2} l S\n",
                        mm_to_pt(a[0]),
                        mm_to_pt(a[1]),
                        mm_to_pt(b[0]),
                        mm_to_pt(b[1])
                    ));
                }
                Item::Arc {
                    center,
                    radius,
                    start_deg,
                    end_deg,
                    pen,
                } => {
                    set_pen(s, &mut pen_state, *pen);
                    arc_path(s, mm_to_pt(center[0]), mm_to_pt(center[1]), mm_to_pt(*radius), *start_deg, *end_deg);
                    s.push_str("S\n");
                }
                Item::Circle {
                    center,
                    radius,
                    pen,
                    fill,
                } => {
                    let (cx, cy, r) = (mm_to_pt(center[0]), mm_to_pt(center[1]), mm_to_pt(*radius));
                    if let Some(fill) = fill {
                        s.push_str(&format!("{} rg ", rgb(*fill)));
                        arc_path(s, cx, cy, r, 0.0, 360.0);
                        s.push_str("h f\n");
                    }
                    set_pen(s, &mut pen_state, *pen);
                    arc_path(s, cx, cy, r, 0.0, 360.0);
                    s.push_str("h S\n");
                }
                Item::Rect { min, max, pen, fill } => {
                    let (x, y) = (mm_to_pt(min[0]), mm_to_pt(min[1]));
                    let (w, h) = (mm_to_pt(max[0] - min[0]), mm_to_pt(max[1] - min[1]));
                    if let Some(fill) = fill {
                        s.push_str(&format!("{} rg {x:.2} {y:.2} {w:.2} {h:.2} re f\n", rgb(*fill)));
                    }
                    if let Some(pen) = pen {
                        set_pen(s, &mut pen_state, *pen);
                        s.push_str(&format!("{x:.2} {y:.2} {w:.2} {h:.2} re S\n"));
                    }
                }
                Item::Fill { points, color, .. } => {
                    if points.len() < 3 {
                        continue;
                    }
                    s.push_str(&format!("{} rg ", rgb(*color)));
                    for (i, p) in points.iter().enumerate() {
                        s.push_str(&format!(
                            "{:.2} {:.2} {} ",
                            mm_to_pt(p[0]),
                            mm_to_pt(p[1]),
                            if i == 0 { "m" } else { "l" }
                        ));
                    }
                    s.push_str("h f\n");
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
                    let shift = match anchor {
                        Anchor::Start => 0.0,
                        Anchor::Middle => -0.5 * text_width(text, *size_mm),
                        Anchor::End => -text_width(text, *size_mm),
                    };
                    let (sin, cos) = angle_deg.to_radians().sin_cos();
                    let x = mm_to_pt(pos[0] + shift * cos);
                    let y = mm_to_pt(pos[1] + shift * sin);
                    let font = if *bold { "/F2" } else { "/F1" };
                    let matrix = if angle_deg.abs() < 1e-3 {
                        format!("1 0 0 1 {x:.2} {y:.2}")
                    } else {
                        format!("{cos:.4} {sin:.4} {:.4} {cos:.4} {x:.2} {y:.2}", -sin)
                    };
                    s.push_str(&format!(
                        "0 0 0 rg BT {font} {:.2} Tf {matrix} Tm ({}) Tj ET\n",
                        mm_to_pt(*size_mm),
                        escape_pdf(text)
                    ));
                }
                Item::Image { min, max, index } => {
                    if !images.contains(index) {
                        images.push(*index);
                    }
                    s.push_str(&format!(
                        "q {:.2} 0 0 {:.2} {:.2} {:.2} cm /Im{index} Do Q\n",
                        mm_to_pt(max[0] - min[0]),
                        mm_to_pt(max[1] - min[1]),
                        mm_to_pt(min[0]),
                        mm_to_pt(min[1])
                    ));
                }
            }
        }
        s.push_str("Q\n");
    }
}

/// Render anotasi GD&T/toleransi (P19). Simbol berupa path vektor dari
/// `drawing::gdt`; hanya angka/huruf yang memakai font. Lembar tanpa anotasi
/// tidak menambah satu byte pun ke stream.
fn render_annotations(s: &mut String, sheet: &DrawingSheet) {
    use crate::drawing::gdt::{annotation_geometry, PathCmd, STROKE_MM};

    if sheet.annotations.is_empty() {
        return;
    }

    let stroke_w = mm_to_pt(STROKE_MM);
    s.push_str(&format!("q 0 0 0 RG 0 0 0 rg {stroke_w:.2} w [] 0 d 1 j 1 J\n"));

    for annotation in &sheet.annotations {
        let Some(geometry) = annotation_geometry(annotation) else {
            continue;
        };
        let [ax, ay] = geometry.anchor;
        let pt = |p: &[f32; 2]| (mm_to_pt(ax + p[0]), mm_to_pt(ay + p[1]));

        for path in &geometry.paths {
            for cmd in &path.cmds {
                match cmd {
                    PathCmd::Move(a) => {
                        let (x, y) = pt(a);
                        s.push_str(&format!("{x:.2} {y:.2} m "));
                    }
                    PathCmd::Line(a) => {
                        let (x, y) = pt(a);
                        s.push_str(&format!("{x:.2} {y:.2} l "));
                    }
                    PathCmd::Cubic(a, b, c) => {
                        let (x1, y1) = pt(a);
                        let (x2, y2) = pt(b);
                        let (x3, y3) = pt(c);
                        s.push_str(&format!("{x1:.2} {y1:.2} {x2:.2} {y2:.2} {x3:.2} {y3:.2} c "));
                    }
                    PathCmd::Close => s.push_str("h "),
                }
            }
            s.push_str(if path.filled { "B\n" } else { "S\n" });
        }

        for run in &geometry.texts {
            let font = if run.bold { "/F2" } else { "/F1" };
            let font_pt = mm_to_pt(run.size_mm);
            let x_mm = if run.centered {
                run.pos[0] - crate::drawing::gdt::text_width_mm(&run.text, run.size_mm) * 0.5
            } else {
                run.pos[0]
            };
            let (x, y) = pt(&[x_mm, run.pos[1]]);
            s.push_str(&format!(
                "BT {font} {font_pt:.2} Tf 1 0 0 1 {x:.2} {y:.2} Tm ({}) Tj ET\n",
                escape_pdf(&run.text)
            ));
        }
    }

    s.push_str("Q\n");
}

/// Escape string PDF: `(`, `)`, `\`, dan karakter non-ASCII ditulis sebagai
/// oktal WinAnsi (font memakai `/WinAnsiEncoding`), sehingga `Ø`, `×`, `°`
/// tampil benar dan stream tetap ASCII murni.
fn escape_pdf(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 4);
    for c in text.chars() {
        match c {
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\\' => out.push_str("\\\\"),
            ' '..='~' => out.push(c),
            _ => {
                let code: u32 = match c {
                    '\u{00A0}'..='\u{00FF}' => c as u32,
                    '€' => 0x80,
                    '…' => 0x85,
                    '‘' => 0x91,
                    '’' => 0x92,
                    '“' => 0x93,
                    '”' => 0x94,
                    '•' => 0x95,
                    '–' => 0x96,
                    '—' => 0x97,
                    '™' => 0x99,
                    _ => b'?' as u32,
                };
                out.push_str(&format!("\\{code:03o}"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::{DrawingSheet, PaperSize};
    use ducad_kernel::{HlrDrawing, HlrLineKind, HlrSegment2D, ProjectedView, ProjectedViewKind};

    fn sample_drawing() -> HlrDrawing {
        let dummy_view = |kind: ProjectedViewKind| ProjectedView {
            kind,
            title: kind.title_id().to_string(),
            bounds_min: [0.0, 0.0],
            bounds_max: [50.0, 30.0],
            segments: vec![
                HlrSegment2D {
                    start: [0.0, 0.0],
                    end: [50.0, 0.0],
                    kind: HlrLineKind::Visible,
                },
                HlrSegment2D {
                    start: [50.0, 0.0],
                    end: [50.0, 30.0],
                    kind: HlrLineKind::Visible,
                },
                HlrSegment2D {
                    start: [10.0, 10.0],
                    end: [40.0, 10.0],
                    kind: HlrLineKind::Hidden,
                },
            ],
            centerlines: vec![HlrSegment2D {
                start: [25.0, -5.0],
                end: [25.0, 35.0],
                kind: HlrLineKind::Centerline,
            }],
            features: Vec::new(),
            width_mm: 50.0,
            height_mm: 30.0,
            depth_mm: 20.0,
            ..ProjectedView::default()
        };

        HlrDrawing {
            front: dummy_view(ProjectedViewKind::Front),
            top: dummy_view(ProjectedViewKind::Top),
            right: dummy_view(ProjectedViewKind::Right),
            isometric: dummy_view(ProjectedViewKind::Isometric),
            sections: Vec::new(),
            detail_views: Vec::new(),
            model_bbox_min: [0.0, 0.0, 0.0],
            model_bbox_max: [50.0, 30.0, 20.0],
            warnings: Vec::new(),
        }
    }

    #[test]
    fn test_generate_pdf_structure() {
        let drawing = sample_drawing();
        let sheet = DrawingSheet::new(drawing, PaperSize::A4Landscape);
        let pdf_bytes = generate_pdf_bytes(&sheet);

        assert!(!pdf_bytes.is_empty(), "PDF bytes tidak boleh kosong");
        let text = String::from_utf8_lossy(&pdf_bytes);

        // Verifikasi PDF 1.4 header
        assert!(text.starts_with("%PDF-1.4"), "Header harus %PDF-1.4");

        // Verifikasi keberadaan elemen kunci ISO PDF
        assert!(text.contains("/Type /Catalog"), "Harus memuat objek Catalog");
        assert!(text.contains("/Type /Pages"), "Harus memuat objek Pages");
        assert!(text.contains("/Type /Page"), "Harus memuat objek Page");
        assert!(text.contains("/MediaBox [0 0"), "Harus memuat MediaBox dimensi kertas");
        assert!(text.contains("xref"), "Harus memuat tabel xref");
        assert!(text.contains("trailer"), "Harus memuat trailer");
        assert!(text.ends_with("%%EOF\n") || text.ends_with("%%EOF"), "Harus diakhiri %%EOF");

        // Verifikasi konten metadata title block tercantum di PDF
        assert!(text.contains("DWG-2026-001"));
        assert!(text.contains("Aluminium 6061-T6"));
    }

    #[test]
    fn test_export_pdf_file() {
        let drawing = sample_drawing();
        let sheet = DrawingSheet::new(drawing, PaperSize::A3Landscape);
        let temp_path = std::env::temp_dir().join(format!("ducad-test-dwg-{}.pdf", std::process::id()));

        let res = export_pdf(&sheet, &temp_path);
        assert!(res.is_ok(), "Ekspor PDF harus berhasil");
        assert!(temp_path.exists());
        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn test_pdf_export_with_bom_and_balloons() {
        let drawing = sample_drawing();
        let mut sheet = DrawingSheet::new(drawing, PaperSize::A4Landscape);

        sheet.bom_table.items.push(crate::drawing::BomItem {
            item_number: 1,
            part_name: "Gear Box Housing".to_string(),
            quantity: 1,
            material: "Cast Iron".to_string(),
            description: "Main housing".to_string(),
        });
        sheet.bom_table.items.push(crate::drawing::BomItem {
            item_number: 2,
            part_name: "Drive Pinion".to_string(),
            quantity: 2,
            material: "Steel 4140".to_string(),
            description: "Hardened gear".to_string(),
        });

        sheet.add_balloon(1, [150.0, 120.0], [170.0, 140.0], ProjectedViewKind::Isometric);
        sheet.add_balloon(2, [130.0, 110.0], [110.0, 90.0], ProjectedViewKind::Isometric);

        let pdf_bytes = generate_pdf_bytes(&sheet);
        assert!(!pdf_bytes.is_empty());
        let text = String::from_utf8_lossy(&pdf_bytes);

        assert!(text.contains("BILL OF MATERIALS"));
        assert!(text.contains("Gear Box Housing"));
        assert!(text.contains("Drive Pinion"));
        assert!(text.contains("Cast Iron"));
        assert!(text.contains("Steel 4140"));
    }
}

