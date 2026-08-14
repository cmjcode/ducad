//! Constraint solver 2D CADRAW: parametrisasi entitas, definisi constraint
//! geometris & dimensional, dan solver numerik Levenberg-Marquardt (ditulis
//! sendiri, Jacobian finite-difference — lihat catatan performa di bawah).
//!
//! ## Lingkup Fase 2 (sengaja dibatasi, bukan lupa)
//! Didukung: Coincident, Horizontal, Vertical, Parallel, Perpendicular,
//! EqualLength, EqualRadius, Fixed, Distance, Radius, Angle, Tangent,
//! Symmetric — 12 jenis, mencakup mayoritas kebutuhan sketch sehari-hari.
//! **Belum ada**: point-on-entity (coincident ke kurva, bukan cuma
//! titik-ke-titik), constraint pada titik ujung Arc (`PointRef` baru
//! mendukung Line start/end & center Circle/Arc/Ellipse), Tangent Line-Line
//! (tak masuk akal secara geometris — dua garis lurus tak "bersinggungan"),
//! tangensial internal (Tangent kita hanya menutupi kasus eksternal:
//! jarak center = jumlah radius).
//!
//! ## Catatan performa
//! Jacobian dihitung numerik (finite-difference), bukan analitik —
//! sederhana & selalu benar tanpa turunan manual per jenis constraint,
//! cukup cepat untuk skala sketch (puluhan-ratusan unknown). Jacobian
//! analitik per constraint bisa menyusul di Fase 7 kalau profiling
//! menunjukkan perlu.
//!
//! ## Catatan keamanan tipe
//! `Constraint` tidak memvalidasi bahwa entitas yang dirujuk berjenis yang
//! sesuai (mis. `Horizontal` dipasang ke `Entity::Circle`) — pemanggil
//! (lapisan UI) bertanggung jawab hanya membangun constraint yang masuk
//! akal untuk jenis entitas terpilih. Melanggar ini bisa panic (index out
//! of bounds) di `solve`, bukan silently salah.

use std::collections::HashMap;

use cadraw_core::Command;
use glam::DVec2;

use crate::{Entity, EntityId, Sketch};

// ---------------------------------------------------------------------
// Definisi constraint
// ---------------------------------------------------------------------

/// Rujukan ke satu titik pada entitas — dipakai constraint yang butuh
/// titik spesifik (Coincident, Fixed, Distance), bukan seluruh entitas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointRef {
    LineStart(EntityId),
    LineEnd(EntityId),
    /// Center Circle/Arc/Ellipse.
    Center(EntityId),
}

impl PointRef {
    pub fn entity_id(&self) -> EntityId {
        match self {
            PointRef::LineStart(id) | PointRef::LineEnd(id) | PointRef::Center(id) => *id,
        }
    }
}

/// Posisi `pr` saat ini di `sketch` (bukan dari vektor parameter solve —
/// dipakai UI, mis. merender marker titik yang sudah diklik untuk
/// Coincident/Symmetric). `None` bila entitasnya hilang atau `pr` tidak
/// cocok dengan jenis entitas saat ini (mis. `Center` merujuk ke Line).
pub fn point_ref_position(sketch: &Sketch, pr: &PointRef) -> Option<DVec2> {
    let entity = sketch.entities.get(pr.entity_id())?;
    match (entity, pr) {
        (Entity::Line { start, .. }, PointRef::LineStart(_)) => Some(*start),
        (Entity::Line { end, .. }, PointRef::LineEnd(_)) => Some(*end),
        (
            Entity::Circle { center, .. } | Entity::Arc { center, .. } | Entity::Ellipse { center, .. },
            PointRef::Center(_),
        ) => Some(*center),
        _ => None,
    }
}

/// Satu constraint geometris/dimensional. Lihat catatan keamanan tipe di
/// atas modul: pemanggil menjamin entitas yang dirujuk berjenis sesuai.
#[derive(Debug, Clone)]
pub enum Constraint {
    Coincident { a: PointRef, b: PointRef },
    Horizontal { line: EntityId },
    Vertical { line: EntityId },
    Parallel { a: EntityId, b: EntityId },
    Perpendicular { a: EntityId, b: EntityId },
    EqualLength { a: EntityId, b: EntityId },
    /// Berlaku untuk Circle/Arc (radius) atau Ellipse (radius_x sbg
    /// representatif — lihat catatan layout parameter di `read_radius_param`).
    EqualRadius { a: EntityId, b: EntityId },
    Fixed { point: PointRef, target: DVec2 },
    Distance { a: PointRef, b: PointRef, value: f64 },
    Radius { entity: EntityId, value: f64 },
    /// Sudut CCW dari arah `a` ke arah `b`, radian, kontinu di (-π, π].
    Angle { a: EntityId, b: EntityId, value: f64 },
    /// Tangensial eksternal. `a`/`b` boleh Line atau Circle/Arc (bebas
    /// urutan): Line-Radial → jarak titik-ke-garis-tak-hingga = radius;
    /// Radial-Radial → jarak antar center = jumlah radius. Line-Line tidak
    /// didukung (lihat catatan lingkup modul).
    Tangent { a: EntityId, b: EntityId },
    /// Titik `a` dan `b` saling cermin melintasi garis `axis`.
    Symmetric { a: PointRef, b: PointRef, axis: EntityId },
}

