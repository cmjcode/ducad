//! Tool Break (Paritas 1.13) + dispatcher perintah palette Fase 1B
//! (Break, Project/Intersect/Break Link, Copy/Cut/Paste, glyph constraint,
//! Insert SVG).
//!
//! Geometri pemecahan ada di `ducad_sketch::break_at` (fungsi murni + command
//! undo); modul ini hanya hover/klik dan pesan status.

use ducad_sketch::{break_entity, BreakEntity, Entity, EntityId};
use glam::DVec2;

use crate::app::DuCADApp;
use crate::sketch_project::ProjectMode;
use crate::types::{ParityCmd, ToolKind};

/// Entitas yang bisa dipecah tool Break.
pub fn is_breakable(e: &Entity) -> bool {
    matches!(e, Entity::Line { .. } | Entity::Arc { .. } | Entity::Circle { .. } | Entity::Spline { .. })
}

impl DuCADApp {
    /// Hover/klik tool Break: entitas di bawah kursor dipecah di titik klik.
    pub fn handle_break_tool(&mut self, rect: egui::Rect, response: &egui::Response, tol: f64, raw: DVec2) {
        self.last_snap = None;
        self.hovered = response
            .hovered()
            .then(|| self.hit_test_hover(rect, response, tol))
            .flatten()
            .filter(|id| self.sketch().entities.get(*id).is_some_and(is_breakable));
        if response.clicked() {
            if let Some(id) = self.hovered {
                self.break_entity_at(id, raw);
                self.hovered = None;
            }
        }
    }

    /// Pecah entitas `id` di titik `p` (satu langkah undo). `false` bila
    /// entitas linked (read-only), tidak didukung, atau titik di ujung.
    pub fn break_entity_at(&mut self, id: EntityId, p: DVec2) -> bool {
        let sk = self.sketch();
        if sk.is_linked(id) {
            self.constraint_status = Some(ducad_i18n::t!("break-linked-readonly"));
            return false;
        }
        let Some(e) = sk.entities.get(id) else { return false };
        if break_entity(e, p).is_none() {
            self.constraint_status = Some(ducad_i18n::t!("break-not-possible"));
            return false;
        }
        self.execute_sketch_command(Box::new(BreakEntity::new(id, p)));
        self.selected.clear();
        self.constraint_status = Some(ducad_i18n::t!("break-done"));
        true
    }

    /// Perintah palette Fase 1B (dipanggil dari `run_parity_cmd`).
    pub fn run_parity_cmd_1b(&mut self, cmd: ParityCmd, ctx: &egui::Context) {
        match cmd {
            ParityCmd::SketchBreak => self.set_tool(ToolKind::Break),
            ParityCmd::ProjectEdges => self.project_to_sketch(ProjectMode::Project),
            ParityCmd::IntersectBody => self.project_to_sketch(ProjectMode::Intersect),
            ParityCmd::BreakLink => self.break_sketch_links(),
            ParityCmd::CopySketch => {
                self.sketch_copy(ctx);
            }
            ParityCmd::CutSketch => self.sketch_cut(ctx),
            ParityCmd::PasteSketch => self.sketch_paste(None, None),
            ParityCmd::ToggleGlyphs => self.toggle_constraint_glyphs(),
            ParityCmd::InsertSvg => self.open_svg_insert(),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_sketch::constraint::{Constraint, PointRef};

    #[test]
    fn break_splits_line_keeps_end_constraint_and_undoes() {
        let mut app = DuCADApp::new_for_test();
        let a = app.sketch_mut().entities.insert(Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0)));
        let b = app.sketch_mut().entities.insert(Entity::line(DVec2::new(10.0, 0.0), DVec2::new(10.0, 4.0)));
        app.sketch_mut().constraints.push(Constraint::Coincident {
            a: PointRef::LineEnd(a),
            b: PointRef::LineStart(b),
        });
        assert!(app.break_entity_at(a, DVec2::new(4.0, 0.1)));
        assert_eq!(app.sketch().entities.len(), 3);
        assert!(!matches!(app.sketch().constraints[0], Constraint::Coincident { a: PointRef::LineEnd(x), .. } if x == a));
        app.undo();
        assert_eq!(app.sketch().entities.len(), 2);
        assert_eq!(app.sketch().constraints.len(), 1);
        // Ujung / entitas tak dikenal / ellipse: tidak dipecah, tidak panik.
        assert!(!app.break_entity_at(a, DVec2::new(10.0, 0.0)));
        let e = app.sketch_mut().entities.insert(Entity::ellipse(DVec2::ZERO, 3.0, 2.0));
        assert!(!app.break_entity_at(e, DVec2::new(3.0, 0.0)));
        app.sketch_mut().entities.remove(e);
        assert!(!app.break_entity_at(e, DVec2::ZERO));
    }
}
