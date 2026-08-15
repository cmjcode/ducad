//! Modern Top Bar & Title Header bergaya Shapr3D dengan Material Icons.
//!
//! Menampilkan bar atas mengambang minimalis dengan nama dokumen,
//! indikator status sinkronisasi/simpan, tombol aksi Share/Export biru,
//! menu Berkas, dan Pengaturan.

use egui::{Color32, RichText, Ui};
use egui_material_icons::icons::{
    ICON_CLOUD, ICON_DOWNLOAD, ICON_FILE_OPEN, ICON_HOME, ICON_NOTE_ADD,
    ICON_PALETTE, ICON_SAVE, ICON_SEARCH, ICON_SETTINGS, ICON_SHARE, ICON_UPLOAD,
};
use crate::theme::{glass_frame, ACCENT_BLUE, TEXT_PRIMARY};

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
}

pub struct TopBar;

impl TopBar {
    /// Render modern top bar. Mengembalikan `Option<TopBarEvent>`.
    pub fn show(
        ui: &mut Ui,
        document_name: &str,
        status_saved: bool,
    ) -> Option<TopBarEvent> {
        let mut event = None;

        glass_frame().show(ui, |ui| {
            ui.set_height(32.0);
            ui.horizontal(|ui| {
                // 1. Home Button
                if ui.button(RichText::new(ICON_HOME).size(16.0)).on_hover_text("Dokumen Baru").clicked() {
                    event = Some(TopBarEvent::HomeClicked);
                }

                ui.add_space(2.0);

                // 2. Document Title & Cloud/File Status
                let cloud_color = if status_saved { ACCENT_BLUE } else { Color32::from_rgb(255, 180, 50) };
                
                ui.horizontal(|ui| {
                    ui.label(RichText::new(ICON_CLOUD).size(15.0).color(cloud_color));
                    ui.label(
                        RichText::new(document_name)
                            .strong()
                            .size(12.5)
                            .color(TEXT_PRIMARY),
                    );
                });

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                // 3. File Menu
                ui.menu_button("Berkas", |ui| {
                    if ui.button(format!("{} Dokumen Baru", ICON_NOTE_ADD)).clicked() {
                        event = Some(TopBarEvent::File(TopBarFileOp::New));
                        ui.close();
                    }
                    if ui.button(format!("{} Buka… (⌘O)", ICON_FILE_OPEN)).clicked() {
                        event = Some(TopBarEvent::File(TopBarFileOp::Open));
                        ui.close();
                    }
                    if ui.button(format!("{} Simpan (⌘S)", ICON_SAVE)).clicked() {
                        event = Some(TopBarEvent::File(TopBarFileOp::Save));
                        ui.close();
                    }
                    if ui.button(format!("{} Simpan Sebagai… (⌘⇧S)", ICON_SAVE)).clicked() {
                        event = Some(TopBarEvent::File(TopBarFileOp::SaveAs));
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(format!("{} Impor STEP…", ICON_DOWNLOAD)).clicked() {
                        event = Some(TopBarEvent::File(TopBarFileOp::ImportStep));
                        ui.close();
                    }
                    if ui.button(format!("{} Impor DXF (2D)…", ICON_DOWNLOAD)).clicked() {
                        event = Some(TopBarEvent::File(TopBarFileOp::ImportDxf));
                        ui.close();
                    }
                });

                // 4. Settings Menu
                ui.menu_button(format!("{} Pengaturan", ICON_SETTINGS), |ui| {
                    if ui.button(format!("{} Ganti Tema (Terang/Gelap)", ICON_PALETTE)).clicked() {
                        event = Some(TopBarEvent::ToggleTheme);
                        ui.close();
                    }
                    if ui.button(format!("{} Command Palette (⌘K)", ICON_SEARCH)).clicked() {
                        event = Some(TopBarEvent::OpenCommandPalette);
                        ui.close();
                    }
                });

                // 5. Right-aligned Export / Share Button
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.menu_button(
                        RichText::new(format!("{} Export / Share", ICON_SHARE))
                            .strong()
                            .size(11.5)
                            .color(Color32::WHITE),
                        |ui| {
                            if ui.button(format!("{} Ekspor STEP (Solid B-Rep)…", ICON_UPLOAD)).clicked() {
                                event = Some(TopBarEvent::File(TopBarFileOp::ExportStep));
                                ui.close();
                            }
                            if ui.button(format!("{} Ekspor STL (Mesh 3D)…", ICON_UPLOAD)).clicked() {
                                event = Some(TopBarEvent::File(TopBarFileOp::ExportStl));
                                ui.close();
                            }
                            if ui.button(format!("{} Ekspor OBJ (Mesh 3D)…", ICON_UPLOAD)).clicked() {
                                event = Some(TopBarEvent::File(TopBarFileOp::ExportObj));
                                ui.close();
                            }
                            if ui.button(format!("{} Ekspor DXF (Sketch 2D)…", ICON_UPLOAD)).clicked() {
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
