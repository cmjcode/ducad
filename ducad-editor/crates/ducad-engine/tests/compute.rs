//! Tes integrasi lapisan `compute` (P0.7).

use std::f64::consts::PI;

use ducad_core::hole::{HoleKind, HoleSpec, IsoMetricThread};
use ducad_engine::compute::{self, EdgePick, PrimitiveShape, ProfilePick};
use ducad_engine::model::BooleanKind;
use ducad_engine::{OpErrorCode, PlaneFrame};
use ducad_kernel::ExtrudeExtent;
use ducad_sketch::constraint::Constraint;
use ducad_sketch::{Entity, EntityId, Sketch};
use glam::DVec2;

fn expect_err<T>(r: ducad_engine::OpResult<T>) -> ducad_engine::OpError {
    match r {
        Ok(_) => panic!("diharapkan Err"),
        Err(e) => e,
    }
}

fn rel_close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() / b.abs() < tol
}

fn add_rect(sketch: &mut Sketch, x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<EntityId> {
    let c = [
        DVec2::new(x0, y0),
        DVec2::new(x1, y0),
        DVec2::new(x1, y1),
        DVec2::new(x0, y1),
    ];
    (0..4)
        .map(|i| sketch.entities.insert(Entity::line(c[i], c[(i + 1) % 4])))
        .collect()
}

fn plate(w: f64, h: f64, t: f64) -> ducad_kernel::KernelShape {
    compute::primitive(
        &PrimitiveShape::Box {
            size: [w, h, t],
            centered: false,
        },
        [0.0; 3],
    )
    .unwrap()
    .shape
}

#[test]
fn extrude_all_regions_with_hole() {
    let mut sketch = Sketch::default();
    add_rect(&mut sketch, 0.0, 0.0, 60.0, 40.0);
    sketch
        .entities
        .insert(Entity::circle(DVec2::new(30.0, 20.0), 5.0));
    let solids = compute::extrude(
        &sketch,
        &ProfilePick::AllRegions,
        &PlaneFrame::top(),
        ExtrudeExtent::Blind(8.0),
    )
    .unwrap();
    assert_eq!(solids.len(), 1);
    let v = solids[0].1.shape.volume().abs();
    let expected = 60.0 * 40.0 * 8.0 - PI * 25.0 * 8.0;
    assert!(rel_close(v, expected, 0.01), "{v} vs {expected}");
}

#[test]
fn extrude_at_point_picks_region() {
    let mut sketch = Sketch::default();
    add_rect(&mut sketch, 0.0, 0.0, 10.0, 10.0);
    add_rect(&mut sketch, 20.0, 0.0, 40.0, 10.0);
    let solids = compute::extrude(
        &sketch,
        &ProfilePick::AtPoint(DVec2::new(30.0, 5.0)),
        &PlaneFrame::top(),
        ExtrudeExtent::Blind(2.0),
    )
    .unwrap();
    assert!(rel_close(solids[0].1.shape.volume().abs(), 400.0, 1e-6));
    let err = expect_err(compute::extrude(
        &sketch,
        &ProfilePick::AtPoint(DVec2::new(15.0, 5.0)),
        &PlaneFrame::top(),
        ExtrudeExtent::Blind(2.0),
    ));
    assert_eq!(err.code, OpErrorCode::ProfileAmbiguous);
}

#[test]
fn open_profile_reports_not_closed_with_hint() {
    let mut sketch = Sketch::default();
    let p = [
        DVec2::new(0.0, 0.0),
        DVec2::new(10.0, 0.0),
        DVec2::new(10.0, 5.0),
        DVec2::new(0.0, 5.0),
    ];
    for i in 0..3 {
        sketch.entities.insert(Entity::line(p[i], p[i + 1]));
    }
    let err = expect_err(compute::extrude(
        &sketch,
        &ProfilePick::AllRegions,
        &PlaneFrame::top(),
        ExtrudeExtent::Blind(5.0),
    ));
    assert_eq!(err.code, OpErrorCode::ProfileNotClosed);
    let hint = err.hint.expect("hint wajib ada");
    assert!(hint.contains("3 entitas"), "{hint}");
    assert!(
        hint.contains("(0.000, 0.000)") && hint.contains("(0.000, 5.000)"),
        "{hint}"
    );
}

#[test]
fn fillet_all_reduces_volume() {
    let b = plate(20.0, 20.0, 20.0);
    let before = b.volume().abs();
    let geo = compute::fillet(&b, &EdgePick::All, 1.0).unwrap();
    assert!(geo.shape.is_valid());
    assert!(geo.shape.volume().abs() < before);
}

#[test]
fn fillet_negative_radius_is_invalid_param() {
    let b = plate(20.0, 20.0, 20.0);
    let err = expect_err(compute::fillet(&b, &EdgePick::All, -1.0));
    assert_eq!(err.code, OpErrorCode::InvalidParam);
}

#[test]
fn intersect_disjoint_is_empty_result() {
    let a = plate(10.0, 10.0, 10.0);
    let b = compute::primitive(
        &PrimitiveShape::Box {
            size: [10.0, 10.0, 10.0],
            centered: false,
        },
        [50.0, 0.0, 0.0],
    )
    .unwrap()
    .shape;
    let err = expect_err(compute::boolean(&a, &b, BooleanKind::Intersect));
    assert_eq!(err.code, OpErrorCode::EmptyResult, "{err:?}");
}

#[test]
fn m5_clearance_through_holes() {
    let p = plate(60.0, 40.0, 8.0);
    let mut spec = HoleSpec::for_iso(IsoMetricThread::M5, HoleKind::Simple, 8.0);
    spec.is_through = true;
    let geo = compute::hole(
        &p,
        &spec,
        &[[15.0, 20.0, 8.0], [45.0, 20.0, 8.0]],
        [0.0, 0.0, 1.0],
    )
    .unwrap();
    let removed = p.volume().abs() - geo.shape.volume().abs();
    let expected = 2.0 * PI * 2.75 * 2.75 * 8.0;
    assert!(
        rel_close(removed, expected, 0.02),
        "{removed} vs {expected}"
    );
}

#[test]
fn primitive_is_placed_at_offset() {
    let geo = compute::primitive(
        &PrimitiveShape::Box {
            size: [10.0, 10.0, 10.0],
            centered: false,
        },
        [5.0, 0.0, 0.0],
    )
    .unwrap();
    let (min, max) = geo.mesh.bounding_box().unwrap();
    let want = ([5.0, 0.0, 0.0], [15.0, 10.0, 10.0]);
    for i in 0..3 {
        assert!((min[i] as f64 - want.0[i]).abs() < 1e-3, "{min:?}");
        assert!((max[i] as f64 - want.1[i]).abs() < 1e-3, "{max:?}");
    }
}

#[test]
fn solve_with_perpendicular_converges() {
    let mut sketch = Sketch::default();
    let a = sketch
        .entities
        .insert(Entity::line(DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0)));
    let b = sketch
        .entities
        .insert(Entity::line(DVec2::new(0.0, 0.0), DVec2::new(3.0, 9.0)));
    let (solved, result, _) =
        compute::solve_with(&sketch, &[Constraint::Perpendicular { a, b }]).unwrap();
    assert!(result.converged);
    assert_eq!(solved.constraints.len(), 1);
    assert!(
        sketch.constraints.is_empty(),
        "sketch asli tidak boleh berubah"
    );
}

