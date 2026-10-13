//! M4 step 11 balance test `bot_run` (GP-16, GP-21, GP-28; M4_PLAN
//! "Defaults chosen" 5): the scripted player (`sim_headless::bot`) plays
//! five fixed seeds of a steady run for at most 40 game days (96,000
//! ticks) each and must win at least three. The seeds run in parallel
//! threads, one app each. Ignored in the normal suite (minutes in release);
//! CI runs it with
//! `cargo test --release -p integration_tests --test bot_run -- --ignored`.
//!
//! The same runs check the pacing director's quiet limit over a whole run
//! (GP-16): every 25 ticks while the run goes on, the time since the last
//! notable event (`Chronicle::last_notable`) is at most the personality's
//! `quiet_limit_hours`.
//!
//! Each seed prints its outcome, day and key numbers (the run summary, the
//! bot's command count, the longest quiet spell) so a tuning pass can
//! compare before and after.

use sim_core::{Calendar, Cause, Chronicle, RunConfig, RunOutcome, RunState, SimTick};
use sim_headless::HeadlessConfig;
use sim_headless::bot::play_bot_with;
use sim_military::DirectorData;

/// The fixed seeds.
const SEEDS: [u64; 5] = [1, 2, 3, 4, 5];
/// The run's length limit, in game days.
const DAYS: u64 = 40;
/// Wins needed (M4_PLAN default 5: loosen this rather than fudge the data).
const MIN_WINS: usize = 3;
/// Ticks between quiet-limit checks.
const QUIET_EVERY: u64 = 25;

/// What one seed's run came to.
#[derive(Debug)]
struct Played {
    seed: u64,
    outcome: Option<RunOutcome>,
    cause: Cause,
    day: f32,
    commands: usize,
    /// The longest time without a notable event, and the limit (ticks).
    quiet: (u64, u64),
    summary: sim_military::RunSummary,
    /// Takes by way (M4a step 5): the bot's own count.
    takes: String,
}

fn play(seed: u64) -> Played {
    let config = HeadlessConfig::run(seed, RunConfig::default());
    let mut worst = 0u64;
    let mut limit = 0u64;
    let max = DAYS * 2_400;
    let (mut app, run) = play_bot_with(config, max, 0, |app| {
        let tick = app.world().resource::<SimTick>().0;
        if !tick.is_multiple_of(QUIET_EVERY) || app.world().resource::<RunState>().outcome.is_some()
        {
            return;
        }
        if limit == 0 {
            let cfg = app.world().resource::<RunState>().config.expect("a run");
            let p = app
                .world()
                .resource::<DirectorData>()
                .personality(cfg.difficulty)
                .expect("a personality")
                .clone();
            limit = app
                .world()
                .resource::<Calendar>()
                .hours(p.quiet_limit_hours);
        }
        let quiet = (tick - 1) - app.world().resource::<Chronicle>().last_notable();
        worst = worst.max(quiet);
    });
    let per_day = app.world().resource::<Calendar>().ticks_per_day;
    assert_eq!(per_day * DAYS, max, "a game day is 2,400 ticks");
    Played {
        seed,
        outcome: run.outcome,
        cause: run.cause,
        day: run.end_tick as f32 / per_day as f32,
        commands: run.log.len(),
        quiet: (worst, limit),
        summary: sim_military::run_summary(app.world_mut()),
        takes: run.takes.describe(),
    }
}

#[test]
#[ignore = "minutes in release; CI runs it with --ignored"]
fn the_bot_wins_three_of_five_runs_within_forty_days() {
    let runs: Vec<Played> = std::thread::scope(|scope| {
        let handles: Vec<_> = SEEDS
            .iter()
            .map(|&seed| scope.spawn(move || play(seed)))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("a seed's run panicked"))
            .collect()
    });
    for r in &runs {
        let s = &r.summary;
        println!(
            "seed {}: {:?} ({:?}) on day {:.1} | {} commands | settlements taken {} lost {} held {} ({} people) | monuments taken {} | battles {} kills {} lost {} | veterans {} | requests {} | longest quiet {} of {} ticks | {}",
            r.seed,
            r.outcome,
            r.cause,
            r.day,
            r.commands,
            s.settlements_taken,
            s.settlements_lost,
            s.settlements_held,
            s.people_held,
            s.monuments_taken,
            s.battles,
            s.kills,
            s.lost,
            s.veterans,
            s.requests_done,
            r.quiet.0,
            r.quiet.1,
            r.takes,
        );
    }
    // GP-16: never quiet for longer than the limit.
    for r in &runs {
        assert!(
            r.quiet.1 > 0 && r.quiet.0 <= r.quiet.1,
            "seed {}: {} ticks without a notable event (limit {})",
            r.seed,
            r.quiet.0,
            r.quiet.1
        );
    }
    let wins = runs
        .iter()
        .filter(|r| r.outcome == Some(RunOutcome::Won))
        .count();
    println!("wins: {wins} of {}", runs.len());
    assert!(
        wins >= MIN_WINS,
        "the bot won {wins} of {} runs (needs {MIN_WINS})",
        runs.len()
    );
}
