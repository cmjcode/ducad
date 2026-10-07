//! Dimensi asosiatif dan generator dimensi otomatis (P21.4).
//!
//! Dua bagian:
//!
//! 1. **Resolusi** — [`DrawingSheet::resolve_dimension`] menghitung posisi
//!    kertas (`start`/`end`/`line_pos`) dan teks sebuah dimensi dari sumber
//!    geometrinya ([`DimensionRef`]) + jarak `offset_mm`. Dipanggil ulang
//!    setiap kali geometri atau skala berubah, sehingga `Ø42` menjadi `Ø45`
//!    tanpa menggeser dimensi lain.
//! 2. **Generator** — [`generate`] memilih dimensi yang layak (ukuran
//!    keseluruhan, Ø poros/lubang, pola lubang + PCD, radius, chamfer, jarak
//!    sumbu), membuang yang sudah muncul di tampak lain, lalu menempatkannya
//!    bertingkat 8/16/24 mm tanpa teks yang saling menimpa.

use std::collections::BTreeSet;

use ducad_core::drawing_annot::{
    DimDirection, DimensionRef, EdgeRef, FeatureRef, PointRef, ViewRef, ViewSide,
};
use ducad_kernel::{HlrGeometricFeature, HlrLineKind, ProjectedView, ProjectedViewKind};

use super::gdt::text_width_mm;
use super::{DimStyle, DimensionAnnotation, DrawingSheet};

/// Tinggi teks dimensi (mm, ISO 3098).
pub const DIM_TEXT_MM: f32 = 2.5;
/// Jarak antar tingkat garis dimensi (mm, ISO 129-1).
pub const TIER_MM: f32 = 8.0;
/// Jarak teks dari garis dimensi/bahu leader (mm).
const TEXT_GAP_MM: f32 = 1.0;

pub fn view_ref(kind: ProjectedViewKind) -> ViewRef {
    match kind {
        ProjectedViewKind::Front => ViewRef::Front,
        ProjectedViewKind::Top => ViewRef::Top,
        ProjectedViewKind::Right => ViewRef::Right,
        ProjectedViewKind::Isometric => ViewRef::Isometric,
        ProjectedViewKind::Section(c) => ViewRef::Section(c),
        ProjectedViewKind::Detail(c) => ViewRef::Detail(c),
    }
}

pub fn view_kind(view: ViewRef) -> ProjectedViewKind {
    match view {
        ViewRef::Front => ProjectedViewKind::Front,
        ViewRef::Top => ProjectedViewKind::Top,
        ViewRef::Right => ProjectedViewKind::Right,
        ViewRef::Isometric => ProjectedViewKind::Isometric,
        ViewRef::Section(c) => ProjectedViewKind::Section(c),
        ViewRef::Detail(c) => ProjectedViewKind::Detail(c),
    }
}

/// Angka dimensi tanpa nol di belakang, paling banyak 2 desimal.
pub fn fmt_dim(value: f32) -> String {
    let mut text = format!("{:.2}", (value as f64 * 100.0).round() / 100.0);
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    if text == "-0" {
        text = "0".to_string();
    }
    text
}

/// Transformasi koordinat model 2D sebuah tampak ke kertas (mm).
pub(crate) struct ViewXf<'a> {
    pub kind: ProjectedViewKind,
    pub view: &'a ProjectedView,
    pub center: [f32; 2],
    pub scale: f32,
}

impl ViewXf<'_> {
    pub fn to_paper(&self, p: [f32; 2]) -> [f32; 2] {
        let c = self.view.center_2d();
        [
            self.center[0] + (p[0] - c[0]) * self.scale,
            self.center[1] + (p[1] - c[1]) * self.scale,
        ]
    }

    /// Kotak tampak di kertas `[x0, y0, x1, y1]`.
    pub fn bbox(&self) -> [f32; 4] {
        let a = self.to_paper(self.view.bounds_min);
        let b = self.to_paper(self.view.bounds_max);
        [
            a[0].min(b[0]),
            a[1].min(b[1]),
            a[0].max(b[0]),
            a[1].max(b[1]),
        ]
    }
}

/// Kandidat terbaik sejauh ini beserta skornya (makin kecil makin cocok).
type Scored<T> = Option<(T, f32)>;
/// (pusat, radius, sudut tengah busur).
type CircleHit = ([f32; 2], f32, Option<f32>);
/// (awal, akhir, panjang, sudut).
type ChamferHit = ([f32; 2], [f32; 2], f32, f32);
/// (awal sumbu, akhir sumbu, radius).
type CylinderHit = ([f32; 2], [f32; 2], f32);
/// (awal, akhir).
type EdgeHit = ([f32; 2], [f32; 2]);

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Lingkaran/busur fitur yang dirujuk `f`: (pusat, radius, sudut tengah busur).
fn find_circle(view: &ProjectedView, f: &FeatureRef) -> Option<([f32; 2], f32, Option<f32>)> {
    let items = view.features.iter().filter_map(|feat| match feat {
        HlrGeometricFeature::Circle {
            center,
            radius,
            edge,
        } => Some((*center, *radius, *edge, None)),
        HlrGeometricFeature::Arc {
            center,
            radius,
            start_angle,
            end_angle,
            edge,
        } => Some((
            *center,
            *radius,
            *edge,
            Some(((start_angle + end_angle) * 0.5).to_degrees()),
        )),
        _ => None,
    });
    let mut best: Scored<CircleHit> = None;
    for (center, radius, edge, mid) in items {
        if f.edge.is_some() && edge == f.edge {
            return Some((center, radius, mid));
        }
        let d = dist(center, f.center);
        if d > 5.0_f32.max(f.radius * 0.5) {
            continue;
        }
        let score = d * 4.0 + (radius - f.radius).abs();
        if best.as_ref().map(|(_, s)| score < *s).unwrap_or(true) {
            best = Some(((center, radius, mid), score));
        }
    }
    best.map(|(b, _)| b)
}

fn find_chamfer(view: &ProjectedView, e: &EdgeRef) -> Option<([f32; 2], [f32; 2], f32, f32)> {
    let mut best: Scored<ChamferHit> = None;
    for feat in &view.features {
        if let HlrGeometricFeature::Chamfer {
            start,
            end,
            length,
            angle_deg,
        } = feat
        {
            let mid = [(start[0] + end[0]) * 0.5, (start[1] + end[1]) * 0.5];
            let d = dist(mid, e.at);
            if d <= 8.0 && best.as_ref().map(|(_, s)| d < *s).unwrap_or(true) {
                best = Some(((*start, *end, *length, *angle_deg), d));
            }
        }
    }
    best.map(|(b, _)| b)
}

/// Sisi silinder yang paling cocok dengan petunjuk sumbu + radius.
fn find_cylinder(
    view: &ProjectedView,
    axis_a: [f32; 2],
    axis_b: [f32; 2],
    radius: f32,
) -> Option<([f32; 2], [f32; 2], f32)> {
    let hint_mid = [(axis_a[0] + axis_b[0]) * 0.5, (axis_a[1] + axis_b[1]) * 0.5];
    let hint_dir = [axis_b[0] - axis_a[0], axis_b[1] - axis_a[1]];
    let hint_len = hint_dir[0].hypot(hint_dir[1]).max(1e-6);
    let mut best: Scored<CylinderHit> = None;
    for feat in &view.features {
        if let HlrGeometricFeature::CylinderSide {
            axis_start,
            axis_end,
            radius: r,
            ..
        } = feat
        {
            let d = [axis_end[0] - axis_start[0], axis_end[1] - axis_start[1]];
            let len = d[0].hypot(d[1]).max(1e-6);
            let cross = (d[0] * hint_dir[1] - d[1] * hint_dir[0]).abs() / (len * hint_len);
            if cross > 0.02 {
                continue;
            }
            let mid = [
                (axis_start[0] + axis_end[0]) * 0.5,
                (axis_start[1] + axis_end[1]) * 0.5,
            ];
            let score = dist(mid, hint_mid) + (r - radius).abs();
            if best.as_ref().map(|(_, s)| score < *s).unwrap_or(true) {
                best = Some(((*axis_start, *axis_end, *r), score));
            }
        }
    }
    best.map(|(b, _)| b)
}

