# Warlord Sim

Indirect-control medieval conquest simulation in Rust + Bevy.
The full spec is docs/SEED.md. Read it before any non-trivial change.
Requirement IDs (GP-, SIM-, RND-, UI-, NFR-) are stable; reference them
in commits, tests and comments.

## Current milestone
M0 (update this line as milestones complete)

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
- Pin the Bevy version. Do not upgrade Bevy or add dependencies without asking.

## Commands
- Run game:       cargo run -p game_app
- Run headless:   cargo run --release -p sim_headless -- --seed 42 --ticks 10000
- Test:           cargo test --workspace
- Lint:           cargo clippy --workspace -- -D warnings && cargo fmt --check
- Bench:          cargo bench -p sim_headless

## Definition of done for any task
1. cargo fmt, clippy and tests pass.
2. The determinism test still passes.
3. Relevant acceptance criteria in docs/SEED.md are checked.
4. Significant design decisions get a short ADR in docs/adr/.

## Build settings
Use opt-level = 1 for dev and opt-level = 3 for dependencies in dev profile
so the game is playable in debug builds.
