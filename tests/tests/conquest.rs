//! M4 step 2 scenario `conquest` (GP-05, GP-06, SIM-24; ADR 0014): on the
//! run island, the champion's band attacks the nearest hamlet, breaks its
//! militia and holds the centre, and the hamlet changes owner. The run
//! then branches from that shared, deterministic prefix ([`conquered`]):
//!
//! - a passive garrison in a fed village: mean loyalty rises every day;
//! - a raiding band plunders the undefended village: its food runs out
//!   and loyalty falls below where it started;
//! - a village left without food or a garrison revolts within two to
//!   three days, and the hamlet is independent again;
//! - that starved branch played single- and multi-threaded gives the same
//!   state hash every 100 ticks (NFR-08), so the owned-settlement steps
//!   (loyalty drift and aggregation, the revolt) don't depend on the
//!   executor.
//!
//! The prefix is played once and saved; each branch loads that save into
//! its own app (GP-12). A branch's scenario edit (a raiding band arriving,
//! the stores spoiling) happens between ticks right after the load. Each
//! test prints the numbers it checks.

mod common;

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use common::*;
use sim_agents::{
    CombatStats, Enlisted, FoodSecurity, Goodwill, Loyalty, Owner, SettlementLoyalty, Threat,
    Villager,
};
use sim_core::{
    Allegiance, Calendar, Cause, CommandKind, FactionId, Offers, OrderSpec, Position, Posture,
    RunConfig, SimEventKind, SimStepper, SimTick, SpawnId,
};
use sim_economy::{SettlementEconomy, Stockpile};
use sim_headless::{HeadlessConfig, Threading, headless_app};
use sim_military::progression::Effect;
use sim_military::{Army, ArmySpec, ArmyStats, Captain, Progression, RunProgress, spawn_army};
use sim_world::{Settlement, SettlementSize, SizeTier};

/// The run whose start (seed 42: the "Outlaws" band of ten, a hamlet of
/// 56 people 121 tiles away) the scenario plays.
const SEED: u64 = 42;
/// The prefix gives up if the hamlet hasn't fallen by then.
const CONQUEST_WITHIN: u64 = 4_000;

fn tick(app: &App) -> u64 {
    app.world().resource::<SimTick>().0
}

fn ticks_per_day(app: &App) -> u64 {
    app.world().resource::<Calendar>().ticks_per_day
}

fn center(app: &App, s: Entity) -> IVec2 {
    app.world().get::<Settlement>(s).expect("settlement").center
}

/// The nearest hamlet to `from` (ties: lowest SpawnId), never the rival's
/// seat: (entity, SpawnId).
fn nearest_hamlet(app: &mut App, from: IVec2) -> (Entity, u64) {
    let seat = app.world().resource::<RunProgress>().rival_seat;
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &SpawnId, &Settlement, &SettlementSize)>();
    q.iter(world)
        .filter(|(_, id, _, size)| size.tier == SizeTier::Hamlet && Some(id.0) != seat)
        .map(|(e, id, s, _)| ((s.center - from).as_vec2().length(), id.0, e))
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, id, e)| (e, id))
        .expect("a hamlet on the run island")
}

/// The allegiance of every resident of `s` who isn't enlisted.
fn allegiances(app: &mut App, s: Entity) -> Vec<FactionId> {
    let world = app.world_mut();
    let mut q = world.query_filtered::<(&Villager, &Allegiance), Without<Enlisted>>();
    q.iter(world)
        .filter(|(v, _)| v.settlement == s)
        .map(|(_, a)| a.0)
        .collect()
}

/// `faction`'s living soldiers within `garrison.radius` of `s`'s centre.
fn garrison(app: &mut App, s: Entity, faction: FactionId) -> u32 {
    let at = center(app, s).as_vec2();
    let radius = app.world().resource::<Army>().data.garrison.radius;
    let world = app.world_mut();
    let mut q = world.query_filtered::<(&Allegiance, &Position, &CombatStats), With<Enlisted>>();
    q.iter(world)
        .filter(|(a, p, cs)| a.0 == faction && cs.alive() && p.0.as_vec2().distance(at) <= radius)
        .count() as u32
}

