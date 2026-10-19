//! Constraint strip untuk seleksi sketsa 2D (Paritas 0.5 dan 0.14).
//!
//! Aturan pemilihan: dari entitas terpilih, tentukan constraint mana yang
//! berlaku, lalu bangun `Constraint` konkret. Untuk constraint yang butuh
//! "titik" (Coincident, Midpoint, PointOnCurve) dipilih kombinasi titik yang
//! saat ini paling dekat dengan sasarannya, karena itulah yang paling mungkin
//! dimaksud pengguna.

use ducad_sketch::constraint::{point_ref_position, Constraint, PointRef};
use ducad_sketch::infer::{infer_constraints, InferOptions};
use ducad_sketch::sketch::Sketch;
use ducad_sketch::{AddConstraints, Entity, EntityId};
use ducad_ui::ConstraintAction;
use glam::DVec2;

use crate::app::DuCADApp;
use crate::types::ToolKind;

fn is_line(sk: &Sketch, id: EntityId) -> bool {
    matches!(sk.entities.get(id), Some(Entity::Line { .. }))
}

fn is_round(sk: &Sketch, id: EntityId) -> bool {
    matches!(sk.entities.get(id), Some(Entity::Circle { .. } | Entity::Arc { .. }))
}

/// Titik yang bisa dirujuk constraint pada entitas `id`.
fn points(sk: &Sketch, id: EntityId) -> Vec<PointRef> {
    match sk.entities.get(id) {
        Some(Entity::Line { .. }) => vec![PointRef::LineStart(id), PointRef::LineEnd(id)],
        Some(Entity::Circle { .. } | Entity::Arc { .. } | Entity::Ellipse { .. }) => {
            vec![PointRef::Center(id)]
        }
        _ => Vec::new(),
    }
}

fn pos(sk: &Sketch, p: &PointRef) -> Option<DVec2> {
    point_ref_position(sk, p)
}

/// Jarak titik `p` ke kurva `curve` (garis tak hingga atau lingkaran).
fn dist_to_curve(sk: &Sketch, p: DVec2, curve: EntityId) -> Option<f64> {
    match sk.entities.get(curve)? {
        Entity::Line { start, end, .. } => {
            let d = *end - *start;
            let len = d.length();
            (len > 1e-12).then(|| ((p - *start).perp_dot(d) / len).abs())
        }
        Entity::Circle { center, radius, .. } | Entity::Arc { center, radius, .. } => {
            Some(((p - *center).length() - radius).abs())
        }
        _ => None,
    }
}

fn line_mid(sk: &Sketch, id: EntityId) -> Option<DVec2> {
    match sk.entities.get(id)? {
        Entity::Line { start, end, .. } => Some((*start + *end) * 0.5),
        _ => None,
    }
}

/// Kandidat (titik dari satu entitas, entitas sasaran) dengan residu terkecil.
fn best_point_target(
    sk: &Sketch,
    a: EntityId,
    b: EntityId,
    target_ok: impl Fn(EntityId) -> bool,
    residual: impl Fn(DVec2, EntityId) -> Option<f64>,
) -> Option<(PointRef, EntityId)> {
    let mut best: Option<(f64, PointRef, EntityId)> = None;
    for (src, dst) in [(a, b), (b, a)] {
        if !target_ok(dst) {
            continue;
        }
        for p in points(sk, src) {
            let Some(r) = pos(sk, &p).and_then(|v| residual(v, dst)) else { continue };
            if best.as_ref().is_none_or(|(br, _, _)| r < *br) {
                best = Some((r, p, dst));
            }
        }
    }
    best.map(|(_, p, d)| (p, d))
}

