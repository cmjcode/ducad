//! Skema operasi (`Op`) — kontrak JSON untuk agent/CLI/MCP.

pub mod num;
pub(crate) mod sketch;
pub mod spec;

pub use num::{eval, eval_arr, Num, Params};
pub use spec::*;
