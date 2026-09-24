use std::collections::{HashMap, HashSet};
use ducad_core::Command;
use glam::DVec2;
use slotmap::Key;

use crate::entity::{Entity, EntityId};
use crate::layer::{Group, GroupId, Layer, LayerId, Origin, TextId};
use crate::ops::translate_entity;
use crate::sketch::Sketch;
use crate::style::{Paint, StrokeStyle, Style};
use crate::text::{regenerate_text, TextObject, TextSpec};

/// Alias nyaman: tumpukan undo/redo khusus operasi sketch.
pub type UndoStack = ducad_core::UndoStack<Sketch>;

/// Sisipkan satu atau lebih entitas sebagai satu langkah undo.
pub struct InsertEntities {
    entities: Vec<Entity>,
    inserted_ids: Vec<EntityId>,
    label: &'static str,
    group_name: Option<String>,
}

impl InsertEntities {
    pub fn new(label: &'static str, entities: Vec<Entity>) -> Self {
        Self {
            entities,
            inserted_ids: Vec::new(),
            label,
            group_name: None,
        }
    }

    pub fn with_group(
        label: &'static str,
        entities: Vec<Entity>,
        group_name: impl Into<String>,
    ) -> Self {
        Self {
            entities,
            inserted_ids: Vec::new(),
            label,
            group_name: Some(group_name.into()),
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
            .map(|e| {
                let id = sketch.entities.insert(e);
                if let Some(ref g) = self.group_name {
                    sketch.entity_names.insert(id, g.clone());
                }
                sketch.touch(id);
                id
            })
            .collect();
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        for id in self.inserted_ids.drain(..) {
            sketch.entities.remove(id);
            sketch.entity_names.remove(&id);
            sketch.touch(id);
        }
    }
}

/// Snapshot data entitas yang dihapus untuk mengembalikan style, layer, group, origin, dan z-order saat revert.
#[derive(Clone)]
struct DeletedEntityData {
    entity: Entity,
    name: Option<String>,
    style: Option<Style>,
    layer: Option<LayerId>,
    group: Option<GroupId>,
    origin: Option<Origin>,
    z_index: Option<usize>,
    group_member_indices: Vec<(GroupId, usize)>,
}

fn remove_single_entity_data(sketch: &mut Sketch, id: EntityId) -> Option<DeletedEntityData> {
    let entity = sketch.entities.remove(id)?;
    let name = sketch.entity_names.remove(&id);
    let style = sketch.styles.remove(id);
    let layer = sketch.entity_layer.remove(id);
    let group = sketch.entity_group.remove(id);
    let origin = sketch.origin.remove(id);
    let z_index = sketch.z_order.iter().position(|&z| z == id);
    if let Some(idx) = z_index {
        sketch.z_order.remove(idx);
    }
    let mut group_member_indices = Vec::new();
    for (gid, grp) in &mut sketch.groups {
        if let Some(pos) = grp.members.iter().position(|&m| m == id) {
            grp.members.remove(pos);
            group_member_indices.push((gid, pos));
        }
    }
    for (_, txt) in &mut sketch.texts {
        txt.glyph_entities.retain(|&g| g != id);
    }
    sketch.touch(id);
    Some(DeletedEntityData {
        entity,
        name,
        style,
        layer,
        group,
        origin,
        z_index,
        group_member_indices,
    })
}

fn restore_single_entity_data(sketch: &mut Sketch, data: DeletedEntityData) -> EntityId {
    let new_id = sketch.entities.insert(data.entity);
    if let Some(name) = data.name {
        sketch.entity_names.insert(new_id, name);
    }
    if let Some(style) = data.style {
        sketch.styles.insert(new_id, style);
    }
    if let Some(layer) = data.layer {
        sketch.entity_layer.insert(new_id, layer);
    }
    if let Some(group) = data.group {
        sketch.entity_group.insert(new_id, group);
    }
    if let Some(origin) = data.origin {
        if let Origin::Text { text } = origin {
            if let Some(txt) = sketch.texts.get_mut(text) {
                if !txt.glyph_entities.contains(&new_id) {
                    txt.glyph_entities.push(new_id);
                }
            }
        }
        sketch.origin.insert(new_id, origin);
    }
    if let Some(idx) = data.z_index {
        let idx = idx.min(sketch.z_order.len());
        sketch.z_order.insert(idx, new_id);
    } else {
        sketch.z_order.push(new_id);
    }
    for (gid, pos) in data.group_member_indices {
        if let Some(grp) = sketch.groups.get_mut(gid) {
            let pos = pos.min(grp.members.len());
            grp.members.insert(pos, new_id);
        }
    }
    sketch.touch(new_id);
    new_id
}

/// Hapus entitas terpilih.
pub struct DeleteEntities {
    ids: Vec<EntityId>,
    removed_data: Vec<DeletedEntityData>,
    restored_ids: Vec<EntityId>,
}

impl DeleteEntities {
    pub fn new(ids: Vec<EntityId>) -> Self {
        Self {
            ids,
            removed_data: Vec::new(),
            restored_ids: Vec::new(),
        }
    }