/// Constraint yang berlaku untuk seleksi `sel` (urutan bebas).
pub fn valid_actions(sk: &Sketch, sel: &[EntityId]) -> Vec<ConstraintAction> {
    use ConstraintAction as A;
    let mut out = vec![A::AutoConstrain];
    match sel {
        [] => {}
        [a] => {
            out.push(A::ApplyFixed);
            if is_line(sk, *a) {
                out.extend([A::ApplyHorizontal, A::ApplyVertical, A::ApplyDistance]);
            }
        }
        [a, b] => {
            let (la, lb) = (is_line(sk, *a), is_line(sk, *b));
            let (ra, rb) = (is_round(sk, *a), is_round(sk, *b));
            if !points(sk, *a).is_empty() && !points(sk, *b).is_empty() {
                out.push(A::ApplyCoincident);
            }
            if la && lb {
                out.extend([
                    A::ApplyParallel,
                    A::ApplyPerpendicular,
                    A::ApplyEqualLength,
                    A::ApplyCollinear,
                    A::ApplyMidpoint,
                    A::ApplyPointOnCurve,
                    A::ApplySymmetric,
                ]);
            }
            if ra && rb {
                out.extend([A::ApplyEqualRadius, A::ApplyConcentric, A::ApplyTangent]);
            }
            if (la && rb) || (ra && lb) {
                out.extend([A::ApplyTangent, A::ApplyMidpoint, A::ApplyPointOnCurve]);
            }
        }
        _ => {
            if sel.iter().all(|id| is_line(sk, *id)) {
                out.extend([A::ApplyHorizontal, A::ApplyVertical]);
            }
        }
    }
    out
}

/// Bangun constraint konkret untuk `action` dari seleksi `sel`. Kosong bila
/// aksi butuh mode pick (Fixed/Symmetric) atau seleksi tidak cocok.
pub fn build_constraints(sk: &Sketch, sel: &[EntityId], action: ConstraintAction) -> Vec<Constraint> {
    use ConstraintAction as A;
    let pair = match sel {
        [a, b] => Some((*a, *b)),
        _ => None,
    };
    let one = |c: Option<Constraint>| c.into_iter().collect::<Vec<_>>();
    match action {
        A::ApplyHorizontal | A::ApplyVertical => sel
            .iter()
            .filter(|id| is_line(sk, **id))
            .map(|id| {
                if action == A::ApplyHorizontal {
                    Constraint::Horizontal { line: *id }
                } else {
                    Constraint::Vertical { line: *id }
                }
            })
            .collect(),
        A::ApplyDistance => one(match sel {
            [id] => match sk.entities.get(*id) {
                Some(Entity::Line { start, end, .. }) => Some(Constraint::Distance {
                    a: PointRef::LineStart(*id),
                    b: PointRef::LineEnd(*id),
                    value: (*end - *start).length(),
                }),
                _ => None,
            },
            _ => None,
        }),
        A::ApplyParallel | A::ApplyPerpendicular | A::ApplyEqualLength | A::ApplyCollinear => {
            one(pair.filter(|(a, b)| is_line(sk, *a) && is_line(sk, *b)).map(|(a, b)| match action {
                A::ApplyParallel => Constraint::Parallel { a, b },
                A::ApplyPerpendicular => Constraint::Perpendicular { a, b },
                A::ApplyEqualLength => Constraint::EqualLength { a, b },
                _ => Constraint::Collinear { a, b },
            }))
        }
        A::ApplyEqualRadius | A::ApplyConcentric => {
            one(pair.filter(|(a, b)| is_round(sk, *a) && is_round(sk, *b)).map(|(a, b)| {
                if action == A::ApplyEqualRadius {
                    Constraint::EqualRadius { a, b }
                } else {
                    Constraint::Concentric { a, b }
                }
            }))
        }
        A::ApplyTangent => one(
            pair.filter(|(a, b)| {
                (is_round(sk, *a) || is_round(sk, *b))
                    && (is_round(sk, *a) || is_line(sk, *a))
                    && (is_round(sk, *b) || is_line(sk, *b))
            })
            .map(|(a, b)| Constraint::Tangent { a, b }),
        ),
        A::ApplyCoincident => one(pair.and_then(|(a, b)| {
            let mut best: Option<(f64, PointRef, PointRef)> = None;
            for pa in points(sk, a) {
                for pb in points(sk, b) {
                    let (Some(x), Some(y)) = (pos(sk, &pa), pos(sk, &pb)) else { continue };
                    let d = (x - y).length();
                    if best.as_ref().is_none_or(|(bd, _, _)| d < *bd) {
                        best = Some((d, pa, pb));
                    }
                }
            }
            best.map(|(_, a, b)| Constraint::Coincident { a, b })
        })),
        A::ApplyMidpoint => one(pair.and_then(|(a, b)| {
            best_point_target(sk, a, b, |d| is_line(sk, d), |p, d| line_mid(sk, d).map(|m| (p - m).length()))
                .map(|(point, line)| Constraint::Midpoint { point, line })
        })),
        A::ApplyPointOnCurve => one(pair.and_then(|(a, b)| {
            best_point_target(
                sk,
                a,
                b,
                |d| is_line(sk, d) || is_round(sk, d),
                |p, d| dist_to_curve(sk, p, d),
            )
            .map(|(point, curve)| Constraint::PointOnCurve { point, curve })
        })),
        A::ToggleSnap | A::ApplyFixed | A::ApplySymmetric | A::AutoConstrain => Vec::new(),
    }
}

