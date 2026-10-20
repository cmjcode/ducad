//! Format mesh pertukaran (Paritas 3.9–3.10): ekspor/impor 3MF, impor OBJ
//! dan glTF/GLB. Semua impor menghasilkan mesh (bukan B-rep) dalam mm.
//!
//! Parser ditulis ringkas tanpa pustaka XML/glTF: cukup untuk berkas dari
//! slicer/CAD umum (3MF core, OBJ `v`/`f`, glTF 2.0 dengan buffer GLB atau
//! `data:` base64). Input rusak menghasilkan `Err`, tidak pernah panic.

use std::io::{Read, Write};
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use ducad_kernel::KernelMesh;

/// Batas segitiga per berkas impor (melindungi memori).
pub const MAX_IMPORT_TRIANGLES: usize = 20_000_000;

/// Satu body mesh hasil impor.
#[derive(Debug, Clone)]
pub struct ImportedMesh {
    pub name: String,
    pub mesh: KernelMesh,
}

/// Mesh dari daftar posisi + indeks segitiga; normal per-vertex dihitung.
pub fn mesh_from_triangles(positions: Vec<[f32; 3]>, indices: Vec<u32>) -> KernelMesh {
    let mut normals = vec![[0.0f32; 3]; positions.len()];
    for t in indices.chunks_exact(3) {
        let (Some(a), Some(b), Some(c)) = (
            positions.get(t[0] as usize),
            positions.get(t[1] as usize),
            positions.get(t[2] as usize),
        ) else {
            continue;
        };
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        for &i in t {
            if let Some(slot) = normals.get_mut(i as usize) {
                for k in 0..3 {
                    slot[k] += n[k];
                }
            }
        }
    }
    for n in &mut normals {
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if l > 0.0 {
            for c in n.iter_mut() {
                *c /= l;
            }
        }
    }
    KernelMesh {
        positions,
        normals,
        indices,
        face_ranges: Vec::new(),
    }
}

