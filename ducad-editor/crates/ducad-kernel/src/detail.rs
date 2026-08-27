//! Ekstraksi Tampak Detail Pembesar (Detail View) dengan Viewport Lingkaran.
//!
//! Mengimplementasikan kliping geometri 2D terhadap lingkaran ROI (*Region of Interest*)
//! dan faktor skala pembesar independen (2:1, 4:1, 5:1, 10:1) berstandar ISO 128 / ASME Y14.24.

use crate::hlr::{HlrGeometricFeature, HlrSegment2D, ProjectedView, ProjectedViewKind};

/// Anotasi indikator lingkaran pembesar detail pada tampak acuan (Parent View).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DetailIndicator {
    /// Huruf / label pengenal tampak detail (mis. 'B', 'C', 'D').
    pub label: char,
    /// Tampak acuan asal detail (Front, Top, Right, atau SectionAA).
    pub parent_view: ProjectedViewKind,
    /// Titik pusat lingkaran area detail pada koordinat 2D model tampak acuan (mm).
    pub center_2d: [f32; 2],
    /// Radius lingkaran area detail pada tampak acuan (mm).
    pub radius_mm: f32,
    /// Posisi label / panah penunjuk pada tampak acuan (mm).
    pub label_pos: [f32; 2],
}

impl DetailIndicator {
    pub fn new(
        label: char,
        parent_view: ProjectedViewKind,
        center_2d: [f32; 2],
        radius_mm: f32,
    ) -> Self {
        let label_pos = [
            center_2d[0] + radius_mm * std::f32::consts::FRAC_1_SQRT_2 + 6.0,
            center_2d[1] + radius_mm * std::f32::consts::FRAC_1_SQRT_2 + 4.0,
        ];
        Self {
            label,
            parent_view,
            center_2d,
            radius_mm: radius_mm.max(1.0),
            label_pos,
        }
    }
}

/// Data satu Tampak Detail Pembesar (Detail View).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DetailViewData {
    /// Indikator & metadata area pembesar detail.
    pub indicator: DetailIndicator,
    /// Proyeksi tampak detail hasil kliping lingkaran.
    pub view: ProjectedView,
    /// Faktor skala pembesar independen terhadap skala utama gambar (mis. 2.0 untuk 2:1, 5.0 untuk 5:1, 10.0 untuk 10:1).
    pub scale_multiplier: f32,
}

/// Algoritma pemotongan (*Clipping*) segmen garis 2D terhadap batas lingkaran presisi.
///
/// Mengembalikan segmen yang berada di dalam lingkaran `(center, radius)`.
/// Jika segmen sepenuhnya di luar, mengembalikan `None`.
pub fn clip_segment_to_circle(
    p1: [f32; 2],
    p2: [f32; 2],
    center: [f32; 2],
    radius: f32,
) -> Option<([f32; 2], [f32; 2])> {
    if radius <= 0.0 {
        return None;
    }

    let dx = p2[0] - p1[0];
    let dy = p2[1] - p1[1];
    let vx = p1[0] - center[0];
    let vy = p1[1] - center[1];

    let r_sq = radius * radius;
    let seg_len_sq = dx * dx + dy * dy;

    // Titik tunggal (panjang 0)
    if seg_len_sq < 1e-8 {
        let dist_sq = vx * vx + vy * vy;
        if dist_sq <= r_sq {
            return Some((p1, p2));
        } else {
            return None;
        }
    }

    // Persamaan kuadrat perpotongan garis parametrik P(t) = P1 + t * (P2 - P1) dengan lingkaran:
    // ||P(t) - C||^2 <= R^2
    // => (dx^2 + dy^2)*t^2 + 2*(vx*dx + vy*dy)*t + (vx^2 + vy^2 - R^2) <= 0
    let a = seg_len_sq;
    let b = 2.0 * (vx * dx + vy * dy);
    let c = (vx * vx + vy * vy) - r_sq;

    let discriminant = b * b - 4.0 * a * c;

    if discriminant < 0.0 {
        // Tidak ada perpotongan dengan batas lingkaran
        // Jika c <= 0, titik awal berada di dalam lingkaran (seluruh segmen di dalam)
        if c <= 0.0 {
            return Some((p1, p2));
        } else {
            return None;
        }
    }

    let sqrt_d = discriminant.sqrt();
    let t1 = (-b - sqrt_d) / (2.0 * a);
    let t2 = (-b + sqrt_d) / (2.0 * a);

    let (t_min, t_max) = if t1 <= t2 { (t1, t2) } else { (t2, t1) };

    // Irisan interval [t_min, t_max] dengan segmen asli [0.0, 1.0]
    let t_start = t_min.max(0.0);
    let t_end = t_max.min(1.0);

    if t_start >= t_end {
        return None;
    }

    let clipped_p1 = [p1[0] + t_start * dx, p1[1] + t_start * dy];
    let clipped_p2 = [p1[0] + t_end * dx, p1[1] + t_end * dy];

    Some((clipped_p1, clipped_p2))
}

