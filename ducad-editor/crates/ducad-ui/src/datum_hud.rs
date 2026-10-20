//! HUD geometri konstruksi datum (PLAN_PARITAS_SOLIDCRAFT Fase 1A, butir
//! 1.2–1.4): pemilih jenis datum (bidang / sumbu / titik), HUD sumbu & titik
//! datum, serta potongan HUD yang dipakai bersama mode bidang baru di
//! [`crate::canvas_hud::CanvasHud::render_datum_plane_hud`].
//!
//! Semua label lewat FTL; ikon dari `egui_icons` (tanpa glyph Unicode).

use ducad_i18n::t;
use egui::{Rect, RichText, Ui};
use egui_icons::icons::{ICON_CLEAR, ICON_GPS_FIXED, ICON_LAYERS, ICON_TIMELINE};

use crate::canvas_hud::CanvasHud;
use crate::theme::{ACCENT_BLUE, ACCENT_ORANGE, TEXT_PRIMARY, TEXT_SECONDARY};

/// Jenis datum yang sedang dibuat oleh tool Datum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DatumKind {
    #[default]
    Plane,
    Axis,
    Point,
}

impl DatumKind {
    pub const ALL: [DatumKind; 3] = [DatumKind::Plane, DatumKind::Axis, DatumKind::Point];

    pub fn label(self) -> String {
        match self {
            DatumKind::Plane => t!("datum-kind-plane"),
            DatumKind::Axis => t!("datum-kind-axis"),
            DatumKind::Point => t!("datum-kind-point"),
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            DatumKind::Plane => ICON_LAYERS.codepoint,
            DatumKind::Axis => ICON_TIMELINE.codepoint,
            DatumKind::Point => ICON_GPS_FIXED.codepoint,
        }
    }
}

/// Cara membuat sumbu datum (sama dengan varian `AxisDef` op `construct_axis`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DatumAxisMode {
    #[default]
    TwoPoints,
    Edge,
    NormalToFace,
    ThroughCylinder,
    TwoPlanes,
}

impl DatumAxisMode {
    pub const ALL: [DatumAxisMode; 5] = [
        DatumAxisMode::TwoPoints,
        DatumAxisMode::Edge,
        DatumAxisMode::NormalToFace,
        DatumAxisMode::ThroughCylinder,
        DatumAxisMode::TwoPlanes,
    ];

    pub fn label(self) -> String {
        match self {
            DatumAxisMode::TwoPoints => t!("datum-axis-two-points"),
            DatumAxisMode::Edge => t!("datum-axis-edge"),
            DatumAxisMode::NormalToFace => t!("datum-axis-normal"),
            DatumAxisMode::ThroughCylinder => t!("datum-axis-cylinder"),
            DatumAxisMode::TwoPlanes => t!("datum-axis-two-planes"),
        }
    }
}

/// Cara membuat titik datum (sama dengan varian `PointDef` op `construct_point`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DatumPointMode {
    #[default]
    Vertex,
    Center,
    EdgePlane,
    TwoEdges,
    ThreePlanes,
    AlongPath,
}

impl DatumPointMode {
    pub const ALL: [DatumPointMode; 6] = [
        DatumPointMode::Vertex,
        DatumPointMode::Center,
        DatumPointMode::EdgePlane,
        DatumPointMode::TwoEdges,
        DatumPointMode::ThreePlanes,
        DatumPointMode::AlongPath,
    ];

    pub fn label(self) -> String {
        match self {
            DatumPointMode::Vertex => t!("datum-point-vertex"),
            DatumPointMode::Center => t!("datum-point-center"),
            DatumPointMode::EdgePlane => t!("datum-point-edge-plane"),
            DatumPointMode::TwoEdges => t!("datum-point-two-edges"),
            DatumPointMode::ThreePlanes => t!("datum-point-three-planes"),
            DatumPointMode::AlongPath => t!("datum-point-along-path"),
        }
    }
}

/// Aksi HUD sumbu/titik datum.
#[derive(Debug, Clone, PartialEq)]
pub enum DatumRefHudAction {
    SwitchKind(DatumKind),
    SetAxisMode(DatumAxisMode),
    SetPointMode(DatumPointMode),
    /// Tambahkan bidang standar/datum (indeks gaya lama: 0=Top, 1=Front,
    /// 2=Right, 3+=datum) sebagai pilihan bidang berikutnya.
    AddPlane(usize),
    ClearPicks,
    SetFraction(f64),
    Commit,
    Cancel,
}

/// Masukan HUD sumbu/titik datum.
pub struct DatumRefHudInput<'a> {
    pub kind: DatumKind,
    pub axis_mode: DatumAxisMode,
    pub point_mode: DatumPointMode,
    pub plane_names: &'a [(usize, String)],
    /// Ringkasan pilihan, mis. "Edge 1/2".
    pub pick_status: &'a str,
    pub ready: bool,
    /// Mode ini butuh bidang (tampilkan pemilih "tambah bidang").
    pub wants_plane: bool,
    pub needs_fraction: bool,
}

/// Tombol pemilih jenis datum [Bidang | Sumbu | Titik]. Mengembalikan jenis
/// yang baru diklik.
pub fn kind_selector(ui: &mut Ui, current: DatumKind) -> Option<DatumKind> {
    let mut out = None;
    for k in DatumKind::ALL {
        let label = format!("{} {}", k.icon(), k.label());
        if CanvasHud::hud_toggle_btn(ui, label, current == k).clicked() && current != k {
            out = Some(k);
        }
    }
    out
}

