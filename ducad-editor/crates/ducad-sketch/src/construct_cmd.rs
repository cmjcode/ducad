//! Command sisip entitas + constraint otomatis sebagai SATU langkah undo
//! (Paritas 1.7–1.12): lingkaran 2/3-tangen, busur tangen, persegi bersudut
//! bulat, dan garis tak hingga.

use ducad_core::Command;

use crate::constraint::solver::{involved_entities, solve};
use crate::constraint::types::{Constraint, PointRef};
use crate::entity::{Entity, EntityId};
use crate::sketch::Sketch;

/// Rujukan entitas di template constraint: entitas baru (indeks dalam
/// daftar sisipan) atau entitas yang sudah ada di sketsa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewRef {
    New(usize),
    Existing(EntityId),
}

/// Constraint yang dipasang setelah entitas baru mendapat id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintTemplate {
    Tangent(NewRef, NewRef),
    EqualRadius(NewRef, NewRef),
    /// Ujung `a` berimpit ujung `b`; `bool` = `true` untuk ujung akhir
    /// (`PointRef::LineEnd`), `false` untuk ujung awal (`LineStart`).
    /// Berlaku untuk Line, Spline, dan Arc.
    CoincidentEnds(NewRef, bool, NewRef, bool),
}

/// Sisipkan entitas, tandai garis tak hingga, pasang constraint, lalu solve.
pub struct InsertConstructed {
    label: &'static str,
    entities: Vec<Entity>,
    constraints: Vec<ConstraintTemplate>,
    /// Indeks entitas baru yang ditandai garis tak hingga.
    infinite: Vec<usize>,
    inserted_ids: Vec<EntityId>,
    added_constraints: usize,
    prior_geometry: Vec<(EntityId, Entity)>,
}

impl InsertConstructed {
    pub fn new(label: &'static str, entities: Vec<Entity>) -> Self {
        Self {
            label,
            entities,
            constraints: Vec::new(),
            infinite: Vec::new(),
            inserted_ids: Vec::new(),
            added_constraints: 0,
            prior_geometry: Vec::new(),
        }
    }

    pub fn with_constraints(mut self, constraints: Vec<ConstraintTemplate>) -> Self {
        self.constraints = constraints;
        self
    }

    pub fn with_infinite(mut self, indices: Vec<usize>) -> Self {
        self.infinite = indices;
        self
    }

    /// Id entitas yang disisipkan pada `apply` terakhir.
    pub fn inserted_ids(&self) -> &[EntityId] {
        &self.inserted_ids
    }

    fn resolve(&self, r: NewRef) -> Option<EntityId> {
        match r {
            NewRef::New(i) => self.inserted_ids.get(i).copied(),
            NewRef::Existing(id) => Some(id),
        }
    }
}

impl Command<Sketch> for InsertConstructed {
    fn name(&self) -> &str {
        self.label
    }

    fn apply(&mut self, sketch: &mut Sketch) {
        self.inserted_ids = self
            .entities
            .iter()
            .cloned()
            .map(|e| {
                let id = sketch.entities.insert(e);
                sketch.touch(id);
                id
            })
            .collect();
        for &i in &self.infinite {
            if let Some(&id) = self.inserted_ids.get(i) {
                sketch.infinite_lines.insert(id);
            }
        }
        let resolved: Vec<Constraint> = self
            .constraints
            .iter()
            .filter_map(|t| match *t {
                ConstraintTemplate::Tangent(a, b) => {
                    Some(Constraint::Tangent { a: self.resolve(a)?, b: self.resolve(b)? })
                }
                ConstraintTemplate::EqualRadius(a, b) => {
                    Some(Constraint::EqualRadius { a: self.resolve(a)?, b: self.resolve(b)? })
                }
                ConstraintTemplate::CoincidentEnds(a, a_end, b, b_end) => {
                    let end = |id: EntityId, is_end: bool| {
                        if is_end {
                            PointRef::LineEnd(id)
                        } else {
                            PointRef::LineStart(id)
                        }
                    };
                    Some(Constraint::Coincident {
                        a: end(self.resolve(a)?, a_end),
                        b: end(self.resolve(b)?, b_end),
                    })
                }
            })
            .filter(|c| involved_entities(std::slice::from_ref(c)).iter().all(|id| sketch.entities.contains_key(*id)))
            .collect();
        self.added_constraints = resolved.len();
        if resolved.is_empty() {
            self.prior_geometry.clear();
            return;
        }
        self.prior_geometry = involved_entities(&sketch.constraints)
            .into_iter()
            .chain(involved_entities(&resolved))
            .filter(|id| !self.inserted_ids.contains(id))
            .filter_map(|id| sketch.entities.get(id).map(|e| (id, e.clone())))
            .collect();
        sketch.constraints.extend(resolved);
        let snapshot = sketch.constraints.clone();
        solve(sketch, &snapshot);
    }

