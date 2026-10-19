//! Tes kontrak backend (PLAN_MIGRASI_TRUCK F2): HANYA API publik, dijalankan
//! apa adanya di kedua backend. Angka yang diharapkan adalah geometri eksak
//! (volume analitik, jumlah face/edge seperti OCCT setelah `clean()`), jadi
//! lulus di dua backend = paritas.
//!
//! ```text
//! cargo test -p ducad-kernel -- contract_tests                                         # OCCT
//! cargo test -p ducad-kernel --no-default-features --features kernel-truck -- contract  # truck
//! ```
//!
//! Modul dinamai sesuai modul kernel (`primitives`, `csg`, `shape`, `topo`,
//! `picking`) supaya filter `-- primitives csg shape` (gate F2.5) mengenainya.

use std::f64::consts::PI;

use crate::*;

/// Kunci tes: backend OCCT tidak thread-safe (lihat `KERNEL_LOCK`); di truck
/// tidak perlu, tapi tes tetap serial agar hasil kedua backend sebanding.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|p| p.into_inner())
}

fn close(got: f64, want: f64, rel: f64, what: &str) {
    let err = (got - want).abs() / want.abs().max(1e-12);
    assert!(
        err <= rel,
        "{what}: {got} ≠ {want} (galat relatif {err:.2e} > {rel:.0e}) [{BACKEND}]"
    );
}

