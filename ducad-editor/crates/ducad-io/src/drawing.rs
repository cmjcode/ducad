//! Struktur data dan tata letak Lembar Kerja Gambar Teknik 2D (Engineering Drawing Sheet).
//!
//! Menyediakan spesifikasi ukuran kertas standar (A4/A3), bingkai ISO dengan grid zona referensi,
//! kepala gambar (title block), tata letak multi-tampak (Front, Top, Right, Isometric),
//! dan anotasi dimensi otomatis.

use ducad_core::drawing_annot::{Annotation, DimensionRef};
use ducad_kernel::{HlrDrawing, ProjectedViewKind};
use serde::{Deserialize, Serialize};

pub mod auto_dim;
pub mod exploded;
pub mod gdt;
pub mod raster;
pub mod scene;
pub mod spec;

pub use auto_dim::{fmt_dim, DIM_TEXT_MM};
pub use raster::{render_shaded, RasterImage, ShadedBody};
pub use spec::{
    parse_paper, parse_scale, parse_view_key, view_key, DimensionLayout, DimensionPolicy,
    DrawingSpec, ParentView, ScaleSpec, SectionSpec, ShadedCamera, ShadedSpec, SheetLayout,
    TitleSpec, ViewLayout, ViewOptions, ViewSet,
};

/// Ukuran kertas standar gambar teknik ISO 216.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum PaperSize {
    #[default]
    A4Landscape,
    A4Portrait,
    A3Landscape,
    A3Portrait,
}

impl PaperSize {
    pub fn dimensions_mm(self) -> (f32, f32) {
        match self {
            PaperSize::A4Landscape => (297.0, 210.0),
            PaperSize::A4Portrait => (210.0, 297.0),
            PaperSize::A3Landscape => (420.0, 297.0),
            PaperSize::A3Portrait => (297.0, 420.0),
        }
    }

    pub fn width_mm(self) -> f32 {
        self.dimensions_mm().0
    }

    pub fn height_mm(self) -> f32 {
        self.dimensions_mm().1
    }

    pub fn label(self) -> &'static str {
        match self {
            PaperSize::A4Landscape => "A4 Landscape (297 × 210 mm)",
            PaperSize::A4Portrait => "A4 Portrait (210 × 297 mm)",
            PaperSize::A3Landscape => "A3 Landscape (420 × 297 mm)",
            PaperSize::A3Portrait => "A3 Portrait (297 × 420 mm)",
        }
    }

    pub fn short_name(self) -> &'static str {
        match self {
            PaperSize::A4Landscape => "A4-L",
            PaperSize::A4Portrait => "A4-P",
            PaperSize::A3Landscape => "A3-L",
            PaperSize::A3Portrait => "A3-P",
        }
    }
}

/// Metadata Kepala Gambar (Title Block / Etiket Gambar Teknik) standar ISO 7200 / ASME Y14.1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TitleBlockInfo {
    pub project_title: String,
    pub drawing_number: String,
    pub drawn_by: String,
    pub date: String,
    pub scale: String,
    pub material: String,
    pub units: String,
    pub sheet_number: String,
    pub company_name: String,
    pub revision: String,
}

impl Default for TitleBlockInfo {
    fn default() -> Self {
        Self {
            project_title: String::new(),
            drawing_number: "DWG-2026-001".to_string(),
            drawn_by: "DUCAD Designer".to_string(),
            date: "2026-08-24".to_string(),
            scale: "1:1".to_string(),
            material: "Aluminium 6061-T6".to_string(),
            units: "mm".to_string(),
            sheet_number: "1 / 1".to_string(),
            company_name: "DUCAD Studio CAD/CAM".to_string(),
            revision: "A".to_string(),
        }
    }
}

/// Cara sebuah dimensi digambar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DimStyle {
    /// Dimensi lama tanpa gaya eksplisit: ditebak dari teksnya (awalan
    /// `R`/`Ø` = leader, akhiran `°` = sudut, selain itu linear).
    #[default]
    Auto,
    /// Linear mendatar/tegak (`is_vertical`), garis bantu + dua panah.
    Linear,
    /// Linear sejajar garis `start`→`end`; `line_pos` = tengah garis dimensi.
    Aligned,
    /// Leader: panah di `end`, siku di `line_pos`, bahu mendatar + teks.
    /// Bila `start != end`, garis `start`→`end` ikut digambar (gaya lama).
    Leader,
    /// Sudut: garis `start`→`end` dan `start`→`line_pos`, teks di `line_pos`.
    Angle,
}

/// Anotasi dimensi untuk gambar kerja.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DimensionAnnotation {
    /// Titik awal ukur (pada objek/viewport lembar kerja dalam mm).
    pub start: [f32; 2],
    /// Titik akhir ukur.
    pub end: [f32; 2],
    /// Posisi garis dimensi utama (jarak offset).
    pub line_pos: [f32; 2],
    /// Arah ekstensi dimensi.
    pub is_vertical: bool,
    /// Teks dimensi (mis. "166", "Ø42", "4×Ø14 PCD Ø130").
    pub text: String,
    #[serde(default)]
    pub style: DimStyle,
    /// Sumber geometri (P21.4). Bila ada, `start/end/line_pos/text` dihitung
    /// ulang dari sini; dimensi lama tanpa `source` tetap mutlak.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<DimensionRef>,
    /// Jarak garis dimensi dari tepi tampak (linear) atau panjang leader.
    #[serde(default)]
    pub offset_mm: f32,
    /// Arah leader (derajat, berlawanan jarum jam dari +X).
    #[serde(default)]
    pub angle_deg: f32,
    /// Tampak pemilik dimensi.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<ProjectedViewKind>,
    /// Lingkaran bantu (PCD pola lubang): `[cx, cy, r]` di kertas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aux_circle: Option<[f32; 3]>,
    /// Posisi (`offset_mm`/`angle_deg`) diatur pengguna: dipertahankan saat
    /// dimensi otomatis dibuat ulang.
    #[serde(default)]
    pub pinned: bool,
}

impl Default for DimensionAnnotation {
    fn default() -> Self {
        Self {
            start: [0.0, 0.0],
            end: [0.0, 0.0],
            line_pos: [0.0, 0.0],
            is_vertical: false,
            text: String::new(),
            style: DimStyle::Auto,
            source: None,
            offset_mm: 0.0,
            angle_deg: 0.0,
            view: None,
            aux_circle: None,
            pinned: false,
        }
    }
}

impl DimensionAnnotation {
    /// Gaya yang benar-benar dipakai penggambar ([`DimStyle::Auto`] ditebak
    /// dari teks, seperti perilaku sebelum P21).
    pub fn effective_style(&self) -> DimStyle {
        match self.style {
            DimStyle::Auto => {
                if self.text.ends_with('°') {
                    DimStyle::Angle
                } else if self.text.starts_with('R') || self.text.starts_with('Ø') {
                    DimStyle::Leader
                } else {
                    DimStyle::Linear
                }
            }
            other => other,
        }
    }
}

/// Penempatan satu tampak proyeksi pada lembar kerja.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetViewPlacement {
    pub kind: ProjectedViewKind,
    /// Posisi titik pusat tampak pada kertas (mm dari pojok kiri-bawah).
    pub center_mm: [f32; 2],
    /// Skala tampak (1.0 = 1:1, 0.5 = 1:2, 2.0 = 2:1).
    pub scale: f32,
    pub visible: bool,
    /// Tulis "SCALE 1:2" di judul tampak (otomatis bila skalanya berbeda
    /// dari skala lembar atau ditetapkan eksplisit).
    #[serde(default)]
    pub show_scale: bool,
}

