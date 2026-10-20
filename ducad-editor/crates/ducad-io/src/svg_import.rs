//! Insert SVG (Paritas 1.19): berkas SVG → entitas `Entity::Path` di sketsa
//! aktif.
//!
//! Parser: `usvg` (sama dengan yang dipakai `ducad-app`/`ducad-engine` untuk
//! raster SVG). `usvg` sudah menormalkan semua bentuk dasar (`rect`,
//! `circle`, `ellipse`, `polygon`, `line`) dan perintah busur `A` menjadi
//! path berisi MoveTo/LineTo/QuadTo/CubicTo, serta menghitung transformasi
//! absolut (termasuk `viewBox`) — modul ini cukup memetakan segmen itu ke
//! `Subpath` sketsa:
//!
//! - LineTo → `PathSeg::Line`
//! - QuadTo → `PathSeg::Cubic` (dinaikkan derajat secara eksak)
//! - CubicTo → `PathSeg::Cubic` (busur SVG tiba di sini sebagai kubik)
//!
//! Satuan: koordinat hasil `usvg` adalah piksel CSS (96 dpi), dikonversi ke
//! mm (`25.4 / 96`) lalu dikali `scale`. Sumbu Y SVG mengarah ke bawah,
//! sumbu V sketsa ke atas — Y dibalik.
//!
//! Input rusak (bukan XML, SVG tanpa isi, angka tak hingga) menghasilkan
//! `Err`/entitas kosong, tidak pernah panik (ADR 0009).

use anyhow::{anyhow, bail, Result};
use ducad_sketch::{Entity, PathSeg, Subpath};
use glam::DVec2;

/// Satu piksel CSS dalam milimeter.
pub const PX_TO_MM: f64 = 25.4 / 96.0;

/// Batas segmen total — penjaga terhadap SVG raksasa/berbahaya yang akan
/// membuat sketsa tidak bisa dipakai.
pub const MAX_SEGMENTS: usize = 200_000;

/// Opsi Insert SVG.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SvgImportOptions {
    /// Pengali skala di atas konversi px→mm (1.0 = ukuran asli SVG).
    pub scale: f64,
    /// Geser hasil sehingga pojok kiri-bawah bbox berada di `offset`.
    /// `false` = koordinat SVG dipertahankan (hanya Y dibalik).
    pub anchor_to_offset: bool,
    /// Posisi pojok kiri-bawah bbox hasil (mm, bidang sketsa).
    pub offset: DVec2,
}

impl Default for SvgImportOptions {
    fn default() -> Self {
        Self {
            scale: 1.0,
            anchor_to_offset: true,
            offset: DVec2::ZERO,
        }
    }
}

/// Hasil impor.
#[derive(Debug, Clone, Default)]
pub struct SvgImport {
    /// Satu `Entity::Path` per elemen path SVG yang berisi geometri.
    pub entities: Vec<Entity>,
    /// Jumlah segmen total.
    pub segments: usize,
    /// Elemen yang dilewati (gambar raster, teks, path tanpa segmen).
    pub skipped: usize,
}

/// Parse `bytes` SVG menjadi entitas sketsa.
pub fn import_svg_entities(bytes: &[u8], opts: &SvgImportOptions) -> Result<SvgImport> {
    if !(opts.scale.is_finite() && opts.scale > 0.0) {
        bail!("skala SVG harus angka positif");
    }
    if !opts.offset.is_finite() {
        bail!("offset SVG tidak valid");
    }
    if bytes.is_empty() {
        bail!("berkas SVG kosong");
    }
    let tree = usvg::Tree::from_data(bytes, &usvg::Options::default())
        .map_err(|e| anyhow!("SVG tidak bisa dibaca: {e}"))?;
    let k = PX_TO_MM * opts.scale;
    let mut out = SvgImport::default();
    collect_group(tree.root(), k, &mut out)?;
    if out.entities.is_empty() {
        bail!("SVG tidak berisi path yang bisa diimpor");
    }
    if opts.anchor_to_offset {
        if let Some((min, _)) = bbox(&out.entities) {
            let d = opts.offset - min;
            for e in &mut out.entities {
                if let Entity::Path { subpaths, .. } = e {
                    for sp in subpaths.iter_mut() {
                        *sp = sp.map_points(|p| p + d);
                    }
                }
            }
        }
    }
    Ok(out)
}

fn bbox(entities: &[Entity]) -> Option<(DVec2, DVec2)> {
    entities
        .iter()
        .filter_map(|e| e.bounding_box())
        .reduce(|(a0, a1), (b0, b1)| (a0.min(b0), a1.max(b1)))
}

fn collect_group(group: &usvg::Group, k: f64, out: &mut SvgImport) -> Result<()> {
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => collect_group(g, k, out)?,
            usvg::Node::Path(p) => {
                let subpaths = convert_path(p.data(), p.abs_transform(), k);
                let n: usize = subpaths.iter().map(|s| s.segs.len()).sum();
                if n == 0 {
                    out.skipped += 1;
                    continue;
                }
                out.segments += n;
                if out.segments > MAX_SEGMENTS {
                    bail!("SVG terlalu kompleks (lebih dari {MAX_SEGMENTS} segmen)");
                }
                out.entities.push(Entity::path(subpaths));
            }
            // Gambar raster dan teks tidak punya padanan geometri sketsa.
            _ => out.skipped += 1,
        }
    }
    Ok(())
}

