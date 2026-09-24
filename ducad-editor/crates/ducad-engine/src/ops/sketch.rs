//! Evaluasi `Op::Sketch` (P1.3): resolusi bidang, ekspansi `EntitySpec`,
//! penamaan entitas, terjemahan `ConstraintSpec`, dan solve.

use std::collections::{BTreeSet, HashMap};

use ducad_kernel::{KernelShape, SurfaceKind};
use ducad_sketch::constraint::{Constraint, DofReport, PointRef};
use ducad_sketch::entity::{PathSeg, Subpath};
use ducad_sketch::layer::Layer;
use ducad_sketch::style::{
    BlendMode, FillRule, LineCap, LineJoin, Paint, Rgba, StrokeStyle, Style,
};
use ducad_sketch::{Entity, EntityId, PlaneRef, Sketch};
use glam::DVec2;

use super::num::{eval, eval_arr, Num, Params};
use super::spec::{ConstraintSpec, EntitySpec, PlaneSpec, SegSpec, StyleSpec};
use crate::error::{OpError, OpErrorCode, OpResult};
use crate::plane::PlaneFrame;

/// Hasil resolusi `PlaneSpec`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ResolvedPlane {
    /// Salah satu bidang standar.
    Standard(PlaneFrame, PlaneRef),
    /// Bidang turunan yang harus didaftarkan sebagai datum baru.
    Datum(PlaneFrame),
}

impl ResolvedPlane {
    #[cfg(test)]
    pub fn frame(&self) -> PlaneFrame {
        match self {
            ResolvedPlane::Standard(f, _) | ResolvedPlane::Datum(f) => *f,
        }
    }
}

fn named_plane(name: &str) -> OpResult<(PlaneFrame, PlaneRef)> {
    match name.to_ascii_lowercase().as_str() {
        "xy" | "top" => Ok((PlaneFrame::top(), PlaneRef::Top)),
        "xz" | "front" => Ok((PlaneFrame::front(), PlaneRef::Front)),
        "yz" | "right" => Ok((PlaneFrame::right(), PlaneRef::Right)),
        _ => Err(OpError::invalid(format!(
            "bidang '{name}' tidak dikenal (pakai XY/XZ/YZ atau top/front/right)"
        ))),
    }
}

/// Resolusi `PlaneSpec`. `body_shape` mencari shape body berdasarkan nama
/// (dipakai `OnFace`).
pub(crate) fn resolve_plane(
    spec: &PlaneSpec,
    params: &Params,
    body_shape: &dyn Fn(&str) -> OpResult<KernelShape>,
) -> OpResult<ResolvedPlane> {
    match spec {
        PlaneSpec::Named(name) => {
            let (f, r) = named_plane(name)?;
            Ok(ResolvedPlane::Standard(f, r))
        }
        PlaneSpec::Offset { base, offset } => {
            let (mut f, _) = named_plane(base)?;
            let d = eval(offset, params)?;
            f.origin = (f.origin_v() + f.normal_v() * d).to_array();
            Ok(ResolvedPlane::Datum(f))
        }
        PlaneSpec::OnFace { body, face } => {
            let shape = body_shape(body)?;
            let idx = crate::select::select_faces(&shape, face)?;
            let faces = ducad_kernel::enumerate_faces(&shape);
            if idx.len() != 1 {
                return Err(OpError::invalid(format!(
                    "selector face \"{face}\" pada body '{body}' harus menghasilkan tepat 1 face, bukan {}",
                    idx.len()
                ))
                .with_context(serde_json::json!({ "matched": idx.len(), "indices": idx })));
            }
            let f = &faces[idx[0]];
            if f.kind != SurfaceKind::Plane {
                return Err(OpError::invalid(format!(
                    "face \"{face}\" pada body '{body}' bukan bidang datar (jenis {:?})",
                    f.kind
                )));
            }
            Ok(ResolvedPlane::Datum(PlaneFrame::on_face(
                f.centroid, f.normal,
            )))
        }
    }
}

fn p2(a: &[Num; 2], params: &Params) -> OpResult<DVec2> {
    let [x, y] = eval_arr(a, params)?;
    Ok(DVec2::new(x, y))
}

fn positive(what: &str, v: f64) -> OpResult<f64> {
    if v.is_nan() || v <= 0.0 {
        return Err(OpError::invalid(format!(
            "{what} harus > 0 (diberikan {v})"
        )));
    }
    Ok(v)
}

/// `(anak (sufiks nama, entitas), nama induk, konstruksi?)`.
type ExpandedGeom = (Vec<(Option<String>, Entity)>, Option<String>, bool);

