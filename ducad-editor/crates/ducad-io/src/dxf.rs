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

    // 2. Entities Section — dari display-list bersama (`drawing::scene`).
    out.push_str("0\nSECTION\n2\nENTITIES\n");
    use crate::drawing::scene::{build_scene, Anchor, Item};
    for group in &build_scene(sheet).groups {
        for item in &group.items {
            match item {
                Item::Line { a, b, pen } => {
                    push_line_layer(&mut out, pen.layer, a[0] as f64, a[1] as f64, b[0] as f64, b[1] as f64);
                }
                Item::Arc {
                    center,
                    radius,
                    start_deg,
                    end_deg,
                    pen,
                } => {
                    out.push_str(&format!(
                        "0\nARC\n8\n{}\n10\n{:.6}\n20\n{:.6}\n30\n0.0\n40\n{:.6}\n50\n{:.6}\n51\n{:.6}\n",
                        pen.layer,
                        center[0],
                        center[1],
                        radius,
                        start_deg.rem_euclid(360.0),
                        end_deg.rem_euclid(360.0)
                    ));
                }
                Item::Circle { center, radius, pen, .. } => {
                    push_circle_layer(&mut out, pen.layer, center[0] as f64, center[1] as f64, *radius as f64);
                }
                Item::Rect { min, max, pen, .. } => {
                    if let Some(pen) = pen {
                        push_rect_layer(&mut out, pen.layer, min[0], min[1], max[0], max[1]);
                    }
                }
                Item::Fill { points, layer, .. } => {
                    for i in 0..points.len() {
                        let (p, q) = (points[i], points[(i + 1) % points.len()]);
                        push_line_layer(&mut out, layer, p[0] as f64, p[1] as f64, q[0] as f64, q[1] as f64);
                    }
                }
                Item::Text {
                    pos,
                    text,
                    size_mm,
                    anchor,
                    angle_deg,
                    layer,
                    ..
                } => {
                    let shift = match anchor {
                        Anchor::Start => 0.0,
                        Anchor::Middle => -0.5 * crate::drawing::scene::text_width(text, *size_mm),
                        Anchor::End => -crate::drawing::scene::text_width(text, *size_mm),
                    };
                    let (sin, cos) = angle_deg.to_radians().sin_cos();
                    out.push_str(&format!(
                        "0\nTEXT\n8\n{layer}\n10\n{:.6}\n20\n{:.6}\n30\n0.0\n40\n{:.6}\n1\n{}\n50\n{:.6}\n",
                        pos[0] + shift * cos,
                        pos[1] + shift * sin,
                        size_mm,
                        text.replace('\n', " "),
                        angle_deg
                    ));
                }
                // DXF R12 tidak memuat raster: render berbayang dilewati.
                Item::Image { .. } => {}
            }
        }
    }

    out.push_str("0\nENDSEC\n0\nEOF\n");
    std::fs::write(path.as_ref(), out)
        .with_context(|| format!("gagal menulis DXF gambar kerja ke {}", path.as_ref().display()))?;
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
            Entity::Ellipse { .. } | Entity::Spline { .. } | Entity::Path { .. } => skipped += 1,
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
/// Ubah satu segmen *bulge* DXF menjadi busur.
///
/// DXF menyimpan busur di dalam polyline sebagai **bulge**: `b = tan(θ/4)`
/// dengan `θ` sudut tempuh bertanda (positif = berlawanan arah jarum jam).
/// Nilai `0` berarti segmen lurus.
///
/// Mengembalikan `None` bila segmennya lurus atau degenerate — pemanggil
/// menggambarnya sebagai garis.
fn bulge_to_arc(p1: DVec2, p2: DVec2, bulge: f64) -> Option<Entity> {
    if bulge.abs() < 1e-12 {
        return None;
    }
    let chord = p2 - p1;
    let chord_len = chord.length();
    if chord_len < 1e-12 {
        return None;
    }
    let half = 2.0 * bulge.atan(); // = θ/2
    let sin_half = half.sin();
    if sin_half.abs() < 1e-12 {
        return None;
    }
    let d = chord_len * 0.5;
    let radius = (d / sin_half).abs();

    let dir = chord / chord_len;
    // Normal +90°. Pusat berada sejauh apotema `d / tan(θ/2)` dari titik
    // tengah tali busur; tandanya otomatis mengikuti tanda `bulge`, jadi
    // busur cekung dan cembung tidak perlu ditangani terpisah.
    let normal = DVec2::new(-dir.y, dir.x);
    let apothem = d / half.tan();
    let center = (p1 + p2) * 0.5 + normal * apothem;

    let ang = |p: DVec2| {
        let v = p - center;
        v.y.atan2(v.x)
    };
    // `Entity::Arc` selalu CCW dari start ke end. Busur searah jarum jam
    // (bulge negatif) karena itu dicatat terbalik: CCW dari p2 ke p1
    // menggambar kurva yang SAMA.
    let (start_angle, end_angle) = if bulge > 0.0 {
        (ang(p1), ang(p2))
    } else {
        (ang(p2), ang(p1))
    };
    Some(Entity::arc(center, radius, start_angle, end_angle))
}

