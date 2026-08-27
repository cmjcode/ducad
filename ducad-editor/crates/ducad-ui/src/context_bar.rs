//! Floating Contextual Action Bar bergaya Shapr3D dengan Material Icons.
//!
//! Menampilkan bar aksi mengambang adaptif di dekat objek/di kanvas hanya ketika
//! pengguna memilih elemen geometri (sketch entity 2D, face 3D, atau body 3D).
//! Menggantikan radial menu acak dengan aksi kontekstual yang presisi.

use ducad_i18n::t;
use egui::{Button, RichText, Ui, Vec2};
use egui_icons::icons::{
    ICON_ADJUST, ICON_ARCHITECTURE, ICON_ARROWS_OUTWARD, ICON_CALL_MERGE, ICON_CATEGORY,
    ICON_CLOSE, ICON_CONTENT_CUT, ICON_DELETE, ICON_DRIVE_FILE_RENAME_OUTLINE, ICON_EDIT,
    ICON_FLIP, ICON_GRID_VIEW, ICON_OPEN_IN_FULL, ICON_REFRESH, ICON_ROUTE, ICON_STRAIGHTEN,
    ICON_WARNING,
};
use crate::theme::{pill_frame, ACCENT_BLUE, ACCENT_ORANGE, TEXT_PRIMARY, TEXT_SECONDARY};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextAction {
    Extrude,
    Offset,
    Mirror,
    Trim,
    Extend,
    Pattern,
    Revolve,
    Sweep,
    Helix,
    Shell,
    Rib,
    DraftAngle,
    SplitBody,
    SplitFace,
    Boolean,
    Fillet,
    SketchOnFace,
    HoleWizard,
    OffsetPlane,
    Delete,
    ClearSelection,
    Rename,
    MateConcentric,
    MateCoincident,
    MateDistance,
    MateAngle,
    OpenAssemblyTree,
    CheckClash,
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
        icon_size: f32,
    ) -> Option<ContextAction> {
        let mut action = None;
        let icon_sz = icon_size.clamp(12.0, 18.0);

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
                if context_action_btn(ui, ICON_OPEN_IN_FULL.codepoint, "Offset", TEXT_PRIMARY, icon_sz, "Offset kurva / kontur terpilih (O)").clicked() {
                    action = Some(ContextAction::Offset);
                }

                // 3. Mirror
                if context_action_btn(ui, ICON_FLIP.codepoint, "Mirror", TEXT_PRIMARY, icon_sz, "Cerminkan elemen terpilih terhadap garis sumbu (M)").clicked() {
                    action = Some(ContextAction::Mirror);
                }

                // 4. Trim
                if context_action_btn(ui, ICON_CONTENT_CUT.codepoint, "Trim", TEXT_PRIMARY, icon_sz, "Pangkas segmen garis yang bersilangan (T)").clicked() {
                    action = Some(ContextAction::Trim);
                }

                // 4.1 Extend
                if context_action_btn(ui, ICON_ARROWS_OUTWARD.codepoint, "Extend", TEXT_PRIMARY, icon_sz, "Perpanjang garis sampai kurva batas terdekat (Shift+E)").clicked() {
                    action = Some(ContextAction::Extend);
                }

                // 4.5 Pattern
                if context_action_btn(ui, ICON_GRID_VIEW.codepoint, "Pattern", TEXT_PRIMARY, icon_sz, "Duplikasi entitas dalam kisi Linier / Sirkular (P)").clicked() {
                    action = Some(ContextAction::Pattern);
                }

                // 5. Revolve
                if context_action_btn(ui, ICON_REFRESH.codepoint, "Revolve", TEXT_PRIMARY, icon_sz, "Putar profil 360° mengelilingi sumbu (V)").clicked() {
                    action = Some(ContextAction::Revolve);
                }

                // 6. Sweep
                if context_action_btn(ui, ICON_ROUTE.codepoint, "Sweep", TEXT_PRIMARY, icon_sz, "Sapu profil 2D menyusuri kurva jalur pemandu (Sweep)").clicked() {
                    action = Some(ContextAction::Sweep);
                }

                // 7. Helix / Coil
                if context_action_btn(ui, egui_icons::icons::ICON_HEATING_COIL.codepoint, "Helix", TEXT_PRIMARY, icon_sz, "Buat pegas atau ulir spiral 3D (Helix / Coil)").clicked() {
                    action = Some(ContextAction::Helix);
                }

                ui.add_space(2.0);
                ui.separator();
                ui.add_space(2.0);

                // Rename / Beri Nama Grup
                if context_action_btn(ui, ICON_DRIVE_FILE_RENAME_OUTLINE.codepoint, "Nama", ACCENT_BLUE, icon_sz, "Beri nama / kelompokkan entitas terpilih").clicked() {
                    action = Some(ContextAction::Rename);
                }

                ui.add_space(2.0);
                ui.separator();
                ui.add_space(2.0);

                // 6. Delete
                if context_action_btn(ui, ICON_DELETE.codepoint, "Hapus", egui::Color32::from_rgb(255, 110, 110), icon_sz, "Hapus elemen terpilih (Delete)").clicked() {
                    action = Some(ContextAction::Delete);
                }

                // 7. Deselect / Close
                if context_action_btn(ui, ICON_CLOSE.codepoint, "", TEXT_SECONDARY, icon_sz, "Batalkan Seleksi (Esc)").clicked() {
                    action = Some(ContextAction::ClearSelection);
                }
            });
        });

        action
    }

    /// Render contextual action bar untuk seleksi 3D Face.
    pub fn show_face_selection(ui: &mut Ui, is_editing_hole: bool, icon_size: f32) -> Option<ContextAction> {
        let mut action = None;
        let icon_sz = icon_size.clamp(12.0, 18.0);

        pill_frame().show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(if is_editing_hole {
                        t!("selection-hole-selected")
                    } else {
                        t!("selection-face-selected")
                    })
                    .size(11.0)
                    .strong()
                    .color(if is_editing_hole {
                        ACCENT_BLUE
                    } else {
                        ACCENT_ORANGE
                    }),
                );

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);

                // Sketch On Face
                if context_action_btn(ui, ICON_EDIT.codepoint, "Sketsa di Face", TEXT_PRIMARY, icon_sz, "Jadikan bidang ini sebagai bidang sketsa baru").clicked() {
                    action = Some(ContextAction::SketchOnFace);
                }

                // Revolve Face
                if context_action_btn(ui, ICON_REFRESH.codepoint, "Revolve", TEXT_PRIMARY, icon_sz, "Putar bidang mengelilingi sumbu (V)").clicked() {
                    action = Some(ContextAction::Revolve);
                }

                // Helix / Coil Face
                if context_action_btn(ui, egui_icons::icons::ICON_HEATING_COIL.codepoint, "Helix", TEXT_PRIMARY, icon_sz, "Buat ulir spiral atau pegas dari permukaan ini (Helix)").clicked() {
                    action = Some(ContextAction::Helix);
                }

                // Shell / Hollow Face
                if context_action_btn(ui, ICON_OPEN_IN_FULL.codepoint, "Shell / Hollow", TEXT_PRIMARY, icon_sz, "Ronggakan benda 3D dengan ketebalan dinding (H)").clicked() {
                    action = Some(ContextAction::Shell);
                }

                // Rib / Tulang Penguat
                if context_action_btn(ui, egui_icons::icons::ICON_TIMELINE.codepoint, "Rib (Tulang)", TEXT_PRIMARY, icon_sz, "Buat tulang penguat (stiffener rib) pada casing ini (R)").clicked() {
                    action = Some(ContextAction::Rib);
                }

                // Draft Angle
                if context_action_btn(ui, ICON_ARCHITECTURE.codepoint, "Draft Angle", crate::theme::ACCENT_ORANGE, icon_sz, "Tambahkan kemiringan cetakan plastik (draft angle) ke face ini (D)").clicked() {
                    action = Some(ContextAction::DraftAngle);
                }

                // Split Face
                if context_action_btn(ui, ICON_CONTENT_CUT.codepoint, "Split Face", TEXT_PRIMARY, icon_sz, "Bagi permukaan (face) ini menjadi bagian terpisah (S)").clicked() {
                    action = Some(ContextAction::SplitFace);
                }

                // Hole Wizard (Standar ISO Fastener)
                if context_action_btn(ui, ICON_ADJUST.codepoint, &t!("tool-hole-wizard"), ACCENT_BLUE, icon_sz, &t!("tool-hole-wizard-desc")).clicked() {
                    action = Some(ContextAction::HoleWizard);
                }

                // New Datum Plane from Face
                if context_action_btn(ui, egui_icons::icons::ICON_LAYERS_OFF.codepoint, "+ Offset Plane", crate::theme::ACCENT_ORANGE, icon_sz, "Buat bidang kerja referensi 3D (Datum Plane) dari permukaan ini").clicked() {
                    action = Some(ContextAction::OffsetPlane);
                }

                ui.add_space(2.0);
                ui.separator();
                ui.add_space(2.0);

                // Deselect
                if context_action_btn(ui, ICON_CLOSE.codepoint, "", TEXT_SECONDARY, icon_sz, "Batalkan Seleksi (Esc)").clicked() {
                    action = Some(ContextAction::ClearSelection);
                }
            });
        });

        action
    }

    /// Render contextual action bar untuk seleksi multi-face 3D (Mate Constraints Perakitan).
    pub fn show_multi_face_selection(ui: &mut Ui, count: usize, icon_size: f32) -> Option<ContextAction> {
        let mut action = None;
        let icon_sz = icon_size.clamp(12.0, 18.0);

        pill_frame().show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{} Face Terpilih (Mate)", count))
                        .size(11.0)
                        .strong()
                        .color(ACCENT_BLUE),
                );

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);

                // 1. Concentric Mate (◎)
                if context_action_btn(ui, ICON_ADJUST.codepoint, "Concentric", TEXT_PRIMARY, icon_sz, &t!("assembly-mate-concentric-desc")).clicked() {
                    action = Some(ContextAction::MateConcentric);
                }

                // 2. Coincident Mate (⫦)
                if context_action_btn(ui, ICON_CALL_MERGE.codepoint, "Coincident", TEXT_PRIMARY, icon_sz, &t!("assembly-mate-coincident-desc")).clicked() {
                    action = Some(ContextAction::MateCoincident);
                }

                // 3. Distance Mate (↔)
                if context_action_btn(ui, ICON_STRAIGHTEN.codepoint, "Distance", TEXT_PRIMARY, icon_sz, &t!("assembly-mate-distance-desc")).clicked() {
                    action = Some(ContextAction::MateDistance);
                }

                // 4. Angle Mate (∡)
                if context_action_btn(ui, ICON_ARCHITECTURE.codepoint, "Angle", TEXT_PRIMARY, icon_sz, &t!("assembly-mate-angle-desc")).clicked() {
                    action = Some(ContextAction::MateAngle);
                }

                ui.add_space(2.0);
                ui.separator();
                ui.add_space(2.0);

                // Open Assembly Drawer
                if context_action_btn(ui, ICON_CATEGORY.codepoint, "Assembly", ACCENT_BLUE, icon_sz, &t!("topbar-assembly-tooltip")).clicked() {
                    action = Some(ContextAction::OpenAssemblyTree);
                }

                ui.add_space(2.0);
                ui.separator();
                ui.add_space(2.0);

                // Deselect
                if context_action_btn(ui, ICON_CLOSE.codepoint, "", TEXT_SECONDARY, icon_sz, "Batalkan Seleksi (Esc)").clicked() {
                    action = Some(ContextAction::ClearSelection);
                }
            });
        });

        action
    }

    /// Render contextual action bar untuk seleksi 3D Body.
    pub fn show_body_selection(ui: &mut Ui, count: usize, icon_size: f32) -> Option<ContextAction> {
        let mut action = None;
        let icon_sz = icon_size.clamp(12.0, 18.0);

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
                    if context_action_btn(ui, ICON_CONTENT_CUT.codepoint, "Split Body", TEXT_PRIMARY, icon_sz, "Potong solid 3D menjadi 2 body terpisah menggunakan bidang pemotong (S)").clicked() {
                        action = Some(ContextAction::SplitBody);
                    }

                    ui.add_space(2.0);
                    ui.separator();
                    ui.add_space(2.0);
                }

                // Operasi Boolean jika minimal 2 body dipilih
                if count >= 2 {
                    if context_action_btn(ui, ICON_CALL_MERGE.codepoint, "Boolean", TEXT_PRIMARY, icon_sz, "Operasi Boolean: Union (Gabung), Subtract (Potong), Intersect (Irisan)").clicked() {
                        action = Some(ContextAction::Boolean);
                    }

                    // Uji Tabrakan / Clash Detection
                    if context_action_btn(ui, ICON_WARNING.codepoint, &t!("context-check-clash"), ACCENT_ORANGE, icon_sz, &t!("assembly-clash-desc")).clicked() {
                        action = Some(ContextAction::CheckClash);
                    }

                    ui.add_space(2.0);
                    ui.separator();
                    ui.add_space(2.0);
                }

                // Pattern Solid (1 or more bodies)
                if context_action_btn(ui, ICON_GRID_VIEW.codepoint, "Pattern", TEXT_PRIMARY, icon_sz, "Duplikasi solid 3D dalam kisi Linier / Sirkular (P)").clicked() {
                    action = Some(ContextAction::Pattern);
                }

                ui.add_space(2.0);
                ui.separator();
                ui.add_space(2.0);

                // Rename Body
                if context_action_btn(ui, ICON_DRIVE_FILE_RENAME_OUTLINE.codepoint, "Nama", ACCENT_BLUE, icon_sz, "Beri nama / ganti nama body terpilih").clicked() {
                    action = Some(ContextAction::Rename);
                }

                ui.add_space(2.0);
                ui.separator();
                ui.add_space(2.0);

                // Delete Body
                if context_action_btn(ui, ICON_DELETE.codepoint, "Hapus Body", egui::Color32::from_rgb(255, 110, 110), icon_sz, "Hapus body terpilih (Delete)").clicked() {
                    action = Some(ContextAction::Delete);
                }

                // Deselect
                if context_action_btn(ui, ICON_CLOSE.codepoint, "", TEXT_SECONDARY, icon_sz, "Batalkan Seleksi (Esc)").clicked() {
                    action = Some(ContextAction::ClearSelection);
                }
            });
        });

        action
    }
}

/// Helper untuk membuat tombol aksi kontekstual dengan ikon berukuran dinamis dan label teks proporsional.
fn context_action_btn(
    ui: &mut Ui,
    icon: &str,
    label: &str,
    color: egui::Color32,
    icon_size: f32,
    tooltip: &str,
) -> egui::Response {
    let mut layout_job = egui::text::LayoutJob::default();
    layout_job.append(
        icon,
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::proportional(icon_size),
            color,
            valign: egui::Align::Center,
            ..Default::default()
        },
    );
    if !label.is_empty() {
        layout_job.append(
            &format!(" {}", label),
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::proportional(11.5),
                color,
                valign: egui::Align::Center,
                ..Default::default()
            },
        );
    }
    let resp = ui.add(Button::new(layout_job));
    if !tooltip.is_empty() {
        resp.on_hover_text(tooltip)
    } else {
        resp
    }
}
