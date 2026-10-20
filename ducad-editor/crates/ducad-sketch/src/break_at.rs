//! Break (Paritas 1.13): pecah satu entitas sketsa di titik klik menjadi dua
//! entitas, dengan constraint di ujung-ujungnya tetap menempel ke potongan
//! yang benar.
//!
//! Strategi id: potongan PERTAMA memakai ulang `EntityId` asli (titik awal,
//! pusat, dan constraint level-entitas seperti Radius/Horizontal tetap sah
//! tanpa dipetakan ulang), potongan KEDUA mendapat id baru dan mengambil alih
//! rujukan `LineEnd` milik entitas asli. Sambungan di titik pecah dikunci
//! dengan `Coincident`, ditambah `Collinear` (garis) atau
//! `Concentric` + `EqualRadius` (busur) supaya kedua potongan tetap berperilaku
//! seperti satu kurva saat solver bergerak.

use ducad_core::Command;
use glam::DVec2;
use std::f64::consts::{PI, TAU};

use crate::constraint::{Constraint, PointRef};
use crate::entity::{sample_catmull_rom, Entity, EntityId};
use crate::sketch::Sketch;

/// Jarak minimum titik pecah dari ujung entitas (mm). Lebih dekat dari ini
/// berarti "klik di ujung" — tidak ada yang perlu dipecah.
pub const BREAK_END_EPS: f64 = 1e-6;

/// Normalisasi sudut ke `[0, 2π)`.
fn norm_angle(a: f64) -> f64 {
    let r = a.rem_euclid(TAU);
    if r.is_finite() {
        r
    } else {
        0.0
    }
}

/// Pecah `entity` di titik terdekat ke `p`. Mengembalikan `(awal, akhir)`:
/// potongan awal berisi ujung `start` entitas asli, potongan akhir berisi
/// ujung `end`. `None` bila tipe entitas tidak didukung (Ellipse, Path,
/// Point), titik tidak hingga/NaN, atau titik jatuh tepat di ujung.
///
/// - Line → dua Line.
/// - Arc → dua Arc (sudut dipecah di arah klik).
/// - Circle → dua Arc setengah lingkaran: dipecah di arah klik dan di titik
///   seberangnya (satu klik tidak cukup menentukan busur yang bermakna).
/// - Spline → dua Spline yang berbagi titik pecah; bentuk eksak (`exact`,
///   mis. dari glyph font) dilepas karena tidak lagi sesuai dengan titiknya.
pub fn break_entity(entity: &Entity, p: DVec2) -> Option<(Entity, Entity)> {
    if !p.is_finite() {
        return None;
    }
    let construction = entity.is_construction();
    match entity {
        Entity::Line { start, end, .. } => {
            let d = *end - *start;
            let len2 = d.length_squared();
            if !(len2.is_finite() && len2 > 0.0) {
                return None;
            }
            let t = ((p - *start).dot(d) / len2).clamp(0.0, 1.0);
            let q = *start + d * t;
            let len = len2.sqrt();
            if t * len <= BREAK_END_EPS || (1.0 - t) * len <= BREAK_END_EPS {
                return None;
            }
            Some((
                Entity::line(*start, q).with_construction(construction),
                Entity::line(q, *end).with_construction(construction),
            ))
        }
        Entity::Arc {
            center,
            radius,
            start_angle,
            end_angle,
            ..
        } => {
            let v = p - *center;
            if v.length_squared() <= 0.0 || !radius.is_finite() || *radius <= 0.0 {
                return None;
            }
            let theta = v.y.atan2(v.x);
            let mut sweep = norm_angle(end_angle - start_angle);
            if sweep <= 0.0 {
                sweep = TAU;
            }
            let d = norm_angle(theta - start_angle);
            let ang_eps = BREAK_END_EPS / radius;
            if d <= ang_eps || d >= sweep - ang_eps {
                return None;
            }
            let mid = start_angle + d;
            Some((
                Entity::arc(*center, *radius, *start_angle, mid).with_construction(construction),
                Entity::arc(*center, *radius, mid, *end_angle).with_construction(construction),
            ))
        }
        Entity::Circle { center, radius, .. } => {
            let v = p - *center;
            if v.length_squared() <= 0.0 || !radius.is_finite() || *radius <= 0.0 {
                return None;
            }
            let theta = v.y.atan2(v.x);
            Some((
                Entity::arc(*center, *radius, theta, theta + PI).with_construction(construction),
                Entity::arc(*center, *radius, theta + PI, theta + TAU)
                    .with_construction(construction),
            ))
        }
        Entity::Spline { points, .. } => break_spline(points, p, construction),
        _ => None,
    }
}

