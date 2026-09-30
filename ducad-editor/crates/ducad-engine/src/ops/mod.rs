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
];

/// Isi contoh bernama `name` dari [`EXAMPLES`].
pub fn example(name: &str) -> Option<&'static str> {
    EXAMPLES
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, text, _)| *text)
}
