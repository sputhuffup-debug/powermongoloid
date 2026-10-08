//! Temporary probe (not committed): traces an Attack in skirmish.

mod common;

use bevy_ecs::prelude::*;
use common::*;
use sim_agents::{Owner, SettlementLoyalty, Villager};
use sim_core::{FactionId, OrderSpec, SimStepper};
use sim_headless::{HeadlessConfig, headless_app};
use sim_military::{ArmyStats, Captain, MilitaryLog, setup_skirmish};

#[test]
fn probe_attack() {
    let mut app = headless_app(HeadlessConfig::new(42));
    let s = setup_skirmish(app.world_mut()).expect("skirmish");
    let p = pos_of(&app, s.player);
    let v = villages(&mut app);
    let (ve, vid, vc, _) = *v
        .iter()
        .min_by(|a, b| {
            a.2.as_vec2()
                .distance(p)
                .total_cmp(&b.2.as_vec2().distance(p))
        })
        .unwrap();
    println!("village {vid} at {vc}, player at {p}");
    order(&mut app, 1, s.player, OrderSpec::Attack { settlement: vid });
    let mut seen = 0;
    for i in 0..60 {
        app.step(100);
        let c = app.world().get::<Captain>(s.player).cloned();
        let world = app.world_mut();
        let mut q = world.query::<(&Villager, &sim_agents::CombatStats)>();
        let militia = q
            .iter(world)
            .filter(|(v, c)| v.settlement == ve && matches!(c.stance, sim_agents::Stance::Militia { .. }))
            .count();
        let owner = world.get::<Owner>(ve).copied();
        let sl = world.get::<SettlementLoyalty>(ve).copied();
        let pos = pos_of(&app, s.player);
        let log = app.world().resource::<MilitaryLog>();
        for (t, line) in log.entries.iter().skip(seen.min(log.entries.len())) {
            println!("  log {t}: {line}");
        }
        seen = log.entries.len();
        println!(
            "t={} pos={pos} d={:.1} order={:?} militia={militia} owner={owner:?} sl={sl:?}",
            (i + 1) * 100,
            pos.distance(vc.as_vec2()),
            c.as_ref().and_then(|c| c.orders.front().copied()),
        );
        if c.is_none_or(|c| c.orders.is_empty()) && i > 30 {
            break;
        }
    }
    let stats = app.world().resource::<ArmyStats>().get(FactionId(1));
    println!("{stats:?}");
}

use sim_economy::{Economy, Home, SettlementEconomy, Stockpile};
use sim_military::Follower;

fn conquered(app: &mut App) -> (Entity, Entity, u64, bevy_math::IVec2) {
    let s = setup_skirmish(app.world_mut()).expect("skirmish");
    if std::env::var("NO_RAIDERS").is_ok() {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &Follower)>();
        let fs: Vec<Entity> = q.iter(world).filter(|(_, f)| f.captain == s.raiders).map(|(e, _)| e).collect();
        for e in fs { world.despawn(e); }
        world.despawn(s.raiders);
    }
    let p = pos_of(app, s.player);
    let v = villages(app);
    let (ve, vid, vc, _) = *v
        .iter()
        .min_by(|a, b| a.2.as_vec2().distance(p).total_cmp(&b.2.as_vec2().distance(p)))
        .unwrap();
    order(app, 1, s.player, OrderSpec::Attack { settlement: vid });
    for _ in 0..40 {
        app.step(50);
        if app.world().get::<Owner>(ve).unwrap().faction == FactionId(1) {
            break;
        }
    }
    assert_eq!(app.world().get::<Owner>(ve).unwrap().faction, FactionId(1));
    (s.player, ve, vid, vc)
}

fn drain(app: &mut App, s: Entity) {
    let world = app.world_mut();
    let econ = world.resource::<Economy>().clone();
    let se = world.get::<SettlementEconomy>(s).unwrap().clone();
    let mut stores = vec![se.storehouse];
    let mut q = world.query::<(Entity, &Home)>();
    for (e, _) in q.iter(world) {
        if se.homes.contains(&e) {
            stores.push(e);
        }
    }
    for e in stores {
        if let Some(mut st) = world.get_mut::<Stockpile>(e) {
            let goods: Vec<_> = st.goods.iter().map(|(g, q)| (*g, *q)).collect();
            for (g, q) in goods {
                if econ.is_food(g) {
                    st.take(g, q);
                }
            }
        }
    }
}