fn rect(x0: f64, y0: f64, w: f64, h: f64) -> Profile {
    let (x1, y1) = (x0 + w, y0 + h);
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

fn bbox(s: &KernelShape) -> ([f64; 3], [f64; 3]) {
    let (lo, hi) = s.tessellate().bounding_box().expect("mesh kosong");
    (lo.map(f64::from), hi.map(f64::from))
}

fn assert_bbox(s: &KernelShape, lo: [f64; 3], hi: [f64; 3], tol: f64) {
    let (a, b) = bbox(s);
    for i in 0..3 {
        assert!(
            (a[i] - lo[i]).abs() <= tol && (b[i] - hi[i]).abs() <= tol,
            "bbox {a:?}..{b:?} ≠ {lo:?}..{hi:?} [{BACKEND}]"
        );
    }
}

fn counts(s: &KernelShape) -> (usize, usize) {
    (enumerate_faces(s).len(), enumerate_edges(s).len())
}

mod primitives {
    use super::*;

    #[test]
    fn box_volume_area_topology() {
        let _g = serial();
        let b = make_box(10.0, 20.0, 30.0, false).unwrap();
        close(b.volume().abs(), 6000.0, 1e-4, "volume box");
        close(b.surface_area(), 2200.0, 1e-4, "luas box");
        assert_eq!(counts(&b), (6, 12));
        assert!(b.is_valid());
        assert_bbox(&b, [0.0; 3], [10.0, 20.0, 30.0], 1e-6);
        let c = make_box(10.0, 20.0, 30.0, true).unwrap();
        assert_bbox(&c, [-5.0, -10.0, -15.0], [5.0, 10.0, 15.0], 1e-6);
    }

    #[test]
    fn cylinder_counts_like_occt() {
        let _g = serial();
        let c = make_cylinder(5.0, 10.0).unwrap();
        close(c.volume().abs(), PI * 25.0 * 10.0, 1e-3, "volume silinder");
        // OCCT: 2 tutup + 1 selimut periodik; 2 lingkaran + 1 seam.
        assert_eq!(counts(&c), (3, 3));
        assert_bbox(&c, [-5.0, -5.0, 0.0], [5.0, 5.0, 10.0], 0.06);
    }

    #[test]
    fn sphere_and_cone_volume() {
        let _g = serial();
        close(
            make_sphere(5.0).unwrap().volume().abs(),
            4.0 / 3.0 * PI * 125.0,
            2e-3,
            "bola",
        );
        let cone = make_cone(5.0, 2.0, 10.0).unwrap();
        close(
            cone.volume().abs(),
            PI * 10.0 / 3.0 * (25.0 + 10.0 + 4.0),
            2e-3,
            "kerucut",
        );
        assert_eq!(enumerate_faces(&cone).len(), 3);
        // Bola = satu face periodik di OCCT; potongan patch SolidCraft digabung.
        let ball = make_sphere(5.0).unwrap();
        assert_eq!(enumerate_faces(&ball).len(), 1);
        assert_eq!(enumerate_faces(&ball)[0].kind, SurfaceKind::Sphere);
        let sharp = make_cone(5.0, 0.0, 10.0).unwrap();
        close(
            sharp.volume().abs(),
            PI * 25.0 * 10.0 / 3.0,
            2e-3,
            "kerucut lancip",
        );
    }

    #[test]
    fn invalid_sizes_are_errors_not_panics() {
        let _g = serial();
        let e = make_box(0.0, 1.0, 1.0, false).err().expect("harus galat");
        assert!(e.to_string().contains("harus > 0"), "{e}");
        assert!(make_cylinder(-1.0, 1.0).is_err());
        assert!(make_sphere(f64::NAN).is_err());
        assert!(make_cone(1.0, -1.0, 1.0).is_err());
    }
}

mod csg {
    use super::*;

    #[test]
    fn extrude_square() {
        let _g = serial();
        let s = extrude_profile(&rect(0.0, 0.0, 10.0, 10.0), 5.0).unwrap();
        close(s.volume().abs(), 500.0, 1e-4, "volume extrude");
        assert_eq!(counts(&s), (6, 12));
        let down = extrude_profile(&rect(0.0, 0.0, 10.0, 10.0), -5.0).unwrap();
        assert_bbox(&down, [0.0, 0.0, -5.0], [10.0, 10.0, 0.0], 1e-6);
    }

    #[test]
    fn extrude_follows_normal_and_extent() {
        let _g = serial();
        // Normal berlawanan u×v: extrude ke −Z.
        let s = extrude_profile_on_plane(
            &rect(0.0, 0.0, 4.0, 4.0),
            [0.0, 0.0, 10.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, -1.0],
            3.0,
        )
        .unwrap();
        assert_bbox(&s, [0.0, 0.0, 7.0], [4.0, 4.0, 10.0], 1e-6);
        // Bidang XZ (normal −Y), simetris 10.
        let sym = extrude_profile_extent(
            &rect(0.0, 0.0, 4.0, 2.0),
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, -1.0, 0.0],
            ExtrudeExtent::Symmetric(10.0),
        )
        .unwrap();
        assert_bbox(&sym, [0.0, -5.0, 0.0], [4.0, 5.0, 2.0], 1e-6);
        close(sym.volume().abs(), 80.0, 1e-4, "volume simetris");
    }

    #[test]
    fn extrude_with_hole_ellipse_and_bezier() {
        let _g = serial();
        let plate = rect(0.0, 0.0, 40.0, 40.0).with_holes(vec![Profile::Circle {
            center: (20.0, 20.0),
            radius: 5.0,
        }]);
        let s = extrude_profile(&plate, 10.0).unwrap();
        close(
            s.volume().abs(),
            16000.0 - PI * 250.0,
            1e-3,
            "plat berlubang",
        );
        // 6 sisi + dinding lubang; 12 + 2 lingkaran + 1 seam.
        assert_eq!(counts(&s), (7, 15));

        let e = extrude_profile(
            &Profile::Ellipse {
                center: (0.0, 0.0),
                radius_x: 3.0,
                radius_y: 2.0,
            },
            1.0,
        )
        .unwrap();
        close(e.volume().abs(), PI * 6.0, 2e-3, "elips");

        // Tetes air dari dua Bézier (seperti outline glyph): luas tiap lobus
        // ∫x dy = 1800·∫t²(1−t)² dt = 60 mm², jadi volume 2·60·2 = 240 mm³.
        let drop = Profile::Loop(vec![
            ProfileSegment::Bezier {
                start: (0.0, 0.0),
                c1: (10.0, 0.0),
                c2: (10.0, 10.0),
                end: (0.0, 10.0),
            },
            ProfileSegment::Bezier {
                start: (0.0, 10.0),
                c1: (-10.0, 10.0),
                c2: (-10.0, 0.0),
                end: (0.0, 0.0),
            },
        ]);
        let d = extrude_profile(&drop, 2.0).unwrap();
        close(d.volume().abs(), 240.0, 2e-3, "Bézier");
    }

    #[test]
    fn revolve_full_and_partial() {
        let _g = serial();
        let ring =
            revolve_profile(&rect(1.0, 0.0, 1.0, 1.0), (0.0, 0.0), (0.0, 1.0), None).unwrap();
        close(ring.volume().abs(), PI * 3.0, 1e-3, "cincin");
        // 2 selimut + 2 annulus; 4 lingkaran + 2 seam.
        assert_eq!(counts(&ring), (4, 6));
        let quarter = revolve_profile(
            &rect(1.0, 0.0, 1.0, 1.0),
            (0.0, 0.0),
            (0.0, 1.0),
            Some(90.0),
        )
        .unwrap();
        close(quarter.volume().abs(), PI * 3.0 / 4.0, 1e-3, "seperempat");
        // Putaran tangan-kanan +90° terhadap +Y membawa +X ke −Z.
        let (lo, hi) = bbox(&quarter);
        assert!(
            lo[2] < -1.9 && hi[2].abs() < 1e-3 && lo[0].abs() < 1e-3,
            "{lo:?}..{hi:?} [{BACKEND}]"
        );
        assert!(revolve_profile(&rect(0.0, 0.0, 1.0, 1.0), (0.0, 0.0), (0.0, 0.0), None).is_err());
    }

    #[test]
    fn booleans() {
        let _g = serial();
        let a = make_box(10.0, 10.0, 10.0, false).unwrap();
        let b = translate_shape(&a, 5.0, 0.0, 0.0).unwrap();
        let u = union(&a, &b).unwrap();
        close(u.volume().abs(), 1500.0, 1e-4, "union");
        // Face sebidang di sambungan digabung (`clean()` OCCT / topologi logis truck).
        assert_eq!(counts(&u), (6, 12));
        let i = intersect(&a, &b).unwrap();
        close(i.volume().abs(), 500.0, 1e-4, "intersect");

        let plate = make_box(20.0, 20.0, 10.0, false).unwrap();
        let pin = translate_shape(&make_cylinder(3.0, 20.0).unwrap(), 10.0, 10.0, -5.0).unwrap();
        let holed = subtract(&plate, &pin).unwrap();
        close(
            holed.volume().abs(),
            4000.0 - PI * 90.0,
            1e-3,
            "lubang tembus",
        );
        assert_eq!(counts(&holed), (7, 15));
        let wall = enumerate_faces(&holed)
            .into_iter()
            .find(|f| f.kind == SurfaceKind::Cylinder)
            .expect("dinding lubang");
        assert_eq!(wall.concave, Some(true));
        close(wall.radius.expect("radius"), 3.0, 1e-6, "radius lubang");

        let far = translate_shape(&a, 100.0, 0.0, 0.0).unwrap();
        let err = intersect(&a, &far).err().expect("irisan kosong");
        assert!(err.downcast_ref::<EmptyIntersection>().is_some(), "{err:#}");
    }

    #[test]
    fn loft_and_sweep() {
        let _g = serial();
        let l = loft_profiles(
            &rect(-5.0, -5.0, 10.0, 10.0),
            &rect(-2.5, -2.5, 5.0, 5.0),
            10.0,
        )
        .unwrap();
        close(
            l.volume().abs(),
            10.0 / 3.0 * (100.0 + 25.0 + 50.0),
            2e-3,
            "loft frustum",
        );
        let s = sweep_profile_along_path(
            &Profile::Circle {
                center: (0.0, 0.0),
                radius: 1.0,
            },
            &[PathSegment::Line {
                start: [0.0, 0.0, 0.0],
                end: [0.0, 0.0, 10.0],
            }],
        )
        .unwrap();
        close(s.volume().abs(), PI * 10.0, 2e-3, "sweep lurus");
    }

    #[test]
    fn emboss_and_deboss() {
        let _g = serial();
        let base = make_box(20.0, 20.0, 5.0, false).unwrap();
        let o = [0.0, 0.0, 5.0];
        let (u, v, n) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
        let profile = [rect(5.0, 5.0, 10.0, 10.0)];
        let up = emboss_profiles_on_plane(Some(&base), &profile, o, u, v, n, 1.0, false).unwrap();
        close(up.volume().abs(), 2100.0, 1e-4, "emboss");
        let down = emboss_profiles_on_plane(Some(&base), &profile, o, u, v, n, 1.0, true).unwrap();
        close(down.volume().abs(), 1900.0, 1e-4, "deboss");
    }
}

