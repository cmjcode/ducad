//! Konfigurasi dan Penanganan Touch Design DUCAD (Apple Pencil & Sentuhan Jari).
//!
//! Mendukung alur kerja CAD presisi gaya Shapr3D di iPad:
//! - Mode Desain Hibrida (Apple Pencil & Jari keduanya bisa menggambar/desain)
//! - Mode Desain Khusus Pencil (Jari khusus untuk Navigasi Kanvas 1 & 2 jari, mencegah goresan tak sengaja / Palm Rejection)
//! - Mode Desain Sentuh Jari (Touch-Optimized dengan target sentuh 44pt Apple HIG dan toleransi snapping lebih besar)

use egui_icons::icons::{ICON_GESTURE, ICON_STYLUS, ICON_TOUCH_APP};

/// Mode Desain Sentuh di iPad dan Layar Sentuh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TouchDesignMode {
    /// Apple Pencil & Jari keduanya dapat mendesain/menggambar sketsa dan manipulasi model 3D.
    /// Navigasi kanvas (pan, orbit, zoom) menggunakan gesture multi-touch 2 jari.
    #[default]
    PencilAndFinger,

    /// Hanya Apple Pencil yang dapat menggambar sketsa, memilih objek, dan memanipulasi gizmo.
    /// Sentuhan 1 jari otomatis melakukan navigasi kanvas (orbit/pan), sementara 2 jari melakukan zoom/pan.
    /// Mencegah goresan yang tidak disengaja saat telapak tangan menyentuh layar (Palm Rejection).
    PencilOnly,

    /// Desain Sentuh Jari (Touch-Optimized).
    /// Mengoptimalkan antarmuka untuk menggambar dengan jari tanpa stylus:
    /// target sentuh tombol diperbesar ke standar 44pt Apple HIG dan toleransi snapping diperluas (24px).
    FingerDesign,
}

impl TouchDesignMode {
    /// Label ramah pengguna dengan ikon representatif.
    pub fn label(self) -> &'static str {
        match self {
            Self::PencilAndFinger => "✏️+👆 Pencil & Jari",
            Self::PencilOnly => "✏️ Hanya Apple Pencil",
            Self::FingerDesign => "👆 Sentuh Jari (44pt)",
        }
    }

    /// Ikon ringkas untuk topbar / pill status.
    pub fn icon(self) -> &'static str {
        match self {
            Self::PencilAndFinger => "✏️+👆",
            Self::PencilOnly => "✏️",
            Self::FingerDesign => "👆",
        }
    }

    /// Ikon Material untuk tombol ikon di header.
    ///
    /// Sengaja terpisah dari [`Self::icon`] yang memakai emoji: emoji dirender
    /// dari font fallback dengan metrik berbeda, sehingga tombolnya jadi lebih
    /// lebar dari tombol header lain pada ukuran font yang sama — terukur 35.5
    /// px untuk emoji tunggal dan 64.2 px untuk `"✏️+👆"`, vs 35.0 px untuk
    /// Material Icon. Emoji tetap dipakai untuk teks menu, yang tidak terikat
    /// grid tombol header.
    pub fn material_icon(self) -> &'static str {
        match self {
            Self::PencilAndFinger => ICON_GESTURE.codepoint,
            Self::PencilOnly => ICON_STYLUS.codepoint,
            Self::FingerDesign => ICON_TOUCH_APP.codepoint,
        }
    }

    /// Penjelasan mode interaksi.
    pub fn description(self) -> &'static str {
        match self {
            Self::PencilAndFinger => "Apple Pencil & Jari keduanya bisa mendesain; 2 jari untuk navigasi kanvas.",
            Self::PencilOnly => "Khusus Apple Pencil untuk menggambar; sentuhan 1 jari memutar/menggeser kanvas (Palm Rejection).",
            Self::FingerDesign => "Dioptimalkan untuk jari: target sentuh besar (44pt HIG) dan snapping lebih mudah.",
        }
    }

    /// Mengembalikan mode berikutnya saat tombol toggle di-klik.
    pub fn next(self) -> Self {
        match self {
            Self::PencilAndFinger => Self::PencilOnly,
            Self::PencilOnly => Self::FingerDesign,
            Self::FingerDesign => Self::PencilAndFinger,
        }
    }
}

/// Konfigurasi lengkap untuk Touch Design & Apple Pencil.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TouchDesignConfig {
    /// Mode aktif untuk sentuhan dan Pencil.
    pub mode: TouchDesignMode,
    /// Tinggi minimum widget interaktif (28.0 untuk desktop, 44.0 untuk Apple HIG).
    pub touch_target_size: f32,
    /// Toleransi jarak snapping saat mendesain dengan jari (dalam pixel layar).
    pub finger_snap_distance: f64,
    /// Toleransi jarak snapping saat mendesain dengan Apple Pencil (dalam pixel layar).
    pub pencil_snap_distance: f64,
    /// Apakah filter penolakan telapak tangan (Palm Rejection) aktif.
    pub palm_rejection: bool,
    /// Sensitivitas tekanan Apple Pencil untuk goresan/feedback.
    pub pressure_enabled: bool,
}

