//! Strip Ikon Constraint & Snap Bergaya Shapr3D dengan Material Icons.
//!
//! Menampilkan strip ikon vertikal mengambang di sebelah kanan kanvas
//! untuk akses cepat pemasangan constraint geometris dan pengaturan snap.

use egui::{RichText, Ui, Vec2};
use egui_material_icons::icons::{ICON_LINK, ICON_LOCK};
use crate::theme::{glass_frame, ACCENT_BLUE, TEXT_PRIMARY, TEXT_SECONDARY};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintAction {
    ToggleSnap,
    ApplyHorizontal,
    ApplyVertical,
    ApplyParallel,
    ApplyPerpendicular,
    ApplyEqualLength,
    ApplyEqualRadius,
    ApplyTangent,
    ApplyCoincident,
    ApplyFixed,
    ApplySymmetric,
}

pub struct ConstraintStrip {
    pub snap_enabled: bool,
}

impl Default for ConstraintStrip {
    fn default() -> Self {
        Self { snap_enabled: true }
    }
}

impl ConstraintStrip {
    pub fn new() -> Self {
        Self::default()
    }

    /// Render strip ikon constraint vertikal. Mengembalikan `Option<ConstraintAction>`.
    pub fn show(&mut self, ui: &mut Ui, selected_count: usize) -> Option<ConstraintAction> {
        let mut action = None;

        glass_frame().show(ui, |ui| {
            ui.set_width(38.0);
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 4.0);

            // 1. Magnet / Pin Snap Toggle
            let snap_color = if self.snap_enabled { ACCENT_BLUE } else { TEXT_SECONDARY };
            let snap_btn = ui.button(RichText::new(ICON_LINK).size(16.0).color(snap_color));
            if snap_btn.on_hover_text("Toggle Snapping (Grid & Endpoint)").clicked() {
                self.snap_enabled = !self.snap_enabled;
                action = Some(ConstraintAction::ToggleSnap);
            }

            ui.separator();

            // 2. Constraint Icons (aktif jika ada entitas terpilih)
            let enabled = selected_count > 0;

            let constraints = [
                (ConstraintAction::ApplyHorizontal, "—", "Horizontal (1 Garis)"),
                (ConstraintAction::ApplyVertical, "|", "Vertical (1 Garis)"),
                (ConstraintAction::ApplyParallel, "//", "Parallel / Sejajar (2 Garis)"),
                (ConstraintAction::ApplyPerpendicular, "⊥", "Perpendicular / Tegak Lurus (2 Garis)"),
                (ConstraintAction::ApplyEqualLength, "==", "Equal Length / Sama Panjang (2 Garis)"),
                (ConstraintAction::ApplyEqualRadius, "=R", "Equal Radius (2 Lingkaran/Arc)"),
                (ConstraintAction::ApplyTangent, "tan", "Tangent / Bersinggungan"),
                (ConstraintAction::ApplyCoincident, "><", "Coincident / Berimpit (Titik)"),
                (ConstraintAction::ApplyFixed, ICON_LOCK, "Lock / Fixed (Titik)"),
                (ConstraintAction::ApplySymmetric, "sym", "Symmetric / Simetris"),
            ];

            for (act, icon, tooltip) in constraints {
                let btn = ui.add_enabled(
                    enabled,
                    egui::Button::new(RichText::new(icon).size(12.0).strong().color(TEXT_PRIMARY)),
                );
                if btn.on_hover_text(tooltip).clicked() {
                    action = Some(act);
                }
            }
        });

        action
    }
}
