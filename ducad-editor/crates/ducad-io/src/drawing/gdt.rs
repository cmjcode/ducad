//! Geometri vektor anotasi GD&T/toleransi (P19), tidak bergantung format.
//!
//! Tiap anotasi diubah menjadi [`AnnotationGeometry`]: daftar path vektor
//! (garis + kurva Bezier kubik) dan potongan teks, dalam koordinat LOKAL mm
//! dengan sumbu Y ke atas dan titik jangkar di (0, 0). Penulis PDF dan SVG
//! memakai geometri yang sama, jadi simbol di kedua format selalu identik.
//!
//! Simbol (14 karakteristik ISO 1101, tanda diameter, lingkaran pengubah
//! M/L, segitiga datum, tanda kekasaran) murni path — tidak ada glyph font.
//! Hanya angka dan huruf yang lewat jalur teks.

use ducad_core::drawing_annot::{
    Annotation, DatumFeature, DimensionTolerance, FeatureControlFrame, GdtSymbol, HoleTable,
    RevisionTable, SurfaceFinish, ToleranceSpec,
};

/// Tinggi huruf anotasi, mm (ISO 3098).
pub const TEXT_MM: f32 = 3.5;
/// Tinggi bingkai kontrol fitur = 2x tinggi huruf.
pub const FRAME_HEIGHT_MM: f32 = 7.0;
/// Tebal garis anotasi, mm.
pub const STROKE_MM: f32 = 0.35;
/// Setengah rentang simbol karakteristik di dalam kompartemennya, mm.
pub const SYMBOL_HALF_MM: f32 = 2.2;

const KAPPA: f32 = 0.552_284_8;
const SIN60: f32 = 0.866_025_4;
const COS60: f32 = 0.5;
const INV_TAN60: f32 = 0.577_350_3;
const DIAG: f32 = std::f32::consts::FRAC_1_SQRT_2;
/// Tinggi huruf kapital terhadap ukuran font (Helvetica).
const CAP_RATIO: f32 = 0.72;

const TABLE_TITLE_H: f32 = 6.5;
const TABLE_ROW_H: f32 = 5.5;

/// Satu perintah path. Titik dalam mm lokal, Y ke atas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathCmd {
    Move([f32; 2]),
    Line([f32; 2]),
    /// Bezier kubik: dua titik kendali lalu titik akhir.
    Cubic([f32; 2], [f32; 2], [f32; 2]),
    Close,
}

/// Path vektor; `filled` = diisi hitam sekaligus digaris.
#[derive(Debug, Clone, PartialEq)]
pub struct VectorPath {
    pub cmds: Vec<PathCmd>,
    pub filled: bool,
}

/// Potongan teks (angka/huruf) pada garis dasar `pos`.
#[derive(Debug, Clone, PartialEq)]
pub struct TextRun {
    /// Awal garis dasar; bila `centered`, tengah garis dasar.
    pub pos: [f32; 2],
    pub text: String,
    pub size_mm: f32,
    pub centered: bool,
    pub bold: bool,
}

/// Hasil gambar satu anotasi dalam koordinat lokal + titik jangkarnya.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnnotationGeometry {
    /// Titik jangkar di kertas, mm dari pojok kiri-bawah.
    pub anchor: [f32; 2],
    pub paths: Vec<VectorPath>,
    pub texts: Vec<TextRun>,
}

impl AnnotationGeometry {
    fn line(&mut self, a: [f32; 2], b: [f32; 2]) {
        self.paths.push(line_path(a, b));
    }

    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.paths.push(polygon_path(
            &[[x, y], [x + w, y], [x + w, y + h], [x, y + h]],
            false,
        ));
    }

    fn text(&mut self, pos: [f32; 2], text: impl Into<String>, size_mm: f32) {
        self.texts.push(TextRun {
            pos,
            text: text.into(),
            size_mm,
            centered: false,
            bold: false,
        });
    }

    fn text_centered(&mut self, pos: [f32; 2], text: impl Into<String>, size_mm: f32) {
        self.texts.push(TextRun {
            pos,
            text: text.into(),
            size_mm,
            centered: true,
            bold: false,
        });
    }
}

fn line_path(a: [f32; 2], b: [f32; 2]) -> VectorPath {
    VectorPath {
        cmds: vec![PathCmd::Move(a), PathCmd::Line(b)],
        filled: false,
    }
}

