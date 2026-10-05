//! Panel "Fitur Industri" (P18–P20): studi lanjutan (frekuensi, buckling,
//! termal), konfigurasi varian, sheet metal, toleransi + GD&T, part standar
//! + ulir, dan kopling/langkah urai perakitan — satu jendela bertab.
//!
//! Murni tampilan: `ducad-app` membangun [`IndustryData`] dan menerjemahkan
//! [`IndustryEvent`] menjadi op/oplog (jalur yang sama dengan panel Simulasi).

use crate::theme::{
    glass_frame, ACCENT_BLUE, ACCENT_GREEN, ACCENT_ORANGE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};
use ducad_core::drawing_annot::{
    tolerance_stackup, Annotation, DatumFeature, DimensionTolerance, FeatureControlFrame,
    GdtSymbol, SurfaceFinish,
};
use ducad_core::{IsoFit, StandardKind};
use ducad_i18n::t;
use egui::{Color32, ComboBox, DragValue, RichText, ScrollArea, Slider, TextEdit, Ui, Vec2};
use egui_icons::icons::{ICON_ADD, ICON_CLOSE, ICON_DELETE, ICON_PLAY_ARROW, ICON_STOP};

const FAIL_RED: Color32 = Color32::from_rgb(255, 69, 58);
const PANEL_W: f32 = crate::theme::BOTTOM_RIGHT_PANEL_WIDTH + 170.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IndustryTab {
    #[default]
    Study,
    Config,
    Sheet,
    Tolerance,
    Parts,
    Assembly,
}

impl IndustryTab {
    const ALL: [IndustryTab; 6] = [
        IndustryTab::Study,
        IndustryTab::Config,
        IndustryTab::Sheet,
        IndustryTab::Tolerance,
        IndustryTab::Parts,
        IndustryTab::Assembly,
    ];
    fn label(self) -> String {
        match self {
            IndustryTab::Study => t!("ind-tab-study"),
            IndustryTab::Config => t!("ind-tab-config"),
            IndustryTab::Sheet => t!("ind-tab-sheet"),
            IndustryTab::Tolerance => t!("ind-tab-tolerance"),
            IndustryTab::Parts => t!("ind-tab-parts"),
            IndustryTab::Assembly => t!("ind-tab-assembly"),
        }
    }
}

// ---- Data dari aplikasi -----------------------------------------------------

