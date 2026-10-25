//! Peta body dengan kunci STABIL untuk undo/redo.
//!
//! `slotmap::SlotMap` memberi kunci baru setiap kali sebuah slot diisi
//! ulang, jadi "undo hapus body" atau "redo buat body" menghasilkan `BodyId`
//! lain dari semula. Langkah undo/redo yang dicatat SETELAHNYA (ganti
//! geometri, hapus, material, instance assembly, seleksi) masih memegang id
//! lama dan diam-diam tidak berlaku: redo setelah undo pembuatan body
//! terlihat "berhasil" tapi body tidak berubah.
//!
//! [`IdMap`] memakai indeks monoton sendiri dan mengizinkan
//! [`IdMap::insert`] pada id yang pernah dipakai, sehingga command cukup
//! menyimpan id-nya dan memasangnya kembali apa adanya. `BodyId` tetap tipe
//! kunci `slotmap` (serde, `data().as_ffi()` untuk UI/berkas tidak berubah).

use std::collections::BTreeMap;

use slotmap::{Key, KeyData};

use crate::{Body, BodyId, Material};

fn id_from_idx(idx: u32) -> BodyId {
    // Versi 1 (ganjil) agar `from_ffi` tidak mengubahnya; indeks u32::MAX
    // adalah kunci null slotmap dan tidak pernah dipakai.
    KeyData::from_ffi((1u64 << 32) | u64::from(idx)).into()
}

fn idx_of(id: BodyId) -> u32 {
    (id.data().as_ffi() & 0xffff_ffff) as u32
}

/// Peta `BodyId -> V` berkunci stabil. Urutan iterasi = urutan indeks
/// (body yang dipasang ulang kembali ke posisinya semula).
#[derive(Debug, Clone)]
pub struct IdMap<V> {
    items: BTreeMap<u32, V>,
    next_idx: u32,
}

impl<V> Default for IdMap<V> {
    fn default() -> Self {
        Self {
            items: BTreeMap::new(),
            next_idx: 1,
        }
    }
}

impl<V> IdMap<V> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Id baru yang belum pernah dipakai.
    pub fn allocate(&mut self) -> BodyId {
        let idx = self.next_idx;
        self.next_idx = self.next_idx.saturating_add(1).min(u32::MAX - 1);
        id_from_idx(idx)
    }

    /// Pasang `value` pada `id` (baru atau bekas); mengembalikan nilai lama.
    pub fn insert(&mut self, id: BodyId, value: V) -> Option<V> {
        if id.is_null() {
            return None;
        }
        let idx = idx_of(id);
        if idx >= self.next_idx {
            self.next_idx = idx.saturating_add(1).min(u32::MAX - 1);
        }
        self.items.insert(idx, value)
    }

    pub fn remove(&mut self, id: BodyId) -> Option<V> {
        self.items.remove(&idx_of(id))
    }

    pub fn get(&self, id: BodyId) -> Option<&V> {
        self.items.get(&idx_of(id))
    }

    pub fn get_mut(&mut self, id: BodyId) -> Option<&mut V> {
        self.items.get_mut(&idx_of(id))
    }

    pub fn contains_key(&self, id: BodyId) -> bool {
        self.items.contains_key(&idx_of(id))
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn iter(&self) -> impl Iterator<Item = (BodyId, &V)> + '_ {
        self.items.iter().map(|(i, v)| (id_from_idx(*i), v))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (BodyId, &mut V)> + '_ {
        self.items.iter_mut().map(|(i, v)| (id_from_idx(*i), v))
    }

    pub fn keys(&self) -> impl Iterator<Item = BodyId> + '_ {
        self.items.keys().map(|i| id_from_idx(*i))
    }

    pub fn values(&self) -> impl Iterator<Item = &V> + '_ {
        self.items.values()
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> + '_ {
        self.items.values_mut()
    }

    pub fn retain(&mut self, mut f: impl FnMut(BodyId, &mut V) -> bool) {
        self.items.retain(|i, v| f(id_from_idx(*i), v));
    }
}

impl<V> std::ops::Index<BodyId> for IdMap<V> {
    type Output = V;
    fn index(&self, id: BodyId) -> &V {
        match self.items.get(&idx_of(id)) {
            Some(v) => v,
            None => panic_missing(),
        }
    }
}

impl<V> std::ops::IndexMut<BodyId> for IdMap<V> {
    fn index_mut(&mut self, id: BodyId) -> &mut V {
        match self.items.get_mut(&idx_of(id)) {
            Some(v) => v,
            None => panic_missing(),
        }
    }
}

#[allow(clippy::panic)]
#[cold]
fn panic_missing() -> ! {
    // Setara `SlotMap` lama: indeks `[]` pada id yang tidak ada adalah bug
    // pemanggil, bukan keadaan yang bisa dipulihkan.
    panic!("BodyId tidak ada di peta")
}

