//! Resolusi referensi datum terhadap model (Fase 1 butir 1.2–1.4, 1.6):
//! `PointRef`/`LineRef`/`EdgeRef`/`FaceRef`/`PlaneSpec` → geometri, lalu
//! rumus murni di [`crate::datum`]. Query B-rep hanya lewat API kernel yang
//! sudah ada (`enumerate_faces`/`enumerate_edges` + selector engine).

use std::collections::BTreeMap;

use ducad_kernel::{EdgeInfo, FaceInfo, KernelShape, SurfaceKind};
use glam::DVec3;
use serde_json::json;

use crate::datum::{self, Axis, EdgeCurve, LIN_TOL};
use crate::error::{OpError, OpErrorCode, OpResult};
use crate::ops::num::{eval, eval_arr, Params};
use crate::ops::spec::{
    AxisDef, EdgeRef, FaceRef, LineRef, PlaneSpec, PointDef, PointRef, VertexEnd,
};
use crate::plane::PlaneFrame;
use crate::select::{parse_datum_ref, DatumRef};

/// Konteks resolusi: param, pencari body, dan datum yang sudah dibuat.
pub(crate) struct DatumCtx<'a> {
    pub params: &'a Params,
    pub body_shape: &'a dyn Fn(&str) -> OpResult<KernelShape>,
    pub axes: &'a BTreeMap<String, Axis>,
    pub points: &'a BTreeMap<String, [f64; 3]>,
}

fn unknown_datum(kind: &str, id: &str, known: Vec<&String>) -> OpError {
    OpError::new(
        OpErrorCode::UnknownRef,
        format!("datum {kind} '{id}' tidak dikenal (yang ada: {known:?})"),
    )
    .with_hint(format!(
        "buat dulu dengan op construct_{kind} id \"{id}\" di batch/oplog SEBELUM op ini"
    ))
    .with_context(json!({ "ref": format!("{kind}:{id}"), "available": known }))
}

/// `"axis:<id>"` → sumbu datum.
pub(crate) fn axis_by_ref(src: &str, ctx: &DatumCtx) -> OpResult<Axis> {
    match parse_datum_ref(src) {
        Some(Ok(DatumRef::Axis(id))) => ctx
            .axes
            .get(&id)
            .copied()
            .ok_or_else(|| unknown_datum("axis", &id, ctx.axes.keys().collect())),
        Some(Err(e)) => Err(e),
        _ => Err(
            OpError::invalid(format!("\"{src}\" bukan referensi sumbu datum"))
                .with_hint("tulis \"axis:<id>\" dengan <id> = id op construct_axis"),
        ),
    }
}

/// Titik: koordinat atau `"point:<id>"`.
pub(crate) fn point(p: &PointRef, ctx: &DatumCtx) -> OpResult<DVec3> {
    match p {
        PointRef::Coords(c) => Ok(DVec3::from_array(eval_arr(c, ctx.params)?)),
        PointRef::Datum(src) => match parse_datum_ref(src) {
            Some(Ok(DatumRef::Point(id))) => ctx
                .points
                .get(&id)
                .map(|p| DVec3::from_array(*p))
                .ok_or_else(|| unknown_datum("point", &id, ctx.points.keys().collect())),
            Some(Err(e)) => Err(e),
            _ => Err(OpError::invalid(format!("\"{src}\" bukan titik"))
                .with_hint("titik ditulis [x,y,z] atau \"point:<id>\" (id op construct_point)")),
        },
    }
}

fn shape(body: &str, ctx: &DatumCtx) -> OpResult<KernelShape> {
    (ctx.body_shape)(body)
}

