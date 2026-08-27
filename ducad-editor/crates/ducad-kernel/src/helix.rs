//! Generator Kurva Parametrik Helix / Spiral 3D & Solid Pegas / Ulir (Fase 10.2).
//!
//! Menyediakan kalkulasi analitik kurva helix 3D:
//!   x(t) = R(t) * cos(θ(t))
//!   y(t) = R(t) * sin(θ(t))
//!   z(t) = pitch * turns * t
//! dan pembuat Wire serta Solid B-Rep dengan operasi Sweep / Pipe OpenCASCADE.

use anyhow::{anyhow, bail, Result};
use glam::{dvec3, DVec3};
use opencascade::primitives::{Edge, Face, Wire};

use crate::lock_kernel;
use crate::profile::{build_wire_on_plane, PathSegment, Profile, ProfileSegment};
use crate::shape::KernelShape;

/// Arah putaran ulir / spiral (Handedness).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelixHandedness {
    /// Ulir Kanan (Right-Handed): Putaran searah jarum jam bila dilihat dari arah datang,
    /// atau kaidah tangan kanan (counter-clockwise saat dilihat dari atas sumbu normal +Z).
    RightHand,
    /// Ulir Kiri (Left-Handed): Putaran berlawanan kaidah tangan kanan (clockwise saat dilihat dari atas sumbu normal +Z).
    LeftHand,
}

/// Bentuk penampang profil untuk solid pegas / ulir.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HelixProfileKind {
    /// Pegas kawat bundar (Compression / Extension round wire spring)
    Circle { radius: f64 },
    /// Bilah pipih / persegi (Auger blade / Rectangular wire spring / Archimedes screw)
    Rectangle { width: f64, height: f64 },
    /// Ulir segitiga / V-thread (Bottle thread / Screw thread)
    Triangle { width: f64, height: f64 },
}

/// Parameter lengkap kurva spiral / helix 3D.
#[derive(Debug, Clone, PartialEq)]
pub struct HelixParams {
    /// Radius spiral dasar (mm)
    pub radius: f64,
    /// Radius spiral di ujung atas (mm) — None = silindris konstan, Some(r) = kerucut / tirus (conical)
    pub end_radius: Option<f64>,
    /// Jarak pitch per satu putaran penuh (mm)
    pub pitch: f64,
    /// Jumlah putaran / revolusi (misal 5.0 atau 3.5)
    pub turns: f64,
    /// Arah putaran (Right-handed vs Left-handed)
    pub handedness: HelixHandedness,
    /// Titik pusat dasar spiral (3D world coordinate)
    pub origin: [f64; 3],
    /// Sumbu memanjang spiral (vektor arah unit, default [0.0, 0.0, 1.0])
    pub axis: [f64; 3],
    /// Arah radial awal pada u=0 (tegak lurus terhadap sumbu axis)
    pub start_dir: [f64; 3],
}

impl Default for HelixParams {
    fn default() -> Self {
        Self {
            radius: 20.0,
            end_radius: None,
            pitch: 10.0,
            turns: 5.0,
            handedness: HelixHandedness::RightHand,
            origin: [0.0, 0.0, 0.0],
            axis: [0.0, 0.0, 1.0],
            start_dir: [1.0, 0.0, 0.0],
        }
    }
}

impl HelixParams {
    /// Total tinggi spiral (mm): Pitch * Turns.
    pub fn total_height(&self) -> f64 {
        self.pitch * self.turns
    }

    /// Radius akhir spiral (mm).
    pub fn resolved_end_radius(&self) -> f64 {
        self.end_radius.unwrap_or(self.radius)
    }
}

/// Helper untuk normalisasi sumbu & basis ortonormal helix.
fn compute_basis(params: &HelixParams) -> (DVec3, DVec3, DVec3, DVec3) {
    let origin = dvec3(params.origin[0], params.origin[1], params.origin[2]);
    let mut axis = dvec3(params.axis[0], params.axis[1], params.axis[2]);
    if axis.length_squared() < 1e-8 {
        axis = DVec3::Z;
    } else {
        axis = axis.normalize();
    }

    let start_dir = dvec3(params.start_dir[0], params.start_dir[1], params.start_dir[2]);
    let proj = start_dir.dot(axis);
    let mut u_dir = start_dir - axis * proj;
    if u_dir.length_squared() < 1e-6 {
        let temp = if axis.z.abs() < 0.9 {
            DVec3::Z
        } else {
            DVec3::Y
        };
        u_dir = axis.cross(temp).normalize();
    } else {
        u_dir = u_dir.normalize();
    }
    let v_dir = axis.cross(u_dir).normalize();

    (origin, axis, u_dir, v_dir)
}

