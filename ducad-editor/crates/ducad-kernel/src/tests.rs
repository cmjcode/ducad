use super::*;
use glam::{dvec3, DVec3};
use opencascade::adhoc::AdHocShape;
use std::sync::Mutex;

/// OCCT (setidaknya jalur transfer STEP yang dipakai `deep_clone`) TIDAK
/// thread-safe di binding ini — ditemukan lewat test, bukan teori: jalan
/// sendiri-sendiri semua lulus, tapi `cargo test` default (multi-thread)
/// crash `SIGABRT`/`Interface_InterfaceError` karena beberapa test
/// menyentuh working-session STEP OCCT yang sama secara bersamaan. Lock
/// global ini memaksa seluruh test modul jalan serial. Tidak mempengaruhi
/// `ducad-app` (single-threaded, kernel selalu dipanggil dari UI thread).
static TEST_LOCK: Mutex<()> = Mutex::new(());

fn lock_test() -> std::sync::MutexGuard<'static, ()> {
    TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner())
}

fn rect_profile(w: f64, h: f64) -> Profile {
    Profile::Loop(vec![
        ProfileSegment::Line {
            start: (0.0, 0.0),
            end: (w, 0.0),
        },
        ProfileSegment::Line {
            start: (w, 0.0),
            end: (w, h),
        },
        ProfileSegment::Line {
            start: (w, h),
            end: (0.0, h),
        },
        ProfileSegment::Line {
            start: (0.0, h),
            end: (0.0, 0.0),
        },
    ])
}

/// Sama seperti `rect_profile`, tapi sudut kiri-bawah di `(x0,y0)`
/// bukan `(0,0)` — dipakai test yang butuh profil TIDAK menyentuh
/// origin/axis (mis. revolve, intersect dua box tidak overlap).
fn offset_rect_profile(x0: f64, y0: f64, x1: f64, y1: f64) -> Profile {
    Profile::Loop(vec![
        ProfileSegment::Line {
            start: (x0, y0),
            end: (x1, y0),
        },
        ProfileSegment::Line {
            start: (x1, y0),
            end: (x1, y1),
        },
        ProfileSegment::Line {
            start: (x1, y1),
            end: (x0, y1),
        },
        ProfileSegment::Line {
            start: (x0, y1),
            end: (x0, y0),
        },
    ])
}

#[test]
fn extrude_rectangle_produces_mesh() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(40.0, 30.0), 20.0).unwrap();
    let mesh = shape.tessellate();
    assert!(mesh.triangle_count() > 0);
    assert!(!mesh.positions.is_empty());
}

#[test]
fn extrude_circle_produces_cylinder_mesh() {
    let _guard = lock_test();
    let profile = Profile::Circle {
        center: (0.0, 0.0),
        radius: 10.0,
    };
    let shape = extrude_profile(&profile, 15.0).unwrap();
    let mesh = shape.tessellate();
    assert!(mesh.triangle_count() > 0);
}

#[test]
fn extrude_empty_loop_errors() {
    let _guard = lock_test();
    assert!(extrude_profile(&Profile::Loop(vec![]), 10.0).is_err());
}

#[test]
fn extrude_zero_distance_errors() {
    let _guard = lock_test();
    assert!(extrude_profile(&rect_profile(10.0, 10.0), 0.0).is_err());
}

#[test]
fn union_and_subtract_produce_valid_mesh() {
    let _guard = lock_test();
    let a = extrude_profile(&rect_profile(40.0, 40.0), 10.0).unwrap();
    let b = extrude_profile(&rect_profile(20.0, 20.0), 10.0).unwrap();
    let unioned = union(&a, &b).unwrap();
    assert!(unioned.tessellate().triangle_count() > 0);
    let subtracted = subtract(&a, &b).unwrap();
    assert!(subtracted.tessellate().triangle_count() > 0);
}

#[test]
fn fillet_all_and_chamfer_all_smoke() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let filleted = fillet_all(&shape, 2.0).unwrap();
    assert!(filleted.tessellate().triangle_count() > 0);
    // Deep-clone di dalam fillet_all/chamfer_all TIDAK memutasi `shape`
    // asli — shape asli harus masih valid & bisa dipakai lagi setelah.
    let chamfered = chamfer_all(&shape, 2.0).unwrap();
    assert!(chamfered.tessellate().triangle_count() > 0);
    assert!(shape.tessellate().triangle_count() > 0);
}

#[test]
fn translate_shape_shifts_bounding_box_by_delta_without_mutating_original() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(20.0, 10.0), 5.0).unwrap();
    let original_mesh = shape.tessellate();
    let moved = translate_shape(&shape, 15.0, -5.0, 2.0).unwrap();
    let moved_mesh = moved.tessellate();
    assert_eq!(original_mesh.positions.len(), moved_mesh.positions.len());

    fn bbox_min(mesh: &KernelMesh) -> [f32; 3] {
        let mut min = [f32::MAX; 3];
        for p in &mesh.positions {
            for i in 0..3 {
                min[i] = min[i].min(p[i]);
            }
        }
        min
    }

    let orig_min = bbox_min(&original_mesh);
    let moved_min = bbox_min(&moved_mesh);
    assert!((moved_min[0] - orig_min[0] - 15.0).abs() < 1e-3);
    assert!((moved_min[1] - orig_min[1] + 5.0).abs() < 1e-3);
    assert!((moved_min[2] - orig_min[2] - 2.0).abs() < 1e-3);

    // Fungsional: `shape` asli tidak ikut bergeser.
    let orig_after = bbox_min(&shape.tessellate());
    assert_eq!(orig_after, orig_min);
}

#[test]
fn scale_shape_grows_bounding_box_uniformly_without_mutating_original() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(20.0, 10.0), 5.0).unwrap();
    let original_mesh = shape.tessellate();

    fn bbox(mesh: &KernelMesh) -> ([f32; 3], [f32; 3]) {
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for p in &mesh.positions {
            for i in 0..3 {
                min[i] = min[i].min(p[i]);
                max[i] = max[i].max(p[i]);
            }
        }
        (min, max)
    }

    let (orig_min, orig_max) = bbox(&original_mesh);
    // Scale 2x mengelilingi origin (0,0,0) — sudut bbox yg nempel origin (0,0,0) itu sendiri.
    let scaled = scale_shape(&shape, (0.0, 0.0, 0.0), 2.0).unwrap();
    let (scaled_min, scaled_max) = bbox(&scaled.tessellate());

    for i in 0..3 {
        assert!((scaled_max[i] - orig_max[i] * 2.0).abs() < 1e-2, "axis {i}");
        assert!((scaled_min[i] - orig_min[i] * 2.0).abs() < 1e-2, "axis {i}");
    }

    // Fungsional: `shape` asli tidak ikut ter-scale.
    let (orig_after_min, _) = bbox(&shape.tessellate());
    assert_eq!(orig_after_min, orig_min);
}

#[test]
fn shell_hollow_smoke() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 30.0), 20.0).unwrap();
    let hollowed = shell_hollow(&shape, 2.0, Direction::PosZ).unwrap();
    assert!(hollowed.tessellate().triangle_count() > 0);
}

#[test]
fn deep_clone_preserves_mesh_vertex_count() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(25.0, 15.0), 10.0).unwrap();
    let cloned = crate::shape::deep_clone(shape.inner()).unwrap();
    let original_mesh = shape.tessellate();
    let cloned_mesh = crate::mesh::tessellate_shape(&cloned);
    assert_eq!(original_mesh.positions.len(), cloned_mesh.positions.len());
}

#[test]
fn clone_shape_independent_of_original() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(25.0, 15.0), 10.0).unwrap();
    let snapshot = clone_shape(&shape).unwrap();
    // Fillet hasil clone TIDAK boleh menyentuh snapshot maupun shape
    // asli — inti pemakaian `clone_shape` sbg base rounding parametrik.
    let filleted = fillet_all(&snapshot, 2.0).unwrap();
    assert!(filleted.tessellate().triangle_count() > 0);
    assert_eq!(
        shape.tessellate().positions.len(),
        snapshot.tessellate().positions.len()
    );
}

#[test]
fn make_filleted_box_smoke() {
    let _guard = lock_test();
    let shape = make_filleted_box(40.0, 30.0, 20.0, 3.0).unwrap();
    let mesh = shape.tessellate();
    assert!(mesh.triangle_count() > 0);
}

#[test]
fn step_string_roundtrip_preserves_mesh_vertex_count() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(25.0, 15.0), 10.0).unwrap();
    let step = shape.to_step_string().unwrap();
    assert!(step.contains("ISO-10303"), "STEP harus AP214 ISO-10303");
    let restored = KernelShape::from_step_string(&step).unwrap();
    assert_eq!(shape.tessellate().positions.len(), restored.tessellate().positions.len());
}

#[test]
fn read_step_roundtrips_write_step() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(10.0, 10.0), 5.0).unwrap();
    let path = std::env::temp_dir().join(format!("ducad-test-read-step-{}.step", std::process::id()));
    shape.write_step(&path).unwrap();
    let restored = KernelShape::read_step(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(shape.tessellate().positions.len(), restored.tessellate().positions.len());
}

#[test]
fn write_step_compound_combines_two_bodies() {
    let _guard = lock_test();
    let a = extrude_profile(&rect_profile(10.0, 10.0), 5.0).unwrap();
    let b = extrude_profile(&rect_profile(20.0, 20.0), 5.0).unwrap();
    let path = std::env::temp_dir().join(format!("ducad-test-compound-{}.step", std::process::id()));
    write_step_compound(&[&a, &b], &path).unwrap();
    let restored = KernelShape::read_step(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    // Compound gabungan dua box terpisah harus punya lebih banyak
    // vertex dari salah satu box sendirian (bukti keduanya masuk).
    assert!(restored.tessellate().positions.len() > a.tessellate().positions.len());
}

#[test]
fn write_step_compound_empty_errors() {
    let _guard = lock_test();
    let path = std::env::temp_dir().join("ducad-test-compound-empty.step");
    assert!(write_step_compound(&[], &path).is_err());
}

#[test]
fn kernel_mesh_merge_shifts_indices() {
    let a = KernelMesh {
        positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        indices: vec![0, 1, 2],
        face_ranges: Vec::new(),
    };
    let b = KernelMesh {
        positions: vec![[2.0, 0.0, 0.0], [3.0, 0.0, 0.0], [2.0, 1.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        indices: vec![0, 1, 2],
        face_ranges: Vec::new(),
    };
    let merged = KernelMesh::merge(&[&a, &b]);
    assert_eq!(merged.positions.len(), 6);
    assert_eq!(merged.indices, vec![0, 1, 2, 3, 4, 5]);
}

// ---- Fase 8: Revolve ----

#[test]
fn revolve_profile_produces_ring_solid() {
    let _guard = lock_test();
    let profile = offset_rect_profile(10.0, 0.0, 20.0, 5.0);
    let shape = revolve_profile(&profile, (0.0, 0.0), (0.0, 1.0), None).unwrap();
    let mesh = shape.tessellate();
    assert!(mesh.triangle_count() > 0);
    let mut max_radius: f32 = 0.0;
    let mut min_radius: f32 = f32::MAX;
    let mut min_y: f32 = f32::MAX;
    let mut max_y: f32 = f32::MIN;
    for p in &mesh.positions {
        let radius = (p[0] * p[0] + p[2] * p[2]).sqrt();
        max_radius = max_radius.max(radius);
        min_radius = min_radius.min(radius);
        min_y = min_y.min(p[1]);
        max_y = max_y.max(p[1]);
    }
    assert!(min_radius > 5.0, "radius dalam {min_radius} seharusnya mendekati 10 (profil tidak menyentuh axis)");
    assert!(max_radius > 15.0 && max_radius < 25.0, "radius luar {max_radius} seharusnya mendekati 20");
    assert!(min_y >= -0.5 && max_y <= 5.5, "tinggi hasil harus dalam rentang y profil asli [0,5], dapat [{min_y},{max_y}]");
}

#[test]
fn revolve_profile_degenerate_axis_errors() {
    let _guard = lock_test();
    let profile = offset_rect_profile(10.0, 0.0, 20.0, 5.0);
    assert!(revolve_profile(&profile, (0.0, 0.0), (0.0, 0.0), None).is_err());
}

#[test]
fn revolve_profile_axis_crossing_profile_returns_err_safely_without_abort() {
    let _guard = lock_test();
    // Profil membentang dari X=10 sampai X=20, Y=0 sampai Y=5
    let profile = offset_rect_profile(10.0, 0.0, 20.0, 5.0);
    // Sumbu X=15 membelah tengah persegi panjang -> memicu self-intersection di OCCT
    let result = revolve_profile(&profile, (15.0, 0.0), (0.0, 1.0), None);
    assert!(result.is_err(), "Revolve dengan sumbu membelah profil harus return Err, bukan abort/crash!");
}

#[test]
fn revolve_profile_partial_angle_succeeds() {
    let _guard = lock_test();
    let profile = offset_rect_profile(10.0, 0.0, 20.0, 5.0);
    let shape_180 = revolve_profile(&profile, (0.0, 0.0), (0.0, 1.0), Some(180.0)).unwrap();
    let mesh = shape_180.tessellate();
    assert!(mesh.triangle_count() > 0);
}


// ---- Fase 8: Loft ----

#[test]
fn loft_between_rectangles_spans_requested_height() {
    let _guard = lock_test();
    let bottom = rect_profile(20.0, 20.0);
    let top = rect_profile(10.0, 10.0);
    let shape = loft_profiles(&bottom, &top, 15.0).unwrap();
    let mesh = shape.tessellate();
    assert!(mesh.triangle_count() > 0);
    let mut min_z: f32 = f32::MAX;
    let mut max_z: f32 = f32::MIN;
    for p in &mesh.positions {
        min_z = min_z.min(p[2]);
        max_z = max_z.max(p[2]);
    }
    assert!((-0.5..=0.5).contains(&min_z), "dasar loft harus di z=0, dapat {min_z}");
    assert!((14.5..=15.5).contains(&max_z), "puncak loft harus di z=15, dapat {max_z}");
}

#[test]
fn loft_zero_height_errors() {
    let _guard = lock_test();
    let bottom = rect_profile(20.0, 20.0);
    let top = rect_profile(10.0, 10.0);
    assert!(loft_profiles(&bottom, &top, 0.0).is_err());
}

// ---- Fase 8: Boolean intersect ----

#[test]
fn intersect_overlapping_boxes_smaller_than_union() {
    let _guard = lock_test();
    let a = extrude_profile(&rect_profile(40.0, 40.0), 10.0).unwrap();
    let b = extrude_profile(&offset_rect_profile(20.0, 20.0, 60.0, 60.0), 10.0).unwrap();
    let intersected = intersect(&a, &b).unwrap();
    let unioned = union(&a, &b).unwrap();
    assert!(intersected.tessellate().positions.len() < unioned.tessellate().positions.len());
    assert!(intersected.tessellate().triangle_count() > 0);
}

#[test]
fn intersect_non_overlapping_boxes_errors() {
    let _guard = lock_test();
    let a = extrude_profile(&rect_profile(10.0, 10.0), 10.0).unwrap();
    let b = extrude_profile(&offset_rect_profile(100.0, 100.0, 110.0, 110.0), 10.0).unwrap();
    assert!(intersect(&a, &b).is_err());
}

// ---- Fase 8: Picking 3D (edge/face) ----

#[test]
fn pick_face_consistent_across_deep_clone() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (15.0, 10.0, 100.0),
        dir: (0.0, 0.0, -1.0),
    };
    let hit_original = pick_face(&shape, ray).expect("harus kena face top shape asli");
    let cloned = crate::shape::deep_clone(shape.inner()).unwrap();
    let hit_cloned = crate::picking::face::resolve_face_along_ray(&cloned, ray)
        .map(|(_, p)| (p.x, p.y, p.z))
        .expect("harus kena face top shape hasil deep_clone");
    assert!((hit_original.0 - hit_cloned.0).abs() < 1e-6);
    assert!((hit_original.1 - hit_cloned.1).abs() < 1e-6);
    assert!((hit_original.2 - hit_cloned.2).abs() < 1e-6);
}

#[test]
fn pick_edge_consistent_across_deep_clone() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (-5.0, -5.0, 7.5),
        dir: (1.0, 1.0, 0.0),
    };
    let tolerance = 1.0;
    let (hit_original, _) = pick_edge(&shape, ray, tolerance).expect("harus kena rusuk shape asli");
    let cloned = crate::shape::deep_clone(shape.inner()).unwrap();
    let (_, hit_cloned, _) =
        crate::picking::edge::resolve_edge_along_ray(&cloned, ray, tolerance).expect("harus kena rusuk shape hasil deep_clone");
    assert!((hit_original.0 - hit_cloned.x).abs() < 1e-3);
    assert!((hit_original.1 - hit_cloned.y).abs() < 1e-3);
    assert!((hit_original.2 - hit_cloned.z).abs() < 1e-3);
}

#[test]
fn edge_dimensions_reports_all_box_edges() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let dims = edge_dimensions(&shape);
    assert_eq!(dims.len(), 12, "box punya 12 rusuk topologi");

    let mut lengths: Vec<f64> = dims.iter().map(|(_, _, _, len)| *len).collect();
    lengths.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let expected: [f64; 12] = [15.0, 15.0, 15.0, 15.0, 20.0, 20.0, 20.0, 20.0, 30.0, 30.0, 30.0, 30.0];
    for (got, want) in lengths.iter().zip(expected.iter()) {
        assert!((got - want).abs() < 1e-3, "panjang rusuk {} tidak cocok dgn {}", got, want);
    }

    for ((mx, my, mz), start, end, length) in &dims {
        assert!((-1e-3..=30.0 + 1e-3).contains(mx));
        assert!((-1e-3..=20.0 + 1e-3).contains(my));
        assert!((-1e-3..=15.0 + 1e-3).contains(mz));

        let chord = ((end.0 - start.0).powi(2) + (end.1 - start.1).powi(2) + (end.2 - start.2).powi(2)).sqrt();
        assert!((chord - length).abs() < 1e-3, "korda {} vs panjang {} beda jauh utk rusuk lurus", chord, length);
        assert!((mx - (start.0 + end.0) * 0.5).abs() < 1e-3);
        assert!((my - (start.1 + end.1) * 0.5).abs() < 1e-3);
        assert!((mz - (start.2 + end.2) * 0.5).abs() < 1e-3);
    }
}

#[test]
fn pick_face_miss_returns_none() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (1000.0, 1000.0, 1000.0),
        dir: (0.0, 0.0, 1.0),
    };
    assert!(pick_face(&shape, ray).is_none());
}

// ---- Vertex Fillet Gizmo: picking vertex (sudut) 3D ----

