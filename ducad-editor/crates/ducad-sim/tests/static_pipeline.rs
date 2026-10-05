//! Tes ujung-ke-ujung alur studi statik: determinisme, galat, statika beban,
//! reaksi, simetri, dan patch test pada mesh voxel.

use ducad_sim::assemble::assemble_stiffness;
use ducad_sim::bc;
use ducad_sim::benchmark::cases::steel;
use ducad_sim::benchmark::{
    box_surface, plate_with_hole, FACE_HOLE, FACE_XMAX, FACE_XMIN, FACE_YMIN, FACE_ZMAX, FACE_ZMIN,
};
use ducad_sim::element::hex8::elasticity_matrix;
use ducad_sim::element::HexModel;
use ducad_sim::post::{internal_forces, nodal_stress};
use ducad_sim::solver::{solve_pcg, BlockJacobi3, CgOptions, TwoLevel};
use ducad_sim::{
    run_static, voxelize, CancelToken, FixtureKind, LoadKind, MeshSettings, ResolvedFixture,
    ResolvedLoad, ResolvedSetup, SimError, VoxelModel,
};

fn mesh(cell: f64) -> MeshSettings {
    MeshSettings {
        cell_mm: Some(cell),
        ..MeshSettings::default()
    }
}

fn fixture(id: &str, face: u32, kind: FixtureKind) -> ResolvedFixture {
    ResolvedFixture {
        id: id.into(),
        faces: vec![face],
        kind,
    }
}

fn load(id: &str, faces: &[u32], kind: LoadKind) -> ResolvedLoad {
    ResolvedLoad {
        id: id.into(),
        faces: faces.to_vec(),
        kind,
    }
}

fn bar_setup(cell: f64, fixtures: Vec<ResolvedFixture>, loads: Vec<ResolvedLoad>) -> ResolvedSetup {
    ResolvedSetup {
        fixtures,
        loads,
        mesh: mesh(cell),
        exact_volume_mm3: Some(10_000.0),
    }
}

fn bar() -> ducad_sim::SurfaceMesh {
    box_surface([100.0, 10.0, 10.0])
}

fn pull() -> ResolvedLoad {
    load(
        "pull",
        &[FACE_XMAX],
        LoadKind::Force {
            newton: [1000.0, 0.0, 0.0],
        },
    )
}

#[test]
fn static_determinism_identical_reports() {
    let surface = plate_with_hole([60.0, 40.0, 4.0], 12.0, 64);
    let setup = ResolvedSetup {
        fixtures: vec![fixture("fix", FACE_XMIN, FixtureKind::Fixed)],
        loads: vec![
            load(
                "pull",
                &[FACE_XMAX],
                LoadKind::Force {
                    newton: [500.0, 20.0, -30.0],
                },
            ),
            load("press", &[FACE_ZMAX], LoadKind::Pressure { mpa: 0.02 }),
            load(
                "pin",
                &[FACE_HOLE],
                LoadKind::Bearing {
                    newton: [0.0, 150.0, 0.0],
                },
            ),
            load(
                "g",
                &[],
                LoadKind::Gravity {
                    g: 9.81,
                    dir: [0.0, 0.0, -1.0],
                },
            ),
        ],
        mesh: mesh(2.0),
        exact_volume_mm3: None,
    };
    let a = run_static(&surface, &steel(), &setup, &CancelToken::new()).unwrap();
    let b = run_static(&surface, &steel(), &setup, &CancelToken::new()).unwrap();
    let ja = serde_json::to_string(&a).unwrap();
    let jb = serde_json::to_string(&b).unwrap();
    assert_eq!(ja, jb);
    assert_eq!(a, b, "medan nodal harus identik bit demi bit");
    // Medan nodal tidak ikut diserialisasi.
    assert!(!ja.contains("nodal_field") && !ja.contains("displacement\""));
    let back: ducad_sim::SimReport = serde_json::from_str(&ja).unwrap();
    assert_eq!(
        back.max_von_mises_mpa.to_bits(),
        a.max_von_mises_mpa.to_bits()
    );
    assert!(back.nodal_field.is_none());
}

