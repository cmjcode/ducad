//! Ujung entitas terbuka dan deteksi ujung menggantung — petunjuk kenapa
//! sketch tidak membentuk loop tertutup (dipakai pesan error engine).
//! Penutupan celah sendiri dikerjakan `faces::build_closed_objects`.

use glam::DVec2;

use crate::constraint::types::PointRef;
use crate::entity::{Entity, EntityId};
use crate::sketch::Sketch;

/// Jarak di bawah ini dianggap sudah tersambung (sama dengan toleransi
/// rantai di `region::find_closed_regions`).
pub const JOIN_EPS: f64 = 0.05;

/// Satu ujung entitas terbuka.
#[derive(Debug, Clone, Copy)]
pub struct Endpoint {
    pub entity: EntityId,
    pub pos: DVec2,
    /// Rujukan titik untuk constraint; `None` untuk ujung busur (solver
    /// tidak punya `PointRef` ujung busur).
    pub point_ref: Option<PointRef>,
}

/// Ujung-ujung seluruh entitas terbuka yang terlihat dan bukan konstruksi.
/// Spline/subpath yang kedua ujungnya sudah berhimpit dalam `join_tol`
/// dianggap tertutup dan dilewati.
pub fn open_endpoints(sketch: &Sketch, join_tol: f64) -> Vec<Endpoint> {
    let mut out = Vec::new();
    for (id, e) in sketch.entities.iter() {
        if e.is_construction() || sketch.is_hidden(id) {
            continue;
        }
        match e {
            Entity::Line { start, end, .. } => {
                out.push(Endpoint {
                    entity: id,
                    pos: *start,
                    point_ref: Some(PointRef::LineStart(id)),
                });
                out.push(Endpoint {
                    entity: id,
                    pos: *end,
                    point_ref: Some(PointRef::LineEnd(id)),
                });
            }
            Entity::Arc {
                center,
                radius,
                start_angle,
                end_angle,
                ..
            } => {
                let pt = |a: f64| *center + DVec2::new(a.cos(), a.sin()) * *radius;
                for pos in [pt(*start_angle), pt(*end_angle)] {
                    out.push(Endpoint {
                        entity: id,
                        pos,
                        point_ref: None,
                    });
                }
            }
            Entity::Spline { points, .. } => {
                let (Some(f), Some(l)) = (points.first(), points.last()) else {
                    continue;
                };
                if points.len() < 2 || (*f - *l).length() <= join_tol {
                    continue;
                }
                out.push(Endpoint {
                    entity: id,
                    pos: *f,
                    point_ref: Some(PointRef::LineStart(id)),
                });
                out.push(Endpoint {
                    entity: id,
                    pos: *l,
                    point_ref: Some(PointRef::LineEnd(id)),
                });
            }
            Entity::Path { subpaths, .. } => {
                for (si, sub) in subpaths.iter().enumerate() {
                    if sub.closed || sub.segs.is_empty() {
                        continue;
                    }
                    let last_idx = sub.node_count() - 1;
                    let last = sub.node(last_idx).unwrap_or(sub.start);
                    if (sub.start - last).length() <= join_tol {
                        continue;
                    }
                    for (pos, node) in [(sub.start, 0usize), (last, last_idx)] {
                        out.push(Endpoint {
                            entity: id,
                            pos,
                            point_ref: Some(PointRef::PathNode {
                                id,
                                sub: si as u16,
                                node: node as u32,
                            }),
                        });
                    }
                }
            }
            Entity::Circle { .. } | Entity::Ellipse { .. } => {}
        }
    }
    out
}

/// Posisi ujung yang tidak punya pasangan dalam `tol` — petunjuk kenapa
/// sketch tidak membentuk loop tertutup.
pub fn dangling_endpoints(sketch: &Sketch, tol: f64) -> Vec<DVec2> {
    dangling(&open_endpoints(sketch, tol), tol)
        .into_iter()
        .map(|e| e.pos)
        .collect()
}

fn dangling(ends: &[Endpoint], tol: f64) -> Vec<Endpoint> {
    ends.iter()
        .enumerate()
        .filter(|(i, p)| {
            !ends
                .iter()
                .enumerate()
                .any(|(j, q)| j != *i && (p.pos - q.pos).length() <= tol)
        })
        .map(|(_, p)| *p)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dangling_reports_open_ends_only() {
        let mut s = Sketch::default();
        let c = [
            DVec2::new(0.0, 0.0),
            DVec2::new(20.0, 0.0),
            DVec2::new(20.0, 10.0),
            DVec2::new(0.0, 10.0),
        ];
        for i in 0..4 {
            s.entities.insert(Entity::line(c[i], c[(i + 1) % 4]));
        }
        assert!(dangling_endpoints(&s, 1e-4).is_empty(), "persegi rapat");
        let mut s = Sketch::default();
        s.entities
            .insert(Entity::line(DVec2::ZERO, DVec2::new(5.0, 0.0)));
        assert_eq!(dangling_endpoints(&s, 1e-4).len(), 2);
    }
}