/// One day tick's reading of the conquered settlement.
#[derive(Debug, Clone, Copy)]
struct Day {
    tick: u64,
    owner: FactionId,
    mean: f32,
    food_days: f32,
    below_since: Option<u64>,
    guards: u32,
    /// `Threat.level`: how frightened the residents are.
    threat: f32,
}

/// Steps to just after each of the next `n` day ticks (when loyalty
/// drifts and revolts are settled) and reads `s` there.
fn days(app: &mut App, s: Entity, n: u32) -> Vec<Day> {
    let tpd = ticks_per_day(app);
    let mut out = Vec::new();
    for _ in 0..n {
        let next = (tick(app) / tpd + 1) * tpd + 1;
        app.step(next - tick(app));
        let owner = app.world().get::<Owner>(s).expect("owner").faction;
        let sl = *app.world().get::<SettlementLoyalty>(s).expect("loyalty");
        let food_days = app.world().get::<FoodSecurity>(s).expect("food").days;
        let guards = garrison(app, s, FactionId::PLAYER);
        let threat = app.world().get::<Threat>(s).expect("threat").level;
        let day = Day {
            tick: next - 1,
            owner,
            mean: sl.mean,
            food_days,
            below_since: sl.below_since,
            guards,
            threat,
        };
        println!(
            "  day tick {}: owner {}, loyalty mean {:.3} min {:.3} ({} residents), \
             food {:.2} days, below since {:?}, garrison {}, threat {:.2}",
            day.tick,
            day.owner.0,
            day.mean,
            sl.min,
            sl.residents,
            day.food_days,
            day.below_since,
            day.guards,
            day.threat
        );
        out.push(day);
    }
    out
}

/// The shared prefix (GP-05, GP-27): the run of `seed`, the start boon
/// that adds food to the band's baggage (so it can hold a garrison for
/// days), then an Attack on the nearest hamlet, polled every 100 ticks
/// until the player owns it. Asserts that the band broke the militia and
/// then held the centre for `conquest.hold_hours` before the hamlet
/// changed hands, the Conquest in the chronicle, and that every resident
/// now follows the player. Returns the app and the hamlet.
fn conquered(seed: u64) -> (App, Entity) {
    static PREFIX: std::sync::OnceLock<(Vec<u8>, u64)> = std::sync::OnceLock::new();
    assert_eq!(seed, SEED, "the prefix is saved for SEED");
    let (bytes, id) = PREFIX.get_or_init(|| {
        let (mut app, hamlet) = conquered_with(seed, Threading::Multi);
        let id = spawn_id(&app, hamlet);
        (sim_headless::save_app(&mut app), id)
    });
    // GP-12: every branch starts from the one save.
    let mut app = sim_headless::load_app(bytes, Threading::Multi).expect("the prefix loads");
    let world = app.world_mut();
    let hamlet = world
        .query::<(Entity, &SpawnId)>()
        .iter(world)
        .find(|(_, s)| s.0 == *id)
        .map(|(e, _)| e)
        .expect("the hamlet after the load");
    (app, hamlet)
}

