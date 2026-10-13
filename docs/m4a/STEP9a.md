# M4a step 9, part one: specialised towns (SIM-62) — lane B ("Govern")

Branch `claude/m4a-govern`, from merge point 2 (`8182929`, steps 1 to 5 in). This is the work lane B does before the smoke-playtest verdict: [M4A_PLAN.md](../M4A_PLAN.md) "Step 9", only its specialties part, and the MP2 row of "Dependencies and merge points". Design: [LORDSHIP_DESIGN.md](../LORDSHIP_DESIGN.md) section 8.4. Decisions: [ADR 0020](../adr/0020-lordship.md) "As built: step 9 (specialties)". The balance census that came with it is in [CENSUS.md](CENSUS.md). Step 8 (building orders) has not started; it waits for the verdict. Stewards are step 9's second part.

## What was built

**Sim (run only).**
- **The data.** villagers.ron `specialties` is `sim_agents::SpecialtyRule`, with `kind`, `site`, `min_tiles`, `road_degree`, `radius` and `shares`. Its matcher is `matches`, its share scaler `scale`, and its validation checks the radius (1 to 64) and that multipliers are at least 0. Loaded through `VillagerData.specialties`. While the list equals `SpecialtyRule::shipped()` it is left out of the data's RON text, so the developed world's hash and saves did not move. An empty list turns specialties off.
- **Counting the site.** `sim_economy::site_tiles(map, centre, radius, rule)` counts the reachable tiles of a site rule within `radius` of the centre, with `build_settlement`'s own `site_ok` and walkable reach, on the bare map. `sim_agents::population::road_degree` counts worldgen roads that end at the town. `specialty_of` returns the first matching row.
- **Populate's run branch.** It needs both a sized island and `RunState::in_run`. Before any town is built, it computes each town's row and scaled shares. Each town's `apportion` uses those shares. `Specialty` goes on the settlement, and the counts go in `PopulationReport.sites`. `PopulationReport.draws` records every town's populate-stream word position (`get_word_pos`) in every preset.
- **Market hire.** `sim_military::govern::market_hire` runs at the end of slot 15. On a hire hour (`market_hire_due`), each Market town the player owns camps a band of `market_band` men and posts their `Hire` offer through `sim_core::post_offer`. The band comes from `sellswords::camp`, factored out of `sellswords::setup` without changing its order or output. Hiring, declining and lapsing go through the sellswords' existing `answer` and `lapsed`. There is no new state and no new RNG stream: the direction is the golden angle times the town's SpawnId.
- **Diagnostics.** `sim_headless --workshops` prints a town's specialty and site counts after its name. `sim_headless::headless_app_with(config, edit)` edits the world before populate, and `common::run_app_without_specialties` uses it.

**UI** (a Sonnet agent; game_ui only, read-only):
- `labels::specialty_tag`: a `[Mining]` line under the label's name.
- `panels::specialty_line`: the panel's "Specialty: Mining — ore and smiths: a Forge here arms your men" line. For a Market town it reads "a crossroads: every 5 days hired men camp here while you hold it", with the days taken from `market.hire_days`.
- Unit test: `specialty_lines_name_the_meaning`.

**Tests:**
- `tests/tests/specialty.rs`: `specialties_follow_the_site`, `mining_towns_have_miners_and_smiths`, `a_market_town_posts_hire_offers`, `populate_draws_are_unchanged`.
- sim_agents unit tests: `villagers_ron_ships_the_default_specialties`, `specialty_rows_match_and_scale`.
- `tests/tests/census.rs` (ignored): the census.

