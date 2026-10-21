//! Sketsa GUI → spesifikasi op `sketch` (ADR 0010, urutan migrasi 1).
//!
//! Kebalikan `build_sketch`: entitas dan constraint sketsa yang digambar di
//! GUI diterjemahkan ke `EntitySpec`/`ConstraintSpec` bernama, supaya sketsa
//! itu bisa disimpan di oplog dan di-replay. Geometri dipertahankan persis:
//! koordinat entitas yang sudah ter-solve menjadi titik awal, jadi constraint
//! yang tidak punya padanan spec (mis. `HorizontalPoints`) cukup dilewati —
//! bentuknya tidak berubah, hanya derajat kebebasannya bertambah. Jumlah yang
//! dilewati dilaporkan di [`SketchSpecs::skipped_constraints`].

use std::collections::HashMap;

use ducad_sketch::constraint::{Constraint, PointRef};
use ducad_sketch::entity::{Entity, EntityId, PathSeg};
use ducad_sketch::Sketch;
use glam::DVec2;

use super::spec::{ConstraintSpec, EntitySpec, SegSpec, SubpathSpec};
use super::Num;

/// Hasil penerjemahan satu sketsa.
#[derive(Debug, Clone, PartialEq)]
pub struct SketchSpecs {
    pub entities: Vec<EntitySpec>,
    pub constraints: Vec<ConstraintSpec>,
    /// Constraint GUI yang tidak punya padanan spec (geometri tetap sama).
    pub skipped_constraints: usize,
}

/// Nama spec entitas `id`: nama yang sudah ada di `sketch.entity_names`,
/// atau nama turunan id yang stabil selama entitas itu hidup.
pub fn entity_spec_name(sketch: &Sketch, id: EntityId) -> String {
    sketch
        .entity_names
        .get(&id)
        .cloned()
        .unwrap_or_else(|| format!("g{:x}", slotmap::Key::data(&id).as_ffi()))
}

fn p(v: DVec2) -> [Num; 2] {
    [Num::Value(v.x), Num::Value(v.y)]
}

fn segs(list: &[PathSeg]) -> Vec<SegSpec> {
    list.iter()
        .map(|s| match *s {
            PathSeg::Line { end } => SegSpec::Line { to: p(end) },
            PathSeg::Cubic { c1, c2, end } => SegSpec::Cubic {
                c1: p(c1),
                c2: p(c2),
                to: p(end),
            },
        })
        .collect()
}

fn entity_spec(e: &Entity, name: String) -> Option<EntitySpec> {
    let name = Some(name);
    Some(match e {
        Entity::Line {
            start,
            end,
            is_construction,
        } => EntitySpec::Line {
            from: p(*start),
            to: p(*end),
            name,
            construction: *is_construction,
        },
        Entity::Circle {
            center,
            radius,
            is_construction,
        } => EntitySpec::Circle {
            center: p(*center),
            r: Num::Value(*radius),
            name,
            construction: *is_construction,
        },
        Entity::Arc {
            center,
            radius,
            start_angle,
            end_angle,
            is_construction,
        } => EntitySpec::Arc {
            center: p(*center),
            r: Num::Value(*radius),
            start_deg: Num::Value(start_angle.to_degrees()),
            end_deg: Num::Value(end_angle.to_degrees()),
            name,
            construction: *is_construction,
        },
        Entity::Ellipse {
            center,
            radius_x,
            radius_y,
            is_construction,
        } => EntitySpec::Ellipse {
            center: p(*center),
            rx: Num::Value(*radius_x),
            ry: Num::Value(*radius_y),
            name,
            construction: *is_construction,
        },
        // Spline dengan kurva eksak (outline glyph) → path, agar dinding
        // extrude tetap mulus.
        Entity::Spline {
            points,
            exact: Some(exact),
            is_construction,
        } => EntitySpec::Path {
            subpaths: vec![SubpathSpec {
                start: p(*points.first()?),
                segs: segs(exact),
                closed: false,
            }],
            style: None,
            layer: None,
            name,
            construction: *is_construction,
        },
        Entity::Spline {
            points,
            exact: None,
            is_construction,
        } => {
            if points.len() < 3 {
                return None;
            }
            EntitySpec::Spline {
                points: points.iter().map(|v| p(*v)).collect(),
                name,
                construction: *is_construction,
            }
        }
        Entity::Path {
            subpaths,
            is_construction,
        } => EntitySpec::Path {
            subpaths: subpaths
                .iter()
                .map(|sp| SubpathSpec {
                    start: p(sp.start),
                    segs: segs(&sp.segs),
                    closed: sp.closed,
                })
                .collect(),
            style: None,
            layer: None,
            name,
            construction: *is_construction,
        },
        Entity::Point {
            pos,
            is_construction,
        } => EntitySpec::Point {
            at: p(*pos),
            name,
            construction: *is_construction,
        },
    })
}

