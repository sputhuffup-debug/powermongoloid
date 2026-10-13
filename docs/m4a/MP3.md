# M4a merge point 3 (MP3): the gate

Branch `claude/brave-turing-sh1ryn` at `8f46d08` ("MP3: merge lane A's step 6 ..."): MP2 (`6e5dbf5`: steps 1 to
5 and the MP2 tuning decisions), lane B's census and step 9 specialties (merged at `4df64d8`), and lane A's
step 6, fallout, with the seat calibration (`ce4d66e`). Gate: [M4A_PLAN.md](../M4A_PLAN.md) "The gate" and the
MP3 row of its execution plan; baselines: [baseline.md](baseline.md) and `baseline/*.out`; the previous gates:
[MP1.md](MP1.md) and [MP2.md](MP2.md). Run on 2026-10-13, 4 cores, `-j 2`, `--test-threads=2`,
`CARGO_INCREMENTAL=0`. **Nothing in the code or data was changed by this gate** (see "What was not changed"
at the end); the only file added is this page. **The two failures were then fixed in code, after the gate,
by the MP3 fixer: section 0.** No data changed, and no hash moved.

**After the fixes: GREEN.** The cost is back to the M4 binary's (live ticks 1 to 4,800, 48 interleaved pairs:
**-0.1%**, 95% interval -3.6% to +3.4%), and the bot's `retaken` now counts at most one retake per reprisal,
with a new column for the towns the rival conquered. Every state hash is unchanged (three replays in both
threading modes, the guarded stdout, `bot_run` day for day), so every other result on this page stands.

**The gate's verdict: RED, on two items; everything else is green** (both fixed in section 0).
1. **Cost (NFR-11).** On the live ticks the MP3 tree costs about **+5% to +6%** more than the M4 binary
   (paired mean +5.9% over 42 pairs, 95% interval +3.2% to +8.5%; median of the 30-pair run +6.2%), against a
   limit of +5%. MP2's window-1 flag persists (+7.8%) and window 2 joins it (+5.8%). The cause is the Decide
   stage (step 2's smithy chain, in since MP1) plus about three points from step 9's specialties; step 6 adds
   nothing a botless run can see. The comparison had to stop at tick 4,800: **the botless seed-7 run now ends
   on tick 7,035** ("lost, killed in battle") on this island, so the whole-run total is no longer a live
   figure (section 5).
2. **Lane A's open note (section 6.4).** No town changes hands without an event, but the bot's `retaken`
   count is wrong (seed 3 says 2 for at most 1; seeds 11 and 42 report more retakes than reprisals), and the
   `Takes` line has no column for the rival's own conquests, which are most of the losses. A reporting bug in
   `crates/sim_headless/src/bot.rs`; no sim state or replay is involved.

**`bot_run` passes**: 4 of 5 won, the earliest on day 17.5 (floor 14), the median 19.9 (limit 14.4 to 22.4;
19.6 over the four wins), the quiet limit held. Things the merge lead should know, none of them a limit:
**seed 3 is lost** again (starvation on day 33.4; as at MP1; step 6 alone had won it on day 21.7), **the
stage-check seed 42 is lost** too (it was a day-16.9 win at MP2), **no bot run on this tree raises a bandit
band** (seeds 1 to 13: the bot never sacks, so fallout's bandit path is covered by `fallout.rs` and the unit
tests only), and the playtest below (section 9) is the first time a human sees a band.

## Summary

| # | Check | Result |
| --- | --- | --- |
| 1 | `cargo fmt --all --check` | clean |
| 1 | `cargo clippy -j 2 --workspace --all-targets -- -D warnings` | clean (0 warnings), twice, the second after the workspace check artifacts were deleted |
| 1 | `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps` | clean |
| 2 | Full suite (`cargo test --workspace`, no fail-fast, `--test-threads=2`) | **449 passed, 0 failed, 4 ignored** in five batches (39.5 min); all 31 integration test files, the new `fallout`, `specialty` and `census` included |
| 3 | Determinism tests (`determinism.rs`, 13) | 13 of 13 pass, in the suite |
| 3 | Run replay, seed 42, 3,000 ticks, 50 commands (51 recorded) | `--replay` and `--replay --single-threaded` both "replay ok ... checkpoints=3", `state_hash=0x229a7b1dece4a34e` |
| 3 | Bot replay with a vow and a reprisal, seed 4, to its win at tick 46,277 (154 commands) | both replays "replay ok ... checkpoints=50", `state_hash=0xb340346612c91ec5` |
| 3 | Bot replay with the whole fallout chain, seed 3, to its loss at tick 80,110 (468 commands) | both replays "replay ok ... checkpoints=82", `state_hash=0x22a3784a1c5c14db` |
| 3 | A replay with a bandit band | **none exists**: no bot run on this tree raises one (seeds 1 to 13) |
| 4 | Guarded stdout, developed, skirmish, battle (release) | all three byte-identical to `baseline/*.out` (`cmp` silent; sha256 equal) |
| 5 | Cost, seed 7 steady, live windows 1 and 2, 42 interleaved pairs against the M4 binary | **FAIL (at the limit): paired mean +5.9% (+3.2% to +8.5%), paired median +5.7%**; window 1 +7.8% (persists), window 2 +5.8%; the run ends on tick 7,035, so window 3 is partly dead; section 5. **Fixed (section 0): -0.1% over 48 pairs (-3.6% to +3.4%)** |
| 5 | Stage check, bot run seed 42 (lost at tick 87,913): Lordship's own stages incl. `fallout::step` | **2.0 us a tick, 0.087% of the 2.307 ms mean tick** (limit 5%); worst 250-tick window 0.64% |
| 6 | `bot_run` (release, `--ignored`), wins | **4 of 5** (limit: at least 3): pass; quiet limit 999 of 1000 on every seed: pass |
| 6 | `bot_run`, win-day floor (no seed before day 14) | **pass**: the earliest win is day 17.5 (seed 2) |
| 6 | `bot_run`, median within 14.4 to 22.4 | **pass**: 19.9 with the lost seed counted at its end day, 19.6 over the four wins |
| 6 | Lane A's open note: how each lost town was lost | every ownership change of a town of the bot's has an event (80 of 80 eventful flips; the other 24 are the rival's lands freed at the seat's fall, by design); **FAIL: the bot's `retaken` over-counts**. **Fixed (section 0): at most one retake per reprisal; the rival's conquests have their column** |
| 7 | `influence` ten-day envoy check (release, `--ignored`) | passes: seed 1 invited on day 7, seed 42 on day 8 |

## 0. The fixes (MP3 fixer, after the gate)

Both failures were diagnosed at the root and fixed in code. **No data changed and no state hash moved**, so
`bot_run`, the scenario tests and every figure of sections 2 to 9 play exactly as gated. Recorded in
[ADR 0020](../adr/0020-lordship.md) "As built: Merge point 3".

### 0.1 Cost (NFR-11): where it went, and the fix

**Diagnosis by instruction counts** (callgrind, which is deterministic where the wall clock swings 10 to 15%
a run), the botless seed-7 run, ticks 2 to 4,800, every thread counted:

| Binary | Instructions per tick | Against M4 |
| --- | --- | --- |
| M4 (`sim_headless_m4`) | 7.90 million | |
| the gated tree (`sim_headless_mp3`) | 8.34 million | **+5.5%** |
| the fixed tree (`sim_headless_fix2`) | 7.68 million | **-2.9%** |

Per function (main thread, `--single-threaded`, 4,800 ticks), the gated tree's excess was:
- **`ai::decide`, +1.25 billion (+12%)**: step 2's two serial passes over all 4,500 villagers (the claims
  read before anyone decides, and the second pass that looked for carters who took the same delivery), and
  about 300 more instructions in each of the 2.3 million decisions. Inside a decision, the biggest cost was
  pre-existing: every decision drew its Idle option's wander spot (`idle_spot`, a fresh ChaCha8 stream: 1.2
  billion, a fifth of all scoring), although Idle seldom wins.
