//! `KernelShape` backend truck (PLAN_MIGRASI_TRUCK 2.4): satu `Body`
//! SolidCraft (atau kosong) plus cache hasil turunan yang mahal.
//!
//! `Body` SolidCraft murah di-clone (`Arc`) dan tidak pernah dimutasi, jadi
//! salinan shape berbagi cache-nya juga.

use std::sync::{Arc, OnceLock};

use anyhow::{anyhow, bail, Context, Result};
use solidcraft_geom::Vec3;
use solidcraft_kernel::Body;

use super::bridge::t3;
use super::kernel;
use super::topology::Topology;
use crate::mesh::KernelMesh;

#[derive(Default)]
struct ShapeCache {
    topology: OnceLock<Topology>,
    /// Hasil `solidcraft_kernel::measure`; `None` bila gagal.
    measure: OnceLock<Option<Measured>>,
    valid: OnceLock<bool>,
}

/// Ukuran dari tesselasi halus konvergen (`solidcraft_kernel::measure`).
#[derive(Clone, Copy)]
struct Measured {
    volume: f64,
    area: f64,
    bbox: ([f64; 3], [f64; 3]),
}

/// Solid/shape B-rep. Field dalam sengaja privat — pemanggil di luar crate
/// ini hanya membangunnya lewat fungsi kernel dan membacanya lewat
/// `tessellate`/`volume`/…, tidak pernah menyentuh tipe SolidCraft.
pub struct KernelShape {
    body: Option<Body>,
    cache: Arc<ShapeCache>,
}

impl KernelShape {
    pub(crate) fn from_body(body: Body) -> Self {
        KernelShape {
            body: Some(body),
            cache: Arc::default(),
        }
    }

    pub(crate) fn body(&self) -> Option<&Body> {
        self.body.as_ref()
    }

    /// Body-nya, atau galat berkonteks bila shape kosong.
    pub(crate) fn require_body(&self, what: &str) -> Result<&Body> {
        match &self.body {
            Some(b) => Ok(b),
            None => bail!("{what}: shape kosong (tidak ada solid)"),
        }
    }

    /// Topologi logis (face/edge setara OCCT) + mesh tampilan, dihitung sekali.
    pub(crate) fn topology(&self) -> &Topology {
        self.cache.topology.get_or_init(|| match &self.body {
            Some(b) => Topology::build(b),
            None => Topology::empty(),
        })
    }

    fn measured(&self) -> Option<Measured> {
        *self.cache.measure.get_or_init(|| {
            let body = self.body.as_ref()?;
            match solidcraft_kernel::guard("measure", || solidcraft_kernel::measure(body)) {
                Ok(m) => Some(Measured {
                    volume: m.volume,
                    area: m.area,
                    bbox: ([m.bbox.min.x, m.bbox.min.y, m.bbox.min.z], [m.bbox.max.x, m.bbox.max.y, m.bbox.max.z]),
                }),
                Err(e) => {
                    log::warn!("measure truck gagal: {e}");
                    None
                }
            }
        })
    }

    /// Buat shape kosong — dipakai untuk mesh body murni (seperti impor STL).
    pub fn empty() -> Self {
        KernelShape {
            body: None,
            cache: Arc::default(),
        }
    }

    pub fn tessellate(&self) -> KernelMesh {
        self.topology().mesh.clone()
    }

    /// Apakah shape ini valid: shell tertutup dan berorientasi konsisten,
    /// semua face ter-mesh, volume positif (`Body::validity` SolidCraft).
    pub fn is_valid(&self) -> bool {
        *self.cache.valid.get_or_init(|| match &self.body {
            None => true,
            Some(b) => solidcraft_kernel::guard("validity", || Ok(b.validity()))
                .map(|issues| {
                    if !issues.is_empty() {
                        log::debug!("body truck tidak valid: {issues:?}");
                    }
                    issues.is_empty()
                })
                .unwrap_or(false),
        })
    }

    /// Luas permukaan total dalam mm², dari tesselasi halus konvergen
    /// (`solidcraft_kernel::measure`, galat relatif ±1e-4 — PLAN §3.1).
    pub fn surface_area(&self) -> f64 {
        self.measured().map(|m| m.area).unwrap_or(0.0)
    }

    /// Volume solid dalam mm³, dari tesselasi halus konvergen
    /// (`solidcraft_kernel::measure`, galat relatif ±1e-4 — PLAN §3.1).
    /// Selalu positif untuk solid valid.
    pub fn volume(&self) -> f64 {
        self.measured().map(|m| m.volume).unwrap_or(0.0)
    }

