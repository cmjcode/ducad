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
use crate::types::{PickMode, ToolKind};

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

/// Pusat massa satu-satunya body (untuk memastikan sisi mana yang terbuka).
fn only_centroid(app: &DuCADApp) -> [f64; 3] {
    let geo = app.model.geometry.values().next().expect("satu body");
    geo.shape.mass_properties().centroid
}

#[test]
fn adapter_shell_side_face_opens_that_side() {
    let mut app = DuCADApp::new_for_test();
    let id = add_box(&mut app, "Box", [20.0, 20.0, 20.0], [0.0; 3]);
    app.selected_bodies = [id].into_iter().collect();
    // Ray dari kiri menumbuk face x = 0 (sisi -X).
    app.selected_faces = vec![PickRay {
        origin: (-100.0, 10.0, 10.0),
        dir: (1.0, 0.0, 0.0),
    }];
    app.shell_thickness_input = "2".to_string();
    app.shell_selected_body();
    assert!(app.model_status.is_none(), "{:?}", app.model_status);
    assert_volume("shell -X", total_volume(&app), SHELL_VOLUME);
    let c = only_centroid(&app);
    assert!(c[0] > 10.1 && (c[2] - 10.0).abs() < 1e-6, "{c:?}");
}

#[test]
fn adapter_shell_direction_without_face_selection() {
    for (dir, axis, sign) in [
        (ducad_kernel::Direction::NegZ, 2, -1.0),
        (ducad_kernel::Direction::PosY, 1, 1.0),
        (ducad_kernel::Direction::NegX, 0, -1.0),
    ] {
        let mut app = DuCADApp::new_for_test();
        let id = add_box(&mut app, "Box", [20.0, 20.0, 20.0], [0.0; 3]);
        app.selected_bodies = [id].into_iter().collect();
        app.shell_direction = dir;
        app.shell_thickness_input = "2".to_string();
        app.commit_shell();
        assert!(app.model_status.is_none(), "{dir:?}: {:?}", app.model_status);
        assert_volume("shell arah", total_volume(&app), SHELL_VOLUME);
        let c = only_centroid(&app);
        assert!((c[axis] - 10.0) * sign < -0.1, "{dir:?}: {c:?}");
    }
}

#[test]
fn adapter_shell_two_faces_opens_both() {
    let mut app = DuCADApp::new_for_test();
    let id = add_box(&mut app, "Box", [20.0, 20.0, 20.0], [0.0; 3]);
    app.selected_bodies = [id].into_iter().collect();
    app.selected_faces = vec![
        top_face_ray(10.0, 10.0),
        PickRay {
            origin: (10.0, 10.0, -100.0),
            dir: (0.0, 0.0, 1.0),
        },
    ];
    app.shell_thickness_input = "2".to_string();
    app.commit_shell();
    assert!(app.model_status.is_none(), "{:?}", app.model_status);
    // Tabung persegi: 20x20x20 dikurangi lubang tembus 16x16x20.
    assert_volume("shell 2 sisi", total_volume(&app), 8000.0 - 16.0 * 16.0 * 20.0);
}

#[test]
fn adapter_shell_depth_limits_cavity_and_zero_is_full() {
    let mut app = DuCADApp::new_for_test();
    let id = add_box(&mut app, "Box", [20.0, 20.0, 20.0], [0.0; 3]);
    app.selected_bodies = [id].into_iter().collect();
    app.selected_faces = vec![top_face_ray(10.0, 10.0)];
    app.shell_thickness_input = "2".to_string();
    app.shell_depth_input = "10".to_string();
    app.commit_shell();
    assert!(app.model_status.is_none(), "{:?}", app.model_status);
    assert_volume("shell depth 10", total_volume(&app), 8000.0 - 16.0 * 16.0 * 10.0);

    // Kedalaman 0 dan kosong = rongga penuh seperti sebelumnya.
    for depth in ["0", ""] {
        let mut app = DuCADApp::new_for_test();
        let id = add_box(&mut app, "Box", [20.0, 20.0, 20.0], [0.0; 3]);
        app.selected_bodies = [id].into_iter().collect();
        app.selected_faces = vec![top_face_ray(10.0, 10.0)];
        app.shell_thickness_input = "2".to_string();
        app.shell_depth_input = depth.to_string();
        app.commit_shell();
        assert_volume("shell depth 0", total_volume(&app), SHELL_VOLUME);
    }
}

