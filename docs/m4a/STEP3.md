# M4a step 3: influence and the map (lane A, for MP1)

Status: built and gated on `claude/brave-turing-sh1ryn` (WIP checkpoints `7536741` and `dfe1aca`, plus
the uncommitted review pass below). The merge lead has not committed it yet. Plan:
[M4A_PLAN.md](../M4A_PLAN.md) step 3. Design: [LORDSHIP_DESIGN.md](../LORDSHIP_DESIGN.md) section 6;
it wins over the plan. Decisions: [ADR 0020](../adr/0020-lordship.md) "As built", step 3.
Requirements: GP-31 and UI-12, plus design question 16 (the small land-and-people reading of
"renown-like power").

## 1. What was built

**The sim.** All of it is in `sim_military`, run only, serial and in SpawnId order. Nothing new is
hashed or saved beyond step 1's types, except `Influence.envoy`, amended in place under
`SAVE_FORMAT` 2.
- `influence.rs` has three pure functions:
  - `power(data, people, loyalty, scar, seat)`: people and loyalty only, less the scar, times
    `seat_power` at the rival's seat.
  - `pull(data, power, distance)`: linear down to 0 at `reach`.
  - `field_at(data, sources, at) -> Shares`: takes any iterator of `Source`, so step 5's watch can
    leave a town out by filtering.
- `influence.rs` also has these helpers: `Source::of`, `Shares { player, rival }` with `net()` and
  `of(faction)`, `contest_value`, `warmth`, `in_sphere` and `realm_units`.
- **Slot 13, `influence::step`.** Every hour it refreshes every run town's `Influence`. On the day
  tick it then:
  - posts a no-spread `Deed::Influence` at each town the player doesn't own, the seat excepted:
    `0.12 x (net - 0.1)`, never past the 0.6 cap;
  - flags the envoys: at most `max_envoys_per_day`, the highest net first, ties by SpawnId;
  - credits the player's champion `rank_points` per 500 people held, through the new
    `champion::credit_times(world, faction, Feat::Realm, n)`. `credit` now calls it with 1.
- **Slot 7, `invitations::step`.** A flagged independent town posts its invitation in the hours
  after the day tick when no captain is near ("{name} sends envoys: it invites {who} to rule it").
  Posting spends the flag. On the day tick itself slot 7 reads no flag (see the review pass).
- **Slot 14, the Contest term** (`influence::contest_term`, summed by `tenure::rebuild_terms`):
  - `-0.25 x` the other side's lead, at either side's towns;
  - `+0.05` heartland at the player's towns where the player leads by at least `sphere`;
  - 0 at the seat, at free towns and with influence off.
- **A taking refreshes the field at once.** `tenure::retake` calls `influence::refresh_field`
  before `rebuild_terms`, so a conquered town's own power stops counting for its old owner in the
  same tick.
- **The drifters' ease.** `drifters::draw` lowers the goodwill line by `drifter_ease` where the
  net is at least `influence.sphere`.
- **`influence.enabled: false`** turns all of it off: the field, the deeds, the envoys, the Contest
  term, the rank credit and the ease.
- `sim_agents::standing` needed no change. `spread_totals` and the deed log already skipped deeds
  that don't spread (step 1).

**The map and the HUD.**
- **The overlay.** `OverlayMode::Influence` ends the O cycle (`crates/game_render/src/overlay.rs`).
  - While the mode is shown, `overlay/influence.rs` rebuilds an `InfluenceView` from the towns'
    own `Influence` every two seconds. It is a grid of nodes 8 tiles apart, using the same
    `field_at`, with shares quantised to 1/64 and blended bilinearly in `tint`.
  - The tint runs from your colour through an amber front (where |net| is under `front`) to the
    rival's.
  - The dashed sphere edges are the net +/-0.5 contours, found by marching squares.
  - Rings go round independent non-seat towns with net of at least `warming`. They fill with
    goodwill over the invitation line and turn gold on the day the town sends envoys.
  - `Overlay.generation` moves only when the tint changes, so terrain is not rebuilt just because
    a ring fills.
  - `influence_marks` draws the dashes and rings with gizmos.
- **The run bar** reads "Sphere N, warming M" (`run_bar.rs`, `sphere_counts`), with a short
  legend while the overlay mode is on.
- **The settlement panel** has an influence line under the tenure line (`panels/why.rs`,
  `influence_lines`). A positive Contest term reads "heartland".

