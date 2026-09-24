use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ducad_sketch::{Entity, Sketch};
use glam::DVec2;

fn build_20k_sketch() -> Sketch {
    let mut sketch = Sketch::default();
    let mut rng_state: u64 = 987654321;
    let mut next_f64 = |min: f64, max: f64| -> f64 {
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let t = ((rng_state >> 32) as u32 as f64) / (u32::MAX as f64);
        min + t * (max - min)
    };

    // Spread 20,000 entities over a 1000mm x 1000mm area
    for _ in 0..20_000 {
        let x = next_f64(-500.0, 500.0);
        let y = next_f64(-500.0, 500.0);
        let len = next_f64(2.0, 20.0);
        let entity = Entity::line(DVec2::new(x, y), DVec2::new(x + len, y + len));
        let id = sketch.entities.insert(entity);
        sketch.touch(id);
    }
    sketch
}

fn bench_hit_test_20k(c: &mut Criterion) {
    let mut sketch = build_20k_sketch();
    // Warm up spatial index
    sketch.spatial();

    c.bench_function("hit_test_20k", |b| {
        let probe = DVec2::new(12.34, 56.78);
        b.iter(|| {
            black_box(sketch.hit_test(black_box(probe), black_box(1.0)))
        })
    });
}

criterion_group!(benches, bench_hit_test_20k);
criterion_main!(benches);
