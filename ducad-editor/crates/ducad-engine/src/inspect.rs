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
    /// Mis. `{"plane": 6, "cylinder": 4}`.
    pub face_kinds: BTreeMap<String, usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topology: Option<Topology>,
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
        .filter_map(|(id, b)| Some(body_report(b, model.geometry.get(id)?, topology, limit)))
        .collect();
    bodies.sort_by(|a, b| a.name.cmp(&b.name));

    let sketches = meta
        .sketch_ids
        .iter()
        .filter_map(|(id, sid)| {
            let slot = sketches.get(*sid)?;
            let mut names: Vec<String> = slot.sketch.entity_names.values().cloned().collect();
            names.sort();
            let dof =
                ducad_sketch::constraint::analyze_dof(&slot.sketch, &slot.sketch.constraints).dof;
            Some(SketchReport {
                id: id.clone(),
                plane: plane_label(slot.plane),
                entities: slot.sketch.entities.len(),
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
        params: meta.design.params.clone(),
        oplog_len: meta.design.oplog.len(),
        warnings: meta.warnings.clone(),
    }
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
