use super::*;
use glam::Vec3;

#[test]
fn measurement_lines_empty_for_single_point() {
    let plane = SketchPlane::top();
    assert!(measurement_lines(&[DVec2::new(0.0, 0.0)], &plane).is_empty());
}

#[test]
fn measurement_lines_one_segment_for_two_points() {
    let plane = SketchPlane::top();
    let verts = measurement_lines(&[DVec2::new(0.0, 0.0), DVec2::new(5.0, 0.0)], &plane);
    assert_eq!(verts.len(), 2);
}

#[test]
fn measurement_lines_two_segments_for_three_points() {
    let plane = SketchPlane::top();
    let verts = measurement_lines(
        &[
            DVec2::new(0.0, 0.0),
            DVec2::new(1.0, 1.0),
            DVec2::new(2.0, 0.0),
        ],
        &plane,
    );
    assert_eq!(verts.len(), 4);
}

#[test]
fn measurement_arrowheads_empty_for_single_point() {
    let plane = SketchPlane::top();
    assert!(measurement_arrowheads(&[DVec2::new(0.0, 0.0)], &plane).is_empty());
}

#[test]
fn measurement_arrowheads_both_ends_for_two_points() {
    let plane = SketchPlane::top();
    let verts = measurement_arrowheads(&[DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0)], &plane);
    assert_eq!(verts.len(), 8);
}

#[test]
fn measurement_arrowheads_skip_shared_vertex_for_three_points() {
    let plane = SketchPlane::top();
    let verts = measurement_arrowheads(
        &[
            DVec2::new(0.0, 0.0),
            DVec2::new(1.0, 1.0),
            DVec2::new(2.0, 0.0),
        ],
        &plane,
    );
    assert_eq!(verts.len(), 8);
}

#[test]
fn measurement_arrowheads_degenerate_coincident_points_no_panic() {
    let plane = SketchPlane::top();
    let verts = measurement_arrowheads(&[DVec2::new(3.0, 3.0), DVec2::new(3.0, 3.0)], &plane);
    assert!(verts.is_empty());
}

#[test]
fn inactive_entity_lines_front_and_right_planes() {
    let mut sketch = Sketch::default();
    sketch.entities.insert(Entity::Line {
        start: DVec2::new(10.0, 20.0),
        end: DVec2::new(30.0, 40.0),
    });

    let front_plane = SketchPlane::front();
    let front_verts = inactive_entity_lines(&sketch, &front_plane);
    assert_eq!(front_verts.len(), 2);
    assert!((front_verts[0].position[0] - 10.0).abs() < 1e-4);
    assert!((front_verts[0].position[1] - (-Z_OFFSET)).abs() < 1e-4);
    assert!((front_verts[0].position[2] - 20.0).abs() < 1e-4);

    let right_plane = SketchPlane::right();
    let right_verts = inactive_entity_lines(&sketch, &right_plane);
    assert_eq!(right_verts.len(), 2);
    assert!((right_verts[0].position[0] - Z_OFFSET).abs() < 1e-4);
    assert!((right_verts[0].position[1] - 10.0).abs() < 1e-4);
    assert!((right_verts[0].position[2] - 20.0).abs() < 1e-4);
}

#[test]
fn solid_double_arrow_gizmo_mesh_produces_valid_triangle_soup() {
    let (positions, normals, colors, indices) = solid_double_arrow_gizmo_mesh(
        [0.0, 0.0, 0.0],
        22.0,
        5.0,
        [0.0, 0.78, 1.0, 1.0],
        Vec3::Z,
    );

    assert!(!positions.is_empty());
    assert_eq!(positions.len(), normals.len());
    assert_eq!(positions.len(), colors.len());
    assert_eq!(indices.len(), positions.len());
    assert_eq!(indices.len() % 3, 0);

    for idx in &indices {
        assert!((*idx as usize) < positions.len());
    }
    for p in &positions {
        assert!(p.iter().all(|v| v.is_finite()));
    }
    for n in &normals {
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        assert!((len - 1.0).abs() < 1e-3, "normal length = {len}");
    }
    for c in &colors {
        assert_eq!(*c, [0.0, 0.78, 1.0, 1.0]);
    }
}

#[test]
fn solid_double_arrow_gizmo_mesh_empty_for_zero_normal() {
    let (positions, normals, colors, indices) = solid_double_arrow_gizmo_mesh(
        [0.0, 0.0, 0.0],
        22.0,
        5.0,
        [0.0, 0.78, 1.0, 1.0],
        Vec3::ZERO,
    );
    assert!(positions.is_empty());
    assert!(normals.is_empty());
    assert!(colors.is_empty());
    assert!(indices.is_empty());
}

#[test]
fn solid_double_arrow_gizmo_mesh_scales_with_arrow_size() {
    let (small, ..) = solid_double_arrow_gizmo_mesh(
        [0.0, 0.0, 0.0],
        4.0,
        1.0,
        [1.0, 1.0, 1.0, 1.0],
        Vec3::Z,
    );
    let (big, ..) = solid_double_arrow_gizmo_mesh(
        [0.0, 0.0, 0.0],
        40.0,
        10.0,
        [1.0, 1.0, 1.0, 1.0],
        Vec3::Z,
    );

    let max_radial = |verts: &[[f32; 3]]| -> f32 {
        verts
            .iter()
            .map(|p| (p[0] * p[0] + p[1] * p[1]).sqrt())
            .fold(0.0, f32::max)
    };
    assert!(max_radial(&big) > max_radial(&small) * 5.0);
}
