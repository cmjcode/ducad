//! Setup studi: bentuk serde yang disimpan di oplog desain (dibaca agent,
//! karena itu doc comment-nya berbahasa Inggris) dan bentuk "resolved" yang
//! selector face-nya sudah diubah engine menjadi indeks face.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Linear static study setup: which body to analyse, how it is held
/// (fixtures) and what acts on it (loads). Units: mm, N, MPa, m/s^2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SimSetup {
    /// Name of the body to analyse (the id of the op that created it).
    pub body: String,
    /// Supports. At least one is required; together they must block all six
    /// rigid-body motions, otherwise the run fails with SIM_UNDERCONSTRAINED.
    pub fixtures: Vec<Fixture>,
    /// Loads applied to the body. All loads act together in one load case.
    pub loads: Vec<Load>,
    /// Mesh controls. Omit to let the solver pick a cell size automatically.
    #[serde(default)]
    pub mesh: MeshSettings,
}

/// A support applied to one or more faces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    /// Unique id of this fixture; reaction forces are reported per id.
    pub id: String,
    /// Face selector (same grammar as modeling ops, e.g. "<X" or
    /// "all[kind=cylinder][r=5]").
    pub faces: String,
    /// How the selected faces are held.
    pub kind: FixtureKind,
}

/// Kind of support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FixtureKind {
    /// All three displacement components are zero on the face.
    Fixed,
    /// Displacement along the face normal is zero; sliding in the face is free.
    Roller,
    /// Symmetry plane: same constraint as `roller` (no displacement along the
    /// face normal). Use it on the cut face of a half or quarter model.
    Symmetry,
}

/// A load. `kind` selects the load type and its parameters, for example
/// `{"id":"f1","faces":">Z","kind":"force","newton":[0,0,-100]}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Load {
    /// Unique id of this load.
    pub id: String,
    /// Face selector the load acts on. Required for every kind except
    /// `gravity`, which acts on the whole body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faces: Option<String>,
    #[serde(flatten)]
    pub kind: LoadKind,
}

/// Load type and parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LoadKind {
    /// Total force vector in newtons, spread uniformly over the area of the
    /// selected faces.
    Force {
        /// Total force [Fx, Fy, Fz] in N.
        newton: [f64; 3],
    },
    /// Uniform pressure normal to the selected faces.
    Pressure {
        /// Pressure in MPa (N/mm^2). Positive pushes on the body (acts along
        /// the inward face normal); negative pulls.
        mpa: f64,
    },
    /// Torque about an axis, applied to the selected faces as tangential
    /// forces that grow linearly with the distance from the axis. It is
    /// applied as a pure couple (zero net force), so `axis_point` documents
    /// where the axis is but does not change the resulting load.
    Torque {
        /// A point on the torque axis, mm.
        axis_point: [f64; 3],
        /// Axis direction (need not be normalised); right-hand rule.
        axis_dir: [f64; 3],
        /// Torque magnitude in N*mm.
        newton_mm: f64,
    },
    /// Gravity (or any uniform acceleration) acting on the whole body. Uses
    /// the body's material density. `faces` must be omitted.
    Gravity {
        /// Acceleration magnitude in m/s^2 (9.81 for standard gravity).
        g: f64,
        /// Direction the acceleration points to, e.g. [0,0,-1].
        dir: [f64; 3],
    },
    /// Bearing load on a cylindrical face: a total force distributed as a
    /// cosine-shaped pressure over the half of the cylinder that the load
    /// pushes against.
    Bearing {
        /// Total force [Fx, Fy, Fz] in N; normally perpendicular to the
        /// cylinder axis.
        newton: [f64; 3],
    },
    /// A force acting at a remote point, transferred to the selected faces
    /// as the statically equivalent force plus moment.
    Remote {
        /// Point where the force acts, mm.
        point: [f64; 3],
        /// Force [Fx, Fy, Fz] in N.
        newton: [f64; 3],
    },
}

/// Mesh type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MeshKind {
    /// Voxel hexahedral mesh (engineering estimate, about +/-10 %).
    #[default]
    Hex,
    /// Conforming quadratic tetrahedral mesh (Tet10): follows curved faces and gives better
    /// stress near holes and fillets. Falls back to hex with warning `SIM_MESH_FALLBACK_HEX`
    /// when the body cannot be meshed.
    Tet,
}

/// Mesh controls. With neither `cell_mm` nor `target_elems` the cell size
/// starts at 1/40 of the largest bounding-box dimension and is adapted so the
/// mesh has between 20,000 and 200,000 elements.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct MeshSettings {
    /// Mesh type; only "hex" is supported for now.
    pub kind: MeshKind,
    /// Explicit cell edge length in mm. Cells are stretched slightly per axis
    /// so the grid fits the bounding box exactly. Takes precedence over
    /// `target_elems`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell_mm: Option<f64>,
    /// Approximate number of elements to aim for when `cell_mm` is omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_elems: Option<usize>,
}

/// Fixture dengan selector yang sudah menjadi indeks face.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedFixture {
    pub id: String,
    pub faces: Vec<u32>,
    pub kind: FixtureKind,
}

/// Beban dengan selector yang sudah menjadi indeks face (kosong untuk gravitasi).
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedLoad {
    pub id: String,
    pub faces: Vec<u32>,
    pub kind: LoadKind,
}

/// Setup siap jalan: engine mengisi indeks face dan volume eksak B-rep.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ResolvedSetup {
    pub fixtures: Vec<ResolvedFixture>,
    pub loads: Vec<ResolvedLoad>,
    pub mesh: MeshSettings,
    /// Volume eksak body (mm³). Bila `None`, volume mesh permukaan dipakai.
    pub exact_volume_mm3: Option<f64>,
}

