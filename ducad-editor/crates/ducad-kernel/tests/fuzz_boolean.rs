//! Fuzz boolean (PLAN_MIGRASI_TRUCK F3.7) — port `sc:crates/kernel/src/fuzz.rs`
//! SolidCraft, tetapi lewat API publik `ducad_kernel` (jadi lapisan DUCAD ikut
//! diuji, dan kedua backend bisa dibandingkan).
//!
//! Tiap seed: dua primitif acak di grid kasar 2,5 mm (sehingga face berimpit
//! dan singgungan sering muncul). Identitas yang dicek, toleransi 0,2 %:
//!
//! - V(A∪B) + V(A∩B) = V(A) + V(B)
//! - V(A−B) = V(A) − V(A∩B)
//!
//! dan tiap hasil tak-kosong harus solid valid.
//!
//! Seed yang gagal BUKAN blocker (PLAN F3.7): dicatat di
//! `tests/fuzz_boolean_known_failures.<backend>.txt`. Tes gagal bila
//! tingkat lulus < 80 % (angka SolidCraft: 80,8 %), bila ada seed yang panic,
//! atau bila muncul seed gagal yang BELUM tercatat (regresi). Perbarui
//! daftar dengan `BLESS=1`.
//!
//! ```text
//! cargo test -p ducad-kernel --no-default-features --features kernel-truck --test fuzz_boolean
//! SEEDS=0..95 cargo test -p ducad-kernel --no-default-features --features kernel-truck --test fuzz_boolean
//! ```
//! Bawaan `SEEDS=0..23` (cepat untuk CI); `0..95` (inklusif, 96 seed) untuk gate F3.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use ducad_kernel::{
    intersect, make_box, make_cylinder, make_sphere, revolve_profile, rotate_shape, subtract,
    transform_shape, translate_shape, union, EmptyIntersection, KernelShape, Profile, BACKEND,
};

/// xorshift64* dengan seed disebar splitmix64 — identik dengan SolidCraft,
/// jadi seed `n` di sini = seed `n` di `solidcraft-cli fuzz`.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        Rng((z ^ (z >> 31)) | 1)
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }
    fn grid(&mut self, lo: f64, hi: f64, step: f64) -> f64 {
        let n = ((hi - lo) / step).floor().max(0.0) as usize;
        lo + step * self.below(n + 1) as f64
    }
}

#[derive(Debug, Clone)]
enum Shape {
    Box {
        a: [f64; 3],
        b: [f64; 3],
    },
    Cylinder {
        base: [f64; 3],
        axis: usize,
        radius: f64,
        height: f64,
    },
    Sphere {
        center: [f64; 3],
        radius: f64,
    },
    Torus {
        center: [f64; 3],
        major: f64,
        minor: f64,
    },
}

impl Shape {
    /// Urutan tarikan acak sama persis dengan `Shape::random` SolidCraft.
    fn random(r: &mut Rng) -> Shape {
        let g = 2.5;
        let at = [
            r.grid(0.0, 30.0, g),
            r.grid(0.0, 30.0, g),
            r.grid(0.0, 30.0, g),
        ];
        match r.below(10) {
            0..=4 => {
                let size = [
                    r.grid(5.0, 30.0, g),
                    r.grid(5.0, 30.0, g),
                    r.grid(5.0, 30.0, g),
                ];
                Shape::Box {
                    a: at,
                    b: [at[0] + size[0], at[1] + size[1], at[2] + size[2]],
                }
            }
            5..=7 => {
                let axis = r.below(3);
                Shape::Cylinder {
                    base: at,
                    axis,
                    radius: r.grid(2.5, 10.0, g),
                    height: r.grid(5.0, 30.0, g),
                }
            }
            8 => Shape::Sphere {
                center: at,
                radius: r.grid(2.5, 12.5, g),
            },
            _ => {
                let minor = r.grid(2.5, 5.0, g);
                Shape::Torus {
                    center: at,
                    major: minor + r.grid(2.5, 10.0, g),
                    minor,
                }
            }
        }
    }

    /// Body lewat API DUCAD.
    fn build(&self) -> anyhow::Result<KernelShape> {
        let half_pi = std::f64::consts::FRAC_PI_2;
        match *self {
            Shape::Box { a, b } => {
                let s = make_box(b[0] - a[0], b[1] - a[1], b[2] - a[2], false)?;
                translate_shape(&s, a[0], a[1], a[2])
            }
            Shape::Cylinder {
                base,
                axis,
                radius,
                height,
            } => {
                // Silinder DUCAD berdiri di +Z; putar ke sumbu yang diminta.
                let c = make_cylinder(radius, height)?;
                let (rot_axis, angle) = match axis {
                    0 => ((0.0, 1.0, 0.0), half_pi),  // Z → X
                    1 => ((1.0, 0.0, 0.0), -half_pi), // Z → Y
                    _ => ((0.0, 0.0, 1.0), 0.0),
                };
                transform_shape(
                    &c,
                    (base[0], base[1], base[2]),
                    (0.0, 0.0, 0.0),
                    rot_axis,
                    angle,
                )
            }
            Shape::Sphere { center, radius } => {
                let s = make_sphere(radius)?;
                translate_shape(&s, center[0], center[1], center[2])
            }
            Shape::Torus {
                center,
                major,
                minor,
            } => {
                // Lingkaran (major, 0) di bidang XY diputar keliling sumbu Y,
                // lalu sumbu torus Y → Z (seperti `torus` SolidCraft).
                let profile = Profile::Circle {
                    center: (major, 0.0),
                    radius: minor,
                };
                let t = revolve_profile(&profile, (0.0, 0.0), (0.0, 1.0), None)?;
                let t = rotate_shape(&t, (0.0, 0.0, 0.0), (1.0, 0.0, 0.0), half_pi)?;
                translate_shape(&t, center[0], center[1], center[2])
            }
        }
    }
}

