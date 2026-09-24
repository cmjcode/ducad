//! Bench budget §7 (`00-konvensi.md`): boolean 2 path × 100 segmen ≤ 5 ms.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ducad_sketch::{boolean, BoolOp, FillRule, PathSeg, Subpath};
use glam::DVec2;

/// Lingkaran bergelombang tertutup `n` segmen kubik.
fn wavy_circle(center: DVec2, radius: f64, n: usize) -> Subpath {
    let point = |i: usize| {
        let a = i as f64 / n as f64 * std::f64::consts::TAU;
        let r = radius * (1.0 + 0.05 * (a * 7.0).sin());
        center + DVec2::new(a.cos(), a.sin()) * r
    };
    let segs = (1..n)
        .map(|i| {
            let (p0, p1) = (point(i - 1), point(i));
            PathSeg::Cubic {
                c1: p0.lerp(p1, 1.0 / 3.0),
                c2: p0.lerp(p1, 2.0 / 3.0),
                end: p1,
            }
        })
        .collect();
    Subpath {
        start: point(0),
        segs,
        closed: true,
    }
}

fn bench_boolean_100_segments(c: &mut Criterion) {
    let a = [wavy_circle(DVec2::ZERO, 50.0, 101)];
    let b = [wavy_circle(DVec2::new(30.0, 10.0), 45.0, 101)];
    for (name, op) in [
        ("boolean_union_2x100", BoolOp::Union),
        ("boolean_difference_2x100", BoolOp::Difference),
        ("boolean_intersection_2x100", BoolOp::Intersection),
    ] {
        c.bench_function(name, |bn| {
            bn.iter(|| black_box(boolean(&a, &b, op, FillRule::NonZero, 0.01)))
        });
    }
}

criterion_group!(benches, bench_boolean_100_segments);
criterion_main!(benches);
