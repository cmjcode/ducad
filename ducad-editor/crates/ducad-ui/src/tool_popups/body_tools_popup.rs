//! Panel alat body 3D: Primitif (Box/Silinder/Bola/Kerucut), Cermin Body, dan
//! Putar Body. Hanya UI — app yang menghitung geometri lewat `ducad-engine`.

use ducad_i18n::t;
use egui::{DragValue, RichText, Ui, Vec2};

use crate::theme::{ACCENT_BLUE, TEXT_SECONDARY};

/// Jenis primitif solid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveKind {
    Box,
    Cylinder,
    Sphere,
    Cone,
    Torus,
}

impl PrimitiveKind {
    pub const ALL: [PrimitiveKind; 5] = [
        PrimitiveKind::Box,
        PrimitiveKind::Cylinder,
        PrimitiveKind::Sphere,
        PrimitiveKind::Cone,
        PrimitiveKind::Torus,
    ];

    pub fn label(self) -> String {
        match self {
            PrimitiveKind::Box => t!("prim-box"),
            PrimitiveKind::Cylinder => t!("prim-cylinder"),
            PrimitiveKind::Sphere => t!("prim-sphere"),
            PrimitiveKind::Cone => t!("prim-cone"),
            PrimitiveKind::Torus => t!("prim-torus"),
        }
    }
}

/// Bidang cermin body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirrorPlaneChoice {
    XY,
    XZ,
    YZ,
    /// Datum plane ke-`n` di dokumen.
    Datum(usize),
}

/// Sumbu putar body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotateAxisChoice {
    X,
    Y,
    Z,
}

