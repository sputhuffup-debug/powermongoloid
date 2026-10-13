# M4a merge point 1 (MP1): the gate

Branch `claude/brave-turing-sh1ryn` at `d8ac0ab` ("MP1: merge lane B's step 2 (the smithy chain, SIM-64)
from claude/m4a-govern"): lane A's steps 1 and 3 merged with lane B's step 2. Gate:
[M4A_PLAN.md](../M4A_PLAN.md) "The gate"; baselines: [baseline.md](baseline.md) and `baseline/*.out`.
Run on 2026-10-13, 4 cores, `-j 2`, `--test-threads=2`, `CARGO_INCREMENTAL=0`. Nothing in the code or
data was changed by this gate (see "Nothing was changed" at the end).

**Verdict: RED.** Four integration tests fail in the full suite, the bot lost one of its five seeds (it
won 4 of 5, so the 3-of-5 rule holds) and the win-day median is over the limit when the lost seed is
counted. Everything else passes: fmt, clippy, doc, determinism, both replays, the three guarded
outputs and the ten-day envoy check. The cost check is inside the limit over ten runs and over it in
the specified three.

## Summary

| # | Check | Result |
| --- | --- | --- |
| 1 | `cargo fmt --check` | clean |
| 1 | `cargo clippy -j 2 --workspace --all-targets -- -D warnings` | clean |
| 1 | `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps` | clean |
| 2 | Full suite (`cargo test -j 2 --workspace -- --test-threads=2`) | **FAILED: 4 tests** (373 passed, 4 failed, 3 ignored); see section 1 |
| 2 | Determinism tests (`determinism.rs`, debug) | 13 of 13 pass |
| 3 | Run replay, seed 42, 3,000 ticks, 50 commands (51 recorded) | recorded, then `--replay` and `--replay --single-threaded` both "replay ok", `state_hash=0xc2f12fc102601102`, 3 checkpoints |
| 3 | Bot replay, seed 5, 12,000 ticks (26 commands) | both replays "replay ok", `state_hash=0xf9592b234479dafd`, 12 checkpoints |
| 4 | Guarded stdout, developed, skirmish, battle (release) | all three byte-identical to `baseline/*.out` (`cmp` silent; sha256 equal) |
| 5 | Cost, seed 7 steady, 7,200 ticks, three interleaved runs per binary | **+9.1% median, +6.6% mean** on the total: over +5%; over ten runs +1.8% median, +3.8% mean; see section 3 |
| 6 | `bot_run` (release, `--ignored`) | wins 4 of 5 (passes the 3-of-5 rule); **seed 3 lost**; earliest win day 16.9 (floor holds); median **23.0** over all five seeds (limit 22.4, FAIL), 20.55 over the four wins (inside); see section 4 |
| 7 | `influence` ten-day envoy check (release, `--ignored`) | passes: seed 1 invited on day 7, seed 42 on day 8, no captain near |

## 1. The full suite

Command: `CARGO_INCREMENTAL=0 cargo test -j 2 --workspace -- --test-threads=2`. The first run (log
`scratchpad/m4a/mp1_full.log`) stopped at the first failing test binary, `champion`, as cargo does. To see
every failure the suite was run again with `--no-fail-fast` (`mp1_full_nofailfast.log`, 784 s):
**373 passed, 4 failed, 3 ignored**. The ignored three are by design: `bot_run`, the ten-day `influence`
test (both run below) and one `sim_military` unit test. The failures are deterministic: the champion
test failed the same way on three further runs (threads 2, 2 and 1).

Test files that failed (all four are run-preset scenario tests; the rest of each file passes):

1. **`champion`: `soldiers_who_fought_rank_up_with_the_configured_gains`** (SIM-36),
   `tests/tests/champion.rs:467`: `enough fed, led soldiers seen`. Printed:
   `warband battle ended at 200: 17 of 18 survivors fought, 3 fell (lost 3)` and
   `morale targets: ranked 0.970 (17 soldiers), unranked 0.940 (0)`. The check needs at least 3 of each:
   no soldier of the 6-man reserve was fed and within `leadership.radius` of its captain 1,500 ticks
   later (0 seen). The battle also differs from the one TESTING.md records for this test ("18 of 19",
   "2 dead"): one more man fell.
2. **`conquest`: `a_plundered_village_loses_faith`** (GP-06, SIM-24),
   `tests/tests/conquest.rs:514`: `revolted by tick 7200`, left `FactionId(0)` (independent), right
   `FactionId(1)` (the player). Printed day by day: loyalty mean 0.453 / 0.318 / 0.600 (seed 0.41),
   food 0.55 / 0.10 / 0.00 days, "below since Some(4800)", owner 1 at ticks 2400 and 4800 and owner 0 at
   7200; raiders seized 17.7 food, provoked 1, killed 3 villagers, 0 rejected. Step 1 had already added
   a 0.3-day larder to this scenario for the resentment scar (see the test's doc comment); the hamlet
   now still starves into the revolt on the third day.
3. **`director`: `a_band_with_nothing_to_do_lingers_then_withdraws`** (GP-16),
   `tests/tests/common/mod.rs:40` (`spawn_id`: "sim entity"), reached from `tests/director.rs:249`:
   the test waits for the band's captain to take a `GatherFood` order and then for that order to end
   (the wait also ends if the captain entity is gone), then reads the captain's `SpawnId` for its
   printout, and the entity no longer exists. So the band was destroyed or disbanded during the raid,
   before the test's linger window, instead of lingering and withdrawing.
4. **`wanderers`: `a_band_that_forages_and_answers_a_request_is_fed_and_grows`**,
   `tests/tests/wanderers.rs:366`: `the band didn't grow: 10 followers`. Printed: seed 25, band of 10 and
   its champion, 2.00 days of food, hamlet #2 (30 people, 80 tiles away) without food, 35.8 wild food
   within 32 tiles; foraged 2.40 by tick 401; request #5 posted at tick 100 for 6.75 food (reward one
   good #5, 1 volunteer, goodwill 0.2); 9.00 food given; request done at tick 1100, 1 volunteer joined;
   on day 5 (tick 12000) 10 followers, 1.73 days of food, worst hunger 0.25, foraged 57.3 in all, 0
   deserted, no starvation. The band ends where it began (10), so the volunteer's gain was lost or never
   counted.

**Where they come from (not bisected).** Git state could not be changed, so no commit was tested alone.
The record says these four files passed on the step 1 branch (`s1_full.log`: 328 passed, 0 failed) and
before step 2 existed (M4: `gf_full.log`). Step 2's gate in [STEP2.md](STEP2.md) did not run any of
the four, and step 3's notes say champion, director and wanderers were last run on lane A before its
"heartland" change. Suspects, in order: step 2's smithy chain (`haul.run_fixes`, which changes
how every run town hauls and feeds, so food, weapons and the militia move in all four scenarios), then
step 3's Contest term and drifter ease. The quickest split for whoever fixes it: run these four files
with `economy.ron` `haul.run_fixes: false`, then with `progression.ron` `influence.enabled: false`.
An experiment of the first kind was started and abandoned (see the end of this page).

Passing in the same log: every unit-test binary (sim_military 97 passed with 1 ignored, game_ui 38,
game_render 31, and the other sim crates); `smithy_chain` 2 of 2, `influence` 6 of 6 (1 ignored),
`tenure` 8 of 8, `invitation` 2 of 2, `save_load` 5 of 5, `run_start` 4 of 4, `docs` 4 of 4,
`village_day` and `famine` 1 of 1 each, `determinism` 13 of 13, `orders` 7 of 7, `worldgen` 7 of 7,
`sellswords` 6 of 6, and `dominion`, `monuments`, `rival`, `gather`, `posture`, `starving_army`,
`army_march` and `chronicle` all ok. The per-binary list is in `mp1_full_nofailfast.log`.

## 2. Replays and invariance

Release binary built with `CARGO_INCREMENTAL=0 cargo build -j 2 --release -p sim_headless` (2 m 09 s),
kept as `scratchpad/m4a/sim_headless_mp1` (sha256
`d7abe1cae112bda43aaddc32a4f6b028cf7923890c422206bb1739acaf3fd32e`).

- Run replay: `--run --seed 42 --ticks 3000 --commands 50 --record mp1.ron` (51 commands, hash
  `0xc2f12fc102601102`), `--replay mp1.ron` and `--replay mp1.ron --single-threaded`: both "replay ok:
  seed=42 ticks=3000 commands=51 checkpoints=3", same hash. (Step 2 alone read `0x9cf30340f172f84a` and
  step 3 alone `0x2dbe8d34a3659269`; the merge differs from both, as it should.)
