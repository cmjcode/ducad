//! Tema (terang/gelap) & gaya target-sentuh DUCAD bergaya Shapr3D.
//!
//! Diterapkan sekali lewat [`apply`] saat startup, lalu lagi tiap kali
//! [`ThemeMode`] berubah (toggle tema dari toolbar/command palette/settings).

use ducad_glass::{GlassFrame, GlassMode, GlassPreset};
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
pub const ACCENT_RED: Color32 = Color32::from_rgb(255, 69, 58); // #ff453a (Aksi destruktif / logout)
/// Isi lingkaran avatar inisial (biru tua) saat foto profil belum/tidak ada.
pub const AVATAR_FILL: Color32 = Color32::from_rgb(30, 58, 138);
pub const BG_CANVAS: Color32 = Color32::from_rgb(18, 19, 22); // Deep charcoal 3D viewport
/// Latar panel/drawer/toolbar kaca. Dulu ~57% — grid dan geometri viewport
/// tembus jelas di balik teks sehingga mengganggu; kini ~84%, masih sedikit
/// kaca tapi konten di belakang hanya samar.
pub const BG_PANEL_DARK: Color32 = Color32::from_rgba_premultiplied(16, 18, 22, 215);
/// Latar popup/menu/window mengambang. Jauh lebih pekat (~92%) daripada panel
/// kaca: menu yang menumpuk di atas viewport 3D harus menutup grid/geometri di
/// belakangnya agar teks item terbaca, bukan "transparan tembus" seperti panel.
pub const BG_POPUP_DARK: Color32 = Color32::from_rgba_premultiplied(20, 22, 27, 235);
pub const BG_CARD_DARK: Color32 = Color32::from_rgba_premultiplied(26, 30, 38, 160); // Card fill (~63%)
pub const BG_HOVER_DARK: Color32 = Color32::from_rgba_premultiplied(40, 45, 56, 160); // Hover fill (~63%)
pub const BORDER_SUBTLE: Color32 = Color32::from_rgba_premultiplied(50, 56, 68, 130); // Thin glass border
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(245, 245, 247);
/// Teks sekunder (WCAG AA >= 4.5:1 di atas `BG_PANEL_DARK`: ~7.5:1).
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(174, 176, 184);
/// Teks redup/petunjuk (WCAG AA >= 4.5:1 di atas `BG_PANEL_DARK`: ~5.0:1).
pub const TEXT_MUTED: Color32 = Color32::from_rgb(142, 144, 152);

// Token Warna Mode Terang
pub const BG_CANVAS_LIGHT: Color32 = Color32::from_rgb(232, 235, 240);
pub const BG_PANEL_LIGHT: Color32 = Color32::from_rgba_premultiplied(238, 240, 245, 240);
pub const BG_POPUP_LIGHT: Color32 = Color32::from_rgb(250, 251, 254);
pub const BG_CARD_LIGHT: Color32 = Color32::from_rgba_premultiplied(224, 228, 236, 236);
pub const BG_HOVER_LIGHT: Color32 = Color32::from_rgba_premultiplied(206, 212, 224, 236);
pub const BORDER_SUBTLE_LIGHT: Color32 = Color32::from_rgba_premultiplied(170, 176, 188, 215);
pub const TEXT_PRIMARY_LIGHT: Color32 = Color32::from_rgb(24, 26, 32);
pub const TEXT_SECONDARY_LIGHT: Color32 = Color32::from_rgb(68, 72, 82);
pub const TEXT_MUTED_LIGHT: Color32 = Color32::from_rgb(92, 96, 108);

/// Sistem token tipografi (lantai minimum 12 pt demi keterbacaan teknis).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypographyTokens {
    pub min: f32,
    pub caption: f32,
    pub body: f32,
    pub button: f32,
    pub mono: f32,
    pub subheading: f32,
    pub heading: f32,
}

pub const TYPOGRAPHY: TypographyTokens = TypographyTokens {
    min: 12.0,
    caption: 12.0,
    body: 13.0,
    button: 13.0,
    mono: 12.5,
    subheading: 14.0,
    heading: 16.0,
};

/// Sistem token jarak tata letak.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpacingTokens {
    pub xxs: f32,
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
}

pub const SPACING: SpacingTokens = SpacingTokens {
    xxs: 2.0,
    xs: 4.0,
    sm: 8.0,
    md: 12.0,
    lg: 16.0,
    xl: 24.0,
};

/// Sistem token radius sudut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadiusTokens {
    pub xs: u8,
    pub sm: u8,
    pub md: u8,
    pub lg: u8,
    pub pill: u8,
}

