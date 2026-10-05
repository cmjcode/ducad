//! Assembly Tree & Mate Constraints Drawer — Panel Hierarki Perakitan & Pengelolaan Mate.
//!
//! Menampilkan panel dock di pojok kanan bawah kanvas untuk:
//! - Visualisasi pohon hierarki part instance dan sub-assembly.
//! - Pengaturan status Grounded (terkunci) dan pelacakan Derajat Kebebasan (DOF).
//! - Daftar kendala mate 3D (Concentric, Coincident, Distance, Angle) dengan status dan parameter.

use crate::theme::{
    card_frame, glass_frame, ACCENT_BLUE, ACCENT_ORANGE, BOTTOM_RIGHT_PANEL_WIDTH, TEXT_MUTED,
    TEXT_PRIMARY, TEXT_SECONDARY,
};
use ducad_core::assembly::{
    AssemblyInstanceId, AssemblyTree, ClashReport, MateConstraint, MateConstraintId, MateKind,
    MateStatus, SubAssemblyId,
};
use ducad_i18n::t;
use egui::{
    Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, ScrollArea, Stroke, Ui,
};
use egui_icons::icons::{
    ICON_ADD, ICON_ADJUST, ICON_ARCHITECTURE, ICON_CALL_MERGE, ICON_CATEGORY, ICON_CHECK_CIRCLE,
    ICON_CLEAR, ICON_CLOSE, ICON_DELETE, ICON_EDIT, ICON_ERROR, ICON_FLIP, ICON_FOLDER,
    ICON_HORIZONTAL_RULE, ICON_KEYBOARD_ARROW_DOWN, ICON_KEYBOARD_ARROW_RIGHT, ICON_LOCK,
    ICON_LOCK_OPEN, ICON_PLAY_ARROW, ICON_SEARCH, ICON_STRAIGHTEN, ICON_VISIBILITY, ICON_WARNING,
};

#[derive(Debug, Clone)]
pub enum AssemblyDrawerEvent {
    /// Pilih instance part tertentu di viewport.
    SelectInstance(AssemblyInstanceId),
    /// Ganti status Grounded (Kunci/Buka) untuk suatu instance.
    ToggleGrounded(AssemblyInstanceId),
    /// Toggle visibilitas instance.
    ToggleInstanceVisibility(AssemblyInstanceId),
    /// Hapus instance part dari perakitan.
    DeleteInstance(AssemblyInstanceId),
    /// Buat sub-assembly baru.
    AddSubAssembly,
    /// Hapus sub-assembly.
    DeleteSubAssembly(SubAssemblyId),
    /// Pilih Mate Constraint tertentu.
    SelectMate(MateConstraintId),
    /// Aktifkan / Nonaktifkan (Suppress) Mate.
    ToggleSuppressMate(MateConstraintId),
    /// Hapus Mate Constraint.
    DeleteMate(MateConstraintId),
    /// Update parameter mate (jarak offset, sudut, atau flip alignment).
    UpdateMateParam {
        id: MateConstraintId,
        val: f64,
        flip: bool,
    },
    /// Picu solver perakitan untuk menghitung ulang posisi seluruh part.
    SolveAssembly,
    /// Jalankan deteksi tabrakan fisik otomatis (Clash & Interference Detection).
    RunClashDetection {
        tolerance: f64,
    },
    /// Pilih / sorot hasil tabrakan tertentu di 3D viewport.
    SelectClash(Option<u32>),
    /// Ubah volume tabrakan menjadi bodi solid independen baru.
    ConvertClashToBody(u32),
    /// Bersihkan hasil deteksi tabrakan.
    ClearClashes,
    /// Sisipkan part dari berkas `.ducad` lain (membuka pemilih berkas).
    AddExternalPart,
    /// Periksa apakah ada sumber eksternal yang berubah, tanpa memuat.
    RefreshExternalStatus,
    /// Muat ulang geometri satu part eksternal dari sumbernya.
    ReloadExternalPart(AssemblyInstanceId),
    /// Putus tautan ke berkas sumber; geometri yang ada dipertahankan.
    MakeIndependent(AssemblyInstanceId),
    /// Ubah faktor urai tampilan (0 = terakit, 1 = terurai penuh).
    SetExplodeFactor(f64),
    /// Isi arah urai tiap part secara radial dari pusat perakitan.
    AutoExplode { distance: f64 },
    /// Buat studi gerak pada mate bernilai numerik.
    AddMotionStudy {
        mate: MateConstraintId,
        from: f64,
        to: f64,
    },
    /// Geser playhead studi gerak ke `t` dalam [0, 1].
    ScrubMotionStudy { index: usize, t: f64 },
    DeleteMotionStudy(usize),
    /// Tutup panel Assembly Tree.
    Close,
}

