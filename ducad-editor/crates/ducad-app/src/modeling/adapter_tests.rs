//! P0.8 — tes regresi adapter GUI → `ducad_engine::compute`.
//!
//! Konstanta `*_VOLUME` diambil dengan menjalankan tes ini pada commit
//! SEBELUM operasi GUI diubah menjadi adapter tipis (commit 4638e98),
//! lalu ditanam di sini: refactor tidak boleh mengubah geometri hasil.

use std::collections::HashSet;

use ducad_core::hole::{HoleKind, HoleSpec, IsoMetricThread};
use ducad_kernel::PickRay;
use ducad_sketch::constraint::Constraint;
use ducad_sketch::Entity;
use glam::DVec2;

use crate::app::DuCADApp;
use crate::model::BooleanKind;

const REL_TOL: f64 = 1e-6;

fn assert_volume(label: &str, got: f64, want: f64) {
    println!("{label}: {got:.9}");
    assert!(
        (got - want).abs() / want.abs() < REL_TOL,
        "{label}: {got} vs {want}"
    );
}

fn total_volume(app: &DuCADApp) -> f64 {
    app.model
        .geometry
        .values()
        .map(|g| g.shape.volume().abs())
        .sum()
}

fn add_rect(
    app: &mut DuCADApp,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
) -> HashSet<ducad_sketch::EntityId> {
    let c = [
        DVec2::new(x0, y0),
        DVec2::new(x1, y0),
        DVec2::new(x1, y1),
        DVec2::new(x0, y1),
    ];
    (0..4)
        .map(|i| {
            app.sketch_mut()
                .entities
                .insert(Entity::line(c[i], c[(i + 1) % 4]))
        })
        .collect()
}

fn add_box(app: &mut DuCADApp, name: &str, size: [f64; 3], at: [f64; 3]) -> ducad_core::BodyId {
    let geo = ducad_engine::compute::primitive(
        &ducad_engine::compute::PrimitiveShape::Box {
            size,
            centered: false,
        },
        at,
    )
    .unwrap();
    let id = app.model.doc.add_body(name);
    app.model.geometry.insert(id, geo);
    id
}

fn top_face_ray(x: f64, y: f64) -> PickRay {
    PickRay {
        origin: (x, y, 100.0),
        dir: (0.0, 0.0, -1.0),
    }
}

const EXTRUDE_VOLUME: f64 = 19200.0;
const REVOLVE_VOLUME: f64 = 1963.495408494;
const UNION_VOLUME: f64 = 1500.0;
const FILLET_VOLUME: f64 = 7804.696111128;
const CHAMFER_VOLUME: f64 = 7562.666666667;
const SHELL_VOLUME: f64 = 3392.0;
const HOLE_VOLUME: f64 = 19009.933644458;
const PATTERN_VOLUME: f64 = 6000.0;

#[test]
fn adapter_extrude_selected_volume() {
    let mut app = DuCADApp::new_for_test();
    app.selected = add_rect(&mut app, 0.0, 0.0, 60.0, 40.0);
    app.extrude_distance_input = "8".to_string();
    app.extrude_selected();
    assert_eq!(app.model.doc.bodies.len(), 1, "{:?}", app.model_status);
    assert_volume("extrude", total_volume(&app), EXTRUDE_VOLUME);
}

#[test]
fn adapter_revolve_selected_volume() {
    let mut app = DuCADApp::new_for_test();
    app.selected = add_rect(&mut app, 10.0, 0.0, 15.0, 10.0);
    assert!(
        app.revolve_selected((0.0, 0.0), (0.0, 1.0), Some(180.0)),
        "{:?}",
        app.model_status
    );
    assert_volume("revolve", total_volume(&app), REVOLVE_VOLUME);
}

#[test]
fn adapter_boolean_selected_volume() {
    let mut app = DuCADApp::new_for_test();
    let a = add_box(&mut app, "A", [10.0, 10.0, 10.0], [0.0, 0.0, 0.0]);
    let b = add_box(&mut app, "B", [10.0, 10.0, 10.0], [5.0, 0.0, 0.0]);
    app.selected_bodies = [a, b].into_iter().collect();
    app.boolean_selected(BooleanKind::Union, "Union", "Union");
    assert_eq!(app.model.doc.bodies.len(), 1, "{:?}", app.model_status);
    assert_volume("union", total_volume(&app), UNION_VOLUME);
}