/// Ujung segmen tampak yang dirujuk tepi `e` (awal, akhir).
fn find_edge(view: &ProjectedView, e: &EdgeRef) -> Option<([f32; 2], [f32; 2])> {
    if let Some(idx) = e.edge {
        if let Some(i) = view.edge_refs.iter().position(|r| *r == Some(idx)) {
            return view.segments.get(i).map(|s| (s.start, s.end));
        }
    }
    let mut best: Scored<EdgeHit> = None;
    for s in &view.segments {
        if !matches!(s.kind, HlrLineKind::Visible | HlrLineKind::Silhouette) {
            continue;
        }
        let mid = [(s.start[0] + s.end[0]) * 0.5, (s.start[1] + s.end[1]) * 0.5];
        let d = dist(mid, e.at);
        if d <= 5.0 && best.as_ref().map(|(_, b)| d < *b).unwrap_or(true) {
            best = Some(((s.start, s.end), d));
        }
    }
    best.map(|(b, _)| b)
}

enum Pt {
    At([f32; 2]),
    Extreme(ViewSide),
}

fn point_model(view: &ProjectedView, p: &PointRef) -> Option<Pt> {
    Some(match p {
        PointRef::Center { feature } => Pt::At(find_circle(view, feature)?.0),
        PointRef::EdgeStart { edge } => Pt::At(find_edge(view, edge)?.0),
        PointRef::EdgeEnd { edge } => Pt::At(find_edge(view, edge)?.1),
        PointRef::Extreme { side, .. } => Pt::Extreme(*side),
        PointRef::Point { point, .. } => Pt::At(*point),
    })
}

/// Apakah pusat-pusat membentuk poligon beraturan (pola melingkar)?
/// Mengembalikan (pusat, radius PCD).
fn pitch_circle(centers: &[[f32; 2]]) -> Option<([f32; 2], f32)> {
    let n = centers.len();
    if n < 3 {
        return None;
    }
    let cx = centers.iter().map(|c| c[0]).sum::<f32>() / n as f32;
    let cy = centers.iter().map(|c| c[1]).sum::<f32>() / n as f32;
    let radii: Vec<f32> = centers.iter().map(|c| dist(*c, [cx, cy])).collect();
    let r = radii.iter().sum::<f32>() / n as f32;
    if r < 1e-3 || radii.iter().any(|x| (x - r).abs() > 0.01 * r.max(1.0)) {
        return None;
    }
    let mut angles: Vec<f32> = centers
        .iter()
        .map(|c| (c[1] - cy).atan2(c[0] - cx).to_degrees().rem_euclid(360.0))
        .collect();
    angles.sort_by(f32::total_cmp);
    let step = 360.0 / n as f32;
    for i in 0..n {
        let next = if i + 1 < n {
            angles[i + 1]
        } else {
            angles[0] + 360.0
        };
        if ((next - angles[i]) - step).abs() > 0.5 {
            return None;
        }
    }
    Some(([cx, cy], r))
}

fn same_feature(a: &FeatureRef, b: &FeatureRef) -> bool {
    a.view == b.view
        && match (a.edge, b.edge) {
            (Some(x), Some(y)) if x == y => true,
            _ => dist(a.center, b.center) < 1.0,
        }
}

fn same_point(a: &PointRef, b: &PointRef) -> bool {
    match (a, b) {
        (PointRef::Center { feature: x }, PointRef::Center { feature: y }) => same_feature(x, y),
        (PointRef::EdgeStart { edge: x }, PointRef::EdgeStart { edge: y })
        | (PointRef::EdgeEnd { edge: x }, PointRef::EdgeEnd { edge: y }) => {
            x.view == y.view && dist(x.at, y.at) < 1.0
        }
        (PointRef::Extreme { view: v1, side: s1 }, PointRef::Extreme { view: v2, side: s2 }) => {
            v1 == v2 && s1 == s2
        }
        (
            PointRef::Point {
                view: v1,
                point: p1,
            },
            PointRef::Point {
                view: v2,
                point: p2,
            },
        ) => v1 == v2 && dist(*p1, *p2) < 1.0,
        _ => false,
    }
}

/// Apakah dua sumber dimensi menunjuk fitur yang sama? Petunjuk radius
/// diabaikan (dalam batas wajar) supaya dimensi tetap dikenali setelah
/// `set_params` mengubah ukurannya.
pub fn same_source(a: &DimensionRef, b: &DimensionRef) -> bool {
    match (a, b) {
        (
            DimensionRef::Linear {
                a: a1,
                b: b1,
                dir: d1,
            },
            DimensionRef::Linear {
                a: a2,
                b: b2,
                dir: d2,
            },
        ) => d1 == d2 && same_point(a1, a2) && same_point(b1, b2),
        (DimensionRef::Diameter { circle: x }, DimensionRef::Diameter { circle: y })
        | (DimensionRef::Radius { arc: x }, DimensionRef::Radius { arc: y }) => {
            same_feature(x, y) && (x.radius - y.radius).abs() <= 0.35 * x.radius.max(y.radius)
        }
        (DimensionRef::HolePattern { circles: x }, DimensionRef::HolePattern { circles: y }) => {
            x.len() == y.len()
                && x.first()
                    .zip(y.first())
                    .is_some_and(|(p, q)| same_feature(p, q))
        }
        (DimensionRef::Chamfer { edge: x }, DimensionRef::Chamfer { edge: y }) => {
            x.view == y.view && dist(x.at, y.at) < 3.0
        }
        (DimensionRef::Angle { e1: x1, e2: x2 }, DimensionRef::Angle { e1: y1, e2: y2 }) => {
            x1.view == y1.view && dist(x1.at, y1.at) < 1.0 && dist(x2.at, y2.at) < 1.0
        }
        (
            DimensionRef::CylinderDiameter {
                view: v1,
                axis_a: a1,
                axis_b: b1,
                radius: r1,
            },
            DimensionRef::CylinderDiameter {
                view: v2,
                axis_a: a2,
                axis_b: b2,
                radius: r2,
            },
        ) => {
            v1 == v2
                && dist(*a1, *a2) < 1.0
                && dist(*b1, *b2) < 1.0
                && (r1 - r2).abs() <= 0.35 * r1.max(*r2)
        }
        _ => false,
    }
}

fn chamfer_text(length: f32, angle_deg: f32) -> String {
    format!("{}×{}°", fmt_dim(length), fmt_dim(angle_deg))
}

impl DimensionAnnotation {
    /// Kotak teks dimensi di kertas `[x0, y0, x1, y1]` — sama persis dengan
    /// yang dipakai penggambar (lihat `scene.rs`).
    pub fn text_box(&self) -> [f32; 4] {
        let w = text_width_mm(&self.text, DIM_TEXT_MM);
        let h = DIM_TEXT_MM;
        match self.effective_style() {
            DimStyle::Leader => {
                let (x0, _) = self.leader_shoulder();
                let y = self.line_pos[1] + TEXT_GAP_MM;
                [x0, y, x0 + w, y + h]
            }
            DimStyle::Angle => {
                let x = self.line_pos[0] + 2.0;
                let y = self.line_pos[1] - 1.0;
                [x, y, x + w, y + h]
            }
            DimStyle::Aligned => {
                let half = (w.max(h)) * 0.5;
                [
                    self.line_pos[0] - half,
                    self.line_pos[1] - half,
                    self.line_pos[0] + half,
                    self.line_pos[1] + half,
                ]
            }
            _ => {
                if self.is_vertical {
                    let x1 = self.line_pos[0] - TEXT_GAP_MM;
                    let mid = (self.start[1] + self.end[1]) * 0.5;
                    [x1 - h, mid - w * 0.5, x1, mid + w * 0.5]
                } else {
                    let y0 = self.line_pos[1] + TEXT_GAP_MM;
                    let mid = (self.start[0] + self.end[0]) * 0.5;
                    [mid - w * 0.5, y0, mid + w * 0.5, y0 + h]
                }
            }
        }
    }

    /// Bahu leader: (x awal teks, x ujung bahu). Bahu memanjang menjauhi
    /// ujung panah.
    pub fn leader_shoulder(&self) -> (f32, f32) {
        let w = text_width_mm(&self.text, DIM_TEXT_MM);
        let elbow = self.line_pos[0];
        if elbow >= self.end[0] {
            (elbow + 1.0, elbow + w + 2.0)
        } else {
            (elbow - w - 1.0, elbow - w - 2.0)
        }
    }
}

