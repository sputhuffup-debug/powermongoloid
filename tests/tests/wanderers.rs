//! M4 step 3 scenario `wanderers` (GP-22, SIM-38): a landless band
//! survives on foraging and requests, and starves if it does neither.
//!
//! Both tests play the run of seed 25, whose band ("Hunters and
//! farmhands") is ten followers and a champion with two days of food, and
//! drive it only through player commands, as the UI would:
//!
//! - `a_band_that_forages_and_answers_a_request_is_fed_and_grows`: the
//!   nearest hamlet's stores spoil before the first tick, so it asks for
//!   food within the hour. The band forages near its start, walks over and
//!   `Give`s, which answers the request: the hamlet's volunteers join. It
//!   then lives off the land, moving on when a spot is bare. At day 5 it
//!   is fed (food in the baggage, nobody hungry, nobody gone) and has more
//!   than ten followers. The director is quiet, so none of its raider
//!   bands meets the band.
//! - `a_band_on_bare_rock_deserts_then_starves`: the same band stands on
//!   rock painted around its start before the first tick, more than
//!   `upkeep.forage_radius` from every settlement (so it can't help itself
//!   to a village's food), and doesn't forage. Once the baggage is empty it
//!   starts deserting within army.ron `first_desertion_within_days`, and
//!   those who starve to death are removed from the world (G4).
//!
//! Each test prints the numbers it checks.

mod common;

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use common::*;
use sim_agents::{CombatStats, Enlisted, Needs};
use sim_core::{
    Calendar, Cause, CommandKind, FactionId, Offers, OrderSpec, Position, Posture, SimEventKind,
    SimStepper, SimTick, SpawnId,
};
use sim_economy::{Economy, SettlementEconomy, Stockpile};
use sim_military::progression::Effect;
use sim_military::{
    Army, ArmyStats, Captain, Follower, Progression, RequestBoard, RequestKind, RunProgress,
    WildFood,
};
use sim_world::{Settlement, SettlementSize, SizeTier, Terrain, WorldMap};

/// The run whose band (ten followers, two days of food) both tests play.
const SEED: u64 = 25;
/// Day 5, where the fed band is checked.
const DAY: u64 = 5;
/// Hours the band forages at its start before it answers the request.
const FORAGE_FIRST_HOURS: u64 = 4;
/// Tiles of rock painted around the start in the starving branch.
const ROCK_RADIUS: i32 = 12;

fn tick(app: &App) -> u64 {
    app.world().resource::<SimTick>().0
}

fn ticks_per_day(app: &App) -> u64 {
    app.world().resource::<Calendar>().ticks_per_day
}

fn ticks_per_hour(app: &App) -> u64 {
    app.world().resource::<Calendar>().ticks_per_hour()
}

/// Sends `kind` for the player, as the UI does.
fn send(app: &mut App, kind: CommandKind) {
    command(app, FactionId::PLAYER.0, kind);
}

/// Sends `spec` to `captain`, replacing its orders, or queued after them.
fn give_order(app: &mut App, captain: Entity, spec: OrderSpec, queue: bool) {
    let captain = spawn_id(app, captain);
    send(
        app,
        CommandKind::IssueOrder {
            captain,
            order: spec,
            queue,
        },
    );
}

/// The run of [`SEED`] with its start boon picked: the first card whose
/// effects are all stat bonuses, so the band starts with exactly its
/// template's food and no goodwill anywhere. Returns the app and the
/// champion.
fn start() -> (App, Entity) {
    let mut app = run_app(SEED);
    let champion = champion(&mut app);
    let offer = start_offer(&app);
    let data = app.world().resource::<Progression>().data.clone();
    let cards = app
        .world()
        .resource::<Offers>()
        .get(offer)
        .expect("the start offer")
        .choices
        .clone();
    let pick = cards
        .iter()
        .position(|&c| {
            data.boons[c as usize]
                .effects
                .iter()
                .all(|e| matches!(e, Effect::Stat(..)))
        })
        .expect("a stat-only boon on offer");
    choose(&mut app, offer, pick as u8);
    let band = followers(&mut app, champion).len();
    let food = baggage_days(&mut app, champion);
    println!(
        "seed {SEED}: a band of {band} and its champion, {food:.2} days of food, picks {}",
        data.boons[cards[pick] as usize].name
    );
    assert_eq!(band, 10, "the template's ten followers");
    assert!(
        (1.8..=2.2).contains(&food),
        "about two days of food: {food}"
    );
    (app, champion)
}

/// People in the champion's band, the champion included.
fn band_size(app: &mut App, champion: Entity) -> usize {
    followers(app, champion).len() + 1
}

