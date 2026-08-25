use super::*;
use glam::DVec2;

#[test]
fn hit_test_finds_nearest_line() {
    let mut sketch = Sketch::default();
    sketch.entities.insert(Entity::line(
        DVec2::new(0.0, 0.0),
        DVec2::new(10.0, 0.0),
    ));
    assert!(sketch.hit_test(DVec2::new(5.0, 0.3), 0.5).is_some());
    assert!(sketch.hit_test(DVec2::new(5.0, 5.0), 0.5).is_none());
}

#[test]
fn snap_prefers_endpoint_over_grid() {
    let mut sketch = Sketch::default();
    sketch.entities.insert(Entity::line(
        DVec2::new(10.2, 0.1),
        DVec2::new(20.0, 0.0),
    ));
    let hit = find_snap(&sketch, DVec2::new(10.0, 0.0), 2.0, 10.0, None).unwrap();
    assert_eq!(hit.kind, SnapKind::Endpoint);
    assert!((hit.point - DVec2::new(10.2, 0.1)).length() < 1e-9);
    assert!(hit.source.is_some(), "snap Endpoint harus bawa PointRef sumber");
}

#[test]
fn snap_source_is_none_for_derived_points() {
    let mut sketch = Sketch::default();
    sketch.entities.insert(Entity::line(
        DVec2::new(-5.0, 0.0),
        DVec2::new(15.0, 0.0),
    ));
    sketch.entities.insert(Entity::line(
        DVec2::new(0.0, -5.0),
        DVec2::new(0.0, 15.0),
    ));
    let hit = find_snap(&sketch, DVec2::new(0.3, 0.3), 1.0, 1000.0, None).unwrap();
    assert_eq!(hit.kind, SnapKind::Intersection);
    assert!(hit.source.is_none());

    let sketch = Sketch::default();
    let hit = find_snap(&sketch, DVec2::new(19.6, 0.2), 1.0, 10.0, None).unwrap();
    assert_eq!(hit.kind, SnapKind::Grid);
    assert!(hit.source.is_none());
}

#[test]
fn snap_center_carries_point_ref() {
    let mut sketch = Sketch::default();
    let c = sketch.entities.insert(Entity::circle(
        DVec2::new(5.0, 5.0),
        3.0,
    ));
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
    sketch.entities.insert(Entity::line(
        DVec2::new(-5.0, 0.0),
        DVec2::new(15.0, 0.0),
    ));
    sketch.entities.insert(Entity::line(
        DVec2::new(0.0, -5.0),
        DVec2::new(0.0, 15.0),
    ));
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
            vec![Entity::line(
                DVec2::ZERO,
                DVec2::new(1.0, 0.0),
            )],
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
        ..
    } = arc
    else {
        panic!("bukan Arc");
    };
    for p in [p1, p2, p3] {
        assert!(((p - center).length() - radius).abs() < 1e-9);
    }
    let angle_p2 = (p2 - center).y.atan2((p2 - center).x);
    assert!(crate::entity::angle_in_range(angle_p2, start_angle, end_angle));
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
    let line = Entity::line(
        DVec2::new(0.0, 0.0),
        DVec2::new(10.0, 0.0),
    );
    let offset = offset_entity(&line, DVec2::new(5.0, 3.0)).unwrap();
    assert_eq!(
        offset,
        Entity::line(
            DVec2::new(0.0, 3.0),
            DVec2::new(10.0, 3.0),
        )
    );
}

#[test]
fn offset_circle_uses_reference_distance_as_new_radius() {
    let circle = Entity::circle(
        DVec2::ZERO,
        5.0,
    );
    let offset = offset_entity(&circle, DVec2::new(8.0, 0.0)).unwrap();
    assert_eq!(
        offset,
        Entity::circle(
            DVec2::ZERO,
            8.0,
        )
    );
}

#[test]
fn offset_ellipse_is_unsupported() {
    let ellipse = Entity::ellipse(
        DVec2::ZERO,
        5.0,
        3.0,
    );
    assert!(offset_entity(&ellipse, DVec2::new(8.0, 0.0)).is_none());
}

#[test]
fn mirror_line_across_vertical_axis() {
    let line = Entity::line(
        DVec2::new(1.0, 0.0),
        DVec2::new(3.0, 4.0),
    );
    let mirrored = mirror_entity(&line, DVec2::new(0.0, 0.0), DVec2::new(0.0, 1.0)).unwrap();
    assert_eq!(
        mirrored,
        Entity::line(
            DVec2::new(-1.0, 0.0),
            DVec2::new(-3.0, 4.0),
        )
    );
}

