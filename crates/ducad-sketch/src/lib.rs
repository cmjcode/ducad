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

#[cfg(test)]
mod tests;

pub use commands::{
    DeleteEntities, InsertEntities, RenameEntities, ReplaceEntities, ResizeRectangle,
    TranslateEntities, UndoStack, UpdateEntity,
};
pub use entity::{Entity, EntityId};
pub use ops::{
    arc_from_three_points, line_intersection_params_in_sketch, mirror_entity, offset_entity,
    project_t, reflect_point, translate_entity, trim_segments,
};
pub use region::{
    detect_rectangle, find_closed_regions, find_region_at_point, find_region_containing_entity,
    ClosedRegion, RectAnchor, RectangleShape,
};
pub use sketch::Sketch;
pub use snap::{find_snap, find_snap_with_extra, SnapHit, SnapKind};
