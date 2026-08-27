use glam::DVec2;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::constraint::Constraint;
use crate::entity::{Entity, EntityId};

/// Satu sketch pada sebuah bidang kerja.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Sketch {
    pub entities: slotmap::SlotMap<EntityId, Entity>,
    /// Constraint aktif (lihat modul `constraint`) — solver menulis balik
    /// geometri `entities` di atas saat constraint berubah.
    pub constraints: Vec<Constraint>,
    /// Nama grup yang ditetapkan pengguna untuk entitas.
    /// Entitas yang berbagi nama yang sama dikelompokkan sebagai satu grup di UI.
    /// Entitas tanpa entry di sini ditampilkan flat tanpa grup.
    #[serde(default)]
    pub entity_names: HashMap<EntityId, String>,
    /// ID entitas yang disembunyikan (hidden).
    #[serde(default)]
    pub hidden_entities: HashSet<EntityId>,
}

impl Sketch {
    /// Mengecek apakah entitas sedang disembunyikan.
    pub fn is_hidden(&self, id: EntityId) -> bool {
        self.hidden_entities.contains(&id)
    }

    /// Mengecek apakah entitas terlihat (visible).
    pub fn is_visible(&self, id: EntityId) -> bool {
        !self.is_hidden(id)
    }

    /// Mengatur visibilitas suatu entitas.
    pub fn set_visible(&mut self, id: EntityId, visible: bool) {
        if visible {
            self.hidden_entities.remove(&id);
        } else {
            self.hidden_entities.insert(id);
        }
    }

    /// Toggle visibilitas suatu entitas. Mengembalikan status visibilitas baru (true jika visible).
    pub fn toggle_visibility(&mut self, id: EntityId) -> bool {
        if self.hidden_entities.contains(&id) {
            self.hidden_entities.remove(&id);
            true
        } else {
            self.hidden_entities.insert(id);
            false
        }
    }

    /// Entitas terdekat dari `p` dalam radius `tolerance`, atau `None`.
    pub fn hit_test(&self, p: DVec2, tolerance: f64) -> Option<EntityId> {
        self.entities
            .iter()
            .filter(|(id, _)| !self.is_hidden(*id))
            .map(|(id, e)| (id, e.distance_to(p)))
            .filter(|(_, d)| *d <= tolerance)
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(id, _)| id)
    }

    /// Mengambil seluruh ID entitas yang berada dalam satu grup logis dengan `id` (misal satu rangkaian teks).
    pub fn related_group_entities(&self, id: EntityId) -> Vec<EntityId> {
        if let Some(g_name) = self.entity_names.get(&id) {
            return self
                .entities
                .keys()
                .filter(|k| self.entity_names.get(k) == Some(g_name))
                .collect();
        }
        // Fallback: Jika entitas adalah spline tertutup (huruf teks), gabungkan dengan seluruh spline tertutup di sketch
        if let Some(Entity::Spline { points, .. }) = self.entities.get(id) {
            if points.len() >= 3 {
                let first = points[0];
                let last = points.last().unwrap();
                if (first - *last).length_squared() < 1e-4 {
                    let all_closed_splines: Vec<EntityId> = self
                        .entities
                        .iter()
                        .filter_map(|(eid, ent)| match ent {
                            Entity::Spline { points: pts, .. } if pts.len() >= 3 => {
                                let f = pts[0];
                                let l = pts.last().unwrap();
                                if (f - *l).length_squared() < 1e-4 {
                                    Some(eid)
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        })
                        .collect();
                    if all_closed_splines.len() > 1 && all_closed_splines.contains(&id) {
                        return all_closed_splines;
                    }
                }
            }
        }
        vec![id]
    }

    /// Menghitung gabungan bounding box 2D (min, max) dari seluruh entitas yang terlihat di sketch.
    pub fn bounding_box(&self) -> Option<(DVec2, DVec2)> {
        let mut min_pt = DVec2::splat(f64::INFINITY);
        let mut max_pt = DVec2::splat(f64::NEG_INFINITY);
        let mut found = false;

        for (id, entity) in &self.entities {
            if self.is_hidden(id) {
                continue;
            }
            if let Some((min, max)) = entity.bounding_box() {
                min_pt = min_pt.min(min);
                max_pt = max_pt.max(max);
                found = true;
            }
        }

        if found {
            Some((min_pt, max_pt))
        } else {
            None
        }
    }
}

