//! Panel "Simulasi" (P17): daftar studi statik, penyusun studi baru
//! (tumpuan + beban dari face yang sedang dipilih di viewport), tombol
//! Jalankan/Batal, ringkasan hasil, legenda warna, dan skala deformasi.
//!
//! Murni tampilan: `ducad-app` membangun [`SimPanelData`], menjalankan
//! solver di thread latar, dan menggambar overlay hasil di viewport.

use crate::theme::{
    glass_frame, ACCENT_BLUE, ACCENT_GREEN, ACCENT_ORANGE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};
use ducad_i18n::t;
use egui::{Color32, ComboBox, DragValue, Grid, RichText, ScrollArea, Slider, Ui, Vec2};
use egui_icons::icons::{ICON_ADD, ICON_CLOSE, ICON_DELETE, ICON_PLAY_ARROW, ICON_STOP};

/// Besaran yang diwarnai di viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SimOverlayUi {
    #[default]
    Stress,
    Displacement,
    SafetyFactor,
}

impl SimOverlayUi {
    pub fn label(self) -> String {
        match self {
            SimOverlayUi::Stress => t!("sim-overlay-stress"),
            SimOverlayUi::Displacement => t!("sim-overlay-displacement"),
            SimOverlayUi::SafetyFactor => t!("sim-overlay-safety"),
        }
    }
    pub fn unit(self) -> &'static str {
        match self {
            SimOverlayUi::Stress => "MPa",
            SimOverlayUi::Displacement => "mm",
            SimOverlayUi::SafetyFactor => "",
        }
    }
}

/// Status satu studi di daftar.
#[derive(Debug, Clone, PartialEq)]
pub enum SimRunStatus {
    NotRun,
    Running,
    /// Ada hasil, tetapi model/setup berubah sejak dihitung.
    Stale,
    Done,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimStudyRow {
    pub id: String,
    pub body: String,
    pub fixtures: usize,
    pub loads: usize,
    pub status: SimRunStatus,
}

/// Ringkasan hasil studi terpilih.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SimResultUi {
    pub max_von_mises_mpa: f64,
    pub max_displacement_mm: f64,
    pub safety_factor: f64,
    pub reactions: Vec<(String, [f64; 3])>,
    pub elements: usize,
    pub nodes: usize,
    pub cell_mm: f64,
    pub iterations: usize,
    pub warnings: Vec<String>,
    /// Rentang nilai overlay aktif `(min, maks)`.
    pub legend: Option<(f64, f64)>,
    /// Hasil lama yang sedang dihitung ulang / sudah basi → digambar redup.
    pub stale: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SimPanelData {
    pub studies: Vec<SimStudyRow>,
    pub selected: Option<usize>,
    pub result: Option<SimResultUi>,
    /// Body sasaran studi baru (body terpilih / pertama).
    pub target_body: Option<String>,
    /// Body sasaran sudah punya material mekanik.
    pub target_has_material: bool,
    /// Selector face yang sedang dipilih di viewport, bila ada.
    pub picked_face: Option<String>,
}

/// Jenis tumpuan di penyusun studi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FixtureKindUi {
    #[default]
    Fixed,
    Roller,
    Symmetry,
}

impl FixtureKindUi {
    pub fn key(self) -> &'static str {
        match self {
            FixtureKindUi::Fixed => "fixed",
            FixtureKindUi::Roller => "roller",
            FixtureKindUi::Symmetry => "symmetry",
        }
    }
    fn label(self) -> String {
        match self {
            FixtureKindUi::Fixed => t!("sim-fixture-fixed"),
            FixtureKindUi::Roller => t!("sim-fixture-roller"),
            FixtureKindUi::Symmetry => t!("sim-fixture-symmetry"),
        }
    }
}

/// Jenis beban di penyusun studi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoadKindUi {
    #[default]
    Force,
    Pressure,
    Gravity,
}

