#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorBooleanKind {
    Union,
    Subtract,
    Intersect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorPickMode {
    None,
    Edge,
    Face,
}

/// Titik acuan yang tetap diam saat rectangle 2D di-resize dari panel properti.
/// Mirror dari `ducad_sketch::region::RectAnchor` — didefinisikan ulang di sini
/// karena `ducad-ui` sengaja tidak bergantung pada `ducad-sketch` (lihat pola
/// `SelectedEntityData`/`InspectorEvent` lain yang juga plain data, bukan tipe kernel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorRectAnchor {
    Center,
    Corner0,
    Corner1,
    Corner2,
    Corner3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorConstraintAction {
    Horizontal,
    Vertical,
    Parallel,
    Perpendicular,
    EqualLength,
    EqualRadius,
    Tangent,
    Coincident,
    Fixed,
    Symmetric,
}

#[derive(Debug, Clone)]
pub enum SelectedEntityData {
    None,
    Line {
        id_raw: u64,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
        length: f64,
        angle_deg: f64,
    },
    Circle {
        id_raw: u64,
        center_x: f64,
        center_y: f64,
        radius: f64,
        diameter: f64,
    },
    Arc {
        id_raw: u64,
        center_x: f64,
        center_y: f64,
        radius: f64,
        start_angle_deg: f64,
        end_angle_deg: f64,
    },
    Ellipse {
        id_raw: u64,
        center_x: f64,
        center_y: f64,
        radius_x: f64,
        radius_y: f64,
    },
    /// 4 `Entity::Line` tertutup & saling tegak lurus (lihat `ducad_sketch::detect_rectangle`).
    Rectangle {
        entity_ids: [u64; 4],
        length_p: f64,
        length_l: f64,
    },
    MultipleEntities {
        count: usize,
    },
}

#[derive(Debug, Clone)]
pub struct SelectedBodyData {
    pub id_raw: u64,
    pub name: String,
    pub vertices_count: usize,
    pub triangles_count: usize,
    pub bbox_size: [f32; 3],
}

#[derive(Debug, Clone)]
pub enum InspectorEvent {
    CloseInspector,
    ToggleAutoHide,
    ToggleShowAllDimensions,
    UndoModel,
    RedoModel,
    UpdateEntityLine {
        id_raw: u64,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
    },
    UpdateEntityCircle {
        id_raw: u64,
        center_x: f64,
        center_y: f64,
        radius: f64,
    },
    UpdateEntityArc {
        id_raw: u64,
        center_x: f64,
        center_y: f64,
        radius: f64,
        start_angle_deg: f64,
        end_angle_deg: f64,
    },
    UpdateEntityEllipse {
        id_raw: u64,
        center_x: f64,
        center_y: f64,
        radius_x: f64,
        radius_y: f64,
    },
    UpdateEntityRectangle {
        entity_ids: [u64; 4],
        length_p: f64,
        length_l: f64,
        anchor: InspectorRectAnchor,
    },
    ApplyConstraint(InspectorConstraintAction),
    ApplyExtrude { distance: f64 },
    ApplyFaceExtrude { distance: f64 },
    SketchOnFace,
    ApplyRevolve,
    ApplyRevolvePreset { preset_idx: u8, angle_deg: f64 },
    StartManualRevolve,
    StageLoftBottom,
    ApplyLoft { height: f64 },
    ApplyBoolean(InspectorBooleanKind),
    ToggleEdgePicking,
    ResetEdgePicking,
    ApplyFillet { radius: f64 },
    ApplyChamfer { distance: f64 },
    ToggleFacePicking,
    ApplyShell { thickness: f64 },
    DeleteSelectedBodies,
    SectionViewChanged,
    RemoveMeasurement(usize),
    ClearMeasurements,
}

pub struct FeatureInspectorState {
    pub auto_hide_enabled: bool,
    pub selected_entity: SelectedEntityData,
    pub selected_body: Option<SelectedBodyData>,
    pub selected_bodies_count: usize,
    pub selected_edges_count: usize,
    pub selected_faces_count: usize,
    pub total_entities_count: usize,
    pub total_bodies_count: usize,

    // Inputs for 2D entity property edit
    pub entity_p1_x: String,
    pub entity_p1_y: String,
    pub entity_p2_x: String,
    pub entity_p2_y: String,
    pub entity_val_1: String,
    pub entity_val_2: String,
    /// Diameter Circle — field kedua yang saling sinkron dgn `entity_val_1` (radius).
    pub entity_val_3: String,
    /// Input Panjang (P) & Lebar (L) utk card Rectangle.
    pub rect_length_p_input: String,
    pub rect_length_l_input: String,
    /// Anchor aktif yg dipakai saat "Terapkan Dimensi" rectangle diklik.
    pub rect_anchor: InspectorRectAnchor,

    // Inputs for 3D operations
    pub extrude_input: String,
    pub active_face_selected: bool,
    pub face_extrude_input: String,
    pub revolve_angle_input: String,
    pub revolve_axis_preset: u8,
    pub revolve_reverse: bool,
    pub loft_height_input: String,
    pub loft_bottom_staged: bool,
    pub fillet_input: String,
    pub chamfer_input: String,
    pub shell_input: String,
    pub picking_mode: InspectorPickMode,
    pub can_undo_model: bool,
    pub can_redo_model: bool,
    pub status_message: Option<String>,
    pub section_enabled: bool,
    pub section_axis: u8, // 0 = X, 1 = Y, 2 = Z
    pub section_offset: f32,
    pub section_invert: bool,

    /// Label siap tampil untuk tiap pengukuran aktif.
    pub measurements: Vec<String>,
    /// True kalau tool Ukur/Ukur Sudut sedang aktif.
    pub measurement_tool_active: bool,
    /// Checkbox "Tampilkan Semua Ukuran".
    pub show_all_dimensions: bool,

    /// Batas tinggi maksimum panel.
    pub max_panel_height: f32,
}

impl Default for FeatureInspectorState {
    fn default() -> Self {
        Self {
            auto_hide_enabled: true,
            selected_entity: SelectedEntityData::None,
            selected_body: None,
            selected_bodies_count: 0,
            selected_edges_count: 0,
            selected_faces_count: 0,
            total_entities_count: 0,
            total_bodies_count: 0,

            entity_p1_x: String::new(),
            entity_p1_y: String::new(),
            entity_p2_x: String::new(),
            entity_p2_y: String::new(),
            entity_val_1: String::new(),
            entity_val_2: String::new(),
            entity_val_3: String::new(),
            rect_length_p_input: String::new(),
            rect_length_l_input: String::new(),
            rect_anchor: InspectorRectAnchor::Center,

            extrude_input: "20.0".to_string(),
            active_face_selected: false,
            face_extrude_input: "15.0".to_string(),
            revolve_angle_input: "360.0".to_string(),
            revolve_axis_preset: 0,
            revolve_reverse: false,
            loft_height_input: "30.0".to_string(),
            loft_bottom_staged: false,
            fillet_input: "5.0".to_string(),
            chamfer_input: "2.0".to_string(),
            shell_input: "2.0".to_string(),
            picking_mode: InspectorPickMode::None,
            can_undo_model: false,
            can_redo_model: false,
            status_message: None,
            section_enabled: false,
            section_axis: 2, // Z
            section_offset: 0.0,
            section_invert: false,

            measurements: Vec::new(),
            measurement_tool_active: false,
            show_all_dimensions: false,

            max_panel_height: 600.0,
        }
    }
}
