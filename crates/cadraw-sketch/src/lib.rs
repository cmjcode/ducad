//! Sketching 2D CADRAW: entitas, hit-testing, snapping, dan command
//! undo/redo. Fase 2 menambah constraint solver di atas modul ini.
//!
//! Lingkup Fase 1 (sengaja dibatasi, bukan lupa): Line/Circle/Arc, snap
//! endpoint/midpoint/center/intersection/grid, tool Line/Rectangle/Circle.
//! Ellipse/spline/fillet-2D/trim/extend/offset/mirror dan constraint
//! menyusul di Fase 1 lanjutan & Fase 2 — belum ada di sini.

use cadraw_core::Command;
use glam::DVec2;
use std::f64::consts::TAU;

slotmap::new_key_type! {
    /// Identitas stabil entitas sketch.
    pub struct EntityId;
}

/// Entitas sketch 2D (koordinat lokal bidang sketch, presisi f64).
#[derive(Debug, Clone, PartialEq)]
pub enum Entity {
    Line {
        start: DVec2,
        end: DVec2,
    },
    Circle {
        center: DVec2,
        radius: f64,
    },
    Arc {
        center: DVec2,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    },
}

impl Entity {
    /// Titik-titik endpoint sebagai kandidat snap "endpoint".
    pub fn endpoints(&self) -> Vec<DVec2> {
        match self {
            Entity::Line { start, end } => vec![*start, *end],
            Entity::Circle { .. } => vec![],
            Entity::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => vec![
                *center + DVec2::new(radius * start_angle.cos(), radius * start_angle.sin()),
                *center + DVec2::new(radius * end_angle.cos(), radius * end_angle.sin()),
            ],
        }
    }

    pub fn midpoint(&self) -> Option<DVec2> {
        match self {
            Entity::Line { start, end } => Some((*start + *end) * 0.5),
            _ => None,
        }
    }

    pub fn center(&self) -> Option<DVec2> {
        match self {
            Entity::Circle { center, .. } | Entity::Arc { center, .. } => Some(*center),
            Entity::Line { .. } => None,
        }
    }

    /// Jarak titik ke entitas — dipakai hit-testing seleksi & (nanti)
    /// snap "nearest on entity".
    pub fn distance_to(&self, p: DVec2) -> f64 {
        match self {
            Entity::Line { start, end } => distance_point_segment(p, *start, *end),
            Entity::Circle { center, radius } => ((p - *center).length() - radius).abs(),
            Entity::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                let to_p = p - *center;
                let angle = to_p.y.atan2(to_p.x);
                if angle_in_range(angle, *start_angle, *end_angle) {
                    (to_p.length() - radius).abs()
                } else {
                    self.endpoints()
                        .into_iter()
                        .map(|e| (p - e).length())
                        .fold(f64::INFINITY, f64::min)
                }
            }
        }
    }
}

