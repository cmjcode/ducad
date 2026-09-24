use glam::DVec2;
use rstar::{RTree, RTreeObject, AABB};
use slotmap::SecondaryMap;

use crate::entity::{Entity, EntityId};
use crate::sketch::Sketch;

/// Bounding box yang diindeks dalam R-Tree untuk entitas sketch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IndexedBox {
    pub id: EntityId,
    pub min: [f64; 2],
    pub max: [f64; 2],
}

impl IndexedBox {
    pub fn from_entity(id: EntityId, entity: &Entity) -> Option<Self> {
        let (min, max) = entity.bounding_box()?;
        Some(Self {
            id,
            min: [min.x, min.y],
            max: [max.x, max.y],
        })
    }
}

impl RTreeObject for IndexedBox {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners(self.min, self.max)
    }
}

impl rstar::PointDistance for IndexedBox {
    fn distance_2(&self, point: &[f64; 2]) -> f64 {
        self.envelope().distance_2(point)
    }
}

/// Indeks spasial berbasis R-Tree untuk entitas sketch.
#[derive(Debug, Clone)]
pub struct SpatialIndex {
    pub(crate) tree: RTree<IndexedBox>,
    pub(crate) boxes: SecondaryMap<EntityId, IndexedBox>,
    pub(crate) last_rev: SecondaryMap<EntityId, u64>,
    pub(crate) built_rev: u64,
    pub(crate) entity_count: usize,
}

impl Default for SpatialIndex {
    fn default() -> Self {
        Self {
            tree: RTree::new(),
            boxes: SecondaryMap::new(),
            last_rev: SecondaryMap::new(),
            built_rev: 0,
            entity_count: 0,
        }
    }
}

