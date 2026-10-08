# Tests

```sh
cargo test --workspace                                  # everything, ~4 min on 4 cores
cargo test-fast                                         # unit tests only, seconds (alias)
cargo test -p integration_tests --test posture          # one integration test file
cargo test -p integration_tests --test orders attacking # tests whose names match
cargo test ... -- --nocapture                           # see what tests print
```

The sim crates build at `opt-level = 3` even in dev, so the scenario tests run at full speed without `--release`.

## What CI runs

On every push and PR (`.github/workflows/ci.yml`):
1. `cargo fmt --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace`
4. `cargo doc --workspace --no-deps` with warnings as errors
5. a replay smoke run: record 10,000 ticks with 200 commands, then verify the replay in parallel and serially

Nightly (`.github/workflows/bench.yml`): `cargo bench -p sim_headless`, compared with the previous night's results. It fails on a slowdown over 10%.

## Integration tests (`tests/tests/`)

Times are on a 4-core machine.

| File | Test | Guards | Time |
| --- | --- | --- | --- |
| `determinism.rs` | `ten_thousand_ticks_with_command_log_are_identical` | NFR-08: same seed and commands give the same hashes | ~30 s |
| | `parallel_and_serial_execution_agree` | Results don't depend on threading | ~30 s |
| | `live_commands_record_and_replay_identically` | UI-style commands record and replay; replays round-trip through RON | ~10 s |
| | `a_dropped_command_is_detected` | Replay verification catches a missing command | ~5 s |
| | `different_seeds_give_different_hashes`, `hash_changes_as_the_sim_advances`, `hashing_does_not_change_state` | The hash actually measures state | ~5 s |
| | `state_hash_covers_every_sim_component` | Every sim component and resource is hashed or exempt (armies included) | ~10 s |
| | `a_battle_replays_without_logging_setup_orders` | NFR-08, guide G1: a `--battle` setup's opening orders are AI commands, so its replay logs no commands, and verifying it (which re-runs the setup) reproduces in parallel and serially | ~5 s |
| | `armies_with_orders_are_deterministic_and_replay` | Armies plus 200 random orders: parallel equals serial, and the replay reproduces | ~60 s |
| | `run_state_hash_covers_every_sim_component` | M4 (GP-21, GP-27): the same coverage on a run, with the start boon picked at tick 0 and random orders for 1,500 ticks. Each M4 step extends this run with its own state. Step 2 (GP-05): a 30-man warband spawned beside the hamlet nearest the start takes it with an Attack (seed 42: at tick 510) and then garrisons it, so owned land and a garrison exist. The run ends before a day tick reaches the conquest; `conquest.rs`'s `a_conquest_and_its_revolt_are_thread_independent` compares serial and parallel runs with an owned settlement at day ticks | ~3 s |
| | `a_run_replays` | NFR-08 for runs: the replay header carries the run's `RunConfig` (`Replay.run`), and a run with its start boon pick and 60 random orders (some refused) reproduces in parallel and serially | ~11 s |
| `worldgen.rs` | `shipped_world_is_2048_square_in_64_tile_chunks`, `same_seed_same_island_regardless_of_threads`, `different_seeds_give_different_islands`, `every_seed_meets_the_m1_criteria`, `the_sim_spawns_settlements_and_roads` | SIM-01 and SIM-02: world size, determinism, terrain mix, settlements and roads (the M1 criteria); the developed world has no monuments or drawn sizes (M4 D6) | ~15 s |
| | `run_preset_meets_its_criteria`, `run_preset_is_the_same_regardless_of_threads` | GP-21 and GP-23: the run island (4 seeds) has the M1 terrain, settlement and road criteria, 4 regions, 4-8 monuments per region on allowed ground away from settlements (beyond army.ron `threat.radius`, read from the data) and each other, a size for every settlement with the seat the unique largest, and 3,500-5,500 people; its monuments and sizes don't depend on the thread count | |
| `run_start.rs` | `twenty_seeds_draw_valid_varied_starts` | M4 step 1 (GP-21, GP-23, GP-27), world level with no App: for seeds 1..=20 the run island has 4 regions, a drawn size for every settlement and 4-8 monuments per region on allowed ground away from settlements and `min_spacing` apart. The start (`draw_start`) is seeded, in another region than the rival's seat (the unique largest settlement), away from every settlement, with a walkable path to the seat, 2-3 monuments within `start.monument_radius` (no more than a day's march), a terrain label its tile really has, and three distinct start boons. Across the seeds the start terrain, the band and the run modifiers each take at least 3 values | ~10 s |
| | `a_run_starts_identically_from_its_seed` | GP-21: two apps of one seed hash the same after setup and after 200 ticks, and the installed run (`RunProgress`, the player's seat, the offer's cards, the champion's trait, `RunStarted`) equals the pure `draw_start` on the generated world; every settlement is populated; one start boon offer of three distinct cards waits; the champion is the player's captain leading the drawn band, each follower with its group's stats and gear (GP-27) | ~3 s |
| | `choosing_the_start_boon_applies_once` | GP-27: on three fresh seed-42 runs, the start boons with food, arms and goodwill (found by effect) are each put into the offer and picked: food is added exactly once, every member is armed, the settlement nearest the champion holds the boon's goodwill toward the player (through the goodwill ledger, empty again by the end of the tick). A second `ChooseOffer` for the same offer is refused and counted in `rejected_commands`, and nothing applies twice | ~2 s |
| | `abandoning_ends_the_run_and_stops_the_clock` | GP-21: only the player can abandon. The outcome, its cause and `RunEnded` are recorded on that tick; afterwards `SimTick` and the state hash freeze and commands are no longer applied | <1 s |
| `conquest.rs` | `a_fed_village_under_a_passive_garrison_grows_loyal` | M4 step 2 scenario (GP-05, GP-06, SIM-24), run island of seed 42. The shared prefix `conquered`: the champion picks the start boon with food, and the band of 10 attacks the nearest hamlet (56 people), breaks its militia and holds the centre (taken at tick 1603). It asserts that blows were struck and that the hamlet changed hands at least `conquest.hold_hours` after the defenders broke (broke at 1402), the `Conquest` entry (taker, hamlet, previous owner, cause, head count) and that every resident follows the player, seeded at `(goodwill + 1) / 2` (0.41: the attack's provocation and four killings cost goodwill -0.18 toward the player). This branch: a passive `Garrison` stands for three day ticks and mean loyalty rises at each (0.41, 0.52, 0.59, 0.64), with no unrest, no threat from the garrison (`Threat.level` 0) and food security at least `loyalty.food_days` | ~13 s |
| | `a_plundered_village_loses_faith` | GP-06, SIM-24: the band leaves; a 40-man raiding band plunders the unguarded hamlet and moves on. Food security falls (0.46, 0.16, 0.05 days) and mean loyalty falls at each day tick and ends below its seed (0.50, 0.42, 0.34 from 0.41). The raiders' plunder lowers goodwill toward the raiders, not the owner, so this branch covers harm through food; treatment (goodwill toward the owner) is covered by the sim_agents unit test `goodwill_toward_the_owner_reaches_drift` | ~13 s |
| | `a_starved_conquest_revolts` | GP-06, SIM-24: the stores spoil, the farmers are cut off and the band leaves. Mean loyalty sinks below `revolt_threshold` at the first day tick (0.30 from its 0.41 seed), and a day later the hamlet revolts (tick 4800, 1.33 days after the conquest; asserted within 1-3 days, and a day of unrest first. Before M4 step 3 the seed was a neutral 0.5 and the revolt came at 2.33 days): a `Revolt` entry, owner and residents independent, loyalty back at `independent_target`, no conquest counted | ~13 s |
| | `a_conquest_and_its_revolt_are_thread_independent` | NFR-08 (GP-05, GP-06, SIM-24): the prefix and the starved branch, played multi- and single-threaded, give equal state hashes every 100 ticks to three days after the conquest (72 checkpoints), and the hamlet revolts. It is the only serial-versus-parallel check with an owned settlement at day ticks (loyalty drift with a real target, the unrest clock, the revolt); replays can't cover it because they don't redo the branch's store edits | ~60 s |
| `village_day.rs` | `five_thousand_villagers_live_a_full_day` | M2: 5,000 villagers eat, work and sleep through a day (`villagers.ron`, `economy.ron` tuning) | ~30 s |
| `famine.rs` | `cutting_off_farmers_starves_that_village_within_two_days` | M2: food security responds to production, and the famine stays local | ~17 s |
| `army_march.rs` | `an_army_of_200_marches_around_a_lake_as_a_crowd` | M3 / SIM-30: 200 followers cross 220 tiles around a lake, stay a crowd, never stand on water, and parallel equals serial | ~12 s |
| `posture.rs` | `posture_orders_how_an_army_treats_a_village` | M3 / GP-03, SIM-13: food taken, harm and villager reactions order passive < neutral < aggressive | ~15 s |
| `starving_army.rs` | `an_unfed_army_loses_morale_then_deserts_then_starves` | M3 / SIM-31: morale falls, desertion begins within `first_desertion_within_days`, starvation only afterwards; a fed army is fine. Starvation damage begins at day 0.78 and kills only after about 2.5 more days, so no one starves to death in its 3 days and the G4 fix (starved soldiers are removed) leaves its numbers unchanged | ~16 s |
| `orders.rs` | `invalid_commands_are_refused` | Command validation (someone else's captain, unknown targets) | ~3 s |
| | `posture_and_cancel_apply_to_own_captain`, `appointing_a_captain_splits_the_army`, `recruiting_turns_villagers_into_followers`, `equip_trade_and_scout`, `attacking_a_village_raises_its_militia`, `skirmish_scenario_is_reachable_from_config` | GP-01 and GP-02: each order does what it says. The attack also checks GP-05: the militia breaks, the army holds the centre, and the village changes hands (`conquests` 1, a `Conquest` in the chronicle, every resident sworn to the warband) | ~3 s each |
| `docs.rs` | `every_data_field_is_documented`, `howto_paths_exist`, `markdown_links_resolve`, `field_extraction_finds_nested_fields_and_skips_comments` | M3b: [DATA.md](DATA.md) covers every RON field; the handbook names only real files and its links resolve | <1 s |

## Unit tests (in the crates)

These run in seconds and sit next to the code they test.

- **sim_core:**
  - calendar phases and clock; a game hour is a 24th of a day
  - RNG stream independence
  - hash reference vectors and the coverage check
  - replay parsing (with a run's config) and divergence finding
  - commands applied exactly once
  - a finished run stops the clock and freezes the hash; outside a run it never stops
  - a tick's events are recorded in sorted order whatever order they were written in
  - offers are answered once and expire; modifiers are exact no-ops when empty
- **sim_world:**
  - map chunk revisions and hashing
  - noise
  - A* paths
  - flow fields: detours, windows, cache
  - world generation fields and roads
  - settings validation, both presets, monument and size rules
  - region count and region edges
  - size draws: tier shares, seeded, the seat is the unique largest
  - monument site rules, weighted kind picks, nearest-tile ties
  - placed monuments suit their kind's site rule on the run islands of seeds 42 and 7 (`placed_monuments_suit_their_kind`); a kind that fits nowhere falls back to the next that does, which is the kind recorded (`a_kind_that_fits_nowhere_falls_back_to_one_that_does`)
  - installing a world spawns settlements, then monuments
- **sim_economy:** inventory and stockpile rules, transfer operations, the resolver serving lower SpawnIds first, data validation, layout rings.
- **sim_agents:**
  - utility choice (best score, inertia, ties), eating, response curves, data validation (including `rejects_bad_loyalty` and `rejects_bad_goodwill`), apportioning
  - goodwill stays sorted by faction and clamped to -1..1 (`goodwill_stays_sorted_and_clamped`)
  - the goodwill ledger (GP-22, `spread_is_order_independent`): the same changes pushed in three orders into worlds whose settlements were spawned in three orders give identical goodwill; a change counts in full at its settlement and `goodwill.spread_fraction` of it within `spread_radius` (centre to centre), not beyond; sums are clamped; zero deltas and unknown settlements change nothing; the ledger is emptied
  - `set_owner` (GP-05, GP-06): only the settlement's own villagers who aren't enlisted change allegiance and loyalty, seeded from goodwill toward the new owner (or `independent_target`), whatever order they were spawned in; the provocation and the unrest clock are cleared (`set_owner_hands_over_residents_in_spawn_id_order`); anything that isn't a populated settlement is left alone (`set_owner_ignores_what_is_not_a_populated_settlement`)
  - loyalty (SIM-24): the target weighs food security, goodwill and time, and the shipped tuning puts a starved conquest below the revolt threshold and a fed one above its seed (`the_target_weighs_food_goodwill_and_time`); a day's drift closes part of the gap (`drift_closes_part_of_the_gap`); run as systems, a starved conquest falls a little each day until its mean is below the threshold and `below_since` is set, a fed one rises, an independent one settles at its target, enlisted villagers are untouched, and feeding the starved one clears the clock (`loyalty_drifts_toward_its_target_each_day`); the owner's `LoyaltyDrift` modifier scales the rate (`the_loyalty_drift_modifier_speeds_drift`); treatment counts: equally fed, a village that likes its owner ends the day more loyal than one that hates it, goodwill toward another faction counts for nothing, and the shipped `w_goodwill` is above 0 (`goodwill_toward_the_owner_reaches_drift`). The conquest scenario's branches are driven by food and time (deeds now move goodwill too; `deeds_move_goodwill_and_word_spreads` covers them)
- **sim_military:**
  - data loading, hostility rules, the fighter grid's masked search matching a full search, formation slots; `shipped_data_loads` also checks the conquest relations (the hold inside the contest radius, `garrison.keep_days` at least villagers.ron `loyalty.food_days`)
  - `progression.ron` loads and its validation rejects bad cards and bad `requests`, `invitations` and `drifters` blocks (`shipped_data_is_valid`, `rejects_bad_data`)
  - start draws (GP-21, GP-27, `a_run_start_is_seeded_and_fits`): on the run island of seed 42 the same seed gives the same start, the seat is the largest settlement, the band starts in another region with 2-3 monuments nearby and a path to the seat, and three distinct start boons are offered; weighted card draws skip zero weights and respect groups; a start spot is labelled by a kind its tile has (`a_spot_is_labelled_by_a_kind_it_has`)
  - a run app (`a_run_is_set_up_its_boon_picked_once_and_abandoned`, ~1 s): setup spawns the drawn band (seed 42: a mixed band, each follower with its group's stats and gear, the champion with the captain's) under a champion with one trait and posts the start offer; the "Rats in the stores" modifier takes a tenth of each food, rounded down, from every storehouse and nothing from homes; the pick adds the boon's food once and a second is refused; only the player can abandon, and abandoning stops the clock
  - army.ron's `conquest`, `garrison` and `recruit.loyalty_bonus` are refused when negative, zero where they must be positive, or non-finite, and a `garrison.revolt_hours` shorter than a day is refused, since the rising must last until the next daily check (`rejects_bad_conquest_and_garrison_data`)
  - conquest (GP-05, GP-28, a run app, <1 s each). `transfer_hands_over_reports_seats_and_wins_the_seat`:
    - it hands the settlement and its residents over, seats the landless player at its first village only, and counts conquests, but not revolts;
    - it reports `Conquest`/`Revolt` events with the previous owner and the cause;
    - Garrison is refused on land the player doesn't hold, and Attack on land it does;
    - taking the rival's seat wins the run.
  - revolts (GP-06). `unrest_frees_the_unguarded_and_raises_the_weakly_held` calls `revolts` at day ticks:
    - after a day of unrest, an unguarded conquest goes free;
    - a garrison of 2 faces a rising (provoked against its owner until the next check);
    - a garrison of more than the able residents is left alone;
    - a rising nobody still fights is put down, and every resident loses `crushed_loyalty` (this test covers only the crush: it never steps the app, so no rebel is in the grid).
  - `a_rising_beats_a_weak_garrison` (GP-06, ~5 s): on the real schedule, a day of unrest under a two-man garrison raises the residents at the day tick; six hours later militia `against` the owner stand in the field, and at the next daily check the settlement goes independent with a `Revolt` entry.
  - `a_garrison_marches_to_its_village_and_ends_when_it_is_lost` (GP-05): Garrison points a distant band at the centre, and the order ends (with a log line) once the settlement is lost.
  - threat (GP-05, GP-06, SIM-13, `an_owners_army_frightens_only_a_risen_village`): the owner's aggressive army at the centre adds no threat; once the village rises against it, it does; another faction's army always does.
  - orders and ownership (GP-05, GP-06, a run app each):
    - `a_held_village_is_neither_plundered_nor_provoked`: an aggressive army with short baggage holding a village it has broken (Attack step 2) doesn't forage there, so nothing provokes the militia and the hold takes the village;
    - `an_owned_village_supplies_its_owners_army_and_is_never_plundered`: GatherFood and foraging by the owner's aggressive army draw only the food beyond `garrison.keep_days`, counted as `supplied`, with no seizing or provocation; a store with nothing spare leaves the forage slot open;
    - `recruiting_answers_to_the_owner_and_its_loyalty`: a rival's village sends nobody ("answers to"), and your own sends more at mean loyalty 1 than at 0;
    - `equipping_from_your_own_village_provokes_nobody`: an aggressive Equip provokes an independent village but not your own;
    - `a_deserter_comes_home_to_the_current_owner`: a deserter whose village changed hands while it was away comes home sworn to the new owner with the residents' current mean loyalty.
  - band life (GP-22, SIM-38, a run app each):
    - `deeds_move_goodwill_and_word_spreads`: a passive army buying food with wood posts a Trade deed, an aggressive raiding band a Plunder and a Provoke; a tick later the village's goodwill moved by progression.ron's `goodwill_deeds` and a neighbour within `goodwill.spread_radius` by `spread_fraction` of that, and the ledger is empty;
    - `giving_hands_over_food_beyond_the_keep`: a Give order at the village hands over the baggage's food beyond `give_keep_days` (counted in `donated`, the storehouse gains it, the order ends), and an army with nothing to spare gives nothing;
    - `a_foraging_band_lives_off_the_land`: Forage with radius 0 or beyond `forage.max_radius` is refused; the champion's band with an empty baggage holds where it stands, and six harvests bring `6 x per_member x people` (give or take a unit each) into the baggage (`foraged`) and take exactly that from the patches; bare land ends the order ("nothing left to forage"), and so does a full baggage ("baggage is full").
  - requests (GP-22, a run app each, ~1 s):
    - `a_hungry_village_asks_for_food_and_rewards_the_giver`: a village with its food cleared posts a BringFood at the second hourly sample (tick 100), not the first; a player band's Give counts exactly what arrived (`donated`); at the next hour the request is done (a `RequestDone` chronicle entry with `Cause::Request`), the village's goodwill rises by `goodwill_deeds.request` a tick later, the cooldown is set, and the reward's goods or volunteers reach the band;
    - `raiders_near_a_village_ask_to_be_driven_off`: a raiders' band within the threat radius posts a DriveOff at tick 0; once it is gone for `clear_hours`, a player captain near gets the food reward (capped at `reward_max_food`) and goodwill rises; with no player captain near the request fails and goodwill stays;
    - `an_unanswered_request_lapses_and_the_village_waits`: with a 2-hour deadline the request fails at tick 300; the still-hungry village asks nothing during its 3-hour cooldown and asks again at tick 600; a village the rival owns never asks.
  - `a_village_that_likes_the_band_invites_it_in` (GP-22, a run app): villages liked above `invitations.goodwill` with a player band near post Invitation offers (accept/decline, expiring); one liked too little or with no band near doesn't; accepting hands the village over with every resident's loyalty at `(goodwill + 1) / 2` and an `Invited` chronicle entry (`Cause::Invitation`); declining and lapsing set the cooldown, and neither village offers again within it.
  - `a_village_that_likes_the_band_sends_it_drifters` (GP-22, two run apps): at the day tick a village liked at `drifters.goodwill`, out of the champion's reach, sends a player band exactly `per_settlement` (forced to 2) of its own working-age villagers, enlisted as followers with allegiance PLAYER, the village's population, the band's baggage capacity and `recruited` updated, and one `DriftersJoined` event; liked a little less, it sends nobody.
  - wild food (SIM-38, `patches_deplete_and_regrow`): patches count grass and forest tiles only; the patches within a radius are found by their nearest tile and emptied in index order; asking for more than is left takes what is there; a day's regrowth adds `regrow_per_day` of capacity, never beyond full.
  - starvation deaths (G4, `a_starved_soldier_and_captain_are_removed`): a soldier and a captain who starve to death in upkeep are despawned on the next tick and counted as lost, and a follower takes command.
  - `survey_twenty_seeds` (`#[ignore]`, run with `--include-ignored survey --nocapture`): prints each seed's start draws for tuning `progression.ron`; `SURVEY_SEEDS=42,7` picks seeds
- **sim_headless:** scenario names round-trip, a run's config travels in the replay header (`scenario_names_round_trip`); a run config selects the run island and everything else the developed world (`a_run_config_selects_the_run_island`); `verify_replay` refuses a scenario it doesn't know (`an_unknown_replay_scenario_is_an_error`).
- **game_render:**
  - detail bands and hysteresis
  - camera maths: pan, zoom, rotation snapping, easing
  - texture determinism
- **game_ui:** the picking ray, action descriptions, text bars, the catch-up cap on ticks per frame.

## Writing tests

- Prefer a **scenario test** that asserts the requirement over one that asserts implementation details. See [HOWTO.md](HOWTO.md#add-a-scenario-test).
- **Print the numbers** a test checks, so a failure in CI explains itself (`-- --nocapture` shows them locally).
- If a test needs a specific world layout, **search the generated world** for it (as `army_march` does for a pair of villages) or **edit the map** before the first tick with `WorldMap::set_terrain`, rather than hard-coding coordinates from one seed.
