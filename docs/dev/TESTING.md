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
| | `armies_with_orders_are_deterministic_and_replay` | Armies plus 200 random orders: parallel equals serial, and the replay reproduces | ~60 s |
| `worldgen.rs` | `shipped_world_is_2048_square_in_64_tile_chunks`, `same_seed_same_island_regardless_of_threads`, `different_seeds_give_different_islands`, `every_seed_meets_the_m1_criteria`, `the_sim_spawns_settlements_and_roads` | SIM-01 and SIM-02: world size, determinism, terrain mix, settlements and roads (the M1 criteria) | ~10 s |
| `village_day.rs` | `five_thousand_villagers_live_a_full_day` | M2: 5,000 villagers eat, work and sleep through a day (`villagers.ron`, `economy.ron` tuning) | ~30 s |
| `famine.rs` | `cutting_off_farmers_starves_that_village_within_two_days` | M2: food security responds to production, and the famine stays local | ~17 s |
| `army_march.rs` | `an_army_of_200_marches_around_a_lake_as_a_crowd` | M3 / SIM-30: 200 followers cross 220 tiles around a lake, stay a crowd, never stand on water, and parallel equals serial | ~12 s |
| `posture.rs` | `posture_orders_how_an_army_treats_a_village` | M3 / GP-03, SIM-13: food taken, harm and villager reactions order passive < neutral < aggressive | ~15 s |
| `starving_army.rs` | `an_unfed_army_loses_morale_then_deserts_then_starves` | M3 / SIM-31: morale falls, desertion begins within `first_desertion_within_days`, starvation only afterwards; a fed army is fine | ~16 s |
| `orders.rs` | `invalid_commands_are_refused` | Command validation (someone else's captain, unknown targets) | ~3 s |
| | `posture_and_cancel_apply_to_own_captain`, `appointing_a_captain_splits_the_army`, `recruiting_turns_villagers_into_followers`, `equip_trade_and_scout`, `attacking_a_village_raises_its_militia`, `skirmish_scenario_is_reachable_from_config` | GP-01 and GP-02: each order does what it says | ~3 s each |
| `docs.rs` | `every_data_field_is_documented`, `howto_paths_exist`, `markdown_links_resolve`, `field_extraction_finds_nested_fields_and_skips_comments` | M3b: [DATA.md](DATA.md) covers every RON field; the handbook names only real files and its links resolve | <1 s |

## Unit tests (in the crates)

These run in seconds and sit next to the code they test.

- **sim_core:**
  - calendar phases and clock
  - RNG stream independence
  - hash reference vectors and the coverage check
  - replay parsing and divergence finding
  - commands applied exactly once
- **sim_world:**
  - map chunk revisions and hashing
  - noise
  - A* paths
  - flow fields: detours, windows, cache
  - world generation fields and roads
  - settings validation
- **sim_economy:** inventory and stockpile rules, transfer operations, the resolver serving lower SpawnIds first, data validation, layout rings.
- **sim_agents:** utility choice (best score, inertia, ties), eating, response curves, data validation, apportioning.
- **sim_military:** data loading, hostility rules, the fighter grid's masked search matching a full search, formation slots.
- **game_render:**
  - detail bands and hysteresis
  - camera maths: pan, zoom, rotation snapping, easing
  - texture determinism
- **game_ui:** the picking ray, action descriptions, text bars, the catch-up cap on ticks per frame.

## Writing tests

- Prefer a **scenario test** that asserts the requirement over one that asserts implementation details. See [HOWTO.md](HOWTO.md#add-a-scenario-test).
- **Print the numbers** a test checks, so a failure in CI explains itself (`-- --nocapture` shows them locally).
- If a test needs a specific world layout, **search the generated world** for it (as `army_march` does for a pair of villages) or **edit the map** before the first tick with `WorldMap::set_terrain`, rather than hard-coding coordinates from one seed.
