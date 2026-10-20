//! Tool Titik sketch (Paritas 1.1).
//!
//! Satu klik menaruh satu `Entity::Point`. Bila klik jatuh pada snap yang
//! berasal dari titik nyata entitas lain (ujung garis, pusat lingkaran,
//! titik lain), titik baru langsung diikat `Coincident` ke sana supaya ikut
//! bergerak saat geometri itu diubah solver.

use std::collections::HashSet;

use ducad_sketch::constraint::{Constraint, PointRef};
use ducad_sketch::{Entity, EntityId, InsertEntities};
use glam::DVec2;

use crate::app::DuCADApp;

impl DuCADApp {
    /// Taruh titik sketch di `raw` (atau di titik snap aktif).
    pub fn place_sketch_point(&mut self, raw: DVec2) {
        let snap = self.last_snap;
        let p = self.snapped_or(raw);
        if !p.is_finite() {
            return;
        }
        let before: HashSet<EntityId> = self.sketch().entities.keys().collect();
        self.execute_sketch_command(Box::new(InsertEntities::new(
            "Titik",
            vec![Entity::point(p).with_construction(self.construction_mode)],
        )));
        let Some(new_id) = self.sketch().entities.keys().find(|id| !before.contains(id)) else {
            return;
        };
        if let Some(src) = snap.and_then(|s| s.source) {
            if src.entity_id() != new_id {
                self.apply_constraint(Constraint::Coincident {
                    a: PointRef::Center(new_id),
                    b: src,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ToolKind;

    fn points(app: &DuCADApp) -> Vec<(EntityId, DVec2)> {
        app.sketch()
            .entities
            .iter()
            .filter_map(|(id, e)| match e {
                Entity::Point { pos, .. } => Some((id, *pos)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn point_tool_places_point_and_ignores_nan() {
        let mut app = DuCADApp::new_for_test();
        app.set_tool(ToolKind::Point);
        app.place_sketch_point(DVec2::new(3.0, 4.0));
        let pts = points(&app);
        assert_eq!(pts.len(), 1);
        assert_eq!(pts[0].1, DVec2::new(3.0, 4.0));
        assert!(app.sketch().constraints.is_empty(), "tanpa snap → tanpa constraint");
        app.place_sketch_point(DVec2::new(f64::NAN, 1.0));
        assert_eq!(points(&app).len(), 1, "NaN diabaikan");
    }

    #[test]
    fn point_snapped_to_line_end_gets_coincident() {
        let mut app = DuCADApp::new_for_test();
        let l = app
            .sketch_mut()
            .entities
            .insert(Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0)));
        app.set_tool(ToolKind::Point);
        app.last_snap = app.find_current_snap(DVec2::new(10.1, 0.1), 1.0, 0.0, None);
        app.place_sketch_point(DVec2::new(10.1, 0.1));
        let pts = points(&app);
        assert_eq!(pts.len(), 1);
        assert_eq!(pts[0].1, DVec2::new(10.0, 0.0));
        assert!(app.sketch().constraints.iter().any(|c| matches!(
            c,
            Constraint::Coincident { a: PointRef::Center(p), b: PointRef::LineEnd(x) }
                if *p == pts[0].0 && *x == l
        )));
    }
}
