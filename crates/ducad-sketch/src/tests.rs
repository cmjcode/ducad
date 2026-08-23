use super::*;
use glam::DVec2;

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

#[test]
fn translate_entity_shifts_all_variants() {
    let delta = DVec2::new(5.0, -2.0);
    let line = Entity::Line {
        start: DVec2::ZERO,
        end: DVec2::new(10.0, 0.0),
    };
    assert_eq!(
        translate_entity(&line, delta),
        Entity::Line {
            start: delta,
            end: DVec2::new(15.0, -2.0),
        }
    );

    let circle = Entity::Circle {
        center: DVec2::new(1.0, 1.0),
        radius: 3.0,
    };
    assert_eq!(
        translate_entity(&circle, delta),
        Entity::Circle {
            center: DVec2::new(6.0, -1.0),
            radius: 3.0,
        }
    );
}

#[test]
fn translate_entities_undo_roundtrip_preserves_id() {
    let mut sketch = Sketch::default();
    let mut undo = UndoStack::default();
    let id = sketch.entities.insert(Entity::Circle {
        center: DVec2::ZERO,
        radius: 5.0,
    });

    undo.execute(
        Box::new(TranslateEntities::new("Geser X", vec![id], DVec2::new(12.0, 0.0))),
        &mut sketch,
    );
    assert_eq!(
        sketch.entities.get(id).unwrap(),
        &Entity::Circle {
            center: DVec2::new(12.0, 0.0),
            radius: 5.0,
        }
    );

    undo.undo(&mut sketch);
    assert_eq!(
        sketch.entities.get(id).unwrap(),
        &Entity::Circle {
            center: DVec2::ZERO,
            radius: 5.0,
        }
    );
}

#[test]
fn test_entity_visibility_toggle_and_hit_test() {
    let mut sketch = Sketch::default();
    let id = sketch.entities.insert(Entity::Line {
        start: DVec2::new(0.0, 0.0),
        end: DVec2::new(10.0, 0.0),
    });

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
    let spline = Entity::Spline { points: pts.clone() };

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
    if let Entity::Spline { points } = translated {
        assert_eq!(points[0], DVec2::new(5.0, 10.0));
        assert_eq!(points[3], DVec2::new(35.0, 10.0));
    } else {
        panic!("expected Spline");
    }

    // Mirror across Y axis (axis along X=0, from (0,0) to (0,1))
    let mirrored = mirror_entity(sketch.entities.get(id).unwrap(), DVec2::ZERO, DVec2::new(0.0, 1.0)).unwrap();
    if let Entity::Spline { points } = mirrored {
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
    sketch.entities.insert(Entity::Spline {
        points: vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(10.0, 10.0),
            DVec2::new(20.0, 10.0),
            DVec2::new(30.0, 0.0),
        ],
    });
    // Line 1: (30, 0) to (30, -10)
    sketch.entities.insert(Entity::Line {
        start: DVec2::new(30.0, 0.0),
        end: DVec2::new(30.0, -10.0),
    });
    // Line 2: (30, -10) to (0, -10)
    sketch.entities.insert(Entity::Line {
        start: DVec2::new(30.0, -10.0),
        end: DVec2::new(0.0, -10.0),
    });
    // Line 3: (0, -10) to (0, 0)
    sketch.entities.insert(Entity::Line {
        start: DVec2::new(0.0, -10.0),
        end: DVec2::new(0.0, 0.0),
    });

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

    if let Entity::Line { start, end } = res.trimmed_line1 {
        assert!((start - DVec2::new(0.0, 0.0)).length() < 1e-6);
        assert!((end - DVec2::new(8.0, 0.0)).length() < 1e-6);
    } else {
        panic!("expected Line");
    }

    if let Entity::Line { start, end } = res.trimmed_line2 {
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

    if let Entity::Line { start, end } = res.bevel_line {
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
    let id1 = sketch.entities.insert(Entity::Line {
        start: DVec2::new(0.0, 0.0),
        end: DVec2::new(10.0, 0.0),
    });
    let id2 = sketch.entities.insert(Entity::Line {
        start: DVec2::new(10.0, 0.0),
        end: DVec2::new(10.0, 10.0),
    });

    let found = find_corner_lines_at_point(&sketch, DVec2::new(10.0, 0.0), 0.5);
    assert!(found.is_some());
    let (f1, f2, corner) = found.unwrap();
    assert_eq!(corner, DVec2::new(10.0, 0.0));
    assert!((f1 == id1 && f2 == id2) || (f1 == id2 && f2 == id1));
}


