use std::collections::HashSet;
use std::path::PathBuf;

use ducad_core::{BodyId, LengthUnit};
use ducad_kernel::{FaceHit, PickRay};
use ducad_render::{OrbitCamera, PlaneKind, SceneRenderer, SketchPlane, ViewPreset};
use ducad_sketch::constraint::Constraint;
use ducad_sketch::{
    detect_rectangle, DeleteEntities, Entity, EntityId, RectAnchor, ResizeRectangle, Sketch,
    SnapHit, UpdateEntity,
};
use ducad_ui::{
    BodyItemInfo, CanvasHud, CanvasHudEvent, CommandPalette, ContextAction, ContextActionBar,
    FeatureInspector, FeatureInspectorState, InspectorBooleanKind, InspectorConstraintAction,
    InspectorEvent, InspectorPickMode, InspectorRectAnchor, ItemsDrawer, ItemsDrawerEvent,
    LeftToolbar, RadialMenu, SelectedBodyData, SelectedEntityData, SketchPlaneItemInfo, ThemeMode,
    ToolbarEvent, TopBar, TopBarEvent, TopBarFileOp, TopBarState, ViewCube, ViewCubeAction,
};
use eframe::egui;
use eframe::egui_wgpu;
use glam::{DVec2, Vec3};
use slotmap::Key;

use crate::import_worker::ImportWorker;
use crate::model::{BooleanKind, ModelDoc};
use crate::types::{
    InspectorContentSig, PickMode, PickedEdge, RoundHistory, SectionAxis, ToolKind,
};
use crate::viewport::{pixel_tolerance_to_world, screen_to_plane_point, ViewportCallback};

pub struct DuCADApp {
    pub camera: OrbitCamera,
    pub sketches: [Sketch; 3],
    pub undos: [ducad_sketch::UndoStack; 3],

    pub tool: ToolKind,
    pub pending_points: Vec<DVec2>,
    pub pending_point_refs: Vec<ducad_sketch::constraint::PointRef>,
    pub offset_source: Option<EntityId>,
    pub line_chain_start: Option<DVec2>,
    pub line_chain_segments: u32,

    pub hovered: Option<EntityId>,
    pub selected: HashSet<EntityId>,
    pub last_snap: Option<SnapHit>,

    pub dynamic_input: String,
    pub dynamic_focus_pending: bool,
    pub constraint_status: Option<String>,

    pub model: ModelDoc,
    pub model_undo: ducad_core::UndoStack<ModelDoc>,
    pub selected_bodies: HashSet<BodyId>,
    pub model_status: Option<String>,
    pub extrude_distance_input: String,
    pub fillet_radius_input: String,
    pub chamfer_distance_input: String,
    pub shell_thickness_input: String,
    pub shell_direction: ducad_kernel::Direction,

    pub pending_loft_bottom: Option<ducad_kernel::Profile>,
    pub loft_height_input: String,

    pub picking_mode: PickMode,
    pub selected_edges: Vec<PickedEdge>,
    pub selected_faces: Vec<PickRay>,
    pub active_face: Option<(BodyId, PickRay, FaceHit)>,
    pub face_extrude_distance_input: String,

    pub active_vertex: Option<(BodyId, PickRay, (f64, f64, f64))>,

    pub current_file_path: Option<PathBuf>,
    pub file_status: Option<String>,

    pub theme: ThemeMode,
    pub palette: CommandPalette,
    pub radial_menu: RadialMenu,
    pub radial_press: Option<(egui::Pos2, f64)>,
    pub radial_suppress_click: bool,
    pub two_finger_tap_press: Option<egui::MultiTouchInfo>,

    pub measurements: Vec<crate::types::Measurement>,

    pub alert_modal: ducad_ui::AlertModalState,
    pub revolve_dialog: ducad_ui::RevolveDialogState,
    pub revolve_angle_setting: f64,
    pub revolve_reverse: bool,
    pub revolve_staged_axis: Option<(glam::DVec2, glam::DVec2)>,

    pub section_enabled: bool,
    pub section_axis: SectionAxis,
    pub section_offset: f32,
    pub section_invert: bool,

    pub show_all_dimensions: bool,
    /// Entity yg pill dimensinya sedang dibuka utk diedit di kanvas (Fase 3 —
    /// klik pill saat "Tampilkan Semua Ukuran" aktif). `None` = tidak ada popup.
    pub editing_dimension_entity: Option<EntityId>,
    pub editing_dimension_input: String,

    pub import_worker: ImportWorker,
    pub pending_imports: u32,

    pub left_toolbar: LeftToolbar,
    pub items_drawer: ItemsDrawer,
    pub viewcube: ViewCube,
    pub feature_inspector_open: bool,
    pub auto_hide_properties: bool,
    pub items_drawer_open: bool,
    pub plane_menu_open: bool,
    pub left_toolbar_content_sig: Option<bool>,
    pub inspector_content_sig: Option<InspectorContentSig>,
    pub prop_input_p1_x: String,
    pub prop_input_p1_y: String,
    pub prop_input_p2_x: String,
    pub prop_input_p2_y: String,
    pub prop_input_val_1: String,
    pub prop_input_val_2: String,
    /// Diameter Circle — sinkron dua-arah dgn `prop_input_val_1` (radius).
    pub prop_input_val_3: String,
    pub prop_input_rect_p: String,
    pub prop_input_rect_l: String,
    pub rect_anchor: InspectorRectAnchor,
    pub last_inspected_entity_id: Option<u64>,

    // Fase 4: resize body 3D via pill dimensi langsung di viewport (bukan panel —
    // lihat `editing_body_dim_axis` di overlay/dimensions.rs).
    pub editing_body_dim_axis: Option<usize>,
    pub editing_body_dim_input: String,
    pub editing_edge_dim: Option<(BodyId, usize)>,
    pub editing_edge_dim_input: String,

    pub is_sketching: bool,
    pub active_plane: SketchPlane,
    pub unit: LengthUnit,

    pub extruding_from_gizmo: bool,
    pub gizmo_distance: f64,
    pub gizmo_dimension_editing: bool,
    pub gizmo_edit_input: String,
    pub gizmo_is_cutting: bool,
    pub gizmo_target_body: Option<BodyId>,

    pub extruding_face_from_gizmo: bool,
    pub face_gizmo_distance: f64,
    pub face_gizmo_dimension_editing: bool,
    pub face_gizmo_edit_input: String,

    pub filleting_vertex_from_gizmo: bool,
    pub vertex_gizmo_radius: f64,
    pub vertex_gizmo_dimension_editing: bool,
    pub vertex_gizmo_edit_input: String,

    pub hovered_vertex_marker: Option<(BodyId, (f64, f64, f64))>,
    pub active_edge: Option<(BodyId, PickRay, (f64, f64, f64))>,

    pub filleting_edge_from_gizmo: bool,
    pub edge_gizmo_radius: f64,
    pub edge_gizmo_dimension_editing: bool,
    pub edge_gizmo_edit_input: String,

    pub round_history: std::collections::HashMap<BodyId, RoundHistory>,
    pub editing_round: Option<(BodyId, usize)>,

    pub sketch_move_target: Option<HashSet<EntityId>>,
    pub sketch_move_dragging: bool,
    pub sketch_move_delta: DVec2,
    pub sketch_move_armed: bool,
    pub body_move_target: Option<BodyId>,
    pub body_move_dragging: bool,
    pub body_move_delta: Vec3,
    pub body_move_armed: bool,
    pub body_transform_part: Option<ducad_render::sketch::TransformGizmoPart>,
    pub body_rotate_axis: Vec3,
    pub body_rotate_angle_deg: f64,
    pub body_rotate_dragging: bool,
    pub body_rotate_editing: bool,
    pub body_rotate_edit_input: String,
    pub body_copy_mode: bool,

