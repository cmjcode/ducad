//! Bagian backend OCCT dari `crate::profile` (PLAN_MIGRASI_TRUCK F2.2): kode di
//! sini menyentuh `opencascade`; tipe & logika netral-backend ada di modul induk.

// Tipe & helper netral-backend dari modul induk.
#[allow(unused_imports)]
use crate::profile::*;

use anyhow::{bail, Result};
use glam::dvec3;
use opencascade::primitives::{Edge, Face, Wire};

pub(crate) fn build_wire(profile: &Profile) -> Result<Wire> {
    build_wire_at_z(profile, 0.0)
}

pub(crate) fn build_wire_on_plane(
    profile: &Profile,
    origin: [f64; 3],
    u_axis: [f64; 3],
    v_axis: [f64; 3],
    normal: [f64; 3],
) -> Result<Wire> {
    let to_3d = |p: (f64, f64)| -> glam::DVec3 {
        dvec3(
            origin[0] + u_axis[0] * p.0 + v_axis[0] * p.1,
            origin[1] + u_axis[1] * p.0 + v_axis[1] * p.1,
            origin[2] + u_axis[2] * p.0 + v_axis[2] * p.1,
        )
    };
    let norm = dvec3(normal[0], normal[1], normal[2]).normalize();

    match profile {
        Profile::Circle { center, radius } => {
            if *radius <= 0.0 {
                bail!("radius lingkaran profil harus > 0");
            }
            let c3 = to_3d(*center);
            let edge = Edge::circle(c3, norm, *radius);
            Ok(Wire::from_edges([&edge]))
        }
        Profile::Ellipse {
            center,
            radius_x,
            radius_y,
        } => {
            if *radius_x <= 0.0 || *radius_y <= 0.0 {
                bail!("radius ellips profil harus > 0");
            }
            let c3 = to_3d(*center);
            let u3 = dvec3(u_axis[0], u_axis[1], u_axis[2]).normalize();
            let v3 = dvec3(v_axis[0], v_axis[1], v_axis[2]).normalize();
            let (major_r, minor_r, x_dir) = if *radius_x >= *radius_y {
                (*radius_x, *radius_y, u3)
            } else {
                (*radius_y, *radius_x, v3)
            };
            let edge = Edge::ellipse(c3, norm, x_dir, major_r, minor_r);
            Ok(Wire::from_edges([&edge]))
        }
        Profile::WithHoles { .. } => {
            // Satu `Wire` adalah SATU kurva tertutup; lubang butuh beberapa
            // wire yang hanya bisa digabung di tingkat `Face`. Pemanggil yang
            // butuh lubang harus lewat `build_face_on_plane`.
            bail!("profil berlubang tidak bisa jadi wire tunggal — pakai build_face_on_plane")
        }
        Profile::Loop(segments) => {
            if segments.is_empty() {
                bail!("profil loop kosong");
            }
            let edges: Vec<Edge> = segments
                .iter()
                .filter_map(|s| match s {
                    ProfileSegment::Line { start, end } => {
                        let p0 = to_3d(*start);
                        let p1 = to_3d(*end);
                        if (p0 - p1).length() > 1e-4 {
                            Some(Edge::segment(p0, p1))
                        } else {
                            None
                        }
                    }
                    ProfileSegment::Arc { start, via, end } => {
                        let p0 = to_3d(*start);
                        let p1 = to_3d(*via);
                        let p2 = to_3d(*end);
                        if (p0 - p2).length() > 1e-4 {
                            Some(Edge::arc(p0, p1, p2))
                        } else {
                            None
                        }
                    }
                    ProfileSegment::Bezier { start, c1, c2, end } => {
                        let p0 = to_3d(*start);
                        let p1 = to_3d(*c1);
                        let p2 = to_3d(*c2);
                        let p3 = to_3d(*end);
                        // Ujung yang berimpit BELUM berarti degenerate — Bézier
                        // boleh menutup jadi tetes air selama titik kontrolnya
                        // menjauh. Yang ditolak hanya kurva yang keempat
                        // titiknya menumpuk, karena OCCT tak bisa membuat edge
                        // tanpa panjang.
                        if bezier_is_degenerate(p0, p1, p2, p3) {
                            None
                        } else {
                            Some(Edge::bezier([p0, p1, p2, p3]))
                        }
                    }
                })
                .collect();
            if edges.is_empty() {
                bail!("semua segmen loop degenerate");
            }
            Ok(Wire::from_edges(edges.iter()))
        }
    }
}

