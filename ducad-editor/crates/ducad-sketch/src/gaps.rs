//! Penutupan celah profil ("auto-close") untuk sketsa tangan.
//!
//! Coretan Pencil hampir tidak pernah bertemu tepat di satu titik: ujung
//! garis dan busur meleset 0,5–3 mm, sedangkan deteksi region dan profil
//! B-rep menuntut ujung berhimpit dalam 0,05 mm. Modul ini mencari ujung
//! yang menggantung, memasangkan yang berdekatan, lalu MENYUSUN rencana
//! perbaikan (geser ujung + constraint Coincident, atau garis jembatan).
//!
//! Murni: tidak mengubah sketch. Pemanggil GUI mengeksekusi rencana lewat
//! command agar menjadi satu langkah undo; [`apply_gap_fix`] tersedia untuk
//! jalur tanpa undo (tes, konversi di sketch sementara).

use std::collections::{HashMap, HashSet};

use glam::DVec2;

use crate::constraint::solver::solve;
use crate::constraint::types::{Constraint, PointRef};
use crate::entity::{Entity, EntityId};
use crate::sketch::Sketch;

/// Jarak di bawah ini dianggap sudah tersambung (sama dengan toleransi
/// rantai di `region::find_closed_regions`).
pub const JOIN_EPS: f64 = 0.05;

/// Ujung mana dari sebuah entitas terbuka.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum End {
    Start,
    End,
}