fn polyline_path(points: &[[f32; 2]]) -> VectorPath {
    let mut cmds = Vec::with_capacity(points.len());
    for (i, p) in points.iter().enumerate() {
        cmds.push(if i == 0 {
            PathCmd::Move(*p)
        } else {
            PathCmd::Line(*p)
        });
    }
    VectorPath {
        cmds,
        filled: false,
    }
}

fn polygon_path(points: &[[f32; 2]], filled: bool) -> VectorPath {
    let mut path = polyline_path(points);
    path.cmds.push(PathCmd::Close);
    path.filled = filled;
    path
}

/// Lingkaran penuh sebagai empat Bezier kubik, mulai dari titik kanan,
/// berlawanan arah jarum jam.
fn circle_path(c: [f32; 2], r: f32) -> VectorPath {
    let [x, y] = c;
    let k = r * KAPPA;
    VectorPath {
        cmds: vec![
            PathCmd::Move([x + r, y]),
            PathCmd::Cubic([x + r, y + k], [x + k, y + r], [x, y + r]),
            PathCmd::Cubic([x - k, y + r], [x - r, y + k], [x - r, y]),
            PathCmd::Cubic([x - r, y - k], [x - k, y - r], [x, y - r]),
            PathCmd::Cubic([x + k, y - r], [x + r, y - k], [x + r, y]),
            PathCmd::Close,
        ],
        filled: false,
    }
}

/// Setengah lingkaran atas dari kanan ke kiri; `closed` menutupnya dengan
/// garis alas.
fn half_circle_path(c: [f32; 2], r: f32, closed: bool) -> VectorPath {
    let [x, y] = c;
    let k = r * KAPPA;
    let mut cmds = vec![
        PathCmd::Move([x + r, y]),
        PathCmd::Cubic([x + r, y + k], [x + k, y + r], [x, y + r]),
        PathCmd::Cubic([x - k, y + r], [x - r, y + k], [x - r, y]),
    ];
    if closed {
        cmds.push(PathCmd::Close);
    }
    VectorPath {
        cmds,
        filled: false,
    }
}

/// Panah miring 45 derajat (batang + kepala terisi) dari `tail` ke `tip`.
fn diagonal_arrow(
    tail: [f32; 2],
    tip: [f32; 2],
    head_len: f32,
    head_half_w: f32,
) -> [VectorPath; 2] {
    let base = [tip[0] - DIAG * head_len, tip[1] - DIAG * head_len];
    let left = [base[0] - DIAG * head_half_w, base[1] + DIAG * head_half_w];
    let right = [base[0] + DIAG * head_half_w, base[1] - DIAG * head_half_w];
    [
        line_path(tail, base),
        polygon_path(&[tip, left, right], true),
    ]
}

