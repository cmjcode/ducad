use glam::DVec2;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::constraint::Constraint;
use crate::entity::{Entity, EntityId};
use crate::index::SpatialIndex;
use crate::layer::{Group, GroupId, Layer, LayerId, Origin, TextId};
use crate::style::{Rgba, Style};
use crate::text::TextObject;

/// Cache indeks spasial internal untuk Sketch.
#[derive(Debug, Default)]
pub struct SpatialCache(pub(crate) std::sync::RwLock<Option<SpatialIndex>>);

impl Clone for SpatialCache {
    fn clone(&self) -> Self {
        let guard = self.0.read().ok();
        let val = guard.and_then(|g| g.clone());
        Self(std::sync::RwLock::new(val))
    }
}

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

    /// Gaya visual per entitas (fill, stroke, opacity, blend).
    #[serde(default)]
    pub styles: slotmap::SecondaryMap<EntityId, Style>,
    /// Definisi layer pada sketch.
    #[serde(default)]
    pub layers: slotmap::SlotMap<LayerId, Layer>,
    /// Urutan gambar layer dari bawah ke atas.
    #[serde(default)]
    pub layer_order: Vec<LayerId>,
    /// Asosiasi entitas ke layer-nya.
    #[serde(default)]
    pub entity_layer: slotmap::SecondaryMap<EntityId, LayerId>,
    /// Urutan gambar entitas dalam layer (z-order).
    #[serde(default)]
    pub z_order: Vec<EntityId>,
    /// Grup entitas hierarkis.
    #[serde(default)]
    pub groups: slotmap::SlotMap<GroupId, Group>,
    /// Asosiasi entitas ke grupnya.
    #[serde(default)]
    pub entity_group: slotmap::SecondaryMap<EntityId, GroupId>,
    /// Asal usul entitas (tinta coretan, impor SVG, teks).
    #[serde(default)]
    pub origin: slotmap::SecondaryMap<EntityId, Origin>,
    /// Objek teks parametrik pada sketch.
    #[serde(default)]
    pub texts: slotmap::SlotMap<TextId, TextObject>,
    /// Palet swatch warna tersimpan pada sketch/dokumen.
    #[serde(default)]
    pub swatches: Vec<Rgba>,


    /// Revisi per entitas; naik setiap kali entitas berubah lewat command.
    #[serde(skip)]
    pub rev: slotmap::SecondaryMap<EntityId, u64>,
    #[serde(skip)]
    pub global_rev: u64,

    #[serde(skip)]
    pub(crate) dirty_entities: Vec<EntityId>,
    #[serde(skip)]
    pub(crate) spatial_cache: SpatialCache,
}

impl Sketch {
    /// Wajib dipanggil setiap command yang mengubah geometri entitas `id`.
    pub fn touch(&mut self, id: EntityId) {
        self.global_rev = self.global_rev.wrapping_add(1);
        let next_rev = self.rev.get(id).copied().unwrap_or(0).wrapping_add(1);
        self.rev.insert(id, next_rev);
        self.dirty_entities.push(id);
    }

    pub fn touch_all(&mut self) {
        self.global_rev = self.global_rev.wrapping_add(1);
        for id in self.entities.keys() {
            let next_rev = self.rev.get(id).copied().unwrap_or(0).wrapping_add(1);
            self.rev.insert(id, next_rev);
        }
        self.dirty_entities.clear();
        if let Ok(mut guard) = self.spatial_cache.0.write() {
            *guard = None;
        }
    }

    /// Bangun/perbarui indeks bbox. O(k log n) untuk k entitas yang berubah sejak `built_rev`.
    pub fn spatial(&mut self) -> &SpatialIndex {
        self.ensure_spatial_index();
        let cache = self.spatial_cache.0.get_mut().unwrap();
        cache.as_ref().unwrap()
    }

    /// Query semua EntityId yang bounding box-nya beririsan dengan titik `p` yang diperluas `tol`.
    pub fn query_spatial_point(&self, p: DVec2, tol: f64) -> Vec<EntityId> {
        self.ensure_spatial_index();
        let guard = self.spatial_cache.0.read().unwrap();
        if let Some(index) = guard.as_ref() {
            index.query_point(p, tol)
        } else {
            Vec::new()
        }
    }

    /// Query semua EntityId yang bounding box-nya beririsan dengan kotak `[min, max]`.
    pub fn query_spatial_rect(&self, min: DVec2, max: DVec2) -> Vec<EntityId> {
        self.ensure_spatial_index();
        let guard = self.spatial_cache.0.read().unwrap();
        if let Some(index) = guard.as_ref() {
            index.query_rect(min, max)
        } else {
            Vec::new()
        }
    }

    fn ensure_spatial_index(&self) {
        let needs_update = {
            let guard = self.spatial_cache.0.read().unwrap();
            match guard.as_ref() {
                None => true,
                Some(idx) => {
                    idx.built_rev() != self.global_rev || idx.entity_count != self.entities.len()
                }
            }
        };

        if needs_update {
            let mut guard = self.spatial_cache.0.write().unwrap();
            let still_needs = match guard.as_ref() {
                None => true,
                Some(idx) => {
                    idx.built_rev() != self.global_rev || idx.entity_count != self.entities.len()
                }
            };
            if still_needs {
                let mut index = guard.take().unwrap_or_default();
                index.update_from_sketch(self, None);
                *guard = Some(index);
            }
        }
    }

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
    /// Menggunakan query indeks spasial sebagai prefilter, lalu uji jarak presisi.
    pub fn hit_test(&self, p: DVec2, tolerance: f64) -> Option<EntityId> {
        let candidates = self.query_spatial_point(p, tolerance);
        candidates
            .into_iter()
            .filter(|id| !self.is_hidden(*id))
            .filter_map(|id| self.entities.get(id).map(|e| (id, e.distance_to(p))))
            .filter(|(_, d)| *d <= tolerance)
            .min_by(|a, b| {
                a.1.partial_cmp(&b.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.0.cmp(&b.0))
            })
            .map(|(id, _)| id)
    }

