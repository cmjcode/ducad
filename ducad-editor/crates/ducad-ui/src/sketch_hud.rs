//! HUD mengambang Mode Sketsa Tinta: pilih alat, "Bentuk Pintar", dan
//! "Jadikan Profil" (tinta → profil sketsa tertutup siap extrude).

use ducad_i18n::t;
use egui::{RichText, Ui, Vec2};

use crate::theme::{pill_frame, ACCENT_BLUE, MIN_TOUCH_TARGET, TEXT_SECONDARY};

/// Alat tinta yang ditampilkan di HUD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InkHudTool {
    Brush,
    Eraser,
    Lasso,
}

/// Aksi yang dipicu HUD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InkHudEvent {
    SetTool(InkHudTool),
    ToggleSmartShape,
    MakeProfile,
}

pub struct InkHud;

impl InkHud {
    /// `active`: alat aktif bila salah satu dari tiga alat HUD.
    /// `selected`: jumlah coretan terpilih (0 = "Jadikan Profil" memakai
    /// semua coretan).
    pub fn show(
        ui: &mut Ui,
        active: Option<InkHudTool>,
        smart_shape: bool,
        selected: usize,
    ) -> Option<InkHudEvent> {
        let mut event = None;
        pill_frame().show(ui, |ui| {
            ui.spacing_mut().interact_size.y = MIN_TOUCH_TARGET;
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
            ui.horizontal(|ui| {
                for (tool, key) in [
                    (InkHudTool::Brush, "ink-tool-brush"),
                    (InkHudTool::Eraser, "ink-tool-eraser"),
                    (InkHudTool::Lasso, "ink-tool-lasso"),
                ] {
                    if ui.selectable_label(active == Some(tool), t!(key)).clicked() {
                        event = Some(InkHudEvent::SetTool(tool));
                    }
                }
                ui.separator();
                if ui
                    .selectable_label(smart_shape, t!("ink-smart-shape"))
                    .on_hover_text(t!("ink-smart-shape-desc"))
                    .clicked()
                {
                    event = Some(InkHudEvent::ToggleSmartShape);
                }
                ui.separator();
                let label = if selected > 0 {
                    format!("{} ({selected})", t!("ink-to-profile"))
                } else {
                    t!("ink-to-profile")
                };
                if ui
                    .button(RichText::new(label).strong().color(ACCENT_BLUE))
                    .on_hover_text(RichText::new(t!("ink-to-profile-desc")).color(TEXT_SECONDARY))
                    .clicked()
                {
                    event = Some(InkHudEvent::MakeProfile);
                }
            });
        });
        event
    }
}
