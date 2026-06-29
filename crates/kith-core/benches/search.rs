//! Bench: ranked multi-field search (`Store::search`) over a
//! deterministic synthetic DB at three sizes — the search baseline.
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

/// Synthetic sizes (individual counts) the baseline records — shared with the
/// layout/views/gedcom benches so the numbers compare.
const SIZES: [u32; 3] = [200, 2_000, 10_000];
/// A fixed seed so every run measures the same graph.
const SEED: u64 = 0x5EED_C0DE;
/// The result cap the GUI palette and CLI default use.
const LIMIT: usize = 50;

/// One seeded in-memory store per size (the FTS index is kept in sync by the
/// migration-0002 triggers during the seed, so search has real data to rank).
fn seeded(individuals: u32) -> Store {
    let store = Store::open_in_memory().expect("open in-memory store");
    seed_synthetic(&store, individuals, SEED).expect("seed synthetic db");
    store
}

/// `Store::search` for three representative queries: a common bloodline surname,
/// a place shared by many events, and the empty "list everyone" path.
fn bench_search(c: &mut Criterion) {
    let queries: [(&str, &str); 3] = [("surname", "Lund"), ("place", "Bergen"), ("empty", "")];
    let mut group = c.benchmark_group("search");
    for n in SIZES {
        let store = seeded(n);
        for (label, query) in queries {
            group.bench_with_input(BenchmarkId::new(label, n), &n, |b, _| {
                b.iter(|| {
                    store
                        .search(black_box(query), black_box(LIMIT))
                        .expect("search")
                });
            });
        }
    }
    group.finish();
}

criterion_group!(benches, bench_search);
criterion_main!(benches);
