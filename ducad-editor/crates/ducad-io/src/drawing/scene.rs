//! Display-list lembar gambar (P21.0): SATU penggambar untuk PDF, SVG, DXF.
//!
//! Sebelum P21 tiap eksportir menggambar lembar sendiri-sendiri, sehingga
//! SVG tercermin sumbu-Y terhadap PDF dan skala per tampak tidak konsisten.
//! [`build_scene`] sekarang menghasilkan primitif dalam mm kertas dengan
//! **Y ke atas** (asal = pojok kiri-bawah); eksportir hanya menerjemahkannya.
//! Anotasi GD&T (P19) tetap digambar eksportir dari `drawing::gdt`.

use ducad_kernel::{HlrArc2D, HlrLineKind, ProjectedViewKind};

use super::auto_dim::{ViewXf, DIM_TEXT_MM};
use super::gdt::text_width_mm;
use super::{view_key, DimStyle, DimensionAnnotation, DrawingSheet, PaperSize, NOTE_TEXT_MM};

/// Pola putus garis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dash {
    Solid,
    Hidden,
    Center,
}

impl Dash {
    /// Pola (mm): panjang garis/celah bergantian. Kosong = menerus.
    pub fn pattern_mm(self) -> &'static [f32] {
        match self {
            Dash::Solid => &[],
            Dash::Hidden => &[2.0, 1.0],
            Dash::Center => &[6.0, 1.2, 0.8, 1.2],
        }
    }
}

/// Gaya garis + layer DXF.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pen {
    pub color: [u8; 3],
    pub width_mm: f32,
    pub dash: Dash,
    pub layer: &'static str,
}

const BLACK: [u8; 3] = [0, 0, 0];

const fn pen(width_mm: f32, dash: Dash, layer: &'static str) -> Pen {
    Pen {
        color: BLACK,
        width_mm,
        dash,
        layer,
    }
}

pub const PEN_VISIBLE: Pen = pen(0.5, Dash::Solid, "VISIBLE");
pub const PEN_HIDDEN: Pen = Pen {
    color: [70, 70, 78],
    width_mm: 0.25,
    dash: Dash::Hidden,
    layer: "HIDDEN",
};
pub const PEN_CENTER: Pen = pen(0.18, Dash::Center, "CENTERLINE");
pub const PEN_HATCH: Pen = pen(0.18, Dash::Solid, "HATCH");
pub const PEN_DIM: Pen = pen(0.18, Dash::Solid, "DIMENSIONS");
pub const PEN_BORDER: Pen = pen(0.7, Dash::Solid, "BORDER");
pub const PEN_BORDER_THIN: Pen = pen(0.25, Dash::Solid, "BORDER");
pub const PEN_TITLE: Pen = pen(0.5, Dash::Solid, "TITLEBLOCK");
pub const PEN_TITLE_THIN: Pen = pen(0.25, Dash::Solid, "TITLEBLOCK");
pub const PEN_CUT: Pen = pen(0.25, Dash::Center, "SECTION");
pub const PEN_CUT_THICK: Pen = pen(0.9, Dash::Solid, "SECTION");
pub const PEN_DETAIL: Pen = pen(0.25, Dash::Hidden, "SECTION");
pub const PEN_BOM: Pen = pen(0.25, Dash::Solid, "BOM_TABLE");
pub const PEN_BOM_THICK: Pen = pen(0.5, Dash::Solid, "BOM_TABLE");
pub const PEN_BALLOON: Pen = pen(0.35, Dash::Solid, "CALLOUT_BALLOONS");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    Start,
    Middle,
    End,
}

