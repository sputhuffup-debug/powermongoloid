# M3b Plan: Developer handbook

Status: **planned 2026-10-06; decisions made; build when asked** (you chose plan only for now). Requested: "add some code base documentation so humans can develop alongside easier".

M3b makes the codebase approachable for human developers working next to Claude Code. It adds no gameplay. The sim has grown to 11 crates, 6 data files and about 70 systems, and today the only guides are the spec, the milestone plans, ADRs, and CLAUDE.md, which is written for an agent. M3b runs alongside the M3 playtest, so it doesn't delay M4.

## Who it's for
Developers who know Rust but not necessarily Bevy or this project. Bevy and ECS ideas are explained where the project depends on them (schedules, system sets, queries, messages), with links to the Bevy 0.19 docs.

## Deliverables

| Document | Contents |
| --- | --- |
| `README.md` (refreshed) | What the game is, a screenshot, controls including M3's army orders, and links into the handbook |
| `CONTRIBUTING.md` | Branches and PRs, commit messages citing requirement IDs, the definition of done, when to write an ADR, the review checklist, and how humans and Claude Code share the same rules (CLAUDE.md) |
| `docs/dev/GETTING_STARTED.md` | Toolchain (pinned in `rust-toolchain.toml`), Linux system packages, build profiles and why, low-memory builds, running the game, headless and tests, and common problems with fixes (out of memory, disk space, missing `libxkbcommon`, software rendering) |
| `docs/dev/ARCHITECTURE.md` | Crate map and dependency diagram (Mermaid, rendered by GitHub); the tick schedule with every `SimSet`, sub-set and system in order; how a click becomes a `SimCommand`, a state change and a rendered frame; where state lives; the sim/render split |
| `docs/dev/DETERMINISM.md` | The rules and why each exists; the state hash and its coverage test; RNG streams; SpawnId ordering; parallel-system rules; a checklist for finding a divergence |
| `docs/dev/HOWTO.md` | Step-by-step recipes, each pointing at a real example in the code: <ul><li>add a component or resource (hash registration)</li><li>add a sim system (set, span, ordering, parallel rules)</li><li>add a villager action</li><li>add a good, recipe or occupation</li><li>add a captain order</li><li>add a UI panel or button</li><li>add a scenario test</li><li>tune behaviour with `--stats-every`</li><li>profile with `--timings` and Tracy</li><li>take an in-game screenshot under Xvfb</li></ul> |
| `docs/dev/DATA.md` | Every field of every file in `assets/data/` (`army`, `calendar`, `economy`, `render`, `villagers`, `worldgen`): meaning, units, valid range, and which systems and tests depend on it |
| `docs/dev/TESTING.md` | Every test: what it guards, which requirement, how long it runs, and how to run one alone; what CI runs; the nightly benchmarks |
| `docs/dev/GLOSSARY.md` | Project terms: tick, SpawnId, state hash, detail band, captain, follower, militia, posture, flow field, rally field, threat, transfer resolver, logistics step, ... |
| Crate docs (rustdoc) | Each crate's `lib.rs` gets an overview: purpose, main types, its systems in schedule order, and links to the handbook and ADRs. Public types and functions in `sim_*` crates get doc comments |
| CI | A `cargo doc` job that fails on broken links (`RUSTDOCFLAGS=-D warnings`). Doc comments are written but not enforced by a lint (decision 1) |
| Cargo aliases | `.cargo/config.toml` aliases for common commands (`cargo lint`, `cargo game`, `cargo headless`, `cargo test-fast`), with no new dependencies |

## Acceptance criteria
- [ ] Following only `GETTING_STARTED.md` in a fresh container builds the project and runs the game, headless and the tests. I'll do this myself and fix whatever the guide got wrong.
- [ ] `cargo doc --workspace --no-deps` builds with no warnings in CI, and every crate has an overview page.
- [ ] Every field in `assets/data/*.ron` appears in `DATA.md`. A test parses the RON files and fails if a field isn't documented.
- [ ] Every recipe in `HOWTO.md` points at code that exists, and a test checks that the file paths it names exist.
- [ ] README and CLAUDE.md link the handbook, and CLAUDE.md asks agents to keep it current: a change to schedules, data fields or tests updates the matching page in the same commit.

## Decisions (answered 2026-10-06)
1. **Doc comments are written but not enforced.** No `missing_docs` lint. CI still fails on broken doc links.
2. **Markdown in `docs/dev/` plus rustdoc.** No mdBook site.
3. **Kept current by checks plus a rule:** tests fail if a data field is undocumented or a HOWTO names a missing file, and CLAUDE.md requires doc updates in the same commit.
4. **Plan only for now;** build when asked.