// ---------------------------------------------------------------------
// Parametrisasi entitas ↔ vektor unknown
// ---------------------------------------------------------------------

/// Derajat kebebasan (jumlah unknown skalar) tiap jenis entitas.
fn entity_dof(entity: &Entity) -> usize {
    match entity {
        Entity::Line { .. } => 4,     // start.x, start.y, end.x, end.y
        Entity::Circle { .. } => 3,   // center.x, center.y, radius
        Entity::Arc { .. } => 5,      // center.x, center.y, radius, start_angle, end_angle
        Entity::Ellipse { .. } => 4,  // center.x, center.y, radius_x, radius_y
    }
}

fn pack_entity(entity: &Entity, out: &mut Vec<f64>) {
    match entity {
        Entity::Line { start, end } => out.extend([start.x, start.y, end.x, end.y]),
        Entity::Circle { center, radius } => out.extend([center.x, center.y, *radius]),
        Entity::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => out.extend([center.x, center.y, *radius, *start_angle, *end_angle]),
        Entity::Ellipse {
            center,
            radius_x,
            radius_y,
        } => out.extend([center.x, center.y, *radius_x, *radius_y]),
    }
}

fn unpack_entity(entity: &mut Entity, params: &[f64]) {
    match entity {
        Entity::Line { start, end } => {
            *start = DVec2::new(params[0], params[1]);
            *end = DVec2::new(params[2], params[3]);
        }
        Entity::Circle { center, radius } => {
            *center = DVec2::new(params[0], params[1]);
            *radius = params[2];
        }
        Entity::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            *center = DVec2::new(params[0], params[1]);
            *radius = params[2];
            *start_angle = params[3];
            *end_angle = params[4];
        }
        Entity::Ellipse {
            center,
            radius_x,
            radius_y,
        } => {
            *center = DVec2::new(params[0], params[1]);
            *radius_x = params[2];
            *radius_y = params[3];
        }
    }
}

/// Kumpulkan entitas unik yang dirujuk sekumpulan constraint, urutan
/// deterministik (dipakai membangun layout vektor parameter).
fn involved_entities(constraints: &[Constraint]) -> Vec<EntityId> {
    let mut ids: Vec<EntityId> = Vec::new();
    let push_unique = |id: EntityId, ids: &mut Vec<EntityId>| {
        if !ids.contains(&id) {
            ids.push(id);
        }
    };
    for c in constraints {
        match c {
            Constraint::Coincident { a, b } | Constraint::Distance { a, b, .. } => {
                push_unique(a.entity_id(), &mut ids);
                push_unique(b.entity_id(), &mut ids);
            }
            Constraint::Horizontal { line } | Constraint::Vertical { line } => {
                push_unique(*line, &mut ids)
            }
            Constraint::Parallel { a, b }
            | Constraint::Perpendicular { a, b }
            | Constraint::EqualLength { a, b }
            | Constraint::EqualRadius { a, b }
            | Constraint::Angle { a, b, .. }
            | Constraint::Tangent { a, b } => {
                push_unique(*a, &mut ids);
                push_unique(*b, &mut ids);
            }
            Constraint::Fixed { point, .. } => push_unique(point.entity_id(), &mut ids),
            Constraint::Radius { entity, .. } => push_unique(*entity, &mut ids),
            Constraint::Symmetric { a, b, axis } => {
                push_unique(a.entity_id(), &mut ids);
                push_unique(b.entity_id(), &mut ids);
                push_unique(*axis, &mut ids);
            }
        }
    }
    ids
}

/// Klasifikasi kasar entitas dipakai `Constraint::Tangent` untuk memilih
/// formula residual yang benar (Line-vs-Radial berbeda dari Radial-vs-
/// Radial). Disnapshot SEKALI dari `Sketch` sebelum solve dan dipegang
/// sebagai peta biasa (bukan `&Sketch` yang di-capture closure) — supaya
/// tidak bentrok dengan `&mut Sketch` yang dipakai `write_back` di akhir
/// `solve()` (variannya tak berubah selama solve, cuma nilai field yang
/// berubah, jadi snapshot di awal aman).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntityKind {
    Line,
    /// Circle, Arc, atau Ellipse — sama-sama punya slot radius di indeks
    /// ke-2 layout parameter kita.
    Radial,
}

fn entity_kind(entity: &Entity) -> EntityKind {
    match entity {
        Entity::Line { .. } => EntityKind::Line,
        Entity::Circle { .. } | Entity::Arc { .. } | Entity::Ellipse { .. } => EntityKind::Radial,
    }
}

fn build_kinds(entity_ids: &[EntityId], sketch: &Sketch) -> HashMap<EntityId, EntityKind> {
    entity_ids
        .iter()
        .filter_map(|id| sketch.entities.get(*id).map(|e| (*id, entity_kind(e))))
        .collect()
}

fn build_offsets_and_x0(entity_ids: &[EntityId], sketch: &Sketch) -> (HashMap<EntityId, usize>, Vec<f64>) {
    let mut offsets = HashMap::new();
    let mut x = Vec::new();
    for id in entity_ids {
        offsets.insert(*id, x.len());
        pack_entity(
            sketch.entities.get(*id).expect("entitas constraint hilang dari sketch"),
            &mut x,
        );
    }
    (offsets, x)
}

