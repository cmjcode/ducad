//! Tipe kontrak JSON `Op` (P1.2). Semua tipe menolak field tak dikenal.
//! Nama field, nama varian, dan bentuk JSON di sini adalah KONTRAK bagi
//! agent/CLI/MCP — jangan diganti tanpa memperbarui `00-konvensi.md` dan
//! `schema/ops.schema.json`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::num::{Num, Params};

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Op {
    Sketch {
        id: String,
        plane: PlaneSpec,
        entities: Vec<EntitySpec>,
        #[serde(default)]
        constraints: Vec<ConstraintSpec>,
    },
    Extrude {
        id: String,
        sketch: String,
        #[serde(default)]
        profile: ProfileSel,
        /// Untuk `direction = symmetric`: tebal TOTAL.
        distance: Num,
        #[serde(default)]
        direction: ExtrudeDir,
        #[serde(default)]
        mode: BodyMode,
        #[serde(default)]
        target: Option<String>,
    },
    Revolve {
        id: String,
        sketch: String,
        #[serde(default)]
        profile: ProfileSel,
        axis: AxisSpec,
        /// `null` = 360°.
        #[serde(default)]
        angle_deg: Option<Num>,
        #[serde(default)]
        mode: BodyMode,
        #[serde(default)]
        target: Option<String>,
    },
    Primitive {
        id: String,
        shape: PrimitiveSpec,
        #[serde(default)]
        at: [Num; 3],
    },
    Boolean {
        id: String,
        kind: BoolKind,
        a: String,
        b: String,
    },
    Fillet {
        id: String,
        body: String,
        /// Selector tepi, mis. `"|Z"`.
        edges: String,
        radius: Num,
    },
    Chamfer {
        id: String,
        body: String,
        edges: String,
        distance: Num,
    },
    Shell {
        id: String,
        body: String,
        /// Selector face yang dibuang, mis. `">Z"`.
        remove_faces: String,
        thickness: Num,
    },
    Hole {
        id: String,
        body: String,
        /// Selector yang harus menghasilkan tepat 1 face planar.
        face: String,
        /// Titik lokal (u, v) pada bidang face (`PlaneFrame::on_face`).
        #[serde(default)]
        at: Vec<[Num; 2]>,
        /// Titik dunia, diproyeksikan tegak lurus ke bidang face.
        #[serde(default)]
        at_world: Vec<[Num; 3]>,
        spec: HoleSpecRef,
    },
    Pattern {
        id: String,
        body: String,
        kind: PatternKind,
        #[serde(default)]
        merge: bool,
    },
    Transform {
        id: String,
        body: String,
        #[serde(default)]
        translate: Option<[Num; 3]>,
        #[serde(default)]
        rotate: Option<RotateSpec>,
    },
    Delete {
        id: String,
        body: String,
    },
}

impl Op {
    pub fn id(&self) -> &str {
        match self {
            Op::Sketch { id, .. }
            | Op::Extrude { id, .. }
            | Op::Revolve { id, .. }
            | Op::Primitive { id, .. }
            | Op::Boolean { id, .. }
            | Op::Fillet { id, .. }
            | Op::Chamfer { id, .. }
            | Op::Shell { id, .. }
            | Op::Hole { id, .. }
            | Op::Pattern { id, .. }
            | Op::Transform { id, .. }
            | Op::Delete { id, .. } => id,
        }
    }

    /// Nama jenis op (sama dengan nilai field `"op"` di JSON).
    pub fn kind(&self) -> &'static str {
        match self {
            Op::Sketch { .. } => "sketch",
            Op::Extrude { .. } => "extrude",
            Op::Revolve { .. } => "revolve",
            Op::Primitive { .. } => "primitive",
            Op::Boolean { .. } => "boolean",
            Op::Fillet { .. } => "fillet",
            Op::Chamfer { .. } => "chamfer",
            Op::Shell { .. } => "shell",
            Op::Hole { .. } => "hole",
            Op::Pattern { .. } => "pattern",
            Op::Transform { .. } => "transform",
            Op::Delete { .. } => "delete",
        }
    }
}

