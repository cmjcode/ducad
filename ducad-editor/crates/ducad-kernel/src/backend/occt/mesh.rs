//! Bagian backend OCCT dari `crate::mesh` (PLAN_MIGRASI_TRUCK F2.2): kode di
//! sini menyentuh `opencascade`; tipe & logika netral-backend ada di modul induk.

// Tipe & helper netral-backend dari modul induk.
#[allow(unused_imports)]
use crate::mesh::*;

use opencascade::primitives::Shape;

pub(crate) fn tessellate_shape(shape: &Shape) -> KernelMesh {
    let Ok(mesh) = shape.mesh() else {
        return KernelMesh::default();
    };
    let positions = mesh
        .vertices
        .iter()
        .map(|v| [v.x as f32, v.y as f32, v.z as f32])
        .collect();
    let normals = mesh
        .normals
        .iter()
        .map(|n| [n.x as f32, n.y as f32, n.z as f32])
        .collect();
    let indices = mesh.indices.iter().map(|i| *i as u32).collect();
    let face_ranges = mesh
        .face_ranges
        .iter()
        .map(|r| (r.start as u32)..(r.end as u32))
        .collect();
    KernelMesh {
        positions,
        normals,
        indices,
        face_ranges,
    }
}
