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

fn one() -> Num {
    Num::Value(1.0)
}

fn z_axis() -> [Num; 3] {
    [Num::Value(0.0), Num::Value(0.0), Num::Value(1.0)]
}

fn bottom_face() -> String {
    "<Z".to_string()
}

/// One modeling step. Field `op` selects the kind; `id` is unique (`^[a-z][a-z0-9_]{0,31}$`) and
/// becomes the name of the body/sketch this op CREATES. Ops that MODIFY a body (fillet, chamfer,
/// shell, hole, transform, pattern merge, draft, scale) keep the existing body name. Numbers may be
/// param expressions: `"$w/2 - 3"`. Units: mm and degrees.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Op {
    /// 2D sketch on a plane: named entities + constraints. Creates no body;
    /// extrude/revolve/loft/sweep use it by `id`. Entity coordinates are the plane's local (u, v)
    /// (XY: u=X, v=Y; XZ: u=X, v=Z; YZ: u=Y, v=Z).
    Sketch {
        id: String,
        /// `"XY"`/`"XZ"`/`"YZ"`, `{"base":"XY","offset":10}`, or `{"body":"plate","face":">Z"}`
        /// (planar face; origin = face centroid).
        plane: PlaneSpec,
        /// Externally tagged entities, e.g.
        /// `{"rect":{"center":[0,0],"w":60,"h":40,"name":"outline"}}`.
        entities: Vec<EntitySpec>,
        /// Constraints between named entities (optional; the initial geometry already defines the
        /// shape).
        #[serde(default)]
        constraints: Vec<ConstraintSpec>,
    },
    /// Extrude closed sketch regions along the plane normal. `mode` `new` creates body `id`;
    /// `add`/`cut` unions with / cuts body `target` (its name is kept). Holes inside a region are
    /// carried along.
    Extrude {
        id: String,
        /// Id of the source sketch op.
        sketch: String,
        /// Regions to extrude: `"all"` (default), `{"names":["outline"]}`, or `{"at":[u,v]}`
        /// (smallest region containing the point).
        #[serde(default)]
        profile: ProfileSel,
        /// Distance (> 0). For `direction = symmetric`: TOTAL thickness.
        distance: Num,
        /// `normal` (default, along the plane normal), `reverse`, `symmetric`.
        #[serde(default)]
        direction: ExtrudeDir,
        /// `new` (default), `add`, `cut`. `add`/`cut` require `target`.
        #[serde(default)]
        mode: BodyMode,
        /// Body name for `add`/`cut` mode; must be empty for `new`.
        #[serde(default)]
        target: Option<String>,
        /// One body per selected entity (`id.<name>`), `new` mode only; for vectors/logos.
        #[serde(default)]
        per_object: bool,
        /// `"default"`, `"from_style"` (fill color of path entities), or a preset name.
        #[serde(default)]
        material: MaterialSel,
        /// Extrude only the outline (stroke) with width `width`, not the fill.
        #[serde(default)]
        outline: Option<OutlineSpec>,
    },
    /// Revolve sketch regions around an axis in the sketch plane. The profile must not cross the
    /// axis.
    Revolve {
        id: String,
        /// Id of the source sketch op.
        sketch: String,
        /// Regions to revolve (see `extrude.profile`).
        #[serde(default)]
        profile: ProfileSel,
        /// `"u"`/`"v"` (plane axis through the sketch origin) or `{"origin":[u,v],"dir":[du,dv]}`.
        axis: AxisSpec,
        /// Sweep angle; `null` = 360°.
        #[serde(default)]
        angle_deg: Option<Num>,
        /// `new` (default), `add`, `cut` (see `extrude.mode`).
        #[serde(default)]
        mode: BodyMode,
        /// Body name for `add`/`cut` mode.
        #[serde(default)]
        target: Option<String>,
    },
    /// Primitive body named `id`. Box: minimum corner at `at` (or centered on `at` when
    /// `centered`). Cylinder/cone: base center at `at`, growing toward +Z. Sphere: center at `at`.
    Primitive {
        id: String,
        /// `{"box":{"size":[x,y,z]}}`, `{"cylinder":{"r":5,"h":10}}`, `{"sphere":{"r":5}}`,
        /// `{"cone":{"r1":5,"r2":0,"h":10}}`.
        shape: PrimitiveSpec,
        /// World position [x,y,z]; default origin.
        #[serde(default)]
        at: [Num; 3],
    },
    /// Boolean of two bodies → new body named `id`. Bodies `a` and `b` are CONSUMED (referencing
    /// them again → `body_consumed`); use `id` in later ops.
    Boolean {
        id: String,
        /// `union`, `subtract` (a − b), `intersect`.
        kind: BoolKind,
        /// First body (for `subtract`: the one being cut).
        a: String,
        /// Second body (for `subtract`: the cutting tool).
        b: String,
    },
    /// Round body edges (body modified in place).
    Fillet {
        id: String,
        /// Name of the body to modify.
        body: String,
        /// Edge selector, e.g. `"|Z"` (vertical edges) or `"of(>Z)"` (top face perimeter).
        edges: String,
        /// Radius (> 0); too large → `fillet_radius_too_large` + `fixes`.
        radius: Num,
        /// Variable fillet: radius at the end of each edge (start = `radius`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        radius_end: Option<Num>,
    },
    /// Equal-distance chamfer on body edges (body modified in place).
    Chamfer {
        id: String,
        /// Name of the body to modify.
        body: String,
        /// Edge selector (see `fillet.edges`).
        edges: String,
        /// Chamfer distance from the edge (> 0).
        distance: Num,
    },
    /// Hollow the body into a shell of `thickness`, opening faces `remove_faces` (body modified in
    /// place; walls grow inward). Any face can be opened, so the cavity can face any direction.
    Shell {
        id: String,
        /// Name of the body to modify.
        body: String,
        /// Selector of the faces to remove: `">Z"` opens the top, `"<Z"` the bottom, `">X"`,
        /// `"<X"`, `">Y"`, `"<Y"` a side; several faces open several sides.
        remove_faces: String,
        /// Wall thickness (> 0, < half the smallest body dimension).
        thickness: Num,
        /// Cavity depth in mm, measured from the opened face into the body. Omitted or `0` = full
        /// cavity (down to a floor of `thickness`). `> 0` leaves the material below that depth
        /// solid; it needs exactly 1 planar opened face and must be < body height − `thickness`
        /// (else `shell_depth_too_deep` + `fixes`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        depth: Option<Num>,
    },
    /// Standard/custom hole perpendicular to a planar face, going into the material (body modified
    /// in place). Give `at` and/or `at_world`.
    Hole {
        id: String,
        /// Name of the body to drill.
        body: String,
        /// Selector that must match exactly 1 planar face, e.g. `">Z"`.
        face: String,
        /// Local (u, v) points on the face plane: origin = face centroid, u = world +X projected
        /// onto the face (+Y if the face is perpendicular to X), v = normal × u. Face `>Z`: u = X,
        /// v = Y.
        #[serde(default)]
        at: Vec<[Num; 2]>,
        /// World points, projected perpendicular onto the face plane.
        #[serde(default)]
        at_world: Vec<[Num; 3]>,
        /// `{"iso":"M5"}` (clearance, through), `{"iso":"M6","kind":"counterbore"}`, or
        /// `{"diameter":4.2,"depth":10}`.
        spec: HoleSpecRef,
    },
    /// Pattern copies of a body. `merge: false` (default): new copies `id.1`, `id.2`, …; `merge:
    /// true`: all copies are unioned into `body`.
    Pattern {
        id: String,
        /// Name of the source body.
        body: String,
        /// `{"linear":{"count":[nx,ny,nz],"pitch":[dx,dy,dz]}}` or
        /// `{"circular":{"pivot":[0,0,0],"axis":[0,0,1],"count":6,"angle_deg":360}}`. `count`
        /// includes the original.
        kind: PatternKind,
        /// Union the copies into the source body.
        #[serde(default)]
        merge: bool,
    },
    /// Move and/or rotate a body (modified in place); translate first, then rotate.
    Transform {
        id: String,
        /// Name of the body to move.
        body: String,
        /// Offset [dx,dy,dz].
        #[serde(default)]
        translate: Option<[Num; 3]>,
        /// `{"pivot":[x,y,z],"axis":[0,0,1],"angle_deg":90}`.
        #[serde(default)]
        rotate: Option<RotateSpec>,
    },
    /// Delete a body from the model (e.g. a helper body or cutting tool).
    Delete {
        id: String,
        /// Name of the body to delete.
        body: String,
    },
    /// Loft a solid through ≥ 2 ordered sketches (one closed region per sketch; any plane, e.g.
    /// `{"base":"XY","offset":30}`).
    Loft {
        id: String,
        /// Section sketch ids, ordered from start to end (≥ 2).
        sections: Vec<String>,
        /// `new` (default), `add`, `cut` (see `extrude.mode`).
        #[serde(default)]
        mode: BodyMode,
        /// Body name for `add`/`cut` mode.
        #[serde(default)]
        target: Option<String>,
    },
    /// Sweep a sketch profile along a path (open sketch or 3D points).
    Sweep {
        id: String,
        /// Id of the section sketch (closed region), ideally perpendicular to the start of the
        /// path.
        sketch: String,
        /// Section region (see `extrude.profile`).
        #[serde(default)]
        profile: ProfileSel,
        /// Id of a sketch holding an open chain, or `{"points":[[x,y,z],…]}`.
        path: SweepPath,
        /// `new` (default), `add`, `cut` (see `extrude.mode`).
        #[serde(default)]
        mode: BodyMode,
        /// Body name for `add`/`cut` mode.
        #[serde(default)]
        target: Option<String>,
    },
    /// Spring / thread: a section swept along a helix. External/internal thread = `mode:
    /// "add"`/`"cut"` on a shaft/hole.
    Helix {
        id: String,
        /// Helix radius (to the section center).
        r: Num,
        /// Rise per turn (mm).
        pitch: Num,
        /// Number of turns.
        turns: Num,
        /// `{"circle":{"r":1}}`, `{"rect":{"w":2,"h":1}}`, `{"triangle":{"w":1.5,"h":1.3}}`.
        section: HelixSection,
        /// Base point of the axis.
        #[serde(default)]
        at: [Num; 3],
        /// Helix axis direction (default +Z).
        #[serde(default = "z_axis")]
        axis: [Num; 3],
        /// Radius at the top end (conical helix).
        #[serde(default)]
        end_r: Option<Num>,
        /// Left-hand thread.
        #[serde(default)]
        left_hand: bool,
        /// `new` (default), `add`, `cut`: external thread = `add`, internal thread = `cut`.
        #[serde(default)]
        mode: BodyMode,
        /// Body name for `add`/`cut` mode.
        #[serde(default)]
        target: Option<String>,
    },
    /// Mold draft on planar faces.
    Draft {
        id: String,
        /// Name of the body to modify.
        body: String,
        /// Selector of the faces to draft, e.g. `"#Z"` (side walls).
        faces: String,
        /// Draft angle (degrees, > 0).
        angle_deg: Num,
        /// Selector of 1 planar face = neutral plane (does not move).
        #[serde(default = "bottom_face")]
        neutral: String,
        /// Mold pull direction.
        #[serde(default = "z_axis")]
        pull: [Num; 3],
    },
    /// Mirror a body across a plane. `copy` (default): new body named `id`; `merge`: the original
    /// body is replaced by its union with the mirror image.
    Mirror {
        id: String,
        /// Name of the source body.
        body: String,
        /// `"XY"`/`"XZ"`/`"YZ"` (through the origin) or `{"point":[x,y,z],"normal":[x,y,z]}`.
        plane: MirrorPlane,
        /// Create a new body `id` (default `true`).
        #[serde(default = "yes")]
        copy: bool,
        /// Union the mirror image into the original body (keeps the name `body`).
        #[serde(default)]
        merge: bool,
    },
    /// Uniform scale about `pivot`.
    Scale {
        id: String,
        /// Name of the body to modify.
        body: String,
        /// Scale factor (> 0).
        factor: Num,
        /// Fixed point of the scale; default origin.
        #[serde(default)]
        pivot: [Num; 3],
    },
    /// Cut a body with a plane. `keep: "both"` (default): the original body keeps the side along
    /// `normal`, new body `id` keeps the other side.
    Split {
        id: String,
        /// Name of the body to cut.
        body: String,
        /// Point on the cutting plane; default origin.
        #[serde(default)]
        point: [Num; 3],
        /// Cutting plane normal.
        normal: [Num; 3],
        /// `both` (default), `positive` (only the side along the normal), `negative`.
        #[serde(default)]
        keep: SplitKeep,
    },
    /// Assign a mechanical material (density, Young's modulus, Poisson ratio, yield strength) to a
    /// body. Drives `mass_g`, the inertia tensor, the `mass`/`moment_of_inertia` checks and
    /// simulation studies. Does not change the body's appearance or geometry.
    SetMaterial {
        id: String,
        /// Name of the body to assign the material to.
        body: String,
        /// Library key (`abs`, `pa6`, `pc`, `al_6061_t6`, `al_7075_t6`, `s235`, `s355`, `aisi_304`,
        /// `aisi_1045`, `ti_6al_4v`, `brass`, `copper`, `soda_lime_glass`) or
        /// `{"custom": {"density_g_cm3": 7.85, "young_modulus_gpa": 210, "poisson_ratio": 0.3,
        /// "yield_strength_mpa": 355, "ultimate_strength_mpa": 510}}`.
        material: MechMaterialSpec,
    },
    /// Sheet metal: create the base plate of a folded part from a sketch whose closed region is a
    /// straight-sided polygon. The body remembers thickness, bend radius and k-factor for later
    /// `edge_flange` / `hem` / `jog` / `unfold` / `flat_pattern` ops.
    BaseFlange {
        id: String,
        /// Id of the sketch holding exactly one closed polygon (line segments only).
        sketch: String,
        /// Sheet thickness in mm (> 0); the plate grows along the sketch plane normal.
        thickness: Num,
        /// Default inner bend radius in mm; default = thickness.
        #[serde(default)]
        bend_radius: Option<Num>,
        /// Neutral-axis k-factor (0..1) used for bend allowance BA = angle * (R + k*t); default
        /// 0.44.
        #[serde(default)]
        k_factor: Option<f64>,
    },
    /// Sheet metal: bend a flange up from one or more free straight edges of the base flange. The
    /// flange spans the full edge. Flanges on flanges are not supported yet.
    EdgeFlange {
        id: String,
        /// Name of a body created by `base_flange`.
        body: String,
        /// Edge selector resolving to outer straight edges of the base plate (top or bottom rim).
        edges: String,
        /// Straight length of the flange after the bend, in mm.
        length: Num,
        /// Bend angle in degrees (default 90). Positive bends toward the sketch normal, negative
        /// away from it; magnitude in (0, 180].
        #[serde(default)]
        angle: Option<Num>,
        /// Inner bend radius; default = the body's default bend radius.
        #[serde(default)]
        radius: Option<Num>,
        /// Bend relief shape recorded for manufacturing notes: `none` (default), `rect`,
        /// `obround`. Not yet cut into the geometry.
        #[serde(default)]
        relief: ReliefSpec,
    },
    /// Sheet metal: fold an edge back on itself by 180 degrees (a hem).
    Hem {
        id: String,
        /// Name of a body created by `base_flange`.
        body: String,
        /// Edge selector (same rules as `edge_flange`).
        edges: String,
        /// Length of the returned lip in mm.
        length: Num,
        /// Gap between the lip and the plate in mm (inner bend radius = gap / 2); default =
        /// thickness (an open hem).
        #[serde(default)]
        gap: Option<Num>,
        /// `true` folds toward the sketch normal (default), `false` away from it.
        #[serde(default = "default_true")]
        up: bool,
    },
    /// Sheet metal: two opposite bends that offset the edge strip to a parallel plane (a jog).
    Jog {
        id: String,
        /// Name of a body created by `base_flange`.
        body: String,
        /// Edge selector (same rules as `edge_flange`).
        edges: String,
        /// Offset between the plate and the jogged strip in mm; positive = toward the sketch normal.
        offset: Num,
        /// Length of the flat strip after the jog, in mm.
        length: Num,
        /// Bend angle of both bends in degrees (default 90), in (0, 90].
        #[serde(default)]
        angle: Option<Num>,
        /// Inner bend radius; default = the body's default bend radius.
        #[serde(default)]
        radius: Option<Num>,
    },
    /// Sheet metal: replace the folded body by its flat blank (bend allowance applied). Undo with
    /// `fold`.
    Unfold {
        id: String,
        /// Name of a sheet metal body.
        body: String,
    },
    /// Sheet metal: fold an unfolded body back into its bent shape.
    Fold {
        id: String,
        /// Name of a sheet metal body that was unfolded.
        body: String,
    },
    /// Sheet metal: create a new body `id` holding the flat blank of `body` (the folded body is
    /// kept). The outcome `detail` lists the bend lines; export the blank with
    /// `ducad-cli build --formats flat`.
    FlatPattern {
        id: String,
        /// Name of a sheet metal body.
        body: String,
    },
    /// Insert a standard part from the toolbox as a new body `id`: axis along +Z, seating plane
    /// at `at` (screws: underside of the head, shank toward -Z; nuts, washers, bearings and pins:
    /// bottom face). Threads are cosmetic (plain shank). The BOM lists the standard designation.
    StandardPart {
        id: String,
        /// `iso4762` (socket head cap screw), `iso4014` (hex bolt), `iso4032` (hex nut),
        /// `iso7089` (plain washer), `iso2338` (parallel pin), `bearing` (deep groove ball
        /// bearing).
        standard: String,
        /// Screws, nuts, washers: `M3`, `M4`, `M5`, `M6`, `M8`, `M10`, `M12`. Pins: diameter
        /// `2`..`12`. Bearings: `608`, `6000`, `6001`, `6002`, `6200`, `6201`, `6202`, `6204`,
        /// `6205`.
        size: String,
        /// Nominal length in mm; required for screws and pins, not allowed otherwise.
        #[serde(default)]
        length: Option<Num>,
        /// Position of the seating point [x,y,z]; default origin. Rotate with op `transform`.
        #[serde(default)]
        at: [Num; 3],
    },
    /// Thread on an external cylindrical face (ISO metric 60 degree profile). `cosmetic: true`
    /// (default) only records the thread for notes; `cosmetic: false` cuts the helical groove
    /// into the body (slow: about a second per ten turns).
    Thread {
        id: String,
        /// Name of the body to modify.
        body: String,
        /// Face selector resolving to exactly one cylindrical face (the shank).
        face: String,
        /// Thread pitch in mm; default = ISO coarse pitch for the cylinder diameter (M2-M12).
        #[serde(default)]
        pitch: Option<Num>,
        /// Threaded length in mm measured from the start end; default = whole cylinder.
        #[serde(default)]
        length: Option<Num>,
        /// Start from the other end of the cylinder.
        #[serde(default)]
        from_end: bool,
        /// Left-hand thread.
        #[serde(default)]
        left_handed: bool,
        /// `true` (default): record only. `false`: cut real thread geometry.
        #[serde(default = "default_true")]
        cosmetic: bool,
    },
    /// Store a simulation study (fixtures, loads, mesh settings) in the design. Creates no body
    /// and does not run the solver: run it with tool `simulate_static` (or `ducad-cli sim`), then
    /// assert on the result with checks `max_stress`, `max_displacement`, `min_safety_factor`
    /// (kinds `static`, `thermal_stress`), `min_natural_frequency` (`frequency`),
    /// `min_buckling_factor` (`buckling`) or `max_temperature` (`thermal`).
    /// The body needs a mechanical material (op `set_material`) before the study can run.
    Study {
        id: String,
        /// Study type; default `static`.
        #[serde(default)]
        kind: StudyKind,
        /// Body, fixtures, loads and mesh settings. `frequency` ignores loads; `thermal` uses
        /// only `body` and `mesh` (fixtures and loads may be empty lists).
        setup: ducad_sim::SimSetup,
        /// Thermal boundary conditions; required for kinds `thermal` and `thermal_stress`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thermal: Option<ducad_sim::ThermalSetup>,
        /// Number of modes for kinds `frequency` (default 10) and `buckling` (default 3); 1..=40.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        modes: Option<u32>,
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
            | Op::Delete { id, .. }
            | Op::Loft { id, .. }
            | Op::Sweep { id, .. }
            | Op::Helix { id, .. }
            | Op::Draft { id, .. }
            | Op::Mirror { id, .. }
            | Op::Scale { id, .. }
            | Op::Split { id, .. }
            | Op::SetMaterial { id, .. }
            | Op::Study { id, .. }
            | Op::BaseFlange { id, .. }
            | Op::EdgeFlange { id, .. }
            | Op::Hem { id, .. }
            | Op::Jog { id, .. }
            | Op::Unfold { id, .. }
            | Op::Fold { id, .. }
            | Op::FlatPattern { id, .. }
            | Op::StandardPart { id, .. }
            | Op::Thread { id, .. } => id,
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
            Op::Loft { .. } => "loft",
            Op::Sweep { .. } => "sweep",
            Op::Helix { .. } => "helix",
            Op::Draft { .. } => "draft",
            Op::Mirror { .. } => "mirror",
            Op::Scale { .. } => "scale",
            Op::Split { .. } => "split",
            Op::SetMaterial { .. } => "set_material",
            Op::Study { .. } => "study",
            Op::BaseFlange { .. } => "base_flange",
            Op::EdgeFlange { .. } => "edge_flange",
            Op::Hem { .. } => "hem",
            Op::Jog { .. } => "jog",
            Op::Unfold { .. } => "unfold",
            Op::Fold { .. } => "fold",
            Op::FlatPattern { .. } => "flat_pattern",
            Op::StandardPart { .. } => "standard_part",
            Op::Thread { .. } => "thread",
        }
    }
}