fn check_indices(n_pos: usize, indices: &[u32]) -> Result<()> {
    if indices.len() / 3 > MAX_IMPORT_TRIANGLES {
        bail!(
            "terlalu banyak segitiga ({}; maks {MAX_IMPORT_TRIANGLES})",
            indices.len() / 3
        );
    }
    if let Some(bad) = indices.iter().find(|&&i| i as usize >= n_pos) {
        bail!("indeks vertex {bad} di luar jangkauan ({n_pos} vertex)");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 3MF
// ---------------------------------------------------------------------------

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
 <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
 <Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml"/>
</Types>"#;

const RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
 <Relationship Target="/3D/3dmodel.model" Id="rel0" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/>
</Relationships>"#;

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Isi `3D/3dmodel.model` (unit mm) untuk body bernama + warna opsional.
pub fn threemf_model_xml(bodies: &[(&str, &KernelMesh, Option<[f32; 3]>)]) -> String {
    let mut x = String::with_capacity(1024);
    x.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    x.push_str("<model unit=\"millimeter\" xml:lang=\"en-US\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n");
    x.push_str(" <metadata name=\"Application\">DUCAD</metadata>\n <resources>\n");
    let colored: Vec<(usize, [f32; 3])> = bodies
        .iter()
        .enumerate()
        .filter_map(|(i, b)| b.2.map(|c| (i, c)))
        .collect();
    if !colored.is_empty() {
        x.push_str("  <basematerials id=\"1\">\n");
        for (i, c) in &colored {
            let to = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            x.push_str(&format!(
                "   <base name=\"{}\" displaycolor=\"#{:02X}{:02X}{:02X}\"/>\n",
                xml_escape(bodies[*i].0),
                to(c[0]),
                to(c[1]),
                to(c[2])
            ));
        }
        x.push_str("  </basematerials>\n");
    }
    for (k, (name, mesh, _)) in bodies.iter().enumerate() {
        let id = k + 2;
        let mat = colored
            .iter()
            .position(|(i, _)| *i == k)
            .map(|p| format!(" pid=\"1\" pindex=\"{p}\""))
            .unwrap_or_default();
        x.push_str(&format!(
            "  <object id=\"{id}\" type=\"model\" name=\"{}\"{mat}>\n   <mesh>\n    <vertices>\n",
            xml_escape(name)
        ));
        for p in &mesh.positions {
            x.push_str(&format!(
                "     <vertex x=\"{}\" y=\"{}\" z=\"{}\"/>\n",
                p[0], p[1], p[2]
            ));
        }
        x.push_str("    </vertices>\n    <triangles>\n");
        for t in mesh.indices.chunks_exact(3) {
            x.push_str(&format!(
                "     <triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>\n",
                t[0], t[1], t[2]
            ));
        }
        x.push_str("    </triangles>\n   </mesh>\n  </object>\n");
    }
    x.push_str(" </resources>\n <build>\n");
    for k in 0..bodies.len() {
        x.push_str(&format!("  <item objectid=\"{}\"/>\n", k + 2));
    }
    x.push_str(" </build>\n</model>\n");
    x
}

/// Berkas 3MF (zip) dalam memori.
pub fn threemf_bytes(bodies: &[(&str, &KernelMesh, Option<[f32; 3]>)]) -> Result<Vec<u8>> {
    if bodies.is_empty() {
        bail!("tidak ada body untuk diekspor ke 3MF");
    }
    let mut cur = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut cur);
        let opt = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, text) in [
            ("[Content_Types].xml", CONTENT_TYPES.to_string()),
            ("_rels/.rels", RELS.to_string()),
            ("3D/3dmodel.model", threemf_model_xml(bodies)),
        ] {
            zip.start_file(name, opt).context("3MF: gagal menulis zip")?;
            zip.write_all(text.as_bytes())
                .context("3MF: gagal menulis zip")?;
        }
        zip.finish().context("3MF: gagal menutup zip")?;
    }
    Ok(cur.into_inner())
}

/// Tulis berkas `.3mf`.
pub fn write_3mf(
    bodies: &[(&str, &KernelMesh, Option<[f32; 3]>)],
    path: impl AsRef<Path>,
) -> Result<()> {
    let bytes = threemf_bytes(bodies)?;
    std::fs::write(path, bytes).context("3MF: gagal menulis berkas")
}

/// Nilai atribut `key="…"` dalam satu tag.
fn attr<'a>(tag: &'a str, key: &str) -> Option<&'a str> {
    let mut from = 0usize;
    loop {
        let i = tag[from..].find(key)? + from;
        let before_ok = i == 0 || tag.as_bytes()[i - 1].is_ascii_whitespace();
        let after = tag[i + key.len()..].trim_start();
        if before_ok {
            if let Some(v) = after.strip_prefix('=') {
                let v = v.trim_start();
                let q = v.chars().next()?;
                if q == '"' || q == '\'' {
                    let body = &v[1..];
                    let end = body.find(q)?;
                    return Some(&body[..end]);
                }
                return None;
            }
        }
        from = i + key.len();
    }
}

/// Semua tag `<name …>` (nama lokal, abaikan prefiks namespace).
fn tags<'a>(xml: &'a str, name: &'a str) -> impl Iterator<Item = &'a str> + 'a {
    let mut pos = 0usize;
    std::iter::from_fn(move || loop {
        let i = xml.get(pos..)?.find('<')? + pos;
        let j = xml.get(i..)?.find('>')? + i;
        pos = j + 1;
        let tag = &xml[i + 1..j];
        let tname = tag
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or("");
        let local = tname.rsplit(':').next().unwrap_or(tname);
        if local == name {
            return Some(tag);
        }
    })
}

