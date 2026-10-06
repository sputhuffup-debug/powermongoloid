# Contributing

Humans and Claude Code work on this project side by side, under the same rules. Those rules live in [CLAUDE.md](CLAUDE.md): it is written for an agent, but every hard rule in it applies to everyone. This page covers the workflow; the [developer handbook](docs/dev/) covers the code.

New here? Start with [GETTING_STARTED](docs/dev/GETTING_STARTED.md), then read [ARCHITECTURE](docs/dev/ARCHITECTURE.md) and [DETERMINISM](docs/dev/DETERMINISM.md).

## How work is organised

- **The spec** is [docs/SEED.md](docs/SEED.md). Every feature has a stable requirement ID (GP-, SIM-, RND-, UI-, NFR-) and a milestone.
- **Milestones are planned before they're built.** Each has a plan (`docs/M*_PLAN.md`) listing requirements, acceptance tests, design and open decisions. Decisions are answered by the project owner before coding starts.
- **Scope:** anything new goes to "Later" in SEED.md, or into a future milestone's plan, until the current milestone ships.
- **Significant design decisions** get a short ADR in [docs/adr](docs/adr), numbered in order: what we chose, why, and the consequences.

## Branches, commits, pull requests

- **Branches.** Work on a branch, never directly on `main`. Merges happen through pull requests once CI is green.
- **Commit messages.** Summary line first, then what changed and why. Cite requirement IDs (`SIM-31`, `GP-03`) and ADRs where relevant. One logical change per commit.
- **PR descriptions.** What's in it, how it was tested, and anything still open.
- **Generated files** (lockfiles and similar) are regenerated with tools, never edited by hand.

## Definition of done

1. `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass (`cargo lint`, `cargo test-all`).
2. The determinism tests still pass: in particular `state_hash_covers_every_sim_component` and `parallel_and_serial_execution_agree`.
3. The acceptance criteria in SEED.md that the change touches are checked, with tests where possible.
4. Significant design decisions have an ADR.
5. **Docs updated in the same commit:**
   - a new or changed data field: [DATA.md](docs/dev/DATA.md)
   - a new or reordered system: the schedule in [ARCHITECTURE.md](docs/dev/ARCHITECTURE.md)
   - a new test: [TESTING.md](docs/dev/TESTING.md)
   - a new recipe-worthy pattern: [HOWTO.md](docs/dev/HOWTO.md)
   - a new term: [GLOSSARY.md](docs/dev/GLOSSARY.md)

   `tests/tests/docs.rs` catches undocumented data fields and broken HOWTO paths.
6. `cargo doc --workspace --no-deps` builds with no warnings.

## Hard rules (summary of CLAUDE.md)

- `sim_*` crates never depend on rendering, windowing, audio or asset crates. The sim changes state only in `FixedUpdate`. Rendering and UI only read it.
- Input reaches the sim only as `SimCommand`s.
- All randomness comes from `SimRng` or seeded streams. No `HashMap` iteration in sim logic.
- Game data lives in `assets/data/*.ron`, not in code.
- Every system has a tracing span and a named `SystemSet`. Every sim component or resource is hashed or exempted with a reason.
- **No new dependencies, and no Bevy upgrade,** without the project owner's approval. Never use `bincode`.

## Review checklist

- **Determinism:** does a parallel system write anything but its own entity? Does anything iterate a hash map, depend on `Query` order, or use the wrong RNG?
- **Hashing:** is new state registered or exempted with a reason?
- **Data:** is tuning in data, with units documented? Did the guarding scenario tests still pass?
- **Cost:** what does it do at 20,000 agents? Check `--timings` before and after on a heavy load.
- **Readability:** can a player tell what happened and why (inspector, panels, log)?
- **Tests:** does a test cover the requirement, not just the implementation?
