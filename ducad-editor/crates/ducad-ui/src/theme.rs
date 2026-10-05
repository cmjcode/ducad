//! Tema (terang/gelap) & gaya target-sentuh DUCAD bergaya Shapr3D.
//!
//! Diterapkan sekali lewat [`apply`] saat startup, lalu lagi tiap kali
//! [`ThemeMode`] berubah (toggle tema dari toolbar/command palette/settings).

use egui::{Color32, CornerRadius, Frame, Margin, Stroke, Style, Vec2, Visuals};

/// Tinggi minimum widget interaktif (tombol, checkbox, combo box, dst),
/// mengikuti rekomendasi target sentuh 44pt Apple HIG. Dipakai sebagai
/// lantai, bukan plafon — mouse tetap nyaman dengan target lebih besar,
/// sentuh (jari/Apple Pencil di iPad) jadi andal tanpa perlu gaya terpisah
/// per platform.
/// Tinggi minimum widget interaktif default (ramping untuk desktop CAD).
pub const MIN_TOUCH_TARGET: f32 = 28.0;

/// Tinggi target sentuh standar Apple Human Interface Guidelines untuk iPad (44pt).
pub const TOUCH_TARGET_IPAD: f32 = 44.0;

/// Lebar standar terpadu untuk semua panel drawer dan popup dialog di pojok kanan bawah.
pub const BOTTOM_RIGHT_PANEL_WIDTH: f32 = 260.0;

/// Ukuran standar terpadu untuk semua ikon antarmuka utama (18px).
pub const ICON_SIZE_DEFAULT: f32 = 18.0;


// Token Warna Shapr3D
pub const ACCENT_BLUE: Color32 = Color32::from_rgb(10, 132, 255); // #0a84ff
pub const ACCENT_ORANGE: Color32 = Color32::from_rgb(255, 149, 0); // #ff9500 (Section View / Active highlight)
pub const ACCENT_GREEN: Color32 = Color32::from_rgb(48, 209, 88); // #30d158 (Success / Constraint OK)
pub const ACCENT_PURPLE: Color32 = Color32::from_rgb(175, 82, 222); // #af52de (Picked point)
pub const BG_CANVAS: Color32 = Color32::from_rgb(18, 19, 22); // Deep charcoal 3D viewport
pub const BG_PANEL_DARK: Color32 = Color32::from_rgba_premultiplied(16, 18, 22, 145); // ~57% translucent glass
pub const BG_CARD_DARK: Color32 = Color32::from_rgba_premultiplied(26, 30, 38, 160); // Card fill (~63%)
pub const BG_HOVER_DARK: Color32 = Color32::from_rgba_premultiplied(40, 45, 56, 160); // Hover fill (~63%)
pub const BORDER_SUBTLE: Color32 = Color32::from_rgba_premultiplied(50, 56, 68, 130); // Thin glass border
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(245, 245, 247);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(142, 142, 147);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(99, 99, 102);

