//! State dan konfigurasi alat mode Sketsa (Ink).

use ducad_ink::brush::BrushId;
use ducad_ink::eraser::EraseMode;
use ducad_ink::lasso::LassoMode;
use ducad_ink::predict::StrokeBuilder;
use ducad_ink::stroke::InkPoint;
use ducad_sketch::layer::LayerId;
use ducad_sketch::style::Rgba;
use glam::Vec2;

/// Pilihan alat dalam Mode Sketsa Tinta (Concepts-like).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InkTool {
    /// Menggambar dengan kuas bertekanan.
    #[default]
    Brush,
    /// Menghapus coretan (seluruhnya atau parsial).
    Eraser,
    /// Memilih coretan dengan poligon lasso.
    Lasso,
    /// Deformasi lokal coretan dengan radius jatuh (falloff) smoothstep.
    Nudge,
    /// Memotong coretan menjadi potongan terpisah dengan garis potong.
    Slice,
    /// Menggeser/memindahkan coretan terpilih.
    Move,
    /// Mengubah warna coretan terpilih.
    Color,
}

/// Status interaksi dan konfigurasi aktif mode Sketsa.
#[derive(Debug, Clone)]
pub struct InkToolState {
    /// Alat yang sedang aktif.
    pub active_tool: InkTool,
    /// ID Kuas aktif dalam `InkDoc::brushes`.
    pub active_brush: Option<BrushId>,
    /// Warna tinta aktif.
    pub active_color: Rgba,
    /// Layer tinta aktif (harus berjenis `LayerKind::Ink`).
    pub active_layer: Option<LayerId>,
    /// Mode penghapus (WholeStroke atau Partial).
    pub erase_mode: EraseMode,
    /// Mode pemilihan lasso (Contain atau Intersect).
    pub lasso_mode: LassoMode,
    /// Radius pengaruh alat Nudge (mm).
    pub nudge_radius_mm: f32,
    /// Opsi kuas "Bentuk Pintar" (smart shape recognize).
    pub smart_shape: bool,

    // --- State dinamis interaksi aktif ---
    /// Pembangun coretan saat sedang menggambar.
    pub builder: Option<StrokeBuilder>,
    /// Titik-titik coretan aktif yang sudah difilter untuk rendering real-time.
    pub active_points: Vec<InkPoint>,
    /// Titik prediksi 1 langkah ke depan untuk rendering latensi ultra-rendah.
    pub predicted_point: Option<InkPoint>,
    /// ID sentuhan egui yang sedang aktif.
    pub active_touch_id: Option<egui::TouchId>,
    /// Waktu mulai coretan aktif.
    pub stroke_start_instant: Option<std::time::Instant>,
    /// ID stroke tinta yang sedang terpilih (mis. via Lasso).
    pub selected_stroke_ids: Vec<u64>,
    /// Poligon titik untuk lasso atau lintasan penghapus.
    pub lasso_polygon: Vec<Vec2>,
    /// Garis potong (awal, akhir) untuk alat Slice.
    pub slice_line: Option<(Vec2, Vec2)>,
    /// Titik pusat awal tarikan untuk alat Nudge atau Move.
    pub nudge_center: Option<Vec2>,
}

impl Default for InkToolState {
    fn default() -> Self {
        Self {
            active_tool: InkTool::Brush,
            active_brush: None,
            active_color: Rgba([0.12, 0.12, 0.12, 1.0]),
            active_layer: None,
            erase_mode: EraseMode::WholeStroke,
            lasso_mode: LassoMode::Intersect,
            nudge_radius_mm: 10.0,
            smart_shape: false,
            builder: None,
            active_points: Vec::new(),
            predicted_point: None,
            active_touch_id: None,
            stroke_start_instant: None,
            selected_stroke_ids: Vec::new(),
            lasso_polygon: Vec::new(),
            slice_line: None,
            nudge_center: None,
        }
    }
}
