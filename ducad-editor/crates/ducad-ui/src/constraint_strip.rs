//! Baris ikon constraint sketsa untuk contextual action bar bawah.
//!
//! Dulu strip vertikal mengambang di kanan kanvas yang selalu menampilkan 16
//! tombol (sebagian besar nonaktif). Sekarang: baris horizontal ringkas yang
//! HANYA memuat constraint yang berlaku untuk seleksi saat ini, dipasang di
//! context bar bawah (seleksi sketsa) atau bar node (Node Edit). Tidak ada
//! seleksi → tidak ada baris. Lebar maksimum ≈ 9 ikon, jauh lebih sempit dari
//! baris aksi pemodelan, sehingga aman di layar kecil.

use ducad_i18n::t;
use egui::{Button, RichText, Ui, Vec2};
use egui_icons::icons::{
    ICON_AUTO_FIX_HIGH, ICON_CLOSE_FULLSCREEN, ICON_COMMIT, ICON_EQUAL, ICON_FLIP, ICON_HEIGHT,
    ICON_HORIZONTAL_RULE, ICON_LINEAR_SCALE, ICON_LINE_CURVE, ICON_LOCK, ICON_PAUSE,
    ICON_RADIO_BUTTON_CHECKED, ICON_SQUARE_FOOT, ICON_STRAIGHTEN, ICON_TRIP_ORIGIN,
    ICON_VERTICAL_ALIGN_CENTER,
};
use crate::theme::{ACCENT_BLUE, TEXT_PRIMARY, TEXT_SECONDARY};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintAction {
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
    ApplyDistance,
    /// Titik di tengah garis (Paritas 0.5).
    ApplyMidpoint,
    /// Pusat dua lingkaran/busur berimpit.
    ApplyConcentric,
    /// Dua garis segaris.
    ApplyCollinear,
    /// Titik menempel di kurva.
    ApplyPointOnCurve,
    /// Inferensi constraint otomatis untuk seleksi / seluruh sketsa (Paritas 0.14).
    AutoConstrain,
}

impl ConstraintAction {
    /// Urutan tampil tetap di baris, apa pun urutan daftar `valid`.
    pub const DISPLAY_ORDER: [ConstraintAction; 16] = [
        Self::ApplyHorizontal,
        Self::ApplyVertical,
        Self::ApplyParallel,
        Self::ApplyPerpendicular,
        Self::ApplyCollinear,
        Self::ApplyEqualLength,
        Self::ApplyEqualRadius,
        Self::ApplyConcentric,
        Self::ApplyTangent,
        Self::ApplyCoincident,
        Self::ApplyMidpoint,
        Self::ApplyPointOnCurve,
        Self::ApplySymmetric,
        Self::ApplyDistance,
        Self::ApplyFixed,
        Self::AutoConstrain,
    ];

    /// Ikon Material (bukan glyph Unicode mentah: lihat aturan "tofu").
    pub fn icon(self) -> &'static str {
        match self {
            Self::ApplyHorizontal => ICON_HORIZONTAL_RULE.codepoint,
            Self::ApplyVertical => ICON_HEIGHT.codepoint,
            Self::ApplyParallel => ICON_PAUSE.codepoint,
            Self::ApplyPerpendicular => ICON_SQUARE_FOOT.codepoint,
            Self::ApplyCollinear => ICON_LINEAR_SCALE.codepoint,
            Self::ApplyEqualLength => ICON_EQUAL.codepoint,
            Self::ApplyEqualRadius => ICON_RADIO_BUTTON_CHECKED.codepoint,
            Self::ApplyConcentric => ICON_TRIP_ORIGIN.codepoint,
            Self::ApplyTangent => ICON_LINE_CURVE.codepoint,
            Self::ApplyCoincident => ICON_CLOSE_FULLSCREEN.codepoint,
            Self::ApplyMidpoint => ICON_VERTICAL_ALIGN_CENTER.codepoint,
            Self::ApplyPointOnCurve => ICON_COMMIT.codepoint,
            Self::ApplySymmetric => ICON_FLIP.codepoint,
            Self::ApplyDistance => ICON_STRAIGHTEN.codepoint,
            Self::ApplyFixed => ICON_LOCK.codepoint,
            Self::AutoConstrain => ICON_AUTO_FIX_HIGH.codepoint,
        }
    }

    pub fn tooltip(self) -> String {
        match self {
            Self::ApplyHorizontal => t!("constraint-horizontal-tip"),
            Self::ApplyVertical => t!("constraint-vertical-tip"),
            Self::ApplyParallel => t!("constraint-parallel-tip"),
            Self::ApplyPerpendicular => t!("constraint-perpendicular-tip"),
            Self::ApplyCollinear => t!("constraint-collinear-tip"),
            Self::ApplyEqualLength => t!("constraint-equal-length-tip"),
            Self::ApplyEqualRadius => t!("constraint-equal-radius-tip"),
            Self::ApplyConcentric => t!("constraint-concentric-tip"),
            Self::ApplyTangent => t!("constraint-tangent-tip"),
            Self::ApplyCoincident => t!("constraint-coincident-tip"),
            Self::ApplyMidpoint => t!("constraint-midpoint-tip"),
            Self::ApplyPointOnCurve => t!("constraint-point-on-curve-tip"),
            Self::ApplySymmetric => t!("constraint-symmetric-tip"),
            Self::ApplyDistance => t!("constraint-distance-tip"),
            Self::ApplyFixed => t!("constraint-fixed-tip"),
            Self::AutoConstrain => t!("constraint-auto-tip"),
        }
    }
}

const MIN_BTN: f32 = 26.0;

/// Baris ikon constraint: hanya `valid` yang digambar, dalam `DISPLAY_ORDER`.
/// Kosong → tidak menggambar apa pun. Mengembalikan aksi yang diklik.
pub struct ConstraintStrip;

impl ConstraintStrip {
    pub fn show_row(ui: &mut Ui, valid: &[ConstraintAction], icon_size: f32) -> Option<ConstraintAction> {
        if valid.is_empty() {
            return None;
        }
        let icon_sz = icon_size.clamp(12.0, 18.0);
        let side = (icon_sz + 12.0).max(MIN_BTN);
        let mut action = None;

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
            ui.label(RichText::new(t!("context-constraints-label")).size(11.0).color(TEXT_SECONDARY));
            ui.add_space(4.0);
            for act in ConstraintAction::DISPLAY_ORDER {
                if !valid.contains(&act) {
                    continue;
                }
                let color = if act == ConstraintAction::AutoConstrain { ACCENT_BLUE } else { TEXT_PRIMARY };
                let btn = ui.add_sized(
                    Vec2::new(side, side),
                    Button::new(RichText::new(act.icon()).size(icon_sz).color(color)).frame(false),
                );
                if btn.on_hover_text(act.tooltip()).clicked() {
                    action = Some(act);
                }
            }
        });

        action
    }
}