/// Bidang sketch: nama (`"XY"`/`"XZ"`/`"YZ"`, alias `"top"`/`"front"`/
/// `"right"`, tidak peka huruf besar), bidang bernama yang digeser, atau
/// face planar sebuah body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged, deny_unknown_fields)]
pub enum PlaneSpec {
    Named(String),
    Offset { base: String, offset: Num },
    OnFace { body: String, face: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExtrudeDir {
    #[default]
    Normal,
    Reverse,
    Symmetric,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BodyMode {
    #[default]
    New,
    Add,
    Cut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BoolKind {
    Union,
    Subtract,
    Intersect,
}

/// Tag string `"all"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub enum AllTag {
    #[default]
    #[serde(rename = "all")]
    All,
}

/// Pemilihan profil: `"all"`, `{"names": [...]}` (nama entitas sketch),
/// atau `{"at": [u, v]}` (region terkecil yang memuat titik).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged, deny_unknown_fields)]
pub enum ProfileSel {
    All(AllTag),
    Names { names: Vec<String> },
    At { at: [Num; 2] },
}

impl Default for ProfileSel {
    fn default() -> Self {
        ProfileSel::All(AllTag::All)
    }
}

/// Sumbu revolve: `"u"`/`"v"` (sumbu bidang sketch lewat origin) atau
/// garis 2D di bidang sketch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged, deny_unknown_fields)]
pub enum AxisSpec {
    Named(String),
    Line { origin: [Num; 2], dir: [Num; 2] },
}

/// Externally tagged: `{"box": {...}}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum PrimitiveSpec {
    Box {
        size: [Num; 3],
        #[serde(default)]
        centered: bool,
    },
    Cylinder {
        r: Num,
        h: Num,
    },
    Sphere {
        r: Num,
    },
    Cone {
        r1: Num,
        r2: Num,
        h: Num,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged, deny_unknown_fields)]
pub enum HoleSpecRef {
    /// `iso`: `"M2"`..`"M12"`, `"M2.5"`. `depth: null` = tembus.
    Iso {
        iso: String,
        #[serde(default)]
        kind: HoleKindSpec,
        #[serde(default)]
        depth: Option<Num>,
    },
    Custom {
        diameter: Num,
        #[serde(default)]
        depth: Option<Num>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HoleKindSpec {
    #[default]
    Clearance,
    Tapped,
    Counterbore,
    Countersink,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum PatternKind {
    Linear {
        count: [u32; 3],
        pitch: [Num; 3],
    },
    Circular {
        pivot: [Num; 3],
        axis: [Num; 3],
        count: u32,
        angle_deg: Num,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RotateSpec {
    pub pivot: [Num; 3],
    pub axis: [Num; 3],
    pub angle_deg: Num,
}

/// Entitas sketch (externally tagged, mis. `{"rect": {...}}`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum EntitySpec {
    Line {
        from: [Num; 2],
        to: [Num; 2],
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        construction: bool,
    },
    /// Tepat satu dari `center`/`corner` (sudut kiri-bawah). Anak:
    /// `name.bottom`, `name.right`, `name.top`, `name.left`.
    Rect {
        #[serde(default)]
        center: Option<[Num; 2]>,
        #[serde(default)]
        corner: Option<[Num; 2]>,
        w: Num,
        h: Num,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        construction: bool,
    },
    Circle {
        center: [Num; 2],
        r: Num,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        construction: bool,
    },
    /// Busur CCW dari `start_deg` ke `end_deg`.
    Arc {
        center: [Num; 2],
        r: Num,
        start_deg: Num,
        end_deg: Num,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        construction: bool,
    },
    /// Busur lewat tiga titik.
    Arc3 {
        p1: [Num; 2],
        p2: [Num; 2],
        p3: [Num; 2],
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        construction: bool,
    },
    Ellipse {
        center: [Num; 2],
        rx: Num,
        ry: Num,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        construction: bool,
    },
    /// Poligon beraturan `sides` (3..=64); anak `name.0 … name.{n-1}`.
    Polygon {
        center: [Num; 2],
        r: Num,
        sides: u32,
        #[serde(default = "yes")]
        inscribed: bool,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        construction: bool,
    },
    /// Slot lonjong antar-pusat `from`–`to` dengan radius `r`; anak `name.0 …`.
    Slot {
        from: [Num; 2],
        to: [Num; 2],
        r: Num,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        construction: bool,
    },
    /// Rangkaian garis; anak `name.0 …`.
    Polyline {
        points: Vec<[Num; 2]>,
        #[serde(default)]
        closed: bool,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        construction: bool,
    },
    Spline {
        points: Vec<[Num; 2]>,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        construction: bool,
    },
}

/// Constraint sketch. Entitas dirujuk dengan nama; titik dengan
/// `"<nama>.start" | ".end" | ".center"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ConstraintSpec {
    Horizontal(String),
    Vertical(String),
    Parallel([String; 2]),
    Perpendicular([String; 2]),
    EqualLength([String; 2]),
    EqualRadius([String; 2]),
    Coincident([String; 2]),
    Fixed {
        point: String,
        at: [Num; 2],
    },
    Distance {
        a: String,
        b: String,
        value: Num,
    },
    Radius {
        entity: String,
        value: Num,
    },
    /// Sudut CCW dari arah `a` ke arah `b`, derajat.
    Angle {
        a: String,
        b: String,
        deg: Num,
    },
    Tangent([String; 2]),
    Concentric([String; 2]),
    Collinear([String; 2]),
    Midpoint {
        point: String,
        line: String,
    },
    PointOnCurve {
        point: String,
        curve: String,
    },
    Symmetric {
        a: String,
        b: String,
        axis: String,
    },
}

/// Isi file ops: `{ "params": {...}, "ops": [...] }`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OpFile {
    #[serde(default)]
    pub params: Params,
    pub ops: Vec<Op>,
}

/// JSON Schema `OpFile`. Salinannya disimpan di `schema/ops.schema.json`.
pub fn op_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(OpFile)).expect("schema selalu bisa diserialisasi")
}

/// Aturan id op: `^[a-z][a-z0-9_]{0,31}$`. Titik dicadangkan engine untuk
/// nama turunan (`plate.2`, `outline.top`).
pub fn is_valid_op_id(id: &str) -> bool {
    let b = id.as_bytes();
    !b.is_empty()
        && b.len() <= 32
        && b[0].is_ascii_lowercase()
        && b[1..]
            .iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLATE: &str = include_str!("../../tests/fixtures/plate.ops.json");

    #[test]
    fn plate_example_deserializes() {
        let f: OpFile = serde_json::from_str(PLATE).unwrap();
        assert_eq!(f.params["w"], 60.0);
        assert_eq!(f.ops.len(), 4);
        assert!(
            matches!(&f.ops[0], Op::Sketch { plane: PlaneSpec::Named(p), entities, .. }
            if p == "XY" && matches!(&entities[0], EntitySpec::Rect { name: Some(n), .. } if n == "outline"))
        );
        assert!(matches!(
            &f.ops[1],
            Op::Extrude {
                profile: ProfileSel::All(_),
                direction: ExtrudeDir::Normal,
                mode: BodyMode::New,
                ..
            }
        ));
        assert!(
            matches!(&f.ops[3], Op::Hole { at, spec: HoleSpecRef::Iso { iso, kind: HoleKindSpec::Clearance, depth: None }, .. }
            if at.len() == 4 && iso == "M5")
        );
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let bad = r#"{"op":"extrude","id":"p","sketch":"s","distance":5,"distanse":6}"#;
        assert!(serde_json::from_str::<Op>(bad).is_err());
        let bad = r#"{"op":"sketch","id":"s","plane":"XY","entities":[{"circle":{"center":[0,0],"r":1,"radius":2}}]}"#;
        assert!(serde_json::from_str::<Op>(bad).is_err());
        let bad = r#"{"params":{},"ops":[],"extra":1}"#;
        assert!(serde_json::from_str::<OpFile>(bad).is_err());
    }

    #[test]
    fn variants_parse() {
        let ops = r#"[
          {"op":"sketch","id":"s2","plane":{"base":"XY","offset":5},"entities":[
             {"circle":{"center":[0,0],"r":"$r","name":"c1"}},
             {"polygon":{"center":[0,0],"r":10,"sides":6}},
             {"slot":{"from":[0,0],"to":[10,0],"r":2}},
             {"polyline":{"points":[[0,0],[1,0],[1,1]],"closed":true}},
             {"arc":{"center":[0,0],"r":5,"start_deg":0,"end_deg":90}}],
           "constraints":[{"horizontal":"l1"},{"parallel":["l1","l2"]},
             {"fixed":{"point":"l1.start","at":[0,0]}},
             {"distance":{"a":"l1.start","b":"l1.end","value":50}},
             {"angle":{"a":"l1","b":"l2","deg":90}}]},
          {"op":"sketch","id":"s3","plane":{"body":"plate","face":">Z"},"entities":[]},
          {"op":"extrude","id":"e","sketch":"s","profile":{"at":[1,2]},"distance":3,"direction":"symmetric","mode":"cut","target":"plate"},
          {"op":"extrude","id":"e2","sketch":"s","profile":{"names":["c1"]},"distance":3},
          {"op":"revolve","id":"r","sketch":"s","axis":"v","angle_deg":180},
          {"op":"revolve","id":"r2","sketch":"s","axis":{"origin":[0,0],"dir":[0,1]}},
          {"op":"primitive","id":"b","shape":{"box":{"size":[1,2,3]}},"at":[0,0,5]},
          {"op":"primitive","id":"c","shape":{"cone":{"r1":2,"r2":0,"h":3}}},
          {"op":"boolean","id":"u","kind":"union","a":"b","b":"c"},
          {"op":"chamfer","id":"ch","body":"u","edges":"of(>Z)","distance":1},
          {"op":"shell","id":"sh","body":"u","remove_faces":">Z","thickness":1},
          {"op":"hole","id":"h","body":"u","face":">Z","at_world":[[0,0,5]],"spec":{"diameter":3,"depth":2}},
          {"op":"pattern","id":"p","body":"u","kind":{"circular":{"pivot":[0,0,0],"axis":[0,0,1],"count":6,"angle_deg":360}},"merge":true},
          {"op":"transform","id":"t","body":"u","translate":[1,0,0],"rotate":{"pivot":[0,0,0],"axis":[0,0,1],"angle_deg":45}},
          {"op":"delete","id":"d","body":"u"}
        ]"#;
        let parsed: Vec<Op> = serde_json::from_str(ops).unwrap();
        assert_eq!(parsed.len(), 15);
        assert!(matches!(
            &parsed[0],
            Op::Sketch {
                plane: PlaneSpec::Offset { .. },
                ..
            }
        ));
        assert!(matches!(
            &parsed[1],
            Op::Sketch {
                plane: PlaneSpec::OnFace { .. },
                ..
            }
        ));
        assert!(matches!(
            &parsed[2],
            Op::Extrude {
                profile: ProfileSel::At { .. },
                ..
            }
        ));
        assert!(matches!(
            &parsed[3],
            Op::Extrude {
                profile: ProfileSel::Names { .. },
                ..
            }
        ));
        assert!(matches!(
            &parsed[11],
            Op::Hole {
                spec: HoleSpecRef::Custom { .. },
                ..
            }
        ));
    }

    #[test]
    fn round_trip_is_identical() {
        let f: OpFile = serde_json::from_str(PLATE).unwrap();
        let json = serde_json::to_string(&f).unwrap();
        let back: OpFile = serde_json::from_str(&json).unwrap();
        assert_eq!(f, back);
    }

    #[test]
    fn op_id_rule() {
        assert!(is_valid_op_id("plate"));
        assert!(is_valid_op_id("h1_top"));
        for bad in ["", "Plate", "1a", "a.b", "a-b", &"a".repeat(33)] {
            assert!(!is_valid_op_id(bad), "{bad}");
        }
    }

    #[test]
    fn schema_file_is_up_to_date() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/schema/ops.schema.json");
        let fresh = serde_json::to_string_pretty(&op_schema()).unwrap() + "\n";
        if std::env::var_os("DUCAD_UPDATE_SCHEMA").is_some() {
            std::fs::write(path, &fresh).unwrap();
        }
        let stored = std::fs::read_to_string(path).unwrap_or_default();
        assert!(
            stored == fresh,
            "schema/ops.schema.json basi — jalankan: DUCAD_UPDATE_SCHEMA=1 cargo test -p ducad-engine schema_file"
        );
        assert!(fresh.contains("\"extrude\""));
    }
}