/// Segmen `usvg` → subpath sketsa (mm, Y ke atas). Titik tak hingga
/// membatalkan subpath yang memuatnya.
fn convert_path(data: &usvg::tiny_skia_path::Path, ts: usvg::Transform, k: f64) -> Vec<Subpath> {
    use usvg::tiny_skia_path::PathSegment as S;
    let map = |p: usvg::tiny_skia_path::Point| -> DVec2 {
        let mut q = p;
        ts.map_point(&mut q);
        DVec2::new(q.x as f64 * k, -(q.y as f64) * k)
    };
    let mut out: Vec<Subpath> = Vec::new();
    let mut cur: Option<Subpath> = None;
    let mut last = DVec2::ZERO;
    let flush = |cur: &mut Option<Subpath>, out: &mut Vec<Subpath>| {
        if let Some(sp) = cur.take() {
            let finite = sp.start.is_finite() && sp.segs.iter().all(|s| s.end().is_finite());
            if finite && !sp.segs.is_empty() {
                out.push(sp);
            }
        }
    };
    for seg in data.segments() {
        match seg {
            S::MoveTo(p) => {
                flush(&mut cur, &mut out);
                last = map(p);
                cur = Some(Subpath { start: last, segs: Vec::new(), closed: false });
            }
            S::LineTo(p) => {
                let end = map(p);
                let sp = cur.get_or_insert_with(|| Subpath { start: last, segs: Vec::new(), closed: false });
                sp.segs.push(PathSeg::Line { end });
                last = end;
            }
            S::QuadTo(c, p) => {
                let end = map(p);
                let seg = PathSeg::cubic_from_quadratic(last, map(c), end);
                let sp = cur.get_or_insert_with(|| Subpath { start: last, segs: Vec::new(), closed: false });
                sp.segs.push(seg);
                last = end;
            }
            S::CubicTo(c1, c2, p) => {
                let end = map(p);
                let seg = PathSeg::Cubic { c1: map(c1), c2: map(c2), end };
                let sp = cur.get_or_insert_with(|| Subpath { start: last, segs: Vec::new(), closed: false });
                sp.segs.push(seg);
                last = end;
            }
            S::Close => {
                if let Some(sp) = cur.as_mut() {
                    sp.closed = true;
                    // Ruas penutup implisit: buang segmen garis terakhir yang
                    // hanya kembali ke titik awal (hindari ruas nol).
                    sp.normalize_closing();
                    last = sp.start;
                }
                flush(&mut cur, &mut out);
            }
        }
    }
    flush(&mut cur, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE_AND_CURVE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96" viewBox="0 0 96 96">
        <rect x="0" y="0" width="96" height="48" fill="none" stroke="black"/>
        <path d="M 0 96 Q 48 48 96 96" fill="none" stroke="black"/>
        <g transform="translate(10 0)"><circle cx="20" cy="70" r="5"/></g>
    </svg>"#;

    #[test]
    fn rect_quad_and_circle_become_paths_in_mm() {
        let r = import_svg_entities(
            SQUARE_AND_CURVE.as_bytes(),
            &SvgImportOptions { anchor_to_offset: false, ..Default::default() },
        )
        .unwrap();
        assert_eq!(r.entities.len(), 3);
        // Persegi 96 px = 25.4 mm lebar, Y dibalik (0..-12.7).
        let (min, max) = r.entities[0].bounding_box().unwrap();
        assert!((max.x - min.x - 25.4).abs() < 1e-3, "{min:?} {max:?}");
        assert!((min.y + 12.7).abs() < 1e-3 && max.y.abs() < 1e-3);
        match &r.entities[0] {
            Entity::Path { subpaths, .. } => assert!(subpaths[0].closed),
            other => panic!("{other:?}"),
        }
        // Kurva kuadratik menjadi kubik.
        match &r.entities[1] {
            Entity::Path { subpaths, .. } => {
                assert!(matches!(subpaths[0].segs[0], PathSeg::Cubic { .. }));
            }
            other => panic!("{other:?}"),
        }
        assert!(r.segments >= 6);
    }

    #[test]
    fn scale_and_anchor_are_applied() {
        let r = import_svg_entities(
            SQUARE_AND_CURVE.as_bytes(),
            &SvgImportOptions { scale: 2.0, anchor_to_offset: true, offset: DVec2::new(5.0, 5.0) },
        )
        .unwrap();
        let (min, max) = bbox(&r.entities).unwrap();
        assert!((min - DVec2::new(5.0, 5.0)).length() < 1e-6);
        assert!((max.x - min.x - 50.8).abs() < 1e-3);
    }

    #[test]
    fn hostile_inputs_error_without_panic() {
        let o = SvgImportOptions::default();
        assert!(import_svg_entities(b"", &o).is_err());
        assert!(import_svg_entities(b"bukan svg <<<", &o).is_err());
        assert!(import_svg_entities(b"<svg xmlns='http://www.w3.org/2000/svg'></svg>", &o).is_err());
        assert!(import_svg_entities(
            b"<svg xmlns='http://www.w3.org/2000/svg' width='10' height='10'><path d='M 0 0 L nan 5 Z'/></svg>",
            &o
        )
        .is_err());
        let bad = SvgImportOptions { scale: f64::NAN, ..Default::default() };
        assert!(import_svg_entities(SQUARE_AND_CURVE.as_bytes(), &bad).is_err());
        let neg = SvgImportOptions { scale: -1.0, ..Default::default() };
        assert!(import_svg_entities(SQUARE_AND_CURVE.as_bytes(), &neg).is_err());
        // Biner acak.
        assert!(import_svg_entities(&[0xff, 0x00, 0x1f, 0x8b, 0x42], &o).is_err());
    }
}
