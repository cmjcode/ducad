//! Interop DXF (AutoCAD Drawing Exchange Format) — subset R12 ASCII minimal:
//! LINE/CIRCLE/ARC saja. Ditulis sendiri (bukan crate `dxf` pihak ketiga)
//! — group-code R12 untuk 3 jenis entitas ini cukup sederhana untuk
//! ditangani langsung, konsisten dengan filosofi proyek menulis sendiri
//! lapisan tipis yang terkontrol penuh (solver LM, snap engine) alih-alih
//! menarik dependensi besar untuk sebagian kecil kemampuannya.
//!
//! **Sengaja belum didukung** (sama pola dengan `offset_entity`/
//! `build_profile_from_selection` yang menolak Ellipse secara eksplisit):
//! `Entity::Ellipse` — entitas ELLIPSE baru ada di DXF R14+/2000, di luar
//! subset R12 yang ditarget di sini. Spline, polyline, layer/blok/style
//! juga tidak — file yang dibuat tool lain dengan entitas semacam itu tetap
//! bisa di-import, entitas yang tak dikenal cuma dilewati & dihitung
//! (`ImportResult::skipped`), bukan bikin seluruh import gagal.

use anyhow::{Context, Result};
use ducad_kernel::{HlrLineKind, ProjectedViewKind};
use ducad_sketch::{Entity, Sketch};
use glam::DVec2;
use std::path::Path;

use crate::drawing::DrawingSheet;

/// Hasil `import`: entitas yang berhasil dibaca, plus jumlah baris entitas
/// yang dilewati karena jenisnya tidak didukung (mis. SPLINE/TEXT/
/// LWPOLYLINE) — dilaporkan ke pemanggil, tidak didiamkan.
pub struct ImportResult {
    pub entities: Vec<Entity>,
    pub skipped: usize,
}