/// Render berbayang yang disematkan di lembar (P21.5).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadedView {
    pub spec: ShadedSpec,
    /// Pusat gambar di kertas (mm).
    pub center_mm: [f32; 2],
    pub width_mm: f32,
    pub height_mm: f32,
    #[serde(default = "default_true")]
    pub visible: bool,
    /// Piksel hasil render; kosong sampai diisi mesin render.
    #[serde(skip)]
    pub image: RasterImage,
}

impl ShadedView {
    /// Kotak gambar di kertas `[x0, y0, x1, y1]`.
    pub fn rect_mm(&self) -> [f32; 4] {
        [
            self.center_mm[0] - self.width_mm * 0.5,
            self.center_mm[1] - self.height_mm * 0.5,
            self.center_mm[0] + self.width_mm * 0.5,
            self.center_mm[1] + self.height_mm * 0.5,
        ]
    }

    /// Ukuran piksel yang diminta untuk gambar ini.
    pub fn pixel_size(&self) -> (u32, u32) {
        let ppm = self.spec.px_per_mm.clamp(1, 16) as f32;
        let (mut w, mut h) = (self.width_mm * ppm, self.height_mm * ppm);
        let long = w.max(h);
        if long > raster::MAX_SHADED_PX as f32 {
            let k = raster::MAX_SHADED_PX as f32 / long;
            w *= k;
            h *= k;
        }
        ((w.round() as u32).max(8), (h.round() as u32).max(8))
    }
}

/// Anotasi teks bebas pada lembar kerja (catatan teknis, keterangan khusus, instruksi).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextAnnotation {
    /// Posisi teks pada kertas dalam milimeter (mm dari pojok kiri bawah kertas).
    pub position: [f32; 2],
    /// Isi string teks anotasi.
    pub text: String,
    /// Ukuran font dalam mm (standar ISO 2.5mm, 3.5mm, 5.0mm, atau 7.0mm).
    pub font_size: f32,
}

impl Default for TextAnnotation {
    fn default() -> Self {
        Self {
            position: [20.0, 20.0],
            text: "CATATAN TEKNIS".to_string(),
            font_size: 3.5,
        }
    }
}

/// Satu baris dalam Tabel Daftar Komponen / Bill of Materials (BOM) standar ISO 7573 / ASME Y14.34.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BomItem {
    /// Nomor item penunjuk (terhubung dengan nomor lingkaran Callout Balloon).
    pub item_number: usize,
    /// Nama part / komponen solid.
    pub part_name: String,
    /// Jumlah kuantitas pemakaian dalam perakitan / model.
    pub quantity: u32,
    /// Jenis bahan material komponen (mis. "Aluminium 6061-T6", "Steel 1045", "ABS Plastic").
    pub material: String,
    /// Catatan teknis / spesifikasi tambahan.
    pub description: String,
}

impl Default for BomItem {
    fn default() -> Self {
        Self {
            item_number: 1,
            part_name: "Part-1".to_string(),
            quantity: 1,
            material: "Aluminium 6061-T6".to_string(),
            description: String::new(),
        }
    }
}

/// Struktur data Tabel BOM (Bill of Materials) pada lembar kerja gambar teknik.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BomTable {
    pub title: String,
    pub items: Vec<BomItem>,
    /// Posisi pojok kiri-bawah tabel dalam mm (jika None, dihitung otomatis di atas Kepala Gambar).
    pub custom_pos_mm: Option<[f32; 2]>,
}

impl Default for BomTable {
    fn default() -> Self {
        Self {
            title: "BILL OF MATERIALS (BOM)".to_string(),
            items: Vec::new(),
            custom_pos_mm: None,
        }
    }
}

/// Lingkaran nomor penunjuk part (*Part Callout Balloon*) standar ISO/ASME.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalloutBalloon {
    pub id: u32,
    /// Nomor item yang ditunjuk (terhubung dengan `BomItem::item_number`).
    pub item_number: usize,
    /// Titik sasaran/panah pada komponen objek 3D (mm dari pojok kiri-bawah kertas).
    pub target_point: [f32; 2],
    /// Posisi titik pusat lingkaran balon pada kertas (mm dari pojok kiri-bawah kertas).
    pub balloon_pos: [f32; 2],
    /// Radius lingkaran balon dalam mm (standar 4.5 s/d 5.0 mm).
    pub radius_mm: f32,
    /// Tampak proyeksi yang diasosiasikan (biasanya Isometric).
    pub view_kind: ProjectedViewKind,
}

impl CalloutBalloon {
    pub fn new(
        id: u32,
        item_number: usize,
        target_point: [f32; 2],
        balloon_pos: [f32; 2],
        view_kind: ProjectedViewKind,
    ) -> Self {
        Self {
            id,
            item_number,
            target_point,
            balloon_pos,
            radius_mm: 5.0,
            view_kind,
        }
    }
}

pub(crate) fn default_true() -> bool {
    true
}

/// Dokumen Lembar Kerja Teknik 2D lengkap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawingSheet {
    pub paper_size: PaperSize,
    pub title_block: TitleBlockInfo,
    pub drawing: HlrDrawing,
    pub view_placements: Vec<SheetViewPlacement>,
    pub scale: f32,
    pub show_hidden_lines: bool,
    pub show_dimensions: bool,
    pub show_centerlines: bool,
    #[serde(default = "default_true")]
    pub show_section_view: bool,
    #[serde(default = "default_true")]
    pub show_detail_views: bool,
    #[serde(default = "default_true")]
    pub show_hatch: bool,
    #[serde(default = "default_true")]
    pub show_bom_table: bool,
    #[serde(default = "default_true")]
    pub show_balloons: bool,
    #[serde(default)]
    pub bom_table: BomTable,
    #[serde(default)]
    pub balloons: Vec<CalloutBalloon>,
    pub auto_dimensions: Vec<DimensionAnnotation>,
    #[serde(default)]
    pub manual_dimensions: Vec<DimensionAnnotation>,
    #[serde(default)]
    pub custom_texts: Vec<TextAnnotation>,
    /// Anotasi GD&T/toleransi (P19): toleransi dimensi, bingkai kontrol fitur,
    /// datum, kekasaran, tabel lubang/revisi. Digambar oleh ekspor PDF dan SVG.
    #[serde(default)]
    pub annotations: Vec<Annotation>,
    /// Penimpaan per tampak (terlihat/skala) — bertahan melewati tata letak ulang.
    #[serde(default)]
    pub view_options: ViewSet,
    /// Skala lembar tetap; `None` = dipilih otomatis.
    #[serde(default)]
    pub fixed_scale: Option<f32>,
    /// Catatan umum, digambar di atas kepala gambar.
    #[serde(default)]
    pub notes: Vec<String>,
    /// Render berbayang (P21.5).
    #[serde(default)]
    pub shaded: Vec<ShadedView>,
    #[serde(default)]
    pub dimension_policy: DimensionPolicy,
    /// Geometri berubah setelah lembar ini dibuat (indikator GUI).
    #[serde(default)]
    pub stale: bool,
}

/// Jarak antar baris catatan (mm) dan tinggi teksnya.
pub const NOTE_PITCH_MM: f32 = 5.0;
pub const NOTE_TEXT_MM: f32 = 3.0;

// Ruang di sekeliling geometri tampak untuk dimensi dan judul (mm kertas).
const PAD_L: f32 = 24.0;
const PAD_R: f32 = 10.0;
const PAD_T: f32 = 22.0;
const PAD_B: f32 = 14.0;