fn write_back(entity_ids: &[EntityId], offsets: &HashMap<EntityId, usize>, x: &[f64], sketch: &mut Sketch) {
    for id in entity_ids {
        let off = offsets[id];
        if let Some(entity) = sketch.entities.get_mut(*id) {
            let dof = entity_dof(entity);
            unpack_entity(entity, &x[off..off + dof]);
        }
    }
}

// ---------------------------------------------------------------------
// Pembacaan nilai dari vektor parameter (dipakai residual)
// ---------------------------------------------------------------------

fn read_point_ref(pr: &PointRef, x: &[f64], offsets: &HashMap<EntityId, usize>) -> DVec2 {
    let off = offsets[&pr.entity_id()];
    match pr {
        PointRef::LineStart(_) => DVec2::new(x[off], x[off + 1]),
        PointRef::LineEnd(_) => DVec2::new(x[off + 2], x[off + 3]),
        PointRef::Center(_) => DVec2::new(x[off], x[off + 1]),
    }
}

fn read_line(id: EntityId, x: &[f64], offsets: &HashMap<EntityId, usize>) -> (DVec2, DVec2) {
    let off = offsets[&id];
    (DVec2::new(x[off], x[off + 1]), DVec2::new(x[off + 2], x[off + 3]))
}

fn line_dir(id: EntityId, x: &[f64], offsets: &HashMap<EntityId, usize>) -> DVec2 {
    let (s, e) = read_line(id, x, offsets);
    let d = e - s;
    let len = d.length();
    if len < 1e-9 {
        d
    } else {
        d / len
    }
}

/// Radius (Circle/Arc) atau radius_x (Ellipse, representatif) — selalu di
/// indeks ke-2 layout parameter kita ([cx,cy,r,...]), jadi tidak perlu
/// tahu jenis entitas persisnya untuk membacanya.
fn read_radius_param(id: EntityId, x: &[f64], offsets: &HashMap<EntityId, usize>) -> f64 {
    x[offsets[&id] + 2]
}

/// Center Circle/Arc/Ellipse — indeks 0,1, sama seperti start Line (layout
/// kita menaruh titik "utama" entitas selalu di dua slot pertama).
fn read_center(id: EntityId, x: &[f64], offsets: &HashMap<EntityId, usize>) -> DVec2 {
    let off = offsets[&id];
    DVec2::new(x[off], x[off + 1])
}

/// Jarak tak-bertanda titik `p` ke garis TAK-HINGGA melalui `a`-`b` (beda
/// dari `distance_point_segment` di lib.rs yang membatasi ke segmen) —
/// dipakai residual Tangent Line-Radial, karena singgungan berlaku di
/// sepanjang garis, bukan cuma di antara kedua endpoint-nya.
fn distance_point_to_infinite_line(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let len = ab.length();
    if len < 1e-9 {
        return (p - a).length();
    }
    (ab.x * (p.y - a.y) - ab.y * (p.x - a.x)).abs() / len
}

fn constraint_residuals(
    c: &Constraint,
    x: &[f64],
    offsets: &HashMap<EntityId, usize>,
    kinds: &HashMap<EntityId, EntityKind>,
) -> Vec<f64> {
    match c {
        Constraint::Coincident { a, b } => {
            let (pa, pb) = (read_point_ref(a, x, offsets), read_point_ref(b, x, offsets));
            vec![pa.x - pb.x, pa.y - pb.y]
        }
        Constraint::Horizontal { line } => {
            let (s, e) = read_line(*line, x, offsets);
            vec![e.y - s.y]
        }
        Constraint::Vertical { line } => {
            let (s, e) = read_line(*line, x, offsets);
            vec![e.x - s.x]
        }
        Constraint::Parallel { a, b } => {
            let (da, db) = (line_dir(*a, x, offsets), line_dir(*b, x, offsets));
            vec![da.x * db.y - da.y * db.x]
        }
        Constraint::Perpendicular { a, b } => {
            let (da, db) = (line_dir(*a, x, offsets), line_dir(*b, x, offsets));
            vec![da.dot(db)]
        }
        Constraint::EqualLength { a, b } => {
            let (sa, ea) = read_line(*a, x, offsets);
            let (sb, eb) = read_line(*b, x, offsets);
            vec![(ea - sa).length() - (eb - sb).length()]
        }
        Constraint::EqualRadius { a, b } => {
            vec![read_radius_param(*a, x, offsets) - read_radius_param(*b, x, offsets)]
        }
        Constraint::Fixed { point, target } => {
            let p = read_point_ref(point, x, offsets);
            vec![p.x - target.x, p.y - target.y]
        }
        Constraint::Distance { a, b, value } => {
            let (pa, pb) = (read_point_ref(a, x, offsets), read_point_ref(b, x, offsets));
            vec![(pb - pa).length() - value]
        }
        Constraint::Radius { entity, value } => {
            vec![read_radius_param(*entity, x, offsets) - value]
        }
        Constraint::Angle { a, b, value } => {
            let (da, db) = (line_dir(*a, x, offsets), line_dir(*b, x, offsets));
            let cross = da.x * db.y - da.y * db.x;
            let dot = da.dot(db);
            vec![cross.atan2(dot) - value]
        }
        Constraint::Tangent { a, b } => {
            match (kinds.get(a), kinds.get(b)) {
                (Some(EntityKind::Radial), Some(EntityKind::Radial)) => {
                    let (ca, ra) = (read_center(*a, x, offsets), read_radius_param(*a, x, offsets));
                    let (cb, rb) = (read_center(*b, x, offsets), read_radius_param(*b, x, offsets));
                    vec![(cb - ca).length() - (ra + rb)]
                }
                (Some(EntityKind::Line), Some(EntityKind::Radial)) => {
                    let (s, e) = read_line(*a, x, offsets);
                    let (c, r) = (read_center(*b, x, offsets), read_radius_param(*b, x, offsets));
                    vec![distance_point_to_infinite_line(c, s, e) - r]
                }
                (Some(EntityKind::Radial), Some(EntityKind::Line)) => {
                    let (s, e) = read_line(*b, x, offsets);
                    let (c, r) = (read_center(*a, x, offsets), read_radius_param(*a, x, offsets));
                    vec![distance_point_to_infinite_line(c, s, e) - r]
                }
                // Line-Line: tidak masuk akal secara geometris (lihat doc
                // modul) — pemanggil (UI) menjamin tidak membangun kombinasi
                // ini; di sini no-op (bukan panic) sebagai jaring pengaman.
                _ => vec![],
            }
        }
        Constraint::Symmetric { a, b, axis } => {
            let (axis_s, axis_e) = read_line(*axis, x, offsets);
            let pa = read_point_ref(a, x, offsets);
            let pb = read_point_ref(b, x, offsets);
            let reflected = crate::reflect_point(pa, axis_s, axis_e);
            vec![reflected.x - pb.x, reflected.y - pb.y]
        }
    }
}

