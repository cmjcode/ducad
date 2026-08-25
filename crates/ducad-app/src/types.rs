use ducad_core::LengthUnit;
use ducad_kernel::PickRay;
use ducad_ui::{SelectedEntityData, ToolbarTool};
use glam::{DVec2, Vec3};

/// Tool sketch aktif.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Select,
    // 2D Sketch tools
    Line,
    Rectangle,
    Circle,
    Ellipse,
    /// Poligon N-sisi beraturan (Segi-3, Segi-5, Segi-6, Segi-8).
    Polygon,
    /// Slot lonjong / rel baut (Center-to-Center / Overall Length).
    Slot,
    /// Kurva Spline multi-titik (Catmull-Rom).
    Spline,
    /// Teks 2D sketsa (vektorisasi font TTF/OTF — Fase 9.5).
    Text,
    /// 2D Fillet (busur sudut tangensial).
    Fillet2D,
    /// 2D Chamfer (garis sudut bevel).
    Chamfer2D,
    /// 3 titik: awal, akhir, titik di busur.
    Arc,
    /// Klik entitas sumber, lalu klik sisi & jarak hasil offset.
    Offset,
    /// Perlu seleksi non-kosong sebelum memilih 2 titik sumbu cermin.
    Mirror,
    /// Klik segmen Line yang mau dipotong.
    Trim,
    /// Constraint Coincident.
    CoincidentPick,
    /// Constraint Fixed.
    FixedPick,
    /// Constraint Symmetric.
    SymmetricPick,
    /// Pattern / Array (Linier X/Y/Z & Sirkular putar).
    Pattern,
    // 3D Solid tools
    Extrude,
    /// Revolve 360° penuh atau sudut custom.
    Revolve,
    Loft,
    Sweep,
    Shell,
    /// Tulang Penguat (Rib / Stiffener Support — Fase 2.4).
    Rib,
    /// Draft Angle — kemiringan cetakan plastik (injection molding, Fase 2.1).
    DraftAngle,
    /// Split Body & Split Face (Fase 2.2 — Potong Benda).
    SplitBody,
    Boolean,
    SectionView,
    /// Inspeksi Garis Zebra (Fase 3.1 Zebra Stripes Reflection Shader).
    ZebraInspection,
    /// Inspeksi Sudut Lepas Cetakan (Fase 3.2 Draft Angle Heatmap Inspector).
    DraftAnalysis,
    // Shared Utilities
    /// Non-destruktif Ukur Jarak.
    Measure,
    /// Non-destruktif Ukur Sudut.
    MeasureAngle,
    /// Hole Wizard — Lubang standar baut ISO (Fase 9.2).
    HoleWizard,
    /// Riwayat Dokumen & Undo/Redo.
    History,
}

impl ToolKind {
    pub fn to_toolbar_tool(self) -> ToolbarTool {
        match self {
            ToolKind::Select => ToolbarTool::Select,
            ToolKind::Line => ToolbarTool::Line,
            ToolKind::Arc => ToolbarTool::Arc,
            ToolKind::Rectangle => ToolbarTool::Rectangle,
            ToolKind::Circle => ToolbarTool::Circle,
            ToolKind::Ellipse => ToolbarTool::Ellipse,
            ToolKind::Polygon => ToolbarTool::Polygon,
            ToolKind::Slot => ToolbarTool::Slot,
            ToolKind::Spline => ToolbarTool::Spline,
            ToolKind::Text => ToolbarTool::Text,
            ToolKind::Fillet2D => ToolbarTool::Fillet2D,
            ToolKind::Chamfer2D => ToolbarTool::Chamfer2D,
            ToolKind::Offset => ToolbarTool::Offset,
            ToolKind::Mirror => ToolbarTool::Mirror,
            ToolKind::Trim => ToolbarTool::Trim,
            ToolKind::CoincidentPick => ToolbarTool::PointCoincident,
            ToolKind::FixedPick => ToolbarTool::PointFixed,
            ToolKind::SymmetricPick => ToolbarTool::PointSymmetric,
            ToolKind::Pattern => ToolbarTool::Pattern,
            ToolKind::Extrude => ToolbarTool::Extrude,
            ToolKind::Revolve => ToolbarTool::Revolve,
            ToolKind::Loft => ToolbarTool::Loft,
            ToolKind::Sweep => ToolbarTool::Sweep,
            ToolKind::Shell => ToolbarTool::Shell,
            ToolKind::Rib => ToolbarTool::Rib,
            ToolKind::DraftAngle => ToolbarTool::DraftAngle,
            ToolKind::SplitBody => ToolbarTool::SplitBody,
            ToolKind::Boolean => ToolbarTool::Boolean,
            ToolKind::SectionView => ToolbarTool::SectionView,
            ToolKind::ZebraInspection => ToolbarTool::ZebraInspection,
            ToolKind::DraftAnalysis => ToolbarTool::DraftAnalysis,
            ToolKind::Measure => ToolbarTool::Measure,
            ToolKind::MeasureAngle => ToolbarTool::MeasureAngle,
            ToolKind::HoleWizard => ToolbarTool::Select,
            ToolKind::History => ToolbarTool::History,
        }
    }

