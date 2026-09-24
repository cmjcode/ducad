use ducad_sketch::layer::LayerId;
use ducad_sketch::style::Rgba;
use glam::Vec2;
use serde::{Deserialize, Serialize};

use crate::brush::BrushId;

/// Satu titik tangkapan tinta dengan koordinat bidang sketsa (mm),
/// tekanan (0..=1), kemiringan stylus (tilt dalam radian), dan waktu relatif (t_ms).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct InkPoint {
    pub x: f32,
    pub y: f32,
    /// Tekanan pena 0.0 s/d 1.0 (default 0.5 jika tanpa sensor tekanan).
    pub pressure: f32,
    /// Kemiringan pena dalam radian (0.0 jika tegak lurus atau tanpa sensor).
    pub tilt: f32,
    /// Waktu perekaman titik dalam milidetik relatif terhadap awal coretan.
    pub t_ms: u32,
}

impl InkPoint {
    pub fn new(x: f32, y: f32, pressure: f32, tilt: f32, t_ms: u32) -> Self {
        Self {
            x,
            y,
            pressure: pressure.clamp(0.0, 1.0),
            tilt,
            t_ms,
        }
    }

    pub fn pos(&self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }
}

/// Satu coretan tinta kontinu yang sudah difilter.
///
/// Deserialisasi lewat [`StrokeData`] supaya `bbox` (tidak ikut disimpan)
/// selalu dihitung ulang saat berkas dimuat — tanpa ini hit-test, culling,
/// dan indeks spasial memakai kotak nol untuk semua coretan hasil load.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "StrokeData")]
pub struct Stroke {
    /// ID stabil dan monoton dalam dokumen.
    pub id: u64,
    /// Titik-titik sampel coretan pada koordinat bidang sketsa (mm).
    pub points: Vec<InkPoint>,
    /// Kuas yang digunakan.
    pub brush: BrushId,
    /// Warna coretan.
    pub color: Rgba,
    /// Layer pemilik coretan (harus LayerKind::Ink).
    pub layer: LayerId,
    /// Status tersembunyi (mis. setelah proses Rapikan di M5).
    #[serde(default)]
    pub hidden: bool,
    /// Bounding box (min, max) dalam mm (dihitung ulang saat pemuatan/modifikasi).
    #[serde(skip)]
    pub bbox: (Vec2, Vec2),
}

/// Bentuk tersimpan [`Stroke`] (tanpa `bbox`).
#[derive(Deserialize)]
struct StrokeData {
    id: u64,
    points: Vec<InkPoint>,
    brush: BrushId,
    color: Rgba,
    layer: LayerId,
    #[serde(default)]
    hidden: bool,
}

impl From<StrokeData> for Stroke {
    fn from(d: StrokeData) -> Self {
        let mut stroke = Stroke::new(d.id, d.points, d.brush, d.color, d.layer);
        stroke.hidden = d.hidden;
        stroke
    }
}

impl Stroke {
    pub fn new(
        id: u64,
        points: Vec<InkPoint>,
        brush: BrushId,
        color: Rgba,
        layer: LayerId,
    ) -> Self {
        let mut stroke = Self {
            id,
            points,
            brush,
            color,
            layer,
            hidden: false,
            bbox: (Vec2::ZERO, Vec2::ZERO),
        };
        stroke.recompute_bbox();
        stroke
    }

    /// Menghitung ulang batas kotak (min, max) dari titik-titik coretan.
    pub fn recompute_bbox(&mut self) {
        if self.points.is_empty() {
            self.bbox = (Vec2::ZERO, Vec2::ZERO);
            return;
        }
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;

        for p in &self.points {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }

        self.bbox = (Vec2::new(min_x, min_y), Vec2::new(max_x, max_y));
    }

    /// Total panjang poliline coretan dalam milimeter.
    pub fn length(&self) -> f32 {
        if self.points.len() < 2 {
            return 0.0;
        }
        let mut total = 0.0;
        for i in 1..self.points.len() {
            let p0 = self.points[i - 1].pos();
            let p1 = self.points[i].pos();
            total += (p1 - p0).length();
        }
        total
    }

    /// Mentransformasikan koordinat coretan menggunakan transformasi affine kurbo.
    pub fn transformed(&self, a: kurbo::Affine) -> Stroke {
        let new_points: Vec<InkPoint> = self
            .points
            .iter()
            .map(|p| {
                let kp = a * kurbo::Point::new(p.x as f64, p.y as f64);
                InkPoint {
                    x: kp.x as f32,
                    y: kp.y as f32,
                    pressure: p.pressure,
                    tilt: p.tilt,
                    t_ms: p.t_ms,
                }
            })
            .collect();

        let mut s = Stroke {
            id: self.id,
            points: new_points,
            brush: self.brush,
            color: self.color,
            layer: self.layer,
            hidden: self.hidden,
            bbox: (Vec2::ZERO, Vec2::ZERO),
        };
        s.recompute_bbox();
        s
    }
}