/// Days of food in `captain`'s baggage for its band.
fn baggage_days(app: &mut App, captain: Entity) -> f32 {
    let people = band_size(app, captain) as f32;
    let world = app.world();
    let econ = world.resource::<Economy>();
    let per_day = world
        .resource::<sim_agents::Villagers>()
        .data
        .needs
        .hunger_per_day;
    let food = world
        .get::<Stockpile>(captain)
        .map_or(0.0, |b| b.food_value(econ));
    food / (per_day * people)
}

/// The hungriest of the champion's band (champion included).
fn worst_hunger(app: &mut App, champion: Entity) -> f32 {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &Needs, Option<&Follower>)>();
    q.iter(world)
        .filter(|(e, _, f)| *e == champion || f.is_some_and(|f| f.captain == champion))
        .map(|(_, n, _)| n.hunger)
        .fold(0.0, f32::max)
}

/// Every populated settlement: (entity, SpawnId, centre).
fn centres(app: &mut App) -> Vec<(Entity, u64, IVec2)> {
    villages(app).into_iter().map(|v| (v.0, v.1, v.2)).collect()
}

/// The hamlet nearest `from` (never the rival's seat; ties: lowest
/// SpawnId): (entity, SpawnId, centre).
fn nearest_hamlet(app: &mut App, from: IVec2) -> (Entity, u64, IVec2) {
    let seat = app.world().resource::<RunProgress>().rival_seat;
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &SpawnId, &Settlement, &SettlementSize)>();
    q.iter(world)
        .filter(|(_, id, _, size)| size.tier == SizeTier::Hamlet && Some(id.0) != seat)
        .map(|(e, id, s, _)| ((s.center - from).as_vec2().length(), id.0, e, s.center))
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, id, e, c)| (e, id, c))
        .expect("a hamlet on the run island")
}

/// Spoils the food in `s`'s storehouse and homes (a setup edit before the
/// first tick); other goods stay.
fn spoil_food(app: &mut App, s: Entity) {
    let econ = app.world().resource::<Economy>().clone();
    let se = app.world().get::<SettlementEconomy>(s).unwrap().clone();
    for store in std::iter::once(se.storehouse).chain(se.homes.iter().copied()) {
        if let Some(mut stock) = app.world_mut().get_mut::<Stockpile>(store) {
            stock.goods.retain(|g, _| !econ.is_food(*g));
        }
    }
}

/// Where a player would take the band next to forage: of the passable
/// tiles 48 tiles away in 16 directions, more than `upkeep.forage_radius`
/// from every settlement, the one with the most wild food within
/// `radius` (ties: the first direction).
fn fresh_ground(app: &mut App, from: IVec2, radius: u16) -> Option<IVec2> {
    let keep_off = app.world().resource::<Army>().data.upkeep.forage_radius;
    let centres = centres(app);
    let world = app.world();
    let wild = world.resource::<WildFood>();
    let map = world.resource::<WorldMap>();
    let mut best: Option<(f32, IVec2)> = None;
    for k in 0..16 {
        let a = k as f32 * std::f32::consts::TAU / 16.0;
        let p = from
            + (bevy_math::Vec2::new(a.cos(), a.sin()) * 48.0)
                .round()
                .as_ivec2();
        if !map.in_bounds(p) || !matches!(map.terrain(p), Terrain::Grass | Terrain::Forest) {
            continue;
        }
        if centres
            .iter()
            .any(|c| (c.2 - p).as_vec2().length() <= keep_off)
        {
            continue;
        }
        let food = wild.available(p, radius as f32);
        if best.is_none_or(|(b, _)| food > b) {
            best = Some((food, p));
        }
    }
    best.map(|(_, p)| p)
}

