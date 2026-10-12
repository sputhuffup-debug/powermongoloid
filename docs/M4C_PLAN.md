# M4c Plan: Saga

Status: **planned 2026-10-12 (revised the same day after review), awaiting your go.** M4c starts after the M4 playtest (`docs/M4_PLAYTEST.md`) and comes before M4b.
Design: `docs/SAGA_DESIGN.md` (draft 2026-10-12; your four decisions are recorded there). Spec: `docs/SEED.md` § "M4c: Saga".

M4c gives combat its stories. Every death names its killer and how it happened. Notable deaths get a brutal line, or a mild one if you choose, and the rest are summed up in the battle's report. Soldiers keep a record of what they did and earn an epithet for it. Captains and long-serving soldiers earn Brave, Shaken, Cruel or Beloved from what they live through. A few become legends who steady the men near them, and whose death shakes an army, pauses your game and leaves a cairn. The end screen remembers them, and a small record of each run's legends is kept for M6's book.

The steps are ordered to deliver value early:
- **The kill lines you asked for reach the game at step 4**, about 40% of the way in, before the steps that change play by design.
- **Steps 7 and 8 change how a run plays, by design** (earned traits and legends).
- **Steps 1 and 5 may shift a run slightly,** and are gated by `bot_run`. Step 1's rebel kill credit changes the player's kill count, which weights dominion boon offers. Step 5 makes a wiped army's `Battle` notable, which the pacing director sees.

## Requirements covered

| ID | Requirement | Where |
| --- | --- | --- |
| SIM-39 | Every death records who killed whom and how; notable deaths are told, the rest counted | `sim_military::combat` (the strike loop, its battle bookkeeping and `casualties`), new `sim_military::saga` |
| SIM-53 | Service records, deeds and epithets | `sim_military::saga`, `sim_military::veterans` (`end_battles`), `sim_military::orders` |
| SIM-34 | Earned traits; the champion's picked traits stay as built | `sim_military::saga`, consumers in `soldiers`, `upkeep`, `combat` and `orders` |
| SIM-54 | Legends: rise, aura, death shock | `sim_military::saga`, `index` (`LegendIndex`), `soldiers`, `upkeep`, `combat` |
| UI-10 | Phrase banks in two tones; tone as a viewer setting | new `assets/data/saga.ron`, `sim_military::chronicle` (the `Teller`), `sim_military::saga` (words), `game_ui`, `game_app` (preferences) |
| UI-09 | Battle-report moments; biographies of people and settlements | `sim_core::events` (`BattleReport`), `sim_military::saga` (words), `game_ui` panels and end screen |
| RND-09 | Legend marks, cairns, memorial stones | `game_render` |
| GP-29 | The run record for M6's legends book | `sim_military::saga::run_record`, `game_app` (writes it), `sim_headless` (`--legends-out`) |
| NFR-09 | The saga's cost in big battles | `sim_headless --timings` (with and without `--saga-bench`), `cargo bench -p sim_headless` |

**Not in M4c:**
- **M4b:** heroes (SIM-51, SIM-52), which may reuse the legend aura; weapon kinds and named injuries, which come with the equipment tiers; and healing.
- **M5:** villagers' memories of the dead (SIM-14), and prisoners and executions (GP-14's dilemmas).
- **M6:** the legends book itself, and the options screen. M4c pulls forward only the tone switch.
- **Not adopted:** nemesis AI. Open question 1 in the design adds only words and a deed.

## Acceptance criteria and how each is tested