impl Default for TouchDesignConfig {
    fn default() -> Self {
        Self {
            mode: TouchDesignMode::PencilAndFinger,
            touch_target_size: 44.0, // Standar Apple HIG untuk iPad
            finger_snap_distance: 24.0,
            pencil_snap_distance: 12.0,
            palm_rejection: true,
            pressure_enabled: true,
        }
    }
}

impl TouchDesignConfig {
    /// Toleransi pixel yang efektif untuk snapping berdasarkan mode aktif.
    pub fn effective_pixel_tolerance(&self) -> f64 {
        match self.mode {
            TouchDesignMode::FingerDesign => self.finger_snap_distance,
            TouchDesignMode::PencilOnly => self.pencil_snap_distance,
            TouchDesignMode::PencilAndFinger => (self.finger_snap_distance + self.pencil_snap_distance) * 0.5,
        }
    }

    /// Apakah sentuhan jari diizinkan untuk membuat entitas desain/sketsa.
    pub fn allows_finger_design(&self) -> bool {
        self.mode != TouchDesignMode::PencilOnly
    }

    /// Apakah sentuhan 1 jari dialihkan untuk navigasi kanvas (orbit/pan) alih-alih menggambar.
    pub fn single_finger_navigates(&self) -> bool {
        self.mode == TouchDesignMode::PencilOnly
    }

    /// Mengubah mode desain dan menyesuaikan ukuran target sentuh otomatis.
    pub fn set_mode(&mut self, new_mode: TouchDesignMode) {
        self.mode = new_mode;
        match new_mode {
            TouchDesignMode::FingerDesign => {
                self.touch_target_size = 44.0;
            }
            TouchDesignMode::PencilOnly => {
                self.touch_target_size = 36.0;
            }
            TouchDesignMode::PencilAndFinger => {
                self.touch_target_size = 40.0;
            }
        }
    }

    /// Apakah sedang dalam mode khusus Apple Pencil.
    pub fn is_pencil_only(&self) -> bool {
        self.mode == TouchDesignMode::PencilOnly
    }

    /// Apakah sedang dalam mode desain sentuh jari (44pt).
    pub fn is_finger_design(&self) -> bool {
        self.mode == TouchDesignMode::FingerDesign
    }

    /// Apakah sedang dalam mode hibrida Pencil & Jari.
    pub fn is_hybrid(&self) -> bool {
        self.mode == TouchDesignMode::PencilAndFinger
    }

    /// Beralih ke mode berikutnya dan memperbarui ukuran target sentuh.
    pub fn toggle_mode(&mut self) -> TouchDesignMode {
        let next_mode = self.mode.next();
        self.set_mode(next_mode);
        next_mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_touch_design_mode_cycling() {
        let mode = TouchDesignMode::PencilAndFinger;
        assert_eq!(mode.next(), TouchDesignMode::PencilOnly);
        assert_eq!(mode.next().next(), TouchDesignMode::FingerDesign);
        assert_eq!(mode.next().next().next(), TouchDesignMode::PencilAndFinger);
    }

    #[test]
    fn test_touch_design_mode_labels_and_icons() {
        let m1 = TouchDesignMode::PencilAndFinger;
        let m2 = TouchDesignMode::PencilOnly;
        let m3 = TouchDesignMode::FingerDesign;

        assert!(!m1.label().is_empty());
        assert!(!m2.label().is_empty());
        assert!(!m3.label().is_empty());

        assert!(!m1.icon().is_empty());
        assert!(!m2.icon().is_empty());
        assert!(!m3.icon().is_empty());

        assert!(!m1.description().is_empty());
        assert!(!m2.description().is_empty());
        assert!(!m3.description().is_empty());
    }

    #[test]
    fn test_touch_design_config_defaults() {
        let cfg = TouchDesignConfig::default();
        assert_eq!(cfg.mode, TouchDesignMode::PencilAndFinger);
        assert_eq!(cfg.touch_target_size, 44.0);
        assert_eq!(cfg.finger_snap_distance, 24.0);
        assert_eq!(cfg.pencil_snap_distance, 12.0);
        assert!(cfg.palm_rejection);
        assert!(cfg.pressure_enabled);
        assert!(cfg.allows_finger_design());
        assert!(!cfg.single_finger_navigates());
        assert_eq!(cfg.effective_pixel_tolerance(), 18.0);
    }

    #[test]
    fn test_touch_design_mode_switching_and_tolerances() {
        let mut cfg = TouchDesignConfig::default();

        // Switch to PencilOnly
        cfg.set_mode(TouchDesignMode::PencilOnly);
        assert!(cfg.is_pencil_only());
        assert!(!cfg.allows_finger_design());
        assert!(cfg.single_finger_navigates());
        assert_eq!(cfg.touch_target_size, 36.0);
        assert_eq!(cfg.effective_pixel_tolerance(), 12.0);

        // Switch to FingerDesign
        cfg.set_mode(TouchDesignMode::FingerDesign);
        assert!(cfg.is_finger_design());
        assert!(cfg.allows_finger_design());
        assert!(!cfg.single_finger_navigates());
        assert_eq!(cfg.touch_target_size, 44.0);
        assert_eq!(cfg.effective_pixel_tolerance(), 24.0);

        // Toggle mode cycling
        let cycled = cfg.toggle_mode();
        assert_eq!(cycled, TouchDesignMode::PencilAndFinger);
        assert!(cfg.is_hybrid());
        assert_eq!(cfg.touch_target_size, 40.0);
    }
}