/// GP-22, SIM-38 (M4_PLAN scenario `wanderers`): a band of ten with two
/// days of food that forages and answers a "bring food" request is fed and
/// growing at day 5.
#[test]
fn a_band_that_forages_and_answers_a_request_is_fed_and_grows() {
    let (mut app, champion) = start();
    // No director's bands: they are not this scenario's business, and
    // since M4a step 2 (the smithy chain) the island's towns are fed sooner
    // and ask for food less, so the director fills the quiet with bands
    // earlier (ticks 2100 and 3500 on this seed, not 3000), and one of
    // them fell on the band and killed a man.
    quiet_director(&mut app);
    let home = app.world().get::<Position>(champion).unwrap().0;
    let (hamlet, hamlet_id, centre) = nearest_hamlet(&mut app, home);
    let people = app
        .world()
        .get::<SettlementEconomy>(hamlet)
        .unwrap()
        .population;
    spoil_food(&mut app, hamlet);
    let forage = app.world().resource::<Progression>().data.forage;
    let radius = forage.default_radius;
    let wild0 = app
        .world()
        .resource::<WildFood>()
        .available(home, radius as f32);
    println!(
        "hamlet #{hamlet_id} ({people} people, {:.0} tiles away) has no food; \
         {wild0:.1} wild food within {radius} tiles of the start",
        (centre - home).as_vec2().length()
    );

    // Tick 0: a passive posture (the band never takes a village's food)
    // and forage where it stands.
    let cid = spawn_id(&app, champion);
    send(
        &mut app,
        CommandKind::SetPosture {
            captain: cid,
            posture: Posture::Passive,
        },
    );
    give_order(&mut app, champion, OrderSpec::Forage { radius }, false);
    let tph = ticks_per_hour(&app);
    app.step(FORAGE_FIRST_HOURS * tph + 1);
    let request = app
        .world()
        .resource::<RequestBoard>()
        .of(hamlet_id)
        .cloned()
        .expect("the hungry hamlet asks for help");
    let RequestKind::BringFood { need, .. } = request.kind else {
        panic!("bring food, not {:?}", request.kind);
    };
    let foraged = app
        .world()
        .resource::<ArmyStats>()
        .get(FactionId::PLAYER)
        .foraged;
    println!(
        "tick {}: foraged {foraged:.2}; request #{} posted at {} for {need:.2} food, \
         promising {:?}",
        tick(&app),
        request.id,
        request.posted,
        request.reward
    );
    assert!(foraged > 0.0, "the band foraged nothing at its start");

    // Answer it: walk over and give.
    give_order(
        &mut app,
        champion,
        OrderSpec::Give {
            settlement: hamlet_id,
        },
        false,
    );
    let men = followers(&mut app, champion).len();
    let mut done_at = None;
    while tick(&app) < request.deadline {
        app.step(tph);
        let board = app.world().resource::<RequestBoard>();
        if board.of(hamlet_id).is_none() {
            done_at = Some(tick(&app));
            break;
        }
    }
    let stats = app.world().resource::<ArmyStats>().get(FactionId::PLAYER);
    let done = chronicle(&app)
        .into_iter()
        .find(|e| e.event.kind == SimEventKind::RequestDone && e.event.subject == request.id);
    let joined = followers(&mut app, champion).len() - men;
    println!(
        "gave {:.2} food; request done at {:?} (seen {done_at:?}); {joined} volunteers joined",
        stats.donated,
        done.as_ref().map(|e| e.tick)
    );
    let done = done.expect("the request was answered");
    assert_eq!(done.event.cause, Cause::Request { id: request.id });
    assert_eq!(done.event.other, hamlet_id);
    assert!(stats.donated >= need, "gave {} of {need}", stats.donated);

    // Then live off the land, away from the villages, until day 5: forage
    // where the band stands, and move on when the ground is bare.
    let day5 = DAY * ticks_per_day(&app);
    let mut moves = 0;
    while tick(&app) < day5 {
        let idle = app
            .world()
            .get::<Captain>(champion)
            .is_some_and(|c| c.orders.is_empty());
        if idle {
            let at = app.world().get::<Position>(champion).unwrap().0;
            let here = app
                .world()
                .resource::<WildFood>()
                .available(at, radius as f32);
            let keep_off = app.world().resource::<Army>().data.upkeep.forage_radius;
            let near_village = centres(&mut app)
                .iter()
                .any(|c| (c.2 - at).as_vec2().length() <= keep_off);
            let enough = 4.0 * forage.per_member * band_size(&mut app, champion) as f32;
            if (here < enough || near_village)
                && let Some(p) = fresh_ground(&mut app, at, radius)
            {
                give_order(
                    &mut app,
                    champion,
                    OrderSpec::MoveTo { x: p.x, y: p.y },
                    false,
                );
                give_order(&mut app, champion, OrderSpec::Forage { radius }, true);
                moves += 1;
            } else {
                give_order(&mut app, champion, OrderSpec::Forage { radius }, false);
            }
        }
        app.step(tph.min(day5 - tick(&app)));
    }

    let stats = app.world().resource::<ArmyStats>().get(FactionId::PLAYER);
    let band = followers(&mut app, champion).len();
    let days = baggage_days(&mut app, champion);
    let hunger = worst_hunger(&mut app, champion);
    let hungry = app.world().resource::<Army>().data.upkeep.hungry;
    println!(
        "day {DAY} (tick {}): {band} followers, {days:.2} days of food in the baggage, \
         worst hunger {hunger:.2} (unfed from {hungry}); foraged {:.1} in all, {moves} moves; \
         deserted {}, starvation from {:?}",
        tick(&app),
        stats.foraged,
        stats.deserted,
        stats.first_starvation
    );
    assert!(band > 10, "the band didn't grow: {band} followers");
    assert!(days > 0.0, "the baggage is empty at day {DAY}");
    assert!(hunger < hungry, "someone is unfed at day {DAY}: {hunger}");
    assert_eq!(stats.deserted, 0, "nobody left");
    assert_eq!(stats.first_starvation, None, "nobody starved");
}

