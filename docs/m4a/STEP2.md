# M4a step 2: the smithy chain (SIM-64) — lane B ("Govern")

Branch `claude/m4a-govern`, from the M4a design commit `e0d68e9` (built beside step 1, which the merge lead builds at the same time). Plan: [M4A_PLAN.md](../M4A_PLAN.md) "Step 2"; design: [LORDSHIP_DESIGN.md](../LORDSHIP_DESIGN.md) section 8.6; [ADR 0020](../adr/0020-lordship.md) decision 9 and its "As built: step 2".

## What was built

All of it in a run only, behind `in_run` and economy.ron `haul.run_fixes` (`sim_agents::ai::run_fixes_on`). The M2 code paths are untouched when the gate is off.

| Fault (design 8.6) | Fix as built | Where |
| --- | --- | --- |
| Smithies stall full of one input (`work_state` counts inputs as fullness) | `work_state_fixed`: only outputs count as fullness; a workplace full of inputs is Ready (a cycle makes room) | `ai.rs` |
| Carters bring full 50-unit carts of one input, several to one job | the pickup is capped at `haul.input_target` (10) cycles' worth minus the workplace's stock (`act::input_cap`, read in `perform`); a carter takes no (workplace, good) delivery a carter or worker is on (`RunFixes::claims`, read before the parallel pass), and carters who chose the same free delivery in one tick are sent back in SpawnId order after it; at the storehouse an input pickup first empties the carter's hands (an interrupted haul's leftovers went to the smithy) | `ai.rs` (`decide`, `haul_job`), `act.rs` |
| Smiths with full hands loop (`Work`, `Done`) | `act::blocking_good`: at a workshop (a recipe with inputs; farms keep the M2 rule), hands too full for a cycle, in units or in the four slots (an output with no slot was lost), name the pile in the way; `work_state` returns `Drop(g)` (surplus inputs into the workplace, anything else to the storehouse) and `perform` ends a cycle start with such hands; a food fetch empties non-food goods into the storehouse first (weapons went home) | `ai.rs`, `act.rs` |
| Weapons wait at the forge (carters collect piles of 8) | `haul.min_load` (weapons 1, tools 2): carters collect such piles from that size, valued as at least a full load; a smith carries in half a load (4) itself | `ai.rs` (`haul_job`, `work_state_fixed`) |
| Only a town's second smithy makes weapons | `sim_economy::set_recipe` (pub; refuses a recipe made elsewhere, changes nothing then) and `layout_recipe` (the layout's alternation) for step 4's Forge and step 8's ordered smithies. No recipe changes in this step | `layout.rs` |

Also:
- economy.ron `haul: (run_fixes: true, input_target: 10, min_load: [("weapons", 1), ("tools", 2)], cycle_hold: 2.0)`, `sim_economy::HaulData`, validated (known goods, minimums at least 1, `input_target` at least 1, `cycle_hold` at least 1). The block is `skip_serializing_if = HaulData::is_shipped`: at the shipped values it is left out of `Economy`'s RON text, so the hash and save of the developed world did not move; changed values (a test's `run_fixes: false`) are hashed and saved.
- The settlement panel's "Workshops:" section beside the storehouse: each smithy and mine, what it makes and its stock without food (`game_ui::panels::workshop_line`).
- `sim_headless --workshops` (plain run or `--stats-every`): each smithy's and mine's recipe, workers, their cycles (work cycles plus their own hauls) and stock by town, then the island's weapons and tools by place (`stats::workshops`, `stats::census`, `GoodCensus`).
- Tests: `tests/tests/smithy_chain.rs` (`the_island_makes_weapons`, `no_smithy_stalls_full_of_inputs`); unit tests `haul_block_resolves_and_round_trips`, `set_recipe_checks_the_workplace` (sim_economy), `input_deliveries_are_capped`, `full_hands_name_what_is_in_the_way` (sim_agents), `workshop_lines_show_stock_without_food` (game_ui).
- Docs: DATA.md (`haul.*`), ARCHITECTURE (crate rows, schedule rows 3 and 5), TESTING, GLOSSARY (Cycle hold, Input delivery, Smithy fixes), ADR 0020 "As built: step 2".

## Deviations from the plan

