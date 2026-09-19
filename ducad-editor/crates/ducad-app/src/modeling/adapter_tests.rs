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