/// `SEEDS=a..b` (inklusif), dipisah koma; bawaan `0..23`.
fn seeds() -> Vec<u64> {
    let spec = std::env::var("SEEDS").unwrap_or_else(|_| "0..23".into());
    let mut out = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        match part.split_once("..") {
            Some((a, b)) => {
                let (a, b): (u64, u64) = (a.parse().unwrap(), b.parse().unwrap());
                out.extend(a..=b);
            }
            None => out.push(part.parse().unwrap()),
        }
    }
    out
}

/// `("ok", "")` atau (jenis kegagalan, rincian).
fn case(seed: u64) -> (String, String) {
    let mut r = Rng::new(seed);
    let (sa, sb) = (Shape::random(&mut r), Shape::random(&mut r));
    let (a, b) = match (sa.build(), sb.build()) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(e), _) | (_, Err(e)) => {
            return ("error".into(), format!("primitif gagal dibuat: {e:#}"));
        }
    };
    let (va, vb) = (a.volume(), b.volume());
    let mut vols = Vec::new();
    for (name, res) in [
        ("union", union(&a, &b)),
        ("intersect", intersect(&a, &b)),
        ("subtract", subtract(&a, &b)),
    ] {
        match res {
            Ok(s) => {
                let v = s.volume();
                // Hasil kosong (cut menelan semua) tidak dicek validitasnya.
                if v > 0.0 && !s.is_valid() {
                    return ("invalid".into(), format!("{name}: solid tidak valid"));
                }
                vols.push(v);
            }
            Err(e) if e.downcast_ref::<EmptyIntersection>().is_some() => vols.push(0.0),
            Err(e) => return ("error".into(), format!("{name}: {e:#}")),
        }
    }
    let (vu, vi, vc) = (vols[0], vols[1], vols[2]);
    let tol = 2e-3 * (va + vb);
    let e1 = (vu + vi - va - vb).abs();
    let e2 = (vc - (va - vi)).abs();
    if e1 <= tol && e2 <= tol {
        ("ok".into(), String::new())
    } else {
        (
            "identity".into(),
            format!(
                "A {va:.3}, B {vb:.3}, A∪B {vu:.3}, A∩B {vi:.3}, A−B {vc:.3}: meleset {e1:.3} dan {e2:.3}"
            ),
        )
    }
}

fn known_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(format!("fuzz_boolean_known_failures.{BACKEND}.txt"))
}

/// Seed → "jenis\trincian", dari berkas known failures.
fn known() -> BTreeMap<u64, String> {
    std::fs::read_to_string(known_path())
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let (seed, rest) = l.split_once('\t')?;
            Some((seed.trim().parse().ok()?, rest.to_string()))
        })
        .collect()
}

#[test]
fn boolean_volume_identity() {
    let seeds = seeds();
    let known = known();
    let mut results: BTreeMap<u64, (String, String)> = BTreeMap::new();
    for &seed in &seeds {
        let started = std::time::Instant::now();
        let outcome = std::panic::catch_unwind(|| case(seed))
            .unwrap_or_else(|_| ("panic".into(), "kernel panic".into()));
        eprintln!(
            "seed {seed:>3}: {:<8} {:>6.2}s {}",
            outcome.0,
            started.elapsed().as_secs_f64(),
            outcome.1
        );
        results.insert(seed, outcome);
    }
    let ok = results.values().filter(|(o, _)| o == "ok").count();
    let rate = ok as f64 / seeds.len().max(1) as f64;
    eprintln!(
        "fuzz boolean [{BACKEND}]: {ok}/{} identitas volume ({:.1} %)",
        seeds.len(),
        rate * 100.0
    );

    if std::env::var_os("BLESS").is_some() {
        // Seed yang tidak dijalankan kali ini dipertahankan.
        let mut merged: BTreeMap<u64, String> = known
            .iter()
            .filter(|(s, _)| !results.contains_key(s))
            .map(|(s, r)| (*s, r.clone()))
            .collect();
        for (s, (o, d)) in &results {
            if o != "ok" {
                merged.insert(*s, format!("{o}\t{d}"));
            }
        }
        let mut text = format!(
            "# Seed fuzz boolean yang gagal di backend {BACKEND} (PLAN_MIGRASI_TRUCK F3.7).\n\
             # Dibuat `BLESS=1 SEEDS=… cargo test --test fuzz_boolean`; bukan blocker.\n\
             # seed<TAB>jenis<TAB>rincian\n"
        );
        for (s, r) in merged {
            text.push_str(&format!("{s}\t{r}\n"));
        }
        std::fs::write(known_path(), text).unwrap();
        return;
    }

    let panics: Vec<u64> = results
        .iter()
        .filter(|(_, (o, _))| o == "panic")
        .map(|(s, _)| *s)
        .collect();
    assert!(panics.is_empty(), "kernel panic pada seed {panics:?}");
    let new_failures: Vec<String> = results
        .iter()
        .filter(|(s, (o, _))| o != "ok" && !known.contains_key(s))
        .map(|(s, (o, d))| format!("{s} {o}: {d}"))
        .collect();
    assert!(
        new_failures.is_empty(),
        "seed gagal yang belum tercatat di {} (regresi, atau jalankan BLESS=1):\n{}",
        known_path().display(),
        new_failures.join("\n")
    );
    assert!(
        rate >= 0.8,
        "tingkat lulus identitas boolean {:.1} % < 80 %",
        rate * 100.0
    );
}
