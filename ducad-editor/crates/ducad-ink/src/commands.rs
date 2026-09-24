//! Implementasi seluruh Command untuk dokumen tinta `InkDoc`.
//!
//! Setiap modifikasi coretan tinta diatur melalui trait `ducad_core::undo::Command`
//! agar mendukung undo/redo deterministik dan penggabungan aksi kontinu.

use std::collections::HashSet;

use ducad_core::undo::Command;
use ducad_sketch::layer::LayerId;
use ducad_sketch::style::Rgba;
use kurbo::Affine;

use crate::document::InkDoc;
use crate::stroke::{InkPoint, Stroke};

/// Menambahkan satu coretan tinta baru ke dalam dokumen.
#[derive(Debug, Clone)]
pub struct AddStroke {
    stroke: Stroke,
    id: Option<u64>,
    index: Option<usize>,
    saved_next_id: Option<u64>,
}

impl AddStroke {
    pub fn new(stroke: Stroke) -> Self {
        Self {
            stroke,
            id: None,
            index: None,
            saved_next_id: None,
        }
    }

    pub fn stroke_id(&self) -> Option<u64> {
        self.id
    }
}

impl Command<InkDoc> for AddStroke {
    fn name(&self) -> &str {
        "Tambah Coretan"
    }

    fn apply(&mut self, doc: &mut InkDoc) {
        if let Some(id) = self.id {
            // Redo: pasang kembali coretan dengan ID aslinya
            self.stroke.id = id;
            self.stroke.recompute_bbox();
            let idx = self
                .index
                .unwrap_or(doc.strokes.len())
                .min(doc.strokes.len());
            doc.strokes.insert(idx, self.stroke.clone());
        } else {
            // Penerapan pertama: alokasikan next_id dari dokumen
            self.saved_next_id = Some(doc.next_id);
            let id = doc.next_id;
            doc.next_id += 1;
            self.stroke.id = id;
            self.stroke.recompute_bbox();
            self.id = Some(id);
            self.index = Some(doc.strokes.len());
            doc.strokes.push(self.stroke.clone());
        }
        doc.touch();
    }

    fn revert(&mut self, doc: &mut InkDoc) {
        if let Some(id) = self.id {
            if let Some(pos) = doc.strokes.iter().position(|s| s.id == id) {
                self.index = Some(pos);
                self.stroke = doc.strokes.remove(pos);
            }
            if let Some(prev) = self.saved_next_id {
                if doc.next_id == prev + 1 {
                    doc.next_id = prev;
                }
            }
            doc.touch();
        }
    }
}

/// Menghapus kumpulan coretan dari dokumen berdasarkan ID.
#[derive(Debug, Clone)]
pub struct DeleteStrokes {
    pub ids: Vec<u64>,
    saved: Vec<(usize, Stroke)>,
}

impl DeleteStrokes {
    pub fn new(ids: Vec<u64>) -> Self {
        Self {
            ids,
            saved: Vec::new(),
        }
    }
}

impl Command<InkDoc> for DeleteStrokes {
    fn name(&self) -> &str {
        "Hapus Coretan"
    }

    fn apply(&mut self, doc: &mut InkDoc) {
        // Uji keanggotaan O(1); iterasi tetap atas `strokes` (deterministik).
        let wanted: HashSet<u64> = self.ids.iter().copied().collect();
        self.saved.clear();
        let mut remaining = Vec::with_capacity(doc.strokes.len());
        for (idx, stroke) in doc.strokes.drain(..).enumerate() {
            if wanted.contains(&stroke.id) {
                self.saved.push((idx, stroke));
            } else {
                remaining.push(stroke);
            }
        }
        doc.strokes = remaining;
        doc.touch();
    }

    fn revert(&mut self, doc: &mut InkDoc) {
        for (idx, stroke) in self.saved.drain(..) {
            let pos = idx.min(doc.strokes.len());
            doc.strokes.insert(pos, stroke);
        }
        doc.touch();
    }
}

/// Mentransformasikan koordinat kumpulan coretan menggunakan matriks afina.
#[derive(Debug, Clone)]
pub struct TransformStrokes {
    pub ids: Vec<u64>,
    pub affine: Affine,
    saved: Vec<(u64, Vec<InkPoint>)>,
}

impl TransformStrokes {
    pub fn new(ids: Vec<u64>, affine: Affine) -> Self {
        Self {
            ids,
            affine,
            saved: Vec::new(),
        }
    }
}

impl Command<InkDoc> for TransformStrokes {
    fn name(&self) -> &str {
        "Transformasi Coretan"
    }

