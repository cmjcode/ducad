//! Glyph constraint di kanvas (Paritas 1.18).
//!
//! Ikon kecil di dekat setiap entitas yang ber-constraint, satu per jenis
//! constraint. Ikon diambil dari `ConstraintAction::icon` (font ikon
//! `egui_icons`, BUKAN glyph Unicode mentah) supaya sama dengan tombol di
//! baris constraint. Klik glyph → constraint itu terpilih (seleksi entitas
//! dikosongkan); Delete/Backspace → constraint dihapus (satu langkah undo).
//! Tampil/sembunyi lewat tombol di baris constraint atau command palette.
//!
//! Dimensi (Distance/Radius/Angle) tidak diberi glyph — sudah punya pill
//! dimensi sendiri.

use ducad_sketch::constraint::{point_ref_position, Constraint, PointRef};
use ducad_sketch::{EntityId, RemoveConstraint, Sketch};
use ducad_ui::ConstraintAction;
use glam::DVec2;

use crate::app::DuCADApp;
use crate::viewport::world_to_screen_pos;

/// Sisi kotak glyph (px).
const GLYPH_SIZE: f32 = 16.0;
/// Geser glyph dari titik jangkar (px) supaya tidak menutupi garis.
const GLYPH_OFFSET: egui::Vec2 = egui::vec2(12.0, -12.0);

#[derive(Debug, Clone, Copy)]
pub struct GlyphState {
    /// Glyph ditampilkan (default menyala).
    pub show: bool,
    /// Indeks constraint (di `Sketch::constraints`) yang sedang dipilih.
    pub selected: Option<usize>,
}

impl Default for GlyphState {
    fn default() -> Self {
        Self { show: true, selected: None }
    }
}

/// Satu glyph: constraint ke-`index`, digambar di `anchor` (bidang sketsa).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphSpec {
    pub index: usize,
    pub anchor: DVec2,
    pub action: ConstraintAction,
}

/// Jenis glyph (ikon) untuk constraint; `None` = tanpa glyph (dimensi).
pub fn glyph_action(c: &Constraint) -> Option<ConstraintAction> {
    use ConstraintAction as A;
    Some(match c {
        Constraint::Coincident { .. } => A::ApplyCoincident,
        Constraint::Horizontal { .. } | Constraint::HorizontalPoints { .. } => A::ApplyHorizontal,
        Constraint::Vertical { .. } | Constraint::VerticalPoints { .. } => A::ApplyVertical,
        Constraint::Parallel { .. } => A::ApplyParallel,
        Constraint::Perpendicular { .. } => A::ApplyPerpendicular,
        Constraint::EqualLength { .. } => A::ApplyEqualLength,
        Constraint::EqualRadius { .. } => A::ApplyEqualRadius,
        Constraint::Fixed { .. } => A::ApplyFixed,
        Constraint::Tangent { .. } => A::ApplyTangent,
        Constraint::Symmetric { .. } => A::ApplySymmetric,
        Constraint::PointOnCurve { .. } => A::ApplyPointOnCurve,
        Constraint::Midpoint { .. } => A::ApplyMidpoint,
        Constraint::Concentric { .. } => A::ApplyConcentric,
        Constraint::Collinear { .. } => A::ApplyCollinear,
        Constraint::Curvature { .. } => A::ApplyCurvature,
        _ => return None,
    })
}

/// Titik jangkar glyph untuk satu entitas: titik tengah, pusat, atau pusat bbox.
fn entity_anchor(sk: &Sketch, id: EntityId) -> Option<DVec2> {
    let e = sk.entities.get(id)?;
    e.midpoint()
        .or_else(|| e.center())
        .or_else(|| e.bounding_box().map(|(a, b)| (a + b) * 0.5))
        .filter(|p| p.is_finite())
}

fn point_anchor(sk: &Sketch, p: &PointRef) -> Option<DVec2> {
    point_ref_position(sk, p).filter(|p| p.is_finite())
}

