# M4a step 1: foundations and the "why" (lane A, MP0)

Status: built on `claude/brave-turing-sh1ryn` (WIP checkpoint `870979a` plus the review fixes below),
gated, not yet committed by the merge lead. Plan: [M4A_PLAN.md](../M4A_PLAN.md) step 1. Design:
[LORDSHIP_DESIGN.md](../LORDSHIP_DESIGN.md) sections 4, 10 and 11. Decisions:
[ADR 0020](../adr/0020-lordship.md) "As built". Baselines: [baseline.md](baseline.md).
Requirements: GP-32, SIM-60, SIM-61, UI-11 (and the skeleton of GP-31, GP-33, GP-34, SIM-62 to
SIM-64).

## 1. What was built

**The skeleton, so lanes merge cleanly.** Every appended variant, type, data block and progress
slot of the milestone exists, hashed and saved (except `CommandKind`, see deviation 1), and is inert
until its step:

- sim_core: `CommandKind::Govern` with `GovernAct` and its payloads; `OrderSpec::Demand` and
  `Subvert`; `Cause` 15-23; `SimEventKind` 22-33 with `quiet()` and the explicit `notable()`.
  Validation refuses `Govern`, `Demand` and `Subvert` with "not yet".
- sim_agents: `Deed` 8-15 with `Deed::spreads()`, `DeedLog`, `Agent`, `LoyaltyTerm` and
  `LoyaltyTerms`, `edicts.rs` (`Edicts`, `DialState`, `Rationed`, `Mustered`, `Assignment`) and
  `Specialty`.
- sim_economy: `Construction`.
- sim_military:
  - New modules: `tenure.rs`, `influence.rs`, `govern.rs` and `fallout.rs`.
  - New types: `Bandits`, `Steward` and `Fallout`.
  - New fields: `RivalState.realm`, `reprisal` and `last_reprisal`, `Vow`, `RivalBand.pinned` and
    `Veteran.trade`.
- Data: progression.ron's ten new blocks, the five new `goodwill_deeds` rows, the rival rows'
  reprisal and realm fields, and the nine new `chronicle.causes` phrases.
- Schedule: progress slots 13 (`influence::step`), 14 (`tenure::step`), 15 (`govern::step`) and 16
  (`fallout::step`), appended, each with its span and run-gated.
- Formats: **`SAVE_FORMAT` 2 and `REPLAY_FORMAT` 6**, bumped once for the milestone. M4 saves and
  replays are refused with the existing messages.

**The cause-to-kind map** (`conquest::taking_kind`, design section 11):
- A sack is a `Conquest` and counts in `conquests`.
- A submission and a subversion are takes with their own kinds (`Submitted`, `Subverted`) and no
  `conquests` count.
- A defiance (`Defied`) reads like a revolt: the old owner is the faction, and there is no champion
  credit.
- The run summary counts `Submitted` and `Subverted` as takes and `Defied` as a loss.

**Tenure for real (SIM-60).**
- `transfer_seeded` writes `Tenure` from the cause through `tenure::record_taking`, with the scar
  from progression.ron `tenure` and the blood the taker spilt.
- `rival::setup` writes `Granted` and records the realm (the seat left out). `free_lands` writes
  `Freed`.
- `combat::casualties` counts each militia death in the town's `Tenure.blood`, for the faction the
  militia fought.
- A taking rebuilds the town's `LoyaltyTerms` at once. `tenure::step` rebuilds them every hour
  (Resentment and Stirred live; Contest, Dues, Table and Steward come from the other modules' stub
  functions, which return 0.0) and fades scars 0.02 on the day tick.

**The "why" for real (SIM-61, UI-11).**
- `loyalty_target` takes the terms' sum as a 7th argument, `bias`. A bias of exactly 0.0 returns the
  old value untouched.
- `drift_loyalty` reads `Option<&LoyaltyTerms>`.
- `apply_goodwill` writes `DeedLog`: merged per day, faction, deed and hearsay, eight entries at
  most.
- The settlement panel's "Why they feel this way" section is in `crates/game_ui/src/panels/why.rs`.
  It shows:
  - the tenure line;
  - the target term by term;
  - goodwill toward you and the rival, with the deed log newest first;
  - the revolt risk, with the deciding term and the two largest pulls (`sim_agents::loyalty::
    deciding_term` and `loyalty_pulls`).