    pub last_select_click: Option<(egui::Pos2, usize)>,
    pub last_body_select_click: Option<(BodyId, std::time::Instant)>,
}

impl DuCADApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let render_state = cc
            .wgpu_render_state
            .as_ref()
            .expect("DUCAD membutuhkan backend wgpu");
        let scene = SceneRenderer::new(
            &render_state.device,
            render_state.target_format,
            Some(ducad_render::wgpu::TextureFormat::Depth32Float),
        );
        render_state
            .renderer
            .write()
            .callback_resources
            .insert(scene);

        let theme = ThemeMode::default();
        ducad_ui::apply_theme(&cc.egui_ctx, theme);

        cc.egui_ctx
            .options_mut(|o| o.input_options.horizontal_scroll_modifier = egui::Modifiers::NONE);

        Self {
            camera: OrbitCamera::default(),
            sketches: [Sketch::default(), Sketch::default(), Sketch::default()],
            undos: [
                ducad_sketch::UndoStack::default(),
                ducad_sketch::UndoStack::default(),
                ducad_sketch::UndoStack::default(),
            ],
            tool: ToolKind::Select,
            pending_points: Vec::new(),
            pending_point_refs: Vec::new(),
            offset_source: None,
            line_chain_start: None,
            line_chain_segments: 0,
            hovered: None,
            selected: HashSet::new(),
            last_snap: None,
            dynamic_input: String::new(),
            dynamic_focus_pending: false,
            constraint_status: None,

            model: ModelDoc::default(),
            model_undo: ducad_core::UndoStack::default(),
            selected_bodies: HashSet::new(),
            model_status: None,
            extrude_distance_input: "10".to_string(),
            fillet_radius_input: "2".to_string(),
            chamfer_distance_input: "2".to_string(),
            shell_thickness_input: "2".to_string(),
            shell_direction: ducad_kernel::Direction::PosZ,

            pending_loft_bottom: None,
            loft_height_input: "10".to_string(),
            picking_mode: PickMode::default(),
            selected_edges: Vec::new(),
            selected_faces: Vec::new(),
            active_face: None,
            face_extrude_distance_input: "15".to_string(),
            active_vertex: None,

            current_file_path: None,
            file_status: None,

            theme,
            palette: CommandPalette::default(),
            radial_menu: RadialMenu::default(),
            radial_press: None,
            radial_suppress_click: false,
            two_finger_tap_press: None,

            measurements: Vec::new(),

            alert_modal: ducad_ui::AlertModalState::default(),
            revolve_dialog: ducad_ui::RevolveDialogState::default(),
            revolve_angle_setting: 360.0,
            revolve_reverse: false,
            revolve_staged_axis: None,

            section_enabled: false,
            section_axis: SectionAxis::Z,
            section_offset: 0.0,
            section_invert: false,
            show_all_dimensions: false,
            editing_dimension_entity: None,
            editing_dimension_input: String::new(),

            import_worker: ImportWorker::spawn(),
            pending_imports: 0,

            left_toolbar: LeftToolbar::default(),
            items_drawer: ItemsDrawer::default(),
            viewcube: ViewCube::default(),
            feature_inspector_open: true,
            auto_hide_properties: true,
            items_drawer_open: false,
            plane_menu_open: false,
            left_toolbar_content_sig: None,
            inspector_content_sig: None,
            prop_input_p1_x: String::new(),
            prop_input_p1_y: String::new(),
            prop_input_p2_x: String::new(),
            prop_input_p2_y: String::new(),
            prop_input_val_1: String::new(),
            prop_input_val_2: String::new(),
            prop_input_val_3: String::new(),
            prop_input_rect_p: String::new(),
            prop_input_rect_l: String::new(),
            rect_anchor: InspectorRectAnchor::Center,
            last_inspected_entity_id: None,

            editing_body_dim_axis: None,
            editing_body_dim_input: String::new(),
            editing_edge_dim: None,
            editing_edge_dim_input: String::new(),

            is_sketching: true,
            active_plane: SketchPlane::top(),
            unit: LengthUnit::Millimeters,

            extruding_from_gizmo: false,
            gizmo_distance: 20.0,
            gizmo_dimension_editing: false,
            gizmo_edit_input: "20".to_string(),
            gizmo_is_cutting: false,
            gizmo_target_body: None,

            extruding_face_from_gizmo: false,
            face_gizmo_distance: 15.0,
            face_gizmo_dimension_editing: false,
            face_gizmo_edit_input: "15".to_string(),

            filleting_vertex_from_gizmo: false,
            vertex_gizmo_radius: 3.0,
            vertex_gizmo_dimension_editing: false,
            vertex_gizmo_edit_input: "3".to_string(),

            hovered_vertex_marker: None,
            active_edge: None,
            filleting_edge_from_gizmo: false,
            edge_gizmo_radius: 3.0,
            edge_gizmo_dimension_editing: false,
            edge_gizmo_edit_input: "3".to_string(),

            round_history: std::collections::HashMap::new(),
            editing_round: None,

            sketch_move_target: None,
            sketch_move_dragging: false,
            sketch_move_delta: DVec2::ZERO,
            sketch_move_armed: false,
            body_move_target: None,
            body_move_dragging: false,
            body_move_delta: Vec3::ZERO,
            body_move_armed: false,
            body_transform_part: None,
            body_rotate_axis: Vec3::Z,
            body_rotate_angle_deg: 0.0,
            body_rotate_dragging: false,
            body_rotate_editing: false,
            body_rotate_edit_input: "0".to_string(),
            body_copy_mode: false,
            last_select_click: None,
            last_body_select_click: None,
        }
    }

    /// Saat gizmo extrude/push-pull mulai di-drag di mode sketsa, otomatis pindah ke mode 3D
    /// supaya hasil extrude langsung terlihat tanpa harus menekan Cmd+Shift+3 manual.
    pub fn auto_enter_3d_mode_on_extrude_drag(&mut self) {
        if self.is_sketching {
            self.is_sketching = false;
            self.left_toolbar.is_sketching = false;
            self.set_tool(ToolKind::Select);
        }
    }

    pub fn viewport(&mut self, ui: &mut egui::Ui) {
        let (rect, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());

        let raw_cursor = response
            .hover_pos()
            .and_then(|p| screen_to_plane_point(&self.camera, rect, p, &self.active_plane));

        self.handle_radial_menu(ui, &response);

        let is_near_gizmo = self.check_near_gizmo(rect, response.hover_pos());
        if is_near_gizmo || self.extruding_from_gizmo || self.extruding_face_from_gizmo {
            let arrow_opt = if let Some(c) = self.selected_closed_region_centroid() {
                let (_, arrow) =
                    self.project_screen_drag_to_extrude_axis(rect, c, egui::Vec2::ZERO);
                arrow
            } else if let Some((_, _, hit)) = &self.active_face {
                let anchor = hit.gizmo_anchor();
                let c_base = Vec3::new(anchor.0 as f32, anchor.1 as f32, anchor.2 as f32);
                let pull_dir = Vec3::new(
                    hit.pull_dir.0 as f32,
                    hit.pull_dir.1 as f32,
                    hit.pull_dir.2 as f32,
                );
                let (_, arrow) = self.project_screen_drag_to_world_axis(
                    rect,
                    c_base,
                    pull_dir,
                    egui::Vec2::ZERO,
                );
                arrow
            } else {
                None
            };
            if let Some(dir) = arrow_opt {
                let u = dir.normalized();
                let cursor = if u.x.abs() > u.y.abs() * 2.0 {
                    egui::CursorIcon::ResizeHorizontal
                } else if u.y.abs() > u.x.abs() * 2.0 {
                    egui::CursorIcon::ResizeVertical
                } else if u.x * u.y < 0.0 {
                    egui::CursorIcon::ResizeNeSw
                } else {
                    egui::CursorIcon::ResizeNwSe
                };
                ui.ctx().set_cursor_icon(cursor);
            } else {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
            }
        }

        if let Some((_, _, hit)) = &self.active_face {
            let anchor = hit.gizmo_anchor();
            let c_base = Vec3::new(anchor.0 as f32, anchor.1 as f32, anchor.2 as f32);
            let pull_dir = Vec3::new(
                hit.pull_dir.0 as f32,
                hit.pull_dir.1 as f32,
                hit.pull_dir.2 as f32,
            );

            if is_near_gizmo && response.drag_started_by(egui::PointerButton::Primary) {
                self.extruding_face_from_gizmo = true;
                if self.face_gizmo_distance == 0.0 {
                    self.face_gizmo_distance = 15.0;
                }
                self.auto_enter_3d_mode_on_extrude_drag();
            }

            if self.extruding_face_from_gizmo
                && response.dragged_by(egui::PointerButton::Primary)
            {
                let (delta_mm, _) = self.project_screen_drag_to_world_axis(
                    rect,
                    c_base,
                    pull_dir,
                    response.drag_delta(),
                );
                self.face_gizmo_distance += delta_mm;
                self.face_gizmo_edit_input = format!(
                    "{:.0}",
                    self.unit.to_display_val(self.face_gizmo_distance)
                );
            }

            if self.extruding_face_from_gizmo && response.drag_stopped() {
                if self.face_gizmo_distance.abs() > 0.1 {
                    self.extrude_active_face(self.face_gizmo_distance);
                }
                self.extruding_face_from_gizmo = false;
                self.face_gizmo_distance = 15.0;
                self.face_gizmo_edit_input = "15".to_string();
            }
        }

        if let Some(c) = self.selected_closed_region_centroid() {
            if is_near_gizmo && response.drag_started_by(egui::PointerButton::Primary) {
                self.extruding_from_gizmo = true;
                if self.gizmo_distance == 0.0 {
                    self.gizmo_distance = 20.0;
                }
                self.auto_enter_3d_mode_on_extrude_drag();
            }

            if self.extruding_from_gizmo && response.dragged_by(egui::PointerButton::Primary) {
                let (delta_mm, _) = self.project_screen_drag_to_extrude_axis(
                    rect,
                    c,
                    response.drag_delta(),
                );
                self.gizmo_distance += delta_mm;
                self.update_gizmo_boolean_detection();
            }

            if self.extruding_from_gizmo && response.drag_stopped() {
                self.commit_gizmo_extrusion();
            }
        }

        let radial_active = self.radial_menu.is_open() || self.radial_press.is_some();
        let allow_primary_orbit = self.tool == ToolKind::Select
            && !radial_active
            && !is_near_gizmo
            && !self.extruding_from_gizmo
            && !self.extruding_face_from_gizmo;
        self.handle_navigation(ui, &response, rect, allow_primary_orbit);
        self.handle_sketch_input(ui, &response, rect, raw_cursor);

        let aspect = rect.width() / rect.height().max(1.0);
        let world_scale = pixel_tolerance_to_world(&self.camera, rect);
        let overlay = self.build_overlay_lines(raw_cursor, world_scale);
        let (body_positions, body_normals, body_colors, body_indices) =
            self.build_combined_body_mesh();
        let (gizmo_positions, gizmo_normals, gizmo_colors, gizmo_indices) =
            self.build_gizmo_mesh(world_scale);
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            ViewportCallback {
                view_proj: self.camera.view_proj(aspect),
                eye: self.camera.eye(),
                sketch_plane: self.active_plane,
                overlay_lines: overlay,
                body_positions,
                body_normals,
                body_colors,
                body_indices,
                gizmo_positions,
                gizmo_normals,
                gizmo_colors,
                gizmo_indices,
                clip_plane: self.section_clip_plane(),
            },
        ));

        self.dynamic_input_ui(ui, rect, raw_cursor);
    }
}

