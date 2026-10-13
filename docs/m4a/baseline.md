# M4a baselines (the M4 build)

Recorded at the start of M4a step 1, before any M4a code change, on the M4 build: code commit
`5837cd8` (main, "M4 Rise"), checked out as `e0d68e9` on `claude/brave-turing-sh1ryn` (the commits
after `5837cd8` change docs only; `git diff 5837cd8 HEAD -- crates assets Cargo.toml Cargo.lock` is
empty). Every later M4a gate compares against this page and the files beside it.

- Binary: `sim_headless` built with `cargo build -j 2 --release -p sim_headless`
  (`CARGO_INCREMENTAL=0`), kept at
  `/tmp/claude-0/-home-user-powermongoloid/e435f2ed-693a-5cce-8727-a393bddc43ee/scratchpad/m4a/sim_headless_m4`
  (sha256 `7ac598569664f4fbf89a5411bd602d5e0af4393e03e518a98ef4aa61c295092d`). It lives in a
  scratchpad, so it is not in the repository; rebuild it from `5837cd8` if it is gone.
- Machine: 4 cores, shared with another lane's builds while these numbers were taken (load average
  about 1.9 at the end of the cost runs), so timings carry noise; stdout does not.

## 1. The three guarded stdout outputs (byte comparison)

Stdout only (stderr carries the world-generation report and speed, which vary). Each was run twice
on the M4 binary and the second stdout matched the file byte for byte, so the files are stable.

| File | Command (release) | Lines | sha256 |
|---|---|---|---|
| `docs/m4a/baseline/developed.out` | `sim_headless --seed 42 --ticks 4801 --stats-every 600` | 21 | `cff552e813a9bd2494e3960af520250a81d50d028def36fb315b98c93927adeb` |
| `docs/m4a/baseline/skirmish.out` | `sim_headless --skirmish --seed 42 --ticks 6000 --stats-every 2000` | 26 | `12762dd06ba25cc5feb4575f75da356c0d99e26f7076c3d0a5df4691c236f80c` |
| `docs/m4a/baseline/battle.out` | `sim_headless --battle 1000 --seed 42 --ticks 3000 --stats-every 600` | 42 | `cb2c212ecbaf8203f7828c08c127414cd4a3fe41be9690ce3db3930d8429b7b8` |

To check a build against them (from the repository root, `B` = the binary under test):

```sh
B=target/release/sim_headless
$B --seed 42 --ticks 4801 --stats-every 600 2>/dev/null | cmp - docs/m4a/baseline/developed.out
$B --skirmish --seed 42 --ticks 6000 --stats-every 2000 2>/dev/null | cmp - docs/m4a/baseline/skirmish.out
$B --battle 1000 --seed 42 --ticks 3000 --stats-every 600 2>/dev/null | cmp - docs/m4a/baseline/battle.out
```

`cmp` printing nothing is a pass. The three took 10.2 s, 13.6 s and 13.0 s on the M4 binary.

## 2. Cost baseline (NFR-11)

Command, three times in a row (release, this binary, nothing else of mine running):

```sh
sim_headless_m4 --run --seed 7 --difficulty steady --ticks 7200 --timings --stats-every 2400
```

Correction to the plan's wording: with `--timings` the per-window lines and the per-stage table go
to **stdout**; stderr carries only the world-generation report (4239 villagers, 25 settlements,
24 monuments). The `run ended` line is printed only by the `--stats-every` branch **without**
`--timings`, also on stdout, and the timings loop has no such check (a run that has ended freezes
`SimTick`, so its later windows would not measure live work). The check used here: the same run without `--timings` (`--run --seed 7 --difficulty steady --ticks 7200
--stats-every 2400 | grep "run ended"`) prints no `run ended` line, and all three timing runs show
live windows (mean 1.6 to 1.9 ms, 4,490 to 4,507 agents). So all three windows of 2,400 ticks are
live; no window is excluded. Later gates should run that `grep` once, since the sim they test may
end this run sooner.

### Mean tick per window (ms), three runs and the median

| Window (ticks) | Agents | Run 1 | Run 2 | Run 3 | Median | Median +5% |
|---|---|---|---|---|---|---|
| 1 to 2400 | 4507 | 1.68 | 1.83 | 1.63 | **1.68** | 1.76 |
| 2401 to 4800 | 4494 | 1.84 | 1.75 | 1.94 | **1.84** | 1.93 |
| 4801 to 7200 | 4490 | 1.80 | 1.74 | 1.86 | **1.80** | 1.89 |