impl DrawingSheet {
    pub(crate) fn view_xf(&self, kind: ProjectedViewKind) -> Option<ViewXf<'_>> {
        let plc = self
            .view_placements
            .iter()
            .find(|p| p.kind == kind && p.visible)?;
        Some(ViewXf {
            kind,
            view: self.drawing.view_by_kind(kind),
            center: plc.center_mm,
            scale: plc.scale,
        })
    }

    /// Hitung dimensi di kertas dari sumber geometrinya. `None` bila
    /// tampaknya tidak ada di lembar atau fiturnya tidak ditemukan lagi.
    ///
    /// `offset_mm`: untuk dimensi linear = jarak garis dimensi dari tepi
    /// kotak tampak (positif = atas/kanan, negatif = bawah/kiri); untuk
    /// leader = panjang leader. `angle_deg` = arah leader.
    pub fn resolve_dimension(
        &self,
        source: &DimensionRef,
        offset_mm: f32,
        angle_deg: f32,
    ) -> Option<DimensionAnnotation> {
        let kind = view_kind(source.view());
        let xf = self.view_xf(kind)?;
        let view = xf.view;
        let bbox = xf.bbox();
        let s = xf.scale;
        let base = DimensionAnnotation {
            source: Some(source.clone()),
            offset_mm,
            angle_deg,
            view: Some(kind),
            ..DimensionAnnotation::default()
        };
        let leader = |tip: [f32; 2], text: String| {
            let a = angle_deg.to_radians();
            let len = offset_mm.abs().max(3.0);
            DimensionAnnotation {
                start: tip,
                end: tip,
                line_pos: [tip[0] + len * a.cos(), tip[1] + len * a.sin()],
                text,
                style: DimStyle::Leader,
                ..base.clone()
            }
        };

        match source {
            DimensionRef::Linear { a, b, dir } => {
                let pa = point_model(view, a)?;
                let pb = point_model(view, b)?;
                match dir {
                    DimDirection::Horizontal => {
                        let line_y = if offset_mm >= 0.0 {
                            bbox[3] + offset_mm
                        } else {
                            bbox[1] + offset_mm
                        };
                        let edge_y = if offset_mm >= 0.0 { bbox[3] } else { bbox[1] };
                        let resolve = |p: &Pt| -> ([f32; 2], f32) {
                            match p {
                                Pt::At(m) => (xf.to_paper(*m), m[0]),
                                Pt::Extreme(side) => {
                                    let mx = match side {
                                        ViewSide::Left => view.bounds_min[0],
                                        ViewSide::Right => view.bounds_max[0],
                                        _ => view.center_2d()[0],
                                    };
                                    ([xf.to_paper([mx, 0.0])[0], edge_y], mx)
                                }
                            }
                        };
                        let (start, ma) = resolve(&pa);
                        let (end, mb) = resolve(&pb);
                        Some(DimensionAnnotation {
                            start,
                            end,
                            line_pos: [(start[0] + end[0]) * 0.5, line_y],
                            is_vertical: false,
                            text: fmt_dim((mb - ma).abs()),
                            style: DimStyle::Linear,
                            ..base
                        })
                    }
                    DimDirection::Vertical => {
                        let line_x = if offset_mm >= 0.0 {
                            bbox[2] + offset_mm
                        } else {
                            bbox[0] + offset_mm
                        };
                        let edge_x = if offset_mm >= 0.0 { bbox[2] } else { bbox[0] };
                        let resolve = |p: &Pt| -> ([f32; 2], f32) {
                            match p {
                                Pt::At(m) => (xf.to_paper(*m), m[1]),
                                Pt::Extreme(side) => {
                                    let my = match side {
                                        ViewSide::Bottom => view.bounds_min[1],
                                        ViewSide::Top => view.bounds_max[1],
                                        _ => view.center_2d()[1],
                                    };
                                    ([edge_x, xf.to_paper([0.0, my])[1]], my)
                                }
                            }
                        };
                        let (start, ma) = resolve(&pa);
                        let (end, mb) = resolve(&pb);
                        Some(DimensionAnnotation {
                            start,
                            end,
                            line_pos: [line_x, (start[1] + end[1]) * 0.5],
                            is_vertical: true,
                            text: fmt_dim((mb - ma).abs()),
                            style: DimStyle::Linear,
                            ..base
                        })
                    }
                    DimDirection::Aligned => {
                        let (Pt::At(ma), Pt::At(mb)) = (pa, pb) else {
                            return None;
                        };
                        let (start, end) = (xf.to_paper(ma), xf.to_paper(mb));
                        let d = [end[0] - start[0], end[1] - start[1]];
                        let len = d[0].hypot(d[1]).max(1e-6);
                        let n = [-d[1] / len, d[0] / len];
                        Some(DimensionAnnotation {
                            start,
                            end,
                            line_pos: [
                                (start[0] + end[0]) * 0.5 + n[0] * offset_mm,
                                (start[1] + end[1]) * 0.5 + n[1] * offset_mm,
                            ],
                            is_vertical: false,
                            text: fmt_dim(dist(ma, mb)),
                            style: DimStyle::Aligned,
                            ..base
                        })
                    }
                }
            }
            DimensionRef::CylinderDiameter {
                axis_a,
                axis_b,
                radius,
                ..
            } => {
                let (a, b, r) = find_cylinder(view, *axis_a, *axis_b, *radius)?;
                let text = format!("Ø{}", fmt_dim(r * 2.0));
                if (a[1] - b[1]).abs() <= 1e-3 * (1.0 + (a[0] - b[0]).abs()) {
                    // Sumbu mendatar → dimensi tegak di kiri/kanan tampak.
                    let mx = if offset_mm >= 0.0 {
                        a[0].max(b[0])
                    } else {
                        a[0].min(b[0])
                    };
                    let start = xf.to_paper([mx, a[1] - r]);
                    let end = xf.to_paper([mx, a[1] + r]);
                    let line_x = if offset_mm >= 0.0 {
                        bbox[2] + offset_mm
                    } else {
                        bbox[0] + offset_mm
                    };
                    Some(DimensionAnnotation {
                        start,
                        end,
                        line_pos: [line_x, (start[1] + end[1]) * 0.5],
                        is_vertical: true,
                        text,
                        style: DimStyle::Linear,
                        ..base
                    })
                } else if (a[0] - b[0]).abs() <= 1e-3 * (1.0 + (a[1] - b[1]).abs()) {
                    let my = if offset_mm >= 0.0 {
                        a[1].max(b[1])
                    } else {
                        a[1].min(b[1])
                    };
                    let start = xf.to_paper([a[0] - r, my]);
                    let end = xf.to_paper([a[0] + r, my]);
                    let line_y = if offset_mm >= 0.0 {
                        bbox[3] + offset_mm
                    } else {
                        bbox[1] + offset_mm
                    };
                    Some(DimensionAnnotation {
                        start,
                        end,
                        line_pos: [(start[0] + end[0]) * 0.5, line_y],
                        is_vertical: false,
                        text,
                        style: DimStyle::Linear,
                        ..base
                    })
                } else {
                    None
                }
            }
            DimensionRef::Diameter { circle } => {
                let (c, r, _) = find_circle(view, circle)?;
                let a = angle_deg.to_radians();
                let tip = xf.to_paper([c[0] + r * a.cos(), c[1] + r * a.sin()]);
                Some(leader(tip, format!("Ø{}", fmt_dim(r * 2.0))))
            }
            DimensionRef::Radius { arc } => {
                let (c, r, _) = find_circle(view, arc)?;
                let a = angle_deg.to_radians();
                let tip = xf.to_paper([c[0] + r * a.cos(), c[1] + r * a.sin()]);
                Some(leader(tip, format!("R{}", fmt_dim(r))))
            }
            DimensionRef::HolePattern { circles } => {
                let found: Vec<([f32; 2], f32)> = circles
                    .iter()
                    .filter_map(|c| find_circle(view, c).map(|(c, r, _)| (c, r)))
                    .collect();
                let (c0, r0) = *found.first()?;
                let centers: Vec<[f32; 2]> = found.iter().map(|f| f.0).collect();
                let mut text = format!("{}×Ø{}", found.len(), fmt_dim(r0 * 2.0));
                let pcd = pitch_circle(&centers);
                if let Some((_, pr)) = pcd {
                    text.push_str(&format!(" PCD Ø{}", fmt_dim(pr * 2.0)));
                }
                let a = angle_deg.to_radians();
                let tip = xf.to_paper([c0[0] + r0 * a.cos(), c0[1] + r0 * a.sin()]);
                let mut dim = leader(tip, text);
                dim.aux_circle = pcd.map(|(pc, pr)| {
                    let p = xf.to_paper(pc);
                    [p[0], p[1], pr * s]
                });
                Some(dim)
            }
            DimensionRef::Chamfer { edge } => {
                let (p0, p1, length, angle) = find_chamfer(view, edge)?;
                let tip = xf.to_paper([(p0[0] + p1[0]) * 0.5, (p0[1] + p1[1]) * 0.5]);
                Some(leader(tip, chamfer_text(length, angle)))
            }
            DimensionRef::Angle { e1, e2 } => {
                let (a0, a1) = find_edge(view, e1)?;
                let (b0, b1) = find_edge(view, e2)?;
                // Titik sudut = pasangan ujung terdekat.
                let pairs = [
                    (a0, a1, b0, b1),
                    (a0, a1, b1, b0),
                    (a1, a0, b0, b1),
                    (a1, a0, b1, b0),
                ];
                let (v, arm1, _, arm2) = pairs
                    .iter()
                    .min_by(|x, y| dist(x.0, x.2).total_cmp(&dist(y.0, y.2)))
                    .copied()?;
                let d1 = [arm1[0] - v[0], arm1[1] - v[1]];
                let d2 = [arm2[0] - v[0], arm2[1] - v[1]];
                let (l1, l2) = (d1[0].hypot(d1[1]).max(1e-6), d2[0].hypot(d2[1]).max(1e-6));
                let cos = ((d1[0] * d2[0] + d1[1] * d2[1]) / (l1 * l2)).clamp(-1.0, 1.0);
                let bis = [d1[0] / l1 + d2[0] / l2, d1[1] / l1 + d2[1] / l2];
                let bl = bis[0].hypot(bis[1]).max(1e-6);
                let vp = xf.to_paper(v);
                let reach = offset_mm.abs().max(6.0);
                Some(DimensionAnnotation {
                    start: vp,
                    end: xf.to_paper(arm1),
                    line_pos: [vp[0] + bis[0] / bl * reach, vp[1] + bis[1] / bl * reach],
                    text: format!("{}°", fmt_dim(cos.acos().to_degrees())),
                    style: DimStyle::Angle,
                    ..base
                })
            }
        }
    }

    /// Hitung ulang semua dimensi yang punya `source` (otomatis + manual).
    /// `offset_mm`/`angle_deg` dipertahankan. Mengembalikan jumlah dimensi
    /// yang sumbernya tidak ditemukan lagi (dibiarkan di posisi lamanya).
    pub fn refresh_associative_dimensions(&mut self) -> usize {
        let mut dangling = 0;
        let refresh =
            |sheet: &DrawingSheet, dim: &DimensionAnnotation| -> Option<DimensionAnnotation> {
                let source = dim.source.as_ref()?;
                let mut fresh = sheet.resolve_dimension(source, dim.offset_mm, dim.angle_deg)?;
                fresh.pinned = dim.pinned;
                Some(fresh)
            };
        let auto: Vec<Option<DimensionAnnotation>> = self
            .auto_dimensions
            .iter()
            .map(|d| refresh(self, d))
            .collect();
        let manual: Vec<Option<DimensionAnnotation>> = self
            .manual_dimensions
            .iter()
            .map(|d| refresh(self, d))
            .collect();
        for (slot, fresh) in self
            .auto_dimensions
            .iter_mut()
            .zip(auto)
            .chain(self.manual_dimensions.iter_mut().zip(manual))
        {
            match fresh {
                Some(f) => *slot = f,
                None if slot.source.is_some() => dangling += 1,
                None => {}
            }
        }
        dangling
    }
}

