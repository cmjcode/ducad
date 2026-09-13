use std::collections::HashMap;

use glam::DVec2;

use crate::constraint::types::{Constraint, PointRef};
use crate::entity::{Entity, EntityId};
use crate::sketch::Sketch;

fn entity_dof(entity: &Entity) -> usize {
    match entity {
        Entity::Line { .. } => 4,
        Entity::Circle { .. } => 3,
        Entity::Arc { .. } => 5,
        Entity::Ellipse { .. } => 4,
        Entity::Spline { points, .. } => points.len() * 2,
    }
}

fn pack_entity(entity: &Entity, out: &mut Vec<f64>) {
    match entity {
        Entity::Line { start, end, .. } => out.extend([start.x, start.y, end.x, end.y]),
        Entity::Circle { center, radius, .. } => out.extend([center.x, center.y, *radius]),
        Entity::Arc {
            center,
            radius,
            start_angle,
            end_angle,
            ..
        } => out.extend([center.x, center.y, *radius, *start_angle, *end_angle]),
        Entity::Ellipse {
            center,
            radius_x,
            radius_y,
            ..
        } => out.extend([center.x, center.y, *radius_x, *radius_y]),
        Entity::Spline { points, .. } => {
            for p in points {
                out.extend([p.x, p.y]);
            }
        }
    }
}

fn unpack_entity(entity: &mut Entity, params: &[f64]) {
    match entity {
        Entity::Line { start, end, .. } => {
            *start = DVec2::new(params[0], params[1]);
            *end = DVec2::new(params[2], params[3]);
        }
        Entity::Circle { center, radius, .. } => {
            *center = DVec2::new(params[0], params[1]);
            *radius = params[2];
        }
        Entity::Arc {
            center,
            radius,
            start_angle,
            end_angle,
            ..
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
            ..
        } => {
            *center = DVec2::new(params[0], params[1]);
            *radius_x = params[2];
            *radius_y = params[3];
        }
        Entity::Spline { points, .. } => {
            for (i, p) in points.iter_mut().enumerate() {
                if 2 * i + 1 < params.len() {
                    *p = DVec2::new(params[2 * i], params[2 * i + 1]);
                }
            }
        }
    }
}

pub(crate) fn involved_entities(constraints: &[Constraint]) -> Vec<EntityId> {
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
            Constraint::PointOnCurve { point, curve } => {
                push_unique(point.entity_id(), &mut ids);
                push_unique(*curve, &mut ids);
            }
            Constraint::Midpoint { point, line } => {
                push_unique(point.entity_id(), &mut ids);
                push_unique(*line, &mut ids);
            }
            Constraint::Concentric { a, b } | Constraint::Collinear { a, b } => {
                push_unique(*a, &mut ids);
                push_unique(*b, &mut ids);
            }
        }
    }
    ids
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntityKind {
    Line,
    Radial,
}

fn entity_kind(entity: &Entity) -> EntityKind {
    match entity {
        Entity::Line { .. } => EntityKind::Line,
        Entity::Circle { .. }
        | Entity::Arc { .. }
        | Entity::Ellipse { .. }
        | Entity::Spline { .. } => EntityKind::Radial,
    }
}

fn build_kinds(entity_ids: &[EntityId], sketch: &Sketch) -> HashMap<EntityId, EntityKind> {
    entity_ids
        .iter()
        .filter_map(|id| sketch.entities.get(*id).map(|e| (*id, entity_kind(e))))
        .collect()
}

fn build_offsets_and_x0(
    entity_ids: &[EntityId],
    sketch: &Sketch,
) -> (HashMap<EntityId, usize>, Vec<f64>) {
    let mut offsets = HashMap::new();
    let mut x = Vec::new();
    for id in entity_ids {
        offsets.insert(*id, x.len());
        pack_entity(
            sketch
                .entities
                .get(*id)
                .expect("entitas constraint hilang dari sketch"),
            &mut x,
        );
    }
    (offsets, x)
}