/// Primitif gambar dalam mm kertas, Y ke atas.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Line {
        a: [f32; 2],
        b: [f32; 2],
        pen: Pen,
    },
    /// Busur berlawanan jarum jam dari `start_deg` ke `end_deg`.
    Arc {
        center: [f32; 2],
        radius: f32,
        start_deg: f32,
        end_deg: f32,
        pen: Pen,
    },
    Circle {
        center: [f32; 2],
        radius: f32,
        pen: Pen,
        fill: Option<[u8; 3]>,
    },
    Rect {
        min: [f32; 2],
        max: [f32; 2],
        pen: Option<Pen>,
        fill: Option<[u8; 3]>,
    },
    /// Poligon terisi (kepala panah).
    Fill {
        points: Vec<[f32; 2]>,
        color: [u8; 3],
        layer: &'static str,
    },
    /// Teks pada garis dasar `pos`, diputar `angle_deg` berlawanan jarum jam.
    Text {
        pos: [f32; 2],
        text: String,
        size_mm: f32,
        bold: bool,
        anchor: Anchor,
        angle_deg: f32,
        layer: &'static str,
    },
    /// Gambar raster `sheet.shaded[index]` mengisi kotak `min`–`max`.
    Image {
        min: [f32; 2],
        max: [f32; 2],
        index: usize,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub id: String,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub width_mm: f32,
    pub height_mm: f32,
    pub groups: Vec<Group>,
}

impl Scene {
    pub fn group(&self, id: &str) -> Option<&Group> {
        self.groups.iter().find(|g| g.id == id)
    }
}

struct G {
    items: Vec<Item>,
}

impl G {
    fn new() -> Self {
        Self { items: Vec::new() }
    }
    fn line(&mut self, a: [f32; 2], b: [f32; 2], pen: Pen) {
        self.items.push(Item::Line { a, b, pen });
    }
    fn rect(&mut self, r: [f32; 4], pen: Option<Pen>, fill: Option<[u8; 3]>) {
        self.items.push(Item::Rect {
            min: [r[0], r[1]],
            max: [r[2], r[3]],
            pen,
            fill,
        });
    }
    fn text(
        &mut self,
        pos: [f32; 2],
        text: &str,
        size_mm: f32,
        bold: bool,
        anchor: Anchor,
        layer: &'static str,
    ) {
        if text.trim().is_empty() {
            return;
        }
        self.items.push(Item::Text {
            pos,
            text: text.to_string(),
            size_mm,
            bold,
            anchor,
            angle_deg: 0.0,
            layer,
        });
    }
    /// Kepala panah terisi: ujung di `tip`, badan memanjang searah `dir`.
    fn arrow(&mut self, tip: [f32; 2], dir: [f32; 2], layer: &'static str) {
        let len = dir[0].hypot(dir[1]);
        if len < 1e-6 {
            return;
        }
        let d = [dir[0] / len, dir[1] / len];
        let n = [-d[1], d[0]];
        let base = [tip[0] + d[0] * 2.5, tip[1] + d[1] * 2.5];
        self.items.push(Item::Fill {
            points: vec![
                tip,
                [base[0] + n[0] * 0.6, base[1] + n[1] * 0.6],
                [base[0] - n[0] * 0.6, base[1] - n[1] * 0.6],
            ],
            color: BLACK,
            layer,
        });
    }
    fn done(self, id: &str) -> Group {
        Group {
            id: id.to_string(),
            items: self.items,
        }
    }
}

/// Jumlah zona (kolom, baris) bingkai untuk tiap ukuran kertas.
pub fn zone_grid(paper: PaperSize) -> (usize, usize) {
    match paper {
        PaperSize::A3Landscape => (8, 6),
        PaperSize::A3Portrait => (6, 8),
        PaperSize::A4Landscape => (6, 4),
        PaperSize::A4Portrait => (4, 6),
    }
}