#[test]
fn static_errors_are_reported_with_codes() {
    let cancel = CancelToken::new();
    let fixed = || vec![fixture("fix", FACE_XMIN, FixtureKind::Fixed)];
    // Tanpa fixture.
    let err = run_static(
        &bar(),
        &steel(),
        &bar_setup(5.0, vec![], vec![pull()]),
        &cancel,
    )
    .unwrap_err();
    assert_eq!(err.code(), "SIM_UNDERCONSTRAINED");
    assert!(err.hint().unwrap().contains("Fixed"));
    // Roller saja pada satu bidang: gerak tegar dalam bidang masih bebas.
    let setup = bar_setup(
        5.0,
        vec![fixture("roll", FACE_XMIN, FixtureKind::Roller)],
        vec![load(
            "side",
            &[FACE_XMAX],
            LoadKind::Force {
                newton: [0.0, 100.0, 0.0],
            },
        )],
    );
    let err = run_static(&bar(), &steel(), &setup, &cancel).unwrap_err();
    assert_eq!(err.code(), "SIM_UNDERCONSTRAINED", "{err}");
    // Face tidak ada, face kosong, material tidak sah, beban tidak hingga.
    let bad_face = bar_setup(
        5.0,
        fixed(),
        vec![load("f", &[42], LoadKind::Pressure { mpa: 1.0 })],
    );
    assert_eq!(
        run_static(&bar(), &steel(), &bad_face, &cancel)
            .unwrap_err()
            .code(),
        "SIM_INVALID_SETUP"
    );
    let no_face = bar_setup(
        5.0,
        fixed(),
        vec![load("f", &[], LoadKind::Pressure { mpa: 1.0 })],
    );
    assert_eq!(
        run_static(&bar(), &steel(), &no_face, &cancel)
            .unwrap_err()
            .code(),
        "SIM_INVALID_SETUP"
    );
    let mut soft = steel();
    soft.poisson = 0.5;
    let ok = bar_setup(5.0, fixed(), vec![pull()]);
    assert_eq!(
        run_static(&bar(), &soft, &ok, &cancel).unwrap_err().code(),
        "SIM_INVALID_SETUP"
    );
    let nan = bar_setup(
        5.0,
        fixed(),
        vec![load(
            "f",
            &[FACE_XMAX],
            LoadKind::Force {
                newton: [f64::NAN, 0.0, 0.0],
            },
        )],
    );
    assert_eq!(
        run_static(&bar(), &steel(), &nan, &cancel)
            .unwrap_err()
            .code(),
        "SIM_INVALID_SETUP"
    );
    let zero_axis = bar_setup(
        5.0,
        fixed(),
        vec![load(
            "t",
            &[FACE_XMAX],
            LoadKind::Torque {
                axis_point: [0.0; 3],
                axis_dir: [0.0; 3],
                newton_mm: 1.0,
            },
        )],
    );
    assert_eq!(
        run_static(&bar(), &steel(), &zero_axis, &cancel)
            .unwrap_err()
            .code(),
        "SIM_INVALID_SETUP"
    );
    // Dibatalkan.
    cancel.cancel();
    assert_eq!(
        run_static(&bar(), &steel(), &ok, &cancel).unwrap_err(),
        SimError::Cancelled
    );
}

#[test]
fn static_no_loads_gives_zero_result_with_warning() {
    let setup = bar_setup(
        5.0,
        vec![fixture("fix", FACE_XMIN, FixtureKind::Fixed)],
        vec![],
    );
    let report = run_static(&bar(), &steel(), &setup, &CancelToken::new()).unwrap();
    assert_eq!(report.max_von_mises_mpa, 0.0);
    assert_eq!(report.max_displacement_mm, 0.0);
    assert_eq!(report.safety_factor, 1.0e6);
    assert!(report.warnings.iter().any(|w| w.contains("no loads")));
}

#[test]
fn static_symmetry_fixtures_give_uniform_tension() {
    // Tiga bidang simetri saling tegak lurus mengunci semua gerak tegar tanpa
    // menahan kontraksi Poisson: tegangan seragam tepat F/A di mana-mana.
    let setup = bar_setup(
        2.5,
        vec![
            fixture("sx", FACE_XMIN, FixtureKind::Symmetry),
            fixture("sy", FACE_YMIN, FixtureKind::Symmetry),
            fixture("sz", FACE_ZMIN, FixtureKind::Roller),
        ],
        vec![pull()],
    );
    let material = steel();
    let report = run_static(&bar(), &material, &setup, &CancelToken::new()).unwrap();
    let field = report.nodal_field.as_ref().unwrap();
    for &vm in &field.von_mises {
        assert!((vm - 10.0).abs() < 1e-3, "von Mises {vm}");
    }
    let tip = field.sample([100.0, 5.0, 5.0]).displacement[0];
    let exact = 1000.0 * 100.0 / (material.young_mpa * 100.0);
    assert!((tip - exact).abs() < 1e-5 * exact, "{tip} vs {exact}");
    // Kontraksi lateral bebas: u_y(y = 10) = −ν·ε·10.
    let lateral = field.sample([50.0, 10.0, 5.0]).displacement[1];
    assert!((lateral + material.poisson * exact / 100.0 * 10.0).abs() < 1e-5 * exact);
    // Reaksi: bidang X memikul seluruh beban, dua bidang lain nol.
    let r = &report.reactions;
    assert_eq!(r.len(), 3);
    assert!((r[0].force_n[0] + 1000.0).abs() < 1e-3, "{:?}", r[0]);
    for other in &r[1..] {
        assert!(other.force_n.iter().all(|f| f.abs() < 1e-3), "{other:?}");
    }
    assert!((report.safety_factor - 25.0).abs() < 1e-2);
}

