use ducad_core::LengthUnit;
use ducad_kernel::PickRay;
use ducad_ui::{SelectedEntityData, ToolbarTool};
use glam::{DVec2, Vec3};

/// Target undo lintas mode (Sketsa 2D, Model 3D, atau Tinta Ink).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UndoTarget {
    Sketch,
    Model,
    Ink,
}

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
    /// Titik sketch mandiri (Paritas 1.1): satu klik = satu titik.
    Point,
    /// Coretan bebas (Pencil/mouse) → bentuk rapi ber-constraint (P12.3).
    Freehand,
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
    /// Perpanjang segmen garis sampai kurva batas terdekat (Extend).
    Extend,
    /// Pecah entitas di titik klik menjadi dua (Paritas 1.13).
    Break,
    /// Constraint Coincident.
    CoincidentPick,
    /// Constraint Fixed.
    FixedPick,
    /// Constraint Symmetric.
    SymmetricPick,
    /// Pattern / Array (Linier X/Y/Z & Sirkular putar).
    Pattern,
    // Mode Vektor (M2.1)
    PenBezier,
    NodeEdit,
    ShapeBuilder,
    Eyedropper,
    Gradient,
    // 3D Solid tools
    Extrude,
    /// Revolve 360° penuh atau sudut custom.
    Revolve,
    Loft,
    Sweep,
    /// Kurva spiral & generator solid pegas/ulir 3D (Helix / Coil — Fase 10.2).
    Helix,
    Shell,
    /// Tulang Penguat (Rib / Stiffener Support — Fase 2.4).
    Rib,
    /// Draft Angle — kemiringan cetakan plastik (injection molding, Fase 2.1).
    DraftAngle,
    /// Split Body & Split Face (Fase 2.2 — Potong Benda).
    SplitBody,
    Boolean,
    /// Bidang Referensi Bebas 3D (Datum Reference Plane — Fase 10.1).
    DatumPlane,
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
            ToolKind::Point => ToolbarTool::Point,
            ToolKind::Freehand => ToolbarTool::Freehand,
            ToolKind::Text => ToolbarTool::Text,
            ToolKind::Fillet2D => ToolbarTool::Fillet2D,
            ToolKind::Chamfer2D => ToolbarTool::Chamfer2D,
            ToolKind::Offset => ToolbarTool::Offset,
            ToolKind::Mirror => ToolbarTool::Mirror,
            ToolKind::Trim => ToolbarTool::Trim,
            ToolKind::Extend => ToolbarTool::Extend,
            ToolKind::Break => ToolbarTool::Break,
            ToolKind::CoincidentPick => ToolbarTool::PointCoincident,
            ToolKind::FixedPick => ToolbarTool::PointFixed,
            ToolKind::SymmetricPick => ToolbarTool::PointSymmetric,
            ToolKind::Pattern => ToolbarTool::Pattern,
            ToolKind::PenBezier => ToolbarTool::PenBezier,
            ToolKind::NodeEdit => ToolbarTool::NodeEdit,
            ToolKind::ShapeBuilder => ToolbarTool::ShapeBuilder,
            ToolKind::Eyedropper => ToolbarTool::Eyedropper,
            ToolKind::Gradient => ToolbarTool::Gradient,
            ToolKind::Extrude => ToolbarTool::Extrude,
            ToolKind::Revolve => ToolbarTool::Revolve,
            ToolKind::Loft => ToolbarTool::Loft,
            ToolKind::Sweep => ToolbarTool::Sweep,
            ToolKind::Helix => ToolbarTool::Helix,
            ToolKind::Shell => ToolbarTool::Shell,
            ToolKind::Rib => ToolbarTool::Rib,
            ToolKind::DraftAngle => ToolbarTool::DraftAngle,
            ToolKind::SplitBody => ToolbarTool::SplitBody,
            ToolKind::Boolean => ToolbarTool::Boolean,
            ToolKind::DatumPlane => ToolbarTool::DatumPlane,
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
            ToolbarTool::Point => ToolKind::Point,
            ToolbarTool::Freehand => ToolKind::Freehand,
            ToolbarTool::Text => ToolKind::Text,
            ToolbarTool::Fillet2D => ToolKind::Fillet2D,
            ToolbarTool::Chamfer2D => ToolKind::Chamfer2D,
            ToolbarTool::Offset => ToolKind::Offset,
            ToolbarTool::Mirror => ToolKind::Mirror,
            ToolbarTool::Trim => ToolKind::Trim,
            ToolbarTool::Extend => ToolKind::Extend,
            ToolbarTool::Break => ToolKind::Break,
            ToolbarTool::PointCoincident => ToolKind::CoincidentPick,
            ToolbarTool::PointFixed => ToolKind::FixedPick,
            ToolbarTool::PointSymmetric => ToolKind::SymmetricPick,
            ToolbarTool::Pattern => ToolKind::Pattern,
            ToolbarTool::PenBezier => ToolKind::PenBezier,
            ToolbarTool::NodeEdit => ToolKind::NodeEdit,
            ToolbarTool::ShapeBuilder => ToolKind::ShapeBuilder,
            ToolbarTool::Eyedropper => ToolKind::Eyedropper,
            ToolbarTool::Gradient => ToolKind::Gradient,
            ToolbarTool::Extrude => ToolKind::Extrude,
            ToolbarTool::Revolve => ToolKind::Revolve,
            ToolbarTool::Loft => ToolKind::Loft,
            ToolbarTool::Sweep => ToolKind::Sweep,
            ToolbarTool::Helix => ToolKind::Helix,
            ToolbarTool::Shell => ToolKind::Shell,
            ToolbarTool::Rib => ToolKind::Rib,
            ToolbarTool::DraftAngle => ToolKind::DraftAngle,
            ToolbarTool::SplitBody => ToolKind::SplitBody,
            ToolbarTool::Boolean => ToolKind::Boolean,
            ToolbarTool::DatumPlane => ToolKind::DatumPlane,
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
pub const KEYBOARD_SHORTCUTS: [(&str, &str); 24] = [
    (
        "Cmd+Shift+P / Cmd+K",
        "Buka Command Palette (Pencarian Perintah)",
    ),
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
    ImportStl,
    ImportDxf,
    ExportStep,
    ExportStl,
    ExportObj,
    ExportGlb,
    ExportDxf,
    ExportSvg,
    ExportPdf,
    ExportDrawingDxf,
    ExportDrawingSvg,
    OpenDrawingSheet,
}

