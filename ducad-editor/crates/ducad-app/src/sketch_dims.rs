//! Dimensi sketsa tambahan (Paritas 0.4): toggle R/Ø pada dimensi radius,
//! dimensi sudut antara dua garis terpilih, dan hotkey `D` yang memilih tipe
//! dimensi dari seleksi.

use ducad_sketch::constraint::Constraint;
use ducad_sketch::sketch::Sketch;
use ducad_sketch::{Entity, EntityId};
use ducad_ui::CanvasHud;
use glam::DVec2;
use slotmap::Key;

use crate::app::DuCADApp;
use crate::types::ToolKind;
use crate::viewport::world_to_screen_pos;

#[derive(Default)]
pub struct SketchDimState {
    /// Tampilkan diameter (Ø) alih-alih radius (R) pada lingkaran.
    pub show_diameter: bool,
    /// Pasangan garis yang dimensi sudutnya sedang diedit.
    pub angle_edit: Option<(EntityId, EntityId)>,
    pub angle_input: String,
}

fn line_ends(sk: &Sketch, id: EntityId) -> Option<(DVec2, DVec2)> {
    match sk.entities.get(id)? {
        Entity::Line { start, end, .. } => Some((*start, *end)),
        _ => None,
    }
}

/// Sudut bertanda (radian, CCW dari arah `a` ke arah `b`), konvensi sama
/// dengan `Constraint::Angle`.
pub fn signed_angle(sk: &Sketch, a: EntityId, b: EntityId) -> Option<f64> {
    let (sa, ea) = line_ends(sk, a)?;
    let (sb, eb) = line_ends(sk, b)?;
    let (da, db) = (ea - sa, eb - sb);
    if da.length() < 1e-12 || db.length() < 1e-12 {
        return None;
    }
    Some(da.perp_dot(db).atan2(da.dot(db)))
}

/// Titik label dimensi sudut: perpotongan dua garis bila dekat, selain itu
/// titik tengah antara kedua titik tengah garis.
fn angle_anchor(sk: &Sketch, a: EntityId, b: EntityId) -> Option<DVec2> {
    let (sa, ea) = line_ends(sk, a)?;
    let (sb, eb) = line_ends(sk, b)?;
    let fallback = ((sa + ea) * 0.5 + (sb + eb) * 0.5) * 0.5;
    let (da, db) = (ea - sa, eb - sb);
    let den = da.perp_dot(db);
    if den.abs() < 1e-9 {
        return Some(fallback);
    }
    let t = (sb - sa).perp_dot(db) / den;
    let p = sa + da * t;
    let span = da.length().max(db.length()) * 3.0;
    Some(if (p - fallback).length() <= span { p } else { fallback })
}

/// Constraint sudut dengan besar `deg` (derajat, tak bertanda) yang
/// mempertahankan arah putar saat ini.
pub fn angle_constraint(sk: &Sketch, a: EntityId, b: EntityId, deg: f64) -> Option<Constraint> {
    if !deg.is_finite() {
        return None;
    }
    let cur = signed_angle(sk, a, b)?;
    let sign = if cur < 0.0 { -1.0 } else { 1.0 };
    Some(Constraint::Angle { a, b, value: sign * deg.abs().to_radians() })
}

impl DuCADApp {
    fn two_selected_lines(&self) -> Option<(EntityId, EntityId)> {
        let mut ids: Vec<EntityId> = self.selected.iter().copied().collect();
        ids.sort();
        match ids.as_slice() {
            [a, b] if line_ends(self.sketch(), *a).is_some() && line_ends(self.sketch(), *b).is_some() => {
                Some((*a, *b))
            }
            _ => None,
        }
    }