/// `(anak (sufiks nama, entitas), nama induk, konstruksi?, style?, layer?)`.
type Expanded = (
    Vec<(Option<String>, Entity)>,
    Option<String>,
    bool,
    Option<Style>,
    Option<String>,
);

/// Satu `EntitySpec` geometris → daftar `(sufiks nama anak, Entity)`.
fn expand_geom(spec: &EntitySpec, params: &Params) -> OpResult<ExpandedGeom> {
    let single = |e: Entity| vec![(None, e)];
    let numbered = |es: Vec<Entity>| {
        es.into_iter()
            .enumerate()
            .map(|(i, e)| (Some(i.to_string()), e))
            .collect::<Vec<_>>()
    };
    let (items, name, construction) = match spec {
        EntitySpec::Line {
            from,
            to,
            name,
            construction,
        } => {
            let (a, b) = (p2(from, params)?, p2(to, params)?);
            if (a - b).length() < 1e-9 {
                return Err(OpError::invalid("garis berpanjang nol"));
            }
            (single(Entity::line(a, b)), name, *construction)
        }
        EntitySpec::Rect {
            center,
            corner,
            w,
            h,
            name,
            construction,
        } => {
            let (w, h) = (
                positive("lebar rect", eval(w, params)?)?,
                positive("tinggi rect", eval(h, params)?)?,
            );
            let bl = match (center, corner) {
                (Some(c), None) => p2(c, params)? - DVec2::new(w, h) * 0.5,
                (None, Some(c)) => p2(c, params)?,
                _ => {
                    return Err(OpError::invalid(
                        "rect butuh tepat satu dari 'center' atau 'corner'",
                    ))
                }
            };
            let c = [
                bl,
                bl + DVec2::new(w, 0.0),
                bl + DVec2::new(w, h),
                bl + DVec2::new(0.0, h),
            ];
            let items = ["bottom", "right", "top", "left"]
                .iter()
                .enumerate()
                .map(|(i, s)| (Some(s.to_string()), Entity::line(c[i], c[(i + 1) % 4])))
                .collect();
            (items, name, *construction)
        }
        EntitySpec::Circle {
            center,
            r,
            name,
            construction,
        } => (
            single(Entity::circle(
                p2(center, params)?,
                positive("radius lingkaran", eval(r, params)?)?,
            )),
            name,
            *construction,
        ),
        EntitySpec::Arc {
            center,
            r,
            start_deg,
            end_deg,
            name,
            construction,
        } => (
            single(Entity::arc(
                p2(center, params)?,
                positive("radius busur", eval(r, params)?)?,
                eval(start_deg, params)?.to_radians(),
                eval(end_deg, params)?.to_radians(),
            )),
            name,
            *construction,
        ),
        EntitySpec::Arc3 {
            p1,
            p2: q2,
            p3,
            name,
            construction,
        } => {
            let e = ducad_sketch::arc_from_three_points(
                p2(p1, params)?,
                p2(q2, params)?,
                p2(p3, params)?,
            )
            .ok_or_else(|| OpError::invalid("ketiga titik arc3 segaris atau berimpit"))?;
            (single(e), name, *construction)
        }
        EntitySpec::Ellipse {
            center,
            rx,
            ry,
            name,
            construction,
        } => (
            single(Entity::ellipse(
                p2(center, params)?,
                positive("rx ellips", eval(rx, params)?)?,
                positive("ry ellips", eval(ry, params)?)?,
            )),
            name,
            *construction,
        ),
        EntitySpec::Polygon {
            center,
            r,
            sides,
            inscribed,
            name,
            construction,
        } => {
            if !(3..=64).contains(sides) {
                return Err(OpError::invalid(format!(
                    "jumlah sisi poligon harus 3..=64 (diberikan {sides})"
                )));
            }
            let c = p2(center, params)?;
            let r = positive("radius poligon", eval(r, params)?)?;
            let mode = if *inscribed {
                ducad_sketch::PolygonMode::Inscribed
            } else {
                ducad_sketch::PolygonMode::Circumscribed
            };
            let es = ducad_sketch::regular_polygon_entities(
                c,
                c + DVec2::new(r, 0.0),
                *sides as usize,
                mode,
                false,
            )
            .ok_or_else(|| OpError::invalid("poligon tidak valid"))?;
            (numbered(es), name, *construction)
        }
        EntitySpec::Slot {
            from,
            to,
            r,
            name,
            construction,
        } => {
            let (a, b) = (p2(from, params)?, p2(to, params)?);
            let r = positive("radius slot", eval(r, params)?)?;
            let d = b - a;
            if d.length() < 1e-9 {
                return Err(OpError::invalid("titik 'from' dan 'to' slot berimpit"));
            }
            let normal = DVec2::new(-d.y, d.x).normalize();
            let es = ducad_sketch::slot_from_points(
                a,
                b,
                a + normal * r,
                ducad_sketch::SlotMode::CenterToCenter,
                false,
            )
            .ok_or_else(|| OpError::invalid("slot tidak valid"))?;
            (numbered(es), name, *construction)
        }
        EntitySpec::Polyline {
            points,
            closed,
            name,
            construction,
        } => {
            if points.len() < 2 {
                return Err(OpError::invalid("polyline butuh minimal 2 titik"));
            }
            let pts = points
                .iter()
                .map(|p| p2(p, params))
                .collect::<OpResult<Vec<_>>>()?;
            let mut es: Vec<Entity> = pts.windows(2).map(|w| Entity::line(w[0], w[1])).collect();
            if *closed && pts.len() >= 3 {
                es.push(Entity::line(pts[pts.len() - 1], pts[0]));
            }
            (numbered(es), name, *construction)
        }
        EntitySpec::Spline {
            points,
            name,
            construction,
        } => {
            if points.len() < 3 {
                return Err(OpError::invalid("spline butuh minimal 3 titik"));
            }
            let pts = points
                .iter()
                .map(|p| p2(p, params))
                .collect::<OpResult<Vec<_>>>()?;
            (single(Entity::spline(pts)), name, *construction)
        }
        EntitySpec::Path { .. } => unreachable!("Path ditangani oleh expand"),
    };
    Ok((items, name.clone(), construction))
}