#[derive(Debug, Clone, PartialEq, Default)]
pub struct IndStudyRow {
    pub id: String,
    /// Nama jenis (`frequency`, `buckling`, …).
    pub kind: String,
    pub body: String,
    pub running: bool,
    /// Hasil ada tetapi model/setup berubah.
    pub stale: bool,
    /// Baris hasil siap tampil (kosong = belum dijalankan).
    pub lines: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct IndConfigRow {
    pub name: String,
    pub active: bool,
    pub params: Vec<(String, f64)>,
    pub suppressed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct IndSheetRow {
    pub name: String,
    pub thickness: f64,
    pub flanges: usize,
    pub unfolded: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct IndustryData {
    pub target_body: Option<String>,
    pub target_has_material: bool,
    /// Selector face yang sedang dipilih di viewport.
    pub picked_face: Option<String>,
    /// Centroid face terpilih (posisi sisip part standar).
    pub picked_point: Option<[f64; 3]>,
    pub studies: Vec<IndStudyRow>,
    pub configs: Vec<IndConfigRow>,
    pub base_params: Vec<(String, f64)>,
    pub op_ids: Vec<String>,
    pub sheets: Vec<IndSheetRow>,
    /// Ringkasan anotasi lembar gambar.
    pub annotations: Vec<String>,
    /// `(id instance, nama)`.
    pub instances: Vec<(u64, String)>,
    pub couplings: Vec<String>,
    pub explode_steps: Vec<String>,
    pub explode_factor: f64,
    /// Ulir yang tercatat: `body: designation`.
    pub threads: Vec<String>,
}

// ---- Draf & event -----------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AdvStudyKind {
    #[default]
    Frequency,
    Buckling,
    Thermal,
    ThermalStress,
}

impl AdvStudyKind {
    const ALL: [AdvStudyKind; 4] = [
        AdvStudyKind::Frequency,
        AdvStudyKind::Buckling,
        AdvStudyKind::Thermal,
        AdvStudyKind::ThermalStress,
    ];
    pub fn key(self) -> &'static str {
        match self {
            AdvStudyKind::Frequency => "frequency",
            AdvStudyKind::Buckling => "buckling",
            AdvStudyKind::Thermal => "thermal",
            AdvStudyKind::ThermalStress => "thermal_stress",
        }
    }
    fn label(self) -> String {
        match self {
            AdvStudyKind::Frequency => t!("ind-study-frequency"),
            AdvStudyKind::Buckling => t!("ind-study-buckling"),
            AdvStudyKind::Thermal => t!("ind-study-thermal"),
            AdvStudyKind::ThermalStress => t!("ind-study-thermal-stress"),
        }
    }
    pub fn uses_fixtures(self) -> bool {
        self != AdvStudyKind::Thermal
    }
    pub fn uses_thermal(self) -> bool {
        matches!(self, AdvStudyKind::Thermal | AdvStudyKind::ThermalStress)
    }
    pub fn uses_modes(self) -> bool {
        matches!(self, AdvStudyKind::Frequency | AdvStudyKind::Buckling)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThermalBcUi {
    #[default]
    Temperature,
    HeatFlux,
    Convection,
}

impl ThermalBcUi {
    const ALL: [ThermalBcUi; 3] = [
        ThermalBcUi::Temperature,
        ThermalBcUi::HeatFlux,
        ThermalBcUi::Convection,
    ];
    fn label(self) -> String {
        match self {
            ThermalBcUi::Temperature => t!("ind-bc-temperature"),
            ThermalBcUi::HeatFlux => t!("ind-bc-flux"),
            ThermalBcUi::Convection => t!("ind-bc-convection"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThermalBcDraft {
    pub faces: String,
    pub kind: ThermalBcUi,
    /// °C, W/mm², atau koefisien film W/(m²·K).
    pub value: f64,
    /// Suhu sekitar (konveksi), °C.
    pub ambient: f64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AdvStudyDraft {
    pub id: String,
    pub kind: AdvStudyKind,
    /// Selector face tumpuan `fixed`.
    pub fixtures: Vec<String>,
    /// Beban gaya total (N) pada satu kelompok face (buckling/thermal_stress).
    pub load: Option<(String, [f64; 3])>,
    pub thermal: Vec<ThermalBcDraft>,
    pub modes: u32,
    pub tet: bool,
    /// Ukuran sel mesh (mm); 0 = otomatis.
    pub cell_mm: f64,
}

/// Id op sah: `^[a-z][a-z0-9_]{0,31}$`.
pub fn valid_op_id(id: &str) -> bool {
    let mut chars = id.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && id.len() <= 32
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

impl AdvStudyDraft {
    pub fn is_complete(&self) -> bool {
        let fixtures_ok = !self.kind.uses_fixtures() || !self.fixtures.is_empty();
        let thermal_ok = !self.kind.uses_thermal()
            || self.thermal.iter().any(|b| b.kind != ThermalBcUi::HeatFlux);
        let load_ok = self.kind != AdvStudyKind::Buckling || self.load.is_some();
        valid_op_id(&self.id) && fixtures_ok && thermal_ok && load_ok
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SheetFeatureUi {
    #[default]
    EdgeFlange,
    Hem,
    Jog,
}

impl SheetFeatureUi {
    const ALL: [SheetFeatureUi; 3] = [
        SheetFeatureUi::EdgeFlange,
        SheetFeatureUi::Hem,
        SheetFeatureUi::Jog,
    ];
    fn label(self) -> String {
        match self {
            SheetFeatureUi::EdgeFlange => t!("ind-sheet-edge-flange"),
            SheetFeatureUi::Hem => t!("ind-sheet-hem"),
            SheetFeatureUi::Jog => t!("ind-sheet-jog"),
        }
    }
}

/// Tepi pelat dasar yang dilipat.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum SheetEdgesUi {
    /// Kedua sisi sejajar sumbu u sketsa.
    #[default]
    AlongX,
    AlongY,
    /// Selector tepi tulisan tangan.
    Custom(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct StackLinkUi {
    pub nominal: f64,
    pub plus: f64,
    pub minus: f64,
    /// Kelas toleransi ISO 286 (`H7`, `g6`); kosong = pakai plus/minus.
    pub fit: String,
    pub reverse: bool,
}

impl Default for StackLinkUi {
    fn default() -> Self {
        Self {
            nominal: 10.0,
            plus: 0.1,
            minus: 0.1,
            fit: String::new(),
            reverse: false,
        }
    }
}

/// Hasil stack-up draf: `(nominal, total kasus terburuk, total RSS)`.
pub fn stackup_of(chain: &[StackLinkUi]) -> Result<(f64, f64, f64), String> {
    if chain.is_empty() {
        return Err(t!("ind-tol-empty"));
    }
    let mut links = Vec::with_capacity(chain.len());
    for (i, link) in chain.iter().enumerate() {
        let (plus, minus) = if link.fit.trim().is_empty() {
            (link.plus, link.minus)
        } else {
            let fit = IsoFit::parse(&link.fit).map_err(|e| format!("{}: {e}", i + 1))?;
            let (upper, lower) = ducad_core::iso286::limits(link.nominal.abs(), &fit)
                .map_err(|e| format!("{}: {e}", i + 1))?;
            (upper, -lower)
        };
        links.push(if link.reverse {
            (-link.nominal, minus, plus)
        } else {
            (link.nominal, plus, minus)
        });
    }
    let r = tolerance_stackup(&links);
    Ok((r.nominal, r.worst_case_total(), r.rss_total()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CouplingKindUi {
    #[default]
    Gear,
    Screw,
    RackPinion,
}

impl CouplingKindUi {
    const ALL: [CouplingKindUi; 3] = [
        CouplingKindUi::Gear,
        CouplingKindUi::Screw,
        CouplingKindUi::RackPinion,
    ];
    fn label(self) -> String {
        match self {
            CouplingKindUi::Gear => t!("ind-asm-gear"),
            CouplingKindUi::Screw => t!("ind-asm-screw"),
            CouplingKindUi::RackPinion => t!("ind-asm-rack"),
        }
    }
    fn value_label(self) -> String {
        match self {
            CouplingKindUi::Gear => t!("ind-asm-ratio"),
            CouplingKindUi::Screw => t!("ind-asm-pitch"),
            CouplingKindUi::RackPinion => t!("ind-asm-pitch-radius"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum IndustryEvent {
    Close,
    CreateStudy(AdvStudyDraft),
    RunStudy(String),
    CancelStudy,
    DeleteStudy(String),
    ActivateConfig(String),
    SaveConfig {
        name: String,
        params: Vec<(String, f64)>,
        suppressed: Vec<String>,
    },
    DeleteConfig(String),
    ExportConfigCsv,
    ImportConfigCsv,
    BaseFlange {
        id: String,
        width: f64,
        height: f64,
        thickness: f64,
        radius: f64,
        k_factor: f64,
    },
    SheetFeature {
        id: String,
        body: String,
        kind: SheetFeatureUi,
        edges: SheetEdgesUi,
        length: f64,
        /// Sudut tekuk (flange/jog), derajat.
        angle: f64,
        /// Celah hem atau offset jog, mm.
        extra: f64,
    },
    Unfold(String),
    Fold(String),
    FlatPattern(String),
    ExportFlatDxf(String),
    AddStackCheck {
        chain: Vec<StackLinkUi>,
        max_total: f64,
        rss: bool,
    },
    AddAnnotation(Annotation),
    RemoveAnnotation(usize),
    OpenDrawing,
    InsertStandard {
        id: String,
        standard: String,
        size: String,
        length: Option<f64>,
        at: [f64; 3],
    },
    AddThread {
        id: String,
        /// 0 = kisar kasar ISO.
        pitch: f64,
        /// 0 = seluruh silinder.
        length: f64,
        cosmetic: bool,
    },
    AddCoupling {
        kind: CouplingKindUi,
        value: f64,
        driver: u64,
        driven: u64,
        /// 0/1/2 = X/Y/Z.
        driver_axis: usize,
        driven_axis: usize,
    },
    RemoveCoupling(usize),
    AddExplodeStep {
        instance: u64,
        translation: [f64; 3],
    },
    RemoveExplodeStep(usize),
    SetExplode(f64),
}

// ---- Keadaan panel ----------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum AnnotKind {
    #[default]
    Frame,
    Datum,
    Dimension,
    Finish,
}

/// Draf pelat dasar sheet metal.
#[derive(Debug, Clone)]
struct BaseDraft {
    id: String,
    width: f64,
    height: f64,
    thickness: f64,
    radius: f64,
    k_factor: f64,
}

#[derive(Debug, Clone)]
pub struct IndustryPanel {
    pub tab: IndustryTab,
    study: Option<AdvStudyDraft>,
    // konfigurasi
    cfg_name: String,
    cfg_params: Vec<(String, f64)>,
    cfg_suppressed: Vec<String>,
    // sheet metal
    base: BaseDraft,
    feat_id: String,
    feat_kind: SheetFeatureUi,
    feat_edges: SheetEdgesUi,
    feat_custom: String,
    feat_length: f64,
    feat_angle: f64,
    feat_extra: f64,
    sheet_body: usize,
    // toleransi
    chain: Vec<StackLinkUi>,
    max_total: f64,
    rss: bool,
    annot_kind: AnnotKind,
    annot_pos: [f64; 2],
    annot_symbol: GdtSymbol,
    annot_value: f64,
    annot_diameter: bool,
    annot_datums: String,
    annot_label: String,
    annot_fit: String,
    // part standar
    std_id: String,
    std_kind: StandardKind,
    std_size: String,
    std_length: f64,
    std_at: [f64; 3],
    thread_id: String,
    thread_pitch: f64,
    thread_length: f64,
    thread_cosmetic: bool,
    // rakitan
    cp_kind: CouplingKindUi,
    cp_value: f64,
    cp_driver: usize,
    cp_driven: usize,
    cp_axes: (usize, usize),
    step_instance: usize,
    step_move: [f64; 3],
}

impl Default for IndustryPanel {
    fn default() -> Self {
        Self {
            tab: IndustryTab::default(),
            study: None,
            cfg_name: String::new(),
            cfg_params: Vec::new(),
            cfg_suppressed: Vec::new(),
            base: BaseDraft {
                id: "tray".into(),
                width: 100.0,
                height: 60.0,
                thickness: 2.0,
                radius: 2.0,
                k_factor: 0.44,
            },
            feat_id: "flange1".into(),
            feat_kind: SheetFeatureUi::default(),
            feat_edges: SheetEdgesUi::default(),
            feat_custom: String::new(),
            feat_length: 20.0,
            feat_angle: 90.0,
            feat_extra: 2.0,
            sheet_body: 0,
            chain: vec![StackLinkUi::default()],
            max_total: 0.5,
            rss: false,
            annot_kind: AnnotKind::default(),
            annot_pos: [60.0, 40.0],
            annot_symbol: GdtSymbol::ALL[0],
            annot_value: 0.1,
            annot_diameter: false,
            annot_datums: String::new(),
            annot_label: "A".into(),
            annot_fit: "H7".into(),
            std_id: "bolt1".into(),
            std_kind: StandardKind::ALL[0],
            std_size: "M6".into(),
            std_length: 20.0,
            std_at: [0.0; 3],
            thread_id: "thread1".into(),
            thread_pitch: 0.0,
            thread_length: 0.0,
            thread_cosmetic: true,
            cp_kind: CouplingKindUi::default(),
            cp_value: -2.0,
            cp_driver: 0,
            cp_driven: 1,
            cp_axes: (2, 2),
            step_instance: 0,
            step_move: [0.0, 0.0, 30.0],
        }
    }
}

fn muted(ui: &mut Ui, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).small().color(TEXT_MUTED));
}

fn heading(ui: &mut Ui, text: impl Into<String>) {
    ui.add_space(2.0);
    ui.label(RichText::new(text.into()).strong().color(TEXT_SECONDARY));
}

fn num(ui: &mut Ui, label: impl Into<String>, value: &mut f64, speed: f64) {
    ui.label(RichText::new(label.into()).small().color(TEXT_SECONDARY));
    ui.add(DragValue::new(value).speed(speed));
}

fn vec3(ui: &mut Ui, v: &mut [f64; 3], speed: f64) {
    for c in v.iter_mut() {
        ui.add(DragValue::new(c).speed(speed));
    }
}

fn axis_combo(ui: &mut Ui, salt: &str, axis: &mut usize) {
    ComboBox::from_id_salt(salt)
        .width(40.0)
        .selected_text(["X", "Y", "Z"][(*axis).min(2)])
        .show_ui(ui, |ui| crate::theme::glass_menu(ui, |ui| {
            for (i, name) in ["X", "Y", "Z"].iter().enumerate() {
                ui.selectable_value(axis, i, *name);
            }
        }));
}

fn instance_combo(ui: &mut Ui, salt: &str, instances: &[(u64, String)], slot: &mut usize) {
    *slot = (*slot).min(instances.len().saturating_sub(1));
    let current = instances
        .get(*slot)
        .map(|(_, n)| n.clone())
        .unwrap_or_default();
    ComboBox::from_id_salt(salt)
        .width(110.0)
        .selected_text(current)
        .show_ui(ui, |ui| crate::theme::glass_menu(ui, |ui| {
            for (i, (_, n)) in instances.iter().enumerate() {
                ui.selectable_value(slot, i, n);
            }
        }));
}

impl IndustryPanel {
    pub fn show(&mut self, ui: &mut Ui, data: &IndustryData) -> Option<IndustryEvent> {
        let mut event = None;
        glass_frame().show(ui, |ui| {
            ui.set_width(PANEL_W);
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(t!("ind-title")).strong().color(TEXT_PRIMARY));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button(ICON_CLOSE.codepoint)
                        .on_hover_text(t!("ind-close"))
                        .clicked()
                    {
                        event = Some(IndustryEvent::Close);
                    }
                });
            });
            ui.horizontal_wrapped(|ui| {
                for tab in IndustryTab::ALL {
                    ui.selectable_value(&mut self.tab, tab, tab.label());
                }
            });
            ui.separator();
            ScrollArea::vertical().max_height(540.0).show(ui, |ui| {
                let ev = match self.tab {
                    IndustryTab::Study => self.study_tab(ui, data),
                    IndustryTab::Config => self.config_tab(ui, data),
                    IndustryTab::Sheet => self.sheet_tab(ui, data),
                    IndustryTab::Tolerance => self.tolerance_tab(ui, data),
                    IndustryTab::Parts => self.parts_tab(ui, data),
                    IndustryTab::Assembly => self.assembly_tab(ui, data),
                };
                if ev.is_some() {
                    event = ev;
                }
            });
        });
        event
    }

    // ---- Studi lanjutan ----

    fn study_tab(&mut self, ui: &mut Ui, data: &IndustryData) -> Option<IndustryEvent> {
        let mut event = None;
        muted(ui, t!("ind-study-hint"));
        let running = data.studies.iter().any(|s| s.running);
        if data.studies.is_empty() && self.study.is_none() {
            muted(ui, t!("ind-study-empty"));
        }
        for row in &data.studies {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&row.id).color(TEXT_PRIMARY));
                ui.label(RichText::new(&row.kind).small().color(ACCENT_BLUE));
                ui.label(RichText::new(&row.body).small().color(TEXT_MUTED));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(!running, egui::Button::new(ICON_DELETE.codepoint).small())
                        .on_hover_text(t!("ind-delete"))
                        .clicked()
                    {
                        event = Some(IndustryEvent::DeleteStudy(row.id.clone()));
                    }
                    if row.running {
                        if ui
                            .small_button(ICON_STOP.codepoint)
                            .on_hover_text(t!("ind-cancel"))
                            .clicked()
                        {
                            event = Some(IndustryEvent::CancelStudy);
                        }
                        ui.spinner();
                    } else if ui
                        .add_enabled(
                            !running,
                            egui::Button::new(ICON_PLAY_ARROW.codepoint).small(),
                        )
                        .on_hover_text(t!("ind-run"))
                        .clicked()
                    {
                        event = Some(IndustryEvent::RunStudy(row.id.clone()));
                    }
                });
            });
            if row.stale {
                ui.label(
                    RichText::new(t!("ind-study-stale"))
                        .small()
                        .color(ACCENT_ORANGE),
                );
            }
            for line in &row.lines {
                ui.label(RichText::new(line).small().color(ACCENT_GREEN));
            }
            if let Some(why) = &row.error {
                ui.label(RichText::new(why).small().color(FAIL_RED));
            }
        }
        ui.separator();
        let Some(draft) = &mut self.study else {
            if ui
                .button(format!("{} {}", ICON_ADD.codepoint, t!("ind-study-new")))
                .clicked()
            {
                self.study = Some(AdvStudyDraft {
                    id: format!("adv{}", data.studies.len() + 1),
                    modes: 6,
                    ..AdvStudyDraft::default()
                });
            }
            return event;
        };
        let body = data.target_body.clone().unwrap_or_default();
        ui.label(
            RichText::new(t!("ind-target-body", body = body.as_str()))
                .small()
                .color(TEXT_SECONDARY),
        );
        if !data.target_has_material {
            ui.label(
                RichText::new(t!("ind-need-material"))
                    .small()
                    .color(ACCENT_ORANGE),
            );
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new("id").small().color(TEXT_SECONDARY));
            ui.add(TextEdit::singleline(&mut draft.id).desired_width(110.0));
            ComboBox::from_id_salt("ind-study-kind")
                .selected_text(draft.kind.label())
                .show_ui(ui, |ui| crate::theme::glass_menu(ui, |ui| {
                    for k in AdvStudyKind::ALL {
                        ui.selectable_value(&mut draft.kind, k, k.label());
                    }
                }));
        });
        let picked = data.picked_face.clone();
        let pick_hint = picked.clone().unwrap_or_else(|| t!("ind-pick-face"));
        if draft.kind.uses_fixtures() {
            heading(ui, t!("ind-study-fixtures"));
            let mut drop = None;
            for (i, f) in draft.fixtures.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(f).small().color(TEXT_PRIMARY));
                    if ui.small_button(ICON_DELETE.codepoint).clicked() {
                        drop = Some(i);
                    }
                });
            }
            if let Some(i) = drop {
                draft.fixtures.remove(i);
            }
            if ui
                .add_enabled(
                    picked.is_some(),
                    egui::Button::new(t!("ind-add-fixture", face = pick_hint.as_str())).small(),
                )
                .clicked()
            {
                draft.fixtures.extend(picked.clone());
            }
        }
        if matches!(
            draft.kind,
            AdvStudyKind::Buckling | AdvStudyKind::ThermalStress
        ) {
            heading(ui, t!("ind-study-load"));
            let mut clear = false;
            match &mut draft.load {
                Some((faces, newton)) => {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(faces.as_str()).small().color(TEXT_PRIMARY));
                        vec3(ui, newton, 1.0);
                        ui.label(RichText::new("N").small().color(TEXT_MUTED));
                        if ui.small_button(ICON_DELETE.codepoint).clicked() {
                            clear = true;
                        }
                    });
                }
                None => {
                    if ui
                        .add_enabled(
                            picked.is_some(),
                            egui::Button::new(t!("ind-add-load", face = pick_hint.as_str()))
                                .small(),
                        )
                        .clicked()
                    {
                        draft.load = picked.clone().map(|f| (f, [0.0, 0.0, -100.0]));
                    }
                }
            }
            if clear {
                draft.load = None;
            }
        }
        if draft.kind.uses_thermal() {
            heading(ui, t!("ind-study-thermal-bc"));
            let mut drop = None;
            for (i, bc) in draft.thermal.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(bc.faces.as_str()).small().color(TEXT_PRIMARY));
                    ComboBox::from_id_salt(("ind-bc", i))
                        .width(96.0)
                        .selected_text(bc.kind.label())
                        .show_ui(ui, |ui| crate::theme::glass_menu(ui, |ui| {
                            for k in ThermalBcUi::ALL {
                                ui.selectable_value(&mut bc.kind, k, k.label());
                            }
                        }));
                    ui.add(DragValue::new(&mut bc.value).speed(0.5));
                    let unit = match bc.kind {
                        ThermalBcUi::Temperature => "°C",
                        ThermalBcUi::HeatFlux => "W/mm²",
                        ThermalBcUi::Convection => "W/m²K",
                    };
                    ui.label(RichText::new(unit).small().color(TEXT_MUTED));
                    if bc.kind == ThermalBcUi::Convection {
                        ui.add(DragValue::new(&mut bc.ambient).speed(0.5).suffix(" °C"));
                    }
                    if ui.small_button(ICON_DELETE.codepoint).clicked() {
                        drop = Some(i);
                    }
                });
            }
            if let Some(i) = drop {
                draft.thermal.remove(i);
            }
            if ui
                .add_enabled(
                    picked.is_some(),
                    egui::Button::new(t!("ind-add-bc", face = pick_hint.as_str())).small(),
                )
                .clicked()
            {
                if let Some(faces) = picked.clone() {
                    draft.thermal.push(ThermalBcDraft {
                        faces,
                        kind: ThermalBcUi::Temperature,
                        value: 100.0,
                        ambient: 25.0,
                    });
                }
            }
        }
        heading(ui, t!("ind-study-mesh"));
        ui.horizontal(|ui| {
            ui.checkbox(&mut draft.tet, t!("ind-study-tet"));
            ui.label(
                RichText::new(t!("ind-study-cell"))
                    .small()
                    .color(TEXT_SECONDARY),
            );
            ui.add(
                Slider::new(&mut draft.cell_mm, 0.0..=20.0)
                    .step_by(0.25)
                    .suffix(" mm"),
            );
        });
        muted(ui, t!("ind-study-cell-hint"));
        if draft.kind.uses_modes() {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(t!("ind-study-modes"))
                        .small()
                        .color(TEXT_SECONDARY),
                );
                ui.add(Slider::new(&mut draft.modes, 1..=20));
            });
        }
        let ready = draft.is_complete() && data.target_body.is_some();
        let mut close = false;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(ready, egui::Button::new(t!("ind-create")))
                .clicked()
            {
                event = Some(IndustryEvent::CreateStudy(draft.clone()));
                close = true;
            }
            if ui.button(t!("ind-discard")).clicked() {
                close = true;
            }
        });
        if close {
            self.study = None;
        }
        event
    }

    // ---- Konfigurasi ----

    fn config_tab(&mut self, ui: &mut Ui, data: &IndustryData) -> Option<IndustryEvent> {
        let mut event = None;
        muted(ui, t!("ind-cfg-hint"));
        for row in &data.configs {
            ui.horizontal(|ui| {
                if ui.radio(row.active, &row.name).clicked() && !row.active {
                    event = Some(IndustryEvent::ActivateConfig(row.name.clone()));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if row.name != "Default" {
                        if ui
                            .small_button(ICON_DELETE.codepoint)
                            .on_hover_text(t!("ind-delete"))
                            .clicked()
                        {
                            event = Some(IndustryEvent::DeleteConfig(row.name.clone()));
                        }
                        if ui.small_button(t!("ind-edit")).clicked() {
                            self.cfg_name = row.name.clone();
                            self.cfg_params = row.params.clone();
                            self.cfg_suppressed = row.suppressed.clone();
                        }
                    }
                });
            });
            let mut parts: Vec<String> =
                row.params.iter().map(|(k, v)| format!("{k}={v}")).collect();
            parts.extend(row.suppressed.iter().map(|s| format!("-{s}")));
            if !parts.is_empty() {
                muted(ui, parts.join("  "));
            }
        }
        ui.separator();
        heading(ui, t!("ind-cfg-editor"));
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(t!("ind-cfg-name"))
                    .small()
                    .color(TEXT_SECONDARY),
            );
            ui.add(TextEdit::singleline(&mut self.cfg_name).desired_width(140.0));
        });
        if data.base_params.is_empty() {
            muted(ui, t!("ind-cfg-no-params"));
        }
        for (name, base) in &data.base_params {
            ui.horizontal(|ui| {
                let slot = self.cfg_params.iter().position(|(k, _)| k == name);
                let mut on = slot.is_some();
                if ui.checkbox(&mut on, name).changed() {
                    match (on, slot) {
                        (true, None) => self.cfg_params.push((name.clone(), *base)),
                        (false, Some(i)) => {
                            self.cfg_params.remove(i);
                        }
                        _ => {}
                    }
                }
                match self.cfg_params.iter_mut().find(|(k, _)| k == name) {
                    Some((_, v)) => {
                        ui.add(DragValue::new(v).speed(0.5));
                    }
                    None => muted(ui, format!("{base}")),
                }
            });
        }
        if !data.op_ids.is_empty() {
            heading(ui, t!("ind-cfg-suppress"));
            ui.horizontal_wrapped(|ui| {
                for id in &data.op_ids {
                    let mut on = self.cfg_suppressed.contains(id);
                    if ui.checkbox(&mut on, id).changed() {
                        if on {
                            self.cfg_suppressed.push(id.clone());
                        } else {
                            self.cfg_suppressed.retain(|s| s != id);
                        }
                    }
                }
            });
        }
        ui.horizontal(|ui| {
            let ok = !self.cfg_name.trim().is_empty() && self.cfg_name.trim() != "Default";
            if ui
                .add_enabled(ok, egui::Button::new(t!("ind-cfg-save")))
                .clicked()
            {
                event = Some(IndustryEvent::SaveConfig {
                    name: self.cfg_name.trim().to_string(),
                    params: self.cfg_params.clone(),
                    suppressed: self.cfg_suppressed.clone(),
                });
            }
            if ui.button(t!("ind-cfg-export")).clicked() {
                event = Some(IndustryEvent::ExportConfigCsv);
            }
            if ui.button(t!("ind-cfg-import")).clicked() {
                event = Some(IndustryEvent::ImportConfigCsv);
            }
        });
        event
    }

    // ---- Sheet metal ----

    fn sheet_tab(&mut self, ui: &mut Ui, data: &IndustryData) -> Option<IndustryEvent> {
        let mut event = None;
        heading(ui, t!("ind-sheet-base"));
        muted(ui, t!("ind-sheet-base-hint"));
        let b = &mut self.base;
        ui.horizontal(|ui| {
            ui.label(RichText::new("id").small().color(TEXT_SECONDARY));
            ui.add(TextEdit::singleline(&mut b.id).desired_width(90.0));
            num(ui, t!("ind-sheet-width"), &mut b.width, 1.0);
            num(ui, t!("ind-sheet-height"), &mut b.height, 1.0);
        });
        ui.horizontal(|ui| {
            num(ui, t!("ind-sheet-thickness"), &mut b.thickness, 0.1);
            num(ui, t!("ind-sheet-radius"), &mut b.radius, 0.1);
            num(ui, "k", &mut b.k_factor, 0.01);
        });
        let base_ok = valid_op_id(&b.id)
            && b.width > 0.0
            && b.height > 0.0
            && b.thickness > 0.0
            && b.radius >= 0.0;
        if ui
            .add_enabled(base_ok, egui::Button::new(t!("ind-sheet-create-base")))
            .clicked()
        {
            event = Some(IndustryEvent::BaseFlange {
                id: b.id.clone(),
                width: b.width,
                height: b.height,
                thickness: b.thickness,
                radius: b.radius,
                k_factor: b.k_factor,
            });
        }
        ui.separator();
        if data.sheets.is_empty() {
            muted(ui, t!("ind-sheet-empty"));
            return event;
        }
        self.sheet_body = self.sheet_body.min(data.sheets.len() - 1);
        heading(ui, t!("ind-sheet-bodies"));
        for (i, row) in data.sheets.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.sheet_body, i, &row.name);
                let flanges = row.flanges.to_string();
                let thickness = format!("{:.2}", row.thickness);
                muted(
                    ui,
                    t!(
                        "ind-sheet-row",
                        thickness = thickness.as_str(),
                        flanges = flanges.as_str()
                    ),
                );
                if row.unfolded {
                    ui.label(
                        RichText::new(t!("ind-sheet-unfolded"))
                            .small()
                            .color(ACCENT_ORANGE),
                    );
                }
            });
        }
        let row = &data.sheets[self.sheet_body];
        ui.horizontal(|ui| {
            if row.unfolded {
                if ui.button(t!("ind-sheet-fold")).clicked() {
                    event = Some(IndustryEvent::Fold(row.name.clone()));
                }
            } else if ui.button(t!("ind-sheet-unfold")).clicked() {
                event = Some(IndustryEvent::Unfold(row.name.clone()));
            }
            if ui.button(t!("ind-sheet-flat")).clicked() {
                event = Some(IndustryEvent::FlatPattern(row.name.clone()));
            }
            if ui.button(t!("ind-sheet-dxf")).clicked() {
                event = Some(IndustryEvent::ExportFlatDxf(row.name.clone()));
            }
        });
        ui.separator();
        heading(ui, t!("ind-sheet-feature"));
        ui.horizontal(|ui| {
            ui.label(RichText::new("id").small().color(TEXT_SECONDARY));
            ui.add(TextEdit::singleline(&mut self.feat_id).desired_width(90.0));
            ComboBox::from_id_salt("ind-sheet-kind")
                .selected_text(self.feat_kind.label())
                .show_ui(ui, |ui| crate::theme::glass_menu(ui, |ui| {
                    for k in SheetFeatureUi::ALL {
                        ui.selectable_value(&mut self.feat_kind, k, k.label());
                    }
                }));
        });
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(t!("ind-sheet-edges"))
                    .small()
                    .color(TEXT_SECONDARY),
            );
            let custom = matches!(self.feat_edges, SheetEdgesUi::Custom(_));
            if ui
                .radio(
                    self.feat_edges == SheetEdgesUi::AlongX,
                    t!("ind-sheet-along-x"),
                )
                .clicked()
            {
                self.feat_edges = SheetEdgesUi::AlongX;
            }
            if ui
                .radio(
                    self.feat_edges == SheetEdgesUi::AlongY,
                    t!("ind-sheet-along-y"),
                )
                .clicked()
            {
                self.feat_edges = SheetEdgesUi::AlongY;
            }
            if ui.radio(custom, t!("ind-sheet-custom")).clicked() {
                self.feat_edges = SheetEdgesUi::Custom(self.feat_custom.clone());
            }
        });
        if matches!(self.feat_edges, SheetEdgesUi::Custom(_)) {
            ui.add(
                TextEdit::singleline(&mut self.feat_custom)
                    .hint_text("|X[len=100][z=0]")
                    .desired_width(220.0),
            );
            self.feat_edges = SheetEdgesUi::Custom(self.feat_custom.clone());
        }
        ui.horizontal(|ui| {
            num(ui, t!("ind-sheet-length"), &mut self.feat_length, 0.5);
            match self.feat_kind {
                SheetFeatureUi::EdgeFlange => {
                    num(ui, t!("ind-sheet-angle"), &mut self.feat_angle, 1.0)
                }
                SheetFeatureUi::Hem => num(ui, t!("ind-sheet-gap"), &mut self.feat_extra, 0.1),
                SheetFeatureUi::Jog => {
                    num(ui, t!("ind-sheet-offset"), &mut self.feat_extra, 0.5);
                    num(ui, t!("ind-sheet-angle"), &mut self.feat_angle, 1.0);
                }
            }
        });
        let custom_ok = match &self.feat_edges {
            SheetEdgesUi::Custom(s) => !s.trim().is_empty(),
            _ => true,
        };
        let ok = valid_op_id(&self.feat_id) && self.feat_length > 0.0 && custom_ok && !row.unfolded;
        if ui
            .add_enabled(ok, egui::Button::new(t!("ind-sheet-add-feature")))
            .clicked()
        {
            event = Some(IndustryEvent::SheetFeature {
                id: self.feat_id.clone(),
                body: row.name.clone(),
                kind: self.feat_kind,
                edges: self.feat_edges.clone(),
                length: self.feat_length,
                angle: self.feat_angle,
                extra: self.feat_extra,
            });
        }
        muted(ui, t!("ind-sheet-limits"));
        event
    }

    // ---- Toleransi & GD&T ----

    fn tolerance_tab(&mut self, ui: &mut Ui, data: &IndustryData) -> Option<IndustryEvent> {
        let mut event = None;
        heading(ui, t!("ind-tol-stack"));
        muted(ui, t!("ind-tol-hint"));
        let mut drop = None;
        for (i, link) in self.chain.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(DragValue::new(&mut link.nominal).speed(0.5).suffix(" mm"));
                ui.add(
                    TextEdit::singleline(&mut link.fit)
                        .hint_text("H7")
                        .desired_width(36.0),
                );
                if link.fit.trim().is_empty() {
                    ui.add(DragValue::new(&mut link.plus).speed(0.01).prefix("+"));
                    ui.add(DragValue::new(&mut link.minus).speed(0.01).prefix("-"));
                }
                ui.checkbox(&mut link.reverse, t!("ind-tol-reverse"));
                if ui.small_button(ICON_DELETE.codepoint).clicked() {
                    drop = Some(i);
                }
            });
        }
        if let Some(i) = drop {
            self.chain.remove(i);
        }
        if ui
            .small_button(format!("{} {}", ICON_ADD.codepoint, t!("ind-tol-add-link")))
            .clicked()
        {
            self.chain.push(StackLinkUi::default());
        }
        let result = stackup_of(&self.chain);
        match &result {
            Ok((nominal, worst, rss)) => {
                let (n, w, r) = (
                    format!("{nominal:.4}"),
                    format!("{worst:.4}"),
                    format!("{rss:.4}"),
                );
                ui.label(
                    RichText::new(t!(
                        "ind-tol-result",
                        nominal = n.as_str(),
                        worst = w.as_str(),
                        rss = r.as_str()
                    ))
                    .color(ACCENT_GREEN),
                );
            }
            Err(why) => {
                ui.label(RichText::new(why).small().color(FAIL_RED));
            }
        }
        ui.horizontal(|ui| {
            num(ui, t!("ind-tol-max"), &mut self.max_total, 0.01);
            ui.checkbox(&mut self.rss, "RSS");
            if ui
                .add_enabled(result.is_ok(), egui::Button::new(t!("ind-tol-add-check")))
                .clicked()
            {
                event = Some(IndustryEvent::AddStackCheck {
                    chain: self.chain.clone(),
                    max_total: self.max_total,
                    rss: self.rss,
                });
            }
        });
        ui.separator();
        heading(ui, t!("ind-gdt-title"));
        muted(ui, t!("ind-gdt-hint"));
        let mut remove = None;
        for (i, a) in data.annotations.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(a).small().color(TEXT_PRIMARY));
                if ui.small_button(ICON_DELETE.codepoint).clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            event = Some(IndustryEvent::RemoveAnnotation(i));
        }
        ui.horizontal(|ui| {
            for (kind, label) in [
                (AnnotKind::Frame, t!("ind-gdt-frame")),
                (AnnotKind::Datum, t!("ind-gdt-datum")),
                (AnnotKind::Dimension, t!("ind-gdt-dimension")),
                (AnnotKind::Finish, t!("ind-gdt-finish")),
            ] {
                ui.selectable_value(&mut self.annot_kind, kind, label);
            }
        });
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(t!("ind-gdt-position"))
                    .small()
                    .color(TEXT_SECONDARY),
            );
            ui.add(DragValue::new(&mut self.annot_pos[0]).speed(1.0));
            ui.add(DragValue::new(&mut self.annot_pos[1]).speed(1.0));
            ui.label(RichText::new("mm").small().color(TEXT_MUTED));
        });
        let annotation = match self.annot_kind {
            AnnotKind::Frame => {
                ui.horizontal(|ui| {
                    ComboBox::from_id_salt("ind-gdt-symbol")
                        .selected_text(self.annot_symbol.name())
                        .show_ui(ui, |ui| crate::theme::glass_menu(ui, |ui| {
                            for s in GdtSymbol::ALL {
                                ui.selectable_value(&mut self.annot_symbol, s, s.name());
                            }
                        }));
                    ui.add(DragValue::new(&mut self.annot_value).speed(0.01));
                    ui.checkbox(&mut self.annot_diameter, t!("ind-gdt-diameter"));
                });
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(t!("ind-gdt-datums"))
                            .small()
                            .color(TEXT_SECONDARY),
                    );
                    ui.add(
                        TextEdit::singleline(&mut self.annot_datums)
                            .hint_text("A,B")
                            .desired_width(80.0),
                    );
                });
                Annotation::FeatureControlFrame {
                    position: self.annot_pos,
                    frame: FeatureControlFrame {
                        symbol: self.annot_symbol,
                        value: self.annot_value,
                        diameter_zone: self.annot_diameter,
                        modifiers: Vec::new(),
                        datums: self
                            .annot_datums
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect(),
                    },
                }
            }
            AnnotKind::Datum => {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(t!("ind-gdt-label"))
                            .small()
                            .color(TEXT_SECONDARY),
                    );
                    ui.add(TextEdit::singleline(&mut self.annot_label).desired_width(40.0));
                });
                Annotation::DatumFeature {
                    position: self.annot_pos,
                    datum: DatumFeature {
                        label: self.annot_label.trim().to_string(),
                    },
                }
            }
            AnnotKind::Dimension => {
                ui.horizontal(|ui| {
                    ui.add(
                        DragValue::new(&mut self.annot_value)
                            .speed(0.5)
                            .suffix(" mm"),
                    );
                    ui.add(
                        TextEdit::singleline(&mut self.annot_fit)
                            .hint_text("H7")
                            .desired_width(40.0),
                    );
                    ui.checkbox(&mut self.annot_diameter, t!("ind-gdt-diameter"));
                });
                let mut dimension = match IsoFit::parse(&self.annot_fit) {
                    Ok(fit) => DimensionTolerance::with_fit(self.annot_value, fit),
                    Err(_) => DimensionTolerance::with_limits(self.annot_value, 0.1, 0.1),
                };
                dimension.diameter = self.annot_diameter;
                Annotation::DimensionTolerance {
                    position: self.annot_pos,
                    dimension,
                }
            }
            AnnotKind::Finish => {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Ra").small().color(TEXT_SECONDARY));
                    ui.add(
                        DragValue::new(&mut self.annot_value)
                            .speed(0.1)
                            .suffix(" µm"),
                    );
                });
                Annotation::SurfaceFinish {
                    position: self.annot_pos,
                    finish: SurfaceFinish {
                        ra_um: self.annot_value,
                    },
                }
            }
        };
        ui.horizontal(|ui| {
            if ui.button(t!("ind-gdt-add")).clicked() {
                event = Some(IndustryEvent::AddAnnotation(annotation));
            }
            if ui.button(t!("ind-gdt-open-sheet")).clicked() {
                event = Some(IndustryEvent::OpenDrawing);
            }
        });
        event
    }

    // ---- Part standar & ulir ----

    fn parts_tab(&mut self, ui: &mut Ui, data: &IndustryData) -> Option<IndustryEvent> {
        let mut event = None;
        heading(ui, t!("ind-parts-title"));
        ui.horizontal(|ui| {
            ui.label(RichText::new("id").small().color(TEXT_SECONDARY));
            ui.add(TextEdit::singleline(&mut self.std_id).desired_width(90.0));
            let before = self.std_kind;
            ComboBox::from_id_salt("ind-std-kind")
                .selected_text(self.std_kind.standard_number())
                .show_ui(ui, |ui| crate::theme::glass_menu(ui, |ui| {
                    for k in StandardKind::ALL {
                        ui.selectable_value(&mut self.std_kind, k, k.standard_number());
                    }
                }));
            let sizes = self.std_kind.sizes();
            if before != self.std_kind || !sizes.contains(&self.std_size.as_str()) {
                self.std_size = sizes.first().copied().unwrap_or_default().to_string();
            }
            ComboBox::from_id_salt("ind-std-size")
                .width(70.0)
                .selected_text(self.std_size.clone())
                .show_ui(ui, |ui| crate::theme::glass_menu(ui, |ui| {
                    for s in sizes {
                        ui.selectable_value(&mut self.std_size, s.to_string(), s);
                    }
                }));
        });
        ui.horizontal(|ui| {
            if self.std_kind.needs_length() {
                num(ui, t!("ind-parts-length"), &mut self.std_length, 1.0);
            }
            ui.label(
                RichText::new(t!("ind-parts-at"))
                    .small()
                    .color(TEXT_SECONDARY),
            );
            vec3(ui, &mut self.std_at, 0.5);
        });
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    data.picked_point.is_some(),
                    egui::Button::new(t!("ind-parts-use-face")).small(),
                )
                .clicked()
            {
                if let Some(p) = data.picked_point {
                    self.std_at = p;
                }
            }
            if ui
                .add_enabled(
                    valid_op_id(&self.std_id),
                    egui::Button::new(t!("ind-parts-insert")),
                )
                .clicked()
            {
                event = Some(IndustryEvent::InsertStandard {
                    id: self.std_id.clone(),
                    standard: self.std_kind.key().to_string(),
                    size: self.std_size.clone(),
                    length: self.std_kind.needs_length().then_some(self.std_length),
                    at: self.std_at,
                });
            }
        });
        ui.separator();
        heading(ui, t!("ind-thread-title"));
        let face = data
            .picked_face
            .clone()
            .unwrap_or_else(|| t!("ind-pick-face"));
        muted(ui, t!("ind-thread-hint", face = face.as_str()));
        ui.horizontal(|ui| {
            ui.label(RichText::new("id").small().color(TEXT_SECONDARY));
            ui.add(TextEdit::singleline(&mut self.thread_id).desired_width(90.0));
            num(ui, t!("ind-thread-pitch"), &mut self.thread_pitch, 0.05);
            num(ui, t!("ind-thread-length"), &mut self.thread_length, 0.5);
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.thread_cosmetic, t!("ind-thread-cosmetic"));
            let ok = valid_op_id(&self.thread_id)
                && data.picked_face.is_some()
                && data.target_body.is_some();
            if ui
                .add_enabled(ok, egui::Button::new(t!("ind-thread-add")))
                .clicked()
            {
                event = Some(IndustryEvent::AddThread {
                    id: self.thread_id.clone(),
                    pitch: self.thread_pitch.max(0.0),
                    length: self.thread_length.max(0.0),
                    cosmetic: self.thread_cosmetic,
                });
            }
        });
        if !self.thread_cosmetic {
            ui.label(
                RichText::new(t!("ind-thread-slow"))
                    .small()
                    .color(ACCENT_ORANGE),
            );
        }
        for line in &data.threads {
            ui.label(RichText::new(line).small().color(TEXT_SECONDARY));
        }
        event
    }

    // ---- Rakitan ----

    fn assembly_tab(&mut self, ui: &mut Ui, data: &IndustryData) -> Option<IndustryEvent> {
        let mut event = None;
        if data.instances.len() < 2 {
            muted(ui, t!("ind-asm-need-two"));
        }
        heading(ui, t!("ind-asm-couplings"));
        muted(ui, t!("ind-asm-coupling-hint"));
        let mut drop = None;
        for (i, c) in data.couplings.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(c).small().color(TEXT_PRIMARY));
                if ui.small_button(ICON_DELETE.codepoint).clicked() {
                    drop = Some(i);
                }
            });
        }
        if let Some(i) = drop {
            event = Some(IndustryEvent::RemoveCoupling(i));
        }
        ui.horizontal(|ui| {
            ComboBox::from_id_salt("ind-cp-kind")
                .selected_text(self.cp_kind.label())
                .show_ui(ui, |ui| crate::theme::glass_menu(ui, |ui| {
                    for k in CouplingKindUi::ALL {
                        ui.selectable_value(&mut self.cp_kind, k, k.label());
                    }
                }));
            num(ui, self.cp_kind.value_label(), &mut self.cp_value, 0.1);
        });
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(t!("ind-asm-driver"))
                    .small()
                    .color(TEXT_SECONDARY),
            );
            instance_combo(ui, "ind-cp-driver", &data.instances, &mut self.cp_driver);
            axis_combo(ui, "ind-cp-driver-axis", &mut self.cp_axes.0);
        });
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(t!("ind-asm-driven"))
                    .small()
                    .color(TEXT_SECONDARY),
            );
            instance_combo(ui, "ind-cp-driven", &data.instances, &mut self.cp_driven);
            axis_combo(ui, "ind-cp-driven-axis", &mut self.cp_axes.1);
        });
        let pair_ok =
            data.instances.len() >= 2 && self.cp_driver != self.cp_driven && self.cp_value != 0.0;
        if ui
            .add_enabled(pair_ok, egui::Button::new(t!("ind-asm-add-coupling")))
            .clicked()
        {
            if let (Some((driver, _)), Some((driven, _))) = (
                data.instances.get(self.cp_driver),
                data.instances.get(self.cp_driven),
            ) {
                event = Some(IndustryEvent::AddCoupling {
                    kind: self.cp_kind,
                    value: self.cp_value,
                    driver: *driver,
                    driven: *driven,
                    driver_axis: self.cp_axes.0,
                    driven_axis: self.cp_axes.1,
                });
            }
        }
        ui.separator();
        heading(ui, t!("ind-asm-explode"));
        muted(ui, t!("ind-asm-explode-hint"));
        let mut drop = None;
        for (i, s) in data.explode_steps.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{}. {s}", i + 1))
                        .small()
                        .color(TEXT_PRIMARY),
                );
                if ui.small_button(ICON_DELETE.codepoint).clicked() {
                    drop = Some(i);
                }
            });
        }
        if let Some(i) = drop {
            event = Some(IndustryEvent::RemoveExplodeStep(i));
        }
        ui.horizontal(|ui| {
            instance_combo(
                ui,
                "ind-step-instance",
                &data.instances,
                &mut self.step_instance,
            );
            vec3(ui, &mut self.step_move, 1.0);
            ui.label(RichText::new("mm").small().color(TEXT_MUTED));
        });
        if ui
            .add_enabled(
                !data.instances.is_empty(),
                egui::Button::new(t!("ind-asm-add-step")),
            )
            .clicked()
        {
            if let Some((instance, _)) = data.instances.get(self.step_instance) {
                event = Some(IndustryEvent::AddExplodeStep {
                    instance: *instance,
                    translation: self.step_move,
                });
            }
        }
        let mut factor = data.explode_factor;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(t!("ind-asm-factor"))
                    .small()
                    .color(TEXT_SECONDARY),
            );
            if ui.add(Slider::new(&mut factor, 0.0..=1.0)).changed() {
                event = Some(IndustryEvent::SetExplode(factor));
            }
        });
        event
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stackup_matches_manual_sum() {
        let chain = vec![
            StackLinkUi {
                nominal: 20.0,
                ..StackLinkUi::default()
            },
            StackLinkUi {
                nominal: 15.0,
                reverse: true,
                ..StackLinkUi::default()
            },
        ];
        let (nominal, worst, rss) = stackup_of(&chain).unwrap();
        assert!((nominal - 5.0).abs() < 1e-9);
        assert!((worst - 0.4).abs() < 1e-9, "{worst}");
        assert!(rss < worst && rss > 0.0);
        assert!(stackup_of(&[]).is_err());
        let bad = vec![StackLinkUi {
            fit: "Q7".into(),
            ..StackLinkUi::default()
        }];
        assert!(stackup_of(&bad).is_err());
    }

    #[test]
    fn study_draft_completeness_follows_kind() {
        let mut d = AdvStudyDraft {
            id: "modes".into(),
            ..AdvStudyDraft::default()
        };
        assert!(!d.is_complete(), "frekuensi butuh tumpuan");
        d.fixtures.push("<X".into());
        assert!(d.is_complete());
        d.kind = AdvStudyKind::Buckling;
        assert!(!d.is_complete(), "buckling butuh beban");
        d.load = Some((">X".into(), [-100.0, 0.0, 0.0]));
        assert!(d.is_complete());
        d.kind = AdvStudyKind::Thermal;
        assert!(!d.is_complete(), "termal butuh suhu/konveksi");
        d.thermal.push(ThermalBcDraft {
            faces: "<X".into(),
            kind: ThermalBcUi::HeatFlux,
            value: 0.01,
            ambient: 25.0,
        });
        assert!(!d.is_complete(), "fluks saja tidak cukup");
        d.thermal[0].kind = ThermalBcUi::Temperature;
        assert!(d.is_complete());
        d.id = "Bad Id".into();
        assert!(!d.is_complete());
    }
}
