//! Struktur data dan tata letak Lembar Kerja Gambar Teknik 2D (Engineering Drawing Sheet).
//!
//! Menyediakan spesifikasi ukuran kertas standar (A4/A3), bingkai ISO dengan grid zona referensi,
//! kepala gambar (title block), tata letak multi-tampak (Front, Top, Right, Isometric),
//! dan anotasi dimensi otomatis.

use ducad_kernel::{HlrDrawing, ProjectedViewKind};
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
            project_title: "KOMPONEN MEKANIKAL".to_string(),
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
        };

        sheet.auto_layout();
        sheet
    }

    /// Menghitung tata letak posisi tampak (Front, Top, Right, Isometric) dan skala yang pas dengan kertas.
    pub fn auto_layout(&mut self) {
        let (paper_w, paper_h) = self.paper_size.dimensions_mm();

        // Area gambar yang dapat digunakan (dikurangi margin tepi dan title block di kanan bawah)
        let left_margin = 20.0; // Margin kiri lebih lebar untuk jilid / binder
        let right_margin = 10.0;
        let top_margin = 10.0;
        let bottom_margin = 10.0;
        let _title_block_w = 140.0;
        let _title_block_h = 45.0;

        let usable_w = paper_w - left_margin - right_margin;
        let usable_h = paper_h - top_margin - bottom_margin;

        // Hitung bounding 2D tampak
        let front_sz = self.drawing.front.size_2d();
        let top_sz = self.drawing.top.size_2d();
        let right_sz = self.drawing.right.size_2d();
        let iso_sz = self.drawing.isometric.size_2d();

        // Total lebar tampak gabungan (Front + Right) + spasi antar tampak (20mm)
        let total_w = front_sz[0] + right_sz[0] + 30.0;
        let total_h = front_sz[1] + top_sz[1] + 30.0;

        // Tentukan skala yang pas (auto scale factor)
        let max_w = (usable_w - 40.0).max(50.0);
        let max_h = (usable_h - 40.0).max(50.0);

        let scale_w = max_w / total_w.max(1.0);
        let scale_h = max_h / total_h.max(1.0);
        let auto_scale = (scale_w.min(scale_h) * 0.85).min(5.0);

        // Pilih skala standar terdekat (mis. 1:1, 1:2, 1:5, 2:1)
        self.scale = pick_standard_scale(auto_scale);
        self.title_block.scale = format_scale_ratio(self.scale);

        let s = self.scale;

        // Tata letak 3rd Angle Projection:
        // Top View di atas Front View
        // Right View di sebelah kanan Front View
        // Isometric View di pojok kanan atas
        let front_center_x = left_margin + 20.0 + (front_sz[0] * s * 0.5);
        let front_center_y = bottom_margin + 20.0 + (front_sz[1] * s * 0.5);

        let top_center_x = front_center_x;
        let top_center_y = front_center_y + (front_sz[1] * s * 0.5) + (top_sz[1] * s * 0.5) + 25.0;

        let right_center_x = front_center_x + (front_sz[0] * s * 0.5) + (right_sz[0] * s * 0.5) + 25.0;
        let right_center_y = front_center_y;

        let iso_center_x = paper_w - right_margin - (iso_sz[0] * s * 0.5) - 20.0;
        let iso_center_y = paper_h - top_margin - (iso_sz[1] * s * 0.5) - 20.0;

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

        // Buat dimensi otomatis untuk tampak depan dan samping
        self.generate_auto_dimensions();
    }

    /// Membuat anotasi dimensi pembatas (Overall Length, Width, Height) secara otomatis.
    pub fn generate_auto_dimensions(&mut self) {
        self.auto_dimensions.clear();
        let s = self.scale;

        // 1. Dimensi Panjang & Tinggi pada Tampak Depan
        if let Some(front_plc) = self.view_placements.iter().find(|p| p.kind == ProjectedViewKind::Front) {
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
                let dim_y = y_min - 12.0;
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
                let dim_x = x_min - 12.0;
                self.auto_dimensions.push(DimensionAnnotation {
                    start: [x_min, y_min],
                    end: [x_min, y_max],
                    line_pos: [dim_x, (y_min + y_max) * 0.5],
                    is_vertical: true,
                    text: format!("{:.2} mm", height_mm),
                });
            }
        }

        // 2. Dimensi Lebar / Tebal pada Tampak Samping Kanan
        if let Some(right_plc) = self.view_placements.iter().find(|p| p.kind == ProjectedViewKind::Right) {
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
                let dim_y = y_min - 12.0;
                self.auto_dimensions.push(DimensionAnnotation {
                    start: [x_min, y_min],
                    end: [x_max, y_min],
                    line_pos: [(x_min + x_max) * 0.5, dim_y],
                    is_vertical: false,
                    text: format!("{:.2} mm", width_mm),
                });
            }
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

/// Memilih skala standar terdekat (mis. 1:1, 1:2, 1:5, 1:10, 2:1, 5:1).
fn pick_standard_scale(raw_scale: f32) -> f32 {
    let standard_scales = [
        0.05, 0.1, 0.2, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 2.5, 5.0, 10.0,
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

    best_scale
}

fn format_scale_ratio(scale: f32) -> String {
    if (scale - 1.0).abs() < 1e-4 {
        "1:1".to_string()
    } else if scale < 1.0 {
        let denom = (1.0 / scale).round() as u32;
        format!("1:{denom}")
    } else {
        let num = scale.round() as u32;
        format!("{num}:1")
    }
}
