use super::*;
use glam::dvec3;
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
    };
    let b = KernelMesh {
        positions: vec![[2.0, 0.0, 0.0], [3.0, 0.0, 0.0], [2.0, 1.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        indices: vec![0, 1, 2],
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
    println!("ORIG FACES: {}, NEW FACES: {}", orig_faces, new_faces);

    let mesh = split.tessellate();
    assert!(mesh.triangle_count() > 0);
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


