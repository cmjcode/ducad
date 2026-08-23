use glam::DVec2;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
}

impl Sketch {
    /// Entitas terdekat dari `p` dalam radius `tolerance`, atau `None`.
    pub fn hit_test(&self, p: DVec2, tolerance: f64) -> Option<EntityId> {
        self.entities
            .iter()
            .map(|(id, e)| (id, e.distance_to(p)))
            .filter(|(_, d)| *d <= tolerance)
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(id, _)| id)
    }
}
