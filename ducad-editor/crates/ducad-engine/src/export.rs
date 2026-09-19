//! Ekspor body sesi ke format pertukaran (dipakai CLI dan MCP).

use std::path::Path;

use ducad_kernel::{KernelMesh, KernelShape};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{OpError, OpErrorCode, OpResult};
use crate::session::Session;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Step,
    Stl,
    Obj,
    Glb,
}

impl std::str::FromStr for ExportFormat {
    type Err = OpError;
    fn from_str(s: &str) -> OpResult<Self> {
        match s.to_ascii_lowercase().as_str() {
            "step" | "stp" => Ok(Self::Step),
            "stl" => Ok(Self::Stl),
            "obj" => Ok(Self::Obj),
            "glb" => Ok(Self::Glb),
            other => Err(OpError::invalid(format!(
                "format ekspor '{other}' tidak dikenal (step, stl, obj, glb)"
            ))),
        }
    }
}

/// Tulis semua body TERLIHAT ke `path`. Mengembalikan ukuran berkas (byte).
pub fn export(s: &Session, format: ExportFormat, path: &Path) -> OpResult<u64> {
    let model = s.model();
    let visible: Vec<(&ducad_core::Body, &crate::model::BodyGeometry)> = model
        .doc
        .bodies
        .iter()
        .filter(|(_, b)| b.visible)
        .filter_map(|(id, b)| Some((b, model.geometry.get(id)?)))
        .collect();
    if visible.is_empty() {
        return Err(OpError::invalid("tidak ada body terlihat untuk diekspor"));
    }
    let io =
        |e: anyhow::Error| OpError::new(OpErrorCode::Io, format!("ekspor {format:?} gagal: {e:#}"));
    match format {
        ExportFormat::Step => {
            let shapes: Vec<&KernelShape> = visible.iter().map(|(_, g)| &g.shape).collect();
            ducad_io::step_io::export(&shapes, path).map_err(io)?;
        }
        ExportFormat::Stl => {
            let meshes: Vec<&KernelMesh> = visible.iter().map(|(_, g)| g.mesh.as_ref()).collect();
            ducad_io::write_stl_binary(&KernelMesh::merge(&meshes), path).map_err(io)?;
        }
        ExportFormat::Obj => {
            let bodies: Vec<(&str, &KernelMesh)> = visible
                .iter()
                .map(|(b, g)| (b.name.as_str(), g.mesh.as_ref()))
                .collect();
            ducad_io::write_obj(&bodies, path).map_err(io)?;
        }
        ExportFormat::Glb => {
            let bodies: Vec<(&str, ducad_core::Material, &KernelMesh)> = visible
                .iter()
                .map(|(b, g)| (b.name.as_str(), b.material, g.mesh.as_ref()))
                .collect();
            ducad_io::write_glb(&bodies, path).map_err(io)?;
        }
    }
    std::fs::metadata(path)
        .map(|m| m.len())
        .map_err(|e| OpError::new(OpErrorCode::Io, format!("berkas ekspor tidak terbaca: {e}")))
}