/// Pemilih "tambah bidang" (bidang standar + datum) untuk mode yang butuh
/// bidang. Mengembalikan indeks bidang yang dipilih.
pub fn add_plane_combo(ui: &mut Ui, id_salt: &str, plane_names: &[(usize, String)]) -> Option<usize> {
    let mut out = None;
    egui::ComboBox::from_id_salt(id_salt)
        .selected_text(RichText::new(t!("datum-add-plane")).size(10.5).color(TEXT_PRIMARY))
        .width(96.0)
        .show_ui(ui, |ui| {
            crate::theme::glass_menu(ui, |ui| {
                for (idx, name) in plane_names {
                    if ui.selectable_label(false, RichText::new(name).size(10.5)).clicked() {
                        out = Some(*idx);
                    }
                }
            })
        });
    out
}

/// Status pilihan + tombol hapus pilihan.
pub fn pick_status(ui: &mut Ui, status: &str, ready: bool, has_picks: bool) -> bool {
    ui.label(
        RichText::new(status)
            .size(10.5)
            .color(if ready { ACCENT_BLUE } else { ACCENT_ORANGE }),
    );
    has_picks
        && CanvasHud::hud_toggle_btn(ui, ICON_CLEAR.codepoint, false)
            .on_hover_text(t!("datum-clear-picks"))
            .clicked()
}

/// Isian pecahan panjang (0..1) untuk mode "sepanjang lintasan".
pub fn fraction_field(ui: &mut Ui, fraction_input: &mut String) -> Option<f64> {
    let mut out = None;
    ui.label(
        RichText::new(t!("datum-fraction"))
            .size(10.0)
            .color(TEXT_SECONDARY),
    );
    for f in [0.25, 0.5, 0.75] {
        let cur = fraction_input.trim().parse::<f64>().ok();
        let active = cur.is_some_and(|c| (c - f).abs() < 1e-6);
        if CanvasHud::hud_circle_btn(ui, format!("{f}"), active).clicked() {
            *fraction_input = format!("{f}");
            out = Some(f);
        }
    }
    let resp = ui.add(
        egui::TextEdit::singleline(fraction_input)
            .desired_width(38.0)
            .font(egui::FontId::monospace(10.5)),
    );
    if resp.changed() {
        if let Ok(v) = fraction_input.trim().parse::<f64>() {
            out = Some(v);
        }
    }
    out
}

/// HUD header untuk membuat sumbu atau titik datum.
pub fn render_datum_ref_hud(
    ui: &mut Ui,
    canvas_rect: Rect,
    input: &DatumRefHudInput<'_>,
    fraction_input: &mut String,
) -> Option<DatumRefHudAction> {
    let banner_w = 760.0;
    CanvasHud::render_header_hud_container(
        ui,
        canvas_rect,
        banner_w,
        input.ready,
        "ducad-hud-datum-ref-banner",
        |ui| {
            let mut action = None;
            if let Some(k) = kind_selector(ui, input.kind) {
                action = Some(DatumRefHudAction::SwitchKind(k));
            }
            ui.separator();

            // Mode: combo agar banner tetap muat untuk 5–6 mode.
            let current = match input.kind {
                DatumKind::Point => input.point_mode.label(),
                _ => input.axis_mode.label(),
            };
            egui::ComboBox::from_id_salt("ducad-datum-ref-mode")
                .selected_text(RichText::new(current).size(10.5).color(TEXT_PRIMARY))
                .width(130.0)
                .show_ui(ui, |ui| {
                    crate::theme::glass_menu(ui, |ui| match input.kind {
                        DatumKind::Point => {
                            for m in DatumPointMode::ALL {
                                if ui
                                    .selectable_label(m == input.point_mode, RichText::new(m.label()).size(10.5))
                                    .clicked()
                                {
                                    action = Some(DatumRefHudAction::SetPointMode(m));
                                }
                            }
                        }
                        _ => {
                            for m in DatumAxisMode::ALL {
                                if ui
                                    .selectable_label(m == input.axis_mode, RichText::new(m.label()).size(10.5))
                                    .clicked()
                                {
                                    action = Some(DatumRefHudAction::SetAxisMode(m));
                                }
                            }
                        }
                    })
                });
            ui.separator();

            if pick_status(ui, input.pick_status, input.ready, true) {
                action = Some(DatumRefHudAction::ClearPicks);
            }
            if input.wants_plane {
                if let Some(idx) = add_plane_combo(ui, "ducad-datum-ref-add-plane", input.plane_names) {
                    action = Some(DatumRefHudAction::AddPlane(idx));
                }
            }
            if input.needs_fraction {
                if let Some(f) = fraction_field(ui, fraction_input) {
                    action = Some(DatumRefHudAction::SetFraction(f));
                }
            }
            ui.separator();

            let create = match input.kind {
                DatumKind::Point => t!("datum-create-point"),
                _ => t!("datum-create-axis"),
            };
            if CanvasHud::hud_commit_btn(ui, create).clicked() {
                action = Some(DatumRefHudAction::Commit);
            }
            if CanvasHud::hud_toggle_btn(ui, t!("datum-cancel"), false).clicked() {
                action = Some(DatumRefHudAction::Cancel);
            }
            action
        },
    )
}