#[test]
fn adapter_fillet_selected_body_volume() {
    let mut app = DuCADApp::new_for_test();
    let id = add_box(&mut app, "Box", [20.0, 20.0, 20.0], [0.0; 3]);
    app.selected_bodies = [id].into_iter().collect();
    app.fillet_radius_input = "2".to_string();
    app.fillet_selected_body();
    assert!(app.model_status.is_none(), "{:?}", app.model_status);
    assert_volume("fillet", total_volume(&app), FILLET_VOLUME);
}

#[test]
fn adapter_chamfer_selected_body_volume() {
    let mut app = DuCADApp::new_for_test();
    let id = add_box(&mut app, "Box", [20.0, 20.0, 20.0], [0.0; 3]);
    app.selected_bodies = [id].into_iter().collect();
    app.chamfer_distance_input = "2".to_string();
    app.chamfer_selected_body();
    assert!(app.model_status.is_none(), "{:?}", app.model_status);
    assert_volume("chamfer", total_volume(&app), CHAMFER_VOLUME);
}

#[test]
fn adapter_shell_selected_body_volume() {
    let mut app = DuCADApp::new_for_test();
    let id = add_box(&mut app, "Box", [20.0, 20.0, 20.0], [0.0; 3]);
    app.selected_bodies = [id].into_iter().collect();
    app.selected_faces = vec![top_face_ray(10.0, 10.0)];
    app.shell_thickness_input = "2".to_string();
    app.shell_selected_body();
    assert!(app.model_status.is_none(), "{:?}", app.model_status);
    assert_volume("shell", total_volume(&app), SHELL_VOLUME);
}

#[test]
fn adapter_apply_hole_wizard_volume() {
    let mut app = DuCADApp::new_for_test();
    let id = add_box(&mut app, "Plate", [60.0, 40.0, 8.0], [0.0; 3]);
    let ray = top_face_ray(30.0, 20.0);
    let hit =
        ducad_kernel::pick_face_details(&app.model.geometry[id].shape, ray).expect("face atas");
    app.active_face = Some((id, ray, hit));
    let mut spec = HoleSpec::for_iso(IsoMetricThread::M5, HoleKind::Simple, 8.0);
    spec.is_through = true;
    app.apply_hole_wizard(spec);
    assert_volume("hole", total_volume(&app), HOLE_VOLUME);
}

#[test]
fn adapter_apply_pattern_3d_volume() {
    let mut app = DuCADApp::new_for_test();
    let id = add_box(&mut app, "Box", [10.0, 10.0, 10.0], [0.0; 3]);
    app.selected_bodies = [id].into_iter().collect();
    app.pattern_kind = ducad_ui::PatternKind::Linear;
    app.pattern_count_x = 3;
    app.pattern_pitch_x = 20.0;
    app.pattern_count_y = 2;
    app.pattern_pitch_y = 20.0;
    app.pattern_count_z = 1;
    app.apply_pattern_3d();
    assert_eq!(app.model.doc.bodies.len(), 6, "{:?}", app.model_status);
    assert_volume("pattern", total_volume(&app), PATTERN_VOLUME);
}

#[test]
fn adapter_apply_constraint_commits_or_reports_residual() {
    let mut app = DuCADApp::new_for_test();
    let a = app
        .sketch_mut()
        .entities
        .insert(Entity::line(DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0)));
    let b = app
        .sketch_mut()
        .entities
        .insert(Entity::line(DVec2::new(0.0, 0.0), DVec2::new(3.0, 9.0)));
    app.apply_constraint(Constraint::Perpendicular { a, b });
    assert!(
        app.constraint_status.is_none(),
        "{:?}",
        app.constraint_status
    );
    assert_eq!(app.sketch().constraints.len(), 1);

    // Constraint mustahil (jarak dua titik yang sama = 5) tidak konvergen:
    // sketch tidak berubah dan pesan residual lama tetap dipakai.
    let p = ducad_sketch::constraint::PointRef::LineStart(a);
    app.apply_constraint(Constraint::Distance {
        a: p,
        b: p,
        value: 5.0,
    });
    let status = app.constraint_status.clone().expect("harus gagal");
    assert!(
        status.starts_with("Constraint gagal diselesaikan (sisa residual "),
        "{status}"
    );
    assert!(
        status.ends_with(" — dibatalkan, sketch tidak berubah"),
        "{status}"
    );
    assert_eq!(app.sketch().constraints.len(), 1);
}

