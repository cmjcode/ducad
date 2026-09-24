//! Modul render vektor 2D untuk DUCAD (`ducad-render`).

pub mod tessellate;

#[cfg(test)]
mod tests;

pub use tessellate::{
    entity_to_subpaths, subpaths_to_stroked_polylines, tessellate_entity, tessellate_fill,
    tessellate_stroke, validate_subpaths_finite, TessError, TessOptions, Tessellated, VectorVertex,
    Z_BASE_OFFSET, Z_LAYER_STEP,
};