fn lower_style(st: &StyleSpec, params: &Params) -> OpResult<Style> {
    let fill = if let Some(hex) = &st.fill {
        let rgba = Rgba::from_hex(hex)
            .ok_or_else(|| OpError::invalid(format!("style.fill: warna hex tidak valid: {hex}")))?;
        Some(Paint::Solid(rgba))
    } else {
        None
    };

    let stroke = if let Some(hex) = &st.stroke {
        let rgba = Rgba::from_hex(hex).ok_or_else(|| {
            OpError::invalid(format!("style.stroke: warna hex tidak valid: {hex}"))
        })?;
        let width_mm = if let Some(sw) = &st.stroke_width {
            let val = eval(sw, params)?;
            if val <= 0.0 {
                return Err(OpError::invalid(format!(
                    "style.stroke_width harus lebih besar dari nol, didapat {val}"
                )));
            }
            val
        } else {
            0.5
        };
        Some(StrokeStyle {
            paint: Paint::Solid(rgba),
            width_mm,
            dash: vec![],
            cap: LineCap::Butt,
            join: LineJoin::Miter,
        })
    } else {
        if let Some(sw) = &st.stroke_width {
            let val = eval(sw, params)?;
            if val <= 0.0 {
                return Err(OpError::invalid(format!(
                    "style.stroke_width harus lebih besar dari nol, didapat {val}"
                )));
            }
        }
        None
    };

    let op_val = eval(&st.opacity, params)?;
    let opacity = op_val.clamp(0.0, 1.0) as f32;

    let fill_rule = if let Some(rule) = &st.fill_rule {
        match rule.to_lowercase().as_str() {
            "nonzero" => FillRule::NonZero,
            "evenodd" => FillRule::EvenOdd,
            _ => {
                return Err(OpError::invalid(format!(
                    "style.fill_rule tidak dikenal: {rule}"
                )))
            }
        }
    } else {
        FillRule::NonZero
    };

    Ok(Style {
        fill,
        fill_rule,
        stroke,
        opacity,
        blend: BlendMode::Normal,
    })
}