#[test]
fn adapter_shell_depth_too_deep_offers_fixes_and_keeps_body() {
    let mut app = DuCADApp::new_for_test();
    let id = add_box(&mut app, "Box", [20.0, 20.0, 20.0], [0.0; 3]);
    app.selected_bodies = [id].into_iter().collect();
    app.selected_faces = vec![top_face_ray(10.0, 10.0)];
    app.shell_thickness_input = "2".to_string();
    app.shell_depth_input = "19".to_string();
    app.commit_shell();
    assert!(app.model_status.is_some());
    assert_volume("tidak berubah", total_volume(&app), 8000.0);
    assert!(app.error_card.open);
    assert_eq!(app.error_fixes.len(), 2, "{:?}", app.error_fixes);
    // Fix pertama (setengah kedalaman maksimum = 9 mm) langsung berhasil.
    app.apply_gui_fix(0);
    assert_volume("fix depth", total_volume(&app), 8000.0 - 16.0 * 16.0 * 9.0);
}

#[test]
fn adapter_shell_records_opening_for_regeneration() {
    let mut app = DuCADApp::new_for_test();
    let id = add_box(&mut app, "Box", [20.0, 20.0, 20.0], [0.0; 3]);
    app.selected_bodies = [id].into_iter().collect();
    app.shell_direction = ducad_kernel::Direction::NegY;
    app.shell_thickness_input = "2".to_string();
    app.shell_depth_input = "5".to_string();
    app.commit_shell();
    let node = app.parametric_dag.nodes.last().expect("fitur shell tercatat");
    match &node.payload {
        ducad_core::parametric::FeaturePayload::Shell {
            thickness,
            open_rays,
            open_direction,
            depth,
            ..
        } => {
            assert_eq!(*thickness, 2.0);
            assert!(open_rays.is_empty());
            assert_eq!(open_direction.as_deref(), Some("-Y"));
            assert_eq!(*depth, 5.0);
        }
        other => panic!("payload bukan Shell: {other:?}"),
    }
}

#[test]
fn shell_payload_without_opening_fields_still_loads() {
    use ducad_core::parametric::FeaturePayload;
    // Berkas lama hanya menyimpan target + tebal: buang field baru dari
    // bentuk JSON saat ini lalu muat kembali.
    let mut json = serde_json::to_value(FeaturePayload::Shell {
        target_feature_id: 3,
        thickness: 2.0,
        open_rays: vec![([0.0; 3], [0.0, 0.0, -1.0])],
        open_direction: Some("-X".to_string()),
        depth: 4.0,
    })
    .unwrap();
    fn strip(v: &mut serde_json::Value) {
        if let Some(map) = v.as_object_mut() {
            for k in ["open_rays", "open_direction", "depth"] {
                map.remove(k);
            }
            map.values_mut().for_each(strip);
        }
    }
    strip(&mut json);
    let FeaturePayload::Shell {
        thickness,
        open_rays,
        open_direction,
        depth,
        ..
    } = serde_json::from_value(json).unwrap()
    else {
        panic!("payload bukan Shell");
    };
    assert_eq!(thickness, 2.0);
    assert!(open_rays.is_empty() && depth == 0.0);
    // Tanpa arah tersimpan = sisi atas, perilaku lama.
    assert_eq!(
        super::parametric_engine::shell_direction_from_label(open_direction.as_deref()),
        ducad_kernel::Direction::PosZ
    );
    for dir in [
        ducad_kernel::Direction::PosX,
        ducad_kernel::Direction::NegX,
        ducad_kernel::Direction::PosY,
        ducad_kernel::Direction::NegY,
        ducad_kernel::Direction::PosZ,
        ducad_kernel::Direction::NegZ,
    ] {
        let label = super::parametric_engine::shell_direction_label(dir);
        assert_eq!(super::parametric_engine::shell_direction_from_label(Some(label)), dir);
    }
}

