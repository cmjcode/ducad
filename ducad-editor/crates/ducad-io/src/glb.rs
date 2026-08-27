//! Export model 3D solid DUCAD ke format binary glTF 2.0 (`.glb`) (Fase 11.4).
//!
//! Format GLB adalah format standar terbuka de-facto untuk penayangan 3D di
//! web e-commerce (<model-viewer>, Three.js, Babylon.js), media sosial 3D,
//! serta Augmented Reality (AR Quick Look di Apple iOS/iPadOS dan Scene Viewer di Android).
//!
//! Modul ini menyertakan:
//! 1. Konversi mesh geometris f32 indexed (posisi, normal per-vertex, indeks segitiga).
//! 2. Transformasi koordinat standar CAD Z-up ke glTF Y-up (+X kanan, +Y atas, -Z kedalaman).
//! 3. Skala metrik standar AR (konversi millimeter internal CAD ke meter glTF: faktor 0.001).
//! 4. Material PBR lengkap (baseColorFactor Albedo RGBA, metallicFactor, roughnessFactor,
//!    transparency alpha blending, doubleSided, dan ekstensi KHR_materials_clearcoat).
//! 5. Penataan chunk biner 4-byte aligned sesuai spesifikasi resmi glTF 2.0 GLB.

use std::path::Path;
use anyhow::{Context, Result};
use ducad_core::Material;
use ducad_kernel::KernelMesh;
use serde_json::{json, Value};

/// Opsi konfigurasi ekspor GLB / glTF.
#[derive(Debug, Clone)]
pub struct GlbExportOptions {
    /// Konversi orientasi CAD (Z-up) ke standar glTF (Y-up). Default: true.
    pub y_up: bool,
    /// Faktor skala panjang (misal 0.001 untuk konversi mm CAD ke meter di AR/Web, atau 1.0 untuk satuan CAD). Default: 0.001.
    pub scale: f32,
    /// Identitas generator pembuat file di metadata asset glTF.
    pub generator: String,
    /// Sertakan ekstensi KHR_materials_clearcoat jika material memiliki lapisan clearcoat > 0.
    pub include_clearcoat_extension: bool,
}

impl Default for GlbExportOptions {
    fn default() -> Self {
        Self {
            y_up: true,
            scale: 0.001,
            generator: "DuCAD 3D CAD & Modeling Engine (glTF 2.0 / GLB Export)".to_string(),
            include_clearcoat_extension: true,
        }
    }
}

/// Ekspor kumpulan solid body (nama, material PBR, mesh) ke berkas binary `.glb`.
pub fn write_glb(
    bodies: &[(&str, Material, &KernelMesh)],
    path: impl AsRef<Path>,
) -> Result<()> {
    write_glb_with_options(bodies, path, &GlbExportOptions::default())
}

/// Ekspor kumpulan solid body dengan opsi kustom ke berkas binary `.glb`.
pub fn write_glb_with_options(
    bodies: &[(&str, Material, &KernelMesh)],
    path: impl AsRef<Path>,
    options: &GlbExportOptions,
) -> Result<()> {
    let glb_bytes = export_glb_bytes(bodies, options)?;
    std::fs::write(path.as_ref(), glb_bytes).with_context(|| {
        format!(
            "Gagal menulis file GLB ke {}",
            path.as_ref().display()
        )
    })
}

