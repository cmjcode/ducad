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