/// Mode tema aplikasi.
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

    /// Label tombol toggle, sudah termasuk ikon.
    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::Light => "☀ Terang",
            ThemeMode::Dark => "🌙 Gelap",
        }
    }

    fn visuals(self) -> Visuals {
        match self {
            ThemeMode::Dark => {
                let mut v = Visuals::dark();
                v.panel_fill = BG_PANEL_DARK;
                v.window_fill = BG_PANEL_DARK;
                v.faint_bg_color = BG_CARD_DARK;
                v.extreme_bg_color = Color32::from_rgb(12, 13, 15);
                v.window_stroke = Stroke::new(1.0, BORDER_SUBTLE);
                v.window_corner_radius = CornerRadius::same(10);
                v.menu_corner_radius = CornerRadius::same(8);

                // Widget styling (inactive, hovered, active, open)
                v.widgets.inactive.bg_fill = Color32::from_rgba_premultiplied(28, 30, 36, 110);
                v.widgets.inactive.corner_radius = CornerRadius::same(6);
                v.widgets.inactive.bg_stroke = Stroke::new(0.5, BORDER_SUBTLE);
                v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);

                v.widgets.hovered.bg_fill = BG_HOVER_DARK;
                v.widgets.hovered.corner_radius = CornerRadius::same(6);
                v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT_BLUE);
                v.widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);

                v.widgets.active.bg_fill = ACCENT_BLUE;
                v.widgets.active.corner_radius = CornerRadius::same(6);
                v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT_BLUE);
                v.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);

                v.widgets.open.bg_fill = BG_CARD_DARK;
                v.widgets.open.corner_radius = CornerRadius::same(6);

                // Latar item terpilih (selectable_label, selectable_value, seleksi teks).
                // Harus `from_rgba_unmultiplied`: versi premultiplied dengan RGB > alpha
                // di-blend aditif oleh egui sehingga tampak biru pekat, dan egui memakai
                // `selection.stroke` sebagai WARNA TEKS item terpilih — teks biru di atas
                // biru pekat tidak terbaca (regresi panel Simulation). Teks putih di sini.
                v.selection.bg_fill = Color32::from_rgba_unmultiplied(10, 132, 255, 90);
                v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
                v
            }
            ThemeMode::Light => {
                let mut v = Visuals::light();
                v.window_corner_radius = CornerRadius::same(10);
                v.menu_corner_radius = CornerRadius::same(8);
                v.widgets.inactive.corner_radius = CornerRadius::same(6);
                v.widgets.hovered.corner_radius = CornerRadius::same(6);
                v.widgets.active.corner_radius = CornerRadius::same(6);
                // Lihat catatan di mode gelap: unmultiplied + teks gelap agar terbaca
                // di atas biru muda translusen.
                v.selection.bg_fill = Color32::from_rgba_unmultiplied(10, 132, 255, 70);
                v.selection.stroke = Stroke::new(1.0, Color32::from_rgb(10, 40, 80));
                v
            }
        }
    }
}

/// Helper frame glassmorphism untuk panel mengambang Shapr3D.
pub fn glass_frame() -> Frame {
    Frame {
        inner_margin: Margin::symmetric(10, 5),
        outer_margin: Margin::ZERO,
        corner_radius: CornerRadius::same(10),
        shadow: egui::Shadow {
            offset: [0, 3],
            blur: 12,
            spread: 0,
            color: Color32::from_black_alpha(70),
        },
        fill: BG_PANEL_DARK,
        stroke: Stroke::new(1.0, BORDER_SUBTLE),
    }
}

/// Helper frame untuk kartu-kartu di dalam inspector / outliner.
pub fn card_frame() -> Frame {
    Frame {
        inner_margin: Margin::same(6),
        outer_margin: Margin::symmetric(0, 2),
        corner_radius: CornerRadius::same(6),
        shadow: egui::Shadow::NONE,
        fill: BG_CARD_DARK,
        stroke: Stroke::new(0.5, BORDER_SUBTLE),
    }
}

/// Helper frame untuk kapsul / pill mengambang (mis. Normal to Sketch, status bar pill).
pub fn pill_frame() -> Frame {
    Frame {
        inner_margin: Margin::symmetric(12, 5),
        outer_margin: Margin::ZERO,
        corner_radius: CornerRadius::same(14),
        shadow: egui::Shadow {
            offset: [0, 2],
            blur: 8,
            spread: 0,
            color: Color32::from_black_alpha(70),
        },
        fill: BG_PANEL_DARK,
        stroke: Stroke::new(1.0, BORDER_SUBTLE),
    }
}

/// Helper frame untuk badge dimensi putih kontras di kanvas.
pub fn dimension_pill_frame() -> Frame {
    Frame {
        inner_margin: Margin::symmetric(8, 4),
        outer_margin: Margin::ZERO,
        corner_radius: CornerRadius::same(10),
        shadow: egui::Shadow {
            offset: [0, 2],
            blur: 6,
            spread: 0,
            color: Color32::from_black_alpha(130),
        },
        fill: Color32::from_rgba_premultiplied(240, 242, 245, 245),
        stroke: Stroke::new(1.0, Color32::from_gray(180)),
    }
}

/// Pasang font ikon Material + fallback glyph simbol ke font proporsional.
///
/// Ubuntu-Light/NotoEmoji (font proporsional bawaan egui) tidak memiliki banyak
/// simbol teknis (`⌀ ∠ ⊥ → ▼ …`) sehingga tampil sebagai kotak (◻). Hack —
/// font monospace bawaan egui — memilikinya, jadi dipasang sebagai fallback
/// terakhir. Cakupan dijaga tes `ui_glyph_coverage`.
fn install_fonts(ctx: &egui::Context) {
    egui_icons::initialize(ctx);
    ctx.add_font(egui::epaint::text::FontInsert::new(
        "ducad-symbol-fallback",
        egui::FontData::from_static(epaint_default_fonts::HACK_REGULAR),
        vec![egui::epaint::text::InsertFontFamily {
            family: egui::FontFamily::Proportional,
            priority: egui::epaint::text::FontPriority::Lowest,
        }],
    ));
}

