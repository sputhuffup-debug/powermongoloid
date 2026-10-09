# M4 "Rise" build guide (code map + shared decisions)

For the implementers of the 12 steps in `docs/M4_PLAN.md`. Read `CLAUDE.md`, `docs/M4_PLAN.md`, `docs/PROGRESSION_DESIGN.md`, then this file. Line numbers are as of commit `7216095` (2026-10-07) and drift as you edit, so search by name.

Section 2 decisions are **binding for every step**. If one of them turns out wrong, change it here and in every step that depends on it. Don't diverge quietly in one step.

---

## 1. Architecture summary

### 1.1 Crates and dependency order (verified in each `Cargo.toml`)

```
sim_core                (bevy_app/ecs/math =0.19.1, rand 0.10, rand_chacha 0.10, ron, serde, tracing)
  └ sim_world           (+ sim_core)
      └ sim_economy     (+ sim_core, sim_world)
          └ sim_agents  (+ sim_core, sim_world, sim_economy; rand_chacha)
              └ sim_military (+ all four above; rand, NO rand_chacha)
sim_knowledge, sim_lod, sim_ai   (sim_core only; 13-line placeholders, empty until M5)
sim_headless            (every sim_* crate, rand, criterion[dev]; bevy_ecs "multi_threaded")
game_render             (bevy, sim_core/world/agents/economy/military, ron, serde)
game_ui                 (bevy, bevy_egui optional [feature debug_ui], sim_*, game_render)
game_app                (bevy, sim_core, sim_headless, sim_military, sim_agents, sim_world, game_render, game_ui)
tests/ = integration_tests (dev-deps: sim_core, sim_headless, sim_world, sim_agents, sim_economy, sim_military, bevy_ecs, bevy_math; NO rand/serde/ron)
```
- A crate can read types only from crates above it in this tree. **sim_agents cannot see sim_military.** Anything that villagers' systems (`act::perform`, loyalty drift) read must therefore be defined in sim_agents or a crate below it.
- `postcard` and `zstd` are **not** in `[workspace.dependencies]` yet (root `Cargo.toml:12-38`). bevy_math has no `serialize` feature.
- Profile: dev `opt-level=1`; `[profile.dev.package."*"]` opt 3; explicit opt 3 for sim_world, sim_agents, sim_economy and sim_military (`Cargo.toml:48-73`). **sim_core, sim_headless and integration_tests run at opt 1 in `cargo test`** (TESTING.md:11 claims otherwise).
- `clippy.toml` denies `rand::rng`. Workspace lint `disallowed_methods = "deny"`.

### 1.2 App construction
- `sim_headless::SimPlugins { seed, population: Option<u32>, worldgen: WorldGenSettings }` (`crates/sim_headless/src/lib.rs:39-74`) adds, in order: `SimCorePlugin{seed}`, `WorldPlugin{settings, threads}`, `EconomyPlugin`, `AgentsPlugin{population}`, `MilitaryPlugin`, `KnowledgePlugin`, `LodPlugin`, `AiPlugin`. `SimPlugins::new(seed)` always uses `WorldGenSettings::default_data()`.
- Plugin `finish()` runs in add order inside `app.prepare_sim()` (`sim_core/src/step.rs`, finish + cleanup):
  1. `WorldPlugin::finish` (`sim_world/src/lib.rs:135`) runs `generate_world(SimSeed, settings, threads)`, then `install_world` (`:145`). That spawns settlements with `NextSpawnId::allocate()` + `world.spawn` and pushes names into NameTable, then inserts WorldMap, RoadNetwork and WorldGenReport.
  2. `AgentsPlugin::finish` (`sim_agents/src/lib.rs:104-109`) runs `population::populate` only if a `WorldMap` exists.
  3. Scenario setup comes **after** `prepare_sim()` and before tick 0: `headless_app` (`sim_headless/src/lib.rs:153-177`, `match config.scenario`) and `game_app::set_up_start` (Startup, exclusive, `game_app/src/main.rs:79-101`; it duplicates the match).
- `HeadlessConfig { seed, population, threading, world_size, scenario }` derives **Copy** (`lib.rs:124-150`). `main.rs` and `record_replay` (`:242-260`) reuse `config` after moving it. `worldgen()` = `default_data().with_size(world_size)`. `headless_app` always passes `population: Some(config.population)`.
- `Scenario { Villages(default), Skirmish, Battle(u32) }` derives Copy+Eq+Default (`lib.rs:88-121`). `name()` and `parse()` are used for `Replay.scenario`. `verify_replay` (`:283`) uses `parse(..).unwrap_or_default()`, so an unknown name silently becomes Villages.
- game_app: `Time::<Fixed>::from_hz(10)`, `SimPlugins::new(seed)`, `Start {Villages, Skirmish(default), Stress(n)}` (`main.rs:20-46`). `OpeningShot` + `frame_start` run in PostStartup only.

### 1.3 Schedule (FixedUpdate, 10 Hz)
`SimSet {Input, Knowledge, Decide, Move, Act, Needs, Aggregate, Lod, Bookkeeping}` is chained (`sim_core/src/schedule.rs:65-82`). Sub-sets and systems today:

| Set | Contents (crate) |
| --- | --- |
| Input | `SimInputSet::Inject` → `Consume`, chained (`sim_core/src/lib.rs:77-82`). Inject holds `inject_scripted → record_commands` (core, chained) and `raiders::think` (military, **unordered** vs core). Consume holds `drain_commands` (core) and `MilitarySet::Orders` = `collect_commands → apply_commands.run_if(has_commands) → run_orders.run_if(any_with_component::<Captain>)` |
| Knowledge | `MilitarySet::Index`: `build_index → update_threats` |
| Decide | `AgentSet::Decide`: `ai::decide`; then `MilitarySet::Decide`: `soldiers::decide` |
| Move | `AgentSet::Move`: `plan_paths → move_agents`; then `MilitarySet::Move`: `move_captains → move_soldiers` |
| Act | `EconomySet::Post` (holds `act::perform` [AgentSet::Perform] and `upkeep::eat` after it) → `EconomySet::Resolve` (`resolve_transfers`); then `MilitarySet::Combat`: `resolve_strikes → find_fallen → casualties.run_if(anyone_fell)` |
| Needs | `AgentSet::Needs`: `update_needs`, `update_food_security` (**not chained**, `sim_agents/src/lib.rs:81-83`); then `MilitarySet::Upkeep`: `soldier_upkeep → civilian_morale → tally → apply_desertions.run_if(..)` |
| Aggregate, Lod | empty |
| Bookkeeping | `refresh_chunk_digests`, `rebuild_grid` (world), `advance_tick` (core, `pub(crate)`, **no ordering**: nothing guarantees it runs last) |

`timing::add_markers` puts marker systems between the sets (outside any SimSet). Exclusive systems (`fn(&mut World)`) are used for serial steps and gated by `run_if`.

### 1.4 Where state lives (hashed unless noted)
- **sim_core:**
  - Resources: SimSeed, Calendar, SimTick, SimRng (unused), NextSpawnId, CommandsProcessed, NameTable.
  - Components: SpawnId, Position(IVec2), Motion, Allegiance(FactionId), DetailLevel.
  - Exempt: SimTimings, CommandLog, CommandScript.
- **sim_world:**
  - Resources: WorldGenSettings (hashed as RON text), WorldMap (per-chunk digests), RoadNetwork.
  - Components: Settlement {name, region, center} (Copy), Mobile.
  - Exempt: WorldGenReport, PathCache, FlowCache, SpatialGrid (rebuilt in Bookkeeping).