#[test]
fn mirror_none_when_axis_degenerate() {
    let line = Entity::line(
        DVec2::ZERO,
        DVec2::new(1.0, 1.0),
    );
    assert!(mirror_entity(&line, DVec2::ZERO, DVec2::ZERO).is_none());
}

#[test]
fn trim_removes_middle_segment_between_two_cuts() {
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
    let id = sketch.entities.insert(Entity::line(
        DVec2::ZERO,
        DVec2::new(10.0, 0.0),
    ));

    undo.execute(
        Box::new(ReplaceEntities::new(
            "Trim",
            vec![id],
            vec![Entity::line(
                DVec2::ZERO,
                DVec2::new(3.0, 0.0),
            )],
        )),
        &mut sketch,
    );
    assert_eq!(sketch.entities.len(), 1);
    assert!(!sketch.entities.contains_key(id));

    undo.undo(&mut sketch);
    assert_eq!(sketch.entities.len(), 1);
    assert_eq!(
        sketch.entities.values().next().unwrap(),
        &Entity::line(
            DVec2::ZERO,
            DVec2::new(10.0, 0.0),
        )
    );
}

#[test]
fn update_entity_preserves_id_and_undo_roundtrip() {
    let mut sketch = Sketch::default();
    let mut undo = UndoStack::default();
    let id = sketch.entities.insert(Entity::circle(
        DVec2::ZERO,
        10.0,
    ));

    undo.execute(
        Box::new(UpdateEntity::new(
            "Ubah Radius",
            id,
            Entity::circle(
                DVec2::ZERO,
                25.0,
            ),
        )),
        &mut sketch,
    );

    assert_eq!(sketch.entities.len(), 1);
    assert!(sketch.entities.contains_key(id));
    assert_eq!(
        sketch.entities.get(id).unwrap(),
        &Entity::circle(
            DVec2::ZERO,
            25.0,
        )
    );

    undo.undo(&mut sketch);
    assert_eq!(
        sketch.entities.get(id).unwrap(),
        &Entity::circle(
            DVec2::ZERO,
            10.0,
        )
    );

    undo.redo(&mut sketch);
    assert_eq!(
        sketch.entities.get(id).unwrap(),
        &Entity::circle(
            DVec2::ZERO,
            25.0,
        )
    );
}

#[test]
fn translate_entity_shifts_all_variants() {
    let delta = DVec2::new(5.0, -2.0);
    let line = Entity::line(
        DVec2::ZERO,
        DVec2::new(10.0, 0.0),
    );
    assert_eq!(
        translate_entity(&line, delta),
        Entity::line(
            delta,
            DVec2::new(15.0, -2.0),
        )
    );

    let circle = Entity::circle(
        DVec2::new(1.0, 1.0),
        3.0,
    );
    assert_eq!(
        translate_entity(&circle, delta),
        Entity::circle(
            DVec2::new(6.0, -1.0),
            3.0,
        )
    );
}

#[test]
fn translate_entities_undo_roundtrip_preserves_id() {
    let mut sketch = Sketch::default();
    let mut undo = UndoStack::default();
    let id = sketch.entities.insert(Entity::circle(
        DVec2::ZERO,
        5.0,
    ));

    undo.execute(
        Box::new(TranslateEntities::new("Geser X", vec![id], DVec2::new(12.0, 0.0))),
        &mut sketch,
    );
    assert_eq!(
        sketch.entities.get(id).unwrap(),
        &Entity::circle(
            DVec2::new(12.0, 0.0),
            5.0,
        )
    );

    undo.undo(&mut sketch);
    assert_eq!(
        sketch.entities.get(id).unwrap(),
        &Entity::circle(
            DVec2::ZERO,
            5.0,
        )
    );
}

#[test]
fn test_entity_visibility_toggle_and_hit_test() {
    let mut sketch = Sketch::default();
    let id = sketch.entities.insert(Entity::line(
        DVec2::new(0.0, 0.0),
        DVec2::new(10.0, 0.0),
    ));

    assert!(sketch.is_visible(id));
    assert!(!sketch.is_hidden(id));
    assert!(sketch.hit_test(DVec2::new(5.0, 0.1), 0.5).is_some());

    // Sembunyikan entitas
    let now_visible = sketch.toggle_visibility(id);
    assert!(!now_visible);
    assert!(sketch.is_hidden(id));
    assert!(!sketch.is_visible(id));
    assert!(sketch.hit_test(DVec2::new(5.0, 0.1), 0.5).is_none());

    // Snap juga harus mengabaikan entitas yang disembunyikan
    let snap = find_snap(&sketch, DVec2::new(0.1, 0.0), 0.5, 100.0, None);
    assert!(snap.is_none() || snap.unwrap().kind == SnapKind::Grid);

    // Tampilkan kembali
    let now_visible2 = sketch.toggle_visibility(id);
    assert!(now_visible2);
    assert!(sketch.is_visible(id));
    assert!(sketch.hit_test(DVec2::new(5.0, 0.1), 0.5).is_some());
}

