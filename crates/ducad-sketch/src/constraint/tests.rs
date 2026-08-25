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
