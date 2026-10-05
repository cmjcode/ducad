//! Gate P18: mesher tet, Tet10, frekuensi, buckling, termal, dan fallback hex.
//! Jalankan dengan `--nocapture` untuk melihat angka terukur.

use ducad_sim::bc::{tri6_nodal, BoundaryMesh};
use ducad_sim::benchmark::advanced::{self, steel_thermal};
use ducad_sim::benchmark::cases::{self, steel, BenchValue};
use ducad_sim::benchmark::{
    box_surface, cylinder, extrude_profile, l_bracket, plate_with_hole, thick_ring, FACE_XMAX,
    FACE_XMIN, FACE_YMIN, FACE_ZMAX, FACE_ZMIN,
};
use ducad_sim::{
    build_mesh, run_static, run_thermal, tetrahedralize, CancelToken, FixtureKind, LoadKind,
    MeshKind, MeshSettings, ResolvedFixture, ResolvedLoad, ResolvedSetup, ResolvedThermalBc,
    ResolvedThermalSetup, SimMesh, SurfaceMesh, ThermalBc, ThermalBcKind, ThermalSetup,
    FALLBACK_WARNING,
};

fn show(name: &str, v: BenchValue) {
    println!(
        "{name}: terukur {:.6}, analitik {:.6}, galat {:+.2} %",
        v.measured,
        v.analytic,
        100.0 * v.rel_error()
    );
}

/// Luas tiap tag pada mesh permukaan masukan.
fn surface_areas(surface: &SurfaceMesh) -> Vec<(u32, f64)> {
    let mut tags: Vec<u32> = surface.tri_face.clone();
    tags.sort_unstable();
    tags.dedup();
    tags.iter()
        .map(|&tag| {
            let mut area = 0.0;
            for (tri, &f) in surface.triangles.iter().zip(&surface.tri_face) {
                if f != tag {
                    continue;
                }
                let p = |i: usize| surface.positions[tri[i] as usize];
                let (a, b, c) = (p(0), p(1), p(2));
                let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                let n = [
                    u[1] * v[2] - u[2] * v[1],
                    u[2] * v[0] - u[0] * v[2],
                    u[0] * v[1] - u[1] * v[0],
                ];
                area += 0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            }
            (tag, area)
        })
        .collect()
}

#[test]
fn mesh_tet_quality_on_fixtures() {
    let pi = std::f64::consts::PI;
    // (nama, permukaan, ukuran, volume eksak)
    let fixtures: Vec<(&str, SurfaceMesh, f64, f64)> = vec![
        ("box", box_surface([100.0, 10.0, 10.0]), 2.5, 10_000.0),
        (
            "plate_with_hole",
            plate_with_hole([100.0, 50.0, 4.0], 10.0, 256),
            3.0,
            (5000.0 - pi * 25.0) * 4.0,
        ),
        ("l_bracket", l_bracket(40.0, 30.0, 5.0, 20.0), 2.5, 6500.0),
        ("cylinder", cylinder(8.0, 20.0, 256), 2.0, pi * 64.0 * 20.0),
        (
            "thick_ring",
            thick_ring(12.0, 7.0, 6.0, 256),
            2.0,
            pi * (144.0 - 49.0) * 6.0,
        ),
        (
            // Baji: face atas miring (tidak sejajar sumbu).
            "wedge",
            extrude_profile(&[[0.0, 0.0], [30.0, 0.0], [30.0, 8.0], [0.0, 20.0]], 15.0),
            2.5,
            0.5 * (8.0 + 20.0) * 30.0 * 15.0,
        ),
        (
            // Kanal U: dua sudut cekung.
            "u_channel",
            extrude_profile(
                &[
                    [0.0, 0.0],
                    [30.0, 0.0],
                    [30.0, 20.0],
                    [24.0, 20.0],
                    [24.0, 6.0],
                    [6.0, 6.0],
                    [6.0, 20.0],
                    [0.0, 20.0],
                ],
                25.0,
            ),
            2.5,
            (30.0 * 20.0 - 18.0 * 14.0) * 25.0,
        ),
    ];
    for (name, surface, size, exact) in fixtures {
        let settings = cases::tet(size);
        let model = tetrahedralize(&surface, &settings, &[], Some(exact), &CancelToken::new())
            .unwrap_or_else(|e| panic!("{name}: mesher gagal: {e:?}"));
        let mesh = &model.mesh;
        let volume = mesh.volume();
        let min_ratio = (0..mesh.elems.len())
            .map(|e| mesh.radius_ratio(e))
            .fold(f64::INFINITY, f64::min);
        println!(
            "mesh_tet {name}: {} elemen, {} node, rasio radius min {:.3}, galat volume {:+.4} %",
            mesh.elems.len(),
            mesh.nodes.len(),
            min_ratio,
            100.0 * (volume - exact) / exact
        );
        assert_eq!(mesh.inverted_count(), 0, "{name}: ada tet terbalik");
        assert!(min_ratio >= 0.1, "{name}: rasio radius min {min_ratio}");
        assert!((model.min_radius_ratio - min_ratio).abs() < 1e-12);
        assert!(
            (volume - exact).abs() <= 5e-4 * exact,
            "{name}: volume {volume} vs {exact}"
        );
        assert!(model.warnings.is_empty(), "{name}: {:?}", model.warnings);
        // Setiap face terpetakan, dan luas sisi batas per tag cocok dengan permukaan.
        for (tag, area) in surface_areas(&surface) {
            assert!(
                !mesh.face_nodes(tag).is_empty(),
                "{name}: face {tag} tanpa node"
            );
            let meshed: f64 = mesh
                .faces_of(tag)
                .iter()
                .map(|f| {
                    tri6_nodal(&f.nodes.map(|n| mesh.nodes[n as usize]))
                        .0
                        .iter()
                        .sum::<f64>()
                })
                .sum();
            assert!(
                (meshed - area).abs() <= 5e-3 * area,
                "{name}: face {tag} luas {meshed} vs {area}"
            );
            let nodal = model.face_nodal(&[tag], name, &mut Vec::new()).unwrap();
            assert!((nodal.total_area() - meshed).abs() < 1e-9 * area);
        }
        assert_eq!(mesh.tagged_faces(), model.surface_tags);
        // Semua node dipakai, konektivitas sah.
        let mut used = vec![false; mesh.nodes.len()];
        for conn in &mesh.elems {
            for &n in conn {
                used[n as usize] = true;
            }
        }
        assert!(used.iter().all(|&u| u), "{name}: ada node tidak terpakai");
    }
}