pub struct AssemblyDrawer {
    pub search_query: String,
    pub custom_height: Option<f32>,
    pub components_expanded: bool,
    pub mates_expanded: bool,
    pub clash_expanded: bool,
    pub clash_tolerance_mm3: f64,
    pub editing_mate_id: Option<MateConstraintId>,
    pub edit_input_val: String,
    pub edit_flip_alignment: bool,
    // ---- P3.1/P3.2: part eksternal, exploded view, studi gerak ----
    pub tools_expanded: bool,
    pub explode_distance_input: String,
    pub motion_mate_selected: Option<MateConstraintId>,
    pub motion_from_input: String,
    pub motion_to_input: String,
    pub motion_selected: usize,
    pub motion_t: f32,
    pub motion_playing: bool,
    /// Instance eksternal yang sumbernya berubah/tak terbaca menurut
    /// pemeriksaan terakhir. Diisi aplikasi; drawer hanya menampilkannya.
    pub stale_external: Vec<AssemblyInstanceId>,
}

impl Default for AssemblyDrawer {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            custom_height: None,
            components_expanded: true,
            mates_expanded: true,
            clash_expanded: true,
            clash_tolerance_mm3: 0.001,
            editing_mate_id: None,
            edit_input_val: String::new(),
            edit_flip_alignment: false,
            tools_expanded: true,
            explode_distance_input: "50".to_string(),
            motion_mate_selected: None,
            motion_from_input: "0".to_string(),
            motion_to_input: "90".to_string(),
            motion_selected: 0,
            motion_t: 0.0,
            motion_playing: false,
            stale_external: Vec::new(),
        }
    }
}

