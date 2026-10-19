//! Bagian backend OCCT dari `crate::section` (PLAN_MIGRASI_TRUCK F2.2): cabang
//! B-rep tampak potongan — prisma pemotong, boolean per body, arsir per face
//! potong, lalu HLR eksak sisa solid. Pemanggil sudah memegang `lock_kernel()`.

use glam::{dvec3, vec2, vec3, DVec3, Vec2, Vec3};
use opencascade::primitives::{IntoShape, Shape};

use crate::hlr::HlrLineKind;
use crate::picking::face::SurfaceKind;
use crate::profile::{Profile, ProfileSegment};
use crate::section::{
    generate_iso_hatch_pattern, loops_bounds, on_cut_plane, BrepCut, BrepCutInput, CutFrame,
};

/// `Ok(None)` bila tidak ada B-rep atau prisma pemotong gagal dibangun —
/// pemanggil lalu mengiris mesh.
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
    let occ: Vec<&Shape> = shapes.iter().map(|s| s.inner()).collect();
    if occ.is_empty() {
        return Ok(None);
    }
    let Ok(cutter) = build_cutter(frame, big) else {
        return Ok(None);
    };
    let mut segments = Vec::new();
    let mut arcs = Vec::new();
    let mut features = Vec::new();
    let mut centerlines = Vec::new();
    let mut cut_area = 0.0f64;
    let mut cut_length = 0.0f64;
    let mut is_exact = false;
    let mut warnings = Vec::new();
    // Potong tiap body sendiri-sendiri supaya arsirnya bisa berselang.
    let mut remains: Vec<Shape> = Vec::new();
    let mut hatched_bodies = 0usize;
    for shape in &occ {
        let cut = match shape.subtract(&cutter) {
            Ok(result) => result.shape,
            Err(_) => {
                warnings.push(format!(
                        "DRAWING_SECTION_FALLBACK: boolean cut failed for one body in section {label}; body drawn uncut"
                    ));
                remains.push(crate::shape::deep_clone(shape).map_err(|e| e.to_string())?);
                continue;
            }
        };
        // Arsir dihitung PER FACE potong: pada potongan bertingkat dua
        // face bertemu di garis siku, dan menggabung konturnya akan
        // merusak paritas even-odd di sepanjang garis itu.
        // ISO 128-50: body bersebelahan diarsir berselang 45°/135°.
        let angle = request.hatch_angle_deg + 90.0 * (hatched_bodies % 2) as f32;
        let mut body_hatched = false;
        for face in cut.faces() {
            if SurfaceKind::from(face.surface_kind().as_str()) != SurfaceKind::Plane {
                continue;
            }
            let n = face.normal_at_center();
            let n = vec3(n.x as f32, n.y as f32, n.z as f32).normalize_or_zero();
            // Face potong menghadap pemirsa.
            if n.dot(frame.d) > -0.999 {
                continue;
            }
            let c = face.center_of_mass();
            let rel = vec3(c.x as f32, c.y as f32, c.z as f32) - frame.origin;
            let (ct, cd) = (rel.dot(frame.t), rel.dot(frame.d));
            if !on_cut_plane(frame, ct, cd) {
                continue;
            }
            cut_area += face.surface_area();
            let mut loops: Vec<[Vec2; 2]> = Vec::new();
            for edge in face.edges() {
                let pts: Vec<Vec2> = edge
                    .approximation_segments()
                    .map(|p| {
                        let q = vec3(p.x as f32, p.y as f32, p.z as f32);
                        vec2(q.dot(right), q.dot(up))
                    })
                    .collect();
                for w in pts.windows(2) {
                    let len = (w[0] - w[1]).length();
                    if len > 1e-4 {
                        cut_length += len as f64;
                        loops.push([w[0], w[1]]);
                    }
                }
            }
            if !loops.is_empty() {
                body_hatched = true;
                let (lo, hi) = loops_bounds(&loops);
                segments.extend(generate_iso_hatch_pattern(
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
        if cut.faces().next().is_some() {
            remains.push(cut);
        }
    }

    let remain_refs: Vec<&Shape> = remains.iter().collect();
    let lines = if exact {
        // Garis tersembunyi ikut dihitung dulu: fitur lingkaran yang
        // sebagian tertutup butuh busur tersembunyinya. Dibuang lagi
        // sebelum digambar (tampak potongan tidak memuat garis putus).
        super::hlr_sheet::exact_lines(&remain_refs, right, up, true, None)
    } else {
        None
    };
    match lines {
        Some(mut lines) => {
            is_exact = true;
            // Siku potongan bertingkat tidak digambar di tampak potongan.
            let t2 = vec2(frame.t.dot(right), frame.t.dot(up));
            let base = vec2(frame.origin.dot(right), frame.origin.dot(up)).dot(t2);
            let steps: Vec<f32> = frame
                .pts
                .windows(2)
                .filter(|w| (w[0].0 - w[1].0).abs() < 1e-3 && (w[0].1 - w[1].1).abs() > 1e-3)
                .map(|w| base + w[0].0)
                .collect();
            if !steps.is_empty() && t2.length_squared() > 0.5 {
                lines.segments.retain(|s| {
                    let a = Vec2::from_array(s.start).dot(t2);
                    let b = Vec2::from_array(s.end).dot(t2);
                    !steps
                        .iter()
                        .any(|st| (a - st).abs() < 2e-3 && (b - st).abs() < 2e-3)
                });
            }
            let (f, c) = super::hlr_sheet::exact_features(&remain_refs, &lines, right, up, diag);
            features = f;
            centerlines = c;
            lines.segments.retain(|s| s.kind != HlrLineKind::Hidden);
            lines.arcs.retain(|a| a.kind != HlrLineKind::Hidden);
            segments.extend(lines.segments);
            arcs = lines.arcs;
        }
        None => {
            if exact && !remain_refs.is_empty() {
                warnings.push(format!(
                        "HLR_EXACT_FALLBACK: exact hidden-line removal failed for SECTION {label}-{label}; only the cut outline is drawn"
                    ));
            }
        }
    }
    Ok(Some(BrepCut {
        segments,
        arcs,
        features,
        centerlines,
        cut_area,
        cut_length,
        is_exact,
        warnings,
    }))
}

/// Prisma pemotong: seluruh ruang di sisi pemirsa garis potong.
fn build_cutter(frame: &CutFrame, big: f64) -> anyhow::Result<Shape> {
    let first = frame.pts[0];
    let last = frame.pts[frame.pts.len() - 1];
    let d_min = frame
        .pts
        .iter()
        .map(|p| p.1 as f64)
        .fold(f64::MAX, f64::min)
        - big;
    let mut poly: Vec<(f64, f64)> = Vec::new();
    poly.push((first.0 as f64 - big, first.1 as f64));
    for p in &frame.pts {
        poly.push((p.0 as f64, p.1 as f64));
    }
    poly.push((last.0 as f64 + big, last.1 as f64));
    poly.push((last.0 as f64 + big, d_min));
    poly.push((first.0 as f64 - big, d_min));
    // Buang titik berimpit berurutan.
    poly.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9);

    let mut segs = Vec::with_capacity(poly.len());
    for i in 0..poly.len() {
        segs.push(ProfileSegment::Line {
            start: poly[i],
            end: poly[(i + 1) % poly.len()],
        });
    }
    let to_d = |v: Vec3| dvec3(v.x as f64, v.y as f64, v.z as f64);
    let (t, d) = (to_d(frame.t), to_d(frame.d));
    let n = t.cross(d).normalize();
    let origin: DVec3 = to_d(frame.origin) - n * big;
    let face = crate::profile::build_face_on_plane(
        &Profile::Loop(segs),
        origin.to_array(),
        t.to_array(),
        d.to_array(),
        n.to_array(),
    )?;
    Ok(face.extrude(n * (2.0 * big)).into_shape())
}
