//! Viewport 3D DUCAD: kamera, grid, dan pipeline wgpu.
//!
//! Konvensi: sumbu Z ke atas (konvensi CAD), satuan milimeter, right-handed.

pub mod camera;
pub mod grid;
pub mod ink;
pub mod plane;
pub mod scene;
pub mod sketch;
pub mod vector;

pub use camera::{CameraMode, OrbitCamera, ViewPreset};
pub use grid::LineVertex;
pub use ink::{
    append_stroke_vertices, build_stroke_vertices, InkBrushKind, InkBrushRef, InkLayerBatch,
    InkPointRef, InkVertex,
};
pub use plane::{PlaneKind, SketchPlane};
pub use scene::{
    BodyInstance, BodyVertex, DraftConfig, InstanceRaw, MeshSource, MeshVertex, SceneRenderer,
    StudioConfig, StudioPreset, ZebraConfig,
};
pub use vector::{
    tessellate_entity, tessellate_fill, tessellate_stroke, LayerBatch, TessError, TessOptions,
    Tessellated, VectorCache, VectorVertex,
};

// Re-export wgpu milik egui_wgpu supaya seluruh workspace memakai versi
// wgpu yang sama persis dengan egui (mismatch versi = error tipe misterius).
pub use egui_wgpu::wgpu;
