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
    // 3D Solid tools
    Extrude,
    /// Revolve 360° penuh atau sudut custom.
    Revolve,
    Loft,
    FilletChamfer,
    Shell,
    Boolean,
    SectionView,
    // Shared Utilities
    /// Non-destruktif Ukur Jarak.
    Measure,
    /// Non-destruktif Ukur Sudut.
    MeasureAngle,
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
            ToolKind::Offset => ToolbarTool::Offset,
            ToolKind::Mirror => ToolbarTool::Mirror,
            ToolKind::Trim => ToolbarTool::Trim,
            ToolKind::CoincidentPick => ToolbarTool::PointCoincident,
            ToolKind::FixedPick => ToolbarTool::PointFixed,
            ToolKind::SymmetricPick => ToolbarTool::PointSymmetric,
            ToolKind::Extrude => ToolbarTool::Extrude,
            ToolKind::Revolve => ToolbarTool::Revolve,
            ToolKind::Loft => ToolbarTool::Loft,
            ToolKind::FilletChamfer => ToolbarTool::FilletChamfer,
            ToolKind::Shell => ToolbarTool::Shell,
            ToolKind::Boolean => ToolbarTool::Boolean,
            ToolKind::SectionView => ToolbarTool::SectionView,
            ToolKind::Measure => ToolbarTool::Measure,
            ToolKind::MeasureAngle => ToolbarTool::MeasureAngle,
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
            ToolbarTool::Offset => ToolKind::Offset,
            ToolbarTool::Mirror => ToolKind::Mirror,
            ToolbarTool::Trim => ToolKind::Trim,
            ToolbarTool::PointCoincident => ToolKind::CoincidentPick,
            ToolbarTool::PointFixed => ToolKind::FixedPick,
            ToolbarTool::PointSymmetric => ToolKind::SymmetricPick,
            ToolbarTool::Extrude => ToolKind::Extrude,
            ToolbarTool::Revolve => ToolKind::Revolve,
            ToolbarTool::Loft => ToolKind::Loft,
            ToolbarTool::FilletChamfer => ToolKind::FilletChamfer,
            ToolbarTool::Shell => ToolKind::Shell,
            ToolbarTool::Boolean => ToolKind::Boolean,
            ToolbarTool::SectionView => ToolKind::SectionView,
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
pub const KEYBOARD_SHORTCUTS: [(&str, &str); 20] = [
    ("L", "Tool Garis"),
    ("R", "Tool Persegi"),
    ("C", "Tool Lingkaran"),
    ("E", "Tool Ellips / Extrude"),
    ("A", "Tool Arc"),
    ("O", "Tool Offset"),
    ("M", "Tool Mirror"),
    ("T", "Tool Trim"),
    ("V", "Tool Revolve"),
    ("F", "Tool Fillet & Chamfer"),
    ("S", "Tool Shell / Hollow"),
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
    SetSketchPlane(ducad_render::PlaneKind),
    EnterSketching,
    ExitSketching,
    File(FileOp),
    ClearMeasurements,
}

pub fn required_points(tool: ToolKind) -> usize {
    match tool {
        ToolKind::Rectangle
        | ToolKind::Circle
        | ToolKind::Ellipse
        | ToolKind::Mirror
        | ToolKind::Revolve
        | ToolKind::Measure => 2,
        ToolKind::Arc | ToolKind::MeasureAngle => 3,
        ToolKind::Select
        | ToolKind::Line
        | ToolKind::Offset
        | ToolKind::Trim
        | ToolKind::CoincidentPick
        | ToolKind::FixedPick
        | ToolKind::SymmetricPick
        | ToolKind::Extrude
        | ToolKind::Loft
        | ToolKind::FilletChamfer
        | ToolKind::Shell
        | ToolKind::Boolean
        | ToolKind::SectionView
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
                format!("Jarak: {:.3} mm", ducad_sketch::measure::distance(*a, *b))
            }
            Measurement::Angle { a, vertex, b } => {
                match ducad_sketch::measure::angle_degrees(*a, *vertex, *b) {
                    Some(angle) => format!("Sudut: {angle:.2}°"),
                    None => "Sudut: tidak terdefinisi (titik berimpit)".to_string(),
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
