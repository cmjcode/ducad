//! Modern Top Bar & Title Header bergaya Shapr3D dengan Material Icons.
//!
//! Menampilkan bar atas mengambang minimalis dengan nama dokumen,
//! indikator status sinkronisasi/simpan, dan burger menu (Berkas, Impor,
//! Ekspor, semua perintah, Pengaturan) — plus (sejak reorganisasi toolbar) kontrol
//! yang selalu tersedia di kedua mode (Sketch & 3D): mode switcher, Items,
//! Search, Sketch Plane selector (khusus saat Sketch Mode), Section View,
//! Measurements, dan Delete. Kontrol yang hanya relevan di satu mode
//! (tool-tool sketsa 2D) tetap tinggal di `LeftToolbar`.

use crate::theme::{
    glass_frame, ThemeTokens, ACCENT_BLUE, ACCENT_GREEN, MIN_TOUCH_TARGET, RADIUS,
    TOUCH_TARGET_IPAD, TYPOGRAPHY,
};
use crate::touch::{TouchDesignConfig, TouchDesignMode};
use crate::unit_input::localized_unit_label;
use ducad_cloud::DucadAccount;
use ducad_i18n::{current_language, t, Language};
use egui::containers::menu::MenuConfig;
use egui::{vec2, Align2, Color32, CornerRadius, Frame, Margin, RichText, Sense, Stroke, Ui, Vec2};
use egui_icons::icons::{
    ICON_AUTO_AWESOME, ICON_BLUR_ON, ICON_CATEGORY, ICON_CLOUD, ICON_CUBE_OUTLINE, ICON_DOWNLOAD, ICON_EDIT, ICON_FILE_OPEN, ICON_HELP,
    ICON_LANGUAGE, ICON_LAYERS_OFF, ICON_MENU, ICON_NOTE_ADD, ICON_OPACITY, ICON_PALETTE, ICON_PERSON,
    ICON_PICTURE_AS_PDF, ICON_SAVE, ICON_SEARCH, ICON_SETTINGS, ICON_SHARE, ICON_STRAIGHTEN, ICON_SYNC,
    ICON_TEXTURE, ICON_UPLOAD,
};

use ducad_core::LengthUnit;

/// Jarak vertikal antara tepi bawah top bar dan menu hamburger (px).
const MENU_GAP_BELOW_BAR: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopBarFileOp {
    New,
    /// Buka layar "Proyek baru dari brief".
    NewFromBrief,
    Open,
    Save,
    SaveAs,
    ImportStep,
    ImportStl,
    ImportDxf,
    ImportIges,
    ImportMesh,
    InsertSvg,
    ExportStep,
    ExportStl,
    ExportObj,
    ExportGlb,
    ExportThreeMf,
    ExportIges,
    ExportPng,
    ExportDxf,
    ExportSvg,
    ExportPdf,
    ExportDrawingDxf,
    ExportDrawingSvg,
    ExportViewDxf,
    /// Tangkapan vektor 2D dari sudut pandang kamera viewport saat ini.
    ExportVectorSnapshot,
    OpenDrawingSheet,
}

/// Aksi dari kontrol % zoom di header (kiri, di sebelah nama dokumen).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TopBarZoom {
    /// Perbesar satu langkah (`ZOOM_STEP`).
    In,
    /// Perkecil satu langkah (`ZOOM_STEP`).
    Out,
    /// Atur ke persentase tertentu; 100 = ukuran sebenarnya (1 mm = 1 mm layar).
    Percent(f32),
    /// Pas seluruh model/sketsa ke layar.
    Fit,
}

/// Faktor zoom satu klik tombol +/− di header.
pub const ZOOM_STEP: f32 = 1.25;

/// Preset persentase yang ditawarkan menu zoom.
pub const ZOOM_PRESETS: [f32; 6] = [25.0, 50.0, 100.0, 200.0, 400.0, 800.0];

