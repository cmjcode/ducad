use crate::brush::Brush;
use crate::filter::OneEuro;
use crate::stroke::InkPoint;

/// Pembangun coretan tinta kontinu dengan pemfilteran 1€ dan pengabaian titik redundan.
#[derive(Debug, Clone)]
pub struct StrokeBuilder {
    filter_x: OneEuro,
    filter_y: OneEuro,
    last_point: Option<InkPoint>,
    min_dist: f32,
    history: Vec<InkPoint>,
}

impl StrokeBuilder {
    /// Membuat StrokeBuilder baru yang disesuaikan dengan konfigurasi kuas.
    pub fn new(brush: &Brush) -> Self {
        Self {
            filter_x: OneEuro::from_smoothing(brush.smoothing),
            filter_y: OneEuro::from_smoothing(brush.smoothing),
            last_point: None,
            min_dist: 0.05, // 0.05 mm
            history: Vec::new(),
        }
    }

    /// Menambahkan titik input mentah. Mengembalikan `Some(point)` bila titik diterima
    /// (jarak ≥ min_dist 0,05 mm ATAU perubahan tekanan > 0,1), atau `None` bila dibuang.
    pub fn push(&mut self, raw: InkPoint) -> Option<InkPoint> {
        if let Some(last) = self.last_point {
            let dist = ((raw.x - last.x).powi(2) + (raw.y - last.y).powi(2)).sqrt();
            let delta_pressure = (raw.pressure - last.pressure).abs();
            if dist < self.min_dist && delta_pressure <= 0.1 {
                return None;
            }
        }

        let t_s = raw.t_ms as f32 / 1000.0;
        let fx = self.filter_x.filter(raw.x, t_s);
        let fy = self.filter_y.filter(raw.y, t_s);

        let pt = InkPoint::new(fx, fy, raw.pressure, raw.tilt, raw.t_ms);
        self.last_point = Some(pt);
        self.history.push(pt);
        Some(pt)
    }

    /// Memprediksi 1 titik ke depan dari kecepatan 3 titik terakhir, dibatasi 8 ms & 2 mm.
    /// Mengembalikan `None` bila riwayat coretan < 3 titik.
    pub fn predict(&self, now_ms: u32) -> Option<InkPoint> {
        let n = self.history.len();
        if n < 3 {
            return None;
        }

        let p0 = self.history[n - 3];
        let p1 = self.history[n - 2];
        let p2 = self.history[n - 1];

        let dt01 = ((p1.t_ms.saturating_sub(p0.t_ms)) as f32 / 1000.0).max(1e-4);
        let dt12 = ((p2.t_ms.saturating_sub(p1.t_ms)) as f32 / 1000.0).max(1e-4);

        let v01 = (p1.pos() - p0.pos()) / dt01;
        let v12 = (p2.pos() - p1.pos()) / dt12;
        let v = (v01 + v12) * 0.5;

        // Batasi estimasi waktu ke depan: 1 s/d 8 ms
        let dt_ms = now_ms.saturating_sub(p2.t_ms).clamp(1, 8);
        let dt_s = dt_ms as f32 / 1000.0;

        let mut disp = v * dt_s;
        let dist = disp.length();
        // Batasi pergeseran prediksi maksimal 2.0 mm
        if dist > 2.0 {
            disp = disp.normalize() * 2.0;
        }

        let pred_pos = p2.pos() + disp;
        Some(InkPoint::new(
            pred_pos.x,
            pred_pos.y,
            p2.pressure,
            p2.tilt,
            p2.t_ms + dt_ms,
        ))
    }

    /// Mendapatkan referensi ke kumpulan titik yang sudah difilter sejauh ini.
    pub fn points(&self) -> &[InkPoint] {
        &self.history
    }

    /// Mendapatkan titik terakhir yang telah difilter jika ada.
    pub fn last_point(&self) -> Option<InkPoint> {
        self.last_point
    }

    /// Menyelesaikan proses pengumpulan titik dan mengembalikan seluruh titik coretan yang telah difilter.
    pub fn finish(self) -> Vec<InkPoint> {
        self.history
    }
}
