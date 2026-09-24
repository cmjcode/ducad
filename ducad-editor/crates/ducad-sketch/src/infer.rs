//! Inferensi constraint untuk entitas hasil coretan (P12.2).
//!
//! Kandidat diurutkan dari yang paling "pasti" (Coincident) ke yang paling
//! spekulatif (PointOnCurve), lalu diterima satu per satu secara serakah:
//! setiap kandidat harus lolos `analyze_dof` (tidak redundan), `solve`
//! (konvergen), dan tidak menggeser geometri yang digambar pengguna lebih
//! dari `2·snap_dist` (distorsi).

use glam::DVec2;

use crate::constraint::solver::{analyze_dof, solve};
use crate::constraint::types::{point_ref_position, Constraint, PointRef};
use crate::entity::{Entity, EntityId};
use crate::sketch::Sketch;

#[derive(Debug, Clone, Copy)]
pub struct InferOptions {
    pub ang_tol_deg: f64,
    /// mm; pemanggil mengisi 8 px × mm/px.
    pub snap_dist: f64,
    pub len_tol_rel: f64,
    pub max_constraints: usize,
}

impl Default for InferOptions {
    fn default() -> Self {
        Self {
            ang_tol_deg: 5.0,
            snap_dist: 2.0,
            len_tol_rel: 0.08,
            max_constraints: 24,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    Redundant,
    Conflict,
    Distortion,
}

#[derive(Debug, Clone)]
pub struct Inferred {
    /// Sketch hasil solve.
    pub sketch: Sketch,
    pub accepted: Vec<Constraint>,
    pub rejected: Vec<(Constraint, RejectReason)>,
}

fn line_dir(sketch: &Sketch, id: EntityId) -> Option<DVec2> {
    match sketch.entities.get(id)? {
        Entity::Line { start, end, .. } => {
            let d = *end - *start;
            (d.length() > 1e-9).then(|| d.normalize())
        }
        _ => None,
    }
}

fn line_len(sketch: &Sketch, id: EntityId) -> Option<f64> {
    match sketch.entities.get(id)? {
        Entity::Line { start, end, .. } => Some((*end - *start).length()),
        _ => None,
    }
}

fn radius_of(sketch: &Sketch, id: EntityId) -> Option<f64> {
    match sketch.entities.get(id)? {
        Entity::Circle { radius, .. } | Entity::Arc { radius, .. } => Some(*radius),
        _ => None,
    }
}

fn center_of(sketch: &Sketch, id: EntityId) -> Option<DVec2> {
    match sketch.entities.get(id)? {
        Entity::Circle { center, .. }
        | Entity::Arc { center, .. }
        | Entity::Ellipse { center, .. } => Some(*center),
        _ => None,
    }
}

/// Ujung entitas yang bisa dirujuk constraint titik.
fn endpoints(sketch: &Sketch, id: EntityId) -> Vec<PointRef> {
    match sketch.entities.get(id) {
        Some(Entity::Line { .. }) | Some(Entity::Spline { .. }) => {
            vec![PointRef::LineStart(id), PointRef::LineEnd(id)]
        }
        Some(Entity::Circle { .. }) | Some(Entity::Arc { .. }) | Some(Entity::Ellipse { .. }) => {
            vec![PointRef::Center(id)]
        }
        Some(Entity::Path { subpaths, .. }) => {
            let mut pts = Vec::new();
            for (sub_idx, sub) in subpaths.iter().enumerate() {
                for node_idx in 0..sub.node_count() {
                    pts.push(PointRef::PathNode {
                        id,
                        sub: sub_idx as u16,
                        node: node_idx as u32,
                    });
                }
            }
            pts
        }
        _ => Vec::new(),
    }
}

/// Sudut (derajat) antara dua arah, dinormalkan ke [0, 90].
fn angle_between(a: DVec2, b: DVec2) -> f64 {
    a.dot(b).abs().clamp(0.0, 1.0).acos().to_degrees()
}

/// Daftar kandidat, prioritas menurun.
fn candidates(sketch: &Sketch, new_ids: &[EntityId], opt: &InferOptions) -> Vec<Constraint> {
    let mut out: Vec<Constraint> = Vec::new();
    let old_ids: Vec<EntityId> = sketch
        .entities
        .keys()
        .filter(|id| !new_ids.contains(id))
        .collect();

    // 1. Coincident antar ujung entitas baru.
    for (i, a) in new_ids.iter().enumerate() {
        for b in new_ids.iter().skip(i + 1) {
            for pa in endpoints(sketch, *a) {
                for pb in endpoints(sketch, *b) {
                    if matches!(pa, PointRef::Center(_)) || matches!(pb, PointRef::Center(_)) {
                        continue;
                    }
                    let (Some(qa), Some(qb)) = (
                        point_ref_position(sketch, &pa),
                        point_ref_position(sketch, &pb),
                    ) else {
                        continue;
                    };
                    if (qa - qb).length() <= opt.snap_dist {
                        out.push(Constraint::Coincident { a: pa, b: pb });
                    }
                }
            }
        }
    }
    // 2. Coincident ke ujung entitas lama.
    for a in new_ids {
        for b in &old_ids {
            for pa in endpoints(sketch, *a) {
                for pb in endpoints(sketch, *b) {
                    // Pusat lingkaran ditangani `Concentric`, bukan Coincident.
                    if matches!(pa, PointRef::Center(_)) || matches!(pb, PointRef::Center(_)) {
                        continue;
                    }
                    let (Some(qa), Some(qb)) = (
                        point_ref_position(sketch, &pa),
                        point_ref_position(sketch, &pb),
                    ) else {
                        continue;
                    };
                    if (qa - qb).length() <= opt.snap_dist {
                        out.push(Constraint::Coincident { a: pa, b: pb });
                    }
                }
            }
        }
    }
    // 3. Horizontal / Vertical.
    for id in new_ids {
        let Some(d) = line_dir(sketch, *id) else {
            continue;
        };
        if angle_between(d, DVec2::X) <= opt.ang_tol_deg {
            out.push(Constraint::Horizontal { line: *id });
        } else if angle_between(d, DVec2::Y) <= opt.ang_tol_deg {
            out.push(Constraint::Vertical { line: *id });
        }
    }
    // 4. Perpendicular / Parallel — garis bertetangga (berbagi ujung) dulu.
    let mut pairs: Vec<(EntityId, EntityId, bool)> = Vec::new();
    for (i, a) in new_ids.iter().enumerate() {
        for b in new_ids.iter().skip(i + 1) {
            let (Some(da), Some(db)) = (line_dir(sketch, *a), line_dir(sketch, *b)) else {
                continue;
            };
            let adjacent = endpoints(sketch, *a).iter().any(|pa| {
                endpoints(sketch, *b).iter().any(|pb| {
                    match (
                        point_ref_position(sketch, pa),
                        point_ref_position(sketch, pb),
                    ) {
                        (Some(qa), Some(qb)) => (qa - qb).length() <= opt.snap_dist,
                        _ => false,
                    }
                })
            });
            let ang = angle_between(da, db);
            if (ang - 90.0).abs() <= opt.ang_tol_deg || ang <= opt.ang_tol_deg {
                pairs.push((*a, *b, adjacent));
            }
        }
    }
    pairs.sort_by_key(|(_, _, adjacent)| !*adjacent);
    for (a, b, _) in pairs {
        let (Some(da), Some(db)) = (line_dir(sketch, a), line_dir(sketch, b)) else {
            continue;
        };
        if (angle_between(da, db) - 90.0).abs() <= opt.ang_tol_deg {
            out.push(Constraint::Perpendicular { a, b });
        } else {
            out.push(Constraint::Parallel { a, b });
        }
    }
    // 5. Tangent: garis–busur berbagi ujung dengan arah sejajar tangen.
    for a in new_ids {
        let Some(da) = line_dir(sketch, *a) else {
            continue;
        };
        for b in new_ids.iter().chain(old_ids.iter()) {
            if b == a {
                continue;
            }
            let (Some(center), Some(r)) = (center_of(sketch, *b), radius_of(sketch, *b)) else {
                continue;
            };
            for pa in endpoints(sketch, *a) {
                let Some(q) = point_ref_position(sketch, &pa) else {
                    continue;
                };
                if ((q - center).length() - r).abs() > opt.snap_dist {
                    continue;
                }
                let tangent = (q - center).perp().normalize_or_zero();
                if angle_between(da, tangent) <= opt.ang_tol_deg {
                    out.push(Constraint::Tangent { a: *a, b: *b });
                }
            }
        }
    }
    // 6. Concentric.
    for a in new_ids {
        let Some(ca) = center_of(sketch, *a) else {
            continue;
        };
        for b in new_ids.iter().chain(old_ids.iter()) {
            if b == a || (new_ids.contains(b) && b < a) {
                continue;
            }
            let Some(cb) = center_of(sketch, *b) else {
                continue;
            };
            if (ca - cb).length() <= opt.snap_dist {
                out.push(Constraint::Concentric { a: *a, b: *b });
            }
        }
    }
    // 7. EqualRadius.
    for a in new_ids {
        let Some(ra) = radius_of(sketch, *a) else {
            continue;
        };
        for b in new_ids.iter().chain(old_ids.iter()) {
            if b == a || (new_ids.contains(b) && b < a) {
                continue;
            }
            let Some(rb) = radius_of(sketch, *b) else {
                continue;
            };
            if (ra - rb).abs() <= opt.len_tol_rel * ra.max(rb) {
                out.push(Constraint::EqualRadius { a: *a, b: *b });
            }
        }
    }
    // 8. EqualLength.
    for (i, a) in new_ids.iter().enumerate() {
        let Some(la) = line_len(sketch, *a) else {
            continue;
        };
        for b in new_ids.iter().skip(i + 1) {
            let Some(lb) = line_len(sketch, *b) else {
                continue;
            };
            if (la - lb).abs() <= opt.len_tol_rel * la.max(lb) {
                out.push(Constraint::EqualLength { a: *a, b: *b });
            }
        }
    }
    // 9. PointOnCurve: ujung entitas baru menempel kurva lama.
    for a in new_ids {
        for pa in endpoints(sketch, *a) {
            if matches!(pa, PointRef::Center(_)) {
                continue;
            }
            let Some(q) = point_ref_position(sketch, &pa) else {
                continue;
            };
            for b in &old_ids {
                let on_curve = match sketch.entities.get(*b) {
                    Some(Entity::Line { start, end, .. }) => {
                        let d = *end - *start;
                        d.length() > 1e-9
                            && (q - *start).perp_dot(d.normalize()).abs() <= opt.snap_dist
                    }
                    Some(Entity::Circle { center, radius, .. })
                    | Some(Entity::Arc { center, radius, .. }) => {
                        ((q - *center).length() - *radius).abs() <= opt.snap_dist
                    }
                    _ => false,
                };
                if on_curve {
                    out.push(Constraint::PointOnCurve {
                        point: pa,
                        curve: *b,
                    });
                }
            }
        }
    }
    out
}

/// Posisi seluruh titik entitas baru (untuk uji distorsi).
fn new_positions(sketch: &Sketch, new_ids: &[EntityId]) -> Vec<DVec2> {
    let mut out = Vec::new();
    for id in new_ids {
        match sketch.entities.get(*id) {
            Some(Entity::Line { start, end, .. }) => out.extend([*start, *end]),
            Some(Entity::Circle { center, .. })
            | Some(Entity::Arc { center, .. })
            | Some(Entity::Ellipse { center, .. }) => out.push(*center),
            Some(Entity::Spline { points, .. }) => out.extend(points.iter().copied()),
            Some(Entity::Path { subpaths, .. }) => {
                for sub in subpaths {
                    out.extend(sub.nodes());
                }
            }
            None => {}
        }
    }
    out
}

/// Infer constraint untuk `new_ids` pada salinan `sketch`.
pub fn infer_constraints(sketch: &Sketch, new_ids: &[EntityId], opt: &InferOptions) -> Inferred {
    let drawn = new_positions(sketch, new_ids);
    let mut working = sketch.clone();
    let mut accepted: Vec<Constraint> = working.constraints.clone();
    let base = accepted.len();
    let mut rejected = Vec::new();

    for cand in candidates(sketch, new_ids, opt) {
        if accepted.len() - base >= opt.max_constraints {
            break;
        }
        let mut trial = accepted.clone();
        trial.push(cand.clone());
        let report = analyze_dof(&working, &trial);
        if report.redundant.contains(&(trial.len() - 1)) {
            rejected.push((cand, RejectReason::Redundant));
            continue;
        }
        let mut candidate_sketch = working.clone();
        if !solve(&mut candidate_sketch, &trial).converged {
            rejected.push((cand, RejectReason::Conflict));
            continue;
        }
        let moved = new_positions(&candidate_sketch, new_ids);
        let distorted = moved.len() != drawn.len()
            || moved
                .iter()
                .zip(&drawn)
                .any(|(a, b)| (*a - *b).length() > 2.0 * opt.snap_dist);
        if distorted {
            rejected.push((cand, RejectReason::Distortion));
            continue;
        }
        working = candidate_sketch;
        accepted = trial;
    }

    working.constraints = accepted.clone();
    Inferred {
        sketch: working,
        accepted: accepted.split_off(base),
        rejected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recognize::{recognize, to_entities, Stroke};

    fn add(sketch: &mut Sketch, entities: Vec<Entity>) -> Vec<EntityId> {
        entities
            .into_iter()
            .map(|e| sketch.entities.insert(e))
            .collect()
    }

    /// Persegi panjang kasar (30 × 20): sudut tidak rapat, sisi miring
    /// beberapa derajat. Sengaja BUKAN bujur sangkar — pada bujur sangkar
    /// `EqualLength` sisi bertetangga adalah constraint yang sah, sehingga
    /// DOF sisanya 3, bukan 4.
    fn rough_square(sketch: &mut Sketch) -> Vec<EntityId> {
        let p = [
            DVec2::new(0.0, 0.2),
            DVec2::new(30.1, -0.3),
            DVec2::new(29.8, 19.7),
            DVec2::new(-0.2, 20.2),
        ];
        let lines = (0..4)
            .map(|i| {
                // Ujung sengaja tidak tepat bertemu (celah < 1 mm).
                let a = p[i] + DVec2::new(0.1, -0.1);
                let b = p[(i + 1) % 4] + DVec2::new(-0.1, 0.1);
                Entity::line(a, b)
            })
            .collect();
        add(sketch, lines)
    }

    #[test]
    fn rough_square_gets_coincident_and_axis_constraints() {
        let mut sketch = Sketch::default();
        let ids = rough_square(&mut sketch);
        let out = infer_constraints(&sketch, &ids, &InferOptions::default());
        let count = |f: fn(&Constraint) -> bool| out.accepted.iter().filter(|c| f(c)).count();
        assert_eq!(
            count(|c| matches!(c, Constraint::Coincident { .. })),
            4,
            "{:?}",
            out.accepted
        );
        assert_eq!(count(|c| matches!(c, Constraint::Horizontal { .. })), 2);
        assert_eq!(count(|c| matches!(c, Constraint::Vertical { .. })), 2);
        assert!(
            out.rejected
                .iter()
                .any(|(_, r)| *r == RejectReason::Redundant),
            "kandidat sisa harus ditolak redundan"
        );
        let report = analyze_dof(&out.sketch, &out.sketch.constraints);
        assert_eq!(report.dof, 4, "x, y, lebar, tinggi: {report:?}");
    }

    #[test]
    fn circle_near_another_center_gets_concentric() {
        let mut sketch = Sketch::default();
        let old = add(
            &mut sketch,
            vec![Entity::Circle {
                center: DVec2::new(10.0, 10.0),
                radius: 8.0,
                is_construction: false,
            }],
        );
        let new = add(
            &mut sketch,
            vec![Entity::Circle {
                center: DVec2::new(10.6, 9.5),
                radius: 4.0,
                is_construction: false,
            }],
        );
        let out = infer_constraints(&sketch, &new, &InferOptions::default());
        assert!(
            out.accepted
                .iter()
                .any(|c| matches!(c, Constraint::Concentric { .. })),
            "{:?}",
            out.accepted
        );
        assert_eq!(old.len(), 1);
    }

    #[test]
    fn line_12_degrees_is_not_straightened() {
        let mut sketch = Sketch::default();
        let a: f64 = 12f64.to_radians();
        let ids = add(
            &mut sketch,
            vec![Entity::line(
                DVec2::ZERO,
                DVec2::new(30.0 * a.cos(), 30.0 * a.sin()),
            )],
        );
        let out = infer_constraints(&sketch, &ids, &InferOptions::default());
        assert!(out.accepted.is_empty(), "{:?}", out.accepted);
        let Some(Entity::Line { start, end, .. }) = out.sketch.entities.get(ids[0]) else {
            panic!("harus Line");
        };
        let deg = (*end - *start).y.atan2((*end - *start).x).to_degrees();
        assert!((deg - 12.0).abs() < 0.5, "{deg}");
    }

    #[test]
    fn new_line_snaps_to_old_endpoint() {
        let mut sketch = Sketch::default();
        add(
            &mut sketch,
            vec![Entity::line(DVec2::ZERO, DVec2::new(20.0, 0.0))],
        );
        let new = add(
            &mut sketch,
            vec![Entity::line(DVec2::new(20.9, 0.4), DVec2::new(21.0, 25.0))],
        );
        let out = infer_constraints(&sketch, &new, &InferOptions::default());
        assert!(
            out.accepted
                .iter()
                .any(|c| matches!(c, Constraint::Coincident { .. })),
            "{:?}",
            out.accepted
        );
        // Solver menggeser KEDUA titik; yang penting keduanya berimpit.
        let old_end = point_ref_position(
            &out.sketch,
            &PointRef::LineEnd(out.sketch.entities.keys().next().unwrap()),
        )
        .unwrap();
        let new_start = point_ref_position(&out.sketch, &PointRef::LineStart(new[0])).unwrap();
        assert!(
            (old_end - new_start).length() < 1e-6,
            "{old_end} vs {new_start}"
        );
    }

    #[test]
    fn recognized_stroke_pipeline_converges() {
        // Coretan persegi → recognize → infer: sketch harus solvable.
        let n = 160;
        let corners = [
            DVec2::new(-15.0, -10.0),
            DVec2::new(15.0, -10.0),
            DVec2::new(15.0, 10.0),
            DVec2::new(-15.0, 10.0),
        ];
        let mut pts = Vec::new();
        for e in 0..4 {
            let (a, b) = (corners[e], corners[(e + 1) % 4]);
            for i in 0..n / 4 {
                pts.push(a + (b - a) * (i as f64 / (n / 4) as f64));
            }
        }
        pts.push(corners[0]);
        let stroke = Stroke {
            points: pts,
            pressure: Vec::new(),
        };
        let r = recognize(&stroke).expect("dikenali");
        let mut sketch = Sketch::default();
        let ids = add(&mut sketch, to_entities(&r));
        let out = infer_constraints(&sketch, &ids, &InferOptions::default());
        assert!(!out.accepted.is_empty());
        let mut solved = out.sketch.clone();
        assert!(solve(&mut solved, &out.sketch.constraints).converged);
        assert!(out
            .rejected
            .iter()
            .all(|(_, r)| *r != RejectReason::Conflict));
    }

    #[test]
    fn infer_accepts_coincident_between_path_and_line() {
        use crate::entity::{PathSeg, Subpath};

        let mut sketch = Sketch::default();
        let path_id = sketch.entities.insert(Entity::Path {
            subpaths: vec![Subpath {
                start: DVec2::new(0.0, 0.0),
                segs: vec![PathSeg::Line {
                    end: DVec2::new(10.0, 0.0),
                }],
                closed: false,
            }],
            is_construction: false,
        });

        let line_id = sketch
            .entities
            .insert(Entity::line(DVec2::new(10.05, 0.05), DVec2::new(20.0, 0.0)));

        let opt = InferOptions {
            snap_dist: 0.5,
            ..InferOptions::default()
        };
        let out = infer_constraints(&sketch, &[line_id], &opt);

        let has_coincident = out.accepted.iter().any(|c| match c {
            Constraint::Coincident { a, b } => {
                (matches!(a, PointRef::PathNode { id, node: 1, .. } if *id == path_id)
                    && matches!(b, PointRef::LineStart(l) if *l == line_id))
                    || (matches!(b, PointRef::PathNode { id, node: 1, .. } if *id == path_id)
                        && matches!(a, PointRef::LineStart(l) if *l == line_id))
            }
            _ => false,
        });
        assert!(has_coincident, "accepted: {:?}", out.accepted);
    }
}
