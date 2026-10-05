//! Panel "Properti Massa" (P16): massa, volume, luas, pusat massa, tensor
//! inersia, momen/sumbu utama, radius girasi, plus pemilih material mekanik.
//!
//! Panel ini murni tampilan: caller (`ducad-app`) menghitung
//! [`MassPanelData`] dari `ducad_engine::inspect` dan menangani event.

use crate::theme::{glass_frame, ACCENT_BLUE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY};
use ducad_core::MechanicalProperties;
use ducad_i18n::t;
use egui::{ComboBox, DragValue, Grid, RichText, ScrollArea, Ui, Vec2};
use egui_icons::icons::{ICON_CLOSE, ICON_CONTENT_COPY, ICON_MY_LOCATION};

/// Satuan massa tampilan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MassUnit {
    #[default]
    Gram,
    Kilogram,
}

impl MassUnit {
    pub fn suffix(self) -> &'static str {
        match self {
            MassUnit::Gram => "g",
            MassUnit::Kilogram => "kg",
        }
    }
    /// Faktor dari gram.
    fn factor(self) -> f64 {
        match self {
            MassUnit::Gram => 1.0,
            MassUnit::Kilogram => 1e-3,
        }
    }
}

/// Satuan panjang tampilan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MassLengthUnit {
    #[default]
    Mm,
    Cm,
    M,
}

impl MassLengthUnit {
    pub fn suffix(self) -> &'static str {
        match self {
            MassLengthUnit::Mm => "mm",
            MassLengthUnit::Cm => "cm",
            MassLengthUnit::M => "m",
        }
    }
    /// Faktor dari milimeter.
    fn factor(self) -> f64 {
        match self {
            MassLengthUnit::Mm => 1.0,
            MassLengthUnit::Cm => 0.1,
            MassLengthUnit::M => 1e-3,
        }
    }
}

/// Angka yang ditampilkan panel; semuanya dalam g / mm (satuan engine).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MassPanelData {
    /// `None` = tidak ada body.
    pub body_name: Option<String>,
    /// Kunci pustaka material mekanik aktif (`"custom"` untuk nilai kustom).
    pub material_key: Option<String>,
    /// Sifat mekanik aktif, bila lengkap (isi awal editor kustom).
    pub mechanical: Option<MechanicalProperties>,
    pub density_g_cm3: Option<f64>,
    pub mass_g: Option<f64>,
    pub volume_mm3: f64,
    pub area_mm2: f64,
    pub center_of_mass: Option<[f64; 3]>,
    pub inertia_com: Option<[[f64; 3]; 3]>,
    pub inertia_origin: Option<[[f64; 3]; 3]>,
    pub principal_moments: Option<[f64; 3]>,
    pub principal_axes: Option<[[f64; 3]; 3]>,
    pub radius_of_gyration: Option<[f64; 3]>,
    /// Massa total dan pusat massa gabungan semua body (≥ 2 body).
    pub assembly: Option<(f64, [f64; 3])>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MassPanelEvent {
    Close,
    /// Pilih material dari pustaka (kunci).
    SetLibraryMaterial(String),
    /// Terapkan nilai kustom.
    SetCustomMaterial(MechanicalProperties),
    /// Tampilkan/sembunyikan penanda pusat massa di viewport.
    ToggleMarker,
}

fn num(v: f64) -> String {
    let a = v.abs();
    if a != 0.0 && !(1e-3..1e7).contains(&a) {
        format!("{v:.4e}")
    } else {
        format!("{v:.4}")
    }
}

fn vec3(v: [f64; 3], k: f64) -> String {
    format!("{}, {}, {}", num(v[0] * k), num(v[1] * k), num(v[2] * k))
}

/// Baris tabel `(label, nilai + satuan)` — dipakai tampilan dan tombol Salin.
pub fn mass_table_rows(
    d: &MassPanelData,
    mass: MassUnit,
    len: MassLengthUnit,
) -> Vec<(String, String)> {
    let (mu, lu) = (mass.suffix(), len.suffix());
    let (mk, lk) = (mass.factor(), len.factor());
    let ik = mk * lk * lk;
    let mut rows = Vec::new();
    let mut push = |label: String, value: String| rows.push((label, value));
    if let Some(rho) = d.density_g_cm3 {
        push(t!("mass-density"), format!("{} g/cm^3", num(rho)));
    }
    match d.mass_g {
        Some(m) => push(t!("mass-mass"), format!("{} {mu}", num(m * mk))),
        None => push(t!("mass-mass"), t!("mass-unknown-density")),
    }
    push(
        t!("mass-volume"),
        format!("{} {lu}^3", num(d.volume_mm3 * lk.powi(3))),
    );
    push(
        t!("mass-area"),
        format!("{} {lu}^2", num(d.area_mm2 * lk * lk)),
    );
    if let Some(c) = d.center_of_mass {
        push(t!("mass-com"), format!("{} {lu}", vec3(c, lk)));
    }
    for (label, tensor) in [
        (t!("mass-inertia-com"), d.inertia_com),
        (t!("mass-inertia-origin"), d.inertia_origin),
    ] {
        if let Some(i) = tensor {
            for (row, axis) in i.iter().zip(["x", "y", "z"]) {
                push(
                    format!("{label} {axis}"),
                    format!("{} {mu}*{lu}^2", vec3(*row, ik)),
                );
            }
        }
    }
    if let Some(m) = d.principal_moments {
        push(
            t!("mass-principal-moments"),
            format!("{} {mu}*{lu}^2", vec3(m, ik)),
        );
    }
    if let Some(axes) = d.principal_axes {
        for (k, axis) in axes.iter().enumerate() {
            push(
                format!("{} {}", t!("mass-principal-axis"), k + 1),
                vec3(*axis, 1.0),
            );
        }
    }
    if let Some(r) = d.radius_of_gyration {
        push(t!("mass-gyration"), format!("{} {lu}", vec3(r, lk)));
    }
    if let Some((m, c)) = d.assembly {
        push(t!("mass-assembly-mass"), format!("{} {mu}", num(m * mk)));
        push(t!("mass-assembly-com"), format!("{} {lu}", vec3(c, lk)));
    }
    rows
}

