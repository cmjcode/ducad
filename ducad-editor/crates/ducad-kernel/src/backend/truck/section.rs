//! Bagian backend truck dari `crate::section` (PLAN_MIGRASI_TRUCK F4.3):
//! tampak potongan B-rep.
//!
//! 1. Tiap body dipotong sendiri-sendiri: potongan lurus lewat
//!    `solidcraft_kernel::half_space` (bidang eksak, tahan face yang
//!    berimpit dengan bidang potong); potongan bertingkat lewat prisma
//!    pemotong + subtract, sama seperti backend lama (≤ 0.5.x).
//! 2. Face potong (bidang menghadap pemirsa di garis potong) diarsir per
//!    face; body bersebelahan berselang 45°/135° (ISO 128-50).
//! 3. Sisa solid digambar dengan HLR hybrid ([`super::hlr_mesh`]).

use glam::{vec2, DVec3, Vec2, Vec3};
use solidcraft_geom::{Plane, Vec3 as SVec3};
use solidcraft_kernel::FaceSurface;

use super::bridge::to_dv;
use super::hlr_mesh;
use super::kernel;
use crate::hlr::HlrLineKind;
use crate::hlr_sheet::drop_section_steps;
use crate::profile::{Profile, ProfileSegment};
use crate::section::{
    generate_iso_hatch_pattern, loops_bounds, on_cut_plane, BrepCut, BrepCutInput, CutFrame,
};
use crate::shape::KernelShape;

fn sv(v: Vec3) -> SVec3 {
    SVec3::new(v.x as f64, v.y as f64, v.z as f64)
}

/// Potongan lurus = semua titik path pada `d` yang sama.
fn is_straight(frame: &CutFrame) -> bool {
    let d0 = frame.pts[0].1;
    frame.pts.iter().all(|p| (p.1 - d0).abs() < 1e-6)
}

/// Bagian body di sisi jauh garis potong (yang terlihat di tampak potongan).
fn cut_one(shape: &KernelShape, frame: &CutFrame, big: f64) -> anyhow::Result<KernelShape> {
    let Some(body) = shape.body() else {
        return Ok(KernelShape::empty());
    };
    if is_straight(frame) {
        let origin = frame.origin + frame.d * frame.pts[0].1;
        let plane = Plane::from_normal(sv(origin), sv(frame.d))
            .ok_or_else(|| anyhow::anyhow!("bidang potong tidak valid"))?;
        let (part, _) = kernel("section cut", || {
            solidcraft_kernel::half_space(body, &plane, true)
        })?;
        return Ok(part
            .map(KernelShape::from_body)
            .unwrap_or_else(KernelShape::empty));
    }
    let cutter = build_cutter(frame, big)?;
    crate::csg::subtract(shape, &cutter)
}

/// Prisma pemotong: seluruh ruang di sisi pemirsa garis potong.
fn build_cutter(frame: &CutFrame, big: f64) -> anyhow::Result<KernelShape> {
    let first = frame.pts[0];
    let last = frame.pts[frame.pts.len() - 1];
    let d_min = frame
        .pts
        .iter()
        .map(|p| p.1 as f64)
        .fold(f64::MAX, f64::min)
        - big;
    let mut poly: Vec<(f64, f64)> = vec![(first.0 as f64 - big, first.1 as f64)];
    poly.extend(frame.pts.iter().map(|p| (p.0 as f64, p.1 as f64)));
    poly.push((last.0 as f64 + big, last.1 as f64));
    poly.push((last.0 as f64 + big, d_min));
    poly.push((first.0 as f64 - big, d_min));
    poly.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9);
    let segs = (0..poly.len())
        .map(|i| ProfileSegment::Line {
            start: poly[i],
            end: poly[(i + 1) % poly.len()],
        })
        .collect();
    let d = |v: Vec3| [v.x as f64, v.y as f64, v.z as f64];
    let n = frame.t.cross(frame.d).normalize();
    let origin = frame.origin - n * big as f32;
    crate::csg::extrude_profile_on_plane(
        &Profile::Loop(segs),
        d(origin),
        d(frame.t),
        d(frame.d),
        d(n),
        2.0 * big,
    )
}

