//! Viewport 3D DUCAD: kamera, grid, dan pipeline wgpu.
//!
//! Konvensi: sumbu Z ke atas (konvensi CAD), satuan milimeter, right-handed.

pub mod camera;
pub mod grid;
pub mod plane;
pub mod scene;
pub mod sketch;

pub use camera::{OrbitCamera, ViewPreset};
pub use grid::LineVertex;
pub use plane::{PlaneKind, SketchPlane};
pub use scene::{MeshVertex, SceneRenderer, ZebraConfig};

// Re-export wgpu milik egui_wgpu supaya seluruh workspace memakai versi
// wgpu yang sama persis dengan egui (mismatch versi = error tipe misterius).
pub use egui_wgpu::wgpu;