#[test]
fn mesh_tet_quality_is_deterministic_and_respects_sizing() {
    let surface = plate_with_hole([60.0, 40.0, 4.0], 12.0, 128);
    let cancel = CancelToken::new();
    let a = tetrahedralize(&surface, &cases::tet(3.0), &[FACE_XMIN], None, &cancel).unwrap();
    let b = tetrahedralize(&surface, &cases::tet(3.0), &[FACE_XMIN], None, &cancel).unwrap();
    assert_eq!(a, b, "mesher tet harus deterministik");
    // Face yang dikunci/dibebani diperhalus: lebih banyak node di sana.
    let plain = tetrahedralize(&surface, &cases::tet(3.0), &[], None, &cancel).unwrap();
    assert!(a.mesh.face_nodes(FACE_XMIN).len() > plain.mesh.face_nodes(FACE_XMIN).len());
    // Target jumlah elemen dihormati secara kasar; bawaan tanpa ukuran juga jalan.
    let by_count = MeshSettings {
        kind: MeshKind::Tet,
        cell_mm: None,
        target_elems: Some(4000),
    };
    let model = tetrahedralize(&surface, &by_count, &[], None, &cancel).unwrap();
    let n = model.mesh.elems.len() as f64;
    assert!(
        (0.4..2.5).contains(&(n / 4000.0)),
        "{n} elemen untuk target 4000"
    );
    cancel.cancel();
    assert!(tetrahedralize(&surface, &cases::tet(3.0), &[], None, &cancel).is_err());
}