/// Menghasilkan urutan titik-titik koordinat 3D yang membentuk kurva spiral / helix parametrik.
pub fn generate_helix_points(params: &HelixParams, samples_per_turn: usize) -> Result<Vec<[f64; 3]>> {
    if params.radius <= 0.0 {
        bail!("Radius helix harus lebih besar dari 0");
    }
    let end_r = params.resolved_end_radius();
    if end_r <= 0.0 {
        bail!("End radius helix harus lebih besar dari 0");
    }
    if params.turns.abs() < 1e-4 {
        bail!("Jumlah putaran (turns) helix harus tidak nol");
    }
    if params.pitch <= 0.0 && params.total_height().abs() < 1e-4 {
        bail!("Pitch helix harus lebih besar dari 0");
    }

    let samples_per_rev = samples_per_turn.max(16);
    let total_samples = ((params.turns.abs() * samples_per_rev as f64).ceil() as usize).max(32);

    let (origin, axis, u_dir, v_dir) = compute_basis(params);
    let sign = match params.handedness {
        HelixHandedness::RightHand => 1.0,
        HelixHandedness::LeftHand => -1.0,
    };

    let mut points = Vec::with_capacity(total_samples + 1);
    for i in 0..=total_samples {
        let s = i as f64 / total_samples as f64;
        let theta = 2.0 * std::f64::consts::PI * params.turns * s * sign;
        let r = params.radius + (end_r - params.radius) * s;
        let z = params.pitch * params.turns * s;

        let radial_vec = (u_dir * theta.cos() + v_dir * theta.sin()) * r;
        let pt = origin + radial_vec + axis * z;
        points.push([pt.x, pt.y, pt.z]);
    }

    Ok(points)
}

/// Menghasilkan segmen jalur (spine PathSegment) untuk kurva helix 3D.
pub fn create_helix_path_segments(
    params: &HelixParams,
    samples_per_turn: usize,
) -> Result<Vec<PathSegment>> {
    let pts = generate_helix_points(params, samples_per_turn)?;
    Ok(vec![PathSegment::Polyline(pts)])
}

/// Menghasilkan OpenCASCADE Wire dari kurva spiral / helix 3D.
pub fn create_helix_wire(params: &HelixParams, samples_per_turn: usize) -> Result<Wire> {
    let pts = generate_helix_points(params, samples_per_turn)?;
    if pts.len() < 2 {
        bail!("Titik helix tidak mencukupi untuk membuat Wire");
    }
    let edges: Vec<Edge> = pts
        .windows(2)
        .map(|w| {
            Edge::segment(
                dvec3(w[0][0], w[0][1], w[0][2]),
                dvec3(w[1][0], w[1][1], w[1][2]),
            )
        })
        .collect();

    Ok(Wire::from_edges(edges.iter()))
}

