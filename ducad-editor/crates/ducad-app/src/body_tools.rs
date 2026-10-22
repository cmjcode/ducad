//! Alat body 3D (Paritas Fase 0.1–0.3): Primitif, Cermin Body, Putar Body.
//!
//! Geometri dihitung lewat `ducad_engine::compute` (sumber yang sama dengan
//! `Op::Primitive`/`Op::Mirror`/`Op::Transform`), lalu masuk undo stack model
//! sebagai `AddSolidCommand`/`ReplaceGeometryCommand`.

use ducad_core::BodyId;
use ducad_engine::compute;
use ducad_ui::{
    BodyToolEvent, BodyToolsPopup, MirrorBodyPopupState, MirrorPlaneChoice, PrimitiveKind,
    PrimitivePopupState, RotateBodyPopupState,
};
use glam::DVec3;

use crate::app::DuCADApp;
use crate::model::{AddSolidCommand, BodyGeometry, ReplaceGeometryCommand};

/// Panel alat body yang sedang terbuka.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyToolPanel {
    Primitive,
    MirrorBody,
    RotateBody,
}

#[derive(Default)]
pub struct BodyToolsState {
    pub open: Option<BodyToolPanel>,
    pub primitive: PrimitivePopupState,
    pub mirror: MirrorBodyPopupState,
    pub rotate: RotateBodyPopupState,
    custom_height: Option<f32>,
}

/// Konversi pilihan UI ke primitif engine.
pub fn primitive_shape(kind: PrimitiveKind, dims: [f64; 3], centered: bool) -> compute::PrimitiveShape {
    match kind {
        PrimitiveKind::Box => compute::PrimitiveShape::Box { size: dims, centered },
        PrimitiveKind::Cylinder => compute::PrimitiveShape::Cylinder { r: dims[0], h: dims[1] },
        PrimitiveKind::Sphere => compute::PrimitiveShape::Sphere { r: dims[0] },
        PrimitiveKind::Cone => compute::PrimitiveShape::Cone { r1: dims[0], r2: dims[1], h: dims[2] },
        PrimitiveKind::Torus => compute::PrimitiveShape::Torus { major: dims[0], minor: dims[1] },
    }
}

