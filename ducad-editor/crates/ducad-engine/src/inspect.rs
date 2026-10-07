//! Laporan terstruktur keadaan sesi (P2.2). Semua angka dibulatkan ke 4
//! desimal — hemat token dan stabil untuk diff.

use std::collections::BTreeMap;

use ducad_kernel::{EdgeInfo, FaceInfo};
use ducad_sketch::{PlaneRef, SketchSet};
use serde::Serialize;

use crate::error::OpResult;
use crate::model::{BodyGeometry, ModelDoc};
use crate::ops::Params;
use crate::session::{Session, SessionMeta};

/// Batas default jumlah face/tepi per body pada `topology`.
pub const DEFAULT_TOPOLOGY_LIMIT: usize = 200;

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    /// Selalu `"mm"`.
    pub unit: &'static str,
    pub bodies: Vec<BodyReport>,
    pub sketches: Vec<SketchReport>,
    pub params: Params,
    pub oplog_len: usize,
    /// Mis. `"oplog_stale"`.
    pub warnings: Vec<String>,
    /// Properti massa gabungan semua body yang densitasnya diketahui
    /// (hanya bila ada ≥ 2 body).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assembly: Option<AssemblyMassReport>,
    /// Konfigurasi varian aktif; kosong = "Default". `params` di atas sudah
    /// memuat penimpaannya.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_configuration: Option<String>,
    /// Konfigurasi varian selain "Default" (P19).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub configurations: Vec<ConfigurationReport>,
    /// Lembar gambar tersimpan (P21.6).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drawings: Vec<DrawingReport>,
}

/// Satu lembar gambar tersimpan di ringkasan.
#[derive(Debug, Clone, Serialize)]
pub struct DrawingReport {
    #[serde(flatten)]
    pub summary: crate::drawing_auto::DrawingSummary,
    /// Geometri berubah sejak lembar ini terakhir dirender di sesi ini.
    pub stale: bool,
}

/// Satu konfigurasi varian di ringkasan.
#[derive(Debug, Clone, Serialize)]
pub struct ConfigurationReport {
    pub name: String,
    pub active: bool,
    pub params: Params,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suppressed_ops: Vec<String>,
}

