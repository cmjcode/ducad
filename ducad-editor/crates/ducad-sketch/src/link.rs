//! Entitas *linked* hasil Project/Intersect (Paritas 1.14).
//!
//! Entitas linked adalah geometri sketsa biasa (`Entity`) yang ASAL-nya
//! tercatat di [`Sketch::linked`]: body sumber, tepi/face yang diproyeksikan
//! atau mode irisan, plus sidik jari mesh body saat diturunkan. GUI
//! membandingkan sidik jari itu dengan body sekarang dan menurunkan ulang
//! entitasnya bila body berubah — entitas linked bersifat read-only bagi
//! pengguna. "Break Link" cukup menghapus catatan asal sehingga entitasnya
//! menjadi entitas biasa.
//!
//! Catatan serde: `HashMap<EntityId, _>` tidak bisa langsung jadi objek JSON
//! (kunci slotmap berupa struct, bukan string), jadi peta disimpan sebagai
//! daftar pasangan lewat modul `pairs` di bawah. Berkas lama tanpa field ini
//! tetap terbaca (`serde(default)`).

use ducad_core::Command;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::entity::{Entity, EntityId};
use crate::sketch::Sketch;

/// Jenis sumber geometri linked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkKind {
    /// Proyeksi tepi B-rep (indeks urutan `enumerate_edges` kernel).
    Edges { edges: Vec<usize> },
    /// Proyeksi seluruh tepi batas face (indeks urutan `enumerate_faces`).
    Faces { faces: Vec<usize> },
    /// Kurva irisan body dengan bidang sketsa.
    Intersect,
}

/// Asal satu entitas linked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkSource {
    /// Nomor grup: semua entitas hasil SATU perintah Project berbagi nomor
    /// ini dan diturunkan ulang bersama-sama.
    pub group: u32,
    /// Nama body sumber. `BodyId` tidak stabil lintas simpan/buka, jadi
    /// rujukan memakai nama (body pertama dengan nama itu).
    pub body: String,
    pub kind: LinkKind,
    /// Sidik jari mesh body saat entitas diturunkan; beda = body berubah.
    #[serde(default)]
    pub fingerprint: u64,
}

/// Serde peta `EntityId → LinkSource` sebagai daftar pasangan.
pub mod pairs {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S: Serializer>(
        map: &HashMap<EntityId, LinkSource>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        let mut v: Vec<(&EntityId, &LinkSource)> = map.iter().collect();
        // Urutan stabil supaya berkas tersimpan deterministik.
        v.sort_by_key(|(id, src)| (src.group, **id));
        v.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<HashMap<EntityId, LinkSource>, D::Error> {
        let v: Vec<(EntityId, LinkSource)> = Vec::deserialize(d)?;
        Ok(v.into_iter().collect())
    }
}

impl Sketch {
    /// `true` bila entitas `id` linked (read-only, diturunkan dari body).
    pub fn is_linked(&self, id: EntityId) -> bool {
        self.linked.contains_key(&id)
    }

    /// Nomor grup link berikutnya yang belum dipakai.
    pub fn next_link_group(&self) -> u32 {
        self.linked.values().map(|s| s.group).max().map_or(1, |g| g.saturating_add(1))
    }

    /// Daftar grup link: `(group, sumber contoh, id entitas berurutan)`.
    /// Entitas yang sudah tidak ada dilewati.
    pub fn link_groups(&self) -> Vec<(u32, LinkSource, Vec<EntityId>)> {
        let mut groups: Vec<(u32, LinkSource, Vec<EntityId>)> = Vec::new();
        let mut ids: Vec<(&EntityId, &LinkSource)> = self
            .linked
            .iter()
            .filter(|(id, _)| self.entities.contains_key(**id))
            .collect();
        ids.sort_by_key(|(id, src)| (src.group, **id));
        for (id, src) in ids {
            match groups.last_mut() {
                Some(g) if g.0 == src.group => g.2.push(*id),
                _ => groups.push((src.group, src.clone(), vec![*id])),
            }
        }
        groups
    }