    pub fn restored_ids(&self) -> &[EntityId] {
        &self.restored_ids
    }
}

impl Command<Sketch> for DeleteEntities {
    fn name(&self) -> &str {
        "Hapus"
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        self.removed_data.clear();
        for &id in &self.ids {
            if let Some(data) = remove_single_entity_data(sketch, id) {
                self.removed_data.push(data);
            }
        }
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        self.restored_ids.clear();
        for data in self.removed_data.drain(..) {
            let new_id = restore_single_entity_data(sketch, data);
            self.restored_ids.push(new_id);
        }
        self.ids = self.restored_ids.clone();
    }
}

/// Hapus sekumpulan entitas dan sisipkan entitas baru sebagai satu langkah undo.
pub struct ReplaceEntities {
    label: &'static str,
    remove_ids: Vec<EntityId>,
    removed_data: Vec<DeletedEntityData>,
    insert: Vec<Entity>,
    insert_styles: Vec<Option<Style>>,
    insert_layers: Vec<Option<LayerId>>,
    inserted_ids: Vec<EntityId>,
}

impl ReplaceEntities {
    pub fn new(label: &'static str, remove_ids: Vec<EntityId>, insert: Vec<Entity>) -> Self {
        Self {
            label,
            remove_ids,
            removed_data: Vec::new(),
            insert,
            insert_styles: Vec::new(),
            insert_layers: Vec::new(),
            inserted_ids: Vec::new(),
        }
    }

    pub fn with_styles(mut self, styles: Vec<Option<Style>>) -> Self {
        self.insert_styles = styles;
        self
    }

    pub fn with_layers(mut self, layers: Vec<Option<LayerId>>) -> Self {
        self.insert_layers = layers;
        self
    }

    pub fn inserted_ids(&self) -> &[EntityId] {
        &self.inserted_ids
    }
}

impl Command<Sketch> for ReplaceEntities {
    fn name(&self) -> &str {
        self.label
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        self.removed_data.clear();
        for &id in &self.remove_ids {
            if let Some(entity) = sketch.entities.remove(id) {
                let name = sketch.entity_names.remove(&id);
                let style = sketch.styles.remove(id);
                let layer = sketch.entity_layer.remove(id);
                let group = sketch.entity_group.remove(id);
                let origin = sketch.origin.remove(id);
                let z_index = sketch.z_order.iter().position(|&z| z == id);
                if let Some(idx) = z_index {
                    sketch.z_order.remove(idx);
                }
                let mut group_member_indices = Vec::new();
                for (gid, grp) in &mut sketch.groups {
                    if let Some(pos) = grp.members.iter().position(|&m| m == id) {
                        grp.members.remove(pos);
                        group_member_indices.push((gid, pos));
                    }
                }
                sketch.touch(id);
                self.removed_data.push(DeletedEntityData {
                    entity,
                    name,
                    style,
                    layer,
                    group,
                    origin,
                    z_index,
                    group_member_indices,
                });
            }
        }
        self.inserted_ids = self
            .insert
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, e)| {
                let id = sketch.entities.insert(e);
                if let Some(Some(style)) = self.insert_styles.get(i) {
                    sketch.styles.insert(id, style.clone());
                }
                if let Some(Some(layer)) = self.insert_layers.get(i) {
                    sketch.entity_layer.insert(id, *layer);
                }
                sketch.touch(id);
                id
            })
            .collect();
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        for id in self.inserted_ids.drain(..) {
            sketch.entities.remove(id);
            sketch.styles.remove(id);
            sketch.entity_layer.remove(id);
            sketch.touch(id);
        }
        let mut restored = Vec::new();
        for data in self.removed_data.drain(..) {
            let new_id = sketch.entities.insert(data.entity);
            if let Some(name) = data.name {
                sketch.entity_names.insert(new_id, name);
            }
            if let Some(style) = data.style {
                sketch.styles.insert(new_id, style);
            }
            if let Some(layer) = data.layer {
                sketch.entity_layer.insert(new_id, layer);
            }
            if let Some(group) = data.group {
                sketch.entity_group.insert(new_id, group);
            }
            if let Some(origin) = data.origin {
                sketch.origin.insert(new_id, origin);
            }
            if let Some(idx) = data.z_index {
                let idx = idx.min(sketch.z_order.len());
                sketch.z_order.insert(idx, new_id);
            }
            for (gid, pos) in data.group_member_indices {
                if let Some(grp) = sketch.groups.get_mut(gid) {
                    let pos = pos.min(grp.members.len());
                    grp.members.insert(pos, new_id);
                }
            }
            sketch.touch(new_id);
            restored.push(new_id);
        }
        self.remove_ids = restored;
    }
}

