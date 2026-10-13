//! Halaman dokumen paket desain (W2): sampul, tabel (BOM, keterlacakan,
//! checks, revisi), dan pola bentangan sheet metal.
//!
//! Setiap halaman adalah lembar kosong (bingkai + kepala gambar dari
//! display-list biasa) ditambah satu grup isi. Isi disusun mengalir dari atas
//! ke bawah dan otomatis pindah halaman, jadi tabel panjang tidak menimpa
//! kepala gambar. Tidak ada glyph di luar WinAnsi: PDF memakai font bawaan.

use ducad_core::FlatPattern;
use ducad_kernel::{HlrDrawing, ProjectedView, ProjectedViewKind};

use super::gdt::text_width_mm;
use super::scene::{Anchor, Dash, Group, Item, Pen};
use super::{DimensionPolicy, DrawingSheet, PaperSize, TitleBlockInfo, ViewSet};

const LAYER: &str = "DOCUMENT";
const INK: [u8; 3] = [0, 0, 0];
const PEN_THIN: Pen = Pen {
    color: INK,
    width_mm: 0.25,
    dash: Dash::Solid,
    layer: LAYER,
};
const PEN_THICK: Pen = Pen {
    color: INK,
    width_mm: 0.5,
    dash: Dash::Solid,
    layer: LAYER,
};
const PEN_BEND: Pen = Pen {
    color: [30, 90, 200],
    width_mm: 0.25,
    dash: Dash::Center,
    layer: "BEND",
};

const BODY_MM: f32 = 3.0;
const HEAD_MM: f32 = 5.0;
const SUB_MM: f32 = 3.6;
const ROW_MM: f32 = 6.0;
const CELL_MM: f32 = 2.6;
const GAP_MM: f32 = 4.0;
/// Jarak antar baris teks di dalam satu sel tabel.
const CELL_PITCH_MM: f32 = 3.6;
const MAX_CELL_LINES: usize = 4;

fn empty_view(kind: ProjectedViewKind) -> ProjectedView {
    ProjectedView {
        kind,
        title: String::new(),
        bounds_min: [0.0; 2],
        bounds_max: [0.0; 2],
        segments: Vec::new(),
        centerlines: Vec::new(),
        features: Vec::new(),
        width_mm: 0.0,
        height_mm: 0.0,
        depth_mm: 0.0,
        arcs: Vec::new(),
        edge_refs: Vec::new(),
        exact: false,
    }
}

/// Lembar tanpa tampak: hanya bingkai dan kepala gambar.
pub fn blank_sheet(paper: PaperSize, title_block: TitleBlockInfo) -> DrawingSheet {
    DrawingSheet {
        paper_size: paper,
        title_block,
        drawing: HlrDrawing {
            front: empty_view(ProjectedViewKind::Front),
            top: empty_view(ProjectedViewKind::Top),
            right: empty_view(ProjectedViewKind::Right),
            isometric: empty_view(ProjectedViewKind::Isometric),
            sections: Vec::new(),
            detail_views: Vec::new(),
            model_bbox_min: [0.0; 3],
            model_bbox_max: [0.0; 3],
            warnings: Vec::new(),
        },
        view_placements: Vec::new(),
        scale: 1.0,
        show_hidden_lines: false,
        show_dimensions: false,
        show_centerlines: false,
        show_section_view: false,
        show_detail_views: false,
        show_hatch: false,
        show_bom_table: false,
        show_balloons: false,
        bom_table: Default::default(),
        balloons: Vec::new(),
        auto_dimensions: Vec::new(),
        manual_dimensions: Vec::new(),
        custom_texts: Vec::new(),
        annotations: Vec::new(),
        view_options: ViewSet::default(),
        fixed_scale: Some(1.0),
        notes: Vec::new(),
        shaded: Vec::new(),
        dimension_policy: DimensionPolicy::None,
        stale: false,
    }
}

/// Tabel sederhana: lebar kolom relatif, isi dipangkas agar muat.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Table {
    pub headers: Vec<String>,
    /// Bobot lebar kolom (dinormalkan ke lebar isi halaman).
    pub weights: Vec<f32>,
    pub rows: Vec<Vec<String>>,
}

