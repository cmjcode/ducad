//! Revolve Dialog & Alert Modal bergaya Shapr3D dengan Material Icons.
//!
//! Menyediakan:
//! 1. `RevolveDialog`: Dialog panduan dan konfigurasi Revolve yang ramah pengguna awam
//!    (pilihan preset sumbu, sudut putaran, status deteksi profil).
//! 2. `AlertModal`: Modal popup peringatan untuk operasi yang gagal atau salah pakai.

use ducad_i18n::t;
use egui::{Align2, Color32, RichText, Vec2, Window};
use egui_icons::icons::{
    ICON_CHECK_CIRCLE, ICON_INFO, ICON_REFRESH, ICON_WARNING,
};

use crate::theme::{
    card_frame, glass_frame, ACCENT_BLUE, ACCENT_GREEN, ACCENT_ORANGE, TEXT_MUTED, TEXT_PRIMARY,
    TEXT_SECONDARY,
};

/// Pilihan mode sumbu putar Revolve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevolveAxisPreset {
    /// Sumbu Y Origin (x=0, y=0 -> x=0, y=1)
    YAxisOrigin,
    /// Sumbu X Origin (x=0, y=0 -> x=1, y=0)
    XAxisOrigin,
    /// Tepi Kiri Bounding Box Profil
    BBoxLeft,
    /// Tepi Kanan Bounding Box Profil
    BBoxRight,
    /// Tepi Bawah Bounding Box Profil
    BBoxBottom,
    /// Tepi Atas Bounding Box Profil
    BBoxTop,
    /// Gambar Manual 2 Titik di Kanvas
    CustomTwoPoints,
}

/// Event yang dihasilkan dari Revolve Dialog.
#[derive(Debug, Clone)]
pub enum RevolveDialogEvent {
    /// Jalankan Revolve dengan preset sumbu dan sudut tertentu.
    Execute {
        axis: RevolveAxisPreset,
        angle_deg: f64,
    },
    /// Aktifkan mode klik 2 titik manual di kanvas.
    StartManualAxisPicking {
        angle_deg: f64,
    },
    /// Tutup dialog.
    Close,
}

/// State untuk Revolve Dialog.
#[derive(Debug, Clone)]
pub struct RevolveDialogState {
    pub is_open: bool,
    pub has_valid_profile: bool,
    pub profile_entities_count: usize,
    pub axis_preset: RevolveAxisPreset,
    pub angle_deg: f64,
    pub angle_input: String,
}

impl Default for RevolveDialogState {
    fn default() -> Self {
        Self {
            is_open: false,
            has_valid_profile: false,
            profile_entities_count: 0,
            axis_preset: RevolveAxisPreset::YAxisOrigin,
            angle_deg: 360.0,
            angle_input: "360.0".to_string(),
        }
    }
}

/// Dialog Konfigurasi dan Panduan Revolve.
pub struct RevolveDialog;