impl DrawingSheet {
    /// Membuat lembar kerja baru dan menghitung tata letak otomatis yang optimal.
    pub fn new(drawing: HlrDrawing, paper_size: PaperSize) -> Self {
        let mut sheet = Self {
            paper_size,
            title_block: TitleBlockInfo::default(),
            drawing,
            view_placements: Vec::new(),
            scale: 1.0,
            show_hidden_lines: true,
            show_dimensions: true,
            show_centerlines: true,
            show_section_view: true,
            show_detail_views: true,
            show_hatch: true,
            show_bom_table: true,
            show_balloons: true,
            bom_table: BomTable::default(),
            balloons: Vec::new(),
            auto_dimensions: Vec::new(),
            manual_dimensions: Vec::new(),
            custom_texts: Vec::new(),
            annotations: Vec::new(),
            view_options: ViewSet::default(),
            fixed_scale: None,
            notes: Vec::new(),
            shaded: Vec::new(),
            dimension_policy: DimensionPolicy::Auto,
            stale: false,
        };

        sheet.auto_layout();
        sheet
    }

    /// Bangun lembar dari [`DrawingSpec`]: tampak/skala/catatan/kebijakan
    /// dimensi, lalu tata letak pengguna (`spec.layout`) bila ada. Gambar
    /// render berbayang masih kosong — isi `sheet.shaded[i].image` sesudahnya
    /// (ukurannya dari [`ShadedView::pixel_size`]).
    pub fn from_spec(drawing: HlrDrawing, spec: &DrawingSpec) -> Self {
        let mut sheet = Self::new(drawing, spec.paper);
        sheet.view_options = spec.views.clone();
        sheet.fixed_scale = match spec.scale {
            ScaleSpec::Auto => None,
            ScaleSpec::Fixed(s) => Some(s),
        };
        sheet.show_hidden_lines = spec.hidden_lines;
        sheet.notes = spec.notes.clone();
        sheet.dimension_policy = spec.dimensions.clone();
        sheet.shaded = spec
            .shaded
            .iter()
            .map(|s| ShadedView {
                spec: *s,
                center_mm: [0.0, 0.0],
                width_mm: 0.0,
                height_mm: 0.0,
                visible: true,
                image: RasterImage::default(),
            })
            .collect();
        let tb = &mut sheet.title_block;
        tb.project_title = spec.title.title.clone();
        tb.drawing_number = spec.title.part_number.clone();
        tb.drawn_by = spec.title.author.clone();
        tb.date = spec.title.date.clone();
        tb.material = spec.title.material.clone();
        tb.revision = spec.title.revision.clone();
        sheet.auto_layout();
        if let Some(layout) = &spec.layout {
            sheet.apply_layout(layout);
        }
        sheet
    }

    /// Tata letak hasil suntingan pengguna, untuk disimpan di `DrawingSpec.layout`.
    pub fn to_layout(&self) -> SheetLayout {
        SheetLayout {
            views: self
                .view_placements
                .iter()
                .map(|p| ViewLayout {
                    view: view_key(p.kind),
                    center_mm: p.center_mm,
                    scale: self.view_options.get(p.kind).scale,
                    visible: p.visible,
                })
                .collect(),
            shaded_centers: self.shaded.iter().map(|s| s.center_mm).collect(),
            dimensions: self
                .auto_dimensions
                .iter()
                .filter(|d| d.pinned)
                .filter_map(|d| {
                    Some(DimensionLayout {
                        source: d.source.clone()?,
                        offset_mm: d.offset_mm,
                        angle_deg: d.angle_deg,
                    })
                })
                .collect(),
            manual_dimensions: self.manual_dimensions.clone(),
            texts: self.custom_texts.clone(),
            balloons: self.balloons.clone(),
            bom: (!self.bom_table.items.is_empty() || self.bom_table.custom_pos_mm.is_some())
                .then(|| self.bom_table.clone()),
            annotations: self.annotations.clone(),
            title_block: Some(self.title_block.clone()),
        }
    }

    /// Terapkan tata letak pengguna di atas tata letak otomatis.
    pub fn apply_layout(&mut self, layout: &SheetLayout) {
        // Skala/keterlihatan dulu, karena memengaruhi tata letak otomatis.
        let mut relayout = false;
        for v in &layout.views {
            let Some(kind) = parse_view_key(&v.view) else {
                continue;
            };
            let opt = self.view_options.entry(kind);
            if opt.scale != v.scale || opt.visible.unwrap_or(true) != v.visible {
                opt.scale = v.scale.or(opt.scale);
                opt.visible = Some(v.visible);
                relayout = true;
            }
        }
        if relayout {
            self.auto_layout();
        }
        for v in &layout.views {
            let Some(kind) = parse_view_key(&v.view) else {
                continue;
            };
            if let Some(plc) = self.view_placements.iter_mut().find(|p| p.kind == kind) {
                plc.center_mm = v.center_mm;
            }
        }
        for (view, center) in self.shaded.iter_mut().zip(&layout.shaded_centers) {
            view.center_mm = *center;
        }
        if let Some(tb) = &layout.title_block {
            let scale = self.title_block.scale.clone();
            self.title_block = tb.clone();
            self.title_block.scale = scale;
        }
        self.custom_texts = layout.texts.clone();
        self.balloons = layout.balloons.clone();
        if let Some(bom) = &layout.bom {
            self.bom_table = bom.clone();
        }
        self.annotations = layout.annotations.clone();
        self.manual_dimensions = layout.manual_dimensions.clone();
        // Posisi tampak berubah → dimensi dibuat ulang, lalu offset pengguna.
        self.generate_auto_dimensions();
        for d in &layout.dimensions {
            if let Some(slot) = self
                .auto_dimensions
                .iter_mut()
                .position(|a| a.source.as_ref().is_some_and(|s| auto_dim::same_source(s, &d.source)))
            {
                // Pakai sumber BARU (petunjuknya sudah mengikuti geometri kini).
                let Some(source) = self.auto_dimensions[slot].source.clone() else {
                    continue;
                };
                if let Some(mut fresh) = self.resolve_dimension(&source, d.offset_mm, d.angle_deg) {
                    fresh.pinned = true;
                    self.auto_dimensions[slot] = fresh;
                }
            }
        }
        self.refresh_associative_dimensions();
    }

    /// Apakah tampak `kind` digambar (opsi per tampak + sakelar global).
    pub fn is_view_enabled(&self, kind: ProjectedViewKind) -> bool {
        let opt = self.view_options.get(kind).visible.unwrap_or(true);
        match kind {
            ProjectedViewKind::Section(_) => opt && self.show_section_view,
            ProjectedViewKind::Detail(_) => opt && self.show_detail_views,
            _ => opt,
        }
    }

    /// Ubah skala satu tampak (`None` = ikut skala lembar) lalu tata ulang.
    pub fn set_view_scale(&mut self, kind: ProjectedViewKind, scale: Option<f32>) {
        self.view_options.entry(kind).scale = scale.filter(|s| s.is_finite() && *s > 1e-4);
        let s = self.scale;
        self.layout_with_scale(s);
    }

    /// Tinggi blok catatan (mm), termasuk baris judul "NOTE:".
    pub fn notes_height_mm(&self) -> f32 {
        if self.notes.is_empty() {
            0.0
        } else {
            (self.notes.len() + 1) as f32 * NOTE_PITCH_MM + 3.0
        }
    }