/// Tepat satu edge `sel` di `body`: `(edge, semua face, semua edge)`.
pub(crate) fn one_edge(
    body: &str,
    sel: &str,
    ctx: &DatumCtx,
) -> OpResult<(EdgeInfo, Vec<FaceInfo>, Vec<EdgeInfo>)> {
    let shape = shape(body, ctx)?;
    let idx = crate::select::select_edges(&shape, sel)?;
    if idx.len() != 1 {
        return Err(OpError::invalid(format!(
            "selector edge \"{sel}\" pada body '{body}' harus menghasilkan tepat 1 edge, bukan {}",
            idx.len()
        ))
        .with_hint("persempit selector dengan filter, mis. \"|Z[x=30][y=20]\" atau idx:<i> (uji dengan query_geometry)")
        .with_context(json!({ "matched": idx.len(), "indices": idx })));
    }
    let edges = ducad_kernel::enumerate_edges(&shape);
    let faces = ducad_kernel::enumerate_faces(&shape);
    let e = edges
        .iter()
        .find(|e| e.index == idx[0])
        .cloned()
        .ok_or_else(|| OpError::invalid(format!("edge #{} tidak ditemukan", idx[0])))?;
    Ok((e, faces, edges))
}

/// Tepat satu face `sel` di `body`: `(face, semua edge)`.
pub(crate) fn one_face(
    body: &str,
    sel: &str,
    ctx: &DatumCtx,
) -> OpResult<(FaceInfo, Vec<EdgeInfo>)> {
    let shape = shape(body, ctx)?;
    let idx = crate::select::select_faces(&shape, sel)?;
    if idx.len() != 1 {
        return Err(OpError::invalid(format!(
            "selector face \"{sel}\" pada body '{body}' harus menghasilkan tepat 1 face, bukan {}",
            idx.len()
        ))
        .with_hint("persempit selector, mis. \">Z\" atau \"all[kind=cylinder][r=5]\" (uji dengan query_geometry)")
        .with_context(json!({ "matched": idx.len(), "indices": idx })));
    }
    let faces = ducad_kernel::enumerate_faces(&shape);
    let f = faces
        .into_iter()
        .find(|f| f.index == idx[0])
        .ok_or_else(|| OpError::invalid(format!("face #{} tidak ditemukan", idx[0])))?;
    Ok((f, ducad_kernel::enumerate_edges(&shape)))
}

/// Kurva eksak satu edge.
pub(crate) fn edge_curve(body: &str, sel: &str, ctx: &DatumCtx) -> OpResult<EdgeCurve> {
    let (e, faces, _) = one_edge(body, sel, ctx)?;
    EdgeCurve::from_edge(&e, &faces)
}

/// Garis lurus: sumbu datum atau edge garis.
pub(crate) fn line(l: &LineRef, ctx: &DatumCtx) -> OpResult<Axis> {
    match l {
        LineRef::Datum(src) => axis_by_ref(src, ctx),
        LineRef::Edge { body, edge } => match edge_curve(body, edge, ctx)? {
            EdgeCurve::Line { start, end } => datum::axis_two_points(start, end),
            EdgeCurve::Arc { .. } => Err(OpError::invalid(format!(
                "edge \"{edge}\" pada body '{body}' melengkung; dibutuhkan edge lurus"
            ))
            .with_hint("tambahkan filter [kind=line], atau buat sumbunya dulu dengan construct_axis {\"edge\":...} lalu pakai \"axis:<id>\"")),
        },
    }
}

fn planar_frame(f: &FaceInfo, body: &str, sel: &str) -> OpResult<PlaneFrame> {
    if f.kind != SurfaceKind::Plane {
        return Err(OpError::invalid(format!(
            "face \"{sel}\" pada body '{body}' bukan bidang datar (jenis {:?})",
            f.kind
        ))
        .with_hint("pilih face datar, mis. \">Z\" atau tambahkan filter [kind=plane]"));
    }
    Ok(PlaneFrame::on_face(f.centroid, f.normal))
}

/// Sumbu face silinder/kerucut; origin = proyeksi centroid face ke sumbu.
fn cylinder_axis(f: &FaceInfo, body: &str, sel: &str) -> OpResult<Axis> {
    match (f.kind, f.axis) {
        (SurfaceKind::Cylinder | SurfaceKind::Cone, Some((o, d))) => {
            let raw = Axis::new(DVec3::from_array(o), DVec3::from_array(d))?;
            Axis::new(
                raw.closest_point(DVec3::from_array(f.centroid)),
                raw.dir_v(),
            )
        }
        _ => Err(OpError::invalid(format!(
            "face \"{sel}\" pada body '{body}' bukan silinder/kerucut (jenis {:?})",
            f.kind
        ))
        .with_hint(
            "pilih face silinder, mis. \"all[kind=cylinder]\" atau \"all[kind=cylinder][r=5]\"",
        )),
    }
}

