//! Parameter material Liquid Glass + preset per jenis permukaan.
//!
//! Material murni data: tidak menyentuh GPU. Shader (`shaders/glass.wgsl`)
//! membaca nilai yang sama lewat `PanelUniform`, dan [`GlassMaterial::composite`]
//! menirukan bagian dalam kaca di CPU sehingga keterbacaan teks bisa dijaga tes.

use egui::Color32;

/// Mode terang/gelap kaca. Menentukan warna tint dan arah penjinakan
/// luminansi latar (gelap: latar terang diredam; terang: latar gelap diangkat).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlassMode {
    #[default]
    Dark,
    Light,
}

/// Jenis permukaan kaca. Makin "penting dibaca" isinya, makin pekat tint-nya.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlassPreset {
    /// Drawer dan panel berisi daftar/teks.
    Panel,
    /// Kapsul mengambang kecil (status, bar konteks).
    Pill,
    /// Bilah alat berisi ikon.
    Toolbar,
    /// Popup/menu: paling pekat, karena menumpuk di atas apa saja.
    Popup,
}

/// Material satu permukaan Liquid Glass.
///
/// Satuan panjang dalam *point* egui (dikali `pixels_per_point` saat dikirim
/// ke shader). Warna dalam ruang gamma sRGB, sama seperti target render egui.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlassMaterial {
    /// Mode yang menentukan arah penjinakan luminansi (lihat `luma_limit`).
    pub mode: GlassMode,
    /// Warna tint (sRGB, tanpa alpha).
    pub tint: [u8; 3],
    /// Seberapa kuat tint menutupi latar, 0 (bening) … 1 (pekat).
    pub tint_strength: f32,
    /// Keburaman latar, 0 (hampir tajam) … 1 (buram penuh).
    pub frost: f32,
    /// Pergeseran maksimum latar di tepi (point) — kekuatan lensa.
    pub refraction: f32,
    /// Lebar pita rim tempat lensa bekerja (point).
    pub rim_width: f32,
    /// Kekuatan sorotan spekular di rim, 0 … 1.
    pub specular: f32,
    /// Dispersi warna di rim sebagai fraksi dari `refraction`, 0 … 1.
    pub dispersion: f32,
    /// Saturasi latar (1 = apa adanya; > 1 = lebih hidup).
    pub saturation: f32,
    /// Batas luminansi latar sebelum ditint. Mode gelap: luminansi di atas
    /// nilai ini diredam; mode terang: berlaku pada warna yang dibalik.
    /// Inilah yang membuat teks tetap terbaca di atas latar apa pun.
    pub luma_limit: f32,
    /// Penggelapan pita rim (ketebalan kaca terlihat), 0 … 1.
    pub rim_shade: f32,
    /// Respons terhadap sentuhan/kursor (gelembung + kilau), 0 … 1.
    pub interactive: f32,
    /// Arah DATANGNYA cahaya di ruang layar (y ke bawah); kiri-atas = (-1, -1).
    pub light_dir: [f32; 2],
}

impl GlassMaterial {
    /// Preset bawaan untuk jenis permukaan dan mode tertentu.
    pub fn preset(preset: GlassPreset, mode: GlassMode) -> Self {
        // Kaca Liquid Glass nyaris bening: tint dan blur tipis. Keterbacaan
        // dijaga `luma_limit` (latar terang diredam di balik kaca), bukan
        // oleh kepekatan tint.
        let (tint, luma_limit) = match mode {
            GlassMode::Dark => ([14, 16, 20], 0.27),
            GlassMode::Light => ([248, 249, 252], 0.34),
        };
        let base = Self {
            mode,
            tint,
            tint_strength: 0.28,
            frost: 0.40,
            refraction: 44.0,
            rim_width: 34.0,
            specular: 0.85,
            dispersion: 0.35,
            saturation: 1.3,
            luma_limit,
            rim_shade: 0.16,
            interactive: 0.4,
            light_dir: [-0.6, -0.8],
        };
        match preset {
            GlassPreset::Panel => base,
            GlassPreset::Pill => Self {
                tint_strength: 0.14,
                frost: 0.18,
                refraction: 38.0,
                rim_width: 26.0,
                specular: 1.0,
                dispersion: 0.45,
                saturation: 1.4,
                rim_shade: 0.20,
                interactive: 1.0,
                ..base
            },
            GlassPreset::Toolbar => Self {
                tint_strength: 0.20,
                frost: 0.28,
                refraction: 34.0,
                rim_width: 22.0,
                specular: 0.95,
                dispersion: 0.40,
                interactive: 0.7,
                ..base
            },
            GlassPreset::Popup => Self {
                tint_strength: 0.60,
                frost: 0.85,
                refraction: 30.0,
                rim_width: 26.0,
                specular: 0.6,
                dispersion: 0.15,
                saturation: 1.1,
                rim_shade: 0.12,
                interactive: 0.0,
                ..base
            },
        }
    }