#[test]
fn adding_conflicting_constraint_shows_error_and_leaves_sketch_unchanged() {
    use ducad_sketch::constraint::PointRef;
    use ducad_sketch::entity::{PathSeg, Subpath};

    let mut app = DuCADApp::new_for_test();
    let p = app.sketch_mut().entities.insert(Entity::Path {
        subpaths: vec![Subpath {
            start: DVec2::new(0.0, 0.0),
            segs: vec![PathSeg::Line {
                end: DVec2::new(10.0, 0.0),
            }],
            closed: false,
        }],
        is_construction: false,
    });
    let n0 = PointRef::PathNode {
        id: p,
        sub: 0,
        node: 0,
    };
    let n1 = PointRef::PathNode {
        id: p,
        sub: 0,
        node: 1,
    };

    // Tambah constraint HorizontalPoints pada dua node
    app.apply_constraint(Constraint::HorizontalPoints { a: n0, b: n1 });
    assert!(app.constraint_status.is_none(), "{:?}", app.constraint_status);
    assert_eq!(app.sketch().constraints.len(), 1);

    // Tambah constraint VerticalPoints yang bertentangan pada dua node yang sama
    app.apply_constraint(Constraint::VerticalPoints { a: n0, b: n1 });
    let status = app.constraint_status.clone().expect("harus menghasilkan pesan error");
    assert!(
        status.contains("dibatalkan, sketch tidak berubah"),
        "Pesan status harus mengindikasikan pembatalan: {status}"
    );
    assert_eq!(
        app.sketch().constraints.len(),
        1,
        "Jumlah constraint sketch harus tidak berubah setelah constraint yang bertentangan ditolak"
    );
}

#[test]
fn editing_source_path_marks_extrude_stale() {
    let mut app = DuCADApp::new_for_test();
    let sub = ducad_sketch::Subpath {
        start: DVec2::new(0.0, 0.0),
        segs: vec![
            ducad_sketch::PathSeg::Line { end: DVec2::new(50.0, 0.0) },
            ducad_sketch::PathSeg::Line { end: DVec2::new(50.0, 30.0) },
            ducad_sketch::PathSeg::Line { end: DVec2::new(0.0, 30.0) },
        ],
        closed: true,
    };
    let eid = app.sketch_mut().entities.insert(Entity::path(vec![sub]));
    app.sketch_mut().entity_names.insert(eid, "logo".to_string());
    app.sketch_mut().touch(eid);

    let f_extrude = app.record_extrude_feature_with_sources(10.0, false, vec!["logo".to_string()], true);
    assert_eq!(
        app.parametric_dag.get_feature(f_extrude).unwrap().status,
        ducad_core::parametric::FeatureStatus::Valid
    );

    // Edit source path melalui command sketch
    let cmd = ducad_sketch::commands::TranslateEntities::new("Geser", vec![eid], DVec2::new(5.0, 5.0));
    app.execute_sketch_command(Box::new(cmd));

    // Feature harus berstatus Stale dan DAG needs_regeneration() == true
    assert_eq!(
        app.parametric_dag.get_feature(f_extrude).unwrap().status,
        ducad_core::parametric::FeatureStatus::Stale
    );
    assert!(app.parametric_dag.needs_regeneration());
}