/// Path vektor simbol karakteristik ISO 1101 berpusat di `c` dengan setengah
/// rentang `u` mm.
pub fn symbol_paths(symbol: GdtSymbol, c: [f32; 2], u: f32) -> Vec<VectorPath> {
    let [x, y] = c;
    match symbol {
        GdtSymbol::Straightness => vec![line_path([x - u, y], [x + u, y])],
        GdtSymbol::Flatness => {
            let h = 0.55 * u;
            vec![polygon_path(
                &[
                    [x - u, y - h],
                    [x + 0.4 * u, y - h],
                    [x + u, y + h],
                    [x - 0.4 * u, y + h],
                ],
                false,
            )]
        }
        GdtSymbol::Circularity => vec![circle_path(c, 0.8 * u)],
        GdtSymbol::Cylindricity => {
            // Lingkaran diapit dua garis singgung miring 60 derajat.
            let r = 0.55 * u;
            let (ox, oy) = (SIN60 * r, COS60 * r);
            let (dx, dy) = (COS60 * u, SIN60 * u);
            vec![
                circle_path(c, r),
                line_path([x + ox - dx, y - oy - dy], [x + ox + dx, y - oy + dy]),
                line_path([x - ox - dx, y + oy - dy], [x - ox + dx, y + oy + dy]),
            ]
        }
        GdtSymbol::ProfileOfLine => vec![half_circle_path([x, y - 0.45 * u], 0.9 * u, false)],
        GdtSymbol::ProfileOfSurface => vec![half_circle_path([x, y - 0.45 * u], 0.9 * u, true)],
        GdtSymbol::Perpendicularity => {
            let base = y - 0.8 * u;
            vec![
                line_path([x - u, base], [x + u, base]),
                line_path([x, base], [x, y + 0.8 * u]),
            ]
        }
        GdtSymbol::Angularity => {
            // Dua kaki membentuk sudut 30 derajat, puncak di kiri-bawah.
            let base = y - 0.5 * u;
            vec![polyline_path(&[
                [x - u + 2.0 * u * SIN60, base + 2.0 * u * COS60],
                [x - u, base],
                [x + u, base],
            ])]
        }
        GdtSymbol::Parallelism => {
            let (dx, dy) = (0.45 * u, 0.78 * u);
            let o = 0.45 * u;
            vec![
                line_path([x - o - dx, y - dy], [x - o + dx, y + dy]),
                line_path([x + o - dx, y - dy], [x + o + dx, y + dy]),
            ]
        }
        GdtSymbol::Position => vec![
            circle_path(c, 0.6 * u),
            line_path([x - u, y], [x + u, y]),
            line_path([x, y - u], [x, y + u]),
        ],
        GdtSymbol::Concentricity => vec![circle_path(c, 0.85 * u), circle_path(c, 0.45 * u)],
        GdtSymbol::Symmetry => {
            let (o, short) = (0.55 * u, 0.6 * u);
            vec![
                line_path([x - short, y + o], [x + short, y + o]),
                line_path([x - u, y], [x + u, y]),
                line_path([x - short, y - o], [x + short, y - o]),
            ]
        }
        GdtSymbol::CircularRunout => {
            let d = 0.7 * u;
            diagonal_arrow([x - d, y - d], [x + d, y + d], 0.7 * u, 0.25 * u).to_vec()
        }
        GdtSymbol::TotalRunout => {
            let d = 0.7 * u;
            let o = 0.45 * u;
            let mut paths = Vec::with_capacity(5);
            paths.extend(diagonal_arrow(
                [x - o - d, y - d],
                [x - o + d, y + d],
                0.7 * u,
                0.25 * u,
            ));
            paths.extend(diagonal_arrow(
                [x + o - d, y - d],
                [x + o + d, y + d],
                0.7 * u,
                0.25 * u,
            ));
            paths.push(line_path([x - o - d, y - d], [x + o - d, y - d]));
            paths
        }
    }
}

/// Tanda diameter: lingkaran dicoret garis miring 60 derajat.
pub fn diameter_sign_paths(c: [f32; 2], r: f32) -> Vec<VectorPath> {
    let (dx, dy) = (COS60 * 1.55 * r, SIN60 * 1.55 * r);
    vec![
        circle_path(c, r),
        line_path([c[0] - dx, c[1] - dy], [c[0] + dx, c[1] + dy]),
    ]
}

/// Tanda datum: segitiga terisi (alas di jangkar), tangkai, dan kotak huruf.
pub fn datum_triangle_paths() -> Vec<VectorPath> {
    vec![
        polygon_path(&[[-2.0, 0.0], [2.0, 0.0], [0.0, 3.464]], true),
        line_path([0.0, 3.464], [0.0, FRAME_HEIGHT_MM]),
        polygon_path(
            &[
                [-3.5, FRAME_HEIGHT_MM],
                [3.5, FRAME_HEIGHT_MM],
                [3.5, 2.0 * FRAME_HEIGHT_MM],
                [-3.5, 2.0 * FRAME_HEIGHT_MM],
            ],
            false,
        ),
    ]
}

/// Tanda kekasaran permukaan (centang ISO 1302) dengan ujung bawah di
/// jangkar dan garis mendatar sepanjang `bar_len` di puncak kaki panjang.
pub fn surface_finish_paths(bar_len: f32) -> Vec<VectorPath> {
    let (short_h, long_h) = (5.0, 10.5);
    let top_right = [long_h * INV_TAN60, long_h];
    vec![polyline_path(&[
        [-short_h * INV_TAN60, short_h],
        [0.0, 0.0],
        top_right,
        [top_right[0] + bar_len, long_h],
    ])]
}

