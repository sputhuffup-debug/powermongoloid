# Warlord Sim

Indirect-control medieval conquest simulation in Rust + Bevy.
The full spec is docs/SEED.md. Read it before any non-trivial change.
Requirement IDs (GP-, SIM-, RND-, UI-, NFR-) are stable; reference them
in commits, tests and comments.

## Current milestone
M4 "Rise" built 2026-10-09 (roguelite runs; docs/PROGRESSION_DESIGN.md, plan docs/M4_PLAN.md), in its playtest (docs/M4_PLAYTEST.md); its first verdict (2026-10-13: towns feel like points, actions unweighted) led to M4a.
Next: M4a "Lordship" (four ways to take a town with a lasting tenure and itemised loyalty, influence spheres, bandits and rival reprisals as fallout, town edicts, stewards, building orders, specialised towns and a working smithy chain; design docs/LORDSHIP_DESIGN.md, ADR 0020) is planned in docs/M4A_PLAN.md, awaiting go, splittable after its step 6, built in parallel lanes.
Then M4c "Saga" (kill lines, battle moments, epithets, earned traits, legends, plus quirks, mishaps, named weapons, kin and moods, told deadpan; combat effects (blood, bodies and severed parts, GPU or CPU), maimed survivors and a kill cam; design docs/SAGA_DESIGN.md, ADR 0019; its own ADR will be 0021) is planned in docs/M4C_PLAN.md, awaiting go, splittable after its step 8b; it follows M4a and comes before M4b.
(M0 complete 2026-10-05, M1, M2 and M3b complete 2026-10-06, M3 complete 2026-10-07, M4 built 2026-10-09, M4c planned 2026-10-12, M4a planned 2026-10-13; update this line as milestones complete)

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
                  (--timings --stats-every 250: mean/worst tick per window)
- Stress (RND-04): cargo run --release -p game_app -- --stress 20000
- Run (M4):       cargo run --release -p sim_headless -- --run --seed 7 --difficulty harsh --ticks 24000 --stats-every 2400
                  (--difficulty steady|chaotic|harsh; the game takes --seed and --difficulty too: cargo run -p game_app -- --seed 7)
- Bot run (M4):   cargo run --release -p sim_headless -- --run --bot --seed 1 --ticks 96000
                  (balance: cargo test --release -p integration_tests --test bot_run -- --ignored)
- Save/resume:    sim_headless --run --ticks 2400 --save day1.sav [--save-at <tick>], then --load day1.sav --ticks 1000
                  (or --load day1.sav --replay run.ron for the rest of a replay; the game saves with F5/F9)