fn write_back(
    entity_ids: &[EntityId],
    offsets: &HashMap<EntityId, usize>,
    x: &[f64],
    sketch: &mut Sketch,
) {
    for id in entity_ids {
        let off = offsets[id];
        if let Some(entity) = sketch.entities.get_mut(*id) {
            let dof = entity_dof(entity);
            unpack_entity(entity, &x[off..off + dof]);
        }
    }
}

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
    (
        DVec2::new(x[off], x[off + 1]),
        DVec2::new(x[off + 2], x[off + 3]),
    )
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

fn read_radius_param(id: EntityId, x: &[f64], offsets: &HashMap<EntityId, usize>) -> f64 {
    x[offsets[&id] + 2]
}

fn read_center(id: EntityId, x: &[f64], offsets: &HashMap<EntityId, usize>) -> DVec2 {
    let off = offsets[&id];
    DVec2::new(x[off], x[off + 1])
}

pub(crate) fn distance_point_to_infinite_line(p: DVec2, a: DVec2, b: DVec2) -> f64 {
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
                _ => vec![],
            }
        }
        Constraint::Symmetric { a, b, axis } => {
            let (axis_s, axis_e) = read_line(*axis, x, offsets);
            let pa = read_point_ref(a, x, offsets);
            let pb = read_point_ref(b, x, offsets);
            let reflected = crate::ops::reflect_point(pa, axis_s, axis_e);
            vec![reflected.x - pb.x, reflected.y - pb.y]
        }
        Constraint::PointOnCurve { point, curve } => {
            let p = read_point_ref(point, x, offsets);
            match kinds.get(curve) {
                Some(EntityKind::Line) => {
                    let (s, e) = read_line(*curve, x, offsets);
                    // Jarak BERTANDA (cross product), bukan `.abs()`: nilai
                    // mutlak punya kink di nol sehingga turunannya tidak
                    // terdefinisi persis di solusi yang dicari — solver
                    // Newton/LM akan berosilasi di sana.
                    let d = e - s;
                    let len = d.length();
                    if len < 1e-9 {
                        vec![0.0]
                    } else {
                        vec![(d.x * (p.y - s.y) - d.y * (p.x - s.x)) / len]
                    }
                }
                Some(EntityKind::Radial) => {
                    let c = read_center(*curve, x, offsets);
                    let r = read_radius_param(*curve, x, offsets);
                    vec![(p - c).length() - r]
                }
                None => vec![],
            }
        }
        Constraint::Midpoint { point, line } => {
            let p = read_point_ref(point, x, offsets);
            let (s, e) = read_line(*line, x, offsets);
            let mid = (s + e) * 0.5;
            vec![p.x - mid.x, p.y - mid.y]
        }
        Constraint::Concentric { a, b } => {
            let (ca, cb) = (read_center(*a, x, offsets), read_center(*b, x, offsets));
            vec![ca.x - cb.x, ca.y - cb.y]
        }
        Constraint::Collinear { a, b } => {
            let (sa, ea) = read_line(*a, x, offsets);
            let (sb, eb) = read_line(*b, x, offsets);
            let da = ea - sa;
            let len = da.length();
            if len < 1e-9 {
                return vec![0.0, 0.0];
            }
            // Sejajar DAN kedua ujung `b` berada di garis tak hingga `a`.
            // Dua residual jarak-bertanda sudah mencakup kesejajaran, jadi
            // tidak perlu persamaan cross terpisah yang akan redundan.
            let signed = |p: glam::DVec2| (da.x * (p.y - sa.y) - da.y * (p.x - sa.x)) / len;
            vec![signed(sb), signed(eb)]
        }
    }
}

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

fn numeric_jacobian(
    residual_fn: &dyn Fn(&[f64]) -> Vec<f64>,
    x: &[f64],
    r0: &[f64],
) -> Vec<Vec<f64>> {
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
/// balik hasilnya ke entitas yang terlibat.
/// Keadaan sebuah sketsa terhadap kendalanya.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintState {
    /// Masih ada derajat kebebasan tersisa — geometri bisa digeser.
    Under,
    /// Derajat kebebasan nol dan tidak ada kendala berlebih.
    Fully,
    /// Ada kendala yang tidak menambah informasi baru (redundan) atau
    /// saling bertentangan.
    Over,
}