/// Frame bidang untuk SETIAP varian `PlaneSpec` (standar, offset, face, dan
/// bidang konstruksi).
pub(crate) fn plane(spec: &PlaneSpec, ctx: &DatumCtx) -> OpResult<PlaneFrame> {
    Ok(crate::ops::sketch::resolve_plane_in(spec, ctx)?.frame())
}

/// Varian konstruksi `PlaneSpec` (Midplane … TwoEdges). Varian dasar →
/// `None` (diurus `ops::sketch::resolve_plane_in`).
pub(crate) fn construct_plane(spec: &PlaneSpec, ctx: &DatumCtx) -> OpResult<Option<PlaneFrame>> {
    let frame = match spec {
        PlaneSpec::Named(_) | PlaneSpec::Offset { .. } | PlaneSpec::OnFace { .. } => return Ok(None),
        PlaneSpec::Midplane { midplane } => {
            let [a, b] = midplane.as_slice() else {
                return Err(OpError::invalid(format!(
                    "midplane butuh tepat 2 bidang, bukan {}",
                    midplane.len()
                ))
                .with_hint("contoh: {\"midplane\":[{\"body\":\"b\",\"face\":\">Z\"},{\"body\":\"b\",\"face\":\"<Z\"}]}"));
            };
            datum::plane_midplane(&plane(a, ctx)?, &plane(b, ctx)?)?
        }
        PlaneSpec::Perpendicular {
            perpendicular_to,
            through,
        } => {
            let ax = line(perpendicular_to, ctx)?;
            datum::plane_perpendicular(ax.dir_v(), point(through, ctx)?)?
        }
        PlaneSpec::Tangent {
            tangent_to: FaceRef { body, face },
            angle_deg,
            through,
        } => {
            let (f, _) = one_face(body, face, ctx)?;
            if f.kind != SurfaceKind::Cylinder {
                return Err(OpError::invalid(format!(
                    "bidang singgung butuh face silinder; \"{face}\" berjenis {:?}",
                    f.kind
                ))
                .with_hint("pilih face silinder, mis. \"all[kind=cylinder]\""));
            }
            let ax = cylinder_axis(&f, body, face)?;
            let r = f
                .radius
                .ok_or_else(|| OpError::invalid("radius face silinder tidak diketahui"))?;
            match (angle_deg, through) {
                (Some(_), Some(_)) => {
                    return Err(OpError::invalid("tangent: pakai salah satu, angle_deg ATAU through")
                        .with_hint("hapus salah satu field"))
                }
                (_, Some(p)) => datum::plane_tangent_cylinder_through(&ax, r, point(p, ctx)?)?,
                (a, None) => {
                    let deg = a.as_ref().map(|a| eval(a, ctx.params)).transpose()?.unwrap_or(0.0);
                    datum::plane_tangent_cylinder(&ax, r, deg)?
                }
            }
        }
        PlaneSpec::AlongPath {
            along: EdgeRef { body, edge },
            fraction,
        } => {
            let c = edge_curve(body, edge, ctx)?;
            datum::plane_along_curve(&c, eval(fraction, ctx.params)?)?
        }
        PlaneSpec::TwoEdges {
            through_edges,
            through,
        } => match (through_edges.as_slice(), through) {
            ([a, b], None) => {
                let (a, b) = (line(a, ctx)?, line(b, ctx)?);
                datum::plane_through_lines(
                    a.origin_v(),
                    a.origin_v() + a.dir_v(),
                    b.origin_v(),
                    b.origin_v() + b.dir_v(),
                )?
            }
            ([a], Some(p)) => {
                let a = line(a, ctx)?;
                datum::plane_line_point(a.origin_v(), a.origin_v() + a.dir_v(), point(p, ctx)?)?
            }
            _ => {
                return Err(OpError::invalid(format!(
                    "through_edges: berikan 2 edge/sumbu tanpa through, atau 1 edge/sumbu + through (diberikan {} edge, through {})",
                    through_edges.len(),
                    if through.is_some() { "ada" } else { "tidak ada" }
                ))
                .with_hint("contoh: {\"through_edges\":[\"axis:ax1\"],\"through\":[0,0,10]}"))
            }
        },
        PlaneSpec::ThreePoints { three_points } => {
            let [a, b, c] = &**three_points;
            let (a, b, c) = (point(a, ctx)?, point(b, ctx)?, point(c, ctx)?);
            datum::plane_line_point(a, b, c)?
        }
        PlaneSpec::Frame { frame } => {
            let v3 = |a: &[crate::ops::Num; 3]| -> OpResult<DVec3> {
                Ok(DVec3::from_array(eval_arr(a, ctx.params)?))
            };
            let (u, v, n) = (v3(&frame.u)?, v3(&frame.v)?, v3(&frame.normal)?);
            let unit = |x: DVec3, name: &str| -> OpResult<DVec3> {
                let l = x.length();
                if l.is_finite() && l > 1e-12 {
                    Ok(x / l)
                } else {
                    Err(OpError::invalid(format!("frame.{name} tidak boleh vektor nol")))
                }
            };
            let (u, v, n) = (unit(u, "u")?, unit(v, "v")?, unit(n, "normal")?);
            if u.dot(v).abs() > 1e-6 || u.dot(n).abs() > 1e-6 || v.dot(n).abs() > 1e-6 {
                return Err(OpError::invalid("frame: u, v, dan normal harus saling tegak lurus"));
            }
            PlaneFrame {
                origin: v3(&frame.origin)?.to_array(),
                u_axis: u.to_array(),
                v_axis: v.to_array(),
                normal: n.to_array(),
            }
        }
    };
    Ok(Some(frame))
}