pub const RADIUS: RadiusTokens = RadiusTokens {
    xs: 4,
    sm: 6,
    md: 8,
    lg: 10,
    pill: 14,
};

/// Token warna semantik sadar-tema (Gelap & Terang).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeTokens {
    pub dark: bool,
    pub bg_canvas: Color32,
    pub bg_panel: Color32,
    pub bg_popup: Color32,
    pub bg_card: Color32,
    pub bg_hover: Color32,
    pub bg_button_idle: Color32,
    pub border_subtle: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    pub accent_primary: Color32,
    pub accent_warning: Color32,
    pub accent_success: Color32,
    pub accent_danger: Color32,
}

impl ThemeTokens {
    pub const fn from_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self {
                dark: true,
                bg_canvas: BG_CANVAS,
                bg_panel: BG_PANEL_DARK,
                bg_popup: BG_POPUP_DARK,
                bg_card: BG_CARD_DARK,
                bg_hover: BG_HOVER_DARK,
                bg_button_idle: Color32::from_rgba_premultiplied(30, 33, 40, 140),
                border_subtle: BORDER_SUBTLE,
                text_primary: TEXT_PRIMARY,
                text_secondary: TEXT_SECONDARY,
                text_muted: TEXT_MUTED,
                accent_primary: ACCENT_BLUE,
                accent_warning: ACCENT_ORANGE,
                accent_success: ACCENT_GREEN,
                accent_danger: ACCENT_RED,
            },
            ThemeMode::Light => Self {
                dark: false,
                bg_canvas: BG_CANVAS_LIGHT,
                bg_panel: BG_PANEL_LIGHT,
                bg_popup: BG_POPUP_LIGHT,
                bg_card: BG_CARD_LIGHT,
                bg_hover: BG_HOVER_LIGHT,
                bg_button_idle: Color32::from_rgba_premultiplied(218, 222, 232, 225),
                border_subtle: BORDER_SUBTLE_LIGHT,
                text_primary: TEXT_PRIMARY_LIGHT,
                text_secondary: TEXT_SECONDARY_LIGHT,
                text_muted: TEXT_MUTED_LIGHT,
                accent_primary: Color32::from_rgb(0, 102, 214),
                accent_warning: Color32::from_rgb(196, 102, 0),
                accent_success: Color32::from_rgb(24, 142, 58),
                accent_danger: Color32::from_rgb(210, 42, 34),
            },
        }
    }

    pub fn from_ui(ui: &egui::Ui) -> Self {
        if ui.visuals().dark_mode {
            Self::from_mode(ThemeMode::Dark)
        } else {
            Self::from_mode(ThemeMode::Light)
        }
    }

    pub fn for_ui(ui: &egui::Ui) -> Self {
        Self::from_ui(ui)
    }
}

/// Mode tema aplikasi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

    /// Label tombol toggle terlokalisasi, sudah termasuk ikon.
    pub fn label(self) -> String {
        match self {
            ThemeMode::Light => ducad_i18n::t!("theme-light"),
            ThemeMode::Dark => ducad_i18n::t!("theme-dark"),
        }
    }

    fn visuals(self) -> Visuals {
        match self {
            ThemeMode::Dark => {
                let mut v = Visuals::dark();
                v.panel_fill = BG_PANEL_DARK;
                // `window_fill` dipakai egui untuk SEMUA Frame::menu / Frame::popup /
                // Window — sengaja lebih pekat daripada `panel_fill` (lihat BG_POPUP_DARK).
                v.window_fill = BG_POPUP_DARK;
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
                v.panel_fill = BG_PANEL_LIGHT;
                v.window_fill = BG_POPUP_LIGHT;
                v.faint_bg_color = BG_CARD_LIGHT;
                v.extreme_bg_color = Color32::WHITE;
                v.window_stroke = Stroke::new(1.0, BORDER_SUBTLE_LIGHT);
                v.window_corner_radius = CornerRadius::same(RADIUS.lg);
                v.menu_corner_radius = CornerRadius::same(RADIUS.md);

                v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY_LIGHT);
                v.widgets.inactive.bg_fill = Color32::from_rgba_premultiplied(218, 222, 232, 225);
                v.widgets.inactive.corner_radius = CornerRadius::same(RADIUS.sm);
                v.widgets.inactive.bg_stroke = Stroke::new(0.5, BORDER_SUBTLE_LIGHT);
                v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY_LIGHT);

                v.widgets.hovered.bg_fill = BG_HOVER_LIGHT;
                v.widgets.hovered.corner_radius = CornerRadius::same(RADIUS.sm);
                v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT_BLUE);
                v.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY_LIGHT);

                v.widgets.active.bg_fill = ACCENT_BLUE;
                v.widgets.active.corner_radius = CornerRadius::same(RADIUS.sm);
                v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT_BLUE);
                v.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);

                v.widgets.open.bg_fill = BG_CARD_LIGHT;
                v.widgets.open.corner_radius = CornerRadius::same(RADIUS.sm);
                v.widgets.open.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY_LIGHT);

                // Lihat catatan di mode gelap: unmultiplied + teks gelap agar terbaca
                // di atas biru muda translusen.
                v.selection.bg_fill = Color32::from_rgba_unmultiplied(10, 132, 255, 70);
                v.selection.stroke = Stroke::new(1.0, Color32::from_rgb(10, 40, 80));
                v
            }
        }
    }
}

