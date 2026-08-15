//! Sketching 2D CADRAW: entitas, hit-testing, snapping, operasi modify
//! (offset/mirror/trim), constraint solver (lihat modul `constraint`), dan
//! command undo/redo.
//!
//! Lingkup Fase 1 lanjutan (sengaja dibatasi, bukan lupa): Line/Circle/
//! Arc/Ellipse, snap endpoint/midpoint/center/intersection/grid, offset
//! (Line/Circle/Arc — Ellipse belum, lihat `offset_entity`), mirror, dan
//! trim (Line-vs-Line saja). Spline, fillet 2D, dan extend menyusul di
//! iterasi berikutnya — belum ada di sini.

pub mod constraint;
pub mod measure;

use cadraw_core::Command;
use glam::DVec2;
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

slotmap::new_key_type! {
    /// Identitas stabil entitas sketch.
    pub struct EntityId;
}

/// Entitas sketch 2D (koordinat lokal bidang sketch, presisi f64).
///
/// `Serialize`/`Deserialize` dipakai format native `.cadraw` (Fase 5,
/// `cadraw-io`) — derive langsung di sini, bukan struct salinan di
/// `cadraw-io`, supaya tidak ada dua sumber kebenaran untuk bentuk entitas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// Ellips axis-aligned (sumbu sejajar X/Y). Ellips berotasi belum
    /// didukung — lihat catatan keterbatasan di `mirror_entity`.
    Ellipse {
        center: DVec2,
        radius_x: f64,
        radius_y: f64,
    },
}

impl Entity {
    /// Titik-titik endpoint sebagai kandidat snap "endpoint".
    pub fn endpoints(&self) -> Vec<DVec2> {
        match self {
            Entity::Line { start, end } => vec![*start, *end],
            Entity::Circle { .. } | Entity::Ellipse { .. } => vec![],
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
            Entity::Circle { center, .. }
            | Entity::Arc { center, .. }
            | Entity::Ellipse { center, .. } => Some(*center),
            Entity::Line { .. } => None,
        }
    }

    /// Sama seperti `endpoints()`, tapi berpasangan dengan `PointRef`
    /// sumbernya — dipakai `find_snap` supaya UI bisa membangun constraint
    /// Coincident/Fixed/Symmetric langsung dari hasil snap, bukan cuma
    /// titik mentah. Endpoint Arc sengaja tidak muncul di sini: `PointRef`
    /// belum punya varian untuknya (lihat catatan lingkup di modul
    /// `constraint`), jadi snap ke endpoint Arc tidak bisa jadi sumber
    /// constraint titik — akan tetap tampil sebagai snap Endpoint biasa,
    /// hanya tidak membawa `source`.
    pub fn endpoint_refs(&self, id: EntityId) -> Vec<(constraint::PointRef, DVec2)> {
        match self {
            Entity::Line { start, end } => vec![
                (constraint::PointRef::LineStart(id), *start),
                (constraint::PointRef::LineEnd(id), *end),
            ],
            _ => vec![],
        }
    }