#[test]
fn shell_tool_picks_face_without_preselected_body_and_toggles() {
    let mut app = DuCADApp::new_for_test();
    let id = add_box(&mut app, "Box", [20.0, 20.0, 20.0], [-10.0, -10.0, -10.0]);
    app.set_tool(ToolKind::Shell);
    assert_eq!(app.picking_mode, PickMode::Face, "Shell selalu masuk mode pilih-face");
    assert!(app.selected_bodies.is_empty());

    use eframe::egui;
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    // Cari piksel yang mengenai box (kamera bawaan mengarah ke sekitar origin).
    let hit_pos = (0..=20)
        .flat_map(|i| (0..=20).map(move |j| egui::pos2(40.0 * i as f32, 30.0 * j as f32)))
        .find(|p| app.pick_body_face_at_cursor(rect, *p).is_some())
        .expect("box terlihat di viewport");

    app.pick_face_for_tool(rect, hit_pos);
    assert!(app.selected_bodies.contains(&id), "body terpilih otomatis");
    assert_eq!(app.selected_faces.len(), 1);
    assert!(app.active_face.is_some());

    // Klik ulang face yang sama membatalkan pilihan.
    app.pick_face_for_tool(rect, hit_pos);
    assert!(app.selected_faces.is_empty());
    assert!(app.active_face.is_none());

    // Keluar dari Shell mengembalikan mode klik normal.
    app.set_tool(ToolKind::Select);
    assert_eq!(app.picking_mode, PickMode::None);
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

/// "Sketsa di Face" harus menaruh bidang sketsa DI permukaan face (bukan di
/// bidang dasar), mendaftarkannya sebagai datum plane sendiri, memproyeksikan
/// tepi face sebagai konstruksi, dan tidak menumpuk datum bila face yang sama
/// dipilih lagi. Regresi: bidang dari face dulu berjenis `Top`, sehingga
/// lingkaran yang digambar tersimpan dan tergambar di Z=0.
#[test]
fn sketch_on_face_puts_plane_on_the_face_and_reuses_datum() {
    use ducad_render::PlaneKind;

    let mut app = DuCADApp::new_for_test();
    let id = add_box(&mut app, "Block", [30.0, 10.0, 8.0], [0.0; 3]);
    let ray = top_face_ray(15.0, 5.0);
    let hit =
        ducad_kernel::pick_face_details(&app.model.geometry[id].shape, ray).expect("face atas");
    app.active_face = Some((id, ray, hit.clone()));

    app.sketch_on_active_face();

    assert!(app.is_sketching);
    assert!(app.active_face.is_none());
    let plane = app.active_plane;
    assert!(matches!(plane.kind, PlaneKind::Custom(_)), "{:?}", plane.kind);
    assert!((plane.origin.z - 8.0).abs() < 1e-4, "origin {:?}", plane.origin);
    assert!(plane.normal.z > 0.999, "normal {:?}", plane.normal);
    // Sumbu U mengikuti tepi terpanjang (30 mm searah X).
    assert!(plane.u_axis.x.abs() > 0.999, "u {:?}", plane.u_axis);
    assert_eq!(app.datum_planes.len(), 1);
    let idx = app.active_plane_index();
    assert_eq!(idx, 3, "slot datum pertama");
    assert_eq!(app.plane_for_index(idx), plane, "overlay memakai bidang yang sama");

    // Tepi face terproyeksi sebagai 4 garis konstruksi di bidang face.
    let projected: Vec<&Entity> = app.sketch_at_index(idx).entities.values().collect();
    assert_eq!(projected.len(), 4, "{projected:?}");
    assert!(projected.iter().all(|e| e.is_construction()));
    let max_abs = projected
        .iter()
        .flat_map(|e| match e {
            Entity::Line { start, end, .. } => vec![*start, *end],
            _ => vec![],
        })
        .map(|p| p.abs().max_element())
        .fold(0.0, f64::max);
    assert!((max_abs - 15.0).abs() < 1e-4, "{max_abs}");

    // Lingkaran yang digambar masuk ke sketsa bidang face, bukan bidang Top.
    app.execute_sketch_command(Box::new(ducad_sketch::commands::InsertEntities::new(
        "Circle",
        vec![Entity::circle(DVec2::ZERO, 3.0)],
    )));
    assert_eq!(app.sketch_at_index(idx).entities.len(), 5);
    assert_eq!(app.sketch_at_index(0).entities.len(), 0, "bidang Top tetap kosong");
    let world = app.active_plane.to_world(DVec2::ZERO, 0.0);
    assert!((world - glam::Vec3::new(15.0, 5.0, 8.0)).length() < 1e-3, "{world:?}");

    // Face yang sama dipilih lagi → datum lama dipakai ulang, tanpa proyeksi ganda.
    app.exit_sketching();
    app.active_face = Some((id, ray, hit));
    app.sketch_on_active_face();
    assert_eq!(app.datum_planes.len(), 1);
    assert_eq!(app.active_plane_index(), idx);
    assert_eq!(app.sketch_at_index(idx).entities.len(), 5);
}

/// Sisi lengkung ditolak dengan pesan, face tetap aktif untuk operasi lain.
#[test]
fn sketch_on_face_rejects_non_planar_face() {
    let mut app = DuCADApp::new_for_test();
    let geo = ducad_engine::compute::primitive(
        &ducad_engine::compute::PrimitiveShape::Cylinder { r: 10.0, h: 20.0 },
        [0.0; 3],
    )
    .unwrap();
    let id = app.model.doc.add_body("Cyl");
    app.model.geometry.insert(id, geo);
    let ray = PickRay {
        origin: (100.0, 0.0, 10.0),
        dir: (-1.0, 0.0, 0.0),
    };
    let hit =
        ducad_kernel::pick_face_details(&app.model.geometry[id].shape, ray).expect("sisi silinder");
    assert_eq!(hit.surface_kind, ducad_kernel::SurfaceKind::Cylinder);
    app.active_face = Some((id, ray, hit));
    app.exit_sketching();
    let plane_before = app.active_plane;

    app.sketch_on_active_face();

    assert!(!app.is_sketching, "mode sketsa tidak boleh aktif");
    assert_eq!(app.active_plane, plane_before, "bidang aktif tidak berubah");
    assert!(app.active_face.is_some(), "face tetap aktif untuk operasi lain");
    assert!(app.datum_planes.is_empty());
    assert!(app.model_status.as_deref().unwrap_or("").contains("datar"));
}

/// Lingkaran yang digambar di sketsa face atas lalu di-extrude KE BAWAH lewat
/// gizmo harus terdeteksi memotong body dan menghasilkan lubang, bukan solid
/// baru yang tersembunyi di dalam body.
#[test]
fn sketch_on_face_extrude_down_cuts_the_body() {
    let mut app = DuCADApp::new_for_test();
    let id = add_box(&mut app, "Block", [30.0, 10.0, 8.0], [0.0; 3]);
    let ray = top_face_ray(15.0, 5.0);
    let hit =
        ducad_kernel::pick_face_details(&app.model.geometry[id].shape, ray).expect("face atas");
    app.active_face = Some((id, ray, hit));
    app.sketch_on_active_face();
    let before = total_volume(&app);

    app.execute_sketch_command(Box::new(ducad_sketch::commands::InsertEntities::new(
        "Circle",
        vec![Entity::circle(DVec2::ZERO, 2.0)],
    )));
    let circle_id = app
        .sketch()
        .entities
        .iter()
        .find(|(_, e)| matches!(e, Entity::Circle { .. }))
        .map(|(id, _)| id)
        .expect("lingkaran ada");
    app.selected = HashSet::from([circle_id]);
    app.set_tool(ToolKind::Select);

    app.gizmo_distance = -5.0;
    app.extruding_from_gizmo = true;
    app.update_gizmo_boolean_detection();
    assert!(app.gizmo_is_cutting, "extrude ke bawah harus terdeteksi memotong");
    assert_eq!(app.gizmo_target_body, Some(id));

    app.commit_gizmo_extrusion();
    assert_eq!(app.model.doc.bodies.len(), 1, "tidak ada body baru");
    let want = before - std::f64::consts::PI * 2.0 * 2.0 * 5.0;
    assert_volume("cut", total_volume(&app), want);
}

/// Preview extrude potong digambar sebagai ghost merah TEMBUS PANDANG yang
/// bidangnya berimpit dengan permukaan body target (sketsa di face). Supaya
/// tidak z-fighting (bercak merah/abu-abu acak), semua vertex preview harus
/// membawa tarikan depth (`material_params.w > 0`), dan body target tidak
/// boleh ikut dimerahkan pekat — hanya volume yang dibuang yang merah.
#[test]
fn cut_extrude_preview_has_depth_pull_and_translucent_red() {
    let mut app = DuCADApp::new_for_test();
    let id = add_box(&mut app, "Block", [30.0, 10.0, 8.0], [0.0; 3]);
    let ray = top_face_ray(15.0, 5.0);
    let hit =
        ducad_kernel::pick_face_details(&app.model.geometry[id].shape, ray).expect("face atas");
    app.active_face = Some((id, ray, hit));
    app.sketch_on_active_face();
    app.execute_sketch_command(Box::new(ducad_sketch::commands::InsertEntities::new(
        "Circle",
        vec![Entity::circle(DVec2::ZERO, 2.0)],
    )));
    let circle_id = app
        .sketch()
        .entities
        .iter()
        .find(|(_, e)| matches!(e, Entity::Circle { .. }))
        .map(|(id, _)| id)
        .expect("lingkaran ada");
    app.selected = HashSet::from([circle_id]);
    app.set_tool(ToolKind::Select);
    app.active_face = None;
    app.gizmo_distance = -5.0;
    app.extruding_from_gizmo = true;
    app.update_gizmo_boolean_detection();
    assert!(app.gizmo_is_cutting);

    let cp = app.gizmo_cut_preview.as_ref().expect("pratinjau cut dihitung");
    assert_eq!(cp.target, id);
    assert!(!cp.removed.positions.is_empty(), "volume irisan (removed) harus ada");
    assert!(!cp.remaining.positions.is_empty(), "body sisa (remaining) harus ada");
    assert!(
        cp.remaining.positions.len() > app.model.geometry[id].mesh.positions.len(),
        "body sisa berlubang: mesh-nya lebih rapat dari balok polos"
    );

    let (_positions, _normals, colors, materials, _indices) = app.build_combined_body_mesh();
    assert_eq!(colors.len(), materials.len());
    // Vertex preview = yang membawa tarikan depth; body biasa tidak.
    let preview: Vec<usize> =
        (0..materials.len()).filter(|&i| materials[i][3] > 0.0).collect();
    assert!(!preview.is_empty(), "preview harus ada saat memotong");
    let mut saw_red = false;
    for &i in &preview {
        assert!(colors[i][3] < 0.75, "preview cut harus tembus pandang: {:?}", colors[i]);
        if colors[i][0] > 0.9 && colors[i][1] < 0.3 && colors[i][2] < 0.3 {
            saw_red = true;
        }
    }
    assert!(saw_red, "volume irisan harus digambar merah");
    // Body target: digambar sebagai hasil potong, tetap opak, tidak merah pekat.
    let body: Vec<usize> =
        (0..materials.len()).filter(|&i| materials[i][3] == 0.0).collect();
    assert_eq!(body.len(), cp.remaining.positions.len(), "body target memakai mesh sisa potong");
    for &i in &body {
        assert!(colors[i][3] >= 0.99, "body target tetap opak");
        assert!(!(colors[i][0] > 0.9 && colors[i][1] < 0.3), "body target tidak merah pekat");
    }
}

/// Tombol Extrude (popup/bilah bawah/palet) bernilai negatif dari sketsa di
/// face atas harus memotong body, sama seperti gizmo — bukan membuat solid
/// baru yang tersembunyi di dalam body.
#[test]
fn sketch_on_face_extrude_button_negative_cuts_the_body() {
    let mut app = DuCADApp::new_for_test();
    let id = add_box(&mut app, "Block", [30.0, 10.0, 8.0], [0.0; 3]);
    let ray = top_face_ray(15.0, 5.0);
    let hit =
        ducad_kernel::pick_face_details(&app.model.geometry[id].shape, ray).expect("face atas");
    app.active_face = Some((id, ray, hit));
    app.sketch_on_active_face();
    let before = total_volume(&app);

    app.execute_sketch_command(Box::new(ducad_sketch::commands::InsertEntities::new(
        "Circle",
        vec![Entity::circle(DVec2::ZERO, 2.0)],
    )));
    let circle_id = app
        .sketch()
        .entities
        .iter()
        .find(|(_, e)| matches!(e, Entity::Circle { .. }))
        .map(|(id, _)| id)
        .expect("lingkaran ada");
    app.selected = HashSet::from([circle_id]);

    app.extrude_distance_input = "-5".to_string();
    app.extrude_selected();
    assert_eq!(app.model.doc.bodies.len(), 1, "{:?}", app.model_status);
    let want = before - std::f64::consts::PI * 2.0 * 2.0 * 5.0;
    assert_volume("cut-button", total_volume(&app), want);

    // Arah positif (menjauhi body) tetap membuat solid baru seperti sebelumnya.
    app.selected = HashSet::from([circle_id]);
    app.extrude_distance_input = "3".to_string();
    app.extrude_selected();
    assert_eq!(app.model.doc.bodies.len(), 2, "{:?}", app.model_status);
}

/// Gizmo tarik-sisi menampilkan dan menerima UKURAN HASIL (dari dasar body /
/// sumbu silinder), bukan hanya selisih — angka yang diketik adalah tinggi
/// atau radius akhir.
#[test]
fn face_gizmo_uses_absolute_dimension_from_base() {
    let mut app = DuCADApp::new_for_test();
    let id = add_box(&mut app, "Blok", [20.0, 30.0, 50.0], [0.0; 3]);
    let ray = top_face_ray(10.0, 15.0);
    let hit =
        ducad_kernel::pick_face_details(&app.model.geometry[id].shape, ray).expect("face atas");
    app.active_face = Some((id, ray, hit));

    // Tutup atas balok 50 mm → dasar = tinggi body.
    let base = app.face_gizmo_base_dimension().expect("dasar face datar");
    assert!((base - 50.0).abs() < 1e-6, "dasar {base}");

    // Drag +11.2 → label memuat ukuran hasil 61.2 dan selisihnya.
    app.face_gizmo_distance = 11.2;
    let text = app.face_gizmo_dimension_text();
    assert!(text.starts_with("61.2 mm"), "label: {text}");
    assert!(text.contains("(+11.20)"), "label: {text}");
    assert_eq!(app.face_gizmo_input_text(), "61.2");

    // Ketik 60 = tinggi akhir 60 → selisih +10; ketik 40 → potong 10.
    app.apply_face_gizmo_typed_value(60.0).unwrap();
    assert!((app.face_gizmo_distance - 10.0).abs() < 1e-9);
    app.apply_face_gizmo_typed_value(40.0).unwrap();
    assert!((app.face_gizmo_distance + 10.0).abs() < 1e-9);
    assert!(app.apply_face_gizmo_typed_value(0.0).is_err());

    // Commit tinggi 60 → volume 20×30×60.
    app.apply_face_gizmo_typed_value(60.0).unwrap();
    app.commit_face_gizmo_extrusion();
    assert_volume("tinggi 60", total_volume(&app), 20.0 * 30.0 * 60.0);
}

#[test]
fn face_gizmo_cylinder_shows_result_radius() {
    let mut app = DuCADApp::new_for_test();
    let geo = ducad_engine::compute::primitive(
        &ducad_engine::compute::PrimitiveShape::Cylinder { r: 10.0, h: 30.0 },
        [0.0; 3],
    )
    .unwrap();
    let id = app.model.doc.add_body("Silinder");
    app.model.geometry.insert(id, geo);
    // Sinar mendatar dari +X menuju sumbu → kena selimut di x = 10.
    let ray = PickRay { origin: (100.0, 0.0, 15.0), dir: (-1.0, 0.0, 0.0) };
    let hit =
        ducad_kernel::pick_face_details(&app.model.geometry[id].shape, ray).expect("selimut");
    assert_eq!(hit.surface_kind, ducad_kernel::SurfaceKind::Cylinder);
    assert!((hit.surface_radius.expect("radius") - 10.0).abs() < 1e-6);
    // Kerangka radial untuk pratinjau: sumbu Z lewat titik asal, sehingga
    // vertex selimut digeser menjauhi sumbu (membesarkan R), bukan ditranslasi.
    let ax = hit.radial_axis.expect("sumbu silinder");
    assert!(ax.origin.0.abs() < 1e-6 && ax.origin.1.abs() < 1e-6, "sumbu lewat (0,0): {ax:?}");
    assert!((ax.dir.2.abs() - 1.0).abs() < 1e-6, "arah sumbu Z: {ax:?}");
    app.active_face = Some((id, ray, hit));

    app.face_gizmo_distance = 2.0;
    let text = app.face_gizmo_dimension_text();
    assert!(text.starts_with("R 12 mm"), "label: {text}");
    assert!(text.contains("(+2.00)"), "label: {text}");

    // Ketik radius akhir 8 → selisih −2 (mengecil).
    app.apply_face_gizmo_typed_value(8.0).unwrap();
    assert!((app.face_gizmo_distance + 2.0).abs() < 1e-9);
}

/// Body silinder (piringan), sketsa di face atas, lingkaran di tengah lebih
/// kecil ATAU sama persis dengan jari-jari piringan (dinding berimpit):
/// keduanya harus terdeteksi memotong saat extrude ke bawah.
#[test]
fn sketch_on_disc_top_detects_cut_even_with_coincident_walls() {
    for (label, radius) in [("kecil", 10.0), ("berimpit", 40.0)] {
        let mut app = DuCADApp::new_for_test();
        let geo = ducad_engine::compute::primitive(
            &ducad_engine::compute::PrimitiveShape::Cylinder { r: 40.0, h: 20.0 },
            [0.0; 3],
        )
        .unwrap();
        let id = app.model.doc.add_body("Disc");
        app.model.geometry.insert(id, geo);
        let ray = top_face_ray(5.0, 5.0);
        let hit = ducad_kernel::pick_face_details(&app.model.geometry[id].shape, ray)
            .expect("face atas piringan");
        app.active_face = Some((id, ray, hit));
        app.sketch_on_active_face();
        assert!((app.active_plane.origin.z - 20.0).abs() < 1e-3, "{label}: {:?}", app.active_plane.origin);

        app.execute_sketch_command(Box::new(ducad_sketch::commands::InsertEntities::new(
            "Circle",
            vec![Entity::circle(DVec2::ZERO, radius)],
        )));
        let circle_id = app
            .sketch()
            .entities
            .iter()
            .find(|(_, e)| matches!(e, Entity::Circle { .. }) && !e.is_construction())
            .map(|(id, _)| id)
            .expect("lingkaran ada");
        app.selected = HashSet::from([circle_id]);
        app.set_tool(ToolKind::Select);

        let target = app.detect_cut_target(-39.32);
        assert_eq!(target, Some(id), "{label}: extrude ke bawah harus memotong piringan");
    }
}

/// Push/pull sisi ke arah DALAM: pratinjau harus menggambar body yang sudah
/// terpotong plus volume merah tembus pandang yang dibuang — bukan prisma
/// yang tersembunyi di dalam body pekat. Commit menghasilkan volume sesuai.
#[test]
fn face_pull_inward_preview_shows_removed_volume_and_hollowed_body() {
    let mut app = DuCADApp::new_for_test();
    let id = add_box(&mut app, "Block", [30.0, 10.0, 8.0], [0.0; 3]);
    let ray = top_face_ray(15.0, 5.0);
    let hit =
        ducad_kernel::pick_face_details(&app.model.geometry[id].shape, ray).expect("face atas");
    app.active_face = Some((id, ray, hit));
    app.set_tool(ToolKind::Select);

    // Sama seperti jalur drag handle di GUI: begin lalu tandai sedang digeser.
    app.begin_face_gizmo_drag();
    app.extruding_face_from_gizmo = true;
    app.face_gizmo_distance = -3.0;
    app.refresh_face_cut_preview();
    let cp = app.face_cut_preview.as_ref().expect("pratinjau potong sisi dihitung");
    assert_eq!(cp.target, id);
    assert!(!cp.removed.positions.is_empty(), "volume yang dibuang harus ada");
    assert!(!cp.remaining.positions.is_empty(), "body sisa harus ada");
    let remaining_top = cp
        .remaining
        .positions
        .iter()
        .map(|p| p[2])
        .fold(f32::NEG_INFINITY, f32::max);
    assert!((remaining_top - 5.0).abs() < 1e-3, "body sisa setinggi 5, dapat {remaining_top}");

    let (positions, _normals, colors, materials, _indices) = app.build_combined_body_mesh();
    let red: Vec<usize> = (0..colors.len())
        .filter(|&i| colors[i][0] > 0.9 && colors[i][1] < 0.3 && colors[i][3] < 0.9)
        .collect();
    assert!(!red.is_empty(), "volume yang dibuang digambar merah tembus pandang");
    assert!(red.iter().all(|&i| materials[i][3] > 0.0), "preview membawa tarikan depth");
    // Mesh body (bukan preview) tidak boleh lagi memuat permukaan di Z=8.
    let body_top = (0..positions.len())
        .filter(|&i| materials[i][3] == 0.0)
        .map(|i| positions[i][2])
        .fold(f32::NEG_INFINITY, f32::max);
    assert!((body_top - 5.0).abs() < 1e-3, "body digambar sudah terpotong, puncak {body_top}");

    // Pratinjau dipakai ulang bila jarak sama, dibuang saat arah keluar.
    app.refresh_face_cut_preview();
    assert!(app.face_cut_preview.is_some());
    app.face_gizmo_distance = 4.0;
    app.refresh_face_cut_preview();
    assert!(app.face_cut_preview.is_none(), "arah keluar memakai pratinjau tutup+dinding");

    app.face_gizmo_distance = -3.0;
    app.commit_face_gizmo_extrusion();
    assert!(app.face_cut_preview.is_none());
    assert_volume("potong sisi", total_volume(&app), 30.0 * 10.0 * 5.0);
}


/// Memulai gizmo dari MODE SKETSA tidak boleh membatalkan dirinya sendiri:
/// `auto_enter_3d_mode_on_extrude_drag` memanggil `set_tool(Select)` yang
/// mereset gizmo, jadi urutannya harus pindah mode dulu baru setel bendera.
#[test]
fn starting_gizmos_from_sketch_mode_keeps_them_active() {
    let mut app = DuCADApp::new_for_test();
    let id = add_box(&mut app, "Block", [30.0, 10.0, 8.0], [0.0; 3]);
    let ray = top_face_ray(15.0, 5.0);
    let hit =
        ducad_kernel::pick_face_details(&app.model.geometry[id].shape, ray).expect("face atas");

    app.is_sketching = true;
    app.active_face = Some((id, ray, hit.clone()));
    app.begin_face_gizmo_drag();
    assert!(app.extruding_face_from_gizmo, "drag tarik-sisi tetap aktif");
    assert!(!app.is_sketching);
    app.cancel_face_gizmo_extrusion();

    app.is_sketching = true;
    app.active_face = Some((id, ray, hit));
    app.open_face_gizmo_precise_input();
    assert!(app.extruding_face_from_gizmo && app.face_gizmo_staged);
    assert!(app.face_gizmo_dimension_editing, "popup nilai presisi terbuka");
    app.cancel_face_gizmo_extrusion();

    app.is_sketching = true;
    app.selected = add_rect(&mut app, 0.0, 0.0, 10.0, 10.0);
    app.begin_gizmo_drag();
    assert!(app.extruding_from_gizmo, "drag extrude profil tetap aktif");
    assert!(!app.is_sketching);
}
