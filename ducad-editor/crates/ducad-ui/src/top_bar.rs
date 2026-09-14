//! Modern Top Bar & Title Header bergaya Shapr3D dengan Material Icons.
//!
//! Menampilkan bar atas mengambang minimalis dengan nama dokumen,
//! indikator status sinkronisasi/simpan, tombol aksi Share/Export biru,
//! menu Berkas, dan Pengaturan — plus (sejak reorganisasi toolbar) kontrol
//! yang selalu tersedia di kedua mode (Sketch & 3D): mode switcher, Items,
//! Search, Sketch Plane selector (khusus saat Sketch Mode), Section View,
//! Measurements, dan Delete. Kontrol yang hanya relevan di satu mode
//! (tool-tool sketsa 2D) tetap tinggal di `LeftToolbar`.

use crate::theme::{
    glass_frame, ACCENT_BLUE, BG_HOVER_DARK, BORDER_SUBTLE, MIN_TOUCH_TARGET, TEXT_PRIMARY,
    TEXT_SECONDARY,
};
use crate::touch::{TouchDesignConfig, TouchDesignMode};
use ducad_cloud::DucadAccount;
use ducad_i18n::{current_language, t, Language};
use egui::{vec2, Align2, Color32, CornerRadius, Frame, Margin, RichText, Sense, Stroke, Ui, Vec2};
use egui_icons::icons::{
    ICON_CATEGORY, ICON_CLOUD, ICON_CUBE_OUTLINE, ICON_DOWNLOAD, ICON_EDIT, ICON_FILE_OPEN,
    ICON_LANGUAGE, ICON_LAYERS_OFF, ICON_MENU, ICON_NOTE_ADD, ICON_PALETTE, ICON_PERSON,
    ICON_PICTURE_AS_PDF, ICON_SAVE, ICON_SEARCH, ICON_SETTINGS, ICON_SHARE, ICON_STRAIGHTEN,
    ICON_TEXTURE, ICON_UPLOAD,
};