    /// Tint sebagai komponen gamma 0…1.
    pub fn tint_f32(&self) -> [f32; 3] {
        [
            self.tint[0] as f32 / 255.0,
            self.tint[1] as f32 / 255.0,
            self.tint[2] as f32 / 255.0,
        ]
    }

    /// Warna isian datar pengganti kaca saat GPU/backdrop tidak tersedia atau
    /// "Kurangi transparansi" aktif.
    ///
    /// Selalu premultiplied yang sah (RGB ≤ alpha — RGB > alpha di-blend aditif
    /// oleh egui) dan cukup pekat (alpha ≥ 200) supaya konten di belakang tidak
    /// mengganggu teks: tanpa blur, transparansi tidak punya penolong.
    pub fn fallback_fill(&self) -> Color32 {
        let alpha = (0.78 + 0.30 * self.tint_strength.clamp(0.0, 1.0)) * 255.0;
        let alpha = alpha.round().clamp(200.0, 255.0) as u8;
        Color32::from_rgba_unmultiplied(self.tint[0], self.tint[1], self.tint[2], alpha)
    }

    /// Warna bagian DALAM kaca (di luar rim) untuk latar `backdrop` yang sudah
    /// diburamkan — tiruan CPU dari langkah saturasi → batas luminansi → tint
    /// di `glass.wgsl`. Dipakai tes keterbacaan; rim (lensa, spekular) tidak
    /// ikut karena teks tidak pernah ditaruh di sana.
    pub fn composite(&self, backdrop: [f32; 3]) -> [f32; 3] {
        let mut c = backdrop.map(|v| v.clamp(0.0, 1.0));
        let gray = luma_of(c);
        for v in &mut c {
            *v = (gray + (*v - gray) * self.saturation).clamp(0.0, 1.0);
        }
        if self.mode == GlassMode::Light {
            c = c.map(|v| 1.0 - v);
        }
        let scale = (self.luma_limit / brightness(c).max(1e-4)).min(1.0);
        c = c.map(|v| v * scale);
        if self.mode == GlassMode::Light {
            c = c.map(|v| 1.0 - v);
        }
        let tint = self.tint_f32();
        let s = self.tint_strength.clamp(0.0, 1.0);
        [
            c[0] + (tint[0] - c[0]) * s,
            c[1] + (tint[1] - c[1]) * s,
            c[2] + (tint[2] - c[2]) * s,
        ]
    }
}