    fn revert(&mut self, sketch: &mut Sketch) {
        let keep = sketch.constraints.len().saturating_sub(self.added_constraints);
        sketch.constraints.truncate(keep);
        for (id, entity) in self.prior_geometry.drain(..) {
            if let Some(slot) = sketch.entities.get_mut(id) {
                *slot = entity;
                sketch.touch(id);
            }
        }
        for id in self.inserted_ids.drain(..) {
            sketch.entities.remove(id);
            sketch.infinite_lines.remove(&id);
            sketch.touch(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::construct2d::{rounded_rect, rounded_rect_constraint_pairs};
    use glam::DVec2;

    #[test]
    fn rounded_rect_inserts_with_constraints_and_undoes_cleanly() {
        let mut sketch = Sketch::default();
        let ents = rounded_rect(DVec2::ZERO, DVec2::new(10.0, 6.0), 1.0, false).unwrap();
        let (tan, eq) = rounded_rect_constraint_pairs();
        let mut tpl: Vec<ConstraintTemplate> =
            tan.iter().map(|&(l, a)| ConstraintTemplate::Tangent(NewRef::New(l), NewRef::New(a))).collect();
        tpl.extend(eq.iter().map(|&(a, b)| ConstraintTemplate::EqualRadius(NewRef::New(a), NewRef::New(b))));
        let mut cmd = InsertConstructed::new("Persegi bulat", ents.clone()).with_constraints(tpl);
        cmd.apply(&mut sketch);
        assert_eq!(sketch.entities.len(), 8);
        assert_eq!(sketch.constraints.len(), 11);
        // Geometri sudah memenuhi constraint: solver tidak menggesernya.
        for (id, orig) in cmd.inserted_ids().iter().zip(&ents) {
            let now = sketch.entities.get(*id).unwrap();
            for (p, q) in now.endpoints().iter().zip(orig.endpoints()) {
                assert!((*p - q).length() < 1e-6);
            }
        }
        cmd.revert(&mut sketch);
        assert!(sketch.entities.is_empty() && sketch.constraints.is_empty());
    }

    #[test]
    fn infinite_flag_is_set_and_cleared() {
        let mut sketch = Sketch::default();
        let line = Entity::line(DVec2::ZERO, DVec2::X).with_construction(true);
        let mut cmd = InsertConstructed::new("Garis tak hingga", vec![line]).with_infinite(vec![0, 7]);
        cmd.apply(&mut sketch);
        let id = cmd.inserted_ids()[0];
        assert!(sketch.infinite_lines.contains(&id));
        assert_eq!(sketch.infinite_lines.len(), 1);
        cmd.revert(&mut sketch);
        assert!(sketch.infinite_lines.is_empty());
    }

    #[test]
    fn infinite_lines_round_trip_and_old_files_still_load() {
        let mut sketch = Sketch::default();
        let line = Entity::line(DVec2::ZERO, DVec2::X).with_construction(true);
        let mut cmd = InsertConstructed::new("Garis tak hingga", vec![line]).with_infinite(vec![0]);
        cmd.apply(&mut sketch);
        let json = serde_json::to_string(&sketch).unwrap();
        let back: Sketch = serde_json::from_str(&json).unwrap();
        let id = cmd.inserted_ids()[0];
        assert!(back.infinite_lines.contains(&id) && back.entities.contains_key(id));
        // Berkas lama tanpa field `infinite_lines` tetap terbaca.
        let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
        value.as_object_mut().unwrap().remove("infinite_lines");
        let old: Sketch = serde_json::from_value(value).unwrap();
        assert!(old.infinite_lines.is_empty() && old.entities.len() == 1);
    }

    #[test]
    fn dangling_template_refs_are_skipped() {
        let mut sketch = Sketch::default();
        let mut cmd = InsertConstructed::new("x", vec![Entity::circle(DVec2::ZERO, 1.0)])
            .with_constraints(vec![ConstraintTemplate::Tangent(NewRef::New(0), NewRef::New(5))]);
        cmd.apply(&mut sketch);
        assert!(sketch.constraints.is_empty());
        cmd.revert(&mut sketch);
        assert!(sketch.entities.is_empty());
    }
}