#[test]
fn test_spline_endpoints_hit_test_and_transform() {
    let mut sketch = Sketch::default();
    let pts = vec![
        DVec2::new(0.0, 0.0),
        DVec2::new(10.0, 5.0),
        DVec2::new(20.0, -5.0),
        DVec2::new(30.0, 0.0),
    ];
    let spline = Entity::spline(pts.clone());

    assert_eq!(
        spline.endpoints(),
        vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(30.0, 0.0),
            DVec2::new(10.0, 5.0),
            DVec2::new(20.0, -5.0),
        ]
    );
    assert_eq!(spline.center(), Some(DVec2::new(15.0, 0.0)));

    let id = sketch.entities.insert(spline);

    // Snap to start point, end point, and intermediate control points
    let snap_start = find_snap(&sketch, DVec2::new(0.1, 0.1), 0.5, 100.0, None).unwrap();
    assert_eq!(snap_start.kind, SnapKind::Endpoint);
    assert_eq!(snap_start.point, DVec2::new(0.0, 0.0));

    let snap_mid = find_snap(&sketch, DVec2::new(10.1, 4.9), 0.5, 100.0, None).unwrap();
    assert_eq!(snap_mid.kind, SnapKind::Endpoint);
    assert_eq!(snap_mid.point, DVec2::new(10.0, 5.0));

    // Hit test near fit points
    assert!(sketch.hit_test(DVec2::new(10.0, 5.1), 0.5).is_some());
    assert!(sketch.hit_test(DVec2::new(20.0, -4.9), 0.5).is_some());
    assert!(sketch.hit_test(DVec2::new(100.0, 100.0), 1.0).is_none());

    // Test snap with extra/pending points (e.g. while drawing)
    let pending = vec![DVec2::new(50.0, 50.0)];
    let snap_extra = find_snap_with_extra(&sketch, DVec2::new(50.1, 49.9), 0.5, 100.0, None, &pending).unwrap();
    assert_eq!(snap_extra.kind, SnapKind::Endpoint);
    assert_eq!(snap_extra.point, DVec2::new(50.0, 50.0));

    // Translate
    let translated = translate_entity(sketch.entities.get(id).unwrap(), DVec2::new(5.0, 10.0));
    if let Entity::Spline { points, .. } = translated {
        assert_eq!(points[0], DVec2::new(5.0, 10.0));
        assert_eq!(points[3], DVec2::new(35.0, 10.0));
    } else {
        panic!("expected Spline");
    }

    // Mirror across Y axis (axis along X=0, from (0,0) to (0,1))
    let mirrored = mirror_entity(sketch.entities.get(id).unwrap(), DVec2::ZERO, DVec2::new(0.0, 1.0)).unwrap();
    if let Entity::Spline { points, .. } = mirrored {
        assert_eq!(points[0], DVec2::new(0.0, 0.0));
        assert_eq!(points[1], DVec2::new(-10.0, 5.0));
        assert_eq!(points[3], DVec2::new(-30.0, 0.0));
    } else {
        panic!("expected Spline");
    }
}

#[test]
fn test_spline_closed_region() {
    let mut sketch = Sketch::default();
    // Spline curve from (0,0) to (30,0) with a dip
    sketch.entities.insert(Entity::spline(vec![
        DVec2::new(0.0, 0.0),
        DVec2::new(10.0, 10.0),
        DVec2::new(20.0, 10.0),
        DVec2::new(30.0, 0.0),
    ]));
    // Line 1: (30, 0) to (30, -10)
    sketch.entities.insert(Entity::line(
        DVec2::new(30.0, 0.0),
        DVec2::new(30.0, -10.0),
    ));
    // Line 2: (30, -10) to (0, -10)
    sketch.entities.insert(Entity::line(
        DVec2::new(30.0, -10.0),
        DVec2::new(0.0, -10.0),
    ));
    // Line 3: (0, -10) to (0, 0)
    sketch.entities.insert(Entity::line(
        DVec2::new(0.0, -10.0),
        DVec2::new(0.0, 0.0),
    ));

    let regions = crate::region::find_closed_regions(&sketch);
    assert_eq!(regions.len(), 1);
    assert!(regions[0].contains_point(DVec2::new(15.0, 0.0)));
    assert!(regions[0].contains_point(DVec2::new(15.0, -5.0)));
}

