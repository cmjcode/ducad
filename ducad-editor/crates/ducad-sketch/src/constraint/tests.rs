use super::*;
use crate::constraint::solver::distance_point_to_infinite_line;
use crate::{Entity, EntityId, Sketch};
use glam::DVec2;

fn line(sketch: &mut Sketch, start: DVec2, end: DVec2) -> EntityId {
    sketch.entities.insert(Entity::line(start, end))
}

fn circle(sketch: &mut Sketch, center: DVec2, radius: f64) -> EntityId {
    sketch.entities.insert(Entity::circle(center, radius))
}

#[test]
fn horizontal_levels_a_tilted_line() {
    let mut sketch = Sketch::default();
    let l = line(&mut sketch, DVec2::new(0.0, 0.0), DVec2::new(10.0, 3.0));
    let result = solve(&mut sketch, &[Constraint::Horizontal { line: l }]);
    assert!(result.converged);
    let Entity::Line { start, end, .. } = sketch.entities[l] else { unreachable!() };
    assert!((end.y - start.y).abs() < 1e-6);
}

#[test]
fn vertical_straightens_a_line() {
    let mut sketch = Sketch::default();
    let l = line(&mut sketch, DVec2::new(0.0, 0.0), DVec2::new(4.0, 10.0));
    let result = solve(&mut sketch, &[Constraint::Vertical { line: l }]);
    assert!(result.converged);
    let Entity::Line { start, end, .. } = sketch.entities[l] else { unreachable!() };
    assert!((end.x - start.x).abs() < 1e-6);
}

#[test]
fn parallel_aligns_two_line_directions() {
    let mut sketch = Sketch::default();
    let a = line(&mut sketch, DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0));
    let b = line(&mut sketch, DVec2::new(0.0, 5.0), DVec2::new(8.0, 7.0));
    let result = solve(&mut sketch, &[Constraint::Parallel { a, b }]);
    assert!(result.converged);
    let (Entity::Line { start: sa, end: ea, .. }, Entity::Line { start: sb, end: eb, .. }) =
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
    let (Entity::Line { start: sa, end: ea, .. }, Entity::Line { start: sb, end: eb, .. }) =
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
    let Entity::Line { start, end, .. } = sketch.entities[l] else { unreachable!() };
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
    let (Entity::Line { start: sa, end: ea, .. }, Entity::Line { start: sb, end: eb, .. }) =
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
    let target = std::f64::consts::FRAC_PI_4;
    let result = solve(&mut sketch, &[Constraint::Angle { a, b, value: target }]);
    assert!(result.converged);
    let (Entity::Line { start: sa, end: ea, .. }, Entity::Line { start: sb, end: eb, .. }) =
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
    let Entity::Line { start, end, .. } = sketch.entities[l] else { unreachable!() };
    assert!((start - target).length() < 1e-5);
    assert!((end.y - start.y).abs() < 1e-5);
}

#[test]
fn conflicting_fixed_constraints_fail_to_converge_without_panicking() {
    let mut sketch = Sketch::default();
    let l = line(&mut sketch, DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0));
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
    let (Entity::Circle { center: ca, radius: ra, .. }, Entity::Circle { center: cb, radius: rb, .. }) =
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
    let (Entity::Line { start, end, .. }, Entity::Circle { center, radius, .. }) =
        (sketch.entities[l].clone(), sketch.entities[c].clone())
    else {
        unreachable!()
    };
    assert!((distance_point_to_infinite_line(center, start, end) - radius).abs() < 1e-5);
}

#[test]
fn tangent_works_with_arc_too() {
    let mut sketch = Sketch::default();
    let arc = sketch.entities.insert(Entity::arc(
        DVec2::ZERO,
        5.0,
        0.0,
        std::f64::consts::PI,
    ));
    let c = circle(&mut sketch, DVec2::new(9.0, 0.0), 3.0);
    let result = solve(&mut sketch, &[Constraint::Tangent { a: arc, b: c }]);
    assert!(result.converged);
    let (Entity::Arc { center: ca, radius: ra, .. }, Entity::Circle { center: cb, radius: rb, .. }) =
        (sketch.entities[arc].clone(), sketch.entities[c].clone())
    else {
        unreachable!()
    };
    assert!(((cb - ca).length() - (ra + rb)).abs() < 1e-5);
}

