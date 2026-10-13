# M4a step 5: taking towns: sack, demand, court (lane A, for MP2)

Status: built and gated on `claude/m4a-take` (from `d8ac0ab`, MP1). Plan:
[M4A_PLAN.md](../M4A_PLAN.md) step 5. Design: [LORDSHIP_DESIGN.md](../LORDSHIP_DESIGN.md) sections
5.1 to 5.3 and 9; it wins over the plan. Decisions: [ADR 0020](../adr/0020-lordship.md) "As built",
step 5. Requirements: GP-32, UI-11 (and the Bond and Awe terms of SIM-60).

## 1. What was built

**The sim** (all in `sim_military`, run only, serial, SpawnId order):
- **The sack** (`orders::sack`, called from `attack` at the hold's completion): a player captain at
  Aggressive in a run plunders the storehouse's food (keeping nothing) and its weapons and tools
  into the baggage, posts `Deed::Plunder`, logs "sacked", then `transfer(.., Cause::Sack)`. The
  rival's always-Aggressive bands still take by `Cause::Conquest`.
- **Demand** (`OrderSpec::Demand`; `validate_demand`, `orders::demand`): the player's, in a run, at
  an independent or rival town other than the seat, not during a cooldown. March within
  `recruit.radius`, parley `demand.parley_hours` (an army at war with you within `threat.radius`,
  never a guard camp, breaks it off), then `tenure::parley`: kneel (`transfer_seeded`,
  `Cause::Submission`, seed 0.45, `tenure::hold_tribute`) or refuse (`Deed::Threat`,
  `Tenure.cool_until`, "X refuses Your warband: 176 would fight; you need 66 within 40 tiles and
  have 5").
- **Tribute**: `hold_tribute` writes lane B's `Edicts` (Dues Tithe, pending cleared, the Dues dial
  locked until it settles); no edict effect is implemented here. The Dues term is 0 while it pays.
- **The watch** (`tenure::watch`, `keep_watch` in slot 14): strength within 80 against 0.75 x
  0.25 x able adults; a garrisoned town of yours within 80; your other towns' influence share at
  least 0.5 (the town's own power left out). Hours counted; day judged on the day tick (not the day
  it knelt in); defiance after two weak days (`Cause::Defiance`, its militia provoked against your
  soldiers there for `garrison.revolt_hours`, the Dues dial let go); settling after 10 days, logged
  once.
- **Awe** (+0.1 while paying tribute and watched this hour) and **Bond** (`tenure::bond_term`) are
  written in `tenure::rebuild_terms`.
- **Court**: `tenure::gift` from the `Give` arm (`Deed::Gift`, once a town a day); `Deed::Defended`
  from the hour's battle reports at the start of `tenure::step`; `requests::complete` of a
  drive-off calls `tenure::drove_off` (the day's defence spent, a promise kept at a sworn town);
  `requests::fail` of a drive-off calls `tenure::forsaken` (`Deed::Forsaken`, `Tenure.broken`, a
  `Forsaken` event, the Bond turned at two).
- **Bandits are never "defence"**: `combat::casualties` writes `Tenure.kin_fell` at the home of a
  fallen man of a `Bandits` band, and a battle that saw one fall (or met a living band of that
  home) posts no `Defended` there.
- `conquest::would_be_militia` is `pub(crate)` and reads `&World`; `invitations::sworn_terms` gives
  the card its words.

**The UI** (`game_ui`, by the UI agent): `panels/take.rs` (a `TakeView` refreshed by one exclusive
system from `parley`, `watch` and `sworn_terms`; "Ways to take it", the watch line, the bond and
"You forsook us (day 9, day 13)"); the right-click menu's "Demand submission (need N within 40,
have M)" (greyed with its reason) and "Gift food (+0.08 goodwill)"; the invitation card's "Sworn:
loyal (starts at 0.75); they will expect you to defend them"; tenure marks on labels ("[sword]",
"[broken sword]", "[bent knee, watched]", "[ribbon]", "[eye]"); the feed's "Gorsefield stays loyal:
awe of your army, +0.10" once a town a day. Unit tests in `take.rs`, `feed.rs`, `labels.rs`,
`cards.rs` (TESTING.md).