impl RevolveDialog {
    pub fn show(
        ctx: &egui::Context,
        state: &mut RevolveDialogState,
    ) -> Option<RevolveDialogEvent> {
        if !state.is_open {
            return None;
        }

        let mut event = None;
        let mut is_open = state.is_open;

        let window_title = t!("revolve-dialog-window-title");
        let window_response = Window::new(window_title)
            .open(&mut is_open)
            .anchor(Align2::CENTER_CENTER, Vec2::new(0.0, 0.0))
            .resizable(false)
            .collapsible(false)
            .frame(glass_frame())
            .fixed_size(Vec2::new(420.0, 480.0))
            .show(ctx, |ui| {
                ui.add_space(4.0);

                // Header Info
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(ICON_REFRESH.codepoint)
                            .size(22.0)
                            .color(ACCENT_BLUE),
                    );
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(t!("revolve-dialog-header-title"))
                                .size(14.0)
                                .strong()
                                .color(TEXT_PRIMARY),
                        );
                        ui.label(
                            RichText::new(t!("revolve-dialog-header-desc"))
                                .size(11.0)
                                .color(TEXT_SECONDARY),
                        );
                    });
                });

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);

                // 1. Status Profil Terpilih
                card_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if state.has_valid_profile {
                            ui.label(
                                RichText::new(ICON_CHECK_CIRCLE.codepoint)
                                    .size(16.0)
                                    .color(ACCENT_GREEN),
                            );
                            ui.label(
                                RichText::new(t!("revolve-dialog-profile-ready", count = state.profile_entities_count))
                                .size(11.5)
                                .strong()
                                .color(ACCENT_GREEN),
                            );
                        } else {
                            ui.label(
                                RichText::new(ICON_WARNING.codepoint)
                                    .size(16.0)
                                    .color(ACCENT_ORANGE),
                            );
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(t!("revolve-dialog-no-profile"))
                                        .size(11.5)
                                        .strong()
                                        .color(ACCENT_ORANGE),
                                );
                                ui.label(
                                    RichText::new(t!("revolve-dialog-select-hint"))
                                        .size(10.5)
                                        .color(TEXT_SECONDARY),
                                );
                            });
                        }
                    });
                });

                ui.add_space(6.0);

                // 2. Pilihan Sumbu Putar (Poros)
                ui.label(
                    RichText::new(t!("revolve-dialog-select-axis-prompt"))
                        .size(12.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.add_space(2.0);

                card_frame().show(ui, |ui| {
                    ui.radio_value(
                        &mut state.axis_preset,
                        RevolveAxisPreset::YAxisOrigin,
                        RichText::new(t!("revolve-dialog-axis-y-origin")).size(11.0),
                    );
                    ui.radio_value(
                        &mut state.axis_preset,
                        RevolveAxisPreset::XAxisOrigin,
                        RichText::new(t!("revolve-dialog-axis-x-origin")).size(11.0),
                    );
                    ui.radio_value(
                        &mut state.axis_preset,
                        RevolveAxisPreset::BBoxLeft,
                        RichText::new(t!("revolve-dialog-axis-bbox-left")).size(11.0),
                    );
                    ui.radio_value(
                        &mut state.axis_preset,
                        RevolveAxisPreset::BBoxBottom,
                        RichText::new(t!("revolve-dialog-axis-bbox-bottom")).size(11.0),
                    );
                    ui.radio_value(
                        &mut state.axis_preset,
                        RevolveAxisPreset::CustomTwoPoints,
                        RichText::new(t!("revolve-dialog-axis-manual")).size(11.0),
                    );
                });

                ui.add_space(6.0);

                // 3. Pilihan Sudut Putaran
                ui.label(
                    RichText::new(t!("revolve-dialog-select-angle-prompt"))
                        .size(12.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.add_space(2.0);

                card_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui.selectable_label(state.angle_deg == 360.0, t!("revolve-dialog-angle-360")).clicked() {
                            state.angle_deg = 360.0;
                            state.angle_input = "360.0".to_string();
                        }
                        if ui.selectable_label(state.angle_deg == 180.0, t!("revolve-dialog-angle-180")).clicked() {
                            state.angle_deg = 180.0;
                            state.angle_input = "180.0".to_string();
                        }
                        if ui.selectable_label(state.angle_deg == 90.0, t!("revolve-dialog-angle-90")).clicked() {
                            state.angle_deg = 90.0;
                            state.angle_input = "90.0".to_string();
                        }
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(t!("revolve-dialog-custom-deg")).size(11.0).color(TEXT_SECONDARY));
                        let resp = ui.add_sized(
                            Vec2::new(70.0, 20.0),
                            egui::TextEdit::singleline(&mut state.angle_input),
                        );
                        if resp.changed() {
                            if let Ok(val) = state.angle_input.trim().parse::<f64>() {
                                if val > 0.0 && val <= 360.0 {
                                    state.angle_deg = val;
                                }
                            }
                        }
                        ui.label(RichText::new("° (1° - 360°)").size(10.5).color(TEXT_MUTED));
                    });
                });

                ui.add_space(6.0);

                // Tips Pemakaian
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(ICON_INFO.codepoint)
                            .size(14.0)
                            .color(ACCENT_BLUE),
                    );
                    ui.label(
                        RichText::new(t!("revolve-dialog-tip"))
                            .size(10.0)
                            .color(TEXT_SECONDARY),
                    );
                });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);

                // Action Buttons
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if state.axis_preset == RevolveAxisPreset::CustomTwoPoints {
                        let btn = ui.add_enabled(
                            state.has_valid_profile,
                            egui::Button::new(
                                RichText::new(t!("revolve-dialog-start-manual-btn"))
                                    .size(11.5)
                                    .strong()
                                    .color(if state.has_valid_profile {
                                        Color32::WHITE
                                    } else {
                                        TEXT_MUTED
                                    }),
                            )
                            .fill(ACCENT_BLUE),
                        );
                        if btn.clicked() {
                            event = Some(RevolveDialogEvent::StartManualAxisPicking {
                                angle_deg: state.angle_deg,
                            });
                        }
                    } else {
                        let btn = ui.add_enabled(
                            state.has_valid_profile,
                            egui::Button::new(
                                RichText::new(t!("inspector-exec-revolve"))
                                    .size(11.5)
                                    .strong()
                                    .color(if state.has_valid_profile {
                                        Color32::WHITE
                                    } else {
                                        TEXT_MUTED
                                    }),
                            )
                            .fill(ACCENT_BLUE),
                        );
                        if btn.clicked() {
                            event = Some(RevolveDialogEvent::Execute {
                                axis: state.axis_preset,
                                angle_deg: state.angle_deg,
                            });
                        }
                    }
                });
            });

        // ESC listener
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            event = Some(RevolveDialogEvent::Close);
        }

        // Tap/click-outside dialog
        if event.is_none() {
            if let Some(ref resp) = window_response {
                if let Some(pointer_pos) = ctx.input(|i| i.pointer.interact_pos()) {
                    if ctx.input(|i| i.pointer.any_pressed()) && !resp.response.rect.contains(pointer_pos) {
                        event = Some(RevolveDialogEvent::Close);
                    }
                }
            }
        }

        state.is_open = is_open;
        if !is_open && event.is_none() {
            event = Some(RevolveDialogEvent::Close);
        }

        event
    }
}

