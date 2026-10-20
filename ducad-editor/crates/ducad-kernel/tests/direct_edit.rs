//! Tes kontrak alat direct editing Fase 2 (PLAN_PARITAS_SOLIDCRAFT §2).

use ducad_kernel::*;
use glam::DVec3;

fn cube() -> KernelShape {
    make_box(10.0, 10.0, 10.0, false).unwrap()
}

/// Indeks face planar yang normalnya searah `n`.
fn face_toward(shape: &KernelShape, n: [f64; 3]) -> usize {
    enumerate_faces(shape)
        .iter()
        .find(|f| f.kind == SurfaceKind::Plane && DVec3::from(f.normal).dot(DVec3::from(n)) > 0.99)
        .map(|f| f.index)
        .unwrap()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= b.abs() * 2e-3 + 1e-6
}

#[test]
fn offset_top_face_grows_box() {
    let c = cube();
    let top = face_toward(&c, [0.0, 0.0, 1.0]);
    let out = offset_faces_by_index(&c, &[top], 5.0).unwrap();
    assert!(close(out.volume(), 1500.0), "volume {}", out.volume());
    let out = offset_faces_by_index(&c, &[top], -4.0).unwrap();
    assert!(close(out.volume(), 600.0), "volume {}", out.volume());
}

#[test]
fn offset_cylinder_face_changes_radius() {
    let c = make_cylinder(5.0, 10.0).unwrap();
    let side = enumerate_faces(&c)
        .iter()
        .find(|f| f.kind == SurfaceKind::Cylinder)
        .map(|f| f.index)
        .unwrap();
    let out = offset_faces_by_index(&c, &[side], 1.0).unwrap();
    let expect = std::f64::consts::PI * 36.0 * 10.0;
    assert!(close(out.volume(), expect), "volume {} vs {expect}", out.volume());
}

#[test]
fn offset_rejects_bad_input() {
    let c = cube();
    assert!(offset_faces_by_index(&c, &[], 1.0).is_err());
    assert!(offset_faces_by_index(&c, &[0], 0.0).is_err());
    assert!(offset_faces_by_index(&c, &[0], f64::NAN).is_err());
    assert!(offset_faces_by_index(&c, &[999], 1.0).is_err());
}

#[test]
fn delete_fillet_face_restores_corner() {
    let c = cube();
    let rounded = fillet_all(&c, 1.0).unwrap();
    assert!(rounded.volume() < 1000.0);
    let fillets: Vec<usize> = enumerate_faces(&rounded)
        .iter()
        .filter(|f| f.kind != SurfaceKind::Plane)
        .map(|f| f.index)
        .collect();
    assert!(!fillets.is_empty());
    let out = delete_faces_by_index(&rounded, &fillets).unwrap();
    assert!(close(out.volume(), 1000.0), "volume {}", out.volume());
    assert!(delete_faces_by_index(&c, &[]).is_err());
}

#[test]
fn replace_face_with_parallel_and_tilted_plane() {
    let c = cube();
    let top = face_toward(&c, [0.0, 0.0, 1.0]);
    let out = replace_face_with_plane(&c, top, DVec3::new(0.0, 0.0, 15.0), DVec3::Z).unwrap();
    assert!(close(out.volume(), 1500.0), "naik: {}", out.volume());
    let out = replace_face_with_plane(&c, top, DVec3::new(0.0, 0.0, 6.0), DVec3::Z).unwrap();
    assert!(close(out.volume(), 600.0), "turun: {}", out.volume());
    // Bidang miring lewat (0,0,10) dan (10,0,5): rata-rata tinggi 7,5.
    let n = DVec3::new(0.5, 0.0, 1.0).normalize();
    let out = replace_face_with_plane(&c, top, DVec3::new(0.0, 0.0, 10.0), n).unwrap();
    assert!(close(out.volume(), 750.0), "miring: {}", out.volume());
    // Bidang tegak lurus face ditolak.
    assert!(replace_face_with_plane(&c, top, DVec3::ZERO, DVec3::X).is_err());
}

