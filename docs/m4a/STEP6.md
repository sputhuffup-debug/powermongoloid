# M4a step 6: fallout: bandits and reprisals (lane A, for MP3)

Status: built and gated on `claude/m4a-take` (from `8182929`, MP2). Plan:
[M4A_PLAN.md](../M4A_PLAN.md) step 6. Design: [LORDSHIP_DESIGN.md](../LORDSHIP_DESIGN.md) sections
6.1 and 7; it wins over the plan. Decisions: [ADR 0020](../adr/0020-lordship.md) "As built",
step 6. Requirements: GP-33 (fallout), GP-31 (the seat follows its realm). Part 1 ("Take and
hold") ends here.

## 1. What was built

**The sim** (`sim_military`, run only, serial, SpawnId order):
- **The queue** (`fallout::queue_taking`, called by `conquest::transfer_seeded`). `tenure::record_taking`
  now returns `(TakenBy, blood)`, the blood the taker spilt read with `Tenure::blood_of` *before*
  the taking resets it. The rival's influence share is read before the taking refreshes the field.
  For a player's take only:
  - a conquest or sack of a town of at least `min_people` queues `PendingBand { men: band_size(..),
    due: +delay_hours }` and stamps `Fallout.last_violent` (the haste multiplier's clock);
  - a take of any kind from the rival, or where the rival's share was at least `fallout.shadow`,
    queues a `PendingVow { settlement, cause }`.
  Nothing spawns there.
- **Slot 16, `fallout::step`** (hourly):
  - due bands rise (`fallout::raise`): the town's first `men` recruit candidates (idle first); the
    first becomes the captain (RAIDERS, aggressive, unarmed, `Raider` and `Bandits { home, raids }`),
    the rest follow with `enlist_into`, so the population drops by the band; `food_days` of food;
    `BanditsRose` (the taker's, `Cause::Fallout`). Fewer than `min_band` left: no band.
  - at the cap (`max_bands` captains with `Bandits`) a due band waits and its town broods once
    (`PendingBand.brooding`; scar `+brood_scar`, at most `tenure.max_sack_scar`, terms rebuilt).
  - a band whose town is no longer the player's never rises.
  - vows: `RivalState.reprisal = Vow { settlement, due: +vow_delay_hours }` and `last_reprisal`, unless
    one came within `vow_cooldown_days` or the town isn't the player's; `ReprisalVowed` (amount: the
    hours; cause: the take's).
- **Bandits' minds** (`raiders::think`, run-only branch for `Bandits`): each think, the populated town
  not the seat, not home, not the last raided, with the best `fallout::bandit_score` (`weight(owner) /
  ((1 + d/150)(1 + garrison/5)(1 + your share))`, the garrison counted from the fighter grid), raided
  with an aggressive `GatherFood`; `Bandits.raids` counts each raid issued. They never hunt armies.
- **The director** counts `With<Raider>, Without<Bandits>` toward `max_bands`.
- **Heirs** (`combat::succeed`): in a run `Raider` and `Bandits` pass to the heir (outside a run an
  heir of raiders still loses its AI, as in M3: the skirmish and battle outputs are unchanged).
- **Their dead are remembered at home** (`combat::casualties`): a bandit killed by a player's blow
  (`Casualties.slain_by`) posts `Deed::Killing` toward the player at his home, beside step 5's
  `kin_fell` (so no `Defended`).
- **Reprisals** (`rival::clock`, slot 11): a vow due, from `reprisal_from_day`, with at least
  `reprisal_min_garrison` followers in the garrison and a town of the player's: `reprisal_size x
  strength` people (x `turned_mult` against a town turned within `subvert.fragile_days`; at most half
  the garrison) split off like a raid (`rival::split_band`, the old `raid` body, factored; stream
  `sim_military::rival::reprisal`), `RivalBand.pinned` = `rival::newest_holding` (latest
  `Owner.since`, ties to the lowest SpawnId), counted in `RivalState.raids`, the vow spent, `next_raid`
  pushed back half a period, `RivalReprisal` with `Cause::Reprisal`. `rival::think` honours `pinned`
  while the player holds it, then drops it. A retake is the rival's ordinary `Attack`, so
  `Cause::Conquest`, `TakenBy::Conquered`, no plunder.
- **The seat follows its realm** (`rival::realm`, `Realm`, `realm_factor`, `seat_bonus`):
  `defence_bonus = defence_on(day) x strength x factor`, the factor exactly 1.0 at a full realm
  (a branch, not arithmetic, so it is bit-exact), `realm_floor + (1 - realm_floor) x share` below.

**The UI** (`game_ui`, by the UI agent; new `fallout.rs` with a `FalloutView` refreshed twice a
second): the vow's feed line with its hour ("The rival vowed to retake Fenstead by day 7, 18:00
(Fenstead was taken by the sword)"); in the run bar the realm line ("The rival's realm: 4 of 9 towns,
450 people (45%) · seat defence +0.50 (its clock says +0.75)", the clock clause dropped when equal),
the vow line ("Vow: the rival will retake Fenstead by day 7, 18:00") and "Broken men abroad: N"; the
vowed town's label mark "[vow: day 7, 18:00]"; the realm line on the seat's panel; the end screen's
timeline keeps ReprisalVowed and RivalReprisal. director.ron `auto_pause` gains ReprisalVowed and
Defied on steady and chaotic.

**The bot** (`sim_headless::bot`, by the bot agent): the reprisal guard (while a vow stands, the
secondary captain nearest the newest holding garrisons it and recruits there up to the expected band
x 1.5); the bandit filter (bands raiding a rival town more than 60 tiles off are no foes for a
conquest or demand); `Takes` counts bands, their raids by owner, vows, reprisals, retakes and revolts.

**Tests**: `tests/tests/fallout.rs` (7), `tests/tests/rival.rs` (+4), unit tests in `fallout.rs`,
`rival.rs` and `game_ui::fallout` (TESTING.md).

## 2. Deviations from the plan and the design, and why

1. **The band's captain is one of the town's own men,** not a spawned stranger (`spawn_army`): the
   task and the design's "they are its own people" ask that the population drop by the band; a
   spawned captain would be a man from nowhere. No RNG is drawn and nothing spawns.
2. **They gather at the town's edge** (`threat.radius` + 10 tiles from the centre, toward the rival's
   seat, turned aside like a withdrawing band). Rising where they stood, inside the town, the first
   runs had the player's garrison cut a 16-man band down within the hour (seed 1, Sedgeholt), with
   Killing deeds at home and the town's own DriveOff request against its sons. The design's "at the
   town's edge" read literally.
3. **Bandits do not pause the game.** The task listed "vows/bandit risings"; the design (section 15,
   "Alarm fatigue": only three kinds pause) and the plan list ReprisalVowed and Defied (AgentCaught at
   step 7). A rising is the player's own doing, on the take they just made.
4. **New state, amended in place under `SAVE_FORMAT` 2:** `PendingBand.brooding` (a town broods once,
   not hourly) and `Fallout.vows: Vec<PendingVow>` (a take queues its vow; step 7's caught agent
   pushes one too). `Realm` is computed, not stored.
5. **A raid counts when it is ordered** (`Bandits.raids`), not when food moves: a raid interrupted
   still turns the band.
6. **A waiting vow stays pending** until it can launch (from `reprisal_from_day`, garrison of 8);
   the next vow (after the cooldown) replaces it.
7. **`edicts::a_levy_musters_and_joins`** opts out with `fallout.enabled: false`: its conquest now
   raises broken men two hours later, idle men first, which took the levy's waiting men (the path
   moved, the plan's rule).

## 3. The seat calibration (merge lead's decision)

At MP2 the bot made two to five towns kneel and stormed an untouched seat on days 6-12. The seat
needs to hold while its realm is intact and fall as the sphere war takes it apart. Data only: the
rival rows' `defence_steps` (now scaled by the realm share), measured with `sim_headless --run --bot
--seed N --ticks 96000` before the bot's guard:

| Steady `defence_steps` | Win days, seeds 1-5 | Note |
|---|---|---|
| MP2: (0, 0) (5, 0.25) (10, 0.5) (20, 0.75) (30, 1.0) | 8.2, 17.2, 7.1, 11.1, 7.3 | floor broken |
| (0, 0.75) (5, 1.0) (10, 1.25) (20, 1.875) (30, 2.5) | 19.4, 8.3, lost 25.2, 13.7, 17.2 | seed 2: weak rival (x0.8), 235 men by day 6 |
| (0, 1.25) (5, 1.5) (10, 1.75) (20, 2.25) (30, 2.5) | 19.9, 11.6, 20.0, 19.4, 18.6 | seed 2 still early |
| **shipped: (0, 1.5) (5, 2.0) (10, 2.25) (20, 2.5) (30, 2.75)** | 22.6, 33.2, 21.7, 19.5, 20.5 | 5/5, earliest 19.5, median 21.7 |

At the floor (0.4, the whole realm gone) the shipped curve is 0.6-1.1, about the old clock's; at a
full realm it is about 2.5-6x the old. Chaotic took the same steps; harsh `(0, 1.75) (4, 2.25) (8,
2.5) (16, 2.75) (24, 3.25)` (recorded, not gated). `reprisal_*` and `realm_floor` keep the design's
values.

## 4. Checks and results

Every cargo command with `-j 2` and `CARGO_INCREMENTAL=0`, tests with `--test-threads=2`.

| Check | Result |
|---|---|
| `cargo fmt --all --check` | clean |
| `cargo clippy -j 2 --workspace --all-targets -- -D warnings` | clean |
| `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps` | clean (after unlinking two private items) |
| Unit tests: sim_military, sim_core, sim_agents, sim_headless, game_ui | all pass (sim_military 100 + 1 ignored; game_ui 65; sim_core 39; sim_agents 30; sim_headless 8) |
| `fallout` (new), `rival` | 7/7, 6/6 |
| Re-run: `champion`, `chronicle`, `conquest`, `director`, `docs`, `dominion`, `edicts`, `gather`, `influence`, `invitation`, `monuments`, `orders`, `run_start`, `save_load`, `sellswords`, `take_modes`, `tenure`, `wanderers`, `village_day`, `famine`, `posture`, `army_march`, `starving_army` | all pass (`edicts::a_levy_musters_and_joins` with fallout off, deviation 7; step 5's three base failures now pass) |
| Determinism (`tests/determinism.rs`) | 13/13, including `a_bot_run_replays` |
| Run replay | `--run --seed 42 --ticks 3000 --commands 50 --record`, then `--replay` and `--replay --single-threaded`: both `replay ok: seed=42 ticks=3000 commands=51 checkpoints=3` |
| Stdout invariance (release, `cmp` against `docs/m4a/baseline/*.out`) | developed, skirmish, battle: **byte-identical** (after the calibration) |
| `bot_run` (release, `--ignored`) | **5 of 5 won**, quiet limit held (999 of 1,000), 357 s; earliest day 20.4, median **21.7** (floor 14, band 14.4..22.4): passes |
| Cost (NFR-11) | not run: a merge-point check (LANE_RULES). A botless run takes nothing, so fallout adds one realm sum an hour and an empty queue check |

**`bot_run` per seed** (release, `--ignored`, steady; takes are Attack / Sack / Submitted / Invited):

| Seed | Won on day | Taken / lost / held | Attack / Sack / Submitted / Invited | Defied | Bandit bands | Raids on rival / player / other | Vows | Reprisals | Retaken | Revolts |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 22.6 | 9 / 1 / 8 | 2 / 0 / 6 / 1 | 1 | 1 | 1 / 0 / 0 | 0 | 0 | 0 | 0 |
| 2 | 24.9 | 9 / 6 / 3 | 1 / 0 / 8 / 0 | 1 | 0 | 0 / 0 / 0 | 4 | 1 | 0 | 0 |
| 3 | 21.7 | 11 / 5 / 6 | 1 / 0 / 8 / 2 | 3 | 0 | 0 / 0 / 0 | 0 | 0 | 0 | 0 |
| 4 | 20.4 | 8 / 1 / 7 | 2 / 0 / 6 / 0 | 1 | 1 | 5 / 0 / 0 | 2 | 1 | 0 | 0 |
| 5 | 20.5 | 9 / 2 / 7 | 1 / 0 / 7 / 1 | 0 | 0 | 0 / 0 / 0 | 0 | 0 | 0 | 0 |

Median 21.7 against M4's 18.4 (MP2: 8.2). Raids are the bot's hourly sample of `Bandits.raids` by the
owner of the band's target (approximate). Seed 2's four vows launched once: the others were replaced
within the cooldown rules or still pending at the win. Its losses beyond the one defiance are not
retakes or revolts in the bot's counts (not investigated further; recorded for MP3).

## 5. Names later steps must use

- `fallout::{queue_taking (pub(crate)), band_size, bandit_score, bands_alive, PendingVow}`,
  `Fallout.vows` (step 7: a caught agent pushes `PendingVow { settlement, cause: Cause::Caught }`),
  `PendingBand.brooding`.
- `rival::{realm, Realm, realm_factor, seat_bonus, newest_holding}`; the reprisal's `turned_mult`
  already reads `TakenBy::Subverted` within `subvert.fragile_days` (step 7 only has to turn towns).
- `raiders::{bandit_target, BanditView}` (pub(crate)).
- `tenure::record_taking` returns `(TakenBy, blood)`.
- UI: `game_ui::fallout::FalloutView`; `vow_feed_text` patches the chronicle's " within N hours":
  reword `chronicle::describe`'s ReprisalVowed line and update it together.

## 6. Open issues

1. **Fallout is thinly exercised by the bot.** It takes most towns by Demand (no band, a vow only
   from the rival or its shadow): in the final `bot_run` one or two seeds raise a band and few
   reprisals launch, none ended in a retake, so the bot's guard is untested in anger. Step 10's
   `bot_uses_lordship` and Attack-only comparison will see more.
2. **Bandits still pick your towns early** when the rival's are far: a rival town 300 tiles off
   scores below an ungarrisoned town of yours 50 tiles off with a modest sphere. The design's
   numbers; the playtest judges it.
3. **The median win day (21.7) sits near the ceiling (22.4)**; step 7's agents and step 10's tuning
   should watch it.
4. **No screenshots** (no GPU here): the run bar, labels and seat panel are covered by unit tests of
   their words only.
5. **The part 1 playtest checklist** (the first half of `docs/M4A_PLAYTEST.md`) is the merge lead's at
   MP3, with the part 1 hand-over.