fn parse_f(v: Option<&str>, what: &str) -> Result<f32> {
    let x: f32 = v
        .ok_or_else(|| anyhow!("3MF: atribut {what} hilang"))?
        .trim()
        .parse()
        .map_err(|_| anyhow!("3MF: angka {what} tidak valid"))?;
    if !x.is_finite() {
        bail!("3MF: angka {what} tidak hingga");
    }
    Ok(x)
}

/// Parse isi `3dmodel.model`: tiap `<object>` bermesh jadi satu body;
/// `transform` pada item build diterapkan.
pub fn parse_3mf_model(xml: &str) -> Result<Vec<ImportedMesh>> {
    let scale = match tags(xml, "model").next().and_then(|t| attr(t, "unit")) {
        Some("micron") => 0.001,
        Some("centimeter") => 10.0,
        Some("inch") => 25.4,
        Some("foot") => 304.8,
        Some("meter") => 1000.0,
        _ => 1.0,
    };
    let mut objects: Vec<(String, String, KernelMesh)> = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<object") {
        let obj = &rest[start..];
        let head_end = obj
            .find('>')
            .ok_or_else(|| anyhow!("3MF: tag object rusak"))?;
        let head = &obj[..head_end];
        let close = obj.find("</object>").unwrap_or(obj.len());
        let body = &obj[..close];
        let id = attr(head, "id").unwrap_or("").to_string();
        let name = xml_unescape(attr(head, "name").unwrap_or(""));
        let mut positions = Vec::new();
        for t in tags(body, "vertex") {
            positions.push([
                parse_f(attr(t, "x"), "x")? * scale,
                parse_f(attr(t, "y"), "y")? * scale,
                parse_f(attr(t, "z"), "z")? * scale,
            ]);
        }
        let mut indices = Vec::new();
        for t in tags(body, "triangle") {
            for k in ["v1", "v2", "v3"] {
                let v: u32 = attr(t, k)
                    .and_then(|s| s.trim().parse().ok())
                    .ok_or_else(|| anyhow!("3MF: indeks {k} tidak valid"))?;
                indices.push(v);
            }
        }
        if !indices.is_empty() {
            check_indices(positions.len(), &indices)?;
            objects.push((id, name, mesh_from_triangles(positions, indices)));
        }
        if close >= obj.len() {
            break;
        }
        rest = &obj[close..];
    }
    if objects.is_empty() {
        bail!("3MF: tidak ada mesh");
    }
    let items: Vec<&str> = tags(xml, "item").collect();
    let mut out = Vec::new();
    if items.is_empty() {
        for (_, name, mesh) in objects {
            out.push(ImportedMesh { name, mesh });
        }
    } else {
        for (k, it) in items.iter().enumerate() {
            let oid = attr(it, "objectid").unwrap_or("");
            let Some((_, name, mesh)) = objects.iter().find(|(id, _, _)| id == oid) else {
                continue;
            };
            let mut mesh = mesh.clone();
            if let Some(tf) = attr(it, "transform") {
                let m: Vec<f32> = tf
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();
                if m.len() == 12 && m.iter().all(|v| v.is_finite()) {
                    for p in &mut mesh.positions {
                        let [x, y, z] = *p;
                        *p = [
                            x * m[0] + y * m[3] + z * m[6] + m[9] * scale,
                            x * m[1] + y * m[4] + z * m[7] + m[10] * scale,
                            x * m[2] + y * m[5] + z * m[8] + m[11] * scale,
                        ];
                    }
                    mesh = mesh_from_triangles(mesh.positions, mesh.indices);
                }
            }
            let name = if name.is_empty() {
                format!("3MF {}", k + 1)
            } else {
                name.clone()
            };
            out.push(ImportedMesh { name, mesh });
        }
    }
    if out.is_empty() {
        bail!("3MF: item build tidak merujuk mesh yang ada");
    }
    Ok(out)
}

