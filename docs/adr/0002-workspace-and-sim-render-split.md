# ADR 0002: Workspace layout and the sim/render split

Date: 2026-10-05 · Status: Accepted · Requirements: SIM-03, NFR-04

## Context

SEED.md calls the separation of simulation from rendering "the single most important structural rule". The simulation must run headless for tests, benchmarks and a possible dedicated server, and must not slow down because of engine features it does not use.

## Decision

- One Cargo workspace at the repo root, with every crate from SEED.md's layout created in M0 as a compiling plugin, so the dependency graph is fixed from the start.
- `sim_*` crates depend only on `bevy_ecs`, `bevy_app` and `bevy_math` (pinned `=0.19.1`), plus `sim_core`. They never depend on `bevy`, or on render, window, asset, audio or `bevy_time` crates.
- `sim_core` owns the shared vocabulary: `SimSet` (the nine ordered tick phases), `SimTick`, `SimSeed`/`SimRng`, `SpawnId`, `SimCommand`, the shared components, and the state hash.
- Simulation time is `SimTick`, not `Time`. The game drives `FixedMain` from Bevy's fixed-timestep clock at `SIM_TICK_HZ`; `sim_headless` calls `run_sim_tick` directly N times.
- `sim_headless` is both a library (the `SimPlugins` group plus `headless_app`/`run_and_hash`) and a binary. `game_app` uses the same `SimPlugins`, so the game and the tests always run the same simulation.
- `game_*` crates use the full engine and only read simulation state. The egui debug panel is behind `game_app`'s default `dev` feature.

## Consequences

- `cargo build -p sim_headless` compiles in seconds; the full engine is only built for the game.
- Anything that needs the clock (day phases, speed controls) derives from `SimTick`; speed controls (GP-11) will scale Bevy's virtual time in the game and need no change to the simulation.
- Every per-system tracing span comes from Bevy's own instrumentation (`trace` feature, enabled by `--features tracy`). Hand-written spans are added for hot inner loops.
