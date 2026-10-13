# M4a step 4: edicts, work, dues and table (GP-34, GP-17 part) — lane B ("Govern")

Branch `claude/m4a-govern`, from merge point 1 (`d8ac0ab`, steps 1 to 3 in). Plan: [M4A_PLAN.md](../M4A_PLAN.md) "Step 4"; design: [LORDSHIP_DESIGN.md](../LORDSHIP_DESIGN.md) section 8.1; [ADR 0020](../adr/0020-lordship.md) "As built: step 4". Everything is run only.

## What was built

**Sim (lane B's files).**
- `sim_agents::edicts`: the write API `Edicts::order_work`, `order_dues`, `order_table` (a pending setting, with who set it), plus `is_default`, `locked` and `feasting`. `Rationed` now carries `{ hunger, work }` (copied from progression.ron when the ration starts) and sits on the settlement as well as its residents. `Edicts.feast_work` is new. Both types are amended in place under `SAVE_FORMAT` 2.
- `sim_agents::jobs` (new): `reassign`, `reassign_to`, `nearest_with_room`, `occupation_at`, `movable`. A move uses populate's rule (nearest workplace of the kind with room). Only empty-handed villagers who aren't mid-cycle move. Carts are resized, and a Work or Haul under way ends.
- Rationing: `update_needs` (hunger ×`hunger`), `update_food_security` (the daily need ×`hunger`, so food days stay honest), `perform` (work cycles ×`work`, `act::rationed_ticks`).
- The AI: a `Mustered` villager never works and idles on the green (near the centre). During a feast, work scores are ×`feast_work` and idlers gather on the green.
- `sim_military::govern`:
  - `step` (slot 15) applies pending settings hourly, with the lock, `EdictSet` and the first wave. It ends feasts (with the Festival deed and the Table term if the town is still fed). At dawn it runs the later Work waves, the Levy's next muster, and the ration for newcomers. It resets the edicts of a town the player doesn't own, and sweeps the Levy and Ration marks off soldiers.
  - Work waves (`work_wave`, `plan`, `destination`, `wave_size`): Forge flips every smithy to weaponsmithing with `set_recipe` and restores `layout_recipe` on leaving. Fields, Walls and Balanced are in too; Balanced sends the idle-born back to idleness.
  - Dues and Table terms: `dues_term` and `table_term`, summed by `tenure::step`.
  - Commands and validation: the `Govern` command (`command`) and `refusal`. Refusals: locked, already set or ordered, a feast the town can't hold, a submitted town's Dues held at Tithe until it settles. Build, Steward and Release are "not yet".
  - Read-only forecasts for the panel: `work_forecast`, `dues_forecast`, `table_forecast` and `lock_left`.
  - `build_speed` (Walls ×1.5, for step 8). `levy_sizes` and `mustered`.
- `orders`:
  - The `Govern` arm of `apply`.
  - `lands`, a pass at the end of `run_orders` on the tick after each hourly step. In the hour a Dues dial turned, then daily, it sends Tithe food and arms, and Levy men, to the nearest player captain within `edicts.reach`. It writes one `Tribute` per receiving captain.
  - Helpers: `collector` and `store_arms`.
- Walls: `update_threats` multiplies the radius by `walls_threat`, and `resolve_strikes` adds `walls_defence` to the militia within `garrison.radius` (`WallsParam`).
- The exhausted worker (step 2's open issue), runs with the smithy fixes only: a worker at rest ≤ 0.1 no longer scores Work he can't start. By day his Sleep takes the evening factor, so he naps instead of looping Work, Done.

**UI** (Sonnet; `crates/game_ui/src/panels/govern.rs`, `labels.rs`, `feed.rs`, minimal `panels.rs` edits):
- The settlement panel's WORK, DUES and TABLE rows: buttons, a pending setting marked, refused settings greyed with the reason, the lock's day, and one-line forecasts.
- Label badges with a first-wave countdown.
- The inspector's "Smith, moved from farmer by the Forge edict, day 6", "Mustered on the green since day N" and "On short rations".
- The feed's grouped Dawn line for a day's tribute.
- Forecasts are computed by an exclusive system every 0.5 s into a UI-side resource; nothing writes sim state.

**Bot** (Sonnet; `crates/sim_headless/src/bot.rs`): a daily `govern` pass. It only sends what `refusal` allows, at most four commands a day. The rules:
- Forge at the town with the most smithies while it has at least 2 days of food; back to Balanced under 1 day.
- Tithe at fed, loyal towns within 60 tiles of a captain.
- One Levy at a time, at a big loyal town.
- Never Ration or Feast.

**Tests** (Sonnet; `tests/tests/edicts.rs`, 10 scenarios on seed 42's run island): every case in the plan's table passes. The numbers they observed:

| Test | Observed |
| --- | --- |
| `the_first_wave_moves_within_the_hour` | Forge ordered at tick 2, applied at tick 100, locked to 4900; one `EdictSet`; 5 moved with `Assignment { Forge, since 100 }` (forge trades 15 to 20) |
| `forge_moves_smiths_and_flips_recipes` | both smithies on weaponsmithing from the first hour; the gain is exactly min(free places, the 20% cap) = 5; Balanced after the lock restores the alternation |
| `fields_moves_workers_to_food` | food trades 114, then 123 after the first wave, then 128 after two dawns (waves of 9 and 5) |
| `walls_raises_haulers_and_the_watch` | carters 23 to 29 (+6); `build_speed` 1.5; a rival army 51.5 tiles off: threat 0 before, 0.127 after |
| `a_tithe_reaches_an_army_within_reach` | a captain 30.5 tiles off: baggage food 3 to 25, all 6 weapons and 4 tools, 6 followers armed, one `Tribute`; a captain 80.5 tiles off: nothing |
| `a_levy_musters_and_joins` | 7 of 364 mustered within the hour, none working; 14 waiting after a dawn; all join a captain 25 tiles off (with the dawn's 7: 21 men); population 364 to 343; Dues term -0.12 at a conquered town |
| `rationing_slows_eating_by_a_quarter` | hunger rises at exactly 0.75× (7,274 samples); `FoodSecurity.days` = food / (need × 0.75) |
| `a_feast_gathers_and_pays_if_fed` | eats 0.50 days of need; more villagers within 8 tiles of the centre; goodwill +0.080 (and at a neighbour); Table term +0.08 for 5 days, the next feast 6 days on; refused at a town without food |
| `dials_lock` | a second Work order refused "locked" (counted); Build, Steward and Release "not yet"; accepted again at the unlock tick |
| `defaults_are_no_ops` | the state hash with `Edicts` and without it (re-inserted at the end) is identical after 2,700 ticks |

**Docs:** DATA.md (`edicts.*` rows corrected), ARCHITECTURE (schedule rows; slot 15; `orders::lands`; `jobs`), GLOSSARY (Edict, Dial, Wave, Tithe, Levy, Muster, Ration, Feast, Dawn line, Lock, Collector, Green), HOWTO "Add an edict", TESTING (edicts, unit and UI tests), ADR 0020 "As built: step 4".

## Deviations from the plan, and why

1. **Moves fill free places only.** Workplaces keep their economy.ron slots, and populate builds just enough of each kind. A town whose smithies, mine and camp are nearly full gets few hands from Forge: seed 42's town #0 got 5 in the first wave and none at later dawns. So "the rest over three dawns" holds only where there is room. Step 8's ordered smithy (six places) is the designed lever. Raising workplace slots would be a data change for the census and step 10.
2. **Dues deliver on the tick after the hourly step** (`lands` in `run_orders`, which comes before `govern::step` in the schedule), so a Tithe set at the hour reaches its collector one tick later, still within the hour.
3. **Balanced sends the idle-born back to idleness** (the design says "back to the trade they had"; for the idle that is none). Workers holding goods or mid-cycle at dawn wait for a later dawn, so Balanced undoes Forge slowly: in the test, 1 of 5 returned in three dawns.
4. **The submitted town's tribute hold is in `govern::refusal` already** (Dues other than Tithe are refused until `demand.settle_days` after a submission), so step 5 only has to call `order_dues(Dues::Tithe, false)` when a town submits.
5. **`Rationed` carries its numbers** (and the feast's work factor sits on `Edicts`), because sim_agents can't read progression.ron. Both types are amended in place.
6. **The run bar's "Lands" line was skipped** (the Dawn line in the feed carries the same totals). `EdictSet` lines stay one per event: they are the player's own clicks.

## Names later steps use

- `sim_agents::Edicts::{order_work, order_dues, order_table, is_default, locked, feasting}`, `Rationed { hunger, work }`, `Edicts.feast_work`.
- `sim_agents::jobs::{reassign, reassign_to, nearest_with_room, occupation_at, movable}`.
- `sim_military::govern::{command, refusal, step, dues_term, table_term, build_speed, levy_sizes, wave_size, mustered, work_forecast, dues_forecast, table_forecast, lock_left, WorkForecast, DuesForecast, TableForecast}`.
- `sim_military::orders::{lands, collector, store_arms}` (crate-private).
- **Step 5 (lane A):** on a submission, `world.get_mut::<Edicts>(s).unwrap().order_dues(Dues::Tithe, false)`. The next hour applies it, and `refusal` holds it until the town settles. `dues_term` is already 0.0 for Tithe at a `TakenBy::Submitted` town.
- **Step 8:** `govern::build_speed` and `edicts.walls_build`.
- **Step 9:** stewards order with `by_player: false`. `Assignment.by_steward` is set from the dial's `by_player`.

## Checks and results

All commands used `-j 2` and `CARGO_INCREMENTAL=0`, and tests ran with `--test-threads=2`.

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | clean (after one reformat) |
| `cargo clippy -j 2 --workspace --all-targets -- -D warnings` | clean |
| Unit tests: sim_agents 30, sim_military 97 (+1 ignored), sim_economy 13, sim_core 39, game_ui 42, sim_headless 6 | all pass |
| Integration: edicts 10, smithy_chain 2, save_load 5, tenure 8, influence 6 (+1 ignored), invitation 2, village_day 1, famine 1, docs 4, orders 7, gather 2, run_start 4, determinism 13 | all pass |
| `conquest` | 3 of 4: `a_plundered_village_loses_faith` fails **on the base commit `d8ac0ab` too** (day 2 loyalty 0.318 there, 0.342 here, against the 0.35 line). See open issues |
| `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps` | clean (after two private links in bot.rs were unlinked) |
| Guarded stdout (release: developed, skirmish, battle) against `docs/m4a/baseline/*.out` | byte-identical, all three |
| Run replay: `--run --seed 42 --ticks 3000 --commands 50 --record`, then `--replay` and `--replay --single-threaded` | the same hash `0x354db2a45cde09dd` all three |
| `bot_run` (release, ignored) | see below |

BOT_RUN_RESULTS

## Open issues

- **`conquest::a_plundered_village_loses_faith` is red on the merged base** (MP1). It is the thin margin step 1 flagged (0.366 against 0.35). Step 3's Contest term pushed it to 0.318, and step 4 raises it to 0.342 (the exhausted nap). Per the plan, the scenario's food changes, never the data, e.g. a 0.4-day larder instead of 0.3. The test is lane A's, so it isn't changed here; the merge lead or lane A should fix it at MP2.
- **Forge needs places.** See deviation 1. Weapons per forge town (the census in the gap before step 8) will show whether workplace slots or step 8's smithies carry it.
- **No screenshots.** The panel grows by about nine lines on a bottom-anchored panel and may overlap the villager panel on short windows. Check it on the dev box.
- **The cost check** (NFR-11, the bot run's Lordship stages under 5% of the mean tick) runs at the merge point, per LANE_RULES. `govern::step`, `orders::lands` and the waves are spanned (`sim_military::govern::step`, `sim_military::orders::lands`).
- The bot's smoke run on seed 1 never chose a Levy town (none was both big and loyal near a captain), and two towns ended near 0 days of food. Watch it in `bot_run`'s numbers.
