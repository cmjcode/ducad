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
    ActivityItemInfo, ActivityKindUi, AssemblyDrawer, AssemblyDrawerEvent, BodyItemInfo, CanvasHud,
    CanvasHudEvent, CmfDrawer, CmfDrawerEvent, CommandPalette, ContextAction, ContextActionBar,
    DraftAnalysisPopup, Entity2dItemInfo, FeatureTreeDrawer, FeatureTreeEvent, HistoryDrawer,
    HistoryPopup, HistoryPopupState, InspectorConstraintAction, InspectorRectAnchor, ItemsDrawer,
    ItemsDrawerEvent, LeftToolbar, LightingDrawer, PlaneItemInfo, PlanesDrawer, PlanesDrawerEvent,
    RadialMenu, RenamePopupEvent, ThemeMode, ToolPopupEvent, ToolbarEvent, TopBar, TopBarEvent,
    TopBarFileOp, TopBarState, ViewCube, ViewCubeAction, ZebraHudAction,
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
    pub sketches: Vec<Sketch>,
    pub undos: Vec<ducad_sketch::UndoStack>,
    pub datum_planes: Vec<ducad_render::plane::DatumPlane>,
    pub datum_plane_counter: u32,

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
    pub fillet_radius_end_input: String,
    pub fillet_variable_enabled: bool,
    pub chamfer_distance_input: String,
    pub shell_thickness_input: String,
    pub shell_direction: ducad_kernel::Direction,
    pub boolean_op: ducad_ui::BooleanOpKind,

    pub fillet_2d_radius: f64,
    pub chamfer_2d_dist: f64,
    pub fillet_chamfer_first_line: Option<EntityId>,

    pub active_sketch_corner: Option<(EntityId, EntityId, glam::DVec2)>,
    pub active_sketch_fillet_arc: Option<EntityId>,
    pub sketch_corner_gizmo_radius: f64,
    pub sketch_corner_gizmo_active: bool,
    pub sketch_corner_dimension_editing: bool,
    pub sketch_corner_edit_input: String,

    pub pending_loft_bottom: Option<ducad_kernel::Profile>,
    pub loft_height_input: String,
    pub loft_alignment_dismissed: bool,
    pub loft_is_flipped: bool,
    pub loft_staged_body_id: Option<BodyId>,
    pub pending_sweep_profile: Option<(ducad_kernel::Profile, ducad_render::SketchPlane)>,
    pub pending_sweep_path: Option<Vec<ducad_kernel::PathSegment>>,
    pub sweep_path_plane_idx: Option<usize>,
    pub hovered_plane_idx: Option<usize>,
    pub selection_box: Option<(glam::DVec2, glam::DVec2)>,

    pub picking_mode: PickMode,
    pub selected_edges: Vec<PickedEdge>,
    pub selected_faces: Vec<PickRay>,
    pub active_face: Option<(BodyId, PickRay, FaceHit)>,
    pub face_extrude_distance_input: String,

    pub active_vertex: Option<(BodyId, PickRay, (f64, f64, f64))>,

    pub current_file_path: Option<PathBuf>,
    pub file_status: Option<String>,

    pub language: ducad_i18n::Language,
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

    /// Studio Lighting, SSAO & Bayangan Kontak Lantai (Fase 4.2).
    pub studio_config: ducad_render::StudioConfig,
    /// Zebra stripes reflection inspection (Fase 3.1).
    pub zebra_config: ducad_render::ZebraConfig,
    /// Draft angle heatmap inspection (Fase 3.2).
    pub draft_config: ducad_render::DraftConfig,

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
    pub cmf_drawer: CmfDrawer,
    pub lighting_drawer: LightingDrawer,
    pub planes_drawer: PlanesDrawer,
    pub feature_tree_drawer: FeatureTreeDrawer,
    pub assembly_drawer: AssemblyDrawer,
    pub viewcube: ViewCube,
    pub items_drawer_open: bool,
    pub history_drawer_open: bool,
    pub cmf_drawer_open: bool,
    pub lighting_drawer_open: bool,
    pub planes_drawer_open: bool,
    pub feature_tree_drawer_open: bool,
    pub assembly_drawer_open: bool,
    pub assembly_tree: ducad_core::AssemblyTree,
    pub selected_assembly_instance: Option<ducad_core::AssemblyInstanceId>,
    pub selected_assembly_mate: Option<ducad_core::MateConstraintId>,
    pub staged_mate_kind: Option<ducad_core::MateKind>,
    pub mate_offset_distance: f64,
    pub mate_angle_deg: f64,
    pub mate_flip_alignment: bool,
    pub mate_lock_rotation: bool,
    pub staged_mate_targets: Vec<(BodyId, PickRay, ducad_kernel::FaceHit)>,
    pub parametric_dag: ducad_core::ParametricDag,
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
    pub construction_mode: bool,
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
    pub hovered_corner_2d: Option<glam::DVec2>,
    pub active_edge: Option<(BodyId, PickRay, (f64, f64, f64))>,

    pub filleting_edge_from_gizmo: bool,
    pub edge_gizmo_radius: f64,
    pub edge_gizmo_dimension_editing: bool,
    pub edge_gizmo_edit_input: String,

    pub round_history: std::collections::HashMap<BodyId, RoundHistory>,
    pub editing_round: Option<(BodyId, usize)>,
    pub hole_history: std::collections::HashMap<BodyId, crate::types::HoleHistory>,
    pub editing_hole_idx: Option<(BodyId, usize)>,

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

    /// State Draft Angle (Fase 2.1 — Manufaktur Plastik).
    pub draft_angle_input: String,
    pub draft_pull_dir: ducad_ui::DraftPullDir,

    /// State Split Body & Split Face (Fase 2.2 — Potong Benda).
    pub split_mode: ducad_ui::SplitMode,
    pub split_plane: ducad_ui::SplitPlaneKind,
    pub split_offset_input: String,

    /// State Datum Plane (Fase 10.1 — Bidang Referensi Bebas).
    pub datum_mode: ducad_ui::DatumPlaneMode,
    pub datum_offset_input: String,
    pub datum_angle_input: String,
    pub datum_flip: bool,
    pub datum_base_plane_idx: usize,
    pub datum_selected_points: Vec<glam::Vec3>,

    /// State Regular Polygon (Fase 9.3 — Segi-N Beraturan: Inscribed vs Circumscribed).
    pub polygon_sides: usize,
    pub polygon_mode: ducad_sketch::PolygonMode,

    /// State Slot (Fase 9.4 — Lubang Pengait / Rel Baut: Center-to-Center vs Overall).
    pub slot_mode: ducad_sketch::SlotMode,
    pub slot_width: f64,

    /// State Pattern / Array (Fase 2.3 — Linier & Sirkular 2D & 3D).
    pub pattern_kind: ducad_ui::PatternKind,
    pub pattern_count_x: usize,
    pub pattern_pitch_x: f64,
    pub pattern_count_y: usize,
    pub pattern_pitch_y: f64,
    pub pattern_count_z: usize,
    pub pattern_pitch_z: f64,
    pub pattern_circ_count: usize,
    pub pattern_circ_angle_deg: f64,
    pub pattern_circ_radius: f64,
    pub pattern_circ_axis: ducad_ui::PatternAxisPreset,
    pub pattern_custom_pivot_2d: Option<glam::DVec2>,
    pub pattern_custom_pivot_3d: Option<glam::Vec3>,
    pub pattern_dimension_editing_x: bool,
    pub pattern_dimension_editing_y: bool,
    pub pattern_dimension_editing_z: bool,
    pub pattern_dimension_editing_angle: bool,
    pub pattern_dimension_editing_radius: bool,
    pub pattern_dimension_edit_input: String,

    /// State Rib Support (Fase 2.4 — Tulang Penguat Casing).
    pub rib_thickness_input: String,
    pub rib_depth_input: String,
    pub rib_draft_input: String,
    pub rib_angle_input: String,
    pub rib_start_pt: Option<glam::DVec3>,
    pub rib_end_pt: Option<glam::DVec3>,
    pub rib_normal_dir: glam::DVec3,

    /// State Variable Thickness Shell (Fase 2.4 — Casing Ketebalan Khusus).
    pub shell_variable_faces: Vec<(ducad_kernel::PickRay, f64)>,
    pub shell_var_thickness_input: String,
    pub shell_is_variable_mode: bool,

    /// State Lembar Kerja Gambar Teknik 2D (Fase 5).
    pub drawing_sheet_state: ducad_ui::DrawingSheetViewState,
    pub drawing_sheet_doc: Option<ducad_io::drawing::DrawingSheet>,

    /// State Hole Wizard & Standar Baut ISO (Fase 9.2).
    pub hole_popup_state: ducad_ui::HolePopupState,
    pub editing_hole_ruler_idx: Option<usize>,
    pub editing_hole_ruler_input: String,

    /// State Teks 2D & Emboss/Deboss (Fase 9.5).
    pub text_popup_state: ducad_ui::TextPopupState,
    pub custom_font_bytes: Option<Vec<u8>>,

    /// State Helix / Coil / Spring Tool (Fase 10.2).
    pub helix_popup_state: ducad_ui::HelixPopupState,
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
            sketches: vec![Sketch::default(), Sketch::default(), Sketch::default()],
            undos: vec![
                ducad_sketch::UndoStack::default(),
                ducad_sketch::UndoStack::default(),
                ducad_sketch::UndoStack::default(),
            ],
            datum_planes: Vec::new(),
            datum_plane_counter: 0,
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
            construction_mode: false,

            model: ModelDoc::default(),
            model_undo: ducad_core::UndoStack::default(),
            selected_bodies: HashSet::new(),
            model_status: None,
            extrude_distance_input: "10".to_string(),
            fillet_radius_input: "2".to_string(),
            fillet_radius_end_input: "5".to_string(),
            fillet_variable_enabled: false,
            chamfer_distance_input: "2".to_string(),
            shell_thickness_input: "2".to_string(),
            shell_direction: ducad_kernel::Direction::PosZ,
            boolean_op: ducad_ui::BooleanOpKind::Union,
            fillet_2d_radius: 5.0,
            chamfer_2d_dist: 5.0,
            fillet_chamfer_first_line: None,
            active_sketch_corner: None,
            active_sketch_fillet_arc: None,
            sketch_corner_gizmo_radius: 5.0,
            sketch_corner_gizmo_active: false,
            sketch_corner_dimension_editing: false,
            sketch_corner_edit_input: "5.0".to_string(),

            pending_loft_bottom: None,
            loft_height_input: "20.0".to_string(),
            loft_alignment_dismissed: false,
            loft_is_flipped: false,
            loft_staged_body_id: None,
            pending_sweep_profile: None,
            pending_sweep_path: None,
            sweep_path_plane_idx: None,
            hovered_plane_idx: None,
            selection_box: None,
            picking_mode: PickMode::default(),
            selected_edges: Vec::new(),
            selected_faces: Vec::new(),
            active_face: None,
            face_extrude_distance_input: "15".to_string(),
            active_vertex: None,

            current_file_path: None,
            file_status: None,

            language: ducad_i18n::Language::default(),
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
            studio_config: ducad_render::StudioConfig::default(),
            zebra_config: ducad_render::ZebraConfig::default(),
            draft_config: ducad_render::DraftConfig::default(),
            show_all_dimensions: false,
            editing_dimension_entity: None,
            editing_dimension_input: String::new(),

            import_worker: ImportWorker::spawn(),
            pending_imports: 0,

            left_toolbar: LeftToolbar::default(),
            items_drawer: ItemsDrawer::default(),
            history_drawer: HistoryDrawer::default(),
            cmf_drawer: CmfDrawer::default(),
            lighting_drawer: LightingDrawer::default(),
            planes_drawer: PlanesDrawer::default(),
            feature_tree_drawer: FeatureTreeDrawer::default(),
            assembly_drawer: AssemblyDrawer::default(),
            viewcube: ViewCube::default(),
            items_drawer_open: false,
            history_drawer_open: false,
            cmf_drawer_open: false,
            lighting_drawer_open: false,
            planes_drawer_open: false,
            feature_tree_drawer_open: false,
            assembly_drawer_open: false,
            assembly_tree: ducad_core::AssemblyTree::new(),
            selected_assembly_instance: None,
            selected_assembly_mate: None,
            staged_mate_kind: None,
            mate_offset_distance: 0.0,
            mate_angle_deg: 0.0,
            mate_flip_alignment: false,
            mate_lock_rotation: false,
            staged_mate_targets: Vec::new(),
            parametric_dag: ducad_core::ParametricDag::new(),
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
            hovered_corner_2d: None,
            active_edge: None,
            filleting_edge_from_gizmo: false,
            edge_gizmo_radius: 3.0,
            edge_gizmo_dimension_editing: false,
            edge_gizmo_edit_input: "3".to_string(),

            round_history: std::collections::HashMap::new(),
            editing_round: None,
            hole_history: std::collections::HashMap::new(),
            editing_hole_idx: None,

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

            draft_angle_input: "3.0".to_string(),
            draft_pull_dir: ducad_ui::DraftPullDir::PosZ,

            split_mode: ducad_ui::SplitMode::SplitBody,
            split_plane: ducad_ui::SplitPlaneKind::XY,
            split_offset_input: "0.0".to_string(),

            datum_mode: ducad_ui::DatumPlaneMode::Offset,
            datum_offset_input: "20.0".to_string(),
            datum_angle_input: "45.0".to_string(),
            datum_flip: false,
            datum_base_plane_idx: 0,
            datum_selected_points: Vec::new(),

            polygon_sides: 6,
            polygon_mode: ducad_sketch::PolygonMode::Inscribed,

            slot_mode: ducad_sketch::SlotMode::CenterToCenter,
            slot_width: 10.0,

            pattern_kind: ducad_ui::PatternKind::Linear,
            pattern_count_x: 3,
            pattern_pitch_x: 20.0,
            pattern_count_y: 2,
            pattern_pitch_y: 20.0,
            pattern_count_z: 1,
            pattern_pitch_z: 20.0,
            pattern_circ_count: 6,
            pattern_circ_angle_deg: 360.0,
            pattern_circ_radius: 30.0,
            pattern_circ_axis: ducad_ui::PatternAxisPreset::Z,
            pattern_custom_pivot_2d: None,
            pattern_custom_pivot_3d: None,
            pattern_dimension_editing_x: false,
            pattern_dimension_editing_y: false,
            pattern_dimension_editing_z: false,
            pattern_dimension_editing_angle: false,
            pattern_dimension_editing_radius: false,
            pattern_dimension_edit_input: "20.0".to_string(),

            rib_thickness_input: "2.0".to_string(),
            rib_depth_input: "15.0".to_string(),
            rib_draft_input: "0.0".to_string(),
            rib_angle_input: "0.0".to_string(),
            rib_start_pt: None,
            rib_end_pt: None,
            rib_normal_dir: glam::dvec3(0.0, 0.0, -1.0),

            shell_variable_faces: Vec::new(),
            shell_var_thickness_input: "4.0".to_string(),
            shell_is_variable_mode: false,

            drawing_sheet_state: ducad_ui::DrawingSheetViewState::default(),
            drawing_sheet_doc: None,
            hole_popup_state: ducad_ui::HolePopupState::default(),
            editing_hole_ruler_idx: None,
            editing_hole_ruler_input: String::new(),
            text_popup_state: ducad_ui::TextPopupState::default(),
            custom_font_bytes: None,
            helix_popup_state: ducad_ui::HelixPopupState::default(),
        }
    }

    #[cfg(test)]
    pub fn new_for_test() -> Self {
        let history_db = crate::history_db::HistoryDb::in_memory();
        Self {
            camera: OrbitCamera::default(),
            sketches: vec![Sketch::default(), Sketch::default(), Sketch::default()],
            undos: vec![
                ducad_sketch::UndoStack::default(),
                ducad_sketch::UndoStack::default(),
                ducad_sketch::UndoStack::default(),
            ],
            datum_planes: Vec::new(),
            datum_plane_counter: 0,
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
            construction_mode: false,

            model: ModelDoc::default(),
            model_undo: ducad_core::UndoStack::default(),
            selected_bodies: HashSet::new(),
            model_status: None,
            extrude_distance_input: "10".to_string(),
            fillet_radius_input: "2".to_string(),
            fillet_radius_end_input: "5".to_string(),
            fillet_variable_enabled: false,
            chamfer_distance_input: "2".to_string(),
            shell_thickness_input: "2".to_string(),
            shell_direction: ducad_kernel::Direction::PosZ,
            boolean_op: ducad_ui::BooleanOpKind::Union,
            fillet_2d_radius: 5.0,
            chamfer_2d_dist: 5.0,
            fillet_chamfer_first_line: None,
            active_sketch_corner: None,
            active_sketch_fillet_arc: None,
            sketch_corner_gizmo_radius: 5.0,
            sketch_corner_gizmo_active: false,
            sketch_corner_dimension_editing: false,
            sketch_corner_edit_input: "5.0".to_string(),

            pending_loft_bottom: None,
            loft_height_input: "20.0".to_string(),
            loft_alignment_dismissed: false,
            loft_is_flipped: false,
            loft_staged_body_id: None,
            pending_sweep_profile: None,
            pending_sweep_path: None,
            sweep_path_plane_idx: None,
            hovered_plane_idx: None,
            selection_box: None,
            picking_mode: PickMode::default(),
            selected_edges: Vec::new(),
            selected_faces: Vec::new(),
            active_face: None,
            face_extrude_distance_input: "15".to_string(),
            active_vertex: None,

            current_file_path: None,
            file_status: None,

            language: ducad_i18n::Language::default(),
            theme: ThemeMode::default(),
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
            studio_config: ducad_render::StudioConfig::default(),
            zebra_config: ducad_render::ZebraConfig::default(),
            draft_config: ducad_render::DraftConfig::default(),
            show_all_dimensions: false,
            editing_dimension_entity: None,
            editing_dimension_input: String::new(),

            import_worker: ImportWorker::spawn(),
            pending_imports: 0,

            left_toolbar: LeftToolbar::default(),
            items_drawer: ItemsDrawer::default(),
            history_drawer: HistoryDrawer::default(),
            cmf_drawer: CmfDrawer::default(),
            lighting_drawer: LightingDrawer::default(),
            planes_drawer: PlanesDrawer::default(),
            feature_tree_drawer: FeatureTreeDrawer::default(),
            assembly_drawer: AssemblyDrawer::default(),
            viewcube: ViewCube::default(),
            items_drawer_open: false,
            history_drawer_open: false,
            cmf_drawer_open: false,
            lighting_drawer_open: false,
            planes_drawer_open: false,
            feature_tree_drawer_open: false,
            assembly_drawer_open: false,
            assembly_tree: ducad_core::AssemblyTree::new(),
            selected_assembly_instance: None,
            selected_assembly_mate: None,
            staged_mate_kind: None,
            mate_offset_distance: 0.0,
            mate_angle_deg: 0.0,
            mate_flip_alignment: false,
            mate_lock_rotation: false,
            staged_mate_targets: Vec::new(),
            parametric_dag: ducad_core::ParametricDag::new(),
            history_db,
            activity_cache: Vec::new(),
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
            hovered_corner_2d: None,
            active_edge: None,
            filleting_edge_from_gizmo: false,
            edge_gizmo_radius: 3.0,
            edge_gizmo_dimension_editing: false,
            edge_gizmo_edit_input: "3".to_string(),

            round_history: std::collections::HashMap::new(),
            editing_round: None,
            hole_history: std::collections::HashMap::new(),
            editing_hole_idx: None,

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

            draft_angle_input: "3.0".to_string(),
            draft_pull_dir: ducad_ui::DraftPullDir::PosZ,

            split_mode: ducad_ui::SplitMode::SplitBody,
            split_plane: ducad_ui::SplitPlaneKind::XY,
            split_offset_input: "0.0".to_string(),

            datum_mode: ducad_ui::DatumPlaneMode::Offset,
            datum_offset_input: "20.0".to_string(),
            datum_angle_input: "45.0".to_string(),
            datum_flip: false,
            datum_base_plane_idx: 0,
            datum_selected_points: Vec::new(),

            polygon_sides: 6,
            polygon_mode: ducad_sketch::PolygonMode::Inscribed,

            slot_mode: ducad_sketch::SlotMode::CenterToCenter,
            slot_width: 10.0,

            pattern_kind: ducad_ui::PatternKind::Linear,
            pattern_count_x: 3,
            pattern_pitch_x: 20.0,
            pattern_count_y: 2,
            pattern_pitch_y: 20.0,
            pattern_count_z: 1,
            pattern_pitch_z: 20.0,
            pattern_circ_count: 6,
            pattern_circ_angle_deg: 360.0,
            pattern_circ_radius: 30.0,
            pattern_circ_axis: ducad_ui::PatternAxisPreset::Z,
            pattern_custom_pivot_2d: None,
            pattern_custom_pivot_3d: None,
            pattern_dimension_editing_x: false,
            pattern_dimension_editing_y: false,
            pattern_dimension_editing_z: false,
            pattern_dimension_editing_angle: false,
            pattern_dimension_editing_radius: false,
            pattern_dimension_edit_input: "20.0".to_string(),

            rib_thickness_input: "2.0".to_string(),
            rib_depth_input: "15.0".to_string(),
            rib_draft_input: "0.0".to_string(),
            rib_angle_input: "0.0".to_string(),
            rib_start_pt: None,
            rib_end_pt: None,
            rib_normal_dir: glam::dvec3(0.0, 0.0, -1.0),

            shell_variable_faces: Vec::new(),
            shell_var_thickness_input: "4.0".to_string(),
            shell_is_variable_mode: false,

            drawing_sheet_state: ducad_ui::DrawingSheetViewState::default(),
            drawing_sheet_doc: None,
            hole_popup_state: ducad_ui::HolePopupState::default(),
            editing_hole_ruler_idx: None,
            editing_hole_ruler_input: String::new(),
            text_popup_state: ducad_ui::TextPopupState::default(),
            custom_font_bytes: None,
            helix_popup_state: ducad_ui::HelixPopupState::default(),
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

        self.sketches = vec![loaded.sketch, loaded.front_sketch, loaded.right_sketch];
        self.undos = vec![
            ducad_sketch::UndoStack::default(),
            ducad_sketch::UndoStack::default(),
            ducad_sketch::UndoStack::default(),
        ];
        self.datum_planes.clear();
        self.datum_plane_counter = 0;
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
        let allow_primary_orbit = matches!(
            self.tool,
            ToolKind::Select
                | ToolKind::SplitBody
                | ToolKind::DraftAngle
                | ToolKind::Boolean
                | ToolKind::SectionView
                | ToolKind::Measure
                | ToolKind::MeasureAngle
                | ToolKind::History
                | ToolKind::Shell
                | ToolKind::Rib
        ) && !radial_active
            && !is_near_gizmo
            && !self.extruding_from_gizmo
            && !self.extruding_face_from_gizmo;
        self.handle_navigation(ui, &response, rect, allow_primary_orbit);
        self.handle_sketch_input(ui, &response, rect, raw_cursor);

        let aspect = rect.width() / rect.height().max(1.0);
        let world_scale = pixel_tolerance_to_world(&self.camera, rect);
        let overlay = self.build_overlay_lines(raw_cursor, world_scale);
        let (body_positions, body_normals, body_colors, body_materials, body_indices) =
            self.build_combined_body_mesh();

        // Hitung bounding box otomatis untuk level lantai (ground_z) dan proyeksi bayangan kontak studio
        if !body_positions.is_empty() {
            let mut min_x = f32::MAX;
            let mut max_x = f32::MIN;
            let mut min_y = f32::MAX;
            let mut max_y = f32::MIN;
            let mut min_z = f32::MAX;
            for p in &body_positions {
                min_x = min_x.min(p[0]);
                max_x = max_x.max(p[0]);
                min_y = min_y.min(p[1]);
                max_y = max_y.max(p[1]);
                min_z = min_z.min(p[2]);
            }
            self.studio_config.ground_z = min_z;
            self.studio_config.shadow_center = [(min_x + max_x) * 0.5, (min_y + max_y) * 0.5];
            let rx = ((max_x - min_x) * 0.5).max(20.0);
            let ry = ((max_y - min_y) * 0.5).max(20.0);
            self.studio_config.shadow_radius = [rx * 1.3, ry * 1.3];
        }

        let (gizmo_positions, gizmo_normals, gizmo_colors, gizmo_indices) =
            self.build_gizmo_mesh(world_scale);
        let grid_extent = self.calculate_grid_extent(raw_cursor);
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            rect,
            ViewportCallback {
                view_proj: self.camera.view_proj(aspect),
                eye: self.camera.eye(),
                sketch_plane: self.active_plane,
                grid_extent,
                overlay_lines: overlay,
                body_positions,
                body_normals,
                body_colors,
                body_materials,
                body_indices,
                gizmo_positions,
                gizmo_normals,
                gizmo_colors,
                gizmo_indices,
                clip_plane: self.section_clip_plane(),
                studio_config: self.studio_config,
                zebra_config: self.zebra_config,
                draft_config: self.draft_config,
            },
        ));

        self.dynamic_input_ui(ui, rect, raw_cursor);
    }

    /// Hitung luas grid yang fleksibel & adaptif:
    /// - Minimal 500.0 unit bawaan.
    /// - Mengembang otomatis bila ada entitas sketch, pending drawing points, atau kursor yang melebihi 500.0.
    /// - Mengembang sesuai jarak kamera / zoom-out agar grid tidak terpotong.
    /// - Diberi padding aman dan dibulatkan ke kelipatan 100.0 (major grid interval) agar garis grid tetap rapi.
    pub fn calculate_grid_extent(&self, cursor_plane_pt: Option<DVec2>) -> f32 {
        calculate_grid_extent_for_params(
            self.sketch(),
            &self.pending_points,
            self.is_sketching,
            cursor_plane_pt,
            self.camera.target,
            self.camera.distance,
        )
    }
}