    /// Terapkan dimensi sudut: perbarui constraint `Angle` yang sudah ada
    /// pada pasangan ini, atau tambahkan yang baru.
    pub fn commit_angle_dimension(&mut self, a: EntityId, b: EntityId, deg: f64) {
        let Some(c) = angle_constraint(self.sketch(), a, b, deg) else {
            self.constraint_status = Some(ducad_i18n::t!("constraint-not-applicable"));
            return;
        };
        let existing = self.sketch().constraints.iter().position(|k| {
            matches!(k, Constraint::Angle { a: x, b: y, .. } if (*x == a && *y == b) || (*x == b && *y == a))
        });
        match existing {
            Some(i) => {
                // Pertahankan urutan a/b milik constraint lama.
                let c = match (&self.sketch().constraints[i], c) {
                    (Constraint::Angle { a: x, .. }, Constraint::Angle { value, .. }) if *x == b => {
                        Constraint::Angle { a: b, b: a, value: -value }
                    }
                    (_, c) => c,
                };
                self.update_constraint(i, c)
            }
            None => self.apply_constraint(c),
        }
    }

    /// Ubah radius lingkaran/busur lewat constraint `Radius` bila sudah ada
    /// (dimensi penggerak). `true` bila ditangani di sini.
    pub fn commit_radius_via_constraint(&mut self, id: EntityId, radius: f64) -> bool {
        let idx = self
            .sketch()
            .constraints
            .iter()
            .position(|c| matches!(c, Constraint::Radius { entity, .. } if *entity == id));
        match idx {
            Some(i) => {
                self.update_constraint(i, Constraint::Radius { entity: id, value: radius });
                true
            }
            None => false,
        }
    }

    /// Hotkey `D`: pilih jenis dimensi dari seleksi.
    /// 1 lingkaran/busur → edit radius; 1 garis → kunci panjang;
    /// 2 garis → edit sudut.
    pub fn dimension_hotkey(&mut self) {
        if let Some((a, b)) = self.two_selected_lines() {
            let deg = signed_angle(self.sketch(), a, b).map(|r| r.to_degrees().abs()).unwrap_or(0.0);
            self.sketch_dims.angle_edit = Some((a, b));
            self.sketch_dims.angle_input = format!("{deg:.2}");
            return;
        }
        let ids: Vec<EntityId> = self.selected.iter().copied().collect();
        let [id] = ids.as_slice() else {
            self.constraint_status = Some(ducad_i18n::t!("dim-hotkey-hint"));
            return;
        };
        match self.sketch().entities.get(*id) {
            Some(Entity::Circle { radius, .. } | Entity::Arc { radius, .. }) => {
                let shown = if self.sketch_dims.show_diameter { radius * 2.0 } else { *radius };
                self.editing_dimension_entity = Some(*id);
                self.editing_dimension_input = format!("{:.2}", self.unit.to_display_val(shown));
            }
            Some(Entity::Line { .. }) => {
                self.apply_sketch_strip_action(ducad_ui::ConstraintAction::ApplyDistance)
            }
            _ => self.constraint_status = Some(ducad_i18n::t!("dim-hotkey-hint")),
        }
    }