#[test]
fn bench_cantilever_tet() {
    // Balok 10×10×100 mm, 100 N di ujung; tiga tingkat kehalusan.
    // δ dan σ ± 3 % analitik, konvergensi monoton.
    let mut deflection_errors = Vec::new();
    let mut stress_errors = Vec::new();
    for size in [5.0, 3.5, 2.5] {
        let result = cases::cantilever_with(cases::tet(size), 0.3).unwrap();
        assert_eq!(result.report.mesh_stats.kind, MeshKind::Tet);
        assert!(
            result.report.warnings.is_empty(),
            "{:?}",
            result.report.warnings
        );
        show(
            &format!("cantilever_tet h={size} deflection [mm]"),
            result.deflection,
        );
        show(
            &format!("cantilever_tet h={size} top-fibre stress at x = 10 mm [MPa]"),
            result.section_stress,
        );
        show(
            &format!("cantilever_tet h={size} max von Mises vs FLc/I [MPa]"),
            result.max_stress,
        );
        assert!(result.deflection.rel_error().abs() < 0.03);
        assert!(result.section_stress.rel_error().abs() < 0.03);
        deflection_errors.push(result.deflection.rel_error().abs());
        stress_errors.push(result.section_stress.rel_error().abs());
        let r = result.report.reactions[0].force_n;
        assert!(
            (r[2] - 100.0).abs() < 1e-3 && r[0].abs() < 1e-3 && r[1].abs() < 1e-3,
            "{r:?}"
        );
    }
    for errors in [&deflection_errors, &stress_errors] {
        assert!(
            errors[0] > errors[1] && errors[1] > errors[2],
            "konvergensi tidak monoton: {errors:?}"
        );
    }
}

#[test]
fn bench_hole_plate_tet() {
    // Pelat 100×50×4 mm berlubang Ø10 ditarik. Acuan: faktor konsentrasi
    // bruto lebar-hingga Howland/Heywood (3.149 untuk d/W = 0.2), ± 5 %.
    let result = cases::hole_plate_with([100.0, 50.0, 4.0], 10.0, cases::tet(3.0), 256).unwrap();
    show(
        "hole_plate_tet stress concentration (gross, vs Howland)",
        result.concentration,
    );
    assert_eq!(result.report.mesh_stats.kind, MeshKind::Tet);
    assert!((result.concentration.analytic - 3.149).abs() < 1e-3);
    assert!(result.concentration.rel_error().abs() < 0.05);
}

#[test]
fn bench_frequency_beam() {
    // Kantilever 150×10×5 mm: tiga frekuensi pertama vs Euler–Bernoulli ± 3 %.
    for (name, settings) in [
        ("tet h=5", cases::tet(5.0)),
        ("hex h=2.5", cases::mesh(2.5)),
    ] {
        let result = advanced::frequency_beam(settings).unwrap();
        assert_eq!(result.modes.len(), 3);
        for (i, mode) in result.modes.iter().enumerate() {
            show(&format!("frequency_beam {name} mode {} [Hz]", i + 1), *mode);
            assert!(mode.rel_error().abs() < 0.03, "{name} mode {}", i + 1);
        }
        assert!(
            result.report.warnings.is_empty(),
            "{:?}",
            result.report.warnings
        );
        // Bentuk mode 1: lentur arah Z, ternormalisasi, nol di jepitan.
        let shape = &result.report.mode_shapes[0];
        let tip = shape.sample([150.0, 5.0, 2.5]);
        let root = shape.sample([0.0, 5.0, 2.5]);
        assert!(tip[2].abs() > 0.9 && tip[2].abs() <= 1.0 + 1e-9, "{tip:?}");
        assert!(root.iter().all(|v| v.abs() < 1e-9));
        let json = serde_json::to_string(&result.report).unwrap();
        assert!(json.contains("frequencies_hz") && !json.contains("mode_shapes"));
    }
}

#[test]
fn bench_euler_buckling() {
    // Kolom sendi–sendi 100×10×5 mm: P_cr = π²EI/L² ± 5 %.
    for (name, settings) in [
        ("tet h=5", cases::tet(5.0)),
        ("hex h=2.5", cases::mesh(2.5)),
    ] {
        let result = advanced::euler_column(settings).unwrap();
        show(
            &format!("euler_buckling {name} critical load [N]"),
            result.critical_load,
        );
        assert!(result.critical_load.rel_error().abs() < 0.05, "{name}");
        // Faktor menaik dan positif; mode pertama melentur ke arah Z di tengah bentang.
        let factors = &result.report.load_factors;
        assert!(
            factors.len() == 2 && factors[0] > 0.0 && factors[0] < factors[1],
            "{factors:?}"
        );
        let mid = result.report.mode_shapes[0].sample([50.0, 5.0, 2.5]);
        assert!(mid[2].abs() > 0.9, "{mid:?}");
        // Pra-tekuk: tekan seragam P/A = 20 MPa (lebih tinggi setempat di kekangan ujung).
        assert!(result.report.prestress_max_von_mises_mpa > 19.0);
    }
}