    /// Y terbawah area bebas di atas kepala gambar + tabel BOM.
    fn stack_top_mm(&self) -> f32 {
        let tb = self.title_block_rect_mm();
        if self.show_bom_table && !self.bom_table.items.is_empty() && self.bom_table.custom_pos_mm.is_none() {
            self.bom_table_rect_mm()[3]
        } else {
            tb[3]
        }
    }

    /// Garis dasar tiap baris catatan: indeks 0 = judul "NOTE:", lalu catatan.
    pub fn note_baselines_mm(&self) -> Vec<[f32; 2]> {
        let tb = self.title_block_rect_mm();
        let n = self.notes.len() + 1;
        let base = self.stack_top_mm() + 4.0;
        (0..n)
            .map(|i| [tb[0] + 2.0, base + (n - 1 - i) as f32 * NOTE_PITCH_MM])
            .collect()
    }

    /// Seberapa jauh judul tampak diturunkan supaya tidak menimpa ujung
    /// garis potong yang keluar dari sisi bawah tampak (mm kertas).
    pub fn view_title_drop_mm(&self, plc: &SheetViewPlacement) -> f32 {
        let view = self.drawing.view_by_kind(plc.kind);
        let bottom = view.bounds_min[1];
        let mut drop = 0.0f32;
        for section in &self.drawing.sections {
            if section.parent != plc.kind || !self.is_view_enabled(section.kind()) {
                continue;
            }
            let pts = section.cutting_line.polyline();
            for end in [pts[0], pts[pts.len() - 1]] {
                if end[1] < bottom {
                    // Ujung garis + panah + huruf label.
                    drop = drop.max((bottom - end[1]) * plc.scale + 9.0);
                }
            }
        }
        drop
    }

    /// Posisi dua huruf label sebuah garis potong di kertas (mm) — dipakai
    /// penggambar dan penghindar tabrakan teks dimensi.
    pub fn cutting_label_positions_mm(
        &self,
        plc: &SheetViewPlacement,
        line: &ducad_kernel::CuttingLineIndicator,
    ) -> [[f32; 2]; 2] {
        let view = self.drawing.view_by_kind(plc.kind);
        let vc = view.center_2d();
        let to_paper = |p: [f32; 2]| {
            [
                plc.center_mm[0] + (p[0] - vc[0]) * plc.scale,
                plc.center_mm[1] + (p[1] - vc[1]) * plc.scale,
            ]
        };
        let pts: Vec<[f32; 2]> = line.polyline().iter().map(|p| to_paper(*p)).collect();
        let n = pts.len();
        let ad = line.arrow_dir;
        let label = |end: [f32; 2], other: [f32; 2]| {
            let d = [end[0] - other[0], end[1] - other[1]];
            let l = d[0].hypot(d[1]).max(1e-6);
            [
                end[0] + d[0] / l * 4.0 + ad[0] * 4.0,
                end[1] + d[1] / l * 4.0 + ad[1] * 4.0 - 1.5,
            ]
        };
        [label(pts[0], pts[1]), label(pts[n - 1], pts[n - 2])]
    }

    /// Kotak judul di bawah sebuah tampak `[x0, y0, x1, y1]`.
    pub fn view_title_box_mm(&self, plc: &SheetViewPlacement) -> [f32; 4] {
        let view = self.drawing.view_by_kind(plc.kind);
        let sz = view.size_2d();
        let bottom = plc.center_mm[1] - sz[1] * plc.scale * 0.5 - self.view_title_drop_mm(plc);
        let w = gdt::text_width_mm(&self.view_title(plc), 3.0).max(20.0);
        [
            plc.center_mm[0] - w * 0.5,
            bottom - 11.5,
            plc.center_mm[0] + w * 0.5,
            bottom - 3.0,
        ]
    }

    /// Judul tampak (baris pertama).
    pub fn view_title(&self, plc: &SheetViewPlacement) -> String {
        let view = self.drawing.view_by_kind(plc.kind);
        match plc.kind {
            ProjectedViewKind::Detail(c) => format!("DETAIL {c}"),
            kind => format!("{} | {}", view.title, kind.title_en()),
        }
    }

    /// Baris skala judul tampak, bila perlu ditulis.
    pub fn view_scale_label(&self, plc: &SheetViewPlacement) -> Option<String> {
        let differs = (plc.scale - self.scale).abs() > 1e-4;
        let wanted = plc.show_scale || differs;
        (wanted && plc.kind != ProjectedViewKind::Isometric)
            .then(|| format!("SCALE {}", format_scale_ratio(plc.scale)))
    }

    /// Kotak yang tidak boleh ditimpa teks dimensi.
    pub(crate) fn layout_obstacles(&self) -> Vec<[f32; 4]> {
        let mut out = vec![self.title_block_rect_mm()];
        if self.show_bom_table && !self.bom_table.items.is_empty() {
            out.push(self.bom_table_rect_mm());
        }
        if !self.notes.is_empty() {
            let tb = self.title_block_rect_mm();
            let y0 = self.stack_top_mm();
            out.push([tb[0], y0, tb[2], y0 + self.notes_height_mm()]);
        }
        for plc in &self.view_placements {
            if !plc.visible {
                continue;
            }
            out.push(self.view_title_box_mm(plc));
            // Huruf + panah garis potong di tampak ini.
            for section in &self.drawing.sections {
                if section.parent == plc.kind && self.is_view_enabled(section.kind()) {
                    for p in self.cutting_label_positions_mm(plc, &section.cutting_line) {
                        out.push([p[0] - 5.0, p[1] - 4.0, p[0] + 5.0, p[1] + 6.0]);
                    }
                }
            }
        }
        for s in &self.shaded {
            if s.visible {
                out.push(s.rect_mm());
            }
        }
        out
    }

    /// Memvalidasi seluruh anotasi GD&T/toleransi lembar ini (aturan ISO 1101,
    /// kelas ISO 286, label datum ganda). Ekspor tidak memanggilnya: anotasi
    /// yang tidak sah tetap digambar apa adanya.
    pub fn validate_annotations(&self) -> Result<(), String> {
        ducad_core::drawing_annot::validate_annotations(&self.annotations)
    }

    /// Menghitung skala lembar (kecuali `fixed_scale`) dan tata letak seluruh
    /// tampak, lalu membuat ulang dimensi otomatis.
    pub fn auto_layout(&mut self) {
        let scale = match self.fixed_scale {
            Some(s) => s.max(0.001),
            None => {
                // Skala standar terbesar yang muat.
                let candidates = [
                    5.0, 4.0, 3.0, 2.5, 2.0, 1.5, 1.25, 1.0, 0.75, 0.667, 0.5, 0.4, 0.333, 0.25,
                    0.2, 0.15, 0.125, 0.1, 0.075, 0.0667, 0.05, 0.04, 0.025, 0.02, 0.01,
                ];
                candidates
                    .iter()
                    .copied()
                    .find(|s| self.place_views(*s).1)
                    .unwrap_or(0.01)
            }
        };
        self.layout_with_scale(scale);
    }

    /// Menghitung tata letak posisi tampak dengan nilai skala tertentu (mis. dari slider fleksibel pengguna).
    pub fn layout_with_scale(&mut self, s: f32) {
        self.scale = s.max(0.001);
        self.title_block.scale = format_scale_ratio(self.scale);
        let (placements, _, shaded) = self.place_views(self.scale);
        self.view_placements = placements;
        for (view, (center, w, h)) in self.shaded.iter_mut().zip(shaded) {
            // Ukuran berubah → gambar lama tidak cocok lagi.
            if (view.width_mm - w).abs() > 1e-3 || (view.height_mm - h).abs() > 1e-3 {
                view.image = RasterImage::default();
            }
            view.center_mm = center;
            view.width_mm = w;
            view.height_mm = h;
        }
        self.generate_auto_dimensions();
    }

