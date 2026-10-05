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

/// One check with optional id/note. Note: serde does not support `flatten` + `deny_unknown_fields`,
/// so unknown fields are ignored here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CheckItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(flatten)]
    pub check: Check,
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
}