/// Frame panel mengambang (drawer, panel, bar atas) bermaterial Liquid Glass.
///
/// Saat backdrop GPU aktif (`ducad_glass::set_runtime`), panel digambar
/// sebagai kaca: latar viewport dibelokkan di rim, diburamkan, dan ditint.
/// Tanpa GPU (tes headless, "Kurangi transparansi") panel memakai isian datar
/// `BG_PANEL_DARK` seperti sebelumnya. Untuk tempat yang menuntut
/// `egui::Frame` (mis. `Window::frame`), pakai `glass_frame().flat()`.
pub fn glass_frame() -> GlassFrame {
    GlassFrame::preset(GlassPreset::Panel, GlassMode::Dark)
        .inner_margin(Margin::symmetric(10, 5))
        .corner_radius(CornerRadius::same(10))
        .shadow(egui::Shadow {
            offset: [0, 3],
            blur: 12,
            spread: 0,
            color: Color32::from_black_alpha(70),
        })
        .fill(BG_PANEL_DARK)
        .stroke(Stroke::new(1.0, BORDER_SUBTLE))
}

/// Frame bilah alat berisi ikon: kaca sedikit lebih bening daripada panel teks.
pub fn toolbar_frame() -> GlassFrame {
    GlassFrame::preset(GlassPreset::Toolbar, GlassMode::Dark)
        .inner_margin(Margin::same(4))
        .corner_radius(CornerRadius::same(RADIUS.md))
        .shadow(egui::Shadow {
            offset: [0, 3],
            blur: 12,
            spread: 0,
            color: Color32::from_black_alpha(70),
        })
        .fill(BG_PANEL_DARK)
        .stroke(Stroke::new(1.0, BORDER_SUBTLE))
}

/// Frame popup/menu bermaterial kaca paling pekat (preset `Popup`): menumpuk
/// di atas apa saja, jadi isinya harus paling mudah dibaca.
pub fn popup_frame() -> GlassFrame {
    GlassFrame::preset(GlassPreset::Popup, GlassMode::Dark)
        .inner_margin(Margin::same(6))
        .corner_radius(CornerRadius::same(8))
        .fill(BG_POPUP_DARK)
        .stroke(Stroke::new(1.0, BORDER_SUBTLE))
}

/// Jadikan isi menu/popup egui (`ui.menu_button`, `ComboBox::show_ui`,
/// `Popup::show`, `Modal::show`) berlatar kaca: panggil di dalam closure
/// kontennya. Kaca dilukis menutupi isian frame bawaan egui (margin dan
/// radius diambil dari `Style` sehingga rect-nya sama persis); tanpa GPU
/// hasilnya identik dengan tampilan lama.
pub fn glass_menu<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let (margin, radius) = {
        let style = ui.style();
        (style.spacing.menu_margin, style.visuals.menu_corner_radius)
    };
    popup_frame()
        .inner_margin(margin)
        .corner_radius(radius)
        .shadow(egui::Shadow::NONE)
        .paint_behind(ui, Margin::ZERO, add_contents)
}

/// Jadikan isi `egui::Window` berlatar kaca (termasuk title bar). `area_id`
/// = `egui::Id::new(judul)` kecuali window memakai `.id(..)`. Pasang
/// `.frame(glass_frame().transparent_flat())` pada window-nya.
pub fn glass_window<R>(
    ui: &mut egui::Ui,
    area_id: egui::Id,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    popup_frame()
        .inner_margin(Margin::symmetric(10, 5))
        .corner_radius(CornerRadius::same(RADIUS.lg))
        .shadow(egui::Shadow::NONE)
        .paint_behind_window(ui, area_id, add_contents)
}