    /// Skala efektif sebuah tampak pada skala lembar `s`.
    fn effective_view_scale(&self, kind: ProjectedViewKind, s: f32) -> f32 {
        let base = match kind {
            ProjectedViewKind::Detail(c) => self
                .drawing
                .detail_views
                .iter()
                .find(|d| d.indicator.label == c)
                .map(|d| s * d.scale_multiplier)
                .unwrap_or(s),
            _ => s,
        };
        self.view_options.get(kind).scale.unwrap_or(base)
    }

    /// Tata letak pada skala lembar `s`: (penempatan, muat?, kotak render
    /// berbayang `(pusat, lebar, tinggi)`).
    ///
    /// Aturan proyeksi sudut ketiga: Tampak Atas di atas Tampak Depan;
    /// potongan yang memandang mendatar (garis potong tegak di Tampak Atas)
    /// sebaris dengan Tampak Depan dan disejajarkan pada sumbu Z model;
    /// potongan lain sebaris dengan Tampak Atas. Isometrik, render berbayang,
    /// dan detail menempati kolom kanan di atas kepala gambar.
    #[allow(clippy::type_complexity)]
    fn place_views(&self, s: f32) -> (Vec<SheetViewPlacement>, bool, Vec<([f32; 2], f32, f32)>) {
        let (_, inner) = self.border_rects_mm();
        let tb = self.title_block_rect_mm();
        let x0 = inner[0] + 2.0;
        let y0 = inner[1] + 2.0;
        let y1 = inner[3] - 2.0;
        let stack_top = self.stack_top_mm() + self.notes_height_mm() + 3.0;

        // Baris tampak ortografik.
        let front_on = self.is_view_enabled(ProjectedViewKind::Front);
        let top_on = self.is_view_enabled(ProjectedViewKind::Top);
        let mut row0: Vec<ProjectedViewKind> = Vec::new();
        let mut row1: Vec<ProjectedViewKind> = Vec::new();
        if front_on {
            row0.push(ProjectedViewKind::Front);
        }
        if top_on {
            row1.push(ProjectedViewKind::Top);
        }
        if self.is_view_enabled(ProjectedViewKind::Right) {
            row0.push(ProjectedViewKind::Right);
        }
        for section in &self.drawing.sections {
            let kind = section.kind();
            if !self.is_view_enabled(kind) {
                continue;
            }
            let d = section.config.normal;
            if d[0].abs() > 0.9 {
                // Memandang sepanjang X: di sebelah Tampak Depan.
                row0.push(kind);
            } else if d[1].abs() > 0.9 {
                // Memandang sepanjang Y: menggantikan Depan bila Depan tidak ada.
                if front_on {
                    row1.push(kind);
                } else {
                    row0.insert(0, kind);
                }
            } else if top_on {
                row1.push(kind);
            } else {
                row1.insert(0, kind);
            }
        }

        // Kolom kanan.
        enum Side {
            Iso,
            Shaded(usize),
            Detail(char),
        }
        let mut column: Vec<Side> = Vec::new();
        for (i, v) in self.shaded.iter().enumerate() {
            if v.visible {
                column.push(Side::Shaded(i));
            }
        }
        if self.is_view_enabled(ProjectedViewKind::Isometric) {
            column.push(Side::Iso);
        }
        for det in &self.drawing.detail_views {
            if self.is_view_enabled(ProjectedViewKind::Detail(det.indicator.label)) {
                column.push(Side::Detail(det.indicator.label));
            }
        }
        let col_w = if column.is_empty() {
            0.0
        } else {
            (tb[2] - tb[0]).min((inner[2] - inner[0]) * 0.36)
        };
        let x1 = if col_w > 0.0 { inner[2] - 2.0 - col_w - 4.0 } else { inner[2] - 2.0 };

        struct Cell {
            kind: ProjectedViewKind,
            scale: f32,
            /// Ukuran geometri di kertas.
            w: f32,
            h: f32,
            /// Pusat model 2D tampak.
            vc: [f32; 2],
        }
        let cell = |kind: ProjectedViewKind| -> Cell {
            let view = self.drawing.view_by_kind(kind);
            let scale = self.effective_view_scale(kind, s);
            let sz = view.size_2d();
            Cell {
                kind,
                scale,
                w: sz[0] * scale,
                h: sz[1] * scale,
                vc: view.center_2d(),
            }
        };
        let cells0: Vec<Cell> = row0.iter().map(|k| cell(*k)).collect();
        let cells1: Vec<Cell> = row1.iter().map(|k| cell(*k)).collect();
        let cw = |c: &Cell| c.w + PAD_L + PAD_R;
        let ch = |c: &Cell| c.h + PAD_T + PAD_B;

        // Kolom pertama dipakai bersama Depan/Atas supaya keduanya sejajar.
        let share_col0 = front_on && top_on;
        let col0_w = if share_col0 {
            cw(&cells0[0]).max(cw(&cells1[0]))
        } else {
            0.0
        };
        let row_w = |cells: &[Cell]| -> f32 {
            cells
                .iter()
                .enumerate()
                .map(|(i, c)| if i == 0 && share_col0 { col0_w } else { cw(c) })
                .sum()
        };
        let (w0, w1) = (row_w(&cells0), row_w(&cells1));
        let h0 = cells0.iter().map(&ch).fold(0.0, f32::max);
        let h1 = cells1.iter().map(&ch).fold(0.0, f32::max);

        // Baris bawah yang menabrak zona kepala gambar dinaikkan seluruhnya.
        let raised = x0 + w0 > tb[0] - 2.0 && !cells0.is_empty();
        let base0 = if raised { stack_top.max(y0) } else { y0 };
        let avail_h = y1 - base0;
        let fits = w0 <= x1 - x0 + 1e-3 && w1 <= x1 - x0 + 1e-3 && h0 + h1 <= avail_h + 1e-3;

        let slack_v = (avail_h - h0 - h1).max(0.0);
        let row0_y = base0 + (slack_v * 0.25).min(10.0);
        let row1_y = row0_y + h0 + if cells0.is_empty() { 0.0 } else { (slack_v * 0.5).min(25.0) };

        let mut placements: Vec<SheetViewPlacement> = Vec::new();
        let mut place_row = |cells: &[Cell], row_y: f32, row_h: f32, total_w: f32| {
            let slack = (x1 - x0 - total_w).max(0.0);
            let gap = (slack / cells.len().max(1) as f32).min(20.0);
            let mut x = x0 + gap * 0.5;
            for (i, c) in cells.iter().enumerate() {
                let width = if i == 0 && share_col0 { col0_w } else { cw(c) };
                let geo_w = width - PAD_L - PAD_R;
                placements.push(SheetViewPlacement {
                    kind: c.kind,
                    center_mm: [x + PAD_L + geo_w * 0.5, row_y + PAD_B + (row_h - PAD_T - PAD_B) * 0.5],
                    scale: c.scale,
                    visible: true,
                    show_scale: self.view_options.get(c.kind).scale.is_some(),
                });
                x += width + gap;
            }
        };
        place_row(&cells0, row0_y, h0, w0);
        place_row(&cells1, row1_y, h1, w1);

        // Sejajarkan pada koordinat model: Depan/Atas pada sumbu X, dan
        // tampak sebaris Depan pada sumbu Z (hanya bila skalanya sama).
        let find = |list: &[SheetViewPlacement], kind: ProjectedViewKind| list.iter().position(|p| p.kind == kind);
        if share_col0 {
            if let (Some(f), Some(t)) = (
                find(&placements, ProjectedViewKind::Front),
                find(&placements, ProjectedViewKind::Top),
            ) {
                if (placements[f].scale - placements[t].scale).abs() < 1e-6 {
                    let k = placements[f].scale;
                    placements[t].center_mm[0] =
                        placements[f].center_mm[0] + (cells1[0].vc[0] - cells0[0].vc[0]) * k;
                }
            }
        }
        if let Some(lead) = cells0.first() {
            if let Some(li) = find(&placements, lead.kind) {
                let (lead_y, lead_scale) = (placements[li].center_mm[1], placements[li].scale);
                for c in cells0.iter().skip(1) {
                    let z_up = match c.kind {
                        ProjectedViewKind::Front | ProjectedViewKind::Right => true,
                        ProjectedViewKind::Section(l) => self
                            .drawing
                            .section(l)
                            .map(|sec| sec.config.v_axis[2] > 0.999)
                            .unwrap_or(false),
                        _ => false,
                    };
                    let lead_z_up = !matches!(lead.kind, ProjectedViewKind::Section(l) if self
                        .drawing
                        .section(l)
                        .map(|sec| sec.config.v_axis[2] <= 0.999)
                        .unwrap_or(true));
                    if z_up && lead_z_up && (c.scale - lead_scale).abs() < 1e-6 {
                        if let Some(i) = find(&placements, c.kind) {
                            placements[i].center_mm[1] = lead_y + (c.vc[1] - lead.vc[1]) * lead_scale;
                        }
                    }
                }
            }
        }

        // Kolom kanan: bagi rata tingginya.
        let mut shaded_rects: Vec<([f32; 2], f32, f32)> = self
            .shaded
            .iter()
            .map(|v| (v.center_mm, v.width_mm, v.height_mm))
            .collect();
        if !column.is_empty() {
            let cx = inner[2] - 2.0 - col_w * 0.5;
            let col_y0 = stack_top.max(y0);
            let cell_h = ((y1 - col_y0) / column.len() as f32).max(20.0);
            for (i, item) in column.iter().enumerate() {
                let cy = y1 - cell_h * (i as f32 + 0.5);
                match item {
                    Side::Shaded(idx) => {
                        let w = (col_w - 6.0).min((cell_h - 6.0) * 4.0 / 3.0).max(16.0);
                        shaded_rects[*idx] = ([cx, cy], w, w * 0.75);
                    }
                    Side::Iso => {
                        let sz = self.drawing.isometric.size_2d();
                        let fit = ((col_w - 8.0) / sz[0]).min((cell_h - 18.0) / sz[1]).max(0.001);
                        placements.push(SheetViewPlacement {
                            kind: ProjectedViewKind::Isometric,
                            center_mm: [cx, cy + 4.0],
                            scale: self
                                .view_options
                                .get(ProjectedViewKind::Isometric)
                                .scale
                                .unwrap_or(s.min(fit)),
                            visible: true,
                            show_scale: false,
                        });
                    }
                    Side::Detail(label) => {
                        let kind = ProjectedViewKind::Detail(*label);
                        placements.push(SheetViewPlacement {
                            kind,
                            center_mm: [cx, cy + 4.0],
                            scale: self.effective_view_scale(kind, s),
                            visible: true,
                            show_scale: true,
                        });
                    }
                }
            }
        }

        (placements, fits, shaded_rects)
    }