/// State untuk Alert Modal Peringatan.
#[derive(Debug, Clone)]
pub struct AlertModalState {
    pub is_open: bool,
    pub title: String,
    pub message: String,
    pub suggestions: Vec<String>,
}

impl Default for AlertModalState {
    fn default() -> Self {
        Self {
            is_open: false,
            title: t!("alert-modal-default-title"),
            message: String::new(),
            suggestions: Vec::new(),
        }
    }
}

impl AlertModalState {
    pub fn show_error(
        &mut self,
        title: impl Into<String>,
        message: impl Into<String>,
        suggestions: Vec<&str>,
    ) {
        self.is_open = true;
        self.title = title.into();
        self.message = message.into();
        self.suggestions = suggestions.into_iter().map(String::from).collect();
    }
}

/// Modal Alert / Peringatan Umum.
pub struct AlertModal;

impl AlertModal {
    pub fn show(ctx: &egui::Context, state: &mut AlertModalState) -> bool {
        if !state.is_open {
            return false;
        }

        let mut is_open = state.is_open;
        let mut closed = false;

        Window::new(format!("⚠️ {}", state.title))
            .open(&mut is_open)
            .anchor(Align2::CENTER_CENTER, Vec2::new(0.0, 0.0))
            .resizable(false)
            .collapsible(false)
            .frame(glass_frame())
            .fixed_size(Vec2::new(400.0, 260.0))
            .show(ctx, |ui| {
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(ICON_WARNING.codepoint)
                            .size(24.0)
                            .color(ACCENT_ORANGE),
                    );
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(&state.title)
                                .size(13.5)
                                .strong()
                                .color(TEXT_PRIMARY),
                        );
                        ui.label(
                            RichText::new(&state.message)
                                .size(11.0)
                                .color(TEXT_SECONDARY),
                        );
                    });
                });

                if !state.suggestions.is_empty() {
                    ui.add_space(8.0);
                    card_frame().show(ui, |ui| {
                        ui.label(
                            RichText::new(t!("alert-modal-tips-title"))
                                .size(11.0)
                                .strong()
                                .color(ACCENT_BLUE),
                        );
                        ui.add_space(2.0);
                        for (i, sug) in state.suggestions.iter().enumerate() {
                            ui.label(
                                RichText::new(format!("{}. {}", i + 1, sug))
                                    .size(10.5)
                                    .color(TEXT_PRIMARY),
                            );
                        }
                    });
                }

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(t!("alert-modal-dismiss-btn"))
                                    .size(11.5)
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .fill(ACCENT_BLUE),
                        )
                        .clicked()
                    {
                        closed = true;
                    }
                });
            });

        if closed || !is_open {
            state.is_open = false;
        }

        closed
    }
}