use ducad_core::LengthUnit;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopBarFileOp {
    New,
    Open,
    Save,
    SaveAs,
    ImportStep,
    ImportStl,
    ImportDxf,
    ExportStep,
    ExportStl,
    ExportObj,
    ExportGlb,
    ExportDxf,
    ExportSvg,
    ExportPdf,
    ExportDrawingDxf,
    ExportDrawingSvg,
    /// Tangkapan vektor 2D dari sudut pandang kamera viewport saat ini.
    ExportVectorSnapshot,
    OpenDrawingSheet,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TopBarEvent {
    HomeClicked,
    File(TopBarFileOp),
    ToggleTheme,
    OpenCommandPalette,
    SetUnit(LengthUnit),
    SetLanguage(Language),
    SetIconSize(f32),
    ToggleItemsDrawer,
    ToggleAssemblyDrawer,
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
    TogglePalmRejection,
    CycleTouchDesignMode,
}

/// State kontrol header yang dibaca & (untuk `plane_menu_open`) ditulis ulang
/// oleh `TopBar::show`. Dikonstruksi ulang tiap frame dari state `App`,
/// meniru pola `FeatureInspectorState` di `feature_inspector.rs`.
pub struct TopBarState {
    pub document_name: String,
    pub status_saved: bool,
    pub current_unit: LengthUnit,
    pub icon_size: f32,
    /// True saat Sketch Mode aktif — mengontrol apakah tombol Sketch Plane
    /// (dan popup pemilih bidangnya) ditampilkan sama sekali.
    pub is_sketching: bool,
    pub items_drawer_open: bool,
    pub assembly_drawer_open: bool,
    pub section_view_active: bool,
    pub is_measure_active: bool,
    pub zebra_view_active: bool,
    pub studio_lighting_active: bool,
    pub active_plane_name: String,
    pub custom_planes: Vec<(usize, String)>,
    /// Dropdown popup pemilih Sketch Plane (Top/Front/Right/Custom). Dibaca & bisa
    /// diubah oleh `show` — caller wajib menyalin nilai baru balik ke state
    /// persisten miliknya setelah `show` selesai (sama seperti field-field
    /// input lain di `FeatureInspectorState`).
    pub plane_menu_open: bool,
    /// Rect layar tombol Items setelah dirender frame ini — dipakai caller
    /// buat menempatkan popup Items Drawer tepat di bawah tombolnya.
    pub items_button_rect: egui::Rect,
    /// Akun pengguna CMJCode / Ducad jika terotentikasi
    pub account: Option<DucadAccount>,
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
}

pub struct TopBar;

impl TopBar {
    /// Render modern top bar. Mengembalikan `Option<TopBarEvent>`.
    pub fn show(ui: &mut Ui, state: &mut TopBarState) -> Option<TopBarEvent> {
        let mut event = None;
        let icon_sz = state.icon_size.clamp(12.0, 18.0);
        let avail_w = ui.available_width();
        let is_compact = avail_w < 920.0 || state.is_ipad;
        let is_tight = avail_w < 780.0;
        let item_gap = if is_tight { 2.0 } else { 4.0 };

        glass_frame().show(ui, |ui| {
            // Kunci tinggi minimum widget interaktif header ke nilai tetap.
            // `apply_with_touch` (dipanggil `App` tiap mode sentuh berganti)
            // mengubah `interact_size.y` GLOBAL ke 36/40/44 px per mode — dan itu
            // tinggi minimum semua tombol egui — sehingga tanpa kunci ini seluruh
            // tombol header dan tinggi bar ikut melompat tiap klik toggle mode.
            // Target sentuh besar tetap berlaku untuk UI lain di luar header.
            ui.spacing_mut().interact_size.y = MIN_TOUCH_TARGET;

            // Tinggi header sengaja TIDAK diikat ke `touch_config.touch_target_size`:
            // tombol header memakai `icon_size` (lihat `header_icon_btn`), jadi mengikat
            // bar ke touch target hanya bikin tingginya melompat 36/40/44 px tiap mode
            // sentuh di-cycle tanpa memperbesar area sentuh satu tombol pun.
            let bar_h = (icon_sz + 14.0).max(30.0);
            ui.set_height(bar_h);
            // `horizontal_centered` (bukan `horizontal`) supaya baris tombol dipusatkan
            // vertikal di dalam `bar_h`. `set_height` di atas mengunci max height ui,
            // jadi sisa ruang terbagi rata atas-bawah — bukan menumpuk di bawah baris
            // seperti `horizontal` yang rata-atas.
            ui.horizontal_centered(|ui| {
                // 1. Hamburger Menu Button (Three Lines) - New, Open, Save, Import
                ui.menu_button(
                    RichText::new(ICON_MENU.codepoint)
                        .size(icon_sz)
                        .color(TEXT_PRIMARY),
                    |ui| {
                        if ui
                            .button(format!("{} {}", ICON_NOTE_ADD.codepoint, t!("menu-new")))
                            .clicked()
                        {
                            event = Some(TopBarEvent::File(TopBarFileOp::New));
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
                        if ui
                            .button(format!(
                                "{} {} {}",
                                ICON_DOWNLOAD.codepoint,
                                t!("menu-import"),
                                t!("menu-import-step")
                            ))
                            .clicked()
                        {
                            event = Some(TopBarEvent::File(TopBarFileOp::ImportStep));
                            ui.close();
                        }
                        if ui
                            .button(format!(
                                "{} {} {}",
                                ICON_DOWNLOAD.codepoint,
                                t!("menu-import"),
                                t!("menu-import-stl")
                            ))
                            .clicked()
                        {
                            event = Some(TopBarEvent::File(TopBarFileOp::ImportStl));
                            ui.close();
                        }
                        if ui
                            .button(format!(
                                "{} {} {}",
                                ICON_DOWNLOAD.codepoint,
                                t!("menu-import"),
                                t!("menu-import-dxf")
                            ))
                            .clicked()
                        {
                            event = Some(TopBarEvent::File(TopBarFileOp::ImportDxf));
                            ui.close();
                        }
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
                    },
                )
                .response
                .on_hover_text(t!("menu-file"));

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
                            .size(12.0)
                            .color(TEXT_PRIMARY),
                    )
                    .on_hover_text(&state.document_name);
                });

                ui.add_space(item_gap);
                ui.separator();
                ui.add_space(item_gap);

                // 3. Mode Switcher + Items + Search + Sketch Plane (Tetap di sebelah File Name)
                let (mode_icon, mode_title, mode_shortcut, mode_sub) = if state.is_sketching {
                    (
                        ICON_EDIT.codepoint,
                        t!("topbar-sketch-mode"),
                        "⌘+Shift+3",
                        t!("topbar-switch-to-solid"),
                    )
                } else {
                    (
                        ICON_CUBE_OUTLINE.codepoint,
                        t!("topbar-solid-mode"),
                        "⌘+Shift+2",
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
                                    ui.set_width(170.0);
                                    ui.spacing_mut().item_spacing = Vec2::new(2.0, 4.0);
                                    ui.label(
                                        RichText::new(t!("topbar-sketch-plane", plane = ""))
                                            .strong()
                                            .size(10.0)
                                            .color(TEXT_SECONDARY),
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
                                            .size(11.5),
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
                                                .size(9.5)
                                                .color(TEXT_SECONDARY),
                                        );
                                        for (idx, name) in &state.custom_planes {
                                            let plane_active = state.active_plane_name.contains(name);
                                            let btn = ui.selectable_label(
                                                plane_active,
                                                RichText::new(format!(
                                                    "{} {}",
                                                    ICON_LAYERS_OFF.codepoint, name
                                                ))
                                                .size(11.0),
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
                                            .size(10.5)
                                            .color(ACCENT_BLUE),
                                    );
                                    if new_plane_btn.clicked() {
                                        event = Some(TopBarEvent::CreateDatumPlane);
                                        state.plane_menu_open = false;
                                    }

                                    let manage_btn = ui.button(
                                        RichText::new(format!("{} {}", ICON_LAYERS_OFF.codepoint, t!("planes-drawer-title")))
                                            .size(10.5)
                                            .color(TEXT_PRIMARY),
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
                        Some("D"),
                        Some(&ds_sub),
                        None,
                        None,
                    );
                    if ds_btn.clicked() {
                        event = Some(TopBarEvent::OpenDrawingSheet);
                    }

                    // 7. Tombol Assembly Mating & Joint Drawer (Pohon Rakitan)
                    let assem_title = t!("assembly-tree-title");
                    let assem_sub = t!("topbar-assembly-tooltip");
                    let assem_btn = header_icon_btn(
                        ui,
                        ICON_CATEGORY.codepoint,
                        icon_sz,
                        state.assembly_drawer_open,
                        &assem_title,
                        None,
                        Some(&assem_sub),
                        None,
                        None,
                    );
                    if assem_btn.clicked() {
                        event = Some(TopBarEvent::ToggleAssemblyDrawer);
                    }
                } else {
                    ui.add_space(2.0);
                    ui.menu_button(
                        RichText::new(ICON_CATEGORY.codepoint).size(icon_sz).color(TEXT_PRIMARY),
                        |ui| {
                            ui.set_min_width(170.0);
                            let m_title = t!("hud-show-dimensions");
                            let m_chk = if state.is_measure_active { "✓ " } else { "  " };
                            if ui.button(format!("{}{} {}", m_chk, ICON_STRAIGHTEN.codepoint, m_title)).clicked() {
                                event = Some(TopBarEvent::ToggleMeasurements);
                                ui.close();
                            }
                            let z_title = t!("tool-zebra-stripes");
                            let z_chk = if state.zebra_view_active { "✓ " } else { "  " };
                            if ui.button(format!("{}{} {}", z_chk, ICON_TEXTURE.codepoint, z_title)).clicked() {
                                event = Some(TopBarEvent::ToggleZebraView);
                                ui.close();
                            }
                            let ds_title = t!("topbar-drawing-sheet");
                            if ui.button(format!("  {} {}", ICON_PICTURE_AS_PDF.codepoint, ds_title)).clicked() {
                                event = Some(TopBarEvent::OpenDrawingSheet);
                                ui.close();
                            }
                            let assem_title = t!("assembly-tree-title");
                            let a_chk = if state.assembly_drawer_open { "✓ " } else { "  " };
                            if ui.button(format!("{}{} {}", a_chk, ICON_CATEGORY.codepoint, assem_title)).clicked() {
                                event = Some(TopBarEvent::ToggleAssemblyDrawer);
                                ui.close();
                            }
                        },
                    ).response.on_hover_text("Menu Alat Tambahan (Ukuran, Zebra, Gambar 2D, Assembly)");
                }

                // 5. Right-aligned Settings and Export Buttons (Minimalist Icon-Only)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Sisi paling kanan: Tombol Akun CMJCode / Cloud
                    let acct_btn_resp = if let Some(acc) = &state.account {
                        let (rect, resp) = ui.allocate_exact_size(vec2(icon_sz + 8.0, icon_sz + 8.0), Sense::click());
                        if resp.hovered() {
                            ui.painter().rect_filled(rect, CornerRadius::same(14), BG_HOVER_DARK);
                        }
                        ui.painter().circle_filled(rect.center(), (icon_sz + 4.0) / 2.0, Color32::from_rgb(30, 58, 138));
                        ui.painter().circle_stroke(
                            rect.center(),
                            (icon_sz + 4.0) / 2.0,
                            Stroke::new(1.0, Color32::from_rgb(56, 189, 248)),
                        );
                        ui.painter().text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            acc.initials(),
                            egui::FontId::proportional(icon_sz - 3.0),
                            Color32::WHITE,
                        );
                        // Dot aktif
                        ui.painter().circle_filled(
                            rect.right_bottom() - vec2(2.0, 2.0),
                            3.0,
                            Color32::from_rgb(74, 222, 128),
                        );
                        resp.on_hover_text(format!("Akun CMJCode: {} ({})", acc.display_title(), acc.email))
                    } else if state.is_authenticating {
                        let (rect, resp) = ui.allocate_exact_size(vec2(icon_sz + 8.0, icon_sz + 8.0), Sense::click());
                        ui.painter().text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            "⏳",
                            egui::FontId::proportional(icon_sz),
                            ACCENT_BLUE,
                        );
                        resp.on_hover_text("Menghubungkan ke browser...")
                    } else {
                        header_icon_btn(
                            ui,
                            ICON_PERSON.codepoint,
                            icon_sz,
                            state.account_drawer_open,
                            "Akun CMJCode",
                            None,
                            Some("Masuk ke Cloud / SSO"),
                            None,
                            None,
                        )
                    };

                    state.account_button_rect = acct_btn_resp.rect;
                    if acct_btn_resp.clicked() {
                        event = Some(TopBarEvent::ToggleAccountDrawer);
                    }

                    ui.add_space(4.0);

                    // Sisi sebelah kiri Akun: Settings Icon Button
                    ui.menu_button(
                        RichText::new(ICON_SETTINGS.codepoint)
                            .size(icon_sz)
                            .color(TEXT_PRIMARY),
                        |ui| {
                            if ui
                                .button(format!("{} {}", ICON_PALETTE.codepoint, t!("menu-theme")))
                                .clicked()
                            {
                                event = Some(TopBarEvent::ToggleTheme);
                                ui.close();
                            }
                            if ui
                                .button(format!(
                                    "{} {} (⌘K / ⌘⇧P)",
                                    ICON_SEARCH.codepoint,
                                    t!("menu-command-palette")
                                 ))
                                .clicked()
                            {
                                event = Some(TopBarEvent::OpenCommandPalette);
                                ui.close();
                            }
                            ui.separator();
                            // Language selector
                            ui.menu_button(
                                format!(
                                    "{} {} ({})",
                                    ICON_LANGUAGE.codepoint,
                                    t!("lang-current"),
                                    current_language().display_name()
                                ),
                                |ui| {
                                    for lang in Language::all() {
                                        let is_sel = current_language() == *lang;
                                        let prefix = if is_sel { "✓ " } else { "   " };
                                        if ui
                                            .button(format!("{}{}", prefix, lang.display_name()))
                                            .clicked()
                                        {
                                            event = Some(TopBarEvent::SetLanguage(*lang));
                                            ui.close();
                                        }
                                    }
                                },
                            );
                            ui.separator();
                            // Icon size selector
                            ui.menu_button(
                                format!(
                                    "🔘 {} ({:.0}px)",
                                    t!("settings-icon-size"),
                                    icon_sz
                                ),
                                |ui| {
                                    for (label, size) in [
                                        ("14 px (Kecil)", 14.0),
                                        ("16 px (Sedang)", 16.0),
                                        ("18 px (Standar)", 18.0),
                                    ] {
                                        let is_sel = (icon_sz - size).abs() < 0.1;
                                        let prefix = if is_sel { "✓ " } else { "   " };
                                        if ui.button(format!("{}{}", prefix, label)).clicked() {
                                            event = Some(TopBarEvent::SetIconSize(size));
                                            ui.close();
                                        }
                                    }
                                },
                            );
                            ui.separator();
                            ui.menu_button(
                                format!(
                                    "📏 {} ({})",
                                    t!("topbar-unit", unit = state.current_unit.suffix()),
                                    state.current_unit.suffix()
                                ),
                                |ui| {
                                    for unit in [
                                        LengthUnit::Millimeters,
                                        LengthUnit::Centimeters,
                                        LengthUnit::Meters,
                                        LengthUnit::Inches,
                                    ] {
                                        let is_sel = state.current_unit == unit;
                                        let prefix = if is_sel { "✓ " } else { "   " };
                                        if ui
                                            .button(format!("{}{}", prefix, unit.label()))
                                            .clicked()
                                        {
                                            event = Some(TopBarEvent::SetUnit(unit));
                                            ui.close();
                                        }
                                    }
                                },
                            );
                            ui.separator();
                            // Konfigurasi Touch Design (Apple Pencil vs Sentuhan Jari)
                            ui.menu_button(
                                format!(
                                    "{} Mode Sentuh / Pencil ({})",
                                    state.touch_config.mode.icon(),
                                    state.touch_config.mode.label()
                                ),
                                |ui| {
                                    ui.label(
                                        RichText::new("Interaksi Layar Sentuh & Stylus:")
                                            .strong()
                                            .size(10.5)
                                            .color(TEXT_SECONDARY),
                                    );
                                    for m in [
                                        TouchDesignMode::PencilAndFinger,
                                        TouchDesignMode::PencilOnly,
                                        TouchDesignMode::FingerDesign,
                                    ] {
                                        let is_sel = state.touch_config.mode == m;
                                        let prefix = if is_sel { "✓ " } else { "   " };
                                        if ui
                                            .button(format!("{}{}", prefix, m.label()))
                                            .on_hover_text(m.description())
                                            .clicked()
                                        {
                                            event = Some(TopBarEvent::SetTouchDesignMode(m));
                                            ui.close();
                                        }
                                    }
                                    ui.separator();
                                    let mut pr = state.touch_config.palm_rejection;
                                    if ui
                                        .checkbox(&mut pr, "🛡️ Tolak Telapak Tangan (Palm Rejection)")
                                        .on_hover_text("Mengabaikan sentuhan telapak tangan saat menggambar dengan Apple Pencil")
                                        .clicked()
                                    {
                                        event = Some(TopBarEvent::TogglePalmRejection);
                                        ui.close();
                                    }
                                },
                            );
                        },
                    )
                    .response
                    .on_hover_text(t!("menu-settings"));

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
                    let touch_btn = header_icon_btn(
                        ui,
                        state.touch_config.mode.material_icon(),
                        icon_sz,
                        state.touch_config.mode != TouchDesignMode::PencilAndFinger,
                        &format!("Mode Sentuh: {}", state.touch_config.mode.label()),
                        None,
                        Some(state.touch_config.mode.description()),
                        touch_bg,
                        touch_stroke,
                    );
                    if touch_btn.clicked() {
                        event = Some(TopBarEvent::CycleTouchDesignMode);
                    }

                    ui.add_space(item_gap);

                    // Sebelah kiri Settings: Export / Share Icon Button
                    ui.menu_button(
                        RichText::new(ICON_SHARE.codepoint)
                            .size(icon_sz)
                            .color(ACCENT_BLUE),
                        |ui| {
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
                            if ui
                                .button(format!(
                                    "{} {}",
                                    ICON_PICTURE_AS_PDF.codepoint,
                                    t!("menu-export-pdf")
                                ))
                                .clicked()
                            {
                                event = Some(TopBarEvent::File(TopBarFileOp::ExportPdf));
                                ui.close();
                            }
                            if ui
                                .button(format!(
                                    "{} {}",
                                    ICON_PICTURE_AS_PDF.codepoint,
                                    t!("menu-export-drawing-svg")
                                ))
                                .clicked()
                            {
                                event = Some(TopBarEvent::File(TopBarFileOp::ExportDrawingSvg));
                                ui.close();
                            }
                            if ui
                                .button(format!(
                                    "{} {}",
                                    ICON_PICTURE_AS_PDF.codepoint,
                                    t!("menu-export-vector-snapshot")
                                ))
                                .clicked()
                            {
                                event =
                                    Some(TopBarEvent::File(TopBarFileOp::ExportVectorSnapshot));
                                ui.close();
                            }
                            ui.separator();
                            if ui
                                .button(format!(
                                    "{} {}",
                                    ICON_UPLOAD.codepoint,
                                    t!("menu-export-step")
                                ))
                                .clicked()
                            {
                                event = Some(TopBarEvent::File(TopBarFileOp::ExportStep));
                                ui.close();
                            }
                            if ui
                                .button(format!(
                                    "{} {}",
                                    ICON_UPLOAD.codepoint,
                                    t!("menu-export-stl")
                                ))
                                .clicked()
                            {
                                event = Some(TopBarEvent::File(TopBarFileOp::ExportStl));
                                ui.close();
                            }
                            if ui
                                .button(format!(
                                    "{} {}",
                                    ICON_UPLOAD.codepoint,
                                    t!("menu-export-obj")
                                ))
                                .clicked()
                            {
                                event = Some(TopBarEvent::File(TopBarFileOp::ExportObj));
                                ui.close();
                            }
                            if ui
                                .button(format!(
                                    "{} {}",
                                    ICON_UPLOAD.codepoint,
                                    t!("menu-export-glb")
                                ))
                                .clicked()
                            {
                                event = Some(TopBarEvent::File(TopBarFileOp::ExportGlb));
                                ui.close();
                            }
                            if ui
                                .button(format!(
                                    "{} {}",
                                    ICON_UPLOAD.codepoint,
                                    t!("menu-export-dxf")
                                ))
                                .clicked()
                            {
                                event = Some(TopBarEvent::File(TopBarFileOp::ExportDxf));
                                ui.close();
                            }
                            if ui
                                .button(format!(
                                    "{} {}",
                                    ICON_UPLOAD.codepoint,
                                    t!("menu-export-svg")
                                ))
                                .clicked()
                            {
                                event = Some(TopBarEvent::File(TopBarFileOp::ExportSvg));
                                ui.close();
                            }
                        },
                    )
                    .response
                    .on_hover_text(t!("topbar-share"));
                });
            });
        });

        event
    }
}

