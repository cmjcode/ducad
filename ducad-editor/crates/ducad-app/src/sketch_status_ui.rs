//! Status kendala sketsa yang bisa ditindaklanjuti (Step 2
//! `docs/PLAN_DESIGNER_UX.md`): entitas yang sudah terkunci diberi warna
//! hijau, constraint berlebih/putus ditandai merah di glyph-nya, dan baris
//! status menyebut DOF serta jumlah entitas yang belum terkunci — bukan
//! hanya angka DOF.

use std::cell::RefCell;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use ducad_sketch::constraint::{sketch_status, ConstraintState, SketchStatus};
use ducad_sketch::{EntityId, Sketch};

use crate::app::DuCADApp;

/// Sketsa sebesar ini (mis. teks tervektorisasi) dilewati: analisisnya
/// terlalu mahal untuk dihitung ulang tiap perubahan.
const MAX_ENTITIES: usize = 1500;

/// Cache `(kunci isi sketsa, status)`; dihitung ulang hanya bila isi sketsa
/// berubah.
pub type StatusCache = RefCell<Option<(u64, Rc<SketchStatus>)>>;

fn sketch_key(sketch: &Sketch) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (id, e) in sketch.entities.iter() {
        id.hash(&mut h);
        format!("{e:?}").hash(&mut h);
    }
    format!("{:?}", sketch.constraints).hash(&mut h);
    h.finish()
}

impl DuCADApp {
    /// Status sketsa aktif selama mode sketsa CAD; `None` di luar itu atau
    /// untuk sketsa yang terlalu besar.
    pub fn active_sketch_status(&self) -> Option<Rc<SketchStatus>> {
        if !self.is_sketching || self.app_mode != crate::mode::AppMode::Sketch {
            return None;
        }
        let sketch = self.sketch();
        if sketch.entities.is_empty() || sketch.entities.len() > MAX_ENTITIES {
            return None;
        }
        let key = sketch_key(sketch);
        if let Some((k, st)) = self.sketch_status_cache.borrow().as_ref() {
            if *k == key {
                return Some(st.clone());
            }
        }
        let st = Rc::new(sketch_status(sketch));
        *self.sketch_status_cache.borrow_mut() = Some((key, st.clone()));
        Some(st)
    }

    /// Entitas yang sudah terkunci penuh (untuk pewarnaan viewport). `None`
    /// bila status tidak tersedia atau belum ada constraint sama sekali.
    pub fn locked_sketch_entities(&self) -> Option<HashSet<EntityId>> {
        let st = self.active_sketch_status()?;
        let sketch = self.sketch();
        if sketch.constraints.is_empty() {
            return None;
        }
        let free: HashSet<EntityId> = st.free_entities.iter().copied().collect();
        Some(
            sketch
                .entities
                .keys()
                .filter(|id| !free.contains(id))
                .collect(),
        )
    }

    /// Constraint indeks `i` redundan atau rujukannya putus.
    pub fn constraint_has_problem(&self, i: usize) -> bool {
        self.active_sketch_status()
            .is_some_and(|st| st.redundant.contains(&i) || st.unresolved.contains(&i))
    }

    /// Ringkasan status untuk baris status kanvas.
    pub fn sketch_status_text(&self) -> Option<String> {
        let st = self.active_sketch_status()?;
        Some(match st.state {
            ConstraintState::Fully => ducad_i18n::t!("sketch-status-fully"),
            ConstraintState::Under => {
                let dof = st.dof.to_string();
                let free = st.free_entities.len().to_string();
                ducad_i18n::t!(
                    "sketch-status-under",
                    dof = dof.as_str(),
                    free = free.as_str()
                )
            }
            ConstraintState::Over => {
                let count = (st.redundant.len() + st.unresolved.len()).to_string();
                ducad_i18n::t!("sketch-status-over", count = count.as_str())
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::app::DuCADApp;
    use ducad_sketch::constraint::{Constraint, PointRef};
    use ducad_sketch::Entity;
    use glam::DVec2;

    fn sketching_app() -> DuCADApp {
        let mut app = DuCADApp::new_for_test();
        app.set_app_mode(crate::mode::AppMode::Sketch);
        app.set_sketch_plane(ducad_render::PlaneKind::Top);
        app.enter_sketching();
        app
    }

    #[test]
    fn status_text_and_locked_set_follow_constraints() {
        let mut app = sketching_app();
        let a = app
            .sketch_mut()
            .entities
            .insert(Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0)));
        let b = app
            .sketch_mut()
            .entities
            .insert(Entity::line(DVec2::new(0.0, 5.0), DVec2::new(10.0, 5.0)));
        assert!(
            app.locked_sketch_entities().is_none(),
            "tanpa constraint: warna biasa"
        );
        let text = app.sketch_status_text().unwrap();
        assert!(text.contains('8'), "{text}");

        app.sketch_mut().constraints.push(Constraint::Fixed {
            point: PointRef::LineStart(a),
            target: DVec2::ZERO,
        });
        app.sketch_mut().constraints.push(Constraint::Fixed {
            point: PointRef::LineEnd(a),
            target: DVec2::new(10.0, 0.0),
        });
        let locked = app.locked_sketch_entities().unwrap();
        assert!(locked.contains(&a) && !locked.contains(&b));

        app.sketch_mut().entities.remove(b);
        assert_eq!(
            app.sketch_status_text().unwrap(),
            ducad_i18n::t!("sketch-status-fully")
        );

        app.sketch_mut()
            .constraints
            .push(Constraint::Horizontal { line: a });
        assert!(app.constraint_has_problem(2), "horizontal kedua berlebih");
        assert!(!app.constraint_has_problem(0));
    }

    #[test]
    fn no_status_outside_sketch_mode() {
        let mut app = sketching_app();
        app.sketch_mut()
            .entities
            .insert(Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0)));
        app.exit_sketching();
        assert!(app.sketch_status_text().is_none());
    }
}