/// Bangun display-list seluruh lembar.
pub fn build_scene(sheet: &DrawingSheet) -> Scene {
    let (pw, ph) = sheet.paper_size.dimensions_mm();
    let mut groups = vec![border(sheet), title_block(sheet)];

    for plc in &sheet.view_placements {
        if !plc.visible {
            continue;
        }
        if let Some(xf) = sheet.view_xf(plc.kind) {
            groups.push(view_group(sheet, &xf));
        }
    }

    if sheet.shaded.iter().any(|s| s.visible) {
        let mut g = G::new();
        for (index, s) in sheet.shaded.iter().enumerate() {
            if !s.visible {
                continue;
            }
            let r = s.rect_mm();
            if s.image.is_empty() {
                g.rect(r, Some(PEN_DIM), None);
            } else {
                g.items.push(Item::Image {
                    min: [r[0], r[1]],
                    max: [r[2], r[3]],
                    index,
                });
            }
        }
        groups.push(g.done("shaded_views"));
    }

    let mut g = G::new();
    let dims: Vec<&DimensionAnnotation> = if sheet.show_dimensions {
        sheet
            .auto_dimensions
            .iter()
            .chain(sheet.manual_dimensions.iter())
            .collect()
    } else {
        sheet.manual_dimensions.iter().collect()
    };
    for dim in dims {
        dimension(&mut g, dim);
    }
    groups.push(g.done("dimensions_layer"));

    if !sheet.notes.is_empty() {
        let mut g = G::new();
        let lines = sheet.note_baselines_mm();
        g.text(
            lines[0],
            "NOTE:",
            NOTE_TEXT_MM,
            true,
            Anchor::Start,
            "TITLEBLOCK",
        );
        for (i, note) in sheet.notes.iter().enumerate() {
            let numbered = note
                .trim_start()
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_digit());
            let text = if numbered {
                note.clone()
            } else {
                format!("{}. {note}", i + 1)
            };
            g.text(
                lines[i + 1],
                &text,
                NOTE_TEXT_MM,
                false,
                Anchor::Start,
                "TITLEBLOCK",
            );
        }
        groups.push(g.done("notes"));
    }

    if sheet.show_bom_table && !sheet.bom_table.items.is_empty() {
        groups.push(bom_table(sheet));
    }
    if sheet.show_balloons && !sheet.balloons.is_empty() {
        groups.push(balloons(sheet));
    }
    if sheet.custom_texts.iter().any(|t| !t.text.trim().is_empty()) {
        let mut g = G::new();
        for note in &sheet.custom_texts {
            g.text(
                note.position,
                &note.text,
                note.font_size,
                true,
                Anchor::Start,
                "TITLEBLOCK",
            );
        }
        groups.push(g.done("custom_texts"));
    }

    Scene {
        width_mm: pw,
        height_mm: ph,
        groups,
    }
}

fn border(sheet: &DrawingSheet) -> Group {
    let (outer, inner) = sheet.border_rects_mm();
    let mut g = G::new();
    g.rect(outer, Some(PEN_BORDER_THIN), None);
    g.rect(inner, Some(PEN_BORDER), None);

    let (cols, rows) = zone_grid(sheet.paper_size);
    let col_w = (inner[2] - inner[0]) / cols as f32;
    let row_h = (inner[3] - inner[1]) / rows as f32;
    for c in 0..cols {
        let x = inner[0] + c as f32 * col_w;
        if c > 0 {
            g.line([x, inner[3]], [x, inner[3] + 3.0], PEN_BORDER_THIN);
            g.line([x, inner[1]], [x, inner[1] - 3.0], PEN_BORDER_THIN);
        }
        // Nomor menurun dari kiri ke kanan (zona 1 di sisi kepala gambar).
        let label = (cols - c).to_string();
        let cx = x + col_w * 0.5;
        g.text(
            [cx, inner[3] + 3.5],
            &label,
            2.5,
            false,
            Anchor::Middle,
            "BORDER",
        );
        g.text(
            [cx, inner[1] - 6.0],
            &label,
            2.5,
            false,
            Anchor::Middle,
            "BORDER",
        );
    }
    for r in 0..rows {
        let y = inner[1] + r as f32 * row_h;
        if r > 0 {
            g.line([inner[0], y], [inner[0] - 3.0, y], PEN_BORDER_THIN);
            g.line([inner[2], y], [inner[2] + 3.0, y], PEN_BORDER_THIN);
        }
        let label = ((b'A' + r as u8) as char).to_string();
        let cy = y + row_h * 0.5 - 1.0;
        g.text(
            [inner[0] - 6.0, cy],
            &label,
            2.5,
            false,
            Anchor::Middle,
            "BORDER",
        );
        g.text(
            [inner[2] + 5.0, cy],
            &label,
            2.5,
            false,
            Anchor::Middle,
            "BORDER",
        );
    }
    g.done("sheet_border")
}

