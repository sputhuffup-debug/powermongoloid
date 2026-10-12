# M4c Plan: Saga

Status: **planned 2026-10-12 (revised the same day after review, and again the same day for your follow-up decisions), awaiting your go.** M4c starts after the M4 playtest (`docs/M4_PLAYTEST.md`) and comes before M4b.
Design: `docs/SAGA_DESIGN.md` (draft 2026-10-12; all your decisions are recorded there). Spec: `docs/SEED.md` § "M4c: Saga".

M4c gives combat its stories, and every run its oddities. Every death names its killer and how it happened. Notable deaths get a brutal line, or a mild one if you choose, and the rest are summed up in the battle's report. Every villager in a run has a quirk or two, and the pacing director lets small things go wrong. Soldiers keep a record, earn an epithet (flattering or not), and sometimes die in ways people retell. Captains and long-serving soldiers earn Brave, Shaken, Cruel or Beloved. A few become legends who steady the men near them, and whose death shakes an army, pauses your game and leaves a cairn. Weapons that kill someone who matters get names and change hands; households march together, grieve and avenge; feuds start at dice. Everything is told in one voice, deadpan dark. The end screen remembers it all, and a small record of each run is kept for M6's book.

**The milestone is bigger now.** Your follow-up (quirks, mishaps, mocking epithets and absurd deaths, named weapons, kin and moods) adds about half again to the first plan: the first plan's whole is about two thirds of this one. **A split is sensible:**
- **Part 1, "Deaths and oddities"** (steps 1–8, about 57%): kill lines in the game, quirks, mishaps, battle moments, deeds and epithets (honourable and mocking), absurd deaths and rescue records. It ends with a short playtest of its own. Every run is already different, and the feed already speaks.
- **Part 2, "Legends and kin"** (steps 9–15, about 43%): earned traits, legends, named weapons, kin, friends, rivals and moods, biographies and the end screen, the run record, docs and the full playtest.

You can stop after part 1 for a look, and part 2 can slip without leaving anything half-built.

The steps are ordered to deliver value early:
- **The kill lines you asked for reach the game at step 4,** about 27% of the way in.
- **Quirks (step 5) and mishaps (step 6) come next,** because they make every run different soonest. From step 6, no two runs read alike.
- **Steps that change how a run plays, by design:** 5 (quirks), 6 (mishaps), 8 (the Vain quirk's epithet effects), 9 (earned traits), 10 (legends), 11 (looting named weapons) and 12 (kin and moods). Each is gated by `bot_run`.
- **Steps 1 and 7 may shift a run slightly,** and are gated by `bot_run`. Step 1's rebel kill credit changes the player's kill count, which weights dominion boon offers. Step 7 makes a wiped army's `Battle` notable, which the pacing director sees.

## Requirements covered

| ID | Requirement | Where |
| --- | --- | --- |
| SIM-39 | Every death records who killed whom and how, absurd ends included; notable deaths are told, the rest counted | `sim_military::combat` (the strike loop, its battle bookkeeping and `casualties`), new `sim_military::saga` |
| SIM-53 | Service records, deeds and epithets, honourable and mocking | `sim_military::saga`, `sim_military::veterans` (`end_battles`), `sim_military::orders` |
| SIM-34 | Earned traits; the champion's picked traits stay as built | `sim_military::saga`, consumers in `soldiers`, `upkeep`, `combat` and `orders` |
| SIM-54 | Legends: rise, aura, death shock | `sim_military::saga`, `index` (`LegendIndex`), `soldiers`, `upkeep`, `combat` |
| SIM-55 | Quirks dealt per person, acting through their holder | new `sim_agents::quirks`, `sim_agents::ai`, `sim_agents::act`, `sim_military::veterans` (cached nudges), `soldiers`, `upkeep`, `spawn`, `progression::setup_run` |
| GP-30 | Mishaps from the pacing director | `sim_military::director`, new `sim_military::mishaps`, `sim_military::saga` (`Mood`) |
| SIM-56 | Named weapons and their fates | `sim_military::saga`, `combat` (`casualties`), `orders` (`arm_followers`, barter, `equip`, `enlist`), `upkeep` (`apply_desertions`) |
| SIM-57 | Kin, friends, rivals and moods | `sim_military::saga`, `combat`, `veterans` (`end_battles`), `soldiers` (the vengeance aim), `mishaps` (the brawl) |
| UI-10 | Phrase banks in two tones and one voice; tone as a viewer setting | new `assets/data/saga.ron`, `sim_military::chronicle` (the `Teller`), `sim_military::saga` (words), `game_ui`, `game_app` (preferences) |
| UI-09 | Battle-report moments; biographies of people and settlements | `sim_core::events` (`BattleReport`), `sim_military::saga` (words), `game_ui` panels and end screen |
| RND-09 | Legend marks, cairns, memorial stones | `game_render` |
| GP-29 | The run record for M6's legends book | `sim_military::saga::run_record`, `game_app` (writes it), `sim_headless` (`--legends-out`) |
| NFR-09 | The saga's cost in big battles and on the run island | `sim_headless --timings` (`--battle 7500` with and without `--saga-bench`, and `--run`), `cargo bench -p sim_headless` |

**Not in M4c:**
- **M4b:** heroes (SIM-51, SIM-52), which may reuse the legend aura; hero memorials raised by saved villages (SIM-58), which read M4c's rescue records; animals with names (which make Fears geese literal); weather quirks such as Hates rain; stale food in mood notes; weapon kinds and named injuries, which come with the equipment tiers; and healing.
- **M5:** villagers' memories of the dead (SIM-14), prisoners and executions (GP-14's dilemmas), and tall tales and rumours about you (faction knowledge).
- **M6:** the legends book itself, and the options screen. M4c pulls forward only the tone switch.
- **Not adopted:** nemesis AI. Open question 1 in the design adds only words and a deed.

## Acceptance criteria and how each is tested

