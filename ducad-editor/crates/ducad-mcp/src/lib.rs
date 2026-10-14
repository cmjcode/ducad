//! Server MCP DUCAD (stdio, JSON-RPC 2.0, satu thread, tanpa tokio).

// `ducad_engine::OpError` sengaja besar karena bentuknya kontrak JSON
// (lihat `ducad-engine/src/lib.rs`); jalur error di sini jarang dilalui.
#![allow(clippy::result_large_err)]
// Prinsip "jangan pernah crash" (ADR 0009): argumen tool bersifat hostil.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::todo, clippy::unimplemented)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod attach;
pub mod server;
pub mod tools;