// ---------------------------------------------------------------------
// Aljabar linear kecil (dipakai internal solver, tanpa dependensi linalg)
// ---------------------------------------------------------------------

/// Selesaikan `a x = b` lewat eliminasi Gauss + pivot parsial. `a` matriks
/// persegi n×n (baris demi baris). `None` bila singular (pivot ~0).
///
/// Loop berbasis indeks di sini disengaja (bukan lupa idiom iterator):
/// operasi baris Gauss (swap baris, kombinasi linear antar baris pada
/// indeks kolom yang berjalan) butuh akses acak ke banyak baris/kolom
/// sekaligus per langkah, yang justru lebih jelas ditulis dengan indeks
/// eksplisit daripada dipaksakan lewat iterator/enumerate.
#[allow(clippy::needless_range_loop)]
fn solve_linear(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for col in 0..n {
        let mut pivot_row = col;
        let mut pivot_val = a[col][col].abs();
        for row in (col + 1)..n {
            if a[row][col].abs() > pivot_val {
                pivot_val = a[row][col].abs();
                pivot_row = row;
            }
        }
        if pivot_val < 1e-12 {
            return None;
        }
        a.swap(col, pivot_row);
        b.swap(col, pivot_row);
        for row in (col + 1)..n {
            let factor = a[row][col] / a[col][col];
            for k in col..n {
                a[row][k] -= factor * a[col][k];
            }
            b[row] -= factor * b[col];
        }
    }
    let mut x = vec![0.0; n];
    for row in (0..n).rev() {
        let sum: f64 = (row + 1..n).map(|k| a[row][k] * x[k]).sum();
        x[row] = (b[row] - sum) / a[row][row];
    }
    Some(x)
}

fn numeric_jacobian(residual_fn: &dyn Fn(&[f64]) -> Vec<f64>, x: &[f64], r0: &[f64]) -> Vec<Vec<f64>> {
    const EPS: f64 = 1e-7;
    let n = x.len();
    let m = r0.len();
    let mut jac = vec![vec![0.0; n]; m];
    for j in 0..n {
        let mut xp = x.to_vec();
        let h = EPS * x[j].abs().max(1.0);
        xp[j] += h;
        let r1 = residual_fn(&xp);
        for i in 0..m {
            jac[i][j] = (r1[i] - r0[i]) / h;
        }
    }
    jac
}

// ---------------------------------------------------------------------
// Solver Levenberg-Marquardt
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct SolveResult {
    pub converged: bool,
    pub iterations: usize,
    pub final_residual_norm: f64,
}

const MAX_ITERS: usize = 50;
const COST_TOL: f64 = 1e-20;
const MAX_LAMBDA_TRIES: usize = 12;