/// Baca 3MF dari byte (zip).
pub fn read_3mf_bytes(bytes: &[u8]) -> Result<Vec<ImportedMesh>> {
    let mut zip =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).context("3MF: bukan berkas zip")?;
    let name = (0..zip.len())
        .filter_map(|i| zip.by_index(i).ok().map(|f| f.name().to_string()))
        .find(|n| n.to_ascii_lowercase().ends_with(".model"))
        .ok_or_else(|| anyhow!("3MF: tidak ada berkas .model"))?;
    let f = zip.by_name(&name).context("3MF: gagal membuka model")?;
    let mut xml = String::new();
    f.take(2_000_000_000)
        .read_to_string(&mut xml)
        .context("3MF: model bukan UTF-8")?;
    parse_3mf_model(&xml)
}

// ---------------------------------------------------------------------------
// OBJ
// ---------------------------------------------------------------------------

/// Parse OBJ: `v x y z` dan `f a b c …` (poligon di-fan; indeks negatif
/// relatif). Tiap `o`/`g` baru memulai body baru.
pub fn parse_obj(text: &str) -> Result<Vec<ImportedMesh>> {
    let mut verts: Vec<[f32; 3]> = Vec::new();
    let mut groups: Vec<(String, Vec<u32>)> = vec![("OBJ".into(), Vec::new())];
    for (ln, line) in text.lines().enumerate() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("v") => {
                let c: Vec<f32> = it.take(3).filter_map(|s| s.parse().ok()).collect();
                if c.len() != 3 || c.iter().any(|v| !v.is_finite()) {
                    bail!("OBJ baris {}: vertex tidak valid", ln + 1);
                }
                verts.push([c[0], c[1], c[2]]);
            }
            Some("o") | Some("g") => {
                let name = it.collect::<Vec<_>>().join(" ");
                if groups.last().is_some_and(|g| g.1.is_empty()) {
                    groups.pop();
                }
                groups.push((if name.is_empty() { "OBJ".into() } else { name }, Vec::new()));
            }
            Some("f") => {
                let mut idx = Vec::new();
                for tok in it {
                    let first = tok.split('/').next().unwrap_or("");
                    let v: i64 = first.parse().map_err(|_| {
                        anyhow!("OBJ baris {}: indeks face tidak valid", ln + 1)
                    })?;
                    let i = if v < 0 { verts.len() as i64 + v } else { v - 1 };
                    if i < 0 || i as usize >= verts.len() {
                        bail!("OBJ baris {}: indeks face di luar jangkauan", ln + 1);
                    }
                    idx.push(i as u32);
                }
                if idx.len() < 3 {
                    bail!("OBJ baris {}: face butuh ≥ 3 vertex", ln + 1);
                }
                if let Some(g) = groups.last_mut() {
                    for k in 1..idx.len() - 1 {
                        g.1.extend([idx[0], idx[k], idx[k + 1]]);
                    }
                }
            }
            _ => {}
        }
    }
    let out: Vec<ImportedMesh> = groups
        .into_iter()
        .filter(|g| !g.1.is_empty())
        .map(|(name, idx)| compact(name, &verts, &idx))
        .collect::<Result<_>>()?;
    if out.is_empty() {
        bail!("OBJ: tidak ada face");
    }
    Ok(out)
}

/// Mesh hanya dengan vertex yang dipakai `idx`.
fn compact(name: String, verts: &[[f32; 3]], idx: &[u32]) -> Result<ImportedMesh> {
    let mut map = std::collections::HashMap::new();
    let mut positions = Vec::new();
    let mut indices = Vec::with_capacity(idx.len());
    for &i in idx {
        let n = *map.entry(i).or_insert_with(|| {
            positions.push(verts.get(i as usize).copied().unwrap_or([0.0; 3]));
            positions.len() as u32 - 1
        });
        indices.push(n);
    }
    check_indices(positions.len(), &indices)?;
    Ok(ImportedMesh {
        name,
        mesh: mesh_from_triangles(positions, indices),
    })
}