#[test]
fn static_reactions_balance_all_loads() {
    let material = steel();
    let setup = bar_setup(
        2.5,
        vec![fixture("wall", FACE_XMIN, FixtureKind::Fixed)],
        vec![
            load(
                "tip",
                &[FACE_XMAX],
                LoadKind::Force {
                    newton: [30.0, -20.0, -100.0],
                },
            ),
            load("press", &[FACE_ZMAX], LoadKind::Pressure { mpa: 0.05 }),
            load(
                "g",
                &[],
                LoadKind::Gravity {
                    g: 9.81,
                    dir: [0.0, 0.0, -2.0],
                },
            ),
            load(
                "twist",
                &[FACE_XMAX],
                LoadKind::Torque {
                    axis_point: [0.0, 5.0, 5.0],
                    axis_dir: [1.0, 0.0, 0.0],
                    newton_mm: 400.0,
                },
            ),
        ],
    );
    let report = run_static(&bar(), &material, &setup, &CancelToken::new()).unwrap();
    let weight = 10_000.0 * material.density_g_cm3 * 1.0e-6 * 9.81;
    let applied = [30.0, -20.0, -100.0 - 0.05 * 1000.0 - weight];
    let r = report.reactions[0].force_n;
    for a in 0..3 {
        assert!(
            (r[a] + applied[a]).abs() < 1e-4 * 150.0,
            "sumbu {a}: reaksi {r:?} vs beban {applied:?}"
        );
    }
    assert!((report.mesh_stats.volume_mm3 - 10_000.0).abs() < 1e-6);
    assert_eq!(report.mesh_stats.elements, 640);
}

fn nodal_resultant(model: &VoxelModel, force: &[f64], about: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let mut f = [0.0; 3];
    let mut m = [0.0; 3];
    for (n, p) in model.mesh.nodes.iter().enumerate() {
        let g = [force[3 * n], force[3 * n + 1], force[3 * n + 2]];
        let r = [p[0] - about[0], p[1] - about[1], p[2] - about[2]];
        for a in 0..3 {
            f[a] += g[a];
        }
        m[0] += r[1] * g[2] - r[2] * g[1];
        m[1] += r[2] * g[0] - r[0] * g[2];
        m[2] += r[0] * g[1] - r[1] * g[0];
    }
    (f, m)
}

fn loads_only(model: &VoxelModel, kind: LoadKind, face: u32) -> Vec<f64> {
    let setup = ResolvedSetup {
        fixtures: vec![],
        loads: vec![load("l", &[face], kind)],
        mesh: MeshSettings::default(),
        exact_volume_mm3: None,
    };
    bc::build(model, &steel(), &setup, &[]).unwrap().force
}