    fn coalesce_key(&self) -> Option<(&'static str, u64)> {
        self.ids.first().copied().map(|id| ("ink-move", id))
    }

    fn apply(&mut self, doc: &mut InkDoc) {
        // Uji keanggotaan O(1); iterasi tetap atas `strokes` (deterministik).
        let wanted: HashSet<u64> = self.ids.iter().copied().collect();
        let is_first = self.saved.is_empty();
        for s in &mut doc.strokes {
            if wanted.contains(&s.id) {
                if is_first {
                    self.saved.push((s.id, s.points.clone()));
                }
                *s = s.transformed(self.affine);
            }
        }
        doc.touch();
    }

    fn revert(&mut self, doc: &mut InkDoc) {
        for (id, pts) in &self.saved {
            if let Some(s) = doc.stroke_mut(*id) {
                s.points = pts.clone();
                s.recompute_bbox();
            }
        }
        doc.touch();
    }
}

/// Mengubah status sembunyi/tampil kumpulan coretan (misal setelah Rapikan).
#[derive(Debug, Clone)]
pub struct SetStrokesHidden {
    pub ids: Vec<u64>,
    pub hidden: bool,
    saved: Vec<(u64, bool)>,
}

impl SetStrokesHidden {
    pub fn new(ids: Vec<u64>, hidden: bool) -> Self {
        Self {
            ids,
            hidden,
            saved: Vec::new(),
        }
    }
}

impl Command<InkDoc> for SetStrokesHidden {
    fn name(&self) -> &str {
        if self.hidden {
            "Sembunyikan Coretan"
        } else {
            "Tampilkan Coretan"
        }
    }

    fn apply(&mut self, doc: &mut InkDoc) {
        // Uji keanggotaan O(1); iterasi tetap atas `strokes` (deterministik).
        let wanted: HashSet<u64> = self.ids.iter().copied().collect();
        self.saved.clear();
        for s in &mut doc.strokes {
            if wanted.contains(&s.id) {
                self.saved.push((s.id, s.hidden));
                s.hidden = self.hidden;
            }
        }
        doc.touch();
    }

    fn revert(&mut self, doc: &mut InkDoc) {
        for (id, hidden) in &self.saved {
            if let Some(s) = doc.stroke_mut(*id) {
                s.hidden = *hidden;
            }
        }
        doc.touch();
    }
}

/// Mengubah warna kuas dari kumpulan coretan.
#[derive(Debug, Clone)]
pub struct SetStrokeColor {
    pub ids: Vec<u64>,
    pub color: Rgba,
    saved: Vec<(u64, Rgba)>,
}

impl SetStrokeColor {
    pub fn new(ids: Vec<u64>, color: Rgba) -> Self {
        Self {
            ids,
            color,
            saved: Vec::new(),
        }
    }
}

impl Command<InkDoc> for SetStrokeColor {
    fn name(&self) -> &str {
        "Ubah Warna Coretan"
    }

    fn apply(&mut self, doc: &mut InkDoc) {
        // Uji keanggotaan O(1); iterasi tetap atas `strokes` (deterministik).
        let wanted: HashSet<u64> = self.ids.iter().copied().collect();
        self.saved.clear();
        for s in &mut doc.strokes {
            if wanted.contains(&s.id) {
                self.saved.push((s.id, s.color));
                s.color = self.color;
            }
        }
        doc.touch();
    }

    fn revert(&mut self, doc: &mut InkDoc) {
        for (id, color) in &self.saved {
            if let Some(s) = doc.stroke_mut(*id) {
                s.color = *color;
            }
        }
        doc.touch();
    }
}

/// Memotong satu coretan menjadi beberapa pecahan coretan baru pada indeks titik tertentu.
#[derive(Debug, Clone)]
pub struct SplitStroke {
    pub id: u64,
    pub at: Vec<usize>,
    pub new_ids: Vec<u64>,
    saved_stroke: Option<(usize, Stroke)>,
    saved_next_id: Option<u64>,
}

impl SplitStroke {
    pub fn new(id: u64, at: Vec<usize>) -> Self {
        Self {
            id,
            at,
            new_ids: Vec::new(),
            saved_stroke: None,
            saved_next_id: None,
        }
    }

    pub fn new_ids(&self) -> &[u64] {
        &self.new_ids
    }
}

impl Command<InkDoc> for SplitStroke {
    fn name(&self) -> &str {
        "Potong Coretan"
    }