// ---------------------------------------------------------------------------
// glTF / GLB
// ---------------------------------------------------------------------------

/// Matriks 4×4 kolom-mayor (konvensi glTF).
type Mat4 = [f32; 16];

fn mat_mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut o = [0.0; 16];
    for c in 0..4 {
        for r in 0..4 {
            o[c * 4 + r] = (0..4).map(|k| a[k * 4 + r] * b[c * 4 + k]).sum();
        }
    }
    o
}

fn node_matrix(n: &serde_json::Value) -> Mat4 {
    if let Some(m) = n["matrix"].as_array().filter(|m| m.len() == 16) {
        let mut o = [0.0; 16];
        for (k, v) in m.iter().enumerate() {
            o[k] = v.as_f64().unwrap_or(0.0) as f32;
        }
        return o;
    }
    let f = |v: &serde_json::Value, i: usize, d: f32| {
        v.get(i)
            .and_then(|x| x.as_f64())
            .map(|x| x as f32)
            .unwrap_or(d)
    };
    let (t, r, s) = (&n["translation"], &n["rotation"], &n["scale"]);
    let (qx, qy, qz, qw) = (f(r, 0, 0.0), f(r, 1, 0.0), f(r, 2, 0.0), f(r, 3, 1.0));
    let (sx, sy, sz) = (f(s, 0, 1.0), f(s, 1, 1.0), f(s, 2, 1.0));
    [
        (1.0 - 2.0 * (qy * qy + qz * qz)) * sx,
        2.0 * (qx * qy + qz * qw) * sx,
        2.0 * (qx * qz - qy * qw) * sx,
        0.0,
        2.0 * (qx * qy - qz * qw) * sy,
        (1.0 - 2.0 * (qx * qx + qz * qz)) * sy,
        2.0 * (qy * qz + qx * qw) * sy,
        0.0,
        2.0 * (qx * qz + qy * qw) * sz,
        2.0 * (qy * qz - qx * qw) * sz,
        (1.0 - 2.0 * (qx * qx + qy * qy)) * sz,
        0.0,
        f(t, 0, 0.0),
        f(t, 1, 0.0),
        f(t, 2, 0.0),
        1.0,
    ]
}

struct Gltf {
    json: serde_json::Value,
    buffers: Vec<Vec<u8>>,
}

/// Isi accessor (sudah dibuang stride-nya): (byte, komponen, componentType, count).
type AccessorData = (Vec<u8>, usize, u64, usize);

impl Gltf {
    fn accessor_bytes(&self, idx: usize) -> Result<AccessorData> {
        let a = &self.json["accessors"][idx];
        let count = a["count"]
            .as_u64()
            .ok_or_else(|| anyhow!("glTF: accessor tanpa count"))? as usize;
        let ctype = a["componentType"].as_u64().unwrap_or(0);
        let comps = match a["type"].as_str() {
            Some("SCALAR") => 1,
            Some("VEC2") => 2,
            Some("VEC3") => 3,
            Some("VEC4") => 4,
            _ => bail!("glTF: tipe accessor tidak didukung"),
        };
        let csize = match ctype {
            5120 | 5121 => 1,
            5122 | 5123 => 2,
            5125 | 5126 => 4,
            _ => bail!("glTF: componentType {ctype} tidak didukung"),
        };
        if count > MAX_IMPORT_TRIANGLES * 3 {
            bail!("glTF: accessor terlalu besar");
        }
        let bv_i = a["bufferView"]
            .as_u64()
            .ok_or_else(|| anyhow!("glTF: accessor tanpa bufferView tidak didukung"))?
            as usize;
        let bv = &self.json["bufferViews"][bv_i];
        let buf = self
            .buffers
            .get(bv["buffer"].as_u64().unwrap_or(0) as usize)
            .ok_or_else(|| anyhow!("glTF: buffer hilang"))?;
        let off = bv["byteOffset"].as_u64().unwrap_or(0) as usize
            + a["byteOffset"].as_u64().unwrap_or(0) as usize;
        let elem = comps * csize;
        let stride = bv["byteStride"]
            .as_u64()
            .map(|s| s as usize)
            .unwrap_or(elem)
            .max(elem);
        let mut out = Vec::with_capacity(count * elem);
        for i in 0..count {
            let s = off + i * stride;
            let chunk = buf
                .get(s..s + elem)
                .ok_or_else(|| anyhow!("glTF: accessor melewati buffer"))?;
            out.extend_from_slice(chunk);
        }
        Ok((out, comps, ctype, count))
    }