- **`resolve_transfers`, +0.57 billion (+86%)**: the same orders, but gathering them from every agent cost
  twice M4's per agent (the closure is no longer inlined once sim_economy grew; `collect` steps the query
  with `next`).

**The fix** (`crates/sim_agents/src/ai.rs`, `crates/sim_economy/src/goods.rs`; each change is exact, which the
unchanged hashes confirm):
1. The Idle candidate carries its anchor (home or the green), and `AiWorld::goal_of` draws the spot only for
   the option chosen. The draw is keyed by villager and tick, so it is the same spot whenever it is drawn.
2. The trace's best three come from `ai::top_three` (no sort, no allocation), the first three of the stable
   sort the trace used; `top_three_matches_a_stable_sort` pins it.
3. Step 2's second pass is gone: carters who take a free input delivery are noted as they choose, inside the
   parallel pass, then sorted by SpawnId; only the losers are fetched and sent back.
4. The claims pass and `resolve_transfers` fold over their queries (`for_each`), which takes Bevy's
   dense-table path.

**Wall clock**, live ticks only (`--run --seed 7 --difficulty steady --ticks 4800 --timings --stats-every
2400`: the run ends on tick 7,035 on this island, so ticks 1 to 4,800 are windows 1 and 2 of the plan's run
and the `total` line is their live total), nothing else running:

| Comparison | Pairs | Total (live), median of medians | Paired mean (95% interval) | Paired median | Decide |
| --- | --- | --- | --- | --- | --- |
| M4 to the gated tree (three-way run, this session) | 24 | +4.5% | +3.3% (-1.0 to +7.6) | +3.0% | +10.7% |
| M4 to the fixed tree (three-way run) | 24 | +1.2% | -0.1% (-5.1 to +5.0) | +3.2% | -3.5% |
| the gated tree to the fixed tree (three-way run) | 24 | -3.1% | -2.9% (-7.5 to +1.8) | -3.4% | **-12.8% (t = -5.2)** |
| M4 to the fixed tree (second session, two-way) | 24 | -0.8% | -0.1% (-5.3 to +5.0) | -0.7% | -4.7% |
| **M4 to the fixed tree, both sessions** | **48** | **+0.4%** | **-0.1% (-3.6 to +3.4)** | **+1.2%** | **-4.3%** |

By window over the 48 pairs: window 1 -0.3% (median of medians; paired mean +1.3%), window 2 -3.7% (-0.8%).
Act is -3.1% against M4 (resolve_transfers); Move reads +3% to +5% against both M4 and the gated tree, with no
code or work changed there (the same paths are searched: the hashes are equal), so it is the stage clock's
split, not cost. The plan's command as specified, three pairs at 7,200 ticks: total -3.6% (window 3 holds 165
dead ticks in the fixed run, as in the gated one), Decide -6.6%. The `run ended: lost at tick 7035 (killed in
battle)` line is unchanged (the island's, not the fix's). **NFR-11 passes: the live cost is M4's, with the
interval's top under +5%.** Raw outputs: `scratchpad/m4a/fixa_cost_*` (three-way), `fixb_cost_*`,
`fixs_cost_*`; callgrind in `scratchpad/cg/`.

### 0.2 The bot's `retaken`, and the column the line lacked

`crates/sim_headless/src/bot.rs`, `Bot::takes` (reporting only):
- **A retake is now the first change of hands of the town a `RivalReprisal` named, after the band was sent,
  counted once per reprisal**, and if it is a rival `Conquest`; the line says how many the reprisal band
  itself made: `retaken N (by its band M)`. It can no longer exceed `reprisals`.
- **New column: `lost to the rival: raids R reprisals P other O`**, the player's towns the rival conquered
  (a rival `Conquest` whose previous owner was the player), by whose captain took them: a `RivalRaid` band's,
  a `RivalReprisal` band's, or another (an heir, who has no event of his own).
