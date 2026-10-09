# M4 build progress log

Implementers append a section per step part: what was built, any deviation from m4_codemap.md (and why), new names other steps must use, and open issues. Read this before starting a part.

## Step 1 part 1a-sim_core

### What was built
- **`crates/sim_core/src/run.rs` (new)**
  - `Difficulty {Steady (default), Chaotic, Harsh}` with `ALL`, `name()` ("steady"/"chaotic"/"harsh") and `parse(&str)`, for `--difficulty`.
  - `RunConfig {difficulty}`: Copy, Eq, Hash, Default, serde, with `#[serde(default)]` on the struct.
  - `RunOutcome {Won, Lost, Abandoned}` with `name()`.
  - `RunState {config: Option<RunConfig>, pending: Option<(RunOutcome, Cause)>, outcome: Option<RunOutcome>, cause: Cause, ended_at: Option<u64>}`: a resource, Copy, hashed, always present. Methods: `in_run()`, `is_over()` (outcome or pending), and `finish(outcome, cause) -> bool`.
  - Run conditions `run_in_progress` and `in_run`, both taking `Option<Res<RunState>>`.
  - `settle_outcome`, a pub system, and `pub(crate) outcome_pending`.
  - `OfferKind {StartBoon, DominionBoon{region: u8, step: u8}, Trait{person: u64}, Invitation{settlement: u64}, Hire{captain: u64}}` with `tag()` and `subject()`.
  - `Offer {id, faction, kind, choices: Vec<u16>, posted, expires: Option<u64>}`.
  - `Offers {next_id, pending}` with `post(faction, kind, choices, tick, expires) -> id` (**no event**), `get(id)`, `take(id)`, `for_faction(f)` and `expire(tick) -> Vec<Offer>` (removes offers with `expires <= tick`).
  - Free fn `post_offer(world, faction, kind, choices, expires) -> id`: posts at the current SimTick and writes an `OfferPosted` SimEvent (`subject` = offer id, `other` = `kind.subject()`, `amount` = number of choices).
- **`crates/sim_core/src/events.rs` (new)**
  - `SimEventKind`: `#[repr(u8)]`, the 22 D8 variants in D8 order. Has `ALL`, `name()`, `chronicled()` and `notable()`. Each variant's doc says what `subject`, `other` and `amount` mean.
  - `Cause`: None (default), Player, Request{id}, Conquest{captain}, Invitation, Starvation, Plunder, Revolt, Raid{captain}, Hold, Boon{card: u16}, Death{faction}. Has `tag()` and `payload()`. Append only.
  - `SimEvent {kind, faction, subject, other, at: IVec2, amount: i32, cause}`, a Message. Builder: `SimEvent::new(kind, faction).subject(..).other(..).at(..).amount(..).cause(..)`, plus `sort_key()`.
  - `ChronicleEntry {tick, event}`.
  - `Chronicle`: fields are private. Accessors: `push(entry)`, `entries()`, `len()`, `is_empty()`, `digest()` and `last_notable()`. The hash covers len, digest and last_notable.
  - `FeedEntry {seq, tick, event}`.
  - `EventFeed`: `CAPACITY` 256, `push(tick, event) -> seq`, `since(cursor)`, `iter()`, `next_seq()`, `len()`.
  - System `record_events` (span `sim_core::record_events`), plus the internal helper `record(tick, event, &mut Chronicle, &mut EventFeed)`.
- **`crates/sim_core/src/modifiers.rs` (new)**
  - `Stat`: 14 variants in D11 order, `#[repr(u8)]`, serde. Has `COUNT`, `ALL`, `index()` and `name()`.
  - `ModifierSource {Monuments{kind: u8}, SetBonus{kind: u8, region: u8}, Boon{card: u16}, Trait{id: u16}, RunModifier{card: u16}}`.
  - `ModifierEntry {faction: Option<FactionId>, stat, source, value}`.
  - `FactionModifiers {by_faction: Vec<[f32; 14]>, world: [f32; 14], breakdown: Vec<ModifierEntry>}` with `value(f, s)`, `mult(f, s)` (exactly 1.0 when empty), `clear()`, `add(f, stat, source, v)` (resizes like `ArmyStats::of`) and `add_world(stat, source, v)`. Registered hashed and empty.
- **`components.rs`**: `FactionId::{INDEPENDENT, PLAYER, RAIDERS, RIVAL, BEASTS}` = 0..4. The FactionId doc is updated.
- **`command.rs`**:
  - `CommandKind::ChooseOffer {offer: u64, choice: u8}` and `CommandKind::AbandonRun`, appended.
  - `SimInputSet::Ai`, between Inject and Consume.
- **`schedule.rs`**:
  - `BookkeepingSet {Record, Advance}`, chained inside `SimSet::Bookkeeping`.
  - The run gate: `SimSet::X.run_if(run_in_progress)` for all nine sets.
  - Run conditions `on_hour` and `on_day`, as `fn(Res<SimTick>, Res<Calendar>) -> bool`.
  - Docs on `advance_tick`.
- **`calendar.rs`**:
  - `ticks_per_hour()` = `ticks_per_day / 24`, at least 1 (100 today).
  - `hours(f32) -> u64` and `days(f32) -> u64`, rounded via f64; negative values give 0.
  - Methods `on_hour(tick)` and `on_day(tick)`.
- **`lib.rs`**:
  - `SimCorePlugin {seed, run: Option<RunConfig>}`. It inserts `RunState {config: run, ..}` and inits Offers, Chronicle, EventFeed and FactionModifiers.
  - Registers RunState, Offers, Chronicle and FactionModifiers as hashed, appended after NameTable. Exempts EventFeed ("UI feed, derived from SimEvents").
  - `add_message::<SimEvent>()`.
  - Chains `(Inject, Ai, Consume)`.
  - Record: `record_events → settle_outcome.run_if(outcome_pending)`. Advance: `advance_tick`.
  - Root re-exports cover every type above, plus `in_run`, `run_in_progress`, `post_offer`, `on_hour`, `on_day` and `BookkeepingSet`. Crate docs are updated.
- **`replay.rs`**: `#[serde(default)] pub run: Option<RunConfig>` sits after `scenario`, and `REPLAY_FORMAT = 5`. The `sample()` fixture now has `scenario: "run"`, `run: Some(Harsh)` and a ChooseOffer command.
- **Other crates**:
  - `sim_headless::SimPlugins::build` passes `run: None`, and `record_replay` writes `run: None`; part 1-sim_headless fills both in.
  - `sim_military::orders::apply` gains the arm `ChooseOffer {..} | AbandonRun => Err("no run")`.
- **Docs**:
  - ARCHITECTURE: the sim_core crate row, a run-gate paragraph, schedule rows 1 (Ai) and 9 (Record/Advance).
  - GLOSSARY: Chronicle, Event feed, Game hour, Offer, Run, RunConfig, SimEvent, Stat/FactionModifiers, and an updated Faction row.
  - TESTING: the sim_core unit-test bullets.
  - DETERMINISM: the Ai set, an events rule, and the run config in replays.
- **Unit tests**, 32 in sim_core, all passing:
  - `step.rs`: `a_finished_run_stops_the_clock`, `outside_a_run_the_clock_never_stops`
  - `events.rs`: `events_are_recorded_in_sorted_order`, `the_feed_keeps_the_most_recent_entries`, `every_kind_is_listed_in_order`
  - `calendar.rs`: `an_hour_is_a_twentyfourth_of_a_day`
  - `replay.rs`: `ron_round_trip` (with `run: Some`, with `None`, and with the field missing)
  - `run.rs`: `finish_is_first_caller_wins_and_only_in_a_run`, `offers_are_taken_once_and_expire`, `posting_an_offer_reports_it`, `difficulty_names_round_trip`
  - `modifiers.rs`: `no_modifiers_is_an_exact_no_op`, `faction_and_world_rows_add_up`, `every_stat_is_listed_in_order`

### Deviations from the guide (and why)
1. **`Offers::post` does not emit `OfferPosted`.** A resource method can't write messages. Exclusive code calls **`sim_core::post_offer(world, faction, kind, choices, expires)`**, which posts and emits. Use bare `Offers::post` only where no event is wanted.
2. **`on_hour` / `on_day` naming.** The root fns are **run conditions** (`.run_if(on_hour)`). For plain checks inside exclusive steps, use the Calendar methods `cal.on_hour(tick)` / `cal.on_day(tick)`. D7's `on_hour(tick, cal)` signature can't coexist with the run-condition form under one name.
3. **`FactionModifiers.breakdown`** is `Vec<ModifierEntry>`, where `faction: None` marks the world row. D11 had a tuple `(FactionId, Stat, Source, f32)`, which had no way to express the world row. `Source` is named **`ModifierSource`**, to avoid a generic name at the crate root.
4. **`settle_outcome` records the `RunEnded` event itself**, directly into Chronicle and EventFeed, after `record_events`. A SimEvent written in Record would never be read, because the next tick is gated off.
   - RunEnded has `faction` PLAYER, `subject` = `outcome as u64` (Won 0, Lost 1, Abandoned 2) and `cause` = the finish cause. It is always the last chronicle entry.
   - **Callers of `RunState::finish` must not emit RunEnded themselves.**
5. **`RunState::finish` returns bool.** It changes nothing and returns false outside a run (`config` None), if an outcome is already pending (first caller wins), or once ended. This guards developed invariance.
6. **Chronicle fields are private**, to keep the digest consistent.
   - Step 8 (`sim_core::save`) can reach them from inside the crate.
   - Step 9 adds `reports`; extend the hash and keep it O(1).
   - The digest chains as `StateHasher(prev_digest, tick, event)`. It starts at 0.
7. **`SimEventKind::notable()` is added**: every chronicled kind plus `RequestPosted`. `Chronicle.last_notable` tracks notable events, not only chronicled ones.
   - `chronicled()` is every kind except OfferPosted, RequestPosted, VeteranRank, RivalLevy and DriftersJoined.
   - Steps 7 and 9 may retune both lists. The lists only decide which events reach the Chronicle and update `last_notable`; changing them changes no stream.
8. **`raiders::think` was NOT moved into `SimInputSet::Ai`.** That edit is in sim_military (part 1c). The Ai set exists, chained, and is empty for now.
9. **The `record_events` sort key** is `(kind, faction, subject, other, at.x, at.y, amount, cause.tag, cause.payload)`. D8's key plus the cause, for a total order.
10. **Serde derives.**
    - Offer, OfferKind, Cause, SimEventKind, Stat, ModifierSource, RunOutcome, RunConfig and Difficulty derive serde.
    - SimEvent and ChronicleEntry do not, because `IVec2` has no serde feature. Step 8 writes them by hand: kind u8, faction u16, subject u64, other u64, at i32 x2, amount i32, cause tag u8 + payload u64.

### Names later parts must use
- **Types, at the `sim_core` root:** `RunConfig`, `Difficulty`, `RunState`, `RunOutcome`, `Offers`, `Offer`, `OfferKind`, `SimEvent`, `SimEventKind`, `Cause`, `Chronicle`, `ChronicleEntry`, `EventFeed`, `FeedEntry`, `FactionModifiers`, `Stat`, `ModifierSource`, `ModifierEntry`, `BookkeepingSet`.
- **Functions and conditions, at the root:** `post_offer`, `in_run`, `run_in_progress`, `on_hour`, `on_day`.
- **Systems:** `sim_core::events::record_events` and `sim_core::run::settle_outcome`. Modules: `sim_core::{run, events, modifiers}`.
- **Plugin:** `SimCorePlugin { seed, run }`. Only two construction sites exist: `sim_headless::SimPlugins::build` and the sim_core unit tests.
- **Ending a run:** `world.resource_mut::<RunState>().finish(RunOutcome::X, Cause::Y)`, from any serial step before Bookkeeping. For abandon, check `RunState.in_run()` and `!is_over()` first, and refuse with Err.
- **Calendar:** `cal.ticks_per_hour()`, `cal.hours(h)`, `cal.days(d)`, `cal.on_hour(t)`, `cal.on_day(t)`.
- **Factions:** `FactionId::{INDEPENDENT, PLAYER, RAIDERS, RIVAL, BEASTS}`.
- **Replay header:** `Replay.run: Option<RunConfig>`, with `REPLAY_FORMAT` = 5.
- **Don't name these at the sim_headless root:** sim_core now has a `run` module, so don't `use sim_core::run;` there (G36).

### Open issues
- **Part 1c (sim_military):**
  - Replace the `Err("no run")` arms with `progression::choose` / `abandon`.
  - Move `raiders::think` into `SimInputSet::Ai`.
  - Swap `FactionId(1)` / `FactionId(2)` literals for the constants in game_ui, spawn.rs and sample_orders. These swaps are not done.
- **Part sim_headless:**
  - `SimPlugins.run` and `SimPlugins::run(seed, cfg)`.
  - `record_replay` writes `run` from `Scenario::Run`; `verify_replay` reads it.
- **Not written by this part:** ADR 0013 (runs and RunConfig) and ADR 0017 (events). CLAUDE.md's "Systems that act on commands go in SimInputSet::Consume" could also mention the Ai set; that belongs to step 12.
- **Hash values changed** in every preset and scenario, because 4 resources were added to the hash. Behaviour is unchanged, and no golden hashes exist.
- **Checks run:** `cargo test -j 2 -p sim_core` (32 ok); `cargo check -j 2 --workspace --all-targets` (clean); `cargo clippy -j 2 --workspace --all-targets -- -D warnings` (clean); `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps` (clean); `cargo fmt --check` (ok); `cargo test -j 2 -p integration_tests --test determinism` (9/9 ok, 153 s); `--test docs` (4/4) and `--test orders` (7/7) ok.

## Step 1 part 1b-sim_world

### What was built
- **`crates/sim_world/src/settings.rs`**
  - `WorldPreset {Developed (default), Run}`: Copy, Eq, Ord, Hash, Default, serde. Has `ALL`, `name()` ("developed"/"run") and `parse(&str) -> Option<Self>`.
  - `WorldGenPresets {developed, run}` (both full `WorldGenSettings`). Has `from_ron` (validates both; errors are prefixed with the preset name), `default_data()` and `get(preset) -> &WorldGenSettings`.
  - `WorldGenSettings` gains `monuments: MonumentSettings` and `sizes: SizeSettings`. Both are explicit in both presets; there are no serde defaults.
  - `WorldGenSettings::preset(p)`; `default_data()` is now `preset(Developed)`. `WorldGenSettings::from_ron` still parses a single settings block.
  - `MonumentSettings {per_region: u32, min_settlement_distance: f32, min_spacing: f32, kinds: Vec<MonumentKindRule>}`.
  - `MonumentKindRule {kind: MonumentKind, weight: f32, site: SiteRule}`.
  - `SiteRule` (Copy, serde): `NearWater{min: u8, max: u8}`, `Hills`, `HillsNearMountain{within: u8}`, `NearRoad{min, max}`, `ForestEdgeOrRoad`, `GrassOrHills`.
  - `SizeSettings {tiers: Vec<SizeTierRule>, seat_min: u32}`; `SizeTierRule {tier: SizeTier, weight: f32, min: u32, max: u32}`.
  - `validate()` now also checks:
    - monuments: per_region > 0 needs kinds; distances ≥ 0; each kind listed at most once; weights finite and > 0; NearWater/NearRoad min ≤ max; enough syllable combinations for `settlements.max + per_region × regions`.
    - sizes: each tier listed at most once; weights finite and > 0; 0 < min ≤ max; `seat_min > 0` needs at least one tier.
  - Unit tests: `shipped_data_is_valid` (developed: 2048, regions 10, 40..=80, no monuments or sizes, equals `preset(Developed)`), `run_preset_is_valid` (1024, regions 4, monuments, tiers, min distance > 45), `preset_names_round_trip`, `rejects_bad_monument_and_size_rules`.
- **`crates/sim_world/src/monuments.rs` (new)**
  - `MonumentKind`, 8 variants in D1 order, `#[repr(u8)]`, serde. Has `COUNT`, `ALL`, `index()`, `name()` (lower-case, e.g. "standing stones") and `title()` (the noun used in names, e.g. "Stones").
  - `Monument {kind, region: RegionId, name: NameId}`: a component, Copy. StateHash writes u8 kind, u8 region, u32 name.
  - `MonumentSite {kind, center: IVec2, region, name: String}` (PartialEq).
- **`crates/sim_world/src/lib.rs`**
  - `SizeTier {Hamlet, Village, Town}`: `#[repr(u8)]`, serde, `ALL`, `name()`.
  - `SettlementSize {tier, people: u32}`: a component, Copy. StateHash writes u8 + u32.
  - `register_hashed::<SettlementSize>()` and `register_hashed::<Monument>()`, right after Settlement.
  - `install_world` spawns settlements first, inserting `SettlementSize` when it is Some, **then** monuments: push the name, `allocate()` a SpawnId, spawn `(SpawnId, Monument, Position)` with no Mobile.
  - New root re-exports: `Monument`, `MonumentKind`, `MonumentSite`, `WorldGenPresets`, `WorldPreset`, `SizeTier`, `SettlementSize`, `draw_sizes`.
  - Unit test `install_spawns_settlements_then_monuments`.
- **`crates/sim_world/src/map.rs`**
  - `region_count() -> usize`: computed in `from_parts` (max non-NO_REGION + 1, or 0). It is derived from the hashed region array, so it isn't hashed separately.
  - `is_region_edge(p) -> bool`: p is in a region and a 4-neighbour is off the map, Water, or in another region (including NO_REGION). False for NO_REGION tiles and out-of-bounds p.
  - Unit test `regions_count_and_edges`.
- **`crates/sim_world/src/worldgen/sizes.rs` (new)**: `pub fn draw_sizes(seed: SimSeed, s: &SizeSettings, n: usize) -> Vec<Option<SettlementSize>>`, using stream `sim_world::gen::sizes` at tick 0. With no tiers, or `n == 0`, it returns all None and makes no draws. Unit tests: `no_tiers_draws_nothing`, `tiers_follow_their_shares_and_the_seat_is_the_largest`, `draws_are_seeded`.
- **`crates/sim_world/src/worldgen/monuments.rs` (new)**
  - `pub(crate) fn place(seed, &MonumentInputs, &WorldGenSettings) -> Vec<MonumentSite>`. Streams: `rng_key("sim_world::gen::monuments") ^ region` and `sim_world::gen::monument_names`, both at tick 0.
  - It returns `[]` (no draws) when `per_region == 0`.
  - Unit tests: `weighted_pick_covers_every_rule`, `nearest_breaks_ties_on_the_lowest_index`, `site_rules_read_the_terrain`.
- **`worldgen/fields.rs`**: new `distance_to(terrain, size, source)` (4-neighbour BFS over any terrain, capped at 255). `water_distance` now delegates to it with an identical result.
- **`worldgen/mod.rs`**
  - `SettlementSite.size: Option<SettlementSize>` and `GeneratedWorld.monuments: Vec<MonumentSite>`.
  - New stages, with laps: `sizes`, then `monuments`, after `regions+names` and before `WorldMap::from_parts`.
- **`assets/data/worldgen.ron`** is now `(developed: (...), run: (...))`.
  - developed is the old file verbatim, plus `monuments: (per_region: 0, min_settlement_distance: 0.0, min_spacing: 0.0, kinds: [])` and `sizes: (tiers: [], seat_min: 0)`.
  - run is identical to developed except:
    - `size: 1024`
    - settlements target/min/max 25/20/30
    - `regions: 4`
    - monuments: `per_region: 6`, `min_settlement_distance: 52.0`, `min_spacing: 40.0`, all 8 kinds. Weights are 1.0, except RoadMarker 0.8, AncientForge 0.6 and MarketCross 0.8.
    - sizes: Hamlet 0.20 (20-60), Village 0.45 (60-200), Town 0.35 (200-400), `seat_min: 450`
- **`tests/tests/worldgen.rs`**
  - `check_settlements` is parametrised by `s.settlements.min..=max`, and so is `the_sim_spawns_settlements_and_roads`. The latter also asserts that no Monument or SettlementSize entities exist in the developed app.
  - `same_seed_same_island_regardless_of_threads` also asserts that developed has no monuments and no sizes.
  - New `run_preset_is_the_same_regardless_of_threads` (1 vs 4 threads: map, settlements with sizes, monuments, roads).
  - New `run_preset_meets_its_criteria` (seeds 42, 7, 1234, 2026). It checks:
    - the M1 terrain, settlement and road checks
    - `region_count == 4`, with 4 distinct settlement regions
    - 4-8 monuments per region
    - none on Water, Road or Mountain; region matches the tile
    - every monument ≥ min_settlement_distance and > 45 from every centre, ≥ min_spacing from the others
    - unique names that never reuse a settlement name
    - every settlement has a size within its tier, the seat is the unique largest and ≥ seat_min, and the total is 3,500-5,500 people
- **Docs**:
  - DATA.md: the worldgen section has a presets table (`developed`, `run`) and rows for every new field.
  - TESTING.md: worldgen rows and sim_world unit bullets.
  - GLOSSARY: Monument, Preset, Region, Seat, Size tier.
  - ARCHITECTURE: the sim_world crate row.

### Measured (run preset, seeds 1..=20, survey not committed)
- Always 25 settlements, 4 regions, and 6 monuments in every region.
- Total population 4,020-4,690; the seat is always 450.
- Settlements per region are uneven: region 0 (central) has 7-12 settlements (1,100-2,100 people); the others have 3-7 (317-1,600 people).
- Monuments sit at 52-plus tiles from centres and 40-plus from each other; the constraints bind, but no relaxation was needed.
- Not every seed has all 8 kinds: 6-8 distinct kinds per island.
- At opt 3, the monuments stage takes 35-60 ms and sizes ~0 ms. A whole 1024 generation takes ~0.5 s with 4 threads.

### Deviations from the guide (and why)
1. **Size tiers are apportioned, not drawn independently.** The tier counts are `n × weight share` by largest remainder (ties go to the earlier tier). They are then Fisher-Yates shuffled over the settlements in index order, and people are drawn uniformly within the tier, all from the one `sim_world::gen::sizes` stream.
   - Why: the island's total stays stable (±~350), where independent draws give ±~500.
   - The weights are **0.20 / 0.45 / 0.35**, not the guide's example 0.35/0.45/0.20, which averages ~3,450 people and misses the 4,000-5,000 target. At n = 25 this gives 5 hamlets, 11 villages and 9 towns.
2. **The seat** is the largest drawn settlement (ties go to the lowest index), set to `max(drawn, seat_min, largest_other + 1)`. It is given the **top tier**: the rule with the largest `max`, with ties going to the earlier rule, so Town in practice. It is always the unique largest.
3. **Placement is anchor-based instead of "best score + jitter".** Per region:
   - All `per_region` kinds are drawn by weight first, then one anchor per slot, uniform among the region's eligible tiles.
   - The slot takes the suitable tile nearest the anchor that keeps the spacing. Distances are integer squared; ties go to the lowest tile index.
   - Eligible means: not Water, Road, Mountain or **Farmland**; in a region; at least `min_settlement_distance` from every centre.
   - Fallbacks: first the kinds after the drawn one (wrapping), then spacing × 0.5, then × 0.25. A slot that still fits nothing is skipped.
   - Spacing applies to every monument already placed, across regions.
4. **Site rules are data** (the `SiteRule` enum in worldgen.ron) with parameters: holy well `NearWater(min: 2, max: 6)`, forge `HillsNearMountain(within: 6)`, marker and cross `NearRoad(min: 1, max: 3)`.
   - "Open ground" = Grass, Forest, Beach or Hills.
   - ForestEdgeOrRoad = a Forest tile with a Grass, Beach, Hills or Farmland 4-neighbour, **or** open ground at road distance 1.
5. **`validate()` accepts `per_region: 0`.** Developed needs it, so the guide's "0 < per_region" can't apply. It adds the uniqueness, seat_min and name-capacity checks listed above.
6. **Monument names** have the form `"<Place> <Title>"`, e.g. "Kestmoor Stones" or "Saltmere Well". Place names come from the settlement syllables, excluding this world's settlement names, shuffled by `sim_world::gen::monument_names`. The full name goes into NameTable.
7. **`SizeTier` and `SettlementSize` live in `sim_world` lib.rs**, next to Settlement; `worldgen/sizes.rs` only draws. `SiteRule`, `MonumentKindRule`, `SizeTierRule`, `MonumentSettings` and `SizeSettings` are in `sim_world::settings` and are not root re-exports.
8. **`draw_sizes` takes `SimSeed`**, not u64.

### Names later parts must use
- **Root types:** `sim_world::{WorldPreset, WorldGenPresets, WorldGenSettings, Monument, MonumentKind, MonumentSite, SizeTier, SettlementSize, draw_sizes, generate_world, install_world}`.
- **Settings types:** `sim_world::settings::{MonumentSettings, MonumentKindRule, SiteRule, SizeSettings, SizeTierRule}`.
- **Constructors:**
  - `WorldGenSettings::preset(WorldPreset::Run)` (size 1024)
  - `WorldGenSettings::default_data()` (developed)
  - `WorldGenPresets::default_data().get(p)`
- **Map:** `WorldMap::region_count() -> usize`, `WorldMap::is_region_edge(IVec2) -> bool`.
- **GeneratedWorld fields:** `settlements[i].size: Option<SettlementSize>`, `monuments: Vec<MonumentSite>` (by region, then slot).
- **SpawnId order in the run preset:** settlements `0..n`, then monuments `n..n+m` in `GeneratedWorld.monuments` order, then everything populate and setup spawn. So `RunWorld` index order (settlements, then monuments) equals SpawnId order.
- **The seat** = the unique max of `SettlementSize.people`.
- **Kind display:** `MonumentKind::name()` (lower-case) for UI and summaries; `title()` only in names.

### Open issues / notes for later parts
- **Part sim_agents:** `populate` size mode keys on `SettlementSize`. Every run settlement has one, the seat has ≥ 450 and towns 200-400. Developed has none, so today's path is unchanged.
- **Part 1c (start draws):** 24 monuments per island. If "2-3 monuments within `monument_radius` of the start" proves hard for some seeds, tune `progression.ron` first. Otherwise adjust worldgen.ron `monuments.per_region` or `min_spacing` and re-run `--test worldgen`; `run_preset_meets_its_criteria` asserts 4-8 per region and spacing ≥ `min_spacing` with no slack.
- **Monuments have no Holder yet** (step 4). Remember D2/step 4: guard camps sit at the monument, which is ≥ 52 from centres; threat radius is 45.
- **Developed invariance verified.** The release `sim_headless --seed 42 --ticks 1200 --stats-every 600` was compared with the pre-M4 release binary:
  - identical terrain mix; settlements=60 roads=81 road_tiles=10451
  - identical village stat lines, and stderr is identical apart from timings
  - only the state hash and the Bevy `Entity` bits in "never ate" debug lines differ. The Entity bits shifted by 6 because part 1a added resources, which are entities in Bevy 0.19; that isn't sim state.
- **Developed hash values changed:** WorldGenSettings is hashed as RON text, which now has the new fields, and two types were newly registered. Nothing is golden.
- **`WorldGenReport` gains laps** "sizes" and "monuments", even in developed: the headless summary prints `sizes=0ms monuments=0ms`.
- **ADR 0013 (not written here):** it should record the preset decisions. Those are: two complete blocks in one file; `default_data()` = developed; sizes and monuments only in run, so developed is byte-identical; apportioned size tiers; anchor-based monument placement with data site rules; monuments kept beyond the threat radius.
- **CLAUDE.md's "World (ADR 0005)" section** could mention presets in step 12.

## Step 1 part 1c-run-setup

I resumed an interrupted attempt. Its edits were mostly correct: the progression module, progression.ron, the population size mode, ArmySpec, the G1 fix, the raiders Ai set, army.ron factions and the orders arms. I kept them and fixed the start draw: seed 42 gave 5 monuments near the start. I then finished sim_headless, game_app and game_ui, the docs and the tests.

### What was built
- **`assets/data/progression.ron` (new)**, read by `sim_military::Progression`:
  - `start`: `monument_radius` 160, `min_monuments` 2, `max_monuments` 3, `min_settlement_distance` 40, `edge_distance` 8, `jitter` 120, and `terrain_weights` for Coast, ForestEdge and Hills.
  - 4 `bands`, each with `groups` (`count`, `unit`, `armed`, `tools`) and `food_days`.
  - `champion_traits` and 7 `traits` (`id`, `name`, `phrase`, `effects: [(Stat, f32)]`).
  - 11 `boons` (`id`, `name`, `text`, `when: Start|Dominion`, `weight`, `effects`).
  - `modifiers_per_run: (1, 2)` and 7 `modifiers` (`id`, `name`, `text`, `group: Option<String>`, `weight`, `effects`).
  - A card's `id` must equal its index.
- **`crates/sim_military/src/progression/mod.rs`**
  - Data types: `ProgressionData`, `StartData`, `BandData` (`followers()`), `BandGroup`, `TraitData`, `BoonData`, `BoonWhen {Start, Dominion}`, `ModifierData`.
  - `Effect {Stat(Stat, f32), FoodDays(f32), ArmBand, Goodwill(f32), Stores(f32), RaiderPace(f32), RivalStrength(f32), Sellswords(u32)}`, append only.
  - `Progression {data}`: a resource hashed as its RON text. It has `from_ron`, `default_data`, `validate`, `boon(card)`, `trait_data(id)` and `modifier(card)`. `DEFAULT_PROGRESSION_RON`.
  - `Champion`, a hashed marker component on the person.
  - `Traits(Vec<u16>)`, a hashed component.
  - `RunProgress`, a hashed resource that is always present: `start`, `terrain`, `start_region`, `rival_seat: Option<u64>`, `seat_center`, `rival_region`, `band`, `modifiers`, `boons`, `pending_goodwill: Vec<(u64, f32)>` and `succession_used`.
  - `RunStart {champion, start, offer}`.
  - `pub fn setup_run(world) -> Option<RunStart>`, with span `sim_military::setup_run`:
    1. Builds the start from `RunWorld::from_world` and `draw_start`, and inserts RunProgress.
    2. Calls `Factions.set_seat(PLAYER, start)` **before** `spawn_army`.
    3. Spawns the band with the first group's stats, then `outfit_groups` re-stats and re-equips the followers group by group, in SpawnId order.
    4. Inserts `(Champion, Traits([trait]))` on the captain.
    5. Applies the `Stores` modifiers (`spoil_stores`: every storehouse, in settlement SpawnId order, `floor(q × f)` of each food, via `Stockpile::take`).
    6. Calls `post_offer(PLAYER, StartBoon, choices, None)`, with a `debug_assert` that this is offer 0.
    7. Writes `RunStarted` (`subject` champion, `other` seat, `at` start, `amount` band).
  - `pub fn champion_of(world, faction) -> Option<Entity>`.
  - MilitaryPlugin inserts Progression and inits RunProgress. It registers Progression, RunProgress, Champion and Traits as hashed.