**Docs:** DATA.md (`specialties.*`, `market.*` as built), GLOSSARY (Specialty; Sellswords now covers Market bands), TESTING, ARCHITECTURE (populate's run branch, slot 15's Market hire, `Specialty`'s inserter), ADR 0020 "As built: step 9 (specialties)", and this note and CENSUS.md.

### What the islands look like

| Seed | Mining | Market | Fishing | Timber | Farming |
| --- | --- | --- | --- | --- | --- |
| 1 | 4 (#18, #19, #20, #22) | 6 | 2 | 1 | 12 |
| 7 | 3 (#14, #23, #24) | 8 | 1 | 1 | 12 |

Mining towns against the same towns laid out without specialties (`mining_towns_have_miners_and_smiths`):

| Town | People | Miners | Smiths | Smithies, mines |
| --- | --- | --- | --- | --- |
| seed 1 #20 | 163 | 3 → 8 | 3 → 6 | 1, 1 |
| seed 1 #22 | 152 | 2 → 8 | 3 → 5 | 1, 1 |
| seed 7 #14 | 161 | 2 → 7 | 3 → 5 | 1, 1 |
| seed 7 #23 | 137 | 2 → 6 | 2 → 4 | 1, 1 |
| seed 7 #24 | 310 | 4 → 15 | 6 → 10 | 1, 1 → 2, 2 |
| seed 1 #18, #19 (hamlets of 68 and 43) | | 1 → 4, 0 → 2 | 2 → 3, 1 → 1 | 1, 0 → 1 |

On Forge they make 8 to 39 weapons a day on their own ore and keep going, where a farming town's starting ore runs out in 2 to 5 days (CENSUS.md).

## Deviations from the plan and design, and why

1. **Fishing and Timber thresholds are 160 and 200, not the design's 400 and 600.** Coast (beach or grass next to water) and forest edge are edges, so within 40 tiles they run to about 270 and 340 on the run island (seeds 1 and 7: most towns 80 to 200). At 400 and 600 no town ever matched, and the two rows were dead. At 160 and 200 each island has one or two of each. They are tried after Mining and Market (the data block's order, which the design also gives in section 10; section 8.4's table lists Timber before Fishing). This changes data only; a two-line edit moves them back.
2. **`Specialty` is inserted by populate's run branch, not `setup_run`.** Populate is where the site is read and where the shares apply, and populate runs before `setup_run`. The component is still run only.
3. **The run gate is "sized and `in_run`".** Populate's existing branch keys on `SettlementSize` alone. A test that builds the run island without a `RunConfig` gets no specialties, so the branch is as run-only as the design asks.
4. **Market hire hours run from tick 0, not from the taking.** On every `hire_days` (5 days, rounded to whole hours), each Market town the player owns then posts. This needs no new state (no "last posted" field, nothing to save). A town taken on day 4 posts on day 5. The interval holds, and the first offer comes between 0 and 5 days after the taking.
5. **The hire band's direction comes from the golden angle times the SpawnId, not a draw.** The design asks for "deterministic"; this needs no stream key, and towns' bands don't all camp on the same side.
6. **Timber's half price is not built.** It belongs to step 8 (`construction` reads `Specialty::Timber` and halves the wood). It is recorded here and in ADR 0020 for step 8.
7. **The mining test allows a hamlet to keep its one smith.** A town of 43 rounds to one smith either way. The test asserts more miners per town, no fewer smiths, smithies or mines, and more smiths across the Mining towns (20 to 34).

## Names later steps use

- `sim_agents::{Specialty, SpecialtyRule, SiteCounts, PopulationReport}`.
- `sim_agents::population::{specialty_of, road_degree}`.
- `sim_economy::{site_tiles, SiteRule}`.
- `sim_military::govern::{market_band, market_hire_due}`.
- `sellswords::camp`, crate-private.
- `sim_headless::headless_app_with`.
- `common::run_app_without_specialties`.
- **Step 8:** read `Specialty::Timber` on the town at the point of use and halve the wood needs. The design says "Buildings there need half the wood"; with a Wright steward's discount, decide the stacking there.
- **Step 10:** CENSUS.md's proposals. The bot's offer step answers any `Hire` it can reach and pay for (`can_hire`), so it hires a Market town's band when its army happens to be near; it does not walk over for one (lane D).

## Checks and results

All commands used `-j 2` and `CARGO_INCREMENTAL=0`, and tests ran with `--test-threads=2`.

| Check | Result |
| --- | --- |
| Guarded stdout (release: developed, skirmish, battle) against `docs/m4a/baseline/*.out` | byte-identical, all three |
| `specialty` (4 tests) | pass |
| Unit tests of the touched crates (sim_agents, sim_economy, sim_military, sim_headless, game_ui) | see the gate below |
| Run-preset tests, determinism, docs, `village_day`, `famine` | see the gate below |

## Open issues

- **Seed 7's #22** (280 people, Farming) starves in every census variant, with or without specialties: a site too poor for its size (CENSUS.md).
- **Weapons may be too plentiful** for step 10's target (12 to 25 a day per smithy on Forge): CENSUS.md proposal 1.
- **The bot hires Market bands only by chance** (when in reach); walking over is lane D's.
- **No screenshots** of the label line and the panel line (lane C's dev box).
