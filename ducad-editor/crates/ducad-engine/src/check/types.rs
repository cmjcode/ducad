//! Tipe `Check`/`CheckResult` (P7.1) — kontrak JSON.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ops::Num;

fn one() -> f64 {
    1.0
}
fn tol_005() -> f64 {
    0.05
}
fn tol_002() -> f64 {
    0.02
}
fn tol_001() -> f64 {
    0.01
}

/// How a failing check is treated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// A failure blocks `ducad-cli build` (exit code 3) and package approval.
    #[default]
    Error,
    /// A failure is reported but does not block the build.
    Warning,
}

impl Severity {
    fn is_error(&self) -> bool {
        *self == Severity::Error
    }
}

/// One check with optional id/note. Note: serde does not support `flatten` + `deny_unknown_fields`,
/// so unknown fields are ignored here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CheckItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Id of the design requirement this check verifies (see tool `set_brief`), e.g. `"R2"`.
    /// Reports group the results per requirement (traceability matrix).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirement: Option<String>,
    /// Standard clause or guideline this check enforces, e.g.
    /// `"ISO 2768-1:1989, Table 1, class m"`. Printed next to the result in reports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub citation: Option<String>,
    /// `error` (default) blocks the build when the check fails; `warning` is reported only.
    #[serde(default, skip_serializing_if = "Severity::is_error")]
    pub severity: Severity,
    /// Filled in by rule packs: `"<pack>@<version>/<rule id>"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    #[serde(flatten)]
    pub check: Check,
}