/// GP-22, SIM-38 (M4_PLAN scenario `wanderers`), G4: the same band,
/// ordered to stand on bare rock far from any village and not foraging,
/// starts deserting within `first_desertion_within_days` of its food
/// running out, and its starved members are removed.
#[test]
fn a_band_on_bare_rock_deserts_then_starves() {
    let (mut app, champion) = start();
    let home = app.world().get::<Position>(champion).unwrap().0;
    {
        let map = &mut *app.world_mut().resource_mut::<WorldMap>();
        for dy in -ROCK_RADIUS..=ROCK_RADIUS {
            for dx in -ROCK_RADIUS..=ROCK_RADIUS {
                let p = home + IVec2::new(dx, dy);
                if dx * dx + dy * dy <= ROCK_RADIUS * ROCK_RADIUS
                    && map.in_bounds(p)
                    && map.terrain(p) != Terrain::Water
                {
                    map.set_terrain(p, Terrain::Mountain);
                }
            }
        }
    }
    let keep_off = app.world().resource::<Army>().data.upkeep.forage_radius;
    let nearest = centres(&mut app)
        .iter()
        .map(|c| (c.2 - home).as_vec2().length())
        .fold(f32::MAX, f32::min);
    println!(
        "rock within {ROCK_RADIUS} tiles of {home:?}; the nearest settlement is {nearest:.0} tiles away"
    );
    assert!(
        nearest > keep_off + ROCK_RADIUS as f32,
        "too close to a village"
    );
    give_order(
        &mut app,
        champion,
        OrderSpec::MoveTo {
            x: home.x,
            y: home.y,
        },
        false,
    );
    let mut band: Vec<Entity> = vec![champion];
    {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &Follower)>();
        band.extend(
            q.iter(world)
                .filter(|(_, f)| f.captain == champion)
                .map(|(e, _)| e),
        );
    }

    // Hourly, until the band is gone or 8 days have passed: when the food
    // ran out, and when the first desertion and starvation came.
    let tph = ticks_per_hour(&app);
    let tpd = ticks_per_day(&app);
    let deadline = app
        .world()
        .resource::<Army>()
        .data
        .upkeep
        .first_desertion_within_days;
    let mut empty_at = None;
    let mut on_rock = false;
    loop {
        app.step(tph);
        let world = app.world();
        let alive = band
            .iter()
            .filter(|e| world.get_entity(**e).is_ok())
            .count();
        if world.get_entity(champion).is_ok() {
            let at = world.get::<Position>(champion).unwrap().0;
            on_rock |= world.resource::<WorldMap>().terrain(at) == Terrain::Mountain;
            if empty_at.is_none() && baggage_days(&mut app, champion) <= 0.0 {
                empty_at = Some(tick(&app));
            }
        }
        let stats = app.world().resource::<ArmyStats>().get(FactionId::PLAYER);
        let starved_and_gone = stats.first_starvation.is_some() && stats.lost > 0;
        if (starved_and_gone && alive < band.len()) || tick(&app) >= 8 * tpd {
            break;
        }
    }
    let stats = app.world().resource::<ArmyStats>().get(FactionId::PLAYER);
    let world = app.world_mut();
    let gone = band
        .iter()
        .filter(|e| world.get_entity(**e).is_err())
        .count();
    let mut q = world.query_filtered::<&CombatStats, With<Enlisted>>();
    let dead_left = q.iter(world).filter(|cs| !cs.alive()).count();
    println!(
        "food ran out by tick {empty_at:?}; first desertion {:?} ({} deserted), \
         starvation from {:?}, {} starved to death; {gone} of {} gone at tick {}, \
         {dead_left} dead still enlisted",
        stats.first_desertion,
        stats.deserted,
        stats.first_starvation,
        stats.lost,
        band.len(),
        tick(&app)
    );
    assert!(on_rock, "the champion never stood on the rock");
    let empty_at = empty_at.expect("the baggage never ran out");
    let first = stats.first_desertion.expect("nobody deserted");
    assert!(
        first + tph >= empty_at && first <= empty_at + (deadline * tpd as f32) as u64,
        "first desertion at {first}; the food ran out by {empty_at}, {deadline} days allowed"
    );
    assert!(stats.foraged == 0.0, "the band foraged");
    assert!(stats.lost > 0, "nobody starved to death");
    assert_eq!(dead_left, 0, "a starved soldier stayed in the world (G4)");
    assert_eq!(
        gone,
        stats.deserted as usize + stats.lost as usize,
        "everyone gone either deserted or died and was removed"
    );
}