/// Command untuk memodifikasi satu entitas di tempat (in-place) dengan mempertahankan `EntityId` yang sama.
pub struct UpdateEntity {
    label: &'static str,
    id: EntityId,
    old_entity: Option<Entity>,
    new_entity: Entity,
    coalesce_key: Option<(&'static str, u64)>,
}

impl UpdateEntity {
    pub fn new(label: &'static str, id: EntityId, new_entity: Entity) -> Self {
        Self {
            label,
            id,
            old_entity: None,
            new_entity,
            coalesce_key: None,
        }
    }

    /// Pasang kunci coalescing untuk penggabungan aksi kontinu (drag node/handle, slider, dll).
    pub fn with_coalesce_key(mut self, tag: &'static str, target_id: u64) -> Self {
        self.coalesce_key = Some((tag, target_id));
        self
    }
}

/// Helper pembentuk kunci coalescing untuk drag node atau handle vektor kontinu.
pub fn node_coalesce_key(id: EntityId, subpath: u16, node: u32) -> (&'static str, u64) {
    let raw = id.data().as_ffi();
    let packed = raw ^ ((subpath as u64) << 32) ^ (node as u64);
    ("node-drag", packed)
}

impl Command<Sketch> for UpdateEntity {
    fn name(&self) -> &str {
        self.label
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        if let Some(e) = sketch.entities.get_mut(self.id) {
            self.old_entity = Some(e.clone());
            *e = self.new_entity.clone();
            sketch.touch(self.id);
        }
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        if let Some(old) = &self.old_entity {
            if let Some(e) = sketch.entities.get_mut(self.id) {
                *e = old.clone();
                sketch.touch(self.id);
            }
        }
    }
    fn coalesce_key(&self) -> Option<(&'static str, u64)> {
        self.coalesce_key
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
                sketch.touch(*id);
            }
        }
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        for (id, old_entity) in self.old_lines.drain(..) {
            if let Some(e) = sketch.entities.get_mut(id) {
                *e = old_entity;
                sketch.touch(id);
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
                sketch.touch(*id);
            }
        }
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        for id in &self.ids {
            if let Some(e) = sketch.entities.get_mut(*id) {
                *e = translate_entity(e, -self.delta);
                sketch.touch(*id);
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
            sketch.touch(id);
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
            sketch.touch(id);
        }
    }
}

/// Toggle atau set status garis konstruksi untuk sekumpulan entitas (undoable).
pub struct ToggleConstruction {
    label: &'static str,
    ids: Vec<EntityId>,
    target_state: bool,
    old_states: Vec<(EntityId, bool)>,
}

impl ToggleConstruction {
    pub fn new(ids: Vec<EntityId>, target_state: bool) -> Self {
        Self {
            label: "Garis Konstruksi",
            ids,
            target_state,
            old_states: Vec::new(),
        }
    }
}