/// [`conquered`], on the executor `threading` picks.
fn conquered_with(seed: u64, threading: Threading) -> (App, Entity) {
    let mut app = headless_app(HeadlessConfig {
        threading,
        ..HeadlessConfig::run(seed, RunConfig::default())
    });
    // The loyalty branches meet no director's raiders (GP-16).
    quiet_director(&mut app);
    let champion = champion(&mut app);
    let offer = start_offer(&app);
    let data = app.world().resource::<Progression>().data.clone();
    let cards = app
        .world()
        .resource::<Offers>()
        .get(offer)
        .unwrap()
        .choices
        .clone();
    let food = cards.iter().position(|&c| {
        data.boons[c as usize]
            .effects
            .iter()
            .any(|e| matches!(e, Effect::FoodDays(_)))
    });
    let food = food.expect("the seed offers a boon with food for the garrison");
    choose(&mut app, offer, food as u8);

    let start = app.world().get::<Position>(champion).unwrap().0;
    let (hamlet, id) = nearest_hamlet(&mut app, start);
    let people = app.world().get::<SettlementSize>(hamlet).unwrap().people;
    let band = followers(&mut app, champion).len();
    println!(
        "seed {seed}: a band of {band} attacks hamlet #{id} ({people} people, {:.0} tiles away)",
        (center(&app, hamlet) - start).as_vec2().length()
    );
    order(&mut app, 1, champion, OrderSpec::Attack { settlement: id });
    // The tick the defenders broke (the Attack's step 2 starts there). The
    // hold lasts longer than a poll, so a poll sees it.
    let mut broke = None;
    while owner_of(&app, hamlet) != FactionId::PLAYER {
        assert!(
            tick(&app) < CONQUEST_WITHIN,
            "seed {seed}: hamlet #{id} not taken by tick {CONQUEST_WITHIN}"
        );
        app.step(100);
        let front = app
            .world()
            .get::<Captain>(champion)
            .unwrap()
            .orders
            .front()
            .copied();
        if let Some(o) = front.filter(|o| o.step == 2) {
            broke.get_or_insert(o.since);
        }
    }
    let stats = app.world().resource::<ArmyStats>().get(FactionId::PLAYER);
    let since = app.world().get::<Owner>(hamlet).unwrap().since;
    let conquest = chronicle(&app)
        .into_iter()
        .find(|e| e.event.kind == SimEventKind::Conquest)
        .expect("a Conquest in the chronicle");
    let sworn = allegiances(&mut app, hamlet);
    println!(
        "  taken at tick {} (seen at {}): {} residents handed over; conquests {}, \
         kills {}, rejected {}",
        conquest.tick,
        tick(&app),
        conquest.event.amount,
        stats.conquests,
        stats.kills,
        stats.rejected_commands
    );
    assert_eq!(stats.rejected_commands, 0, "every command applied");
    assert_eq!(stats.conquests, 1);
    // GP-05: break the militia, then hold the centre.
    assert!(stats.kills > 0, "no blow struck: the militia never fought");
    let broke = broke.expect("the hamlet changed hands without a hold");
    let hold = {
        let w = app.world();
        let hours = w.resource::<Army>().data.conquest.hold_hours;
        w.resource::<Calendar>().hours(hours)
    };
    println!("  defenders broke at tick {broke}; hold of {hold} ticks");
    assert!(
        conquest.tick >= broke + hold,
        "taken at {} only {} ticks after the break at {broke}",
        conquest.tick,
        conquest.tick - broke
    );
    let ev = conquest.event;
    let captain = spawn_id(&app, champion);
    assert_eq!(
        (ev.faction, ev.subject, ev.other, ev.cause),
        (
            FactionId::PLAYER,
            id,
            FactionId::INDEPENDENT.0 as u64,
            Cause::Conquest { captain }
        ),
        "the Conquest names the taker, the hamlet, its previous owner and the cause"
    );
    assert_eq!(ev.at, center(&app, hamlet));
    assert_eq!(since, conquest.tick, "owned since the conquest");
    assert!(!sworn.is_empty());
    assert_eq!(
        ev.amount as usize,
        sworn.len(),
        "every resident handed over"
    );
    assert!(
        sworn.iter().all(|a| *a == FactionId::PLAYER),
        "residents follow the warband: {sworn:?}"
    );
    (app, hamlet)
}