#[test]
fn split_faces_keeps_solid() {
    let c = cube();
    let before = enumerate_faces(&c).len();
    let out = split_faces_by_plane(&c, &[], DVec3::new(5.0, 0.0, 0.0), DVec3::X).unwrap();
    assert!(enumerate_faces(&out).len() > before);
    assert!(close(out.volume(), 1000.0));
    let top = face_toward(&c, [0.0, 0.0, 1.0]);
    let chain = vec![ProfileSegment::Line { start: (5.0, -1.0), end: (5.0, 11.0) }];
    let out = split_faces_by_curves(
        &c,
        &[top],
        [0.0, 0.0, 10.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        &[chain],
    )
    .unwrap();
    assert_eq!(enumerate_faces(&out).len(), before + 1);
}

#[test]
fn point_inside_box() {
    let c = cube();
    assert!(point_inside(&c, DVec3::splat(5.0)));
    assert!(!point_inside(&c, DVec3::splat(15.0)));
    assert!(!point_inside(&c, DVec3::new(5.0, 5.0, -1.0)));
}

#[test]
fn boundary_fill_overlap_and_cavity() {
    let a = cube();
    let b = translate_shape(&cube(), 5.0, 0.0, 0.0).unwrap();
    let out = boundary_fill(&[&a, &b], &[DVec3::new(7.0, 5.0, 5.0)]).unwrap();
    assert!(close(out.volume(), 500.0), "irisan: {}", out.volume());
    let out = boundary_fill(&[&a, &b], &[DVec3::new(2.0, 5.0, 5.0)]).unwrap();
    assert!(close(out.volume(), 500.0), "sisa a: {}", out.volume());
    // Rongga tertutup di dalam kotak berongga.
    let inner = translate_shape(&make_box(4.0, 4.0, 4.0, false).unwrap(), 3.0, 3.0, 3.0).unwrap();
    let hollow = subtract(&a, &inner).unwrap();
    let out = boundary_fill(&[&hollow], &[DVec3::splat(5.0)]).unwrap();
    assert!(close(out.volume(), 64.0), "rongga: {}", out.volume());
    // Titik di ruang bebas terbuka → error.
    assert!(boundary_fill(&[&a], &[DVec3::splat(50.0)]).is_err());
    assert!(boundary_fill(&[], &[DVec3::ZERO]).is_err());
}

#[test]
fn bounding_solids_wrap_shape() {
    let s = make_sphere(5.0).unwrap();
    let bx = bounding_box_solid(&s, 1.0).unwrap();
    assert!(close(bx.volume(), 12.0f64.powi(3)), "box {}", bx.volume());
    let cy = bounding_cylinder_solid(&s, DVec3::Z, 0.0).unwrap();
    let v = cy.volume();
    let ideal = std::f64::consts::PI * 25.0 * 10.0;
    assert!(v >= ideal * 0.999 && v < ideal * 1.03, "cyl {v} vs {ideal}");
    assert!(bounding_box_solid(&s, -1.0).is_err());
}

#[test]
fn torus_volume() {
    let t = make_torus(10.0, 2.0).unwrap();
    let expect = 2.0 * std::f64::consts::PI.powi(2) * 10.0 * 4.0;
    assert!(close(t.volume(), expect), "torus {} vs {expect}", t.volume());
    assert!(make_torus(2.0, 3.0).is_err());
    assert!(make_torus(f64::NAN, 1.0).is_err());
}

#[test]
fn iges_round_trip_keeps_volume() {
    let c = cube();
    let text = iges::iges_string(&[("kubus", &c)]).unwrap();
    assert!(text.contains('S') && text.len() > 100);
    let (shapes, _warn) = iges::read_iges_string(&text).unwrap();
    assert_eq!(shapes.len(), 1);
    assert!(close(shapes[0].1.volume().abs(), 1000.0), "{}", shapes[0].1.volume());
    assert!(iges::read_iges_string("bukan iges").is_err());
}

#[test]
fn continuity_classes_on_box_and_fillet() {
    let c = cube();
    let seams = seam_continuity(&c, 1e-3, 1.0, 0.01).unwrap();
    assert!(!seams.is_empty());
    assert!(seams.iter().all(|s| s.class == Continuity::G0), "kotak: semua sudut tajam");
    let r = fillet_all(&c, 2.0).unwrap();
    let seams = seam_continuity(&r, 1e-3, 1.0, 0.01).unwrap();
    assert!(seams.iter().any(|s| s.class >= Continuity::G1), "fillet: tangen");
}