- Unit test `retakes_follow_the_reprisal` (seed 3's Thornbrook as the fixture: a raid band's conquest before
  the vow, the reprisal, the raid band's conquest after it, the reprisal band's Duncombe; then a revolt that
  comes first, and a band's own retake).

`bot_run` on the fixed tree (release, 505 s): **the same five runs** (won on days 19.9, 17.5, 19.3 and 20.3,
seed 3 lost on day 33.4; the same command counts), now reporting:

| Seed | Vows | Reprisals | Retaken (by its band) | Lost to the rival: raids / reprisals / other | Defied | Revolts | Lost in all (run summary) |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 0 | 0 | 0 (0) | 3 / 0 / 0 | 3 | 0 | 6 |
| 2 | 2 | 0 | 0 (0) | 0 / 0 / 0 | 1 | 0 | 1 |
| 3 | 1 | 1 | **1 (0)** (was 2) | 10 / 1 / 0 | 2 | 2 | 15 |
| 4 | 1 | 1 | 0 (0) | 0 / 0 / 0 | 0 | 0 | 0 |
| 5 | 0 | 0 | 0 (0) | 2 / 0 / 0 | 0 | 0 | 2 |

The line now adds up: defiances, revolts and the rival's conquests make every lost town of the run summary
(seed 1: 3 + 3 = 6; seed 3: 2 + 2 + 11 = 15), and the conquests agree with section 6.4's probe (3, 11 and 2,
one of seed 3's by the reprisal band). Seed 3's retake is the raid band's conquest of Thornbrook 107 ticks
after the reprisal left; the reprisal band took Duncombe instead. Outside the gate set: seed 11 now reads
`reprisals 2 retaken 2 (by its band 1)` (was 3) and seed 42 `reprisals 2 retaken 1 (by its band 0)` (was 3).

### 0.3 Checks re-run on the fixed tree

| Check | Result |
| --- | --- |
| `cargo fmt --all --check`; `cargo clippy -j 2 --workspace --all-targets -- -D warnings` | clean |
| `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps` | clean |
| Full suite (five batches, as section 2; `scratchpad/m4a/fix_suite.sh`, log `fix_full.log`) | **451 passed, 0 failed, 4 ignored** in 27.8 minutes (the gate's 449 plus the two new unit tests); every `tests/tests/*.rs` file ran; determinism 13 of 13 |
| Run replay, seed 42 (`mp3.ron`); bot replays, seed 4 to its win (`mp3_bot4.ron`) and seed 3 to its loss (`mp3_bot3.ron`) | all "replay ok", threaded and `--single-threaded`, **the gate's hashes**: `0x229a7b1dece4a34e`, `0xb340346612c91ec5`, `0x22a3784a1c5c14db` |
| A 4,800-tick botless seed-7 run | `0x701744d957fa2d24`, the gated binary's |
| Guarded stdout: developed, skirmish, battle | `cmp` silent against `baseline/*.out` |
| `bot_run` (release, `--ignored`) | passes, 4 of 5, the same days (0.2) |
| Docs: ARCHITECTURE.md (the Decide row), TESTING.md (the two unit tests, the `bot_run` line), ADR 0020 | updated |

Not re-run (nothing they measure changed, and every hash is equal): the bot-run stage check of 5.4 (the
Lordship stages are untouched) and the ten-day envoy check.

## 1. Build and lint

- `cargo fmt --all --check`: clean (exit 0).
- `cargo clippy -j 2 --workspace --all-targets -- -D warnings`: exit 0, no `warning` or `error` line. Run twice:
  first on the tree as handed over (the log says `Finished` in 0.7 s: the merge lead's identical run was still
  in the build cache, and cargo replays a cached unit's diagnostics, so a warning would have shown), then again
  after `prune_target.sh 0` had thrown away every workspace check artifact (33 s, every workspace crate
  re-checked, 0 warnings).
- `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps`: exit 0, no warning line (2 m 39 s,
  fresh).
- `git status` was clean before and after each of these and at the end.

## 2. The full suite

Run as batches, for disk, with the same script shape as MP2 (`scratchpad/m4a/mp3_suite.sh`): every command
`-j 2`, `--no-fail-fast` (before the `--`), `--test-threads=2`, `CARGO_INCREMENTAL=0`, the finished
integration binaries deleted between batches (free space was 9.8 GB at the start and 3.8 GB after the last
batch):

0. every crate except `integration_tests` (unit tests, binaries, doc tests);
1. `army_march bot_run champion chronicle conquest determinism director fallout`;
2. `docs dominion edicts famine gather influence invitation census`;
3. `monuments orders posture rival run_start save_load sellswords specialty`;
4. `smithy_chain starving_army take_modes tenure village_day wanderers worldgen`.

The script checks at its end that every `tests/tests/*.rs` file was in a batch (it printed nothing: all 31
ran, the new `fallout`, `specialty` and `census` among them). All five batches exited 0. **449 passed, 0
failed, 4 ignored** in 39.5 minutes including the compile (MP2: 422 passed, 3 ignored; the 27 extra are 15
integration tests, `fallout` 7, `rival` 4 and `specialty` 4, plus 7 in `game_ui`, 3 in `sim_military` and 2
in `sim_agents`). The four ignored are by design: `bot_run` and the ten-day `influence` test (both run
below), lane B's `census` (it measures and asserts nothing; CENSUS.md) and one `sim_military` unit test. The log is
`scratchpad/m4a/mp3_full.log`.

| Binary | Passed | Failed | Ignored | Seconds |
| --- | --- | --- | --- | --- |
| unit: game_app | 4 | 0 | 0 | 5.1 |
| unit: game_render | 31 | 0 | 0 | 0.1 |
| unit: game_ui | 66 | 0 | 0 | 0.0 |
| unit: sim_agents | 32 | 0 | 0 | 0.0 |
| unit: sim_core | 39 | 0 | 0 | 0.0 |
| unit: sim_economy | 13 | 0 | 0 | 0.0 |
| unit: sim_headless | 8 | 0 | 0 | 0.4 |
| unit: sim_military | 100 | 0 | 1 | 62.6 |
| unit: sim_world | 32 | 0 | 0 | 1.1 |
| unit: sim_ai, sim_headless, sim_knowledge, sim_lod (no tests) | 0 | 0 | 0 | |
| doc tests, 11 crates | 0 | 0 | 0 | |
| army_march | 1 | 0 | 0 | 16.9 |
| bot_run | 0 | 0 | 1 | 0.0 |
| champion | 3 | 0 | 0 | 6.5 |
| chronicle | 1 | 0 | 0 | 21.1 |
| conquest | 4 | 0 | 0 | 76.6 |
| determinism | 13 | 0 | 0 | 267.4 |
| director | 3 | 0 | 0 | 38.5 |
| fallout | 7 | 0 | 0 | 18.9 |
| census | 0 | 0 | 1 | 0.0 |
| docs | 4 | 0 | 0 | 0.0 |
| dominion | 2 | 0 | 0 | 14.5 |
| edicts | 10 | 0 | 0 | 120.7 |
| famine | 1 | 0 | 0 | 29.7 |
| gather | 2 | 0 | 0 | 25.2 |
| influence | 6 | 0 | 1 | 26.5 |
| invitation | 2 | 0 | 0 | 8.4 |
| monuments | 2 | 0 | 0 | 5.7 |
| orders | 7 | 0 | 0 | 14.2 |
| posture | 1 | 0 | 0 | 22.2 |
| rival | 6 | 0 | 0 | 59.4 |
| run_start | 4 | 0 | 0 | 18.0 |
| save_load | 5 | 0 | 0 | 36.6 |
| sellswords | 6 | 0 | 0 | 21.6 |
| specialty | 4 | 0 | 0 | 11.1 |
| smithy_chain | 2 | 0 | 0 | 24.7 |
| starving_army | 1 | 0 | 0 | 27.3 |
| take_modes | 9 | 0 | 0 | 111.0 |
| tenure | 8 | 0 | 0 | 23.3 |
| village_day | 1 | 0 | 0 | 16.8 |
| wanderers | 2 | 0 | 0 | 54.2 |
| worldgen | 7 | 0 | 0 | 18.6 |
| **total, 55 binaries** | **449** | **0** | **4** | |

(The four "no tests" crates are `sim_ai`, `sim_knowledge`, `sim_lod` and the `sim_headless` binary. Doc tests: 11
crates, none.)

## 3. Determinism and replays

Release binary: `CARGO_INCREMENTAL=0 cargo build -j 2 --release -p sim_headless` (2 m 55 s), kept as
`scratchpad/m4a/sim_headless_mp3` (sha256
`fda03598a436385300844634146744a3f1d87ec034921919c98a757967736f94`). After the stage probe of section 5.4 had
been built and removed, the tree was rebuilt clean and gave the same sha256, so the binary is the tree's.

- `determinism.rs`: 13 of 13 in the suite (267 s), including `a_bot_run_replays`.
- Run replay: `--run --seed 42 --ticks 3000 --commands 50 --record mp3.ron` (51 commands, hash
  `0x229a7b1dece4a34e`), `--replay mp3.ron` and `--replay mp3.ron --single-threaded`: both "replay ok:
  seed=42 ticks=3000 commands=51 checkpoints=3", the same hash. (MP2's was `0xffa9bfa76a10936b`; steps 6 and 9
  change it, as they should: the island and the rival's clock differ.)
- **Bot replay with fallout, seed 4, to its win** (`--run --bot --seed 4 --ticks 50000 --record mp3_bot4.ron`;
  the bot wins at tick 46,277; 154 commands). This run has a **vow and a reprisal**: the rival vowed on day
  14.9 to retake Barrowridge (a submission in its shadow) and sent a band of 6 on day 15.7. Both replays
  "replay ok: seed=4 ticks=50000 commands=154 checkpoints=50", hash `0xb340346612c91ec5`, equal to the bot's
  own run (the recorder checks that and would have exited 2).
- **Bot replay with the whole fallout chain, seed 3, to its loss** (`--ticks 82000`; the run is lost on tick
  80,110 and the tick freezes; 468 commands): a vow, a reprisal, rival retakes, defiances, revolts and six
  Forsaken deeds. Both replays "replay ok ... checkpoints=82", hash `0x22a3784a1c5c14db`, equal to the
  `--chronicle` run's final hash.
- **No replay holds a bandit band.** The brief asked for a seed "that raises a bandit band or a reprisal";
  seeds 3 and 4 give the reprisal, but **no bot run on this tree raises a band** (seeds 1 to 13 were played
  to look, section 6.2). A band is covered by `fallout.rs` (7 tests: `broken_men_are_its_own_people`,
  `heirs_keep_the_band`, `the_cap_holds` ...) and `save_load.rs`'s `every_lordship_type_round_trips`, not by a
  replay. Recording one would need a scripted sack; it is not in the brief and nothing here does it.

## 4. Invariance: the three guarded stdout outputs

From the repository root, `cmp` silent for all three (release binary above):

| Output | sha256 (equal to the baseline's) |
| --- | --- |
| developed `--seed 42 --ticks 4801 --stats-every 600` | `cff552e813a9bd2494e3960af520250a81d50d028def36fb315b98c93927adeb` |
| skirmish `--skirmish --seed 42 --ticks 6000 --stats-every 2000` | `12762dd06ba25cc5feb4575f75da356c0d99e26f7076c3d0a5df4691c236f80c` |
| battle `--battle 1000 --seed 42 --ticks 3000 --stats-every 600` | `cb2c212ecbaf8203f7828c08c127414cd4a3fe41be9690ce3db3930d8429b7b8` |

## 5. Cost (NFR-11), live ticks only

### 5.1 The botless run no longer lasts 7,200 ticks

The plan's check is `--run --seed 7 --difficulty steady --ticks 7200 --timings --stats-every 2400`, the M4
binary (`sim_headless_m4`) against the one under test, "on live ticks only". baseline.md says to run the same
command without `--timings` and look for a `run ended` line first. **On this tree it prints one:**
`run ended: lost at tick 7035 (killed in battle)`. The M4 and MP2 binaries and the gated tree with specialties
off (5.3) print none (`grep -c` gave 0, 0 and 0). The idle band's champion is killed in the third window, so
that window holds 165 dead ticks and every 7,200-tick mean and total of the MP3 binary is deflated by up to
2.3% (the stages by the same). Per the plan ("compare up to the last full window before the end, and say
so") the comparison below uses **windows 1 and 2 (ticks 1 to 4,800), live in every run**, which run 4,505
and 4,496 agents (M4: 4,507 and 4,494). The cause is the specialties' island: the gated tree with
`specialties: []` does not end the run (5.3).

### 5.2 The comparison

Interleaved pairs, the order alternating, nothing else running (checked with `top`), in four sessions. The
box's speed drifts between sessions by up to 15% (the M4 binary's windows 1 and 2 average 1.87 to 2.32 ms), so
only the per-pair changes mean anything.

| Session | Pairs | M4 to MP3, live windows 1 and 2 (median of the pairs' means, ms) | Change, median of medians / paired mean |
| --- | --- | --- | --- |
| 1 (pairs 1 to 10) | 10 | 2.098 to 2.210 | +5.4% / +4.1% |
| 2 (pairs 11 to 20) | 10 | 2.315 to 2.388 | +3.1% / +4.5% |
| 3 (pairs 21 to 30) | 10 | 2.173 to 2.310 | +6.3% / +6.3% |
| 4 (12 rounds of the three-way run in 5.3) | 12 | 1.865 to 2.107 | +13.0% / +8.0% |
| **all 42 pairs** | 42 | | **paired mean +5.9%** (sd 8.8%, t = 4.3, 95% interval +3.2% to +8.5%), **paired median +5.7%**; 24 of 42 pairs above +5% |

The first three pairs of session 1, the check as specified: +2.2% (2.144 to 2.192 ms on the whole run). The
first 30 pairs by window (the whole-run total is not used: MP3's is deflated, 5.1):

| Window | M4 | MP3 | Change, median | Paired mean (sd, t) |
| --- | --- | --- | --- | --- |
| 1 (ticks 1 to 2400) | 2.045 | 2.205 | **+7.8%** | +5.2% (9.2%, t = 3.1) |
| 2 (2401 to 4800) | 2.240 | 2.370 | **+5.8%** | +5.0% (11.5%, t = 2.4) |
| 3 (4801 to 7200, 165 dead ticks in MP3's) | 2.210 | 2.275 | +2.9% (raw) | +2.9% (10.3%, t = 1.6) |
| live, windows 1 and 2 | 2.172 | 2.307 | **+6.2%** | +5.0% (8.8%, t = 3.1) |

Over all 42 pairs, window 1 is +5.4% (paired mean, t = 3.5; paired median +6.4%) and window 2 +6.6% (t = 3.6;
+5.7%). **MP2's window-1 flag (+6.8%) persists (+7.8% on the 30 pairs, +5.4% on all 42), and window 2 now
joins it.** On the live ticks the cost is **about +5% to +6%: at the limit, not clearly inside it.** MP2's
gate had +1.5% on the whole run with window 1 at +6.8% and window 2 at -1.5%, so the live figure has grown by
about three points since.

Stage means over the 30 pairs (median, ms a tick, whole 7,200 ticks, so MP3's are deflated 2.3%): Input
0.203 to 0.210, Knowledge 0.141 to 0.145, **Decide 0.581 to 0.646 (+11.2% median, +11.6% paired mean, t =
7.2; about +13.8% corrected for the dead ticks; +0.065 to +0.08 ms, 3 to 4 points of a tick)**, Move 0.501
to 0.508, Act 0.311 to 0.317, Needs 0.223 to 0.217, Bookkeeping 0.217 to 0.224. Decide is where MP1 and MP2
put step 2's smithy chain (+0.07 to +0.09 and +0.074 ms): **steps 6 and 9 do not show in it.** Raw outputs:
`scratchpad/m4a/mp3_cost_{m4,mp3}_{1..30}.out`, `mp3y_cost_*`.

### 5.3 Where the excess comes from: a three-way run

Twelve rounds, each running three binaries in a rotating order: M4, the gated tree, and the gated tree with
`assets/data/villagers.ron` `specialties: []` (that is step 6 on the MP2 layout; it was built from a
temporary edit, restored with `git checkout -- assets`; `sim_headless_mp3ns`). The botless run does not end
early on the third, and sees no fallout work (it owns nothing).

| Comparison, live windows 1 and 2 | Change, median | Paired mean (sd, t) |
| --- | --- | --- |
| M4 to MP3 (the gated tree) | +13.0% | +8.0% (8.7%, 3.2) |
| M4 to MP3 without specialties (step 6 on the MP2 layout) | +8.7% | +4.9% (7.0%, 2.4) |
| MP3 without specialties to MP3 (what step 9's specialties add) | +3.9% | +3.2% (7.9%, 1.4) |

Decide (7,200 ticks): M4 0.520, without specialties 0.564 (+8.4% median, +11.7% paired mean, t = 4.7), MP3
0.577 (+2.2% on top, t = 0.9). Reading it: **most of the excess is the Decide stage and is there without the
specialties**, as at MP1 and MP2 (step 2's smithy chain). **The specialties add about three points** (the
interval is wide: t = 1.4 at 12 rounds), which plausibly follows from the design (Mining towns weight miners
four times and smiths twice, so more hands work, and the layout, and with it the botless band's fate,
changes). **Step 6 adds nothing measurable to a botless run.** A direct MP2-against-MP3 run (20 pairs,
`mp3x_cost_*`) gave +11% on both live windows, the two sessions disagreeing by 5 to 11 points; it is
noise-dominated and not used, but it says the figure above could be higher.

### 5.4 The bot-run stage check (from step 4 on)

The plan asks for `--run --bot --seed 42 --ticks 96000 --timings --stats-every 250` with Lordship's own stages
(slots 13 to 16, `orders::lands`) summed and kept under 5% of the mean tick. `--timings` reports per `SimSet`
only and the repository has no per-system timer, so this was measured as MP2's was, with a **temporary probe,
since removed**: a wall-clock guard (`Instant`, an atomic nanosecond counter per stage) around
`influence::step`, `tenure::step`, `govern::step`, `fallout::step` and `orders::lands`, and a line of the
counters' per-window deltas after each `--stats-every` window in `--timings` mode (four edits in `crates/`,
built into `scratchpad/m4a/sim_headless_probe3`, restored with `git checkout -- crates` at once; the tree was
rebuilt clean and its sha256 equals the gated binary's).

**On this tree the bot loses seed 42** (the champion starves on tick 87,913, day 36.6; MP2 won it on tick
40,591). The 351 live windows (tick 250 to 87,750; 4,368 to 4,723 agents) give a **mean tick of 2.307 ms** and,
per tick:

| Stage | us a tick (mean of the windows) | worst 250-tick window |
| --- | --- | --- |
| slot 13 `influence::step` | 0.144 | 0.5 |
| slot 14 `tenure::step` | 0.658 | 4.0 |
| slot 15 `govern::step` (with `market_hire`, `jobs::reassign`) | 0.260 | 7.1 |
| slot 16 `fallout::step` | **0.013** | 0.2 |
| `orders::lands` | 0.932 | 14.4 |
| **sum** | **2.0 us = 0.087% of the mean tick** (median window 0.078%) | **worst window 0.64%** |

Limit 5%: **passes by a factor of about fifty-seven** (MP2: 0.09%). `fallout::step` is now non-empty and costs
0.013 us a tick: the bot never sacks, so no band rises and the vow queue is short. Not measured: `construction::
advance` (step 8, not built).

## 6. `bot_run`: balance

`cargo test --release -j 2 -p integration_tests --test bot_run -- --ignored --nocapture` (524 s): the test
passes (4 wins, needs 3; the director's quiet limit holds, 999 of 1000 ticks on every seed). **The gate's
win-day floor and median pass.** A second, throwaway probe test (`tests/tests/mp3_probe.rs`, since removed)
played the same five seeds in the same way and printed what `bot_run` does not: every ownership change of
every populated town with the chronicle events at its tick, the Market hire offers and what became of them,
and the bandit, vow and reprisal events. It agreed with `bot_run` on every win day, take and taken/lost/held
figure. `sim_headless --run --bot --seed N --ticks 96000 --chronicle` agrees too (same end ticks, command
counts and winning captains).

### 6.1 Win days

| Seed | M4 baseline | MP1 (fixed) | MP2 | Step 9 specialties alone (lane B's note) | Step 6 alone (STEP6.md, MP2 layout) | **MP3** | Before day 14? |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 17.4 | 16.9 | 8.2 | 7.9 | 22.6 | **19.9** | no |
| 2 | 19.3 | 19.1 | 17.2 | 6.2 | 24.9 | **17.5** | no |
| 3 | 26.3 | lost 32.0 | 7.1 | 7.5 | 21.7 | **lost 33.4** (starvation) | no |
| 4 | 18.4 | 25.1 | 11.1 | 8.1 | 20.4 | **19.3** | no |
| 5 | 15.5 | 18.1 | 7.3 | 14.2 | 20.5 | **20.3** | no |
| median | 18.4 | 19.1 | 8.2 | 7.9 | 21.7 | **19.9** | |

- **Wins:** 4 of 5 (limit: at least 3). Pass.
- **Floor** (no seed before day 14): the earliest win is day 17.5 (seed 2). **Pass.**
- **Median** (limit 14.4 to 22.4): **19.9** counting the lost seed at its end day (the MP1 convention), 19.6 over
  the four wins. Both inside. **Pass.** (STEP6's 21.7 sat 0.7 under the ceiling; this is 2.5 under.)
- **Quiet limit** (GP-16): 999 of 1000 ticks on every seed. Pass.
- **Seed 3 is lost again.** It is the island with the modifiers `Lean year` and `Stronger rival`. It was lost
  at MP1 (day 32.0), won far too early at MP2 (7.1), and won on day 21.7 with step 6's calibration on the MP2
  layout. With the specialties' layout the bot makes 18 takings and loses 15 (11 to the rival's raid bands, 2
  defiances, 2 revolts, section 6.4); its champion falls of hunger on day 32.9 (tick 78,850) and the successor
  on day 33.4 (tick 80,109): `Cause::Starvation`. A loss does not break any limit above (4 of 5 wins), as at
  MP1; it is recorded here because step 6 alone had the seed winning.

### 6.2 Per seed: the numbers the gate asks for

Counted in the same runs as the win days. "Takes by way" is `Takes` (the `bot_run` line); the rival's own
conquests of the bot's towns, the Market hire columns and the events come from the probe; "retakes" is the
bot's own count, with the true figure in brackets where they differ (section 6.4).

| Seed | Win day | Attack | Sack | Submitted | Invited | Subverted | Defiances | Bandit bands | Raids on rival / player / other | Vows | Reprisals | Retakes | Revolts vs you | Rival conquests of your towns | Taken / lost / held (people) | Market hires taken (offers) | Commands |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 19.9 | 1 | 0 | 6 | 0 | 0 | 3 | 0 | 0 / 0 / 0 | 0 | 0 | 0 | 0 | 3 | 7 / 6 / 1 (0) | 0 (0) | 183 |
| 2 | 17.5 | 1 | 0 | 5 | 0 | 0 | 1 | 0 | 0 / 0 / 0 | 2 | 0 | 0 | 0 | 0 | 6 / 1 / 5 (500) | 0 (0) | 184 |
| 3 | lost 33.4 | 0 | 0 | 10 | 8 | 0 | 2 | 0 | 0 / 0 / 0 | 1 | 1 | 2 (1) | 2 | 11 | 18 / 15 / 3 (541) | 2 (8) | 468 |
| 4 | 19.3 | 1 | 0 | 6 | 0 | 0 | 0 | 0 | 0 / 0 / 0 | 1 | 1 | 0 | 0 | 0 | 7 / 0 / 7 (959) | 0 (2) | 154 |
| 5 | 20.3 | 1 | 0 | 6 | 1 | 0 | 0 | 0 | 0 / 0 / 0 | 0 | 0 | 0 | 0 | 2 | 8 / 2 / 6 (773) | 2 (7) | 126 |

Other columns, same runs: demands issued 6 / 6 / 11 / 6 / 9, refused 0 / 0 / 0 / 0 / 1; gifts 0 on every seed;
Forsaken deeds 0 / 0 / 6 / 0 / 1 (`Forsaken` events in the chronicle); "Attack" is the seat on the four wins,
the only Attack the bot makes. Weapons tithed and men levied are not counted by the bot or the test (MP2's
second probe counted them; this one did not repeat it, the dials are step 4's and unchanged). Subverted is 0
by construction (step 7 is not built).

**What the numbers say.**
- **Every win is Demands (and a few invitations), then one Attack on the seat.** Six submissions on seeds 1, 4
  and 5, five on seed 2 (ten, and eight invitations, on the lost seed 3); the one Attack is the seat. There is
  **no sack on any seed**, so **no bandit band rose on any seed**: fallout's bandit path is not exercised by
  the bot on this tree at all (it was, twice, in STEP6's table: seeds 1 and 4 made a second Attack). The same
  holds on seeds 6 to 13, which were played to scan for a band (`--run --bot`, no band on any; they win seeds
  6, 7, 8, 10, 11, 12 and 13, and lose seed 9 to a champion death on day 13.7). The only coverage of the
  bandits is `tests/tests/fallout.rs` (7) and the unit tests. This is the thing the user's playtest (section
  9, check 2) is for.
- **Reprisals do launch from the bot's takes:** seed 3 vowed (day 15.1, Thornbrook, an invitation) and sent a
  band of 10 on day 15.8; seed 4 vowed (day 14.9, Barrowridge, a submission) and sent 6 on day 15.7 and won;
  seed 2 vowed twice (days 11.2 and 14.1) and no band launched before the win. Seeds 1 and 5 never vowed.
- **The rival's own conquests are where most losses come from** (3 / 0 / 11 / 0 / 2), which no `Takes` column
  names (section 6.4).

### 6.3 What the specialties moved (against STEP6.md's table, the MP2 layout)

| Seed | Win day, step 6 on MP2 layout to MP3 | Takes (A / S / Sub / Inv) | Defied | Bands | Vows / reprisals / retakes | Taken / lost / held |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 22.6 to **19.9** | 2 / 0 / 6 / 1 to 1 / 0 / 6 / 0 | 1 to 3 | 1 to 0 | 0 / 0 / 0 to 0 / 0 / 0 | 9 / 1 / 8 to 7 / 6 / 1 |
| 2 | 24.9 to **17.5** | 1 / 0 / 8 / 0 to 1 / 0 / 5 / 0 | 1 to 1 | 0 to 0 | 4 / 1 / 0 to 2 / 0 / 0 | 9 / 6 / 3 to 6 / 1 / 5 |
| 3 | 21.7 to **lost 33.4** | 1 / 0 / 8 / 2 to 0 / 0 / 10 / 8 | 3 to 2 | 0 to 0 | 0 / 0 / 0 to 1 / 1 / 2 (1) | 11 / 5 / 6 to 18 / 15 / 3 |
| 4 | 20.4 to **19.3** | 2 / 0 / 6 / 0 to 1 / 0 / 6 / 0 | 1 to 0 | 1 to 0 | 2 / 1 / 0 to 1 / 1 / 0 | 8 / 1 / 7 to 7 / 0 / 7 |
| 5 | 20.5 to **20.3** | 1 / 0 / 7 / 1 to 1 / 0 / 6 / 1 | 0 to 0 | 0 to 0 | 0 / 0 / 0 to 0 / 0 / 0 | 9 / 2 / 7 to 8 / 2 / 6 |

Specialties (`Market` towns on the roads, `Mining` towns on the hills, `Fishing` and `Timber` towns: seed 1
has 4 Mining, 6 Market, 2 Fishing, 1 Timber, 12 Farming; STEP9a.md) changed which towns a demand reaches and
what the army finds in the stores. Seeds 1, 2 and 4 now win 2.7, 7.4 and 1.1 days sooner, seed 5 is the same,
and seed 3 is lost. The median of all five (loss last) falls from 21.7 to 19.9, and the second Attack that
raised STEP6's two bands is gone. **No limit moves outside its band.**

### 6.4 Lane A's open note: how each lost town was lost

STEP6.md: "on seed 2 the bot lost 6 towns with only 1 defiance and no retakes or revolts counted". **On this
tree seed 2 loses one town** (the note was on the MP2 layout, where its figures were 9 taken, 6 lost, 3 held;
here 6 / 1 / 5). Reading its chronicle (`--chronicle`, and the probe's ownership log): **Reedhaven (328
people) knelt on day 5.9 (tick 14149, cause `Submission`) and defied the player on day 11.0 (tick 26400,
`Defied`, cause `Defiance`)**; no other town of the bot's changed hands. Seed 2's other towns (Reedby,
Copperford, Marshcombe, Northstow) are held to the win; Iverbrook (the seat) falls to the Attack on tick 41,975.

The same pattern the note describes (losses beyond the defiances, no retakes or revolts counted) is on seeds 1
and 3 here, and the probe shows what it is. Every town of the bot's that changed hands, with its event
(ticks; `D` = `Defied`, `R` = `Revolt` against the bot, `C` = a Conquest by the rival's own army):

| Seed | Lost (tick, how) |
| --- | --- |
| 1 (6) | Sedgeholt D 26400; Gorsefield D 36000; Sedgeholt C 38891 (captain 3721); Loweby C 42327 (same captain); Pikecross C 44018 (same captain); Rookworth D 45600. **3 defiances + 3 conquests by the rival's raiding band**, no vow, no reprisal |
| 2 (1) | Reedhaven D 26400 |
| 3 (15) | Defied: Vanestow 38400, Bramthorpe 50400. Revolt against the bot: Thornbrook 52800, Heathhaven 60000. Rival conquests (11): Sedgewick 31824, Thornbrook 35022, Heathhaven 36535, **Thornbrook 38107 (raid band 5081 again, 107 ticks after the reprisal left; the conquest the bot counts as a retake)**, Daleholt 54116, Vanestow 62013, Tarnwell 63330, Duncombe 70654, Hollowbrook 72200, Kesthaven 73426, Bramthorpe 78364 |
| 4 (0) | none |
| 5 (2) | Emberbrook C 32629 (captain 5553), Heathwick C 39413 (captain 1082) |

- **No town changes hands without an event.** The probe saw 104 ownership changes of populated towns on the
  five seeds; 80 carry a Conquest, Invited, Submitted, Revolt or Defied event within a tick of the change, and
  the figures agree with the run summary's taken and lost on every seed (seed 1: 7 and 6; seed 3: 18 and 15).
  The other 24 are by design: when the player takes the seat, `rival::free_lands` frees every other town the
  rival holds, quietly (no event per town: "the run has ended"); they are all `rival -> free` flips on the four
  winning ticks. No flip of a town of the bot's lacks an event.
- **The `Takes` columns do not name the rival's ordinary conquests,** which is what makes lost towns look
  unexplained. The probe matched the captain of each of the 16 conquests (seeds 1, 3 and 5: 3, 11 and 2)
  against the chronicle's `RivalRaid` and `RivalReprisal` events: **15 were taken by `RivalRaid` bands** (the
  rival's periodic raids, which `Attack` the nearest town of the player's, as before step 6; seed 1's three
  all by one captain, 3721, within 5,100 ticks) and **1 by a reprisal band** (seed 3, Duncombe, tick 70,654, 13.6 days
  after that band was sent for Thornbrook). A column for them would make the line add up (seed 1: 3 defied + 3
  = 6 lost; seed 3: 2 defied + 2 revolts + 11 = 15; seed 5: 2).
- **FAILURE (a reporting bug in the bot, not a sim bug): `retaken` over-counts.** `Bot::takes` counts every
  rival Conquest of any town a `RivalReprisal` ever named, including conquests before the vow. Seed 3 reports
  `retaken 2` with one reprisal sent: the rival's raid band 5081 conquered Thornbrook at tick 35,022; the
  player's invitation took it back at 36,200, which made the rival vow (due 38,000); the reprisal band 5863 left
  at about 38,000; **raid band 5081 conquered Thornbrook again at 38,107**, and the reprisal band never retook
  it (it took Duncombe at 70,654 instead). So at most one conquest (38,107) follows the vow, and the first
  (35,022) precedes it: the bot counts both. A correct count cannot exceed the number of reprisals sent; seed 3
  reports 2 against 1, and two runs outside the gate set do too (seed 11: `reprisals 2 retaken 3`; seed 42,
  the stage-check run: `reprisals 2 retaken 3`). Fix in `crates/sim_headless/src/bot.rs` (`Takes` in
  `takes()`): count a Conquest only after the `RivalReprisal` that names its town, once per reprisal, and say
  which band (a reprisal band's or a raid band's) took it. No sim state, replay or hash is involved.

### 6.5 Market hires (SIM-62), per seed

A Market town the player holds posts a `Hire` offer every 5 days (a band of 3 for 3 food: `market.min_men` 3,
`people_per_man` 80); the bot answers a hire it can reach and pay for, but never walks over to one.

| Seed | Offers posted | Hired | Lapsed | Notes |
| --- | --- | --- | --- | --- |
| 1 | 0 | 0 | 0 | no Market town held until the seat (a Market town) fell on the winning tick |
| 2 | 0 | 0 | 0 | none held |
| 3 | 8 | 2 | 6 | Hollowbrook (days 10, 15, 20, 25, 30) and Bramthorpe (20, 25, 30); hired on days 15 and 25 |
| 4 | 2 | 0 | 2 | Otterford, days 10 and 15 |
| 5 | 7 | 2 | 5 | Keststead, Ashgate, Heathwick, Heathhaven; hired on days 10.8 and 15; the start `Sellswords` band (12 men) was hired on day 0 as well |

4 of 17 offers were taken, 3 men each. The mechanism works (offers appear on the 5-day marks, a hire joins the
champion's army, an unanswered band moves on the next day: "The sellswords grew tired of waiting for Your
warband and moved on"), and at 3 men for 3 food it is small. Whether it matters is the user's call (check 6).

### 6.6 Compared with STEP6.md's open issues

1. "Fallout is thinly exercised by the bot": now **not exercised at all for bandits** (above); vows come on
   seeds 2, 3 and 4 and reprisals launch on seeds 3 and 4 (and, in the scan, on seeds 7 to 12).
2. "Bandits still pick your towns early": untested on this tree for the same reason.
3. "The median win day sits near the ceiling": 19.9 now, 2.5 under it.

## 7. The ten-day envoy check

`cargo test --release -j 2 -p integration_tests --test influence -- --ignored --nocapture`: **passes** (94 s).
Seed 1: target #15 (net 0.712, goodwill 0.509), invited on **day 7**. Seed 42: target #16 (net 0.684, goodwill
0.553), invited on **day 8**. (MP2 and MP1: day 7 and day 8, the same figures.)

## 8. Machine notes

- Disk: 9.8 GB free at the start; the full suite's compile phase took it to 3.8 GB after the last batch (the
  integration binaries were deleted batch by batch, as in MP2), then `prune_target.sh 0` and
  `rm -rf target/debug/incremental` brought it back to 11 GB for the release work. Every debug test binary is
  gone from `target/debug/deps`; the next full suite rebuilds them. 11 GB free at the end.
- Release artifacts in `scratchpad/m4a/`: `sim_headless_mp3` (the gated binary), `sim_headless_probe3` (the
  stage probe, not for use), `sim_headless_mp3ns` (the gated tree with `villagers.ron` `specialties: []`, a
  cost experiment, not for use), and the unchanged `sim_headless_m4` and `sim_headless_mp2`.
- Timing noise: this box is about 25% slower than when baseline.md was recorded (the M4 binary's total is 2.0
  to 2.6 ms here against 1.776 there) and single runs swing 10 to 15%, so the binaries are compared with each
  other, never with baseline.md's absolute table.
- Logs in `scratchpad/m4a/` (not in the repository): `mp3_full.log`, `mp3_fmt.log`, `mp3_clippy2.log`,
  `mp3_clippy3.log`, `mp3_doc.log`, `mp3_release_build.log`, `mp3.ron`, `mp3_bot{3,4}.ron`,
  `mp3_{developed,skirmish,battle}.out`, `mp3_chron_{1..5}.{out,err}`, `mp3_scan_{6..13}.err`,
  `mp3_botrun.log`, `mp3_probe_run.log` and `mp3_probe2.log` (the ownership log), `mp3_influence.log`,
  `mp3_cost_*`, `mp3x_cost_*`, `mp3y_cost_*`, `mp3_stage42.out`.

## What was not changed

By the gate itself (the fixes of section 0 came after it): no code, no data, no test, no doc other than this page. Four temporary things existed and were removed before
the end: `tests/tests/mp3_probe.rs` (the per-seed counts and ownership log; deleted after each of its two runs,
`git status` clean after), the stage probe's four edits in `crates/` (restored with `git checkout -- crates`;
the tree was rebuilt clean afterwards and the release binary's sha256 equals the gated binary's), the
`specialties: []` edit to `assets/data/villagers.ron` for the cost experiment of section 5.3 (restored with
`git checkout -- assets`), and the matching lines in `.git/info/exclude` (deleted). The merge lead commits this
page.

## 9. Part 1 playtest for the user (about 30 minutes): go for part 2, hold, or still unweighted

Part 1 ("Take and hold", steps 1 to 6) is done at this merge point, and the plan hands its fuller playtest to
you. Command, from the repository root (a release build is required; seed 1 is the gentle island: "Kin and
neighbours", 12 men and the champion, 3 days of food, all unarmed, a Bumper harvest; the start boons are Well
fed / Armed / Silver tongue, pick **Armed**; Space pauses, the time bar sets the speed, F5 saves, and a click
on a feed line moves the camera to it):

```
cargo run --release -p game_app -- --seed 1
```

The names below are seed 1's. The rival's seat is **Kestridge**, in the east (a Market town); the start band
is in the west. **Mining** towns: Nettleley, Sedgeridge, Gorsefield (163 people), Bramwick. **Market** towns:
Marshhaven, Woldbrook, Heathwick, Tarnridge, Sedgecombe. Small towns the bot's demands reached first: Pikecross
(91 people) and Loweby (127); bigger ones: Sedgeholt (358), Rookworth (274). Do the checks in order; each is
about 3 to 5 minutes, and the clock can run fast between them.

**Two things to know first.** (1) The bot never sacks, so **no bandit band has ever risen in a bot run on the
combined tree** (section 6.2); check 2 is the first time anyone but a unit test sees them. (2) The bot wins by
making towns kneel (6 submissions a win), so check 1 carries the question the gate cannot answer.

1. **Demand a hamlet.**
   - *Do:* select your champion, right-click the nearest village, **Recruit 10**; right-click Pikecross and
     read the menu line **"Demand submission (need N within 40, have M)"**; press it once M is enough.
   - *Look for:* need and have always shown (the line carries the reason when you are refused); on success the
     feed reads "... knelt to your warband (91 people)", the label gains **[bent knee, watched]**, the panel
     gains **"Ways to take it"** and the watch line, and the **DUES** row is held at Tithe.
   - *Question:* did the town kneel because you had earned it (you gathered strength first), or for nothing?
     **Did Demand feel too strong?** The bot's wins use it five or six times each.

2. **Sack a town, and watch the broken men.**
   - *Do:* press P until the captain panel shows **Aggressive**, right-click a town of at least 60 people that
     is not yours (Sedgeholt, 358), choose **Attack**, and let the army hold its centre.
   - *Look for:* a feed line ending "sacked by the army of <your captain>", the label mark **[broken sword]**,
     the panel's tenure line "Yours since day N, sacked (resentment fades in D days)" and a *resentment* term
     in "Why they feel this way"; and about two game hours later **"N men of <town> took to the woods"** in the
     feed with **"Broken men abroad: N"** on the run bar. Click the line to follow the band: an unarmed,
     aggressive army that rises at the town's edge and raids the best-scoring town that is not the seat, not
     its home and not the last it raided (the rival's towns count double, yours single, free towns three
     quarters; near and ungarrisoned beats far and garrisoned), then asks the town you raid to drive it off.
   - *Question:* where do they go first, the rival or your own thin towns? Does the sack cost enough
     (neighbours' goodwill, resentment, a band behind you) that you would choose between it and a demand?

3. **The rival's reprisal vow, and its band at your newest holding.**
   - *Do:* take a town the rival owns, or a free town whose panel line "Influence: you X%, the rival Y%" shows
     the rival at 30% or more (check 7 shows where). The rival's side is the east, so run the clock fast on the
     way.
   - *Look for:* the game pauses (steady pauses on a vow); the feed reads "Rival vowed to retake <town> by day
     N, HH:00, <why>"; the run bar gains **"Vow: the rival will retake <town> by day N, HH:00"**; the town's
     label gains **[vow: day N, HH:00]**. From day 6, with at least 8 men in its garrison, the rival sends **"Rival
     sent N men to retake <town>"** (about 8, never more than half the garrison) and the band goes for the
     town you took **most recently**.
   - *Question:* is the 18 hours' warning enough to garrison that town (right-click it, **Garrison**)? Did the
     reprisal feel like the fair price of taking land, or a punishment that arrives too late or too early?

4. **The seat's realm line falling as you take its towns.**
   - *Look for:* on the run bar, **"The rival's realm: N of M towns, P people (X%) · seat defence +D"**, with
     **"(its clock says +C)"** added once the realm is no longer whole; the same line on Kestridge's own panel.
     Take or free rival towns (check 3 does) and watch N, X% and D fall toward 40% of the clock's value.
   - *Question:* does the seat feel like it weakens as you take its land, so that going for the towns is the
     way to the seat, or like a fixed wall? Does the number move enough to plan with?

5. **Forge and Tithe at a Mining town.**
   - *Do:* own Gorsefield (or Nettleley, Sedgeridge, Bramwick). In the settlement panel's **WORK** row press
     **Forge**; then set the **DUES** row to **Tithe** with an army within 60 tiles.
   - *Look for:* the label's second line **[Mining]** and the panel line "Specialty: Mining — ore and smiths: a
     Forge here arms your men"; the forecast "Forge: N now, M in all over D dawns" (or "Forge: nobody to move":
     a Mining town's smithy and mine are already staffed, so Forge flips the recipe instead of moving hands);
     within a day the **Workshops:** list's "Smithy (weapons): ..." line shows the weapons count rising (the
     census made 13 to 39 a day at a Mining town); the Tithe forecast "Tithe: X food and N arms to captain C
     now" and, at dawn, "Day N, dawn: your lands sent F food, A arms and M men to 1 band."
   - *Question:* does a Mining town on Forge and Tithe feel like an armoury that changes your fights, and is
     its food cost (about 5 to 7% of the island's food over six days) visible and fair?

6. **A Market town's hired men.**
   - *Do:* own a Market town (Marshhaven, Woldbrook, Heathwick, Tarnridge or Sedgecombe) and keep the champion
     within 40 tiles of it on a 5-day mark (day 5, 10, 15 ...).
   - *Look for:* the label's **[Market]** and the panel line "Specialty: Market — a crossroads: every 5 days
     hired men camp here while you hold it"; on the mark the game pauses on "Your warband has a choice to make"
     and the run bar gains a button **"Sellswords (3)"** whose **Hire** card reads "3 armed sellswords join your
     champion's army for 3 food from its baggage. Your champion must be within 40 tiles of their camp. Then
     they eat with you." (the camp itself, three men 16 tiles from the town, is on the map); unanswered,
     "The sellswords grew tired of waiting for Your warband and moved on" a day later.
   - *Question:* is a band of 3 worth walking the champion over, or noise? (The bot took 4 of 17 offers.)

7. **The influence overlay (O).**
   - *Do:* press **O** until the run bar reads **"Overlay: influence (O): amber front, dashed sphere edge"**.
   - *Look for:* your sphere and the rival's, the amber front where they meet, rings filling on towns warming
     to you, and **"Sphere N, warming M"** on the run bar.
   - *Question:* do the shapes match the towns you hold and where the armies are? Does it tell you which town
     to take next, or which will invite you?

8. **"Why they feel this way".**
   - *Do:* open the panel of the town you demanded, the town you sacked, a Mining or Market town and a free
     one.
   - *Look for:* the heading **"Why they feel this way"**, the tenure line ("Yours since day N, it knelt to a
     demand" / "sacked"), "Influence: your pull X%, the rival's Y%", "Loyalty X%, rising to Y%:" and its terms
     (food, treatment, time, then resentment, awe, bond, dues, table), "Thinks of you +0.21, of the rival
     -0.10" with the dated deed log, and "Revolt risk: ...".
   - *Question:* before a town defies you, could you have told from this panel that it would?

**Then answer the gate's question:** go (lane B starts step 8, building orders), hold, or "still unweighted"
(which reopens the design's section 1, the mix of verbs, rather than a half-built step 8). And tell the merge
lead, in one line: **did Demand feel too strong?** (The bot's win days depend on it: six kneelings a win.)
