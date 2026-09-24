pub mod cache;
pub mod pipeline;
pub mod tessellate;

#[cfg(test)]
mod tests;

pub use cache::{
    extract_gradient, GradientStop, GradientUniform, LayerBatch, VectorCache,
    MAX_GRADIENTS_PER_BATCH,
};
pub use pipeline::{
    create_vector_pipelines, upload_layer_batch, GpuVectorBatch, VectorPipelines,
};
pub use tessellate::{
    entity_to_subpaths, subpaths_to_stroked_polylines, tessellate_entity, tessellate_fill,
    tessellate_stroke, validate_subpaths_finite, TessError, TessOptions, Tessellated, VectorVertex,
    Z_BASE_OFFSET, Z_LAYER_STEP,
};