/// Perkiraan lebar teks Helvetica dalam mm (cukup untuk tata letak
/// kompartemen; bukan pengukuran font yang tepat).
pub fn text_width_mm(text: &str, size_mm: f32) -> f32 {
    let em: f32 = text
        .chars()
        .map(|c| match c {
            '0'..='9' => 0.556,
            '.' | ',' | ' ' | '/' | ':' => 0.278,
            '(' | ')' | '-' => 0.333,
            '+' => 0.584,
            'M' | 'W' => 0.9,
            'I' => 0.3,
            'A'..='Z' => 0.7,
            'i' | 'j' | 'l' | 't' | 'f' => 0.3,
            _ => 0.556,
        })
        .sum();
    em * size_mm
}

/// Angka tanpa nol di belakang, paling banyak `max_decimals` desimal.
fn fmt_trim(value: f64, max_decimals: usize) -> String {
    let mut text = format!("{value:.max_decimals$}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    if text == "-0" {
        text = "0".to_string();
    }
    text
}

/// Deviasi bertanda: `+0.021`, `-0.007`, atau `0`.
fn fmt_deviation(value: f64) -> String {
    let text = fmt_trim(value, 4);
    if text == "0" || text.starts_with('-') {
        text
    } else {
        format!("+{text}")
    }
}

/// Deviasi kelas ISO: minimal 3 desimal (`-0.020`), 4 bila perlu
/// (`+0.0105`), dan `0` untuk nol.
fn fmt_fit_deviation(value: f64) -> String {
    let mut text = format!("{value:+.4}");
    if text.ends_with('0') {
        text.pop();
    }
    if text == "+0.000" || text == "-0.000" {
        text = "0".to_string();
    }
    text
}

/// Teks satu baris sebuah dimensi berkelas ISO, mis. `25 H7 (+0.021/0)`.
/// Bila kelas tidak bisa diurai untuk ukuran itu, batasnya dihilangkan.
pub fn fit_dimension_text(dimension: &DimensionTolerance) -> Option<String> {
    let ToleranceSpec::Fit(fit) = &dimension.tolerance else {
        return None;
    };
    let nominal = fmt_trim(dimension.nominal, 3);
    Some(match dimension.deviations() {
        Ok((upper, lower)) => format!(
            "{nominal} {fit} ({}/{})",
            fmt_fit_deviation(upper),
            fmt_fit_deviation(lower)
        ),
        Err(_) => format!("{nominal} {fit}"),
    })
}

fn dimension_geometry(g: &mut AnnotationGeometry, dimension: &DimensionTolerance) {
    let mut x = 0.0;
    if dimension.diameter {
        let r = 1.3;
        g.paths
            .extend(diameter_sign_paths([r + 0.3, TEXT_MM * CAP_RATIO * 0.5], r));
        x = 2.0 * r + 1.4;
    }
    match &dimension.tolerance {
        ToleranceSpec::Fit(_) => {
            if let Some(text) = fit_dimension_text(dimension) {
                g.text([x, 0.0], text, TEXT_MM);
            }
        }
        ToleranceSpec::Limits { plus, minus } => {
            let nominal = fmt_trim(dimension.nominal, 3);
            let after = x + text_width_mm(&nominal, TEXT_MM) + 0.9;
            g.text([x, 0.0], nominal, TEXT_MM);
            if (plus - minus).abs() < 1e-12 {
                // Simetris: tanda plus-minus sebagai path, lalu nilainya.
                let s = 2.2;
                let plus_y = 1.75;
                g.line([after, plus_y], [after + s, plus_y]);
                g.line(
                    [after + s * 0.5, plus_y - s * 0.5],
                    [after + s * 0.5, plus_y + s * 0.5],
                );
                g.line([after, 0.1], [after + s, 0.1]);
                g.text([after + s + 0.7, 0.0], fmt_trim(plus.abs(), 4), TEXT_MM);
            } else {
                // Tak simetris: batas atas di atas batas bawah, huruf kecil.
                let small = 2.5;
                g.text([after, 2.0], fmt_deviation(*plus), small);
                g.text([after, -1.2], fmt_deviation(-*minus), small);
            }
        }
    }
}