impl<'a, V> IntoIterator for &'a IdMap<V> {
    type Item = (BodyId, &'a V);
    type IntoIter = Box<dyn Iterator<Item = (BodyId, &'a V)> + 'a>;
    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.iter())
    }
}

impl<'a, V> IntoIterator for &'a mut IdMap<V> {
    type Item = (BodyId, &'a mut V);
    type IntoIter = Box<dyn Iterator<Item = (BodyId, &'a mut V)> + 'a>;
    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.iter_mut())
    }
}

/// Iterator pemilik `IdMap`.
pub struct IntoIter<V>(std::collections::btree_map::IntoIter<u32, V>);

impl<V> Iterator for IntoIter<V> {
    type Item = (BodyId, V);
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(i, v)| (id_from_idx(i), v))
    }
}

impl<V> IntoIterator for IdMap<V> {
    type Item = (BodyId, V);
    type IntoIter = IntoIter<V>;
    fn into_iter(self) -> Self::IntoIter {
        IntoIter(self.items.into_iter())
    }
}

impl<V> FromIterator<(BodyId, V)> for IdMap<V> {
    fn from_iter<I: IntoIterator<Item = (BodyId, V)>>(iter: I) -> Self {
        let mut m = Self::default();
        for (id, v) in iter {
            m.insert(id, v);
        }
        m
    }
}

/// Peta body dokumen: [`IdMap<Body>`] + alokasi id untuk body baru.
#[derive(Debug, Clone, Default)]
pub struct BodyMap(IdMap<Body>);

impl BodyMap {
    /// Body baru dengan id baru (perilaku `SlotMap::insert` lama).
    pub fn insert(&mut self, body: Body) -> BodyId {
        let id = self.0.allocate();
        self.0.insert(id, body);
        id
    }

    /// Pasang body pada id yang SUDAH diketahui (undo hapus / redo buat).
    /// Mengembalikan body lama bila id itu ternyata masih terisi.
    pub fn insert_with_id(&mut self, id: BodyId, body: Body) -> Option<Body> {
        self.0.insert(id, body)
    }
}

impl std::ops::Deref for BodyMap {
    type Target = IdMap<Body>;
    fn deref(&self) -> &IdMap<Body> {
        &self.0
    }
}

impl std::ops::DerefMut for BodyMap {
    fn deref_mut(&mut self) -> &mut IdMap<Body> {
        &mut self.0
    }
}

impl<'a> IntoIterator for &'a BodyMap {
    type Item = (BodyId, &'a Body);
    type IntoIter = Box<dyn Iterator<Item = (BodyId, &'a Body)> + 'a>;
    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.0.iter())
    }
}

impl<'a> IntoIterator for &'a mut BodyMap {
    type Item = (BodyId, &'a mut Body);
    type IntoIter = Box<dyn Iterator<Item = (BodyId, &'a mut Body)> + 'a>;
    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.0.iter_mut())
    }
}

impl IntoIterator for BodyMap {
    type Item = (BodyId, Body);
    type IntoIter = IntoIter<Body>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Body {
    /// Body baru yang terlihat, tanpa material mekanik, dengan UUID baru.
    pub fn new(name: impl Into<String>, material: Material) -> Self {
        Self {
            name: name.into(),
            visible: true,
            material,
            mechanical: None,
            uuid: crate::new_part_uuid(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(n: &str) -> Body {
        Body::new(n, Material::default())
    }

    #[test]
    fn reinsert_keeps_id_and_order() {
        let mut m = BodyMap::default();
        let a = m.insert(body("a"));
        let b = m.insert(body("b"));
        let c = m.insert(body("c"));
        assert_ne!(a, b);
        let removed = m.remove(b).unwrap();
        assert!(!m.contains_key(b));
        assert_eq!(m.len(), 2);
        assert!(m.insert_with_id(b, removed).is_none());
        assert_eq!(m.keys().collect::<Vec<_>>(), vec![a, b, c]);
        assert_eq!(m[b].name, "b");
        // id baru tidak pernah menabrak id bekas
        let d = m.insert(body("d"));
        assert!(![a, b, c].contains(&d));
    }

    #[test]
    fn ffi_round_trip_matches_slotmap_key_encoding() {
        let mut m = BodyMap::default();
        let a = m.insert(body("a"));
        let raw = a.data().as_ffi();
        let back: BodyId = KeyData::from_ffi(raw).into();
        assert_eq!(a, back);
        assert!(!a.is_null());
    }

    #[test]
    fn secondary_idmap_follows_reinserted_id() {
        let mut bodies = BodyMap::default();
        let mut geo: IdMap<u32> = IdMap::new();
        let a = bodies.insert(body("a"));
        geo.insert(a, 7);
        let ba = bodies.remove(a).unwrap();
        geo.remove(a);
        bodies.insert_with_id(a, ba);
        assert!(geo.get(a).is_none());
        assert_eq!(geo.insert(a, 9), None);
        assert_eq!(geo[a], 9);
    }
}
