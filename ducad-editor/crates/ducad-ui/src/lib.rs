//! Komponen UI bersama DUCAD bergaya Shapr3D (Floating Canvas-First UI):
//! - Tema glassmorphism gelap & token warna Shapr3D (`theme`)
//! - Interactive 3D ViewCube & Orientation Gizmo (`viewcube`)
//! - Bilah alat vertikal mengambang di sisi kiri (`left_toolbar`)
//! - Outliner drawer pohon item (`items_drawer`)
//! - Pohon fitur parametrik & inspektor 3D (`feature_inspector`)
//! - Modular tool popups pojok kanan bawah (`tool_popups`)
//! - Strip ikon constraint mengambang (`constraint_strip`)
//! - In-Canvas HUD & Dimension Pills (`canvas_hud`)
//! - Modern top bar & title header (`top_bar`)
//! - Command palette (`command_palette`)
//! - Radial menu untuk sentuh/iPad (`radial_menu`)

pub mod account_drawer;
pub mod assembly_drawer;
pub mod canvas_hud;
pub mod chat_panel;
pub mod checks_panel;
pub mod cmf_drawer;
pub mod command_palette;
pub mod constraint_strip;
pub mod context_bar;
pub mod drawing_sheet_view;
pub mod assist_dialog;
pub mod error_card;
pub mod proposal_card;
pub mod feature_inspector;
pub mod feature_tree_drawer;
pub mod history_drawer;
pub mod sketch_hud;
pub mod items_drawer;
pub mod left_toolbar;
pub mod lighting_drawer;
pub mod planes_drawer;
pub mod radial_menu;
pub mod revolve_dialog;
pub mod theme;
pub mod tool_guides;
pub mod tool_popups;
pub mod touch;
pub mod top_bar;
pub mod vector;
pub mod viewcube;

pub use vector::{
    ColorPickerAction, ColorPickerState, LayersPanelEvent, LayersPanelState,
    PropertiesPanelEvent, PropertiesPanelState, PropertyVal, StyleDiff, SwatchManager,
};

pub use account_drawer::{AccountDrawer, AccountDrawerEvent};
pub use assembly_drawer::{AssemblyDrawer, AssemblyDrawerEvent};
pub use canvas_hud::{
    BooleanHudAction, BooleanOpKind, CanvasHud, CanvasHudEvent, DatumPlaneHudAction, DatumPlaneMode,
    DraftHudAction, DraftInspectionHudAction, DraftPullDir, LoftHudAction, MateHudAction,
    PatternAxisPreset, PatternHudAction, PatternKind, PolygonHudAction, RenamePopupEvent,
    RevolveHudAction, RibHudAction, RoundingHudAction, RoundingHudStyle, ShellHudAction,
    SlotHudAction, SplitHudAction, SplitMode, SplitPlaneKind, StudioHudAction,
    StudioLightingPresetUi, SweepHudAction, ZebraHudAction,
};
pub use chat_panel::{
    ChatItem, ChatPanel, ChatPanelEvent, ChatPanelState, ChatProviderForm, ChatRole, CliFormProfile,
    CliMeta,
};
pub use checks_panel::{checks_summary, CheckRowStatus, CheckRowUi, ChecksPanel, ChecksPanelEvent};
pub use cmf_drawer::{CmfDrawer, CmfDrawerEvent};
pub use command_palette::CommandPalette;
pub use constraint_strip::{ConstraintAction, ConstraintStrip};
pub use context_bar::{ContextAction, ContextActionBar, VectorExtrudeBarState};
pub use sketch_hud::{InkHudState, InkHudTool, SketchHud, SketchHudEvent};
pub use drawing_sheet_view::{DrawingSheetEvent, DrawingSheetView, DrawingSheetViewState};
pub use assist_dialog::{AssistDialog, AssistDialogEvent, AssistDialogState};
pub use error_card::{ErrorCard, ErrorCardEvent, ErrorCardState};
pub use proposal_card::{ProposalCard, ProposalCardEvent, ProposalCardState};
pub use feature_inspector::{
    FeatureInspector, FeatureInspectorState, InspectorBooleanKind, InspectorConstraintAction,
    InspectorEvent, InspectorPickMode, InspectorRectAnchor, SelectedBodyData, SelectedEntityData,
};
pub use feature_tree_drawer::{FeatureTreeDrawer, FeatureTreeEvent};
pub use history_drawer::{
    ActivityItemInfo, ActivityKindUi, HistoryDrawer, HistoryDrawerEvent,
};
pub use items_drawer::{
    BodyItemInfo, Entity2dItemInfo, ItemsDrawer, ItemsDrawerEvent, ItemsDrawerTab,
    VectorDrawerContext,
};
pub use left_toolbar::{LeftToolbar, ToolbarEvent, ToolbarTool};
pub use lighting_drawer::{LightingDrawer, LightingDrawerEvent};
pub use planes_drawer::{PlaneItemInfo, PlanesDrawer, PlanesDrawerEvent};
pub use radial_menu::RadialMenu;
pub use revolve_dialog::{
    AlertModal, AlertModalState, RevolveAxisPreset, RevolveDialog, RevolveDialogEvent,
    RevolveDialogState,
};
pub use theme::{
    apply as apply_theme, apply_with_touch, card_frame, dimension_pill_frame, glass_frame,
    pill_frame, ThemeMode, ACCENT_BLUE, ACCENT_GREEN, ACCENT_ORANGE, ACCENT_PURPLE, BG_CANVAS,
    BG_CARD_DARK, BG_HOVER_DARK, BG_PANEL_DARK, BORDER_SUBTLE, BOTTOM_RIGHT_PANEL_WIDTH,
    ICON_SIZE_DEFAULT, MIN_TOUCH_TARGET, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
    TOUCH_TARGET_IPAD,
};
pub use tool_guides::ToolGuides;
pub use tool_popups::{
    render_bottom_right_panel_custom, render_bottom_right_popup, BooleanPopup, BooleanPopupState,
    DraftAnalysisPopup, DraftPopupState, Entity2dPopup, Entity2dPopupState, ExtrudePopup,
    ExtrudePopupState, HelixPopup, HelixPopupState, HelixPreset, HelixSectionType, HistoryPopup,
    HistoryPopupState, HoleOperationMode, HolePopup, HolePopupState, LoftPopup,
    LoftPopupState, MeasurePopup, MeasurePopupState, RevolvePopup, RevolvePopupState, ShellPopup,
    ShellPopupState, TextPopup, TextPopupState, ToolPopupEvent,
};
pub use top_bar::{TopBar, TopBarEvent, TopBarFileOp, TopBarState};
pub use touch::{TouchDesignConfig, TouchDesignMode};
pub use viewcube::{ViewCube, ViewCubeAction};
pub use ducad_i18n::{current_language, set_language, t, Language};
