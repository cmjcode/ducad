//! Ulir fisik metrik ISO (P20): alur ulir profil dasar ISO 68-1 (60°)
//! dipotong dari silinder luar dengan sapuan helix.
//!
//! Fungsi di sini TIDAK memegang `KERNEL_LOCK` sendiri — ia hanya merangkai
//! fungsi publik kernel lain (yang masing-masing mengunci), sesuai aturan
//! "Mutex tidak reentrant".

use anyhow::{bail, Result};

use crate::csg::subtract;
use crate::helix::{create_helix_solid_with_custom_profile, HelixHandedness, HelixParams};
use crate::profile::{Profile, ProfileSegment};
use crate::shape::KernelShape;

/// Titik sampel spline helix per putaran.
const SAMPLES_PER_TURN: usize = 36;
/// Putaran penuh per potongan pemotong (lihat `cut_iso_thread`).
const TURNS_PER_CHUNK: f64 = 2.0;

/// Geometri alur ulir luar profil dasar ISO 68-1 untuk kisar `pitch`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IsoThreadProfile {
    pub pitch: f64,
    /// Kedalaman alur dari diameter mayor: `5H/8`, `H = √3/2 · P`.
    pub depth: f64,
    /// Lebar alur di diameter mayor: `7P/8` (puncak rata `P/8`).
    pub width_major: f64,
    /// Lebar dasar alur: `P/4`.
    pub width_root: f64,
}

impl IsoThreadProfile {
    pub fn new(pitch: f64) -> Self {
        let h = 3.0_f64.sqrt() / 2.0 * pitch;
        Self {
            pitch,
            depth: 5.0 * h / 8.0,
            width_major: 7.0 * pitch / 8.0,
            width_root: pitch / 4.0,
        }
    }

    /// Luas penampang alur (trapesium) di dalam material.
    pub fn area(&self) -> f64 {
        (self.width_major + self.width_root) / 2.0 * self.depth
    }

    /// Jarak titik berat penampang dari diameter mayor, ke arah sumbu.
    pub fn centroid_depth(&self) -> f64 {
        let (a, b) = (self.width_major, self.width_root);
        self.depth / 3.0 * (a + 2.0 * b) / (a + b)
    }

    /// Volume yang hilang untuk ulir sepanjang `length` pada diameter mayor
    /// `major_d`: luas × panjang lintasan titik berat (Pappus).
    pub fn removed_volume(&self, major_d: f64, length: f64) -> f64 {
        let turns = length / self.pitch;
        let r = major_d / 2.0 - self.centroid_depth();
        let per_turn = ((2.0 * std::f64::consts::PI * r).powi(2) + self.pitch.powi(2)).sqrt();
        self.area() * per_turn * turns
    }
}

/// Potong ulir luar ISO pada `shape`: silinder berdiameter mayor `major_d`
/// dengan sumbu lewat `axis_origin` searah `axis_dir`; ulir mulai di
/// `axis_origin` dan berjalan sejauh `length`.
pub fn cut_iso_thread(
    shape: &KernelShape,
    axis_origin: [f64; 3],
    axis_dir: [f64; 3],
    major_d: f64,
    pitch: f64,
    length: f64,
    left_handed: bool,
) -> Result<KernelShape> {
    if !(major_d > 0.0 && pitch > 0.0 && length > 0.0) {
        bail!("diameter, kisar, dan panjang ulir harus > 0");
    }
    let p = IsoThreadProfile::new(pitch);
    if p.depth >= major_d / 2.0 {
        bail!(
            "kisar {pitch} mm terlalu besar untuk diameter {major_d} mm (alur sedalam {:.3} mm)",
            p.depth
        );
    }
    if length < pitch {
        bail!("panjang ulir {length} mm lebih pendek dari satu kisar {pitch} mm");
    }
    // Koordinat profil: u = radial keluar (0 di diameter mayor), v ≈ aksial.
    // Sisi luar diperpanjang sedikit melewati permukaan supaya boolean tidak
    // bertemu face yang berimpit.
    let over = 0.1 * pitch;
    let slope = (p.width_major - p.width_root) / 2.0 / p.depth;
    let outer = p.width_major / 2.0 + over * slope;
    let root = p.width_root / 2.0;
    let pts = [
        (over, -outer),
        (over, outer),
        (-p.depth, root),
        (-p.depth, -root),
    ];
    let profile = Profile::Loop(
        (0..4)
            .map(|i| ProfileSegment::Line {
                start: pts[i],
                end: pts[(i + 1) % 4],
            })
            .collect(),
    );
    // Satu spline helix panjang membuat `MakePipe` menghasilkan solid rusak
    // (terbukti pada 13 putaran: tidak valid, volume ≈ 0), jadi pemotong
    // dibuat per potongan `TURNS_PER_CHUNK` putaran PENUH — awal tiap
    // potongan jatuh di sudut yang sama sehingga alurnya menyambung.
    let norm = (axis_dir[0].powi(2) + axis_dir[1].powi(2) + axis_dir[2].powi(2)).sqrt();
    if norm < 1e-9 {
        bail!("arah sumbu ulir nol");
    }
    let axis = axis_dir.map(|c| c / norm);
    let total_turns = length / pitch;
    let mut result: Option<KernelShape> = None;
    let mut done = 0.0;
    while total_turns - done > 1e-6 {
        let mut turns = (total_turns - done).min(TURNS_PER_CHUNK);
        // Sisa yang sangat pendek digabung ke potongan ini.
        if total_turns - done - turns < 0.25 {
            turns = total_turns - done;
        }
        let shift = done * pitch;
        let params = HelixParams {
            radius: major_d / 2.0,
            end_radius: None,
            pitch,
            turns,
            handedness: if left_handed {
                HelixHandedness::LeftHand
            } else {
                HelixHandedness::RightHand
            },
            origin: [
                axis_origin[0] + axis[0] * shift,
                axis_origin[1] + axis[1] * shift,
                axis_origin[2] + axis[2] * shift,
            ],
            axis,
            ..HelixParams::default()
        };
        let cutter = create_helix_solid_with_custom_profile(&params, &profile, SAMPLES_PER_TURN)?;
        if !cutter.is_valid() {
            bail!("pemotong ulir tidak valid pada putaran {done:.2}");
        }
        let base = result.as_ref().unwrap_or(shape);
        result = Some(subtract(base, &cutter)?);
        done += turns;
    }
    result.ok_or_else(|| anyhow::anyhow!("ulir tanpa putaran"))
}