    /// Ganti geometri satu grup link dengan hasil turunan baru (bukan
    /// langkah undo — entitas linked adalah data turunan). Bila jumlah dan
    /// jenis entitas sama, id lama dipertahankan (constraint tetap menempel);
    /// selain itu entitas lama dihapus, yang baru disisipkan, dan constraint
    /// yang kehilangan rujukan dilepas. Mengembalikan id entitas grup sekarang.
    pub fn rederive_link_group(
        &mut self,
        group: u32,
        fresh: Vec<Entity>,
        fingerprint: u64,
    ) -> Vec<EntityId> {
        let Some((_, template, old_ids)) =
            self.link_groups().into_iter().find(|(g, _, _)| *g == group)
        else {
            return Vec::new();
        };
        let same_shape = old_ids.len() == fresh.len()
            && old_ids.iter().zip(&fresh).all(|(id, e)| {
                self.entities
                    .get(*id)
                    .is_some_and(|old| std::mem::discriminant(old) == std::mem::discriminant(e))
            });
        let mut src = template;
        src.fingerprint = fingerprint;
        if same_shape {
            for (id, e) in old_ids.iter().zip(fresh) {
                if let Some(slot) = self.entities.get_mut(*id) {
                    *slot = e;
                }
                self.linked.insert(*id, src.clone());
                self.touch(*id);
            }
            return old_ids;
        }
        for id in &old_ids {
            self.entities.remove(*id);
            self.linked.remove(id);
            self.entity_names.remove(id);
            self.z_order.retain(|z| z != id);
            self.touch(*id);
        }
        let new_ids: Vec<EntityId> = fresh
            .into_iter()
            .map(|e| {
                let id = self.entities.insert(e);
                self.linked.insert(id, src.clone());
                self.touch(id);
                id
            })
            .collect();
        let snapshot = std::mem::take(&mut self.constraints);
        self.constraints = snapshot
            .into_iter()
            .filter(|c| crate::constraint::constraint_is_resolvable(self, c))
            .collect();
        new_ids
    }

    /// Buang catatan link milik entitas yang sudah tidak ada.
    pub fn prune_links(&mut self) {
        let entities = &self.entities;
        self.linked.retain(|id, _| entities.contains_key(*id));
    }
}

/// Sisipkan entitas linked (satu grup) sebagai satu langkah undo.
pub struct InsertLinked {
    entities: Vec<Entity>,
    source: LinkSource,
    group_name: Option<String>,
    inserted: Vec<EntityId>,
}

impl InsertLinked {
    /// `source.group` diabaikan; nomor grup baru dipilih saat `apply`.
    pub fn new(entities: Vec<Entity>, source: LinkSource) -> Self {
        Self {
            entities,
            source,
            group_name: None,
            inserted: Vec::new(),
        }
    }

    pub fn with_group_name(mut self, name: impl Into<String>) -> Self {
        self.group_name = Some(name.into());
        self
    }

    pub fn inserted_ids(&self) -> &[EntityId] {
        &self.inserted
    }
}

impl Command<Sketch> for InsertLinked {
    fn name(&self) -> &str {
        "Project"
    }

    fn apply(&mut self, sketch: &mut Sketch) {
        let mut src = self.source.clone();
        src.group = sketch.next_link_group();
        self.inserted = self
            .entities
            .iter()
            .cloned()
            .map(|e| {
                let id = sketch.entities.insert(e);
                sketch.linked.insert(id, src.clone());
                if let Some(n) = &self.group_name {
                    sketch.entity_names.insert(id, n.clone());
                }
                sketch.touch(id);
                id
            })
            .collect();
    }

    fn revert(&mut self, sketch: &mut Sketch) {
        for id in self.inserted.drain(..) {
            sketch.entities.remove(id);
            sketch.linked.remove(&id);
            sketch.entity_names.remove(&id);
            sketch.touch(id);
        }
    }
}

/// "Break Link": lepas status linked entitas `ids` (geometri tidak berubah).
pub struct BreakLinks {
    ids: Vec<EntityId>,
    removed: Vec<(EntityId, LinkSource)>,
}

impl BreakLinks {
    pub fn new(ids: Vec<EntityId>) -> Self {
        Self { ids, removed: Vec::new() }
    }

