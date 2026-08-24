pub mod edge;
pub mod face;
pub mod ray;
pub mod vertex;

pub use edge::{edge_dimensions, pick_edge, EdgeDimension, EdgePickHit};
pub use face::{pick_face, pick_face_details, FaceHit, SurfaceKind};
pub use ray::{point_in_polygon_2d, PickRay};
pub use vertex::{pick_vertex, shape_vertices};