#[derive(Debug, Clone, Copy)]
pub enum PaletteAction {
    /// Buka/tutup panel properti massa (P16).
    ToggleMassPanel,
    /// Buka/tutup panel simulasi (P17).
    ToggleSimPanel,
    /// Buka/tutup panel Fitur Industri (P18–P20).
    ToggleIndustryPanel,
    /// Buka/tutup panel Review & Rilis (brief, redline, persetujuan).
    ToggleReviewPanel,
    /// Buka layar "Proyek baru dari brief".
    OpenBriefStudio,
    SetTool(ToolKind),
    CreateDatumPlane,
    OpenRevolveDialog,
    Undo,
    Redo,
    ModelUndo,
    ModelRedo,
    DeleteSelection,
    ToggleTheme,
    /// Nyalakan/matikan material Liquid Glass pada panel.
    ToggleLiquidGlass,
    /// Nyalakan/matikan "Kurangi transparansi".
    ToggleReduceTransparency,
    ToggleZebraView,
    ToggleStudioLighting,
    ToggleConstruction,
    OpenDrawingSheet,
    SetSketchPlane(ducad_render::PlaneKind),
    EnterSketching,
    ExitSketching,
    File(FileOp),
    ClearMeasurements,
    SetTouchDesignMode(ducad_ui::TouchDesignMode),
    TogglePalmRejection,
    /// Buka dialog asisten AI lokal (P11.4).
    OpenAssist,
    OpenChat,
    /// Nyalakan/matikan jembatan agent live (P5.1).
    ToggleAgentBridge,
    /// Pindah mode aplikasi (Sketsa CAD / Vektor / Tinta / 3D).
    SetAppMode(crate::mode::AppMode),
    /// Sketsa/tinta → objek tertutup (tombol HUD "Objek Tertutup").
    ConvertToClosedObjects,
    /// Nyalakan/matikan opsi kuas "Bentuk Pintar".
    ToggleInkSmartShape,
    /// Buka tutorial selamat datang dari awal.
    StartTutorial,
    /// Periksa rilis baru di GitHub Releases (desktop).
    CheckForUpdates,
    /// Extrude profil sketsa terpilih, atau tarik sisi solid terpilih.
    ExtrudeSelection,
    /// Perintah dari plan paritas SolidCraft (Fase 0).
    Parity(ParityCmd),
}

