use super::*;
use crate::constraint::solver::{measured_value, refresh_driven_values};
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
    let Entity::Line { start, end, .. } = sketch.entities[l] else {
        unreachable!()
    };
    assert!((end.y - start.y).abs() < 1e-6);
}

#[test]
fn vertical_straightens_a_line() {
    let mut sketch = Sketch::default();
    let l = line(&mut sketch, DVec2::new(0.0, 0.0), DVec2::new(4.0, 10.0));
    let result = solve(&mut sketch, &[Constraint::Vertical { line: l }]);
    assert!(result.converged);
    let Entity::Line { start, end, .. } = sketch.entities[l] else {
        unreachable!()
    };
    assert!((end.x - start.x).abs() < 1e-6);
}

#[test]
fn parallel_aligns_two_line_directions() {
    let mut sketch = Sketch::default();
    let a = line(&mut sketch, DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0));
    let b = line(&mut sketch, DVec2::new(0.0, 5.0), DVec2::new(8.0, 7.0));
    let result = solve(&mut sketch, &[Constraint::Parallel { a, b }]);
    assert!(result.converged);
    let (
        Entity::Line {
            start: sa, end: ea, ..
        },
        Entity::Line {
            start: sb, end: eb, ..
        },
    ) = (sketch.entities[a].clone(), sketch.entities[b].clone())
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
    let (
        Entity::Line {
            start: sa, end: ea, ..
        },
        Entity::Line {
            start: sb, end: eb, ..
        },
    ) = (sketch.entities[a].clone(), sketch.entities[b].clone())
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
            driven: false,
        }],
    );
    assert!(result.converged);
    let Entity::Line { start, end, .. } = sketch.entities[l] else {
        unreachable!()
    };
    assert!(((end - start).length() - 25.0).abs() < 1e-5);
}

