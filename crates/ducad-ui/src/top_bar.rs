//! Modern Top Bar & Title Header bergaya Shapr3D dengan Material Icons.
//!
//! Menampilkan bar atas mengambang minimalis dengan nama dokumen,
//! indikator status sinkronisasi/simpan, tombol aksi Share/Export biru,
//! menu Berkas, dan Pengaturan — plus (sejak reorganisasi toolbar) kontrol
//! yang selalu tersedia di kedua mode (Sketch & 3D): mode switcher, Items,
//! Search, Sketch Plane selector (khusus saat Sketch Mode), Section View,
//! Measurements, dan Delete. Kontrol yang hanya relevan di satu mode
//! (tool-tool sketsa 2D) tetap tinggal di `LeftToolbar`.

use crate::theme::{glass_frame, ACCENT_BLUE, BG_HOVER_DARK, BORDER_SUBTLE, TEXT_PRIMARY, TEXT_SECONDARY};
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
}

pub struct TopBar;

impl TopBar {
    /// Render modern top bar. Mengembalikan `Option<TopBarEvent>`.
    pub fn show(ui: &mut Ui, state: &mut TopBarState) -> Option<TopBarEvent> {
        let mut event = None;
        let icon_sz = state.icon_size.clamp(12.0, 18.0);

        glass_frame().show(ui, |ui| {
            ui.set_height((icon_sz + 14.0).max(30.0));
            ui.horizontal(|ui| {
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

                // 2. Document Title & Cloud/File Status
                let cloud_color = if state.status_saved {
                    ACCENT_BLUE
                } else {
                    Color32::from_rgb(255, 180, 50)
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
                        RichText::new(&state.document_name)
                            .strong()
                            .size(12.0)
                            .color(TEXT_PRIMARY),
                    );
                });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

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
                    Some("⌘K"),
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

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // 4. Utilities (Measurements & Zebra Inspection) — Tetap di sebelah File Name & Mode
                let meas_title = t!("hud-show-dimensions");
                let meas_sub = t!("hud-click-to-edit");
                let meas_btn = header_icon_btn(
                    ui,
                    ICON_STRAIGHTEN.codepoint,
                    icon_sz,
                    state.is_measure_active,
                    &meas_title,
                    Some("I"),
                    Some(&meas_sub),
                    None,
                    None,
                );
                if meas_btn.clicked() {
                    event = Some(TopBarEvent::ToggleMeasurements);
                }

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

                // Drawing Sheet Button (Fase 5)
                let ds_title = t!("topbar-drawing-sheet");
                let ds_sub = t!("topbar-drawing-sheet-tooltip");
                let ds_btn = header_icon_btn(
                    ui,
                    ICON_PICTURE_AS_PDF.codepoint,
                    icon_sz,
                    false,
                    &ds_title,
                    None,
                    Some(&ds_sub),
                    None,
                    None,
                );
                if ds_btn.clicked() {
                    event = Some(TopBarEvent::OpenDrawingSheet);
                }

                // Assembly Tree & Mates Button (Fase 12.2)
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
                                    "{} {} (⌘K)",
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
                        },
                    )
                    .response
                    .on_hover_text(t!("menu-settings"));

                    ui.add_space(4.0);

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
