use anyhow::{bail, Result};
use glam::dvec3;
use opencascade::primitives::{Edge, Wire};

/// Satu segmen loop profil 2D di bidang XY, dalam koordinat mentah (mm) —
/// bukan `glam::DVec2` supaya tidak membocorkan versi glam manapun ke
/// pemanggil (crate ini sengaja pin glam 0.23, lihat `Cargo.toml`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProfileSegment {
    Line { start: (f64, f64), end: (f64, f64) },
    /// Busur 3 titik: awal, titik-di-busur (menentukan sisi), akhir — sama
    /// konvensi dengan `ducad_sketch::arc_from_three_points`.
    Arc {
        start: (f64, f64),
        via: (f64, f64),
        end: (f64, f64),
    },
}

/// Profil 2D tertutup di bidang XY, siap di-extrude/revolve. Dibangun
/// pemanggil (biasanya `ducad-app` dari seleksi entitas sketch).
#[derive(Debug, Clone)]
pub enum Profile {
    /// Lingkaran penuh — jadi silinder saat di-extrude.
    Circle { center: (f64, f64), radius: f64 },
    /// Elips penuh parametrik analitik — jadi silinder elips mulus saat di-extrude.
    Ellipse {
        center: (f64, f64),
        radius_x: f64,
        radius_y: f64,
    },
    /// Loop tertutup segmen Line/Arc; segmen harus sudah berurutan
    /// end-to-end kembali ke titik awal (verifikasi kontinuitas jadi
    /// tanggung jawab pemanggil — lihat pembangun chain di `ducad-app`).
    Loop(Vec<ProfileSegment>),
}

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
        Profile::Loop(segments) => {
            if segments.is_empty() {
                bail!("profil loop kosong");
            }
            let edges: Vec<Edge> = segments
                .iter()
                .map(|s| match s {
                    ProfileSegment::Line { start, end } => {
                        Edge::segment(to_3d(*start), to_3d(*end))
                    }
                    ProfileSegment::Arc { start, via, end } => {
                        Edge::arc(to_3d(*start), to_3d(*via), to_3d(*end))
                    }
                })
                .collect();
            Ok(Wire::from_edges(edges.iter()))
        }
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
        Profile::Loop(segments) => {
            if segments.is_empty() {
                bail!("profil loop kosong");
            }
            let edges: Vec<Edge> = segments
                .iter()
                .map(|s| match s {
                    ProfileSegment::Line { start, end } => {
                        Edge::segment(dvec3(start.0, start.1, z), dvec3(end.0, end.1, z))
                    }
                    ProfileSegment::Arc { start, via, end } => Edge::arc(
                        dvec3(start.0, start.1, z),
                        dvec3(via.0, via.1, z),
                        dvec3(end.0, end.1, z),
                    ),
                })
                .collect();
            Ok(Wire::from_edges(edges.iter()))
        }
    }
}

/// Satu segmen kurva jalur pemandu 3D (spine path) untuk operasi Sweep.
#[derive(Debug, Clone, PartialEq)]
pub enum PathSegment {
    Line { start: [f64; 3], end: [f64; 3] },
    Arc {
        start: [f64; 3],
        via: [f64; 3],
        end: [f64; 3],
    },
    Polyline(Vec<[f64; 3]>),
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

