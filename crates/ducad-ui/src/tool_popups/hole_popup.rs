//! Hole Wizard Tool Popup — Pojok Kanan Bawah.
//!
//! Dialog interaktif pembuatan lubang standar ISO (Simple, Counterbore, Countersink, Tapped)
//! dengan tabel ukuran metrik M2-M12 dan penyesuaian dimensi parametrik.

use ducad_core::hole::{HoleKind, HoleSpec, IsoMetricThread};
use ducad_i18n::t;
use egui::{
    Align, Color32, CornerRadius, DragValue, Layout, RichText, Ui, Vec2,
};
use egui_material_icons::icons::ICON_CHECK;

use super::ToolPopupEvent;
use crate::theme::{
    ACCENT_BLUE, ACCENT_ORANGE, BORDER_SUBTLE, TEXT_PRIMARY, TEXT_SECONDARY,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoleOperationMode {
    NewHole,
    EditHole,
}

#[derive(Debug, Clone)]
pub struct HolePopupState {
    pub spec: HoleSpec,
    /// Pergeseran posisi lubang dari titik referensi face sepanjang sumbu U (mm).
    pub offset_u: f64,
    /// Pergeseran posisi lubang dari titik referensi face sepanjang sumbu V (mm).
    pub offset_v: f64,
    /// Posisi 3D absolut titik lubang saat ini (x, y, z).
    pub current_pos_3d: Option<(f64, f64, f64)>,
    /// Apakah pengguna sedang men-drag titik lubang secara interaktif.
    pub is_dragging: bool,
    /// Mode operasi: Buat lubang baru atau Edit lubang eksisting
    pub mode: HoleOperationMode,
    /// Apakah ada lubang eksisting yang terdeteksi pada face terpilih
    pub has_existing_hole: bool,
    /// Daftar lubang yang tersedia pada bodi untuk diedit: (index, label_summary)
    pub available_holes: Vec<(usize, String)>,
    /// Indeks lubang yang sedang dipilih untuk diedit
    pub selected_hole_idx: Option<usize>,
}

impl Default for HolePopupState {
    fn default() -> Self {
        Self {
            spec: HoleSpec::default(),
            offset_u: 0.0,
            offset_v: 0.0,
            current_pos_3d: None,
            is_dragging: false,
            mode: HoleOperationMode::NewHole,
            has_existing_hole: false,
            available_holes: Vec::new(),
            selected_hole_idx: None,
        }
    }
}

pub struct HolePopup;

impl HolePopup {
    /// Render konten panel Hole Wizard di pojok kanan bawah (tanpa frame luar ganda).
    pub fn show(ui: &mut Ui, state: &mut HolePopupState) -> Option<ToolPopupEvent> {
        let mut event = None;
        let mut apply_clicked = false;

        const DRAWER_W: f32 = 256.0;
        ui.set_min_width(DRAWER_W);
        ui.set_max_width(DRAWER_W);
        ui.set_width(DRAWER_W);
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 5.0);

        // =========================================================================
        // 1. MODE OPERASI (BUAT BARU / EDIT LUBANG) - SELALU TAMPIL
        // =========================================================================
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(t!("hole-mode"))
                    .strong()
                    .size(10.5)
                    .color(TEXT_PRIMARY),
            );

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let is_edit = state.mode == HoleOperationMode::EditHole;
                let edit_bg = if is_edit {
                    ACCENT_ORANGE
                } else {
                    Color32::from_rgba_premultiplied(35, 38, 48, 180)
                };
                let edit_text = if is_edit {
                    Color32::WHITE
                } else {
                    TEXT_SECONDARY
                };

                let edit_btn = egui::Button::new(
                    RichText::new(format!("✏️ {}", t!("hole-mode-edit")))
                        .size(10.0)
                        .strong()
                        .color(edit_text),
                )
                .fill(edit_bg)
                .corner_radius(CornerRadius::same(4))
                .min_size(Vec2::new(64.0, 22.0));

                if ui.add(edit_btn).clicked() {
                    state.mode = HoleOperationMode::EditHole;
                }

                let is_new = state.mode == HoleOperationMode::NewHole;
                let new_bg = if is_new {
                    ACCENT_BLUE
                } else {
                    Color32::from_rgba_premultiplied(35, 38, 48, 180)
                };
                let new_text = if is_new {
                    Color32::WHITE
                } else {
                    TEXT_SECONDARY
                };

                let new_btn = egui::Button::new(
                    RichText::new(format!("➕ {}", t!("hole-mode-new")))
                        .size(10.0)
                        .strong()
                        .color(new_text),
                )
                .fill(new_bg)
                .corner_radius(CornerRadius::same(4))
                .min_size(Vec2::new(64.0, 22.0));

                if ui.add(new_btn).clicked() {
                    state.mode = HoleOperationMode::NewHole;
                }
            });
        });
        ui.separator();

        // =========================================================================
        // 1.5. COMBOBOX PILIH LUBANG (HANYA MUNCUL PADA MODE EDIT)
        // =========================================================================
        if state.mode == HoleOperationMode::EditHole {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(t!("hole-select-target"))
                        .size(10.0)
                        .color(TEXT_PRIMARY)
                        .strong(),
                );
            });

            if state.available_holes.is_empty() {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("ℹ️ {}", t!("hole-no-holes-found")))
                        .size(9.5)
                        .color(Color32::from_rgb(220, 180, 100)),
                );
                ui.add_space(4.0);
                return event;
            }

            let curr_idx = state.selected_hole_idx.unwrap_or(0);
            let selected_text = state
                .available_holes
                .iter()
                .find(|(i, _)| *i == curr_idx)
                .map(|(_, text)| text.clone())
                .unwrap_or_else(|| state.available_holes[0].1.clone());

            egui::ComboBox::from_id_salt("hole_wizard_select_combobox")
                .width(DRAWER_W - 8.0)
                .selected_text(RichText::new(&selected_text).size(10.0).color(TEXT_PRIMARY))
                .show_ui(ui, |ui| {
                    for (idx, label) in &state.available_holes {
                        if ui
                            .selectable_value(&mut state.selected_hole_idx, Some(*idx), RichText::new(label).size(9.5))
                            .clicked()
                        {
                            state.selected_hole_idx = Some(*idx);
                        }
                    }
                });

            ui.add_space(2.0);
            ui.separator();
        }

        // =========================================================================
        // 2. TIPE LUBANG (HOLE TYPE TABS)
        // =========================================================================
        ui.label(
            RichText::new(t!("hole-type"))
                .strong()
                .size(10.5)
                .color(TEXT_PRIMARY),
        );

        let kinds = [
            (HoleKind::Simple, "Simple"),
            (HoleKind::Counterbore, "C-Bore"),
            (HoleKind::Countersink, "C-Sink"),
            (HoleKind::Tapped, "Tapped"),
        ];

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(3.0, 2.0);
            for (k, lbl) in kinds {
                let is_active = state.spec.kind == k;
                let bg = if is_active {
                    ACCENT_BLUE
                } else {
                    Color32::from_rgba_premultiplied(35, 38, 48, 180)
                };
                let text_color = if is_active {
                    Color32::WHITE
                } else {
                    TEXT_SECONDARY
                };

                let btn = egui::Button::new(
                    RichText::new(lbl)
                        .size(10.0)
                        .color(text_color)
                        .strong(),
                )
                .fill(bg)
                .stroke(egui::Stroke::new(
                    1.0,
                    if is_active {
                        ACCENT_BLUE
                    } else {
                        BORDER_SUBTLE
                    },
                ))
                .corner_radius(CornerRadius::same(4))
                .min_size(Vec2::new(56.0, 22.0));

                if ui.add(btn).clicked() {
                    state.spec.kind = k;
                }
            }
        });

        ui.add_space(2.0);
        ui.separator();

        // =========================================================================
        // 3. STANDAR BAUT ISO PRESET PILLS
        // =========================================================================
        ui.label(
            RichText::new(t!("hole-iso-standard"))
                .strong()
                .size(10.5)
                .color(TEXT_PRIMARY),
        );

        let presets = [
            IsoMetricThread::M2,
            IsoMetricThread::M2_5,
            IsoMetricThread::M3,
            IsoMetricThread::M4,
            IsoMetricThread::M5,
            IsoMetricThread::M6,
            IsoMetricThread::M8,
            IsoMetricThread::M10,
            IsoMetricThread::M12,
        ];

        // Baris 1: M2, M2.5, M3, M4, M5
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(3.0, 2.0);
            for &p in &presets[0..5] {
                let is_active = state.spec.thread_size == p;
                let bg = if is_active {
                    ACCENT_ORANGE
                } else {
                    Color32::from_rgba_premultiplied(30, 32, 40, 160)
                };
                let text_color = if is_active {
                    Color32::WHITE
                } else {
                    TEXT_SECONDARY
                };

                let btn = egui::Button::new(
                    RichText::new(p.label())
                        .size(9.5)
                        .color(text_color)
                        .strong(),
                )
                .fill(bg)
                .stroke(egui::Stroke::new(
                    1.0,
                    if is_active {
                        ACCENT_ORANGE
                    } else {
                        BORDER_SUBTLE
                    },
                ))
                .corner_radius(CornerRadius::same(4))
                .min_size(Vec2::new(44.0, 20.0));

                if ui.add(btn).clicked() {
                    let curr_depth = state.spec.depth;
                    let is_through = state.spec.is_through;
                    let has_drill_tip = state.spec.has_drill_tip;
                    state.spec = HoleSpec::for_iso(p, state.spec.kind, curr_depth);
                    state.spec.is_through = is_through;
                    state.spec.has_drill_tip = has_drill_tip;
                }
            }
        });

        // Baris 2: M6, M8, M10, M12
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(3.0, 2.0);
            for &p in &presets[5..9] {
                let is_active = state.spec.thread_size == p;
                let bg = if is_active {
                    ACCENT_ORANGE
                } else {
                    Color32::from_rgba_premultiplied(30, 32, 40, 160)
                };
                let text_color = if is_active {
                    Color32::WHITE
                } else {
                    TEXT_SECONDARY
                };

                let btn = egui::Button::new(
                    RichText::new(p.label())
                        .size(9.5)
                        .color(text_color)
                        .strong(),
                )
                .fill(bg)
                .stroke(egui::Stroke::new(
                    1.0,
                    if is_active {
                        ACCENT_ORANGE
                    } else {
                        BORDER_SUBTLE
                    },
                ))
                .corner_radius(CornerRadius::same(4))
                .min_size(Vec2::new(56.0, 20.0));

                if ui.add(btn).clicked() {
                    let curr_depth = state.spec.depth;
                    let is_through = state.spec.is_through;
                    let has_drill_tip = state.spec.has_drill_tip;
                    state.spec = HoleSpec::for_iso(p, state.spec.kind, curr_depth);
                    state.spec.is_through = is_through;
                    state.spec.has_drill_tip = has_drill_tip;
                }
            }
        });

        // Info Callout Baut Aktif
        let callout_text = state.spec.technical_callout();
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("🏷 {}", callout_text))
                    .size(9.5)
                    .color(ACCENT_BLUE),
            );
        });

        ui.add_space(2.0);
        ui.separator();

        // =========================================================================
        // 4. PARAMETER DIMENSI (INPUT HARGA / SLIDER)
        // =========================================================================
        // 4.1 Diameter Lubang Utama
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(t!("hole-dia"))
                    .size(10.0)
                    .color(TEXT_PRIMARY),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add(
                    DragValue::new(&mut state.spec.diameter)
                        .range(0.5..=100.0)
                        .speed(0.1)
                        .suffix(" mm"),
                );
            });
        });

        // 4.2 Kedalaman (Depth / Through All)
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(t!("hole-depth"))
                    .size(10.0)
                    .color(TEXT_PRIMARY),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if state.spec.is_through {
                    ui.label(
                        RichText::new(t!("hole-through-all"))
                            .size(10.0)
                            .color(ACCENT_ORANGE),
                    );
                } else {
                    ui.add(
                        DragValue::new(&mut state.spec.depth)
                            .range(0.5..=500.0)
                            .speed(0.5)
                            .suffix(" mm"),
                    );
                }
            });
        });

        // Checkbox Tembus (Through All)
        ui.checkbox(&mut state.spec.is_through, t!("hole-through-all"));

        // Parameter khusus berdasarkan tipe lubang
        match state.spec.kind {
            HoleKind::Counterbore => {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(t!("hole-cbore-dia"))
                            .size(10.0)
                            .color(TEXT_SECONDARY),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add(
                            DragValue::new(&mut state.spec.counterbore_diameter)
                                .range(state.spec.diameter..=200.0)
                                .speed(0.1)
                                .suffix(" mm"),
                        );
                    });
                });
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(t!("hole-cbore-depth"))
                            .size(10.0)
                            .color(TEXT_SECONDARY),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add(
                            DragValue::new(&mut state.spec.counterbore_depth)
                                .range(0.1..=100.0)
                                .speed(0.1)
                                .suffix(" mm"),
                        );
                    });
                });
            }
            HoleKind::Countersink => {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(t!("hole-csink-dia"))
                            .size(10.0)
                            .color(TEXT_SECONDARY),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add(
                            DragValue::new(&mut state.spec.countersink_diameter)
                                .range(state.spec.diameter..=200.0)
                                .speed(0.1)
                                .suffix(" mm"),
                        );
                    });
                });
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(t!("hole-csink-angle"))
                            .size(10.0)
                            .color(TEXT_SECONDARY),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add(
                            DragValue::new(&mut state.spec.countersink_angle_deg)
                                .range(30.0..=150.0)
                                .speed(1.0)
                                .suffix("°"),
                        );
                    });
                });
            }
            HoleKind::Tapped => {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(t!("hole-thread-pitch"))
                            .size(10.0)
                            .color(TEXT_SECONDARY),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add(
                            DragValue::new(&mut state.spec.thread_pitch)
                                .range(0.1..=10.0)
                                .speed(0.05)
                                .suffix(" mm"),
                        );
                    });
                });
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(t!("hole-thread-depth"))
                            .size(10.0)
                            .color(TEXT_SECONDARY),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add(
                            DragValue::new(&mut state.spec.thread_depth)
                                .range(0.5..=state.spec.depth)
                                .speed(0.5)
                                .suffix(" mm"),
                        );
                    });
                });
            }
            HoleKind::Simple => {}
        }

        // Checkbox Ujung Bor 118°
        if !state.spec.is_through {
            ui.checkbox(&mut state.spec.has_drill_tip, t!("hole-drill-tip"));
        }

        ui.add_space(2.0);
        ui.separator();

        // =========================================================================
        // 5. POSISI & GESER LUBANG (PLACEMENT & OFFSET)
        // =========================================================================
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(t!("hole-pos-offset"))
                    .strong()
                    .size(10.5)
                    .color(TEXT_PRIMARY),
            );

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .button(
                        RichText::new(t!("hole-center-btn"))
                            .size(9.0)
                            .color(Color32::from_rgb(180, 225, 255)),
                    )
                    .clicked()
                {
                    state.offset_u = 0.0;
                    state.offset_v = 0.0;
                    state.current_pos_3d = None;
                }
            });
        });

        ui.horizontal(|ui| {
            ui.label(
                RichText::new(t!("hole-offset-u"))
                    .size(10.0)
                    .color(TEXT_SECONDARY),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add(
                    DragValue::new(&mut state.offset_u)
                        .speed(0.2)
                        .suffix(" mm"),
                );
            });
        });

        ui.horizontal(|ui| {
            ui.label(
                RichText::new(t!("hole-offset-v"))
                    .size(10.0)
                    .color(TEXT_SECONDARY),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add(
                    DragValue::new(&mut state.offset_v)
                        .speed(0.2)
                        .suffix(" mm"),
                );
            });
        });

        ui.label(
            RichText::new(format!("💡 {}", t!("hole-drag-hint")))
                .italics()
                .size(9.0)
                .color(Color32::from_rgb(150, 190, 220)),
        );

        ui.add_space(3.0);
        ui.separator();
        ui.add_space(2.0);

        // =========================================================================
        // 6. TOMBOL TERAPKAN LUBANG (APPLY BUTTON)
        // =========================================================================
        let is_editing = state.mode == HoleOperationMode::EditHole;
        let btn_label = if is_editing {
            t!("hole-update-apply")
        } else {
            t!("hole-apply")
        };

        let apply_btn = egui::Button::new(
            RichText::new(format!(
                "{} {}",
                ICON_CHECK.codepoint,
                btn_label
            ))
            .size(11.5)
            .strong()
            .color(Color32::WHITE),
        )
        .fill(ACCENT_BLUE)
        .corner_radius(CornerRadius::same(5))
        .min_size(Vec2::new(DRAWER_W, 26.0));

        if ui.add(apply_btn).clicked() {
            apply_clicked = true;
        }

        if apply_clicked {
            event = Some(ToolPopupEvent::ApplyHole(state.spec));
        }

        event
    }
}