impl RotateAxisChoice {
    pub fn vector(self) -> [f64; 3] {
        match self {
            RotateAxisChoice::X => [1.0, 0.0, 0.0],
            RotateAxisChoice::Y => [0.0, 1.0, 0.0],
            RotateAxisChoice::Z => [0.0, 0.0, 1.0],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BodyToolEvent {
    CreatePrimitive {
        kind: PrimitiveKind,
        /// Box: [x, y, z]; Silinder: [r, h, _]; Bola: [r, _, _]; Kerucut: [r1, r2, h].
        dims: [f64; 3],
        at: [f64; 3],
        centered: bool,
    },
    MirrorBody {
        plane: MirrorPlaneChoice,
        /// Bidang digeser supaya melewati pusat body (bukan origin).
        through_center: bool,
        keep_original: bool,
    },
    RotateBody {
        axis: RotateAxisChoice,
        angle_deg: f64,
        /// Pivot di pusat body (bukan origin).
        about_center: bool,
        keep_original: bool,
    },
}

#[derive(Debug, Clone)]
pub struct PrimitivePopupState {
    pub kind: PrimitiveKind,
    pub box_size: [f64; 3],
    pub centered: bool,
    pub radius: f64,
    pub height: f64,
    pub radius_top: f64,
    /// Radius tabung torus (radius besar = `radius`).
    pub tube_radius: f64,
    pub at: [f64; 3],
}

impl Default for PrimitivePopupState {
    fn default() -> Self {
        Self {
            kind: PrimitiveKind::Box,
            box_size: [20.0, 20.0, 20.0],
            centered: false,
            radius: 10.0,
            height: 20.0,
            radius_top: 0.0,
            tube_radius: 3.0,
            at: [0.0; 3],
        }
    }
}

impl PrimitivePopupState {
    pub fn dims(&self) -> [f64; 3] {
        match self.kind {
            PrimitiveKind::Box => self.box_size,
            PrimitiveKind::Cylinder => [self.radius, self.height, 0.0],
            PrimitiveKind::Sphere => [self.radius, 0.0, 0.0],
            PrimitiveKind::Cone => [self.radius, self.radius_top, self.height],
            PrimitiveKind::Torus => [self.radius, self.tube_radius, 0.0],
        }
    }
}

#[derive(Debug, Clone)]
pub struct MirrorBodyPopupState {
    pub plane: MirrorPlaneChoice,
    pub through_center: bool,
    pub keep_original: bool,
}

impl Default for MirrorBodyPopupState {
    fn default() -> Self {
        Self { plane: MirrorPlaneChoice::YZ, through_center: false, keep_original: true }
    }
}

#[derive(Debug, Clone)]
pub struct RotateBodyPopupState {
    pub axis: RotateAxisChoice,
    pub angle_deg: f64,
    pub snap_15: bool,
    pub about_center: bool,
    pub keep_original: bool,
}

impl Default for RotateBodyPopupState {
    fn default() -> Self {
        Self {
            axis: RotateAxisChoice::Z,
            angle_deg: 90.0,
            snap_15: true,
            about_center: true,
            keep_original: false,
        }
    }
}

/// Bulatkan sudut ke kelipatan 15° (snap rotasi).
pub fn snap_angle_15(deg: f64) -> f64 {
    (deg / 15.0).round() * 15.0
}

fn mm_drag(ui: &mut Ui, v: &mut f64, min: f64) {
    ui.add(DragValue::new(v).speed(0.5).range(min..=10_000.0).suffix(" mm"));
}

fn apply_button(ui: &mut Ui, label: &str) -> bool {
    ui.add_sized(
        Vec2::new(ui.available_width(), 26.0),
        egui::Button::new(RichText::new(label).strong().color(egui::Color32::WHITE))
            .fill(ACCENT_BLUE),
    )
    .clicked()
}

fn field_label(ui: &mut Ui, text: String) {
    ui.label(RichText::new(text).size(10.5).color(TEXT_SECONDARY));
}

pub struct BodyToolsPopup;

impl BodyToolsPopup {
    pub fn show_primitive(ui: &mut Ui, state: &mut PrimitivePopupState) -> Option<BodyToolEvent> {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
        ui.horizontal_wrapped(|ui| {
            for kind in PrimitiveKind::ALL {
                ui.selectable_value(&mut state.kind, kind, RichText::new(kind.label()).size(11.0));
            }
        });
        ui.separator();
        egui::Grid::new("prim_dims_grid").num_columns(2).spacing([6.0, 4.0]).show(ui, |ui| {
            match state.kind {
                PrimitiveKind::Box => {
                    field_label(ui, t!("prim-width"));
                    mm_drag(ui, &mut state.box_size[0], 0.01);
                    ui.end_row();
                    field_label(ui, t!("prim-depth"));
                    mm_drag(ui, &mut state.box_size[1], 0.01);
                    ui.end_row();
                    field_label(ui, t!("prim-height"));
                    mm_drag(ui, &mut state.box_size[2], 0.01);
                    ui.end_row();
                    field_label(ui, t!("prim-centered"));
                    ui.checkbox(&mut state.centered, "");
                    ui.end_row();
                }
                PrimitiveKind::Cylinder => {
                    field_label(ui, t!("prim-radius"));
                    mm_drag(ui, &mut state.radius, 0.01);
                    ui.end_row();
                    field_label(ui, t!("prim-height"));
                    mm_drag(ui, &mut state.height, 0.01);
                    ui.end_row();
                }
                PrimitiveKind::Sphere => {
                    field_label(ui, t!("prim-radius"));
                    mm_drag(ui, &mut state.radius, 0.01);
                    ui.end_row();
                }
                PrimitiveKind::Cone => {
                    field_label(ui, t!("prim-radius-bottom"));
                    mm_drag(ui, &mut state.radius, 0.0);
                    ui.end_row();
                    field_label(ui, t!("prim-radius-top"));
                    mm_drag(ui, &mut state.radius_top, 0.0);
                    ui.end_row();
                    field_label(ui, t!("prim-height"));
                    mm_drag(ui, &mut state.height, 0.01);
                    ui.end_row();
                }
                PrimitiveKind::Torus => {
                    field_label(ui, t!("prim-torus-major"));
                    mm_drag(ui, &mut state.radius, 0.01);
                    ui.end_row();
                    field_label(ui, t!("prim-torus-minor"));
                    mm_drag(ui, &mut state.tube_radius, 0.01);
                    ui.end_row();
                }
            }
            field_label(ui, t!("prim-position"));
            ui.horizontal(|ui| {
                for v in state.at.iter_mut() {
                    ui.add(DragValue::new(v).speed(0.5));
                }
            });
            ui.end_row();
        });
        ui.add_space(4.0);
        apply_button(ui, &t!("prim-create")).then_some(BodyToolEvent::CreatePrimitive {
            kind: state.kind,
            dims: state.dims(),
            at: state.at,
            centered: state.centered,
        })
    }

    /// `datums`: nama datum plane yang ada di dokumen.
    pub fn show_mirror(
        ui: &mut Ui,
        state: &mut MirrorBodyPopupState,
        datums: &[String],
    ) -> Option<BodyToolEvent> {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
        field_label(ui, t!("mirror-body-plane"));
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut state.plane, MirrorPlaneChoice::XY, "XY");
            ui.selectable_value(&mut state.plane, MirrorPlaneChoice::XZ, "XZ");
            ui.selectable_value(&mut state.plane, MirrorPlaneChoice::YZ, "YZ");
            for (i, name) in datums.iter().enumerate() {
                ui.selectable_value(&mut state.plane, MirrorPlaneChoice::Datum(i), name.as_str());
            }
        });
        if !matches!(state.plane, MirrorPlaneChoice::Datum(_)) {
            ui.checkbox(&mut state.through_center, t!("mirror-body-through-center"));
        }
        ui.checkbox(&mut state.keep_original, t!("body-tool-keep-original"));
        ui.add_space(4.0);
        apply_button(ui, &t!("mirror-body-apply")).then_some(BodyToolEvent::MirrorBody {
            plane: state.plane,
            through_center: state.through_center,
            keep_original: state.keep_original,
        })
    }