fn distance_point_segment(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq < f64::EPSILON {
        return (p - a).length();
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

/// `true` bila sudut (radian, bebas rentang) berada di antara `start` dan
/// `end` mengikuti arah CCW baku, termasuk kasus rentang yang membungkus 0.
fn angle_in_range(angle: f64, start: f64, end: f64) -> bool {
    let norm = |a: f64| ((a % TAU) + TAU) % TAU;
    let (a, s, e) = (norm(angle), norm(start), norm(end));
    if s <= e {
        a >= s && a <= e
    } else {
        a >= s || a <= e
    }
}

/// Satu sketch pada sebuah bidang kerja.
#[derive(Debug, Default)]
pub struct Sketch {
    pub entities: slotmap::SlotMap<EntityId, Entity>,
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

// ---------------------------------------------------------------------
// Snapping
// ---------------------------------------------------------------------

/// Jenis snap — dipakai UI untuk memilih glyph indikator yang sesuai.
/// Urutan varian = urutan prioritas pencarian di `find_snap`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapKind {
    Endpoint,
    Midpoint,
    Center,
    Intersection,
    Grid,
}

#[derive(Debug, Clone, Copy)]
pub struct SnapHit {
    pub point: DVec2,
    pub kind: SnapKind,
}

/// Cari titik snap terbaik di sekitar kursor. Prioritas: endpoint >
/// midpoint > center > intersection > grid — titik geometris eksak
/// didahulukan atas grid, konsisten dengan konvensi AutoCAD.
///
/// `tolerance` dan `grid_step` dalam unit dunia sketch (mm); pemanggil
/// bertanggung jawab mengonversi toleransi piksel-ke-dunia (adaptif
/// mouse vs sentuh disempurnakan di Fase 4).
pub fn find_snap(
    sketch: &Sketch,
    cursor: DVec2,
    tolerance: f64,
    grid_step: f64,
    exclude: Option<EntityId>,
) -> Option<SnapHit> {
    let nearest = |kind: SnapKind, pts: Vec<DVec2>| -> Option<SnapHit> {
        pts.into_iter()
            .map(|p| (p, (p - cursor).length()))
            .filter(|(_, d)| *d <= tolerance)
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(point, _)| SnapHit { point, kind })
    };

    let others = || {
        sketch
            .entities
            .iter()
            .filter(move |(id, _)| Some(*id) != exclude)
    };

    if let Some(hit) = nearest(
        SnapKind::Endpoint,
        others().flat_map(|(_, e)| e.endpoints()).collect(),
    ) {
        return Some(hit);
    }
    if let Some(hit) = nearest(
        SnapKind::Midpoint,
        others().filter_map(|(_, e)| e.midpoint()).collect(),
    ) {
        return Some(hit);
    }
    if let Some(hit) = nearest(
        SnapKind::Center,
        others().filter_map(|(_, e)| e.center()).collect(),
    ) {
        return Some(hit);
    }
    if let Some(hit) = nearest(SnapKind::Intersection, find_intersections(sketch, exclude)) {
        return Some(hit);
    }

    let snapped = DVec2::new(
        (cursor.x / grid_step).round() * grid_step,
        (cursor.y / grid_step).round() * grid_step,
    );
    ((snapped - cursor).length() <= tolerance).then_some(SnapHit {
        point: snapped,
        kind: SnapKind::Grid,
    })
}

/// Titik potong antar entitas Line (Fase 1: line-line saja; arc/circle
/// menyusul saat use-case-nya muncul).
fn find_intersections(sketch: &Sketch, exclude: Option<EntityId>) -> Vec<DVec2> {
    let lines: Vec<(DVec2, DVec2)> = sketch
        .entities
        .iter()
        .filter(|(id, _)| Some(*id) != exclude)
        .filter_map(|(_, e)| match e {
            Entity::Line { start, end } => Some((*start, *end)),
            _ => None,
        })
        .collect();

    let mut pts = Vec::new();
    for i in 0..lines.len() {
        for j in (i + 1)..lines.len() {
            if let Some(p) = line_intersection(lines[i], lines[j]) {
                pts.push(p);
            }
        }
    }
    pts
}

fn line_intersection((a1, a2): (DVec2, DVec2), (b1, b2): (DVec2, DVec2)) -> Option<DVec2> {
    let r = a2 - a1;
    let s = b2 - b1;
    let denom = r.x * s.y - r.y * s.x;
    if denom.abs() < 1e-9 {
        return None; // sejajar / berimpit
    }
    let diff = b1 - a1;
    let t = (diff.x * s.y - diff.y * s.x) / denom;
    let u = (diff.x * r.y - diff.y * r.x) / denom;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        Some(a1 + r * t)
    } else {
        None
    }
}

// ---------------------------------------------------------------------
// Command undo/redo
// ---------------------------------------------------------------------

/// Alias nyaman: tumpukan undo/redo khusus operasi sketch.
pub type UndoStack = cadraw_core::UndoStack<Sketch>;

/// Sisipkan satu atau lebih entitas sebagai satu langkah undo — mis. tool
/// Rectangle menghasilkan 4 Line yang harus di-undo sekaligus.
pub struct InsertEntities {
    entities: Vec<Entity>,
    inserted_ids: Vec<EntityId>,
    label: &'static str,
}

impl InsertEntities {
    pub fn new(label: &'static str, entities: Vec<Entity>) -> Self {
        Self {
            entities,
            inserted_ids: Vec::new(),
            label,
        }
    }
}

impl Command<Sketch> for InsertEntities {
    fn name(&self) -> &str {
        self.label
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        self.inserted_ids = self
            .entities
            .iter()
            .cloned()
            .map(|e| sketch.entities.insert(e))
            .collect();
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        for id in self.inserted_ids.drain(..) {
            sketch.entities.remove(id);
        }
    }
}