    pub fn from_toolbar_tool(tool: ToolbarTool) -> Self {
        match tool {
            ToolbarTool::Select => ToolKind::Select,
            ToolbarTool::Line => ToolKind::Line,
            ToolbarTool::Arc => ToolKind::Arc,
            ToolbarTool::Rectangle => ToolKind::Rectangle,
            ToolbarTool::Circle => ToolKind::Circle,
            ToolbarTool::Ellipse => ToolKind::Ellipse,
            ToolbarTool::Polygon => ToolKind::Polygon,
            ToolbarTool::Slot => ToolKind::Slot,
            ToolbarTool::Spline => ToolKind::Spline,
            ToolbarTool::Text => ToolKind::Text,
            ToolbarTool::Fillet2D => ToolKind::Fillet2D,
            ToolbarTool::Chamfer2D => ToolKind::Chamfer2D,
            ToolbarTool::Offset => ToolKind::Offset,
            ToolbarTool::Mirror => ToolKind::Mirror,
            ToolbarTool::Trim => ToolKind::Trim,
            ToolbarTool::PointCoincident => ToolKind::CoincidentPick,
            ToolbarTool::PointFixed => ToolKind::FixedPick,
            ToolbarTool::PointSymmetric => ToolKind::SymmetricPick,
            ToolbarTool::Pattern => ToolKind::Pattern,
            ToolbarTool::Extrude => ToolKind::Extrude,
            ToolbarTool::Revolve => ToolKind::Revolve,
            ToolbarTool::Loft => ToolKind::Loft,
            ToolbarTool::Sweep => ToolKind::Sweep,
            ToolbarTool::Shell => ToolKind::Shell,
            ToolbarTool::Rib => ToolKind::Rib,
            ToolbarTool::DraftAngle => ToolKind::DraftAngle,
            ToolbarTool::SplitBody => ToolKind::SplitBody,
            ToolbarTool::Boolean => ToolKind::Boolean,
            ToolbarTool::SectionView => ToolKind::SectionView,
            ToolbarTool::ZebraInspection => ToolKind::ZebraInspection,
            ToolbarTool::DraftAnalysis => ToolKind::DraftAnalysis,
            ToolbarTool::Measure => ToolKind::Measure,
            ToolbarTool::MeasureAngle => ToolKind::MeasureAngle,
            ToolbarTool::History => ToolKind::History,
        }
    }
}

pub const RADIAL_TOOLS: [(ToolKind, &str); 8] = [
    (ToolKind::Line, "Garis"),
    (ToolKind::Rectangle, "Persegi"),
    (ToolKind::Circle, "Lingkaran"),
    (ToolKind::Ellipse, "Ellips"),
    (ToolKind::Arc, "Arc"),
    (ToolKind::Offset, "Offset"),
    (ToolKind::Mirror, "Mirror"),
    (ToolKind::Trim, "Trim"),
];

#[allow(dead_code)]
pub const KEYBOARD_SHORTCUTS: [(&str, &str); 23] = [
    ("L", "Tool Garis"),
    ("R", "Tool Persegi"),
    ("C", "Tool Lingkaran"),
    ("E", "Tool Ellips / Extrude"),
    ("Y", "Tool Segi-N Beraturan (Polygon)"),
    ("A", "Tool Arc"),
    ("O", "Tool Offset"),
    ("M", "Tool Mirror"),
    ("T", "Tool Trim"),
    ("V", "Tool Revolve"),
    ("F", "Tool Fillet & Chamfer"),
    ("S", "Tool Shell / Hollow / Split"),
    ("D", "Tool Draft Angle (Kemiringan Cetakan)"),
    ("P", "Tool Pattern / Array (Linier & Sirkular)"),
    ("B", "Tool Boolean"),
    ("I", "Tool Pengukuran"),
    ("H", "Riwayat & Undo/Redo"),
    ("Esc", "Batal titik pending, atau kembali ke tool Pilih"),
    ("Tab", "Ganti plane sketsa aktif"),
    ("Cmd+Z", "Undo langkah terakhir"),
    ("Cmd+Shift+Z", "Redo langkah terakhir"),
    ("Cmd+A", "Pilih semua entitas"),
    ("Delete/Backspace", "Hapus entitas terpilih"),
];

#[derive(Debug, Clone, Copy)]
pub enum FileOp {
    New,
    Open,
    Save,
    SaveAs,
    ImportStep,
    ImportDxf,
    ExportStep,
    ExportStl,
    ExportObj,
    ExportDxf,
    ExportPdf,
    ExportDrawingDxf,
    OpenDrawingSheet,
}

