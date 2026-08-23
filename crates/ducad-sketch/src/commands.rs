use ducad_core::Command;
use glam::DVec2;

use crate::entity::{Entity, EntityId};
use crate::ops::translate_entity;
use crate::sketch::Sketch;

/// Alias nyaman: tumpukan undo/redo khusus operasi sketch.
pub type UndoStack = ducad_core::UndoStack<Sketch>;

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

/// Ubah 4 garis pembentuk satu rectangle sekaligus (resize P/L via anchor) sebagai
/// satu langkah undo, bukan 4 langkah `UpdateEntity` terpisah.
pub struct ResizeRectangle {
    label: &'static str,
    new_lines: Vec<(EntityId, Entity)>,
    old_lines: Vec<(EntityId, Entity)>,
}

impl ResizeRectangle {
    pub fn new(label: &'static str, new_lines: Vec<(EntityId, Entity)>) -> Self {
        Self {
            label,
            new_lines,
            old_lines: Vec::new(),
        }
    }
}

impl Command<Sketch> for ResizeRectangle {
    fn name(&self) -> &str {
        self.label
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        self.old_lines.clear();
        for (id, new_entity) in &self.new_lines {
            if let Some(e) = sketch.entities.get_mut(*id) {
                self.old_lines.push((*id, e.clone()));
                *e = new_entity.clone();
            }
        }
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        for (id, old_entity) in self.old_lines.drain(..) {
            if let Some(e) = sketch.entities.get_mut(id) {
                *e = old_entity;
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

/// Beri nama grup pada sekumpulan entitas secara serentak (undoable).
///
/// Jika `new_name` kosong, entri nama untuk masing-masing entity dihapus
/// (entity kembali ke tampilan flat tanpa grup).
pub struct RenameEntities {
    ids: Vec<EntityId>,
    new_name: String,
    /// Nama lama per-entity sebelum command ini diaplikasikan (untuk revert).
    old_names: std::collections::HashMap<EntityId, Option<String>>,
}

impl RenameEntities {
    pub fn new(ids: Vec<EntityId>, new_name: impl Into<String>) -> Self {
        Self {
            ids,
            new_name: new_name.into(),
            old_names: std::collections::HashMap::new(),
        }
    }
}

impl Command<Sketch> for RenameEntities {
    fn name(&self) -> &str {
        "Beri Nama Grup"
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        self.old_names.clear();
        for &id in &self.ids {
            // Simpan nama lama untuk revert
            let old = sketch.entity_names.get(&id).cloned();
            self.old_names.insert(id, old);

            if self.new_name.is_empty() {
                sketch.entity_names.remove(&id);
            } else {
                sketch.entity_names.insert(id, self.new_name.clone());
            }
        }
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        for (&id, old_opt) in &self.old_names {
            match old_opt {
                Some(old_name) => {
                    sketch.entity_names.insert(id, old_name.clone());
                }
                None => {
                    sketch.entity_names.remove(&id);
                }
            }
        }
    }
}