/// Mesin ekstraksi Tampak Detail (Detail View Extractor).
pub struct DetailExtractor;

impl DetailExtractor {
    /// Mengekstrak tampak detail dari suatu tampak acuan dengan memotong garis-garis dalam lingkaran ROI.
    pub fn extract_detail_view(
        parent_view: &ProjectedView,
        indicator: &DetailIndicator,
        scale_multiplier: f32,
    ) -> DetailViewData {
        let center = indicator.center_2d;
        let r = indicator.radius_mm;
        let scale_mult = scale_multiplier.max(0.1);

        // 1. Klip seluruh segmen garis tampak, tersembunyi, siluet, dan arsir
        let mut clipped_segments = Vec::new();
        for seg in &parent_view.segments {
            if let Some((cp1, cp2)) = clip_segment_to_circle(seg.start, seg.end, center, r) {
                clipped_segments.push(HlrSegment2D {
                    start: cp1,
                    end: cp2,
                    kind: seg.kind,
                });
            }
        }

        // 2. Klip garis sumbu (centerlines)
        let mut clipped_centerlines = Vec::new();
        for cl in &parent_view.centerlines {
            if let Some((cp1, cp2)) = clip_segment_to_circle(cl.start, cl.end, center, r) {
                clipped_centerlines.push(HlrSegment2D {
                    start: cp1,
                    end: cp2,
                    kind: cl.kind,
                });
            }
        }

        // 3. Filter fitur geometri yang jatuh di dalam atau bersinggungan dengan lingkaran detail
        let mut detail_features = Vec::new();
        for feat in &parent_view.features {
            match feat {
                HlrGeometricFeature::Circle { center: c, radius } => {
                    let d = ((c[0] - center[0]).powi(2) + (c[1] - center[1]).powi(2)).sqrt();
                    if d <= r + radius {
                        detail_features.push(feat.clone());
                    }
                }
                HlrGeometricFeature::Arc {
                    center: c, radius, ..
                } => {
                    let d = ((c[0] - center[0]).powi(2) + (c[1] - center[1]).powi(2)).sqrt();
                    if d <= r + radius {
                        detail_features.push(feat.clone());
                    }
                }
                HlrGeometricFeature::Ellipse {
                    center: c,
                    radius_x,
                    radius_y,
                    ..
                } => {
                    let max_r = radius_x.max(*radius_y);
                    let d = ((c[0] - center[0]).powi(2) + (c[1] - center[1]).powi(2)).sqrt();
                    if d <= r + max_r {
                        detail_features.push(feat.clone());
                    }
                }
                HlrGeometricFeature::Angle { vertex, .. } => {
                    let d = ((vertex[0] - center[0]).powi(2) + (vertex[1] - center[1]).powi(2))
                        .sqrt();
                    if d <= r {
                        detail_features.push(feat.clone());
                    }
                }
            }
        }

        let bounds_min = [center[0] - r, center[1] - r];
        let bounds_max = [center[0] + r, center[1] + r];

        let detail_proj_view = ProjectedView {
            kind: ProjectedViewKind::Detail(indicator.label),
            title: format!("DETAIL {}", indicator.label),
            bounds_min,
            bounds_max,
            segments: clipped_segments,
            centerlines: clipped_centerlines,
            features: detail_features,
            width_mm: r * 2.0,
            height_mm: r * 2.0,
            depth_mm: parent_view.depth_mm,
        };

        DetailViewData {
            indicator: indicator.clone(),
            view: detail_proj_view,
            scale_multiplier: scale_mult,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hlr::HlrLineKind;

    #[test]
    fn test_clip_segment_inside_circle() {
        let center = [10.0, 10.0];
        let radius = 10.0;

        // Segmen sepenuhnya di dalam
        let p1 = [8.0, 8.0];
        let p2 = [12.0, 12.0];
        let clipped = clip_segment_to_circle(p1, p2, center, radius);
        assert!(clipped.is_some());
        let (c1, c2) = clipped.unwrap();
        assert_eq!(c1, p1);
        assert_eq!(c2, p2);
    }

    #[test]
    fn test_clip_segment_outside_circle() {
        let center = [0.0, 0.0];
        let radius = 5.0;

        // Segmen di luar
        let p1 = [10.0, 10.0];
        let p2 = [20.0, 20.0];
        let clipped = clip_segment_to_circle(p1, p2, center, radius);
        assert!(clipped.is_none());
    }

    #[test]
    fn test_clip_segment_crossing_circle() {
        let center = [0.0, 0.0];
        let radius = 5.0;

        // Garis horizontal memotong lingkaran dari -10 ke +10
        let p1 = [-10.0, 0.0];
        let p2 = [10.0, 0.0];
        let clipped = clip_segment_to_circle(p1, p2, center, radius);
        assert!(clipped.is_some());
        let (c1, c2) = clipped.unwrap();
        assert!((c1[0] - (-5.0)).abs() < 1e-4);
        assert!((c1[1] - 0.0).abs() < 1e-4);
        assert!((c2[0] - 5.0).abs() < 1e-4);
        assert!((c2[1] - 0.0).abs() < 1e-4);
    }

    #[test]
    fn test_extract_detail_view() {
        let parent = ProjectedView {
            kind: ProjectedViewKind::Front,
            title: "TAMPAK DEPAN".to_string(),
            bounds_min: [0.0, 0.0],
            bounds_max: [100.0, 50.0],
            segments: vec![
                HlrSegment2D {
                    start: [0.0, 0.0],
                    end: [100.0, 0.0],
                    kind: HlrLineKind::Visible,
                },
                HlrSegment2D {
                    start: [50.0, 0.0],
                    end: [50.0, 50.0],
                    kind: HlrLineKind::Visible,
                },
            ],
            centerlines: vec![HlrSegment2D {
                start: [50.0, -10.0],
                end: [50.0, 60.0],
                kind: HlrLineKind::Centerline,
            }],
            features: vec![HlrGeometricFeature::Circle {
                center: [50.0, 25.0],
                radius: 5.0,
            }],
            width_mm: 100.0,
            height_mm: 50.0,
            depth_mm: 30.0,
        };

        let indicator = DetailIndicator::new(
            'B',
            ProjectedViewKind::Front,
            [50.0, 25.0], // Titik pusat di perpotongan garis
            10.0,         // Radius 10mm
        );

        let detail_data = DetailExtractor::extract_detail_view(&parent, &indicator, 2.0);
        assert_eq!(detail_data.indicator.label, 'B');
        assert_eq!(detail_data.scale_multiplier, 2.0);
        assert_eq!(detail_data.view.kind, ProjectedViewKind::Detail('B'));
        assert_eq!(detail_data.view.title, "DETAIL B");

        // Verifikasi garis terkliping
        assert!(!detail_data.view.segments.is_empty());
        assert!(!detail_data.view.centerlines.is_empty());
        assert_eq!(detail_data.view.features.len(), 1);
    }
}