#[test]
fn test_fillet_2d_right_angle() {
    use crate::ops::compute_fillet_2d;

    // Line 1: (0, 0) to (10, 0)
    // Line 2: (10, 0) to (10, 10)
    // Corner at (10, 0)
    let l1 = (DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0));
    let l2 = (DVec2::new(10.0, 0.0), DVec2::new(10.0, 10.0));

    let res = compute_fillet_2d(l1, l2, 2.0).expect("fillet should succeed");
    assert!((res.tangent1 - DVec2::new(8.0, 0.0)).length() < 1e-6);
    assert!((res.tangent2 - DVec2::new(10.0, 2.0)).length() < 1e-6);
    assert!((res.center - DVec2::new(8.0, 2.0)).length() < 1e-6);

    if let Entity::Line { start, end, .. } = res.trimmed_line1 {
        assert!((start - DVec2::new(0.0, 0.0)).length() < 1e-6);
        assert!((end - DVec2::new(8.0, 0.0)).length() < 1e-6);
    } else {
        panic!("expected Line");
    }

    if let Entity::Line { start, end, .. } = res.trimmed_line2 {
        assert!((start - DVec2::new(10.0, 2.0)).length() < 1e-6);
        assert!((end - DVec2::new(10.0, 10.0)).length() < 1e-6);
    } else {
        panic!("expected Line");
    }

    if let Entity::Arc { center, radius, .. } = res.arc {
        assert!((center - DVec2::new(8.0, 2.0)).length() < 1e-6);
        assert!((radius - 2.0).abs() < 1e-6);
    } else {
        panic!("expected Arc");
    }

    // Radius too large (exceeds segment length) -> None
    assert!(compute_fillet_2d(l1, l2, 15.0).is_none());
}

#[test]
fn test_fillet_2d_acute_and_obtuse() {
    use crate::ops::compute_fillet_2d;

    // Angle of 60 degrees (acute)
    let l1 = (DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0));
    let l2 = (DVec2::new(10.0, 0.0), DVec2::new(15.0, 5.0 * 3.0f64.sqrt()));

    let res = compute_fillet_2d(l1, l2, 1.5);
    assert!(res.is_some());

    // Parallel lines -> None
    let p1 = (DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0));
    let p2 = (DVec2::new(0.0, 5.0), DVec2::new(10.0, 5.0));
    assert!(compute_fillet_2d(p1, p2, 1.0).is_none());
}

#[test]
fn test_chamfer_2d() {
    use crate::ops::compute_chamfer_2d;

    let l1 = (DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0));
    let l2 = (DVec2::new(10.0, 0.0), DVec2::new(10.0, 10.0));

    let res = compute_chamfer_2d(l1, l2, 3.0, 4.0).expect("chamfer should succeed");
    assert_eq!(res.tangent1, DVec2::new(7.0, 0.0));
    assert_eq!(res.tangent2, DVec2::new(10.0, 4.0));

    if let Entity::Line { start, end, .. } = res.bevel_line {
        assert_eq!(start, DVec2::new(7.0, 0.0));
        assert_eq!(end, DVec2::new(10.0, 4.0));
    } else {
        panic!("expected Line");
    }

    // Chamfer distance exceeds length -> None
    assert!(compute_chamfer_2d(l1, l2, 12.0, 2.0).is_none());
}

#[test]
fn test_find_corner_lines_at_point() {
    use crate::ops::find_corner_lines_at_point;

    let mut sketch = Sketch::default();
    let id1 = sketch.entities.insert(Entity::line(
        DVec2::new(0.0, 0.0),
        DVec2::new(10.0, 0.0),
    ));
    let id2 = sketch.entities.insert(Entity::line(
        DVec2::new(10.0, 0.0),
        DVec2::new(10.0, 10.0),
    ));

    let found = find_corner_lines_at_point(&sketch, DVec2::new(10.0, 0.0), 0.5);
    assert!(found.is_some());
    let (f1, f2, corner) = found.unwrap();
    assert_eq!(corner, DVec2::new(10.0, 0.0));
    assert!((f1 == id1 && f2 == id2) || (f1 == id2 && f2 == id1));
}