fn default_true() -> bool {
    true
}

/// Bend relief shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReliefSpec {
    #[default]
    None,
    Rect,
    Obround,
}

/// Simulation study type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StudyKind {
    /// Linear static stress analysis.
    #[default]
    Static,
    /// Natural frequencies (Hz) of the body held by the fixtures; loads are ignored.
    Frequency,
    /// Linear buckling: load factors that multiply the applied loads at the onset of buckling.
    Buckling,
    /// Steady-state heat conduction: temperature field from the `thermal` boundary conditions.
    Thermal,
    /// Steady-state conduction followed by a static study loaded by thermal expansion (plus any
    /// mechanical loads); the stress-free reference temperature is 20 degrees Celsius.
    ThermalStress,
}

impl StudyKind {
    /// Nama JSON jenis studi.
    pub fn name(self) -> &'static str {
        match self {
            StudyKind::Static => "static",
            StudyKind::Frequency => "frequency",
            StudyKind::Buckling => "buckling",
            StudyKind::Thermal => "thermal",
            StudyKind::ThermalStress => "thermal_stress",
        }
    }

    /// Jenis yang menghasilkan `SimReport` (tegangan + deformasi).
    pub fn has_stress(self) -> bool {
        matches!(self, StudyKind::Static | StudyKind::ThermalStress)
    }

    /// Jenis yang memakai syarat batas termal.
    pub fn is_thermal(self) -> bool {
        matches!(self, StudyKind::Thermal | StudyKind::ThermalStress)
    }
}

