//! Export mesh tessellation ke STL (biner) dan OBJ (Fase 5) — dua format
//! yang sengaja hanya EXPORT (bukan lossless, sudah berupa segitiga, tidak
//! ada jalan balik ke B-rep — import STL/OBJ diputuskan di luar lingkup
//! Fase 5, lihat docs/PLAN.md).
//!
//! Ditulis sendiri dari `KernelMesh` (bukan lewat `KernelShape::write_stl`
//! milik kernel) supaya bisa menggabungkan BEBERAPA body jadi satu file —
//! kernel cuma tahu cara menulis satu shape sekaligus. `KernelMesh::merge`
//! (dipakai render juga) yang menyatukan mesh multi-body sebelum sampai
//! di sini.

use anyhow::{Context, Result};
use ducad_kernel::KernelMesh;
use std::path::Path;

/// Tulis satu mesh (sudah digabung kalau multi-body, lihat `KernelMesh::merge`)
/// sebagai STL biner — format de-facto standar dunia percetakan 3D/slicer,
/// jauh lebih ringkas dari ASCII STL untuk mesh besar.
///
/// Normal per-facet dihitung ULANG dari cross product 3 vertex segitiga
/// (bukan dipakai dari `mesh.normals`, yang per-VERTEX bukan per-FACET) —
/// standar STL memang menyimpan satu normal per facet, dan menghitungnya
/// dari geometri segitiga sendiri menjamin konsisten (tidak bergantung
/// smoothing normal tessellation OCCT).
pub fn write_stl_binary(mesh: &KernelMesh, path: impl AsRef<Path>) -> Result<()> {
    let mut buf = Vec::with_capacity(84 + mesh.triangle_count() * 50);

    let mut header = [0u8; 80];
    let banner = b"DUCAD STL export";
    let n = banner.len().min(80);
    header[..n].copy_from_slice(&banner[..n]);
    buf.extend_from_slice(&header);
    buf.extend_from_slice(&(mesh.triangle_count() as u32).to_le_bytes());

    for tri in mesh.indices.chunks_exact(3) {
        let a = mesh.positions[tri[0] as usize];
        let b = mesh.positions[tri[1] as usize];
        let c = mesh.positions[tri[2] as usize];
        let normal = facet_normal(a, b, c);
        for v in [normal, a, b, c] {
            buf.extend_from_slice(&v[0].to_le_bytes());
            buf.extend_from_slice(&v[1].to_le_bytes());
            buf.extend_from_slice(&v[2].to_le_bytes());
        }
        buf.extend_from_slice(&0u16.to_le_bytes()); // attribute byte count
    }

    std::fs::write(path, buf).context("gagal menulis STL")
}

fn facet_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len < 1e-12 {
        [0.0, 0.0, 0.0]
    } else {
        [n[0] / len, n[1] / len, n[2] / len]
    }
}

/// Tulis beberapa body (nama + mesh) sebagai satu file OBJ ASCII, satu
/// blok `o <nama>` per body sehingga tool lain (Blender, MeshLab, dst)
/// tetap melihatnya sebagai objek terpisah — beda dari STL biner yang
/// harus digabung jadi satu mesh (`KernelMesh::merge`) sebelum ditulis.
pub fn write_obj(bodies: &[(&str, &KernelMesh)], path: impl AsRef<Path>) -> Result<()> {
    let mut out = String::from("# DUCAD OBJ export\n");
    let mut vertex_offset = 0usize;

    for (name, mesh) in bodies {
        out.push_str(&format!("o {}\n", sanitize_obj_name(name)));
        for p in &mesh.positions {
            out.push_str(&format!("v {} {} {}\n", p[0], p[1], p[2]));
        }
        for n in &mesh.normals {
            out.push_str(&format!("vn {} {} {}\n", n[0], n[1], n[2]));
        }
        for tri in mesh.indices.chunks_exact(3) {
            // OBJ 1-based; `v//vn` sah karena `positions[i]`/`normals[i]`
            // selalu berpasangan 1:1 (kontrak `KernelMesh`), jadi indeks
            // vertex & normal sama persis.
            let idx = |i: u32| i as usize + 1 + vertex_offset;
            let (i0, i1, i2) = (idx(tri[0]), idx(tri[1]), idx(tri[2]));
            out.push_str(&format!("f {i0}//{i0} {i1}//{i1} {i2}//{i2}\n"));
        }
        vertex_offset += mesh.positions.len();
    }

    std::fs::write(path, out).context("gagal menulis OBJ")
}

