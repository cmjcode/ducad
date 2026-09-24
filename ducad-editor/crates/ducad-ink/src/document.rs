use ducad_sketch::layer::LayerId;
use glam::Vec2;
use serde::{Deserialize, Serialize};
use slotmap::SlotMap;

use crate::brush::{Brush, BrushId};
use crate::index::SpatialIndex;
use crate::stroke::Stroke;

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
            && self
                .brushes
                .iter()
                .all(|(k, v)| other.brushes.get(k) == Some(v))
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
        self.index = Some(SpatialIndex::build(self));
    }

    /// Mengambil referensi ke indeks spasial bila sudah dibangun.
    pub fn index(&self) -> Option<&SpatialIndex> {
        self.index.as_ref()
    }

    /// Indeks spasial bila masih sesuai revisi dokumen saat ini.
    fn fresh_index(&self) -> Option<&SpatialIndex> {
        self.index.as_ref().filter(|i| i.built_rev == self.rev)
    }

    /// Mengembalikan ID coretan yang terlihat di dalam viewport kotak (min..max)
    /// dan layer-nya aktif terlihat, terurut berdasarkan z-order.
    ///
    /// Memakai indeks spasial bila segar (O(log n + k)); selain itu linear.
    /// Hasil keduanya identik.
    pub fn visible_in(
        &self,
        min: Vec2,
        max: Vec2,
        layers_visible: &dyn Fn(LayerId) -> bool,
    ) -> Vec<u64> {
        let overlaps = |s: &Stroke| {
            s.bbox.0.x <= max.x && s.bbox.1.x >= min.x && s.bbox.0.y <= max.y && s.bbox.1.y >= min.y
        };
        let keep = |s: &Stroke| !s.hidden && layers_visible(s.layer) && overlaps(s);
        match self.fresh_index() {
            Some(index) => index
                .candidates_z_ordered(min, max)
                .into_iter()
                .filter_map(|i| self.strokes.get(i))
                .filter(|s| keep(s))
                .map(|s| s.id)
                .collect(),
            None => self
                .strokes
                .iter()
                .filter(|s| keep(s))
                .map(|s| s.id)
                .collect(),
        }
    }

    /// Hit-test: mengembalikan ID coretan teratas dalam toleransi jarak `tol` (mm).
    ///
    /// Memakai indeks spasial bila segar; selain itu linear. Hasil identik.
    pub fn hit(&self, p: Vec2, tol: f32) -> Option<u64> {
        match self.fresh_index() {
            Some(index) => index
                .candidates_z_ordered(p - Vec2::splat(tol), p + Vec2::splat(tol))
                .into_iter()
                .rev()
                .filter_map(|i| self.strokes.get(i))
                .find(|s| stroke_hit(s, p, tol))
                .map(|s| s.id),
            None => self
                .strokes
                .iter()
                .rev()
                .find(|s| stroke_hit(s, p, tol))
                .map(|s| s.id),
        }
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

/// Apakah `p` berada dalam jarak `tol` dari coretan `s` (tersembunyi = tidak).
fn stroke_hit(s: &Stroke, p: Vec2, tol: f32) -> bool {
    if s.hidden
        || p.x < s.bbox.0.x - tol
        || p.x > s.bbox.1.x + tol
        || p.y < s.bbox.0.y - tol
        || p.y > s.bbox.1.y + tol
    {
        return false;
    }
    match s.points.as_slice() {
        [] => false,
        [only] => (p - only.pos()).length() <= tol,
        pts => pts
            .windows(2)
            .any(|w| dist_to_segment(p, w[0].pos(), w[1].pos()) <= tol),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stroke::InkPoint;

    fn doc_with_grid(n: usize) -> InkDoc {
        let mut doc = InkDoc::default();
        let bid = doc.brushes.keys().next().unwrap();
        for i in 0..n {
            let (x, y) = ((i % 20) as f32 * 7.0, (i / 20) as f32 * 7.0);
            let mut s = Stroke::new(
                0,
                vec![
                    InkPoint::new(x, y, 0.5, 0.0, 0),
                    InkPoint::new(x + 9.0, y + 4.0, 0.5, 0.0, 10),
                ],
                bid,
                ducad_sketch::style::Rgba([0.0, 0.0, 0.0, 1.0]),
                LayerId::default(),
            );
            s.hidden = i.is_multiple_of(7);
            doc.add_stroke(s);
        }
        doc
    }

    /// Regresi REVIEW-2026-09-24 #16: jalur indeks dan jalur linear memberi
    /// hasil identik (termasuk urutan z dan coretan tersembunyi).
    #[test]
    fn indexed_queries_match_linear() {
        let linear = doc_with_grid(200);
        let mut indexed = linear.clone();
        indexed.ensure_index();
        assert!(indexed.fresh_index().is_some());
        assert!(linear.fresh_index().is_none());

        for (min, max) in [
            (Vec2::new(0.0, 0.0), Vec2::new(30.0, 30.0)),
            (Vec2::new(50.0, 20.0), Vec2::new(51.0, 60.0)),
            (Vec2::new(-10.0, -10.0), Vec2::new(500.0, 500.0)),
        ] {
            assert_eq!(
                indexed.visible_in(min, max, &|_| true),
                linear.visible_in(min, max, &|_| true)
            );
        }
        for i in 0..60 {
            let p = Vec2::new(i as f32 * 2.3, i as f32 * 1.1);
            assert_eq!(indexed.hit(p, 1.5), linear.hit(p, 1.5), "p = {p:?}");
        }
    }
}