#[test]
fn test_rotate_entity() {
    use crate::ops::rotate_entity;

    // Line from (10, 0) to (20, 0) rotated 90 deg around (0, 0) -> (0, 10) to (0, 20)
    let line = Entity::line(
        DVec2::new(10.0, 0.0),
        DVec2::new(20.0, 0.0),
    );
    let rotated = rotate_entity(&line, DVec2::ZERO, std::f64::consts::FRAC_PI_2);
    if let Entity::Line { start, end, .. } = rotated {
        assert!((start.x - 0.0).abs() < 1e-5);
        assert!((start.y - 10.0).abs() < 1e-5);
        assert!((end.x - 0.0).abs() < 1e-5);
        assert!((end.y - 20.0).abs() < 1e-5);
    } else {
        panic!("expected Line");
    }

    // Circle centered at (10, 0) radius 5 rotated 180 deg around (0, 0) -> center (-10, 0)
    let circle = Entity::circle(
        DVec2::new(10.0, 0.0),
        5.0,
    );
    let rot_c = rotate_entity(&circle, DVec2::ZERO, std::f64::consts::PI);
    if let Entity::Circle { center, radius, .. } = rot_c {
        assert!((center.x - (-10.0)).abs() < 1e-5);
        assert!((center.y - 0.0).abs() < 1e-5);
        assert_eq!(radius, 5.0);
    } else {
        panic!("expected Circle");
    }
}

#[test]
fn test_linear_pattern_entities() {
    use crate::ops::linear_pattern_entities;

    let circle = Entity::circle(
        DVec2::new(0.0, 0.0),
        4.0,
    );
    // 3 x 2 grid with pitch X = 20, pitch Y = 30
    // Total copies generated = 3*2 - 1 = 5 new entities
    let pattern = linear_pattern_entities(&[circle], 3, 20.0, 2, 30.0);
    assert_eq!(pattern.len(), 5);

    // Check that we have a circle at (40, 30) (ix=2, iy=1)
    let has_corner = pattern.iter().any(|e| match e {
        Entity::Circle { center, radius, .. } => {
            (center.x - 40.0).abs() < 1e-5 && (center.y - 30.0).abs() < 1e-5 && *radius == 4.0
        }
        _ => false,
    });
    assert!(has_corner);
}

#[test]
fn test_circular_pattern_entities() {
    use crate::ops::circular_pattern_entities;

    let circle = Entity::circle(
        DVec2::new(10.0, 0.0),
        2.0,
    );
    // 4 items around origin, 360 degrees (TAU) -> step 90 deg (PI/2)
    // Expect 3 new items at (0, 10), (-10, 0), (0, -10)
    let pattern = circular_pattern_entities(&[circle], DVec2::ZERO, 4, std::f64::consts::TAU);
    assert_eq!(pattern.len(), 3);

    let centers: Vec<DVec2> = pattern
        .iter()
        .filter_map(|e| match e {
            Entity::Circle { center, .. } => Some(*center),
            _ => None,
        })
        .collect();

    assert_eq!(centers.len(), 3);
    assert!((centers[0].x - 0.0).abs() < 1e-4 && (centers[0].y - 10.0).abs() < 1e-4);
    assert!((centers[1].x - (-10.0)).abs() < 1e-4 && (centers[1].y - 0.0).abs() < 1e-4);
    assert!((centers[2].x - 0.0).abs() < 1e-4 && (centers[2].y - (-10.0)).abs() < 1e-4);
}

#[test]
fn test_circular_pattern_entities_with_radius() {
    use crate::ops::circular_pattern_entities_with_radius;

    let circle_at_origin = Entity::circle(
        DVec2::ZERO,
        3.0,
    );
    // 4 items with custom radius = 25.0 mm around origin -> template at origin (dist 0 != 25)
    // Jadi semua 4 posisi (0°, 90°, 180°, 270°) dibuatkan salinan di lingkaran orbit
    let pattern = circular_pattern_entities_with_radius(
        &[circle_at_origin],
        DVec2::ZERO,
        4,
        std::f64::consts::TAU,
        Some(25.0),
    );
    assert_eq!(pattern.len(), 4);

    let centers: Vec<DVec2> = pattern
        .iter()
        .filter_map(|e| match e {
            Entity::Circle { center, .. } => Some(*center),
            _ => None,
        })
        .collect();

    assert_eq!(centers.len(), 4);
    assert!((centers[0].x - 25.0).abs() < 1e-4 && (centers[0].y - 0.0).abs() < 1e-4);
    assert!((centers[1].x - 0.0).abs() < 1e-4 && (centers[1].y - 25.0).abs() < 1e-4);
    assert!((centers[2].x - (-25.0)).abs() < 1e-4 && (centers[2].y - 0.0).abs() < 1e-4);
    assert!((centers[3].x - 0.0).abs() < 1e-4 && (centers[3].y - (-25.0)).abs() < 1e-4);

    // Kasus objek asli sudah berada di radius 25.0 (mis. di (25, 0)) -> hanya 3 salinan tambahan
    let circle_at_25 = Entity::circle(
        DVec2::new(25.0, 0.0),
        3.0,
    );
    let pattern_on_orbit = circular_pattern_entities_with_radius(
        &[circle_at_25],
        DVec2::ZERO,
        4,
        std::f64::consts::TAU,
        Some(25.0),
    );
    assert_eq!(pattern_on_orbit.len(), 3);
}