Worst single tick per window (ms): 26.18 / 26.99 / 16.10 (median 26.18) in window 1, 22.83 / 19.81 /
16.81 (median 19.81) in window 2, 8.66 / 9.61 / 9.92 (median 9.61) in window 3. Worst ticks are
spikes (the daily and hourly serial steps) and noisy; do not gate on them.

Three costliest stages per window, mean ms per tick (the per-window line prints the top three), median of
three:

| Window | Decide | Move | Act |
|---|---|---|---|
| 1 | 0.43 | 0.39 | 0.25 |
| 2 | 0.48 | 0.45 | 0.27 |
| 3 | 0.46 | 0.40 | 0.26 |

### Mean per stage over all 7,200 ticks (ms per tick), the `--timings` table

| Stage (`SimSet`) | Run 1 | Run 2 | Run 3 | Median |
|---|---|---|---|---|
| Input | 0.165 | 0.162 | 0.168 | **0.165** |
| Knowledge | 0.116 | 0.120 | 0.120 | **0.120** |
| Decide | 0.458 | 0.449 | 0.463 | **0.458** |
| Move | 0.411 | 0.409 | 0.416 | **0.411** |
| Act | 0.259 | 0.261 | 0.266 | **0.261** |
| Needs | 0.193 | 0.196 | 0.196 | **0.196** |
| Aggregate | 0.005 | 0.005 | 0.005 | **0.005** |
| Lod | 0.004 | 0.004 | 0.004 | **0.004** |
| Bookkeeping | 0.166 | 0.169 | 0.172 | **0.169** |
| total | 1.776 | 1.776 | 1.810 | **1.776** |

The plan's limit is "at most +5% over this baseline": a median total above 1.865 ms (1.776 x 1.05),
or a median window mean above the "Median +5%" column, fails. The spread between my three runs was 2%
on the total but up to 12% on a single window mean (1.63 to 1.83 in window 1), so judge the whole-run
total and the window medians, not one run's one window, and re-run when another build is competing
for the cores.

## 3. `bot_run` baseline (win days)

From the existing log of the merge lead's run on current main (`5837cd8`), copied from
`/tmp/claude-0/-home-user-powermongoloid/e435f2ed-693a-5cce-8727-a393bddc43ee/scratchpad/gf_botrun.log`
(not re-run for this page; the 492 s test is a merge-point check). Command:
`cargo test --release -j 2 -p integration_tests --test bot_run -- --ignored`.

```
seed 1: Some(Won) (Conquest { captain: 116 }) on day 17.4 | 123 commands | settlements taken 17 lost 5 held 12 (969 people) | monuments taken 5 | battles 30 kills 225 lost 79 | veterans 97 | requests 2 | longest quiet 999 of 1000 ticks
seed 2: Some(Won) (Conquest { captain: 6174 }) on day 19.3 | 58 commands | settlements taken 16 lost 11 held 5 (538 people) | monuments taken 3 | battles 28 kills 379 lost 47 | veterans 109 | requests 5 | longest quiet 999 of 1000 ticks
seed 3: Some(Won) (Conquest { captain: 687 }) on day 26.3 | 216 commands | settlements taken 19 lost 13 held 6 (618 people) | monuments taken 6 | battles 33 kills 350 lost 81 | veterans 149 | requests 11 | longest quiet 899 of 1000 ticks
seed 4: Some(Won) (Conquest { captain: 709 }) on day 18.4 | 72 commands | settlements taken 11 lost 3 held 8 (978 people) | monuments taken 7 | battles 34 kills 302 lost 56 | veterans 113 | requests 10 | longest quiet 999 of 1000 ticks
seed 5: Some(Won) (Conquest { captain: 5400 }) on day 15.5 | 99 commands | settlements taken 14 lost 1 held 13 (1144 people) | monuments taken 4 | battles 35 kills 321 lost 63 | veterans 142 | requests 10 | longest quiet 999 of 1000 ticks
wins: 5 of 5
test the_bot_wins_three_of_five_runs_within_forty_days ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 492.53s
```

| Seed | Win day |
|---|---|
| 1 | 17.4 |
| 2 | 19.3 |
| 3 | 26.3 |
| 4 | 18.4 |
| 5 | 15.5 |

Wins 5 of 5; earliest 15.5; **median 18.4**. The M4a win-day floor (LORDSHIP_DESIGN, M4A_PLAN "The
gate"): no seed won before day 14, and the median within four days of this median, so between 14.4
and 22.4.

## 4. Not here

The weapons census (the smithy chain) is lane B's and is recorded in its own step notes.