**Tests and docs.**
- `tests/tests/influence.rs`: six scenarios and one ignored ten-day run.
- Unit tests in `influence.rs` (14), `drifters.rs`, `overlay.rs`, `overlay/influence.rs`,
  `run_bar.rs` and `panels/why.rs`.
- Docs:
  - DATA.md: the `influence` block.
  - ARCHITECTURE: slot 13, slot 7's branch and the overlay systems.
  - GLOSSARY: Envoys, Front, Heartland, Influence, Sphere, Warming, Overlay and Run bar.
  - TESTING, and ADR 0020 "As built".
  - HOWTO: "Add a loyalty term".
  - README and PLAY_GUIDE: the overlay and the run-bar line.

## 2. The review pass (this pass)

| Finding | Verdict | What was done |
|---|---|---|
| should-fix: the heartland +0.05 is given at the rival's towns too | **Holds, fixed.** The design's table says "your heartland +0.05". With the seat's power, the bonus propped up every rival town deep in the rival's sphere from tick 0. On seed 42 that was 6 towns, now shown by `the_rival_contests_the_border`. It would also have undercut design 5.4's subversion arithmetic (a fed rival town crossing 0.5 on day 6). | `contest_value(data, owner, own, other)` now takes the owner and gives the heartland only when the owner is the player. The negative contest still applies at both sides' towns. Unit test `the_contest_follows_the_lead` covers both owners. `a_held_town_warms_its_neighbours` asserts no positive term at a rival town. The integration test `the_rival_contests_the_border` asserts that the rival's deep towns carry 0. Also updated: ADR 0020, the DATA and GLOSSARY Heartland rows, the `InfluenceData.heartland` doc and the `tenure` doc. |
| should-fix: the run bar's "warming M" counts your own towns and the rival's | **Holds, fixed.** A fresh, restless holding of yours at net 0.3 was counted as "warming to you", though no ring or panel line says so. | `sphere_counts` now takes `(net, can_warm)`. `can_warm` means not the player's and not the rival's seat, which is where the day's `Deed::Influence` goes, so rival towns that warm to you count. "Sphere N" is unchanged. `the_sphere_counts_towns_by_net` adds an owned town at net 0.3 that must not count. Also updated: the DATA warming row, the GLOSSARY Run bar and Warming rows, and the ADR 0020 HUD bullet. The rings still show free towns only, since only those can send envoys. |
| should-fix: the ten-day done-when test's ignore reason says "release in CI", but CI never runs it | **Holds, fixed by rewording.** CI wiring is the merge lead's file. | The reason now reads "ten game days; run with --ignored at merge points (not in CI)". TESTING says the same. It belongs in MP1's gate (see open issues). |
| nit: a stale envoy flag can post at the next dawn tick, making two captain-free invitations in one day | **Holds, fixed.** Slot 7 runs before slot 13 on the day tick. A town flagged yesterday whose goodwill stayed under the line all day, and rose just before dawn, could post at dawn. The new day's flagged town would then post an hour later. | `invitations::step` reads no envoy flag on the day tick: `envoy: enabled && !cal.on_day(tick) && flag`. New unit test `a_stale_envoy_flag_sends_no_one_at_dawn`: no post at the dawn tick; the same flag posts off the day tick and is spent. Normal timing is unchanged, because a dawn flag's warmth only lands on the next tick anyway. ADR 0020 and ARCHITECTURE's slot-7 text now say so. |
| nit: the unit test `the_seat_is_never_swayed` would pass with the seat exclusions deleted | **Holds, fixed.** | The test now sets `seat_power` 0, holds every other town within reach of the seat, and asserts that the formulas would sway it (net 0.99 at least `envoy_net`, warmth > 0, contest < 0). Then it checks there is no deed and no Contest term. Phase 2 lets the seat go free by a revolt; no rule prevents that, and it does not end the run. It then checks there is no deed and no envoy flag. Mutation check: removing the warm exclusion, the contest exclusion, the envoy exclusion or the dawn rule each fails its test; all were restored. |

## 3. Deviations from the plan and the design, and why

1. **`Influence` gained `envoy: bool`.** No field for the flag existed, and slot 7 runs before
   slot 13, so the flag has to live in state. The type is amended in place under `SAVE_FORMAT` 2,
   and the `save_load` literal is updated. No format was bumped; that is the merge lead's call.
2. **`Influence` has two writers besides slot 13.**
   - `tenure::retake`, which refreshes the field at a taking.
   - `invitations::step`, which spends an envoy flag.
3. **The envoy goodwill check counts that dawn's own warmth.** The deed is applied on the next
   tick, before slot 7 looks. Without this, the design's "about eight days" becomes nine.