/// Semua glyph sketsa: constraint antar-dua-entitas mendapat glyph di
/// kedua entitasnya; entitas tersembunyi dilewati.
pub fn glyph_specs(sk: &Sketch) -> Vec<GlyphSpec> {
    let mut out = Vec::new();
    for (index, c) in sk.constraints.iter().enumerate() {
        let Some(action) = glyph_action(c) else { continue };
        let anchors: Vec<Option<DVec2>> = match c {
            Constraint::Coincident { a, .. } | Constraint::Curvature { a, .. } => vec![point_anchor(sk, a)],
            Constraint::Fixed { point, .. }
            | Constraint::Midpoint { point, .. }
            | Constraint::PointOnCurve { point, .. } => vec![point_anchor(sk, point)],
            Constraint::HorizontalPoints { a, b } | Constraint::VerticalPoints { a, b } => {
                vec![point_anchor(sk, a).zip(point_anchor(sk, b)).map(|(p, q)| (p + q) * 0.5)]
            }
            Constraint::Horizontal { line } | Constraint::Vertical { line } => vec![entity_anchor(sk, *line)],
            Constraint::Symmetric { axis, .. } => vec![entity_anchor(sk, *axis)],
            Constraint::Parallel { a, b }
            | Constraint::Perpendicular { a, b }
            | Constraint::EqualLength { a, b }
            | Constraint::EqualRadius { a, b }
            | Constraint::Tangent { a, b }
            | Constraint::Concentric { a, b }
            | Constraint::Collinear { a, b } => {
                [*a, *b].iter().filter(|id| !sk.is_hidden(**id)).map(|id| entity_anchor(sk, *id)).collect()
            }
            _ => Vec::new(),
        };
        for anchor in anchors.into_iter().flatten() {
            out.push(GlyphSpec { index, anchor, action });
        }
    }
    out
}

impl DuCADApp {
    /// Perintah palette / tombol baris constraint: tampil/sembunyi glyph.
    pub fn toggle_constraint_glyphs(&mut self) {
        self.glyphs.show = !self.glyphs.show;
        if !self.glyphs.show {
            self.glyphs.selected = None;
        }
        self.model_status = Some(if self.glyphs.show {
            ducad_i18n::t!("glyphs-shown")
        } else {
            ducad_i18n::t!("glyphs-hidden")
        });
    }

    /// Hapus constraint glyph terpilih (satu langkah undo).
    pub fn delete_selected_constraint_glyph(&mut self) -> bool {
        let Some(index) = self.glyphs.selected.take() else { return false };
        if index >= self.sketch().constraints.len() {
            return false;
        }
        self.execute_sketch_command(Box::new(RemoveConstraint::new(index)));
        self.constraint_status = Some(ducad_i18n::t!("glyph-constraint-removed"));
        true
    }