/// Steady-state thermal study setup: boundary conditions on faces. Faces
/// without a boundary condition are insulated. At least one `temperature`
/// or `convection` boundary is required. Units: degrees Celsius, W, mm.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ThermalSetup {
    /// Thermal boundary conditions.
    pub boundary: Vec<ThermalBc>,
}

/// A thermal boundary condition on one or more faces, for example
/// `{"id":"hot","faces":"<X","kind":"temperature","celsius":120}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ThermalBc {
    /// Unique id of this boundary condition.
    pub id: String,
    /// Face selector (same grammar as modeling ops).
    pub faces: String,
    #[serde(flatten)]
    pub kind: ThermalBcKind,
}

/// Thermal boundary condition type and parameters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ThermalBcKind {
    /// Fixed temperature on the faces.
    Temperature {
        /// Temperature in degrees Celsius.
        celsius: f64,
    },
    /// Prescribed heat flux through the faces.
    HeatFlux {
        /// Heat flux in W/mm^2. Positive heats the body (flows into it).
        w_per_mm2: f64,
    },
    /// Convection to a fluid at ambient temperature.
    Convection {
        /// Film coefficient in W/(mm^2*K). 1 W/(m^2*K) = 1e-6 W/(mm^2*K).
        h_w_mm2k: f64,
        /// Ambient fluid temperature in degrees Celsius.
        ambient_c: f64,
    },
}

/// Syarat batas termal dengan selector yang sudah menjadi indeks face.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedThermalBc {
    pub id: String,
    pub faces: Vec<u32>,
    pub kind: ThermalBcKind,
}

/// Setup termal siap jalan.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ResolvedThermalSetup {
    pub boundary: Vec<ResolvedThermalBc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SimSetup {
        SimSetup {
            body: "base".into(),
            fixtures: vec![Fixture {
                id: "fix".into(),
                faces: "<X".into(),
                kind: FixtureKind::Fixed,
            }],
            loads: vec![
                Load {
                    id: "f".into(),
                    faces: Some(">X".into()),
                    kind: LoadKind::Force {
                        newton: [0.0, 0.0, -100.0],
                    },
                },
                Load {
                    id: "g".into(),
                    faces: None,
                    kind: LoadKind::Gravity {
                        g: 9.81,
                        dir: [0.0, 0.0, -1.0],
                    },
                },
                Load {
                    id: "t".into(),
                    faces: Some("all[kind=cylinder]".into()),
                    kind: LoadKind::Torque {
                        axis_point: [0.0; 3],
                        axis_dir: [0.0, 0.0, 1.0],
                        newton_mm: 500.0,
                    },
                },
            ],
            mesh: MeshSettings {
                kind: MeshKind::Hex,
                cell_mm: Some(2.0),
                target_elems: None,
            },
        }
    }

    #[test]
    fn sim_setup_serde_roundtrip() {
        let setup = sample();
        let json = serde_json::to_string(&setup).unwrap();
        let back: SimSetup = serde_json::from_str(&json).unwrap();
        assert_eq!(setup, back);
        // Bentuk JSON yang ditulis agent.
        let text = r#"{
            "body": "b",
            "fixtures": [{"id": "a", "faces": "<Z", "kind": "symmetry"}],
            "loads": [
                {"id": "p", "faces": ">Z", "kind": "pressure", "mpa": 0.5},
                {"id": "r", "faces": ">X", "kind": "remote", "point": [1,2,3], "newton": [0,0,-1]},
                {"id": "b", "faces": "all[kind=cylinder]", "kind": "bearing", "newton": [10,0,0]},
                {"id": "g", "kind": "gravity", "g": 9.81, "dir": [0,0,-1]}
            ]
        }"#;
        let parsed: SimSetup = serde_json::from_str(text).unwrap();
        assert_eq!(parsed.mesh, MeshSettings::default());
        assert_eq!(parsed.fixtures[0].kind, FixtureKind::Symmetry);
        assert_eq!(parsed.loads[0].kind, LoadKind::Pressure { mpa: 0.5 });
        assert!(parsed.loads[3].faces.is_none());
        let value = serde_json::to_value(&parsed).unwrap();
        assert_eq!(value["loads"][0]["kind"], "pressure");
        assert_eq!(value["loads"][0]["mpa"], 0.5);
    }

    #[test]
    fn sim_setup_rejects_unknown_fields() {
        for bad in [
            r#"{"body":"b","fixtures":[],"loads":[],"extra":1}"#,
            r#"{"body":"b","fixtures":[{"id":"a","faces":"<Z","kind":"fixed","x":1}],"loads":[]}"#,
            r#"{"body":"b","fixtures":[],"loads":[{"id":"p","faces":">Z","kind":"pressure","mpa":1,"oops":2}]}"#,
            r#"{"body":"b","fixtures":[],"loads":[{"id":"p","faces":">Z","kind":"warp","mpa":1}]}"#,
            r#"{"body":"b","fixtures":[],"loads":[],"mesh":{"kind":"hex","cells":3}}"#,
        ] {
            assert!(serde_json::from_str::<SimSetup>(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn sim_setup_json_schema() {
        let schema = schemars::schema_for!(SimSetup);
        let text = serde_json::to_string(&schema).unwrap();
        for needle in [
            "fixtures",
            "loads",
            "newton_mm",
            "axis_dir",
            "pressure",
            "gravity",
            "bearing",
            "remote",
            "symmetry",
            "roller",
            "cell_mm",
            "target_elems",
        ] {
            assert!(text.contains(needle), "skema tidak memuat {needle}");
        }
        assert!(text.is_ascii(), "teks skema harus ASCII");
    }
}
