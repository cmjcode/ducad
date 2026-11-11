//! Widget input angka tunggal sadar-satuan, ekspresi aritmetika, koma desimal,
//! dan parameter desain (`$nama`).

use std::collections::BTreeMap;

use crate::theme::{ThemeTokens, TYPOGRAPHY};
use ducad_core::LengthUnit;
use ducad_i18n::t;
use egui::{RichText, Stroke, Ui, Vec2};

/// Render kotak input angka satu baris disertai lencana satuan aktif (`mm`/`cm`/`m`/`in`),
/// validasi ekspresi langsung, sorotan merah saat tidak sah, serta informasi pembaca layar.
pub fn unit_number_field(
    ui: &mut Ui,
    text: &mut String,
    unit: LengthUnit,
    width: f32,
) -> egui::Response {
    let empty_params = BTreeMap::new();
    unit_number_field_with_params(ui, text, unit, &empty_params, width, None)
}

/// Varian lengkap [`unit_number_field`] dengan tabel parameter `$nama` dan label aksesibilitas.
pub fn unit_number_field_with_params(
    ui: &mut Ui,
    text: &mut String,
    unit: LengthUnit,
    params: &BTreeMap<String, f64>,
    width: f32,
    aria_label: Option<&str>,
) -> egui::Response {
    let tok = ThemeTokens::from_ui(ui);
    let trimmed = text.trim();
    let validation_err = if trimmed.is_empty() {
        None
    } else {
        unit.parse_input_mm_with_params(trimmed, params).err()
    };

    let mut edit_resp = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let resp = ui.scope(|ui| {
                if validation_err.is_some() {
                    let v = ui.visuals_mut();
                    v.widgets.inactive.bg_stroke = Stroke::new(1.2, tok.accent_danger);
                    v.widgets.hovered.bg_stroke = Stroke::new(1.5, tok.accent_danger);
                    v.widgets.active.bg_stroke = Stroke::new(1.5, tok.accent_danger);
                }
                ui.add_sized(
                    Vec2::new(width, ui.spacing().interact_size.y.max(24.0)),
                    egui::TextEdit::singleline(text),
                )
            }).inner;
            ui.label(
                RichText::new(unit.suffix())
                    .size(TYPOGRAPHY.caption)
                    .color(tok.text_secondary),
            );
            resp
        })
        .inner;

    let a11y_label = aria_label
        .map(|s| format!("{s} ({})", unit.suffix()))
        .unwrap_or_else(|| format!("Length ({})", unit.suffix()));
    let current_val = text.clone();
    edit_resp.widget_info(move || {
        egui::WidgetInfo::text_edit(true, current_val.clone(), current_val.clone(), a11y_label.clone())
    });

    if let Some(err) = validation_err {
        edit_resp = edit_resp.on_hover_text(t!("unit-input-invalid", error = err.as_str()));
    }
    edit_resp
}

/// Label satuan panjang terlokalisasi untuk menu Pengaturan & dialog.
pub fn localized_unit_label(unit: LengthUnit) -> String {
    match unit {
        LengthUnit::Millimeters => t!("unit-mm-label"),
        LengthUnit::Centimeters => t!("unit-cm-label"),
        LengthUnit::Meters => t!("unit-m-label"),
        LengthUnit::Inches => t!("unit-in-label"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{apply, ThemeMode};

    #[test]
    fn unit_number_field_renders_active_unit_and_validates_params() {
        let ctx = egui::Context::default();
        apply(&ctx, ThemeMode::Light);
        let mut params = BTreeMap::new();
        params.insert("w".to_string(), 25.4);
        let mut valid_buf = "$w / 2 + 0,5 in".to_string();
        let mut invalid_buf = "12 abc".to_string();
        let mut out = ctx.run_ui(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = unit_number_field_with_params(
                    ui,
                    &mut valid_buf,
                    LengthUnit::Inches,
                    &params,
                    80.0,
                    Some("Width"),
                );
                let _ = unit_number_field(ui, &mut invalid_buf, LengthUnit::Centimeters, 80.0);
            });
        });
        out.textures_delta.clear();
    }
}

