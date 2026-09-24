//! Bench budget §7 (`00-konvensi.md`) untuk pipeline vektor:
//! - tesselasi ulang 1 path 200 segmen ≤ 300 µs;
//! - frame 5.000 path ber-fill tanpa perubahan (sync + batches) ≤ 4 ms CPU.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ducad_render::plane::SketchPlane;
use ducad_render::{tessellate_entity, TessOptions, VectorCache};
use ducad_sketch::{Entity, EntityId, Paint, PathSeg, Rgba, Sketch, Style, Subpath};
use glam::DVec2;

/// Poligon bintang tertutup `n` segmen kubik di sekitar `center`.
fn star_path(center: DVec2, radius: f64, n: usize) -> Entity {
    let point = |i: usize| {
        let a = i as f64 / n as f64 * std::f64::consts::TAU;
        let r = if i.is_multiple_of(2) {
            radius
        } else {
            radius * 0.6
        };
        center + DVec2::new(a.cos(), a.sin()) * r
    };
    let segs = (1..n)
        .map(|i| {
            let (p0, p1) = (point(i - 1), point(i));
            PathSeg::Cubic {
                c1: p0.lerp(p1, 1.0 / 3.0) + DVec2::new(0.3, 0.0),
                c2: p0.lerp(p1, 2.0 / 3.0) - DVec2::new(0.3, 0.0),
                end: p1,
            }
        })
        .collect();
    Entity::path(vec![Subpath {
        start: point(0),
        segs,
        closed: true,
    }])
}

fn filled() -> Style {
    Style {
        fill: Some(Paint::Solid(Rgba([0.2, 0.4, 0.8, 1.0]))),
        ..Style::cad_default()
    }
}

fn bench_tessellate_200_segments(c: &mut Criterion) {
    let entity = star_path(DVec2::ZERO, 50.0, 201);
    let style = filled();
    let plane = SketchPlane::top();
    let opts = TessOptions::default();
    c.bench_function("tessellate_path_200_segments", |b| {
        b.iter(|| black_box(tessellate_entity(&entity, &style, &plane, &opts)))
    });
}

fn bench_frame_5000_unchanged(c: &mut Criterion) {
    let mut sketch = Sketch::default();
    let mut ids: Vec<EntityId> = Vec::with_capacity(5_000);
    for i in 0..5_000 {
        let center = DVec2::new((i % 100) as f64 * 12.0, (i / 100) as f64 * 12.0);
        let id = sketch.entities.insert(star_path(center, 5.0, 12));
        sketch.styles.insert(id, filled());
        sketch.touch(id);
        ids.push(id);
    }
    let plane = SketchPlane::top();
    let opts = TessOptions::default();
    let mut cache = VectorCache::new(64 * 1024 * 1024);
    cache.sync(&sketch, &plane, &opts, &ids);

    let mut group = c.benchmark_group("vector_frame");
    group.sample_size(20);
    // Frame tanpa perubahan: kontrak M1.2 — `sync` mengembalikan `false`
    // sehingga batch/GPU buffer tidak disentuh. Inilah yang dibatasi 4 ms.
    group.bench_function("frame_5000_filled_paths_unchanged", |b| {
        b.iter(|| black_box(cache.sync(&sketch, &plane, &opts, black_box(&ids))))
    });
    // Penggabungan batch per layer — hanya dibayar pada frame yang berubah.
    group.bench_function("batches_5000_filled_paths", |b| {
        b.iter(|| black_box(cache.batches(&sketch)))
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_tessellate_200_segments,
    bench_frame_5000_unchanged
);
criterion_main!(benches);