| Criterion | Test |
| --- | --- |
| Every death by a blow names its killer and how it happened, and a rebel's kill is credited to the rebels | **Test `kill_records_name_the_killer`** (sim_military unit, hand-built world): a killing blow records victim, killer, credited side and one-blow. A second swing at the dead records nothing. The manner bits (armed, fleeing, last stand) are set in `casualties`. **Scenario `rebels_kill_for_themselves`** (`tests/tests/saga.rs`, run preset): a conquered village with a small garrison revolts. Every rebel kill counts for faction 0 in `ArmyStats`, the garrison's report lists faction 0 among its foes, and a champion slain by rebels is told as killed by the villagers. |
| Captains, the champion and legends are always told; any other death only if it clears its battle's rising bar, at most 10 a battle and 8 told deaths a tick; the feed at 8x shows only lines about your people or a legend; the report's "more died" matches | **Scenario `notable_deaths_follow_the_rules`** (1,000 a side, seed 42, 3,000 ticks). Every captain's death is told. Per battle, at most 10 tier-B deaths are told, and at seed 42 the big battle tells between 4 and 10. No tick tells more than 8 deaths unless they're all tier A. Every told line raised the bar. Told plus untold equals deaths, and every named person's death is kept. Each report's "more died" equals its losses minus its named dead. **Scenario `battles_share_a_number`:** two armies meeting get one battle number, place and ordinal; a third army striking one of them joins it. **`bot_run`:** over every seed, at most 8 deaths a battle pass `saga::feed_filter` at 8x. |
| In a run, every person has one or two quirks from their own stream; each acts only through its holder; nobody without one changes | **`tests/tests/quirks.rs`:** `quirks_are_dealt_per_person`, `quirks_act_through_their_holder` and `sweetheart_walks_home`, as in step 5. `village_day` and `famine` pass unchanged. |
| The director's mishaps come from real state, do what their lines say, never touch the champion, a captain or a legend, never kill more than the rules allow, never empty a baggage or delay a march toward a fight, and never reset the quiet clock | **`tests/tests/mishaps.rs`:** `each_mishap_does_what_it_says`, `mishaps_never_wipe_a_run` and `mishaps_are_not_notable`, as in step 6. **`bot_run`:** the guards hold over every seed; mishap counts by kind per seed in the step notes. |
| Battle reports name first blood, who ran first, when the army broke, the told deaths, the best killer and anything odd; in a run, an army killed to the last man still gets its report | **Scenario `moments_reach_the_report`** (two squads of 20, seed 42): both armies' reports carry one battle number and name, and the same first blood. Who ran first is the soldier whose rout set the turning point. The broke tick is the first with 30% of the living members routing. The best killer is the top killer even if killed later. **Scenario `wiped_army_gets_a_report`** (run preset, a band of 5 against 40): a report marked wiped, with a `Battle` entry. |
| In a run, deeds earn one epithet by rule, honourable or mocking, and a mocking one never replaces an honourable one; absurd deaths are told only for causes the sim produced; answered drive-off requests record their heroes | **`tests/tests/saga.rs`:** `deeds_are_recorded_once`, `epithets_follow_rank`, `mocking_epithets_follow_rank`, `absurd_deaths_are_real` and `rescues_are_recorded`, as in step 8. |
| In a run, Brave, Shaken, Cruel and Beloved are earned from experience; each acts only through its holder (their own morale, aim and desertion, and a captain's own dealings), and anyone without one behaves exactly as before | **`tests/tests/saga_traits.rs`:** `brave_and_shaken`, `cruel_and_beloved`, `picked_and_earned_are_separate` and `earned_traits_are_exact_no_ops`, as in step 9. |
| Legends rise by fame, at most three enlisted per faction; they steady men within 8 tiles; their death shakes their army once and pauses your game for your own | **`tests/tests/legends.rs`:** `legend_rises_at_fame`, `legend_steadies_men` and `legend_death_shakes_once`, as in step 10. **A `game_ui` test** (`legend_fell_pauses_for_your_own`): the feed pauses for your legend's `LegendFell` and not for an enemy's. **`bot_run`:** still wins 3 of 5 seeds, and a player legend rises on at least 3 of them. It prints legends, epithets and earned traits per seed. |
| A weapon that kills a captain, the champion, a legend or a monument's beast is named, and is always in exactly one place as it is looted, passed on, put in the baggage, sold, taken home or lost | **`tests/tests/weapons.rs`:** `a_weapon_is_named_by_a_notable_kill` and `named_weapons_change_hands`, as in step 11. |
| Kin grieve and avenge; friends and rivals form by rule and feuds end; moods are bounded, expire and act only through their holder; the inspector explains a soldier's mood | **`tests/tests/kin.rs`:** `kin_grieve_and_avenge`, `friends_and_rivals_form_by_rule`, `moods_are_bounded_and_expire` and `moods_are_exact_no_ops`, as in step 12. A `game_ui` unit test of the mood note's wording from a hand-built record. |
| Brutal and mild banks have the same keys; every line names its key's facts and uses only its key's slots; no gendered words; the deadpan lint passes; feed lines fit the feed; the tone never changes the simulation | **`tests/tests/saga_words.rs`:** a lint of the real `saga.ron` (with the voice rules) and a golden test on a small fixture bank (both in step 3, extended as banks are added). **Test `tone_is_not_state`:** a bot run's chronicle, told in both tones, reads differently, while its state hash and a recorded replay are identical. |
| The feed stays readable at 8x | **`bot_run`:** over every seed, at most 8 deaths a battle and at most 2 mishap, weapon, kin and feud lines a game day pass the 8x filter. **A `game_ui` test** that the throttle drops tier-B and saga-note lines at 8x and folds the overflow. |
| The inspector shows the biographies of soldiers and settlements; the end screen lists legends, foes remembered, the fallen and the run's oddities; cairns mark where legends fell | **Unit tests** of the biography words: a dead legend, a living soldier, a spawned soldier and a settlement, in both tones; nobody named is ever told as "a soldier". **Screenshots** of each surface in the step notes. |
| A finished run writes its legends record, and no sim crate reads it | **Test `run_record_lists_legends`** (`tests/tests/saga.rs`): the bot plays seed 1 to the end of its run. Its record round-trips through RON, names every legend and gives each a fate, lists its named weapons and rescues, with names written as strings. An abandoned run writes one with outcome `Abandoned`. |
| NFR-09 holds; developed, skirmish and `--battle` stdout are unchanged; saves and determinism hold | `--battle 7500 --timings --stats-every 250` and, from step 9, the same with `--saga-bench`; from step 5, `--run --seed 7 --difficulty steady --ticks 24000 --timings`. For each, the median of three runs of the mean tick rises at most 2% over the M4 baselines recorded at step 1. The bench. The stdout comparisons in "What stays unchanged". **`save_load`:** records, quirks, moods, bonds, weapons, legends and moments round-trip, `every_hashed_type_is_saved` covers the new types, and a format-1 save is refused. The determinism tests, single- and multi-threaded. |
| **Playtest:** you can name a legend after a run and say how they fell; a death landed; a mishap or an odd death made you laugh or wince, and none felt unfair; brutal reads grim, not numbing; mild reads sober, not dry; the deadpan reads dry, not silly | You play two runs, one in each tone (checklist `docs/M4C_PLAYTEST.md`; part 1's checklist is written in step 8, the whole in step 15). |

## What stays unchanged

- **Stdout of the guarded commands.** Each step compares these release commands with the previous step's binary, and the stdout must be byte-identical. State hashes and timings, on stderr, may change.
  - developed: `sim_headless --seed 42 --ticks 4801 --stats-every 600`
  - skirmish: `--skirmish --seed 42 --ticks 6000 --stats-every 2000`
  - battle: `--battle 1000 --seed 42 --ticks 3000 --stats-every 600`
- **The M2 and M3 tests** pass unchanged, with today's tuning: `village_day`, `famine`, `army_march`, `posture`, `starving_army` and `orders`. Every saga effect is run-only, and is an exact no-op without a trait, quirk, mood, bond or legend. Outside a run nobody has a quirk, so the villager AI scores exactly as today. `villagers.ron` gains `quirks` and `director.ron` gains mishap sections: their data text is hashed, so the developed world's state hash changes once at steps 5 and 6, and its behaviour doesn't.
- **Replays.** The replay format (`REPLAY_FORMAT` 5) and the command kinds don't change: M4c adds no command, and the tone is never an input.
  - Old replay files still parse, but their checkpoint hashes stop matching after step 2, as after every M4 step. Step 1 already changes those in which rebels fought. Re-record any you want to keep.
  - In every step, a fresh run replay records and verifies, multi- and single-threaded.
- **Pacing.** The director's quiet clock sees only `LegendFell` and a wiped army's `Battle` as new notable events. Mishaps are chronicled but not notable, and fast-forward stops on nothing else new. The raider schedule is unchanged by mishaps (tested in step 6).
- **`FactionModifiers` is untouched.** Nothing personal goes into it, and the champion's picked traits still act faction-wide.
- **The chronicle's existing entries keep their words, with two exceptions.** The `Battle` line gains faction nouns ("against the rival's band" instead of "against Rival") and the battle's name (steps 3 and 7), and `RequestDone` names the hero of a rescue (step 8). `chronicle.causes` gains one phrase.
- **`populate`'s draws are untouched** (`docs/m4/BUILD_GUIDE.md` G10): quirks come from their own stream per SpawnId, after population.
- **Saves are the one intended break.** From step 2 (`SAVE_FORMAT` 2), M4 saves are refused. That's why M4c follows the playtest.

## The gate (every step)

1. **Build and lint:** `cargo fmt --check`; `cargo clippy -j 2 --workspace -- -D warnings`; `cargo test -j 2 --workspace`; `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps`.
2. **Determinism:** the determinism tests, plus a run replay. Record `--run --seed 42 --ticks 3000 --commands 50 --record run.ron`, then verify it with `--replay run.ron` and `--replay run.ron --single-threaded`.
3. **Invariance:** the three stdout comparisons above.
4. **Cost, for sim steps:** `--battle 7500 --seed 42 --timings --stats-every 250`, three runs, against the baseline in `docs/m4c/` (NFR-09). From step 5, also `--run --seed 7 --difficulty steady --ticks 24000 --timings`, three runs, against its baseline. From step 9, also `--battle 7500` with `--saga-bench`.
5. **Docs in the same commit** (CLAUDE.md "Developer handbook"):
   - `docs/dev/DATA.md` for data;
   - `docs/dev/ARCHITECTURE.md` for systems and slots;
   - `docs/dev/TESTING.md` for tests;
   - `docs/dev/GLOSSARY.md` for terms;
   - `docs/dev/HOWTO.md` for recipes others will repeat;
   - ADR 0019 for decisions.
6. **Balance, for steps marked bot_run:** `cargo test --release -j 2 -p integration_tests --test bot_run -- --ignored` still wins 3 of 5. Record the win days and saga counts in the step notes: legends, epithets (honourable and mocking), earned traits, quirks, mishaps by kind, ironies, weapons named and their fates, feuds, and desertions by cause, as each exists. Change data when the game is unbalanced, and the bot when it plays badly (HOWTO "Tune run balance").

Step notes go in `docs/m4c/`, as M4's went in `docs/m4/`.

## Build order

Sizes are shares of the enlarged milestone: S is up to 3%, M about 6–8%, L about 10–11%. The first plan's steps keep their relative sizes, scaled to two thirds.

### Part 1: Deaths and oddities

### Step 1: Kill attribution and the rebels' credit (S, 3%; bot_run)
- **What.**
  - **The baselines first.** Three runs each, recorded in `docs/m4c/baseline.md` with the spread between runs:
    - `--battle 7500 --seed 42 --timings --stats-every 250` on the M4 build: deaths a tick, the clash window's mean tick, and the Act, Decide and Needs stage times;
    - `--run --seed 7 --difficulty steady --ticks 24000 --timings`: the mean tick and the Decide and Needs stage times on the run island, where 4,000 to 5,000 villagers live.
  - `Casualties.slain_by` becomes `kills`, a list of `Kill`: victim, killer, the credited side, and one blow (the victim was at full health).
  - A rebel's kill is credited to the independents, matching how `casualties` already treats rebel victims. A rebel is militia whose `against` is its own allegiance. The credit applies in `ArmyStats.kills`, in the garrison's `battle_foes`, and in the champion's cause of death.
- **Where.** `crates/sim_military/src/combat.rs` (`Casualties`, the kill branch of `resolve_strikes`); `crates/sim_military/src/progression/champion.rs` (`cause_of_death` reads the kills through a small helper); `crates/sim_military/src/veterans.rs` (its unit test asserts on the scratch); `crates/sim_military/src/save.rs` (`after_load` clears the scratch).
- **Tests.** `kill_records_name_the_killer`; `rebels_kill_for_themselves` (new `tests/tests/saga.rs`). Re-run `champion` and `conquest`.
- **Docs.** ARCHITECTURE schedule row 5 (Act): the kill scratch. TESTING rows. ADR 0019 is opened with the kill record and the rebel fix.
- **Done when.** Every kill names its killer, rebels' kills are theirs, both baselines are recorded, and nothing else moves.
- **Gate note.** The player's kills weight the dominion boon offers, so the fix can change a run: re-run `bot_run`.
- **Ships as:** a fix. When rebels fight, the run summary's kills and the champion's cause of death come out right.

### Step 2: Battle numbers, service records and told deaths (L, 10%)
- **What.**
  - **Battle numbers.** The strike loop's battle bookkeeping, in striker SpawnId order, joins two armies into one battle: an army with a number keeps it, one without takes the other's or a new `Saga.next_battle`. A militia blow joins the army it fights. `Captain` gains `battle`, which passes to an heir with `battle_since`. The first blow fixes the battle's place (nearest settlement), ordinal there and terrain, in `Saga`.
  - **Broke.** `build_index` sets `Captain::battle_broke` at the first tick with `saga.broke_share` (0.3) of the army's living members routing, beside `battle_rout`.
  - **`Veteran` gains three fields:**
    - `routed`: ran in this battle, set in `soldier_upkeep` on the soldier's own component;
    - `hurt`: the share of health lost to blows this battle, a saturating byte added in the strike loop on the victim's `Veteran`, which is already borrowed;
    - `trade`: the occupation before enlisting, captured in `enlist`.
  - **New module `sim_military::saga`:**
    - `Service`, stored as a sparse-set component so that inserting one never moves a soldier's table row. It has the fields of the design's section 6: trade, home, kills, streak, battles and wounds here, and the rest from steps 6, 8 and 9;
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

### Step 3: Words, tone and the voice (M, 8%)
- **What.**
  - **`assets/data/saga.ron`**, embedded at compile time like `worldgen.ron`. The brutal and mild banks share one type, so a key missing from one tone fails to parse. Each key declares its fact slots and flavour slots.
  - **`SagaWords`**, exempt from the hash ("words only; the sim never reads it") and not saved.
  - **`Tone`** as a parameter of `describe`, `describe_entries` and the new words functions. Game call sites pass Brutal until step 4.
  - **The `Teller`:**
    - names the dead from `Saga`'s kept deaths;
    - picks keys most specific first, with militia and rebel victims always on their grave keys and first blood or a streak carried by `{moment}`;
    - fills the slots: the per-person weapon word, the old trade's tool, ground by terrain, `{daypart}` and `{then}` from ticks, beast nouns and faction nouns;
    - chooses variants by told-death index, never by RNG;
    - names battles from their number's facts ("the Second Battle of Loweby Fields").
  - **The voice** (the design's section 4): every bank is written deadpan dark, and DATA.md carries its do and don't table.
  - **Headless:** `--saga` prints told deaths and legends after the run, or with `--chronicle`; `--mild` switches the tone.
- **Where.** `assets/data/saga.ron` (new); `crates/sim_military/src/chronicle.rs`; `crates/sim_military/src/saga.rs` (words); `crates/sim_headless/src/main.rs` and `crates/sim_headless/src/stats.rs`; the `describe` call sites in `crates/game_ui/src/`.
- **Tests.**
  - **`saga_words` lint (`tests/tests/saga_words.rs`):**
    - the same keys in both tones;
    - every line uses only its key's slots, and names every fact slot of its key in both tones (mild may omit flavour slots);
    - at least 6 brutal and 4 mild variants, and every bank shorter than 37;
    - no he, she, his, her, him or hers, and none of the design's gendered nouns;
    - a short deny-list of content;
    - **the voice:** no `!` or `?`, no word in capitals, no ellipsis outside the fold line, and none of the design's winking and pitying words;
    - every line that can reach the feed, filled with the longest given name, family name, epithet, weapon name and place in the data, fits 72 characters (one feed row after the "Day N: " prefix); report-only lines fit 110;
    - every epithet id, deed and earned-trait id has words in both tones (and, as later steps add them, every quirk, mishap kind, irony, mood cause and note kind).
  - **`saga_words_golden`:** a fixture bank inside the test and a scripted duel give exact lines in both tones, so edits to the real bank don't churn it.
  - **`tone_is_not_state`.**
  - Re-run `chronicle`.
- **Docs.**
  - DATA.md: a `## saga.ron` section covering every field, with the content rules (graphic toward fighters, grave toward militia and rebels, never sexual violence, torture as spectacle, slurs or children; no gendered words; words may invent the manner, never the facts; times only from `{daypart}` and `{then}`) and the voice's do and don't table.
  - HOWTO: a recipe, "Add a kill line, an epithet or an earned trait", and the voice in one paragraph.
  - GLOSSARY: Phrase bank, Tone, Fact slot, Deadpan dark.
  - ADR 0019: the words-only file, the tone as a viewer setting, and the one voice.
- **Done when.** These read well in both tones and the lint passes:
  - `sim_headless --skirmish --ticks 6000 --saga`;
  - `sim_headless --run --bot --seed 1 --ticks 24000 --chronicle --saga`.
- **Ships as:** kill lines in headless.

### Step 4: Kill lines in the game (M, 6%) — the first slice you can play
- **What.**
  - **Feed lines for told deaths,** read from `Saga` with the feed's own cursor (a `Local`, like the event cursor), so they never crowd `EventFeed`. The same cursor will read saga notes from step 11.
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
- **Ships as:** **the brutal kill messages you asked for**, in the skirmish and in runs. A natural point for a quick look.

### Step 5: Quirks (M, 7%; run only; bot_run)
- **What.**
  - **New module `sim_agents::quirks`:** the `Quirks` component (one or two ids, two bytes), the deck from `villagers.ron` `quirks` (`two_chance`, rows of id, key, weight, `excludes` and effects; `#[serde(default)]`), and `deal`, drawing from `SimSeed::stream(rng_key("sim_agents::quirks") ^ SpawnId, 0)`. A deal depends only on the seed, the deck and the SpawnId.
  - **Who is dealt, in a run only:** everyone alive at the end of `setup_run` (after `sellswords::setup`, the last setup spawn), and the people `spawn_army` spawns later in the run. Never beasts; Sweetheart is skipped for anyone without a home. `populate` is untouched (G10).
  - **Villager effects** in `ai::decide` (work by phase, farm and pasture work, idle by phase, militia courage, flee radius), `update_needs` (hunger, rest gained asleep), `drift_loyalty` (Pious) and the recruit dealing's willingness (Sweetheart, compared against the same draw as today). Each is skipped entirely when the villager has no `Quirks`. `Factors` grows from four slots to five, so a quirk shows as its own factor in the `DecisionTrace`.
  - **`Veteran` gains the cached nudges** (break-point offset, morale-target offset, desertion multiplier, hunger multiplier, and the glutton's fed-morale rule), written only by serial steps: the deal, and `enlist` (rebuilt from `Quirks`). `soldiers::decide` reads the break point; `soldier_upkeep` reads the rest. Earned traits (step 9) and moods (step 6) write the same cache later. The sums are clamped as the design's section 8 says.
  - **New progress slot 13, `saga::settle`** (hourly, every preset, after `director::step`). Here it runs the Sweetheart's walk home (out of battle, within 12 tiles of home: 5% a day, from `sim_military::saga::homesick ^ SpawnId` at the tick, through the ordinary desertion step).
  - **Acting from later steps:** Fears geese's near-farm mood and Pious's plunder mood need moods (step 6); Vain needs epithets (step 8); the mishap invitations need mishaps (step 6).
  - **Inspector:** the quirk line with tooltips, for villagers and soldiers.
  - **`bot_run`** prints quirk counts per seed.
- **Where.** `crates/sim_agents/src/quirks.rs` (new), `crates/sim_agents/src/ai.rs`, `crates/sim_agents/src/act.rs` (`update_needs`), `crates/sim_agents/src/loyalty.rs` (`drift_loyalty`), `crates/sim_agents/src/components.rs` (`Factors`), `crates/sim_agents/src/data.rs`, `crates/sim_agents/src/lib.rs` (registration); `assets/data/villagers.ron`; in `crates/sim_military/src/`: `progression/mod.rs` (`setup_run`, slot 13), `spawn.rs`, `veterans.rs` (the nudges), `soldiers.rs`, `upkeep.rs`, `orders.rs` (`enlist`), `saga.rs` (`settle`); `assets/data/saga.ron` (quirk words); `crates/game_ui/src/panels.rs`.
- **Tests (`tests/tests/quirks.rs`).**
  - `quirks_are_dealt_per_person`: the same seed deals the same quirks; dealing all at once, in reverse order or at spawn gives the same result; one or two each; exclusions hold; beasts, the developed world, the skirmish and `--battle` have none.
  - `quirks_act_through_their_holder`: each row's effect on a hand-built world (a glutton's hunger 1.15× exactly, a coward's break point, a drunkard's morning work score, and so on), and a person without quirks bit-identical to today.
  - `sweetheart_walks_home`: near home, out of battle, the walk home happens at the documented rate over many hours; far from home, or in battle, never.
  - Re-run `village_day`, `famine`, `army_march`, `posture`, `starving_army` and `orders`; `save_load`; the determinism coverage tests.
- **Docs.** DATA.md (`villagers.ron` `quirks`, and the quirk words); ARCHITECTURE: slot 13, the deal in setup and spawn, the cached nudges; GLOSSARY: Quirk, Cached nudges; TESTING; HOWTO: "Add a quirk"; ADR 0019: quirks dealt per SpawnId, and the amendment to "no new RNG" with the stream keys.
- **Done when.** Every person in a run has one or two quirks, the same for a seed whatever the order; each effect is measured; the guarded stdout is unchanged; `village_day` and `famine` pass unchanged; the run-island cost is within NFR-09; `bot_run` still wins 3 of 5.
- **Ships as:** villagers who differ. The inspector shows their quirks, and they act on them.

### Step 6: Mishaps (M, 8%; run only; bot_run)
- **What.**
  - **The mishap clock** in `director::step` (slot 12), after the raider check: nothing in the first 24 hours or while a run end is pending; `spacing_hours`, `chance_per_hour`, `max_per_day` and `tension_below` per personality; the battle kinds ignore tension and are capped at one per battle; the player's armies weigh 2 and armies at war with the player 1; beasts, guards and bands for hire are never subjects.
  - **New module `sim_military::mishaps`** with the kinds of the design's section 11, except the camp brawl (step 12): drunk before battle, bad mutton, a dog at the rations, a wedding on the road, a slip in the shield wall, the victory feast, wandered off, dice, and the goose rout. Each has its condition, its effect, and its guards, enforced in code: never the champion, a captain or a legend as subject; deaths only from the slip and the feast, one ordinary soldier, in armies of 10 or more, one a game day; food never below a day of the army's need; delays only for marches with no foe near; one mishap a day per army.
  - **`Mood`** (sparse): up to four timed entries, clamped, expired hourly in `saga::settle`, and written into `Veteran`'s cached nudges. Here it carries the mishap moods (`sick`, `hungover`, `wedding`, `feasted`, `dice`, `geese`) and the quirk moods that need it (Fears geese near farms, Pious after a plunder).
  - **`Captain::halt_until`**, checked by `run_orders` for a halted march.
  - **`SimEventKind::Mishap`**, chronicled and not notable, so the quiet clock, the run's first event and fast-forward don't see it.
  - **A mishap's death** sets health to 0 and notes its manner in `Saga`; `find_fallen` finds it on the next tick (G4), and `casualties` tells it on its `mishap_*` key as a tier-A `Fall`.
  - **The feed:** mishap lines, dimmer; at 8x only those that killed one of yours or befell your champion's army; at most one every 20 real seconds at any speed.
  - **Inspector:** the moods a mishap left, with their time to run.
  - **`bot_run`** prints mishaps by kind per seed and checks the guards over every seed.
- **Where.** `crates/sim_military/src/director.rs`, `crates/sim_military/src/mishaps.rs` (new), `crates/sim_military/src/saga.rs` (`Mood`, `settle`), `crates/sim_military/src/orders.rs` (`halt_until`), `crates/sim_military/src/components.rs` (`Captain`), `crates/sim_military/src/combat.rs` (mishap deaths), `crates/sim_military/src/upkeep.rs` and `crates/sim_military/src/veterans.rs` (moods in the nudges), `crates/sim_core/src/events.rs` (`Mishap`, `ALL`, `name`, `chronicled`, `notable`); `assets/data/director.ron`; `assets/data/saga.ron`; `crates/game_ui/src/feed.rs`, `crates/game_ui/src/panels.rs`.
- **Tests (`tests/tests/mishaps.rs`).**
  - `each_mishap_does_what_it_says`: each kind on a hand-built world, with its condition false (nothing happens) and then true (its effect, exactly).
  - `mishaps_never_wipe_a_run`: over many hours of seeded runs, and again in `bot_run` over every seed: no champion, captain or legend touched, at most one death per mishap and one a game day, no baggage under the floor, no delay with a foe near, at most `max_per_day`, none in the first 24 hours.
  - `mishaps_are_not_notable`: a run with mishaps and the same run with an empty kind list have the same quiet clock, first event and raider schedule.
  - Re-run `director`, `chronicle` and `save_load`.
- **Docs.** DATA.md (`director.ron` `mishaps`, `mishap_rules`, `mishap_kinds`; the mishap banks); ARCHITECTURE: slot 12's mishap clock, slot 13's mood expiry; GLOSSARY: Mishap, Mood, Low-tension beat; TESTING; HOWTO: "Add a mishap"; ADR 0019: mishaps aren't notable, the never-run-ending rules, the stream keys.
- **Done when.** Bot runs show about one mishap a game day on steady, with none breaking a guard over every seed; each kind's line matches its effect; `bot_run` still wins 3 of 5; the quiet clock and raids are unchanged by them.
- **Ships as:** runs that go wrong in small ways, and say so.

### Step 7: Battle moments (M, 7%; bot_run)
- **What.**
  - **`BattleReport` gains the moments:** `battle` (the number, which leads to the name, first blood, the told deaths and the odd in `Saga`), `broke`, `drew_first`, `best`, `ran_first`, `hurt` (the wounded), `deserted`, `wiped`, and `earned` (empty until step 8).
  - **`Captain` gains** `battle_ran_first`, `battle_deserted` and `starved`.
  - **`build_index` names who ran first.** It already finds the moment.
  - **`apply_desertions` counts deserters** per army.
  - **`end_battles`:**
    - counts a wound for each survivor whose `hurt` reached a quarter of their health;
    - fills the report;
    - clears the new fields with `fought`.
  - **Slot 13 (`saga::settle`)** gains the hourly starvation summary `Starved`, feed-only.
  - **A wiped army in a run:** when no heir is left, `succeed` writes the report (`wiped`) and its `Battle` entry.
  - **Words:** the report's moments, "N more died", the odd (a mishap during the battle), the `Battle` line's battle name and faction nouns, the end screen's "Your battles", and the feed's battle line opening into the full report.
- **Where.**
  - in `crates/sim_core/src/`: `events.rs` (`BattleReport`, its hash and save; the `Starved` kind, `ALL`, `name`, `chronicled`);
  - in `crates/sim_military/src/`:
    - `index.rs`, `upkeep.rs`, `veterans.rs`;
    - `combat.rs` (`succeed`);
    - `saga.rs` (slot 13);
    - `chronicle.rs`;
  - in `crates/game_ui/src/`: `feed.rs` and `screens.rs`.
- **Tests.** `moments_reach_the_report`, `wiped_army_gets_a_report`, `starving_is_told_once_an_hour`, the event order test. Re-run `chronicle`, `director` and `save_load`.
- **Docs.**
  - ARCHITECTURE: row 6 (slot 13's starvation lines) and row 2 (who ran first).
  - GLOSSARY: Moment.
  - TESTING.
  - ADR 0019: the wiped report amends ADR 0017's "as built".
- **Gate note.** A wiped army's `Battle` is notable, so pacing can move: re-run `bot_run`.
- **Done when.** Every report tells its moments in both tones, both sides' reports of a fight share its name, and a wiped army is reported in a run.

### Step 8: Deeds, epithets, absurd deaths and rescues (M, 8%; run only; bot_run) — the end of part 1
- **What.**
  - **Deeds** (`SagaDeed`) are recorded once each, as (deed id, tick, battle number or nearest settlement, victim `NameId`):
    - at the kill: first blood, slayings, shield, five in a fight, doorwarden, kinslayer;
    - at `end_battles`: held the line, survivor, ran first, last stand (a kill after the army broke, and alive at the end);
    - in `run_orders`, on the captain: plunderer, breadgiver;
    - in `requests::complete`: rescuer.
  - **The rest of `Service`:** `joined`, `rejoined`, `held`, `fled`, `mishaps` (per kind), `fights` (the latest 16 battles with their kills).
  - **Fame** is worked out from the record.
  - **Epithet rows** (`saga.epithets`: a stable `id`, `kind` honour or mock, a `rank` and a rule): ids 0–18 honourable at ranks 8–26, ids 19–26 mocking at ranks 0–7. They're applied by one function, `saga::judge`:
    - from `end_battles`, for each survivor, so the report lists what was earned;
    - from slot 13, for every other record changed since the last hour (militia after a kill, captains after dealings, anyone after a mishap or a re-enlistment).
  - **Ironies** (`saga.ironies`): ids 0–6 and 8–9 of the design's section 7, classified in `casualties`' second pass from the records, never for militia, rebel or villager victims; +50 to the score; the `absurd_*` keys and the `{irony}` slot. Irony 7 (own weapon) comes with step 11.
  - **Rescues:** an answered `DriveOff` records its rescuers and heroes in `Saga.rescues`, the heroes earn the Rescuer deed, and the `RequestDone` line names the hero.
  - **Vain acts now:** the morale with an honourable epithet, `envy` when a comrade is named, `mocked` on a mocking epithet.
  - **`Named`**, chronicled but not notable, with the new `Cause::Earned { deed }`. `notable()` excludes the new chronicled kinds.
  - **Inspector:** the epithet after the name, then kills, battles, wounds and deeds.
  - **Part 1's playtest checklist** (the first half of `docs/M4C_PLAYTEST.md`), so you can stop here for a look.
- **Where.**
  - `crates/sim_military/src/`: `saga.rs`, `veterans.rs`, `orders.rs`, `combat.rs` (ironies), `requests.rs` (rescues);
  - `crates/sim_core/src/events.rs`: `Named`, `Cause::Earned`, `notable`;
  - `assets/data/progression.ron`: deed fame, epithet rows, ironies, the `chronicle.causes` phrase;
  - `assets/data/saga.ron`: epithet, deed, irony and rescue words, keyed by id;
  - `crates/game_ui/src/panels.rs`.
- **Tests.**
  - `deeds_are_recorded_once`, with their place and victim;
  - `epithets_follow_rank`: Nine-Kills, then a captain slain, replaces it; a lower rank never replaces a higher; re-ranking the rows keeps every stored id; militia earn Doorwarden at the kill;
  - `mocking_epithets_follow_rank`: each mocking rule from a scripted record; Twice-Fled is replaced by Banner-Taker and never the reverse;
  - `absurd_deaths_are_real`: each irony from a scripted state; none for a militia victim; the lint finds no `absurd_*` key whose irony the table lacks;
  - `rescues_are_recorded`: an answered drive-off names a legend first, then the named by fame, then the best killer; the captain when nobody fought named;
  - `every_cause_is_worded`;
  - the event order test;
  - `chronicle` and `director`;
  - `bot_run` prints epithet counts (honourable and mocking) and ironies; Vain's morale may move it.
- **Docs.** DATA.md; GLOSSARY: Saga deed (kept distinct from goodwill's **Deed**), Fame (a per-person score with no unlocks, not FUN_DESIGN's renown), Epithet, Mocking epithet, Irony, Rescue; TESTING; ADR 0019.
- **Done when.** A bot run of seed 1 earns several epithets, some mocking, each with its reason and place; at least one absurd death appears over the five seeds; answered drive-offs name their heroes; the inspector shows the record.
- **Ships as:** names you notice, flattering or not, and deaths people retell. **Part 1 is done:** tick its criteria in SEED.md and hand over part 1's playtest.

### Part 2: Legends and kin

### Step 9: Earned traits (M, 6%; run only; bot_run)
- **What.**
  - **Marks** count only once a person is eligible (a captain, the champion, a legend, or three battles survived), and a battle gives at most one of each kind:
    - held and broke at `end_battles`;
    - cruelty at the kill (a fleeing victim, a kinslaying) and in `run_orders` (plunder);
    - kindness in `run_orders` (food to a short village, a bring-food request) and at `end_battles` (held at home).
  - **Cruel** is appended as trait id 7 and kept out of `champion_traits`.
  - **`saga.earned`** has one row per earned trait: the mark and count, the opposite, and the personal effects.
  - **`judge`** awards at most one trait per battle. Opposites replace each other: Brave over Shaken records Redeemed. It adds the trait's nudges to `Veteran`'s cache (step 5), which `enlist` rebuilds from `Service`, `Quirks` and `Mood`.
  - **Picked and earned stay separate:** earned traits live only in `Service`; `draw_traits` ignores them; succession never moves them; an earned trait the champion already picked isn't awarded again.
  - **`TraitEarned`** is a new kind, chronicled and not notable (today's `TraitGained` is notable and stays for picks).
  - **Consumers through the holder, run-only, exact no-ops without the trait:**
    - the break point in `soldiers::decide` and the morale target and desertion in `soldier_upkeep`, through the cache;
    - Cruel's aim at a fleeing foe in `aim`;
    - Cruel's extra kill morale in `casualties`' first pass, so the strike loop is untouched;
    - a Cruel captain's plunder goodwill and a Beloved captain's recruit chance, in that captain's own dealings in `run_orders`.
  - **`--saga-bench`** for `--battle`: turns on the saga's run-only effects (not the rest of the run rules), gives every soldier a quirk and every tenth an earned trait; steps 10 to 12 add legends, weapons, friends and moods.
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

### Step 10: Legends (M, 6%; run only; bot_run)
- **What.**
  - **`Legend { faction, rose }`.** The rise rule and the cap are in `saga.legend`: fame 25, three battles, an epithet, and three enlisted per faction by current allegiance. A deserted legend keeps the title in words and frees the slot.
  - **`LegendIndex`**, exempt and rebuilt in `build_index` each tick in a run, before any reader. It holds only enlisted legends, and is the set the cap counts.
  - **The aura:** −0.03 on the break point in `decide`, and +0.03 on the morale target in `soldier_upkeep`. It works within 8 tiles, doesn't stack and only counts enlisted legends.
  - **The death shock,** in `casualties`: −0.10 once to the legend's army and to their faction's fighters within 12 tiles, +0.03 to foes within 12 tiles. A captain who is also a legend costs the larger of the two losses, not the sum.
  - **One event per death:** champion > legend > captain. `LegendRose` is chronicled, not notable; `LegendFell` is chronicled and notable.
  - **The pause:** `LegendFell` is added to every difficulty's `auto_pause` in `director.ron`, and the feed pauses only for your own.
  - **`--saga-bench`** gains three legends per army.
  - **Grudges and the Avenger deed,** only if you answer yes to open question 1. They're about 1% of the milestone.
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

### Step 11: Named weapons (M, 7%; run only; bot_run)
- **What.**
  - **`NamedWeapon(index)`** (sparse) on a holder, always with `Equipment.weapon` true, and **`Saga.weapons`**: the first namer's weapon word, the naming, up to 8 holders, kills while named, up to 8 notable kills, and the place.
  - **Naming** in `casualties`' first pass: an armed killer of a captain, the champion, a legend or a monument's beast names their weapon, at most 2 a battle and 24 a run (`saga.weapons`).
  - **Fates** in the second pass, the first that applies: looted (a soldier killer takes it in hand; a militiaman's village puts it in its storehouse), passed on (an unarmed member of the holder's household in the same army, else the nearest unarmed member within 3 tiles), into the baggage, or lost where its holder fell.
  - **Out of a hand:** a deserter takes it home (`apply_desertions`) and brings it back on re-enlisting (`enlist`); `arm_followers` and an `Equip` dealing hand named weapons out first; a barter spends them last, and one spent is sold into that settlement's storehouse.
  - **Reconciliation** in slot 13: a place holding fewer `weapons` units than the named weapons placed there marks the newest one lost.
  - **Irony 7** (killed with a weapon they once carried); `{weapon}` fills with the name; Vain's morale with a named weapon.
  - **Saga notes** (`Saga.notes`, read by the feed's own cursor): namings, passings, lootings, sales, losses.
  - **Inspector:** the weapon line and its history; the settlement panel's relics.
  - **`--saga-bench`** gains three named weapons per army.
- **Where.** In `crates/sim_military/src/`: `saga.rs`, `combat.rs`, `orders.rs` (`arm_followers`, `equip`, barter, `enlist`), `upkeep.rs` (`apply_desertions`); `assets/data/progression.ron` (`saga.weapons`); `assets/data/saga.ron` (name patterns, fate lines); `crates/game_ui/src/feed.rs`, `crates/game_ui/src/panels.rs`.
- **Tests (`tests/tests/weapons.rs`).**
  - `a_weapon_is_named_by_a_notable_kill`: a captain's killer names their weapon; an ordinary kill doesn't; an unarmed killer names nothing; the caps hold.
  - `named_weapons_change_hands`: looted by a soldier and by militia, passed to kin and to the nearest comrade, into the baggage and out first, sold last in a barter, taken home and back, lost when the army is wiped; after every move it's in exactly one place, and the places' `weapons` units cover it.
  - Re-run `orders` (barter and equip unchanged without named weapons), `save_load`.
- **Docs.** DATA.md; GLOSSARY: Named weapon, Saga note; TESTING; ADR 0019: a weapon's identity is a sparse index beside the `Equipment` bool.
- **Gate note.** Looting arms the looter, so a run can move: re-run `bot_run`, with weapons named and their fates per seed in the step notes.
- **Done when.** Bot runs name a handful of weapons, each with a history that matches what the sim did, and none is ever in two places.

### Step 12: Kin, friends, rivals and moods (L, 11%; run only; bot_run)
- **What.**
  - **Kin** through `Home.residents` (one household), never stored: in `casualties`' second pass, the death of a villager-recruit gives their enlisted kin `grief`, and those within 12 tiles **vengeance** for the rest of the battle (morale +0.05 once, and the aim at the killer in `soldiers::decide`); killing the killer is the Avenged kin deed and a told `vengeance` line. A militia or rebel killed by a blow of a soldier's own faction gives that soldier's enlisted kin `kin_killed` (−0.08, desertion ×2, 3 days).
  - **`Bonds`** (sparse, at most three): friends at `end_battles` (two held survivors within 2 tiles, in an army that broke or lost a quarter); rivals from dice (the mishap now leaves its grudge), from running first, and from a named weapon passed outside the household. Feuds end when both hold in one battle, when one dies, or in a brawl's one in three.
  - **The camp brawl** joins the mishap kinds.
  - **Bond effects** through the cache: friend desertion ×0.9, rival morale target −0.02.
  - **The full mood table** of the design's section 13, the clamps, replacement of the entry ending soonest, and hourly expiry.
  - **Mood notes** in the inspector ("Wat is upset: Hild Ashby, kin, died at Loweby, and the mutton was bad."); kin, friends and feud lines; the captain panel's "Mourning · Feuds".
  - **Saga notes** for friendships, feuds begun and ended, and kin killed; the feed shows `kin_killed` for yours at 1x–4x and feud and friendship lines at 1x–2x.
  - **`--saga-bench`** gains friends for every tenth soldier and moods for every fifth.
- **Where.** In `crates/sim_military/src/`: `saga.rs`, `combat.rs`, `veterans.rs` (`end_battles`), `soldiers.rs` (the vengeance aim), `mishaps.rs` (the brawl, the dice grudge), `upkeep.rs`; `assets/data/progression.ron` (`saga.bonds`, `saga.moods`); `assets/data/director.ron` (the brawl's row); `assets/data/saga.ron`; `crates/game_ui/src/panels.rs`, `crates/game_ui/src/army.rs`, `crates/game_ui/src/feed.rs`.
- **Tests (`tests/tests/kin.rs`).**
  - `kin_grieve_and_avenge`: grief, the vengeance aim within 3 tiles of the nearest foe, the deed and the line; `kin_killed` when your own army kills a recruit's household in their village.
  - `friends_and_rivals_form_by_rule`: each source, the caps, and each way a feud ends.
  - `moods_are_bounded_and_expire`: the clamps, replacement, expiry, and the cached nudges matching the entries after every change.
  - `moods_are_exact_no_ops`: no bonds and no moods give bit-identical morale and desertion.
  - A game_ui unit test of the mood note's wording.
  - Re-run `army_march`, `posture`, `starving_army`, `orders` and `save_load`.
- **Docs.** DATA.md; ARCHITECTURE: what `casualties`, `end_battles` and slot 13 gained; GLOSSARY: Kin, Shield-friend, Rival, Feud, Mood note; TESTING; ADR 0019: kin are never stored, bonds and moods are sparse and cached.
- **Gate note.** Grief, `kin_killed` and the rival morale move desertion: re-run `bot_run`, with feuds and desertions by cause per seed in the step notes.
- **Done when.** Bot runs show grief, a few vengeances, friendships and feuds that start and end; the inspector explains every mood from its causes; `bot_run` still wins 3 of 5; NFR-09 holds with `--saga-bench` and on the run island.

### Step 13: Biographies, end screen and memorials (M, 8%)
- **What.**
  - **Words:** `saga::biography` and `saga::settlement_history`, words-only functions taking a tone. They draw on every record: quirks, deeds, epithets, ironies, weapons, kin and feuds, and rescues and relics for settlements.
  - **Inspector:** the biography replaces "Memories: none yet" for anyone with a record.
  - **Captain panel:** Notables; clicking one selects them.
  - **Settlement panel:** the settlement's history, its rescues and the relics in its storehouse.
  - **End screen:** Legends of the run, Foes remembered (enemy legends, and whoever killed one of your legends or your champion), The Fallen, Oddities of the run (the strangest death, the most storied weapon and its fate, mishaps by kind, the longest feud) and the bloodiest battle. The timeline shows legends risen and fallen and hides `Named`, `TraitEarned` and `Mishap`.
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
  - Unit tests of the biography words: a dead legend, a living soldier with a quirk and a feud, a spawned soldier and a saved settlement with a relic, in both tones. Nobody named is ever told as "a soldier".
  - Screenshots of each surface in the step notes.
- **Docs.** ARCHITECTURE's game-side table; GLOSSARY: Biography, Memorial, Oddities; TESTING.
- **Done when.** Every surface in the design's section 15 table shows its saga, in both tones.

### Step 14: The run record for M6 (S, 2%)
- **What.**
  - **`saga::run_record(world) -> RunRecord`:** pure, serde RON, versioned (`RUN_RECORD_FORMAT` 1). It holds facts and proper nouns, never sentences: legends (with quirks and their weapon), foes, the fallen, battles, named weapons and their fates, rescues, and the run's oddities.
  - **The game** writes `legends/<seed>-<difficulty>-<ended tick>.ron` under its save directory when a run ends, abandoned runs included. This happens before `leave_run` removes the finished run's slot. The same ending, after a quickload, overwrites its file.
  - **Headless:** `sim_headless --legends-out <path>`.
- **Where.** `crates/sim_military/src/saga.rs`, `crates/game_app/src/saves.rs`, `crates/sim_headless/src/main.rs`.
- **Tests.** `run_record_lists_legends`.
- **Docs.**
  - ARCHITECTURE: the saves section, on what survives a run.
  - GLOSSARY: Run record.
  - ADR 0019: no code reads the record in M4c, and nothing carries between runs; M4b adds memorials to it.
- **Done when.** A finished run in the game leaves one record file, and headless writes the same record for the same run.

### Step 15: Docs and playtest (S, 3%)
- **What.**
  - Complete ADR 0019.
  - CLAUDE.md:
    - add a "Saga (ADR 0019)" rules block: the sim stores ids, never words; `saga.ron` is words only, exempt and unsaved; a told death goes through a `Fall` in `Saga`, never through `EventFeed`; battles get a number at their first blow; effects act through their holder, through `Veteran`'s cached nudges, and are run-only; quirks are dealt per SpawnId from their own stream, never from `populate`; mishaps are never notable and never touch the champion, a captain or a legend; a named weapon is always in exactly one place; kin are never stored; `Saga` is append-only; `saga::settle` is progress slot 13;
    - update the milestone line.
  - README's status line.
  - PLAY_GUIDE: legends, quirks, mishaps, named weapons, kin and moods, the tone switch, and a cheat-sheet row.
  - SEED.md: tick the criteria that pass, with dated measurements, including NFR-09's.
  - Finish `docs/M4C_PLAYTEST.md` (part 2 after part 1).
- **Done when.** Every M4c criterion except the playtest is ticked, and the playtest checklist is ready.

Steps 1–3 and 5–12 are sim work, testable headless; step 3's words are checked in headless output. Steps 4 and 13 need the game. Step 14 needs both.

## Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Brutal words read as gratuitous, especially about villagers | High | Militia and rebel victims always take grave keys and never an irony; content rules in DATA.md; a deny-list lint; words reach the game at step 4 for an early look; a playtest question |
| The milestone is now half again as big | High | The split after step 8: part 1 stands on its own (kill lines, quirks, mishaps, moments, epithets, absurd deaths) and part 2 can slip; the largest step (kin and moods) is the last sim step, so it can be trimmed without touching the rest |
| Deadpan tips into silly, or the jokes wear thin | Medium | The voice rules and the do and don't table; the lint bans `!`, `?`, capitals and winking words; mishaps about once a day, not more; a playtest question |
| Mishaps feel unfair, or wipe a band | Medium | Low-tension beats; a third fall on your enemies; good ones as well as bad; the never-run-ending rules in code, each tested over every `bot_run` seed |
| Mishaps change pacing | Low | Not notable; tested that the quiet clock, the first event and the raider schedule don't move |
| Quirks shift balance or a village's food | Medium | Tiny effects through the holder, run-only; `bot_run` at step 5; `village_day` and `famine` re-run after the villagers.ron change |
| Something told didn't happen (geese, weddings, fords) | Medium | Flavour only on a real effect; ironies only from the design's table, with the "not in M4c" ends having no key; the geese named as the one flavoured trigger |
| Kill and saga lines flood the feed | Medium | Tier A narrowed; the rising bar, 10 tier-B lines a battle and 8 told deaths a tick in the sim; the speed filter, rate limit and fold in the feed; saga notes dropped first, at most 2 a game day at 8x; tested at 1,000 a side and over every `bot_run` seed |
| Moods snowball into desertion waves | Medium | Four entries, clamped sums, desertion ×2 at most and only from the rare `kin_killed`; `bot_run` reports desertions by cause |
| Named weapons duplicate or vanish | Medium | One place per weapon, out first in arming and last in a barter, hourly reconciliation, a test for every way one moves |
| Legends too rare to notice, or too common to matter | Medium | The numbers are data, tuned against `bot_run`'s counts; epithets carry the mid-game; the cap of three |
| Earned traits too common (Cruel above all) | Medium | Marks only from eligibility, one of each kind per battle; target rates checked in step 9 |
| Balance or pacing drift | Medium | Effects are run-only and exact no-ops; new kinds aren't notable except `LegendFell`; `bot_run` after steps 1, 5, 6, 7, 8, 9, 10, 11 and 12 |
| Cost in big battles and on the run island | Medium | A push per kill is the only new work per blow; per-death work is bounded; nudges cached on `Veteran`; hourly work for the sparse parts; `--timings` on `--battle 7500` in every sim step against a recorded baseline, `--saga-bench` from step 9, and the run island from step 5 |
| Names of the dead lost | Medium | Kept deaths hold `NameId`s; anyone named has their death kept; the biography test checks it |
| Lines repeat | Medium | Variants stepped by told-death index; composition; the bank-size lint; the golden test |
| Saves break mid-milestone | Low | `SAVE_FORMAT` 2 at step 2, bumped again on any later layout change; M4 saves are refused by design, after the playtest |
| Scope creep (nemesis AI, memorial effects, feed volume, a blow-by-blow log, animals, rumours) | Medium | Each is an open question or a "later" note in the design, defaulted to "not in M4c"; memorials with effects are M4b's, animals M4b's, rumours M5's |

## Defaults chosen (change any)

1. **The step order** puts kill lines in the game at step 4, quirks at 5 and mishaps at 6, before the steps that change play most (traits, legends, kin). You can stop after step 4 for a look, or after step 8 for part 1's playtest.
2. **The split** after step 8 into part 1 ("Deaths and oddities", about 57%) and part 2 ("Legends and kin", about 43%).
3. **Records and kill lines exist in every preset; effects only in runs.** The skirmish then shows kill lines, which helps tune the words. Quirks, mishaps, weapons, bonds and moods are run-only.
4. **Notability:** tier A is captains, the champion and legends (and a legend's milestone kills, and the rare mishap death); the bar is 40 + 15 per told line, with at most 10 tier-B lines a battle and 8 told deaths a tick. The weights are in the design's section 2.
5. **Legends:** fame 25, three battles and an epithet; three enlisted per faction; an aura of 8 tiles with −0.03 on the break point and +0.03 on the morale target; a death shock of 0.10, and 0.03 to foes.
6. **One new system slot:** progress slot 13, `saga::settle`, hourly, after `director::step`, from step 5. Everything else lives in existing systems.
7. **`Service`, `Mood`, `Bonds` and `NamedWeapon` are sparse-set components,** so starting one never moves a soldier's table row; what the hot loops read is cached on `Veteran` (introduced in step 5).
8. **The golden words test uses its own fixture bank,** so editing the real bank never churns it.
9. **Step notes** go in `docs/m4c/`.
10. **Grudges and the Avenger deed** (open question 1) are built in step 10 only if you say yes.
11. **Battles get a number** at their first blow, so both sides' reports share a name, first blood and bar.
12. **Told deaths and saga notes live in `Saga`,** read by the feed with its own cursor, never through `EventFeed`; mishaps are events, not notable.
13. **Quirk and mishap draws** use new stream keys only (`sim_agents::quirks ^ SpawnId`, `sim_military::director::mishap`, `sim_military::mishap ^ n`, `sim_military::saga::homesick ^ SpawnId`); no existing stream moves.