#[test]
fn static_loads_are_statically_equivalent() {
    let cancel = CancelToken::new();
    let size = [60.0, 40.0, 6.0];
    let surface = plate_with_hole(size, 16.0, 96);
    let model = voxelize(&surface, &mesh(1.5), None, &cancel).unwrap();
    let centre = [30.0, 20.0, 3.0];

    // Gaya: resultan tepat.
    let f = loads_only(
        &model,
        LoadKind::Force {
            newton: [12.0, -7.0, 3.0],
        },
        FACE_XMAX,
    );
    let (sum, _) = nodal_resultant(&model, &f, centre);
    for (s, e) in sum.iter().zip([12.0, -7.0, 3.0]) {
        assert!((s - e).abs() < 1e-9);
    }
    // Tekanan pada face atas (berlubang): −p × luas sebenarnya, arah −Z.
    let f = loads_only(&model, LoadKind::Pressure { mpa: 0.5 }, FACE_ZMAX);
    let (sum, _) = nodal_resultant(&model, &f, centre);
    let area = 60.0 * 40.0 - std::f64::consts::PI * 64.0;
    assert!(sum[0].abs() < 1e-9 && sum[1].abs() < 1e-9);
    assert!((sum[2] + 0.5 * area).abs() < 2e-3 * 0.5 * area, "{sum:?}");
    // Tekanan pada dinding lubang: resultan nol (permukaan tertutup melingkar).
    let f = loads_only(&model, LoadKind::Pressure { mpa: 1.0 }, FACE_HOLE);
    let (sum, _) = nodal_resultant(&model, &f, centre);
    assert!(sum.iter().all(|s| s.abs() < 1e-6), "{sum:?}");
    assert!(f.iter().any(|v| v.abs() > 1e-3));
    // Torsi pada dinding lubang: kopel murni dengan momen tepat.
    let torque = LoadKind::Torque {
        axis_point: centre,
        axis_dir: [0.0, 0.0, 2.0],
        newton_mm: 900.0,
    };
    let f = loads_only(&model, torque, FACE_HOLE);
    let (sum, moment) = nodal_resultant(&model, &f, centre);
    assert!(sum.iter().all(|s| s.abs() < 1e-8), "{sum:?}");
    assert!((moment[2] - 900.0).abs() < 1e-8, "{moment:?}");
    // Bearing: resultan tepat, dan hanya separuh lubang di sisi +X yang ditekan.
    let f = loads_only(
        &model,
        LoadKind::Bearing {
            newton: [250.0, 0.0, 0.0],
        },
        FACE_HOLE,
    );
    let (sum, _) = nodal_resultant(&model, &f, centre);
    assert!(
        (sum[0] - 250.0).abs() < 1e-9 && sum[1].abs() < 1e-9 && sum[2].abs() < 1e-9,
        "{sum:?}"
    );
    for (n, p) in model.mesh.nodes.iter().enumerate() {
        let mag = f[3 * n].abs() + f[3 * n + 1].abs() + f[3 * n + 2].abs();
        if mag > 1e-9 {
            assert!(
                p[0] > centre[0] - 1.5,
                "node terbebani di sisi yang salah: {p:?}"
            );
        }
    }
    // Remote: gaya di titik jauh → gaya + momen ekuivalen (momen nol terhadap titik itu).
    let point = [90.0, 20.0, 30.0];
    let remote = LoadKind::Remote {
        point,
        newton: [5.0, -40.0, 15.0],
    };
    let f = loads_only(&model, remote, FACE_XMAX);
    let (sum, moment) = nodal_resultant(&model, &f, point);
    for (s, e) in sum.iter().zip([5.0, -40.0, 15.0]) {
        assert!((s - e).abs() < 1e-9);
    }
    assert!(moment.iter().all(|m| m.abs() < 1e-3), "{moment:?}");
}

#[test]
fn element_hex8_patch_test_on_voxel_mesh() {
    // Patch test regangan konstan pada mesh 3×3×3: medan linier dipasang di
    // semua node; gaya dalam di node interior harus nol dan tegangan nodal
    // sama persis dengan D·ε.
    let cancel = CancelToken::new();
    let model = voxelize(&box_surface([6.0, 4.5, 3.0]), &mesh(1.5), None, &cancel).unwrap();
    assert_eq!(model.mesh.elems.len(), 4 * 3 * 2);
    let (young, nu) = (70_000.0, 0.33);
    let hex = HexModel::new(&model.mesh, young, nu).unwrap();
    let a = [
        [2.0e-4, -1.0e-4, 3.0e-4],
        [5.0e-5, 4.0e-4, -2.0e-4],
        [1.0e-4, 2.5e-4, -3.0e-4],
    ];
    let mut u = vec![0.0; 3 * model.mesh.nodes.len()];
    for (n, p) in model.mesh.nodes.iter().enumerate() {
        for c in 0..3 {
            u[3 * n + c] =
                a[c][0] * p[0] + a[c][1] * p[1] + a[c][2] * p[2] + 0.01 * (c as f64 + 1.0);
        }
    }
    let strain = [
        a[0][0],
        a[1][1],
        a[2][2],
        a[0][1] + a[1][0],
        a[1][2] + a[2][1],
        a[0][2] + a[2][0],
    ];
    let d = elasticity_matrix(young, nu);
    let expect: Vec<f64> = (0..6)
        .map(|r| (0..6).map(|c| d[r][c] * strain[c]).sum())
        .collect();
    for s in nodal_stress(&hex, &u) {
        for r in 0..6 {
            assert!(
                (s[r] - expect[r]).abs() < 1e-9 * young * 1e-3,
                "{s:?} vs {expect:?}"
            );
        }
    }
    let f = internal_forces(&hex, &u);
    let scale = f.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let mut interior = 0;
    for (n, p) in model.mesh.nodes.iter().enumerate() {
        let inside =
            p[0] > 0.1 && p[0] < 5.9 && p[1] > 0.1 && p[1] < 4.4 && p[2] > 0.1 && p[2] < 2.9;
        if inside {
            interior += 1;
            for c in 0..3 {
                assert!(
                    f[3 * n + c].abs() < 1e-10 * scale,
                    "node interior {n}: {}",
                    f[3 * n + c]
                );
            }
        }
    }
    assert_eq!(interior, 3 * 2);
    assert!(scale > 0.0);
}