mod shape {
    use super::*;

    #[test]
    fn transforms() {
        let _g = serial();
        let b = make_box(10.0, 20.0, 30.0, false).unwrap();
        let moved = translate_shape(&b, 1.0, 2.0, 3.0).unwrap();
        assert_bbox(&moved, [1.0, 2.0, 3.0], [11.0, 22.0, 33.0], 1e-6);
        let r = rotate_shape(&b, (0.0, 0.0, 0.0), (0.0, 0.0, 1.0), PI / 2.0).unwrap();
        assert_bbox(&r, [-20.0, 0.0, 0.0], [0.0, 10.0, 30.0], 1e-6);
        let s = scale_shape(&b, (0.0, 0.0, 0.0), 2.0).unwrap();
        close(s.volume().abs(), 48000.0, 1e-4, "skala 2×");
        let t = transform_shape(
            &b,
            (100.0, 0.0, 0.0),
            (0.0, 0.0, 0.0),
            (0.0, 0.0, 1.0),
            PI / 2.0,
        )
        .unwrap();
        assert_bbox(&t, [80.0, 0.0, 0.0], [100.0, 10.0, 30.0], 1e-6);
        let m = mirror_shape(&b, glam::DVec3::ZERO, glam::DVec3::X).unwrap();
        assert_bbox(&m, [-10.0, 0.0, 0.0], [0.0, 20.0, 30.0], 1e-6);
        close(m.volume().abs(), 6000.0, 1e-4, "cermin");
        assert!(m.is_valid());
        close(
            clone_shape(&b).unwrap().volume().abs(),
            6000.0,
            1e-9,
            "clone",
        );
    }