**The bot** (`sim_headless::bot`, by the bot agent): `take_mode` picks Demand (projected strength
at least 1.1 x need), then Court (gift an independent town with goodwill 0.35 to 0.62 and food to
spare), then Attack; it keeps its targets within 70 tiles of a fresh unwatched submission for two
days; `Takes` counts takes by way, printed by `bot_run`.

**Tests**: `tests/tests/take_modes.rs`, nine scenarios (TESTING.md).

## 2. Deviations from the plan and the design, and why

1. **New `Tenure` fields** `gift_day`, `defended_day` (day + 1) and `kin_fell` (tick + 1): "once a
   town a day" and "never against its own bandits" need state; a band wiped out to the last man
   leaves no `Bandits` component to read. Amended in place under `SAVE_FORMAT` 2.
2. **An answered drive-off spends the day's defence** rather than "this hour": no new state, and the
   town already has the request's +0.2.
3. **The day a town knelt in is not judged** for weak days (a town taken at dusk would otherwise
   defy a day early).
4. **The watch's defence** counts the able adults only (no garrison term: you are its garrison).
5. **A gift's cap applies before the hungry doubling** (the design's order), so a hungry town's
   gift reaches 0.24 at most.
6. **Guard camps don't break a parley** (they never leave their post; otherwise towns near a
   monument could never be demanded).
7. **Subvert is omitted from "Ways to take it"** (step 7).
8. **`sample_orders` does not issue `Demand`**: the bot's runs (and `a_bot_run_replays`) carry it;
   adding it to the random orders would change every recorded command stream.

## 3. Names later steps must use

- `sim_military::tenure::{Parley, parley, WatchView, watch, strength_near, defence,
  soldier_strength, settles_at, paying_tribute, bond_term, awe_term}`; `pub(crate) hold_tribute,
  gift, drove_off, forsaken`.
- `Tenure.{gift_day, defended_day, kin_fell}`.
- `invitations::{sworn_terms, SwornTerms}`.
- `bot::Takes` and `BotRun.takes`.
- **Step 6** writes `Bandits` on spawned bands; `kin_fell` already works for them. Its own
  `Killing` deed at a bandit's home belongs in `combat::casualties` beside `kin_fell`.

## 4. Checks and results

Every cargo command with `-j 2` (one base check with `-j 3`) and `CARGO_INCREMENTAL=0`, tests with
`--test-threads=2`.