impl LoadKindUi {
    fn label(self) -> String {
        match self {
            LoadKindUi::Force => t!("sim-load-force"),
            LoadKindUi::Pressure => t!("sim-load-pressure"),
            LoadKindUi::Gravity => t!("sim-load-gravity"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FixtureDraft {
    pub faces: String,
    pub kind: FixtureKindUi,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadDraft {
    /// Selector face; diabaikan untuk gravitasi.
    pub faces: String,
    pub kind: LoadKindUi,
    /// Gaya (N) atau arah gravitasi.
    pub vector: [f64; 3],
    /// Tekanan (MPa) atau percepatan gravitasi (m/s²).
    pub scalar: f64,
}

/// Studi baru yang sedang disusun.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StudyDraft {
    pub id: String,
    pub fixtures: Vec<FixtureDraft>,
    pub loads: Vec<LoadDraft>,
    /// Ukuran sel mesh (mm); 0 = otomatis.
    pub cell_mm: f64,
}

impl StudyDraft {
    /// Id sah (`^[a-z][a-z0-9_]{0,31}$`) + minimal satu tumpuan dan beban.
    pub fn is_complete(&self) -> bool {
        let mut chars = self.id.chars();
        let id_ok = chars.next().is_some_and(|c| c.is_ascii_lowercase())
            && self.id.len() <= 32
            && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        id_ok && !self.fixtures.is_empty() && !self.loads.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SimPanelEvent {
    Close,
    Select(usize),
    Run(usize),
    Cancel,
    Delete(usize),
    Create(StudyDraft),
}

const FAIL_RED: Color32 = Color32::from_rgb(255, 69, 58);
const PANEL_W: f32 = crate::theme::BOTTOM_RIGHT_PANEL_WIDTH + 110.0;

/// Keadaan panel.
#[derive(Debug, Clone)]
pub struct SimPanel {
    pub overlay: SimOverlayUi,
    pub show_overlay: bool,
    /// Skala deformasi otomatis (deformasi maksimum tampak ±5 % ukuran model).
    pub auto_deform: bool,
    pub deform_scale: f32,
    draft: Option<StudyDraft>,
}

impl Default for SimPanel {
    fn default() -> Self {
        Self {
            overlay: SimOverlayUi::Stress,
            show_overlay: true,
            auto_deform: true,
            deform_scale: 1.0,
            draft: None,
        }
    }
}

impl SimPanel {
    pub fn show(&mut self, ui: &mut Ui, data: &SimPanelData) -> Option<SimPanelEvent> {
        let mut event = None;
        glass_frame().show(ui, |ui| {
            ui.set_width(PANEL_W);
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(t!("sim-title")).strong().color(TEXT_PRIMARY));
                ui.label(
                    RichText::new(t!("sim-accuracy"))
                        .small()
                        .color(ACCENT_ORANGE),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button(ICON_CLOSE.codepoint)
                        .on_hover_text(t!("sim-close"))
                        .clicked()
                    {
                        event = Some(SimPanelEvent::Close);
                    }
                    if ui
                        .small_button(ICON_ADD.codepoint)
                        .on_hover_text(t!("sim-new"))
                        .clicked()
                        && self.draft.is_none()
                    {
                        self.draft = Some(StudyDraft {
                            id: format!("study{}", data.studies.len() + 1),
                            ..StudyDraft::default()
                        });
                    }
                });
            });
            ui.separator();
            ScrollArea::vertical().max_height(520.0).show(ui, |ui| {
                if let Some(ev) = self.study_list(ui, data) {
                    event = Some(ev);
                }
                if let Some(ev) = self.draft_editor(ui, data) {
                    event = Some(ev);
                }
                if let Some(result) = &data.result {
                    ui.separator();
                    self.result_view(ui, result);
                }
            });
        });
        event
    }

    fn study_list(&mut self, ui: &mut Ui, data: &SimPanelData) -> Option<SimPanelEvent> {
        let mut event = None;
        if data.studies.is_empty() && self.draft.is_none() {
            ui.label(RichText::new(t!("sim-empty")).color(TEXT_MUTED));
        }
        let running = data
            .studies
            .iter()
            .any(|s| s.status == SimRunStatus::Running);
        for (i, study) in data.studies.iter().enumerate() {
            let (status, color) = match &study.status {
                SimRunStatus::NotRun => (t!("sim-status-not-run"), TEXT_MUTED),
                SimRunStatus::Running => (t!("sim-status-running"), ACCENT_BLUE),
                SimRunStatus::Stale => (t!("sim-status-stale"), ACCENT_ORANGE),
                SimRunStatus::Done => (t!("sim-status-done"), ACCENT_GREEN),
                SimRunStatus::Failed(_) => (t!("sim-status-failed"), FAIL_RED),
            };
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(data.selected == Some(i), &study.id)
                    .clicked()
                {
                    event = Some(SimPanelEvent::Select(i));
                }
                ui.label(RichText::new(status).small().color(color));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(!running, egui::Button::new(ICON_DELETE.codepoint).small())
                        .on_hover_text(t!("sim-delete"))
                        .clicked()
                    {
                        event = Some(SimPanelEvent::Delete(i));
                    }
                    if study.status == SimRunStatus::Running {
                        if ui
                            .small_button(ICON_STOP.codepoint)
                            .on_hover_text(t!("sim-cancel"))
                            .clicked()
                        {
                            event = Some(SimPanelEvent::Cancel);
                        }
                        ui.spinner();
                    } else if ui
                        .add_enabled(
                            !running,
                            egui::Button::new(ICON_PLAY_ARROW.codepoint).small(),
                        )
                        .on_hover_text(t!("sim-run"))
                        .clicked()
                    {
                        event = Some(SimPanelEvent::Run(i));
                    }
                });
            });
            let (n_fixtures, n_loads) = (study.fixtures.to_string(), study.loads.to_string());
            let summary = t!(
                "sim-study-summary",
                body = study.body.as_str(),
                fixtures = n_fixtures.as_str(),
                loads = n_loads.as_str()
            );
            ui.label(RichText::new(summary).small().color(TEXT_SECONDARY));
            if let SimRunStatus::Failed(why) = &study.status {
                ui.label(RichText::new(why).small().color(FAIL_RED));
            }
        }
        event
    }

    fn draft_editor(&mut self, ui: &mut Ui, data: &SimPanelData) -> Option<SimPanelEvent> {
        let draft = self.draft.as_mut()?;
        let mut event = None;
        let mut close = false;
        ui.separator();
        ui.label(RichText::new(t!("sim-new")).strong().color(TEXT_PRIMARY));
        match &data.target_body {
            Some(body) => {
                ui.label(
                    RichText::new(t!("sim-target-body", body = body.as_str()))
                        .small()
                        .color(TEXT_SECONDARY),
                );
                if !data.target_has_material {
                    ui.label(
                        RichText::new(t!("sim-no-material"))
                            .small()
                            .color(ACCENT_ORANGE),
                    );
                }
            }
            None => {
                ui.label(
                    RichText::new(t!("sim-no-body"))
                        .small()
                        .color(ACCENT_ORANGE),
                );
            }
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new(t!("sim-study-id")).color(TEXT_SECONDARY));
            ui.text_edit_singleline(&mut draft.id);
        });
        let picked = data.picked_face.clone();
        let picked_hint = match &picked {
            Some(sel) => t!("sim-picked-face", selector = sel.as_str()),
            None => t!("sim-pick-hint"),
        };
        ui.label(RichText::new(picked_hint).small().color(TEXT_MUTED));

        ui.horizontal(|ui| {
            ui.label(RichText::new(t!("sim-fixtures")).color(TEXT_SECONDARY));
            if ui
                .add_enabled(
                    picked.is_some(),
                    egui::Button::new(ICON_ADD.codepoint).small(),
                )
                .on_hover_text(t!("sim-add-fixture"))
                .clicked()
            {
                draft.fixtures.push(FixtureDraft {
                    faces: picked.clone().unwrap_or_default(),
                    kind: FixtureKindUi::Fixed,
                });
            }
        });
        let mut drop_fixture = None;
        for (i, f) in draft.fixtures.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&f.faces).monospace().color(TEXT_PRIMARY));
                ComboBox::from_id_salt(("ducad-sim-fixture", i))
                    .selected_text(f.kind.label())
                    .show_ui(ui, |ui| crate::theme::glass_menu(ui, |ui| {
                        for kind in [
                            FixtureKindUi::Fixed,
                            FixtureKindUi::Roller,
                            FixtureKindUi::Symmetry,
                        ] {
                            ui.selectable_value(&mut f.kind, kind, kind.label());
                        }
                    }));
                if ui.small_button(ICON_DELETE.codepoint).clicked() {
                    drop_fixture = Some(i);
                }
            });
        }
        if let Some(i) = drop_fixture {
            draft.fixtures.remove(i);
        }

        ui.horizontal(|ui| {
            ui.label(RichText::new(t!("sim-loads")).color(TEXT_SECONDARY));
            if ui
                .add_enabled(
                    picked.is_some(),
                    egui::Button::new(ICON_ADD.codepoint).small(),
                )
                .on_hover_text(t!("sim-add-load"))
                .clicked()
            {
                draft.loads.push(LoadDraft {
                    faces: picked.clone().unwrap_or_default(),
                    kind: LoadKindUi::Force,
                    vector: [0.0, 0.0, -100.0],
                    scalar: 1.0,
                });
            }
            if ui.small_button(t!("sim-load-gravity")).clicked() {
                draft.loads.push(LoadDraft {
                    faces: String::new(),
                    kind: LoadKindUi::Gravity,
                    vector: [0.0, 0.0, -1.0],
                    scalar: 9.80665,
                });
            }
        });
        let mut drop_load = None;
        for (i, l) in draft.loads.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                if l.kind != LoadKindUi::Gravity {
                    ui.label(RichText::new(&l.faces).monospace().color(TEXT_PRIMARY));
                    ComboBox::from_id_salt(("ducad-sim-load", i))
                        .selected_text(l.kind.label())
                        .show_ui(ui, |ui| crate::theme::glass_menu(ui, |ui| {
                            for kind in [LoadKindUi::Force, LoadKindUi::Pressure] {
                                ui.selectable_value(&mut l.kind, kind, kind.label());
                            }
                        }));
                } else {
                    ui.label(RichText::new(l.kind.label()).color(TEXT_PRIMARY));
                }
                if ui.small_button(ICON_DELETE.codepoint).clicked() {
                    drop_load = Some(i);
                }
            });
            ui.horizontal(|ui| match l.kind {
                LoadKindUi::Force => {
                    for v in l.vector.iter_mut() {
                        ui.add(DragValue::new(v).speed(1.0).suffix(" N"));
                    }
                }
                LoadKindUi::Pressure => {
                    ui.add(DragValue::new(&mut l.scalar).speed(0.01).suffix(" MPa"));
                }
                LoadKindUi::Gravity => {
                    ui.add(DragValue::new(&mut l.scalar).speed(0.01).suffix(" m/s^2"));
                    for v in l.vector.iter_mut() {
                        ui.add(DragValue::new(v).speed(0.01));
                    }
                }
            });
        }
        if let Some(i) = drop_load {
            draft.loads.remove(i);
        }

        ui.horizontal(|ui| {
            ui.label(RichText::new(t!("sim-cell")).color(TEXT_SECONDARY));
            ui.add(
                DragValue::new(&mut draft.cell_mm)
                    .speed(0.05)
                    .range(0.0..=1000.0)
                    .suffix(" mm"),
            )
            .on_hover_text(t!("sim-cell-auto"));
        });
        ui.horizontal(|ui| {
            let ready = draft.is_complete() && data.target_body.is_some();
            if ui
                .add_enabled(ready, egui::Button::new(t!("sim-create")))
                .clicked()
            {
                event = Some(SimPanelEvent::Create(draft.clone()));
                close = true;
            }
            if ui.button(t!("sim-discard")).clicked() {
                close = true;
            }
        });
        if close {
            self.draft = None;
        }
        event
    }

    fn result_view(&mut self, ui: &mut Ui, r: &SimResultUi) {
        let dim = |c: Color32| if r.stale { c.gamma_multiply(0.45) } else { c };
        Grid::new("ducad-sim-result").num_columns(2).show(ui, |ui| {
            let mut row = |label: String, value: String| {
                ui.label(RichText::new(label).color(dim(TEXT_SECONDARY)));
                ui.label(RichText::new(value).monospace().color(dim(TEXT_PRIMARY)));
                ui.end_row();
            };
            row(
                t!("sim-max-stress"),
                format!("{:.3} MPa", r.max_von_mises_mpa),
            );
            row(
                t!("sim-max-displacement"),
                format!("{:.5} mm", r.max_displacement_mm),
            );
            row(t!("sim-safety-factor"), format!("{:.2}", r.safety_factor));
            for (id, f) in &r.reactions {
                row(
                    format!("{} {id}", t!("sim-reaction")),
                    format!("{:.2}, {:.2}, {:.2} N", f[0], f[1], f[2]),
                );
            }
            row(
                t!("sim-mesh"),
                format!("{} / {} / {:.3} mm", r.elements, r.nodes, r.cell_mm),
            );
            row(t!("sim-iterations"), r.iterations.to_string());
        });
        for w in &r.warnings {
            ui.label(RichText::new(w).small().color(ACCENT_ORANGE));
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.show_overlay, t!("sim-show-overlay"));
            ComboBox::from_id_salt("ducad-sim-overlay")
                .selected_text(self.overlay.label())
                .show_ui(ui, |ui| crate::theme::glass_menu(ui, |ui| {
                    for overlay in [
                        SimOverlayUi::Stress,
                        SimOverlayUi::Displacement,
                        SimOverlayUi::SafetyFactor,
                    ] {
                        ui.selectable_value(&mut self.overlay, overlay, overlay.label());
                    }
                }));
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.auto_deform, t!("sim-deform-auto"));
            ui.add_enabled(
                !self.auto_deform,
                Slider::new(&mut self.deform_scale, 0.0..=1000.0)
                    .logarithmic(true)
                    .text(t!("sim-deform-scale")),
            );
        });
        if let Some((lo, hi)) = r.legend {
            legend_bar(ui, lo, hi, self.overlay);
        }
    }
}