#[test]
fn pick_vertex_on_box_corner() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (-5.0, -5.0, -5.0),
        dir: (1.0, 1.0, 1.0),
    };
    let hit = pick_vertex(&shape, ray, 1.0).expect("harus kena sudut box di (0,0,0)");
    assert!(hit.0.abs() < 1e-3);
    assert!(hit.1.abs() < 1e-3);
    assert!(hit.2.abs() < 1e-3);
}

#[test]
fn pick_vertex_consistent_across_deep_clone() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (-5.0, -5.0, -5.0),
        dir: (1.0, 1.0, 1.0),
    };
    let tolerance = 1.0;
    let hit_original = pick_vertex(&shape, ray, tolerance).expect("harus kena sudut shape asli");
    let cloned = crate::shape::deep_clone(shape.inner()).unwrap();
    let hit_cloned = crate::picking::vertex::resolve_vertex_along_ray(&cloned, ray, tolerance)
        .map(|p| (p.x, p.y, p.z))
        .expect("harus kena sudut shape hasil deep_clone");
    assert!((hit_original.0 - hit_cloned.0).abs() < 1e-6);
    assert!((hit_original.1 - hit_cloned.1).abs() < 1e-6);
    assert!((hit_original.2 - hit_cloned.2).abs() < 1e-6);
}

// ---- Fase 8: Fillet/Chamfer per-tepi, Shell multi-face ----

#[test]
fn fillet_edges_affects_only_picked_edge() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (-5.0, -5.0, 7.5),
        dir: (1.0, 1.0, 0.0),
    };
    let filleted_one = fillet_edges(&shape, 2.0, &[ray], 1.0).unwrap();
    let filleted_all = fillet_all(&shape, 2.0).unwrap();
    let original_verts = shape.tessellate().positions.len();
    let one_verts = filleted_one.tessellate().positions.len();
    let all_verts = filleted_all.tessellate().positions.len();
    assert!(one_verts > original_verts, "fillet 1 tepi harus mengubah mesh (tambah vertex bulat)");
    assert!(
        one_verts < all_verts,
        "fillet 1 tepi HARUS lebih sedikit vertex baru dibanding fillet SEMUA 12 tepi box — bukti hanya 1 tepi yang kena"
    );
}

#[test]
fn fillet_edges_empty_rays_errors() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    assert!(fillet_edges(&shape, 2.0, &[], 1.0).is_err());
}

#[test]
fn fillet_edges_no_match_errors() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (1000.0, 1000.0, 1000.0),
        dir: (0.0, 0.0, 1.0),
    };
    assert!(fillet_edges(&shape, 2.0, &[ray], 1.0).is_err());
}

#[test]
fn fillet_edges_variable_success() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (-5.0, -5.0, 7.5),
        dir: (1.0, 1.0, 0.0),
    };
    let filleted_var = fillet_edges_variable(&shape, 1.0, 4.0, &[ray], 1.0).unwrap();
    let original_verts = shape.tessellate().positions.len();
    let var_verts = filleted_var.tessellate().positions.len();
    assert!(var_verts > original_verts, "variable radius fillet harus memodifikasi mesh tepi");
}

#[test]
fn fillet_edges_variable_validation_errors() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (-5.0, -5.0, 7.5),
        dir: (1.0, 1.0, 0.0),
    };
    assert!(fillet_edges_variable(&shape, 0.0, 4.0, &[ray], 1.0).is_err());
    assert!(fillet_edges_variable(&shape, 2.0, -1.0, &[ray], 1.0).is_err());
    assert!(fillet_edges_variable(&shape, 2.0, 4.0, &[], 1.0).is_err());
}

// ---- Vertex Fillet Gizmo: fillet SEMUA tepi yang bertemu di 1 sudut ----

#[test]
fn fillet_vertex_rounds_box_corner() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (-5.0, -5.0, -5.0),
        dir: (1.0, 1.0, 1.0),
    };
    let base_volume = shape.inner().volume();
    let filleted = fillet_vertex(&shape, 2.0, ray, 1.0).unwrap();
    assert!(
        filleted.inner().volume() < base_volume,
        "membulatkan sudut harus memotong material (volume berkurang)"
    );
    assert!(filleted.tessellate().triangle_count() > 0);
    assert!((shape.inner().volume() - base_volume).abs() < 1e-6);
}

#[test]
fn fillet_vertex_zero_radius_errors() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (-5.0, -5.0, -5.0),
        dir: (1.0, 1.0, 1.0),
    };
    assert!(fillet_vertex(&shape, 0.0, ray, 1.0).is_err());
}

#[test]
fn fillet_vertex_no_match_errors() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (1000.0, 1000.0, 1000.0),
        dir: (0.0, 0.0, 1.0),
    };
    assert!(fillet_vertex(&shape, 2.0, ray, 1.0).is_err());
}

#[test]
fn fillet_edges_oversized_radius_errors_not_crashes() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (-5.0, -5.0, 7.5),
        dir: (1.0, 1.0, 0.0),
    };
    let result = fillet_edges(&shape, 1000.0, &[ray], 1.0);
    assert!(result.is_err(), "radius jauh melebihi ukuran box harus ditolak sbg Err, bukan sukses/crash");
}

#[test]
fn fillet_vertex_oversized_radius_errors_not_crashes() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (-5.0, -5.0, -5.0),
        dir: (1.0, 1.0, 1.0),
    };
    let result = fillet_vertex(&shape, 1000.0, ray, 1.0);
    assert!(result.is_err(), "radius jauh melebihi ukuran box harus ditolak sbg Err, bukan sukses/crash");
}

// ---- Vertex Chamfer Gizmo: chamfer SEMUA tepi yang bertemu di 1 sudut ----

#[test]
fn chamfer_vertex_flattens_box_corner() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (-5.0, -5.0, -5.0),
        dir: (1.0, 1.0, 1.0),
    };
    let base_volume = shape.inner().volume();
    let chamfered = chamfer_vertex(&shape, 2.0, ray, 1.0).unwrap();
    assert!(
        chamfered.inner().volume() < base_volume,
        "memangkas sudut harus memotong material (volume berkurang)"
    );
    assert!(chamfered.tessellate().triangle_count() > 0);
    assert!((shape.inner().volume() - base_volume).abs() < 1e-6);
}

#[test]
fn chamfer_vertex_zero_distance_errors() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (-5.0, -5.0, -5.0),
        dir: (1.0, 1.0, 1.0),
    };
    assert!(chamfer_vertex(&shape, 0.0, ray, 1.0).is_err());
}

#[test]
fn chamfer_vertex_no_match_errors() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (1000.0, 1000.0, 1000.0),
        dir: (0.0, 0.0, 1.0),
    };
    assert!(chamfer_vertex(&shape, 2.0, ray, 1.0).is_err());
}

#[test]
fn chamfer_vertex_oversized_distance_errors_not_crashes() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (-5.0, -5.0, -5.0),
        dir: (1.0, 1.0, 1.0),
    };
    let result = chamfer_vertex(&shape, 1000.0, ray, 1.0);
    assert!(result.is_err(), "jarak jauh melebihi ukuran box harus ditolak sbg Err, bukan sukses/crash");
}

#[test]
fn chamfer_edges_oversized_distance_errors_not_crashes() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (-5.0, -5.0, 7.5),
        dir: (1.0, 1.0, 0.0),
    };
    let result = chamfer_edges(&shape, 1000.0, &[ray], 1.0);
    assert!(result.is_err(), "jarak chamfer jauh melebihi ukuran box harus ditolak sbg Err, bukan sukses/crash");
}

#[test]
fn fillet_vertex_radius_near_shortest_edge_succeeds_without_manual_precheck() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (-5.0, -5.0, -5.0),
        dir: (1.0, 1.0, 1.0),
    };
    let result = fillet_vertex(&shape, 14.0, ray, 1.0);
    assert!(
        result.is_ok(),
        "radius 14mm mendekati tepi terpendek (15mm) harus tetap sukses: {}",
        result.as_ref().err().map(|e| e.to_string()).unwrap_or_default()
    );
}

#[test]
fn fillet_edges_radius_near_shortest_touching_edge_succeeds_without_manual_precheck() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (-5.0, -5.0, 7.5),
        dir: (1.0, 1.0, 0.0),
    };
    let result = fillet_edges(&shape, 14.0, &[ray], 1.0);
    assert!(
        result.is_ok(),
        "radius 14mm mendekati tepi terpendek (15mm) harus tetap sukses: {}",
        result.as_ref().err().map(|e| e.to_string()).unwrap_or_default()
    );
}

#[test]
fn chamfer_edges_smoke() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (-5.0, -5.0, 7.5),
        dir: (1.0, 1.0, 0.0),
    };
    let chamfered = chamfer_edges(&shape, 2.0, &[ray], 1.0).unwrap();
    assert!(chamfered.tessellate().triangle_count() > 0);
    assert!(shape.tessellate().triangle_count() > 0);
}

fn l_shape_solid() -> KernelShape {
    let profile = Profile::Loop(vec![
        ProfileSegment::Line { start: (0.0, 0.0), end: (30.0, 0.0) },
        ProfileSegment::Line { start: (30.0, 0.0), end: (30.0, 10.0) },
        ProfileSegment::Line { start: (30.0, 10.0), end: (10.0, 10.0) },
        ProfileSegment::Line { start: (10.0, 10.0), end: (10.0, 30.0) },
        ProfileSegment::Line { start: (10.0, 30.0), end: (0.0, 30.0) },
        ProfileSegment::Line { start: (0.0, 30.0), end: (0.0, 0.0) },
    ]);
    extrude_profile(&profile, 20.0).unwrap()
}

#[test]
fn fillet_concave_inner_edge_adds_material_and_succeeds() {
    let _guard = lock_test();
    let shape = l_shape_solid();
    let base_volume = shape.inner().volume();

    // Ray diarahkan ke rusuk dalam (x=10, y=10, z=0..20)
    let ray = PickRay {
        origin: (25.0, 25.0, 10.0),
        dir: (-1.0, -1.0, 0.0),
    };
    let filleted = fillet_edges(&shape, 2.0, &[ray], 1.0).unwrap();
    assert!(
        filleted.inner().volume() > base_volume,
        "Fillet pada sudut/rusuk dalam harus menambahkan material (volume membesar)"
    );
    assert!(filleted.tessellate().triangle_count() > 0);
}

#[test]
fn chamfer_concave_inner_edge_adds_material_and_succeeds() {
    let _guard = lock_test();
    let shape = l_shape_solid();
    let base_volume = shape.inner().volume();

    let ray = PickRay {
        origin: (25.0, 25.0, 10.0),
        dir: (-1.0, -1.0, 0.0),
    };
    let chamfered = chamfer_edges(&shape, 2.0, &[ray], 1.0).unwrap();
    assert!(
        chamfered.inner().volume() > base_volume,
        "Chamfer pada sudut/rusuk dalam harus menambahkan material bevel (volume membesar)"
    );
    assert!(chamfered.tessellate().triangle_count() > 0);
}

#[test]
fn fillet_concave_inner_vertex_succeeds() {
    let _guard = lock_test();
    let shape = l_shape_solid();

    // Vertex dalam di (10, 10, 0)
    let ray = PickRay {
        origin: (25.0, 25.0, -10.0),
        dir: (-1.0, -1.0, 0.67),
    };
    let filleted = fillet_vertex(&shape, 2.0, ray, 1.0).unwrap();
    assert!(filleted.tessellate().triangle_count() > 0);
}

#[test]
fn chamfer_concave_inner_vertex_succeeds() {
    let _guard = lock_test();
    let shape = l_shape_solid();

    let ray = PickRay {
        origin: (25.0, 25.0, -10.0),
        dir: (-1.0, -1.0, 0.67),
    };
    let chamfered = chamfer_vertex(&shape, 2.0, ray, 1.0).unwrap();
    assert!(chamfered.tessellate().triangle_count() > 0);
}

#[test]
fn shell_hollow_faces_multi_face_differs_from_single() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 30.0), 20.0).unwrap();
    let ray_top = PickRay {
        origin: (15.0, 15.0, 100.0),
        dir: (0.0, 0.0, -1.0),
    };
    let ray_bottom = PickRay {
        origin: (15.0, 15.0, -100.0),
        dir: (0.0, 0.0, 1.0),
    };
    let hollow_two = shell_hollow_faces(&shape, 2.0, &[ray_top, ray_bottom]).unwrap();
    let hollow_one = shell_hollow(&shape, 2.0, Direction::PosZ).unwrap();
    assert!(hollow_two.tessellate().triangle_count() > 0);
    assert_ne!(hollow_two.inner().faces().count(), hollow_one.inner().faces().count());
    assert_ne!(hollow_two.tessellate().triangle_count(), hollow_one.tessellate().triangle_count());
}

#[test]
fn shell_hollow_faces_empty_rays_errors() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 30.0), 20.0).unwrap();
    assert!(shell_hollow_faces(&shape, 2.0, &[]).is_err());
}

#[test]
fn shell_hollow_already_hollow_shape_returns_err_without_crashing() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 30.0), 20.0).unwrap();
    // Memilih ray yang meleset dari shape akan mengembalikan Err rapi tanpa panic/crash
    let ray_miss = PickRay {
        origin: (1000.0, 1000.0, 1000.0),
        dir: (0.0, 0.0, -1.0),
    };
    let res = shell_hollow_faces(&shape, 2.0, &[ray_miss]);
    assert!(res.is_err());
}

#[test]
fn extrude_vertical_front_xz_produces_solid() {
    let _guard = lock_test();
    let shape = extrude_profile_on_plane(
        &rect_profile(30.0, 20.0),
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, -1.0, 0.0],
        15.0,
    )
    .unwrap();
    let mesh = shape.tessellate();
    assert!(mesh.triangle_count() > 0);
    assert!(!mesh.positions.is_empty());
}

#[test]
fn extrude_vertical_right_yz_produces_solid() {
    let _guard = lock_test();
    let shape = extrude_profile_on_plane(
        &rect_profile(25.0, 35.0),
        [0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
        10.0,
    )
    .unwrap();
    let mesh = shape.tessellate();
    assert!(mesh.triangle_count() > 0);
    assert!(!mesh.positions.is_empty());
}

#[test]
fn test_pick_face_huge_magnitude_direction_matches_unit_direction() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(200.0, 200.0), 20.0).unwrap();

    let unit_dir = (0.0_f64, 0.0, -1.0);
    let huge_dir = (0.0_f64, 0.0, -20000.0);

    let ray_unit = PickRay { origin: (100.0, 100.0, 500.0), dir: unit_dir };
    let ray_huge = PickRay { origin: (100.0, 100.0, 500.0), dir: huge_dir };

    let hit_unit = pick_face_details(&shape, ray_unit);
    let hit_huge = pick_face_details(&shape, ray_huge);

    assert!(hit_unit.is_some(), "ray unit-length harus kena top face");
    assert!(hit_huge.is_some(), "ray magnitude besar (20000x) HARUS tetap kena face yang sama");
}

#[test]
fn test_pick_face_real_world_oblique_ray_reproduction() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(194.468, 77.195), 51.933).unwrap();

    let ray = PickRay {
        origin: (152.94723510742188 + 120.0, -152.9267120361328 + 20.0, 124.88241577148438),
        dir: (-14566.6611328125, 16616.1640625, -11743.1689453125),
    };
    let hit = pick_face_details(&shape, ray);
    assert!(hit.is_some(), "ray nyata menembus box harus kena");
}

#[test]
fn test_pick_face_same_oblique_direction_unit_length_isolation() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(194.468, 77.195), 51.933).unwrap();

    let ray = PickRay {
        origin: (152.94723510742188 + 120.0, -152.9267120361328 + 20.0, 124.88241577148438),
        dir: (-0.5821141452051053, 0.664016554764354, -0.4692815114097371),
    };
    let hit = pick_face_details(&shape, ray);
    assert!(hit.is_some(), "arah oblique SAMA tapi unit-length harus tetap kena");
}

#[test]
fn test_pick_face_simple_clean_oblique_ray_baseline() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(100.0, 100.0), 100.0).unwrap();

    let origin = (300.0_f64, 150.0, 250.0);
    let target = (50.0_f64, 60.0, 90.0);
    let dir = (target.0 - origin.0, target.1 - origin.1, target.2 - origin.2);

    let ray = PickRay { origin, dir };
    let hit = pick_face_details(&shape, ray);
    assert!(hit.is_some(), "ray oblique asimetris menuju tengah wajah atas HARUS kena");
    if let Some(h) = hit {
        assert!((h.hit_point.2 - 100.0).abs() < 1e-3, "harus kena wajah Z=100 (atas), bukan wajah lain");
    }
}

#[test]
fn test_pick_face_min_bound_face_same_box_dims_as_real_case() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(194.468, 77.195), 51.933).unwrap();

    let ray = PickRay { origin: (100.0, -100.0, 25.0), dir: (20.0, 130.0, 3.0) };
    let hit = pick_face_details(&shape, ray);
    assert!(hit.is_some(), "ray bersih menuju wajah Y=min box dimensi real HARUS kena");
}

#[test]
fn test_pick_face_max_bound_side_face_oblique() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(194.468, 77.195), 51.933).unwrap();

    let ray = PickRay { origin: (300.0, 20.0, 25.0), dir: (-150.0, 20.0, 5.0) };
    let hit = pick_face_details(&shape, ray);
    assert!(hit.is_some(), "ray bersih menuju wajah X=max HARUS kena");
}

#[test]
fn test_pick_face_cap_face_real_box_dims_isolation() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(194.468, 77.195), 51.933).unwrap();

    let ray = PickRay { origin: (100.0, 30.0, 300.0), dir: (20.0, 5.0, -255.0) };
    let hit = pick_face_details(&shape, ray);
    assert!(hit.is_some(), "ray oblique ke cap face Z=max HARUS kena");
}

#[test]
fn test_pick_face_details_and_extrude_box_faces() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();

    let ray_top = PickRay {
        origin: (15.0, 10.0, 100.0),
        dir: (0.0, 0.0, -1.0),
    };
    let hit_top = pick_face_details(&shape, ray_top).expect("harus kena top face");
    assert!((hit_top.normal.0 - 0.0).abs() < 1e-5);
    assert!((hit_top.normal.1 - 0.0).abs() < 1e-5);
    assert!((hit_top.normal.2 - 1.0).abs() < 1e-5);
    assert!((hit_top.centroid.2 - 15.0).abs() < 1e-5);

    let extruded_top = extrude_face(&shape, ray_top, 10.0).expect("extrude top face berhasil");
    assert!(extruded_top.tessellate().triangle_count() > 0);

    let ray_right = PickRay {
        origin: (100.0, 10.0, 7.5),
        dir: (-1.0, 0.0, 0.0),
    };
    let hit_right = pick_face_details(&shape, ray_right).expect("harus kena side face");
    assert!((hit_right.normal.0 - 1.0).abs() < 1e-5);
    assert!((hit_right.normal.1 - 0.0).abs() < 1e-5);
    assert!((hit_right.normal.2 - 0.0).abs() < 1e-5);

    let extruded_right = extrude_face(&shape, ray_right, 5.0).expect("extrude side face berhasil");
    assert!(extruded_right.tessellate().triangle_count() > 0);
}

