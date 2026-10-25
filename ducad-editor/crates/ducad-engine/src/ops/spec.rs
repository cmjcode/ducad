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
        /// `{"cone":{"r1":5,"r2":0,"h":10}}`, `{"torus":{"major":10,"minor":2}}` (around +Z).
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
        /// `{"iso":"M5"}` (clearance, through), `{"iso":"M6","kind":"counterbore"}`,
        /// `{"diameter":4.2,"depth":10}`, or a custom stepped hole such as
        /// `{"diameter":6.6,"kind":"counterbore","cbore_diameter":12,"cbore_depth":6.5}`.
        spec: HoleSpecRef,
    },
    /// Pattern copies of a body. `merge: false` (default): new copies `id.1`, `id.2`, …; `merge:
    /// true`: all copies are unioned into `body`.
    Pattern {
        id: String,
        /// Name of the source body.
        body: String,
        /// `{"linear":{"count":[nx,ny,nz],"pitch":[dx,dy,dz]}}` or
        /// `{"circular":{"pivot":[0,0,0],"axis":[0,0,1],"count":6,"angle_deg":360}}` (or
        /// `"axis":"axis:<id>"` for a datum axis, without `pivot`), or
        /// `{"path":{"path":"<sketch id>"|{"points":[…]},"count":5,"align":true}}`. `count`
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
        /// Keep `body` unchanged and create the moved copy as a new body named `id`.
        #[serde(default, skip_serializing_if = "is_false")]
        copy: bool,
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
        /// Keep `body` unchanged and create the scaled copy as a new body named `id`.
        #[serde(default, skip_serializing_if = "is_false")]
        copy: bool,
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
    /// Sheet metal: create the base plate of a folded part from a sketch with one closed region
    /// (lines, arcs and curves; arcs/curves are faceted). Holes inside the region become sheet
    /// cutouts that unfold with the part. The body remembers thickness, bend radius and k-factor for later
    /// `edge_flange` / `hem` / `jog` / `unfold` / `flat_pattern` ops.
    BaseFlange {
        id: String,
        /// Id of the sketch holding exactly one closed region (holes allowed).
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
    /// Sheet metal: bend a flange up from one or more free straight edges of the base flange, or
    /// from the free tip edge of an existing flange (the chain continues). The flange spans the
    /// full edge.
    EdgeFlange {
        id: String,
        /// Name of a body created by `base_flange`.
        body: String,
        /// Edge selector resolving to outer straight edges of the base plate (top or bottom rim) or
        /// to the tip edge of an existing flange.
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
        /// Bend relief notch cut at both ends of the bend: `none` (default), `rect`, `obround`.
        /// Width = min(t, edge/3), depth = R + t into the base plate; also cut in the flat
        /// pattern.
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
    /// Sheet metal: punch a grid of round holes into the base plate (perforated sheet). Holes keep
    /// `margin` from the plate edges, existing cutouts and relief notches; at most 2000 holes.
    Perforate {
        id: String,
        /// Name of a sheet metal body.
        body: String,
        /// Hole diameter in mm.
        d: Num,
        /// Center-to-center distance in mm (> d).
        pitch: Num,
        /// Minimum clear distance from hole edge to plate edge / other holes; default
        /// max(d, 2t + R).
        #[serde(default)]
        margin: Option<Num>,
        /// `true` (default) = staggered 60° rows, `false` = straight grid.
        #[serde(default = "default_true")]
        stagger: bool,
    },
    /// Sheet metal: turn an existing flat plate solid of uniform thickness (e.g. an imported STEP
    /// blank) into a sheet metal body, so flanges / perforation / flat pattern work on it. Round
    /// through holes become cutouts. A solid that is already bent is rejected; the error context
    /// lists the bend radii found.
    ConvertToSheet {
        id: String,
        /// Name of the solid body; it keeps its name and becomes a sheet metal body.
        body: String,
        /// Default inner bend radius for later flanges; default = detected thickness.
        #[serde(default)]
        bend_radius: Option<Num>,
        /// Neutral-axis k-factor (0..1); default 0.44.
        #[serde(default)]
        k_factor: Option<f64>,
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
    /// Add body geometry to an EXISTING sketch (creates no body). `mode` `project` (default):
    /// project the edges picked by `edges` and/or the boundary edges of the faces picked by
    /// `faces` onto the sketch plane. `intersect`: the intersection curve of `body` with the
    /// sketch plane (`edges`/`faces` must be omitted). New entities are named `<id>.1`,
    /// `<id>.2`, … and can be used by `extrude.profile` like any other entity.
    SketchProject {
        id: String,
        /// Id of the target sketch op; entities are appended to it.
        sketch: String,
        /// Source body name.
        body: String,
        /// Edge selector to project, e.g. `"of(>Z)"` (`project` mode).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        edges: Option<String>,
        /// Face selector whose boundary edges are projected, e.g. `">Z"` (`project` mode).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        faces: Option<String>,
        /// `project` (default) or `intersect`.
        #[serde(default)]
        mode: ProjectMode,
        /// Add the entities as construction geometry (reference only, not part of profiles).
        #[serde(default)]
        construction: bool,
    },
    /// Datum axis named `id` (creates no body). Reference it as `"axis:<id>"` in `revolve.axis`
    /// (the axis must lie in the sketch plane), in a circular `pattern` axis, in plane
    /// `perpendicular_to` / `through_edges`, and in point `two_edges`.
    ConstructAxis {
        id: String,
        /// `{"two_points":{"a":[0,0,0],"b":[0,0,10]}}`, `{"edge":{"body":"b","edge":"|Z and >X and >Y"}}`,
        /// `{"normal_to_face":{"body":"b","face":">Z"}}`,
        /// `{"through_cylinder":{"body":"b","face":"all[kind=cylinder]"}}`,
        /// `{"two_planes":{"a":"XZ","b":{"base":"YZ","offset":10}}}`.
        by: AxisDef,
    },
    /// Datum point named `id` (creates no body). Reference it as `"point:<id>"` wherever a
    /// datum point is accepted (`two_points`, `through`).
    ConstructPoint {
        id: String,
        /// `{"vertex":{"body":"b","near":[30,20,10]}}`, `{"center":{"body":"b","edge":"..."}}`,
        /// `{"edge_plane":{"body":"b","edge":"|Z","plane":{"base":"XY","offset":5}}}`,
        /// `{"two_edges":{"a":{"body":"b","edge":"..."},"b":"axis:ax1"}}`,
        /// `{"three_planes":{"a":"XY","b":"XZ","c":"YZ"}}`,
        /// `{"along_path":{"body":"b","edge":"...","fraction":0.25}}`.
        by: PointDef,
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
    // --- Fase 2: solid & direct editing (PLAN_PARITAS_SOLIDCRAFT §2). ---
    /// Move faces along their normal; neighbours follow (direct edit, in place).
    OffsetFace {
        id: String,
        /// Name of the body to modify.
        body: String,
        /// Face selector, e.g. `">Z"` or `"all[kind=cylinder][r=5]"`.
        faces: String,
        /// Offset distance (mm, non-zero; negative = inward).
        distance: Num,
    },
    /// Delete faces (fillet, boss, hole) and heal the gap (in place).
    DeleteFace {
        id: String,
        /// Name of the body to modify.
        body: String,
        /// Face selector of the faces to remove, e.g. `"all[kind=cylinder][r=2]"`.
        faces: String,
    },
    /// Replace one planar face with a plane or another planar face (in place).
    ReplaceFace {
        id: String,
        /// Name of the body to modify.
        body: String,
        /// Selector of exactly 1 planar face.
        face: String,
        /// Target plane (same forms as `sketch.plane`); must not be perpendicular to the face.
        with: PlaneSpec,
    },
    /// Circle swept along a path; `wall` makes a tube.
    Pipe {
        id: String,
        /// Id of a sketch holding an open chain, or `{"points":[[x,y,z],…]}`.
        path: SweepPath,
        /// Outer radius (> 0).
        r: Num,
        /// Wall thickness (0 < wall < r) for a hollow tube; omitted = solid rod.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        wall: Option<Num>,
        /// `new` (default), `add`, `cut` (see `extrude.mode`).
        #[serde(default)]
        mode: BodyMode,
        /// Body name for `add`/`cut` mode.
        #[serde(default)]
        target: Option<String>,
    },
    /// Thin walls along the lines of a sketch, unioned into `body`.
    Web {
        id: String,
        /// Name of the body that receives the walls.
        body: String,
        /// Id of a sketch with line entities (each line = one wall).
        sketch: String,
        /// Wall thickness (> 0), centered on the line.
        thickness: Num,
        /// Wall height from the sketch plane (> 0).
        depth: Num,
        /// Grow against the sketch normal.
        #[serde(default)]
        reverse: bool,
    },
    /// Raise (or engrave with `deboss`) closed sketch regions on a body.
    Emboss {
        id: String,
        /// Id of the sketch with the closed regions (text outlines, logos, any shape).
        sketch: String,
        /// Regions to use (see `extrude.profile`); default all.
        #[serde(default)]
        profile: ProfileSel,
        /// Name of the body to modify.
        body: String,
        /// Height / engraving depth (> 0).
        depth: Num,
        /// Engrave instead of raise.
        #[serde(default)]
        deboss: bool,
    },
    /// Move a body so its face meets a face of another body (coplanar/coaxial).
    Align {
        id: String,
        /// Name of the body to move.
        body: String,
        /// Selector of 1 planar or cylindrical face on `body`.
        from: String,
        /// Target face `{"body":"b2","face":">Z"}` (same kind as `from`).
        to: FaceRef,
        /// Planar faces: keep both normals pointing the same way.
        #[serde(default)]
        flip: bool,
        /// Gap along the target face normal (planar) or along the axis (cylindrical).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        offset: Option<Num>,
    },
    /// New body filling the cell of `bodies` that holds each point (overlap or cavity).
    BoundaryFill {
        id: String,
        /// Bounding bodies (≥ 1).
        bodies: Vec<String>,
        /// One point per cell to fill (≥ 1).
        points: Vec<[Num; 3]>,
    },
    /// New box/cylinder enclosing a body, grown by `margin`.
    BoundingSolid {
        id: String,
        /// Name of the body to enclose.
        body: String,
        /// `box` (default) or `cylinder`.
        #[serde(default)]
        shape: BoundingShape,
        /// Extra clearance on every side (≥ 0, default 0).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        margin: Option<Num>,
        /// Cylinder axis direction (default +Z).
        #[serde(default = "z_axis")]
        axis: [Num; 3],
    },
    /// Split faces along a plane or sketch curves; solid unchanged.
    SplitFace {
        id: String,
        /// Name of the body to modify.
        body: String,
        /// Face selector of the faces to split; default every face the tool crosses (plane) /
        /// every face (sketch).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        faces: Option<String>,
        /// `{"plane": <plane spec>}` or `{"sketch": "<sketch id>"}`.
        tool: SplitFaceTool,
    },
    /// Surface (sheet) modeling: open-profile extrude/revolve/sweep/loft, patch, boundary,
    /// ruled, offset, extend, trim/untrim, stitch/unstitch, thicken, blend, match, fair, reverse.
    Surface {
        id: String,
        /// One surface operation, e.g. `{"extrude":{"sketch":"sk","end":10}}`.
        make: SurfaceSpec,
    },
    /// Injection-molded plastic features on `body` (boss, lip/groove, snap fit, rest); omitted
    /// sizes default to the active `dfm-injection` rules (draft, wall, radius).
    Plastic {
        id: String,
        /// Name of the body to modify.
        body: String,
        /// One feature, e.g. `{"boss":{"face":">Z","at":[[0,0]],"height":10,"hole_d":3}}`.
        make: PlasticSpec,
    },
    // --- Alat langsung GUI sebagai op oplog (ADR 0010). ---
    /// Push/pull one face along its outward normal (body modified in place): `distance > 0` adds
    /// a prism on a planar face (or grows a curved face), `< 0` cuts into the body.
    PushPull {
        id: String,
        /// Name of the body to modify.
        body: String,
        /// Selector of exactly 1 face, e.g. `">Z"`.
        face: String,
        /// Signed distance (mm, non-zero).
        distance: Num,
    },
    /// Revolve one planar face around an axis lying in its plane; the swept solid is unioned into
    /// `body` (in place).
    RevolveFace {
        id: String,
        /// Name of the body to modify.
        body: String,
        /// Selector of exactly 1 planar face.
        face: String,
        /// `{"origin":[x,y,z],"dir":[dx,dy,dz]}`; the direction must lie in the face plane.
        axis: AxisLine,
        /// Sweep angle; omitted = 360°.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        angle_deg: Option<Num>,
    },
    /// Fillet (or chamfer) every edge meeting at the body corner nearest `near` (in place). The
    /// corner must lie within 5 mm of `near`.
    RoundVertex {
        id: String,
        /// Name of the body to modify.
        body: String,
        /// World point at (or near) the corner [x,y,z].
        near: [Num; 3],
        /// Fillet radius / chamfer distance (> 0).
        radius: Num,
        /// Chamfer instead of fillet.
        #[serde(default)]
        chamfer: bool,
    },
    /// Stiffening rib from `start` to `end`, `thickness` wide (centered on the line) and `depth`
    /// tall along `normal`, unioned into `body` (in place).
    Rib {
        id: String,
        /// Name of the body that receives the rib.
        body: String,
        /// Start point of the rib line [x,y,z].
        start: [Num; 3],
        /// End point of the rib line [x,y,z].
        end: [Num; 3],
        /// Direction the rib grows in (its height).
        normal: [Num; 3],
        /// Rib thickness (> 0).
        thickness: Num,
        /// Rib height along `normal` (> 0).
        depth: Num,
        /// Draft of both long sides in degrees (narrower at the top); omitted = none.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        draft_deg: Option<Num>,
    },
    /// Resize the body along an edge from `start` to `end` to the new `length` (in place): the
    /// planar end face perpendicular to the edge moves; otherwise the body scales uniformly.
    ResizeEdge {
        id: String,
        /// Name of the body to modify.
        body: String,
        /// Edge start point [x,y,z].
        start: [Num; 3],
        /// Edge end point [x,y,z] (this end moves).
        end: [Num; 3],
        /// New edge length (> 0).
        length: Num,
    },
    /// Shell with per-face wall thickness (in place). Convex bodies with planar faces only.
    ShellVariable {
        id: String,
        /// Name of the body to modify.
        body: String,
        /// Default wall thickness (> 0).
        thickness: Num,
        /// Selector of the faces to open (may match several); omitted = closed hollow body.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        remove_faces: Option<String>,
        /// Faces with their own wall thickness, e.g. `[{"faces":"<Z","thickness":4}]`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        custom: Vec<FaceThickness>,
    },
    /// Frozen geometry from an edit that has no parametric op: sets the geometry of `body` to the
    /// stored shape (creates body `body` when it does not exist). Written by the GUI; not editable.
    Adopt {
        id: String,
        /// Name of the body that receives the geometry.
        body: String,
        /// Encoded B-rep (native cache `"<producer>:<base64>"` or STEP text).
        brep: String,
    },
}

/// Axis line in world space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AxisLine {
    /// Point on the axis [x,y,z].
    pub origin: [Num; 3],
    /// Axis direction [dx,dy,dz] (non-zero).
    pub dir: [Num; 3],
}

/// Per-face wall thickness for `shell_variable`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FaceThickness {
    /// Face selector (may match several faces).
    pub faces: String,
    /// Wall thickness of those faces (> 0).
    pub thickness: Num,
}

impl Op {
    pub fn id(&self) -> &str {
        match self {
            Op::Sketch { id, .. }
            | Op::ConstructAxis { id, .. }
            | Op::ConstructPoint { id, .. }
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
            | Op::Perforate { id, .. }
            | Op::ConvertToSheet { id, .. }
            | Op::StandardPart { id, .. }
            | Op::Thread { id, .. }
            | Op::SketchProject { id, .. }
            | Op::OffsetFace { id, .. }
            | Op::DeleteFace { id, .. }
            | Op::ReplaceFace { id, .. }
            | Op::Pipe { id, .. }
            | Op::Web { id, .. }
            | Op::Emboss { id, .. }
            | Op::Align { id, .. }
            | Op::BoundaryFill { id, .. }
            | Op::BoundingSolid { id, .. }
            | Op::SplitFace { id, .. }
            | Op::Surface { id, .. }
            | Op::Plastic { id, .. }
            | Op::PushPull { id, .. }
            | Op::RevolveFace { id, .. }
            | Op::RoundVertex { id, .. }
            | Op::Rib { id, .. }
            | Op::ResizeEdge { id, .. }
            | Op::ShellVariable { id, .. }
            | Op::Adopt { id, .. } => id,
        }
    }

    /// Nama jenis op (sama dengan nilai field `"op"` di JSON).
    pub fn kind(&self) -> &'static str {
        match self {
            Op::Sketch { .. } => "sketch",
            Op::ConstructAxis { .. } => "construct_axis",
            Op::ConstructPoint { .. } => "construct_point",
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
            Op::Perforate { .. } => "perforate",
            Op::ConvertToSheet { .. } => "convert_to_sheet",
            Op::StandardPart { .. } => "standard_part",
            Op::Thread { .. } => "thread",
            Op::SketchProject { .. } => "sketch_project",
            Op::OffsetFace { .. } => "offset_face",
            Op::DeleteFace { .. } => "delete_face",
            Op::ReplaceFace { .. } => "replace_face",
            Op::Pipe { .. } => "pipe",
            Op::Web { .. } => "web",
            Op::Emboss { .. } => "emboss",
            Op::Align { .. } => "align",
            Op::BoundaryFill { .. } => "boundary_fill",
            Op::BoundingSolid { .. } => "bounding_solid",
            Op::SplitFace { .. } => "split_face",
            Op::Surface { .. } => "surface",
            Op::Plastic { .. } => "plastic",
            Op::PushPull { .. } => "push_pull",
            Op::RevolveFace { .. } => "revolve_face",
            Op::RoundVertex { .. } => "round_vertex",
            Op::Rib { .. } => "rib",
            Op::ResizeEdge { .. } => "resize_edge",
            Op::ShellVariable { .. } => "shell_variable",
            Op::Adopt { .. } => "adopt",
        }
    }
}

fn default_true() -> bool {
    true
}

/// `sketch_project` mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjectMode {
    /// Project selected edges / face boundaries onto the sketch plane.
    #[default]
    Project,
    /// Intersection curve of the body with the sketch plane.
    Intersect,
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
/// case-insensitive), an offset named plane, a planar face of a body, or a construction plane:
/// `{"midplane":[A,B]}`, `{"perpendicular_to":LINE,"through":POINT}`,
/// `{"tangent_to":{"body":"b","face":"..."},"angle_deg":90}`,
/// `{"along":{"body":"b","edge":"..."},"fraction":0.5}`, `{"through_edges":[LINE,LINE]}`,
/// `{"three_points":[P,P,P]}`, or an explicit frame
/// `{"frame":{"origin":[x,y,z],"u":[..],"v":[..],"normal":[..]}}`.
/// Every variant has its own key set, so the form is never ambiguous.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged, deny_unknown_fields)]
pub enum PlaneSpec {
    Named(String),
    Offset {
        base: String,
        offset: Num,
    },
    OnFace {
        body: String,
        face: String,
    },
    /// Plane halfway between two parallel planes (normal of the first).
    Midplane {
        /// Exactly two parallel planes, e.g. `[{"body":"b","face":">Z"},{"body":"b","face":"<Z"}]`.
        midplane: Vec<PlaneSpec>,
    },
    /// Plane perpendicular to a straight edge or datum axis, through a point.
    Perpendicular {
        /// `"axis:<id>"` or `{"body":"b","edge":"..."}` (straight edge).
        perpendicular_to: LineRef,
        /// `[x,y,z]` or `"point:<id>"`.
        through: PointRef,
    },
    /// Plane tangent to one cylindrical face. Give `angle_deg` (around the cylinder axis, CCW from
    /// world +X projected onto the cross-section; +Y when the axis is parallel to X) or `through`
    /// (tangent at the surface point nearest to it); neither = angle 0.
    Tangent {
        tangent_to: FaceRef,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        angle_deg: Option<Num>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        through: Option<PointRef>,
    },
    /// Plane normal to a line/circle edge at `fraction` of its length (0 = start, 1 = end).
    AlongPath { along: EdgeRef, fraction: Num },
    /// Plane through two coplanar straight edges/datum axes, or through one of them and `through`.
    TwoEdges {
        /// One or two `"axis:<id>"` / `{"body":"b","edge":"..."}` (straight edges).
        through_edges: Vec<LineRef>,
        /// Required when `through_edges` has one element.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        through: Option<PointRef>,
    },
    /// Plane through three non-collinear points (`[x,y,z]` or `"point:<id>"`); origin = first
    /// point, u toward the second.
    ThreePoints { three_points: Box<[PointRef; 3]> },
    /// Explicit plane frame (e.g. a GUI datum plane at any angle): origin, in-plane axes `u`/`v`
    /// and `normal` (unit vectors are normalised; `normal` may differ in sign from `u × v`).
    Frame { frame: Box<FrameSpec> },
}

/// Explicit plane frame for [`PlaneSpec::Frame`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FrameSpec {
    pub origin: [Num; 3],
    pub u: [Num; 3],
    pub v: [Num; 3],
    pub normal: [Num; 3],
}