    fn apply(&mut self, doc: &mut InkDoc) {
        let pos = match doc.strokes.iter().position(|s| s.id == self.id) {
            Some(p) => p,
            None => return,
        };
        let orig = doc.strokes.remove(pos);
        let n_pts = orig.points.len();

        let mut cut_indices: Vec<usize> = self
            .at
            .iter()
            .copied()
            .filter(|&i| i > 0 && i < n_pts)
            .collect();
        cut_indices.sort_unstable();
        cut_indices.dedup();

        if cut_indices.is_empty() {
            doc.strokes.insert(pos, orig);
            return;
        }

        let mut segments: Vec<Vec<InkPoint>> = Vec::new();
        let mut prev = 0;
        for &idx in &cut_indices {
            segments.push(orig.points[prev..idx].to_vec());
            prev = idx;
        }
        if prev < n_pts {
            segments.push(orig.points[prev..n_pts].to_vec());
        }

        let allocate_ids = self.new_ids.len() != segments.len();
        if allocate_ids {
            self.saved_next_id = Some(doc.next_id);
            self.new_ids.clear();
        }

        let mut new_strokes = Vec::with_capacity(segments.len());
        for (i, seg) in segments.into_iter().enumerate() {
            let stroke_id = if allocate_ids {
                let id = doc.next_id;
                doc.next_id += 1;
                self.new_ids.push(id);
                id
            } else {
                self.new_ids[i]
            };
            let mut s = Stroke::new(stroke_id, seg, orig.brush, orig.color, orig.layer);
            s.hidden = orig.hidden;
            new_strokes.push(s);
        }

        for (offset, s) in new_strokes.into_iter().enumerate() {
            doc.strokes.insert(pos + offset, s);
        }

        self.saved_stroke = Some((pos, orig));
        doc.touch();
    }

    fn revert(&mut self, doc: &mut InkDoc) {
        if let Some((pos, orig)) = self.saved_stroke.take() {
            doc.strokes.retain(|s| !self.new_ids.contains(&s.id));
            let insert_pos = pos.min(doc.strokes.len());
            doc.strokes.insert(insert_pos, orig);
            if let Some(prev) = self.saved_next_id {
                if doc.next_id == prev + self.new_ids.len() as u64 {
                    doc.next_id = prev;
                }
            }
            doc.touch();
        }
    }
}

/// Mengganti titik-titik pada satu coretan (misal untuk nudge atau eraser sebagian).
#[derive(Debug, Clone)]
pub struct ReplacePoints {
    pub id: u64,
    pub points: Vec<InkPoint>,
    saved: Option<Vec<InkPoint>>,
}

impl ReplacePoints {
    pub fn new(id: u64, points: Vec<InkPoint>) -> Self {
        Self {
            id,
            points,
            saved: None,
        }
    }
}

impl Command<InkDoc> for ReplacePoints {
    fn name(&self) -> &str {
        "Ganti Titik Coretan"
    }

    fn apply(&mut self, doc: &mut InkDoc) {
        if let Some(s) = doc.stroke_mut(self.id) {
            self.saved = Some(s.points.clone());
            s.points = self.points.clone();
            s.recompute_bbox();
            doc.touch();
        }
    }

    fn revert(&mut self, doc: &mut InkDoc) {
        if let Some(ref old_pts) = self.saved {
            if let Some(s) = doc.stroke_mut(self.id) {
                s.points = old_pts.clone();
                s.recompute_bbox();
                doc.touch();
            }
        }
    }
}

/// Memindahkan kumpulan coretan ke layer lain.
#[derive(Debug, Clone)]
pub struct MoveStrokesToLayer {
    pub ids: Vec<u64>,
    pub layer: LayerId,
    saved: Vec<(u64, LayerId)>,
}

impl MoveStrokesToLayer {
    pub fn new(ids: Vec<u64>, layer: LayerId) -> Self {
        Self {
            ids,
            layer,
            saved: Vec::new(),
        }
    }
}

impl Command<InkDoc> for MoveStrokesToLayer {
    fn name(&self) -> &str {
        "Pindah Coretan ke Layer"
    }

    fn apply(&mut self, doc: &mut InkDoc) {
        // Uji keanggotaan O(1); iterasi tetap atas `strokes` (deterministik).
        let wanted: HashSet<u64> = self.ids.iter().copied().collect();
        self.saved.clear();
        for s in &mut doc.strokes {
            if wanted.contains(&s.id) {
                self.saved.push((s.id, s.layer));
                s.layer = self.layer;
            }
        }
        doc.touch();
    }

    fn revert(&mut self, doc: &mut InkDoc) {
        for (id, layer) in &self.saved {
            if let Some(s) = doc.stroke_mut(*id) {
                s.layer = *layer;
            }
        }
        doc.touch();
    }
}