/// Tombol ikon kompak untuk header, dengan kartu tooltip hover berisi
/// title + shortcut opsional + subtitle. Meniru gaya & semantik parameter
/// `square_btn` di `left_toolbar.rs` (custom_bg/custom_fg dipakai hanya saat
/// `active`), tapi berukuran lebih pendek supaya muat di satu baris header.
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
    let (bg, icon_color) = if active {
        (
            active_bg.unwrap_or(ACCENT_BLUE),
            active_fg.unwrap_or(Color32::WHITE),
        )
    } else {
        (Color32::TRANSPARENT, active_fg.unwrap_or(TEXT_PRIMARY))
    };

    let btn = egui::Button::new(RichText::new(icon).size(icon_size).color(icon_color))
        .fill(bg)
        .corner_radius(CornerRadius::same(5))
        .min_size(Vec2::new(icon_size + 8.0, icon_size + 6.0));
    let response = ui.add(btn);

    response.on_hover_ui(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 2.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(title)
                    .strong()
                    .size(12.0)
                    .color(Color32::WHITE),
            );
            if let Some(sc) = shortcut {
                if !sc.is_empty() {
                    Frame::NONE
                        .fill(Color32::from_rgba_premultiplied(50, 54, 65, 230))
                        .corner_radius(CornerRadius::same(4))
                        .inner_margin(Margin::symmetric(4, 1))
                        .stroke(Stroke::new(0.5, BORDER_SUBTLE))
                        .show(ui, |ui| {
                            ui.label(RichText::new(sc).size(9.5).strong().color(TEXT_PRIMARY));
                        });
                }
            }
        });
        if let Some(sub) = subtitle {
            if !sub.is_empty() {
                ui.label(RichText::new(sub).size(10.0).color(TEXT_SECONDARY));
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{apply, apply_with_touch, ThemeMode};

    fn dummy_state(mode: TouchDesignMode) -> TopBarState {
        let mut touch_config = TouchDesignConfig::default();
        touch_config.set_mode(mode);
        TopBarState {
            document_name: "Untitled.ducad".to_string(),
            status_saved: false,
            current_unit: LengthUnit::Millimeters,
            icon_size: crate::theme::ICON_SIZE_DEFAULT,
            is_sketching: false,
            items_drawer_open: false,
            assembly_drawer_open: false,
            section_view_active: false,
            is_measure_active: false,
            zebra_view_active: false,
            studio_lighting_active: false,
            active_plane_name: "Top".to_string(),
            custom_planes: Vec::new(),
            plane_menu_open: false,
            items_button_rect: egui::Rect::NOTHING,
            account: None,
            is_authenticating: false,
            account_drawer_open: false,
            account_button_rect: egui::Rect::NOTHING,
            touch_config,
            is_ipad: false,
        }
    }

    /// Tinggi header diukur setelah `TopBar::show` untuk satu mode sentuh.
    fn header_height(mode: TouchDesignMode) -> f32 {
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