/// Luminansi (bobot Rec. 709) dihitung langsung pada nilai gamma — sama
/// dengan yang dilakukan shader; bukan luminansi WCAG.
fn luma_of(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// Ukuran "seberapa terang" yang dibatasi `luma_limit`: luminansi, tetapi
/// tidak kurang dari 0.9 × kanal terkuat. Tanpa suku kedua, warna jenuh
/// (hijau/kuning murni) lolos dari batas dan teks di atasnya tak terbaca.
fn brightness(c: [f32; 3]) -> f32 {
    luma_of(c).max(0.9 * c[0].max(c[1]).max(c[2]))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRESETS: [GlassPreset; 4] = [
        GlassPreset::Panel,
        GlassPreset::Pill,
        GlassPreset::Toolbar,
        GlassPreset::Popup,
    ];

    /// Luminansi relatif WCAG (sRGB → linear).
    fn wcag_luminance(c: [f32; 3]) -> f32 {
        let lin = |f: f32| {
            if f <= 0.04045 {
                f / 12.92
            } else {
                ((f + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
    }

    fn contrast(a: [f32; 3], b: [f32; 3]) -> f32 {
        let (la, lb) = (wcag_luminance(a), wcag_luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    /// Latar terburuk: delapan sudut kubus RGB + abu-abu bertingkat.
    fn backdrops() -> Vec<[f32; 3]> {
        let mut out = Vec::new();
        for r in [0.0, 1.0] {
            for g in [0.0, 1.0] {
                for b in [0.0, 1.0] {
                    out.push([r, g, b]);
                }
            }
        }
        for i in 0..=10 {
            let v = i as f32 / 10.0;
            out.push([v, v, v]);
        }
        out
    }

    #[test]
    fn fallback_fill_is_valid_premultiplied_and_opaque_enough() {
        for mode in [GlassMode::Dark, GlassMode::Light] {
            for preset in PRESETS {
                let fill = GlassMaterial::preset(preset, mode).fallback_fill();
                assert!(
                    fill.r() <= fill.a() && fill.g() <= fill.a() && fill.b() <= fill.a(),
                    "{mode:?}/{preset:?}: {fill:?} bukan premultiplied valid (aditif)"
                );
                assert!(
                    fill.a() >= 200,
                    "{mode:?}/{preset:?}: alpha {} < 200",
                    fill.a()
                );
            }
        }
    }

    #[test]
    fn popup_is_more_opaque_than_panel() {
        for mode in [GlassMode::Dark, GlassMode::Light] {
            let panel = GlassMaterial::preset(GlassPreset::Panel, mode);
            let popup = GlassMaterial::preset(GlassPreset::Popup, mode);
            assert!(popup.tint_strength > panel.tint_strength);
            assert!(popup.fallback_fill().a() > panel.fallback_fill().a());
            assert!(popup.fallback_fill().a() >= 230);
        }
    }

    /// Inti janji tema ini: transparan tetapi tidak mengganggu. Untuk latar
    /// apa pun, teks utama harus tetap ≥ 4.5:1 dan teks sekunder ≥ 3:1.
    #[test]
    fn text_stays_legible_over_any_backdrop() {
        // Warna teks tema DUCAD (theme.rs) dan teks bawaan egui mode terang.
        let cases = [
            (
                GlassMode::Dark,
                [245.0, 245.0, 247.0],
                [142.0, 142.0, 147.0],
            ),
            (GlassMode::Light, [28.0, 28.0, 30.0], [80.0, 80.0, 80.0]),
        ];
        for (mode, primary, secondary) in cases {
            let primary = primary.map(|v: f32| v / 255.0);
            let secondary = secondary.map(|v: f32| v / 255.0);
            for preset in PRESETS {
                let m = GlassMaterial::preset(preset, mode);
                for bg in backdrops() {
                    let glass = m.composite(bg);
                    let rp = contrast(primary, glass);
                    let rs = contrast(secondary, glass);
                    assert!(
                        rp >= 4.5,
                        "{mode:?}/{preset:?} latar {bg:?}: kontras teks utama {rp:.2} < 4.5"
                    );
                    assert!(
                        rs >= 3.0,
                        "{mode:?}/{preset:?} latar {bg:?}: kontras teks sekunder {rs:.2} < 3.0"
                    );
                }
            }
        }
    }

    #[test]
    fn composite_keeps_some_backdrop_visible() {
        // Kaca bukan cat: latar merah dan latar biru harus menghasilkan warna
        // berbeda (transparan), bukan satu warna tint yang sama.
        let m = GlassMaterial::preset(GlassPreset::Panel, GlassMode::Dark);
        let red = m.composite([1.0, 0.0, 0.0]);
        let blue = m.composite([0.0, 0.0, 1.0]);
        assert!(
            red[0] > blue[0] + 0.05 && blue[2] > red[2] + 0.05,
            "{red:?} vs {blue:?}"
        );
    }

    #[test]
    fn parameters_are_in_range() {
        for mode in [GlassMode::Dark, GlassMode::Light] {
            for preset in PRESETS {
                let m = GlassMaterial::preset(preset, mode);
                for v in [
                    m.tint_strength,
                    m.frost,
                    m.specular,
                    m.dispersion,
                    m.interactive,
                    m.rim_shade,
                ] {
                    assert!((0.0..=1.0).contains(&v), "{mode:?}/{preset:?}: {v}");
                }
                assert!(m.refraction > 0.0 && m.rim_width > 0.0 && m.saturation > 0.0);
                assert!(m.luma_limit > 0.0 && m.luma_limit < 1.0);
            }
        }
    }
}