/// A point: `[x,y,z]` (numbers or expressions) or `"point:<id>"` (datum point from op
/// `construct_point`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum PointRef {
    Coords([Num; 3]),
    Datum(String),
}

/// A straight line: `"axis:<id>"` (datum axis from op `construct_axis`) or one straight body
/// edge `{"body":"b","edge":"|Z and >X and >Y"}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged, deny_unknown_fields)]
pub enum LineRef {
    Datum(String),
    Edge { body: String, edge: String },
}

/// Exactly one edge of a body: `{"body":"b","edge":"<edge selector>"}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EdgeRef {
    pub body: String,
    pub edge: String,
}

/// Exactly one face of a body: `{"body":"b","face":"<face selector>"}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FaceRef {
    pub body: String,
    pub face: String,
}

/// How op `construct_axis` builds its axis (externally tagged).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum AxisDef {
    /// Through point `a` toward point `b`.
    TwoPoints { a: PointRef, b: PointRef },
    /// Along one straight edge (a circle edge gives the axis of its circle).
    Edge { body: String, edge: String },
    /// Normal of one planar face, through `through` projected onto the face (default: face
    /// centroid).
    NormalToFace {
        body: String,
        face: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        through: Option<PointRef>,
    },
    /// Axis of one cylindrical or conical face.
    ThroughCylinder { body: String, face: String },
    /// Intersection line of two non-parallel planes.
    TwoPlanes { a: PlaneSpec, b: PlaneSpec },
}

