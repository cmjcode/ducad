//! Floating Contextual Action Bar bergaya Shapr3D dengan Material Icons.
//!
//! Menampilkan bar aksi mengambang adaptif di dekat objek/di kanvas hanya ketika
//! pengguna memilih elemen geometri (sketch entity 2D, face 3D, atau body 3D).
//! Menggantikan radial menu acak dengan aksi kontekstual yang presisi.

use egui::{Button, RichText, Ui, Vec2};
use egui_material_icons::icons::{
    ICON_ARCHITECTURE, ICON_CALL_MERGE, ICON_CLOSE, ICON_CONTENT_CUT, ICON_DELETE,
    ICON_DRIVE_FILE_RENAME_OUTLINE, ICON_EDIT, ICON_FLIP, ICON_OPEN_IN_FULL, ICON_REFRESH, ICON_ROUTE,
};
use crate::theme::{pill_frame, ACCENT_BLUE, ACCENT_ORANGE, TEXT_PRIMARY, TEXT_SECONDARY};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextAction {
    Extrude,
    Offset,
    Mirror,
    Trim,
    Revolve,
    Sweep,
    Shell,
    DraftAngle,
    SplitBody,
    SplitFace,
    Boolean,
    Fillet,
    SketchOnFace,
    Delete,
    ClearSelection,
    Rename,
}

#[derive(Default)]
pub struct ContextActionBar;

impl ContextActionBar {
    pub fn new() -> Self {
        Self
    }

