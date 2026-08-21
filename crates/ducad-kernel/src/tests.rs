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
    let _guard = TEST_LOCK.lock().unwrap();
    let shape = extrude_profile(&rect_profile(40.0, 30.0), 20.0).unwrap();
    let mesh = shape.tessellate();
    assert!(mesh.triangle_count() > 0);
    assert!(!mesh.positions.is_empty());
}

#[test]
fn extrude_circle_produces_cylinder_mesh() {
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
    assert!(extrude_profile(&Profile::Loop(vec![]), 10.0).is_err());
}

#[test]
fn extrude_zero_distance_errors() {
    let _guard = TEST_LOCK.lock().unwrap();
    assert!(extrude_profile(&rect_profile(10.0, 10.0), 0.0).is_err());
}

#[test]
fn union_and_subtract_produce_valid_mesh() {
    let _guard = TEST_LOCK.lock().unwrap();
    let a = extrude_profile(&rect_profile(40.0, 40.0), 10.0).unwrap();
    let b = extrude_profile(&rect_profile(20.0, 20.0), 10.0).unwrap();
    let unioned = union(&a, &b).unwrap();
    assert!(unioned.tessellate().triangle_count() > 0);
    let subtracted = subtract(&a, &b).unwrap();
    assert!(subtracted.tessellate().triangle_count() > 0);
}

#[test]
fn fillet_all_and_chamfer_all_smoke() {
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
    let shape = extrude_profile(&rect_profile(30.0, 30.0), 20.0).unwrap();
    let hollowed = shell_hollow(&shape, 2.0, Direction::PosZ).unwrap();
    assert!(hollowed.tessellate().triangle_count() > 0);
}

#[test]
fn deep_clone_preserves_mesh_vertex_count() {
    let _guard = TEST_LOCK.lock().unwrap();
    let shape = extrude_profile(&rect_profile(25.0, 15.0), 10.0).unwrap();
    let cloned = crate::shape::deep_clone(shape.inner()).unwrap();
    let original_mesh = shape.tessellate();
    let cloned_mesh = crate::mesh::tessellate_shape(&cloned);
    assert_eq!(original_mesh.positions.len(), cloned_mesh.positions.len());
}

#[test]
fn clone_shape_independent_of_original() {
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
    let shape = make_filleted_box(40.0, 30.0, 20.0, 3.0).unwrap();
    let mesh = shape.tessellate();
    assert!(mesh.triangle_count() > 0);
}

#[test]
fn step_string_roundtrip_preserves_mesh_vertex_count() {
    let _guard = TEST_LOCK.lock().unwrap();
    let shape = extrude_profile(&rect_profile(25.0, 15.0), 10.0).unwrap();
    let step = shape.to_step_string().unwrap();
    assert!(step.contains("ISO-10303"), "STEP harus AP214 ISO-10303");
    let restored = KernelShape::from_step_string(&step).unwrap();
    assert_eq!(shape.tessellate().positions.len(), restored.tessellate().positions.len());
}

