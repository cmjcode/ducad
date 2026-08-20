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

/// Hasil `import`: entitas yang berhasil dibaca, plus jumlah baris entitas
/// yang dilewati karena jenisnya tidak didukung (mis. SPLINE/TEXT/
/// LWPOLYLINE) — dilaporkan ke pemanggil, tidak didiamkan.
pub struct ImportResult {
    pub entities: Vec<Entity>,
    pub skipped: usize,
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
            Entity::Line { start, end } => push_line(&mut out, *start, *end),
            Entity::Circle { center, radius } => push_circle(&mut out, *center, *radius),
            Entity::Arc { center, radius, start_angle, end_angle } => {
                push_arc(&mut out, *center, *radius, *start_angle, *end_angle)
            }
            Entity::Ellipse { .. } => skipped += 1,
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
                Some("LINE") => entities.push(Entity::Line {
                    start: DVec2::new(fields.x0, fields.y0),
                    end: DVec2::new(fields.x1, fields.y1),
                }),
                Some("CIRCLE") => entities.push(Entity::Circle {
                    center: DVec2::new(fields.x0, fields.y0),
                    radius: fields.radius,
                }),
                Some("ARC") => entities.push(Entity::Arc {
                    center: DVec2::new(fields.x0, fields.y0),
                    radius: fields.radius,
                    start_angle: fields.start_angle.to_radians(),
                    end_angle: fields.end_angle.to_radians(),
                }),
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
            continue;
        }
        let Some(cur) = current else { continue };
        let parsed: f64 = value.parse().unwrap_or(0.0);
        match (cur, code) {
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

    Ok(ImportResult { entities, skipped })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn sample_sketch() -> Sketch {
        let mut sketch = Sketch::default();
        sketch.entities.insert(Entity::Line {
            start: DVec2::new(0.0, 0.0),
            end: DVec2::new(10.0, 5.0),
        });
        sketch.entities.insert(Entity::Circle {
            center: DVec2::new(3.0, 4.0),
            radius: 2.5,
        });
        sketch.entities.insert(Entity::Arc {
            center: DVec2::new(1.0, 1.0),
            radius: 5.0,
            start_angle: 0.0,
            end_angle: PI,
        });
        sketch.entities.insert(Entity::Ellipse {
            center: DVec2::new(0.0, 0.0),
            radius_x: 3.0,
            radius_y: 1.0,
        });
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
            .any(|e| matches!(e, Entity::Line { start, end } if (*start - DVec2::new(0.0,0.0)).length() < 1e-9 && (*end - DVec2::new(10.0,5.0)).length() < 1e-9));
        assert!(has_line);

        let has_circle = result
            .entities
            .iter()
            .any(|e| matches!(e, Entity::Circle { center, radius } if (*center - DVec2::new(3.0,4.0)).length() < 1e-9 && (radius - 2.5).abs() < 1e-9));
        assert!(has_circle);

        let has_arc = result.entities.iter().any(|e| {
            matches!(e, Entity::Arc { center, radius, start_angle, end_angle }
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
}