fn expand(spec: &EntitySpec, params: &Params) -> OpResult<Expanded> {
    match spec {
        EntitySpec::Path {
            subpaths,
            style,
            layer,
            name,
            construction,
        } => {
            if subpaths.is_empty() {
                return Err(OpError::invalid("path harus memiliki minimal satu subpath"));
            }
            let mut built_subpaths = Vec::with_capacity(subpaths.len());
            for (sp_idx, sp) in subpaths.iter().enumerate() {
                if sp.segs.is_empty() {
                    return Err(OpError::invalid(format!(
                        "subpath {sp_idx} tidak boleh kosong"
                    )));
                }
                let start = p2(&sp.start, params)?;
                let mut segs = Vec::with_capacity(sp.segs.len());
                for seg in &sp.segs {
                    match seg {
                        SegSpec::Line { to } => {
                            segs.push(PathSeg::Line {
                                end: p2(to, params)?,
                            });
                        }
                        SegSpec::Cubic { c1, c2, to } => {
                            segs.push(PathSeg::Cubic {
                                c1: p2(c1, params)?,
                                c2: p2(c2, params)?,
                                end: p2(to, params)?,
                            });
                        }
                    }
                }
                built_subpaths.push(Subpath {
                    start,
                    segs,
                    closed: sp.closed,
                });
            }

            let lowered_style = if let Some(st) = style {
                Some(lower_style(st, params)?)
            } else {
                None
            };

            let single = |e: Entity| vec![(None, e)];
            Ok((
                single(Entity::Path {
                    subpaths: built_subpaths,
                    is_construction: *construction,
                }),
                name.clone(),
                *construction,
                lowered_style,
                layer.clone(),
            ))
        }
        other => {
            let (items, name, construction) = expand_geom(other, params)?;
            Ok((items, name, construction, None, None))
        }
    }
}

/// Hasil `build_sketch`.
#[derive(Debug)]
pub(crate) struct SketchBuild {
    pub sketch: Sketch,
    pub dof: DofReport,
    pub closed_regions: usize,
    /// Nama → EntityId (juga tersimpan di `sketch.entity_names`).
    pub names: HashMap<String, EntityId>,
    /// Entitas yang namanya dibuat otomatis (`e<n>`) karena spesifikasinya
    /// tidak memberi `name` — nama eksplisit `"e5"` TIDAK termasuk.
    pub auto_named: BTreeSet<EntityId>,
}

/// Langkah 2–4 P1.3: bangun sketch, beri nama, terjemahkan constraint, solve.
pub(crate) fn build_sketch(
    entities: &[EntitySpec],
    constraints: &[ConstraintSpec],
    params: &Params,
) -> OpResult<SketchBuild> {
    let mut sketch = Sketch::default();
    let mut names: HashMap<String, EntityId> = HashMap::new();
    let mut auto_named = BTreeSet::new();
    for (i, spec) in entities.iter().enumerate() {
        let (items, name, construction, style, layer) = expand(spec, params)
            .map_err(|e| e.with_context(serde_json::json!({ "entity_index": i })))?;
        let is_auto = name.is_none();
        let base = name.unwrap_or_else(|| format!("e{}", i + 1));
        for (suffix, entity) in items {
            let full = match suffix {
                Some(s) => format!("{base}.{s}"),
                None => base.clone(),
            };
            if names.contains_key(&full) {
                return Err(OpError::new(
                    OpErrorCode::DuplicateId,
                    format!("nama entitas '{full}' dipakai lebih dari sekali dalam sketch"),
                ));
            }
            let id = sketch
                .entities
                .insert(entity.with_construction(construction));
            sketch.entity_names.insert(id, full.clone());
            names.insert(full, id);
            if is_auto {
                auto_named.insert(id);
            }

            if let Some(st) = &style {
                sketch.styles.insert(id, st.clone());
            }
            if let Some(layer_name) = &layer {
                let layer_id = if let Some((lid, _)) =
                    sketch.layers.iter().find(|(_, l)| l.name == *layer_name)
                {
                    lid
                } else {
                    let lid = sketch
                        .layers
                        .insert(Layer::new(layer_name.clone(), Rgba::WHITE));
                    sketch.layer_order.push(lid);
                    lid
                };
                sketch.entity_layer.insert(id, layer_id);
            }
        }
    }

    let constraints = constraints
        .iter()
        .map(|c| translate_constraint(c, &names, &sketch, params))
        .collect::<OpResult<Vec<_>>>()?;
    let (solved, _, dof) = crate::compute::solve_with(&sketch, &constraints)?;
    let closed_regions = ducad_sketch::find_closed_regions(&solved).len();
    Ok(SketchBuild {
        sketch: solved,
        dof,
        closed_regions,
        names,
        auto_named,
    })
}

fn unknown_name(name: &str, names: &HashMap<String, EntityId>) -> OpError {
    let known: BTreeSet<&str> = names.keys().map(String::as_str).collect();
    OpError::new(
        OpErrorCode::UnknownRef,
        format!("entitas '{name}' tidak dikenal dalam sketch (yang ada: {known:?})"),
    )
    .with_context(serde_json::json!({ "name": name, "available": known }))
}

