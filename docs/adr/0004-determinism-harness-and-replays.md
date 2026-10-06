# ADR 0004: Determinism harness, command log and replays

Date: 2026-10-05 · Status: Accepted · Requirements: NFR-08, NFR-03, NFR-04

## Context

SEED.md requires a CI test that runs 10,000 ticks twice from the same seed and command log and compares state hashes, and says replays are "just a recorded event stream". M0 only compared two command-free runs of 1,000 ticks, and nothing checked that the hash actually covered all simulation state. M1 and later milestones need this harness in place before they add state.

## Decision

**Commands are recorded and replayed through the same message path.** `SimSet::Input` has two ordered sub-sets. In `SimInputSet::Inject`, `inject_scripted` writes this tick's scripted commands (`CommandScript`) as ordinary `SimCommand` messages, then `record_commands` appends every command for the tick to `CommandLog`. Systems that act on commands live in `SimInputSet::Consume`. Consumers can't tell a replayed command from a live one, so replays exercise the real code path.

**Replay files** (`sim_core::Replay`, RON, versioned by `REPLAY_FORMAT`) contain every input to a run: the seed, the M0 placeholder agent count, the tick count, and the command log. They also store the expected state hash at regular checkpoints. Verifying a replay re-runs it and reports the first checkpoint that differs, which narrows a divergence to a window of ticks. `sim_headless --record` and `--replay` expose this; `--replay` exits with code 2 on divergence.

**Hash coverage is enforced, not assumed.** `HashRegistry` tracks every hashed type plus explicit exemptions, each with a reason. `unhashed_state` reports any component on a `SpawnId` entity, or any resource from a `sim_*` crate, that is neither. The integration test `state_hash_covers_every_sim_component` fails with the type names. A mutation check confirmed it catches an unregistered component. Current exemptions: `SimTimings` (diagnostics), `CommandLog` (history), `CommandScript` (pending input). Type names need `bevy_ecs`'s `debug` feature, now enabled workspace-wide.

**Parallel execution must match serial execution.** `sim_headless` now runs `FixedUpdate` on Bevy's multi-threaded executor, like the game. `Threading::Single` swaps in the single-threaded executor. Tests and CI check that both produce identical checkpoints, which catches systems whose results depend on scheduling order.

**Benchmarks.** `cargo bench -p sim_headless` measures tick cost and state-hash cost at 1,000, 5,000 and 20,000 agents. A nightly workflow compares against the previous night's criterion baseline. `scripts/check_bench_regression.py` fails the job when the lower bound of the 95% confidence interval for a benchmark's slowdown exceeds 10%, so runner noise alone doesn't trip it.

## Consequences

- The determinism suite takes about 4 s in debug builds. CI also records and verifies a 10,000-tick, 200-command replay in release under both executors.
- The M0 state hash changed (`0xbce7…` became `0x9c55…` for seed 42, 1,000 ticks), because the placeholder marker component and agent count are now hashed. Hash values are only comparable within a build anyway (NFR-08).
- The parallel executor costs about 0.13 ms per tick in release on a 4-core machine with the current tiny systems (about 7,600 ticks/s, against about 130,000 single-threaded). That is about 4% of NFR-04's budget. It should be watched as systems are added; the 10 timing marker systems between sets add sync points and are the first thing to revisit.
- After a regression is flagged, the new results become the next night's baseline, so each regression is reported once.
- Replay files from before a format change are rejected rather than silently misread.