impl eframe::App for DuCADApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_import_worker();
        if self.pending_imports > 0 {
            ctx.request_repaint();
        }

        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::K)) {
            self.palette.toggle();
        }
        let mode_sketch_pressed = ctx.input(|i| {
            i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::Num2)
        });
        let mode_3d_pressed = ctx.input(|i| {
            i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::Num3)
        });
        if mode_sketch_pressed && !self.is_sketching {
            self.is_sketching = true;
            self.left_toolbar.is_sketching = true;
            self.camera.orient_to_plane(&self.active_plane);
        }
        if mode_3d_pressed && self.is_sketching {
            self.is_sketching = false;
            self.left_toolbar.is_sketching = false;
            self.set_tool(ToolKind::Select);
        }

        let undo_pressed = ctx.input(|i| {
            i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::Z)
        });
        let redo_pressed = ctx.input(|i| {
            i.modifiers.command
                && (i.key_pressed(egui::Key::Y)
                    || (i.modifiers.shift && i.key_pressed(egui::Key::Z)))
        });
        if undo_pressed {
            self.undo_active_sketch();
        }
        if redo_pressed {
            self.redo_active_sketch();
        }

        let save_as_pressed = ctx.input(|i| {
            i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::S)
        });
        let save_pressed = ctx.input(|i| {
            i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::S)
        });
        let open_pressed = ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::O));
        if save_as_pressed {
            self.save_native_as();
        } else if save_pressed {
            self.save_native();
        }
        if open_pressed {
            self.open_native();
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                self.viewport(ui);
            });

        let screen_rect = ctx.content_rect();
        let screen_center_x = screen_rect.center().x;

        let topbar_margin_right = 12.0;
        let topbar_x = 12.0;
        let topbar_w = (screen_rect.max.x - topbar_x - topbar_margin_right).max(200.0);
        let doc_name = self
            .current_file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled.ducad")
            .to_string();
        let is_saved = self.current_file_path.is_some();

        let mut topbar_state = TopBarState {
            document_name: doc_name,
            status_saved: is_saved,
            current_unit: self.model.doc.unit,
            is_sketching: self.is_sketching,
            items_drawer_open: self.items_drawer_open,
            section_view_active: self.section_enabled,
            is_measure_active: matches!(self.tool, ToolKind::Measure | ToolKind::MeasureAngle),
            active_plane_name: self.active_plane.name().to_string(),
            plane_menu_open: self.plane_menu_open,
            items_button_rect: egui::Rect::NOTHING,
        };

        egui::Area::new(egui::Id::new("ducad-topbar-area"))
            .fixed_pos(egui::pos2(topbar_x, 10.0))
            .order(egui::Order::Foreground)
            .show(&ctx, |ui| {
                ui.set_width(topbar_w);
                if let Some(top_event) = TopBar::show(ui, &mut topbar_state) {
                    match top_event {
                        TopBarEvent::HomeClicked => {
                            self.new_document();
                        }
                        TopBarEvent::SetUnit(u) => {
                            self.unit = u;
                            self.model.doc.unit = u;
                        }
                        TopBarEvent::File(op) => match op {
                            TopBarFileOp::New => self.new_document(),
                            TopBarFileOp::Open => self.open_native(),
                            TopBarFileOp::Save => self.save_native(),
                            TopBarFileOp::SaveAs => self.save_native_as(),
                            TopBarFileOp::ImportStep => self.import_step(),
                            TopBarFileOp::ImportDxf => self.import_dxf(),
                            TopBarFileOp::ExportStep => self.export_step(),
                            TopBarFileOp::ExportStl => self.export_stl(),
                            TopBarFileOp::ExportObj => self.export_obj(),
                            TopBarFileOp::ExportDxf => self.export_dxf(),
                        },
                        TopBarEvent::ToggleTheme => {
                            self.theme = self.theme.toggled();
                            ducad_ui::apply_theme(&ctx, self.theme);
                        }
                        TopBarEvent::OpenCommandPalette => {
                            self.palette.open();
                        }
                        TopBarEvent::ToggleItemsDrawer => {
                            self.items_drawer_open = !self.items_drawer_open;
                        }
                        TopBarEvent::OpenSearch => {
                            self.palette.open();
                        }
                        TopBarEvent::EnterSketching => {
                            self.is_sketching = true;
                            self.left_toolbar.is_sketching = true;
                            self.camera.orient_to_plane(&self.active_plane);
                        }
                        TopBarEvent::ExitSketching => {
                            self.is_sketching = false;
                            self.left_toolbar.is_sketching = false;
                            self.set_tool(ToolKind::Select);
                        }
                        TopBarEvent::SelectSketchPlane(idx) => {
                            let kind = match idx {
                                0 => PlaneKind::Top,
                                1 => PlaneKind::Front,
                                2 => PlaneKind::Right,
                                _ => PlaneKind::Top,
                            };
                            self.set_sketch_plane(kind);
                        }
                        TopBarEvent::ToggleSectionView => {
                            self.section_enabled = !self.section_enabled;
                        }
                        TopBarEvent::ToggleMeasurements => {
                            let already_active =
                                matches!(self.tool, ToolKind::Measure | ToolKind::MeasureAngle);
                            self.set_tool(if already_active {
                                ToolKind::Select
                            } else {
                                ToolKind::Measure
                            });
                        }
                        TopBarEvent::DeleteSelection => {
                            if !self.selected.is_empty() {
                                let to_delete: Vec<EntityId> =
                                    self.selected.iter().copied().collect();
                                self.execute_sketch_command(Box::new(DeleteEntities::new(
                                    to_delete,
                                )));
                                self.selected.clear();
                            }
                            if !self.selected_bodies.is_empty() {
                                self.delete_selected_bodies();
                            }
                        }
                    }
                }
            });

        self.plane_menu_open = topbar_state.plane_menu_open;
        let items_button_rect = topbar_state.items_button_rect;

        self.left_toolbar.is_sketching = self.is_sketching;
        let left_toolbar_force_resize = self.left_toolbar_content_sig != Some(self.is_sketching);
        self.left_toolbar_content_sig = Some(self.is_sketching);
        egui::Area::new(egui::Id::new("ducad-left-toolbar-area"))
            .fixed_pos(egui::pos2(12.0, screen_rect.center().y))
            .pivot(egui::Align2::LEFT_CENTER)
            .constrain_to(screen_rect)
            .default_size(egui::vec2(60.0, 460.0))
            .sizing_pass(left_toolbar_force_resize)
            .order(egui::Order::Foreground)
            .show(&ctx, |ui| {
                if let Some(tb_ev) = self.left_toolbar.show(ui, self.tool.to_toolbar_tool()) {
                    match tb_ev {
                        ToolbarEvent::SelectTool(ducad_ui::ToolbarTool::Revolve) => {
                            self.feature_inspector_open = true;
                            self.set_tool(ToolKind::Revolve);
                        }
                        ToolbarEvent::SelectTool(t) => {
                            self.set_tool(ToolKind::from_toolbar_tool(t));
                        }
                    }
                }
            });

        if self.items_drawer_open {
            let sketch_planes = vec![
                SketchPlaneItemInfo {
                    index: 0,
                    name: format!(
                        "Plane 01 - Top (XY) ({})",
                        self.sketches[0].entities.len()
                    ),
                    active: self.active_plane.kind == PlaneKind::Top,
                    visible: true,
                },
                SketchPlaneItemInfo {
                    index: 1,
                    name: format!(
                        "Plane 02 - Front (XZ) ({})",
                        self.sketches[1].entities.len()
                    ),
                    active: self.active_plane.kind == PlaneKind::Front,
                    visible: true,
                },
                SketchPlaneItemInfo {
                    index: 2,
                    name: format!(
                        "Plane 03 - Right (YZ) ({})",
                        self.sketches[2].entities.len()
                    ),
                    active: self.active_plane.kind == PlaneKind::Right,
                    visible: true,
                },
            ];
            let bodies: Vec<BodyItemInfo> = self
                .model
                .doc
                .bodies
                .iter()
                .map(|(id, b)| BodyItemInfo {
                    id_raw: id.data().as_ffi(),
                    name: b.name.clone(),
                    visible: b.visible,
                    selected: self.selected_bodies.contains(&id),
                })
                .collect();

            let drawer_pos =
                egui::pos2(items_button_rect.left(), items_button_rect.bottom() + 6.0);
            egui::Area::new(egui::Id::new("ducad-items-drawer-area"))
                .fixed_pos(drawer_pos)
                .order(egui::Order::Foreground)
                .show(&ctx, |ui| {
                    if let Some(ev) = self.items_drawer.show(ui, &sketch_planes, &bodies) {
                        match ev {
                            ItemsDrawerEvent::ToggleBodyVisibility(raw_id) => {
                                for (id, b) in self.model.doc.bodies.iter_mut() {
                                    if id.data().as_ffi() == raw_id {
                                        b.visible = !b.visible;
                                        break;
                                    }
                                }
                            }
                            ItemsDrawerEvent::SelectBody { id_raw, extend } => {
                                for (id, _) in self.model.doc.bodies.iter() {
                                    if id.data().as_ffi() == id_raw {
                                        if !extend {
                                            self.selected_bodies.clear();
                                        }
                                        if !self.selected_bodies.remove(&id) {
                                            self.selected_bodies.insert(id);
                                        }
                                        self.model_status = None;
                                        break;
                                    }
                                }
                            }
                            ItemsDrawerEvent::ToggleSketchVisibility(_) => {}
                            ItemsDrawerEvent::SelectSketchPlane(idx) => {
                                let kind = match idx {
                                    0 => PlaneKind::Top,
                                    1 => PlaneKind::Front,
                                    2 => PlaneKind::Right,
                                    _ => PlaneKind::Top,
                                };
                                self.set_sketch_plane(kind);
                            }
                        }
                    }
                });
        }

        let is_editing_or_drawing = self.tool != ToolKind::Select;
        let has_active_selection =
            !self.selected.is_empty() || !self.selected_bodies.is_empty();
        let measure_tool_active = matches!(self.tool, ToolKind::Measure | ToolKind::MeasureAngle);
        let has_measurements = !self.measurements.is_empty();
        let show_right_sidebar = if self.auto_hide_properties {
            ((!is_editing_or_drawing && has_active_selection)
                || measure_tool_active
                || has_measurements)
                && self.feature_inspector_open
        } else {
            self.feature_inspector_open
        };

        let viewcube_y = 102.0;
        let viewcube_pos = egui::pos2(screen_rect.max.x - topbar_margin_right - 42.0, viewcube_y);
        egui::Area::new(egui::Id::new("ducad-viewcube-area"))
            .fixed_pos(viewcube_pos - egui::vec2(42.0, 42.0))
            .order(egui::Order::Foreground)
            .show(&ctx, |ui| {
                if let Some(action) = self.viewcube.show(
                    ui,
                    viewcube_pos,
                    self.camera.yaw,
                    self.camera.pitch,
                ) {
                    match action {
                        ViewCubeAction::Top => self.camera.set_preset(ViewPreset::Top),
                        ViewCubeAction::Bottom => self.camera.set_preset(ViewPreset::Bottom),
                        ViewCubeAction::Front => self.camera.set_preset(ViewPreset::Front),
                        ViewCubeAction::Back => self.camera.set_preset(ViewPreset::Back),
                        ViewCubeAction::Right => self.camera.set_preset(ViewPreset::Right),
                        ViewCubeAction::Left => self.camera.set_preset(ViewPreset::Left),
                        ViewCubeAction::Isometric => {
                            self.camera.set_preset(ViewPreset::Isometric);
                        }
                    }
                }
            });


        if self.section_enabled {
            egui::Area::new(egui::Id::new("ducad-hud-section-banner"))
                .fixed_pos(egui::pos2(screen_center_x - 140.0, 94.0))
                .order(egui::Order::Foreground)
                .show(&ctx, |ui| {
                    if let Some(hud_ev) = CanvasHud::show_section_view_banner(ui) {
                        if hud_ev == CanvasHudEvent::TurnOffSectionView {
                            self.section_enabled = false;
                        }
                    }
                });
        }

        let selected_entity_data = if self.selected.len() == 1 {
            let &id = self.selected.iter().next().unwrap();
            let id_raw = id.data().as_ffi();
            let entity_opt = self.sketch().entities.get(id).cloned();
            match entity_opt {
                Some(Entity::Line { start, end }) => {
                    let length = (end - start).length();
                    let angle_deg = (end - start).y.atan2((end - start).x).to_degrees();
                    if self.last_inspected_entity_id != Some(id_raw) {
                        self.prop_input_p1_x = format!("{:.2}", start.x);
                        self.prop_input_p1_y = format!("{:.2}", start.y);
                        self.prop_input_p2_x = format!("{:.2}", end.x);
                        self.prop_input_p2_y = format!("{:.2}", end.y);
                        self.prop_input_val_1 = format!("{:.2}", length);
                        self.prop_input_val_2 = format!("{:.1}", angle_deg);
                        self.last_inspected_entity_id = Some(id_raw);
                    }
                    SelectedEntityData::Line {
                        id_raw,
                        start_x: start.x,
                        start_y: start.y,
                        end_x: end.x,
                        end_y: end.y,
                        length,
                        angle_deg,
                    }
                }
                Some(Entity::Circle { center, radius }) => {
                    let diameter = radius * 2.0;
                    if self.last_inspected_entity_id != Some(id_raw) {
                        self.prop_input_p1_x = format!("{:.2}", center.x);
                        self.prop_input_p1_y = format!("{:.2}", center.y);
                        self.prop_input_val_1 = format!("{:.2}", radius);
                        self.prop_input_val_2 = format!("{:.2}", diameter);
                        self.prop_input_val_3 = format!("{:.2}", diameter);
                        self.last_inspected_entity_id = Some(id_raw);
                    }
                    SelectedEntityData::Circle {
                        id_raw,
                        center_x: center.x,
                        center_y: center.y,
                        radius,
                        diameter,
                    }
                }
                Some(Entity::Arc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                }) => {
                    let start_deg = start_angle.to_degrees();
                    let end_deg = end_angle.to_degrees();
                    if self.last_inspected_entity_id != Some(id_raw) {
                        self.prop_input_p1_x = format!("{:.2}", center.x);
                        self.prop_input_p1_y = format!("{:.2}", center.y);
                        self.prop_input_val_1 = format!("{:.2}", radius);
                        self.prop_input_val_2 = format!("{:.1}", start_deg);
                        self.prop_input_p2_x = format!("{:.1}", end_deg);
                        self.last_inspected_entity_id = Some(id_raw);
                    }
                    SelectedEntityData::Arc {
                        id_raw,
                        center_x: center.x,
                        center_y: center.y,
                        radius,
                        start_angle_deg: start_deg,
                        end_angle_deg: end_deg,
                    }
                }
                Some(Entity::Ellipse {
                    center,
                    radius_x,
                    radius_y,
                }) => {
                    if self.last_inspected_entity_id != Some(id_raw) {
                        self.prop_input_p1_x = format!("{:.2}", center.x);
                        self.prop_input_p1_y = format!("{:.2}", center.y);
                        self.prop_input_val_1 = format!("{:.2}", radius_x);
                        self.prop_input_val_2 = format!("{:.2}", radius_y);
                        self.last_inspected_entity_id = Some(id_raw);
                    }
                    SelectedEntityData::Ellipse {
                        id_raw,
                        center_x: center.x,
                        center_y: center.y,
                        radius_x,
                        radius_y,
                    }
                }
                None => {
                    self.last_inspected_entity_id = None;
                    SelectedEntityData::None
                }
            }
        } else if self.selected.len() > 1 {
            // Klik satu sisi rectangle di kanvas menyeleksi ke-4 Line pembentuknya
            // sekaligus (lihat input/sketch.rs region_hit) — deteksi itu di sini
            // supaya panel kanan tampilkan card Rectangle (P/L + anchor) alih-alih
            // MultipleEntities generik.
            if let Some(rect) = detect_rectangle(self.sketch(), &self.selected) {
                let entity_ids = [
                    rect.entity_ids[0].data().as_ffi(),
                    rect.entity_ids[1].data().as_ffi(),
                    rect.entity_ids[2].data().as_ffi(),
                    rect.entity_ids[3].data().as_ffi(),
                ];
                // Kunci "sudah pernah diinspeksi" pakai id sisi pertama — cukup
                // untuk deteksi ganti-seleksi tanpa menimpa input P/L yg lagi diketik.
                if self.last_inspected_entity_id != Some(entity_ids[0]) {
                    self.prop_input_rect_p = format!("{:.2}", rect.length_p);
                    self.prop_input_rect_l = format!("{:.2}", rect.length_l);
                    self.last_inspected_entity_id = Some(entity_ids[0]);
                }
                SelectedEntityData::Rectangle {
                    entity_ids,
                    length_p: rect.length_p,
                    length_l: rect.length_l,
                }
            } else {
                self.last_inspected_entity_id = None;
                SelectedEntityData::MultipleEntities {
                    count: self.selected.len(),
                }
            }
        } else {
            self.last_inspected_entity_id = None;
            SelectedEntityData::None
        };

        let selected_body_data = if self.selected_bodies.len() == 1 {
            let &bid = self.selected_bodies.iter().next().unwrap();
            let body_name = self
                .model
                .doc
                .bodies
                .get(bid)
                .map(|b| b.name.clone())
                .unwrap_or_else(|| "Solid Body".to_string());
            if let Some(geo) = self.model.geometry.get(bid) {
                let v_count = geo.mesh.positions.len();
                let t_count = geo.mesh.indices.len() / 3;
                let mut min_p = [f32::INFINITY; 3];
                let mut max_p = [f32::NEG_INFINITY; 3];
                for pos in &geo.mesh.positions {
                    for i in 0..3 {
                        min_p[i] = min_p[i].min(pos[i]);
                        max_p[i] = max_p[i].max(pos[i]);
                    }
                }
                let bbox_size = [
                    (max_p[0] - min_p[0]).abs().max(0.0),
                    (max_p[1] - min_p[1]).abs().max(0.0),
                    (max_p[2] - min_p[2]).abs().max(0.0),
                ];
                Some(SelectedBodyData {
                    id_raw: bid.data().as_ffi(),
                    name: body_name,
                    vertices_count: v_count,
                    triangles_count: t_count,
                    bbox_size,
                })
            } else {
                None
            }
        } else {
            None
        };

        if !show_right_sidebar {
            egui::Area::new(egui::Id::new("ducad-inspector-toggle-area"))
                .fixed_pos(egui::pos2(
                    screen_rect.max.x - topbar_margin_right,
                    screen_rect.center().y,
                ))
                .pivot(egui::Align2::RIGHT_CENTER)
                .order(egui::Order::Foreground)
                .show(&ctx, |ui| {
                    let btn = egui::Button::new(
                        egui::RichText::new("⚙ Properties")
                            .size(12.0)
                            .color(egui::Color32::from_rgb(220, 230, 242)),
                    )
                    .fill(egui::Color32::from_rgba_premultiplied(22, 27, 34, 235))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(48, 54, 61)))
                    .corner_radius(egui::CornerRadius::same(8));
                    if ui.add(btn).clicked() {
                        self.feature_inspector_open = true;
                        self.auto_hide_properties = false;
                    }
                });
        }

        let inspector_top_bound = viewcube_y + 52.0;
        let inspector_bottom_margin = 12.0;
        let inspector_max_h =
            (screen_rect.max.y - inspector_top_bound - inspector_bottom_margin).max(120.0);
        if show_right_sidebar {
            let inspector_sig: InspectorContentSig = (
                std::mem::discriminant(&selected_entity_data),
                selected_body_data.is_some(),
                self.selected_bodies.len(),
                self.selected_edges.len(),
                self.selected_faces.len(),
                self.active_face.is_some(),
                self.pending_loft_bottom.is_some(),
                self.section_enabled,
                self.picking_mode,
                self.measurements.len(),
                measure_tool_active,
            );
            let inspector_force_resize = self.inspector_content_sig != Some(inspector_sig);
            self.inspector_content_sig = Some(inspector_sig);

            let mut inspector_state = FeatureInspectorState {
                auto_hide_enabled: self.auto_hide_properties,
                selected_entity: selected_entity_data,
                selected_body: selected_body_data,
                selected_bodies_count: self.selected_bodies.len(),
                selected_edges_count: self.selected_edges.len(),
                selected_faces_count: self.selected_faces.len(),
                total_entities_count: self.sketch().entities.len(),
                total_bodies_count: self.model.doc.bodies.len(),

                entity_p1_x: self.prop_input_p1_x.clone(),
                entity_p1_y: self.prop_input_p1_y.clone(),
                entity_p2_x: self.prop_input_p2_x.clone(),
                entity_p2_y: self.prop_input_p2_y.clone(),
                entity_val_1: self.prop_input_val_1.clone(),
                entity_val_2: self.prop_input_val_2.clone(),
                entity_val_3: self.prop_input_val_3.clone(),
                rect_length_p_input: self.prop_input_rect_p.clone(),
                rect_length_l_input: self.prop_input_rect_l.clone(),
                rect_anchor: self.rect_anchor,

                extrude_input: self.extrude_distance_input.clone(),
                active_face_selected: self.active_face.is_some(),
                face_extrude_input: self.face_extrude_distance_input.clone(),
                revolve_angle_input: self.revolve_dialog.angle_input.clone(),
                revolve_axis_preset: match self.revolve_dialog.axis_preset {
                    ducad_ui::RevolveAxisPreset::YAxisOrigin => 0,
                    ducad_ui::RevolveAxisPreset::XAxisOrigin => 1,
                    ducad_ui::RevolveAxisPreset::BBoxLeft => 2,
                    ducad_ui::RevolveAxisPreset::BBoxBottom => 3,
                    _ => 4,
                },
                revolve_reverse: self.revolve_reverse,
                loft_height_input: self.loft_height_input.clone(),
                loft_bottom_staged: self.pending_loft_bottom.is_some(),
                fillet_input: self.fillet_radius_input.clone(),
                chamfer_input: self.chamfer_distance_input.clone(),
                shell_input: self.shell_thickness_input.clone(),
                picking_mode: match self.picking_mode {
                    PickMode::None => InspectorPickMode::None,
                    PickMode::Edge => InspectorPickMode::Edge,
                    PickMode::Face => InspectorPickMode::Face,
                },
                can_undo_model: self.model_undo.can_undo(),
                can_redo_model: self.model_undo.can_redo(),
                status_message: self.model_status.clone(),
                section_enabled: self.section_enabled,
                section_axis: match self.section_axis {
                    SectionAxis::X => 0,
                    SectionAxis::Y => 1,
                    SectionAxis::Z => 2,
                },
                section_offset: self.section_offset,
                section_invert: self.section_invert,

                measurements: self.measurements.iter().map(|m| m.label()).collect(),
                measurement_tool_active: measure_tool_active,
                show_all_dimensions: self.show_all_dimensions,

                max_panel_height: inspector_max_h,
            };

            egui::Area::new(egui::Id::new("ducad-inspector-area"))
                .fixed_pos(egui::pos2(
                    screen_rect.max.x - topbar_margin_right,
                    screen_rect.center().y,
                ))
                .pivot(egui::Align2::RIGHT_CENTER)
                .constrain_to(screen_rect)
                .default_size(egui::vec2(264.0, inspector_max_h))
                .sizing_pass(inspector_force_resize)
                .order(egui::Order::Foreground)
                .show(&ctx, |ui| {
                    let insp_ev = FeatureInspector::show(ui, &mut inspector_state);

                    // Sinkron balik SEMUA buffer teks tiap frame, bukan cuma pas ada
                    // event (klik tombol/checkbox). `inspector_state` dibangun ULANG
                    // dari `self.*` tiap frame di atas; `TextEdit` polos (P1/P2/radius/
                    // P-L rectangle/ukuran body dst) tidak pernah mengembalikan
                    // `InspectorEvent` sendiri — kalau sync-back cuma jalan di dalam
                    // `if let Some(insp_ev)`, karakter yg baru diketik hilang lagi di
                    // frame berikutnya (balik ke nilai lama) karena tidak sempat
                    // tersimpan ke `self`. Root cause laporan "kotak properties tidak
                    // bisa diubah, balik ke nilai awal lagi pas diketik".
                    self.prop_input_p1_x = inspector_state.entity_p1_x;
                    self.prop_input_p1_y = inspector_state.entity_p1_y;
                    self.prop_input_p2_x = inspector_state.entity_p2_x;
                    self.prop_input_p2_y = inspector_state.entity_p2_y;
                    self.prop_input_val_1 = inspector_state.entity_val_1;
                    self.prop_input_val_2 = inspector_state.entity_val_2;
                    self.prop_input_val_3 = inspector_state.entity_val_3;
                    self.prop_input_rect_p = inspector_state.rect_length_p_input;
                    self.prop_input_rect_l = inspector_state.rect_length_l_input;
                    self.rect_anchor = inspector_state.rect_anchor;
                    self.face_extrude_distance_input = inspector_state.face_extrude_input;

                    if let Some(insp_ev) = insp_ev {
                        match insp_ev {
                            InspectorEvent::CloseInspector => {
                                self.feature_inspector_open = false;
                            }
                            InspectorEvent::ToggleAutoHide => {
                                self.auto_hide_properties = !self.auto_hide_properties;
                            }
                            InspectorEvent::ToggleShowAllDimensions => {
                                self.show_all_dimensions = !self.show_all_dimensions;
                            }
                            InspectorEvent::UpdateEntityLine {
                                id_raw,
                                start_x,
                                start_y,
                                end_x,
                                end_y,
                            } => {
                                if let Some(&id) = self
                                    .selected
                                    .iter()
                                    .find(|i| i.data().as_ffi() == id_raw)
                                {
                                    let new_entity = Entity::Line {
                                        start: DVec2::new(start_x, start_y),
                                        end: DVec2::new(end_x, end_y),
                                    };
                                    self.execute_sketch_command(Box::new(
                                        UpdateEntity::new("Ubah Garis", id, new_entity),
                                    ));
                                }
                            }
                            InspectorEvent::UpdateEntityCircle {
                                id_raw,
                                center_x,
                                center_y,
                                radius,
                            } => {
                                if let Some(&id) = self
                                    .selected
                                    .iter()
                                    .find(|i| i.data().as_ffi() == id_raw)
                                {
                                    let new_entity = Entity::Circle {
                                        center: DVec2::new(center_x, center_y),
                                        radius,
                                    };
                                    self.execute_sketch_command(Box::new(
                                        UpdateEntity::new("Ubah Lingkaran", id, new_entity),
                                    ));
                                }
                            }
                            InspectorEvent::UpdateEntityArc {
                                id_raw,
                                center_x,
                                center_y,
                                radius,
                                start_angle_deg,
                                end_angle_deg,
                            } => {
                                if let Some(&id) = self
                                    .selected
                                    .iter()
                                    .find(|i| i.data().as_ffi() == id_raw)
                                {
                                    let new_entity = Entity::Arc {
                                        center: DVec2::new(center_x, center_y),
                                        radius,
                                        start_angle: start_angle_deg.to_radians(),
                                        end_angle: end_angle_deg.to_radians(),
                                    };
                                    self.execute_sketch_command(Box::new(
                                        UpdateEntity::new("Ubah Busur", id, new_entity),
                                    ));
                                }
                            }
                            InspectorEvent::UpdateEntityEllipse {
                                id_raw,
                                center_x,
                                center_y,
                                radius_x,
                                radius_y,
                            } => {
                                if let Some(&id) = self
                                    .selected
                                    .iter()
                                    .find(|i| i.data().as_ffi() == id_raw)
                                {
                                    let new_entity = Entity::Ellipse {
                                        center: DVec2::new(center_x, center_y),
                                        radius_x,
                                        radius_y,
                                    };
                                    self.execute_sketch_command(Box::new(
                                        UpdateEntity::new("Ubah Elips", id, new_entity),
                                    ));
                                }
                            }
                            InspectorEvent::UpdateEntityRectangle {
                                entity_ids: _,
                                length_p,
                                length_l,
                                anchor,
                            } => {
                                if let Some(rect) = detect_rectangle(self.sketch(), &self.selected)
                                {
                                    let anchor = match anchor {
                                        InspectorRectAnchor::Center => RectAnchor::Center,
                                        InspectorRectAnchor::Corner0 => RectAnchor::Corner0,
                                        InspectorRectAnchor::Corner1 => RectAnchor::Corner1,
                                        InspectorRectAnchor::Corner2 => RectAnchor::Corner2,
                                        InspectorRectAnchor::Corner3 => RectAnchor::Corner3,
                                    };
                                    let new_lines = rect.resized_lines(length_p, length_l, anchor);
                                    self.execute_sketch_command(Box::new(ResizeRectangle::new(
                                        "Ubah Rectangle",
                                        new_lines,
                                    )));
                                }
                            }
                            InspectorEvent::ApplyConstraint(act) => {
                                let ids: Vec<EntityId> =
                                    self.selected.iter().copied().collect();
                                match act {
                                    InspectorConstraintAction::Horizontal => {
                                        if let [id] = ids.as_slice() {
                                            self.apply_constraint(Constraint::Horizontal {
                                                line: *id,
                                            });
                                        }
                                    }
                                    InspectorConstraintAction::Vertical => {
                                        if let [id] = ids.as_slice() {
                                            self.apply_constraint(Constraint::Vertical {
                                                line: *id,
                                            });
                                        }
                                    }
                                    InspectorConstraintAction::Parallel => {
                                        if let [a, b] = ids.as_slice() {
                                            self.apply_constraint(Constraint::Parallel {
                                                a: *a,
                                                b: *b,
                                            });
                                        }
                                    }
                                    InspectorConstraintAction::Perpendicular => {
                                        if let [a, b] = ids.as_slice() {
                                            self.apply_constraint(Constraint::Perpendicular {
                                                a: *a,
                                                b: *b,
                                            });
                                        }
                                    }
                                    InspectorConstraintAction::EqualLength => {
                                        if let [a, b] = ids.as_slice() {
                                            self.apply_constraint(Constraint::EqualLength {
                                                a: *a,
                                                b: *b,
                                            });
                                        }
                                    }
                                    InspectorConstraintAction::EqualRadius => {
                                        if let [a, b] = ids.as_slice() {
                                            self.apply_constraint(Constraint::EqualRadius {
                                                a: *a,
                                                b: *b,
                                            });
                                        }
                                    }
                                    InspectorConstraintAction::Tangent => {
                                        if let [a, b] = ids.as_slice() {
                                            self.apply_constraint(Constraint::Tangent {
                                                a: *a,
                                                b: *b,
                                            });
                                        }
                                    }
                                    InspectorConstraintAction::Coincident => {
                                        self.set_tool(ToolKind::CoincidentPick);
                                    }
                                    InspectorConstraintAction::Fixed => {
                                        self.set_tool(ToolKind::FixedPick);
                                    }
                                    InspectorConstraintAction::Symmetric => {
                                        self.set_tool(ToolKind::SymmetricPick);
                                    }
                                }
                            }
                            InspectorEvent::UndoModel => {
                                self.model_undo.undo(&mut self.model);
                                self.selected_bodies.clear();
                            }
                            InspectorEvent::RedoModel => {
                                self.model_undo.redo(&mut self.model);
                                self.selected_bodies.clear();
                            }
                            InspectorEvent::ApplyExtrude { distance } => {
                                self.extrude_distance_input = distance.to_string();
                                self.extrude_selected();
                            }
                            InspectorEvent::ApplyFaceExtrude { distance } => {
                                self.face_extrude_distance_input = distance.to_string();
                                self.extrude_active_face(distance);
                            }
                            InspectorEvent::SketchOnFace => {
                                self.sketch_on_active_face();
                            }
                            InspectorEvent::ApplyRevolve => {
                                self.feature_inspector_open = true;
                                self.set_tool(ToolKind::Revolve);
                            }
                            InspectorEvent::ApplyRevolvePreset { preset_idx, angle_deg } => {
                                let preset = match preset_idx {
                                    0 => ducad_ui::RevolveAxisPreset::YAxisOrigin,
                                    1 => ducad_ui::RevolveAxisPreset::XAxisOrigin,
                                    2 => ducad_ui::RevolveAxisPreset::BBoxLeft,
                                    3 => ducad_ui::RevolveAxisPreset::BBoxBottom,
                                    _ => ducad_ui::RevolveAxisPreset::CustomTwoPoints,
                                };
                                self.revolve_selected_with_preset(preset, angle_deg);
                            }
                            InspectorEvent::StartManualRevolve => {
                                self.set_tool(ToolKind::Revolve);
                            }
                            InspectorEvent::StageLoftBottom => {
                                match crate::model::build_profile_from_selection(
                                    self.sketch(),
                                    &self.selected,
                                ) {
                                    Ok(profile) => {
                                        self.pending_loft_bottom = Some(profile);
                                        self.model_status = None;
                                    }
                                    Err(msg) => self.model_status = Some(msg),
                                }
                            }
                            InspectorEvent::ApplyLoft { height } => {
                                self.loft_height_input = height.to_string();
                                self.loft_selected();
                            }
                            InspectorEvent::ApplyBoolean(kind) => {
                                let (b_kind, label) = match kind {
                                    InspectorBooleanKind::Union => {
                                        (BooleanKind::Union, "Union")
                                    }
                                    InspectorBooleanKind::Subtract => {
                                        (BooleanKind::Subtract, "Subtract")
                                    }
                                    InspectorBooleanKind::Intersect => {
                                        (BooleanKind::Intersect, "Intersect")
                                    }
                                };
                                self.boolean_selected(b_kind, label, label);
                            }
                            InspectorEvent::ToggleEdgePicking => {
                                self.picking_mode = if self.picking_mode == PickMode::Edge {
                                    PickMode::None
                                } else {
                                    PickMode::Edge
                                };
                            }
                            InspectorEvent::ResetEdgePicking => {
                                self.selected_edges.clear();
                            }
                            InspectorEvent::ApplyFillet { radius } => {
                                self.fillet_radius_input = radius.to_string();
                                self.fillet_selected_body();
                            }
                            InspectorEvent::ApplyChamfer { distance } => {
                                self.chamfer_distance_input = distance.to_string();
                                self.chamfer_selected_body();
                            }
                            InspectorEvent::ToggleFacePicking => {
                                self.picking_mode = if self.picking_mode == PickMode::Face {
                                    PickMode::None
                                } else {
                                    PickMode::Face
                                };
                            }
                            InspectorEvent::ApplyShell { thickness } => {
                                self.shell_thickness_input = thickness.to_string();
                                self.shell_selected_body();
                            }
                            InspectorEvent::DeleteSelectedBodies => {
                                self.delete_selected_bodies();
                            }
                            InspectorEvent::SectionViewChanged => {
                                self.section_enabled = inspector_state.section_enabled;
                                self.section_axis = match inspector_state.section_axis {
                                    0 => SectionAxis::X,
                                    1 => SectionAxis::Y,
                                    _ => SectionAxis::Z,
                                };
                                self.section_offset = inspector_state.section_offset;
                                self.section_invert = inspector_state.section_invert;
                            }
                            InspectorEvent::RemoveMeasurement(i) => {
                                if i < self.measurements.len() {
                                    self.measurements.remove(i);
                                }
                            }
                            InspectorEvent::ClearMeasurements => {
                                self.measurements.clear();
                            }
                        }
                    }
                    if let Ok(ang) = inspector_state.revolve_angle_input.trim().parse::<f64>() {
                        self.revolve_angle_setting = ang;
                    }
                    self.revolve_reverse = inspector_state.revolve_reverse;
                    self.revolve_dialog.angle_input = inspector_state.revolve_angle_input;
                    self.revolve_dialog.axis_preset = match inspector_state.revolve_axis_preset {
                        0 => ducad_ui::RevolveAxisPreset::YAxisOrigin,
                        1 => ducad_ui::RevolveAxisPreset::XAxisOrigin,
                        2 => ducad_ui::RevolveAxisPreset::BBoxLeft,
                        3 => ducad_ui::RevolveAxisPreset::BBoxBottom,
                        _ => ducad_ui::RevolveAxisPreset::CustomTwoPoints,
                    };
                });
        }

        let bottom_center = egui::pos2(screen_center_x, screen_rect.max.y);

        // Shapr3D-Style Floating Contextual Action Bar
        let has_sketch_sel = !self.selected.is_empty();
        let has_face_sel = self.active_face.is_some();
        let has_body_sel = !self.selected_bodies.is_empty();

        if has_sketch_sel || has_face_sel || has_body_sel {
            egui::Area::new(egui::Id::new("ducad-context-action-bar-area"))
                .fixed_pos(egui::pos2(screen_center_x, screen_rect.max.y - 56.0))
                .pivot(egui::Align2::CENTER_BOTTOM)
                .order(egui::Order::Foreground)
                .show(&ctx, |ui| {
                    if has_sketch_sel {
                        let has_closed = self.selected_closed_region_centroid().is_some()
                            || crate::model::build_profile_from_selection(self.sketch(), &self.selected).is_ok();
                        if let Some(act) = ContextActionBar::show_sketch_selection(ui, self.selected.len(), has_closed) {
                            match act {
                                ContextAction::Extrude => self.extrude_selected(),
                                ContextAction::Offset => self.set_tool(ToolKind::Offset),
                                ContextAction::Mirror => self.set_tool(ToolKind::Mirror),
                                ContextAction::Trim => self.set_tool(ToolKind::Trim),
                                ContextAction::Revolve => self.open_revolve_dialog(),
                                ContextAction::Delete => {
                                    if !self.selected.is_empty() {
                                        let to_delete: Vec<EntityId> = self.selected.iter().copied().collect();
                                        self.execute_sketch_command(Box::new(DeleteEntities::new(to_delete)));
                                        self.selected.clear();
                                    }
                                }
                                ContextAction::ClearSelection => self.selected.clear(),
                                _ => {}
                            }
                        }
                    } else if has_face_sel {
                        if let Some(act) = ContextActionBar::show_face_selection(ui) {
                            match act {
                                ContextAction::Extrude => {
                                    self.extruding_face_from_gizmo = true;
                                    if self.face_gizmo_distance == 0.0 {
                                        self.face_gizmo_distance = 15.0;
                                    }
                                    self.auto_enter_3d_mode_on_extrude_drag();
                                }
                                ContextAction::SketchOnFace => {
                                    self.sketch_on_active_face();
                                }
                                ContextAction::Revolve => {
                                    self.open_revolve_dialog();
                                }
                                ContextAction::ClearSelection => {
                                    self.active_face = None;
                                }
                                _ => {}
                            }
                        }
                    } else if has_body_sel {
                        if let Some(act) = ContextActionBar::show_body_selection(ui, self.selected_bodies.len()) {
                            match act {
                                ContextAction::Delete => {
                                    self.delete_selected_bodies();
                                }
                                ContextAction::ClearSelection => {
                                    self.selected_bodies.clear();
                                }
                                _ => {}
                            }
                        }
                    }
                });
        }

        let sel_summary = if !self.selected.is_empty() {
            format!("{} entitas terpilih", self.selected.len())
        } else if !self.selected_bodies.is_empty() {
            format!("{} body terpilih", self.selected_bodies.len())
        } else {
            self.status_text()
        };
        let m_summary = self.measurements.last().map(|m| m.label());
        let show_normal_to_sketch = self.tool != ToolKind::Select;

        egui::Area::new(egui::Id::new("ducad-hud-bottom-status-area"))
            .pivot(egui::Align2::CENTER_BOTTOM)
            .fixed_pos(bottom_center - egui::vec2(0.0, 18.0))
            .order(egui::Order::Foreground)
            .show(&ctx, |ui| {
                if let Some(ev) = CanvasHud::show_bottom_status_pill(
                    ui,
                    &sel_summary,
                    m_summary.as_deref(),
                    show_normal_to_sketch,
                ) {
                    match ev {
                        CanvasHudEvent::OrientNormalToSketch => {
                            self.camera.orient_to_plane(&self.active_plane);
                        }
                        CanvasHudEvent::OpenMeasurements => {
                            self.set_tool(ToolKind::Measure);
                        }
                        CanvasHudEvent::TurnOffSectionView => {}
                    }
                }
            });

        let palette_actions = self.palette_actions();
        let palette_entries: Vec<(&str, &str)> = palette_actions
            .iter()
            .map(|(label, hint, _)| (label.as_str(), hint.as_str()))
            .collect();
        if let Some(idx) = self.palette.show(&ctx, &palette_entries) {
            let action = palette_actions[idx].2;
            self.run_palette_action(&ctx, action);
        }

        // Render Alert Modal Peringatan jika ada operasi yang gagal
        ducad_ui::AlertModal::show(&ctx, &mut self.alert_modal);
    }
}

impl DuCADApp {
    /// Buka konfigurasi Revolve di panel properti kanan dan aktifkan tool Revolve.
    pub fn open_revolve_dialog(&mut self) {
        self.feature_inspector_open = true;
        self.set_tool(ToolKind::Revolve);
    }

    /// Eksekusi revolve setelah sumbu 2 titik di-stage dan sudut disesuaikan.
    pub fn commit_staged_revolve(&mut self) {
        if let Some((axis_origin, axis_end)) = self.revolve_staged_axis.take() {
            let raw_dir = axis_end - axis_origin;
            let axis_dir = if self.revolve_reverse { -raw_dir } else { raw_dir };
            let angle_opt = if (self.revolve_angle_setting - 360.0).abs() < 1e-4 {
                None
            } else {
                Some(self.revolve_angle_setting)
            };
            self.revolve_selected(
                (axis_origin.x, axis_origin.y),
                (axis_dir.x, axis_dir.y),
                angle_opt,
            );
            self.set_tool(ToolKind::Select);
        }
    }

    /// Batalkan sumbu revolve yang sedang di-stage.
    pub fn cancel_staged_revolve(&mut self) {
        self.revolve_staged_axis = None;
        self.set_tool(ToolKind::Select);
    }
}