impl Summary {
    /// Nama body, terurut.
    pub fn body_names(&self) -> Vec<String> {
        self.bodies.iter().map(|b| b.name.clone()).collect()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BodyReport {
    pub name: String,
    pub uuid: String,
    pub visible: bool,
    pub valid: bool,
    pub volume: f64,
    pub area: f64,
    pub bbox: [[f64; 3]; 2],
    pub size: [f64; 3],
    pub centroid: [f64; 3],
    pub faces: usize,
    pub edges: usize,
    /// Massa (gram) = volume × densitas preset material; kosong bila
    /// densitas preset tidak diketahui.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mass_g: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<MaterialReport>,
    /// Pusat massa eksak dari B-rep (mm). Kosong untuk body tanpa volume.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub center_of_mass: Option<[f64; 3]>,
    /// Tensor inersia terhadap pusat massa, sumbu global (g·mm²).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inertia_com: Option<[[f64; 3]; 3]>,
    /// Tensor inersia terhadap origin (g·mm²).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inertia_origin: Option<[[f64; 3]; 3]>,
    /// Momen utama terurut naik (g·mm²).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal_moments: Option<[f64; 3]>,
    /// Sumbu utama (vektor satuan), urut sama dengan `principal_moments`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal_axes: Option<[[f64; 3]; 3]>,
    /// Radius girasi terhadap tiap sumbu utama (mm).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius_of_gyration: Option<[f64; 3]>,
    /// Material mekanik yang dipilih (`set_material`), bila ada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mechanical: Option<MechanicalReport>,
    /// Sebutan part standar untuk BOM (hanya body buatan `standard_part`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub standard: Option<String>,
    /// Ulir yang tercatat pada body (kosmetik maupun fisik).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub threads: Vec<crate::session::ThreadNote>,
    /// Data sheet metal (hanya body buatan `base_flange`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheet_metal: Option<SheetMetalReport>,
    /// Mis. `{"plane": 6, "cylinder": 4}`.
    pub face_kinds: BTreeMap<String, usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topology: Option<Topology>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MaterialReport {
    pub preset: String,
    pub base_color: [f64; 4],
    pub roughness: f64,
    pub metallic: f64,
    pub clearcoat: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub density_g_cm3: Option<f64>,
}

impl MaterialReport {
    pub fn from_material(m: &ducad_core::Material) -> Self {
        Self {
            preset: format!("{:?}", m.preset),
            base_color: m.base_color.map(|c| round4(c as f64)),
            roughness: round4(m.roughness as f64),
            metallic: round4(m.metallic as f64),
            clearcoat: round4(m.clearcoat as f64),
            density_g_cm3: m.preset.density_g_cm3(),
        }
    }
}

/// Material mekanik sebuah body beserta sifatnya (bila sumbernya lengkap).
#[derive(Debug, Clone, Serialize)]
pub struct MechanicalReport {
    /// Kunci pustaka, `"custom"`, atau `"preset:<nama>"`.
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub density_g_cm3: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub young_modulus_gpa: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poisson_ratio: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yield_strength_mpa: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ultimate_strength_mpa: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thermal_expansion_per_k: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thermal_conductivity_w_mk: Option<f64>,
}

impl MechanicalReport {
    pub fn from_source(
        source: &ducad_core::MaterialSource,
        custom: &[(String, ducad_core::MechanicalProperties)],
    ) -> Self {
        let p = source.resolve(custom);
        Self {
            source: source.label(),
            density_g_cm3: source.density_g_cm3(custom),
            young_modulus_gpa: p.map(|p| p.young_modulus_gpa),
            poisson_ratio: p.map(|p| p.poisson_ratio),
            yield_strength_mpa: p.map(|p| p.yield_strength_mpa),
            ultimate_strength_mpa: p.map(|p| p.ultimate_strength_mpa),
            thermal_expansion_per_k: p.map(|p| p.thermal_expansion_per_k),
            thermal_conductivity_w_mk: p.map(|p| p.thermal_conductivity_w_mk),
        }
    }
}

/// Ringkasan sheet metal satu body.
#[derive(Debug, Clone, Serialize)]
pub struct SheetMetalReport {
    pub thickness: f64,
    pub default_radius: f64,
    pub k_factor: f64,
    /// Sedang berbentuk pola datar (`unfold`).
    pub unfolded: bool,
    pub flanges: Vec<FlangeReport>,
    /// Luas pola datar (mm²).
    pub flat_area: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FlangeReport {
    pub id: String,
    /// Indeks sisi pelat dasar.
    pub edge: usize,
    /// Panjang bentangan = Σ (bend allowance + panjang lurus), mm.
    pub developed_length: f64,
    /// `[sudut°, radius, panjang lurus]` per tekukan.
    pub bends: Vec<[f64; 3]>,
}

impl SheetMetalReport {
    pub fn from_state(state: &crate::session::SheetMetalState) -> Self {
        let m = &state.model;
        Self {
            thickness: round4(m.thickness),
            default_radius: round4(m.default_radius),
            k_factor: round4(m.k_factor),
            unfolded: state.unfolded,
            flanges: m
                .flanges
                .iter()
                .map(|f| FlangeReport {
                    id: f.id.clone(),
                    edge: f.edge,
                    developed_length: round4(m.developed_length(f)),
                    bends: f
                        .segments
                        .iter()
                        .map(|s| [round4(s.angle_deg), round4(s.radius), round4(s.length)])
                        .collect(),
                })
                .collect(),
            flat_area: round4(m.flat_area()),
        }
    }
}

/// Properti massa gabungan beberapa body/instance.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AssemblyMassReport {
    pub total_mass_g: f64,
    pub center_of_mass: [f64; 3],
    /// Tensor inersia gabungan terhadap pusat massa gabungan (g·mm²).
    pub inertia_com: [[f64; 3]; 3],
    /// Body yang tidak ikut dijumlahkan karena densitasnya tidak diketahui.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<String>,
}

/// Properti massa satu bagian di koordinat dunia: masukan [`aggregate_mass`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PartMass {
    pub mass_g: f64,
    pub center_of_mass: [f64; 3],
    /// Tensor inersia terhadap pusat massa bagian itu, sumbu dunia (g·mm²).
    pub inertia_com: [[f64; 3]; 3],
}

/// Properti massa satu body (tanpa pembulatan). `None` bila body tidak
/// bervolume (mesh murni) atau densitasnya tidak diketahui.
pub fn part_mass(
    doc: &ducad_core::Document,
    body: &ducad_core::Body,
    geo: &BodyGeometry,
) -> Option<PartMass> {
    let density = doc.density_of(body)?;
    let mp = geo.shape.mass_properties();
    if mp.volume_mm3 <= 1e-9 {
        return None;
    }
    let rho = density / 1000.0;
    Some(PartMass {
        mass_g: mp.volume_mm3 * rho,
        center_of_mass: mp.centroid,
        inertia_com: mp.inertia_com().map(|r| r.map(|v| v * rho)),
    })
}

/// Jumlahkan properti massa beberapa bagian (teorema sumbu sejajar). Untuk
/// perakitan, pemanggil lebih dulu memindahkan tiap bagian ke koordinat
/// dunia dengan transformasi instance-nya. `None` bila massa total nol.
pub fn aggregate_mass(parts: &[PartMass]) -> Option<(f64, [f64; 3], [[f64; 3]; 3])> {
    let total: f64 = parts.iter().map(|p| p.mass_g).sum();
    if total <= 0.0 {
        return None;
    }
    let mut com = [0.0; 3];
    for p in parts {
        for (c, x) in com.iter_mut().zip(p.center_of_mass) {
            *c += p.mass_g * x / total;
        }
    }
    let mut inertia = [[0.0; 3]; 3];
    for p in parts {
        let d = [
            p.center_of_mass[0] - com[0],
            p.center_of_mass[1] - com[1],
            p.center_of_mass[2] - com[2],
        ];
        let shifted = ducad_kernel::shift_from_centroid(p.inertia_com, p.mass_g, d);
        for (row, srow) in inertia.iter_mut().zip(shifted) {
            for (v, sv) in row.iter_mut().zip(srow) {
                *v += sv;
            }
        }
    }
    Some((total, com, inertia))
}

#[derive(Debug, Clone, Serialize)]
pub struct Topology {
    pub faces: Vec<FaceInfo>,
    pub edges: Vec<EdgeInfo>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SketchReport {
    pub id: String,
    /// `"XY"`, `"XZ"`, `"YZ"`, atau `"datum:<n>"`.
    pub plane: String,
    pub entities: usize,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub entity_kinds: BTreeMap<String, usize>,
    pub closed_regions: usize,
    pub dof: i64,
    /// Nama entitas, terurut.
    pub names: Vec<String>,
}

pub fn round4(v: f64) -> f64 {
    let r = (v * 1e4).round() / 1e4;
    // Hindari "-0.0" di JSON.
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

fn r3(a: [f64; 3]) -> [f64; 3] {
    a.map(round4)
}

fn r33(a: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    a.map(r3)
}

fn round_face(mut f: FaceInfo) -> FaceInfo {
    f.centroid = r3(f.centroid);
    f.normal = r3(f.normal);
    f.area = round4(f.area);
    f.radius = f.radius.map(round4);
    f.axis = f.axis.map(|(p, d)| (r3(p), r3(d)));
    f.bbox = (r3(f.bbox.0), r3(f.bbox.1));
    f
}

fn round_edge(mut e: EdgeInfo) -> EdgeInfo {
    e.start = r3(e.start);
    e.end = r3(e.end);
    e.mid = r3(e.mid);
    e.length = round4(e.length);
    e.dir = e.dir.map(r3);
    e.radius = e.radius.map(round4);
    e
}

pub(crate) fn body_report(
    doc: &ducad_core::Document,
    body: &ducad_core::Body,
    geo: &BodyGeometry,
    topology: bool,
    limit: usize,
) -> BodyReport {
    let faces = ducad_kernel::enumerate_faces(&geo.shape);
    let edges = ducad_kernel::enumerate_edges(&geo.shape);
    let mut face_kinds: BTreeMap<String, usize> = BTreeMap::new();
    for f in &faces {
        *face_kinds
            .entry(crate::select::eval::face_kind_name(f.kind).to_string())
            .or_default() += 1;
    }
    let (min, max) = geo.mesh.bounding_box().unwrap_or(([0.0; 3], [0.0; 3]));
    let min = min.map(|v| v as f64);
    let max = max.map(|v| v as f64);
    let (n_faces, n_edges) = (faces.len(), edges.len());
    let topology = topology.then(|| Topology {
        truncated: n_faces > limit || n_edges > limit,
        faces: faces.into_iter().take(limit).map(round_face).collect(),
        edges: edges.into_iter().take(limit).map(round_edge).collect(),
    });
    let mp = geo.shape.mass_properties();
    let solid = mp.volume_mm3 > 1e-9;
    let density = doc.density_of(body);
    // g/mm³; `None` → besaran bermassa tidak dilaporkan.
    let rho = density.filter(|_| solid).map(|d| d / 1000.0);
    let (principal_geo, axes) = mp.principal();
    BodyReport {
        name: body.name.clone(),
        uuid: body.uuid.clone(),
        visible: body.visible,
        valid: geo.shape.is_valid(),
        volume: round4(geo.shape.volume().abs()),
        area: round4(geo.shape.surface_area()),
        bbox: [r3(min), r3(max)],
        size: r3([max[0] - min[0], max[1] - min[1], max[2] - min[2]]),
        centroid: r3(ducad_kernel::compute_mesh_centroid(&geo.mesh)),
        faces: n_faces,
        edges: n_edges,
        mass_g: density.map(|d| round4(mp.volume_mm3 / 1000.0 * d)),
        material: Some(MaterialReport::from_material(&body.material)),
        center_of_mass: solid.then(|| r3(mp.centroid)),
        inertia_com: rho.map(|r| r33(mp.inertia_com().map(|row| row.map(|v| v * r)))),
        inertia_origin: rho.map(|r| r33(mp.inertia_origin.map(|row| row.map(|v| v * r)))),
        principal_moments: rho.map(|r| r3(principal_geo.map(|v| v * r))),
        principal_axes: solid.then(|| r33(axes)),
        radius_of_gyration: solid
            .then(|| r3(principal_geo.map(|v| (v.max(0.0) / mp.volume_mm3).sqrt()))),
        mechanical: body
            .mechanical
            .as_ref()
            .map(|m| MechanicalReport::from_source(m, &doc.material_library)),
        sheet_metal: None,
        standard: None,
        threads: Vec::new(),
        face_kinds,
        topology,
    }
}

fn plane_label(p: PlaneRef) -> String {
    match p {
        PlaneRef::Top => "XY".into(),
        PlaneRef::Front => "XZ".into(),
        PlaneRef::Right => "YZ".into(),
        PlaneRef::Datum(n) => format!("datum:{n}"),
    }
}

/// Inti `summarize` di atas state pinjaman (dipakai `Session::run`).
pub(crate) fn summarize_state(
    model: &ModelDoc,
    sketches: &SketchSet,
    meta: &SessionMeta,
    only_body: Option<&str>,
    topology: bool,
    limit: usize,
) -> Summary {
    let mut bodies: Vec<BodyReport> = model
        .doc
        .bodies
        .iter()
        .filter(|(_, b)| only_body.is_none_or(|n| b.name == n))
        .filter_map(|(id, b)| {
            Some(body_report(
                &model.doc,
                b,
                model.geometry.get(id)?,
                topology,
                limit,
            ))
        })
        .collect();
    for b in &mut bodies {
        b.sheet_metal = meta
            .sheet_metal
            .get(&b.name)
            .map(SheetMetalReport::from_state);
        b.standard = meta.standard_parts.get(&b.name).cloned();
        b.threads = meta.threads.get(&b.name).cloned().unwrap_or_default();
    }
    bodies.sort_by(|a, b| a.name.cmp(&b.name));
    let assembly = (only_body.is_none() && model.doc.bodies.len() >= 2)
        .then(|| assembly_mass(model))
        .flatten();

    let sketches = meta
        .sketch_ids
        .iter()
        .filter_map(|(id, sid)| {
            let slot = sketches.get(*sid)?;
            let mut names: Vec<String> = slot.sketch.entity_names.values().cloned().collect();
            names.sort();
            let mut entity_kinds = BTreeMap::new();
            for (_, ent) in &slot.sketch.entities {
                let kind = match ent {
                    ducad_sketch::Entity::Line { .. } => "line",
                    ducad_sketch::Entity::Circle { .. } => "circle",
                    ducad_sketch::Entity::Arc { .. } => "arc",
                    ducad_sketch::Entity::Ellipse { .. } => "ellipse",
                    ducad_sketch::Entity::Spline { .. } => "spline",
                    ducad_sketch::Entity::Path { .. } => "path",
                };
                *entity_kinds.entry(kind.to_string()).or_default() += 1;
            }
            let dof =
                ducad_sketch::constraint::analyze_dof(&slot.sketch, &slot.sketch.constraints).dof;
            Some(SketchReport {
                id: id.clone(),
                plane: plane_label(slot.plane),
                entities: slot.sketch.entities.len(),
                entity_kinds,
                closed_regions: ducad_sketch::find_closed_regions(&slot.sketch).len(),
                dof: dof as i64,
                names,
            })
        })
        .collect();

    Summary {
        unit: "mm",
        bodies,
        sketches,
        params: meta.design.effective_params(),
        active_configuration: meta.design.active().map(|c| c.name.clone()),
        configurations: meta
            .design
            .configurations
            .iter()
            .map(|c| ConfigurationReport {
                name: c.name.clone(),
                active: meta.design.active_configuration.as_deref() == Some(c.name.as_str()),
                params: c.params.clone(),
                suppressed_ops: c.suppressed_ops.clone(),
            })
            .collect(),
        drawings: {
            // Sidik jari hanya dihitung bila memang ada lembar yang pernah dirender.
            let current = (!meta.drawing_rendered.is_empty() && !meta.design.drawings.is_empty())
                .then(|| crate::session::fingerprint(model));
            meta.design
                .drawings
                .iter()
                .map(|d| DrawingReport {
                    summary: crate::drawing_auto::summarize(d),
                    stale: match (meta.drawing_rendered.get(&d.name), &current) {
                        (Some(rendered), Some(now)) => rendered != now,
                        _ => false,
                    },
                })
                .collect()
        },
        oplog_len: meta.design.oplog.len(),
        warnings: meta.warnings.clone(),
        assembly,
    }
}

/// Properti massa gabungan seluruh body model (posisi dunia apa adanya).
pub fn assembly_mass(model: &ModelDoc) -> Option<AssemblyMassReport> {
    let mut named: Vec<(&str, Option<PartMass>)> = model
        .doc
        .bodies
        .iter()
        .filter_map(|(id, b)| {
            let geo = model.geometry.get(id)?;
            Some((b.name.as_str(), part_mass(&model.doc, b, geo)))
        })
        .collect();
    // Urutan tetap supaya penjumlahan floating point deterministik.
    named.sort_by(|a, b| a.0.cmp(b.0));
    let parts: Vec<PartMass> = named.iter().filter_map(|(_, p)| *p).collect();
    let (total, com, inertia) = aggregate_mass(&parts)?;
    Some(AssemblyMassReport {
        total_mass_g: round4(total),
        center_of_mass: r3(com),
        inertia_com: r33(inertia),
        skipped: named
            .iter()
            .filter(|(_, p)| p.is_none())
            .map(|(n, _)| n.to_string())
            .collect(),
    })
}

/// Seperti [`summarize`] tetapi di atas state pinjaman — dipakai jembatan
/// live (P5) yang menjalankan tool atas state GUI, bukan atas `Session`.
pub fn summarize_core(
    core: &crate::session::SessionCore,
    body: Option<&str>,
    topology: bool,
    limit: usize,
) -> OpResult<Summary> {
    if let Some(name) = body {
        core.body(name)?;
    }
    Ok(summarize_state(
        core.model,
        core.sketches,
        core.meta,
        body,
        topology,
        limit,
    ))
}

/// Ringkasan sesi. `body` membatasi ke satu body (UnknownRef /
/// BodyConsumed bila tidak ada); `topology` menyertakan daftar face/tepi
/// maksimal `limit` per body.
pub fn summarize(
    s: &Session,
    body: Option<&str>,
    topology: bool,
    limit: usize,
) -> OpResult<Summary> {
    if let Some(name) = body {
        s.body(name)?;
    }
    Ok(summarize_state(
        s.model(),
        s.sketches(),
        s.meta(),
        body,
        topology,
        limit,
    ))
}
