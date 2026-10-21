use glam::DVec2;
use serde::{Deserialize, Serialize};

use crate::entity::{Entity, EntityId};
use crate::sketch::Sketch;

/// Rujukan ke satu titik pada entitas — dipakai constraint yang butuh
/// titik spesifik (Coincident, Fixed, Distance), bukan seluruh entitas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PointRef {
    /// Awal garis / Spline, atau titik di `start_angle` sebuah Arc.
    LineStart(EntityId),
    /// Akhir garis / Spline, atau titik di `end_angle` sebuah Arc.
    LineEnd(EntityId),
    /// Center Circle/Arc/Ellipse, atau posisi `Entity::Point`.
    Center(EntityId),
    /// Node ke-`node` pada subpath ke-`sub` dari `Entity::Path`.
    PathNode {
        id: EntityId,
        sub: u16,
        node: u32,
    },
}

impl PointRef {
    pub fn entity_id(&self) -> EntityId {
        match self {
            PointRef::LineStart(id) | PointRef::LineEnd(id) | PointRef::Center(id) => *id,
            PointRef::PathNode { id, .. } => *id,
        }
    }
}

/// Posisi `pr` saat ini di `sketch`.
pub fn point_ref_position(sketch: &Sketch, pr: &PointRef) -> Option<DVec2> {
    let entity = sketch.entities.get(pr.entity_id())?;
    match (entity, pr) {
        (Entity::Line { start, .. }, PointRef::LineStart(_)) => Some(*start),
        (Entity::Line { end, .. }, PointRef::LineEnd(_)) => Some(*end),
        // Ujung busur: LineStart = titik di `start_angle`, LineEnd = di `end_angle`.
        (
            Entity::Arc {
                center,
                radius,
                start_angle,
                end_angle,
                ..
            },
            PointRef::LineStart(_) | PointRef::LineEnd(_),
        ) => {
            let ang = if matches!(pr, PointRef::LineStart(_)) { *start_angle } else { *end_angle };
            Some(*center + DVec2::new(ang.cos(), ang.sin()) * *radius)
        }
        (
            Entity::Circle { center, .. }
            | Entity::Arc { center, .. }
            | Entity::Ellipse { center, .. },
            PointRef::Center(_),
        ) => Some(*center),
        // Titik sketch (Fase 1.1): posisinya dirujuk sebagai `Center`, dan
        // LineStart/LineEnd diterima juga agar alat yang memakai endpoint
        // generik tetap bekerja.
        (
            Entity::Point { pos, .. },
            PointRef::Center(_) | PointRef::LineStart(_) | PointRef::LineEnd(_),
        ) => Some(*pos),
        (Entity::Spline { points, .. }, PointRef::LineStart(_)) => points.first().copied(),
        (Entity::Spline { points, .. }, PointRef::LineEnd(_)) => points.last().copied(),
        (Entity::Spline { points, .. }, PointRef::Center(_)) => {
            if points.is_empty() {
                None
            } else {
                Some(points.iter().copied().sum::<DVec2>() / (points.len() as f64))
            }
        }
        (Entity::Path { subpaths, .. }, PointRef::PathNode { sub, node, .. }) => {
            subpaths.get(*sub as usize)?.node(*node as usize)
        }
        _ => None,
    }
}

/// Satu constraint geometris/dimensional.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Constraint {
    Coincident {
        a: PointRef,
        b: PointRef,
    },
    Horizontal {
        line: EntityId,
    },
    Vertical {
        line: EntityId,
    },
    HorizontalPoints {
        a: PointRef,
        b: PointRef,
    },
    VerticalPoints {
        a: PointRef,
        b: PointRef,
    },
    Parallel {
        a: EntityId,
        b: EntityId,
    },
    Perpendicular {
        a: EntityId,
        b: EntityId,
    },
    EqualLength {
        a: EntityId,
        b: EntityId,
    },
    EqualRadius {
        a: EntityId,
        b: EntityId,
    },
    Fixed {
        point: PointRef,
        target: DVec2,
    },
    /// `driven = true` (Fase 1.16): dimensi acuan/terukur — dilewati solver,
    /// `value` hanya diperbarui dari geometri (lihat [`refresh_driven_values`]).
    Distance {
        a: PointRef,
        b: PointRef,
        value: f64,
        #[serde(default, skip_serializing_if = "is_false")]
        driven: bool,
    },
    Radius {
        entity: EntityId,
        value: f64,
        #[serde(default, skip_serializing_if = "is_false")]
        driven: bool,
    },
    /// Sudut CCW dari arah `a` ke arah `b`, radian, kontinu di (-π, π].
    Angle {
        a: EntityId,
        b: EntityId,
        value: f64,
        #[serde(default, skip_serializing_if = "is_false")]
        driven: bool,
    },
    Tangent {
        a: EntityId,
        b: EntityId,
    },
    /// Titik `a` dan `b` saling cermin melintasi garis `axis`.
    Symmetric {
        a: PointRef,
        b: PointRef,
        axis: EntityId,
    },

    // ---- P1.2: constraint yang sebelumnya tidak ada ----
    /// Titik `point` menempel pada kurva `curve` (garis tak hingga untuk
    /// Line, lingkaran untuk Circle/Arc). Berbeda dari `Coincident` yang
    /// hanya mengikat titik-ke-TITIK, ini mengikat titik-ke-KURVA sehingga
    /// titiknya masih bebas meluncur sepanjang kurva — kebutuhan sehari-hari
    /// yang sebelumnya tidak bisa dinyatakan sama sekali.
    PointOnCurve {
        point: PointRef,
        curve: EntityId,
    },
    /// `point` berada tepat di tengah `line`.
    Midpoint {
        point: PointRef,
        line: EntityId,
    },
    /// Pusat dua entitas radial berimpit.
    Concentric {
        a: EntityId,
        b: EntityId,
    },
    /// Dua garis terletak pada satu garis lurus yang sama (sejajar DAN
    /// segaris) — lebih kuat dari `Parallel`.
    Collinear {
        a: EntityId,
        b: EntityId,
    },

    // ---- Fase 1.20 ----
    /// Kontinuitas kelengkungan (G2) antara ujung kurva `a` dan ujung kurva
    /// `b` (Spline–Spline atau Spline–Arc). Ujung dipilih lewat
    /// `PointRef::LineStart`/`LineEnd` pada entitas Spline/Arc. Residual:
    /// kedua ujung berimpit (G0), arah tangen menyambung mulus (G1), dan
    /// kelengkungan bertanda sama (G2) — kurva `a` "masuk" ke sambungan,
    /// kurva `b` "keluar" darinya.
    Curvature {
        a: PointRef,
        b: PointRef,
    },
}