#[test]
fn read_step_roundtrips_write_step() {
    let _guard = TEST_LOCK.lock().unwrap();
    let shape = extrude_profile(&rect_profile(10.0, 10.0), 5.0).unwrap();
    let path = std::env::temp_dir().join(format!("ducad-test-read-step-{}.step", std::process::id()));
    shape.write_step(&path).unwrap();
    let restored = KernelShape::read_step(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(shape.tessellate().positions.len(), restored.tessellate().positions.len());
}

#[test]
fn write_step_compound_combines_two_bodies() {
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
    let profile = offset_rect_profile(10.0, 0.0, 20.0, 5.0);
    assert!(revolve_profile(&profile, (0.0, 0.0), (0.0, 0.0), None).is_err());
}

#[test]
fn revolve_profile_axis_crossing_profile_returns_err_safely_without_abort() {
    let _guard = TEST_LOCK.lock().unwrap();
    // Profil membentang dari X=10 sampai X=20, Y=0 sampai Y=5
    let profile = offset_rect_profile(10.0, 0.0, 20.0, 5.0);
    // Sumbu X=15 membelah tengah persegi panjang -> memicu self-intersection di OCCT
    let result = revolve_profile(&profile, (15.0, 0.0), (0.0, 1.0), None);
    assert!(result.is_err(), "Revolve dengan sumbu membelah profil harus return Err, bukan abort/crash!");
}

#[test]
fn revolve_profile_partial_angle_succeeds() {
    let _guard = TEST_LOCK.lock().unwrap();
    let profile = offset_rect_profile(10.0, 0.0, 20.0, 5.0);
    let shape_180 = revolve_profile(&profile, (0.0, 0.0), (0.0, 1.0), Some(180.0)).unwrap();
    let mesh = shape_180.tessellate();
    assert!(mesh.triangle_count() > 0);
}


// ---- Fase 8: Loft ----

#[test]
fn loft_between_rectangles_spans_requested_height() {
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
    let bottom = rect_profile(20.0, 20.0);
    let top = rect_profile(10.0, 10.0);
    assert!(loft_profiles(&bottom, &top, 0.0).is_err());
}

// ---- Fase 8: Boolean intersect ----

#[test]
fn intersect_overlapping_boxes_smaller_than_union() {
    let _guard = TEST_LOCK.lock().unwrap();
    let a = extrude_profile(&rect_profile(40.0, 40.0), 10.0).unwrap();
    let b = extrude_profile(&offset_rect_profile(20.0, 20.0, 60.0, 60.0), 10.0).unwrap();
    let intersected = intersect(&a, &b).unwrap();
    let unioned = union(&a, &b).unwrap();
    assert!(intersected.tessellate().positions.len() < unioned.tessellate().positions.len());
    assert!(intersected.tessellate().triangle_count() > 0);
}

#[test]
fn intersect_non_overlapping_boxes_errors() {
    let _guard = TEST_LOCK.lock().unwrap();
    let a = extrude_profile(&rect_profile(10.0, 10.0), 10.0).unwrap();
    let b = extrude_profile(&offset_rect_profile(100.0, 100.0, 110.0, 110.0), 10.0).unwrap();
    assert!(intersect(&a, &b).is_err());
}

// ---- Fase 8: Picking 3D (edge/face) ----

#[test]
fn pick_face_consistent_across_deep_clone() {
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    assert!(fillet_edges(&shape, 2.0, &[], 1.0).is_err());
}

#[test]
fn fillet_edges_no_match_errors() {
    let _guard = TEST_LOCK.lock().unwrap();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (1000.0, 1000.0, 1000.0),
        dir: (0.0, 0.0, 1.0),
    };
    assert!(fillet_edges(&shape, 2.0, &[ray], 1.0).is_err());
}

// ---- Vertex Fillet Gizmo: fillet SEMUA tepi yang bertemu di 1 sudut ----

#[test]
fn fillet_vertex_rounds_box_corner() {
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (-5.0, -5.0, -5.0),
        dir: (1.0, 1.0, 1.0),
    };
    assert!(fillet_vertex(&shape, 0.0, ray, 1.0).is_err());
}

#[test]
fn fillet_vertex_no_match_errors() {
    let _guard = TEST_LOCK.lock().unwrap();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (1000.0, 1000.0, 1000.0),
        dir: (0.0, 0.0, 1.0),
    };
    assert!(fillet_vertex(&shape, 2.0, ray, 1.0).is_err());
}

#[test]
fn fillet_edges_oversized_radius_errors_not_crashes() {
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (-5.0, -5.0, -5.0),
        dir: (1.0, 1.0, 1.0),
    };
    assert!(chamfer_vertex(&shape, 0.0, ray, 1.0).is_err());
}

#[test]
fn chamfer_vertex_no_match_errors() {
    let _guard = TEST_LOCK.lock().unwrap();
    let shape = extrude_profile(&rect_profile(30.0, 20.0), 15.0).unwrap();
    let ray = PickRay {
        origin: (1000.0, 1000.0, 1000.0),
        dir: (0.0, 0.0, 1.0),
    };
    assert!(chamfer_vertex(&shape, 2.0, ray, 1.0).is_err());
}

#[test]
fn chamfer_vertex_oversized_distance_errors_not_crashes() {
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
    let shape = extrude_profile(&rect_profile(30.0, 30.0), 20.0).unwrap();
    assert!(shell_hollow_faces(&shape, 2.0, &[]).is_err());
}

#[test]
fn extrude_vertical_front_xz_produces_solid() {
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
    let shape = extrude_profile(&rect_profile(194.468, 77.195), 51.933).unwrap();

    let ray = PickRay { origin: (100.0, -100.0, 25.0), dir: (20.0, 130.0, 3.0) };
    let hit = pick_face_details(&shape, ray);
    assert!(hit.is_some(), "ray bersih menuju wajah Y=min box dimensi real HARUS kena");
}