/// Op `construct_axis`.
pub(crate) fn construct_axis(def: &AxisDef, ctx: &DatumCtx) -> OpResult<Axis> {
    match def {
        AxisDef::TwoPoints { a, b } => datum::axis_two_points(point(a, ctx)?, point(b, ctx)?),
        AxisDef::Edge { body, edge } => edge_curve(body, edge, ctx)?.as_axis(),
        AxisDef::NormalToFace {
            body,
            face,
            through,
        } => {
            let (f, _) = one_face(body, face, ctx)?;
            let frame = planar_frame(&f, body, face)?;
            let p = through.as_ref().map(|p| point(p, ctx)).transpose()?;
            datum::axis_normal_to_plane(&frame, p)
        }
        AxisDef::ThroughCylinder { body, face } => {
            let (f, _) = one_face(body, face, ctx)?;
            cylinder_axis(&f, body, face)
        }
        AxisDef::TwoPlanes { a, b } => datum::axis_two_planes(&plane(a, ctx)?, &plane(b, ctx)?),
    }
}

/// Op `construct_point` → `(titik, peringatan)`.
pub(crate) fn construct_point(def: &PointDef, ctx: &DatumCtx) -> OpResult<(DVec3, Vec<String>)> {
    let mut warnings = Vec::new();
    let p = match def {
        PointDef::Vertex {
            body,
            edge,
            end,
            near,
        } => match (edge, near) {
            (Some(sel), None) => {
                let (e, _, _) = one_edge(body, sel, ctx)?;
                DVec3::from_array(match end {
                    VertexEnd::Start => e.start,
                    VertexEnd::End => e.end,
                })
            }
            (None, Some(near)) => {
                let target = DVec3::from_array(eval_arr(near, ctx.params)?);
                let edges = ducad_kernel::enumerate_edges(&shape(body, ctx)?);
                let best = edges
                    .iter()
                    .flat_map(|e| [e.start, e.end])
                    .map(DVec3::from_array)
                    .filter(|p| p.is_finite())
                    .min_by(|a, b| (*a - target).length().total_cmp(&(*b - target).length()))
                    .ok_or_else(|| OpError::invalid(format!("body '{body}' tidak punya vertex")))?;
                let d = (best - target).length();
                if d > LIN_TOL {
                    warnings.push(format!(
                        "vertex terdekat berjarak {d:.3} mm dari titik near"
                    ));
                }
                best
            }
            _ => {
                return Err(
                    OpError::invalid("vertex: berikan tepat satu dari `edge` atau `near`")
                        .with_hint("contoh: {\"vertex\":{\"body\":\"b\",\"near\":[30,20,10]}}"),
                )
            }
        },
        PointDef::Center { body, edge, face } => match (edge, face) {
            (Some(sel), None) => match edge_curve(body, sel, ctx)? {
                EdgeCurve::Arc { center, .. } => center,
                EdgeCurve::Line { .. } => {
                    return Err(OpError::invalid(format!(
                        "edge \"{sel}\" lurus; center butuh edge lingkaran/busur"
                    ))
                    .with_hint("tambahkan filter [kind=circle], mis. \"of(>Z)[kind=circle]\""))
                }
            },
            (None, Some(sel)) => {
                let (f, edges) = one_face(body, sel, ctx)?;
                face_center(&f, &edges, sel, &mut warnings)?
            }
            _ => {
                return Err(
                    OpError::invalid("center: berikan tepat satu dari `edge` atau `face`")
                        .with_hint("edge lingkaran → pusat lingkaran; face bola/torus → pusatnya"),
                )
            }
        },
        PointDef::EdgePlane {
            body,
            edge,
            plane: pl,
        } => {
            let c = edge_curve(body, edge, ctx)?;
            c.intersect_plane(&plane(pl, ctx)?)?
        }
        PointDef::TwoEdges { a, b } => {
            let (p, d) = datum::point_two_lines(&line(a, ctx)?, &line(b, ctx)?)?;
            if d > LIN_TOL {
                warnings.push(format!(
                    "kedua garis tidak berpotongan (jarak {d:.4} mm); dipakai titik tengah ruas terdekat"
                ));
            }
            p
        }
        PointDef::ThreePlanes { a, b, c } => {
            datum::point_three_planes(&plane(a, ctx)?, &plane(b, ctx)?, &plane(c, ctx)?)?
        }
        PointDef::AlongPath {
            body,
            edge,
            fraction,
        } => {
            let f = eval(fraction, ctx.params)?;
            if !(f.is_finite() && (0.0..=1.0).contains(&f)) {
                return Err(
                    OpError::invalid(format!("fraction harus di 0..1 (diberikan {f})"))
                        .with_hint("0 = awal edge, 0.5 = tengah, 1 = akhir"),
                );
            }
            edge_curve(body, edge, ctx)?.point_at(f)
        }
    };
    if !p.is_finite() {
        return Err(OpError::new(
            OpErrorCode::EmptyResult,
            "titik datum tidak finite",
        ));
    }
    Ok((p, warnings))
}

