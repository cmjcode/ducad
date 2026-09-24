//! Sketching 2D DUCAD: entitas, hit-testing, snapping, operasi modify
//! (offset/mirror/trim), constraint solver (lihat modul `constraint`), dan
//! command undo/redo.

pub mod commands;
pub mod constraint;
pub mod document;
pub mod entity;
pub mod index;
pub mod infer;
pub mod layer;
pub mod measure;
pub mod ops;
pub mod recognize;
pub mod path_edit;
pub mod region;
pub mod sketch;
pub mod snap;
pub mod style;
pub mod text;

#[cfg(test)]
mod tests;

pub use path_edit::{
    shape_to_path, shape_to_path_circle, shape_to_path_ellipse, shape_to_path_polygon,
    shape_to_path_rect, PenBuilder, VectorShape, KAPPA,
};

pub use commands::{
    CreateLayer, DeleteEntities, DeleteLayer, DeleteText, GroupEntities, InsertEntities, InsertText,
    MoveToLayer, RenameEntities, RenameLayer, ReorderLayers, ReplaceEntities, ResizeRectangle,
    SetLayerFlags, SetStyle, SetStyleField, SetZOrder, StyleField, ToggleConstruction,
    TranslateEntities, UndoStack, Ungroup, UpdateEntity, UpdateText, ZOrderAction,
};
pub use document::{PlaneRef, SketchId, SketchSet, SketchSlot};
pub use entity::{map_exact, Entity, EntityId, PathSeg, Subpath};
pub use index::{IndexedBox, SpatialIndex};
pub use layer::{Group, GroupId, Layer, LayerId, LayerKind, Origin, TextId};
pub use style::{
    BlendMode, FillRule, LineCap, LineJoin, Paint, Rgba, StrokeStyle, Style,
};
pub use ops::{
    arc_from_three_points, biarc_fit, circular_pattern_entities, circular_pattern_entities_with_radius,
    compute_chamfer_2d, compute_entities_centroid, compute_fillet_2d, extend_preview,
    extend_segment, find_all_corners, find_all_fillet_targets, find_corner_lines_at_point,
    line_intersection_params_in_sketch, linear_pattern_entities, mirror_entity,
    multi_arc_parallel_offset_ellipse, multi_arc_parallel_offset_spline, offset_entity,
    offset_entity_multi_arc, project_t, ray_intersect_entity, reflect_point,
    regular_polygon_entities, regular_polygon_vertices, rotate_entity, rotate_point,
    slot_from_points, slot_from_radius, translate_entity, trim_segments, Chamfer2DResult,
    Fillet2DResult, FilletTarget, PolygonMode, SlotMode,
};
pub use infer::{infer_constraints, Inferred, InferOptions, RejectReason};
pub use recognize::{recognize, to_entities, Recognized, Stroke};
pub use region::{
    build_hierarchy, detect_rectangle, find_closed_regions, find_region_at_point,
    find_region_containing_entity, find_region_hierarchy, ClosedRegion, RectAnchor, RectangleShape,
    RegionWithHoles,
};
pub use sketch::Sketch;
pub use snap::{
    all_snap_candidate_points, all_snap_candidate_points_with_exclude_set, find_intersections,
    find_snap, find_snap_with_exclude_set, find_snap_with_extra, SnapHit, SnapKind,
};
pub use text::{
    regenerate_text, spec_to_path_entities, text_to_entities, FontPreset, TextAlign, TextObject,
    TextOptions, TextSpec, DEFAULT_FONT_BYTES,
};


