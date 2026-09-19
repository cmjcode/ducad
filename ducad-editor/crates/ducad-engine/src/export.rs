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

/// Ganti cap waktu di `FILE_NAME` header STEP (argumen ke-2) dengan
/// `<date>T00:00:00` agar keluaran build deterministik (P10.2). Teks tanpa
/// `FILE_NAME` dikembalikan apa adanya.
pub fn normalize_step_timestamp(text: &str, date: &str) -> String {
    let Some(start) = text.find("FILE_NAME('") else {
        return text.to_string();
    };
    let after_name = start + "FILE_NAME('".len();
    // Akhir nama: `'` pertama yang bukan bagian dari escape `''`.
    let bytes = text.as_bytes();
    let mut i = after_name;
    while i < bytes.len() {
        if bytes[i] == b'\'' {
            if bytes.get(i + 1) == Some(&b'\'') {
                i += 2;
                continue;
            }
            break;
        }
        i += 1;
    }
    // Berikutnya harus `,'<timestamp>'`.
    let Some(rest) = text.get(i + 1..) else {
        return text.to_string();
    };
    let Some(ts_rel) = rest.strip_prefix(",'") else {
        return text.to_string();
    };
    let ts_start = i + 1 + 2;
    let Some(ts_len) = ts_rel.find('\'') else {
        return text.to_string();
    };
    format!(
        "{}{date}T00:00:00{}",
        &text[..ts_start],
        &text[ts_start + ts_len..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_timestamp_is_replaced() {
        let t = "HEADER;\nFILE_NAME('Open CASCADE Shape Model','2026-09-20T04:34:45',('Author'),(\n";
        let n = normalize_step_timestamp(t, "2026-01-02");
        assert_eq!(
            n,
            "HEADER;\nFILE_NAME('Open CASCADE Shape Model','2026-01-02T00:00:00',('Author'),(\n"
        );
        assert_eq!(normalize_step_timestamp("DATA;", "2026-01-02"), "DATA;");
    }
}