#[test]
fn test_revolve_face() {
    let _guard = lock_test();
    // Box 30x20x15
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();

    let ray_top = PickRay {
        origin: (15.0, 10.0, 50.0),
        dir: (0.0, 0.0, -1.0),
    };

    // Revolve top face (z=15) around axis at edge (0, 0, 15) along (0, 1, 0)
    let revolved = revolve_face(
        &shape,
        ray_top,
        glam::dvec3(0.0, 0.0, 15.0),
        glam::dvec3(0.0, 1.0, 0.0),
        Some(90.0),
    )
    .expect("revolve top face 90 deg harus berhasil");

    assert!(revolved.tessellate().triangle_count() > 0);
}

#[test]
fn test_resize_shape_along_edge_only_changes_target_axis() {
    let _guard = lock_test();
    // Box width=30 (X), depth=20 (Y), height=15 (Z)
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();

    // Resize vertical edge (Z) from 15.0 to 35.0 (+20.0)
    let edge_start = (0.0, 0.0, 0.0);
    let edge_end = (0.0, 0.0, 15.0);
    let resized = resize_shape_along_edge(&shape, edge_start, edge_end, 35.0)
        .expect("resize vertical edge harus berhasil");

    let mesh = resized.tessellate();
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for p in &mesh.positions {
        for k in 0..3 {
            min[k] = min[k].min(p[k]);
            max[k] = max[k].max(p[k]);
        }
    }
    // Width (X) harus tetap 30.0
    assert!((max[0] - min[0] - 30.0).abs() < 1e-3, "Width X harus tetap 30.0, dapat {}", max[0] - min[0]);
    // Depth (Y) harus tetap 20.0
    assert!((max[1] - min[1] - 20.0).abs() < 1e-3, "Depth Y harus tetap 20.0, dapat {}", max[1] - min[1]);
    // Shrink vertical edge (Z) from 35.0 to 10.0 (-25.0)
    let shrink_edge_start = (0.0, 0.0, 0.0);
    let shrink_edge_end = (0.0, 0.0, 35.0);
    let shrunk = resize_shape_along_edge(&resized, shrink_edge_start, shrink_edge_end, 10.0)
        .expect("shrink vertical edge harus berhasil");

    let mesh_shrunk = shrunk.tessellate();
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for p in &mesh_shrunk.positions {
        for k in 0..3 {
            min[k] = min[k].min(p[k]);
            max[k] = max[k].max(p[k]);
        }
    }
    assert!((max[0] - min[0] - 30.0).abs() < 1e-3, "Width X harus tetap 30.0, dapat {}", max[0] - min[0]);
    assert!((max[1] - min[1] - 20.0).abs() < 1e-3, "Depth Y harus tetap 20.0, dapat {}", max[1] - min[1]);
    assert!((max[2] - min[2] - 10.0).abs() < 1e-3, "Height Z harus menjadi 10.0, dapat {}", max[2] - min[2]);
}

#[test]
fn test_extrude_face_cylinder_top() {
    let _guard = lock_test();
    let circle_profile = Profile::Circle {
        center: (0.0, 0.0),
        radius: 12.0,
    };
    let cylinder = extrude_profile(&circle_profile, 25.0).unwrap();
    let ray_top = PickRay {
        origin: (0.0, 0.0, 100.0),
        dir: (0.0, 0.0, -1.0),
    };
    let hit_top = pick_face_details(&cylinder, ray_top).expect("harus kena top cap silinder");
    assert!((hit_top.normal.2 - 1.0).abs() < 1e-5);
    assert!((hit_top.centroid.2 - 25.0).abs() < 1e-5);

    let taller_cylinder = extrude_face(&cylinder, ray_top, 15.0).expect("extrude top cap silinder berhasil");
    assert!(taller_cylinder.tessellate().triangle_count() > 0);
}

#[test]
fn surface_kind_detects_plane_faces_on_cube() {
    let _guard = lock_test();
    let cube = AdHocShape::make_box(10.0, 10.0, 10.0);
    let faces: Vec<_> = cube.faces().collect();
    assert_eq!(faces.len(), 6, "kubus harus punya 6 face");
    for face in &faces {
        assert_eq!(SurfaceKind::from(face.surface_kind().as_str()), SurfaceKind::Plane);
    }
}

#[test]
fn surface_kind_detects_plane_and_cylinder_faces_on_cylinder() {
    let _guard = lock_test();
    let cylinder = AdHocShape::make_cylinder(dvec3(0.0, 0.0, 0.0), 5.0, 12.0);
    let mut plane_count = 0;
    let mut cylinder_count = 0;
    for face in cylinder.faces() {
        match SurfaceKind::from(face.surface_kind().as_str()) {
            SurfaceKind::Plane => plane_count += 1,
            SurfaceKind::Cylinder => cylinder_count += 1,
            other => panic!("face silinder tak terduga: {other:?}"),
        }
    }
    assert_eq!(plane_count, 2, "silinder harus punya 2 face Plane (tutup atas & bawah)");
    assert_eq!(cylinder_count, 1, "silinder harus punya 1 face Cylinder (selimut)");
}

#[test]
fn surface_kind_detects_sphere_face() {
    let _guard = lock_test();
    let sphere = AdHocShape::make_sphere(7.0);
    let faces: Vec<_> = sphere.faces().collect();
    assert_eq!(faces.len(), 1, "bola harus punya 1 face");
    assert_eq!(SurfaceKind::from(faces[0].surface_kind().as_str()), SurfaceKind::Sphere);
}

fn assert_close(actual: f64, expected: f64, label: &str) {
    let rel_diff = (actual - expected).abs() / expected.abs().max(1e-9);
    assert!(
        rel_diff < 1e-6,
        "{label}: actual={actual}, expected={expected}, rel_diff={rel_diff}"
    );
}

#[test]
fn extrude_face_cylinder_outer_wall_grows_radius_when_pulled_out() {
    let _guard = lock_test();
    const R: f64 = 10.0;
    const H: f64 = 20.0;
    let cylinder = KernelShape(AdHocShape::make_cylinder(dvec3(0.0, 0.0, 0.0), R, H).0);
    let ray = PickRay { origin: (R + 50.0, 0.0, H / 2.0), dir: (-1.0, 0.0, 0.0) };
    let hit = pick_face_details(&cylinder, ray).expect("harus kena selimut silinder");
    assert_eq!(hit.surface_kind, SurfaceKind::Cylinder);

    let grown = extrude_face(&cylinder, ray, 2.0).expect("pull +2 pada selimut silinder harus berhasil");
    assert_close(grown.inner().volume(), std::f64::consts::PI * 12.0 * 12.0 * H, "volume silinder R=12,h=20");
}

#[test]
fn extrude_face_cylinder_outer_wall_shrinks_radius_when_pulled_in() {
    let _guard = lock_test();
    const R: f64 = 10.0;
    const H: f64 = 20.0;
    let cylinder = KernelShape(AdHocShape::make_cylinder(dvec3(0.0, 0.0, 0.0), R, H).0);
    let ray = PickRay { origin: (R + 50.0, 0.0, H / 2.0), dir: (-1.0, 0.0, 0.0) };

    let shrunk = extrude_face(&cylinder, ray, -3.0).expect("push -3 pada selimut silinder harus berhasil");
    assert_close(shrunk.inner().volume(), std::f64::consts::PI * 7.0 * 7.0 * H, "volume silinder R=7,h=20");
}

#[test]
fn extrude_face_cylinder_outer_wall_rejects_offset_making_radius_non_positive() {
    let _guard = lock_test();
    const R: f64 = 10.0;
    const H: f64 = 20.0;
    let cylinder = KernelShape(AdHocShape::make_cylinder(dvec3(0.0, 0.0, 0.0), R, H).0);
    let ray = PickRay { origin: (R + 50.0, 0.0, H / 2.0), dir: (-1.0, 0.0, 0.0) };

    match extrude_face(&cylinder, ray, -10.0) {
        Ok(_) => panic!("radius jadi 0 harus ditolak"),
        Err(err) => assert!(err.to_string().contains("radius"), "pesan error harus jelas soal radius: {err}"),
    }
}

#[test]
fn extrude_face_hollow_cylinder_inner_wall_shrinks_hole_when_pushed_radially_inward() {
    let _guard = lock_test();
    const R_OUT: f64 = 20.0;
    const R_IN: f64 = 8.0;
    const H: f64 = 20.0;
    let outer = AdHocShape::make_cylinder(dvec3(0.0, 0.0, 0.0), R_OUT, H);
    let inner = AdHocShape::make_cylinder(dvec3(0.0, 0.0, -1.0), R_IN, H + 2.0);
    let mut tube_shape = outer.0.subtract(&inner.0).unwrap().shape;
    tube_shape = tube_shape.clean();
    let tube = KernelShape(tube_shape);

    let hole_ray = PickRay { origin: (0.0, 0.0, H / 2.0), dir: (1.0, 0.0, 0.0) };
    let hit = pick_face_details(&tube, hole_ray).expect("harus kena dinding lubang");
    assert_eq!(hit.surface_kind, SurfaceKind::Cylinder);

    let shrunk_hole = extrude_face(&tube, hole_ray, 2.0).expect("offset dinding lubang harus berhasil");
    let expect_vol = std::f64::consts::PI * (R_OUT * R_OUT - 6.0 * 6.0) * H;
    assert_close(shrunk_hole.inner().volume(), expect_vol, "volume tabung dgn lubang R=6 (mengecil dari R=8)");
}

#[test]
fn extrude_face_hollow_cylinder_inner_wall_enlarges_past_original_radius_when_pulled_radially_outward() {
    let _guard = lock_test();
    const R_OUT: f64 = 20.0;
    const R_IN: f64 = 8.0;
    const H: f64 = 20.0;
    let outer = AdHocShape::make_cylinder(dvec3(0.0, 0.0, 0.0), R_OUT, H);
    let inner = AdHocShape::make_cylinder(dvec3(0.0, 0.0, -1.0), R_IN, H + 2.0);
    let mut tube_shape = outer.0.subtract(&inner.0).unwrap().shape;
    tube_shape = tube_shape.clean();
    let tube = KernelShape(tube_shape);

    let hole_ray = PickRay { origin: (0.0, 0.0, H / 2.0), dir: (1.0, 0.0, 0.0) };
    let hit = pick_face_details(&tube, hole_ray).expect("harus kena dinding lubang");
    assert_eq!(hit.surface_kind, SurfaceKind::Cylinder);

    let enlarged_hole =
        extrude_face(&tube, hole_ray, -5.0).expect("offset dinding lubang (memperbesar) harus berhasil");
    let expect_vol = std::f64::consts::PI * (R_OUT * R_OUT - 13.0 * 13.0) * H;
    assert_close(enlarged_hole.inner().volume(), expect_vol, "volume tabung dgn lubang R=13 (membesar dari R=8)");
}

#[test]
fn extrude_face_hollow_cylinder_inner_wall_rejects_offset_that_closes_hole_completely() {
    let _guard = lock_test();
    const R_OUT: f64 = 20.0;
    const R_IN: f64 = 8.0;
    const H: f64 = 20.0;
    let outer = AdHocShape::make_cylinder(dvec3(0.0, 0.0, 0.0), R_OUT, H);
    let inner = AdHocShape::make_cylinder(dvec3(0.0, 0.0, -1.0), R_IN, H + 2.0);
    let mut tube_shape = outer.0.subtract(&inner.0).unwrap().shape;
    tube_shape = tube_shape.clean();
    let tube = KernelShape(tube_shape);

    let hole_ray = PickRay { origin: (0.0, 0.0, H / 2.0), dir: (1.0, 0.0, 0.0) };
    pick_face_details(&tube, hole_ray).expect("harus kena dinding lubang");

    match extrude_face(&tube, hole_ray, R_IN) {
        Ok(_) => panic!("lubang menutup penuh (radius jadi 0) harus ditolak"),
        Err(err) => assert!(err.to_string().contains("radius"), "pesan error harus jelas soal radius: {err}"),
    }
}

#[test]
fn extrude_face_sphere_grows_radius_when_pulled_out() {
    let _guard = lock_test();
    const R: f64 = 7.0;
    let sphere = KernelShape(AdHocShape::make_sphere(R).0);
    let ray = PickRay { origin: (50.0, 0.0, 0.0), dir: (-1.0, 0.0, 0.0) };

    let (face, _) = crate::picking::face::resolve_face_along_ray(sphere.inner(), ray).expect("harus kena permukaan bola");
    assert_eq!(SurfaceKind::from(face.surface_kind().as_str()), SurfaceKind::Sphere);

    let grown = extrude_face(&sphere, ray, 1.5).expect("pull +1.5 pada bola harus berhasil");
    let expect_vol = 4.0 / 3.0 * std::f64::consts::PI * (R + 1.5) * (R + 1.5) * (R + 1.5);
    assert_close(grown.inner().volume(), expect_vol, "volume bola R=8.5");
}

#[test]
fn extrude_face_cone_lateral_face_changes_volume_in_pull_direction() {
    let _guard = lock_test();
    const CONE_R: f64 = 6.0;
    const CONE_H: f64 = 14.0;
    let cone_profile = Profile::Loop(vec![
        ProfileSegment::Line { start: (0.0, 0.0), end: (CONE_R, 0.0) },
        ProfileSegment::Line { start: (CONE_R, 0.0), end: (0.0, CONE_H) },
        ProfileSegment::Line { start: (0.0, CONE_H), end: (0.0, 0.0) },
    ]);
    let cone = revolve_profile(&cone_profile, (0.0, 0.0), (0.0, 1.0), None).unwrap();
    let base_vol = cone.inner().volume();
    assert_close(base_vol, std::f64::consts::PI * CONE_R * CONE_R * CONE_H / 3.0, "volume kerucut awal");

    let ray = PickRay { origin: (50.0, CONE_H / 2.0, 0.0), dir: (-1.0, 0.0, 0.0) };
    let hit = pick_face_details(&cone, ray).expect("harus kena selimut kerucut");
    assert_eq!(hit.surface_kind, SurfaceKind::Cone);

    let grown = extrude_face(&cone, ray, 1.0).expect("pull +1.0 pada selimut kerucut harus berhasil");
    assert!(grown.inner().volume() > base_vol, "menarik selimut kerucut keluar harus menambah volume");
    assert!(grown.tessellate().triangle_count() > 0);

    let shrunk = extrude_face(&cone, ray, -1.0).expect("push -1.0 pada selimut kerucut harus berhasil");
    assert!(shrunk.inner().volume() < base_vol, "menekan selimut kerucut masuk harus mengurangi volume");
    assert!(shrunk.tessellate().triangle_count() > 0);
}

#[test]
fn extrude_face_planar_regression_still_uses_extrude_and_boolean_path() {
    let _guard = lock_test();
    let circle_profile = Profile::Circle { center: (0.0, 0.0), radius: 12.0 };
    let cylinder = extrude_profile(&circle_profile, 25.0).unwrap();
    let ray_top = PickRay { origin: (0.0, 0.0, 100.0), dir: (0.0, 0.0, -1.0) };
    let hit_top = pick_face_details(&cylinder, ray_top).expect("harus kena top cap silinder");
    assert_eq!(hit_top.surface_kind, SurfaceKind::Plane);

    let taller = extrude_face(&cylinder, ray_top, 15.0).expect("extrude top cap silinder berhasil");
    let expect_vol = std::f64::consts::PI * 12.0 * 12.0 * 40.0;
    assert_close(taller.inner().volume(), expect_vol, "volume silinder tinggi 40 (25+15) hasil jalur planar lama");
}

#[test]
fn extrude_face_adjacent_to_fillet_does_not_crash() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let edge_ray = PickRay { origin: (-5.0, -5.0, 7.5), dir: (1.0, 1.0, 0.0) };
    let filleted = fillet_edges(&shape, 8.0, &[edge_ray], 1.0).expect("fillet tepi vertikal box harus berhasil");

    let face_ray = PickRay { origin: (15.0, -50.0, 7.5), dir: (0.0, 1.0, 0.0) };
    let result = extrude_face(&filleted, face_ray, 5.0);
    match result {
        Ok(extruded) => {
            assert!(extruded.tessellate().triangle_count() > 0, "hasil sukses harus punya mesh valid");
        }
        Err(err) => {
            assert!(!err.to_string().is_empty(), "hasil gagal harus punya pesan error, bukan diam");
        }
    }
}

// ---- DUCAD Fase 4: `FaceHit::pull_dir` per `SurfaceKind` ----

#[test]
fn pull_dir_equals_normal_on_planar_face() {
    let _guard = lock_test();
    let circle_profile = Profile::Circle { center: (0.0, 0.0), radius: 12.0 };
    let cylinder = extrude_profile(&circle_profile, 25.0).unwrap();
    let ray_top = PickRay { origin: (0.0, 0.0, 100.0), dir: (0.0, 0.0, -1.0) };
    let hit_top = pick_face_details(&cylinder, ray_top).expect("harus kena top cap silinder");
    assert_eq!(hit_top.surface_kind, SurfaceKind::Plane);
    assert_eq!(hit_top.pull_dir, hit_top.normal, "Plane: pull_dir harus identik dgn normal Newell (perilaku lama)");
}

#[test]
fn pull_dir_is_radial_on_cylinder_wall() {
    let _guard = lock_test();
    const R: f64 = 10.0;
    const H: f64 = 20.0;
    let cylinder = KernelShape(AdHocShape::make_cylinder(dvec3(0.0, 0.0, 0.0), R, H).0);
    let ray = PickRay { origin: (R + 50.0, 0.0, H / 2.0), dir: (-1.0, 0.0, 0.0) };
    let hit = pick_face_details(&cylinder, ray).expect("harus kena selimut silinder");
    assert_eq!(hit.surface_kind, SurfaceKind::Cylinder);
    assert!((hit.pull_dir.0 - 1.0).abs() < 1e-6, "pull_dir salah: {:?}", hit.pull_dir);
    assert!(hit.pull_dir.1.abs() < 1e-6, "pull_dir salah: {:?}", hit.pull_dir);
    assert!(hit.pull_dir.2.abs() < 1e-6, "pull_dir salah: {:?}", hit.pull_dir);
}

