# M0 Plan: Skeleton

Status: **approved 2026-10-05**; workspace at repo root.
Spec: `docs/SEED.md` § "M0: Skeleton". Covers SIM-03, UI-07, NFR-08 (harness only).

## Pinned versions (checked on crates.io, 2026-10-05)

| Crate | Version | Used by | Notes |
| --- | --- | --- | --- |
| Rust toolchain | 1.97.0, edition 2024 | all | `rust-toolchain.toml`, with rustfmt + clippy |
| `bevy` | `=0.19.1` | `game_*` only | full engine |
| `bevy_ecs`, `bevy_app`, `bevy_math`, `bevy_time` | `=0.19.1` | `sim_*` | no render/window/asset crates |
| `bevy_egui` | `=0.42.0` | `game_ui` | targets `bevy ^0.19`; behind a `dev` feature |
| `rand` / `rand_chacha` | `0.10.3` / `0.10.0` | `sim_core` | `ChaCha8Rng` |
| `serde` / `ron` | `1.0.229` / `0.12.2` | `sim_core` | data files from M1 on |
| `criterion` | `0.8.2` | `sim_headless` (dev) | benchmark skeleton (prompt 3) |
| `tracing` | via Bevy | all | one span per system set |

Not added in M0 (used later): `noise 0.9.0` (M1; uses its own `rand 0.8` internally, which is fine), `smallvec`, `zstd 0.14`.

**Save format (decided):** `postcard 1.1.3` + `zstd`, added in M4. `bincode` is not used anywhere (see `docs/adr/0001-save-format-postcard.md`).

## Workspace

The workspace lives at the repo root, laid out as in SEED.md § "Workspace layout". Every crate listed there is created now as a compiling plugin, mostly empty, so the dependency graph is fixed from day one.

```
Cargo.toml                 # [workspace], shared [workspace.dependencies], dev profile opt-levels
rust-toolchain.toml
.github/workflows/ci.yml
assets/data/.gitkeep
docs/adr/0002-workspace-and-sim-render-split.md
docs/adr/0003-determinism-and-state-hash.md
crates/sim_core            # real content (below)
crates/sim_world, sim_agents, sim_economy, sim_military,
       sim_knowledge, sim_lod, sim_ai          # empty plugin + named SystemSet
crates/game_render, game_ui                    # game_ui holds the debug panel
crates/game_app            # binary
crates/sim_headless        # binary + lib (lib so tests/benches can drive it)
tests/                     # workspace-level integration test crate
```

Dev profile follows CLAUDE.md: `opt-level = 1` for our crates and `opt-level = 3` for dependencies.

## sim_core content

- **`SimSet` enum**, matching the 9-step tick schedule (Input, Knowledge, Decide, Move, Act, Needs, Aggregate, Lod, Bookkeeping), configured `.chain()` in `FixedUpdate`. Other crates add their systems to these sets.
- **`SimTick(u64)`**: incremented by one system in `SimSet::Bookkeeping`.
- **Fixed tick (SIM-03)**: `Time<Fixed>` set to 10 Hz. Speed control (GP-11, M2) scales `Time<Virtual>`, so it needs no changes later.
- **`SimRng(ChaCha8Rng)`**: seeded from `SimSeed(u64)`. `SimRng::derive(key: u64)` gives per-system streams via `ChaCha8Rng::from_seed(hash(seed, key))`, so systems running in parallel never share RNG state.
- **`SpawnId(u64)`**: a component plus a `NextSpawnId` resource. It is the stable sort key the determinism rules require, because `Entity` bits are not stable.
- **`SimCommand` event**: an empty `Command` enum for now (variants arrive in M3). It is drained in `SimSet::Input`, so the input path exists from M0.
- **State hash (NFR-08)**: a `StateHash` trait, plus an in-house FNV-1a 64-bit hasher (no dependency, and stable across runs of the same build). Floats are hashed with `to_bits()`. `compute_state_hash(&mut World)` hashes resources, then every entity with a `SpawnId` in sorted SpawnId order, each through a registry of hashable component types. Each crate registers its components via `app.register_hashed::<T>()`. That makes "the hash covers every sim component" a reviewable rule rather than an afterthought.
- **`SimTimings` resource**: wall-clock duration per `SimSet`, recorded by small marker systems between sets. It is diagnostics only and is excluded from the hash.

## Binaries

**`sim_headless`** (`cargo run -p sim_headless -- --ticks 1000 --seed 42`)
- Builds an `App` with `MinimalPlugins`-equivalent sim plugins only, so there is no window and nothing from rendering is linked.
- Does **not** depend on wall-clock time. It runs the `FixedUpdate` schedule exactly N times directly (`world.run_schedule(FixedUpdate)`), so results are independent of machine speed.
- Prints `seed`, `ticks`, `state_hash=0x…` and the elapsed time / ticks per second. Hand-rolled argument parsing, so no `clap` dependency.
- So the hash covers more than a counter in M0, a tiny placeholder system spawns a few `SpawnId`'d entities with a `Position` and moves them using `SimRng`. It is deleted when real systems arrive.

**`game_app`** (`cargo run -p game_app`)
- `DefaultPlugins` + all `sim_*` plugins + `game_render` (camera and light only, empty scene) + `game_ui`.
- **Debug panel (UI-07)** in egui: tick count, tick time per `SimSet`, total tick time, entity count, seed. Panels for LOD state and counts by detail level are stubs until M5.

## Tests and CI

- `sim_core` unit tests: RNG derivation is deterministic and key-separated; the FNV hasher matches known vectors; `SimTick` advances once per fixed step.
- `tests/determinism.rs`: runs the headless sim twice with the same seed and asserts the hashes are equal, and runs it with different seeds and asserts they differ. M0 uses 1,000 ticks; prompt 3 raises this to the spec's 10,000 with a command log.
- `.github/workflows/ci.yml` on push and PR, on ubuntu-latest with Bevy's Linux system libs (`libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev`) and `Swatinem/rust-cache`: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`. A nightly benchmark job comes with prompt 3.
- A `.claude` SessionStart hook to install those system libs in cloud sessions, so later sessions can build without manual setup.

## Verifying each acceptance criterion

| Criterion | How I will verify it |
| --- | --- |
| `game_app` opens a window with an empty scene and a debug panel showing tick count and tick time | Build it here, then launch it under Xvfb with the pre-installed tooling, take a screenshot after ~3 s, and check that the panel shows a rising tick count. The screenshot is attached to the report. |
| `sim_headless --ticks 1000 --seed 42` prints a state hash, the same one twice | Run it twice in the shell and diff the output; also enforced by `tests/determinism.rs`. |
| CI runs fmt, clippy and tests, and all pass | Run the same three commands locally before pushing, then confirm the Actions run on the branch is green. |

## ADRs to write in M0

1. **0002 Workspace and sim/render split**: why `sim_*` crates use only `bevy_ecs`/`bevy_app`/`bevy_math`/`bevy_time`.
2. **0003 Determinism and state hash**: SpawnId ordering, derived RNG streams, the FNV hash, and headless runs that don't depend on wall-clock time.