- **sim_economy:**
  - Resources: Economy (hashed as data RON).
  - Components: Stockpile (buildings and captains' baggage), Inventory, Transfers (None between ticks), Building, Workplace, Home, SettlementEconomy {storehouse, homes, workplaces, population}, Occupation.
- **sim_agents:**
  - Resources: Villagers (data RON).
  - Components: Villager {name, age_days, home, settlement}, Employment, Needs, Activity, Travel, NextDecision, FoodSecurity, Enlisted, CombatStats {stance: Civilian | Militia{against} | Soldier}, Threat {level, from, provoked_by, provoked_until}, Strike.
  - Exempt: DecisionTrace, PopulationReport.
- **sim_military:**
  - Resources: Army (data RON), Factions {seats}, ArmyStats {Vec<FactionStats>, hand-written StateHash}.
  - Components: Captain (hand-written StateHash, `components.rs:119`), Follower, Equipment, Soldier, Raider.
  - Exempt: MilitaryLog (64 lines), CommandInbox, ArmyIndex, FighterGrid, Casualties, Deserters.
- Settlement-level components (SettlementEconomy, FoodSecurity, Threat) exist **only on populated settlements** (`population.rs:414-423`). `find_settlement` (`orders.rs:51`) finds only those.
- There is no Owner, loyalty, goodwill, tier, monument, run, offer or event concept anywhere yet.

### 1.5 Commands and replays
- `SimCommand { issuer: FactionId, kind: CommandKind, origin: CommandOrigin {Player(default), Ai} }`, built with `::player` / `::ai` (`sim_core/src/command.rs:15-50`).
- `CommandKind {Noop, IssueOrder{captain:u64, order, queue}, SetPosture, CancelOrders, AppointCaptain{follower}}` (`:111`). `OrderSpec` (`:80`) is **Copy+Eq**, integers only, and names entities by SpawnId u64.
- `inject_scripted` writes `CommandScript.by_tick[tick]`. `record_commands` logs Player-origin commands only into `CommandLog`. `drain_commands` counts everything into `CommandsProcessed`.
- The only consumer is `sim_military::orders::apply` (`orders.rs:127-172`): an exhaustive match, applied in arrival order. Any `Err(&'static str)` bumps `ArmyStats.of(issuer).rejected_commands` and logs "order refused".
- Validation: `own_captain` (`:118`), `validate_spec` (`:91`, exhaustive), `appoint` (`:176`, max_captains 9).
- `Replay { format, seed, population, world_size, #[serde(default)] scenario, ticks, checkpoint_every, commands, checkpoints }` (`replay.rs:19-37`). `REPLAY_FORMAT = 4`, and `from_ron` rejects any other format. No replays or golden hashes are checked in (verified), and CI records and verifies in the same job.
- Messages live exactly 2 `run_sim_tick`s headless. In the game, buffers swap only after a fixed tick, so a command written while paused applies on the next tick and is recorded at that tick.
- A command written between `app.step(1)` calls is logged at the current SimTick and replays identically (`determinism.rs:71`). That is the bot and UI pattern.

### 1.6 Hashing
- `StateHash` trait + FNV `StateHasher` (`hash.rs:32`) with helpers `bytes/bool/u8/u16/u32/u64/u128/i32/f32/ivec2/vec2/entity`. `entity` writes `1+SpawnId`, or 0 for none. There is no i64/f64/str/Option helper: write a tag byte, then the value.
- `AppHashExt::{register_hashed, register_hashed_resource, exempt_from_hash(reason)}`. Hash order = registration order, and the type name is part of the hash.
- `HashRegistry.covered` merges hashed and exempt types (`hash.rs:126-133`).
- `unhashed_state` flags components on SpawnId entities, and resources whose type path starts with `sim_`, that are neither hashed nor exempt. It sees only types **present at that moment**. Enforced by `determinism.rs:139` (a 1,500-tick skirmish).

### 1.7 RNG
- `SimSeed::stream(key, tick) -> ChaCha8Rng` (`rng.rs:17`).
- `sim_core::rng::rng_key(&str)` is a const fn and is **not re-exported** (the doc comment wrongly says `rng_key!`).
- Per-entity streams are `rng_key("crate::system") ^ spawn_id` at the current tick.
- No system borrows `SimRng`: don't start.
- Use `use rand::RngExt;` for `random_range`/`random_bool`. sim_military and sim_headless cannot name `ChaCha8Rng` (no rand_chacha dep), so write helpers generic over `R: rand::Rng + ?Sized` (as `sim_headless::near` does).

### 1.8 Time
- `SIM_TICK_HZ = 10`. Calendar: 2,400 ticks/day (4 real minutes at 1x), phases 0.15/0.45/0.15/0.25, 30 days/season.
- `Calendar::at(tick).day` is 0-based. There are no hour helpers. `ClockReading::hours_minutes` is a non-uniform 24-hour face.
- `SimTick` is 0 during the first tick.

### 1.9 Data loading
- Every `assets/data/*.ron` is embedded with `include_str!` and parsed with `ron` into a `*Data` struct, which is wrapped in a resource with resolved fields.
- `default_data()` panics on invalid data.
- The resource hashes `ron::to_string(&data)`, so new fields are hashed automatically.
- Files: calendar (sim_core), worldgen (sim_world), economy (sim_economy), villagers (sim_agents), army (sim_military; `Army::from_ron(text, &Calendar, &Economy)`, no `#[serde(default)]` anywhere), render (game_render).
- `tests/tests/docs.rs` requires every lowercase `ident:` in every `assets/data/*.ron` to appear in backticks inside a DATA.md section headed exactly `## <file>` (`documented_words`, `:90`). A missing section panics. Comments `//` and string contents are stripped; `/* */` is not.

### 1.10 Tests, CI, docs
- Integration tests are one binary per `tests/tests/*.rs`, with `mod common;` helpers (`command`, `order`, `spawn_id`, `pos_of`, `followers`, `villages`, `passable_near`, `quantile`).
- Pattern: `headless_app(HeadlessConfig::new(SEED))`, edit before tick 0 (`set_terrain`, `spawn_army`, `cut_off_farmers`), send input only through `common::order`/`command`, step in 10–100-tick chunks, print numbers, assert.
- CI (`.github/workflows/ci.yml`): fmt, clippy `--all-targets -D warnings`, `cargo test --workspace` (~4 min), `cargo doc` with `-D warnings`, and a release replay smoke run (villages only). The nightly bench fails on a >10% mean regression.
- Docs checks: `every_data_field_is_documented`, `howto_paths_exist` (every backticked `crates/ tests/ assets/ scripts/ docs/ .github/` path in the handbook must exist), `markdown_links_resolve`.

---

## 2. Cross-cutting decisions (binding)

### D1. Type ownership

| Type | Crate / module | Why |
| --- | --- | --- |
| `RunConfig {difficulty: Difficulty}` (Copy, Eq, Default, serde), `Difficulty {Steady(default), Chaotic, Harsh}` | `sim_core::run` | Replay header (sim_core) and HeadlessConfig/Scenario must stay Copy |
| `RunState {config: Option<RunConfig>, pending: Option<(RunOutcome, Cause)>, outcome: Option<RunOutcome>, cause, ended_at}` (resource, hashed, always present), `RunOutcome {Won, Lost, Abandoned}`, `RunState::finish(outcome, cause)` (first caller wins), run conditions `run_in_progress`, `in_run` | `sim_core::run` | The schedule gate lives in sim_core. RunConfig is folded into RunState, so there is one resource |
| `Offers {next_id, pending: Vec<Offer>}` (hashed, always present), `Offer {id, faction, kind: OfferKind, choices: Vec<u16>, posted, expires: Option<u64>}`, `OfferKind {StartBoon, DominionBoon{region:u8, step:u8}, Trait{person:u64}, Invitation{settlement:u64}, Hire{captain:u64}}` | `sim_core::run` | UI, save and progression all read it without depending on sim_military |
| `SimEvent` (Message), `SimEventKind`, `Cause`, `Chronicle` (hashed, saved), `EventFeed` (exempt UI ring with `seq`), `record_events` system | `sim_core::events` | Any sim crate can emit |
| `Stat` enum, `FactionModifiers` (hashed resource) | `sim_core::modifiers` | Read by sim_agents (yield, smithing, loyalty) and sim_military. Written only by sim_military |
| `FactionId` named constants | `sim_core::components` (`impl FactionId { pub const INDEPENDENT, PLAYER, RAIDERS, RIVAL, BEASTS }` = 0..4) | Replaces the hard-coded `FactionId(1)` in game_ui, spawn.rs and sample_orders |
| `SaveState`, `SaveWriter/Reader`, `AppSaveExt`, `SaveRegistry`, `SaveHeader` | `sim_core::save` | Every crate registers its types |
| `WorldPreset {Developed(default), Run}`, `WorldGenPresets {developed, run}` | `sim_world::settings` | |
| `MonumentKind`, `Monument {kind, region, name}` (component), `MonumentSite` | `sim_world::monuments` (+ `worldgen/monuments.rs`) | Worldgen places them |
| `SizeTier {Hamlet, Village, Town}`, `SettlementSize {tier, people}` (component, inserted only when drawn) | `sim_world` (lib.rs or `sizes.rs`) | Drawn at worldgen, before populate |
| `Owner {faction, since}`, `Goodwill {by_faction: Vec<(FactionId, f32)>}` (sorted, −1..1), `GoodwillLedger` (hashed queue), `set_owner`, `apply_goodwill` | `sim_agents::standing` | Read by sim_agents (loyalty, allegiance), written by sim_military. Follows the precedent of `Threat` |
| `Loyalty(f32)` per villager, `SettlementLoyalty {mean, min, below_since: Option<u64>}` | `sim_agents::loyalty` | |
| `Holder {faction, since, contender: Option<(FactionId,u64)>, last_presence}`, `Guard {monument: u64}`, `NoUpkeep` marker | `sim_military::monuments` | Military state. game_* may read it |
| `RunProgress` (start tile, regions, rival seat, band, modifiers, boons, succession_used, dominion flags), `Progression` data (progression.ron), `Champion`, `ChampionRank`, `Traits(Vec<u16>)` | `sim_military::progression` | |
| `Veteran {battles, rank, fought}` | `sim_military::veterans` | |
| `RequestBoard` (resource) and `Request` | `sim_military::requests` | Detection needs armies and monuments. sim_agents never reads requests |
| `WildFood` patch grid | `sim_military::wild` | |
| `RivalState`, `rival::think`, `rival::clock` | `sim_military::rival` | |
| `DirectorData` (director.ron), `Director` state | `sim_military::director` | |
| `Bot`, `run_bot`, `BotRun` | `sim_headless::bot` | Needs rand; the tests crate has none |

- There is no separate Treatment component. **Goodwill toward the owner is the treatment term** in the loyalty target. Seizing, plundering, killing, feeding and requests all move goodwill.
- Prefer **always-present components with Option/zero fields** over components inserted on events, so coverage tests see them from tick 0:
  - Insert Owner, Goodwill and SettlementLoyalty on every populated settlement in `populate`, for every preset.
  - Insert Loyalty on every villager.
  - Insert Veteran on every soldier in `spawn_army` and `enlist`.
  - Insert Holder on every monument at run setup.

### D2. Factions
- Append two factions to `army.ron` `factions`: `(name: "Rival", color: ..)` at index 3 and `(name: "Beasts", color: ..)` at index 4. Never reorder 0/1/2, because M3 tests use 1 and 2.
- `at_war` stays "any two non-zero, different factions", so raiders, rival and beasts also fight each other. Side masks allow 32 factions.
- `game_ui::army::PLAYER` becomes `FactionId::PLAYER`.
- Guard camps:
  - Raider camps use `FactionId::RAIDERS` **without** the `Raider` marker, plus `Guard`.
  - Beast lairs use `FactionId::BEASTS` plus `Guard`.
  - Both get `NoUpkeep` on their soldiers.
- `raiders::think` prey: `Without<Raider>, Without<Guard>` and, in a run, only `FactionId::PLAYER` captains.

### D3. Module and data-file names
- **sim_core:** `run.rs`, `events.rs`, `modifiers.rs`, `save.rs`, plus `pub use` of the key types at the root. Calendar gets `ticks_per_hour()`, `hours(f32)->u64` and `days(f32)->u64`. Schedule gets `BookkeepingSet` and the `on_hour`/`on_day` helpers.
- **sim_world:** `monuments.rs`, `worldgen/monuments.rs`, `worldgen/sizes.rs`. settings.rs gets presets. map.rs gets `region_count()` and `is_region_edge()`.
- **sim_agents:** `standing.rs`, `loyalty.rs`.
- **sim_military:**
  - `progression/` (mod.rs: data, `RunProgress`, setup and the progress driver; `start.rs`; `offers.rs`; `dominion.rs`; `champion.rs`)
  - `conquest.rs`, `wild.rs` (**not** "forage": `orders::forage`, `Captain.last_forage` and `upkeep.forage_radius` already mean settlement auto-gather)
  - `requests.rs`, `drifters.rs`, `monuments.rs`, `veterans.rs`, `rival.rs`, `director.rs`, `chronicle.rs` (event text)
- **sim_headless:** `bot.rs`. **Never add a `run` module or a root re-export named `run`**: `lib.rs:8` doc-links `[`run`]`, and rustdoc `-D warnings` fails on ambiguity. Also don't `use sim_core::run;` at the sim_headless root.
- **Data:**
  - New `assets/data/progression.ron` (`sim_military::Progression`) and `assets/data/director.ron` (`sim_military::DirectorData`).
  - DATA.md sections headed exactly `## progression.ron (\`sim_military::Progression\`)` and `## director.ron (\`sim_military::DirectorData\`)`.
  - worldgen.ron gains presets. villagers.ron gains `loyalty:` and `goodwill:` (the spread is applied in sim_agents). army.ron gains `veterans:`, `conquest:` and `garrison:`. economy.ron gains an optional `recipes[].modifier: Some(FoodYield|SmithSpeed)` (`#[serde(default)]`).
  - Goodwill **deltas per deed** go in progression.ron `goodwill_deeds` (applied by sim_military).
- **ADRs:**
  - 0013: runs, presets and RunConfig
  - 0014: ownership, goodwill and loyalty
  - 0015: monuments and FactionModifiers
  - 0016: save format implementation (follows ADR 0001)
  - 0017: events and chronicle

  Write each in the step that makes the decision; step 12 reviews them.

### D4. New commands and validation
- Append, never reorder (postcard saves encode the variant index):
  - `CommandKind::ChooseOffer { offer: u64, choice: u8 }` and `CommandKind::AbandonRun`
  - `OrderSpec::Garrison { settlement: u64 }` (step 2), `Forage { radius: u16 }` (step 3), `Give { settlement: u64 }` (step 3)
  - hash tags 8, 9 and 10 in `hash_spec` (`sim_military/src/components.rs:33`)
- **Every** new CommandKind is applied in `sim_military::orders::apply` (`orders.rs:127`), in arrival order, returning `Err(&'static str)` on refusal so `rejected_commands` counts it:
  - `ChooseOffer` → `progression::choose(world, cmd.issuer, offer, choice)`. Refuse with "no such offer" if the offer is unknown or already taken, "not your offer" if `offer.faction != issuer`, "no such choice" if `choice >= choices.len()`, and "no run" if `RunState.config` is None.
  - `AbandonRun` → `progression::abandon(world, issuer)`. Refuse if not in a run, if the issuer isn't PLAYER, or if the run is already ended. Otherwise call `RunState::finish(Abandoned, Cause::Player)`.
  - Each new OrderSpec is checked in `validate_spec`: Garrison needs the settlement **owned by the issuer**; Give and Forage need a valid settlement or radius; Attack refuses your own settlement.
- A UI double-click sends two ChooseOffers. The second is refused ("no such offer") because `choose` removes the offer from `pending`.
- Add all new kinds and orders to `sample_orders` (`sim_headless/src/lib.rs:348` `0..12` and `:371` `0..10`; widen the ranges). Changing the ranges changes the stream, which is fine because no golden hashes exist. In skirmish they are mostly refused, which exercises the refusal path.
- AI commands (rival, raiders) are `SimCommand::ai` and are never recorded. **Setup code never writes player-origin messages** (see G1).

### D5. Run config, presets, app construction
- `SimCorePlugin { seed, run: Option<RunConfig> }` inserts `RunState { config: run, .. }` and also inits `Offers`, `Chronicle`, `EventFeed` and `FactionModifiers`, registered hashed or exempt.
  - Update both construction sites: `sim_core/src/step.rs:47` and `sim_headless/src/lib.rs:60`.
- `SimPlugins` gains `run: Option<RunConfig>` and `generate: bool` (step 8, default true). Constructors:
  - `SimPlugins::new(seed)`: unchanged meaning (developed).
  - `SimPlugins::run(seed, cfg)`: `worldgen = WorldGenSettings::preset(WorldPreset::Run)`, `population: None`, `run: Some(cfg)`.
- `Scenario::Run(RunConfig)`. `name()` → `"run"`; `parse("run")` → `Scenario::Run(RunConfig::default())`, with `verify_replay` substituting `replay.run.unwrap_or_default()`. Make `verify_replay` **panic with a clear message on an unknown scenario name** instead of `unwrap_or_default()`.
- `HeadlessConfig` keeps Copy. Add `HeadlessConfig::run(seed, cfg)`, which sets `scenario: Scenario::Run(cfg)` and `world_size = WorldGenSettings::preset(Run).size` (1024).
  - `worldgen()` → `WorldGenSettings::preset(self.preset()).with_size(self.world_size)`, where `preset()` = Run iff the scenario is Run.
  - `headless_app` passes `population: None` and `run: Some(cfg)` for Run.
- **Presets:** worldgen.ron becomes `(developed: (<full WorldGenSettings>), run: (<full WorldGenSettings>))`, parsed into `WorldGenPresets`.
  - `WorldGenSettings::default_data()` **must keep returning developed** (2048, regions 10, 40..=80). It is relied on by `settings.rs:159`, `tests/tests/worldgen.rs:21,83,178`, `HeadlessConfig::new`, `sim_economy/src/layout.rs:~436`, `game_ui/src/selection.rs:~202` and `worldgen/roads.rs:~98`.
  - `WorldGenSettings::from_ron` keeps parsing a single struct (unit tests use it).
  - New fields in WorldGenSettings, explicit in both presets: `monuments: MonumentSettings` (`per_region: 0` in developed) and `sizes: SizeSettings { tiers: [], seat_min: 0 }` in developed.
- **Run setup** = `sim_military::progression::setup_run(world: &mut World) -> Option<RunStart>`. It reads `RunState.config`, runs after `prepare_sim`, and is called from one shared `sim_headless::set_up_scenario(world, Scenario)`. headless_app and game_app's `set_up_start` both call that function; delete the duplicated match. Setup edits state directly and never writes player messages.
- Replay: `#[serde(default)] pub run: Option<RunConfig>` on `Replay`. Bump `REPLAY_FORMAT` 4→5 **once, in step 1**. RON encodes variants by name, so later appended OrderSpec variants don't need another bump. Bump again only if a header field changes.

### D6. How tests select developed vs run
- `HeadlessConfig::new(seed)` is developed + Villages, and stays byte-for-byte today's behaviour. All M1–M3 tests and benches stay on it.
- `HeadlessConfig::run(seed, RunConfig::default())` is the run island. Add helpers to `tests/tests/common/mod.rs`:
  - `run_app(seed) -> App`
  - `choose(app, offer, choice)` wrapping `command(app, 1, ChooseOffer{..})`
  - `start_offer(app) -> u64`
  - `champion(app) -> Entity`
  - `owner_of(app, settlement) -> FactionId`
  - `step_days(app, n)`
  - `step_until(app, max_ticks, pred) -> Option<u64>`
  - `chronicle(app) -> Vec<ChronicleEntry>`
- World-only checks (20 seeds) call `sim_world::generate_world(seed, &WorldGenSettings::preset(WorldPreset::Run), default_threads())` plus the pure `sim_military::progression::draw_start(..)`, with no App and no population.
- **Developed-invariance rule** (M4_PLAN default 6): nothing new may change developed or skirmish outcomes. Concretely:
  1. Add no draws to existing RNG streams; use new keys.
  2. Spawn no new SpawnId entities before villagers in developed. Monuments and SettlementSize exist only in the run preset.
  3. Multipliers must be exact no-ops at 1.0 (`x * 1.0`; `(d as f32 / 1.0).round() as u64 == d` for d < 2^24), and stochastic rounding must not draw when the fraction is 0.
  4. Run-only systems are gated by `in_run`. New always-on behaviour (conquest step 2, owner-aware threat, loyalty drift) must leave today's tests passing; re-run the guarded tests named in CLAUDE.md after each step.

### D7. Schedule additions and cadence
- `SimInputSet::Ai`: add the variant and chain `(Inject, Ai, Consume)` in `sim_core/src/lib.rs:77-82`. Move `raiders::think` into Ai and chain `raiders::think → rival::think` there. This fixes the unordered AI vs scripted arrival order (G2).
- `BookkeepingSet { Record, Advance }`, chained inside `SimSet::Bookkeeping`.
  - `advance_tick` goes in Advance.
  - Record holds `events::record_events → run::settle_outcome`.
  - Anything in Bookkeeping that reads SimTick goes in Record. sim_world's two systems stay as they are.
- **Run gate:** `configure_sets(FixedUpdate, SimSet::X.run_if(run_in_progress))` for all nine sets (`schedule.rs:65`).
  - `run_in_progress(state: Option<Res<RunState>>) = state.is_none_or(|s| s.outcome.is_none())`.
  - Outcomes are written as `pending` by serial steps. `settle_outcome` (Bookkeeping, after `record_events`) promotes `pending` to `outcome` and sets `ended_at = tick`.
  - The ending tick therefore completes normally (SimTick advances), and every later tick is a no-op, so the hash freezes.
- **Hour** = `ticks_per_day / 24` = **100 ticks**, not the UI clock face. Document this in DATA.md wherever a field ends in `_hours`.
  - `on_hour(tick, cal) = tick % cal.ticks_per_hour() == 0`; `on_day(tick, cal) = tick % cal.ticks_per_day == 0`. Tick 0 counts as both, so the start boon picked at tick 0 is folded into modifiers at tick 0.
- **New sets:**
  - `AgentSet::Standing` (SimSet::Needs, `.after(AgentSet::Needs)`): `apply_goodwill.run_if(ledger_nonempty) → drift_loyalty.run_if(on_day) → aggregate_loyalty.run_if(on_day)`, chained.
  - `MilitarySet::Progress` (SimSet::Needs, `.after(MilitarySet::Upkeep).after(AgentSet::Standing)`): **one exclusive driver** `progression::progress(world)` with `.run_if(on_hour)`. It calls these sub-steps in this fixed order (daily ones check `on_day` themselves; each opens a tracing span; run-only ones check `in_run`):
    1. `veterans::end_battles`
    2. `monuments::hold`
    3. `conquest::revolts` (daily)
    4. `wild::regrow` (daily, run)
    5. `drifters::draw` (daily, run)
    6. `requests::step` (run)
    7. `standing` invitations (run)
    8. `dominion::tally` (run)
    9. `champion::rank_ups` (run)
    10. `progression::rebuild_modifiers`
    11. `rival::clock` (run)
    12. `director::step` (run)
- Per-tick work stays where it is today:
  - Conquest hold, Give, Forage harvest and Garrison feeding run in `step_order` (Orders).
  - Champion death and succession run in `casualties` (Combat).
  - Veterans' `fought` flags are set in `resolve_strikes`.
  - Goodwill pushes go to the ledger.
- Never hang settlement-level steps off `run_orders` (gated by `any_with_component::<Captain>`).
- Update the ARCHITECTURE.md schedule table (rows 1, 6 and 9) in the same commit as each set change.

### D8. SimEvent flow (messages are not hashed)
- `#[derive(Message, Debug, Clone, Copy, PartialEq)] pub struct SimEvent { kind: SimEventKind, faction: FactionId, subject: u64 /*SpawnId or id*/, other: u64, at: IVec2, amount: i32, cause: Cause }`. Register it with `add_message::<SimEvent>()` next to `sim_core/src/lib.rs:72`.
- `SimEventKind` (append-only, `#[repr(u8)]`): RunStarted, OfferPosted, OfferChosen, MonumentTaken, MonumentLost, RequestPosted, RequestDone, RequestFailed, Conquest, Invited, Revolt, RankUp, TraitGained, VeteranRank, ChampionFell, Succession, RivalLevy, RivalRaid, RaidersArrived, Battle, DriftersJoined, RunEnded.
- `Cause` is a Copy enum (None, Player, Request{id}, Conquest{captain}, Invitation, Starvation, Plunder, Revolt, Raid{captain}, Hold, Boon{card}, Death{faction}, ...). Text is formatted later; the sim stores no strings.
- **Writers:**
  - Exclusive steps: `world.write_message(SimEvent{..})`.
  - Systems: `MessageWriter<SimEvent>`.
  - Nobody writes from within `BookkeepingSet::Record` except `RunState::finish` callers, which run earlier.
- **Single reader:** `sim_core::events::record_events` in `BookkeepingSet::Record`.
  1. It reads all of this tick's events and **sorts them by (kind as u8, faction, subject, other, at, amount)**, because writers in the same set run in executor order.
  2. Each event is stamped with SimTick.
  3. Every event is appended to `EventFeed` (exempt, ring of 256, monotonically increasing `seq`, which the UI reads by cursor).
  4. Chronicled kinds (`SimEventKind::chronicled()`) also go to `Chronicle` (hashed + saved: `entries: Vec<ChronicleEntry>`, `digest: u64` running FNV, `last_notable: u64`). Its StateHash writes `len + digest`, so the cost is O(1).
- The director reads `Chronicle.last_notable`.
- Tests assert on `Chronicle` or `EventFeed`, **never** on `MessageReader` (messages live 2 ticks).
- MilitaryLog stays (`posture.rs:92-96` greps "plundered"). In step 9 it gains a `seq` counter so the UI feed can merge it.

### D9. RNG keys (new; one per draw; never reuse existing ones)

| Key | Tick arg | Where |
| --- | --- | --- |
| `sim_world::gen::sizes` | 0 | size tier draws, settlements in index order |
| `sim_world::gen::monuments` `^ region` | 0 | monument kinds and placement jitter |
| `sim_world::gen::monument_names` | 0 | monument names (never the settlement `names` stream) |
| `sim_military::progression::start` | 0 | start region, terrain kind, tile |
| `sim_military::progression::band` | 0 | band template |
| `sim_military::progression::modifiers` | 0 | run modifiers |
| `sim_military::progression::boons` `^ offer_id` | posted tick | boon offers |
| `sim_military::progression::traits` `^ person` | tick | champion start trait and trait offers |
| `sim_military::monuments::guards` `^ monument` | 0 | guard make-up |
| `sim_agents::yield` `^ villager id` | tick | stochastic rounding of yields |
| `sim_military::wild` `^ captain` | tick | forage harvest rounding |
| `sim_military::requests` `^ settlement` | tick | request posting and rewards |
| `sim_military::requests::camp` `^ settlement` | tick | ClearCamp request rewards (step 4b) |
| `sim_military::drifters` `^ captain` | day start tick | drifter counts and picks |
| `sim_military::rival` | tick | rival levy and raids |
| `sim_military::director` | tick | director choices and spawn spots |
| `sim_headless::bot` | tick | bot tie-breaks |

Keys never include RunConfig. Difficulty selects **data**, not streams.

### D10. Offers
- Offers are posted by sim_military with `Offers::post(faction, kind, choices, tick, expires) -> id`, which emits `OfferPosted`. They are taken with `Offers::take(id)`.
- `choices` are indices into the deck in progression.ron for that kind. For an Invitation they are `[0 = accept, 1 = decline]`.
- An unanswered offer just waits. Invitations expire (`expires`), with a cooldown per settlement in RunProgress.
- The start boon offer is posted by `setup_run` before tick 0. Picks are always recorded player commands, from the UI, bot, tests or CommandScript.

### D11. FactionModifiers semantics
- `Stat` (append-only, serde): Morale, Defence, Attack, FoodYield, MarchSpeed, SmithSpeed, TradeValue, LoyaltyDrift, ThreatRadius (flat tiles), CaptainSlots (flat), VeteranRate, Desertion, ForageYield, RecruitChance.
- `FactionModifiers { by_faction: Vec<[f32; Stat::COUNT]>, world: [f32; Stat::COUNT], breakdown: Vec<(FactionId, Stat, Source, f32)> }`. It resizes like `ArmyStats::of`, and its hand-written StateHash covers all three fields.
  - `value(f, s)` = `by_faction[f][s] + world[s]`; `mult(f, s)` = `1.0 + value(f, s)`.
  - The `world` row holds run modifiers that apply to everyone (lean year −0.15 FoodYield).
- It is rebuilt only in `progression::rebuild_modifiers` (hourly), from:
  - held monuments, with per-kind caps and set bonuses
  - boons
  - the champion's traits
  - run modifiers
- Consumers read only `mult`/`value`: never write the resource, and never mutate `Economy.recipes` or `Army.data` at runtime (G9).

### D12. Step-order adjustments (resolve plan conflicts)
- **Monument placement** (worldgen plus the `Monument` entity) moves into **step 1**, because run_start needs "2–3 monuments within a day's march". Holder, guards, holding and bonuses stay in step 4.
- **Events infrastructure** (SimEvent, record_events, Chronicle, EventFeed) lands in **step 1**, so steps 2–7 emit events as they go. Step 9 adds text, battle reports, grouping and the chronicle test.
- **A bot skeleton** (`sim_headless::bot`) lands in **step 7**. The director test and the step 9 chronicle test need "bot runs". Step 11 makes it play to win.
- **Fix the starvation-death leak in step 3** (G4) before the wanderers scenario relies on starvation.
- **Fix the setup_battle player-message replay bug in step 1** (G1).
- Captain slots formula (M4_PLAN conflict): `slots = min(cap 8, max(first_village_slots if held_settlements ≥ 1, floor(held_pop / per_captain)) + boon_slots)`. Refuse when `captains_of_issuer − 1 ≥ slots` (the champion's own army doesn't count). The `dominion` test asserts this formula, with `first_village_slots: 1` in progression.ron.

### D13. Docs per step (CLAUDE.md DoD)
Each commit updates, in the same commit:
- DATA.md for every new RON field (otherwise docs.rs fails)
- ARCHITECTURE.md's schedule table and crate table
- TESTING.md rows for each new test
- GLOSSARY.md for new terms
- HOWTO.md recipes, naming only existing paths
- `///` on every new pub item in sim_*

It also adds the ADR from D3, references requirement IDs, and re-runs fmt, clippy, tests and the determinism suite.

---

## 3. Steps

### Step 1. Run foundations (GP-21, GP-27) — test `run_start`
**sim_core**
- `run.rs`: RunConfig, Difficulty, RunState, RunOutcome, `Cause` (or import it from events), Offers/Offer/OfferKind, `run_in_progress`, `in_run`, and `settle_outcome` (system). StateHash for each, with an explicit tag byte for every Option and enum.
- `events.rs`: SimEvent, SimEventKind, Cause, Chronicle, ChronicleEntry, EventFeed, and `record_events` (span `sim_core::record_events`).
- `modifiers.rs`: Stat, FactionModifiers. Empty until step 4, but registered now.
- `components.rs`: the FactionId constants. Update the FactionId doc (`:39`).
- `command.rs`:
  - Append `ChooseOffer` and `AbandonRun` to CommandKind (`:111`).
  - Add `SimInputSet::Ai` (`:136`).
- `lib.rs`:
  - `SimCorePlugin { seed, run }`; register the resources (hashed: RunState, Offers, Chronicle, FactionModifiers; exempt: EventFeed "UI feed, derived from SimEvents").
  - `add_message::<SimEvent>()`.
  - Chain `(Inject, Ai, Consume)`.
  - Add `BookkeepingSet` with `advance_tick` in Advance.
- `schedule.rs`: `BookkeepingSet`, the `run_if(run_in_progress)` gate on all SimSets, `on_hour`/`on_day`.
- `calendar.rs`: `ticks_per_hour`, `hours`, `days`.
- `replay.rs`: the `run` field; bump `REPLAY_FORMAT` to 5; update the `sample()` fixture (`:100-121`).
- Unit tests:
  - `step.rs`: `a_finished_run_stops_the_clock` (bare App + SimCorePlugin; set `pending`, step, then assert SimTick and hash frozen).
  - `events.rs`: `events_are_recorded_in_sorted_order`.
  - `calendar.rs`: `an_hour_is_a_twentyfourth_of_a_day`.
  - `replay.rs`: `ron_round_trip` with `run: Some(..)`.

**sim_world**
- `settings.rs`:
  - WorldPreset, WorldGenPresets, `WorldGenSettings::preset(p)` (parses DEFAULT_WORLDGEN_RON).
  - `MonumentSettings { per_region, min_settlement_distance, min_spacing, kinds: Vec<MonumentKindRule{kind, weight, site: SiteRule}> }`.
  - `SizeSettings { tiers: Vec<SizeTierRule{tier, weight, min, max}>, seat_min }`.
  - Extend `validate()` (weights > 0, min ≤ max, 0 < per_region).
  - Keep `shipped_data_is_valid` asserting developed; add `run_preset_is_valid` (size 1024, regions 4).
- `assets/data/worldgen.ron`: the two-preset wrapper.
  - run: size 1024, regions 4, settlements target 25 / min ~20 / max ~30, monuments `per_region: 6`, size tiers.
  - Size arithmetic: hamlet 20–60, village 60–200, town 200–400, seat ≥ 450. To reach ~4–5k people with 25 settlements, the mean must be ~170. Tune weights (e.g. 0.35/0.45/0.20) and say so in DATA.md.
  - Note `min_spacing_tiles_at_2048: 110` scales to 55 at 1024. Keep `min ≤ ~20` (settle relaxes spacing).
- `worldgen/sizes.rs`:
  - `draw_sizes(seed, &SizeSettings, n) -> Vec<Option<SettlementSize>>` using stream `sim_world::gen::sizes`.
  - The seat is the largest drawn (ties → lowest index); raise it to `max(seat_min, max+1)`.
  - Call it after regions and names (`worldgen/mod.rs:147`), and add a `lap("sizes")`.
- `monuments.rs` + `worldgen/monuments.rs`:
  - MonumentKind {StandingStones, RuinedFort, HolyWell, RoadMarker, AncientForge, MarketCross, Wayshrine, Watchtower} with `ALL` and `name()`; `Monument {kind, region, name: NameId}` (StateHash: u8, u8, u32); `MonumentSite {kind, center, region, name: String}` (PartialEq).
  - Placement mirrors `settle::place` (`worldgen/settle.rs:46`). Per region in id order, draw kinds by weight, then pick per kind the best-scoring candidate. Sort with `total_cmp`, then by tile index. Respect the distance to every settlement centre and the spacing between monuments. Never on Water, Road or Mountain. `map.region(tile) == region`.
  - Site rules:
    - HolyWell: water_dist 2..=6
    - Watchtower and RuinedFort: Hills
    - AncientForge: Hills near Mountain
    - RoadMarker and MarketCross: 1–3 tiles from a Road
    - Wayshrine: forest edge or road
    - StandingStones: Grass or Hills
  - `water_dist`, `slope` and the tile-region array are still locals at `mod.rs:145-159`. Run the stage before `WorldMap::from_parts` consumes them. **Paint no terrain.**
- `GeneratedWorld` (`worldgen/mod.rs:40`) gains `monuments: Vec<MonumentSite>`; `SettlementSite` (`:23`) gains `size: Option<SettlementSize>`. `install_world` destructures exhaustively (`lib.rs:145`), so update it:
  - Spawn settlements (inserting `SettlementSize` when Some).
  - **Then** spawn monuments: allocate, push the name, `(id, Monument, Position(center))`, without Mobile.
- `WorldPlugin::build` (`lib.rs:113`): `register_hashed::<Monument>()` and `register_hashed::<SettlementSize>()`.
- `map.rs`: `region_count()` (computed in `from_parts`: max non-NO_REGION + 1) and `is_region_edge(p)` (a 4-neighbour is Water or in another region).

**sim_agents**
- `populate` (`population.rs:115-169`): if any settlement has `SettlementSize`, then:
  - `chosen` = **all** settlements sorted by SpawnId, and `sizes` = their `people`.
  - Skip `choose_region`, the farmland weights, `min_per_settlement` and `apportion`.
  - Otherwise leave today's path byte-identical.
  - Don't touch the per-settlement `sim_agents::populate` stream (`:180`).
- `PopulationReport.region` → `regions: Vec<RegionId>` (exempt; update readers in sim_headless stats if any).
- `AgentsPlugin.population` is ignored in size mode.

**sim_military**
- `assets/data/progression.ron` + `progression/mod.rs` `Progression` (hashed resource; `from_ron`/`default_data`; inserted in `MilitaryPlugin::build`, `lib.rs:74`). Step 1 fields:
  - `start: (monument_radius (tiles, ≈ half a day's march; a full day is ~312 tiles at 0.13 t/tick), min_monuments: 2, max_monuments: 3, min_settlement_distance, terrain_weights: [(Coast, w), (ForestEdge, w), (Hills, w)])`
  - `bands: [(name, groups: [(count, unit: (health, attack, defense, morale), armed, tools)], food_days)]`
  - `champion_traits` (start pool)
  - `traits: [(id, name, phrase, effects: [(stat, value)])]`
  - `boons: [(id, name, text, when: Start|Dominion, weight, effects: [...])]` with Effect {Stat(stat, value), FoodDays(f32), ArmBand, Goodwill(f32), CaptainSlots(u8), ...}
  - `modifiers: [(id, name, weight, effects)]` (lean year, bumper harvest, rats −10% stores, restless roads, sellswords, stronger rival, weaker rival); `modifiers_per_run: (1, 2)`
- `progression/start.rs`:
  - `RunWorld { map, settlements: Vec<SiteInfo{id: u64 (or index), center, region, people}>, monuments: Vec<SiteInfo> }`, built by `from_generated(&GeneratedWorld)` or `from_world(&mut World)`. Both use the same order: index order equals SpawnId order.
  - Pure `draw_start(seed: SimSeed, data: &Progression, w: &RunWorld) -> Option<StartDraws { seat, rival_region, start_region, start: IVec2, terrain: StartTerrain, band: u16, champion_trait: u16, modifiers: Vec<u16>, boon_choices: [u16; 3] }>`.
  - Algorithm:
    1. The seat is the largest `people` (ties → lowest id); its region is the rival region.
    2. Region centroids = mean of settlement centres (Vec by RegionId).
    3. Start region = argmax centroid distance from the seat, excluding the rival region (ties → lowest id).
    4. Draw a terrain kind, then pick candidates: passable, not Road or Farmland, `is_region_edge`, `map.region(p) == start_region`, `dot(p − start_c, seat − start_c) < 0`, ≥ min_settlement_distance from every centre, with `min..=max` monuments within monument_radius.
    5. Score = distance from seat + seeded jitter; sort with `total_cmp`, then by tile index.
    6. If the kind has no candidate, fall back to any kind, then relax the monument count.
  - The boon choices are three **distinct** Start boons, drawn without replacement by weight.
- `progression/mod.rs`:
  - `RunProgress` (hashed; fields from D1).
  - `setup_run(world) -> Option<RunStart{champion, start}>`:
    1. Build RunWorld from the world, call `draw_start`, insert RunProgress.
    2. `Factions.set_seat(PLAYER, start)` **before** spawning, so `spawn_army` doesn't seat the faction at the snap tile (`spawn.rs:149`).
    3. Spawn the band with `spawn_army`; the captain is the champion. Insert `Champion` and `Traits(vec![trait])`.
    4. Apply one-off modifiers: rats takes `−10%` from every storehouse, in SpawnId order, via `Stockpile::take`.
    5. Post the StartBoon offer and emit RunStarted.
- `ArmySpec` (`spawn.rs:23`) gains `soldier: Option<UnitStats>` and `tools: bool`, staying **Copy**. For mixed groups, spawn with the first group's stats, then overwrite CombatStats and Equipment of followers per group in SpawnId order (a setup edit before tick 0).
  - Note that soldier_upkeep's morale *base* still comes from `army.data.soldier.morale` (`upkeep.rs:128-133`).
- `progression/offers.rs`:
  - `choose(world, issuer, offer, choice) -> Result<(), &'static str>`.
  - Start boon effects now: FoodDays adds grain to the champion's baggage; ArmBand sets `Equipment.weapon`. Goodwill is recorded and pushed to the ledger in step 3.
  - `abandon(world, issuer)`.
- `orders.rs::apply` (`:127`): the two new match arms.
- `spawn.rs::setup_battle` (`:252-259`): write `SimCommand::ai`, not `::player` (G1).
- `raiders::think` → `.in_set(SimInputSet::Ai)` (`lib.rs:115`).
- army.ron: append factions 3 Rival and 4 Beasts (D2).

**sim_headless**
- `Scenario::Run`, `HeadlessConfig::run`, `SimPlugins::run`, `set_up_scenario`.
- `record_replay` writes `run`; `verify_replay` reads it, and panics on unknown scenario names.
- main.rs: `--run`, `--difficulty <steady|chaotic|harsh>` (Args `:35`, parse `:51-99`, USAGE `:17-33`). With `--run`, the default `--commands` source picks the start boon at tick 0 (e.g. a ChooseOffer of choice 0) plus sample_orders.
- `print_world_summary` (`main.rs:110`) adds the regions, monuments by kind, size tiers, seat and start. `write_map_ppm` (`:148`) marks monuments, start and seat.
- `sample_orders`: ChooseOffer (mostly invalid ids) and AbandonRun.

**game_app / game_ui (minimal in step 1)**
- `Start::Run` is **not** the default until step 10. Add `--run` now, calling `set_up_scenario`.
- `PLAYER` → `FactionId::PLAYER`.

**Tests**
- `tests/tests/run_start.rs`:
  - `twenty_seeds_draw_valid_varied_starts` (world-level, sequential, seeds 1..=20). Check:
    - `map.region_count() == 4` and 4 distinct settlement regions
    - every region has 4–8 monuments, none on Water, Road or Mountain, all ≥ min distance from centres
    - 2–3 monuments within `monument_radius` of the start
    - `find_path(start→seat)` is Some (`sim_world/src/path.rs:38`, villagers travel costs)
    - three distinct boon choices
    - BTreeSets of terrain, band and modifier-set each have len ≥ 3
  - `a_run_starts_identically_from_its_seed` (2 apps of one seed):
    - hashes equal at setup and after 200 ticks
    - every settlement has SettlementEconomy with population > 0
    - one pending StartBoon
    - the champion exists
  - `choosing_the_start_boon_applies_once`: second ChooseOffer refused, `rejected_commands` +1.
  - `abandoning_ends_the_run_and_stops_the_clock`.
- `determinism.rs`:
  - `run_state_hash_covers_every_sim_component`: run app, choose boon, sample_orders, 1,500 ticks, `unhashed_state` empty. Extend it in every later step.
  - `a_run_replays`: `record_replay(HeadlessConfig::run(..))`, `replay.run == Some(cfg)`, verify Multi and Single.
- `worldgen.rs`:
  - `run_preset_meets_its_criteria` (seeds [42, 7, 1234, 2026]).
  - Parametrise `check_settlements` by `s.settlements.min..=max` instead of the literal `(40..=80)` (`:83`, `:178`).
  - Add 1-vs-4-thread equality including monuments and sizes.

**Docs**
- DATA.md: worldgen presets, monuments and sizes; the new `## progression.ron` section; army.ron factions.
- GLOSSARY: Run, RunConfig, Preset, Offer, Boon, Run modifier, Monument, Start spot, Rival seat, Size tier, SimEvent, Chronicle.
- ARCHITECTURE: Ai set, Bookkeeping subsets, run gate.
- TESTING; ADR 0013.

**Risks**
- 1024 worldgen at opt 3 is ~¼ s; populating ~4–5k villagers adds seconds per app. Run the 20 seeds world-level only.
- `build_settlement` clones the whole WorldMap and Economy per settlement (`sim_economy/src/layout.rs:296-297`). That's fine at 1024 × 25; use `resource_scope` if it shows up in the profile.
- Shared `Occupied`: lower-SpawnId settlements win contested sites. The overflow households sleep in the storehouse (`population.rs:280-286`).
- Region 0 is always the central region (farthest-point sampling), so region sizes are uneven, which matters for dominion percentages.

### Step 2. Ownership, conquest, garrison, loyalty, revolt (GP-05, GP-06, SIM-24) — scenarios `conquest` (+ `invitation` after step 3)
**sim_agents**
- `standing.rs`:
  - `Owner {faction, since}` and `Goodwill` (step 3 fills it), inserted with SettlementEconomy (`population.rs:414-423`).
  - `pub fn set_owner(world, s: Entity, to: FactionId, seed_loyalty: Option<f32>)`. It sets Owner, then queries `(Entity, &SpawnId, &Villager, &mut Allegiance, &mut Loyalty), Without<Enlisted>`, filtered on `v.settlement == s` and sorted by SpawnId. It sets `Allegiance(to)` and `Loyalty = seed` (default: `(goodwill(to) + 1) / 2`) and clears `Threat.provoked_by` and `provoked_until`.
  - Spawn villagers with `Allegiance(owner)` rather than the literal at `population.rs:337`. Only matters if the owner is known before populate (no).
- `loyalty.rs`:
  - `Loyalty(f32)` goes in the villager bundle's second tuple (`population.rs:332-342`).
  - `SettlementLoyalty` is inserted at `:414`.
  - `drift_loyalty` (daily, par_iter over `(&Villager, &mut Loyalty), Without<Enlisted>`, reading only settlement components and `FactionModifiers`). The target is a weighted sum of:
    - food security: `FoodSecurity.days` against `loyalty.food_days`
    - goodwill toward the owner: `(g + 1) / 2`
    - time under the owner: `(tick − Owner.since)` against `settle_days`
  - Owner 0 uses `loyalty.independent_target`. The rate is × `mult(owner, LoyaltyDrift)`.
  - `aggregate_loyalty` (daily, serial by SpawnId) computes mean and min, and sets or clears `below_since` against `loyalty.revolt_threshold`.
  - Register both hashed.
- villagers.ron `loyalty: (food_days, w_food, w_goodwill, w_time, settle_days, drift_per_day, independent_target, revolt_threshold)` (+ DATA.md; Villagers hashes data automatically, `data.rs:336`).
- `AgentSet::Standing` (D7).
- Optional SIM-13: add `&Loyalty` to `ai::decide`'s query (12 items now) and to `Me` (`ai.rs:82`). Scale Fight courage by loyalty, or by (1 − loyalty) when `provoked_by == owner`.

**sim_military**
- `conquest.rs`:
  - `pub fn transfer(world, s, to, cause)`:
    1. Calls `standing::set_owner`.
    2. Emits Conquest, Invited or Revolt.
    3. Win check: if `s` is `RunProgress.rival_seat && to == PLAYER`, call `RunState::finish(Won)`, then transfer the rival's other lands to 0 (step 7 fills that in).
    4. If the player held no land, `Factions.set_seat(to, center)`.
    5. Bumps a new `FactionStats.conquests`, which must be added to the manual StateHash at `components.rs:295`.
  - `pub fn militia_or_foes_near(world, at, faction, r) -> bool`: militia `against == faction` not routed, **or** soldiers of a faction at war with `faction` (FighterGrid).
- Attack, `step_order` (`orders.rs:723-738`):
  - When step 1 ends because the militia broke (`fought > 300 && !militia_standing`), set `order.step = 2; since = tick`, and keep the "broke the defenders" log line. **Stop provoking** in step 2.
  - In step 2, require the captain within `conquest.hold_radius` of the centre and `!militia_or_foes_near`. Reset `since` while that fails. Transfer when `tick − since ≥ cal.hours(conquest.hold_hours)`. Fail (log, done) after `conquest.timeout_hours`.
  - Move the literals 300, `tpd` and 35 (`orders.rs:633`, `:732`) into `army.ron conquest: (break_ticks, give_up_days, militia_radius, hold_radius, hold_hours, timeout_hours)`.
  - Attack on your own settlement is refused in `validate_spec`.
- Garrison (`OrderSpec::Garrison`, tag 8 in `hash_spec`, plus describe and validate):
  - In step_order, goal = centre; it never pops.
  - **Owned-village supply**: in `forage` (`orders.rs:776`) and `gather_food` (`:384`), when `Owner == c.faction`, take `spare_food(s, garrison.keep_days)` via `move_food` with no provoke, no payment and no goodwill change, and count it in a new `FactionStats.supplied`. Relax the `wants` gate for owned settlements: top up to `gather_target_days`, at most daily.
  - Aggressive posture never plunders its own village.
- `threat.rs` (`:21-43`): skip armies whose faction equals the settlement's Owner (query `Option<&Owner>`). Otherwise your own garrison keeps your villagers hiding.
- `upkeep.rs:290`: a deserter gets `Allegiance(owner of v.settlement)`, not 0.
- `recruit` (`orders.rs:521-578`):
  - Owner == recruiter: + `loyalty_bonus × SettlementLoyalty.mean` (the army.ron comment "Loyalty joins these in M4").
  - Owner is another warlord faction (≠ 0, ≠ recruiter): recruit 0 and log it.
- `conquest::revolts` (daily, in the Progress driver, SpawnId order):
  - `owner ≠ 0 && below_since ≤ tick − tpd`. Garrison strength = owner soldiers within `garrison.radius` (FighterGrid, `side.against == None`). Would-be militia = adult, alive, fed (`hunger < needs.starving`), Civilian villagers of s with morale ≥ `fight.break_morale` (the `ai.rs:347-358` filters).
  - **No garrison** → `transfer(s, 0, Cause::Revolt)` at once. Starving villagers can't fight, so a starved, ungarrisoned conquest revolts without combat.
  - Otherwise, if the garrison is weaker, set `Threat.provoked_by = owner` with `provoked_until = tick + cal.hours(revolt_hours)` and record `SettlementLoyalty.revolt_since`.
  - Next day: if the militia is still standing, transfer to 0. If it broke, clear it and apply a loyalty penalty.
  - `provoke` lasts half a day (`orders.rs:317-323`) and `update_threats` clears it (`threat.rs:23-25`), so the revolt must refresh it daily.
- `MilitarySet::Progress` + the driver skeleton `progression::progress` (D7). It runs in all presets, because conquest and revolts are general.
- game_ui `army.rs` MenuAction gains Garrison (`:93-112`, `menu_buttons` `:351`). `order_line` has a `_` fallback (`:552`).

**sim_headless**: sample_orders gains Garrison.

**Tests**
- `tests/tests/conquest.rs`, run preset (or developed Skirmish if run-preset timing is bad): pick the nearest small settlement, `Attack`, poll every 100 ticks until `owner_of == PLAYER`. Assert the Conquest event in Chronicle and that the villagers' Allegiance is 1. Then three branches from a shared deterministic prefix helper `fn conquered(seed) -> (App, Entity)`; switch to save/load after step 8:
  - passive garrison + fed: mean loyalty rises over 3 days
  - aggressive garrison + plunder (`GatherFood` on an owned village with a hostile-posture army that is not the owner, or seize via a second faction): loyalty falls
  - starved: drain the storehouse plus `cut_off_farmers`, no garrison; Revolt within 2–3 days and the owner returns to 0
- Re-run `orders.rs::attacking_a_village_raises_its_militia`. It asserts the Attack order ends within 4,000 ticks; step 2 must end it (transfer or timeout).
- Re-run posture.rs, starving_army.rs, famine.rs and village_day.rs (D6).

**Docs**: army.ron conquest and garrison, villagers.ron loyalty, GLOSSARY (Owner, Garrison, Loyalty, Revolt, Conquest), HOWTO "Add a captain order" for Garrison, ADR 0014.

**Risks**
- `Threat.provoked_by` is one slot: a revolt and a raid overwrite each other (last writer wins).
- `perform` resets the stance to Civilian unless the activity is Fight (`act.rs:77-81`), so revolt state must live on the settlement.
- `soldier_upkeep`'s casualty shock now fires for player soldiers near dying militia of player-owned villages (Allegiance 1).

### Step 3. Band life: foraging, goodwill, requests, invitations, drifters (GP-22, SIM-38) — scenario `wanderers`
**Fix first (G4):** `find_fallen` (`combat.rs:34`) only scans `grid.fighters`, and `build_index` skips dead fighters (`index.rs:483`). A soldier or captain who starves in `soldier_upkeep` (Needs, `upkeep.rs:118-121`) is therefore **never despawned**, so a starving champion would never end the run. Also scan `Query<(Entity, &SpawnId, &CombatStats), With<Enlisted>>` for `!alive`, merge, sort by SpawnId and dedup. Re-run starving_army; its assertions ("starvation only after desertion", ">100 deserted") may shift.

**sim_agents::standing**
- `Goodwill` sorted Vec with helpers `get(f)` and `add(f, d)` (clamp −1..1).
- `GoodwillLedger(Vec<GoodwillChange{settlement: u64, faction, delta, cause}>)` (hashed).
- `apply_goodwill` (AgentSet::Standing, run_if non-empty):
  1. Sort by (settlement, faction, cause as u8, delta bits).
  2. Accumulate direct deltas plus spread deltas (`goodwill.spread_fraction` to settlements within `goodwill.spread_radius`, by centre distance) into a Vec, iterating settlements in SpawnId order.
  3. Apply them, clamp, then clear the ledger.
- villagers.ron `goodwill: (spread_radius, spread_fraction)`.

**sim_military**
- Goodwill pushes (deltas from progression.ron `goodwill_deeds`):
  - `barter`/Trade (`orders.rs:334`, `:747-751`) and passive equip: +
  - Neutral seize (`:408-424`): −
  - Aggressive plunder (`:425-433`) and `provoke`: −
  - `casualties` militia death (`combat.rs:135-140`, keyed by `Villager.settlement` and `against`): −
  - request completion: +
  - the "Known" start boon: +
- `OrderSpec::Give {settlement}` (tag 10):
  - step_order settlement arm (`orders.rs:710-756`); within reach, `move_food(world, e, storehouse(s), baggage_food − give_keep_days × daily_need)` (`:232`, `:284`).
  - Record the delivered value (the **returned** value, since `move_food` floors and caps) on the open BringFood request and in a new `FactionStats.donated`. `given` means village→army; don't reuse it.
- `OrderSpec::Forage {radius}` (tag 9) in `wild.rs`:
  - `WildFood { cell: 16, per_side, level: Vec<f32> }` (hashed). Capacity per patch = count of Grass and Forest tiles × `forage.per_tile`.
  - In step_order the band holds, goal None. Every `forage.every_ticks`, using `order.since`, harvest from patches within the radius in index order, deplete, and add the least-nourishing food good to the baggage with `Stockpile::add`. This is goods from nothing, allowed outside a stockpile-to-stockpile move. Use stochastic rounding (`sim_military::wild ^ captain`) and `× mult(f, ForageYield)`. Stop when `baggage_room == 0`.
  - `wild::regrow` is daily.
  - Rates go in progression.ron `forage:`.
  - `orders::forage` still runs after step_order. A hungry band within `upkeep.forage_radius` (25) of a village will also auto-gather from it, so the "stand on rock" test must camp > 25 tiles from every centre.
- `requests.rs`:
  - `RequestBoard { next_id, open: Vec<Request{id, settlement: u64, kind: DriveOff{since}|BringFood{need, delivered}|ClearCamp{monument}, posted, deadline, reward: Reward{food, goods: Vec<(GoodId,u32)>, volunteers, goodwill}}> }` (hashed). Hourly in the driver, settlements in SpawnId order:
    - **post** BringFood when `FoodSecurity.days < request_food_days`, and DriveOff when an at-war army (ArmyIndex, faction ≠ owner) is within threat radius. ClearCamp is added in step 4.
    - **complete**: BringFood when `delivered ≥ need`; DriveOff when there are no hostile armies within radius for an hour. Pay the reward from the real storehouse via `move_food`/`move_good`, to the nearest player captain within radius. Volunteers go through the shared enlist helper.
    - **expire** at the deadline.
  - Emit RequestPosted, RequestDone and RequestFailed.
- Invitations: hourly. When goodwill(f) ≥ `invite_goodwill`, a captain of f is within `invite_radius`, and the owner is 0 or weak, post `Offer{Invitation{settlement}}`, with a cooldown. Accepting calls `conquest::transfer(s, f, Cause::Invitation)` with loyalty seeded from goodwill.
- `drifters.rs`:
  - Daily, captains of PLAYER in SpawnId order. For settlements within `drifters.radius` with goodwill ≥ threshold, draw a count (`sim_military::drifters ^ captain`, day start tick) and pick candidates with recruit's filter (`orders.rs:542-556`).
  - Factor recruit's bookkeeping (`:565-575`: baggage capacity, `se.population −= n`, `stats.recruited`) into `pub fn enlist_into(world, villager, captain)`, used by recruit, drifters, request volunteers and the rival levy.
  - Survivors of a settlement taken by raiders: a later polish, optional.
- `Factions`: `spawn_army` sets a landless band's seat to its spawn tile (`spawn.rs:149-154`). That's fine, because setup_run sets the start first.

**Tests**
- `tests/tests/wanderers.rs`: band of 10, 2 days of food.
  - (a) Forage near the start, then answer a request (Give for BringFood, or MoveTo onto raiders for DriveOff). Fed at day 5 and followers > 10.
  - (b) A band ordered to stand on painted rock (`set_terrain` Mountain/Hills before tick 0, as `army_march.rs:50-60` does) more than 25 tiles from every centre. `ArmyStats.first_desertion ≤ first_desertion_within_days` (pattern from `starving_army.rs:72-99`).
- Unit tests:
  - `standing.rs`: `spread_is_order_independent`
  - `wild.rs`: `patches_deplete_and_regrow`

**Docs**: progression.ron `forage`, `requests`, `goodwill_deeds`, `invite_*` and `drifters`; villagers.ron goodwill; GLOSSARY (Goodwill, Request, Invitation, Drifter, Wild food); HOWTO for the orders.

**Risks**
- `FoodSecurity.days` dips during the work day and excludes food at farms (`food.rs:26-42`). Sample it hourly with hysteresis (post only when below for 2 hours) or `request_food_days` will flap.
- `spare_food` counts the storehouse only, while `FoodSecurity` counts storehouse plus homes. Pick one per rule and document it.
- `Stockpile.add` silently caps.

### Step 4. Monuments and FactionModifiers (GP-23) — scenario `monuments`
**sim_military::monuments**
- In `setup_run`, insert `Holder{faction: guard faction or 0}` on every Monument. Spawn guards for some monuments (progression.ron `monuments.guards: [(kind: RaiderCamp|Beasts, chance, base, per_tile_from_start)]`).
  - Use `spawn_army` (FactionId::RAIDERS or BEASTS, **no** `raider: true`, posture Neutral) + `Guard{monument}` + `NoUpkeep` on each soldier.
  - Strength scales with distance from the start.
- `NoUpkeep`:
  - `soldier_upkeep` (`upkeep.rs:113-121`) skips hunger for it (add `Has<NoUpkeep>`).
  - `eat` skips it.
  - `orders::forage` (`:776`) skips captains with `Guard`.
  - `raiders::think` prey excludes `Guard` (`raiders.rs:27`).
- A captain with no orders holds its tile and fights anything at war within `engage_radius` (soldiers::decide), which makes it a stationary camp.
- `monuments::hold` (hourly, monuments in SpawnId order):
  - Count soldiers by faction within `hold_radius` via `FighterGrid::for_each_within` (`side.against == None`, not routed), into a small Vec sorted by FactionId.
  - If exactly one faction is present and it isn't the holder: track `contender` and `since`, and transfer after `cal.hours(hold_hours)` (emit MonumentTaken / MonumentLost).
  - Drift: if the holder ≠ 0, no holder men have been near for `drift_days`, and the monument is farther than `drift_distance` from every settlement the holder owns, set `Holder` to 0.
  - Place monuments farther than `threat.radius` (45) from centres (worldgen `min_settlement_distance`), or a guard camp keeps villages' Threat up.
- ClearCamp requests (step 3 board): post at settlements within `requests.camp_radius` of a guarded monument; complete when the Guard captain is gone.

**progression::rebuild_modifiers** (hourly, last state step). For each faction:
- Σ per kind `min(count × bonus, cap)`
- plus a set bonus when holding every monument of a kind in a region
- plus boons (`RunProgress.boons`)
- plus champion traits
- plus the run modifiers' `world` row
- Write `breakdown`.

**Consumers** (exact no-ops at 1.0):
- Morale target: `soldier_upkeep` (`upkeep.rs:128-134`), `base × mult(f, Morale)`.
- Defence: `resolve_strikes` (`combat.rs:103`). The victim's faction comes from the query's Allegiance (currently `_` at `:95`).
- Attack (traits): `combat.rs:89-93`.
- FoodYield: `act::perform` (`sim_agents/src/act.rs:216-218`).
  - Add params `owners: Query<&Owner>` and `mods: Res<FactionModifiers>`.
  - Apply only to recipes with `modifier: Some(FoodYield)` (economy.ron).
  - `q × mult` with stochastic rounding (`sim_agents::yield ^ id`, draw only when the fraction > 0).
  - **Farming outputs 8 = carry 8, so `inv.add` silently drops extra** (`act.rs:217`, `goods.rs:138`). Use `add_over_capacity` (`goods.rs:156`) for outputs, and update `Brain::work_state` out_units (`ai.rs:395`).
- SmithSpeed: `act.rs:196`, `until_tick = tick + (d as f32 / mult).round().max(1.0) as u64` for `modifier: Some(SmithSpeed)`.
- MarchSpeed: `move_captains` (`soldiers.rs:252`) **and** `move_soldiers`' `speed_at` (`soldiers.rs:347`). Scaling only the captain strings the band out. The soldier query has no Allegiance, so take the faction from `index.get(follower.captain)`.
- TradeValue: `barter` (`orders.rs:350-353` offer sum **and** `:362` payment `v`; factor out one helper). Decide whether equip's passive price (`:465`) divides by it.
- ThreatRadius: `update_threats` per-settlement radius = `threat.radius + value(owner, ThreatRadius)` (`threat.rs:20`).
- LoyaltyDrift: `drift_loyalty` (step 2).

**Tests** `tests/tests/monuments.rs`:
- Take an unguarded monument by holding it for `hold_hours`.
- Take a guarded one by beating the guard (spawn a big player army next to it before tick 0).
- `FactionModifiers.value(PLAYER, Morale)` equals Σ capped (set the hold count above the cap via direct Holder edits before tick 0 to test the cap).
- An enemy (spawned faction 2 army, held longer) takes it back, and the bonus drops at the next hourly rebuild.
- Unit test `caps_and_set_bonus` in `progression/mod.rs`.

**Docs**: progression.ron monuments, bonuses, caps, guards and hold; economy.ron `modifier`; GLOSSARY (Holder, Guard, FactionModifiers, Stat, Set bonus); HOWTO "Add a monument type", "Add a modifier"; ADR 0015.

### Step 5. Dominion boons and captain slots (GP-24, GP-25) — test `dominion`
- `progression/dominion.rs` `tally` (hourly):
  - Per region (Vec indexed by `RegionId`, size `map.region_count()`; skip NO_REGION), sum `SettlementEconomy.population` grouped by owner. Store it in `RunProgress.dominion: Vec<RegionTally{total, held_by_player, flags: u8}>`.
  - On first crossing 25% (bit 0) or 50% (bit 1), post a `DominionBoon` offer with 3 distinct Dominion boons drawn by weight. Weights are raised by holdings (farms count, smithy, the region's largest settlement, battles fought) per progression.ron `boons[].favour: [(Holding, w)]`.
  - Flags never reset, so there is no second offer after losing land.
- `captain_slots(world, f) -> Option<u32>` (D12 formula). It is Some only `in_run && f == PLAYER`; otherwise the old `max_captains` rule. Call it in `appoint` (`orders.rs:184-193`), keeping `max_captains` (9) as the hard cap. Refuse with Err("no captain slot").
- Also store `RunProgress.captain_slots` hourly for the UI.
- `succeed()` (`combat.rs:166`) creates captains without a slot check; that is correct.
- Boon effects: `CaptainSlots(n)`, `VeteranRate`, and `Stat` boons via rebuild_modifiers.

**Tests** `tests/tests/dominion.rs`:
- Transfer settlements with commands. Use a test-only path: conquer via Attack, or accept invitations prepared by setting goodwill before tick 0. Don't call `transfer` directly mid-run except before tick 0.
- Cross 25%: exactly one offer with 3 distinct choices; pick it and see the effect.
- Lose land (revolt or transfer) and re-cross: no new offer.
- Slots: formula, capped at 8; `AppointCaptain` beyond the slots bumps `rejected_commands` (`orders.rs:62` pattern).
- Keep `sample_orders` appointments working in skirmish: no run, so no slot rule.

**Docs**: progression.ron `dominion` (`thresholds`, `per_captain`, `cap`, `first_village_slots`), GLOSSARY (Dominion, Captain slot).

### Step 6. Champion, traits, veterans, succession (SIM-34/35/36) — test `champion`
- `veterans.rs`:
  - `Veteran {battles: u16, rank: u8, fought: bool}` in the spawn_army person bundle's second tuple (`spawn.rs:177-195`; 7 items now) and in `enlist` (`orders.rs:613-623`).
  - `resolve_strikes` (`combat.rs:58-65` query) adds `Option<&mut Veteran>` and sets `fought` on attacker and victim after the alive check (`:98`).
  - Per-army battle state goes in Captain fields `battle_since: Option<u64>, last_engaged: u64`. Add them to `Captain::new` (`components.rs:105`) **and** the StateHash (`:119-150`); `succeed()` then carries them.
  - `last_engaged` is updated from `ArmyInfo.fighting > 0` in run_orders (or from resolve_strikes).
  - `end_battles` (hourly): a battle with `tick − last_engaged ≥ cal.hours(1)` ends. Every living member with `fought` gets `battles += 1` (× VeteranRate boon), a rank from army.ron `veterans: (thresholds: [1,3,6], gains: [(attack, defense, morale, desertion)])`, and `fought` cleared. Emit Battle with losses from per-battle counters, and VeteranRank.
  - Gate it on `in_run` (D6): in skirmish, rank gains would move the M3 tests.
  - Rank effects: attack and defence in `resolve_strikes`; morale target and desertion chance in `soldier_upkeep` (`upkeep.rs:128-162`).
  - `apply_desertions` (`upkeep.rs:285-287`) removes Veteran with Enlisted, Follower and Equipment.
- `champion.rs`:
  - `ChampionRank {rank, points}` on the champion person. Points come from progression.ron `champion.points: (battle, monument, settlement, request)`, credited where those events fire.
  - Each rank posts `Offer{Trait{person}}` with 3 distinct traits (`sim_military::progression::traits ^ person`). The pick is pushed to `Traits`, and the effects arrive in the next rebuild.
  - **Succession in `casualties`** (`combat.rs:145`), *before* `succeed()`: if the dead entity has `Champion`, take candidates = the faction's **other** living captains (excluding the heir `succeed` is about to create), ordered by ChampionRank or Veteran rank, then SpawnId.
    - If `succession_used` is false and a candidate exists: move Champion, ChampionRank and half the Traits (floor, in pick order) to it, set `succession_used`, and emit Succession.
    - Otherwise call `RunState::finish(Lost, Cause::Death)`.
  - The Champion marker lives on the person entity, never in Captain, because `succeed` moves Captain.
  - Traits (Brave, Beloved, Feared, Forager, Quartermaster, Duelist, Shaken) map to `Stat` effects plus optional AI weights. Keep AI-weight effects to existing knobs (morale, desertion, forage yield).

**Tests** `tests/tests/champion.rs`:
- Award points (e.g. win a staged battle, take a monument): rank-up, offer, pick applies.
- Soldiers who fought rank up with the configured gains (spawn two armies adjacent before tick 0).
- Kill the champion (set health 0 before a tick, i.e. a setup edit, or a staged battle) with another captain alive → that captain is champion. Kill again → outcome Lost and the clock frozen.

**Docs**: army.ron `veterans`, progression.ron `champion` and `traits`, GLOSSARY (Champion, Rank, Trait, Veteran rank, Succession), HOWTO "Add a trait or boon".

### Step 7. The rival and the director (GP-28, GP-16) — scenario `rival`, director test, bot skeleton
- `rival.rs`:
  - In `setup_run`: `Factions.set_seat(RIVAL, seat_center)` **before** spawning. `conquest::transfer` the rival region's settlements to RIVAL (loyalty seeded high, `rival.start_loyalty`). Spawn the seat garrison with `spawn_army(RIVAL, no raider)` + standing `Garrison{seat}`.
  - `RivalState {seat: u64, seat_center, defence_bonus, next_levy, next_raid, raids: u32}` (hashed).
  - Clock: progression.ron `rival: [(difficulty: Steady, levy_every_days, levy_size, defence_steps: [(day, bonus)], raid_from_day, raid_every_days, raid_size)]`, with the row chosen by `RunConfig.difficulty`.
    - The **levy** is a serial step that enlists real villagers from rival lands into the seat garrison via `enlist_into`.
    - **Raids** split a band off the garrison with a `split_army(world, captain, n)` helper extracted from `appoint` (`orders.rs:195-219`), so raids weaken the seat.
  - `rival::think` (SimInputSet::Ai, sorted by SpawnId, every 300 ticks) sends `SimCommand::ai(RIVAL, ..)`: raid bands target player-owned settlements (GatherFood Aggressive, or Attack) and contest player monuments (MoveTo).
  - Seat defence: `resolve_strikes` adds `defence_bonus` to victims of RIVAL within `rival.seat_radius` of the seat (`Option<Res<RivalState>>`).
  - The win is already handled in `transfer`. On win, the rest of the rival's lands go to 0.
  - Run modifiers stronger/weaker rival scale `levy_size` and `defence_steps`.
- `director.rs`:
  - `DirectorData` (director.ron: `personalities: [(difficulty, weights: (threat, hunger, fights, quiet), quiet_limit_hours, first_event_within_hours, spacing_hours, raider_band: (min, max, per_day), auto_pause: [SimEventKind])]`) and `Director {tension, last_spawn, next_check}` (hashed).
  - Step (hourly): compute tension from hashed state (threat near player lands, player food days, recent Battle events, `tick − Chronicle.last_notable`).
  - If quiet beyond the limit or past the first-event deadline, spawn a raider band (`ArmySpec{raider: true, Aggressive, armed, RAIDERS}`) at a seeded far-edge tile (`sim_military::director`), or nudge a request.
  - Emit RaidersArrived.
  - Spawning is exclusive and happens inside the driver, so it is ordered.
  - Restless roads shortens the spacing.
  - Any director "target" must live in `Raider` state (add `target: Option<u64>` and update its StateHash): `raiders::think` re-issues `queue:false` orders every 300 ticks.
- `sim_headless::bot` (skeleton):
  - `Bot::act(&mut self, app: &mut App) -> Vec<SimCommand>`. It reads state between ticks with SpawnId-sorted queries and returns `SimCommand::player(PLAYER, ..)`. It thinks every 100 ticks.
  - `run_bot(config, max_ticks, checkpoint_every) -> BotRun {outcome, end_tick, log, checkpoints}` writes with `app.world_mut().write_message` and then calls `app.step(1)`.
  - Step-7 strategy: pick boon 0, forage when hungry, take the nearest monument, answer requests.
- `sim_headless::step_until_event(app, max) -> Option<u64>` (the fast-forward analogue).

**Tests**
- `tests/tests/rival.rs`: no player action, steps over days. The rival garrison grows per schedule, the first `RivalRaid` falls within the window, and seat defence steps. Then a staged takeover (a big player army spawned at the seat before tick 0 + Attack) gives outcome Won.
- `tests/tests/director.rs`:
  - (quick, 3 days) a notable event within `first_event_within_hours`; no gap > `quiet_limit_hours` in Chronicle; `step_until_event` stops at the next one.
  - The 40-day version lives with bot_run (`#[ignore]`).

**Docs**: progression.ron `rival`, the new `## director.ron` section, GLOSSARY (Rival, Levy, Director, Tension), ARCHITECTURE (Ai set rows).

**Risks**
- `at_war` makes raiders, rival and beasts fight each other: acceptable flavour. Keep raiders' prey on PLAYER (D2), or the rival's growth curve becomes noisy.
- `spawn_army`'s RNG is keyed by the first SpawnId (`spawn.rs:124`), so spawn order is part of the contract. Spawn only inside the driver.

### Step 8. Save and resume (GP-12, ADR 0001) — test `save_load`
- **Dependencies:** add `postcard` (1.1.x, std/alloc features) and `zstd` (0.13.x) to `[workspace.dependencies]` and to sim_core. Approved in ADR 0001 and M4_PLAN ("with your approval of postcard and zstd"); say so in the commit. Verify the versions offline before pinning. Never bincode. No bevy_math `serialize` feature: write IVec2/Vec2 as i32/f32 pairs.
- `sim_core::save`:
  - `SaveState` trait (mirrors StateHash): `save(&self, w: &mut SaveWriter)` / `load(r: &mut SaveReader) -> Result<Self, SaveError>`.
  - Writer and reader helpers: u8..u64, i32, f32, bool, ivec2, vec2, str, bytes, `entity` (SpawnId, or 0 for none; the reader remaps through the SpawnId→Entity map), plus `serde<T: Serialize>` via postcard for plain data types.
  - `AppSaveExt::{register_saved::<T: Component+SaveState>, register_saved_resource::<T: Resource+SaveState>, on_load(fn(&mut World))}` and `SaveRegistry`.
  - `HashRegistry` must keep **hashed TypeIds separate from exempt** ones (today `covered` merges them, `hash.rs:126-133`). Add `hashed: BTreeSet<TypeId>` plus an accessor.
  - `SaveHeader { magic: *b"WLSV", format: u32, build: &'static str (CARGO_PKG_VERSION), seed, preset, run: Option<RunConfig> }` (postcard).
  - `save_world(&mut World) -> Vec<u8>`: header + zstd(payload). The payload has the resources in registration order, then the SpawnId list (sorted), then each component type: name, then per entity a presence byte + data.
  - `load_world(&mut World, &[u8]) -> Result<(), SaveError>`:
    1. Check magic and format.
    2. Restore resources.
    3. Spawn one entity per SpawnId, in order.
    4. Insert components.
    5. Run `on_load` hooks in plugin order.
- **Per-crate save impls**, for every hashed type:
  - *sim_core:* SimSeed, Calendar (serde), SimTick, SimRng (`get_seed`/`get_stream`/`get_word_pos` and their setters), NextSpawnId, CommandsProcessed, NameTable (needs a `pub fn names()` and `from_names`), RunState, Offers, Chronicle, FactionModifiers, SpawnId, Position, Motion, Allegiance, DetailLevel.
  - *Data resources* (Economy, Villagers, Army, WorldGenSettings, Progression, DirectorData): save the **data RON text** and rebuild with `from_ron` on load, so old saves keep their tuning.
  - *sim_world:* WorldMap is **regenerated** from seed + saved settings in an `on_load` hook. Assert `WorldMap::digest()` equals the saved digest; overlay chunks whose `chunk_revision ≠ 0` (new accessors; nothing changes terrain at runtime today). Also RoadNetwork, Settlement, Mobile, Monument, SettlementSize. `on_load` rebuilds SpatialGrid: expose `pub fn rebuild_spatial_grid(world)`, since `rebuild_grid` is `pub(crate)` at `spatial.rs:154`.
  - *sim_economy:* GoodId and RecipeId as u8/u16. Stockpile, Inventory, Transfers (None between ticks), Building, Workplace, Home, SettlementEconomy. Entity fields are written as SpawnIds.
  - *sim_agents:* Villager, Employment, Needs, Activity, Travel (save the goal, `path[0]` and next; re-plan on load, since paths are pure), NextDecision, FoodSecurity, Enlisted, CombatStats, Threat, Strike, Owner, Goodwill, GoodwillLedger, Loyalty, SettlementLoyalty.
  - *sim_military:* Army (text), Factions, ArmyStats, Captain, Follower, Equipment, Soldier, Raider, plus every M4 type. `on_load` runs `build_index` once (`world.run_system_once`) so `raiders::think` and `militia_standing` see a populated ArmyIndex and FighterGrid on the first tick. FlowCache refills in `run_orders` (`get_or_build` every tick, `orders.rs:849`, `:877`) before `build_index`.
- **sim_headless:**
  - `SimPlugins.generate: bool`; `WorldPlugin { generate }` skips `finish` generation. AgentsPlugin already skips populate without a WorldMap.
  - `pub fn load_app(bytes, threading) -> Result<App, SaveError>`.
  - CLI `--save <file>` and `--load <file>`.
  - Commands after a load: re-insert `CommandScript::from_log(log filtered to tick ≥ saved tick)`. CommandLog and CommandScript are exempt and unsaved.

**Tests**
- `tests/tests/save_load.rs`, `save_load_restores_an_identical_hash`: a run with armies, a pending offer and an owned settlement (use the bot for ~1 day). Save, hash; `load_app` into a fresh app; equal hash. Step both apps M=1,000 ticks with the same remaining commands; equal hashes. Run with Multi and Single.
- `every_hashed_type_is_saved`: HashRegistry hashed TypeIds ⊆ SaveRegistry.
- NFR-07: time `save_world` and `load_app` and print them (≤ 3 s).

**Risks**
- `ArmyIndex.signatures` (exempt) is empty after a load, so every army renumbers slots at the next 50-tick boundary (`index.rs:413-453`). That's harmless only if renumbering is idempotent; prove it with the test.
- Messages in flight are not saved: save only between ticks.
- The Entity→SpawnId remap must cover every Entity field: Villager.home and settlement, Employment.workplace, Activity.target and dropoff, Strike.target, Building.settlement, Workplace.workers, Home.residents, SettlementEconomy.*, TransferOrder.store, Follower.captain, and any M4 fields.

**Docs**: ADR 0016, GLOSSARY (Save, Load hook), HOWTO "Add a component or resource" (now: register hashed **and** saved), DETERMINISM.md.

### Step 9. Events, chronicle, run summary (UI-05, UI-09) — test `chronicle`
- `sim_military::chronicle`: `pub fn describe(world: &World, e: &ChronicleEntry) -> String`, using NameTable, `Army::faction_name` and Calendar days. Phrases come from progression.ron (`traits[].phrase`, `causes`).
- Battle reports: `veterans::end_battles` assembles `BattleReport {sides, losses, turning_point (first rout tick), traits_earned}` into `Chronicle.reports` (hashed).
- `RunSummary` helper (pure fn over Chronicle, ArmyStats and RunProgress) gives days, settlements held, monuments, battles and veterans.
- `MilitaryLog` gains `seq` (`components.rs:318`).
- `sim_headless::stats::run_summary(app) -> Vec<String>`, plus a `--chronicle` CLI flag.

**Tests** `tests/tests/chronicle.rs`: a bot run (seed fixed, up to N days, or until outcome). The chronicle contains Conquest, MonumentTaken, a Battle report and RunEnded, each with a non-None cause; `describe` mentions the settlement name. Assert on `Chronicle`, not messages.

**Docs**: GLOSSARY (Chronicle, Event feed, Battle report), ADR 0017.

### Step 10. UI and rendering
- **game_app:**
  - `Start::Run` becomes the default. Keep `--skirmish`, `--villages` and `--stress n` (developed). Add `--difficulty`.
  - Use `sim_headless::set_up_scenario`.
  - An in-process **reset path** (exclusive, game_app): despawn SpawnId entities, reset sim resources, `generate_world` + `install_world` + `populate` + `setup_run`. Load reuses the clear step. Build a fresh App if reset proves fragile.
  - A render-side `WorldEpoch(u32)` resource tells caches to clear:
    - TerrainChunks.built (revisions repeat across worlds, `terrain.rs:148-150`)
    - BuildingVisuals.by_sim (no cleanup today)
    - labels (`Local<Labelled>` never despawns)
    - Selection, `setup_ocean`, `OpeningShot`/`frame_start` (move to Update on `resource_changed`)
  - Save on exit; F5/F9 quicksave and quickload.
- **game_ui** (bevy_ui only; egui stays dev-only behind the `debug_ui` feature):
  - `run_screens.rs`:
    - New-game screen: seed typed via KeyboardInput into a UI String, or random from SystemTime (`rand::rng` is denied); difficulty; three boon cards from `Offers` and Progression; modifiers shown.
    - End screen: RunState outcome plus the chronicle timeline and numbers; "New run" and "Same seed again".
    - Full-screen Node with `GlobalZIndex(20+)` and an Interaction so `pick` ignores world clicks (`selection.rs:88`).
    - Add a `UiScreen {NewGame, Playing, Ended}` Bevy State. Gate `speed_keys` (Space, 1-4), army hotkeys and right_click (which ignores UI hover), and camera N.
  - `run_bar.rs` (next to the time bar, `time_bar.rs:62`):
    - the day via `Calendar::at(tick).day + 1`; also fix `army.rs:576` (`tick / 2400 + 1`)
    - region shares from RunProgress.dominion
    - rival strength and the next clock step from RivalState
    - waiting offers as buttons that open cards; track sent ids in a `Local<BTreeSet>`
    - a fast-forward button with auto-pause on director-configured kinds
  - `feed.rs`: bottom-left. Reads `EventFeed` by `seq` cursor plus MilitaryLog by `seq`. Click jumps the camera.
  - Panels:
    - settlement panel (`panels.rs:342`, replace the placeholder at `:373`): owner, goodwill bar, mean/min loyalty, requests
    - captain panel (`army.rs:474`): champion rank and traits, slots used/available; grey out Appoint
    - inspector (`panels.rs:236`, separate query; SoldierQuery has 10 items): rank and traits
    - menus: Garrison, Give, Forage
  - Hover tooltip for monuments: ray-cast only when the cursor moves (`ground_hit` takes up to 20k steps).
- **game_render:**
  - `monuments.rs`: cuboid `parts(kind)`, sharing `Finish`/`Part`/materials with buildings.rs (make them `pub(crate)`). Holder pennant via `faction_color`.
  - Overlay tint in terrain.rs: an `OverlayTint` generation folded into the TerrainChunks stale check. Bump the generation only on **quantised** colour changes; 48 chunks rebuild per frame.
  - Gizmo rings for the monument overlay.
  - The rival and beasts get colours from army.ron (`agents.rs:101` falls back to grey).
- Rules: no writes to sim entities, not even markers; picks go out through `army::send`. Every new system gets a tracing span and a `UiSet`/`RenderSet` (none exist today; add them). Verify visually under Xvfb (GETTING_STARTED §6).

### Step 11. Bot run, tuning, benchmark, playtest checklist
- Bot strategy:
  1. Pick a boon.
  2. Forage or Give for requests.
  3. Take near monuments.
  4. Take weak settlements (Attack, then Garrison).
  5. Recruit and appoint up to the slots.
  6. Equip from the smithy.
  7. March on the seat when the army is bigger than garrison × k, or when the rival raids.
- `tests/tests/bot_run.rs` `#[ignore]`: 5 fixed seeds × ≤ 40 days (96,000 ticks), run **in parallel threads** (one app each; memory is fine at ~0.3 GB each). Assert wins ≥ 3 and print outcome and day per seed. The director's 40-day quiet-limit check goes here too.
- CI (`.github/workflows/ci.yml`) steps:
  - `cargo test --release -p integration_tests --test bot_run -- --ignored`
  - a run replay smoke: `$bin --run --bot --seed 42 --ticks 24000 --record run.ron && $bin --replay run.ron && $bin --replay run.ron --single-threaded`
- A short bot determinism test (2–3 days) runs in the normal suite: Multi vs Single, then record, RON round-trip, verify.
- Bench `tick/run_island` in `crates/sim_headless/benches/tick.rs`. Pre-run the bot to day ~10, then iterate `step(1)` with a pre-recorded CommandScript (no bot cost). Add it to `criterion_group!` (`:71`). Use `--run --bot --timings --stats-every 250` for the worst tick.
- CLI `--bot`. `stats::run_summary` goes in the `--stats-every` loop (`main.rs:345-349`).
- Write `docs/M4_PLAYTEST.md`.
- Tuning loop: change data only, re-run bot_run plus every CLAUDE.md-guarded test. If it's flaky, loosen the bar (M4_PLAN default 5), don't fudge the data.

### Step 12. Docs, ADRs, SEED criteria, CI
- Review ADRs 0013–0017.
- Tick the SEED.md M4 criteria (GP-05/06/12/16/21–28, SIM-24/34–36/38, UI-02/04/05/09).
- Update the CLAUDE.md milestone line, and add CLAUDE.md sections for runs, standing and loyalty, monuments and modifiers, and saves (house rules like "every hashed type is also saved").
- TESTING.md times. GETTING_STARTED aliases (optional `cargo bot`). HOWTO recipes: add a monument type, a boon, trait or run modifier, a run scenario test.
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`.

---

## 4. Gotchas

**Determinism and hashing**
- **G1.** `setup_battle` writes `SimCommand::player` before tick 0 (`spawn.rs:252-259`). `record_commands` logs them, and `verify_replay` re-runs the setup **and** replays the log, so the commands apply twice. Battle replays diverge (untested). Fix it (D12), and never write player messages in setup.
- **G2.** AI writers and `inject_scripted` are unordered in Inject today (`sim_military/src/lib.rs:115`), and `apply_commands` applies in arrival order. Use `SimInputSet::Ai` (D7).
- **G3.** `advance_tick` has no ordering inside Bookkeeping. Put SimTick readers in `BookkeepingSet::Record`.
- **G4.** Starved soldiers and captains are never despawned: `find_fallen` scans only the grid, and the grid skips the dead (`combat.rs:34`, `index.rs:483`, `upkeep.rs:113-121`). A starved champion would never end the run. Fix in step 3.
- **G5.** `unhashed_state` checks only types present at that moment, only `sim_*` resources, and only components on SpawnId entities. Messages, game_* resources and components on non-SpawnId entities escape it. Use always-present state (D1) and extend `run_state_hash_covers_every_sim_component` each step.
- **G6.** Hand-written StateHash impls must be extended for new fields: Captain (`components.rs:119`), FactionStats (`:295`), Settlement (`sim_world/src/lib.rs:52`), Raider.
- **G7.** Hash order = registration order, and the type name is hashed. Moving or renaming a type changes every hash. That's fine, since nothing is golden.
- **G8.** No HashMap iteration. Use BTreeMap or sorted Vecs. Sort parallel results by SpawnId. Accumulate cross-settlement deltas (goodwill spread) before applying them. Sort f32 sums into a canonical order (for example by delta bits) when writers' order may vary.
- **G9.** `Economy`, `Villagers` and `Army` hash only their `data`. Mutating resolved fields (recipes, trade_value) at runtime changes behaviour without changing the hash, so apply modifiers at the use site (D11).
- **G10.** New RNG draws use new keys (D9). Never extend `sim_agents::populate` (`population.rs:180`) or `sim_world::gen::names`. A new settlement or monument spawned before villagers shifts every villager SpawnId, and with it NextDecision staggering, courage and carter jitter (`population.rs:307`, `ai.rs:350`, `:442`). That is acceptable in the run preset only.
- **G11.** Never borrow `SimRng`.
- **G12.** `spawn_sim` uses deferred Commands. Spawn only in exclusive or serial steps, with `world.resource_mut::<NextSpawnId>().allocate()` + `world.spawn`, as every current spawner does.
- **G13.** Exhaustive matches on CommandKind and OrderSpec live in `orders.rs` (`apply`, `validate_spec`, `step_order`) and `components.rs` (`hash_spec`, `describe`). game_ui has `_` fallbacks. Land variants and every arm in one commit. Append only (postcard encodes the index); OrderSpec stays Copy+Eq (integers only).
- **G14.** `REPLAY_FORMAT` is a hard gate with no migration. Bump it to 5 once in step 1 (D5).
- **G15.** Messages live 2 ticks headless. Tests read Chronicle or EventFeed. Commands written while the game is paused wait for the next tick.
- **G16.** The run gate freezes SimTick. `run()` keeps stepping, so CommandScript entries after the end are never injected and `RunResult.log` is shorter. Don't assert `log == commands` on runs that end.

**Structure and data**
- **G17.** `default_data()` must keep meaning developed (D5). `with_size` scales only the `*_at_2048` fields. Absolute tile constants (core_radius, PathCache MARGIN 48, the flow window margin 48, `nearest_passable` 64 rings, HOUSE_RADIUS 50, MAX_RADIUS 64) do not scale.
- **G18.** HeadlessConfig and Scenario must stay Copy (+Eq for Scenario). RunConfig must be Copy+Eq+Default; M6 unlocks must not add a Vec to it without fixing `main.rs` and `record_replay`.
- **G19.** ArmyData has no serde defaults, so every new army.ron field must be present. Every new RON field needs a DATA.md row in the exact `## <file>` section. A new .ron file without a section **panics** docs.rs. Paths named in the handbook must already exist in the same commit.
- **G20.** The `Terrain` enum has no serde. Use MonumentKind or site-rule enums in RON, or add derives deliberately.
- **G21.** Settlement-level components exist only on populated settlements. In the run preset every settlement is populated; in developed only one region is. `find_settlement`, raiders, `setup_skirmish` and UI "populated" checks key on SettlementEconomy.
- **G22.** `FoodSecurity.days` excludes farm stock and carried food, and dips by day. `spare_food` counts the storehouse only. Pick one per rule.
- **G23.** Farming output (8) equals carry (8), so a positive yield bonus is silently dropped (`act.rs:217`). Use `add_over_capacity` and update `out_units` (`ai.rs:395`).
- **G24.** `orders::forage`, `Captain.last_forage` and `upkeep.forage_radius` mean settlement auto-gather. Name wild foraging `wild`.
- **G25.** `FactionStats.given` means village→army. Add new fields (`donated`, `supplied`, `conquests`) and extend the StateHash.
- **G26.** `Threat.provoked_by` is a single slot (last writer wins, `orders.rs:322`) and lasts half a day. Revolts must refresh it. Starving villagers (hunger ≥ 0.85) can't become militia.
- **G27.** `appoint` counts every captain of the issuer, champion included, against `max_captains` (9). The slot rule applies only `in_run && issuer == PLAYER`, or `sample_orders` appointments in skirmish break.
- **G28.** The champion marker must live on the person, not in Captain (`succeed` moves Captain, `combat.rs:196-206`). The succession candidates are computed before `succeed` creates the heir.
- **G29.** Region 0 is always central. Tile regions come from a BFS over land including Mountain; unreachable land is `NO_REGION` (255), so filter it from per-region Vecs. `WorldGenSettings.regions` is a request; the actual count is `min(regions, settlements, 255)`, so use `map.region_count()`.
- **G30.** `spread_weight` in worldgen.ron is parsed but never read (`settle.rs`). Implementing it changes developed worlds unless it is gated.

**Tooling: memory, disk, time**
- **G31.** Build with **`-j 2`**: `cargo build -j 2`, `cargo test -j 2 --workspace`, `cargo clippy -j 2 --workspace --all-targets -- -D warnings`. The box has 4 cores and 15 GB RAM; CLAUDE.md's `-j 3` is the ceiling. Set `CARGO_INCREMENTAL=0` as CI does.
- **G32.** **Disk:** `target/` is ~19 GB and `/` has ~9.8 GB free. Release builds (bench, bot_run, replay smoke) plus debug test builds can fill it. Check `df -h /` before release builds. If it runs low, `rm -rf target/debug/incremental` or `cargo clean --release`. Don't run `cargo doc` for dependencies (`--no-deps` only).
- **G33.** sim_core, sim_headless and the tests build at opt 1 in `cargo test`, so heavy save, hash or bot work is slow in tests. Adding `[profile.dev.package.sim_core]` (and `sim_headless`) `opt-level = 3` is a profile change, not a dependency change, and is consistent with the existing overrides; mention it in the commit. Run expensive scenario files with one `#[test]` that loops over seeds sequentially, because tests in a file run in parallel threads with a full world each.
- **G34.** `cargo test --workspace` is already ~4 min. Keep every new scenario under ~60 s at the test profile. bot_run is `#[ignore]` and runs in a release CI step.
- **G35.** Never use bincode. Do not upgrade Bevy (=0.19.1). Ask before any new dependency other than the approved postcard and zstd.
- **G36.** rustdoc: a `pub mod run` next to a `pub fn run` (sim_headless) makes `[`run`]` ambiguous. Every new pub item in sim_* needs `///`, and the docs build uses `-D warnings`.