impl Command<Sketch> for ToggleConstruction {
    fn name(&self) -> &str {
        self.label
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        self.old_states.clear();
        for &id in &self.ids {
            if let Some(e) = sketch.entities.get_mut(id) {
                self.old_states.push((id, e.is_construction()));
                e.set_construction(self.target_state);
                sketch.touch(id);
            }
        }
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        for &(id, old_state) in &self.old_states {
            if let Some(e) = sketch.entities.get_mut(id) {
                e.set_construction(old_state);
                sketch.touch(id);
            }
        }
    }
}

/// Atur gaya visual (fill, stroke, opacity, dll.) untuk satu atau lebih entitas.
pub struct SetStyle {
    ids: Vec<EntityId>,
    style: Style,
    old_styles: HashMap<EntityId, Option<Style>>,
}

impl SetStyle {
    pub fn new(ids: Vec<EntityId>, style: Style) -> Self {
        Self {
            ids,
            style,
            old_styles: HashMap::new(),
        }
    }
}

impl Command<Sketch> for SetStyle {
    fn name(&self) -> &str {
        "Ubah Style"
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        self.old_styles.clear();
        for &id in &self.ids {
            let old = sketch.styles.get(id).cloned();
            self.old_styles.insert(id, old);
            sketch.styles.insert(id, self.style.clone());
            sketch.touch(id);
        }
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        for (&id, old_opt) in &self.old_styles {
            match old_opt {
                Some(style) => {
                    sketch.styles.insert(id, style.clone());
                }
                None => {
                    sketch.styles.remove(id);
                }
            }
            sketch.touch(id);
        }
    }
}

/// Bagian gaya visual individual untuk drag slider kontinu (coalesced).
#[derive(Debug, Clone, PartialEq)]
pub enum StyleField {
    Fill(Option<Paint>),
    Stroke(Option<StrokeStyle>),
    Opacity(f32),
}

/// Command untuk memodifikasi satu field style secara kontinu dengan coalesce_key.
pub struct SetStyleField {
    ids: Vec<EntityId>,
    field: StyleField,
    old_styles: HashMap<EntityId, Option<Style>>,
}

impl SetStyleField {
    pub fn new(ids: Vec<EntityId>, field: StyleField) -> Self {
        Self {
            ids,
            field,
            old_styles: HashMap::new(),
        }
    }
}

impl Command<Sketch> for SetStyleField {
    fn name(&self) -> &str {
        match self.field {
            StyleField::Fill(_) => "Ubah Fill",
            StyleField::Stroke(_) => "Ubah Stroke",
            StyleField::Opacity(_) => "Ubah Opacity",
        }
    }
    fn coalesce_key(&self) -> Option<(&'static str, u64)> {
        let tag = match self.field {
            StyleField::Fill(_) => "set_style_field_fill",
            StyleField::Stroke(_) => "set_style_field_stroke",
            StyleField::Opacity(_) => "set_style_field_opacity",
        };
        let target = self.ids.len() as u64;
        Some((tag, target))
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        self.old_styles.clear();
        for &id in &self.ids {
            let old = sketch.styles.get(id).cloned();
            self.old_styles.insert(id, old);
            let mut style = sketch.style_of(id);
            match &self.field {
                StyleField::Fill(fill) => style.fill = fill.clone(),
                StyleField::Stroke(stroke) => style.stroke = stroke.clone(),
                StyleField::Opacity(opacity) => style.opacity = *opacity,
            }
            sketch.styles.insert(id, style);
            sketch.touch(id);
        }
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        for (&id, old_opt) in &self.old_styles {
            match old_opt {
                Some(style) => {
                    sketch.styles.insert(id, style.clone());
                }
                None => {
                    sketch.styles.remove(id);
                }
            }
            sketch.touch(id);
        }
    }
}

/// Buat layer baru pada sketch.
pub struct CreateLayer {
    layer: Layer,
    created_id: Option<LayerId>,
}

impl CreateLayer {
    pub fn new(layer: Layer) -> Self {
        Self {
            layer,
            created_id: None,
        }
    }

    pub fn created_id(&self) -> Option<LayerId> {
        self.created_id
    }
}

