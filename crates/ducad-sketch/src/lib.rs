//! Sketching 2D DUCAD: entitas, hit-testing, snapping, operasi modify
//! (offset/mirror/trim), constraint solver (lihat modul `constraint`), dan
//! command undo/redo.

pub mod commands;
pub mod constraint;
pub mod entity;
pub mod measure;
pub mod ops;
pub mod region;
pub mod sketch;
pub mod snap;
pub mod text;

#[cfg(test)]
mod tests;

pub use commands::{
    DeleteEntities, InsertEntities, RenameEntities, ReplaceEntities, ResizeRectangle,
    ToggleConstruction, TranslateEntities, UndoStack, UpdateEntity,
};
pub use entity::{Entity, EntityId};
pub use ops::{
    arc_from_three_points, circular_pattern_entities, circular_pattern_entities_with_radius,
    compute_chamfer_2d, compute_entities_centroid, compute_fillet_2d, find_all_corners,
    find_all_fillet_targets, find_corner_lines_at_point, line_intersection_params_in_sketch,
    linear_pattern_entities, mirror_entity, offset_entity, project_t, reflect_point,
    regular_polygon_entities, regular_polygon_vertices, rotate_entity, rotate_point,
    slot_from_points, slot_from_radius, translate_entity, trim_segments, Chamfer2DResult,
    Fillet2DResult, FilletTarget, PolygonMode, SlotMode,
};
pub use region::{
    detect_rectangle, find_closed_regions, find_region_at_point, find_region_containing_entity,
    ClosedRegion, RectAnchor, RectangleShape,
};
pub use sketch::Sketch;
pub use snap::{
    all_snap_candidate_points, all_snap_candidate_points_with_exclude_set, find_intersections,
    find_snap, find_snap_with_exclude_set, find_snap_with_extra, SnapHit, SnapKind,
};
pub use text::{text_to_entities, FontPreset, TextAlign, TextOptions, DEFAULT_FONT_BYTES};