/// Pecah spline Catmull-Rom: cari bentang (span) yang paling dekat ke `p`,
/// sisipkan titik terdekat pada kurva sebagai titik bersama dua potongan.
fn break_spline(points: &[DVec2], p: DVec2, construction: bool) -> Option<(Entity, Entity)> {
    const SAMPLES: usize = 16;
    if points.len() < 2 || points.iter().any(|q| !q.is_finite()) {
        return None;
    }
    // Titik kontrol interior yang diklik langsung → pecah tepat di sana.
    if let Some(k) = (1..points.len() - 1).find(|&k| (points[k] - p).length() <= 1e-3) {
        return Some((
            Entity::spline(points[..=k].to_vec()).with_construction(construction),
            Entity::spline(points[k..].to_vec()).with_construction(construction),
        ));
    }
    let samples = sample_catmull_rom(points, SAMPLES);
    // Untuk 2 titik, `sample_catmull_rom` hanya mengembalikan kedua ujung —
    // anggap satu bentang lurus.
    let per_span = if points.len() == 2 { 1 } else { SAMPLES };
    let mut best: Option<(f64, usize, DVec2)> = None;
    for (i, w) in samples.windows(2).enumerate() {
        let (a, b) = (w[0], w[1]);
        let d = b - a;
        let l2 = d.length_squared();
        let t = if l2 > 0.0 { ((p - a).dot(d) / l2).clamp(0.0, 1.0) } else { 0.0 };
        let q = a + d * t;
        let dist = (p - q).length();
        if best.is_none_or(|(bd, _, _)| dist < bd) {
            best = Some((dist, i, q));
        }
    }
    let (_, seg, q) = best?;
    let first = points.first().copied()?;
    let last = points.last().copied()?;
    if (q - first).length() <= BREAK_END_EPS || (q - last).length() <= BREAK_END_EPS {
        return None;
    }
    let span = (seg / per_span).min(points.len() - 2);
    let mut a: Vec<DVec2> = points[..=span].to_vec();
    a.push(q);
    let mut b = vec![q];
    b.extend_from_slice(&points[span + 1..]);
    Some((
        Entity::spline(a).with_construction(construction),
        Entity::spline(b).with_construction(construction),
    ))
}

/// Semua `PointRef` di dalam constraint (mutable). Varian tanpa rujukan titik
/// mengembalikan daftar kosong.
fn point_refs_mut(c: &mut Constraint) -> Vec<&mut PointRef> {
    match c {
        Constraint::Coincident { a, b }
        | Constraint::HorizontalPoints { a, b }
        | Constraint::VerticalPoints { a, b }
        | Constraint::Distance { a, b, .. }
        | Constraint::Symmetric { a, b, .. }
        | Constraint::Curvature { a, b } => vec![a, b],
        Constraint::Fixed { point, .. }
        | Constraint::PointOnCurve { point, .. }
        | Constraint::Midpoint { point, .. } => vec![point],
        _ => Vec::new(),
    }
}

/// Command Break sebagai satu langkah undo.
pub struct BreakEntity {
    id: EntityId,
    point: DVec2,
    original: Option<Entity>,
    new_id: Option<EntityId>,
    prior_constraints: Vec<Constraint>,
}

impl BreakEntity {
    pub fn new(id: EntityId, point: DVec2) -> Self {
        Self {
            id,
            point,
            original: None,
            new_id: None,
            prior_constraints: Vec::new(),
        }
    }

    /// Id potongan kedua hasil `apply` terakhir (`None` bila tidak terjadi apa-apa).
    pub fn new_id(&self) -> Option<EntityId> {
        self.new_id
    }
}

impl Command<Sketch> for BreakEntity {
    fn name(&self) -> &str {
        "Break"
    }