/// Teks indikator zoom: bilangan bulat untuk >= 10%, satu desimal di bawahnya,
/// dan `--%` bila nilainya tidak finit.
pub fn format_zoom_percent(percent: f32) -> String {
    if !percent.is_finite() {
        "--%".to_string()
    } else if percent >= 10.0 {
        format!("{}%", percent.round() as i64)
    } else {
        format!("{percent:.1}%")
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TopBarEvent {
    HomeClicked,
    /// Kontrol % zoom viewport.
    Zoom(TopBarZoom),
    File(TopBarFileOp),
    ToggleTheme,
    /// Nyalakan/matikan material Liquid Glass.
    ToggleLiquidGlass,
    /// Nyalakan/matikan "Kurangi transparansi" (panel datar pekat).
    ToggleReduceTransparency,
    /// Tampilkan/sembunyikan label teks di toolbar kiri.
    ToggleToolbarLabels,
    /// Nyalakan/matikan "Kurangi gerak" (animasi 0 ms).
    ToggleReduceMotion,
    /// Atur faktor skala UI (`1.0`, `1.15`, `1.25`, `1.5`).
    SetUiScale(f32),
    OpenCommandPalette,
    SetUnit(LengthUnit),
    SetLanguage(Language),
    SetIconSize(f32),
    ToggleItemsDrawer,
    OpenSearch,
    EnterSketching,
    ExitSketching,
    SelectSketchPlane(usize),
    CreateDatumPlane,
    TogglePlanesDrawer,
    ToggleSectionView,
    ToggleMeasurements,
    ToggleZebraView,
    ToggleStudioLighting,
    OpenDrawingSheet,
    DeleteSelection,
    ToggleAccountDrawer,
    SetTouchDesignMode(TouchDesignMode),
    CycleTouchDesignMode,
    /// Buka/tutup panel Checks (P7.5).
    ToggleChecksPanel,
    /// Buka dialog "Tanya AI…".
    OpenAssist,
    /// Buka/tutup sidebar Chat AI (P13.3).
    ToggleChat,
    /// Buka tutorial selamat datang dari awal (tombol bantuan).
    StartTutorial,
    /// Periksa pembaruan rilis terbaru dari GitHub.
    CheckForUpdates,
    /// Jalankan perintah palette; nilainya `TopBarCommand::index`.
    RunCommand(usize),
}

/// Satu perintah palette yang ikut tampil di burger menu.
#[derive(Debug, Clone)]
pub struct TopBarCommand {
    pub label: String,
    /// Pintasan keyboard, boleh kosong.
    pub hint: String,
    /// Indeks ke daftar perintah milik caller.
    pub index: usize,
}

/// Submenu burger menu: satu grup perintah palette.
#[derive(Debug, Clone)]
pub struct TopBarCommandGroup {
    pub icon: &'static str,
    pub title: String,
    pub items: Vec<TopBarCommand>,
}

/// State kontrol header yang dibaca & (untuk `plane_menu_open`) ditulis ulang
/// oleh `TopBar::show`. Dikonstruksi ulang tiap frame dari state `App`.
pub struct TopBarState {
    pub document_name: String,
    pub status_saved: bool,
    /// Persentase zoom viewport saat ini (100 = ukuran sebenarnya); dihitung
    /// caller dari kamera + tinggi viewport, lihat `OrbitCamera::zoom_percent`.
    pub zoom_percent: f32,
    pub current_unit: LengthUnit,
    pub icon_size: f32,
    /// True saat Sketch Mode aktif — mengontrol apakah tombol Sketch Plane
    /// (dan popup pemilih bidangnya) ditampilkan sama sekali.
    pub is_sketching: bool,
    pub items_drawer_open: bool,
    pub section_view_active: bool,
    pub is_measure_active: bool,
    pub zebra_view_active: bool,
    pub studio_lighting_active: bool,
    pub active_plane_name: String,
    pub custom_planes: Vec<(usize, String)>,
    /// Dropdown popup pemilih Sketch Plane (Top/Front/Right/Custom). Dibaca & bisa
    /// diubah oleh `show` — caller wajib menyalin nilai baru balik ke state
    /// persisten miliknya setelah `show` selesai.
    pub plane_menu_open: bool,
    /// Rect layar tombol Items setelah dirender frame ini — dipakai caller
    /// buat menempatkan popup Items Drawer tepat di bawah tombolnya.
    pub items_button_rect: egui::Rect,
    /// Akun pengguna DUCAD jika terotentikasi
    pub account: Option<DucadAccount>,
    /// Foto profil akun login yang sudah termuat; `None` = tampilkan inisial.
    pub account_avatar: Option<egui::TextureHandle>,
    /// Status apakah sedang dalam proses otentikasi browser
    pub is_authenticating: bool,
    /// Status apakah popup akun sedang terbuka
    pub account_drawer_open: bool,
    /// Rect layar tombol Account untuk anchor popup
    pub account_button_rect: egui::Rect,
    /// Konfigurasi Touch Design (Apple Pencil vs Sentuhan Jari)
    pub touch_config: TouchDesignConfig,
    /// Mode tampilan iPad / tablet layar sentuh
    pub is_ipad: bool,
    /// Ringkasan checks desain `(lulus, tidak lulus)`; `None` = tidak ada check.
    pub checks_summary: Option<(usize, usize)>,
    /// Status AI (P11.4): `Some(true)` = backend di perangkat aktif,
    /// `Some(false)` = backend eksternal diizinkan, `None` = tidak ada AI.
    pub ai_on_device: Option<bool>,
    /// Jumlah klien jembatan agent (P5.1); `None` = jembatan mati.
    pub bridge_clients: Option<usize>,
    pub checks_panel_open: bool,
    /// Sidebar Chat AI sedang terbuka (ikon header disorot).
    pub chat_open: bool,
    /// Semua perintah command palette, per grup — dirender sebagai submenu
    /// burger menu supaya tiap perintah punya jalur klik di GUI.
    pub command_groups: Vec<TopBarCommandGroup>,
    /// Updater desktop aktif — menu "Periksa Pembaruan" hanya tampil bila
    /// true (di iPad/tablet pembaruan lewat App Store).
    pub updates_enabled: bool,
    pub show_toolbar_labels: bool,
    pub reduce_motion: bool,
    pub ui_scale: f32,
}

pub struct TopBar;

impl TopBar {
    /// Tinggi baris isi top bar (di dalam margin frame) untuk `icon_size`.
    /// Dipakai juga kepala sidebar Chat AI supaya barisnya sama tinggi dan
    /// ikonnya sejajar dengan tombol top bar di sebelah kirinya.
    pub fn bar_height(icon_size: f32) -> f32 {
        (icon_size.clamp(12.0, 18.0) + 14.0).max(30.0)
    }

    /// Render modern top bar. Mengembalikan `Option<TopBarEvent>`.
    pub fn show(ui: &mut Ui, state: &mut TopBarState) -> Option<TopBarEvent> {
        let mut event = None;
        let icon_sz = state.icon_size.clamp(12.0, 18.0);
        let avail_w = ui.available_width();
        let is_compact = avail_w < 920.0 || state.is_ipad;
        let is_tight = avail_w < 780.0;
        let item_gap = if is_tight { 2.0 } else { 4.0 };

        let bar_frame = glass_frame();
        let bar_margin = bar_frame.inner_margin;
        bar_frame.show(ui, |ui| {
            let tok = ThemeTokens::from_ui(ui);
            // Rect LUAR bar (termasuk inner margin frame) — anchor menu hamburger.
            let bar_outer_rect = ui.max_rect() + bar_margin;
            // Kunci tinggi minimum widget interaktif header ke nilai tetap di desktop
            // agar tinggi bar tidak melompat saat mode sentuh di-cycle; di perangkat
            // iPad/tablet sejati (`state.is_ipad`), hormati target sentuh 44 pt HIG.
            let target_h = if state.is_ipad {
                TOUCH_TARGET_IPAD
            } else {
                MIN_TOUCH_TARGET
            };
            ui.spacing_mut().interact_size.y = target_h;

            let bar_h = if state.is_ipad {
                Self::bar_height(icon_sz).max(TOUCH_TARGET_IPAD)
            } else {
                Self::bar_height(icon_sz)
            };
            ui.set_height(bar_h);
            ui.horizontal_centered(|ui| {
                let menu_btn = ui.add(egui::Button::new(
                    RichText::new(ICON_MENU.codepoint)
                        .size(icon_sz)
                        .color(tok.text_primary),
                ));
                let menu_a11y = t!("menu-file");
                menu_btn.widget_info(move || {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, menu_a11y.clone())
                });
                let menu_anchor = egui::Rect::from_min_max(
                    egui::pos2(bar_outer_rect.left(), menu_btn.rect.top()),
                    egui::pos2(menu_btn.rect.right(), bar_outer_rect.bottom()),
                );
                egui::Popup::menu(&menu_btn)
                    .anchor(menu_anchor)
                    .gap(MENU_GAP_BELOW_BAR)
                    .info(
                        egui::UiStackInfo::new(egui::UiKind::Menu)
                            .with_tag_value(MenuConfig::MENU_CONFIG_TAG, MenuConfig::new()),
                    )
                    .show(|ui| crate::theme::glass_menu(ui, |ui| {
                        if ui
                            .button(format!("{} {}", ICON_NOTE_ADD.codepoint, t!("menu-new")))
                            .clicked()
                        {
                            event = Some(TopBarEvent::File(TopBarFileOp::New));
                            ui.close();
                        }
                        if ui
                            .button(format!(
                                "{} {}",
                                ICON_AUTO_AWESOME.codepoint,
                                t!("menu-new-from-brief")
                            ))
                            .clicked()
                        {
                            event = Some(TopBarEvent::File(TopBarFileOp::NewFromBrief));
                            ui.close();
                        }
                        if ui
                            .button(format!(
                                "{} {} (⌘O)",
                                ICON_FILE_OPEN.codepoint,
                                t!("menu-open")
                            ))
                            .clicked()
                        {
                            event = Some(TopBarEvent::File(TopBarFileOp::Open));
                            ui.close();
                        }
                        if ui
                            .button(format!("{} {} (⌘S)", ICON_SAVE.codepoint, t!("menu-save")))
                            .clicked()
                        {
                            event = Some(TopBarEvent::File(TopBarFileOp::Save));
                            ui.close();
                        }
                        if ui
                            .button(format!(
                                "{} {} (⌘+Shift+S)",
                                ICON_SAVE.codepoint,
                                t!("menu-save-as")
                            ))
                            .clicked()
                        {
                            event = Some(TopBarEvent::File(TopBarFileOp::SaveAs));
                            ui.close();
                        }
                        ui.separator();
                        ui.menu_button(
                            format!("{} {}", ICON_DOWNLOAD.codepoint, t!("menu-import")),
                            |ui| crate::theme::glass_menu(ui, |ui| {
                                let items = [
                                    (t!("menu-import-step"), TopBarFileOp::ImportStep),
                                    (t!("menu-import-iges"), TopBarFileOp::ImportIges),
                                    (t!("menu-import-stl"), TopBarFileOp::ImportStl),
                                    (t!("menu-import-mesh"), TopBarFileOp::ImportMesh),
                                    (t!("menu-import-dxf"), TopBarFileOp::ImportDxf),
                                    (t!("menu-insert-svg"), TopBarFileOp::InsertSvg),
                                ];
                                for (label, op) in items {
                                    if ui.button(label).clicked() {
                                        event = Some(TopBarEvent::File(op));
                                        ui.close();
                                    }
                                }
                            }),
                        );
                        ui.menu_button(
                            format!("{} {}", ICON_SHARE.codepoint, t!("menu-export")),
                            |ui| crate::theme::glass_menu(ui, |ui| export_menu_items(ui, &mut event)),
                        );
                        ui.separator();
                        if ui
                            .button(format!(
                                "{} {}",
                                ICON_PICTURE_AS_PDF.codepoint,
                                t!("menu-drawing-sheet")
                            ))
                            .clicked()
                        {
                            event = Some(TopBarEvent::OpenDrawingSheet);
                            ui.close();
                        }
                        if !state.command_groups.is_empty() {
                            ui.separator();
                            ui.label(
                                RichText::new(t!("menu-all-commands"))
                                    .strong()
                                    .size(TYPOGRAPHY.caption)
                                    .color(tok.text_secondary),
                            );
                            let max_h = ui.ctx().content_rect().height() * 0.6;
                            for group in &state.command_groups {
                                ui.menu_button(format!("{} {}", group.icon, group.title), |ui| crate::theme::glass_menu(ui, |ui| {
                                    ui.set_min_width(260.0);
                                    egui::ScrollArea::vertical().max_height(max_h).show(ui, |ui| {
                                        for cmd in &group.items {
                                            let mut btn = egui::Button::new(cmd.label.as_str());
                                            if !cmd.hint.is_empty() {
                                                btn = btn.shortcut_text(cmd.hint.as_str());
                                            }
                                            if ui.add(btn).clicked() {
                                                event = Some(TopBarEvent::RunCommand(cmd.index));
                                                ui.close();
                                            }
                                        }
                                    });
                                }));
                            }
                            ui.separator();
                            if ui
                                .add(
                                    egui::Button::new(format!(
                                        "{} {}",
                                        ICON_SEARCH.codepoint,
                                        t!("menu-command-palette")
                                    ))
                                    .shortcut_text("⌘K"),
                                )
                                .clicked()
                            {
                                event = Some(TopBarEvent::OpenCommandPalette);
                                ui.close();
                            }
                        }
                        ui.separator();
                        ui.menu_button(
                            format!("{} {}", ICON_SETTINGS.codepoint, t!("menu-settings")),
                            |ui| crate::theme::glass_menu(ui, |ui| {
                                settings_menu_items(ui, state, icon_sz, &mut event)
                            }),
                        );
                    }));
                // Target tutorial "Share" kini menunjuk burger menu, tempat
                // submenu Ekspor berada.
                crate::onboarding::Onboarding::publish_target(
                    ui.ctx(),
                    crate::onboarding::OnboardingTarget::ShareButton,
                    menu_btn.rect,
                );
                menu_btn.on_hover_text(t!("menu-file"));

                ui.add_space(2.0);

                // 2. Document Title & Cloud/File Status (Responsif iPad)
                let cloud_color = if state.status_saved {
                    ACCENT_BLUE
                } else {
                    Color32::from_rgb(255, 180, 50)
                };

                let display_doc_name = if is_tight && state.document_name.len() > 14 {
                    format!("{}...", &state.document_name[..11])
                } else if is_compact && state.document_name.len() > 20 {
                    format!("{}...", &state.document_name[..17])
                } else {
                    state.document_name.clone()
                };

                ui.horizontal(|ui| {
                    let cloud_tooltip = if state.status_saved {
                        t!("topbar-saved-tooltip")
                    } else {
                        t!("topbar-unsaved-tooltip")
                    };
                    ui.label(
                        RichText::new(ICON_CLOUD.codepoint)
                            .size(icon_sz)
                            .color(cloud_color),
                    )
                    .on_hover_text(cloud_tooltip);
                    ui.label(
                        RichText::new(display_doc_name)
                            .strong()
                            .size(TYPOGRAPHY.caption)
                            .color(tok.text_primary),
                    )
                    .on_hover_text(&state.document_name);
                });

                // Kontrol zoom kini pil mengambang di pojok kanan bawah (`app.rs`), bukan di header.
                ui.add_space(item_gap);
                ui.separator();
                ui.add_space(item_gap);

                // 3. Mode Switcher + Items + Search + Sketch Plane (Tetap di sebelah File Name)
                let (mode_icon, mode_title, mode_shortcut, mode_sub) = if state.is_sketching {
                    (
                        ICON_EDIT.codepoint,
                        t!("topbar-sketch-mode"),
                        "⌘+Alt+3",
                        t!("topbar-switch-to-solid"),
                    )
                } else {
                    (
                        ICON_CUBE_OUTLINE.codepoint,
                        t!("topbar-solid-mode"),
                        "⌘+Alt+2",
                        t!("topbar-switch-to-sketch"),
                    )
                };
                let mode_btn = header_icon_btn(
                    ui,
                    mode_icon,
                    icon_sz,
                    true,
                    &mode_title,
                    Some(mode_shortcut),
                    Some(&mode_sub),
                    Some(Color32::from_rgba_premultiplied(18, 42, 85, 100)),
                    Some(ACCENT_BLUE),
                );
                crate::onboarding::Onboarding::publish_target(
                    ui.ctx(),
                    crate::onboarding::OnboardingTarget::ModeButton,
                    mode_btn.rect,
                );
                if mode_btn.clicked() {
                    event = Some(if state.is_sketching {
                        TopBarEvent::ExitSketching
                    } else {
                        TopBarEvent::EnterSketching
                    });
                }

                let search_title = t!("menu-command-palette");
                let search_sub = t!("topbar-search-tooltip");
                let search_btn = header_icon_btn(
                    ui,
                    ICON_SEARCH.codepoint,
                    icon_sz,
                    false,
                    &search_title,
                    Some("⌘K / ⌘⇧P"),
                    Some(&search_sub),
                    None,
                    None,
                );
                crate::onboarding::Onboarding::publish_target(
                    ui.ctx(),
                    crate::onboarding::OnboardingTarget::PaletteButton,
                    search_btn.rect,
                );
                if search_btn.clicked() {
                    event = Some(TopBarEvent::OpenSearch);
                }

                if state.is_sketching {
                    let plane_btn = header_icon_btn(
                        ui,
                        ICON_LAYERS_OFF.codepoint,
                        icon_sz,
                        state.plane_menu_open,
                        &t!(
                            "topbar-sketch-plane",
                            plane = state.active_plane_name.as_str()
                        ),
                        None,
                        Some(state.active_plane_name.as_str()),
                        Some(Color32::from_rgba_premultiplied(18, 42, 85, 100)),
                        Some(ACCENT_BLUE),
                    );
                    if plane_btn.clicked() {
                        state.plane_menu_open = !state.plane_menu_open;
                    }

                    if state.plane_menu_open {
                        let p_rect = plane_btn.rect;
                        let menu_pos = egui::pos2(p_rect.left(), p_rect.bottom() + 6.0);
                        egui::Area::new(egui::Id::new("ducad-topbar-plane-select-popup"))
                            .fixed_pos(menu_pos)
                            .order(egui::Order::Tooltip)
                            .show(ui.ctx(), |ui| {
                                glass_frame().show(ui, |ui| {
                                    let tok = ThemeTokens::from_ui(ui);
                                    ui.set_width(180.0);
                                    ui.spacing_mut().item_spacing = Vec2::new(2.0, 4.0);
                                    ui.label(
                                        RichText::new(t!("topbar-sketch-plane", plane = ""))
                                            .strong()
                                            .size(TYPOGRAPHY.caption)
                                            .color(tok.text_secondary),
                                    );
                                    ui.separator();

                                    let planes = [
                                        (0, t!("plane-top"), "Top Plane"),
                                        (1, t!("plane-front"), "Front Plane"),
                                        (2, t!("plane-right"), "Right Plane"),
                                    ];

                                    for (idx, label, sub) in planes {
                                        let plane_active =
                                            state.active_plane_name.contains(&label[..3]);
                                        let btn = ui.selectable_label(
                                            plane_active,
                                            RichText::new(format!(
                                                "{} {}",
                                                ICON_LAYERS_OFF.codepoint, label
                                            ))
                                            .size(TYPOGRAPHY.caption),
                                        );
                                        if btn.on_hover_text(sub).clicked() {
                                            event = Some(TopBarEvent::SelectSketchPlane(idx));
                                            state.plane_menu_open = false;
                                        }
                                    }

                                    if !state.custom_planes.is_empty() {
                                        ui.separator();
                                        ui.label(
                                            RichText::new(t!("datum-planes-header"))
                                                .strong()
                                                .size(TYPOGRAPHY.caption)
                                                .color(tok.text_secondary),
                                        );
                                        for (idx, name) in &state.custom_planes {
                                            let plane_active = state.active_plane_name.contains(name);
                                            let btn = ui.selectable_label(
                                                plane_active,
                                                RichText::new(format!(
                                                    "{} {}",
                                                    ICON_LAYERS_OFF.codepoint, name
                                                ))
                                                .size(TYPOGRAPHY.caption),
                                            );
                                            if btn.clicked() {
                                                event = Some(TopBarEvent::SelectSketchPlane(*idx));
                                                state.plane_menu_open = false;
                                            }
                                        }
                                    }

                                    ui.separator();
                                    let new_plane_btn = ui.button(
                                        RichText::new(t!("datum-plane-new"))
                                            .size(TYPOGRAPHY.caption)
                                            .color(tok.accent_primary),
                                    );
                                    if new_plane_btn.clicked() {
                                        event = Some(TopBarEvent::CreateDatumPlane);
                                        state.plane_menu_open = false;
                                    }

                                    let manage_btn = ui.button(
                                        RichText::new(format!("{} {}", ICON_LAYERS_OFF.codepoint, t!("planes-drawer-title")))
                                            .size(TYPOGRAPHY.caption)
                                            .color(tok.text_primary),
                                    );
                                    if manage_btn.clicked() {
                                        event = Some(TopBarEvent::TogglePlanesDrawer);
                                        state.plane_menu_open = false;
                                    }
                                });
                            });
                    }
                }

                if !is_compact {
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // 4. Toggle Show All Dimensions (Ruler / Measure)
                    let meas_title = t!("hud-show-dimensions");
                    let meas_sub = t!("hud-click-to-edit");
                    let meas_btn = header_icon_btn(
                        ui,
                        ICON_STRAIGHTEN.codepoint,
                        icon_sz,
                        state.is_measure_active,
                        &meas_title,
                        Some("M"),
                        Some(&meas_sub),
                        None,
                        None,
                    );
                    if meas_btn.clicked() {
                        event = Some(TopBarEvent::ToggleMeasurements);
                    }

                    // 5. Toggle Zebra Reflection Stripes (Material / Texture)
                    let zebra_title = t!("tool-zebra-stripes");
                    let zebra_sub = t!("topbar-zebra-tooltip");
                    let zebra_btn = header_icon_btn(
                        ui,
                        ICON_TEXTURE.codepoint,
                        icon_sz,
                        state.zebra_view_active,
                        &zebra_title,
                        Some("Z"),
                        Some(&zebra_sub),
                        None,
                        None,
                    );
                    if zebra_btn.clicked() {
                        event = Some(TopBarEvent::ToggleZebraView);
                    }

                    // 6. Tombol Masuk ke Lembar Kerja Gambar Teknik 2D (Drawing Sheet)
                    let ds_title = t!("topbar-drawing-sheet");
                    let ds_sub = t!("topbar-drawing-sheet-tooltip");
                    let ds_btn = header_icon_btn(
                        ui,
                        ICON_PICTURE_AS_PDF.codepoint,
                        icon_sz,
                        false,
                        &ds_title,
                        Some("⌘D"),
                        Some(&ds_sub),
                        None,
                        None,
                    );
                    if ds_btn.clicked() {
                        event = Some(TopBarEvent::OpenDrawingSheet);
                    }

                    // Tombol Assembly kini berada di bar tombol mengambang pojok kanan bawah
                    // (`app.rs`), dekat drawer yang dibukanya.

                    // 8a2. Chip status jembatan agent (P5.1).
                    if let Some(clients) = state.bridge_clients {
                        let n = clients.to_string();
                        let label = t!("bridge-chip", clients = n.as_str());
                        ui.add(egui::Button::new(
                            RichText::new(label)
                                .size(11.0)
                                .color(crate::theme::ACCENT_GREEN),
                        ))
                        .on_hover_text(t!("bridge-on"));
                    }

                    // 8b. Chip status AI (P11.4).
                    if let Some(on_device) = state.ai_on_device {
                        let (label, color) = if on_device {
                            (t!("ai-chip-on-device"), crate::theme::ACCENT_GREEN)
                        } else {
                            (t!("ai-chip-external"), Color32::from_rgb(255, 159, 10))
                        };
                        if ui
                            .add(egui::Button::new(RichText::new(label).size(11.0).color(color)))
                            .on_hover_text(t!("assist-title"))
                            .clicked()
                        {
                            event = Some(TopBarEvent::OpenAssist);
                        }
                    }

                    // 8. Ringkasan checks desain (hanya bila part punya check).
                    if let Some((pass, not_pass)) = state.checks_summary {
                        let color = if not_pass == 0 {
                            crate::theme::ACCENT_GREEN
                        } else {
                            Color32::from_rgb(255, 69, 58)
                        };
                        let chk_btn = ui
                            .add(
                                egui::Button::new(
                                    RichText::new(format!("✔ {pass} ✖ {not_pass}")).size(11.0).color(color),
                                )
                                .selected(state.checks_panel_open),
                            )
                            .on_hover_text(t!("checks-summary-tooltip"));
                        if chk_btn.clicked() {
                            event = Some(TopBarEvent::ToggleChecksPanel);
                        }
                    }
                } else {
                    ui.add_space(2.0);
                    ui.menu_button(
                        RichText::new(ICON_CATEGORY.codepoint).size(icon_sz).color(tok.text_primary),
                        |ui| crate::theme::glass_menu(ui, |ui| {
                            ui.set_min_width(170.0);
                            let m_title = t!("hud-show-dimensions");
                            let m_chk = if state.is_measure_active { "✔ " } else { "  " };
                            if ui.button(format!("{}{} {}", m_chk, ICON_STRAIGHTEN.codepoint, m_title)).clicked() {
                                event = Some(TopBarEvent::ToggleMeasurements);
                                ui.close();
                            }
                            let z_title = t!("tool-zebra-stripes");
                            let z_chk = if state.zebra_view_active { "✔ " } else { "  " };
                            if ui.button(format!("{}{} {}", z_chk, ICON_TEXTURE.codepoint, z_title)).clicked() {
                                event = Some(TopBarEvent::ToggleZebraView);
                                ui.close();
                            }
                            let ds_title = t!("topbar-drawing-sheet");
                            if ui.button(format!("  {} {}", ICON_PICTURE_AS_PDF.codepoint, ds_title)).clicked() {
                                event = Some(TopBarEvent::OpenDrawingSheet);
                                ui.close();
                            }
                            if let Some((pass, not_pass)) = state.checks_summary {
                                let c_chk = if state.checks_panel_open { "✔ " } else { "  " };
                                let label = format!("{c_chk}{} (✔ {pass} ✖ {not_pass})", t!("checks-title"));
                                if ui.button(label).clicked() {
                                    event = Some(TopBarEvent::ToggleChecksPanel);
                                    ui.close();
                                }
                            }
                        }),
                    ).response.on_hover_text(t!("topbar-more-tools"));
                }

                // Ujung kanan kelompok tombol kiri: awal celah tengah yang bisa
                // dipakai HUD header tanpa menumpuk tombol.
                let left_cluster_end = ui.cursor().left();

                // 5. Right-aligned Settings and Export Buttons (Minimalist Icon-Only)
                let right_cluster_start = ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let tok = ThemeTokens::from_ui(ui);
                    // Sisi paling kanan: Tombol Akun DUCAD / Cloud
                    let acct_btn_resp = if let Some(acc) = &state.account {
                        let (rect, resp) = ui.allocate_exact_size(vec2(icon_sz + 8.0, icon_sz + 8.0), Sense::click());
                        let acct_tip = t!(
                            "topbar-account-logged-in",
                            name = acc.display_title(),
                            email = acc.email.as_str()
                        );
                        let acct_a11y = acct_tip.clone();
                        resp.widget_info(move || {
                            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, acct_a11y.clone())
                        });
                        if resp.hovered() {
                            ui.painter().rect_filled(rect, CornerRadius::same(RADIUS.pill), tok.bg_hover);
                        }
                        let avatar_rect = egui::Rect::from_center_size(rect.center(), Vec2::splat(icon_sz + 4.0));
                        crate::account_drawer::paint_avatar(
                            ui,
                            avatar_rect,
                            state.account_avatar.as_ref(),
                            &acc.initials(),
                            Stroke::new(1.0, tok.accent_primary),
                        );
                        ui.painter().circle_filled(rect.right_bottom() - vec2(2.0, 2.0), 4.0, tok.bg_popup);
                        ui.painter().circle_filled(rect.right_bottom() - vec2(2.0, 2.0), 3.0, ACCENT_GREEN);
                        resp.on_hover_text(acct_tip)
                    } else if state.is_authenticating {
                        let (rect, resp) = ui.allocate_exact_size(vec2(icon_sz + 8.0, icon_sz + 8.0), Sense::click());
                        let conn_tip = t!("topbar-account-connecting");
                        let conn_a11y = conn_tip.clone();
                        resp.widget_info(move || {
                            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, conn_a11y.clone())
                        });
                        ui.painter().text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            ICON_SYNC.codepoint,
                            egui::FontId::proportional(icon_sz),
                            tok.accent_primary,
                        );
                        resp.on_hover_text(conn_tip)
                    } else {
                        let acct_title = t!("topbar-account-title");
                        let acct_sub = t!("topbar-account-login-sub");
                        header_icon_btn(
                            ui,
                            ICON_PERSON.codepoint,
                            icon_sz,
                            state.account_drawer_open,
                            &acct_title,
                            None,
                            Some(&acct_sub),
                            None,
                            None,
                        )
                    };

                    state.account_button_rect = acct_btn_resp.rect;
                    if acct_btn_resp.clicked() {
                        event = Some(TopBarEvent::ToggleAccountDrawer);
                    }

                    ui.add_space(4.0);


                    // Sebelah kiri Settings: Chat AI (P13.3) — buka/tutup sidebar kanan.
                    let chat_title = t!("chat-title");
                    let chat_sub = t!("chat-open");
                    let chat_btn = header_icon_btn(
                        ui,
                        ICON_AUTO_AWESOME.codepoint,
                        icon_sz,
                        state.chat_open,
                        &chat_title,
                        Some("⌘⇧A"),
                        Some(&chat_sub),
                        None,
                        None,
                    );
                    crate::onboarding::Onboarding::publish_target(
                        ui.ctx(),
                        crate::onboarding::OnboardingTarget::ChatButton,
                        chat_btn.rect,
                    );
                    if chat_btn.clicked() {
                        event = Some(TopBarEvent::ToggleChat);
                    }

                    ui.add_space(item_gap);

                    // Tombol Quick Toggle Touch Design Mode (Apple Pencil vs Sentuhan Jari)
                    let touch_bg = if state.touch_config.mode == TouchDesignMode::PencilOnly {
                        Some(Color32::from_rgba_premultiplied(18, 42, 85, 120))
                    } else if state.touch_config.mode == TouchDesignMode::FingerDesign {
                        Some(Color32::from_rgba_premultiplied(28, 50, 28, 120))
                    } else {
                        None
                    };
                    let touch_stroke = if state.touch_config.mode == TouchDesignMode::PencilOnly {
                        Some(ACCENT_BLUE)
                    } else {
                        None
                    };
                    let touch_mode_label = state.touch_config.mode.label();
                    let touch_title = t!("touch-mode-tooltip", mode = touch_mode_label.as_str());
                    let touch_desc = state.touch_config.mode.description();
                    let touch_btn = header_icon_btn(
                        ui,
                        state.touch_config.mode.material_icon(),
                        icon_sz,
                        state.touch_config.mode != TouchDesignMode::PencilAndFinger,
                        &touch_title,
                        None,
                        Some(&touch_desc),
                        touch_bg,
                        touch_stroke,
                    );
                    if touch_btn.clicked() {
                        event = Some(TopBarEvent::CycleTouchDesignMode);
                    }

                    ui.add_space(item_gap);

                    // Paling kiri di kelompok kanan: buka tutorial selamat datang.
                    let help_title = t!("onboard-help-title");
                    let help_sub = t!("onboard-help-desc");
                    let help_btn = header_icon_btn(
                        ui,
                        ICON_HELP.codepoint,
                        icon_sz,
                        false,
                        &help_title,
                        None,
                        Some(&help_sub),
                        None,
                        None,
                    );
                    crate::onboarding::Onboarding::publish_target(
                        ui.ctx(),
                        crate::onboarding::OnboardingTarget::HelpButton,
                        help_btn.rect,
                    );
                    if help_btn.clicked() {
                        event = Some(TopBarEvent::StartTutorial);
                    }
                    ui.min_rect().left()
                }).inner;
                Self::publish_free_center_width(
                    ui.ctx(),
                    (right_cluster_start - left_cluster_end).max(0.0),
                );
            });
        });

        event
    }

    fn free_center_id() -> egui::Id {
        egui::Id::new("ducad-topbar-free-center-width")
    }

    /// Simpan lebar celah antara kelompok tombol kiri dan kanan (px). Tidak
    /// diberi nomor frame: nilai terakhir tetap berlaku saat top bar sedang
    /// disembunyikan, supaya keputusan "HUD muat atau tidak" stabil.
    fn publish_free_center_width(ctx: &egui::Context, width: f32) {
        ctx.data_mut(|d| d.insert_temp(Self::free_center_id(), width));
    }

    /// Lebar celah tengah top bar yang terakhir terukur; `None` bila top bar
    /// belum pernah digambar di konteks ini.
    pub fn last_free_center_width(ctx: &egui::Context) -> Option<f32> {
        ctx.data(|d| d.get_temp::<f32>(Self::free_center_id()))
    }
}