/// Mechanical material: a library key string, or `{"custom": {...}}` with explicit properties.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged, deny_unknown_fields)]
pub enum MechMaterialSpec {
    /// Key of a built-in or per-file library material (case-insensitive).
    Key(String),
    Custom {
        /// Explicit isotropic material properties.
        custom: MechPropsSpec,
    },
}

/// Isotropic material properties at room temperature.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MechPropsSpec {
    /// Density in g/cm^3 (> 0).
    pub density_g_cm3: f64,
    /// Young's modulus in GPa (> 0).
    pub young_modulus_gpa: f64,
    /// Poisson ratio, strictly between 0 and 0.5.
    pub poisson_ratio: f64,
    /// Yield strength in MPa (> 0, at most `ultimate_strength_mpa`).
    pub yield_strength_mpa: f64,
    /// Ultimate tensile strength in MPa (> 0).
    pub ultimate_strength_mpa: f64,
    /// Linear thermal expansion coefficient in 1/K; default 0.
    #[serde(default)]
    pub thermal_expansion_per_k: f64,
    /// Thermal conductivity in W/(m*K); default 0.
    #[serde(default)]
    pub thermal_conductivity_w_mk: f64,
}

impl From<MechPropsSpec> for ducad_core::MechanicalProperties {
    fn from(p: MechPropsSpec) -> Self {
        Self {
            density_g_cm3: p.density_g_cm3,
            young_modulus_gpa: p.young_modulus_gpa,
            poisson_ratio: p.poisson_ratio,
            yield_strength_mpa: p.yield_strength_mpa,
            ultimate_strength_mpa: p.ultimate_strength_mpa,
            thermal_expansion_per_k: p.thermal_expansion_per_k,
            thermal_conductivity_w_mk: p.thermal_conductivity_w_mk,
        }
    }
}

