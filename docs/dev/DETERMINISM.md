# Determinism

NFR-08: the same seed and the same command stream give a bit-identical state hash after N ticks, on the same build and platform, whether systems run in parallel or one at a time. Replays, save files (M4) and bug reports depend on it. Design: [ADR 0003](../adr/0003-determinism-and-state-hash.md) and [ADR 0004](../adr/0004-determinism-harness-and-replays.md).

## The rules, and why each exists

| Rule | Why |
| --- | --- |
| **The sim changes state only in `FixedUpdate`.** | Frame rate varies between machines; ticks don't. |
| **Input arrives only as `SimCommand` messages,** consumed in `SimInputSet::Consume`. The sim's own AI writes its commands in `SimInputSet::Ai`, after scripted input. | A replay is just the recorded command stream. Anything else that changes the sim (a UI writing to a component, say) wouldn't be replayed. The fixed sub-set order fixes the order commands arrive in. |
| **Events (`SimEvent`) are messages, not state.** `record_events` sorts each tick's events before appending them to the hashed `Chronicle`. | Writers in one set run in executor order. Messages live two ticks, so tests read the `Chronicle` or `EventFeed`. |
| **All randomness comes from `SimRng` or `SimSeed::stream(key, tick)`.** Never `rand::rng()`, the thread RNG (`clippy.toml` forbids it). | Parallel systems take their own stream per agent and tick (`seed.stream(rng_key("crate::system") ^ id.0, tick)`), so results never depend on which thread runs first. |
| **No iteration over a `HashMap` or `HashSet` in sim logic.** Lookups are fine. Use `BTreeMap`, or collect and sort. | Hash iteration order is random per process. |
| **Order-dependent work is sorted by `SpawnId`.** | `Entity` ids are reused after despawns; `SpawnId`s are allocated in spawn order and never reused. |
| **Parallel systems write only their own entity's components.** Shared state changes go through a serial, sorted step. | The step either posts an order to a resolver (`Transfers`, `Strike`) or runs as an exclusive system over a sorted list. Two threads writing the same data would race. |
| **Every sim component and resource is registered** with `register_hashed` / `register_hashed_resource`, or exempted with `exempt_from_hash::<T>("reason")`. | The coverage test (`state_hash_covers_every_sim_component`) fails otherwise. Unhashed state could diverge silently. |
| **New sim entities get a `SpawnId`** (`sim_core::spawn_sim`, or allocate from `NextSpawnId` in setup code). | The hash and every sort depend on it. |
| **Caches must be pure.** `PathCache`, `FlowCache` and the grids are exempt from the hash, because what they return depends only on hashed state. | A system that asks a cache must get the same answer whether or not the entry was cached. For example, `ensure_field` rebuilds a missing field every tick before anyone reads it. |
| **Floating-point sums run in a fixed order.** | `f32` addition isn't associative. Counts are summed as integers where possible (see `upkeep::tally`). |
| **Wall-clock time** (`Instant`, `Time<Real>`) is for diagnostics only. | `SimTimings`, logs. |

## The state hash

`compute_state_hash(world)` hashes:
- every registered resource, in registration order;
- then every entity with a `SpawnId`, in SpawnId order, through every registered component.

`Entity` references are hashed as the target's `SpawnId`. Each type implements `StateHash` by feeding its fields to a `StateHasher` (FNV-1a over the bytes).

## The harness

- **`sim_headless::run(config, ticks, commands, checkpoint_every)`** returns the hash at each checkpoint.
- **`record_replay` / `verify_replay`:** replay files are RON (format version in `sim_core::REPLAY_FORMAT`). They hold the seed, population, world size, scenario, the run's `RunConfig` (for a run), the player's commands and the expected checkpoints.
- **The CLI:**

  ```sh
  cargo run --release -p sim_headless -- --skirmish --ticks 6000 --commands 200 --record run.ron
  cargo run --release -p sim_headless -- --replay run.ron --single-threaded
  ```

- **`tests/tests/determinism.rs`:** 10,000-tick runs, parallel vs serial, recorded vs live commands, dropped-command detection, hash coverage, and armies with random orders.

## When a determinism test fails

1. **Find the first divergent checkpoint.** Run both ways with `--checkpoint-every 100` (then 10, then 1) and compare. A replay failure already names the first divergent tick.
2. **Compare single- and multi-threaded runs first** (`--single-threaded`). If they differ, a parallel system is order-dependent: it writes shared state, sums floats in thread order, or iterates a hash map.
3. **Check what changed recently** against the rules above. The usual suspects:
   - a new `HashMap` iteration
   - a parallel system writing another entity's components
   - `Query` iteration order used where SpawnId order was needed
   - a cache whose answer depends on what was cached before
   - a new component that isn't hashed, so the divergence shows up later and elsewhere
4. **Bisect the systems.** Disable systems one at a time (comment them out of the plugin) until the runs agree.
5. **Add a test** that reproduces the case. `parallel_and_serial_execution_agree` is the template.
