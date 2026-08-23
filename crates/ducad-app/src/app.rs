use std::collections::HashSet;
use std::path::PathBuf;

use ducad_core::{BodyId, LengthUnit};
use ducad_kernel::{FaceHit, PickRay};
use ducad_render::{OrbitCamera, PlaneKind, SceneRenderer, SketchPlane, ViewPreset};
use ducad_sketch::constraint::Constraint;
use ducad_sketch::{
    detect_rectangle, DeleteEntities, Entity, EntityId, RectAnchor, RenameEntities,
    ResizeRectangle, Sketch, SnapHit, UpdateEntity,
};
use ducad_ui::{
    ActivityItemInfo, ActivityKindUi, BodyItemInfo, CanvasHud, CanvasHudEvent, CommandPalette,
    ContextAction, ContextActionBar, Entity2dItemInfo, HistoryDrawer, HistoryDrawerEvent,
    HistoryPopup, HistoryPopupState,
    InspectorConstraintAction, InspectorRectAnchor, ItemsDrawer, ItemsDrawerEvent, LeftToolbar,
    RadialMenu, RenamePopupEvent,
    ThemeMode, ToolPopupEvent, ToolbarEvent, TopBar, TopBarEvent,
    TopBarFileOp, TopBarState, ViewCube, ViewCubeAction,
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
    pub boolean_op: ducad_ui::BooleanOpKind,

    pub pending_loft_bottom: Option<ducad_kernel::Profile>,
    pub loft_height_input: String,
    pub loft_alignment_dismissed: bool,
    pub loft_is_flipped: bool,
    pub loft_staged_body_id: Option<BodyId>,
    pub selection_box: Option<(glam::DVec2, glam::DVec2)>,

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
    pub history_drawer: HistoryDrawer,
    pub viewcube: ViewCube,
    pub feature_inspector_open: bool,
    pub auto_hide_properties: bool,
    pub items_drawer_open: bool,
    pub history_drawer_open: bool,
    pub history_db: crate::history_db::HistoryDb,
    pub activity_cache: Vec<ActivityItemInfo>,
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

    // Rename popup state
    pub rename_popup_open: bool,
    pub rename_input: String,
    pub rename_target: RenameTarget,
}