#[test]
fn test_snap_to_closed_region_centroid() {
    use crate::snap::{all_snap_candidate_points_with_exclude_set, find_snap_with_exclude_set};
    use std::collections::HashSet;

    let mut sketch = Sketch::default();
    // Buat persegi panjang dari 4 garis: (0,0) ke (20,10), centroid = (10, 5)
    let _id1 = sketch.entities.insert(Entity::line(
        DVec2::new(0.0, 0.0),
        DVec2::new(20.0, 0.0),
    ));
    let _id2 = sketch.entities.insert(Entity::line(
        DVec2::new(20.0, 0.0),
        DVec2::new(20.0, 10.0),
    ));
    let _id3 = sketch.entities.insert(Entity::line(
        DVec2::new(20.0, 10.0),
        DVec2::new(0.0, 10.0),
    ));
    let _id4 = sketch.entities.insert(Entity::line(
        DVec2::new(0.0, 10.0),
        DVec2::new(0.0, 0.0),
    ));

    // Lingkaran di (50, 50) dengan radius 10
    let id_circle = sketch.entities.insert(Entity::circle(
        DVec2::new(50.0, 50.0),
        10.0,
    ));

    // Snap ke centroid persegi panjang di (10, 5)
    let hit_rect_center = find_snap(&sketch, DVec2::new(10.2, 4.9), 1.0, 100.0, None).unwrap();
    assert_eq!(hit_rect_center.kind, SnapKind::Center);
    assert!((hit_rect_center.point.x - 10.0).abs() < 1e-4 && (hit_rect_center.point.y - 5.0).abs() < 1e-4);

    // Snap ke pusat lingkaran di (50, 50)
    let hit_circle_center = find_snap(&sketch, DVec2::new(49.8, 50.1), 1.0, 100.0, None).unwrap();
    assert_eq!(hit_circle_center.kind, SnapKind::Center);
    assert_eq!(hit_circle_center.point, DVec2::new(50.0, 50.0));

    // Test exclude set: jika id_circle di-exclude, tidak boleh snap ke lingkaran
    let mut exclude_circle = HashSet::new();
    exclude_circle.insert(id_circle);
    let hit_excluded = find_snap_with_exclude_set(&sketch, DVec2::new(49.8, 50.1), 1.0, 100.0, Some(&exclude_circle), &[]);
    assert!(hit_excluded.is_none());

    // Test all_snap_candidate_points
    let candidates = all_snap_candidate_points_with_exclude_set(&sketch, None);
    let center_candidates: Vec<DVec2> = candidates
        .iter()
        .filter(|(_, k)| *k == SnapKind::Center)
        .map(|(p, _)| *p)
        .collect();

    // Harus ada center lingkaran (50,50) dan center persegi panjang (10,5)
    assert!(center_candidates.iter().any(|p| (p.x - 50.0).abs() < 1e-4 && (p.y - 50.0).abs() < 1e-4));
    assert!(center_candidates.iter().any(|p| (p.x - 10.0).abs() < 1e-4 && (p.y - 5.0).abs() < 1e-4));
}

#[test]
fn test_bounding_box_entities_and_sketch() {
    let mut sketch = Sketch::default();
    assert!(sketch.bounding_box().is_none());

    // Line from (-100, -50) to (200, 300)
    let _id1 = sketch.entities.insert(Entity::line(
        DVec2::new(-100.0, -50.0),
        DVec2::new(200.0, 300.0),
    ));

    let (min, max) = sketch.bounding_box().unwrap();
    assert_eq!(min, DVec2::new(-100.0, -50.0));
    assert_eq!(max, DVec2::new(200.0, 300.0));

    // Circle at (600, 0) with radius 150 -> x in [450, 750], y in [-150, 150]
    let id2 = sketch.entities.insert(Entity::circle(
        DVec2::new(600.0, 0.0),
        150.0,
    ));

    let (min, max) = sketch.bounding_box().unwrap();
    assert_eq!(min, DVec2::new(-100.0, -150.0));
    assert_eq!(max, DVec2::new(750.0, 300.0));

    // Hide circle -> bounding box reverts to line
    sketch.set_visible(id2, false);
    let (min, max) = sketch.bounding_box().unwrap();
    assert_eq!(min, DVec2::new(-100.0, -50.0));
    assert_eq!(max, DVec2::new(200.0, 300.0));
}