impl Command<Sketch> for CreateLayer {
    fn name(&self) -> &str {
        "Buat Layer"
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        let lid = sketch.layers.insert(self.layer.clone());
        sketch.layer_order.push(lid);
        self.created_id = Some(lid);
        sketch.touch_all();
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        if let Some(lid) = self.created_id.take() {
            sketch.layers.remove(lid);
            sketch.layer_order.retain(|&x| x != lid);
            sketch.touch_all();
        }
    }
}

/// Hapus layer; seluruh entitas pada layer ini dipindahkan ke layer default ("Layer 1").
pub struct DeleteLayer {
    layer: LayerId,
    removed_layer: Option<Layer>,
    removed_order_idx: Option<usize>,
    moved_entities: Vec<EntityId>,
}

impl DeleteLayer {
    pub fn new(layer: LayerId) -> Self {
        Self {
            layer,
            removed_layer: None,
            removed_order_idx: None,
            moved_entities: Vec::new(),
        }
    }
}

impl Command<Sketch> for DeleteLayer {
    fn name(&self) -> &str {
        "Hapus Layer"
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        if !sketch.layers.contains_key(self.layer) {
            return;
        }
        let def = sketch.ensure_default_layer();
        let target_def = if def == self.layer {
            let new_def = sketch
                .layers
                .insert(Layer::new("Layer 1", crate::style::Rgba::WHITE));
            sketch.layer_order.push(new_def);
            new_def
        } else {
            def
        };

        self.moved_entities.clear();
        for (id, &lid) in &sketch.entity_layer {
            if lid == self.layer {
                self.moved_entities.push(id);
            }
        }
        for &id in &self.moved_entities {
            sketch.entity_layer.insert(id, target_def);
            sketch.touch(id);
        }

        if let Some(pos) = sketch.layer_order.iter().position(|&l| l == self.layer) {
            self.removed_order_idx = Some(pos);
            sketch.layer_order.remove(pos);
        }
        self.removed_layer = sketch.layers.remove(self.layer);
        sketch.touch_all();
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        if let Some(layer) = self.removed_layer.take() {
            let restored_lid = sketch.layers.insert(layer);
            if let Some(pos) = self.removed_order_idx {
                let pos = pos.min(sketch.layer_order.len());
                sketch.layer_order.insert(pos, restored_lid);
            } else {
                sketch.layer_order.push(restored_lid);
            }
            for &id in &self.moved_entities {
                sketch.entity_layer.insert(id, restored_lid);
                sketch.touch(id);
            }
            self.layer = restored_lid;
            sketch.touch_all();
        }
    }
}

/// Ganti nama layer.
pub struct RenameLayer {
    layer: LayerId,
    new_name: String,
    old_name: Option<String>,
}

impl RenameLayer {
    pub fn new(layer: LayerId, new_name: impl Into<String>) -> Self {
        Self {
            layer,
            new_name: new_name.into(),
            old_name: None,
        }
    }
}

impl Command<Sketch> for RenameLayer {
    fn name(&self) -> &str {
        "Ganti Nama Layer"
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        if let Some(layer) = sketch.layers.get_mut(self.layer) {
            self.old_name = Some(layer.name.clone());
            layer.name = self.new_name.clone();
            sketch.touch_all();
        }
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        if let Some(old) = self.old_name.take() {
            if let Some(layer) = sketch.layers.get_mut(self.layer) {
                layer.name = old;
                sketch.touch_all();
            }
        }
    }
}

/// Ubah urutan layer (bawah -> atas).
pub struct ReorderLayers {
    new_order: Vec<LayerId>,
    old_order: Vec<LayerId>,
}

impl ReorderLayers {
    pub fn new(new_order: Vec<LayerId>) -> Self {
        Self {
            new_order,
            old_order: Vec::new(),
        }
    }
}

impl Command<Sketch> for ReorderLayers {
    fn name(&self) -> &str {
        "Ubah Urutan Layer"
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        self.old_order = sketch.layer_order.clone();
        sketch.layer_order = self.new_order.clone();
        sketch.touch_all();
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        sketch.layer_order = self.old_order.clone();
        sketch.touch_all();
    }
}

/// Ubah visibilitas atau status penguncian layer.
pub struct SetLayerFlags {
    layer: LayerId,
    visible: Option<bool>,
    locked: Option<bool>,
    old_flags: Option<(bool, bool)>,
}

