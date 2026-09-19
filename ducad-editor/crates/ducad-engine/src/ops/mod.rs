//! Skema operasi (`Op`) — kontrak JSON untuk agent/CLI/MCP.

pub mod num;
pub(crate) mod sketch;
pub mod spec;

pub use num::{eval, eval_arr, Num, Params};
pub use spec::*;

/// Contoh OpFile lengkap (plat 60×40×8, fillet tepi tegak, 4 lubang M5).
pub const EXAMPLE_PLATE: &str = include_str!("../../tests/fixtures/plate.ops.json");