    /// Bounding box geometris `(min, max)` dalam mm, dari tesselasi halus
    /// `measure` (meleset ke dalam paling banyak ~5e-5 × diagonal body).
    /// Lebih akurat dari bbox mesh tampilan, yang vertexnya tidak selalu
    /// jatuh di titik ekstrem lingkaran. `None` untuk shape kosong.
    pub fn bounding_box(&self) -> Option<([f64; 3], [f64; 3])> {
        self.measured().map(|m| m.bbox)
    }

    pub fn write_stl(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let bytes = stl_bytes(&self.tessellate());
        std::fs::write(path, bytes).context("write_stl: gagal menulis berkas")
    }

    pub fn write_step(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let text = self.to_step_string()?;
        std::fs::write(path, text).context("write_step: gagal menulis berkas")
    }

    /// Baca shape B-rep dari file STEP (AP203/214/242, termasuk keluaran
    /// OCCT) lewat pembaca Part 21 SolidCraft. Beberapa solid digabung jadi
    /// satu body ber-banyak-shell (padanan compound OCCT).
    pub fn read_step(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let text = std::fs::read_to_string(path).context("read_step: gagal membaca STEP")?;
        Self::from_step_string(&text).context("read_step: gagal membaca STEP")
    }

    /// Baca shape dari file STL (biner/ASCII) sebagai body mesh.
    pub fn read_stl(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let bytes = std::fs::read(path).context("read_stl: gagal membaca STL")?;
        let (positions, triangles) = parse_stl(&bytes).context("read_stl: gagal membaca STL")?;
        let body = kernel("read_stl", || {
            solidcraft_kernel::mesh_body(&positions, &triangles)
        })?;
        Ok(Self::from_body(body))
    }

    /// Serialize B-rep ini jadi teks STEP AP214 (penulis STEP SolidCraft).
    pub fn to_step_string(&self) -> Result<String> {
        let body = self.require_body("to_step_string")?;
        kernel("to_step_string", || {
            solidcraft_kernel::step_export(&[body], "DUCAD")
        })
    }

    /// Serialize B-rep ini ke format biner native SolidCraft
    /// (`encode_native_body`). Padanan BREP biner OCCT: untuk penyimpanan
    /// internal, bukan pertukaran data.
    pub fn to_brep_bytes(&self) -> Result<Vec<u8>> {
        let body = self.require_body("to_brep_bytes")?;
        kernel("to_brep_bytes", || {
            solidcraft_kernel::encode_native_body(body)
        })
    }

    /// Kebalikan [`KernelShape::to_brep_bytes`].
    pub fn from_brep_bytes(bytes: &[u8]) -> Result<Self> {
        kernel("from_brep_bytes", || {
            solidcraft_kernel::decode_native_body(bytes)
        })
        .map(Self::from_body)
    }

    /// Identitas penulis cache B-rep native (field `brep.producer` di berkas
    /// `.ducad` v4). Cache dari backend lain dianggap *miss*.
    pub const NATIVE_CACHE_PRODUCER: &'static str = "ducad-truck/1";

    /// Encode B-rep ini untuk cache native non-otoritatif.
    pub fn to_native_cache(&self) -> Result<Vec<u8>> {
        self.to_brep_bytes()
    }

    /// Kebalikan [`KernelShape::to_native_cache`]. Pemanggil WAJIB sudah
    /// memeriksa produsen cache sama dengan [`Self::NATIVE_CACHE_PRODUCER`].
    pub fn from_native_cache(bytes: &[u8]) -> Result<Self> {
        Self::from_brep_bytes(bytes)
    }

    /// Kebalikan `to_step_string`. Face yang gagal dibaca dicatat di log
    /// (PLAN_MIGRASI_TRUCK F1.4: pembaca toleran).
    pub fn from_step_string(step: &str) -> Result<Self> {
        let import = kernel("from_step_string", || solidcraft_kernel::step_import(step))?;
        for w in &import.warnings {
            log::warn!("STEP: {w}");
        }
        let mut bodies: Vec<Body> = import.bodies.into_iter().map(|b| b.body).collect();
        if bodies.len() <= 1 {
            return bodies
                .pop()
                .map(Self::from_body)
                .ok_or_else(|| anyhow!("from_step_string: STEP tidak berisi solid"));
        }
        kernel("from_step_string", || {
            solidcraft_kernel::join_apart(&bodies)
        })?
        .map(Self::from_body)
        .ok_or_else(|| anyhow!("from_step_string: solid STEP tidak bisa digabung"))
    }
}

/// Kloning murah: `Body` SolidCraft immutable, jadi salinan berbagi data
/// (dan cache tesselasi/topologinya).
pub fn clone_shape(shape: &KernelShape) -> Result<KernelShape> {
    Ok(KernelShape {
        body: shape.body.clone(),
        cache: shape.cache.clone(),
    })
}

