//! Scenario `gather` (GP-02, GP-03, ADR 0018): in a run, a neutral band
//! asking a village for food is fed from the village's pantry (storehouse
//! and homes, the food its Food security counts), and every GatherFood
//! ends with a line saying what it brought or why it brought nothing.
//!
//! - `a_band_of_thirteen_is_fed_on_the_second_morning`: the regression
//!   test for the playtest report "gather food does not seem to work". On
//!   seeds 4 (no harvest modifier) and 3 ("Lean year"), with the start
//!   boon picked and the director silenced, a player band of thirteen
//!   (three days of food) waits about 50 tiles from the independent
//!   village of about 120 people (outside its threat range) until 06:00
//!   on the second day, the storehouse's morning low, when villagers have
//!   carried most of its food home. Ordered to gather food there, it gets
//!   at least a day's food for the band, and the log says "shared food
//!   with Your warband (+N days)". Ordered again at once, with its baggage
//!   at the target, the gather still ends with a line. Before ADR 0018 the
//!   storehouse held 0-0.12 days at that hour and the band got nothing,
//!   with no line at all.
//! - `the_first_morning_gather_says_what_it_got`: the same band on seed 4
//!   at 06:00 on the first day, the leanest hour (the starting stock is in
//!   villagers' hands): whatever it gets, the gather ends with a line, a
//!   share or why there was none (ADR 0018 leaves the first morning lean
//!   for villages under about 150 people).

mod common;

use bevy_ecs::prelude::*;
use bevy_math::Vec2;
use common::*;
use sim_agents::Villagers;
use sim_core::{FactionId, OrderSpec, Posture, SimStepper, SimTick};
use sim_economy::{Economy, SettlementEconomy, Stockpile};
use sim_military::{ArmySpec, Captain, MilitaryLog, Progression, RunProgress, spawn_army};

/// The tick the band is ordered to gather: 06:00 on the second day.
const ORDER_AT: u64 = 3000;
/// 06:00 on the first day: the leanest hour.
const FIRST_MORNING: u64 = 600;

/// Food value in `e`'s stockpile.
fn food(app: &App, e: Entity) -> f32 {
    let econ = app.world().resource::<Economy>();
    app.world()
        .get::<Stockpile>(e)
        .map_or(0.0, |s| s.food_value(econ))
}

/// Military log lines pushed since sequence number `from`.
fn lines_since(app: &App, from: u64) -> Vec<String> {
    app.world()
        .resource::<MilitaryLog>()
        .since(from)
        .map(|(_, _, l)| l.to_string())
        .collect()
}

/// Steps until `cap` has no orders (at most `max` ticks). Returns the
/// baggage's food value just before and just after the tick that ended
/// the last order.
fn step_until_done(app: &mut App, cap: Entity, max: u64) -> (f32, f32) {
    for _ in 0..max {
        let before = food(app, cap);
        app.step(1);
        if app.world().get::<Captain>(cap).unwrap().orders.is_empty() {
            return (before, food(app, cap));
        }
    }
    panic!("the order didn't end within {max} ticks");
}

/// A gathering scene on `seed`: the run with the start boon picked and
/// the director silenced, the independent village of about 120 people,
/// and a neutral player band of thirteen with three days of food about
/// 50 tiles from it.
struct Scene {
    app: App,
    seed: u64,
    modifiers: Vec<String>,
    /// The village: entity, SpawnId, population.
    village: (Entity, u64, u32),
    cap: Entity,
    /// A day's food for the band and for the village.
    band_day: f32,
    village_day: f32,
}