    /// Render contextual action bar untuk seleksi 2D (Sketch Entities).
    pub fn show_sketch_selection(
        ui: &mut Ui,
        selected_count: usize,
        _has_closed_profile: bool,
    ) -> Option<ContextAction> {
        let mut action = None;

        pill_frame().show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
            ui.horizontal(|ui| {
                // Header ringkas info seleksi
                ui.label(
                    RichText::new(format!("{} terpilih", selected_count))
                        .size(11.0)
                        .strong()
                        .color(ACCENT_BLUE),
                );

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);

                // 1. Offset
                let btn = ui.add(
                    Button::new(
                        RichText::new(format!("{} Offset", ICON_OPEN_IN_FULL.codepoint))
                            .size(11.5)
                            .color(TEXT_PRIMARY),
                    ),
                );
                if btn.on_hover_text("Offset kurva / kontur terpilih (O)").clicked() {
                    action = Some(ContextAction::Offset);
                }

                // 3. Mirror
                let btn = ui.add(
                    Button::new(
                        RichText::new(format!("{} Mirror", ICON_FLIP.codepoint))
                            .size(11.5)
                            .color(TEXT_PRIMARY),
                    ),
                );
                if btn.on_hover_text("Cerminkan elemen terpilih terhadap garis sumbu (M)").clicked() {
                    action = Some(ContextAction::Mirror);
                }

                // 4. Trim
                let btn = ui.add(
                    Button::new(
                        RichText::new(format!("{} Trim", ICON_CONTENT_CUT.codepoint))
                            .size(11.5)
                            .color(TEXT_PRIMARY),
                    ),
                );
                if btn.on_hover_text("Pangkas segmen garis yang bersilangan (T)").clicked() {
                    action = Some(ContextAction::Trim);
                }

                // 5. Revolve
                let btn = ui.add(
                    Button::new(
                        RichText::new(format!("{} Revolve", ICON_REFRESH.codepoint))
                            .size(11.5)
                            .color(TEXT_PRIMARY),
                    ),
                );
                if btn.on_hover_text("Putar profil 360° mengelilingi sumbu (V)").clicked() {
                    action = Some(ContextAction::Revolve);
                }

                // 6. Sweep
                let btn = ui.add(
                    Button::new(
                        RichText::new(format!("{} Sweep", ICON_ROUTE.codepoint))
                            .size(11.5)
                            .color(TEXT_PRIMARY),
                    ),
                );
                if btn.on_hover_text("Sapu profil 2D menyusuri kurva jalur pemandu (Sweep)").clicked() {
                    action = Some(ContextAction::Sweep);
                }


                ui.add_space(2.0);
                ui.separator();
                ui.add_space(2.0);

                // Rename / Beri Nama Grup
                let rename_btn = ui.add(
                    Button::new(
                        RichText::new(format!("{} Nama", ICON_DRIVE_FILE_RENAME_OUTLINE.codepoint))
                            .size(11.5)
                            .color(ACCENT_BLUE),
                    ),
                );
                if rename_btn.on_hover_text("Beri nama / kelompokkan entitas terpilih").clicked() {
                    action = Some(ContextAction::Rename);
                }

                ui.add_space(2.0);
                ui.separator();
                ui.add_space(2.0);

                // 6. Delete
                let del_btn = ui.add(
                    Button::new(
                        RichText::new(format!("{} Hapus", ICON_DELETE.codepoint))
                            .size(11.5)
                            .color(egui::Color32::from_rgb(255, 110, 110)),
                    ),
                );
                if del_btn.on_hover_text("Hapus elemen terpilih (Delete)").clicked() {
                    action = Some(ContextAction::Delete);
                }

                // 7. Deselect / Close
                let close_btn = ui.add(
                    Button::new(
                        RichText::new(ICON_CLOSE.codepoint)
                            .size(12.0)
                            .color(TEXT_SECONDARY),
                    ),
                );
                if close_btn.on_hover_text("Batalkan Seleksi (Esc)").clicked() {
                    action = Some(ContextAction::ClearSelection);
                }
            });
        });

        action
    }

    /// Render contextual action bar untuk seleksi 3D Face.
    pub fn show_face_selection(ui: &mut Ui) -> Option<ContextAction> {
        let mut action = None;

        pill_frame().show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Face Terpilih")
                        .size(11.0)
                        .strong()
                        .color(ACCENT_ORANGE),
                );

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);

                // Sketch On Face
                let btn = ui.add(
                    Button::new(
                        RichText::new(format!("{} Sketsa di Face", ICON_EDIT.codepoint))
                            .size(11.5)
                            .color(TEXT_PRIMARY),
                    ),
                );
                if btn.on_hover_text("Jadikan bidang ini sebagai bidang sketsa baru").clicked() {
                    action = Some(ContextAction::SketchOnFace);
                }

                // Revolve Face
                let btn = ui.add(
                    Button::new(
                        RichText::new(format!("{} Revolve", ICON_REFRESH.codepoint))
                            .size(11.5)
                            .color(TEXT_PRIMARY),
                    ),
                );
                if btn.on_hover_text("Putar bidang mengelilingi sumbu (V)").clicked() {
                    action = Some(ContextAction::Revolve);
                }

                // Shell / Hollow Face
                let btn = ui.add(
                    Button::new(
                        RichText::new(format!("{} Shell / Hollow", ICON_OPEN_IN_FULL.codepoint))
                            .size(11.5)
                            .color(TEXT_PRIMARY),
                    ),
                );
                if btn.on_hover_text("Ronggakan benda 3D dengan ketebalan dinding (S)").clicked() {
                    action = Some(ContextAction::Shell);
                }

                // Draft Angle
                let btn = ui.add(
                    Button::new(
                        RichText::new(format!("{}  Draft Angle", ICON_ARCHITECTURE.codepoint))
                            .size(11.5)
                            .color(crate::theme::ACCENT_ORANGE),
                    ),
                );
                if btn.on_hover_text("Tambahkan kemiringan cetakan plastik (draft angle) ke face ini (D)").clicked() {
                    action = Some(ContextAction::DraftAngle);
                }

                // Split Face
                let btn = ui.add(
                    Button::new(
                        RichText::new(format!("{}  Split Face", ICON_CONTENT_CUT.codepoint))
                            .size(11.5)
                            .color(TEXT_PRIMARY),
                    ),
                );
                if btn.on_hover_text("Bagi permukaan (face) ini menjadi bagian terpisah (S)").clicked() {
                    action = Some(ContextAction::SplitFace);
                }

                ui.add_space(2.0);
                ui.separator();
                ui.add_space(2.0);

                // Deselect
                let close_btn = ui.add(
                    Button::new(
                        RichText::new(ICON_CLOSE.codepoint)
                            .size(12.0)
                            .color(TEXT_SECONDARY),
                    ),
                );
                if close_btn.on_hover_text("Batalkan Seleksi (Esc)").clicked() {
                    action = Some(ContextAction::ClearSelection);
                }
            });
        });

        action
    }

    /// Render contextual action bar untuk seleksi 3D Body.
    pub fn show_body_selection(ui: &mut Ui, count: usize) -> Option<ContextAction> {
        let mut action = None;

        pill_frame().show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{} Body Terpilih", count))
                        .size(11.0)
                        .strong()
                        .color(ACCENT_BLUE),
                );

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);

                // Split Body jika 1 body dipilih
                if count == 1 {
                    let split_btn = ui.add(
                        Button::new(
                            RichText::new(format!("{} Split Body", ICON_CONTENT_CUT.codepoint))
                                .size(11.5)
                                .color(TEXT_PRIMARY),
                        ),
                    );
                    if split_btn
                        .on_hover_text("Potong solid 3D menjadi 2 body terpisah menggunakan bidang pemotong (S)")
                        .clicked()
                    {
                        action = Some(ContextAction::SplitBody);
                    }

                    ui.add_space(2.0);
                    ui.separator();
                    ui.add_space(2.0);
                }

                // Operasi Boolean jika minimal 2 body dipilih
                if count >= 2 {
                    let bool_btn = ui.add(
                        Button::new(
                            RichText::new(format!("{} Boolean", ICON_CALL_MERGE.codepoint))
                                .size(11.5)
                                .color(TEXT_PRIMARY),
                        ),
                    );
                    if bool_btn
                        .on_hover_text("Operasi Boolean: Union (Gabung), Subtract (Potong), Intersect (Irisan)")
                        .clicked()
                    {
                        action = Some(ContextAction::Boolean);
                    }

                    ui.add_space(2.0);
                    ui.separator();
                    ui.add_space(2.0);
                }

                // Rename Body
                let rename_btn = ui.add(
                    Button::new(
                        RichText::new(format!("{} Nama", ICON_DRIVE_FILE_RENAME_OUTLINE.codepoint))
                            .size(11.5)
                            .color(ACCENT_BLUE),
                    ),
                );
                if rename_btn.on_hover_text("Beri nama / ganti nama body terpilih").clicked() {
                    action = Some(ContextAction::Rename);
                }

                ui.add_space(2.0);
                ui.separator();
                ui.add_space(2.0);

                // Delete Body
                let del_btn = ui.add(
                    Button::new(
                        RichText::new(format!("{} Hapus Body", ICON_DELETE.codepoint))
                            .size(11.5)
                            .color(egui::Color32::from_rgb(255, 110, 110)),
                    ),
                );
                if del_btn.on_hover_text("Hapus body terpilih (Delete)").clicked() {
                    action = Some(ContextAction::Delete);
                }

                // Deselect
                let close_btn = ui.add(
                    Button::new(
                        RichText::new(ICON_CLOSE.codepoint)
                            .size(12.0)
                            .color(TEXT_SECONDARY),
                    ),
                );
                if close_btn.on_hover_text("Batalkan Seleksi (Esc)").clicked() {
                    action = Some(ContextAction::ClearSelection);
                }
            });
        });

        action
    }
}
