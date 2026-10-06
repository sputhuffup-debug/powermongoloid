//! M3 acceptance (GP-03, SIM-13): posture visibly changes how followers
//! treat villagers. The same hungry army passes the same village once per
//! posture: passive harms nobody and takes nothing; aggressive plunders,
//! provokes the militia and scatters the villagers; neutral sits between.

mod common;

use bevy_math::IVec2;
use common::*;
use sim_agents::{ActionKind, Activity, CombatStats, Stage, Villager};
use sim_core::{FactionId, OrderSpec, Posture, SimStepper};
use sim_headless::{HeadlessConfig, headless_app};
use sim_military::{ArmySpec, ArmyStats, MilitaryLog, spawn_army};

const SEED: u64 = 42;
const TICKS: u64 = 1_500;

#[derive(Debug, Default)]
struct Pass {
    seized: f32,
    villagers_killed: u32,
    provocations: u32,
    /// Villager-samples spent fleeing, hiding or fighting (every 10 ticks).
    scared: u32,
    hurt: u32,
    plunder_logged: bool,
}

fn pass(posture: Posture) -> Pass {
    let mut app = headless_app(HeadlessConfig::new(SEED));
    // The largest village, passed straight through from 40 tiles out.
    let v = villages(&mut app);
    let (village, _, center, _) = *v
        .iter()
        .max_by_key(|x| (x.3, std::cmp::Reverse(x.1)))
        .unwrap();
    let dirs = [IVec2::X, IVec2::Y, IVec2::new(1, 1), IVec2::new(1, -1)];
    let (start, goal) = dirs
        .iter()
        .find_map(|d| {
            let a = passable_near(&app, center - *d * 40)?;
            let b = passable_near(&app, center + *d * 40)?;
            ((a - center).as_vec2().length() > 30.0 && (b - center).as_vec2().length() > 30.0)
                .then_some((a, b))
        })
        .expect("a way through the village");
    let captain = spawn_army(
        app.world_mut(),
        &ArmySpec {
            posture,
            food_days: 0.5,
            ..ArmySpec::new(FactionId(1), start, 100)
        },
    );
    order(
        &mut app,
        1,
        captain,
        OrderSpec::MoveTo {
            x: goal.x,
            y: goal.y,
        },
    );
    let mut out = Pass::default();
    for _ in 0..TICKS / 10 {
        app.step(10);
        let world = app.world_mut();
        let mut q = world.query::<(&Villager, &Activity)>();
        out.scared += q
            .iter(world)
            .filter(|(v, a)| {
                v.settlement == village
                    && a.stage != Stage::Done
                    && matches!(
                        a.kind,
                        ActionKind::Flee | ActionKind::Hide | ActionKind::Fight
                    )
            })
            .count() as u32;
    }
    let world = app.world_mut();
    let mut q = world.query::<(&Villager, &CombatStats)>();
    out.hurt = q
        .iter(world)
        .filter(|(v, c)| v.settlement == village && c.health < c.max_health)
        .count() as u32;
    let s = world.resource::<ArmyStats>().get(FactionId(1));
    out.seized = s.seized;
    out.villagers_killed = s.villagers_killed;
    out.provocations = s.provocations;
    out.hurt += s.villagers_killed;
    out.plunder_logged = world
        .resource::<MilitaryLog>()
        .entries
        .iter()
        .any(|(_, t)| t.contains("plundered"));
    println!("{}: {out:?}", posture.name());
    out
}

#[test]
fn posture_orders_how_an_army_treats_a_village() {
    let p = pass(Posture::Passive);
    let n = pass(Posture::Neutral);
    let a = pass(Posture::Aggressive);
    // Passive: nothing taken, nobody harmed, nobody provoked.
    assert_eq!(p.seized, 0.0);
    assert_eq!(p.hurt, 0);
    assert_eq!(p.provocations, 0);
    // Neutral takes what it needs when hungry but doesn't fight.
    assert!(n.seized > 0.0, "a hungry neutral army takes food");
    assert_eq!(n.provocations, 0);
    assert_eq!(n.hurt, 0);
    // Aggressive plunders, provokes the militia, and it's logged.
    assert!(
        a.seized > n.seized,
        "seized {} vs neutral {}",
        a.seized,
        n.seized
    );
    assert!(a.provocations > 0 && a.plunder_logged);
    assert!(a.hurt > 0, "the militia rose and was hurt");
    // Villagers react more the harsher the posture.
    assert!(
        p.scared < n.scared && n.scared < a.scared,
        "fleeing/hiding/fighting: passive {} neutral {} aggressive {}",
        p.scared,
        n.scared,
        a.scared
    );
}