/// Pusat face bola (dari titik batas/edge + radius + centroid)
/// atau torus (centroid face; tepat untuk torus penuh).
fn face_center(
    f: &FaceInfo,
    edges: &[EdgeInfo],
    sel: &str,
    warnings: &mut Vec<String>,
) -> OpResult<DVec3> {
    let own: Vec<&EdgeInfo> = edges
        .iter()
        .filter(|e| f.edge_indices.contains(&e.index))
        .collect();
    match f.kind {
        SurfaceKind::Sphere => {
            let r = f
                .radius
                .ok_or_else(|| OpError::invalid("radius face bola tidak diketahui"))?;
            let samples: Vec<DVec3> = f
                .boundary
                .iter()
                .copied()
                .chain(own.iter().flat_map(|e| [e.start, e.mid, e.end]))
                .map(DVec3::from_array)
                .collect();
            datum::sphere_center(&samples, r, DVec3::from_array(f.centroid))
        }
        SurfaceKind::Torus => {
            warnings.push("pusat torus diambil dari centroid face (tepat untuk torus penuh)".into());
            Ok(DVec3::from_array(f.centroid))
        }
        k => Err(OpError::invalid(format!(
            "face \"{sel}\" berjenis {k:?}; center butuh face bola/torus atau edge lingkaran"
        ))
        .with_hint("untuk lubang/poros pakai construct_axis {\"through_cylinder\":...}, atau center dengan edge lingkarannya")),
    }
}