    fn positions(&self, idx: usize) -> Result<Vec<[f32; 3]>> {
        let (b, comps, ctype, count) = self.accessor_bytes(idx)?;
        if comps != 3 || ctype != 5126 {
            bail!("glTF: POSITION harus float VEC3");
        }
        let f = |s: usize| f32::from_le_bytes([b[s], b[s + 1], b[s + 2], b[s + 3]]);
        Ok((0..count)
            .map(|i| [f(i * 12), f(i * 12 + 4), f(i * 12 + 8)])
            .collect())
    }

    fn indices(&self, idx: usize) -> Result<Vec<u32>> {
        let (b, comps, ctype, count) = self.accessor_bytes(idx)?;
        if comps != 1 {
            bail!("glTF: indeks harus SCALAR");
        }
        Ok((0..count)
            .map(|i| match ctype {
                5120 | 5121 => b[i] as u32,
                5122 | 5123 => u16::from_le_bytes([b[i * 2], b[i * 2 + 1]]) as u32,
                _ => u32::from_le_bytes([b[i * 4], b[i * 4 + 1], b[i * 4 + 2], b[i * 4 + 3]]),
            })
            .collect())
    }
}

fn decode_data_uri(uri: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    let data = uri.split_once(";base64,").map(|(_, d)| d).ok_or_else(|| {
        anyhow!("glTF: hanya buffer data:…;base64 yang didukung (buffer eksternal tidak)")
    })?;
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|_| anyhow!("glTF: base64 buffer rusak"))
}

