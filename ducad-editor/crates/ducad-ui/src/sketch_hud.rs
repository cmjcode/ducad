//! HUD mengambang pensil, tepat di bawah header — tampil hanya saat alat
//! pensil (Freehand) aktif atau di Mode Tinta: tombol **Objek Tertutup**
//! (ditekan SETELAH selesai menggambar) dan, di Mode Tinta, pilihan alat +
//! "Bentuk Pintar".

use ducad_i18n::t;
use egui::{Color32, CornerRadius, RichText, Ui, Vec2};
use egui_icons::icons::ICON_JOIN;

use crate::theme::{pill_frame, ACCENT_BLUE, MIN_TOUCH_TARGET};

/// Alat tinta yang ditampilkan di HUD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InkHudTool {
    Brush,
    Eraser,
    Lasso,
}

/// Kontrol khusus Mode Tinta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InkHudState {
    /// Alat aktif bila salah satu dari tiga alat HUD.
    pub active: Option<InkHudTool>,
    pub smart_shape: bool,
}

/// Aksi yang dipicu HUD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SketchHudEvent {
    SetInkTool(InkHudTool),
    ToggleSmartShape,
    ConvertToClosedObjects,
}

pub struct SketchHud;

impl SketchHud {
    /// `ink`: `Some` di Mode Tinta (menampilkan alat tinta).
    pub fn show(ui: &mut Ui, ink: Option<InkHudState>) -> Option<SketchHudEvent> {
        let mut event = None;
        pill_frame().show(ui, |ui| {
            ui.spacing_mut().interact_size.y = MIN_TOUCH_TARGET;
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
            ui.horizontal(|ui| {
                if let Some(ink) = ink {
                    for (tool, key) in [
                        (InkHudTool::Brush, "ink-tool-brush"),
                        (InkHudTool::Eraser, "ink-tool-eraser"),
                        (InkHudTool::Lasso, "ink-tool-lasso"),
                    ] {
                        if ui
                            .selectable_label(ink.active == Some(tool), t!(key))
                            .clicked()
                        {
                            event = Some(SketchHudEvent::SetInkTool(tool));
                        }
                    }
                    ui.separator();
                    if ui
                        .selectable_label(ink.smart_shape, t!("ink-smart-shape"))
                        .on_hover_text(t!("ink-smart-shape-desc"))
                        .clicked()
                    {
                        event = Some(SketchHudEvent::ToggleSmartShape);
                    }
                    ui.separator();
                }
                let close_btn = ui
                    .add(
                        egui::Button::new(
                            RichText::new(format!(
                                "{} {}",
                                ICON_JOIN.codepoint,
                                t!("hud-close-objects")
                            ))
                            .size(12.0)
                            .strong()
                            .color(Color32::WHITE),
                        )
                        .fill(ACCENT_BLUE)
                        .corner_radius(CornerRadius::same(5)),
                    )
                    .on_hover_text(t!("hud-close-objects-desc"));
                if close_btn.clicked() {
                    event = Some(SketchHudEvent::ConvertToClosedObjects);
                }
            });
        });
        event
    }
}