impl DuCADApp {
    fn selected_vec(&self) -> Vec<EntityId> {
        let mut v: Vec<EntityId> = self.selected.iter().copied().collect();
        v.sort();
        v
    }

    /// Jalankan aksi strip constraint untuk seleksi sketsa saat ini.
    pub fn apply_sketch_strip_action(&mut self, action: ConstraintAction) {
        match action {
            ConstraintAction::ToggleSnap => {}
            ConstraintAction::ApplyFixed => self.set_tool(ToolKind::FixedPick),
            ConstraintAction::ApplySymmetric => self.set_tool(ToolKind::SymmetricPick),
            ConstraintAction::AutoConstrain => self.auto_constrain(),
            _ => {
                let sel = self.selected_vec();
                let cs = build_constraints(self.sketch(), &sel, action);
                if cs.is_empty() {
                    self.constraint_status = Some(ducad_i18n::t!("constraint-not-applicable"));
                }
                for c in cs {
                    self.apply_constraint(c);
                }
            }
        }
    }

    /// Auto-constrain: inferensi constraint pada seleksi (atau seluruh sketsa
    /// bila kosong), diterapkan sebagai satu langkah undo.
    pub fn auto_constrain(&mut self) {
        let ids: Vec<EntityId> = if self.selected.is_empty() {
            self.sketch().entities.keys().collect()
        } else {
            self.selected_vec()
        };
        if ids.is_empty() {
            self.constraint_status = Some(ducad_i18n::t!("constraint-auto-none"));
            return;
        }
        let opt = InferOptions {
            snap_dist: self.freehand_snap_dist(),
            max_constraints: 64,
            ..InferOptions::default()
        };
        let inferred = infer_constraints(self.sketch(), &ids, &opt);
        let n = inferred.accepted.len();
        if n == 0 {
            self.constraint_status = Some(ducad_i18n::t!("constraint-auto-none"));
            return;
        }
        self.execute_sketch_command(Box::new(AddConstraints::new(inferred.accepted)));
        self.constraint_status = Some(ducad_i18n::t!("constraint-auto-done", count = n));
    }

