//! DUCAD: topology queries the DUCAD backend needs and the SolidCraft API does not offer (not
//! part of SolidCraft upstream; see vendors/solidcraft/README.md). Read-only views of a body's
//! B-rep, in the orders the rest of the API already uses: faces in `face_iter` order (as
//! `Mesh::tri_face`, [`crate::face_surface`]), edges in [`Body::edges`] / `Mesh::edges` order.

use std::collections::HashMap;

use solidcraft_geom::Vec3;
use truck_modeling as mt;

use crate::body::from_p3;
use crate::{Body, Result, guard};

/// One use of an edge in a face boundary wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgeUse {
    /// Index in [`Body::edges`] order.
    pub edge: usize,
    /// The wire runs along the edge from its front to its back (see [`edge_ends`]).
    pub forward: bool,
}

/// Each face's boundary wires (truck keeps the outer one first), each wire its edge uses in
/// order. Empty for a mesh body.
pub fn face_wires(b: &Body) -> Result<Vec<Vec<Vec<EdgeUse>>>> {
    if b.is_mesh() {
        return Ok(Vec::new());
    }
    guard("face wires", || {
        let edges = Body::unique_edges(&b.solid);
        let index: HashMap<mt::EdgeID, (usize, bool)> =
            edges.iter().enumerate().map(|(i, e)| (e.id(), (i, e.orientation()))).collect();
        Ok(b.solid
            .face_iter()
            .map(|f| {
                f.absolute_boundaries()
                    .iter()
                    .map(|w| {
                        w.edge_iter()
                            .filter_map(|e| {
                                index.get(&e.id()).map(|&(i, o)| EdgeUse { edge: i, forward: e.orientation() == o })
                            })
                            .collect()
                    })
                    .collect()
            })
            .collect())
    })
}

/// Each edge's front and back point ([`Body::edges`] order). Equal for a closed edge. Empty for
/// a mesh body.
pub fn edge_ends(b: &Body) -> Result<Vec<(Vec3, Vec3)>> {
    if b.is_mesh() {
        return Ok(Vec::new());
    }
    guard("edge ends", || {
        Ok(Body::unique_edges(&b.solid).iter().map(|e| (from_p3(e.front().point()), from_p3(e.back().point()))).collect())
    })
}

/// The body's distinct B-rep vertices. Empty for a mesh body.
pub fn vertex_points(b: &Body) -> Result<Vec<Vec3>> {
    if b.is_mesh() {
        return Ok(Vec::new());
    }
    guard("vertex points", || {
        let mut seen = std::collections::HashSet::new();
        Ok(b.solid.vertex_iter().filter(|v| seen.insert(v.id())).map(|v| from_p3(v.point())).collect())
    })
}