impl SpatialIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn built_rev(&self) -> u64 {
        self.built_rev
    }

    pub fn len(&self) -> usize {
        self.tree.size()
    }

    pub fn is_empty(&self) -> bool {
        self.tree.size() == 0
    }

    /// Query semua EntityId yang bounding box-nya beririsan dengan titik `p` yang diperluas `tol`.
    pub fn query_point(&self, p: DVec2, tol: f64) -> Vec<EntityId> {
        let tol = tol.abs();
        let query_env = AABB::from_corners([p.x - tol, p.y - tol], [p.x + tol, p.y + tol]);
        self.tree
            .locate_in_envelope_intersecting(&query_env)
            .map(|b| b.id)
            .collect()
    }

    /// Query semua EntityId yang bounding box-nya beririsan dengan kotak `[min, max]`.
    pub fn query_rect(&self, min: DVec2, max: DVec2) -> Vec<EntityId> {
        let query_env = AABB::from_corners(
            [min.x.min(max.x), min.y.min(max.y)],
            [min.x.max(max.x), min.y.max(max.y)],
        );
        self.tree
            .locate_in_envelope_intersecting(&query_env)
            .map(|b| b.id)
            .collect()
    }

    /// Bangun ulang seluruh indeks dari sketch menggunakan bulk load OMT.
    pub fn rebuild_from_sketch(&mut self, sketch: &Sketch) {
        let mut new_boxes = SecondaryMap::new();
        let mut new_last_rev = SecondaryMap::new();
        let mut rtree_boxes = Vec::with_capacity(sketch.entities.len());

        for (id, entity) in &sketch.entities {
            let rev = sketch.rev.get(id).copied().unwrap_or(0);
            new_last_rev.insert(id, rev);
            if let Some(ibox) = IndexedBox::from_entity(id, entity) {
                rtree_boxes.push(ibox);
                new_boxes.insert(id, ibox);
            }
        }

        self.tree = RTree::bulk_load(rtree_boxes);
        self.boxes = new_boxes;
        self.last_rev = new_last_rev;
        self.built_rev = sketch.geom_rev;
        self.entity_count = sketch.entities.len();
    }

    /// Perbarui indeks secara inkremental atau bangun ulang bila struktur berubah banyak.
    pub fn update_from_sketch(&mut self, sketch: &Sketch, dirty: Option<&[EntityId]>) {
        if self.tree.size() == 0 && !sketch.entities.is_empty() {
            self.rebuild_from_sketch(sketch);
            return;
        }

        // Daftar kotor hanya dipakai bila ia menjelaskan seluruh perubahan
        // jumlah entitas; mutasi langsung di luar command (tanpa `touch`)
        // jatuh ke pemindaian penuh di bawah.
        let dirty: Option<Vec<EntityId>> = dirty.map(|d| {
            let mut seen = std::collections::HashSet::with_capacity(d.len());
            d.iter().copied().filter(|id| seen.insert(*id)).collect()
        });
        let dirty = dirty.filter(|d| {
            let removed = d
                .iter()
                .filter(|id| {
                    self.last_rev.contains_key(**id) && !sketch.entities.contains_key(**id)
                })
                .count();
            let added = d
                .iter()
                .filter(|id| {
                    sketch.entities.contains_key(**id) && !self.last_rev.contains_key(**id)
                })
                .count();
            self.entity_count + added == sketch.entities.len() + removed
        });

        if let Some(dirty_ids) = dirty {
            for &id in &dirty_ids {
                let current_rev = sketch.rev.get(id).copied().unwrap_or(0);
                if let Some(entity) = sketch.entities.get(id) {
                    if let Some(old_box) = self.boxes.remove(id) {
                        self.tree.remove(&old_box);
                    }
                    if let Some(new_box) = IndexedBox::from_entity(id, entity) {
                        self.tree.insert(new_box);
                        self.boxes.insert(id, new_box);
                    }
                    self.last_rev.insert(id, current_rev);
                } else {
                    if let Some(old_box) = self.boxes.remove(id) {
                        self.tree.remove(&old_box);
                    }
                    self.last_rev.remove(id);
                }
            }
            self.built_rev = sketch.geom_rev;
            self.entity_count = sketch.entities.len();
            return;
        }

        // Jaring pengaman: jika jumlah entitas berbeda, bangun ulang
        if self.entity_count != sketch.entities.len() {
            self.rebuild_from_sketch(sketch);
            return;
        }

        // Cek revisi per entitas
        let mut changed = Vec::new();
        for (id, entity) in &sketch.entities {
            let rev = sketch.rev.get(id).copied().unwrap_or(0);
            let prev_rev = self.last_rev.get(id).copied().unwrap_or(0);
            if rev != prev_rev {
                changed.push((id, entity, rev));
            }
        }

        for (id, entity, rev) in changed {
            if let Some(old_box) = self.boxes.remove(id) {
                self.tree.remove(&old_box);
            }
            if let Some(new_box) = IndexedBox::from_entity(id, entity) {
                self.tree.insert(new_box);
                self.boxes.insert(id, new_box);
            }
            self.last_rev.insert(id, rev);
        }

        self.built_rev = sketch.geom_rev;
        self.entity_count = sketch.entities.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{
        DeleteEntities, InsertEntities, RenameEntities, ReplaceEntities, ResizeRectangle,
        ToggleConstruction, TranslateEntities, UpdateEntity,
    };
    use crate::entity::{PathSeg, Subpath};
    use ducad_core::Command;

    #[test]
    fn touch_increments_entity_and_global_rev() {
        let mut sketch = Sketch::default();
        let id1 = sketch
            .entities
            .insert(Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0)));
        let id2 = sketch
            .entities
            .insert(Entity::circle(DVec2::new(20.0, 20.0), 5.0));

        assert_eq!(sketch.global_rev, 0);
        assert_eq!(sketch.rev.get(id1), None);
        assert_eq!(sketch.rev.get(id2), None);

        sketch.touch(id1);
        assert_eq!(sketch.global_rev, 1);
        assert_eq!(sketch.rev.get(id1), Some(&1));
        assert_eq!(sketch.rev.get(id2), None);

        sketch.touch(id1);
        assert_eq!(sketch.global_rev, 2);
        assert_eq!(sketch.rev.get(id1), Some(&2));

        sketch.touch(id2);
        assert_eq!(sketch.global_rev, 3);
        assert_eq!(sketch.rev.get(id2), Some(&1));

        sketch.touch_all();
        assert_eq!(sketch.global_rev, 4);
        assert_eq!(sketch.rev.get(id1), Some(&3));
        assert_eq!(sketch.rev.get(id2), Some(&2));
    }

    #[test]
    fn every_command_touches_its_entities() {
        // 1. InsertEntities
        let mut sketch = Sketch::default();
        let mut cmd_ins = InsertEntities::new(
            "insert",
            vec![Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0))],
        );
        let rev0 = sketch.global_rev;
        cmd_ins.apply(&mut sketch);
        assert!(sketch.global_rev > rev0);
        let id = sketch.entities.keys().next().unwrap();
        assert_eq!(sketch.rev.get(id), Some(&1));

        // 2. DeleteEntities
        let rev1 = sketch.global_rev;
        let mut cmd_del = DeleteEntities::new(vec![id]);
        cmd_del.apply(&mut sketch);
        assert!(sketch.global_rev > rev1);

        // 3. ReplaceEntities
        let mut sketch = Sketch::default();
        let c_id = sketch.entities.insert(Entity::circle(DVec2::ZERO, 5.0));
        let rev0 = sketch.global_rev;
        let mut cmd_rep = ReplaceEntities::new(
            "replace",
            vec![c_id],
            vec![Entity::circle(DVec2::ZERO, 10.0)],
        );
        cmd_rep.apply(&mut sketch);
        assert!(sketch.global_rev > rev0);
        let new_id = sketch.entities.keys().next().unwrap();
        assert_eq!(sketch.rev.get(new_id), Some(&1));

        // 4. UpdateEntity
        let mut sketch = Sketch::default();
        let u_id = sketch.entities.insert(Entity::circle(DVec2::ZERO, 5.0));
        let rev0 = sketch.global_rev;
        let mut cmd_up = UpdateEntity::new("up", u_id, Entity::circle(DVec2::ZERO, 12.0));
        cmd_up.apply(&mut sketch);
        assert!(sketch.global_rev > rev0);
        assert_eq!(sketch.rev.get(u_id), Some(&1));

        // 5. ResizeRectangle
        let mut sketch = Sketch::default();
        let l1 = sketch
            .entities
            .insert(Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0)));
        let l2 = sketch
            .entities
            .insert(Entity::line(DVec2::new(10.0, 0.0), DVec2::new(10.0, 5.0)));
        let rev0 = sketch.global_rev;
        let mut cmd_resize = ResizeRectangle::new(
            "resize",
            vec![
                (l1, Entity::line(DVec2::ZERO, DVec2::new(20.0, 0.0))),
                (
                    l2,
                    Entity::line(DVec2::new(20.0, 0.0), DVec2::new(20.0, 5.0)),
                ),
            ],
        );
        cmd_resize.apply(&mut sketch);
        assert!(sketch.global_rev > rev0);
        assert_eq!(sketch.rev.get(l1), Some(&1));
        assert_eq!(sketch.rev.get(l2), Some(&1));

        // 6. TranslateEntities
        let mut sketch = Sketch::default();
        let tr_id = sketch.entities.insert(Entity::circle(DVec2::ZERO, 5.0));
        let rev0 = sketch.global_rev;
        let mut cmd_tr = TranslateEntities::new("translate", vec![tr_id], DVec2::new(3.0, 4.0));
        cmd_tr.apply(&mut sketch);
        assert!(sketch.global_rev > rev0);
        assert_eq!(sketch.rev.get(tr_id), Some(&1));

        // 7. RenameEntities
        let mut sketch = Sketch::default();
        let rn_id = sketch.entities.insert(Entity::circle(DVec2::ZERO, 5.0));
        let rev0 = sketch.global_rev;
        let mut cmd_rn = RenameEntities::new(vec![rn_id], "group_a");
        cmd_rn.apply(&mut sketch);
        assert!(sketch.global_rev > rev0);
        assert_eq!(sketch.rev.get(rn_id), Some(&1));

        // 8. ToggleConstruction
        let mut sketch = Sketch::default();
        let tc_id = sketch.entities.insert(Entity::circle(DVec2::ZERO, 5.0));
        let rev0 = sketch.global_rev;
        let mut cmd_tc = ToggleConstruction::new(vec![tc_id], true);
        cmd_tc.apply(&mut sketch);
        assert!(sketch.global_rev > rev0);
        assert_eq!(sketch.rev.get(tc_id), Some(&1));
    }

    struct SimpleRng(u64);
    impl SimpleRng {
        fn next_u32(&mut self) -> u32 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.0 >> 32) as u32
        }
        fn next_f64(&mut self, min: f64, max: f64) -> f64 {
            let t = (self.next_u32() as f64) / (u32::MAX as f64);
            min + t * (max - min)
        }
        fn next_dvec2(&mut self, min: f64, max: f64) -> DVec2 {
            DVec2::new(self.next_f64(min, max), self.next_f64(min, max))
        }
    }

    #[test]
    fn spatial_query_matches_linear_hit_test() {
        let mut rng = SimpleRng(123456789);

        for seed in 0..200 {
            let mut sketch = Sketch::default();
            let count = (rng.next_u32() % 15 + 5) as usize;

            for _ in 0..count {
                let kind = rng.next_u32() % 5;
                let entity = match kind {
                    0 => Entity::line(rng.next_dvec2(-100.0, 100.0), rng.next_dvec2(-100.0, 100.0)),
                    1 => Entity::circle(rng.next_dvec2(-100.0, 100.0), rng.next_f64(1.0, 30.0)),
                    2 => Entity::Arc {
                        center: rng.next_dvec2(-100.0, 100.0),
                        radius: rng.next_f64(5.0, 40.0),
                        start_angle: rng.next_f64(0.0, std::f64::consts::PI),
                        end_angle: rng.next_f64(std::f64::consts::PI, std::f64::consts::TAU),
                        is_construction: false,
                    },
                    3 => Entity::ellipse(
                        rng.next_dvec2(-100.0, 100.0),
                        rng.next_f64(10.0, 40.0),
                        rng.next_f64(5.0, 20.0),
                    ),
                    _ => {
                        let p0 = rng.next_dvec2(-100.0, 100.0);
                        let p1 = rng.next_dvec2(-100.0, 100.0);
                        Entity::Path {
                            subpaths: vec![Subpath {
                                start: p0,
                                segs: vec![PathSeg::Line { end: p1 }],
                                closed: false,
                            }],
                            is_construction: false,
                        }
                    }
                };
                let id = sketch.entities.insert(entity);
                sketch.touch(id);
            }

            // Test 10 probe points per random sketch
            for _ in 0..10 {
                let p = rng.next_dvec2(-120.0, 120.0);
                let tol = rng.next_f64(0.5, 10.0);

                let hit_spatial = sketch.hit_test(p, tol);
                let hit_linear = sketch.hit_test_linear(p, tol);

                assert_eq!(
                    hit_spatial, hit_linear,
                    "Seed {seed}: Mismatch at p={:?}, tol={}",
                    p, tol
                );
            }

            // Also test mutation: modify one entity and touch it
            if let Some(id) = sketch.entities.keys().next() {
                let new_pos = rng.next_dvec2(-50.0, 50.0);
                if let Some(e) = sketch.entities.get_mut(id) {
                    *e = Entity::circle(new_pos, 8.0);
                }
                sketch.touch(id);

                let p = new_pos + DVec2::new(7.8, 0.0);
                let hit_spatial = sketch.hit_test(p, 1.0);
                let hit_linear = sketch.hit_test_linear(p, 1.0);
                assert_eq!(
                    hit_spatial, hit_linear,
                    "Seed {seed} after touch mutation: Mismatch at p={:?}",
                    p
                );
            }
        }
    }

    /// Regresi REVIEW-2026-09-24 #15: operasi layer (visual) tidak boleh
    /// membangun ulang indeks spasial; perubahan geometri tetap terlihat.
    #[test]
    fn layer_rename_keeps_spatial_index_and_dirty_is_drained() {
        use crate::commands::RenameLayer;
        use crate::layer::Layer;

        let mut sketch = Sketch::default();
        let lid = sketch
            .layers
            .insert(Layer::new("A", crate::style::Rgba::WHITE));
        sketch.layer_order.push(lid);
        let mut ins = InsertEntities::new(
            "Line",
            (0..10)
                .map(|i| {
                    Entity::line(
                        DVec2::new(i as f64 * 10.0, 0.0),
                        DVec2::new(i as f64 * 10.0 + 5.0, 0.0),
                    )
                })
                .collect(),
        );
        ins.apply(&mut sketch);
        let geom_before = sketch.spatial().built_rev();

        let mut rename = RenameLayer::new(lid, "B");
        rename.apply(&mut sketch);
        assert_eq!(
            sketch.geom_rev, geom_before,
            "ganti nama layer bukan perubahan geometri"
        );
        assert!(
            sketch.spatial_cache.read().index.is_some(),
            "indeks tidak dibuang"
        );

        let id = sketch.entities.keys().next().unwrap();
        let mut mv = TranslateEntities::new("Move", vec![id], DVec2::new(0.0, 100.0));
        mv.apply(&mut sketch);
        assert_eq!(sketch.spatial_cache.read().dirty.len(), 1);
        let hits = sketch.query_spatial_point(DVec2::new(2.0, 100.0), 0.5);
        assert_eq!(hits, vec![id]);
        assert!(
            sketch.spatial_cache.read().dirty.is_empty(),
            "daftar kotor dikonsumsi"
        );
    }

    /// Daftar kotor dibatasi walau indeks tidak pernah di-query.
    #[test]
    fn dirty_list_is_bounded_without_queries() {
        let mut sketch = Sketch::default();
        let id = sketch.entities.insert(Entity::line(DVec2::ZERO, DVec2::X));
        for _ in 0..1000 {
            sketch.touch(id);
        }
        assert!(sketch.spatial_cache.read().dirty.len() <= 64);
        assert_eq!(
            sketch.query_spatial_point(DVec2::new(0.5, 0.0), 0.1),
            vec![id]
        );
    }
}
