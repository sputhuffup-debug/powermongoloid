# How to...

Recipes for common changes. Each one points at a real example to copy, and `tests/tests/docs.rs` checks that every file path on this page exists. First read [ARCHITECTURE.md](ARCHITECTURE.md) for the big picture and [DETERMINISM.md](DETERMINISM.md) for the rules every sim change must follow.

The definition of done for any change is in [CONTRIBUTING.md](../../CONTRIBUTING.md#definition-of-done).

## Add a component or resource

1. **Define it** next to related types, e.g. `Threat` in `crates/sim_agents/src/components.rs`. Derive `Component` (or `Resource`), `Debug` and `Clone`.
2. **Implement `StateHash`** by feeding every field to the hasher:
   - Hash `Entity` fields with `h.entity(e)`, which writes the target's SpawnId.
   - Hash an `Option` as a tag byte, then the value.
   - Iterate collections in a stable order: `Vec` in order, `BTreeMap` in key order.
3. **Register it** in the crate's plugin with `.register_hashed::<T>()` or `.register_hashed_resource::<T>()` (see `crates/sim_agents/src/lib.rs`). If it is derived, a cache or diagnostics, use `.exempt_from_hash::<T>("why")` instead (see `crates/sim_military/src/lib.rs`).
4. **Add it to every spawn site** that should have it: villagers in `crates/sim_agents/src/population.rs`, soldiers in `crates/sim_military/src/spawn.rs`.
5. **Run the coverage test:** `cargo test -p integration_tests --test determinism state_hash_covers`.

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
4. **Make it reachable from the UI** in `crates/game_ui/src/army.rs` (menu entry or hotkey). The UI only writes `SimCommand`s.
5. **Add it to the random orders** in `sample_orders` in `crates/sim_headless/src/lib.rs`, so the determinism test exercises it. Add a scenario test in `tests/tests/orders.rs`.

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
1. **Build a world:** `headless_app(HeadlessConfig::new(SEED))`, or `Scenario::Skirmish` for armies, or `common::run_app(SEED)` for a run on the run island (`HeadlessConfig::run(SEED, RunConfig::default())`; its start boon waits for a pick: `common::choose(&mut app, offer, 0)` with `offer` from `common::start_offer(&app)`). Set up extra state before the first tick (`spawn_army`, `cut_off_farmers`). `tests/tests/run_start.rs` is a run example.
2. **Send input** as commands: `common::order(...)`, never by editing components mid-run.
3. **Step** with `app.step(n)`, and read results through queries or `sim_headless::stats`.
4. **Assert** the requirement, and print the numbers so a failure explains itself.
5. **Note what the test guards** in [TESTING.md](TESTING.md).

## Add a run-only rule

Runs (GP-21, M4) add rules that the developed world, the benchmarks and the M2/M3 tests must never feel ([ADR 0013](../adr/0013-runs-presets-and-run-config.md)). Example: the start boon, applied in `crates/sim_military/src/progression/offers.rs`.
1. **Put the data** in `assets/data/progression.ron` and document every field in [DATA.md](DATA.md). Cards are append only: a card's `id` is its index, and offers and saves store it.
2. **Gate it:** add the system with `.run_if(in_run)`, or check `RunState::in_run()` in a serial step, so nothing runs or draws outside a run.
3. **Draw from a new stream key** (`rng_key("sim_military::your_step")`), never an existing one, and spawn only in a serial step with `NextSpawnId::allocate`.
4. **Keep multipliers exact no-ops:** read `FactionModifiers::mult`, which is exactly 1.0 when nothing applies.
5. **Player choices arrive as commands.** Post an offer with `sim_core::post_offer` and apply the pick in `progression::choose`; a run ends only through `RunState::finish(outcome, cause)` from a serial step (`settle_outcome` records `RunEnded`).
6. **Test it on a run** with `common::run_app` and read what happened from `common::chronicle`, never from `SimEvent` messages. Extend `run_state_hash_covers_every_sim_component` in `tests/tests/determinism.rs` so the new state is covered.
7. **Show the developed world didn't change:** re-run `village_day`, `famine`, `army_march`, `posture`, `starving_army` and `orders`, and compare `cargo run --release -p sim_headless -- --seed 42 --ticks 1200 --stats-every 600` with its output before the change (only the state hash may differ).

## Tune behaviour

- **Population summaries:**

  ```sh
  cargo run --release -p sim_headless -- --ticks 7200 --stats-every 600
  ```

  `STATS_DETAIL=1` adds who isn't working, restless sleepers and the hungriest villager. `--skirmish` or `--battle 1000` add a line per army (men, fighting, routed, spread, morale, food) and per-faction totals.
- **Why someone did something:** check the inspector in game, or `DecisionTrace` in code, before changing weights.
- **After tuning,** re-run the scenario tests that guard the file ([DATA.md](DATA.md) lists them).

## Profile

- **Per-stage tick cost:** `cargo run --release -p sim_headless -- --ticks 1500 --timings`. Add `--population 20000` or `--battle 7500` for heavy loads, and `--stats-every 250` to see the mean and worst tick and the costliest stages for each window of 250 ticks (spikes hide in a whole-run average).
- **Per-system detail:** Tracy. `cargo run -p game_app --release --features tracy`, then connect the Tracy profiler; every system's span shows up.
- **Benchmarks:** `cargo bench -p sim_headless` (`tick/agents/*`, `tick/army/2000`, `state_hash/*`). A nightly CI job fails on a slowdown over 10% (`scripts/check_bench_regression.py`).
