use cadraw_core::Command;
use glam::DVec2;

use crate::entity::{Entity, EntityId};
use crate::ops::translate_entity;
use crate::sketch::Sketch;

/// Alias nyaman: tumpukan undo/redo khusus operasi sketch.
pub type UndoStack = cadraw_core::UndoStack<Sketch>;

/// Sisipkan satu atau lebih entitas sebagai satu langkah undo.
pub struct InsertEntities {
    entities: Vec<Entity>,
    inserted_ids: Vec<EntityId>,
    label: &'static str,
}

impl InsertEntities {
    pub fn new(label: &'static str, entities: Vec<Entity>) -> Self {
        Self {
            entities,
            inserted_ids: Vec::new(),
            label,
        }
    }
}

impl Command<Sketch> for InsertEntities {
    fn name(&self) -> &str {
        self.label
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        self.inserted_ids = self
            .entities
            .iter()
            .cloned()
            .map(|e| sketch.entities.insert(e))
            .collect();
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        for id in self.inserted_ids.drain(..) {
            sketch.entities.remove(id);
        }
    }
}

/// Hapus entitas terpilih.
pub struct DeleteEntities {
    ids: Vec<EntityId>,
    removed: Vec<Entity>,
}

impl DeleteEntities {
    pub fn new(ids: Vec<EntityId>) -> Self {
        Self {
            ids,
            removed: Vec::new(),
        }
    }
}

impl Command<Sketch> for DeleteEntities {
    fn name(&self) -> &str {
        "Hapus"
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        self.removed = self
            .ids
            .iter()
            .filter_map(|id| sketch.entities.remove(*id))
            .collect();
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        for entity in self.removed.drain(..) {
            sketch.entities.insert(entity);
        }
    }
}

/// Hapus sekumpulan entitas dan sisipkan entitas baru sebagai satu langkah undo.
pub struct ReplaceEntities {
    label: &'static str,
    remove_ids: Vec<EntityId>,
    removed: Vec<Entity>,
    insert: Vec<Entity>,
    inserted_ids: Vec<EntityId>,
}

impl ReplaceEntities {
    pub fn new(label: &'static str, remove_ids: Vec<EntityId>, insert: Vec<Entity>) -> Self {
        Self {
            label,
            remove_ids,
            removed: Vec::new(),
            insert,
            inserted_ids: Vec::new(),
        }
    }
}

impl Command<Sketch> for ReplaceEntities {
    fn name(&self) -> &str {
        self.label
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        self.removed = self
            .remove_ids
            .iter()
            .filter_map(|id| sketch.entities.remove(*id))
            .collect();
        self.inserted_ids = self
            .insert
            .iter()
            .cloned()
            .map(|e| sketch.entities.insert(e))
            .collect();
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        for id in self.inserted_ids.drain(..) {
            sketch.entities.remove(id);
        }
        for entity in self.removed.drain(..) {
            sketch.entities.insert(entity);
        }
    }
}

/// Command untuk memodifikasi satu entitas di tempat (in-place) dengan mempertahankan `EntityId` yang sama.
pub struct UpdateEntity {
    label: &'static str,
    id: EntityId,
    old_entity: Option<Entity>,
    new_entity: Entity,
}

impl UpdateEntity {
    pub fn new(label: &'static str, id: EntityId, new_entity: Entity) -> Self {
        Self {
            label,
            id,
            old_entity: None,
            new_entity,
        }
    }
}

impl Command<Sketch> for UpdateEntity {
    fn name(&self) -> &str {
        self.label
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        if let Some(e) = sketch.entities.get_mut(self.id) {
            self.old_entity = Some(e.clone());
            *e = self.new_entity.clone();
        }
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        if let Some(old) = &self.old_entity {
            if let Some(e) = sketch.entities.get_mut(self.id) {
                *e = old.clone();
            }
        }
    }
}

/// Geser satu/lebih entitas sepanjang bidang sketsa-nya.
pub struct TranslateEntities {
    label: &'static str,
    ids: Vec<EntityId>,
    delta: DVec2,
}

impl TranslateEntities {
    pub fn new(label: &'static str, ids: Vec<EntityId>, delta: DVec2) -> Self {
        Self { label, ids, delta }
    }
}

impl Command<Sketch> for TranslateEntities {
    fn name(&self) -> &str {
        self.label
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        for id in &self.ids {
            if let Some(e) = sketch.entities.get_mut(*id) {
                *e = translate_entity(e, self.delta);
            }
        }
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        for id in &self.ids {
            if let Some(e) = sketch.entities.get_mut(*id) {
                *e = translate_entity(e, -self.delta);
            }
        }
    }
}
