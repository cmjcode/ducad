//! Radial menu gaya Shapr3D — long-press (jari/Apple Pencil/mouse) memicu
//! roda pilihan tool di sekitar titik tekan, drag ke arah slice lalu
//! lepas untuk memilih. Ini pelengkap toolbar untuk sentuh: menu muncul
//! DI BAWAH jari (tidak perlu menjangkau tepi layar), dan seluruh gesture
//! satu-tangan (tekan tahan → geser → lepas), pola interaksi yang jauh
//! lebih ramah iPad daripada toolbar linear.
//!
//! `RadialMenu` sendiri tidak tahu apa isi slice-nya (tool, aksi, dst) —
//! caller (`cadraw-app`) yang memutuskan kapan `open_at` dipanggil (deteksi
//! long-press ada di sisi caller, karena butuh tahu state drag/klik tool
//! yang sedang aktif) dan apa arti tiap index slice yang dikembalikan.

use egui::{Color32, Pos2, Stroke, Vec2};
use std::f32::consts::TAU;

const OUTER_RADIUS: f32 = 120.0;
/// Zona mati di tengah: lepas jari/mouse di sini = batal (radial menu
/// selalu punya "jalan keluar tanpa memilih apa-apa" tanpa perlu Esc).
const INNER_RADIUS: f32 = 34.0;
/// Sedikit lebih longgar dari OUTER_RADIUS supaya jari yang sedikit
/// "kelewatan" saat menggeser masih terhitung memilih slice terluar,
/// bukan langsung batal begitu keluar lingkaran gambar.
const CANCEL_RADIUS: f32 = OUTER_RADIUS * 1.5;

#[derive(Default)]
pub struct RadialMenu {
    open: bool,
    center: Pos2,
}

impl RadialMenu {
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open_at(&mut self, center: Pos2) {
        self.open = true;
        self.center = center;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    /// Gambar radial menu & proses input. Panggil setiap frame selama
    /// terbuka (biasanya sepanjang drag dari titik long-press sampai
    /// pointer dilepas). `items`: label tiap slice, ditata melingkar mulai
    /// dari atas (jam 12) searah jarum jam.
    ///
    /// Return `Some(index)` saat primary pointer dilepas di atas sebuah
    /// slice (di luar zona mati tengah); `None` kalau menu masih terbuka
    /// ATAU baru saja dibatalkan (lepas di zona mati/di luar radius/Esc) —
    /// caller cukup cek `is_open()` sesudahnya untuk membedakan "masih
    /// terbuka" dari "baru ditutup tanpa pilihan".
    pub fn show(&mut self, ctx: &egui::Context, items: &[&str]) -> Option<usize> {
        if !self.open || items.is_empty() {
            return None;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.close();
            return None;
        }

        let pointer = ctx.input(|i| i.pointer.hover_pos());
        let released = ctx.input(|i| i.pointer.primary_released());
        let hovered_index = pointer.and_then(|p| slice_at(self.center, p, items.len()));

        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("cadraw-radial-menu"),
        ));
        paint_radial(&painter, self.center, items, hovered_index);

        if released {
            self.close();
            return hovered_index;
        }
        None
    }
}

/// Sudut dari `center` ke `p`, 0 = atas (jam 12), bertambah searah jarum
/// jam. Konvensi ini (bukan atan2 matematis biasa) supaya slice pertama
/// alami muncul di atas, posisi paling mudah dijangkau ibu jari saat
/// menu dibuka dari titik tekan di tengah layar.
fn angle_from_top(delta: Vec2) -> f32 {
    delta.x.atan2(-delta.y).rem_euclid(TAU)
}

fn point_at_angle(center: Pos2, angle: f32, radius: f32) -> Pos2 {
    center + Vec2::new(angle.sin(), -angle.cos()) * radius
}

fn slice_at(center: Pos2, p: Pos2, count: usize) -> Option<usize> {
    let delta = p - center;
    let dist = delta.length();
    if !(INNER_RADIUS..=CANCEL_RADIUS).contains(&dist) {
        return None;
    }
    let slice = TAU / count as f32;
    let idx = (angle_from_top(delta) / slice) as usize;
    Some(idx.min(count - 1))
}

fn paint_radial(painter: &egui::Painter, center: Pos2, items: &[&str], hovered: Option<usize>) {
    let count = items.len();
    let slice = TAU / count as f32;

    painter.circle_stroke(
        center,
        INNER_RADIUS,
        Stroke::new(1.5, Color32::from_gray(200)),
    );

    for (i, label) in items.iter().enumerate() {
        let mid_angle = i as f32 * slice + slice * 0.5;
        let mid = point_at_angle(center, mid_angle, (INNER_RADIUS + OUTER_RADIUS) * 0.5);
        let bg = if hovered == Some(i) {
            Color32::from_rgb(90, 140, 220)
        } else {
            Color32::from_rgba_premultiplied(40, 40, 40, 230)
        };
        painter.circle_filled(mid, 28.0, bg);
        painter.text(
            mid,
            egui::Align2::CENTER_CENTER,
            *label,
            egui::FontId::proportional(13.0),
            Color32::WHITE,
        );
    }
}