/// Colormap "turbo" (pendekatan polinomial), `t` di 0..1.
pub fn turbo_color(t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let r = 0.135_721_38
        + t * (4.615_392_6
            + t * (-42.660_32 + t * (132.131_08 + t * (-152.942_4 + t * 59.286_38))));
    let g = 0.091_402_61
        + t * (2.194_188_4
            + t * (4.842_966_6 + t * (-14.185_033 + t * (4.277_298_6 + t * 2.829_566))));
    let b = 0.106_673_3
        + t * (12.641_946 + t * (-60.582_05 + t * (110.362_77 + t * (-89.903_11 + t * 27.348_25))));
    let to8 = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgb(to8(r), to8(g), to8(b))
}

fn legend_bar(ui: &mut Ui, lo: f64, hi: f64, overlay: SimOverlayUi) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(PANEL_W - 16.0, 12.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let bands = 48;
    for i in 0..bands {
        let t = (i as f32 + 0.5) / bands as f32;
        // Faktor keamanan: merah = rendah.
        let color = turbo_color(if overlay == SimOverlayUi::SafetyFactor {
            1.0 - t
        } else {
            t
        });
        let x0 = rect.min.x + rect.width() * i as f32 / bands as f32;
        let x1 = rect.min.x + rect.width() * (i + 1) as f32 / bands as f32;
        painter.rect_filled(
            egui::Rect::from_min_max(egui::pos2(x0, rect.min.y), egui::pos2(x1 + 0.5, rect.max.y)),
            0.0,
            color,
        );
    }
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{lo:.3}"))
                .small()
                .color(TEXT_SECONDARY),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(format!("{hi:.3} {}", overlay.unit()))
                    .small()
                    .color(TEXT_SECONDARY),
            );
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sim_draft_needs_valid_id_fixture_and_load() {
        let mut d = StudyDraft {
            id: "tip_load".into(),
            ..StudyDraft::default()
        };
        assert!(!d.is_complete());
        d.fixtures.push(FixtureDraft {
            faces: "<Y".into(),
            kind: FixtureKindUi::Fixed,
        });
        d.loads.push(LoadDraft {
            faces: ">Y".into(),
            kind: LoadKindUi::Force,
            vector: [0.0, 0.0, -1.0],
            scalar: 0.0,
        });
        assert!(d.is_complete());
        for bad in ["", "Tip", "1a", "a-b", &"x".repeat(33)] {
            d.id = bad.to_string();
            assert!(!d.is_complete(), "{bad:?}");
        }
    }

    #[test]
    fn sim_turbo_runs_blue_to_red() {
        let (lo, hi) = (turbo_color(0.12), turbo_color(0.9));
        assert!(lo.b() > lo.r(), "{lo:?}");
        assert!(hi.r() > hi.b(), "{hi:?}");
    }

    #[test]
    fn sim_panel_renders_all_states() {
        let ctx = egui::Context::default();
        let mut panel = SimPanel::default();
        let data = SimPanelData {
            studies: vec![
                SimStudyRow {
                    id: "a".into(),
                    body: "bracket".into(),
                    fixtures: 1,
                    loads: 1,
                    status: SimRunStatus::Done,
                },
                SimStudyRow {
                    id: "b".into(),
                    body: "bracket".into(),
                    fixtures: 1,
                    loads: 2,
                    status: SimRunStatus::Failed("sim_underconstrained".into()),
                },
            ],
            selected: Some(0),
            result: Some(SimResultUi {
                max_von_mises_mpa: 24.0,
                reactions: vec![("wall".into(), [0.0, 0.0, 200.0])],
                legend: Some((0.0, 24.0)),
                ..SimResultUi::default()
            }),
            target_body: Some("bracket".into()),
            target_has_material: false,
            picked_face: Some(">Z".into()),
        };
        panel.draft = Some(StudyDraft::default());
        let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert_eq!(panel.show(ui, &data), None);
                assert_eq!(panel.show(ui, &SimPanelData::default()), None);
            });
        });
        output.textures_delta.clear();
    }
}