    /// Gambar glyph + tangani klik dan tombol Delete. Dipanggil tiap frame
    /// dari overlay kanvas saat mode sketsa.
    pub fn show_constraint_glyphs(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        if !self.is_sketching || !self.glyphs.show {
            return;
        }
        // Indeks basi (constraint terhapus lewat undo/aksi lain) dibuang.
        if self.glyphs.selected.is_some_and(|i| i >= self.sketch().constraints.len()) {
            self.glyphs.selected = None;
        }
        let specs = glyph_specs(self.sketch());
        if specs.is_empty() {
            return;
        }
        let painter = ui.painter_at(rect);
        let mut placed: Vec<(egui::Pos2, u32)> = Vec::new();
        let mut clicked: Option<usize> = None;
        for (k, spec) in specs.iter().enumerate() {
            let world = self.active_plane.to_world(spec.anchor, 0.0);
            let Some(px) = world_to_screen_pos(&self.camera, rect, world) else { continue };
            // Glyph di jangkar yang sama ditumpuk ke kanan.
            let slot = match placed.iter_mut().find(|(p, _)| p.distance(px) < 4.0) {
                Some((_, n)) => {
                    *n += 1;
                    *n
                }
                None => {
                    placed.push((px, 0));
                    0
                }
            };
            let center = px + GLYPH_OFFSET + egui::vec2(slot as f32 * (GLYPH_SIZE + 2.0), 0.0);
            let r = egui::Rect::from_center_size(center, egui::vec2(GLYPH_SIZE, GLYPH_SIZE));
            if !rect.contains_rect(r) {
                continue;
            }
            let resp = ui
                .interact(r, ui.id().with(("ducad-cglyph", spec.index, k)), egui::Sense::click())
                .on_hover_text(spec.action.tooltip());
            if resp.clicked() {
                clicked = Some(spec.index);
            }
            let is_sel = self.glyphs.selected == Some(spec.index);
            let (bg, fg) = if is_sel {
                (ducad_ui::theme::ACCENT_BLUE, egui::Color32::WHITE)
            } else if resp.hovered() {
                (egui::Color32::from_rgb(255, 214, 102), egui::Color32::BLACK)
            } else {
                (egui::Color32::from_rgba_unmultiplied(40, 40, 46, 220), egui::Color32::from_rgb(255, 214, 102))
            };
            painter.rect_filled(r, 3.0, bg);
            painter.text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                spec.action.icon(),
                egui::FontId::proportional(GLYPH_SIZE - 4.0),
                fg,
            );
        }
        if let Some(i) = clicked {
            self.glyphs.selected = Some(i);
            self.selected.clear();
        }
        let text_focused = ui.ctx().memory(|m| m.focused().is_some());
        if self.glyphs.selected.is_some()
            && !text_focused
            && ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
        {
            self.delete_selected_constraint_glyph();
        }
        if self.glyphs.selected.is_some() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.glyphs.selected = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_sketch::Entity;

    #[test]
    fn specs_cover_geometric_constraints_but_not_dimensions() {
        let mut sk = Sketch::default();
        let a = sk.entities.insert(Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0)));
        let b = sk.entities.insert(Entity::line(DVec2::new(0.0, 5.0), DVec2::new(10.0, 5.0)));
        sk.constraints.push(Constraint::Horizontal { line: a });
        sk.constraints.push(Constraint::Parallel { a, b });
        sk.constraints.push(Constraint::Coincident { a: PointRef::LineEnd(a), b: PointRef::LineStart(b) });
        sk.constraints.push(Constraint::Radius { entity: a, value: 1.0, driven: false });
        let specs = glyph_specs(&sk);
        assert_eq!(specs.len(), 4, "{specs:?}");
        assert_eq!(specs[0].anchor, DVec2::new(5.0, 0.0));
        assert!(specs.iter().filter(|s| s.index == 1).count() == 2);
        assert!(specs.iter().all(|s| s.index != 3));
    }

    #[test]
    fn dangling_references_produce_no_glyph() {
        let mut sk = Sketch::default();
        let a = sk.entities.insert(Entity::line(DVec2::ZERO, DVec2::X));
        sk.entities.remove(a);
        sk.constraints.push(Constraint::Horizontal { line: a });
        sk.constraints.push(Constraint::Fixed { point: PointRef::LineStart(a), target: DVec2::ZERO });
        assert!(glyph_specs(&sk).is_empty());
    }

    #[test]
    fn delete_selected_glyph_removes_constraint_with_undo() {
        let mut app = DuCADApp::new_for_test();
        let a = app.sketch_mut().entities.insert(Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0)));
        app.sketch_mut().constraints.push(Constraint::Horizontal { line: a });
        assert!(!app.delete_selected_constraint_glyph());
        app.glyphs.selected = Some(7); // indeks basi
        assert!(!app.delete_selected_constraint_glyph());
        app.glyphs.selected = Some(0);
        assert!(app.delete_selected_constraint_glyph());
        assert!(app.sketch().constraints.is_empty());
        app.undo();
        assert_eq!(app.sketch().constraints.len(), 1);
        app.toggle_constraint_glyphs();
        assert!(!app.glyphs.show);
    }
}