/// Transformasi afin baris-vektor (`p' = p·M`, baris 3 = translasi) —
/// konvensi `solidcraft_kernel::transform_matrix`.
pub(crate) fn apply_matrix(
    shape: &KernelShape,
    m: [[f64; 4]; 4],
    what: &str,
) -> Result<KernelShape> {
    match &shape.body {
        None => Ok(KernelShape::empty()),
        Some(b) => {
            kernel(what, || solidcraft_kernel::transform_matrix(b, m)).map(KernelShape::from_body)
        }
    }
}

/// Geser shape sejauh `(dx, dy, dz)` mm (DELTA terhadap posisi sekarang).
pub fn translate_shape(shape: &KernelShape, dx: f64, dy: f64, dz: f64) -> Result<KernelShape> {
    if dx.abs() < 1e-12 && dy.abs() < 1e-12 && dz.abs() < 1e-12 {
        return clone_shape(shape);
    }
    apply_matrix(
        shape,
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [dx, dy, dz, 1.0],
        ],
        "translate_shape",
    )
}

/// Putar shape mengelilingi sumbu yang melewati titik `pivot` dengan arah
/// `axis` sebesar `angle_rad` radian (tangan kanan).
pub fn rotate_shape(
    shape: &KernelShape,
    pivot: (f64, f64, f64),
    axis: (f64, f64, f64),
    angle_rad: f64,
) -> Result<KernelShape> {
    transform_shape(shape, (0.0, 0.0, 0.0), pivot, axis, angle_rad)
}

/// Scale shape secara UNIFORM mengelilingi `pivot`.
pub fn scale_shape(
    shape: &KernelShape,
    pivot: (f64, f64, f64),
    factor: f64,
) -> Result<KernelShape> {
    if !factor.is_finite() || factor.abs() < 1e-12 {
        bail!("faktor skala harus bilangan berhingga bukan nol (diberikan {factor})");
    }
    let t = |p: f64| p * (1.0 - factor);
    apply_matrix(
        shape,
        [
            [factor, 0.0, 0.0, 0.0],
            [0.0, factor, 0.0, 0.0],
            [0.0, 0.0, factor, 0.0],
            [t(pivot.0), t(pivot.1), t(pivot.2), 1.0],
        ],
        "scale_shape",
    )
}

/// Transformasi rigid-body: rotasi `angle_rad` mengelilingi sumbu `axis`
/// yang melewati `pivot`, LALU pergeseran `translation` (keduanya DELTA).
pub fn transform_shape(
    shape: &KernelShape,
    translation: (f64, f64, f64),
    pivot: (f64, f64, f64),
    axis: (f64, f64, f64),
    angle_rad: f64,
) -> Result<KernelShape> {
    let Some(body) = &shape.body else {
        return Ok(KernelShape::empty());
    };
    let angle = if angle_rad.abs() > 1e-6 {
        angle_rad
    } else {
        0.0
    };
    let moved =
        translation.0.abs() > 1e-6 || translation.1.abs() > 1e-6 || translation.2.abs() > 1e-6;
    let t = if moved { t3(translation) } else { Vec3::ZERO };
    if angle == 0.0 && !moved {
        return clone_shape(shape);
    }
    kernel("transform_shape", || {
        solidcraft_kernel::transform(body, t, t3(pivot), t3(axis), angle)
    })
    .map(KernelShape::from_body)
}

/// Gabungkan beberapa shape jadi SATU shape ber-banyak-solid (padanan
/// `TopoDS_Compound` OCCT): tiap solid tetap terpisah, tanpa boolean.
pub fn make_compound(shapes: &[&KernelShape]) -> Result<KernelShape> {
    if shapes.is_empty() {
        bail!("tidak ada shape untuk digabungkan menjadi compound");
    }
    if shapes.len() == 1 {
        return clone_shape(shapes[0]);
    }
    let mut bodies: Vec<Body> = shapes.iter().filter_map(|s| s.body.clone()).collect();
    if bodies.len() <= 1 {
        return Ok(bodies
            .pop()
            .map(KernelShape::from_body)
            .unwrap_or_else(KernelShape::empty));
    }
    kernel("make_compound", || solidcraft_kernel::join_apart(&bodies))?
        .map(KernelShape::from_body)
        .ok_or_else(|| anyhow!("make_compound: body mesh tidak bisa digabung jadi compound"))
}