#[test]
fn bench_thermal_rod() {
    // Batang dengan suhu ujung 20 °C dan 120 °C: profil linier eksak (1e-6),
    // pemuaian bebas tanpa tegangan (< 1e-6 MPa).
    let tet = advanced::thermal_rod(cases::tet(5.0), 20.0, 120.0).unwrap();
    println!(
        "thermal_rod tet: galat profil {:.2e} K, tegangan muai bebas {:.2e} MPa",
        tet.profile_error, tet.free_expansion_stress
    );
    show("thermal_rod tet elongation [mm]", tet.elongation);
    assert!(tet.profile_error < 1e-6);
    assert!(tet.free_expansion_stress < 1e-6);
    assert!(tet.elongation.rel_error().abs() < 1e-6);
    assert!((tet.thermal.max_temperature_c - 120.0).abs() < 1e-9);
    assert!((tet.thermal.min_temperature_c - 20.0).abs() < 1e-9);
    assert!((tet.thermal.location[0] - 100.0).abs() < 1e-9);
    assert!(
        (tet.thermal
            .nodal_field
            .as_ref()
            .unwrap()
            .sample([25.0, 5.0, 5.0])
            - 45.0)
            .abs()
            < 1e-6
    );
    // Hex: profil linier juga eksak. Pemuaian bebas eksak hanya untuk suhu
    // seragam (gradien suhu butuh medan perpindahan kuadratik yang kompatibel).
    let hex = advanced::thermal_rod(cases::mesh(2.5), 20.0, 120.0).unwrap();
    assert!(hex.profile_error < 1e-6);
    let uniform = advanced::thermal_rod(cases::mesh(2.5), 70.0, 70.0).unwrap();
    println!(
        "thermal_rod hex: galat profil {:.2e} K, tegangan muai bebas seragam {:.2e} MPa",
        hex.profile_error, uniform.free_expansion_stress
    );
    assert!(uniform.free_expansion_stress < 1e-6);
    assert!(uniform.elongation.rel_error().abs() < 1e-6);
}