/// Rujukan titik spec (`<nama>.start|.end|.center`, `<nama>.s<i>.n<j>`, atau
/// nama titik sketsa).
fn point_name(sketch: &Sketch, names: &HashMap<EntityId, String>, r: &PointRef) -> Option<String> {
    let base = names.get(&r.entity_id())?;
    let is_point = matches!(
        sketch.entities.get(r.entity_id()),
        Some(Entity::Point { .. })
    );
    Some(match r {
        _ if is_point => base.clone(),
        PointRef::LineStart(_) => format!("{base}.start"),
        PointRef::LineEnd(_) => format!("{base}.end"),
        PointRef::Center(_) => format!("{base}.center"),
        PointRef::PathNode { sub, node, .. } => format!("{base}.s{sub}.n{node}"),
    })
}

fn constraint_spec(
    sketch: &Sketch,
    names: &HashMap<EntityId, String>,
    c: &Constraint,
) -> Option<ConstraintSpec> {
    let e = |id: &EntityId| names.get(id).cloned();
    let pt = |r: &PointRef| point_name(sketch, names, r);
    Some(match c {
        Constraint::Coincident { a, b } => ConstraintSpec::Coincident([pt(a)?, pt(b)?]),
        Constraint::Horizontal { line } => ConstraintSpec::Horizontal(e(line)?),
        Constraint::Vertical { line } => ConstraintSpec::Vertical(e(line)?),
        Constraint::Parallel { a, b } => ConstraintSpec::Parallel([e(a)?, e(b)?]),
        Constraint::Perpendicular { a, b } => ConstraintSpec::Perpendicular([e(a)?, e(b)?]),
        Constraint::EqualLength { a, b } => ConstraintSpec::EqualLength([e(a)?, e(b)?]),
        Constraint::EqualRadius { a, b } => ConstraintSpec::EqualRadius([e(a)?, e(b)?]),
        Constraint::Fixed { point, target } => ConstraintSpec::Fixed {
            point: pt(point)?,
            at: p(*target),
        },
        Constraint::Distance {
            a,
            b,
            value,
            driven,
        } => ConstraintSpec::Distance {
            a: pt(a)?,
            b: pt(b)?,
            value: Num::Value(*value),
            driven: *driven,
        },
        Constraint::Radius {
            entity,
            value,
            driven,
        } => ConstraintSpec::Radius {
            entity: e(entity)?,
            value: Num::Value(*value),
            driven: *driven,
        },
        Constraint::Angle {
            a,
            b,
            value,
            driven,
        } => ConstraintSpec::Angle {
            a: e(a)?,
            b: e(b)?,
            deg: Num::Value(value.to_degrees()),
            driven: *driven,
        },
        Constraint::Tangent { a, b } => ConstraintSpec::Tangent([e(a)?, e(b)?]),
        Constraint::Concentric { a, b } => ConstraintSpec::Concentric([e(a)?, e(b)?]),
        Constraint::Collinear { a, b } => ConstraintSpec::Collinear([e(a)?, e(b)?]),
        Constraint::Midpoint { point, line } => ConstraintSpec::Midpoint {
            point: pt(point)?,
            line: e(line)?,
        },
        Constraint::PointOnCurve { point, curve } => ConstraintSpec::PointOnCurve {
            point: pt(point)?,
            curve: e(curve)?,
        },
        Constraint::Symmetric { a, b, axis } => ConstraintSpec::Symmetric {
            a: pt(a)?,
            b: pt(b)?,
            axis: e(axis)?,
        },
        Constraint::Curvature { a, b } => ConstraintSpec::Curvature {
            a: pt(a)?,
            b: pt(b)?,
        },
        Constraint::HorizontalPoints { .. } | Constraint::VerticalPoints { .. } => return None,
    })
}