fn title_block(sheet: &DrawingSheet) -> Group {
    let tb = sheet.title_block_rect_mm();
    let info = &sheet.title_block;
    let (x0, y0, x1, y1) = (tb[0], tb[1], tb[2], tb[3]);
    let mut g = G::new();
    g.rect(tb, Some(PEN_TITLE), Some([255, 255, 255]));
    g.line([x0, y0 + 9.0], [x1, y0 + 9.0], PEN_TITLE_THIN);
    g.line([x0, y0 + 18.0], [x1, y0 + 18.0], PEN_TITLE);
    g.line([x0, y0 + 32.0], [x1, y0 + 32.0], PEN_TITLE_THIN);
    g.line([x0 + 95.0, y0 + 32.0], [x0 + 95.0, y1], PEN_TITLE_THIN);
    g.line([x0 + 85.0, y0 + 18.0], [x0 + 85.0, y0 + 32.0], PEN_TITLE);
    g.line(
        [x0 + 45.0, y0 + 9.0],
        [x0 + 45.0, y0 + 18.0],
        PEN_TITLE_THIN,
    );
    g.line([x0 + 90.0, y0], [x0 + 90.0, y0 + 18.0], PEN_TITLE_THIN);
    g.line(
        [x0 + 115.0, y0 + 9.0],
        [x0 + 115.0, y0 + 18.0],
        PEN_TITLE_THIN,
    );

    let l = "TITLEBLOCK";
    let label = 1.9;
    let tx = x0 + 3.0;
    g.text(
        [tx, y0 + 39.0],
        &info.company_name,
        3.3,
        true,
        Anchor::Start,
        l,
    );
    g.text(
        [tx, y0 + 34.5],
        "LEMBAR KERJA GAMBAR TEKNIK - ISO 5457",
        label,
        false,
        Anchor::Start,
        l,
    );
    g.text(
        [tx, y0 + 28.0],
        "JUDUL GAMBAR / PART TITLE:",
        label,
        false,
        Anchor::Start,
        l,
    );
    let title = if info.project_title.is_empty() {
        "KOMPONEN UTAMA"
    } else {
        &info.project_title
    };
    g.text([tx, y0 + 21.5], title, 3.9, true, Anchor::Start, l);

    let dx = x0 + 88.0;
    g.text(
        [dx, y0 + 28.0],
        "NO. GAMBAR / DWG NO:",
        label,
        false,
        Anchor::Start,
        l,
    );
    g.text(
        [dx, y0 + 21.5],
        &info.drawing_number,
        3.2,
        true,
        Anchor::Start,
        l,
    );
    g.text([x1 - 2.0, y0 + 28.0], "REV", label, false, Anchor::End, l);
    g.text(
        [x1 - 2.0, y0 + 21.5],
        &info.revision,
        3.2,
        true,
        Anchor::End,
        l,
    );

    g.text([tx, y0 + 15.0], "DIGAMBAR:", label, false, Anchor::Start, l);
    g.text(
        [tx, y0 + 11.0],
        &info.drawn_by,
        2.4,
        false,
        Anchor::Start,
        l,
    );
    g.text(
        [x0 + 48.0, y0 + 15.0],
        "TANGGAL:",
        label,
        false,
        Anchor::Start,
        l,
    );
    g.text(
        [x0 + 48.0, y0 + 11.0],
        &info.date,
        2.4,
        false,
        Anchor::Start,
        l,
    );
    g.text(
        [x0 + 93.0, y0 + 15.0],
        "SKALA / SCALE:",
        label,
        false,
        Anchor::Start,
        l,
    );
    g.text(
        [x0 + 93.0, y0 + 11.0],
        &info.scale,
        2.8,
        true,
        Anchor::Start,
        l,
    );
    g.text(
        [x0 + 118.0, y0 + 15.0],
        "LEMBAR:",
        label,
        false,
        Anchor::Start,
        l,
    );
    g.text(
        [x0 + 118.0, y0 + 11.0],
        &info.sheet_number,
        2.4,
        false,
        Anchor::Start,
        l,
    );
    g.text([tx, y0 + 6.0], "MATERIAL:", label, false, Anchor::Start, l);
    g.text([tx, y0 + 2.2], &info.material, 2.8, true, Anchor::Start, l);
    g.text(
        [x0 + 93.0, y0 + 6.0],
        "TOLERANSI & SATUAN:",
        label,
        false,
        Anchor::Start,
        l,
    );
    g.text(
        [x0 + 93.0, y0 + 2.2],
        &format!("ISO 2768-m | {}", info.units),
        2.4,
        false,
        Anchor::Start,
        l,
    );

    // Simbol proyeksi sudut ketiga: kerucut terpancung + dua lingkaran.
    let (cx, cy) = (x0 + 117.0, y0 + 38.5);
    let xc = cx - 9.0;
    let cone = [
        [xc, cy - 2.0],
        [xc + 7.0, cy - 4.0],
        [xc + 7.0, cy + 4.0],
        [xc, cy + 2.0],
    ];
    for i in 0..4 {
        g.line(cone[i], cone[(i + 1) % 4], PEN_TITLE_THIN);
    }
    for r in [2.0, 4.0] {
        g.items.push(Item::Circle {
            center: [cx + 6.0, cy],
            radius: r,
            pen: PEN_TITLE_THIN,
            fill: None,
        });
    }
    g.line([xc - 4.0, cy], [cx + 13.0, cy], PEN_CENTER);
    g.done("title_block")
}