    /// Jumlah entitas yang dilepas pada `apply` terakhir.
    pub fn count(&self) -> usize {
        self.removed.len()
    }
}

impl Command<Sketch> for BreakLinks {
    fn name(&self) -> &str {
        "Break Link"
    }

    fn apply(&mut self, sketch: &mut Sketch) {
        self.removed = self
            .ids
            .iter()
            .filter_map(|id| sketch.linked.remove(id).map(|s| (*id, s)))
            .collect();
        for (id, _) in &self.removed {
            sketch.touch(*id);
        }
    }

    fn revert(&mut self, sketch: &mut Sketch) {
        for (id, src) in self.removed.drain(..) {
            if sketch.entities.contains_key(id) {
                sketch.linked.insert(id, src);
                sketch.touch(id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;

    fn src(kind: LinkKind) -> LinkSource {
        LinkSource { group: 0, body: "block".into(), kind, fingerprint: 7 }
    }

    #[test]
    fn insert_break_and_undo() {
        let mut sk = Sketch::default();
        let ents = vec![Entity::line(DVec2::ZERO, DVec2::X), Entity::line(DVec2::X, DVec2::ONE)];
        let mut ins = InsertLinked::new(ents, src(LinkKind::Faces { faces: vec![3] }));
        ins.apply(&mut sk);
        let ids = ins.inserted_ids().to_vec();
        assert!(ids.iter().all(|id| sk.is_linked(*id)));
        assert_eq!(sk.link_groups().len(), 1);
        let mut br = BreakLinks::new(vec![ids[0]]);
        br.apply(&mut sk);
        assert_eq!(br.count(), 1);
        assert!(!sk.is_linked(ids[0]) && sk.is_linked(ids[1]));
        br.revert(&mut sk);
        assert!(sk.is_linked(ids[0]));
        ins.revert(&mut sk);
        assert!(sk.entities.is_empty() && sk.linked.is_empty());
    }

    #[test]
    fn rederive_keeps_ids_when_shape_matches_and_replaces_otherwise() {
        let mut sk = Sketch::default();
        let mut ins = InsertLinked::new(
            vec![Entity::line(DVec2::ZERO, DVec2::X)],
            src(LinkKind::Intersect),
        );
        ins.apply(&mut sk);
        let id = ins.inserted_ids()[0];
        let g = sk.linked[&id].group;
        let kept = sk.rederive_link_group(g, vec![Entity::line(DVec2::ZERO, DVec2::Y)], 9);
        assert_eq!(kept, vec![id]);
        assert_eq!(sk.entities[id].endpoints()[1], DVec2::Y);
        assert_eq!(sk.linked[&id].fingerprint, 9);
        let replaced = sk.rederive_link_group(
            g,
            vec![Entity::circle(DVec2::ZERO, 2.0), Entity::circle(DVec2::ONE, 1.0)],
            10,
        );
        assert_eq!(replaced.len(), 2);
        assert!(!sk.entities.contains_key(id));
        assert!(replaced.iter().all(|r| sk.linked[r].group == g));
        // Grup tak dikenal: tidak terjadi apa-apa.
        assert!(sk.rederive_link_group(99, vec![], 0).is_empty());
    }

    #[test]
    fn linked_map_roundtrips_through_json() {
        let mut sk = Sketch::default();
        let mut ins = InsertLinked::new(
            vec![Entity::line(DVec2::ZERO, DVec2::X)],
            src(LinkKind::Edges { edges: vec![1, 4] }),
        );
        ins.apply(&mut sk);
        let json = serde_json::to_string(&sk).unwrap();
        let back: Sketch = serde_json::from_str(&json).unwrap();
        assert_eq!(back.linked, sk.linked);
        // Sketsa tanpa link tidak menulis field `linked`; berkas lama tanpa
        // field itu tetap terbaca.
        let plain = serde_json::to_string(&Sketch::default()).unwrap();
        assert!(!plain.contains("\"linked\""));
        let old: Sketch = serde_json::from_str(&plain).unwrap();
        assert!(old.linked.is_empty());
    }
}