/// Terjemahkan sketsa GUI ke spec op `sketch`. Entitas diurutkan menurut
/// urutan iterasi slotmap (stabil antar pemanggilan untuk sketsa yang sama).
pub fn sketch_to_specs(sketch: &Sketch) -> SketchSpecs {
    let mut names: HashMap<EntityId, String> = HashMap::new();
    let mut entities = Vec::new();
    for (id, e) in sketch.entities.iter() {
        let name = entity_spec_name(sketch, id);
        if let Some(spec) = entity_spec(e, name.clone()) {
            names.insert(id, name);
            entities.push(spec);
        }
    }
    let mut constraints = Vec::new();
    let mut skipped_constraints = 0;
    for c in sketch.constraints.iter() {
        match constraint_spec(sketch, &names, c) {
            Some(spec) => constraints.push(spec),
            None => skipped_constraints += 1,
        }
    }
    SketchSpecs {
        entities,
        constraints,
        skipped_constraints,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::sketch::build_sketch;
    use crate::ops::Params;

    fn roundtrip(sketch: &Sketch) -> Sketch {
        let specs = sketch_to_specs(sketch);
        build_sketch(&specs.entities, &specs.constraints, &Params::new())
            .expect("spec hasil terjemahan harus valid")
            .sketch
    }

    /// Bandingkan geometri per nama entitas.
    fn assert_same_geometry(a: &Sketch, b: &Sketch) {
        assert_eq!(a.entities.len(), b.entities.len());
        for (id, ea) in a.entities.iter() {
            let name = entity_spec_name(a, id);
            let (bid, _) = b
                .entity_names
                .iter()
                .find(|(_, n)| **n == name)
                .unwrap_or_else(|| panic!("entitas '{name}' hilang"));
            let eb = &b.entities[*bid];
            let near = |x: DVec2, y: DVec2| (x - y).length() < 1e-6;
            match (ea, eb) {
                (
                    Entity::Line { start, end, .. },
                    Entity::Line {
                        start: s2, end: e2, ..
                    },
                ) => assert!(near(*start, *s2) && near(*end, *e2), "{name}"),
                (
                    Entity::Circle { center, radius, .. },
                    Entity::Circle {
                        center: c2,
                        radius: r2,
                        ..
                    },
                ) => assert!(near(*center, *c2) && (radius - r2).abs() < 1e-6, "{name}"),
                (
                    Entity::Arc {
                        center,
                        radius,
                        start_angle,
                        end_angle,
                        ..
                    },
                    Entity::Arc {
                        center: c2,
                        radius: r2,
                        start_angle: s2,
                        end_angle: e2,
                        ..
                    },
                ) => {
                    assert!(near(*center, *c2) && (radius - r2).abs() < 1e-6, "{name}");
                    assert!((start_angle - s2).abs() < 1e-9 && (end_angle - e2).abs() < 1e-9);
                }
                (x, y) => assert_eq!(
                    std::mem::discriminant(x),
                    std::mem::discriminant(y),
                    "{name}"
                ),
            }
        }
    }

    fn l_profile() -> Sketch {
        let mut s = Sketch::default();
        let pts = [
            DVec2::new(0.0, 0.0),
            DVec2::new(40.0, 0.0),
            DVec2::new(40.0, 5.0),
            DVec2::new(5.0, 5.0),
            DVec2::new(5.0, 35.0),
            DVec2::new(0.0, 35.0),
        ];
        let ids: Vec<EntityId> = (0..6)
            .map(|i| s.entities.insert(Entity::line(pts[i], pts[(i + 1) % 6])))
            .collect();
        for i in 0..6 {
            s.constraints.push(Constraint::Coincident {
                a: PointRef::LineEnd(ids[i]),
                b: PointRef::LineStart(ids[(i + 1) % 6]),
            });
        }
        s.constraints.push(Constraint::Horizontal { line: ids[0] });
        s.constraints.push(Constraint::Vertical { line: ids[5] });
        s.constraints.push(Constraint::Fixed {
            point: PointRef::LineStart(ids[0]),
            target: DVec2::ZERO,
        });
        s.constraints.push(Constraint::Distance {
            a: PointRef::LineStart(ids[0]),
            b: PointRef::LineEnd(ids[0]),
            value: 40.0,
            driven: false,
        });
        s.constraints.push(Constraint::HorizontalPoints {
            a: PointRef::LineStart(ids[2]),
            b: PointRef::LineEnd(ids[2]),
        });
        s
    }

    #[test]
    fn l_profile_roundtrips_with_same_geometry_and_region() {
        let s = l_profile();
        let specs = sketch_to_specs(&s);
        assert_eq!(specs.entities.len(), 6);
        assert_eq!(
            specs.skipped_constraints, 1,
            "HorizontalPoints tidak punya padanan"
        );
        assert_eq!(specs.constraints.len(), s.constraints.len() - 1);
        let back = roundtrip(&s);
        assert_same_geometry(&s, &back);
        assert_eq!(ducad_sketch::find_closed_regions(&back).len(), 1);
    }

    #[test]
    fn named_entities_keep_their_names_and_arcs_keep_angles() {
        let mut s = Sketch::default();
        let c = s
            .entities
            .insert(Entity::circle(DVec2::new(10.0, 10.0), 4.0));
        s.entity_names.insert(c, "hole".into());
        s.entities.insert(Entity::Arc {
            center: DVec2::ZERO,
            radius: 7.0,
            start_angle: 0.3,
            end_angle: 2.1,
            is_construction: false,
        });
        let pt = s.entities.insert(Entity::point(DVec2::new(1.0, 2.0)));
        s.constraints.push(Constraint::Fixed {
            point: PointRef::Center(pt),
            target: DVec2::new(1.0, 2.0),
        });
        let specs = sketch_to_specs(&s);
        let json = serde_json::to_string(&specs.entities).unwrap();
        assert!(json.contains("\"hole\""), "{json}");
        let back = roundtrip(&s);
        assert_same_geometry(&s, &back);
    }

    #[test]
    fn exact_spline_becomes_path_and_construction_flag_survives() {
        let mut s = Sketch::default();
        s.entities.insert(Entity::Spline {
            points: vec![DVec2::ZERO, DVec2::new(5.0, 5.0), DVec2::new(10.0, 0.0)],
            exact: Some(vec![PathSeg::Cubic {
                c1: DVec2::new(3.0, 6.0),
                c2: DVec2::new(7.0, 6.0),
                end: DVec2::new(10.0, 0.0),
            }]),
            is_construction: false,
        });
        s.entities
            .insert(Entity::line(DVec2::ZERO, DVec2::new(0.0, 9.0)).with_construction(true));
        let specs = sketch_to_specs(&s);
        assert!(matches!(specs.entities[0], EntitySpec::Path { .. }));
        assert!(matches!(
            specs.entities[1],
            EntitySpec::Line {
                construction: true,
                ..
            }
        ));
        roundtrip(&s);
    }
}
