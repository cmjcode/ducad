//! Palet Swatch Warna Dokumen & Warna Terakhir Dipakai (M2.7).

use ducad_sketch::Rgba;
use egui::{Sense, Ui, Vec2};

/// Manajer swatch warna: warna baru dipakai (recent) dan warna palet dokumen.
#[derive(Debug, Clone, PartialEq)]
pub struct SwatchManager {
    /// Daftar warna terakhir dipakai, dibatasi maksimal 8.
    pub recent: Vec<Rgba>,
}

impl Default for SwatchManager {
    fn default() -> Self {
        Self {
            recent: Vec::with_capacity(8),
        }
    }
}

impl SwatchManager {
    pub const MAX_RECENT: usize = 8;

    pub fn new() -> Self {
        Self::default()
    }

    /// Menambahkan warna ke daftar recent.
    /// Jika warna sudah ada, posisinya dipindah ke paling depan.
    /// Daftar dipotong sehingga maksimal 8 warna (`MAX_RECENT`).
    pub fn push_recent(&mut self, color: Rgba) {
        if let Some(pos) = self.recent.iter().position(|&c| c == color) {
            self.recent.remove(pos);
        }
        self.recent.insert(0, color);
        if self.recent.len() > Self::MAX_RECENT {
            self.recent.truncate(Self::MAX_RECENT);
        }
    }

    /// Render baris swatch warna (misal swatch dokumen atau recent).
    /// Mengembalikan `Option<Rgba>` jika salah satu swatch diklik.
    pub fn show_swatch_row(
        ui: &mut Ui,
        swatches: &[Rgba],
        size: Vec2,
    ) -> Option<Rgba> {
        let mut clicked = None;
        ui.horizontal_wrapped(|ui| {
            for &color in swatches {
                let (rect, response) = ui.allocate_exact_size(size, Sense::click());
                let egui_col = egui::Color32::from_rgba_unmultiplied(
                    (color.0[0] * 255.0) as u8,
                    (color.0[1] * 255.0) as u8,
                    (color.0[2] * 255.0) as u8,
                    (color.0[3] * 255.0) as u8,
                );
                ui.painter().rect_filled(rect, 3.0, egui_col);
                ui.painter().rect_stroke(
                    rect,
                    3.0,
                    egui::Stroke::new(1.0, egui::Color32::from_white_alpha(40)),
                    egui::StrokeKind::Inside,
                );
                if response.clicked() {
                    clicked = Some(color);
                }
            }
        });
        clicked
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swatch_recent_is_capped_at_8() {
        let mut mgr = SwatchManager::new();
        for i in 0..15 {
            let col = Rgba([i as f32 / 20.0, 0.5, 0.5, 1.0]);
            mgr.push_recent(col);
        }
        assert_eq!(mgr.recent.len(), 8);

        // Warna terakhir yang di-push (i=14) harus berada di indeks 0
        let last_pushed = Rgba([14.0 / 20.0, 0.5, 0.5, 1.0]);
        assert_eq!(mgr.recent[0], last_pushed);

        // Menambahkan kembali warna yang sudah ada membawanya ke depan tanpa menambah panjang
        mgr.push_recent(mgr.recent[3]);
        assert_eq!(mgr.recent.len(), 8);
    }
}