/// Bangun entitas dari deretan titik polyline beserta bulge per segmen.
fn polyline_to_entities(points: &[(DVec2, f64)], closed: bool) -> Vec<Entity> {
    let mut out = Vec::new();
    if points.len() < 2 {
        return out;
    }
    let n = points.len();
    let last = if closed { n } else { n - 1 };
    for i in 0..last {
        let (p1, bulge) = points[i];
        let (p2, _) = points[(i + 1) % n];
        if (p2 - p1).length() < 1e-12 {
            continue;
        }
        match bulge_to_arc(p1, p2, bulge) {
            Some(arc) => out.push(arc),
            None => out.push(Entity::line(p1, p2)),
        }
    }
    out
}

pub fn import(path: impl AsRef<Path>) -> Result<ImportResult> {
    let text = std::fs::read_to_string(path).context("gagal membaca file DXF")?;
    import_str(&text)
}

/// Bagian murni dari [`import`] — memisahkan parsing dari I/O berkas supaya
/// bisa diuji dengan fixture string tanpa menyentuh disk.
pub fn import_str(text: &str) -> Result<ImportResult> {
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
        return Ok(ImportResult {
            entities: Vec::new(),
            skipped: 0,
        });
    }

    /// Akumulator satu entitas. Berbeda dari versi sebelumnya yang menimpa
    /// kode 10/20: LWPOLYLINE mengulang kode yang sama sekali per vertex,
    /// jadi titik harus DIKUMPULKAN, bukan ditimpa.
    #[derive(Default)]
    struct Rec {
        kind: Option<String>,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
        ratio: f64,
        flags: i64,
        /// Titik terkumpul (polyline). `f64` kedua adalah bulge segmen itu.
        pts: Vec<(DVec2, f64)>,
        /// Vertex polyline yang sedang dibaca tapi belum punya pasangan Y.
        pending_x: Option<f64>,
        pending_bulge: f64,
    }

    impl Rec {
        fn push_pending(&mut self) {
            if let Some(x) = self.pending_x.take() {
                self.pts.push((DVec2::new(x, self.y0), self.pending_bulge));
                self.pending_bulge = 0.0;
            }
        }
    }

    let mut entities: Vec<Entity> = Vec::new();
    let mut skipped = 0usize;
    let mut rec = Rec::default();
    // Vertex POLYLINE gaya lama datang sebagai entitas VERTEX terpisah di
    // antara POLYLINE dan SEQEND, jadi butuh akumulator sendiri.
    let mut poly_pts: Vec<(DVec2, f64)> = Vec::new();
    let mut poly_closed = false;
    let mut in_polyline = false;

    macro_rules! flush {
        () => {
            match rec.kind.as_deref() {
                Some("LINE") => entities.push(Entity::line(
                    DVec2::new(rec.x0, rec.y0),
                    DVec2::new(rec.x1, rec.y1),
                )),
                Some("CIRCLE") => {
                    entities.push(Entity::circle(DVec2::new(rec.x0, rec.y0), rec.radius))
                }
                Some("ARC") => entities.push(Entity::arc(
                    DVec2::new(rec.x0, rec.y0),
                    rec.radius,
                    rec.start_angle.to_radians(),
                    rec.end_angle.to_radians(),
                )),
                Some("LWPOLYLINE") => {
                    rec.push_pending();
                    let closed = rec.flags & 1 != 0;
                    entities.extend(polyline_to_entities(&rec.pts, closed));
                }
                Some("ELLIPSE") => {
                    // 11/21 adalah vektor sumbu MAYOR relatif terhadap pusat;
                    // 40 adalah rasio minor/mayor. Model `Entity::Ellipse`
                    // DUCAD masih sejajar sumbu, jadi ellips yang berotasi
                    // TIDAK diimpor — dihitung sebagai dilewati, bukan
                    // diimpor dengan rotasi yang diam-diam dibuang.
                    let major = DVec2::new(rec.x1, rec.y1);
                    let major_len = major.length();
                    let center = DVec2::new(rec.x0, rec.y0);
                    let minor_len = major_len * rec.ratio;
                    if major_len < 1e-9 || rec.ratio <= 0.0 {
                        skipped += 1;
                    } else if major.y.abs() < 1e-9 {
                        entities.push(Entity::ellipse(center, major_len, minor_len));
                    } else if major.x.abs() < 1e-9 {
                        entities.push(Entity::ellipse(center, minor_len, major_len));
                    } else {
                        skipped += 1;
                    }
                }
                _ => {}
            }
        };
    }

    while let (Some(code), Some(value)) = (lines.next(), lines.next()) {
        if code == "0" {
            match value {
                "VERTEX" => {
                    // Vertex POLYLINE gaya lama: kumpulkan, jangan flush.
                    rec.push_pending();
                    if in_polyline {
                        poly_pts.append(&mut rec.pts);
                    }
                    rec = Rec {
                        kind: Some("VERTEX".to_string()),
                        ..Default::default()
                    };
                    continue;
                }
                "SEQEND" => {
                    rec.push_pending();
                    if in_polyline {
                        poly_pts.append(&mut rec.pts);
                        entities.extend(polyline_to_entities(&poly_pts, poly_closed));
                    }
                    poly_pts.clear();
                    in_polyline = false;
                    rec = Rec::default();
                    continue;
                }
                _ => {}
            }

            if rec.kind.as_deref() == Some("VERTEX") {
                rec.push_pending();
                if in_polyline {
                    poly_pts.append(&mut rec.pts);
                }
            } else {
                flush!();
            }

            if value == "ENDSEC" || value == "EOF" {
                if in_polyline && !poly_pts.is_empty() {
                    entities.extend(polyline_to_entities(&poly_pts, poly_closed));
                }
                break;
            }

            let known = matches!(value, "LINE" | "CIRCLE" | "ARC" | "LWPOLYLINE" | "ELLIPSE");
            if value == "POLYLINE" {
                in_polyline = true;
                poly_pts.clear();
                poly_closed = false;
                rec = Rec {
                    kind: Some("POLYLINE".to_string()),
                    ..Default::default()
                };
            } else if known {
                rec = Rec {
                    kind: Some(value.to_string()),
                    ..Default::default()
                };
            } else {
                skipped += 1;
                rec = Rec::default();
            }
            continue;
        }

        let Ok(parsed) = value.parse::<f64>() else {
            continue;
        };
        let kind = rec.kind.clone().unwrap_or_default();
        match (kind.as_str(), code) {
            // Polyline: kode 10/20 BERULANG per vertex, jadi X ditahan
            // sampai Y-nya datang lalu pasangannya disimpan.
            ("LWPOLYLINE", "10") | ("VERTEX", "10") => {
                rec.push_pending();
                rec.pending_x = Some(parsed);
            }
            ("LWPOLYLINE", "20") | ("VERTEX", "20") => {
                rec.y0 = parsed;
                rec.push_pending();
            }
            ("LWPOLYLINE", "42") | ("VERTEX", "42") => {
                // Bulge muncul SESUDAH vertex-nya tersimpan, jadi dipasang
                // ke titik terakhir yang sudah masuk.
                if let Some(last) = rec.pts.last_mut() {
                    last.1 = parsed;
                } else {
                    rec.pending_bulge = parsed;
                }
            }
            ("LWPOLYLINE", "70") | ("POLYLINE", "70") => {
                rec.flags = parsed as i64;
                if kind == "POLYLINE" {
                    poly_closed = rec.flags & 1 != 0;
                }
            }
            (_, "10") => rec.x0 = parsed,
            (_, "20") => rec.y0 = parsed,
            ("LINE", "11") | ("ELLIPSE", "11") => rec.x1 = parsed,
            ("LINE", "21") | ("ELLIPSE", "21") => rec.y1 = parsed,
            ("ELLIPSE", "40") => rec.ratio = parsed,
            (_, "40") => rec.radius = parsed,
            ("ARC", "50") => rec.start_angle = parsed,
            ("ARC", "51") => rec.end_angle = parsed,
            _ => {}
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
            ..ProjectedView::default()
        };

        let drawing = HlrDrawing {
            front: dummy_view(ProjectedViewKind::Front),
            top: dummy_view(ProjectedViewKind::Top),
            right: dummy_view(ProjectedViewKind::Right),
            isometric: dummy_view(ProjectedViewKind::Isometric),
            sections: Vec::new(),
            detail_views: Vec::new(),
            model_bbox_min: [0.0, 0.0, 0.0],
            model_bbox_max: [40.0, 40.0, 20.0],
            warnings: Vec::new(),
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

#[cfg(test)]
mod import_coverage_tests {
    use super::*;
    use std::f64::consts::PI;

    fn wrap(body: &str) -> String {
        format!("0\nSECTION\n2\nENTITIES\n{body}0\nENDSEC\n0\nEOF\n")
    }

    #[test]
    fn lwpolyline_open_becomes_line_chain() {
        // LWPOLYLINE adalah entitas paling umum di DXF nyata dan sebelumnya
        // dilewati seluruhnya.
        let dxf = wrap(
            "0\nLWPOLYLINE\n90\n3\n70\n0\n\
             10\n0.0\n20\n0.0\n\
             10\n10.0\n20\n0.0\n\
             10\n10.0\n20\n5.0\n",
        );
        let res = import_str(&dxf).unwrap();
        assert_eq!(res.entities.len(), 2, "3 titik terbuka = 2 segmen");
        assert!(res.entities.iter().all(|e| matches!(e, Entity::Line { .. })));
    }

    #[test]
    fn lwpolyline_closed_adds_the_closing_segment() {
        let dxf = wrap(
            "0\nLWPOLYLINE\n90\n3\n70\n1\n\
             10\n0.0\n20\n0.0\n\
             10\n10.0\n20\n0.0\n\
             10\n10.0\n20\n10.0\n",
        );
        let res = import_str(&dxf).unwrap();
        assert_eq!(res.entities.len(), 3, "segitiga tertutup = 3 segmen");
    }

    #[test]
    fn bulge_segment_becomes_geometrically_correct_arc() {
        // bulge = tan(θ/4). Untuk θ = π/2, bulge = tan(π/8) ≈ 0.414214.
        // Busur CCW dari (0,0) ke (1,0) dengan sudut tempuh 90° punya pusat
        // (0.5, 0.5) dan radius 1/√2. Diverifikasi angkanya, bukan sekadar
        // "menghasilkan sebuah Arc".
        let b = (PI / 8.0).tan();
        let dxf = wrap(&format!(
            "0\nLWPOLYLINE\n90\n2\n70\n0\n\
             10\n0.0\n20\n0.0\n42\n{b}\n\
             10\n1.0\n20\n0.0\n"
        ));
        let res = import_str(&dxf).unwrap();
        assert_eq!(res.entities.len(), 1);
        match &res.entities[0] {
            Entity::Arc {
                center,
                radius,
                start_angle,
                end_angle,
                ..
            } => {
                assert!(
                    (*center - DVec2::new(0.5, 0.5)).length() < 1e-9,
                    "pusat {center:?}"
                );
                assert!(
                    (*radius - (0.5f64).sqrt()).abs() < 1e-9,
                    "radius {radius}"
                );
                let sweep = (end_angle - start_angle).rem_euclid(2.0 * PI);
                assert!(
                    (sweep - PI / 2.0).abs() < 1e-9,
                    "sudut tempuh {sweep} bukan 90°"
                );
            }
            other => panic!("harus Arc, dapat {other:?}"),
        }
    }

    #[test]
    fn negative_bulge_produces_the_same_curve_reversed() {
        // Bulge negatif = searah jarum jam. `Entity::Arc` selalu CCW, jadi
        // busurnya dicatat terbalik — kurvanya harus tetap melalui kedua
        // titik ujung yang sama.
        let b = -(PI / 8.0).tan();
        let dxf = wrap(&format!(
            "0\nLWPOLYLINE\n90\n2\n70\n0\n\
             10\n0.0\n20\n0.0\n42\n{b}\n\
             10\n1.0\n20\n0.0\n"
        ));
        let res = import_str(&dxf).unwrap();
        match &res.entities[0] {
            Entity::Arc { center, radius, .. } => {
                // Cermin dari kasus positif: pusat di bawah tali busur.
                assert!((*center - DVec2::new(0.5, -0.5)).length() < 1e-9);
                for p in [DVec2::ZERO, DVec2::new(1.0, 0.0)] {
                    assert!(
                        ((p - *center).length() - radius).abs() < 1e-9,
                        "ujung {p:?} harus berada di busur"
                    );
                }
            }
            other => panic!("harus Arc, dapat {other:?}"),
        }
    }

    #[test]
    fn old_style_polyline_with_vertex_entities() {
        let dxf = wrap(
            "0\nPOLYLINE\n70\n0\n\
             0\nVERTEX\n10\n0.0\n20\n0.0\n\
             0\nVERTEX\n10\n5.0\n20\n0.0\n\
             0\nVERTEX\n10\n5.0\n20\n5.0\n\
             0\nSEQEND\n",
        );
        let res = import_str(&dxf).unwrap();
        assert_eq!(res.entities.len(), 2);
    }

    #[test]
    fn axis_aligned_ellipse_is_imported() {
        // 11/21 adalah vektor sumbu mayor RELATIF terhadap pusat; 40 rasio.
        let dxf = wrap(
            "0\nELLIPSE\n10\n1.0\n20\n2.0\n11\n10.0\n21\n0.0\n40\n0.5\n",
        );
        let res = import_str(&dxf).unwrap();
        assert_eq!(res.entities.len(), 1);
        match &res.entities[0] {
            Entity::Ellipse {
                center,
                radius_x,
                radius_y,
                ..
            } => {
                assert!((*center - DVec2::new(1.0, 2.0)).length() < 1e-9);
                assert!((*radius_x - 10.0).abs() < 1e-9);
                assert!((*radius_y - 5.0).abs() < 1e-9);
            }
            other => panic!("harus Ellipse, dapat {other:?}"),
        }
    }

    #[test]
    fn rotated_ellipse_is_skipped_not_silently_flattened() {
        // Model `Entity::Ellipse` DUCAD masih sejajar sumbu. Mengimpor
        // ellips berotasi berarti membuang rotasinya diam-diam — geometri
        // yang SALAH. Lebih baik dilaporkan sebagai dilewati.
        let dxf = wrap(
            "0\nELLIPSE\n10\n0.0\n20\n0.0\n11\n7.07\n21\n7.07\n40\n0.5\n",
        );
        let res = import_str(&dxf).unwrap();
        assert!(res.entities.is_empty());
        assert_eq!(res.skipped, 1);
    }

    #[test]
    fn mixed_file_keeps_previously_supported_entities() {
        // Regresi: penulisan ulang parser tidak boleh menghilangkan
        // dukungan LINE/CIRCLE/ARC yang sudah ada.
        let dxf = wrap(
            "0\nLINE\n10\n0.0\n20\n0.0\n11\n1.0\n21\n1.0\n\
             0\nCIRCLE\n10\n5.0\n20\n5.0\n40\n2.0\n\
             0\nARC\n10\n0.0\n20\n0.0\n40\n3.0\n50\n0.0\n51\n90.0\n\
             0\nTEXT\n1\nhalo\n\
             0\nLWPOLYLINE\n90\n2\n70\n0\n10\n0.0\n20\n0.0\n10\n4.0\n20\n0.0\n",
        );
        let res = import_str(&dxf).unwrap();
        assert_eq!(res.entities.len(), 4, "3 lama + 1 segmen polyline");
        assert_eq!(res.skipped, 1, "TEXT masih dilaporkan dilewati");
        assert!(res
            .entities
            .iter()
            .any(|e| matches!(e, Entity::Circle { .. })));
        assert!(res.entities.iter().any(|e| matches!(e, Entity::Arc { .. })));
    }
}
