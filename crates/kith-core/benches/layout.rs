//! The performance baseline: criterion benches over the query
//! walks and all four layout modes, against a deterministic synthetic DB.
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

use kith_core::layout::{ChartMode, compute_layout};
use kith_core::prelude::*;
use kith_core::query::network;
use kith_core::synth::seed_synthetic;

/// Synthetic sizes (individual counts) the baseline records.
const SIZES: [u32; 3] = [200, 2_000, 10_000];
/// A fixed seed so every run measures the same graph.
const SEED: u64 = 0x5EED_C0DE;
/// Generation budget for the tree walks (Network ignores it).
const GENERATIONS: u32 = 6;

/// One seeded in-memory store per size, plus its mid-tree focal.
fn seeded(individuals: u32) -> (Store, PersonId) {
    let store = Store::open_in_memory().expect("open in-memory store");
    let focal = seed_synthetic(&store, individuals, SEED).expect("seed synthetic db");
    (store, focal)
}

/// The query walks: ancestors / descendants / relatives (hourglass) / network.
fn bench_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("query");
    for n in SIZES {
        let (store, focal) = seeded(n);
        group.bench_with_input(BenchmarkId::new("ancestors", n), &n, |b, _| {
            b.iter(|| ancestors(black_box(&store), black_box(focal), GENERATIONS).expect("walk"));
        });
        group.bench_with_input(BenchmarkId::new("descendants", n), &n, |b, _| {
            b.iter(|| descendants(black_box(&store), black_box(focal), GENERATIONS).expect("walk"));
        });
        group.bench_with_input(BenchmarkId::new("relatives", n), &n, |b, _| {
            b.iter(|| {
                relatives(
                    black_box(&store),
                    black_box(focal),
                    GENERATIONS,
                    GENERATIONS,
                )
                .expect("walk")
            });
        });
        group.bench_with_input(BenchmarkId::new("network", n), &n, |b, _| {
            b.iter(|| network(black_box(&store), black_box(focal)).expect("walk"));
        });
    }
    group.finish();
}

/// `compute_layout` end-to-end (walk + positioner) in all four modes.
fn bench_layout(c: &mut Criterion) {
    let mut group = c.benchmark_group("layout");
    let modes = [
        ChartMode::Ancestors,
        ChartMode::Descendants,
        ChartMode::Hourglass,
        ChartMode::Network,
    ];
    for n in SIZES {
        let (store, focal) = seeded(n);
        for mode in modes {
            group.bench_with_input(BenchmarkId::new(format!("{mode:?}"), n), &n, |b, _| {
                b.iter(|| {
                    compute_layout(black_box(&store), black_box(focal), mode, GENERATIONS)
                        .expect("layout")
                });
            });
        }
    }
    group.finish();
}

criterion_group!(benches, bench_query, bench_layout);
criterion_main!(benches);