fn frame_geometry(g: &mut AnnotationGeometry, frame: &FeatureControlFrame) {
    let h = FRAME_HEIGHT_MM;
    let baseline = (h - TEXT_MM * CAP_RATIO) * 0.5;
    let pad = 1.2;
    let mut dividers: Vec<f32> = Vec::new();

    // Kompartemen 1: simbol karakteristik.
    g.paths.extend(symbol_paths(
        frame.symbol,
        [h * 0.5, h * 0.5],
        SYMBOL_HALF_MM,
    ));
    let mut x = h;
    dividers.push(x);

    // Kompartemen 2: [tanda diameter] nilai [pengubah M/L].
    x += pad;
    if frame.diameter_zone {
        let r = 1.3;
        g.paths.extend(diameter_sign_paths([x + r, h * 0.5], r));
        x += 2.0 * r + 1.0;
    }
    let value = fmt_trim(frame.value, 4);
    let value_w = text_width_mm(&value, TEXT_MM);
    g.text([x, baseline], value, TEXT_MM);
    x += value_w;
    for modifier in &frame.modifiers {
        let r = 1.9;
        x += 0.8;
        g.paths.push(circle_path([x + r, h * 0.5], r));
        let size = 2.5;
        g.text_centered(
            [x + r, h * 0.5 - size * CAP_RATIO * 0.5],
            modifier.letter(),
            size,
        );
        x += 2.0 * r;
    }
    x += pad;

    // Kompartemen datum.
    for datum in &frame.datums {
        dividers.push(x);
        let w = (text_width_mm(datum, TEXT_MM) + 2.0 * pad).max(h);
        g.text_centered([x + w * 0.5, baseline], datum.clone(), TEXT_MM);
        x += w;
    }

    g.rect(0.0, 0.0, x, h);
    for divider in dividers {
        g.line([divider, 0.0], [divider, h]);
    }
}

fn datum_geometry(g: &mut AnnotationGeometry, datum: &DatumFeature) {
    g.paths.extend(datum_triangle_paths());
    g.text_centered(
        [0.0, FRAME_HEIGHT_MM * 1.5 - TEXT_MM * CAP_RATIO * 0.5],
        datum.label.clone(),
        TEXT_MM,
    );
}

fn finish_geometry(g: &mut AnnotationGeometry, finish: &SurfaceFinish) {
    let size = 3.0;
    let text = format!("Ra {}", fmt_trim(finish.ra_um, 3));
    let width = text_width_mm(&text, size);
    g.paths.extend(surface_finish_paths(width + 2.0));
    g.text(
        [10.5 * INV_TAN60 + 1.0, 10.5 - 0.9 - size * CAP_RATIO],
        text,
        size,
    );
}

/// Tabel sederhana berjangkar di pojok kiri-bawah: judul, kepala kolom,
/// lalu baris data dari atas ke bawah.
fn table_geometry(
    g: &mut AnnotationGeometry,
    title: &str,
    headers: &[&str],
    widths: &[f32],
    rows: &[Vec<String>],
) {
    let width: f32 = widths.iter().sum();
    let height = TABLE_TITLE_H + TABLE_ROW_H * (rows.len() as f32 + 1.0);
    let header_top = height - TABLE_TITLE_H;

    g.rect(0.0, 0.0, width, height);
    g.line([0.0, header_top], [width, header_top]);
    for i in 0..rows.len() {
        let y = header_top - TABLE_ROW_H * (i as f32 + 1.0);
        g.line([0.0, y], [width, y]);
    }
    let mut x = 0.0;
    for w in widths.iter().take(widths.len().saturating_sub(1)) {
        x += w;
        g.line([x, 0.0], [x, header_top]);
    }

    let bold = |g: &mut AnnotationGeometry, pos: [f32; 2], text: &str, size_mm: f32| {
        g.texts.push(TextRun {
            pos,
            text: text.to_string(),
            size_mm,
            centered: false,
            bold: true,
        });
    };
    bold(g, [1.5, header_top + 2.0], title, 2.8);
    let mut x = 0.0;
    for (header, w) in headers.iter().zip(widths) {
        bold(g, [x + 1.5, header_top - TABLE_ROW_H + 1.7], header, 2.3);
        x += w;
    }
    for (i, row) in rows.iter().enumerate() {
        let y = header_top - TABLE_ROW_H * (i as f32 + 2.0) + 1.7;
        let mut x = 0.0;
        for (cell, w) in row.iter().zip(widths) {
            if !cell.is_empty() {
                g.text([x + 1.5, y], cell.clone(), 2.3);
            }
            x += w;
        }
    }
}

