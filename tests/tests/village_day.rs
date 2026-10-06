//! M2 acceptance: one region with 5,000 villagers runs a full day cycle:
//! they go to work, produce goods, eat, return home and sleep (SIM-04,
//! SIM-10/11/12, SIM-20/21).

use sim_agents::Villagers;
use sim_core::{Calendar, Phase, SimStepper, compute_state_hash};
use sim_economy::Occupation;
use sim_headless::stats::{Stats, collect};
use sim_headless::{HeadlessConfig, headless_app};

const SEED: u64 = 42;
const SAMPLE_EVERY: u64 = 60;

/// Runs one full day, sampling the population through it.
fn run_day() -> (Vec<(Phase, f32, Stats)>, u64) {
    let mut app = headless_app(HeadlessConfig::new(SEED));
    let calendar = Calendar::default_data();
    let mut samples = Vec::new();
    let mut tick = 0;
    while tick < calendar.ticks_per_day {
        app.step(SAMPLE_EVERY);
        tick += SAMPLE_EVERY;
        let reading = calendar.at(tick - 1);
        samples.push((reading.phase, reading.phase_fraction, collect(&mut app)));
    }
    let hash = compute_state_hash(app.world_mut());
    (samples, hash)
}

#[test]
fn five_thousand_villagers_live_a_full_day() {
    let (samples, hash) = run_day();
    let last = &samples.last().unwrap().2;
    assert_eq!(
        last.villagers, 5000,
        "one region populated with 5,000 villagers"
    );

    // They go to work: through the work phase, workers are working,
    // hauling for their workplace, or walking there, apart from meal
    // breaks (eating is the AI's choice too) and carters waiting for the
    // first goods of the day.
    let work: Vec<&Stats> = samples
        .iter()
        .filter(|(p, f, _)| *p == Phase::Work && (0.15..0.95).contains(f))
        .map(|(_, _, s)| s)
        .collect();
    assert!(!work.is_empty());
    let shares: Vec<f32> = work
        .iter()
        .map(|s| s.working_or_commuting as f32 / s.workers as f32)
        .collect();
    let mean = shares.iter().sum::<f32>() / shares.len() as f32;
    assert!(
        mean >= 0.85,
        "on average only {:.0}% of workers at work",
        mean * 100.0
    );
    for (s, share) in work.iter().zip(&shares) {
        assert!(
            *share >= 0.6,
            "only {:.0}% of workers at work: {}",
            share * 100.0,
            s.line()
        );
    }

    // They return home and sleep: after the first stretch of the night,
    // nearly everyone is asleep in their own home.
    let night: Vec<&Stats> = samples
        .iter()
        .filter(|(p, f, _)| *p == Phase::Night && *f > 0.3)
        .map(|(_, _, s)| s)
        .collect();
    assert!(!night.is_empty());
    for s in &night {
        let share = s.asleep_at_home as f32 / s.villagers as f32;
        assert!(
            share >= 0.9,
            "only {:.0}% asleep at home: {}",
            share * 100.0,
            s.line()
        );
    }

    // They produce goods: every occupation that has workers completed work.
    for (occ, cycles) in &last.cycles {
        if *occ == Occupation::Idle {
            continue;
        }
        let workers = samples
            .iter()
            .any(|(_, _, s)| s.not_working.keys().any(|(o, _)| o == occ))
            || *cycles > 0;
        if workers {
            assert!(*cycles > 0, "no {} produced anything in a day", occ.name());
        }
    }

    // They eat: everyone has eaten, and starvation is rare.
    assert_eq!(
        last.ever_ate, last.villagers,
        "never ate: {:?}",
        last.never_ate
    );
    let starving = Villagers::default_data(&Calendar::default_data())
        .data
        .needs
        .starving;
    for (_, _, s) in &samples {
        assert!(
            s.mean_hunger < 0.5,
            "mean hunger {:.2}: {}",
            s.mean_hunger,
            s.line()
        );
    }
    assert!(
        last.max_hunger < starving,
        "someone is starving at day's end: {}",
        last.line()
    );

    // And the day is reproducible.
    let again = {
        let mut app = headless_app(HeadlessConfig::new(SEED));
        app.step(Calendar::default_data().ticks_per_day);
        compute_state_hash(app.world_mut())
    };
    assert_eq!(hash, again, "the day did not replay identically");
}