- **`progression/start.rs`**
  - `StartTerrain {Coast, ForestEdge, Hills, Open (default)}`, `#[repr(u8)]`, serde, with `name()`.
  - `SiteInfo {id, center, region, people}`.
  - `RunWorld<'a> {map, costs: &TravelCosts, settlements, monuments}`, built by `from_generated(&GeneratedWorld, &costs)` or `from_world(&World, &costs)`. Ids are SpawnIds: settlements `0..n`, then monuments.
  - `StartDraws {seat, seat_center, rival_region, start_region, start, terrain, nearby_monuments, band, champion_trait, modifiers, boon_choices: [u16; 3]}`.
  - `pub fn draw_start(SimSeed, &Progression, &RunWorld) -> Option<StartDraws>`, pure, with span `sim_military::draw_start`.
  - `pub(crate) pick_weighted` and `draw_cards(rng, &[(card, weight, group)], n)`: weighted, without replacement, and a group excludes its siblings. Reuse them for dominion and trait offers.
- **`progression/offers.rs`**
  - `pub fn choose(world, issuer, offer, choice) -> Result<(), &'static str>`. Refusals: "no run", "no such offer" (unknown or already taken), "not your offer", "no such choice", and "not available yet" (Invitation and Hire).
  - Boon effects:
    - `FoodDays`: grain for the whole army into the champion's baggage; capacity grows if needed.
    - `ArmBand`: every member gets a weapon.
    - `Goodwill`: pushed to `RunProgress.pending_goodwill` for the populated settlement nearest the champion.
    - `Stat`: recorded only.
  - The picked card goes to `RunProgress.boons`.
  - Trait offers go to `gain_trait`, which pushes to `Traits` and emits `TraitGained`.
  - Every pick emits `OfferChosen` (`subject` offer, `other` card, `amount` choice, cause Player) and a MilitaryLog line.
  - `pub fn abandon(world, issuer)`. Refusals: "no run", "not your run" (issuer is not PLAYER), "the run is over". On success: `RunState::finish(Abandoned, Cause::Player)`.
- **`orders.rs`**: `apply` routes `ChooseOffer` to `progression::choose` and `AbandonRun` to `progression::abandon`. `log` is `pub(crate)`.
- **`spawn.rs`**
  - `ArmySpec` gains `soldier: Option<UnitStats>` and `tools: bool`, and stays Copy. Its fields have docs.
  - `stats` is renamed `pub(crate) soldier_stats`.
  - `setup_battle` writes `SimCommand::ai` (G1).
  - FactionId literals are replaced by the constants.
- **`raiders.rs`**
  - `think` runs in `SimInputSet::Ai` and takes `Res<RunState>`.
  - In a run, its prey is only PLAYER captains. It already had `Without<Raider>`; `Guard` comes in step 4.
- **sim_agents `population.rs`**
  - Size mode: if any settlement has a `SettlementSize`, every settlement with a size > 0 is populated at that size, in SpawnId order.
  - Otherwise the M2 path runs, through `apportion_by_farmland` (factored out, byte-identical).
  - `PopulationReport.regions: Vec<RegionId>` replaces `region`; nothing else read it.
- **army.ron**: factions 3 "Rival" and 4 "Beasts" are appended.
- **sim_headless lib.rs**
  - `SimPlugins.run: Option<RunConfig>` and `SimPlugins::run(seed, cfg)`, which uses the run preset with population None.
  - `Scenario::Run(RunConfig)`: `name()` gives "run", `parse("run")` gives `Run(default)`. New helpers `run_config()` and `preset()`.
  - `ScenarioSetup {Nothing, Skirmish(Skirmish), Battle([Entity; 2]), Run(RunStart)}`.
  - `pub fn set_up_scenario(&mut World, Scenario) -> ScenarioSetup`.
  - `HeadlessConfig::run(seed, cfg)` (world_size 1024), `preset()`, and `worldgen()` built from `preset()`.
  - `headless_app` passes `population: None` and `run` for a run.
  - `record_replay` writes `run: scenario.run_config()`.
  - `verify_replay` uses `replay.run` for "run" and **panics** on unknown scenario names.
  - `sample_orders`: the command draw is now `0..12`; 10 is `ChooseOffer{offer 0..4, choice 0..4}` and 11 is `AbandonRun`, issued by RAIDERS so it is always refused. The issuer is otherwise `FactionId::PLAYER`.
  - New `start_offer(&App) -> Option<u64>` and `start_boon_pick(&App, choice) -> Option<LoggedCommand>`, the latter at tick 0.
  - Unit tests: `scenario_names_round_trip`, `a_run_config_selects_the_run_island`, `an_unknown_replay_scenario_is_an_error`.
- **sim_headless main.rs**
  - `--run` and `--difficulty <steady|chaotic|harsh>`; `--difficulty` without `--run` is an error.
  - `--world-size` defaults to the scenario preset's size.
  - With `--run`, commands are `start_boon_pick(choice 0)` at tick 0 plus `sample_orders`.
  - `print_run_summary` prints: regions, monuments by kind, villagers, size tiers with people, the seat (name, id, people, region, centre), and the start (tile, region, terrain, band, food days, champion traits, run modifiers, pending offers with card names).
  - `--map-ppm` marks monuments in white, the start in cyan and the seat in magenta.
  - The final line shows the run difficulty instead of population in a run.
- **game_app**: `Start::Run(RunConfig)` comes from `--run [--difficulty ..]`, with `SimPlugins::run` for it. `set_up_start` now goes through `set_up_scenario`; the duplicated match is gone. Run is not the default.
- **game_ui**: the `army::PLAYER` const is removed; it uses `FactionId::PLAYER`.
- **sim_core `events.rs` docs**: RunStarted (`other` = seat, `amount` = band) and OfferChosen (`other` = card).
- **sim_military unit tests**:
  - `shipped_data_is_valid` and `rejects_bad_data`
  - `weighted_draws_skip_zero_weights_and_groups` and `dilate_grows_a_square`
  - `a_run_start_is_seeded_and_fits` (seed 42, from_generated)
  - `a_run_is_set_up_its_boon_picked_once_and_abandoned`: a full App built from the plugins, about 1 s.
  - `survey_twenty_seeds` (`#[ignore]`; `SURVEY_SEEDS=42,7` picks seeds).
- **Docs**
  - DATA.md: the new `## progression.ron (\`sim_military::Progression\`)` section, every field. The army.ron factions row was already updated.
  - GLOSSARY: Band template, Boon, Champion, Run modifier, RunProgress, Start spot, Trait, and an updated Scenario.
  - TESTING: sim_military and sim_headless unit bullets.
  - ARCHITECTURE: sim_military, sim_headless and game_app rows; schedule row 1 now has raiders in Ai.
  - HOWTO: the "Add a scenario test" step 1 mentions `HeadlessConfig::run` and `start_offer`.
  - DETERMINISM: the rule that setup never writes player commands, and a `--run` replay example.

### Deviations from the guide (and why)
1. **The start region falls back across regions.** Non-rival regions are ranked by centroid distance from the seat, farthest first (ties: lowest id). The start is in the first region whose best spot has `min..=max` monuments within `monument_radius`; if none has one, the farthest region is used.
   - Why: on seed 42 the farthest region (r3) is small, and every candidate there sees 4-5 monuments. The guide's "relax the monument count" gave 5.
   - Seeds 1..=20, 42, 7, 1234 and 2026 all now get 2-3 nearby monuments. Seed 42 starts in r1, 485 tiles from the seat.
2. **Candidates are ranked by class instead of hard filters.**
   - Hard filters: Grass, Beach, Forest or Hills (so never Road, Farmland, Mountain or Water); walkable; reachable on foot (8-connected) from the seat; at least `min_settlement_distance` (40) from every centre; in the region.
   - Class, lexicographic (lower is better):
     1. the monument count: in range, then above max, then 1, then 0
     2. `far`: `dot(p − c, seat − c) < 0`
     3. the drawn kind
     4. near the region edge: within `edge_distance` (8) of an `is_region_edge` tile
   - Within the best class: the highest `distance to seat + jitter × U(0,1)`, ties going to the lowest tile index.
   - The reported `terrain` is the drawn kind if the tile has it, else its first kind, else `Open`.
3. **New data fields** not in the guide: `start.edge_distance`, `start.jitter`, `modifiers[].text` and `modifiers[].group` ("harvest" and "rival" groups keep lean year and bumper harvest apart, and stronger and weaker rival).
   - The `Effect` variants are those listed above. The guide's `CaptainSlots(u8)` is written as `Stat(CaptainSlots, 1.0)` in the "Seat of power" Dominion boon.
   - Step 5 may add a dedicated effect if a flat slot count is easier; append it, never reorder.
4. **RNG streams:**
   - The champion's starting trait uses `sim_military::progression::traits ^ 0` at tick 0: the champion has no SpawnId when the pure draw runs.
   - The start boons use `boons ^ 0`, because the start offer is always offer 0 (debug_assert).
   - Later trait offers must use `traits ^ person_spawn_id` at the posting tick, and dominion offers `boons ^ offer_id`.
5. **`RunWorld` borrows `TravelCosts`**, which is needed for reachability. It also has a lifetime, and `from_world` takes `&World`.
6. **`StartDraws` also carries** `seat_center` and `nearby_monuments` (SpawnIds).
7. **`Goodwill` start boon:** the guide says "pushed to the ledger in step 3". For now it goes to `RunProgress.pending_goodwill` as `(settlement SpawnId, amount)`, for the settlement nearest the champion at pick time.
8. **Run modifiers** (`RunProgress.modifiers`): only `Stores` acts at setup. The `Stat` effects (world row), `RaiderPace`, `RivalStrength` and `Sellswords` are inert until steps 4 and 7.
9. **`sample_orders`' AbandonRun is issued by RAIDERS**, so it is always refused ("not your run", or "no run" in skirmish). A random player abandon would end runs early and shrink hash coverage and replay length (G16). The success path is covered by the sim_military unit test; part 1d's `abandoning_ends_the_run_and_stops_the_clock` should cover it in tests/.
10. **`set_up_scenario` returns `ScenarioSetup`**, so game_app can frame the camera. Stress mode computes `per_side` in game_app, then calls `Scenario::Battle(per_side)`.
11. **The populate size mode skips settlements without a size** (or with size 0), rather than populating "all". In the run preset every settlement has one.
12. **`choose` refuses Invitation and Hire offers** with "not available yet" until steps 3 and 7 implement them.

### Names later parts must use
- **sim_military root:** `Progression`, `RunProgress`, `RunStart`, `RunWorld`, `StartDraws`, `StartTerrain`, `Champion`, `Traits`, `champion_of`, `draw_start`, `setup_run`.
- **`sim_military::progression::`:** `choose`, `abandon`, `Effect`, `BoonWhen`, `BoonData`, `TraitData`, `ModifierData`, `BandData`, `BandGroup`, `StartData`, `ProgressionData`, `SiteInfo`, `DEFAULT_PROGRESSION_RON`.
- **`progression::start::{pick_weighted, draw_cards}`** (pub(crate)), for later offer draws.
- **Data accessors:** `Progression::boon(card)`, `trait_data(id)` and `modifier(card)`.
- **sim_headless:** `Scenario::Run(RunConfig)`, `Scenario::{run_config, preset}`, `ScenarioSetup`, `set_up_scenario`, `HeadlessConfig::run(seed, cfg)`, `HeadlessConfig::preset`, `SimPlugins::run(seed, cfg)`, `start_offer(&App)`, `start_boon_pick(&App, choice)`.
- **Test helpers (part 1d):**
  - `run_app(seed)` = `headless_app(HeadlessConfig::run(seed, RunConfig::default()))`
  - `start_offer(app)` = `sim_headless::start_offer(&app).unwrap()`
  - `champion(app)` = `sim_military::champion_of(world, FactionId::PLAYER)`
- **World-level 20-seed test:**
  1. `generate_world(seed, &WorldGenSettings::preset(WorldPreset::Run), default_threads())`
  2. `RunWorld::from_generated(&g, &costs)`, with `costs = sim_agents::Villagers::default_data(&Calendar::default_data()).travel_costs`
  3. `draw_start(SimSeed(seed), &Progression::default_data(), &w)`
- **SpawnIds in a run:** settlements `0..25`, then monuments `25..49`, then villagers, then the band. The champion is the first SpawnId after the villagers.
- **The seat** is `RunProgress.rival_seat` (`Some` only in a run).

### Measured
- **Seeds 1..=20 (survey):**
  - every start has 2-3 nearby monuments
  - start terrain: coast 12, forest edge 5, open ground 2, hills 1
  - bands: all 4 templates appear; modifier sets vary
  - the start-to-seat distance is 458-943 tiles
  - `draw_start` takes 60-120 ms at opt 3
