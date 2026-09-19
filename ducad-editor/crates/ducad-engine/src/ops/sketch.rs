//! Evaluasi `Op::Sketch` (P1.3): resolusi bidang, ekspansi `EntitySpec`,
//! penamaan entitas, terjemahan `ConstraintSpec`, dan solve.

use std::collections::{BTreeSet, HashMap};

use ducad_kernel::{KernelShape, SurfaceKind};
use ducad_sketch::constraint::{Constraint, DofReport, PointRef};
use ducad_sketch::{Entity, EntityId, PlaneRef, Sketch};
use glam::DVec2;

use super::num::{eval, eval_arr, Num, Params};
use super::spec::{ConstraintSpec, EntitySpec, PlaneSpec};
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
type Expanded = (Vec<(Option<String>, Entity)>, Option<String>, bool);

/// Satu `EntitySpec` → daftar `(sufiks nama anak, Entity)`. Sufiks `None`
/// berarti entitas tunggal yang memakai nama induk apa adanya.
fn expand(spec: &EntitySpec, params: &Params) -> OpResult<Expanded> {
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
    };
    Ok((items, name.clone(), construction))
}

/// Hasil `build_sketch`.
pub(crate) struct SketchBuild {
    pub sketch: Sketch,
    pub dof: DofReport,
    pub closed_regions: usize,
    /// Nama → EntityId (juga tersimpan di `sketch.entity_names`).
    pub names: HashMap<String, EntityId>,
}

/// Langkah 2–4 P1.3: bangun sketch, beri nama, terjemahkan constraint, solve.
pub(crate) fn build_sketch(
    entities: &[EntitySpec],
    constraints: &[ConstraintSpec],
    params: &Params,
) -> OpResult<SketchBuild> {
    let mut sketch = Sketch::default();
    let mut names: HashMap<String, EntityId> = HashMap::new();
    for (i, spec) in entities.iter().enumerate() {
        let (items, name, construction) = expand(spec, params)
            .map_err(|e| e.with_context(serde_json::json!({ "entity_index": i })))?;
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

/// `"<nama>.start|.end|.center"` — sufiks diuraikan dari KANAN karena nama
/// entitas sendiri bisa mengandung titik (`outline.top.end`).
fn point(r: &str, names: &HashMap<String, EntityId>, sketch: &Sketch) -> OpResult<PointRef> {
    let Some((base, suffix)) = r.rsplit_once('.') else {
        return Err(OpError::invalid(format!(
            "rujukan titik '{r}' harus berakhiran .start, .end, atau .center"
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
}