1. **A sixth fault and a fourth `haul` field, `cycle_hold`.** With the five fixes, seed 1 made 40 to 44 weapons, but about 145 weaponsmithing cycles started: a smith left a cycle half done for a meal (Eat beat Work by a few hundredths) or for a haul `work_state` offered mid-cycle, and the inputs, already taken, were lost. Now a cycle under way stays Work (`Me::mid_cycle`), and one that has used inputs scores `cycle_hold` times as high (shown in the inspector as "cycle under way"). At 1.5 the island made 54 weapons, at 2.0 74, at 3.0 92 (before the review fixes below; 68 as shipped); mean hunger did not move at any of them. 2.0 ships: well over the target without the flood that 3.0 risks (weapons are +8 attack). Farms, pastures and other recipes without inputs only keep the mid-cycle rule (no hold), so food cycles change little.
2. **"One carter per job" covers carters only.** A worker fetching its own inputs (a load of 8) is not held back by another's fetch: with one fetch at a time the smiths stood waiting and the island made 29 weapons. Their fetches count as claims, so carters don't add to them. A smithy can therefore hold its cap plus a few smiths' loads (`no_smithy_stalls_full_of_inputs` allows the cap plus a load per smith, per input; the M2 rules filled it to 200).
3. **The stall test judges work hours with a fit worker.** A smithy whose workers are all starving, exhausted (rest at or below 0.1, `perform`'s own test) or enlisted has nobody to work it; on seed 1 one town's smiths fought the raiders at night and spent day 1 too tired to work (see open issues). The test also checks the input cap.
4. **The weapons criterion reads "where nobody looks" as homes and forges.** None in a home, at most a tenth at a forge (the last hours' work: a smith carries in four, a carter collects one). Strictly zero at a forge would hang on the instant the test samples. The panel now shows forge stocks too.
5. **Review fixes** (an adversarial review of the diff): a carter keeps a delivery it is on even if a worker claims it later (`Me::hauling`); a worker's surplus inputs go into the workplace, not the storehouse; the full-hands rule (`Drop`) applies only at workshops (recipes with inputs), so farms keep the M2 rule; the stall test checks each input against its own cap and its smithy's workers.
6. **`--workshops` cycles** are `Employment.cycles`, which counts a worker's own hauls as well as work cycles (as `--stats-every`'s "cycles by occupation" does).

## Names later steps use

- `sim_economy::set_recipe(&Economy, &mut Workplace, RecipeId) -> Result<(), String>` and `sim_economy::layout_recipe(&[RecipeId], index) -> Option<RecipeId>` (the alternation to restore when Forge ends: index = the workplace's position among its kind in `SettlementEconomy.workplaces`, as `layout::build_settlement` placed them).
- `sim_economy::HaulData` and `Economy::{min_load(good), input_target()}`; economy.ron `haul.*`.
- `sim_agents::ai::{RunFixes, run_fixes_on, MIN_LOAD}`, `sim_agents::act::{input_cap, blocking_good}`. Step 8's site deliveries should reuse `input_cap`'s rule (deliver at most the need minus the stock) and the claims in `RunFixes` (one carter per site and good).
- `sim_headless::stats::{census, GoodCensus, workshops}`; `sim_headless --workshops`.

## Checks and results

| Check | Result |
| --- | --- |
| Guarded stdout (release; developed `--seed 42 --ticks 4801 --stats-every 600`, skirmish `--skirmish --seed 42 --ticks 6000 --stats-every 2000`, battle `--battle 1000 --seed 42 --ticks 3000 --stats-every 600`), against baselines recorded from `e0d68e9` before any change | byte-identical, all three (`cmp`) |
| `cargo fmt --check`; `cargo clippy -j 2 --workspace --all-targets -- -D warnings` | clean |
| Unit tests: sim_economy, sim_agents, sim_headless, game_ui | all pass (sim_economy 21, sim_agents 13, game_ui 19, sim_headless 4) |
| `smithy_chain` | both pass (68 weapons: 63 in storehouses, 5 at forges) |
| `village_day`, `famine` (unchanged) | pass, untouched |
| `determinism`, `docs` | pass (13 and 4) |
| Run replay: `--run --seed 42 --ticks 3000 --commands 50 --record run.ron`, then `--replay run.ron` and `--replay run.ron --single-threaded` | recorded 51 commands, hash `0x9cf30340f172f84a`; both replays ok with the same hash |
| `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps` | clean |
| Cost: `--run --seed 7 --difficulty steady --ticks 7200 --timings --stats-every 2400`, three runs, base vs step 2 | live window (no `run ended` before tick 7,200): mean tick base 2.78, 2.49, 2.29 ms (mean 2.52); step 2 2.56, 2.51, 2.40 ms (mean 2.49): within noise, under +5% |
| `bot_run` (release, `--ignored`) | PENDING |

## Weapons census (three days, `--run --ticks 7200 --workshops`, no commands)

| Seed | Before (`e0d68e9`) | After (step 2) |
| --- | --- | --- |
| 1 | 10: 0 in storehouses, 10 at forges or in hands | 68: 63 in storehouses, 5 at forges |
| 7 | 0 | 20: all in storehouses |

Food security at the same hour (day 3, morning; 25 towns): seed 1 mean 2.82 days (lowest 0.3) before, 3.61 (lowest 0.7) after; seed 7 mean 2.78 (lowest 0.1) before, 3.52 (lowest 0.0, the same starving hamlet) after. Mean hunger 0.32 to 0.30 (seed 1) and 0.31 to 0.32 (seed 7). Carters no longer spend their trips on 50-unit carts of ore and wood, so food is hauled in sooner (the plan's risk "the smithy fixes shift run food" went the other way). Tools (storehouses and workplaces): seed 1 from 20 to 364, seed 7 from 57 to 395 (`--workshops`).

## Open issues

- **Exhausted workers loop at work.** A villager with rest at or below 0.1 by day scores Work (fit 0.2) above Sleep (day factor 0.05), but `perform` refuses to start a cycle, so it loops Work, Done all day until night (seen on seed 1 after a night of fighting raiders). This is the M2 AI, not the smithy chain; it costs a provoked town a day's work. Worth a fix in lane B's step 4 (where the Walls edict touches the militia) or M4b.
- Seed 7 makes far fewer weapons (20): few of its towns have a second smithy. Step 4's Forge and step 8's ordered smithies are the levers, as designed.
- Tools now pile up (about 360 on seed 1 by day 3); nothing but Equip uses them.