impl DuCADApp {
    /// Jalankan perintah palette Fase 0 paritas.
    pub fn run_parity_cmd(&mut self, cmd: crate::types::ParityCmd, ctx: &egui::Context) {
        use crate::types::ParityCmd;
        match cmd {
            ParityCmd::Primitive => self.open_body_tool(BodyToolPanel::Primitive),
            ParityCmd::MirrorBody => self.open_body_tool(BodyToolPanel::MirrorBody),
            ParityCmd::RotateBody => self.open_body_tool(BodyToolPanel::RotateBody),
            ParityCmd::AutoConstrain => self.auto_constrain(),
            ParityCmd::EditText => self.open_text_edit_from_selection(),
            ParityCmd::ToggleDiameter => self.sketch_dims.show_diameter = !self.sketch_dims.show_diameter,
            ParityCmd::LooseEnds => self.toggle_loose_ends(),
            ParityCmd::Parameters => self.open_params_panel(),
            ParityCmd::Inspect => self.design_tools.inspect_open = true,
            ParityCmd::CopyOplog => self.copy_oplog_json(ctx),
            ParityCmd::ExportPng => self.view_tools.png_open = true,
            ParityCmd::Compare => self.view_tools.compare_open = true,
            ParityCmd::RigidGroup => self.make_rigid_group_from_selection(),
            ParityCmd::DissolveRigidGroup => self.dissolve_rigid_group_of_selection(),
            ParityCmd::SketchPoint => self.set_tool(crate::types::ToolKind::Point),
            ParityCmd::ToggleDriven => self.toggle_driven_for_selection(),
            ParityCmd::ResetDimText => self.reset_dim_text_positions(),
            ParityCmd::DatumAxis => self.open_datum_tool(ducad_ui::DatumKind::Axis),
            ParityCmd::DatumPoint => self.open_datum_tool(ducad_ui::DatumKind::Point),
            ParityCmd::DatumDrawer => self.planes_drawer_open = true,
            ParityCmd::Direct(t) => self.open_direct_tool(t),
            ParityCmd::ExportThreeMf => self.export_3mf(),
            ParityCmd::OpenInSlicer => self.open_in_slicer(0),
            ParityCmd::ImportMesh => self.import_mesh_file(),
            ParityCmd::ExportIges => self.export_iges(),
            ParityCmd::ImportIges => self.import_iges(),
            ParityCmd::ExportViewDxf => self.export_view_dxf(),
            ParityCmd::RecoverableDesigns => self.refresh_recovery(true),
            ParityCmd::NamedViews => self.named_views.open = true,
            ParityCmd::CrashReports => self.crash_reports.open = true,
            ParityCmd::Joint(k) => {
                if self.staged_mate_targets.len() >= 2 {
                    let as_built = self.assembly_tools.as_built;
                    self.create_joint(k, as_built);
                } else {
                    self.assembly_tools.joint_kind = k;
                    self.assembly_tools.open = true;
                    self.model_status = Some(ducad_i18n::t!("asm-joint-need-two"));
                }
            }
            ParityCmd::AssemblyTools => self.assembly_tools.open = true,
            ParityCmd::MotionTools => self.motion_tools.open = true,
            ParityCmd::CapturePositions => self.capture_positions(),
            ParityCmd::RevertPositions => self.revert_positions(),
            ParityCmd::CopyOccurrence => {
                let o = self.assembly_tools.occurrence_offset;
                self.copy_occurrence(o);
            }
            ParityCmd::ContactSet => self.add_contact_set_from_selection(),
            ParityCmd::InspectTools => self.inspect_tools.open = true,
            ParityCmd::SurfaceTools => self.surface_tools.open = true,
            ParityCmd::PlasticTools => self.plastic_tools.open = true,
            ParityCmd::CurvatureMap => {
                self.inspect_tools.open = true;
                self.inspect_tools.curvature.enabled = !self.inspect_tools.curvature.enabled;
            }
            ParityCmd::SketchBreak
            | ParityCmd::ProjectEdges
            | ParityCmd::IntersectBody
            | ParityCmd::BreakLink
            | ParityCmd::CopySketch
            | ParityCmd::CutSketch
            | ParityCmd::PasteSketch
            | ParityCmd::ToggleGlyphs
            | ParityCmd::InsertSvg => self.run_parity_cmd_1b(cmd, ctx),
            ParityCmd::CurvatureG2 => {
                self.apply_sketch_strip_action(ducad_ui::ConstraintAction::ApplyCurvature)
            }
        }
    }

    /// Jadikan body terpilih (≥ 2) satu grup kaku perakitan (Paritas 0.13).
    pub fn make_rigid_group_from_selection(&mut self) {
        self.sync_assembly_instances();
        let ids: Vec<_> = self.selected_bodies.iter().filter_map(|b| self.instance_of_body(*b)).collect();
        if self.assembly_tree.make_rigid_group(&ids) {
            self.model.doc.dirty = true;
            self.solve_and_apply_assembly();
            self.model_status = Some(ducad_i18n::t!("rigid-group-made", count = ids.len()));
        } else {
            self.model_status = Some(ducad_i18n::t!("rigid-group-need-two"));
        }
    }

    /// Lepas grup kaku dari body terpilih.
    pub fn dissolve_rigid_group_of_selection(&mut self) {
        let ids: Vec<_> = self.selected_bodies.iter().filter_map(|b| self.instance_of_body(*b)).collect();
        let before = self.assembly_tree.rigid_groups.len();
        for id in ids {
            self.assembly_tree.dissolve_rigid_group(id);
        }
        let removed = before - self.assembly_tree.rigid_groups.len();
        if removed > 0 {
            self.model.doc.dirty = true;
        }
        self.model_status = Some(ducad_i18n::t!("rigid-group-dissolved", count = removed));
    }

    pub fn open_body_tool(&mut self, panel: BodyToolPanel) {
        self.body_tools.open = Some(panel);
    }