#[test]
fn radius_sets_exact_circle_radius() {
    let mut sketch = Sketch::default();
    let c = circle(&mut sketch, DVec2::ZERO, 5.0);
    let result = solve(
        &mut sketch,
        &[Constraint::Radius {
            entity: c,
            value: 12.5,
            driven: false,
        }],
    );
    assert!(result.converged);
    let Entity::Circle { radius, .. } = sketch.entities[c] else {
        unreachable!()
    };
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
    let (
        Entity::Line {
            start: sa, end: ea, ..
        },
        Entity::Line {
            start: sb, end: eb, ..
        },
    ) = (sketch.entities[a].clone(), sketch.entities[b].clone())
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
    let result = solve(
        &mut sketch,
        &[Constraint::Angle {
            a,
            b,
            value: target,
            driven: false,
        }],
    );
    assert!(result.converged);
    let (
        Entity::Line {
            start: sa, end: ea, ..
        },
        Entity::Line {
            start: sb, end: eb, ..
        },
    ) = (sketch.entities[a].clone(), sketch.entities[b].clone())
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
    let Entity::Line { start, end, .. } = sketch.entities[l] else {
        unreachable!()
    };
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
    let (
        Entity::Circle {
            center: ca,
            radius: ra,
            ..
        },
        Entity::Circle {
            center: cb,
            radius: rb,
            ..
        },
    ) = (sketch.entities[a].clone(), sketch.entities[b].clone())
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
    let arc = sketch
        .entities
        .insert(Entity::arc(DVec2::ZERO, 5.0, 0.0, std::f64::consts::PI));
    let c = circle(&mut sketch, DVec2::new(9.0, 0.0), 3.0);
    let result = solve(&mut sketch, &[Constraint::Tangent { a: arc, b: c }]);
    assert!(result.converged);
    let (
        Entity::Arc {
            center: ca,
            radius: ra,
            ..
        },
        Entity::Circle {
            center: cb,
            radius: rb,
            ..
        },
    ) = (sketch.entities[arc].clone(), sketch.entities[c].clone())
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
        Entity::Line {
            start: axis_s,
            end: axis_e,
            ..
        },
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
    let Entity::Line { start, end, .. } = sketch.entities[l] else {
        unreachable!()
    };
    assert!((end.y - start.y).abs() < 1e-6);

    undo.undo(&mut sketch);
    assert_eq!(sketch.constraints.len(), 0);
    let Entity::Line { start, end, .. } = sketch.entities[l] else {
        unreachable!()
    };
    assert!((end.y - start.y - 4.0).abs() < 1e-9);

    undo.redo(&mut sketch);
    assert_eq!(sketch.constraints.len(), 1);
    let Entity::Line { start, end, .. } = sketch.entities[l] else {
        unreachable!()
    };
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
        let b = sketch
            .entities
            .insert(Entity::line(DVec2::new(100.0, 0.0), DVec2::new(110.0, 4.0)));

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
                    driven: false,
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
                    driven: false,
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
                driven: false,
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
        let new_node0 = sp.node(0).unwrap();
        let new_node1 = sp.node(1).unwrap();
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

/// Regresi REVIEW-2026-09-24 #1–#2: constraint yatim/basi tidak boleh membuat
/// solver panic, dan undo harus memulihkannya.
mod stale_references {
    use super::*;
    use crate::commands::{DeleteEntities, UpdateEntity};
    use crate::entity::{PathSeg, Subpath};
    use ducad_core::Command;

    fn two_node_path(sketch: &mut Sketch) -> EntityId {
        sketch.entities.insert(Entity::Path {
            subpaths: vec![Subpath {
                start: DVec2::ZERO,
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
        })
    }

    #[test]
    fn delete_entity_removes_its_constraints_and_solve_does_not_panic() {
        let mut sketch = Sketch::default();
        let a = line(&mut sketch, DVec2::ZERO, DVec2::new(10.0, 1.0));
        let b = line(&mut sketch, DVec2::new(0.0, 5.0), DVec2::new(10.0, 7.0));
        sketch.constraints.push(Constraint::Horizontal { line: a });
        sketch.constraints.push(Constraint::Parallel { a, b });
        sketch.constraints.push(Constraint::Horizontal { line: b });

        let mut cmd = DeleteEntities::new(vec![a]);
        cmd.apply(&mut sketch);

        assert_eq!(
            sketch.constraints.len(),
            1,
            "hanya constraint milik b tersisa"
        );
        let snapshot = sketch.constraints.clone();
        let res = solve(&mut sketch, &snapshot);
        assert!(res.converged);
        assert_eq!(res.skipped, 0);
    }

    #[test]
    fn solve_skips_orphan_constraint_without_panicking() {
        let mut sketch = Sketch::default();
        let a = line(&mut sketch, DVec2::ZERO, DVec2::new(10.0, 1.0));
        let orphan = Constraint::Horizontal { line: a };
        sketch.entities.remove(a);
        let res = solve(&mut sketch, std::slice::from_ref(&orphan));
        assert!(!res.converged, "constraint yatim harus dilaporkan gagal");
        assert_eq!(res.skipped, 1);
        let report = analyze_dof(&sketch, &[orphan]);
        assert_eq!(report.unknowns, 0);
    }

    #[test]
    fn revert_restores_removed_constraints() {
        let mut sketch = Sketch::default();
        let a = line(&mut sketch, DVec2::ZERO, DVec2::new(10.0, 1.0));
        let b = line(&mut sketch, DVec2::new(0.0, 5.0), DVec2::new(10.0, 7.0));
        sketch.constraints.push(Constraint::Horizontal { line: b });
        sketch.constraints.push(Constraint::Parallel { a, b });

        let mut cmd = DeleteEntities::new(vec![a]);
        cmd.apply(&mut sketch);
        assert_eq!(sketch.constraints.len(), 1);
        cmd.revert(&mut sketch);

        assert_eq!(sketch.constraints.len(), 2);
        let restored_a = cmd.restored_ids()[0];
        match &sketch.constraints[1] {
            Constraint::Parallel { a: ra, b: rb } => {
                assert_eq!(*ra, restored_a, "id dipetakan ke entitas hasil pemulihan");
                assert_eq!(*rb, b);
            }
            other => panic!("urutan constraint berubah: {other:?}"),
        }
        let snapshot = sketch.constraints.clone();
        assert!(solve(&mut sketch, &snapshot).converged);
    }

    #[test]
    fn stale_path_node_constraint_is_dropped_not_panicking() {
        let mut sketch = Sketch::default();
        let p = two_node_path(&mut sketch);
        let node2 = PointRef::PathNode {
            id: p,
            sub: 0,
            node: 2,
        };
        sketch.constraints.push(Constraint::Fixed {
            point: node2,
            target: DVec2::new(10.0, 12.0),
        });

        // Path dipendekkan: node 2 hilang.
        let shorter = Entity::Path {
            subpaths: vec![Subpath {
                start: DVec2::ZERO,
                segs: vec![PathSeg::Line {
                    end: DVec2::new(10.0, 0.0),
                }],
                closed: false,
            }],
            is_construction: false,
        };
        let mut cmd = UpdateEntity::new("Edit Node", p, shorter);
        cmd.apply(&mut sketch);
        assert!(
            sketch.constraints.is_empty(),
            "constraint ke node hilang dilepas"
        );

        // Solver dengan rujukan basi (sub & node di luar rentang) tidak panic.
        let stale = [
            Constraint::Fixed {
                point: node2,
                target: DVec2::ZERO,
            },
            Constraint::Fixed {
                point: PointRef::PathNode {
                    id: p,
                    sub: 3,
                    node: 0,
                },
                target: DVec2::ZERO,
            },
        ];
        let res = solve(&mut sketch, &stale);
        assert_eq!(res.skipped, 2);

        cmd.revert(&mut sketch);
        assert_eq!(sketch.constraints.len(), 1, "undo memulihkan constraint");
    }
}

/// Fase 1 plan paritas: titik sketch (1.1), dimensi driven (1.16),
/// constraint kelengkungan G2 (1.20).
mod fase1 {
    use super::*;
    use crate::constraint::solver::{arc_end, spline_end};

    fn pt(sketch: &mut Sketch, p: DVec2) -> EntityId {
        sketch.entities.insert(Entity::point(p))
    }

    fn pos(sketch: &Sketch, id: EntityId) -> DVec2 {
        match sketch.entities[id] {
            Entity::Point { pos, .. } => pos,
            _ => unreachable!(),
        }
    }

    // ---------- 1.1 Point ----------

    #[test]
    fn point_coincident_with_line_endpoint() {
        let mut sk = Sketch::default();
        let l = line(&mut sk, DVec2::ZERO, DVec2::new(10.0, 0.0));
        let p = pt(&mut sk, DVec2::new(3.0, 4.0));
        let res = solve(
            &mut sk,
            &[
                Constraint::Fixed {
                    point: PointRef::LineStart(l),
                    target: DVec2::ZERO,
                },
                Constraint::Fixed {
                    point: PointRef::LineEnd(l),
                    target: DVec2::new(10.0, 0.0),
                },
                Constraint::Coincident {
                    a: PointRef::Center(p),
                    b: PointRef::LineEnd(l),
                },
            ],
        );
        assert!(res.converged);
        assert!((pos(&sk, p) - DVec2::new(10.0, 0.0)).length() < 1e-6);
    }

    #[test]
    fn point_fixed_distance_and_horizontal() {
        let mut sk = Sketch::default();
        let a = pt(&mut sk, DVec2::new(0.0, 0.0));
        let b = pt(&mut sk, DVec2::new(3.0, 1.0));
        let res = solve(
            &mut sk,
            &[
                Constraint::Fixed {
                    point: PointRef::Center(a),
                    target: DVec2::new(1.0, 1.0),
                },
                Constraint::HorizontalPoints {
                    a: PointRef::Center(a),
                    b: PointRef::Center(b),
                },
                Constraint::Distance {
                    a: PointRef::Center(a),
                    b: PointRef::Center(b),
                    value: 7.0,
                    driven: false,
                },
            ],
        );
        assert!(res.converged, "{res:?}");
        assert!((pos(&sk, a) - DVec2::new(1.0, 1.0)).length() < 1e-6);
        assert!((pos(&sk, b).y - 1.0).abs() < 1e-6);
        assert!(((pos(&sk, b) - pos(&sk, a)).length() - 7.0).abs() < 1e-6);
    }

    #[test]
    fn point_on_line_slides_along_curve() {
        let mut sk = Sketch::default();
        let l = line(&mut sk, DVec2::ZERO, DVec2::new(10.0, 0.0));
        let p = pt(&mut sk, DVec2::new(4.0, 2.0));
        let res = solve(
            &mut sk,
            &[
                Constraint::Fixed {
                    point: PointRef::LineStart(l),
                    target: DVec2::ZERO,
                },
                Constraint::Fixed {
                    point: PointRef::LineEnd(l),
                    target: DVec2::new(10.0, 0.0),
                },
                Constraint::PointOnCurve {
                    point: PointRef::Center(p),
                    curve: l,
                },
            ],
        );
        assert!(res.converged);
        assert!(pos(&sk, p).y.abs() < 1e-6);
    }

    #[test]
    fn point_is_snap_target_and_has_no_profile_edges() {
        let mut sk = Sketch::default();
        let p = pt(&mut sk, DVec2::new(5.0, 5.0));
        let hit = crate::snap::find_snap(&sk, DVec2::new(5.2, 4.9), 1.0, 0.0, None).unwrap();
        assert_eq!(hit.point, DVec2::new(5.0, 5.0));
        assert_eq!(hit.source, Some(PointRef::Center(p)));
        assert!(sk.entities[p].flatten_all(0.01).is_empty());
        assert_eq!(sk.entities[p].bounding_box(), Some((DVec2::new(5.0, 5.0), DVec2::new(5.0, 5.0))));
        // Persegi + titik di dalamnya tetap satu region.
        let c = [
            DVec2::ZERO,
            DVec2::new(10.0, 0.0),
            DVec2::new(10.0, 10.0),
            DVec2::new(0.0, 10.0),
        ];
        for i in 0..4 {
            line(&mut sk, c[i], c[(i + 1) % 4]);
        }
        assert_eq!(crate::region::find_closed_regions(&sk).len(), 1);
    }

    #[test]
    fn point_serde_round_trip_and_default_construction() {
        let e = Entity::point(DVec2::new(1.5, -2.0)).with_construction(true);
        let json = serde_json::to_string(&e).unwrap();
        let back: Entity = serde_json::from_str(&json).unwrap();
        assert_eq!(back, e);
        let old: Entity = serde_json::from_str(r#"{"Point":{"pos":[1.0,2.0]}}"#).unwrap();
        assert!(!old.is_construction());
    }

    #[test]
    fn hostile_point_inputs_do_not_panic() {
        let mut sk = Sketch::default();
        let p = pt(&mut sk, DVec2::new(f64::NAN, 0.0));
        let _ = solve(
            &mut sk,
            &[Constraint::Fixed {
                point: PointRef::Center(p),
                target: DVec2::new(f64::INFINITY, 0.0),
            }],
        );
        // Rujukan jenis salah (radius pada titik) dilewati, bukan panic.
        let res = solve(
            &mut sk,
            &[Constraint::Radius {
                entity: p,
                value: 3.0,
                driven: false,
            }],
        );
        assert_eq!(res.skipped, 1);
        let _ = sk.entities[p].distance_to(DVec2::ZERO);
    }

    #[test]
    fn arc_endpoint_refs_follow_angles_in_solver() {
        let mut sk = Sketch::default();
        let l = line(&mut sk, DVec2::ZERO, DVec2::new(10.0, 0.0));
        let arc = sk.entities.insert(Entity::arc(DVec2::new(10.0, 5.0), 5.0, -1.4, 0.0));
        assert_eq!(
            point_ref_position(&sk, &PointRef::LineEnd(arc)),
            Some(DVec2::new(15.0, 5.0))
        );
        let res = solve(
            &mut sk,
            &[
                Constraint::Fixed {
                    point: PointRef::LineEnd(l),
                    target: DVec2::new(10.0, 0.0),
                },
                Constraint::Coincident {
                    a: PointRef::LineEnd(l),
                    b: PointRef::LineStart(arc),
                },
            ],
        );
        assert!(res.converged, "{res:?}");
        let s = point_ref_position(&sk, &PointRef::LineStart(arc)).unwrap();
        assert!((s - DVec2::new(10.0, 0.0)).length() < 1e-6, "{s}");
    }

    // ---------- 1.16 Driven ----------

    #[test]
    fn driven_dimension_is_ignored_by_solver_and_refreshed() {
        let mut sk = Sketch::default();
        let l = line(&mut sk, DVec2::ZERO, DVec2::new(10.0, 0.0));
        let driven = Constraint::Distance {
            a: PointRef::LineStart(l),
            b: PointRef::LineEnd(l),
            value: 25.0,
            driven: true,
        };
        sk.constraints.push(driven.clone());
        let snap = sk.constraints.clone();
        let res = solve(&mut sk, &snap);
        assert!(res.converged);
        assert_eq!(res.skipped, 0, "driven bukan constraint yang dilewati");
        let Entity::Line { end, .. } = sk.entities[l] else {
            unreachable!()
        };
        assert_eq!(end, DVec2::new(10.0, 0.0), "geometri tidak digerakkan");
        assert_eq!(refresh_driven_values(&mut sk), 1);
        let Constraint::Distance { value, .. } = sk.constraints[0] else {
            unreachable!()
        };
        assert!((value - 10.0).abs() < 1e-12);
        // DOF: dimensi driven tidak mengurangi derajat kebebasan.
        let rep = analyze_dof(&sk, &sk.constraints);
        assert_eq!(rep.rank, 0);
        assert!(rep.redundant.is_empty());
    }

    #[test]
    fn driven_flag_serde_is_backward_compatible() {
        let mut sk = Sketch::default();
        let c = circle(&mut sk, DVec2::ZERO, 4.0);
        let normal = Constraint::Radius {
            entity: c,
            value: 4.0,
            driven: false,
        };
        let json = serde_json::to_string(&normal).unwrap();
        assert!(!json.contains("driven"), "berkas tanpa driven tidak berubah: {json}");
        let back: Constraint = serde_json::from_str(&json).unwrap();
        assert!(!back.is_driven());

        let mut d = normal.clone();
        assert!(d.set_driven(true));
        let json = serde_json::to_string(&d).unwrap();
        let back: Constraint = serde_json::from_str(&json).unwrap();
        assert!(back.is_driven());

        let mut h = Constraint::Horizontal { line: c };
        assert!(!h.set_driven(true), "constraint non-dimensi tidak bisa driven");
        assert!(!h.is_driven());
    }

    #[test]
    fn measured_value_angle_and_radius() {
        let mut sk = Sketch::default();
        let a = line(&mut sk, DVec2::ZERO, DVec2::X);
        let b = line(&mut sk, DVec2::ZERO, DVec2::new(0.0, 2.0));
        let ang = Constraint::Angle {
            a,
            b,
            value: 0.0,
            driven: true,
        };
        let v = measured_value(&sk, &ang).unwrap();
        assert!((v - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        let c = circle(&mut sk, DVec2::ZERO, 3.5);
        let r = Constraint::Radius {
            entity: c,
            value: 0.0,
            driven: true,
        };
        assert_eq!(measured_value(&sk, &r), Some(3.5));
        assert_eq!(measured_value(&sk, &Constraint::Horizontal { line: a }), None);
    }

    // ---------- 1.20 Curvature (G2) ----------

    fn spline(sk: &mut Sketch, pts: &[DVec2]) -> EntityId {
        sk.entities.insert(Entity::spline(pts.to_vec()))
    }

    fn spline_pts(sk: &Sketch, id: EntityId) -> Vec<DVec2> {
        match &sk.entities[id] {
            Entity::Spline { points, .. } => points.clone(),
            _ => unreachable!(),
        }
    }

    #[test]
    fn curvature_spline_spline_converges_and_matches() {
        let mut sk = Sketch::default();
        let s1 = spline(
            &mut sk,
            &[DVec2::new(0.0, 0.0), DVec2::new(5.0, 2.0), DVec2::new(10.0, 3.0)],
        );
        let s2 = spline(
            &mut sk,
            &[DVec2::new(10.5, 3.2), DVec2::new(15.0, 1.0), DVec2::new(20.0, -3.0)],
        );
        let fix = |pr: PointRef, at: DVec2| Constraint::Fixed { point: pr, target: at };
        let res = solve(
            &mut sk,
            &[
                fix(PointRef::LineStart(s1), DVec2::new(0.0, 0.0)),
                fix(PointRef::LineEnd(s2), DVec2::new(20.0, -3.0)),
                Constraint::Curvature {
                    a: PointRef::LineEnd(s1),
                    b: PointRef::LineStart(s2),
                },
            ],
        );
        assert!(res.converged, "{res:?}");
        let ea = spline_end(&spline_pts(&sk, s1), true).unwrap();
        let eb = spline_end(&spline_pts(&sk, s2), false).unwrap();
        assert!((ea.pos - eb.pos).length() < 1e-6);
        assert!((ea.tangent - eb.tangent).length() < 1e-6);
        assert!((ea.curvature - eb.curvature).abs() < 1e-6, "{} vs {}", ea.curvature, eb.curvature);
    }

    #[test]
    fn curvature_spline_arc_matches_arc_curvature() {
        let mut sk = Sketch::default();
        // Busur CCW r=5 berpusat di (0,0), ujung awal di (5,0) dengan tangen +Y.
        let arc = sk.entities.insert(Entity::arc(DVec2::ZERO, 5.0, 0.0, 1.2));
        // Spline yang datang ke (5,0) dari bawah, hampir lurus.
        let s = spline(
            &mut sk,
            &[DVec2::new(5.3, -8.0), DVec2::new(5.2, -4.0), DVec2::new(5.1, -0.2)],
        );
        let res = solve(
            &mut sk,
            &[
                Constraint::Fixed {
                    point: PointRef::Center(arc),
                    target: DVec2::ZERO,
                },
                Constraint::Radius {
                    entity: arc,
                    value: 5.0,
                    driven: false,
                },
                Constraint::Curvature {
                    a: PointRef::LineEnd(s),
                    b: PointRef::LineStart(arc),
                },
            ],
        );
        assert!(res.converged, "{res:?}");
        let es = spline_end(&spline_pts(&sk, s), true).unwrap();
        let Entity::Arc {
            center,
            radius,
            start_angle,
            ..
        } = sk.entities[arc]
        else {
            unreachable!()
        };
        let ea = arc_end(center, radius, start_angle).unwrap();
        assert!((es.pos - ea.pos).length() < 1e-6);
        assert!((es.tangent - ea.tangent).length() < 1e-6);
        assert!((es.curvature - 0.2).abs() < 1e-6, "kelengkungan {}", es.curvature);
    }

    #[test]
    fn curvature_rejects_bad_refs_without_panic() {
        let mut sk = Sketch::default();
        let l = line(&mut sk, DVec2::ZERO, DVec2::X);
        let s = spline(&mut sk, &[DVec2::ZERO, DVec2::new(f64::NAN, 1.0), DVec2::new(2.0, 0.0)]);
        let arc = sk.entities.insert(Entity::arc(DVec2::ZERO, 1.0, 0.0, 1.0));
        let arc2 = sk.entities.insert(Entity::arc(DVec2::X, 1.0, 0.0, 1.0));
        let bad = [
            // Garis bukan kurva G2.
            Constraint::Curvature {
                a: PointRef::LineEnd(l),
                b: PointRef::LineStart(s),
            },
            // Busur–busur tanpa spline tidak didukung.
            Constraint::Curvature {
                a: PointRef::LineEnd(arc),
                b: PointRef::LineStart(arc2),
            },
            // Center bukan ujung kurva.
            Constraint::Curvature {
                a: PointRef::Center(s),
                b: PointRef::LineStart(arc),
            },
            // Diri sendiri.
            Constraint::Curvature {
                a: PointRef::LineEnd(s),
                b: PointRef::LineStart(s),
            },
        ];
        let res = solve(&mut sk, &bad);
        assert_eq!(res.skipped, 4);
        // Spline dengan NaN: sah secara rujukan, tidak boleh panic.
        let _ = solve(
            &mut sk,
            &[Constraint::Curvature {
                a: PointRef::LineEnd(s),
                b: PointRef::LineStart(arc),
            }],
        );
        let _ = analyze_dof(
            &sk,
            &[Constraint::Curvature {
                a: PointRef::LineEnd(s),
                b: PointRef::LineStart(arc),
            }],
        );
    }

    #[test]
    fn curvature_serde_and_entity_remap() {
        let mut sk = Sketch::default();
        let s1 = spline(&mut sk, &[DVec2::ZERO, DVec2::X, DVec2::new(2.0, 1.0)]);
        let s2 = spline(&mut sk, &[DVec2::new(2.0, 1.0), DVec2::new(3.0, 3.0), DVec2::new(4.0, 6.0)]);
        let mut c = Constraint::Curvature {
            a: PointRef::LineEnd(s1),
            b: PointRef::LineStart(s2),
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: Constraint = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, Constraint::Curvature { .. }));
        c.map_entity_ids(|id| if id == s1 { s2 } else { s1 });
        let Constraint::Curvature { a, b } = c else {
            unreachable!()
        };
        assert_eq!(a, PointRef::LineEnd(s2));
        assert_eq!(b, PointRef::LineStart(s1));
    }
}

/// Status sketsa untuk UI (Step 2 `PLAN_DESIGNER_UX`).
mod sketch_status_tests {
    use crate::constraint::{sketch_status, Constraint, ConstraintState, PointRef};
    use crate::entity::Entity;
    use crate::sketch::Sketch;
    use glam::DVec2;

    #[test]
    fn unconstrained_line_is_under_and_free() {
        let mut s = Sketch::default();
        let l = s.entities.insert(Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0)));
        let st = sketch_status(&s);
        assert_eq!(st.state, ConstraintState::Under);
        assert_eq!(st.dof, 4);
        assert_eq!(st.free_entities, vec![l]);
    }

    #[test]
    fn fixed_line_is_fully_constrained_and_other_line_stays_free() {
        let mut s = Sketch::default();
        let a = s.entities.insert(Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0)));
        let b = s
            .entities
            .insert(Entity::line(DVec2::new(0.0, 5.0), DVec2::new(10.0, 5.0)));
        s.constraints.push(Constraint::Fixed {
            point: PointRef::LineStart(a),
            target: DVec2::ZERO,
        });
        s.constraints.push(Constraint::Fixed {
            point: PointRef::LineEnd(a),
            target: DVec2::new(10.0, 0.0),
        });
        let st = sketch_status(&s);
        assert_eq!(st.state, ConstraintState::Under);
        assert_eq!(st.free_entities, vec![b], "hanya garis b yang masih bebas");
        assert_eq!(st.dof, 4);

        s.entities.remove(b);
        let st = sketch_status(&s);
        assert_eq!(st.state, ConstraintState::Fully);
        assert_eq!(st.dof, 0);
        assert!(st.free_entities.is_empty());
    }

    #[test]
    fn duplicate_constraint_is_redundant_and_dangling_is_unresolved() {
        let mut s = Sketch::default();
        let a = s.entities.insert(Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0)));
        s.constraints.push(Constraint::Horizontal { line: a });
        s.constraints.push(Constraint::Horizontal { line: a });
        let st = sketch_status(&s);
        assert_eq!(st.state, ConstraintState::Over);
        assert_eq!(st.redundant, vec![1]);

        let gone = s.entities.insert(Entity::line(DVec2::ZERO, DVec2::ONE));
        s.entities.remove(gone);
        s.constraints.push(Constraint::Vertical { line: gone });
        let st = sketch_status(&s);
        assert_eq!(st.unresolved, vec![2]);
    }
}