#[derive(Debug, Clone, Copy)]
pub enum PaletteAction {
    SetTool(ToolKind),
    OpenRevolveDialog,
    Undo,
    Redo,
    ModelUndo,
    ModelRedo,
    DeleteSelection,
    ToggleTheme,
    ToggleZebraView,
    ToggleStudioLighting,
    ToggleConstruction,
    OpenDrawingSheet,
    SetSketchPlane(ducad_render::PlaneKind),
    EnterSketching,
    ExitSketching,
    File(FileOp),
    ClearMeasurements,
}

pub fn required_points(tool: ToolKind) -> usize {
    match tool {
        ToolKind::Text => 1,
        ToolKind::Rectangle
        | ToolKind::Circle
        | ToolKind::Ellipse
        | ToolKind::Polygon
        | ToolKind::Mirror
        | ToolKind::Revolve
        | ToolKind::Measure => 2,
        ToolKind::Arc | ToolKind::Slot | ToolKind::MeasureAngle => 3,
        ToolKind::Select
        | ToolKind::Line
        | ToolKind::Spline
        | ToolKind::Fillet2D
        | ToolKind::Chamfer2D
        | ToolKind::Offset
        | ToolKind::Trim
        | ToolKind::CoincidentPick
        | ToolKind::FixedPick
        | ToolKind::SymmetricPick
        | ToolKind::Pattern
        | ToolKind::Extrude
        | ToolKind::Loft
        | ToolKind::Sweep
        | ToolKind::Shell
        | ToolKind::Rib
        | ToolKind::DraftAngle
        | ToolKind::SplitBody
        | ToolKind::Boolean
        | ToolKind::SectionView
        | ToolKind::ZebraInspection
        | ToolKind::DraftAnalysis
        | ToolKind::HoleWizard
        | ToolKind::History => 0,
    }
}

pub enum Measurement {
    Distance { a: DVec2, b: DVec2 },
    Angle { a: DVec2, vertex: DVec2, b: DVec2 },
}

impl Measurement {
    pub fn label(&self) -> String {
        match self {
            Measurement::Distance { a, b } => {
                let dist_str = format!("{:.3} mm", ducad_sketch::measure::distance(*a, *b));
                ducad_i18n::t!("param-distance-val", val = dist_str.as_str())
            }
            Measurement::Angle { a, vertex, b } => {
                match ducad_sketch::measure::angle_degrees(*a, *vertex, *b) {
                    Some(angle) => {
                        let angle_str = format!("{angle:.2}°");
                        ducad_i18n::t!("param-angle-val", val = angle_str.as_str())
                    }
                    None => ducad_i18n::t!("measure-angle-undefined"),
                }
            }
        }
    }

    pub fn points(&self) -> Vec<DVec2> {
        match self {
            Measurement::Distance { a, b } => vec![*a, *b],
            Measurement::Angle { a, vertex, b } => vec![*a, *vertex, *b],
        }
    }

    pub fn inline_value(&self, unit: LengthUnit) -> Option<String> {
        match self {
            Measurement::Distance { a, b } => {
                Some(unit.format_precise(ducad_sketch::measure::distance(*a, *b)))
            }
            Measurement::Angle { a, vertex, b } => ducad_sketch::measure::angle_degrees(*a, *vertex, *b)
                .map(|deg| format!("{deg:.1}°")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionAxis {
    X,
    Y,
    Z,
}

impl SectionAxis {
    pub fn normal(self) -> Vec3 {
        match self {
            SectionAxis::X => Vec3::X,
            SectionAxis::Y => Vec3::Y,
            SectionAxis::Z => Vec3::Z,
        }
    }

    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            SectionAxis::X => "X",
            SectionAxis::Y => "Y",
            SectionAxis::Z => "Z",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PickMode {
    #[default]
    None,
    Edge,
    Face,
}

pub type InspectorContentSig = (
    std::mem::Discriminant<SelectedEntityData>,
    bool,
    usize,
    usize,
    usize,
    bool,
    bool,
    bool,
    PickMode,
    usize,
    bool,
);

pub struct PickedEdge {
    pub ray: PickRay,
    pub polyline: Vec<(f64, f64, f64)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RoundKind {
    Vertex,
    Edge,
}

/// Gaya rounding: `Fillet` atau `Chamfer`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RoundStyle {
    Fillet,
    Chamfer,
}

#[derive(Clone)]
pub struct RoundFeature {
    pub kind: RoundKind,
    pub style: RoundStyle,
    pub ray: PickRay,
    pub anchor: (f64, f64, f64),
    pub radius: f64,
    pub polyline: Vec<(f64, f64, f64)>,
}

pub struct RoundHistory {
    pub base: ducad_kernel::KernelShape,
    pub features: Vec<RoundFeature>,
}

#[derive(Clone, Debug)]
pub struct HoleFeature {
    pub spec: ducad_core::hole::HoleSpec,
    pub pos: (f64, f64, f64),
    pub normal: (f64, f64, f64),
    pub face_hit: ducad_kernel::FaceHit,
    pub offset_u: f64,
    pub offset_v: f64,
}

pub struct HoleHistory {
    pub base: ducad_kernel::KernelShape,
    pub features: Vec<HoleFeature>,
}
