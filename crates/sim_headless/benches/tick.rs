//! Tick-cost benchmarks for NFR-03 (≤ 8 ms per tick at 20,000 full-detail
//! agents) and NFR-04 (≥ 300 ticks/s headless at 20,000 agents).
//!
//! Load: villagers populated in one region (M2), plus two armies of 1,000
//! marching on each other and fighting (`tick/army`, M3), plus a run on the
//! run island in its tenth day (`tick/run_island`, M4: the bot's army,
//! garrisons, the rival, raiders and director bands).
//!
//!   cargo bench -p sim_headless
//!   cargo bench -p sim_headless -- --save-baseline nightly   # CI nightly job

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use sim_core::{RunConfig, RunState, SimStepper, compute_state_hash};
use sim_headless::{HeadlessConfig, Scenario, headless_app, resume_commands};

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

/// A game day, in ticks.
const DAY: u64 = 2_400;

/// M4: a run on the run island (seed 42, steady) at day 10, played by the
/// bot. The bot plays once to day 12 to record its commands; a fresh app
/// then replays them to day 10 and every measured tick is a plain
/// `step(1)` with the rest of the recording scheduled (no bot cost).
fn run_island(c: &mut Criterion) {
    let mut group = c.benchmark_group("tick");
    group.throughput(Throughput::Elements(1));
    let config = HeadlessConfig::run(42, RunConfig::default());
    let played = sim_headless::bot::run_bot(config, 12 * DAY, 0);
    let mut app = headless_app(config);
    resume_commands(&mut app, &played.log);
    app.step(10 * DAY);
    assert!(
        app.world().resource::<RunState>().outcome.is_none(),
        "the run goes on at day 10"
    );
    group.bench_function("run_island", |bench| {
        bench.iter(|| {
            app.step(1);
        });
    });
    group.finish();
}

criterion_group!(benches, tick, army, run_island, state_hash);
criterion_main!(benches);