/// Helper frame untuk kartu-kartu di dalam inspector / outliner.
pub fn card_frame() -> Frame {
    Frame {
        inner_margin: Margin::same(6),
        outer_margin: Margin::symmetric(0, 2),
        corner_radius: CornerRadius::same(RADIUS.sm),
        shadow: egui::Shadow::NONE,
        fill: BG_CARD_DARK,
        stroke: Stroke::new(0.5, BORDER_SUBTLE),
    }
}

/// Kartu inspector sadar-tema (Gelap / Terang).
pub fn card_frame_for(ui: &egui::Ui) -> Frame {
    let tok = ThemeTokens::from_ui(ui);
    Frame {
        inner_margin: Margin::same(6),
        outer_margin: Margin::symmetric(0, 2),
        corner_radius: CornerRadius::same(RADIUS.sm),
        shadow: egui::Shadow::NONE,
        fill: tok.bg_card,
        stroke: Stroke::new(0.5, tok.border_subtle),
    }
}

/// Helper frame untuk kapsul / pill mengambang (mis. Normal to Sketch, status bar pill).
/// Material kaca paling bening dan paling responsif terhadap sentuhan.
pub fn pill_frame() -> GlassFrame {
    GlassFrame::preset(GlassPreset::Pill, GlassMode::Dark)
        .inner_margin(Margin::symmetric(12, 5))
        .corner_radius(CornerRadius::same(14))
        .shadow(egui::Shadow {
            offset: [0, 2],
            blur: 8,
            spread: 0,
            color: Color32::from_black_alpha(70),
        })
        .fill(BG_PANEL_DARK)
        .stroke(Stroke::new(1.0, BORDER_SUBTLE))
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

/// Opsi aksesibilitas & ergonomi antarmuka untuk [`apply_with_options`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeOptions {
    pub touch_target_y: f32,
    pub ui_scale: f32,
    pub reduce_motion: bool,
}

impl Default for ThemeOptions {
    fn default() -> Self {
        Self {
            touch_target_y: MIN_TOUCH_TARGET,
            ui_scale: 1.0,
            reduce_motion: false,
        }
    }
}

/// Terapkan tema + gaya target-sentuh ke context egui.
pub fn apply(ctx: &egui::Context, mode: ThemeMode) {
    apply_with_options(ctx, mode, ThemeOptions::default());
}

/// Terapkan tema + gaya target-sentuh kustom (misalnya 44pt Apple HIG untuk iPad) ke context egui.
pub fn apply_with_touch(ctx: &egui::Context, mode: ThemeMode, touch_target_y: f32) {
    apply_with_options(
        ctx,
        mode,
        ThemeOptions {
            touch_target_y,
            ..ThemeOptions::default()
        },
    );
}