- Bot replay: `--run --bot --seed 5 --ticks 12000 --record mp1_bot.ron` (26 commands, hash
  `0xf9592b234479dafd`), both replays "replay ok ... checkpoints=12", same hash.
- Guarded stdout, from the repository root, `cmp` silent for all three:

| Output | sha256 (equal to the baseline's) |
| --- | --- |
| developed `--seed 42 --ticks 4801 --stats-every 600` | `cff552e813a9bd2494e3960af520250a81d50d028def36fb315b98c93927adeb` |
| skirmish `--skirmish --seed 42 --ticks 6000 --stats-every 2000` | `12762dd06ba25cc5feb4575f75da356c0d99e26f7076c3d0a5df4691c236f80c` |
| battle `--battle 1000 --seed 42 --ticks 3000 --stats-every 600` | `cb2c212ecbaf8203f7828c08c127414cd4a3fe41be9690ce3db3930d8429b7b8` |

## 3. Cost (NFR-11), live ticks only

`--run --seed 7 --difficulty steady --ticks 7200 --timings --stats-every 2400`, the M4 binary
(`sim_headless_m4`) against the MP1 binary, interleaved. Both runs printed no `run ended` line before
tick 7,200 (`--stats-every 2400` without `--timings`, `grep -c "run ended"` gave 0 for each), and every
window has about 4,500 agents, so all three windows are live. Agents at ticks 2400, 4800 and 7200: M4
4507 / 4494 / 4490, MP1 4501 / 4502 / 4499.

**Today's M4 numbers are about 12% slower than baseline.md's** (total 1.77 to 2.22 ms against 1.776
there), so the box is slower or noisier now. Compare the two binaries with each other, not with
baseline.md's table.

Mean tick per window (ms) and the total (ms per tick over all 7,200), the three specified runs:

| Run | M4 w1 / w2 / w3 | M4 total | MP1 w1 / w2 / w3 | MP1 total |
| --- | --- | --- | --- | --- |
| 1 | 1.80 / 1.88 / 1.96 | 1.882 | 1.89 / 1.88 / 2.26 | 2.009 |
| 2 | 2.01 / 2.24 / 2.08 | 2.108 | 2.01 / 1.92 / 2.31 | 2.080 |
| 3 | 1.70 / 1.76 / 1.86 | 1.772 | 1.82 / 1.90 / 2.44 | 2.053 |
| median | 1.80 / 1.88 / 1.96 | **1.882** | 1.89 / 1.90 / 2.31 | **2.053** |
| mean | 1.84 / 1.96 / 1.97 | 1.921 | 1.91 / 1.90 / 2.34 | 2.047 |

Three runs: total **+9.1% on the median, +6.6% on the mean** (window 3 +17.9% on the median, +18.8% on
the mean; windows 1 and 2 +5.0% and +1.1% on the median). Over the +5% target, as specified.

The box was noisy (the M4 binary alone ranged 1.77 to 2.22 ms), so seven more runs were interleaved,
alternating who goes first (totals, ms):

| Set | M4 totals | MP1 totals |
| --- | --- | --- |
| runs 4 to 6 | 2.078, 1.972, 2.026 | 1.864, 2.226, 1.995 |
| runs 7 to 10 | 1.901, 2.034, 2.216, 2.067 | 2.403, 2.135, 1.929, 2.121 |

Over all ten pairs: M4 median 2.030, mean 2.006; MP1 median 2.067, mean 2.082: total **+1.8% median, +3.8%
mean**. By window over ten: w1 -2.5% / +1.9%, w2 0.0% / +5.0%, w3 +4.2% / +4.5% (median / mean). The one
stage that moves the same way in every set is **Decide: 0.536 to 0.581 ms (+8.5% median over ten)**, the
smithy chain's haul decisions; Act, Knowledge and Bookkeeping are within +3%. So the likely truth is
a cost of about +2% to +4% on the total from Decide, with run-to-run noise of about 10%; the first three
runs were unlucky. Worst ticks are spikes and are not gated.

Not measured here: the bot run's Lordship stages (the plan asks for them from step 4).

## 4. `bot_run` (balance)

`cargo test --release -j 2 -p integration_tests --test bot_run -- --ignored --nocapture` (493 s, test
passes: 4 wins, needs 3; the director's quiet limit also holds, longest quiet 999 of 1000 ticks on
every seed). Per seed, with the M4 baseline and step 2 (lane B's own gate) beside it:

| Seed | M4 baseline | Step 2 alone | **MP1** |
| --- | --- | --- | --- |
| 1 | won 17.4 | won 17.2 | **won 16.9** |
| 2 | won 19.3 | won 29.3 | **won 23.0** |
| 3 | won 26.3 | won 21.3 | **LOST on day 33.1** (killed in battle) |
| 4 | won 18.4 | won 22.1 | **won 25.1** |
| 5 | won 15.5 | won 16.2 | **won 18.1** |

- **Wins:** 4 of 5 (limit: at least 3). Seed 3's champion fell to the Rival on day 28 (a successor took up
  the banner, keeping 3 traits) and again on day 33, which lost the run.
- **Floor:** earliest win is day 16.9, so no seed won before day 14: holds.
- **Median:** the baseline's is 18.4, so the limit is 14.4 to 22.4. Over the four wins the median is
  20.55 (16.9, 18.1, 23.0, 25.1), inside. Over all five seeds, counting the lost run as the slowest, it is
  **23.0**, which is **0.6 day over**; the plan does not say how a loss counts, but a lost seed is
  worse than a late win, so this report reads it as a miss.

Takes by way and the rest, per seed, from `--run --bot --seed N --ticks 96000 --chronicle` on the MP1
binary (the run lengths match the test's: 16.9, 23.0, 33.1, 25.1, 18.1) and on the M4 binary (17.4, 19.3,
26.3, 18.4, 15.5, equal to baseline.md). Counts are the chronicle's lines; "taken" and "lost" add up to the
run summary's settlement counts on every seed.

| Seed | Taken by force (from the rival / from independent) | By invitation | Monuments taken / lost | Lost by force | Revolts against you | Settlements taken / lost / held (people) |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 16 (7 / 9) | 0 | 5 / 2 | 4 | 2 | 16 / 6 / 10 (904) |
| 2 | 24 (5 / 19) | 1 | 4 / 3 | 10 | 6 (3 more against the rival) | 25 / 16 / 9 (701) |
| 3 | 25 (4 / 21) | 0 | 8 / 4 | 2 | 16 (2 more against the rival) | 25 / 18 / 7 (593) |
| 4 | 21 (10 / 11) | 2 | 8 / 5 | 9 | 3 | 23 / 12 / 11 (902) |
| 5 | 18 (9 / 9) | 0 | 3 / 1 | 2 | 7 | 18 / 9 / 9 (826) |

Same seeds on the M4 binary: revolts against you 2 / 4 / 13 / 0 / 1; invitations 0 on all five; taken by
force 17 / 16 / 19 / 11 / 14; lost by force 3 / 7 / 0 / 3 / 0.

**Reading it.** MP1 takes more towns and loses far more of them to revolts (seed 5: 7 against 1; seed 4:
3 against 0; seed 2: 6 against 4; seed 3: 16 against 13), which fits step 1's resentment scar and step
3's Contest term pulling loyalty down after a conquest. The wins slow down and seed 3 is lost. The bot
accepted its first invitations of any kind (3 across seeds 2 and 4); the chronicle does not say whether
they came from envoys. Weapons made and equipped are not visible in a bot run (`--workshops` does not go
with `--bot`); defiances, Forsaken deeds, weapons tithed, men levied, bandit bands and reprisals do not
exist before steps 4 to 6.

## 5. The ten-day envoy check

`cargo test --release -j 2 -p integration_tests --test influence -- --ignored --nocapture`: **passes**
(90 s). Seed 1: target #15 at [824, 462] (neighbours #23 at 105 tiles and #8 at 126), net 0.703, goodwill
0.503, invited on **day 7**. Seed 42: target #16 at [206, 408] (neighbours #4 at 104 and #11 at 109), net
0.675, goodwill 0.545, invited on **day 8**. No captain within the invitation radius, and the run was
still in progress. These are the same numbers as step 3's own gate.

## Machine notes

- Disk: 6.2 GB free at the start; the full suite's debug test binaries took it to 2.2 GB mid-suite, so
  `prune_target.sh` was run twice (90 and 60 minutes) and `target/debug/incremental` removed (1.5 GB).
  After the suite, `prune_target.sh 5` left 13 GB free for the release builds. Rebuild debug test
  binaries when the next full suite runs.
- Logs and artifacts are in the scratchpad `m4a/` directory (`mp1_full.log`, `mp1_full_nofailfast.log`,
  `mp1_clippy.log`, `mp1_doc.log`, `mp1_cost_{m4,mp1}_{1..10}.out`, `mp1_botrun.log`,
  `mp1_chron_{1..5}.out`, `mp1_m4chron_{1..5}.out`, `mp1_influence.log`, `mp1.ron`, `mp1_bot.ron`); they
  are not in the repository.

## Nothing was changed

This gate changed no code and no data. One diagnostic was started and stopped: `assets/data/economy.ron`
was switched to `run_fixes: false` to split the four failures between steps 2 and 3, a debug rebuild was
begun, and then the experiment was abandoned and the build killed before any test ran. The file was put
back from a byte-for-byte copy taken first (`run_fixes: true`), and `git status` was clean afterwards
apart from this page. No test result in this page comes from that experiment.