impl CheckItem {
    /// Check polos tanpa id/sitasi.
    pub fn plain(check: Check) -> Self {
        Self {
            id: None,
            note: None,
            requirement: None,
            citation: None,
            severity: Severity::Error,
            rule: None,
            check,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "check", rename_all = "snake_case")]
pub enum Check {
    Valid {
        bodies: BodySel,
    },
    BodyCount {
        expect: usize,
    },
    Volume {
        body: BodySel,
        #[serde(default)]
        expect: Option<Num>,
        #[serde(default)]
        min: Option<Num>,
        #[serde(default)]
        max: Option<Num>,
        #[serde(default = "one")]
        tol_pct: f64,
    },
    /// Order X, Y, Z.
    BboxSize {
        body: BodySel,
        expect: [Num; 3],
        #[serde(default = "tol_005")]
        tol: f64,
    },
    BboxMax {
        body: BodySel,
        max: [Num; 3],
    },
    Mass {
        body: BodySel,
        #[serde(default)]
        min_g: Option<Num>,
        #[serde(default)]
        max_g: Option<Num>,
        #[serde(default)]
        density_g_cm3: Option<f64>,
    },
    MinWall {
        body: BodySel,
        min: Num,
    },
    HoleCount {
        body: BodySel,
        diameter: Num,
        #[serde(default = "tol_002")]
        tol: f64,
        expect: usize,
    },
    Clearance {
        a: String,
        b: String,
        min: Num,
    },
    NoInterference {
        bodies: BodySel,
        #[serde(default = "tol_001")]
        tol_mm3: f64,
    },
    /// Exact center of mass (mm) must be within `tol` of `expect` on every axis.
    CenterOfMass {
        body: BodySel,
        expect: [Num; 3],
        #[serde(default = "tol_005")]
        tol: f64,
    },
    /// Moment of inertia about an axis through the center of mass, in g*mm^2. Uses the body's
    /// mechanical material density (see op `set_material`) unless `density_g_cm3` is given.
    MomentOfInertia {
        body: BodySel,
        axis: InertiaAxis,
        #[serde(default)]
        min: Option<Num>,
        #[serde(default)]
        max: Option<Num>,
        #[serde(default)]
        density_g_cm3: Option<f64>,
    },
    /// Maximum von Mises stress (MPa) of a study result must not exceed `max_mpa`. `study` is the
    /// id of a `study` op (`"*"` = the only study). Status is `error` until the study has been run
    /// (tool `simulate_static`) and again after the model or the setup changes.
    MaxStress {
        #[serde(default = "default_study")]
        study: String,
        max_mpa: Num,
    },
    /// Maximum displacement magnitude (mm) of a study result must not exceed `max_mm`.
    MaxDisplacement {
        #[serde(default = "default_study")]
        study: String,
        max_mm: Num,
    },
    /// Safety factor (yield strength / max von Mises stress) of a study result must be at least
    /// `min`.
    MinSafetyFactor {
        #[serde(default = "default_study")]
        study: String,
        min: Num,
    },
    /// First natural frequency (Hz) of a `frequency` study must be at least `min_hz`.
    MinNaturalFrequency {
        #[serde(default = "default_study")]
        study: String,
        min_hz: Num,
    },
    /// Critical load factor of a `buckling` study must be at least `min` (below 1 = buckles).
    MinBucklingFactor {
        #[serde(default = "default_study")]
        study: String,
        min: Num,
    },
    /// Highest temperature (deg C) of a `thermal` study must not exceed `max_c`.
    MaxTemperature {
        #[serde(default = "default_study")]
        study: String,
        max_c: Num,
    },
    /// Sheet metal: every bend's inner radius divided by the sheet thickness must be at least
    /// `min_ratio_to_t` (e.g. 1.0 for mild steel, more for hard alloys).
    MinBendRadius {
        body: BodySel,
        min_ratio_to_t: Num,
    },
    /// Sheet metal: the straight length of every flange must be at least `min` mm (tooling needs
    /// material to grip; a common rule is 4 x thickness).
    MinFlangeLength {
        body: BodySel,
        min: Num,
    },
    /// Tolerance stack-up of a dimension chain: the total variation of the closing dimension
    /// must not exceed `max_total` (mm). `method` is `worst_case` (default, arithmetic sum) or
    /// `rss` (root-sum-square, statistical). No geometry is measured: the chain is given
    /// explicitly.
    ToleranceStackup {
        chain: Vec<StackLink>,
        max_total: Num,
        #[serde(default)]
        method: StackMethod,
    },
    /// DFM (drilling): depth / diameter of every cylindrical hole of the body must not exceed
    /// `max_ratio` (standard twist drills: about 4-5; deep-hole tooling: up to 10). Holes
    /// narrower than `min_diameter` mm are ignored.
    HoleDepthRatio {
        body: BodySel,
        max_ratio: Num,
        #[serde(default)]
        min_diameter: Option<Num>,
    },
    /// DFM: the material left between every hole wall and the nearest other surface (outer
    /// edge, pocket or neighbouring hole) must be at least `min_ratio` x the hole diameter
    /// and/or at least `min` mm. Give one or both limits. Measured on the mesh.
    HoleWeb {
        body: BodySel,
        #[serde(default)]
        min_ratio: Option<Num>,
        #[serde(default)]
        min: Option<Num>,
    },
    /// DFM (moulding/casting): every planar, cylindrical or conical face that is not a parting
    /// face must have at least `min_deg` degrees of draft relative to the `pull` axis (default
    /// `z`). Faces perpendicular to the pull direction are parting faces and pass.
    MinDraftAngle {
        body: BodySel,
        #[serde(default)]
        pull: CheckAxis,
        min_deg: Num,
    },
    /// DFM (FDM printing): the total area (mm^2) of surfaces overhanging more than `max_deg`
    /// degrees from vertical must not exceed `max_area` (default 0). `up` is the build
    /// direction (default `z`); faces lying on the build plate are not overhangs.
    MaxOverhang {
        body: BodySel,
        #[serde(default)]
        up: CheckAxis,
        #[serde(default = "deg_45")]
        max_deg: Num,
        #[serde(default)]
        max_area: Option<Num>,
    },
    /// DFM (milling): internal corners that an end mill travelling along `axis` (default `z`)
    /// has to form must have a radius of at least `min` mm. Fails on concave fillets with a
    /// smaller radius and on sharp concave edges between two walls parallel to `axis`.
    MinInternalRadius {
        body: BodySel,
        #[serde(default)]
        axis: CheckAxis,
        min: Num,
    },
    /// Thread engagement: threaded length / nominal diameter of every thread on the body (ops
    /// `thread` and tapped `hole`) must be at least `min_ratio` (rule of thumb: 1.0 in steel,
    /// 1.5-2.0 in aluminium, 2.0-2.5 in plastics).
    ThreadEngagement {
        body: BodySel,
        min_ratio: Num,
    },
    /// ISO 286 fit of a hole/shaft pair at `nominal` mm, e.g. `hole: "H7"`, `shaft: "g6"`.
    /// Passes when the fit is of the wanted `kind` (omit to only compute it) and the clearance
    /// stays inside `min_clearance`/`max_clearance` (mm, negative = interference) when given.
    Fit {
        nominal: Num,
        hole: String,
        shaft: String,
        #[serde(default)]
        kind: Option<FitKind>,
        #[serde(default)]
        min_clearance: Option<Num>,
        #[serde(default)]
        max_clearance: Option<Num>,
    },
    /// ISO 2768-1 general tolerance class of the drawing versus the manufacturing `process`:
    /// passes when the process normally holds the requested `class` (`f` fine, `m` medium,
    /// `c` coarse, `v` very coarse). The result lists the permitted deviation of the body's
    /// overall dimensions for that class.
    GeneralTolerance {
        body: BodySel,
        class: ToleranceClass,
        process: Process,
    },
}

fn deg_45() -> Num {
    Num::Value(45.0)
}

/// A world axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CheckAxis {
    X,
    Y,
    #[default]
    Z,
}

impl CheckAxis {
    pub fn unit(self) -> [f64; 3] {
        match self {
            CheckAxis::X => [1.0, 0.0, 0.0],
            CheckAxis::Y => [0.0, 1.0, 0.0],
            CheckAxis::Z => [0.0, 0.0, 1.0],
        }
    }
}

