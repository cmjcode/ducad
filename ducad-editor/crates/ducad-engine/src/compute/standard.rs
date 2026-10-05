//! Geometri part standar (P20) dari `ducad_core::StandardShape`, dan ulir
//! fisik pada face silinder. Sumbu part = +Z; bidang dudukan di z = 0.

use ducad_core::StandardShape;
use ducad_kernel::{FaceInfo, KernelShape, Profile, ProfileSegment, SurfaceKind};

use super::finish;
use crate::error::{OpError, OpResult};
use crate::model::BodyGeometry;

fn k(e: anyhow::Error) -> OpError {
    OpError::kernel("StandardPart", e)
}

fn cylinder(d: f64, h: f64, z0: f64) -> OpResult<KernelShape> {
    let c = ducad_kernel::make_cylinder(d / 2.0, h).map_err(k)?;
    ducad_kernel::translate_shape(&c, 0.0, 0.0, z0).map_err(k)
}

/// Prisma heksagon selebar kunci `across_flats`, z = z0..z0 + h.
fn hex_prism(across_flats: f64, h: f64, z0: f64) -> OpResult<KernelShape> {
    let r = across_flats / 3.0_f64.sqrt();
    let pts: Vec<(f64, f64)> = (0..6)
        .map(|i| {
            let a = std::f64::consts::FRAC_PI_3 * i as f64;
            (r * a.cos(), r * a.sin())
        })
        .collect();
    let profile = Profile::Loop(
        (0..6)
            .map(|i| ProfileSegment::Line {
                start: pts[i],
                end: pts[(i + 1) % 6],
            })
            .collect(),
    );
    let prism = ducad_kernel::extrude_profile(&profile, h).map_err(k)?;
    ducad_kernel::translate_shape(&prism, 0.0, 0.0, z0).map_err(k)
}

/// Bangun solid part standar, lalu geser ke `at`.
pub fn standard_part(shape: &StandardShape, at: [f64; 3]) -> OpResult<BodyGeometry> {
    let solid = match *shape {
        StandardShape::CapScrew {
            d,
            length,
            head_d,
            head_h,
            socket_af,
            socket_depth,
        } => {
            let shank = cylinder(d, length, -length)?;
            let head = cylinder(head_d, head_h, 0.0)?;
            let body = ducad_kernel::union(&shank, &head).map_err(k)?;
            // Soket sedikit menembus puncak supaya boolean tidak berimpit.
            let socket = hex_prism(socket_af, socket_depth + 0.5, head_h - socket_depth)?;
            ducad_kernel::subtract(&body, &socket).map_err(k)?
        }
        StandardShape::HexBolt {
            d,
            length,
            across_flats,
            head_h,
        } => {
            let shank = cylinder(d, length, -length)?;
            let head = hex_prism(across_flats, head_h, 0.0)?;
            ducad_kernel::union(&shank, &head).map_err(k)?
        }
        StandardShape::HexNut {
            d,
            across_flats,
            height,
        } => {
            let prism = hex_prism(across_flats, height, 0.0)?;
            let bore = cylinder(d, height + 1.0, -0.5)?;
            ducad_kernel::subtract(&prism, &bore).map_err(k)?
        }
        StandardShape::Ring {
            inner_d,
            outer_d,
            height,
        } => {
            let outer = cylinder(outer_d, height, 0.0)?;
            let bore = cylinder(inner_d, height + 1.0, -0.5)?;
            ducad_kernel::subtract(&outer, &bore).map_err(k)?
        }
        StandardShape::Pin { d, length } => cylinder(d, length, 0.0)?,
    };
    let placed = if at.iter().any(|c| c.abs() > 1e-12) {
        ducad_kernel::translate_shape(&solid, at[0], at[1], at[2]).map_err(k)?
    } else {
        solid
    };
    finish("StandardPart", placed)
}

/// Rentang sebuah face silinder di sepanjang sumbunya: `(titik awal, arah
/// satuan, panjang, diameter)`.
pub fn cylinder_span(face: &FaceInfo) -> OpResult<([f64; 3], [f64; 3], f64, f64)> {
    let (Some((point, dir)), Some(radius), SurfaceKind::Cylinder) =
        (face.axis, face.radius, face.kind)
    else {
        return Err(OpError::invalid(format!(
            "face #{} bukan silinder; ulir butuh face silinder",
            face.index
        ))
        .with_hint(
            "select the shank with e.g. all[kind=cylinder][r=5] and test it with query_geometry",
        ));
    };
    // Proyeksikan delapan sudut bbox face ke sumbu.
    let (lo, hi) = face.bbox;
    let (mut tmin, mut tmax) = (f64::MAX, f64::MIN);
    for i in 0..8 {
        let corner = [
            if i & 1 == 0 { lo[0] } else { hi[0] },
            if i & 2 == 0 { lo[1] } else { hi[1] },
            if i & 4 == 0 { lo[2] } else { hi[2] },
        ];
        let t = (0..3).map(|a| (corner[a] - point[a]) * dir[a]).sum::<f64>();
        tmin = tmin.min(t);
        tmax = tmax.max(t);
    }
    // Sudut bbox menjorok sejauh radius di luar ujung silinder bila sumbunya
    // miring; untuk sumbu sejajar sumbu koordinat nilainya eksak.
    let start = [
        point[0] + dir[0] * tmin,
        point[1] + dir[1] * tmin,
        point[2] + dir[2] * tmin,
    ];
    Ok((start, dir, tmax - tmin, radius * 2.0))
}

/// Kisar ulir kasar ISO untuk diameter nominal `d` (M2–M12).
pub fn coarse_pitch(d: f64) -> Option<f64> {
    ducad_core::IsoMetricThread::all().iter().find_map(|t| {
        if matches!(t, ducad_core::IsoMetricThread::Custom) {
            return None;
        }
        let (nominal, pitch, ..) = t.standard_params();
        ((nominal - d).abs() < 0.05).then_some(pitch)
    })
}

/// Potong ulir fisik ISO pada silinder luar.
#[allow(clippy::too_many_arguments)]
pub fn thread(
    shape: &KernelShape,
    start: [f64; 3],
    dir: [f64; 3],
    major_d: f64,
    pitch: f64,
    length: f64,
    left_handed: bool,
) -> OpResult<BodyGeometry> {
    let cut = ducad_kernel::cut_iso_thread(shape, start, dir, major_d, pitch, length, left_handed)
        .map_err(|e| OpError::kernel("Thread", e))?;
    finish("Thread", cut)
}