/// Perintah Fase 0 plan paritas (`docs/PLAN_PARITAS_SOLIDCRAFT.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParityCmd {
    // ---- Paritas 1.13–1.15, 1.18, 1.19 ----
    SketchBreak,
    ProjectEdges,
    IntersectBody,
    BreakLink,
    CopySketch,
    CutSketch,
    PasteSketch,
    ToggleGlyphs,
    InsertSvg,
    Primitive,
    MirrorBody,
    RotateBody,
    AutoConstrain,
    EditText,
    ToggleDiameter,
    LooseEnds,
    Parameters,
    Inspect,
    CopyOplog,
    ExportPng,
    Compare,
    RigidGroup,
    DissolveRigidGroup,
    // Fase 1 (1.1, 1.16, 1.17, 1.20)
    SketchPoint,
    ToggleDriven,
    ResetDimText,
    CurvatureG2,
    // Fase 1A (1.2–1.5): geometri konstruksi datum
    DatumAxis,
    DatumPoint,
    DatumDrawer,
    // Fase 2: direct editing & alat solid (`direct_tools.rs`).
    Direct(crate::direct_tools::DirectTool),
    // Fase 3: berkas, interop, keselamatan data.
    ExportThreeMf,
    OpenInSlicer,
    ImportMesh,
    ExportIges,
    ImportIges,
    ExportViewDxf,
    RecoverableDesigns,
    NamedViews,
    CrashReports,
    // Fase 4: perakitan & gerak.
    Joint(ducad_core::assembly::JointKind),
    AssemblyTools,
    MotionTools,
    CapturePositions,
    RevertPositions,
    CopyOccurrence,
    ContactSet,
    // Fase 5: inspeksi & analisis.
    InspectTools,
    CurvatureMap,
    // Fase 6A: permukaan.
    SurfaceTools,
    // Fase 6B: plastik.
    PlasticTools,
}

impl ParityCmd {
    pub const ALL: [ParityCmd; 67] = [
        ParityCmd::SketchBreak,
        ParityCmd::ProjectEdges,
        ParityCmd::IntersectBody,
        ParityCmd::BreakLink,
        ParityCmd::CopySketch,
        ParityCmd::CutSketch,
        ParityCmd::PasteSketch,
        ParityCmd::ToggleGlyphs,
        ParityCmd::InsertSvg,
        ParityCmd::RigidGroup,
        ParityCmd::DissolveRigidGroup,
        ParityCmd::ExportPng,
        ParityCmd::Compare,
        ParityCmd::Parameters,
        ParityCmd::Inspect,
        ParityCmd::CopyOplog,
        ParityCmd::LooseEnds,
        ParityCmd::Primitive,
        ParityCmd::MirrorBody,
        ParityCmd::RotateBody,
        ParityCmd::AutoConstrain,
        ParityCmd::EditText,
        ParityCmd::ToggleDiameter,
        ParityCmd::SketchPoint,
        ParityCmd::ToggleDriven,
        ParityCmd::ResetDimText,
        ParityCmd::CurvatureG2,
        ParityCmd::DatumAxis,
        ParityCmd::DatumPoint,
        ParityCmd::DatumDrawer,
        ParityCmd::Direct(crate::direct_tools::DirectTool::OffsetFace),
        ParityCmd::Direct(crate::direct_tools::DirectTool::DeleteFace),
        ParityCmd::Direct(crate::direct_tools::DirectTool::ReplaceFace),
        ParityCmd::Direct(crate::direct_tools::DirectTool::Align),
        ParityCmd::Direct(crate::direct_tools::DirectTool::Pipe),
        ParityCmd::Direct(crate::direct_tools::DirectTool::PatternPath),
        ParityCmd::Direct(crate::direct_tools::DirectTool::Web),
        ParityCmd::Direct(crate::direct_tools::DirectTool::Emboss),
        ParityCmd::Direct(crate::direct_tools::DirectTool::BoundaryFill),
        ParityCmd::Direct(crate::direct_tools::DirectTool::BoundingSolid),
        ParityCmd::ExportThreeMf,
        ParityCmd::OpenInSlicer,
        ParityCmd::ImportMesh,
        ParityCmd::ExportIges,
        ParityCmd::ImportIges,
        ParityCmd::ExportViewDxf,
        ParityCmd::RecoverableDesigns,
        ParityCmd::NamedViews,
        ParityCmd::CrashReports,
        ParityCmd::Joint(ducad_core::assembly::JointKind::Rigid),
        ParityCmd::Joint(ducad_core::assembly::JointKind::Revolute),
        ParityCmd::Joint(ducad_core::assembly::JointKind::Slider),
        ParityCmd::Joint(ducad_core::assembly::JointKind::Cylindrical),
        ParityCmd::Joint(ducad_core::assembly::JointKind::Planar),
        ParityCmd::Joint(ducad_core::assembly::JointKind::Ball),
        ParityCmd::Joint(ducad_core::assembly::JointKind::PinSlot),
        ParityCmd::Joint(ducad_core::assembly::JointKind::Cam),
        ParityCmd::AssemblyTools,
        ParityCmd::MotionTools,
        ParityCmd::CapturePositions,
        ParityCmd::RevertPositions,
        ParityCmd::CopyOccurrence,
        ParityCmd::ContactSet,
        ParityCmd::InspectTools,
        ParityCmd::CurvatureMap,
        ParityCmd::SurfaceTools,
        ParityCmd::PlasticTools,
    ];

