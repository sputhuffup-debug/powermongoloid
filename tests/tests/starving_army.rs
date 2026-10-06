//! M3 acceptance (SIM-31): an army without food loses morale and starts
//! deserting within the days set in army.ron; starvation damage comes only
//! after desertions have begun. A fed army in the same run doesn't desert.

mod common;

use bevy_math::IVec2;
use common::*;
use sim_agents::CombatStats;
use sim_core::{Calendar, FactionId, SimStepper};
use sim_headless::{HeadlessConfig, headless_app};
use sim_military::{Army, ArmySpec, ArmyStats, Follower, spawn_army};

const SEED: u64 = 42;

fn mean_morale(app: &mut App, captain: bevy_ecs::entity::Entity) -> f32 {
    let world = app.world_mut();
    let mut q = world.query::<(&Follower, &CombatStats)>();
    let m: Vec<f32> = q
        .iter(world)
        .filter(|(f, _)| f.captain == captain)
        .map(|(_, c)| c.morale)
        .collect();
    m.iter().sum::<f32>() / m.len().max(1) as f32
}

/// Passable tiles far from every village (no foraging, no threat).
fn wilderness(app: &mut App) -> Vec<IVec2> {
    let v = villages(app);
    let (lo, hi) = v
        .iter()
        .fold((v[0].2, v[0].2), |(lo, hi), x| (lo.min(x.2), hi.max(x.2)));
    let mut out = Vec::new();
    for y in (lo.y - 100..=hi.y + 100).step_by(16) {
        for x in (lo.x - 100..=hi.x + 100).step_by(16) {
            let p = IVec2::new(x, y);
            let far = v.iter().all(|s| (s.2 - p).as_vec2().length() > 80.0);
            if far
                && let Some(t) = passable_near(app, p)
                && t == p
            {
                out.push(p);
            }
        }
    }
    out
}

#[test]
fn an_unfed_army_loses_morale_then_deserts_then_starves() {
    let mut app = headless_app(HeadlessConfig::new(SEED));
    let spots = wilderness(&mut app);
    let hungry_at = spots[0];
    let fed_at = *spots
        .iter()
        .find(|p| (**p - hungry_at).as_vec2().length() > 60.0)
        .expect("a second camp");
    let hungry = spawn_army(
        app.world_mut(),
        &ArmySpec {
            food_days: 0.0,
            ..ArmySpec::new(FactionId(1), hungry_at, 200)
        },
    );
    let fed = spawn_army(
        app.world_mut(),
        &ArmySpec {
            food_days: 10.0,
            ..ArmySpec::new(FactionId(2), fed_at, 200)
        },
    );
    let day = Calendar::default_data().ticks_per_day;
    let deadline = app
        .world()
        .resource::<Army>()
        .data
        .upkeep
        .first_desertion_within_days;
    let start = (mean_morale(&mut app, hungry), mean_morale(&mut app, fed));
    app.step(day);
    let day1 = (mean_morale(&mut app, hungry), mean_morale(&mut app, fed));
    app.step(2 * day);
    let day3 = (mean_morale(&mut app, hungry), mean_morale(&mut app, fed));
    let stats = app.world().resource::<ArmyStats>().clone();
    let (h, f) = (stats.get(FactionId(1)), stats.get(FactionId(2)));
    println!(
        "morale hungry {:.2} -> {:.2} -> {:.2}; fed {:.2} -> {:.2} -> {:.2}",
        start.0, day1.0, day3.0, start.1, day1.1, day3.1
    );
    println!(
        "hungry: deserted {} first at {:?}, starvation from {:?}; fed: deserted {}",
        h.deserted, h.first_desertion, h.first_starvation, f.deserted
    );
    assert!(day1.0 < start.0 - 0.2, "morale didn't fall on day 1");
    let first = h.first_desertion.expect("nobody deserted");
    assert!(
        first as f32 <= deadline * day as f32,
        "first desertion at tick {first}, after {deadline} days"
    );
    if let Some(starved) = h.first_starvation {
        assert!(
            starved > first,
            "starvation ({starved}) before desertion ({first})"
        );
    }
    assert!(h.deserted > 100, "only {} deserted in 3 days", h.deserted);
    assert_eq!(f.deserted, 0, "the fed army deserted");
    assert!(
        day3.1 >= 0.75,
        "the fed army's morale fell to {:.2}",
        day3.1
    );
}