fn sanitize_obj_name(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_whitespace() { '_' } else { c }).collect();
    if s.is_empty() {
        "body".to_string()
    } else {
        s
    }
}

/// Baca file STL (mendeteksi biner vs ASCII secara otomatis) menjadi `KernelMesh`.
pub fn read_stl(path: impl AsRef<Path>) -> Result<KernelMesh> {
    let bytes = std::fs::read(&path).context("gagal membaca file STL")?;
    read_stl_from_bytes(&bytes)
}

/// Parse data byte STL (biner atau ASCII) menjadi `KernelMesh`.
pub fn read_stl_from_bytes(bytes: &[u8]) -> Result<KernelMesh> {
    if bytes.len() >= 84 {
        let tri_count = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
        let expected_len = 84 + tri_count * 50;
        if bytes.len() == expected_len {
            return read_stl_binary_bytes(bytes);
        }
    }

    if let Ok(text) = std::str::from_utf8(bytes) {
        if let Ok(mesh) = read_stl_ascii_str(text) {
            if mesh.triangle_count() > 0 {
                return Ok(mesh);
            }
        }
    }

    if bytes.len() >= 84 {
        if let Ok(mesh) = read_stl_binary_bytes(bytes) {
            if mesh.triangle_count() > 0 {
                return Ok(mesh);
            }
        }
    }

    anyhow::bail!("Format berkas STL tidak valid atau rusak")
}

/// Parse format STL biner dari slice byte.
pub fn read_stl_binary_bytes(bytes: &[u8]) -> Result<KernelMesh> {
    if bytes.len() < 84 {
        anyhow::bail!("Ukuran file STL binary terlalu kecil (kurang dari 84 byte)");
    }
    let tri_count = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
    let mut positions = Vec::with_capacity(tri_count * 3);
    let mut normals = Vec::with_capacity(tri_count * 3);
    let mut indices = Vec::with_capacity(tri_count * 3);

    let mut offset = 84;
    for _ in 0..tri_count {
        if offset + 50 > bytes.len() {
            break;
        }
        let read_f32 = |off: usize| -> f32 {
            f32::from_le_bytes(bytes[off..off + 4].try_into().unwrap())
        };

        let normal = [read_f32(offset), read_f32(offset + 4), read_f32(offset + 8)];
        let v0 = [read_f32(offset + 12), read_f32(offset + 16), read_f32(offset + 20)];
        let v1 = [read_f32(offset + 24), read_f32(offset + 28), read_f32(offset + 32)];
        let v2 = [read_f32(offset + 36), read_f32(offset + 40), read_f32(offset + 44)];

        let computed_norm = if normal[0].is_nan()
            || (normal[0] == 0.0 && normal[1] == 0.0 && normal[2] == 0.0)
        {
            facet_normal(v0, v1, v2)
        } else {
            normal
        };

        let base_idx = positions.len() as u32;
        positions.push(v0);
        positions.push(v1);
        positions.push(v2);
        normals.push(computed_norm);
        normals.push(computed_norm);
        normals.push(computed_norm);
        indices.push(base_idx);
        indices.push(base_idx + 1);
        indices.push(base_idx + 2);

        offset += 50;
    }

    Ok(KernelMesh {
        positions,
        normals,
        indices,
    })
}