#[test]
fn solve_with_horizontal_and_vertical_same_line_fails() {
    let mut sketch = Sketch::default();
    let line = sketch
        .entities
        .insert(Entity::line(DVec2::new(0.0, 0.0), DVec2::new(10.0, 3.0)));
    let err = compute::solve_with(
        &sketch,
        &[
            Constraint::Horizontal { line },
            Constraint::Vertical { line },
        ],
    )
    .unwrap_err();
    assert!(
        matches!(
            err.code,
            OpErrorCode::ConstraintUnsolved | OpErrorCode::OverConstrained
        ),
        "{err:?}"
    );
}

#[test]
fn revolve_on_front_plane_is_placed() {
    // Persegi 5×10 di x=10..15, y=0..10 (koordinat bidang) diputar 360°
    // mengelilingi sumbu v: di bidang Front sumbu itu = sumbu Z dunia.
    let mut sketch = Sketch::default();
    add_rect(&mut sketch, 10.0, 0.0, 15.0, 10.0);
    let geo = compute::revolve(
        &sketch,
        &ProfilePick::AllRegions,
        &PlaneFrame::front(),
        DVec2::ZERO,
        DVec2::Y,
        None,
    )
    .unwrap();
    let expected = PI * (15.0f64.powi(2) - 10.0f64.powi(2)) * 10.0;
    assert!(rel_close(geo.shape.volume().abs(), expected, 1e-3));
    let (min, max) = geo.mesh.bounding_box().unwrap();
    assert!(
        (min[2] as f64).abs() < 1e-2 && (max[2] as f64 - 10.0).abs() < 1e-2,
        "{min:?} {max:?}"
    );
}