/// Export Dokumen Lembar Kerja 2D (Drawing Sheet) ke file DXF lengkap dengan layer terorganisir.
pub fn export_drawing_sheet(sheet: &DrawingSheet, path: impl AsRef<Path>) -> Result<()> {
    let mut out = String::new();

    // 1. Header Section dengan tabel Linetypes dan Layers
    out.push_str("0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1009\n0\nENDSEC\n");
    out.push_str("0\nSECTION\n2\nTABLES\n");

    // Linetype Table
    out.push_str("0\nTABLE\n2\nLTYPE\n70\n3\n");
    out.push_str("0\nLTYPE\n2\nCONTINUOUS\n70\n0\n3\nSolid line\n72\n65\n73\n0\n40\n0.0\n");
    out.push_str("0\nLTYPE\n2\nHIDDEN\n70\n0\n3\n__ __ __ __ __\n72\n65\n73\n2\n40\n9.525\n49\n6.35\n49\n-3.175\n");
    out.push_str("0\nLTYPE\n2\nCENTER\n70\n0\n3\n____ _ ____ _ __\n72\n65\n73\n4\n40\n31.75\n49\n19.05\n49\n-3.175\n49\n3.175\n49\n-3.175\n");
    out.push_str("0\nENDTAB\n");

    // Layer Table
    out.push_str("0\nTABLE\n2\nLAYER\n70\n10\n");
    out.push_str("0\nLAYER\n2\nBORDER\n70\n0\n62\n7\n6\nCONTINUOUS\n");
    out.push_str("0\nLAYER\n2\nTITLEBLOCK\n70\n0\n62\n7\n6\nCONTINUOUS\n");
    out.push_str("0\nLAYER\n2\nVISIBLE\n70\n0\n62\n7\n6\nCONTINUOUS\n");
    out.push_str("0\nLAYER\n2\nHIDDEN\n70\n0\n62\n1\n6\nHIDDEN\n");
    out.push_str("0\nLAYER\n2\nCENTERLINE\n70\n0\n62\n3\n6\nCENTER\n");
    out.push_str("0\nLAYER\n2\nDIMENSIONS\n70\n0\n62\n5\n6\nCONTINUOUS\n");
    out.push_str("0\nLAYER\n2\nHATCH\n70\n0\n62\n4\n6\nCONTINUOUS\n");
    out.push_str("0\nLAYER\n2\nSECTION\n70\n0\n62\n1\n6\nCONTINUOUS\n");
    out.push_str("0\nLAYER\n2\nBOM_TABLE\n70\n0\n62\n7\n6\nCONTINUOUS\n");
    out.push_str("0\nLAYER\n2\nCALLOUT_BALLOONS\n70\n0\n62\n7\n6\nCONTINUOUS\n");
    out.push_str("0\nENDTAB\n");
    out.push_str("0\nENDSEC\n");

    // 2. Entities Section
    out.push_str("0\nSECTION\n2\nENTITIES\n");

    // A. Bingkai Kertas & Margin
    let (outer, inner) = sheet.border_rects_mm();
    push_rect_layer(&mut out, "BORDER", outer[0], outer[1], outer[2], outer[3]);
    push_rect_layer(&mut out, "BORDER", inner[0], inner[1], inner[2], inner[3]);

    // B. Kepala Gambar (Title Block)
    let tb = sheet.title_block_rect_mm();
    push_rect_layer(&mut out, "TITLEBLOCK", tb[0], tb[1], tb[2], tb[3]);
    let info = &sheet.title_block;
    push_text_layer(&mut out, "TITLEBLOCK", tb[0] + 4.0, tb[1] + 36.0, 3.5, &info.company_name);
    push_text_layer(&mut out, "TITLEBLOCK", tb[0] + 4.0, tb[1] + 22.0, 4.0, &info.project_title);
    push_text_layer(&mut out, "TITLEBLOCK", tb[0] + 4.0, tb[1] + 12.0, 2.5, &format!("DIGAMBAR: {} ({})", info.drawn_by, info.date));
    push_text_layer(&mut out, "TITLEBLOCK", tb[0] + 4.0, tb[1] + 5.0, 2.5, &format!("MATERIAL: {}", info.material));
    push_text_layer(&mut out, "TITLEBLOCK", tb[0] + 80.0, tb[1] + 22.0, 3.0, &format!("NO: {}", info.drawing_number));
    push_text_layer(&mut out, "TITLEBLOCK", tb[0] + 80.0, tb[1] + 12.0, 2.5, &format!("SKALA: {}", info.scale));
    push_text_layer(&mut out, "TITLEBLOCK", tb[0] + 80.0, tb[1] + 5.0, 2.5, &format!("SATUAN: {} | LBR: {}", info.units, info.sheet_number));

    // C. Tampak-tampak Proyeksi (Visible, Hidden, Centerlines, Hatch, Section)
    for plc in &sheet.view_placements {
        if !plc.visible {
            continue;
        }
        let view = sheet.drawing.view_by_kind(plc.kind);
        let center = plc.center_mm;
        let scale = plc.scale;
        let v_center = view.center_2d();

        // Judul Tampak
        let title_x = center[0] - (view.size_2d()[0] * scale * 0.5);
        let title_y = center[1] - (view.size_2d()[1] * scale * 0.5) - 7.0;
        push_text_layer(&mut out, "TITLEBLOCK", title_x, title_y, 3.0, &view.title);

        // Garis Tampak, Tersembunyi, Sumbu, dan Arsir
        for seg in &view.segments {
            let x1 = center[0] + (seg.start[0] - v_center[0]) * scale;
            let y1 = center[1] + (seg.start[1] - v_center[1]) * scale;
            let x2 = center[0] + (seg.end[0] - v_center[0]) * scale;
            let y2 = center[1] + (seg.end[1] - v_center[1]) * scale;

            match seg.kind {
                HlrLineKind::Visible | HlrLineKind::Silhouette => {
                    push_line_layer(&mut out, "VISIBLE", x1 as f64, y1 as f64, x2 as f64, y2 as f64);
                }
                HlrLineKind::Hidden => {
                    if sheet.show_hidden_lines {
                        push_line_layer(&mut out, "HIDDEN", x1 as f64, y1 as f64, x2 as f64, y2 as f64);
                    }
                }
                HlrLineKind::Centerline => {
                    if sheet.show_centerlines {
                        push_line_layer(&mut out, "CENTERLINE", x1 as f64, y1 as f64, x2 as f64, y2 as f64);
                    }
                }
                HlrLineKind::Hatch => {
                    if sheet.show_hatch {
                        push_line_layer(&mut out, "HATCH", x1 as f64, y1 as f64, x2 as f64, y2 as f64);
                    }
                }
                HlrLineKind::CuttingPlane => {
                    push_line_layer(&mut out, "SECTION", x1 as f64, y1 as f64, x2 as f64, y2 as f64);
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
                push_line_layer(&mut out, "CENTERLINE", x1 as f64, y1 as f64, x2 as f64, y2 as f64);
            }
        }

        // Indikator Garis Potong A-A pada Tampak Atas
        if plc.kind == ProjectedViewKind::Top {
            if let Some(ind) = &sheet.drawing.cutting_plane {
                let p1_x = center[0] + (ind.start[0] - v_center[0]) * scale;
                let p1_y = center[1] + (ind.start[1] - v_center[1]) * scale;
                let p2_x = center[0] + (ind.end[0] - v_center[0]) * scale;
                let p2_y = center[1] + (ind.end[1] - v_center[1]) * scale;

                push_line_layer(&mut out, "SECTION", p1_x as f64, p1_y as f64, p2_x as f64, p2_y as f64);

                let lbl1_x = center[0] + (ind.label1_pos[0] - v_center[0]) * scale;
                let lbl1_y = center[1] + (ind.label1_pos[1] - v_center[1]) * scale;
                let lbl2_x = center[0] + (ind.label2_pos[0] - v_center[0]) * scale;
                let lbl2_y = center[1] + (ind.label2_pos[1] - v_center[1]) * scale;

                push_text_layer(&mut out, "SECTION", lbl1_x, lbl1_y, 4.0, &ind.label);
                push_text_layer(&mut out, "SECTION", lbl2_x, lbl2_y, 4.0, &ind.label);
            }
        }

        // Bingkai Lingkaran Detail View
        if let ProjectedViewKind::Detail(_) = plc.kind {
            let r = (view.size_2d()[0] * 0.5 * scale) as f64;
            push_circle_layer(&mut out, "VISIBLE", center[0] as f64, center[1] as f64, r);
        }

        // Indikator Lingkaran Detail pada Tampak Acuan
        for det in &sheet.drawing.detail_views {
            if det.indicator.parent_view == plc.kind {
                let ind = &det.indicator;
                let cx = center[0] + (ind.center_2d[0] - v_center[0]) * scale;
                let cy = center[1] + (ind.center_2d[1] - v_center[1]) * scale;
                let r = ind.radius_mm * scale;

                push_circle_layer(&mut out, "SECTION", cx as f64, cy as f64, r as f64);

                let lbl_x = center[0] + (ind.label_pos[0] - v_center[0]) * scale;
                let lbl_y = center[1] + (ind.label_pos[1] - v_center[1]) * scale;
                push_text_layer(&mut out, "SECTION", lbl_x, lbl_y, 4.0, &format!("DETAIL {}", ind.label));
            }
        }
    }

    // D. Dimensi (Otomatis & Manual)
    let dims_to_export: Vec<&crate::drawing::DimensionAnnotation> = if sheet.show_dimensions {
        sheet.auto_dimensions.iter().chain(sheet.manual_dimensions.iter()).collect()
    } else {
        sheet.manual_dimensions.iter().collect()
    };
    for dim in dims_to_export {
        let x1 = dim.start[0] as f64;
        let y1 = dim.start[1] as f64;
        let x2 = dim.end[0] as f64;
        let y2 = dim.end[1] as f64;

            let is_leader = dim.text.starts_with('R')
                || dim.text.starts_with('Ø')
                || dim.text.starts_with("Rx");
            let is_angle = dim.text.ends_with('°');

            if is_angle {
                let tx = dim.line_pos[0] as f64;
                let ty = dim.line_pos[1] as f64;
                push_line_layer(&mut out, "DIMENSIONS", x1, y1, x2, y2);
                push_line_layer(&mut out, "DIMENSIONS", x1, y1, tx, ty);
                push_text_layer(&mut out, "DIMENSIONS", (tx + 2.0) as f32, (ty + 1.0) as f32, 2.5, &dim.text);
            } else if is_leader {
                let bx = dim.line_pos[0] as f64;
                let by = dim.line_pos[1] as f64;
                push_line_layer(&mut out, "DIMENSIONS", x1, y1, x2, y2);
                push_line_layer(&mut out, "DIMENSIONS", x2, y2, bx, by);
                push_line_layer(&mut out, "DIMENSIONS", bx, by, bx + 8.0, by);
                push_text_layer(&mut out, "DIMENSIONS", (bx + 1.0) as f32, (by + 1.5) as f32, 2.5, &dim.text);
            } else if dim.is_vertical {
                let dim_x = dim.line_pos[0] as f64;
                push_line_layer(&mut out, "DIMENSIONS", x1, y1, dim_x, y1);
                push_line_layer(&mut out, "DIMENSIONS", x2, y2, dim_x, y2);
                push_line_layer(&mut out, "DIMENSIONS", dim_x, y1, dim_x, y2);
                push_text_layer(&mut out, "DIMENSIONS", (dim_x - 3.0) as f32, ((y1 + y2) * 0.5) as f32, 2.5, &dim.text);
            } else {
                let dim_y = dim.line_pos[1] as f64;
                push_line_layer(&mut out, "DIMENSIONS", x1, y1, x1, dim_y);
                push_line_layer(&mut out, "DIMENSIONS", x2, y2, x2, dim_y);
                push_line_layer(&mut out, "DIMENSIONS", x1, dim_y, x2, dim_y);
                push_text_layer(&mut out, "DIMENSIONS", ((x1 + x2) * 0.5 - 5.0) as f32, (dim_y + 1.5) as f32, 2.5, &dim.text);
            }
        }

    // E. Anotasi Teks Bebas
    for note in &sheet.custom_texts {
        if !note.text.trim().is_empty() {
            push_text_layer(
                &mut out,
                "TEXT_NOTES",
                note.position[0],
                note.position[1],
                note.font_size,
                &note.text,
            );
        }
    }

    // F. Tabel BOM (Bill of Materials)
    if sheet.show_bom_table && !sheet.bom_table.items.is_empty() {
        let tb = sheet.bom_table_rect_mm();
        let (bx, by, bw, bh) = (tb[0] as f64, tb[1] as f64, (tb[2] - tb[0]) as f64, (tb[3] - tb[1]) as f64);
        let col_w = sheet.bom_column_widths_mm();
        let title_h = sheet.bom_title_height_mm() as f64;
        let header_h = sheet.bom_header_height_mm() as f64;
        let row_h = sheet.bom_row_height_mm() as f64;

        // Border luar tabel
        push_rect_layer(&mut out, "BOM_TABLE", tb[0], tb[1], tb[2], tb[3]);

        // Garis pemisah judul dan header
        let y_title_bot = by + bh - title_h;
        let y_header_bot = y_title_bot - header_h;
        push_line_layer(&mut out, "BOM_TABLE", bx, y_title_bot, bx + bw, y_title_bot);
        push_line_layer(&mut out, "BOM_TABLE", bx, y_header_bot, bx + bw, y_header_bot);

        // Judul tabel BOM
        let title_str = if sheet.bom_table.title.is_empty() { "BILL OF MATERIALS" } else { &sheet.bom_table.title };
        push_text_layer(&mut out, "BOM_TABLE", (bx + 4.0) as f32, (y_title_bot + 2.0) as f32, 3.0, title_str);

        // Header kolom dan garis vertikal
        let col_names = ["ITEM", "PART NAME", "QTY", "MATERIAL", "DESCRIPTION"];
        let mut cur_x = bx;
        for (i, &name) in col_names.iter().enumerate() {
            let cw = col_w[i] as f64;
            push_text_layer(&mut out, "BOM_TABLE", (cur_x + 2.0) as f32, (y_header_bot + 1.8) as f32, 2.5, name);
            if i > 0 {
                push_line_layer(&mut out, "BOM_TABLE", cur_x, by, cur_x, y_title_bot);
            }
            cur_x += cw;
        }

        // Baris data item
        for (row_idx, item) in sheet.bom_table.items.iter().enumerate() {
            let y_row = y_header_bot - ((row_idx + 1) as f64 * row_h);
            push_line_layer(&mut out, "BOM_TABLE", bx, y_row, bx + bw, y_row);

            let vals = [
                format!("{}", item.item_number),
                item.part_name.clone(),
                format!("{}", item.quantity),
                item.material.clone(),
                item.description.clone(),
            ];

            let mut cell_x = bx;
            for (c_idx, val) in vals.iter().enumerate() {
                let cw = col_w[c_idx] as f64;
                push_text_layer(&mut out, "BOM_TABLE", (cell_x + 2.0) as f32, (y_row + 1.6) as f32, 2.3, val);
                cell_x += cw;
            }
        }
    }

    // G. Part Callout Balloons
    if sheet.show_balloons && !sheet.balloons.is_empty() {
        for balloon in &sheet.balloons {
            let (tx, ty) = (balloon.target_point[0] as f64, balloon.target_point[1] as f64);
            let (bx, by) = (balloon.balloon_pos[0] as f64, balloon.balloon_pos[1] as f64);
            let r = balloon.radius_mm as f64;

            let dx = tx - bx;
            let dy = ty - by;
            let len = (dx * dx + dy * dy).sqrt().max(0.1);

            let ex = bx + (dx / len) * r;
            let ey = by + (dy / len) * r;

            // Garis leader
            push_line_layer(&mut out, "CALLOUT_BALLOONS", tx, ty, ex, ey);

            // Lingkaran balon
            push_circle_layer(&mut out, "CALLOUT_BALLOONS", bx, by, r);

            // Nomor item
            push_text_layer(
                &mut out,
                "CALLOUT_BALLOONS",
                (bx - 1.5) as f32,
                (by - 1.2) as f32,
                3.0,
                &format!("{}", balloon.item_number),
            );
        }
    }

    out.push_str("0\nENDSEC\n0\nEOF\n");
    std::fs::write(path, out).context("gagal menulis file DXF lembar kerja")?;
    Ok(())
}

fn push_line_layer(out: &mut String, layer: &str, x0: f64, y0: f64, x1: f64, y1: f64) {
    out.push_str(&format!(
        "0\nLINE\n8\n{layer}\n10\n{x0}\n20\n{y0}\n30\n0.0\n11\n{x1}\n21\n{y1}\n31\n0.0\n"
    ));
}

fn push_rect_layer(out: &mut String, layer: &str, x0: f32, y0: f32, x1: f32, y1: f32) {
    push_line_layer(out, layer, x0 as f64, y0 as f64, x1 as f64, y0 as f64);
    push_line_layer(out, layer, x1 as f64, y0 as f64, x1 as f64, y1 as f64);
    push_line_layer(out, layer, x1 as f64, y1 as f64, x0 as f64, y1 as f64);
    push_line_layer(out, layer, x0 as f64, y1 as f64, x0 as f64, y0 as f64);
}

fn push_circle_layer(out: &mut String, layer: &str, cx: f64, cy: f64, radius: f64) {
    out.push_str(&format!(
        "0\nCIRCLE\n8\n{layer}\n10\n{cx}\n20\n{cy}\n30\n0.0\n40\n{radius}\n"
    ));
}

fn push_text_layer(out: &mut String, layer: &str, x: f32, y: f32, height: f32, text: &str) {
    out.push_str(&format!(
        "0\nTEXT\n8\n{layer}\n10\n{x}\n20\n{y}\n30\n0.0\n40\n{height}\n1\n{text}\n"
    ));
}

/// Export entitas Line/Circle/Arc sebuah sketch ke DXF R12 ASCII minimal.
/// `Entity::Ellipse` dilewati (dihitung, dikembalikan lewat return value)
/// — lihat catatan lingkup di atas modul.
pub fn export(sketch: &Sketch, path: impl AsRef<Path>) -> Result<usize> {
    let mut out = String::new();
    out.push_str("0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1009\n0\nENDSEC\n");
    out.push_str("0\nSECTION\n2\nENTITIES\n");

    let mut skipped = 0usize;
    for (_, entity) in sketch.entities.iter() {
        match entity {
            Entity::Line { start, end, .. } => push_line(&mut out, *start, *end),
            Entity::Circle { center, radius, .. } => push_circle(&mut out, *center, *radius),
            Entity::Arc {
                center,
                radius,
                start_angle,
                end_angle,
                ..
            } => {
                push_arc(&mut out, *center, *radius, *start_angle, *end_angle)
            }
            Entity::Ellipse { .. } | Entity::Spline { .. } => skipped += 1,
        }
    }

    out.push_str("0\nENDSEC\n0\nEOF\n");
    std::fs::write(path, out).context("gagal menulis DXF")?;
    Ok(skipped)
}

fn push_line(out: &mut String, start: DVec2, end: DVec2) {
    out.push_str(&format!(
        "0\nLINE\n8\n0\n10\n{}\n20\n{}\n30\n0.0\n11\n{}\n21\n{}\n31\n0.0\n",
        start.x, start.y, end.x, end.y
    ));
}

fn push_circle(out: &mut String, center: DVec2, radius: f64) {
    out.push_str(&format!(
        "0\nCIRCLE\n8\n0\n10\n{}\n20\n{}\n30\n0.0\n40\n{}\n",
        center.x, center.y, radius
    ));
}

/// Sudut DXF (group 50/51) dalam derajat, CCW dari sumbu X positif — sama
/// konvensi dengan `Entity::Arc::start_angle`/`end_angle` (radian, CCW),
/// jadi cukup konversi rad↔deg, tidak ada pembalikan arah.
fn push_arc(out: &mut String, center: DVec2, radius: f64, start_angle: f64, end_angle: f64) {
    out.push_str(&format!(
        "0\nARC\n8\n0\n10\n{}\n20\n{}\n30\n0.0\n40\n{}\n50\n{}\n51\n{}\n",
        center.x,
        center.y,
        radius,
        start_angle.to_degrees(),
        end_angle.to_degrees()
    ));
}

/// Import entitas LINE/CIRCLE/ARC dari file DXF — parser group-code
/// minimal (pasangan baris kode+nilai), cukup untuk subset yang ditulis
/// `export` di atas dan file R12 sejenis dari tool lain. Kalau section
/// `ENTITIES` tidak ditemukan sama sekali (file bukan DXF, atau varian
/// yang jauh dari R12), mengembalikan hasil kosong alih-alih error keras —
/// parser ini sengaja minimal, bukan implementasi spek DXF penuh.
pub fn import(path: impl AsRef<Path>) -> Result<ImportResult> {
    let text = std::fs::read_to_string(path).context("gagal membaca file DXF")?;
    let mut lines = text.lines().map(str::trim);

    // Cari pasangan (kode=2, nilai=ENTITIES) — dikonsumsi berpasangan
    // supaya tidak kehilangan sinkronisasi kode/nilai DXF (tiap entri
    // group-code SELALU 2 baris: kode lalu nilai).
    let mut found_entities = false;
    while let (Some(code), Some(value)) = (lines.next(), lines.next()) {
        if code == "2" && value == "ENTITIES" {
            found_entities = true;
            break;
        }
    }
    if !found_entities {
        return Ok(ImportResult { entities: Vec::new(), skipped: 0 });
    }

    #[derive(Default)]
    struct Fields {
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    }

    let mut entities = Vec::new();
    let mut skipped = 0usize;
    let mut current: Option<&str> = None;
    let mut fields = Fields::default();

    // Flush TIDAK mereset `current`/`fields` sendiri — kedua titik
    // pemanggilnya di bawah selalu langsung menimpa keduanya lagi
    // (match baru atau `None` sebelum `break`), jadi reset di dalam macro
    // cuma jadi assignment mati yang langsung tertimpa (kompiler warn).
    macro_rules! flush {
        () => {
            match current {
                Some("LINE") => entities.push(Entity::line(
                    DVec2::new(fields.x0, fields.y0),
                    DVec2::new(fields.x1, fields.y1),
                )),
                Some("CIRCLE") => entities.push(Entity::circle(
                    DVec2::new(fields.x0, fields.y0),
                    fields.radius,
                )),
                Some("ARC") => entities.push(Entity::arc(
                    DVec2::new(fields.x0, fields.y0),
                    fields.radius,
                    fields.start_angle.to_radians(),
                    fields.end_angle.to_radians(),
                )),
                _ => {}
            }
        };
    }

    while let (Some(code), Some(value)) = (lines.next(), lines.next()) {
        if code == "0" {
            flush!();
            fields = Fields::default();
            if value == "ENDSEC" || value == "EOF" {
                break;
            }
            current = match value {
                "LINE" => Some("LINE"),
                "CIRCLE" => Some("CIRCLE"),
                "ARC" => Some("ARC"),
                _ => {
                    skipped += 1;
                    None
                }
            };
        } else if let Ok(parsed) = value.parse::<f64>() {
            match (current.unwrap_or(""), code) {
                (_, "10") => fields.x0 = parsed,
                (_, "20") => fields.y0 = parsed,
                ("LINE", "11") => fields.x1 = parsed,
                ("LINE", "21") => fields.y1 = parsed,
                (_, "40") => fields.radius = parsed,
                ("ARC", "50") => fields.start_angle = parsed,
                ("ARC", "51") => fields.end_angle = parsed,
                _ => {}
            }
        }
    }

    Ok(ImportResult { entities, skipped })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn sample_sketch() -> Sketch {
        let mut sketch = Sketch::default();
        sketch.entities.insert(Entity::line(
            DVec2::new(0.0, 0.0),
            DVec2::new(10.0, 5.0),
        ));
        sketch.entities.insert(Entity::circle(
            DVec2::new(3.0, 4.0),
            2.5,
        ));
        sketch.entities.insert(Entity::arc(
            DVec2::new(1.0, 1.0),
            5.0,
            0.0,
            PI,
        ));
        sketch.entities.insert(Entity::ellipse(
            DVec2::new(0.0, 0.0),
            3.0,
            1.0,
        ));
        sketch
    }

    #[test]
    fn export_reports_one_skipped_ellipse() {
        let sketch = sample_sketch();
        let path = std::env::temp_dir().join(format!("ducad-io-test-{}.dxf", std::process::id()));
        let skipped = export(&sketch, &path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(skipped, 1);
    }

    #[test]
    fn export_then_import_roundtrips_line_circle_arc() {
        let sketch = sample_sketch();
        let path = std::env::temp_dir().join(format!("ducad-io-test-roundtrip-{}.dxf", std::process::id()));
        export(&sketch, &path).unwrap();
        let result = import(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(result.entities.len(), 3, "Line+Circle+Arc, Ellipse tidak ikut ter-export");
        assert_eq!(result.skipped, 0, "semua entitas yang di-export DXF-nya dikenal balik oleh import");

        let has_line = result
            .entities
            .iter()
            .any(|e| matches!(e, Entity::Line { start, end, .. } if (*start - DVec2::new(0.0,0.0)).length() < 1e-9 && (*end - DVec2::new(10.0,5.0)).length() < 1e-9));
        assert!(has_line);

        let has_circle = result
            .entities
            .iter()
            .any(|e| matches!(e, Entity::Circle { center, radius, .. } if (*center - DVec2::new(3.0,4.0)).length() < 1e-9 && (radius - 2.5).abs() < 1e-9));
        assert!(has_circle);

        let has_arc = result.entities.iter().any(|e| {
            matches!(e, Entity::Arc { center, radius, start_angle, end_angle, .. }
                if (*center - DVec2::new(1.0,1.0)).length() < 1e-9
                && (radius - 5.0).abs() < 1e-9
                && start_angle.abs() < 1e-9
                && (end_angle - PI).abs() < 1e-6)
        });
        assert!(has_arc, "sudut ARC harus roundtrip rad->deg->rad tanpa drift berarti");
    }

    #[test]
    fn import_missing_entities_section_returns_empty() {
        let path = std::env::temp_dir().join(format!("ducad-io-test-nosec-{}.dxf", std::process::id()));
        std::fs::write(&path, "bukan dxf sama sekali").unwrap();
        let result = import(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(result.entities.is_empty());
        assert_eq!(result.skipped, 0);
    }

    #[test]
    fn import_skips_unsupported_entity_types() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nTEXT\n1\nhello\n0\nLINE\n8\n0\n10\n0.0\n20\n0.0\n30\n0.0\n11\n1.0\n21\n1.0\n31\n0.0\n0\nENDSEC\n0\nEOF\n";
        let path = std::env::temp_dir().join(format!("ducad-io-test-skip-{}.dxf", std::process::id()));
        std::fs::write(&path, dxf).unwrap();
        let result = import(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(result.entities.len(), 1);
        assert_eq!(result.skipped, 1);
    }

    #[test]
    fn test_export_drawing_sheet_dxf() {
        use crate::drawing::{DrawingSheet, PaperSize};
        use ducad_kernel::{HlrDrawing, HlrLineKind, HlrSegment2D, ProjectedView, ProjectedViewKind};

        let dummy_view = |kind: ProjectedViewKind| ProjectedView {
            kind,
            title: kind.title_id().to_string(),
            bounds_min: [0.0, 0.0],
            bounds_max: [40.0, 40.0],
            segments: vec![
                HlrSegment2D {
                    start: [0.0, 0.0],
                    end: [40.0, 0.0],
                    kind: HlrLineKind::Visible,
                },
                HlrSegment2D {
                    start: [5.0, 5.0],
                    end: [35.0, 5.0],
                    kind: HlrLineKind::Hidden,
                },
            ],
            centerlines: vec![HlrSegment2D {
                start: [20.0, -2.0],
                end: [20.0, 42.0],
                kind: HlrLineKind::Centerline,
            }],
            features: Vec::new(),
            width_mm: 40.0,
            height_mm: 40.0,
            depth_mm: 20.0,
        };

        let drawing = HlrDrawing {
            front: dummy_view(ProjectedViewKind::Front),
            top: dummy_view(ProjectedViewKind::Top),
            right: dummy_view(ProjectedViewKind::Right),
            isometric: dummy_view(ProjectedViewKind::Isometric),
            section_a: Some(dummy_view(ProjectedViewKind::SectionAA)),
            cutting_plane: None,
            detail_views: Vec::new(),
            model_bbox_min: [0.0, 0.0, 0.0],
            model_bbox_max: [40.0, 40.0, 20.0],
        };

        let sheet = DrawingSheet::new(drawing, PaperSize::A4Landscape);
        let path = std::env::temp_dir().join(format!("ducad-test-dwg-dxf-{}.dxf", std::process::id()));

        let res = export_drawing_sheet(&sheet, &path);
        assert!(res.is_ok(), "Ekspor DXF Drawing Sheet harus berhasil");

        let content = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert!(content.contains("SECTION\n2\nTABLES"));
        assert!(content.contains("LAYER\n2\nBORDER"));
        assert!(content.contains("LAYER\n2\nTITLEBLOCK"));
        assert!(content.contains("LAYER\n2\nVISIBLE"));
        assert!(content.contains("LAYER\n2\nHIDDEN"));
        assert!(content.contains("LAYER\n2\nCENTERLINE"));
        assert!(content.contains("LAYER\n2\nDIMENSIONS"));
        assert!(content.contains("LAYER\n2\nBOM_TABLE"));
        assert!(content.contains("LAYER\n2\nCALLOUT_BALLOONS"));
        assert!(content.contains("EOF"));
    }
}