#[test]
fn pull_dir_is_radial_on_cone_lateral_face() {
    let _guard = lock_test();
    const CONE_R: f64 = 6.0;
    const CONE_H: f64 = 14.0;
    let cone_profile = Profile::Loop(vec![
        ProfileSegment::Line { start: (0.0, 0.0), end: (CONE_R, 0.0) },
        ProfileSegment::Line { start: (CONE_R, 0.0), end: (0.0, CONE_H) },
        ProfileSegment::Line { start: (0.0, CONE_H), end: (0.0, 0.0) },
    ]);
    let cone = revolve_profile(&cone_profile, (0.0, 0.0), (0.0, 1.0), None).unwrap();
    let ray = PickRay { origin: (50.0, CONE_H / 2.0, 0.0), dir: (-1.0, 0.0, 0.0) };
    let hit = pick_face_details(&cone, ray).expect("harus kena selimut kerucut");
    assert_eq!(hit.surface_kind, SurfaceKind::Cone);
    assert!((hit.pull_dir.0 - 1.0).abs() < 1e-6, "pull_dir salah: {:?}", hit.pull_dir);
    assert!(hit.pull_dir.1.abs() < 1e-6, "pull_dir salah: {:?}", hit.pull_dir);
    assert!(hit.pull_dir.2.abs() < 1e-6, "pull_dir salah: {:?}", hit.pull_dir);
}

#[test]
fn pick_face_details_works_on_full_sphere_with_radial_pull_dir() {
    let _guard = lock_test();
    const R: f64 = 7.0;
    let sphere = KernelShape(AdHocShape::make_sphere(R).0);
    let ray = PickRay { origin: (50.0, 0.0, 0.0), dir: (-1.0, 0.0, 0.0) };
    let hit = pick_face_details(&sphere, ray)
        .expect("Fase 4: pick_face_details harus berhasil utk bola penuh (fallback GProp)");
    assert_eq!(hit.surface_kind, SurfaceKind::Sphere);
    assert!(
        hit.centroid.0.abs() < 1e-4 && hit.centroid.1.abs() < 1e-4 && hit.centroid.2.abs() < 1e-4,
        "centroid GProp bola penuh berpusat di origin, actual={:?}",
        hit.centroid
    );
    assert!((hit.pull_dir.0 - 1.0).abs() < 1e-4, "pull_dir salah: {:?}", hit.pull_dir);
    assert!(hit.pull_dir.1.abs() < 1e-4, "pull_dir salah: {:?}", hit.pull_dir);
    assert!(hit.pull_dir.2.abs() < 1e-4, "pull_dir salah: {:?}", hit.pull_dir);
}

#[test]
fn pull_dir_is_radial_on_partial_sphere_octant_face() {
    let _guard = lock_test();
    const R: f64 = 10.0;
    let sphere = AdHocShape::make_sphere(R);
    let octant_box =
        AdHocShape::make_box_point_point(dvec3(0.0, 0.0, 0.0), dvec3(R + 5.0, R + 5.0, R + 5.0));
    let octant = intersect(&KernelShape(sphere.0), &KernelShape(octant_box.0))
        .expect("irisan bola dgn box oktan harus berhasil");

    let ray = PickRay { origin: (R + 50.0, 0.001, 0.001), dir: (-1.0, 0.0, 0.0) };
    let hit = pick_face_details(&octant, ray).expect("harus kena permukaan bola oktan");
    assert_eq!(hit.surface_kind, SurfaceKind::Sphere);
    assert!(
        hit.centroid.0 > 1.0 && hit.centroid.1 > 1.0 && hit.centroid.2 > 1.0,
        "fixture salah: centroid loop harus condong ke oktan (+,+,+), BUKAN pusat bola: {:?}",
        hit.centroid
    );
    assert!((hit.pull_dir.0 - 1.0).abs() < 1e-3, "pull_dir salah (bukan radial dari pusat bola): {:?}", hit.pull_dir);
    assert!(hit.pull_dir.1.abs() < 1e-3, "pull_dir salah (bukan radial dari pusat bola): {:?}", hit.pull_dir);
    assert!(hit.pull_dir.2.abs() < 1e-3, "pull_dir salah (bukan radial dari pusat bola): {:?}", hit.pull_dir);
}

#[test]
fn rotate_shape_rotates_geometry_correctly() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(10.0, 5.0), 20.0).unwrap();
    // Putar 90 derajat sekeliling sumbu Z di origin (0, 0, 0)
    let rotated = rotate_shape(&shape, (0.0, 0.0, 0.0), (0.0, 0.0, 1.0), std::f64::consts::FRAC_PI_2)
        .expect("rotate_shape harus berhasil");
    let mesh = rotated.tessellate();
    assert!(mesh.triangle_count() > 0);
    // Bounding check: semula X in [0, 10], Y in [0, 5] -> setelah rotasi +90° di Z: X in [-5, 0], Y in [0, 10]
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for p in &mesh.positions {
        min_x = min_x.min(p[0]);
        max_x = max_x.max(p[0]);
        min_y = min_y.min(p[1]);
        max_y = max_y.max(p[1]);
    }
    assert!(min_x >= -5.01 && max_x <= 0.01, "X bounds mismatch: min={}, max={}", min_x, max_x);
    assert!(min_y >= -0.01 && max_y <= 10.01, "Y bounds mismatch: min={}, max={}", min_y, max_y);
}

#[test]
fn transform_shape_translates_and_rotates() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(10.0, 10.0), 10.0).unwrap();
    let transformed = transform_shape(
        &shape,
        (100.0, 50.0, 20.0),
        (5.0, 5.0, 5.0),
        (0.0, 0.0, 1.0),
        std::f64::consts::PI,
    )
    .expect("transform_shape harus berhasil");
    let mesh = transformed.tessellate();
    assert!(mesh.triangle_count() > 0);
    // Centroid harus berada dekat (105, 55, 25)
    let mut avg = glam::Vec3::ZERO;
    for p in &mesh.positions {
        avg += glam::Vec3::from_slice(p);
    }
    avg /= mesh.positions.len() as f32;
    assert!((avg.x - 105.0).abs() < 1.0, "avg.x = {}", avg.x);
    assert!((avg.y - 55.0).abs() < 1.0, "avg.y = {}", avg.y);
    assert!((avg.z - 25.0).abs() < 1.0, "avg.z = {}", avg.z);
}

#[test]
fn sweep_circle_along_line_produces_cylinder() {
    let _guard = lock_test();
    let profile = Profile::Circle { center: (0.0, 0.0), radius: 5.0 };
    let path = vec![
        PathSegment::Line {
            start: [0.0, 0.0, 0.0],
            end: [0.0, 0.0, 50.0],
        },
    ];
    let swept = sweep_profile_along_path(&profile, &path).expect("sweep circle along line harus berhasil");
    let mesh = swept.tessellate();
    assert!(mesh.triangle_count() > 0);
    assert!(mesh.positions.len() > 10);
    // Bounding Z harus mencakup [0, 50]
    let mut min_z = f32::INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    for p in &mesh.positions {
        min_z = min_z.min(p[2]);
        max_z = max_z.max(p[2]);
    }
    assert!((min_z - 0.0).abs() < 0.1, "min_z = {min_z}");
    assert!((max_z - 50.0).abs() < 0.1, "max_z = {max_z}");
}

#[test]
fn sweep_circle_along_arc_produces_curved_pipe() {
    let _guard = lock_test();
    let profile = Profile::Circle { center: (0.0, 0.0), radius: 3.0 };
    // Busur 90 derajat di bidang XZ dari (0,0,0) via (29.29, 0, 70.71) ke (100, 0, 100) (radius 100)
    let path = vec![
        PathSegment::Arc {
            start: [0.0, 0.0, 0.0],
            via: [29.289, 0.0, 70.711],
            end: [100.0, 0.0, 100.0],
        },
    ];
    let swept = sweep_profile_along_path(&profile, &path).expect("sweep circle along arc harus berhasil");
    let mesh = swept.tessellate();
    assert!(mesh.triangle_count() > 0);
    // Verifikasi bounding box
    let mut max_x = f32::NEG_INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    for p in &mesh.positions {
        max_x = max_x.max(p[0]);
        max_z = max_z.max(p[2]);
    }
    assert!(max_x >= 95.0, "max_x = {max_x}");
    assert!(max_z >= 95.0, "max_z = {max_z}");
}

#[test]
fn sweep_rectangle_along_polyline_path() {
    let _guard = lock_test();
    let profile = rect_profile(10.0, 6.0);
    let path = vec![
        PathSegment::Line { start: [0.0, 0.0, 0.0], end: [0.0, 0.0, 30.0] },
        PathSegment::Line { start: [0.0, 0.0, 30.0], end: [20.0, 0.0, 50.0] },
    ];
    let swept = sweep_profile_along_path(&profile, &path).expect("sweep rect along polyline path harus berhasil");
    let mesh = swept.tessellate();
    assert!(mesh.triangle_count() > 0);
}

#[test]
fn sweep_empty_path_fails_gracefully() {
    let _guard = lock_test();
    let profile = Profile::Circle { center: (0.0, 0.0), radius: 5.0 };
    let path = vec![];
    let res = sweep_profile_along_path(&profile, &path);
    assert!(res.is_err());
}

#[test]
fn draft_angle_single_face_success() {
    let _guard = lock_test();
    let profile = rect_profile(20.0, 20.0);
    let shape = extrude_profile(&profile, 30.0).expect("extrude box harus berhasil");

    // Raycast ke side face di X=20 (menghadap +X)
    let side_ray = PickRay {
        origin: (50.0, 10.0, 15.0),
        dir: (-1.0, 0.0, 0.0),
    };

    let drafted = draft_angle(
        &shape,
        glam::DVec3::new(0.0, 0.0, 0.0),      // neutral plane at Z=0
        glam::DVec3::new(0.0, 0.0, 1.0),      // neutral plane normal = +Z
        glam::DVec3::new(0.0, 0.0, 1.0),      // pull direction = +Z
        3.0,                                   // 3 degrees draft
        &[side_ray],
    )
    .expect("draft_angle single face harus berhasil");

    let mesh = drafted.tessellate();
    assert!(mesh.triangle_count() > 0);
}

#[test]
fn draft_angle_multiple_faces_success() {
    let _guard = lock_test();
    let profile = rect_profile(30.0, 30.0);
    let shape = extrude_profile(&profile, 40.0).expect("extrude box harus berhasil");

    // 4 side faces
    let ray_right = PickRay { origin: (50.0, 15.0, 20.0), dir: (-1.0, 0.0, 0.0) };
    let ray_left = PickRay { origin: (-50.0, 15.0, 20.0), dir: (1.0, 0.0, 0.0) };
    let ray_front = PickRay { origin: (15.0, -50.0, 20.0), dir: (0.0, 1.0, 0.0) };
    let ray_back = PickRay { origin: (15.0, 50.0, 20.0), dir: (0.0, -1.0, 0.0) };

    let drafted = draft_angle(
        &shape,
        glam::DVec3::new(0.0, 0.0, 0.0),
        glam::DVec3::new(0.0, 0.0, 1.0),
        glam::DVec3::new(0.0, 0.0, 1.0),
        2.5,
        &[ray_right, ray_left, ray_front, ray_back],
    )
    .expect("draft_angle 4 faces harus berhasil");

    let mesh = drafted.tessellate();
    assert!(mesh.triangle_count() > 0);
}

#[test]
fn draft_angle_invalid_angle_errors() {
    let _guard = lock_test();
    let profile = rect_profile(20.0, 20.0);
    let shape = extrude_profile(&profile, 30.0).unwrap();
    let ray = PickRay { origin: (50.0, 10.0, 15.0), dir: (-1.0, 0.0, 0.0) };

    // Sudut 0 atau negatif harus error
    let res_zero = draft_angle(&shape, glam::DVec3::ZERO, glam::DVec3::Z, glam::DVec3::Z, 0.0, &[ray]);
    assert!(res_zero.is_err());

    let res_neg = draft_angle(&shape, glam::DVec3::ZERO, glam::DVec3::Z, glam::DVec3::Z, -5.0, &[ray]);
    assert!(res_neg.is_err());

    // Sudut >= 90 harus error
    let res_90 = draft_angle(&shape, glam::DVec3::ZERO, glam::DVec3::Z, glam::DVec3::Z, 90.0, &[ray]);
    assert!(res_90.is_err());
}

#[test]
fn draft_angle_empty_rays_errors() {
    let _guard = lock_test();
    let profile = rect_profile(20.0, 20.0);
    let shape = extrude_profile(&profile, 30.0).unwrap();

    let res = draft_angle(&shape, glam::DVec3::ZERO, glam::DVec3::Z, glam::DVec3::Z, 3.0, &[]);
    assert!(res.is_err());
}

#[test]
fn split_box_into_two_bodies() {
    let _guard = lock_test();
    let profile = rect_profile(20.0, 20.0);
    // Extrude 40mm tinggi (Z: 0 .. 40)
    let shape = extrude_profile(&profile, 40.0).expect("extrude box harus berhasil");

    // Potong dengan bidang horizontal Z=20 (tengah-tengah)
    let parts = split_body(
        &shape,
        glam::DVec3::new(0.0, 0.0, 20.0),
        glam::DVec3::new(0.0, 0.0, 1.0),
    )
    .expect("split_body harus berhasil");

    assert_eq!(parts.len(), 2, "Harus menghasilkan tepat 2 body terpisah");
    let mesh1 = parts[0].tessellate();
    let mesh2 = parts[1].tessellate();
    assert!(mesh1.triangle_count() > 0);
    assert!(mesh2.triangle_count() > 0);
}

#[test]
fn split_cylinder_into_two_halves() {
    let _guard = lock_test();
    let profile = Profile::Circle { center: (0.0, 0.0), radius: 10.0 };
    let shape = extrude_profile(&profile, 30.0).expect("extrude cylinder harus berhasil");

    // Potong dengan bidang vertikal X=0 (normal +X)
    let parts = split_body(
        &shape,
        glam::DVec3::new(0.0, 0.0, 0.0),
        glam::DVec3::new(1.0, 0.0, 0.0),
    )
    .expect("split cylinder harus berhasil");

    assert_eq!(parts.len(), 2, "Harus menghasilkan 2 setengah silinder");
    assert!(parts[0].tessellate().triangle_count() > 0);
    assert!(parts[1].tessellate().triangle_count() > 0);
}

#[test]
fn split_face_on_box() {
    let _guard = lock_test();
    let profile = rect_profile(20.0, 20.0);
    let shape = extrude_profile(&profile, 20.0).expect("extrude box harus berhasil");

    let split = split_face(
        &shape,
        glam::DVec3::new(0.0, 0.0, 10.0),
        glam::DVec3::new(0.0, 0.0, 1.0),
    )
    .expect("split_face harus berhasil");

    let orig_faces = shape.inner().faces().count();
    let new_faces = split.inner().faces().count();

    let mesh = split.tessellate();
    assert!(mesh.triangle_count() > 0);
    assert_eq!(orig_faces, 6, "Box sebelum split harus punya 6 face");
    assert!(
        new_faces > orig_faces,
        "split_face harus MENAMBAH jumlah face (6 -> {new_faces}), bukan menyisakannya apa adanya"
    );
    assert_eq!(new_faces, 10, "Box 6 face saat di-split di tengah harus memiliki 10 face terpisah");
}

#[test]
fn split_box_offset_from_origin() {
    let _guard = lock_test();
    let profile = rect_profile(20.0, 20.0);
    let shape = extrude_profile(&profile, 40.0).expect("extrude box harus berhasil");
    // Translate shape to X=100, Y=100, Z=100
    let moved = crate::shape::translate_shape(&shape, 100.0, 100.0, 100.0).expect("translate harus berhasil");

    // Center-nya sekarang di (100, 100, 120). Potong di Z=120
    let parts = split_body(
        &moved,
        glam::DVec3::new(100.0, 100.0, 120.0),
        glam::DVec3::new(0.0, 0.0, 1.0),
    )
    .expect("split_body pada box yang jauh dari origin harus berhasil");

    assert_eq!(parts.len(), 2, "Harus menghasilkan tepat 2 body terpisah");
}

#[test]
fn test_linear_pattern_shape() {
    let _guard = lock_test();
    let profile = rect_profile(10.0, 10.0);
    let shape = extrude_profile(&profile, 10.0).expect("extrude box harus berhasil");

    // 2 x 2 x 2 pattern -> 8 total, 7 new copies
    let pattern = linear_pattern_shape(&shape, 2, 20.0, 2, 20.0, 2, 20.0).expect("linear_pattern_shape harus berhasil");
    assert_eq!(pattern.len(), 7);

    for s in &pattern {
        let mesh = s.tessellate();
        assert!(mesh.triangle_count() > 0);
    }
}

#[test]
fn test_circular_pattern_shape() {
    let _guard = lock_test();
    let profile = rect_profile(5.0, 5.0);
    let shape = extrude_profile(&profile, 10.0).expect("extrude box harus berhasil");
    // Geser box 20mm ke arah +X
    let moved = crate::shape::translate_shape(&shape, 20.0, 0.0, 0.0).unwrap();

    // 4 items 360 deg sekeliling sumbu Z -> 3 new copies
    let pattern = circular_pattern_shape(
        &moved,
        (0.0, 0.0, 0.0),
        (0.0, 0.0, 1.0),
        4,
        std::f64::consts::TAU,
    )
    .expect("circular_pattern_shape harus berhasil");

    assert_eq!(pattern.len(), 3);
    for s in &pattern {
        let mesh = s.tessellate();
        assert!(mesh.triangle_count() > 0);
    }
}

#[test]
fn test_shell_variable_thickness() {
    let _guard = lock_test();
    let profile = rect_profile(40.0, 40.0);
    let shape = extrude_profile(&profile, 20.0).expect("extrude box harus berhasil");

    // Ray ke top face (+Z) untuk dibuka / dihilangkan
    let ray_top = PickRay {
        origin: (20.0, 20.0, 100.0),
        dir: (0.0, 0.0, -1.0),
    };
    // Ray ke bottom face (-Z) untuk diberi custom thickness 4.0 mm (dinding samping 2.0 mm)
    let ray_bottom = PickRay {
        origin: (20.0, 20.0, -100.0),
        dir: (0.0, 0.0, 1.0),
    };

    let result = shell_variable_thickness(
        &shape,
        2.0,
        &[ray_top],
        &[(ray_bottom, 4.0)],
    )
    .expect("shell_variable_thickness harus berhasil");

    let mesh = result.tessellate();
    assert!(mesh.triangle_count() > 0, "hasil shell variable harus memiliki mesh bertriangle");
    assert!(!mesh.positions.is_empty());
}

