//! PLAN_MIGRASI_TRUCK 5.4 — backend truck tanpa kunci global: import STEP
//! dan extrude/boolean berjalan bersamaan di thread berbeda, hasilnya sama
//! dengan versi berurutan; `union_all` paralel sama dengan lipatan biasa.
//! Hanya backend truck (OCCT tidak thread-safe).
#![cfg(feature = "kernel-truck")]

use std::sync::Arc;

use ducad_kernel::{
    capture_kernel_warnings, clone_shape, extrude_profile, make_box, make_cylinder, subtract, translate_shape,
    union, union_all, KernelShape, Profile, THREAD_SAFE,
};

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(1e-12)
}

fn plate_with_hole() -> KernelShape {
    let plate = make_box(40.0, 30.0, 5.0, false).unwrap();
    let pin = translate_shape(&make_cylinder(4.0, 20.0).unwrap(), 20.0, 15.0, -5.0).unwrap();
    subtract(&plate, &pin).unwrap()
}

fn boss() -> KernelShape {
    extrude_profile(&Profile::Circle { center: (0.0, 0.0), radius: 7.0 }, 12.0).unwrap()
}

#[test]
fn step_import_and_modeling_run_concurrently() {
    const { assert!(THREAD_SAFE) };
    let step: Arc<str> = plate_with_hole().to_step_string().unwrap().into();
    let want_import = KernelShape::from_step_string(&step).unwrap().volume();
    let want_extrude = boss().volume();

    // Shape dibagi antar-thread (Send + Sync) tanpa kunci.
    let shared = Arc::new(plate_with_hole());
    let handles: Vec<_> = (0..6)
        .map(|k| {
            let (step, shared) = (step.clone(), shared.clone());
            std::thread::spawn(move || {
                if k % 2 == 0 {
                    KernelShape::from_step_string(&step).unwrap().volume()
                } else {
                    let cut = subtract(&shared, &boss()).unwrap();
                    assert!(cut.volume() < shared.volume());
                    boss().volume()
                }
            })
        })
        .collect();
    for (k, h) in handles.into_iter().enumerate() {
        let v = h.join().expect("thread kernel tidak boleh panic");
        let want = if k % 2 == 0 { want_import } else { want_extrude };
        assert!(rel(v, want) < 1e-9, "thread {k}: {v} vs {want}");
    }
}

#[test]
fn union_all_matches_sequential_fold() {
    // Bertumpuk: jalur reduksi pohon paralel.
    let bars: Vec<KernelShape> = (0..5)
        .map(|i| {
            translate_shape(&make_box(10.0, 4.0, 4.0, false).unwrap(), i as f64 * 6.0, 0.0, 0.0)
                .unwrap()
        })
        .collect();
    let mut fold = clone_shape(&bars[0]).unwrap();
    for b in &bars[1..] {
        fold = union(&fold, b).unwrap();
    }
    let (all, warnings) = capture_kernel_warnings(|| union_all(bars.iter().map(|b| clone_shape(b).unwrap()).collect()).unwrap());
    assert!(rel(all.volume(), fold.volume()) < 1e-4, "{} vs {}", all.volume(), fold.volume());
    assert!(rel(all.volume(), (4.0 * 6.0 + 10.0) * 16.0) < 1e-4);
    assert!(warnings.is_empty(), "{warnings:?}");

    // Terpisah: jalur tanpa boolean (union_apart).
    let pins: Vec<KernelShape> = (0..6)
        .map(|i| {
            translate_shape(&make_cylinder(1.0, 3.0).unwrap(), i as f64 * 5.0, 0.0, 0.0).unwrap()
        })
        .collect();
    let one = pins[0].volume();
    assert!(rel(union_all(pins).unwrap().volume(), 6.0 * one) < 1e-4);

    // Kosong → shape kosong; satu → shape itu sendiri.
    assert!(union_all(Vec::new()).unwrap().volume().abs() < 1e-12);
    assert!(rel(union_all(vec![boss()]).unwrap().volume(), boss().volume()) < 1e-12);
}