    #[test]
    fn compound_and_empty() {
        let _g = serial();
        let a = make_box(1.0, 1.0, 1.0, false).unwrap();
        let b = translate_shape(&a, 5.0, 0.0, 0.0).unwrap();
        let c = make_compound(&[&a, &b]).unwrap();
        close(c.volume().abs(), 2.0, 1e-4, "compound");
        assert_eq!(enumerate_faces(&c).len(), 12);
        assert!(make_compound(&[]).is_err());
        let e = KernelShape::empty();
        assert_eq!(e.tessellate().triangle_count(), 0);
        assert!(e.volume().abs() < 1e-9);
    }

    #[test]
    fn step_and_brep_roundtrip() {
        let _g = serial();
        let plate = rect(0.0, 0.0, 40.0, 40.0).with_holes(vec![Profile::Circle {
            center: (20.0, 20.0),
            radius: 5.0,
        }]);
        let s = extrude_profile(&plate, 10.0).unwrap();
        let back = KernelShape::from_step_string(&s.to_step_string().unwrap()).unwrap();
        close(
            back.volume().abs(),
            s.volume().abs(),
            1e-3,
            "STEP roundtrip",
        );
        assert_eq!(counts(&back), counts(&s));
        let bin = KernelShape::from_brep_bytes(&s.to_brep_bytes().unwrap()).unwrap();
        close(bin.volume().abs(), s.volume().abs(), 1e-6, "BREP roundtrip");
        let cache = KernelShape::from_native_cache(&s.to_native_cache().unwrap()).unwrap();
        close(cache.volume().abs(), s.volume().abs(), 1e-6, "cache native");
    }

