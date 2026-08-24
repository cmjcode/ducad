//! Generator PDF Vektor Resolusi Tinggi untuk Lembar Kerja Gambar Teknik 2D (Engineering Drawing Sheets).
//!
//! Menghasilkan file PDF standar (PDF 1.4 compliant) murni tanpa dependensi eksternal.
//! Mendukung gambar garis tampak tebal (0.5mm solid), garis tersembunyi (0.25mm dashed),
//! garis sumbu simetri (0.25mm dash-dot `— · —`), bingkai gambar ISO dengan grid zona,
//! kepala gambar (title block), panah dan teks dimensi, serta simbol proyeksi sudut ketiga.

use anyhow::{Context, Result};
use ducad_kernel::HlrLineKind;
use std::io::Write;
use std::path::Path;

use crate::drawing::DrawingSheet;

const MM_TO_PT: f32 = 72.0 / 25.4; // 1 mm = ~2.83465 points

pub struct PdfWriter {
    buffer: Vec<u8>,
    offsets: Vec<usize>,
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
pub fn generate_pdf_bytes(sheet: &DrawingSheet) -> Vec<u8> {
    let mut writer = PdfWriter::new();
    writer.write_header();

    let (pw_mm, ph_mm) = sheet.paper_size.dimensions_mm();
    let pw_pt = pw_mm * MM_TO_PT;
    let ph_pt = ph_mm * MM_TO_PT;

    // 1. Render Stream Grafik
    let mut stream = String::with_capacity(16 * 1024);

    // Set background putih
    stream.push_str(&format!(
        "q 1 1 1 rg 0 0 {pw_pt:.2} {ph_pt:.2} re f Q\n"
    ));

    // Render Bingkai & Border Gambar
    render_border_and_grid(&mut stream, sheet);

    // Render Kepala Gambar (Title Block)
    render_title_block(&mut stream, sheet);

    // Render Tampak-Tampak Proyeksi
    render_projected_views(&mut stream, sheet);

    // Render Dimensi Otomatis
    if sheet.show_dimensions {
        render_dimensions(&mut stream, sheet);
    }

    // Objek 1: Font Helvetica Standar
    let font1_obj = writer.add_object("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>");
    // Objek 2: Font Helvetica-Bold Standar
    let font2_obj = writer.add_object("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold >>");
    // Objek 3: Font Courier Standar (Monospace)
    let font3_obj = writer.add_object("<< /Type /Font /Subtype /Type1 /BaseFont /Courier >>");

    // Objek 4: Stream Konten Halaman
    let stream_obj = writer.add_stream_object(stream.as_bytes());

    // Objek 5: Objek Halaman (Page)
    let page_obj = writer.add_object(&format!(
        "<< /Type /Page /Parent 6 0 R /MediaBox [0 0 {pw_pt:.2} {ph_pt:.2}] /Contents {stream_obj} 0 R /Resources << /Font << /F1 {font1_obj} 0 R /F2 {font2_obj} 0 R /F3 {font3_obj} 0 R >> >> >>"
    ));

    // Objek 6: Pages
    let pages_obj = writer.add_object(&format!(
        "<< /Type /Pages /Kids [{page_obj} 0 R] /Count 1 >>"
    ));

    // Objek 7: Catalog (Root)
    let catalog_obj = writer.add_object(&format!(
        "<< /Type /Catalog /Pages {pages_obj} 0 R >>"
    ));

    writer.finalize(catalog_obj)
}

fn mm_to_pt(val_mm: f32) -> f32 {
    val_mm * MM_TO_PT
}

/// Menggambar garis batas luar, margin jilid 20mm, bingkai dalam 10mm, dan penanda zona grid (A-D, 1-6).
fn render_border_and_grid(s: &mut String, sheet: &DrawingSheet) {
    let (outer, inner) = sheet.border_rects_mm();

    let ox = mm_to_pt(outer[0]);
    let oy = mm_to_pt(outer[1]);
    let ow = mm_to_pt(outer[2] - outer[0]);
    let oh = mm_to_pt(outer[3] - outer[1]);

    let ix = mm_to_pt(inner[0]);
    let iy = mm_to_pt(inner[1]);
    let iw = mm_to_pt(inner[2] - inner[0]);
    let ih = mm_to_pt(inner[3] - inner[1]);

    // Garis bingkai dalam tebal (0.7 mm)
    s.push_str("q 0 0 0 RG [] 0 d 2.0 w\n");
    s.push_str(&format!("{ix:.2} {iy:.2} {iw:.2} {ih:.2} re S\n"));

    // Garis batas luar tipis (0.35 mm)
    s.push_str("0.5 w\n");
    s.push_str(&format!("{ox:.2} {oy:.2} {ow:.2} {oh:.2} re S\n"));

    // Grid zona referensi gambar (1, 2, 3, 4, 5, 6 secara horizontal; A, B, C, D secara vertikal)
    let cols = 6;
    let rows = 4;
    let col_w = (inner[2] - inner[0]) / cols as f32;
    let row_h = (inner[3] - inner[1]) / rows as f32;

    s.push_str("0 0 0 rg BT /F1 7 Tf\n");

    // Label kolom horizontal (1..6)
    for c in 0..cols {
        let x_mm = inner[0] + (c as f32 + 0.5) * col_w;
        let x_pt = mm_to_pt(x_mm);
        let num = c + 1;
        // Atas & Bawah
        let y_top = mm_to_pt(inner[3] + 2.0);
        let y_bot = mm_to_pt(inner[1] - 6.0);
        s.push_str(&format!("1 0 0 1 {x_pt:.2} {y_top:.2} Tm ({num}) Tj\n"));
        s.push_str(&format!("1 0 0 1 {x_pt:.2} {y_bot:.2} Tm ({num}) Tj\n"));
    }

    // Label baris vertikal (A..D)
    let row_chars = ["D", "C", "B", "A"];
    for r in 0..rows {
        let y_mm = inner[1] + (r as f32 + 0.5) * row_h;
        let y_pt = mm_to_pt(y_mm);
        let ch = row_chars[r.min(3)];
        // Kiri & Kanan
        let x_left = mm_to_pt(inner[0] - 8.0);
        let x_right = mm_to_pt(inner[2] + 2.0);
        s.push_str(&format!("1 0 0 1 {x_left:.2} {y_pt:.2} Tm ({ch}) Tj\n"));
        s.push_str(&format!("1 0 0 1 {x_right:.2} {y_pt:.2} Tm ({ch}) Tj\n"));
    }

    s.push_str("ET Q\n");
}

/// Render Kepala Gambar (Title Block) standar ISO/ANSI di pojok kanan bawah.
fn render_title_block(s: &mut String, sheet: &DrawingSheet) {
    let tb = sheet.title_block_rect_mm();
    let info = &sheet.title_block;

    let x0 = mm_to_pt(tb[0]);
    let y0 = mm_to_pt(tb[1]);
    let w = mm_to_pt(tb[2] - tb[0]);
    let h = mm_to_pt(tb[3] - tb[1]);

    s.push_str("q 0 0 0 RG 0 0 0 rg 1.0 w [] 0 d\n");
    // Kotak luar title block
    s.push_str(&format!("{x0:.2} {y0:.2} {w:.2} {h:.2} re S\n"));

    // Garis pembagi horizontal internal
    let y_div1 = mm_to_pt(tb[1] + 15.0);
    let y_div2 = mm_to_pt(tb[1] + 30.0);
    s.push_str(&format!("{x0:.2} {y_div1:.2} m {x0:.2} {w:.2} add {y_div1:.2} l S\n", x0 = x0, w = w));
    s.push_str(&format!("{x0:.2} {y_div2:.2} m {x0:.2} {w:.2} add {y_div2:.2} l S\n", x0 = x0, w = w));

    // Garis pembagi vertikal
    let x_mid = mm_to_pt(tb[0] + 75.0);
    s.push_str(&format!("{x_mid:.2} {y0:.2} m {x_mid:.2} {y_div2:.2} l S\n"));

    let x_qtr = mm_to_pt(tb[0] + 110.0);
    s.push_str(&format!("{x_qtr:.2} {y0:.2} m {x_qtr:.2} {y_div1:.2} l S\n"));

    // Teks Metadata Title Block
    s.push_str("BT\n");

    // Header Perusahaan / Studio (Baris Atas)
    s.push_str("/F2 10 Tf\n");
    let tx_comp = x0 + mm_to_pt(4.0);
    let ty_comp = y0 + mm_to_pt(36.0);
    s.push_str(&format!("1 0 0 1 {tx_comp:.2} {ty_comp:.2} Tm ({}) Tj\n", escape_pdf(&info.company_name)));

    s.push_str("/F1 7 Tf\n");
    let ty_proj_lbl = y0 + mm_to_pt(32.0);
    s.push_str(&format!("1 0 0 1 {tx_comp:.2} {ty_proj_lbl:.2} Tm (JUDUL GAMBAR / PROJECT TITLE:) Tj\n"));

    // Judul Part / Komponen Utama
    s.push_str("/F2 11 Tf\n");
    let ty_title = y0 + mm_to_pt(20.0);
    s.push_str(&format!("1 0 0 1 {tx_comp:.2} {ty_title:.2} Tm ({}) Tj\n", escape_pdf(&info.project_title)));

    // Kolom Kiri Bawah: Drafter & Tanggal
    s.push_str("/F1 6.5 Tf\n");
    let ty_dr_lbl = y0 + mm_to_pt(10.5);
    s.push_str(&format!("1 0 0 1 {tx_comp:.2} {ty_dr_lbl:.2} Tm (DIGAMBAR: {} | TGL: {}) Tj\n", escape_pdf(&info.drawn_by), escape_pdf(&info.date)));

    let ty_mat_lbl = y0 + mm_to_pt(4.0);
    s.push_str(&format!("1 0 0 1 {tx_comp:.2} {ty_mat_lbl:.2} Tm (MATERIAL: {}) Tj\n", escape_pdf(&info.material)));

    // Kolom Kanan: Nomor Gambar & Skala
    let tx_dwg = x_mid + mm_to_pt(3.0);
    let ty_dwg_lbl = y0 + mm_to_pt(24.0);
    s.push_str(&format!("1 0 0 1 {tx_dwg:.2} {ty_dwg_lbl:.2} Tm (NO. GAMBAR / DWG NO:) Tj\n"));

    s.push_str("/F2 9 Tf\n");
    let ty_dwg_val = y0 + mm_to_pt(17.5);
    s.push_str(&format!("1 0 0 1 {tx_dwg:.2} {ty_dwg_val:.2} Tm ({}) Tj\n", escape_pdf(&info.drawing_number)));

    s.push_str("/F1 7 Tf\n");
    let ty_scale_lbl = y0 + mm_to_pt(9.0);
    s.push_str(&format!("1 0 0 1 {tx_dwg:.2} {ty_scale_lbl:.2} Tm (SKALA: {}) Tj\n", escape_pdf(&info.scale)));

    let ty_unit_lbl = y0 + mm_to_pt(3.5);
    s.push_str(&format!("1 0 0 1 {tx_dwg:.2} {ty_unit_lbl:.2} Tm (SATUAN: {} | LEMBAR: {}) Tj\n", escape_pdf(&info.units), escape_pdf(&info.sheet_number)));

    s.push_str("ET Q\n");

    // Gambar Simbol Proyeksi Sudut Ketiga (Third Angle Projection Icon) di title block
    render_projection_symbol(s, tb[0] + 122.0, tb[1] + 20.0);
}

/// Menggambar simbol standar 3rd Angle Projection (lingkaran konsentris + kerucut terpotong).
fn render_projection_symbol(s: &mut String, cx_mm: f32, cy_mm: f32) {
    let cx = mm_to_pt(cx_mm);
    let cy = mm_to_pt(cy_mm);
    let r1 = mm_to_pt(2.5);
    let r2 = mm_to_pt(4.5);
    let k_w = mm_to_pt(8.0);
    let k_h1 = mm_to_pt(5.0);
    let k_h2 = mm_to_pt(9.0);

    s.push_str("q 0 0 0 RG 0.5 w [] 0 d\n");
    // Lingkaran konsentris
    s.push_str(&format!("{cx:.2} {cy:.2} {r1:.2} 0 360 arc S\n", cx = cx - mm_to_pt(10.0), cy = cy, r1 = r1));
    s.push_str(&format!("{cx:.2} {cy:.2} {r2:.2} 0 360 arc S\n", cx = cx - mm_to_pt(10.0), cy = cy, r2 = r2));

    // Trapesium kerucut
    let x_cone = cx;
    s.push_str(&format!(
        "{x1:.2} {y1:.2} m {x2:.2} {y2:.2} l {x3:.2} {y3:.2} l {x4:.2} {y4:.2} l h S\n",
        x1 = x_cone,
        y1 = cy - k_h1 * 0.5,
        x2 = x_cone + k_w,
        y2 = cy - k_h2 * 0.5,
        x3 = x_cone + k_w,
        y3 = cy + k_h2 * 0.5,
        x4 = x_cone,
        y4 = cy + k_h1 * 0.5,
    ));

    // Garis sumbu simetri simbol
    s.push_str("[4 2 1 2] 0 d 0.3 w\n");
    let c_start = cx - mm_to_pt(16.0);
    let c_end = cx + k_w + mm_to_pt(3.0);
    s.push_str(&format!("{c_start:.2} {cy:.2} m {c_end:.2} {cy:.2} l S\n"));
    s.push_str("Q\n");
}

/// Render garis-garis tampak proyeksi 2D pada posisi lembar kerja masing-masing.
fn render_projected_views(s: &mut String, sheet: &DrawingSheet) {
    for plc in &sheet.view_placements {
        if !plc.visible {
            continue;
        }

        let view = sheet.drawing.view_by_kind(plc.kind);
        let center_mm = plc.center_mm;
        let scale = plc.scale;
        let v_center = view.center_2d();

        // 1. Render Judul Tampak (View Title)
        let title_x = mm_to_pt(center_mm[0] - (view.size_2d()[0] * scale * 0.5));
        let title_y = mm_to_pt(center_mm[1] - (view.size_2d()[1] * scale * 0.5) - 8.0);
        s.push_str(&format!(
            "q 0 0 0 rg BT /F2 8.5 Tf 1 0 0 1 {title_x:.2} {title_y:.2} Tm ({}) Tj ET Q\n",
            escape_pdf(&view.title)
        ));

        // 2. Render Garis Sumbu (Centerlines) jika aktif
        if sheet.show_centerlines {
            s.push_str("q 0.1 0.6 0.2 RG 0.4 w [6 2 1 2] 0 d\n");
            for cl in &view.centerlines {
                let x1 = mm_to_pt(center_mm[0] + (cl.start[0] - v_center[0]) * scale);
                let y1 = mm_to_pt(center_mm[1] + (cl.start[1] - v_center[1]) * scale);
                let x2 = mm_to_pt(center_mm[0] + (cl.end[0] - v_center[0]) * scale);
                let y2 = mm_to_pt(center_mm[1] + (cl.end[1] - v_center[1]) * scale);
                s.push_str(&format!("{x1:.2} {y1:.2} m {x2:.2} {y2:.2} l S\n"));
            }
            s.push_str("Q\n");
        }

        // 3. Render Garis Tersembunyi (Hidden Lines) jika aktif
        if sheet.show_hidden_lines {
            s.push_str("q 0.3 0.3 0.35 RG 0.5 w [4 2] 0 d\n");
            for seg in &view.segments {
                if seg.kind == HlrLineKind::Hidden {
                    let x1 = mm_to_pt(center_mm[0] + (seg.start[0] - v_center[0]) * scale);
                    let y1 = mm_to_pt(center_mm[1] + (seg.start[1] - v_center[1]) * scale);
                    let x2 = mm_to_pt(center_mm[0] + (seg.end[0] - v_center[0]) * scale);
                    let y2 = mm_to_pt(center_mm[1] + (seg.end[1] - v_center[1]) * scale);
                    s.push_str(&format!("{x1:.2} {y1:.2} m {x2:.2} {y2:.2} l S\n"));
                }
            }
            s.push_str("Q\n");
        }

        // 4. Render Garis Tampak (Visible Lines & Silhouettes)
        s.push_str("q 0 0 0 RG 1.1 w [] 0 d 1 j 1 J\n");
        for seg in &view.segments {
            if seg.kind == HlrLineKind::Visible || seg.kind == HlrLineKind::Silhouette {
                let x1 = mm_to_pt(center_mm[0] + (seg.start[0] - v_center[0]) * scale);
                let y1 = mm_to_pt(center_mm[1] + (seg.start[1] - v_center[1]) * scale);
                let x2 = mm_to_pt(center_mm[0] + (seg.end[0] - v_center[0]) * scale);
                let y2 = mm_to_pt(center_mm[1] + (seg.end[1] - v_center[1]) * scale);
                s.push_str(&format!("{x1:.2} {y1:.2} m {x2:.2} {y2:.2} l S\n"));
            }
        }
        s.push_str("Q\n");
    }
}

/// Render garis dimensi teknis (extension lines, dimension line, panah berujung lancip, dan teks nilai).
fn render_dimensions(s: &mut String, sheet: &DrawingSheet) {
    s.push_str("q 0.1 0.25 0.7 RG 0.1 0.25 0.7 rg 0.5 w [] 0 d\n");

    for dim in &sheet.auto_dimensions {
        let x1 = mm_to_pt(dim.start[0]);
        let y1 = mm_to_pt(dim.start[1]);
        let x2 = mm_to_pt(dim.end[0]);
        let y2 = mm_to_pt(dim.end[1]);

        if dim.is_vertical {
            let dim_x = mm_to_pt(dim.line_pos[0]);
            // Garis ekstensi horizontal
            s.push_str(&format!("{x1:.2} {y1:.2} m {dim_x:.2} {y1:.2} l S\n"));
            s.push_str(&format!("{x2:.2} {y2:.2} m {dim_x:.2} {y2:.2} l S\n"));

            // Garis dimensi vertikal
            s.push_str(&format!("{dim_x:.2} {y1:.2} m {dim_x:.2} {y2:.2} l S\n"));

            // Panah atas & bawah
            render_arrow_pt(s, dim_x, y1, 0.0, 1.0);
            render_arrow_pt(s, dim_x, y2, 0.0, -1.0);

            // Teks dimensi diputar vertikal
            let mid_y = (y1 + y2) * 0.5;
            let text_x = dim_x - mm_to_pt(3.0);
            s.push_str(&format!(
                "BT /F2 7 Tf 0 1 -1 0 {text_x:.2} {mid_y:.2} Tm ({}) Tj ET\n",
                escape_pdf(&dim.text)
            ));
        } else {
            let dim_y = mm_to_pt(dim.line_pos[1]);
            // Garis ekstensi vertikal
            s.push_str(&format!("{x1:.2} {y1:.2} m {x1:.2} {dim_y:.2} l S\n"));
            s.push_str(&format!("{x2:.2} {y2:.2} m {x2:.2} {dim_y:.2} l S\n"));

            // Garis dimensi horizontal
            s.push_str(&format!("{x1:.2} {dim_y:.2} m {x2:.2} {dim_y:.2} l S\n"));

            // Panah kiri & kanan
            render_arrow_pt(s, x1, dim_y, 1.0, 0.0);
            render_arrow_pt(s, x2, dim_y, -1.0, 0.0);

            // Teks dimensi di atas garis
            let mid_x = (x1 + x2) * 0.5 - mm_to_pt(8.0);
            let text_y = dim_y + mm_to_pt(1.5);
            s.push_str(&format!(
                "BT /F2 7 Tf 1 0 0 1 {mid_x:.2} {text_y:.2} Tm ({}) Tj ET\n",
                escape_pdf(&dim.text)
            ));
        }
    }

    s.push_str("Q\n");
}

/// Menggambar panah dimensi lancip terisi (filled arrowhead).
fn render_arrow_pt(s: &mut String, tip_x: f32, tip_y: f32, dir_x: f32, dir_y: f32) {
    let arrow_len = mm_to_pt(2.5);
    let arrow_half_w = mm_to_pt(0.6);

    let perp_x = -dir_y;
    let perp_y = dir_x;

    let base_x = tip_x + dir_x * arrow_len;
    let base_y = tip_y + dir_y * arrow_len;

    let p1_x = base_x + perp_x * arrow_half_w;
    let p1_y = base_y + perp_y * arrow_half_w;

    let p2_x = base_x - perp_x * arrow_half_w;
    let p2_y = base_y - perp_y * arrow_half_w;

    s.push_str(&format!(
        "{tip_x:.2} {tip_y:.2} m {p1_x:.2} {p1_y:.2} l {p2_x:.2} {p2_y:.2} l h f\n"
    ));
}

/// Escape karakter khusus PDF string `(`, `)`, `\`.
fn escape_pdf(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 4);
    for c in text.chars() {
        match c {
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
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
            width_mm: 50.0,
            height_mm: 30.0,
            depth_mm: 20.0,
        };

        HlrDrawing {
            front: dummy_view(ProjectedViewKind::Front),
            top: dummy_view(ProjectedViewKind::Top),
            right: dummy_view(ProjectedViewKind::Right),
            isometric: dummy_view(ProjectedViewKind::Isometric),
            model_bbox_min: [0.0, 0.0, 0.0],
            model_bbox_max: [50.0, 30.0, 20.0],
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
        assert!(text.contains("KOMPONEN MEKANIKAL"));
        assert!(text.contains("DWG-2026-001"));
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
}
