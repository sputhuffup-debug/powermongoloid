//! GP-12, NFR-07: save and resume. A run in progress (the bot's band, a
//! warband that took a hamlet, the rival's lands and garrison, raiders,
//! guard camps, a pending offer) is saved after a game day and loaded into
//! a fresh app: the state hashes match, every entity has the same
//! components, and the original and the loaded game keep agreeing for
//! 1,000 more ticks of the same commands, with either executor. A
//! coverage test fails if any hashed type isn't saved. See ADR 0016.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use bevy_ecs::prelude::*;
use common::{owner_of, spawn_id};
use sim_core::{
    CommandKind, CommandLog, Offers, OrderSpec, RunConfig, SaveRegistry, SimStepper, SimTick,
    SpawnId, compute_state_hash,
};
use sim_core::{FactionId, hash::HashRegistry};
use sim_headless::bot::Bot;
use sim_headless::{
    App, HeadlessConfig, SimPlugins, Threading, headless_app, load_app, resume_commands, save_app,
};
use sim_military::{ArmySpec, Captain, RunProgress, spawn_army};
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