/// Selesaikan `constraints` di atas geometri `sketch` saat ini, menulis
/// balik hasilnya ke entitas yang terlibat. Entitas yang tidak dirujuk
/// constraint mana pun tidak disentuh sama sekali (tidak ikut parametrisasi).
pub fn solve(sketch: &mut Sketch, constraints: &[Constraint]) -> SolveResult {
    let entity_ids = involved_entities(constraints);
    if entity_ids.is_empty() || constraints.is_empty() {
        return SolveResult {
            converged: true,
            iterations: 0,
            final_residual_norm: 0.0,
        };
    }
    let (offsets, mut x) = build_offsets_and_x0(&entity_ids, sketch);
    // Snapshot jenis entitas (Line vs Radial) SEKALI di sini — dipakai
    // Tangent memilih formula residual. Dibaca dari `sketch` sebelum solve
    // mulai supaya closure di bawah tidak perlu meng-capture `&Sketch`
    // (yang akan bentrok dengan `&mut Sketch` di `write_back` nanti).
    let kinds = build_kinds(&entity_ids, sketch);

    let residual_fn = |x: &[f64]| -> Vec<f64> {
        constraints
            .iter()
            .flat_map(|c| constraint_residuals(c, x, &offsets, &kinds))
            .collect()
    };

    let mut r = residual_fn(&x);
    let mut lambda = 1e-3;

    for iter in 0..MAX_ITERS {
        let cost0: f64 = r.iter().map(|v| v * v).sum();
        if cost0 < COST_TOL {
            write_back(&entity_ids, &offsets, &x, sketch);
            return SolveResult {
                converged: true,
                iterations: iter,
                final_residual_norm: cost0.sqrt(),
            };
        }

        let jac = numeric_jacobian(&residual_fn, &x, &r);
        let n = x.len();
        let mut jtj = vec![vec![0.0; n]; n];
        let mut jtr = vec![0.0; n];
        for (i, row) in jac.iter().enumerate() {
            for a in 0..n {
                jtr[a] += row[a] * r[i];
                for b in 0..n {
                    jtj[a][b] += row[a] * row[b];
                }
            }
        }

        let mut improved = false;
        for _ in 0..MAX_LAMBDA_TRIES {
            let mut a = jtj.clone();
            // Damping Levenberg klasik (lambda * I), BUKAN diskalakan
            // diagonal JtJ (varian Marquardt) — parameter yang sama sekali
            // tak muncul di residual manapun (mis. center lingkaran saat
            // hanya constraint Radius aktif) punya jtj[d][d] == 0, dan
            // damping berskala-JtJ akan ikut nol di situ → sistem singular.
            // lambda*I menjamin arah bebas tetap teregularisasi.
            for (d, row) in a.iter_mut().enumerate() {
                row[d] += lambda;
            }
            let neg_jtr: Vec<f64> = jtr.iter().map(|v| -v).collect();
            let Some(delta) = solve_linear(a, neg_jtr) else {
                lambda *= 10.0;
                continue;
            };
            let x_new: Vec<f64> = x.iter().zip(&delta).map(|(xi, di)| xi + di).collect();
            let r_new = residual_fn(&x_new);
            let cost_new: f64 = r_new.iter().map(|v| v * v).sum();
            if cost_new < cost0 {
                x = x_new;
                r = r_new;
                lambda = (lambda * 0.5).max(1e-12);
                improved = true;
                break;
            }
            lambda *= 4.0;
        }

        if !improved {
            write_back(&entity_ids, &offsets, &x, sketch);
            return SolveResult {
                converged: false,
                iterations: iter,
                final_residual_norm: cost0.sqrt(),
            };
        }
    }

    let final_cost: f64 = r.iter().map(|v| v * v).sum();
    write_back(&entity_ids, &offsets, &x, sketch);
    SolveResult {
        converged: final_cost < COST_TOL,
        iterations: MAX_ITERS,
        final_residual_norm: final_cost.sqrt(),
    }
}

// ---------------------------------------------------------------------
// Command undo/redo
// ---------------------------------------------------------------------

/// Tambah satu constraint ke sketch dan langsung solve seluruh sistem
/// (constraint lama + baru). Snapshot geometri entitas terlibat diambil
/// SEBELUM solve supaya `revert` bisa mengembalikannya persis.
///
/// Pemanggil (UI) disarankan melakukan "dry run" solve di atas clone
/// sketch dulu untuk memutuskan apakah constraint ini layak dikirim ke
/// undo stack sama sekali — lihat `cadraw-app`. `apply` di sini tidak
/// menolak constraint yang gagal konvergen; ia menyimpan hasil "terbaik
/// yang didapat" solver, konsisten dengan filosofi command lain di crate
/// ini (tidak diam-diam gagal, tapi juga tidak crash).
pub struct AddConstraint {
    constraint: Constraint,
    prior_geometry: Vec<(EntityId, Entity)>,
}

impl AddConstraint {
    pub fn new(constraint: Constraint) -> Self {
        Self {
            constraint,
            prior_geometry: Vec::new(),
        }
    }
}

impl Command<Sketch> for AddConstraint {
    fn name(&self) -> &str {
        "Constraint"
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        self.prior_geometry = involved_entities(std::slice::from_ref(&self.constraint))
            .into_iter()
            .filter_map(|id| sketch.entities.get(id).map(|e| (id, e.clone())))
            .collect();
        sketch.constraints.push(self.constraint.clone());
        let snapshot = sketch.constraints.clone();
        solve(sketch, &snapshot);
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        sketch.constraints.pop();
        for (id, entity) in &self.prior_geometry {
            if let Some(slot) = sketch.entities.get_mut(*id) {
                *slot = entity.clone();
            }
        }
    }
}

