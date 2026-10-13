# How to...

Recipes for common changes. Each one points at a real example to copy, and `tests/tests/docs.rs` checks that every file path on this page exists. First read [ARCHITECTURE.md](ARCHITECTURE.md) for the big picture and [DETERMINISM.md](DETERMINISM.md) for the rules every sim change must follow.

The definition of done for any change is in [CONTRIBUTING.md](../../CONTRIBUTING.md#definition-of-done).

## Add a component or resource

1. **Define it** next to related types, e.g. `Threat` in `crates/sim_agents/src/components.rs`. Derive `Component` (or `Resource`), `Debug` and `Clone`.
2. **Implement `StateHash`** by feeding every field to the hasher:
   - Hash `Entity` fields with `h.entity(e)`, which writes the target's SpawnId.
   - Hash an `Option` as a tag byte, then the value.
   - Iterate collections in a stable order: `Vec` in order, `BTreeMap` in key order.
3. **Implement `SaveState`** (GP-12, [ADR 0016](../adr/0016-save-format.md)), writing the same fields in a fixed order. For a plain struct use `sim_core::save_fields!(T { a, b, c })` where its fields are visible (see `crates/sim_military/src/save.rs`); `save_newtype!` for `struct T(U)`, `save_unit!` for a marker, `save_serde!` for a serde enum with no entities. `Entity` fields save as SpawnIds by themselves; an enum holding entities or vectors gets a hand-written `save`/`load` with a tag byte (see `TravelState` in `crates/sim_agents/src/components.rs`). Saved enums are append only.
4. **Register it hashed and saved** in the crate's plugin: `.register_hashed::<T>()` and `.register_saved::<T>()` (or the `_resource` forms), in the same order as the other types (see `crates/sim_agents/src/lib.rs`). A data resource is saved as its RON text with `register_saved_resource_with` (see `crates/sim_agents/src/data.rs`). If it is derived, a cache or diagnostics, use `.exempt_from_hash::<T>("why")` instead (see `crates/sim_military/src/lib.rs`), and if it must be rebuilt after a load, add an `.on_load(hook)` (see `crates/sim_world/src/lib.rs`). An exempt type the next tick reads before rebuilding it must be saved anyway (the army index in `crates/sim_military/src/index.rs`).
5. **Add it to every spawn site** that should have it: villagers in `crates/sim_agents/src/population.rs`, soldiers in `crates/sim_military/src/spawn.rs`.
6. **Run the coverage tests:** `cargo test -p integration_tests --test determinism state_hash_covers` and `cargo test -p integration_tests --test save_load` (`every_hashed_type_is_saved`, and a run saved, loaded and played on).

## Add a simulation system

1. **Write the function** in the module that owns the behaviour, e.g. `update_threats` in `crates/sim_military/src/threat.rs`. Open a span first: `let _span = tracing::info_span!("sim_military::update_threats").entered();`.
2. **Decide how it runs:**
   - **Per-agent work:** `par_iter_mut`, writing only the agent's own components.
   - **Shared changes:** post an order (as `Transfers` does) for a serial resolver, or use an exclusive `fn(&mut World)` that walks a list sorted by SpawnId (`crates/sim_military/src/combat.rs`).
   - **Randomness:** `seed.stream(rng_key("crate::system") ^ id.0, tick)`.
3. **Add it to the plugin** in a set:
   - Use the crate's set enum (`MilitarySet`, `AgentSet`, `EconomySet`) inside the right `SimSet`.
   - Order it against other crates with `.after(...)` / `.before(...)` on sets, not on functions.
   - If it is rare, gate it with `.run_if(...)` (see `crates/sim_military/src/lib.rs`).
4. **Check the cost** with `cargo run --release -p sim_headless -- --ticks 1500 --timings`, at 5,000 and at `--population 20000`.
5. **Update the schedule table** in [ARCHITECTURE.md](ARCHITECTURE.md#one-tick).

## Add a villager action

Example: `Hide` (SIM-13).
1. **Add the variant** to `ActionKind` in `crates/sim_agents/src/components.rs`, including `ALL` and `name()`.
2. **Score it** in `AiWorld::candidates` in `crates/sim_agents/src/ai.rs`:
   - Push a `Candidate` whose `ScoredOption` has a score and up to four named `factors` (the inspector shows them).
   - Set its target and the tile to walk to.
3. **Carry it out** in `perform` in `crates/sim_agents/src/act.rs`:
   - The `Stage::Perform` arm decides what happens on arrival and when the action is done (`Stage::Done`).
   - Goods change hands only through `transfers.post(...)`.
4. **Tune it in data:** add a struct in `crates/sim_agents/src/data.rs` and its entry in `assets/data/villagers.ron`, and document the fields in [DATA.md](DATA.md).
5. **Handle the new variant** wherever `ActionKind` is matched: `crates/sim_headless/src/stats.rs` and `crates/game_ui/src/panels.rs` (the compiler lists them all).
6. **Re-run** `village_day`, `famine` and `posture`, then look at the behaviour with `--stats-every 600` and in the inspector.

## Add a good, recipe or occupation

- **A good or recipe:** data only. Add it to `assets/data/economy.ron`; recipes refer to goods by name, and the loader validates them. Re-run `village_day` and `famine`.
- **An occupation:**
  1. Add it to `Occupation` in `crates/sim_economy/src/data.rs` (including `ALL`, `name()`, `workplace()`).
  2. Add its share in `assets/data/villagers.ron`; shares must sum to 1.
  3. Give it a colour in `occupation_color` in `crates/game_render/src/agents.rs`.
- **A workplace:**
  1. Add it to `WorkplaceKind` and its row in `economy.ron` (slots, radius, site rules).
  2. Give it a mesh in `crates/game_render/src/buildings.rs`.

## Add a captain order

1. **Add the variant** at the end of `OrderSpec` in `crates/sim_core/src/command.rs`. Append only: saves encode the variant's position. Name entities by SpawnId (`u64`), never `Entity`. Bump `REPLAY_FORMAT` in `crates/sim_core/src/replay.rs` if a command's meaning changes.
2. **Hash and describe it:** `hash_spec` (the next free tag byte) and `describe` in `crates/sim_military/src/components.rs`.
3. **Validate it** in `validate_spec`, which knows the issuer, and **execute it** in `step_order`, both in `crates/sim_military/src/orders.rs`. `Garrison` is a small example. It is refused unless the issuer owns the settlement (`conquest::owner_of`), and it keeps the army at the centre until the settlement is lost. For the execution:
   - Set the captain's goal.
   - Do the order's work when the army arrives.
   - Pop the order when it's done.
   - Anything that moves goods between stockpiles belongs here: it is the serial logistics step.
   - An order that stays put and works on a clock, like `Forage` (SIM-38, `crates/sim_military/src/wild.rs`), sets the goal to `None`, records its start in `order.since` at step 0, and acts every N ticks from it.
4. **Make it reachable from the UI** in `crates/game_ui/src/army.rs` (menu entry or hotkey). The UI only writes `SimCommand`s.
5. **Add it to the random orders** in `sample_orders` in `crates/sim_headless/src/lib.rs`, so the determinism test exercises it. Add a scenario test in `tests/tests/orders.rs`, or in a file of its own for an order that plays out over days (Garrison's is `tests/tests/conquest.rs`).

## Add a way to take a town

M4a (GP-32, [ADR 0020](../adr/0020-lordship.md)) has a town change hands in exactly one place, `conquest::transfer_seeded` in `crates/sim_military/src/conquest.rs`, and every way of taking it leaves its own `Tenure` (`crates/sim_military/src/tenure.rs`). Example: the Demand (step 5).
1. **Name it:** append a `Cause` (in `crates/sim_core/src/events.rs`, with its tag, `name` and a phrase in `assets/data/progression.ron` `chronicle.causes`), a `TakenBy` if the tenure is new (`TakenBy::of` maps the cause), and, if it is reported as its own kind, a `SimEventKind` with its words in `sim_military::chronicle::describe`. Map it in `conquest::taking_kind` and decide its counters there (`conquests`, champion credit, the run summary in `chronicle.rs`); `tests/tests/tenure.rs` `every_cause_maps_to_its_kind_and_counters` pins the table.
2. **The verb:** an order (see "Add a captain order": `validate_spec` and `step_order` in `crates/sim_military/src/orders.rs`; `demand` is a two-step example: march, then a clocked parley) or a progress step. Show its preconditions with a pure function the UI can call with `&World` (`tenure::parley`), so the menu says what the sim will weigh.
3. **Hand it over** with `transfer_seeded(world, s, to, cause, seed)` from a serial step; never `set_owner` directly. Its scar comes from `TenureData::scar`; what the town remembers lives in `Tenure` (amend the type, its hash, `save_fields!` and `Tenure::new`/`retake` together).
4. **What it leaves:** a loyalty term (see "Add a loyalty term": Awe and Bond are written in `tenure::rebuild_terms`) and any hourly rule in `tenure::step` (progress slot 14: the submitted town's watch, defiance and settling). A rule that touches another lane's state writes it through that lane's type (the tribute holds `sim_agents::Edicts`' Dues dial) and does not reimplement its effects.
5. **Run only:** gate it on a `Tenure` or `RunState::in_run`, and check the three guarded stdout outputs stay byte-identical.
6. **Data and docs:** its block in progression.ron with [DATA.md](DATA.md) rows; the glossary term; the slot or system in [ARCHITECTURE.md](ARCHITECTURE.md).
7. **Test it** in `tests/tests/take_modes.rs`: the take and its tenure, its refusal or failure, and what it leaves over the following days; teach the bot (`crates/sim_headless/src/bot.rs`) and re-run `bot_run`.

## Make a deed move goodwill

Goodwill (GP-22) changes only through the ledger in `crates/sim_agents/src/standing.rs`, so the spread to nearby settlements and the order of application stay deterministic.
1. **Name the deed:** append a variant to `Deed` (append only: the tag is hashed and saved).
2. **Give it a delta** in `assets/data/progression.ron` `goodwill_deeds`, read through `GoodwillDeeds::delta` in `crates/sim_military/src/progression/mod.rs`, and document it in [DATA.md](DATA.md).
3. **Post it from a serial step** with `orders::goodwill_deed(world, settlement, faction, deed)` (in `crates/sim_military/src/orders.rs`), once per deed, not once per tick of an ongoing one (see `provoke`, which posts only a fresh provocation). Never write `Goodwill` directly.
4. **Test it** like `deeds_move_goodwill_and_word_spreads`: check the ledger right after the deed, then step a tick and check the settlement and a neighbour within `goodwill.spread_radius`.

## Add a loyalty term

A loyalty term (SIM-60, M4a) is one named, signed share of a settlement's loyalty target, kept apart so the panel can say why. `LoyaltyTerms` holds one value per `LoyaltyTerm`; the sum (the `bias`) is added to the target in `loyalty_target` before it is clamped to 0..1, and a bias of exactly 0.0 changes nothing. Each term is owned by the module of its rule: Resentment and Stirred by `crates/sim_military/src/tenure.rs`, Contest by `crates/sim_military/src/influence.rs` (`contest_term`), Dues, Table and Steward by `crates/sim_military/src/govern.rs`. Example: Contest.
1. **Name the term:** append a variant to `LoyaltyTerm` in `crates/sim_agents/src/loyalty.rs` (append only: the position is hashed and saved), with `COUNT`, `ALL` and its `name()`, the words the panel prints. The saved array grows with `COUNT`: that is a saved-layout change, so the merge lead decides about `SAVE_FORMAT` (inside M4a the type is amended in place).
2. **Write the term as a pure read in the module that owns the rule:** `pub fn my_term(world: &World, settlement: Entity) -> f32`, like `influence::contest_term`. It is exactly `0.0` (never `-0.0`: write `0.0 - x`) outside a run, where it doesn't apply, and when its data block's `enabled` is false, so scenario tests can opt out. It reads state only and is called from serial steps.
3. **Sum it in `tenure::rebuild_terms`:** one `next.set(LoyaltyTerm::X, module::my_term(world, s))`. That function is the only writer of `LoyaltyTerms`. `tenure::step` (progress slot 14) calls it every hour for every town and a taking (`tenure::retake`) calls it at once, so a term that reads another slot's work needs that slot to run first (Contest reads `Influence`, rebuilt in slot 13); if a taking changes what it reads, refresh that in `retake` first (see `influence::refresh_field`).
4. **Name it in the panel:** `Why::term_name` in `crates/game_ui/src/panels/why.rs` words a term from the owner's side and by its sign (Contest reads "heartland" when positive and "your pull" at the rival's towns); `LoyaltyTerm::name` is the default. The risk line and "Pulled down by" pick a term up by themselves (`deciding_term` and `loyalty_pulls` in `crates/sim_agents/src/loyalty.rs`).
5. **Data:** its knobs go in `assets/data/progression.ron` with a switch, documented in [DATA.md](DATA.md).
6. **Test it:** unit tests of the term's function; a scenario that reads `LoyaltyTerms` after a tick and the drift it causes, with a twin run that opts out (`tests/tests/influence.rs`, `the_rival_contests_the_border`; `tests/tests/tenure.rs`, `terms_sum_into_the_target`); and a case in the panel's tests.

## Add a data field

1. **Add the field** to the data struct (e.g. `ArmyData` in `crates/sim_military/src/data.rs`) and to the RON file, with a comment.
2. **Convert units** once in the resource if needed. Per-day rates become per-tick with `Army::per_tick`.
3. **Document it** in [DATA.md](DATA.md). `tests/tests/docs.rs` fails until you do.

## Add a UI panel or button

UI code reads sim state and writes `SimCommand`s only. Copy the captain panel in `crates/game_ui/src/army.rs`:
1. **Spawn the panel** once in `Startup`, using the `Node` and `BackgroundColor` styles from `crates/game_ui/src/style.rs`.
2. **Update its text and visibility** every frame from queries.
3. **Handle buttons** with `Query<(&Interaction, &YourButton), Changed<Interaction>>`.
4. **Give clickable panels `Interaction::None`,** so clicks on them don't also select things in the world (see `crates/game_ui/src/selection.rs`).
5. **Check it in the real game** under Xvfb ([GETTING_STARTED.md](GETTING_STARTED.md#6-screenshots-without-a-display)).

## Add a scenario test

Copy `tests/tests/posture.rs`. Shared helpers (commands, positions, village lists) are in `tests/tests/common/mod.rs`.
1. **Build a world:** `headless_app(HeadlessConfig::new(SEED))`, or `Scenario::Skirmish` for armies, or `common::run_app(SEED)` for a run on the run island (`HeadlessConfig::run(SEED, RunConfig::default())`; its start boon waits for a pick: `common::choose(&mut app, offer, 0)` with `offer` from `common::start_offer(&app)`). Set up extra state before the first tick (`spawn_army`, `cut_off_farmers`, spoiling a village's stores so it asks for food, setting a settlement's `Goodwill`). `tests/tests/run_start.rs` is a run example. For a run scenario that needs a particular band or start, pick a seed whose draws fit (`cargo run --release -p sim_headless -- --run --seed N --ticks 1` prints the band, the start and the boons on offer) and search its world for the settlement you need; `tests/tests/wanderers.rs` plays a band through commands only, as a player would. In a run the pacing director brings raider bands and the rival levies and raids on its clock: a test that needs quiet sets the director's limits beyond its length (edit `DirectorData` before the first tick), and a test of the rival's later clock shortens its `progression.ron` `rival` row before the first tick together with `RivalState`'s next levy and raid (`tests/tests/rival.rs` does both). `sim_headless::step_until_event` fast-forwards to the next notable event (`tests/tests/director.rs`), and `sim_headless::bot::run_bot` plays a whole run with player commands.
2. **Send input** as commands: `common::order(...)`, never by editing components mid-run. Another faction's band takes orders the same way, with its faction as the issuer (`common::order(&mut app, 2, raiders, ...)`). A scenario that branches from a shared prefix, as the conquest scenario (`tests/tests/conquest.rs`) does until saves arrive, may make each branch's setup edits (spawning a band, spoiling stores) right after the prefix, between ticks, as it would after loading a save.
3. **Step** with `app.step(n)`, and read results through queries or `sim_headless::stats`.
4. **Assert** the requirement, and print the numbers so a failure explains itself.
5. **Note what the test guards** in [TESTING.md](TESTING.md).

## Add a run scenario test

A run scenario plays an M4 rule on the run island with a real band, from one seed, through the same commands a player sends. Copy `tests/tests/wanderers.rs` (a band, commands only), `tests/tests/champion.rs` (state set up before the first tick) or `tests/tests/conquest.rs` (branches from one saved prefix). It is a scenario test (see "Add a scenario test") with these run-specific steps:
1. **Build it:** `common::run_app(SEED)`, then `common::start_offer(&app)` and `common::choose(&mut app, offer, 0)` to pick the start boon (the cards are generic over which boon the seed offers, so assert on effects, not names). Pick a seed whose draws fit (`--run --seed N --ticks 1` prints the band and the boons on offer) and search its world for the settlements or monuments you need; don't hard-code coordinates.
2. **Quiet what you aren't testing:** `common::quiet_director(&mut app)` before the first tick stops director bands, and the rival's clock can be shortened in `Progression` data (`tests/tests/rival.rs`). Hourly steps run at ticks 0, 100, 200, ...; modifiers apply at the next hourly rebuild, not at once.
3. **Set up before the first tick** by editing state directly (spawn a band with `sim_military::spawn_army`, set a `Goodwill`, spoil a store; queue an offer with `Offers::post`, never `post_offer`, because the run's first tick reports every pending offer and `post_offer`'s own report would tell it twice), then **act only with commands** (`common::order`, `common::command`), as the player would.
4. **Read the result** from the chronicle (`common::chronicle`), `RunState`, `RunProgress` and the components, not from `SimEvent` messages (they live two ticks). A run that ends freezes `SimTick`, so don't assert that every scripted command ran.
5. **Keep it short and cheap:** under about a minute at the test profile. Tests in one file run in parallel threads with a full world each, so loop seeds inside one `#[test]`, and play a shared prefix once (a `OnceLock` plus `sim_headless::save_app` and `load_app`, as `conquest.rs` does) when several branches need it.
6. **Cover the state it adds:** extend `run_state_hash_covers_every_sim_component` in `tests/tests/determinism.rs`, add the test's row to [TESTING.md](TESTING.md), and say which requirement ID it guards in its doc comment.

## Add an edict

An edict (GP-34, M4a step 4) is one setting of a town's Work, Dues or Table dial, set with `CommandKind::Govern` and applied by the next hourly `govern::step`. Run only, and an exact no-op at the default. Example: the Ration setting of Table.
1. **Name the setting:** append a variant to `WorkFocus`, `Dues` or `Table` in `crates/sim_core/src/command.rs` with its `tag` and `name` (append only: tags are hashed, saved and recorded in replays). Add it to the `GovernAct` handling in `refusal` and `command` if it needs more than the dial's value.
2. **State, if it needs any:** a field of `Edicts` or a component in `crates/sim_agents/src/edicts.rs` (as `Rationed` is), added to its `hash_state` and its `save_fields!`, and registered with `register_hashed` and `register_saved` if it is a new type. Extend the `save_load` round trip in `tests/tests/save_load.rs` with a town in that setting, pending and locked.
3. **Its effect, at the point of use** (never in `FactionModifiers`, ADR 0015): read the setting where the rule runs (`update_needs`, `perform`, `ai::decide`, `resolve_strikes`, `update_threats`, `orders::lands`), run only, and exactly the old behaviour at the default (`x * 1.0`, `+ 0.0`). A rule in sim_agents cannot read sim_military's data, so copy the numbers into the component when the setting starts, as `Rationed` does.
4. **Apply it in `crates/sim_military/src/govern.rs`:** `apply_pending` turns the dial and starts its lock; a Work setting's wave goes in `work_wave` (its donors and targets in `plan`, moves through `sim_agents::jobs::reassign`); a Table setting's start and end go in `set_table` (and `end_feast`/`lift_ration` for what must be undone); a Dues setting's dawn work goes in `step`'s dawn pass and, for goods that move to a captain, `orders::lands`. `reset` must undo it when the town is lost, and `sweep_soldiers` must clear any per-villager mark from the enlisted.
5. **Its loyalty term** in `dues_term` or `table_term` (summed by `tenure::step`; see "Add a loyalty term"), 0.0 outside a run.
6. **Its forecast** in `work_forecast`, `dues_forecast` or `table_forecast` (read-only; the panel prints it before the player commits) and its words in `crates/game_ui/src/panels/govern.rs`.
7. **Its validation** in `govern::refusal` (a reason string for the panel, e.g. "locked" or "already set"); `refusal` is the one place a command is refused and the panel greyed.
8. **Data:** its knobs go in the `edicts` block of `assets/data/progression.ron` (`EdictsData` in `crates/sim_military/src/govern.rs`), documented row by row in [DATA.md](DATA.md).
9. **Test it** in `tests/tests/edicts.rs`: the setting does what it says, the lock holds, a lost town resets, and a run with no dial set is byte-identical (`defaults_are_no_ops`). Re-run `village_day` and `famine`, which must not change.

## Add a run-only rule

Runs (GP-21, M4) add rules that the developed world, the benchmarks and the M2/M3 tests must never feel ([ADR 0013](../adr/0013-runs-presets-and-run-config.md)). Example: the start boon, applied in `crates/sim_military/src/progression/offers.rs`.
1. **Put the data** in `assets/data/progression.ron` and document every field in [DATA.md](DATA.md). Cards are append only: a card's `id` is its index, and offers and saves store it.
2. **Gate it:** add the system with `.run_if(in_run)`, or check `RunState::in_run()` in a serial step, so nothing runs or draws outside a run. Hourly or daily steps go inside the progress driver (`progression::progress`) in its documented order, like `wild::regrow` or `rival::clock`, never as systems of their own. A scripted faction's decisions (like `rival::think`) go in `SimInputSet::Ai`, chained after the others, and send `SimCommand::ai` orders. A rule that changes goodwill posts to the `GoodwillLedger` (see "Make a deed move goodwill").
3. **Draw from a new stream key** (`rng_key("sim_military::your_step")`), never an existing one, and spawn only in a serial step with `NextSpawnId::allocate`.
4. **Keep multipliers exact no-ops:** read `FactionModifiers::mult`, which is exactly 1.0 when nothing applies (see "Add a modifier").
5. **Player choices arrive as commands.** Inside a tick, post an offer with `sim_core::post_offer` (it reports `OfferPosted`); run setup posts with `Offers::post` (never `post_offer`, or the offer is told twice) and writes no `SimEvent`s, and the run's first tick tells the start and every pending offer from state (`report_start` in `crates/sim_military/src/progression/mod.rs`), so a save before the first tick keeps them. Apply the pick in `progression::choose`; a run ends only through `RunState::finish(outcome, cause)` from a serial step (`settle_outcome` records `RunEnded`).
6. **Test it on a run** with `common::run_app` and read what happened from `common::chronicle`, never from `SimEvent` messages. Extend `run_state_hash_covers_every_sim_component` in `tests/tests/determinism.rs` so the new state is covered.
7. **Show the developed world didn't change:** re-run `village_day`, `famine`, `army_march`, `posture`, `starving_army` and `orders`, and compare `cargo run --release -p sim_headless -- --seed 42 --ticks 1200 --stats-every 600` with its output before the change (only the state hash may differ).

## Add a monument type

Monuments (GP-23, [ADR 0015](../adr/0015-monuments-and-faction-modifiers.md)) are placed by worldgen and held, guarded and turned into bonuses by `sim_military`. Example: the watchtower, which widens its holder's threat radius.
1. **Name the kind:** append a variant to `MonumentKind` in `crates/sim_world/src/monuments.rs` (append only: the index is hashed and saved), with its `name()` and `title()`, and bump `COUNT` and `ALL`.
2. **Place it:** add it to `monuments.kinds` in the run preset of `assets/data/worldgen.ron` with a weight and a `SiteRule` (`crates/sim_world/src/settings.rs`; a new kind of site needs a rule in `crates/sim_world/src/worldgen/monuments.rs`). Re-run `--test worldgen` and `--test run_start`: the start still needs 2-3 monuments nearby.
3. **Give it a bonus:** add a `monuments.kinds` entry in `assets/data/progression.ron` (`stat`, `bonus`, `cap`, `set_bonus`). If its stat has no consumer yet, add one (see "Add a modifier"). `shipped_monument_data_fits_the_island` in `crates/sim_military/src/progression/mod.rs` checks every kind has an entry.
4. **Document it** in [DATA.md](DATA.md) and the glossary's Monument row, and test the bonus with `monument_modifiers` like `caps_and_set_bonus`.

## Add a trait or boon

Traits (SIM-34) and boons (GP-24, GP-27) are cards in `assets/data/progression.ron`, read by `crates/sim_military/src/progression/mod.rs`. Example: the "Quartermaster" trait, whose `(Desertion, -0.1)` makes the champion's soldiers 10% less likely to desert.
1. **Append the card:** a `traits` entry (`id`, `name`, `phrase`, `effects`) or a `boons` entry (`id`, `name`, `text`, `when: Start` or `Dominion`, `weight`, `effects`, and for a Dominion boon optionally `favour`). The `id` is the card's index: append only, never reorder (offers, `RunProgress` and saves store it).
2. **Use effects something reads.** A `Stat(stat, value)` effect is folded into `FactionModifiers` at the next hourly rebuild (`rebuild_modifiers`): the trait's for its champion's faction, the boon's for the player. Pick a stat with a consumer (DATA.md's `traits[].effects` row lists them); a new kind of effect needs a consumer first (see "Add a modifier"). Boons also have one-off effects applied at the pick in `crates/sim_military/src/progression/offers.rs` (`FoodDays`, `ArmBand`, `Goodwill`, `CaptainSlots`); a new one is an appended `Effect` variant handled there.
3. **Let the champion be offered a trait** by adding its id to `champion_traits`: its starting trait and each rank's offer of three come from that pool (`crates/sim_military/src/progression/champion.rs`). Leave negative traits (Shaken) out of it.
4. **Check it:** `shipped_data_is_valid` and `rejects_bad_data` in `crates/sim_military/src/progression/mod.rs` must pass; re-run `--test champion` (traits) or `--test run_start` and `--test dominion` (boons), whose draws change with a new card. Document the card's effect in [DATA.md](DATA.md) if it needs more than its `text`.

## Add a run modifier

Run modifiers (GP-27) are the one or two cards drawn from the seed at the start of every run and shown on the new-game screen. They live in `assets/data/progression.ron` (`modifiers`, `modifiers_per_run`), are drawn in `crates/sim_military/src/progression/start.rs` and applied in `crates/sim_military/src/progression/mod.rs`. Example: "Lean year", whose `Stat(FoodYield, -0.15)` cuts every crop by 15%.
1. **Append the card:** `id` (its index, append only: `RunProgress` and saves store it), `name`, `text`, `group` (cards in one group, like "harvest" or "rival", never come together), `weight` and `effects`.
2. **Pick an effect something reads.** `Stat(stat, v)` goes into the world row of `FactionModifiers` at the hourly rebuild and applies to every faction, so the stat needs a consumer (see "Add a modifier"). `Stores(f)` acts once at setup, `RaiderPace(x)` is read by the director (`crates/sim_military/src/director.rs`) and `RivalStrength(x)` by the rival (`crates/sim_military/src/rival.rs`). `Sellswords(n)` spawns a band for hire at the end of `setup_run` and queues a `Hire` offer, reported on tick 0 (`crates/sim_military/src/sellswords.rs`, tuned by `hire`). A new kind of effect is an appended `Effect` variant with its reader. A modifier that spawns something does it at the end of setup, after every other spawn, from its own stream, so no earlier SpawnId or draw changes (and runs without the card stay byte-identical); if it posts an offer that expires, clean up after the lapse in `invitations::step`'s expiry pass (`crates/sim_military/src/invitations.rs`), the one place offers lapse.
3. **Check it:** `shipped_data_is_valid` and `rejects_bad_data` in `crates/sim_military/src/progression/mod.rs`, then `--test run_start` (the twenty seeds' draws change with a new card) and `--test rival`, `--test director` or `--test sellswords` for a card they read. Document the card's effect in [DATA.md](DATA.md) (`modifiers[].effects`).

## Add a modifier

A modifier is a `Stat` in `crates/sim_core/src/modifiers.rs` that monuments, boons, traits and run modifiers can raise, summed per faction into `FactionModifiers` by `rebuild_modifiers` (`crates/sim_military/src/progression/mod.rs`). Example: `MarchSpeed`, read by both `move_captains` and `move_soldiers` in `crates/sim_military/src/soldiers.rs`.
1. **Append the stat** to `Stat` (append only: data files and saves name it), with `COUNT`, `ALL` and `name()`. Fractions are added to 1.0 (`mult`); flat amounts are read with `value`.
2. **Read it where the stat is used,** at the point of use: `mods.mult(faction, Stat::X)` with `Res<FactionModifiers>` in a system, or `world.resource::<FactionModifiers>()` in a serial step. Never fold a modifier into `Economy`, `Army` or other data at runtime: their hash covers only the data text (guide G9).
3. **Keep it an exact no-op at 1.0:** multiply (`x * mult`) or divide by it, never add a rounded difference; integer results round only when there is a fraction, and stochastic rounding draws (from a new stream key) only then (see `yield_units` in `crates/sim_agents/src/act.rs`). The developed world must print the same `sim_headless --seed 42 --ticks 4801 --stats-every 600` before and after.
4. **Give something the stat:** a `monuments.kinds` entry, or a `Stat(stat, value)` effect of a boon, trait or run modifier in `assets/data/progression.ron`; nothing else writes `FactionModifiers`. Document the stat's meaning in [DATA.md](DATA.md) (`monuments.kinds[].stat`) and test the consumer: run its system once with a modifier set by hand (as `the_loyalty_drift_modifier_speeds_drift` in `crates/sim_agents/src/loyalty.rs` does; in a full app the hourly rebuild would replace it), or give it through a boon or a held monument in a run.

## Tune behaviour

- **Population summaries:**

  ```sh
  cargo run --release -p sim_headless -- --ticks 7200 --stats-every 600
  ```

  `STATS_DETAIL=1` adds who isn't working, restless sleepers and the hungriest villager. `--skirmish` or `--battle 1000` add a line per army (men, fighting, routed, spread, morale, food) and per-faction totals.
- **Why someone did something:** check the inspector in game, or `DecisionTrace` in code, before changing weights.
- **After tuning,** re-run the scenario tests that guard the file ([DATA.md](DATA.md) lists them).

## Tune run balance

A run's balance (the rival's clock, the director's bands, boons, requests, forage) lives in `assets/data/progression.ron` and `assets/data/director.ron`. The scripted player in `crates/sim_headless/src/bot.rs` is the yardstick: it must win 3 of 5 fixed seeds within 40 days.

1. **Watch one run:** `cargo run --release -p sim_headless -- --run --bot --seed 1 --ticks 96000` prints the outcome; stderr has the bot's notes (a status line every game day: army, food, captains, land held, the seat's estimated defence) and the chronicle by kind. Add `--difficulty harsh` for another row, `--chronicle` for the story.
2. **Change data only** when the bot loses because the game is unbalanced (not because the bot plays badly: fix `bot.rs` then). Keep the developed and skirmish presets as they are.
3. **Re-run** `cargo test --release -p integration_tests --test bot_run -- --ignored --nocapture` (`tests/tests/bot_run.rs`; about 5 minutes; per-seed outcome, day and numbers) and the tests CLAUDE.md names for the files you touched.
4. **If the 3-of-5 bar turns flaky,** loosen the bar (`MIN_WINS` in `tests/tests/bot_run.rs`) rather than fudge the data, and note it with before/after numbers in a tuning log (step 11's is `docs/m4/PROGRESS_BOT.md`).

## Profile

- **Per-stage tick cost:** `cargo run --release -p sim_headless -- --ticks 1500 --timings`. Add `--population 20000` or `--battle 7500` for heavy loads, and `--stats-every 250` to see the mean and worst tick and the costliest stages for each window of 250 ticks (spikes hide in a whole-run average).
- **Per-system detail:** Tracy. `cargo run -p game_app --release --features tracy`, then connect the Tracy profiler; every system's span shows up.
- **Benchmarks:** `cargo bench -p sim_headless` (`tick/agents/*`, `tick/army/2000`, `tick/run_island` (a run at day 10, the bot's commands replayed), `state_hash/*`). A nightly CI job fails on a slowdown over 10% (`scripts/check_bench_regression.py`).