/// Sketch plane: a name (`"XY"`/`"XZ"`/`"YZ"`, aliases `"top"`/`"front"`/`"right"`,
/// case-insensitive), an offset named plane, or a planar face of a body.
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

/// Solid material for extrude: `"default"`, `"from_style"`, or a specific preset (e.g.
/// `"matte_plastic"`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum MaterialSel {
    #[default]
    Default,
    FromStyle,
    Preset(String),
}

impl<'de> Deserialize<'de> for MaterialSel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de;
        struct MaterialSelVisitor;

        impl<'de> de::Visitor<'de> for MaterialSelVisitor {
            type Value = MaterialSel;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string (\"default\", \"from_style\", or preset name) or an object with preset")
            }

            fn visit_str<E>(self, value: &str) -> Result<MaterialSel, E>
            where
                E: de::Error,
            {
                match value.to_ascii_lowercase().as_str() {
                    "default" => Ok(MaterialSel::Default),
                    "from_style" => Ok(MaterialSel::FromStyle),
                    other => Ok(MaterialSel::Preset(other.to_string())),
                }
            }

            fn visit_map<M>(self, mut map: M) -> Result<MaterialSel, M::Error>
            where
                M: de::MapAccess<'de>,
            {
                let mut preset = None;
                while let Some(key) = map.next_key::<String>()? {
                    if key == "preset" {
                        preset = Some(map.next_value::<String>()?);
                    } else {
                        let _: serde_json::Value = map.next_value()?;
                    }
                }
                if let Some(p) = preset {
                    Ok(MaterialSel::Preset(p))
                } else {
                    Ok(MaterialSel::Default)
                }
            }
        }

        deserializer.deserialize_any(MaterialSelVisitor)
    }
}