    pub fn label(self) -> String {
        match self {
            ParityCmd::SketchBreak => ducad_i18n::t!("tool-break"),
            ParityCmd::ProjectEdges => ducad_i18n::t!("cmd-project-edges"),
            ParityCmd::IntersectBody => ducad_i18n::t!("cmd-intersect-body"),
            ParityCmd::BreakLink => ducad_i18n::t!("cmd-break-link"),
            ParityCmd::CopySketch => ducad_i18n::t!("cmd-sketch-copy"),
            ParityCmd::CutSketch => ducad_i18n::t!("cmd-sketch-cut"),
            ParityCmd::PasteSketch => ducad_i18n::t!("cmd-sketch-paste"),
            ParityCmd::ToggleGlyphs => ducad_i18n::t!("cmd-toggle-glyphs"),
            ParityCmd::InsertSvg => ducad_i18n::t!("svg-insert-title"),
            ParityCmd::Primitive => ducad_i18n::t!("prim-title"),
            ParityCmd::MirrorBody => ducad_i18n::t!("mirror-body-title"),
            ParityCmd::RotateBody => ducad_i18n::t!("rotate-body-title"),
            ParityCmd::AutoConstrain => ducad_i18n::t!("cmd-auto-constrain"),
            ParityCmd::EditText => ducad_i18n::t!("text-edit-title"),
            ParityCmd::ToggleDiameter => ducad_i18n::t!("cmd-toggle-diameter"),
            ParityCmd::LooseEnds => ducad_i18n::t!("cmd-loose-ends"),
            ParityCmd::Parameters => ducad_i18n::t!("params-title"),
            ParityCmd::Inspect => ducad_i18n::t!("inspect-title"),
            ParityCmd::CopyOplog => ducad_i18n::t!("oplog-copy"),
            ParityCmd::ExportPng => ducad_i18n::t!("png-title"),
            ParityCmd::Compare => ducad_i18n::t!("compare-title"),
            ParityCmd::RigidGroup => ducad_i18n::t!("rigid-group-make"),
            ParityCmd::DissolveRigidGroup => ducad_i18n::t!("rigid-group-dissolve"),
            ParityCmd::SketchPoint => ducad_i18n::t!("tool-point"),
            ParityCmd::ToggleDriven => ducad_i18n::t!("cmd-toggle-driven"),
            ParityCmd::ResetDimText => ducad_i18n::t!("cmd-reset-dim-text"),
            ParityCmd::CurvatureG2 => ducad_i18n::t!("constraint-curvature-tip"),
            ParityCmd::DatumAxis => ducad_i18n::t!("cmd-datum-axis"),
            ParityCmd::DatumPoint => ducad_i18n::t!("cmd-datum-point"),
            ParityCmd::DatumDrawer => ducad_i18n::t!("cmd-datum-drawer"),
            ParityCmd::Direct(t) => t.label(),
            ParityCmd::ExportThreeMf => ducad_i18n::t!("cmd-export-3mf"),
            ParityCmd::OpenInSlicer => ducad_i18n::t!("cmd-open-in-slicer"),
            ParityCmd::ImportMesh => ducad_i18n::t!("cmd-import-mesh"),
            ParityCmd::ExportIges => ducad_i18n::t!("cmd-export-iges"),
            ParityCmd::ImportIges => ducad_i18n::t!("cmd-import-iges"),
            ParityCmd::ExportViewDxf => ducad_i18n::t!("cmd-export-view-dxf"),
            ParityCmd::RecoverableDesigns => ducad_i18n::t!("recovery-title"),
            ParityCmd::NamedViews => ducad_i18n::t!("views-title"),
            ParityCmd::CrashReports => ducad_i18n::t!("crash-title"),
            ParityCmd::Joint(k) => ducad_i18n::t!("cmd-joint", kind = k.label()),
            ParityCmd::AssemblyTools => ducad_i18n::t!("asm-tools-title"),
            ParityCmd::MotionTools => ducad_i18n::t!("motion-title"),
            ParityCmd::CapturePositions => ducad_i18n::t!("asm-capture"),
            ParityCmd::RevertPositions => ducad_i18n::t!("asm-revert"),
            ParityCmd::CopyOccurrence => ducad_i18n::t!("asm-occurrence-copy"),
            ParityCmd::ContactSet => ducad_i18n::t!("asm-contact-add"),
            ParityCmd::InspectTools => ducad_i18n::t!("inspect-tools-title"),
            ParityCmd::CurvatureMap => ducad_i18n::t!("inspect-curvature-map"),
            ParityCmd::SurfaceTools => ducad_i18n::t!("surface-title"),
            ParityCmd::PlasticTools => ducad_i18n::t!("plastic-title"),
        }
    }