**Tests.**
- `tests/tests/tenure.rs` has 8 scenarios (the plan's six, `every_cause_is_worded` and
  `another_factions_blood_is_not_the_takers`).
- `save_load` gains `every_lordship_type_round_trips`.
- sim_core's `a_format_1_save_is_refused` and `rejects_other_formats` pin both formats.
- There are unit tests in sim_core, sim_agents, sim_military and game_ui (docs/dev/TESTING.md lists
  them).

## 2. Review and sweep findings, and what was done

| Finding (severity) | Verdict | What changed |
|---|---|---|
| `Tenure.blood` counted every militia death whatever the killer, so a later conqueror's scar (and step 6's broken men) included raiders' or the rival's kills; the design says "militia killed in the taking" (should-fix) | **Fixed** | `Tenure.blood_by: FactionId` added (hash, save, `new`, `retake`). `Tenure::spill(by)` restarts the count when another faction kills. `combat::casualties` skips rebels (`against` equal to the villager's own allegiance). `tenure::retake` reads `Tenure::blood_of(to)`, which is 0 unless the blood is the taker's. Tests: unit `a_taking_reads_only_its_takers_blood`; scenario `another_factions_blood_is_not_the_takers` (a real raider fight, then a player conquest leaves the bare 0.12). DATA.md, GLOSSARY, ARCHITECTURE and the ADR were reworded |
| Terms lag a taking by up to an hour; a scar taken on a day tick fades in the same tick (nit) | **Fixed** | The per-town body of `step` is now `pub(crate) tenure::rebuild_terms`, called at the end of `tenure::retake`. The fade skips a town whose `since` is this tick. Test: `a_scar_is_not_faded_on_the_tick_of_its_taking`; `a_taking_reads_only_its_takers_blood` and the new scenario check the term in the taking's own tick |
| `RivalState.realm` included the seat, so the share could never reach 0 and `realm_floor` was never the floor (nit) | **Fixed** | `rival::setup` leaves the seat out; the field doc, DATA.md's `realm_floor` row and `setup_gives_every_town_its_lordship_state` say so |
| `LoyaltyTerm::Contest` always reads "rival's pull", wrong at a rival-held town (nit) | **Fixed** (UI only) | `Why::term_name` and `pull_name` in why.rs name it "your pull" at the rival's towns. The sim name is unchanged. Test: `the_contest_is_named_from_the_owners_side` |
| No save round trip for the types with no instance yet (nit) | **Fixed** | `save_load::every_lordship_type_round_trips`: one of each, plus non-default per-town state, a pending band and a vow. Saved and loaded, it gives the same hash, components, bytes and values, and the `Entity` fields resolve by SpawnId |
| No test pins `REPLAY_FORMAT` 6 (nit) | **Fixed** | `rejects_other_formats` asserts 6 and refuses format 5 with "replay format 5 is not supported (expected 6)" |
| The new `CausePhrases` are required while the M4a blocks are optional; the defaults test's comment said "an M4 progression text" (nit) | **Fixed by wording** | The phrases stay required, like every cause phrase, which DATA.md already demanded ("one phrase per Cause"). The test comment and DATA.md's `chronicle.causes` row now say so. Optional phrases would let a missing phrase print nothing |
| `CommandKind::Govern` has no hash or save arm (sweep should-fix; review nit) | **Recorded as a decision** | `CommandKind` is replay-only (RON, by variant name), never hashed or saved, as before. Its payloads that live on in state are hashed and saved by position. M4A_PLAN.md step 1 and ADR 0020's skeleton bullet now say so. No dead tag arm was added |
| CLAUDE.md and ADR 0016 list `CommandKind` among the saved enums (sweep nit) | **Not changed; flagged** | This is pre-existing wording. CLAUDE.md is the user's instruction file, which a lane agent does not edit, and ADR 0016 is an accepted record. ADR 0020 notes the inaccuracy. Up to you: drop `CommandKind` from CLAUDE.md's list |

## 3. Deviations from the plan, and why

1. **`CommandKind` is not hashed or saved** (see above). The plan's "tag, hashing and saving" holds
   for every other appended variant.
2. **Two duplicate design knobs are left out:** `tenure.defended` and `edicts.feast.deed`. They
   repeated `goodwill_deeds.defended` and `goodwill_deeds.festival`, which `GoodwillDeeds::delta`
   reads. Steps 4 and 5 use the `goodwill_deeds` rows.
3. **Pulled forward, inert:**
   - the `market` block (design section 10; the plan's list omitted it);
   - `Deed::spreads()` in the spread and the log (step 3);
   - `RivalState.realm` recorded at setup (step 6);
   - the Stirred term written as `-Tenure.stir` (0 until step 7).
4. **Registered and saved with no instance until their step:** `Specialty` (step 9), `Agent` (7),
   `Rationed`, `Mustered` and `Assignment` (4), `Construction` (8), `Bandits` (6) and `Steward` (9).
5. **Semantics chosen:**
   - Blood is per taker (above).
   - The bias does not apply to independent towns (`loyalty_target` still returns
     `independent_target`).
   - A new taking resets `blood`, `stir`, `kept`, `broken` and the watch, but keeps a refused
     demand's `cool_until`.
   - `Tribute.other` packs food (low 32 bits) and arms (high 32 bits).
6. **`tests/tests/conquest.rs`:**
   - `a_plundered_village_loses_faith`: the conquest's scar (about -0.16) made the plundered hamlet
     revolt on day 3. Per the plan ("the scenario's food changes, never the data") its homes now
     get a 0.3-day larder that the raid can't plunder. The margin is thin: 0.366 against the 0.35
     line on day 2.
   - `a_fed_village_under_a_passive_garrison_grows_loyal` also checks the taking's tenure.
7. **The revolt's chronicle words and the Dawn line** don't yet cite the deciding term or the two
   largest pulls. `SimEvent` has no field to carry them, and the Dawn line is step 4's feed. The
   rules (`deciding_term`, `loyalty_pulls`) are built and tested, and the panel uses them.
8. **The panel's code lives in `panels/why.rs`** (not inline in `panels.rs`) to keep the merge with
   lane B's workshop-stocks edit clean. It prints ASCII signs only, to match the panel font.
9. **The scenarios take towns from tick 1.** Since the fade guard, tick 0 would also work; real play
   can't take a town at tick 0.

## 4. Names later steps must use

- **sim_core** (`command.rs`, `events.rs`):
  - Commands and payloads:
    - `CommandKind::Govern { settlement: u64, act: GovernAct }`.
    - `GovernAct::{Work(WorkFocus), Dues(Dues), Table(Table), Build(BuildKind), Steward { follower: u64 }, Release(Dial)}`.
    - `WorkFocus::{Balanced, Fields, Forge, Walls}`, `Dues::{Light, Tithe, Levy}`, `Table::{Normal, Ration, Feast}`, `BuildKind::{Smithy, Granary, Palisade, Watchtower}` and `Dial::{Work, Dues, Table}`. Each has `.tag()`, `.name()` and `ALL`, and `Dial::setting_name(u8)` names a setting.
  - Orders: `OrderSpec::Demand { settlement }` (11) and `Subvert { settlement }` (12).
  - Causes 15-23: `Cause::{Sack{captain}, Submission{captain}, Subversion, Defiance, Fallout{settlement}, Reprisal{settlement}, Edict, Caught, Deposed}`.
  - Event kinds 22-33: `SimEventKind::{Submitted, Defied, Subverted, BanditsRose, ReprisalVowed, RivalReprisal, AgentCaught, Forsaken, Built, StewardNamed, EdictSet, Tribute}`, with `quiet()` and `notable()`.
  - Formats: `SAVE_FORMAT = 2`, `REPLAY_FORMAT = 6`. Only the merge lead changes them; later M4a layout changes amend the types in place.
- **sim_agents:**
  - Deeds 8-15: `Deed::{Gift, Defended, Influence, Whisper, Festival, Forsaken, Threat, Plot}`, with `Deed::ALL` and `Deed::spreads()`.
  - The deed log: `DeedLog { entries: VecDeque<DeedEntry{day, faction, deed, heard, delta}> }`, with `DeedLog::record` and `CAPACITY = 8`.
  - Agents: `Agent { town: Entity, by, since, suspicion }`.
  - Terms:
    - `LoyaltyTerm::{Resentment, Awe, Contest, Bond, Dues, Table, Steward, Stirred}`.
    - `LoyaltyTerms([f32; 8])`, with `get`, `set`, `bias` and `nonzero`.
    - `loyalty_target(.., bias)` and `loyalty_parts(..) -> (food, treatment, time)`.
    - `loyalty::{Pull, deciding_term, loyalty_pulls}`.
  - `edicts.rs`:
    - `Edicts { work, dues, table, pending_work/dues/table, dials: [DialState; 3], feast_until, feast_term_until, next_feast, builds }` and `DialState { by_player, locked_until, since }`.
    - `Rationed`, `Mustered { since }` and `Assignment { from, focus, since, by_steward }`.
  - `population.rs`: `Specialty::{Mining, Market, Timber, Fishing, Farming}`.
- **sim_economy:** `Construction { needs: Vec<(GoodId, u32)>, hours_left: f32 }`.
- **sim_military:**
  - `tenure.rs`:
    - `Tenure { how, by, since, scar, blood, blood_by, stir, kept, broken, weak_days, watched_hours, cool_until, history }`, with `Tenure::new`, `retake`, **`spill(by)`** and **`blood_of(taker)`**.
    - `TakenBy::{Native, Conquered, Sacked, Submitted, Invited, Subverted, Granted, Freed}`, with `TakenBy::of(cause, to)`, and `Taking`.
    - Data: `TenureData::scar`, `DemandData`, `CourtData` and `SubvertData`.
    - Functions: `pub(crate) record_taking`, `retake` and **`rebuild_terms`** (call it after changing anything a term reads, from a serial step, if the change must show before the next hour), and `step` (slot 14).
    - **Step 6 must read `blood_of(taker)` for its broken men before `record_taking`**, which resets the blood.
  - `influence.rs`: `Influence { power, player, rival }` with `net()`, `InfluenceData`, `step` (slot 13) and `contest_term(world, s)`, the term at owned towns.
  - `govern.rs`:
    - `Steward { town: Entity, traits: Vec<u8>, appointed, since }`.
    - Data: `EdictsData` (with `TitheData`, `LevyData`, `RationData`, `FeastData`), `StewardsData`, `StewardTrait`, `TraitSource`, `Setting` (with `dial()`), `Condition`, `MarketData` and `BuildingsData`.
    - Functions: `step` (slot 15), `dues_term`, `table_term` and `steward_term`.
  - `fallout.rs`: `Fallout { pending: Vec<PendingBand{settlement, men, due}>, last_violent }` (always present), `FalloutData`, `BanditWeights` and `step` (slot 16).
  - `components.rs`: `Bandits { home: u64, raids }`.
  - `rival.rs`:
    - `RivalState.realm: Vec<(u64, u32)>` (**without the seat**), `.reprisal: Option<Vow{settlement, due}>` and `.last_reprisal`.
    - `RivalBand.pinned: Option<u64>`.
    - `RivalData`: `reprisal_from_day`, `reprisal_size`, `reprisal_min_garrison`, `turned_mult` and `realm_floor`.
  - Elsewhere:
    - `Veteran.trade: Option<Occupation>` and `conquest::taking_kind(cause)`.
    - `GoodwillDeeds` gains `defended`, `festival`, `forsaken`, `threat` and `plot`.
    - `ProgressionData` gains `influence`, `tenure`, `demand`, `court`, `subvert`, `fallout`, `edicts`, `stewards`, `market` and `buildings` (all `#[serde(default)]`).
    - `CausePhrases` gains `sack` through `deposed` (required).
  - Lane B replaces the `Govern` arm in `orders.rs`, which returns "not yet" today.
- **game_ui:** `panels/why.rs`. `Why::term_name` and `pull_name` name the Contest term from the
  owner's side.

## 5. Checks and results

Every cargo command ran with `-j 2` and `CARGO_INCREMENTAL=0`, and tests ran with
`--test-threads=2`. The release binary is `target/release/sim_headless` from this tree (copied to
the scratchpad as `sim_headless_s1`, sha256 `9b1ccafe...`).

| Check | Result |
|---|---|
| `cargo fmt --all --check` | clean |
| `cargo clippy -j 2 --workspace --all-targets -- -D warnings` | clean |
| `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps` | clean, after one private intra-doc link was fixed |
| **Full suite** (`cargo test -j 2 --workspace -- --test-threads=2`) | **328 passed, 0 failed, 2 ignored** (`bot_run` and one sim_military test, both ignored by design). See the counts below |
| Determinism (`tests/determinism.rs`) | 13/13 in 222 s, including `state_hash_covers_every_sim_component`, `run_state_hash_covers_every_sim_component`, `a_run_replays`, `a_bot_run_replays` and `parallel_and_serial_execution_agree` |
| Run replay | `--run --seed 42 --ticks 3000 --commands 50 --record s1.ron`, which writes `format: 6`. Then `--replay` and `--replay --single-threaded` both print `replay ok: seed=42 ticks=3000 commands=51 checkpoints=3`, final hash `0xf7acb63eb01c336f` |
| Old formats | An M4 binary's replay is refused with "replay format 5 is not supported (expected 6)", and its save with "save format 1 is not supported (expected 2)" |
| Stdout invariance, against `docs/m4a/baseline/*.out` with `cmp` | developed (21 lines), skirmish (26) and battle (42): all **byte-identical** |
| Cost (NFR-11) | see below: **within +5% when measured side by side with the M4 binary**, but **not** against baseline.md's absolute numbers, because the machine was slower today |
| Bot smoke run (`--run --bot --seed 1 --ticks 24000 --chronicle`) | Runs to the end with no panic. At day 10: 11 settlements taken, 1 lost, 10 held (849 people); 2 monuments; 19 battles, 117 foes slain, 16 lost; 66 veterans, 4 elites. This matches the pre-fix smoke run |

Full-suite counts:
- Unit tests:
  - sim_core 39 (including the new `rejects_other_formats` assertions);
  - sim_agents 27;
  - sim_economy 11;
  - sim_military 82 + 1 ignored (including the new `a_taking_reads_only_its_takers_blood` and `a_scar_is_not_faded_on_the_tick_of_its_taking`);
  - sim_world 32;
  - sim_headless 4;
  - game_ui 31 (including `the_contest_is_named_from_the_owners_side`);
  - game_render 16;
  - game_app 4.
- Integration tests (all green):
  - `tenure` 8/8;
  - `save_load` 5/5 (with `every_lordship_type_round_trips`, a 367,818-byte save that loads back in about 1.5 s);
  - `conquest` 4/4;
  - `invitation` 2/2;
  - `run_start` 4/4;
  - `chronicle` 1/1;
  - `docs` 4/4;
  - `rival` 2/2;
  - `director` 3/3;
  - `champion` 3/3;
  - `dominion` 2/2;
  - `monuments` 2/2;
  - `sellswords` 6/6;
  - `wanderers` 2/2;
  - `gather` 2/2;
  - `orders` 7/7;
  - `posture` 1/1;
  - `army_march` 1/1;
  - `starving_army` 1/1;
  - `village_day` 1/1;
  - `famine` 1/1;
  - `worldgen` 7/7.
- The M2 and M3 guards (`village_day`, `famine`, `army_march`, `posture`, `starving_army`, `orders`) pass unchanged.

`conquest`'s `a_plundered_village_loses_faith` reads the same numbers after the review fixes: loyalty 0.453, 0.366 and 0.262 on days 1 to 3, against the 0.35 line, with the 0.3-day larder.

**Cost check (NFR-11).**
- **The `run ended` grep.** `--run --seed 7 --difficulty steady --ticks 7200 --stats-every 2400` prints no `run ended` line, so all three windows are live.
- **Three runs back to back** (load average about 2.5 at the start; nothing of mine was running):

  | Run | Window 1 | Window 2 | Window 3 | Total (ms/tick) |
  |---|---|---|---|---|
  | 1 | 1.66 | 2.03 | 1.85 | 1.847 |
  | 2 | 1.77 | 2.06 | 2.10 | 1.976 |
  | 3 | 1.82 | 2.40 | 2.01 | 2.077 |

  The median total is 1.976 against baseline.md's 1.776, +11% and over the 1.865 limit. Every stage slowed together, run after run: Decide 0.478, 0.514 and 0.544, and Move, Act and Input alike. That is the machine, not the code.
- **Then the M4 binary (`sim_headless_m4`, baseline.md's) and this one, interleaved,** three pairs under the same conditions:

  | Pair | M4 total | Step 1 total |
  |---|---|---|
  | 1 | 2.004 | 1.899 |
  | 2 | 2.070 | 2.150 |
  | 3 | 2.256 | 2.223 |

  - Medians: M4 2.070, step 1 2.150 (+3.9%).
  - Means: M4 2.110, step 1 2.091 (-0.9%).
  - The M4 binary itself now measures 13 to 27% over its own recorded baseline.
  - The Needs stage (`SimSet::Needs`, where the progress driver and slots 13-16 run) is 0.213, 0.215 and 0.233 for M4, and 0.196, 0.218 and 0.224 for step 1 (medians 0.215 and 0.218, +1.4%).
- **Verdict.** Step 1 costs nothing measurable. The +5% gate holds in the like-for-like comparison but cannot be judged against baseline.md's absolute numbers on today's machine. Re-run the three runs at MP0 on a quiet box, or re-record the baseline there.

`bot_run` was not run (the task said it isn't needed for step 1; see open issues).

## 6. Open issues

- **`bot_run` and the win-day floor** must run at MP0. Step 1 changes how a run plays through the
  Resentment term: lower loyalty after a conquest or sack, fading 0.02 a day, now applied from
  the taking's own tick. The seed-1 smoke run looked healthy and matched the pre-fix numbers, but a
  smoke run proves nothing about balance.
- **The cost check needs a quiet machine** (see above): re-run against `baseline.md`, or re-record
  the baseline and the step 1 numbers side by side.
- **Golden numbers.** State hashes change in every preset (new registered types, `Veteran.trade`
  and the progression text). Stdout is identical, and no golden hash is stored in the repository,
  so there is nothing to re-record.
- **The thin conquest margin.** `a_plundered_village_loses_faith` sits at 0.366 against 0.35 on
  day 2. Any later step that lowers a conquered town's target further (Contest at step 3, Dues at
  step 4) may tip it, and then the scenario's food changes, never the data.
- **Step 6 must read the blood before the taking resets it.** `fallout`'s men need
  `Tenure::blood_of(taker)` before `record_taking`; `record_taking` returns only the `TakenBy` today.
  Changing it to also return the blood would be the clean seam.
- **The revolt's words and the Dawn line** don't cite the deciding term or the largest pulls yet:
  - `SimEvent` has no field to carry them.
  - The Dawn line is step 4's.
  - A later step could capture `loyalty_pulls` in `transfer_seeded` for `Cause::Revolt`.
- **Panel screenshots.** The acceptance criteria want screenshots of the panel (own, independent and
  rival towns) in the step notes. Nobody could run the game here (no GPU session). The section can
  add up to about 22 lines to a bottom-anchored 380 px panel. Check it on the dev box, and cap the
  deed log at 5 or 6 lines in `goodwill_block` if it overlaps the villager panel.
- **Merge with lane B.** My edits to shared files are local:
  - In `orders.rs`, the `Govern` arm says "not yet", for lane B to replace.
  - Also: `progression/mod.rs`, the `lib.rs` and `save.rs` registrations, and progression.ron.
  - The docs' rows are in "Lordship (M4a, lane A step 1)" subsections or are marked M4a.
  - `panels.rs` gains only imports, `mod why;`, one parameter tuple and one call, and the section's
    code is in `panels/why.rs`.

  Lane B's files (economy.ron, `ai.rs`, `act.rs`, sim_economy `data.rs` and `layout.rs`, the
  workshop-stocks code) are untouched.
- **CLAUDE.md wording.** CLAUDE.md lists `CommandKind` among the saved enums, which it isn't (it is
  replay-only, by variant name). Up to you: drop it from that line. A lane agent does not edit
  CLAUDE.md.
- **HOWTO.** It has no recipe yet for "add a loyalty term" or "add a panel line". It should get one
  when step 3 adds the first live term from another module (Contest).
- **Disk.** Mid-gate, the full suite's test binaries (two sets of about 190 MB each, one stale) took
  free space down to 0.6 GB. The prune script (`prune_target.sh 90`, then 15) and removing
  `target/debug/incremental` brought it back to 7.7 GB. Later gates should prune before the full
  suite.
