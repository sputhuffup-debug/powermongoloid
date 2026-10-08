//! M3 orders (GP-01, GP-02): commands are validated, and each order does
//! what it says. Uses the playtest start (`--skirmish`).

mod common;

use bevy_ecs::prelude::*;
use common::*;
use sim_agents::{Enlisted, Owner, Villager};
use sim_core::{
    Allegiance, CommandKind, FactionId, OrderSpec, Posture, SimEventKind, SimStepper, SpawnId,
};
use sim_economy::{Economy, GoodId, SettlementEconomy, Stockpile};
use sim_headless::{HeadlessConfig, Scenario, headless_app};
use sim_military::{ArmyStats, Captain, Equipment, Factions, Follower, Skirmish, setup_skirmish};

fn skirmish() -> (App, Skirmish) {
    let mut app = headless_app(HeadlessConfig::new(42));
    let s = setup_skirmish(app.world_mut()).expect("skirmish");
    (app, s)
}

fn captain(app: &App, e: Entity) -> Captain {
    app.world().get::<Captain>(e).expect("captain").clone()
}

fn nearest_village(app: &mut App, to: Entity) -> (Entity, u64) {
    let p = pos_of(app, to);
    let v = villages(app);
    let x = v
        .iter()
        .min_by(|a, b| {
            a.2.as_vec2()
                .distance(p)
                .total_cmp(&b.2.as_vec2().distance(p))
        })
        .unwrap();
    (x.0, x.1)
}

fn good(app: &App, name: &str) -> GoodId {
    let econ = app.world().resource::<Economy>();
    GoodId(econ.data.goods.iter().position(|g| g.name == name).unwrap() as u8)
}

#[test]
fn invalid_commands_are_refused() {
    let (mut app, s) = skirmish();
    let raider_orders = captain(&app, s.raiders).orders.len();
    // Someone else's captain.
    order(&mut app, 1, s.raiders, OrderSpec::Return);
    // An unknown settlement, and a tile off the map.
    order(
        &mut app,
        1,
        s.player,
        OrderSpec::Attack {
            settlement: 9_999_999,
        },
    );
    order(&mut app, 1, s.player, OrderSpec::MoveTo { x: -1, y: 5 });
    // Not a follower.
    command(&mut app, 1, CommandKind::AppointCaptain { follower: 0 });
    app.step(1);
    let stats = app.world().resource::<ArmyStats>().get(FactionId(1));
    assert_eq!(stats.rejected_commands, 4);
    assert_eq!(captain(&app, s.raiders).orders.len(), raider_orders);
    assert!(captain(&app, s.player).orders.is_empty());
}

#[test]
fn posture_and_cancel_apply_to_own_captain() {
    let (mut app, s) = skirmish();
    let id = spawn_id(&app, s.player);
    command(
        &mut app,
        1,
        CommandKind::SetPosture {
            captain: id,
            posture: Posture::Passive,
        },
    );
    order(&mut app, 1, s.player, OrderSpec::MoveTo { x: 10, y: 10 });
    app.step(1);
    let c = captain(&app, s.player);
    assert_eq!(c.posture, Posture::Passive);
    assert_eq!(c.orders.len(), 1);
    command(&mut app, 1, CommandKind::CancelOrders { captain: id });
    app.step(1);
    assert!(captain(&app, s.player).orders.is_empty());
}

#[test]
fn appointing_a_captain_splits_the_army() {
    let (mut app, s) = skirmish();
    let before = followers(&mut app, s.player).len();
    let follower = {
        let world = app.world_mut();
        let mut q = world.query::<(&SpawnId, &Follower)>();
        q.iter(world)
            .filter(|(_, f)| f.captain == s.player)
            .map(|(id, _)| id.0)
            .min()
            .unwrap()
    };
    command(&mut app, 1, CommandKind::AppointCaptain { follower });
    app.step(1);
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &SpawnId, &Captain)>();
    let new = q
        .iter(world)
        .find(|(_, id, _)| id.0 == follower)
        .map(|(e, ..)| e)
        .expect("promoted");
    let (a, b) = (
        followers(&mut app, s.player).len(),
        followers(&mut app, new).len(),
    );
    assert_eq!(a + b + 1, before);
    assert!(a.abs_diff(b) <= 1, "split {a}/{b}");
    let econ = app.world().resource::<Economy>().clone();
    let food = |e| app.world().get::<Stockpile>(e).unwrap().food_value(&econ);
    assert!(food(new) > 0.0 && (food(new) - food(s.player)).abs() <= food(s.player) * 0.1);
}

#[test]
fn recruiting_turns_villagers_into_followers() {
    let (mut app, s) = skirmish();
    let (village, vid) = nearest_village(&mut app, s.player);
    let pop = app
        .world()
        .get::<SettlementEconomy>(village)
        .unwrap()
        .population;
    let before = followers(&mut app, s.player).len();
    order(
        &mut app,
        1,
        s.player,
        OrderSpec::Recruit {
            settlement: vid,
            count: 20,
        },
    );
    for _ in 0..30 {
        app.step(100);
        if captain(&app, s.player).orders.is_empty() {
            break;
        }
    }
    assert!(
        captain(&app, s.player).orders.is_empty(),
        "recruit order unfinished"
    );
    let recruited = app
        .world()
        .resource::<ArmyStats>()
        .get(FactionId(1))
        .recruited as usize;
    assert!((1..=20).contains(&recruited), "recruited {recruited}");
    let world = app.world_mut();
    let mut q = world.query_filtered::<(&Villager, &Follower), With<Enlisted>>();
    let joined = q
        .iter(world)
        .filter(|(v, f)| v.settlement == village && f.captain == s.player)
        .count();
    assert_eq!(joined, recruited, "recruits keep their villager identity");
    assert_eq!(
        app.world()
            .get::<SettlementEconomy>(village)
            .unwrap()
            .population,
        pop - recruited as u32
    );
    assert!(followers(&mut app, s.player).len() >= before + recruited - 2);
}