#[test]
fn thermal_flux_and_convection_match_one_dimensional_solution() {
    // Fluks q masuk di −X, konveksi h ke udara T∞ di +X:
    // T(+X) = T∞ + q/h, T(−X) = T(+X) + q·L/k.
    let (length, side) = (100.0, 10.0);
    let material = steel_thermal();
    let (q, h, ambient) = (0.002, 1.0e-4, 25.0);
    let thermal = ResolvedThermalSetup {
        boundary: vec![
            ResolvedThermalBc {
                id: "heater".into(),
                faces: vec![FACE_XMIN],
                kind: ThermalBcKind::HeatFlux { w_per_mm2: q },
            },
            ResolvedThermalBc {
                id: "air".into(),
                faces: vec![FACE_XMAX],
                kind: ThermalBcKind::Convection {
                    h_w_mm2k: h,
                    ambient_c: ambient,
                },
            },
        ],
    };
    let cold = ambient + q / h;
    let hot = cold + q * length / (material.conductivity_w_mk * 1.0e-3);
    for settings in [cases::tet(5.0), cases::mesh(2.5)] {
        let setup = ResolvedSetup {
            mesh: settings,
            ..ResolvedSetup::default()
        };
        let report = run_thermal(
            &box_surface([length, side, side]),
            &material,
            &setup,
            &thermal,
            &CancelToken::new(),
        )
        .unwrap();
        assert!(
            (report.max_temperature_c - hot).abs() < 1e-6 * hot,
            "{} vs {hot}",
            report.max_temperature_c
        );
        assert!(
            (report.min_temperature_c - cold).abs() < 1e-6 * cold,
            "{} vs {cold}",
            report.min_temperature_c
        );
        assert!(report.location[0].abs() < 1e-9);
        assert!(!serde_json::to_string(&report)
            .unwrap()
            .contains("nodal_field"));
    }
    // Tanpa suhu atau konveksi: tingkat suhu tak tentu → galat setup.
    let only_flux = ResolvedThermalSetup {
        boundary: vec![thermal.boundary[0].clone()],
    };
    let setup = ResolvedSetup {
        mesh: cases::mesh(5.0),
        ..ResolvedSetup::default()
    };
    let err = run_thermal(
        &box_surface([length, side, side]),
        &material,
        &setup,
        &only_flux,
        &CancelToken::new(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "SIM_INVALID_SETUP");
}

fn broken_box() -> SurfaceMesh {
    // Kotak tanpa face +X: permukaan tidak tertutup.
    let full = box_surface([40.0, 10.0, 10.0]);
    let mut broken = SurfaceMesh {
        positions: full.positions.clone(),
        ..SurfaceMesh::default()
    };
    for (tri, &face) in full.triangles.iter().zip(&full.tri_face) {
        if face != FACE_XMAX {
            broken.triangles.push(*tri);
            broken.tri_face.push(face);
        }
    }
    broken
}

#[test]
fn fallback_hex_on_broken_surface() {
    let surface = broken_box();
    let cancel = CancelToken::new();
    // Mesher tet sendiri melaporkan galat, tidak panik.
    assert!(tetrahedralize(&surface, &cases::tet(2.5), &[], None, &cancel).is_err());
    let setup = ResolvedSetup {
        fixtures: vec![ResolvedFixture {
            id: "wall".into(),
            faces: vec![FACE_XMIN],
            kind: FixtureKind::Fixed,
        }],
        loads: vec![ResolvedLoad {
            id: "press".into(),
            faces: vec![FACE_ZMAX],
            kind: LoadKind::Pressure { mpa: 0.1 },
        }],
        mesh: cases::tet(2.5),
        exact_volume_mm3: None,
    };
    let report = run_static(&surface, &steel(), &setup, &cancel).unwrap();
    assert_eq!(report.mesh_stats.kind, MeshKind::Hex);
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.starts_with(FALLBACK_WARNING)),
        "{:?}",
        report.warnings
    );
    assert!(report.warnings.iter().all(|w| w.is_ascii()));
    assert!(report.max_von_mises_mpa > 0.0);
    assert!(serde_json::to_string(&report)
        .unwrap()
        .contains(FALLBACK_WARNING));
    // Mesh yang diminta terlalu halus juga jatuh ke hex, bukan galat.
    let tiny = MeshSettings {
        kind: MeshKind::Tet,
        cell_mm: None,
        target_elems: Some(50),
    };
    let mesh = build_mesh(&box_surface([400.0, 1.0, 1.0]), &tiny, None, &[], &cancel).unwrap();
    if let SimMesh::Hex(_) = &mesh {
        assert!(mesh
            .warnings()
            .iter()
            .any(|w| w.starts_with(FALLBACK_WARNING)));
    }
    // Permukaan sehat tidak memicu fallback.
    let good = build_mesh(
        &box_surface([40.0, 10.0, 10.0]),
        &cases::tet(2.5),
        None,
        &[],
        &cancel,
    )
    .unwrap();
    assert_eq!(good.kind(), MeshKind::Tet);
    assert!(good.warnings().is_empty());
}