    /// Menambahkan atau memperbarui Detail View lingkaran pembesar pada tampak acuan.
    pub fn add_or_update_detail_view(
        &mut self,
        parent_kind: ProjectedViewKind,
        center_2d: [f32; 2],
        radius_mm: f32,
        scale_multiplier: f32,
        label: char,
    ) {
        let parent_view = self.drawing.view_by_kind(parent_kind);
        let indicator = ducad_kernel::DetailIndicator::new(label, parent_kind, center_2d, radius_mm);
        let detail_data = ducad_kernel::DetailExtractor::extract_detail_view(parent_view, &indicator, scale_multiplier);

        if let Some(existing) = self.drawing.detail_views.iter_mut().find(|d| d.indicator.label == label) {
            *existing = detail_data;
        } else {
            self.drawing.detail_views.push(detail_data);
        }

        // Perbarui placement atau tambah baru
        let det_scale = self.scale * scale_multiplier;
        if let Some(plc) = self.view_placements.iter_mut().find(|p| p.kind == ProjectedViewKind::Detail(label)) {
            plc.scale = det_scale;
        } else {
            let (paper_w, paper_h) = self.paper_size.dimensions_mm();
            let def_x = (paper_w - 60.0).max(40.0);
            let def_y = (paper_h - 60.0).max(40.0);
            self.view_placements.push(SheetViewPlacement {
                kind: ProjectedViewKind::Detail(label),
                center_mm: [def_x, def_y],
                scale: det_scale,
                visible: true,
                show_scale: true,
            });
        }
    }

    /// Menghapus Detail View berdasarkan huruf label ('B', 'C', dll).
    pub fn remove_detail_view(&mut self, label: char) {
        self.drawing.detail_views.retain(|d| d.indicator.label != label);
        self.view_placements.retain(|p| p.kind != ProjectedViewKind::Detail(label));
    }

    /// Membuat ulang dimensi otomatis menurut `dimension_policy`. Dimensi
    /// yang posisinya sudah diatur pengguna (`pinned`) mempertahankan
    /// `offset_mm`/`angle_deg`-nya; dimensi manual tidak disentuh.
    pub fn generate_auto_dimensions(&mut self) {
        let previous = std::mem::take(&mut self.auto_dimensions);
        let mut fresh = match &self.dimension_policy {
            DimensionPolicy::Auto => auto_dim::generate(self),
            DimensionPolicy::None => Vec::new(),
            DimensionPolicy::Only(refs) => auto_dim::place_only(self, refs),
        };
        for dim in &mut fresh {
            let Some(source) = dim.source.clone() else {
                continue;
            };
            if let Some(old) = previous
                .iter()
                .find(|p| p.pinned && p.source.as_ref().is_some_and(|s| auto_dim::same_source(s, &source)))
            {
                if let Some(mut kept) = self.resolve_dimension(&source, old.offset_mm, old.angle_deg) {
                    kept.pinned = true;
                    *dim = kept;
                }
            }
        }
        self.auto_dimensions = fresh;
    }

    /// Lebar masing-masing kolom tabel BOM dalam mm: [ITEM, PART NAME, QTY, MATERIAL, DESCRIPTION].
    pub fn bom_column_widths_mm(&self) -> [f32; 5] {
        [14.0, 44.0, 14.0, 38.0, 30.0]
    }

    /// Total lebar tabel BOM dalam mm (140 mm sejajar dengan lebar Title Block).
    pub fn bom_table_width_mm(&self) -> f32 {
        self.bom_column_widths_mm().iter().sum()
    }

    /// Tinggi baris data tabel BOM dalam mm.
    pub fn bom_row_height_mm(&self) -> f32 {
        5.5
    }

    /// Tinggi header judul "BILL OF MATERIALS" dalam mm.
    pub fn bom_title_height_mm(&self) -> f32 {
        6.5
    }