impl SetLayerFlags {
    pub fn new(layer: LayerId, visible: Option<bool>, locked: Option<bool>) -> Self {
        Self {
            layer,
            visible,
            locked,
            old_flags: None,
        }
    }
}

impl Command<Sketch> for SetLayerFlags {
    fn name(&self) -> &str {
        "Ubah Status Layer"
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        if let Some(l) = sketch.layers.get_mut(self.layer) {
            self.old_flags = Some((l.visible, l.locked));
            if let Some(v) = self.visible {
                l.visible = v;
            }
            if let Some(lock) = self.locked {
                l.locked = lock;
            }
            sketch.touch_all();
        }
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        if let Some((v, lock)) = self.old_flags.take() {
            if let Some(l) = sketch.layers.get_mut(self.layer) {
                l.visible = v;
                l.locked = lock;
                sketch.touch_all();
            }
        }
    }
}

/// Pindahkan satu atau lebih entitas ke layer target.
pub struct MoveToLayer {
    ids: Vec<EntityId>,
    layer: LayerId,
    old_layers: HashMap<EntityId, Option<LayerId>>,
}

impl MoveToLayer {
    pub fn new(ids: Vec<EntityId>, layer: LayerId) -> Self {
        Self {
            ids,
            layer,
            old_layers: HashMap::new(),
        }
    }
}

impl Command<Sketch> for MoveToLayer {
    fn name(&self) -> &str {
        "Pindah ke Layer"
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        self.old_layers.clear();
        for &id in &self.ids {
            let old = sketch.entity_layer.get(id).copied();
            self.old_layers.insert(id, old);
            sketch.entity_layer.insert(id, self.layer);
            sketch.touch(id);
        }
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        for (&id, old_opt) in &self.old_layers {
            match old_opt {
                Some(lid) => {
                    sketch.entity_layer.insert(id, *lid);
                }
                None => {
                    sketch.entity_layer.remove(id);
                }
            }
            sketch.touch(id);
        }
    }
}

/// Aksi manipulasi urutan kedalaman gambar entitas dalam layer (Z-Order).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZOrderAction {
    BringFront,
    SendBack,
    Forward,
    Backward,
}

/// Command untuk memanipulasi z-order entitas.
pub struct SetZOrder {
    ids: Vec<EntityId>,
    action: ZOrderAction,
    old_z_order: Vec<EntityId>,
}

impl SetZOrder {
    pub fn new(ids: Vec<EntityId>, action: ZOrderAction) -> Self {
        Self {
            ids,
            action,
            old_z_order: Vec::new(),
        }
    }
}

impl Command<Sketch> for SetZOrder {
    fn name(&self) -> &str {
        match self.action {
            ZOrderAction::BringFront => "Bawa ke Depan",
            ZOrderAction::SendBack => "Kirim ke Belakang",
            ZOrderAction::Forward => "Maju Selangkah",
            ZOrderAction::Backward => "Mundur Selangkah",
        }
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        self.old_z_order = sketch.z_order.clone();
        for &id in &self.ids {
            if !sketch.z_order.contains(&id) {
                sketch.z_order.push(id);
            }
        }
        let id_set: HashSet<EntityId> = self.ids.iter().copied().collect();
        match self.action {
            ZOrderAction::BringFront => {
                let mut moved = Vec::new();
                sketch.z_order.retain(|id| {
                    if id_set.contains(id) {
                        moved.push(*id);
                        false
                    } else {
                        true
                    }
                });
                sketch.z_order.extend(moved);
            }
            ZOrderAction::SendBack => {
                let mut moved = Vec::new();
                sketch.z_order.retain(|id| {
                    if id_set.contains(id) {
                        moved.push(*id);
                        false
                    } else {
                        true
                    }
                });
                let mut new_order = moved;
                new_order.append(&mut sketch.z_order);
                sketch.z_order = new_order;
            }
            ZOrderAction::Forward => {
                if sketch.z_order.len() > 1 {
                    for i in (0..(sketch.z_order.len() - 1)).rev() {
                        if id_set.contains(&sketch.z_order[i])
                            && !id_set.contains(&sketch.z_order[i + 1])
                        {
                            sketch.z_order.swap(i, i + 1);
                        }
                    }
                }
            }
            ZOrderAction::Backward => {
                for i in 1..sketch.z_order.len() {
                    if id_set.contains(&sketch.z_order[i])
                        && !id_set.contains(&sketch.z_order[i - 1])
                    {
                        sketch.z_order.swap(i, i - 1);
                    }
                }
            }
        }
        for &id in &self.ids {
            sketch.touch(id);
        }
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        sketch.z_order = self.old_z_order.clone();
        for &id in &self.ids {
            sketch.touch(id);
        }
    }
}