    fn apply(&mut self, sketch: &mut Sketch) {
        self.new_id = None;
        self.original = None;
        let Some(orig) = sketch.entities.get(self.id).cloned() else {
            return;
        };
        let Some((first, second)) = break_entity(&orig, self.point) else {
            return;
        };
        let was_circle = matches!(orig, Entity::Circle { .. });
        let is_line = matches!(orig, Entity::Line { .. });
        let is_round = matches!(orig, Entity::Arc { .. } | Entity::Circle { .. });

        self.prior_constraints = sketch.constraints.clone();
        if let Some(slot) = sketch.entities.get_mut(self.id) {
            *slot = first;
        }
        sketch.touch(self.id);
        let new_id = sketch.entities.insert(second);
        sketch.touch(new_id);

        // Metadata ikut potongan baru: nama grup, style, layer, grup, z-order.
        if let Some(name) = sketch.entity_names.get(&self.id).cloned() {
            sketch.entity_names.insert(new_id, name);
        }
        if let Some(style) = sketch.styles.get(self.id).cloned() {
            sketch.styles.insert(new_id, style);
        }
        if let Some(layer) = sketch.entity_layer.get(self.id).copied() {
            sketch.entity_layer.insert(new_id, layer);
        }
        if let Some(gid) = sketch.entity_group.get(self.id).copied() {
            sketch.entity_group.insert(new_id, gid);
            if let Some(g) = sketch.groups.get_mut(gid) {
                g.members.push(new_id);
            }
        }
        if let Some(pos) = sketch.z_order.iter().position(|&z| z == self.id) {
            sketch.z_order.insert(pos + 1, new_id);
        }
        if sketch.hidden_entities.contains(&self.id) {
            sketch.hidden_entities.insert(new_id);
        }

        // Ujung akhir entitas asli kini milik potongan kedua.
        let old = self.id;
        for c in &mut sketch.constraints {
            for pr in point_refs_mut(c) {
                if *pr == PointRef::LineEnd(old) {
                    *pr = PointRef::LineEnd(new_id);
                }
            }
        }

        // Kunci sambungan agar kedua potongan tetap satu kurva.
        sketch.constraints.push(Constraint::Coincident {
            a: PointRef::LineEnd(old),
            b: PointRef::LineStart(new_id),
        });
        if was_circle {
            sketch.constraints.push(Constraint::Coincident {
                a: PointRef::LineEnd(new_id),
                b: PointRef::LineStart(old),
            });
        }
        if is_line {
            sketch.constraints.push(Constraint::Collinear { a: old, b: new_id });
        }
        if is_round {
            sketch.constraints.push(Constraint::Concentric { a: old, b: new_id });
            sketch.constraints.push(Constraint::EqualRadius { a: old, b: new_id });
        }

        self.original = Some(orig);
        self.new_id = Some(new_id);
    }