fn report(app: &mut App, cap: Entity, ve: Entity, label: &str, seen: &mut usize) {
    let t = app.world().resource::<sim_core::SimTick>().0;
    let owner = app.world().get::<Owner>(ve).copied();
    let sl = app.world().get::<SettlementLoyalty>(ve).copied();
    let thr = app.world().get::<sim_agents::Threat>(ve).copied();
    let c = app.world().get::<Captain>(cap).cloned();
    let world = app.world_mut();
    let mut q = world.query::<(&Villager, &sim_agents::CombatStats)>();
    let militia = q
        .iter(world)
        .filter(|(v, c)| v.settlement == ve && matches!(c.stance, sim_agents::Stance::Militia { .. }))
        .count();
    let mut q = world.query::<&Follower>();
    let men = q.iter(world).filter(|f| f.captain == cap).count();
    let st = app.world().resource::<ArmyStats>().get(FactionId(1));
    let econ = app.world().resource::<Economy>().clone();
    let se = app.world().get::<SettlementEconomy>(ve).unwrap().clone();
    let store = app.world().get::<Stockpile>(se.storehouse).unwrap().food_value(&econ);
    let hpd = app.world().resource::<sim_agents::Villagers>().data.needs.hunger_per_day;
    let fs = app.world().get::<sim_agents::FoodSecurity>(ve).unwrap().days;
    let bag = app.world().get::<Stockpile>(cap).map(|b| b.food_value(&econ));
    println!("   store_days={:.2} foodsec={fs:.2} pop={} bag={bag:?}", store / (hpd * se.population as f32), se.population);
    let log = app.world().resource::<MilitaryLog>();
    for (lt, line) in log.entries.iter().skip((*seen).min(log.entries.len())) {
        println!("  log {lt}: {line}");
    }
    *seen = log.entries.len();
    println!(
        "{label} t={t} owner={:?} mean={:?} below={:?} revolt={:?} militia={militia} men={men} order={:?} threat={:?} supplied={} conq={}",
        owner.map(|o| o.faction.0),
        sl.map(|s| s.mean),
        sl.and_then(|s| s.below_since),
        sl.and_then(|s| s.revolt_since),
        c.and_then(|c| c.orders.front().map(|o| o.spec)),
        thr.map(|t| (t.level, t.provoked_by)),
        st.supplied,
        st.conquests,
    );
}

#[test]
fn probe_garrison_fed() {
    let mut app = headless_app(HeadlessConfig::new(42));
    let (cap, ve, vid, _) = conquered(&mut app);
    order(&mut app, 1, cap, OrderSpec::Garrison { settlement: vid });
    let mut seen = 0;
    for _ in 0..12 {
        app.step(600);
        report(&mut app, cap, ve, "fed", &mut seen);
    }
}

#[test]
fn probe_revolt_unguarded() {
    let mut app = headless_app(HeadlessConfig::new(42));
    let (cap, ve, _, vc) = conquered(&mut app);
    let away = passable_near(&app, vc + bevy_math::IVec2::new(80, 0)).unwrap();
    order(&mut app, 1, cap, OrderSpec::MoveTo { x: away.x, y: away.y });
    drain(&mut app, ve);
    sim_headless::cut_off_farmers(&mut app, ve);
    let mut seen = 0;
    for _ in 0..16 {
        app.step(600);
        if app.world().resource::<sim_core::SimTick>().0 % 2400 < 1200 {
            drain(&mut app, ve);
        }
        report(&mut app, cap, ve, "unguarded", &mut seen);
    }
}

#[test]
fn probe_revolt_garrisoned() {
    let mut app = headless_app(HeadlessConfig::new(42));
    let (cap, ve, vid, _) = conquered(&mut app);
    order(&mut app, 1, cap, OrderSpec::Garrison { settlement: vid });
    drain(&mut app, ve);
    sim_headless::cut_off_farmers(&mut app, ve);
    let mut seen = 0;
    for _ in 0..20 {
        app.step(600);
        if app.world().resource::<sim_core::SimTick>().0 % 2400 < 1200 {
            drain(&mut app, ve);
        }
        report(&mut app, cap, ve, "garrisoned", &mut seen);
    }
}

#[test]
fn probe_rising() {
    let mut app = headless_app(HeadlessConfig::new(42));
    let (cap, ve, vid, _) = conquered(&mut app);
    order(&mut app, 1, cap, OrderSpec::Garrison { settlement: vid });
    let mut seen = 0;
    for _ in 0..40 {
        app.step(150);
        let t = app.world().resource::<sim_core::SimTick>().0;
        if t % 2400 == 2250 && t < 2400 * 3 {
            let world = app.world_mut();
            let mut q = world.query::<(&Villager, &mut sim_agents::Loyalty)>();
            for (v, mut l) in q.iter_mut(world) {
                if v.settlement == ve { l.0 = 0.0; }
            }
        }
        report(&mut app, cap, ve, "rising", &mut seen);
    }
}