#[test]
fn regenerate_updates_body_volume() {
    let mut app = DuCADApp::new_for_test();
    let sub = ducad_sketch::Subpath {
        start: DVec2::new(0.0, 0.0),
        segs: vec![
            ducad_sketch::PathSeg::Line { end: DVec2::new(40.0, 0.0) },
            ducad_sketch::PathSeg::Line { end: DVec2::new(40.0, 20.0) },
            ducad_sketch::PathSeg::Line { end: DVec2::new(0.0, 20.0) },
        ],
        closed: true,
    };
    let eid = app.sketch_mut().entities.insert(Entity::path(vec![sub]));
    app.sketch_mut().entity_names.insert(eid, "rect".to_string());
    app.sketch_mut().touch(eid);

    let f_extrude = app.record_extrude_feature_with_sources(10.0, false, vec!["rect".to_string()], false);
    assert!(app.regenerate_parametric_model().is_ok());
    assert_volume("initial", total_volume(&app), 8000.0);
    assert_eq!(
        app.parametric_dag.get_feature(f_extrude).unwrap().status,
        ducad_core::parametric::FeatureStatus::Valid
    );

    // Edit sketch: perbesar lebar dari 40 menjadi 80 (volume jadi 80 * 20 * 10 = 16000)
    let new_sub = ducad_sketch::Subpath {
        start: DVec2::new(0.0, 0.0),
        segs: vec![
            ducad_sketch::PathSeg::Line { end: DVec2::new(80.0, 0.0) },
            ducad_sketch::PathSeg::Line { end: DVec2::new(80.0, 20.0) },
            ducad_sketch::PathSeg::Line { end: DVec2::new(0.0, 20.0) },
        ],
        closed: true,
    };
    let cmd = ducad_sketch::commands::UpdateEntity::new("Ubah Profil", eid, Entity::path(vec![new_sub]));
    app.execute_sketch_command(Box::new(cmd));

    assert_eq!(
        app.parametric_dag.get_feature(f_extrude).unwrap().status,
        ducad_core::parametric::FeatureStatus::Stale
    );
    assert!(app.parametric_dag.needs_regeneration());

    // Jalankan regenerasi
    assert!(app.regenerate_parametric_model().is_ok());
    assert_volume("regenerated", total_volume(&app), 16000.0);
    assert_eq!(
        app.parametric_dag.get_feature(f_extrude).unwrap().status,
        ducad_core::parametric::FeatureStatus::Valid
    );
    assert!(!app.parametric_dag.needs_regeneration());
}

#[test]
fn regenerate_failure_keeps_old_body_and_reports() {
    let mut app = DuCADApp::new_for_test();
    let sub = ducad_sketch::Subpath {
        start: DVec2::new(0.0, 0.0),
        segs: vec![
            ducad_sketch::PathSeg::Line { end: DVec2::new(40.0, 0.0) },
            ducad_sketch::PathSeg::Line { end: DVec2::new(40.0, 20.0) },
            ducad_sketch::PathSeg::Line { end: DVec2::new(0.0, 20.0) },
        ],
        closed: true,
    };
    let eid = app.sketch_mut().entities.insert(Entity::path(vec![sub]));
    app.sketch_mut().entity_names.insert(eid, "box_profile".to_string());
    app.sketch_mut().touch(eid);

    let f_extrude = app.record_extrude_feature_with_sources(10.0, false, vec!["box_profile".to_string()], false);
    assert!(app.regenerate_parametric_model().is_ok());
    assert_volume("initial", total_volume(&app), 8000.0);
    assert_eq!(app.model.doc.bodies.len(), 1);

    // Rusak profil: buka kurva tertutup menjadi kurva terbuka sehingga tidak bisa diekstrusi
    let broken_sub = ducad_sketch::Subpath {
        start: DVec2::new(0.0, 0.0),
        segs: vec![
            ducad_sketch::PathSeg::Line { end: DVec2::new(40.0, 0.0) },
            ducad_sketch::PathSeg::Line { end: DVec2::new(40.0, 20.0) },
        ],
        closed: false,
    };
    let cmd = ducad_sketch::commands::UpdateEntity::new("Rusak Profil", eid, Entity::path(vec![broken_sub]));
    app.execute_sketch_command(Box::new(cmd));

    assert_eq!(
        app.parametric_dag.get_feature(f_extrude).unwrap().status,
        ducad_core::parametric::FeatureStatus::Stale
    );

    // Jalankan regenerasi — harus gagal dan melaporkan error
    let res = app.regenerate_parametric_model();
    assert!(res.is_err());

    // Fitur harus berstatus Error
    assert!(matches!(
        app.parametric_dag.get_feature(f_extrude).unwrap().status,
        ducad_core::parametric::FeatureStatus::Error(_)
    ));

    // Body lama harus TETAP DIPERTAHANKAN (tidak hilang dan volumenya tetap sama)
    assert_eq!(app.model.doc.bodies.len(), 1);
    assert_volume("preserved old body", total_volume(&app), 8000.0);
}

