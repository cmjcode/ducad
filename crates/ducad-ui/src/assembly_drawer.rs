//! Assembly Tree & Mate Constraints Drawer — Panel Hierarki Perakitan & Pengelolaan Mate.
//!
//! Menampilkan panel dock di pojok kanan bawah kanvas untuk:
//! - Visualisasi pohon hierarki part instance dan sub-assembly.
//! - Pengaturan status Grounded (terkunci) dan pelacakan Derajat Kebebasan (DOF).
//! - Daftar kendala mate 3D (Concentric, Coincident, Distance, Angle) dengan status dan parameter.

use crate::theme::{
    card_frame, glass_frame, ACCENT_BLUE, ACCENT_ORANGE, TEXT_MUTED, TEXT_PRIMARY,
    TEXT_SECONDARY,
};
use ducad_core::assembly::{
    AssemblyInstanceId, AssemblyTree, MateConstraint, MateConstraintId, MateKind, MateStatus,
    SubAssemblyId,
};
use ducad_i18n::t;
use egui::{
    Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, ScrollArea, Stroke, Ui,
};
use egui_material_icons::icons::{
    ICON_ADJUST, ICON_ARCHITECTURE, ICON_CALL_MERGE, ICON_CATEGORY, ICON_CHECK_CIRCLE, ICON_CLEAR,
    ICON_CLOSE, ICON_DELETE, ICON_EDIT, ICON_ERROR, ICON_FLIP, ICON_FOLDER, ICON_HORIZONTAL_RULE,
    ICON_KEYBOARD_ARROW_DOWN, ICON_KEYBOARD_ARROW_RIGHT, ICON_LOCK, ICON_LOCK_OPEN, ICON_PLAY_ARROW,
    ICON_SEARCH, ICON_STRAIGHTEN,
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
    /// Tutup panel Assembly Tree.
    Close,
}

pub struct AssemblyDrawer {
    pub search_query: String,
    pub custom_height: Option<f32>,
    pub components_expanded: bool,
    pub mates_expanded: bool,
    pub editing_mate_id: Option<MateConstraintId>,
    pub edit_input_val: String,
    pub edit_flip_alignment: bool,
}

impl Default for AssemblyDrawer {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            custom_height: None,
            components_expanded: true,
            mates_expanded: true,
            editing_mate_id: None,
            edit_input_val: String::new(),
            edit_flip_alignment: false,
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
    ) -> Vec<AssemblyDrawerEvent> {
        let mut events = Vec::new();

        glass_frame().show(ui, |ui| {
            let width = 340.0;
            let height = self.custom_height.unwrap_or(480.0);
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
                });
            });
        });

        events
    }
}
