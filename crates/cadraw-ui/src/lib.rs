//! Komponen UI bersama CADRAW bergaya Shapr3D (Floating Canvas-First UI):
//! - Tema glassmorphism gelap & token warna Shapr3D (`theme`)
//! - Interactive 3D ViewCube & Orientation Gizmo (`viewcube`)
//! - Bilah alat vertikal mengambang di sisi kiri (`left_toolbar`)
//! - Outliner drawer pohon item (`items_drawer`)
//! - Pohon fitur parametrik & inspektor 3D (`feature_inspector`)
//! - Strip ikon constraint mengambang (`constraint_strip`)
//! - In-Canvas HUD & Dimension Pills (`canvas_hud`)
//! - Modern top bar & title header (`top_bar`)
//! - Command palette (`command_palette`)
//! - Radial menu untuk sentuh/iPad (`radial_menu`)

pub mod canvas_hud;
pub mod command_palette;
pub mod constraint_strip;
pub mod context_bar;
pub mod feature_inspector;
pub mod items_drawer;
pub mod left_toolbar;
pub mod radial_menu;
pub mod theme;
pub mod top_bar;
pub mod viewcube;

pub use canvas_hud::{CanvasHud, CanvasHudEvent};
pub use command_palette::CommandPalette;
pub use constraint_strip::{ConstraintAction, ConstraintStrip};
pub use context_bar::{ContextAction, ContextActionBar};
pub use feature_inspector::{
    FeatureInspector, FeatureInspectorState, InspectorBooleanKind, InspectorConstraintAction,
    InspectorEvent, InspectorPickMode, InspectorRectAnchor, SelectedBodyData, SelectedEntityData,
};
pub use items_drawer::{BodyItemInfo, ItemsDrawer, ItemsDrawerEvent, SketchPlaneItemInfo};
pub use left_toolbar::{LeftToolbar, ToolbarEvent, ToolbarTool};
pub use radial_menu::RadialMenu;
pub use theme::{
    apply as apply_theme, card_frame, dimension_pill_frame, glass_frame, pill_frame, ThemeMode,
    ACCENT_BLUE, ACCENT_GREEN, ACCENT_ORANGE, ACCENT_PURPLE, BG_CANVAS, BG_CARD_DARK,
    BG_HOVER_DARK, BG_PANEL_DARK, BORDER_SUBTLE, MIN_TOUCH_TARGET, TEXT_MUTED, TEXT_PRIMARY,
    TEXT_SECONDARY,
};
pub use top_bar::{TopBar, TopBarEvent, TopBarFileOp, TopBarState};
pub use viewcube::{ViewCube, ViewCubeAction};