/// Basis lokal bidang penampang (cross-section plane) di titik awal kurva helix (s=0).
fn compute_start_profile_plane(params: &HelixParams) -> Result<([f64; 3], [f64; 3], [f64; 3], [f64; 3])> {
    let (origin, axis, u_dir, v_dir) = compute_basis(params);
    let sign = match params.handedness {
        HelixHandedness::RightHand => 1.0,
        HelixHandedness::LeftHand => -1.0,
    };

    let start_pos = origin + u_dir * params.radius;

    // Vektor tangen dP/ds pada s=0:
    let end_r = params.resolved_end_radius();
    let dr_ds = end_r - params.radius;
    let dtheta_ds = 2.0 * std::f64::consts::PI * params.turns * sign;
    let dz_ds = params.pitch * params.turns;

    let tangent = (u_dir * dr_ds + v_dir * (params.radius * dtheta_ds) + axis * dz_ds).normalize();

    // Normal bidang penampang adalah arah tangen
    let normal_vec = tangent;

    // Sumbu U bidang penampang sejajar arah radial ke luar
    let mut prof_u = u_dir - normal_vec * u_dir.dot(normal_vec);
    if prof_u.length_squared() < 1e-6 {
        prof_u = axis - normal_vec * axis.dot(normal_vec);
    }
    let prof_u = prof_u.normalize();
    let prof_v = normal_vec.cross(prof_u).normalize();

    Ok((
        [start_pos.x, start_pos.y, start_pos.z],
        [prof_u.x, prof_u.y, prof_u.z],
        [prof_v.x, prof_v.y, prof_v.z],
        [normal_vec.x, normal_vec.y, normal_vec.z],
    ))
}

/// Menghasilkan bentuk solid B-Rep 3D pegas / ulir dengan menyapu profil tertentu di sepanjang kurva helix.
pub fn create_helix_solid(
    params: &HelixParams,
    profile_kind: HelixProfileKind,
    samples_per_turn: usize,
) -> Result<KernelShape> {
    let profile = match profile_kind {
        HelixProfileKind::Circle { radius } => {
            if radius <= 0.0 {
                bail!("Radius kawat pegas harus > 0");
            }
            if radius >= params.pitch * 0.5 {
                bail!("Radius kawat ({:.2} mm) terlalu besar untuk pitch ({:.2} mm) — pegas akan berpotongan sendiri", radius, params.pitch);
            }
            Profile::Circle {
                center: (0.0, 0.0),
                radius,
            }
        }
        HelixProfileKind::Rectangle { width, height } => {
            if width <= 0.0 || height <= 0.0 {
                bail!("Lebar dan tinggi profil persegi harus > 0");
            }
            if height >= params.pitch {
                bail!("Tinggi penampang ({:.2} mm) melebihi pitch ({:.2} mm)", height, params.pitch);
            }
            let hw = width * 0.5;
            let hh = height * 0.5;
            Profile::Loop(vec![
                ProfileSegment::Line {
                    start: (-hw, -hh),
                    end: (hw, -hh),
                },
                ProfileSegment::Line {
                    start: (hw, -hh),
                    end: (hw, hh),
                },
                ProfileSegment::Line {
                    start: (hw, hh),
                    end: (-hw, hh),
                },
                ProfileSegment::Line {
                    start: (-hw, hh),
                    end: (-hw, -hh),
                },
            ])
        }
        HelixProfileKind::Triangle { width, height } => {
            if width <= 0.0 || height <= 0.0 {
                bail!("Lebar dan tinggi profil segitiga harus > 0");
            }
            let hw = width * 0.5;
            let hh = height * 0.5;
            Profile::Loop(vec![
                ProfileSegment::Line {
                    start: (-hw, -hh),
                    end: (hw, 0.0),
                },
                ProfileSegment::Line {
                    start: (hw, 0.0),
                    end: (-hw, hh),
                },
                ProfileSegment::Line {
                    start: (-hw, hh),
                    end: (-hw, -hh),
                },
            ])
        }
    };

    create_helix_solid_with_custom_profile(params, &profile, samples_per_turn)
}

/// Menghasilkan bentuk solid B-Rep 3D dengan menyapu profil kustom di sepanjang kurva helix.
pub fn create_helix_solid_with_custom_profile(
    params: &HelixParams,
    profile: &Profile,
    samples_per_turn: usize,
) -> Result<KernelShape> {
    let _guard = lock_kernel();
    let spine_wire = create_helix_wire(params, samples_per_turn)?;
    let (origin, u_axis, v_axis, normal) = compute_start_profile_plane(params)?;

    let profile_wire = build_wire_on_plane(profile, origin, u_axis, v_axis, normal)?;
    let profile_face = Face::from_wire(&profile_wire);

    let shape = profile_face
        .pipe(&spine_wire)
        .map_err(|e| anyhow!("Operasi Helix Sweep gagal: pastikan profil dan parameter spiral valid ({e})"))?;

    Ok(KernelShape::from_inner(shape))
}