impl AssemblyDrawer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Mulai edit parameter mate.
    pub fn start_editing_mate(&mut self, mate: &MateConstraint) {
        self.editing_mate_id = Some(mate.id);
        match &mate.kind {
            MateKind::Distance {
                offset,
                opposite_normal,
            } => {
                self.edit_input_val = format!("{:.2}", offset);
                self.edit_flip_alignment = *opposite_normal;
            }
            MateKind::Angle {
                angle_deg,
                opposite_normal,
            } => {
                self.edit_input_val = format!("{:.1}", angle_deg);
                self.edit_flip_alignment = *opposite_normal;
            }
            MateKind::Concentric { aligned, .. } => {
                self.edit_input_val.clear();
                self.edit_flip_alignment = *aligned;
            }
            MateKind::Coincident { opposite_normal } => {
                self.edit_input_val.clear();
                self.edit_flip_alignment = *opposite_normal;
            }
        }
    }

    /// Render panel Assembly Drawer. Mengembalikan daftar event yang dipicu interaksi pengguna.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        tree: &AssemblyTree,
        selected_instance: Option<AssemblyInstanceId>,
        selected_mate: Option<MateConstraintId>,
        clash_report: Option<&ClashReport>,
        selected_clash_id: Option<u32>,
    ) -> Vec<AssemblyDrawerEvent> {
        let mut events = Vec::new();

        glass_frame().show(ui, |ui| {
            let width = BOTTOM_RIGHT_PANEL_WIDTH;
            let height = self.custom_height.unwrap_or(520.0);
            ui.set_min_width(width);
            ui.set_max_width(width);
            ui.set_width(width);
            ui.set_height(height);

            ui.vertical(|ui| {
                // 1. Header Panel
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(ICON_CATEGORY.codepoint)
                            .size(16.0)
                            .color(ACCENT_BLUE),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(t!("assembly-tree-title"))
                            .size(13.5)
                            .strong()
                            .color(TEXT_PRIMARY),
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .button(
                                RichText::new(ICON_CLOSE.codepoint)
                                    .size(14.0)
                                    .color(TEXT_SECONDARY),
                            )
                            .on_hover_text("Tutup panel")
                            .clicked()
                        {
                            events.push(AssemblyDrawerEvent::Close);
                        }

                        if ui
                            .button(
                                RichText::new(format!("{} Solve", ICON_PLAY_ARROW.codepoint))
                                    .size(11.0)
                                    .color(ACCENT_BLUE),
                            )
                            .on_hover_text(t!("assembly-solve"))
                            .clicked()
                        {
                            events.push(AssemblyDrawerEvent::SolveAssembly);
                        }
                    });
                });

                ui.add_space(6.0);

                // 2. Search Box
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(ICON_SEARCH.codepoint)
                            .size(13.0)
                            .color(TEXT_MUTED),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut self.search_query)
                            .hint_text("Cari komponen atau mate…")
                            .desired_width(width - 60.0),
                    );
                    if !self.search_query.is_empty()
                        && ui
                            .button(
                                RichText::new(ICON_CLEAR.codepoint)
                                    .size(12.0)
                                    .color(TEXT_MUTED),
                            )
                            .clicked()
                    {
                        self.search_query.clear();
                    }
                });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // 3. Scroll Area: Komponen & Mates
                ScrollArea::vertical().show(ui, |ui| {
                    // SEKSI 1: Part Instances & Sub-Assemblies
                    ui.horizontal(|ui| {
                        let icon = if self.components_expanded {
                            ICON_KEYBOARD_ARROW_DOWN.codepoint
                        } else {
                            ICON_KEYBOARD_ARROW_RIGHT.codepoint
                        };
                        if ui
                            .button(RichText::new(icon).size(13.0).color(TEXT_SECONDARY))
                            .clicked()
                        {
                            self.components_expanded = !self.components_expanded;
                        }
                        ui.label(
                            RichText::new(format!(
                                "COMPONENTS ({})",
                                tree.instances.len()
                            ))
                            .size(11.0)
                            .strong()
                            .color(TEXT_SECONDARY),
                        );

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .button(
                                    RichText::new(format!(
                                        "{} Sub",
                                        ICON_FOLDER.codepoint
                                    ))
                                    .size(10.0)
                                    .color(TEXT_MUTED),
                                )
                                .on_hover_text(t!("assembly-new-sub"))
                                .clicked()
                            {
                                events.push(AssemblyDrawerEvent::AddSubAssembly);
                            }
                        });
                    });

                    if self.components_expanded {
                        if tree.instances.is_empty() {
                            card_frame().show(ui, |ui| {
                                ui.vertical_centered(|ui| {
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new(t!("assembly-tree-empty"))
                                            .size(11.0)
                                            .color(TEXT_MUTED),
                                    );
                                    ui.label(
                                        RichText::new(t!("assembly-tree-empty-sub"))
                                            .size(10.0)
                                            .color(TEXT_MUTED),
                                    );
                                    ui.add_space(4.0);
                                });
                            });
                        } else {
                            for (&id, inst) in &tree.instances {
                                if !self.search_query.is_empty()
                                    && !inst
                                        .name
                                        .to_lowercase()
                                        .contains(&self.search_query.to_lowercase())
                                {
                                    continue;
                                }

                                let is_selected = selected_instance == Some(id);
                                let dof = tree.compute_instance_dof(id);

                                ui.push_id(format!("inst_{}", id), |ui| {
                                    let bg_color = if is_selected {
                                        Color32::from_rgb(18, 38, 68)
                                    } else {
                                        Color32::from_rgb(26, 29, 36)
                                    };
                                    let stroke = if is_selected {
                                        Stroke::new(1.0, ACCENT_BLUE)
                                    } else {
                                        Stroke::new(0.5, Color32::from_rgb(45, 48, 56))
                                    };

                                    Frame::new()
                                        .fill(bg_color)
                                        .stroke(stroke)
                                        .corner_radius(CornerRadius::same(6))
                                        .inner_margin(Margin::symmetric(8, 6))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                // Icon status Grounded
                                                let ground_icon = if inst.is_grounded {
                                                    ICON_LOCK.codepoint
                                                } else {
                                                    ICON_LOCK_OPEN.codepoint
                                                };
                                                let ground_color = if inst.is_grounded {
                                                    ACCENT_ORANGE
                                                } else {
                                                    TEXT_MUTED
                                                };
                                                if ui
                                                    .button(
                                                        RichText::new(ground_icon)
                                                            .size(12.0)
                                                            .color(ground_color),
                                                    )
                                                    .on_hover_text(if inst.is_grounded {
                                                        t!("assembly-unground")
                                                    } else {
                                                        t!("assembly-ground")
                                                    })
                                                    .clicked()
                                                {
                                                    events.push(
                                                        AssemblyDrawerEvent::ToggleGrounded(id),
                                                    );
                                                }

                                                // Nama Instance
                                                let label_btn = ui.add(
                                                    egui::Button::new(
                                                        RichText::new(&inst.name)
                                                            .size(11.5)
                                                            .color(TEXT_PRIMARY),
                                                    )
                                                    .frame(false),
                                                );
                                                if label_btn.clicked() {
                                                    events.push(
                                                        AssemblyDrawerEvent::SelectInstance(id),
                                                    );
                                                }

                                                ui.with_layout(
                                                    Layout::right_to_left(Align::Center),
                                                    |ui| {
                                                        // Tombol Delete
                                                        if ui
                                                            .button(
                                                                RichText::new(
                                                                    ICON_DELETE.codepoint,
                                                                )
                                                                .size(11.0)
                                                                .color(TEXT_MUTED),
                                                            )
                                                            .on_hover_text("Hapus part instance")
                                                            .clicked()
                                                        {
                                                            events.push(
                                                                AssemblyDrawerEvent::DeleteInstance(
                                                                    id,
                                                                ),
                                                            );
                                                        }

                                                        // Badge DOF / Grounded
                                                        if inst.is_grounded {
                                                            ui.label(
                                                                RichText::new(t!(
                                                                    "assembly-grounded-badge"
                                                                ))
                                                                .size(9.5)
                                                                .color(ACCENT_ORANGE),
                                                            );
                                                        } else {
                                                            let dof_str = format!(
                                                                "{} DOF",
                                                                dof.total_dof()
                                                            );
                                                            let dof_color =
                                                                if dof.is_fully_constrained() {
                                                                    Color32::from_rgb(46, 204, 113)
                                                                } else {
                                                                    ACCENT_BLUE
                                                                };
                                                            ui.label(
                                                                RichText::new(dof_str)
                                                                    .size(9.5)
                                                                    .color(dof_color),
                                                            );
                                                        }
                                                    },
                                                );
                                            });
                                        });
                                });
                                ui.add_space(2.0);
                            }
                        }
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // SEKSI 2: Mate Constraints
                    ui.horizontal(|ui| {
                        let icon = if self.mates_expanded {
                            ICON_KEYBOARD_ARROW_DOWN.codepoint
                        } else {
                            ICON_KEYBOARD_ARROW_RIGHT.codepoint
                        };
                        if ui
                            .button(RichText::new(icon).size(13.0).color(TEXT_SECONDARY))
                            .clicked()
                        {
                            self.mates_expanded = !self.mates_expanded;
                        }
                        ui.label(
                            RichText::new(format!(
                                "MATE CONSTRAINTS ({})",
                                tree.mates.len()
                            ))
                            .size(11.0)
                            .strong()
                            .color(TEXT_SECONDARY),
                        );
                    });

                    if self.mates_expanded {
                        if tree.mates.is_empty() {
                            card_frame().show(ui, |ui| {
                                ui.vertical_centered(|ui| {
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new(t!("assembly-no-mates"))
                                            .size(11.0)
                                            .color(TEXT_MUTED),
                                    );
                                    ui.add_space(4.0);
                                });
                            });
                        } else {
                            for (&id, mate) in &tree.mates {
                                if !self.search_query.is_empty()
                                    && !mate
                                        .name
                                        .to_lowercase()
                                        .contains(&self.search_query.to_lowercase())
                                {
                                    continue;
                                }

                                let is_selected = selected_mate == Some(id);
                                let is_editing = self.editing_mate_id == Some(id);

                                ui.push_id(format!("mate_{}", id), |ui| {
                                    let bg_color = if is_selected {
                                        Color32::from_rgb(18, 38, 68)
                                    } else {
                                        Color32::from_rgb(26, 29, 36)
                                    };
                                    let stroke = if is_selected {
                                        Stroke::new(1.0, ACCENT_BLUE)
                                    } else {
                                        Stroke::new(0.5, Color32::from_rgb(45, 48, 56))
                                    };

                                    Frame::new()
                                        .fill(bg_color)
                                        .stroke(stroke)
                                        .corner_radius(CornerRadius::same(6))
                                        .inner_margin(Margin::symmetric(8, 6))
                                        .show(ui, |ui| {
                                            ui.vertical(|ui| {
                                                ui.horizontal(|ui| {
                                                    // Ikon jenis mate
                                                    let mate_icon = match &mate.kind {
                                                        MateKind::Concentric { .. } => {
                                                            ICON_ADJUST.codepoint
                                                        }
                                                        MateKind::Coincident { .. } => {
                                                            ICON_CALL_MERGE.codepoint
                                                        }
                                                        MateKind::Distance { .. } => {
                                                            ICON_STRAIGHTEN.codepoint
                                                        }
                                                        MateKind::Angle { .. } => {
                                                            ICON_ARCHITECTURE.codepoint
                                                        }
                                                    };
                                                    ui.label(
                                                        RichText::new(mate_icon)
                                                            .size(13.0)
                                                            .color(ACCENT_BLUE),
                                                    );

                                                    // Nama mate
                                                    let label_btn = ui.add(
                                                        egui::Button::new(
                                                            RichText::new(&mate.name)
                                                                .size(11.5)
                                                                .color(TEXT_PRIMARY),
                                                        )
                                                        .frame(false),
                                                    );
                                                    if label_btn.clicked() {
                                                        events.push(AssemblyDrawerEvent::SelectMate(
                                                            id,
                                                        ));
                                                    }

                                                    ui.with_layout(
                                                        Layout::right_to_left(Align::Center),
                                                        |ui| {
                                                            // Tombol Hapus
                                                            if ui
                                                                .button(
                                                                    RichText::new(
                                                                        ICON_DELETE.codepoint,
                                                                    )
                                                                    .size(11.0)
                                                                    .color(TEXT_MUTED),
                                                                )
                                                                .on_hover_text("Hapus mate")
                                                                .clicked()
                                                            {
                                                                events.push(
                                                                    AssemblyDrawerEvent::DeleteMate(
                                                                        id,
                                                                    ),
                                                                );
                                                            }

                                                            // Tombol Edit
                                                            if ui
                                                                .button(
                                                                    RichText::new(
                                                                        ICON_EDIT.codepoint,
                                                                    )
                                                                    .size(11.0)
                                                                    .color(TEXT_SECONDARY),
                                                                )
                                                                .on_hover_text("Edit parameter")
                                                                .clicked()
                                                            {
                                                                if is_editing {
                                                                    self.editing_mate_id = None;
                                                                } else {
                                                                    self.start_editing_mate(mate);
                                                                }
                                                            }

                                                            // Status icon
                                                            match &mate.status {
                                                                MateStatus::Satisfied => {
                                                                    ui.label(
                                                                        RichText::new(
                                                                            ICON_CHECK_CIRCLE
                                                                                .codepoint,
                                                                        )
                                                                        .size(11.0)
                                                                        .color(Color32::from_rgb(
                                                                            46, 204, 113,
                                                                        )),
                                                                    );
                                                                }
                                                                MateStatus::Conflicted(err) => {
                                                                    ui.label(
                                                                        RichText::new(
                                                                            ICON_ERROR.codepoint,
                                                                        )
                                                                        .size(11.0)
                                                                        .color(ACCENT_ORANGE),
                                                                    )
                                                                    .on_hover_text(err);
                                                                }
                                                                MateStatus::Suppressed => {
                                                                    ui.label(
                                                                        RichText::new(
                                                                            ICON_HORIZONTAL_RULE
                                                                                .codepoint,
                                                                        )
                                                                        .size(11.0)
                                                                        .color(TEXT_MUTED),
                                                                    );
                                                                }
                                                                _ => {}
                                                            }
                                                        },
                                                    );
                                                });

                                                // Info detail target mate
                                                let name_a = tree
                                                    .instances
                                                    .get(&mate.target_a.instance_id)
                                                    .map_or("Unknown", |i| i.name.as_str());
                                                let name_b = tree
                                                    .instances
                                                    .get(&mate.target_b.instance_id)
                                                    .map_or("Unknown", |i| i.name.as_str());
                                                ui.label(
                                                    RichText::new(format!(
                                                        "{} ⟷ {}",
                                                        name_a, name_b
                                                    ))
                                                    .size(10.0)
                                                    .color(TEXT_MUTED),
                                                );

                                                // Inline parameter editing
                                                if is_editing {
                                                    ui.add_space(4.0);
                                                    ui.horizontal(|ui| {
                                                        match &mate.kind {
                                                            MateKind::Distance { .. } => {
                                                                ui.label(
                                                                    RichText::new("Offset (mm):")
                                                                        .size(10.5)
                                                                        .color(TEXT_SECONDARY),
                                                                );
                                                                ui.add(
                                                                    egui::TextEdit::singleline(
                                                                        &mut self.edit_input_val,
                                                                    )
                                                                    .desired_width(55.0),
                                                                );
                                                            }
                                                            MateKind::Angle { .. } => {
                                                                ui.label(
                                                                    RichText::new("Sudut (°):")
                                                                        .size(10.5)
                                                                        .color(TEXT_SECONDARY),
                                                                );
                                                                ui.add(
                                                                    egui::TextEdit::singleline(
                                                                        &mut self.edit_input_val,
                                                                    )
                                                                    .desired_width(55.0),
                                                                );
                                                            }
                                                            _ => {}
                                                        }

                                                        if ui
                                                            .button(
                                                                RichText::new(format!(
                                                                    "{} Flip",
                                                                    ICON_FLIP.codepoint
                                                                ))
                                                                .size(10.0)
                                                                .color(TEXT_SECONDARY),
                                                            )
                                                            .clicked()
                                                        {
                                                            self.edit_flip_alignment =
                                                                !self.edit_flip_alignment;
                                                        }

                                                        if ui
                                                            .button(
                                                                RichText::new("Terapkan")
                                                                    .size(10.0)
                                                                    .color(ACCENT_BLUE),
                                                            )
                                                            .clicked()
                                                        {
                                                            let val = self
                                                                .edit_input_val
                                                                .parse::<f64>()
                                                                .unwrap_or(0.0);
                                                            events.push(
                                                                AssemblyDrawerEvent::UpdateMateParam {
                                                                    id,
                                                                    val,
                                                                    flip: self.edit_flip_alignment,
                                                                },
                                                            );
                                                            self.editing_mate_id = None;
                                                        }
                                                    });
                                                }
                                            });
                                        });
                                });
                                ui.add_space(2.0);
                            }
                        }
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // SEKSI 3: Clash & Interference Detection (Fase 12.3)
                    ui.horizontal(|ui| {
                        let icon = if self.clash_expanded {
                            ICON_KEYBOARD_ARROW_DOWN.codepoint
                        } else {
                            ICON_KEYBOARD_ARROW_RIGHT.codepoint
                        };
                        if ui
                            .button(RichText::new(icon).size(13.0).color(TEXT_SECONDARY))
                            .clicked()
                        {
                            self.clash_expanded = !self.clash_expanded;
                        }
                        ui.label(
                            RichText::new(format!(
                                "INTERFERENCE ({})",
                                clash_report.map_or(0, |r| r.clashes.len())
                            ))
                            .size(11.0)
                            .strong()
                            .color(if clash_report.is_some_and(|r| r.has_clashes()) {
                                ACCENT_ORANGE
                            } else {
                                TEXT_SECONDARY
                            }),
                        );

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .button(
                                    RichText::new(format!(
                                        "{} {}",
                                        ICON_PLAY_ARROW.codepoint,
                                        t!("assembly-clash-run")
                                    ))
                                    .size(10.0)
                                    .color(ACCENT_BLUE),
                                )
                                .on_hover_text(t!("assembly-clash-desc"))
                                .clicked()
                            {
                                events.push(AssemblyDrawerEvent::RunClashDetection {
                                    tolerance: self.clash_tolerance_mm3,
                                });
                            }
                        });
                    });

                    if self.clash_expanded {
                        ui.add_space(3.0);

                        // Input toleransi volume
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(t!("assembly-clash-tolerance"))
                                    .size(10.0)
                                    .color(TEXT_MUTED),
                            );
                            ui.add(
                                egui::DragValue::new(&mut self.clash_tolerance_mm3)
                                    .speed(0.001)
                                    .range(0.0001..=10.0)
                                    .max_decimals(4)
                                    .suffix(format!(" {}", t!("assembly-clash-tolerance-unit"))),
                            );
                        });

                        ui.add_space(3.0);

                        if let Some(report) = clash_report {
                            if report.has_clashes() {
                                // Status banner: Clashes detected
                                card_frame().show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(ICON_WARNING.codepoint)
                                                .size(14.0)
                                                .color(ACCENT_ORANGE),
                                        );
                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(t!(
                                                    "assembly-clash-detected",
                                                    count = report.clashes.len()
                                                ))
                                                .size(11.0)
                                                .strong()
                                                .color(ACCENT_ORANGE),
                                            );
                                            ui.label(
                                                RichText::new(t!(
                                                    "assembly-clash-total-volume",
                                                    vol = format!("{:.2}", report.total_volume)
                                                ))
                                                .size(9.5)
                                                .color(TEXT_MUTED),
                                            );
                                        });

                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            if ui
                                                .button(
                                                    RichText::new(ICON_CLEAR.codepoint)
                                                        .size(11.0)
                                                        .color(TEXT_MUTED),
                                                )
                                                .on_hover_text(t!("assembly-clash-clear"))
                                                .clicked()
                                            {
                                                events.push(AssemblyDrawerEvent::ClearClashes);
                                            }
                                        });
                                    });
                                });

                                ui.add_space(4.0);

                                // List of clashes
                                for clash in &report.clashes {
                                    let is_selected = selected_clash_id == Some(clash.id);
                                    let frame = if is_selected {
                                        card_frame().stroke(Stroke::new(1.0, ACCENT_BLUE))
                                    } else {
                                        card_frame()
                                    };

                                    frame.show(ui, |ui| {
                                        ui.vertical(|ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    RichText::new(ICON_ERROR.codepoint)
                                                        .size(12.0)
                                                        .color(ACCENT_ORANGE),
                                                );
                                                let pair_label = ui.add(
                                                    egui::Button::new(
                                                        RichText::new(t!(
                                                            "assembly-clash-pair",
                                                            part_a = &clash.body_a_name,
                                                            part_b = &clash.body_b_name
                                                        ))
                                                        .size(11.0)
                                                        .strong()
                                                        .color(TEXT_PRIMARY),
                                                    )
                                                    .frame(false),
                                                );
                                                if pair_label.clicked() {
                                                    events.push(AssemblyDrawerEvent::SelectClash(
                                                        if is_selected { None } else { Some(clash.id) },
                                                    ));
                                                }
                                            });

                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    RichText::new(t!(
                                                        "assembly-clash-vol-label",
                                                        vol = format!("{:.3}", clash.volume)
                                                    ))
                                                    .size(9.5)
                                                    .color(TEXT_MUTED),
                                                );
                                                ui.label(
                                                    RichText::new(t!(
                                                        "assembly-clash-centroid-label",
                                                        x = format!("{:.1}", clash.center.0),
                                                        y = format!("{:.1}", clash.center.1),
                                                        z = format!("{:.1}", clash.center.2)
                                                    ))
                                                    .size(9.0)
                                                    .color(TEXT_MUTED),
                                                );
                                            });

                                            ui.horizontal(|ui| {
                                                if ui
                                                    .button(
                                                        RichText::new(format!(
                                                            "{} {}",
                                                            ICON_VISIBILITY.codepoint,
                                                            t!("assembly-clash-focus")
                                                        ))
                                                        .size(9.5)
                                                        .color(if is_selected { ACCENT_BLUE } else { TEXT_SECONDARY }),
                                                    )
                                                    .clicked()
                                                {
                                                    events.push(AssemblyDrawerEvent::SelectClash(Some(clash.id)));
                                                }

                                                if ui
                                                    .button(
                                                        RichText::new(format!(
                                                            "{} {}",
                                                            ICON_ADD.codepoint,
                                                            t!("assembly-clash-create-body")
                                                        ))
                                                        .size(9.5)
                                                        .color(TEXT_MUTED),
                                                    )
                                                    .on_hover_text("Ekstrak volume tabrakan jadi bodi solid baru")
                                                    .clicked()
                                                {
                                                    events.push(AssemblyDrawerEvent::ConvertClashToBody(clash.id));
                                                }
                                            });
                                        });
                                    });
                                    ui.add_space(2.0);
                                }
                            } else {
                                // Status banner: Clean (No clashes)
                                card_frame().show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(ICON_CHECK_CIRCLE.codepoint)
                                                .size(14.0)
                                                .color(Color32::from_rgb(46, 204, 113)),
                                        );
                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(t!("assembly-clash-clean"))
                                                    .size(11.0)
                                                    .strong()
                                                    .color(Color32::from_rgb(46, 204, 113)),
                                            );
                                            ui.label(
                                                RichText::new(t!("assembly-clash-clean-desc"))
                                                    .size(9.5)
                                                    .color(TEXT_MUTED),
                                            );
                                        });
                                    });
                                });
                            }
                        } else {
                            // Belum dijalankan
                            card_frame().show(ui, |ui| {
                                ui.vertical_centered(|ui| {
                                    ui.add_space(2.0);
                                    ui.label(
                                        RichText::new(t!("assembly-clash-desc"))
                                            .size(9.5)
                                            .color(TEXT_MUTED),
                                    );
                                    ui.add_space(2.0);
                                    if ui
                                        .button(
                                            RichText::new(format!(
                                                "{} {}",
                                                ICON_PLAY_ARROW.codepoint,
                                                t!("assembly-clash-run")
                                            ))
                                            .size(11.0)
                                            .color(ACCENT_BLUE),
                                        )
                                        .clicked()
                                    {
                                        events.push(AssemblyDrawerEvent::RunClashDetection {
                                            tolerance: self.clash_tolerance_mm3,
                                        });
                                    }
                                    ui.add_space(2.0);
                                });
                            });
                        }
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // SEKSI 4: Part eksternal, exploded view, studi gerak (P3.1/P3.2)
                    ui.horizontal(|ui| {
                        let icon = if self.tools_expanded {
                            ICON_KEYBOARD_ARROW_DOWN.codepoint
                        } else {
                            ICON_KEYBOARD_ARROW_RIGHT.codepoint
                        };
                        if ui
                            .button(RichText::new(icon).size(13.0).color(TEXT_SECONDARY))
                            .clicked()
                        {
                            self.tools_expanded = !self.tools_expanded;
                        }
                        ui.label(
                            RichText::new("PERAKITAN LANJUTAN")
                                .size(11.0)
                                .strong()
                                .color(TEXT_SECONDARY),
                        );
                    });

                    if self.tools_expanded {
                        // 4a. Part eksternal
                        card_frame().show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Part eksternal").size(10.5).strong().color(TEXT_PRIMARY));
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui
                                        .button(RichText::new(format!("{} Sisipkan", ICON_ADD.codepoint)).size(10.0).color(ACCENT_BLUE))
                                        .on_hover_text("Sisipkan part dari berkas .ducad lain; tautannya dipertahankan")
                                        .clicked()
                                    {
                                        events.push(AssemblyDrawerEvent::AddExternalPart);
                                    }
                                    if ui
                                        .button(RichText::new("Periksa perubahan").size(10.0).color(TEXT_SECONDARY))
                                        .on_hover_text("Bandingkan berkas sumber dengan saat terakhir dimuat, tanpa memuat ulang")
                                        .clicked()
                                    {
                                        events.push(AssemblyDrawerEvent::RefreshExternalStatus);
                                    }
                                });
                            });
                            let externals: Vec<_> = tree.external_instances().collect();
                            if externals.is_empty() {
                                ui.label(RichText::new("Belum ada part dari berkas lain.").size(9.5).color(TEXT_MUTED));
                            }
                            for (id, r) in externals {
                                let stale = self.stale_external.contains(&id);
                                let name = tree.instances.get(&id).map(|i| i.name.as_str()).unwrap_or("?");
                                ui.horizontal(|ui| {
                                    if stale {
                                        ui.label(RichText::new(ICON_WARNING.codepoint).size(12.0).color(ACCENT_ORANGE))
                                            .on_hover_text("Berkas sumber berubah sejak terakhir dimuat");
                                    }
                                    ui.label(RichText::new(name).size(10.0).color(TEXT_PRIMARY));
                                    ui.label(RichText::new(&r.relative_path).size(9.0).color(TEXT_MUTED));
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if ui.button(RichText::new("Lepas").size(9.5).color(TEXT_SECONDARY))
                                            .on_hover_text("Putus tautan ke berkas sumber; geometri saat ini dipertahankan")
                                            .clicked()
                                        {
                                            events.push(AssemblyDrawerEvent::MakeIndependent(id));
                                        }
                                        let reload = RichText::new("Muat ulang").size(9.5)
                                            .color(if stale { ACCENT_ORANGE } else { ACCENT_BLUE });
                                        if ui.button(reload).clicked() {
                                            events.push(AssemblyDrawerEvent::ReloadExternalPart(id));
                                        }
                                    });
                                });
                            }
                        });

                        ui.add_space(4.0);

                        // 4b. Exploded view
                        card_frame().show(ui, |ui| {
                            ui.label(RichText::new("Exploded view").size(10.5).strong().color(TEXT_PRIMARY));
                            let mut f = tree.explode_factor as f32;
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Urai").size(9.5).color(TEXT_SECONDARY));
                                if ui.add(egui::Slider::new(&mut f, 0.0..=1.0).show_value(false)).changed() {
                                    events.push(AssemblyDrawerEvent::SetExplodeFactor(f as f64));
                                }
                                ui.label(RichText::new(format!("{:.0}%", f * 100.0)).size(9.5).color(TEXT_MUTED));
                            });
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Jarak (mm)").size(9.5).color(TEXT_SECONDARY));
                                ui.add(egui::TextEdit::singleline(&mut self.explode_distance_input).desired_width(48.0));
                                if ui
                                    .button(RichText::new("Urai otomatis").size(10.0).color(ACCENT_BLUE))
                                    .on_hover_text("Dorong tiap part menjauh dari pusat perakitan; part yang di-ground tetap di tempat")
                                    .clicked()
                                {
                                    let distance = self.explode_distance_input.trim().parse::<f64>().unwrap_or(50.0);
                                    events.push(AssemblyDrawerEvent::AutoExplode { distance });
                                }
                            });
                        });

                        ui.add_space(4.0);

                        // 4c. Studi gerak
                        card_frame().show(ui, |ui| {
                            ui.label(RichText::new("Studi gerak").size(10.5).strong().color(TEXT_PRIMARY));

                            let numeric_mates: Vec<&MateConstraint> = tree
                                .mates
                                .values()
                                .filter(|m| m.kind.driven_value().is_some())
                                .collect();
                            if numeric_mates.is_empty() {
                                ui.label(
                                    RichText::new("Butuh mate Distance atau Angle untuk digerakkan.")
                                        .size(9.5)
                                        .color(TEXT_MUTED),
                                );
                            } else {
                                ui.horizontal(|ui| {
                                    let label = self
                                        .motion_mate_selected
                                        .and_then(|id| tree.mates.get(&id))
                                        .map(|m| m.name.clone())
                                        .unwrap_or_else(|| "Pilih mate".to_string());
                                    egui::ComboBox::from_id_salt("motion_mate")
                                        .selected_text(RichText::new(label).size(9.5))
                                        .show_ui(ui, |ui| crate::theme::glass_menu(ui, |ui| {
                                            for m in &numeric_mates {
                                                ui.selectable_value(&mut self.motion_mate_selected, Some(m.id), &m.name);
                                            }
                                        }));
                                    ui.label(RichText::new("dari").size(9.5).color(TEXT_SECONDARY));
                                    ui.add(egui::TextEdit::singleline(&mut self.motion_from_input).desired_width(40.0));
                                    ui.label(RichText::new("ke").size(9.5).color(TEXT_SECONDARY));
                                    ui.add(egui::TextEdit::singleline(&mut self.motion_to_input).desired_width(40.0));
                                    if ui.button(RichText::new(ICON_ADD.codepoint).size(12.0).color(ACCENT_BLUE)).clicked() {
                                        if let Some(mate) = self.motion_mate_selected {
                                            let from = self.motion_from_input.trim().parse::<f64>().unwrap_or(0.0);
                                            let to = self.motion_to_input.trim().parse::<f64>().unwrap_or(0.0);
                                            events.push(AssemblyDrawerEvent::AddMotionStudy { mate, from, to });
                                        }
                                    }
                                });
                            }

                            for (i, study) in tree.motion_studies.iter().enumerate() {
                                let selected = i == self.motion_selected;
                                ui.horizontal(|ui| {
                                    if ui.selectable_label(selected, RichText::new(&study.name).size(10.0)).clicked() {
                                        self.motion_selected = i;
                                        self.motion_t = 0.0;
                                        self.motion_playing = false;
                                    }
                                    ui.label(RichText::new(format!("{:.1} » {:.1}", study.from, study.to)).size(9.0).color(TEXT_MUTED));
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if ui.button(RichText::new(ICON_DELETE.codepoint).size(11.0).color(TEXT_SECONDARY)).clicked() {
                                            events.push(AssemblyDrawerEvent::DeleteMotionStudy(i));
                                        }
                                    });
                                });
                            }

                            if let Some(study) = tree.motion_studies.get(self.motion_selected) {
                                ui.horizontal(|ui| {
                                    let play_icon = if self.motion_playing { "⏸" } else { ICON_PLAY_ARROW.codepoint };
                                    if ui.button(RichText::new(play_icon).size(12.0).color(ACCENT_BLUE)).clicked() {
                                        self.motion_playing = !self.motion_playing;
                                    }
                                    let mut t = self.motion_t;
                                    if ui.add(egui::Slider::new(&mut t, 0.0..=1.0).show_value(false)).changed() {
                                        self.motion_t = t;
                                        self.motion_playing = false;
                                        events.push(AssemblyDrawerEvent::ScrubMotionStudy { index: self.motion_selected, t: t as f64 });
                                    }
                                    ui.label(RichText::new(format!("{:.1}", study.value_at(self.motion_t as f64))).size(9.5).color(TEXT_MUTED));
                                });

                                if self.motion_playing {
                                    // Satu putaran penuh from->to memakan ~3 detik, bolak-balik.
                                    let dt = ui.input(|i| i.stable_dt).min(0.1);
                                    let step = dt / 3.0;
                                    let cycle = (self.motion_t + step).rem_euclid(2.0);
                                    self.motion_t = cycle;
                                    let t_eff = if cycle <= 1.0 { cycle } else { 2.0 - cycle };
                                    events.push(AssemblyDrawerEvent::ScrubMotionStudy { index: self.motion_selected, t: t_eff as f64 });
                                    ui.ctx().request_repaint();
                                }
                            }
                        });
                    }
                });
            });
        });

        events
    }
}