/// Kelompokkan sekumpulan entitas menjadi satu Group (undoable).
pub struct GroupEntities {
    ids: Vec<EntityId>,
    name: String,
    created_id: Option<GroupId>,
    old_groups: HashMap<EntityId, Option<GroupId>>,
}

impl GroupEntities {
    pub fn new(ids: Vec<EntityId>, name: impl Into<String>) -> Self {
        Self {
            ids,
            name: name.into(),
            created_id: None,
            old_groups: HashMap::new(),
        }
    }

    pub fn created_group_id(&self) -> Option<GroupId> {
        self.created_id
    }
}

impl Command<Sketch> for GroupEntities {
    fn name(&self) -> &str {
        "Grup Entitas"
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        self.old_groups.clear();
        let grp = Group {
            name: self.name.clone(),
            members: self.ids.clone(),
            parent: None,
        };
        let gid = sketch.groups.insert(grp);
        self.created_id = Some(gid);
        for &id in &self.ids {
            let old = sketch.entity_group.get(id).copied();
            self.old_groups.insert(id, old);
            sketch.entity_group.insert(id, gid);
            sketch.touch(id);
        }
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        if let Some(gid) = self.created_id.take() {
            sketch.groups.remove(gid);
        }
        for (&id, old_opt) in &self.old_groups {
            match old_opt {
                Some(old_gid) => {
                    sketch.entity_group.insert(id, *old_gid);
                }
                None => {
                    sketch.entity_group.remove(id);
                }
            }
            sketch.touch(id);
        }
    }
}

/// Lepaskan pengelompokan suatu Group (undoable).
pub struct Ungroup {
    group: GroupId,
    removed_group: Option<Group>,
    member_groups: HashMap<EntityId, Option<GroupId>>,
}

impl Ungroup {
    pub fn new(group: GroupId) -> Self {
        Self {
            group,
            removed_group: None,
            member_groups: HashMap::new(),
        }
    }
}

impl Command<Sketch> for Ungroup {
    fn name(&self) -> &str {
        "Pisahkan Grup"
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        if let Some(grp) = sketch.groups.remove(self.group) {
            self.member_groups.clear();
            for &mid in &grp.members {
                let old = sketch.entity_group.remove(mid);
                self.member_groups.insert(mid, old);
                sketch.touch(mid);
            }
            self.removed_group = Some(grp);
        }
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        if let Some(grp) = self.removed_group.take() {
            let restored_gid = sketch.groups.insert(grp);
            for &mid in self.member_groups.keys() {
                sketch.entity_group.insert(mid, restored_gid);
                sketch.touch(mid);
            }
            self.group = restored_gid;
        }
    }
}

/// Sisipkan teks parametrik baru ke dalam sketch.
pub struct InsertText {
    spec: TextSpec,
    style: Option<Style>,
    layer: Option<LayerId>,
    text_id: Option<TextId>,
    created_glyphs: Vec<EntityId>,
}

impl InsertText {
    pub fn new(spec: TextSpec) -> Self {
        Self {
            spec,
            style: None,
            layer: None,
            text_id: None,
            created_glyphs: Vec::new(),
        }
    }

    pub fn with_style(mut self, style: Style) -> Self {
        self.style = Some(style);
        self
    }

    pub fn with_layer(mut self, layer: LayerId) -> Self {
        self.layer = Some(layer);
        self
    }

    pub fn text_id(&self) -> Option<TextId> {
        self.text_id
    }

    pub fn created_glyphs(&self) -> &[EntityId] {
        &self.created_glyphs
    }
}