4. **The heartland** is a town of the player's where the player's share leads the rival's by at
   least `sphere`. The design names the bonus but doesn't define the lead. This pass narrowed it
   to the player's towns (section 2).
5. **The rank credit is in slot 13** (the plan), not in `dominion::tally` (the design's list), and
   is for the player only. `Feat::Realm`'s points come from `influence.rank_points`;
   `ChampionData::points_for(Realm)` returns 0. `credit_times` is new.
6. **One step-1 unit test opts out of influence.**
   `tenure::tests::scars_fade_and_write_the_resentment_term` sets `influence.enabled: false`,
   because it asserted bias == -scar.
7. **The front is amber, not purple.** army.ron's rival is already purple (0.55, 0.25, 0.7), so a
   purple front would not read as a third thing.
8. **The rival's sphere edge (net -0.5) is drawn too**, in the rival's colour.
9. **"Sphere N" counts every town at net >= `sphere`, yours included.** "Warming M" counts towns
   not yours and not the seat in [`warming`, `sphere`). The design's example "Sphere 7, warming 3"
   doesn't say which.
10. **Test scenarios that set data.**
    - `envoys_come_without_a_captain` and the drifter case shrink `influence.independence`.
    - `the_seat_is_never_swayed` also sets `seat_power` to 0.
    - These put towns deep in the sphere within a few ticks. The shipped tuning is carried by the
      ignored ten-day test.
11. **The ten-day done-when is an ignored test.** A band without the bot starves, and its fall
    ends the run, so the test tops up the champion's baggage each day.

## 4. Names later steps must use

- **Sim:** `sim_military::influence::{Influence { power, player, rival, envoy }, Influence::net,
  Influence::shares, Source, Source::of, Shares, field_at, power, pull, warmth, in_sphere,
  contest_value(data, owner, own, other), contest_term, realm_units, InfluenceData}`; also
  `champion::credit_times` and `Feat::Realm`.
- **The overlay and run-bar lines:** `InfluenceData.warming` (0.2), `sphere` (0.5) and `front`
  (0.1).
- **Step 5's Demand watch** counts "your other towns' influence share" with `field_at` over
  `Source`s, with that town filtered out. The overlay's `TownView` list has the same shape.
- **Envoy invitations.**
  - An invitation by envoys is the ordinary `OfferKind::Invitation` with `Cause::Invitation`.
  - The UI can tell it by there being no captain within 40 tiles; the flag is already spent once
    the offer is posted.
  - The log line contains "sends envoys".
- **UI:** `game_render::overlay::influence::{InfluenceView, TownView, Palette, Dash, Camp, Ring,
  RingState, CELL}`; `Overlay::influence()`; `OverlayMode::{Influence, legend}`;
  `game_ui::panels::why::{InfluenceFacts, influence_lines}` (crate-private). `Why::term_name(term,
  value)` and `pull_name(pull, value)` take the value.
- **Unit tests:**
  - influence.rs: `power_is_people_and_loyalty_only`, `the_pull_falls_off_with_distance`,
    `field_at_shares_the_pull`, `a_source_is_a_held_town_with_power`,
    `the_contest_follows_the_lead`, `warmth_stops_at_the_cap`, `the_realm_counts_whole_units`,
    `influence_round_trips`, `a_held_town_warms_its_neighbours`, `envoys_come_without_a_captain`,
    `envoys_count_the_dawns_warmth`, `a_stale_envoy_flag_sends_no_one_at_dawn`,
    `the_seat_is_never_swayed`, `a_realm_ranks_the_champion`.
  - drifters.rs: `a_sphere_eases_the_drifters_line`.
- **Integration tests** (`tests/tests/influence.rs`): `a_held_town_warms_its_neighbour`,
  `the_rival_contests_the_border`, `envoys_come_without_a_captain`,
  `a_free_town_between_two_of_yours_invites_within_ten_days` (ignored),
  `influence_deeds_do_not_spread`, `the_seat_is_never_swayed`,
  `a_realm_ranks_the_champion_and_draws_drifters`.

## 5. Checks and results (the lane gate, after the review pass)

All cargo runs used `-j 2` and `CARGO_INCREMENTAL=0`, and tests ran with `--test-threads=2`.
Disk stayed at 8.1 GB free.

| Check | Result |
|---|---|
| `cargo fmt --all --check` | clean |
| `cargo clippy -j 2 --workspace --all-targets -- -D warnings` | clean |
| `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps` | clean |
| Unit tests: sim_military / game_ui / game_render | 97 pass, 1 ignored (not this step's) / 37 pass / 31 pass |
| `tests/influence.rs` | 6 pass, 1 ignored. `the_rival_contests_the_border`: the rival's town at Contest -0.060; your heartland +0.05 at #11; loyalty mean 0.622 against 0.665 with influence off; your border town at -0.042; 6 rival towns deep in their own sphere at 0 |
| Ignored ten-day test (`--ignored`, test profile, 98 s) | passes. Seed 1: #15 invited on day 7 (net 0.703, goodwill 0.503). Seed 42: #16 on day 8 (net 0.675, goodwill 0.545). No captain within 40 tiles |
| Determinism | 13/13 (217 s) |
| invitation, tenure, conquest, run_start, save_load, docs | 2/2, 8/8, 4/4, 4/4, 5/5, 4/4 |
| Stdout invariance (release, against `docs/m4a/baseline/*.out`): developed, skirmish, battle | all byte-identical (`cmp` silent) |
| Release replay: `--run --seed 42 --ticks 3000 --commands 50 --record`, then `--replay` and `--replay --single-threaded` | "replay ok ... checkpoints=3" both ways, `state_hash=0x2dbe8d34a3659269` (the implementer's run read 0xc5c956bf54ffb2a8; the rival's towns no longer get the heartland) |
| Bot smoke run: `--run --bot --seed 1 --ticks 24000 --chronicle` (release) | exit 0, no panic. 11 taken, 0 lost, 11 held (967 people); 2 monuments taken, 1 held; 18 battles. No invitation of any kind in the chronicle |

The other run-preset tests were run earlier, by the implementer and the UI pass, before the
heartland change; no `influence.enabled: false` opt-out was needed then. They are rival 2/2,
champion 3/3, chronicle 1/1, dominion 2/2, director 3/3, monuments 2/2, gather 2/2, sellswords 6/6
and wanderers 2/2. They are not re-run here; MP1's full suite covers them.

## 6. Did seed 1 get an envoy invitation within ten days?

**On the shipped tuning, yes; in the bot's run, no.**
- **Scripted (the done-when).** Two free towns are handed over at loyalty 0.8, with strangers at
  the start. The free town between them (#15) sends envoys on **day 7**, with no captain within 40
  tiles. Seed 42's #16 does so on day 8.
- **The bot.** The seed-1 bot smoke run saw **no envoys and no invitation at all** in ten days.
  The bot takes its neighbours by force first (all 11 of its town takings in the chronicle), so no free
  town is left between two quiet holdings long enough to warm the roughly eight days it needs.
- **What the probe measured.** The implementer's probe, since removed, found that with only a
  town's two nearest free neighbours held at loyalty 0.5:
  - 5 of 19 free towns reach net >= 0.5 on seed 1, and 11 of 21 on seed 7;
  - at loyalty 0.7 the counts are 10 and 12.
- **Where it goes next.** Envoys in bot play are step 10's tuning: influence's pace, and the bot
  choosing to court rather than conquer. `bot_uses_lordship` deliberately does not count them.

## 7. Open issues

- **`bot_run` must run at MP1.** Influence changes how a run plays: the rival's pull at your
  border, the heartland at your towns, envoys, faster drifters and a small rank credit. The
  seed-1 smoke run lost 0 towns, against step 1's 1.
- **Run the ignored ten-day test at MP1** (`cargo test --release -p integration_tests --test
  influence -- --ignored`). Wiring it into `.github/workflows/ci.yml` next to `bot_run` is the
  merge lead's call. It costs about 100 s in the test profile and less in release.
- **No screenshots.** No GPU session ran, so nothing has been looked at:
  - the overlay's colours, dash length, ring size and gizmo lift;
  - the panel's extra two or three lines. The panel is 380 px wide at 44 characters, and the
    section is now up to about 25 lines on a bottom-anchored panel.

  The plan's frame gate for render steps (lavapipe under Xvfb) and SEED's UI-12 criterion still
  owe screenshots.
- **Overlay cost** is unmeasured on real frames: 129 x 129 nodes, every two seconds. If it
  hitches, widen `CELL` to 16.
- **SEED criterion** "Held towns radiate influence ..." stays unchecked: its levies and tithes come
  with step 4.
- **SAVE_FORMAT.** `Influence.envoy` is amended in place, and the HOWTO's "Add a loyalty term"
  notes that a new `LoyaltyTerm` changes the saved array length. Both are the merge lead's call.
- **Bands run without the bot end early.** On seed 42 the champion starves before day 5 and the
  run ends. This matters if a cost check relies on long windows without the bot.
