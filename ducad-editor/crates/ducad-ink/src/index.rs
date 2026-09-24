//! Indeks spasial R-Tree untuk query coretan tinta cepat dan pre-filtering.

use std::collections::HashMap;

use glam::Vec2;
use rstar::{RTree, RTreeObject, AABB};

use crate::document::InkDoc;

/// Objek bounding box coretan untuk indeks spasial RTree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IndexedStroke {
    pub id: u64,
    pub min: [f32; 2],
    pub max: [f32; 2],
}

impl RTreeObject for IndexedStroke {
    type Envelope = AABB<[f32; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners(self.min, self.max)
    }
}

/// Indeks spasial berbasis R-Tree untuk query coretan tinta cepat.
#[derive(Debug, Clone)]
pub struct SpatialIndex {
    pub(crate) tree: RTree<IndexedStroke>,
    pub(crate) built_rev: u64,
    /// Id coretan → posisinya di `InkDoc::strokes` (z-order) saat dibangun.
    pub(crate) z_pos: HashMap<u64, usize>,
}

impl SpatialIndex {
    /// Membangun indeks spasial dari dokumen tinta.
    pub fn build(doc: &InkDoc) -> Self {
        let items: Vec<IndexedStroke> = doc
            .strokes
            .iter()
            .filter(|s| !s.hidden)
            .map(|s| IndexedStroke {
                id: s.id,
                min: [s.bbox.0.x, s.bbox.0.y],
                max: [s.bbox.1.x, s.bbox.1.y],
            })
            .collect();
        let z_pos = doc
            .strokes
            .iter()
            .enumerate()
            .map(|(i, s)| (s.id, i))
            .collect();

        Self {
            tree: RTree::bulk_load(items),
            built_rev: doc.rev,
            z_pos,
        }
    }

    /// Posisi z-order kandidat (coretan non-tersembunyi) yang beririsan
    /// dengan kotak, terurut dari bawah ke atas.
    pub(crate) fn candidates_z_ordered(&self, min: Vec2, max: Vec2) -> Vec<usize> {
        let envelope = AABB::from_corners([min.x, min.y], [max.x, max.y]);
        let mut pos: Vec<usize> = self
            .tree
            .locate_in_envelope_intersecting(&envelope)
            .filter_map(|item| self.z_pos.get(&item.id).copied())
            .collect();
        pos.sort_unstable();
        pos
    }

    pub fn tree(&self) -> &RTree<IndexedStroke> {
        &self.tree
    }

    /// Query ID coretan yang berpotensi tumpang tindih dengan kotak AABB (min..max),
    /// dikembalikan terurut menaik untuk determinisme.
    pub fn query_aabb(&self, min: Vec2, max: Vec2) -> Vec<u64> {
        let envelope = AABB::from_corners([min.x, min.y], [max.x, max.y]);
        let mut ids: Vec<u64> = self
            .tree
            .locate_in_envelope_intersecting(&envelope)
            .map(|item| item.id)
            .collect();
        ids.sort_unstable();
        ids
    }
}
