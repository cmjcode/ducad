//! Sheet metal (P19): solid terlipat dan pola datar dari
//! `ducad_core::SheetMetalModel`.
//!
//! Batasan versi ini (dilaporkan sebagai error yang jelas, bukan hasil
//! diam-diam salah): pelat dasar harus poligon bersisi lurus; flange hanya
//! pada sisi pelat dasar yang masih bebas (belum ada flange-di-atas-flange);
//! flange selalu selebar sisi itu; `relief` dicatat tetapi belum memotong
//! geometri (tidak dibutuhkan selama flange selebar sisi pada sudut cembung).

use ducad_core::{Flange, SectionSegment, SheetMetalModel};
use ducad_kernel::{EdgeInfo, KernelShape, Profile, ProfileSegment};

use super::finish;
use crate::error::{OpError, OpResult};
use crate::model::BodyGeometry;
use crate::plane::PlaneFrame;

type V3 = [f64; 3];

fn axpy(a: V3, x: V3, k: f64) -> V3 {
    [a[0] + x[0] * k, a[1] + x[1] * k, a[2] + x[2] * k]
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Titik bidang `(u, v)` pada ketinggian `h` di atas bidang sketsa.
pub fn to_world(frame: &PlaneFrame, p: [f64; 2], h: f64) -> V3 {
    let on_plane = axpy(axpy(frame.origin, frame.u_axis, p[0]), frame.v_axis, p[1]);
    axpy(on_plane, frame.normal, h)
}

/// Poligon dasar dari profil sketsa: hanya loop garis lurus, dikembalikan
/// berlawanan arah jarum jam.
pub fn outline_from_profile(profile: &Profile) -> OpResult<Vec<[f64; 2]>> {
    let Profile::Loop(segments) = profile else {
        return Err(OpError::invalid(
            "base_flange butuh sketsa poligon bersisi lurus (lingkaran/elips/profil berlubang belum didukung)",
        ));
    };
    let mut points = Vec::with_capacity(segments.len());
    for seg in segments {
        match seg {
            ProfileSegment::Line { start, .. } => points.push([start.0, start.1]),
            _ => {
                return Err(OpError::invalid(
                    "base_flange butuh sketsa poligon bersisi lurus (busur/kurva belum didukung)",
                ))
            }
        }
    }
    if ducad_core::sheet_metal::signed_area(&points) < 0.0 {
        points.reverse();
    }
    Ok(points)
}

fn polygon_profile(points: &[[f64; 2]]) -> Profile {
    let n = points.len();
    Profile::Loop(
        (0..n)
            .map(|i| ProfileSegment::Line {
                start: (points[i][0], points[i][1]),
                end: (points[(i + 1) % n][0], points[(i + 1) % n][1]),
            })
            .collect(),
    )
}

fn plate(points: &[[f64; 2]], frame: &PlaneFrame, thickness: f64) -> OpResult<KernelShape> {
    ducad_kernel::extrude_profile_on_plane(
        &polygon_profile(points),
        frame.origin,
        frame.u_axis,
        frame.v_axis,
        frame.normal,
        thickness,
    )
    .map_err(|e| OpError::kernel("SheetMetal", e))
}

/// Solid satu flange: penampang `(s, h)` di-extrude sepanjang sisi pelat.
fn flange_solid(
    model: &SheetMetalModel,
    frame: &PlaneFrame,
    flange: &Flange,
) -> OpResult<KernelShape> {
    let (a, b, out) = model.edge_frame(flange.edge).ok_or_else(|| {
        OpError::invalid(format!("sisi {} pelat dasar berpanjang nol", flange.edge))
    })?;
    let (a3, b3) = (to_world(frame, a, 0.0), to_world(frame, b, 0.0));
    let length = dot(axpy(b3, a3, -1.0), axpy(b3, a3, -1.0)).sqrt();
    let edge_dir = {
        let d = axpy(b3, a3, -1.0);
        [d[0] / length, d[1] / length, d[2] / length]
    };
    // Arah keluar di dunia (tanpa origin).
    let out3 = axpy(axpy([0.0; 3], frame.u_axis, out[0]), frame.v_axis, out[1]);
    let segments: Vec<ProfileSegment> = model
        .cross_section(flange)
        .into_iter()
        .map(|s| match s {
            SectionSegment::Line { start, end } => ProfileSegment::Line {
                start: (start[0], start[1]),
                end: (end[0], end[1]),
            },
            SectionSegment::Arc { start, via, end } => ProfileSegment::Arc {
                start: (start[0], start[1]),
                via: (via[0], via[1]),
                end: (end[0], end[1]),
            },
        })
        .collect();
    // Bidang penampang (s = keluar, h = normal pelat) di-extrude searah
    // `u × v` supaya orientasinya tangan-kanan: mulai dari ujung sisi yang
    // membuat arah itu menunjuk ke sepanjang sisi.
    let sweep = cross(out3, frame.normal);
    let (origin, normal) = if dot(sweep, edge_dir) > 0.0 {
        (a3, edge_dir)
    } else {
        (b3, [-edge_dir[0], -edge_dir[1], -edge_dir[2]])
    };
    ducad_kernel::extrude_profile_on_plane(
        &Profile::Loop(segments),
        origin,
        out3,
        frame.normal,
        normal,
        length,
    )
    .map_err(|e| OpError::kernel("SheetMetal", e))
}

/// Solid terlipat: pelat dasar ∪ semua flange.
pub fn folded(model: &SheetMetalModel, frame: &PlaneFrame) -> OpResult<BodyGeometry> {
    model.validate().map_err(OpError::invalid)?;
    let mut shape = plate(&model.outline, frame, model.thickness)?;
    for flange in &model.flanges {
        let solid = flange_solid(model, frame, flange)?;
        shape =
            ducad_kernel::union(&shape, &solid).map_err(|e| OpError::kernel("SheetMetal", e))?;
    }
    finish("SheetMetal", shape)
}

/// Solid pola datar (pelat dasar ∪ strip flange) di bidang sketsa yang sama.
pub fn flat(model: &SheetMetalModel, frame: &PlaneFrame) -> OpResult<BodyGeometry> {
    let pattern = model.flat_pattern().map_err(OpError::invalid)?;
    let mut polygons = pattern.polygons.iter();
    let base = polygons
        .next()
        .ok_or_else(|| OpError::invalid("pola datar kosong"))?;
    let mut shape = plate(base, frame, model.thickness)?;
    for strip in polygons {
        let mut pts = strip.clone();
        if ducad_core::sheet_metal::signed_area(&pts) < 0.0 {
            pts.reverse();
        }
        let solid = plate(&pts, frame, model.thickness)?;
        shape =
            ducad_kernel::union(&shape, &solid).map_err(|e| OpError::kernel("FlatPattern", e))?;
    }
    finish("FlatPattern", shape)
}

/// Cocokkan tepi terpilih dengan sisi pelat dasar (di muka bawah `h = 0`
/// atau muka atas `h = t`). Sisi yang sudah punya flange atau tepi yang
/// bukan sisi pelat dasar ditolak.
pub fn match_base_edges(
    model: &SheetMetalModel,
    frame: &PlaneFrame,
    edges: &[EdgeInfo],
) -> OpResult<Vec<usize>> {
    let n = model.outline.len();
    let near = |p: V3, q: V3| {
        let d = axpy(p, q, -1.0);
        dot(d, d).sqrt() < 1e-4
    };
    let mut out = Vec::new();
    for edge in edges {
        let found = (0..n).find(|&i| {
            [0.0, model.thickness].into_iter().any(|h| {
                let a = to_world(frame, model.outline[i], h);
                let b = to_world(frame, model.outline[(i + 1) % n], h);
                (near(edge.start, a) && near(edge.end, b))
                    || (near(edge.start, b) && near(edge.end, a))
            })
        });
        let Some(i) = found else {
            return Err(OpError::invalid(format!(
                "tepi #{} bukan sisi lurus pelat dasar yang masih bebas (flange di atas flange belum didukung)",
                edge.index
            ))
            .with_hint("pick an outer straight edge of the base flange, e.g. test `|X` or `|Y` with query_geometry"));
        };
        if model.flanges.iter().any(|f| f.edge == i) {
            return Err(OpError::invalid(format!(
                "sisi {i} pelat dasar sudah punya flange '{}'",
                model
                    .flanges
                    .iter()
                    .find(|f| f.edge == i)
                    .map(|f| f.id.as_str())
                    .unwrap_or_default()
            )));
        }
        if !out.contains(&i) {
            out.push(i);
        }
    }
    Ok(out)
}