/// Which end of the edge for `construct_point.vertex`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VertexEnd {
    #[default]
    Start,
    End,
}

/// How op `construct_point` builds its point (externally tagged).
// `ThreePlanes` memuat tiga `PlaneSpec`; op dibuat jarang (sekali per op
// oplog), jadi selisih ukuran varian tidak berarti — `Box` hanya memperumit
// pola pencocokan pemakai API.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum PointDef {
    /// A body vertex: the `end` (`start` default / `end`) of one edge picked by `edge`, or the
    /// vertex nearest to `near` [x,y,z]. Give exactly one of `edge` / `near`.
    Vertex {
        body: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        edge: Option<String>,
        #[serde(default)]
        end: VertexEnd,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        near: Option<[Num; 3]>,
    },
    /// Center of one circle/arc edge (`edge`) or of one sphere/torus face (`face`); give exactly
    /// one.
    Center {
        body: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        edge: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        face: Option<String>,
    },
    /// Intersection of one edge with a plane (a straight edge is extended to a full line).
    EdgePlane {
        body: String,
        edge: String,
        plane: PlaneSpec,
    },
    /// Intersection (or the midpoint of the closest approach) of two straight lines.
    TwoEdges { a: LineRef, b: LineRef },
    /// Intersection of three planes.
    ThreePlanes {
        a: PlaneSpec,
        b: PlaneSpec,
        c: PlaneSpec,
    },
    /// Point at `fraction` (0..1) of the length of one line/circle edge.
    AlongPath {
        body: String,
        edge: String,
        fraction: Num,
    },
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