    pub fn group(self) -> PaletteGroup {
        match self {
            ParityCmd::SketchBreak
            | ParityCmd::ProjectEdges
            | ParityCmd::IntersectBody
            | ParityCmd::BreakLink
            | ParityCmd::ToggleGlyphs => PaletteGroup::Sketch,
            ParityCmd::CopySketch | ParityCmd::CutSketch | ParityCmd::PasteSketch => PaletteGroup::Edit,
            ParityCmd::InsertSvg => PaletteGroup::File,
            ParityCmd::Primitive
            | ParityCmd::MirrorBody
            | ParityCmd::RotateBody
            | ParityCmd::RigidGroup
            | ParityCmd::DissolveRigidGroup
            | ParityCmd::DatumAxis
            | ParityCmd::DatumPoint => PaletteGroup::Solid,
            ParityCmd::DatumDrawer => PaletteGroup::View,
            ParityCmd::Direct(_) => PaletteGroup::Solid,
            ParityCmd::ExportThreeMf => PaletteGroup::File,
            ParityCmd::OpenInSlicer => PaletteGroup::File,
            ParityCmd::ImportMesh => PaletteGroup::File,
            ParityCmd::ExportIges => PaletteGroup::File,
            ParityCmd::ImportIges => PaletteGroup::File,
            ParityCmd::ExportViewDxf => PaletteGroup::File,
            ParityCmd::RecoverableDesigns => PaletteGroup::File,
            ParityCmd::NamedViews => PaletteGroup::View,
            ParityCmd::CrashReports => PaletteGroup::Analysis,
            ParityCmd::Joint(_) => PaletteGroup::Solid,
            ParityCmd::AssemblyTools => PaletteGroup::Solid,
            ParityCmd::MotionTools => PaletteGroup::Analysis,
            ParityCmd::CapturePositions => PaletteGroup::Solid,
            ParityCmd::RevertPositions => PaletteGroup::Solid,
            ParityCmd::CopyOccurrence => PaletteGroup::Solid,
            ParityCmd::ContactSet => PaletteGroup::Solid,
            ParityCmd::InspectTools | ParityCmd::CurvatureMap => PaletteGroup::Analysis,
            ParityCmd::SurfaceTools | ParityCmd::PlasticTools => PaletteGroup::Solid,
            ParityCmd::AutoConstrain
            | ParityCmd::EditText
            | ParityCmd::ToggleDiameter
            | ParityCmd::LooseEnds
            | ParityCmd::SketchPoint
            | ParityCmd::ToggleDriven
            | ParityCmd::ResetDimText
            | ParityCmd::CurvatureG2 => PaletteGroup::Sketch,
            ParityCmd::Parameters => PaletteGroup::Edit,
            ParityCmd::Inspect | ParityCmd::CopyOplog | ParityCmd::Compare => PaletteGroup::Analysis,
            ParityCmd::ExportPng => PaletteGroup::File,
        }
    }
}