    /// Tinggi header kolom dalam mm.
    pub fn bom_header_height_mm(&self) -> f32 {
        5.5
    }

    /// Koordinat kotak Tabel BOM [min_x, min_y, max_x, max_y] dalam mm.
    pub fn bom_table_rect_mm(&self) -> [f32; 4] {
        let w = self.bom_table_width_mm();
        let num_rows = self.bom_table.items.len().max(1);
        let total_h = self.bom_title_height_mm() + self.bom_header_height_mm() + (num_rows as f32 * self.bom_row_height_mm());

        if let Some(pos) = self.bom_table.custom_pos_mm {
            [pos[0], pos[1], pos[0] + w, pos[1] + total_h]
        } else {
            let (_, inner) = self.border_rects_mm();
            let tb = self.title_block_rect_mm();
            let x1 = inner[2] - w;
            let y1 = tb[3] + 1.5;
            [x1, y1, x1 + w, y1 + total_h]
        }
    }

    /// Tambahkan satu part callout balloon baru.
    pub fn add_balloon(
        &mut self,
        item_number: usize,
        target_point: [f32; 2],
        balloon_pos: [f32; 2],
        view_kind: ProjectedViewKind,
    ) -> u32 {
        let max_id = self.balloons.iter().map(|b| b.id).max().unwrap_or(0);
        let id = max_id + 1;
        self.balloons.push(CalloutBalloon::new(id, item_number, target_point, balloon_pos, view_kind));
        id
    }

    /// Hapus callout balloon berdasarkan ID.
    pub fn remove_balloon(&mut self, id: u32) {
        self.balloons.retain(|b| b.id != id);
    }

    /// Otomatis hitung posisi balon melingkar di sekeliling tampak isometrik.
    pub fn auto_position_balloons_around_iso(&mut self) {
        if self.balloons.is_empty() {
            return;
        }

        let count = self.balloons.len();
        for (i, balloon) in self.balloons.iter_mut().enumerate() {
            if balloon.view_kind != ProjectedViewKind::Isometric {
                continue;
            }
            // Sudut sebaran radial merata
            let angle = (i as f32 / count as f32) * std::f32::consts::TAU + (std::f32::consts::PI * 0.25);
            let dir = [angle.cos(), angle.sin()];
            let dist = 22.0; // 22 mm offset dari target point
            balloon.balloon_pos = [
                balloon.target_point[0] + dir[0] * dist,
                balloon.target_point[1] + dir[1] * dist,
            ];
        }
    }

    /// Garis batas tepi luar dan bingkai gambar dalam (mm).
    pub fn border_rects_mm(&self) -> ([f32; 4], [f32; 4]) {
        let (pw, ph) = self.paper_size.dimensions_mm();
        // Luar kertas: [0, 0, pw, ph]
        let outer = [0.0, 0.0, pw, ph];
        // Bingkai dalam: margin kiri 20mm (jilid), atas/bawah/kanan 10mm
        let inner = [20.0, 10.0, pw - 10.0, ph - 10.0];
        (outer, inner)
    }

    /// Koordinat kotak kepala gambar (Title Block) di pojok kanan-bawah bingkai (mm).
    pub fn title_block_rect_mm(&self) -> [f32; 4] {
        let (_, inner) = self.border_rects_mm();
        let w = 140.0;
        let h = 45.0;
        [inner[2] - w, inner[1], inner[2], inner[1] + h]
    }
}