    /// Pusat bbox mesh body (mm).
    pub fn body_bbox_center(&self, id: BodyId) -> Option<DVec3> {
        let geo = self.model.geometry.get(id)?;
        let mut min = [f64::INFINITY; 3];
        let mut max = [f64::NEG_INFINITY; 3];
        for p in &geo.mesh.positions {
            for k in 0..3 {
                min[k] = min[k].min(p[k] as f64);
                max[k] = max[k].max(p[k] as f64);
            }
        }
        min[0].is_finite().then(|| {
            DVec3::new((min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5, (min[2] + max[2]) * 0.5)
        })
    }

    fn single_selected_body(&mut self) -> Option<BodyId> {
        let id = self
            .selected_bodies
            .iter()
            .copied()
            .find(|id| self.model.geometry.contains_key(*id));
        if id.is_none() {
            self.model_status = Some(ducad_i18n::t!("body-tool-need-body"));
        }
        id
    }

    pub fn create_primitive(&mut self, kind: PrimitiveKind, dims: [f64; 3], at: [f64; 3], centered: bool) {
        // Op `primitive`: ukurannya bisa diedit di Feature Tree.
        let (prefix, shape) = match kind {
            PrimitiveKind::Box => ("box", serde_json::json!({ "box": { "size": dims, "centered": centered } })),
            PrimitiveKind::Cylinder => ("cylinder", serde_json::json!({ "cylinder": { "r": dims[0], "h": dims[1] } })),
            PrimitiveKind::Sphere => ("sphere", serde_json::json!({ "sphere": { "r": dims[0] } })),
            PrimitiveKind::Cone => ("cone", serde_json::json!({ "cone": { "r1": dims[0], "r2": dims[1], "h": dims[2] } })),
            PrimitiveKind::Torus => ("torus", serde_json::json!({ "torus": { "major": dims[0], "minor": dims[1] } })),
        };
        let name = kind.label();
        match self.primitive_via_oplog(prefix, shape, at, "Primitif") {
            Ok(()) => self.model_status = Some(ducad_i18n::t!("prim-created", name = name.as_str())),
            Err(e) => self.report_gui_op_error("Primitif", &e),
        }
    }

    fn mirror_plane(&self, body: BodyId, plane: MirrorPlaneChoice, through_center: bool) -> Option<(DVec3, DVec3)> {
        let (point, normal) = match plane {
            MirrorPlaneChoice::XY => (DVec3::ZERO, DVec3::Z),
            MirrorPlaneChoice::XZ => (DVec3::ZERO, DVec3::Y),
            MirrorPlaneChoice::YZ => (DVec3::ZERO, DVec3::X),
            MirrorPlaneChoice::Datum(i) => {
                let d = self.datum_planes.get(i)?;
                return Some((d.plane.origin.as_dvec3(), d.plane.normal.as_dvec3()));
            }
        };
        let point = if through_center { self.body_bbox_center(body).unwrap_or(point) } else { point };
        Some((point, normal))
    }

    /// Pasang geometri hasil (salinan baru atau ganti body asli).
    fn commit_body_result(&mut self, id: BodyId, geo: BodyGeometry, keep_original: bool, suffix_key: &str, cmd_label: &'static str, detail: &str) {
        if keep_original {
            let base = self.model.doc.bodies.get(id).map(|b| b.name.clone()).unwrap_or_default();
            let name = format!("{base} ({})", ducad_i18n::t!(suffix_key));
            self.execute_model_command(Box::new(AddSolidCommand::new(name, geo)), detail);
        } else {
            self.execute_model_command(
                Box::new(ReplaceGeometryCommand::new(cmd_label, id, geo)),
                detail,
            );
            self.archive_round_history(id);
            self.hole_history.remove(&id);
        }
        self.model_status = Some(detail.to_string());
    }

    pub fn mirror_selected_body(&mut self, plane: MirrorPlaneChoice, through_center: bool, keep_original: bool) {
        let Some(id) = self.single_selected_body() else { return };
        let Some((point, normal)) = self.mirror_plane(id, plane, through_center) else {
            self.model_status = Some(ducad_i18n::t!("mirror-body-no-plane"));
            return;
        };
        // Op `mirror` (salinan baru atau ganti body asli).
        let r = self.mirror_body_via_oplog(id, point, normal, keep_original);
        if let Some(ok) = self.try_gui_op("Mirror", r) {
            if ok {
                self.archive_round_history(id);
                self.hole_history.remove(&id);
                self.model_status = Some(ducad_i18n::t!("mirror-body-done"));
            }
            return;
        }
        let Some(geo) = self.model.geometry.get(id) else { return };
        match compute::advanced::mirror(&geo.shape, point, normal) {
            Ok(geo) => self.commit_body_result(id, geo, keep_original, "mirror-body-suffix", "Cermin Body", &ducad_i18n::t!("mirror-body-done")),
            Err(e) => self.model_status = Some(crate::modeling::operations::op_status("Mirror", &e)),
        }
    }

    pub fn rotate_selected_body_precise(&mut self, axis: [f64; 3], angle_deg: f64, about_center: bool, keep_original: bool) {
        let Some(id) = self.single_selected_body() else { return };
        if !angle_deg.is_finite() || angle_deg.abs() < 1e-9 {
            self.model_status = Some(ducad_i18n::t!("rotate-body-zero"));
            return;
        }
        let pivot = if about_center { self.body_bbox_center(id).unwrap_or(DVec3::ZERO) } else { DVec3::ZERO };
        // Op `transform` (putar), `copy` = salinan baru.
        let r = self.transform_body_via_oplog(id, None, Some((pivot, DVec3::from(axis), angle_deg)), keep_original);
        if let Some(ok) = self.try_gui_op("Rotate", r) {
            if ok {
                if !keep_original {
                    self.archive_round_history(id);
                    self.hole_history.remove(&id);
                }
                let angle = format!("{angle_deg:.1}");
                self.model_status = Some(ducad_i18n::t!("rotate-body-done", angle = angle.as_str()));
            }
            return;
        }
        let Some(geo) = self.model.geometry.get(id) else { return };
        match ducad_kernel::rotate_shape(
            &geo.shape,
            (pivot.x, pivot.y, pivot.z),
            (axis[0], axis[1], axis[2]),
            angle_deg.to_radians(),
        ) {
            Ok(s) => {
                let angle = format!("{angle_deg:.1}");
                let detail = ducad_i18n::t!("rotate-body-done", angle = angle.as_str());
                self.commit_body_result(id, BodyGeometry::from_shape(s), keep_original, "rotate-body-suffix", "Putar Body", &detail);
            }
            Err(e) => self.model_status = Some(format!("Rotate: {e:#}")),
        }
    }

    fn handle_body_tool_event(&mut self, ev: BodyToolEvent) {
        match ev {
            BodyToolEvent::CreatePrimitive { kind, dims, at, centered } => {
                self.create_primitive(kind, dims, at, centered)
            }
            BodyToolEvent::MirrorBody { plane, through_center, keep_original } => {
                self.mirror_selected_body(plane, through_center, keep_original)
            }
            BodyToolEvent::RotateBody { axis, angle_deg, about_center, keep_original } => {
                self.rotate_selected_body_precise(axis.vector(), angle_deg, about_center, keep_original)
            }
        }
    }

    /// Render panel alat body yang terbuka (pojok kanan bawah).
    pub fn show_body_tools(&mut self, ctx: &egui::Context, screen_rect: egui::Rect) {
        let Some(panel) = self.body_tools.open else { return };
        let (title, icon) = match panel {
            BodyToolPanel::Primitive => (ducad_i18n::t!("prim-title"), egui_icons::icons::ICON_VIEW_IN_AR.codepoint),
            BodyToolPanel::MirrorBody => (ducad_i18n::t!("mirror-body-title"), egui_icons::icons::ICON_FLIP.codepoint),
            BodyToolPanel::RotateBody => (ducad_i18n::t!("rotate-body-title"), egui_icons::icons::ICON_ROTATE_RIGHT.codepoint),
        };
        let datums: Vec<String> = self.datum_planes.iter().map(|d| d.name.clone()).collect();
        let mut custom_h = self.body_tools.custom_height;
        let state = &mut self.body_tools;
        let (ev, closed) = ducad_ui::render_bottom_right_panel_custom(
            ctx,
            "body_tools_popup",
            &title,
            icon,
            ducad_ui::theme::ACCENT_BLUE,
            screen_rect,
            screen_rect.max.y - 62.0,
            (screen_rect.height() - 140.0).max(300.0),
            &mut custom_h,
            300.0,
            ducad_ui::BOTTOM_RIGHT_PANEL_WIDTH,
            false,
            |ui| {
                let ev = match panel {
                    BodyToolPanel::Primitive => BodyToolsPopup::show_primitive(ui, &mut state.primitive),
                    BodyToolPanel::MirrorBody => BodyToolsPopup::show_mirror(ui, &mut state.mirror, &datums),
                    BodyToolPanel::RotateBody => BodyToolsPopup::show_rotate(ui, &mut state.rotate),
                };
                (ev, false)
            },
        );
        self.body_tools.custom_height = custom_h;
        if closed {
            self.body_tools.open = None;
        }
        if let Some(ev) = ev.flatten() {
            self.handle_body_tool_event(ev);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_ui::RotateAxisChoice;

    fn app_with_box() -> (DuCADApp, BodyId) {
        let mut app = DuCADApp::new_for_test();
        app.create_primitive(PrimitiveKind::Box, [10.0, 20.0, 30.0], [5.0, 0.0, 0.0], false);
        let id = app.model.doc.bodies.keys().next().expect("box dibuat");
        app.selected_bodies = [id].into_iter().collect();
        (app, id)
    }

    #[test]
    fn primitive_box_adds_body_with_volume() {
        let (app, id) = app_with_box();
        let v = app.model.geometry[id].shape.volume();
        assert!((v - 6000.0).abs() < 1.0, "volume {v}");
    }

    #[test]
    fn primitive_rejects_bad_dims_without_panic() {
        let mut app = DuCADApp::new_for_test();
        for dims in [[0.0, 1.0, 1.0], [f64::NAN, 1.0, 1.0], [-5.0, 1.0, 1.0]] {
            app.create_primitive(PrimitiveKind::Box, dims, [0.0; 3], false);
        }
        app.create_primitive(PrimitiveKind::Sphere, [1.0, 0.0, 0.0], [f64::INFINITY, 0.0, 0.0], false);
        assert!(app.model.doc.bodies.is_empty());
        assert!(app.model_status.is_some());
    }

    #[test]
    fn mirror_copy_adds_second_body_on_other_side() {
        let (mut app, id) = app_with_box();
        app.mirror_selected_body(MirrorPlaneChoice::YZ, false, true);
        assert_eq!(app.model.doc.bodies.len(), 2);
        let other = app.model.doc.bodies.keys().find(|k| *k != id).unwrap();
        let c = app.body_bbox_center(other).unwrap();
        assert!((c.x + 10.0).abs() < 0.5, "pusat cermin {c:?}");
    }

    #[test]
    fn rotate_replace_keeps_volume() {
        let (mut app, id) = app_with_box();
        app.rotate_selected_body_precise(RotateAxisChoice::Z.vector(), 90.0, true, false);
        assert_eq!(app.model.doc.bodies.len(), 1);
        let v = app.model.geometry[id].shape.volume();
        assert!((v - 6000.0).abs() < 1.0);
    }

    #[test]
    fn rigid_group_from_selected_bodies() {
        let mut app = DuCADApp::new_for_test();
        app.create_primitive(PrimitiveKind::Box, [5.0; 3], [0.0; 3], false);
        app.create_primitive(PrimitiveKind::Box, [5.0; 3], [20.0, 0.0, 0.0], false);
        app.selected_bodies = app.model.doc.bodies.keys().take(1).collect();
        app.make_rigid_group_from_selection();
        assert!(app.assembly_tree.rigid_groups.is_empty(), "1 body tidak cukup");
        app.selected_bodies = app.model.doc.bodies.keys().collect();
        app.make_rigid_group_from_selection();
        assert_eq!(app.assembly_tree.rigid_groups.len(), 1);
        app.dissolve_rigid_group_of_selection();
        assert!(app.assembly_tree.rigid_groups.is_empty());
    }

    #[test]
    fn rotate_without_selection_reports_status() {
        let mut app = DuCADApp::new_for_test();
        app.rotate_selected_body_precise([0.0, 0.0, 1.0], 45.0, true, false);
        assert!(app.model_status.is_some());
    }
}