/// Baca glTF (`.gltf` JSON dengan buffer `data:`) atau GLB biner. Satuan
/// glTF meter → mm (×1000), sumbu +Y-atas glTF diputar ke +Z-atas DUCAD.
pub fn read_gltf_bytes(bytes: &[u8]) -> Result<Vec<ImportedMesh>> {
    let (json, bin) = if bytes.len() >= 12 && &bytes[0..4] == b"glTF" {
        let mut pos = 12usize;
        let (mut json, mut bin) = (None, None);
        while pos + 8 <= bytes.len() {
            let len = u32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]])
                as usize;
            let kind = &bytes[pos + 4..pos + 8];
            let data = bytes
                .get(pos + 8..(pos + 8).saturating_add(len))
                .ok_or_else(|| anyhow!("GLB: chunk terpotong"))?;
            match kind {
                b"JSON" => {
                    json = Some(
                        serde_json::from_slice::<serde_json::Value>(data)
                            .context("GLB: JSON rusak")?,
                    )
                }
                b"BIN\0" => bin = Some(data.to_vec()),
                _ => {}
            }
            pos += 8 + len;
        }
        (json.ok_or_else(|| anyhow!("GLB: tanpa chunk JSON"))?, bin)
    } else {
        (
            serde_json::from_slice::<serde_json::Value>(bytes).context("glTF: JSON rusak")?,
            None,
        )
    };
    let mut buffers = Vec::new();
    for (i, b) in json["buffers"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .enumerate()
    {
        match b["uri"].as_str() {
            Some(uri) => buffers.push(decode_data_uri(uri)?),
            None if i == 0 => {
                buffers.push(bin.clone().ok_or_else(|| anyhow!("GLB: chunk BIN hilang"))?)
            }
            None => bail!("glTF: buffer tanpa uri"),
        }
    }
    let g = Gltf { json, buffers };
    // Y-atas (glTF) → Z-atas DUCAD, meter → mm: (x, y, z) → (x, −z, y)·1000.
    let root: Mat4 = [
        1000., 0., 0., 0., 0., 0., 1000., 0., 0., -1000., 0., 0., 0., 0., 0., 1.,
    ];
    let scene = g.json["scene"].as_u64().unwrap_or(0) as usize;
    let roots: Vec<usize> = g.json["scenes"][scene]["nodes"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_u64().map(|x| x as usize)).collect())
        .unwrap_or_else(|| (0..g.json["nodes"].as_array().map(|a| a.len()).unwrap_or(0)).collect());
    let mut out = Vec::new();
    let mut stack: Vec<(usize, Mat4, usize)> = roots.into_iter().map(|n| (n, root, 0)).collect();
    while let Some((ni, parent, depth)) = stack.pop() {
        if depth > 64 {
            bail!("glTF: hierarki node terlalu dalam/berputar");
        }
        let node = &g.json["nodes"][ni];
        let m = mat_mul(&parent, &node_matrix(node));
        if let Some(mi) = node["mesh"].as_u64() {
            let mesh = &g.json["meshes"][mi as usize];
            let name = node["name"]
                .as_str()
                .or(mesh["name"].as_str())
                .unwrap_or("glTF")
                .to_string();
            let mut positions = Vec::new();
            let mut indices = Vec::new();
            for prim in mesh["primitives"].as_array().cloned().unwrap_or_default() {
                if prim["mode"].as_u64().unwrap_or(4) != 4 {
                    continue; // hanya TRIANGLES
                }
                let Some(pa) = prim["attributes"]["POSITION"].as_u64() else {
                    continue;
                };
                let base = positions.len() as u32;
                let pts = g.positions(pa as usize)?;
                let n = pts.len() as u32;
                for p in pts {
                    positions.push([
                        m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12],
                        m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13],
                        m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14],
                    ]);
                }
                match prim["indices"].as_u64() {
                    Some(ii) => indices.extend(g.indices(ii as usize)?.into_iter().map(|i| i + base)),
                    None => indices.extend(base..base + n - n % 3),
                }
            }
            if !indices.is_empty() {
                check_indices(positions.len(), &indices)?;
                out.push(ImportedMesh {
                    name,
                    mesh: mesh_from_triangles(positions, indices),
                });
            }
        }
        for c in node["children"].as_array().cloned().unwrap_or_default() {
            if let Some(c) = c.as_u64() {
                stack.push((c as usize, m, depth + 1));
            }
        }
    }
    if out.is_empty() {
        bail!("glTF: tidak ada mesh segitiga");
    }
    Ok(out)
}

