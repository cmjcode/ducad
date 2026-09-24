//! Pipeline dan geometri rendering tinta goresan bertekanan (`InkVertex`, `InkBrushRef`, `InkPointRef`).

pub mod pipeline;
pub mod stroke;

#[cfg(test)]
mod tests;

pub use pipeline::{create_ink_pipelines, upload_ink_layer_batch, GpuInkLayer, InkPipelines};
pub use stroke::{
    append_stroke_vertices, build_stroke_vertices, InkBrushKind, InkBrushRef, InkLayerBatch,
    InkPointRef, InkVertex, INK_Z_OFFSET,
};