/// Segmen garis tepi 3D (edge B-rep logis) untuk rendering "Shaded with
/// Visible Edges". Body tanpa edge B-rep (mesh STL) memakai tepi lipatan mesh.
pub fn extract_shape_edges(
    shape: &KernelShape,
    mesh: Option<&KernelMesh>,
) -> Vec<([f32; 3], [f32; 3])> {
    const MAX_EDGES: usize = 5000;
    let quantize = |p: [f32; 3]| -> (i64, i64, i64) {
        (
            (p[0] * 100.0).round() as i64,
            (p[1] * 100.0).round() as i64,
            (p[2] * 100.0).round() as i64,
        )
    };
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for edge in shape.topology().edges.iter().take(MAX_EDGES) {
        for w in edge.points.windows(2) {
            let a = [w[0].x as f32, w[0].y as f32, w[0].z as f32];
            let b = [w[1].x as f32, w[1].y as f32, w[1].z as f32];
            let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
            if d[0] * d[0] + d[1] * d[1] + d[2] * d[2] > 1e-6 {
                let (qa, qb) = (quantize(a), quantize(b));
                if seen.insert(if qa <= qb { (qa, qb) } else { (qb, qa) }) {
                    out.push((a, b));
                }
            }
        }
    }
    if out.is_empty() {
        if let Some(m) = mesh {
            for (p1, p2) in crate::hlr::extract_mesh_feature_edges(m) {
                out.push(([p1.x, p1.y, p1.z], [p2.x, p2.y, p2.z]));
            }
        }
    }
    out
}

/// STL biner dari mesh (normal per segitiga dari urutan vertex).
fn stl_bytes(mesh: &KernelMesh) -> Vec<u8> {
    let n = mesh.indices.len() / 3;
    let mut out = Vec::with_capacity(84 + 50 * n);
    let mut header = [0u8; 80];
    let tag = b"DUCAD STL (backend truck)";
    header[..tag.len()].copy_from_slice(tag);
    out.extend_from_slice(&header);
    out.extend_from_slice(&(n as u32).to_le_bytes());
    let p = |i: u32| mesh.positions.get(i as usize).copied().unwrap_or([0.0; 3]);
    for t in mesh.indices.chunks_exact(3) {
        let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let mut nrm = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let len = (nrm[0] * nrm[0] + nrm[1] * nrm[1] + nrm[2] * nrm[2]).sqrt();
        if len > 0.0 {
            nrm = nrm.map(|x| x / len);
        }
        for v in [nrm, a, b, c] {
            for x in v {
                out.extend_from_slice(&x.to_le_bytes());
            }
        }
        out.extend_from_slice(&[0, 0]);
    }
    out
}

/// Parse STL biner/ASCII jadi vertex ter-weld + segitiga.
fn parse_stl(bytes: &[u8]) -> Result<(Vec<Vec3>, Vec<[u32; 3]>)> {
    let mut raw: Vec<[f32; 3]> = Vec::new();
    let binary_len = bytes
        .get(80..84)
        .and_then(|b| <[u8; 4]>::try_from(b).ok())
        .map(|b| 84 + 50 * u32::from_le_bytes(b) as usize);
    if binary_len == Some(bytes.len()) {
        for tri in bytes.get(84..).unwrap_or(&[]).chunks_exact(50) {
            for k in 0..3 {
                let at = 12 + k * 12;
                let f = |o: usize| {
                    tri.get(at + o..at + o + 4)
                        .and_then(|b| <[u8; 4]>::try_from(b).ok())
                        .map(f32::from_le_bytes)
                        .unwrap_or(0.0)
                };
                raw.push([f(0), f(4), f(8)]);
            }
        }
    } else {
        let text = std::str::from_utf8(bytes).context("STL bukan biner yang sah maupun ASCII")?;
        for line in text.lines() {
            let mut it = line.split_whitespace();
            if it.next() == Some("vertex") {
                let v: Vec<f32> = it.filter_map(|s| s.parse().ok()).collect();
                if v.len() != 3 {
                    bail!("baris vertex STL rusak: {line}");
                }
                raw.push([v[0], v[1], v[2]]);
            }
        }
    }
    if raw.len() < 3 || !raw.len().is_multiple_of(3) {
        bail!("STL tidak berisi segitiga");
    }
    let mut index: std::collections::HashMap<[u32; 3], u32> = std::collections::HashMap::new();
    let mut positions = Vec::new();
    let mut triangles = Vec::with_capacity(raw.len() / 3);
    for t in raw.chunks_exact(3) {
        let mut tri = [0u32; 3];
        for (k, p) in t.iter().enumerate() {
            tri[k] = *index.entry(p.map(f32::to_bits)).or_insert_with(|| {
                positions.push(Vec3::new(p[0] as f64, p[1] as f64, p[2] as f64));
                (positions.len() - 1) as u32
            });
        }
        if tri[0] != tri[1] && tri[1] != tri[2] && tri[0] != tri[2] {
            triangles.push(tri);
        }
    }
    Ok((positions, triangles))
}
