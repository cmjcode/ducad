//! Struktur data dan tata letak Lembar Kerja Gambar Teknik 2D (Engineering Drawing Sheet).
//!
//! Menyediakan spesifikasi ukuran kertas standar (A4/A3), bingkai ISO dengan grid zona referensi,
//! kepala gambar (title block), tata letak multi-tampak (Front, Top, Right, Isometric),
//! dan anotasi dimensi otomatis.

use ducad_kernel::{HlrDrawing, HlrGeometricFeature, ProjectedViewKind};
use serde::{Deserialize, Serialize};

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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Anotasi dimensi linier untuk gambar kerja.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionAnnotation {
    /// Titik awal ukur (pada objek/viewport lembar kerja dalam mm).
    pub start: [f32; 2],
    /// Titik akhir ukur.
    pub end: [f32; 2],
    /// Posisi garis dimensi utama (jarak offset).
    pub line_pos: [f32; 2],
    /// Arah ekstensi dimensi.
    pub is_vertical: bool,
    /// Teks dimensi (mis. "50.00 mm" atau "R 12.5").
    pub text: String,
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
}

/// Anotasi teks bebas pada lembar kerja (catatan teknis, keterangan khusus, instruksi).
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub auto_dimensions: Vec<DimensionAnnotation>,
    #[serde(default)]
    pub custom_texts: Vec<TextAnnotation>,
}

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
            auto_dimensions: Vec::new(),
            custom_texts: Vec::new(),
        };

        sheet.auto_layout();
        sheet
    }

    /// Menghitung tata letak posisi tampak (Front, Top, Right, Isometric) dan skala yang pas dengan kertas
    /// secara adaptif menggunakan formula dimensi geometris 3D untuk memaksimalkan area gambar (bebas ruang kosong).
    pub fn auto_layout(&mut self) {
        let (paper_w, paper_h) = self.paper_size.dimensions_mm();

        // Area gambar yang dapat digunakan (dikurangi margin tepi dan title block di kanan bawah)
        let left_margin = 20.0; // Margin jilid ISO
        let right_margin = 10.0;
        let top_margin = 10.0;
        let bottom_margin = 10.0;
        let title_block_h = 45.0;

        let inner_w = paper_w - left_margin - right_margin;
        let inner_h = paper_h - top_margin - bottom_margin;

        // Bounding box 2D tampak nyata (dari objek 3D)
        let front_sz = self.drawing.front.size_2d();
        let top_sz = self.drawing.top.size_2d();
        let right_sz = self.drawing.right.size_2d();
        let iso_sz = self.drawing.isometric.size_2d();

        let col1_w = front_sz[0].max(top_sz[0]);
        let col1_h = front_sz[1] + top_sz[1];

        let col2_w = right_sz[0].max(iso_sz[0] * 0.85);
        let col2_h = (right_sz[1] + iso_sz[1]).max(title_block_h + iso_sz[1]);

        let total_w_raw = col1_w + col2_w;
        let total_h_raw = col1_h.max(col2_h);

        // Clearance terukur untuk garis dimensi, judul tampak, dan toleransi tepi
        let clearance_w = 36.0;
        let clearance_h = 42.0;

        let avail_w = (inner_w - clearance_w).max(40.0);
        let avail_h = (inner_h - clearance_h).max(40.0);

        let scale_w = avail_w / total_w_raw.max(1.0);
        let scale_h = avail_h / total_h_raw.max(1.0);
        let raw_best_scale = scale_w.min(scale_h).min(5.0);

        // Pilih skala standar terdekat dengan resolusi granular tinggi agar kertas terisi penuh
        let optimal_scale = pick_standard_scale(raw_best_scale);
        self.layout_with_scale(optimal_scale);
    }

    /// Menghitung tata letak posisi tampak dengan nilai skala tertentu (mis. dari slider fleksibel pengguna).
    pub fn layout_with_scale(&mut self, s: f32) {
        let (paper_w, paper_h) = self.paper_size.dimensions_mm();

        let left_margin = 20.0;
        let right_margin = 10.0;
        let top_margin = 10.0;
        let bottom_margin = 10.0;
        let title_block_w = 140.0;
        let title_block_h = 45.0;

        self.scale = s.max(0.001);
        self.title_block.scale = format_scale_ratio(self.scale);

        let front_sz = self.drawing.front.size_2d();
        let top_sz = self.drawing.top.size_2d();
        let right_sz = self.drawing.right.size_2d();
        let iso_sz = self.drawing.isometric.size_2d();

        let w_f_raw = front_sz[0];
        let h_f_raw = front_sz[1];
        let w_t_raw = top_sz[0];
        let h_t_raw = top_sz[1];
        let w_r_raw = right_sz[0];
        let h_r_raw = right_sz[1];
        let w_iso_raw = iso_sz[0];
        let h_iso_raw = iso_sz[1];

        let w_f = w_f_raw * s;
        let h_f = h_f_raw * s;
        let w_t = w_t_raw * s;
        let h_t = h_t_raw * s;
        let w_r = w_r_raw * s;
        let h_r = h_r_raw * s;
        let w_iso = w_iso_raw * s;
        let h_iso = h_iso_raw * s;

        // Batas kerja vertikal yang aman (bebas tumpukan dengan border bawah & atas)
        let y_usable_min = bottom_margin + 22.0;
        let y_usable_max = paper_h - top_margin - 12.0;
        let h_avail_usable = (y_usable_max - y_usable_min).max(40.0);

        // Ruang sisa vertikal kolom utama (Front + Top)
        let total_views_h = h_f + h_t;
        let slack_h = (h_avail_usable - total_views_h).max(0.0);

        // Distribusikan sisa vertikal secara proporsional ke gap tengah dan padding atas/bawah
        let gap_y = (slack_h * 0.50).clamp(16.0, 70.0);
        let pad_y = ((slack_h - gap_y).max(0.0) * 0.50).max(0.0);

        // 1. Tampak Depan (Front View): terangkat naik dari batas bawah sehingga judul & dimensi lega
        let front_center_y = y_usable_min + pad_y + h_f * 0.5;

        // 2. Tampak Atas (Top View): naik ke atas mengisi ruang kosong kertas di zona atas
        let top_center_y = front_center_y + (h_f + h_t) * 0.5 + gap_y;

        // Batas kerja horizontal yang aman
        let x_usable_min = left_margin + 16.0;
        let x_usable_max = paper_w - right_margin - 8.0;
        let w_avail_usable = (x_usable_max - x_usable_min).max(40.0);

        let total_views_w = w_f.max(w_t) + w_r.max(w_iso);
        let slack_w = (w_avail_usable - total_views_w).max(0.0);

        let gap_x = (slack_w * 0.40).clamp(16.0, 60.0);
        let pad_x = ((slack_w - gap_x).max(0.0) * 0.50).max(0.0);

        let front_center_x = x_usable_min + pad_x + w_f * 0.5;
        let top_center_x = front_center_x;

        // 3. Tampak Samping Kanan (Right View): sejajar horizontal dengan Front View,
        // namun jika posisi horizontalnya berada di atas Title Block (kanan bawah),
        // naikkan posisi Y agar tidak menabrak Title Block (Kepala Gambar)
        let right_center_x = front_center_x + (w_f + w_r) * 0.5 + gap_x;

        let title_block_left_x = paper_w - right_margin - title_block_w;
        let title_block_top_y = bottom_margin + title_block_h;

        let right_center_y = if right_center_x + w_r * 0.5 + 4.0 > title_block_left_x {
            // Berada di kolom kanan (zona Title Block): posisikan aman di atas Title Block
            let min_safe_y = title_block_top_y + 22.0 + h_r * 0.5;
            front_center_y.max(min_safe_y)
        } else {
            front_center_y
        };

        // 4. Tampak Isometrik 3D: di kuadran kanan-atas, mengisi zona atas
        let iso_min_x = front_center_x + (w_f + w_iso) * 0.5 + 6.0;
        let iso_max_x = paper_w - right_margin - w_iso * 0.5 - 4.0;
        let iso_target_x = right_center_x + (w_r - w_iso) * 0.2;
        let iso_center_x = clamp_safe(iso_target_x, iso_min_x, iso_max_x);

        let min_iso_y = right_center_y + (h_r + h_iso) * 0.5 + 12.0;
        let max_iso_y = paper_h - top_margin - 8.0 - h_iso * 0.5;
        let iso_center_y = clamp_safe(top_center_y, min_iso_y, max_iso_y);

        self.view_placements = vec![
            SheetViewPlacement {
                kind: ProjectedViewKind::Front,
                center_mm: [front_center_x, front_center_y],
                scale: s,
                visible: true,
            },
            SheetViewPlacement {
                kind: ProjectedViewKind::Top,
                center_mm: [top_center_x, top_center_y],
                scale: s,
                visible: true,
            },
            SheetViewPlacement {
                kind: ProjectedViewKind::Right,
                center_mm: [right_center_x, right_center_y],
                scale: s,
                visible: true,
            },
            SheetViewPlacement {
                kind: ProjectedViewKind::Isometric,
                center_mm: [iso_center_x, iso_center_y],
                scale: s,
                visible: true,
            },
        ];

        // Buat dimensi otomatis untuk seluruh tampak
        self.generate_auto_dimensions();
    }

    /// Membuat anotasi dimensi pembatas (Overall Length, Width, Height) serta dimensi geometri lengkap (R, Ø, sudut, ellips).
    pub fn generate_auto_dimensions(&mut self) {
        self.auto_dimensions.clear();
        let s = self.scale;

        // 1. Dimensi Panjang & Tinggi pada Tampak Depan
        if let Some(front_plc) = self
            .view_placements
            .iter()
            .find(|p| p.kind == ProjectedViewKind::Front)
        {
            let front_view = &self.drawing.front;
            let cx = front_plc.center_mm[0];
            let cy = front_plc.center_mm[1];
            let v_center = front_view.center_2d();

            let x_min = cx + (front_view.bounds_min[0] - v_center[0]) * s;
            let x_max = cx + (front_view.bounds_max[0] - v_center[0]) * s;
            let y_min = cy + (front_view.bounds_min[1] - v_center[1]) * s;
            let y_max = cy + (front_view.bounds_max[1] - v_center[1]) * s;

            let length_mm = (front_view.bounds_max[0] - front_view.bounds_min[0]).abs();
            let height_mm = (front_view.bounds_max[1] - front_view.bounds_min[1]).abs();

            if length_mm > 0.5 {
                // Dimensi horizontal di bawah Tampak Depan
                let dim_y = y_min - 8.0;
                self.auto_dimensions.push(DimensionAnnotation {
                    start: [x_min, y_min],
                    end: [x_max, y_min],
                    line_pos: [(x_min + x_max) * 0.5, dim_y],
                    is_vertical: false,
                    text: format!("{:.2} mm", length_mm),
                });
            }

            if height_mm > 0.5 {
                // Dimensi vertikal di kiri Tampak Depan
                let dim_x = x_min - 8.0;
                self.auto_dimensions.push(DimensionAnnotation {
                    start: [x_min, y_min],
                    end: [x_min, y_max],
                    line_pos: [dim_x, (y_min + y_max) * 0.5],
                    is_vertical: true,
                    text: format!("{:.2} mm", height_mm),
                });
            }

            // Anotasi fitur geometri (R, Ø, Ellips, Sudut) pada Tampak Depan
            append_view_feature_dimensions(&mut self.auto_dimensions, front_view, cx, cy, s);
        }

        // 2. Dimensi Lebar / Tebal pada Tampak Samping Kanan
        if let Some(right_plc) = self
            .view_placements
            .iter()
            .find(|p| p.kind == ProjectedViewKind::Right)
        {
            let right_view = &self.drawing.right;
            let cx = right_plc.center_mm[0];
            let cy = right_plc.center_mm[1];
            let v_center = right_view.center_2d();

            let x_min = cx + (right_view.bounds_min[0] - v_center[0]) * s;
            let x_max = cx + (right_view.bounds_max[0] - v_center[0]) * s;
            let y_min = cy + (right_view.bounds_min[1] - v_center[1]) * s;

            let width_mm = (right_view.bounds_max[0] - right_view.bounds_min[0]).abs();
            if width_mm > 0.5 {
                // Dimensi horizontal di bawah Tampak Samping
                let dim_y = y_min - 8.0;
                self.auto_dimensions.push(DimensionAnnotation {
                    start: [x_min, y_min],
                    end: [x_max, y_min],
                    line_pos: [(x_min + x_max) * 0.5, dim_y],
                    is_vertical: false,
                    text: format!("{:.2} mm", width_mm),
                });
            }

            append_view_feature_dimensions(&mut self.auto_dimensions, right_view, cx, cy, s);
        }

        // 3. Dimensi Ukuran Pendukung pada Tampak Atas (Top View)
        if let Some(top_plc) = self
            .view_placements
            .iter()
            .find(|p| p.kind == ProjectedViewKind::Top)
        {
            let top_view = &self.drawing.top;
            let cx = top_plc.center_mm[0];
            let cy = top_plc.center_mm[1];
            let v_center = top_view.center_2d();

            let x_min = cx + (top_view.bounds_min[0] - v_center[0]) * s;
            let x_max = cx + (top_view.bounds_max[0] - v_center[0]) * s;
            let y_min = cy + (top_view.bounds_min[1] - v_center[1]) * s;
            let y_max = cy + (top_view.bounds_max[1] - v_center[1]) * s;

            let length_mm = (top_view.bounds_max[0] - top_view.bounds_min[0]).abs();
            let width_mm = (top_view.bounds_max[1] - top_view.bounds_min[1]).abs();

            if width_mm > 0.5 {
                // Dimensi vertikal di kiri Tampak Atas (lebar/kedalaman Y)
                let dim_x = x_min - 8.0;
                self.auto_dimensions.push(DimensionAnnotation {
                    start: [x_min, y_min],
                    end: [x_min, y_max],
                    line_pos: [dim_x, (y_min + y_max) * 0.5],
                    is_vertical: true,
                    text: format!("{:.2} mm", width_mm),
                });
            }

            if length_mm > 0.5 {
                // Dimensi horizontal di atas Tampak Atas (panjang X)
                let dim_y = y_max + 8.0;
                self.auto_dimensions.push(DimensionAnnotation {
                    start: [x_min, y_max],
                    end: [x_max, y_max],
                    line_pos: [(x_min + x_max) * 0.5, dim_y],
                    is_vertical: false,
                    text: format!("{:.2} mm", length_mm),
                });
            }

            // Anotasi koordinat/posisi titik sumbu (centerlines) lubang/silinder terhadap tepi referensi
            for cl in &top_view.centerlines {
                let mid_u = (cl.start[0] + cl.end[0]) * 0.5;
                let mid_v = (cl.start[1] + cl.end[1]) * 0.5;
                let cx_c = cx + (mid_u - v_center[0]) * s;
                let cy_c = cy + (mid_v - v_center[1]) * s;

                // Hanya ambil garis sumbu horizontal sebagai wakil titik pusat
                if (cl.start[1] - cl.end[1]).abs() < 1e-3 && (cx_c - x_min) > 2.0 && (x_max - cx_c) > 2.0 {
                    let offset_x_mm = (mid_u - top_view.bounds_min[0]).abs();
                    if offset_x_mm > 5.0 && offset_x_mm < length_mm - 5.0 {
                        self.auto_dimensions.push(DimensionAnnotation {
                            start: [x_min, cy_c],
                            end: [cx_c, cy_c],
                            line_pos: [(x_min + cx_c) * 0.5, cy_c],
                            is_vertical: false,
                            text: format!("{:.2} mm", offset_x_mm),
                        });
                    }
                }
            }

            append_view_feature_dimensions(&mut self.auto_dimensions, top_view, cx, cy, s);
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

/// Menambahkan anotasi fitur geometris (Radius, Diameter, Sudut, Ellips) untuk suatu tampak
fn append_view_feature_dimensions(
    dims: &mut Vec<DimensionAnnotation>,
    view: &ducad_kernel::ProjectedView,
    cx: f32,
    cy: f32,
    scale: f32,
) {
    let v_center = view.center_2d();
    let mut idx = 0;

    for feat in &view.features {
        match feat {
            HlrGeometricFeature::Circle { center, radius } => {
                let cx_c = cx + (center[0] - v_center[0]) * scale;
                let cy_c = cy + (center[1] - v_center[1]) * scale;
                let r_screen = radius * scale;

                let angle_deg = 35.0 + (idx as f32 * 45.0);
                idx += 1;
                let angle = angle_deg.to_radians();
                let end_pt = [cx_c + r_screen * angle.cos(), cy_c + r_screen * angle.sin()];
                let leader_bend = [end_pt[0] + 6.0, end_pt[1] + 4.0];

                dims.push(DimensionAnnotation {
                    start: [cx_c, cy_c],
                    end: end_pt,
                    line_pos: leader_bend,
                    is_vertical: false,
                    text: format!("Ø {:.2} mm", radius * 2.0),
                });
            }
            HlrGeometricFeature::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                let cx_c = cx + (center[0] - v_center[0]) * scale;
                let cy_c = cy + (center[1] - v_center[1]) * scale;
                let r_screen = radius * scale;

                let mid_a = (start_angle + end_angle) * 0.5;
                let end_pt = [cx_c + r_screen * mid_a.cos(), cy_c + r_screen * mid_a.sin()];
                let leader_bend = [end_pt[0] + 6.0, end_pt[1] + 4.0];

                dims.push(DimensionAnnotation {
                    start: [cx_c, cy_c],
                    end: end_pt,
                    line_pos: leader_bend,
                    is_vertical: false,
                    text: format!("R {:.2} mm", radius),
                });
            }
            HlrGeometricFeature::Ellipse {
                center,
                radius_x,
                radius_y,
                ..
            } => {
                let cx_c = cx + (center[0] - v_center[0]) * scale;
                let cy_c = cy + (center[1] - v_center[1]) * scale;

                let angle = 40.0f32.to_radians();
                let end_pt = [
                    cx_c + radius_x * scale * angle.cos(),
                    cy_c + radius_y * scale * angle.sin(),
                ];
                let leader_bend = [end_pt[0] + 6.0, end_pt[1] + 4.0];

                dims.push(DimensionAnnotation {
                    start: [cx_c, cy_c],
                    end: end_pt,
                    line_pos: leader_bend,
                    is_vertical: false,
                    text: format!("Rx {:.2} / Ry {:.2} mm", radius_x, radius_y),
                });
            }
            HlrGeometricFeature::Angle {
                vertex,
                arm1_end,
                angle_deg,
                ..
            } => {
                let v_x = cx + (vertex[0] - v_center[0]) * scale;
                let v_y = cy + (vertex[1] - v_center[1]) * scale;
                let a1_x = cx + (arm1_end[0] - v_center[0]) * scale;
                let a1_y = cy + (arm1_end[1] - v_center[1]) * scale;

                let mid_x = (v_x + a1_x) * 0.5;
                let mid_y = (v_y + a1_y) * 0.5 + 4.0;

                dims.push(DimensionAnnotation {
                    start: [v_x, v_y],
                    end: [a1_x, a1_y],
                    line_pos: [mid_x, mid_y],
                    is_vertical: false,
                    text: format!("{:.1}°", angle_deg),
                });
            }
        }
    }
}

/// Memilih skala standar terdekat dengan resolusi tinggi (mis. 1:1, 1:1.5, 1:2, 1:2.5, 1:3, 1:4, 1:5, 1:7.5, 1:10, 2:1, 5:1).
fn pick_standard_scale(raw_scale: f32) -> f32 {
    let standard_scales = [
        0.01, 0.02, 0.025, 0.04, 0.05, 0.0667, 0.075, 0.1, 0.125, 0.15, 0.2, 0.25, 0.333, 0.4,
        0.5, 0.667, 0.75, 1.0, 1.25, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 10.0,
    ];

    let mut best_scale = 1.0;
    let mut min_diff = f32::MAX;

    for &s in &standard_scales {
        if s <= raw_scale {
            let diff = (raw_scale - s).abs();
            if diff < min_diff {
                min_diff = diff;
                best_scale = s;
            }
        }
    }

    // Jika raw_scale lebih kecil dari batas terkecil
    if raw_scale < 0.01 {
        return raw_scale;
    }

    best_scale
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

/// Helper clamp aman yang menangani kasus ketika batas min > max tanpa panic.
fn clamp_safe(val: f32, a: f32, b: f32) -> f32 {
    let min = a.min(b);
    let max = a.max(b);
    val.clamp(min, max)
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
        };

        HlrDrawing {
            front: make_view(
                ProjectedViewKind::Front,
                w,
                h,
                vec![
                    HlrGeometricFeature::Circle { center: [w * 0.5, h * 0.5], radius: 25.0 },
                    HlrGeometricFeature::Angle { vertex: [0.0, 0.0], arm1_end: [20.0, 0.0], arm2_end: [20.0, 20.0], angle_deg: 45.0 },
                ],
            ),
            top: make_view(
                ProjectedViewKind::Top,
                w,
                d,
                vec![
                    HlrGeometricFeature::Arc { center: [w * 0.5, d * 0.5], radius: 15.0, start_angle: 0.0, end_angle: 3.14159 },
                    HlrGeometricFeature::Ellipse { center: [w * 0.25, d * 0.5], radius_x: 30.0, radius_y: 12.0, rotation: 0.0 },
                ],
            ),
            right: make_view(ProjectedViewKind::Right, d, h, Vec::new()),
            isometric: make_view(ProjectedViewKind::Isometric, w * 0.9, (h + d) * 0.8, Vec::new()),
            model_bbox_min: [0.0, 0.0, 0.0],
            model_bbox_max: [w, d, h],
        }
    }

    #[test]
    fn test_auto_layout_adaptive_scaling_and_fill_factor() {
        // Model 790 x 329.2 x 130.53 mm sesuai kasus pengguna
        let drawing = make_test_drawing(790.0, 130.53, 329.2);

        // 1. A4 Landscape (297 x 210)
        let sheet_a4 = DrawingSheet::new(drawing.clone(), PaperSize::A4Landscape);
        assert!(sheet_a4.scale > 0.0, "Skala A4 harus bernilai positif");
        assert_eq!(sheet_a4.view_placements.len(), 4, "Harus ada 4 tampak proyeksi");

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
        let has_radius = sheet.auto_dimensions.iter().any(|d| d.text.starts_with("R "));
        let has_diameter = sheet.auto_dimensions.iter().any(|d| d.text.starts_with("Ø "));
        let has_ellipse = sheet.auto_dimensions.iter().any(|d| d.text.starts_with("Rx "));
        let has_angle = sheet.auto_dimensions.iter().any(|d| d.text.ends_with('°'));

        assert!(has_diameter, "Dimensi diameter lingkaran harus muncul");
        assert!(has_radius, "Dimensi radius busur R harus muncul");
        assert!(has_ellipse, "Dimensi radius ellips Rx/Ry harus muncul");
        assert!(has_angle, "Dimensi sudut ° harus muncul");
    }
}
