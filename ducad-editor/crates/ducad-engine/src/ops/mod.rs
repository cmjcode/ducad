//! Skema operasi (`Op`) — kontrak JSON untuk agent/CLI/MCP.

pub mod num;
pub(crate) mod sketch;
pub mod spec;

pub use num::{eval, eval_arr, Num, Params};
pub use spec::*;

/// Contoh OpFile lengkap (plat 60×40×8, fillet tepi tegak, 4 lubang M5).
pub const EXAMPLE_PLATE: &str = include_str!("../../tests/fixtures/plate.ops.json");

/// Katalog contoh OpFile untuk agent: `(nama, isi, op yang diperagakan)`.
/// Semuanya di-replay oleh tes `examples_replay`.
pub const EXAMPLES: &[(&str, &str, &str)] = &[
    (
        "plate",
        EXAMPLE_PLATE,
        "sketch rect + extrude + fillet |Z + ISO hole on face >Z (local coordinates)",
    ),
    (
        "bracket",
        include_str!("../../tests/fixtures/bracket.ops.json"),
        "primitive box + boolean union + fillet with [y=][z=] filters + hole at_world",
    ),
    (
        "flange",
        include_str!("../../tests/fixtures/flange.ops.json"),
        "revolve a polyline on XZ around axis v + circular pattern merge + boolean subtract + chamfer + checks",
    ),
    (
        "enclosure",
        include_str!("../../tests/fixtures/enclosure.ops.json"),
        "fillet + shell open top + sketch on face <Y + extrude cut direction reverse + custom hole + checks",
    ),
    (
        "loft_helix",
        include_str!("../../tests/fixtures/loft_helix.ops.json"),
        "offset-plane sketch + loft rect→circle + helix spring + mirror across a custom plane",
    ),
    (
        "mass_bracket",
        include_str!("../../tests/fixtures/mass_bracket.ops.json"),
        "set_material by library key and custom properties + transform + mass / center_of_mass / moment_of_inertia checks",
    ),
    (
        "sim_bracket",
        include_str!("../../tests/fixtures/sim_bracket.ops.json"),
        "set_material + static study (fixed fixture, force load, mesh cell size) + max_stress / max_displacement / min_safety_factor checks",
    ),
    (
        "sim_modes",
        include_str!("../../tests/fixtures/sim_modes.ops.json"),
        "study kinds frequency / buckling / thermal (tet mesh, temperature boundaries) / thermal_stress + min_natural_frequency / min_buckling_factor / max_temperature checks",
    ),
    (
        "sheet_box",
        include_str!("../../tests/fixtures/sheet_box.ops.json"),
        "sheet metal tray: base_flange from a rect sketch + edge_flange on all four sides (edge selectors with [len=][z=]) + flat_pattern + min_bend_radius / min_flange_length checks",
    ),
    (
        "sheet_vent",
        include_str!("../../tests/fixtures/sheet_vent.ops.json"),
        "sheet metal 6C: base_flange with a round hole + edge_flange with rect relief + edge_flange on the tip of that flange + perforate (staggered vent grid) + flat_pattern + convert_to_sheet of an extruded plate",
    ),
    (
        "toolbox_joint",
        include_str!("../../tests/fixtures/toolbox_joint.ops.json"),
        "bolted joint from the toolbox: standard_part washer / socket head cap screw / nut placed with `at` + cosmetic thread on the screw shank + no_interference check",
    ),
    (
        "datum_axis",
        include_str!("../../tests/fixtures/datum_axis.ops.json"),
        "construct_axis (normal_to_face, two_planes, two_points) + construct_point (center of a circle edge, three_planes) + sketch on a midplane + revolve around \"axis:<id>\" + circular pattern around a datum axis",
    ),
    (
        "sketch_project",
        include_str!("../../tests/fixtures/sketch_project.ops.json"),
        "sketch_project: project a body face outline onto an offset sketch (mode project, faces >Z) + intersection of the body with a YZ sketch plane (mode intersect), both extruded via profile at",
    ),
    (
        "sketch_point_g2",
        include_str!("../../tests/fixtures/sketch_point_g2.ops.json"),
        "sketch point entities (referenced by bare name) + driven (reference) distance dimension + curvature (G2) constraint between a spline end and an arc start",
    ),
    (
        "direct_edit",
        include_str!("../../tests/fixtures/direct_edit.ops.json"),
        "delete/offset/replace/split_face, web, emboss, torus, bounding_solid, pipe, pattern path, align, boundary_fill",
    ),
    (
        "surfaces",
        include_str!("../../tests/fixtures/surfaces.ops.json"),
        "surface: revolve open profile, extrude a line, extend + thicken a sheet, boundary through curves, copy_faces",
    ),
    (
        "plastic_case",
        include_str!("../../tests/fixtures/plastic_case.ops.json"),
        "plastic: shelled case + lip on the rim + bosses + cantilever snap fit (strain reported) + rest pad; sizes default to dfm-injection rules",
    ),
];

/// Isi contoh bernama `name` dari [`EXAMPLES`].
pub fn example(name: &str) -> Option<&'static str> {
    EXAMPLES
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, text, _)| *text)
}