    /// Sama seperti `center()`, berpasangan dengan `PointRef::Center`.
    pub fn center_ref(&self, id: EntityId) -> Option<(constraint::PointRef, DVec2)> {
        self.center().map(|c| (constraint::PointRef::Center(id), c))
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
            Entity::Ellipse {
                center,
                radius_x,
                radius_y,
            } => {
                // Tidak ada rumus tertutup untuk jarak titik-ke-ellips;
                // aproksimasi dengan sampling batas — cukup akurat untuk
                // toleransi hit-test pada skala sketch (mm).
                const SAMPLES: usize = 64;
                (0..SAMPLES)
                    .map(|i| {
                        let t = TAU * (i as f64) / (SAMPLES as f64);
                        let boundary =
                            *center + DVec2::new(radius_x * t.cos(), radius_y * t.sin());
                        (p - boundary).length()
                    })
                    .fold(f64::INFINITY, f64::min)
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
///
/// `Serialize`/`Deserialize` (Fase 5): berkat `slotmap` di-build dengan
/// fitur "serde", `SlotMap<EntityId, Entity>` di-roundtrip APA ADANYA —
/// index+versi internal ikut tersimpan, jadi `EntityId` yang dibaca balik
/// sama persis dengan sebelum disimpan. Itu sebabnya `constraints` (yang
/// menyimpan `EntityId` mentah lewat `PointRef`) tidak butuh remapping id
/// manual sama sekali — lihat `cadraw-io::native`.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Sketch {
    pub entities: slotmap::SlotMap<EntityId, Entity>,
    /// Constraint aktif (lihat modul `constraint`) — solver menulis balik
    /// geometri `entities` di atas saat constraint berubah.
    pub constraints: Vec<constraint::Constraint>,
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
    /// Rujukan titik sumber (entitas + Start/End/Center) bila snap ini
    /// berasal dari titik yang benar-benar jadi DOF entitas — dipakai UI
    /// membangun constraint Coincident/Fixed/Symmetric langsung dari hasil
    /// snap. `None` untuk Midpoint/Intersection/Grid (bukan DOF tunggal,
    /// melainkan turunan dari titik lain) dan untuk endpoint Arc
    /// (`PointRef` belum mencakupnya — lihat `Entity::endpoint_refs`).
    pub source: Option<constraint::PointRef>,
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
    let nearest = |kind: SnapKind, pts: Vec<(DVec2, Option<constraint::PointRef>)>| -> Option<SnapHit> {
        pts.into_iter()
            .map(|(p, src)| (p, src, (p - cursor).length()))
            .filter(|(_, _, d)| *d <= tolerance)
            .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap())
            .map(|(point, source, _)| SnapHit { point, kind, source })
    };

    let others = || {
        sketch
            .entities
            .iter()
            .filter(move |(id, _)| Some(*id) != exclude)
    };

    if let Some(hit) = nearest(
        SnapKind::Endpoint,
        others()
            .flat_map(|(id, e)| e.endpoint_refs(id))
            .map(|(r, p)| (p, Some(r)))
            .collect(),
    ) {
        return Some(hit);
    }
    if let Some(hit) = nearest(
        SnapKind::Midpoint,
        others()
            .filter_map(|(_, e)| e.midpoint())
            .map(|p| (p, None))
            .collect(),
    ) {
        return Some(hit);
    }
    if let Some(hit) = nearest(
        SnapKind::Center,
        others()
            .filter_map(|(id, e)| e.center_ref(id))
            .map(|(r, p)| (p, Some(r)))
            .collect(),
    ) {
        return Some(hit);
    }
    if let Some(hit) = nearest(
        SnapKind::Intersection,
        find_intersections(sketch, exclude)
            .into_iter()
            .map(|p| (p, None))
            .collect(),
    ) {
        return Some(hit);
    }

    let snapped = DVec2::new(
        (cursor.x / grid_step).round() * grid_step,
        (cursor.y / grid_step).round() * grid_step,
    );
    ((snapped - cursor).length() <= tolerance).then_some(SnapHit {
        point: snapped,
        kind: SnapKind::Grid,
        source: None,
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

fn line_intersection(a: (DVec2, DVec2), b: (DVec2, DVec2)) -> Option<DVec2> {
    let (t, _u) = line_intersection_params(a, b)?;
    Some(a.0 + (a.1 - a.0) * t)
}

/// Parameter `(t, u)` — posisi perpotongan sepanjang segmen `a` dan `b`
/// masing-masing (0..1 di dalam segmen). `None` bila sejajar/berimpit atau
/// perpotongan jatuh di luar salah satu segmen.
fn line_intersection_params(
    (a1, a2): (DVec2, DVec2),
    (b1, b2): (DVec2, DVec2),
) -> Option<(f64, f64)> {
    let r = a2 - a1;
    let s = b2 - b1;
    let denom = r.x * s.y - r.y * s.x;
    if denom.abs() < 1e-9 {
        return None; // sejajar / berimpit
    }
    let diff = b1 - a1;
    let t = (diff.x * s.y - diff.y * s.x) / denom;
    let u = (diff.x * r.y - diff.y * r.x) / denom;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some((t, u))
}

// ---------------------------------------------------------------------
// Konstruksi & modifikasi geometri (Arc 3-titik, Offset, Mirror, Trim)
// ---------------------------------------------------------------------

/// Bangun Arc yang melalui tiga titik: `p1` jadi salah satu ujung, `p3`
/// ujung lainnya, `p2` menentukan sisi mana yang jadi busur (arc akan
/// melewati sudut `p2` saat berjalan CCW dari salah satu ujung ke ujung
/// lain). `None` bila ketiganya kolinear (tak ada lingkaran unik).
pub fn arc_from_three_points(p1: DVec2, p2: DVec2, p3: DVec2) -> Option<Entity> {
    let center = circumcenter(p1, p2, p3)?;
    let radius = (p1 - center).length();
    let angle_of = |p: DVec2| (p - center).y.atan2((p - center).x);
    let (a1, a2, a3) = (angle_of(p1), angle_of(p2), angle_of(p3));

    let ccw_span = |from: f64, to: f64| {
        let d = to - from;
        if d < 0.0 {
            d + TAU
        } else {
            d
        }
    };

    // p2 ada di busur CCW(a1→a3) jika jarak CCW a1→a2 lebih pendek dari
    // a1→a3; kalau tidak, p2 ada di busur komplemennya (CCW a3→a1).
    let (start_angle, end_angle) = if ccw_span(a1, a2) <= ccw_span(a1, a3) {
        (a1, a3)
    } else {
        (a3, a1)
    };

    Some(Entity::Arc {
        center,
        radius,
        start_angle,
        end_angle,
    })
}

fn circumcenter(p1: DVec2, p2: DVec2, p3: DVec2) -> Option<DVec2> {
    let (ax, ay) = (p1.x, p1.y);
    let (bx, by) = (p2.x, p2.y);
    let (cx, cy) = (p3.x, p3.y);
    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-9 {
        return None; // kolinear
    }
    let a2 = ax * ax + ay * ay;
    let b2 = bx * bx + by * by;
    let c2 = cx * cx + cy * cy;
    let ux = (a2 * (by - cy) + b2 * (cy - ay) + c2 * (ay - by)) / d;
    let uy = (a2 * (cx - bx) + b2 * (ax - cx) + c2 * (bx - ax)) / d;
    Some(DVec2::new(ux, uy))
}

/// Bangun entitas sejajar `entity` yang melalui (atau mendekati)
/// `reference_point` — dipakai tool Offset. Klik kira-kira di mana hasil
/// offset diinginkan mengkodekan jarak & sisi sekaligus, jadi pengguna
/// tidak perlu mengetik sisi secara terpisah.
///
/// `None` untuk `Entity::Ellipse`: parallel-curve ellips-sejati bukan
/// ellips lagi secara umum, jadi tidak direpresentasikan oleh model
/// ellips axis-aligned kita — didokumentasikan sebagai belum didukung
/// alih-alih memberi hasil yang salah.
pub fn offset_entity(entity: &Entity, reference_point: DVec2) -> Option<Entity> {
    match entity {
        Entity::Line { start, end } => {
            let dir = (*end - *start).normalize_or_zero();
            if dir == DVec2::ZERO {
                return None;
            }
            let normal = DVec2::new(-dir.y, dir.x);
            let offset_vec = normal * (reference_point - *start).dot(normal);
            Some(Entity::Line {
                start: *start + offset_vec,
                end: *end + offset_vec,
            })
        }
        Entity::Circle { center, .. } => {
            let radius = (reference_point - *center).length();
            (radius > 1e-6).then_some(Entity::Circle {
                center: *center,
                radius,
            })
        }
        Entity::Arc {
            center,
            start_angle,
            end_angle,
            ..
        } => {
            let radius = (reference_point - *center).length();
            (radius > 1e-6).then_some(Entity::Arc {
                center: *center,
                radius,
                start_angle: *start_angle,
                end_angle: *end_angle,
            })
        }
        Entity::Ellipse { .. } => None,
    }
}

/// Pantulkan titik `p` melintasi garis tak-hingga melalui `axis_a`-`axis_b`.
/// Dipakai `mirror_entity` (di bawah) dan `Constraint::Symmetric` di modul
/// `constraint`. Bila sumbu berdegenerasi (dua titik berimpit), turun
/// dengan aman jadi refleksi lewat titik `axis_a` (bukan NaN/panic) —
/// bukan hasil yang bermakna geometris, tapi deterministik & tak crash.
pub fn reflect_point(p: DVec2, axis_a: DVec2, axis_b: DVec2) -> DVec2 {
    let axis_dir = (axis_b - axis_a).normalize_or_zero();
    let rel = p - axis_a;
    let along = axis_dir * rel.dot(axis_dir);
    let perp = rel - along;
    p - perp * 2.0
}

/// Pantulkan `entity` melintasi garis tak-hingga melalui `axis_a`-`axis_b`.
/// `None` bila sumbu berdegenerasi (dua titik berimpit).
///
/// Catatan keterbatasan `Entity::Ellipse`: model kita axis-aligned tanpa
/// rotasi, padahal refleksi ellips lintas sumbu sembarang umumnya
/// menghasilkan ellips berotasi. Hasil di sini hanya presisi untuk sumbu
/// cermin horizontal/vertikal; untuk sudut lain, radius dipertahankan apa
/// adanya (orientasi tidak diikutsertakan) — didokumentasikan, bukan bug
/// tersembunyi.
pub fn mirror_entity(entity: &Entity, axis_a: DVec2, axis_b: DVec2) -> Option<Entity> {
    let axis_dir = (axis_b - axis_a).normalize_or_zero();
    if axis_dir == DVec2::ZERO {
        return None;
    }
    let reflect = |p: DVec2| reflect_point(p, axis_a, axis_b);

    Some(match entity {
        Entity::Line { start, end } => Entity::Line {
            start: reflect(*start),
            end: reflect(*end),
        },
        Entity::Circle { center, radius } => Entity::Circle {
            center: reflect(*center),
            radius: *radius,
        },
        Entity::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let axis_angle = axis_dir.y.atan2(axis_dir.x);
            let reflect_angle = |a: f64| 2.0 * axis_angle - a;
            // Refleksi membalik arah CCW; tukar start/end supaya konvensi
            // "CCW dari start ke end" tetap konsisten setelah dipantulkan.
            Entity::Arc {
                center: reflect(*center),
                radius: *radius,
                start_angle: reflect_angle(*end_angle),
                end_angle: reflect_angle(*start_angle),
            }
        }
        Entity::Ellipse {
            center,
            radius_x,
            radius_y,
        } => Entity::Ellipse {
            center: reflect(*center),
            radius_x: *radius_x,
            radius_y: *radius_y,
        },
    })
}

/// Titik potong (parameter `t`, 0..1) `line` dengan entitas Line lain di
/// `sketch` (mengecualikan `exclude`) — dipakai tool Trim. Fase ini hanya
/// menghitung potongan Line-vs-Line; Line-vs-Circle/Arc menyusul saat
/// use-case-nya muncul.
pub fn line_intersection_params_in_sketch(
    sketch: &Sketch,
    line: (DVec2, DVec2),
    exclude: EntityId,
) -> Vec<f64> {
    sketch
        .entities
        .iter()
        .filter(|(id, _)| *id != exclude)
        .filter_map(|(_, e)| match e {
            Entity::Line { start, end } => line_intersection_params(line, (*start, *end)),
            _ => None,
        })
        .map(|(t, _u)| t)
        .collect()
}

/// Parameter proyeksi (tidak diklem) titik `p` pada segmen `start..end` —
/// dipakai menentukan posisi klik Trim sepanjang garis.
pub fn project_t(start: DVec2, end: DVec2, p: DVec2) -> f64 {
    let ab = end - start;
    let len_sq = ab.length_squared();
    if len_sq < f64::EPSILON {
        return 0.0;
    }
    (p - start).dot(ab) / len_sq
}

/// Sisa segmen (pasangan titik) setelah menghapus interval `[a,b]` yang
/// memuat `click_t`, dibatasi titik potong `cut_ts` plus ujung 0/1.
/// Mengembalikan 0, 1, atau 2 segmen tersisa.
pub fn trim_segments(start: DVec2, end: DVec2, cut_ts: &[f64], click_t: f64) -> Vec<(DVec2, DVec2)> {
    let mut ts: Vec<f64> = cut_ts
        .iter()
        .copied()
        .filter(|t| *t > 1e-6 && *t < 1.0 - 1e-6)
        .collect();
    ts.push(0.0);
    ts.push(1.0);
    ts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ts.dedup_by(|a, b| (*a - *b).abs() < 1e-9);

    ts.windows(2)
        .filter(|w| !(click_t >= w[0] && click_t <= w[1]))
        .map(|w| (start + (end - start) * w[0], start + (end - start) * w[1]))
        .collect()
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

/// Hapus sekumpulan entitas dan sisipkan entitas baru sebagai satu langkah
/// undo — dipakai Trim (hapus 1 Line, sisipkan 0-2 potongan sisa). Sama
/// seperti `DeleteEntities`, `revert` menyisipkan ulang dengan `EntityId`
/// baru (bukan id lama).
pub struct ReplaceEntities {
    label: &'static str,
    remove_ids: Vec<EntityId>,
    removed: Vec<Entity>,
    insert: Vec<Entity>,
    inserted_ids: Vec<EntityId>,
}

impl ReplaceEntities {
    pub fn new(label: &'static str, remove_ids: Vec<EntityId>, insert: Vec<Entity>) -> Self {
        Self {
            label,
            remove_ids,
            removed: Vec::new(),
            insert,
            inserted_ids: Vec::new(),
        }
    }
}

impl Command<Sketch> for ReplaceEntities {
    fn name(&self) -> &str {
        self.label
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        self.removed = self
            .remove_ids
            .iter()
            .filter_map(|id| sketch.entities.remove(*id))
            .collect();
        self.inserted_ids = self
            .insert
            .iter()
            .cloned()
            .map(|e| sketch.entities.insert(e))
            .collect();
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        for id in self.inserted_ids.drain(..) {
            sketch.entities.remove(id);
        }
        for entity in self.removed.drain(..) {
            sketch.entities.insert(entity);
        }
    }
}

/// Command untuk memodifikasi satu entitas di tempat (in-place) dengan
/// mempertahankan `EntityId` yang sama — dipakai oleh panel Properti saat
/// pengguna mengubah koordinat/dimensi entitas secara langsung.
pub struct UpdateEntity {
    label: &'static str,
    id: EntityId,
    old_entity: Option<Entity>,
    new_entity: Entity,
}

impl UpdateEntity {
    pub fn new(label: &'static str, id: EntityId, new_entity: Entity) -> Self {
        Self {
            label,
            id,
            old_entity: None,
            new_entity,
        }
    }
}

impl Command<Sketch> for UpdateEntity {
    fn name(&self) -> &str {
        self.label
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        if let Some(e) = sketch.entities.get_mut(self.id) {
            self.old_entity = Some(e.clone());
            *e = self.new_entity.clone();
        }
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        if let Some(old) = &self.old_entity {
            if let Some(e) = sketch.entities.get_mut(self.id) {
                *e = old.clone();
            }
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
        assert!(hit.source.is_some(), "snap Endpoint harus bawa PointRef sumber");
    }

    #[test]
    fn snap_source_is_none_for_derived_points() {
        let mut sketch = Sketch::default();
        sketch.entities.insert(Entity::Line {
            start: DVec2::new(-5.0, 0.0),
            end: DVec2::new(15.0, 0.0),
        });
        sketch.entities.insert(Entity::Line {
            start: DVec2::new(0.0, -5.0),
            end: DVec2::new(0.0, 15.0),
        });
        // Snap ke intersection: titik turunan, bukan DOF entitas manapun.
        let hit = find_snap(&sketch, DVec2::new(0.3, 0.3), 1.0, 1000.0, None).unwrap();
        assert_eq!(hit.kind, SnapKind::Intersection);
        assert!(hit.source.is_none());

        // Snap ke grid (sketch kosong): juga bukan DOF entitas.
        let sketch = Sketch::default();
        let hit = find_snap(&sketch, DVec2::new(19.6, 0.2), 1.0, 10.0, None).unwrap();
        assert_eq!(hit.kind, SnapKind::Grid);
        assert!(hit.source.is_none());
    }

    #[test]
    fn snap_center_carries_point_ref() {
        let mut sketch = Sketch::default();
        let c = sketch.entities.insert(Entity::Circle {
            center: DVec2::new(5.0, 5.0),
            radius: 3.0,
        });
        let hit = find_snap(&sketch, DVec2::new(5.1, 5.1), 1.0, 1000.0, None).unwrap();
        assert_eq!(hit.kind, SnapKind::Center);
        assert_eq!(hit.source, Some(constraint::PointRef::Center(c)));
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

    #[test]
    fn arc_from_three_points_passes_through_all_three() {
        let (p1, p2, p3) = (
            DVec2::new(10.0, 0.0),
            DVec2::new(0.0, 10.0),
            DVec2::new(-10.0, 0.0),
        );
        let arc = arc_from_three_points(p1, p2, p3).unwrap();
        let Entity::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } = arc
        else {
            panic!("bukan Arc");
        };
        for p in [p1, p2, p3] {
            assert!(((p - center).length() - radius).abs() < 1e-9);
        }
        // p2 (atas, sudut 90°) harus berada dalam rentang CCW start..end.
        let angle_p2 = (p2 - center).y.atan2((p2 - center).x);
        assert!(angle_in_range(angle_p2, start_angle, end_angle));
    }

    #[test]
    fn arc_from_three_points_none_when_collinear() {
        assert!(arc_from_three_points(
            DVec2::new(0.0, 0.0),
            DVec2::new(5.0, 0.0),
            DVec2::new(10.0, 0.0),
        )
        .is_none());
    }

    #[test]
    fn offset_line_moves_perpendicular_toward_reference() {
        let line = Entity::Line {
            start: DVec2::new(0.0, 0.0),
            end: DVec2::new(10.0, 0.0),
        };
        let offset = offset_entity(&line, DVec2::new(5.0, 3.0)).unwrap();
        assert_eq!(
            offset,
            Entity::Line {
                start: DVec2::new(0.0, 3.0),
                end: DVec2::new(10.0, 3.0),
            }
        );
    }

    #[test]
    fn offset_circle_uses_reference_distance_as_new_radius() {
        let circle = Entity::Circle {
            center: DVec2::ZERO,
            radius: 5.0,
        };
        let offset = offset_entity(&circle, DVec2::new(8.0, 0.0)).unwrap();
        assert_eq!(
            offset,
            Entity::Circle {
                center: DVec2::ZERO,
                radius: 8.0,
            }
        );
    }

    #[test]
    fn offset_ellipse_is_unsupported() {
        let ellipse = Entity::Ellipse {
            center: DVec2::ZERO,
            radius_x: 5.0,
            radius_y: 3.0,
        };
        assert!(offset_entity(&ellipse, DVec2::new(8.0, 0.0)).is_none());
    }

    #[test]
    fn mirror_line_across_vertical_axis() {
        let line = Entity::Line {
            start: DVec2::new(1.0, 0.0),
            end: DVec2::new(3.0, 4.0),
        };
        // Sumbu cermin: garis vertikal x=0.
        let mirrored = mirror_entity(&line, DVec2::new(0.0, 0.0), DVec2::new(0.0, 1.0)).unwrap();
        assert_eq!(
            mirrored,
            Entity::Line {
                start: DVec2::new(-1.0, 0.0),
                end: DVec2::new(-3.0, 4.0),
            }
        );
    }

    #[test]
    fn mirror_none_when_axis_degenerate() {
        let line = Entity::Line {
            start: DVec2::ZERO,
            end: DVec2::new(1.0, 1.0),
        };
        assert!(mirror_entity(&line, DVec2::ZERO, DVec2::ZERO).is_none());
    }

    #[test]
    fn trim_removes_middle_segment_between_two_cuts() {
        // Garis 0..10, dipotong di t=0.3 dan t=0.7, klik di tengah (t=0.5)
        // → segmen tengah dihapus, sisa dua potongan di ujung.
        let start = DVec2::new(0.0, 0.0);
        let end = DVec2::new(10.0, 0.0);
        let segments = trim_segments(start, end, &[0.3, 0.7], 0.5);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0], (DVec2::new(0.0, 0.0), DVec2::new(3.0, 0.0)));
        assert_eq!(segments[1], (DVec2::new(7.0, 0.0), DVec2::new(10.0, 0.0)));
    }

    #[test]
    fn trim_removes_dangling_end_beyond_last_cut() {
        let start = DVec2::new(0.0, 0.0);
        let end = DVec2::new(10.0, 0.0);
        // Satu potongan di t=0.5, klik di t=0.8 (setelah potongan) → sisa
        // cuma potongan awal 0..0.5.
        let segments = trim_segments(start, end, &[0.5], 0.8);
        assert_eq!(segments, vec![(DVec2::new(0.0, 0.0), DVec2::new(5.0, 0.0))]);
    }

    #[test]
    fn trim_with_no_cuts_removes_whole_line() {
        let segments = trim_segments(DVec2::ZERO, DVec2::new(10.0, 0.0), &[], 0.5);
        assert!(segments.is_empty());
    }

    #[test]
    fn replace_entities_undo_roundtrip() {
        let mut sketch = Sketch::default();
        let mut undo = UndoStack::default();
        let id = sketch.entities.insert(Entity::Line {
            start: DVec2::ZERO,
            end: DVec2::new(10.0, 0.0),
        });

        undo.execute(
            Box::new(ReplaceEntities::new(
                "Trim",
                vec![id],
                vec![Entity::Line {
                    start: DVec2::ZERO,
                    end: DVec2::new(3.0, 0.0),
                }],
            )),
            &mut sketch,
        );
        assert_eq!(sketch.entities.len(), 1);
        assert!(!sketch.entities.contains_key(id));

        // revert menyisipkan ulang dengan EntityId BARU (didokumentasikan
        // di doc comment ReplaceEntities) — cek isinya, bukan id lama.
        undo.undo(&mut sketch);
        assert_eq!(sketch.entities.len(), 1);
        assert_eq!(
            sketch.entities.values().next().unwrap(),
            &Entity::Line {
                start: DVec2::ZERO,
                end: DVec2::new(10.0, 0.0),
            }
        );
    }

    #[test]
    fn update_entity_preserves_id_and_undo_roundtrip() {
        let mut sketch = Sketch::default();
        let mut undo = UndoStack::default();
        let id = sketch.entities.insert(Entity::Circle {
            center: DVec2::ZERO,
            radius: 10.0,
        });

        undo.execute(
            Box::new(UpdateEntity::new(
                "Ubah Radius",
                id,
                Entity::Circle {
                    center: DVec2::ZERO,
                    radius: 25.0,
                },
            )),
            &mut sketch,
        );

        assert_eq!(sketch.entities.len(), 1);
        assert!(sketch.entities.contains_key(id));
        assert_eq!(
            sketch.entities.get(id).unwrap(),
            &Entity::Circle {
                center: DVec2::ZERO,
                radius: 25.0,
            }
        );

        undo.undo(&mut sketch);
        assert_eq!(
            sketch.entities.get(id).unwrap(),
            &Entity::Circle {
                center: DVec2::ZERO,
                radius: 10.0,
            }
        );

        undo.redo(&mut sketch);
        assert_eq!(
            sketch.entities.get(id).unwrap(),
            &Entity::Circle {
                center: DVec2::ZERO,
                radius: 25.0,
            }
        );
    }
}