/// Revolve axis: `"u"`/`"v"` (sketch plane axis through the origin), `"axis:<id>"` (datum axis
/// from `construct_axis`; it must lie in the sketch plane), or a 2D line in the sketch plane.
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
    /// Torus around +Z centered on `at`: `major` = tube center radius, `minor` = tube radius
    /// (`major > minor > 0`).
    Torus {
        major: Num,
        minor: Num,
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
    /// Custom hole: `diameter` of the main bore; `depth: null` = through. `kind` (default
    /// `clearance`) selects the form: `counterbore` uses `cbore_diameter`/`cbore_depth`,
    /// `countersink` uses `csink_diameter`/`csink_angle_deg` (default 90), `tapped` uses
    /// `thread_pitch`. Omitted form fields fall back to generic defaults.
    Custom {
        diameter: Num,
        #[serde(default)]
        depth: Option<Num>,
        #[serde(default, skip_serializing_if = "is_default_hole_kind")]
        kind: HoleKindSpec,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cbore_diameter: Option<Num>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cbore_depth: Option<Num>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        csink_diameter: Option<Num>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        csink_angle_deg: Option<Num>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thread_pitch: Option<Num>,
    },
}

fn is_default_hole_kind(k: &HoleKindSpec) -> bool {
    *k == HoleKindSpec::Clearance
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
        /// Point on the axis; default origin. Must be omitted (or zero) with a datum axis.
        #[serde(default)]
        pivot: [Num; 3],
        /// Direction `[x,y,z]` through `pivot`, or `"axis:<id>"` (datum axis).
        axis: PatternAxis,
        count: u32,
        angle_deg: Num,
    },
    /// Copies spaced evenly along a path (first copy = original at the path start), e.g.
    /// `{"path":{"path":"rail","count":5,"align":true}}`.
    Path {
        /// Id of a sketch holding an open chain, or `{"points":[[x,y,z],…]}`.
        path: SweepPath,
        /// Total instances including the original (≥ 2).
        count: u32,
        /// Rotate each copy with the path tangent (default: translate only).
        #[serde(default)]
        align: bool,
    },
}