fn push_arc(g: &mut G, xf: &ViewXf<'_>, arc: &HlrArc2D, pen: Pen) {
    let center = xf.to_paper(arc.center);
    let radius = arc.radius * xf.scale;
    if arc.is_full() {
        g.items.push(Item::Circle {
            center,
            radius,
            pen,
            fill: None,
        });
    } else {
        g.items.push(Item::Arc {
            center,
            radius,
            start_deg: arc.start_deg,
            end_deg: arc.end_deg,
            pen,
        });
    }
}

fn view_group(sheet: &DrawingSheet, xf: &ViewXf<'_>) -> Group {
    let view = xf.view;
    let mut g = G::new();
    let bbox = xf.bbox();

    // Judul di bawah tampak (diturunkan bila ada garis potong keluar bawah).
    let placement = sheet.view_placements.iter().find(|p| p.kind == xf.kind);
    if let Some(plc) = placement {
        let base = bbox[1] - sheet.view_title_drop_mm(plc);
        g.text(
            [xf.center[0], base - 6.5],
            &sheet.view_title(plc),
            3.0,
            true,
            Anchor::Middle,
            "TITLEBLOCK",
        );
        if let Some(scale) = sheet.view_scale_label(plc) {
            g.text(
                [xf.center[0], base - 10.5],
                &scale,
                2.5,
                false,
                Anchor::Middle,
                "TITLEBLOCK",
            );
        }
    }

    if let ProjectedViewKind::Detail(_) = xf.kind {
        g.items.push(Item::Circle {
            center: xf.center,
            radius: view.size_2d()[0] * 0.5 * xf.scale,
            pen: PEN_VISIBLE,
            fill: None,
        });
    }

    let seg = |g: &mut G, s: &ducad_kernel::HlrSegment2D, pen: Pen| {
        g.line(xf.to_paper(s.start), xf.to_paper(s.end), pen);
    };
    // Urutan: sumbu, tersembunyi, arsir, lalu garis tampak di atasnya.
    if sheet.show_centerlines {
        for cl in &view.centerlines {
            seg(&mut g, cl, PEN_CENTER);
        }
        for s in view
            .segments
            .iter()
            .filter(|s| s.kind == HlrLineKind::Centerline)
        {
            seg(&mut g, s, PEN_CENTER);
        }
    }
    if sheet.show_hidden_lines {
        for s in view
            .segments
            .iter()
            .filter(|s| s.kind == HlrLineKind::Hidden)
        {
            seg(&mut g, s, PEN_HIDDEN);
        }
        for a in view.arcs.iter().filter(|a| a.kind == HlrLineKind::Hidden) {
            push_arc(&mut g, xf, a, PEN_HIDDEN);
        }
    }
    if sheet.show_hatch {
        for s in view
            .segments
            .iter()
            .filter(|s| s.kind == HlrLineKind::Hatch)
        {
            seg(&mut g, s, PEN_HATCH);
        }
    }
    for s in &view.segments {
        match s.kind {
            HlrLineKind::Visible | HlrLineKind::Silhouette => seg(&mut g, s, PEN_VISIBLE),
            HlrLineKind::CuttingPlane => seg(&mut g, s, PEN_CUT_THICK),
            _ => {}
        }
    }
    for a in &view.arcs {
        if matches!(a.kind, HlrLineKind::Visible | HlrLineKind::Silhouette) {
            push_arc(&mut g, xf, a, PEN_VISIBLE);
        }
    }

    // Garis potong milik tampak ini.
    for section in &sheet.drawing.sections {
        if section.parent != xf.kind || !sheet.is_view_enabled(section.kind()) {
            continue;
        }
        let ind = &section.cutting_line;
        let pts: Vec<[f32; 2]> = ind.polyline().iter().map(|p| xf.to_paper(*p)).collect();
        let n = pts.len();
        for w in pts.windows(2) {
            g.line(w[0], w[1], PEN_CUT);
        }
        // Ujung dan siku ditebalkan (ISO 128-40).
        let toward = |a: [f32; 2], b: [f32; 2], len: f32| -> [f32; 2] {
            let d = [b[0] - a[0], b[1] - a[1]];
            let l = d[0].hypot(d[1]).max(1e-6);
            let k = if len >= 0.0 {
                len.min(l * 0.5) / l
            } else {
                len / l
            };
            [a[0] + d[0] * k, a[1] + d[1] * k]
        };
        g.line(pts[0], toward(pts[0], pts[1], 6.0), PEN_CUT_THICK);
        g.line(
            pts[n - 1],
            toward(pts[n - 1], pts[n - 2], 6.0),
            PEN_CUT_THICK,
        );
        for i in 1..n - 1 {
            g.line(pts[i], toward(pts[i], pts[i - 1], 3.0), PEN_CUT_THICK);
            g.line(pts[i], toward(pts[i], pts[i + 1], 3.0), PEN_CUT_THICK);
        }
        let ad = ind.arrow_dir;
        let labels = placement
            .map(|plc| sheet.cutting_label_positions_mm(plc, ind))
            .unwrap_or([pts[0], pts[n - 1]]);
        for (end, label) in [(pts[0], labels[0]), (pts[n - 1], labels[1])] {
            let tip = [end[0] + ad[0] * 8.0, end[1] + ad[1] * 8.0];
            g.line(end, tip, PEN_DIM);
            g.arrow(tip, [-ad[0], -ad[1]], "SECTION");
            g.text(label, &ind.label, 4.0, true, Anchor::Middle, "SECTION");
        }
    }

    // Lingkaran penanda detail milik tampak ini.
    for det in &sheet.drawing.detail_views {
        if det.indicator.parent_view != xf.kind {
            continue;
        }
        let ind = &det.indicator;
        g.items.push(Item::Circle {
            center: xf.to_paper(ind.center_2d),
            radius: ind.radius_mm * xf.scale,
            pen: PEN_DETAIL,
            fill: None,
        });
        g.text(
            xf.to_paper(ind.label_pos),
            &ind.label.to_string(),
            3.5,
            true,
            Anchor::Start,
            "SECTION",
        );
    }

    g.done(&format!("view_{}", view_key(xf.kind)))
}

