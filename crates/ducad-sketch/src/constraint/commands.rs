use ducad_core::Command;

use crate::constraint::solver::{involved_entities, solve};
use crate::constraint::types::Constraint;
use crate::entity::{Entity, EntityId};
use crate::sketch::Sketch;

/// Tambah satu constraint ke sketch dan langsung solve seluruh sistem.
pub struct AddConstraint {
    constraint: Constraint,
    prior_geometry: Vec<(EntityId, Entity)>,
}

impl AddConstraint {
    pub fn new(constraint: Constraint) -> Self {
        Self {
            constraint,
            prior_geometry: Vec::new(),
        }
    }
}

impl Command<Sketch> for AddConstraint {
    fn name(&self) -> &str {
        "Constraint"
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        self.prior_geometry = involved_entities(std::slice::from_ref(&self.constraint))
            .into_iter()
            .filter_map(|id| sketch.entities.get(id).map(|e| (id, e.clone())))
            .collect();
        sketch.constraints.push(self.constraint.clone());
        let snapshot = sketch.constraints.clone();
        solve(sketch, &snapshot);
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        sketch.constraints.pop();
        for (id, entity) in &self.prior_geometry {
            if let Some(slot) = sketch.entities.get_mut(*id) {
                *slot = entity.clone();
            }
        }
    }
}

/// Hapus constraint pada indeks tertentu dan solve ulang sisanya.
pub struct RemoveConstraint {
    index: usize,
    removed: Option<Constraint>,
    prior_geometry: Vec<(EntityId, Entity)>,
}

impl RemoveConstraint {
    pub fn new(index: usize) -> Self {
        Self {
            index,
            removed: None,
            prior_geometry: Vec::new(),
        }
    }
}

impl Command<Sketch> for RemoveConstraint {
    fn name(&self) -> &str {
        "Hapus Constraint"
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        if self.index >= sketch.constraints.len() {
            return;
        }
        self.prior_geometry = involved_entities(&sketch.constraints)
            .into_iter()
            .filter_map(|id| sketch.entities.get(id).map(|e| (id, e.clone())))
            .collect();
        self.removed = Some(sketch.constraints.remove(self.index));
        let snapshot = sketch.constraints.clone();
        solve(sketch, &snapshot);
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        if let Some(c) = self.removed.take() {
            let at = self.index.min(sketch.constraints.len());
            sketch.constraints.insert(at, c);
        }
        for (id, entity) in &self.prior_geometry {
            if let Some(slot) = sketch.entities.get_mut(*id) {
                *slot = entity.clone();
            }
        }
    }
}