/// Circular pattern axis: a direction `[x,y,z]` or `"axis:<id>"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum PatternAxis {
    Dir([Num; 3]),
    Datum(String),
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
        /// Exact curve (e.g. font glyph outline) as segments chained from `points[0]`; `points`
        /// stays the flattened polyline used for snapping/regions.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        exact: Option<Vec<SegSpec>>,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        construction: bool,
    },
    /// Standalone sketch point (reference for snapping/constraints, no profile edge).
    /// Referenced in constraints as `"<name>"` or `"<name>.center"`.
    Point {
        at: [Num; 2],
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
    /// `driven: true` = reference dimension: measured from geometry, ignored by the solver.
    Distance {
        a: String,
        b: String,
        value: Num,
        #[serde(default, skip_serializing_if = "is_false")]
        driven: bool,
    },
    Radius {
        entity: String,
        value: Num,
        #[serde(default, skip_serializing_if = "is_false")]
        driven: bool,
    },
    /// CCW angle from direction `a` to direction `b`, degrees.
    Angle {
        a: String,
        b: String,
        deg: Num,
        #[serde(default, skip_serializing_if = "is_false")]
        driven: bool,
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
    /// G2 (curvature) continuity between two curve ends (spline–spline or spline–arc), e.g.
    /// `{"a": "s1.end", "b": "s2.start"}`; curve `a` enters the joint, `b` leaves it.
    Curvature {
        a: String,
        b: String,
    },
}

fn is_false(v: &bool) -> bool {
    !*v
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
    /// Optional stored drawing sheets (same fields as the `drawing` tool: name, paper, title,
    /// notes, sections, views, scale, shaded, dimensions). `ducad-cli build` renders the first one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<serde_json::Value>")]
    pub drawings: Vec<ducad_io::drawing::DrawingSpec>,
    /// Optional design brief with numbered requirements (same shape as tool `set_brief`).
    /// Checks reference a requirement through their `requirement` field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brief: Option<crate::brief::DesignBrief>,
    /// Optional rule packs expanded into extra checks when the file is loaded, e.g.
    /// `["dfm-cnc"]` or `[{"pack": "dfm-sheet-metal", "vars": {"thickness": "$t"}}]`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rule_packs: Vec<crate::check::RulePackRef>,
    /// Optional design package: the sheets of the multi-page document set built by
    /// `ducad-cli build --package` / tool `build_package`. Omitted = automatic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<crate::package::PackageSpec>,
}

/// JSON Schema `OpFile`. Salinannya disimpan di `schema/ops.schema.json`.
pub fn op_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(OpFile)).unwrap_or_default()
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

    /// Spec lubang custom bertingkat (counterbore/countersink/tapped) ikut
    /// bentuk `{diameter,..}`; bentuk lama `{iso,kind,depth}` dan
    /// `{diameter,depth}` tetap terbaca dan serialisasi ulang tidak menambah
    /// field.
    #[test]
    fn hole_spec_custom_forms_round_trip_and_stay_backward_compatible() {
        let legacy_iso: HoleSpecRef =
            serde_json::from_value(serde_json::json!({"iso":"M6","kind":"counterbore","depth":12}))
                .unwrap();
        assert!(matches!(legacy_iso, HoleSpecRef::Iso { kind: HoleKindSpec::Counterbore, .. }));

        let legacy_custom: HoleSpecRef =
            serde_json::from_value(serde_json::json!({"diameter":4.2,"depth":10})).unwrap();
        assert!(matches!(
            &legacy_custom,
            HoleSpecRef::Custom { kind: HoleKindSpec::Clearance, cbore_diameter: None, .. }
        ));
        assert_eq!(
            serde_json::to_value(&legacy_custom).unwrap(),
            serde_json::json!({"diameter":4.2,"depth":10.0}),
            "bentuk lama tidak boleh berubah saat diserialisasi ulang"
        );

        let cbore_json = serde_json::json!({
            "diameter":6.6,"depth":12.0,"kind":"counterbore","cbore_diameter":12.5,"cbore_depth":6.5
        });
        let cbore: HoleSpecRef = serde_json::from_value(cbore_json.clone()).unwrap();
        assert!(matches!(&cbore, HoleSpecRef::Custom { kind: HoleKindSpec::Counterbore, .. }));
        let back: HoleSpecRef =
            serde_json::from_value(serde_json::to_value(&cbore).unwrap()).unwrap();
        assert_eq!(cbore, back);
        assert_eq!(serde_json::to_value(&back).unwrap(), cbore_json);

        let csink: HoleSpecRef = serde_json::from_value(serde_json::json!({
            "diameter":5.5,"kind":"countersink","csink_diameter":11.0,"csink_angle_deg":82
        }))
        .unwrap();
        assert!(matches!(&csink, HoleSpecRef::Custom { kind: HoleKindSpec::Countersink, depth: None, .. }));

        let tapped: HoleSpecRef = serde_json::from_value(
            serde_json::json!({"diameter":5.0,"depth":15,"kind":"tapped","thread_pitch":1.0}),
        )
        .unwrap();
        assert!(matches!(&tapped, HoleSpecRef::Custom { kind: HoleKindSpec::Tapped, .. }));

        // Field tak dikenal tetap ditolak.
        assert!(serde_json::from_value::<HoleSpecRef>(
            serde_json::json!({"diameter":5.0,"cbore_dia":9.0})
        )
        .is_err());
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

/// Shape of op `bounding_solid`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BoundingShape {
    #[default]
    Box,
    Cylinder,
}