#[test]
fn test_pick_face_max_bound_side_face_oblique() {
    let _guard = TEST_LOCK.lock().unwrap();
    let shape = extrude_profile(&rect_profile(194.468, 77.195), 51.933).unwrap();

    let ray = PickRay { origin: (300.0, 20.0, 25.0), dir: (-150.0, 20.0, 5.0) };
    let hit = pick_face_details(&shape, ray);
    assert!(hit.is_some(), "ray bersih menuju wajah X=max HARUS kena");
}

#[test]
fn test_pick_face_cap_face_real_box_dims_isolation() {
    let _guard = TEST_LOCK.lock().unwrap();
    let shape = extrude_profile(&rect_profile(194.468, 77.195), 51.933).unwrap();

    let ray = PickRay { origin: (100.0, 30.0, 300.0), dir: (20.0, 5.0, -255.0) };
    let hit = pick_face_details(&shape, ray);
    assert!(hit.is_some(), "ray oblique ke cap face Z=max HARUS kena");
}

#[test]
fn test_pick_face_details_and_extrude_box_faces() {
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
    let cube = AdHocShape::make_box(10.0, 10.0, 10.0);
    let faces: Vec<_> = cube.faces().collect();
    assert_eq!(faces.len(), 6, "kubus harus punya 6 face");
    for face in &faces {
        assert_eq!(SurfaceKind::from(face.surface_kind().as_str()), SurfaceKind::Plane);
    }
}

#[test]
fn surface_kind_detects_plane_and_cylinder_faces_on_cylinder() {
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
    const R: f64 = 10.0;
    const H: f64 = 20.0;
    let cylinder = KernelShape(AdHocShape::make_cylinder(dvec3(0.0, 0.0, 0.0), R, H).0);
    let ray = PickRay { origin: (R + 50.0, 0.0, H / 2.0), dir: (-1.0, 0.0, 0.0) };

    let shrunk = extrude_face(&cylinder, ray, -3.0).expect("push -3 pada selimut silinder harus berhasil");
    assert_close(shrunk.inner().volume(), std::f64::consts::PI * 7.0 * 7.0 * H, "volume silinder R=7,h=20");
}

#[test]
fn extrude_face_cylinder_outer_wall_rejects_offset_making_radius_non_positive() {
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
    const R_OUT: f64 = 20.0;
    const R_IN: f64 = 8.0;
    const H: f64 = 20.0;
    let outer = AdHocShape::make_cylinder(dvec3(0.0, 0.0, 0.0), R_OUT, H);
    let inner = AdHocShape::make_cylinder(dvec3(0.0, 0.0, -1.0), R_IN, H + 2.0);
    let mut tube_shape = outer.0.subtract(&inner.0).unwrap().shape;
    tube_shape.clean();
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
    let _guard = TEST_LOCK.lock().unwrap();
    const R_OUT: f64 = 20.0;
    const R_IN: f64 = 8.0;
    const H: f64 = 20.0;
    let outer = AdHocShape::make_cylinder(dvec3(0.0, 0.0, 0.0), R_OUT, H);
    let inner = AdHocShape::make_cylinder(dvec3(0.0, 0.0, -1.0), R_IN, H + 2.0);
    let mut tube_shape = outer.0.subtract(&inner.0).unwrap().shape;
    tube_shape.clean();
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
    let _guard = TEST_LOCK.lock().unwrap();
    const R_OUT: f64 = 20.0;
    const R_IN: f64 = 8.0;
    const H: f64 = 20.0;
    let outer = AdHocShape::make_cylinder(dvec3(0.0, 0.0, 0.0), R_OUT, H);
    let inner = AdHocShape::make_cylinder(dvec3(0.0, 0.0, -1.0), R_IN, H + 2.0);
    let mut tube_shape = outer.0.subtract(&inner.0).unwrap().shape;
    tube_shape.clean();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
    let circle_profile = Profile::Circle { center: (0.0, 0.0), radius: 12.0 };
    let cylinder = extrude_profile(&circle_profile, 25.0).unwrap();
    let ray_top = PickRay { origin: (0.0, 0.0, 100.0), dir: (0.0, 0.0, -1.0) };
    let hit_top = pick_face_details(&cylinder, ray_top).expect("harus kena top cap silinder");
    assert_eq!(hit_top.surface_kind, SurfaceKind::Plane);
    assert_eq!(hit_top.pull_dir, hit_top.normal, "Plane: pull_dir harus identik dgn normal Newell (perilaku lama)");
}

#[test]
fn pull_dir_is_radial_on_cylinder_wall() {
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
    let _guard = TEST_LOCK.lock().unwrap();
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