/// Impor mesh berdasarkan ekstensi berkas (`3mf`, `obj`, `gltf`, `glb`, `stl`).
pub fn read_mesh_file(path: impl AsRef<Path>) -> Result<Vec<ImportedMesh>> {
    let path = path.as_ref();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let bytes =
        std::fs::read(path).with_context(|| format!("gagal membaca {}", path.display()))?;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Mesh")
        .to_string();
    match ext.as_str() {
        "3mf" => read_3mf_bytes(&bytes),
        "obj" => parse_obj(&String::from_utf8_lossy(&bytes)),
        "gltf" | "glb" => read_gltf_bytes(&bytes),
        "stl" => Ok(vec![ImportedMesh {
            name: stem,
            mesh: crate::mesh_export::read_stl_from_bytes(&bytes)?,
        }]),
        other => bail!("format mesh '.{other}' tidak didukung (3mf, obj, gltf, glb, stl)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tetra() -> KernelMesh {
        mesh_from_triangles(
            vec![[0., 0., 0.], [10., 0., 0.], [0., 10., 0.], [0., 0., 10.]],
            vec![0, 2, 1, 0, 1, 3, 0, 3, 2, 1, 2, 3],
        )
    }

    #[test]
    fn threemf_round_trip() {
        let m = tetra();
        let bytes = threemf_bytes(&[("tetra & co", &m, Some([1.0, 0.0, 0.0]))]).unwrap();
        let back = read_3mf_bytes(&bytes).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].name, "tetra & co");
        assert_eq!(back[0].mesh.positions, m.positions);
        assert_eq!(back[0].mesh.indices, m.indices);
        assert!(threemf_model_xml(&[("a", &m, Some([1.0, 0.0, 0.0]))]).contains("#FF0000"));
    }

    #[test]
    fn threemf_transform_and_units() {
        let xml = r#"<model unit="centimeter"><resources><object id="1"><mesh><vertices>
            <vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/>
            </vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object></resources>
            <build><item objectid="1" transform="1 0 0 0 1 0 0 0 1 5 0 0"/></build></model>"#;
        let m = parse_3mf_model(xml).unwrap();
        assert_eq!(m[0].mesh.positions[1], [60.0, 0.0, 0.0]);
    }

    #[test]
    fn obj_quads_negative_indices_and_groups() {
        let text = "o a\nv 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3 4\no b\nv 0 0 1\nv 1 0 1\nv 0 1 1\nf -3/1/1 -2 -1\n";
        let m = parse_obj(text).unwrap();
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].mesh.indices.len(), 6);
        assert_eq!(m[1].name, "b");
        assert_eq!(m[1].mesh.positions.len(), 3);
    }

    #[test]
    fn gltf_embedded_buffer_scaled_and_z_up() {
        use base64::Engine;
        let mut buf = Vec::new();
        for v in [0.0f32, 0., 0., 1., 0., 0., 0., 1., 0.] {
            buf.extend(v.to_le_bytes());
        }
        let uri = format!(
            "data:application/octet-stream;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&buf)
        );
        let json = serde_json::json!({
            "asset": {"version": "2.0"},
            "buffers": [{"byteLength": 36, "uri": uri}],
            "bufferViews": [{"buffer": 0, "byteLength": 36}],
            "accessors": [{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3"}],
            "meshes": [{"primitives": [{"attributes": {"POSITION": 0}}]}],
            "nodes": [{"mesh": 0, "name": "tri"}],
            "scenes": [{"nodes": [0]}]
        });
        let m = read_gltf_bytes(json.to_string().as_bytes()).unwrap();
        assert_eq!(m[0].name, "tri");
        assert_eq!(m[0].mesh.positions[1], [1000.0, 0.0, 0.0]);
        // +Y glTF → +Z DUCAD.
        assert_eq!(m[0].mesh.positions[2], [0.0, 0.0, 1000.0]);
    }

    #[test]
    fn ducad_glb_export_reimports() {
        let m = tetra();
        let bytes = crate::glb::export_glb_bytes(
            &[("t", ducad_core::Material::default(), &m)],
            &Default::default(),
        )
        .unwrap();
        let back = read_gltf_bytes(&bytes).unwrap();
        assert_eq!(back[0].mesh.indices.len(), m.indices.len());
    }

    #[test]
    fn hostile_inputs_fail_cleanly() {
        assert!(read_3mf_bytes(b"not a zip").is_err());
        assert!(parse_3mf_model("<model></model>").is_err());
        assert!(parse_3mf_model(
            r#"<object id="1"><vertex x="1" y="2" z="0"/><triangle v1="0" v2="0" v3="9"/></object>"#
        )
        .is_err());
        assert!(parse_obj("v 0 0 0\nf 1 2 3\n").is_err());
        assert!(parse_obj("v nan 0 0\n").is_err());
        assert!(read_gltf_bytes(b"{}").is_err());
        assert!(read_gltf_bytes(b"glTF\x02\0\0\0\x20\0\0\0\xff\xff\0\0JSON").is_err());
    }
}