fn entity(name: &str, names: &HashMap<String, EntityId>) -> OpResult<EntityId> {
    names
        .get(name)
        .copied()
        .ok_or_else(|| unknown_name(name, names))
}

/// `"<nama>.start|.end|.center"` atau `"<nama>.s<subpath>.n<node>"` (untuk path).
fn point(r: &str, names: &HashMap<String, EntityId>, sketch: &Sketch) -> OpResult<PointRef> {
    if let Some((rest, n_str)) = r.rsplit_once('.') {
        if let Some(n_digits) = n_str.strip_prefix('n') {
            if let Ok(node) = n_digits.parse::<u32>() {
                if let Some((base, s_str)) = rest.rsplit_once('.') {
                    if let Some(s_digits) = s_str.strip_prefix('s') {
                        if let Ok(sub) = s_digits.parse::<u16>() {
                            let id = entity(base, names)?;
                            if let Some(Entity::Path { subpaths, .. }) = sketch.entities.get(id) {
                                if let Some(sp) = subpaths.get(sub as usize) {
                                    if (node as usize) < sp.node_count() {
                                        return Ok(PointRef::PathNode { id, sub, node });
                                    } else {
                                        return Err(OpError::invalid(format!(
                                            "rujukan node index {node} melebihi jumlah node ({}) pada '{base}.s{sub}'",
                                            sp.node_count()
                                        )));
                                    }
                                } else {
                                    return Err(OpError::invalid(format!(
                                        "rujukan subpath index {sub} melebihi jumlah subpath ({}) pada '{base}'",
                                        subpaths.len()
                                    )));
                                }
                            } else {
                                return Err(OpError::invalid(format!(
                                    "entitas '{base}' bukan sebuah Path"
                                )));
                            }
                        }
                    }
                }
            }
        }
    }

    let Some((base, suffix)) = r.rsplit_once('.') else {
        return Err(OpError::invalid(format!(
            "rujukan titik '{r}' harus berakhiran .start, .end, .center, atau .s<subpath>.n<node>"
        )));
    };
    let id = entity(base, names)?;
    let is_line = matches!(sketch.entities.get(id), Some(Entity::Line { .. }));
    match suffix {
        "start" if is_line => Ok(PointRef::LineStart(id)),
        "end" if is_line => Ok(PointRef::LineEnd(id)),
        "center" if !is_line => Ok(PointRef::Center(id)),
        _ => Err(OpError::invalid(format!(
            "rujukan titik '{r}' tidak berlaku: .start/.end hanya untuk garis, .center untuk lingkaran/busur/ellips"
        ))),
    }
}