    /// Pill dimensi sudut untuk dua garis terpilih (mode sketsa, tool Pilih).
    pub fn render_sketch_angle_dimension(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        if !self.is_sketching || self.tool != ToolKind::Select {
            self.sketch_dims.angle_edit = None;
            return;
        }
        let Some((a, b)) = self.two_selected_lines() else {
            self.sketch_dims.angle_edit = None;
            return;
        };
        let (Some(cur), Some(anchor)) = (signed_angle(self.sketch(), a, b), angle_anchor(self.sketch(), a, b)) else {
            return;
        };
        let Some(pos) = world_to_screen_pos(&self.camera, rect, self.active_plane.to_world(anchor, 0.0)) else {
            return;
        };
        let pos = pos + egui::vec2(0.0, -22.0);
        let is_editing = self.sketch_dims.angle_edit == Some((a, b));
        let text = format!("{:.1} deg", cur.to_degrees().abs());
        let resp = ui
            .push_id(("ducad-dim-pill-angle", a.data().as_ffi(), b.data().as_ffi()), |ui| {
                CanvasHud::render_interactive_dimension_pill_aligned(ui, pos, 0.0, &text, is_editing)
            })
            .inner;
        if resp.clicked() && !is_editing {
            self.sketch_dims.angle_edit = Some((a, b));
            self.sketch_dims.angle_input = format!("{:.2}", cur.to_degrees().abs());
        }
        if !is_editing {
            return;
        }
        let mut commit = None;
        egui::Area::new(egui::Id::new("ducad-dim-angle-edit-popup"))
            .fixed_pos(pos + egui::vec2(-50.0, 14.0))
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    let resp = ui.text_edit_singleline(&mut self.sketch_dims.angle_input);
                    if !resp.has_focus() {
                        resp.request_focus();
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.sketch_dims.angle_edit = None;
                    } else if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        commit = self.sketch_dims.angle_input.trim().parse::<f64>().ok();
                        self.sketch_dims.angle_edit = None;
                    }
                });
            });
        if let Some(deg) = commit {
            self.commit_angle_dimension(a, b, deg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_lines(a: (DVec2, DVec2), b: (DVec2, DVec2)) -> (DuCADApp, EntityId, EntityId) {
        let mut app = DuCADApp::new_for_test();
        let ia = app.sketch_mut().entities.insert(Entity::line(a.0, a.1));
        let ib = app.sketch_mut().entities.insert(Entity::line(b.0, b.1));
        (app, ia, ib)
    }

    #[test]
    fn signed_angle_matches_constraint_convention() {
        let (app, a, b) = app_with_lines((DVec2::ZERO, DVec2::X), (DVec2::ZERO, DVec2::new(1.0, 1.0)));
        let ang = signed_angle(app.sketch(), a, b).unwrap();
        assert!((ang.to_degrees() - 45.0).abs() < 1e-9);
        let ang = signed_angle(app.sketch(), b, a).unwrap();
        assert!((ang.to_degrees() + 45.0).abs() < 1e-9);
    }

    #[test]
    fn commit_angle_adds_then_updates_constraint() {
        let (mut app, a, b) = app_with_lines(
            (DVec2::ZERO, DVec2::new(10.0, 0.0)),
            (DVec2::ZERO, DVec2::new(7.0, 7.5)),
        );
        app.commit_angle_dimension(a, b, 60.0);
        assert_eq!(app.sketch().constraints.len(), 1);
        let got = signed_angle(app.sketch(), a, b).unwrap().to_degrees();
        assert!((got - 60.0).abs() < 1e-4, "{got}");
        app.commit_angle_dimension(a, b, 30.0);
        assert_eq!(app.sketch().constraints.len(), 1, "update, bukan tambah");
        let got = signed_angle(app.sketch(), a, b).unwrap().to_degrees();
        assert!((got - 30.0).abs() < 1e-4, "{got}");
    }

    #[test]
    fn bad_angle_input_is_rejected_without_change() {
        let (mut app, a, b) = app_with_lines((DVec2::ZERO, DVec2::X), (DVec2::ZERO, DVec2::Y));
        app.commit_angle_dimension(a, b, f64::NAN);
        assert!(app.sketch().constraints.is_empty());
    }

    #[test]
    fn hotkey_two_lines_opens_angle_edit() {
        let (mut app, a, b) = app_with_lines((DVec2::ZERO, DVec2::X), (DVec2::ZERO, DVec2::Y));
        app.selected = [a, b].into_iter().collect();
        app.dimension_hotkey();
        assert!(app.sketch_dims.angle_edit.is_some());
        assert_eq!(app.sketch_dims.angle_input, "90.00");
    }

    #[test]
    fn hotkey_circle_opens_radius_edit() {
        let mut app = DuCADApp::new_for_test();
        let id = app.sketch_mut().entities.insert(Entity::circle(DVec2::ZERO, 4.0));
        app.selected = [id].into_iter().collect();
        app.sketch_dims.show_diameter = true;
        app.dimension_hotkey();
        assert_eq!(app.editing_dimension_entity, Some(id));
        assert_eq!(app.editing_dimension_input, "8.00");
    }
}