pub fn format_scale_ratio(scale: f32) -> String {
    if (scale - 1.0).abs() < 1e-4 {
        "1:1".to_string()
    } else if (scale - 0.5).abs() < 1e-3 {
        "1:2".to_string()
    } else if (scale - 0.4).abs() < 1e-3 {
        "1:2.5".to_string()
    } else if (scale - 0.333).abs() < 0.01 {
        "1:3".to_string()
    } else if (scale - 0.25).abs() < 1e-3 {
        "1:4".to_string()
    } else if (scale - 0.2).abs() < 1e-3 {
        "1:5".to_string()
    } else if (scale - 0.1333).abs() < 0.01 || (scale - 0.125).abs() < 1e-3 {
        "1:8".to_string()
    } else if (scale - 0.1).abs() < 1e-3 {
        "1:10".to_string()
    } else if (scale - 0.0667).abs() < 0.005 || (scale - 0.075).abs() < 0.005 {
        "1:15".to_string()
    } else if (scale - 0.05).abs() < 1e-3 {
        "1:20".to_string()
    } else if (scale - 0.02).abs() < 1e-3 {
        "1:50".to_string()
    } else if (scale - 0.01).abs() < 1e-3 {
        "1:100".to_string()
    } else if scale < 1.0 {
        let denom = (1.0 / scale * 10.0).round() / 10.0;
        if denom.fract() < 1e-2 {
            format!("1:{}", denom as u32)
        } else {
            format!("1:{:.1}", denom)
        }
    } else {
        let num = (scale * 10.0).round() / 10.0;
        if num.fract() < 1e-2 {
            format!("{}:1", num as u32)
        } else {
            format!("{:.1}:1", num)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_kernel::{HlrDrawing, HlrGeometricFeature, HlrLineKind, HlrSegment2D, ProjectedView, ProjectedViewKind};

    fn make_test_drawing(w: f32, h: f32, d: f32) -> HlrDrawing {
        let make_view = |kind: ProjectedViewKind, vw: f32, vh: f32, feat: Vec<HlrGeometricFeature>| ProjectedView {
            kind,
            title: kind.title_id().to_string(),
            bounds_min: [0.0, 0.0],
            bounds_max: [vw, vh],
            segments: vec![
                HlrSegment2D { start: [0.0, 0.0], end: [vw, 0.0], kind: HlrLineKind::Visible },
                HlrSegment2D { start: [vw, 0.0], end: [vw, vh], kind: HlrLineKind::Visible },
                HlrSegment2D { start: [vw, vh], end: [0.0, vh], kind: HlrLineKind::Visible },
                HlrSegment2D { start: [0.0, vh], end: [0.0, 0.0], kind: HlrLineKind::Visible },
            ],
            centerlines: vec![HlrSegment2D { start: [vw * 0.5, 0.0], end: [vw * 0.5, vh], kind: HlrLineKind::Centerline }],
            features: feat,
            width_mm: vw,
            height_mm: vh,
            depth_mm: d,
            ..ProjectedView::default()
        };

        HlrDrawing {
            front: make_view(
                ProjectedViewKind::Front,
                w,
                h,
                vec![
                    HlrGeometricFeature::Circle { center: [w * 0.5, h * 0.5], radius: 25.0, edge: None },
                    HlrGeometricFeature::Angle { vertex: [0.0, 0.0], arm1_end: [20.0, 0.0], arm2_end: [20.0, 20.0], angle_deg: 45.0 },
                ],
            ),
            top: make_view(
                ProjectedViewKind::Top,
                w,
                d,
                vec![
                    HlrGeometricFeature::Arc { center: [w * 0.5, d * 0.5], radius: 15.0, start_angle: 0.0, end_angle: std::f32::consts::PI, edge: None },
                    HlrGeometricFeature::Ellipse { center: [w * 0.25, d * 0.5], radius_x: 30.0, radius_y: 12.0, rotation: 0.0 },
                ],
            ),
            right: make_view(ProjectedViewKind::Right, d, h, Vec::new()),
            isometric: make_view(ProjectedViewKind::Isometric, w * 0.9, (h + d) * 0.8, Vec::new()),
            sections: Vec::new(),
            detail_views: Vec::new(),
            model_bbox_min: [0.0, 0.0, 0.0],
            model_bbox_max: [w, d, h],
            warnings: Vec::new(),
        }
    }

    #[test]
    fn test_detail_view_creation_and_layout() {
        let drawing = make_test_drawing(200.0, 100.0, 80.0);
        let mut sheet = DrawingSheet::new(drawing, PaperSize::A4Landscape);

        // Tambah Detail View 'B' pada Tampak Depan
        sheet.add_or_update_detail_view(
            ProjectedViewKind::Front,
            [100.0, 50.0],
            15.0,
            2.0, // Skala 2:1
            'B',
        );

        assert_eq!(sheet.drawing.detail_views.len(), 1);
        let det = &sheet.drawing.detail_views[0];
        assert_eq!(det.indicator.label, 'B');
        assert_eq!(det.scale_multiplier, 2.0);

        // Cek placement
        let det_plc = sheet
            .view_placements
            .iter()
            .find(|p| p.kind == ProjectedViewKind::Detail('B'));
        assert!(det_plc.is_some());
        let det_plc = det_plc.unwrap();
        assert_eq!(det_plc.scale, sheet.scale * 2.0);

        // Hapus detail view
        sheet.remove_detail_view('B');
        assert_eq!(sheet.drawing.detail_views.len(), 0);
        assert!(!sheet.view_placements.iter().any(|p| p.kind == ProjectedViewKind::Detail('B')));
    }

    #[test]
    fn test_auto_layout_adaptive_scaling_and_fill_factor() {
        // Model 790 x 329.2 x 130.53 mm sesuai kasus pengguna
        let drawing = make_test_drawing(790.0, 130.53, 329.2);

        // 1. A4 Landscape (297 x 210)
        let sheet_a4 = DrawingSheet::new(drawing.clone(), PaperSize::A4Landscape);
        assert!(sheet_a4.scale > 0.0, "Skala A4 harus bernilai positif");
        assert_eq!(sheet_a4.view_placements.len(), 4, "Harus ada 4 tampak proyeksi (fixture tanpa potongan)");

        let (a4_w, a4_h) = sheet_a4.paper_size.dimensions_mm();
        for plc in &sheet_a4.view_placements {
            assert!(plc.center_mm[0] > 10.0 && plc.center_mm[0] < a4_w - 5.0, "Tampak harus berada dalam kertas X");
            assert!(plc.center_mm[1] > 10.0 && plc.center_mm[1] < a4_h - 5.0, "Tampak harus berada dalam kertas Y");
        }

        // 2. A3 Landscape (420 x 297)
        let sheet_a3 = DrawingSheet::new(drawing, PaperSize::A3Landscape);
        assert!(sheet_a3.scale >= sheet_a4.scale, "Skala A3 harus lebih besar atau sama dengan A4");
    }

    #[test]
    fn test_auto_dimensions_geometric_features() {
        let drawing = make_test_drawing(100.0, 50.0, 40.0);
        let sheet = DrawingSheet::new(drawing, PaperSize::A4Landscape);

        // Verifikasi dimensi linier dan dimensi kurva / sudut ada
        let has_radius = sheet.auto_dimensions.iter().any(|d| d.text.starts_with('R') && !d.text.starts_with("Rx"));
        let has_diameter = sheet.auto_dimensions.iter().any(|d| d.text.starts_with('Ø'));
        let has_ellipse = sheet.auto_dimensions.iter().any(|d| d.text.starts_with("Rx "));
        let has_angle = sheet.auto_dimensions.iter().any(|d| d.text.ends_with('°'));

        assert!(has_diameter, "Dimensi diameter lingkaran harus muncul");
        assert!(has_radius, "Dimensi radius busur R harus muncul");
        assert!(has_ellipse, "Dimensi radius ellips Rx/Ry harus muncul");
        assert!(has_angle, "Dimensi sudut ° harus muncul");
    }

    #[test]
    fn test_manual_dimensions_persistence() {
        let drawing = make_test_drawing(100.0, 50.0, 40.0);
        let mut sheet = DrawingSheet::new(drawing, PaperSize::A4Landscape);

        // Tambahkan dimensi kustom manual
        sheet.manual_dimensions.push(DimensionAnnotation {
            start: [20.0, 30.0],
            end: [80.0, 30.0],
            line_pos: [50.0, 35.0],
            is_vertical: false,
            text: "60.00 mm (Custom)".to_string(),
            ..Default::default()
        });
        sheet.manual_dimensions.push(DimensionAnnotation {
            start: [50.0, 50.0],
            end: [60.0, 60.0],
            line_pos: [65.0, 62.0],
            is_vertical: false,
            text: "Ø 20.00 mm".to_string(),
            ..Default::default()
        });
        sheet.manual_dimensions.push(DimensionAnnotation {
            start: [50.0, 50.0],
            end: [70.0, 50.0],
            line_pos: [65.0, 65.0],
            is_vertical: false,
            text: "45.0°".to_string(),
            ..Default::default()
        });

        assert_eq!(sheet.manual_dimensions.len(), 3);

        // Pastikan regenerate auto dimensions tidak menghapus manual dimensions
        sheet.generate_auto_dimensions();
        assert_eq!(sheet.manual_dimensions.len(), 3);
        assert_eq!(sheet.manual_dimensions[0].text, "60.00 mm (Custom)");
        assert_eq!(sheet.manual_dimensions[1].text, "Ø 20.00 mm");
        assert_eq!(sheet.manual_dimensions[2].text, "45.0°");
    }

    #[test]
    fn test_bom_table_and_callout_balloons() {
        let drawing = make_test_drawing(100.0, 50.0, 40.0);
        let mut sheet = DrawingSheet::new(drawing, PaperSize::A4Landscape);

        // Tambahkan baris BOM
        sheet.bom_table.items.push(BomItem {
            item_number: 1,
            part_name: "Base Frame".to_string(),
            quantity: 1,
            material: "Aluminium 6061-T6".to_string(),
            description: "Main chassis".to_string(),
        });
        sheet.bom_table.items.push(BomItem {
            item_number: 2,
            part_name: "Mounting Bracket".to_string(),
            quantity: 4,
            material: "Steel 1045".to_string(),
            description: "Fastener support".to_string(),
        });

        assert_eq!(sheet.bom_table.items.len(), 2);
        let rect = sheet.bom_table_rect_mm();
        assert!(rect[2] > rect[0], "Lebar tabel BOM harus positif");
        assert!(rect[3] > rect[1], "Tinggi tabel BOM harus positif");
        assert!((sheet.bom_table_width_mm() - 140.0).abs() < 1e-3, "Lebar tabel BOM harus 140 mm");

        // Tambahkan Callout Balloons
        let b1 = sheet.add_balloon(1, [200.0, 150.0], [220.0, 165.0], ProjectedViewKind::Isometric);
        let b2 = sheet.add_balloon(2, [180.0, 140.0], [160.0, 125.0], ProjectedViewKind::Isometric);
        assert_eq!(sheet.balloons.len(), 2);
        assert_eq!(b1, 1);
        assert_eq!(b2, 2);

        // Uji auto position balloons
        sheet.auto_position_balloons_around_iso();
        assert!((sheet.balloons[0].balloon_pos[0] - 200.0).abs() > 1.0);

        // Hapus balon
        sheet.remove_balloon(b1);
        assert_eq!(sheet.balloons.len(), 1);
        assert_eq!(sheet.balloons[0].id, b2);
    }
}