fn hole_table_geometry(g: &mut AnnotationGeometry, table: &HoleTable) {
    let title = if table.title.trim().is_empty() {
        "HOLE TABLE"
    } else {
        table.title.as_str()
    };
    let rows: Vec<Vec<String>> = table
        .rows
        .iter()
        .map(|row| {
            vec![
                row.tag.clone(),
                format!("{:.2}", row.x),
                format!("{:.2}", row.y),
                fmt_trim(row.diameter, 3),
                match row.depth {
                    Some(depth) => fmt_trim(depth, 3),
                    None => "THRU".to_string(),
                },
                row.note.clone(),
            ]
        })
        .collect();
    table_geometry(
        g,
        title,
        &["TAG", "X", "Y", "DIA", "DEPTH", "NOTE"],
        &[14.0, 20.0, 20.0, 18.0, 18.0, 40.0],
        &rows,
    );
}

fn revision_table_geometry(g: &mut AnnotationGeometry, table: &RevisionTable) {
    let rows: Vec<Vec<String>> = table
        .rows
        .iter()
        .map(|row| {
            vec![
                row.rev.clone(),
                row.description.clone(),
                row.date.clone(),
                row.by.clone(),
            ]
        })
        .collect();
    table_geometry(
        g,
        "REVISIONS",
        &["REV", "DESCRIPTION", "DATE", "BY"],
        &[12.0, 70.0, 24.0, 24.0],
        &rows,
    );
}

/// Geometri sebuah anotasi. `None` bila titik jangkarnya bukan angka hingga
/// (anotasi seperti itu dilewati saat ekspor, bukan merusak berkas).
pub fn annotation_geometry(annotation: &Annotation) -> Option<AnnotationGeometry> {
    let position = annotation.position();
    if !position[0].is_finite() || !position[1].is_finite() {
        return None;
    }
    let mut g = AnnotationGeometry {
        anchor: [position[0] as f32, position[1] as f32],
        ..AnnotationGeometry::default()
    };
    match annotation {
        Annotation::DimensionTolerance { dimension, .. } => dimension_geometry(&mut g, dimension),
        Annotation::FeatureControlFrame { frame, .. } => frame_geometry(&mut g, frame),
        Annotation::DatumFeature { datum, .. } => datum_geometry(&mut g, datum),
        Annotation::SurfaceFinish { finish, .. } => finish_geometry(&mut g, finish),
        Annotation::HoleTable { table, .. } => hole_table_geometry(&mut g, table),
        Annotation::RevisionTable { table, .. } => revision_table_geometry(&mut g, table),
    }
    Some(g)
}

/// Bentuk teks kanonik sekumpulan path (3 desimal) — masukan hash snapshot.
pub fn path_signature(paths: &[VectorPath]) -> String {
    // `+ 0.0` menormalkan nol negatif supaya tidak tertulis "-0.000".
    let p = |pt: &[f32; 2]| format!("{:.3},{:.3}", pt[0] + 0.0, pt[1] + 0.0);
    let mut out = String::new();
    for path in paths {
        for cmd in &path.cmds {
            match cmd {
                PathCmd::Move(a) => out.push_str(&format!("M{} ", p(a))),
                PathCmd::Line(a) => out.push_str(&format!("L{} ", p(a))),
                PathCmd::Cubic(a, b, c) => out.push_str(&format!("C{} {} {} ", p(a), p(b), p(c))),
                PathCmd::Close => out.push_str("Z "),
            }
        }
        out.push_str(if path.filled { "F;" } else { "S;" });
    }
    out
}