/// Isi submenu "Ekspor" di burger menu (dulu tombol Share/Export terpisah
/// di header kanan).
fn export_menu_items(ui: &mut Ui, event: &mut Option<TopBarEvent>) {
    let drawing_items = [
        (ICON_PICTURE_AS_PDF.codepoint, t!("menu-export-pdf"), TopBarFileOp::ExportPdf),
        (
            ICON_PICTURE_AS_PDF.codepoint,
            t!("menu-export-drawing-dxf"),
            TopBarFileOp::ExportDrawingDxf,
        ),
        (
            ICON_PICTURE_AS_PDF.codepoint,
            t!("menu-export-drawing-svg"),
            TopBarFileOp::ExportDrawingSvg,
        ),
        (
            ICON_PICTURE_AS_PDF.codepoint,
            t!("menu-export-view-dxf"),
            TopBarFileOp::ExportViewDxf,
        ),
        (
            ICON_PICTURE_AS_PDF.codepoint,
            t!("menu-export-vector-snapshot"),
            TopBarFileOp::ExportVectorSnapshot,
        ),
        (
            ICON_PICTURE_AS_PDF.codepoint,
            t!("menu-export-png"),
            TopBarFileOp::ExportPng,
        ),
    ];
    if ui
        .button(format!(
            "{} {}",
            ICON_PICTURE_AS_PDF.codepoint,
            t!("menu-drawing-sheet")
        ))
        .clicked()
    {
        *event = Some(TopBarEvent::OpenDrawingSheet);
        ui.close();
    }
    for (icon, label, op) in drawing_items {
        if ui.button(format!("{icon} {label}")).clicked() {
            *event = Some(TopBarEvent::File(op));
            ui.close();
        }
    }
    ui.separator();
    let cad_items = [
        (t!("menu-export-step"), TopBarFileOp::ExportStep),
        (t!("menu-export-iges"), TopBarFileOp::ExportIges),
        (t!("menu-export-3mf"), TopBarFileOp::ExportThreeMf),
        (t!("menu-export-stl"), TopBarFileOp::ExportStl),
        (t!("menu-export-obj"), TopBarFileOp::ExportObj),
        (t!("menu-export-glb"), TopBarFileOp::ExportGlb),
        (t!("menu-export-dxf"), TopBarFileOp::ExportDxf),
        (t!("menu-export-svg"), TopBarFileOp::ExportSvg),
    ];
    for (label, op) in cad_items {
        if ui.button(format!("{} {}", ICON_UPLOAD.codepoint, label)).clicked() {
            *event = Some(TopBarEvent::File(op));
            ui.close();
        }
    }
}