/// Hasil analisis derajat kebebasan sebuah sketsa.
///
/// Inilah yang membedakan sketsa CAD dari sekadar gambar: pengguna harus
/// bisa melihat apakah geometrinya masih bisa bergeser (biru), sudah
/// terkunci penuh (hitam), atau justru dikendalai berlebihan (merah).
/// Sebelumnya DUCAD tidak punya informasi ini sama sekali.
#[derive(Debug, Clone)]
pub struct DofReport {
    /// Jumlah parameter bebas geometri yang terlibat.
    pub unknowns: usize,
    /// Jumlah persamaan yang dihasilkan seluruh kendala.
    pub equations: usize,
    /// Rank matriks Jacobian — jumlah persamaan yang benar-benar saling
    /// bebas.
    pub rank: usize,
    /// Derajat kebebasan tersisa (`unknowns - rank`).
    pub dof: usize,
    /// Indeks kendala (pada slice masukan) yang tidak menambah rank, alias
    /// tidak memberi informasi baru. Dilaporkan per kendala — bukan sekadar
    /// "sketsa over-constrained" — supaya UI bisa menunjuk kendala mana yang
    /// perlu dihapus.
    pub redundant: Vec<usize>,
    pub state: ConstraintState,
}

/// Analisis derajat kebebasan tanpa memodifikasi sketsa.
///
/// Rank dihitung lewat eliminasi Gauss dengan pivot parsial pada Jacobian
/// numerik di posisi geometri SAAT INI. Rank Jacobian bisa berbeda di
/// konfigurasi yang berbeda (mis. dua garis yang kebetulan sejajar membuat
/// sebuah kendala jadi redundan secara lokal), jadi hasilnya berlaku untuk
/// konfigurasi sekarang — sama seperti solver CAD komersial.
pub fn analyze_dof(sketch: &Sketch, constraints: &[Constraint]) -> DofReport {
    let entity_ids = involved_entities(constraints);
    if entity_ids.is_empty() {
        return DofReport {
            unknowns: 0,
            equations: 0,
            rank: 0,
            dof: 0,
            redundant: Vec::new(),
            state: ConstraintState::Fully,
        };
    }
    let (offsets, x) = build_offsets_and_x0(&entity_ids, sketch);
    let kinds = build_kinds(&entity_ids, sketch);
    let n = x.len();

    // Baris Jacobian dikelompokkan per kendala supaya redundansi bisa
    // dilaporkan per kendala, bukan per persamaan.
    let mut rows_per_constraint: Vec<Vec<Vec<f64>>> = Vec::with_capacity(constraints.len());
    for c in constraints {
        let single = std::slice::from_ref(c);
        let f = |xx: &[f64]| -> Vec<f64> {
            single
                .iter()
                .flat_map(|cc| constraint_residuals(cc, xx, &offsets, &kinds))
                .collect()
        };
        let r0 = f(&x);
        rows_per_constraint.push(numeric_jacobian(&f, &x, &r0));
    }

    // Tambahkan baris satu kendala sekaligus; kendala yang tidak menaikkan
    // rank berarti tidak membawa informasi baru.
    let mut basis: Vec<Vec<f64>> = Vec::new();
    let mut redundant = Vec::new();
    let mut equations = 0;
    for (idx, rows) in rows_per_constraint.iter().enumerate() {
        equations += rows.len();
        let before = basis.len();
        for row in rows {
            if let Some(reduced) = reduce_against(row, &basis) {
                basis.push(reduced);
            }
        }
        if basis.len() == before && !rows.is_empty() {
            redundant.push(idx);
        }
    }

    let rank = basis.len();
    let dof = n.saturating_sub(rank);
    let state = if !redundant.is_empty() {
        ConstraintState::Over
    } else if dof == 0 {
        ConstraintState::Fully
    } else {
        ConstraintState::Under
    };

    DofReport {
        unknowns: n,
        equations,
        rank,
        dof,
        redundant,
        state,
    }
}