/// Serialisasi kumpulan solid body menjadi byte buffer binary `.glb` utuh.
pub fn export_glb_bytes(
    bodies: &[(&str, Material, &KernelMesh)],
    options: &GlbExportOptions,
) -> Result<Vec<u8>> {
    let mut bin_buffer: Vec<u8> = Vec::new();
    let mut buffer_views: Vec<Value> = Vec::new();
    let mut accessors: Vec<Value> = Vec::new();
    let mut materials: Vec<Value> = Vec::new();
    let mut meshes: Vec<Value> = Vec::new();
    let mut nodes: Vec<Value> = Vec::new();
    let mut extensions_used: Vec<String> = Vec::new();

    for (raw_name, material, mesh) in bodies {
        if mesh.positions.is_empty() || mesh.indices.is_empty() {
            continue;
        }

        let name = sanitize_name(raw_name);

        // 1. Transformasi Posisi & Kalkulasi Bounding Box Min/Max
        let mut transformed_positions = Vec::with_capacity(mesh.positions.len());
        let mut min_pos = [f32::INFINITY; 3];
        let mut max_pos = [f32::NEG_INFINITY; 3];

        for p in &mesh.positions {
            let tp = if options.y_up {
                // CAD (X, Y, Z_up) -> glTF (X, Z_up as Y, -Y as Z)
                [p[0] * options.scale, p[2] * options.scale, -p[1] * options.scale]
            } else {
                [p[0] * options.scale, p[1] * options.scale, p[2] * options.scale]
            };

            for c in 0..3 {
                if tp[c] < min_pos[c] {
                    min_pos[c] = tp[c];
                }
                if tp[c] > max_pos[c] {
                    max_pos[c] = tp[c];
                }
            }
            transformed_positions.push(tp);
        }

        // 2. Transformasi Normal per-vertex
        let mut transformed_normals = Vec::with_capacity(mesh.normals.len());
        for n in &mesh.normals {
            let tn = if options.y_up {
                [n[0], n[2], -n[1]]
            } else {
                [n[0], n[1], n[2]]
            };
            let len = (tn[0] * tn[0] + tn[1] * tn[1] + tn[2] * tn[2]).sqrt();
            let norm = if len > 1e-12 {
                [tn[0] / len, tn[1] / len, tn[2] / len]
            } else {
                [0.0, 1.0, 0.0]
            };
            transformed_normals.push(norm);
        }

        // Align bin_buffer ke 4-byte boundary untuk vertex positions
        pad_buffer_4bytes(&mut bin_buffer);
        let pos_byte_offset = bin_buffer.len();
        for p in &transformed_positions {
            bin_buffer.extend_from_slice(&p[0].to_le_bytes());
            bin_buffer.extend_from_slice(&p[1].to_le_bytes());
            bin_buffer.extend_from_slice(&p[2].to_le_bytes());
        }
        let pos_byte_length = bin_buffer.len() - pos_byte_offset;

        let pos_bv_idx = buffer_views.len();
        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": pos_byte_offset,
            "byteLength": pos_byte_length,
            "target": 34962 // ARRAY_BUFFER
        }));

        let pos_acc_idx = accessors.len();
        accessors.push(json!({
            "bufferView": pos_bv_idx,
            "byteOffset": 0,
            "componentType": 5126, // FLOAT
            "count": transformed_positions.len(),
            "type": "VEC3",
            "min": [min_pos[0], min_pos[1], min_pos[2]],
            "max": [max_pos[0], max_pos[1], max_pos[2]]
        }));

        // Align bin_buffer ke 4-byte boundary untuk vertex normals
        pad_buffer_4bytes(&mut bin_buffer);
        let norm_byte_offset = bin_buffer.len();
        for n in &transformed_normals {
            bin_buffer.extend_from_slice(&n[0].to_le_bytes());
            bin_buffer.extend_from_slice(&n[1].to_le_bytes());
            bin_buffer.extend_from_slice(&n[2].to_le_bytes());
        }
        let norm_byte_length = bin_buffer.len() - norm_byte_offset;

        let norm_bv_idx = buffer_views.len();
        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": norm_byte_offset,
            "byteLength": norm_byte_length,
            "target": 34962 // ARRAY_BUFFER
        }));

        let norm_acc_idx = accessors.len();
        accessors.push(json!({
            "bufferView": norm_bv_idx,
            "byteOffset": 0,
            "componentType": 5126, // FLOAT
            "count": transformed_normals.len(),
            "type": "VEC3"
        }));

        // Align bin_buffer ke 4-byte boundary untuk indices
        pad_buffer_4bytes(&mut bin_buffer);
        let idx_byte_offset = bin_buffer.len();
        for &idx in &mesh.indices {
            bin_buffer.extend_from_slice(&idx.to_le_bytes());
        }
        let idx_byte_length = bin_buffer.len() - idx_byte_offset;

        let idx_bv_idx = buffer_views.len();
        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": idx_byte_offset,
            "byteLength": idx_byte_length,
            "target": 34963 // ELEMENT_ARRAY_BUFFER
        }));

        let idx_acc_idx = accessors.len();
        accessors.push(json!({
            "bufferView": idx_bv_idx,
            "byteOffset": 0,
            "componentType": 5125, // UNSIGNED_INT
            "count": mesh.indices.len(),
            "type": "SCALAR"
        }));

        // 3. Konfigurasi Material PBR (Physically-Based Rendering)
        let mat_idx = materials.len();
        let mut mat_json = json!({
            "name": format!("{}_Material", name),
            "pbrMetallicRoughness": {
                "baseColorFactor": [
                    material.base_color[0],
                    material.base_color[1],
                    material.base_color[2],
                    material.base_color[3]
                ],
                "metallicFactor": material.metallic.clamp(0.0, 1.0),
                "roughnessFactor": material.roughness.clamp(0.0, 1.0)
            },
            "alphaMode": if material.is_translucent() { "BLEND" } else { "OPAQUE" },
            "doubleSided": true
        });

        if options.include_clearcoat_extension && material.clearcoat > 0.001 {
            let ext_name = "KHR_materials_clearcoat";
            if !extensions_used.contains(&ext_name.to_string()) {
                extensions_used.push(ext_name.to_string());
            }
            if let Some(obj) = mat_json.as_object_mut() {
                obj.insert(
                    "extensions".to_string(),
                    json!({
                        "KHR_materials_clearcoat": {
                            "clearcoatFactor": material.clearcoat.clamp(0.0, 1.0),
                            "clearcoatRoughnessFactor": 0.03
                        }
                    }),
                );
            }
        }
        materials.push(mat_json);

        // 4. Mesh Primitive
        let mesh_idx = meshes.len();
        meshes.push(json!({
            "name": name.clone(),
            "primitives": [
                {
                    "attributes": {
                        "POSITION": pos_acc_idx,
                        "NORMAL": norm_acc_idx
                    },
                    "indices": idx_acc_idx,
                    "material": mat_idx,
                    "mode": 4 // TRIANGLES
                }
            ]
        }));

        // 5. Node
        nodes.push(json!({
            "name": name,
            "mesh": mesh_idx
        }));
    }

    // Jika tidak ada mesh valid, buat node kosong minimal
    if nodes.is_empty() {
        nodes.push(json!({
            "name": "EmptyModel"
        }));
    }

    let node_indices: Vec<usize> = (0..nodes.len()).collect();

    // 6. Bangun Struktur Root glTF JSON
    let mut gltf_root = json!({
        "asset": {
            "version": "2.0",
            "generator": options.generator.as_str()
        },
        "scene": 0,
        "scenes": [
            {
                "name": "DuCAD_Scene",
                "nodes": node_indices
            }
        ],
        "nodes": nodes,
        "meshes": meshes,
        "materials": materials,
        "accessors": accessors,
        "bufferViews": buffer_views,
        "buffers": [
            {
                "byteLength": bin_buffer.len()
            }
        ]
    });

    if !extensions_used.is_empty() {
        if let Some(root_obj) = gltf_root.as_object_mut() {
            root_obj.insert(
                "extensionsUsed".to_string(),
                json!(extensions_used),
            );
        }
    }

    // 7. Serialisasi JSON dan Padding ke Kelipatan 4 Byte (ASCII Space 0x20)
    let json_str = serde_json::to_string(&gltf_root)?;
    let mut json_bytes = json_str.into_bytes();
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(0x20); // space padding sesuai spesifikasi glTF
    }

    // Pad BIN buffer ke kelipatan 4 byte (0x00)
    pad_buffer_4bytes(&mut bin_buffer);

    // 8. Bentuk Binary GLB Container
    // Header: 12 bytes
    // JSON Chunk: 8 bytes header + json_bytes
    // BIN Chunk: 8 bytes header + bin_bytes (jika ada)
    let total_file_length = 12 + (8 + json_bytes.len()) + if !bin_buffer.is_empty() {
        8 + bin_buffer.len()
    } else {
        0
    };

    let mut glb: Vec<u8> = Vec::with_capacity(total_file_length);

    // Header GLB
    glb.extend_from_slice(&0x46546C67u32.to_le_bytes()); // magic 'glTF'
    glb.extend_from_slice(&2u32.to_le_bytes());          // version 2
    glb.extend_from_slice(&(total_file_length as u32).to_le_bytes()); // total file size

    // Chunk 0: JSON
    glb.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes()); // chunkLength
    glb.extend_from_slice(&0x4E4F534Au32.to_le_bytes());             // chunkType 'JSON'
    glb.extend_from_slice(&json_bytes);

    // Chunk 1: BIN
    if !bin_buffer.is_empty() {
        glb.extend_from_slice(&(bin_buffer.len() as u32).to_le_bytes()); // chunkLength
        glb.extend_from_slice(&0x004E4942u32.to_le_bytes());             // chunkType 'BIN\0'
        glb.extend_from_slice(&bin_buffer);
    }

    Ok(glb)
}