impl Table {
    pub fn new(headers: &[&str], weights: &[f32]) -> Self {
        Self {
            headers: headers.iter().map(|h| h.to_string()).collect(),
            weights: weights.to_vec(),
            rows: Vec::new(),
        }
    }

    pub fn row<S: Into<String>>(&mut self, cells: impl IntoIterator<Item = S>) {
        self.rows.push(cells.into_iter().map(Into::into).collect());
    }
}

/// Satu blok isi halaman dokumen.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// Judul besar (sekali per halaman pertama).
    Heading(String),
    /// Sub-judul bagian.
    Section(String),
    /// Paragraf; dibungkus otomatis.
    Text(String),
    /// Pasangan label–nilai, satu per baris.
    Fields(Vec<(String, String)>),
    Table(Table),
    /// Jarak vertikal tambahan (mm).
    Space(f32),
}

/// Satu halaman jadi: lembar kosong + grup isi.
#[derive(Debug, Clone)]
pub struct DocPage {
    pub sheet: DrawingSheet,
    pub groups: Vec<Group>,
}

/// Teks yang aman untuk font PDF bawaan: karakter di luar Latin-1 diganti
/// padanan ASCII agar tidak tampil sebagai kotak.
pub fn printable(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\u{2013}' | '\u{2014}' | '\u{2212}' => out.push('-'),
            '\u{2018}' | '\u{2019}' => out.push('\''),
            '\u{201C}' | '\u{201D}' => out.push('"'),
            '\u{2264}' => out.push_str("<="),
            '\u{2265}' => out.push_str(">="),
            '\u{2192}' => out.push_str("->"),
            '\u{2026}' => out.push_str("..."),
            '\t' | '\n' | '\r' => out.push(' '),
            c if (c as u32) < 0x100 => out.push(c),
            _ => out.push('?'),
        }
    }
    out
}

/// Pangkas `text` agar lebarnya ≤ `max_mm`, diakhiri "..." bila terpotong.
pub fn fit_text(text: &str, size_mm: f32, max_mm: f32) -> String {
    let text = printable(text);
    if text_width_mm(&text, size_mm) <= max_mm {
        return text;
    }
    let mut out = String::new();
    for c in text.chars() {
        let mut next = out.clone();
        next.push(c);
        if text_width_mm(&format!("{next}..."), size_mm) > max_mm {
            break;
        }
        out = next;
    }
    format!("{}...", out.trim_end())
}