- **`sim_headless --run --seed 42 --ticks 2000`** (release):
  - 25 settlements, 4 regions, 24 monuments (7 kinds), 4,557 villagers
  - 5 hamlets, 11 villages, 9 towns; the seat is Dunmoor (#19, 450, region 2)
  - start (134, 522) in r1 (coast), band "Outlaws", trait Brave, modifiers Weaker rival and Restless roads, boons on offer Hardy / Well fed / Woodwise
  - setup 0.4-0.5 s; about 1 ms per tick (960 ticks/s)
- **Seed 7** gives 4,239 villagers.
- **Developed invariance:** `--seed 42 --ticks 1200 --stats-every 600` stdout is identical to the pre-M4 `old_s.out`, and the never-ate lines match apart from Entity bits. stderr gains the line `regions=10 monuments=0 villagers=5000`.

### Open issues
- **Not written here (part 1d / tests):**
  - `tests/tests/run_start.rs` (`twenty_seeds_draw_valid_varied_starts`, `a_run_starts_identically_from_its_seed`, `choosing_the_start_boon_applies_once`, `abandoning_ends_the_run_and_stops_the_clock`)
  - the `determinism.rs` additions (`run_state_hash_covers_every_sim_component`, `a_run_replays`)
  - `tests/tests/common` run helpers
  - ADR 0013
- **Start terrain is coast-heavy** (12/20). If the playtest wants more variety, raise ForestEdge/Hills `terrain_weights`, or move the kind ahead of `far` in `Spot::rank`. That changes draws, which is fine (nothing is golden).
- **Skirmish and battle hashes changed:**
  - `raiders::think` now runs in Ai, after scripted input.
  - `setup_battle`'s orders are AI-origin, so `--battle` replays no longer log those two orders.
  - The sample_orders stream changed (range 0..12).
  - The M3 tests (orders, posture, army_march, starving_army, determinism) all pass.
- **In a run, raiders raid any village**, including future rival lands; step 7 may restrict that.
- **Boons and modifiers waiting on later steps:** `Stat` boons (Hardy, Woodwise, Silver tongue) are recorded but have no effect until step 4's `rebuild_modifiers` folds in `RunProgress.boons`. Step 3 must drain `RunProgress.pending_goodwill` into `GoodwillLedger`.
- **In `game_app --run` the start boon waits:** nothing picks it until the new-game screen arrives in step 10. The sim runs fine meanwhile.
- **CLAUDE.md's command list** could gain `sim_headless --run [--difficulty ..]` (step 12).
- **Checks run (all pass):**
  - `cargo check -j 2 --workspace --all-targets`; `cargo clippy -j 2 --workspace --all-targets -- -D warnings`; `cargo fmt --check`; `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps`
  - `cargo test -j 2 -p sim_core -p sim_world -p sim_agents -p sim_economy -p sim_military -p sim_headless` (32/30/7/10/10 + 1 ignored/3)
  - integration: determinism 9/9 (103 s), orders 7/7, posture 1/1, docs 4/4, army_march, starving_army, village_day, famine, worldgen 7/7
  - release `--run --seed 42 --ticks 2000 --checkpoint-every 500`, plus `--record` and `--replay` (multi and `--single-threaded`): ok, with identical hashes 0x102570e0ba0b96e3. Also with `--commands 200`, and with `--difficulty harsh`.

## Step 1 part 1d-tests-docs

Only tests and docs changed; no `crates/` file was touched.

### What was built
- **`tests/tests/run_start.rs` (new)**, the M4 step 1 criterion test (GP-21, GP-23, GP-27). Four tests; about 11.5 s for the file with `--test-threads=2`.
  - `twenty_seeds_draw_valid_varied_starts` (~10 s): world level, with no App. For seeds 1..=20, in sequence, it calls `generate_world(seed, &preset(Run), default_threads())`, `RunWorld::from_generated` and `draw_start`. It asserts:
    - region_count 4, and 4 distinct settlement regions
    - every settlement has a size with people > 0
    - 4-8 monuments per region, none on Water, Road or Mountain, and each ≥ `monuments.min_settlement_distance` from every centre
    - `draw_start` drawn twice is equal
    - the seat is the unique largest, and `rival_region` is the seat's region
    - the start region differs from the rival's
    - the start tile is Grass, Beach, Forest or Hills, in the start region, and ≥ `start.min_settlement_distance` from every centre
    - recounted monuments within `monument_radius` equal `nearby_monuments`, and number 2-3
    - `monument_radius` is no more than a day's march (grass speed × ticks_per_day = 312)
    - `find_path(start → seat_center)` is Some
    - three distinct boons, all `when: Start`
    - the band index and champion trait are valid, the modifier count is within `modifiers_per_run`, and no two modifiers share a group
    - across seeds, the BTreeSets of terrain, band and sorted modifier set each have ≥ 3 entries (observed 4, 4 and 14)

    It prints one line per seed.
  - `a_run_starts_identically_from_its_seed` (~2 s, seed 7): two `run_app`s. It asserts:
    - equal hashes at setup and after 200 ticks, and the hash changes
    - Settlement count within the run preset's min..=max, all of them populated (`villages()`)
    - `RunState` config is Some with no outcome; `rival_seat` is Some and populated
    - exactly one pending offer: the player's StartBoon, equal to `start_offer`, with 3 distinct cards
    - the champion has `Captain.faction == PLAYER`, one trait, and a follower count equal to the band template's `followers()`
    - the first chronicle entry is RunStarted at tick 0
  - `choosing_the_start_boon_applies_once` (<1 s, seed 42; the offer is Hardy / Well fed / Woodwise):
    - It picks the most observable choice: FoodDays, then Goodwill, then ArmBand, then anything.
    - After the pick: `RunProgress.boons == [card]`, offers are empty, and nothing is refused. Per effect, the baggage food value gain is ≥ `hunger_per_day × members × days`, `pending_goodwill == [g]`, or every member is armed.
    - The second ChooseOffer is refused: `rejected_commands` +1, and `RunProgress`, the baggage food and the armed count are unchanged.
    - The chronicle has exactly one OfferChosen, with (PLAYER, offer, card, choice, Cause::Player).
  - `abandoning_ends_the_run_and_stops_the_clock` (<1 s, seed 42):
    - A RAIDERS AbandonRun is refused (rejected +1, no outcome).
    - On the player's AbandonRun at tick T: outcome Abandoned, cause Player, `ended_at == T`, pending None. The last chronicle entry is RunEnded at T, with subject `Abandoned as u64` and cause Player. SimTick is T+1.
    - Then a ChooseOffer and another AbandonRun are sent, plus 50 more steps: SimTick and the hash are frozen, the start offer is still pending, and the chronicle is unchanged.
- **`tests/tests/common/mod.rs`**: new helpers, documented in its module doc:
  - `run_app(seed) -> App`
  - `start_offer(&App) -> u64` (panics if none)
  - `choose(&mut App, offer, choice)` (a player ChooseOffer message)
  - `champion(&mut App) -> Entity`
  - `chronicle(&App) -> Vec<ChronicleEntry>`
  - `rejected(&App, FactionId) -> u32`
- **`tests/tests/determinism.rs`**:
  - `run_state_hash_covers_every_sim_component` (~3 s):
    - `HeadlessConfig::run(42, default)`, with `start_boon_pick(choice 0)` at tick 0 plus `sample_orders(42, 600, 30)` as a CommandScript.
    - Steps 1,500 ticks. Asserts SimTick 1,500, no outcome, one boon picked, and `unhashed_state` empty.
  - `a_run_replays` (~11 s):
    - Seed 9, `Difficulty::Harsh` (a wrong config would diverge through the RunState hash), 2,000 ticks, checkpoints every 500.
    - The commands are a probe app's `start_boon_pick(1)` plus `sample_orders(9, 2000, 60)`.
    - Asserts `scenario == "run"`, `run == Some(cfg)` and `replay.commands == commands`; the RON round trip is equal.
    - `verify_replay` passes both Multi and Single.
- **Docs**
  - **TESTING.md**: rows for the four run_start tests and the two determinism tests, with timings.
  - **DETERMINISM.md**:
    - A new rule row: "Run-only rules leave the developed world alone" (in_run gate, new stream keys, no entities before villagers outside the run preset, no-op multipliers), linking ADR 0013.
    - The harness bullet now mentions run coverage and run replays.
  - **GLOSSARY.md**:
    - New rows: **Rival** (rival warlord) and **Run gate**.
    - **Seat** now names `RunProgress.rival_seat`.
    - The other terms the guide lists were already present from 1a-1c: Run, RunConfig, Preset, Offer, Boon, Run modifier, Monument, Start spot, Size tier, SimEvent and Chronicle.
  - **ARCHITECTURE.md**:
    - "From a click to a frame" uses `FactionId::PLAYER`.
    - "Where state lives" gains a "Run state (M4)" bullet (always present and hashed; EventFeed exempt; links ADR 0013) and lists `Progression` among the data resources.
    - The schedule and crate tables were already complete from 1a-1c.
  - **HOWTO.md**:
    - "Add a scenario test" now points at `common::run_app`, `common::choose` and `common::start_offer`, and at `tests/tests/run_start.rs`.
    - A new recipe, **"Add a run-only rule"**, in 7 steps: data in progression.ron, an `in_run` gate, new stream keys, no-op multipliers, offers and `RunState::finish`, testing with `run_app`/`chronicle` and extending the run coverage test, and showing the developed world didn't change.
  - **DATA.md**: no change was needed. Worldgen presets, monuments and sizes, the progression.ron section and the army.ron factions were already complete, and `every_data_field_is_documented` passes.
  - **`docs/adr/0013-runs-presets-and-run-config.md` (new)**, in ADR 0012's style. It records:
    - the run as seed + RunConfig in the replay header (format 5), with Difficulty selecting data, not streams
    - RunState and the other run resources always present and hashed
    - the run gate and `settle_outcome`
    - two presets in one worldgen.ron, with `default_data` = developed
    - apportioned size tiers and the seat
    - monument placement moved into step 1, beyond the threat radius
    - the pure `draw_start` with its algorithm and streams
    - the shared, command-free `set_up_scenario` and the G1 fix
    - Offers in sim_core, with picks and abandons as recorded commands
    - `SimInputSet::Ai`
    - the developed-invariance rules
    - consequences: developed output identical, skirmish and battle hash changes, format-4 replays refused, costs, coast-heavy starts, shorter logs for ended runs, the tests, and events landing early with their ADR in step 9

### Deviations from the guide (and why)
1. **Helper signatures.**
   - `champion` takes `&mut App`, because `champion_of` needs `&mut World`.
   - `start_offer` returns `u64` and panics when there is no offer, which suits tests.
   - The extra helper `rejected(app, faction)` was added for refusal checks; steps 5 and 6 will reuse it.
   - Not added yet, as nothing needs them: `owner_of` (step 2), `step_days` and `step_until`.
2. **The choosing test is generic over the offered card,** rather than hard-coding "Well fed". It checks the food gain as "at least the band's hunger for `days` days" in food value, not as an exact unit count, so it doesn't copy `add_food_days`' rounding. "Applied once" is shown by equality after the refused second pick.
3. **The twenty-seeds test checks more than the guide lists** (the recounted nearby monuments, the day's-march bound, the start tile's ground and distance, the modifier groups, the repeat draw), because these are the M4_PLAN criterion's own words.
4. **ADR 0013 also records the 1b decisions** (apportioned sizes, anchor-based monuments with data site rules) and the G1 fix. These are the run-foundation decisions, and no other ADR in D3 covers them. The events infrastructure is only mentioned; its ADR is still 0017 (step 9).
5. **There is no separate "Rival seat" glossary row:** the existing "Seat (rival's seat)" row covers it, and a "Rival" row now points to it.

### Names later parts must use
- **Test helpers** (`tests/tests/common/mod.rs`): `run_app`, `start_offer`, `choose`, `champion`, `chronicle`, `rejected`, plus the M3 ones (`command`, `order`, `villages`, ...).
- **Extend `run_state_hash_covers_every_sim_component`** (determinism.rs) in every later step, so that its state exists in that run: for example, conquer something, hold a monument, or post a request before step 1,500.
- **Test file:** `tests/tests/run_start.rs`. Its TESTING.md rows sit between `worldgen.rs` and `village_day.rs`.
- **HOWTO recipe "Add a run-only rule":** keep it current when steps 2-7 change the pattern (for example, `GoodwillLedger` in step 3, or `rebuild_modifiers` in step 4).
- **ADRs:** 0013 is written. 0014 (ownership, goodwill, loyalty), 0015 (monuments, FactionModifiers), 0016 (save format) and 0017 (events and chronicle) remain.

### Open issues
- **The SEED.md M4 criteria are not ticked,** and the `CLAUDE.md` milestone line and command list (`sim_headless --run [--difficulty ..]`) are not updated. Both belong to step 12 or the orchestrator.
- **determinism.rs as a whole takes ~104 s** with `--test-threads=2`, as before (the M3 `armies_with_orders...` alone takes over 60 s). The two new tests add about 14 s.
- **Start terrain stays coast-heavy:** across seeds 1-20, coast 12, forest edge 5, open 2, hills 1. The variety assertion needs ≥ 3 and sees 4; retuning `terrain_weights` or the ranking may move this, so re-run run_start.
- **Checks run (all pass):**
  - `cargo fmt --check`
  - `cargo clippy -j 2 -p integration_tests --all-targets -- -D warnings`
  - `cargo test -j 2 -p integration_tests --test run_start --test determinism --test docs -- --test-threads=2`: run_start 4/4 (11.5 s), determinism 11/11 (104 s), docs 4/4

  No sim crate changed, so the crate unit tests and rustdoc were not re-run. The full `cargo test --workspace` was not run; the other integration test files compile under clippy all-targets.

### Gate
Step 1 definition-of-done gate, 2026-10-07, on the uncommitted working tree. **Green.** No code or docs changed; nothing needed fixing.
- `cargo fmt` changed nothing, and `cargo fmt --check` passes.
- `cargo clippy -j 2 --workspace --all-targets -- -D warnings`: clean.
- `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps`: clean.
- `cargo test -j 2 --workspace -- --test-threads=2`: 36 test binaries, **148 passed, 0 failed, 1 ignored** (`survey_twenty_seeds`).
  - Integration: determinism 11/11 (112 s), run_start 4/4 (12 s), worldgen 7/7, orders 7/7, docs 4/4, army_march 1, posture 1, starving_army 1, village_day 1, famine 1.
  - Unit: sim_core 32, sim_world 30, sim_military 10 (+1 ignored), sim_economy 10, sim_agents 7, sim_headless 3, game_render 12, game_ui 6.
  - The guarded tuning tests (village_day, famine, army_march, posture, starving_army, orders) all pass unchanged.
- Release `sim_headless --run --seed 42 --ticks 3000 --checkpoint-every 1000`: exit 0, 752 ticks/s. Checkpoints 1000 `0x5207cec5bb8c9194` and 2000 `0x102570e0ba0b96e3` (the same as part 1c's tick-2000 hash). Final `0x9341cba8acab56ea`.
- Run replay smoke (`--run --ticks 3000 --commands 50 --record gate.ron`): 51 commands (the boon pick at tick 0 plus 50), final `0xea826f9eff450f08`. `--replay` and `--replay --single-threaded` both report "replay ok", with 3 checkpoints and the same hash.
- Extra developed-world replay smoke (no `--run`, `gate_dev.ron`, 50 commands): record, replay and single-threaded replay all reproduce `0xafc8496b1d1d578f`.
- Housekeeping: free disk was 3.2 GB before the release step, so I removed `target/debug/incremental`, which left 6.7 GB free. The next debug build is a non-incremental rebuild.

### Fixes
Review findings on step 1, applied 2026-10-07 on top of commit `b14ea53` (working tree only, nothing committed). All 12 confirmed findings were fixed; none declined.
- **DATA.md guards (minor).**
  - The worldgen.ron header now also names `run_start` for the `run` preset.
  - progression.ron's "Guarded by" names `run_start` (`twenty_seeds_draw_valid_varied_starts`; re-run it after changing `start.*`, `bands`, `modifiers` or weights) and determinism's `run_state_hash_covers_every_sim_component` and `a_run_replays`.
- **threat.radius copied as 45 (nit).**
  - tests/tests/worldgen.rs `run_preset_meets_its_criteria` reads `Army::default_data(..).data.threat.radius`, asserts `monuments.min_settlement_distance > radius`, and passes it to `check_monuments` (replacing `d > 45.0`).
  - sim_world's `run_preset_is_valid` drops the literal; a comment points at the integration check.
  - DATA.md army.ron `threat.radius` and worldgen `monuments.min_settlement_distance` rows name the coupling and the test.
  - **Step 4 note:** the monument tests must check the effective radius (`threat.radius` plus the largest ThreatRadius modifier), or assert that a guarded monument leaves `Threat.level` at 0 at every settlement.
- **`--population` with `--run` (nit).**
  - `Args.population` is now `Option<u32>`, and `parse_args` refuses it with `--run` ("--population has no effect with --run (settlements use their drawn sizes)"), mirroring `--difficulty`. Release check: exit 1 with that message.
  - The USAGE line and DATA.md `population.*` rows say runs ignore them; the villagers.ron comment says so too.
  - `Replay.population` and REPLAY_FORMAT are unchanged. The developed replay header still says `population: 5000`, and the developed hash is unchanged.
- **ADR 0013 monument spacing (nit).** It now says monuments are `min_spacing` (40) apart where possible, relaxed to a half and then a quarter for a slot nothing fits, and that step 4 must use actual distances, not assume 40. DATA.md's `min_spacing` row says the same. `twenty_seeds_draw_valid_varied_starts` now asserts full `min_spacing` between every pair over seeds 1-20, which backs the ADR's "none of seeds 1-20 needed it".
- **Start label vs tile (nit).** `twenty_seeds_draw_valid_varied_starts` re-derives coast (beach or water in a 5x5 square), forest edge and hills from the map at `a.start` and asserts the label is one the tile has (Open = none). New sim_military unit test `a_spot_is_labelled_by_a_kind_it_has` covers `Spot::terrain` (drawn kind wins, else first in ALL order, else Open).
- **Boon effects only FoodDays (minor) and the FoodDays upper bound (nit).**
  - `choosing_the_start_boon_applies_once` now finds the Start cards with FoodDays, ArmBand and Goodwill by effect. For each it runs `pick_once` on a fresh `run_app(42)`, putting the card first in the start offer before tick 0 (a test-only edit).
  - It asserts at least one effect was checked:
    - FoodDays: the gain is `>= need` and `< need + food_value(best food)`.
    - ArmBand: armed goes from 3 to 11 of 11 at seed 42.
    - Goodwill: `pending_goodwill == [(nearest populated settlement to the champion, ties lowest SpawnId), g]`.
  - Each pass keeps the double-click, unchanged-state and single-OfferChosen checks.
  - The unit test's food check is now "`0 < gain <= ceil(need / value)` units".
- **Band make-up (minor).**
  - The sim_military unit test asserts that seed 42 draws a mixed band, and checks every follower in SpawnId order against its group's `soldier_stats(unit)` and `Equipment{armed, tools}`. It also checks the champion's captain stats and first-group gear.
  - The integration helper `assert_band_make_up` does the same for seed 7 in `a_run_starts_identically_from_its_seed`.
- **"Rats in the stores" (minor).**
  - setup_run's modifier loop moved into `apply_start_modifiers(world, &Progression, &[u16])` (private, behaviour identical: the seed-42 run hashes did not change).
  - The unit test finds the Stores card by effect and calls `apply_start_modifiers(.., &[rats])`. It asserts each storehouse food is `q - floor(q * fraction)`, non-food goods and the first home's stockpile are unchanged, and something was lost.
- **App start vs pure draw (minor).** `a_run_starts_identically_from_its_seed` regenerates seed 7, calls `draw_start(from_generated)` and asserts:
  - RunProgress (start, terrain, regions, seat, centre, band, modifiers) equals it;
  - `Factions.seat(PLAYER) == start`;
  - the offer's choices equal `boon_choices`;
  - Traits are `[champion_trait]`;
  - RunStarted has `(other, at, amount) == (seat, start, band)`.
- **G1 regression (minor).** New `determinism.rs::a_battle_replays_without_logging_setup_orders`: `Scenario::Battle(20)`, population 1,500, 200 ticks. It asserts `replay.commands` is empty and that `verify_replay` passes both Multi and Single (~5 s).
- **Monument site rules (minor).** New sim_world unit tests in worldgen/monuments.rs:
  - `placed_monuments_suit_their_kind`: the run islands of seeds 42 and 7. It rebuilds water, road and mountain distances from the final map and asserts `allows(rule)` and not Farmland for every monument.
  - `a_kind_that_fits_nowhere_falls_back_to_one_that_does`: a synthetic 48x48 grass map where Watchtower (Hills) fits nowhere, so every slot records StandingStones.
  - `check_monuments`' doc points at these tests.
- **Assert messages (nit).** Every assert the finding listed now names the seed and prints its values.
- **Docs.** TESTING.md has the new battle row (~5 s), new descriptions for the run_start rows (identical start ~3 s, choosing ~2 s), new sim_world unit bullets, and updated sim_military bullets. ADR 0013 and DATA.md are as above.

### Gate
Gate run after the fixes above, 2026-10-07, on the uncommitted working tree on top of `b14ea53`. **Green.**
- `cargo fmt`: it reformatted only the new test code. `cargo fmt --check` then passed.
- `cargo clippy -j 2 --workspace --all-targets -- -D warnings`: clean.
- `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps`: clean.
- `cargo test -j 2 --workspace -- --test-threads=2`: 36 test binaries, **152 passed, 0 failed, 1 ignored** (`survey_twenty_seeds`). That is 148 plus the 4 new tests.
  - Integration: determinism 12/12 (117 s), run_start 4/4 (13.8 s), worldgen 7/7, orders 7/7, docs 4/4, army_march 1, posture 1, starving_army 1, village_day 1, famine 1.
  - Unit: sim_core 32, sim_world 32, sim_military 11 (+1 ignored), sim_economy 10, sim_agents 7, sim_headless 3, game_render 12, game_ui 6.
  - The guarded tuning tests (village_day, famine, army_march, posture, starving_army, orders) passed unchanged. No data values changed; only a comment was added to villagers.ron.
  - Log: scratchpad/gate_test.log.
- Release `sim_headless --run --seed 42 --ticks 3000 --checkpoint-every 1000`: exit 0, 797 ticks/s. Checkpoints were 1000 `0x5207cec5bb8c9194` and 2000 `0x102570e0ba0b96e3`, final `0x9341cba8acab56ea`, identical to the previous gate.
- Run replay smoke (`--run --ticks 3000 --commands 50 --record gate.ron`): 51 commands, final `0xea826f9eff450f08`. `--replay` and `--replay --single-threaded` both printed "replay ok" with 3 checkpoints and the same hash.
- Developed replay smoke (`gate_dev.ron`, 50 commands, header population 5000): recording, replay and single-threaded replay all reproduced `0xafc8496b1d1d578f`, the same as before.
- `--run --population 2000` is refused (exit 1); `--population 300` alone still works.
- Disk: 3.3 GB free after the release build. Nothing was deleted this time; the next agent should check `df -h /` and remove `target/debug/incremental` if under 3 GB.

## Step 2 part 2a-standing-loyalty

### What was built
- **`crates/sim_agents/src/standing.rs` (new)** (GP-05, GP-06, GP-22)
  - `Owner {faction: FactionId, since: u64}`: a component, Copy + Eq. `Default` is `(INDEPENDENT, 0)`. StateHash: u16 faction, then u64 since.
  - `Goodwill`: a component with a **private** `by_faction: Vec<(FactionId, f32)>`, sorted by faction, one entry per faction, each value in -1..1. StateHash: u32 len, then u16 + f32 per entry. API:
    - `get(f) -> f32`: 0.0 if there is no entry.
    - `add(f, delta) -> f32`: clamps and returns the new value. A zero or non-finite delta changes nothing and adds no entry.
    - `set(f, v)`: clamps; a non-finite value is ignored.
    - `entries() -> &[(FactionId, f32)]`.
    - Step 8's save code lives in sim_agents, so it can reach the private field.
  - `OwnerChange {previous: FactionId, residents: u32}`.
  - `pub fn default_seed(world, settlement, to) -> f32`: `(goodwill(to) + 1) / 2`, or `loyalty.independent_target` when `to == INDEPENDENT`.
  - `pub fn set_owner(world, settlement, to, seed_loyalty: Option<f32>) -> Option<OwnerChange>`, with span `sim_agents::set_owner`. It returns `None` and changes nothing if the settlement has no `Owner` (not populated). Otherwise it:
    1. resolves the seed: `seed_loyalty` if finite, else `default_seed`, clamped to 0..1;
    2. collects the `(SpawnId, Entity)` of every `Villager` with `v.settlement == settlement` and no `Enlisted`, sorts them, and sets `Allegiance(to)` and `Loyalty(seed)` in that order;
    3. inserts `Owner {to, since: SimTick}` (a same-owner call also restarts `since`);
    4. clears `Threat.provoked_by` (None) and `provoked_until` (0), leaving `level` and `from`;
    5. resets `SettlementLoyalty` to `{mean: seed, min: seed, residents: n, below_since: None, revolt_since: None}`, or mean and min 0 when n = 0.

    It emits no event; that is the caller's job (`conquest::transfer`).
- **`crates/sim_agents/src/loyalty.rs` (new)** (GP-06, SIM-24)
  - `Loyalty(pub f32)`: a component, 0..1.
  - `SettlementLoyalty {mean, min, residents: u32, below_since: Option<u64>, revolt_since: Option<u64>}`: a component. StateHash: f32, f32, u32, then tag + u64 for each Option.
  - `pub fn loyalty_target(&LoyaltyData, Owner, goodwill, food_days, tick, ticks_per_day) -> f32`. Owner 0 gives `independent_target`. Otherwise it is `(w_food·food + w_goodwill·treatment + w_time·time) / (w_food + w_goodwill + w_time)`, where:
    - food = `food_days / data.food_days`, clamped to 0..1;
    - treatment = `(g + 1) / 2`;
    - time = days since `owner.since` / `settle_days`, clamped to 0..1.
  - `pub fn drift_toward(loyalty, target, rate) -> f32` = `loyalty + (target − loyalty) · rate.clamp(0, 1)`, clamped to 0..1. It is exactly a no-op at the target.
  - `pub fn drift_loyalty` (system, span `sim_agents::drift_loyalty`): `par_iter_mut` over `(&Villager, &mut Loyalty), Without<Enlisted>`. It reads `Query<(&Owner, &Goodwill, &FoodSecurity)>` at `v.settlement` and `Res<FactionModifiers>`. rate = `drift_per_day × mods.mult(owner, Stat::LoyaltyDrift)`.
  - `pub fn aggregate_loyalty` (system, span `sim_agents::aggregate_loyalty`): it collects `(SpawnId, settlement, loyalty)` for villagers who aren't enlisted, sorts by SpawnId, and tallies per settlement (f64 sum, min, n) in a BTreeMap. It then writes each settlement's mean, min and residents.
    - `below_since = Some(previous or this tick)` only if `n > 0`, owner ≠ INDEPENDENT and `mean < revolt_threshold`; otherwise None.
    - `revolt_since` is carried over untouched.
- **`data.rs`**:
  - `LoyaltyData {food_days, w_food, w_goodwill, w_time, settle_days, drift_per_day, independent_target, revolt_threshold}`, with `validate()`: finite values; food_days and settle_days > 0; weights ≥ 0 and not all 0; drift_per_day, independent_target and revolt_threshold in 0..=1. It is called from `Villagers::resolve`.
  - `VillagerData.loyalty`, placed after `combat`.
- **`assets/data/villagers.ron`**: a `loyalty:` block after `combat`, with `food_days: 1.0`, `w_food: 0.55`, `w_goodwill: 0.3`, `w_time: 0.15`, `settle_days: 10.0`, `drift_per_day: 0.4`, `independent_target: 0.6` and `revolt_threshold: 0.35`.
- **`population.rs`** (all presets):
  - Every villager's second bundle tuple gains `Loyalty(independent_target)`.
  - The Allegiance literal is now `FactionId::INDEPENDENT`.
  - Each populated settlement's insert gains `Owner::default()`, `Goodwill::default()` and `SettlementLoyalty {mean = min = independent_target, residents: residents_total, ..}`.
- **`lib.rs`**:
  - `pub mod standing; pub mod loyalty;`
  - Root re-exports: `Owner`, `Goodwill`, `OwnerChange`, `set_owner`, `Loyalty` and `SettlementLoyalty`.
  - `AgentSet::Standing`, configured `.in_set(SimSet::Needs).after(AgentSet::Needs)`.
  - Systems: `(drift_loyalty.run_if(on_day), aggregate_loyalty.run_if(on_day)).chain().in_set(AgentSet::Standing)`. A comment marks the hook: step 3 puts `standing::apply_goodwill.run_if(..)` **first in that tuple**.
  - `register_hashed` for Owner, Goodwill, Loyalty and SettlementLoyalty, in that order, after Strike.
- **`crates/sim_military/src/lib.rs`**: `MilitarySet::Upkeep` gains `.after(AgentSet::Standing)` (see deviation 6).
- **Unit tests** (sim_agents now has 15, all passing):
  - standing: `goodwill_stays_sorted_and_clamped`, `set_owner_hands_over_residents_in_spawn_id_order` and `set_owner_ignores_what_is_not_a_populated_settlement`.
    - The second builds two worlds with the same SpawnIds spawned in opposite entity order and asserts identical results.
    - It also checks that enlisted and other-settlement villagers are untouched, the goodwill seed is (0.5 + 1) / 2, Owner.since is the tick, Threat is cleared, and SettlementLoyalty is reset.
    - It covers the explicit and clamped seed and the independent default seed.
  - loyalty: `the_target_weighs_food_goodwill_and_time` (it also asserts the tuning relations: starved < revolt_threshold, fed > 0.5), `drift_closes_part_of_the_gap`, `loyalty_drifts_toward_its_target_each_day` and `the_loyalty_drift_modifier_speeds_drift`.
    - The last two run the real systems with `run_system_once` in an `App` with `TaskPoolPlugin`, because par_iter needs the ComputeTaskPool when `multi_threaded` is unified in.
  - data: `rejects_bad_loyalty`.
- **Docs**:
  - DATA.md villagers.ron: a `loyalty` row and a row per field; the section header names the guard tests.
  - ARCHITECTURE: the sim_agents crate row, schedule row 6 (Standing between AgentSet::Needs and MilitarySet::Upkeep, and why), and a "Standing and loyalty (M4)" bullet in "Where state lives".
  - GLOSSARY: new Goodwill, Loyalty and Owner rows; the Allegiance row is updated.
  - TESTING: the sim_agents unit bullets.

### Deviations from the guide (and why)
1. **`set_owner` returns `Option<OwnerChange{previous, residents}>`** instead of nothing. `None` means the settlement isn't populated (no `Owner`), and nothing changed. Callers get the previous owner and the head count for events without a second query.
2. **The default seed for `to == INDEPENDENT` is `loyalty.independent_target`.** Goodwill "toward faction 0" means nothing. Elsewhere it is `(goodwill(to) + 1) / 2`, as in the guide.
3. **`set_owner` also resets `SettlementLoyalty`** (mean = min = seed, residents, below_since and revolt_since None), so the unrest clock restarts with each owner. The guide only says it sets Owner, villagers and Threat.
4. **`SettlementLoyalty` has two more fields:** `residents: u32` (empty settlements never count as "below") and `revolt_since: Option<u64>`. The guide's sim_military bullet already names `revolt_since`. sim_agents only clears it (in set_owner) and carries it over (in aggregate).
5. **Target and drift form.**
   - The target is a weighted **mean** (divided by the weight sum), so it stays in 0..1 whatever the weights; the guide said "weighted sum".
   - Drift is proportional: each day closes `drift_per_day` of the gap. The rate is clamped to 0..1 after the modifier.
   - `below_since` uses the **mean**, applies only to owned (owner ≠ 0), non-empty settlements, and keeps its first tick while the mean stays below.
6. **A schedule edge was added in sim_military:** `MilitarySet::Upkeep.after(AgentSet::Standing)`.
   - Why: `apply_desertions` (exclusive, in Upkeep) removes `Enlisted`, and `drift_loyalty` and `aggregate_loyalty` filter on `Without<Enlisted>`. Unordered, the multi-threaded executor could run them either way on a day tick, which is a determinism hazard.
   - Upkeep reads nothing Standing writes, so behaviour is unchanged (verified below).
   - D7's `MilitarySet::Progress .after(Upkeep).after(Standing)` remains valid. The order in Needs is now AgentSet::Needs → AgentSet::Standing → MilitarySet::Upkeep (→ Progress in 2b).
7. **Starting loyalty** is `independent_target` (0.6) for every villager in every preset, and `SettlementLoyalty` is filled in `populate`, not left at zero until the tick-0 aggregate. Developed villages are all independent, so their loyalty stays exactly 0.6 (`drift_toward` is a no-op at the target).
8. **Spawned soldiers** (`Soldier`, no `Villager`) get no `Loyalty`. They are not villagers, and deserters without a `Villager` are despawned. Recruited villagers keep their (frozen) Loyalty while enlisted.
9. **`drift_loyalty` and `aggregate_loyalty` are `pub`** (documented), so tests and later steps can name them; the guide didn't say.
10. **No SIM-13 courage hook**, as instructed. Nothing reads Loyalty for behaviour yet.

### Names later parts must use
- **Root:** `sim_agents::{Owner, Goodwill, OwnerChange, set_owner, Loyalty, SettlementLoyalty, AgentSet::Standing}`.
- **Modules:** `sim_agents::standing::default_seed`; `sim_agents::loyalty::{loyalty_target, drift_toward, drift_loyalty, aggregate_loyalty}`; `sim_agents::data::LoyaltyData` (`villagers.data.loyalty.revolt_threshold` etc.).
- **Transfers:** `conquest::transfer` should call `sim_agents::set_owner(world, s, to, seed)` and use `OwnerChange.previous`/`.residents` for its event.
  - For a revolt to independence, pass `None` (seeds 0.6) or an explicit seed.
  - Invitations (step 3) pass `None` to seed from goodwill.
  - Rival setup (step 7) passes `Some(rival.start_loyalty)`.
- **Revolt step (2b):** read `SettlementLoyalty.below_since` (it is a day tick) with `below_since <= tick - tpd`, and use `revolt_since` for its own bookkeeping. It runs in `MilitarySet::Progress`, after Standing, so on a day tick it sees this day's aggregate.
- **Step 3:**
  - Put `standing::apply_goodwill.run_if(ledger_nonempty)` as the first element of the Standing chain in `crates/sim_agents/src/lib.rs` (comment marks it).
  - Use `Goodwill::add` (it clamps, and ignores 0 and non-finite deltas).
  - `GoodwillLedger` and the spread go in `standing.rs`.
- **Owner-aware code in sim_military (2b):** read `Option<&Owner>` on settlements. Owner exists on every populated settlement in all presets; unpopulated developed settlements have none.

### Tuning notes (villagers.ron `loyalty`)
- At the morning day tick, `FoodSecurity.days` in fed developed villages is about 0.4-1.9 (storehouse + homes only; farm stock is excluded). With `food_days: 1.0`, most fed villages get the full food term.
- A **fresh conquest at neutral goodwill** is seeded at 0.5:
  - starved: the target is ≈ 0.15 (+0.015/day of time), so loyalty goes 0.36 (day 1), 0.28 (day 2, below 0.35, below_since set) and keeps falling. The revolt check fires on day 3.
  - fed: the target is ≈ 0.70, and loyalty rises about 0.08 on the first day.
  - with ~0.44 days of food (Millham-like): the target is ≈ 0.39, so loyalty dips but stays above 0.35.
- A **long-held starved village** has a target of 0.30, still below the threshold. Time alone never saves a starving village.
- The **2b conquest scenario** may retune `drift_per_day`/`revolt_threshold` for "revolt within 2-3 days". Re-run the sim_agents unit tests, which assert `starved < revolt_threshold` and `fed > 0.5`.

### Open issues
- **ADR 0014** (ownership, goodwill and loyalty) is not written; the conquest part (or step 12) should write it. It should record:
  - Owner, Goodwill and SettlementLoyalty on every populated settlement and Loyalty on every villager, in all presets;
  - `set_owner` as the single transfer path (SpawnId order, seeds, Threat and unrest reset);
  - the weighted-mean target and proportional drift;
  - goodwill toward the owner as the treatment term;
  - `below_since` semantics;
  - the Upkeep-after-Standing edge.

  sim_agents doc comments already say "ADR 0014" in plain text (no links).
- **`apply_desertions`** still sets `Allegiance(0)` (`upkeep.rs`). Step 2b changes it to the owner of `v.settlement`.
- **game_ui** shows neither owner nor loyalty yet (step 10, UI-02).
- **Nothing calls `set_owner` yet,** so integration tests exercise only the independent path (constant loyalty). The systems' non-trivial dynamics are covered by the unit tests; 2b's conquest scenario covers them end to end.
- **Hashes changed** in every preset (4 newly registered components; villagers.ron's hashed RON has the new block). Behaviour is unchanged; see the checks.

### Checks run (all pass)
- `cargo test -j 2 -p sim_agents`: 15/15. With features unified, `cargo test -j 2 -p sim_agents -p sim_military -p sim_headless`: 15 + 11 (1 ignored) + 3.
- `cargo check -j 2 --workspace --all-targets`: clean.
- `cargo clippy -j 2 --workspace --all-targets -- -D warnings`: clean. `cargo fmt --check`: ok.
- `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 -p sim_agents -p sim_military --no-deps`: clean.
- `cargo test -j 2 -p integration_tests --test village_day --test famine --test determinism -- --test-threads=2`: village_day 1/1, famine 1/1, determinism 12/12 (110 s).
- Also: `--test docs` 4/4, `--test posture --test starving_army --test army_march --test orders --test run_start`: all pass (1, 1, 1, 7, 4).
- **Developed invariance:** release `sim_headless --seed 42 --ticks 4801 --stats-every 600` and `--skirmish` with the same flags, compared with the pre-change release binary over two day boundaries. Stdout is byte-identical in both; stderr differs only in timing and hash lines.
- **Run smoke:** release `--run --seed 42 --ticks 4801 --checkpoint-every 2400` exits 0 (final `0x7de2538c8ca131a2`).
- Disk: ~13 GB free after the release build.

## Step 2 part 2b-conquest-garrison-revolt

I resumed an interrupted attempt. Most of its edits were correct and are kept: conquest.rs, the army.ron `conquest`/`garrison` blocks, Garrison wiring, threat, desertion allegiance, recruit, the Progress set, game_ui and sample_orders. I fixed three behaviour bugs it had, listed under deviations 1-3. I then added the casualty-shock fix, the unit tests, the strengthened attack test, the docs and ADR 0014.

### What was built
- **`crates/sim_military/src/conquest.rs` (new)** (GP-05, GP-06, GP-28):
  - `pub fn owner_of(&World, s) -> FactionId`: INDEPENDENT if there is no `Owner`.
  - `pub fn transfer(world, s, to, cause) -> Option<OwnerChange>`, which is `transfer_seeded(.., None)`.
  - `pub fn transfer_seeded(world, s, to, cause, seed: Option<f32>)`, span `sim_military::transfer`. It returns None (and changes nothing) unless `s` has an `Owner`. Otherwise it:
    1. computes whether `to` (≠ 0) held any other settlement;
    2. calls `sim_agents::set_owner`;
    3. writes a SimEvent: `Invited` for `Cause::Invitation`, `Revolt` for `Cause::Revolt`, otherwise `Conquest`. Conquest/Invited have `faction = to`, `other = previous owner`; Revolt has `faction = previous owner`, `other = to (0)`. All have `subject` = settlement SpawnId, `at` = centre, `amount` = residents, and `cause`;
    4. increments `FactionStats.conquests` for Conquest with `to ≠ 0`;
    5. calls `Factions.set_seat(to, centre)` if `to` held no land before;
    6. writes a MilitaryLog line;
    7. if `to == PLAYER` and the settlement is `RunProgress.rival_seat`, calls `RunState::finish(Won, cause)` (a no-op outside a run). Step 7 must also free the rival's other lands here (a comment marks the spot).
  - `pub fn militia_or_foes_near(world, at, faction, r) -> bool`: FighterGrid entries within r that aren't routed and are either militia with `against == faction` or soldiers `at_war` with faction.
  - `pub fn revolts(world)`: daily (it returns unless `cal.on_day(tick)`), span `sim_military::revolts`. Owned settlements are handled in SpawnId order. Private helpers: `garrison_and_rebels`, `would_be_militia` (the ai.rs fight filters), `rise` and `crush`.
  - Unit tests: `transfer_hands_over_reports_seats_and_wins_the_seat`, `unrest_frees_the_unguarded_and_raises_the_weakly_held`. Each builds a run app and takes under 1 s.
- **`orders.rs`**:
  - `validate_spec(world, issuer, spec)` now takes the issuer. Attack on your own settlement is refused ("your own settlement"). Garrison on a settlement you don't own is refused ("not your settlement").
  - `step_order`'s Garrison arm: the goal is the centre while the settlement is owned. The order pops, with a log line, once the settlement is lost or gone.
  - Attack is now the private `attack(..)` with steps 0 (march), 1 (provoke and fight) and 2 (hold), driven by `army.data.conquest`:
    - Step 1 → 2 when `fought > break_ticks` and no militia stands within `militia_radius`. It logs "broke the defenders", **calls off the provocation** (`call_off`) and sets `order.since = tick`.
    - Step 1 gives up when `fought > cal.days(give_up_days)`.
    - In step 2, `Order.held_since` is set while the captain is within `hold_radius` and `!militia_or_foes_near(.., militia_radius)`, and reset otherwise. After `cal.hours(hold_hours)` it calls `transfer(.., Cause::Conquest{captain})`. After `cal.hours(timeout_hours)` from the break the attack fails.
    - The order also ends if the settlement became the attacker's meanwhile.
  - `pub(crate) fn call_off(world, s, faction)`: clears `Threat.provoked_by/until` only when the provocation is `faction`'s.
  - `fn supply(world, captain, s) -> f32`: moves `spare_food(s, garrison.keep_days)` (capped by baggage room) with `move_food`, unpaid and unprovoking, adds it to `FactionStats.supplied`, and logs "X supplied Y".
  - `gather_food` on an owned settlement goes to `supply`, whatever the posture.
  - `forage` on an owned settlement: `supply` when due (at most daily) and the baggage holds less than `gather_target_days`. **`last_forage` is set only when something was drawn.**
  - `equip` doesn't provoke your own village.
  - `recruit`:
    - another non-zero faction's village → 0 recruits and a log line ("X answers to Y: nobody joins Z");
    - your own village → `p += recruit.loyalty_bonus × SettlementLoyalty.mean`.
  - `militia_standing` takes the radius.
- **`components.rs`**:
  - `Order.held_since: Option<u64>`, in the Captain hash (tag + u64).
  - `hash_spec` tag 8 = Garrison; `describe` gives "garrison".
  - `FactionStats.conquests: u32` and `supplied: f32`, both in the ArmyStats hash.
- **`data.rs`**:
  - `RecruitData.loyalty_bonus`, `ConquestData {break_ticks: u64, give_up_days, militia_radius, hold_radius, hold_hours, timeout_hours}`, `GarrisonData {radius, keep_days, revolt_hours, crushed_loyalty}`, and `ArmyData.{conquest, garrison}`.
  - `ArmyData::validate()` is called from `Army::from_ron`. It rejects negative or non-finite values; `hold_radius`, `garrison.radius` and `revolt_hours` must be > 0, and `crushed_loyalty` must be in 0..=1.
  - Tests: `shipped_data_is_valid` gains the relations `hold_radius <= militia_radius` and `revolt_hours >= 24`; new `rejects_bad_conquest_and_garrison_data`.
- **`assets/data/army.ron`**:
  - `recruit.loyalty_bonus: 0.3`
  - `conquest: (break_ticks: 300, give_up_days: 1.0, militia_radius: 35.0, hold_radius: 12.0, hold_hours: 2.0, timeout_hours: 12.0)`
  - `garrison: (radius: 30.0, keep_days: 1.0, revolt_hours: 24.0, crushed_loyalty: 0.1)`
- **`threat.rs`**: an army whose faction is the settlement's `Owner` adds no threat, unless `provoked_by == owner` (a rising).
- **`upkeep.rs`**: a deserter gets `Allegiance(owner of v.settlement)`.
- **`combat.rs`**: a dying rebel (militia with `against == its own allegiance`) is pushed into `Casualties.fallen` as INDEPENDENT, so it doesn't shake its owner's soldiers. This is a no-op in developed, where militia are allegiance 0 against a non-zero faction.
- **`lib.rs`**:
  - `pub mod conquest`, with root re-exports `militia_or_foes_near`, `owner_of`, `transfer` and `transfer_seeded`.
  - `MilitarySet::Progress` is configured `.in_set(SimSet::Needs).after(MilitarySet::Upkeep).after(AgentSet::Standing)` and holds `progression::progress.run_if(on_hour)`. `MilitarySet::Upkeep` keeps 2a's `.after(AgentSet::Standing)`.
- **`progression/mod.rs`**:
  - `pub fn progress(world)`, span `sim_military::progress`, the D7 driver. Today it calls only `conquest::revolts`; its doc lists the 12 slots in order.
  - The test module is now `pub(crate) mod tests`, with `pub(crate) run_app(seed)` and `pub(crate) command(app, issuer, kind)` for reuse by other modules' tests.
- **sim_core**: `OrderSpec::Garrison { settlement: u64 }` appended (documented). The `Conquest`/`Invited`/`Revolt` docs in events.rs give the `other`/`at`/`amount` meanings.
- **sim_headless `sample_orders`**: the order draw is `0..13`; 11 = `Garrison { settlement: s }`, mostly refused.
- **game_ui `army.rs`**: `MenuAction::Garrison` ("Garrison") in the settlement menu, and in the order line ("garrison at <name>").
- **tests/tests/orders.rs**: `attacking_a_village_raises_its_militia` now also asserts `conquests == 1`, `Owner == 1`, every resident who isn't enlisted has Allegiance 1, and a Conquest chronicle entry `(faction 1, subject vid)`.
- **Docs**:
  - DATA.md army.ron: header (ADR 0014, guards, game-hour note), `recruit.loyalty_bonus`, `conquest` + 6 fields, `garrison` + 4 fields, and a revolt-rule paragraph.
  - ARCHITECTURE: sim_military crate row, schedule row 6 (Progress), and the standing bullet.
  - GLOSSARY: Conquest, Garrison, Progress driver, Revolt (rising), Supply.
  - TESTING: the orders row and the sim_military bullets.
  - HOWTO "Add a captain order": append-only, the next hash tag, the issuer-aware `validate_spec`, and Garrison as the example.
  - **ADR 0014 `docs/adr/0014-ownership-conquest-and-loyalty.md` (new)**: covers 2a and 2b. Step 3 may append its goodwill ledger and spread to it.

### Deviations from the guide (and why)
1. **The break calls off the attacker's provocation** (`call_off`). The guide only says "stop provoking in step 2". Measured in skirmish (seed 42, Emberwick): with the provocation's half-day timer still running, villagers kept re-forming militia (1-14 at a time) through the whole hold. Every hold was contested until the 12-hour timeout, at tick 1555, exactly when the provocation expired, so no attack ever conquered. With `call_off`, the break was at 354, the hold began at 355, and Emberwick was taken at 555.
2. **The rising settle rule is "no garrison left, or rebels standing (FighterGrid, not routed, within `garrison.radius`) outnumber the garrison"**, not the guide's "militia still standing".
   - Measured: only 1-6 of about 250 residents actually take up arms against a 50-man garrison standing in the village. "Any rebel standing" would make every rising succeed, which makes garrisons pointless.
   - The trigger is unchanged: a rising starts when guards < able residents (adult, alive, hunger < starving, Civilian, morale ≥ `fight.break_morale`).
   - A put-down rising costs every resident `crushed_loyalty`. Unrest continues, so it can rise again the next day while guards < able.
3. **Owned-village supply:**
   - `garrison.keep_days` is **1.0**, not the partial attempt's 2.0. A skirmish storehouse holds 0-1.4 days through the day, so 2.0 never yielded anything. 1.0 equals villagers.ron `loyalty.food_days`, so supply never costs the village its food-security term.
   - A draw that finds nothing spare does **not** use up the daily slot; the army retries every 30 ticks.
   - The garrison probe drew 74.6 food value at once and more later, with no desertions over 3 days.
4. **Garrison pops when the settlement is lost** (or isn't found), with a log line. The guide says "never pops". A garrison of land you no longer hold is meaningless, and a still-garrisoning army would try to supply from an enemy village.
5. **New data fields beyond the guide's list:** `conquest.give_up_days` and `conquest.break_ticks` (the old 300-tick and 1-day literals), `garrison.revolt_hours`, `garrison.crushed_loyalty` and `recruit.loyalty_bonus`.
   - There is no separate `conquest.militia_radius` for the break check versus the hold: the one `militia_radius` (35, the old literal) serves both.
   - The hold timeout counts from the break, not from the order.
6. **`transfer_seeded`** exists beside `transfer` for callers that seed loyalty explicitly, such as the rival setup in step 7 (`Some(rival.start_loyalty)`). Invitations should use `transfer(.., Cause::Invitation)` (seed from goodwill).
7. **`owner_of` is a pub fn in sim_military** (`sim_military::owner_of(&World, Entity)`). The test helper `owner_of(app, settlement)` from guide D6 can wrap it.
8. **The rebel casualty-shock fix** (combat.rs) is from the guide's step-2 risk list ("soldier_upkeep's casualty shock now fires for player soldiers near dying militia"). It is fixed for rebels only. Militia of an owned village defending against raiders still count as allies of the owner's soldiers, which is intended.
9. **ADR 0014 is named `0014-ownership-conquest-and-loyalty.md`** (D3 said "ownership, goodwill and loyalty"). Goodwill's ledger and spread arrive in step 3, which should add a short section to it.
10. **No conquest scenario test (`tests/tests/conquest.rs`)** in this part; the task's checks didn't include it. The unit tests and the strengthened orders test cover the mechanics.

### Names later parts must use
- **`sim_military::{owner_of, transfer, transfer_seeded, militia_or_foes_near}`**, plus `sim_military::conquest::revolts` and `sim_military::progression::progress`.
- **`MilitarySet::Progress`**. Add new hourly or daily steps inside `progression::progress`, in the D7 order (its doc lists the slots), each with its own span. Don't add systems to the set.
- **`FactionStats.conquests`, `FactionStats.supplied`** and `Order.held_since`.
- **`army.data.conquest.*`, `army.data.garrison.*`, `army.data.recruit.loyalty_bonus`.**
- **`OrderSpec::Garrison { settlement }`** has hash tag 8. **Step 3's Forage and Give take tags 9 and 10.**
- **`crate::orders::call_off(world, s, faction)`**: use it to end a provocation; it never touches another faction's.
- **Unit tests in other sim_military modules** can use `crate::progression::tests::{run_app, command}`.
- **Step 3 invitations:** call `transfer(world, s, f, Cause::Invitation)`; it emits `Invited` and seats a landless faction.
- **Step 7:** the "free the rival's other lands" hook is in `transfer_seeded`, after the Won check.
  - Careful: every cause other than `Invitation` and `Revolt` (including `Cause::None`) emits a `Conquest` event and bumps `conquests`.
  - So the rival's setup lands should not go through `transfer`. Either call `sim_agents::set_owner(world, s, RIVAL, Some(start_loyalty))` directly, plus `Factions.set_seat`, or add a quiet setup variant.

### Measured (skirmish seed 42, attack on Emberwick, 278 people)
- Conquest: break at tick 354, hold from 355, taken at 555. Loyalty seed 0.5, 264 residents handed over.
- **An unguarded starved conquest** (food drained, farmers cut, army 80 tiles away): `below_since` at 4800 (day 2), free at 7200 (day 3). That is within the "revolt within 2-3 days" criterion.
- **A fed garrison with no raiders:** mean loyalty went 0.50 → 0.457 → 0.427 → 0.42, falling. `FoodSecurity.days` at the midnight day tick is about 0.42-0.55, so the food term is only about 0.5 (see open issues).
- **A garrison of 50 against a rising** (loyalty forced to 0): the rising started at day 2, only 1-6 rebels fought at any time, and it was put down at the next check.
- **Skirmish raiders** reached the garrisoned village around tick 6200 and nearly wiped out the 50-man garrison. A scenario that needs a stable garrison should remove them or use the run preset.

### Open issues
- **Loyalty tuning for "a passive garrison with a fed village (loyalty rises)" (the conquest scenario).** With villagers.ron `loyalty.food_days: 1.0`, a fed skirmish village scores only about 0.5 on food at the day tick, so a fed conquest drifts down from 0.5 to about 0.42 and recovers only through the time term (+0.015/day).
  - Options: lower `loyalty.food_days` to about 0.4-0.5, or sample food security over the day.
  - Re-run the sim_agents loyalty unit tests (they assert starved < `revolt_threshold` and fed > 0.5) and this part's conquest unit tests.
- **`tests/tests/conquest.rs` (scenario) is not written.** Suggested shape: the skirmish with the raiders despawned before tick 0, or the run preset. Attack the nearest village (conquered by about tick 600), then branch into three cases:
  - Garrison + fed;
  - an aggressive non-owner army plundering it;
  - drain the stores + `cut_off_farmers` + move the army more than `garrison.radius` away. Expect a revolt by about day 3.
- **`Threat.provoked_by` is a single slot.** A raid during a rising overwrites it, and the rising is then put down at its check.
- **Raider pressure on garrisons in skirmish** is heavy (see Measured). That is not a bug.
- **Rival lands freed on a win:** step 7.
- **sim_headless stats** print the two new FactionStats fields (Debug), so `--skirmish` stdout differs from 2a's only by `, conquests: 0, supplied: 0.0` on each stats line.

### Checks run (all pass)
- `cargo check -j 2 --workspace --all-targets`; `cargo clippy -j 2 --workspace --all-targets -- -D warnings`; `cargo fmt --check`; `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps`.
- `cargo test -j 2 -p integration_tests --test orders --test posture --test starving_army --test famine --test village_day --test army_march --test determinism --test run_start -- --test-threads=2`: orders 7/7, posture 1, starving_army 1, famine 1, village_day 1, army_march 1, determinism 12/12 (125 s), run_start 4/4. orders, posture and starving_army were re-run after the last (refactor-only) edit.
- Unit tests: sim_military 14 (+1 ignored), sim_agents 15, sim_core 32, sim_headless 3, game_ui 6, and docs 4/4.
- **Developed invariance:**
  - release `--seed 42 --ticks 4801 --stats-every 600` stdout is byte-identical to 2a's `b2_dev.out`;
  - `--skirmish --ticks 6000 --stats-every 2000` is identical to 2a's `b2_skirm.out` once the two new FactionStats fields are removed;
  - the named `--skirmish --ticks 6000 --stats-every 2000` run exits 0.
- **Release replays**, record + verify multi + `--single-threaded`, all "replay ok" with the same hash:
  - skirmish `--commands 200 --ticks 6000`: `0xea78b159bfdea196`;
  - `--run --seed 42 --ticks 4801 --commands 100`: `0x9317ca32ad3d4bde`.
- **Housekeeping:**
  - The temporary probe `tests/tests/zz_probe.rs` was deleted. Something had staged the working tree mid-part (`git add`; HEAD is still a63a784), so I removed the deleted probe from the index with `git rm --cached` (no reset or checkout). Other files' staged state was left as found.
  - Disk: 7.9 GB free.

## Step 2 part 2c-tests-docs

The scenario runs on the **run preset** (seed 42) as the guide asks; no skirmish fallback was needed. A run tick costs about 1.5 ms at the test profile, and the champion's own band of 10 ("Outlaws") takes the nearest hamlet, so no extra army is spawned for the conquest.

### What was built
- **`tests/tests/conquest.rs` (new, untracked: the orchestrator must `git add` it)**, scenario `conquest` (GP-05, GP-06, SIM-24). It has three `#[test]`s that run in parallel, 26-28 s wall for the file with `--test-threads=2`.
  - **Helpers:**
    - `nearest_hamlet(app, from)`: the nearest `SizeTier::Hamlet` that isn't the rival's seat (ties: lowest SpawnId).
    - `allegiances(app, s)`
    - `garrison(app, s, faction)`: living enlisted soldiers within `garrison.radius`.
    - `Day` plus `days(app, s, n)`: steps to just after each of the next n day ticks and reads the owner, the mean and min loyalty, the food days, `below_since` and the garrison.
    - `conquest_tick_and_seed(app, s)`: the seed is read from the settlement's `Goodwill` as `(g + 1) / 2`, so it stays valid once step 3 moves goodwill.
  - **Prefix `conquered(seed) -> (App, Entity)`:**
    1. `run_app(seed)`.
    2. Picks the start-offer card with a `FoodDays` effect (seed 42: "Well fed"); the test fails if none is offered.
    3. The champion Attacks the nearest hamlet, and the test polls every 100 ticks until `owner_of == PLAYER` (bounded by 4,000 ticks).
    4. Asserts: no refused commands; `conquests == 1`; the Conquest chronicle entry `(PLAYER, hamlet id, other 0, Cause::Conquest{champion})`, with `at` = the centre, `amount` = the residents, and `Owner.since` = the entry's tick; every resident who isn't enlisted has Allegiance PLAYER.
    5. Seed 42: hamlet #21 (56 people, 121 tiles away), taken at tick 1603, seen at 1700, 52 residents.
  - **`a_fed_village_under_a_passive_garrison_grows_loyal`:** SetPosture Passive, then Garrison. Over 3 day ticks it asserts:
    - owner PLAYER, garrison > 0, no `below_since`;
    - the mean strictly rises at each tick, ending above seed + 0.1. Measured: 0.582, 0.637, 0.676.
    - Garrison is still the front order, and nothing was refused.
  - **`a_plundered_village_loses_faith`:**
    - Setup: the band MoveTo's the start. A 40-man RAIDERS band (Aggressive, armed, 3 days of food, no `Raider` marker) is spawned between ticks, 25 tiles from the centre on the side away from the start. As faction 2 commands, it is ordered to GatherFood at the hamlet, then (queued) MoveTo a tile 2 × `threat.radius` away.
    - Asserts:
      - RAIDERS `seized > 0`, `provocations >= 1`, no refusals;
      - the owner stays PLAYER with no garrison;
      - food security falls (0.46, 0.16, 0.05 days);
      - the mean falls at each day tick and ends below the seed. Measured: 0.563, 0.474, 0.380.
  - **`a_starved_conquest_revolts`:**
    - Setup: the band leaves; the storehouse **and homes** are emptied (`goods.clear()`); `cut_off_farmers` (7 farmers).
    - Asserts:
      - a Revolt chronicle entry `(PLAYER, id, other 0, Cause::Revolt)` 2-3 days after the conquest (measured: tick 7200, 2.33 days);
      - `below_since` came at least a day before it (4800);
      - no garrison stood;
      - afterwards the owner is INDEPENDENT with `since` = the revolt tick, every resident is INDEPENDENT, the mean equals `independent_target`, and `conquests` is still 1.
    - Loyalty went 0.362, then 0.285.
- **`tests/tests/determinism.rs`, `run_state_hash_covers_every_sim_component`:**
  - New helper `spawn_beside_nearest_hamlet(app) -> (hamlet, id, captain)`: before tick 0, it spawns a 30-man armed PLAYER warband 20 tiles from the hamlet nearest the start, toward the start. This happens after `sample_orders` has run, so the random orders never name it.
  - The test issues a live Attack before the first step, then steps 15 × 100 ticks, and issues a live Garrison once the hamlet is owned.
  - It asserts SimTick 1500, no outcome, the boon picked, `conquests >= 1`, Garrison at the warband's front order, and `unhashed_state` empty.
  - It prints the conquest tick: 510. The test takes 3.3 s.
- **`tests/tests/common/mod.rs`:** new `owner_of(&App, Entity) -> FactionId`, which wraps `sim_military::owner_of` (guide D6). The module doc is updated.
- **Tuning, `assets/data/villagers.ron`:** `loyalty.food_days` goes from **1.0 to 0.5**, and its comment is rewritten. The `garrison.keep_days` comment in `army.ron` now reads "kept at or above loyalty.food_days".
- **`crates/sim_military/src/data.rs`:** the `shipped_data_loads` unit test also asserts `garrison.keep_days >= Villagers::default_data(..).data.loyalty.food_days`. This is test-only; no pub items changed.
- **Docs:**
  - **DATA.md:**
    - the villagers.ron header names the conquest scenario as a guard of `loyalty`;
    - the `loyalty.food_days` row explains the 0.5 value (the day-tick food security of a fed village, and why 1.0 drifted down);
    - the army.ron header names the scenario;
    - the `garrison.keep_days` row gives the keep_days ≥ food_days rule, the test that checks it, and that small villages' garrisons live on their baggage.
  - **TESTING.md:** three `conquest.rs` rows (between run_start and village_day) with the measured numbers. The `run_state_hash_covers_every_sim_component` row now covers the step 2 conquest and garrison. The sim_military unit bullet mentions the conquest relations in `shipped_data_loads`.
  - **HOWTO.md:**
    - "Add a captain order" step 5 points at `tests/tests/conquest.rs` for multi-day orders.
    - "Add a scenario test" step 2 adds two points: another faction's band takes orders with its faction as the issuer, and a branching scenario may make its branch setup edits right after the shared prefix, between ticks, as after a load.
  - **GLOSSARY:** the Garrison row adds that supply works only when the village has food to spare, and that a hamlet's garrison lives on its own baggage. Owner, Loyalty, Revolt, Conquest and Supply were already complete from 2a/2b.
  - **ARCHITECTURE:** no change was needed. Schedule row 6 (Standing, Progress and the driver) and the crate rows were already complete from 2a/2b.
  - **ADR 0014** (`docs/adr/0014-ownership-conquest-and-loyalty.md`): the "Loyalty tuning is still open" consequence is replaced by:
    - the food_days 0.5 tuning and why;
    - the scenario and its numbers;
    - "harsh treatment by the owner reaches loyalty only through goodwill" (the raiding-band stand-in, and food outweighing treatment);
    - small villages can't supply garrisons;
    - the tests list.

### Deviations from the guide (and why)
1. **The aggressive branch is a non-owner raiding band, not the owner's aggressive garrison.** The guide (binding) says an owner's army never plunders its own village, and nothing moves goodwill until step 3. A test-side goodwill edit was rejected for two reasons:
   - It would test the test.
   - With the shipped weights (0.55/0.3/0.15), a fed village whose owner it hates still aims at about 0.55 and would not fall below its 0.5 seed.

   So the RAIDERS band plunders and leaves (the guide's "GatherFood ... with a hostile-posture army that is not the owner"). Loyalty falls through food: the raid costs the hamlet its reserve and half a day's work, and it never feeds itself back.
2. **The raid must move on.** A hostile band that **stays** near the village freezes `FoodSecurity`: hiding villagers neither work nor eat. Measured: 1.19 days for two days, during which loyalty *rose* (0.58, then 0.64). The raiders therefore get a queued MoveTo out of sight.
3. **The prefix picks the start boon with food** ("Well fed", +3 days). The hamlet's storehouse never holds more than `garrison.keep_days` (1.0) of food (measured 0-18 food value against a need of about 52 a day), so supply never fires. Without the boon, the Outlaws band (1 day of food) deserted by the third day tick, and the "passive garrison" disappeared. Picking the boon is ordinary player input, shared by all branches.
4. **Branch edits happen between ticks after the prefix:** spawning the raiders, and emptying the stores plus `cut_off_farmers`. Each branch replays the prefix (there are no saves before step 8), so these edits stand in for setup after a load. HOWTO now documents the pattern; the M3 rule "never edit components mid-run" still holds for input.
5. **The starved branch empties the storehouse *and the homes*.** The guide said "the storehouse". With the storehouse alone, food lasted another 1.5 days: `below_since` was set at 9600 and the revolt came on day 4, outside "2-3 days".
6. **Retune:** `loyalty.food_days` goes from 1.0 to 0.5, as 2b's open issue asked. This is data only, with no weights or code changed.
   - Measured at the day tick, a fed run hamlet holds 0.68-0.96 days and a skirmish village about 0.45. At 1.0, the fed garrisoned hamlet drifted 0.50 → 0.475 → 0.492 → 0.490.
   - Starved: the target is about 0.16, unchanged.
   - Developed and skirmish are unaffected: they own nothing, and independent targets ignore food.
7. **ADR file name kept** as `0014-ownership-conquest-and-loyalty.md`. The task text named `0014-ownership-goodwill-and-loyalty.md`, but 2b recorded this name in this log (which overrides the guide), and DATA.md and ARCHITECTURE.md link to it. Step 3 can add a goodwill section.
8. **Determinism extension:** Garrison can't be pre-scripted in the CommandScript, because `validate_spec` refuses it until the issuer owns the settlement. Both orders are live player commands written between steps, which is replay-safe (determinism.rs:73 pattern). A second warband is used, not the champion, so `sample_orders`' random champion orders can't cancel the attack.
9. **Extras not in the guide:** `common::owner_of`; the keep_days ≥ food_days assertion in sim_military's `shipped_data_loads`. Guide D6's `step_days` and `step_until` are not added; `days()` lives in conquest.rs.

### Names later parts must use
- **Test helpers:**
  - `common::owner_of(&App, Entity)`;
  - in conquest.rs: `conquered(seed)`, `nearest_hamlet`, `days`, `garrison`, `conquest_tick_and_seed`;
  - in determinism.rs: `spawn_beside_nearest_hamlet`.
- **Test names:** `a_fed_village_under_a_passive_garrison_grows_loyal`, `a_plundered_village_loses_faith`, `a_starved_conquest_revolts`.
- **Tuning relation:** `garrison.keep_days >= loyalty.food_days`, asserted in `sim_military::data::tests::shipped_data_loads`.

### Measured (seed 42, run island, test profile)
- The prefix: hamlet #21, 56 people, 121 tiles away. The militia rose at 1094 and broke at 1402, the hamlet was taken at 1603 with 52 residents and 4 kills, and the seed was 0.5.
- Branches, at day ticks 2400 / 4800 / 7200:
  - **garrison:** loyalty 0.582 / 0.637 / 0.676; food 0.68 / 0.96 / 0.82; 11 guards; supplied 0.
  - **plunder:** loyalty 0.563 / 0.474 / 0.380; food 0.46 / 0.16 / 0.05; raiders seized 15.3, provoked 1, killed 3.
  - **starved:** loyalty 0.362 / 0.285 (below since 4800); revolt at 7200.
- **Coverage run:** the 30-man warband took the hamlet at tick 510.

### Open issues
- **Step 3 (goodwill):**
  - The Attack provokes and kills militia, so goodwill toward PLAYER (and the conquest seed) will drop below 0.5. `conquest_tick_and_seed` reads the seed from Goodwill, so it adapts. But `a_plundered_village_loses_faith` asserts "ends below its seed": re-run it, and if a lower seed breaks it, assert against 0.5 or the passive branch instead.
  - The raiders' plunder will lower goodwill toward RAIDERS, not toward the owner.
  - Consider whether an owner who fails to drive off raiders (a failed DriveOff request) should lose goodwill. That is the natural "treatment" hit for this branch.
- **Loyalty weights:** food dominates (0.55), so a harsh owner can't lose a fed village. Revisit when goodwill deeds land, if the playtest wants that.
- **Siege model gap:** while a hostile army stays within the threat radius, villagers hide, and `FoodSecurity` and loyalty look fine even as hunger rises. This is a loyalty/food-security modelling issue for a later step (for example, a threat term, or sampling hunger).
- **Supply rarely fires in small villages:** a hamlet's storehouse holds under a day, below `keep_days` (1.0). Garrisons there live on their baggage, so the step-11 bot must feed them (Trade or GatherFood elsewhere, or the food boon).
- **Step 8:** switch the three branches to branch from one save (`save_world` after `conquered`), per M4_PLAN.
- **Cosmetic:** `FoodSecurity.days` prints as `-0.00` when every stock is empty, because the float `Sum` starts at -0.0. It is harmless and deterministic.

### Checks run (all pass)
- `cargo fmt --check`.
- `cargo clippy -j 2 --workspace --all-targets -- -D warnings`: clean, re-run on integration_tests after the last edit.
- **Named checks:** `cargo test -j 2 -p integration_tests --test conquest --test determinism --test orders --test docs -- --test-threads=2`: conquest 3/3 (26-28 s), determinism 12/12 (127 s), orders 7/7 (9 s, including `attacking_a_village_raises_its_militia`), docs 4/4. conquest and docs were re-run after the final helper edit.
- `cargo test -j 2 -p sim_agents -p sim_military`: 15 and 14 pass (+1 ignored).
- **Guarded tuning tests**, after the villagers.ron change: village_day 1, famine 1, posture 1, starving_army 1, army_march 1, run_start 4/4.
- **Developed invariance** (release `sim_headless`):
  - `--seed 42 --ticks 4801 --stats-every 600`: stdout byte-identical to 2b's `2b_dev.out`;
  - `--skirmish --seed 42 --ticks 6000 --stats-every 2000`: byte-identical to `2b_skirmish.out`.
- **Run replay smoke:** `--run --seed 42 --ticks 4801 --commands 100 --record 2c_run.ron`, then `--replay` and `--replay --single-threaded`: "replay ok", final `0x748756ac337c07b6`.
- **Not re-run:** rustdoc (no pub items changed) and the full `cargo test --workspace`.
- **Disk:** 5.7 GB free; nothing was pruned. The probe file used while tuning is kept at `scratchpad/conquest_probe.rs` and is not in the tree.

### Gate
Step 2 definition-of-done gate, 2026-10-08, on the uncommitted working tree on top of `a63a784` (parts 2a, 2b and 2c). **Green.** No code, data or docs changed; nothing needed fixing.
- `cargo fmt` changed nothing, and `cargo fmt --check` passes.
- `cargo clippy -j 2 --workspace --all-targets -- -D warnings`: clean.
- `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps`: clean.
- `cargo test -j 2 --workspace -- --test-threads=2` (CARGO_INCREMENTAL=0): 25 test binaries plus 12 doc-test runs, **166 passed, 0 failed, 1 ignored** (`survey_twenty_seeds`). That is step 1's 152, plus sim_agents +8, sim_military +3 and conquest.rs +3.
  - Integration: determinism 12/12 (131 s), conquest 3/3 (29 s), run_start 4/4 (13 s), worldgen 7/7, orders 7/7, docs 4/4, army_march 1, posture 1, starving_army 1, village_day 1, famine 1.
  - Unit: sim_core 32, sim_world 32, sim_agents 15, sim_military 14 (+1 ignored), sim_economy 10, sim_headless 3, game_render 12, game_ui 6.
  - The guarded tuning tests (village_day, famine, army_march, posture, starving_army, orders) all pass. The only tuning change in step 2 is 2c's villagers.ron `loyalty.food_days` 1.0 → 0.5, which only affects owned settlements, so developed and skirmish are unaffected.
  - Log: scratchpad/gate2_test.log.
- Release `sim_headless --run --seed 42 --ticks 3000 --checkpoint-every 1000`: exit 0, 744 ticks/s. Checkpoints were 1000 `0x021552ff7a931655` and 2000 `0xd29ec004d24bebce`, final `0xe2baeaf59cdd1fef`. They differ from step 1's because of the newly hashed step-2 components and the villagers.ron data; nothing is golden. The start is unchanged: Dunmoor seat, start (134, 522), Outlaws, Brave, Weaker rival + Restless roads.
- Run replay smoke (`--run --ticks 3000 --commands 50 --record gate.ron`): 51 commands, final `0xa843a4bdc8199806`. `--replay` and `--replay --single-threaded` both printed "replay ok" with 3 checkpoints and the same hash.
- Developed replay smoke (`gate_dev.ron`, 50 commands): record, replay and single-threaded replay all reproduce `0xaeb06c9d069ed487`.
- Housekeeping: free disk was 3.1 GB before the release step. I ran `prune_target.sh 90` (6.1 GB) and removed `target/debug/incremental`, which left 11 GB free. The next debug build is non-incremental.
- Reminder for the orchestrator: `tests/tests/conquest.rs` is still untracked and must be `git add`ed with this step.

### Fixes
Review fixes on the step 2 working tree (2026-10-08). Every confirmed finding was fixed; none was declined.
- **Major: an aggressive attacker plundered and re-provoked the village it was holding.** `orders::forage` now returns early, before `last_forage` is set, when the captain's front order is `Attack { settlement }` at step >= 2 on the nearest settlement (new private `holding(&Captain, SpawnId)`). The `attack`/`forage` docs and ADR 0014 say so. Test written first: `orders::tests::a_held_village_is_neither_plundered_nor_provoked` (aggressive army with 2 days of baggage, Attack forced to step 2 at a stocked village). It fails without the fix: provoked during the hold.
- **Deserters' loyalty:** `apply_desertions` also inserts `Loyalty` = the settlement's `SettlementLoyalty.mean` (when residents > 0), otherwise `standing::default_seed(owner)`. Test: `a_deserter_comes_home_to_the_current_owner` (fails without the fix). The loyalty.rs module doc and ADR 0014 are updated. Developed is unaffected: independent means are exactly `independent_target`.
- **`garrison.revolt_hours` shorter than a day is refused** in `Army::from_ron` (`calendar.hours(revolt_hours) < ticks_per_day`). `rejects_bad_conquest_and_garrison_data` gains `revolt_hours: 12.0`, and the redundant `>= 24` assert was dropped from `shipped_data_loads`. The DATA.md row now reads "at least one day (shorter is refused)".
- **Determinism coverage:** new `conquest.rs::a_conquest_and_its_revolt_are_thread_independent` (NFR-08). It plays the prefix plus the starved branch with `Threading::Multi` and with `Threading::Single`, checks equal state hashes every 100 ticks (72 checkpoints, to tick 8900), and asserts the Revolt. It takes about 60 s.
  - The prefix is now `conquered_with(seed, Threading)`, and `conquered` wraps it.
  - The starved setup moved into `starve(app, s) -> u32`.
  - The determinism.rs coverage-test doc and TESTING.md point to the new test.
  - I did not add the optional fed/garrison variant, to save test time.
- **New tests** (named in TESTING.md):
  - sim_military `orders::tests` (new module):
    - `an_owned_village_supplies_its_owners_army_and_is_never_plundered`: gather_food, the forage owned branch, and the nothing-spare retry;
    - `recruiting_answers_to_the_owner_and_its_loyalty`: a rival's village gives 0; at mean 1 your own gives more than at 0;
    - `equipping_from_your_own_village_provokes_nobody`.
  - conquest unit tests:
    - `a_rising_beats_a_weak_garrison`: the real schedule, a 2-man garrison; the rising at 2400, militia against PLAYER in the field 6 h later, independent with a Revolt by 4800;
    - `a_garrison_marches_to_its_village_and_ends_when_it_is_lost`: goal = centre for a distant champion; the order pops with "no longer holds" after a transfer away.
  - `threat::tests::an_owners_army_frightens_only_a_risen_village`.
  - sim_agents `goodwill_toward_the_owner_reaches_drift`: liked > neutral > hated; goodwill toward RIVAL changes nothing. `the_target_weighs_food_goodwill_and_time` also asserts `w_goodwill > 0`.
  - conquest.rs prefix: asserts `kills > 0` and that the transfer came at least `hold_hours` after the break, recorded from the front order's `since` at step 2 (seed 42: broke at 1402, taken at 1603).
  - Passive branch: asserts `Threat.level == 0.0` and `food_days >= loyalty.food_days` at each day tick. `Day` gains `threat`.
- **Docs:**
  - ADR 0014: developed vs skirmish output wording, owned land can arise in developed/skirmish via Attack, the deserter loyalty, the hold/forage rule, and the tests list.
  - `revolts` rustdoc wording.
  - ARCHITECTURE: the sim_military crate row lists the real locations (conquest.rs vs the orders.rs logistics step), and the exclusive-systems rule names the hourly progress driver.
  - The `OrderSpec::Garrison` doc says saves store the position and `hash_spec` a tag byte.
  - README village-orders row gains "garrison (a village you hold)".
  - TESTING names `set_owner_ignores_what_is_not_a_populated_settlement`. The plunder row says it covers food only, and that treatment is covered at unit level until step 3.
- **Visibility:** `conquest::tests` is now `pub(crate)` with `pub(crate) settlements`, for reuse by the orders and threat tests.
- **Open for step 3** (unchanged): an end-to-end branch where the owner's deed lowers goodwill and so loyalty.

### Gate
After the fixes, 2026-10-08. **Green.**
- `cargo fmt --check` ok.
- `cargo clippy -j 2 --workspace --all-targets -- -D warnings` clean.
- `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps` clean.
- `cargo test -j 2 --workspace -- --test-threads=2` (CARGO_INCREMENTAL=0): exit 0, **176 passed, 0 failed, 1 ignored** (166 + 10 new). Log: scratchpad/gate2b_test.log.
  - determinism 12/12 (215 s), conquest 4/4 (79 s), orders 7, run_start 4, worldgen 7, docs 4, army_march 1, posture 1, starving_army 1, village_day 1, famine 1.
  - Units: sim_military 22 (+1 ignored), sim_agents 16.
  - Guarded tuning tests are unchanged.
- Release `--run --seed 42 --ticks 3000 --checkpoint-every 1000`: exit 0, with the same hashes as the previous gate (1000 `0x021552ff7a931655`, 2000 `0xd29ec004d24bebce`, final `0xe2baeaf59cdd1fef`). No fix touches that run.
- Run replay smoke (`--run --ticks 3000 --commands 50 --record gate.ron`): 51 commands, `0xa843a4bdc8199806`. `--replay` and `--replay --single-threaded` both print "replay ok" (3 checkpoints, same hash).
- Disk: 8.8 GB free before the release build; nothing pruned.
- `tests/tests/conquest.rs` must still be `git add`ed with this step.

## Step 3 part 3a

Built on `5dff41e`. Everything is in the working tree; **`crates/sim_military/src/wild.rs` is new (untracked): the orchestrator must `git add` it.**

### What was built
- **G4 fix (`combat.rs`, `upkeep.rs`).** `find_fallen` also scans `Query<(Entity, &SpawnId, &CombatStats), With<Enlisted>>` for `!alive` (captains are `Enlisted` too), merges with the grid's dead, sorts by SpawnId and dedups. `soldier_upkeep` no longer rolls desertion for a follower who died this tick (`&& cs.alive()`), so a starved man can't turn back into a dead villager that nothing removes. Unit test `combat::tests::a_starved_soldier_and_captain_are_removed` (fails without the fix).
  - **starving_army numbers are unchanged** (deserted 195, first at 1252, starvation from 1868; fed 0): starvation damage starts at day 0.78 and at 40/day kills a 100-health soldier only ~2.5 days later, after the test's 3 days. No assertion was touched.
- **Goodwill (sim_agents `standing.rs`)** (GP-22):
  - `Deed {Trade, Equip, Seize, Plunder, Provoke, Killing, Request, Boon}` (`#[repr(u8)]`, Ord, append only).
  - `GoodwillChange {settlement: u64, faction, delta, cause: Deed}`; `GoodwillLedger(pub Vec<GoodwillChange>)` (hashed resource, `push` drops zero/non-finite deltas, `is_empty`); `ledger_nonempty` run condition.
  - `pub fn spread_totals(&mut [GoodwillChange], &[(SpawnId, IVec2)] sorted, radius, fraction) -> Vec<(index, FactionId, f32)>`: sorts by (settlement, faction, deed, delta bits); per settlement in SpawnId order sums direct + `fraction` x changes whose settlement centre is within `radius`; changes naming no listed settlement are dropped.
  - `pub fn apply_goodwill` (span `sim_agents::apply_goodwill`): query `(&SpawnId, &Settlement, &mut Goodwill)`, applies each sum once with `Goodwill::add` (clamped), clears the ledger. First in the `AgentSet::Standing` chain with `.run_if(standing::ledger_nonempty)`.
  - villagers.ron `goodwill: (spread_radius: 150.0, spread_fraction: 0.25)`; `GoodwillData` + `validate` (radius >= 0, fraction 0..1); test `rejects_bad_goodwill`.
  - Unit test `spread_is_order_independent` (three push orders x three spawn orders, exact sums, clamp, out-of-range, unknown settlement, zero delta).
  - Root re-exports: `Deed, GoodwillChange, GoodwillLedger`.
- **Deeds (sim_military)**: `orders::goodwill_deed(world, s: Entity, faction, deed)` (pub(crate)) posts `Progression.data.goodwill_deeds.delta(deed)` for `s`'s SpawnId. Posted from:
  - `barter` when `bought > 0` (Trade order and passive gathering) -> Trade;
  - passive `equip` when it paid food -> Equip;
  - neutral `gather_food` when `seized > 0` -> Seize;
  - aggressive `gather_food` when `seized > 0` -> Plunder (then `provoke`);
  - `provoke` only on a **fresh** provocation (it is called every tick of an Attack) -> Provoke;
  - `casualties`: a dying militiaman -> Killing at `Villager.settlement` toward `against`;
  - the "Known" boon (`offers.rs`) pushes its own `Goodwill(g)` amount as `Deed::Boon` to the settlement nearest the champion. **`RunProgress.pending_goodwill` was removed** (field and hash).
  - progression.ron `goodwill_deeds: (trade 0.02, equip 0.02, seize -0.05, plunder -0.1, provoke -0.1, killing -0.02, request 0.2)`; `GoodwillDeeds {..}` with `delta(Deed)` (Boon -> 0); validated in -1..1.
- **Give** (`OrderSpec::Give { settlement }`, hash tag 10, describe "give food"): validated as a populated settlement; in `step_order`'s settlement arm, within `recruit.radius`, `orders::give(world, captain, s) -> f32` (pub(crate)) moves baggage food beyond army.ron `give_keep_days` (1.0) x daily need with `move_food` into the storehouse, adds the **returned** value to new `FactionStats.donated`, logs "X gave food to Y" / "X had no food to spare for Y", and the order ends.
- **Forage** (`OrderSpec::Forage { radius: u16 }`, hash tag 9, describe "forage within r"): validated `1..=forage.max_radius` ("bad forage radius").
  - `crates/sim_military/src/wild.rs`: `CELL = 16`; `WildFood {cell, per_side, tiles: Vec<u16>, level: Vec<f32>}` (hashed resource, init empty); `build(map, per_tile)`, `is_built`, `capacity`, `patches_within(at, radius)` (nearest tile of the patch within radius, index order), `take`, `available`, `grow`; `ensure_built(world)`; `regrow(world)` (daily, span `sim_military::wild::regrow`); `Harvest {Gathered(f32), Full, Exhausted}`; `pub fn harvest(world, captain, radius, tick)`.
  - `step_order` arm: goal None; step 0 sets `since`/`origin`; every `forage.every_ticks` from `since` calls `harvest`. Full (baggage_room <= 0 or no stockpile room) or Exhausted (nothing within the radius) logs and ends the order.
  - Harvest: want = min(per_member x members, baggage_room / mult); patches depleted by the base amount; base x `mult(f, ForageYield)` turned into units of the least nourishing food, stochastic rounding with `rng_key("sim_military::wild") ^ captain` at the tick (drawn only when the fraction > 0), `Stockpile::add`; new `FactionStats.foraged` counts the food value added.
  - `wild::regrow` is slot 4 of `progression::progress` (after revolts). `setup_run` builds the patches (`ensure_built`) after the start modifiers.
  - progression.ron `forage: (per_tile 0.01, regrow_per_day 0.1, every_ticks 100, per_member 0.05, max_radius 64, default_radius 32)`; `ForageData`, validated.
  - `orders::army_size` and `orders::baggage_room` are now pub(crate).
- `FactionStats.{donated, foraged}` are in the hand-written ArmyStats hash. `WildFood` registered hashed; `GoodwillLedger` registered hashed (after SettlementLoyalty).
- sim_military root re-exports `ForageData`, `GoodwillDeeds`, `WildFood`; `pub mod wild`.
- **sample_orders**: order draw `0..15`; 12 = `Forage { radius: 0..80 }` (0 and >64 refused), 13 = `Give { settlement: s }`.
- **game_ui army.rs**: settlement menu gains "Give food"; panel button "Forage (F)" and hotkey F send `Forage { radius: forage.default_radius }` (reads `Res<Progression>`); `order_line` names Give's settlement. README controls updated.
- **Tests**: run_start's boon check now asserts the nearest settlement's `Goodwill.get(PLAYER) == g`, the ledger empty, and goodwill unchanged after the refused double pick. sim_military unit tests `deeds_move_goodwill_and_word_spreads`, `giving_hands_over_food_beyond_the_keep`, `a_foraging_band_lives_off_the_land`, `patches_deplete_and_regrow`, `a_starved_soldier_and_captain_are_removed`; `rejects_bad_data` covers the new blocks.
- **Docs**: DATA.md (army `give_keep_days`; villagers `goodwill` x3; progression `goodwill_deeds` x8, `forage` x7, section intro and the boon `Goodwill` text), GLOSSARY (Deed, Forage, Give, Goodwill ledger, Wild food; Goodwill updated), ARCHITECTURE (crate rows, schedule rows 5 and 6, standing bullet), TESTING (new unit bullets, conquest and starving_army rows), HOWTO (new recipe "Make a deed move goodwill"; captain-order and run-only-rule notes), ADR 0014 addendum (goodwill ledger, spread, measured conquest numbers).

### Deviations from the guide (and why)
1. **The conquest scenario's starved branch now revolts at 1.33 days, and its bound is 1-3 days (was 2-3).** The attack provokes the hamlet and kills 4 militia, so goodwill toward the player is -0.18 and the conquest seed is 0.41 instead of 0.5; the starved hamlet drops below 0.35 at the first day tick (0.30) and revolts a day later (tick 4800). This is the intended effect of goodwill seeding loyalty (D1, M4_PLAN "loyalty starts from its goodwill toward the new owner"). The "a day of unrest first" assertion is kept. Other branches: garrison 0.52/0.59/0.64 (rises), plundered 0.50/0.42/0.34 (ends below the 0.41 seed).
2. **`killing` is -0.02**, not the first-draft -0.05: per militiaman, so a bloody town assault would otherwise clamp goodwill to -1 and seed loyalty at 0.
3. **`wild::regrow` is gated on "patches built", not `in_run`.** In a run they are built at setup, so it is run-only in practice; elsewhere they exist only after a `Forage` order (built lazily by `harvest`), and a skirmish band foraging should see regrowth too. Developed/M1-M3 outcomes are unaffected (no Forage there).
4. **`WildFood` also stores `tiles: Vec<u16>`** (grass+forest count per patch), so capacity = tiles x `per_tile` follows the data without rescanning the map.
5. **The "Known" boon's delta is its own `Goodwill(0.4)` effect**, not a `goodwill_deeds` field (no duplicate data); `GoodwillDeeds::delta(Boon)` is 0.
6. **`Deed` enum instead of `sim_core::Cause`** for `GoodwillChange.cause` (cleaner, sortable; Cause carries payloads).
7. **Extra stat `FactionStats.foraged`** (not asked) for tests and 3b's wanderers.
8. **Forage ends** (pops) when the baggage is full or the land within the radius is bare, with a log line, so a queued order can follow.
9. **No goodwill for a plain `Give`** (not in the deed list); a gift counts only through 3b's BringFood request.
10. `wild::harvest` is `pub` (rustdoc links from the module doc).

### Names later parts must use
- sim_agents: `Deed`, `GoodwillChange`, `GoodwillLedger::push(settlement_spawn_id, faction, delta, deed)`, `standing::{apply_goodwill, ledger_nonempty, spread_totals}`, `data::GoodwillData` (`villagers.data.goodwill`).
- sim_military: `crate::orders::goodwill_deed(world, s, faction, Deed::Request)` for request completion (3b); `GoodwillDeeds::delta`; `progression.data.goodwill_deeds.request` (0.2).
- **Give for BringFood (3b):** `crate::orders::give(world, captain, s) -> f32` returns the delivered value; it is called from `step_order`'s settlement arm (`OrderSpec::Give { .. } => { give(world, e, s); }`). 3b should take that return value there and credit the open BringFood request of `s` (or call a `requests::delivered(world, s, faction, value)` hook from there). `FactionStats.donated` already sums it.
- Forage: `crate::wild::{WildFood, CELL, ensure_built, regrow, harvest, Harvest}`; `Progression.data.forage` (`ForageData`); `FactionStats.foraged`. The wanderers "stand on rock" branch: a band on Mountain/Hills tiles gets `Harvest::Exhausted` at the first harvest (patches count only grass and forest) and its Forage order ends; camp > 25 tiles from centres (auto-gather).
- Measured (seed 42 run, test profile): the start (coast) has 25.1 food value of wild food within 32 tiles; the band of 11 gathers 0.55/hour (3.30 over 6 harvests), i.e. ~13.2/day against a need of 9.9/day, so the spot lasts about 2-2.5 days with regrowth. Give: a 5-man army with 5 days hands over 21.6 of 27.0 and keeps 5.4 (1 day).

### Open issues
- **Loyalty balance under goodwill**: any conquest by force now starts below 0.5; big assaults (many killings) start lower. Garrison + food still recovers it. Revisit in step 11 tuning if conquests revolt too readily.
- Goodwill is per deed, flat (a 1-unit trade earns as much as a 100-unit one). Fine for M4; could scale by value later.
- `apply_goodwill` is O(settlements x changes) per tick with changes; trivial at M4 sizes.
- ADR 0014's step-2 measured numbers are kept as history; the addendum gives the new ones.

### Checks run (all pass)
- `cargo fmt --check`; `cargo clippy -j 2 --workspace --all-targets -- -D warnings`; `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps`.
- `cargo test -j 2 -p sim_agents -p sim_military`: 18 and 27 (+1 ignored).
- `cargo test -j 2 -p integration_tests --no-fail-fast --test starving_army --test posture --test orders --test famine --test village_day --test determinism --test run_start --test conquest --test docs -- --test-threads=2`: starving_army 1, posture 1, orders 7, famine 1, village_day 1, determinism 12/12 (211 s), run_start 4, conquest 4 (65 s), docs 4 (docs re-run after the last doc edit).
- **Developed invariance** (release `sim_headless`): `--seed 42 --ticks 4801 --stats-every 600` stdout byte-identical to 2c's `2c_dev.out`; `--skirmish --seed 42 --ticks 6000 --stats-every 2000` identical to `2c_skirmish.out` once `, donated: 0.0, foraged: 0.0` is removed.
- Run replay smoke: `--run --seed 42 --ticks 3000 --commands 60 --record 3a_run.ron` (61 commands, including 1 Forage and 3 Give), `--replay` and `--replay --single-threaded`: "replay ok", `0xf534af04ca371660`. `--run --seed 42 --ticks 3000 --checkpoint-every 1000`: final `0x5ea93f742528a682`; speed equal to the older binary on this box (~520 ticks/s now; the box is slower than at 2c's gate).
- Disk: ~12 GB free.

## Step 3 part 3b

Built on `9ecaaeb`; the orchestrator's checkpoint `143ddf5` already holds everything below (new files `requests.rs`, `invitations.rs`, `drifters.rs` are tracked there). Only `requests::delivered` becoming `pub` (for rustdoc) and later doc/fmt touches may be uncommitted.

### What was built
- **`orders.rs`**
  - `pub(crate) fn recruit_candidates(world, s) -> Vec<Entity>`: recruit's filter (working-age, alive, Civilian, not Enlisted, of `s`), idle first then SpawnId.
  - `pub fn enlist_into(world, villager, captain) -> bool` (root re-export `sim_military::enlist_into`): `enlist` (via `resource_scope` on Army) + baggage `capacity += baggage_per_head` + `se.population -= 1` (saturating) + `FactionStats.recruited += 1`. Refuses (false) non-villagers, the already enlisted, non-captains. `recruit` now uses both; per-villager bookkeeping sums to the old batch bookkeeping (developed/skirmish unchanged).
  - `daily_need`, `move_food`, `move_good`, `storehouse`, `spare_food`, `center`, `settlement_name`, `faction_name` are now `pub(crate)`.
  - `step_order`'s Give arm: `let delivered = give(..); crate::requests::delivered(world, s, c.faction, delivered);`.
- **`requests.rs` (new)** (GP-22): `RequestKind {DriveOff{since}, BringFood{need, delivered}, ClearCamp{monument}}` (`tag()` 0/1/2, `name()`), `Reward {food, goods: Vec<(GoodId,u32)>, volunteers, goodwill}`, `Request {id, settlement, kind, posted, deadline, reward}`, `Watch {settlement, low_food: u8, quiet_until}`, `RequestBoard {next_id, open, watch (sorted by settlement), posted, done, failed}` (hashed by hand, always present; `of(s)`). `pub fn delivered(world, s, faction, value)` (PLAYER only). `pub fn step(world)` (span `sim_military::requests`, `in_run` only), hourly, settlements in SpawnId order:
  - samples `FoodSecurity.days < requests.food_days` into `Watch.low_food` (consecutive count, reset on a good sample) every hour, open request or not;
  - an open request: DriveOff refreshes `since` while a hostile army is near; clear for `cal.hours(clear_hours)` -> Done if a PLAYER captain is within `reward_radius`, else Failed. BringFood -> Done when `delivered >= need` (reward to the nearest player captain within radius if any; goodwill regardless). ClearCamp is never detected yet (step 4). Then `tick >= deadline` -> Failed. Done/Failed set `quiet_until = tick + cooldown_hours`.
  - no open request, owner INDEPENDENT or PLAYER, not `RunProgress.rival_seat`, `tick >= quiet_until`: DriveOff if any `ArmyIndex` army with `army.at_war(f, PLAYER)` and `f != owner` has its captain within `army.threat.radius` of the centre (+0.5, as threat.rs); else BringFood if `low_food >= low_samples`, need = `min(need_days x daily_need(pop), max_need)`.
  - reward drawn at posting (`rng_key("sim_military::requests") ^ settlement` at the tick; draws: share U(0.5,1), goods pick + qty if any, volunteers): DriveOff food = `min(reward_food_days x need x share, reward_max_food, storehouse food)`; one valued non-food good (`trade_value > 0`) up to `reward_goods`; volunteers in `volunteers`. Payment via `move_food`/`move_good` (capped by stock and baggage room) and `enlist_into` over `recruit_candidates`; goodwill via `goodwill_deed(.., PLAYER, Deed::Request)` (applied by the ledger on the **next** tick).
  - events: RequestPosted / RequestDone (cause `Cause::Request{id}`) / RequestFailed, faction PLAYER, subject id, other settlement, at centre, amount kind tag. Log lines.
  - `pub(crate) fn nearest_player_captain(world, at, radius)` (ties lowest SpawnId).
- **`invitations.rs` (new)**: `pub fn step(world)` (span `sim_military::invitations`, `in_run` only, hourly): first `Offers::expire(tick)` (any kind; an expired Invitation sets the cooldown and logs); then settlements in SpawnId order post `post_offer(PLAYER, Invitation{settlement}, vec![0,1], Some(tick + hours(expiry_hours)))` when: owner != PLAYER, not the rival seat, no pending invitation of it, `tick >= RunProgress::invite_cooldown(s)`, `Goodwill.get(PLAYER) >= invitations.goodwill`, a player captain within `invitations.radius`, and weak owner (INDEPENDENT, or `SettlementLoyalty.mean < weak_loyalty` and the owner's garrison (`conquest::garrison_and_rebels`, now pub(crate)) < player soldiers within `radius`). `pub fn answer(world, faction, settlement, card) -> String`: sets the cooldown; card 0 -> `transfer(s, faction, Cause::Invitation)` (loyalty seeded from goodwill, `Invited` event); else decline.
  - `progression::choose` now answers Invitation offers through `invitations::answer`; only Hire is still "not available yet".
- **`drifters.rs` (new)**: `pub fn draw(world)` (span `sim_military::drifters`, day ticks, `in_run`): PLAYER captains in SpawnId order, each with `rng_key("sim_military::drifters") ^ captain` at the day tick; eligible settlements (owner INDEPENDENT/PLAYER, goodwill(PLAYER) >= `drifters.goodwill`) in SpawnId order within `drifters.radius` of the captain each draw `random_range(per_settlement)` (one draw per eligible settlement in range until the captain's `max_per_day` is reached), capped by the remaining day budget; the first n `recruit_candidates` join via `enlist_into`. `DriftersJoined` (subject captain, other settlement, at captain tile, amount n) and a log line per settlement; `RunProgress.drifters += n`.
- **Driver** (`progression::progress`): revolts, regrow, `drifters::draw`, `requests::step`, `invitations::step` (slots 5-7).
- **`progression/mod.rs`**: `RequestData {food_days, low_samples, need_days, max_need, deadline_hours, cooldown_hours, clear_hours, reward_radius, reward_food_days, reward_max_food, reward_goods, volunteers}`, `InvitationData {goodwill, radius, weak_loyalty, expiry_hours, cooldown_days}`, `DriftersData {radius, goodwill, per_settlement, max_per_day}` (root re-exports), validated. `RunProgress` gains `invite_cooldowns: Vec<(u64, u64)>` (sorted; `invite_cooldown(s)`, `set_invite_cooldown(s, until)`) and `drifters: u32`, both hashed.
- **progression.ron**: `requests: (food_days 0.5, low_samples 2, need_days 0.25, max_need 30, deadline_hours 36, cooldown_hours 12, clear_hours 1, reward_radius 40, reward_food_days 0.5, reward_max_food 30, reward_goods 4, volunteers (0, 2))`, `invitations: (goodwill 0.5, radius 40, weak_loyalty 0.4, expiry_hours 24, cooldown_days 3)`, `drifters: (radius 60, goodwill 0.25, per_settlement (0, 2), max_per_day 3)`.
- **lib.rs**: modules, `RequestBoard` init + `register_hashed_resource` (after WildFood), re-exports `Request, RequestBoard, RequestKind, Reward, enlist_into, RequestData, InvitationData, DriftersData`.
- **sim_core events.rs**: doc comments of RequestPosted/Done/Failed and DriftersJoined give the field meanings (docs only).
- **sim_headless stats.rs**: `pub fn run_line(&mut World) -> Option<String>`, appended to `stats::armies` in a run: `run: requests open .. posted .. done .. failed .. | invitations pending .. | drifters .. | best goodwill ..`.
- **Unit tests** (sim_military, now 32 + 1 ignored): `requests::tests::{a_hungry_village_asks_for_food_and_rewards_the_giver, raiders_near_a_village_ask_to_be_driven_off, an_unanswered_request_lapses_and_the_village_waits}`, `invitations::tests::a_village_that_likes_the_band_invites_it_in`, `drifters::tests::a_village_that_likes_the_band_sends_it_drifters`; `rejects_bad_data` covers the new blocks.
- **Docs**: DATA.md (every new field + intro sentence + `goodwill_deeds.request`), GLOSSARY (Drifter, `enlist_into`, Invitation, Request), ARCHITECTURE (crate row, schedule row 6, run-state bullet, standing bullet), TESTING (new bullets), ADR 0014 addendum "requests, invitations and drifters".

### Deviations / decisions (and why)
1. **Measure for BringFood = `FoodSecurity.days`** (storehouse + homes, what loyalty reads), with hysteresis `low_samples` (2) consecutive hourly samples. `spare_food` (storehouse only) drops to nothing every day as villagers fetch food home.
2. **Requests, invitations and drifters address only PLAYER** (only the player answers in M4). Settlements owned by another warlord and the rival's seat never ask or invite (the seat must be won by conquest, not invitation).
3. **"Hostile army"** = an army at war with PLAYER (`at_war(f, PLAYER)`: raiders, rival, beasts) whose faction isn't the settlement's owner; radius = army.ron `threat.radius`.
4. **DriveOff completion needs a player captain within `reward_radius`** when the area has been clear for `clear_hours`; otherwise the request fails ("the danger passed without the player"). BringFood counts as done without a captain near (goodwill only; nothing paid).
5. **Cooldown after any end** (done or failed), not only expiry: stops a village farming requests every hour.
6. **New field `reward_max_food`**: an uncapped DriveOff food promise for a town was ~130 food value.
7. **`enlist_into` lives in `orders.rs`** next to `enlist` (re-exported at the root), not in drifters.rs; `recruit_candidates` is the shared filter.
8. **Invitation offers expire through `Offers::expire` in the invitation step**, which removes every expired offer (only invitations have expiries now). Step 7's Hire offers, if they expire, should be handled there too.
9. **Request goodwill lands a tick after completion** (the ledger is applied in `AgentSet::Standing`, before the driver).
10. **No new FactionStats fields**; drifters count in `recruited` and in `RunProgress.drifters`; board counters `posted/done/failed` on `RequestBoard`.

### Names later parts must use
- `sim_military::{RequestBoard, Request, RequestKind, Reward, enlist_into}`, `crate::requests::{step, delivered, nearest_player_captain}`, `crate::invitations::{step, answer}`, `crate::drifters::draw`, `crate::orders::recruit_candidates`.
- Step 4: post `RequestKind::ClearCamp { monument }` in `requests::step` (posting branch; the module doc and `progress_request` mark where) and detect it in `progress_request`'s ClearCamp arm.
- Step 7: the rival levy should use `enlist_into` + `recruit_candidates`.
- 3c (wanderers / invitation scenarios): a Give by a PLAYER captain within `recruit.radius` (14) of a starving village with `FoodSecurity.days < 0.5` for 2 hourly samples completes BringFood at the next hour; Known boon (+0.4) or 2 completed requests (+0.2 each, ±spread) reach `invitations.goodwill` 0.5; drifters need 0.25.

### Open issues
- **Nobody answers requests in headless runs yet** (no bot until step 7): in `--run --seed 42 --ticks 12000`, 4 BringFood requests were posted (Ashmere sits at 0.36-0.6 days of food all run and re-asks after each 36 h deadline + 12 h cooldown) and all 4 lapsed; no DriveOff (no raiders in a run until the director, step 7); no invitations or drifters (best goodwill toward the player stays 0.00: the default pick, Hardy, isn't Known and nothing does deeds). The 3c wanderers/invitation scenarios must play the band (Give, Known boon or answered requests).
- **Chronically short villages re-ask forever** (e.g. Ashmere). Fine for a source of requests; step 7's director may want to space them (GP-16).
- **A failed DriveOff costs no goodwill.** The 2c open issue (an owner failing to protect a village) is still open; a `goodwill_deeds` field for failed requests would be the hook.
- **`run_state_hash_covers_every_sim_component` was not extended** to post a request (RequestBoard is a resource present from tick 0, so coverage holds); 3c may add a Give-answered request there.
- **ClearCamp** is defined but neither posted nor detected (step 4).

### Checks run (all pass)
- `cargo fmt --check`; `cargo clippy -j 2 --workspace --all-targets -- -D warnings`; `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps`.
- `cargo test -j 2 -p sim_military`: 32 passed, 1 ignored.
- `cargo test -j 2 -p integration_tests --test determinism --test run_start --test conquest --test orders --test docs --test starving_army --test posture --test famine --test village_day -- --test-threads=2`: determinism 12/12 (206 s), run_start 4, conquest 4 (81 s), orders 7, docs 4, starving_army 1, posture 1, famine 1, village_day 1. Log: scratchpad/3b_integ.log.
- Release `sim_headless --run --seed 42 --ticks 12000 --stats-every 2400`: exit 0, ~500 ticks/s; requests posted 4 / done 0 / failed 4, invitations 0, drifters 0 (see open issues). Output: scratchpad/3b_run.out.
- Developed invariance: release `--seed 42 --ticks 4801 --stats-every 600` stdout byte-identical to 3a's `3a_dev.out`.

## Step 3 part 3c

Built on `143ddf5`. Everything is in the working tree; **new untracked files: `tests/tests/wanderers.rs`, `tests/tests/invitation.rs`** (the orchestrator must `git add` them).

### What was built
- **`tests/tests/wanderers.rs`** (M4_PLAN scenario `wanderers`, GP-22, SIM-38, G4), run island of **seed 25** (band "Hunters and farmhands": exactly 10 followers + champion, 2.00 days of food; the first stat-only start boon, Hardy). Seed 42's band is "Outlaws" with 1 day of food, so seed 25 matches the plan's "band of 10 with 2 days of food" without editing the baggage. Played only through player commands.
  - `a_band_that_forages_and_answers_a_request_is_fed_and_grows`: setup edit before tick 0 spoils the food of the nearest hamlet (#2, 30 people, 80 tiles); SetPosture Passive + Forage (default radius 32) at tick 0; BringFood posted at tick 100 (need 6.75, reward 2 volunteers); after 4 hours of foraging (2.4) the band `Give`s (9.0 delivered), the request is done at tick 1100 (`RequestDone`, cause `Request`), 2 volunteers join; then a simple player loop forages hourly and moves (MoveTo + queued Forage) to the richest fresh ground 48 tiles away (> `upkeep.forage_radius` from every centre) when a spot runs low (2 moves). Day 5: 12 followers, 1.87 days in the baggage, worst hunger 0.25 < `upkeep.hungry` 0.6, deserted 0, no starvation. Foraged 68.7 in all.
  - `a_band_on_bare_rock_deserts_then_starves`: Mountain painted within 12 tiles of the start (nearest centre 55 tiles), MoveTo onto it, no foraging. Food out by tick 4800 (hourly sample); first desertion 6775 (asserted within `first_desertion_within_days` of the food running out, and not before it), all 10 followers desert, the champion starves to death at ~tick 16,800 (`lost` 1) and is despawned; no dead `Enlisted` entity remains; everyone gone either deserted or died. The desertion deadline is measured **from the food running out**, as M3's criterion ("an army with no food"), since the band starts with 2 days.
  - File time ~62 s (the two tests run in parallel; the rock test needs ~7 game days because only the champion can't desert and must starve).
- **`tests/tests/invitation.rs`** (scenario `invitation`, GP-22, GP-05), seed 25. Shared prefix `invited(edit)`: hamlet #2's goodwill set to 0.35 before tick 0 (below `invitations.goodwill` 0.5) and its food spoiled; passive band Gives; no invitation during the 3 hours it stands beside the hamlet below the threshold; the answered request (+0.2) lifts it to 0.55 and the Invitation is posted at tick 800 (seen at 900), the only one pending, choices [0, 1], `expires = posted + hours(expiry_hours)`.
  - `accepting_an_invitation_hands_the_village_over`: owner PLAYER, `Invited` chronicle entry (cause Invitation) on the accept tick, all 28 residents Allegiance PLAYER with Loyalty exactly `(0.55 + 1) / 2 = 0.775`, cooldown = accept tick + `cooldown_days`, a second answer refused, no further invitation.
  - `declining_an_invitation_waits_out_the_cooldown` (data edit before tick 0: `cooldown_days` 0.25, `expiry_hours` 2): decline keeps it independent with cooldown to 1500; re-invited exactly at 1500; an answer arriving on the `expires` tick (1700) is refused; it lapses, is logged, and the cooldown restarts (2300).
  - ~7 s for the file.
- **`determinism.rs` `run_state_hash_covers_every_sim_component`** extended: the settlement nearest the start other than the attacked hamlet has its food spoiled before tick 0 (helper `spoil_food_near_start`), so a request is posted (seed 42: 4 posted in the run, #11 asks BringFood); the attack's provocation/killings move goodwill (2 settlements with goodwill entries); from tick 700 (after the random orders) the champion forages (2.10). Asserts all three.
- Docs: TESTING.md rows (wanderers x2, invitation x2, the extended coverage row, the giving unit test); DATA.md progression.ron intro names the new guards and `give_keep_days` row notes the gift pause; HOWTO "Add a scenario test" step 1 (setup edits for band life, choosing a seed by its draws with `sim_headless --run --seed N --ticks 1`, pointer to wanderers.rs); GLOSSARY Give.

### Review of step 3 (`git diff 5dff41e`) and fixes
Fixed:
1. **A Give was taken straight back (logic bug).** `give` leaves exactly `give_keep_days` (1.0) days in the baggage, which is the neutral posture's auto-gather trigger (`orders::forage`: `days < 1.0`), so within ticks the band seized the gift back from the village it had just fed (and paid a Seize deed, -0.05 goodwill: seen in the first invitation run as goodwill 0.50 instead of 0.55). Fix: a gift that delivered anything sets `Captain.last_forage = tick`, pausing auto-gathering for a quarter day like any other dealing (`orders.rs::give`). `giving_hands_over_food_beyond_the_keep` now also stocks the storehouse and asserts nothing is taken back over 550 ticks while the baggage is below a day (fails without the fix, verified).
2. **An offer could be answered on its `expires` tick** (commands apply in Input, the hourly invitation step that drops lapsed offers runs later the same tick). `progression::choose` now refuses "no such offer" when `expires <= tick`. Covered by the decline scenario.
3. **Accepting an invitation for a settlement the player already holds** (e.g. conquered while the offer stood) re-ran `transfer`, reseeding every resident's loyalty and emitting an `Invited` event. `invitations::answer` now does nothing but the cooldown in that case.
4. **A settlement with no residents could post (and instantly "complete") BringFood**: need = 0.25 x daily_need(0) = 0, so `delivered >= need` at the next hour gave +0.2 goodwill for free, repeatedly after each cooldown. `requests::step` no longer posts for `population == 0`. (Reachable once recruiting/drifters/killings empty a hamlet.)
Checked and fine: every new per-settlement/captain loop sorts by SpawnId (requests, invitations, drifters, apply_goodwill, find_fallen merge+dedup); goodwill sums are order-independent (sorted ledger, per-settlement accumulation in canonical order); Stockpile goods are a BTreeMap (reward goods pick is deterministic); new RNG keys are unique per draw site and tick (`requests ^ settlement`, `drifters ^ captain` at the day tick, `wild ^ captain`); `random_range` bounds are guarded by data validation (volunteers, per_settlement, reward_goods > 0 and have > 0); `apply_goodwill` indexes two views of the same query; `requests::step` takes the board with `mem::take` and nothing inside touches `RequestBoard`; casualties' heir is always alive; no new HashMap iteration.

### Deviations
- Seed 25 instead of 42 for both new scenarios (band of 10 with 2 days of food, as the plan says). Seed 42's numbers in other tests are unchanged.
- `wanderers` (b) measures `first_desertion_within_days` from the hour the baggage is empty, not from tick 0.
- `invitation` sets goodwill (0.35) before tick 0 and earns the rest (+0.2) by answering a request, so it shows both the threshold and a deed passing it.

### Names later parts must use
- Test helpers are local to each file (`start()`, `fresh_ground`, `spoil_food`, `invited(edit)`); `determinism.rs::spoil_food_near_start`.
- `give` now sets `Captain.last_forage`; anything that relies on auto-gathering right after a Give (bot in step 7/11) must wait a quarter day or move on.

### Open issues
- `WildFood` is built at run setup from the map then; terrain painted later (tests only; nothing changes terrain at runtime in M4) isn't reflected in patch capacity. Rebuild (`WildFood::default()` + `wild::ensure_built`) if a future feature edits terrain.
- `wild::harvest` takes the full `want` from the patches even when the baggage has room for fewer units (the remainder is lost); minor.
- Still open from 3b: a failed DriveOff costs no goodwill; chronically short villages re-ask forever.

### Checks run (all pass)
- `cargo fmt --check`; `cargo clippy -j 2 --workspace --all-targets -- -D warnings`; `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps`.
- Full suite `cargo test -j 2 --workspace -- --test-threads=2` (log `3c_full.log`): exit 0. wanderers 2 (56.8 s), invitation 2 (7.4 s), determinism 12 (199 s), conquest 4 (76 s), run_start 4, orders 7, posture, starving_army, famine, village_day, army_march, worldgen 7, docs 4; sim_military 32 + 1 ignored.
- Release replay smoke: `--run --seed 42 --ticks 3000 --commands 50 --record gate.ron` (51 commands), `--replay` and `--replay --single-threaded`: "replay ok", `0xdad4147086722c5e`.
- Developed invariance: release `--seed 42 --ticks 4801 --stats-every 600` stdout byte-identical to `3a_dev.out`.

## Step 4 part 4a

Built on `5d8d584`. Everything is in the working tree; **new untracked files: `crates/sim_military/src/monuments.rs`, `docs/adr/0015-monuments-and-faction-modifiers.md`** (the orchestrator must `git add` them).

### What was built
- **`sim_military::monuments` (new)** (GP-23):
  - `Holder {faction, since, contender: Option<(FactionId, u64)>, last_presence}` (component on every monument in a run, `Default` = nobody), `Guard {monument: u64}` and `NoUpkeep` (marker). All three `register_hashed` in MilitaryPlugin (after RequestBoard); root re-exports `Guard, Holder, NoUpkeep`.
  - `pub fn setup(world, start)` (span `sim_military::monuments::setup`), called from `setup_run` right after `apply_start_modifiers`: monuments in SpawnId order get a `Holder`; each draws its guard with `rng_key("sim_military::monuments::guards") ^ monument` at tick 0 (`pick_guard`: chances laid end to end). A guard = `spawn_army(ArmySpec{faction: RAIDERS|BEASTS, food_days 0, armed, raider: false, posture Neutral})` of `guard_size = min(max, round(base + per_tile x dist(start, monument)))` followers; **every member (captain and followers) gets `Guard` + `NoUpkeep`**, so a follower who inherits command keeps the camp. The guard faction holds the monument.
  - `pub fn hold(world)` (span `sim_military::monuments::hold`), slot 2 of `progression::progress` (first call today): monuments in SpawnId order; `soldiers_at(grid, at, hold_radius)` counts FighterGrid soldiers (side.against None, not routed, faction != 0) into a sorted Vec. Holder's men present -> `last_presence = tick`. Exactly one faction present, not the holder -> contender (keeps its `since` while the same faction stays alone); `tick - since >= hours(hold_hours)` -> transfer (`MonumentTaken` faction to / other previous, `MonumentLost` for a non-zero previous, cause `Cause::Hold`, `amount` = kind, `at` = tile; log "X took N"). Otherwise contender = None. Drift: holder != 0, `tick - last_presence >= days(drift_days)` and no settlement it owns within `drift_distance` -> nobody (`MonumentLost` other 0, cause None).
  - `pub(crate) guarded(world) -> Vec<(monument, tile)>` (monuments whose camp still has a `Captain` with `Guard`), `centre_of(world, id)`.
- **NoUpkeep / Guard hooks**: `eat` query `Without<NoUpkeep>`; `soldier_upkeep` `Has<NoUpkeep>` skips hunger and starvation; `orders::forage` returns at once for `Guard` captains; `raiders::think` prey `(Without<Raider>, Without<Guard>)`. Nothing else needed for "holds its tile and fights": a captain with no orders already has goal None, and soldiers::decide engages anything at war within `engage_radius`.
- **`progression::rebuild_modifiers(world)`** (pub, root re-export, span `sim_military::rebuild_modifiers`), slot 10 (last call of `progress` today). Builds a fresh `FactionModifiers`: `monument_modifiers` (pub fn, pure: per faction != 0 ascending, kind ascending `min(count x bonus, cap)` as `ModifierSource::Monuments{kind}`; then per kind, region ascending: a set when >= `set_min` monuments of the kind in the region all held by one non-zero faction and `set_bonus > 0` -> `SetBonus{kind, region}`), then `RunProgress.boons` Stat effects -> PLAYER (`Boon{card}`), then every `Champion` (SpawnId order) `Traits` effects -> its Allegiance (`Trait{id}`), then `RunProgress.modifiers` Stat effects -> world row (`RunModifier{card}`). Writes the resource only if different. Runs in every preset (empty -> stays default).
- **Data** (`progression.ron`, `ProgressionData.monuments: MonumentData {hold_radius 10, hold_hours 2, drift_days 3, drift_distance 120, set_min 2, kinds: Vec<MonumentBonus{kind, stat, bonus, cap, set_bonus}>, guards: Vec<GuardData{kind: GuardKind{RaiderCamp, Beasts}, chance, base, per_tile, max, armed}>}`, `MonumentData::bonus(kind)`, `GuardKind::faction()`; `RequestData.camp_radius` (80). All validated (`rejects_bad_data` extended). Shipped: every kind +0.01, cap 0.1, set 0.02; Watchtower ThreatRadius 1 tile, cap 2, set 1; guards RaiderCamp 0.3 (4 + 0.02/tile, max 24, armed), Beasts 0.2 (3 + 0.015/tile, max 18, unarmed). Root re-exports `GuardData, GuardKind, MonumentBonus, MonumentData`.
- **economy.ron / sim_economy**: `RecipeDef.modifier` and `Recipe.modifier: Option<Stat>` (`#[serde(default)]`), validated to `FoodYield | SmithSpeed`. farming, fishing -> `Some(FoodYield)`; toolmaking, weaponsmithing -> `Some(SmithSpeed)`.
- **Consumers** (all exact no-ops at 1.0):
  - morale target: `soldier_upkeep` base x `mult(allegiance, Morale)`;
  - `resolve_strikes`: `(attack x bonus x roll x mult(striker, Attack) - defense).max(1) / mult(victim allegiance, Defence)`;
  - `act::perform` (sim_agents; params `owners: Query<&Owner>`, `mods: Res<FactionModifiers>`): FoodYield via `pub fn yield_units(q, mult, draw)` (stochastic rounding, stream `sim_agents::yield ^ villager` at the tick, created lazily and drawn only when there is a fraction) and private `add_output` (base units with today's capped `inv.add`, only the **bonus** beyond the recipe's units with `add_over_capacity`: G23 fixed without changing developed output, since workers may carry food and `add` caps); SmithSpeed via `pub fn smith_ticks(d, mult)`. Owner = the villager's settlement's `Owner`, else INDEPENDENT (world row only).
  - `ai.rs` `work_state` `out_units` = sum of `ceil(q x FoodYield mult)` (AiWorld gains `owners`, `mods`).
  - MarchSpeed: `move_captains` speed x mult(captain.faction); `move_soldiers` `apply` gets a `march` factor from `index.get(follower.captain).faction` (militia 1.0).
  - TradeValue: private `orders::trade_price(army, mods, faction, g)` used for both `barter`'s offer and payment. Equip's passive price is left on army.ron `trade_value` (decision, ADR 0015).
  - ThreatRadius: `update_threats` radius = `threat.radius + value(owner or INDEPENDENT, ThreatRadius)`.
- **ClearCamp requests** (`requests.rs`): on their **own track** (`camp_requests`, called at the end of `step`): the main loop now ignores ClearCamp requests (`!is_camp`), so a settlement can have one DriveOff/BringFood **and** one ClearCamp open. Open ClearCamps (settlements in SpawnId order): camp still has a captain -> open until deadline (Failed); camp gone -> Done with the nearest player captain within `reward_radius` **of the monument**, else Failed. Posting: per camp (monument SpawnId order) not named by an open request, the nearest settlement (ties lowest SpawnId) within `camp_radius` that `may_ask` (shared helper: independent/player, population > 0, not the seat), has no open ClearCamp and `tick >= Watch.camp_quiet_until`. New `Watch.camp_quiet_until` (hashed) is its own cooldown. Reward: ClearCamp promises food like DriveOff (`draw_reward`; draws unchanged for other kinds). New `RequestBoard::camp(monument)`.
- **sim_core events.rs**: MonumentTaken/MonumentLost docs give other/at/amount (docs only).
- **Tests**: sim_military `caps_and_set_bonus`, `shipped_monument_data_fits_the_island` (threat.radius + watchtower cap + set x 4 regions < worldgen run `monuments.min_settlement_distance`), `a_march_speed_trait_speeds_the_whole_band`, monuments.rs `guard_camps_hold_their_ground_and_never_hunger`, `an_unguarded_monument_is_taken_by_holding_it_and_drifts_back`, `a_cleared_camp_answers_its_request_and_frees_the_monument`; sim_agents `modifiers_scale_yields_and_cycles_and_are_no_ops_at_one`; sim_economy `recipe_modifiers_are_food_yield_or_smith_speed`.
- **Docs**: DATA.md (economy `recipes[].modifier`; progression intro, `requests.camp_radius`, `monuments` + 15 rows; trait/boon/modifier `Stat` rows no longer say "from step 4"), GLOSSARY (Stat and FactionModifiers split into two rows, Guard, Holder, NoUpkeep, Set bonus; Monument updated), HOWTO ("Add a monument type", "Add a modifier"; run-only rule step 4 links it), ARCHITECTURE (sim_agents and sim_military crate rows, schedule row 6, run-state bullet), TESTING (unit bullets), ADR 0015.

### Deviations / decisions (and why)
1. **`Guard` on every member**, not only the captain: `succeed()` moves `Captain` to a follower when the captain dies; the camp must stay a camp. "Camp gone" = no `Captain` with that `Guard`.
2. **ClearCamp is a separate track** (not "at most one request per settlement"): otherwise a camp near a village (posted at tick 0) would block BringFood for 36 h and break the band-life flows (wanderers/invitation). One open request per camp, nearest eligible settlement only (no reward spam), separate cooldown.
3. **ClearCamp completion needs a player captain within `reward_radius` of the monument** (mirrors DriveOff: cleared by someone else = Failed).
4. **Set bonus**: needs `set_min` (2) monuments of the kind in the region (a lone monument is no set), is per region, and sits **outside** the cap (the guide's formula). Zero `set_bonus` adds no entry.
5. **Yield over capacity only for the bonus part** (see consumers): switching all outputs to `add_over_capacity` would change developed output for workers carrying food.
6. **Monument presence = within `hold_radius`** for both taking and `last_presence` (drift).
7. **Guards seat the raiders' faction**: `spawn_army` seats a seatless faction, so in a run `Factions.seat(RAIDERS)` is the first raider camp. Harmless today; step 7's director should not rely on RAIDERS' seat.
8. **Factions other than 0 holding monuments get bonuses too** (guard camps: RAIDERS/BEASTS get e.g. +1% from a road marker they guard); faction 0 never.
9. `rebuild_modifiers` and `hold` run in every preset (no-ops without monuments/run data), not gated by `in_run`.

### Names later parts must use
- `sim_military::{Holder, Guard, NoUpkeep, rebuild_modifiers, MonumentData, MonumentBonus, GuardData, GuardKind}`; `sim_military::monuments::{setup, hold}`, `crate::monuments::{guarded, centre_of, soldiers_at, pick_guard, guard_size}`; `progression::monument_modifiers(&mut mods, &[(MonumentKind, RegionId, FactionId)], &MonumentData)`.
- `RequestBoard::camp(monument)`, `Watch.camp_quiet_until`.
- sim_agents `act::{yield_units, smith_ticks}`.
- **Hand-set `FactionModifiers` are overwritten at the next hour** (tick 0 included): tests change data (boons, traits, `Holder`s before tick 0) or run a consumer system once.
- 4b scenario `monuments`: guards exist from setup on the run island (seed 42: 12 camps of 24 monuments, 5-16 men). To test the cap, edit `Holder`s (and `Progression.data.monuments`) before tick 0; the first rebuild at tick 0 folds them in. An enemy retake = spawn a faction-2/3 army at a held monument with the player gone; `Cause::Hold` events.
- Step 6 (veterans/traits): champion traits already flow into modifiers; `Desertion`, `VeteranRate`, `RecruitChance`, `CaptainSlots` are summed but have no consumer yet.

### Measured
- Release `--run --seed 42 --ticks 4801 --stats-every 2400`: 12 guard camps (Beasts 5-12 men, Raiders 8-16), all holding with morale 1.0; **11 ClearCamp requests open at once** (camp_radius 80 reaches most settlements) - step 11 may narrow `camp_radius`. ~614 ticks/s.

### Open issues
- ClearCamp volume (above). Monument-level chronicle text for step 9 (`amount` = kind).
- Developed/skirmish hashes changed (economy.ron text + new hashed types); outputs are byte-identical.

### Checks run (all pass)
- `cargo fmt --check`; `cargo clippy -j 2 --workspace --all-targets -- -D warnings`; `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps`.
- `cargo test -j 2 -p sim_military -p sim_agents -p sim_economy`: 38 (+1 ignored), 19, 11.
- `cargo test -j 2 -p integration_tests --test determinism --test run_start --test conquest --test wanderers --test invitation --test orders --test posture --test starving_army --test famine --test village_day --test army_march --test docs -- --test-threads=2`: determinism 12 (136 s), run_start 4, conquest 4 (52 s), wanderers 2 (44 s), invitation 2, orders 7, posture 1, starving_army 1, famine 1, village_day 1, army_march 1, docs 4 (log scratchpad/4a_integ.log; docs re-run after the last doc edit).
- Developed invariance: release `--seed 42 --ticks 4801 --stats-every 600` stdout byte-identical to the HEAD binary's (`scratchpad/sim_headless_head4a`, `4a_head.out` vs `4a_after.out`; stderr differs only in timings); `--skirmish --seed 42 --ticks 6000 --stats-every 2000` byte-identical too.

## Step 4 part 4b

Built on `cd79299`. Everything is in the working tree; **new untracked file: `tests/tests/monuments.rs`** (the orchestrator must `git add` it).

### What was built
- **Scenario `tests/tests/monuments.rs`** (GP-23, run island of seed 42, ~4 s for both tests at test-threads=2):
  - `monuments_are_taken_held_and_lost_and_bonuses_stack_to_their_caps`: before tick 0, every unguarded monument of the kind with the most all-unguarded regional sets (seed 42: Wayshrine/`LoyaltyDrift`, 3 monuments in regions 1, 1, 3) gets `Holder.faction = PLAYER` and that kind's `cap` is lowered to 1.5 x `bonus` (shipped caps need 11 of a kind, the island has ~3), so the tick-0 rebuild gives cap 0.015 + one set 0.02 = 0.035. A 6-man player band at a quiet unguarded monument (#25 Market cross, `TradeValue`; 40+ tiles from start, camps and other monuments) takes it at tick 200 (= `hold_hours`, one `MonumentTaken`, cause Hold), bonus 0 -> 0.01 at that rebuild. The band marches off (MoveTo 45 tiles); a 12-man faction-2 army spawned before tick 0 45 tiles away is ordered in (issuer 2), stands alone from tick 1000, takes it at 1200 (`MonumentLost` PLAYER/other 2, `MonumentTaken` 2/other 1); the player's value drops 0.01 -> 0 at that hour's rebuild, faction 2's rises 0.01 -> 0.02 (faction 2 is RAIDERS; a raider camp already holds a Market cross). At every hour the test's own `expected(faction, stat)` (monuments with caps and sets, boons, champion traits, run modifiers, from the holders and data) equals `FactionModifiers::value` (1e-6).
  - `beating_a_guard_camp_takes_its_monument_and_clears_the_camp`: the smallest quiet camp with a non-seat settlement within `camp_radius` (seed 42: #26 Wayshrine, 9 beasts) has a ClearCamp at tick 0; a 37-man armed player army spawned 5 tiles away (contested at tick 0: no contender) kills the guards by tick 100; the request is done at 100 (`RequestDone`, volunteers join), the army is contender from 100 and takes the monument at 300 (`MonumentTaken` PLAYER/other 4, `MonumentLost` 4/other 1, cause Hold).
- **determinism.rs `run_state_hash_covers_every_sim_component`**: new helper `band_at_a_quiet_monument` spawns a 4-man player band before tick 0 at the first unguarded monument >40 tiles from the start and every camp; the test asserts its `MonumentTaken` by PLAYER (seed 42: #25 at tick 200) and a player entry in `FactionModifiers.breakdown`. Other numbers unchanged (hamlet at 510, 12 requests posted, foraged 4.2).
- **Docs**: TESTING.md (two monuments.rs rows, the run-coverage row's step 4 sentence and refreshed numbers, unit bullet for the new unit test), DATA.md `requests.camp_radius` (still-holds rule, lost-monument completion, camp stream, scenario named), ADR 0015 (guards routed, ClearCamp rule and stream, tests line).

### Review of step 4 (git diff 5d8d584) - fixed
1. **RNG stream reuse (D9):** a ClearCamp reward was drawn from `sim_military::requests ^ settlement` at the tick, the same stream as a DriveOff/BringFood posted by the same settlement in the same hour (possible since ClearCamp has its own track), so both rewards repeated each other's draws. ClearCamp now draws from the new key `sim_military::requests::camp ^ settlement` (`draw_reward`). Run outcomes only; add the key to the D9 table.
2. **Routed camps stayed "guarded" forever:** `monuments::guarded` listed any monument whose camp still had a `Guard` captain. A guard captain has no orders, so a camp that routes and rallies elsewhere never walks back: the player could hold the monument while settlements kept posting ClearCamp requests naming it, which could never complete before the deadline. `guarded` now also requires the monument's `Holder` to be the camp captain's faction; a camp that lost its monument counts as gone (the open request is done if a player captain is within `reward_radius` of the monument). New unit test `a_camp_that_lost_its_monument_is_no_camp_to_clear`.

### Review - checked, no change needed
- Iteration order: `hold`, `setup`, `rebuild_modifiers`, `monument_modifiers`, `camp_requests`, `guarded` all sort by SpawnId/faction/kind/region; `soldiers_at` only counts, then sorts. Float sums in `FactionModifiers` are accumulated in that fixed order.
- No-ops at 1.0: morale `base * mult`, attack `x * mult`, defence `/ mult`, march `speed * mult` (both systems), `trade_price`, threat `radius + 0.0`, `yield_units` (no draw without a fraction), `smith_ticks` (`d/1.0` rounds to d; recipe durations are validated > 0), `ai.rs` `ceil(q * 1.0)`. Morale targets above 1.0 are clamped by the existing `clamp(0, 1)`.
- Panics: none reachable (holder `expect` on a queried entity; resources always present; `faction_name` handles any id).
- Plan/guide: per-kind caps, set bonuses (beyond the cap, set_min 2), drift, guards, NoUpkeep hooks, consumers all match ADR 0015 / guide step 4.

### Open issues / notes for later steps
- `sample_orders` in a run picks among **all** captains, now mostly the 12 guard camps (issuing as faction 2/4), so `--run --commands` and the run replays mostly move guard camps around. Harmless for determinism; step 7/11 may restrict it to the player's captains in a run.
- `RequestBoard::of(s)` returns the oldest open request, which may be a ClearCamp; current callers (wanderers, run-state test) still pass.
- A guard camp that routes never returns to its monument (no orders); with fix 2 that is consistent, but step 7 could send rallied guard captains back.
- SEED.md M4 boxes are left for step 12, as in earlier steps.

### Checks run
- `cargo fmt --check`; `cargo clippy -j 2 --workspace --all-targets -- -D warnings`; `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps`: all clean.
- Full suite `cargo test -j 2 --workspace -- --test-threads=2`: all pass (11.4 min wall incl. build; log `scratchpad/4b_full.log`; determinism 12 in 129 s, monuments 2 in 4.6 s, sim_military 39 + 1 ignored).
- Release replay smoke: `--run --seed 42 --ticks 3000 --commands 50 --record SP/gate.ron` (51 commands, 584 ticks/s, hash 0x96f5df2288a5a573), `--replay` and `--replay --single-threaded` both "replay ok" with the same hash.

## Step 5 (dominion boons and captain slots)

Built on `cb79bfa`. Everything is in the working tree; **new untracked files: `crates/sim_military/src/progression/dominion.rs`, `tests/tests/dominion.rs`** (the orchestrator must `git add` them). The codemap's D9 table now lists `sim_military::requests::camp ^ settlement` (step 4b).

### What was built
- **`sim_military::progression::dominion` (new)** (GP-24, GP-25):
  - Data: `DominionData {thresholds: Vec<f32>, per_captain: u32, cap: u32, first_village_slots: u32}` with `validate()` (ascending, in (0, 1], at most 8; per_captain > 0; first_village_slots <= cap). `ProgressionData.dominion`.
  - `Holding {Farms, Smithy, LargestSettlement, Battles, Kills}` (serde, append only), `Holdings {farms, smithy, largest, battles, kills}` with `amount(h)`; `pub fn holdings(world, region)`: farms / smithy in the region's player-owned settlements, the region's most populous settlement (ties lowest SpawnId) owned by the player, `Battle` chronicle entries of PLAYER (step 6 writes them), `FactionStats.kills`.
  - `BoonData.favour: Vec<(Holding, f32)>` (`#[serde(default)]`, Start boons omit it); `boon_weight(b, h) = weight x (1 + sum(w x amount))`; `draw_dominion_boons(rng, data, holdings)` (3 distinct Dominion boons via `start::draw_cards`).
  - `RegionTally {total, held_by_player, flags: u8}` with `share()` and `reaches(threshold)` (f64, never for an empty region).
  - `pub fn tally(world)` (span `sim_military::dominion`, `in_run` only): settlements (Owner + SettlementEconomy, not NO_REGION) in SpawnId order -> `RunProgress.dominion` (len `map.region_count()`, flags kept); for each region, then threshold, first reach sets the bit and posts `post_offer(PLAYER, DominionBoon{region, step}, choices, None)` with choices drawn from `rng_key("sim_military::progression::boons") ^ offer_id` at the tick (offer id read from `Offers.next_id` first, `debug_assert` it matches); a log line.
  - `slots_for(data, held_settlements, held_pop, boon_slots, extra)` (pure) and `pub fn captain_slots(world, faction) -> Option<u32>` (root re-export): `Some` only in a run and for PLAYER: `min(cap, max(first_village_slots if any held, floor(pop / per_captain)) + sum CaptainSlots(n) of RunProgress.boons + floor(value(PLAYER, Stat::CaptainSlots) + 1e-3))`, clamped at 0.
  - `pub fn refresh_slots(world)`: stores the live value in `RunProgress.captain_slots` (run only).
- **Driver**: `progress` calls `dominion::tally` (slot 8, after invitations) and `dominion::refresh_slots` right after `rebuild_modifiers` (it reads the rebuilt CaptainSlots modifier).
- **`RunProgress`** gains `dominion: Vec<RegionTally>` and `captain_slots: u32`, both in its hand-written hash; `setup_run` initialises them empty/0.
- **`Effect::CaptainSlots(u8)`** appended. "Seat of power" is now `CaptainSlots(1)` (was `Stat(CaptainSlots, 1.0)`), so a slot boon counts the moment it is picked. `apply_boon` treats it as recorded-only and calls `refresh_slots` after every pick. `VeteranRate` stays a `Stat` boon folded into `FactionModifiers` (consumer in step 6).
- **`orders::appoint`**: after the `max_captains` check, `if let Some(slots) = captain_slots(world, issuer) && captains - 1 >= slots` -> `Err("no captain slot")`. Skirmish/developed: `None`, unchanged.
- **progression.ron**: Dominion boons get `favour` (Granary `(Farms, 0.15)`, Seat of power `(LargestSettlement, 2.0)`, Hardened `(Battles, 0.25), (Kills, 0.01)`, Smiths' guild `(Smithy, 2.0)`, Iron discipline `(Kills, 0.01)`); new block `dominion: (thresholds: [0.25, 0.5], per_captain: 300, cap: 8, first_village_slots: 1)`. Validation: favour weights >= 0; at least three Dominion boons while thresholds is non-empty.
- **sim_headless `stats::run_line`** appends `| dominion r0 12%/01 ... | captain slots N`.
- sim_core `CommandKind::AppointCaptain` doc mentions slots.
- **Tests**:
  - unit (dominion.rs): `captain_slots_follow_the_population_held` (formula incl. cap, negative, NaN, whole/half modifier), `holdings_weigh_the_dominion_draw` (weights, 400 draws distinct, Seat of power likelier with the largest settlement), `a_tally_reaches_its_thresholds_exactly` (+ DominionData validation). `rejects_bad_data` and `shipped_data_is_valid` (cap < army `max_captains`) extended.
  - `tests/tests/dominion.rs` (seed 42, ~8 s): `dominion_boons_come_once_per_threshold` and `captain_slots_follow_the_land_held` (numbers below).
  - `determinism.rs::run_state_hash_covers_every_sim_component`: asserts a tally per region with the conquered hamlet held, and stored slots == `captain_slots`.
- **Docs**: DATA.md (`boons[].favour`, `boons[].when/weight/effects` updated, `dominion` + 4 fields, intro guards), GLOSSARY (Captain slot, Dominion, Dominion boon; Boon), TESTING (two dominion.rs rows, unit bullet, coverage row, rejects_bad_data bullet), ARCHITECTURE (schedule row 6, sim_military crate row, run-state bullet), ADR 0013 addendum "dominion boons and captain slots" (+ GP-24/25 in its header), ADR 0015 consequence line about CaptainSlots.

### Measured (seed 42, test profile)
- `dominion_boons_come_once_per_threshold`: start region 1 (561 people). Pre-held hamlet #4 (10.7%) + village #16 (20.3%, starved, loyalty 0); #16 and town #11 (59%) at goodwill 0.9; a 2-man warband 35 tiles from #16. Tick 0: 30.8%, one offer `DominionBoon{1, 0}` = [Seat of power, Smiths' guild, Granary]; slots 1 with 2 captains -> AppointCaptain refused ("no captain slot"); Seat of power picked -> slots 2, appointment succeeds. Day tick 2400: #16 revolts (10.8%, flags 01), invites the warband back at once; regained -> 30.5%, no new offer, only the invitation posted. The champion walks to #11, invited at tick 3200: 89.9%, one `DominionBoon{1, 1}` = [Granary, Smiths' guild, Seat of power]; nothing more in 3 hours. ~8 s for the file.
- `captain_slots_follow_the_land_held`: 640 people held -> 2 slots (stored at tick 0), 2 appointments OK, third refused. All but the seat (4,107 people, 13 by population) -> 8; 8 offers (each region, steps 0 and 1, in region-then-step order), none more after 300 ticks; 8 appointments OK, 9th "too many captains".
- Run coverage: region 1 tally 52 of 557 held after the hamlet conquest; stored slots 1.

### Deviations / decisions (and why)
1. **`dominion::refresh_slots` runs right after `rebuild_modifiers`** (an extra call after D7 slot 10), not inside the tally (slot 8): the slots read the `Stat::CaptainSlots` modifier, which the rebuild may change. It also runs after every boon pick, so the UI sees a slot boon at once. `AppointCaptain` always uses the live `captain_slots`.
2. **Dedicated `Effect::CaptainSlots(u8)`** (appended) replaces "Seat of power"'s `Stat(CaptainSlots, 1.0)`: read from `RunProgress.boons` at the check, so a pick counts at once rather than at the next hourly rebuild. The `Stat::CaptainSlots` modifier still adds (floored, +1e-3) for traits/monuments.
3. **Holdings**: guide named farms, smithy, the region's largest settlement and battles fought. Battles = `SimEventKind::Battle` chronicle entries with faction PLAYER (none until step 6 emits them; **step 6 must write Battle with `faction` = the player's faction for this to count**); added `Kills` (`FactionStats.kills`) so fighting weighs the draw now. Farms/smithy/largest are the offer's region's; largest = by current `SettlementEconomy.population`, ties lowest SpawnId. Weight = `weight x (1 + sum(w x amount))`; `favour` is `#[serde(default)]` (Start boons omit it).
4. **Picked boons may be offered again** in a later dominion offer and stack (not excluded). Revisit in step 11 if playtests want variety.
5. **One threshold crossing per hour can post several offers** (both steps, several regions), region then step order.
6. **No new ADR number** (0016/0017 are reserved for saves/events): an addendum "dominion boons and captain slots" in ADR 0013 (runs), header now lists GP-24/25; ADR 0015's consequence line updated.
7. The test uses setup edits before tick 0 (owners via `sim_agents::set_owner`, goodwill, starving stores, a spawned warband), then only commands: the revolt is real (daily `revolts`), the regain and the 50% crossing are real invitations.

### Review of step 5 (git diff cb79bfa)
- Found while testing (test bug, fixed): the test appointed the lowest-SpawnId player follower, which was a drifter standing in the starved village, so the new captain garrisoned it and blocked its revolt. Appointments now come from the champion's band.
- Checked, no change needed: every new loop sorts by SpawnId (tally, holdings, workplaces), counts and u32 sums are order-free; the dominion stream `boons ^ offer id` at the posting tick never collides with the start offer's `boons ^ 0` (id 0 is always the start offer) or with traits; `1u8 << step` is safe (≤ 8 thresholds validated); empty regions never reach a threshold; NO_REGION settlements skipped; `tally`, `refresh_slots` and `captain_slots` are no-ops outside a run, so developed/skirmish output is byte-identical (verified) and skirmish appointments are unchanged (orders tests pass); `RunProgress` hash covers both new fields; validation guarantees 3 Dominion boons with positive weights, so an offer always has 3 distinct choices; slot boons only ever reach the player (`captain_slots` is player-only).

### Names later parts must use
- `sim_military::{captain_slots, DominionData, Holding, Holdings, RegionTally}`, `progression::dominion::{tally, refresh_slots, holdings, boon_weight, draw_dominion_boons, slots_for}`, `Effect::CaptainSlots(u8)`, `RunProgress.{dominion, captain_slots}`, `Progression.data.dominion`.
- Step 6: `Battle` events with `faction = PLAYER` feed `Holding::Battles`; `Stat::VeteranRate` (Hardened) is in `FactionModifiers` for `end_battles`.
- Step 10 UI: region shares from `RunProgress.dominion[r].share()`/`flags`; slots from `RunProgress.captain_slots`; grey Appoint when player captains − 1 ≥ slots.

### Open issues
- `--run` replays' random `AppointCaptain`s are now mostly refused (no land early); fine for determinism.
- Dominion counts all settlements of a region including the rival's seat region; the rival's lands (step 7) count toward totals as expected.

### Checks run (all pass)
- `cargo fmt --check`; `cargo clippy -j 2 --workspace --all-targets -- -D warnings`; `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps`.
- Full suite `CARGO_INCREMENTAL=0 cargo test -j 2 --workspace -- --test-threads=2`: exit 0, **208 passed, 0 failed, 1 ignored** (log `scratchpad/5_full.log`): dominion 2 (8.4 s), determinism 12 (143 s), conquest 4, wanderers 2, invitation 2, monuments 2, run_start 4, orders 7, docs 4, worldgen 7, guarded tuning tests all pass; sim_military 42 + 1 ignored.
- Release replay smoke: `--run --seed 42 --ticks 3000 --commands 50 --record SP/gate.ron` (51 commands), `--replay` and `--replay --single-threaded`: "replay ok", `0x07b32125f5d605c6`.
- Developed invariance: release `--seed 42 --ticks 4801 --stats-every 600` and `--skirmish --seed 42 --ticks 6000 --stats-every 2000` stdout byte-identical to 4a's `4a_after.out` / `4a_sk_after.out`.

## Step 6 (champion, traits, veterans, succession)

Built on `97d3959`. A first attempt was interrupted by an API spending limit; its work is in checkpoints `0823d7d` and `0caf47c` (all new files tracked there). This section was finished by the second attempt, which reviewed the whole `git diff 97d3959`, kept it, made the small fixes listed under Review, and ran every check. The orchestrator's checkpoint `72487d6` holds the events.rs doc and GLOSSARY typo fixes; uncommitted on top of it: `crates/sim_military/src/progression/champion.rs` (RankUp `at` and its unit-test assertion).

### What was built
- **`sim_military::veterans` (new)** (SIM-36):
  - `Veteran {battles: u16, rank: u8, fought: bool}` (component, hashed, root re-export) on every soldier and captain: `spawn_army`'s person bundle and `orders::enlist` (so `enlist_into`, recruits, drifters, request volunteers); `apply_desertions` removes it with Enlisted/Follower/Equipment.
  - `Captain` gains `battle_since: Option<u64>`, `last_engaged: u64`, `battle_losses: u32` (in `Captain::new` and the hand-written hash; `succeed` carries them; `appoint` copies `battle_since`/`last_engaged` to the split-off band so its fighters count the battle).
  - `resolve_strikes`: new params `ResMut<Casualties>`, `Query<&mut Captain>`, `Query<&Follower>`, and `Option<&mut Veteran>` in the main query. A blow on a living victim sets `fought` on victim and striker; after the loop every engaged person's army (`veterans::army_of`) gets `last_engaged = tick` and `battle_since.get_or_insert(tick)` (sorted+dedup, order-free). Killing blows push `(victim, striker faction)` to new `Casualties::slain_by` (cleared at the start of `resolve_strikes`).
  - `casualties`: an enlisted death bumps its army's `battle_losses` while in battle (before `succeed`).
  - `pub fn end_battles(world)` (span `sim_military::end_battles`): slot 1 of `progression::progress`, **every preset**. Captains (SpawnId order) with `battle_since` and `tick - last_engaged >= cal.hours(veterans.quiet_hours)` end the battle: living members with `fought` get `battles += 1`, `fought = false`; **in a run only** rank = `max(old, veterans.rank_for(battles, mult(faction, VeteranRate)))` with a `VeteranRank` event per promotion (subject soldier, amount rank, at captain tile). Emits `Battle` (faction = the army's faction, subject captain SpawnId, `other` = survivors who fought, `at` captain tile, `amount` = `battle_losses`), credits `Feat::Battle` if the champion is in the army, logs a line.
  - Rank effects (exact no-ops for rank 0, applied only when rank > 0): `resolve_strikes` attack `+ gain.attack`, defence `+ gain.defense`; `soldier_upkeep` morale base `+ gain.morale`, desertion chance `x (1 - gain.desertion)`.
- **army.ron `veterans: (quiet_hours: 1.0, thresholds: [1, 3, 6], names: [Recruit, Soldier, Veteran, Elite], gains: [(attack, defense, morale, desertion) x3])`**: `VeteranData` (`rank_for(battles, rate)` = thresholds reached by `battles x rate` (+1e-4), `gain(rank)`, `name(rank)`, `validate()` called from `ArmyData::validate`), `RankGain`; root re-exports `VeteranData`, `RankGain`. Gains are each rank's total (1/0.5/0.03/0.1, 2/1/0.06/0.25, 3.5/1.5/0.1/0.5).
- **New Stat consumers** (all exact no-ops at 1.0): `Stat::Desertion` (soldier desertion chance x mult, `.max(0)`), `Stat::RecruitChance` (`recruit`: `p *= mult`), `Stat::VeteranRate` (rank pace). Every Stat now has a consumer.
- **`sim_military::progression::champion` (new)** (SIM-34, SIM-35):
  - `ChampionRank {rank: u8, points: u32}` (component, hashed, root re-export), inserted with `Champion` in `setup_run`.
  - progression.ron `champion: (points: (battle: 1, monument: 2, settlement: 3, request: 1), ranks: [3, 7, 12, 18, 25])` -> `ChampionData {points: ChampionPoints, ranks}` (`points_for(Feat)`, `rank_for(points)`, `validate`), `ProgressionData.champion`. `champion_traits` is now the champion's trait pool (start trait **and** rank offers); validation requires >= 3 distinct ids.
  - `Feat {Battle, Monument, Settlement, Request}`; `pub fn credit(world, faction, feat)` (run only, faction's champion via `champion_of`). Credited in `end_battles` (champion's own army), `monuments::changed_hands` (to != 0), `conquest::transfer_seeded` (Conquest and Invited, not Revolt), `requests::complete` (PLAYER).
  - `pub fn rank_ups(world)` (span `sim_military::champion::rank_ups`, slot 9 of the driver, run only): champions in SpawnId order whose points reach new ranks: one `RankUp` (subject champion, other points, amount rank) and one `post_offer(faction, Trait{person}, choices, None)` per rank, all of that champion's offers this tick drawn in turn from **one** stream `rng_key("sim_military::progression::traits") ^ person` at the tick.
  - `pub fn draw_traits(rng, pool, held)`: 3 distinct, uniform, not-held first (via `start::draw_cards`), topped up from held ones.
  - `pub fn fallen(world, dead, faction)` (span `sim_military::champion::fallen`, run only), called from `casualties` **before** `succeed()` when the dead has `Champion`: `ChampionFell` (subject, at, cause); candidates = the faction's other living captains sorted by (ChampionRank desc, Veteran rank desc, SpawnId); if `!succession_used` and one exists: move `Champion`, `ChampionRank` and the first `len/2` traits (appended to the heir's own) to it, remove them from the dead, set `succession_used`, retarget pending `Trait{person: dead}` offers to the heir, emit `Succession` (subject heir, other fallen, at heir tile, amount kept, cause); else `RunState::finish(Lost, cause)`. Cause = `Death{striker faction}` from `Casualties::slain_by`, else `Starvation` if hunger >= `upkeep.starving`, else `Death{INDEPENDENT}`.
- **Driver**: `progress` = end_battles, hold, revolts, regrow, drifters, requests, invitations, tally, **rank_ups**, rebuild_modifiers, refresh_slots.
- sim_core docs only: events.rs field meanings for RankUp, VeteranRank, ChampionFell, Succession, Battle; modifiers.rs Stat docs.
- **Tests**: unit `veterans::tests::{a_blow_marks_both_sides_and_ranks_change_the_damage, a_battle_ends_after_a_quiet_hour_and_ranks_its_fighters, ranks_follow_battles_and_the_veteran_rate}`, `champion::tests::{trait_draws_prefer_new_traits, points_past_two_ranks_post_two_trait_offers}`, `rejects_bad_data` (champion) and `rejects_bad_conquest_and_garrison_data` (veterans) extended. Integration `tests/tests/champion.rs` (seed 42, ~4.5 s for the file): `a_won_battle_ranks_the_champion_up_and_its_pick_applies`, `soldiers_who_fought_rank_up_with_the_configured_gains`, `the_champion_falls_once_to_a_successor_then_the_run_is_lost`. `determinism.rs::run_state_hash_covers_every_sim_component` asserts the warband's assault ends as a PLAYER `Battle` (tick 300, 4 fought), ranked veterans exist, and the champion holds the conquest+monument points (5, rank 1).
- `RankUp` events carry `at` = the champion's tile (second attempt; for the feed's camera jump).
- **Docs**: DATA.md (army.ron `veterans` + 4 rows and header; progression.ron `champion` + 2 rows, `champion_traits` and `traits[].effects` rows (consumer list), guards), GLOSSARY (Battle, Champion updated, Champion rank, Succession, Trait updated, Veteran rank, Stat, Progress driver slots), HOWTO "Add a trait or boon", TESTING (3 champion.rs rows, coverage row, dominion row note, unit bullets), ARCHITECTURE (Act and Needs schedule rows, sim_military crate row, run-state bullet), ADR 0013 addendum "champion, traits, veterans and succession" (+ SIM-34/35/36 in header), ADR 0015 consequence line.

### Deviations / decisions (and why)
1. **`end_battles` runs in every preset** (battle bookkeeping, `Battle` events, `battles` counts, `fought` cleared); only **ranks** (and so gains, `VeteranRank`, champion points) are run-only. Developed, skirmish and battle stdout are byte-identical to HEAD (verified); only hashes change (Captain fields, Veteran, army.ron text, skirmish chronicle gains Battle entries).
2. **Battle state is updated from `resolve_strikes`** (every landed blow, both sides), not from `ArmyInfo.fighting`: exact ("struck or was struck"), no one-tick lag, and the captain alone counts too.
3. **`VeteranRate` scales battles at ranking time** (`battles x mult`), no RNG and no fractional storage; ranks never drop.
4. **New data fields** beyond the guide: `veterans.quiet_hours` (the "hour" of the plan as data) and `veterans.names`; `Captain.battle_losses`; `Casualties::slain_by` (exempt resource, for the cause of death).
5. **Rank-up offers reuse `champion_traits`** (now "the champion's trait pool") rather than a second list; Shaken (6) stays out of the pool. A held trait can be re-offered only when fewer than three new ones remain (it stacks).
6. **Battle points go only to the champion's own army's battles** ("battles survived"); monument/settlement/request points go to the faction's champion whoever did the deed.
7. **Succession also moves pending trait offers** to the heir and keeps the heir's own traits ahead of the kept half; rank and points move whole.
8. **`dominion.rs` test adjusted** (test only): the accepted re-invitation now also earns settlement points, so a champion trait offer may be posted with the invitation; the assertion counts those (the "no second dominion offer" check is unchanged). The town's dominion offer id moved from 4 to 5.
9. Traits gained by soldiers/captains from triggers (SIM-34 "what they live through") are **not** in this step: traits come only from the champion's start and ranks. Noted in the ADR addendum.

### Names later parts must use
- `sim_military::{Veteran, VeteranData, RankGain, ChampionRank, ChampionData, ChampionPoints}`, `sim_military::veterans::end_battles`, `sim_military::progression::champion::{credit, rank_ups, fallen, draw_traits, Feat}`, `Casualties::slain_by`, `Captain.{battle_since, last_engaged, battle_losses}`, `Army.data.veterans` (`rank_for`, `gain`, `name`), `Progression.data.champion`.
- Step 7: rival levies via `enlist_into` get a `Veteran` automatically. Director raider bands' battles report `Battle` (faction RAIDERS).
- Step 9: battle reports can hang off `end_battles` (it already has faction, captain, fought, losses, promoted list); Elites' names via `veterans.name(rank)` and `Soldier.name`/`Villager.name`. `VeteranRank` is per soldier (a big battle floods the 256-entry EventFeed: group them in step 9).
- Step 10 UI: `ChampionRank` (rank, points, next = `champion.ranks[rank]`), `Traits`, `Veteran` (rank name via `Army.data.veterans.name`).

### Open issues
- `VeteranRank` events are one per soldier; a large run battle can push hundreds into the EventFeed ring in one tick (not the Chronicle). Step 9 grouping.
- An army destroyed to the last man reports no `Battle` (no captain left to end it).
- Soldiers' trait triggers (SIM-34) not implemented (see deviation 9).

### Measured (seed 42, test profile, `--test champion --nocapture`)
- Champion's band vs 3 weak raiders: battle ends at tick 200 (9 fought, 0 fell), points 2 -> 3 = rank 1, offer of traits [Feared, Duelist, Forager] (held [Brave]); Feared picked: Attack 0 -> 0.03 and LoyaltyDrift 0 -> -0.05 at the next hour, not before.
- 20-man warband vs 10 raiders: battle ends at 200, 18 of 19 survivors fought, 2 fell (Battle amount 2 = ArmyStats lost); 18 `VeteranRank` 1; morale target ranked 0.970 vs unranked 0.940.
- Succession: champion 6081 killed at tick 2 (Death by 0) -> captain 6226 (veteran rank 2, beats the lower-SpawnId recruit captain) is champion with rank 2, points 7, traits [Brave] (half of 3) and the pending offer; it falls at 101 -> Lost, clock and hash frozen.
- File time ~4-6 s.

### Review of step 6 (git diff 97d3959)
Fixed (second attempt):
1. `RankUp` had no `at` (default 0,0): the feed's click-to-jump would have sent the camera to the map corner. Now the champion's tile; `points_past_two_ranks_post_two_trait_offers` asserts it.
2. GLOSSARY Stat row typo.
Checked, no change needed:
- Determinism: `end_battles` (captains sorted, `members` sorted), `rank_ups` (sorted), `fallen` (candidates sorted by (rank desc, veteran rank desc, SpawnId)), offers retargeted in Vec order; `resolve_strikes` sets the same tick on every engaged army (sort+dedup, order-free); ranks scale battles by `VeteranRate` with no RNG; the trait stream `traits ^ person` at the posting tick never collides with the start trait's unkeyed `traits` at tick 0 (person SpawnIds are never 0); `Casualties::slain_by` is transient (cleared every tick by `resolve_strikes`, which is not gated) in an exempt resource; Captain's new fields, `Veteran` and `ChampionRank` are hashed; no HashMap iteration.
- No-ops outside runs/at 1.0: `p *= mult(RecruitChance)` (and `random::<f32>() < p` tolerates p > 1), `desert * mult(Desertion)` with `.max(0)`, rank 0 has no gain, ranks only rise in a run. Developed, skirmish and `--battle 1000` stdout byte-identical to the step-5 binary.
- Logic: `fallen` runs before `succeed`, so succeed's heir (still a follower) is never a candidate; candidates must be alive, so a captain dying the same tick (either SpawnId order) is skipped; `battle_losses` is bumped before `succeed` clones the Captain; `appoint` carries the battle into a split-off band; `apply_desertions` removes `Veteran`; a Trait offer for a missing person is taken without effect (run already lost in that case).
- `Cause` of the loss is `Death { striker faction }` for a blow, `Starvation` for hunger (the guide says `Cause::Death`; Starvation is kept as the more exact cause for the chronicle).

### Checks run (all pass)
- `cargo fmt --check`; `cargo clippy -j 2 --workspace --all-targets -- -D warnings`; `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps`.
- Full suite `CARGO_INCREMENTAL=0 cargo test -j 2 --workspace -- --test-threads=2`: exit 0 (log `scratchpad/6_full.log`): champion 3 (4.1 s), determinism 12 (126 s), conquest 4, dominion 2, wanderers 2, invitation 2, monuments 2, run_start 4, orders 7, docs 4, worldgen 7, army_march, posture, starving_army, famine, village_day; sim_military 47 + 1 ignored.
- Release replay smoke: `--run --seed 42 --ticks 3000 --commands 50 --record SP/gate.ron` (51 commands), `--replay` and `--replay --single-threaded`: "replay ok", `0x218888081e2ae170`.
- Developed invariance: release `--seed 42 --ticks 4801 --stats-every 600` and `--skirmish --seed 42 --ticks 6000 --stats-every 2000` stdout byte-identical to step 5's `5_dev.out` / `5_sk.out`; `--battle 1000 --seed 42 --ticks 3000 --stats-every 600` identical to the step-5 binary (`sim_headless_head6`).

## Step 7 (the rival and the pacing director, GP-28, GP-16)

Built on `5539233`. Earlier agents were stopped mid-step; their work is in checkpoints `e3b3633`, `09f1481` and `e783d87` (all new files tracked: `crates/sim_military/src/{rival,director}.rs`, `assets/data/director.ron`, `crates/sim_headless/src/bot.rs`, `tests/tests/{rival,director}.rs`). The finishing agent reviewed the whole `git diff 5539233`, kept it, made the fixes under "Review", and ran every check. Its changes are uncommitted on top of `e783d87` (bot.rs, main.rs, progression/mod.rs, rival.rs, determinism.rs, rival.rs test, DATA/TESTING/ADR 0013). An earlier draft of this section is `scratchpad/7_progress.md`; this one supersedes it.

### What was built
- **`sim_military::rival` (new, GP-28)**
  - `RivalData` (a row of progression.ron `rival`, one per `Difficulty`): `difficulty, start_loyalty, garrison, garrison_food_days, seat_radius, levy_every_days, levy_size, defence_steps: Vec<(day, bonus)>, raid_from_day, raid_every_days, raid_size`, with `validate()` and `defence_on(day)`. `ProgressionData.rival: Vec<RivalData>`, `Progression::rival(difficulty)`; validation requires exactly one row per difficulty. Shipped: steady garrison 24, levy 6 every 2 days, defence +0.25/0.5/0.75/1.0 from days 5/10/20/30, first raid day 10 then every 3 days, raid_size 10; chaotic raids from day 8 every 2 (12); harsh garrison 32, levy 8 every 1.5 days, defence from +0.25 at day 0 to +1.5 at day 24, raids from day 8 every 2.5 (14).
  - `RivalState {seat: Option<u64>, seat_center, defence_bonus, seat_radius, next_levy, next_raid, raids, levied}` (resource, always present, hashed) with `seat_defence(faction, at)`.
  - `RivalBand {target: Option<u64>, next_think}` (component, hashed) on raiding-band captains; `combat::succeed` copies it to the heir.
  - `setup(world, seat, seat_center, region)` from `setup_run` (after the band, guard camps and wild food, before the director and the start offer, so earlier SpawnIds are unchanged): `Factions.set_seat(RIVAL, centre)` first; every populated settlement of the region goes to RIVAL through `sim_agents::set_owner(.., Some(start_loyalty))` (quiet: no Conquest event or count); garrison = `spawn_army(RIVAL, armed, round(garrison x strength) followers)` with a standing `Garrison{seat}` pushed directly; RivalState initialised (`next_levy = days(levy_every_days)`, `next_raid = days(raid_from_day)`, bonus `defence_on(0) x strength`).
  - `strength(world)` = product of the run modifiers' `RivalStrength`; `clock_data(world)`; `garrison(world)` (RIVAL captain without RivalBand, Garrison-on-seat first, then nearest the seat, then SpawnId).
  - `clock(world)`: driver slot 11, run only. While RIVAL owns the seat: bonus = `defence_on(day) x strength`; levy when due (`levy`: `recruit_candidates` per RIVAL settlement in SpawnId order, round-robin one each, `enlist_into` the garrison; a wiped-out garrison is re-spawned at the seat; `RivalLevy` (subject garrison captain, at seat, amount); `levied`); raid when due (`raid`: lead drawn from `rng_key("sim_military::rival")` at the tick; `min(raid_size, followers/2)` people via `orders::split_army(.., Some(n-1), Aggressive)`; `RivalBand`; `RivalRaid` (subject lead, at seat, amount); `raids`). A seat it no longer holds: bonus 0, clock waits; missed levies/raids are skipped.
  - `think` (system, `SimInputSet::Ai`, chained after `raiders::think`, SpawnId order, every 300 ticks, run only): nearest of PLAYER-owned settlements (Attack; GatherFood first and Attack queued when < 1 day of food) and PLAYER-held monuments (MoveTo), ties lowest SpawnId; none: MoveTo the nearest PLAYER captain; none: Garrison the seat. Re-issues only when the target changes or the band is idle. `SimCommand::ai`, never recorded.
  - `free_lands(world)`: RIVAL settlements to INDEPENDENT via `set_owner` (quiet), called from `conquest::transfer_seeded` when `finish(Won)` returns true.
- **Seat defence**: `resolve_strikes` divides damage by `mult(victim, Defence) + RivalState::seat_defence(victim faction, victim tile)` (looked up only when `defence_bonus > 0`; exact no-op otherwise).
- **`orders::split_army(world, old, f, take: Option<u32>, posture) -> u32`** (pub(crate)) extracted from `appoint`: `None` = appoint's behaviour (odd-numbered followers, half of each good), `Some(n)` = first n odd then even ones, baggage by head count. Appoint is byte-identical (orders tests and developed/skirmish output unchanged).
- **`sim_military::director` (new, GP-16)**, `assets/data/director.ron`:
  - `DirectorData {personalities: Vec<Personality>, spawn_distance: (f32, f32), band_food_days}` (resource, hashed as RON; `from_ron`, `default_data`, `validate`, `personality(d)`). `Personality {difficulty, weights: TensionWeights{threat, hunger, fights, quiet}, quiet_limit_hours, first_event_within_hours, spacing_hours, raider_band: RaiderBandData{min, max, per_day, armed}, max_bands, auto_pause: Vec<SimEventKind>}`. Steady: quiet limit 10 h, first event 4 h, spacing 30 h, bands 5..16 (+0.5/day, unarmed), max 2; chaotic 8/3/16 h, 4..20 (+0.75, armed), max 4; harsh 8/3/20 h, 8..24 (+1, armed), max 3.
  - `Director {tension, last_spawn, next_check, first_event, bands}` (resource, hashed). `setup(world)` from setup_run sets `next_check = hours(spacing / pace)`.
  - `step(world)`: driver slot 12 (last), run only. Records `first_event` (first `eventful` chronicle entry's tick, or now if `RequestBoard.posted > 0`); tension = `wt*threat + wh*hunger + wf*fights - wq*quiet` (threat 1 if an at-war army is within threat.radius of a player army, else the max Threat.level of player settlements; hunger = 1 - player baggage days/2; fights = a player captain in battle; quiet = (tick - last_notable)/limit, capped 1). Brings a band when `quiet_for + hour > hours(quiet_limit)`, or no first event and `tick + hour > hours(first_event_within)`, or tension < 0 with `tick >= next_check` and fewer than `max_bands` Raider captains.
  - `bring_band`: target = populated settlement nearest the champion owned by PLAYER (preferred) or nobody, not the seat; tile = up to 24 seeded tries (`rng_key("sim_military::director")` at the tick) at `spawn_distance` around the target (or the champion), `nearest_passable`, region not NO_REGION nor the rival's; `spawn_army(RAIDERS, Aggressive, raider: true, armed, size_on(day))`, `Raider.target = target`; `RaidersArrived` (subject captain, other target, at tile, amount size).
  - `eventful(kind)`, `raider_pace(world)` (product of `RaiderPace`, floor 0.01), `personality(world)`.
- **`Raider.target: Option<u64>`** (+ tag byte in its hash). `raiders::think`: a target is raided first (GatherFood once, then cleared). In a run: prey = PLAYER captains whose army is no bigger than the band; villages = PLAYER-owned only; hungry with no player village: hunt anyway. Outside a run unchanged.
- **Validation**: every `RaiderPace(x)` / `RivalStrength(x)` effect in boons and modifiers must be finite and > 0 (finishing agent).
- **sim_headless**:
  - `bot` module: `Bot::new(seed)`, `Bot::act(&mut App) -> Vec<SimCommand>` (every `THINK_EVERY` = 100 ticks: offers with choice 0, then food first (forage below a day of food, wander 48 tiles from a bare spot), then requests (Give for BringFood when it can spare food; MoveTo a DriveOff village when it outnumbers the hostiles by half again), then the nearest unheld monument away from guard camps, else forage below 3 days), `bot::step(&mut Bot, &mut App)`, `run_bot(config, max_ticks, checkpoint_every) -> BotRun {outcome, cause, end_tick, log, checkpoints, final_hash, chronicle, notes}`.
  - `step_until_event(app, max_ticks) -> Option<u64>` (stops on a change of `Chronicle::last_notable`, or a notable event at tick 0; None if the clock stops or nothing happens).
  - CLI `--bot` (needs `--run`, not with `--commands`). Plain: the bot plays and prints outcome and hash; stderr lists its notes and a chronicle summary by kind. With `--record` / `--stats-every` / `--timings` the bot's log is replayed through the normal path; with `--record` its own checkpoints must match the replay's (exit 2 otherwise). `--stats-every` in a run stops with a `run ended: <outcome> at tick N (<cause>)` line once the run is over (developed/skirmish never end, so their output is unchanged).
  - `sample_orders` in a run gives orders to PLAYER captains only; `stats::run_line` gains the rival's lands/people, garrison, defence, levied, raids and the director's tension and bands.
- **Tests**: `tests/tests/rival.rs` (2), `tests/tests/director.rs` (2), `determinism.rs::a_bot_run_replays` (new, finishing agent), unit tests `rival::tests::{the_seat_guards_only_its_own_near_it, defence_steps_and_shipped_rows, levies_fill_the_garrison_and_raids_split_it}`, `director::tests::{shipped_data_is_valid_and_bands_grow_by_day, rejects_bad_data}`, `progression::tests::rejects_bad_data` (rival rows, positive factors); `run_state_hash_covers_every_sim_component` moves the first levy/raid to ticks 200/300 and asserts a levy and a RivalBand; `common::quiet_director` silences the director in scenario tests (conquest's loyalty branches use it).
- **Docs**: DATA.md (progression `rival` + 11 field rows; `modifiers[].effects` RaiderPace/RivalStrength; `## director.ron (\`sim_military::DirectorData\`)` with every field and the `Raider` target), GLOSSARY (Director, Levy, Raiders, Raiding band, Tension; Rival, Progress driver updated), ARCHITECTURE (crate rows, schedule rows 1 and 6 incl. driver slots 11-12, run-state bullet), TESTING (5 rows, unit bullets, coverage row), HOWTO (run scenario recipe, run-only rule), ADR 0013 addendum "the rival and the pacing director" (+GP-16, GP-28 in its header).

### Deviations / decisions (and why)
1. **progression.ron `rival` is a list of full rows** (every field per difficulty, including `start_loyalty`, `garrison`, `garrison_food_days`, `seat_radius`), not shared fields plus a clock; new fields `garrison`, `garrison_food_days`.
2. **Rival lands are handed over quietly** with `set_owner`, not `transfer` (which would report 8 Conquests and count them). `free_lands` on a win is quiet too (the run has ended).
3. **`RivalState.seat` is `Option<u64>`** (None outside a run) and carries `seat_radius`, so `resolve_strikes` needs only `Res<RivalState>`.
4. **Raids**: band = min(raid_size, garrison followers / 2); a garrison under 4 followers sends nobody. Levied villagers walk to the seat from the rival's other settlements; a few die on the way (beast camps), which the rival test's books account for.
5. **Director**: bands forced by quiet or the first-event deadline ignore spacing and `max_bands`, so the quiet limit is a hard guarantee; the "nudge a request" is the band's target village, which posts a real DriveOff on arrival. New fields `max_bands`, `raider_band.armed`, `spawn_distance`, `band_food_days`; `per_day` = band growth per day of the run.
6. **Raiders in a run hunt only player armies no bigger than themselves** (otherwise every director band hunted a band of ten to death by day 2-3) and raid only the player's settlements.
7. **`Director.first_event`** stores the event's own tick (chronicle entry tick, or the hour a request was first posted).
8. **`run_bot` takes `checkpoint_every` and returns `checkpoints` and `chronicle`** (guide signature; `cause`, `final_hash`, `notes` kept). The rival test uses a shortened clock (levies every half day, raids from day 1) to stay under 15 s, and checks the shipped clock's first levy (tick 4800) and raid (tick 24000, day 10) only as scheduled.
9. **Bot priorities** (finishing agent): food first, then requests, then monuments. With requests first (the original skeleton) the band of seed 42 stood by a raided village at 0.1 days of food answering DriveOffs and lost on day 4; foraging first it lives through day 10.
10. Steady raiders are unarmed (`armed: false`); chaotic and harsh armed.

### Review (finishing agent, `git diff 5539233`)
Fixed:
1. `rival::raid` doc said a garrison of fewer than two followers sends nobody; it is fewer than four (band of >= 2 at half the garrison).
2. `RaiderPace(0)` would make the director's spacing infinite (masked by the 0.01 floor) and `RivalStrength(<= 0)` a negative bonus or empty garrison: validation now refuses non-positive or non-finite factors (DATA.md row says so; `rejects_bad_data` checks it).
3. No test ran `run_bot`: added `a_bot_run_replays` (its log reproduces its checkpoints, serially too), and `--run --bot --record` now checks the replay against the bot's own checkpoints.
4. Bot priority (deviation 9); `--stats-every` kept printing the frozen state after a run ended (now one `run ended` line and stop); `--bot` prints a chronicle summary.
5. The rival test now also checks the shipped clock's first levy/raid ticks and starting bonus.
Checked, no change needed: every new loop is SpawnId-sorted or order-free (`think`s, `levy`, `raid`, `garrison`, `free_lands`, `setup`, `bring_band` target with total keys, ArmyIndex armies sorted by id for the tension sums); new RNG keys only (`sim_military::rival`, `sim_military::director`, `sim_headless::bot`); RivalState, RivalBand, Director, DirectorData registered hashed and `Raider.target` in its hash; no panics from real states (`raid` indexes a non-empty list, `size_on` clamps a validated min <= max, spawn tries are bounded, clock loops advance by `max(1)`); outside a run every new path is a no-op (developed, skirmish and battle stdout byte-identical).

### Measured
- `rival` (seed 42, steady, "weaker rival" 0.8; ~15 s for the file): lands #1, 2, 8, 9, 15, 18, 19 (seat), 22, loyalty 0.8, garrison 20 (19 + captain). Shipped clock: first levy tick 4800, first raid tick 24000. Shortened clock: levies of 5 every 1200 ticks, defence 0.20 / 0.40 / 0.80, garrison peak 25; first raid at 2400 (10 people, garrison 18 left), 3 raids by 5400; the bands MoveTo the player's band. Seat taken by a 150-man army at tick 1888: Won, cause Conquest, 7 other lands independent, clock and hash frozen.
- `director` (~14 s): three days quiet-checked every 25 ticks, longest quiet 999 of 1,000 ticks; first event tick 0 (requests); bands at 1900 and 3400. With requests off: stops at 0 (run start), 400 (RaidersArrived, deadline 400), 800 (RequestPosted), 1300 (RequestFailed).
- `a_bot_run_replays` (~10 s): 3 commands in 1,500 ticks (start boon, Monument(34), Forage at 400), checkpoints 500/1000/1500 equal on replay.
- **Release `--run --bot --seed 42 --ticks 24000 --stats-every 2400`** (steady; Outlaws, 10 + champion, 1 day of food; modifiers Weaker rival + Restless roads; boon 0 "Hardy"): bot alive at tick 24000, 53 commands. Bot: forages from tick 400, walks to the nearest unheld monuments (34, 33, 45, 43, 46; the chronicle has 6 MonumentTaken and 5 MonumentLost, all factions), 3 rank-ups with trait picks (6300, 9200, 18500), answers DriveOffs (village #21, later #0), one request done (21000); band 10 -> 7 (day 4) -> 5 (day 8) -> champion alone (day 10), 24 kills, 12 lost, 62 food foraged. Director: 6 bands (1900, 3400, 8400, 13200, 21300, 23900), tension -0.51..1.17 at the day lines. Rival: 8 settlements (~1,780 people), levies of 5 at days 2, 4, 6, 8 (levied 20; garrison 20 -> 26, some levied men lost on the way), defence +0.20 from day 5; no raid yet (the first is due on tick 24000, one past the window). 29 battles, 37 requests failed.
- Other seeds (steady, 24,000 ticks, plain `--bot`): seeds 1, 2, 3 lost to raiders at ticks 18873, 21635, 20751; seeds 4, 5 alive. Harsh seed 42: lost at tick 4325 (three bands by 3600). Plain bot run ~16 s for 24,000 ticks (release).

### Names later steps must use
- `sim_military::{RivalState, RivalBand, RivalData, DirectorData, Director, Personality, RaiderBandData, TensionWeights}`, `sim_military::rival::{setup, clock, think, garrison, strength, clock_data, free_lands}`, `sim_military::director::{setup, step, eventful, raider_pace, personality, DEFAULT_DIRECTOR_RON}`, `crate::orders::split_army`, `Progression::rival(d)`, `Raider.target`, `common::quiet_director`.
- `sim_headless::{step_until_event, bot::{Bot, BotRun, run_bot, step, THINK_EVERY}}`; `run_bot(config, max_ticks, checkpoint_every)`.
- Step 8 (saves): save `RivalState`, `RivalBand`, `Director`, `Raider.target`, `DirectorData` (data, hashed as RON).
- Step 9: `BotRun.chronicle` for the chronicle test; `RivalLevy` is not chronicled (feed only).
- Step 10 (UI): auto-pause kinds from `DirectorData.personality(..).auto_pause`; fast-forward = `step_until_event` logic (stop on a `Chronicle::last_notable` change); run bar: `RivalState` (defence, next_levy, next_raid), `rival::garrison`.
- Step 11 (bot): `Bot::choose` is the strategy; `--run --bot --record` already checks the replay against the bot's checkpoints.

### Open issues
- **The skeleton bot is not a winner**: 3 of 5 seeds lose to director raiders by day 9 (they hunt the player's band once it is no bigger than theirs, and bands grow by `per_day`); it oscillates between foraging (to about one day of food) and walking to a monument, so it holds few. Step 11.
- A director (or M3) raider band whose captain falls loses its AI: `Raider` (and a guard camp's `Guard`) stays on the dead captain, so the heir's army idles; `RivalBand` is moved. Moving `Raider` would change skirmish output unless gated by `in_run`.
- Notable kinds count events anywhere on the island (a Battle between a guard camp and director raiders resets the quiet clock), and a player's own `OfferChosen` counts too.
- Rival garrison food: lives on the seat's spare food (supply ~225 food value by day 10 on seed 42, no starvation); levied men walking in are sometimes killed on the way (lost 11 by day 9).
- RAIDERS' seat is still set by the first guard camp (step 4 note); nothing reads it.
- Rival clock catch-up: if the rival loses and retakes its seat, missed levies/raids are skipped.

### Checks run (all pass, final tree)
- `cargo fmt --check`; `cargo clippy -j 2 --workspace --all-targets -- -D warnings`; `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps`.
- Full suite `CARGO_INCREMENTAL=0 cargo test -j 2 --workspace -- --test-threads=2`: exit 0 (log `scratchpad/7f_full.log`): rival 2 (14.9 s), director 2 (15.6 s), determinism 13 (140.7 s, incl. `a_bot_run_replays` ~10 s alone), conquest 4 (51.4 s), champion, dominion, wanderers, invitation, monuments, run_start, orders, docs, worldgen, army_march, posture, starving_army, famine, village_day; sim_military 52 + 1 ignored.
- Release replay smoke: `--run --seed 42 --ticks 3000 --commands 50 --record SP/gate.ron` (51 commands), `--replay` and `--replay --single-threaded`: "replay ok", `0x43d058361cb7a228`. Bot replay: `--run --bot --seed 42 --ticks 6000 --record SP/7_botrun.ron` (12 commands, bot checkpoints matched), both replays ok, `0xea97639b02eabf7b`.
- Developed invariance: release `--seed 42 --ticks 4801 --stats-every 600` and `--skirmish --seed 42 --ticks 6000 --stats-every 2000` byte-identical to `5_dev.out` / `5_sk.out`; `--battle 1000 --seed 42 --ticks 3000 --stats-every 600` identical to the step-6 binary (`sim_headless_head6`).

## Steps 8-9

Built on `a4abbd6` by one agent, step 8 then step 9. Everything is in the working tree. **New untracked files (the orchestrator must `git add` them):** `crates/sim_core/src/save.rs`, `crates/sim_military/src/save.rs`, `crates/sim_military/src/chronicle.rs`, `tests/tests/save_load.rs`, `tests/tests/chronicle.rs`, `docs/adr/0016-save-format.md`, `docs/adr/0017-events-and-chronicle.md`. `Cargo.lock` changes (postcard 1.1.3, cobs, zstd 0.13.3, zstd-safe, zstd-sys, cc, jobserver, ...). game_ui, game_render and game_app are untouched.

### Step 8: what was built (GP-12, NFR-07, ADR 0001 / ADR 0016)
- **Dependencies:** `postcard = 1.1.3` (`default-features = false, features = ["use-std"]`) and `zstd = 0.13.3` (`default-features = false`) in `[workspace.dependencies]` and sim_core only (approved in ADR 0001 / M4_PLAN). No bincode.
- **`sim_core::save`** (root re-exports `AppSaveExt, SAVE_FORMAT, SAVE_MAGIC, SaveError, SaveHeader, SaveReader, SaveRegistry, SaveState, SaveWriter, load_world, read_header, save_world`):
  - `SaveState { fn save(&self, &mut SaveWriter); fn load(&mut SaveReader) -> Result<Self, SaveError> }`, implemented for u8..u128, i32, i64, f32, f64, bool, usize, IVec2, Vec2, Entity, String, Option, Vec, VecDeque, `Arc<[T]>`, `[T; N]`, 2-4 tuples, BTreeMap, BTreeSet.
  - `SaveWriter` (`raw, bytes, count, bool, u8..u128, i32, i64, f32, f64, ivec2, vec2, str, entity, serde, ron, save`) and `SaveReader` (the same as readers, plus `remaining`). Lengths are `count()` (clippy forbids `len` without `is_empty`). `entity` writes `SpawnId + 1` (0 = none/despawned/no SpawnId) and reads back the entity now holding that SpawnId, or `Entity::PLACEHOLDER`.
  - Macros (exported at the sim_core root): `save_fields!(T { a, b })`, `save_newtype!(T, U)`, `save_unit!(T)`, `save_serde!(T)` (postcard, variant index: append only).
  - `AppSaveExt::{register_saved, register_saved_resource, register_saved_resource_with(save_fn, load_fn), on_load(fn(&mut World)), save_preset_name(fn(&World) -> String)}`; `SaveRegistry` (`contains(TypeId)`, `saved_types()`, `resource_names()`, `component_names()`).
  - `HashRegistry::hashed_types()`: hashed TypeIds and names, exempt ones excluded (new `hashed` map beside `covered`).
  - `SaveHeader {magic, format, build, seed, preset, run, tick}` (postcard; `SAVE_FORMAT` = 1); `read_header(bytes)`.
  - `save_world(&mut World) -> Vec<u8>`: header + zstd(level 3, checksum) payload = SpawnIds ascending, then resources in registration order (name, presence, length-prefixed blob), then components in registration order (name, count, blob of `(entity index, value)` in SpawnId order).
  - `load_world(&mut World, &[u8]) -> Result<(), SaveError>`: checks magic, format, zstd and every type name against the registry before touching the world; then despawns any SpawnId entities, clears `Messages<SimCommand>`/`Messages<SimEvent>`, resets `CommandScript` and `CommandLog`, spawns one entity per SpawnId, loads resources then components (`insert_batch`), checks every blob is read to the end, and runs the `on_load` hooks in registration order.
  - Unit tests: `a_saved_world_loads_identically`, `bad_saves_are_refused_before_anything_changes`, `readers_refuse_short_and_bad_data`.
- **Saved types:** every hashed type of every sim crate, registered next to its `register_hashed` in the same order (sim_military's in `crates/sim_military/src/save.rs`, `register(app)`).
  - Data resources as RON text: Calendar, WorldGenSettings (`from_ron`, validated), Economy, Villagers (`register_saved_resource_with`, against the loaded Calendar), Army (against Calendar + Economy), Progression, DirectorData.
  - `WorldMap` (`register_saved_resource_with(map::save_map, map::load_map)`): saves its digest plus terrain and revision of every chunk with `chunk_revision != 0`; on load it is **regenerated** from the loaded `SimSeed` and `WorldGenSettings` (`generate_world`), the overlay applied, and the digest compared (Mismatch otherwise). `WorldGenReport` gets the regeneration timings.
  - Also saved though exempt: `EventFeed`, `MilitaryLog` (UI cursors), `ArmyIndex` (armies without flow fields, signatures sorted by SpawnId; `index::save_index/load_index`), `FighterGrid` (`save_grid/load_grid`: used cells' entries, fighters, militia; masks and per-tile counts recomputed), `DecisionTrace` (presence only).
  - Load hooks: sim_world `after_load` (fresh `PathCache`, `FlowCache`, `SpatialGrid`, then `rebuild_grid` once); sim_military `after_load` (fresh `CommandInbox`, `Casualties`, `Deserters`).
- **sim_headless:** `SimPlugins.generate: bool` (true in `new`/`run`), `WorldPlugin.generate` (skips `finish` generation; AgentsPlugin already skips populate without a map). `load_app(bytes, threading) -> Result<App, SaveError>`, `save_app(&mut App) -> Vec<u8>`, `resume_commands(&mut App, &[LoggedCommand])` (CommandScript of the commands with `tick >= SimTick`), `continue_run(&mut App, until, commands, checkpoint_every, save_at) -> (RunResult, Option<Vec<u8>>)`, `run_saving(..)` (`run` wraps it), `verify_replay_saving(replay, threading, save_at)` (`verify_replay` wraps it).
  - CLI: `--save <file>` (plain run or `--replay`; at the end, or `--save-at <tick>`), `--load <file>` (`--ticks` more, or with `--replay` the rest of that replay with its later checkpoints checked: "replay ok from the save at tick N"; refuses world-building flags).
- **Tests:** `tests/tests/save_load.rs`: `save_load_restores_an_identical_hash` (~12 s) and `every_hashed_type_is_saved` (<1 s). `tests/tests/conquest.rs`: the prefix is played once (`OnceLock`) and saved; every branch loads it (the 2c open issue; file 45 s, was 51-79 s).
- **Docs:** ADR 0016; HOWTO "Add a component or resource" (implement `SaveState`, register hashed **and** saved, load hooks, the second coverage test); DETERMINISM (two rules, a Saves section with the CLI check, the save_load harness bullet); GLOSSARY (Save, Load hook); TESTING (rows, sim_core unit bullet, conquest row); ARCHITECTURE (sim_core, sim_military, sim_headless rows; a Saves bullet in "Where state lives").

### Step 9: what was built (UI-05, UI-09, ADR 0017)
- **sim_core `events.rs`:** `BattleReport {tick, started, faction, captain, foes: Vec<FactionId>, at, fought, losses, kills, turning_point: Option<u64>, promotions: Vec<Promotion>}`, `Promotion {soldier, rank, name: NameId}` (root re-exports). `Chronicle` gains private `reports` with `push_report`, `reports()`, `report_for(&ChronicleEntry)`; reports fold into the digest and the hash writes both lengths (O(1)); saved. `Cause` appends `Lapsed` (12), `Neglect` (13), `Fight { faction }` (14). Event docs updated: `OfferChosen.amount` = offer kind tag; `VeteranRank` grouped (`subject` captain, `other` count, `amount` rank); `ChampionFell.amount` = the champion's NameId; causes on MonumentLost, RequestFailed, RivalRaid, RaidersArrived, Battle.
- **sim_military:**
  - `Captain` gains `battle_foes: u32` (bit per faction), `battle_kills: u32`, `battle_rout: Option<u64>` (hash, save, `new`; `split_army` copies `battle_foes`). `resolve_strikes` ORs foes and counts kills per army (order-free); `build_index` (now `&mut Captain`) sets `battle_rout` the first tick a follower of an army in battle is seen routing.
  - `veterans::end_battles`: takes/resets the battle fields, emits `Battle` with `Cause::Fight{first foe}`, one `VeteranRank` per rank reached, and `Chronicle::push_report(BattleReport)` (promotions named via `veterans::person_name`, pub(crate)).
  - Causes: `RequestFailed` → `Lapsed`; drift `MonumentLost` → `Neglect`; `RivalRaid`/`RaidersArrived` → `Raid{captain}`; `ChampionFell.amount` = name; `OfferChosen.amount` = `o.kind.tag()`.
  - **`chronicle.rs` (new):** `ChronicleData {causes: CausePhrases}` (progression.ron `chronicle`, validated: non-empty, placeholders `{faction} {settlement} {captain} {boon} {foe}` only), `CausePhrases::phrase(Cause)`, `pub fn describe(world, &ChronicleEntry) -> String`, `pub fn describe_entries(world, &[ChronicleEntry]) -> Vec<String>` (one name lookup for the batch), `RunSummary {outcome, cause, days, settlements_taken/lost/held, people_held, monuments_taken/held, battles, kills, lost, veterans, elites, requests_done, boons}` with `RunSummary::of(chronicle, stats, progress, run_state, ticks_per_day, now, top_rank)` (pure) and `lines()`, `pub fn run_summary(world) -> RunSummary`. Root re-exports: `CausePhrases, ChronicleData, RunSummary, describe_entries, run_summary` (**not** `describe`: the root `describe` already words orders; use `sim_military::chronicle::describe`).
  - `MilitaryLog` gains `pub seq: u64` (entries ever pushed) and `since(cursor) -> (seq, tick, &str)`; the `entries` type is unchanged.
  - progression.ron `chronicle: (causes: (player, request, conquest, invitation, starvation, plunder, revolt, raid, hold, boon, death, lapsed, neglect, fight))`; `ProgressionData.chronicle`.
- **sim_headless:** `stats::run_summary(&App) -> Vec<String>` (empty outside a run), `stats::chronicle(&App) -> Vec<String>`; `bot::play_bot(config, max, every) -> (App, BotRun)` (`run_bot` wraps it); CLI `--chronicle` (plain run, `--bot`, `--load`: the chronicle in words and the summary); `--stats-every` prints the summary lines after "run ended".
- **Tests:** `tests/tests/chronicle.rs` `a_bot_run_is_chronicled_with_causes` (~16 s); sim_military unit tests `chronicle::tests::{entries_read_as_sentences_with_their_causes, the_summary_counts_the_run, the_log_numbers_its_lines}`; `rejects_bad_data` (chronicle phrases); updated: `champion.rs` `soldiers_who_fought_rank_up_with_the_configured_gains` (grouped VeteranRank + report promotions, foes, kills), `veterans::tests::a_battle_ends_after_a_quiet_hour_and_ranks_its_fighters` (grouped events + report), `run_start.rs` (`OfferChosen.amount` = kind tag).
- **Docs:** ADR 0017; DATA.md progression `chronicle` + 16 rows; GLOSSARY (Battle report, Run summary; Chronicle and Event feed updated); TESTING rows and unit bullets; ARCHITECTURE crate rows.

### Deviations from the guide (and why)
1. **The army index and fighter grid are saved, not rebuilt with `build_index` in a load hook.** The next tick's `raiders::think`, `rival::think` and the attack hold (`militia_standing`, `militia_or_foes_near`) read them *before* `build_index`, as built at the start of the previous tick; rebuilding from the end-of-tick state could change their decisions, and running `build_index` at a 50-tick boundary would renumber slots and change the hash at load. Saving them makes the resume exact.
2. **`Travel` saves the whole path** (not goal + `path[0]` + re-plan): exact, and cheap after zstd.
3. **The map is regenerated in its own resource loader** (`register_saved_resource_with`), which runs right after `WorldGenSettings` loads, rather than an `on_load` hook. Same effect, and the digest check happens before anything else loads.
4. **The payload lists SpawnIds first**, so resources holding entities (the army index) can map them.
5. **`SaveWriter::len` / `SaveReader::len` are named `count`** (clippy `len_without_is_empty`).
6. **CommandLog is not saved** (as the guide says); `load_world` resets it and `CommandScript`, and clears pending messages, so an in-process load starts clean. Saves must be taken between ticks with no live commands written yet.
7. **Step 9 event semantics changed** (append-only kinds untouched): `OfferChosen.amount` is now the offer kind's tag (the choice index was redundant with `other` = card, and a reader needs the deck); `VeteranRank` is grouped per army and rank; `ChampionFell.amount` carries the name. New causes `Lapsed`, `Neglect`, `Fight`. Only `RunStarted` and `RankUp` keep `Cause::None`.
8. **"Traits earned" in a report are the veteran ranks** (with names): soldiers' trait triggers (SIM-34) are still not implemented (step 6 deviation 9).
9. **The turning point is the army's own first rout** (`battle_rout`), per army; each side of a battle has its own report. A side wiped out to the last man still has no report (no captain to end it; step 6 open issue).
10. **The chronicle test has no Conquest** (the bot doesn't conquer before step 11) and abandons the run at day 3 to reach `RunEnded`; the task's list (battle report, monument taken, request outcome, RunEnded) is covered.
11. **`sim_military::chronicle::describe` is not re-exported at the root** (name clash with the order `describe` that game_ui uses).

### Names later steps must use (UI, step 10)
- **Save / load / resume:** `let bytes = sim_headless::save_app(&mut app);` (or `sim_core::save_world(world)`) between ticks; `sim_core::read_header(&bytes) -> Result<SaveHeader, SaveError>` for a resume menu (`seed`, `preset`, `run`, `tick`, `build`); `sim_headless::load_app(&bytes, Threading::Multi) -> Result<App, SaveError>` builds a fresh app (simplest for the game: build a new App the same way and swap it in), or `sim_core::load_world(app.world_mut(), &bytes)` into an existing app built by the same plugins (it despawns the old sim entities and clears pending commands; render-side caches must be reset by the UI, e.g. `WorldEpoch`). After a load in a recorded session: `sim_headless::resume_commands(&mut app, &log)`.
- **Feed:** `EventFeed::since(cursor)` and `MilitaryLog::since(cursor)` by `seq`; both survive a load.
- **Chronicle / end screen:** `sim_military::describe_entries(world, chronicle.entries())` (or `sim_military::chronicle::describe(world, &entry)` for one), `Chronicle::report_for(&entry)` / `reports()`, `sim_military::run_summary(world) -> RunSummary` (`lines()` for text), `sim_headless::stats::{run_summary, chronicle}`.

### Open issues
- `SAVE_FORMAT` 1 has no migration: a save loads only into the build that wrote it (same types and worldgen code; the map digest guards the latter).
- `describe` on a single entry scans every named entity; the UI should batch with `describe_entries` and cache lines by `seq`.
- Faction names come from army.ron as written ("Rival", "Beasts"), so sentences read "against Rival"; a `chronicle` name per faction could be added to the data later.
- CLAUDE.md's command list could gain `--save/--load/--chronicle` (step 12).
- The skeleton bot answers no requests in three days at seed 42 and fights no battle of its own; the chronicle test's battles are other factions' (rival vs beasts). Step 11's bot will make the player's story richer.

### Review (quick self-review of the diff) - fixed
1. `unhashed_state` flagged the new `SaveRegistry` resource as unhashed sim state (both coverage tests failed in the first full run): it is a registry like `HashRegistry`, so `unhashed_state` now skips it too (`hash.rs`).
2. Save bytes are deterministic: the only HashMap in saved state (`ArmyIndex.signatures`) is written sorted by SpawnId; the grid's per-tile counts are recomputed, not iterated (the save_load test re-saves the loaded game to identical bytes).
3. Checked, no change needed: `resolve_strikes`' foes/kills are bit-or and sums per army (order-free); `build_index` sets `battle_rout` only once per battle and only for armies in one; `end_battles` resets every battle field; `split_army` carries `battle_foes`; `succeed` moves the whole `Captain`; outside a run no `VeteranRank`/promotions happen; developed and skirmish stdout are byte-identical; dangling entity references save as 0 and load as `Entity::PLACEHOLDER` (lookups fail the same way); `load_world` validates everything before it changes the world and keeps the registry on error.

### Checks run (all pass, final tree)
- `cargo fmt --check`; `cargo clippy -j 2 --workspace --all-targets -- -D warnings`; `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps`.
- Full suite `CARGO_INCREMENTAL=0 cargo test -j 2 --workspace --no-fail-fast -- --test-threads=2` (log `scratchpad/89_full2.log`): exit 0, 235 tests passed, 1 ignored (`survey_twenty_seeds`). save_load 2 (13.1 s), chronicle 1 (15.4 s), conquest 4 (49.8 s, from one save), determinism 13 (155 s), champion 3, run_start 4, director 2, rival 2, dominion 2, monuments 2, invitation 2, wanderers 2, orders 7, docs 4, worldgen 7, army_march, posture, starving_army, famine, village_day; units sim_core 35, sim_military 55 (+1 ignored), sim_agents 19, sim_world 32, sim_economy 11, sim_headless 3, game_render 12, game_ui 6. (The first full run, `89_full.log`, stopped at the SaveRegistry coverage failure fixed above.)
- Release replay smoke: `--run --seed 42 --ticks 3000 --commands 50 --record SP/gate.ron` (51 commands), `--replay` and `--replay --single-threaded`: "replay ok", `0x004905ef0f268f72` (changed from step 7's `0x43d058361cb7a228` by step 9's new Captain fields, causes and progression.ron text; step 8 alone reproduced `0x43d058361cb7a228`).
- Save/resume smoke: `--replay SP/gate.ron --save SP/89_mid.sav --save-at 1500`, then `--load SP/89_mid.sav --replay SP/gate.ron` (multi and `--single-threaded`): "replay ok from the save at tick 1500", same final hash. A skirmish (developed, 2048) replay saved at tick 1234 resumes too (468 KB, loads in 1.4 s).
- Bot replay: `--run --bot --seed 42 --ticks 6000 --record` (12 commands, bot checkpoints matched), both replays `0x1bd36d3eeac37ae4`.
- Developed invariance: release `--seed 42 --ticks 4801 --stats-every 600` and `--skirmish --seed 42 --ticks 6000 --stats-every 2000` byte-identical to `5_dev.out` / `5_sk.out`.
- Disk: pruned twice (`prune_target.sh`, `target/debug/incremental`); 12 GB free at the end.
