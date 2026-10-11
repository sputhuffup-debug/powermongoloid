//! GP-12, NFR-07: save and resume. A run in progress (the bot's band, a
//! warband that took a hamlet, the rival's lands and garrison, raiders,
//! guard camps, a pending offer) is saved after a game day and loaded into
//! a fresh app: the state hashes match, every entity has the same
//! components, and the original and the loaded game keep agreeing for
//! 1,000 more ticks of the same commands, with either executor. A
//! coverage test fails if any hashed type isn't saved. A run saved before
//! its first tick (an in-place new run, `--save-at 0`) starts exactly like
//! a fresh one, its start and offers told on tick 0. See ADR 0016 and
//! ADR 0017.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use common::{owner_of, spawn_id};
use sim_core::{
    Chronicle, CommandKind, CommandLog, Difficulty, EventFeed, FeedEntry, LoggedCommand, Offer,
    OfferKind, Offers, OrderSpec, RunConfig, SaveRegistry, SimCommand, SimEvent, SimEventKind,
    SimStepper, SimTick, SpawnId, compute_state_hash, read_header,
};
use sim_core::{FactionId, hash::HashRegistry};
use sim_headless::bot::Bot;
use sim_headless::{
    App, HeadlessConfig, SimPlugins, Threading, continue_run, headless_app, load_app,
    record_replay, resume_commands, save_app, start_boon_pick, verify_replay_saving,
};
use sim_military::{ArmySpec, Captain, RunProgress, champion_of, spawn_army};
use sim_world::{Settlement, SettlementSize, SizeTier};

const SEED: u64 = 42;
/// A game day: the save is taken here.
const SAVE_AT: u64 = 2_400;
/// Ticks both games run on after the save.
const AFTER: u64 = 1_000;
const CHECK_EVERY: u64 = 50;

/// A 30-man player warband spawned before the first tick 20 tiles from
/// the hamlet nearest the start (toward the start). Returns the hamlet and
/// the warband's captain.
fn warband_beside_a_hamlet(app: &mut App) -> (Entity, Entity) {
    let world = app.world_mut();
    let run = world.resource::<RunProgress>();
    let (start, seat) = (run.start, run.rival_seat);
    let mut q = world.query::<(Entity, &SpawnId, &Settlement, &SettlementSize)>();
    let (_, _, hamlet, center) = q
        .iter(world)
        .filter(|(_, id, _, size)| size.tier == SizeTier::Hamlet && Some(id.0) != seat)
        .map(|(e, id, s, _)| ((s.center - start).as_vec2().length(), id.0, e, s.center))
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .expect("a hamlet on the run island");
    let toward = (start - center).as_vec2().normalize_or_zero();
    let at = center + (toward * 20.0).round().as_ivec2();
    let spec = ArmySpec {
        armed: true,
        ..ArmySpec::new(FactionId::PLAYER, at, 30)
    };
    (hamlet, spawn_army(world, &spec))
}

fn tick(app: &App) -> u64 {
    app.world().resource::<SimTick>().0
}

/// The player plays: the bot, except that it leaves the start boon offer
/// pending until `pick_at` (so the save holds an offer, and the loaded game
/// answers it), and the warband attacks the hamlet, then garrisons it once
/// it is taken. Steps until `until`.
struct Player {
    bot: Bot,
    start_offer: u64,
    hamlet: Entity,
    warband: Entity,
    garrisoned: bool,
    pick_at: u64,
}

impl Player {
    fn play(&mut self, app: &mut App, until: u64) {
        while tick(app) < until {
            if tick(app) == 0 {
                let id = spawn_id(app, self.hamlet);
                common::order(
                    app,
                    FactionId::PLAYER.0,
                    self.warband,
                    OrderSpec::Attack { settlement: id },
                );
            }
            if !self.garrisoned
                && tick(app).is_multiple_of(100)
                && owner_of(app, self.hamlet) == FactionId::PLAYER
                && app.world().get::<Captain>(self.warband).is_some()
            {
                self.garrisoned = true;
                let id = spawn_id(app, self.hamlet);
                common::order(
                    app,
                    FactionId::PLAYER.0,
                    self.warband,
                    OrderSpec::Garrison { settlement: id },
                );
            }
            let start = self.start_offer;
            if tick(app) == self.pick_at {
                common::choose(app, start, 1);
            }
            for c in self.bot.act(app) {
                if !matches!(c.kind, CommandKind::ChooseOffer { offer, .. } if offer == start) {
                    app.world_mut().write_message(c);
                }
            }
            app.step(1);
        }
    }
}

