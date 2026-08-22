//! Modern Top Bar & Title Header bergaya Shapr3D dengan Material Icons.
//!
//! Menampilkan bar atas mengambang minimalis dengan nama dokumen,
//! indikator status sinkronisasi/simpan, tombol aksi Share/Export biru,
//! menu Berkas, dan Pengaturan — plus (sejak reorganisasi toolbar) kontrol
//! yang selalu tersedia di kedua mode (Sketch & 3D): mode switcher, Items,
//! Search, Sketch Plane selector (khusus saat Sketch Mode), Section View,
//! Measurements, dan Delete. Kontrol yang hanya relevan di satu mode
//! (tool-tool sketsa 2D) tetap tinggal di `LeftToolbar`.

use crate::theme::{glass_frame, ACCENT_BLUE, BORDER_SUBTLE, TEXT_PRIMARY, TEXT_SECONDARY};
use egui::{Color32, CornerRadius, Frame, Margin, RichText, Stroke, Ui, Vec2};
use egui_material_icons::icons::{
    ICON_CLOUD, ICON_CUBE_OUTLINE, ICON_DOWNLOAD, ICON_EDIT, ICON_FILE_OPEN, ICON_HOME,
    ICON_LAYERS, ICON_NOTE_ADD, ICON_PALETTE, ICON_SAVE, ICON_SEARCH, ICON_SETTINGS, ICON_SHARE,
    ICON_STRAIGHTEN, ICON_UPLOAD,
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
    ExportDxf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopBarEvent {
    HomeClicked,
    File(TopBarFileOp),
    ToggleTheme,
    OpenCommandPalette,
    SetUnit(LengthUnit),
    ToggleItemsDrawer,
    OpenSearch,
    EnterSketching,
    ExitSketching,
    SelectSketchPlane(usize),
    ToggleSectionView,
    ToggleMeasurements,
    DeleteSelection,
}

/// State kontrol header yang dibaca & (untuk `plane_menu_open`) ditulis ulang
/// oleh `TopBar::show`. Dikonstruksi ulang tiap frame dari state `App`,
/// meniru pola `FeatureInspectorState` di `feature_inspector.rs`.
pub struct TopBarState {
    pub document_name: String,
    pub status_saved: bool,
    pub current_unit: LengthUnit,
    /// True saat Sketch Mode aktif — mengontrol apakah tombol Sketch Plane
    /// (dan popup pemilih bidangnya) ditampilkan sama sekali.
    pub is_sketching: bool,
    pub items_drawer_open: bool,
    pub section_view_active: bool,
    pub is_measure_active: bool,
    pub active_plane_name: String,
    /// Dropdown popup pemilih Sketch Plane (Top/Front/Right). Dibaca & bisa
    /// diubah oleh `show` — caller wajib menyalin nilai baru balik ke state
    /// persisten miliknya setelah `show` selesai (sama seperti field-field
    /// input lain di `FeatureInspectorState`).
    pub plane_menu_open: bool,
    /// Rect layar tombol Items setelah dirender frame ini — dipakai caller
    /// buat menempatkan popup Items Drawer tepat di bawah tombolnya.
    pub items_button_rect: egui::Rect,
}

pub struct TopBar;

impl TopBar {
    /// Render modern top bar. Mengembalikan `Option<TopBarEvent>`.
    pub fn show(ui: &mut Ui, state: &mut TopBarState) -> Option<TopBarEvent> {
        let mut event = None;

        glass_frame().show(ui, |ui| {
            ui.set_height(30.0);
            ui.horizontal(|ui| {
                // 1. Home Button
                if ui
                    .button(RichText::new(ICON_HOME.codepoint).size(14.0))
                    .on_hover_text("Dokumen Baru")
                    .clicked()
                {
                    event = Some(TopBarEvent::HomeClicked);
                }

                ui.add_space(2.0);

                // 2. Document Title & Cloud/File Status
                let cloud_color = if state.status_saved {
                    ACCENT_BLUE
                } else {
                    Color32::from_rgb(255, 180, 50)
                };

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(ICON_CLOUD.codepoint)
                            .size(14.0)
                            .color(cloud_color),
                    );
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

                // 3. Mode Switcher + Items + Search + Sketch Plane (Selalu Ada
                // di Kedua Mode, Kecuali Sketch Plane yang Khusus Sketch Mode)
                let (mode_icon, mode_title, mode_shortcut, mode_sub) = if state.is_sketching {
                    (
                        ICON_EDIT.codepoint,
                        "Sketch Mode",
                        "⌘⇧3",
                        "Klik atau ⌘⇧3 untuk ke Mode 3D",
                    )
                } else {
                    (
                        ICON_CUBE_OUTLINE.codepoint,
                        "3D Mode",
                        "⌘⇧2",
                        "Klik atau ⌘⇧2 untuk ke Mode Sketch",
                    )
                };
                let mode_btn = header_icon_btn(
                    ui,
                    mode_icon,
                    true,
                    mode_title,
                    Some(mode_shortcut),
                    Some(mode_sub),
                    Some(Color32::from_rgba_premultiplied(18, 42, 85, 220)),
                    Some(ACCENT_BLUE),
                );
                if mode_btn.clicked() {
                    event = Some(if state.is_sketching {
                        TopBarEvent::ExitSketching
                    } else {
                        TopBarEvent::EnterSketching
                    });
                }

                let search_btn = header_icon_btn(
                    ui,
                    ICON_SEARCH.codepoint,
                    false,
                    "Search",
                    Some("⌘K"),
                    Some("Cari tool & perintah"),
                    None,
                    None,
                );
                if search_btn.clicked() {
                    event = Some(TopBarEvent::OpenSearch);
                }

                if state.is_sketching {
                    let plane_btn = header_icon_btn(
                        ui,
                        ICON_LAYERS.codepoint,
                        state.plane_menu_open,
                        "Sketch Plane",
                        None,
                        Some(state.active_plane_name.as_str()),
                        Some(Color32::from_rgba_premultiplied(18, 42, 85, 220)),
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
                                        RichText::new("PILIH BIDANG SKETSA")
                                            .strong()
                                            .size(10.0)
                                            .color(TEXT_SECONDARY),
                                    );
                                    ui.separator();

                                    let planes = [
                                        (0, "Top Plane (XY)", "Bidang Horizontal"),
                                        (1, "Front Plane (XZ)", "Bidang Vertikal Depan"),
                                        (2, "Right Plane (YZ)", "Bidang Vertikal Samping"),
                                    ];

                                    for (idx, label, sub) in planes {
                                        let plane_active =
                                            state.active_plane_name.contains(&label[..5]);
                                        let btn = ui.selectable_label(
                                            plane_active,
                                            RichText::new(format!(
                                                "{} {}",
                                                ICON_LAYERS.codepoint, label
                                            ))
                                            .size(11.5),
                                        );
                                        if btn.on_hover_text(sub).clicked() {
                                            event = Some(TopBarEvent::SelectSketchPlane(idx));
                                            state.plane_menu_open = false;
                                        }
                                    }
                                });
                            });
                    }
                }

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // 4. Utilities (Measurements, Delete) — Selalu
                // Ada di Kedua Mode

                let meas_btn = header_icon_btn(
                    ui,
                    ICON_STRAIGHTEN.codepoint,
                    state.is_measure_active,
                    "Tampilkan Semua Ukuran",
                    Some("I"),
                    Some("Tampilkan angka nominal dimensi tiap garis/rusuk di kanvas"),
                    None,
                    None,
                );
                if meas_btn.clicked() {
                    event = Some(TopBarEvent::ToggleMeasurements);
                }

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // 5. File Menu
                ui.menu_button(RichText::new("Berkas").size(11.5), |ui| {
                    if ui
                        .button(format!("{} Dokumen Baru", ICON_NOTE_ADD.codepoint))
                        .clicked()
                    {
                        event = Some(TopBarEvent::File(TopBarFileOp::New));
                        ui.close();
                    }
                    if ui
                        .button(format!("{} Buka… (⌘O)", ICON_FILE_OPEN.codepoint))
                        .clicked()
                    {
                        event = Some(TopBarEvent::File(TopBarFileOp::Open));
                        ui.close();
                    }
                    if ui
                        .button(format!("{} Simpan (⌘S)", ICON_SAVE.codepoint))
                        .clicked()
                    {
                        event = Some(TopBarEvent::File(TopBarFileOp::Save));
                        ui.close();
                    }
                    if ui
                        .button(format!("{} Simpan Sebagai… (⌘⇧S)", ICON_SAVE.codepoint))
                        .clicked()
                    {
                        event = Some(TopBarEvent::File(TopBarFileOp::SaveAs));
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .button(format!("{} Impor STEP…", ICON_DOWNLOAD.codepoint))
                        .clicked()
                    {
                        event = Some(TopBarEvent::File(TopBarFileOp::ImportStep));
                        ui.close();
                    }
                    if ui
                        .button(format!("{} Impor DXF (2D)…", ICON_DOWNLOAD.codepoint))
                        .clicked()
                    {
                        event = Some(TopBarEvent::File(TopBarFileOp::ImportDxf));
                        ui.close();
                    }
                });

                // 6. Settings Menu
                ui.menu_button(
                    RichText::new(format!("{} Pengaturan", ICON_SETTINGS.codepoint)).size(11.5),
                    |ui| {
                        if ui
                            .button(format!(
                                "{} Ganti Tema (Terang/Gelap)",
                                ICON_PALETTE.codepoint
                            ))
                            .clicked()
                        {
                            event = Some(TopBarEvent::ToggleTheme);
                            ui.close();
                        }
                        if ui
                            .button(format!("{} Command Palette (⌘K)", ICON_SEARCH.codepoint))
                            .clicked()
                        {
                            event = Some(TopBarEvent::OpenCommandPalette);
                            ui.close();
                        }
                        ui.separator();
                        ui.menu_button(
                            format!("📏 Satuan ({})", state.current_unit.suffix()),
                            |ui| {
                                for unit in [
                                    LengthUnit::Millimeters,
                                    LengthUnit::Centimeters,
                                    LengthUnit::Meters,
                                    LengthUnit::Inches,
                                ] {
                                    let is_sel = state.current_unit == unit;
                                    let prefix = if is_sel { "✓ " } else { "   " };
                                    if ui.button(format!("{}{}", prefix, unit.label())).clicked() {
                                        event = Some(TopBarEvent::SetUnit(unit));
                                        ui.close();
                                    }
                                }
                            },
                        );
                    },
                );

                // 7. Right-aligned Export / Share Button (Shapr3D blue accent)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.menu_button(
                        RichText::new(format!("{} Share", ICON_SHARE.codepoint))
                            .strong()
                            .size(11.5)
                            .color(Color32::WHITE),
                        |ui| {
                            if ui
                                .button(format!(
                                    "{} Ekspor STEP (Solid B-Rep)…",
                                    ICON_UPLOAD.codepoint
                                ))
                                .clicked()
                            {
                                event = Some(TopBarEvent::File(TopBarFileOp::ExportStep));
                                ui.close();
                            }
                            if ui
                                .button(format!("{} Ekspor STL (Mesh 3D)…", ICON_UPLOAD.codepoint))
                                .clicked()
                            {
                                event = Some(TopBarEvent::File(TopBarFileOp::ExportStl));
                                ui.close();
                            }
                            if ui
                                .button(format!("{} Ekspor OBJ (Mesh 3D)…", ICON_UPLOAD.codepoint))
                                .clicked()
                            {
                                event = Some(TopBarEvent::File(TopBarFileOp::ExportObj));
                                ui.close();
                            }
                            if ui
                                .button(format!(
                                    "{} Ekspor DXF (Sketch 2D)…",
                                    ICON_UPLOAD.codepoint
                                ))
                                .clicked()
                            {
                                event = Some(TopBarEvent::File(TopBarFileOp::ExportDxf));
                                ui.close();
                            }
                        },
                    );
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

    let btn = egui::Button::new(RichText::new(icon).size(14.0).color(icon_color))
        .fill(bg)
        .corner_radius(CornerRadius::same(5))
        .min_size(Vec2::new(24.0, 22.0));
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
