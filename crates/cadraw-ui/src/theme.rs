//! Tema (terang/gelap) & gaya target-sentuh CADRAW.
//!
//! Diterapkan sekali lewat [`apply`] saat startup, lalu lagi tiap kali
//! [`ThemeMode`] berubah (toggle tema dari toolbar/command palette).

use egui::{Style, Vec2, Visuals};

/// Tinggi minimum widget interaktif (tombol, checkbox, combo box, dst),
/// mengikuti rekomendasi target sentuh 44pt Apple HIG. Dipakai sebagai
/// lantai, bukan plafon — mouse tetap nyaman dengan target lebih besar,
/// sentuh (jari/Apple Pencil di iPad) jadi andal tanpa perlu gaya terpisah
/// per platform.
pub const MIN_TOUCH_TARGET: f32 = 44.0;

/// Mode tema aplikasi. Cuma dua pilihan (bukan "ikuti sistem") karena
/// eframe/winit tidak selalu punya cara portabel membaca preferensi OS di
/// semua target (termasuk iOS nanti) — deteksi otomatis bisa ditambahkan
/// belakangan tanpa mengubah tipe ini.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    Light,
    #[default]
    Dark,
}

impl ThemeMode {
    pub fn toggled(self) -> Self {
        match self {
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::Light,
        }
    }

    /// Label tombol toggle, sudah termasuk ikon — dipakai langsung sebagai
    /// teks tombol di toolbar/command palette.
    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::Light => "☀ Terang",
            ThemeMode::Dark => "🌙 Gelap",
        }
    }

    fn visuals(self) -> Visuals {
        match self {
            ThemeMode::Dark => Visuals::dark(),
            ThemeMode::Light => Visuals::light(),
        }
    }
}

/// Terapkan tema + gaya target-sentuh ke context egui. Style dibangun dari
/// `Style::default()` (bukan style lama context) supaya idempoten dipanggil
/// berkali-kali (toggle tema) tanpa menumpuk penyesuaian dari panggilan
/// sebelumnya.
pub fn apply(ctx: &egui::Context, mode: ThemeMode) {
    let mut style = Style {
        visuals: mode.visuals(),
        ..Default::default()
    };
    // `interact_size.y` adalah lantai tinggi baris yang dipakai egui untuk
    // semua widget interaktif standar (Button, Checkbox, ComboBox,
    // SelectableLabel, dst) — satu titik pengaturan untuk seluruh app,
    // tidak perlu disentuh manual di tiap situs pemanggilan widget.
    style.spacing.interact_size.y = MIN_TOUCH_TARGET;
    style.spacing.button_padding = Vec2::new(12.0, 8.0);
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    ctx.set_style(style);
}