#[test]
fn tet_static_symmetry_reactions_and_determinism() {
    // Tarik seragam dengan tiga bidang simetri pada mesh tet: tegangan tepat
    // F/A di semua node (menguji normal roller dan beban konsisten Tet10).
    let bar = box_surface([100.0, 10.0, 10.0]);
    let fixture = |id: &str, face: u32, kind: FixtureKind| ResolvedFixture {
        id: id.into(),
        faces: vec![face],
        kind,
    };
    let setup = ResolvedSetup {
        fixtures: vec![
            fixture("sx", FACE_XMIN, FixtureKind::Symmetry),
            fixture("sy", FACE_YMIN, FixtureKind::Symmetry),
            fixture("sz", FACE_ZMIN, FixtureKind::Roller),
        ],
        loads: vec![ResolvedLoad {
            id: "pull".into(),
            faces: vec![FACE_XMAX],
            kind: LoadKind::Force {
                newton: [1000.0, 0.0, 0.0],
            },
        }],
        mesh: cases::tet(5.0),
        exact_volume_mm3: Some(10_000.0),
    };
    let material = steel();
    let report = run_static(&bar, &material, &setup, &CancelToken::new()).unwrap();
    let field = report.nodal_field.as_ref().unwrap();
    for &vm in &field.von_mises {
        assert!((vm - 10.0).abs() < 1e-3, "von Mises {vm}");
    }
    let exact = 1000.0 * 100.0 / (material.young_mpa * 100.0);
    assert!((field.sample([100.0, 5.0, 5.0]).displacement[0] - exact).abs() < 1e-5 * exact);
    assert!((field.sample([50.0, 3.3, 7.1]).displacement[0] - 0.5 * exact).abs() < 1e-5 * exact);
    // Titik sedikit di luar dan jauh dari mesh tetap memberi nilai wajar.
    assert!((field.sample([100.2, 5.0, 5.0]).von_mises_mpa - 10.0).abs() < 1e-2);
    assert!((field.sample([500.0, 5.0, 5.0]).von_mises_mpa - 10.0).abs() < 1e-2);
    assert!((report.reactions[0].force_n[0] + 1000.0).abs() < 1e-3);
    assert!((report.mesh_stats.volume_mm3 - 10_000.0).abs() < 1e-6);

    // Gravitasi + tekanan pada mesh tet: reaksi menyeimbangkan beban.
    let loaded = ResolvedSetup {
        fixtures: vec![fixture("wall", FACE_XMIN, FixtureKind::Fixed)],
        loads: vec![
            ResolvedLoad {
                id: "g".into(),
                faces: vec![],
                kind: LoadKind::Gravity {
                    g: 9.81,
                    dir: [0.0, 0.0, -1.0],
                },
            },
            ResolvedLoad {
                id: "p".into(),
                faces: vec![FACE_ZMAX],
                kind: LoadKind::Pressure { mpa: 0.05 },
            },
        ],
        mesh: cases::tet(5.0),
        exact_volume_mm3: Some(10_000.0),
    };
    let first = run_static(&bar, &material, &loaded, &CancelToken::new()).unwrap();
    let weight = 10_000.0 * material.density_g_cm3 * 1.0e-6 * 9.81;
    let r = first.reactions[0].force_n;
    assert!(
        (r[2] - (weight + 0.05 * 1000.0)).abs() < 1e-4 * 50.0,
        "{r:?}"
    );
    assert!(r[0].abs() < 1e-4 && r[1].abs() < 1e-4);
    // Deterministik: dua kali jalan, laporan dan medan identik.
    let second = run_static(&bar, &material, &loaded, &CancelToken::new()).unwrap();
    assert_eq!(
        serde_json::to_string(&first).unwrap(),
        serde_json::to_string(&second).unwrap()
    );
    assert_eq!(first, second);
}

#[test]
fn thermal_setup_serde_roundtrip_and_schema() {
    let text = r#"{"boundary":[
        {"id":"hot","faces":"<X","kind":"temperature","celsius":120},
        {"id":"q","faces":">Z","kind":"heat_flux","w_per_mm2":0.01},
        {"id":"air","faces":"all[kind=cylinder]","kind":"convection","h_w_mm2k":2.5e-5,"ambient_c":22}
    ]}"#;
    let setup: ThermalSetup = serde_json::from_str(text).unwrap();
    assert_eq!(setup.boundary.len(), 3);
    assert_eq!(
        setup.boundary[0].kind,
        ThermalBcKind::Temperature { celsius: 120.0 }
    );
    let back: ThermalSetup = serde_json::from_str(&serde_json::to_string(&setup).unwrap()).unwrap();
    assert_eq!(setup, back);
    let _one: ThermalBc = setup.boundary[2].clone();
    for bad in [
        r#"{"boundary":[{"id":"a","faces":"<X","kind":"temperature","celsius":1,"extra":2}]}"#,
        r#"{"boundary":[{"id":"a","faces":"<X","kind":"radiation","celsius":1}]}"#,
        r#"{"boundary":[],"oops":1}"#,
    ] {
        assert!(serde_json::from_str::<ThermalSetup>(bad).is_err(), "{bad}");
    }
    let schema = serde_json::to_string(&schemars::schema_for!(ThermalSetup)).unwrap();
    for needle in [
        "temperature",
        "heat_flux",
        "convection",
        "h_w_mm2k",
        "ambient_c",
        "w_per_mm2",
        "celsius",
    ] {
        assert!(schema.contains(needle), "skema tidak memuat {needle}");
    }
    assert!(schema.is_ascii());
    // Setup statik lama tetap terbaca dan diserialisasi sama (tanpa field baru).
    let old = r#"{"body":"b","fixtures":[{"id":"a","faces":"<Z","kind":"fixed"}],"loads":[],"mesh":{"kind":"tet","cell_mm":2.0}}"#;
    let parsed: ducad_sim::SimSetup = serde_json::from_str(old).unwrap();
    assert_eq!(parsed.mesh.kind, MeshKind::Tet);
    assert_eq!(
        serde_json::to_string(&parsed).unwrap(),
        r#"{"body":"b","fixtures":[{"id":"a","faces":"<Z","kind":"fixed"}],"loads":[],"mesh":{"kind":"tet","cell_mm":2.0}}"#
    );
}