impl Serialize for MaterialSel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            MaterialSel::Default => serializer.serialize_str("default"),
            MaterialSel::FromStyle => serializer.serialize_str("from_style"),
            MaterialSel::Preset(p) => serializer.serialize_str(p),
        }
    }
}

impl JsonSchema for MaterialSel {
    fn schema_name() -> String {
        "MaterialSel".to_string()
    }

    fn json_schema(gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        String::json_schema(gen)
    }
}

/// Outline spec to extrude only the stroke line (not the fill).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct OutlineSpec {
    #[serde(default)]
    pub width: Option<Num>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BoolKind {
    Union,
    Subtract,
    Intersect,
}

/// String tag `"all"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub enum AllTag {
    #[default]
    #[serde(rename = "all")]
    All,
}

/// Profile selection: `"all"`, `{"names": [...]}` (sketch entity names), or `{"at": [u, v]}`
/// (smallest region containing the point).
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

/// Revolve axis: `"u"`/`"v"` (sketch plane axis through the origin) or a 2D line in the sketch
/// plane.
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
    /// `iso`: `"M2"`..`"M12"`, `"M2.5"`. `depth: null` = through.
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

/// Sketch entity (externally tagged, e.g. `{"rect": {...}}`).
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
    /// Exactly one of `center`/`corner` (bottom-left corner). Children: `name.bottom`,
    /// `name.right`, `name.top`, `name.left`.
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
    /// CCW arc from `start_deg` to `end_deg`.
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
    /// Arc through three points.
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
    /// Regular polygon with `sides` (3..=64); children `name.0 … name.{n-1}`.
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
    /// Obround slot between centers `from`–`to` with radius `r`; children `name.0 …`.
    Slot {
        from: [Num; 2],
        to: [Num; 2],
        r: Num,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        construction: bool,
    },
    /// Chain of lines; children `name.0 …`.
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
    Path {
        subpaths: Vec<SubpathSpec>,
        #[serde(default)]
        style: Option<StyleSpec>,
        #[serde(default)]
        layer: Option<String>,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        construction: bool,
    },
}