/// The conquest tick and the residents' starting loyalty: everyone is
/// seeded from the hamlet's goodwill toward the player, (g + 1) / 2 (0.5
/// while nothing moves goodwill), and no day tick has drifted it yet (the
/// prefix sees the conquest within 100 ticks).
fn conquest_tick_and_seed(app: &mut App, s: Entity) -> (u64, f32) {
    let since = app.world().get::<Owner>(s).unwrap().since;
    let goodwill = app
        .world()
        .get::<Goodwill>(s)
        .unwrap()
        .get(FactionId::PLAYER);
    let seed = (goodwill + 1.0) / 2.0;
    let world = app.world_mut();
    let mut q = world.query_filtered::<(&Villager, &Loyalty), Without<Enlisted>>();
    let seeds: Vec<f32> = q
        .iter(world)
        .filter(|(v, _)| v.settlement == s)
        .map(|(_, l)| l.0)
        .collect();
    assert!(
        seeds.iter().all(|l| *l == seed),
        "seeded at {seed}: {seeds:?}"
    );
    (since, seed)
}

/// GP-05, GP-06, SIM-24: a passive garrison in a fed village. The garrison
/// stands (fed from its own baggage: a hamlet has no food to spare), and
/// mean loyalty rises at every day tick, pulled up by food security and
/// time under the owner.
#[test]
fn a_fed_village_under_a_passive_garrison_grows_loyal() {
    let (mut app, s) = conquered(SEED);
    let (_, seed) = conquest_tick_and_seed(&mut app, s);
    let champion = champion(&mut app);
    let (id, cid) = (spawn_id(&app, s), spawn_id(&app, champion));
    let passive = CommandKind::SetPosture {
        captain: cid,
        posture: Posture::Passive,
    };
    command(&mut app, 1, passive);
    order(
        &mut app,
        1,
        champion,
        OrderSpec::Garrison { settlement: id },
    );
    println!("a passive garrison; loyalty seeded at {seed}:");
    let log = days(&mut app, s, 3);
    let means: Vec<f32> = log.iter().map(|d| d.mean).collect();
    let refused = rejected(&app, FactionId::PLAYER);
    let front = app
        .world()
        .get::<Captain>(champion)
        .unwrap()
        .orders
        .front()
        .map(|o| o.spec);
    println!("  means {means:?}; rejected {refused}; front order {front:?}");
    assert_eq!(refused, 0);
    assert_eq!(
        front,
        Some(OrderSpec::Garrison { settlement: id }),
        "the garrison stays while the hamlet is ours"
    );
    let fed = app
        .world()
        .resource::<sim_agents::Villagers>()
        .data
        .loyalty
        .food_days;
    for d in &log {
        assert_eq!(d.owner, FactionId::PLAYER);
        assert!(d.guards > 0, "no garrison at tick {}", d.tick);
        assert_eq!(d.below_since, None, "unrest at tick {}", d.tick);
        // GP-05: the garrison doesn't frighten its own village (or it would
        // hide instead of working), and the village is fed.
        assert_eq!(d.threat, 0.0, "the garrison frightens the village");
        assert!(
            d.food_days >= fed,
            "not a fed village: {:.2} days at tick {}",
            d.food_days,
            d.tick
        );
    }
    let mut last = seed;
    for d in &log {
        assert!(
            d.mean > last,
            "loyalty fell to {} at tick {}",
            d.mean,
            d.tick
        );
        last = d.mean;
    }
    assert!(last > seed + 0.1, "loyalty only rose to {last}");
}