/// Teks tab-separated untuk papan klip.
pub fn mass_table_text(d: &MassPanelData, mass: MassUnit, len: MassLengthUnit) -> String {
    let mut out = String::new();
    if let Some(name) = &d.body_name {
        out.push_str(name);
        out.push('\n');
    }
    for (label, value) in mass_table_rows(d, mass, len) {
        out.push_str(&label);
        out.push('\t');
        out.push_str(&value);
        out.push('\n');
    }
    out
}

const PANEL_W: f32 = crate::theme::BOTTOM_RIGHT_PANEL_WIDTH + 110.0;
const CUSTOM_KEY: &str = "custom";

/// Keadaan panel (satuan terpilih, editor material kustom).
#[derive(Debug, Clone, Default)]
pub struct MassPropertiesPanel {
    pub mass_unit: MassUnit,
    pub length_unit: MassLengthUnit,
    /// Penanda pusat massa di viewport menyala.
    pub show_marker: bool,
    /// Editor kustom terbuka dengan nilai kerja ini.
    custom_draft: Option<MechanicalProperties>,
}

impl MassPropertiesPanel {
    pub fn show(&mut self, ui: &mut Ui, data: &MassPanelData) -> Option<MassPanelEvent> {
        let mut event = None;
        glass_frame().show(ui, |ui| {
            ui.set_width(PANEL_W);
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(t!("mass-title")).strong().color(TEXT_PRIMARY));
                if let Some(name) = &data.body_name {
                    ui.label(RichText::new(name).color(TEXT_SECONDARY));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button(ICON_CLOSE.codepoint)
                        .on_hover_text(t!("mass-close"))
                        .clicked()
                    {
                        event = Some(MassPanelEvent::Close);
                    }
                    if ui
                        .small_button(ICON_CONTENT_COPY.codepoint)
                        .on_hover_text(t!("mass-copy"))
                        .clicked()
                    {
                        ui.ctx()
                            .copy_text(mass_table_text(data, self.mass_unit, self.length_unit));
                    }
                    let marker =
                        RichText::new(ICON_MY_LOCATION.codepoint).color(if self.show_marker {
                            ACCENT_BLUE
                        } else {
                            TEXT_SECONDARY
                        });
                    if ui
                        .small_button(marker)
                        .on_hover_text(t!("mass-marker"))
                        .clicked()
                    {
                        event = Some(MassPanelEvent::ToggleMarker);
                    }
                });
            });
            ui.separator();
            if data.body_name.is_none() {
                ui.label(RichText::new(t!("mass-empty")).color(TEXT_MUTED));
                return;
            }

            if let Some(ev) = self.material_picker(ui, data) {
                event = Some(ev);
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new(t!("mass-units")).color(TEXT_SECONDARY));
                for unit in [MassUnit::Gram, MassUnit::Kilogram] {
                    ui.selectable_value(&mut self.mass_unit, unit, unit.suffix());
                }
                ui.separator();
                for unit in [MassLengthUnit::Mm, MassLengthUnit::Cm, MassLengthUnit::M] {
                    ui.selectable_value(&mut self.length_unit, unit, unit.suffix());
                }
            });
            ui.separator();
            ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                Grid::new("ducad-mass-grid")
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| {
                        for (label, value) in
                            mass_table_rows(data, self.mass_unit, self.length_unit)
                        {
                            ui.label(RichText::new(label).color(TEXT_SECONDARY));
                            ui.label(RichText::new(value).monospace().color(TEXT_PRIMARY));
                            ui.end_row();
                        }
                    });
            });
        });
        event
    }

    fn material_picker(&mut self, ui: &mut Ui, data: &MassPanelData) -> Option<MassPanelEvent> {
        let mut event = None;
        let current = data.material_key.as_deref();
        let selected_text = match current {
            Some(CUSTOM_KEY) => t!("mass-material-custom"),
            Some(key) => ducad_core::library_material(key)
                .map(|m| m.name.to_string())
                .unwrap_or_else(|| key.to_string()),
            None => t!("mass-material-none"),
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(t!("mass-material")).color(TEXT_SECONDARY));
            ComboBox::from_id_salt("ducad-mass-material")
                .selected_text(selected_text)
                .width(PANEL_W - 110.0)
                .show_ui(ui, |ui| crate::theme::glass_menu(ui, |ui| {
                    for m in ducad_core::material_library() {
                        if ui
                            .selectable_label(current == Some(m.key), m.name)
                            .clicked()
                        {
                            self.custom_draft = None;
                            event = Some(MassPanelEvent::SetLibraryMaterial(m.key.to_string()));
                        }
                    }
                    if ui
                        .selectable_label(current == Some(CUSTOM_KEY), t!("mass-material-custom"))
                        .clicked()
                    {
                        self.custom_draft = Some(
                            data.mechanical
                                .unwrap_or(ducad_core::material_library()[0].props),
                        );
                    }
                }));
        });
        if let Some(draft) = &mut self.custom_draft {
            Grid::new("ducad-mass-custom")
                .num_columns(2)
                .show(ui, |ui| {
                    let field = |ui: &mut Ui, label: String, v: &mut f64, speed: f64| {
                        ui.label(RichText::new(label).color(TEXT_SECONDARY));
                        ui.add(DragValue::new(v).speed(speed).range(0.0..=f64::MAX));
                        ui.end_row();
                    };
                    field(ui, t!("mass-density"), &mut draft.density_g_cm3, 0.01);
                    field(ui, t!("mass-young"), &mut draft.young_modulus_gpa, 0.5);
                    field(ui, t!("mass-poisson"), &mut draft.poisson_ratio, 0.005);
                    field(ui, t!("mass-yield"), &mut draft.yield_strength_mpa, 1.0);
                    field(
                        ui,
                        t!("mass-ultimate"),
                        &mut draft.ultimate_strength_mpa,
                        1.0,
                    );
                });
            let verdict = draft.validate();
            let candidate = *draft;
            let mut close_editor = false;
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(verdict.is_ok(), egui::Button::new(t!("mass-apply")))
                    .clicked()
                {
                    event = Some(MassPanelEvent::SetCustomMaterial(candidate));
                    close_editor = true;
                }
                if ui.button(t!("mass-cancel")).clicked() {
                    close_editor = true;
                }
            });
            if let Err(why) = verdict {
                ui.label(
                    RichText::new(why)
                        .small()
                        .color(crate::theme::ACCENT_ORANGE),
                );
            }
            if close_editor {
                self.custom_draft = None;
            }
        }
        event
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> MassPanelData {
        MassPanelData {
            body_name: Some("bracket".into()),
            material_key: Some("al_6061_t6".into()),
            density_g_cm3: Some(2.7),
            mass_g: Some(2700.0),
            volume_mm3: 1_000_000.0,
            area_mm2: 60_000.0,
            center_of_mass: Some([50.0, 50.0, 50.0]),
            inertia_com: Some([[4.5e6, 0.0, 0.0], [0.0, 4.5e6, 0.0], [0.0, 0.0, 4.5e6]]),
            principal_moments: Some([4.5e6, 4.5e6, 4.5e6]),
            radius_of_gyration: Some([40.8248, 40.8248, 40.8248]),
            ..MassPanelData::default()
        }
    }

    #[test]
    fn unit_conversion_scales_every_quantity() {
        let d = sample();
        let text = mass_table_text(&d, MassUnit::Kilogram, MassLengthUnit::M);
        assert!(text.starts_with("bracket\n"));
        assert!(text.contains("2.7000 kg"), "{text}");
        // 1e6 mm³ = 1e-3 m³; 6e4 mm² = 0.06 m².
        assert!(text.contains("0.0010 m^3"), "{text}");
        assert!(text.contains("0.0600 m^2"), "{text}");
        assert!(text.contains("0.0500, 0.0500, 0.0500 m"), "{text}");
        // 4.5e6 g·mm² = 4.5e-3 kg·m².
        assert!(text.contains("0.0045, 0.0000, 0.0000 kg*m^2"), "{text}");

        let mm = mass_table_text(&d, MassUnit::Gram, MassLengthUnit::Mm);
        assert!(mm.contains("2700.0000 g"), "{mm}");
        assert!(mm.contains("1000000.0000 mm^3"), "{mm}");
    }

    #[test]
    fn unknown_density_is_reported_not_zero() {
        let d = MassPanelData {
            mass_g: None,
            density_g_cm3: None,
            ..sample()
        };
        let rows = mass_table_rows(&d, MassUnit::Gram, MassLengthUnit::Mm);
        let mass_row = rows.iter().find(|(l, _)| *l == t!("mass-mass")).unwrap();
        assert_eq!(mass_row.1, t!("mass-unknown-density"));
    }

    #[test]
    fn panel_renders_without_panic() {
        let ctx = egui::Context::default();
        let mut panel = MassPropertiesPanel::default();
        let data = sample();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert_eq!(panel.show(ui, &data), None);
                assert_eq!(panel.show(ui, &MassPanelData::default()), None);
            });
        });
        output.textures_delta.clear();
    }
}