#[test]
fn test_construction_entities_and_closed_regions() {
    let mut sketch = Sketch::default();

    // 1. Line biasa vs garis konstruksi
    let mut normal_line = Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0));
    assert!(!normal_line.is_construction());
    normal_line.set_construction(true);
    assert!(normal_line.is_construction());

    let construction_circle = Entity::circle(DVec2::ZERO, 20.0).with_construction(true);
    assert!(construction_circle.is_construction());

    // 2. Garis konstruksi lingkaran tidak dihitung sebagai ClosedRegion
    let c_id = sketch.entities.insert(construction_circle);
    let regions = crate::region::find_closed_regions(&sketch);
    assert_eq!(regions.len(), 0, "Garis konstruksi lingkaran tidak boleh membentuk closed region");

    // 3. Toggle construction command
    let mut undo = UndoStack::default();
    undo.execute(
        Box::new(ToggleConstruction::new(vec![c_id], false)),
        &mut sketch,
    );
    assert!(!sketch.entities[c_id].is_construction());
    let regions_after = crate::region::find_closed_regions(&sketch);
    assert_eq!(regions_after.len(), 1, "Setelah di-toggle ke normal, lingkaran harus membentuk closed region");

    undo.undo(&mut sketch);
    assert!(sketch.entities[c_id].is_construction());
    assert_eq!(crate::region::find_closed_regions(&sketch).len(), 0);

    // 4. Garis konstruksi yang membelah persegi panjang tidak merusak/memecah closed region
    let _l1 = sketch.entities.insert(Entity::line(DVec2::new(0.0, 0.0), DVec2::new(20.0, 0.0)));
    let _l2 = sketch.entities.insert(Entity::line(DVec2::new(20.0, 0.0), DVec2::new(20.0, 20.0)));
    let _l3 = sketch.entities.insert(Entity::line(DVec2::new(20.0, 20.0), DVec2::new(0.0, 20.0)));
    let _l4 = sketch.entities.insert(Entity::line(DVec2::new(0.0, 20.0), DVec2::new(0.0, 0.0)));
    // Center reference line
    let _centerline = sketch.entities.insert(Entity::line(DVec2::new(10.0, 0.0), DVec2::new(10.0, 20.0)).with_construction(true));

    let rect_regions = crate::region::find_closed_regions(&sketch);
    assert_eq!(rect_regions.len(), 1, "Persegi panjang dengan garis konstruksi di tengah harus tetap 1 closed region");
    assert_eq!(rect_regions[0].entity_ids.len(), 4);
    assert!((rect_regions[0].area - 400.0).abs() < 1e-4);
}

#[test]
fn test_regular_polygon_inscribed_and_circumscribed() {
    use crate::ops::{regular_polygon_entities, regular_polygon_vertices, PolygonMode};

    let center = DVec2::new(10.0, 20.0);
    let p2 = DVec2::new(20.0, 20.0); // radius 10.0 along +X

    // 1. Inscribed Hexagon (N=6)
    let verts_inscribed = regular_polygon_vertices(center, p2, 6, PolygonMode::Inscribed).unwrap();
    assert_eq!(verts_inscribed.len(), 6);
    // Vertex 0 must equal p2 exactly
    assert!((verts_inscribed[0] - p2).length() < 1e-6);
    // All vertices must be distance 10.0 from center
    for v in &verts_inscribed {
        assert!(((v - center).length() - 10.0).abs() < 1e-6);
    }

    // 2. Circumscribed Hexagon (N=6)
    let verts_circumscribed = regular_polygon_vertices(center, p2, 6, PolygonMode::Circumscribed).unwrap();
    assert_eq!(verts_circumscribed.len(), 6);
    // Midpoint of first edge (v0 to v1) must equal p2 (tangent to circle at p2)
    let mid01 = (verts_circumscribed[0] + verts_circumscribed[1]) * 0.5;
    assert!((mid01 - p2).length() < 1e-6, "Titik tengah sisi pertama harus berada tepat di p2");
    // Circumradius = 10.0 / cos(30 deg) = 20 / sqrt(3) ~= 11.547
    let expected_r_v = 10.0 / (std::f64::consts::PI / 6.0).cos();
    for v in &verts_circumscribed {
        assert!(((v - center).length() - expected_r_v).abs() < 1e-6);
    }

    // 3. Circumscribed Square (N=4)
    let verts_sq = regular_polygon_vertices(center, p2, 4, PolygonMode::Circumscribed).unwrap();
    assert_eq!(verts_sq.len(), 4);
    let mid_sq = (verts_sq[0] + verts_sq[1]) * 0.5;
    assert!((mid_sq - p2).length() < 1e-6);
    // Side length of circumscribed square with in-radius 10 is 20
    let side_len = (verts_sq[1] - verts_sq[0]).length();
    assert!((side_len - 20.0).abs() < 1e-6);

    // 4. Entities and Closed Region
    let mut sketch = Sketch::default();
    let entities = regular_polygon_entities(center, p2, 6, PolygonMode::Inscribed, false).unwrap();
    assert_eq!(entities.len(), 6);
    for e in entities {
        sketch.entities.insert(e);
    }
    let regions = crate::region::find_closed_regions(&sketch);
    assert_eq!(regions.len(), 1, "Hexagon harus membentuk 1 closed region");
    // Area of regular hexagon of radius R=10: (3 * sqrt(3) / 2) * R^2 ~= 259.8076
    let expected_area = (3.0 * 3.0_f64.sqrt() / 2.0) * 100.0;
    assert!((regions[0].area - expected_area).abs() < 1e-3);
}