/// Character of an ISO 286 fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FitKind {
    /// Always a gap (minimum clearance >= 0).
    Clearance,
    /// Gap or interference depending on the actual sizes.
    Transition,
    /// Always interference (maximum clearance <= 0).
    Interference,
}

/// ISO 2768-1 tolerance class for linear dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ToleranceClass {
    /// Fine.
    F,
    /// Medium.
    M,
    /// Coarse.
    C,
    /// Very coarse.
    V,
}

/// Manufacturing process, used by process-dependent checks and rule packs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Process {
    /// CNC milling/turning.
    Cnc,
    /// Laser/punch cutting and press-brake bending.
    SheetMetal,
    /// Fused filament 3D printing.
    Fdm,
    /// Resin 3D printing.
    Sla,
    /// Powder-bed 3D printing.
    Sls,
    /// Injection moulding.
    Injection,
    /// Sand/investment casting.
    Casting,
}

/// One link of a tolerance chain. Give either `plus`/`minus` (amounts above/below nominal, both
/// normally >= 0) or an ISO 286 `fit` class such as `"H7"` or `"g6"`. `reverse: true` for a link
/// that runs against the chain direction (its nominal is subtracted).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StackLink {
    pub nominal: Num,
    #[serde(default)]
    pub plus: Option<Num>,
    #[serde(default)]
    pub minus: Option<Num>,
    /// ISO 286 tolerance class (holes: D E F G H JS K M N P; shafts: d e f g h js k m n p r s).
    #[serde(default)]
    pub fit: Option<String>,
    #[serde(default)]
    pub reverse: bool,
}

/// Stack-up method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StackMethod {
    #[default]
    WorstCase,
    Rss,
}

fn default_study() -> String {
    "*".to_string()
}

/// Axis through the center of mass: a global axis or a principal axis (sorted by moment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InertiaAxis {
    X,
    Y,
    Z,
    PrincipalMin,
    PrincipalMid,
    PrincipalMax,
}

impl Check {
    pub fn kind(&self) -> &'static str {
        match self {
            Check::Valid { .. } => "valid",
            Check::BodyCount { .. } => "body_count",
            Check::Volume { .. } => "volume",
            Check::BboxSize { .. } => "bbox_size",
            Check::BboxMax { .. } => "bbox_max",
            Check::Mass { .. } => "mass",
            Check::MinWall { .. } => "min_wall",
            Check::HoleCount { .. } => "hole_count",
            Check::Clearance { .. } => "clearance",
            Check::NoInterference { .. } => "no_interference",
            Check::CenterOfMass { .. } => "center_of_mass",
            Check::MomentOfInertia { .. } => "moment_of_inertia",
            Check::MaxStress { .. } => "max_stress",
            Check::MaxDisplacement { .. } => "max_displacement",
            Check::MinSafetyFactor { .. } => "min_safety_factor",
            Check::MinNaturalFrequency { .. } => "min_natural_frequency",
            Check::MinBucklingFactor { .. } => "min_buckling_factor",
            Check::MaxTemperature { .. } => "max_temperature",
            Check::MinBendRadius { .. } => "min_bend_radius",
            Check::MinFlangeLength { .. } => "min_flange_length",
            Check::ToleranceStackup { .. } => "tolerance_stackup",
            Check::HoleDepthRatio { .. } => "hole_depth_ratio",
            Check::HoleWeb { .. } => "hole_web",
            Check::MinDraftAngle { .. } => "min_draft_angle",
            Check::MaxOverhang { .. } => "max_overhang",
            Check::MinInternalRadius { .. } => "min_internal_radius",
            Check::ThreadEngagement { .. } => "thread_engagement",
            Check::Fit { .. } => "fit",
            Check::GeneralTolerance { .. } => "general_tolerance",
        }
    }
}

/// `"*"` in field `body` = the only body (error if the count ≠ 1); `"*"` in field `bodies` = all
/// bodies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum BodySel {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Pass,
    Fail,
    /// Check tidak bisa dievaluasi (body tak dikenal, kernel gagal) —
    /// dihitung sebagai tidak lulus.
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckResult {
    pub index: usize,
    pub id: Option<String>,
    pub kind: &'static str,
    pub status: CheckStatus,
    pub measured: serde_json::Value,
    pub expected: serde_json::Value,
    /// Bahasa Indonesia, menyebut angka.
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<[f64; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// Id persyaratan yang diverifikasi check ini (keterlacakan).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirement: Option<String>,
    /// Pasal standar / pedoman yang ditegakkan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub citation: Option<String>,
    #[serde(default)]
    pub severity: Severity,
    /// `"<paket>@<versi>/<aturan>"` bila berasal dari paket aturan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
}

impl CheckResult {
    /// Check ini tidak lulus DAN menghalangi build (severity `error`).
    pub fn is_blocking(&self) -> bool {
        self.status != CheckStatus::Pass && self.severity == Severity::Error
    }
}