/// GP-06, SIM-24: a raiding band plunders the undefended village and moves
/// on. The raid provokes the militia and costs the village its reserve and
/// half a day's work; it can't feed itself back, so mean loyalty falls day
/// on day and ends the three days below its seed.
#[test]
fn a_plundered_village_loses_faith() {
    let (mut app, s) = conquered(SEED);
    let (_, seed) = conquest_tick_and_seed(&mut app, s);
    let champion = champion(&mut app);
    let id = spawn_id(&app, s);
    let at = center(&app, s);
    // The warband rides back to its start, leaving the hamlet unguarded...
    let home = app.world().resource::<RunProgress>().start;
    order(
        &mut app,
        1,
        champion,
        OrderSpec::MoveTo {
            x: home.x,
            y: home.y,
        },
    );
    // ...and a raiding band from the other side plunders it, then moves on
    // out of sight (twice the threat radius away).
    let away = (at - home).as_vec2().normalize_or_zero();
    let radius = app.world().resource::<Army>().data.threat.radius;
    let from = passable_near(&app, at + (away * 25.0).round().as_ivec2()).unwrap();
    let to = passable_near(&app, at + (away * radius * 2.0).round().as_ivec2()).unwrap();
    let raiders = spawn_army(
        app.world_mut(),
        &ArmySpec {
            posture: Posture::Aggressive,
            armed: true,
            food_days: 3.0,
            ..ArmySpec::new(FactionId::RAIDERS, from, 40)
        },
    );
    let rid = spawn_id(&app, raiders);
    order(
        &mut app,
        2,
        raiders,
        OrderSpec::GatherFood { settlement: id },
    );
    let leave = CommandKind::IssueOrder {
        captain: rid,
        order: OrderSpec::MoveTo { x: to.x, y: to.y },
        queue: true,
    };
    command(&mut app, 2, leave);
    println!("raiders come from {from} and leave for {to}; loyalty seeded at {seed}:");
    let log = days(&mut app, s, 3);
    let means: Vec<f32> = log.iter().map(|d| d.mean).collect();
    let raid = app.world().resource::<ArmyStats>().get(FactionId::RAIDERS);
    println!(
        "  means {means:?}; raiders seized {:.1} food, provoked {}, killed {} villagers, \
         rejected {}",
        raid.seized, raid.provocations, raid.villagers_killed, raid.rejected_commands
    );
    assert_eq!(raid.rejected_commands, 0);
    assert!(raid.seized > 0.0, "nothing plundered");
    assert!(raid.provocations >= 1, "the raid provoked no one");
    for d in &log {
        assert_eq!(d.owner, FactionId::PLAYER, "revolted by tick {}", d.tick);
        assert_eq!(d.guards, 0, "a garrison at tick {}", d.tick);
    }
    let (first, last) = (log[0].food_days, log[2].food_days);
    assert!(last < first, "food came back: {first:.2} -> {last:.2} days");
    assert!(
        means[2] < means[1] && means[1] < means[0],
        "loyalty didn't fall day on day: {means:?}"
    );
    assert!(
        means[2] < seed,
        "loyalty ended at {} (seed {seed})",
        means[2]
    );
}

/// The starved branch's setup: the warband rides back to its start, the
/// hamlet's stores (storehouse and homes) spoil and its farmers are cut
/// off. Returns how many farmers were cut off.
fn starve(app: &mut App, s: Entity) -> u32 {
    let champion = champion(app);
    let home = app.world().resource::<RunProgress>().start;
    order(
        app,
        1,
        champion,
        OrderSpec::MoveTo {
            x: home.x,
            y: home.y,
        },
    );
    let se = app.world().get::<SettlementEconomy>(s).unwrap().clone();
    for store in std::iter::once(se.storehouse).chain(se.homes.iter().copied()) {
        if let Some(mut stock) = app.world_mut().get_mut::<Stockpile>(store) {
            stock.goods.clear();
        }
    }
    sim_headless::cut_off_farmers(app, s)
}

