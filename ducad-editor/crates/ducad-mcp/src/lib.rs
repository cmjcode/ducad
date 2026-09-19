//! Server MCP DUCAD (stdio, JSON-RPC 2.0, satu thread, tanpa tokio).

// `ducad_engine::OpError` sengaja besar karena bentuknya kontrak JSON
// (lihat `ducad-engine/src/lib.rs`); jalur error di sini jarang dilalui.
#![allow(clippy::result_large_err)]

pub mod server;
pub mod tools;