/// Parse format STL ASCII dari teks string.
pub fn read_stl_ascii_str(text: &str) -> Result<KernelMesh> {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    let mut current_normal = [0.0f32, 0.0, 1.0];
    let mut current_vertices = Vec::with_capacity(3);

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("facet normal") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 5 {
                let nx = parts[2].parse::<f32>().unwrap_or(0.0);
                let ny = parts[3].parse::<f32>().unwrap_or(0.0);
                let nz = parts[4].parse::<f32>().unwrap_or(1.0);
                current_normal = [nx, ny, nz];
            }
            current_vertices.clear();
        } else if trimmed.starts_with("vertex") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 4 {
                let vx = parts[1].parse::<f32>().unwrap_or(0.0);
                let vy = parts[2].parse::<f32>().unwrap_or(0.0);
                let vz = parts[3].parse::<f32>().unwrap_or(0.0);
                current_vertices.push([vx, vy, vz]);
            }
        } else if trimmed.starts_with("endfacet") {
            if current_vertices.len() == 3 {
                let v0 = current_vertices[0];
                let v1 = current_vertices[1];
                let v2 = current_vertices[2];
                let norm = if current_normal[0] == 0.0
                    && current_normal[1] == 0.0
                    && current_normal[2] == 0.0
                {
                    facet_normal(v0, v1, v2)
                } else {
                    current_normal
                };

                let base_idx = positions.len() as u32;
                positions.push(v0);
                positions.push(v1);
                positions.push(v2);
                normals.push(norm);
                normals.push(norm);
                normals.push(norm);
                indices.push(base_idx);
                indices.push(base_idx + 1);
                indices.push(base_idx + 2);
            }
            current_vertices.clear();
        }
    }

    Ok(KernelMesh {
        positions,
        normals,
        indices,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle_mesh() -> KernelMesh {
        KernelMesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            indices: vec![0, 1, 2],
        }
    }

    #[test]
    fn write_stl_binary_header_and_triangle_count() {
        let mesh = triangle_mesh();
        let path = std::env::temp_dir().join(format!("ducad-io-test-{}.stl", std::process::id()));
        write_stl_binary(&mesh, &path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(bytes.len(), 80 + 4 + 50);
        let count = u32::from_le_bytes(bytes[80..84].try_into().unwrap());
        assert_eq!(count, 1);
        // Normal segitiga (0,0,0)-(1,0,0)-(0,1,0) harus +Z.
        let nz = f32::from_le_bytes(bytes[92..96].try_into().unwrap());
        assert!(nz > 0.9, "normal Z harus mendekati 1.0, dapat {nz}");
    }

    #[test]
    fn read_write_stl_binary_roundtrip() {
        let mesh = triangle_mesh();
        let path = std::env::temp_dir().join(format!("ducad-io-roundtrip-{}.stl", std::process::id()));
        write_stl_binary(&mesh, &path).unwrap();
        let loaded = read_stl(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(loaded.triangle_count(), 1);
        assert_eq!(loaded.positions.len(), 3);
        assert_eq!(loaded.positions[0], [0.0, 0.0, 0.0]);
        assert_eq!(loaded.positions[1], [1.0, 0.0, 0.0]);
        assert_eq!(loaded.positions[2], [0.0, 1.0, 0.0]);
    }

    #[test]
    fn read_stl_ascii_format() {
        let ascii_data = r#"solid test
  facet normal 0.0 0.0 1.0
    outer loop
      vertex 0.0 0.0 0.0
      vertex 10.0 0.0 0.0
      vertex 0.0 10.0 0.0
    endloop
  endfacet
endsolid test
"#;
        let loaded = read_stl_ascii_str(ascii_data).unwrap();
        assert_eq!(loaded.triangle_count(), 1);
        assert_eq!(loaded.positions.len(), 3);
        assert_eq!(loaded.positions[1], [10.0, 0.0, 0.0]);
    }

    #[test]
    fn write_obj_line_counts_match_mesh() {
        let mesh = triangle_mesh();
        let path = std::env::temp_dir().join(format!("ducad-io-test-{}.obj", std::process::id()));
        write_obj(&[("Body 1", &mesh)], &path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(text.lines().filter(|l| l.starts_with("v ")).count(), 3);
        assert_eq!(text.lines().filter(|l| l.starts_with("vn ")).count(), 3);
        assert_eq!(text.lines().filter(|l| l.starts_with("f ")).count(), 1);
        assert!(text.contains("o Body_1\n"));
    }

    #[test]
    fn write_obj_multi_body_offsets_indices() {
        let mesh = triangle_mesh();
        let path = std::env::temp_dir().join(format!("ducad-io-test-multi-{}.obj", std::process::id()));
        write_obj(&[("A", &mesh), ("B", &mesh)], &path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        // Body kedua harus memakai indeks vertex 4/5/6 (offset 3 dari body
        // pertama), bukan 1/2/3 lagi.
        let f_lines: Vec<&str> = text.lines().filter(|l| l.starts_with("f ")).collect();
        assert_eq!(f_lines.len(), 2);
        assert_eq!(f_lines[1], "f 4//4 5//5 6//6");
    }
}
