//! `ducad-engine` — lapisan modeling headless DUCAD.
//!
//! Crate ini menyatukan sketch (`ducad-sketch`), kernel (`ducad-kernel`),
//! dan I/O (`ducad-io`) TANPA egui/wgpu, sehingga operasi modeling yang
//! sama bisa dipanggil dari GUI, CLI, maupun server MCP.

pub mod error;
pub mod model;
pub mod plane;
pub mod profile;

pub use error::{OpError, OpErrorCode, OpResult};
pub use plane::PlaneFrame;

#[test]
fn engine_has_no_gui_dependency() {
    let manifest = include_str!("../Cargo.toml");
    for banned in ["egui", "eframe", "wgpu", "ducad-render", "ducad-ui", "rfd"] {
        assert!(!manifest.contains(banned), "ducad-engine tidak boleh bergantung pada {banned}");
    }
}