/// Satu ujung entitas terbuka.
#[derive(Debug, Clone, Copy)]
pub struct Endpoint {
    pub entity: EntityId,
    pub pos: DVec2,
    /// Rujukan titik untuk constraint; `None` untuk ujung busur (solver
    /// tidak punya `PointRef` ujung busur).
    pub point_ref: Option<PointRef>,
    /// Bisa digeser dengan mengubah geometri entitas (garis, spline, path).
    pub movable: bool,
    end: End,
    /// Indeks subpath untuk `Entity::Path`.
    sub: usize,
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
                    movable: true,
                    end: End::Start,
                    sub: 0,
                });
                out.push(Endpoint {
                    entity: id,
                    pos: *end,
                    point_ref: Some(PointRef::LineEnd(id)),
                    movable: true,
                    end: End::End,
                    sub: 0,
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
                for (pos, end) in [(pt(*start_angle), End::Start), (pt(*end_angle), End::End)] {
                    out.push(Endpoint {
                        entity: id,
                        pos,
                        point_ref: None,
                        movable: false,
                        end,
                        sub: 0,
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
                // Spline berkurva eksak (glyph) jangan diubah titiknya.
                let movable = matches!(e, Entity::Spline { exact: None, .. });
                out.push(Endpoint {
                    entity: id,
                    pos: *f,
                    point_ref: Some(PointRef::LineStart(id)),
                    movable,
                    end: End::Start,
                    sub: 0,
                });
                out.push(Endpoint {
                    entity: id,
                    pos: *l,
                    point_ref: Some(PointRef::LineEnd(id)),
                    movable,
                    end: End::End,
                    sub: 0,
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
                    for (pos, node, end) in
                        [(sub.start, 0usize, End::Start), (last, last_idx, End::End)]
                    {
                        out.push(Endpoint {
                            entity: id,
                            pos,
                            point_ref: Some(PointRef::PathNode {
                                id,
                                sub: si as u16,
                                node: node as u32,
                            }),
                            movable: true,
                            end,
                            sub: si,
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

/// Opsi penutupan celah.
#[derive(Debug, Clone, Copy)]
pub struct GapOptions {
    /// Celah terbesar (mm) yang masih ditutup.
    pub max_gap: f64,
    /// Celah sampai ukuran ini ditutup dengan MENGGESER ujung; di atasnya
    /// dijembatani garis baru agar bentuk yang digambar tidak terdistorsi.
    pub move_max: f64,
}

impl Default for GapOptions {
    fn default() -> Self {
        Self {
            max_gap: 3.0,
            move_max: 3.0,
        }
    }
}

/// Ujung garis jembatan yang harus diikat ke titik lain.
#[derive(Debug, Clone, Copy)]
pub struct BridgeLink {
    /// Indeks di [`GapFix::bridges`].
    pub bridge: usize,
    /// `true` = ujung akhir garis jembatan, `false` = ujung awal.
    pub at_end: bool,
    pub to: PointRef,
}

impl BridgeLink {
    /// Constraint Coincident setelah jembatan disisipkan dengan id `bridge_id`.
    pub fn constraint(&self, bridge_id: EntityId) -> Constraint {
        let a = if self.at_end {
            PointRef::LineEnd(bridge_id)
        } else {
            PointRef::LineStart(bridge_id)
        };
        Constraint::Coincident { a, b: self.to }
    }
}

/// Rencana penutupan celah.
#[derive(Debug, Clone, Default)]
pub struct GapFix {
    /// Entitas yang geometrinya diganti (satu entri per id).
    pub updates: Vec<(EntityId, Entity)>,
    /// Garis jembatan baru.
    pub bridges: Vec<Entity>,
    pub bridge_links: Vec<BridgeLink>,
    /// Constraint Coincident antar entitas yang SUDAH ada.
    pub constraints: Vec<Constraint>,
    /// Jumlah celah yang ditutup.
    pub closed: usize,
    /// Ujung menggantung yang tersisa (celah di atas `max_gap`) dalam cakupan.
    pub remaining: Vec<DVec2>,
    /// Celah terkecil yang tersisa, bila ada — untuk pesan ke pengguna.
    pub smallest_remaining_gap: Option<f64>,
}

impl GapFix {
    pub fn is_empty(&self) -> bool {
        self.closed == 0
    }
}

fn set_endpoint(entity: &mut Entity, ep: &Endpoint, pos: DVec2) {
    match entity {
        Entity::Line { start, end, .. } => match ep.end {
            End::Start => *start = pos,
            End::End => *end = pos,
        },
        Entity::Spline { points, .. } => {
            let slot = match ep.end {
                End::Start => points.first_mut(),
                End::End => points.last_mut(),
            };
            if let Some(p) = slot {
                *p = pos;
            }
        }
        Entity::Path { subpaths, .. } => {
            if let Some(sub) = subpaths.get_mut(ep.sub) {
                let idx = match ep.end {
                    End::Start => 0,
                    End::End => sub.node_count() - 1,
                };
                let _ = sub.set_node(idx, pos);
            }
        }
        _ => {}
    }
}

/// Susun rencana penutupan celah.
///
/// `scope`: bila `Some`, hanya pasangan yang salah satu ujungnya milik
/// entitas di dalam himpunan ini yang ditutup (mis. entitas coretan baru).
pub fn plan_gap_closure(
    sketch: &Sketch,
    scope: Option<&HashSet<EntityId>>,
    opt: &GapOptions,
) -> GapFix {
    let in_scope = |id: EntityId| scope.is_none_or(|s| s.contains(&id));
    let ends = dangling(&open_endpoints(sketch, JOIN_EPS), JOIN_EPS);

    // Kandidat pasangan, terurut dari celah terkecil (serakah, deterministik).
    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
    for i in 0..ends.len() {
        for j in i + 1..ends.len() {
            let (a, b) = (&ends[i], &ends[j]);
            if !(in_scope(a.entity) || in_scope(b.entity)) {
                continue;
            }
            let d = (a.pos - b.pos).length();
            if d > opt.max_gap {
                continue;
            }
            if a.entity == b.entity {
                // Hanya spline/subpath yang boleh menutup dirinya sendiri;
                // garis/busur yang ujungnya saling dekat itu degenerat.
                let self_closable = match sketch.entities.get(a.entity) {
                    Some(Entity::Spline { points, .. }) => points.len() >= 3,
                    Some(Entity::Path { .. }) => a.sub == b.sub,
                    _ => false,
                };
                if !self_closable {
                    continue;
                }
            }
            pairs.push((d, i, j));
        }
    }
    pairs.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)).then(x.2.cmp(&y.2)));

    let mut used = vec![false; ends.len()];
    let mut edited: HashMap<EntityId, Entity> = HashMap::new();
    let mut order: Vec<EntityId> = Vec::new();
    let mut fix = GapFix::default();

    let mut edit = |id: EntityId, ep: &Endpoint, pos: DVec2| {
        let Some(orig) = sketch.entities.get(id) else {
            return;
        };
        let e = edited.entry(id).or_insert_with(|| {
            order.push(id);
            orig.clone()
        });
        set_endpoint(e, ep, pos);
    };

    for (d, i, j) in pairs {
        if used[i] || used[j] {
            continue;
        }
        used[i] = true;
        used[j] = true;
        let (a, b) = (ends[i], ends[j]);
        fix.closed += 1;

        let bridge = d > opt.move_max || (!a.movable && !b.movable);
        if bridge {
            let idx = fix.bridges.len();
            fix.bridges.push(Entity::line(a.pos, b.pos));
            if let Some(r) = a.point_ref {
                fix.bridge_links.push(BridgeLink {
                    bridge: idx,
                    at_end: false,
                    to: r,
                });
            }
            if let Some(r) = b.point_ref {
                fix.bridge_links.push(BridgeLink {
                    bridge: idx,
                    at_end: true,
                    to: r,
                });
            }
            continue;
        }

        let target = match (a.movable, b.movable) {
            (true, true) if a.entity == b.entity => a.pos,
            (true, true) => (a.pos + b.pos) * 0.5,
            (true, false) => b.pos,
            _ => a.pos,
        };
        if a.movable {
            edit(a.entity, &a, target);
        }
        if b.movable {
            edit(b.entity, &b, target);
        }
        if let (Some(ra), Some(rb)) = (a.point_ref, b.point_ref) {
            fix.constraints
                .push(Constraint::Coincident { a: ra, b: rb });
        }
    }

    fix.updates = order
        .into_iter()
        .filter_map(|id| edited.remove(&id).map(|e| (id, e)))
        .collect();

    for (k, e) in ends.iter().enumerate() {
        if !used[k] && in_scope(e.entity) {
            fix.remaining.push(e.pos);
            let nearest = ends
                .iter()
                .enumerate()
                .filter(|(m, _)| *m != k)
                .map(|(_, q)| (q.pos - e.pos).length())
                .fold(f64::INFINITY, f64::min);
            if nearest.is_finite() {
                fix.smallest_remaining_gap = Some(
                    fix.smallest_remaining_gap
                        .map_or(nearest, |g| g.min(nearest)),
                );
            }
        }
    }
    fix
}

/// Terapkan rencana langsung ke `sketch` (tanpa undo) lalu solve.
/// Mengembalikan id garis jembatan yang disisipkan.
pub fn apply_gap_fix(sketch: &mut Sketch, fix: &GapFix) -> Vec<EntityId> {
    for (id, e) in &fix.updates {
        if let Some(slot) = sketch.entities.get_mut(*id) {
            *slot = e.clone();
            sketch.touch(*id);
        }
    }
    let bridge_ids: Vec<EntityId> = fix
        .bridges
        .iter()
        .map(|e| sketch.entities.insert(e.clone()))
        .collect();
    sketch.constraints.extend(fix.constraints.iter().cloned());
    for link in &fix.bridge_links {
        if let Some(id) = bridge_ids.get(link.bridge) {
            sketch.constraints.push(link.constraint(*id));
        }
    }
    if !sketch.constraints.is_empty() {
        let snapshot = sketch.constraints.clone();
        solve(sketch, &snapshot);
    }
    bridge_ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::find_closed_regions;

    fn ids(sketch: &mut Sketch, ents: Vec<Entity>) -> Vec<EntityId> {
        ents.into_iter()
            .map(|e| sketch.entities.insert(e))
            .collect()
    }

    /// Persegi 20×10 yang tiap sudutnya meleset `gap` mm.
    fn gappy_square(sketch: &mut Sketch, gap: f64) -> Vec<EntityId> {
        ids(
            sketch,
            vec![
                Entity::line(DVec2::new(0.0, 0.0), DVec2::new(20.0, 0.0)),
                Entity::line(DVec2::new(20.0 + gap, gap), DVec2::new(20.0, 10.0)),
                Entity::line(DVec2::new(20.0 - gap, 10.0 + gap), DVec2::new(0.0, 10.0)),
                Entity::line(DVec2::new(-gap, 10.0 - gap), DVec2::new(gap * 0.5, -gap)),
            ],
        )
    }

    #[test]
    fn gappy_square_becomes_one_region() {
        let mut s = Sketch::default();
        gappy_square(&mut s, 0.8);
        assert!(find_closed_regions(&s).is_empty(), "awalnya terbuka");

        let fix = plan_gap_closure(&s, None, &GapOptions::default());
        assert_eq!(fix.closed, 4);
        assert!(
            fix.bridges.is_empty(),
            "celah kecil digeser, bukan dijembatani"
        );
        assert_eq!(fix.constraints.len(), 4);
        assert!(fix.remaining.is_empty());

        apply_gap_fix(&mut s, &fix);
        let regions = find_closed_regions(&s);
        assert_eq!(regions.len(), 1);
        assert!(
            (regions[0].area - 200.0).abs() < 5.0,
            "luas {}",
            regions[0].area
        );
    }

    #[test]
    fn two_arcs_with_gap_are_bridged() {
        use std::f64::consts::PI;
        let mut s = Sketch::default();
        // Dua setengah lingkaran yang ujungnya meleset 1 mm.
        ids(
            &mut s,
            vec![
                Entity::arc(DVec2::ZERO, 10.0, 0.05, PI - 0.05),
                Entity::arc(DVec2::ZERO, 10.0, PI + 0.05, 2.0 * PI - 0.05),
            ],
        );
        assert!(find_closed_regions(&s).is_empty());
        let fix = plan_gap_closure(&s, None, &GapOptions::default());
        assert_eq!(fix.closed, 2);
        assert_eq!(fix.bridges.len(), 2, "busur tidak bisa digeser → jembatan");
        apply_gap_fix(&mut s, &fix);
        assert_eq!(find_closed_regions(&s).len(), 1);
    }

    #[test]
    fn open_spline_closes_itself() {
        let mut s = Sketch::default();
        ids(
            &mut s,
            vec![Entity::spline(vec![
                DVec2::new(0.0, 0.0),
                DVec2::new(10.0, -2.0),
                DVec2::new(14.0, 8.0),
                DVec2::new(3.0, 12.0),
                DVec2::new(0.9, 1.2),
            ])],
        );
        assert!(find_closed_regions(&s).is_empty());
        let fix = plan_gap_closure(&s, None, &GapOptions::default());
        assert_eq!(fix.closed, 1);
        apply_gap_fix(&mut s, &fix);
        assert_eq!(find_closed_regions(&s).len(), 1);
    }

    #[test]
    fn gap_above_limit_is_left_and_reported() {
        let mut s = Sketch::default();
        gappy_square(&mut s, 5.0);
        let fix = plan_gap_closure(&s, None, &GapOptions::default());
        assert_eq!(fix.closed, 0);
        assert!(fix.is_empty());
        assert_eq!(fix.remaining.len(), 8);
        assert!(fix.smallest_remaining_gap.is_some_and(|g| g > 3.0));
    }

    #[test]
    fn large_gap_is_bridged_when_move_is_limited() {
        let mut s = Sketch::default();
        gappy_square(&mut s, 2.0);
        let opt = GapOptions {
            max_gap: 6.0,
            move_max: 0.5,
        };
        let fix = plan_gap_closure(&s, None, &opt);
        assert_eq!(fix.closed, 4);
        assert_eq!(fix.bridges.len(), 4);
        assert!(fix.updates.is_empty(), "geometri asli tidak digeser");
        apply_gap_fix(&mut s, &fix);
        assert_eq!(find_closed_regions(&s).len(), 1);
    }

    #[test]
    fn scope_limits_which_gaps_close() {
        let mut s = Sketch::default();
        let old = ids(
            &mut s,
            vec![
                Entity::line(DVec2::new(100.0, 0.0), DVec2::new(110.0, 0.0)),
                Entity::line(DVec2::new(110.5, 0.5), DVec2::new(110.0, 10.0)),
            ],
        );
        let new = ids(
            &mut s,
            vec![Entity::line(DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0))],
        );
        let _ = old;
        let scope: HashSet<EntityId> = new.into_iter().collect();
        let fix = plan_gap_closure(&s, Some(&scope), &GapOptions::default());
        assert_eq!(fix.closed, 0, "celah antar entitas lama tidak disentuh");
    }

    #[test]
    fn dangling_reports_open_ends_only() {
        let mut s = Sketch::default();
        gappy_square(&mut s, 0.0);
        assert!(dangling_endpoints(&s, 1e-4).is_empty(), "persegi rapat");
        let mut s = Sketch::default();
        ids(
            &mut s,
            vec![Entity::line(DVec2::ZERO, DVec2::new(5.0, 0.0))],
        );
        assert_eq!(dangling_endpoints(&s, 1e-4).len(), 2);
    }
}