    pub fn show_rotate(ui: &mut Ui, state: &mut RotateBodyPopupState) -> Option<BodyToolEvent> {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
        field_label(ui, t!("rotate-body-axis"));
        ui.horizontal(|ui| {
            ui.selectable_value(&mut state.axis, RotateAxisChoice::X, "X");
            ui.selectable_value(&mut state.axis, RotateAxisChoice::Y, "Y");
            ui.selectable_value(&mut state.axis, RotateAxisChoice::Z, "Z");
        });
        ui.horizontal(|ui| {
            field_label(ui, t!("rotate-body-angle"));
            let speed = if state.snap_15 { 3.0 } else { 0.5 };
            ui.add(
                DragValue::new(&mut state.angle_deg)
                    .speed(speed)
                    .range(-360.0..=360.0)
                    .suffix(" deg"),
            );
        });
        ui.horizontal_wrapped(|ui| {
            for d in [-90.0, -15.0, 15.0, 45.0, 90.0, 180.0] {
                if ui.small_button(format!("{d:+}")).clicked() {
                    state.angle_deg = d;
                }
            }
        });
        ui.checkbox(&mut state.snap_15, t!("rotate-body-snap"));
        ui.checkbox(&mut state.about_center, t!("rotate-body-about-center"));
        ui.checkbox(&mut state.keep_original, t!("body-tool-keep-original"));
        if state.snap_15 {
            state.angle_deg = snap_angle_15(state.angle_deg);
        }
        ui.add_space(4.0);
        apply_button(ui, &t!("rotate-body-apply")).then_some(BodyToolEvent::RotateBody {
            axis: state.axis,
            angle_deg: state.angle_deg,
            about_center: state.about_center,
            keep_original: state.keep_original,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_rounds_to_nearest_15() {
        assert_eq!(snap_angle_15(7.4), 0.0);
        assert_eq!(snap_angle_15(7.6), 15.0);
        assert_eq!(snap_angle_15(-50.0), -45.0);
        assert_eq!(snap_angle_15(90.0), 90.0);
    }

    #[test]
    fn primitive_dims_follow_kind() {
        let mut s = PrimitivePopupState {
            kind: PrimitiveKind::Cone,
            radius: 5.0,
            radius_top: 2.0,
            height: 7.0,
            ..Default::default()
        };
        assert_eq!(s.dims(), [5.0, 2.0, 7.0]);
        s.kind = PrimitiveKind::Sphere;
        assert_eq!(s.dims()[0], 5.0);
    }
}
