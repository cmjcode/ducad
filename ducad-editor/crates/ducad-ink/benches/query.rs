use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ducad_ink::document::InkDoc;
use ducad_ink::eraser::{erase, EraseMode};
use ducad_ink::lasso::{lasso_select, LassoMode};
use ducad_ink::stroke::{InkPoint, Stroke};
use ducad_sketch::layer::LayerId;
use ducad_sketch::style::Rgba;
use glam::Vec2;
use slotmap::SlotMap;

fn generate_doc(n: usize) -> InkDoc {
    let mut doc = InkDoc::default();
    let bid = doc.brushes.keys().next().unwrap();
    let mut lm: SlotMap<LayerId, ()> = SlotMap::with_key();
    let lid = lm.insert(());

    let mut state: u64 = 42;
    let mut next_rand = || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 33) as f32 / (1u64 << 31) as f32
    };

    for i in 0..n {
        let cx = next_rand() * 1000.0;
        let cy = next_rand() * 1000.0;
        let pts = vec![
            InkPoint::new(cx, cy, 0.5, 0.0, 0),
            InkPoint::new(
                cx + next_rand() * 10.0,
                cy + next_rand() * 10.0,
                0.5,
                0.0,
                10,
            ),
            InkPoint::new(
                cx + next_rand() * 20.0,
                cy + next_rand() * 20.0,
                0.5,
                0.0,
                20,
            ),
        ];
        doc.add_stroke(Stroke::new(
            i as u64,
            pts,
            bid,
            Rgba([0.0, 0.0, 0.0, 1.0]),
            lid,
        ));
    }
    doc.ensure_index();
    doc
}

fn bench_query_20k(c: &mut Criterion) {
    let doc = generate_doc(20_000);
    let index = doc.index().unwrap();

    c.bench_function("spatial_index_query_aabb_20k", |b| {
        b.iter(|| {
            let min = Vec2::new(400.0, 400.0);
            let max = Vec2::new(500.0, 500.0);
            black_box(index.query_aabb(min, max))
        })
    });

    c.bench_function("eraser_whole_stroke_20k", |b| {
        let path = vec![Vec2::new(450.0, 450.0), Vec2::new(470.0, 470.0)];
        b.iter(|| black_box(erase(&doc, &path, EraseMode::WholeStroke)))
    });

    c.bench_function("lasso_select_contain_20k", |b| {
        let poly = vec![
            Vec2::new(400.0, 400.0),
            Vec2::new(500.0, 400.0),
            Vec2::new(500.0, 500.0),
            Vec2::new(400.0, 500.0),
        ];
        b.iter(|| black_box(lasso_select(&doc, &poly, LassoMode::Contain)))
    });
}

/// `hit`/`visible_in` pada 20k coretan: jalur indeks (indeks segar) vs
/// linear (indeks basi) — REVIEW-2026-09-24 #16.
fn bench_hit_visible_20k(c: &mut Criterion) {
    let indexed = generate_doc(20_000);
    let mut linear = indexed.clone();
    linear.touch(); // indeks basi → jalur linear
    let (min, max) = (Vec2::new(400.0, 400.0), Vec2::new(500.0, 500.0));
    let p = Vec2::new(450.0, 450.0);

    c.bench_function("hit_20k_indexed", |b| {
        b.iter(|| black_box(indexed.hit(p, 1.0)))
    });
    c.bench_function("hit_20k_linear", |b| {
        b.iter(|| black_box(linear.hit(p, 1.0)))
    });
    c.bench_function("visible_in_20k_indexed", |b| {
        b.iter(|| black_box(indexed.visible_in(min, max, &|_| true)))
    });
    c.bench_function("visible_in_20k_linear", |b| {
        b.iter(|| black_box(linear.visible_in(min, max, &|_| true)))
    });
}

criterion_group!(benches, bench_query_20k, bench_hit_visible_20k);
criterion_main!(benches);
