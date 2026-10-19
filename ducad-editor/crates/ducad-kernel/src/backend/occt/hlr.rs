//! Bagian backend OCCT dari `crate::hlr` (PLAN_MIGRASI_TRUCK F2.2): satu tampak
//! lewat HLR eksak `hlr_sheet`. Pemanggil sudah memegang `lock_kernel()`.

use glam::Vec3;

use crate::hlr::{HlrLineKind, HlrSegment2D, ProjectedView, ProjectedViewKind};
use crate::shape::KernelShape;

/// Tampak eksak, atau `None` bila HLR eksak gagal (pemanggil jatuh ke HLR
/// mesh dan mencatat `HLR_EXACT_FALLBACK`).
pub(crate) fn exact_projected_view(
    shapes: &[&KernelShape],
    sketch_segments: &[(Vec3, Vec3)],
    view_kind: ProjectedViewKind,
    model_bbox: ([f32; 3], [f32; 3]),
    warnings: &mut Vec<String>,
) -> Option<ProjectedView> {
    let (_, right_vec, up_vec) = view_kind.camera_vectors();
    let occ: Vec<&opencascade::primitives::Shape> = shapes.iter().map(|s| s.inner()).collect();
    let sources = super::hlr_sheet::source_edges(&occ);
    let started = std::time::Instant::now();
    if let Some(mut lines) =
        super::hlr_sheet::exact_lines(&occ, right_vec, up_vec, true, Some(&sources))
    {
        if started.elapsed().as_secs_f32() > 10.0 {
            warnings.push(format!(
                "HLR_EXACT_SLOW: {} took {:.1} s",
                view_kind.title_en(),
                started.elapsed().as_secs_f32()
            ));
        }
        for (p1, p2) in sketch_segments {
            let a = [p1.dot(right_vec), p1.dot(up_vec)];
            let b = [p2.dot(right_vec), p2.dot(up_vec)];
            if (a[0] - b[0]).hypot(a[1] - b[1]) > 0.02 {
                lines.segments.push(HlrSegment2D {
                    start: a,
                    end: b,
                    kind: HlrLineKind::Visible,
                });
                lines.edge_refs.push(None);
            }
        }
        let diag = (Vec3::from_array(model_bbox.1) - Vec3::from_array(model_bbox.0)).length();
        let (features, centerlines) =
            super::hlr_sheet::exact_features(&occ, &lines, right_vec, up_vec, diag);
        let (bounds_min, bounds_max) =
            super::hlr_sheet::lines_bounds(&lines).unwrap_or(([0.0, 0.0], [100.0, 100.0]));
        return Some(ProjectedView {
            kind: view_kind,
            title: view_kind.title_id(),
            bounds_min,
            bounds_max,
            segments: lines.segments,
            centerlines,
            features,
            width_mm: (model_bbox.1[0] - model_bbox.0[0]).abs(),
            height_mm: (model_bbox.1[2] - model_bbox.0[2]).abs(),
            depth_mm: (model_bbox.1[1] - model_bbox.0[1]).abs(),
            arcs: lines.arcs,
            edge_refs: lines.edge_refs,
            exact: true,
        });
    }
    None
}