    fn revert(&mut self, sketch: &mut Sketch) {
        let (Some(orig), Some(new_id)) = (self.original.take(), self.new_id.take()) else {
            return;
        };
        sketch.entities.remove(new_id);
        sketch.entity_names.remove(&new_id);
        sketch.styles.remove(new_id);
        sketch.entity_layer.remove(new_id);
        if let Some(gid) = sketch.entity_group.remove(new_id) {
            if let Some(g) = sketch.groups.get_mut(gid) {
                g.members.retain(|&m| m != new_id);
            }
        }
        sketch.z_order.retain(|&z| z != new_id);
        sketch.hidden_entities.remove(&new_id);
        sketch.touch(new_id);
        if let Some(slot) = sketch.entities.get_mut(self.id) {
            *slot = orig;
        }
        sketch.touch(self.id);
        sketch.constraints = std::mem::take(&mut self.prior_constraints);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraint::point_ref_position;

    fn ends(e: &Entity) -> (DVec2, DVec2) {
        // Line/Arc: [awal, akhir]; Spline: [awal, akhir, interior...].
        let pts = e.endpoints();
        (pts[0], pts[1])
    }

    #[test]
    fn line_breaks_at_projected_point() {
        let l = Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0));
        let (a, b) = break_entity(&l, DVec2::new(4.0, 3.0)).unwrap();
        assert_eq!(ends(&a), (DVec2::ZERO, DVec2::new(4.0, 0.0)));
        assert_eq!(ends(&b), (DVec2::new(4.0, 0.0), DVec2::new(10.0, 0.0)));
    }

    #[test]
    fn click_at_endpoint_or_nan_does_nothing() {
        let l = Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0));
        assert!(break_entity(&l, DVec2::new(-5.0, 0.0)).is_none());
        assert!(break_entity(&l, DVec2::new(f64::NAN, 0.0)).is_none());
        let deg = Entity::line(DVec2::ONE, DVec2::ONE);
        assert!(break_entity(&deg, DVec2::ONE).is_none());
        assert!(break_entity(&Entity::ellipse(DVec2::ZERO, 2.0, 1.0), DVec2::X).is_none());
    }

    #[test]
    fn arc_breaks_inside_sweep_only() {
        let arc = Entity::arc(DVec2::ZERO, 5.0, 0.0, PI);
        let (a, b) = break_entity(&arc, DVec2::new(0.0, 7.0)).unwrap();
        match (a, b) {
            (
                Entity::Arc { start_angle: s1, end_angle: e1, .. },
                Entity::Arc { start_angle: s2, end_angle: e2, .. },
            ) => {
                assert!((s1 - 0.0).abs() < 1e-12 && (e1 - PI / 2.0).abs() < 1e-12);
                assert!((s2 - PI / 2.0).abs() < 1e-12 && (e2 - PI).abs() < 1e-12);
            }
            other => panic!("bukan dua busur: {other:?}"),
        }
        // Di luar sapuan busur → tidak dipecah.
        assert!(break_entity(&arc, DVec2::new(0.0, -5.0)).is_none());
    }

    #[test]
    fn circle_becomes_two_half_arcs() {
        let c = Entity::circle(DVec2::new(1.0, 1.0), 2.0);
        let (a, b) = break_entity(&c, DVec2::new(5.0, 1.0)).unwrap();
        let (a0, a1) = ends(&a);
        let (b0, b1) = ends(&b);
        assert!((a0 - DVec2::new(3.0, 1.0)).length() < 1e-9);
        assert!((a1 - b0).length() < 1e-9);
        assert!((b1 - a0).length() < 1e-9);
    }

    #[test]
    fn spline_breaks_into_two_sharing_a_point() {
        let s = Entity::spline(vec![DVec2::ZERO, DVec2::new(5.0, 5.0), DVec2::new(10.0, 0.0)]);
        let (a, b) = break_entity(&s, DVec2::new(2.0, 3.0)).unwrap();
        let (_, a1) = ends(&a);
        let (b0, b1) = ends(&b);
        assert!((a1 - b0).length() < 1e-12);
        assert_eq!(b1, DVec2::new(10.0, 0.0));
        // Klik di titik kontrol interior memecah tepat di sana.
        let (a, _) = break_entity(&s, DVec2::new(5.0, 5.0)).unwrap();
        assert_eq!(ends(&a).1, DVec2::new(5.0, 5.0));
    }

    #[test]
    fn command_remaps_end_constraint_and_undoes() {
        let mut sk = Sketch::default();
        let l = sk.entities.insert(Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0)));
        let other = sk.entities.insert(Entity::line(DVec2::new(10.0, 0.0), DVec2::new(10.0, 5.0)));
        sk.constraints.push(Constraint::Coincident {
            a: PointRef::LineEnd(l),
            b: PointRef::LineStart(other),
        });
        sk.constraints.push(Constraint::Horizontal { line: l });
        let mut cmd = BreakEntity::new(l, DVec2::new(3.0, 0.0));
        cmd.apply(&mut sk);
        let n = cmd.new_id().unwrap();
        assert_eq!(sk.entities.len(), 3);
        // Coincident ujung akhir kini menunjuk potongan kedua, posisinya sama.
        match &sk.constraints[0] {
            Constraint::Coincident { a, .. } => {
                assert_eq!(*a, PointRef::LineEnd(n));
                assert_eq!(point_ref_position(&sk, a), Some(DVec2::new(10.0, 0.0)));
            }
            other => panic!("{other:?}"),
        }
        // Horizontal tetap di id asli; sambungan dan collinear ditambahkan.
        assert!(matches!(sk.constraints[1], Constraint::Horizontal { line } if line == l));
        assert!(sk.constraints.iter().any(|c| matches!(c, Constraint::Collinear { .. })));
        assert_eq!(sk.constraints.len(), 4);

        cmd.revert(&mut sk);
        assert_eq!(sk.entities.len(), 2);
        assert_eq!(sk.constraints.len(), 2);
        assert_eq!(ends(&sk.entities[l]).1, DVec2::new(10.0, 0.0));
        assert!(matches!(&sk.constraints[0], Constraint::Coincident { a, .. } if *a == PointRef::LineEnd(l)));
    }

    #[test]
    fn command_on_missing_or_unsupported_entity_is_noop() {
        let mut sk = Sketch::default();
        let e = sk.entities.insert(Entity::ellipse(DVec2::ZERO, 3.0, 2.0));
        let mut cmd = BreakEntity::new(e, DVec2::new(3.0, 0.0));
        cmd.apply(&mut sk);
        assert!(cmd.new_id().is_none());
        cmd.revert(&mut sk);
        assert_eq!(sk.entities.len(), 1);
        sk.entities.remove(e);
        let mut cmd = BreakEntity::new(e, DVec2::ZERO);
        cmd.apply(&mut sk);
        assert!(sk.entities.is_empty());
    }
}