#[test]
fn equip_trade_and_scout() {
    let (mut app, s) = skirmish();
    let (village, vid) = nearest_village(&mut app, s.player);
    let store = app
        .world()
        .get::<SettlementEconomy>(village)
        .unwrap()
        .storehouse;
    let (weapons, wood) = (good(&app, "weapons"), good(&app, "wood"));
    // A good harvest: food to sell.
    let grain = good(&app, "grain");
    app.world_mut()
        .get_mut::<Stockpile>(store)
        .unwrap()
        .add(grain, 5_000);
    app.world_mut()
        .get_mut::<Stockpile>(store)
        .unwrap()
        .add(weapons, 30);
    app.world_mut()
        .get_mut::<Stockpile>(s.player)
        .unwrap()
        .add(wood, 200);
    let id = spawn_id(&app, s.player);
    command(
        &mut app,
        1,
        CommandKind::SetPosture {
            captain: id,
            posture: Posture::Neutral,
        },
    );
    let start = app.world().get::<sim_core::Position>(s.player).unwrap().0;
    order(&mut app, 1, s.player, OrderSpec::Equip { settlement: vid });
    for spec in [
        OrderSpec::Trade { settlement: vid },
        OrderSpec::Scout {
            x: start.x,
            y: start.y,
        },
        OrderSpec::Return,
    ] {
        command(
            &mut app,
            1,
            CommandKind::IssueOrder {
                captain: id,
                order: spec,
                queue: true,
            },
        );
    }
    for _ in 0..60 {
        app.step(100);
        if captain(&app, s.player).orders.is_empty() {
            break;
        }
    }
    assert!(
        captain(&app, s.player).orders.is_empty(),
        "orders unfinished: {:?}",
        captain(&app, s.player).orders
    );
    // Neutral asks for half the arms.
    let world = app.world_mut();
    let mut q = world.query::<(&Follower, &Equipment)>();
    let armed = q
        .iter(world)
        .filter(|(f, e)| f.captain == s.player && e.weapon)
        .count();
    assert_eq!(armed, 15, "armed {armed}");
    let stats = app.world().resource::<ArmyStats>().get(FactionId(1));
    assert!(stats.bought > 0.0, "traded wood for food");
    assert!(app.world().get::<Stockpile>(store).unwrap().get(wood) > 0);
    // Back at the seat.
    let seat = app
        .world()
        .resource::<Factions>()
        .seat(FactionId(1))
        .unwrap();
    let here = pos_of(&app, s.player);
    assert!(
        here.distance(seat.as_vec2()) < 5.0,
        "not home: {here} vs {seat}"
    );
}

#[test]
fn skirmish_scenario_is_reachable_from_config() {
    let mut app = headless_app(HeadlessConfig {
        scenario: Scenario::Skirmish,
        ..HeadlessConfig::new(42)
    });
    let world = app.world_mut();
    let mut q = world.query::<&Captain>();
    let mut factions: Vec<u16> = q.iter(world).map(|c| c.faction.0).collect();
    factions.sort_unstable();
    assert_eq!(factions, [1, 2]);
}

/// GP-02, SIM-13, GP-05: an Attack raises the militia, breaks it, holds
/// the centre and takes the village, whose residents then follow the
/// warband.
#[test]
fn attacking_a_village_raises_its_militia() {
    let (mut app, s) = skirmish();
    let (village, vid) = nearest_village(&mut app, s.player);
    order(&mut app, 1, s.player, OrderSpec::Attack { settlement: vid });
    let mut fought = 0;
    for _ in 0..40 {
        app.step(100);
        let world = app.world_mut();
        let mut q = world.query::<(&Villager, &sim_agents::CombatStats)>();
        fought = fought.max(
            q.iter(world)
                .filter(|(_, c)| matches!(c.stance, sim_agents::Stance::Militia { .. }))
                .count(),
        );
        if captain(&app, s.player).orders.is_empty() {
            break;
        }
    }
    let stats = app.world().resource::<ArmyStats>().get(FactionId(1));
    println!("militia up to {fought}; {stats:?}");
    assert_eq!(stats.provocations, 1);
    assert!(fought > 10, "only {fought} took up arms");
    assert!(stats.kills > 0, "no blows landed");
    assert!(
        captain(&app, s.player).orders.is_empty(),
        "attack never ended"
    );
    // GP-05: the hold took it.
    assert_eq!(stats.conquests, 1, "the attack ended without a conquest");
    let world = app.world_mut();
    assert_eq!(world.get::<Owner>(village).unwrap().faction, FactionId(1));
    let mut q = world.query_filtered::<(&Villager, &Allegiance), Without<Enlisted>>();
    let (sworn, all) = q
        .iter(world)
        .filter(|(v, _)| v.settlement == village)
        .fold((0, 0), |(n, t), (_, a)| {
            (n + (a.0 == FactionId(1)) as u32, t + 1)
        });
    assert!(
        all > 0 && sworn == all,
        "{sworn} of {all} follow the warband"
    );
    let conquest = chronicle(&app)
        .into_iter()
        .find(|e| e.event.kind == SimEventKind::Conquest)
        .expect("a Conquest in the chronicle");
    assert_eq!(
        (conquest.event.faction, conquest.event.subject),
        (FactionId(1), vid)
    );
}
