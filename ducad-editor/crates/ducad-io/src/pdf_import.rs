//! Impor PDF vektor (W6): garis dari gambar teknik lama menjadi polyline
//! dalam mm, untuk dipakai sebagai sketsa referensi.
//!
//! Sengaja sederhana dan tanpa dependensi PDF: berkas dipindai untuk objek
//! stream, yang tak terkompresi atau ber-`/FlateDecode` diurai sebagai
//! content stream, dan operator lintasan (`m l c v y h re`) yang DIGAMBAR
//! (`S s f F B b` beserta varian `*`) dikumpulkan dengan matriks `cm`/`q`/`Q`.
//!
//! Batasannya dinyatakan apa adanya: teks, gambar raster, pola isian, dan
//! lintasan pemotong (`n`) tidak diimpor; PDF terenkripsi dan filter selain
//! Flate dilewati dengan peringatan; karena pohon halaman tidak dibaca,
//! "halaman" berarti content stream ke-n yang memuat lintasan (biasanya sama
//! dengan nomor halaman pada PDF hasil ekspor CAD).

use anyhow::{bail, Result};

const PT_TO_MM: f64 = 25.4 / 72.0;

/// Satu rantai garis dalam mm (Y ke atas, asal = pojok kiri-bawah halaman).
#[derive(Debug, Clone, PartialEq)]
pub struct PdfPolyline {
    pub points: Vec<[f64; 2]>,
    pub closed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PdfImportOptions {
    /// Content stream ke-n yang memuat lintasan (mulai 1); `None` = semua.
    pub page: Option<usize>,
    /// Jumlah ruas garis per kurva Bezier.
    pub curve_segments: usize,
}

impl Default for PdfImportOptions {
    fn default() -> Self {
        Self {
            page: Some(1),
            curve_segments: 8,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PdfImport {
    pub polylines: Vec<PdfPolyline>,
    /// Kotak pembatas semua titik, mm.
    pub bbox: Option<([f64; 2], [f64; 2])>,
    /// Jumlah content stream yang memuat lintasan di berkas.
    pub pages: usize,
    pub warnings: Vec<String>,
}

impl PdfImport {
    pub fn point_count(&self) -> usize {
        self.polylines.iter().map(|p| p.points.len()).sum()
    }
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from >= hay.len() {
        return None;
    }
    hay[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|i| i + from)
}

fn rfind(hay: &[u8], needle: &[u8], to: usize) -> Option<usize> {
    hay[..to.min(hay.len())]
        .windows(needle.len())
        .rposition(|w| w == needle)
}

/// Nilai `/Length` langsung (bukan rujukan `n 0 R`).
fn direct_length(dict: &[u8]) -> Option<usize> {
    let at = find(dict, b"/Length", 0)? + 7;
    let rest = std::str::from_utf8(&dict[at..]).ok()?;
    let mut tokens = rest.split_whitespace();
    let first = tokens.next()?;
    let digits: String = first.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() || digits.len() != first.trim_end_matches(['>', '/']).len() {
        return None;
    }
    // `12 0 R` = rujukan tak langsung.
    if tokens.next() == Some("0") && tokens.next().is_some_and(|t| t.starts_with('R')) {
        return None;
    }
    digits.parse().ok()
}

fn has(dict: &[u8], key: &[u8]) -> bool {
    find(dict, key, 0).is_some()
}

/// Isi mentah setiap objek stream beserta kamusnya.
fn streams(bytes: &[u8]) -> Vec<(&[u8], &[u8])> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(pos) = find(bytes, b"stream", at) {
        at = pos + 6;
        // "endstream" juga memuat "stream".
        if pos >= 3 && &bytes[pos - 3..pos] == b"end" {
            continue;
        }
        let data_start = match bytes.get(pos + 6..pos + 8) {
            Some(b"\r\n") => pos + 8,
            Some([b'\n', _]) => pos + 7,
            _ => continue,
        };
        let dict_end = match rfind(bytes, b">>", pos) {
            Some(e) => e,
            None => continue,
        };
        let obj_start = rfind(bytes, b"obj", dict_end).unwrap_or(0);
        let dict = &bytes[obj_start..dict_end];
        let end = direct_length(dict)
            .map(|len| data_start + len)
            .filter(|e| {
                *e <= bytes.len() && find(bytes, b"endstream", *e).is_some_and(|x| x <= *e + 4)
            })
            .or_else(|| find(bytes, b"endstream", data_start));
        let Some(end) = end else { continue };
        out.push((dict, &bytes[data_start..end]));
        at = end;
    }
    out
}

#[derive(Clone, Copy)]
struct Mat([f64; 6]);

impl Mat {
    const ID: Mat = Mat([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    fn apply(&self, x: f64, y: f64) -> [f64; 2] {
        let m = &self.0;
        [
            (m[0] * x + m[2] * y + m[4]) * PT_TO_MM,
            (m[1] * x + m[3] * y + m[5]) * PT_TO_MM,
        ]
    }
    /// `self` = matriks baru × matriks lama (aturan `cm`).
    fn pre(&self, n: [f64; 6]) -> Mat {
        let m = &self.0;
        Mat([
            n[0] * m[0] + n[1] * m[2],
            n[0] * m[1] + n[1] * m[3],
            n[2] * m[0] + n[3] * m[2],
            n[2] * m[1] + n[3] * m[3],
            n[4] * m[0] + n[5] * m[2] + m[4],
            n[4] * m[1] + n[5] * m[3] + m[5],
        ])
    }
}

enum Tok<'a> {
    Num(f64),
    Op(&'a [u8]),
}

/// Tokenizer content stream: angka dan operator; string, nama, larik, kamus,
/// dan gambar inline dilewati.
fn tokens(data: &[u8]) -> Vec<Tok<'_>> {
    let mut out = Vec::new();
    let mut i = 0;
    let is_ws = |b: u8| matches!(b, b' ' | b'\n' | b'\r' | b'\t' | 0x0c | 0);
    let is_delim = |b: u8| {
        matches!(
            b,
            b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
        )
    };
    while i < data.len() {
        let b = data[i];
        if is_ws(b) {
            i += 1;
        } else if b == b'%' {
            while i < data.len() && data[i] != b'\n' && data[i] != b'\r' {
                i += 1;
            }
        } else if b == b'(' {
            let mut depth = 0usize;
            while i < data.len() {
                match data[i] {
                    b'\\' => i += 1,
                    b'(' => depth += 1,
                    b')' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            i += 1;
        } else if b == b'<' {
            if data.get(i + 1) == Some(&b'<') {
                i += 2;
            } else {
                while i < data.len() && data[i] != b'>' {
                    i += 1;
                }
                i += 1;
            }
        } else if b == b'/' {
            i += 1;
            while i < data.len() && !is_ws(data[i]) && !is_delim(data[i]) {
                i += 1;
            }
        } else if is_delim(b) {
            i += 1;
        } else {
            let start = i;
            while i < data.len() && !is_ws(data[i]) && !is_delim(data[i]) {
                i += 1;
            }
            let word = &data[start..i];
            if word == b"BI" {
                // Gambar inline: lewati sampai `EI` yang berdiri sendiri.
                while let Some(e) = find(data, b"EI", i) {
                    i = e + 2;
                    let before = e == 0 || is_ws(data[e - 1]);
                    let after = data.get(i).is_none_or(|c| is_ws(*c));
                    if before && after {
                        break;
                    }
                }
                continue;
            }
            match std::str::from_utf8(word)
                .ok()
                .and_then(|w| w.parse::<f64>().ok())
            {
                Some(n) if n.is_finite() => out.push(Tok::Num(n)),
                _ => out.push(Tok::Op(word)),
            }
        }
    }
    out
}

fn bezier(out: &mut Vec<[f64; 2]>, p: [[f64; 2]; 4], segments: usize) {
    let n = segments.max(1);
    for k in 1..=n {
        let t = k as f64 / n as f64;
        let u = 1.0 - t;
        let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
        out.push([
            w[0] * p[0][0] + w[1] * p[1][0] + w[2] * p[2][0] + w[3] * p[3][0],
            w[0] * p[0][1] + w[1] * p[1][1] + w[2] * p[2][1] + w[3] * p[3][1],
        ]);
    }
}

/// Lintasan yang digambar oleh satu content stream.
fn paths_of(data: &[u8], segments: usize) -> Vec<PdfPolyline> {
    let mut done: Vec<PdfPolyline> = Vec::new();
    let mut pending: Vec<PdfPolyline> = Vec::new();
    let mut current: Vec<[f64; 2]> = Vec::new();
    // Titik terkini dalam ruang pengguna (sebelum CTM) untuk `v`.
    let mut cursor = [0.0f64; 2];
    let mut ctm = Mat::ID;
    let mut stack: Vec<Mat> = Vec::new();
    let mut args: Vec<f64> = Vec::new();

    let finish = |current: &mut Vec<[f64; 2]>, pending: &mut Vec<PdfPolyline>, closed: bool| {
        if current.len() >= 2 {
            pending.push(PdfPolyline {
                points: std::mem::take(current),
                closed,
            });
        } else {
            current.clear();
        }
    };

    for tok in tokens(data) {
        let op = match tok {
            Tok::Num(n) => {
                args.push(n);
                continue;
            }
            Tok::Op(op) => op,
        };
        let a = |i: usize| args.get(args.len().wrapping_sub(i)).copied();
        match op {
            b"q" => stack.push(ctm),
            b"Q" => ctm = stack.pop().unwrap_or(Mat::ID),
            b"cm" => {
                if let (Some(a0), Some(b0), Some(c0), Some(d0), Some(e0), Some(f0)) =
                    (a(6), a(5), a(4), a(3), a(2), a(1))
                {
                    ctm = ctm.pre([a0, b0, c0, d0, e0, f0]);
                }
            }
            b"m" => {
                finish(&mut current, &mut pending, false);
                if let (Some(x), Some(y)) = (a(2), a(1)) {
                    cursor = [x, y];
                    current.push(ctm.apply(x, y));
                }
            }
            b"l" => {
                if let (Some(x), Some(y)) = (a(2), a(1)) {
                    cursor = [x, y];
                    current.push(ctm.apply(x, y));
                }
            }
            b"c" | b"v" | b"y" => {
                let pts = match op {
                    b"c" => (a(6), a(5), a(4), a(3), a(2), a(1)),
                    // `v`: kendali pertama = titik terkini.
                    b"v" => (Some(cursor[0]), Some(cursor[1]), a(4), a(3), a(2), a(1)),
                    // `y`: kendali kedua = titik akhir.
                    _ => (a(4), a(3), a(2), a(1), a(2), a(1)),
                };
                if let (Some(x1), Some(y1), Some(x2), Some(y2), Some(x3), Some(y3)) = pts {
                    if let Some(start) = current.last().copied() {
                        bezier(
                            &mut current,
                            [
                                start,
                                ctm.apply(x1, y1),
                                ctm.apply(x2, y2),
                                ctm.apply(x3, y3),
                            ],
                            segments,
                        );
                    }
                    cursor = [x3, y3];
                }
            }
            b"h" => finish(&mut current, &mut pending, true),
            b"re" => {
                finish(&mut current, &mut pending, false);
                if let (Some(x), Some(y), Some(w), Some(h)) = (a(4), a(3), a(2), a(1)) {
                    pending.push(PdfPolyline {
                        points: vec![
                            ctm.apply(x, y),
                            ctm.apply(x + w, y),
                            ctm.apply(x + w, y + h),
                            ctm.apply(x, y + h),
                        ],
                        closed: true,
                    });
                    cursor = [x, y];
                }
            }
            // Menggaris dan/atau mengisi: lintasan menjadi geometri.
            b"S" | b"f" | b"F" | b"f*" | b"B" | b"B*" => {
                finish(&mut current, &mut pending, false);
                done.append(&mut pending);
            }
            // Varian yang menutup lintasan dulu.
            b"s" | b"b" | b"b*" => {
                finish(&mut current, &mut pending, true);
                done.append(&mut pending);
            }
            // Akhiri tanpa menggambar (lintasan pemotong): buang.
            b"n" => {
                current.clear();
                pending.clear();
            }
            _ => {}
        }
        args.clear();
    }
    done
}

/// Ambil garis vektor dari berkas PDF.
pub fn import_pdf_vectors(bytes: &[u8], options: &PdfImportOptions) -> Result<PdfImport> {
    if !bytes.starts_with(b"%PDF-") {
        bail!("not a PDF file (missing %PDF- header)");
    }
    if find(bytes, b"/Encrypt", 0).is_some() {
        bail!("the PDF is encrypted; remove the password protection first");
    }
    let mut import = PdfImport::default();
    let mut skipped_filter = 0usize;
    let mut pages: Vec<Vec<PdfPolyline>> = Vec::new();
    for (dict, raw) in streams(bytes) {
        // Bukan content stream: gambar, font, metadata, tabel rujukan.
        let other = [
            b"/Subtype /Image".as_slice(),
            b"/Subtype/Image",
            b"/Type /XRef",
            b"/Type/XRef",
            b"/Type /ObjStm",
            b"/Type/ObjStm",
            b"/Type /Metadata",
            b"/Type/Metadata",
            b"/Length1",
            b"/FontFile",
        ];
        if other.iter().any(|k| has(dict, k)) {
            continue;
        }
        let data: Vec<u8> = if has(dict, b"/Filter") {
            let flate_only = has(dict, b"FlateDecode")
                && ![
                    "ASCII85Decode",
                    "ASCIIHexDecode",
                    "LZWDecode",
                    "DCTDecode",
                    "CCITTFaxDecode",
                    "JBIG2Decode",
                    "JPXDecode",
                    "RunLengthDecode",
                ]
                .iter()
                .any(|f| has(dict, f.as_bytes()));
            if !flate_only {
                skipped_filter += 1;
                continue;
            }
            match miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(raw, 64 * 1024 * 1024) {
                Ok(d) => d,
                Err(_) => continue,
            }
        } else {
            raw.to_vec()
        };
        let paths = paths_of(&data, options.curve_segments);
        if !paths.is_empty() {
            pages.push(paths);
        }
    }
    import.pages = pages.len();
    if skipped_filter > 0 {
        import.warnings.push(format!(
            "PDF_IMPORT_FILTER: {skipped_filter} stream(s) use a compression other than Flate and were skipped"
        ));
    }
    if pages.is_empty() {
        bail!("no vector paths found: the PDF is probably a scan (raster image) or only contains text");
    }
    import.polylines = match options.page {
        Some(n) => {
            let index = n.checked_sub(1).filter(|i| *i < pages.len());
            match index {
                Some(i) => pages.swap_remove(i),
                None => bail!(
                    "page {n} does not exist: the PDF has {} page(s) with vector paths",
                    pages.len()
                ),
            }
        }
        None => pages.into_iter().flatten().collect(),
    };
    let mut bbox: Option<([f64; 2], [f64; 2])> = None;
    for p in import.polylines.iter().flat_map(|l| l.points.iter()) {
        let (lo, hi) = bbox.get_or_insert((*p, *p));
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    import.bbox = bbox;
    Ok(import)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pdf_with(streams: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let mut out = b"%PDF-1.4\n".to_vec();
        for (i, (dict, data)) in streams.iter().enumerate() {
            out.extend_from_slice(
                format!(
                    "{} 0 obj\n<< {dict} /Length {} >>\nstream\n",
                    i + 1,
                    data.len()
                )
                .as_bytes(),
            );
            out.extend_from_slice(data);
            out.extend_from_slice(b"\nendstream\nendobj\n");
        }
        out.extend_from_slice(b"%%EOF\n");
        out
    }

    fn close(a: [f64; 2], b: [f64; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6
    }

    #[test]
    fn lines_rects_and_transforms_become_mm_polylines() {
        // 72 pt = 25,4 mm. Garis digeser 72 pt lewat `cm`, lalu `Q` memulihkannya.
        let content = b"q 1 0 0 1 72 0 cm 0 0 m 72 0 l 72 72 l S Q\n\
                        0 0 144 72 re f\n\
                        (teks dengan ) kurung \\( dan m l S di dalamnya) Tj\n\
                        10 10 m 20 20 l n\n"
            .to_vec();
        let pdf = pdf_with(&[("", content)]);
        let import = import_pdf_vectors(&pdf, &PdfImportOptions::default()).unwrap();
        assert_eq!(import.pages, 1);
        assert_eq!(
            import.polylines.len(),
            2,
            "lintasan `n` dan teks tidak ikut"
        );
        let line = &import.polylines[0];
        assert!(!line.closed);
        assert!(close(line.points[0], [25.4, 0.0]));
        assert!(close(line.points[2], [50.8, 25.4]));
        let rect = &import.polylines[1];
        assert!(rect.closed && rect.points.len() == 4);
        assert!(close(rect.points[2], [50.8, 25.4]));
        let (lo, hi) = import.bbox.unwrap();
        assert!(close(lo, [0.0, 0.0]) && close(hi, [50.8, 25.4]));
    }

    #[test]
    fn flate_streams_are_inflated_and_images_are_ignored() {
        let content = b"0 0 m 72 0 72 72 0 72 c h S\n".to_vec();
        let packed = miniz_oxide::deflate::compress_to_vec_zlib(&content, 6);
        let pdf = pdf_with(&[
            (
                "/Type /XObject /Subtype /Image /Width 1 /Height 1",
                b"0 0 m 9 9 l S".to_vec(),
            ),
            ("/Filter /FlateDecode", packed),
            ("/Filter /LZWDecode", vec![1, 2, 3]),
        ]);
        let import = import_pdf_vectors(
            &pdf,
            &PdfImportOptions {
                page: None,
                curve_segments: 4,
            },
        )
        .unwrap();
        assert_eq!(import.polylines.len(), 1);
        let curve = &import.polylines[0];
        assert!(curve.closed);
        assert_eq!(curve.points.len(), 5, "titik awal + 4 ruas");
        assert!(close(curve.points[4], [0.0, 25.4]));
        assert!(import.warnings[0].starts_with("PDF_IMPORT_FILTER"));
    }

    #[test]
    fn pages_are_selectable_and_failures_explain_themselves() {
        let pdf = pdf_with(&[
            ("", b"0 0 m 10 0 l S".to_vec()),
            ("", b"0 0 m 0 10 l S 5 5 m 6 6 l S".to_vec()),
        ]);
        let second = import_pdf_vectors(
            &pdf,
            &PdfImportOptions {
                page: Some(2),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(second.polylines.len(), 2);
        assert_eq!(second.pages, 2);
        let err = import_pdf_vectors(
            &pdf,
            &PdfImportOptions {
                page: Some(3),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("page 3"));
        assert!(import_pdf_vectors(b"bukan pdf", &PdfImportOptions::default()).is_err());
        let scan = pdf_with(&[("/Subtype /Image", vec![0; 8])]);
        assert!(import_pdf_vectors(&scan, &PdfImportOptions::default())
            .unwrap_err()
            .to_string()
            .contains("scan"));
        let locked = pdf_with(&[("/Encrypt 5 0 R", b"0 0 m 1 1 l S".to_vec())]);
        assert!(import_pdf_vectors(&locked, &PdfImportOptions::default())
            .unwrap_err()
            .to_string()
            .contains("encrypted"));
    }

    #[test]
    fn ducad_own_drawing_pdf_round_trips_to_vectors() {
        use crate::drawing::pages::{document_pages, Block, Table};
        use crate::drawing::{PaperSize, TitleBlockInfo};
        use crate::pdf::{generate_pdf_document, PdfPage};
        let mut table = Table::new(&["A", "B"], &[1.0, 1.0]);
        table.row(["1", "2"]);
        let pages = document_pages(
            PaperSize::A4Landscape,
            &TitleBlockInfo::default(),
            &[Block::Table(table)],
        );
        let list: Vec<PdfPage<'_>> = pages
            .iter()
            .map(|p| PdfPage {
                sheet: &p.sheet,
                extra: &p.groups,
                bookmark: "",
            })
            .collect();
        let pdf = generate_pdf_document(&list);
        let import = import_pdf_vectors(&pdf, &PdfImportOptions::default()).unwrap();
        // Bingkai luar A4 lanskap 297 × 210 mm terbaca kembali.
        let (lo, hi) = import.bbox.unwrap();
        assert!(lo[0].abs() < 0.5 && lo[1].abs() < 0.5, "{lo:?}");
        assert!(
            (hi[0] - 297.0).abs() < 0.5 && (hi[1] - 210.0).abs() < 0.5,
            "{hi:?}"
        );
        assert!(import.polylines.len() > 10);
    }
}
