//! Tick-cost benchmarks for NFR-03 (≤ 8 ms per tick at 20,000 full-detail
//! agents) and NFR-04 (≥ 300 ticks/s headless at 20,000 agents).
//!
//! Load: villagers populated in one region (M2), plus two armies of 1,000
//! marching on each other and fighting (`tick/army`, M3).
//!
//!   cargo bench -p sim_headless
//!   cargo bench -p sim_headless -- --save-baseline nightly   # CI nightly job

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use sim_core::{SimStepper, compute_state_hash};
use sim_headless::{HeadlessConfig, Scenario, headless_app};

/// Agent counts: small, M2's one-region target, and NFR-02's full detail.
const LOADS: [u32; 3] = [1_000, 5_000, 20_000];

fn tick(c: &mut Criterion) {
    let mut group = c.benchmark_group("tick");
    group.throughput(Throughput::Elements(1));
    for agents in LOADS {
        let mut app = headless_app(HeadlessConfig {
            population: agents,
            ..HeadlessConfig::new(42)
        });
        // Tick 0 spawns the agents; measure steady-state ticks only.
        app.step(10);
        group.bench_with_input(BenchmarkId::new("agents", agents), &agents, |b, _| {
            b.iter(|| {
                app.step(1);
            });
        });
    }
    group.finish();
}

fn state_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_hash");
    for agents in LOADS {
        let mut app = headless_app(HeadlessConfig {
            population: agents,
            ..HeadlessConfig::new(42)
        });
        app.step(10);
        group.bench_with_input(BenchmarkId::new("agents", agents), &agents, |b, _| {
            b.iter(|| black_box(compute_state_hash(app.world_mut())));
        });
    }
    group.finish();
}

/// Two 1,000-strong armies at war meeting in battle, in the default
/// 5,000-villager region: marching, crowd steering and combat.
fn army(c: &mut Criterion) {
    let mut group = c.benchmark_group("tick");
    group.throughput(Throughput::Elements(1));
    let mut app = headless_app(HeadlessConfig {
        scenario: Scenario::Battle(1_000),
        ..HeadlessConfig::new(42)
    });
    app.step(10);
    group.bench_function(BenchmarkId::new("army", 2_000), |bench| {
        bench.iter(|| {
            app.step(1);
        });
    });
    group.finish();
}

criterion_group!(benches, tick, army, state_hash);
criterion_main!(benches);