// ---------------------------------------------------------------------------
// Interaksi editor: memilih fitur, membuat dan menggeser dimensi
// ---------------------------------------------------------------------------

impl DrawingSheet {
    /// Tampak (ortografik/potongan/detail) yang kotaknya memuat titik kertas `p`.
    pub fn view_at(&self, p: [f32; 2]) -> Option<ProjectedViewKind> {
        let mut best: Option<(ProjectedViewKind, f32)> = None;
        for plc in self.view_placements.iter().filter(|v| v.visible) {
            let Some(xf) = self.view_xf(plc.kind) else {
                continue;
            };
            let b = grow(xf.bbox(), 6.0);
            if p[0] >= b[0] && p[0] <= b[2] && p[1] >= b[1] && p[1] <= b[3] {
                let d = dist(p, plc.center_mm);
                if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                    best = Some((plc.kind, d));
                }
            }
        }
        best.map(|(k, _)| k)
    }

    /// Titik kertas → koordinat model 2D tampak `kind`.
    pub fn paper_to_model(&self, kind: ProjectedViewKind, p: [f32; 2]) -> Option<[f32; 2]> {
        let xf = self.view_xf(kind)?;
        let c = xf.view.center_2d();
        Some([
            c[0] + (p[0] - xf.center[0]) / xf.scale,
            c[1] + (p[1] - xf.center[1]) / xf.scale,
        ])
    }

    /// Skala tampak di titik kertas `p` (skala lembar bila di luar tampak).
    pub fn scale_at(&self, p: [f32; 2]) -> f32 {
        self.view_at(p)
            .and_then(|k| self.view_placements.iter().find(|v| v.kind == k))
            .map(|v| v.scale)
            .unwrap_or(self.scale)
    }

    /// Lingkaran/busur fitur di dekat titik kertas `p` (pusat atau tepinya
    /// dalam `tol_mm` kertas): (rujukan, benar bila busur parsial).
    pub fn pick_circle(&self, p: [f32; 2], tol_mm: f32) -> Option<(FeatureRef, bool)> {
        let kind = self.view_at(p)?;
        let xf = self.view_xf(kind)?;
        let m = self.paper_to_model(kind, p)?;
        let tol = tol_mm / xf.scale;
        let mut best: Option<((FeatureRef, bool), f32)> = None;
        for f in &xf.view.features {
            let (center, radius, edge, partial) = match f {
                HlrGeometricFeature::Circle {
                    center,
                    radius,
                    edge,
                } => (*center, *radius, *edge, false),
                HlrGeometricFeature::Arc {
                    center,
                    radius,
                    edge,
                    ..
                } => (*center, *radius, *edge, true),
                _ => continue,
            };
            let d = dist(m, center);
            let score = d.min((d - radius).abs());
            if score <= tol && best.as_ref().map(|(_, s)| score < *s).unwrap_or(true) {
                best = Some((
                    (
                        FeatureRef {
                            view: view_ref(kind),
                            edge,
                            center,
                            radius,
                        },
                        partial,
                    ),
                    score,
                ));
            }
        }
        best.map(|(b, _)| b)
    }

    fn point_ref_at(&self, kind: ProjectedViewKind, p: [f32; 2]) -> Option<PointRef> {
        let model = self.paper_to_model(kind, p)?;
        let xf = self.view_xf(kind)?;
        // Pusat lingkaran lebih disukai daripada titik bebas.
        for f in &xf.view.features {
            if let HlrGeometricFeature::Circle {
                center,
                radius,
                edge,
            }
            | HlrGeometricFeature::Arc {
                center,
                radius,
                edge,
                ..
            } = f
            {
                if dist(model, *center) * xf.scale < 1.2 {
                    return Some(PointRef::Center {
                        feature: FeatureRef {
                            view: view_ref(kind),
                            edge: *edge,
                            center: *center,
                            radius: *radius,
                        },
                    });
                }
            }
        }
        Some(PointRef::Point {
            view: view_ref(kind),
            point: model,
        })
    }

    /// Dimensi linear antara dua titik kertas (hasil snap). Bila keduanya di
    /// tampak yang sama, dimensi menjadi asosiatif (punya `source`) dan
    /// nilainya memakai skala tampak itu; selain itu dimensi mutlak.
    pub fn make_linear_dimension(&self, p1: [f32; 2], p2: [f32; 2]) -> Option<DimensionAnnotation> {
        let vertical = (p2[0] - p1[0]).abs() < (p2[1] - p1[1]).abs();
        if let (Some(k1), Some(k2)) = (self.view_at(p1), self.view_at(p2)) {
            if k1 == k2 {
                let (a, b) = (self.point_ref_at(k1, p1)?, self.point_ref_at(k1, p2)?);
                let source = DimensionRef::Linear {
                    a,
                    b,
                    dir: if vertical {
                        DimDirection::Vertical
                    } else {
                        DimDirection::Horizontal
                    },
                };
                let xf = self.view_xf(k1)?;
                let c = xf.center;
                // Sisi tampak yang terdekat dengan titik tengah ukuran.
                let mid = [(p1[0] + p2[0]) * 0.5, (p1[1] + p2[1]) * 0.5];
                let offset = if vertical {
                    if mid[0] >= c[0] {
                        TIER_MM
                    } else {
                        -TIER_MM
                    }
                } else if mid[1] >= c[1] {
                    TIER_MM
                } else {
                    -16.0
                };
                if let Some(mut dim) = self.resolve_dimension(&source, offset, 0.0) {
                    dim.pinned = true;
                    return Some(dim);
                }
            }
        }
        let value = dist(p1, p2) / self.scale_at(p1).max(1e-6);
        (value > 0.05).then(|| DimensionAnnotation {
            start: p1,
            end: p2,
            line_pos: [(p1[0] + p2[0]) * 0.5, (p1[1] + p2[1]) * 0.5],
            is_vertical: vertical,
            text: fmt_dim(value),
            style: DimStyle::Linear,
            ..DimensionAnnotation::default()
        })
    }

    /// Dimensi Ø (`radius = false`) atau R dari klik pusat lalu klik tepi.
    /// Menempel ke lingkaran/busur fitur bila ada di dekat salah satu klik.
    pub fn make_radial_dimension(
        &self,
        center: [f32; 2],
        rim: [f32; 2],
        radius: bool,
    ) -> Option<DimensionAnnotation> {
        let angle = (rim[1] - center[1]).atan2(rim[0] - center[0]).to_degrees();
        let picked = self
            .pick_circle(rim, 2.0)
            .or_else(|| self.pick_circle(center, 2.0));
        if let Some((feature, _)) = picked {
            let source = if radius {
                DimensionRef::Radius { arc: feature }
            } else {
                DimensionRef::Diameter { circle: feature }
            };
            if let Some(mut dim) = self.resolve_dimension(&source, 10.0, angle) {
                dim.pinned = true;
                return Some(dim);
            }
        }
        let r = dist(center, rim) / self.scale_at(center).max(1e-6);
        (r > 0.025).then(|| {
            let a = angle.to_radians();
            DimensionAnnotation {
                start: center,
                end: rim,
                line_pos: [rim[0] + 10.0 * a.cos(), rim[1] + 10.0 * a.sin()],
                text: if radius {
                    format!("R{}", fmt_dim(r))
                } else {
                    format!("Ø{}", fmt_dim(r * 2.0))
                },
                style: DimStyle::Leader,
                ..DimensionAnnotation::default()
            }
        })
    }

    /// Geser dimensi ke posisi garis/siku baru `target` (mm kertas). Dimensi
    /// asosiatif menyimpan hasilnya sebagai `offset_mm`/`angle_deg` (dan
    /// ditandai `pinned`); dimensi mutlak hanya memindah `line_pos`.
    pub fn move_dimension(&mut self, auto: bool, index: usize, target: [f32; 2]) {
        let list = if auto {
            &self.auto_dimensions
        } else {
            &self.manual_dimensions
        };
        let Some(dim) = list.get(index).cloned() else {
            return;
        };
        let updated = match (&dim.source, dim.view.and_then(|k| self.view_xf(k))) {
            (Some(source), Some(xf)) => {
                let b = xf.bbox();
                let (offset, angle) = match dim.effective_style() {
                    DimStyle::Leader | DimStyle::Angle => {
                        let d = [target[0] - dim.end[0], target[1] - dim.end[1]];
                        (d[0].hypot(d[1]).max(3.0), d[1].atan2(d[0]).to_degrees())
                    }
                    DimStyle::Aligned => {
                        let d = [dim.end[0] - dim.start[0], dim.end[1] - dim.start[1]];
                        let len = d[0].hypot(d[1]).max(1e-6);
                        let mid = [
                            (dim.start[0] + dim.end[0]) * 0.5,
                            (dim.start[1] + dim.end[1]) * 0.5,
                        ];
                        (
                            (target[0] - mid[0]) * (-d[1] / len)
                                + (target[1] - mid[1]) * (d[0] / len),
                            dim.angle_deg,
                        )
                    }
                    _ if dim.is_vertical => {
                        let mid = (b[0] + b[2]) * 0.5;
                        let o = if target[0] >= mid {
                            (target[0] - b[2]).max(2.0)
                        } else {
                            (target[0] - b[0]).min(-2.0)
                        };
                        (o, dim.angle_deg)
                    }
                    _ => {
                        let mid = (b[1] + b[3]) * 0.5;
                        let o = if target[1] >= mid {
                            (target[1] - b[3]).max(2.0)
                        } else {
                            (target[1] - b[1]).min(-2.0)
                        };
                        (o, dim.angle_deg)
                    }
                };
                self.resolve_dimension(source, offset, angle).map(|mut d| {
                    d.pinned = true;
                    d
                })
            }
            _ => None,
        };
        let list = if auto {
            &mut self.auto_dimensions
        } else {
            &mut self.manual_dimensions
        };
        match updated {
            Some(d) => list[index] = d,
            None => {
                let d = &mut list[index];
                match d.effective_style() {
                    DimStyle::Linear if d.is_vertical => d.line_pos[0] = target[0],
                    DimStyle::Linear => d.line_pos[1] = target[1],
                    _ => d.line_pos = target,
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Generator
// ---------------------------------------------------------------------------

fn boxes_overlap(a: &[f32; 4], b: &[f32; 4]) -> bool {
    a[0] < b[2] - 1e-3 && b[0] < a[2] - 1e-3 && a[1] < b[3] - 1e-3 && b[1] < a[3] - 1e-3
}

fn grow(b: [f32; 4], m: f32) -> [f32; 4] {
    [b[0] - m, b[1] - m, b[2] + m, b[3] + m]
}

struct Placer {
    /// Kotak yang tidak boleh ditimpa teks: teks lain, judul, kepala gambar.
    obstacles: Vec<[f32; 4]>,
    /// Kotak geometri tiap tampak.
    views: Vec<(ProjectedViewKind, [f32; 4])>,
    frame: [f32; 4],
}

impl Placer {
    /// `own` = tampak pemilik dimensi; kotaknya sendiri tidak dihitung untuk
    /// dimensi linear (teksnya memang di luar kotak itu).
    fn is_free(&self, text: &[f32; 4], own: Option<ProjectedViewKind>) -> bool {
        let t = grow(*text, 0.4);
        if t[0] < self.frame[0]
            || t[1] < self.frame[1]
            || t[2] > self.frame[2]
            || t[3] > self.frame[3]
        {
            return false;
        }
        if self.obstacles.iter().any(|o| boxes_overlap(&t, o)) {
            return false;
        }
        !self
            .views
            .iter()
            .any(|(k, b)| Some(*k) != own && boxes_overlap(&t, b))
    }
}

struct LinearCand {
    source: DimensionRef,
    /// Panjang bentang di kertas, untuk urutan pendek → panjang.
    span: f32,
    /// Sisi yang disukai: +1 atas/kanan, −1 bawah/kiri.
    side: f32,
}

struct LeaderCand {
    source: DimensionRef,
    /// Sudut yang dicoba berurutan.
    angles: Vec<f32>,
}

/// Sumbu dunia ('x','y','z') yang sejajar vektor satuan `v`, bila ada.
fn world_axis(v: [f32; 3]) -> Option<char> {
    ['x', 'y', 'z']
        .into_iter()
        .zip(v)
        .find(|(_, c)| c.abs() > 0.999)
        .map(|(a, _)| a)
}

fn view_axes(sheet: &DrawingSheet, kind: ProjectedViewKind) -> (Option<char>, Option<char>) {
    match kind {
        ProjectedViewKind::Front => (Some('x'), Some('z')),
        ProjectedViewKind::Top => (Some('x'), Some('y')),
        ProjectedViewKind::Right => (Some('y'), Some('z')),
        ProjectedViewKind::Section(c) => sheet
            .drawing
            .section(c)
            .map(|s| (world_axis(s.config.u_axis), world_axis(s.config.v_axis)))
            .unwrap_or((None, None)),
        _ => (None, None),
    }
}

fn model_extent(sheet: &DrawingSheet, axis: char) -> f32 {
    let i = match axis {
        'x' => 0,
        'y' => 1,
        _ => 2,
    };
    (sheet.drawing.model_bbox_max[i] - sheet.drawing.model_bbox_min[i]).abs()
}

fn key_num(v: f32) -> i64 {
    (v as f64 * 100.0).round() as i64
}

fn new_placer(sheet: &DrawingSheet) -> Placer {
    let (_, inner) = sheet.border_rects_mm();
    let mut placer = Placer {
        obstacles: sheet.layout_obstacles(),
        views: Vec::new(),
        frame: grow(inner, -1.0),
    };
    for plc in &sheet.view_placements {
        if plc.visible {
            if let Some(xf) = sheet.view_xf(plc.kind) {
                placer.views.push((plc.kind, grow(xf.bbox(), 1.0)));
            }
        }
    }
    placer
}

/// Tempatkan daftar dimensi eksplisit (`DimensionPolicy::Only`) dengan aturan
/// tingkat dan uji tabrak yang sama dengan dimensi otomatis.
pub(crate) fn place_only(sheet: &DrawingSheet, refs: &[DimensionRef]) -> Vec<DimensionAnnotation> {
    let mut placer = new_placer(sheet);
    let mut out = Vec::new();
    // Per tampak, dalam urutan kemunculan pertama.
    let mut kinds: Vec<ProjectedViewKind> = Vec::new();
    for r in refs {
        let k = view_kind(r.view());
        if !kinds.contains(&k) {
            kinds.push(k);
        }
    }
    for kind in kinds {
        let Some(xf) = sheet.view_xf(kind) else {
            continue;
        };
        let mut horizontal = Vec::new();
        let mut vertical = Vec::new();
        let mut leaders = Vec::new();
        for r in refs.iter().filter(|r| view_kind(r.view()) == kind) {
            match r {
                DimensionRef::Linear {
                    dir: DimDirection::Horizontal,
                    ..
                } => horizontal.push(LinearCand {
                    source: r.clone(),
                    span: horizontal.len() as f32,
                    side: 1.0,
                }),
                DimensionRef::Linear {
                    dir: DimDirection::Vertical,
                    ..
                } => vertical.push(LinearCand {
                    source: r.clone(),
                    span: vertical.len() as f32,
                    side: -1.0,
                }),
                DimensionRef::CylinderDiameter { axis_a, axis_b, .. } => {
                    if (axis_a[1] - axis_b[1]).abs() <= (axis_a[0] - axis_b[0]).abs() {
                        vertical.push(LinearCand {
                            source: r.clone(),
                            span: vertical.len() as f32,
                            side: -1.0,
                        });
                    } else {
                        horizontal.push(LinearCand {
                            source: r.clone(),
                            span: horizontal.len() as f32,
                            side: 1.0,
                        });
                    }
                }
                DimensionRef::Linear {
                    dir: DimDirection::Aligned,
                    ..
                }
                | DimensionRef::Angle { .. } => {
                    if let Some(dim) = sheet.resolve_dimension(r, TIER_MM, 0.0) {
                        placer.obstacles.push(grow(dim.text_box(), 0.3));
                        out.push(dim);
                    }
                }
                _ => leaders.push(LeaderCand {
                    source: r.clone(),
                    angles: vec![45.0, 135.0, 315.0, 225.0, 0.0, 90.0, 180.0, 270.0],
                }),
            }
        }
        place_linear(sheet, &mut placer, kind, horizontal, false, &mut out);
        place_linear(sheet, &mut placer, kind, vertical, true, &mut out);
        place_leaders(sheet, &mut placer, &xf, leaders, &mut out);
    }
    out
}

/// Bangun dimensi otomatis seluruh lembar.
pub(crate) fn generate(sheet: &DrawingSheet) -> Vec<DimensionAnnotation> {
    let mut order: Vec<ProjectedViewKind> = vec![ProjectedViewKind::Front, ProjectedViewKind::Top];
    order.extend(sheet.drawing.sections.iter().map(|s| s.kind()));
    order.push(ProjectedViewKind::Right);

    let mut placer = new_placer(sheet);

    // Radius seluruh sisi silinder di gambar — untuk mengenali cincin chamfer.
    let mut cyl_radii: Vec<f32> = Vec::new();
    let mut chamfer_lengths: Vec<f32> = Vec::new();
    for kind in &order {
        for f in &sheet.drawing.view_by_kind(*kind).features {
            match f {
                HlrGeometricFeature::CylinderSide { radius, .. } => cyl_radii.push(*radius),
                HlrGeometricFeature::Chamfer {
                    length, angle_deg, ..
                } => {
                    // Selisih radius cincin chamfer.
                    chamfer_lengths.push(length * angle_deg.to_radians().tan());
                }
                _ => {}
            }
        }
    }

    // Seluruh diameter yang akan muncul: ukuran keseluruhan yang sama dengan
    // sebuah Ø tidak diulang sebagai dimensi linear.
    let mut diameters: Vec<f32> = cyl_radii.iter().map(|r| r * 2.0).collect();
    for kind in &order {
        for f in &sheet.drawing.view_by_kind(*kind).features {
            if let HlrGeometricFeature::Circle { radius, .. } = f {
                diameters.push(radius * 2.0);
            }
        }
    }

    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut out: Vec<DimensionAnnotation> = Vec::new();

    for kind in order {
        let Some(xf) = sheet.view_xf(kind) else {
            continue;
        };
        let view = xf.view;
        let vr = view_ref(kind);
        let size = view.size_2d();
        let vc = view.center_2d();
        let mut horizontal: Vec<LinearCand> = Vec::new();
        let mut vertical: Vec<LinearCand> = Vec::new();
        let mut leaders: Vec<LeaderCand> = Vec::new();

        // a. Ukuran keseluruhan.
        let (ax_u, ax_v) = view_axes(sheet, kind);
        let overall = |axis: Option<char>, extent: f32, seen: &mut BTreeSet<String>| -> bool {
            // Bentang yang sama dengan Ø terbesar sudah diwakili dimensi Ø itu.
            let max_dia = diameters.iter().copied().fold(0.0, f32::max);
            if extent < 0.5 || (max_dia - extent).abs() < 0.02 {
                return false;
            }
            let key = match axis {
                Some(a) if (model_extent(sheet, a) - extent).abs() < 0.02 => format!("ext:{a}"),
                _ => format!("len:{}", key_num(extent)),
            };
            seen.insert(key)
        };
        if overall(ax_u, view.bounds_max[0] - view.bounds_min[0], &mut seen) {
            horizontal.push(LinearCand {
                source: DimensionRef::Linear {
                    a: PointRef::Extreme {
                        view: vr,
                        side: ViewSide::Left,
                    },
                    b: PointRef::Extreme {
                        view: vr,
                        side: ViewSide::Right,
                    },
                    dir: DimDirection::Horizontal,
                },
                span: size[0] * xf.scale,
                side: 1.0,
            });
        }
        if overall(ax_v, view.bounds_max[1] - view.bounds_min[1], &mut seen) {
            vertical.push(LinearCand {
                source: DimensionRef::Linear {
                    a: PointRef::Extreme {
                        view: vr,
                        side: ViewSide::Bottom,
                    },
                    b: PointRef::Extreme {
                        view: vr,
                        side: ViewSide::Top,
                    },
                    dir: DimDirection::Vertical,
                },
                span: size[1] * xf.scale,
                side: -1.0,
            });
        }

        // b. Ø sisi silinder + jarak sumbu ke tepi terjauh.
        let max_radius = view
            .features
            .iter()
            .filter_map(|f| match f {
                HlrGeometricFeature::CylinderSide { radius, .. } => Some(*radius),
                _ => None,
            })
            .fold(0.0, f32::max);
        for f in &view.features {
            let HlrGeometricFeature::CylinderSide {
                axis_start,
                axis_end,
                radius,
                visible,
            } = f
            else {
                continue;
            };
            let horizontal_axis = (axis_start[1] - axis_end[1]).abs() < 1e-3;
            let vertical_axis = (axis_start[0] - axis_end[0]).abs() < 1e-3;
            if !horizontal_axis && !vertical_axis {
                continue;
            }
            let mid = [
                (axis_start[0] + axis_end[0]) * 0.5,
                (axis_start[1] + axis_end[1]) * 0.5,
            ];
            if *visible && seen.insert(format!("dia:{}", key_num(radius * 2.0))) {
                let source = DimensionRef::CylinderDiameter {
                    view: vr,
                    axis_a: *axis_start,
                    axis_b: *axis_end,
                    radius: *radius,
                };
                let cand = |side: f32| LinearCand {
                    source: source.clone(),
                    span: radius * 2.0 * xf.scale,
                    side,
                };
                if horizontal_axis {
                    vertical.push(cand(if mid[0] > vc[0] + 1e-3 { 1.0 } else { -1.0 }));
                } else {
                    horizontal.push(cand(if mid[1] < vc[1] - 1e-3 { -1.0 } else { 1.0 }));
                }
            }
        }

        // Sumbu utama = garis sumbu silinder terbesar; bila gabungan silinder
        // segaris pada sumbu itu membentang ≥ 50 % tampak dan sumbunya tidak
        // di tengah, beri jarak sumbu → tepi yang lebih jauh (mis. sumbu
        // aliran → muka flange atas).
        let main_axis = view.features.iter().find_map(|f| match f {
            HlrGeometricFeature::CylinderSide {
                axis_start,
                axis_end,
                radius,
                ..
            } if (*radius - max_radius).abs() < 1e-3 => Some((*axis_start, *axis_end)),
            _ => None,
        });
        if let Some((ma, mb)) = main_axis {
            let horizontal_axis = (ma[1] - mb[1]).abs() < 1e-3;
            let vertical_axis = (ma[0] - mb[0]).abs() < 1e-3;
            // Rentang gabungan semua silinder pada garis sumbu yang sama.
            let (mut lo, mut hi) = (f32::MAX, f32::MIN);
            for f in &view.features {
                if let HlrGeometricFeature::CylinderSide {
                    axis_start,
                    axis_end,
                    ..
                } = f
                {
                    let (i, j) = if horizontal_axis { (0, 1) } else { (1, 0) };
                    let collinear =
                        (axis_start[j] - ma[j]).abs() < 1e-3 && (axis_end[j] - ma[j]).abs() < 1e-3;
                    if collinear && (horizontal_axis || vertical_axis) {
                        lo = lo.min(axis_start[i]).min(axis_end[i]);
                        hi = hi.max(axis_start[i]).max(axis_end[i]);
                    }
                }
            }
            if horizontal_axis && hi - lo >= 0.5 * size[0] {
                let top = view.bounds_max[1] - ma[1];
                let bottom = ma[1] - view.bounds_min[1];
                let far = top.max(bottom);
                if (top - bottom).abs() > 0.02
                    && far > 0.5
                    && seen.insert(format!("len:{}", key_num(far)))
                {
                    let side = if top > bottom {
                        ViewSide::Top
                    } else {
                        ViewSide::Bottom
                    };
                    vertical.push(LinearCand {
                        source: DimensionRef::Linear {
                            a: PointRef::Point {
                                view: vr,
                                point: [view.bounds_min[0], ma[1]],
                            },
                            b: PointRef::Extreme { view: vr, side },
                            dir: DimDirection::Vertical,
                        },
                        span: far * xf.scale,
                        side: -1.0,
                    });
                }
            }
            if vertical_axis && hi - lo >= 0.5 * size[1] {
                let right = view.bounds_max[0] - ma[0];
                let left = ma[0] - view.bounds_min[0];
                let far = right.max(left);
                if (right - left).abs() > 0.02
                    && far > 0.5
                    && seen.insert(format!("len:{}", key_num(far)))
                {
                    let side = if right > left {
                        ViewSide::Right
                    } else {
                        ViewSide::Left
                    };
                    horizontal.push(LinearCand {
                        source: DimensionRef::Linear {
                            a: PointRef::Point {
                                view: vr,
                                point: [ma[0], view.bounds_max[1]],
                            },
                            b: PointRef::Extreme { view: vr, side },
                            dir: DimDirection::Horizontal,
                        },
                        span: far * xf.scale,
                        side: 1.0,
                    });
                }
            }
        }

        // c. Lingkaran tampak muka: kelompokkan menurut radius.
        struct Circ {
            center: [f32; 2],
            radius: f32,
            edge: Option<u32>,
        }
        let circles: Vec<Circ> = view
            .features
            .iter()
            .filter_map(|f| match f {
                HlrGeometricFeature::Circle {
                    center,
                    radius,
                    edge,
                } => Some(Circ {
                    center: *center,
                    radius: *radius,
                    edge: *edge,
                }),
                _ => None,
            })
            .collect();
        let is_chamfer_ring = |c: &Circ| -> bool {
            circles.iter().any(|o| {
                dist(o.center, c.center) < 0.01
                    && chamfer_lengths
                        .iter()
                        .any(|l| ((o.radius - c.radius).abs() - l).abs() < 0.02)
                    && {
                        // `o` adalah silinder nyata; `c` hanya tepi chamfer-nya.
                        let real = |r: f32| cyl_radii.iter().any(|x| (x - r).abs() < 0.02);
                        real(o.radius) && !real(c.radius)
                    }
            })
        };
        let mut groups: Vec<Vec<&Circ>> = Vec::new();
        for c in &circles {
            if is_chamfer_ring(c) {
                continue;
            }
            match groups.iter_mut().find(|g| {
                (g[0].radius - c.radius).abs() < 0.01 && dist(g[0].center, c.center) > 0.01
            }) {
                Some(g) => g.push(c),
                None => groups.push(vec![c]),
            }
        }
        let fref = |c: &Circ| FeatureRef {
            view: vr,
            edge: c.edge,
            center: c.center,
            radius: c.radius,
        };
        let away_angles = |p: [f32; 2]| -> Vec<f32> {
            let d = [p[0] - vc[0], p[1] - vc[1]];
            let base = if d[0].hypot(d[1]) < 1e-3 {
                45.0
            } else {
                // Bulatkan ke kelipatan 15° supaya leader rapi dan stabil.
                (d[1].atan2(d[0]).to_degrees() / 15.0).round() * 15.0
            };
            [
                0.0, 30.0, -30.0, 60.0, -60.0, 90.0, -90.0, 135.0, -135.0, 180.0,
            ]
            .iter()
            .map(|o| base + o)
            .collect()
        };
        for (gi, group) in groups.iter().enumerate() {
            let d = group[0].radius * 2.0;
            if group.len() == 1 {
                if seen.insert(format!("dia:{}", key_num(d))) {
                    // Lingkaran konsentris: sebar arah leader.
                    let mut angles = away_angles(group[0].center);
                    let n = angles.len();
                    angles.rotate_left((gi * 2) % n);
                    leaders.push(LeaderCand {
                        source: DimensionRef::Diameter {
                            circle: fref(group[0]),
                        },
                        angles,
                    });
                }
                continue;
            }
            if !seen.insert(format!("pat:{}x{}", group.len(), key_num(d))) {
                continue;
            }
            // Jangkar leader = lubang paling jauh dari pusat tampak (stabil:
            // ikatan diselesaikan lewat koordinat).
            let mut sorted: Vec<&Circ> = group.clone();
            sorted.sort_by(|a, b| {
                dist(b.center, vc)
                    .total_cmp(&dist(a.center, vc))
                    .then(b.center[0].total_cmp(&a.center[0]))
                    .then(b.center[1].total_cmp(&a.center[1]))
            });
            leaders.push(LeaderCand {
                source: DimensionRef::HolePattern {
                    circles: sorted.iter().map(|c| fref(c)).collect(),
                },
                angles: away_angles(sorted[0].center),
            });
            let centers: Vec<[f32; 2]> = group.iter().map(|c| c.center).collect();
            if pitch_circle(&centers).is_some() {
                continue;
            }
            // Pola tak melingkar: jarak antar sumbu mendatar dan tegak.
            let extreme = |axis: usize, max: bool| -> &Circ {
                let mut best = group[0];
                for c in group.iter() {
                    let better = if max {
                        c.center[axis] > best.center[axis] + 1e-3
                    } else {
                        c.center[axis] < best.center[axis] - 1e-3
                    };
                    if better {
                        best = c;
                    }
                }
                best
            };
            let (l, r) = (extreme(0, false), extreme(0, true));
            let dx = r.center[0] - l.center[0];
            if dx > 0.5 && seen.insert(format!("len:{}", key_num(dx))) {
                horizontal.push(LinearCand {
                    source: DimensionRef::Linear {
                        a: PointRef::Center { feature: fref(l) },
                        b: PointRef::Center { feature: fref(r) },
                        dir: DimDirection::Horizontal,
                    },
                    span: dx * xf.scale,
                    side: 1.0,
                });
            }
            let (b, t) = (extreme(1, false), extreme(1, true));
            let dy = t.center[1] - b.center[1];
            if dy > 0.5 && seen.insert(format!("len:{}", key_num(dy))) {
                vertical.push(LinearCand {
                    source: DimensionRef::Linear {
                        a: PointRef::Center { feature: fref(b) },
                        b: PointRef::Center { feature: fref(t) },
                        dir: DimDirection::Vertical,
                    },
                    span: dy * xf.scale,
                    side: -1.0,
                });
            }
        }

        // d. Busur → radius; e. chamfer.
        for f in &view.features {
            match f {
                HlrGeometricFeature::Arc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                    edge,
                } => {
                    let sweep = (end_angle - start_angle).to_degrees();
                    if sweep < 20.0 {
                        continue;
                    }
                    // Busur dari lingkaran yang sudah berdimensi Ø tidak diberi R.
                    if seen.contains(&format!("dia:{}", key_num(radius * 2.0)))
                        || cyl_radii.iter().any(|x| (x - radius).abs() < 0.02)
                    {
                        continue;
                    }
                    if !seen.insert(format!("rad:{}", key_num(*radius))) {
                        continue;
                    }
                    let mid = ((start_angle + end_angle) * 0.5).to_degrees();
                    leaders.push(LeaderCand {
                        source: DimensionRef::Radius {
                            arc: FeatureRef {
                                view: vr,
                                edge: *edge,
                                center: *center,
                                radius: *radius,
                            },
                        },
                        angles: vec![mid, mid + 15.0, mid - 15.0, mid + 30.0, mid - 30.0],
                    });
                }
                HlrGeometricFeature::Chamfer {
                    start,
                    end,
                    length,
                    angle_deg,
                } => {
                    if !seen.insert(format!("chf:{}x{}", key_num(*length), key_num(*angle_deg))) {
                        continue;
                    }
                    let mid = [(start[0] + end[0]) * 0.5, (start[1] + end[1]) * 0.5];
                    leaders.push(LeaderCand {
                        source: DimensionRef::Chamfer {
                            edge: EdgeRef {
                                view: vr,
                                edge: None,
                                at: mid,
                            },
                        },
                        angles: away_angles(mid),
                    });
                }
                _ => {}
            }
        }

        place_linear(sheet, &mut placer, kind, horizontal, false, &mut out);
        place_linear(sheet, &mut placer, kind, vertical, true, &mut out);
        place_leaders(sheet, &mut placer, &xf, leaders, &mut out);

        // Fitur jalur mesh (tanpa rujukan): elips & sudut, gaya lama.
        append_legacy_features(&xf, &mut out);
    }
    out
}

fn place_linear(
    sheet: &DrawingSheet,
    placer: &mut Placer,
    kind: ProjectedViewKind,
    mut cands: Vec<LinearCand>,
    is_vertical: bool,
    out: &mut Vec<DimensionAnnotation>,
) {
    // Pendek di dalam, panjang di luar (ISO 129-1). Urutan stabil.
    cands.sort_by(|a, b| a.span.total_cmp(&b.span));
    let mut next_tier = [0usize, 0usize]; // [sisi +, sisi −]
    for cand in cands {
        let prefer = if cand.side >= 0.0 { 0 } else { 1 };
        // Sisi yang disukai dulu (3 tingkat), lalu sisi seberang, lalu bebas.
        let tries: Vec<(usize, usize)> = (0..3)
            .map(|t| (prefer, t))
            .chain((0..3).map(|t| (1 - prefer, t)))
            .chain((3..8).map(|t| (prefer, t)))
            .collect();
        let mut fallback: Option<(DimensionAnnotation, usize, usize)> = None;
        let mut chosen: Option<(DimensionAnnotation, usize, usize)> = None;
        for (side, extra) in tries {
            let tier = next_tier[side] + extra;
            let sign = if side == 0 { 1.0 } else { -1.0 };
            // Di bawah tampak ada judul tampak: mulai lebih jauh.
            let first = if !is_vertical && side == 1 {
                16.0
            } else {
                TIER_MM
            };
            let offset = sign * (first + TIER_MM * tier as f32);
            let Some(dim) = sheet.resolve_dimension(&cand.source, offset, 0.0) else {
                break;
            };
            if placer.is_free(&dim.text_box(), Some(kind)) {
                chosen = Some((dim, side, tier));
                break;
            }
            if fallback.is_none() {
                fallback = Some((dim, side, tier));
            }
        }
        if let Some((dim, side, tier)) = chosen.or(fallback) {
            placer.obstacles.push(grow(dim.text_box(), 0.3));
            next_tier[side] = tier + 1;
            out.push(dim);
        }
    }
}

fn place_leaders(
    sheet: &DrawingSheet,
    placer: &mut Placer,
    xf: &ViewXf<'_>,
    cands: Vec<LeaderCand>,
    out: &mut Vec<DimensionAnnotation>,
) {
    let bbox = grow(xf.bbox(), 5.0);
    for cand in cands {
        let mut fallback: Option<DimensionAnnotation> = None;
        let mut placed = None;
        'search: for angle in &cand.angles {
            let angle = angle.rem_euclid(360.0);
            // Titik panah tidak bergantung panjang leader.
            let Some(probe) = sheet.resolve_dimension(&cand.source, 3.0, angle) else {
                break;
            };
            let tip = probe.end;
            let (c, s) = (angle.to_radians().cos(), angle.to_radians().sin());
            // Panjang agar siku berada di luar kotak tampak.
            let mut exit = f32::MAX;
            if c > 1e-3 {
                exit = exit.min((bbox[2] - tip[0]) / c);
            } else if c < -1e-3 {
                exit = exit.min((bbox[0] - tip[0]) / c);
            }
            if s > 1e-3 {
                exit = exit.min((bbox[3] - tip[1]) / s);
            } else if s < -1e-3 {
                exit = exit.min((bbox[1] - tip[1]) / s);
            }
            let exit = exit.clamp(4.0, 400.0);
            for extra in [0.0, 6.0, 12.0] {
                let Some(dim) = sheet.resolve_dimension(&cand.source, exit + extra, angle) else {
                    break 'search;
                };
                if placer.is_free(&dim.text_box(), None) {
                    placed = Some(dim);
                    break 'search;
                }
                if fallback.is_none() {
                    fallback = Some(dim);
                }
            }
        }
        if let Some(dim) = placed.or(fallback) {
            placer.obstacles.push(grow(dim.text_box(), 0.3));
            out.push(dim);
        }
    }
}

fn append_legacy_features(xf: &ViewXf<'_>, out: &mut Vec<DimensionAnnotation>) {
    for feat in &xf.view.features {
        match feat {
            HlrGeometricFeature::Ellipse {
                center,
                radius_x,
                radius_y,
                ..
            } => {
                let a = 40.0f32.to_radians();
                let tip = xf.to_paper([
                    center[0] + radius_x * a.cos(),
                    center[1] + radius_y * a.sin(),
                ]);
                out.push(DimensionAnnotation {
                    start: tip,
                    end: tip,
                    line_pos: [tip[0] + 6.0, tip[1] + 4.0],
                    text: format!("Rx {} / Ry {}", fmt_dim(*radius_x), fmt_dim(*radius_y)),
                    style: DimStyle::Leader,
                    view: Some(xf.kind),
                    ..DimensionAnnotation::default()
                });
            }
            HlrGeometricFeature::Angle {
                vertex,
                arm1_end,
                angle_deg,
                ..
            } => {
                let v = xf.to_paper(*vertex);
                let a1 = xf.to_paper(*arm1_end);
                out.push(DimensionAnnotation {
                    start: v,
                    end: a1,
                    line_pos: [(v[0] + a1[0]) * 0.5, (v[1] + a1[1]) * 0.5 + 4.0],
                    text: format!("{}°", fmt_dim(*angle_deg)),
                    style: DimStyle::Angle,
                    view: Some(xf.kind),
                    ..DimensionAnnotation::default()
                });
            }
            _ => {}
        }
    }
}