/// Grup perintah di command palette dan burger menu; urutan varian =
/// urutan tampil.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PaletteGroup {
    Ai,
    Mode,
    File,
    Sketch,
    Solid,
    Analysis,
    Edit,
    View,
}

impl PaletteGroup {
    pub const ALL: [PaletteGroup; 8] = [
        PaletteGroup::Ai,
        PaletteGroup::Mode,
        PaletteGroup::File,
        PaletteGroup::Sketch,
        PaletteGroup::Solid,
        PaletteGroup::Analysis,
        PaletteGroup::Edit,
        PaletteGroup::View,
    ];

    pub fn title(self) -> String {
        match self {
            PaletteGroup::Ai => ducad_i18n::t!("cmd-group-ai"),
            PaletteGroup::Mode => ducad_i18n::t!("cmd-group-mode"),
            PaletteGroup::File => ducad_i18n::t!("cmd-group-file"),
            PaletteGroup::Sketch => ducad_i18n::t!("cmd-group-sketch"),
            PaletteGroup::Solid => ducad_i18n::t!("cmd-group-solid"),
            PaletteGroup::Analysis => ducad_i18n::t!("cmd-group-analysis"),
            PaletteGroup::Edit => ducad_i18n::t!("cmd-group-edit"),
            PaletteGroup::View => ducad_i18n::t!("cmd-group-view"),
        }
    }

    pub fn icon(self) -> &'static str {
        use egui_icons::icons::*;
        match self {
            PaletteGroup::Ai => ICON_AUTO_AWESOME.codepoint,
            PaletteGroup::Mode => ICON_LAYERS.codepoint,
            PaletteGroup::File => ICON_DESCRIPTION.codepoint,
            PaletteGroup::Sketch => ICON_EDIT.codepoint,
            PaletteGroup::Solid => ICON_VIEW_IN_AR.codepoint,
            PaletteGroup::Analysis => ICON_STRAIGHTEN.codepoint,
            PaletteGroup::Edit => ICON_UNDO.codepoint,
            PaletteGroup::View => ICON_TUNE.codepoint,
        }
    }
}

impl PaletteAction {
    pub fn group(self) -> PaletteGroup {
        match self {
            PaletteAction::OpenAssist
            | PaletteAction::OpenBriefStudio
            | PaletteAction::OpenChat
            | PaletteAction::ToggleAgentBridge => PaletteGroup::Ai,
            PaletteAction::SetAppMode(_)
            | PaletteAction::EnterSketching
            | PaletteAction::ExitSketching
            | PaletteAction::SetSketchPlane(_)
            | PaletteAction::CreateDatumPlane => PaletteGroup::Mode,
            PaletteAction::File(_) | PaletteAction::OpenDrawingSheet => PaletteGroup::File,
            PaletteAction::SetTool(ToolKind::Measure | ToolKind::MeasureAngle)
            | PaletteAction::ClearMeasurements
            | PaletteAction::ToggleMassPanel
            | PaletteAction::ToggleSimPanel
            | PaletteAction::ToggleIndustryPanel
            | PaletteAction::ToggleReviewPanel
            | PaletteAction::ToggleZebraView => PaletteGroup::Analysis,
            PaletteAction::SetTool(
                ToolKind::Extrude
                | ToolKind::Revolve
                | ToolKind::Loft
                | ToolKind::Sweep
                | ToolKind::Helix
                | ToolKind::Shell
                | ToolKind::Rib
                | ToolKind::DraftAngle
                | ToolKind::SplitBody
                | ToolKind::Boolean
                | ToolKind::DatumPlane,
            )
            | PaletteAction::OpenRevolveDialog
            | PaletteAction::ExtrudeSelection => PaletteGroup::Solid,
            PaletteAction::SetTool(_)
            | PaletteAction::ToggleConstruction
            | PaletteAction::ConvertToClosedObjects
            | PaletteAction::ToggleInkSmartShape => PaletteGroup::Sketch,
            PaletteAction::Undo
            | PaletteAction::Redo
            | PaletteAction::ModelUndo
            | PaletteAction::ModelRedo
            | PaletteAction::DeleteSelection => PaletteGroup::Edit,
            PaletteAction::ToggleTheme
            | PaletteAction::ToggleLiquidGlass
            | PaletteAction::ToggleReduceTransparency
            | PaletteAction::ToggleStudioLighting
            | PaletteAction::SetTouchDesignMode(_)
            | PaletteAction::TogglePalmRejection
            | PaletteAction::StartTutorial
            | PaletteAction::CheckForUpdates => PaletteGroup::View,
            PaletteAction::Parity(cmd) => cmd.group(),
        }
    }
}