/// Isi submenu "Pengaturan" di burger menu: tema, Liquid Glass, label toolbar,
/// kurangi gerak, skala UI, bahasa, ukuran ikon, satuan, dan mode sentuh.
fn settings_menu_items(
    ui: &mut Ui,
    state: &TopBarState,
    icon_sz: f32,
    event: &mut Option<TopBarEvent>,
) {
    let tok = ThemeTokens::from_ui(ui);
    if ui
        .button(format!("{} {}", ICON_PALETTE.codepoint, t!("menu-theme")))
        .clicked()
    {
        *event = Some(TopBarEvent::ToggleTheme);
        ui.close();
    }
    let glass = ducad_glass::runtime(ui.ctx());
    let mut glass_on = glass.enabled;
    if ui
        .checkbox(
            &mut glass_on,
            format!("{} {}", ICON_BLUR_ON.codepoint, t!("menu-liquid-glass")),
        )
        .changed()
    {
        *event = Some(TopBarEvent::ToggleLiquidGlass);
    }
    let mut reduce = glass.reduce_transparency;
    if ui
        .add_enabled(
            glass.enabled,
            egui::Checkbox::new(
                &mut reduce,
                format!("{} {}", ICON_OPACITY.codepoint, t!("menu-reduce-transparency")),
            ),
        )
        .changed()
    {
        *event = Some(TopBarEvent::ToggleReduceTransparency);
    }
    let mut labels_on = state.show_toolbar_labels;
    if ui
        .checkbox(&mut labels_on, t!("settings-toolbar-labels"))
        .changed()
    {
        *event = Some(TopBarEvent::ToggleToolbarLabels);
    }
    let mut reduce_motion = state.reduce_motion;
    if ui
        .checkbox(&mut reduce_motion, t!("settings-reduce-motion"))
        .changed()
    {
        *event = Some(TopBarEvent::ToggleReduceMotion);
    }
    if ui
        .button(format!(
            "{} {} (⌘K / ⌘⇧P)",
            ICON_SEARCH.codepoint,
            t!("menu-command-palette")
        ))
        .clicked()
    {
        *event = Some(TopBarEvent::OpenCommandPalette);
        ui.close();
    }
    if state.updates_enabled {
        ui.separator();
        if ui
            .button(format!("{} {}", ICON_SYNC.codepoint, t!("menu-check-updates")))
            .clicked()
        {
            *event = Some(TopBarEvent::CheckForUpdates);
            ui.close();
        }
    }
    ui.separator();
    ui.menu_button(
        format!(
            "{} {} ({})",
            ICON_LANGUAGE.codepoint,
            t!("lang-current"),
            current_language().display_name()
        ),
        |ui| {
            crate::theme::glass_menu(ui, |ui| {
                for lang in Language::all() {
                    let is_sel = current_language() == *lang;
                    let prefix = if is_sel { "✔ " } else { "   " };
                    if ui
                        .button(format!("{}{}", prefix, lang.display_name()))
                        .clicked()
                    {
                        *event = Some(TopBarEvent::SetLanguage(*lang));
                        ui.close();
                    }
                }
            })
        },
    );
    ui.separator();
    ui.menu_button(
        format!(
            "{} ({:.0}%)",
            t!("settings-ui-scale"),
            (state.ui_scale * 100.0).round()
        ),
        |ui| {
            crate::theme::glass_menu(ui, |ui| {
                for scale in [1.0_f32, 1.15, 1.25, 1.5] {
                    let is_sel = (state.ui_scale - scale).abs() < 0.02;
                    let prefix = if is_sel { "✔ " } else { "   " };
                    if ui
                        .button(format!("{prefix}{:.0}%", scale * 100.0))
                        .clicked()
                    {
                        *event = Some(TopBarEvent::SetUiScale(scale));
                        ui.close();
                    }
                }
            })
        },
    );
    ui.menu_button(
        format!("{} ({:.0}px)", t!("settings-icon-size"), icon_sz),
        |ui| {
            crate::theme::glass_menu(ui, |ui| {
                for (label, size) in [
                    (t!("settings-icon-small"), 14.0),
                    (t!("settings-icon-medium"), 16.0),
                    (t!("settings-icon-standard"), 18.0),
                ] {
                    let is_sel = (icon_sz - size).abs() < 0.1;
                    let prefix = if is_sel { "✔ " } else { "   " };
                    if ui.button(format!("{}{}", prefix, label)).clicked() {
                        *event = Some(TopBarEvent::SetIconSize(size));
                        ui.close();
                    }
                }
            })
        },
    );
    ui.separator();
    ui.menu_button(
        format!(
            "{} ({})",
            t!("topbar-unit", unit = state.current_unit.suffix()),
            state.current_unit.suffix()
        ),
        |ui| {
            crate::theme::glass_menu(ui, |ui| {
                for unit in [
                    LengthUnit::Millimeters,
                    LengthUnit::Centimeters,
                    LengthUnit::Meters,
                    LengthUnit::Inches,
                ] {
                    let is_sel = state.current_unit == unit;
                    let prefix = if is_sel { "✔ " } else { "   " };
                    if ui
                        .button(format!("{}{}", prefix, localized_unit_label(unit)))
                        .clicked()
                    {
                        *event = Some(TopBarEvent::SetUnit(unit));
                        ui.close();
                    }
                }
            })
        },
    );
    ui.separator();
    let mode_lbl = state.touch_config.mode.label();
    ui.menu_button(
        format!(
            "{} {}",
            state.touch_config.mode.icon(),
            t!("settings-touch-menu", mode = mode_lbl.as_str())
        ),
        |ui| {
            crate::theme::glass_menu(ui, |ui| {
                ui.label(
                    RichText::new(t!("settings-touch-header"))
                        .strong()
                        .size(TYPOGRAPHY.caption)
                        .color(tok.text_secondary),
                );
                for m in [
                    TouchDesignMode::PencilAndFinger,
                    TouchDesignMode::PencilOnly,
                    TouchDesignMode::FingerDesign,
                ] {
                    let is_sel = state.touch_config.mode == m;
                    let prefix = if is_sel { "✔ " } else { "   " };
                    if ui
                        .button(format!("{}{}", prefix, m.label()))
                        .on_hover_text(m.description())
                        .clicked()
                    {
                        *event = Some(TopBarEvent::SetTouchDesignMode(m));
                        ui.close();
                    }
                }
            })
        },
    );
}