    #[test]
    fn stl_roundtrip() {
        let _g = serial();
        let path = std::env::temp_dir().join(format!(
            "ducad-contract-{}-{BACKEND}.stl",
            std::process::id()
        ));
        let b = make_box(10.0, 10.0, 10.0, false).unwrap();
        b.write_stl(&path).unwrap();
        let back = KernelShape::read_stl(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(back.tessellate().triangle_count() >= 12);
    }

    #[test]
    fn mass_properties_of_box() {
        let _g = serial();
        let b = make_box(10.0, 20.0, 30.0, false).unwrap();
        let mp = b.mass_properties();
        close(mp.volume_mm3, 6000.0, 1e-4, "volume massa");
        for (got, want) in mp.centroid.iter().zip([5.0, 10.0, 15.0]) {
            close(*got, want, 1e-4, "pusat massa");
        }
        let i = mp.inertia_com();
        close(i[0][0], 6000.0 * (400.0 + 900.0) / 12.0, 1e-3, "Ixx");
        close(i[2][2], 6000.0 * (100.0 + 400.0) / 12.0, 1e-3, "Izz");
        assert!(i[0][1].abs() < 1.0, "Ixy {}", i[0][1]);
    }

    #[test]
    fn clearance_between_boxes() {
        let _g = serial();
        let a = make_box(10.0, 10.0, 10.0, false).unwrap();
        let b = translate_shape(&a, 15.0, 0.0, 0.0).unwrap();
        let r = check_clearance(&a, &b, 4.0).unwrap();
        close(r.distance, 5.0, 1e-4, "celah");
        assert!(r.passes);
    }
}

mod topo {
    use super::*;

    #[test]
    fn box_faces_and_edges() {
        let _g = serial();
        let b = make_box(10.0, 20.0, 30.0, false).unwrap();
        let faces = enumerate_faces(&b);
        let center = [5.0, 10.0, 15.0];
        let mut area = 0.0;
        for f in &faces {
            assert_eq!(f.kind, SurfaceKind::Plane);
            let out: f64 = (0..3)
                .map(|i| f.normal[i] * (f.centroid[i] - center[i]))
                .sum();
            assert!(
                out > 0.0,
                "normal face {} harus keluar: {:?} [{BACKEND}]",
                f.index,
                f.normal
            );
            assert_eq!(f.edge_indices.len(), 4);
            area += f.area;
        }
        close(area, 2200.0, 1e-6, "luas face");
        let edges = enumerate_edges(&b);
        let mut lens: Vec<f64> = edges.iter().map(|e| e.length).collect();
        lens.sort_by(f64::total_cmp);
        for (i, want) in [10.0, 20.0, 30.0].iter().enumerate() {
            for l in &lens[i * 4..i * 4 + 4] {
                close(*l, *want, 1e-9, "panjang tepi");
            }
        }
        assert!(edges
            .iter()
            .all(|e| e.kind == EdgeKind::Line && e.dir.is_some()));
        let m = b.tessellate();
        assert_eq!(m.face_ranges.len(), faces.len());
        let covered: u32 = m.face_ranges.iter().map(|r| r.end - r.start).sum();
        assert_eq!(covered as usize, m.indices.len());
    }

