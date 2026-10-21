//! Baseline waktu op kernel (PLAN_MIGRASI_TRUCK F0.3).
//!
//! Hanya memakai API publik `ducad-kernel`, jadi bench yang sama bisa
//! dijalankan ulang pada backend lain untuk dibandingkan:
//!
//! ```bash
//! cargo bench -p ducad-kernel --bench ops -- --save-baseline truck
//! ```

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ducad_kernel::{
    chamfer_all, fillet_all, linear_pattern_shape, make_box, make_cylinder, revolve_profile,
    shell_open, subtract, translate_shape, union, Direction, KernelShape, Profile,
    ProfileSegment, ShellOpening,
};

fn plate() -> KernelShape {
    make_box(60.0, 40.0, 8.0, false).expect("box")
}

fn plate_with_hole() -> KernelShape {
    let pin = translate_shape(&make_cylinder(5.0, 20.0).expect("cyl"), 30.0, 20.0, -5.0)
        .expect("translate");
    subtract(&plate(), &pin).expect("subtract")
}

fn bench_primitives(c: &mut Criterion) {
    c.bench_function("primitive/box", |b| {
        b.iter(|| make_box(black_box(60.0), 40.0, 8.0, false).expect("box"))
    });
    c.bench_function("primitive/cylinder", |b| {
        b.iter(|| make_cylinder(black_box(5.0), 20.0).expect("cyl"))
    });
}

fn bench_extrude_revolve(c: &mut Criterion) {
    let rect = Profile::Loop(vec![
        ProfileSegment::Line { start: (0.0, 0.0), end: (60.0, 0.0) },
        ProfileSegment::Line { start: (60.0, 0.0), end: (60.0, 40.0) },
        ProfileSegment::Line { start: (60.0, 40.0), end: (0.0, 40.0) },
        ProfileSegment::Line { start: (0.0, 40.0), end: (0.0, 0.0) },
    ]);
    let holed = Profile::WithHoles {
        outer: Box::new(rect.clone()),
        holes: (0..4)
            .map(|i| Profile::Circle {
                center: (10.0 + 13.0 * i as f64, 20.0),
                radius: 3.0,
            })
            .collect(),
    };
    c.bench_function("extrude/rect", |b| {
        b.iter(|| ducad_kernel::extrude_profile(black_box(&rect), 8.0).expect("extrude"))
    });
    c.bench_function("extrude/rect_4_holes", |b| {
        b.iter(|| ducad_kernel::extrude_profile(black_box(&holed), 8.0).expect("extrude"))
    });
    let section = Profile::Loop(vec![
        ProfileSegment::Line { start: (10.0, 0.0), end: (30.0, 0.0) },
        ProfileSegment::Line { start: (30.0, 0.0), end: (30.0, 10.0) },
        ProfileSegment::Line { start: (30.0, 10.0), end: (10.0, 10.0) },
        ProfileSegment::Line { start: (10.0, 10.0), end: (10.0, 0.0) },
    ]);
    c.bench_function("revolve/ring_360", |b| {
        b.iter(|| revolve_profile(black_box(&section), (0.0, 0.0), (0.0, 1.0), None).expect("rev"))
    });
}

fn bench_boolean(c: &mut Criterion) {
    let a = plate();
    let pin = translate_shape(&make_cylinder(5.0, 20.0).expect("cyl"), 30.0, 20.0, -5.0)
        .expect("translate");
    let boss = translate_shape(&make_cylinder(8.0, 10.0).expect("cyl"), 30.0, 20.0, 8.0)
        .expect("translate");
    c.bench_function("boolean/subtract_hole", |b| {
        b.iter(|| subtract(black_box(&a), &pin).expect("subtract"))
    });
    c.bench_function("boolean/union_boss", |b| {
        b.iter(|| union(black_box(&a), &boss).expect("union"))
    });
}

fn bench_modify(c: &mut Criterion) {
    let a = plate();
    let holed = plate_with_hole();
    c.bench_function("modify/fillet_all_r1", |b| {
        b.iter(|| fillet_all(black_box(&a), 1.0).expect("fillet"))
    });
    c.bench_function("modify/fillet_all_r1_holed", |b| {
        b.iter(|| fillet_all(black_box(&holed), 1.0).expect("fillet"))
    });
    c.bench_function("modify/chamfer_all_0.5", |b| {
        b.iter(|| chamfer_all(black_box(&a), 0.5).expect("chamfer"))
    });
    let block = make_box(60.0, 40.0, 30.0, false).expect("box");
    c.bench_function("modify/shell_open_top_t2", |b| {
        b.iter(|| {
            shell_open(black_box(&block), 2.0, ShellOpening::Farthest(Direction::PosZ), 0.0)
                .expect("shell")
        })
    });
    let pin = make_cylinder(2.0, 8.0).expect("cyl");
    c.bench_function("pattern/linear_5x4", |b| {
        b.iter(|| linear_pattern_shape(black_box(&pin), 5, 10.0, 4, 10.0, 1, 0.0).expect("pattern"))
    });
}

fn bench_query(c: &mut Criterion) {
    let holed = fillet_all(&plate_with_hole(), 1.0).expect("fillet");
    c.bench_function("query/tessellate", |b| b.iter(|| black_box(&holed).tessellate()));
    c.bench_function("query/volume", |b| b.iter(|| black_box(&holed).volume()));
    c.bench_function("query/surface_area", |b| b.iter(|| black_box(&holed).surface_area()));
    c.bench_function("query/enumerate_faces", |b| {
        b.iter(|| ducad_kernel::enumerate_faces(black_box(&holed)))
    });
    c.bench_function("io/step_roundtrip", |b| {
        b.iter(|| {
            let text = black_box(&holed).to_step_string().expect("step out");
            KernelShape::from_step_string(&text).expect("step in")
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(20);
    targets = bench_primitives, bench_extrude_revolve, bench_boolean, bench_modify, bench_query
}
criterion_main!(benches);
