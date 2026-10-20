//! Posisi teks dimensi yang dipindah pengguna (Paritas 1.17).
//!
//! Label dimensi digambar di posisi bawaan (mis. tengah garis). Pengguna
//! boleh menyeret labelnya; geseran itu disimpan sebagai offset di ruang
//! sketch (mm) relatif terhadap posisi bawaan, sehingga label tetap ikut
//! geometri saat geometri berubah dan ikut tersimpan di berkas.

use ducad_core::Command;
use glam::DVec2;
use serde::{Deserialize, Serialize};

use crate::constraint::{Constraint, PointRef};
use crate::entity::EntityId;
use crate::sketch::Sketch;

/// Kunci stabil sebuah label dimensi. Berbasis rujukan geometri — bukan
/// indeks constraint — supaya tidak bergeser saat constraint lain dihapus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DimKey {
    /// Pill dimensi bawaan entitas (panjang garis, radius lingkaran/busur).
    Entity(EntityId),
    Distance(PointRef, PointRef),
    Radius(EntityId),
    Angle(EntityId, EntityId),
}

impl DimKey {
    /// Kunci label untuk constraint dimensi; `None` untuk constraint lain.
    pub fn of_constraint(c: &Constraint) -> Option<Self> {
        match c {
            Constraint::Distance { a, b, .. } => Some(DimKey::Distance(*a, *b)),
            Constraint::Radius { entity, .. } => Some(DimKey::Radius(*entity)),
            Constraint::Angle { a, b, .. } => Some(DimKey::Angle(*a, *b)),
            _ => None,
        }
    }
}

/// Satu entri offset label.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DimTextOffset {
    pub key: DimKey,
    pub offset: DVec2,
}

/// Offset di bawah ambang ini dianggap nol (entri dihapus).
const ZERO_EPS: f64 = 1e-9;

impl Sketch {
    /// Offset label `key` (nol bila belum pernah dipindah).
    pub fn dim_text_offset(&self, key: &DimKey) -> DVec2 {
        self.dim_text_offsets
            .iter()
            .find(|e| e.key == *key)
            .map(|e| e.offset)
            .unwrap_or(DVec2::ZERO)
    }

    /// Set offset label; offset nol menghapus entrinya, nilai tak hingga/NaN
    /// diabaikan. Mengembalikan offset sebelumnya.
    pub fn set_dim_text_offset(&mut self, key: DimKey, offset: DVec2) -> DVec2 {
        let prev = self.dim_text_offset(&key);
        if !offset.is_finite() {
            return prev;
        }
        self.dim_text_offsets.retain(|e| e.key != key);
        if offset.length() > ZERO_EPS {
            self.dim_text_offsets.push(DimTextOffset { key, offset });
        }
        prev
    }
}

/// Command undo/redo untuk memindah teks dimensi.
pub struct SetDimTextOffset {
    key: DimKey,
    offset: DVec2,
    prev: Option<DVec2>,
}

impl SetDimTextOffset {
    pub fn new(key: DimKey, offset: DVec2) -> Self {
        Self {
            key,
            offset,
            prev: None,
        }
    }
}

impl Command<Sketch> for SetDimTextOffset {
    fn name(&self) -> &str {
        "Pindah Teks Dimensi"
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        self.prev = Some(sketch.set_dim_text_offset(self.key, self.offset));
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        if let Some(prev) = self.prev.take() {
            sketch.set_dim_text_offset(self.key, prev);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Entity;

    #[test]
    fn offset_set_get_clear_and_hostile() {
        let mut sk = Sketch::default();
        let id = sk.entities.insert(Entity::line(DVec2::ZERO, DVec2::X));
        let key = DimKey::Entity(id);
        assert_eq!(sk.dim_text_offset(&key), DVec2::ZERO);
        sk.set_dim_text_offset(key, DVec2::new(1.0, 2.0));
        assert_eq!(sk.dim_text_offset(&key), DVec2::new(1.0, 2.0));
        // NaN diabaikan, nilai lama bertahan.
        sk.set_dim_text_offset(key, DVec2::new(f64::NAN, 0.0));
        assert_eq!(sk.dim_text_offset(&key), DVec2::new(1.0, 2.0));
        sk.set_dim_text_offset(key, DVec2::ZERO);
        assert!(sk.dim_text_offsets.is_empty());
    }

    #[test]
    fn offset_round_trips_through_json_and_old_files_load() {
        let mut sk = Sketch::default();
        let a = sk.entities.insert(Entity::point(DVec2::ZERO));
        let b = sk.entities.insert(Entity::point(DVec2::X));
        let key = DimKey::Distance(PointRef::Center(a), PointRef::Center(b));
        let mut cmd = SetDimTextOffset::new(key, DVec2::new(0.0, 5.0));
        cmd.apply(&mut sk);
        let json = serde_json::to_string(&sk).unwrap();
        let back: Sketch = serde_json::from_str(&json).unwrap();
        assert_eq!(back.dim_text_offset(&key), DVec2::new(0.0, 5.0));
        cmd.revert(&mut sk);
        assert_eq!(sk.dim_text_offset(&key), DVec2::ZERO);

        // Berkas lama (tanpa field) tetap terbaca, dan sketsa tanpa offset
        // tidak menulis field baru.
        let empty = serde_json::to_string(&Sketch::default()).unwrap();
        assert!(!empty.contains("dim_text_offsets"));
        let old: Sketch = serde_json::from_str(&empty).unwrap();
        assert!(old.dim_text_offsets.is_empty());
    }
}
