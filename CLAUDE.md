# Warlord Sim

Indirect-control medieval conquest simulation in Rust + Bevy.
The full spec is docs/SEED.md. Read it before any non-trivial change.
Requirement IDs (GP-, SIM-, RND-, UI-, NFR-) are stable; reference them
in commits, tests and comments.

## Current milestone
M1 (M0 complete 2026-10-05; update this line as milestones complete)

## Hard rules
- sim_* crates never depend on rendering, windowing, audio or asset crates.
- The simulation changes state only in FixedUpdate. Rendering and UI only read.
- Player and AI input reach the sim only as Command events.
- All randomness comes from SimRng or RNGs derived from it. Never thread_rng.
- No HashMap iteration in sim logic unless sorted. Sort parallel results
  by stable spawn ID where order affects outcomes.
- Game data (recipes, occupations, units, worldgen) lives in assets/data/*.ron,
  not in code.
- Every new system gets a tracing span and lives in a named SystemSet.
- Write the test first for sim logic when practical.
- Never use bincode (unmaintained). Saves use serde + postcard + zstd.
- Pin the Bevy version. Do not upgrade Bevy or add dependencies without asking.

## Commands
- Run game:       cargo run -p game_app
- Run headless:   cargo run --release -p sim_headless -- --seed 42 --ticks 10000
- Test:           cargo test --workspace
- Lint:           cargo clippy --workspace -- -D warnings && cargo fmt --check
- Bench:          cargo bench -p sim_headless
- Record replay:  cargo run --release -p sim_headless -- --ticks 10000 --commands 200 --record run.ron
- Verify replay:  cargo run --release -p sim_headless -- --replay run.ron [--single-threaded]
- Map image:      cargo run --release -p sim_headless -- --seed 42 --ticks 1 --map-ppm map.ppm
- Low-memory build (4-core cloud box): add `-j 3` to cargo build/test/clippy

## World (ADR 0005)
- Worldgen parameters live in assets/data/worldgen.ron (embedded at compile time).
- Change tiles only through `WorldMap::set_terrain`; it bumps chunk revisions
  (the renderer rebuilds only those chunks) and keeps the state hash current.
- Rendering and UI never write to sim entities, not even marker components;
  track render-side state in render-side resources or `Local`s.

## Determinism harness (ADR 0003, ADR 0004)
- Every new sim component or resource is registered with `register_hashed` /
  `register_hashed_resource` in the same commit, or exempted with
  `exempt_from_hash::<T>("reason")`. `state_hash_covers_every_sim_component`
  fails otherwise.
- New sim entities are spawned with `sim_core::spawn_sim` so they get a SpawnId.
- Systems that act on commands go in `SimInputSet::Consume`; never inject input
  any other way, or replays stop reproducing.
- If the determinism tests fail, find the first divergent checkpoint
  (`--checkpoint-every`) and compare single- vs multi-threaded runs first.

## Definition of done for any task
1. cargo fmt, clippy and tests pass.
2. The determinism test still passes.
3. Relevant acceptance criteria in docs/SEED.md are checked.
4. Significant design decisions get a short ADR in docs/adr/.

## Build settings
Use opt-level = 1 for dev and opt-level = 3 for dependencies in dev profile
so the game is playable in debug builds.