    #[test]
    fn cylinder_faces_and_edges() {
        let _g = serial();
        let c = make_cylinder(5.0, 10.0).unwrap();
        let faces = enumerate_faces(&c);
        let side = faces
            .iter()
            .find(|f| f.kind == SurfaceKind::Cylinder)
            .expect("selimut");
        close(side.radius.expect("radius"), 5.0, 1e-6, "radius selimut");
        assert_eq!(side.concave, Some(false));
        let (_, dir) = side.axis.expect("sumbu");
        assert!(dir[2].abs() > 1.0 - 1e-9, "sumbu {dir:?}");
        assert_eq!(
            faces
                .iter()
                .filter(|f| f.kind == SurfaceKind::Plane)
                .count(),
            2
        );
        let edges = enumerate_edges(&c);
        let circles: Vec<_> = edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Circle)
            .collect();
        assert_eq!(circles.len(), 2);
        for e in circles {
            close(e.radius.expect("radius"), 5.0, 1e-6, "radius tepi");
            close(e.length, 2.0 * PI * 5.0, 2e-3, "keliling");
        }
        assert_eq!(shape_vertices(&c).len(), 2);
    }
}

mod picking {
    use super::*;

    fn down(x: f64, y: f64) -> PickRay {
        PickRay {
            origin: (x, y, 100.0),
            dir: (0.0, 0.0, -1.0),
        }
    }

    #[test]
    fn pick_box_face_edge_vertex() {
        let _g = serial();
        let b = make_box(10.0, 20.0, 30.0, false).unwrap();
        let hit = pick_face_details(&b, down(5.0, 10.0)).expect("kena atas");
        assert!((hit.hit_point.2 - 30.0).abs() < 1e-6);
        assert!((hit.normal.2 - 1.0).abs() < 1e-9, "{:?}", hit.normal);
        assert_eq!(hit.surface_kind, SurfaceKind::Plane);
        let top = enumerate_faces(&b)
            .into_iter()
            .find(|f| f.normal[2] > 0.9)
            .expect("face atas");
        assert_eq!(hit.face_index, Some(top.index));
        assert_eq!(hit.boundary_points.len(), 4);
        assert!(pick_face(&b, down(50.0, 50.0)).is_none());

        let (p, poly) = pick_edge(&b, down(10.0, 10.0), 0.5).expect("tepi");
        assert!((p.0 - 10.0).abs() < 1e-6 && (p.2 - 30.0).abs() < 1e-6);
        assert!(poly.len() >= 2);
        let v = pick_vertex(&b, down(10.0, 20.0), 0.5).expect("sudut");
        assert!(
            (v.0 - 10.0).abs() < 1e-6 && (v.1 - 20.0).abs() < 1e-6 && (v.2 - 30.0).abs() < 1e-6
        );
        assert_eq!(shape_vertices(&b).len(), 8);
        assert_eq!(edge_dimensions(&b).len(), 12);
        let n = vertex_outward_normal(&b, (10.0, 20.0, 30.0));
        let k = 1.0 / 3f64.sqrt();
        assert!(
            (n.0 - k).abs() < 1e-6 && (n.1 - k).abs() < 1e-6 && (n.2 - k).abs() < 1e-6,
            "{n:?}"
        );
        let (_, en) = edge_outward_normal(&b, down(10.0, 10.0), 0.5).expect("normal tepi");
        let h = std::f64::consts::FRAC_1_SQRT_2;
        assert!((en.0 - h).abs() < 1e-6 && (en.2 - h).abs() < 1e-6, "{en:?}");
        assert!(!extract_shape_edges(&b, None).is_empty());
    }

    #[test]
    fn pick_cylinder_side() {
        let _g = serial();
        let c = make_cylinder(5.0, 10.0).unwrap();
        let ray = PickRay {
            origin: (100.0, 0.0, 5.0),
            dir: (-1.0, 0.0, 0.0),
        };
        let hit = pick_face_details(&c, ray).expect("kena selimut");
        assert_eq!(hit.surface_kind, SurfaceKind::Cylinder);
        assert!((hit.hit_point.0 - 5.0).abs() < 0.06, "{:?}", hit.hit_point);
        assert!((hit.pull_dir.0 - 1.0).abs() < 1e-3, "{:?}", hit.pull_dir);
        close(hit.surface_radius.expect("radius"), 5.0, 1e-6, "radius hit");
        assert!(hit.radial_axis.is_some());
    }
}
