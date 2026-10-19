//! Bagian backend OCCT dari `crate::hlr_exact` (PLAN_MIGRASI_TRUCK F2.2): kode di
//! sini menyentuh `opencascade`; tipe & logika netral-backend ada di modul induk.

// Tipe & helper netral-backend dari modul induk.
#[allow(unused_imports)]
use crate::hlr_exact::*;

use anyhow::{bail, Result};
use glam::DVec3;
use opencascade::primitives::EdgeType;
use crate::hlr_exact::{ExactCurve2D, ExactCurveType, ExactHlrView, ExactLineKind};
use crate::lock_kernel;
use crate::shape::KernelShape;

fn curve_type(t: EdgeType) -> ExactCurveType {
    match t {
        EdgeType::Line => ExactCurveType::Line,
        EdgeType::Circle => ExactCurveType::Circle,
        EdgeType::Ellipse => ExactCurveType::Ellipse,
        EdgeType::Hyperbola => ExactCurveType::Hyperbola,
        EdgeType::Parabola => ExactCurveType::Parabola,
        EdgeType::BezierCurve => ExactCurveType::BezierCurve,
        EdgeType::BSplineCurve => ExactCurveType::BSplineCurve,
        EdgeType::OffsetCurve => ExactCurveType::OffsetCurve,
        EdgeType::OtherCurve => ExactCurveType::OtherCurve,
    }
}

/// Jalankan HLR eksak pada `shape` dari arah pandang `view_dir`.
///
/// `up` menentukan orientasi vertikal gambar; ia tidak boleh sejajar dengan
/// arah pandang.
pub fn extract_exact_hlr(
    shape: &KernelShape,
    view_dir: (f64, f64, f64),
    up: (f64, f64, f64),
) -> Result<ExactHlrView> {
    let _guard = lock_kernel();
    let dir = DVec3::new(view_dir.0, view_dir.1, view_dir.2);
    let up_v = DVec3::new(up.0, up.1, up.2);
    if dir.length() < 1e-9 || up_v.length() < 1e-9 {
        bail!("arah pandang dan vektor atas HLR tidak boleh nol");
    }
    if dir.normalize().cross(up_v.normalize()).length() < 1e-6 {
        bail!("vektor atas HLR tidak boleh sejajar arah pandang");
    }

    let Some((vis_sharp, vis_out, hid_sharp, hid_out)) =
        shape.inner().hidden_line_removal(dir, up_v)
    else {
        bail!("HLR gagal untuk geometri ini");
    };

    let mut curves = Vec::new();
    for (compound, kind) in [
        (vis_sharp, ExactLineKind::VisibleSharp),
        (vis_out, ExactLineKind::VisibleOutline),
        (hid_sharp, ExactLineKind::HiddenSharp),
        (hid_out, ExactLineKind::HiddenOutline),
    ] {
        for edge in compound.edges() {
            // Hasil HLR berada di koordinat proyektor: bidang gambar adalah
            // XY dan Z cuma kedalaman, jadi Z dibuang.
            let pts: Vec<(f64, f64)> = edge
                .approximation_segments()
                .map(|p| (p.x, p.y))
                .collect();
            if pts.len() < 2 {
                continue;
            }
            curves.push(ExactCurve2D {
                kind,
                curve: curve_type(edge.edge_type()),
                points: pts,
            });
        }
    }
    Ok(ExactHlrView { curves })
}
