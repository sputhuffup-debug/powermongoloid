# M4 step 11: bot run, balance, benchmark, playtest checklist

Built on `3a8baa0` (tip of `claude/brave-turing-sh1ryn`) on branch `claude/m4-bot-tuning`. Only `sim_headless`, `tests/`, CI and docs changed; `game_ui`, `game_render` and `game_app` are untouched. **No data file changed**: the bot wins steady runs on the shipped tuning, so the developed and skirmish outputs (village_day, famine, army_march, posture, starving_army, orders) are unaffected by construction.

## What was built

- **`sim_headless::bot` plays to win** (`crates/sim_headless/src/bot.rs`, public API unchanged: `Bot::new`, `Bot::act`, `bot::step`, `run_bot`, `play_bot`, `THINK_EVERY`; new `Bot::ignore(captain)` and `play_bot_with(config, max, every, after_tick)`).
- **`tests/tests/bot_run.rs`** (`#[ignore]`): seeds 1-5, steady, at most 40 days, one thread and app per seed; asserts wins >= 3 (`MIN_WINS`) and the director's quiet limit over each whole run (every 25 ticks while the run goes on); prints outcome, cause, day, commands and the run summary's numbers per seed.
- **`determinism.rs::a_bot_run_replays`** (normal suite) now plays two game days (4,800 ticks) in parallel, packs the log and checkpoints as a `Replay`, round-trips it through RON and verifies it serially (~18 s; it was 1,500 ticks without the RON step). It no longer expects the start boon's choice 0 (the bot picks by score).
- **`save_load.rs`**: the bot now orders every player captain, so the test calls `Bot::ignore` on its scripted warband.
- **Bench `tick/run_island`** in `crates/sim_headless/benches/tick.rs` (in `criterion_group!`).
- **CI** (`.github/workflows/ci.yml`): a run replay smoke after the existing one, and the bot balance run.
- **Docs:** `docs/M4_PLAYTEST.md` (new); TESTING.md (CI list, `bot_run` row, `a_bot_run_replays` and `save_load` rows, the bench); HOWTO "Tune run balance" recipe and the bench in "Profile"; ARCHITECTURE sim_headless row; GLOSSARY "Bot"; CLAUDE.md command line for a bot run.

## Bot strategy

Every game hour (`THINK_EVERY` = 100 ticks) the bot reads a `View`: settlements (owner, people, weapons and food in the storehouse, militia standing against the player), its captains (people, unarmed, food days, orders), the rival's seat, garrison and defence bonus, captain slots and hostile armies; every list is sorted by SpawnId. It keeps its own notes: a task per captain with a deadline, when each village was last recruited, failed attacks and the most militia seen per settlement, settlements to shun for a day, and the assault plan.

