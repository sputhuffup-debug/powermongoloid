# M4a merge point 2 (MP2): the gate

Branch `claude/brave-turing-sh1ryn` at `8182929` ("MP2: tuning decisions ..."): MP1 (steps 1 to 3), lane B's
step 4 (edicts), lane A's step 5 (taking towns), and the merge lead's two tuning decisions
(`demand.unarmed_weight` 0.5, `demand.watch_radius` 125; ADR 0020 "As built: Merge point 2"). Gate:
[M4A_PLAN.md](../M4A_PLAN.md) "The gate"; baselines: [baseline.md](baseline.md) and `baseline/*.out`; the
previous gate: [MP1.md](MP1.md). Run on 2026-10-13, 4 cores, `-j 2`, `--test-threads=2`,
`CARGO_INCREMENTAL=0`. **Nothing in the code or data was changed by this gate** (see "What was not
changed" at the end); the only file added is this page.

**Verdict: RED.** Everything passes except the balance rows of `bot_run`: the bot wins 5 of 5 seeds, but
**four seeds win before day 14** (days 8.2, 7.1, 11.1, 7.3) and the **median is day 8.2**, against a floor of
14 and a median limit of 14.4 to 22.4. The cost check passes on the total (one single-window caveat, see
section 5), the stage check passes by a wide margin, and the smoke playtest checklist is in section 9.

## Summary

| # | Check | Result |
| --- | --- | --- |
| 1 | `cargo fmt --all --check` | clean |
| 1 | `cargo clippy -j 2 --workspace --all-targets -- -D warnings` | clean (0 warnings) |
| 1 | `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps` | clean |
| 2 | Full suite (`cargo test --workspace`, no fail-fast, `--test-threads=2`) | **422 passed, 0 failed, 3 ignored**; run in five batches for disk, and the command as briefed has a typo (section 2) |
| 3 | Determinism tests (`determinism.rs`, 13) | 13 of 13 pass, in the suite |
| 3 | Run replay, seed 42, 3,000 ticks, 50 commands (51 recorded) | `--replay` and `--replay --single-threaded` both "replay ok ... checkpoints=3", `state_hash=0xffa9bfa76a10936b` |
| 3 | Bot replay, seed 5, 12,000 ticks (34 commands) | both replays "replay ok ... checkpoints=12", `state_hash=0x41b6514ca76bbbe5` |
| 3 | Bot replay, seed 1, 40,000 ticks (49 commands; a bot win at tick 19,678, Demand and Equip in the log) | both replays "replay ok ... checkpoints=40", `state_hash=0xb30e14ba2c4ddb7b` |
| 4 | Guarded stdout, developed, skirmish, battle (release) | all three byte-identical to `baseline/*.out` (`cmp` silent; sha256 equal) |
| 5 | Cost, seed 7 steady, 7,200 ticks, 20 interleaved pairs against the M4 binary | total **+1.5% median, +0.3% mean** (inside +5%); first three pairs of session 1 +5.1%, of session 2 -0.5%; **window 1 +6.8% over the 20 pairs** (noise-dominated, flagged); see section 5 |
| 5 | Stage check, bot run seed 42 (win at tick 40,591): Lordship's own stages | **0.0022 ms a tick, 0.09% of the 2.54 ms mean tick** (limit 5%); worst 250-tick window 1.14% |
| 6 | `bot_run` (release, `--ignored`), wins | **5 of 5** (limit: at least 3): pass; quiet limit 999 of 1000 on every seed: pass |
| 6 | `bot_run`, win-day floor (no seed before day 14) | **FAIL: seeds 1, 3, 4 and 5 win on days 8.2, 7.1, 11.1 and 7.3** |
| 6 | `bot_run`, median within 14.4 to 22.4 | **FAIL: median 8.2** (7.1, 7.3, 8.2, 11.1, 17.2) |
| 7 | `influence` ten-day envoy check (release, `--ignored`) | passes: seed 1 invited on day 7, seed 42 on day 8 |

## 1. Build and lint

- `cargo fmt --all --check`: clean.
- `cargo clippy -j 2 --workspace --all-targets -- -D warnings`: exit 0, no `warning` or `error` line.
- `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps`: exit 0, no warning line.

## 2. The full suite

**The briefed command has a typo.** `cargo test -j 2 --workspace -- --test-threads=2 --no-fail-fast` puts
`--no-fail-fast` after `--`, so cargo hands it to the test binary, and the first binary run (`game_app`) died
with `error: Unrecognized option: 'no-fail-fast'`; cargo stopped there with exit 101 and no test had run. That
is a command-line error, not a code failure. The flag belongs before the `--`, as MP1 ran it
(`cargo test -j 2 --workspace --no-fail-fast -- --test-threads=2`).

**Run as batches, for disk.** Free space was 7.3 GB at the start, and the first attempt's compile phase alone
took it to 1.3 GB (each of the 28 integration test binaries is about 200 MB; `game_app`, `game_ui` and
`game_render` are 260 to 345 MB). The suite was therefore run as one script
(`scratchpad/m4a/mp2_suite.sh`), batch after batch, each with `-j 2`, `--no-fail-fast` and
`--test-threads=2`, deleting the finished integration binaries between batches:

0. every crate except `integration_tests` (unit tests, binaries, doc tests);
1. `army_march bot_run champion chronicle conquest determinism director`;
2. `docs dominion edicts famine gather influence invitation`;
3. `monuments orders posture rival run_start save_load sellswords`;
4. `smithy_chain starving_army take_modes tenure village_day wanderers worldgen`.

All 28 integration test files and every unit-test crate ran; the five batches exited 0. **422 passed, 0
failed, 3 ignored** (28.7 minutes including the compile). The ignored three are by design: `bot_run` and the
ten-day `influence` test (both run below) and one `sim_military` unit test.

| Binary | Passed | Failed | Ignored |
| --- | --- | --- | --- |
| unit: game_app 4, game_render 31, game_ui 59, sim_agents 30, sim_core 39, sim_economy 13, sim_headless 8, sim_military 97, sim_world 32 | 313 | 0 | 1 |
| army_march 1, champion 3, chronicle 1, conquest 4, determinism 13, director 3, docs 4, dominion 2, edicts 10, famine 1, gather 2, invitation 2 | 46 | 0 | 0 |
| influence 6, monuments 2, orders 7, posture 1, rival 2, run_start 4, save_load 5, sellswords 6, smithy_chain 2, starving_army 1 | 36 | 0 | 1 |
| take_modes 9, tenure 8, village_day 1, wanderers 2, worldgen 7 | 27 | 0 | 0 |
| bot_run | 0 | 0 | 1 |
| **total** | **422** | **0** | **3** |

(The four scenario tests that MP1 fixed, `champion`, `conquest`, `director` and `wanderers`, pass: 3, 4, 3, 2.
`take_modes` is 9 of 9 and `edicts` 10 of 10 on the merge.) The log is `scratchpad/m4a/mp2_full.log`.

## 3. Determinism and replays

Release binary: `CARGO_INCREMENTAL=0 cargo build -j 2 --release -p sim_headless` (2 m 02 s), kept as
`scratchpad/m4a/sim_headless_mp2` (sha256
`2ae505863158289d2f6790b0dbfc0186c3850439009ea22f452dd8b687fa1bf2`; after the cost probe below the tree was
rebuilt clean and gave the same sha256, so the binary is the tree's).

- Run replay: `--run --seed 42 --ticks 3000 --commands 50 --record mp2.ron` (51 commands, hash
  `0xffa9bfa76a10936b`), `--replay mp2.ron` and `--replay mp2.ron --single-threaded`: both "replay ok:
  seed=42 ticks=3000 commands=51 checkpoints=3", the same hash. (MP1's was `0xc2f12fc102601102`; the merge
  of steps 4 and 5 changes it, as it should.)
- Bot replay: `--run --bot --seed 5 --ticks 12000 --record mp2_bot.ron` (34 commands; MP1 had 26), both
  replays "replay ok ... checkpoints=12", hash `0x41b6514ca76bbbe5`, equal to the bot's own run.
- A longer bot replay, because five days on seed 5 never reach Demand: `--run --bot --seed 1 --ticks 40000`
  (the bot wins at tick 19,678; 49 commands: 2 Demand, 3 Equip), both replays "replay ok ...
  checkpoints=40", hash `0xb30e14ba2c4ddb7b`.
- `determinism.rs`, 13 of 13 in the suite, including `a_bot_run_replays` (the bot now demands).

## 4. Invariance: the three guarded stdout outputs

From the repository root, `cmp` silent for all three (release binary above):

| Output | sha256 (equal to the baseline's) |
| --- | --- |
| developed `--seed 42 --ticks 4801 --stats-every 600` | `cff552e813a9bd2494e3960af520250a81d50d028def36fb315b98c93927adeb` |
| skirmish `--skirmish --seed 42 --ticks 6000 --stats-every 2000` | `12762dd06ba25cc5feb4575f75da356c0d99e26f7076c3d0a5df4691c236f80c` |
| battle `--battle 1000 --seed 42 --ticks 3000 --stats-every 600` | `cb2c212ecbaf8203f7828c08c127414cd4a3fe41be9690ce3db3930d8429b7b8` |

## 5. Cost (NFR-11), live ticks only

### 5.1 The total and the windows

`--run --seed 7 --difficulty steady --ticks 7200 --timings --stats-every 2400`, the M4 binary
(`sim_headless_m4`) against the MP2 binary, interleaved, the order alternating, nothing else running (checked
with `top`; the box was idle). The same run without `--timings` printed no `run ended` line (`grep -c` gave
0), and every window has 4,490 to 4,507 agents (MP2: 4,499 to 4,502), so all three windows are live.

**This box is about 30% slower than when baseline.md was recorded** (the M4 binary's total is 2.03 to 2.65 ms
here, 1.776 there), so the two binaries are compared with each other, never with baseline.md's table. Two
sessions of ten pairs, run back to back:

| Session | Pairs | Total, M4 to MP2 (median) | Change, median / mean | First three pairs (the specified check) |
| --- | --- | --- | --- | --- |
| 1 | 10 | 2.236 to 2.261 ms | +1.2% / -0.8% | **+5.1%** (2.173 to 2.284) |
| 2 | 10 | 2.354 to 2.405 ms | +2.2% / +1.3% | -0.5% (2.407 to 2.396) |
| both | 20 | 2.312 to 2.347 ms | **+1.5% / +0.3%** | |

Per-pair change in the total, session 1: +5.1, +14.7, -8.0, +1.2, -2.7, +3.7, -11.8, -9.2, +9.8, -8.4 (%);
session 2: +10.8, -4.6, -5.3, +0.3, +4.9, +13.6, -1.4, +6.8, -0.9, -7.2. Over the 20: median -0.3%, mean +0.6%,
standard deviation 7.9%, t = 0.3; 6 pairs over +5%, 10 below zero. The total is **inside +5%**.

By window over the 20 pairs (median, with the paired mean and its t value):

| Window | M4 | MP2 | Change, median | Paired mean (sd, t) |
| --- | --- | --- | --- | --- |
| 1 to 2400 | 2.215 | 2.365 | **+6.8%** | +6.8% (13.0%, t = 2.3) |
| 2401 to 4800 | 2.315 | 2.280 | -1.5% | -3.6% (12.5%, -1.3) |
| 4801 to 7200 | 2.340 | 2.355 | +0.6% | +0.7% (12.9%, 0.3) |

**Two things to read carefully.**
- **The specified three-run check missed by 0.1 point in session 1** (+5.1%, three runs of a box whose single
  runs swing 10 to 15%) and passed in session 2 (-0.5%). With the sample tripled to the 20 pairs the total
  is +1.5%.
- **Window 1 alone is +6.8% over the 20 pairs** (+8.7% in session 2 and -2.5% in session 1; the sign
  flips between sessions). Its spread is the widest (the M4 binary's own window 1 runs from 1.79 to 2.64 ms)
  and its best-of-20 is +3.9%, but the paired t of 2.3 means it may be a real early-game cost. baseline.md's
  wording ("a median window mean above the Median +5% column fails") would fail it; MP1 judged the total and
  reported windows. This page does the same, and flags window 1 for the next gate (the merge lead's call).

Stage means over the 20 pairs (median, ms a tick): Decide 0.609 to 0.682 (**+12.1% median, +9.2% paired mean,
t = 4.5**, +0.074 ms, 3.2% of an M4 tick); Input 0.22 to 0.21, Knowledge 0.15 to 0.15, Move 0.52 to 0.51, Act
0.32 to 0.33, Needs 0.23 to 0.23, Bookkeeping 0.23 to 0.23 (M4 to MP2, the two sessions' medians averaged; no
stage but Decide moves). Decide is
the same +0.07 to +0.09 ms MP1 recorded (step 2's smithy chain in `ai::decide`): **steps 4 and 5 added nothing
measurable to it**, and the botless run owns no town, so it does not see step 4's or step 5's work at all.
Raw outputs: `scratchpad/m4a/mp2_cost_{m4,mp2}_{1..10}.out`, `mp2b_cost_{m4,mp2b}_{1..10}.out`.

### 5.2 The bot-run stage check (from step 4 on)

The plan asks for `--run --bot --seed 42 --ticks 96000 --timings --stats-every 250` with Lordship's own stages
(slots 13 to 16, `orders::lands`, `construction::advance`, `jobs::reassign`) summed and kept under 5% of the
mean tick. `--timings` reports per `SimSet` only, and the repository has no per-system timer (no tracing
subscriber), so this was measured with a **temporary probe, since removed**: a wall-clock guard
(`Instant`, an atomic nanosecond counter per stage) around `influence::step`, `tenure::step`, `govern::step`,
`fallout::step` and `orders::lands`, and a line of cumulative counters after each `--stats-every` window in
`--timings` mode. It was built into a separate binary (`scratchpad/m4a/sim_headless_probe`), the sources were
restored at once with `git checkout -- crates` (the tree was clean before the run), and the tree was rebuilt
clean afterwards.

The bot wins seed 42 at **tick 40,591 (day 16.9)** with 95 commands. The 161 live windows from tick 251 to
40,500 (4,539 to 4,723 agents; the first window is left out because the counters also hold the bot's own
play-through) give a **mean tick of 2.541 ms** and, in ms a tick:

| Stage | ms a tick |
| --- | --- |
| slot 13 `influence::step` | 0.0002 |
| slot 14 `tenure::step` | 0.0009 |
| slot 15 `govern::step` | 0.0002 |
| slot 16 `fallout::step` | 0.0000 (empty until step 6) |
| `orders::lands` | 0.0009 |
| **sum** | **0.0022 = 0.09% of the mean tick** (median window 0.08%, worst 250-tick window 1.14%) |

Limit 5%: **passes by a factor of fifty.** Not measured, because they do not exist yet or sit inside another
stage: `construction::advance` (step 8) and `jobs::reassign` (called only from `govern::step`, so already in its
figure). The bot's other costs (it plays through ordinary commands) are in the mean tick, not in this sum.

## 6. `bot_run`: balance

`cargo test --release -j 2 -p integration_tests --test bot_run -- --ignored --nocapture` (277 s): the test
passes (5 wins, needs 3; the director's quiet limit holds, 999 of 1000 ticks on every seed). **The gate's
win-day floor and median fail.** A second, throwaway probe test (since removed) played the same five seeds and
printed what `bot_run` does not: it agreed with `bot_run` on every win day, command count and take.

### 6.1 Win days

| Seed | M4 baseline | MP1 (fixed) | Step 5 alone (lane A's note) | **MP2** | Before day 14? |
| --- | --- | --- | --- | --- | --- |
| 1 | 17.4 | 16.9 | 8.2 | **8.2** | **yes** |
| 2 | 19.3 | 19.1 | 5.8 | **17.2** | no |
| 3 | 26.3 | lost 32.0 | 6.8 | **7.1** | **yes** |
| 4 | 18.4 | 25.1 | 12.0 | **11.1** | **yes** |
| 5 | 15.5 | 18.1 | 11.5 | **7.3** | **yes** |
| median | 18.4 | 19.1 | 8.2 | **8.2** | |

- **Wins:** 5 of 5 (limit: at least 3). Pass.
- **Floor** (no seed before day 14): **FAIL**, four seeds (7.1, 7.3, 8.2 and 11.1; the earliest is day 7.1).
- **Median** (the limit is 14.4 to 22.4, four days either side of the baseline's 18.4): **FAIL**, 8.2.

The merge lead's tuning (unarmed men count half, the watch reaches 125 tiles) moved only seed 2 much (5.8 to
17.2); seeds 1, 3 and 4 are where step 5 alone left them, and seed 5 got faster (11.5 to 7.3).

### 6.2 Per seed: the numbers the gate asks for

Counted in the same run as the win days. "Takes by way" is `Takes` (the `bot_run` line); "Forsaken", "weapons
tithed", "men levied" and the dial counts come from the second probe (the feed's `Tribute` and `EdictSet`
events, read every tick; "arms" is **weapons and tools together**, which is how `Tribute` stores them);
"revolts" are `Revolt` events against you in the chronicle (defiances are a separate event kind).

| Seed | Win day | Attack | Sack | Submitted | Invited | Subverted | Defiances | Forsaken | Arms tithed (food) | Men levied | Revolts vs you | Taken / lost / held (people) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 8.2 | 1 | 0 | 2 | 0 | 0 | 2 | 0 | 8 (16) | 0 | 0 | 3 / 2 / 1 (0) |
| 2 | 17.2 | 1 | 0 | 5 | 0 | 0 | 2 | 0 | 163 (118) | 0 | 1 | 6 / 3 / 3 (206) |
| 3 | 7.1 | 1 | 0 | 2 | 0 | 0 | 2 | 0 | 41 (7) | 0 | 0 | 3 / 2 / 1 (0) |
| 4 | 11.1 | 1 | 0 | 3 | 0 | 0 | 1 | 0 | 51 (43) | 0 | 0 | 4 / 1 / 3 (55) |
| 5 | 7.3 | 1 | 0 | 2 | 0 | 0 | 0 | 0 | 11 (73) | 0 | 0 | 3 / 0 / 3 (118) |

Other columns, same runs: commands 49 / 173 / 38 / 48 / 38; demands issued 2 / 5 / 2 / 3 / 2, none refused;
gifts 0 on every seed; `Tribute` events (a tithe reaching a captain) 4 / 10 / 2 / 4 / 5; dials set by the
player (the bot) 0 / 6 / 1 / 3 / 0 and by the tribute rule (`Cause::Edict`; stewards come in step 9) 2 / 5 / 2 /
3 / 2. Subverted is 0 by construction (step 7 is not built); bandit bands, reprisals and retakes do not exist
before step 6.

**What the numbers say, without fixing anything.**
- **The route to the seat is Demand, twice, then one Attack.** In every seed the bot makes two to five towns
  kneel (seed 1: Pikecross, 91 people, on day 1, and Northwell, 244, on day 4; seed 3: Thornbrook, 104, on
  day 2, and Duncombe, 248, on day 4) and takes the seat on days 7 to 11 in four seeds, against a garrison of
  25 to 40 men (estimated defence 85 to 190 on seeds 1 and 3). Sacks, invitations and courting never fire (the bot's `take_mode` tries
  Demand first, and it qualifies at once).
- **Its army is still mostly unarmed.** Seed 1's main army on day 4 is 46 unarmed of 54 men; the tithes
  deliver only 8 to 163 arms a run. The half weight on unarmed men did not keep the bot from demanding.
- **The defiances arrive after the win is in sight, not before it:** seed 1's two towns defy on days 8 and 9
  (the seat falls on day 9), seed 3's both on day 8 (the seat falls on day 8). Both seeds end holding one
  town ("1 (0 people)" in the run summary).
- **No man reached an army through a Levy** (0 men levied on every seed, so the Levy dial does nothing in the
  bot's runs); Forsaken never happens.
- ADR 0020's MP2 section already assigns the floor to step 6 (the seat follows its realm). The gate still
  counts it: step 5's own note flagged it, the tuning did not close it, and it stays open into MP3.
  Candidates already on record (docs/m4a/STEP5.md "Open issues" 1; HOWTO "Tune run balance"), none applied
  here: a seat that needs more than an unbled army (the clock's defence, the `rival` rows), a stiffer demand
  (`demand.ratio`, or a smaller `unarmed_weight`), or a bot that will not assault before it holds more land
  (`assault_margin`).

## 7. The ten-day envoy check

`cargo test --release -j 2 -p integration_tests --test influence -- --ignored --nocapture`: **passes** (87 s).
Seed 1: target #15 (net 0.712, goodwill 0.509), invited on **day 7**. Seed 42: target #16 (net 0.684,
goodwill 0.553), invited on **day 8**. (MP1: day 7 and day 8.)

## 8. Machine notes

- Disk: 7.5 GB free at the start; the full suite's compile phase took it to 1.3 GB, so the integration
  binaries were deleted batch by batch and `prune_target.sh 0` plus `rm -rf target/debug/incremental` run
  before the release builds (14 GB free after). Every debug test binary is gone from `target/debug/deps`; the
  next full suite rebuilds them.
- Release artifacts in `scratchpad/m4a/`: `sim_headless_mp2` (the gated binary), `sim_headless_probe` (the
  stage probe, not for use), `sim_headless_m4` (the baseline, unchanged).
- Logs in `scratchpad/m4a/` (not in the repository): `mp2_full.log`, `mp2_clippy.log`, `mp2_doc.log`,
  `mp2_release_build.log`, `mp2.ron`, `mp2_bot.ron`, `mp2_bot1.ron`, `mp2_{developed,skirmish,battle}.out`,
  `mp2_cost_*`, `mp2b_cost_*`, `mp2_stage42.out`, `mp2_botrun.log`, `mp2_probe.log`, `mp2_influence.log`,
  `mp2_chron_{1,3}.out`.

## What was not changed

No code, no data, no test, no doc other than this page. Three temporary things existed and were removed
before the end: `tests/tests/mp2_probe.rs` (the per-seed counts; deleted, `git status` was clean after),
the stage probe's seven edits in `crates/` (restored with `git checkout -- crates`, `git status` clean), and
the instrumented `target/release/sim_headless` (rebuilt clean; its sha256 equals the gated binary's). The
page is not committed.

## 9. Smoke playtest for the user (about 15 minutes): the go or hold for part 2

Command, from the repository root (a release build is required; the island is seed 1, the gentle one, a band
of kin and neighbours with 3 days of food, all unarmed; pick the **Armed** boon, press Space to pause, F5
saves):

```
cargo run --release -p game_app -- --seed 1
```

1. **Demand a hamlet** (about 3 min). Select your champion, right-click the nearest village, **Recruit 10**,
   then right-click a small village that is not yours and read the menu line "Demand submission (need N within
   40, have M)". Look for: the line always shows need and have (greyed with the reason when short); at enough
   strength the town kneels ("... knelt to your warband (N people)" in the feed), its label gains
   "[bent knee, watched]", its panel shows "Ways to take it" and the watch line, and its DUES row is held at
   Tithe. The question: did that feel earned, or free? (The bot wins on day 7 to 11 by demanding twice.)
2. **Sack a town** (about 3 min). Press P until the captain panel shows **Aggressive**, right-click a bigger
   town that is not yours, **Attack**, and let the army hold its centre. Look for: a "sacked" line in the
   feed, the town's food, weapons and tools moving into your baggage (the captain panel's Food), its tenure
   mark on the label, and "sacked" in the town's panel. The question: does a sack cost enough (neighbours'
   goodwill, resentment) that you would choose between it and a demand?
3. **Set Forge at an owned town** (about 3 min). Click an owned town near its centre (one of the larger ones
   has smithies), and in the settlement panel's **WORK** row press **Forge**. Look for: the forecast line
   ("Forge: N now, M in all over D dawns"), a "Forge" badge with a countdown on the label, the first smiths
   moved within the hour (open a villager: "Smith, moved from farmer by the Forge edict, day N"), and over a
   day or two the weapons count in the panel's workshop stocks rising. Then set **DUES** to Tithe and look for
   the dawn line "... lands sent captain N ... arms".
4. **Read the influence overlay** (about 2 min). Press **O** until the run bar reads "Overlay: Influence".
   Look for: your blue sphere and the rival's purple, an amber front where they meet, rings filling on towns
   warming to you, and "Sphere N, warming M" on the run bar. The question: do the shapes match the towns you
   hold and where the armies are?
5. **Read "Why they feel this way"** (about 4 min). Open the panel of the town you demanded, the town you
   sacked and a free one. Look for: the loyalty target term by term (food, treatment, time, then Awe, Bond,
   Resentment, Dues, Table), goodwill toward you and toward the rival, the deed log, the revolt risk, and a
   tenure line ("Yours since day N, it knelt to a demand" or "... sacked"). **Then answer the gate's
   question:** go (lane B starts step 8), hold, or "still unweighted" (which reopens the design's section 1,
   the mix of verbs, rather than a half-built step 8).

Tell the merge lead also, in one line, whether Demand felt too strong: the gate above says the bot's win
days depend on it.