/// SpawnId -> names of the entity's components.
fn component_sets(app: &mut App) -> BTreeMap<u64, BTreeSet<String>> {
    let world = app.world_mut();
    let entities: Vec<(Entity, u64)> = world
        .query::<(Entity, &SpawnId)>()
        .iter(world)
        .map(|(e, id)| (e, id.0))
        .collect();
    entities
        .into_iter()
        .map(|(e, id)| {
            let names = world
                .inspect_entity(e)
                .expect("a live entity")
                .map(|c| c.name().to_string())
                .collect();
            (id, names)
        })
        .collect()
}

/// Steps `app` to `until`, hashing every `CHECK_EVERY` ticks.
fn hashes(app: &mut App, until: u64) -> Vec<(u64, u64)> {
    let mut out = Vec::new();
    while tick(app) < until {
        app.step(1);
        if tick(app).is_multiple_of(CHECK_EVERY) {
            out.push((tick(app), compute_state_hash(app.world_mut())));
        }
    }
    out
}

fn assert_same(what: &str, expected: &[(u64, u64)], actual: &[(u64, u64)]) {
    if let Some((e, a)) = expected.iter().zip(actual).find(|(e, a)| e != a) {
        panic!(
            "{what}: diverged by tick {}: 0x{:016x} vs 0x{:016x} (tick {})",
            e.0, e.1, a.1, a.0
        );
    }
    assert_eq!(expected.len(), actual.len(), "{what}: checkpoints");
}

/// GP-12, NFR-07, NFR-08: a saved run loads into a fresh app with an
/// identical state hash, and both play on identically.
#[test]
fn save_load_restores_an_identical_hash() {
    let mut app = headless_app(HeadlessConfig::run(SEED, RunConfig::default()));
    let (hamlet, warband) = warband_beside_a_hamlet(&mut app);
    // The bot answers offers and plays the champion; the test drives the
    // warband itself.
    let mut bot = Bot::new(SEED);
    bot.ignore(spawn_id(&app, warband));
    let mut player = Player {
        bot,
        start_offer: common::start_offer(&app),
        hamlet,
        warband,
        garrisoned: false,
        pick_at: SAVE_AT + 250,
    };
    player.play(&mut app, SAVE_AT);

    // What the save holds: a run with armies on the move, the player's
    // own village, the rival's lands, and an offer waiting.
    let world = app.world_mut();
    assert!(
        !world.resource::<Offers>().pending.is_empty(),
        "an offer is pending"
    );
    let captains = world.query::<&Captain>().iter(world).count();
    assert!(captains > 5, "armies on the island: {captains}");
    assert_eq!(
        owner_of(&app, hamlet),
        FactionId::PLAYER,
        "the warband took the hamlet"
    );
    let rival_lands = app
        .world_mut()
        .query::<&sim_agents::Owner>()
        .iter(app.world())
        .filter(|o| o.faction == FactionId::RIVAL)
        .count();
    assert!(rival_lands > 0, "the rival holds land");

    let start = Instant::now();
    let bytes = save_app(&mut app);
    let save_time = start.elapsed();
    let hash = compute_state_hash(app.world_mut());
    let header = sim_core::read_header(&bytes).expect("a header");
    assert_eq!(
        (header.seed, header.tick, header.run, header.preset.as_str()),
        (SEED, SAVE_AT, Some(RunConfig::default()), "run")
    );

    let start = Instant::now();
    let mut multi = load_app(&bytes, Threading::Multi).expect("the save loads");
    let load_time = start.elapsed();
    let mut single = load_app(&bytes, Threading::Single).expect("the save loads");
    println!(
        "save: {} bytes, saved in {:.0} ms, loaded in {:.0} ms (NFR-07: <= 3 s)",
        bytes.len(),
        save_time.as_secs_f64() * 1000.0,
        load_time.as_secs_f64() * 1000.0
    );
    assert!(save_time.as_secs_f64() <= 3.0 && load_time.as_secs_f64() <= 3.0);
    assert_eq!(tick(&multi), SAVE_AT);
    assert_eq!(compute_state_hash(multi.world_mut()), hash, "loaded hash");
    assert_eq!(compute_state_hash(single.world_mut()), hash, "loaded hash");
    let sets = component_sets(&mut app);
    assert_eq!(
        component_sets(&mut multi),
        sets,
        "every entity's components"
    );
    assert_eq!(
        save_app(&mut multi),
        bytes,
        "saving the loaded game gives the same bytes"
    );

    // The original plays on; the loaded games get the same commands.
    let logged = app.world().resource::<CommandLog>().0.len();
    let expected = {
        let mut out = Vec::new();
        while tick(&app) < SAVE_AT + AFTER {
            let next = (tick(&app) / CHECK_EVERY + 1) * CHECK_EVERY;
            player.play(&mut app, next);
            out.push((tick(&app), compute_state_hash(app.world_mut())));
        }
        out
    };
    let rest = app.world().resource::<CommandLog>().0[logged..].to_vec();
    println!(
        "{} commands before the save, {} after; final hash 0x{:016x}",
        logged,
        rest.len(),
        expected.last().unwrap().1
    );
    assert!(
        rest.iter()
            .any(|c| matches!(c.command.kind, CommandKind::ChooseOffer { .. })),
        "the loaded games answer the pending offer"
    );
    for (name, loaded) in [("multi", &mut multi), ("single", &mut single)] {
        resume_commands(loaded, &rest);
        let got = hashes(loaded, SAVE_AT + AFTER);
        assert_same(name, &expected, &got);
        assert_eq!(
            loaded.world().resource::<CommandLog>().0,
            rest,
            "{name}: the same commands applied"
        );
    }
}