/// Bangun `Face` planar dari profil — satu-satunya jalur yang mendukung
/// [`Profile::WithHoles`].
///
/// Untuk profil tanpa lubang hasilnya identik dengan
/// `Face::from_wire(build_wire_on_plane(..))`; untuk profil berlubang,
/// batas dalam ditambahkan lewat `Face::from_wire_with_holes`, yang
/// membalik orientasi tiap wire lubang sehingga OCCT memperlakukannya
/// sebagai rongga, bukan sebagai face terpisah.
pub(crate) fn build_face_on_plane(
    profile: &Profile,
    origin: [f64; 3],
    u_axis: [f64; 3],
    v_axis: [f64; 3],
    normal: [f64; 3],
) -> Result<Face> {
    match profile {
        Profile::WithHoles { outer, holes } => {
            if outer.has_holes() || holes.iter().any(|h| h.has_holes()) {
                bail!("profil berlubang tidak boleh bersarang — pecah jadi beberapa profil");
            }
            let outer_wire = build_wire_on_plane(outer, origin, u_axis, v_axis, normal)?;
            // Wire lubang WAJIB berorientasi berlawanan dengan batas luar.
            // Dengan orientasi yang sama, `BRepBuilderAPI_MakeFace::Add`
            // memperlakukannya sebagai material tambahan, bukan rongga —
            // terbukti lewat test: plat 40x40x10 berlubang R5 menghasilkan
            // 16785 mm^3 (= 16000 + volume silindernya) alih-alih 15215.
            // Binding tidak meng-expose `Wire::Reversed()`, jadi
            // pembalikannya dilakukan di tingkat geometri profil.
            let hole_wires: Vec<Wire> = holes
                .iter()
                .map(|h| build_hole_wire_on_plane(h, origin, u_axis, v_axis, normal))
                .collect::<Result<_>>()?;
            if hole_wires.is_empty() {
                return Ok(Face::from_wire(&outer_wire));
            }
            Ok(Face::from_wire_with_holes(&outer_wire, &hole_wires))
        }
        simple => {
            let wire = build_wire_on_plane(simple, origin, u_axis, v_axis, normal)?;
            Ok(Face::from_wire(&wire))
        }
    }
}