/// Primitif satu dimensi — dipakai editor GUI supaya tampilannya sama persis
/// dengan ekspor.
pub fn dimension_items(dim: &DimensionAnnotation) -> Vec<Item> {
    let mut g = G::new();
    dimension(&mut g, dim);
    g.items
}

fn dimension(g: &mut G, dim: &DimensionAnnotation) {
    let l = "DIMENSIONS";
    let text = |g: &mut G, pos: [f32; 2], anchor: Anchor, angle_deg: f32| {
        g.items.push(Item::Text {
            pos,
            text: dim.text.clone(),
            size_mm: DIM_TEXT_MM,
            bold: false,
            anchor,
            angle_deg,
            layer: l,
        });
    };
    if let Some([cx, cy, r]) = dim.aux_circle {
        g.items.push(Item::Circle {
            center: [cx, cy],
            radius: r,
            pen: PEN_CENTER,
            fill: None,
        });
    }
    match dim.effective_style() {
        DimStyle::Leader => {
            let elbow = dim.line_pos;
            if (dim.start[0] - dim.end[0]).hypot(dim.start[1] - dim.end[1]) > 1e-3 {
                g.line(dim.start, dim.end, PEN_DIM);
            }
            g.line(dim.end, elbow, PEN_DIM);
            let (text_x, shoulder_x) = dim.leader_shoulder();
            g.line(elbow, [shoulder_x, elbow[1]], PEN_DIM);
            g.arrow(dim.end, [elbow[0] - dim.end[0], elbow[1] - dim.end[1]], l);
            text(g, [text_x, elbow[1] + 1.0], Anchor::Start, 0.0);
        }
        DimStyle::Angle => {
            g.line(dim.start, dim.end, PEN_DIM);
            g.line(dim.start, dim.line_pos, PEN_DIM);
            text(
                g,
                [dim.line_pos[0] + 2.0, dim.line_pos[1] - 1.0],
                Anchor::Start,
                0.0,
            );
        }
        DimStyle::Aligned => {
            let d = [dim.end[0] - dim.start[0], dim.end[1] - dim.start[1]];
            let len = d[0].hypot(d[1]).max(1e-6);
            let u = [d[0] / len, d[1] / len];
            let n = [-u[1], u[0]];
            let mid = [
                (dim.start[0] + dim.end[0]) * 0.5,
                (dim.start[1] + dim.end[1]) * 0.5,
            ];
            let o = (dim.line_pos[0] - mid[0]) * n[0] + (dim.line_pos[1] - mid[1]) * n[1];
            let sign = if o >= 0.0 { 1.0 } else { -1.0 };
            let p1 = [dim.start[0] + n[0] * o, dim.start[1] + n[1] * o];
            let p2 = [dim.end[0] + n[0] * o, dim.end[1] + n[1] * o];
            let over = 1.5 * sign;
            g.line(
                dim.start,
                [p1[0] + n[0] * over, p1[1] + n[1] * over],
                PEN_DIM,
            );
            g.line(dim.end, [p2[0] + n[0] * over, p2[1] + n[1] * over], PEN_DIM);
            g.line(p1, p2, PEN_DIM);
            g.arrow(p1, u, l);
            g.arrow(p2, [-u[0], -u[1]], l);
            let mut angle = u[1].atan2(u[0]).to_degrees();
            if !(-90.0..=90.0).contains(&angle) {
                angle += 180.0;
            }
            text(
                g,
                [dim.line_pos[0] + n[0] * sign, dim.line_pos[1] + n[1] * sign],
                Anchor::Middle,
                angle,
            );
        }
        _ => {
            if dim.is_vertical {
                let x = dim.line_pos[0];
                for p in [dim.start, dim.end] {
                    let sign = if x >= p[0] { 1.0 } else { -1.0 };
                    if (x - p[0]).abs() > 1.0 {
                        g.line([p[0] + sign, p[1]], [x + 1.5 * sign, p[1]], PEN_DIM);
                    }
                }
                let (ya, yb) = (dim.start[1].min(dim.end[1]), dim.start[1].max(dim.end[1]));
                let inside = yb - ya >= 7.0;
                let k = if inside { 1.0 } else { -1.0 };
                if inside {
                    g.line([x, ya], [x, yb], PEN_DIM);
                } else {
                    g.line([x, ya - 4.0], [x, yb + 4.0], PEN_DIM);
                }
                g.arrow([x, ya], [0.0, k], l);
                g.arrow([x, yb], [0.0, -k], l);
                text(g, [x - 1.0, (ya + yb) * 0.5], Anchor::Middle, 90.0);
            } else {
                let y = dim.line_pos[1];
                for p in [dim.start, dim.end] {
                    let sign = if y >= p[1] { 1.0 } else { -1.0 };
                    if (y - p[1]).abs() > 1.0 {
                        g.line([p[0], p[1] + sign], [p[0], y + 1.5 * sign], PEN_DIM);
                    }
                }
                let (xa, xb) = (dim.start[0].min(dim.end[0]), dim.start[0].max(dim.end[0]));
                let inside = xb - xa >= 7.0;
                let k = if inside { 1.0 } else { -1.0 };
                if inside {
                    g.line([xa, y], [xb, y], PEN_DIM);
                } else {
                    g.line([xa - 4.0, y], [xb + 4.0, y], PEN_DIM);
                }
                g.arrow([xa, y], [k, 0.0], l);
                g.arrow([xb, y], [-k, 0.0], l);
                text(g, [(xa + xb) * 0.5, y + 1.0], Anchor::Middle, 0.0);
            }
        }
    }
}