pub fn required_points(tool: ToolKind) -> usize {
    match tool {
        // Freehand tidak memakai klik titik: satu coretan penuh.
        ToolKind::Freehand => 0,
        ToolKind::Text | ToolKind::Point => 1,
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
        | ToolKind::Extend
        | ToolKind::Break
        | ToolKind::CoincidentPick
        | ToolKind::FixedPick
        | ToolKind::SymmetricPick
        | ToolKind::Pattern
        | ToolKind::Extrude
        | ToolKind::Loft
        | ToolKind::Sweep
        | ToolKind::Helix
        | ToolKind::Shell
        | ToolKind::Rib
        | ToolKind::DraftAngle
        | ToolKind::SplitBody
        | ToolKind::Boolean
        | ToolKind::SectionView
        | ToolKind::ZebraInspection
        | ToolKind::DraftAnalysis
        | ToolKind::HoleWizard
        | ToolKind::DatumPlane
        | ToolKind::PenBezier
        | ToolKind::NodeEdit
        | ToolKind::ShapeBuilder
        | ToolKind::Eyedropper
        | ToolKind::Gradient
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
            Measurement::Angle { a, vertex, b } => {
                ducad_sketch::measure::angle_degrees(*a, *vertex, *b)
                    .map(|deg| format!("{deg:.1}°"))
            }
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

/// Batas extrude profil dari GUI (op `extrude`): satu arah sejauh jarak,
/// simetris (jarak = tebal total), atau sampai face sasaran (`up_to`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ExtrudeExtentMode {
    #[default]
    Blind,
    Symmetric,
    UpToFace,
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RoundKind {
    Vertex,
    Edge,
}

/// Gaya rounding: `Fillet` atau `Chamfer`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RoundStyle {
    Fillet,
    Chamfer,
}

#[derive(Clone, Debug)]
pub struct RoundFeature {
    pub kind: RoundKind,
    pub style: RoundStyle,
    pub ray: PickRay,
    pub anchor: (f64, f64, f64),
    pub radius: f64,
    pub radius_end: Option<f64>,
    pub polyline: Vec<(f64, f64, f64)>,
}

impl From<&RoundFeature> for ducad_io::native::NativeRoundFeature {
    fn from(f: &RoundFeature) -> Self {
        ducad_io::native::NativeRoundFeature {
            kind: match f.kind {
                RoundKind::Vertex => ducad_io::native::NativeRoundKind::Vertex,
                RoundKind::Edge => ducad_io::native::NativeRoundKind::Edge,
            },
            style: match f.style {
                RoundStyle::Fillet => ducad_io::native::NativeRoundStyle::Fillet,
                RoundStyle::Chamfer => ducad_io::native::NativeRoundStyle::Chamfer,
            },
            ray_origin: f.ray.origin,
            ray_dir: f.ray.dir,
            anchor: f.anchor,
            radius: f.radius,
            radius_end: f.radius_end,
            polyline: f.polyline.clone(),
        }
    }
}

impl From<ducad_io::native::NativeRoundFeature> for RoundFeature {
    fn from(f: ducad_io::native::NativeRoundFeature) -> Self {
        RoundFeature {
            kind: match f.kind {
                ducad_io::native::NativeRoundKind::Vertex => RoundKind::Vertex,
                ducad_io::native::NativeRoundKind::Edge => RoundKind::Edge,
            },
            style: match f.style {
                ducad_io::native::NativeRoundStyle::Fillet => RoundStyle::Fillet,
                ducad_io::native::NativeRoundStyle::Chamfer => RoundStyle::Chamfer,
            },
            ray: ducad_kernel::PickRay {
                origin: f.ray_origin,
                dir: f.ray_dir,
            },
            anchor: f.anchor,
            radius: f.radius,
            radius_end: f.radius_end,
            polyline: f.polyline,
        }
    }
}

/// Edit lain yang dilakukan pada body SESUDAH fillet/chamfer gizmo. Disimpan
/// supaya rounding tetap bisa diedit: edit rounding memutar ulang
/// `base → rounding → edit susulan` berurutan, bukan membuang riwayat.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RoundPostOp {
    /// Tarik/potong sisi (push/pull) lewat `ducad_kernel::extrude_face`.
    ExtrudeFace { ray: PickRay, distance: f64 },
}