/// `Ok(None)` bila tidak ada B-rep — pemanggil lalu mengiris mesh.
pub(crate) fn cut_bodies(input: &BrepCutInput<'_>) -> Result<Option<BrepCut>, String> {
    let BrepCutInput {
        shapes,
        frame,
        request,
        right,
        up,
        big,
        diag,
        exact,
        label,
    } = *input;
    if !shapes.iter().any(|s| s.body().is_some()) {
        return Ok(None);
    }
    let mut out = BrepCut::default();
    let mut remains: Vec<KernelShape> = Vec::new();
    let mut hatched_bodies = 0usize;
    let view = DVec3::new(frame.d.x as f64, frame.d.y as f64, frame.d.z as f64);
    for shape in shapes.iter().filter(|s| s.body().is_some()) {
        let cut = match cut_one(shape, frame, big) {
            Ok(cut) => cut,
            Err(_) => {
                out.warnings.push(format!(
                    "DRAWING_SECTION_FALLBACK: boolean cut failed for one body in section {label}; body drawn uncut"
                ));
                remains.push(crate::shape::clone_shape(shape).map_err(|e| e.to_string())?);
                continue;
            }
        };
        if cut.body().is_none() {
            continue;
        }
        // Arsir PER FACE potong: pada potongan bertingkat dua face bertemu
        // di garis siku, dan menggabung konturnya merusak paritas even-odd.
        let angle = request.hatch_angle_deg + 90.0 * (hatched_bodies % 2) as f32;
        let mut body_hatched = false;
        let topo = cut.topology();
        for face in &topo.faces {
            let Some(FaceSurface::Plane { normal, .. }) = face.surface else {
                continue;
            };
            // Face potong menghadap pemirsa.
            if to_dv(*normal).dot(view) > -0.999 {
                continue;
            }
            let rel = face.centroid.as_vec3() - frame.origin;
            if !on_cut_plane(frame, rel.dot(frame.t), rel.dot(frame.d)) {
                continue;
            }
            out.cut_area += face.area;
            let mut loops: Vec<[Vec2; 2]> = Vec::new();
            for e in face.edges.iter().filter_map(|&e| topo.edges.get(e)) {
                let pts: Vec<Vec2> = e
                    .points
                    .iter()
                    .map(|p| {
                        let q = p.as_vec3();
                        vec2(q.dot(right), q.dot(up))
                    })
                    .collect();
                for w in pts.windows(2) {
                    let len = (w[0] - w[1]).length();
                    if len > 1e-4 {
                        out.cut_length += len as f64;
                        loops.push([w[0], w[1]]);
                    }
                }
            }
            if !loops.is_empty() {
                body_hatched = true;
                let (lo, hi) = loops_bounds(&loops);
                out.segments.extend(generate_iso_hatch_pattern(
                    &loops,
                    lo,
                    hi,
                    request.hatch_spacing,
                    angle,
                ));
            }
        }
        if body_hatched {
            hatched_bodies += 1;
        }
        remains.push(cut);
    }

    let refs: Vec<&KernelShape> = remains.iter().collect();
    // Garis tersembunyi ikut dihitung dulu: fitur lingkaran yang sebagian
    // tertutup butuh busur tersembunyinya. Dibuang sebelum digambar.
    let lines = if exact && !refs.is_empty() {
        hlr_mesh::exact_lines(&refs, right, up, true)
    } else {
        None
    };
    match lines {
        Some(mut lines) => {
            out.is_exact = true;
            drop_section_steps(&mut lines, frame, right, up);
            let (features, centerlines) = crate::hlr_sheet::exact_features(
                &hlr_mesh::sheet_faces(&refs),
                &lines,
                right,
                up,
                diag,
            );
            out.features = features;
            out.centerlines = centerlines;
            out.segments.extend(
                lines
                    .segments
                    .into_iter()
                    .filter(|s| s.kind != HlrLineKind::Hidden),
            );
            out.arcs = lines
                .arcs
                .into_iter()
                .filter(|a| a.kind != HlrLineKind::Hidden)
                .collect();
        }
        None => {
            if exact && !refs.is_empty() {
                out.warnings.push(format!(
                    "HLR_EXACT_FALLBACK: exact hidden-line removal failed for SECTION {label}-{label}; only the cut outline is drawn"
                ));
            }
        }
    }
    Ok(Some(out))
}
