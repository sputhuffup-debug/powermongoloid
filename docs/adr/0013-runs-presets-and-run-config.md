# ADR 0013: Runs, presets and the run config

Date: 2026-10-07 · Status: Accepted · Requirements: GP-21, GP-23, GP-27, NFR-08

## Context

M4 turns the sandbox into roguelite runs ([PROGRESSION_DESIGN.md](../PROGRESSION_DESIGN.md), [M4_PLAN.md](../M4_PLAN.md)). A run is about an hour on a fresh seed: a landless band of ten against a rival warlord's seat, ending in a win, a loss or abandonment. Its start (where the band wakes up, who is in it, the run modifiers and three starting boons) must come from the seed and differ visibly between seeds (GP-27). Several things constrain how runs fit into the existing code:

- **The developed world must not change.** The 2048-tile world carries every M1-M3 test, the benchmarks and the stress mode (M4_PLAN default 6).
- **Runs must replay exactly** (NFR-08), including the player's picks, and replays of the M2/M3 scenarios must keep working.
- **`HeadlessConfig` and `Scenario` are `Copy`,** and the CLI, tests and `record_replay` rely on that.
- **The sim has to stop when a run ends,** although the game and the headless loop keep stepping it.
- **A latent bug (guide G1).** `setup_battle` wrote *player* commands before tick 0. They were recorded, and `verify_replay` re-ran the setup and replayed the log, so they applied twice.

## Decision

- **A run is a seed plus a `RunConfig`** (`sim_core::run`). For now the config holds only a `Difficulty`; M6 adds origins and unlocks. It is `Copy + Eq + Default`, so `Scenario::Run(RunConfig)` keeps `Scenario` and `HeadlessConfig` `Copy`. It travels in the replay header (`Replay.run`, `#[serde(default)]`), and `REPLAY_FORMAT` went from 4 to 5 once. The difficulty selects data, never RNG streams.
- **`RunState` is one resource, always present and hashed.** It holds `config: Option<RunConfig>` (`None` means no run, as in the developed world), a `pending` outcome, the final `outcome`, its `cause` and `ended_at`. `Offers`, `Chronicle`, `FactionModifiers` and `RunProgress` are always present too, with defaults outside a run, so the coverage test and saves see them from tick 0.
- **The run gate.** All nine `SimSet`s run only while `run_in_progress` holds. A serial step ends a run with `RunState::finish(outcome, cause)`: the first caller in a tick wins, and outside a run the call does nothing. `settle_outcome` (in `BookkeepingSet::Record`, after `record_events`) makes the outcome final and records `RunEnded`. The ending tick completes; every later tick is a no-op, so `SimTick` and the state hash freeze and commands are no longer consumed. Frontends keep stepping and need no special case.
- **Two presets in one `worldgen.ron`.** The file holds two complete settings blocks, `developed` and `run`. `WorldGenSettings::default_data()` stays the developed world. The run preset is a 1024-tile island with 4 regions and about 25 settlements. Only the run preset draws settlement sizes and places monuments; developed has `monuments.per_region: 0` and no size tiers. The developed world therefore generates the same terrain, settlements and roads as before and spawns no new entities.
- **Every run settlement is populated at a drawn size.** Tier counts are apportioned by weight (largest remainder), shuffled over the settlements, and each settlement's people are drawn uniformly within its tier, all from one new stream. The largest becomes the rival's seat, the unique largest at 450 or more. `populate` has a size mode keyed on `SettlementSize`; without sizes it runs the M2 path unchanged.
- **Monuments are placed by worldgen in step 1** (GP-23 placement), because the start needs to count them. Each region draws its kinds by weight. Each slot takes the suitable tile nearest a seeded anchor, under data-driven site rules. Monuments sit at least 52 tiles from every settlement centre (beyond the 45-tile threat radius, so a guard camp never frightens a village) and 40 tiles from each other. Holders, guards and bonuses come in step 4.
- **The start draws are a pure function,** `draw_start(seed, &Progression, &RunWorld)`, of the seed, the data and the generated world. The seat is the largest settlement. A start tile is open ground at least 40 tiles from every settlement and reachable on foot from the seat. The band starts in the region farthest from the seat (by settlement centroid) that has such a tile with 2-3 monuments within `monument_radius`, or else in the farthest region. Tiles are ranked by class (monument count, then the side away from the seat, then the drawn kind of ground, then the region's edge); within the best class, distance from the seat plus seeded jitter decides. The band template, the champion's trait, one or two run modifiers (no two from one group) and three distinct start boons are drawn, each from its own new stream at tick 0. With no App involved, a test can check twenty seeds in about ten seconds.
- **Setup is shared and writes no player commands.** `sim_headless::set_up_scenario` is the one setup path for headless runs, tests and `game_app`. Before tick 0, `setup_run` seats the player at the start, spawns the band and its champion (with one trait), applies one-off modifiers, posts the start offer and records `RunStarted`, all by editing state directly. Setup that needs orders writes `SimCommand::ai`, which is never recorded; `setup_battle` now does, which fixes G1.
- **Offers live in sim_core, and picks are commands.** `Offers` is hashed in sim_core, so the UI, saves and progression can read it without depending on sim_military. A pick is a recorded player command, `ChooseOffer { offer, choice }`, applied in `orders::apply` like every other command. The first pick takes the offer, so a second one (a double click) is refused as "no such offer". `AbandonRun` works the same way, and only the player may abandon.
- **The sim's own AI has its own input sub-set.** `SimInputSet::Ai` runs between `Inject` and `Consume`, so scripted or recorded commands and AI commands always arrive in the same order. Raiders moved there; the rival joins in step 7.
- **Run-only rules leave the developed world alone** (guide D6):
  - they are gated by `in_run`;
  - new draws use new stream keys;
  - outside the run preset, no entity is spawned before the villagers;
  - multipliers are exact no-ops at 1.0.

## Consequences

- **The developed world is unchanged.** `sim_headless --seed 42 --ticks 1200 --stats-every 600` prints the same terrain mix, settlements, roads and village statistics as before M4. Only the state hash differs: there are new hashed resources, and `WorldGenSettings` is hashed as its RON text. No golden hashes existed.
- **Skirmish and battle hashes changed.** `raiders::think` now runs after scripted input, battle setup orders are AI commands (so `--battle` replays log two fewer commands), and `sample_orders` draws from a wider range that includes offer picks and refused abandons. The M3 tests pass unchanged.
- **Format-4 replays are refused.** None were checked in.
- **The run island is cheap.** It has about 4,000-4,700 people. A tick costs about 1 ms in release, and setup about 0.5 s.
- **Start terrain leans to the coast** (12 of seeds 1-20), because the ranking puts the monument count and the side away from the rival before the drawn kind. `start.terrain_weights` and the ranking can be retuned; nothing depends on the current draws.
- **A run that ends logs fewer commands than its script.** The gate stops consuming commands, so its `RunResult.log` is shorter than the scripted list.
- **Tests:**
  - `run_start`: 20 seeds checked at world level, plus same-seed identity, the boon picked once, and abandoning.
  - `run_state_hash_covers_every_sim_component`, which every later M4 step extends with its state.
  - `a_run_replays`.
- **Events infrastructure landed with this step** (`SimEvent`, `Chronicle`, `EventFeed`, sorted recording), so later steps can report as they go. Its design is recorded with the chronicle in step 9.