/// FNV-1a 64-bit: stabil lintas platform dan versi Rust.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Hash snapshot path vektor sebuah simbol pada ukuran bakunya (pusat di
/// titik asal, setengah rentang [`SYMBOL_HALF_MM`]).
pub fn symbol_path_hash(symbol: GdtSymbol) -> u64 {
    fnv1a64(path_signature(&symbol_paths(symbol, [0.0, 0.0], SYMBOL_HALF_MM)).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_core::drawing_annot::MaterialModifier;
    use ducad_core::IsoFit;

    fn bounds(paths: &[VectorPath]) -> ([f32; 2], [f32; 2]) {
        let mut min = [f32::MAX; 2];
        let mut max = [f32::MIN; 2];
        let mut add = |p: &[f32; 2]| {
            for i in 0..2 {
                min[i] = min[i].min(p[i]);
                max[i] = max[i].max(p[i]);
            }
        };
        for path in paths {
            for cmd in &path.cmds {
                match cmd {
                    PathCmd::Move(a) | PathCmd::Line(a) => add(a),
                    PathCmd::Cubic(a, b, c) => {
                        add(a);
                        add(b);
                        add(c);
                    }
                    PathCmd::Close => {}
                }
            }
        }
        (min, max)
    }

    #[test]
    fn gdt_symbols_fit_inside_their_compartment() {
        let half = FRAME_HEIGHT_MM * 0.5;
        for symbol in GdtSymbol::ALL {
            let paths = symbol_paths(symbol, [0.0, 0.0], SYMBOL_HALF_MM);
            assert!(!paths.is_empty(), "{symbol:?} tanpa path");
            let (min, max) = bounds(&paths);
            for i in 0..2 {
                assert!(
                    min[i] > -half + STROKE_MM && max[i] < half - STROKE_MM,
                    "{symbol:?} keluar kompartemen: {min:?}..{max:?}"
                );
            }
            // Tiap path diawali Move dan semua titiknya hingga.
            for path in &paths {
                assert!(matches!(path.cmds.first(), Some(PathCmd::Move(_))));
            }
            assert!(min.iter().chain(max.iter()).all(|v| v.is_finite()));
        }
    }

    #[test]
    fn gdt_symbols_are_all_distinct() {
        let hashes: Vec<u64> = GdtSymbol::ALL
            .iter()
            .map(|s| symbol_path_hash(*s))
            .collect();
        for (i, hash) in hashes.iter().enumerate() {
            assert!(
                !hashes[..i].contains(hash),
                "{:?} sama dengan simbol lain",
                GdtSymbol::ALL[i]
            );
        }
    }

    #[test]
    fn gdt_frame_layout_has_one_compartment_per_part() {
        let frame = FeatureControlFrame {
            symbol: GdtSymbol::Position,
            value: 0.1,
            diameter_zone: true,
            modifiers: vec![MaterialModifier::Mmc],
            datums: vec!["A".to_string(), "B".to_string()],
        };
        let geometry = annotation_geometry(&Annotation::FeatureControlFrame {
            position: [100.0, 200.0],
            frame,
        })
        .unwrap_or_default();
        assert_eq!(geometry.anchor, [100.0, 200.0]);

        // Simbol posisi (3) + tanda diameter (2) + lingkaran M (1)
        // + bingkai luar (1) + pembatas: setelah simbol dan sebelum tiap datum (3).
        assert_eq!(geometry.paths.len(), 10);
        let texts: Vec<&str> = geometry.texts.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["0.1", "M", "A", "B"]);

        // Bingkai luar: tinggi 7 mm, lebar = pembatas terakhir + kompartemen datum.
        let outer = &geometry.paths[6];
        let (min, max) = bounds(std::slice::from_ref(outer));
        assert_eq!(min, [0.0, 0.0]);
        assert_eq!(max[1], FRAME_HEIGHT_MM);
        assert!(max[0] > 3.0 * FRAME_HEIGHT_MM);
        // Seluruh isi berada di dalam bingkai.
        let (all_min, all_max) = bounds(&geometry.paths);
        assert!(all_min[0] >= 0.0 && all_max[0] <= max[0]);
        assert!(all_min[1] >= 0.0 && all_max[1] <= FRAME_HEIGHT_MM);
    }

    #[test]
    fn gdt_dimension_text_forms() {
        let fit = |class: &str| IsoFit::parse(class).unwrap_or_else(|e| panic!("{e}"));
        let hole = DimensionTolerance::with_fit(25.0, fit("H7"));
        assert_eq!(
            fit_dimension_text(&hole).as_deref(),
            Some("25 H7 (+0.021/0)")
        );
        let shaft = DimensionTolerance::with_fit(25.0, fit("g6"));
        assert_eq!(
            fit_dimension_text(&shaft).as_deref(),
            Some("25 g6 (-0.007/-0.020)")
        );
        // Di luar tabel: batas dihilangkan, kelas tetap tampil.
        let big = DimensionTolerance::with_fit(800.0, fit("H7"));
        assert_eq!(fit_dimension_text(&big).as_deref(), Some("800 H7"));

        // Tak simetris: nominal + dua baris kecil bertumpuk.
        let stacked = annotation_geometry(&Annotation::DimensionTolerance {
            position: [0.0, 0.0],
            dimension: DimensionTolerance::with_limits(10.5, 0.1, 0.2),
        })
        .unwrap_or_default();
        let texts: Vec<&str> = stacked.texts.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["10.5", "+0.1", "-0.2"]);
        assert!(stacked.texts[1].pos[1] > stacked.texts[2].pos[1]);
        assert!(stacked.paths.is_empty());

        // Simetris: tanda plus-minus berupa tiga garis, bukan glyph.
        let mut symmetric = DimensionTolerance::with_limits(10.0, 0.05, 0.05);
        symmetric.diameter = true;
        let geometry = annotation_geometry(&Annotation::DimensionTolerance {
            position: [0.0, 0.0],
            dimension: symmetric,
        })
        .unwrap_or_default();
        let texts: Vec<&str> = geometry.texts.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["10", "0.05"]);
        assert_eq!(geometry.paths.len(), 2 + 3);
        assert!(texts.iter().all(|t| t.is_ascii()));
    }

    #[test]
    fn gdt_tables_datum_and_finish_geometry() {
        use ducad_core::drawing_annot::{HoleTableRow, RevisionRow};

        let holes = HoleTable {
            title: String::new(),
            rows: vec![
                HoleTableRow {
                    tag: "A1".to_string(),
                    x: 10.0,
                    y: 12.5,
                    diameter: 5.5,
                    depth: None,
                    note: "M6".to_string(),
                },
                HoleTableRow {
                    tag: "A2".to_string(),
                    x: 40.0,
                    y: 12.5,
                    diameter: 5.0,
                    depth: Some(12.0),
                    note: String::new(),
                },
            ],
        };
        let g = annotation_geometry(&Annotation::HoleTable {
            position: [10.0, 10.0],
            table: holes,
        })
        .unwrap_or_default();
        let texts: Vec<&str> = g.texts.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts[0], "HOLE TABLE");
        assert!(texts.contains(&"THRU") && texts.contains(&"12.50") && texts.contains(&"5.5"));
        // Bingkai + garis bawah judul + 2 garis baris + 5 pembatas kolom.
        assert_eq!(g.paths.len(), 1 + 1 + 2 + 5);
        let (min, max) = bounds(&g.paths);
        assert_eq!(min, [0.0, 0.0]);
        assert_eq!(max, [130.0, TABLE_TITLE_H + 3.0 * TABLE_ROW_H]);

        let revisions = RevisionTable {
            rows: vec![RevisionRow {
                rev: "B".to_string(),
                description: "Ubah toleransi".to_string(),
                date: "2026-10-05".to_string(),
                by: "YJ".to_string(),
            }],
        };
        let g = annotation_geometry(&Annotation::RevisionTable {
            position: [10.0, 10.0],
            table: revisions,
        })
        .unwrap_or_default();
        assert_eq!(g.paths.len(), 1 + 1 + 1 + 3);
        assert!(g.texts.iter().any(|t| t.text == "Ubah toleransi"));

        let g = annotation_geometry(&Annotation::DatumFeature {
            position: [10.0, 10.0],
            datum: DatumFeature {
                label: "A".to_string(),
            },
        })
        .unwrap_or_default();
        assert_eq!(g.paths.len(), 3);
        assert!(g.paths[0].filled, "segitiga datum harus terisi");
        assert_eq!(g.texts.len(), 1);

        let g = annotation_geometry(&Annotation::SurfaceFinish {
            position: [10.0, 10.0],
            finish: SurfaceFinish { ra_um: 3.2 },
        })
        .unwrap_or_default();
        assert_eq!(g.paths.len(), 1);
        assert_eq!(g.paths[0].cmds.len(), 4);
        assert_eq!(g.texts[0].text, "Ra 3.2");

        // Jangkar tak hingga dilewati.
        assert!(annotation_geometry(&Annotation::SurfaceFinish {
            position: [f64::NAN, 10.0],
            finish: SurfaceFinish { ra_um: 3.2 },
        })
        .is_none());
    }
}