/// Terapkan tema, target sentuh, skala UI, tipografi minimum 12 pt, dan preferensi kurangi gerak.
pub fn apply_with_options(ctx: &egui::Context, mode: ThemeMode, opts: ThemeOptions) {
    install_fonts(ctx);

    let theme = match mode {
        ThemeMode::Dark => egui::Theme::Dark,
        ThemeMode::Light => egui::Theme::Light,
    };
    ctx.set_theme(theme);
    ctx.set_zoom_factor(opts.ui_scale.clamp(0.75, 2.0));

    let mut style = Style {
        visuals: mode.visuals(),
        ..Default::default()
    };
    style.animation_time = if opts.reduce_motion { 0.0 } else { 0.12 };
    style.spacing.interact_size.y = opts.touch_target_y.max(MIN_TOUCH_TARGET);
    style.spacing.button_padding = Vec2::new(SPACING.sm, SPACING.xs);
    style.spacing.item_spacing = Vec2::new(SPACING.xs, SPACING.xs);

    // Lantai tipografi minimum 12 pt di seluruh gaya teks standar egui.
    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::proportional(TYPOGRAPHY.caption),
    );
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::proportional(TYPOGRAPHY.body),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::proportional(TYPOGRAPHY.button),
    );
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        egui::FontId::monospace(TYPOGRAPHY.mono),
    );
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::proportional(TYPOGRAPHY.heading),
    );

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

    fn has_gpu_callback(shape: &egui::Shape) -> bool {
        match shape {
            egui::Shape::Callback(_) => true,
            egui::Shape::Vec(v) => v.iter().any(has_gpu_callback),
            _ => false,
        }
    }

    /// Render semua frame kaca tema; mengembalikan apakah ada callback GPU.
    fn render_glass_frames(ctx: &egui::Context) -> bool {
        let mut output = ctx.run_ui(Default::default(), |ui| {
            for frame in [glass_frame(), pill_frame(), toolbar_frame()] {
                frame.show(ui, |ui| ui.label("DUCAD"));
            }
        });
        output.textures_delta.clear();
        output.shapes.iter().any(|s| has_gpu_callback(&s.shape))
    }

    /// Penjaga Liquid Glass: tanpa backdrop GPU (tes headless, GPU belum siap)
    /// atau dengan "Kurangi transparansi", panel HARUS jatuh ke isian datar
    /// pekat lama — transparansi tanpa blur membuat teks tak terbaca.
    #[test]
    fn glass_frames_fall_back_to_flat_fill_without_gpu() {
        use ducad_glass::GlassRuntime;
        for frame in [glass_frame(), pill_frame(), toolbar_frame()] {
            assert_eq!(frame.flat().fill, BG_PANEL_DARK);
            assert_eq!(frame.flat().stroke, Stroke::new(1.0, BORDER_SUBTLE));
        }

        let ctx = egui::Context::default();
        apply(&ctx, ThemeMode::Dark);
        assert!(!render_glass_frames(&ctx), "bawaan (runtime kosong) harus datar");

        let on = GlassRuntime { enabled: true, gpu: true, reduce_transparency: false };
        ducad_glass::set_runtime(&ctx, GlassRuntime { reduce_transparency: true, ..on });
        assert!(!render_glass_frames(&ctx), "kurangi transparansi harus datar");

        ducad_glass::set_runtime(&ctx, on);
        assert!(render_glass_frames(&ctx), "runtime aktif harus memakai kaca GPU");
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

    /// Penjaga regresi: latar popup/menu (`window_fill`, dipakai Frame::menu,
    /// Frame::popup, dan Window) harus jauh lebih pekat daripada panel kaca.
    /// Dulu sama-sama ~57% sehingga grid viewport tembus di balik item menu.
    #[test]
    fn popup_fill_is_nearly_opaque_in_dark_mode() {
        let v = ThemeMode::Dark.visuals();
        let fill = v.window_fill;
        assert!(
            fill.r() <= fill.a() && fill.g() <= fill.a() && fill.b() <= fill.a(),
            "window_fill {fill:?} bukan premultiplied valid"
        );
        assert!(
            fill.a() >= 230,
            "window_fill alpha {} < 230: menu masih tembus pandang",
            fill.a()
        );
        assert!(
            fill.a() > v.panel_fill.a(),
            "popup harus lebih pekat daripada panel"
        );
        assert!(
            v.panel_fill.a() >= 200,
            "panel_fill alpha {} < 200: panel kaca masih terlalu tembus",
            v.panel_fill.a()
        );
        assert_eq!(BG_POPUP_DARK, fill);
        assert_eq!(ThemeMode::Light.visuals().window_fill.a(), 255);
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

    #[test]
    fn semantic_tokens_meet_wcag_aa_contrast_in_both_themes() {
        for mode in [ThemeMode::Dark, ThemeMode::Light] {
            let tok = ThemeTokens::from_mode(mode);
            let panel = over(tok.bg_panel, tok.bg_canvas);
            for (name, fg) in [
                ("text_primary", tok.text_primary),
                ("text_secondary", tok.text_secondary),
                ("text_muted", tok.text_muted),
            ] {
                let ratio = contrast(fg, panel);
                assert!(
                    ratio >= 4.5,
                    "{mode:?} {name}: kontras {ratio:.2} < 4.5 (fg={fg:?}, panel={panel:?})"
                );
            }
        }
    }

    #[test]
    fn typography_tokens_never_drop_below_12pt_and_reduce_motion_zeroes_anim() {
        let ctx = egui::Context::default();
        apply_with_options(
            &ctx,
            ThemeMode::Light,
            ThemeOptions {
                touch_target_y: TOUCH_TARGET_IPAD,
                ui_scale: 1.25,
                reduce_motion: true,
            },
        );
        let style = ctx.global_style();
        assert_eq!(style.animation_time, 0.0);
        assert_eq!(style.spacing.interact_size.y, TOUCH_TARGET_IPAD);
        for (ts, font) in &style.text_styles {
            assert!(
                font.size >= 12.0,
                "TextStyle {ts:?} berukuran {} < 12.0 pt",
                font.size
            );
        }
        let mut out = ctx.run_ui(Default::default(), |_| {});
        out.textures_delta.clear();
        assert!((ctx.zoom_factor() - 1.25).abs() < 1e-4);
    }
}