#[test]
fn symmetric_mirrors_point_b_to_match_reflection_of_a() {
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
        Entity::Line { start: axis_s, end: axis_e, .. },
    ) = (
        sketch.entities[a].clone(),
        sketch.entities[b].clone(),
        sketch.entities[axis].clone(),
    )
    else {
        unreachable!()
    };
    let reflected = crate::ops::reflect_point(pa, axis_s, axis_e);
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
    let Entity::Line { start, end, .. } = sketch.entities[l] else { unreachable!() };
    assert!((end.y - start.y).abs() < 1e-6);

    undo.undo(&mut sketch);
    assert_eq!(sketch.constraints.len(), 0);
    let Entity::Line { start, end, .. } = sketch.entities[l] else { unreachable!() };
    assert!((end.y - start.y - 4.0).abs() < 1e-9);

    undo.redo(&mut sketch);
    assert_eq!(sketch.constraints.len(), 1);
    let Entity::Line { start, end, .. } = sketch.entities[l] else { unreachable!() };
    assert!((end.y - start.y).abs() < 1e-6);
}

// ---------------------------------------------------------------------
// P1.2 — analisis DOF, dekomposisi gugus, dan constraint baru.
// ---------------------------------------------------------------------
mod p1_2 {
    use super::*;
    use crate::constraint::{analyze_dof, ConstraintState};

    #[test]
    fn free_line_reports_four_degrees_of_freedom() {
        // Garis punya 4 parameter (x,y start + x,y end). Satu kendala
        // Horizontal mengunci satu di antaranya, menyisakan 3.
        let mut sketch = Sketch::default();
        let l = sketch
            .entities
            .insert(Entity::line(DVec2::ZERO, DVec2::new(10.0, 1.0)));

        let report = analyze_dof(&sketch, &[Constraint::Horizontal { line: l }]);
        assert_eq!(report.unknowns, 4);
        assert_eq!(report.rank, 1);
        assert_eq!(report.dof, 3);
        assert_eq!(report.state, ConstraintState::Under);
        assert!(report.redundant.is_empty());
    }