#[test]
fn extrude_vector_selection_produces_bodies_with_fill_color() {
    let mut app = DuCADApp::new_for_test();
    app.set_app_mode(crate::mode::AppMode::Vector);

    // Dua kurva tertutup dengan warna fill berbeda (merah dan biru)
    let red_sub = ducad_sketch::Subpath {
        start: DVec2::new(0.0, 0.0),
        segs: vec![
            ducad_sketch::PathSeg::Line { end: DVec2::new(10.0, 0.0) },
            ducad_sketch::PathSeg::Line { end: DVec2::new(10.0, 10.0) },
            ducad_sketch::PathSeg::Line { end: DVec2::new(0.0, 10.0) },
        ],
        closed: true,
    };
    let red_path = app.sketch_mut().entities.insert(Entity::path(vec![red_sub]));
    app.sketch_mut().entity_names.insert(red_path, "red_box".to_string());
    app.sketch_mut().styles.insert(
        red_path,
        ducad_sketch::Style {
            fill: Some(ducad_sketch::Paint::Solid(ducad_sketch::Rgba([1.0, 0.0, 0.0, 1.0]))),
            fill_rule: ducad_sketch::FillRule::NonZero,
            stroke: None,
            opacity: 1.0,
            blend: ducad_sketch::BlendMode::Normal,
        },
    );

    let blue_sub = ducad_sketch::Subpath {
        start: DVec2::new(20.0, 0.0),
        segs: vec![
            ducad_sketch::PathSeg::Line { end: DVec2::new(30.0, 0.0) },
            ducad_sketch::PathSeg::Line { end: DVec2::new(30.0, 10.0) },
            ducad_sketch::PathSeg::Line { end: DVec2::new(20.0, 10.0) },
        ],
        closed: true,
    };
    let blue_path = app.sketch_mut().entities.insert(Entity::path(vec![blue_sub]));
    app.sketch_mut().entity_names.insert(blue_path, "blue_box".to_string());
    app.sketch_mut().styles.insert(
        blue_path,
        ducad_sketch::Style {
            fill: Some(ducad_sketch::Paint::Solid(ducad_sketch::Rgba([0.0, 0.0, 1.0, 1.0]))),
            fill_rule: ducad_sketch::FillRule::NonZero,
            stroke: None,
            opacity: 1.0,
            blend: ducad_sketch::BlendMode::Normal,
        },
    );

    app.selected.insert(red_path);
    app.selected.insert(blue_path);

    // Ekstrusi vektor seleksi: per_object = true, material_from_style = true
    let ok = app.extrude_vector_selection(5.0, true, true, false);
    assert!(ok, "{:?}", app.model_status);

    assert_eq!(app.model.doc.bodies.len(), 2);
    assert_eq!(app.app_mode, crate::mode::AppMode::Solid);

    let red_body = app
        .model
        .doc
        .bodies
        .values()
        .find(|b| b.name == "red_box")
        .expect("body red_box harus ada");
    let blue_body = app
        .model
        .doc
        .bodies
        .values()
        .find(|b| b.name == "blue_box")
        .expect("body blue_box harus ada");

    assert_eq!(red_body.material.base_color, [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(blue_body.material.base_color, [0.0, 0.0, 1.0, 1.0]);
}

#[test]
fn extrude_layer_produces_per_object_bodies() {
    let mut app = DuCADApp::new_for_test();
    app.set_app_mode(crate::mode::AppMode::Vector);

    let layer_id = app.sketch_mut().layers.insert(ducad_sketch::Layer::new(
        "Artwork",
        ducad_sketch::Rgba([1.0, 1.0, 1.0, 1.0]),
    ));
    app.sketch_mut().layer_order.push(layer_id);

    let sub = ducad_sketch::Subpath {
        start: DVec2::new(0.0, 0.0),
        segs: vec![
            ducad_sketch::PathSeg::Line { end: DVec2::new(15.0, 0.0) },
            ducad_sketch::PathSeg::Line { end: DVec2::new(15.0, 15.0) },
            ducad_sketch::PathSeg::Line { end: DVec2::new(0.0, 15.0) },
        ],
        closed: true,
    };
    let path_id = app.sketch_mut().entities.insert(Entity::path(vec![sub]));
    app.sketch_mut().entity_names.insert(path_id, "art_shape".to_string());
    app.sketch_mut().entity_layer.insert(path_id, layer_id);
    app.sketch_mut().styles.insert(
        path_id,
        ducad_sketch::Style {
            fill: Some(ducad_sketch::Paint::Solid(ducad_sketch::Rgba([0.0, 1.0, 0.0, 1.0]))),
            fill_rule: ducad_sketch::FillRule::NonZero,
            stroke: None,
            opacity: 1.0,
            blend: ducad_sketch::BlendMode::Normal,
        },
    );

    let ok = app.extrude_layer(layer_id, 4.0, true, false);
    assert!(ok, "{:?}", app.model_status);

    assert_eq!(app.model.doc.bodies.len(), 1);
    assert_eq!(app.app_mode, crate::mode::AppMode::Solid);
    let body = app
        .model
        .doc
        .bodies
        .values()
        .find(|b| b.name == "art_shape")
        .expect("body art_shape harus ada");
    assert_eq!(body.material.base_color, [0.0, 1.0, 0.0, 1.0]);
}


fn rect_path(app: &mut DuCADApp, name: &str, x0: f64) -> ducad_sketch::EntityId {
    let sub = ducad_sketch::Subpath {
        start: DVec2::new(x0, 0.0),
        segs: vec![
            ducad_sketch::PathSeg::Line { end: DVec2::new(x0 + 10.0, 0.0) },
            ducad_sketch::PathSeg::Line { end: DVec2::new(x0 + 10.0, 10.0) },
            ducad_sketch::PathSeg::Line { end: DVec2::new(x0, 10.0) },
        ],
        closed: true,
    };
    let eid = app.sketch_mut().entities.insert(Entity::path(vec![sub]));
    app.sketch_mut().entity_names.insert(eid, name.to_string());
    app.sketch_mut().touch(eid);
    eid
}

/// Regresi REVIEW-2026-09-24 #13: satu fitur gagal tidak menghentikan
/// regenerasi fitur independen sesudahnya.
#[test]
fn regenerate_continues_past_failed_independent_feature() {
    let mut app = DuCADApp::new_for_test();
    rect_path(&mut app, "ada", 0.0);
    let f_bad =
        app.record_extrude_feature_with_sources(5.0, false, vec!["hilang".to_string()], false);
    let f_ok = app.record_extrude_feature_with_sources(5.0, false, vec!["ada".to_string()], false);

    let res = app.regenerate_parametric_model();
    let err = res.expect_err("fitur gagal harus dilaporkan");
    assert!(err.contains("tidak ditemukan"), "{err}");
    assert!(matches!(
        app.parametric_dag.get_feature(f_bad).unwrap().status,
        ducad_core::parametric::FeatureStatus::Error(_)
    ));
    assert_eq!(
        app.parametric_dag.get_feature(f_ok).unwrap().status,
        ducad_core::parametric::FeatureStatus::Valid,
        "fitur independen tetap diregenerasi"
    );
    assert_volume("fitur valid", total_volume(&app), 500.0);
}

/// Regresi REVIEW-2026-09-24 #13: varian tanpa jalur regenerasi (Hole) tidak
/// boleh ditandai `Valid` begitu saja.
#[test]
fn regenerate_does_not_mark_unsupported_feature_valid() {
    let mut app = DuCADApp::new_for_test();
    rect_path(&mut app, "blok", 0.0);
    app.record_extrude_feature_with_sources(5.0, false, vec!["blok".to_string()], false);
    let spec = ducad_core::hole::HoleSpec::for_iso(
        ducad_core::hole::IsoMetricThread::default(),
        ducad_core::hole::HoleKind::Simple,
        3.0,
    );
    let f_hole = app.record_hole_feature(spec, (5.0, 5.0, 5.0), (0.0, 0.0, -1.0));

    assert!(app.regenerate_parametric_model().is_err());
    assert!(matches!(
        app.parametric_dag.get_feature(f_hole).unwrap().status,
        ducad_core::parametric::FeatureStatus::Error(_)
    ));
}
