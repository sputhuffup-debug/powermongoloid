//! M4 step 9 scenario `chronicle` (UI-05, UI-09; ADR 0017): the scripted
//! player (`sim_headless::bot`) plays three days of a run on seed 42, then
//! abandons it. The chronicle tells the run with causes: a battle with its
//! report (sides, losses, turning point, ranks earned), a monument taken by
//! holding it, a village's request answered or lapsed, and the run's end.
//! `describe` words each entry, naming the settlement or monument, and the
//! run summary counts what the chronicle says. The test reads the
//! `Chronicle` resource, never `SimEvent` messages.

mod common;

use common::{chronicle, command};
use sim_core::{
    Calendar, Cause, Chronicle, CommandKind, FactionId, NameTable, RunConfig, RunOutcome, RunState,
    SimEventKind, SimStepper, SimTick, SpawnId,
};
use sim_headless::bot::{self, Bot};
use sim_headless::{App, HeadlessConfig, headless_app};
use sim_military::chronicle::describe;
use sim_world::{Monument, Settlement};

const SEED: u64 = 42;
const DAYS: u64 = 3;

/// The name of settlement or monument `id` (a SpawnId).
fn place_name(app: &mut App, id: u64) -> String {
    let world = app.world_mut();
    let mut q = world.query::<(&SpawnId, Option<&Settlement>, Option<&Monument>)>();
    let name = q
        .iter(world)
        .find(|(s, ..)| s.0 == id)
        .and_then(|(_, s, m)| s.map(|s| s.name).or(m.map(|m| m.name)))
        .expect("a settlement or monument");
    world.resource::<NameTable>().get(name).to_string()
}

/// UI-09: a bot run's chronicle names a cause for every entry that has
/// one, carries a report per battle, and reads as sentences that name
/// their places; the summary agrees with it.
#[test]
fn a_bot_run_is_chronicled_with_causes() {
    let mut app = headless_app(HeadlessConfig::run(SEED, RunConfig::default()));
    let mut player = Bot::new(SEED);
    let end = DAYS * app.world().resource::<Calendar>().ticks_per_day;
    while app.world().resource::<SimTick>().0 < end
        && app.world().resource::<RunState>().outcome.is_none()
    {
        bot::step(&mut player, &mut app);
    }
    // The run's end: the player gives up, if nothing ended it first.
    if app.world().resource::<RunState>().outcome.is_none() {
        command(&mut app, FactionId::PLAYER.0, CommandKind::AbandonRun);
        app.step(1);
    }
    let state = *app.world().resource::<RunState>();
    assert!(state.outcome.is_some(), "the run is over");

    let entries = chronicle(&app);
    let texts = sim_military::describe_entries(app.world(), &entries);
    for t in &texts {
        println!("{t}");
    }
    assert_eq!(texts.len(), entries.len());
    for (e, t) in entries.iter().zip(&texts) {
        let day = app.world().resource::<Calendar>().at(e.tick).day + 1;
        assert!(t.starts_with(&format!("Day {day}: ")), "{t}");
        assert_eq!(
            describe(app.world(), e),
            *t,
            "one entry or many, the same words"
        );
        // Every kind but the run's start and a rank-up names its cause.
        if !matches!(
            e.event.kind,
            SimEventKind::RunStarted | SimEventKind::RankUp
        ) {
            assert_ne!(e.event.cause, Cause::None, "no cause: {t}");
        }
    }

    // A battle and its report: who fought whom, the losses, the ranks.
    let reports = app.world().resource::<Chronicle>().reports().to_vec();
    let battles: Vec<_> = entries
        .iter()
        .zip(&texts)
        .filter(|(e, _)| e.event.kind == SimEventKind::Battle)
        .collect();
    assert!(!battles.is_empty(), "no battle in {DAYS} days");
    assert_eq!(reports.len(), battles.len(), "a report per battle");
    for (e, t) in &battles {
        let r = app
            .world()
            .resource::<Chronicle>()
            .report_for(e)
            .expect("the battle's report");
        assert!(!r.foes.is_empty() && r.started <= r.tick, "{r:?}");
        assert_eq!(
            e.event.cause,
            Cause::Fight { faction: r.foes[0] },
            "a battle's cause is its first foe"
        );
        assert_eq!(
            (r.fought as u64, r.losses as i32),
            (e.event.other, e.event.amount)
        );
        assert!(t.contains("fought near") && t.contains("slain"), "{t}");
        assert!(
            r.turning_point
                .is_none_or(|p| p >= r.started && p <= r.tick)
        );
    }

    // A monument taken by holding it, named.
    let (taken, text) = entries
        .iter()
        .zip(&texts)
        .find(|(e, _)| e.event.kind == SimEventKind::MonumentTaken)
        .expect("a monument taken");
    assert_eq!(taken.event.cause, Cause::Hold);
    let name = place_name(&mut app, taken.event.subject);
    assert!(
        text.contains(&name) && text.contains("holding the ground"),
        "{text}"
    );

    // A request answered or lapsed, naming the village.
    let (request, text) = entries
        .iter()
        .zip(&texts)
        .find(|(e, _)| {
            matches!(
                e.event.kind,
                SimEventKind::RequestDone | SimEventKind::RequestFailed
            )
        })
        .expect("a request answered or lapsed");
    let want = match request.event.kind {
        SimEventKind::RequestDone => Cause::Request {
            id: request.event.subject,
        },
        _ => Cause::Lapsed,
    };
    assert_eq!(request.event.cause, want);
    let name = place_name(&mut app, request.event.other);
    assert!(text.contains(&name), "{text}");

    // The run's end, last, with its cause.
    let last = entries.last().expect("entries");
    assert_eq!(last.event.kind, SimEventKind::RunEnded);
    assert_eq!(last.event.cause, state.cause);
    assert_ne!(last.event.cause, Cause::None);

    // The summary agrees with the chronicle and the totals.
    let s = sim_military::run_summary(app.world());
    for line in s.lines() {
        println!("summary: {line}");
    }
    let mine = |kind: SimEventKind| {
        entries
            .iter()
            .filter(|e| e.event.kind == kind && e.event.faction == FactionId::PLAYER)
            .count() as u32
    };
    assert_eq!(s.outcome, state.outcome);
    if state.outcome == Some(RunOutcome::Abandoned) {
        assert_eq!(s.cause, Cause::Player);
        assert!((s.days - DAYS as f32).abs() < 0.01, "{}", s.days);
    }
    assert_eq!(s.battles, mine(SimEventKind::Battle));
    assert_eq!(s.monuments_taken, mine(SimEventKind::MonumentTaken));
    assert_eq!(s.requests_done, mine(SimEventKind::RequestDone));
    let promoted: std::collections::BTreeSet<u64> = reports
        .iter()
        .filter(|r| r.faction == FactionId::PLAYER)
        .flat_map(|r| r.promotions.iter().map(|p| p.soldier))
        .collect();
    assert_eq!(s.veterans as usize, promoted.len());
}
