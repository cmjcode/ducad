use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ducad_render::ink::{append_stroke_vertices, build_stroke_vertices, InkBrushRef, InkPointRef};
use ducad_render::plane::SketchPlane;

fn bench_append_one_point(c: &mut Criterion) {
    let plane = SketchPlane::top();
    let brush = InkBrushRef::default();

    let mut points: Vec<InkPointRef> = (0..100)
        .map(|i| InkPointRef {
            pos: [i as f32 * 2.0, (i as f32).sin() * 5.0],
            pressure: 0.5,
        })
        .collect();

    let mut out = Vec::new();
    build_stroke_vertices(&points, &brush, &plane, &mut out);

    c.bench_function("append_one_point", |b| {
        b.iter(|| {
            let mut test_out = out.clone();
            points.push(InkPointRef {
                pos: [202.0, 5.0],
                pressure: 0.5,
            });
            append_stroke_vertices(
                black_box(&points),
                black_box(100),
                black_box(&brush),
                black_box(&plane),
                black_box(&mut test_out),
            );
            points.pop();
        })
    });
}

fn bench_rebuild_strokes(c: &mut Criterion) {
    let plane = SketchPlane::top();
    let brush = InkBrushRef::default();

    let stroke: Vec<InkPointRef> = (0..100)
        .map(|i| InkPointRef {
            pos: [i as f32, (i as f32 * 0.1).sin() * 10.0],
            pressure: 0.5,
        })
        .collect();

    let mut group = c.benchmark_group("rebuild_strokes");
    group.sample_size(10);
    group.bench_function("rebuild_100_strokes_100pts", |b| {
        b.iter(|| {
            let mut out = Vec::new();
            for _ in 0..100 {
                build_stroke_vertices(
                    black_box(&stroke),
                    black_box(&brush),
                    black_box(&plane),
                    black_box(&mut out),
                );
            }
            black_box(out);
        })
    });
    group.finish();
}

criterion_group!(benches, bench_append_one_point, bench_rebuild_strokes);
criterion_main!(benches);