/// Helper murni untuk kalkulasi grid extent dinamis (bisa diuji tanpa eframe context).
pub fn calculate_grid_extent_for_params(
    sketch: &Sketch,
    pending_points: &[DVec2],
    is_sketching: bool,
    cursor_plane_pt: Option<DVec2>,
    cam_target: Vec3,
    cam_distance: f32,
) -> f32 {
    let mut max_coord = 500.0f64;

    // 1. Periksa seluruh entitas sketch
    if let Some((min_pt, max_pt)) = sketch.bounding_box() {
        max_coord = max_coord
            .max(min_pt.x.abs())
            .max(max_pt.x.abs())
            .max(min_pt.y.abs())
            .max(max_pt.y.abs());
    }

    // 2. Periksa pending points saat menggambar (misal multi-point spline, polyline, pattern, dll)
    for pt in pending_points {
        max_coord = max_coord.max(pt.x.abs()).max(pt.y.abs());
    }

    // 3. Periksa posisi kursor saat dalam mode sketch aktif
    if is_sketching {
        if let Some(p) = cursor_plane_pt {
            max_coord = max_coord.max(p.x.abs()).max(p.y.abs());
        }
    }

    // 4. Periksa jarak pandang kamera (zoom-out & pan)
    let cam_dist = cam_distance as f64;
    let cam_target_dist = cam_target.length() as f64;
    max_coord = max_coord.max(cam_target_dist + cam_dist * 0.75);

    // 5. Beri margin 100 unit dan bulatkan ke kelipatan major grid (100.0)
    let padded = max_coord + 100.0;
    let step = 100.0;
    let extent = (padded / step).ceil() * step;

    extent.clamp(500.0, 50_000.0) as f32
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

        let z_pressed = ctx.input(|i| {
            !i.modifiers.command
                && !i.modifiers.alt
                && !i.modifiers.ctrl
                && !i.modifiers.shift
                && i.key_pressed(egui::Key::Z)
        });
        if z_pressed && !self.is_sketching && self.editing_dimension_entity.is_none() {
            self.zebra_config.enabled = !self.zebra_config.enabled;
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                self.viewport(ui);
            });

        let screen_rect = ctx.content_rect();

        // Mate HUD harus di-render di luar CentralPanel agar tidak terblokir oleh
        // Sense::click_and_drag yang dialokasikan oleh viewport(). Ini memastikan
        // klik pada tombol Apply Mate, Toggle Lock, dan Flip selalu terdeteksi.
        self.show_mate_hud_ctx(&ctx, screen_rect);

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
            assembly_drawer_open: self.assembly_drawer_open,
            section_view_active: self.section_enabled,
            is_measure_active: self.show_all_dimensions,
            zebra_view_active: self.zebra_config.enabled,
            studio_lighting_active: self.studio_config.enabled,
            active_plane_name: match self.active_plane.kind {
                PlaneKind::Custom(id) => self
                    .datum_planes
                    .iter()
                    .find(|dp| dp.id == id)
                    .map(|dp| dp.name.clone())
                    .unwrap_or_else(|| self.active_plane.name().to_string()),
                _ => self.active_plane.name().to_string(),
            },
            custom_planes: self
                .datum_planes
                .iter()
                .enumerate()
                .map(|(i, dp)| (i + 3, dp.name.clone()))
                .collect(),
            plane_menu_open: self.plane_menu_open,
            items_button_rect: egui::Rect::NOTHING,
        };

        if !self.drawing_sheet_state.is_open {
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
                            TopBarEvent::SetLanguage(lang) => {
                                self.language = lang;
                                ducad_i18n::set_language(lang);
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
                                TopBarFileOp::ExportGlb => self.export_glb(),
                                TopBarFileOp::ExportDxf => self.export_dxf(),
                                TopBarFileOp::ExportSvg => self.export_sketch_svg(),
                                TopBarFileOp::ExportPdf => self.export_drawing_pdf(),
                                TopBarFileOp::ExportDrawingDxf => self.export_drawing_dxf(),
                                TopBarFileOp::ExportDrawingSvg => self.export_drawing_svg(),
                                TopBarFileOp::OpenDrawingSheet => self.open_drawing_sheet(),
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
                            TopBarEvent::ToggleAssemblyDrawer => {
                                self.assembly_drawer_open = !self.assembly_drawer_open;
                            }
                            TopBarEvent::OpenSearch => {
                                self.palette.open();
                            }
                            TopBarEvent::OpenDrawingSheet => {
                                self.open_drawing_sheet();
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
                                self.set_sketch_plane_by_index(idx);
                            }
                            TopBarEvent::CreateDatumPlane => {
                                self.set_tool(ToolKind::DatumPlane);
                            }
                            TopBarEvent::TogglePlanesDrawer => {
                                self.planes_drawer_open = !self.planes_drawer_open;
                            }
                            TopBarEvent::ToggleSectionView => {
                                self.section_enabled = !self.section_enabled;
                            }
                            TopBarEvent::ToggleMeasurements => {
                                self.show_all_dimensions = !self.show_all_dimensions;
                            }
                            TopBarEvent::ToggleZebraView => {
                                self.zebra_config.enabled = !self.zebra_config.enabled;
                            }
                            TopBarEvent::ToggleStudioLighting => {
                                self.lighting_drawer_open = !self.lighting_drawer_open;
                                self.studio_config.enabled = self.lighting_drawer_open;
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
                            ToolbarEvent::SelectTool(ducad_ui::ToolbarTool::ZebraInspection) => {
                                self.zebra_config.enabled = !self.zebra_config.enabled;
                            }
                            ToolbarEvent::SelectTool(ducad_ui::ToolbarTool::DraftAnalysis) => {
                                self.draft_config.enabled = !self.draft_config.enabled;
                            }
                            ToolbarEvent::SelectTool(t) => {
                                let kind = ToolKind::from_toolbar_tool(t);
                                match kind {
                                    ToolKind::Shell | ToolKind::DraftAngle | ToolKind::SplitBody | ToolKind::Rib => {
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
        }

        let entities_2d: Vec<Entity2dItemInfo> = self
            .sketch()
            .entities
            .iter()
            .map(|(id, entity)| {
                let (name, icon_str) = match entity {
                    Entity::Line { start, end, .. } => {
                        let len = start.distance(*end);
                        let label = if entity.is_construction() {
                            format!("Garis [Konstruksi] ({:.1} mm)", len)
                        } else {
                            format!("Garis ({:.1} mm)", len)
                        };
                        (
                            label,
                            egui_material_icons::icons::ICON_HORIZONTAL_RULE.codepoint,
                        )
                    }
                    Entity::Circle { radius, .. } => {
                        let label = if entity.is_construction() {
                            format!("Lingkaran [Konstruksi] (R: {:.1} mm)", radius)
                        } else {
                            format!("Lingkaran (R: {:.1} mm)", radius)
                        };
                        (
                            label,
                            egui_material_icons::icons::ICON_CIRCLE.codepoint,
                        )
                    }
                    Entity::Arc { radius, .. } => {
                        let label = if entity.is_construction() {
                            format!("Busur [Konstruksi] (R: {:.1} mm)", radius)
                        } else {
                            format!("Busur (R: {:.1} mm)", radius)
                        };
                        (
                            label,
                            egui_material_icons::icons::ICON_ARCHITECTURE.codepoint,
                        )
                    }
                    Entity::Ellipse {
                        radius_x, radius_y, ..
                    } => {
                        let label = if entity.is_construction() {
                            format!("Elips [Konstruksi] ({:.1}x{:.1} mm)", radius_x, radius_y)
                        } else {
                            format!("Elips ({:.1}x{:.1} mm)", radius_x, radius_y)
                        };
                        (
                            label,
                            egui_material_icons::icons::ICON_HOME_MINI.codepoint,
                        )
                    }
                    Entity::Spline { points, .. } => {
                        let label = if entity.is_construction() {
                            format!("Spline [Konstruksi] ({} titik)", points.len())
                        } else {
                            format!("Spline ({} titik)", points.len())
                        };
                        (
                            label,
                            egui_material_icons::icons::ICON_TIMELINE.codepoint,
                        )
                    }
                };
                let group_name = self.sketch().entity_names.get(&id).cloned().or_else(|| {
                    if let Entity::Spline { points, .. } = entity {
                        if points.len() >= 3 {
                            let first = points[0];
                            let last = points.last().unwrap();
                            if (first - *last).length_squared() < 1e-4 {
                                return Some("Teks 2D".to_string());
                            }
                        }
                    }
                    None
                });
                Entity2dItemInfo {
                    id_raw: id.data().as_ffi(),
                    name,
                    icon: icon_str,
                    visible: self.sketch().is_visible(id),
                    selected: self.selected.contains(&id),
                    group_name,
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
                material: b.material,
            })
            .collect();

        let has_tool_popup = matches!(self.tool, ToolKind::Helix | ToolKind::Text | ToolKind::HoleWizard);
        let open_drawers_count = (self.items_drawer_open as usize)
            + (self.history_drawer_open as usize)
            + (self.planes_drawer_open as usize)
            + (self.cmf_drawer_open as usize)
            + (self.lighting_drawer_open as usize)
            + (self.draft_config.enabled as usize)
            + (has_tool_popup as usize);
        let screen_avail_h = (screen_rect.height() - 140.0).max(300.0);
        let max_drawer_h = if open_drawers_count > 1 {
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
                            ItemsDrawerEvent::ToggleEntity2dVisibility(raw_id) => {
                                let mut found = false;
                                for sketch in self.sketches.iter_mut() {
                                    if let Some(id) = sketch.entities.keys().find(|i| i.data().as_ffi() == raw_id) {
                                        let is_now_visible = sketch.toggle_visibility(id);
                                        if !is_now_visible {
                                            self.selected.remove(&id);
                                            if self.hovered == Some(id) {
                                                self.hovered = None;
                                            }
                                        }
                                        let name = sketch.entity_names.get(&id).cloned().unwrap_or_else(|| "Objek 2D".to_string());
                                        self.model_status = Some(format!(
                                            "Objek 2D '{}' {}",
                                            name,
                                            if is_now_visible { "ditampilkan" } else { "disembunyikan" }
                                        ));
                                        found = true;
                                        ctx.request_repaint();
                                        break;
                                    }
                                }
                                if !found {
                                    println!("[DUCAD APP] Entity 2D dengan raw_id: {} tidak ditemukan!", raw_id);
                                }
                            }
                            ItemsDrawerEvent::ToggleGroupVisibility(group_name) => {
                                for sketch in self.sketches.iter_mut() {
                                    let member_ids: Vec<_> = sketch
                                        .entity_names
                                        .iter()
                                        .filter(|(_, name)| *name == &group_name)
                                        .map(|(id, _)| *id)
                                        .collect();
                                    if !member_ids.is_empty() {
                                        let any_visible = member_ids.iter().any(|id| sketch.is_visible(*id));
                                        for id in &member_ids {
                                            sketch.set_visible(*id, !any_visible);
                                            if any_visible {
                                                self.selected.remove(id);
                                                if self.hovered == Some(*id) {
                                                    self.hovered = None;
                                                }
                                            }
                                        }
                                        self.model_status = Some(format!(
                                            "Grup '{}' {}",
                                            group_name,
                                            if !any_visible { "ditampilkan" } else { "disembunyikan" }
                                        ));
                                        ctx.request_repaint();
                                        break;
                                    }
                                }
                            }
                            ItemsDrawerEvent::SelectBody { id_raw, extend } => {
                                for (id, body) in self.model.doc.bodies.iter() {
                                    if id.data().as_ffi() == id_raw && body.visible {
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
                                    if self.sketch().is_visible(id) {
                                        let target_ids: Vec<_> = if let Some(g_name) = self.sketch().entity_names.get(&id) {
                                            self.sketch()
                                                .entities
                                                .keys()
                                                .filter(|k| self.sketch().entity_names.get(k) == Some(g_name))
                                                .collect()
                                        } else {
                                            vec![id]
                                        };
                                        if !extend {
                                            self.selected.clear();
                                            for tid in target_ids {
                                                self.selected.insert(tid);
                                            }
                                        } else {
                                            let any_present = target_ids.iter().any(|tid| self.selected.contains(tid));
                                            if any_present {
                                                for tid in &target_ids {
                                                    self.selected.remove(tid);
                                                }
                                            } else {
                                                for tid in target_ids {
                                                    self.selected.insert(tid);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            ItemsDrawerEvent::SelectGroup { name, extend } => {
                                let matching_ids: Vec<_> = self
                                    .sketch()
                                    .entities
                                    .keys()
                                    .filter(|id| self.sketch().entity_names.get(id).map(|s| s.as_str()) == Some(&name))
                                    .collect();
                                if !extend {
                                    self.selected.clear();
                                    for id in matching_ids {
                                        self.selected.insert(id);
                                    }
                                } else {
                                    let any_present = matching_ids.iter().any(|id| self.selected.contains(id));
                                    if any_present {
                                        for id in &matching_ids {
                                            self.selected.remove(id);
                                        }
                                    } else {
                                        for id in matching_ids {
                                            self.selected.insert(id);
                                        }
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

        // 2. Reference Planes Drawer (Pojok Kanan Bawah)
        let mut planes_top_y = None;
        if self.planes_drawer_open {
            let planes_bottom_y = if self.items_drawer_open {
                folder_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
            } else {
                folder_bottom_y
            };
            let planes_pos = egui::pos2(screen_rect.max.x - 16.0, planes_bottom_y);

            let all_planes_info: Vec<PlaneItemInfo> = self
                .all_planes()
                .iter()
                .map(|(idx, _, name)| {
                    let is_custom = *idx >= 3;
                    let custom_id = if is_custom {
                        self.datum_planes.get(*idx - 3).map(|p| p.id)
                    } else {
                        None
                    };
                    PlaneItemInfo {
                        index: *idx,
                        name: name.clone(),
                        is_active: *idx == self.active_plane_index(),
                        is_custom,
                        custom_id,
                    }
                })
                .collect();

            let planes_area_resp = egui::Area::new(egui::Id::new("ducad-planes-drawer-area"))
                .fixed_pos(planes_pos)
                .pivot(egui::Align2::RIGHT_BOTTOM)
                .order(egui::Order::Foreground)
                .show(&ctx, |ui| {
                    if let Some(ev) = self.planes_drawer.show(
                        ui,
                        &all_planes_info,
                        max_drawer_h,
                        planes_bottom_y,
                    ) {
                        match ev {
                            PlanesDrawerEvent::SelectPlane(idx) => {
                                self.set_sketch_plane_by_index(idx);
                                let label = all_planes_info
                                    .iter()
                                    .find(|p| p.index == idx)
                                    .map(|p| p.name.as_str())
                                    .unwrap_or("Plane");
                                self.model_status = Some(format!(
                                    "Bidang '{}' kini aktif untuk sketsa",
                                    label
                                ));
                                ctx.request_repaint();
                            }
                            PlanesDrawerEvent::DeletePlane(custom_id) => {
                                self.delete_datum_plane(custom_id);
                                self.model_status = Some(format!(
                                    "Bidang referensi kustom #{} dihapus",
                                    custom_id
                                ));
                                ctx.request_repaint();
                            }
                            PlanesDrawerEvent::CreateNewPlane => {
                                self.set_tool(ToolKind::DatumPlane);
                            }
                            PlanesDrawerEvent::Close => {
                                self.planes_drawer_open = false;
                            }
                        }
                    }
                });

            planes_top_y = Some(planes_area_resp.response.rect.min.y);
        }

        // 3. Unified Feature Tree & History Drawer (Pojok Kanan Bawah)
        let mut feat_tree_top_y = None;
        if self.feature_tree_drawer_open {
            let feat_bottom_y = if self.planes_drawer_open {
                planes_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
            } else if self.items_drawer_open {
                folder_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
            } else {
                folder_bottom_y
            };
            let feat_pos = egui::pos2(screen_rect.max.x - 16.0, feat_bottom_y);

            let feat_area_resp = egui::Area::new(egui::Id::new("ducad-feature-tree-drawer-area"))
                .fixed_pos(feat_pos)
                .pivot(egui::Align2::RIGHT_BOTTOM)
                .order(egui::Order::Foreground)
                .show(&ctx, |ui| {
                    let needs_regen = self.parametric_dag.needs_regeneration();
                    if let Some(ev) = self.feature_tree_drawer.show(
                        ui,
                        &self.parametric_dag.nodes,
                        &self.activity_cache,
                        needs_regen,
                        max_drawer_h,
                        feat_bottom_y,
                    ) {
                        match ev {
                            FeatureTreeEvent::Regenerate => {
                                match self.regenerate_parametric_model() {
                                    Ok(()) => {
                                        self.record_activity(
                                            ActivityKindUi::Solid3D,
                                            "Regenerasi Model Parametrik",
                                            "Rekonstruksi bodi solid 3D dari pohon fitur",
                                        );
                                    }
                                    Err(e) => {
                                        self.model_status = Some(format!("Regenerasi gagal: {e}"));
                                    }
                                }
                                ctx.request_repaint();
                            }
                            FeatureTreeEvent::SelectFeature(id) => {
                                self.model_status = Some(format!("Fitur #{id} terpilih"));
                                ctx.request_repaint();
                            }
                            FeatureTreeEvent::ToggleSuppress(id) => {
                                self.parametric_dag.toggle_suppress(id);
                                let _ = self.regenerate_parametric_model();
                                self.record_activity(
                                    ActivityKindUi::Solid3D,
                                    &format!("Toggle Suppress Fitur #{id}"),
                                    "",
                                );
                                ctx.request_repaint();
                            }
                            FeatureTreeEvent::DeleteFeature(id) => {
                                self.parametric_dag.remove_feature(id);
                                let _ = self.regenerate_parametric_model();
                                self.record_activity(
                                    ActivityKindUi::Solid3D,
                                    &format!("Hapus Fitur #{id}"),
                                    "",
                                );
                                ctx.request_repaint();
                            }
                            FeatureTreeEvent::SaveFeatureParams { id, val1, val2 } => {
                                match self.save_feature_params_and_regenerate(id, val1, val2) {
                                    Ok(()) => {
                                        let details = if let Some(v2) = val2 {
                                            format!("Nilai baru: {:.1} × {:.1} mm", val1, v2)
                                        } else {
                                            format!("Nilai baru: {:.1} mm", val1)
                                        };
                                        self.record_activity(
                                            ActivityKindUi::Solid3D,
                                            &format!("Edit Parametrik: Fitur #{id}"),
                                            &details,
                                        );
                                    }
                                    Err(e) => {
                                        self.model_status = Some(format!("Gagal update parameter: {e}"));
                                    }
                                }
                                ctx.request_repaint();
                            }
                            FeatureTreeEvent::JumpToHistory { id, timestamp, action } => {
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
                                ctx.request_repaint();
                            }
                            FeatureTreeEvent::ClearHistory => {
                                self.history_db.clear();
                                self.activity_cache.clear();
                                ctx.request_repaint();
                            }
                            FeatureTreeEvent::Close => {
                                self.feature_tree_drawer_open = false;
                            }
                        }
                    }
                });

            feat_tree_top_y = Some(feat_area_resp.response.rect.min.y);
        }

        // 3.5. Assembly Tree & Mate Constraints Drawer (Pojok Kanan Bawah)
        let mut assem_top_y = None;
        if self.assembly_drawer_open {
            self.sync_assembly_instances();
            let assem_bottom_y = if self.feature_tree_drawer_open {
                feat_tree_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
            } else if self.planes_drawer_open {
                planes_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
            } else if self.items_drawer_open {
                folder_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
            } else {
                folder_bottom_y
            };
            let assem_pos = egui::pos2(screen_rect.max.x - 16.0, assem_bottom_y);

            let assem_area_resp = egui::Area::new(egui::Id::new("ducad-assembly-drawer-area"))
                .fixed_pos(assem_pos)
                .pivot(egui::Align2::RIGHT_BOTTOM)
                .order(egui::Order::Foreground)
                .show(&ctx, |ui| {
                    let evs = self.assembly_drawer.show(
                        ui,
                        &self.assembly_tree,
                        self.selected_assembly_instance,
                        self.selected_assembly_mate,
                    );
                    for ev in evs {
                        match ev {
                            AssemblyDrawerEvent::SelectInstance(id) => {
                                self.selected_assembly_instance = Some(id);
                                if let Some(inst) = self.assembly_tree.instances.get(&id) {
                                    self.selected_bodies.clear();
                                    if let Some(bid) = self.model.doc.bodies.iter().find_map(|(b, _)| {
                                        if b.data().as_ffi() == inst.body_id_raw {
                                            Some(b)
                                        } else {
                                            None
                                        }
                                    }) {
                                        self.selected_bodies.insert(bid);
                                    }
                                }
                                ctx.request_repaint();
                            }
                            AssemblyDrawerEvent::ToggleGrounded(id) => {
                                if let Some(inst) = self.assembly_tree.instances.get(&id) {
                                    let new_grounded = !inst.is_grounded;
                                    self.assembly_tree.set_grounded(id, new_grounded);
                                    self.solve_and_apply_assembly();
                                }
                                ctx.request_repaint();
                            }
                            AssemblyDrawerEvent::ToggleInstanceVisibility(id) => {
                                if let Some(inst) = self.assembly_tree.instances.get_mut(&id) {
                                    inst.visible = !inst.visible;
                                }
                                ctx.request_repaint();
                            }
                            AssemblyDrawerEvent::DeleteInstance(id) => {
                                self.assembly_tree.remove_instance(id);
                                ctx.request_repaint();
                            }
                            AssemblyDrawerEvent::AddSubAssembly => {
                                self.assembly_tree.add_sub_assembly("Sub-Assembly", None);
                                ctx.request_repaint();
                            }
                            AssemblyDrawerEvent::DeleteSubAssembly(id) => {
                                self.assembly_tree.remove_sub_assembly(id);
                                ctx.request_repaint();
                            }
                            AssemblyDrawerEvent::SelectMate(id) => {
                                self.selected_assembly_mate = Some(id);
                                ctx.request_repaint();
                            }
                            AssemblyDrawerEvent::ToggleSuppressMate(id) => {
                                self.assembly_tree.toggle_suppress_mate(id);
                                self.solve_and_apply_assembly();
                                ctx.request_repaint();
                            }
                            AssemblyDrawerEvent::DeleteMate(id) => {
                                self.assembly_tree.remove_mate(id);
                                ctx.request_repaint();
                            }
                            AssemblyDrawerEvent::UpdateMateParam { id, val, flip } => {
                                if let Some(mate) = self.assembly_tree.mates.get_mut(&id) {
                                    match &mut mate.kind {
                                        ducad_core::MateKind::Distance {
                                            offset,
                                            opposite_normal,
                                        } => {
                                            *offset = val;
                                            *opposite_normal = flip;
                                        }
                                        ducad_core::MateKind::Angle {
                                            angle_deg,
                                            opposite_normal,
                                        } => {
                                            *angle_deg = val;
                                            *opposite_normal = flip;
                                        }
                                        ducad_core::MateKind::Concentric { aligned, .. } => {
                                            *aligned = flip;
                                        }
                                        ducad_core::MateKind::Coincident { opposite_normal } => {
                                            *opposite_normal = flip;
                                        }
                                    }
                                    self.solve_and_apply_assembly();
                                }
                                ctx.request_repaint();
                            }
                            AssemblyDrawerEvent::SolveAssembly => {
                                self.solve_and_apply_assembly();
                                self.model_status =
                                    Some("Pohon perakitan berhasil dihitung ulang".to_string());
                                ctx.request_repaint();
                            }
                            AssemblyDrawerEvent::Close => {
                                self.assembly_drawer_open = false;
                            }
                        }
                    }
                });

            assem_top_y = Some(assem_area_resp.response.rect.min.y);
        }

        // 4. Draft Analysis Inspector Window (Tersusun rapi di atas Folder/Planes/FeatureTree/Assembly drawer)
        let mut draft_top_y = None;
        if self.draft_config.enabled {
            let draft_bottom_y = if self.assembly_drawer_open {
                assem_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
            } else if self.feature_tree_drawer_open {
                feat_tree_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
            } else if self.planes_drawer_open {
                planes_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
            } else if self.items_drawer_open {
                folder_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
            } else {
                folder_bottom_y
            };
            let draft_pos = egui::pos2(screen_rect.max.x - 16.0, draft_bottom_y);

            let mut draft_state = ducad_ui::DraftPopupState {
                pull_dir: self.draft_config.pull_dir,
                target_angle_deg: self.draft_config.target_angle_deg,
                blend: self.draft_config.blend,
            };

            let draft_area_resp = egui::Area::new(egui::Id::new("ducad-draft-heatmap-area"))
                .fixed_pos(draft_pos)
                .pivot(egui::Align2::RIGHT_BOTTOM)
                .order(egui::Order::Foreground)
                .show(&ctx, |ui| {
                    if let Some(ev) = DraftAnalysisPopup::show(ui, &mut draft_state) {
                        match ev {
                            ToolPopupEvent::UpdateDraftInspection {
                                pull_dir,
                                target_angle_deg,
                                blend,
                            } => {
                                self.draft_config.pull_dir = pull_dir;
                                self.draft_config.target_angle_deg = target_angle_deg;
                                self.draft_config.blend = blend;
                            }
                            ToolPopupEvent::CloseDraftInspection | ToolPopupEvent::Close => {
                                self.draft_config.enabled = false;
                            }
                            _ => {}
                        }
                    }
                });

            draft_top_y = Some(draft_area_resp.response.rect.min.y);
        }

        // 5. Preset Material Industri & CMF Drawer (Pojok Kanan Bawah)
        let mut cmf_top_y = None;
        if self.cmf_drawer_open {
            let cmf_bottom_y = if self.draft_config.enabled {
                draft_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
            } else if self.assembly_drawer_open {
                assem_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
            } else if self.feature_tree_drawer_open {
                feat_tree_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
            } else if self.planes_drawer_open {
                planes_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
            } else if self.items_drawer_open {
                folder_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
            } else {
                folder_bottom_y
            };
            let cmf_pos = egui::pos2(screen_rect.max.x - 16.0, cmf_bottom_y);

            let (cur_mat, body_name, count) = if let Some(&bid) = self.selected_bodies.iter().next() {
                if let Some(b) = self.model.doc.bodies.get(bid) {
                    (b.material, Some(b.name.clone()), self.selected_bodies.len())
                } else {
                    (ducad_core::Material::default(), None, 0)
                }
            } else if let Some((_, b)) = self.model.doc.bodies.iter().next() {
                (b.material, Some(b.name.clone()), 0)
            } else {
                (ducad_core::Material::default(), None, 0)
            };

            let cmf_area_resp = egui::Area::new(egui::Id::new("ducad-cmf-drawer-area"))
                .fixed_pos(cmf_pos)
                .pivot(egui::Align2::RIGHT_BOTTOM)
                .order(egui::Order::Foreground)
                .show(&ctx, |ui| {
                    if let Some(ev) = self.cmf_drawer.show(
                        ui,
                        &cur_mat,
                        body_name.as_deref(),
                        count,
                        max_drawer_h,
                    ) {
                        match ev {
                            CmfDrawerEvent::SetMaterial(new_mat) => {
                                let targets: Vec<_> = if !self.selected_bodies.is_empty() {
                                    self.selected_bodies.iter().copied().collect()
                                } else if let Some((bid, _)) = self.model.doc.bodies.iter().next() {
                                    vec![bid]
                                } else {
                                    Vec::new()
                                };

                                for bid in targets {
                                    let cmd = crate::model::SetBodyMaterialCommand::new("Ubah Material", bid, new_mat);
                                    self.model_undo.execute(Box::new(cmd), &mut self.model);
                                }
                                self.model_status = Some("✓ Preset Material diterapkan".to_string());
                                ctx.request_repaint();
                            }
                            CmfDrawerEvent::Close => {
                                self.cmf_drawer_open = false;
                            }
                        }
                    }
                });

            cmf_top_y = Some(cmf_area_resp.response.rect.min.y);
        }

        // 6. Studio Lighting & SSAO Drawer (Pojok Kanan Bawah)
        let mut lighting_top_y = None;
        if self.lighting_drawer_open {
            let lighting_bottom_y = if self.cmf_drawer_open {
                cmf_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
            } else if self.draft_config.enabled {
                draft_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
            } else if self.assembly_drawer_open {
                assem_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
            } else if self.feature_tree_drawer_open {
                feat_tree_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
            } else if self.planes_drawer_open {
                planes_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
            } else if self.items_drawer_open {
                folder_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
            } else {
                folder_bottom_y
            };
            let lighting_pos = egui::pos2(screen_rect.max.x - 16.0, lighting_bottom_y);

            let preset_ui = match self.studio_config.preset {
                ducad_render::StudioPreset::CleanStudio => ducad_ui::StudioLightingPresetUi::CleanStudio,
                ducad_render::StudioPreset::WarmShowcase => ducad_ui::StudioLightingPresetUi::WarmShowcase,
                ducad_render::StudioPreset::CoolTech => ducad_ui::StudioLightingPresetUi::CoolTech,
                ducad_render::StudioPreset::DramaticDark => ducad_ui::StudioLightingPresetUi::DramaticDark,
            };

            let lighting_area_resp = egui::Area::new(egui::Id::new("ducad-lighting-drawer-area"))
                .fixed_pos(lighting_pos)
                .pivot(egui::Align2::RIGHT_BOTTOM)
                .order(egui::Order::Foreground)
                .show(&ctx, |ui| {
                    if let Some(ev) = self.lighting_drawer.show(
                        ui,
                        preset_ui,
                        self.studio_config.ssao_intensity,
                        self.studio_config.floor_shadow_enabled,
                        self.studio_config.floor_shadow_intensity,
                        self.studio_config.key_intensity,
                        self.studio_config.fill_intensity,
                        self.studio_config.rim_intensity,
                        max_drawer_h,
                    ) {
                        match ev {
                            ducad_ui::LightingDrawerEvent::SetPreset(p) => {
                                self.studio_config.preset = match p {
                                    ducad_ui::StudioLightingPresetUi::CleanStudio => {
                                        ducad_render::StudioPreset::CleanStudio
                                    }
                                    ducad_ui::StudioLightingPresetUi::WarmShowcase => {
                                        ducad_render::StudioPreset::WarmShowcase
                                    }
                                    ducad_ui::StudioLightingPresetUi::CoolTech => {
                                        ducad_render::StudioPreset::CoolTech
                                    }
                                    ducad_ui::StudioLightingPresetUi::DramaticDark => {
                                        ducad_render::StudioPreset::DramaticDark
                                    }
                                };
                            }
                            ducad_ui::LightingDrawerEvent::SetKeyIntensity(k) => {
                                self.studio_config.key_intensity = k;
                            }
                            ducad_ui::LightingDrawerEvent::SetFillIntensity(f) => {
                                self.studio_config.fill_intensity = f;
                            }
                            ducad_ui::LightingDrawerEvent::SetRimIntensity(r) => {
                                self.studio_config.rim_intensity = r;
                            }
                            ducad_ui::LightingDrawerEvent::SetSsaoIntensity(s) => {
                                self.studio_config.ssao_intensity = s;
                            }
                            ducad_ui::LightingDrawerEvent::ToggleFloorShadow => {
                                self.studio_config.floor_shadow_enabled =
                                    !self.studio_config.floor_shadow_enabled;
                            }
                            ducad_ui::LightingDrawerEvent::SetFloorShadowIntensity(si) => {
                                self.studio_config.floor_shadow_intensity = si;
                            }
                            ducad_ui::LightingDrawerEvent::Close => {
                                self.lighting_drawer_open = false;
                                self.studio_config.enabled = false;
                            }
                        }
                    }
                });

            lighting_top_y = Some(lighting_area_resp.response.rect.min.y);
        }

        // Posisi anchor bottom untuk Tool Popups (Helix, Text, Hole Wizard, dll) yang tersusun rapi di atas drawer aktif
        let tool_popup_bottom_y = if self.lighting_drawer_open {
            lighting_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
        } else if self.cmf_drawer_open {
            cmf_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
        } else if self.draft_config.enabled {
            draft_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
        } else if self.feature_tree_drawer_open {
            feat_tree_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
        } else if self.planes_drawer_open {
            planes_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
        } else if self.items_drawer_open {
            folder_top_y.unwrap_or(folder_bottom_y - 200.0) - 8.0
        } else {
            folder_bottom_y
        };

        // 2. Floating Buttons Bar di Pojok Kanan Bawah (Lighting, CMF Material, Draft Analysis, History, Folder)
        if !self.drawing_sheet_state.is_open {
            let btns_pos = egui::pos2(screen_rect.max.x - 16.0, screen_rect.max.y - 16.0);
            egui::Area::new(egui::Id::new("ducad-bottom-right-floating-btns"))
                .fixed_pos(btns_pos)
                .pivot(egui::Align2::RIGHT_BOTTOM)
                .order(egui::Order::Foreground)
                .show(&ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);

                        // Tombol Studio Lighting & SSAO
                        let lighting_resp = round_floating_icon_btn(
                            ui,
                            egui_material_icons::icons::ICON_LIGHTBULB_ON.codepoint,
                            self.lighting_drawer_open,
                            "Studio Lighting & Bayangan Kontak Lantai (SSAO)",
                        );
                        if lighting_resp.clicked() {
                            self.lighting_drawer_open = !self.lighting_drawer_open;
                            self.studio_config.enabled = self.lighting_drawer_open;
                        }

                        // Tombol Preset Industri & CMF Material
                        let cmf_resp = round_floating_icon_btn(
                            ui,
                            egui_material_icons::icons::ICON_PALETTE.codepoint,
                            self.cmf_drawer_open,
                            "Preset Material Industri & CMF (Warna & Finishing)",
                        );
                        if cmf_resp.clicked() {
                            self.cmf_drawer_open = !self.cmf_drawer_open;
                        }

                        // Tombol Draft Analysis (Kiri dari History)
                        let draft_resp = round_floating_icon_btn(
                            ui,
                            egui_material_icons::icons::ICON_ARCHITECTURE.codepoint,
                            self.draft_config.enabled,
                            "Draft Analysis (Heatmap Sudut Kemiringan / Draft Angle)",
                        );
                        if draft_resp.clicked() {
                            self.draft_config.enabled = !self.draft_config.enabled;
                        }

                        // Tombol Feature Tree & Riwayat Desain (Pohon Parametrik & Snapshot)
                        let feat_tree_resp = round_floating_icon_btn(
                            ui,
                            egui_material_icons::icons::ICON_TIMELINE.codepoint,
                            self.feature_tree_drawer_open,
                            "Feature Tree & Riwayat Desain (Pohon Parametrik & Time-Travel)",
                        );
                        if feat_tree_resp.clicked() {
                            self.feature_tree_drawer_open = !self.feature_tree_drawer_open;
                        }

                        // Tombol Reference Planes (Kiri dari Folder)
                        let planes_resp = round_floating_icon_btn(
                            ui,
                            egui_material_icons::icons::ICON_LAYERS_OFF.codepoint,
                            self.planes_drawer_open,
                            "Daftar Bidang Referensi 3D (Reference Planes)",
                        );
                        if planes_resp.clicked() {
                            self.planes_drawer_open = !self.planes_drawer_open;
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
        }


        if self.section_enabled {
            if let Some(hud_ev) = CanvasHud::show_section_view_banner(ui, screen_rect) {
                if hud_ev == CanvasHudEvent::TurnOffSectionView {
                    self.section_enabled = false;
                }
            }
        }

        if self.zebra_config.enabled {
            if let Some(act) = CanvasHud::show_zebra_inspection_panel(
                ui,
                screen_rect,
                &mut self.zebra_config.frequency,
                &mut self.zebra_config.angle,
                &mut self.zebra_config.blend,
            ) {
                match act {
                    ZebraHudAction::SetFrequency(f) => self.zebra_config.frequency = f,
                    ZebraHudAction::SetAngle(a) => self.zebra_config.angle = a,
                    ZebraHudAction::SetBlend(b) => self.zebra_config.blend = b,
                    ZebraHudAction::TurnOff => self.zebra_config.enabled = false,
                }
            }
        }

        // =========================================================================
        // MODULAR TOOL POPUPS & INSPECTOR WINDOWS DI POJOK KANAN BAWAH (BOTTOM-RIGHT)
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
            ToolKind::HoleWizard => {
                // Siapkan daftar lubang yang tersedia pada bodi saat ini
                if let Some((body_id, _, _)) = &self.active_face {
                    if let Some(hist) = self.hole_history.get(body_id) {
                        self.hole_popup_state.available_holes = hist
                            .features
                            .iter()
                            .enumerate()
                            .map(|(idx, feat)| {
                                let label = format!(
                                    "#{} ({} @ {:.1}, {:.1})",
                                    idx + 1,
                                    feat.spec.technical_callout(),
                                    feat.offset_u,
                                    feat.offset_v
                                );
                                (idx, label)
                            })
                            .collect();

                        if !hist.features.is_empty() {
                            if self.hole_popup_state.selected_hole_idx.is_none()
                                || self
                                    .hole_popup_state
                                    .selected_hole_idx
                                    .is_some_and(|i| i >= hist.features.len())
                            {
                                let default_idx = self
                                    .editing_hole_idx
                                    .map(|(_, i)| i)
                                    .unwrap_or(0)
                                    .min(hist.features.len() - 1);
                                self.hole_popup_state.selected_hole_idx = Some(default_idx);
                            }
                        } else {
                            self.hole_popup_state.selected_hole_idx = None;
                        }
                    } else {
                        self.hole_popup_state.available_holes.clear();
                        self.hole_popup_state.selected_hole_idx = None;
                    }
                }

                let prev_selected = self.hole_popup_state.selected_hole_idx;

                let mut hole_custom_h = None;
                popup_ev = ducad_ui::render_bottom_right_panel_custom(
                    &ctx,
                    "hole_wizard_popup",
                    &ducad_i18n::t!("tool-hole-wizard"),
                    egui_material_icons::icons::ICON_ADJUST.codepoint,
                    ducad_ui::theme::ACCENT_BLUE,
                    screen_rect,
                    tool_popup_bottom_y,
                    max_drawer_h,
                    &mut hole_custom_h,
                    340.0,
                    280.0,
                    false,
                    |ui| {
                        let ev = ducad_ui::HolePopup::show(ui, &mut self.hole_popup_state);
                        (ev, false)
                    },
                )
                .0
                .flatten();

                // Jika pengguna mengganti pilihan lubang di combobox, sinkronkan spesifikasi dan posisinya
                if self.hole_popup_state.mode == ducad_ui::HoleOperationMode::EditHole {
                    if let Some(new_idx) = self.hole_popup_state.selected_hole_idx {
                        let needs_sync = prev_selected != Some(new_idx)
                            || self.editing_hole_idx.map(|(_, i)| i) != Some(new_idx);
                        if needs_sync {
                            if let Some((body_id, _, _)) = &self.active_face {
                                self.editing_hole_idx = Some((*body_id, new_idx));
                                if let Some(hist) = self.hole_history.get(body_id) {
                                    if let Some(feat) = hist.features.get(new_idx) {
                                        self.hole_popup_state.spec = feat.spec.clone();
                                        self.hole_popup_state.offset_u = feat.offset_u;
                                        self.hole_popup_state.offset_v = feat.offset_v;
                                        self.hole_popup_state.current_pos_3d = Some(feat.pos);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            ToolKind::Text => {
                let popup_title = ducad_i18n::t!("tool-text");
                let mut text_custom_h = None;
                popup_ev = ducad_ui::render_bottom_right_panel_custom(
                    &ctx,
                    "text_popup",
                    &popup_title,
                    egui_material_icons::icons::ICON_TITLE.codepoint,
                    ducad_ui::theme::ACCENT_BLUE,
                    screen_rect,
                    tool_popup_bottom_y,
                    max_drawer_h,
                    &mut text_custom_h,
                    320.0,
                    250.0,
                    false,
                    |ui| {
                        let ev = ducad_ui::TextPopup::show(ui, &mut self.text_popup_state);
                        (ev, false)
                    },
                )
                .0
                .flatten();
            }
            ToolKind::Helix => {
                let popup_title = ducad_i18n::t!("tool-helix");
                let mut custom_h = self.helix_popup_state.custom_height;
                popup_ev = ducad_ui::render_bottom_right_panel_custom(
                    &ctx,
                    "helix_popup",
                    &popup_title,
                    egui_material_icons::icons::ICON_HEATING_COIL.codepoint,
                    ducad_ui::theme::ACCENT_BLUE,
                    screen_rect,
                    tool_popup_bottom_y,
                    max_drawer_h,
                    &mut custom_h,
                    360.0,
                    230.0,
                    false,
                    |ui| {
                        let ev = ducad_ui::HelixPopup::show(ui, &mut self.helix_popup_state);
                        (ev, false)
                    },
                )
                .0
                .flatten();
                self.helix_popup_state.custom_height = custom_h;
            }
            _ => {}
        }

        if let Some(ev) = popup_ev {
            match ev {
                ToolPopupEvent::Close => {
                    self.set_tool(ToolKind::Select);
                    self.picking_mode = PickMode::None;
                }
                ToolPopupEvent::ApplyHelix { params, profile } => {
                    self.create_helix_coil_with_params(params, profile);
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
                ToolPopupEvent::ApplyText {
                    text,
                    font_height_mm,
                    letter_spacing,
                    align,
                    font_preset,
                    is_construction,
                } => {
                    let options = ducad_sketch::TextOptions {
                        font_height_mm,
                        letter_spacing,
                        line_spacing: self.text_popup_state.line_spacing,
                        align,
                        font_preset,
                        is_construction,
                    };
                    let origin = self.pending_points.first().copied().unwrap_or(glam::DVec2::ZERO);
                    self.apply_text_to_sketch(&text, origin, &options);
                    self.pending_points.clear();
                }
                ToolPopupEvent::PickCustomFont => {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Font Files (*.ttf, *.otf)", &["ttf", "otf"])
                        .pick_file()
                    {
                        if let Ok(bytes) = std::fs::read(&path) {
                            let file_name = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("Custom Font")
                                .to_string();
                            self.custom_font_bytes = Some(bytes);
                            self.text_popup_state.custom_font_name = Some(file_name);
                            self.model_status = Some("Font kustom berhasil dimuat".to_string());
                        }
                    }
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
                ToolPopupEvent::ApplyDraftAngle { angle_deg, pull_dir } => {
                    self.apply_draft_angle(angle_deg, pull_dir);
                }
                ToolPopupEvent::UpdateDraftInspection {
                    pull_dir,
                    target_angle_deg,
                    blend,
                } => {
                    self.draft_config.pull_dir = pull_dir;
                    self.draft_config.target_angle_deg = target_angle_deg;
                    self.draft_config.blend = blend;
                }
                ToolPopupEvent::CloseDraftInspection => {
                    self.draft_config.enabled = false;
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
                ToolPopupEvent::ApplyHole(spec) => {
                    self.apply_hole_wizard(spec);
                    self.set_tool(ToolKind::Select);
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
                        let is_const = self.sketch().entities.get(id).map_or(false, |e| e.is_construction());
                        let new_entity = Entity::line(
                            DVec2::new(start_x, start_y),
                            DVec2::new(end_x, end_y),
                        ).with_construction(is_const);
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
                        let is_const = self.sketch().entities.get(id).map_or(false, |e| e.is_construction());
                        let new_entity = Entity::circle(
                            DVec2::new(center_x, center_y),
                            radius,
                        ).with_construction(is_const);
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
                        let is_const = self.sketch().entities.get(id).map_or(false, |e| e.is_construction());
                        let new_entity = Entity::arc(
                            DVec2::new(center_x, center_y),
                            radius,
                            start_angle_deg.to_radians(),
                            end_angle_deg.to_radians(),
                        ).with_construction(is_const);
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
                        let is_const = self.sketch().entities.get(id).map_or(false, |e| e.is_construction());
                        let new_entity = Entity::ellipse(
                            DVec2::new(center_x, center_y),
                            radius_x,
                            radius_y,
                        ).with_construction(is_const);
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
        let has_face_sel = self.active_face.is_some()
            || !self.staged_mate_targets.is_empty()
            || !self.selected_faces.is_empty();
        let has_body_sel = !self.selected_bodies.is_empty() && !has_face_sel;

        if !self.drawing_sheet_state.is_open && (has_sketch_sel || has_face_sel || has_body_sel) {
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
                                ContextAction::Extend => self.set_tool(ToolKind::Extend),
                                ContextAction::Pattern => self.set_tool(ToolKind::Pattern),
                                ContextAction::Revolve => self.open_revolve_dialog(),
                                ContextAction::Sweep => {
                                    if let Ok(profile) = crate::model::build_profile_from_selection(self.sketch(), &self.selected) {
                                        self.pending_sweep_profile = Some((profile, self.active_plane));
                                        self.selected.clear();
                                        self.pending_sweep_path = None;
                                        self.sweep_path_plane_idx = None;
                                        self.model_status = Some("✓ Profil tersimpan! Sekarang klik kurva jalur pada bidang manapun di kanvas.".to_string());
                                    } else {
                                        self.pending_sweep_profile = None;
                                        self.pending_sweep_path = None;
                                        self.sweep_path_plane_idx = None;
                                    }
                                    self.set_tool(ToolKind::Sweep);
                                }
                                ContextAction::Helix => {
                                    self.set_tool(ToolKind::Helix);
                                }
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
                        if self.staged_mate_targets.len() >= 2 {
                            if let Some(act) = ContextActionBar::show_multi_face_selection(
                                ui,
                                self.staged_mate_targets.len(),
                            ) {
                                match act {
                                    ContextAction::MateConcentric => {
                                        self.staged_mate_kind =
                                            Some(ducad_core::MateKind::Concentric {
                                                aligned: true,
                                                lock_rotation: false,
                                            });
                                    }
                                    ContextAction::MateCoincident => {
                                        self.staged_mate_kind =
                                            Some(ducad_core::MateKind::Coincident {
                                                opposite_normal: true,
                                            });
                                    }
                                    ContextAction::MateDistance => {
                                        self.staged_mate_kind =
                                            Some(ducad_core::MateKind::Distance {
                                                offset: 10.0,
                                                opposite_normal: true,
                                            });
                                    }
                                    ContextAction::MateAngle => {
                                        self.staged_mate_kind =
                                            Some(ducad_core::MateKind::Angle {
                                                angle_deg: 45.0,
                                                opposite_normal: true,
                                            });
                                    }
                                    ContextAction::OpenAssemblyTree => {
                                        self.assembly_drawer_open = true;
                                    }
                                    ContextAction::ClearSelection => {
                                        self.staged_mate_targets.clear();
                                        self.selected_faces.clear();
                                        self.active_face = None;
                                    }
                                    _ => {}
                                }
                            }
                        } else {
                            let is_editing_hole = self.editing_hole_idx.is_some();
                            if let Some(act) =
                                ContextActionBar::show_face_selection(ui, is_editing_hole)
                            {
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
                                    ContextAction::Helix => {
                                        self.set_tool(ToolKind::Helix);
                                    }
                                    ContextAction::Shell => {
                                        self.set_tool(ToolKind::Shell);
                                    }
                                    ContextAction::Rib => {
                                        self.set_tool(ToolKind::Rib);
                                    }
                                    ContextAction::DraftAngle => {
                                        self.set_tool(ToolKind::DraftAngle);
                                    }
                                    ContextAction::SplitFace => {
                                        self.split_mode = ducad_ui::SplitMode::SplitFace;
                                        self.set_tool(ToolKind::SplitBody);
                                    }
                                    ContextAction::HoleWizard => {
                                        self.set_tool(ToolKind::HoleWizard);
                                    }
                                    ContextAction::OffsetPlane => {
                                        self.datum_mode = ducad_ui::DatumPlaneMode::Offset;
                                        self.set_tool(ToolKind::DatumPlane);
                                    }
                                    ContextAction::ClearSelection => {
                                        self.active_face = None;
                                        self.staged_mate_targets.clear();
                                        self.selected_faces.clear();
                                    }
                                    _ => {}
                                }
                            }
                        }
                    } else if has_body_sel {
                        if let Some(act) = ContextActionBar::show_body_selection(ui, self.selected_bodies.len()) {
                            match act {
                                ContextAction::SplitBody => {
                                    self.split_mode = ducad_ui::SplitMode::SplitBody;
                                    self.set_tool(ToolKind::SplitBody);
                                }
                                ContextAction::Pattern => {
                                    self.set_tool(ToolKind::Pattern);
                                }
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
        if !self.drawing_sheet_state.is_open && self.rename_popup_open {
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

        let has_top_bar_hud = matches!(
            self.tool,
            ToolKind::Loft
                | ToolKind::Shell
                | ToolKind::Rib
                | ToolKind::DraftAngle
                | ToolKind::SplitBody
                | ToolKind::DatumPlane
                | ToolKind::Boolean
                | ToolKind::Sweep
                | ToolKind::Pattern
                | ToolKind::Polygon
                | ToolKind::Slot
                | ToolKind::Revolve
        ) || self.staged_mate_kind.is_some();

        if !has_top_bar_hud && !self.drawing_sheet_state.is_open {
            if let Some(ev) = CanvasHud::show_status_pill(
                ui,
                screen_rect,
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
        }

        let palette_actions = self.palette_actions();
        let palette_entries: Vec<(&str, &str)> = palette_actions
            .iter()
            .map(|(label, hint, _)| (label.as_str(), hint.as_str()))
            .collect();
        if let Some(idx) = self.palette.show(&ctx, &palette_entries) {
            let action = palette_actions[idx].2;
            self.run_palette_action(&ctx, action);
        }

        // Render Lembar Kerja Gambar Teknik 2D (Fase 5) jika sedang aktif
        let mut ds_action = None;
        if self.drawing_sheet_state.is_open {
            if self.drawing_sheet_doc.is_none() {
                let sheet = self.build_current_drawing_sheet();
                self.drawing_sheet_doc = Some(sheet);
            }
            if let Some(sheet) = &mut self.drawing_sheet_doc {
                egui::Area::new(egui::Id::new("ducad-drawing-sheet-overlay"))
                    .fixed_pos(screen_rect.min)
                    .order(egui::Order::Foreground)
                    .show(&ctx, |ui| {
                        ui.set_width(screen_rect.width());
                        ui.set_height(screen_rect.height());
                        if let Some(event) = ducad_ui::DrawingSheetView::show(ui, &mut self.drawing_sheet_state, sheet) {
                            ds_action = Some(event);
                        }
                    });
            }
        }

        if let Some(event) = ds_action {
            match event {
                ducad_ui::DrawingSheetEvent::Close => {
                    self.drawing_sheet_state.is_open = false;
                }
                ducad_ui::DrawingSheetEvent::ExportPdf => {
                    self.export_drawing_pdf();
                }
                ducad_ui::DrawingSheetEvent::ExportDxf => {
                    self.export_drawing_dxf();
                }
                ducad_ui::DrawingSheetEvent::ExportSvg => {
                    self.export_drawing_svg();
                }
            }
        }

        // Render Alert Modal Peringatan jika ada operasi yang gagal
        ducad_ui::AlertModal::show(&ctx, &mut self.alert_modal);
    }
}

impl DuCADApp {
    /// Buka konfigurasi Revolve dan aktifkan tool Revolve.
    pub fn open_revolve_dialog(&mut self) {
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
        egui::Color32::from_rgba_premultiplied(18, 48, 88, 160)
    } else if is_hovered {
        egui::Color32::from_rgba_premultiplied(38, 44, 58, 160)
    } else {
        ducad_ui::BG_PANEL_DARK
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

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_sketch::{Entity, Sketch};
    use glam::{DVec2, Vec3};

    #[test]
    fn test_calculate_grid_extent_default() {
        let sketch = Sketch::default();
        let extent = calculate_grid_extent_for_params(
            &sketch,
            &[],
            false,
            None,
            Vec3::ZERO,
            250.0,
        );
        // Default base extent is 500.0 (or default camera distance offset), minimum 500.0
        assert!(extent >= 500.0);
    }

    #[test]
    fn test_calculate_grid_extent_expands_with_large_sketch_entity() {
        let mut sketch = Sketch::default();
        // Insert a circle at (1200, 0) with radius 300 -> max coordinate = 1500
        sketch.entities.insert(Entity::circle(
            DVec2::new(1200.0, 0.0),
            300.0,
        ));

        let extent = calculate_grid_extent_for_params(
            &sketch,
            &[],
            false,
            None,
            Vec3::ZERO,
            250.0,
        );
        // Should expand to encompass 1500 + padding (100) -> >= 1600
        assert!(extent >= 1600.0, "Extent was {extent}, expected >= 1600.0");
    }

    #[test]
    fn test_calculate_grid_extent_expands_with_cursor_when_sketching() {
        let sketch = Sketch::default();
        let cursor = Some(DVec2::new(2500.0, 0.0));
        let extent = calculate_grid_extent_for_params(
            &sketch,
            &[],
            true,
            cursor,
            Vec3::ZERO,
            250.0,
        );
        assert!(extent >= 2600.0, "Extent was {extent}, expected >= 2600.0");
    }

    #[test]
    fn test_calculate_grid_extent_expands_with_zoom_out() {
        let sketch = Sketch::default();
        let extent = calculate_grid_extent_for_params(
            &sketch,
            &[],
            false,
            None,
            Vec3::ZERO,
            5000.0,
        );
        // Camera distance 5000 * 0.75 = 3750 + 100 = 3850 -> ceil to 3900
        assert!(extent >= 3900.0, "Extent was {extent}, expected >= 3900.0");
    }
}

