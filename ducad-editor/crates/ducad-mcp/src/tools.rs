//! Daftar tool MCP — diisi di P3.2.

use serde_json::{json, Value};

use crate::server::Server;

pub const TOOL_NAMES: &[&str] = &[];

/// Daftar tool untuk `tools/list`.
pub fn definitions() -> Vec<Value> {
    Vec::new()
}

/// Jalankan tool.
pub fn call(_server: &mut Server, name: &str, _arguments: Value) -> Value {
    json!({ "content": [{ "type": "text", "text": format!("tool {name} belum tersedia") }], "isError": true })
}