    /// Strip constraint di kanan kanvas untuk seleksi sketsa (bukan Node Edit).
    pub fn show_sketch_constraint_strip(&mut self, ctx: &egui::Context, screen_rect: egui::Rect) {
        if !self.is_sketching
            || self.tool != ToolKind::Select
            || self.app_mode == crate::mode::AppMode::Vector
            || self.drawing_sheet_state.is_open
        {
            return;
        }
        let sel = self.selected_vec();
        let valid = valid_actions(self.sketch(), &sel);
        let picked = egui::Area::new(egui::Id::new("ducad-sketch-constraint-strip"))
            .fixed_pos(egui::pos2(
                screen_rect.max.x - 40.0 - self.drawer_right_inset(),
                screen_rect.center().y,
            ))
            .pivot(egui::Align2::RIGHT_CENTER)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ducad_ui::ConstraintStrip::default().show_with_valid(ui, sel.len(), Some(&valid))
            })
            .inner;
        if let Some(action) = picked.filter(|a| *a != ConstraintAction::ToggleSnap) {
            self.apply_sketch_strip_action(action);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_sketch::constraint::solve;

    fn sketch_with(entities: Vec<Entity>) -> (Sketch, Vec<EntityId>) {
        let mut sk = Sketch::default();
        let ids = entities.into_iter().map(|e| sk.entities.insert(e)).collect();
        (sk, ids)
    }

    fn solved(mut sk: Sketch, cs: Vec<Constraint>) -> Sketch {
        sk.constraints.extend(cs);
        let snap = sk.constraints.clone();
        assert!(solve(&mut sk, &snap).converged);
        sk
    }

    #[test]
    fn two_circles_offer_concentric_and_solve() {
        let (sk, ids) = sketch_with(vec![
            Entity::circle(DVec2::ZERO, 5.0),
            Entity::circle(DVec2::new(1.0, 0.5), 8.0),
        ]);
        assert!(valid_actions(&sk, &ids).contains(&ConstraintAction::ApplyConcentric));
        let cs = build_constraints(&sk, &ids, ConstraintAction::ApplyConcentric);
        let sk = solved(sk, cs);
        let c = |id| match sk.entities.get(id) {
            Some(Entity::Circle { center, .. }) => *center,
            _ => panic!("bukan lingkaran"),
        };
        assert!((c(ids[0]) - c(ids[1])).length() < 1e-6);
    }

    #[test]
    fn midpoint_picks_nearest_endpoint_to_other_line_middle() {
        let (sk, ids) = sketch_with(vec![
            Entity::line(DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0)),
            Entity::line(DVec2::new(5.2, 0.3), DVec2::new(5.2, 8.0)),
        ]);
        let cs = build_constraints(&sk, &ids, ConstraintAction::ApplyMidpoint);
        assert!(
            matches!(cs.as_slice(), [Constraint::Midpoint { point: PointRef::LineStart(p), line }] if *p == ids[1] && *line == ids[0]),
            "{cs:?}"
        );
        let sk = solved(sk, cs);
        match sk.entities.get(ids[1]) {
            Some(Entity::Line { start, end, .. }) => {
                let (s0, e0) = match sk.entities.get(ids[0]) {
                    Some(Entity::Line { start, end, .. }) => (*start, *end),
                    _ => panic!("bukan garis"),
                };
                assert!((*start - (s0 + e0) * 0.5).length() < 1e-6, "{start:?} {end:?}");
            }
            _ => panic!("bukan garis"),
        }
    }

    #[test]
    fn collinear_and_point_on_curve_build() {
        let (sk, ids) = sketch_with(vec![
            Entity::line(DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0)),
            Entity::line(DVec2::new(12.0, 0.4), DVec2::new(20.0, 0.2)),
        ]);
        let cs = build_constraints(&sk, &ids, ConstraintAction::ApplyCollinear);
        assert!(matches!(cs.as_slice(), [Constraint::Collinear { .. }]));
        solved(sk, cs);
        let (sk2, ids2) = sketch_with(vec![
            Entity::circle(DVec2::ZERO, 5.0),
            Entity::line(DVec2::new(5.1, 0.0), DVec2::new(9.0, 3.0)),
        ]);
        let cs = build_constraints(&sk2, &ids2, ConstraintAction::ApplyPointOnCurve);
        assert!(
            matches!(cs.as_slice(), [Constraint::PointOnCurve { point: PointRef::LineStart(_), .. }]),
            "{cs:?}"
        );
        solved(sk2, cs);
    }

    #[test]
    fn mismatched_selection_builds_nothing() {
        let (sk, ids) = sketch_with(vec![
            Entity::line(DVec2::ZERO, DVec2::X),
            Entity::circle(DVec2::ZERO, 1.0),
        ]);
        assert!(build_constraints(&sk, &ids, ConstraintAction::ApplyConcentric).is_empty());
        assert!(build_constraints(&sk, &ids, ConstraintAction::ApplyCollinear).is_empty());
        assert!(build_constraints(&sk, &[], ConstraintAction::ApplyMidpoint).is_empty());
    }

    #[test]
    fn auto_constrain_is_one_undo_step() {
        let mut app = DuCADApp::new_for_test();
        for e in [
            Entity::line(DVec2::new(0.0, 0.1), DVec2::new(30.0, -0.2)),
            Entity::line(DVec2::new(30.05, -0.2), DVec2::new(30.2, 20.0)),
        ] {
            app.sketch_mut().entities.insert(e);
        }
        assert_eq!(app.sketch().constraints.len(), 0);
        app.auto_constrain();
        assert!(!app.sketch().constraints.is_empty(), "{:?}", app.constraint_status);
        app.undo();
        assert_eq!(app.sketch().constraints.len(), 0);
    }
}