/// Tool of op `split_face`: `{"plane": <plane spec>}` or `{"sketch": "<sketch id>"}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged, deny_unknown_fields)]
pub enum SplitFaceTool {
    Plane { plane: PlaneSpec },
    Sketch { sketch: String },
}

/// Edges of one body: `{"body":"s1","edges":"all"}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EdgeSel {
    pub body: String,
    pub edges: String,
}

/// Continuity across a blend/match seam.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ContinuitySpec {
    G0,
    #[default]
    G1,
    G2,
}

fn default_tolerance() -> Num {
    Num::Value(0.01)
}

/// Surface operation of op `surface` (externally tagged). Creating forms make body `id`;
/// `extend`, `trim`, `untrim`, `thicken`, `reverse`, `fair`, `match` change `body` in place;
/// `stitch` consumes `bodies`; `unstitch` keeps the rest in `body` and makes `id.1`, `id.2`, ….
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SurfaceSpec {
    /// Extrude every curve of a sketch (open or closed) from `start` to `end` along its normal.
    Extrude {
        sketch: String,
        #[serde(default)]
        start: Num,
        end: Num,
    },
    /// Revolve the sketch curves about a sketch axis (`"u"`/`"v"` or a 2D line).
    Revolve {
        sketch: String,
        axis: AxisSpec,
        #[serde(default = "full_turn")]
        angle_deg: Num,
    },
    /// Sweep the sketch curves along a path (sketch id or points).
    Sweep { sketch: String, path: SweepPath },
    /// Loft through the first curve chain of each sketch (≥ 2).
    Loft { sections: Vec<String> },
    /// Fill the hole bounded by the selected open edges of `body`.
    Patch { body: String, edges: String },
    /// Surface through `u` curves (and `v`: Coons 2×2 / Gordon), each a list of 3D points.
    Boundary {
        u: Vec<Vec<[Num; 3]>>,
        #[serde(default)]
        v: Vec<Vec<[Num; 3]>>,
    },
    /// Strip `distance` wide from edges, along the face normal (default), the face tangent, or
    /// a fixed `direction`.
    Ruled {
        body: String,
        edges: String,
        distance: Num,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        direction: Option<[Num; 3]>,
        #[serde(default)]
        tangent: bool,
        #[serde(default)]
        flip: bool,
    },
    /// New surface offset from faces by `distance`.
    Offset { body: String, faces: String, distance: Num },
    /// Faces of a body copied out as a new surface.
    CopyFaces { body: String, faces: String },
    /// Surface joining edges of two bodies with `continuity` on both sides.
    Blend {
        a: EdgeSel,
        b: EdgeSel,
        #[serde(default)]
        continuity: ContinuitySpec,
    },
    /// Extend boundary edges by `distance` (in place).
    Extend { body: String, edges: String, distance: Num },
    /// Trim with a plane, keeping the side holding `keep` (in place).
    Trim { body: String, plane: PlaneSpec, keep: [Num; 3] },
    /// Remove every hole of one face (in place).
    Untrim { body: String, face: String },
    /// Thicken a surface into a solid (in place).
    Thicken {
        body: String,
        thickness: Num,
        #[serde(default)]
        symmetric: bool,
    },
    /// Flip the normals of faces (in place).
    Reverse { body: String, faces: String },
    /// Smooth faces (in place).
    Fair { body: String, faces: String },
    /// Reshape faces to meet a target edge with `continuity` (in place).
    Match {
        body: String,
        faces: String,
        target: EdgeSel,
        #[serde(default)]
        continuity: ContinuitySpec,
    },
    /// Sew surfaces into one body `id` (closed result = solid). The inputs are consumed.
    Stitch {
        bodies: Vec<String>,
        #[serde(default = "default_tolerance")]
        tolerance: Num,
    },
    /// Detach faces into their own surfaces `id.1`, `id.2`, …; the rest stays in `body`.
    Unstitch { body: String, faces: String },
}

