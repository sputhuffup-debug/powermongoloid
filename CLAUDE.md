# Warlord Sim

Indirect-control medieval conquest simulation in Rust + Bevy.
The full spec is docs/SEED.md. Read it before any non-trivial change.
Requirement IDs (GP-, SIM-, RND-, UI-, NFR-) are stable; reference them
in commits, tests and comments.

## Current milestone
M3 (M0 complete 2026-10-05, M1 and M2 complete 2026-10-06, M3b built 2026-10-06; M3 awaits its stress run and playtest; update this line as milestones complete)

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
- Armies:         sim_headless --skirmish (playtest start) or --battle 1000 (two armies),
                  with --stats-every 600 for army lines, --timings for per-stage cost
- Stress (RND-04): cargo run --release -p game_app -- --stress 20000
- Village stats:  cargo run --release -p sim_headless -- --ticks 7200 --stats-every 600
                  (STATS_DETAIL=1 adds who isn't working, restless sleepers, the hungriest villager)
- Low-memory build (4-core cloud box): add `-j 3` to cargo build/test/clippy
- Docs:           RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
- Aliases (.cargo/config.toml): cargo game, cargo headless, cargo lint,
                  cargo test-fast (unit tests), cargo test-all

## World (ADR 0005)
- Worldgen parameters live in assets/data/worldgen.ron (embedded at compile time).
- Change tiles only through `WorldMap::set_terrain`; it bumps chunk revisions
  (the renderer rebuilds only those chunks) and keeps the state hash current.
- Rendering and UI never write to sim entities, not even marker components;
  track render-side state in render-side resources or `Local`s.

## Villagers and economy (ADR 0006, ADR 0007)
- Behaviour is tuned in assets/data/villagers.ron and economy.ron; the
  village_day and famine tests guard the tuning. Re-run them after any change.
- Agents never touch a building's Stockpile directly: post a `Transfers` order
  in `EconomySet::Post`; `resolve_transfers` applies orders in SpawnId order.
- Per-agent systems may run in parallel only if they write the agent's own
  components; anything shared goes through a sorted, serial step.
- Every decision records a `DecisionTrace`; use it (and the inspector) to
  debug behaviour before changing weights.

## Armies (ADR 0010, ADR 0011)
- Tuning lives in assets/data/army.ron; army_march, posture, starving_army and
  orders tests guard it. Re-run them after any change.
- Orders reach captains only as `SimCommand`s, validated against the issuer.
  AI commands carry `CommandOrigin::Ai` and are not recorded in replays.
- Settlement dealings (recruit, food, arms, trade) are the army's serial
  logistics step in `orders::run_orders`; nothing else moves goods between
  two stockpiles directly.
- Per-soldier systems query `Enlisted` (or the fighter grid's militia list),
  never all villagers; blows and deaths resolve serially in SpawnId order.
- Followers rejoin along the rally field toward the captain; only the captain
  walks the goal field.

## Rendering (ADR 0008, ADR 0009)
- Detail bands come from assets/data/render.ron: stylized textures inside the
  detail distance, flat low-poly beyond, then billboards, then dots.
- Textures are generated in game_render::art; replace by swapping handles.

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

## Developer handbook (docs/dev, M3b)
Humans develop alongside you; keep their docs current in the same commit:
- New or changed data field: docs/dev/DATA.md (tests/tests/docs.rs fails otherwise).
- New, moved or reordered system or set: the schedule table in docs/dev/ARCHITECTURE.md.
- New test: docs/dev/TESTING.md. New term: docs/dev/GLOSSARY.md.
- A new kind of change others will repeat: a recipe in docs/dev/HOWTO.md
  (every path it names must exist; the docs test checks).
- Public items in sim_* crates get a `///` doc comment; `cargo doc` must stay warning-free.

## Definition of done for any task
1. cargo fmt, clippy and tests pass.
2. The determinism test still passes.
3. Relevant acceptance criteria in docs/SEED.md are checked.
4. Significant design decisions get a short ADR in docs/adr/.
5. The developer handbook and rustdoc reflect the change (see above).

## Build settings
Use opt-level = 1 for dev and opt-level = 3 for dependencies in dev profile
so the game is playable in debug builds.
