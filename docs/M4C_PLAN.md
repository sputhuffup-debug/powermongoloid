# M4c Plan: Saga

Status: **planned 2026-10-12 (revised the same day after review, again for your follow-up decisions, once more after a second review of those against the code, then for your combat-spectacle decisions: gore effects with three backends and bevy_hanabi, maimed survivors and the kill cam, and last after a review of those against the code and Hanabi's source), awaiting your go; renumbered 2026-10-13 for M4a "Lordship", which now comes first: ADR 0021, `SAVE_FORMAT` 3, progress slot 17, and baselines on the M4a build.** M4c starts after M4a "Lordship" (`docs/M4A_PLAN.md`), which follows the M4 playtest (`docs/M4_PLAYTEST.md`), and comes before M4b.
Design: `docs/SAGA_DESIGN.md` (draft 2026-10-12; all your decisions are recorded there). Spec: `docs/SEED.md` § "M4c: Saga".

M4c gives combat its stories, and every run its oddities. Every death names its killer and how it happened. Notable deaths get a brutal line, or a mild one if you choose, and the rest are summed up in the battle's report. Every villager in a run has a quirk or two, and the pacing director lets small things go wrong. Soldiers keep a record, earn an epithet (flattering or not), and sometimes die in ways people retell. Captains and long-serving soldiers earn Brave, Shaken, Cruel or Beloved. A few become legends who steady the men near them, and whose death shakes an army, pauses your game and leaves a cairn. Weapons that kill someone who matters get names and change hands; households march together, grieve and avenge; feuds start at dice. Everything is told in one voice, deadpan dark. Fights show it too: blood, bodies and severed parts that match the kill lines, over the top in the brutal tone and dust in the mild one, drawn by GPU particles (bevy_hanabi) where the machine has compute shaders and by a CPU fallback where it hasn't. Survivors of bad wounds can be maimed, and when a captain, the champion or a legend dies near your camera, time slows for a second. The end screen remembers it all, and a small record of each run is kept for M6's book.

**The milestone is bigger now.** Your follow-up (quirks, mishaps, mocking epithets and absurd deaths, named weapons, kin and moods) made it about **1.6 times** the first plan. The four new steps (quirks, mishaps, weapons, kin) were a third of that whole. The rest came from additions folded into existing steps:
- mocking epithets, ironies and rescues in step 8;
- the odd in step 7;
- the voice and feed forms in step 3;
- richer biographies in step 13.

Your combat-spectacle decisions add about a quarter again, so the milestone is now about **1.25 times** the previous plan and about twice the first. The additions are four new steps and a grown step 4:
- **3b:** the effects layer with its CPU and Off backends: bodies, pools and the spray, with no new dependency;
- **4:** severed parts that match the lines, and the Effects setting;
- **4b:** the GPU backend, with the bevy_hanabi dependency;
- **8b:** maimed survivors;
- **10b:** the kill cam.

New steps carry a letter, as M3b and M4b do, so every existing step number and cross-reference stays valid. **A split is sensible:**
- **Part 1, "Deaths and oddities"** (steps 1–8b, about 64%): kill lines in the game with blood and bodies, GPU particles, quirks, mishaps, battle moments, deeds and epithets (honourable, mocking and scarred), absurd deaths, rescue records and maimed survivors. It ends with a short playtest of its own. Every run is already different, the feed already speaks, and a fight already looks like one.
- **Part 2, "Legends and kin"** (steps 9–15, about 36%): earned traits, legends and the kill cam, named weapons, kin, friends, rivals and moods, biographies and the end screen, the run record, docs and the full playtest.

You can stop after part 1 for a look, and part 2 can slip without leaving anything half-built.

The steps are ordered to deliver value early:
- **The kill lines you asked for reach the game at step 4,** under a third of the way in (30%). The effects layer (step 3b, CPU and Off backends) comes just before, so the lines arrive with their blood, bodies and severed parts. The GPU backend (step 4b) comes right after: it only makes the spray denser, so the first playable slice never waits on the new dependency, and if Hanabi gives trouble the CPU backend carries the effects alone.
- **Quirks (step 5) and mishaps (step 6) come next,** because they make every run different soonest. From step 6, no two runs read alike.
- **Maimed survivors (step 8b)** come right after the epithets and wounds they build on, closing part 1. **The kill cam (step 10b)** comes right after legends, so it fires for all three: captains, the champion and legends.
- **Steps that change how a run plays, by design:** 5 (quirks), 6 (mishaps), 8 (the Vain quirk's epithet effects), 8b (maims), 9 (earned traits), 10 (legends), 11 (looting named weapons) and 12 (kin and moods). Each is gated by `bot_run`. Steps 3b, 4b and 10b, and the visuals of steps 4, 8b and 13, are render and UI only, and change nothing in the sim.
- **Steps 1 and 7 may shift a run slightly,** and are gated by `bot_run`. Step 1's rebel kill credit changes the player's kill count, which weights dominion boon offers. Step 7 makes a wiped army's `Battle` notable, which the pacing director sees.

## Requirements covered

| ID | Requirement | Where |
| --- | --- | --- |
| SIM-39 | Every death records who killed whom and how, absurd ends included; notable deaths are told, the rest counted | `sim_military::combat` (the strike loop, its battle bookkeeping and `casualties`), new `sim_military::saga` |
| SIM-53 | Service records, deeds and epithets, honourable and mocking | `sim_military::saga`, `sim_military::veterans` (`end_battles`), `sim_military::orders` |
| SIM-34 | Earned traits; the champion's picked traits stay as built | `sim_military::saga`, consumers in `soldiers`, `upkeep`, `combat` and `orders` |
| SIM-54 | Legends: rise, aura, death shock | `sim_military::saga`, `index` (`LegendIndex`), `soldiers`, `upkeep`, `combat` |
| SIM-55 | Quirks dealt per person, acting through their holder | new `sim_agents::quirks` (the deck in `villagers.ron`), `sim_agents::ai`, `sim_agents::act`, `sim_military::veterans` (cached nudges, from `progression.ron` `saga.quirks`), `soldiers`, `upkeep`, `spawn`, `progression::setup_run` |
| GP-30 | Mishaps from the pacing director | `sim_military::director`, new `sim_military::mishaps`, `sim_military::saga` (`Mood`) |
| SIM-56 | Named weapons and their fates | `sim_military::saga`, `combat` (`casualties`), `orders` (`arm_followers`, barter, `equip`, `enlist`), `upkeep` (`apply_desertions`) |
| SIM-57 | Kin, friends, rivals and moods | `sim_military::saga`, `combat`, `veterans` (`end_battles`), `soldiers` (the vengeance aim), `mishaps` (the brawl) |
| UI-10 | Phrase banks in two tones and one voice; tone as a viewer setting | new `assets/data/saga.ron`, `sim_military::chronicle` (the `Teller`), `sim_military::saga` (words), `game_ui`, `game_app` (preferences) |
| UI-09 | Battle-report moments; biographies of people and settlements | `sim_core::events` (`BattleReport`), `sim_military::saga` (words), `game_ui` panels and end screen |
| RND-09 | Legend marks, cairns, memorial stones | `game_render` |
| GP-29 | The run record for M6's legends book | `sim_military::saga::run_record`, `game_app` (writes it), `sim_headless` (`--legends-out`) |
| SIM-59 | Maimed survivors: a hand, an eye or a leg lost once in a life, recorded, penalised in a run, named | `sim_military::combat` (the strike loop's check, the new `maims` step after `casualties`), `saga` (records, notes, scar epithets), `veterans` (cached multipliers), `soldiers` (pace); `game_render` (the moment, maimed figures) |
| RND-10 | Combat effects: blood, bodies and severed parts that match the lines; GPU, CPU and Off backends | new `game_render::effects` (requests, the collector, backends, bodies, pools, parts, selection; the GPU backend in `effects/gpu.rs`), `game_render::agents` (the corpse hand-off, SpawnId per visual), `game_render::art`; `game_app` (adds the plugin, flags, preferences, `--effects-bench`); `sim_military::combat` (`Casualties` scratch: `struck`, the victim in `fallen`, maims), `sim_military::saga` (`wound`, `Teller::told_wound`); `assets/data/render.ron`, `assets/data/saga.ron` |
| RND-11 | The kill cam | new `game_ui::kill_cam`, `game_ui::feed` (the held pause), `game_ui::time_bar`, `game_render::camera` (the nudge) |
| NFR-10 | The effects' frame cost | `game_app --stress 20000` and `--effects-bench` with `stress_log`, on the reference machine and on a software adapter |
| NFR-09 | The saga's cost in big battles and on the run island | `sim_headless --timings` (`--battle 7500` with and without `--saga-bench`; a run's live ticks, `--run --ticks 7200`; a bot run's live windows, with the saga's own spans), `cargo bench -p sim_headless` |

**Not in M4c:**
- **M4b:** heroes (SIM-51, SIM-52), which may reuse the legend aura; hero memorials raised by saved villages (SIM-58), which read M4c's rescue records; animals with names (which make Fears geese literal); weather quirks such as Hates rain; stale food in mood notes; weapon kinds and named injuries, which come with the equipment tiers; and healing.
- **M5:** villagers' memories of the dead (SIM-14), prisoners and executions (GP-14's dilemmas), and tall tales and rumours about you (faction knowledge).
- **M6:** the legends book itself, and the options screen. M4c pulls forward only the tone, Effects and Kill cam switches. Figures with limbs and ragdoll falls are M6's art polish.
- **Not planned:** decals (forward decals need a depth prepass; clustered decals need compute shaders), blood on figures and weapons, and a web build (Hanabi needs WebGPU; WebGL2 would get the CPU fallback).
- **Not adopted:** nemesis AI. Open question 1 in the design adds only words and a deed.

## Acceptance criteria and how each is tested

| Criterion | Test |
| --- | --- |
| Every death by a blow names its killer and how it happened, and a rebel's kill is credited to the rebels | **Test `kill_records_name_the_killer`** (sim_military unit, hand-built world): a killing blow records victim, killer, credited side and one-blow. A second swing at the dead records nothing. The manner bits (armed, fleeing, last stand) are set in `casualties`. **Scenario `rebels_kill_for_themselves`** (`tests/tests/saga.rs`, run preset): a conquered village with a small garrison revolts. Every rebel kill counts for faction 0 in `ArmyStats`, the garrison's report lists faction 0 among its foes, and a champion slain by rebels is told as killed by the villagers. |
| Captains, the champion and legends are always told; any other death only if it clears its battle's rising bar, at most 10 a battle and 8 told deaths a tick; the feed at 8x shows only lines about your people or a legend; the report's "more died" matches | **Scenario `notable_deaths_follow_the_rules`** (1,000 a side, seed 42, 3,000 ticks). Every captain's death is told. Per battle, at most 10 tier-B deaths are told, and at seed 42 the big battle tells between 4 and 10. No tick tells more than 8 deaths unless they're all tier A. Every told line raised the bar. Told plus untold equals deaths, and every named person's death is kept. Each report's "more died" equals its losses minus its named dead. **Scenario `battles_share_a_number`:** two armies meeting get one battle number, place and ordinal; a third army striking one of them joins it. **`bot_run`:** over every seed, at most 8 deaths a battle pass `saga::feed_filter` at 8x. |
| In a run, every person has one or two quirks from their own stream; each acts only through its holder; nobody without one changes | **`tests/tests/quirks.rs`:** `quirks_are_dealt_per_person`, `quirks_act_through_their_holder` and `sweetheart_walks_home`, as in step 5. `village_day` and `famine` pass unchanged. |
| The director's mishaps come from real state, do what their lines say, never touch the champion, a captain or a legend, never kill more than the rules allow, never empty a baggage, never override an order or delay a march toward a fight, and never count as notable events | **`tests/tests/mishaps.rs`:** `each_mishap_does_what_it_says`, `mishaps_never_wipe_a_run` (all three personalities, compressed data) and `mishaps_are_not_notable` (a unit test and a stubbed-effects scenario), as in step 6. **`bot_run`:** the guards hold over every seed; mishap counts by kind per seed in the step notes. |
| Battle reports name first blood, who ran first, when the army broke, the told deaths, the best killer and anything odd; in a run, an army killed to the last man still gets its report | **Scenario `moments_reach_the_report`** (two squads of 20, seed 42): both armies' reports carry one battle number and name, and the same first blood. Who ran first is the soldier whose rout set the turning point. The broke tick is the first with 30% of the living members routing. The best killer is the top killer even if killed later. **Scenario `wiped_army_gets_a_report`** (run preset, a band of 5 against 40): a report marked wiped, with a `Battle` entry. |
| In a run, deeds earn one epithet by rule, honourable or mocking, and a mocking one never replaces an honourable one; absurd deaths are told only for causes the sim produced; an answered drive-off request records its heroes only when they fought raiders there | **`tests/tests/saga.rs`:** `deeds_are_recorded_once`, `epithets_follow_rank`, `mocking_epithets_follow_rank`, `absurd_deaths_are_real` and `rescues_are_recorded`, as in step 8. |
| In a run, Brave, Shaken, Cruel and Beloved are earned from experience; each acts only through its holder (their own morale, aim and desertion, and a captain's own dealings), and anyone without one behaves exactly as before | **`tests/tests/saga_traits.rs`:** `brave_and_shaken`, `cruel_and_beloved`, `picked_and_earned_are_separate` and `earned_traits_are_exact_no_ops`, as in step 9. |
| Legends rise by fame, at most three enlisted per faction; they steady men within 8 tiles; their death shakes their army once and pauses your game for your own | **`tests/tests/legends.rs`:** `legend_rises_at_fame`, `legend_steadies_men` and `legend_death_shakes_once`, as in step 10. **A `game_ui` test** (`legend_fell_pauses_for_your_own`): the feed pauses for your legend's `LegendFell` and not for an enemy's. **`bot_run`:** still wins 3 of 5 seeds, and a player legend rises on at least 3 of them. It prints legends, epithets and earned traits per seed. |
| A weapon that kills a captain, the champion, a legend or a monument's beast is named, and is always in exactly one place as it is looted, passed on, put in the baggage, sold, taken home or lost | **`tests/tests/weapons.rs`:** `a_weapon_is_named_by_a_notable_kill` and `named_weapons_change_hands`, as in step 11. |
| Kin grieve and avenge; friends and rivals form by rule and feuds end; moods are bounded, expire and act only through their holder; the inspector explains a soldier's mood | **`tests/tests/kin.rs`:** `kin_grieve_and_avenge`, `friends_and_rivals_form_by_rule`, `moods_are_bounded_and_expire` and `moods_are_exact_no_ops`, as in step 12. A `game_ui` unit test of the mood note's wording from a hand-built record. |
| Brutal and mild banks have the same keys; every line names its key's facts and uses only its key's slots; no gendered words; the deadpan lint passes; feed lines fit the feed; the tone never changes the simulation | **`tests/tests/saga_words.rs`:** a lint of the real `saga.ron` (with the voice rules) and a golden test on a small fixture bank (both in step 3, extended as banks are added). **Test `tone_is_not_state`:** a bot run's chronicle, told in both tones, reads differently, while its state hash and a recorded replay are identical. |
| The feed stays readable at 8x | **A `game_ui` test** that the throttle drops tier-B and saga-note lines at 8x, folds the overflow, and never passes more than 2 saga lines a game day at 8x, whatever comes in (a hard cap, on every difficulty). **`bot_run`:** over every seed, at most 8 deaths a battle pass the 8x filter, and the saga lines it passes are counted. |
| The inspector shows the biographies of soldiers and settlements; the end screen lists legends, foes remembered, the fallen and the run's oddities; cairns mark where legends fell | **Unit tests** of the biography words: a dead legend, a living soldier, a spawned soldier and a settlement, in both tones; nobody named is ever told as "a soldier". **Screenshots** of each surface in the step notes. |
| A finished run writes its legends record, and no sim crate reads it | **Test `run_record_lists_legends`** (`tests/tests/saga.rs`): the bot plays seed 1 to the end of its run. Its record round-trips through RON, names every legend and gives each a fate, lists its named weapons and rescues, with names written as strings. An abandoned run writes one with outcome `Abandoned`. |
| NFR-09 holds; developed, skirmish and `--battle` stdout are unchanged; saves and determinism hold | `--battle 7500 --timings --stats-every 250` and, from step 9, the same with `--saga-bench`; from step 5, the run island's live window, `--run --seed 7 --difficulty steady --ticks 7200 --timings --stats-every 2400` (no `run ended` before its end). For each, the median of three runs of the mean tick rises at most 2% over the baselines recorded at step 1 (on the M4a build). A bot run's live windows (`--run --bot --seed 42 --ticks 96000 --timings --stats-every 250`, before the win) at steps 6, 12 and 15: the saga's own spans at most 2% of the mean tick. Never a whole-run mean that includes frozen ticks after a run ended. The bench. The stdout comparisons in "What stays unchanged". **`save_load`:** records, quirks, moods, bonds, weapons, legends and moments round-trip, `every_hashed_type_is_saved` covers the new types, and a format-2 (M4a) save is refused. The determinism tests, single- and multi-threaded. |
| In the brutal tone, inside the detail band, blows spray, kills burst and leave pools that fade, told kills throw the part their line names, and the dead lie where they fell and then sink; deserters never leave a body or a spray; a finished run sends no more; mild shows dust and falls only; Off draws no spray and the same bodies, pools and parts; a new world or a load clears it all | **game_render unit tests** `effect_requests_follow_the_records` and `bodies_and_pools_fade_and_clear` (step 3b). **`wound_and_line_agree`** (`tests/saga_words.rs`, step 4): for every told death of a seeded 1,000-a-side battle, every brutal form that tells it names a severing wound exactly when `told_wound` severs, with the part the request builder sends, also after the killer earns a new epithet. **Screenshots** on each backend in the step notes |
| The backend is chosen from the adapter (GPU with compute shaders, CPU without them or on a software adapter, Off without a renderer) and the Effects setting and `--effects` override it; headless builds, tests and CI need no GPU | **`select_backend`** (game_render unit, every row). **Smoke runs** (steps 3b and 4b notes): each backend forced, automatic, `WGPU_SETTINGS_PRIO=webgl2` and `WGPU_BACKEND=gl` on the dev box's lavapipe; automatic on your RTX 4070 when you can. **`cargo tree -p sim_headless -i bevy_hanabi`** reports that no package matches (it exits with an error, which is the pass), and the same for `integration_tests`; CI's workflow is unchanged and green |
| A survivor past half their health in a battle may lose a hand, an eye or a leg, once in a life, by rule from their own stream, even on a tick when nobody dies; it's recorded, earns a scar epithet, weakens only its holder's attack, guard or pace in a run (a captain's pace only in a rout), and is never contradicted by a later line or visual | **`tests/maims.rs`:** `maims_follow_the_rule`, `maims_act_through_their_holder`, `maims_are_exact_no_ops` and `maim_words_and_visuals_agree` (step 8b). `save_load`; `bot_run` counts maims per seed |
| The kill cam slows to a quarter for about a real second and leans in when a captain, the champion or a legend is killed by a blow on screen near the camera at 1x–4x (never for a starving captain, or for an ordinary soldier a legend kills); never at 8x, in fast-forward, paused or behind a dialog; once in 10 s; your input wins; a pause it holds is never lost; switchable off | **game_ui unit tests** `kill_cam_*` (step 10b), on the pure policy. Its speed changes only `Time<Virtual>`; the sim's tick sequence doesn't depend on it |
| NFR-10: the effects stay within their frame budget | Steps 3b, 4b and 15 notes: `--effects-bench` medians per backend and Reduced against Off on lavapipe here (at most +10% median frame time). On your RTX 4070, handed over and not blocking a step: `--stress 20000` with Full effects against the same window measured before step 3b (worst clash window at 60 fps or more, at most +1.5 ms) and 1,000 a side in the detail band at 60 fps. NFR-10 is ticked only once those numbers are in (step 15) |
| **Playtest:** you can name a legend after a run and say how they fell; a death landed; a mishap or an odd death made you laugh or wince, and none felt unfair; brutal reads grim, not numbing; mild reads sober, not dry; the deadpan reads dry, not silly; a fight seen up close reads as a fight, the gore lands in brutal and nothing bloody shows in mild; the kill cam feels earned, not intrusive | You play two runs, one in each tone (checklist `docs/M4C_PLAYTEST.md`; part 1's checklist is written in step 8b, the whole in step 15). |

## What stays unchanged

- **Stdout of the guarded commands.** Each step compares these release commands with the previous step's binary, and the stdout must be byte-identical. State hashes and timings, on stderr, may change.
  - developed: `sim_headless --seed 42 --ticks 4801 --stats-every 600`
  - skirmish: `--skirmish --seed 42 --ticks 6000 --stats-every 2000`
  - battle: `--battle 1000 --seed 42 --ticks 3000 --stats-every 600`
- **The M2 and M3 tests** pass unchanged, with today's tuning: `village_day`, `famine`, `army_march`, `posture`, `starving_army` and `orders`. Every saga effect is run-only, and is an exact no-op without a trait, quirk, mood, bond or legend. Outside a run nobody has a quirk, so the villager AI scores exactly as today. `villagers.ron` and `director.ron` hash their data text, so the developed world's state hash changes whenever either gains a section (steps 5, 6, 10 and 12: quirks, mishaps, `LegendFell` in `auto_pause`, the brawl's row), and its behaviour doesn't.
- **The run-preset tests** (`champion`, `chronicle`, `conquest`, `director`, `dominion`, `gather`, `invitation`, `monuments`, `rival`, `run_start`, `save_load`, `sellswords`, `wanderers`, and M4a's `tenure`, `influence`, `take_modes`, `fallout`, `edicts`, `stewards`, `construction`, `specialty` and `smithy_chain`) can move once quirks (step 5) and mishaps (step 6) act in every run. They're re-run in both steps. Where one's tuning moves only because the run took another path, it opts out through data it already loads (an empty `quirks.deck` and an empty `mishap_kinds` both deal nothing) instead of being retuned, and the dedicated `quirks` and `mishaps` tests run with both on. `conquest` and `invitation` guard loyalty drift (CLAUDE.md), which Pious changes: they're re-run after step 5 with quirks on, and opt out only if the drift they guard is what moved.
- **Replays.** The replay format (`REPLAY_FORMAT` 6, where M4a put it) and the command kinds don't change: M4c adds no command, and the tone is never an input.
  - Old replay files still parse, but their checkpoint hashes stop matching after step 2, as after every M4 step. Step 1 already changes those in which rebels fought. Re-record any you want to keep.
  - In every step, a fresh run replay records and verifies, multi- and single-threaded.
- **Pacing.** The director's quiet clock sees only `LegendFell` and a wiped army's `Battle` as new notable events. Mishaps are chronicled but never notable, and fast-forward stops on nothing else new (tested in step 6). Mishaps do change a run's state, so a run with them unfolds differently, as any change to a run does.
- **`FactionModifiers` is untouched.** Nothing personal goes into it, and the champion's picked traits still act faction-wide.
- **M4a's rules stay as built** (Lordship, [ADR 0020](adr/0020-lordship.md)). Stewards, agents and bandits are ordinary villagers and soldiers to M4c: quirks are dealt to them like anyone alive at the end of `setup_run` or spawned later, a bandit's kill is told like a raider's, a sack leaves Cruel marks (plunder by a captain's army) and a gift Beloved ones (food given to a village). M4a's bandits and reprisals bring more drive-off fights, so more rescues (the design's section 14).
- **The chronicle's existing entries keep their words, with two exceptions.** The `Battle` line gains faction nouns ("against the rival's band" instead of "against Rival") and the battle's name (steps 3 and 7), and `RequestDone` names the hero of a rescue (step 8). `chronicle.causes` gains one phrase.
- **`populate`'s draws are untouched** (`docs/m4/BUILD_GUIDE.md` G10): quirks come from their own stream per SpawnId, after population.
- **Saves are the one intended break.** From step 2 (`SAVE_FORMAT` one above M4a's final value, 3 as planned: M4a bumps it exactly once, to 2), M4a saves are refused.
- **The sim, saves and replays never see the effects.** Steps 3b, 4b and 10b, and the visuals of steps 4, 8b and 13, are render and UI state. The effects collector runs in `FixedLast` and writes only render-side resources. `Casualties.struck`, `fallen`'s victim, `maim_checks` and `maims` are exempt scratch, reset on load. The kill cam changes only `Time<Virtual>`. None of it is a command, a save field or a replay input.
- **Headless and CI need no GPU.** `sim_headless` and `integration_tests` don't depend on game_render. Only game_app adds `EffectsPlugin`, and it adds `HanabiPlugin` only when the render device has compute shaders. No test adds either, and Hanabi's `gpu_tests` feature stays off. `cargo test --workspace` compiles bevy_hanabi (into game_render) and never runs it.

## The gate (every step)

1. **Build and lint:** `cargo fmt --check`; `cargo clippy -j 2 --workspace -- -D warnings`; `cargo test -j 2 --workspace`; `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps`.
2. **Determinism:** the determinism tests, plus a run replay. Record `--run --seed 42 --ticks 3000 --commands 50 --record run.ron`, then verify it with `--replay run.ron` and `--replay run.ron --single-threaded`.
3. **Invariance:** the three stdout comparisons above.
4. **Cost, for sim steps (NFR-09), on live ticks only:** `--battle 7500 --seed 42 --timings --stats-every 250`, three runs, against the baseline in `docs/m4c/`. From step 5, also the run island's live window, `--run --seed 7 --difficulty steady --ticks 7200 --timings --stats-every 2400`, three runs, against its baseline, after checking stderr has no `run ended` line before tick 7,200 (if it does, compare up to the last full window before the end, and say so). From step 9, also `--battle 7500` with `--saga-bench`. At steps 6, 12 and 15, also a bot run's live windows (`--run --bot --seed 42 --ticks 96000 --timings --stats-every 250`, the windows before the win): the saga's own spans at most 2% of the mean tick, with the whole-tick mean and stage times recorded beside it. A whole-run mean that includes frozen ticks after a run ended is never a measurement.
5. **Docs in the same commit** (CLAUDE.md "Developer handbook"):
   - `docs/dev/DATA.md` for data;
   - `docs/dev/ARCHITECTURE.md` for systems and slots;
   - `docs/dev/TESTING.md` for tests;
   - `docs/dev/GLOSSARY.md` for terms;
   - `docs/dev/HOWTO.md` for recipes others will repeat;
   - ADR 0021 for the saga's decisions, and [ADR 0019](adr/0019-combat-effects-and-bevy-hanabi.md) for the effects layer and its dependency.
6. **Balance, for steps marked bot_run:** `cargo test --release -j 2 -p integration_tests --test bot_run -- --ignored` still wins 3 of 5. Record the win days and saga counts in the step notes: legends, epithets (honourable and mocking), earned traits, quirks, mishaps by kind, ironies, weapons named and their fates, feuds, and desertions by cause, as each exists. Change data when the game is unbalanced, and the bot when it plays badly (HOWTO "Tune run balance"). From step 8b, maims by part.
7. **Frames, for render steps** (3b, 4, 4b, 8b's visuals, 10b, 13):
   - **On the dev box** (lavapipe under Xvfb, 3 to 4 fps), part of the gate:
     - the smoke runs (each backend that exists forced, automatic, `WGPU_SETTINGS_PRIO=webgl2`, `WGPU_BACKEND=gl`);
     - screenshots in both tones;
     - `--effects-bench` medians of the effects spans;
     - Reduced against Off frame time.
   - **On your RTX 4070,** handed over, not part of the gate: after steps 3b and 4b (and 4 and 13 if their visuals add draws), the step notes give NFR-10's `--stress 20000` and 1,000-a-side commands, and the numbers join the notes when you run them. NFR-10 is ticked at step 15 only once they're in.

Step notes go in `docs/m4c/`, as M4's went in `docs/m4/`.

## Build order

Sizes are shares of the milestone as it stands now, with the combat spectacle: S is 2–3%, M 5–7%, L 8%. The earlier steps keep their relative sizes, scaled to about 0.8 of their previous shares, except where the spectacle added work to them (steps 1, 2, 3 and 4 grew a little). The lettered additions are about a fifth of the whole (3b 5%, 4b 3%, the growth of step 4 about 2%, 8b 5%, 10b 2%).

| Part | Step | Size | Share | Running total |
| --- | --- | --- | --- | --- |
| 1 | 1. Kill attribution, the rebels' credit, the blow list | S | 3% | 3% |
| 1 | 2. Battle numbers, service records, told deaths | L | 8% | 11% |
| 1 | 3. Words, tone and the voice | M | 7% | 18% |
| 1 | 3b. The effects layer: CPU and Off backends, bodies and pools | M | 5% | 23% |
| 1 | 4. Kill lines in the game, with severed parts | M | 7% | 30% |
| 1 | 4b. The GPU backend: bevy_hanabi | S | 3% | 33% |
| 1 | 5. Quirks | M | 6% | 39% |
| 1 | 6. Mishaps | M | 6% | 45% |
| 1 | 7. Battle moments | M | 6% | 51% |
| 1 | 8. Deeds, epithets, absurd deaths and rescues | L | 8% | 59% |
| 1 | 8b. Maimed survivors (part 1 ends) | M | 5% | 64% |
| 2 | 9. Earned traits | M | 5% | 69% |
| 2 | 10. Legends | M | 5% | 74% |
| 2 | 10b. The kill cam | S | 2% | 76% |
| 2 | 11. Named weapons | M | 5% | 81% |
| 2 | 12. Kin, friends, rivals and moods | L | 8% | 89% |
| 2 | 13. Biographies, end screen and memorials | M | 6% | 95% |
| 2 | 14. The run record for M6 | S | 2% | 97% |
| 2 | 15. Docs and playtest | S | 3% | 100% |

### Part 1: Deaths and oddities

### Step 1: Kill attribution, the rebels' credit and the blow list (S, 3%; bot_run)
- **What.**
  - **The baselines first.** Three runs each, recorded in `docs/m4c/baseline.md` with the spread between runs:
    - `--battle 7500 --seed 42 --timings --stats-every 250` on the M4a build (the build M4c starts from): deaths and blows a tick, the clash window's mean tick, and the Act, Decide and Needs stage times;
    - the run island's live window, `--run --seed 7 --difficulty steady --ticks 7200 --timings --stats-every 2400`: the mean tick and the Decide and Needs stage times, where 4,000 to 5,000 villagers live. Not `--ticks 24000`: without a bot the run ends on day 3.7 (tick 8,969, on the M4 build; re-check it on the M4a build, whose bandits and reprisals can move the end, and shorten the window if it now ends before tick 7,200), and the frozen ticks after it (0.03 ms each, 63% of that command) would swamp the mean;
    - a bot run's live windows, the M4 convention (`docs/m4/PROGRESS_BOT.md`): `--run --bot --seed 42 --ticks 96000 --timings --stats-every 250`, averaging the windows before the win, with the stage times.
  - `Casualties.slain_by` becomes `kills`, a list of `Kill`: victim, killer, the credited side, and one blow (the victim was at full health).
  - A rebel's kill is credited to the independents, matching how `casualties` already treats rebel victims. A rebel is militia whose `against` is its own allegiance. The credit applies in `ArmyStats.kills`, in the garrison's `battle_foes`, and in the champion's cause of death.
  - **The blow list,** `Casualties.struck`: this tick's landed blows as (victim, striker), exempt scratch, cleared at the top of `resolve_strikes` with the kills. It's cleared, never dropped, so it keeps its capacity and allocates nothing once warm. Its length is the blow count, summed into `--timings`' windows on stderr. No blow is counted today; the effects' budget (step 3b) is built on an estimate of 25 to 50 a tick at the clash's peak, and the baseline replaces it with a measurement. Step 3b's hits read this list, never the renderer's view of health.
- **Where.** `crates/sim_military/src/combat.rs` (`Casualties`, the kill branch of `resolve_strikes`, the blow list); `crates/sim_headless/src/main.rs` (the blows in `--timings`); `crates/sim_military/src/progression/champion.rs` (`cause_of_death` reads the kills through a small helper); `crates/sim_military/src/veterans.rs` (its unit test asserts on the scratch); `crates/sim_military/src/save.rs` (`after_load` clears the scratch).
- **Tests.** `kill_records_name_the_killer` (with the blow list: every landed blow listed once, none for a swing at the dead); `rebels_kill_for_themselves` (new `tests/tests/saga.rs`). Re-run `champion` and `conquest`.
- **Docs.** ARCHITECTURE schedule row 5 (Act): the kill scratch and the blow list. TESTING rows. ADR 0021 is opened with the kill record and the rebel fix.
- **Done when.** Every kill names its killer, rebels' kills are theirs, both baselines are recorded, and nothing else moves.
- **Gate note.** The player's kills weight the dominion boon offers, so the fix can change a run: re-run `bot_run`.
- **Ships as:** a fix. When rebels fight, the run summary's kills and the champion's cause of death come out right.

### Step 2: Battle numbers, service records and told deaths (L, 8%)
- **What.**
  - **Battle numbers.** The strike loop's battle bookkeeping, in striker SpawnId order, joins two armies into one battle: an army with a number keeps it, one without takes the other's or a new `Saga.next_battle`. A militia blow joins the army it fights. `Captain` gains `battle`, which passes to an heir with `battle_since`. The first blow fixes the battle's place (nearest settlement), ordinal there and terrain, in `Saga`.
  - **Broke.** `build_index` sets `Captain::battle_broke` at the first tick with `saga.broke_share` (0.3) of the army's living members routing, beside `battle_rout`.
  - **`Veteran` gains four fields:**
    - `joined`: the tick they enlisted or were spawned, set in `enlist` and at spawn (irony 1 reads it in step 8);
    - `routed`: ran in this battle, set in `soldier_upkeep` on the soldier's own component;
    - `hurt`: the share of health lost to blows this battle, a saturating byte added in the strike loop on the victim's `Veteran`, which is already borrowed;
    - `trade`: the occupation before enlisting, captured in `enlist` (already added by M4a for stewards; M4c reuses it).
  - **New module `sim_military::saga`:**
    - `Fall` holds, besides the victim, killer, manner, tick and battle, **the position where they fell** (`at`), **the victim's role** (captain, champion, legend) and **the killer's epithet as it stood** (filled from step 8), so a told line never changes after it's told: the feed's "move the camera there", the cairn (step 13), the severed parts (step 4) and the kill cam (step 10b) need the position and the role;
    - `Service`, stored as a sparse-set component so that inserting one never moves a soldier's table row. It has the fields of the design's section 6: trade, home, kills, streak, battles and wounds here, and the rest from steps 6, 8 and 9;
    - `Saga`: kept deaths (`Fall`, with a `told` flag), battles, per-settlement battle counts, an untold count, `next_battle` and a running digest. **Its hash has two parts:** the append-only parts (`Fall`s, and battles once closed) as lengths and the running digest, folded in when pushed or closed; the parts changed in place (open battles with their lines told and first blood, the counts, `next_battle`) hashed in full. Later steps put their in-place state (weapons, grudges, pending mishap deaths) in the second part and their append-only lists (notes, rescues) in the first;
    - `SagaDeed` ids.
  - **`casualties` gains a first pass** over the kills, in strike order, before anything is despawned. It snapshots each killer, starts or updates their record, counts the streak, and updates the army's best killer (`Captain.battle_best`).
  - **The existing loop** (victim SpawnId order) classifies each death:
    - **Tier A:** the victim is a captain, the champion or a legend, or a legend makes a milestone kill. Always told, first.
    - **Tier B:** told against the battle's rising bar, at most 10 a battle (`max_per_battle`), with whatever remains of the tick's cap of 8.
    - **Every told line raises the bar.** A told death is pushed as a `Fall`, and so is the untold death of anyone named. A tier-A death by hunger gets a `Fall` with no killer.
  - **No new event for a death.** Told deaths live in `Saga`, and the feed reads them there (step 4), so they never pass through `EventFeed`.
  - **Rules** in `progression.ron` `saga`: `broke_share`, and `notable` (weights, bar, step, `per_tick`, `max_per_battle`). It's a `#[serde(default)]` section.
  - `SAVE_FORMAT` becomes one above M4a's final value (3 as planned: M4a bumps it exactly once, to 2). Later steps bump it again whenever they change a saved layout.
- **Where.** `crates/sim_military/src/saga.rs` (new); `combat.rs`, `index.rs`, `veterans.rs`, `orders.rs` (`enlist`, the heir's battle state), `upkeep.rs` and `components.rs` (`Captain` and its hash) in `crates/sim_military/src/`. Registration is in `lib.rs` and `save.rs`, in hash order. Also `crates/sim_core/src/save.rs` (`SAVE_FORMAT`), `crates/sim_military/src/progression/mod.rs` (the `saga` section) and `assets/data/progression.ron`.
- **Tests.**
  - `notable_deaths_follow_the_rules`;
  - `battles_share_a_number`;
  - `starving_is_not_a_kill`: a starving captain gets a told death with no killer and cause `Starvation`;
  - `records_survive_desertion`: a soldier with kills deserts and re-enlists, and the record keeps its kills;
  - `saga_hash_sees_every_change`: changing an open battle's lines told or first blood, a per-settlement count or `next_battle` changes the state hash (so does pushing a `Fall`);
  - `save_load`, with the new state round-tripped;
  - the determinism coverage tests.
- **Docs.**
  - DATA.md: `saga.broke_share`, `saga.notable`.
  - ARCHITECTURE: row 5 (the battle bookkeeping and `casualties`' two passes), row 2 (`battle_broke`) and row 6 (`routed`).
  - GLOSSARY: Battle number; Broke, Lost and Held; Service record; Told death and Kept death; Tier; Rising bar. The existing **Battle** entry gains the number.
  - TESTING.
  - ADR 0021: records hold ids, and the words come late; battles get a number; told deaths live in `Saga`, not in the event feed; `Saga`'s hash has an append-only part (digest) and an in-place part (hashed in full).
- **Done when.** A battle of any size keeps its told deaths bounded, every death is told, kept or counted, both sides of a fight share its number, the hashes cover the new state, and saves round-trip.
- **Ships as:** `sim_headless --saga` lists the told deaths in a plain debug form, until step 3 puts words to them.

### Step 3: Words, tone and the voice (M, 7%)
- **What.**
  - **`assets/data/saga.ron`**, embedded at compile time like `worldgen.ron`. The brutal and mild banks share one type, so a key missing from one tone fails to parse. Each key declares its fact slots and flavour slots, and has its feed variants, a `terse` form (facts only) and, where it wants two sentences, a long form for reports, the inspector and biographies (the design's section 3).
  - **`SagaWords`**, exempt from the hash ("words only; the sim never reads it") and not saved.
  - **`Tone`** as a parameter of `describe`, `describe_entries` and the new words functions. Game call sites pass Brutal until step 4.
  - **The `Teller`:**
    - names the dead from `Saga`'s kept deaths;
    - picks keys most specific first, with militia and rebel victims always on their grave keys and first blood or a streak carried by `{moment}`;
    - fills the slots: the per-person weapon word, the old trade's tool, ground by terrain, `{daypart}` and `{then}` from ticks, beast nouns and faction nouns;
    - chooses variants by told-death index, never by RNG, and **fits the feed at read time**: a feed variant that, filled with the real names, runs over 72 characters gives way to the next in the stepped order that fits, and to the key's `terse` form if none does;
    - names battles from their number's facts ("the Second Battle of Loweby Fields");
    - **names the wound** (the design's section 20): `saga::wound(fall, words)` picks a part (head, neck, face, throat, chest, belly, back, arm, hand, leg, eye) from saga.ron's `wounds` table by a stable hash of (victim SpawnId, tick), among the rows for the killer's kind, and decides whether it severs. None for a death with no killer and for militia, rebel and villager victims. `{wound}` fills with that part's phrase. `Teller::told_wound(fall, tone)` is that wound for a told death in brutal, and none in mild; it reads only the record and the table, never which form a line took;
    - **names a severed wound in every brutal telling:** a severing death picks only among its key's variants that carry `{wound}`; its `terse` fallback is the key's severing twin, with `{wound_terse}` ("head off"); and its long form, its report line and the fate clauses of biographies and epitaphs (step 13) carry `{wound}` or `{wound_terse}` too;
    - **fits with names as they stood at the death:** a `Fall` keeps the killer's epithet then (from step 8, when epithets exist), so a later epithet never changes an old line's fit.
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
    - **the voice:** no `!` or `?`, no word in capitals, no ellipsis outside the fold line, and none of the design's winking and pitying words, checked after stripping `{slot}` tokens (so `{irony}` isn't the banned word);
    - **the fit:** every feed variant fits 72 characters (one feed row after the "Day N: " prefix) filled with the data's median given name, family name, epithet, weapon name and place; every key's `terse` form fits 72 filled with the longest; long forms and report-only lines fit 110;
    - every epithet id, deed and earned-trait id has words in both tones (and, as later steps add them, every quirk, mishap kind, irony, mood cause and note kind);
    - **body parts:** no line names a body part in its own text (the nouns come from the `wounds` table); mild lines never use `{wound}` or `{wound_terse}`; every kill key has at least 4 brutal variants with `{wound}`, a severing `terse` twin that fits 72 characters with the longest names and the longest `{wound_terse}`, and a long form and a report line that carry the wound; every part has a phrase, and every severable part a severing one and a terse one.
  - **`wound_is_stable`:** `saga::wound` gives the same wound for the same record in any order and after a load; never the part the victim had lost (`Fall.maim`, filled from step 8b); never for militia or rebels.
  - **`saga_words_golden`:** a fixture bank inside the test and a scripted duel give exact lines in both tones, so edits to the real bank don't churn it.
  - **`tone_is_not_state`.**
  - Re-run `chronicle`.
- **Docs.**
  - DATA.md: a `## saga.ron` section covering every field, with the content rules (graphic toward fighters, grave toward militia and rebels, never sexual violence, torture as spectacle, slurs or children; no gendered words; words may invent the manner, never the facts; times only from `{daypart}` and `{then}`) and the voice's do and don't table.
  - HOWTO: a recipe, "Add a kill line, an epithet or an earned trait", and the voice in one paragraph.
  - GLOSSARY: Phrase bank, Tone, Fact slot, Deadpan dark.
  - ADR 0021: the words-only file, the tone as a viewer setting, and the one voice.
- **Done when.** These read well in both tones and the lint passes:
  - `sim_headless --skirmish --ticks 6000 --saga`;
  - `sim_headless --run --bot --seed 1 --ticks 24000 --chronicle --saga`.
- **Ships as:** kill lines in headless.

### Step 3b: The effects layer: blood, bodies and the CPU backend (M, 5%) — before the kill lines reach the game
- **What.** The whole layer with its CPU and Off backends, and no new dependency (the GPU backend is step 4b).
  - **`EffectsPlugin`** (new `crates/game_render/src/effects/`: requests and settings, the collector, selection, `cpu`, and the solids: bodies, pools, parts).
    - Only `crates/game_app/src/main.rs` adds it, after `DefaultPlugins` and game_render's plugin.
    - Its `finish` reads `RenderAdapter`, `RenderAdapterInfo` and `RenderDevice` (which `RenderPlugin::finish` has put in the main world) and picks the backend by `select_backend(caps, setting, flag)`, a pure function (the design's section 20). Compute is read from the adapter's downlevel flags and the **device's** limits (`RenderDevice::limits()`), GL from `Backends::from(info.backend)`, and a software adapter from `device_type`'s debug name. Until step 4b, a result of GPU draws CPU at its Full caps.
    - Without a `RenderApp` it picks Off and adds nothing.
  - **The effect clock:** game speed (`Time<Virtual>`) up to 1x, real time (`Time<Real>`) above, stopped when paused. Never virtual time divided by the speed, which the catch-up cap slows at 8x.
  - **The collector,** a `FixedLast` system, copies this tick's `Casualties.struck` (step 1), `kills` and `fallen`, with the positions they name, into a render-side buffer, **only when `SimTick` has advanced** since its last copy (a `Local`). After a run ends nothing clears `Casualties`, and the check keeps the last deaths from being sent again every tick. `fallen` gains the victim's `Entity` and SpawnId: exempt scratch, so no hash, save or replay change.
  - **Hits** come from `Casualties.struck`, never from health seen by the renderer, so a deserter's or a recruit's change of stats makes none. A blow on someone killed in the same tick is left to the kill. The renderer keeps each visual's SpawnId (`PersonQuery` gains `&SpawnId`).
  - **Backends:** CPU (a recycled pool of instanced droplets, oldest first, at Full and Reduced caps) and Off (no droplets; bodies, pools and parts as on every backend).
  - **Bodies:**
    - in `sync_visuals`, a dead person's visual becomes a body: tipped over, lingering, sinking, removed;
    - a desertion still vanishes;
    - a captain's banner falls.
  - **Pools:** splat textures from `game_render::art`, quantized fade steps.
  - **Gore follows a `gore` flag,** brutal by default, which step 4 ties to the tone: without gore, dust and falls only.
  - **Bands, caps and ranking** from the new render.ron `effects` (the design's section 20).
  - **`RenderSet::Epoch`** clears bodies, pools, parts and droplets, empties the buffer and resets the collector's last tick.
  - **Flags and tools:**
    - `--effects cpu|off` (`gpu` arrives in step 4b);
    - `--effects-bench <bursts a second>`, a synthetic load at the caps around the camera focus, no battle needed;
    - `stress_log` gains live droplets, bodies, pools, parts, bursts a second and the effects spans.
  - **The stress baseline:** before any of this, the step notes record `--stress 20000`'s worst clash window and the 1,000-a-side window on the build without the layer. That's what NFR-10's +1.5 ms is measured against, so bodies, pools and parts count as well as the spray. It's a command for your RTX 4070 (below).
- **Where.**
  - `crates/game_render/src/`: `effects/` (new), `agents.rs`, `art.rs`, `lib.rs`;
  - `crates/game_app/src/main.rs`;
  - `crates/sim_military/src/combat.rs` (`Casualties.fallen`);
  - `assets/data/render.ron`.
- **Tests** (pure functions, no GPU):
  - `select_backend`: every row, including a Vulkan adapter whose device has WebGL2's limits;
  - `effect_requests_follow_the_records`: a death against a desertion; a landed blow against a deserter's change of stats, starvation, a slip and a brawl; a frozen tick yields nothing; bands, caps and ranking; mild; Off draws no droplets and the same solids;
  - `bodies_and_pools_fade_and_clear`: lifetimes on the effect clock and their floors, the caps, and the epoch;
  - re-run the determinism tests and `save_load` (the scratch is exempt).
- **Smoke runs** (step notes). **On this dev box,** which has mesa's lavapipe (`/usr/share/vulkan/icd.d/lvp_icd.json`), Xvfb and `xvfb-run`, and runs the game at 3 to 4 fps:
  - the game starts and draws a skirmish clash with `--effects off` and `--effects cpu`;
  - automatic selection picks CPU, since lavapipe is a software adapter;
  - `WGPU_SETTINGS_PRIO=webgl2` and `WGPU_BACKEND=gl` start and draw CPU;
  - screenshots of each.
- **Frames (NFR-10).**
  - **Here, part of the gate:** `--effects-bench` medians of the effects spans on CPU at Full and Reduced (at most 1.0 and 0.6 ms), and Reduced against Off on lavapipe, at most +10% median frame time over three 60-second runs. Absolute fps can't be judged on a software renderer, and a real clash is about 15 minutes away at 3 to 4 fps.
  - **On your RTX 4070, handed over:** the stress baseline above, then `--stress 20000` and 1,000 a side framed in the detail band with the CPU backend at Full. These don't block the step; the numbers join its notes when you run them.
- **Docs.**
  - DATA.md: render.ron `effects`.
  - ARCHITECTURE's game-side table: `EffectsPlugin`, the `FixedLast` collector and its tick check, the corpse hand-off.
  - GETTING_STARTED: the flags, software adapters, and the smoke runs under Xvfb.
  - GLOSSARY: Effect request, Effects backend, Effect clock, Body (render-side).
  - TESTING.
  - HOWTO: "Add a combat effect" (a request, its caps, both drawing backends, the band, and the mild variant).
  - ADR 0019's "as built" for the layer.
- **Done when.** In a skirmish, close up, blows spray, deaths burst and leave bodies and pools that fade, deserters vanish as before, a finished run sends nothing more, mild shows dust and falls, CPU and Off draw, CI is green with no GPU, and the dev box's frame checks pass.
- **Ships as:** blood and bodies in every battle (no words yet in the game; step 4 adds them).

### Step 4: Kill lines in the game, with severed parts (M, 7%) — the first slice you can play
- **What.**
  - **Feed lines for told deaths,** read from `Saga` with the feed's own cursor (a `Local`, like the event cursor), so they never crowd `EventFeed`. The same cursor will read saga notes from step 11.
  - **`saga::feed_filter`**, a pure function in sim_military: at 1x–2x, deaths where your people killed or died plus tier A of anyone; at 4x, tier A involving your faction or a legend, plus first blood in your battles; at 8x and fast-forward, tier A involving your faction or a legend.
  - **The rate limit and fold** at every speed: at most one line every 1.5 real seconds per place, the overflow folded into "…and 3 more fell at Loweby". Its numbers sit in `saga.ron`'s `feed` section.
  - **Colour** by kind.
  - **Clicking a line** selects the killer if alive, through a UI-side SpawnId lookup, or else moves the camera there.
  - **The tone setting:** "Words: Brutal / Mild" on the new-game screen and as a switch on the feed. `WordsTone` is a game_ui resource, kept in a preferences file next to the saves (game_app does the file IO). The game also takes `--mild`.
  - **The feed's width stays** at 520 px and one row a line: the fit is settled in step 3 (feed variants, the `terse` fallback chosen at read time), not left to a widening here.
  - **Two log-line fixes:** the military log lines that print a SpawnId where a name belongs (a captain's heir, and the champion's successor). `log_line_shown` is kept in step.
  - **Severed parts that match the line** (the design's section 20):
    - a render-side cursor over `Saga`'s `Fall`s (a `Local`, reset on a new world to the end of the loaded `Saga`) asks `Teller::told_wound(fall, Tone::Brutal)`, the same function that fills the feed's `{wound}`;
    - for a severing wound inside the detail band, it finds the body by SpawnId and swaps it to a headless figure for a head;
    - the part (head, arm, hand or lower leg) arcs from `Fall.at`, bounces and settles on the effect clock, on a path hashed from the `Fall`'s index, on every backend, Off included;
    - a told death throws its larger burst and pool (the stump's fountain comes with the GPU backend, step 4b).
  - **Gore follows the tone.** game_ui mirrors `WordsTone` into game_render's effects settings. Switching to mild despawns pools and parts and gives heads back.
  - **The Effects setting** ("Effects: Full / Reduced / Off") on the new-game screen and in a small view menu by the time bar. Off turns off the spray only; bodies, pools and parts stay. It's stored in the same preferences file, only once you've chosen (until then the machine's choice applies), and `--effects` still overrides it for the session.
- **Where.**
  - `crates/sim_military/src/saga.rs` (`feed_filter`);
  - `crates/game_ui/src/`: `feed.rs`, `screens.rs`, `time_bar.rs` (the view menu);
  - `crates/game_app/src/main.rs`, `crates/game_app/src/saves.rs` (preferences);
  - `crates/game_render/src/effects/` (parts, the saga cursor), `crates/game_render/src/agents.rs` (the headless figure);
  - `crates/sim_military/src/combat.rs` (`succeed`'s line), `crates/sim_military/src/progression/champion.rs`.
- **Tests.**
  - `feed_filter` unit tests per speed;
  - game_ui unit tests: the throttle (tier B dropped at 8x, the fold count) and the preferences round-trip, with tone and Effects;
  - `wound_and_line_agree` (`tests/saga_words.rs`): for every told death of a seeded 1,000-a-side battle, every brutal form that tells it (the fitted feed variant, the `terse` form, the long form and its report line) names a severing wound exactly when `told_wound` severs, and the part is the one game_render's request builder sends (a pure function there); the same after the killer is given a new epithet by hand; mild names none, and militia and rebels never get one;
  - game_render unit tests: a severing request makes one part and a headless body; a tone switch clears parts and pools;
  - a manual check in the skirmish and a run, at 1x and 8x, in both tones, with screenshots in the step notes.
- **Docs.**
  - ARCHITECTURE's game-side table (feed; the saga cursor in the effects layer);
  - DATA.md: saga.ron `wounds`, render.ron's part numbers;
  - TESTING;
  - README's controls if a key is added.
- **Done when.**
  - Notable deaths are told in the feed without flooding it.
  - A told kill close up throws exactly the part its line names, and every later telling of it names the same part.
  - Switching tone rewrites the visible lines and the gore.
  - The tone and Effects choices survive a restart.
- **Ships as:** **the brutal kill messages you asked for,** with their blood and severed parts, in the skirmish and in runs. A natural point for a quick look.

### Step 4b: The GPU backend: bevy_hanabi (S, 3%)
- **What.** Denser sprays where the device can run them. Nothing else changes: bodies, pools and parts are step 3b's and step 4's on every backend.
  - **The dependency** (approved 2026-10-12, [ADR 0019](adr/0019-combat-effects-and-bevy-hanabi.md)): `bevy_hanabi = { version = "=0.19.0", default-features = false, features = ["3d"] }` in `[workspace.dependencies]`, used by game_render only, in one module (`effects/gpu.rs`) and nowhere else. No cargo feature: the module is the seam if a Bevy upgrade ever outpaces bevy_hanabi. Its checks, in the step notes:
    - **The lockfile** gains exactly bevy_hanabi 0.19.0, anyhow and rand_pcg. `cargo tree -e features -i <crate>` for wgpu, naga, naga_oil, rand, bevy_render and bevy_core_pipeline shows no feature change, before and after.
    - **Licences.** All three are MIT OR Apache-2.0 (`cargo metadata`). The repo has no cargo-deny, so the check is that read, kept in the notes.
    - **Build time.** A clean and an incremental `cargo build -j 2 -p game_app`, before and after.
    - **Lint.** `cargo clippy -j 2 --workspace --all-targets -- -D warnings`, as CI runs it. Hanabi's own `#![deny(warnings)]` doesn't reach us, since cargo caps lints for dependencies.
    - **No GPU in headless or CI:** `cargo tree -p sim_headless -i bevy_hanabi` and `cargo tree -p integration_tests -i bevy_hanabi` report that no package matches (cargo exits with an error, which here is the pass; a script can instead count `bevy_hanabi` lines in `cargo tree -p sim_headless --prefix none` and expect 0). CI's workflow is unchanged and stays green: it compiles Hanabi into game_render and never runs it.
  - **The gate.** With compute, and unless forced to `cpu` or `off`, `EffectsPlugin::finish` calls `app.add_plugins(HanabiPlugin)` (its `build` runs and it's registered, so `is_plugin_added` works and a second add panics) and then `HanabiPlugin.finish(app)` by hand, since the finish loop never reaches a plugin added during it. Never anywhere else.
  - **Four `EffectAsset`s** (hit spray, kill burst, stump fountain, dust), each with its `capacity` from render.ron (1,024, 4,096, 512 and 1,024 to start), in global simulation space, and about 40 pooled instances (12, 12, 4 and 12), fired round-robin with `EffectSpawner::reset()`. The GPU burst caps (8 hits and 2 kills a frame) fit the capacities, and a test checks the arithmetic over render.ron. The instances exist only while the GPU backend draws (switching to Reduced or Off despawns them), and are spawned at load so their shaders compile before the first battle. Imports are explicit (`bevy_hanabi::Gradient`, `bevy_hanabi::AlphaMode`), never the prelude, which clashes with Bevy's.
  - **The clock on the GPU:** a system in `Last` sets `Time<EffectSimulation>`'s relative speed to 1 at 1x and below, and above 1x to the frame's real delta ÷ virtual delta (clamped), so Hanabi follows the same effect clock as the CPU backend.
  - **The stump fountain** on severings (step 4's parts), and dust in the mild tone.
  - **`--effects gpu`;** without compute it logs a warning and draws CPU.
  - **The spans:** Hanabi's main-world systems (its clock in `First`, `EffectSystems` in `PreUpdate` and `PostUpdate`) are counted in the effects' CPU budget; its render-world systems are logged beside them.
- **Where.** The root `Cargo.toml` and `crates/game_render/Cargo.toml`; `crates/game_render/src/effects/gpu.rs` (new) and `effects/mod.rs` (the gate); `crates/game_app/src/main.rs` (the flag); `assets/data/render.ron`.
- **Tests.** `select_backend` gains its GPU rows; a render.ron check that each GPU burst cap fits its asset's capacity; the rest are unchanged (no GPU in any test).
- **Smoke runs** (step notes, on the dev box): `--effects gpu` (lavapipe has compute shaders) starts and draws; automatic still picks CPU (software adapter); `WGPU_SETTINGS_PRIO=webgl2` and `WGPU_BACKEND=gl` still draw CPU and never add Hanabi, with no validation error quitting the game; screenshots.
- **Frames (NFR-10).** **Here:** `--effects-bench` medians with `--effects gpu` (ours and Hanabi's main-world spans at most 0.5 ms). **On your RTX 4070, handed over:** automatic selection picks GPU; `--stress 20000` with Full effects against step 3b's baseline (worst clash window at 60 fps or more, at most +1.5 ms), and 1,000 a side framed in the detail band at 60 fps.
- **Docs.** DATA.md (the GPU rows of render.ron `effects`); ARCHITECTURE's game-side table (the gate); GETTING_STARTED (`--effects gpu`); TESTING; ADR 0019's "as built" for the dependency and the gate.
- **Done when.** The dependency checks are in the notes, CI is green with no GPU, every backend draws on the dev box (GPU forced, CPU automatic, CPU under WebGL2 limits and on GL), and the dev box's frame checks pass.
- **Ships as:** the full spray, where the machine can draw it.

### Step 5: Quirks (M, 6%; run only; bot_run)
- **What.**
  - **New module `sim_agents::quirks`:** the `Quirks` component (one or two ids, two bytes), the deck from `villagers.ron` `quirks` (`two_chance`, rows of id, key, weight, `excludes` and the villager effects; `#[serde(default)]`), and `deal`, drawing from `SimSeed::stream(rng_key("sim_agents::quirks") ^ SpawnId, 0)`. A deal depends only on the seed, the deck and the SpawnId.
  - **Who is dealt, in a run only:** everyone alive at the end of `setup_run` (after `sellswords::setup`, the last setup spawn), and the people `spawn_army` spawns later in the run. Never beasts; Sweetheart is skipped for anyone without a home. `populate` is untouched (G10).
  - **Villager effects** in `ai::decide` (work by phase, farm and pasture work, idle by phase, militia courage, flee radius), `update_needs` (hunger, rest gained asleep), `drift_loyalty` (Pious) and the recruit dealing's willingness (Sweetheart, compared against the same draw as today). Each is skipped entirely when the villager has no `Quirks`. `Factors` grows from four slots to five, so a quirk shows as its own factor in the `DecisionTrace`.
  - **The soldier side of the deck** lives in `progression.ron` `saga.quirks` (soldier effects and mishap invitations, keyed by quirk id and checked against the deck when the data loads), so sim_agents holds no sim_military id and a soldier-side tweak never edits villagers.ron.
  - **`Veteran` gains the cached nudges** (break-point offset, morale-target offset, desertion multiplier, hunger multiplier, and the glutton's fed-morale rule), **fixed-point** (`i8` hundredths, `i16` thousandths) so `Veteran` keeps `Eq` and stays small, written only by serial steps: the deal, and `enlist` (rebuilt from `Quirks`). `soldiers::decide` gains a read of `Veteran` for the break point (its query doesn't hold one today); `soldier_upkeep`, which already reads it, takes the rest. Earned traits (step 9) and moods (step 6) write the same cache later. The sums are clamped as the design's section 8 says.
  - **New progress slot 17, `saga::settle`** (hourly, every preset, after `director::step` and M4a's slots 13–16). Here it runs the Sweetheart's walk home (a follower out of battle, within 12 tiles of home: 5% a day, from `sim_military::saga::homesick ^ SpawnId` at the tick). Slot 13 runs after this tick's `apply_desertions`, and `tally` clears `Deserters` (exempt, reset on load) at the start of the next tick, so the walk home **sets the hashed, saved `Follower.deserting`** and records the cause; the next tick's `tally` and `apply_desertions` send them home like any deserter.
  - **Acting from later steps:** Fears geese's near-farm mood and Pious's plunder mood need moods (step 6); Vain needs epithets (step 8); the mishap invitations need mishaps (step 6).
  - **Inspector:** the quirk line with tooltips, for villagers and soldiers.
  - **`bot_run`** prints quirk counts per seed.
- **Where.** `crates/sim_agents/src/quirks.rs` (new), `crates/sim_agents/src/ai.rs`, `crates/sim_agents/src/act.rs` (`update_needs`), `crates/sim_agents/src/loyalty.rs` (`drift_loyalty`), `crates/sim_agents/src/components.rs` (`Factors`), `crates/sim_agents/src/data.rs`, `crates/sim_agents/src/lib.rs` (registration); `assets/data/villagers.ron`; in `crates/sim_military/src/`: `progression/mod.rs` (`setup_run`, slot 17, the `saga.quirks` rows), `spawn.rs`, `veterans.rs` (the nudges), `soldiers.rs`, `upkeep.rs`, `orders.rs` (`enlist`), `saga.rs` (`settle`); `assets/data/progression.ron` (`saga.quirks`); `assets/data/saga.ron` (quirk words); `crates/game_ui/src/panels.rs`.
- **Tests (`tests/tests/quirks.rs`).**
  - `quirks_are_dealt_per_person`: the same seed deals the same quirks; dealing all at once, in reverse order or at spawn gives the same result; one or two each; exclusions hold; beasts, the developed world, the skirmish and `--battle` have none.
  - `quirks_act_through_their_holder`: each row's effect on a hand-built world (a glutton's hunger 1.15× exactly, a coward's break point, a drunkard's morning work score, and so on), and a person without quirks bit-identical to today.
  - `sweetheart_walks_home`: near home, out of battle, the walk home happens at the documented rate over many hours; far from home, or in battle, never; a walk home drawn at slot 17 survives a save and load before the next tick, and the loaded game matches the original.
  - Re-run `village_day`, `famine`, `army_march`, `posture`, `starving_army` and `orders`; `save_load`; the determinism coverage tests.
  - Re-run the run-preset tests ("What stays unchanged"): `champion`, `chronicle`, `conquest`, `director`, `dominion`, `gather`, `invitation`, `monuments`, `rival`, `run_start`, `sellswords` and `wanderers`. One that moves only because the run took another path opts out with an empty `quirks.deck`; `conquest` and `invitation`, which guard loyalty drift, run with Pious on.
- **Docs.** DATA.md (`villagers.ron` `quirks`, `progression.ron` `saga.quirks`, and the quirk words); ARCHITECTURE: slot 17, the deal in setup and spawn, the cached nudges; GLOSSARY: Quirk, Cached nudges; TESTING; HOWTO: "Add a quirk"; ADR 0021: quirks dealt per SpawnId, and the amendment to "no new RNG" with the stream keys.
- **Done when.** Every person in a run has one or two quirks, the same for a seed whatever the order; each effect is measured; the guarded stdout is unchanged; `village_day` and `famine` pass unchanged; the run-island cost is within NFR-09; `bot_run` still wins 3 of 5.
- **Ships as:** villagers who differ. The inspector shows their quirks, and they act on them.

### Step 6: Mishaps (M, 6%; run only; bot_run)
- **What.**
  - **The mishap clock** in `director::step` (slot 12), after the raider check: nothing in the first 24 hours or while a run end is pending; `spacing_hours`, `chance_per_hour`, `max_per_day` and `tension_below` per personality; the battle kinds ignore tension and are capped at one per battle; **the side is drawn first** (the player's with `player_share`, 2 in 3, else the side at war with the player; the only side with a candidate if one has none), then an army within it by kind weight and quirk holders; beasts, guards and bands for hire are never subjects.
  - **New module `sim_military::mishaps`** with the kinds of the design's section 11, except the camp brawl (step 12): drunk before battle, bad mutton, a dog at the rations, a wedding on the road, a slip in the shield wall, the victory feast, wandered off, dice, and the goose rout. Each has its condition, its effect, its drawn **subjects** (only they count it in `Service.mishaps`; army-wide moods count for nobody; the dice loser, never the winner), and its guards, enforced in code: never the champion, a captain or a legend as subject; deaths only from the slip and the feast, one ordinary soldier, in armies of 10 or more, one a game day; food never below a day of the army's need; delays only for marches with no foe near and no open request at their goal; one mishap a day per army. The dice leave no grudge until step 12, and their words pick the variant by outcome.
  - **Two kinds are new movement,** said so in the design: the goose rout is a **timed flee** (a `geese` mood entry carrying a flee point and an end tick, honoured by `soldiers::decide` while no foe is near, through the rout's movement but marking no `routed`, counting toward no break and opening no desertion roll); wandered off is a **direct move** (set `Position` to a passable tile 4 to 8 tiles away with no hostile army within `threat.radius`, reset `Motion`, and let the rally field bring them back).
  - **`Mood`** (sparse): up to four timed entries, clamped, expired hourly in `saga::settle`, and written into `Veteran`'s cached nudges. Here it carries the mishap moods (`sick`, `hungover`, `wedding`, `feasted`, `dice`, `geese`) and the quirk moods that need it (Fears geese near farms, Pious after a plunder).
  - **`Captain::halt_until`**, checked by `run_orders` for a halted march, and cleared by any new command for that captain in `apply_commands` and by a hostile army within `threat.radius` in `run_orders`.
  - **`SimEventKind::Mishap`**, chronicled and not notable, so the quiet clock, the run's first event and fast-forward don't see it.
  - **A mishap's death** sets health to 0 and notes its manner in `Saga` (an in-place part of its hash); `find_fallen` finds it on the next tick (G4), and `casualties` tells it on its `mishap_*` key as a tier-A `Fall`. **It's told once:** a lethal mishap pushes no `Mishap` event, so its `Fall` is its one line.
  - **The feed:** mishap lines, dimmer; at 8x only those that befell your champion's army (a death of yours is already a tier-A `Fall`); at most one every 20 real seconds at any speed; and the **hard cap at 8x, 2 saga lines a game day** (mishaps now, weapons and kin lines as they arrive), in the feed's throttle, on every difficulty.
  - **Inspector:** the moods a mishap left, with their time to run.
  - **`bot_run`** prints mishaps by kind per seed and checks the guards over every seed.
- **Where.** `crates/sim_military/src/director.rs`, `crates/sim_military/src/mishaps.rs` (new), `crates/sim_military/src/saga.rs` (`Mood`, `settle`), `crates/sim_military/src/orders.rs` (`halt_until`, cleared in `apply_commands` and `run_orders`), `crates/sim_military/src/soldiers.rs` (the goose rout's timed flee), `crates/sim_military/src/components.rs` (`Captain`), `crates/sim_military/src/combat.rs` (mishap deaths), `crates/sim_military/src/upkeep.rs` and `crates/sim_military/src/veterans.rs` (moods in the nudges), `crates/sim_core/src/events.rs` (`Mishap`, `ALL`, `name`, `chronicled`, `notable`); `assets/data/director.ron`; `assets/data/saga.ron`; `crates/game_ui/src/feed.rs`, `crates/game_ui/src/panels.rs`.
- **Tests (`tests/tests/mishaps.rs`).**
  - `each_mishap_does_what_it_says`: each kind on a hand-built world, with its condition false (nothing happens) and then true (its effect, exactly). Also: a lethal slip leaves one `Fall` and no `Mishap` event; a new command, or a hostile army arriving, ends a wedding's halt; a march to a settlement with an open request gets no wedding; the goose rout's flee ends on time or when a foe comes near, and marks no `routed`; a sleepwalker never lands near a hostile army; only drawn subjects count the mishap.
  - `mishaps_never_wipe_a_run`: on all three personalities (steady, chaotic, harsh) with compressed data overrides (`free_hours` 0, `spacing_hours` 1, `chance_per_hour` 1.0), under 60 s (G34), and again in `bot_run` over every seed: no champion, captain or legend touched, at most one death per mishap and one a game day, no baggage under the floor, no delay with a foe near, at most `max_per_day`, none in the first `free_hours`.
  - `mishaps_are_not_notable`: a unit test that a chronicled `Mishap` leaves `Chronicle::last_notable`, `Director.first_event` and `eventful()` unchanged; and a scenario running the mishap clock with every kind's effect stubbed out (rolled and recorded, never applied), whose quiet clock and raider spawns match the same run with no mishap kinds. (A run with real mishaps differs from one without, by design: their effects change its path.)
  - A `game_ui` test of the 8x hard cap on saga lines.
  - Re-run `director`, `chronicle` and `save_load`, and the run-preset tests ("What stays unchanged"); one that moves only because the run took another path opts out with an empty `mishap_kinds`.
- **Docs.** DATA.md (`director.ron` `mishaps`, `mishap_rules`, `mishap_kinds`; the mishap banks); ARCHITECTURE: slot 12's mishap clock, slot 17's mood expiry; GLOSSARY: Mishap, Mood, Low-tension beat; TESTING; HOWTO: "Add a mishap"; ADR 0021: mishaps aren't notable, the never-run-ending rules, the stream keys.
- **Done when.** Bot runs show about one mishap a game day on steady, with none breaking a guard over every seed, and the guards hold on chaotic and harsh too; each kind's line matches its effect; `bot_run` still wins 3 of 5; mishaps never count as notable; the saga's own spans stay under 2% of a bot run's live tick.
- **Ships as:** runs that go wrong in small ways, and say so.

### Step 7: Battle moments (M, 6%; bot_run)
- **What.**
  - **`BattleReport` gains the moments:** `battle` (the number, which leads to the name, first blood, the told deaths and the odd in `Saga`), `broke`, `drew_first`, `best`, `ran_first`, `hurt` (the wounded), `deserted`, `wiped`, and `earned` (empty until step 8).
  - **`Captain` gains** `battle_ran_first`, `battle_deserted`, `starved` (the army's run-long count of hunger deaths, passed to an heir) and `starved_told` (how many of them the hourly summary has told).
  - **`build_index` names who ran first.** It already finds the moment.
  - **`apply_desertions` counts deserters** per army.
  - **`end_battles`:**
    - counts a wound for each survivor whose `hurt` reached a quarter of their health;
    - fills the report;
    - clears the new fields with `fought`.
  - **Slot 13 (`saga::settle`)** gains the hourly starvation summary `Starved`, feed-only: `starved` minus `starved_told`, leaving out anyone told by name.
  - **A wiped army in a run:** when no heir is left, `succeed` writes the report (`wiped`) and its `Battle` entry.
  - **Words:** the report's moments, "N more died", the odd (a mishap during the battle), the `Battle` line's battle name and faction nouns, the end screen's "Your battles", and the feed's battle line opening into the full report.
- **Where.**
  - in `crates/sim_core/src/`: `events.rs` (`BattleReport`, its hash and save; the `Starved` kind, `ALL`, `name`, `chronicled`);
  - in `crates/sim_military/src/`:
    - `index.rs`, `upkeep.rs`, `veterans.rs`;
    - `combat.rs` (`succeed`);
    - `saga.rs` (slot 17);
    - `chronicle.rs`;
  - in `crates/game_ui/src/`: `feed.rs` and `screens.rs`.
- **Tests.** `moments_reach_the_report`, `wiped_army_gets_a_report`, `starving_is_told_once_an_hour`, the event order test. Re-run `chronicle`, `director` and `save_load`.
- **Docs.**
  - ARCHITECTURE: row 6 (slot 17's starvation lines) and row 2 (who ran first).
  - GLOSSARY: Moment.
  - TESTING.
  - ADR 0021: the wiped report amends ADR 0017's "as built".
- **Gate note.** A wiped army's `Battle` is notable, so pacing can move: re-run `bot_run`.
- **Done when.** Every report tells its moments in both tones, both sides' reports of a fight share its name, and a wiped army is reported in a run.

### Step 8: Deeds, epithets, absurd deaths and rescues (L, 8%; run only; bot_run)
- **What.**
  - **Deeds** (`SagaDeed`) are recorded once each, as (deed id, tick, battle number or nearest settlement, victim `NameId`):
    - at the kill: first blood, slayings, shield, five in a fight, doorwarden, kinslayer;
    - at `end_battles`: held the line, survivor, ran first, last stand (a kill after the army broke, and alive at the end);
    - in `run_orders`, on the captain: plunderer, breadgiver;
    - in `requests::complete`: rescuer.
  - **The rest of `Service`:** `joined` (copied from `Veteran.joined`), `rejoined`, `held`, `fled`, `mishaps` (per kind, counted for drawn subjects only), `fights` (the latest 16 battles with their kills).
  - **Fame** is worked out from the record.
  - **Epithet rows** (`saga.epithets`: a stable `id`, `kind` honour or mock, a `rank` and a rule): ids 0–18 honourable at ranks 11–29, ids 19–26 mocking at ranks 0–7; ranks 8–10 are left for step 8b's scar epithets. They're applied by one function, `saga::judge`:
    - from `end_battles`, for each survivor, so the report lists what was earned;
    - from slot 17, for every other record changed since the last hour (militia after a kill, captains after dealings, anyone after a mishap or a re-enlistment).
  - **Ironies** (`saga.ironies`): ids 0–6 and 8–9 of the design's section 7, classified in `casualties`' second pass from the records, never for militia, rebel or villager victims; +50 to the score; the `absurd_*` keys and the `{irony}` slot. Irony 1 reads `Veteran.joined` (step 2), so a recruit killed on the day of joining qualifies with no record. Irony 6 is the one hunger death classified: an army's first (`Captain::starved` was 0) when the victim is a Glutton, told as a tier-A `Fall`. Irony 7 (own weapon) comes with step 11.
  - **Mocking epithets count subjects:** the Unlucky (three mishaps), Empty-Purse (three dice losses) and Bread-Belly (a glutton sick from bad mutton twice) read `Service.mishaps`, which counts drawn subjects only.
  - **Rescues:** an answered `DriveOff` records a rescue **only if** the answering captain's army fought an army at war with the player since the request was posted, with the battle's first blow within `threat.radius` of the settlement's centre (a request can complete after a band withdraws on its own, with nobody fighting). Then its rescuers (those who fought it) and heroes go in `Saga.rescues`, the heroes earn the Rescuer deed, and the `RequestDone` line names the hero. Without such a battle nothing is recorded and the line keeps its built words.
  - **Vain acts now:** the morale with an honourable epithet, `envy` when a comrade is named, `mocked` on a mocking epithet.
  - **`Named`**, chronicled but not notable, with the new `Cause::Earned { deed }`. `notable()` excludes the new chronicled kinds, by adding them to the `quiet()` list that M4a's step 1 introduces (Forsaken, Built, StewardNamed), so this step, `Mishap`, `TraitEarned` and `LegendRose` all use one mechanism.
  - **A `Fall` keeps the killer's epithet as it stood** (step 2's field), so the Teller fits a told line with the names of the day it was told, and a later epithet never rewrites it.
  - **Inspector:** the epithet after the name, then kills, battles, wounds and deeds.
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
  - `rescues_are_recorded`: an answered drive-off names a legend first, then the named by fame, then the best killer; the captain when nobody who fought is named; a battle inside `threat.radius` but nearest a neighbour counts; a request answered after the band withdrew without a fight records no rescue, no deed and no hero in its line;
  - `every_cause_is_worded`;
  - the event order test;
  - `chronicle` and `director`;
  - `bot_run` prints epithet counts (honourable and mocking) and ironies; Vain's morale may move it.
- **Docs.** DATA.md; GLOSSARY: Saga deed (kept distinct from goodwill's **Deed**), Fame (a per-person score with no unlocks, not FUN_DESIGN's renown), Epithet, Mocking epithet, Irony, Rescue; TESTING; ADR 0021.
- **Done when.** A bot run of seed 1 earns several epithets, some mocking, each with its reason and place; at least one absurd death appears over the five seeds; drive-offs won in a fight name their heroes, and none is credited without one; the inspector shows the record.
- **Ships as:** names you notice, flattering or not, and deaths people retell.

### Step 8b: Maimed survivors (M, 5%; maims recorded in every preset, effects run only; bot_run) — the end of part 1
- **What** (the design's section 21):
  - **The check.** In the strike loop, a blow that takes a living person with a `Veteran` past `saga.maims.hurt` (0.5) of their health lost to blows this battle pushes a maim check onto `Casualties.maim_checks` (exempt scratch, cleared at the top of `resolve_strikes`). That costs a compare per blow and a push only on the crossing. `Veteran.hurt` (step 2) resets each battle, so a person is checked at most once a battle.
  - **The roll, in its own step.** A new serial system, `combat::maims`, chained after `casualties` in `MilitarySet::Combat` with `run_if` any maim checks. Not a pass of `casualties`: that runs only when someone fell (`anyone_fell`), and most battle ticks have no death. In SpawnId order, for those still alive, it draws `sim_military::saga::maim ^ SpawnId` at the tick: maimed with `chance` (0.25), and which part by weight (hand 4, eye 3, leg 3). It's never a beast or militia, and never someone already maimed.
  - **Once in a life.** Wounds never heal, so after a maim less than half their health is left, and a second crossing would kill them first; the code also skips anyone with a maim.
  - **Records.** `Service.maim`: the part, the tick and the battle, if any. A saga note `Maimed`. `Casualties.maims` (exempt scratch) for the renderer. `Fall` copies the part at death, so `saga::wound` never takes a lost part.
  - **Effects, run only,** through `Veteran`'s cached nudges (three new fixed-point multipliers, rebuilt on enlisting):
    - a lost hand gives attack ×0.85 on the striker's `Veteran` in `resolve_strikes`, already borrowed;
    - a lost eye gives defence ×0.9 on the victim's `Veteran`, already borrowed;
    - a lost leg gives pace ×0.85 to a follower in a chase, a rout and catching up (`move_soldiers`, one new optional read), and to a captain or the champion only in a rout (`move_captains`): a captain's march pace is the whole army's, so a one-legged captain must never slow the march.
    
    All are exact no-ops at 1.0, and nothing changes outside a run.
  - **Scar epithets.** Ids 27 to 29 (One-Hand, One-Eye, One-Leg; kind `scar`) at ranks 8 to 10, which step 8 left free. Vain takes one as mocked.
  - **Words:** the maim notes (feed form and long form, both tones), the `{maim}` slot, the epithet lines, and the inspector line ("Lost a hand (Loweby, day 4) · Attack −15%"). The feed shows your people's maims at 1x–4x, never at 8x.
  - **Visuals:**
    - the moment: in brutal, a hand or lower leg flies, or a burst at the head for an eye, with a small pool; in mild, a dust flick;
    - after: in the detail band, both tones and every backend, an eye patch and a bound stump (two figure mesh variants) and a limp (a tilt in `place_visuals`), read from `Service.maim`.
  - **`bot_run`** prints maims by part per seed. `SAVE_FORMAT` goes up.
  - **Part 1's playtest checklist** (the first half of `docs/M4C_PLAYTEST.md`), so you can stop here for a look.
- **Where.**
  - `crates/sim_military/src/`: `combat.rs` (the check, the `maims` step), `lib.rs` (its place in the chain), `saga.rs` (records, notes, judge's scar rows), `veterans.rs` (the multipliers), `soldiers.rs` (pace), `orders.rs` (`enlist` rebuilds the cache), `save.rs`;
  - `crates/sim_core/src/save.rs` (`SAVE_FORMAT`);
  - `assets/data/progression.ron` (`saga.maims`, the epithet rows), `assets/data/saga.ron`;
  - `crates/game_render/src/effects/`, `crates/game_render/src/agents.rs` (the figure variants), `crates/game_render/src/art.rs`;
  - `crates/game_ui/src/panels.rs`.
- **Tests** (`tests/tests/maims.rs`):
  - `maims_follow_the_rule`: the crossing, the roll and its stream; a crossing on a tick when nobody dies is rolled on that tick; no maim on a killing blow, below the threshold, or for someone already maimed; never beasts or militia; the same seed, the same maims.
  - `maims_act_through_their_holder`: ×0.85 attack, ×0.9 defence and ×0.85 pace in a chase and a rout, exactly; a one-legged captain marches the army at full pace and routs at ×0.85.
  - `maims_are_exact_no_ops`: outside a run, and for anyone without one, bit-identical blows and movement.
  - `maim_words_and_visuals_agree`: the note's part, the epithet by rank, no later wound on a lost part, and the renderer's request carrying the same part.
  - Also: `epithets_follow_rank` extended with the scar rows; `save_load`; the determinism coverage tests; `army_march`, `posture`, `starving_army` and `orders` re-run; the three guarded stdout comparisons (maims change nothing outside a run).
- **Docs.** DATA.md (`saga.maims`, the scar rows, the maim words); ARCHITECTURE (row 5: the check, and `combat::maims` after `casualties` with its run condition); GLOSSARY: Maim, Scar epithet; TESTING; ADR 0021 (maims are recorded in every preset and act in a run; the stream key; the interplay with "wounds never heal").
- **Gate note.** The penalties change fights in a run: re-run `bot_run`, with maims by part per seed in the step notes.
- **Done when.** Bot runs show a handful of maims a run, each with its line, epithet and figure; nobody is maimed twice; the penalties measure exactly; `bot_run` still wins 3 of 5.
- **Ships as:** survivors who carry their battles. **Part 1 is done:** tick its criteria in SEED.md and hand over part 1's playtest.

### Part 2: Legends and kin

### Step 9: Earned traits (M, 5%; run only; bot_run)
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
- **Docs.** DATA.md; GLOSSARY: Earned trait, Mark, Eligible; TESTING; ADR 0021.
- **Done when.**
  - Traits are earned at the design's target rates: at the end of a won run, at most a quarter of eligible veterans hold one, and Cruel is no more common than Brave. Counts per bot seed go in the step notes.
  - Each trait acts only through its holder.
  - `bot_run` still wins 3 of 5, and NFR-09 holds with `--saga-bench`.

### Step 10: Legends (M, 5%; run only; bot_run)
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
- **Docs.** DATA.md; ARCHITECTURE: row 2 (`LegendIndex`); GLOSSARY: Legend, kept distinct from Hero, Champion, Veteran and Elite; TESTING; ADR 0021.
- **Done when.** Legends rise in most bot runs, steady and shake as specified, and a legend's death pauses your game.

### Step 10b: The kill cam (S, 2%)
- **What** (the design's section 20):
  - **The policy.** New `game_ui::kill_cam`, a pure decision over this frame's deaths, the speed, `FastForward`, the pause, `InputGate` and its cooldown.
    - **Triggers:** a captain or a legend killed by a blow, found by the victim's role in their `Fall` (read with a cursor, using `Fall.at`), never by its tier, which also covers a legend's milestone kill of an ordinary soldier, mishap deaths and a Glutton's starvation; and `ChampionFell` (which carries `at` and its cause) when the champion was killed by a blow. The death must be on screen (`Camera::world_to_viewport`) and within render.ron's `detail_distance` of the camera.
    - **When:** at 1x–4x only, never in fast-forward, paused, behind a dialog or during the opening shot.
    - **How often:** once per 10 real seconds; the champion before a legend before a captain, nearest first.
  - **Slow motion.** `Time<Virtual>` goes to 0.25 for 1 real second, eased. The time bar lights the speed it will return to. Any speed key, pause or camera input ends it and keeps your choice. The speed is restored only if it's still at 0.25 and unpaused.
  - **The nudge.** New `CameraNudge` in `game_render::camera`: the rig's targets move 35% toward the death and zoom in 15%, on `Time<Real>`, with no snap back.
  - **The held pause.** When the death that started the kill cam auto-pauses (`ChampionFell`, your `LegendFell`), the feed holds the pause until the kill cam ends, and applies it when the kill cam ends for any reason.
  - **Settings.** "Kill cam: On / Off" on the new-game screen and in the view menu, in the preferences file; `--no-kill-cam`. Its numbers are in render.ron `kill_cam`.
- **Where.**
  - `crates/game_ui/src/`: `kill_cam.rs` (new), `feed.rs` (the held pause), `time_bar.rs` (the lit speed, the view menu), `screens.rs`;
  - `crates/game_render/src/camera.rs`;
  - `crates/game_app/src/main.rs` and `crates/game_app/src/saves.rs` (flag and preferences);
  - `assets/data/render.ron`.
- **Tests.** game_ui unit tests (`kill_cam_*`):
  - each trigger, and none for an ordinary soldier, a legend's milestone kill of an ordinary soldier, or a starving captain;
  - never at 8x, in fast-forward, paused or behind a dialog;
  - the distance and on-screen test;
  - the cooldown, and a chain of heirs giving one kill cam;
  - restoring only when untouched;
  - a held pause always applied;
  - the preferences round-trip with the kill cam.
- **Manual check:** a skirmish and a run at 1x, 2x, 4x and 8x, with screenshots or a short capture in the step notes.
- **Docs.** DATA.md (render.ron `kill_cam`); ARCHITECTURE's game-side table; GLOSSARY: Kill cam; TESTING; README's controls; ADR 0019's "as built" (the kill cam is a `Time<Virtual>` effect).
- **Done when.** A captain, the champion or a legend dying near the camera at 1x–4x slows the game for a second and draws the camera in. It never fires at 8x or in fast-forward. Your speed comes back unless you changed it. A death that pauses still pauses, after the slow motion.

### Step 11: Named weapons (M, 5%; run only; bot_run)
- **What.**
  - **`NamedWeapon(index)`** (sparse) on a holder, always with `Equipment.weapon` true, and **`Saga.weapons`**: the first namer's weapon word, the naming, up to 8 holders, kills while named, up to 8 notable kills, and the place.
  - **Naming** in `casualties`' first pass: an armed killer of a captain, the champion, a legend or a monument's beast names their weapon, at most 2 a battle and 24 a run (`saga.weapons`).
  - **Fates** in the second pass, the first that applies: looted (a soldier killer takes it in hand; a militiaman's village puts it in its storehouse), passed on (an unarmed member of the holder's household in the same army, else the nearest unarmed member within 3 tiles), into the baggage, or lost where its holder fell.
  - **Out of a hand:** a deserter takes it home (`apply_desertions`) and brings it back on re-enlisting (`enlist`); `arm_followers` and an `Equip` dealing hand named weapons out first; a barter spends them last, and one spent is sold into that settlement's storehouse.
  - **Reconciliation** in slot 17: a place holding fewer `weapons` units than the named weapons placed there marks the newest one lost.
  - **Hashing:** a weapon's record changes in place (place, holders, kills), so `Saga.weapons` is in the in-place part of `Saga`'s hash (step 2), hashed in full; at most 24.
  - **Irony 7** (killed with a weapon they once carried); `{weapon}` fills with the name; Vain's morale with a named weapon.
  - **Saga notes** (`Saga.notes`, read by the feed's own cursor): namings, passings, lootings, sales, losses.
  - **Inspector:** the weapon line and its history; the settlement panel's relics.
  - **`--saga-bench`** gains three named weapons per army.
- **Where.** In `crates/sim_military/src/`: `saga.rs`, `combat.rs`, `orders.rs` (`arm_followers`, `equip`, barter, `enlist`), `upkeep.rs` (`apply_desertions`); `assets/data/progression.ron` (`saga.weapons`); `assets/data/saga.ron` (name patterns, fate lines); `crates/game_ui/src/feed.rs`, `crates/game_ui/src/panels.rs`.
- **Tests (`tests/tests/weapons.rs`).**
  - `a_weapon_is_named_by_a_notable_kill`: a captain's killer names their weapon; an ordinary kill doesn't; an unarmed killer names nothing; the caps hold.
  - `named_weapons_change_hands`: looted by a soldier and by militia, passed to kin and to the nearest comrade, into the baggage and out first, sold last in a barter, taken home and back, lost when the army is wiped; after every move it's in exactly one place, and the places' `weapons` units cover it.
  - Re-run `orders` (barter and equip unchanged without named weapons), `save_load`.
- **Docs.** DATA.md; GLOSSARY: Named weapon, Saga note; TESTING; ADR 0021: a weapon's identity is a sparse index beside the `Equipment` bool.
- **Gate note.** Looting arms the looter, so a run can move: re-run `bot_run`, with weapons named and their fates per seed in the step notes.
- **Done when.** Bot runs name a handful of weapons, each with a history that matches what the sim did, and none is ever in two places.

### Step 12: Kin, friends, rivals and moods (L, 8%; run only; bot_run)
- **What.**
  - **Kin** through `Home.residents` (one household), never stored: in `casualties`' second pass, the death of a villager-recruit gives their enlisted kin `grief`, and those within 12 tiles **vengeance** for the rest of the battle (morale +0.05 once, and the aim at the killer in `soldiers::decide`); killing the killer is the Avenged kin deed and a told `vengeance` line. A militia or rebel killed by a blow of a soldier's own faction gives that soldier's enlisted kin `kin_killed` (−0.08, desertion ×2, 3 days).
  - **`Bonds`** (sparse, at most three): friends at `end_battles` (two held survivors within 2 tiles, in an army that broke or lost a quarter); rivals from dice (the mishap now leaves its grudge, and its words a grudge variant), from running first, and from a named weapon passed outside the household. Feuds end when both hold in one battle, when one dies, or in a brawl's one in three (a roll from `sim_military::mishap ^ n`).
  - **A mood's `about` is plain data:** the other's SpawnId and `NameId`, the place and the tick, never an `Entity` (grief is about the dead, whom `casualties` despawns). Vengeance resolves its killer's SpawnId through the fighter grid.
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
- **Docs.** DATA.md; ARCHITECTURE: what `casualties`, `end_battles` and slot 17 gained; GLOSSARY: Kin, Shield-friend, Rival, Feud, Mood note; TESTING; ADR 0021: kin are never stored, bonds and moods are sparse and cached.
- **Gate note.** Grief, `kin_killed` and the rival morale move desertion: re-run `bot_run`, with feuds and desertions by cause per seed in the step notes.
- **Done when.** Bot runs show grief, a few vengeances, friendships and feuds that start and end; the inspector explains every mood from its causes; `bot_run` still wins 3 of 5; NFR-09 holds with `--saga-bench` and on the run island.

### Step 13: Biographies, end screen and memorials (M, 6%)
- **What.**
  - **Words:** `saga::biography` and `saga::settlement_history`, words-only functions taking a tone. They draw on every record: quirks, deeds, epithets, ironies, maims, weapons, kin and feuds, and rescues and relics for settlements. A fate clause names a severed wound in brutal (`{wound_terse}`), so the end screen agrees with the feed and the field.
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
  - Unit tests of the biography words: a dead legend, a living soldier with a quirk, a maim and a feud, a spawned soldier and a saved settlement with a relic, in both tones. Nobody named is ever told as "a soldier".
  - Screenshots of each surface in the step notes.
- **Docs.** ARCHITECTURE's game-side table; GLOSSARY: Biography, Memorial, Oddities; TESTING.
- **Done when.** Every surface in the design's section 15 table shows its saga, in both tones.

### Step 14: The run record for M6 (S, 2%)
- **What.**
  - **`saga::run_record(world) -> RunRecord`:** pure, serde RON, versioned (`RUN_RECORD_FORMAT` 1). It holds facts and proper nouns, never sentences: legends (with quirks, any maim and their weapon), foes, the fallen, battles, named weapons and their fates, rescues, and the run's oddities.
  - **The game** writes `legends/<seed>-<difficulty>-<ended tick>.ron` under its save directory when a run ends, abandoned runs included. This happens before `leave_run` removes the finished run's slot. The same ending, after a quickload, overwrites its file.
  - **Headless:** `sim_headless --legends-out <path>`.
- **Where.** `crates/sim_military/src/saga.rs`, `crates/game_app/src/saves.rs`, `crates/sim_headless/src/main.rs`.
- **Tests.** `run_record_lists_legends`.
- **Docs.**
  - ARCHITECTURE: the saves section, on what survives a run.
  - GLOSSARY: Run record.
  - ADR 0021: no code reads the record in M4c, and nothing carries between runs; M4b adds memorials to it.
- **Done when.** A finished run in the game leaves one record file, and headless writes the same record for the same run.

### Step 15: Docs and playtest (S, 3%)
- **What.**
  - Complete ADR 0021, and bring ADR 0019's "as built" up to date.
  - NFR-10's final measurements on the dev box's lavapipe and, from you, on your RTX 4070, with every M4c visual in place (`--stress 20000` Full against step 3b's baseline, 1,000 a side in the detail band, `--effects-bench` per backend). NFR-10 is ticked only with your numbers in.
  - CLAUDE.md:
    - add a "Saga (ADR 0021)" rules block: the sim stores ids, never words; `saga.ron` is words only, exempt and unsaved; a told death goes through a `Fall` in `Saga`, never through `EventFeed`; battles get a number at their first blow; effects act through their holder, through `Veteran`'s cached nudges, and are run-only; quirks are dealt per SpawnId from their own stream, never from `populate`; mishaps are never notable and never touch the champion, a captain or a legend; a named weapon is always in exactly one place; kin are never stored and moods name people by SpawnId and `NameId`, never `Entity`; `Saga` hashes its append-only parts by digest and its in-place parts in full; a lethal mishap is told by its `Fall` alone; a rescue needs a fight; `saga::settle` is progress slot 17, and anything it decides for the next tick goes through hashed state (`Follower.deserting`), never the exempt scratch; NFR-09 is measured on live ticks only;
    - check the "Rendering" rules for the effects layer (added with this design, ADR 0019) against what was built;
    - update the milestone line.
  - README's status line.
  - PLAY_GUIDE: legends, quirks, mishaps, named weapons, kin and moods, maims, the tone, Effects and Kill cam switches, and a cheat-sheet row.
  - SEED.md: tick the criteria that pass, with dated measurements, including NFR-09's and NFR-10's.
  - Finish `docs/M4C_PLAYTEST.md` (part 2 after part 1).
- **Done when.** Every M4c criterion except the playtest is ticked, and the playtest checklist is ready.

Steps 1–3 and 5–12 are sim work, testable headless; step 3's words are checked in headless output, and step 8b's maims are sim work with a render part. Steps 3b, 4, 4b, 10b and 13 need the game. Step 14 needs both.

## Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Brutal words read as gratuitous, especially about villagers | High | Militia and rebel victims always take grave keys and never an irony; content rules in DATA.md; a deny-list lint; words reach the game at step 4 for an early look; a playtest question |
| The milestone is now about twice the first plan (1.25 times the previous one) | High | The split after step 8b: part 1 stands on its own (kill lines with their blood and bodies, quirks, mishaps, moments, epithets, absurd deaths, maims) and part 2 can slip; the largest step of part 2 (kin and moods) is the last sim step, so it can be trimmed without touching the rest; the effects layer (3b) needs no new dependency, and the GPU backend (4b) comes after the first playable slice, so the CPU backend carries the effects alone if Hanabi gives trouble; your RTX 4070's frame checks never block a step; the kill cam (10b) is small |
| Deadpan tips into silly, or the jokes wear thin | Medium | The voice rules and the do and don't table; the lint bans `!`, `?`, capitals and winking words; mishaps about once a day, not more; a playtest question |
| Mishaps feel unfair, wipe a band or take control away | Medium | Low-tension beats; a third fall on your enemies (the side is drawn first); good ones as well as bad; a wedding's halt ends at your next order or an approaching foe; the never-run-ending rules in code, each tested on all three personalities and over every `bot_run` seed |
| Mishaps change pacing | Low | Never notable: a unit test on the quiet clock, the first event and fast-forward, and a stubbed-effects run whose quiet clock and raids match a run without mishaps. Their effects can change a run's path, like any change to a run |
| Quirks shift balance or a village's food | Medium | Tiny effects through the holder, run-only; `bot_run` at step 5; `village_day` and `famine` re-run after the villagers.ron change |
| Something told didn't happen (geese, weddings, fords, rescues) | Medium | Flavour only on a real effect; ironies only from the design's table, with the "not in M4c" ends having no key; the geese named as the one flavoured trigger; a rescue only after a real fight near the village |
| Kill and saga lines flood the feed | Medium | Tier A narrowed; the rising bar, 10 tier-B lines a battle and 8 told deaths a tick in the sim; the speed filter, rate limit and fold in the feed; saga notes dropped first, with a hard cap of 2 a game day at 8x on every difficulty; one line per death (a lethal mishap is told by its `Fall` alone); tested at 1,000 a side, in a `game_ui` test and over every `bot_run` seed |
| Moods snowball into desertion waves | Medium | Four entries, clamped sums, desertion ×2 at most and only from the rare `kin_killed`; `bot_run` reports desertions by cause |
| Named weapons duplicate or vanish | Medium | One place per weapon, out first in arming and last in a barter, hourly reconciliation, a test for every way one moves |
| Legends too rare to notice, or too common to matter | Medium | The numbers are data, tuned against `bot_run`'s counts; epithets carry the mid-game; the cap of three |
| Earned traits too common (Cruel above all) | Medium | Marks only from eligibility, one of each kind per battle; target rates checked in step 9 |
| Balance or pacing drift | Medium | Effects are run-only and exact no-ops; new kinds aren't notable except `LegendFell`; `bot_run` after steps 1, 5, 6, 7, 8, 8b, 9, 10, 11 and 12 |
| Cost in big battles and on the run island | Medium | A push per kill is the only new work per blow; per-death work is bounded; nudges cached on `Veteran`; hourly work for the sparse parts; `--timings` on `--battle 7500` in every sim step against a recorded baseline, `--saga-bench` from step 9, the run island's live ticks from step 5, and the saga's own spans in a bot run's live windows; never a mean over a run that has already ended |
| Names of the dead lost | Medium | Kept deaths hold `NameId`s; anyone named has their death kept; the biography test checks it |
| Lines repeat | Medium | Variants stepped by told-death index; composition; the bank-size lint; the golden test |
| Saves break mid-milestone | Low | `SAVE_FORMAT` one above M4a's final value (3 as planned) at step 2, bumped again on any later layout change; M4a saves are refused by design |
| A GPU-only dependency breaks CI, a weak machine or a software renderer | High | bevy_hanabi only in game_render, in one module (`effects/gpu.rs`), and only from step 4b; only game_app adds `EffectsPlugin`, which adds `HanabiPlugin` only when the render device has compute shaders (otherwise CPU or Off); compute is read from the device's limits, not the adapter's, so WebGL2 limits on a Vulkan adapter never load Hanabi; headless builds don't compile it; smoke runs with each backend forced, with WebGL2 limits and on GL at steps 3b and 4b |
| Gating Hanabi in our plugin's `finish` breaks on a Bevy change | Medium | Bevy is pinned exactly; the gate is checked against bevy_app 0.19.1's source (the finish loop's range is fixed when it starts; new plugins are refused only after it has finished); the smoke runs exercise it; `RenderCreation::Manual` is the fallback with no new dependency, and a direct wgpu probe needs your OK (the design's open question 20) |
| Effects eat the frame budget in a big battle | Medium | Detail band only (bodies to low-poly); per-frame caps ranked told kill > kill > maim > hit, nearest first, sized to each Hanabi asset's capacity (a render.ron test); capped droplets, bodies, pools and parts; about 40 pooled GPU instances, never one per hit; NFR-10 measured with `--effects-bench` here at steps 3b and 4b, and on your RTX 4070 against step 3b's baseline before it's ticked at step 15 |
| The effects misread the sim | Medium | Hits from the sim's blow list, never from health seen by the renderer (a deserting recruit's health drops by 40); bodies only from `Casualties.fallen`; the collector copies only when `SimTick` advanced, so a finished run never replays its last deaths; tested in `effect_requests_follow_the_records` |
| Gore contradicts the words | Medium | One function (`Teller::told_wound`) for the feed and the renderer, from the record alone; every brutal telling of a severed death names the part (a `terse` twin with `{wound_terse}`, long forms, report lines, fate clauses), and names are frozen at the death; no part noun in fixed text (lint); `wound_and_line_agree` over every form of every told death of a seeded battle; maimed victims never wounded in the lost part |
| Over-the-top gore reads as tasteless, or numbs | Medium | Mild shows dust and falls; militia, rebels and villagers never lose a part; parts only on told deaths (a handful a battle); a playtest question ("grim, not numbing" for pictures too) |
| The kill cam annoys or disorients | Medium | Only captains, the champion and legends, on screen and near; never at 8x or in fast-forward; once per 10 s; any input of yours ends it; a toggle; a playtest question |
| Maims snowball into a weak band, or slow an army | Low | Once in a life, one chance in four past half health; small penalties through the holder only, run-only; a lost leg slows a captain only in a rout, never the army's march; `bot_run` at step 8b counts them |
| Maims are missed on ticks with no death | Medium | The roll is its own step after `casualties`, run whenever a check was noted; `maims_follow_the_rule` scripts a crossing on a tick when nobody dies |
| Scope creep (nemesis AI, memorial effects, feed volume, a blow-by-blow log, animals, rumours) | Medium | Each is an open question or a "later" note in the design, defaulted to "not in M4c"; memorials with effects are M4b's, animals M4b's, rumours M5's |

## Defaults chosen (change any)

1. **The step order** puts the effects layer (CPU and Off) at 3b, kill lines with their blood in the game at step 4, the GPU backend at 4b, quirks at 5 and mishaps at 6, before the steps that change play most (traits, legends, kin). You can stop after step 4 for a look, or after step 8b for part 1's playtest.
2. **The split** after step 8b into part 1 ("Deaths and oddities", about 64%) and part 2 ("Legends and kin", about 36%).
3. **Records and kill lines exist in every preset; effects only in runs.** The skirmish then shows kill lines, which helps tune the words. Quirks, mishaps, weapons, bonds and moods are run-only.
4. **Notability:** tier A is captains, the champion and legends (and a legend's milestone kills, and the rare mishap death); the bar is 40 + 15 per told line, with at most 10 tier-B lines a battle and 8 told deaths a tick. The weights are in the design's section 2.
5. **Legends:** fame 25, three battles and an epithet; three enlisted per faction; an aura of 8 tiles with −0.03 on the break point and +0.03 on the morale target; a death shock of 0.10, and 0.03 to foes.
6. **One new system slot:** progress slot 17, `saga::settle`, hourly, after M4a's slots 13–16 (which follow `director::step`), from step 5. Everything else lives in existing systems.
7. **`Service`, `Mood`, `Bonds` and `NamedWeapon` are sparse-set components,** so starting one never moves a soldier's table row; what the hot loops read is cached on `Veteran` (introduced in step 5).
8. **The golden words test uses its own fixture bank,** so editing the real bank never churns it.
9. **Step notes** go in `docs/m4c/`.
10. **Grudges and the Avenger deed** (open question 1) are built in step 10 only if you say yes.
11. **Battles get a number** at their first blow, so both sides' reports share a name, first blood and bar.
12. **Told deaths and saga notes live in `Saga`,** read by the feed with its own cursor, never through `EventFeed`; mishaps are events, not notable.
13. **Quirk and mishap draws** use new stream keys only (`sim_agents::quirks ^ SpawnId`, `sim_military::director::mishap`, `sim_military::mishap ^ n`, `sim_military::saga::homesick ^ SpawnId`); no existing stream moves.
14. **From the second review** (the design's decisions 33–41): a rescue needs a fight near the village; one line per death; a mishap counts only for its drawn subjects; a wedding yields to your orders and to an approaching foe; the side of a mishap is drawn first; the 8x cap on saga lines is a hard throttle; feed lines are fitted at read time with a `terse` fallback; quirk data is split between `villagers.ron` and `progression.ron`; the walk home goes through `Follower.deserting`; `Saga` is hashed in two parts; NFR-09 measures live ticks.
15. **The combat spectacle** (the design's decisions 42–58, section 20 and 21):
    - **Order.** The effects layer with its CPU and Off backends at step 3b, so the kill lines arrive with blood and bodies at step 4; severed parts in step 4, once told deaths and their words exist; the GPU backend (bevy_hanabi) at 4b, after the first playable slice; maims at 8b, after the epithets they extend; the kill cam at 10b, after legends.
    - **New steps take letters,** so no existing step number or reference changes.
    - **Hanabi is gated** in `EffectsPlugin::finish` (`add_plugins`, then its `finish` by hand), with no direct wgpu dependency and no cargo feature: the GPU backend is one module.
    - **One function, `Teller::told_wound`,** names the part for the feed and the renderer alike, from the record; every brutal telling of a severed death names it.
    - **A `Fall` keeps where it fell, the victim's role and the killer's epithet as it stood,** from step 2.
    - **A blow list joins step 1's baselines,** and is what the effects' hits read.
    - **Sim additions for the effects are exempt scratch only:** `Casualties.struck`, `fallen`'s victim, `maim_checks` and `maims`.
    - **The maim roll is its own step after `casualties`,** and a maim comes once in a life.
    - **Your RTX 4070's frame checks are handed over,** not part of any step's gate; NFR-10 waits for them.
    - **The saga's ADR becomes 0021:** 0019 is the effects layer's, written first, and 0020 is M4a Lordship's, built first.