fn full_turn() -> Num {
    Num::Value(360.0)
}

/// Feature of op `plastic` (externally tagged). Points use the `hole` convention: `at` local
/// (u, v) on the face, or `at_world` projected onto it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum PlasticSpec {
    /// Drafted (hollow) screw boss standing on a planar face.
    Boss {
        face: String,
        #[serde(default)]
        at: Vec<[Num; 2]>,
        #[serde(default)]
        at_world: Vec<[Num; 3]>,
        height: Num,
        /// Hole diameter (0 or omitted = solid post).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hole_d: Option<Num>,
        /// Default: hole_d + 4 × rule min_wall.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        outer_d: Option<Num>,
        /// Default: rule min_draft_deg.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        draft_deg: Option<Num>,
        /// Base fillet; default: rule min_internal_radius.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fillet: Option<Num>,
    },
    /// Lip raised along the inner loop of a planar rim face (`groove`: cut instead, grown by
    /// `clearance`).
    Lip {
        face: String,
        height: Num,
        /// Default: rule min_wall.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        width: Option<Num>,
        #[serde(default)]
        groove: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        clearance: Option<Num>,
    },
    /// Cantilever snap fit with a hook at the tip; reports the bending strain.
    SnapFit {
        face: String,
        #[serde(default)]
        at: Vec<[Num; 2]>,
        #[serde(default)]
        at_world: Vec<[Num; 3]>,
        length: Num,
        width: Num,
        hook: Num,
        /// Default: 1.5 × rule min_wall.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thickness: Option<Num>,
        /// Hook ramp angle (default 30°).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        angle_deg: Option<Num>,
        /// Hook direction in the face plane (default face u axis).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        direction: Option<[Num; 3]>,
    },
    /// Flat round pad on a (curved) surface: top `height` above `at` along `direction`.
    Rest {
        at: [Num; 3],
        diameter: Num,
        #[serde(default)]
        height: Num,
        /// Default +Z.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        direction: Option<[Num; 3]>,
    },
}
