//! Bench: the detail-view loads (`PersonView`/`FamilyView`/`EventView`)
//! over a deterministic synthetic DB — the views baseline.
//!
//! Built only with `--features dev` (it needs [`kith_core::synth`]); the
//! `required-features` on the `[[bench]]` entry enforces that. Run with:
//!
//! ```text
//! cargo bench -p kith-core --features dev
//! ```
#![allow(
    missing_docs,
    reason = "criterion_group!/criterion_main! generate undocumented public items"
)]

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

use kith_core::prelude::*;
use kith_core::synth::seed_synthetic;

/// Synthetic sizes (individual counts) the baseline records.
const SIZES: [u32; 3] = [200, 2_000, 10_000];
/// A fixed seed so every run measures the same graph.
const SEED: u64 = 0x5EED_C0DE;

/// One seeded in-memory store per size, plus its mid-tree focal.
fn seeded(individuals: u32) -> (Store, PersonId) {
    let store = Store::open_in_memory().expect("open in-memory store");
    let focal = seed_synthetic(&store, individuals, SEED).expect("seed synthetic db");
    (store, focal)
}

/// The three composite detail views, each loaded for a stable focal-derived id.
fn bench_views(c: &mut Criterion) {
    let mut group = c.benchmark_group("views");
    for n in SIZES {
        let (store, focal) = seeded(n);
        // The focal is mid-tree, so it has parents (a `child_in` family) and its
        // own dated events — stable, representative ids for the family/event loads.
        let view = PersonView::load(&store, focal).expect("person view");
        let family = *view
            .child_in
            .first()
            .or_else(|| view.partner_in.first())
            .expect("focal has a family");
        let event = view.events.first().expect("focal has an event").id;

        group.bench_with_input(BenchmarkId::new("person", n), &n, |b, _| {
            b.iter(|| PersonView::load(black_box(&store), black_box(focal)).expect("person view"));
        });
        group.bench_with_input(BenchmarkId::new("family", n), &n, |b, _| {
            b.iter(|| FamilyView::load(black_box(&store), black_box(family)).expect("family view"));
        });
        group.bench_with_input(BenchmarkId::new("event", n), &n, |b, _| {
            b.iter(|| EventView::load(black_box(&store), black_box(event)).expect("event view"));
        });
    }
    group.finish();
}

criterion_group!(benches, bench_views);
criterion_main!(benches);
