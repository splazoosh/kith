//! Bench: bulk `gedcom::import` of an exported synthetic DB — the
//! GEDCOM-import baseline. The text is a *real* export of the synthetic tree, so
//! it scales with N (not a fixed fixture).
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

/// Exports a freshly-seeded synthetic DB to a GEDCOM string of the given size.
fn exported(individuals: u32) -> String {
    let store = Store::open_in_memory().expect("open in-memory store");
    seed_synthetic(&store, individuals, SEED).expect("seed synthetic db");
    kith_core::gedcom::export(&store).expect("export gedcom")
}

/// `gedcom::import` into a fresh (empty) store each iteration — a clean non-merge
/// import (the engine refuses a non-empty target without `--merge`).
fn bench_import(c: &mut Criterion) {
    let mut group = c.benchmark_group("gedcom_import");
    // Import is the heaviest path; keep the 10k wall-clock sane.
    group.sample_size(20);
    for n in SIZES {
        let text = exported(n);
        group.bench_with_input(BenchmarkId::new("import", n), &n, |b, _| {
            b.iter(|| {
                let store = Store::open_in_memory().expect("open in-memory store");
                let options = ImportOptions::default();
                kith_core::gedcom::import(black_box(&store), black_box(&text), black_box(&options))
                    .expect("import gedcom")
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_import);
criterion_main!(benches);