/// Bangun wire batas DALAM dengan orientasi terbalik terhadap batas luar.
///
/// Caranya berbeda per bentuk karena arah wire ditentukan hal yang berbeda:
/// - `Loop`: arahnya berasal dari urutan segmen, jadi urutannya dibalik dan
///   tiap segmen ditukar ujungnya.
/// - `Circle`/`Ellipse`: arahnya berasal dari normal bidang yang diberikan
///   ke `Edge::circle`/`Edge::ellipse`, jadi cukup normalnya dinegasikan.
fn build_hole_wire_on_plane(
    profile: &Profile,
    origin: [f64; 3],
    u_axis: [f64; 3],
    v_axis: [f64; 3],
    normal: [f64; 3],
) -> Result<Wire> {
    let flipped = [-normal[0], -normal[1], -normal[2]];
    match profile {
        Profile::Loop(segments) => {
            let reversed: Vec<ProfileSegment> =
                segments.iter().rev().copied().map(reverse_profile_segment).collect();
            build_wire_on_plane(&Profile::Loop(reversed), origin, u_axis, v_axis, normal)
        }
        Profile::WithHoles { .. } => {
            bail!("profil berlubang tidak boleh jadi lubang — pecah jadi beberapa profil")
        }
        analytic => build_wire_on_plane(analytic, origin, u_axis, v_axis, flipped),
    }
}
/// Sama seperti `build_wire`, tapi diangkat ke ketinggian `z` — dipakai
/// `loft_profiles` untuk menempatkan profil ATAS di `z = height` sementara
/// profil BAWAH tetap di `z = 0` (sketch DUCAD cuma satu bidang XY, lihat
/// docs/PLAN.md — ini bukan workplane sungguhan, cuma translasi Z).
pub(crate) fn build_wire_at_z(profile: &Profile, z: f64) -> Result<Wire> {
    match profile {
        Profile::Circle { center, radius } => {
            if *radius <= 0.0 {
                bail!("radius lingkaran profil harus > 0");
            }
            let edge = Edge::circle(dvec3(center.0, center.1, z), dvec3(0.0, 0.0, 1.0), *radius);
            Ok(Wire::from_edges([&edge]))
        }
        Profile::Ellipse {
            center,
            radius_x,
            radius_y,
        } => {
            if *radius_x <= 0.0 || *radius_y <= 0.0 {
                bail!("radius ellips profil harus > 0");
            }
            let c3 = dvec3(center.0, center.1, z);
            let norm = dvec3(0.0, 0.0, 1.0);
            let (major_r, minor_r, x_dir) = if *radius_x >= *radius_y {
                (*radius_x, *radius_y, dvec3(1.0, 0.0, 0.0))
            } else {
                (*radius_y, *radius_x, dvec3(0.0, 1.0, 0.0))
            };
            let edge = Edge::ellipse(c3, norm, x_dir, major_r, minor_r);
            Ok(Wire::from_edges([&edge]))
        }
        Profile::WithHoles { .. } => {
            bail!("profil berlubang tidak bisa jadi wire tunggal — pakai build_face_on_plane")
        }
        Profile::Loop(segments) => {
            if segments.is_empty() {
                bail!("profil loop kosong");
            }
            let edges: Vec<Edge> = segments
                .iter()
                .filter_map(|s| match s {
                    ProfileSegment::Line { start, end } => {
                        let p0 = dvec3(start.0, start.1, z);
                        let p1 = dvec3(end.0, end.1, z);
                        if (p0 - p1).length() > 1e-4 {
                            Some(Edge::segment(p0, p1))
                        } else {
                            None
                        }
                    }
                    ProfileSegment::Arc { start, via, end } => {
                        let p0 = dvec3(start.0, start.1, z);
                        let p1 = dvec3(via.0, via.1, z);
                        let p2 = dvec3(end.0, end.1, z);
                        if (p0 - p2).length() > 1e-4 {
                            Some(Edge::arc(p0, p1, p2))
                        } else {
                            None
                        }
                    }
                    ProfileSegment::Bezier { start, c1, c2, end } => {
                        let p0 = dvec3(start.0, start.1, z);
                        let p1 = dvec3(c1.0, c1.1, z);
                        let p2 = dvec3(c2.0, c2.1, z);
                        let p3 = dvec3(end.0, end.1, z);
                        if bezier_is_degenerate(p0, p1, p2, p3) {
                            None
                        } else {
                            Some(Edge::bezier([p0, p1, p2, p3]))
                        }
                    }
                })
                .collect();
            if edges.is_empty() {
                bail!("semua segmen loop degenerate");
            }
            Ok(Wire::from_edges(edges.iter()))
        }
    }
}
pub(crate) fn build_spine_wire(segments: &[PathSegment]) -> Result<Wire> {
    if segments.is_empty() {
        bail!("jalur sweep kosong");
    }
    let mut edges: Vec<Edge> = Vec::new();
    for seg in segments {
        match seg {
            PathSegment::Line { start, end } => {
                edges.push(Edge::segment(
                    dvec3(start[0], start[1], start[2]),
                    dvec3(end[0], end[1], end[2]),
                ));
            }
            PathSegment::Arc { start, via, end } => {
                edges.push(Edge::arc(
                    dvec3(start[0], start[1], start[2]),
                    dvec3(via[0], via[1], via[2]),
                    dvec3(end[0], end[1], end[2]),
                ));
            }
            PathSegment::Polyline(pts) => {
                if pts.len() < 2 {
                    continue;
                }
                for window in pts.windows(2) {
                    edges.push(Edge::segment(
                        dvec3(window[0][0], window[0][1], window[0][2]),
                        dvec3(window[1][0], window[1][1], window[1][2]),
                    ));
                }
            }
        }
    }
    if edges.is_empty() {
        bail!("tidak ada edge valid pada jalur sweep");
    }
    Ok(Wire::from_edges(edges.iter()))
}
