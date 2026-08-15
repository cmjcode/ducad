//! Tema (terang/gelap) & gaya target-sentuh CADRAW bergaya Shapr3D.
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

// Token Warna Shapr3D
pub const ACCENT_BLUE: Color32 = Color32::from_rgb(10, 132, 255);       // #0a84ff
pub const ACCENT_ORANGE: Color32 = Color32::from_rgb(255, 149, 0);     // #ff9500 (Section View / Active highlight)
pub const ACCENT_GREEN: Color32 = Color32::from_rgb(48, 209, 88);      // #30d158 (Success / Constraint OK)
pub const ACCENT_PURPLE: Color32 = Color32::from_rgb(175, 82, 222);    // #af52de (Picked point)
pub const BG_CANVAS: Color32 = Color32::from_rgb(18, 19, 22);          // Deep charcoal 3D viewport
pub const BG_PANEL_DARK: Color32 = Color32::from_rgba_premultiplied(20, 22, 26, 230); // 90% glass
pub const BG_CARD_DARK: Color32 = Color32::from_rgba_premultiplied(30, 33, 40, 240);  // Card fill
pub const BG_HOVER_DARK: Color32 = Color32::from_rgba_premultiplied(44, 48, 58, 220); // Hover fill
pub const BORDER_SUBTLE: Color32 = Color32::from_rgba_premultiplied(48, 52, 62, 180); // Thin glass border
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
                v.widgets.inactive.bg_fill = Color32::from_rgba_premultiplied(30, 32, 38, 160);
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

                v.selection.bg_fill = Color32::from_rgba_premultiplied(10, 132, 255, 60);
                v.selection.stroke = Stroke::new(1.0, ACCENT_BLUE);
                v
            }
            ThemeMode::Light => {
                let mut v = Visuals::light();
                v.window_corner_radius = CornerRadius::same(10);
                v.menu_corner_radius = CornerRadius::same(8);
                v.widgets.inactive.corner_radius = CornerRadius::same(6);
                v.widgets.hovered.corner_radius = CornerRadius::same(6);
                v.widgets.active.corner_radius = CornerRadius::same(6);
                v.selection.bg_fill = Color32::from_rgba_premultiplied(10, 132, 255, 50);
                v.selection.stroke = Stroke::new(1.0, ACCENT_BLUE);
                v
            }
        }
    }
}

/// Helper frame glassmorphism untuk panel mengambang Shapr3D.
pub fn glass_frame() -> Frame {
    Frame {
        inner_margin: Margin::same(8),
        outer_margin: Margin::ZERO,
        corner_radius: CornerRadius::same(10),
        shadow: egui::Shadow {
            offset: [0, 3],
            blur: 12,
            spread: 0,
            color: Color32::from_black_alpha(100),
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
            color: Color32::from_black_alpha(110),
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

/// Terapkan tema + gaya target-sentuh ke context egui.
pub fn apply(ctx: &egui::Context, mode: ThemeMode) {
    egui_material_icons::initialize(ctx);

    let mut style = Style {
        visuals: mode.visuals(),
        ..Default::default()
    };
    style.spacing.interact_size.y = MIN_TOUCH_TARGET;
    style.spacing.button_padding = Vec2::new(8.0, 4.0);
    style.spacing.item_spacing = Vec2::new(4.0, 4.0);
    ctx.set_style(style);
}