fn bom_table(sheet: &DrawingSheet) -> Group {
    let mut g = G::new();
    let tb = sheet.bom_table_rect_mm();
    let (x1, y1, x2, y2) = (tb[0], tb[1], tb[2], tb[3]);
    let col_w = sheet.bom_column_widths_mm();
    let title_h = sheet.bom_title_height_mm();
    let header_h = sheet.bom_header_height_mm();
    let row_h = sheet.bom_row_height_mm();
    let y_title = y2 - title_h;
    let y_header = y_title - header_h;
    let l = "BOM_TABLE";

    g.rect([x1, y1, x2, y2], None, Some([255, 255, 255]));
    g.rect([x1, y_title, x2, y2], None, Some([240, 242, 247]));
    g.rect([x1, y_header, x2, y_title], None, Some([224, 229, 237]));
    g.rect([x1, y1, x2, y2], Some(PEN_BOM_THICK), None);
    g.line([x1, y_title], [x2, y_title], PEN_BOM_THICK);
    g.line([x1, y_header], [x2, y_header], PEN_BOM_THICK);
    for i in 1..sheet.bom_table.items.len() {
        let y = y_header - i as f32 * row_h;
        g.line([x1, y], [x2, y], PEN_BOM);
    }
    let mut x = x1;
    for w in &col_w[..col_w.len() - 1] {
        x += w;
        g.line([x, y1], [x, y_title], PEN_BOM);
    }

    let title = if sheet.bom_table.title.is_empty() {
        "BILL OF MATERIALS"
    } else {
        &sheet.bom_table.title
    };
    g.text(
        [x1 + 4.0, y_title + 1.8],
        title,
        2.8,
        true,
        Anchor::Start,
        l,
    );
    let cell = |g: &mut G, col: usize, x: f32, y: f32, text: &str, bold: bool| {
        if col == 0 || col == 2 {
            g.text(
                [x + col_w[col] * 0.5, y],
                text,
                2.3,
                bold,
                Anchor::Middle,
                l,
            );
        } else {
            g.text([x + 2.0, y], text, 2.3, bold, Anchor::Start, l);
        }
    };
    let mut x = x1;
    for (i, name) in ["ITEM", "PART NAME", "QTY", "MATERIAL", "DESCRIPTION"]
        .iter()
        .enumerate()
    {
        cell(&mut g, i, x, y_header + 1.6, name, true);
        x += col_w[i];
    }
    for (row, item) in sheet.bom_table.items.iter().enumerate() {
        let y = y_header - (row + 1) as f32 * row_h + 1.5;
        let values = [
            item.item_number.to_string(),
            item.part_name.clone(),
            item.quantity.to_string(),
            item.material.clone(),
            item.description.clone(),
        ];
        let mut x = x1;
        for (i, v) in values.iter().enumerate() {
            cell(&mut g, i, x, y, v, i == 0 || i == 2);
            x += col_w[i];
        }
    }
    g.done("bom_table")
}

fn balloons(sheet: &DrawingSheet) -> Group {
    let mut g = G::new();
    let l = "CALLOUT_BALLOONS";
    for b in &sheet.balloons {
        let (t, c, r) = (b.target_point, b.balloon_pos, b.radius_mm);
        let d = [t[0] - c[0], t[1] - c[1]];
        let len = d[0].hypot(d[1]).max(0.1);
        let rim = [c[0] + d[0] / len * r, c[1] + d[1] / len * r];
        g.line(t, rim, PEN_BALLOON);
        g.arrow(t, [-d[0], -d[1]], l);
        g.items.push(Item::Circle {
            center: c,
            radius: r,
            pen: PEN_BALLOON,
            fill: Some([255, 255, 255]),
        });
        g.text(
            [c[0], c[1] - 1.1],
            &b.item_number.to_string(),
            3.0,
            true,
            Anchor::Middle,
            l,
        );
    }
    g.done("callout_balloons")
}

/// Lebar perkiraan teks (mm) — dipakai eksportir untuk jangkar tengah/akhir.
pub fn text_width(text: &str, size_mm: f32) -> f32 {
    text_width_mm(text, size_mm)
}