/// GP-06, SIM-24: a conquest left without food or a garrison revolts. The
/// stores spoil and the farmers are cut off, and the warband rides away.
/// Mean loyalty sinks below `loyalty.revolt_threshold`; a day later the
/// hamlet throws off its owner. Within one to three days of the conquest
/// it is independent again, its residents with it. (Before M4 step 3 it
/// took two to three: since goodwill moves, the attack's provocation and
/// killings lower goodwill toward the conqueror, so loyalty is seeded
/// below the neutral 0.5 and sinks below the threshold at the first day
/// tick.)
#[test]
fn a_starved_conquest_revolts() {
    let (mut app, s) = conquered(SEED);
    let (taken, seed) = conquest_tick_and_seed(&mut app, s);
    let id = spawn_id(&app, s);
    let cut = starve(&mut app, s);
    let rules = app
        .world()
        .resource::<sim_agents::Villagers>()
        .data
        .loyalty
        .clone();
    println!(
        "starved: stores spoiled, {cut} farmers cut off; loyalty seeded at {seed}, \
         unrest below {}:",
        rules.revolt_threshold
    );
    assert!(cut > 0, "no farmers to cut off");
    let tpd = ticks_per_day(&app);
    let log = days(&mut app, s, 3);
    let revolt = chronicle(&app)
        .into_iter()
        .find(|e| e.event.kind == SimEventKind::Revolt)
        .expect("a Revolt in the chronicle");
    let after = revolt.tick - taken;
    println!(
        "  revolt at tick {}, {:.2} days after the conquest at {taken}",
        revolt.tick,
        after as f32 / tpd as f32
    );
    let ev = revolt.event;
    assert_eq!(
        (ev.faction, ev.subject, ev.other, ev.cause),
        (
            FactionId::PLAYER,
            id,
            FactionId::INDEPENDENT.0 as u64,
            Cause::Revolt
        ),
        "the Revolt names the old owner, the hamlet, the new owner and the cause"
    );
    assert!(
        (tpd..=3 * tpd).contains(&after),
        "revolt {after} ticks after the conquest, not within 1-3 days"
    );
    let below = log
        .iter()
        .find_map(|d| d.below_since)
        .expect("loyalty sank below the threshold first");
    assert!(
        below + tpd <= revolt.tick,
        "unrest for a day before the rising"
    );
    assert!(log.iter().all(|d| d.guards == 0), "a garrison stood");
    let owner = *app.world().get::<Owner>(s).unwrap();
    assert_eq!(owner.faction, FactionId::INDEPENDENT);
    assert_eq!(owner.since, revolt.tick);
    let free = allegiances(&mut app, s);
    assert!(!free.is_empty());
    assert!(
        free.iter().all(|a| *a == FactionId::INDEPENDENT),
        "residents still sworn: {free:?}"
    );
    let sl = *app.world().get::<SettlementLoyalty>(s).unwrap();
    assert_eq!(
        sl.mean, rules.independent_target,
        "loyalty restarts as a free village's"
    );
    let conquests = app
        .world()
        .resource::<ArmyStats>()
        .get(FactionId::PLAYER)
        .conquests;
    assert_eq!(conquests, 1, "a revolt is no conquest");
}

/// NFR-08 (GP-05, GP-06, SIM-24): the starved branch, played on the
/// multi-threaded and on the single-threaded executor, gives the same
/// state hash every 100 ticks from the conquest to three days after it.
/// Over those days the hamlet is owned at day ticks, so loyalty drifts
/// toward a real target (a parallel step), the unrest clock starts and the
/// revolt frees it; none of that may depend on the executor. (Replays
/// can't cover this: they don't redo the branch's store edits.)
#[test]
fn a_conquest_and_its_revolt_are_thread_independent() {
    let play = |threading: Threading| {
        let (mut app, s) = conquered_with(SEED, threading);
        let taken = app.world().get::<Owner>(s).unwrap().since;
        starve(&mut app, s);
        let end = taken + 3 * ticks_per_day(&app) + 1;
        let mut hashes = Vec::new();
        while tick(&app) < end {
            app.step(100);
            hashes.push((tick(&app), sim_core::compute_state_hash(app.world_mut())));
        }
        let revolted = chronicle(&app)
            .iter()
            .any(|e| e.event.kind == SimEventKind::Revolt);
        (hashes, revolted)
    };
    let (multi, revolted) = play(Threading::Multi);
    let (single, _) = play(Threading::Single);
    assert!(revolted, "the starved hamlet never revolted");
    assert_eq!(multi.len(), single.len());
    if let Some(i) = (0..multi.len()).find(|&i| multi[i] != single[i]) {
        panic!(
            "single- and multi-threaded runs diverge by tick {} ({:#018x} vs {:#018x})",
            multi[i].0, multi[i].1, single[i].1
        );
    }
    println!(
        "{} checkpoints agree; final {:#018x} at tick {}",
        multi.len(),
        multi.last().unwrap().1,
        multi.last().unwrap().0
    );
}