/// Hapus constraint pada indeks tertentu dan solve ulang sisanya.
pub struct RemoveConstraint {
    index: usize,
    removed: Option<Constraint>,
    prior_geometry: Vec<(EntityId, Entity)>,
}

impl RemoveConstraint {
    pub fn new(index: usize) -> Self {
        Self {
            index,
            removed: None,
            prior_geometry: Vec::new(),
        }
    }
}

impl Command<Sketch> for RemoveConstraint {
    fn name(&self) -> &str {
        "Hapus Constraint"
    }
    fn apply(&mut self, sketch: &mut Sketch) {
        if self.index >= sketch.constraints.len() {
            return;
        }
        self.prior_geometry = involved_entities(&sketch.constraints)
            .into_iter()
            .filter_map(|id| sketch.entities.get(id).map(|e| (id, e.clone())))
            .collect();
        self.removed = Some(sketch.constraints.remove(self.index));
        let snapshot = sketch.constraints.clone();
        solve(sketch, &snapshot);
    }
    fn revert(&mut self, sketch: &mut Sketch) {
        if let Some(c) = self.removed.take() {
            let at = self.index.min(sketch.constraints.len());
            sketch.constraints.insert(at, c);
        }
        for (id, entity) in &self.prior_geometry {
            if let Some(slot) = sketch.entities.get_mut(*id) {
                *slot = entity.clone();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(sketch: &mut Sketch, start: DVec2, end: DVec2) -> EntityId {
        sketch.entities.insert(Entity::Line { start, end })
    }

    fn circle(sketch: &mut Sketch, center: DVec2, radius: f64) -> EntityId {
        sketch.entities.insert(Entity::Circle { center, radius })
    }

    #[test]
    fn horizontal_levels_a_tilted_line() {
        let mut sketch = Sketch::default();
        let l = line(&mut sketch, DVec2::new(0.0, 0.0), DVec2::new(10.0, 3.0));
        let result = solve(&mut sketch, &[Constraint::Horizontal { line: l }]);
        assert!(result.converged);
        let Entity::Line { start, end } = sketch.entities[l] else { unreachable!() };
        assert!((end.y - start.y).abs() < 1e-6);
    }

    #[test]
    fn vertical_straightens_a_line() {
        let mut sketch = Sketch::default();
        let l = line(&mut sketch, DVec2::new(0.0, 0.0), DVec2::new(4.0, 10.0));
        let result = solve(&mut sketch, &[Constraint::Vertical { line: l }]);
        assert!(result.converged);
        let Entity::Line { start, end } = sketch.entities[l] else { unreachable!() };
        assert!((end.x - start.x).abs() < 1e-6);
    }

    #[test]
    fn parallel_aligns_two_line_directions() {
        let mut sketch = Sketch::default();
        let a = line(&mut sketch, DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0));
        let b = line(&mut sketch, DVec2::new(0.0, 5.0), DVec2::new(8.0, 7.0));
        let result = solve(&mut sketch, &[Constraint::Parallel { a, b }]);
        assert!(result.converged);
        let (Entity::Line { start: sa, end: ea }, Entity::Line { start: sb, end: eb }) =
            (sketch.entities[a].clone(), sketch.entities[b].clone())
        else {
            unreachable!()
        };
        let (da, db) = ((ea - sa).normalize(), (eb - sb).normalize());
        assert!((da.x * db.y - da.y * db.x).abs() < 1e-6);
    }

    #[test]
    fn perpendicular_makes_directions_orthogonal() {
        let mut sketch = Sketch::default();
        let a = line(&mut sketch, DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0));
        let b = line(&mut sketch, DVec2::new(0.0, 0.0), DVec2::new(8.0, 2.0));
        let result = solve(&mut sketch, &[Constraint::Perpendicular { a, b }]);
        assert!(result.converged);
        let (Entity::Line { start: sa, end: ea }, Entity::Line { start: sb, end: eb }) =
            (sketch.entities[a].clone(), sketch.entities[b].clone())
        else {
            unreachable!()
        };
        let (da, db) = ((ea - sa).normalize(), (eb - sb).normalize());
        assert!(da.dot(db).abs() < 1e-6);
    }

    #[test]
    fn distance_sets_exact_length_between_two_points() {
        let mut sketch = Sketch::default();
        let l = line(&mut sketch, DVec2::new(0.0, 0.0), DVec2::new(3.0, 0.0));
        let result = solve(
            &mut sketch,
            &[Constraint::Distance {
                a: PointRef::LineStart(l),
                b: PointRef::LineEnd(l),
                value: 25.0,
            }],
        );
        assert!(result.converged);
        let Entity::Line { start, end } = sketch.entities[l] else { unreachable!() };
        assert!(((end - start).length() - 25.0).abs() < 1e-5);
    }

    #[test]
    fn radius_sets_exact_circle_radius() {
        let mut sketch = Sketch::default();
        let c = circle(&mut sketch, DVec2::ZERO, 5.0);
        let result = solve(&mut sketch, &[Constraint::Radius { entity: c, value: 12.5 }]);
        assert!(result.converged);
        let Entity::Circle { radius, .. } = sketch.entities[c] else { unreachable!() };
        assert!((radius - 12.5).abs() < 1e-6);
    }

    #[test]
    fn coincident_brings_two_separate_points_together() {
        let mut sketch = Sketch::default();
        let a = line(&mut sketch, DVec2::new(0.0, 0.0), DVec2::new(5.0, 0.0));
        let b = line(&mut sketch, DVec2::new(10.0, 10.0), DVec2::new(15.0, 10.0));
        let result = solve(
            &mut sketch,
            &[Constraint::Coincident {
                a: PointRef::LineEnd(a),
                b: PointRef::LineStart(b),
            }],
        );
        assert!(result.converged);
        let (Entity::Line { end: ea, .. }, Entity::Line { start: sb, .. }) =
            (sketch.entities[a].clone(), sketch.entities[b].clone())
        else {
            unreachable!()
        };
        assert!((ea - sb).length() < 1e-5);
    }

    #[test]
    fn equal_length_matches_two_lines() {
        let mut sketch = Sketch::default();
        let a = line(&mut sketch, DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0));
        let b = line(&mut sketch, DVec2::new(0.0, 5.0), DVec2::new(3.0, 5.0));
        let result = solve(&mut sketch, &[Constraint::EqualLength { a, b }]);
        assert!(result.converged);
        let (Entity::Line { start: sa, end: ea }, Entity::Line { start: sb, end: eb }) =
            (sketch.entities[a].clone(), sketch.entities[b].clone())
        else {
            unreachable!()
        };
        assert!(((ea - sa).length() - (eb - sb).length()).abs() < 1e-5);
    }