impl Scene {
    fn new(seed: u64) -> Self {
        let mut app = run_app(seed);
        let offer = start_offer(&app);
        choose(&mut app, offer, 0);
        quiet_director(&mut app);
        let seat = app.world().resource::<RunProgress>().rival_seat;
        let modifiers: Vec<String> = {
            let p = app.world().resource::<Progression>();
            app.world()
                .resource::<RunProgress>()
                .modifiers
                .iter()
                .filter_map(|&m| p.modifier(m))
                .map(|m| m.name.clone())
                .collect()
        };
        // The independent village of about 120 people (ties by SpawnId).
        let (s, sid, centre, pop) = villages(&mut app)
            .into_iter()
            .filter(|&(s, id, ..)| Some(id) != seat && owner_of(&app, s) == FactionId::INDEPENDENT)
            .min_by_key(|&(_, id, _, pop)| (pop.abs_diff(120), id))
            .expect("an independent village");
        // About 50 tiles out, beyond its threat range: the first compass
        // direction with a passable tile there.
        let at = (0..8)
            .find_map(|k| {
                let dir = Vec2::from_angle(k as f32 * std::f32::consts::FRAC_PI_4);
                let to = centre + (dir * 50.0).round().as_ivec2();
                passable_near(&app, to).filter(|t| (*t - centre).as_vec2().length() >= 45.0)
            })
            .expect("a passable tile 50 tiles out");
        let cap = spawn_army(
            app.world_mut(),
            &ArmySpec {
                posture: Posture::Neutral,
                food_days: 3.0,
                ..ArmySpec::new(FactionId::PLAYER, at, 12)
            },
        );
        let need = app
            .world()
            .resource::<Villagers>()
            .data
            .needs
            .hunger_per_day;
        println!(
            "seed {seed} {modifiers:?}: village {sid} of {pop} at ({}, {}), band from ({}, {})",
            centre.x, centre.y, at.x, at.y
        );
        Self {
            app,
            seed,
            modifiers,
            village: (s, sid, pop),
            cap,
            band_day: need * 13.0,
            village_day: need * pop as f32,
        }
    }

    /// Steps to `tick`, orders GatherFood at the village and steps until
    /// the order is done. Returns the food the band got and the log lines
    /// since the order, and prints the village's storehouse and homes (in
    /// days of its need) at the order.
    fn gather_at(&mut self, tick: u64) -> (f32, Vec<String>) {
        let (s, sid, _) = self.village;
        let now = self.app.world().resource::<SimTick>().0;
        self.app.step(tick - now);
        let (store, homes) = {
            let se = self
                .app
                .world()
                .get::<SettlementEconomy>(s)
                .unwrap()
                .clone();
            let homes: f32 = se.homes.iter().map(|&h| food(&self.app, h)).sum();
            (food(&self.app, se.storehouse), homes)
        };
        let from = self.app.world().resource::<MilitaryLog>().seq;
        order(
            &mut self.app,
            FactionId::PLAYER.0,
            self.cap,
            OrderSpec::GatherFood { settlement: sid },
        );
        let (before, after) = step_until_done(&mut self.app, self.cap, 800);
        let got = after - before;
        let told = lines_since(&self.app, from);
        println!(
            "seed {} {:?} at tick {tick}: storehouse {:.2} days, homes {:.2} days (pool {:.2}); \
             the band got {got:.1} ({:.2} days); {told:?}",
            self.seed,
            self.modifiers,
            store / self.village_day,
            homes / self.village_day,
            (store + homes) / self.village_day,
            got / self.band_day,
        );
        (got, told)
    }
}

#[test]
fn a_band_of_thirteen_is_fed_on_the_second_morning() {
    for seed in [4, 3] {
        let mut scene = Scene::new(seed);
        let (got, told) = scene.gather_at(ORDER_AT);
        let band_day = scene.band_day;
        assert!(
            got >= band_day,
            "seed {seed}: under a day of food ({got:.1} < {band_day:.1})"
        );
        assert!(
            told.iter()
                .any(|l| l.contains("shared food with Your warband (+")),
            "seed {seed}: {told:?}"
        );

        // Again at once, with the baggage at its target: still a line.
        let sid = scene.village.1;
        let app = &mut scene.app;
        let from = app.world().resource::<MilitaryLog>().seq;
        order(
            app,
            FactionId::PLAYER.0,
            scene.cap,
            OrderSpec::GatherFood { settlement: sid },
        );
        step_until_done(app, scene.cap, 50);
        let told = lines_since(app, from);
        println!("seed {seed}, again: {told:?}");
        assert!(
            told.iter().any(|l| {
                l.contains("Your warband's baggage is full (")
                    || l.contains("had no food to spare for Your warband")
                    || l.contains("had only crumbs to spare for Your warband")
                    || l.contains("shared food with Your warband (+")
            }),
            "seed {seed}: the second gather said nothing: {told:?}"
        );
    }
}

#[test]
fn the_first_morning_gather_says_what_it_got() {
    let mut scene = Scene::new(4);
    // 06:00 on the first day.
    let (got, told) = scene.gather_at(FIRST_MORNING);
    let line = told.iter().find(|l| {
        l.contains("shared food with Your warband (+")
            || l.contains("had no food to spare for Your warband (it holds ")
            || l.contains("had only crumbs to spare for Your warband (it holds ")
    });
    assert!(line.is_some(), "no feed line: {told:?}");
    if got > 0.0 {
        assert!(
            told.iter()
                .any(|l| l.contains("shared food with Your warband (+")),
            "{told:?}"
        );
    }
}