/// Hapus entitas terpilih.
///
/// Catatan: `revert` menyisipkan ulang entitas dengan `EntityId` BARU
/// (slotmap tidak menjamin key lama bisa dipakai lagi secara umum). Untuk
/// Fase 1 ini cukup — pemanggil (UI) diharapkan mengosongkan seleksi
/// setelah delete, jadi tidak ada referensi id lama yang bergantung pada
/// kestabilan key lintas undo.
pub struct DeleteEntities {
    ids: Vec<EntityId>,
    removed: Vec<Entity>,
}

impl DeleteEntities {
    pub fn new(ids: Vec<EntityId>) -> Self {
        Self {
            ids,
            removed: Vec::new(),
        }
    }
}

impl Command<Sketch> for DeleteEntities {
    fn name(&self) -> &str {
        "Hapus"
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        self.removed = self
            .ids
            .iter()
            .filter_map(|id| sketch.entities.remove(*id))
            .collect();
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        for entity in self.removed.drain(..) {
            sketch.entities.insert(entity);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_test_finds_nearest_line() {
        let mut sketch = Sketch::default();
        sketch.entities.insert(Entity::Line {
            start: DVec2::new(0.0, 0.0),
            end: DVec2::new(10.0, 0.0),
        });
        assert!(sketch.hit_test(DVec2::new(5.0, 0.3), 0.5).is_some());
        assert!(sketch.hit_test(DVec2::new(5.0, 5.0), 0.5).is_none());
    }

    #[test]
    fn snap_prefers_endpoint_over_grid() {
        let mut sketch = Sketch::default();
        sketch.entities.insert(Entity::Line {
            start: DVec2::new(10.2, 0.1),
            end: DVec2::new(20.0, 0.0),
        });
        // Kursor dekat endpoint (10.2, 0.1), grid step 10 → grid terdekat
        // (10, 0) juga dalam toleransi, tapi endpoint harus menang.
        let hit = find_snap(&sketch, DVec2::new(10.0, 0.0), 2.0, 10.0, None).unwrap();
        assert_eq!(hit.kind, SnapKind::Endpoint);
        assert!((hit.point - DVec2::new(10.2, 0.1)).length() < 1e-9);
    }

    #[test]
    fn snap_falls_back_to_grid() {
        let sketch = Sketch::default();
        let hit = find_snap(&sketch, DVec2::new(19.6, 0.2), 1.0, 10.0, None).unwrap();
        assert_eq!(hit.kind, SnapKind::Grid);
        assert_eq!(hit.point, DVec2::new(20.0, 0.0));
    }

    #[test]
    fn snap_finds_line_intersection() {
        let mut sketch = Sketch::default();
        // Dua garis yang berpotongan bukan di titik tengah keduanya —
        // supaya snap "midpoint" (prioritas lebih tinggi) tidak kebetulan
        // bertumpuk dengan titik intersection dan membuat test ambigu.
        sketch.entities.insert(Entity::Line {
            start: DVec2::new(-5.0, 0.0),
            end: DVec2::new(15.0, 0.0),
        });
        sketch.entities.insert(Entity::Line {
            start: DVec2::new(0.0, -5.0),
            end: DVec2::new(0.0, 15.0),
        });
        let hit = find_snap(&sketch, DVec2::new(0.3, 0.3), 1.0, 1000.0, None).unwrap();
        assert_eq!(hit.kind, SnapKind::Intersection);
        assert!(hit.point.length() < 1e-9);
    }

    #[test]
    fn insert_and_delete_undo_roundtrip() {
        let mut sketch = Sketch::default();
        let mut undo = UndoStack::default();

        undo.execute(
            Box::new(InsertEntities::new(
                "Garis",
                vec![Entity::Line {
                    start: DVec2::ZERO,
                    end: DVec2::new(1.0, 0.0),
                }],
            )),
            &mut sketch,
        );
        assert_eq!(sketch.entities.len(), 1);

        undo.undo(&mut sketch);
        assert_eq!(sketch.entities.len(), 0);
        undo.redo(&mut sketch);
        assert_eq!(sketch.entities.len(), 1);

        let id = sketch.entities.keys().next().unwrap();
        undo.execute(Box::new(DeleteEntities::new(vec![id])), &mut sketch);
        assert_eq!(sketch.entities.len(), 0);
        undo.undo(&mut sketch);
        assert_eq!(sketch.entities.len(), 1);
    }
}