1. **Offers.** Boons and traits are scored from their effects (food and arms count fully only before day 3; captain slots 1.5 each; Attack x25, Defence x22, RecruitChance x16, Morale x14, ... Desertion negative); invitations and hired bands are accepted.
2. **Food first.** Below 1.2 days the main army gathers from the nearest owned settlement with spare food (preferred by 60 tiles), else an independent one; starving (< 0.5 days) it accepts any village holding food (a hungry neutral army takes a day's food); else it forages, moving 48 tiles on when a spot is bare.
3. **Grow.** Recruit in non-rival settlements of 30+ people within 220 tiles (owned preferred), each rested half a day, at most an eighth of the village and no more than keeps 1.5 days of food per head, while the army is under the assault target (seat defence x 1.5 x 1.1, at least 40). Under 30 people it recruits before it fights. Equip at a storehouse with 4+ weapons when 5+ followers are unarmed.
4. **Take land.** Attack the best settlement within 260 tiles (not the seat, not owned; score = distance - 0.3 x people) once the army outnumbers its expected militia (12% of its people, or the most seen, x1.5 per failed attack, plus hostile armies near and the garrison if near the seat) by 1.6 plus 5. Food requests within 150 tiles are answered when the baggage can spare it. Before day 12, the nearest unheld, unguarded monument within 200 tiles is taken by standing at it.
5. **Captains.** With a free slot and an owned settlement nobody holds, a follower is appointed: the first from an army of 24 (an heir if the champion falls), later ones from 40. Secondary captains garrison an owned settlement, recruit there when under 16 (village of 60+), and join the assault with 6+ men.
6. **The seat.** Seat defence = (garrison + seat militia estimate) x (1 + defence bonus). Once all captains' soldiers reach it times the margin (1.5 before day 8, 1.25, 1.0 from day 16, 0.75 from day 24, any army from day 32) with the main army fed (1.5 days), or a starving army has 70% of it, every captain marches to a staging point 70 tiles from the seat, then (all within 40 tiles, or after 10 hours) attacks it together. After two days, or with under 40% of the need left, the assault is called off and counts as a failure.
7. **Deadlines.** Each task gets a deadline (walk at 12 ticks a tile plus 300 ticks of work; 3,600 for attacks); an overdue task is dropped, and its settlement is shunned for a day. Foraging, holding and staging end on their own.

Commands are only `SimCommand::player(PLAYER, ..)`: `ChooseOffer`, `IssueOrder` (queued where it takes several orders) and `AppointCaptain`. Ties draw from `rng_key("sim_headless::bot") ^ captain` at the tick.

## Tuning log

The game needed no data change; all the tuning was in the bot. Seeds 1-5 (and 6-9 to check for overfitting), steady, 96,000 ticks, release CLI (`--run --bot`); "won@d" is the winning day.

| Version | Change | Seeds 1-5 | Extra seeds |
| --- | --- | --- | --- |
| step 7 skeleton | (baseline) | lost d7.9, d9.0, d8.6, d29.2, (5 not run) | |
| v1 | strategy above, militia 30% of people, margin 1.8 | won d36.4, d36.8, lost d13.5, won d29.3 | |
| v2 | starving armies take any village's food; recruit only with 2+ days of food | won d35.2, lost d33.5, lost d35.9, won d35.9, won d35.7 | |
| v3 | militia estimate 12% (seen: 1-27%, mostly 10-20%) or the most seen; margins fall with the clock; strike when starving | won d19.6, d35.2, lost d11.9, won d16.9, d16.1 | |
| v4 | recruit before fighting under 30 men; first captain at 24 (an heir) | won d16.9, d16.6, d35.9, d18.6, lost d33.9 | won d13.1, d31.0, d18.2, lost d20.6 |
| v5 | task timeouts; later captains only from 40 | won d16.8, d15.5, alive at d40, won d16.8, d17.7 | won d14.2, d24.2, lost d25.2, won d34.4 |
| v6 | shun settlements a captain couldn't reach | won d16.8, d15.5, lost d11.3, won d27.0, d34.3 | |
| **v7 (final)** | deadlines by distance (walk + work) instead of a flat 12 hours | **won d18.0, d30.6, d23.3, d24.7, d30.2** | won d14.0, d19.9, d22.2 |

Seed 42 (the CI smoke seed) with the final bot: won on day 35.2.

What the logs showed along the way: the seat's militia estimate dominated (v1 never struck before day 34; real militia was a tenth to a fifth of a settlement, and the seat fell to 28 men once); big armies starved because villages hold only 0.5-2 days of food; the main army sat on a recruit or monument order it could never finish (an unreachable centre) for days; and a lone champion with no other captain died to director bands, ending the run with the succession unused.

### Other difficulties (information only; the bar is steady)

| Difficulty | Seeds 1-5 |
| --- | --- |
| Chaotic | won d15.8, lost d6.8, lost d13.8, lost d18.1, won d25.2 (2/5) |
| Harsh | lost d2.8, d2.3, d15.8, d2.8, d8.9 (0/5) |

## Per-seed results (`bot_run`, release, final tree)

`cargo test --release -p integration_tests --test bot_run -- --ignored --nocapture`: **5 of 5 won**, 285 s wall on 4 cores.

| Seed | Outcome | Day | Commands | Settlements taken / lost / held (people) | Monuments | Battles | Kills / lost | Veterans | Requests | Longest quiet |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | Won (Conquest) | 18.0 | 80 | 12 / 3 / 9 (872) | 6 | 25 | 227 / 38 | 123 | 2 | 999 of 1,000 |
| 2 | Won (Conquest) | 30.6 | 228 | 22 / 9 / 13 (1,081) | 4 | 55 | 451 / 179 | 184 | 16 | 999 of 1,000 |
| 3 | Won (Conquest) | 23.3 | 106 | 12 / 6 / 6 (662) | 10 | 30 | 281 / 76 | 114 | 6 | 999 of 1,000 |
| 4 | Won (Conquest) | 24.7 | 118 | 16 / 0 / 16 (1,135) | 6 | 45 | 348 / 85 | 108 | 17 | 999 of 1,000 |
| 5 | Won (Conquest) | 30.2 | 145 | 18 / 12 / 6 (272) | 7 | 38 | 308 / 113 | 134 | 10 | 999 of 1,000 |

The director's quiet limit (steady: 10 h = 1,000 ticks) held over every whole run.

## Benchmark

- **`tick/run_island`** (`cargo bench -p sim_headless --bench tick -- run_island`, 4 cores): the bot plays seed 42 to day 12 once; a fresh app replays its commands to day 10 (run still going), then each iteration is `step(1)`. **Mean 1.51 ms per tick** (95% CI 1.45-1.58 ms), about 660 ticks/s, ~4,700 people.
- **`--run --bot --seed 42 --ticks 96000 --timings --stats-every 250`** (release; the run is won at tick 84,540): 338 windows of 250 ticks before the win, mean tick 1.44 ms on average (the worst window's mean 2.1 ms). Worst single tick per window: under 10 ms in 275 windows, 10-20 ms in 45, 20-50 ms in 17, and one of 85.3 ms (tick 26,500, day 11, `Decide` 0.73 ms mean in that window). Whole-run stage means: Input 0.15, Knowledge 0.08, Decide 0.30, Move 0.29, Act 0.19, Needs 0.14, Bookkeeping 0.13 ms; total 1.28 ms.

## CI changes

In `.github/workflows/ci.yml`, after the existing replay smoke run (which builds the release binary):

```yaml
- name: Run replay smoke (bot)
  run: |
    bin=target/release/sim_headless
    $bin --run --bot --seed 42 --ticks 24000 --record run.ron
    $bin --replay run.ron
    $bin --replay run.ron --single-threaded
- name: Bot balance run
  run: cargo test --release -p integration_tests --test bot_run -- --ignored --nocapture
```

Locally: the smoke recorded 39 commands and both replays said "replay ok" with `0x0ee28fd8d12d5724` (~37 s each); the balance run takes ~5 minutes on 4 cores plus the release build of the test.

## Checks run

- `cargo fmt --check`; `cargo clippy -j 2 --workspace --all-targets -- -D warnings`; `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps`: clean.
- `bot_run` (release, `--ignored`): 5 of 5.
- `chronicle`, `save_load`, `determinism::a_bot_run_replays`: pass (13.5 s, 11.5 s, 17.8 s).
- Full suite `CARGO_INCREMENTAL=0 cargo test -j 2 --workspace -- --test-threads=2`: see the last section.

## Open issues

- **Harsh and chaotic are much harder for the bot** (0/5 and 2/5). On harsh the director's armed bands (8+ from day 0) kill the 10-man band on days 2-3; the bot doesn't flee or hide in a village. Either the bot learns to avoid fights it can't win, or harsh's early bands are too strong for any start. Worth a look after the playtest.
- **Hardly any weapons on the run island:** the storehouses outside the rival's lands held 0 weapons in most of the bot's daily notes (6 at most, in all of them together), so armies stay unarmed except for the Armed boon and the starting gear (the bot's Equip step, which wants 4 in one place, never fired). The M2 smithing chain may not run at run-island sizes, or the weapons go elsewhere; not investigated.
- **Revolts are frequent** (up to 12 settlements lost in a run, seed 5): conquest seeds loyalty from goodwill that the attack itself spent, and the bot garrisons only as many settlements as it has captain slots. It retakes them, at a cost in men.
- **Requests mostly lapse** (100-230 failed per run): the bot answers food requests only when it can spare food and never drives raiders off on purpose; requests are not needed to win.
- **The seat's militia estimate is crude** (12% of its people, raised 1.5x per failed assault); a failed early assault makes the bot wait for the clock's last margin (seed 2 of v3: day 35).
- **The bot's notes are long** (an entry per new task); `--bot` prints them all to stderr. Fine for tuning, noisy otherwise.
- The bot keeps no state across a save/load (a fresh `Bot` after `load_app` plays on from what it sees, not from its notes). Only the headless harness uses it.
- Wins with large margins on steady (5/5 on days 18-31, extra seeds 3/3): the 3-of-5 bar has headroom; steady might be on the easy side for a human who plays as well as the bot (the playtest will tell).
