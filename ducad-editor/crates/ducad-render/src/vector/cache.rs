//! Cache dan batching hasil tesselasi CPU per-layer (`VectorCache` dan `LayerBatch`).
//!
//! Cache meng-invalidasi entri berdasarkan revisi entitas (`rev: u64`) dan mendukung
//! penggusuran LRU bila pemakaian memori melampaui `budget_bytes`.

use std::collections::HashMap;

use ducad_sketch::{EntityId, LayerId, Paint, Sketch, Style};

use super::tessellate::{tessellate_entity, TessOptions, Tessellated, VectorVertex};
use crate::plane::SketchPlane;

/// Batas maksimal gradien dalam satu uniform batch GPU (shader WGSL).
pub const MAX_GRADIENTS_PER_BATCH: usize = 64;

/// Stop warna pada gradien uniform GPU (std140, 32 byte).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GradientStop {
    pub offset: f32,
    pub _pad: [f32; 3],
    pub color: [f32; 4],
}

/// Struktur data uniform untuk gradien di fragment shader (std140, 288 byte).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GradientUniform {
    /// 0 = linier, 1 = radial.
    pub kind: u32,
    /// Jumlah stop warna valid (maksimal 8).
    pub count: u32,
    /// p0: titik awal (from) untuk linier, atau titik pusat (center) untuk radial.
    pub p0: [f32; 2],
    /// p1: titik akhir (to) untuk linier, atau [radius, 0.0] untuk radial.
    pub p1: [f32; 2],
    pub _pad: [f32; 2],
    /// Daftar stop warna berurutan (maksimal 8).
    pub stops: [GradientStop; 8],
}

/// Ekstrak `GradientUniform` dari cat `Paint` bila berupa gradien linier atau radial.
pub fn extract_gradient(paint: &Paint) -> Option<GradientUniform> {
    match paint {
        Paint::Solid(_) => None,
        Paint::Linear { from, to, stops } => {
            let mut uniform_stops = [GradientStop {
                offset: 0.0,
                _pad: [0.0; 3],
                color: [0.0; 4],
            }; 8];
            let count = stops.len().min(8);
            for (i, &(offset, rgba)) in stops.iter().take(8).enumerate() {
                uniform_stops[i] = GradientStop {
                    offset: offset as f32,
                    _pad: [0.0; 3],
                    color: rgba.0,
                };
            }
            Some(GradientUniform {
                kind: 0,
                count: count as u32,
                p0: [from.x as f32, from.y as f32],
                p1: [to.x as f32, to.y as f32],
                _pad: [0.0; 2],
                stops: uniform_stops,
            })
        }
        Paint::Radial {
            center,
            radius,
            stops,
        } => {
            let mut uniform_stops = [GradientStop {
                offset: 0.0,
                _pad: [0.0; 3],
                color: [0.0; 4],
            }; 8];
            let count = stops.len().min(8);
            for (i, &(offset, rgba)) in stops.iter().take(8).enumerate() {
                uniform_stops[i] = GradientStop {
                    offset: offset as f32,
                    _pad: [0.0; 3],
                    color: rgba.0,
                };
            }
            Some(GradientUniform {
                kind: 1,
                count: count as u32,
                p0: [center.x as f32, center.y as f32],
                p1: [*radius as f32, 0.0],
                _pad: [0.0; 2],
                stops: uniform_stops,
            })
        }
    }
}

/// Batch render untuk satu layer (atau sub-layer bila gradien > 64).
#[derive(Debug, Clone, PartialEq)]
pub struct LayerBatch {
    pub layer: LayerId,
    pub vertices: Vec<VectorVertex>,
    pub indices: Vec<u32>,
    pub gradients: Vec<GradientUniform>,
}

impl LayerBatch {
    pub fn new(layer: LayerId) -> Self {
        Self {
            layer,
            vertices: Vec::new(),
            indices: Vec::new(),
            gradients: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty() || self.indices.is_empty()
    }
}

/// Entri dalam cache tesselasi.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub rev: u64,
    pub tess: Tessellated,
    pub gradients: Vec<GradientUniform>,
    pub bytes: usize,
    pub last_used: u64,
}