/// Closed or open chain of curves for `EntitySpec::Path`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SubpathSpec {
    pub start: [Num; 2],
    pub segs: Vec<SegSpec>,
    #[serde(default)]
    pub closed: bool,
}

/// Segment of a `SubpathSpec` (straight line or cubic Bézier curve).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SegSpec {
    Line {
        to: [Num; 2],
    },
    Cubic {
        c1: [Num; 2],
        c2: [Num; 2],
        to: [Num; 2],
    },
}

/// Visual style for `EntitySpec::Path`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StyleSpec {
    #[serde(default)]
    pub fill: Option<String>, // hex; null = tanpa fill
    #[serde(default)]
    pub stroke: Option<String>,
    #[serde(default)]
    pub stroke_width: Option<Num>, // mm
    #[serde(default = "one")]
    pub opacity: Num,
    #[serde(default)]
    pub fill_rule: Option<String>, // "nonzero" | "evenodd"
}

/// Sketch constraint. Entities are referenced by name; points by `"<name>.start" | ".end" |
/// ".center"`.
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
    /// CCW angle from direction `a` to direction `b`, degrees.
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

/// One design variant: parameter overrides, suppressed ops and material overrides on top of the
/// base design. The base design itself is the reserved configuration `"Default"`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigurationSpec {
    /// Unique name (1-48 chars: letters, digits, space, `_`, `-`, `.`); also the build output
    /// directory name.
    pub name: String,
    /// Parameters overridden by this variant; all others keep their base value.
    #[serde(default)]
    pub params: Params,
    /// Ids of ops skipped in this variant (e.g. an optional hole pattern).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suppressed_ops: Vec<String>,
    /// Body name -> mechanical material used in this variant.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub material_overrides: std::collections::BTreeMap<String, MechMaterialSpec>,
}

