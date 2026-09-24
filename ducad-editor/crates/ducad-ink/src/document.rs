use ducad_sketch::layer::LayerId;
use glam::Vec2;
use serde::{Deserialize, Serialize};
use slotmap::SlotMap;

use crate::brush::{Brush, BrushId};
use crate::stroke::Stroke;

/// Objek bounding box coretan untuk indeks spasial RTree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IndexedStroke {
    pub id: u64,
    pub min: [f32; 2],
    pub max: [f32; 2],
}

impl rstar::RTreeObject for IndexedStroke {
    type Envelope = rstar::AABB<[f32; 2]>;

    fn envelope(&self) -> Self::Envelope {
        rstar::AABB::from_corners(self.min, self.max)
    }
}

/// Indeks spasial berbasis R-Tree untuk query coretan tinta cepat.
#[derive(Debug, Clone)]
pub struct SpatialIndex {
    pub(crate) tree: rstar::RTree<IndexedStroke>,
    pub(crate) built_rev: u64,
}

impl SpatialIndex {
    pub fn tree(&self) -> &rstar::RTree<IndexedStroke> {
        &self.tree
    }
}

/// Dokumen penampung seluruh coretan tinta, kuas, dan indeks spasial.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InkDoc {
    /// Daftar coretan dalam dokumen (urutan indeks = z-order).
    pub strokes: Vec<Stroke>,
    /// Koleksi kuas yang tersedia.
    pub brushes: SlotMap<BrushId, Brush>,
    /// Alokator ID coretan berikutnya (monoton menaik).
    pub next_id: u64,
    /// Nomor revisi untuk invalidasi cache.
    #[serde(skip)]
    pub rev: u64,
    /// Indeks spasial R-Tree (dibangun malas).
    #[serde(skip)]
    index: Option<SpatialIndex>,
}

impl PartialEq for InkDoc {
    fn eq(&self, other: &Self) -> bool {
        self.strokes == other.strokes
            && self.next_id == other.next_id
            && self.brushes.len() == other.brushes.len()
            && self.brushes.iter().all(|(k, v)| other.brushes.get(k) == Some(v))
    }
}

impl Default for InkDoc {
    fn default() -> Self {
        let mut brushes = SlotMap::with_key();
        for b in Brush::presets() {
            brushes.insert(b);
        }
        Self {
            strokes: Vec::new(),
            brushes,
            next_id: 1,
            rev: 0,
            index: None,
        }
    }
}

impl InkDoc {
    /// Mengambil referensi coretan berdasarkan id.
    pub fn stroke(&self, id: u64) -> Option<&Stroke> {
        self.strokes.iter().find(|s| s.id == id)
    }

    /// Mengambil referensi mutable coretan berdasarkan id.
    pub fn stroke_mut(&mut self, id: u64) -> Option<&mut Stroke> {
        self.strokes.iter_mut().find(|s| s.id == id)
    }

    /// Menambahkan coretan baru dan mengembalikan ID-nya.
    pub fn add_stroke(&mut self, mut stroke: Stroke) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        stroke.id = id;
        stroke.recompute_bbox();
        self.strokes.push(stroke);
        self.touch();
        id
    }

    /// Menandai dokumen telah dimodifikasi (meningkatkan nomor revisi).
    pub fn touch(&mut self) {
        self.rev = self.rev.wrapping_add(1);
        self.index = None;
    }

    /// Memastikan indeks spasial terkini tersedia.
    pub fn ensure_index(&mut self) {
        if self.index.as_ref().map(|i| i.built_rev) == Some(self.rev) {
            return;
        }
        let items: Vec<IndexedStroke> = self
            .strokes
            .iter()
            .filter(|s| !s.hidden)
            .map(|s| IndexedStroke {
                id: s.id,
                min: [s.bbox.0.x, s.bbox.0.y],
                max: [s.bbox.1.x, s.bbox.1.y],
            })
            .collect();

        self.index = Some(SpatialIndex {
            tree: rstar::RTree::bulk_load(items),
            built_rev: self.rev,
        });
    }

    /// Mengembalikan ID coretan yang terlihat di dalam viewport kotak (min..max)
    /// dan layer-nya aktif terlihat, terurut berdasarkan z-order.
    pub fn visible_in(
        &self,
        min: Vec2,
        max: Vec2,
        layers_visible: &dyn Fn(LayerId) -> bool,
    ) -> Vec<u64> {
        let mut out = Vec::new();
        for s in &self.strokes {
            if s.hidden {
                continue;
            }
            if !layers_visible(s.layer) {
                continue;
            }
            let overlaps = s.bbox.0.x <= max.x
                && s.bbox.1.x >= min.x
                && s.bbox.0.y <= max.y
                && s.bbox.1.y >= min.y;
            if overlaps {
                out.push(s.id);
            }
        }
        out
    }

    /// Hit-test: mengembalikan ID coretan teratas dalam toleransi jarak `tol` (mm).
    pub fn hit(&self, p: Vec2, tol: f32) -> Option<u64> {
        for s in self.strokes.iter().rev() {
            if s.hidden {
                continue;
            }
            // Pemeriksaan awal AABB dengan toleransi
            if p.x < s.bbox.0.x - tol
                || p.x > s.bbox.1.x + tol
                || p.y < s.bbox.0.y - tol
                || p.y > s.bbox.1.y + tol
            {
                continue;
            }
            if s.points.is_empty() {
                continue;
            }
            if s.points.len() == 1 {
                if (p - s.points[0].pos()).length() <= tol {
                    return Some(s.id);
                }
                continue;
            }
            for i in 1..s.points.len() {
                let p0 = s.points[i - 1].pos();
                let p1 = s.points[i].pos();
                if dist_to_segment(p, p0, p1) <= tol {
                    return Some(s.id);
                }
            }
        }
        None
    }

    /// Perkiraan total penggunaan memori dokumen dalam byte (heap + stack).
    pub fn memory_bytes(&self) -> usize {
        let mut total = std::mem::size_of::<Self>();
        total += self.strokes.capacity() * std::mem::size_of::<Stroke>();
        for s in &self.strokes {
            total += s.points.capacity() * std::mem::size_of::<crate::stroke::InkPoint>();
        }
        total += self.brushes.capacity() * std::mem::size_of::<crate::brush::Brush>();
        total
    }
}

/// Jarak tegak lurus dari titik `p` ke ruas garis `a..b`.
fn dist_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let ap = p - a;
    let ab_len_sq = ab.length_squared();
    if ab_len_sq <= 1e-9 {
        return ap.length();
    }
    let t = (ap.dot(ab) / ab_len_sq).clamp(0.0, 1.0);
    let proj = a + ab * t;
    (p - proj).length()
}