/// Terapkan tema + gaya target-sentuh ke context egui.
pub fn apply(ctx: &egui::Context, mode: ThemeMode) {
    install_fonts(ctx);

    apply_with_touch(ctx, mode, MIN_TOUCH_TARGET);
}

/// Terapkan tema + gaya target-sentuh kustom (misalnya 44pt Apple HIG untuk iPad) ke context egui.
pub fn apply_with_touch(ctx: &egui::Context, mode: ThemeMode, touch_target_y: f32) {
    install_fonts(ctx);

    let theme = match mode {
        ThemeMode::Dark => egui::Theme::Dark,
        ThemeMode::Light => egui::Theme::Light,
    };
    ctx.set_theme(theme);

    let mut style = Style {
        visuals: mode.visuals(),
        ..Default::default()
    };
    style.spacing.interact_size.y = touch_target_y.max(MIN_TOUCH_TARGET);
    style.spacing.button_padding = Vec2::new(8.0, 4.0);
    style.spacing.item_spacing = Vec2::new(4.0, 4.0);
    ctx.set_style_of(theme, style);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_apply_and_material_icons() {
        let ctx = egui::Context::default();
        apply(&ctx, ThemeMode::Dark);

        let mut output = ctx.run_ui(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.label(egui_icons::icons::ICON_HOME.rich_text());
                ui.label(egui_icons::icons::ICON_SETTINGS.rich_text());
                ui.label(egui_icons::icons::ICON_ADS_CLICK.rich_text());
                ui.label(egui_icons::icons::ICON_HORIZONTAL_RULE.rich_text());
                // Test Pictogrammers MDI icons
                ui.label(egui_icons::icons::ICON_CUBE.rich_text());
                ui.label(egui_icons::icons::ICON_CUBE_OUTLINE.rich_text());
                ui.label(egui_icons::icons::MDI_CUBE.rich_text());
            });
        });
        output.textures_delta.clear();
    }

    /// Luminansi relatif (sRGB → linear) untuk rasio kontras WCAG.
    fn luminance(c: Color32) -> f32 {
        let lin = |v: u8| {
            let f = v as f32 / 255.0;
            if f <= 0.04045 { f / 12.92 } else { ((f + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f32 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    /// Komposit `src` (premultiplied) di atas `dst` opak.
    fn over(src: Color32, dst: Color32) -> Color32 {
        let k = 1.0 - src.a() as f32 / 255.0;
        let ch = |s: u8, d: u8| (s as f32 + d as f32 * k).round().clamp(0.0, 255.0) as u8;
        Color32::from_rgb(ch(src.r(), dst.r()), ch(src.g(), dst.g()), ch(src.b(), dst.b()))
    }

    /// Penjaga regresi: teks item terpilih (`selection.stroke`) harus kontras
    /// dengan latar seleksi yang dikomposit di atas panel. Dulu keduanya biru
    /// aksen sehingga nama studi di panel Simulation tidak terlihat.
    #[test]
    fn selection_text_is_legible_on_selection_fill() {
        for mode in [ThemeMode::Dark, ThemeMode::Light] {
            let v = mode.visuals();
            let fill = v.selection.bg_fill;
            // Premultiplied valid: tiap kanal <= alpha (RGB > alpha = blending aditif).
            assert!(
                fill.r() <= fill.a() && fill.g() <= fill.a() && fill.b() <= fill.a(),
                "{mode:?}: selection.bg_fill {fill:?} bukan premultiplied valid (aditif)"
            );
            let bg_opaque = over(v.panel_fill, Color32::from_rgb(18, 19, 22));
            let composited = over(fill, bg_opaque);
            let ratio = contrast(v.selection.stroke.color, composited);
            assert!(
                ratio >= 4.5,
                "{mode:?}: kontras teks seleksi {ratio:.2} < 4.5 (teks {:?} di atas {composited:?})",
                v.selection.stroke.color
            );
        }
    }
}