fn pad_buffer_4bytes(buf: &mut Vec<u8>) {
    while !buf.len().is_multiple_of(4) {
        buf.push(0x00);
    }
}

fn sanitize_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    if s.is_empty() {
        "Body".to_string()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_triangle_mesh() -> KernelMesh {
        KernelMesh {
            positions: vec![
                [0.0, 0.0, 0.0],
                [100.0, 0.0, 0.0],
                [0.0, 100.0, 0.0],
            ],
            normals: vec![
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
            ],
            indices: vec![0, 1, 2],
        }
    }

    #[test]
    fn test_export_glb_valid_header_and_chunks() {
        let mesh = sample_triangle_mesh();
        let material = Material::glossy_plastic(Some([1.0, 0.5, 0.0, 1.0]));

        let glb_bytes = export_glb_bytes(&[("TestPart", material, &mesh)], &GlbExportOptions::default()).unwrap();

        // Validasi header
        assert!(glb_bytes.len() >= 20);
        let magic = u32::from_le_bytes(glb_bytes[0..4].try_into().unwrap());
        assert_eq!(magic, 0x46546C67, "Magic harus 'glTF'");

        let version = u32::from_le_bytes(glb_bytes[4..8].try_into().unwrap());
        assert_eq!(version, 2, "Versi glTF harus 2");

        let total_length = u32::from_le_bytes(glb_bytes[8..12].try_into().unwrap());
        assert_eq!(total_length as usize, glb_bytes.len());

        // Validasi JSON chunk
        let json_chunk_len = u32::from_le_bytes(glb_bytes[12..16].try_into().unwrap()) as usize;
        let json_chunk_type = u32::from_le_bytes(glb_bytes[16..20].try_into().unwrap());
        assert_eq!(json_chunk_type, 0x4E4F534A, "Chunk 0 harus 'JSON'");
        assert_eq!(json_chunk_len % 4, 0, "JSON chunk harus 4-byte aligned");

        let json_raw = &glb_bytes[20..20 + json_chunk_len];
        let json_val: Value = serde_json::from_slice(json_raw).expect("JSON chunk harus valid");

        // Validasi struktur glTF
        assert_eq!(json_val["asset"]["version"], "2.0");
        assert_eq!(json_val["meshes"].as_array().unwrap().len(), 1);
        assert_eq!(json_val["materials"].as_array().unwrap().len(), 1);

        // Validasi PBR material
        let mat = &json_val["materials"][0];
        let base_color = &mat["pbrMetallicRoughness"]["baseColorFactor"];
        assert_eq!(base_color[0], 1.0);
        assert_eq!(base_color[1], 0.5);
        assert_eq!(base_color[2], 0.0);
        assert_eq!(base_color[3], 1.0);

        // Validasi BIN chunk
        let bin_offset = 20 + json_chunk_len;
        let bin_chunk_len = u32::from_le_bytes(glb_bytes[bin_offset..bin_offset + 4].try_into().unwrap()) as usize;
        let bin_chunk_type = u32::from_le_bytes(glb_bytes[bin_offset + 4..bin_offset + 8].try_into().unwrap());
        assert_eq!(bin_chunk_type, 0x004E4942, "Chunk 1 harus 'BIN'");
        assert_eq!(bin_chunk_len % 4, 0, "BIN chunk harus 4-byte aligned");
        assert_eq!(bin_offset + 8 + bin_chunk_len, glb_bytes.len());
    }

    #[test]
    fn test_export_glb_multi_body_pbr_clearcoat() {
        let mesh1 = sample_triangle_mesh();
        let mesh2 = sample_triangle_mesh();
        let mat_plastic = Material::glossy_plastic(Some([0.9, 0.1, 0.1, 1.0]));
        let mat_metal = Material::anodized_aluminum(Some([0.8, 0.8, 0.8, 1.0]));

        let bodies = vec![
            ("Cover_Top", mat_plastic, &mesh1),
            ("Chassis_Base", mat_metal, &mesh2),
        ];

        let temp_path = std::env::temp_dir().join(format!("ducad_test_{}.glb", std::process::id()));
        write_glb(&bodies, &temp_path).unwrap();

        let bytes = std::fs::read(&temp_path).unwrap();
        let _ = std::fs::remove_file(&temp_path);

        assert!(!bytes.is_empty());
        let json_chunk_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let json_val: Value = serde_json::from_slice(&bytes[20..20 + json_chunk_len]).unwrap();

        assert_eq!(json_val["meshes"].as_array().unwrap().len(), 2);
        assert_eq!(json_val["nodes"].as_array().unwrap().len(), 2);
        assert_eq!(json_val["materials"].as_array().unwrap().len(), 2);

        // Pastikan ekstensi KHR_materials_clearcoat dicantumkan jika ada clearcoat
        let ext_used = json_val["extensionsUsed"].as_array().unwrap();
        assert!(ext_used.iter().any(|v| v == "KHR_materials_clearcoat"));
    }
}