fn translate_constraint(
    c: &ConstraintSpec,
    names: &HashMap<String, EntityId>,
    sketch: &Sketch,
    params: &Params,
) -> OpResult<Constraint> {
    let e = |n: &str| entity(n, names);
    let p = |n: &str| point(n, names, sketch);
    let pair = |[a, b]: &[String; 2]| -> OpResult<(EntityId, EntityId)> { Ok((e(a)?, e(b)?)) };
    Ok(match c {
        ConstraintSpec::Horizontal(l) => Constraint::Horizontal { line: e(l)? },
        ConstraintSpec::Vertical(l) => Constraint::Vertical { line: e(l)? },
        ConstraintSpec::Parallel(ab) => {
            let (a, b) = pair(ab)?;
            Constraint::Parallel { a, b }
        }
        ConstraintSpec::Perpendicular(ab) => {
            let (a, b) = pair(ab)?;
            Constraint::Perpendicular { a, b }
        }
        ConstraintSpec::EqualLength(ab) => {
            let (a, b) = pair(ab)?;
            Constraint::EqualLength { a, b }
        }
        ConstraintSpec::EqualRadius(ab) => {
            let (a, b) = pair(ab)?;
            Constraint::EqualRadius { a, b }
        }
        ConstraintSpec::Coincident([a, b]) => Constraint::Coincident { a: p(a)?, b: p(b)? },
        ConstraintSpec::Fixed { point: pt, at } => Constraint::Fixed {
            point: p(pt)?,
            target: p2(at, params)?,
        },
        ConstraintSpec::Distance { a, b, value } => Constraint::Distance {
            a: p(a)?,
            b: p(b)?,
            value: positive("jarak constraint", eval(value, params)?)?,
        },
        ConstraintSpec::Radius { entity: ent, value } => Constraint::Radius {
            entity: e(ent)?,
            value: positive("radius constraint", eval(value, params)?)?,
        },
        // `Constraint::Angle.value` dalam RADIAN — diverifikasi di
        // `ducad-sketch/src/constraint/solver.rs` (residual
        // `atan2(cross, dot) - value`). Kontrak JSON memakai derajat.
        ConstraintSpec::Angle { a, b, deg } => Constraint::Angle {
            a: e(a)?,
            b: e(b)?,
            value: eval(deg, params)?.to_radians(),
        },
        ConstraintSpec::Tangent(ab) => {
            let (a, b) = pair(ab)?;
            Constraint::Tangent { a, b }
        }
        ConstraintSpec::Concentric(ab) => {
            let (a, b) = pair(ab)?;
            Constraint::Concentric { a, b }
        }
        ConstraintSpec::Collinear(ab) => {
            let (a, b) = pair(ab)?;
            Constraint::Collinear { a, b }
        }
        ConstraintSpec::Midpoint { point: pt, line } => Constraint::Midpoint {
            point: p(pt)?,
            line: e(line)?,
        },
        ConstraintSpec::PointOnCurve { point: pt, curve } => Constraint::PointOnCurve {
            point: p(pt)?,
            curve: e(curve)?,
        },
        ConstraintSpec::Symmetric { a, b, axis } => Constraint::Symmetric {
            a: p(a)?,
            b: p(b)?,
            axis: e(axis)?,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(kv: &[(&str, f64)]) -> Params {
        kv.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn specs(json: &str) -> (Vec<EntitySpec>, Vec<ConstraintSpec>) {
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        (
            serde_json::from_value(v["entities"].clone()).unwrap(),
            serde_json::from_value(
                v.get("constraints")
                    .cloned()
                    .unwrap_or(serde_json::json!([])),
            )
            .unwrap(),
        )
    }

    #[test]
    fn rect_and_circle_make_one_region_with_hole() {
        let (e, c) = specs(
            r#"{"entities":[{"rect":{"center":[0,0],"w":"$w","h":40,"name":"outline"}},
                            {"circle":{"center":[0,0],"r":5,"name":"hole"}}]}"#,
        );
        let b = build_sketch(&e, &c, &params(&[("w", 60.0)])).unwrap();
        assert_eq!(b.sketch.entities.len(), 5);
        assert_eq!(b.closed_regions, 2);
        let tree = ducad_sketch::find_region_hierarchy(&b.sketch);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].holes.len(), 1);
        let top = b.names["outline.top"];
        match b.sketch.entities[top] {
            Entity::Line { start, end, .. } => {
                assert_eq!(start, DVec2::new(30.0, 20.0));
                assert_eq!(end, DVec2::new(-30.0, 20.0));
            }
            _ => panic!("outline.top harus garis"),
        }
        assert_eq!(b.sketch.entity_names[&top], "outline.top");
    }

    #[test]
    fn distance_constraint_changes_line_length() {
        let (e, c) = specs(
            r#"{"entities":[{"line":{"from":[0,0],"to":[10,0],"name":"l1"}}],
                "constraints":[{"horizontal":"l1"},
                               {"fixed":{"point":"l1.start","at":[0,0]}},
                               {"distance":{"a":"l1.start","b":"l1.end","value":50}}]}"#,
        );
        let b = build_sketch(&e, &c, &Params::new()).unwrap();
        match b.sketch.entities[b.names["l1"]] {
            Entity::Line { start, end, .. } => {
                assert!(((end - start).length() - 50.0).abs() < 1e-6)
            }
            _ => unreachable!(),
        }
        assert_eq!(b.sketch.constraints.len(), 3);
        assert_eq!(b.closed_regions, 0);
    }

    #[test]
    fn duplicate_names_and_unknown_refs() {
        let (e, c) = specs(
            r#"{"entities":[{"circle":{"center":[0,0],"r":1,"name":"a"}},{"circle":{"center":[5,0],"r":1,"name":"a"}}]}"#,
        );
        assert_eq!(
            build_sketch(&e, &c, &Params::new()).err().unwrap().code,
            OpErrorCode::DuplicateId
        );
        let (e, c) = specs(
            r#"{"entities":[{"line":{"from":[0,0],"to":[1,0],"name":"l1"}}],"constraints":[{"vertical":"l9"}]}"#,
        );
        let err = build_sketch(&e, &c, &Params::new()).err().unwrap();
        assert_eq!(err.code, OpErrorCode::UnknownRef);
        assert!(
            err.message.contains("l9") && err.message.contains("l1"),
            "{}",
            err.message
        );
    }

    #[test]
    fn point_suffix_parsed_from_right() {
        let (e, c) = specs(
            r#"{"entities":[{"rect":{"corner":[0,0],"w":10,"h":5,"name":"outline"}}],
                "constraints":[{"fixed":{"point":"outline.top.end","at":[0,5]}}]}"#,
        );
        assert!(build_sketch(&e, &c, &Params::new()).is_ok());
    }

    #[test]
    fn unnamed_entities_get_sequential_names_and_rect_requires_one_anchor() {
        let (e, c) = specs(
            r#"{"entities":[{"circle":{"center":[0,0],"r":1}},{"polygon":{"center":[9,0],"r":2,"sides":6}}]}"#,
        );
        let b = build_sketch(&e, &c, &Params::new()).unwrap();
        assert!(b.names.contains_key("e1"));
        assert!(b.names.contains_key("e2.5"));
        let (e, c) = specs(r#"{"entities":[{"rect":{"w":1,"h":1}}]}"#);
        assert_eq!(
            build_sketch(&e, &c, &Params::new()).err().unwrap().code,
            OpErrorCode::InvalidParam
        );
        let (e, c) =
            specs(r#"{"entities":[{"rect":{"center":[0,0],"corner":[0,0],"w":1,"h":1}}]}"#);
        assert_eq!(
            build_sketch(&e, &c, &Params::new()).err().unwrap().code,
            OpErrorCode::InvalidParam
        );
        let (e, c) = specs(r#"{"entities":[{"circle":{"center":[0,0],"r":0}}]}"#);
        assert_eq!(
            build_sketch(&e, &c, &Params::new()).err().unwrap().code,
            OpErrorCode::InvalidParam
        );
    }

    #[test]
    fn on_face_plane_sits_on_top_of_box() {
        let shape = crate::compute::primitive(
            &crate::compute::PrimitiveShape::Box {
                size: [60.0, 40.0, 8.0],
                centered: false,
            },
            [0.0; 3],
        )
        .unwrap()
        .shape;
        let lookup = |name: &str| -> OpResult<KernelShape> {
            assert_eq!(name, "plate");
            ducad_kernel::clone_shape(&shape).map_err(|e| OpError::kernel("clone", e))
        };
        let spec = PlaneSpec::OnFace {
            body: "plate".into(),
            face: ">Z".into(),
        };
        let r = resolve_plane(&spec, &Params::new(), &lookup).unwrap();
        let f = r.frame();
        assert!(matches!(r, ResolvedPlane::Datum(_)));
        assert!((f.origin[2] - 8.0).abs() < 1e-9);
        assert!((f.normal[2] - 1.0).abs() < 1e-9);
        let spec = PlaneSpec::OnFace {
            body: "plate".into(),
            face: "#Z".into(),
        };
        assert_eq!(
            resolve_plane(&spec, &Params::new(), &lookup)
                .unwrap_err()
                .code,
            OpErrorCode::InvalidParam
        );
        let spec = PlaneSpec::Offset {
            base: "front".into(),
            offset: Num::Value(5.0),
        };
        assert_eq!(
            resolve_plane(&spec, &Params::new(), &lookup)
                .unwrap()
                .frame()
                .origin,
            [0.0, -5.0, 0.0]
        );
    }

    #[test]
    fn path_spec_lowers_to_entity_with_style() {
        let json = r##"{
            "entities": [{
                "path": {
                    "name": "logo",
                    "subpaths": [{
                        "start": [0, 0],
                        "closed": true,
                        "segs": [
                            {"line": {"to": [40, 0]}},
                            {"cubic": {"c1": [50, 0], "c2": [50, 20], "to": [40, 20]}},
                            {"line": {"to": [0, 20]}}
                        ]
                    }],
                    "style": {
                        "fill": "#ff8800",
                        "stroke": "#000000",
                        "stroke_width": 0.5,
                        "opacity": 1.0,
                        "fill_rule": "nonzero"
                    },
                    "layer": "Layer 1",
                    "construction": false
                }
            }]
        }"##;
        let (entities, constraints) = specs(json);
        let build = build_sketch(&entities, &constraints, &Params::new()).unwrap();
        assert!(build.names.contains_key("logo"));
        let id = build.names["logo"];
        match &build.sketch.entities[id] {
            Entity::Path {
                subpaths,
                is_construction,
            } => {
                assert!(!is_construction);
                assert_eq!(subpaths.len(), 1);
                assert_eq!(subpaths[0].start, DVec2::new(0.0, 0.0));
                assert!(subpaths[0].closed);
                assert_eq!(subpaths[0].segs.len(), 3);
            }
            _ => panic!("Expected Entity::Path"),
        }
        let style = build
            .sketch
            .styles
            .get(id)
            .expect("style should be present");
        assert_eq!(
            style.fill,
            Some(Paint::Solid(Rgba::from_hex("#ff8800").unwrap()))
        );
        assert_eq!(style.fill_rule, FillRule::NonZero);
        let stroke = style.stroke.as_ref().expect("stroke should be present");
        assert_eq!(stroke.paint, Paint::Solid(Rgba::BLACK));
        assert_eq!(stroke.width_mm, 0.5);
        assert_eq!(style.opacity, 1.0);
    }

    #[test]
    fn invalid_hex_is_invalid_param() {
        let json = r#"{
            "entities": [{
                "path": {
                    "subpaths": [{
                        "start": [0, 0],
                        "closed": true,
                        "segs": [{"line": {"to": [10, 0]}}]
                    }],
                    "style": {
                        "fill": "not-a-hex"
                    }
                }
            }]
        }"#;
        let (entities, constraints) = specs(json);
        let err = build_sketch(&entities, &constraints, &Params::new()).unwrap_err();
        assert_eq!(err.code, OpErrorCode::InvalidParam);
        assert!(err.message.contains("style.fill: warna hex tidak valid"));

        // Test stroke_width <= 0
        let json2 = r##"{
            "entities": [{
                "path": {
                    "subpaths": [{
                        "start": [0, 0],
                        "closed": true,
                        "segs": [{"line": {"to": [10, 0]}}]
                    }],
                    "style": {
                        "stroke": "#000000",
                        "stroke_width": 0.0
                    }
                }
            }]
        }"##;
        let (entities2, constraints2) = specs(json2);
        let err2 = build_sketch(&entities2, &constraints2, &Params::new()).unwrap_err();
        assert_eq!(err2.code, OpErrorCode::InvalidParam);

        // Test subpath kosong
        let json3 = r#"{
            "entities": [{
                "path": {
                    "subpaths": []
                }
            }]
        }"#;
        let (entities3, constraints3) = specs(json3);
        let err3 = build_sketch(&entities3, &constraints3, &Params::new()).unwrap_err();
        assert_eq!(err3.code, OpErrorCode::InvalidParam);
    }

    #[test]
    fn unknown_layer_is_created() {
        let json = r#"{
            "entities": [{
                "path": {
                    "subpaths": [{
                        "start": [0, 0],
                        "closed": true,
                        "segs": [{"line": {"to": [10, 0]}}]
                    }],
                    "layer": "BrandNewLayer"
                }
            }]
        }"#;
        let (entities, constraints) = specs(json);
        let build = build_sketch(&entities, &constraints, &Params::new()).unwrap();
        let (layer_id, layer) = build
            .sketch
            .layers
            .iter()
            .find(|(_, l)| l.name == "BrandNewLayer")
            .expect("layer should exist");
        assert_eq!(layer.kind, ducad_sketch::layer::LayerKind::Vector);
        assert!(build.sketch.layer_order.contains(&layer_id));
        let ent_id = build.sketch.entities.keys().next().unwrap();
        assert_eq!(build.sketch.entity_layer.get(ent_id), Some(&layer_id));
    }

    #[test]
    fn constraint_on_path_node_by_name() {
        let json = r#"{
            "entities": [
                {
                    "path": {
                        "name": "logo",
                        "subpaths": [{
                            "start": [0, 0],
                            "closed": false,
                            "segs": [
                                {"line": {"to": [10, 0]}},
                                {"line": {"to": [10, 10]}}
                            ]
                        }]
                    }
                },
                {
                    "line": {
                        "from": [10, 10],
                        "to": [20, 20],
                        "name": "l1"
                    }
                }
            ],
            "constraints": [
                {"coincident": ["logo.s0.n2", "l1.start"]}
            ]
        }"#;
        let (entities, constraints) = specs(json);
        let build = build_sketch(&entities, &constraints, &Params::new()).unwrap();
        assert_eq!(build.sketch.constraints.len(), 1);
        match &build.sketch.constraints[0] {
            Constraint::Coincident { a, b } => {
                let logo_id = build.names["logo"];
                let l1_id = build.names["l1"];
                assert_eq!(
                    *a,
                    PointRef::PathNode {
                        id: logo_id,
                        sub: 0,
                        node: 2
                    }
                );
                assert_eq!(*b, PointRef::LineStart(l1_id));
            }
            _ => panic!("Expected Constraint::Coincident"),
        }
    }
}
