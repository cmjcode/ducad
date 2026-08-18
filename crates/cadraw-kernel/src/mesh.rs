use opencascade::primitives::Shape;

/// Mesh hasil tessellation, siap di-upload ke GPU (f32, indexed).
#[derive(Debug, Clone, Default)]
pub struct KernelMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

impl KernelMesh {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Gabungkan beberapa mesh jadi satu buffer, menggeser indeks per mesh
    /// supaya tetap valid. Dipakai render (satu draw call untuk semua body
    /// visible) dan export STL/OBJ multi-body (Fase 5, `cadraw-io`) — dua
    /// pemakai yang sebelumnya menduplikasi logika gabung-mesh ini sendiri.
    pub fn merge(meshes: &[&KernelMesh]) -> KernelMesh {
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut indices = Vec::new();
        for mesh in meshes {
            let offset = positions.len() as u32;
            positions.extend_from_slice(&mesh.positions);
            normals.extend_from_slice(&mesh.normals);
            indices.extend(mesh.indices.iter().map(|i| i + offset));
        }
        KernelMesh {
            positions,
            normals,
            indices,
        }
    }
}

pub(crate) fn tessellate_shape(shape: &Shape) -> KernelMesh {
    let mesh = shape.mesh();
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
    KernelMesh {
        positions,
        normals,
        indices,
    }
}