| Criterion | Test |
| --- | --- |
| Every death by a blow names its killer and how it happened, and a rebel's kill is credited to the rebels | **Test `kill_records_name_the_killer`** (sim_military unit, hand-built world): a killing blow records victim, killer, credited side and one-blow. A second swing at the dead records nothing. The manner bits (armed, fleeing, last stand) are set in `casualties`. **Scenario `rebels_kill_for_themselves`** (`tests/tests/saga.rs`, run preset): a conquered village with a small garrison revolts. Every rebel kill counts for faction 0 in `ArmyStats`, the garrison's report lists faction 0 among its foes, and a champion slain by rebels is told as killed by the villagers. |
| Captains, the champion and legends are always told; any other death only if it clears its battle's rising bar, at most 10 a battle and 8 told deaths a tick; the feed at 8x shows only lines about your people or a legend; the report's "more died" matches | **Scenario `notable_deaths_follow_the_rules`** (1,000 a side, seed 42, 3,000 ticks). Every captain's death is told. Per battle, at most 10 tier-B deaths are told, and at seed 42 the big battle tells between 4 and 10. No tick tells more than 8 deaths unless they're all tier A. Every told line raised the bar. Told plus untold equals deaths, and every named person's death is kept. Each report's "more died" equals its losses minus its named dead. **Scenario `battles_share_a_number`:** two armies meeting get one battle number, place and ordinal; a third army striking one of them joins it. **`bot_run`:** over every seed, at most 8 deaths a battle pass `saga::feed_filter` at 8x. |
| Battle reports name first blood, who ran first, when the army broke, the told deaths and the best killer; in a run, an army killed to the last man still gets its report | **Scenario `moments_reach_the_report`** (two squads of 20, seed 42): both armies' reports carry one battle number and name, and the same first blood. Who ran first is the soldier whose rout set the turning point. The broke tick is the first with 30% of the living members routing. The best killer is the top killer even if killed later. **Scenario `wiped_army_gets_a_report`** (run preset, a band of 5 against 40): a report marked wiped, with a `Battle` entry. |
| In a run, deeds earn one epithet by rule, and Brave, Shaken, Cruel and Beloved are earned from experience; each acts only through its holder (their own morale, aim and desertion, and a captain's own dealings), and anyone without one behaves exactly as before | **`tests/tests/saga_traits.rs`:** `brave_and_shaken`, `cruel_and_beloved`, `picked_and_earned_are_separate`, `epithets_follow_rank` and `earned_traits_are_exact_no_ops`, as described in steps 6 and 7. |
| Legends rise by fame, at most three enlisted per faction; they steady men within 8 tiles; their death shakes their army once and pauses your game for your own | **`tests/tests/legends.rs`:** `legend_rises_at_fame`, `legend_steadies_men` and `legend_death_shakes_once`, as in step 8. **A `game_ui` test** (`legend_fell_pauses_for_your_own`): the feed pauses for your legend's `LegendFell` and not for an enemy's. **`bot_run`:** still wins 3 of 5 seeds, and a player legend rises on at least 3 of them. It prints legends, epithets and earned traits per seed. |
| Brutal and mild banks have the same keys; every line names its key's facts and uses only its key's slots; no gendered words; feed lines fit the feed; the tone never changes the simulation | **`tests/tests/saga_words.rs`:** a lint of the real `saga.ron` and a golden test on a small fixture bank (both in step 3). **Test `tone_is_not_state`:** a bot run's chronicle, told in both tones, reads differently, while its state hash and a recorded replay are identical. |
| The inspector shows biographies of soldiers and settlements; the end screen lists legends, foes remembered and the fallen; cairns mark where legends fell | **Unit tests** of the biography words: a dead legend, a living soldier, a spawned soldier and a settlement, in both tones; nobody named is ever told as "a soldier". **Screenshots** of each surface in the step notes. **A `game_ui` test** that the feed's filter drops tier-B lines at 8x and folds the overflow. |
| A finished run writes its legends record, and no sim crate reads it | **Test `run_record_lists_legends`** (`tests/tests/saga.rs`): the bot plays seed 1 to the end of its run. Its record round-trips through RON, names every legend and gives each a fate, with names written as strings. An abandoned run writes one with outcome `Abandoned`. |
| NFR-09 holds; developed, skirmish and `--battle` stdout are unchanged; saves and determinism hold | `--battle 7500 --timings --stats-every 250` and, from step 7, the same with `--saga-bench`: the median of three runs of the clash window's mean tick rises at most 2% over the M4 baseline recorded at step 1. The bench. The stdout comparisons in "What stays unchanged". **`save_load`:** records, legends and moments round-trip, `every_hashed_type_is_saved` covers the new types, and a format-1 save is refused. The determinism tests, single- and multi-threaded. |
| **Playtest:** you can name a legend after a run and say how they fell; a death landed; brutal reads grim, not numbing; mild reads sober, not dry | You play two runs, one in each tone (checklist `docs/M4C_PLAYTEST.md`, written in step 11). |

## What stays unchanged

- **Stdout of the guarded commands.** Each step compares these release commands with the previous step's binary, and the stdout must be byte-identical. State hashes and timings, on stderr, may change.
  - developed: `sim_headless --seed 42 --ticks 4801 --stats-every 600`
  - skirmish: `--skirmish --seed 42 --ticks 6000 --stats-every 2000`
  - battle: `--battle 1000 --seed 42 --ticks 3000 --stats-every 600`
- **The M2 and M3 tests** pass unchanged, with today's tuning: `village_day`, `famine`, `army_march`, `posture`, `starving_army` and `orders`. Every saga effect is run-only, and is an exact no-op without a trait or a legend.
- **Replays.** The replay format (`REPLAY_FORMAT` 5) and the command kinds don't change: M4c adds no command, and the tone is never an input.
  - Old replay files still parse, but their checkpoint hashes stop matching after step 2, as after every M4 step. Step 1 already changes those in which rebels fought. Re-record any you want to keep.
  - In every step, a fresh run replay records and verifies, multi- and single-threaded.
- **Pacing.** The director's quiet clock sees only `LegendFell` and a wiped army's `Battle` as new notable events. Fast-forward stops on nothing else new.
- **`FactionModifiers` is untouched.** Nothing personal goes into it, and the champion's picked traits still act faction-wide.
- **The chronicle's existing entries keep their words, with one exception.** The `Battle` line gains faction nouns ("against the rival's band" instead of "against Rival") and the battle's name (steps 3 and 5). `chronicle.causes` gains one phrase.
- **Saves are the one intended break.** From step 2 (`SAVE_FORMAT` 2), M4 saves are refused. That's why M4c follows the playtest.

## The gate (every step)

1. **Build and lint:** `cargo fmt --check`; `cargo clippy -j 2 --workspace -- -D warnings`; `cargo test -j 2 --workspace`; `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps`.
2. **Determinism:** the determinism tests, plus a run replay. Record `--run --seed 42 --ticks 3000 --commands 50 --record run.ron`, then verify it with `--replay run.ron` and `--replay run.ron --single-threaded`.
3. **Invariance:** the three stdout comparisons above.
4. **Cost, for sim steps:** `--battle 7500 --seed 42 --timings --stats-every 250`, three runs, against the baseline in `docs/m4c/` (NFR-09). From step 7, also with `--saga-bench`.
5. **Docs in the same commit** (CLAUDE.md "Developer handbook"):
   - `docs/dev/DATA.md` for data;
   - `docs/dev/ARCHITECTURE.md` for systems and slots;
   - `docs/dev/TESTING.md` for tests;
   - `docs/dev/GLOSSARY.md` for terms;
   - ADR 0019 for decisions.
6. **Balance, for steps marked bot_run:** `cargo test --release -j 2 -p integration_tests --test bot_run -- --ignored` still wins 3 of 5. Record the win days and saga counts in the step notes. Change data when the game is unbalanced, and the bot when it plays badly (HOWTO "Tune run balance").

Step notes go in `docs/m4c/`, as M4's went in `docs/m4/`.

## Build order

Sizes are relative shares of the milestone: S is up to 5%, M about 8–12%, L about 15%.

### Step 1: Kill attribution and the rebels' credit (S, 5%; bot_run)
- **What.**
  - **The baseline first.** Three runs of `--battle 7500 --seed 42 --timings --stats-every 250` on the M4 build, recorded in `docs/m4c/baseline.md`: deaths a tick, the clash window's mean tick, the Act, Decide and Needs stage times, and the spread between runs.
  - `Casualties.slain_by` becomes `kills`, a list of `Kill`: victim, killer, the credited side, and one blow (the victim was at full health).
  - A rebel's kill is credited to the independents, matching how `casualties` already treats rebel victims. A rebel is militia whose `against` is its own allegiance. The credit applies in `ArmyStats.kills`, in the garrison's `battle_foes`, and in the champion's cause of death.
- **Where.** `crates/sim_military/src/combat.rs` (`Casualties`, the kill branch of `resolve_strikes`); `crates/sim_military/src/progression/champion.rs` (`cause_of_death` reads the kills through a small helper); `crates/sim_military/src/veterans.rs` (its unit test asserts on the scratch); `crates/sim_military/src/save.rs` (`after_load` clears the scratch).
- **Tests.** `kill_records_name_the_killer`; `rebels_kill_for_themselves` (new `tests/tests/saga.rs`). Re-run `champion` and `conquest`.
- **Docs.** ARCHITECTURE schedule row 5 (Act): the kill scratch. TESTING rows. ADR 0019 is opened with the kill record and the rebel fix.
- **Done when.** Every kill names its killer, rebels' kills are theirs, the baseline is recorded, and nothing else moves.
- **Gate note.** The player's kills weight the dominion boon offers, so the fix can change a run: re-run `bot_run`.
- **Ships as:** a fix. When rebels fight, the run summary's kills and the champion's cause of death come out right.

### Step 2: Battle numbers, service records and told deaths (L, 15%)
- **What.**
  - **Battle numbers.** The strike loop's battle bookkeeping, in striker SpawnId order, joins two armies into one battle: an army with a number keeps it, one without takes the other's or a new `Saga.next_battle`. A militia blow joins the army it fights. `Captain` gains `battle`, which passes to an heir with `battle_since`. The first blow fixes the battle's place (nearest settlement), ordinal there and terrain, in `Saga`.
  - **Broke.** `build_index` sets `Captain::battle_broke` at the first tick with `saga.broke_share` (0.3) of the army's living members routing, beside `battle_rout`.
  - **`Veteran` gains three fields:**
    - `routed`: ran in this battle, set in `soldier_upkeep` on the soldier's own component;
    - `hurt`: the share of health lost to blows this battle, a saturating byte added in the strike loop on the victim's `Veteran`, which is already borrowed;
    - `trade`: the occupation before enlisting, captured in `enlist`.
  - **New module `sim_military::saga`:**
    - `Service`, stored as a sparse-set component so that inserting one never moves a soldier's table row. It has the fields of the design's section 5: trade, home, kills, streak, battles and wounds here, and the rest from steps 6 and 7;
    - `Saga`: kept deaths (`Fall`, with a `told` flag), battles, per-settlement battle counts, an untold count, `next_battle` and a running digest;
    - `SagaDeed` ids.
  - **`casualties` gains a first pass** over the kills, in strike order, before anything is despawned. It snapshots each killer, starts or updates their record, counts the streak, and updates the army's best killer (`Captain.battle_best`).
  - **The existing loop** (victim SpawnId order) classifies each death:
    - **Tier A:** the victim is a captain, the champion or a legend, or a legend makes a milestone kill. Always told, first.
    - **Tier B:** told against the battle's rising bar, at most 10 a battle (`max_per_battle`), with whatever remains of the tick's cap of 8.
    - **Every told line raises the bar.** A told death is pushed as a `Fall`, and so is the untold death of anyone named. A tier-A death by hunger gets a `Fall` with no killer.
  - **No new event for a death.** Told deaths live in `Saga`, and the feed reads them there (step 4), so they never pass through `EventFeed`.
  - **Rules** in `progression.ron` `saga`: `broke_share`, and `notable` (weights, bar, step, `per_tick`, `max_per_battle`). It's a `#[serde(default)]` section.
  - `SAVE_FORMAT` becomes 2. Later steps bump it again whenever they change a saved layout.
- **Where.** `crates/sim_military/src/saga.rs` (new); `combat.rs`, `index.rs`, `veterans.rs`, `orders.rs` (`enlist`, the heir's battle state), `upkeep.rs` and `components.rs` (`Captain` and its hash) in `crates/sim_military/src/`. Registration is in `lib.rs` and `save.rs`, in hash order. Also `crates/sim_core/src/save.rs` (`SAVE_FORMAT`), `crates/sim_military/src/progression/mod.rs` (the `saga` section) and `assets/data/progression.ron`.
- **Tests.**
  - `notable_deaths_follow_the_rules`;
  - `battles_share_a_number`;
  - `starving_is_not_a_kill`: a starving captain gets a told death with no killer and cause `Starvation`;
  - `records_survive_desertion`: a soldier with kills deserts and re-enlists, and the record keeps its kills;
  - `save_load`, with the new state round-tripped;
  - the determinism coverage tests.
- **Docs.**
  - DATA.md: `saga.broke_share`, `saga.notable`.
  - ARCHITECTURE: row 5 (the battle bookkeeping and `casualties`' two passes), row 2 (`battle_broke`) and row 6 (`routed`).
  - GLOSSARY: Battle number; Broke, Lost and Held; Service record; Told death and Kept death; Tier; Rising bar. The existing **Battle** entry gains the number.
  - TESTING.
  - ADR 0019: records hold ids, and the words come late; battles get a number; told deaths live in `Saga`, not in the event feed.
- **Done when.** A battle of any size keeps its told deaths bounded, every death is told, kept or counted, both sides of a fight share its number, the hashes cover the new state, and saves round-trip.
- **Ships as:** `sim_headless --saga` lists the told deaths in a plain debug form, until step 3 puts words to them.

### Step 3: Words and tone (M, 12%)
- **What.**
  - **`assets/data/saga.ron`**, embedded at compile time like `worldgen.ron`. The brutal and mild banks share one type, so a key missing from one tone fails to parse. Each key declares its fact slots and flavour slots.
  - **`SagaWords`**, exempt from the hash ("words only; the sim never reads it") and not saved.
  - **`Tone`** as a parameter of `describe`, `describe_entries` and the new words functions. Game call sites pass Brutal until step 4.
  - **The `Teller`:**
    - names the dead from `Saga`'s kept deaths;
    - picks keys most specific first, with militia and rebel victims always on their grave keys and first blood or a streak carried by `{moment}`;
    - fills the slots: the per-person weapon word, the old trade's tool, ground by terrain, `{daypart}` from the tick, beast nouns and faction nouns;
    - chooses variants by told-death index, never by RNG;
    - names battles from their number's facts ("the Second Battle of Loweby Fields").
  - **Headless:** `--saga` prints told deaths and legends after the run, or with `--chronicle`; `--mild` switches the tone.
- **Where.** `assets/data/saga.ron` (new); `crates/sim_military/src/chronicle.rs`; `crates/sim_military/src/saga.rs` (words); `crates/sim_headless/src/main.rs` and `crates/sim_headless/src/stats.rs`; the `describe` call sites in `crates/game_ui/src/`.
- **Tests.**
  - **`saga_words` lint (`tests/tests/saga_words.rs`):**
    - the same keys in both tones;
    - every line uses only its key's slots, and names every fact slot of its key in both tones (mild may omit flavour slots);
    - at least 6 brutal and 4 mild variants, and every bank shorter than 37;
    - no he, she, his, her, him or hers, and none of the design's gendered nouns;
    - a short deny-list;
    - every line that can reach the feed, filled with the longest given name, family name, epithet and place in the data, fits 72 characters (one feed row after the "Day N: " prefix); report-only lines fit 110;
    - every epithet id, deed and earned-trait id has words in both tones.
  - **`saga_words_golden`:** a fixture bank inside the test and a scripted duel give exact lines in both tones, so edits to the real bank don't churn it.
  - **`tone_is_not_state`.**
  - Re-run `chronicle`.
- **Docs.**
  - DATA.md: a `## saga.ron` section covering every field, with the content rules (graphic toward fighters, grave toward militia and rebels, never sexual violence, torture as spectacle, slurs or children; no gendered words; words may invent the manner, never the facts; times only from `{daypart}`).
  - HOWTO: a recipe, "Add a kill line, an epithet or an earned trait".
  - GLOSSARY: Phrase bank, Tone, Fact slot.
  - ADR 0019: the words-only file, and the tone as a viewer setting.
- **Done when.** These read well in both tones and the lint passes:
  - `sim_headless --skirmish --ticks 6000 --saga`;
  - `sim_headless --run --bot --seed 1 --ticks 24000 --chronicle --saga`.
- **Ships as:** kill lines in headless.

### Step 4: Kill lines in the game (M, 8%) — the first slice you can play
- **What.**
  - **Feed lines for told deaths,** read from `Saga` with the feed's own cursor (a `Local`, like the event cursor), so they never crowd `EventFeed`.
  - **`saga::feed_filter`**, a pure function in sim_military: at 1x–2x, deaths where your people killed or died plus tier A of anyone; at 4x, tier A involving your faction or a legend, plus first blood in your battles; at 8x and fast-forward, tier A involving your faction or a legend.
  - **The rate limit and fold** at every speed: at most one line every 1.5 real seconds per place, the overflow folded into "…and 3 more fell at Loweby". Its numbers sit in `saga.ron`'s `feed` section.
  - **Colour** by kind.
  - **Clicking a line** selects the killer if alive, through a UI-side SpawnId lookup, or else moves the camera there.
  - **The tone setting:** "Words: Brutal / Mild" on the new-game screen and as a switch on the feed. `WordsTone` is a game_ui resource, kept in a preferences file next to the saves (game_app does the file IO). The game also takes `--mild`.
  - **The feed's width.** If 72 characters proves too tight for good lines, widen the feed here, and raise the lint's limit with it.
  - **Two log-line fixes:** the military log lines that print a SpawnId where a name belongs (a captain's heir, and the champion's successor). `log_line_shown` is kept in step.
- **Where.** `crates/sim_military/src/saga.rs` (`feed_filter`), `crates/game_ui/src/feed.rs`, `crates/game_ui/src/screens.rs`, `crates/game_app/src/main.rs`, `crates/game_app/src/saves.rs`, `crates/sim_military/src/combat.rs` (`succeed`'s line), `crates/sim_military/src/progression/champion.rs`.
- **Tests.**
  - `feed_filter` unit tests per speed;
  - game_ui unit tests: the throttle (tier B dropped at 8x, the fold count) and the preferences round-trip;
  - a manual check in the skirmish and a run, at 1x and 8x, with screenshots in the step notes.
- **Docs.** ARCHITECTURE's game-side table (feed); TESTING; README's controls if a key is added.
- **Done when.** Notable deaths are told in the feed without flooding it, switching tone rewrites the visible lines, and the choice survives a restart.
- **Ships as:** **the brutal kill messages you asked for**, in the skirmish and in runs. This is a natural point for a quick look before the rest.

### Step 5: Battle moments (M, 12%; bot_run)
- **What.**
  - **`BattleReport` gains the moments:** `battle` (the number, which leads to the name, first blood and the told deaths in `Saga`), `broke`, `drew_first`, `best`, `ran_first`, `hurt` (the wounded), `deserted`, `wiped`, and `earned` (empty until step 6).
  - **`Captain` gains** `battle_ran_first`, `battle_deserted` and `starved`.
  - **`build_index` names who ran first.** It already finds the moment.
  - **`apply_desertions` counts deserters** per army.
  - **`end_battles`:**
    - counts a wound for each survivor whose `hurt` reached a quarter of their health;
    - fills the report;
    - clears the new fields with `fought`.
  - **New progress slot 13, `saga::settle`** (hourly, every preset, after `director::step`): the hourly starvation summary `Starved`, feed-only.
  - **A wiped army in a run:** when no heir is left, `succeed` writes the report (`wiped`) and its `Battle` entry.
  - **Words:** the report's moments, "N more died", the `Battle` line's battle name and faction nouns, the end screen's "Your battles", and the feed's battle line opening into the full report.
- **Where.**
  - in `crates/sim_core/src/`: `events.rs` (`BattleReport`, its hash and save; the `Starved` kind, `ALL`, `name`, `chronicled`);
  - in `crates/sim_military/src/`:
    - `index.rs`, `upkeep.rs`, `veterans.rs`;
    - `combat.rs` (`succeed`);
    - `progression/mod.rs` (slot 13);
    - `chronicle.rs`;
  - in `crates/game_ui/src/`: `feed.rs` and `screens.rs`.
- **Tests.** `moments_reach_the_report`, `wiped_army_gets_a_report`, `starving_is_told_once_an_hour`, the event order test. Re-run `chronicle`, `director` and `save_load`.
- **Docs.**
  - ARCHITECTURE: row 6 (slot 13, after `director::step`) and row 2 (who ran first).
  - GLOSSARY: Moment.
  - TESTING.
  - ADR 0019: the wiped report amends ADR 0017's "as built".
- **Gate note.** A wiped army's `Battle` is notable, so pacing can move: re-run `bot_run`.
- **Done when.** Every report tells its moments in both tones, both sides' reports of a fight share its name, and a wiped army is reported in a run.

### Step 6: Deeds and epithets (M, 10%; run only)
- **What.**
  - **Deeds** (`SagaDeed`) are recorded once each, as (deed id, tick, battle number or nearest settlement, victim `NameId`):
    - at the kill: first blood, slayings, shield, five in a fight, doorwarden, kinslayer;
    - at `end_battles`: held the line, survivor, ran first, last stand (a kill after the army broke, and alive at the end);
    - in `run_orders`, on the captain: plunderer, breadgiver.
  - **The rest of `Service`:** `joined`, `held`, `fights` (the latest 16 battles with their kills).
  - **Fame** is worked out from the record.
  - **Epithet rows** (`saga.epithets`: a stable `id`, a `rank` and a rule) are applied by one function, `saga::judge`:
    - from `end_battles`, for each survivor, so the report lists what was earned;
    - from slot 13, for every other record changed since the last hour (militia after a kill, captains after dealings).
  - **`Named`**, chronicled but not notable, with the new `Cause::Earned { deed }`. `notable()` excludes the new chronicled kinds.
  - **Inspector:** the epithet after the name, then kills, battles, wounds and deeds.
- **Where.**
  - `crates/sim_military/src/`: `saga.rs`, `veterans.rs`, `orders.rs`;
  - `crates/sim_core/src/events.rs`: `Named`, `Cause::Earned`, `notable`;
  - `assets/data/progression.ron`: deed fame, epithet rows, the `chronicle.causes` phrase;
  - `assets/data/saga.ron`: epithet and deed words, keyed by id;
  - `crates/game_ui/src/panels.rs`.
- **Tests.**
  - `deeds_are_recorded_once`, with their place and victim;
  - `epithets_follow_rank`: Nine-Kills, then a captain slain, replaces it; a lower rank never replaces a higher; re-ranking the rows keeps every stored id; militia earn Doorwarden at the kill;
  - `every_cause_is_worded`;
  - the event order test;
  - `chronicle` and `director`;
  - `bot_run` prints epithet counts; no behaviour change is expected.
- **Docs.** DATA.md; GLOSSARY: Saga deed (kept distinct from goodwill's **Deed**), Fame (a per-person score with no unlocks, not FUN_DESIGN's renown), Epithet; TESTING; ADR 0019.
- **Done when.** A bot run of seed 1 earns several epithets, each with its reason and place, and the inspector shows the record.
- **Ships as:** names you notice.

### Step 7: Earned traits (M, 10%; run only; bot_run)
- **What.**
  - **Marks** count only once a person is eligible (a captain, the champion, a legend, or three battles survived), and a battle gives at most one of each kind:
    - held and broke at `end_battles`;
    - cruelty at the kill (a fleeing victim, a kinslaying) and in `run_orders` (plunder);
    - kindness in `run_orders` (food to a short village, a bring-food request) and at `end_battles` (held at home).
  - **Cruel** is appended as trait id 7 and kept out of `champion_traits`.
  - **`saga.earned`** has one row per earned trait: the mark and count, the opposite, and the personal effects.
  - **`judge`** awards at most one trait per battle. Opposites replace each other: Brave over Shaken records Redeemed. It writes the holder's cached nudges on `Veteran` (break-point offset, morale-target offset, desertion multiplier), and `enlist` rebuilds them from `Service`.
  - **Picked and earned stay separate:** earned traits live only in `Service`; `draw_traits` ignores them; succession never moves them; an earned trait the champion already picked isn't awarded again.
  - **`TraitEarned`** is a new kind, chronicled and not notable (today's `TraitGained` is notable and stays for picks).
  - **Consumers through the holder, run-only, exact no-ops without the trait:**
    - the break point in `soldiers::decide`;
    - Cruel's aim at a fleeing foe in `aim`;
    - the morale target and desertion in `soldier_upkeep`;
    - Cruel's extra kill morale in `casualties`' first pass, so the strike loop is untouched;
    - a Cruel captain's plunder goodwill and a Beloved captain's recruit chance, in that captain's own dealings in `run_orders`.
  - **`--saga-bench`** for `--battle`: turns on the saga's run-only effects (not the rest of the run rules) and gives every tenth soldier an earned trait; step 8 adds three legends per army.
  - **Inspector:** traits with what earned them, and picked traits marked "chosen".
- **Where.**
  - `crates/sim_military/src/`: `saga.rs`, `soldiers.rs`, `upkeep.rs`, `combat.rs`, `orders.rs`, `progression/champion.rs` (`draw_traits` and succession left as they are, with tests);
  - `crates/sim_headless/src/main.rs` (`--saga-bench`);
  - `assets/data/progression.ron`: `traits` and `saga.earned`;
  - `assets/data/saga.ron`;
  - `crates/game_ui/src/panels.rs`.
- **Tests (`tests/tests/saga_traits.rs`).**
  - `brave_and_shaken`: an army that breaks twice. Its eligible veteran who held earns Brave, and one who ran twice earns Shaken. A later Brave cures Shaken and records Redeemed. Marks from before eligibility don't count.
  - `cruel_and_beloved`: a captain whose army plunders in three separate hours turns Cruel; giving food to a short village twice makes the captain Beloved instead.
  - `picked_and_earned_are_separate`: the champion with picked Brave can earn Shaken, keeps the faction-wide Brave, and never earns Brave again; succession moves no earned trait; offers ignore earned traits.
  - `earned_traits_are_exact_no_ops`: on a hand-built world, soldiers with no trait, under a captain with none and with no legend within 8 tiles, have bit-identical morale, break point and desertion.
  - `champion`: picked traits stay faction-wide.
  - Re-run `army_march`, `posture`, `starving_army` and `orders`.
- **Docs.** DATA.md; GLOSSARY: Earned trait, Mark, Eligible; TESTING; ADR 0019.
- **Done when.**
  - Traits are earned at the design's target rates: at the end of a won run, at most a quarter of eligible veterans hold one, and Cruel is no more common than Brave. Counts per bot seed go in the step notes.
  - Each trait acts only through its holder.
  - `bot_run` still wins 3 of 5, and NFR-09 holds with `--saga-bench`.

### Step 8: Legends (M, 10%; run only; bot_run)
- **What.**
  - **`Legend { faction, rose }`.** The rise rule and the cap are in `saga.legend`: fame 25, three battles, an epithet, and three enlisted per faction by current allegiance. A deserted legend keeps the title in words and frees the slot.
  - **`LegendIndex`**, exempt and rebuilt in `build_index` each tick in a run, before any reader. It holds only enlisted legends, and is the set the cap counts.
  - **The aura:** −0.03 on the break point in `decide`, and +0.03 on the morale target in `soldier_upkeep`. It works within 8 tiles, doesn't stack and only counts enlisted legends.
  - **The death shock,** in `casualties`: −0.10 once to the legend's army and to their faction's fighters within 12 tiles, +0.03 to foes within 12 tiles. A captain who is also a legend costs the larger of the two losses, not the sum.
  - **One event per death:** champion > legend > captain. `LegendRose` is chronicled, not notable; `LegendFell` is chronicled and notable.
  - **The pause:** `LegendFell` is added to every difficulty's `auto_pause` in `director.ron`, and the feed pauses only for your own.
  - **`--saga-bench`** gains three legends per army.
  - **Grudges and the Avenger deed,** only if you answer yes to open question 1. They're about 2% of the milestone.
- **Where.**
  - `crates/sim_military/src/`: `saga.rs`, `index.rs`, `soldiers.rs`, `upkeep.rs`, `combat.rs`;
  - `crates/sim_core/src/events.rs`;
  - `assets/data/director.ron` and `assets/data/progression.ron`;
  - `crates/game_ui/src/feed.rs`.
- **Tests (`tests/tests/legends.rs`).**
  - `legend_rises_at_fame`: threshold, battles and epithet; with the cap full, a fourth waits; a legend who deserts frees the slot.
  - `legend_steadies_men`: an ally at 7.9 tiles gets both nudges, one at 8.1 tiles is exactly unchanged, two legends don't stack, and a deserted legend steadies no one.
  - `legend_death_shakes_once`: the numbers above, `LegendFell` carrying the right faction, the captain-legend rule, and a champion who is a legend getting `ChampionFell` only.
  - `legend_fell_pauses_for_your_own` (game_ui unit test).
  - `director`; `bot_run` (a player legend on at least 3 seeds; at most 8 deaths a battle pass the 8x filter; legends per seed in the step notes).
- **Docs.** DATA.md; ARCHITECTURE: row 2 (`LegendIndex`); GLOSSARY: Legend, kept distinct from Hero, Champion, Veteran and Elite; TESTING; ADR 0019.
- **Done when.** Legends rise in most bot runs, steady and shake as specified, and a legend's death pauses your game.

### Step 9: Biographies, end screen and memorials (M, 12%)
- **What.**
  - **Words:** `saga::biography` and `saga::settlement_history`, words-only functions taking a tone.
  - **Inspector:** the biography replaces "Memories: none yet" for anyone with a record.
  - **Captain panel:** Notables; clicking one selects them.
  - **Settlement panel:** the settlement's history.
  - **End screen:** Legends of the run, Foes remembered (enemy legends, and whoever killed one of your legends or your champion), The Fallen and the bloodiest battle. The timeline shows legends risen and fallen and hides `Named` and `TraitEarned`.
  - **Map,** in `game_render`, as render-side state only:
    - a pennant over each living legend inside the detail band;
    - a cairn where each legend fell;
    - a memorial stone at their home village's centre;
    - hovering a cairn or stone shows the epitaph.
- **Where.**
  - `crates/sim_military/src/saga.rs`;
  - `crates/game_ui/src/`: `panels.rs`, `army.rs`, `screens.rs`, `selection.rs`;
  - `crates/game_render/src/`: a new memorials module beside `monuments.rs`, with meshes from `crates/game_render/src/art.rs`.
- **Tests.**
  - Unit tests of the biography words: a dead legend, a living soldier, a spawned soldier and a settlement, in both tones. Nobody named is ever told as "a soldier".
  - Screenshots of each surface in the step notes.
- **Docs.** ARCHITECTURE's game-side table; GLOSSARY: Biography, Memorial; TESTING.
- **Done when.** Every surface in the design's section 8 table shows its saga, in both tones.

### Step 10: The run record for M6 (S, 3%)
- **What.**
  - **`saga::run_record(world) -> RunRecord`:** pure, serde RON, versioned (`RUN_RECORD_FORMAT` 1). It holds facts and proper nouns, never sentences.
  - **The game** writes `legends/<seed>-<difficulty>-<ended tick>.ron` under its save directory when a run ends, abandoned runs included. This happens before `leave_run` removes the finished run's slot. The same ending, after a quickload, overwrites its file.
  - **Headless:** `sim_headless --legends-out <path>`.
- **Where.** `crates/sim_military/src/saga.rs`, `crates/game_app/src/saves.rs`, `crates/sim_headless/src/main.rs`.
- **Tests.** `run_record_lists_legends`.
- **Docs.**
  - ARCHITECTURE: the saves section, on what survives a run.
  - GLOSSARY: Run record.
  - ADR 0019: no code reads the record in M4c, and nothing carries between runs.
- **Done when.** A finished run in the game leaves one record file, and headless writes the same record for the same run.

### Step 11: Docs and playtest (S, 3%)
- **What.**
  - Complete ADR 0019.
  - CLAUDE.md:
    - add a "Saga (ADR 0019)" rules block: the sim stores ids, never words; `saga.ron` is words only, exempt and unsaved; a told death goes through a `Fall` in `Saga`, never through `EventFeed`; battles get a number at their first blow; effects act through their holder and are run-only; `Saga` is append-only; `saga::settle` is progress slot 13;
    - update the milestone line.
  - README's status line.
  - PLAY_GUIDE: legends, the tone switch, and a cheat-sheet row.
  - SEED.md: tick the criteria that pass, with dated measurements, including NFR-09's.
  - Write `docs/M4C_PLAYTEST.md`.
- **Done when.** Every M4c criterion except the playtest is ticked, and the playtest checklist is ready.

Steps 1–3 and 5–8 are sim work, testable headless; step 3's words are checked in headless output. Steps 4 and 9 need the game. Step 10 needs both.

## Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Brutal words read as gratuitous, especially about villagers | High | Militia and rebel victims always take grave keys; content rules in DATA.md; a deny-list lint; words reach the game at step 4 for an early look; a playtest question |
| Kill lines flood the feed in big battles | Medium | Tier A narrowed to captains, the champion and legends; the rising bar, 10 tier-B lines a battle and 8 told deaths a tick in the sim; the speed filter, rate limit and fold in the feed; tested at 1,000 a side and over every `bot_run` seed |
| Legends too rare to notice, or too common to matter | Medium | The numbers are data, tuned against `bot_run`'s counts; epithets carry the mid-game; the cap of three |
| Earned traits too common (Cruel above all) | Medium | Marks only from eligibility, one of each kind per battle; target rates checked in step 7 |
| Balance or pacing drift | Medium | Effects are run-only and exact no-ops; new kinds aren't notable except `LegendFell`; `bot_run` after steps 1, 5, 7 and 8 |
| Big-battle cost | Medium | A push per kill is the only new work per blow; per-death work is bounded; nudges cached on `Veteran`; `--timings` on `--battle 7500` in every sim step against a recorded baseline, and `--saga-bench` from step 7 |
| Names of the dead lost | Medium | Kept deaths hold `NameId`s; anyone named has their death kept; the biography test checks it |
| Lines repeat | Medium | Variants stepped by told-death index; composition; the bank-size lint; the golden test |
| Words drift from the facts | Low | "The manner, never the facts"; fact slots required per key; times only from `{daypart}`; the golden test checks the facts in each line |
| Saves break mid-milestone | Low | `SAVE_FORMAT` 2 at step 2, bumped again on any later layout change; M4 saves are refused by design, after the playtest |
| Scope creep (nemesis AI, memorial effects, feed volume, a blow-by-blow log) | Medium | Each is an open question in the design, defaulted to "not in M4c"; new ideas wait for M5 or M6 |

## Defaults chosen (change any)

1. **The step order** puts kill lines in the game at step 4, before steps 7 and 8, which change play by design. You can stop after step 4 for a look.
2. **Records and kill lines exist in every preset; effects only in runs.** The skirmish then shows kill lines, which helps tune the words.
3. **Notability:** tier A is captains, the champion and legends (and a legend's milestone kills); the bar is 40 + 15 per told line, with at most 10 tier-B lines a battle and 8 told deaths a tick. The weights are in the design's section 2.
4. **Legends:** fame 25, three battles and an epithet; three enlisted per faction; an aura of 8 tiles with −0.03 on the break point and +0.03 on the morale target; a death shock of 0.10, and 0.03 to foes.
5. **One new system slot:** progress slot 13, `saga::settle`, hourly, after `director::step`. Everything else lives in existing systems.
6. **`Service` is a sparse-set component,** so starting a record never moves a soldier's table row; the nudges the hot loops read are cached on `Veteran`.
7. **The golden words test uses its own fixture bank,** so editing the real bank never churns it.
8. **Step notes** go in `docs/m4c/`.
9. **Grudges and the Avenger deed** (open question 1) are built in step 8 only if you say yes.
10. **Battles get a number** at their first blow, so both sides' reports share a name, first blood and bar.
11. **Told deaths live in `Saga`,** read by the feed with its own cursor, never through `EventFeed`.