/// Helper serde: lewati field `driven` bila `false` agar berkas lama
/// tetap identik byte-per-byte saat disimpan ulang.
fn is_false(v: &bool) -> bool {
    !*v
}

impl PointRef {
    fn map_entity_id(&mut self, f: &impl Fn(EntityId) -> EntityId) {
        match self {
            PointRef::LineStart(id) | PointRef::LineEnd(id) | PointRef::Center(id) => *id = f(*id),
            PointRef::PathNode { id, .. } => *id = f(*id),
        }
    }
}

impl Constraint {
    /// `true` bila ini dimensi acuan (driven) — tidak ikut diselesaikan solver.
    pub fn is_driven(&self) -> bool {
        matches!(
            self,
            Constraint::Distance { driven: true, .. }
                | Constraint::Radius { driven: true, .. }
                | Constraint::Angle { driven: true, .. }
        )
    }

    /// Apakah constraint ini dimensi (punya nilai & bisa driven).
    pub fn is_dimension(&self) -> bool {
        matches!(
            self,
            Constraint::Distance { .. } | Constraint::Radius { .. } | Constraint::Angle { .. }
        )
    }

    /// Ubah status driven; `false` bila constraint ini bukan dimensi.
    pub fn set_driven(&mut self, value: bool) -> bool {
        match self {
            Constraint::Distance { driven, .. }
            | Constraint::Radius { driven, .. }
            | Constraint::Angle { driven, .. } => {
                *driven = value;
                true
            }
            _ => false,
        }
    }

    /// Ganti setiap `EntityId` yang dirujuk lewat `f` — dipakai saat undo
    /// memulihkan entitas terhapus dengan id baru.
    pub fn map_entity_ids(&mut self, f: impl Fn(EntityId) -> EntityId) {
        match self {
            Constraint::Coincident { a, b }
            | Constraint::Distance { a, b, .. }
            | Constraint::HorizontalPoints { a, b }
            | Constraint::VerticalPoints { a, b } => {
                a.map_entity_id(&f);
                b.map_entity_id(&f);
            }
            Constraint::Horizontal { line } | Constraint::Vertical { line } => *line = f(*line),
            Constraint::Parallel { a, b }
            | Constraint::Perpendicular { a, b }
            | Constraint::EqualLength { a, b }
            | Constraint::EqualRadius { a, b }
            | Constraint::Angle { a, b, .. }
            | Constraint::Tangent { a, b }
            | Constraint::Concentric { a, b }
            | Constraint::Collinear { a, b } => {
                *a = f(*a);
                *b = f(*b);
            }
            Constraint::Curvature { a, b } => {
                a.map_entity_id(&f);
                b.map_entity_id(&f);
            }
            Constraint::Fixed { point, .. } => point.map_entity_id(&f),
            Constraint::Radius { entity, .. } => *entity = f(*entity),
            Constraint::Symmetric { a, b, axis } => {
                a.map_entity_id(&f);
                b.map_entity_id(&f);
                *axis = f(*axis);
            }
            Constraint::PointOnCurve { point, curve } => {
                point.map_entity_id(&f);
                *curve = f(*curve);
            }
            Constraint::Midpoint { point, line } => {
                point.map_entity_id(&f);
                *line = f(*line);
            }
        }
    }
}

/// Alamat stabil sebuah dimensi (jenis + rujukan, tanpa nilai) untuk
/// [`crate::Sketch::dim_exprs`]. `None` untuk constraint yang bukan dimensi.
pub fn dim_key(c: &Constraint) -> Option<String> {
    use slotmap::Key;
    let e = |id: &EntityId| format!("{:x}", id.data().as_ffi());
    let p = |r: &PointRef| match r {
        PointRef::LineStart(id) => format!("s{}", e(id)),
        PointRef::LineEnd(id) => format!("e{}", e(id)),
        PointRef::Center(id) => format!("c{}", e(id)),
        PointRef::PathNode { id, sub, node } => format!("n{}.{sub}.{node}", e(id)),
    };
    match c {
        Constraint::Distance { a, b, .. } => Some(format!("distance:{}:{}", p(a), p(b))),
        Constraint::Radius { entity, .. } => Some(format!("radius:{}", e(entity))),
        Constraint::Angle { a, b, .. } => Some(format!("angle:{}:{}", e(a), e(b))),
        _ => None,
    }
}