/// Target objek yang sedang di-rename.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameTarget {
    None,
    /// Rename/group semua entitas 2D yang saat ini terpilih.
    Sketch2d,
    /// Rename body 3D tertentu.
    Body3d(BodyId),
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

        let mut history_db = crate::history_db::HistoryDb::new();
        history_db.clear(); // Selalu mulai dengan riwayat kosong saat aplikasi dibuka
        let activity_cache = history_db.load_activities();

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
            boolean_op: ducad_ui::BooleanOpKind::Union,

            pending_loft_bottom: None,
            loft_height_input: "20.0".to_string(),
            loft_alignment_dismissed: false,
            loft_is_flipped: false,
            loft_staged_body_id: None,
            selection_box: None,
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
            history_drawer: HistoryDrawer::default(),
            viewcube: ViewCube::default(),
            feature_inspector_open: true,
            auto_hide_properties: true,
            items_drawer_open: false,
            history_drawer_open: false,
            history_db,
            activity_cache,
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

            rename_popup_open: false,
            rename_input: String::new(),
            rename_target: RenameTarget::None,
        }
    }

    /// Catat aktivitas baru ke SQLite bersama snapshot dokumen penuh, lalu perbarui cache riwayat.
    pub fn record_activity(&mut self, kind: ActivityKindUi, action: &str, details: &str) {
        let body_refs = self.native_body_refs();
        let snapshot_json = ducad_io::native::serialize_to_json(&self.sketches, &body_refs).ok();

        self.history_db.log_activity(kind, action, details, snapshot_json.as_deref());
        self.activity_cache = self.history_db.load_activities();
    }

    /// Pulihkan dokumen ke snapshot JSON tertentu (Time-Travel).
    pub fn restore_snapshot_from_json(&mut self, json: &str) -> anyhow::Result<()> {
        let loaded = ducad_io::native::deserialize_from_json(json)?;

        self.sketches = [loaded.sketch, loaded.front_sketch, loaded.right_sketch];
        self.undos = [
            ducad_sketch::UndoStack::default(),
            ducad_sketch::UndoStack::default(),
            ducad_sketch::UndoStack::default(),
        ];
        self.selected.clear();
        self.hovered = None;
        self.pending_points.clear();
        self.pending_point_refs.clear();
        self.offset_source = None;
        self.line_chain_start = None;
        self.line_chain_segments = 0;

        let mut new_model = crate::model::ModelDoc::default();
        for nb in loaded.bodies {
            let geo = crate::model::BodyGeometry::from_shape(nb.shape);
            let id = new_model.doc.add_body(&nb.name);
            new_model.geometry.insert(id, geo);
            if let Some(meta) = new_model.doc.bodies.get_mut(id) {
                meta.visible = nb.visible;
            }
        }
        self.model = new_model;
        self.model_undo = ducad_core::UndoStack::default();
        self.selected_bodies.clear();
        self.selected_faces.clear();
        self.picking_mode = crate::types::PickMode::None;
        self.active_face = None;
        self.round_history.clear();
        self.loft_staged_body_id = None;
        self.set_tool(ToolKind::Select);

        Ok(())
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
            is_measure_active: self.show_all_dimensions,
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
                            self.show_all_dimensions = !self.show_all_dimensions;
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
                        ToolbarEvent::SelectTool(ducad_ui::ToolbarTool::SectionView) => {
                            self.section_enabled = !self.section_enabled;
                        }
                        ToolbarEvent::SelectTool(t) => {
                            let kind = ToolKind::from_toolbar_tool(t);
                            match kind {
                                ToolKind::Shell => {
                                    self.picking_mode = PickMode::Face;
                                }
                                ToolKind::Select => {
                                    self.picking_mode = PickMode::None;
                                }
                                _ => {}
                            }
                            self.set_tool(kind);
                        }
                    }
                }
            });

        let entities_2d: Vec<Entity2dItemInfo> = self
            .sketch()
            .entities
            .iter()
            .map(|(id, entity)| {
                let (name, icon_str) = match entity {
                    Entity::Line { start, end } => {
                        let len = start.distance(*end);
                        (
                            format!("Garis ({:.1} mm)", len),
                            egui_material_icons::icons::ICON_HORIZONTAL_RULE.codepoint,
                        )
                    }
                    Entity::Circle { radius, .. } => {
                        (
                            format!("Lingkaran (R: {:.1} mm)", radius),
                            egui_material_icons::icons::ICON_CIRCLE.codepoint,
                        )
                    }
                    Entity::Arc { radius, .. } => {
                        (
                            format!("Busur (R: {:.1} mm)", radius),
                            egui_material_icons::icons::ICON_ARCHITECTURE.codepoint,
                        )
                    }
                    Entity::Ellipse {
                        radius_x, radius_y, ..
                    } => (
                        format!("Elips ({:.1}x{:.1} mm)", radius_x, radius_y),
                        egui_material_icons::icons::ICON_HOME_MINI.codepoint,
                    ),
                };
                Entity2dItemInfo {
                    id_raw: id.data().as_ffi(),
                    name,
                    icon: icon_str,
                    selected: self.selected.contains(&id),
                    group_name: self.sketch().entity_names.get(&id).cloned(),
                }
            })
            .collect();

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

        let both_open = self.items_drawer_open && self.history_drawer_open;
        let screen_avail_h = (screen_rect.height() - 140.0).max(300.0);
        let max_drawer_h = if both_open {
            ((screen_rect.height() - 180.0) * 0.75).clamp(200.0, screen_avail_h)
        } else {
            screen_avail_h
        };

        // 1. Folder / Items Drawer (Pojok Kanan Bawah, tepat di atas tombol)
        let mut folder_top_y = None;
        let folder_bottom_y = screen_rect.max.y - 62.0;
        if self.items_drawer_open {
            let folder_pos = egui::pos2(screen_rect.max.x - 16.0, folder_bottom_y);

            let area_resp = egui::Area::new(egui::Id::new("ducad-items-drawer-area"))
                .fixed_pos(folder_pos)
                .pivot(egui::Align2::RIGHT_BOTTOM)
                .order(egui::Order::Foreground)
                .show(&ctx, |ui| {
                    if let Some(ev) = self.items_drawer.show(ui, &entities_2d, &bodies, max_drawer_h, folder_bottom_y) {
                        match ev {
                            ItemsDrawerEvent::ToggleBodyVisibility(raw_id) => {
                                println!("[DUCAD APP] Event ToggleBodyVisibility diterima untuk raw_id: {}", raw_id);
                                let mut found = false;
                                for (id, b) in self.model.doc.bodies.iter_mut() {
                                    if id.data().as_ffi() == raw_id {
                                        found = true;
                                        let old_vis = b.visible;
                                        b.visible = !b.visible;
                                        println!("[DUCAD APP] Visibilitas body '{}' diubah: {} -> {}", b.name, old_vis, b.visible);
                                        self.model_status = Some(format!(
                                            "Body '{}' {}",
                                            b.name,
                                            if b.visible { "ditampilkan" } else { "disembunyikan" }
                                        ));
                                        if !b.visible {
                                            self.selected_bodies.remove(&id);
                                            if let Some((active_id, _, _)) = self.active_face {
                                                if active_id == id {
                                                    self.active_face = None;
                                                }
                                            }
                                            if let Some((active_id, _, _)) = self.active_vertex {
                                                if active_id == id {
                                                    self.active_vertex = None;
                                                }
                                            }
                                            if let Some((active_id, _, _)) = self.active_edge {
                                                if active_id == id {
                                                    self.active_edge = None;
                                                }
                                            }
                                        }
                                        ctx.request_repaint();
                                        break;
                                    }
                                }
                                if !found {
                                    println!("[DUCAD APP] Body dengan raw_id: {} tidak ditemukan di doc.bodies! (Total bodies: {})", raw_id, self.model.doc.bodies.len());
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
                            ItemsDrawerEvent::SelectEntity2d { id_raw, extend } => {
                                if let Some(id) = self.sketch().entities.keys().find(|i| i.data().as_ffi() == id_raw) {
                                    if !extend {
                                        self.selected.clear();
                                    }
                                    if !self.selected.remove(&id) {
                                        self.selected.insert(id);
                                    }
                                }
                            }
                            ItemsDrawerEvent::Close => {
                                self.items_drawer_open = false;
                            }
                            ItemsDrawerEvent::Open => {
                                self.items_drawer_open = true;
                            }
                            ItemsDrawerEvent::ToggleGroup(name) => {
                                let current = *self.items_drawer.expanded_groups.get(&name).unwrap_or(&true);
                                self.items_drawer.expanded_groups.insert(name, !current);
                            }
                        }
                    }
                });

            folder_top_y = Some(area_resp.response.rect.min.y);
        }

        // 2. History Drawer (Tersusun di atas tombol atau di atas Folder Drawer jika keduanya terbuka)
        if self.history_drawer_open {
            let hist_bottom_y = if self.items_drawer_open {
                folder_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
            } else {
                folder_bottom_y
            };
            let hist_pos = egui::pos2(screen_rect.max.x - 16.0, hist_bottom_y);

            egui::Area::new(egui::Id::new("ducad-history-drawer-area"))
                .fixed_pos(hist_pos)
                .pivot(egui::Align2::RIGHT_BOTTOM)
                .order(egui::Order::Foreground)
                .show(&ctx, |ui| {
                    if let Some(ev) = self.history_drawer.show(ui, &self.activity_cache, max_drawer_h, hist_bottom_y) {
                        match ev {
                            HistoryDrawerEvent::Close => {
                                self.history_drawer_open = false;
                            }
                            HistoryDrawerEvent::ClearAll => {
                                self.history_db.clear();
                                self.activity_cache.clear();
                            }
                            HistoryDrawerEvent::JumpTo { id, timestamp, action } => {
                                if let Some(snap_json) = self.history_db.get_snapshot(id) {
                                    match self.restore_snapshot_from_json(&snap_json) {
                                        Ok(()) => {
                                            self.model_status = Some(format!(
                                                "✓ Dokumen dipulihkan ke waktu {} ({})",
                                                timestamp, action
                                            ));
                                        }
                                        Err(e) => {
                                            self.model_status = Some(format!("Gagal memulihkan snapshot: {e}"));
                                        }
                                    }
                                } else {
                                    self.model_status = Some("Snapshot untuk riwayat ini tidak tersedia".to_string());
                                }
                            }
                        }
                    }
                });
        }

        // 2. Floating Buttons Bar di Pojok Kanan Bawah (Icon History di Kiri, Icon Folder di Kanan)
        let btns_pos = egui::pos2(screen_rect.max.x - 16.0, screen_rect.max.y - 16.0);
        egui::Area::new(egui::Id::new("ducad-bottom-right-floating-btns"))
            .fixed_pos(btns_pos)
            .pivot(egui::Align2::RIGHT_BOTTOM)
            .order(egui::Order::Foreground)
            .show(&ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);

                    // Tombol History (Kiri)
                    let hist_resp = round_floating_icon_btn(
                        ui,
                        egui_material_icons::icons::ICON_HISTORY.codepoint,
                        self.history_drawer_open,
                        "Riwayat Aktivitas & Perubahan (2D & 3D)",
                    );
                    if hist_resp.clicked() {
                        self.history_drawer_open = !self.history_drawer_open;
                    }

                    // Tombol Folder (Kanan)
                    let folder_resp = round_floating_icon_btn(
                        ui,
                        egui_material_icons::icons::ICON_FOLDER.codepoint,
                        self.items_drawer_open,
                        "Properties Dokumen (Objek 2D & Solid Body 3D)",
                    );
                    if folder_resp.clicked() {
                        self.items_drawer_open = !self.items_drawer_open;
                    }
                });
            });


        let viewcube_y = 102.0;
        let viewcube_x = screen_rect.max.x - topbar_margin_right - 42.0;
        let viewcube_pos = egui::pos2(viewcube_x, viewcube_y);
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

        // =========================================================================
        // MODULAR TOOL POPUPS DI POJOK KANAN BAWAH (BOTTOM-RIGHT)
        // =========================================================================
        let mut popup_ev: Option<ToolPopupEvent> = None;

        match self.tool {
            ToolKind::History => {
                let mut state = HistoryPopupState {
                    can_undo_model: self.model_undo.can_undo(),
                    can_redo_model: self.model_undo.can_redo(),
                    total_entities_count: self.sketch().entities.len(),
                    total_bodies_count: self.model.doc.bodies.len(),
                    status_message: self.model_status.clone(),
                };
                popup_ev = HistoryPopup::show(&ctx, &mut state, screen_rect);
            }
            _ => {}
        }

        if let Some(ev) = popup_ev {
            match ev {
                ToolPopupEvent::Close => {
                    self.set_tool(ToolKind::Select);
                    self.picking_mode = PickMode::None;
                }
                ToolPopupEvent::ApplyExtrude { distance } => {
                    self.extrude_distance_input = distance.to_string();
                    self.extrude_selected();
                }
                ToolPopupEvent::ApplyFaceExtrude { distance } => {
                    self.face_extrude_distance_input = distance.to_string();
                    self.extrude_active_face(distance);
                }
                ToolPopupEvent::SketchOnFace => {
                    self.sketch_on_active_face();
                }
                ToolPopupEvent::ApplyRevolvePreset { preset_idx, angle_deg } => {
                    let preset = match preset_idx {
                        0 => ducad_ui::RevolveAxisPreset::YAxisOrigin,
                        1 => ducad_ui::RevolveAxisPreset::XAxisOrigin,
                        2 => ducad_ui::RevolveAxisPreset::BBoxLeft,
                        3 => ducad_ui::RevolveAxisPreset::BBoxBottom,
                        _ => ducad_ui::RevolveAxisPreset::CustomTwoPoints,
                    };
                    self.revolve_selected_with_preset(preset, angle_deg);
                }
                ToolPopupEvent::StartManualRevolve => {
                    self.set_tool(ToolKind::Revolve);
                }
                ToolPopupEvent::StageLoftBottom => {
                    match crate::model::build_profile_from_selection(
                        self.sketch(),
                        &self.selected,
                    ) {
                        Ok(profile) => {
                            self.pending_loft_bottom = Some(profile);
                            self.selected.clear();
                            self.model_status = Some(
                                "✓ Profil bawah tersimpan! Sekarang klik profil kedua di kanvas lalu klik 'Eksekusi Loft'."
                                    .to_string(),
                            );
                        }
                        Err(msg) => self.model_status = Some(format!("Pilih profil di kanvas: {msg}")),
                    }
                }
                ToolPopupEvent::ApplyLoft { height } => {
                    self.loft_height_input = height.to_string();
                    self.loft_selected();
                }
                ToolPopupEvent::ToggleFacePicking => {
                    if self.picking_mode == PickMode::Face {
                        self.picking_mode = PickMode::None;
                    } else {
                        self.picking_mode = PickMode::Face;
                    }
                }
                ToolPopupEvent::ApplyShell { thickness } => {
                    self.shell_thickness_input = thickness.to_string();
                    self.shell_selected_body();
                }
                ToolPopupEvent::ApplyBooleanUnion => {
                    self.boolean_selected(BooleanKind::Union, "Union", "Union");
                }
                ToolPopupEvent::ApplyBooleanSubtract => {
                    self.boolean_selected(BooleanKind::Subtract, "Subtract", "Subtract");
                }
                ToolPopupEvent::ApplyBooleanIntersect => {
                    self.boolean_selected(BooleanKind::Intersect, "Intersect", "Intersect");
                }
                ToolPopupEvent::UndoModel => {
                    self.model_undo.undo(&mut self.model);
                    self.selected_bodies.clear();
                }
                ToolPopupEvent::RedoModel => {
                    self.model_undo.redo(&mut self.model);
                    self.selected_bodies.clear();
                }
                ToolPopupEvent::ToggleShowAllDimensions => {
                    self.show_all_dimensions = !self.show_all_dimensions;
                }
                ToolPopupEvent::RemoveMeasurement(i) => {
                    if i < self.measurements.len() {
                        self.measurements.remove(i);
                    }
                }
                ToolPopupEvent::ClearMeasurements => {
                    self.measurements.clear();
                }
                ToolPopupEvent::UpdateEntityLine {
                    id_raw,
                    start_x,
                    start_y,
                    end_x,
                    end_y,
                } => {
                    if let Some(&id) = self.selected.iter().find(|i| i.data().as_ffi() == id_raw) {
                        let new_entity = Entity::Line {
                            start: DVec2::new(start_x, start_y),
                            end: DVec2::new(end_x, end_y),
                        };
                        self.execute_sketch_command(Box::new(
                            UpdateEntity::new("Ubah Garis", id, new_entity),
                        ));
                    }
                }
                ToolPopupEvent::UpdateEntityCircle {
                    id_raw,
                    center_x,
                    center_y,
                    radius,
                } => {
                    if let Some(&id) = self.selected.iter().find(|i| i.data().as_ffi() == id_raw) {
                        let new_entity = Entity::Circle {
                            center: DVec2::new(center_x, center_y),
                            radius,
                        };
                        self.execute_sketch_command(Box::new(
                            UpdateEntity::new("Ubah Lingkaran", id, new_entity),
                        ));
                    }
                }
                ToolPopupEvent::UpdateEntityArc {
                    id_raw,
                    center_x,
                    center_y,
                    radius,
                    start_angle_deg,
                    end_angle_deg,
                } => {
                    if let Some(&id) = self.selected.iter().find(|i| i.data().as_ffi() == id_raw) {
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
                ToolPopupEvent::UpdateEntityEllipse {
                    id_raw,
                    center_x,
                    center_y,
                    radius_x,
                    radius_y,
                } => {
                    if let Some(&id) = self.selected.iter().find(|i| i.data().as_ffi() == id_raw) {
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
                ToolPopupEvent::UpdateEntityRectangle {
                    entity_ids: _,
                    length_p,
                    length_l,
                    anchor,
                } => {
                    if let Some(rect) = detect_rectangle(self.sketch(), &self.selected) {
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
                ToolPopupEvent::ApplyConstraint(act) => {
                    let ids: Vec<EntityId> = self.selected.iter().copied().collect();
                    match act {
                        InspectorConstraintAction::Horizontal => {
                            if let [id] = ids.as_slice() {
                                self.apply_constraint(Constraint::Horizontal { line: *id });
                            }
                        }
                        InspectorConstraintAction::Vertical => {
                            if let [id] = ids.as_slice() {
                                self.apply_constraint(Constraint::Vertical { line: *id });
                            }
                        }
                        InspectorConstraintAction::Parallel => {
                            if let [a, b] = ids.as_slice() {
                                self.apply_constraint(Constraint::Parallel { a: *a, b: *b });
                            }
                        }
                        InspectorConstraintAction::Perpendicular => {
                            if let [a, b] = ids.as_slice() {
                                self.apply_constraint(Constraint::Perpendicular { a: *a, b: *b });
                            }
                        }
                        InspectorConstraintAction::EqualLength => {
                            if let [a, b] = ids.as_slice() {
                                self.apply_constraint(Constraint::EqualLength { a: *a, b: *b });
                            }
                        }
                        InspectorConstraintAction::EqualRadius => {
                            if let [a, b] = ids.as_slice() {
                                self.apply_constraint(Constraint::EqualRadius { a: *a, b: *b });
                            }
                        }
                        InspectorConstraintAction::Tangent => {
                            if let [a, b] = ids.as_slice() {
                                self.apply_constraint(Constraint::Tangent { a: *a, b: *b });
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
                ToolPopupEvent::DeleteSelectedBodies => {
                    self.delete_selected_bodies();
                }
                ToolPopupEvent::DeleteSelectedEntities => {
                    if !self.selected.is_empty() {
                        let ids: Vec<EntityId> = self.selected.iter().copied().collect();
                        self.execute_sketch_command(Box::new(DeleteEntities::new(ids)));
                        self.selected.clear();
                    }
                }
            }
        }

        // Shapr3D-Style Floating Contextual Action Bar
        let has_sketch_sel = !self.selected.is_empty();
        let has_face_sel = self.active_face.is_some();
        let has_body_sel = !self.selected_bodies.is_empty();

        if has_sketch_sel || has_face_sel || has_body_sel {
            egui::Area::new(egui::Id::new("ducad-context-action-bar-area"))
                .fixed_pos(egui::pos2(screen_center_x, screen_rect.max.y - 18.0))
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
                                ContextAction::Rename => {
                                    // Buka popup rename untuk grup 2D
                                    // Isi input dengan nama grup saat ini (jika semua entitas punya nama yang sama)
                                    let current_name = {
                                        let sketch = self.sketch();
                                        let mut names = self.selected.iter()
                                            .filter_map(|id| sketch.entity_names.get(id).cloned())
                                            .collect::<std::collections::HashSet<_>>();
                                        if names.len() == 1 {
                                            names.drain().next().unwrap_or_default()
                                        } else {
                                            String::new()
                                        }
                                    };
                                    self.rename_input = current_name;
                                    self.rename_target = RenameTarget::Sketch2d;
                                    self.rename_popup_open = true;
                                }
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
                                ContextAction::Shell => {
                                    self.set_tool(ToolKind::Shell);
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
                                ContextAction::Boolean => {
                                    self.set_tool(ToolKind::Boolean);
                                }
                                ContextAction::Delete => {
                                    self.delete_selected_bodies();
                                }
                                ContextAction::ClearSelection => {
                                    self.selected_bodies.clear();
                                }
                                ContextAction::Rename => {
                                    // Buka popup rename untuk body 3D pertama yang dipilih
                                    if let Some(&body_id) = self.selected_bodies.iter().next() {
                                        let current_name = self.model.doc.bodies
                                            .get(body_id)
                                            .map(|b| b.name.clone())
                                            .unwrap_or_default();
                                        self.rename_input = current_name;
                                        self.rename_target = RenameTarget::Body3d(body_id);
                                        self.rename_popup_open = true;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                });
        }

        // =========================================================================
        // RENAME POPUP (top-center HUD, muncul saat tombol Nama diklik)
        // =========================================================================
        if self.rename_popup_open {
            let popup_label = match &self.rename_target {
                RenameTarget::Sketch2d => "Nama Grup Objek 2D",
                RenameTarget::Body3d(_) => "Nama Body 3D",
                RenameTarget::None => "Nama",
            };
            let rename_input_clone = self.rename_input.clone();
            let rename_target_clone = self.rename_target.clone();

            let popup_x = screen_center_x;
            let popup_y = screen_rect.min.y + 60.0;

            egui::Area::new(egui::Id::new("ducad-rename-popup-area"))
                .fixed_pos(egui::pos2(popup_x, popup_y))
                .pivot(egui::Align2::CENTER_TOP)
                .order(egui::Order::Foreground)
                .show(&ctx, |ui| {
                    let mut input_buf = rename_input_clone.clone();
                    if let Some(ev) = CanvasHud::show_rename_popup(ui, popup_label, &mut input_buf) {
                        match ev {
                            RenamePopupEvent::Confirm(new_name) => {
                                match &rename_target_clone {
                                    RenameTarget::Sketch2d => {
                                        let ids: Vec<EntityId> = self.selected.iter().copied().collect();
                                        if !ids.is_empty() {
                                            self.execute_sketch_command(Box::new(
                                                RenameEntities::new(ids, new_name.clone()),
                                            ));
                                        }
                                    }
                                    RenameTarget::Body3d(body_id) => {
                                        if let Some(b) = self.model.doc.bodies.get_mut(*body_id) {
                                            b.name = new_name.clone();
                                            self.model.doc.dirty = true;
                                        }
                                    }
                                    RenameTarget::None => {}
                                }
                                self.rename_popup_open = false;
                                self.rename_target = RenameTarget::None;
                                self.rename_input.clear();
                            }
                            RenamePopupEvent::Cancel => {
                                self.rename_popup_open = false;
                                self.rename_target = RenameTarget::None;
                                self.rename_input.clear();
                            }
                        }
                    } else {
                        // Popup masih terbuka — sync buffer
                        self.rename_input = input_buf;
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

        egui::Area::new(egui::Id::new("ducad-hud-status-area"))
            .pivot(egui::Align2::LEFT_TOP)
            .fixed_pos(egui::pos2(screen_rect.min.x + 16.0, screen_rect.min.y + 60.0))
            .order(egui::Order::Foreground)
            .show(&ctx, |ui| {
                if let Some(ev) = CanvasHud::show_status_pill(
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
            if self.active_face.is_some() {
                self.revolve_active_face(
                    (axis_origin.x, axis_origin.y),
                    (axis_dir.x, axis_dir.y),
                    angle_opt,
                );
            } else {
                self.revolve_selected(
                    (axis_origin.x, axis_origin.y),
                    (axis_dir.x, axis_dir.y),
                    angle_opt,
                );
            }
            self.set_tool(ToolKind::Select);
        }
    }

    /// Batalkan sumbu revolve yang sedang di-stage.
    pub fn cancel_staged_revolve(&mut self) {
        self.revolve_staged_axis = None;
        self.set_tool(ToolKind::Select);
    }

    /// Eksekusi / perbarui loft 3D yang sedang di-stage (live preview) dari 2 region.
    pub fn update_staged_loft(&mut self, regions: &[ducad_sketch::region::ClosedRegion]) {
        if regions.len() != 2 {
            return;
        }
        let height: f64 = match self.loft_height_input.trim().parse() {
            Ok(v) if v > 0.0 => v,
            _ => return,
        };

        let (idx_b, idx_t) = if self.loft_is_flipped {
            (1, 0)
        } else {
            (0, 1)
        };

        let bottom = match crate::model::build_profile_from_selection(
            self.sketch(),
            &regions[idx_b].entity_ids,
        ) {
            Ok(p) => p,
            Err(e) => {
                self.model_status = Some(format!("Profil 1 error: {e}"));
                return;
            }
        };

        let top = match crate::model::build_profile_from_selection(
            self.sketch(),
            &regions[idx_t].entity_ids,
        ) {
            Ok(p) => p,
            Err(e) => {
                self.model_status = Some(format!("Profil 2 error: {e}"));
                return;
            }
        };

        match ducad_kernel::loft_profiles(&bottom, &top, height) {
            Ok(shape) => {
                let geo = crate::model::BodyGeometry::from_shape(shape);
                if let Some(existing_id) = self.loft_staged_body_id {
                    self.model.geometry.insert(existing_id, geo);
                } else {
                    let id = self.model.doc.add_body("Loft");
                    self.model.geometry.insert(id, geo);
                    self.loft_staged_body_id = Some(id);
                }
            }
            Err(e) => {
                self.model_status = Some(format!("Loft gagal: {e}"));
            }
        }
    }

    /// Terapkan (commit) loft yang sedang di-stage ke model dan catat ke undo history.
    pub fn commit_staged_loft(&mut self, regions: &[ducad_sketch::region::ClosedRegion]) {
        if let Some(staged_id) = self.loft_staged_body_id.take() {
            if let Some(geo) = self.model.geometry.remove(staged_id) {
                self.model.doc.bodies.remove(staged_id);
                self.execute_model_command(
                    Box::new(crate::model::AddSolidCommand::new("Loft", geo)),
                    &format!("Tinggi {} mm", self.loft_height_input.trim()),
                );
            }
            self.set_tool(ToolKind::Select);
            self.selected.clear();
            self.selection_box = None;
            self.loft_alignment_dismissed = false;
            self.model_status = Some("✓ Loft 3D berhasil dibuat!".to_string());
        } else if regions.len() == 2 {
            self.update_staged_loft(regions);
            self.commit_staged_loft(regions);
        }
    }

    /// Batalkan loft yang sedang di-stage (hapus preview body).
    pub fn cancel_staged_loft(&mut self) {
        if let Some(staged_id) = self.loft_staged_body_id.take() {
            self.model.geometry.remove(staged_id);
            self.model.doc.bodies.remove(staged_id);
        }
        self.set_tool(ToolKind::Select);
        self.selected.clear();
        self.selection_box = None;
        self.loft_alignment_dismissed = false;
    }
}

/// Helper tombol lingkaran mengambang di pojok kanan bawah bergaya Shapr3D.
fn round_floating_icon_btn(
    ui: &mut egui::Ui,
    icon: &'static str,
    is_active: bool,
    tooltip: &str,
) -> egui::Response {
    let size = egui::Vec2::splat(38.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());

    let is_hovered = resp.hovered();
    let bg_color = if is_active {
        egui::Color32::from_rgb(18, 48, 88)
    } else if is_hovered {
        egui::Color32::from_rgb(38, 44, 58)
    } else {
        egui::Color32::from_rgb(24, 27, 34)
    };

    let stroke_color = if is_active || is_hovered {
        ducad_ui::ACCENT_BLUE
    } else {
        ducad_ui::BORDER_SUBTLE
    };

    ui.painter().rect(
        rect,
        egui::CornerRadius::same(19),
        bg_color,
        egui::Stroke::new(if is_active || is_hovered { 1.5 } else { 1.0 }, stroke_color),
        egui::StrokeKind::Inside,
    );

    let icon_color = if is_active || is_hovered {
        egui::Color32::WHITE
    } else {
        ducad_ui::ACCENT_BLUE
    };

    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(19.0),
        icon_color,
    );

    resp.on_hover_text(tooltip)
}