impl Command<Sketch> for InsertText {
    fn name(&self) -> &str {
        "Sisipkan Teks"
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        let text_obj = TextObject {
            spec: self.spec.clone(),
            glyph_entities: Vec::new(),
            style: self.style.clone().unwrap_or_else(TextObject::default_style),
            layer: self.layer,
        };
        let tid = sketch.texts.insert(text_obj);
        self.text_id = Some(tid);
        self.created_glyphs = regenerate_text(sketch, tid);
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        if let Some(tid) = self.text_id.take() {
            if let Some(obj) = sketch.texts.remove(tid) {
                for gid in obj.glyph_entities {
                    remove_single_entity_data(sketch, gid);
                }
            }
            self.created_glyphs.clear();
        }
    }
}

/// Perbarui spesifikasi teks parametrik (konten, font, ukuran, tata letak).
pub struct UpdateText {
    id: TextId,
    new_spec: TextSpec,
    old_spec: Option<TextSpec>,
    removed_glyphs: Vec<DeletedEntityData>,
    created_glyphs: Vec<EntityId>,
}

impl UpdateText {
    pub fn new(id: TextId, new_spec: TextSpec) -> Self {
        Self {
            id,
            new_spec,
            old_spec: None,
            removed_glyphs: Vec::new(),
            created_glyphs: Vec::new(),
        }
    }

    pub fn text_id(&self) -> TextId {
        self.id
    }
}

impl Command<Sketch> for UpdateText {
    fn name(&self) -> &str {
        "Perbarui Teks"
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        if let Some(obj) = sketch.texts.get_mut(self.id) {
            self.old_spec = Some(obj.spec.clone());
            self.removed_glyphs.clear();
            let old_glyphs = std::mem::take(&mut obj.glyph_entities);
            for gid in old_glyphs {
                if let Some(data) = remove_single_entity_data(sketch, gid) {
                    self.removed_glyphs.push(data);
                }
            }
            if let Some(obj) = sketch.texts.get_mut(self.id) {
                obj.spec = self.new_spec.clone();
            }
            self.created_glyphs = regenerate_text(sketch, self.id);
        }
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        if let Some(old_spec) = self.old_spec.take() {
            for gid in self.created_glyphs.drain(..) {
                remove_single_entity_data(sketch, gid);
            }
            let mut restored_ids = Vec::with_capacity(self.removed_glyphs.len());
            for data in self.removed_glyphs.drain(..) {
                let nid = restore_single_entity_data(sketch, data);
                sketch.origin.insert(nid, Origin::Text { text: self.id });
                restored_ids.push(nid);
            }
            if let Some(obj) = sketch.texts.get_mut(self.id) {
                obj.spec = old_spec;
                obj.glyph_entities = restored_ids;
            }
        }
    }
}

/// Hapus objek teks parametrik beserta seluruh glyph turunannya.
pub struct DeleteText {
    id: TextId,
    removed_text: Option<TextObject>,
    removed_glyphs: Vec<DeletedEntityData>,
}

impl DeleteText {
    pub fn new(id: TextId) -> Self {
        Self {
            id,
            removed_text: None,
            removed_glyphs: Vec::new(),
        }
    }

    pub fn text_id(&self) -> TextId {
        self.id
    }
}

impl Command<Sketch> for DeleteText {
    fn name(&self) -> &str {
        "Hapus Teks"
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        if let Some(mut obj) = sketch.texts.remove(self.id) {
            self.removed_glyphs.clear();
            for gid in obj.glyph_entities.drain(..) {
                if let Some(data) = remove_single_entity_data(sketch, gid) {
                    self.removed_glyphs.push(data);
                }
            }
            self.removed_text = Some(obj);
        }
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        if let Some(mut obj) = self.removed_text.take() {
            let mut restored_ids = Vec::with_capacity(self.removed_glyphs.len());
            for data in self.removed_glyphs.drain(..) {
                let nid = restore_single_entity_data(sketch, data);
                restored_ids.push(nid);
            }
            obj.glyph_entities = restored_ids.clone();
            let new_tid = sketch.texts.insert(obj);
            self.id = new_tid;
            for &nid in &restored_ids {
                sketch.origin.insert(nid, Origin::Text { text: new_tid });
            }
        }
    }
}