impl CacheEntry {
    pub fn new(rev: u64, tess: Tessellated, gradients: Vec<GradientUniform>, clock: u64) -> Self {
        let bytes = std::mem::size_of::<Self>()
            + tess.vertices.len() * std::mem::size_of::<VectorVertex>()
            + tess.indices.len() * std::mem::size_of::<u32>()
            + gradients.len() * std::mem::size_of::<GradientUniform>();
        Self {
            rev,
            tess,
            gradients,
            bytes,
            last_used: clock,
        }
    }
}

/// Cache tesselasi vektor GPU dengan strategi invalidasi berbasis revisi dan LRU eviction.
pub struct VectorCache {
    pub entries: HashMap<EntityId, CacheEntry>,
    budget_bytes: usize,
    current_bytes: usize,
    clock: u64,
    /// Counter pengujian untuk menghitung jumlah aktual eksekusi fungsi tesselasi.
    pub tessellate_count: usize,
}

impl VectorCache {
    /// Buat instance cache baru dengan budget memori (mis. 64 MB = 64 * 1024 * 1024).
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            budget_bytes,
            current_bytes: 0,
            clock: 0,
            tessellate_count: 0,
        }
    }

    /// Total pemakaian memori saat ini dalam byte.
    pub fn current_bytes(&self) -> usize {
        self.current_bytes
    }

    /// Budget memori maksimum dalam byte.
    pub fn budget_bytes(&self) -> usize {
        self.budget_bytes
    }

    /// Perbarui hanya entitas yang `sketch.rev[id] != entry.rev` atau belum ada dalam cache.
    ///
    /// Menghapus entitas yang sudah tidak ada di sketch.
    /// Mengembalikan `true` bila ada perubahan (pemanggil harus meng-upload ulang buffer GPU).
    pub fn sync(
        &mut self,
        sketch: &Sketch,
        plane: &SketchPlane,
        opts: &TessOptions,
        visible: &[EntityId],
    ) -> bool {
        let mut changed = false;

        // 1. Buang entitas dari cache yang sudah dihapus dari sketch
        let stale_keys: Vec<EntityId> = self
            .entries
            .keys()
            .copied()
            .filter(|id| !sketch.entities.contains_key(*id))
            .collect();

        for id in stale_keys {
            if let Some(entry) = self.entries.remove(&id) {
                self.current_bytes = self.current_bytes.saturating_sub(entry.bytes);
                changed = true;
            }
        }

        // 2. Periksa dan tesselasi entitas yang tampak (visible)
        for &id in visible {
            let Some(entity) = sketch.entities.get(id) else {
                continue;
            };

            let cur_rev = sketch.rev.get(id).copied().unwrap_or(0);

            if let Some(entry) = self.entries.get_mut(&id) {
                if entry.rev == cur_rev {
                    // Entitas tidak berubah: perbarui stempel LRU saja
                    self.clock += 1;
                    entry.last_used = self.clock;
                    continue;
                }
            }

            // Butuh tesselasi ulang
            self.tessellate_count += 1;
            changed = true;

            let style = sketch
                .styles
                .get(id)
                .cloned()
                .unwrap_or_else(Style::cad_default);

            // Tentukan layer_index untuk penataan Z_OFFSET
            let layer_id = sketch.entity_layer.get(id).copied();
            let layer_idx = layer_id
                .and_then(|lid| sketch.layer_order.iter().position(|&l| l == lid))
                .unwrap_or(0) as u32;

            let entity_opts = TessOptions {
                layer_index: layer_idx,
                ..*opts
            };

            // Ekstrak gradien dari fill dan stroke
            let mut gradients = Vec::new();
            if let Some(ref fill) = style.fill {
                if let Some(gu) = extract_gradient(fill) {
                    gradients.push(gu);
                }
            }
            if let Some(ref stroke) = style.stroke {
                if let Some(gu) = extract_gradient(&stroke.paint) {
                    gradients.push(gu);
                }
            }

            let tess = match tessellate_entity(entity, &style, plane, &entity_opts) {
                Ok(t) => t,
                Err(e) => {
                    log::warn!("Tesselasi entitas {:?} gagal: {e}", id);
                    Tessellated::empty()
                }
            };

            // Stempel monoton PER entitas (bukan per frame): entitas yang baru
            // saja ditesselasi selalu lebih "baru" dari yang lain, sehingga
            // tidak tergusur di frame yang sama.
            self.clock += 1;
            let new_entry = CacheEntry::new(cur_rev, tess, gradients, self.clock);
            let old_bytes = self.entries.get(&id).map_or(0, |e| e.bytes);
            self.current_bytes = self
                .current_bytes
                .saturating_sub(old_bytes)
                .saturating_add(new_entry.bytes);
            self.entries.insert(id, new_entry);
        }

        // 3. Penggusuran LRU jika melebihi budget
        self.evict_over_budget();

        changed
    }

    /// Gusur entri terlama (LRU) bila total memori melebihi budget.
    fn evict_over_budget(&mut self) {
        if self.current_bytes <= self.budget_bytes {
            return;
        }

        let mut sorted_entries: Vec<(EntityId, u64, usize)> = self
            .entries
            .iter()
            .map(|(&id, entry)| (id, entry.last_used, entry.bytes))
            .collect();

        // Urutkan dari last_used paling kecil (paling lama tak dipakai);
        // `EntityId` sebagai pemecah seri agar urutan tidak bergantung pada
        // iterasi `HashMap`.
        sorted_entries.sort_by_key(|&(id, last_used, _)| (last_used, id));

        for (id, _, bytes) in sorted_entries {
            if self.current_bytes <= self.budget_bytes {
                break;
            }
            self.entries.remove(&id);
            self.current_bytes = self.current_bytes.saturating_sub(bytes);
        }
    }

    /// Gabungkan entri cache sesuai urutan `sketch.draw_order()` per layer menjadi `Vec<LayerBatch>`.
    ///
    /// Bila suatu layer memiliki lebih dari 64 gradien, batch untuk layer tersebut akan dipecah.
    /// Entitas yang disembunyikan (`sketch.is_hidden(id)` atau layer tidak visible) dilewati.
    pub fn batches(&self, sketch: &Sketch) -> Vec<LayerBatch> {
        let draw_order = sketch.draw_order();
        let mut batches: Vec<LayerBatch> = Vec::new();

        // Peta layer aktif ke indeks batch berjalan
        let mut current_layer: Option<LayerId> = None;
        let mut active_batch: Option<LayerBatch> = None;

        for id in draw_order {
            // 1. Lewati bila entitas disembunyikan secara individual
            if sketch.is_hidden(id) {
                continue;
            }

            // 2. Periksa layer entitas
            let layer_id = sketch.entity_layer.get(id).copied().unwrap_or_default();
            if let Some(layer) = sketch.layers.get(layer_id) {
                if !layer.visible {
                    continue;
                }
            }

            // 3. Ambil entri dari cache
            let Some(entry) = self.entries.get(&id) else {
                continue;
            };
            if entry.tess.is_empty() {
                continue;
            }

            // Ganti layer batch bila beralih layer
            if current_layer != Some(layer_id) {
                if let Some(batch) = active_batch.take() {
                    if !batch.is_empty() {
                        batches.push(batch);
                    }
                }
                current_layer = Some(layer_id);
                active_batch = Some(LayerBatch::new(layer_id));
            }

            let mut batch = active_batch
                .take()
                .unwrap_or_else(|| LayerBatch::new(layer_id));

            // Periksa kapasitas gradien (maksimal 64 per batch)
            if batch.gradients.len() + entry.gradients.len() > MAX_GRADIENTS_PER_BATCH
                && !batch.is_empty()
            {
                batches.push(batch);
                batch = LayerBatch::new(layer_id);
            }

            // Offset gradien dasar dalam batch ini
            let grad_base_idx = batch.gradients.len() as u32;
            batch.gradients.extend(entry.gradients.iter().copied());

            let vert_base_idx = batch.vertices.len() as u32;

            // Tambahkan vertex dengan remapping indeks gradien
            for v in &entry.tess.vertices {
                let mut vert = *v;
                if vert.paint > 0 {
                    vert.paint += grad_base_idx;
                }
                batch.vertices.push(vert);
            }

            // Tambahkan indeks segitiga
            for &idx in &entry.tess.indices {
                batch.indices.push(vert_base_idx + idx);
            }

            active_batch = Some(batch);
        }

        if let Some(batch) = active_batch {
            if !batch.is_empty() {
                batches.push(batch);
            }
        }

        batches
    }
}