#[test]
fn solver_cg_two_level_matches_block_jacobi() {
    // Prekondisi dua-tingkat harus memberi solusi yang sama dengan Jacobi
    // blok, dengan iterasi jauh lebih sedikit pada kantilever tipis.
    let cancel = CancelToken::new();
    let surface = box_surface([120.0, 20.0, 4.0]);
    let model = voxelize(&surface, &mesh(2.0), None, &cancel).unwrap();
    let setup = ResolvedSetup {
        fixtures: vec![fixture("wall", FACE_XMIN, FixtureKind::Fixed)],
        loads: vec![load(
            "tip",
            &[FACE_XMAX],
            LoadKind::Force {
                newton: [0.0, 0.0, -10.0],
            },
        )],
        mesh: MeshSettings::default(),
        exact_volume_mm3: None,
    };
    let conditions = bc::build(&model, &steel(), &setup, &[]).unwrap();
    let hex = HexModel::new(&model.mesh, 210_000.0, 0.3).unwrap();
    let k = assemble_stiffness(&hex, &conditions.fixed, &conditions.penalties)
        .unwrap()
        .k;
    let mut rhs = conditions.force.clone();
    for (b, &fixed) in rhs.iter_mut().zip(&conditions.fixed) {
        if fixed {
            *b = 0.0;
        }
    }
    let options = CgOptions {
        rel_tol: 1e-10,
        max_iter: 20_000,
        check_every: 50,
    };
    let (u_jacobi, jacobi) =
        solve_pcg(&k, &rhs, &BlockJacobi3::new(&k), &options, &cancel).unwrap();
    let two_level = TwoLevel::build(&k, &conditions.fixed, &model.mesh.nodes, 2.0).unwrap();
    assert!(two_level.aggregates() > 1);
    let (u_two, two) = solve_pcg(&k, &rhs, &two_level, &options, &cancel).unwrap();
    let scale = u_jacobi.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    for (a, b) in u_jacobi.iter().zip(&u_two) {
        assert!((a - b).abs() < 1e-6 * scale);
    }
    println!(
        "iterasi: Jacobi blok {}, dua-tingkat {}",
        jacobi.iterations, two.iterations
    );
    assert!(two.iterations * 2 < jacobi.iterations);
}

#[test]
fn static_field_sample_handles_points_off_the_mesh() {
    let setup = bar_setup(
        2.5,
        vec![fixture("fix", FACE_XMIN, FixtureKind::Fixed)],
        vec![pull()],
    );
    let report = run_static(&bar(), &steel(), &setup, &CancelToken::new()).unwrap();
    let field = report.nodal_field.as_ref().unwrap();
    assert_eq!(field.nodes.len(), field.von_mises.len());
    assert_eq!(field.nodes.len(), field.displacement.len());
    let on_tip = field.sample([100.0, 5.0, 5.0]);
    let beyond = field.sample([100.4, 5.0, 5.0]);
    let far = field.sample([500.0, 5.0, 5.0]);
    assert!((on_tip.displacement[0] - beyond.displacement[0]).abs() < 1e-12);
    assert!((on_tip.displacement[0] - far.displacement[0]).abs() < 1e-9);
    assert!(on_tip.von_mises_mpa > 5.0);
    let nan = field.sample([f64::NAN, 0.0, 0.0]);
    assert_eq!(nan.von_mises_mpa, 0.0);
}