    #[test]
    fn equal_radius_matches_two_circles() {
        let mut sketch = Sketch::default();
        let a = circle(&mut sketch, DVec2::ZERO, 4.0);
        let b = circle(&mut sketch, DVec2::new(20.0, 0.0), 9.0);
        let result = solve(&mut sketch, &[Constraint::EqualRadius { a, b }]);
        assert!(result.converged);
        let (Entity::Circle { radius: ra, .. }, Entity::Circle { radius: rb, .. }) =
            (sketch.entities[a].clone(), sketch.entities[b].clone())
        else {
            unreachable!()
        };
        assert!((ra - rb).abs() < 1e-5);
    }

    #[test]
    fn angle_sets_angle_between_two_lines() {
        let mut sketch = Sketch::default();
        let a = line(&mut sketch, DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0));
        let b = line(&mut sketch, DVec2::new(0.0, 0.0), DVec2::new(10.0, 1.0));
        let target = std::f64::consts::FRAC_PI_4; // 45°
        let result = solve(&mut sketch, &[Constraint::Angle { a, b, value: target }]);
        assert!(result.converged);
        let (Entity::Line { start: sa, end: ea }, Entity::Line { start: sb, end: eb }) =
            (sketch.entities[a].clone(), sketch.entities[b].clone())
        else {
            unreachable!()
        };
        let (da, db) = ((ea - sa).normalize(), (eb - sb).normalize());
        let angle = (da.x * db.y - da.y * db.x).atan2(da.dot(db));
        assert!((angle - target).abs() < 1e-4);
    }

    #[test]
    fn fixed_pins_a_point_while_other_constraint_is_satisfied() {
        let mut sketch = Sketch::default();
        let l = line(&mut sketch, DVec2::new(1.0, 1.0), DVec2::new(11.0, 4.0));
        let target = DVec2::new(2.0, 3.0);
        let result = solve(
            &mut sketch,
            &[
                Constraint::Fixed {
                    point: PointRef::LineStart(l),
                    target,
                },
                Constraint::Horizontal { line: l },
            ],
        );
        assert!(result.converged);
        let Entity::Line { start, end } = sketch.entities[l] else { unreachable!() };
        assert!((start - target).length() < 1e-5);
        assert!((end.y - start.y).abs() < 1e-5);
    }

    #[test]
    fn conflicting_fixed_constraints_fail_to_converge_without_panicking() {
        let mut sketch = Sketch::default();
        let l = line(&mut sketch, DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0));
        // Dua Fixed pada titik YANG SAMA (start) ke dua target berbeda —
        // tidak mungkin dipenuhi keduanya sekaligus.
        let result = solve(
            &mut sketch,
            &[
                Constraint::Fixed {
                    point: PointRef::LineStart(l),
                    target: DVec2::new(0.0, 0.0),
                },
                Constraint::Fixed {
                    point: PointRef::LineStart(l),
                    target: DVec2::new(100.0, 100.0),
                },
            ],
        );
        assert!(!result.converged);
    }

    #[test]
    fn tangent_external_sets_center_distance_to_sum_of_radii() {
        let mut sketch = Sketch::default();
        let a = circle(&mut sketch, DVec2::ZERO, 5.0);
        let b = circle(&mut sketch, DVec2::new(9.0, 0.0), 3.0);
        let result = solve(&mut sketch, &[Constraint::Tangent { a, b }]);
        assert!(result.converged);
        let (Entity::Circle { center: ca, radius: ra }, Entity::Circle { center: cb, radius: rb }) =
            (sketch.entities[a].clone(), sketch.entities[b].clone())
        else {
            unreachable!()
        };
        assert!(((cb - ca).length() - (ra + rb)).abs() < 1e-5);
    }

    #[test]
    fn tangent_line_circle_sets_distance_to_radius() {
        let mut sketch = Sketch::default();
        let l = line(&mut sketch, DVec2::new(-10.0, 0.0), DVec2::new(10.0, 0.0));
        let c = circle(&mut sketch, DVec2::new(0.0, 4.0), 2.0);
        let result = solve(&mut sketch, &[Constraint::Tangent { a: l, b: c }]);
        assert!(result.converged);
        let (Entity::Line { start, end }, Entity::Circle { center, radius }) =
            (sketch.entities[l].clone(), sketch.entities[c].clone())
        else {
            unreachable!()
        };
        assert!((distance_point_to_infinite_line(center, start, end) - radius).abs() < 1e-5);
    }

    #[test]
    fn tangent_works_with_arc_too() {
        // Memastikan layout parameter Arc (5 DOF, bukan 3 seperti Circle)
        // tidak merusak pembacaan center/radius di indeks 0,1,2.
        let mut sketch = Sketch::default();
        let arc = sketch.entities.insert(Entity::Arc {
            center: DVec2::ZERO,
            radius: 5.0,
            start_angle: 0.0,
            end_angle: std::f64::consts::PI,
        });
        let c = circle(&mut sketch, DVec2::new(9.0, 0.0), 3.0);
        let result = solve(&mut sketch, &[Constraint::Tangent { a: arc, b: c }]);
        assert!(result.converged);
        let (Entity::Arc { center: ca, radius: ra, .. }, Entity::Circle { center: cb, radius: rb }) =
            (sketch.entities[arc].clone(), sketch.entities[c].clone())
        else {
            unreachable!()
        };
        assert!(((cb - ca).length() - (ra + rb)).abs() < 1e-5);
    }

    #[test]
    fn symmetric_mirrors_point_b_to_match_reflection_of_a() {
        // Catatan: Symmetric HANYA menjamin reflect(a) == b — tidak
        // memaksa titik `a` atau sumbu tetap diam. Dengan cuma 2 residual
        // vs 12 unknown (3 entitas × 4 DOF Line), solver bebas menggeser
        // ketiganya bersamaan. Jadi assert di sini memverifikasi invarian
        // yang benar-benar dijamin constraint (reflect(a_final) == b_final
        // relatif sumbu FINAL), bukan asumsi posisi awal `a`/sumbu tak
        // berubah — pola sama seperti test Parallel/Perpendicular di atas.
        let mut sketch = Sketch::default();
        let axis = line(&mut sketch, DVec2::new(0.0, -10.0), DVec2::new(0.0, 10.0));
        let a = line(&mut sketch, DVec2::new(3.0, 2.0), DVec2::new(3.0, 2.0));
        let b = line(&mut sketch, DVec2::new(-1.0, -1.0), DVec2::new(-1.0, -1.0));
        let result = solve(
            &mut sketch,
            &[Constraint::Symmetric {
                a: PointRef::LineStart(a),
                b: PointRef::LineStart(b),
                axis,
            }],
        );
        assert!(result.converged);
        let (
            Entity::Line { start: pa, .. },
            Entity::Line { start: pb, .. },
            Entity::Line { start: axis_s, end: axis_e },
        ) = (
            sketch.entities[a].clone(),
            sketch.entities[b].clone(),
            sketch.entities[axis].clone(),
        )
        else {
            unreachable!()
        };
        let reflected = crate::reflect_point(pa, axis_s, axis_e);
        assert!((reflected - pb).length() < 1e-5);
    }

    #[test]
    fn point_ref_position_reads_current_geometry() {
        let mut sketch = Sketch::default();
        let l = line(&mut sketch, DVec2::new(1.0, 2.0), DVec2::new(3.0, 4.0));
        assert_eq!(
            point_ref_position(&sketch, &PointRef::LineStart(l)),
            Some(DVec2::new(1.0, 2.0))
        );
        assert_eq!(
            point_ref_position(&sketch, &PointRef::LineEnd(l)),
            Some(DVec2::new(3.0, 4.0))
        );
        // Center pada Line tidak cocok jenis -> None, bukan panic.
        assert_eq!(point_ref_position(&sketch, &PointRef::Center(l)), None);
    }

    #[test]
    fn add_constraint_undo_restores_geometry_and_constraint_list() {
        let mut sketch = Sketch::default();
        let mut undo = crate::UndoStack::default();
        let l = line(&mut sketch, DVec2::new(0.0, 0.0), DVec2::new(10.0, 4.0));

        undo.execute(
            Box::new(AddConstraint::new(Constraint::Horizontal { line: l })),
            &mut sketch,
        );
        assert_eq!(sketch.constraints.len(), 1);
        let Entity::Line { start, end } = sketch.entities[l] else { unreachable!() };
        assert!((end.y - start.y).abs() < 1e-6);

        undo.undo(&mut sketch);
        assert_eq!(sketch.constraints.len(), 0);
        let Entity::Line { start, end } = sketch.entities[l] else { unreachable!() };
        assert!((end.y - start.y - 4.0).abs() < 1e-9); // kembali ke geometri semula

        undo.redo(&mut sketch);
        assert_eq!(sketch.constraints.len(), 1);
        let Entity::Line { start, end } = sketch.entities[l] else { unreachable!() };
        assert!((end.y - start.y).abs() < 1e-6);
    }
}