#[test]
fn test_create_rib_solid_and_union() {
    let _guard = lock_test();
    let profile = rect_profile(50.0, 50.0);
    let shape = extrude_profile(&profile, 30.0).expect("extrude box harus berhasil");

    // Hollow box dengan membuka face atas
    let hollowed = shell_hollow(&shape, 2.0, Direction::PosZ).expect("hollow box harus berhasil");
    let initial_tri_count = hollowed.tessellate().triangle_count();

    // Buat tulang penguat (rib) di tengah kotak dari X=2.0 hingga X=48.0 pada Y=25.0
    let start_pt = glam::dvec3(2.0, 25.0, 30.0);
    let end_pt = glam::dvec3(48.0, 25.0, 30.0);
    let normal_dir = glam::dvec3(0.0, 0.0, -1.0);

    let rib_solid = create_rib_solid(start_pt, end_pt, normal_dir, 2.0, 25.0, Some(1.5))
        .expect("create_rib_solid harus berhasil");
    assert!(rib_solid.tessellate().triangle_count() > 0);

    // Union rib ke hollow casing
    let casing_with_rib = create_rib(&hollowed, start_pt, end_pt, normal_dir, 2.0, 25.0, None)
        .expect("create_rib union ke casing harus berhasil");
    
    let mesh = casing_with_rib.tessellate();
    assert!(mesh.triangle_count() >= initial_tri_count, "mesh harus memuat rib yang menyatu");
}

#[test]
fn test_create_rib_from_curve() {
    let _guard = lock_test();
    let profile = rect_profile(60.0, 60.0);
    let shape = extrude_profile(&profile, 20.0).expect("extrude box harus berhasil");
    let hollowed = shell_hollow(&shape, 2.0, Direction::PosZ).expect("hollow box harus berhasil");

    // L-shaped rib path
    let pts = vec![
        glam::dvec3(5.0, 30.0, 20.0),
        glam::dvec3(30.0, 30.0, 20.0),
        glam::dvec3(30.0, 55.0, 20.0),
    ];
    let normal_dir = glam::dvec3(0.0, 0.0, -1.0);

    let result = create_rib_from_curve(&hollowed, &pts, normal_dir, 1.8, 15.0, None)
        .expect("create_rib_from_curve harus berhasil");

    let mesh = result.tessellate();
    assert!(mesh.triangle_count() > 0);
}

#[test]
fn test_hlr_extract_orthogonal_views_box() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(50.0, 30.0), 20.0).expect("extrude box harus berhasil");
    let mesh = shape.tessellate();

    let drawing = HlrExtractor::extract_drawing(&[&shape], &[&mesh]);

    // Verifikasi 4 tampak terproyeksi
    assert!(!drawing.front.segments.is_empty(), "Tampak Depan harus memiliki segmen garis");
    assert!(!drawing.top.segments.is_empty(), "Tampak Atas harus memiliki segmen garis");
    assert!(!drawing.right.segments.is_empty(), "Tampak Samping harus memiliki segmen garis");
    assert!(!drawing.isometric.segments.is_empty(), "Tampak Isometrik harus memiliki segmen garis");

    // Periksa dimensi bounding model
    let (dim_x, dim_y, dim_z) = drawing.model_dimensions();
    assert!((dim_x - 50.0).abs() < 1e-2, "Dimensi X harus 50mm, dapat {dim_x}");
    assert!((dim_y - 30.0).abs() < 1e-2, "Dimensi Y harus 30mm, dapat {dim_y}");
    assert!((dim_z - 20.0).abs() < 1e-2, "Dimensi Z harus 20mm, dapat {dim_z}");

    // Periksa adanya garis tampak (Visible)
    let has_visible_front = drawing.front.segments.iter().any(|s| s.kind == HlrLineKind::Visible || s.kind == HlrLineKind::Silhouette);
    assert!(has_visible_front, "Tampak Depan harus memiliki garis tampak (Visible)");
}

#[test]
fn test_hlr_extract_views_cylinder() {
    let _guard = lock_test();
    let circle_prof = Profile::Circle {
        center: (20.0, 20.0),
        radius: 15.0,
    };
    let shape = extrude_profile(&circle_prof, 40.0).expect("extrude silinder harus berhasil");
    let mesh = shape.tessellate();

    let drawing = HlrExtractor::extract_drawing(&[&shape], &[&mesh]);

    assert!(!drawing.front.segments.is_empty());
    assert!(!drawing.top.segments.is_empty());

    let (dim_x, dim_y, dim_z) = drawing.model_dimensions();
    assert!((dim_x - 30.0).abs() < 1.0, "Diameter silinder X ~30mm");
    assert!((dim_y - 30.0).abs() < 1.0, "Diameter silinder Y ~30mm");
    assert!((dim_z - 40.0).abs() < 1e-2, "Tinggi silinder Z 40mm");
}

#[test]
fn test_hole_wizard_simple_blind_and_through() {
    let _guard = lock_test();
    let box_prof = rect_profile(50.0, 50.0);
    let box_shape = extrude_profile(&box_prof, 30.0).expect("extrude box");

    // 1. Simple Blind Hole (Ø10mm, depth 15mm, 118° drill tip)
    let spec_blind = ducad_core::hole::HoleSpec {
        kind: ducad_core::hole::HoleKind::Simple,
        thread_size: ducad_core::hole::IsoMetricThread::Custom,
        diameter: 10.0,
        depth: 15.0,
        is_through: false,
        counterbore_diameter: 0.0,
        counterbore_depth: 0.0,
        countersink_diameter: 0.0,
        countersink_angle_deg: 90.0,
        thread_pitch: 1.5,
        thread_depth: 10.0,
        has_drill_tip: true,
    };

    let holed_blind = apply_hole(&box_shape, &spec_blind, (25.0, 25.0, 30.0), (0.0, 0.0, 1.0))
        .expect("apply blind hole");
    let mesh_blind = holed_blind.tessellate();
    assert!(mesh_blind.triangle_count() > 12, "mesh hasil blind hole harus memiliki segitiga lubang");

    // 2. Simple Through Hole (Ø12mm, Through All)
    let mut spec_through = spec_blind;
    spec_through.is_through = true;
    spec_through.diameter = 12.0;

    let holed_through = apply_hole(&box_shape, &spec_through, (25.0, 25.0, 30.0), (0.0, 0.0, 1.0))
        .expect("apply through hole");
    let mesh_through = holed_through.tessellate();
    assert!(mesh_through.triangle_count() > 12, "mesh hasil through hole harus valid");
}

#[test]
fn test_hole_wizard_counterbore_iso4762_m6() {
    let _guard = lock_test();
    let box_prof = rect_profile(60.0, 60.0);
    let box_shape = extrude_profile(&box_prof, 40.0).expect("extrude box");

    let spec_cbore = ducad_core::hole::HoleSpec::for_iso(
        ducad_core::hole::IsoMetricThread::M6,
        ducad_core::hole::HoleKind::Counterbore,
        25.0,
    );
    assert_eq!(spec_cbore.counterbore_diameter, 11.5);
    assert_eq!(spec_cbore.counterbore_depth, 6.5);
    assert_eq!(spec_cbore.diameter, 6.6);

    let holed = apply_hole(&box_shape, &spec_cbore, (30.0, 30.0, 40.0), (0.0, 0.0, 1.0))
        .expect("apply counterbore hole");
    let mesh = holed.tessellate();
    assert!(mesh.triangle_count() > 20, "mesh counterbore harus memiliki segitiga bertingkat");
}

#[test]
fn test_hole_wizard_countersink_iso10642_m4() {
    let _guard = lock_test();
    let box_prof = rect_profile(50.0, 50.0);
    let box_shape = extrude_profile(&box_prof, 30.0).expect("extrude box");

    let spec_csink = ducad_core::hole::HoleSpec::for_iso(
        ducad_core::hole::IsoMetricThread::M4,
        ducad_core::hole::HoleKind::Countersink,
        15.0,
    );
    assert_eq!(spec_csink.countersink_diameter, 8.9);
    assert_eq!(spec_csink.countersink_angle_deg, 90.0);
    assert_eq!(spec_csink.diameter, 4.5);

    let holed = apply_hole(&box_shape, &spec_csink, (25.0, 25.0, 30.0), (0.0, 0.0, 1.0))
        .expect("apply countersink hole");
    let mesh = holed.tessellate();
    assert!(mesh.triangle_count() > 20, "mesh countersink harus memiliki segitiga kerucut tirus");
}

#[test]
fn test_hole_wizard_tapped_m8() {
    let _guard = lock_test();
    let box_prof = rect_profile(50.0, 50.0);
    let box_shape = extrude_profile(&box_prof, 30.0).expect("extrude box");

    let spec_tap = ducad_core::hole::HoleSpec::for_iso(
        ducad_core::hole::IsoMetricThread::M8,
        ducad_core::hole::HoleKind::Tapped,
        20.0,
    );
    assert_eq!(spec_tap.diameter, 6.8); // Tap drill M8
    assert_eq!(spec_tap.thread_pitch, 1.25);

    let holed = apply_hole(&box_shape, &spec_tap, (25.0, 25.0, 30.0), (0.0, 0.0, 1.0))
        .expect("apply tapped hole");
    let mesh = holed.tessellate();
    assert!(mesh.triangle_count() > 15, "mesh tapped hole harus valid");
}

#[test]
fn test_hole_wizard_on_side_face() {
    let _guard = lock_test();
    let box_prof = rect_profile(40.0, 40.0);
    let box_shape = extrude_profile(&box_prof, 40.0).expect("extrude box");

    let spec = ducad_core::hole::HoleSpec::for_iso(
        ducad_core::hole::IsoMetricThread::M5,
        ducad_core::hole::HoleKind::Counterbore,
        15.0,
    );

    // Buat lubang pada sisi X+ (normal: (1.0, 0.0, 0.0)) di (40.0, 20.0, 20.0)
    let holed = apply_hole(&box_shape, &spec, (40.0, 20.0, 20.0), (1.0, 0.0, 0.0))
        .expect("apply counterbore hole on X+ face");
    let mesh = holed.tessellate();
    assert!(mesh.triangle_count() > 20, "mesh side hole harus valid");
}