/// Kurangi `row` terhadap basis ortogonal-baris yang sudah ada
/// (Gram-Schmidt termodifikasi). Mengembalikan `None` bila baris itu
/// kombinasi linear dari basis — alias tidak menambah rank.
fn reduce_against(row: &[f64], basis: &[Vec<f64>]) -> Option<Vec<f64>> {
    let mut v = row.to_vec();
    for b in basis {
        let dot: f64 = v.iter().zip(b).map(|(a, c)| a * c).sum();
        let bb: f64 = b.iter().map(|c| c * c).sum();
        if bb > 1e-18 {
            let k = dot / bb;
            for (vi, bi) in v.iter_mut().zip(b) {
                *vi -= k * bi;
            }
        }
    }
    let norm: f64 = v.iter().map(|c| c * c).sum::<f64>().sqrt();
    // Ambang relatif terhadap besaran baris asalnya, supaya kendala berskala
    // besar (mis. jarak ratusan mm) tidak salah dianggap redundan.
    let base: f64 = row.iter().map(|c| c * c).sum::<f64>().sqrt().max(1.0);
    if norm / base < 1e-7 {
        None
    } else {
        Some(v)
    }
}

/// Pecah kendala jadi kelompok-kelompok yang tidak berbagi entitas apa pun.
///
/// Sketsa nyata hampir selalu terdiri dari beberapa gugus terpisah (profil
/// luar, lubang, garis konstruksi). Menyelesaikannya sebagai SATU sistem
/// berarti eliminasi Gauss O(n³) atas seluruh sketsa, padahal gugus-gugus
/// itu tidak saling memengaruhi sama sekali. Memecahnya mengubah satu
/// sistem besar jadi banyak sistem kecil — penghematan yang tumbuh kubik
/// terhadap ukuran sketsa.
fn partition_into_clusters(constraints: &[Constraint]) -> Vec<Vec<usize>> {
    let mut parent: HashMap<EntityId, EntityId> = HashMap::new();
    fn find(parent: &mut HashMap<EntityId, EntityId>, x: EntityId) -> EntityId {
        let p = *parent.get(&x).unwrap_or(&x);
        if p == x {
            return x;
        }
        let root = find(parent, p);
        parent.insert(x, root);
        root
    }

    for c in constraints {
        let ids = involved_entities(std::slice::from_ref(c));
        for w in ids.windows(2) {
            let (ra, rb) = (find(&mut parent, w[0]), find(&mut parent, w[1]));
            if ra != rb {
                parent.insert(ra, rb);
            }
        }
        for id in ids {
            parent.entry(id).or_insert(id);
        }
    }

    let mut groups: HashMap<EntityId, Vec<usize>> = HashMap::new();
    for (i, c) in constraints.iter().enumerate() {
        let ids = involved_entities(std::slice::from_ref(c));
        let key = match ids.first() {
            Some(id) => find(&mut parent, *id),
            // Kendala tanpa entitas (mis. Tangent Line-Line yang no-op)
            // tidak punya gugus; dibuang di sini karena residualnya kosong.
            None => continue,
        };
        groups.entry(key).or_default().push(i);
    }
    groups.into_values().collect()
}

/// Selesaikan seluruh kendala, memecahnya jadi gugus-gugus independen lebih
/// dulu. Lihat [`partition_into_clusters`].
pub fn solve(sketch: &mut Sketch, constraints: &[Constraint]) -> SolveResult {
    if constraints.is_empty() {
        return SolveResult {
            converged: true,
            iterations: 0,
            final_residual_norm: 0.0,
        };
    }
    let clusters = partition_into_clusters(constraints);
    if clusters.len() <= 1 {
        return solve_cluster(sketch, constraints);
    }

    let mut converged = true;
    let mut iterations = 0;
    let mut sq_sum = 0.0;
    for cluster in clusters {
        let subset: Vec<Constraint> = cluster.iter().map(|&i| constraints[i].clone()).collect();
        let r = solve_cluster(sketch, &subset);
        converged &= r.converged;
        iterations = iterations.max(r.iterations);
        sq_sum += r.final_residual_norm * r.final_residual_norm;
    }
    SolveResult {
        converged,
        iterations,
        final_residual_norm: sq_sum.sqrt(),
    }
}

fn solve_cluster(sketch: &mut Sketch, constraints: &[Constraint]) -> SolveResult {
    let entity_ids = involved_entities(constraints);
    if entity_ids.is_empty() || constraints.is_empty() {
        return SolveResult {
            converged: true,
            iterations: 0,
            final_residual_norm: 0.0,
        };
    }
    let (offsets, mut x) = build_offsets_and_x0(&entity_ids, sketch);
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
