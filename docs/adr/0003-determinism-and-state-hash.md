# ADR 0003: Determinism and the state hash

Date: 2026-10-05 · Status: Accepted · Requirements: NFR-08, SIM-02

## Context

NFR-08 requires that the same seed and command stream produce a bit-identical state after N ticks on the same build and platform. Determinism breaks silently, so it needs to be checked mechanically from M0.

## Decision

- **Stable IDs.** Every simulation entity carries a `SpawnId(u64)`, allocated in spawn order by `NextSpawnId` (use `sim_core::spawn_sim`). Order-dependent logic sorts by `SpawnId`, never by `Entity` bits or query order.
- **RNG.** `SimRng(ChaCha8Rng)` is the shared RNG for serial systems. Systems that may run in parallel instead take `SimSeed::stream(key, tick)`, a ChaCha8 stream seeded from `(seed, key, tick)` where `key` is a per-system constant from `rng_key("crate::system")`. No system ever needs another system's RNG state, so parallel scheduling can't change the outcome. `rand::rng()` and `thread_rng` are banned with clippy's `disallowed_methods`.
- **State hash.** FNV-1a 64-bit, implemented in `sim_core` (no dependency, stable across runs). Each plugin registers its state types with `app.register_hashed::<T>()` / `register_hashed_resource::<T>()`; each type implements `StateHash`. `compute_state_hash` hashes registered resources in registration order, then every `SpawnId` entity in ascending ID order, writing a presence tag plus the data for each registered component. Floats hash by bit pattern. Entity references hash as the target's `SpawnId`.
- **Headless runs don't depend on time.** `sim_headless` steps `FixedMain` directly and rotates message buffers once per tick, so results don't depend on machine speed.
- **Diagnostics are excluded.** `SimTimings` uses `Instant` for the debug panel and is never read by simulation logic or hashed.

## Consequences

- New state that is not registered is invisible to the hash. Rule: a component or resource that holds simulation state is registered in the same commit that adds it; review checks this.
- Hashing walks every entity, so it is for tests, debugging and save verification, not for every tick.
- The determinism test (`tests/tests/determinism.rs`) and a CI smoke run compare hashes on every push.