/// GP-12: every type in the state hash is also saved (ADR 0016). Exempt
/// types may be saved too (the event feed, the army index).
#[test]
fn every_hashed_type_is_saved() {
    let mut app = App::new();
    app.add_plugins(SimPlugins {
        generate: false,
        ..SimPlugins::run(SEED, RunConfig::default())
    });
    let world = app.world();
    let hashed = world.resource::<HashRegistry>();
    let saved = world.resource::<SaveRegistry>();
    let missing: Vec<&str> = hashed
        .hashed_types()
        .filter(|(id, _)| !saved.contains(*id))
        .map(|(_, name)| name)
        .collect();
    assert!(missing.is_empty(), "hashed but not saved: {missing:?}");
    assert!(hashed.hashed_types().count() > 70);
    // Hashed components are saved in the order they are hashed.
    let hashed_names: Vec<_> = hashed.component_names().collect();
    let saved_names: Vec<_> = saved
        .component_names()
        .filter(|n| hashed_names.contains(n))
        .collect();
    assert_eq!(saved_names, hashed_names);
}

/// The feed's entries, oldest first.
fn feed(app: &App) -> Vec<FeedEntry> {
    app.world()
        .resource::<EventFeed>()
        .iter()
        .copied()
        .collect()
}

/// GP-12, GP-21, GP-27, UI-05, UI-09, NFR-08: an in-place new run in the
/// game, `--save-at 0` and an F5 before the first tick all load a save
/// taken before tick 0; that save must start exactly like a fresh run. The
/// fresh run, the save loaded into a new app (either executor), over a run
/// in progress and over a run just set up (the game's *Build it*) keep the
/// same state hash for 300 ticks and end with the same Chronicle and feed:
/// the start told (`RunStarted`, "far from the rival's seat") and every
/// offer setup posted reported on tick 0. Seed 5 draws the Sellswords, so
/// the feed holds the `Hire` offer that pauses the game (seed 1 does not).
#[test]
fn a_run_saved_before_its_first_tick_starts_like_a_fresh_one() {
    for seed in [5, 1] {
        let mut fresh = headless_app(HeadlessConfig::run(seed, RunConfig::default()));
        // Asserted last, so a regression reports its symptom first.
        let setup_events = fresh.world().resource::<Messages<SimEvent>>().len();
        let setup_offers: Vec<Offer> = fresh.world().resource::<Offers>().pending.clone();
        let hire = setup_offers
            .iter()
            .find(|o| matches!(o.kind, OfferKind::Hire { .. }))
            .map(|o| o.id);
        if seed == 5 {
            assert!(
                hire.is_some(),
                "seed 5 draws Sellswords; if data changes, pick a seed that does"
            );
        }
        let progress = fresh.world().resource::<RunProgress>().clone();
        let champion = champion_of(fresh.world_mut(), FactionId::PLAYER).expect("a champion");
        let champion = spawn_id(&fresh, champion);

        let bytes = save_app(&mut fresh);
        assert_eq!(read_header(&bytes).expect("a header").tick, 0);
        let pick = start_boon_pick(&fresh, 0).expect("a start boon offer");
        let hash0 = compute_state_hash(fresh.world_mut());

        let mut variants: Vec<(&str, App)> = Vec::new();
        let mut multi = load_app(&bytes, Threading::Multi).expect("the save loads");
        assert_eq!(
            save_app(&mut multi),
            bytes,
            "seed {seed}: saving the loaded game gives the same bytes"
        );
        variants.push(("loaded", multi));
        if seed == 5 {
            let single = load_app(&bytes, Threading::Single).expect("the save loads");
            variants.push(("loaded, single-threaded", single));
        }
        let harsh = RunConfig {
            difficulty: Difficulty::Harsh,
        };
        let mut running = headless_app(HeadlessConfig::run(seed + 1, harsh));
        running.step(30);
        sim_core::load_world(running.world_mut(), &bytes).expect("the save loads");
        variants.push(("loaded over a run in progress", running));
        let mut built = headless_app(HeadlessConfig::run(seed + 2, RunConfig::default()));
        sim_core::load_world(built.world_mut(), &bytes).expect("the save loads");
        variants.push(("loaded over a new run", built));
        for (name, app) in &mut variants {
            assert_eq!(
                compute_state_hash(app.world_mut()),
                hash0,
                "seed {seed}, {name}: the hash at tick 0"
            );
        }

        // Everyone picks the first start boon on tick 0, then plays on.
        let play = |name: &str, app: &mut App| {
            resume_commands(app, std::slice::from_ref(&pick));
            app.step(1);
            // What `read_feed` pauses on: the Hire offer, reported and open.
            if let Some(hire) = hire {
                assert!(
                    app.world().resource::<Offers>().get(hire).is_some(),
                    "seed {seed}, {name}: the Hire offer stands after tick 0"
                );
            }
            let mut out = vec![(1, compute_state_hash(app.world_mut()))];
            out.extend(hashes(app, 300));
            out
        };
        let expected = play("fresh", &mut fresh);
        let world = fresh.world();
        let tick0: Vec<SimEvent> = world
            .resource::<EventFeed>()
            .iter()
            .filter(|f| f.tick == 0)
            .map(|f| f.event)
            .collect();
        for (name, app) in &mut variants {
            let got = play(name, app);
            assert_same(&format!("seed {seed}, {name}"), &expected, &got);
            let world = app.world();
            assert_eq!(
                *world.resource::<Chronicle>(),
                *fresh.world().resource::<Chronicle>(),
                "seed {seed}, {name}: the Chronicle"
            );
            assert_eq!(
                feed(app),
                feed(&fresh),
                "seed {seed}, {name}: the event feed"
            );
            assert_eq!(
                world.resource::<EventFeed>().next_seq(),
                fresh.world().resource::<EventFeed>().next_seq(),
                "seed {seed}, {name}: the feed's next entry"
            );
        }

        // The fresh run told its start and its offers on tick 0.
        let first = world.resource::<Chronicle>().entries()[0];
        assert_eq!(first.tick, 0);
        assert_eq!(
            first.event,
            SimEvent::new(SimEventKind::RunStarted, FactionId::PLAYER)
                .subject(champion)
                .other(progress.rival_seat.expect("a rival seat"))
                .at(progress.start)
                .amount(progress.band as i32),
            "seed {seed}: RunStarted"
        );
        let mut offers: Vec<SimEvent> = setup_offers.iter().map(Offer::posted_event).collect();
        offers.sort_by_key(SimEvent::sort_key);
        let told: Vec<SimEvent> = tick0
            .iter()
            .copied()
            .filter(|e| e.kind == SimEventKind::OfferPosted)
            .collect();
        assert_eq!(told, offers, "seed {seed}: every offer reported on tick 0");
        assert_eq!(setup_events, 0, "seed {seed}: setup writes no SimEvents");
    }
}