#[test]
fn test_emboss_and_deboss_profiles() {
    let _guard = lock_test();
    let box_prof = rect_profile(50.0, 50.0);
    let box_shape = extrude_profile(&box_prof, 20.0).expect("extrude base box");

    // 1. Emboss profil lingkaran di atas balok (Z = 20)
    let circle_prof = Profile::Circle {
        center: (25.0, 25.0),
        radius: 8.0,
    };
    let embossed = emboss_profiles_on_plane(
        Some(&box_shape),
        std::slice::from_ref(&circle_prof),
        [0.0, 0.0, 20.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        3.0,
        false, // Emboss (timbul)
    )
    .expect("emboss circle on top face");

    let mesh_emboss = embossed.tessellate();
    assert!(
        mesh_emboss.triangle_count() > 12,
        "mesh emboss solid harus valid"
    );

    // 2. Deboss (ukiran tenggelam) lingkaran ke dalam balok
    let debossed = emboss_profiles_on_plane(
        Some(&box_shape),
        &[circle_prof],
        [0.0, 0.0, 20.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        4.0,
        true, // Deboss (ukir / subtract)
    )
    .expect("deboss circle into top face");

    let mesh_deboss = debossed.tessellate();
    assert!(
        mesh_deboss.triangle_count() > 12,
        "mesh deboss solid harus valid"
    );
}

#[test]
fn helix_points_and_wire_generation() {
    let params = HelixParams {
        radius: 15.0,
        end_radius: None,
        pitch: 8.0,
        turns: 3.0,
        handedness: HelixHandedness::RightHand,
        origin: [0.0, 0.0, 0.0],
        axis: [0.0, 0.0, 1.0],
        start_dir: [1.0, 0.0, 0.0],
    };

    let pts = generate_helix_points(&params, 32).expect("generate helix points");
    assert!(pts.len() >= 32 * 3);

    // Titik awal harus di (15, 0, 0)
    assert!((pts[0][0] - 15.0).abs() < 1e-4);
    assert!(pts[0][1].abs() < 1e-4);
    assert!(pts[0][2].abs() < 1e-4);

    // Titik akhir harus di Z = pitch * turns = 24.0
    let last = pts.last().unwrap();
    assert!((last[2] - 24.0).abs() < 1e-3);

    let _wire = create_helix_wire(&params, 32).expect("create helix wire");
}

#[test]
fn helix_solid_circular_spring_produces_mesh() {
    let _guard = lock_test();
    let params = HelixParams {
        radius: 20.0,
        end_radius: None,
        pitch: 10.0,
        turns: 2.0,
        handedness: HelixHandedness::RightHand,
        origin: [0.0, 0.0, 0.0],
        axis: [0.0, 0.0, 1.0],
        start_dir: [1.0, 0.0, 0.0],
    };

    let spring = create_helix_solid(&params, HelixProfileKind::Circle { radius: 2.0 }, 32)
        .expect("create circular spring solid");

    let mesh = spring.tessellate();
    assert!(mesh.triangle_count() > 50, "mesh spring solid harus valid");
    assert!(!mesh.positions.is_empty());
}

#[test]
fn helix_solid_rectangular_auger_blade_produces_mesh() {
    let _guard = lock_test();
    let params = HelixParams {
        radius: 25.0,
        end_radius: None,
        pitch: 15.0,
        turns: 1.5,
        handedness: HelixHandedness::RightHand,
        origin: [0.0, 0.0, 0.0],
        axis: [0.0, 0.0, 1.0],
        start_dir: [1.0, 0.0, 0.0],
    };

    let auger = create_helix_solid(
        &params,
        HelixProfileKind::Rectangle {
            width: 8.0,
            height: 2.5,
        },
        32,
    )
    .expect("create rectangular auger blade");

    let mesh = auger.tessellate();
    assert!(mesh.triangle_count() > 50, "mesh auger blade solid harus valid");
}

#[test]
fn helix_solid_triangular_thread_produces_mesh() {
    let _guard = lock_test();
    let params = HelixParams {
        radius: 12.0,
        end_radius: None,
        pitch: 5.0,
        turns: 2.0,
        handedness: HelixHandedness::LeftHand,
        origin: [0.0, 0.0, 0.0],
        axis: [0.0, 0.0, 1.0],
        start_dir: [1.0, 0.0, 0.0],
    };

    let thread = create_helix_solid(
        &params,
        HelixProfileKind::Triangle {
            width: 3.0,
            height: 2.0,
        },
        32,
    )
    .expect("create triangular thread");

    let mesh = thread.tessellate();
    assert!(mesh.triangle_count() > 50, "mesh thread solid harus valid");
}

#[test]
fn conical_tapered_helix_spring_produces_mesh() {
    let _guard = lock_test();
    let params = HelixParams {
        radius: 25.0,
        end_radius: Some(10.0), // Tirus mengecil dari R=25 ke R=10
        pitch: 8.0,
        turns: 2.0,
        handedness: HelixHandedness::RightHand,
        origin: [0.0, 0.0, 0.0],
        axis: [0.0, 0.0, 1.0],
        start_dir: [1.0, 0.0, 0.0],
    };

    let conical_spring = create_helix_solid(&params, HelixProfileKind::Circle { radius: 1.5 }, 32)
        .expect("create conical spring solid");

    let mesh = conical_spring.tessellate();
    assert!(mesh.triangle_count() > 50, "mesh conical spring solid harus valid");
}

#[test]
fn test_section_view_brep_slice_and_hatch_generation() {
    let _guard = lock_test();
    let box_prof = rect_profile(60.0, 40.0);
    let box_shape = extrude_profile(&box_prof, 30.0).expect("extrude box");
    let mesh = box_shape.tessellate();

    // Iris solid pada bidang Y = 20.0 (potongan tengah melintang)
    let sec_cfg = crate::section::SectionPlaneConfig {
        origin: [0.0, 20.0, 0.0],
        normal: [0.0, 1.0, 0.0],
        u_axis: [1.0, 0.0, 0.0],
        v_axis: [0.0, 0.0, 1.0],
        hatch_spacing: 3.0,
        hatch_angle_deg: 45.0,
    };

    let (section_view, indicator) = crate::section::SectionExtractor::extract_section_view(
        &[&box_shape],
        &[&mesh],
        &sec_cfg,
        ([0.0, 0.0, 0.0], [60.0, 40.0, 30.0]),
    );

    assert_eq!(section_view.kind, ProjectedViewKind::SectionAA);
    assert!(!section_view.segments.is_empty(), "Tampak potongan harus memiliki segmen");

    // Periksa adanya garis batas irisan (Visible) dan garis arsir (Hatch)
    let has_visible = section_view.segments.iter().any(|s| s.kind == HlrLineKind::Visible);
    let has_hatch = section_view.segments.iter().any(|s| s.kind == HlrLineKind::Hatch);

    assert!(has_visible, "Section view harus memiliki garis batas solid");
    assert!(has_hatch, "Section view harus memiliki garis arsir miring 45° (Hatch)");

    // Periksa indikator garis potong panah A-A
    assert_eq!(indicator.label, "A");
    assert!((indicator.start[1] - 20.0).abs() < 1e-3, "Garis potong berada di Y=20mm");
    assert!(indicator.end[0] > indicator.start[0], "Rentang garis potong horizontal valid");
}

#[test]
fn test_iso_hatch_pattern_45_degree_even_odd() {
    use glam::vec2;

    // Poligon persegi [0, 100] x [0, 50]
    let segs = [
        [vec2(0.0, 0.0), vec2(100.0, 0.0)],
        [vec2(100.0, 0.0), vec2(100.0, 50.0)],
        [vec2(100.0, 50.0), vec2(0.0, 50.0)],
        [vec2(0.0, 50.0), vec2(0.0, 0.0)],
    ];

    let hatches = crate::section::generate_iso_hatch_pattern(
        &segs,
        vec2(0.0, 0.0),
        vec2(100.0, 50.0),
        5.0,
        45.0,
    );

    assert!(!hatches.is_empty(), "Harus menghasilkan garis arsir");
    for h in &hatches {
        assert_eq!(h.kind, HlrLineKind::Hatch);
        let dx = h.end[0] - h.start[0];
        let dy = h.end[1] - h.start[1];
        let angle_deg = (dy / dx).atan().to_degrees();
        assert!((angle_deg - 45.0).abs() < 1.0, "Kemiringan sudut arsir ~45°, dapat {angle_deg}°");
    }
}

#[test]
fn test_write_and_read_stl_shape() {
    let _lock = lock_test();
    let box_shape = extrude_profile(&rect_profile(20.0, 30.0), 40.0).unwrap();
    let path = std::env::temp_dir().join(format!("ducad-kernel-stl-test-{}.stl", std::process::id()));
    box_shape.write_stl(&path).unwrap();

    let loaded = KernelShape::read_stl(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    let mesh = loaded.tessellate();
    assert!(mesh.triangle_count() > 0, "Loaded STL should have triangles");
}

#[test]
fn test_extract_shape_edges_for_box() {
    let _lock = lock_test();
    let box_shape = extrude_profile(&rect_profile(20.0, 30.0), 40.0).unwrap();
    let edges = extract_shape_edges(&box_shape, None);
    // Kotak memiliki 12 rusuk tepi
    assert_eq!(edges.len(), 12, "Kotak harus menghasilkan tepat 12 garis tepi, dapat {}", edges.len());
}



/// BREP biner adalah jalur penyimpanan internal yang diusulkan menggantikan
/// teks STEP di file native `.ducad`. Test ini membuktikan dua hal yang
/// harus benar sebelum penggantian itu layak: roundtrip-nya menjaga
/// geometri, dan ukurannya memang jauh lebih kecil.
#[test]
fn brep_bytes_roundtrip_preserves_geometry() {
    let _guard = lock_test();
    let profile = rect_profile(40.0, 30.0);
    let shape = extrude_profile(&profile, 20.0).expect("extrude harus berhasil");
    let rounded = fillet_all(&shape, 3.0).expect("fillet harus berhasil");

    let bytes = rounded.to_brep_bytes().expect("to_brep_bytes harus berhasil");
    assert!(!bytes.is_empty(), "BREP tidak boleh kosong");

    let restored =
        KernelShape::from_brep_bytes(&bytes).expect("from_brep_bytes harus berhasil");

    let before = rounded.tessellate();
    let after = restored.tessellate();
    assert_eq!(
        before.positions.len(),
        after.positions.len(),
        "jumlah vertex harus identik setelah roundtrip"
    );
    assert_eq!(
        before.triangle_count(),
        after.triangle_count(),
        "jumlah segitiga harus identik setelah roundtrip"
    );
    assert_eq!(
        rounded.inner().faces().count(),
        restored.inner().faces().count(),
        "jumlah face harus identik setelah roundtrip"
    );
}

#[test]
fn brep_bytes_are_much_smaller_than_step_text() {
    let _guard = lock_test();
    let profile = rect_profile(40.0, 30.0);
    let shape = extrude_profile(&profile, 20.0).expect("extrude harus berhasil");
    let rounded = fillet_all(&shape, 3.0).expect("fillet harus berhasil");

    let brep = rounded.to_brep_bytes().expect("to_brep_bytes harus berhasil");
    let step = rounded.to_step_string().expect("to_step_string harus berhasil");

    // Angka persisnya tidak dikunci (bisa bergeser antar versi OCCT); yang
    // dikunci adalah KLAIM yang mendasari keputusan format file: BREP biner
    // secara substansial lebih kecil daripada teks STEP AP214.
    println!("BREP {} byte vs STEP {} byte", brep.len(), step.len());
    assert!(
        brep.len() * 2 < step.len(),
        "BREP ({} byte) seharusnya < separuh STEP ({} byte)",
        brep.len(),
        step.len()
    );
}

/// P1.4 — profil berlubang jadi solid dalam SATU operasi extrude, bukan
/// extrude lalu boolean subtract per lubang.
#[test]
fn extrude_profile_with_holes_produces_hollow_solid() {
    let _guard = lock_test();
    let plate = rect_profile(40.0, 40.0);
    // `rect_profile` membentang (0,0)..(w,h), jadi pusatnya (20,20) —
    // bukan titik asal.
    let hole = Profile::Circle {
        center: (20.0, 20.0),
        radius: 5.0,
    };
    let profile = plate.with_holes(vec![hole]);
    assert!(profile.has_holes());

    let solid = extrude_profile(&profile, 10.0).expect("extrude berlubang harus berhasil");
    let mesh = solid.tessellate();
    assert!(mesh.triangle_count() > 0);

    // Volume = (40x40 - pi*5^2) * 10, dicek terhadap volume B-rep EKSAK —
    // bukan volume mesh, yang menyimpang mengikuti kerapatan tesselasi
    // dinding lubang.
    let expected = (40.0 * 40.0 - std::f64::consts::PI * 25.0) * 10.0;
    let actual = solid.volume().abs();
    let rel_err = (actual - expected).abs() / expected;
    assert!(
        rel_err < 1e-6,
        "volume {actual:.3} mm^3 menyimpang {:.4}% dari {expected:.3} mm^3",
        rel_err * 100.0
    );

    // Solid tanpa lubang: 6 face. Dengan satu lubang silindris tembus:
    // 6 + dinding lubang. Jumlahnya harus BERTAMBAH — kalau lubangnya
    // diabaikan diam-diam, angka ini akan tetap 6.
    let solid_faces = extrude_profile(&rect_profile(40.0, 40.0), 10.0)
        .unwrap()
        .inner()
        .faces()
        .count();
    assert!(
        solid.inner().faces().count() > solid_faces,
        "lubang harus menambah face, bukan diabaikan"
    );
}

#[test]
fn profile_with_empty_holes_is_unchanged() {
    // `with_holes(vec![])` mengembalikan profil apa adanya supaya pemanggil
    // tidak perlu membedakan kasus "ternyata tidak ada lubang".
    let p = rect_profile(10.0, 10.0).with_holes(Vec::new());
    assert!(!p.has_holes());
    assert!(matches!(p, Profile::Loop(_)));
}

#[test]
fn nested_holes_are_rejected_rather_than_silently_wrong() {
    let _guard = lock_test();
    let inner = rect_profile(10.0, 10.0).with_holes(vec![Profile::Circle {
        center: (5.0, 5.0),
        radius: 2.0,
    }]);
    let bad = rect_profile(40.0, 40.0).with_holes(vec![inner]);
    assert!(
        extrude_profile(&bad, 5.0).is_err(),
        "profil berlubang bersarang harus ditolak eksplisit"
    );
}

// ---------------------------------------------------------------------
// P2.7 — validasi hasil operasi B-rep. P2.6 — mass properties.
// ---------------------------------------------------------------------

#[test]
fn boolean_and_fillet_results_are_validated() {
    let _guard = lock_test();
    let a = extrude_profile(&rect_profile(40.0, 40.0), 20.0).unwrap();
    let b = extrude_profile(&rect_profile(20.0, 20.0), 40.0).unwrap();

    // Jalur boolean/fillet kini melewati `validate_or_heal`; yang diuji di
    // sini adalah bahwa jalur normal TIDAK jadi menolak geometri yang sah.
    let fused = union(&a, &b).expect("union sah harus tetap berhasil");
    assert!(fused.is_valid(), "hasil union harus valid");

    let cut = subtract(&a, &b).expect("subtract sah harus tetap berhasil");
    assert!(cut.is_valid(), "hasil subtract harus valid");

    let rounded = fillet_all(&a, 2.0).expect("fillet sah harus tetap berhasil");
    assert!(rounded.is_valid(), "hasil fillet harus valid");
}

#[test]
fn fillet_radius_too_large_fails_instead_of_returning_broken_solid() {
    let _guard = lock_test();
    let box_shape = extrude_profile(&rect_profile(20.0, 20.0), 20.0).unwrap();
    // Radius jauh lebih besar dari setengah sisi terkecil: tidak ada solid
    // yang masuk akal. Yang penting ia GAGAL, bukan mengembalikan sesuatu
    // yang kelihatan benar di viewport lalu meledak saat ekspor STEP.
    let result = fillet_all(&box_shape, 50.0);
    if let Ok(shape) = result {
        assert!(
            shape.is_valid(),
            "kalau fillet dilaporkan berhasil, hasilnya WAJIB valid"
        );
    }
}

#[test]
fn surface_area_of_a_box_matches_analytic_value() {
    let _guard = lock_test();
    // Balok 40 x 30 x 20 mm: 2*(40*30 + 40*20 + 30*20) = 2*(1200+800+600).
    let solid = extrude_profile(&rect_profile(40.0, 30.0), 20.0).unwrap();
    let expected = 2.0 * (40.0 * 30.0 + 40.0 * 20.0 + 30.0 * 20.0);
    let actual = solid.surface_area();
    assert!(
        (actual - expected).abs() / expected < 1e-9,
        "luas {actual} != {expected}"
    );
}

#[test]
fn surface_area_increases_when_a_hole_is_added() {
    // Lubang tembus MENAMBAH luas permukaan (dinding silinder) sekaligus
    // MENGURANGI volume — dua arah yang berlawanan. Menguji keduanya
    // sekaligus memastikan lubangnya benar-benar terpotong, bukan sekadar
    // menghasilkan angka yang berubah.
    let _guard = lock_test();
    let plain = extrude_profile(&rect_profile(40.0, 40.0), 10.0).unwrap();
    let holed = extrude_profile(
        &rect_profile(40.0, 40.0).with_holes(vec![Profile::Circle {
            center: (20.0, 20.0),
            radius: 5.0,
        }]),
        10.0,
    )
    .unwrap();

    assert!(
        holed.surface_area() > plain.surface_area(),
        "dinding lubang harus menambah luas permukaan"
    );
    assert!(
        holed.volume().abs() < plain.volume().abs(),
        "lubang harus mengurangi volume"
    );
}

// ---------------------------------------------------------------------
// P2.1 — mode extrude (simetris, dua sisi, mundur).
// ---------------------------------------------------------------------

/// Rentang Z mesh sebuah shape — dipakai memverifikasi DI MANA material
/// berada, bukan sekadar berapa banyak.
fn z_range(shape: &KernelShape) -> (f32, f32) {
    let mesh = shape.tessellate();
    let (min, max) = mesh.bounding_box().expect("mesh tidak boleh kosong");
    (min[2], max[2])
}

#[test]
fn symmetric_extrude_straddles_the_sketch_plane() {
    let _guard = lock_test();
    let solid = extrude_profile_extent(
        &rect_profile(10.0, 10.0),
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        ExtrudeExtent::Symmetric(20.0),
    )
    .expect("extrude simetris harus berhasil");

    let (zmin, zmax) = z_range(&solid);
    assert!((zmin - -10.0).abs() < 1e-3, "zmin = {zmin}");
    assert!((zmax - 10.0).abs() < 1e-3, "zmax = {zmax}");
    // Volume total harus sama dengan blind sepanjang 20 — simetris hanya
    // memindahkan materialnya, tidak mengubah jumlahnya.
    assert!((solid.volume().abs() - 10.0 * 10.0 * 20.0).abs() < 1e-6);
}

#[test]
fn two_sided_extrude_uses_different_lengths_per_side() {
    let _guard = lock_test();
    let solid = extrude_profile_extent(
        &rect_profile(10.0, 10.0),
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        ExtrudeExtent::TwoSided {
            forward: 30.0,
            backward: 5.0,
        },
    )
    .unwrap();

    let (zmin, zmax) = z_range(&solid);
    assert!((zmin - -5.0).abs() < 1e-3, "zmin = {zmin}");
    assert!((zmax - 30.0).abs() < 1e-3, "zmax = {zmax}");
    assert!((solid.volume().abs() - 10.0 * 10.0 * 35.0).abs() < 1e-6);
}

#[test]
fn negative_blind_extrude_goes_backward_not_nowhere() {
    // `Blind` negatif harus menghasilkan material di sisi MUNDUR bidang,
    // bukan gagal atau menghasilkan solid bervolume negatif.
    let _guard = lock_test();
    let solid = extrude_profile_extent(
        &rect_profile(10.0, 10.0),
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        ExtrudeExtent::Blind(-15.0),
    )
    .unwrap();

    let (zmin, zmax) = z_range(&solid);
    assert!((zmin - -15.0).abs() < 1e-3, "zmin = {zmin}");
    assert!(zmax.abs() < 1e-3, "zmax = {zmax}");
    assert!((solid.volume().abs() - 10.0 * 10.0 * 15.0).abs() < 1e-6);
}

#[test]
fn zero_length_extent_is_rejected() {
    let _guard = lock_test();
    for extent in [
        ExtrudeExtent::Blind(0.0),
        ExtrudeExtent::Symmetric(0.0),
        ExtrudeExtent::TwoSided {
            forward: 5.0,
            backward: -5.0,
        },
    ] {
        assert!(
            extrude_profile_extent(
                &rect_profile(10.0, 10.0),
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
                extent,
            )
            .is_err(),
            "{extent:?} berpanjang total nol dan harus ditolak"
        );
    }
}

#[test]
fn extents_are_built_as_one_prism_without_internal_seam() {
    // Simetris diselesaikan dengan menggeser titik awal lalu satu extrude,
    // BUKAN dua extrude + union. Kalau ia memakai boolean, solid hasilnya
    // akan punya face sambungan di bidang sketsa sehingga jumlah face-nya
    // melebihi prisma biasa.
    let _guard = lock_test();
    let plain = extrude_profile(&rect_profile(10.0, 10.0), 20.0).unwrap();
    let symmetric = extrude_profile_extent(
        &rect_profile(10.0, 10.0),
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        ExtrudeExtent::Symmetric(20.0),
    )
    .unwrap();
    assert_eq!(
        symmetric.inner().faces().count(),
        plain.inner().faces().count(),
        "extrude simetris tidak boleh menyisakan face sambungan"
    );
}

// ---------------------------------------------------------------------
// P4.1 — HLR eksak.
// ---------------------------------------------------------------------

#[test]
fn exact_hlr_keeps_a_circle_as_a_circle() {
    // INTI P4.1. HLR berbasis mesh mengembalikan lingkaran sebagai poligon
    // puluhan sisi — bergerigi saat dicetak dan tidak bisa diberi dimensi
    // diameter yang benar. HLR eksak bekerja pada topologi B-rep, jadi
    // lingkarannya tetap kurva analitik.
    let _guard = lock_test();
    let cyl = extrude_profile(
        &Profile::Circle {
            center: (0.0, 0.0),
            radius: 10.0,
        },
        30.0,
    )
    .unwrap();

    // Pandang dari atas (-Z): tutup silinder menghadap kamera.
    let view = crate::hlr_exact::extract_exact_hlr(&cyl, (0.0, 0.0, -1.0), (0.0, 1.0, 0.0))
        .expect("HLR harus berhasil untuk silinder sederhana");

    assert!(!view.curves.is_empty(), "HLR tidak boleh kosong");
    assert!(
        view.analytic_count() > 0,
        "minimal satu kurva harus tetap analitik, bukan semuanya jadi garis"
    );
    assert!(
        view.curves
            .iter()
            .any(|c| matches!(c.curve, opencascade::primitives::EdgeType::Circle)),
        "tutup silinder harus muncul sebagai LINGKARAN, bukan rantai garis; \
         jenis kurva yang didapat: {:?}",
        view.curves.iter().map(|c| c.curve).collect::<Vec<_>>()
    );
}

#[test]
fn exact_hlr_separates_visible_from_hidden_edges() {
    // Balok dipandang dari depan: tiga rusuk belakang tertutup badan solid.
    // HLR mesh menentukannya lewat uji oklusi terhadap segitiga; HLR eksak
    // menentukannya dari topologi, jadi hasilnya tidak berubah-ubah
    // mengikuti kerapatan tesselasi.
    let _guard = lock_test();
    let solid = extrude_profile(&rect_profile(40.0, 30.0), 20.0).unwrap();

    let view = crate::hlr_exact::extract_exact_hlr(&solid, (0.0, 1.0, 0.0), (0.0, 0.0, 1.0))
        .expect("HLR harus berhasil untuk balok");

    assert!(view.visible().count() > 0, "harus ada rusuk yang terlihat");
    assert!(
        view.hidden().count() > 0,
        "balok pejal dipandang dari depan HARUS punya rusuk tersembunyi"
    );
}

#[test]
fn exact_hlr_rejects_up_vector_parallel_to_view() {
    // `gp_Ax2` menolak sumbu X yang sejajar normalnya. Disaring lebih awal
    // supaya jadi error Rust yang jelas, bukan lemparan C++ yang menembus
    // batas FFI.
    let _guard = lock_test();
    let solid = extrude_profile(&rect_profile(10.0, 10.0), 10.0).unwrap();
    assert!(
        crate::hlr_exact::extract_exact_hlr(&solid, (0.0, 0.0, 1.0), (0.0, 0.0, 1.0)).is_err(),
        "vektor atas sejajar arah pandang harus ditolak"
    );
    assert!(
        crate::hlr_exact::extract_exact_hlr(&solid, (0.0, 0.0, 0.0), (0.0, 1.0, 0.0)).is_err(),
        "arah pandang nol harus ditolak"
    );
}

#[test]
fn exact_hlr_is_independent_of_tessellation_density() {
    // Klaim kunci dibanding HLR berbasis mesh: mengubah kerapatan tesselasi
    // TIDAK boleh mengubah gambar tekniknya. Di sini di-mesh dengan
    // toleransi berbeda lebih dulu, lalu HLR dijalankan pada shape yang sama.
    let _guard = lock_test();
    let cyl = extrude_profile(
        &Profile::Circle {
            center: (0.0, 0.0),
            radius: 8.0,
        },
        20.0,
    )
    .unwrap();

    let before = crate::hlr_exact::extract_exact_hlr(&cyl, (0.0, 1.0, 0.0), (0.0, 0.0, 1.0))
        .unwrap()
        .curves
        .len();
    // Paksa tesselasi (mengisi triangulasi internal shape).
    let _ = cyl.tessellate();
    let after = crate::hlr_exact::extract_exact_hlr(&cyl, (0.0, 1.0, 0.0), (0.0, 0.0, 1.0))
        .unwrap()
        .curves
        .len();

    assert_eq!(
        before, after,
        "jumlah kurva HLR tidak boleh berubah karena tesselasi"
    );
}

// ---------------------------------------------------------------------
// P3.3 — mid-phase deteksi tabrakan.
// ---------------------------------------------------------------------

#[test]
fn interference_mid_phase_does_not_miss_full_containment() {
    // JEBAKAN YANG PALING BERBAHAYA di mid-phase berbasis segitiga: bodi
    // yang SEPENUHNYA berada di dalam bodi lain tidak punya satu pun
    // segitiga yang beririsan, padahal itu interferensi total. Tanpa
    // penjagaan containment, mid-phase akan menolaknya dan tabrakan itu
    // hilang dari laporan.
    let _guard = lock_test();
    let outer = extrude_profile(&rect_profile(100.0, 100.0), 100.0).unwrap();
    let inner_profile = Profile::Loop(vec![
        ducad_kernel_seg((40.0, 40.0), (60.0, 40.0)),
        ducad_kernel_seg((60.0, 40.0), (60.0, 60.0)),
        ducad_kernel_seg((60.0, 60.0), (40.0, 60.0)),
        ducad_kernel_seg((40.0, 60.0), (40.0, 40.0)),
    ]);
    let inner = crate::csg::extrude_profile_on_plane(
        &inner_profile,
        [0.0, 0.0, 40.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        20.0,
    )
    .unwrap();

    let name_a = "Outer".to_string();
    let name_b = "Inner".to_string();
    let clashes = detect_interference(&[(1, name_a, &outer), (2, name_b, &inner)], 0.001);
    assert_eq!(
        clashes.len(),
        1,
        "bodi yang tertelan seluruhnya HARUS tetap terdeteksi sebagai tabrakan"
    );
    assert!(clashes[0].volume > 7_000.0, "volume {}", clashes[0].volume);
}

#[test]
fn interference_mid_phase_rejects_overlapping_boxes_that_do_not_touch() {
    // Dua balok yang bounding box gabungannya tumpang tindih tapi
    // solid-nya tidak bersentuhan. Broad-phase AABB saja meloloskannya,
    // sehingga dulu tetap membayar operasi boolean penuh.
    let _guard = lock_test();
    let a = crate::csg::extrude_profile_on_plane(
        &rect_profile(20.0, 5.0),
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        5.0,
    )
    .unwrap();
    // Digeser sehingga AABB-nya beririsan di sumbu X, tapi terpisah di Y.
    let b = crate::csg::extrude_profile_on_plane(
        &rect_profile(5.0, 20.0),
        [10.0, 30.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        5.0,
    )
    .unwrap();

    let clashes = detect_interference(
        &[(1, "A".to_string(), &a), (2, "B".to_string(), &b)],
        0.001,
    );
    assert!(clashes.is_empty(), "tidak bersentuhan, tidak boleh ada clash");
}

#[test]
fn interference_still_detects_genuine_overlap() {
    // Regresi: mid-phase tidak boleh menghilangkan tabrakan yang nyata.
    let _guard = lock_test();
    let a = extrude_profile(&rect_profile(40.0, 40.0), 40.0).unwrap();
    let b = crate::csg::extrude_profile_on_plane(
        &rect_profile(40.0, 40.0),
        [20.0, 20.0, 20.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        40.0,
    )
    .unwrap();

    let clashes = detect_interference(
        &[(1, "A".to_string(), &a), (2, "B".to_string(), &b)],
        0.001,
    );
    assert_eq!(clashes.len(), 1, "tumpang tindih nyata harus terdeteksi");
    // Irisan 20x20x20 = 8000 mm^3.
    assert!(
        (clashes[0].volume - 8000.0).abs() / 8000.0 < 0.02,
        "volume tabrakan {}",
        clashes[0].volume
    );
}

fn ducad_kernel_seg(start: (f64, f64), end: (f64, f64)) -> ProfileSegment {
    ProfileSegment::Line { start, end }
}

// ---------------------------------------------------------------------
// P3.3 — clearance check.
// ---------------------------------------------------------------------

#[test]
fn clearance_measures_the_real_gap_between_two_bodies() {
    // Pertanyaan yang TIDAK bisa dijawab deteksi tabrakan: tabrakan hanya
    // melaporkan yang sudah saling menembus, sementara part berjarak 0,1 mm
    // lolos begitu saja padahal mustahil dirakit.
    let _guard = lock_test();
    let a = extrude_profile(&rect_profile(10.0, 10.0), 10.0).unwrap();
    let b = crate::csg::extrude_profile_on_plane(
        &rect_profile(10.0, 10.0),
        [25.0, 0.0, 0.0], // celah 15 mm dari x = 10 ke x = 25
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        10.0,
    )
    .unwrap();

    let r = crate::interference::check_clearance(&a, &b, 2.0).unwrap();
    assert!(
        (r.distance - 15.0).abs() < 1e-6,
        "jarak terukur {} mm, seharusnya 15",
        r.distance
    );
    assert!(r.passes, "15 mm harus lolos syarat 2 mm");

    // Syarat yang lebih ketat dari celah nyata harus GAGAL.
    let strict = crate::interference::check_clearance(&a, &b, 20.0).unwrap();
    assert!(!strict.passes, "15 mm tidak boleh lolos syarat 20 mm");
    assert!((strict.distance - 15.0).abs() < 1e-6, "jaraknya tetap sama");
}

#[test]
fn clearance_is_zero_for_touching_and_overlapping_bodies() {
    let _guard = lock_test();
    let a = extrude_profile(&rect_profile(10.0, 10.0), 10.0).unwrap();
    // Bersentuhan tepat di x = 10.
    let touching = crate::csg::extrude_profile_on_plane(
        &rect_profile(10.0, 10.0),
        [10.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        10.0,
    )
    .unwrap();
    let r = crate::interference::check_clearance(&a, &touching, 0.5).unwrap();
    assert!(r.distance < 1e-6, "bersentuhan harus berjarak nol");
    assert!(!r.passes, "celah nol tidak memenuhi syarat 0,5 mm");
}

#[test]
fn clearance_on_curved_surfaces_uses_exact_geometry() {
    // Titik utama memakai BRepExtrema alih-alih jarak antar mesh: pada
    // permukaan lengkung, jarak antar titik tesselasi selalu sedikit
    // MELEBIHI jarak permukaan sesungguhnya. Dua silinder R5 yang pusatnya
    // berjarak 30 mm punya celah tepat 20 mm.
    let _guard = lock_test();
    let a = extrude_profile(
        &Profile::Circle {
            center: (0.0, 0.0),
            radius: 5.0,
        },
        10.0,
    )
    .unwrap();
    let b = extrude_profile(
        &Profile::Circle {
            center: (30.0, 0.0),
            radius: 5.0,
        },
        10.0,
    )
    .unwrap();

    let r = crate::interference::check_clearance(&a, &b, 1.0).unwrap();
    assert!(
        (r.distance - 20.0).abs() < 1e-6,
        "celah antar silinder {} mm, seharusnya tepat 20",
        r.distance
    );
}

// ---------------------------------------------------------------------
// Regresi: translasi harus KUMULATIF, bukan absolut.
// ---------------------------------------------------------------------

#[test]
fn translating_twice_accumulates_instead_of_resetting() {
    // `set_global_translation` yang lama MENGATUR Location secara absolut:
    // geser (10,0,0) lalu geser (5,0,0) berakhir di x=5, bukan x=15. Drag
    // body dua kali mereset drag pertama, dan solver perakitan menempatkan
    // part di posisi yang salah pada setiap solve sesudah yang pertama.
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(10.0, 10.0), 10.0).unwrap();
    let once = translate_shape(&shape, 10.0, 0.0, 0.0).unwrap();
    let twice = translate_shape(&once, 5.0, 0.0, 0.0).unwrap();

    let cx = |s: &KernelShape| s.tessellate().center()[0];
    assert!((cx(&shape) - 5.0).abs() < 1e-3);
    assert!((cx(&once) - 15.0).abs() < 1e-3, "sekali: {}", cx(&once));
    assert!((cx(&twice) - 20.0).abs() < 1e-3, "dua kali harus kumulatif: {}", cx(&twice));
}

#[test]
fn transform_after_translate_composes_with_existing_position() {
    // Jalur yang dipakai `apply_mate_transform_to_shape`: koreksi solver
    // diterapkan pada geometri yang SUDAH berpindah.
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(10.0, 10.0), 10.0).unwrap();
    let moved = translate_shape(&shape, 5.0, 7.0, 30.0).unwrap();
    let corrected = crate::shape::transform_shape(
        &moved,
        (-5.0, -7.0, 0.0),
        (5.0, 7.0, 30.0),
        (0.0, 0.0, 1.0),
        0.0,
    )
    .unwrap();
    let c = corrected.tessellate().center();
    assert!((c[0] - 5.0).abs() < 1e-3 && (c[1] - 5.0).abs() < 1e-3, "x,y kembali ke asal: {c:?}");
    assert!((c[2] - 35.0).abs() < 1e-3, "z tetap 30 + 5: {c:?}");
}

// ---------------------------------------------------------------------
// P0.4 — enumerasi topologi + operasi berbasis indeks
// ---------------------------------------------------------------------

fn box_60_40_8() -> KernelShape {
    extrude_profile(&rect_profile(60.0, 40.0), 8.0).unwrap()
}

fn box_with_hole() -> KernelShape {
    let profile = rect_profile(60.0, 40.0).with_holes(vec![Profile::Circle {
        center: (30.0, 20.0),
        radius: 2.75,
    }]);
    extrude_profile(&profile, 8.0).unwrap()
}

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[test]
fn topo_box_faces_outward_and_area() {
    let _guard = lock_test();
    let shape = box_60_40_8();
    let faces = topo::enumerate_faces(&shape);
    assert_eq!(faces.len(), 6);
    let center = [30.0, 20.0, 4.0];
    let mut total = 0.0;
    for f in &faces {
        assert_eq!(f.kind, SurfaceKind::Plane);
        assert_eq!(f.concave, None);
        let rel = [f.centroid[0] - center[0], f.centroid[1] - center[1], f.centroid[2] - center[2]];
        assert!(dot3(f.normal, rel) > 0.0, "normal face {} harus keluar: {:?} @ {:?}", f.index, f.normal, f.centroid);
        total += f.area;
    }
    let sa = shape.surface_area();
    assert!((total - sa).abs() / sa < 1e-6, "{total} vs {sa}");
}

#[test]
fn topo_box_has_twelve_unique_line_edges() {
    let _guard = lock_test();
    let edges = topo::enumerate_edges(&box_60_40_8());
    assert_eq!(edges.len(), 12);
    assert!(edges.iter().all(|e| e.kind == EdgeKind::Line));
    let vertical = edges
        .iter()
        .filter(|e| e.dir.is_some_and(|d| (d[2] - 1.0).abs() < 1e-9))
        .count();
    assert_eq!(vertical, 4);
}

#[test]
fn topo_cylinder_face_and_circle_edges() {
    let _guard = lock_test();
    let shape = extrude_profile(&Profile::Circle { center: (0.0, 0.0), radius: 10.0 }, 20.0).unwrap();
    let faces = topo::enumerate_faces(&shape);
    let cyl: Vec<_> = faces.iter().filter(|f| f.kind == SurfaceKind::Cylinder).collect();
    assert_eq!(cyl.len(), 1);
    assert!((cyl[0].radius.unwrap() - 10.0).abs() < 1e-6);
    let axis_dir = cyl[0].axis.unwrap().1;
    assert!((axis_dir[2].abs() - 1.0).abs() < 1e-9, "{axis_dir:?}");
    assert_eq!(cyl[0].concave, Some(false));
    let circles: Vec<_> = topo::enumerate_edges(&shape)
        .into_iter()
        .filter(|e| e.kind == EdgeKind::Circle)
        .collect();
    assert_eq!(circles.len(), 2);
    for c in circles {
        assert!((c.radius.unwrap() - 10.0).abs() < 1e-3, "{:?}", c.radius);
    }
}

#[test]
fn topo_hole_is_concave_and_top_boundary() {
    let _guard = lock_test();
    let shape = box_with_hole();
    let faces = topo::enumerate_faces(&shape);
    let cyl: Vec<_> = faces.iter().filter(|f| f.kind == SurfaceKind::Cylinder).collect();
    assert!(!cyl.is_empty());
    assert!(cyl.iter().all(|f| f.concave == Some(true)), "{:?}", cyl.iter().map(|f| f.concave).collect::<Vec<_>>());
    let top = faces
        .iter()
        .find(|f| f.kind == SurfaceKind::Plane && f.normal[2] > 0.9)
        .expect("face +Z");
    assert!(top.boundary.len() >= 4);
    assert!(top.boundary.iter().all(|p| (p[2] - 8.0).abs() < 1e-6));
}

#[test]
fn fillet_by_index_vertical_edges_volume() {
    let _guard = lock_test();
    let shape = box_60_40_8();
    let idx: Vec<usize> = topo::enumerate_edges(&shape)
        .iter()
        .filter(|e| e.dir.is_some_and(|d| (d[2] - 1.0).abs() < 1e-9))
        .map(|e| e.index)
        .collect();
    assert_eq!(idx.len(), 4);
    let out = fillet_edges_by_index(&shape, 3.0, &idx).unwrap();
    assert!(out.is_valid());
    let expected = 60.0 * 40.0 * 8.0 - 4.0 * (1.0 - std::f64::consts::PI / 4.0) * 9.0 * 8.0;
    let v = out.volume().abs();
    assert!((v - expected).abs() / expected < 0.005, "{v} vs {expected}");
}

#[test]
fn shell_by_index_top_face_volume() {
    let _guard = lock_test();
    let shape = box_60_40_8();
    let top = topo::enumerate_faces(&shape)
        .into_iter()
        .find(|f| f.normal[2] > 0.9)
        .unwrap()
        .index;
    let out = shell_faces_by_index(&shape, 2.0, &[top]).unwrap();
    assert!(out.is_valid());
    let expected = 60.0 * 40.0 * 8.0 - 56.0 * 36.0 * 6.0;
    let v = out.volume().abs();
    assert!((v - expected).abs() / expected < 0.01, "{v} vs {expected}");
}

#[test]
fn by_index_out_of_range_errors() {
    let _guard = lock_test();
    let shape = box_60_40_8();
    assert!(fillet_edges_by_index(&shape, 1.0, &[12]).is_err());
    assert!(chamfer_edges_by_index(&shape, 1.0, &[99]).is_err());
    assert!(shell_faces_by_index(&shape, 1.0, &[6]).is_err());
    assert!(fillet_edges_by_index(&shape, 1.0, &[]).is_err());
}

// ---------------------------------------------------------------------
// P0.5 — primitif solid
// ---------------------------------------------------------------------

fn rel_close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() / b.abs() < tol
}

#[test]
fn primitives_volumes_match_analytic() {
    let _guard = lock_test();
    use std::f64::consts::PI;
    let b = make_box(60.0, 40.0, 8.0, false).unwrap();
    assert!(b.is_valid());
    assert!(rel_close(b.volume().abs(), 60.0 * 40.0 * 8.0, 1e-6));
    let bc = make_box(10.0, 20.0, 30.0, true).unwrap();
    assert!(rel_close(bc.volume().abs(), 6000.0, 1e-6));

    let c = make_cylinder(10.0, 20.0).unwrap();
    assert!(c.is_valid());
    assert!(rel_close(c.volume().abs(), PI * 100.0 * 20.0, 1e-3));

    let s = make_sphere(5.0).unwrap();
    assert!(s.is_valid());
    assert!(rel_close(s.volume().abs(), 4.0 / 3.0 * PI * 125.0, 1e-3));

    let (r1, r2, h) = (10.0, 4.0, 12.0);
    let k = make_cone(r1, r2, h).unwrap();
    assert!(k.is_valid());
    assert!(rel_close(k.volume().abs(), PI * h / 3.0 * (r1 * r1 + r1 * r2 + r2 * r2), 1e-3));
    let tip = make_cone(10.0, 0.0, 12.0).unwrap();
    assert!(rel_close(tip.volume().abs(), PI * 12.0 / 3.0 * 100.0, 1e-3));
}

#[test]
fn make_box_mesh_bbox_starts_at_origin() {
    let _guard = lock_test();
    let (min, max) = make_box(60.0, 40.0, 8.0, false).unwrap().tessellate().bounding_box().unwrap();
    for (got, want) in min.iter().zip([0.0, 0.0, 0.0]).chain(max.iter().zip([60.0, 40.0, 8.0])) {
        assert!((*got as f64 - want).abs() < 1e-3, "{min:?} {max:?}");
    }
}

#[test]
fn primitives_reject_non_positive() {
    let _guard = lock_test();
    assert!(make_box(0.0, 1.0, 1.0, false).is_err());
    assert!(make_box(1.0, -1.0, 1.0, true).is_err());
    assert!(make_cylinder(0.0, 5.0).is_err());
    assert!(make_cylinder(5.0, -1.0).is_err());
    assert!(make_sphere(0.0).is_err());
    assert!(make_cone(0.0, 1.0, 1.0).is_err());
    assert!(make_cone(1.0, -1.0, 1.0).is_err());
    assert!(make_cone(1.0, 0.5, 0.0).is_err());
}

// ---------------------------------------------------------------------
// P7.3 — tebal dinding minimum berbasis mesh
// ---------------------------------------------------------------------

#[test]
fn wall_thickness_of_plate_is_its_height() {
    let _guard = lock_test();
    let mesh = box_60_40_8().tessellate();
    let r = min_wall_thickness(&mesh, DEFAULT_WALL_SAMPLES).unwrap();
    assert!((r.min - 8.0).abs() <= 0.05, "{r:?}");
    assert!(r.samples > 0);
    let again = min_wall_thickness(&mesh, DEFAULT_WALL_SAMPLES).unwrap();
    assert_eq!(r, again, "hasil harus deterministik");
}

#[test]
fn wall_thickness_of_shelled_box() {
    let _guard = lock_test();
    let shape = box_60_40_8();
    let top = topo::enumerate_faces(&shape)
        .into_iter()
        .find(|f| f.normal[2] > 0.9)
        .unwrap()
        .index;
    let shelled = shell_faces_by_index(&shape, 2.0, &[top]).unwrap();
    let r = min_wall_thickness(&shelled.tessellate(), DEFAULT_WALL_SAMPLES).unwrap();
    assert!((r.min - 2.0).abs() <= 0.1, "{r:?}");
}

#[test]
fn wall_thickness_next_to_hole() {
    let _guard = lock_test();
    // Lubang r=2.75 berpusat 5 mm dari tepi x=0 → dinding 2.25 mm.
    let profile = rect_profile(60.0, 40.0).with_holes(vec![Profile::Circle {
        center: (5.0, 20.0),
        radius: 2.75,
    }]);
    let shape = extrude_profile(&profile, 8.0).unwrap();
    let r = min_wall_thickness(&shape.tessellate(), DEFAULT_WALL_SAMPLES).unwrap();
    assert!((r.min - 2.25).abs() <= 0.15, "{r:?}");
    assert!(r.at[0] < 5.0, "lokasi minimum harus di antara lubang dan tepi: {:?}", r.at);
}

/// Dua pelat sejajar sejarak 3 mm, total ~100k segitiga.
fn two_plates_mesh(cells: usize) -> KernelMesh {
    let mut m = KernelMesh::default();
    for (z, nz) in [(0.0f32, -1.0f32), (3.0, 1.0)] {
        let base = m.positions.len() as u32;
        for j in 0..=cells {
            for i in 0..=cells {
                m.positions.push([i as f32 * 0.5, j as f32 * 0.5, z]);
                m.normals.push([0.0, 0.0, nz]);
            }
        }
        let w = (cells + 1) as u32;
        for j in 0..cells as u32 {
            for i in 0..cells as u32 {
                let a = base + j * w + i;
                // Winding CCW dilihat dari luar: pelat bawah (normal −Z) dibalik.
                if nz > 0.0 {
                    m.indices.extend([a, a + 1, a + w, a + 1, a + w + 1, a + w]);
                } else {
                    m.indices.extend([a, a + w, a + 1, a + 1, a + w, a + w + 1]);
                }
            }
        }
    }
    m
}

#[test]
fn wall_thickness_synthetic_plates() {
    let r = min_wall_thickness(&two_plates_mesh(20), DEFAULT_WALL_SAMPLES).unwrap();
    assert!((r.min - 3.0).abs() < 1e-3, "{r:?}");
}

/// Jalankan dengan `cargo test --release -p ducad-kernel -- --ignored wall_thickness_100k`.
#[test]
#[ignore]
fn wall_thickness_100k_triangles_under_2s() {
    let mesh = two_plates_mesh(158); // 2 × 158² × 2 ≈ 99 856 segitiga
    assert!(mesh.triangle_count() > 99_000);
    let t0 = std::time::Instant::now();
    let r = min_wall_thickness(&mesh, DEFAULT_WALL_SAMPLES).unwrap();
    let dt = t0.elapsed();
    eprintln!("min_wall 100k segitiga: {dt:?} ({} sampel)", r.samples);
    assert!(dt.as_secs_f64() < 2.0, "{dt:?}");
}

#[test]
fn ray_hit_distance_through_plate() {
    let _guard = lock_test();
    let mesh = box_60_40_8().tessellate();
    let d = crate::ray_hit_distance(&mesh, [30.0, 20.0, 7.999], [0.0, 0.0, -1.0]).unwrap();
    assert!((d - 7.999).abs() < 1e-3, "{d}");
    assert!(crate::ray_hit_distance(&mesh, [30.0, 20.0, 20.0], [0.0, 0.0, 1.0]).is_none());
}

/// Konstanta baku aproksimasi seperempat lingkaran dengan Bézier kubik:
/// `4/3 · (√2 − 1)`. Galatnya ~0,027% dari radius — jauh di bawah toleransi
/// manufaktur, dan itulah cara font sendiri menggambar bentuk bundar.
const KAPPA: f64 = 0.552_284_749_830_793_4;

/// Lingkaran radius `r` berpusat di origin, dirakit dari EMPAT Bézier kubik
/// (satu per kuadran, berlawanan arah jarum jam).
fn bezier_circle_loop(r: f64) -> Vec<ProfileSegment> {
    let k = r * KAPPA;
    vec![
        ProfileSegment::Bezier {
            start: (r, 0.0),
            c1: (r, k),
            c2: (k, r),
            end: (0.0, r),
        },
        ProfileSegment::Bezier {
            start: (0.0, r),
            c1: (-k, r),
            c2: (-r, k),
            end: (-r, 0.0),
        },
        ProfileSegment::Bezier {
            start: (-r, 0.0),
            c1: (-r, -k),
            c2: (-k, -r),
            end: (0.0, -r),
        },
        ProfileSegment::Bezier {
            start: (0.0, -r),
            c1: (k, -r),
            c2: (r, -k),
            end: (r, 0.0),
        },
    ]
}

/// Lingkaran radius `r` sebagai poliline `segments` ruas lurus — pembanding
/// yang mewakili perilaku LAMA (kurva font dicacah jadi garis).
fn polyline_circle_loop(r: f64, segments: usize) -> Vec<ProfileSegment> {
    (0..segments)
        .map(|i| {
            let a0 = std::f64::consts::TAU * (i as f64) / (segments as f64);
            let a1 = std::f64::consts::TAU * ((i + 1) as f64) / (segments as f64);
            ProfileSegment::Line {
                start: (r * a0.cos(), r * a0.sin()),
                end: (r * a1.cos(), r * a1.sin()),
            }
        })
        .collect()
}

#[test]
fn extrude_bezier_loop_is_valid_and_matches_circle_volume() {
    let _guard = lock_test();
    let (r, h) = (10.0, 5.0);
    let shape = extrude_profile(&Profile::Loop(bezier_circle_loop(r)), h)
        .expect("extrude loop Bézier harus berhasil");

    assert!(shape.is_valid(), "solid hasil extrude Bézier harus valid");

    let expected = std::f64::consts::PI * r * r * h;
    let got = shape.volume();
    assert!(
        (got - expected).abs() / expected < 0.01,
        "volume {got} menyimpang >1% dari lingkaran ideal {expected}"
    );
}

/// Inti perbaikan "teks patah-patah": kurva yang masuk sebagai Bézier jadi
/// SATU face melengkung per segmen, sementara kurva yang dicacah jadi garis
/// menghasilkan satu face datar per ruas. Perbandingan langsung jumlah face
/// di bawah ini yang membedakan dinding mulus dari dinding bersegi.
#[test]
fn bezier_wall_has_far_fewer_faces_than_polyline_wall() {
    let _guard = lock_test();
    let (r, h) = (10.0, 5.0);

    let smooth = extrude_profile(&Profile::Loop(bezier_circle_loop(r)), h).unwrap();
    let faceted = extrude_profile(&Profile::Loop(polyline_circle_loop(r, 40)), h).unwrap();

    let smooth_faces = enumerate_faces(&smooth).len();
    let faceted_faces = enumerate_faces(&faceted).len();

    // 4 dinding Bézier + tutup atas + tutup bawah.
    assert_eq!(smooth_faces, 6, "dinding Bézier harus 4 face + 2 tutup");
    assert!(
        faceted_faces > smooth_faces * 5,
        "pembanding poliline seharusnya jauh lebih banyak face: {faceted_faces} vs {smooth_faces}"
    );
}

/// Huruf berongga seperti "O" bergantung pada batas DALAM yang berorientasi
/// terbalik. Bézier punya titik kontrol yang ikut harus bertukar saat dibalik,
/// jadi jalur itu diuji terpisah dari Line/Arc.
#[test]
fn bezier_hole_removes_volume_from_plate() {
    let _guard = lock_test();
    let (w, h, t, r) = (40.0, 40.0, 10.0, 5.0);

    let hole = Profile::Loop(
        bezier_circle_loop(r)
            .into_iter()
            .map(|seg| match seg {
                // Geser lubang ke tengah plat.
                ProfileSegment::Bezier { start, c1, c2, end } => {
                    let shift = |p: (f64, f64)| (p.0 + w / 2.0, p.1 + h / 2.0);
                    ProfileSegment::Bezier {
                        start: shift(start),
                        c1: shift(c1),
                        c2: shift(c2),
                        end: shift(end),
                    }
                }
                other => other,
            })
            .collect(),
    );

    let plate = rect_profile(w, h).with_holes(vec![hole]);
    let shape = extrude_profile(&plate, t).expect("extrude plat berlubang Bézier harus berhasil");

    assert!(shape.is_valid());
    let expected = w * h * t - std::f64::consts::PI * r * r * t;
    let got = shape.volume();
    assert!(
        (got - expected).abs() / expected < 0.01,
        "volume {got} != plat berlubang {expected} — cek orientasi wire lubang"
    );
}

// --- P14: operasi lanjutan untuk agent -----------------------------------

fn xy_section(profile: Profile, z: f64) -> LoftSection {
    LoftSection {
        profile,
        origin: [0.0, 0.0, z],
        u_axis: [1.0, 0.0, 0.0],
        v_axis: [0.0, 1.0, 0.0],
        normal: [0.0, 0.0, 1.0],
    }
}

fn mesh_bbox(s: &KernelShape) -> ([f32; 3], [f32; 3]) {
    s.tessellate().bounding_box().expect("mesh tidak kosong")
}

#[test]
fn loft_sections_prism_and_frustum() {
    let _l = lock_test();
    let prism = loft_sections(&[xy_section(rect_profile(10.0, 10.0), 0.0), xy_section(rect_profile(10.0, 10.0), 5.0)]).unwrap();
    assert!(prism.is_valid());
    assert!((prism.volume().abs() - 500.0).abs() < 1e-3, "{}", prism.volume());
    let circle = |r: f64| Profile::Circle { center: (0.0, 0.0), radius: r };
    let frustum = loft_sections(&[xy_section(circle(10.0), 0.0), xy_section(circle(5.0), 12.0)]).unwrap();
    let expected = std::f64::consts::PI * 12.0 / 3.0 * (100.0 + 50.0 + 25.0);
    assert!((frustum.volume().abs() - expected).abs() / expected < 1e-3, "{}", frustum.volume());
    assert!(loft_sections(&[xy_section(circle(1.0), 0.0)]).is_err());
}

#[test]
fn mirror_shape_reflects_across_plane() {
    let _l = lock_test();
    let b = make_box(10.0, 10.0, 10.0, false).unwrap();
    let b = translate_shape(&b, 10.0, 0.0, 0.0).unwrap();
    let m = mirror_shape(&b, dvec3(0.0, 0.0, 0.0), dvec3(1.0, 0.0, 0.0)).unwrap();
    assert!(m.is_valid());
    assert!((m.volume().abs() - 1000.0).abs() < 1e-6);
    let (min, max) = mesh_bbox(&m);
    assert!((min[0] + 20.0).abs() < 1e-4 && (max[0] + 10.0).abs() < 1e-4, "{min:?} {max:?}");
    assert!(min[1].abs() < 1e-4 && (max[1] - 10.0).abs() < 1e-4, "Y tidak berubah: {min:?} {max:?}");
    assert!(mirror_shape(&b, DVec3::ZERO, DVec3::ZERO).is_err());
}

#[test]
fn draft_and_variable_fillet_by_index() {
    let _l = lock_test();
    let b = make_box(20.0, 20.0, 10.0, false).unwrap();
    let sides: Vec<usize> = enumerate_faces(&b)
        .iter()
        .filter(|f| f.normal[2].abs() < 1e-6)
        .map(|f| f.index)
        .collect();
    assert_eq!(sides.len(), 4);
    let d = draft_faces_by_index(&b, &sides, DVec3::ZERO, DVec3::Z, DVec3::Z, 5.0).unwrap();
    assert!(d.is_valid());
    assert!(d.volume().abs() < 4000.0 - 1.0, "draft ke dalam mengurangi volume: {}", d.volume());
    assert!(draft_faces_by_index(&b, &sides, DVec3::ZERO, DVec3::Z, DVec3::Z, 95.0).is_err());

    let verticals: Vec<usize> = enumerate_edges(&b)
        .iter()
        .filter(|e| e.dir.is_some_and(|d| d[2].abs() > 0.99))
        .map(|e| e.index)
        .collect();
    assert_eq!(verticals.len(), 4);
    let f = fillet_edges_variable_by_index(&b, 1.0, 3.0, &verticals).unwrap();
    assert!(f.is_valid());
    let v = f.volume().abs();
    assert!(v < 4000.0 && v > 3900.0, "{v}");
    assert!(fillet_edges_variable_by_index(&b, 0.0, 1.0, &verticals).is_err());
}

#[test]
fn helix_spring_has_real_volume() {
    let _l = lock_test();
    let params = HelixParams {
        radius: 10.0,
        pitch: 4.0,
        turns: 3.0,
        ..HelixParams::default()
    };
    let spring = create_helix_solid(&params, HelixProfileKind::Circle { radius: 1.0 }, 36).unwrap();
    let length = 3.0 * ((2.0 * std::f64::consts::PI * 10.0f64).powi(2) + 16.0).sqrt();
    let expected = std::f64::consts::PI * length;
    let v = spring.volume().abs();
    assert!(spring.is_valid(), "helix harus solid valid");
    assert!((v - expected).abs() / expected < 0.03, "volume pegas {v} vs {expected}");
}

/// Highlight face terpilih harus eksak: `face_index` hasil pick menunjuk
/// rentang `face_ranges` yang SEMUA segitiganya berada di face itu — bukan
/// tebakan radius di sekitar titik klik (dulu memberi sorotan gradasi yang
/// merembes ke face tetangga).
#[test]
fn test_pick_face_index_maps_to_exact_mesh_triangles() {
    let _guard = lock_test();
    let shape = extrude_profile(&rect_profile(200.0, 100.0), 20.0).unwrap();
    let mesh = shape.tessellate();
    assert_eq!(mesh.face_ranges.len(), 6, "kotak punya 6 face");
    assert_eq!(
        mesh.face_ranges.last().map(|r| r.end as usize),
        Some(mesh.indices.len()),
        "rentang face harus menutup seluruh indices"
    );

    // Ray dari atas mengenai face atas (z = 20).
    let ray = PickRay { origin: (50.0, 50.0, 500.0), dir: (0.0, 0.0, -1.0) };
    let hit = pick_face_details(&shape, ray).expect("ray harus kena face atas");
    let fi = hit.face_index.expect("face_index harus ditemukan");
    let range = mesh.face_ranges[fi].clone();
    assert!(range.end > range.start, "face atas harus punya segitiga");
    for &vi in &mesh.indices[range.start as usize..range.end as usize] {
        let z = mesh.positions[vi as usize][2];
        assert!((z - 20.0).abs() < 1e-3, "vertex di luar face atas: z = {z}");
    }

    // Gizmo push/pull berada di titik klik, bukan di centroid batas.
    let anchor = hit.gizmo_anchor();
    assert!((anchor.0 - 50.0).abs() < 1e-6 && (anchor.1 - 50.0).abs() < 1e-6);
}

// ── P16: properti massa ────────────────────────────────────────────────

fn assert_rel(actual: f64, expected: f64, tol: f64, what: &str) {
    let denom = expected.abs().max(1e-12);
    assert!(
        ((actual - expected) / denom).abs() < tol,
        "{what}: {actual} != {expected}"
    );
}

#[test]
fn mass_properties_box_matches_closed_form() {
    let _l = lock_test();
    let (a, b, c) = (10.0, 20.0, 30.0);
    let shape = make_box(a, b, c, false).unwrap();
    let mp = shape.mass_properties();
    let v = a * b * c;
    assert_rel(mp.volume_mm3, v, 1e-9, "volume");
    for (got, want) in mp.centroid.iter().zip([5.0, 10.0, 15.0]) {
        assert_rel(*got, want, 1e-9, "centroid");
    }
    let i = mp.inertia_com();
    assert_rel(i[0][0], v * (b * b + c * c) / 12.0, 1e-6, "Ixx");
    assert_rel(i[1][1], v * (a * a + c * c) / 12.0, 1e-6, "Iyy");
    assert_rel(i[2][2], v * (a * a + b * b) / 12.0, 1e-6, "Izz");
    for (r, s) in [(0, 1), (0, 2), (1, 2)] {
        assert!(i[r][s].abs() < 1e-6 * i[0][0], "produk inersia {r}{s}: {}", i[r][s]);
    }
    // Terhadap origin: teorema sumbu sejajar, termasuk produk inersia.
    assert_rel(mp.inertia_origin[0][0], v * (b * b + c * c) / 3.0, 1e-6, "Ixx origin");
    assert_rel(mp.inertia_origin[0][1], -v * (a / 2.0) * (b / 2.0), 1e-6, "Ixy origin");
}

#[test]
fn mass_properties_sphere_has_equal_principal_moments() {
    let _l = lock_test();
    let r = 7.0;
    let mp = make_sphere(r).unwrap().mass_properties();
    let v = 4.0 / 3.0 * std::f64::consts::PI * r.powi(3);
    assert_rel(mp.volume_mm3, v, 1e-6, "volume");
    let (m, _) = mp.principal();
    for k in m {
        assert_rel(k, 0.4 * v * r * r, 1e-6, "momen bola");
    }
}

#[test]
fn mass_properties_cylinder_principal_axis_is_cylinder_axis() {
    let _l = lock_test();
    let (r, h) = (4.0, 50.0);
    let upright = make_cylinder(r, h).unwrap();
    // Miringkan supaya sumbu utama tidak kebetulan sejajar sumbu global.
    let ang = 30.0_f64.to_radians();
    let tilted = rotate_shape(&upright, (0.0, 0.0, 0.0), (1.0, 0.0, 0.0), ang).unwrap();
    let mp = tilted.mass_properties();
    let v = std::f64::consts::PI * r * r * h;
    let (m, ax) = mp.principal();
    // Silinder ramping: momen terkecil di sumbu silinder.
    assert_rel(m[0], 0.5 * v * r * r, 1e-6, "momen aksial");
    assert_rel(m[1], v * (3.0 * r * r + h * h) / 12.0, 1e-6, "momen lateral");
    assert_rel(m[2], m[1], 1e-6, "momen lateral kembar");
    let expect = [0.0, -ang.sin(), ang.cos()];
    let dot: f64 = ax[0].iter().zip(expect).map(|(p, q)| p * q).sum();
    assert!(dot.abs() > 1.0 - 1e-9, "sumbu utama {:?} vs {:?}", ax[0], expect);
}

// ── P20: ulir fisik ISO ────────────────────────────────────────────────

#[test]
fn thread_iso_m10_removes_profile_volume() {
    let _l = lock_test();
    let (d, pitch, length) = (10.0, 1.5, 20.0);
    let rod = make_cylinder(d / 2.0, 30.0).unwrap();
    let plain = rod.volume().abs();
    let threaded = cut_iso_thread(&rod, [0.0, 0.0, 5.0], [0.0, 0.0, 1.0], d, pitch, length, false)
        .unwrap();
    assert!(threaded.is_valid(), "solid berulir harus valid");
    let removed = plain - threaded.volume().abs();
    let expected = IsoThreadProfile::new(pitch).removed_volume(d, length);
    // Profil dasar ISO 68-1: 0.3045·P² per penampang.
    assert_rel(IsoThreadProfile::new(pitch).area(), 0.304_46 * pitch * pitch, 1e-3, "luas profil");
    assert_rel(removed, expected, 0.02, "volume alur ulir");

    // Ulir kiri: volume sama, bentuk berbeda (cermin).
    let left = cut_iso_thread(&rod, [0.0, 0.0, 5.0], [0.0, 0.0, 1.0], d, pitch, length, true)
        .unwrap();
    assert!(left.is_valid());
    assert_rel(plain - left.volume().abs(), expected, 0.02, "volume ulir kiri");

    // Masukan tak masuk akal ditolak, bukan panik.
    assert!(cut_iso_thread(&rod, [0.0; 3], [0.0, 0.0, 1.0], d, 20.0, 40.0, false).is_err());
    assert!(cut_iso_thread(&rod, [0.0; 3], [0.0, 0.0, 1.0], d, pitch, 0.5, false).is_err());
    assert!(cut_iso_thread(&rod, [0.0; 3], [0.0, 0.0, 1.0], -1.0, pitch, 5.0, false).is_err());
}