    #[test]
    fn fully_constrained_line_reports_zero_dof() {
        let mut sketch = Sketch::default();
        let l = sketch
            .entities
            .insert(Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0)));
        let report = analyze_dof(
            &sketch,
            &[
                Constraint::Fixed {
                    point: PointRef::LineStart(l),
                    target: DVec2::ZERO,
                },
                Constraint::Fixed {
                    point: PointRef::LineEnd(l),
                    target: DVec2::new(10.0, 0.0),
                },
            ],
        );
        assert_eq!(report.dof, 0);
        assert_eq!(report.state, ConstraintState::Fully);
    }

    #[test]
    fn duplicate_constraint_is_reported_as_redundant_by_index() {
        // Kegunaan utamanya: UI bisa menunjuk kendala MANA yang harus
        // dihapus, bukan sekadar bilang "sketsa over-constrained".
        let mut sketch = Sketch::default();
        let l = sketch
            .entities
            .insert(Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0)));
        let report = analyze_dof(
            &sketch,
            &[
                Constraint::Horizontal { line: l },
                Constraint::Horizontal { line: l },
            ],
        );
        assert_eq!(report.state, ConstraintState::Over);
        assert_eq!(report.redundant, vec![1], "yang KEDUA yang berlebih");
        assert_eq!(report.rank, 1, "dua kendala identik tetap rank 1");
    }

    #[test]
    fn independent_groups_still_solve_correctly() {
        // Dekomposisi gugus hanya boleh mempercepat, tidak mengubah hasil.
        let mut sketch = Sketch::default();
        let a = sketch
            .entities
            .insert(Entity::line(DVec2::ZERO, DVec2::new(10.0, 3.0)));
        let b = sketch.entities.insert(Entity::line(
            DVec2::new(100.0, 0.0),
            DVec2::new(110.0, 4.0),
        ));

        let res = solve(
            &mut sketch,
            &[
                Constraint::Horizontal { line: a },
                Constraint::Horizontal { line: b },
            ],
        );
        assert!(res.converged);

        for id in [a, b] {
            match sketch.entities.get(id).unwrap() {
                Entity::Line { start, end, .. } => {
                    assert!((end.y - start.y).abs() < 1e-6, "garis harus horizontal");
                }
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn point_on_curve_slides_point_onto_circle() {
        // Beda dari Coincident: titiknya menempel pada KURVA, masih bebas
        // meluncur sepanjangnya. Sebelumnya tidak bisa dinyatakan sama sekali.
        let mut sketch = Sketch::default();
        let c = sketch.entities.insert(Entity::circle(DVec2::ZERO, 10.0));
        let l = sketch
            .entities
            .insert(Entity::line(DVec2::new(3.0, 0.0), DVec2::new(50.0, 0.0)));

        let res = solve(
            &mut sketch,
            &[
                Constraint::Fixed {
                    point: PointRef::Center(c),
                    target: DVec2::ZERO,
                },
                Constraint::Radius {
                    entity: c,
                    value: 10.0,
                },
                Constraint::PointOnCurve {
                    point: PointRef::LineStart(l),
                    curve: c,
                },
            ],
        );
        assert!(res.converged, "residual {}", res.final_residual_norm);

        let Entity::Line { start, .. } = sketch.entities.get(l).unwrap() else {
            unreachable!()
        };
        assert!(
            (start.length() - 10.0).abs() < 1e-6,
            "ujung garis harus berada di lingkaran, jaraknya {}",
            start.length()
        );
    }

    #[test]
    fn midpoint_places_point_at_line_centre() {
        let mut sketch = Sketch::default();
        let l = sketch
            .entities
            .insert(Entity::line(DVec2::ZERO, DVec2::new(20.0, 0.0)));
        let m = sketch
            .entities
            .insert(Entity::line(DVec2::new(5.0, 5.0), DVec2::new(6.0, 6.0)));

        let res = solve(
            &mut sketch,
            &[
                Constraint::Fixed {
                    point: PointRef::LineStart(l),
                    target: DVec2::ZERO,
                },
                Constraint::Fixed {
                    point: PointRef::LineEnd(l),
                    target: DVec2::new(20.0, 0.0),
                },
                Constraint::Midpoint {
                    point: PointRef::LineStart(m),
                    line: l,
                },
            ],
        );
        assert!(res.converged);
        let Entity::Line { start, .. } = sketch.entities.get(m).unwrap() else {
            unreachable!()
        };
        assert!((*start - DVec2::new(10.0, 0.0)).length() < 1e-6);
    }

    #[test]
    fn concentric_aligns_two_centres() {
        let mut sketch = Sketch::default();
        let a = sketch.entities.insert(Entity::circle(DVec2::ZERO, 10.0));
        let b = sketch
            .entities
            .insert(Entity::circle(DVec2::new(7.0, 3.0), 4.0));

        let res = solve(&mut sketch, &[Constraint::Concentric { a, b }]);
        assert!(res.converged);

        let ca = match sketch.entities.get(a).unwrap() {
            Entity::Circle { center, .. } => *center,
            _ => unreachable!(),
        };
        let cb = match sketch.entities.get(b).unwrap() {
            Entity::Circle { center, .. } => *center,
            _ => unreachable!(),
        };
        assert!((ca - cb).length() < 1e-6);
    }

    #[test]
    fn collinear_puts_both_lines_on_one_straight_line() {
        let mut sketch = Sketch::default();
        let a = sketch
            .entities
            .insert(Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0)));
        let b = sketch
            .entities
            .insert(Entity::line(DVec2::new(20.0, 5.0), DVec2::new(30.0, 8.0)));

        let res = solve(
            &mut sketch,
            &[
                Constraint::Fixed {
                    point: PointRef::LineStart(a),
                    target: DVec2::ZERO,
                },
                Constraint::Fixed {
                    point: PointRef::LineEnd(a),
                    target: DVec2::new(10.0, 0.0),
                },
                Constraint::Collinear { a, b },
            ],
        );
        assert!(res.converged, "residual {}", res.final_residual_norm);

        let Entity::Line { start, end, .. } = sketch.entities.get(b).unwrap() else {
            unreachable!()
        };
        // Garis `a` terkunci di sumbu X, jadi kedua ujung `b` harus y = 0.
        assert!(start.y.abs() < 1e-6, "start.y = {}", start.y);
        assert!(end.y.abs() < 1e-6, "end.y = {}", end.y);
    }

    #[test]
    fn coincident_path_node_to_line_end_solves() {
        use crate::entity::{PathSeg, Subpath};

        let mut sketch = Sketch::default();
        let p = sketch.entities.insert(Entity::Path {
            subpaths: vec![Subpath {
                start: DVec2::new(0.0, 0.0),
                segs: vec![PathSeg::Line {
                    end: DVec2::new(10.0, 5.0),
                }],
                closed: false,
            }],
            is_construction: false,
        });
        let l = sketch
            .entities
            .insert(Entity::line(DVec2::new(20.0, 0.0), DVec2::new(25.0, 20.0)));

        let res = solve(
            &mut sketch,
            &[
                Constraint::Fixed {
                    point: PointRef::LineEnd(l),
                    target: DVec2::new(25.0, 20.0),
                },
                Constraint::Coincident {
                    a: PointRef::PathNode {
                        id: p,
                        sub: 0,
                        node: 1,
                    },
                    b: PointRef::LineEnd(l),
                },
            ],
        );
        assert!(res.converged, "residual: {}", res.final_residual_norm);
        let node_pos = point_ref_position(
            &sketch,
            &PointRef::PathNode {
                id: p,
                sub: 0,
                node: 1,
            },
        )
        .unwrap();
        let line_end = point_ref_position(&sketch, &PointRef::LineEnd(l)).unwrap();
        assert!((node_pos - line_end).length() < 1e-6);
        assert!((node_pos - DVec2::new(25.0, 20.0)).length() < 1e-6);
    }

    #[test]
    fn fixed_path_node_keeps_position_after_solve() {
        use crate::entity::{PathSeg, Subpath};

        let mut sketch = Sketch::default();
        let target = DVec2::new(15.0, 30.0);
        let p = sketch.entities.insert(Entity::Path {
            subpaths: vec![Subpath {
                start: DVec2::new(0.0, 0.0),
                segs: vec![PathSeg::Line {
                    end: DVec2::new(10.0, 0.0),
                }],
                closed: false,
            }],
            is_construction: false,
        });
        let res = solve(
            &mut sketch,
            &[
                Constraint::Fixed {
                    point: PointRef::PathNode {
                        id: p,
                        sub: 0,
                        node: 0,
                    },
                    target,
                },
                Constraint::Distance {
                    a: PointRef::PathNode {
                        id: p,
                        sub: 0,
                        node: 0,
                    },
                    b: PointRef::PathNode {
                        id: p,
                        sub: 0,
                        node: 1,
                    },
                    value: 20.0,
                },
            ],
        );
        assert!(res.converged, "residual: {}", res.final_residual_norm);
        let node0 = point_ref_position(
            &sketch,
            &PointRef::PathNode {
                id: p,
                sub: 0,
                node: 0,
            },
        )
        .unwrap();
        let node1 = point_ref_position(
            &sketch,
            &PointRef::PathNode {
                id: p,
                sub: 0,
                node: 1,
            },
        )
        .unwrap();
        assert!((node0 - target).length() < 1e-6);
        assert!(((node1 - node0).length() - 20.0).abs() < 1e-5);
    }

    #[test]
    fn distance_between_two_path_nodes_converges() {
        use crate::entity::{PathSeg, Subpath};

        let mut sketch = Sketch::default();
        let p = sketch.entities.insert(Entity::Path {
            subpaths: vec![Subpath {
                start: DVec2::new(0.0, 0.0),
                segs: vec![
                    PathSeg::Line {
                        end: DVec2::new(10.0, 0.0),
                    },
                    PathSeg::Line {
                        end: DVec2::new(20.0, 5.0),
                    },
                ],
                closed: false,
            }],
            is_construction: false,
        });
        let res = solve(
            &mut sketch,
            &[Constraint::Distance {
                a: PointRef::PathNode {
                    id: p,
                    sub: 0,
                    node: 0,
                },
                b: PointRef::PathNode {
                    id: p,
                    sub: 0,
                    node: 2,
                },
                value: 35.0,
            }],
        );
        assert!(res.converged, "residual: {}", res.final_residual_norm);
        let n0 = point_ref_position(
            &sketch,
            &PointRef::PathNode {
                id: p,
                sub: 0,
                node: 0,
            },
        )
        .unwrap();
        let n2 = point_ref_position(
            &sketch,
            &PointRef::PathNode {
                id: p,
                sub: 0,
                node: 2,
            },
        )
        .unwrap();
        assert!(((n2 - n0).length() - 35.0).abs() < 1e-5);
    }

    #[test]
    fn solve_moves_handles_rigidly_with_node() {
        use crate::entity::{PathSeg, Subpath};

        let mut sketch = Sketch::default();
        let start = DVec2::new(0.0, 0.0);
        let c1 = DVec2::new(2.0, 4.0);
        let c2 = DVec2::new(8.0, 6.0);
        let end = DVec2::new(10.0, 0.0);

        let p = sketch.entities.insert(Entity::Path {
            subpaths: vec![Subpath {
                start,
                segs: vec![PathSeg::Cubic { c1, c2, end }],
                closed: false,
            }],
            is_construction: false,
        });

        let old_c1_minus_node = c1 - start;
        let old_c2_minus_node = c2 - end;

        let target_node0 = DVec2::new(5.0, 10.0);
        let res = solve(
            &mut sketch,
            &[Constraint::Fixed {
                point: PointRef::PathNode {
                    id: p,
                    sub: 0,
                    node: 0,
                },
                target: target_node0,
            }],
        );
        assert!(res.converged);

        let Entity::Path { subpaths, .. } = &sketch.entities[p] else {
            unreachable!()
        };
        let sp = &subpaths[0];
        let new_node0 = sp.node(0);
        let new_node1 = sp.node(1);
        let PathSeg::Cubic {
            c1: new_c1,
            c2: new_c2,
            ..
        } = sp.segs[0]
        else {
            unreachable!()
        };

        let new_c1_minus_node = new_c1 - new_node0;
        let new_c2_minus_node = new_c2 - new_node1;

        assert!((new_c1_minus_node - old_c1_minus_node).length() < 1e-6);
        assert!((new_c2_minus_node - old_c2_minus_node).length() < 1e-6);
        assert!((new_node0 - target_node0).length() < 1e-6);
    }

    #[test]
    fn dof_counts_two_per_path_node() {
        use crate::constraint::{analyze_dof, ConstraintState};
        use crate::entity::{PathSeg, Subpath};

        let mut sketch = Sketch::default();
        // Path dengan 1 subpath dan 2 segmen -> node_count = 3 -> 6 unknowns.
        let p = sketch.entities.insert(Entity::Path {
            subpaths: vec![Subpath {
                start: DVec2::new(0.0, 0.0),
                segs: vec![
                    PathSeg::Line {
                        end: DVec2::new(10.0, 0.0),
                    },
                    PathSeg::Line {
                        end: DVec2::new(10.0, 10.0),
                    },
                ],
                closed: false,
            }],
            is_construction: false,
        });

        // Kunci 1 node (2 persamaan: x dan y).
        let report = analyze_dof(
            &sketch,
            &[Constraint::Fixed {
                point: PointRef::PathNode {
                    id: p,
                    sub: 0,
                    node: 0,
                },
                target: DVec2::ZERO,
            }],
        );
        assert_eq!(report.unknowns, 6);
        assert_eq!(report.rank, 2);
        assert_eq!(report.dof, 4);
        assert_eq!(report.state, ConstraintState::Under);

        // Ubah is_construction = true, DoF harus tetap sama (2 DoF per node).
        let Entity::Path {
            ref mut is_construction,
            ..
        } = sketch.entities[p]
        else {
            unreachable!()
        };
        *is_construction = true;
        let report_constr = analyze_dof(
            &sketch,
            &[Constraint::Fixed {
                point: PointRef::PathNode {
                    id: p,
                    sub: 0,
                    node: 0,
                },
                target: DVec2::ZERO,
            }],
        );
        assert_eq!(report_constr.unknowns, 6);
        assert_eq!(report_constr.rank, 2);
        assert_eq!(report_constr.dof, 4);
    }
}
