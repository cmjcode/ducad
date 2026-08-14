//! Wrapper CADRAW di atas kernel OpenCASCADE (via opencascade-rs).
//!
//! Seluruh aplikasi hanya boleh menyentuh tipe dari crate ini, bukan
//! `opencascade` langsung — agar detail FFI terisolasi dan kernel bisa
//! ditambal/diganti tanpa merombak app.

use anyhow::Result;
use glam::dvec3;
use opencascade::primitives::{IntoShape, Shape};
use opencascade::workplane::Workplane;

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
}

/// Tessellate sebuah shape OCCT menjadi mesh GPU-ready.
pub fn tessellate(shape: &Shape) -> Result<KernelMesh> {
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
    Ok(KernelMesh {
        positions,
        normals,
        indices,
    })
}

/// Smoke-test kemampuan kernel: kotak di-extrude dari sketch lalu difillet
/// — persis alur "sketch → push/pull → fillet" yang jadi inti CADRAW.
pub fn make_filleted_box(width: f64, depth: f64, height: f64, fillet: f64) -> Result<Shape> {
    let profile = Workplane::xy().rect(width, depth);
    let solid = profile.to_face().extrude(dvec3(0.0, 0.0, height));
    let mut shape = solid.into_shape();
    if fillet > 0.0 {
        shape.fillet(fillet);
    }
    Ok(shape)
}

/// Tulis shape ke STL (validasi watertightness lewat tool eksternal).
pub fn write_stl(shape: &Shape, path: &str) -> Result<()> {
    shape.write_stl(path)?;
    Ok(())
}