/// Tombol ikon kompak untuk header, dengan kartu tooltip hover berisi
/// title + shortcut opsional + subtitle dan informasi AccessKit.
#[allow(clippy::too_many_arguments)]
fn header_icon_btn(
    ui: &mut Ui,
    icon: &str,
    icon_size: f32,
    active: bool,
    title: &str,
    shortcut: Option<&str>,
    subtitle: Option<&str>,
    active_bg: Option<Color32>,
    active_fg: Option<Color32>,
) -> egui::Response {
    let tok = ThemeTokens::from_ui(ui);
    let (bg, icon_color) = if active {
        (
            active_bg.unwrap_or(tok.accent_primary),
            active_fg.unwrap_or(Color32::WHITE),
        )
    } else {
        (Color32::TRANSPARENT, active_fg.unwrap_or(tok.text_primary))
    };

    let btn = egui::Button::new(RichText::new(icon).size(icon_size).color(icon_color))
        .fill(bg)
        .corner_radius(CornerRadius::same(RADIUS.xs))
        .min_size(Vec2::new(icon_size + 8.0, icon_size + 6.0));
    let response = ui.add(btn);

    let a11y_label = match shortcut.filter(|s| !s.is_empty()) {
        Some(sc) => format!("{title} ({sc})"),
        None => title.to_string(),
    };
    response.widget_info(move || {
        egui::WidgetInfo::selected(egui::WidgetType::Button, true, active, a11y_label.clone())
    });

    response.on_hover_ui(|ui| {
        let tok = ThemeTokens::from_ui(ui);
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 2.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(title)
                    .strong()
                    .size(TYPOGRAPHY.body)
                    .color(tok.text_primary),
            );
            if let Some(sc) = shortcut {
                if !sc.is_empty() {
                    Frame::NONE
                        .fill(tok.bg_card)
                        .corner_radius(CornerRadius::same(RADIUS.xs))
                        .inner_margin(Margin::symmetric(5, 1))
                        .stroke(Stroke::new(0.5, tok.border_subtle))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(sc)
                                    .size(TYPOGRAPHY.caption)
                                    .strong()
                                    .color(tok.text_primary),
                            );
                        });
                }
            }
        });
        if let Some(sub) = subtitle {
            if !sub.is_empty() {
                ui.label(
                    RichText::new(sub)
                        .size(TYPOGRAPHY.caption)
                        .color(tok.text_secondary),
                );
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_zoom_percent_rounds_and_guards() {
        assert_eq!(format_zoom_percent(100.0), "100%");
        assert_eq!(format_zoom_percent(133.4), "133%");
        assert_eq!(format_zoom_percent(7.26), "7.3%");
        assert_eq!(format_zoom_percent(f32::NAN), "--%");
        assert_eq!(format_zoom_percent(f32::INFINITY), "--%");
    }

    #[test]
    fn zoom_presets_include_actual_size_and_ascend() {
        assert!(ZOOM_PRESETS.contains(&100.0));
        assert!(ZOOM_PRESETS.windows(2).all(|w| w[0] < w[1]));
    }
    use crate::theme::{apply, apply_with_touch, ThemeMode};

    fn dummy_state(mode: TouchDesignMode) -> TopBarState {
        let mut touch_config = TouchDesignConfig::default();
        touch_config.set_mode(mode);
        TopBarState {
            document_name: "Untitled.ducad".to_string(),
            status_saved: false,
            zoom_percent: 100.0,
            current_unit: LengthUnit::Millimeters,
            icon_size: crate::theme::ICON_SIZE_DEFAULT,
            is_sketching: false,
            items_drawer_open: false,
            section_view_active: false,
            is_measure_active: false,
            command_groups: Vec::new(),
            updates_enabled: true,
            zebra_view_active: false,
            studio_lighting_active: false,
            active_plane_name: "Top".to_string(),
            custom_planes: Vec::new(),
            plane_menu_open: false,
            items_button_rect: egui::Rect::NOTHING,
            account: None,
            account_avatar: None,
            is_authenticating: false,
            account_drawer_open: false,
            account_button_rect: egui::Rect::NOTHING,
            touch_config,
            is_ipad: false,
            checks_summary: None,
            ai_on_device: None,
            bridge_clients: None,
            checks_panel_open: false,
            chat_open: false,
            show_toolbar_labels: true,
            reduce_motion: false,
            ui_scale: 1.0,
        }
    }

    /// Tinggi header diukur setelah `TopBar::show` untuk satu mode sentuh.
    fn header_height(mode: TouchDesignMode) -> f32 {
        header_height_with(mode, None)
    }

    fn header_height_with(mode: TouchDesignMode, checks: Option<(usize, usize)>) -> f32 {
        let ctx = egui::Context::default();
        // Tiru `App` saat mode sentuh di-cycle: `apply_with_touch` mengubah
        // `interact_size.y` global ke 36/40/44 sesuai mode.
        let touch_target = dummy_state(mode).touch_config.touch_target_size;
        apply_with_touch(&ctx, ThemeMode::Dark, touch_target);
        let mut height = 0.0;
        // Dua frame: frame pertama memanaskan layout/font, frame kedua diukur.
        for _ in 0..2 {
            let mut output = ctx.run_ui(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.set_width(1200.0);
                    let mut state = dummy_state(mode);
                    state.checks_summary = checks;
                    let scope = ui.scope(|ui| {
                        TopBar::show(ui, &mut state);
                    });
                    height = scope.response.rect.height();
                });
            });
            output.textures_delta.clear();
        }
        height
    }

    /// Tombol ringkasan checks (P7.5) hanya muncul bila part punya check dan
    /// tidak boleh mengubah tinggi header.
    #[test]
    fn tombol_checks_tidak_mengubah_tinggi_header() {
        let without = header_height_with(TouchDesignMode::PencilAndFinger, None);
        let with = header_height_with(TouchDesignMode::PencilAndFinger, Some((3, 1)));
        assert!((without - with).abs() < 0.5, "{without} vs {with}");
    }

    /// Regresi: klik tombol quick-toggle mode sentuh sempat mengubah tinggi
    /// header karena `bar_h` diikat ke `touch_config.touch_target_size`
    /// (36/40/44 px per mode). Tinggi header harus sama di ketiga mode.
    #[test]
    fn tinggi_header_stabil_saat_mode_sentuh_di_cycle() {
        let hybrid = header_height(TouchDesignMode::PencilAndFinger);
        let pencil = header_height(TouchDesignMode::PencilOnly);
        let finger = header_height(TouchDesignMode::FingerDesign);

        assert!(hybrid > 0.0, "header tidak ter-render");
        assert_eq!(
            hybrid, pencil,
            "tinggi header berubah saat pindah ke PencilOnly"
        );
        assert_eq!(
            hybrid, finger,
            "tinggi header berubah saat pindah ke FingerDesign"
        );
    }

    /// Regresi: tombol mode sentuh sempat memakai emoji (`icon()`), yang
    /// dirender dari font fallback dengan metrik berbeda — `"\u{270f}\u{fe0f}+\u{1f446}"`
    /// bahkan melebar ke ~64 px vs 35 px tombol header lain. `material_icon()`
    /// harus menghasilkan tombol yang persis seukuran tombol header lainnya.
    #[test]
    fn tombol_mode_sentuh_seukuran_tombol_header_lain() {
        let ctx = egui::Context::default();
        apply(&ctx, ThemeMode::Dark);
        let mut sizes: Vec<(String, Vec2)> = Vec::new();
        // Dua frame: frame pertama memanaskan layout/font, frame kedua diukur.
        for _ in 0..2 {
            sizes.clear();
            let mut output = ctx.run_ui(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let mut measure = |label: &str, icon: &str| {
                        let rect =
                            header_icon_btn(ui, icon, 18.0, false, label, None, None, None, None)
                                .rect;
                        sizes.push((label.to_string(), rect.size()));
                    };
                    measure("settings", ICON_SETTINGS.codepoint);
                    measure("share", ICON_SHARE.codepoint);
                    for mode in [
                        TouchDesignMode::PencilAndFinger,
                        TouchDesignMode::PencilOnly,
                        TouchDesignMode::FingerDesign,
                    ] {
                        measure("touch-mode", mode.material_icon());
                    }
                });
            });
            output.textures_delta.clear();
        }

        let (_, baseline) = sizes[0].clone();
        assert!(baseline.x > 0.0, "tombol tidak ter-render");
        for (label, size) in &sizes {
            assert_eq!(
                *size, baseline,
                "tombol `{label}` ({size:?}) beda ukuran dari tombol header lain ({baseline:?})"
            );
        }
    }
}