/// Ops file content: `{ "params": {...}, "ops": [...], "checks"?: [...] }`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OpFile {
    #[serde(default)]
    pub params: Params,
    pub ops: Vec<Op>,
    /// Optional design checks (P7 format); `ducad-cli run` installs them into `design.checks`
    /// before running the ops.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<crate::check::CheckItem>,
    /// Optional design variants; `ducad-cli build --all-configs` builds every one of them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub configurations: Vec<ConfigurationSpec>,
    /// Variant to activate after loading; default is the base design (`"Default"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_configuration: Option<String>,
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

/// Sweep path: id of a sketch holding an open chain (lines/arcs/splines), or `{"points": [[x,y,z],
/// ...]}` 3D polyline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged, deny_unknown_fields)]
pub enum SweepPath {
    Sketch(String),
    Points { points: Vec<[Num; 3]> },
}

/// Helix section (externally tagged): `{"circle":{"r":1}}`, `{"rect":{"w":2,"h":1}}`,
/// `{"triangle":{"w":1.5,"h":1.3}}` (V thread).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum HelixSection {
    Circle { r: Num },
    Rect { w: Num, h: Num },
    Triangle { w: Num, h: Num },
}

/// Mirror plane: `"XY"`/`"XZ"`/`"YZ"` (through the origin) or `{"point":[x,y,z],"normal":[x,y,z]}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged, deny_unknown_fields)]
pub enum MirrorPlane {
    Named(String),
    Custom {
        #[serde(default)]
        point: [Num; 3],
        normal: [Num; 3],
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SplitKeep {
    #[default]
    Both,
    Positive,
    Negative,
}
