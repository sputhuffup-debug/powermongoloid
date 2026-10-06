//! M3 acceptance (SIM-30): the player orders a captain with 200 followers
//! across the populated region, with a lake in the straight-line path; the
//! army moves as a crowd around it.

mod common;

use bevy_math::{IVec2, Vec2};
use common::*;
use sim_core::{CommandLog, OrderSpec, SimStepper, compute_state_hash};
use sim_headless::{HeadlessConfig, Threading, headless_app};
use sim_military::{Army, ArmySpec, spawn_army};
use sim_world::{Terrain, WorldMap};

const SEED: u64 = 42;
const FOLLOWERS: u32 = 200;
const LAKE_RADIUS: i32 = 14;
/// Straight-line length of the march, in tiles.
const MARCH: f32 = 220.0;
/// Time limit: two game days.
const LIMIT: u64 = 4_800;

struct Outcome {
    arrived: usize,
    total: usize,
    arrival_tick: Option<u64>,
    worst_spread: f32,
    on_water: u32,
    hash: u64,
    logged_orders: usize,
}

fn march(threading: Threading) -> Outcome {
    let mut app = headless_app(HeadlessConfig {
        threading,
        ..HeadlessConfig::new(SEED)
    });
    // Two populated settlements about MARCH tiles apart.
    let v = villages(&mut app);
    let (mut a, mut b, mut best) = (v[0].2, v[0].2, f32::MAX);
    for x in &v {
        for y in &v {
            let d = (x.2 - y.2).as_vec2().length();
            if (d - MARCH).abs() < (best - MARCH).abs() {
                (a, b, best) = (x.2, y.2, d);
            }
        }
    }
    assert!(best > 120.0, "region too small for a march: {best}");
    // A lake across the straight line, halfway.
    let mid = (a + b) / 2;
    {
        let map = &mut *app.world_mut().resource_mut::<WorldMap>();
        for dy in -LAKE_RADIUS..=LAKE_RADIUS {
            for dx in -LAKE_RADIUS..=LAKE_RADIUS {
                if dx * dx + dy * dy <= LAKE_RADIUS * LAKE_RADIUS {
                    map.set_terrain(mid + IVec2::new(dx, dy), Terrain::Water);
                }
            }
        }
    }
    let start = passable_near(&app, a).unwrap();
    let goal = passable_near(&app, b).unwrap();
    let captain = spawn_army(
        app.world_mut(),
        &ArmySpec {
            food_days: 6.0,
            ..ArmySpec::new(sim_core::FactionId(1), start, FOLLOWERS)
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
    let army = app.world().resource::<Army>().clone();
    let loose = army.formation_radius(FOLLOWERS);
    let mut worst_spread: f32 = 0.0;
    let mut on_water = 0;
    let mut arrival_tick = None;
    let mut tick = 0;
    while tick < LIMIT {
        app.step(10);
        tick += 10;
        let c = pos_of(&app, captain);
        let mut fs = followers(&mut app, captain);
        {
            let map = app.world().resource::<WorldMap>();
            on_water += fs
                .iter()
                .filter(|(_, t)| map.terrain(*t) == Terrain::Water)
                .count() as u32;
        }
        let mut d: Vec<f32> = fs.iter().map(|(p, _)| p.distance(c)).collect();
        let spread = quantile(&mut d, 0.95);
        worst_spread = worst_spread.max(spread);
        if tick % 300 == 0 {
            println!(
                "  tick {tick}: captain at {c:.0}, {} followers, spread 95% {spread:.1} max {:.1}",
                fs.len(),
                d.last().copied().unwrap_or(0.0)
            );
        }
        let at_goal = fs
            .iter_mut()
            .filter(|(p, _)| p.distance(goal.as_vec2() + Vec2::splat(0.5)) <= loose + 4.0)
            .count();
        if arrival_tick.is_none() && at_goal * 100 >= fs.len() * 95 {
            arrival_tick = Some(tick);
            break;
        }
    }
    let fs = followers(&mut app, captain);
    let arrived = fs
        .iter()
        .filter(|(p, _)| p.distance(goal.as_vec2() + Vec2::splat(0.5)) <= loose + 4.0)
        .count();
    println!(
        "march {:.0} tiles, lake at {mid:?}: {arrived}/{} arrived at tick {arrival_tick:?}; \
         95th-percentile spread up to {worst_spread:.1} (loose radius {loose:.1})",
        best,
        fs.len()
    );
    let logged_orders = app.world().resource::<CommandLog>().0.len();
    Outcome {
        arrived,
        total: fs.len(),
        arrival_tick,
        worst_spread,
        on_water,
        hash: compute_state_hash(app.world_mut()),
        logged_orders,
    }
}

#[test]
fn an_army_of_200_marches_around_a_lake_as_a_crowd() {
    let run = march(Threading::Multi);
    assert!(
        run.arrival_tick.is_some(),
        "only {}/{} arrived within {LIMIT} ticks",
        run.arrived,
        run.total
    );
    assert!(run.total >= 195, "lost followers on the way: {}", run.total);
    assert_eq!(run.on_water, 0, "followers stood on water");
    let loose = Army::default_data(
        &sim_core::Calendar::default_data(),
        &sim_economy::Economy::default_data(),
    )
    .formation_radius(FOLLOWERS);
    assert!(
        run.worst_spread <= 2.5 * loose,
        "the crowd strung out: 95% within {:.1}, loose radius {loose:.1}",
        run.worst_spread
    );
    // The order arrived only as a command, and is in the replay log.
    assert_eq!(run.logged_orders, 1);
    // The same march, run serially, ends in the same state (NFR-08).
    let serial = march(Threading::Single);
    assert_eq!(run.hash, serial.hash, "march is not deterministic");
}