| Check | Result |
|---|---|
| `cargo fmt --all --check` | clean |
| `cargo clippy -j 2 --workspace --all-targets -- -D warnings` | clean |
| `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps` | clean |
| Unit tests: sim_military, sim_core, sim_agents, sim_headless, game_ui | all pass (sim_military 97 + 1 ignored; game_ui 55; sim_core 39; sim_agents 29; sim_headless 5) |
| `take_modes` | 9/9 |
| Re-run: `conquest`, `invitation`, `gather`, `tenure`, `influence`, `run_start`, `chronicle`, `dominion`, `director`, `monuments`, `rival`, `sellswords`, `wanderers`, `save_load`, `docs`, `orders`, `village_day`, `famine`, `posture`, `army_march`, `starving_army` | all pass except three failures that **reproduce identically at the base `d8ac0ab`** (checked in a worktree of the base): `conquest::a_plundered_village_loses_faith` (loyalty 0.318 on day 2 against the 0.35 line, the thin margin STEP1 warned of, at the base too), `director::a_band_with_nothing_to_do_lingers_then_withdraws` (panics in `common::spawn_id`, "sim entity") and `champion::soldiers_who_fought_rank_up_with_the_configured_gains` (the director's band reaches the reserve, so no unranked soldier is seen) |
| Determinism (`tests/determinism.rs`) | 13/13, including `a_bot_run_replays` (whose bot now demands) |
| Run replay | `--run --seed 42 --ticks 3000 --commands 50 --record run.ron`, then `--replay` and `--replay --single-threaded`: both `replay ok: seed=42 ticks=3000 commands=51 checkpoints=3` |
| Stdout invariance (release, `cmp` against `docs/m4a/baseline/*.out`) | developed (21 lines), skirmish (26), battle (42): **byte-identical** |
| `bot_run` (release, `--ignored`) | **5 of 5 won**, quiet limit held, 170 s; but every win before day 14 (below) |
| Cost (NFR-11) | not run: a merge-point check (LANE_RULES). A botless run owns nothing, so step 5 adds one battle-report scan an hour and no watch |

**`bot_run` per seed** (the line now ends with the bot's takes by way):

| Seed | Won on day | Taken / lost / held | Attack (the seat) | Sack | Submitted | Invited (gifted) | Defied | Demands / refused | Gifts |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 8.2 | 3 / 1 / 2 | 1 | 0 | 2 | 0 (0) | 1 | 2 / 0 | 0 |
| 2 | 5.8 | 2 / 1 / 1 | 1 | 0 | 1 | 0 (0) | 1 | 1 / 0 | 0 |
| 3 | 6.8 | 3 / 2 / 1 | 1 | 0 | 2 | 0 (0) | 2 | 2 / 0 | 0 |
| 4 | 12.0 | 4 / 0 / 4 | 1 | 0 | 3 | 0 (0) | 0 | 3 / 0 | 0 |
| 5 | 11.5 | 5 / 2 / 3 | 1 | 0 | 4 | 0 (0) | 2 | 4 / 0 | 0 |

Median win day 8.2, against the M4 baseline's 18.4 (17.4, 19.3, 26.3, 18.4, 15.5). The bot agent's
run of the **old** bot on this branch's sim (before its changes): days 16.9, 23.0, 27.9 and 17.1,
seed 3 lost on day 35.8.

## 5. Open issues

1. **The win-day floor is broken (balance, for MP2).** The bot now takes two to five towns by
   Demand and then assaults the seat with an army that never bled, winning on days 5.8 to 12.0. The
   old bot took 11 to 19 towns by Attack, losing men at each, and won around day 18. The plan's
   floor (no win before day 14, median within four days of 18.4) fails on every seed. It is the
   merge-point gate, and step 6 (the seat follows its realm) and lane B's edicts (tithes, levies)
   move the same numbers, so I changed no data. Candidates, per HOWTO "Tune run balance": a seat
   that needs more than an unbled army (the clock's defence, `rival` rows), a stiffer demand
   (`demand.ratio`, or unarmed men at less than 1), or a bot that won't assault before it holds
   more land (the bot's `assault_margin`). Your call.
2. **The garrisoned-neighbour watch cannot happen with the shipped `watch_radius` 80.** The run
   island's towns stand at least about 100 tiles apart (seed 42: nearest neighbours 101 to 152
   tiles), so the design's "a garrisoned neighbour ... does as well as an army parked beside it"
   never applies, and step 5's done-when ("a submitted town stays kneeling while a garrisoned
   neighbour watches it") holds only with a wider radius. `a_garrisoned_neighbour_or_a_sphere_keeps_it_watched`
   sets 125. Proposed: `demand.watch_radius: 125.0` (it also widens the strength check), or a
   separate garrison radius. Not changed: it is a design number.
3. **Courting is untested in a real run.** The bot's Court path never fired (Demand qualifies
   first), so `bot_uses_lordship`'s "at least one gift-courted town" would fail today.
4. **Three failures at the base** (section 4) belong to MP1's merged state, not to this lane.
5. **No panel screenshots** (no GPU here): the UI is covered by unit tests of its words only.
6. **`sample_orders` does not issue `Demand`** (deviation 8).