- Chronicle:      sim_headless --run --bot --seed 1 --ticks 24000 --chronicle (the run told in words, then its summary)
- Village stats:  cargo run --release -p sim_headless -- --ticks 7200 --stats-every 600
                  (STATS_DETAIL=1 adds who isn't working, restless sleepers, the hungriest villager)
- Low-memory build (4-core cloud box): add `-j 3` to cargo build/test/clippy
- Docs:           RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
- Aliases (.cargo/config.toml): cargo game, cargo headless, cargo bot, cargo lint,
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
  two stockpiles directly. In a run, food dealings other than plunder read a
  village's pantry (storehouse + homes, `orders::food_sources`, ADR 0018);
  plunder stays storehouse-only. Every explicit dealing logs what moved or why not.
- Per-soldier systems query `Enlisted` (or the fighter grid's militia list),
  never all villagers; blows and deaths resolve serially in SpawnId order.
- Followers rejoin along the rally field toward the captain; only the captain
  walks the goal field.
- Fighters closing on a foe respect `formation.max_per_tile` (ADR 0012), or
  big battles crush into one blob that costs seconds per tick.

## Runs (ADR 0013)
- A run is a seed plus a `RunConfig` (`sim_core::run`, Copy + Eq + Default, in the replay header).
  Difficulty selects data (progression.ron and director.ron rows), never RNG streams. The run
  preset (1024-tile island, every settlement populated) is what `Scenario::Run` builds and the
  game's default start; the developed preset stays what `default_data()` means, and the M1-M3
  tests, benches and stress mode use it.
- Run-only rules are gated by `in_run`, draw from new stream keys, spawn nothing before the
  villagers outside the run preset, and leave the developed and skirmish output byte-identical.
- The run gate: all `SimSet`s run only while `run_in_progress`. End a run only with
  `RunState::finish(outcome, cause)` (first caller wins); `settle_outcome` records `RunEnded`.
  After that `SimTick` and the hash freeze, so frontends keep stepping and never special-case it.
- Offers (`Offers`, `post_offer`) live in sim_core. A pick is the recorded player command
  `ChooseOffer`, taken once. Setup writes state directly or `SimCommand::ai`, never player commands
  and never `SimEvent`s: offers via `Offers::post`; `report_start` (`MilitarySet::Start`, tick 0)
  tells the start from state, so a save before the first tick keeps it.
- `sim_military::progression::progress` (`MilitarySet::Progress`) is the hourly driver of run
  state: a fixed list of serial steps (end_battles, monument hold, revolts, requests, dominion
  tally, champion ranks, `rebuild_modifiers`, rival clock, director). Add a step at the end and
  document its slot; never reorder.
- The `sim_headless::bot` plays through ordinary player commands; `bot_run` (ignored, run in
  release by CI) guards the balance: it must win 3 of 5 seeds. Re-run it after any data change
  that touches a run. Change data when the game is unbalanced, the bot when it plays badly
  (HOWTO "Tune run balance").

## Standing and loyalty (ADR 0014)
- `Owner`, `Goodwill`, `SettlementLoyalty` on every populated settlement and `Loyalty` on every
  villager (sim_agents `standing.rs`, `loyalty.rs`) exist in all presets. Independent is owner 0.
- A settlement changes hands only through `sim_agents::set_owner`; sim_military calls it through
  `conquest::transfer` (conquest, revolt, accepted invitation), which reports the event, counts
  it and can end the run (the rival's seat).
- Goodwill changes only through `GoodwillLedger` deeds (`Deed`), posted from serial steps and
  applied once a tick by `apply_goodwill` (sorted, spread to neighbours). Never write it directly.
- Conquest is an `Attack` that breaks the militia and then holds the centre for
  army.ron's `conquest.hold_hours`; there is no instant capture. Owned villages feed and garrison their
  owner's armies (`garrison` in army.ron, the `Garrison` order) and are never plundered by them.
- Revolts are the daily serial `conquest::revolts`: no garrison means independence, a small one
  means a rising. Loyalty drift is tuned in villagers.ron `loyalty`; `conquest` and
  `invitation` guard it, so re-run them after any change.

## Monuments and modifiers (ADR 0015)
- `sim_core::FactionModifiers` is rebuilt from scratch once a game hour by
  `sim_military::rebuild_modifiers` (held monuments, boons, champion traits, run modifiers) and
  written by nothing else. Hand-set values do not survive an hour in a full app.
- Read it at the point of use: `mult(faction, stat)` or `value(..)`. Never fold a bonus into
  `Economy`, `Villagers` or `Army` data: they hash only their data text, so the behaviour would
  change without the hash noticing. Run the consumer with a hand-built resource to test it.
- A consumer must be an exact no-op at 1.0 (`x * 1.0`, `d / 1.0`, `+ 0.0`), or the developed
  world and the M2/M3 tests change. A yield bonus beyond the recipe's units goes in with
  `add_over_capacity`. A new `Stat` needs a consumer (HOWTO "Add a modifier").
- Monuments are placed by worldgen (run preset only), held by soldiers standing at them
  (`monuments::hold`), and guarded by ordinary armies that carry `Guard` and `NoUpkeep`.
  Bonuses are capped per kind; a region's complete set adds `set_bonus` beyond the cap.

## Saves and events (ADR 0016, ADR 0017)
- Saves are serde + postcard + zstd behind a versioned `SaveHeader` (`sim_core::save_world`,
  `load_world`, `SAVE_FORMAT`). Never bincode. Save between ticks (tick 0 before the first step
  counts, except for a battle: `setup_battle` leaves AI orders in flight, which a save then loses,
  ADR 0016); `load_world` validates everything before it changes the world.
- Every hashed type is also saved: implement `SaveState` (`save_fields!`, `save_newtype!`,
  `save_unit!`, `save_serde!`) and register it with `register_saved` next to `register_hashed`.
  `every_hashed_type_is_saved` fails otherwise. Exempt types the next tick reads before
  rebuilding them (`ArmyIndex`, `FighterGrid`) are saved too.
- `Entity` fields save as SpawnId + 1 and load as the entity now carrying that SpawnId. Saved
  enums are append only, as `CommandKind`, `OrderSpec`, `SimEventKind` and `Cause` are.
- The map is regenerated from the seed on load and checked against the saved digest, so
  worldgen changes make old saves unloadable. Bump `SAVE_FORMAT` when the layout changes.
- Events: any sim crate writes a `SimEvent`; `record_events` (`BookkeepingSet::Record`) sorts
  them and feeds the `Chronicle` (hashed and saved) and the `EventFeed` (exempt, a UI ring,
  saved so cursors survive). The sim stores no text: words come from
  `sim_military::chronicle::describe` and progression.ron `chronicle.causes`. Name a cause
  wherever there is a why.

## Rendering (ADR 0008, ADR 0009, ADR 0019)
- Detail bands come from assets/data/render.ron: stylized textures inside the
  detail distance, flat low-poly beyond, then billboards, then dots.
- Textures are generated in game_render::art; replace by swapping handles.
- Combat effects are render-side requests built from sim records (landed blows,
  kills, falls, maims; never a visual's health) and drawn by one backend: GPU
  (bevy_hanabi), CPU fallback or Off. GPU and CPU differ only in spray density,
  and Off draws no spray; bodies, pools and parts are plain meshes, the same on
  every backend. Gore follows the tone, not the backend.
- Render systems may read sim state (the effects collector in FixedLast copies
  `Casualties` only when `SimTick` advanced) and never write it. Effects are
  never saved; the kill cam touches only `Time<Virtual>`; every effect stays
  inside its band and render.ron's caps.
- bevy_hanabi is pinned (`=0.19.0`, default features off, `3d`) and named only in
  game_render's `effects/gpu.rs`. Never add `HanabiPlugin` yourself:
  `EffectsPlugin::finish` adds it only when the render device has compute
  shaders (ADR 0019). Only game_app adds `EffectsPlugin`, so headless builds,
  tests and CI never need a GPU.

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