/// GP-12, NFR-08: a replay saved before its first tick (`--replay R --save
/// X --save-at 0`) resumes from that save (`--load X --replay R`) with
/// every checkpoint matching, on either executor.
#[test]
fn a_replay_resumes_from_a_save_before_its_first_tick() {
    let config = HeadlessConfig::run(5, RunConfig::default());
    // The start offer is always the run's first offer, id 0.
    let pick = LoggedCommand {
        tick: 0,
        command: SimCommand::player(
            FactionId::PLAYER,
            CommandKind::ChooseOffer {
                offer: 0,
                choice: 0,
            },
        ),
    };
    let replay = record_replay(config, 300, &[pick], 100);
    let (ok, saved) = verify_replay_saving(&replay, Threading::Multi, Some(0));
    ok.expect("the replay reproduces");
    let bytes = saved.expect("saved before tick 1");
    for threading in [Threading::Multi, Threading::Single] {
        let mut app = load_app(&bytes, threading).expect("the save loads");
        let (result, _) = continue_run(
            &mut app,
            replay.ticks,
            &replay.commands,
            replay.checkpoint_every,
            None,
        );
        assert_eq!(
            replay.first_divergence(&result.checkpoints),
            None,
            "{threading:?}: the replay from the save at tick 0"
        );
    }
}