/// [`RoundPostOp`] beserta posisinya: dijalankan sesudah `after` fitur
/// rounding pertama.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoundPostStep {
    pub after: usize,
    pub op: RoundPostOp,
}

/// Fillet/chamfer gizmo yang tercatat sebagai op oplog (`fillet`,
/// `chamfer`, `round_vertex`). Radius/gaya dibaca dari op-nya; posisi
/// disimpan dalam kerangka SAAT op dibuat dan diikutkan geser/putar/skala
/// body sesudahnya (lihat `round_ops`).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RoundMarker {
    pub op_id: String,
    /// `true` = rounding sudut (vertex), selain itu rusuk.
    pub vertex: bool,
    pub ray_origin: (f64, f64, f64),
    pub ray_dir: (f64, f64, f64),
    pub anchor: (f64, f64, f64),
    #[serde(default)]
    pub polyline: Vec<(f64, f64, f64)>,
}

impl RoundMarker {
    pub fn ray(&self) -> PickRay {
        PickRay { origin: self.ray_origin, dir: self.ray_dir }
    }
}

pub struct RoundHistory {
    pub base: ducad_kernel::KernelShape,
    pub features: Vec<RoundFeature>,
    pub post_ops: Vec<RoundPostStep>,
    /// `mesh_fingerprint` geometri yang dihasilkan riwayat ini. Beda dengan
    /// geometri body sekarang (undo/redo, edit lain) = riwayat basi.
    pub result_fingerprint: u64,
}

impl From<&RoundPostStep> for ducad_io::native::NativeRoundPostStep {
    fn from(s: &RoundPostStep) -> Self {
        match s.op {
            RoundPostOp::ExtrudeFace { ray, distance } => ducad_io::native::NativeRoundPostStep {
                after: s.after,
                op: ducad_io::native::NativeRoundPostOp::ExtrudeFace {
                    ray_origin: ray.origin,
                    ray_dir: ray.dir,
                    distance,
                },
            },
        }
    }
}

impl From<ducad_io::native::NativeRoundPostStep> for RoundPostStep {
    fn from(s: ducad_io::native::NativeRoundPostStep) -> Self {
        match s.op {
            ducad_io::native::NativeRoundPostOp::ExtrudeFace { ray_origin, ray_dir, distance } => {
                RoundPostStep {
                    after: s.after,
                    op: RoundPostOp::ExtrudeFace {
                        ray: PickRay { origin: ray_origin, dir: ray_dir },
                        distance,
                    },
                }
            }
        }
    }
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
    /// `mesh_fingerprint` geometri hasil riwayat ini. Beda dengan body
    /// sekarang = body sudah diedit sesudahnya; membangun ulang dari `base`
    /// akan MEMBUANG edit itu, jadi jalur lama tidak boleh dipakai.
    pub result_fingerprint: u64,
}

/// Animasi transisi orientasi kamera (yaw/pitch) bergaya CAD (M3.4).
#[derive(Debug, Clone)]
pub struct CameraAnimation {
    pub start_yaw: f32,
    pub start_pitch: f32,
    pub target_yaw: f32,
    pub target_pitch: f32,
    pub start_time: std::time::Instant,
    pub duration_ms: u64,
}