    /// Hit-test linier murni tanpa indeks spasial (dipakai untuk validasi dan tes).
    pub fn hit_test_linear(&self, p: DVec2, tolerance: f64) -> Option<EntityId> {
        self.entities
            .iter()
            .filter(|(id, _)| !self.is_hidden(*id))
            .map(|(id, e)| (id, e.distance_to(p)))
            .filter(|(_, d)| *d <= tolerance)
            .min_by(|a, b| {
                a.1.partial_cmp(&b.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.0.cmp(&b.0))
            })
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

    /// Gaya visual entitas `id`, atau `Style::cad_default()` jika tidak memiliki style eksplisit.
    pub fn style_of(&self, id: EntityId) -> Style {
        self.styles.get(id).cloned().unwrap_or_else(Style::cad_default)
    }

    /// Mengambil LayerId dari entitas `id` jika ada.
    pub fn layer_of(&self, id: EntityId) -> Option<LayerId> {
        self.entity_layer.get(id).copied()
    }

    /// Mengecek apakah entitas dapat diedit (terlihat dan layer-nya tidak dikunci).
    pub fn is_editable(&self, id: EntityId) -> bool {
        if self.is_hidden(id) {
            return false;
        }
        if let Some(lid) = self.entity_layer.get(id) {
            if let Some(layer) = self.layers.get(*lid) {
                if !layer.visible || layer.locked {
                    return false;
                }
            }
        }
        true
    }

    /// Mengembalikan urutan gambar seluruh entitas secara deterministik berdasarkan layer_order dan z_order.
    pub fn draw_order(&self) -> Vec<EntityId> {
        let mut result = Vec::with_capacity(self.entities.len());
        let mut visited = HashSet::with_capacity(self.entities.len());

        let sort_layer_entities = |eids: Vec<EntityId>| -> Vec<EntityId> {
            let mut unz = Vec::new();
            let mut in_z = Vec::new();

            for id in eids {
                if let Some(pos) = self.z_order.iter().position(|&z| z == id) {
                    in_z.push((pos, id));
                } else {
                    unz.push(id);
                }
            }
            // Entitas tanpa entri z_order digambar lebih dulu, urutan id deterministik
            unz.sort();
            // Entitas dengan entri z_order digambar mengikuti urutan z_order
            in_z.sort_by_key(|&(pos, _)| pos);

            let mut out = unz;
            out.extend(in_z.into_iter().map(|(_, id)| id));
            out
        };

        // 1. Entitas tanpa layer (atau layer tidak terdaftar di layers)
        let unlayered: Vec<EntityId> = self
            .entities
            .keys()
            .filter(|&id| match self.entity_layer.get(id) {
                None => true,
                Some(lid) => !self.layers.contains_key(*lid),
            })
            .collect();
        for id in sort_layer_entities(unlayered) {
            visited.insert(id);
            result.push(id);
        }

        // 2. Entitas per layer sesuai urutan layer_order (bawah -> atas)
        for &lid in &self.layer_order {
            if !self.layers.contains_key(lid) {
                continue;
            }
            let layer_entities: Vec<EntityId> = self
                .entities
                .keys()
                .filter(|&id| self.entity_layer.get(id) == Some(&lid))
                .collect();
            for id in sort_layer_entities(layer_entities) {
                if visited.insert(id) {
                    result.push(id);
                }
            }
        }

        // 3. Fallback: layer yang ada di layers tapi tidak ada di layer_order
        let mut missing_layers: Vec<LayerId> = self
            .layers
            .keys()
            .filter(|lid| !self.layer_order.contains(lid))
            .collect();
        missing_layers.sort();
        for lid in missing_layers {
            let layer_entities: Vec<EntityId> = self
                .entities
                .keys()
                .filter(|&id| self.entity_layer.get(id) == Some(&lid))
                .collect();
            for id in sort_layer_entities(layer_entities) {
                if visited.insert(id) {
                    result.push(id);
                }
            }
        }

        result
    }

    /// Memastikan setidaknya ada satu layer default ("Layer 1") dan mengembalikan LayerId-nya.
    pub fn ensure_default_layer(&mut self) -> LayerId {
        for &lid in &self.layer_order {
            if self.layers.contains_key(lid) {
                return lid;
            }
        }
        if let Some(lid) = self.layers.keys().next() {
            if !self.layer_order.contains(&lid) {
                self.layer_order.push(lid);
            }
            return lid;
        }
        let layer = Layer::new("Layer 1", Rgba([1.0, 1.0, 1.0, 1.0]));
        let lid = self.layers.insert(layer);
        self.layer_order.push(lid);
        lid
    }

    /// Mengambil grup terluar (top group) dari entitas `id`, untuk seleksi klik.
    pub fn top_group(&self, id: EntityId) -> Option<GroupId> {
        let mut curr = self.entity_group.get(id).copied()?;
        while let Some(grp) = self.groups.get(curr) {
            if let Some(parent) = grp.parent {
                if self.groups.contains_key(parent) {
                    curr = parent;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        Some(curr)
    }
}