/// Bungkus `text` menjadi baris-baris selebar ≤ `max_mm`.
pub fn wrap_text(text: &str, size_mm: f32, max_mm: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in printable(text).split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if text_width_mm(&candidate, size_mm) <= max_mm || line.is_empty() {
            line = candidate;
        } else {
            lines.push(std::mem::take(&mut line));
            line = word.to_string();
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

fn text(items: &mut Vec<Item>, pos: [f32; 2], text: &str, size_mm: f32, bold: bool) {
    if text.trim().is_empty() {
        return;
    }
    items.push(Item::Text {
        pos,
        text: text.to_string(),
        size_mm,
        bold,
        anchor: Anchor::Start,
        angle_deg: 0.0,
        layer: LAYER,
    });
}

/// Daerah isi lembar: di dalam bingkai, di atas kepala gambar.
/// `[x0, y_bawah, x1, y_atas]`.
pub fn content_rect(sheet: &DrawingSheet) -> [f32; 4] {
    let (_, inner) = sheet.border_rects_mm();
    let tb = sheet.title_block_rect_mm();
    [inner[0] + 8.0, tb[3] + 6.0, inner[2] - 8.0, inner[3] - 8.0]
}

struct Flow {
    paper: PaperSize,
    title_block: TitleBlockInfo,
    pages: Vec<DocPage>,
    items: Vec<Item>,
    rect: [f32; 4],
    y: f32,
}

impl Flow {
    fn new(paper: PaperSize, title_block: TitleBlockInfo) -> Self {
        let rect = content_rect(&blank_sheet(paper, title_block.clone()));
        Self {
            paper,
            title_block,
            pages: Vec::new(),
            items: Vec::new(),
            rect,
            y: rect[3],
        }
    }

    fn width(&self) -> f32 {
        self.rect[2] - self.rect[0]
    }

    fn flush(&mut self) {
        let items = std::mem::take(&mut self.items);
        self.pages.push(DocPage {
            sheet: blank_sheet(self.paper, self.title_block.clone()),
            groups: vec![Group {
                id: "document".to_string(),
                items,
            }],
        });
        self.y = self.rect[3];
    }

    /// Pastikan masih ada ruang setinggi `h`; bila tidak, pindah halaman.
    fn need(&mut self, h: f32) {
        if self.y - h < self.rect[1] && !self.items.is_empty() {
            self.flush();
        }
    }

    fn line(&mut self, s: &str, size_mm: f32, bold: bool) {
        let pitch = size_mm * 1.6;
        self.need(pitch);
        self.y -= pitch;
        text(&mut self.items, [self.rect[0], self.y], s, size_mm, bold);
    }

    fn table(&mut self, table: &Table) {
        let total: f32 = table.weights.iter().sum::<f32>().max(1e-6);
        let widths: Vec<f32> = table
            .weights
            .iter()
            .map(|w| w / total * self.width())
            .collect();
        let x0 = self.rect[0];
        let header = |flow: &mut Flow| {
            flow.need(ROW_MM * 2.0);
            let top = flow.y;
            flow.y -= ROW_MM;
            flow.items.push(Item::Rect {
                min: [x0, flow.y],
                max: [x0 + flow.width(), top],
                pen: Some(PEN_THICK),
                fill: Some([232, 236, 242]),
            });
            let mut x = x0;
            for (h, w) in table.headers.iter().zip(&widths) {
                text(
                    &mut flow.items,
                    [x + 1.5, flow.y + 1.8],
                    &fit_text(h, CELL_MM, w - 3.0),
                    CELL_MM,
                    true,
                );
                x += w;
            }
        };
        header(self);
        for row in &table.rows {
            // Sel dibungkus (maks. MAX_CELL_LINES baris); tinggi baris
            // mengikuti sel tertinggi supaya sitasi panjang tetap terbaca.
            let cells: Vec<Vec<String>> = widths
                .iter()
                .enumerate()
                .map(|(i, w)| {
                    let mut lines = wrap_text(
                        row.get(i).map(String::as_str).unwrap_or(""),
                        CELL_MM,
                        w - 3.0,
                    );
                    if lines.len() > MAX_CELL_LINES {
                        lines.truncate(MAX_CELL_LINES);
                        if let Some(last) = lines.last_mut() {
                            *last = fit_text(&format!("{last} ..."), CELL_MM, w - 3.0);
                        }
                    }
                    lines
                        .into_iter()
                        .map(|l| fit_text(&l, CELL_MM, w - 3.0))
                        .collect()
                })
                .collect();
            let lines = cells.iter().map(Vec::len).max().unwrap_or(1).max(1);
            let height = ROW_MM + (lines as f32 - 1.0) * CELL_PITCH_MM;
            if self.y - height < self.rect[1] {
                self.flush();
                header(self);
            }
            let top = self.y;
            self.y -= height;
            self.items.push(Item::Rect {
                min: [x0, self.y],
                max: [x0 + self.width(), top],
                pen: Some(PEN_THIN),
                fill: None,
            });
            let mut x = x0;
            for (i, w) in widths.iter().enumerate() {
                if i > 0 {
                    self.items.push(Item::Line {
                        a: [x, self.y],
                        b: [x, top],
                        pen: PEN_THIN,
                    });
                }
                for (k, line) in cells[i].iter().enumerate() {
                    text(
                        &mut self.items,
                        [x + 1.5, top - ROW_MM + 1.8 - k as f32 * CELL_PITCH_MM],
                        line,
                        CELL_MM,
                        false,
                    );
                }
                x += w;
            }
        }
        self.y -= GAP_MM;
    }

    fn block(&mut self, block: &Block) {
        match block {
            Block::Heading(s) => {
                self.line(&fit_text(s, HEAD_MM, self.width()), HEAD_MM, true);
                self.y -= 2.0;
            }
            Block::Section(s) => {
                // Sub-judul tidak boleh yatim di dasar halaman.
                self.need(SUB_MM * 1.6 + ROW_MM * 2.0);
                self.y -= 2.0;
                self.line(&fit_text(s, SUB_MM, self.width()), SUB_MM, true);
                let y = self.y - 1.2;
                self.items.push(Item::Line {
                    a: [self.rect[0], y],
                    b: [self.rect[2], y],
                    pen: PEN_THIN,
                });
                self.y -= 2.4;
            }
            Block::Text(s) => {
                for l in wrap_text(s, BODY_MM, self.width()) {
                    self.line(&l, BODY_MM, false);
                }
                self.y -= 1.5;
            }
            Block::Fields(fields) => {
                let label_w = fields
                    .iter()
                    .map(|(k, _)| text_width_mm(&printable(k), BODY_MM))
                    .fold(0.0f32, f32::max)
                    + 6.0;
                for (k, v) in fields {
                    let lines = wrap_text(v, BODY_MM, (self.width() - label_w).max(20.0));
                    let lines = if lines.is_empty() {
                        vec!["-".to_string()]
                    } else {
                        lines
                    };
                    for (i, l) in lines.iter().enumerate() {
                        let pitch = BODY_MM * 1.6;
                        self.need(pitch);
                        self.y -= pitch;
                        if i == 0 {
                            text(
                                &mut self.items,
                                [self.rect[0], self.y],
                                &printable(k),
                                BODY_MM,
                                true,
                            );
                        }
                        text(
                            &mut self.items,
                            [self.rect[0] + label_w, self.y],
                            l,
                            BODY_MM,
                            false,
                        );
                    }
                }
                self.y -= 1.5;
            }
            Block::Table(t) => self.table(t),
            Block::Space(mm) => self.y -= mm,
        }
    }
}

/// Susun `blocks` menjadi satu atau lebih halaman dokumen.
pub fn document_pages(
    paper: PaperSize,
    title_block: &TitleBlockInfo,
    blocks: &[Block],
) -> Vec<DocPage> {
    let mut flow = Flow::new(paper, title_block.clone());
    for b in blocks {
        flow.block(b);
    }
    if !flow.items.is_empty() || flow.pages.is_empty() {
        flow.flush();
    }
    flow.pages
}

/// Halaman pola bentangan: garis luar tebal, garis tekuk putus-titik biru
/// berlabel sudut/radius, ukuran bentangan, dan skala terbesar yang muat.
pub fn flat_pattern_page(
    paper: PaperSize,
    title_block: &TitleBlockInfo,
    heading: &str,
    flat: &FlatPattern,
    thickness_mm: f64,
) -> DocPage {
    let mut sheet = blank_sheet(paper, title_block.clone());
    let rect = content_rect(&sheet);
    let mut items: Vec<Item> = Vec::new();
    text(
        &mut items,
        [rect[0], rect[3] - HEAD_MM * 1.6],
        &fit_text(heading, HEAD_MM, rect[2] - rect[0]),
        HEAD_MM,
        true,
    );

    let points = flat
        .outline
        .iter()
        .flat_map(|(a, b)| [*a, *b])
        .chain(flat.bend_lines.iter().flat_map(|l| [l.a, l.b]));
    let (mut min, mut max) = ([f64::MAX; 2], [f64::MIN; 2]);
    for p in points {
        for k in 0..2 {
            min[k] = min[k].min(p[k]);
            max[k] = max[k].max(p[k]);
        }
    }
    if min[0] > max[0] {
        return DocPage {
            sheet,
            groups: vec![Group {
                id: "flat_pattern".to_string(),
                items,
            }],
        };
    }
    let (w, h) = ((max[0] - min[0]) as f32, (max[1] - min[1]) as f32);
    // Daerah gambar: di bawah judul dan baris keterangan.
    let area = [
        rect[0],
        rect[1] + 4.0,
        rect[2],
        rect[3] - HEAD_MM * 1.6 - 14.0,
    ];
    let (aw, ah) = (area[2] - area[0], area[3] - area[1]);
    let scale = [5.0f32, 2.0, 1.0, 0.5, 0.4, 0.25, 0.2, 0.1, 0.05, 0.02, 0.01]
        .into_iter()
        .find(|s| w * s <= aw && h * s <= ah)
        .unwrap_or(0.01);
    let origin = [
        area[0] + (aw - w * scale) * 0.5,
        area[1] + (ah - h * scale) * 0.5,
    ];
    let map = |p: [f64; 2]| {
        [
            origin[0] + (p[0] - min[0]) as f32 * scale,
            origin[1] + (p[1] - min[1]) as f32 * scale,
        ]
    };
    for (a, b) in &flat.outline {
        items.push(Item::Line {
            a: map(*a),
            b: map(*b),
            pen: PEN_THICK,
        });
    }
    for bend in &flat.bend_lines {
        let (a, b) = (map(bend.a), map(bend.b));
        items.push(Item::Line {
            a,
            b,
            pen: PEN_BEND,
        });
        let label = format!(
            "{} {:.0} deg R{}",
            if bend.up { "UP" } else { "DOWN" },
            bend.angle_deg,
            trim_num(bend.radius)
        );
        items.push(Item::Text {
            pos: [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5 + 1.0],
            text: label,
            size_mm: 2.4,
            bold: false,
            anchor: Anchor::Middle,
            angle_deg: 0.0,
            layer: "BEND",
        });
    }
    let caption = format!(
        "FLAT PATTERN {} x {} mm, t = {} mm, {} bend(s)",
        trim_num(w as f64),
        trim_num(h as f64),
        trim_num(thickness_mm),
        flat.bend_lines.len()
    );
    text(
        &mut items,
        [rect[0], rect[3] - HEAD_MM * 1.6 - 7.0],
        &caption,
        BODY_MM,
        false,
    );
    sheet.scale = scale;
    sheet.title_block.scale = super::format_scale_ratio(scale);
    DocPage {
        sheet,
        groups: vec![Group {
            id: "flat_pattern".to_string(),
            items,
        }],
    }
}

fn trim_num(v: f64) -> String {
    let t = format!("{v:.2}");
    t.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::{generate_pdf_document, PdfPage};

    fn title() -> TitleBlockInfo {
        TitleBlockInfo {
            project_title: "PAKET UJI".into(),
            ..TitleBlockInfo::default()
        }
    }

    #[test]
    fn long_table_flows_onto_more_pages_and_repeats_its_header() {
        let mut t = Table::new(&["NO", "NAMA", "CATATAN"], &[1.0, 3.0, 6.0]);
        for i in 0..120 {
            t.row([i.to_string(), format!("part {i}"), "x".repeat(400)]);
        }
        let pages = document_pages(
            PaperSize::A4Landscape,
            &title(),
            &[Block::Heading("BOM".into()), Block::Table(t)],
        );
        assert!(pages.len() >= 4, "{}", pages.len());
        let rect = content_rect(&pages[0].sheet);
        for page in &pages {
            let items = &page.groups[0].items;
            // Tiap halaman mengulang kepala tabel.
            assert!(items
                .iter()
                .any(|i| matches!(i, Item::Text { text, bold: true, .. } if text == "NAMA")));
            for item in items {
                if let Item::Text {
                    pos, text, size_mm, ..
                } = item
                {
                    assert!(
                        pos[1] >= rect[1] - 0.01,
                        "teks menimpa kepala gambar: {pos:?}"
                    );
                    assert!(
                        pos[0] + text_width_mm(text, *size_mm) <= rect[2] + 0.5,
                        "{text}"
                    );
                }
            }
        }
    }

    #[test]
    fn wrap_and_fit_respect_the_width() {
        let lines = wrap_text(&"kata ".repeat(60), 3.0, 80.0);
        assert!(lines.len() > 2);
        assert!(lines.iter().all(|l| text_width_mm(l, 3.0) <= 80.0));
        let cut = fit_text(&"W".repeat(100), 3.0, 40.0);
        assert!(cut.ends_with("...") && text_width_mm(&cut, 3.0) <= 40.0);
        assert_eq!(
            printable("a \u{2192} b \u{2265} 2 \u{4e2d}"),
            "a -> b >= 2 ?"
        );
    }

    #[test]
    fn document_is_a_multi_page_pdf_with_bookmarks_and_is_deterministic() {
        let pages = document_pages(
            PaperSize::A4Landscape,
            &title(),
            &[
                Block::Heading("SAMPUL".into()),
                Block::Fields(vec![("Part".into(), "Bracket".into())]),
                Block::Text("Paragraf (uji) \\ tanda kurung.".into()),
            ],
        );
        let more = document_pages(
            PaperSize::A3Landscape,
            &title(),
            &[Block::Section("B".into())],
        );
        let build = || {
            let list: Vec<PdfPage<'_>> = pages
                .iter()
                .chain(&more)
                .enumerate()
                .map(|(i, p)| PdfPage {
                    sheet: &p.sheet,
                    extra: &p.groups,
                    bookmark: if i == 0 { "Cover (1)" } else { "Part" },
                })
                .collect();
            generate_pdf_document(&list)
        };
        let bytes = build();
        assert_eq!(bytes, build(), "harus deterministik");
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("/Count 2 >>"), "dua halaman");
        assert_eq!(text.matches("/Type /Page ").count(), 2);
        assert!(text.contains("/Type /Outlines"));
        assert!(text.contains("/Title (Cover \\(1\\))"));
        // Dua ukuran kertas berbeda dalam satu dokumen.
        assert!(text.contains("/MediaBox [0 0 841.89 595.28]"));
        assert!(text.contains("/MediaBox [0 0 1190.55 841.89]"));
        // Tabel xref menunjuk awal tiap objek (diperiksa pada byte mentah:
        // kepala PDF memuat byte non-UTF-8).
        let find = |needle: &[u8]| {
            bytes
                .windows(needle.len())
                .rposition(|w| w == needle)
                .unwrap()
        };
        let tail = String::from_utf8_lossy(&bytes[find(b"startxref") + 9..]).to_string();
        let offset: usize = tail.split_whitespace().next().unwrap().parse().unwrap();
        assert!(bytes[offset..].starts_with(b"xref"));
        let table = String::from_utf8_lossy(&bytes[offset..]).to_string();
        let mut objects = 0;
        for (n, line) in table
            .lines()
            .skip(3)
            .take_while(|l| l.ends_with("n "))
            .enumerate()
        {
            let at: usize = line[..10].parse().unwrap();
            assert!(
                bytes[at..].starts_with(format!("{} 0 obj", n + 1).as_bytes()),
                "objek {}",
                n + 1
            );
            objects += 1;
        }
        assert!(objects >= 9, "{objects}");
    }

    #[test]
    fn flat_pattern_page_draws_outline_and_bends() {
        let flat = FlatPattern {
            polygons: vec![],
            outline: vec![([0.0, 0.0], [100.0, 0.0]), ([100.0, 0.0], [100.0, 50.0])],
            bend_lines: vec![ducad_core::sheet_metal::BendLine {
                a: [0.0, 20.0],
                b: [100.0, 20.0],
                up: true,
                angle_deg: 90.0,
                radius: 2.0,
                flange: "f1".into(),
            }],
        };
        let page = flat_pattern_page(PaperSize::A4Landscape, &title(), "FLAT - tray", &flat, 2.0);
        let items = &page.groups[0].items;
        assert_eq!(
            items
                .iter()
                .filter(|i| matches!(i, Item::Line { .. }))
                .count(),
            3
        );
        assert!(items
            .iter()
            .any(|i| matches!(i, Item::Text { text, .. } if text == "UP 90 deg R2")));
        assert!(items.iter().any(
            |i| matches!(i, Item::Text { text, .. } if text.starts_with("FLAT PATTERN 100 x 50 mm"))
        ));
        assert_eq!(page.sheet.title_block.scale, "2:1");
    }
}