#[test]
fn test_slot_center_to_center_and_overall() {
    use crate::ops::{slot_from_points, slot_from_radius, SlotMode};

    let p1 = DVec2::new(0.0, 0.0);
    let p2 = DVec2::new(50.0, 0.0);
    let radius = 10.0; // width = 20.0

    // 1. Center-to-Center Slot
    let c2c_entities = slot_from_radius(p1, p2, radius, SlotMode::CenterToCenter, false).unwrap();
    assert_eq!(c2c_entities.len(), 4, "Slot harus terdiri dari 4 entitas (2 Line, 2 Arc)");

    let mut sketch_c2c = Sketch::default();
    for e in c2c_entities {
        sketch_c2c.entities.insert(e);
    }
    let regions_c2c = crate::region::find_closed_regions(&sketch_c2c);
    assert_eq!(regions_c2c.len(), 1, "Center-to-Center slot harus membentuk 1 closed region");
    // Expected area: rectangle (50 * 20 = 1000) + circle (pi * 10^2 = 314.159) = 1314.159
    let expected_c2c_area = 50.0 * 20.0 + std::f64::consts::PI * 100.0;
    assert!(
        (regions_c2c[0].area - expected_c2c_area).abs() < 5.0,
        "Luas slot C2C harus ~1314.16 mm² (got {})",
        regions_c2c[0].area
    );

    // 2. Overall Slot (Total Length = 50.0, so center distance = 50 - 20 = 30.0)
    let p3 = DVec2::new(25.0, 10.0); // radius 10.0
    let ov_entities = slot_from_points(p1, p2, p3, SlotMode::Overall, false).unwrap();
    assert_eq!(ov_entities.len(), 4);

    let mut sketch_ov = Sketch::default();
    for e in ov_entities {
        sketch_ov.entities.insert(e);
    }
    let regions_ov = crate::region::find_closed_regions(&sketch_ov);
    assert_eq!(regions_ov.len(), 1, "Overall slot harus membentuk 1 closed region");
    // Expected area: rectangle (30 * 20 = 600) + circle (pi * 10^2 = 314.159) = 914.159
    let expected_ov_area = 30.0 * 20.0 + std::f64::consts::PI * 100.0;
    assert!(
        (regions_ov[0].area - expected_ov_area).abs() < 5.0,
        "Luas slot Overall harus ~914.16 mm² (got {})",
        regions_ov[0].area
    );
}

#[test]
fn test_text_vectorization() {
    use crate::text::{text_to_entities, FontPreset, TextAlign, TextOptions};

    let options = TextOptions {
        font_height_mm: 15.0,
        letter_spacing: 1.0,
        line_spacing: 1.2,
        align: TextAlign::Center,
        font_preset: FontPreset::DefaultSans,
        is_construction: false,
    };

    let entities = text_to_entities("DUCAD", DVec2::new(0.0, 0.0), &options, None)
        .expect("Vektorisasi teks DUCAD harus berhasil");

    assert!(!entities.is_empty(), "Entitas teks tidak boleh kosong");
    // Huruf D, U, C, A, D masing-masing membentuk objek kurva 2D tertutup (Spline closed loops)
    assert!(entities.len() >= 5, "Jumlah loop huruf harus >= 5");

    let mut sketch = Sketch::default();
    for e in entities {
        sketch.entities.insert(e);
    }

    let regions = crate::region::find_closed_regions(&sketch);
    // DUCAD memiliki closed regions untuk D, U, C, A, D
    assert!(regions.len() >= 5, "Teks DUCAD harus menghasilkan minimal 5 closed regions (got {})", regions.len());
}


