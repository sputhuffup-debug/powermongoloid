//! M4a step 5 scenario `take_modes` (GP-32; ADR 0020, LORDSHIP_DESIGN
//! section 5): three ways to take a town, each with its preconditions and a
//! lasting tenure.
//!
//! - `a_sack_empties_the_storehouse_and_scars_deeper`: a player captain who
//!   holds at Aggressive sacks the town: its storehouse's food and arms go
//!   into the baggage, the cause is `Sack`, the tenure `Sacked`, the scar
//!   the sack's.
//! - `a_rival_retake_is_conquered_not_sacked`: the rival's bands are always
//!   Aggressive, but a retake is a `Conquest`, `Conquered`, no plunder.
//! - `demand_submits_at_the_ratio`: below the ratio the town refuses (a
//!   Threat deed, the cooldown, the numbers in the log, a second demand
//!   refused); a hostile army breaks a parley off; at the ratio a town
//!   kneels: `Submitted`, loyalty at the seed, its dues held at Tithe, the
//!   Awe term, no Dues term and no `conquests` count.
//! - `a_submitted_town_defies_when_left`: two weak days (fewer than six
//!   watched hours) and it is free again, its militia up against the few
//!   soldiers you left there.
//! - `a_garrisoned_neighbour_or_a_sphere_keeps_it_watched`: no army need
//!   stand within 80 tiles: a garrisoned town of yours, or your other towns'
//!   influence (never the town's own power), keeps it kneeling.
//! - `a_submitted_town_settles`: after `demand.settle_days` the tribute
//!   lock, the watch and the awe end; `how` stays `Submitted`.
//! - `gifts_and_defence_court_a_town`: a gift's goodwill (once a day), a
//!   fight beside a sworn town (`Defended`, a promise kept, the Bond).
//! - `defence_against_its_own_bandits_is_not_defence`: the same fight
//!   against the town's own broken men earns nothing.
//! - `an_unanswered_raid_forsakes`: two lapsed drive-offs at a sworn town:
//!   `Forsaken` twice, and the bond turns.
//!
//! Each test prints the numbers it checks.

mod common;

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use common::*;
use sim_agents::{
    Deed, DeedLog, Edicts, FoodSecurity, Goodwill, Loyalty, LoyaltyTerm, LoyaltyTerms,
    SettlementLoyalty, Threat, Villager,
};
use sim_core::{
    Calendar, Cause, Dial, Dues, FactionId, OrderSpec, Posture, SimEventKind, SimStepper, SimTick,
};
use sim_economy::{Economy, SettlementEconomy, Stockpile};
use sim_military::tenure::{self, DemandData};
use sim_military::{
    ArmySpec, ArmyStats, Bandits, MilitaryLog, Progression, RunProgress, TakenBy, Tenure,
    spawn_army, transfer, transfer_seeded,
};
use sim_world::Settlement;

const SEED: u64 = 42;
const P: FactionId = FactionId::PLAYER;

fn tick(app: &App) -> u64 {
    app.world().resource::<SimTick>().0
}

fn tpd(app: &App) -> u64 {
    app.world().resource::<Calendar>().ticks_per_day
}

fn hour(app: &App) -> u64 {
    app.world().resource::<Calendar>().ticks_per_hour()
}

fn demand_data(app: &App) -> DemandData {
    app.world().resource::<Progression>().data.demand
}

fn tenure_of(app: &App, s: Entity) -> Tenure {
    app.world().get::<Tenure>(s).expect("a run town").clone()
}

fn terms(app: &App, s: Entity) -> LoyaltyTerms {
    *app.world().get::<LoyaltyTerms>(s).expect("terms")
}

fn center(app: &App, s: Entity) -> IVec2 {
    app.world().get::<Settlement>(s).expect("settlement").center
}

fn dist(a: IVec2, b: IVec2) -> f32 {
    (a - b).as_vec2().length()
}

/// A quiet run (no director bands).
fn run() -> App {
    let mut app = run_app(SEED);
    quiet_director(&mut app);
    app
}

/// Free towns (not the seat), by SpawnId: (entity, SpawnId, centre, people).
fn free_towns(app: &mut App) -> Vec<(Entity, u64, IVec2, u32)> {
    let seat = app.world().resource::<RunProgress>().rival_seat;
    villages(app)
        .into_iter()
        .filter(|&(e, id, ..)| Some(id) != seat && owner_of(app, e) == FactionId::INDEPENDENT)
        .collect()
}

/// The player's captains (their entities), by SpawnId, for tests that
/// must keep the band far from a town.
fn player_captain_tiles(app: &mut App) -> Vec<IVec2> {
    let world = app.world_mut();
    let mut q = world.query::<(&sim_military::Captain, &sim_core::Position)>();
    q.iter(world)
        .filter(|(c, _)| c.faction == P)
        .map(|(_, p)| p.0)
        .collect()
}

/// An army of `faction` with `followers` at a passable tile near `at`.
fn army(
    app: &mut App,
    faction: FactionId,
    at: IVec2,
    followers: u32,
    armed: bool,
    posture: Posture,
) -> Entity {
    let at = passable_near(app, at).expect("a passable tile");
    spawn_army(
        app.world_mut(),
        &ArmySpec {
            posture,
            armed,
            food_days: 6.0,
            ..ArmySpec::new(faction, at, followers)
        },
    )
}

/// The military log, kept whole across steps (the resource keeps 64).
#[derive(Default)]
struct Log {
    cursor: u64,
    lines: Vec<(u64, String)>,
}

impl Log {
    fn read(&mut self, app: &App) {
        let log = app.world().resource::<MilitaryLog>();
        for (seq, t, text) in log.since(self.cursor) {
            self.lines.push((t, text.to_string()));
            self.cursor = seq + 1;
        }
    }

    fn step(&mut self, app: &mut App, ticks: u64) {
        let mut left = ticks;
        while left > 0 {
            let n = left.min(50);
            app.step(n);
            self.read(app);
            left -= n;
        }
    }

    fn find(&self, needle: &str) -> Option<&(u64, String)> {
        self.lines.iter().find(|(_, l)| l.contains(needle))
    }
}

/// Deeds of `deed` by the player done at `s` (not heard), from its log.
fn deeds(app: &App, s: Entity, deed: Deed) -> Vec<(u32, f32)> {
    app.world()
        .get::<DeedLog>(s)
        .expect("a deed log")
        .entries
        .iter()
        .filter(|d| d.deed == deed && d.faction == P && !d.heard)
        .map(|d| (d.day, d.delta))
        .collect()
}

fn goodwill(app: &App, s: Entity) -> f32 {
    app.world().get::<Goodwill>(s).expect("goodwill").get(P)
}

/// Food value in `s`'s storehouse, and its weapons and tools.
fn store(app: &App, s: Entity) -> (f32, u32) {
    let w = app.world();
    let econ = w.resource::<Economy>();
    let st = w
        .get::<Stockpile>(w.get::<SettlementEconomy>(s).unwrap().storehouse)
        .unwrap();
    let arms = ["weapons", "tools"]
        .iter()
        .filter_map(|n| econ.data.goods.iter().position(|g| g.name == *n))
        .map(|i| st.get(sim_economy::GoodId(i as u8)))
        .sum();
    (st.food_value(econ), arms)
}

/// Puts `value` of the cheapest food and five weapons into `s`'s storehouse.
fn stock(app: &mut App, s: Entity, value: f32) {
    let w = app.world_mut();
    let econ = w.resource::<Economy>().clone();
    let house = w.get::<SettlementEconomy>(s).unwrap().storehouse;
    let food = *econ.foods_by_value().last().unwrap();
    let per = econ.food_value(food);
    let weapons = econ
        .data
        .goods
        .iter()
        .position(|g| g.name == "weapons")
        .map(|i| sim_economy::GoodId(i as u8));
    let mut st = w.get_mut::<Stockpile>(house).unwrap();
    st.add(food, (value / per) as u32);
    if let Some(g) = weapons {
        st.add(g, 5);
    }
}

/// Hands `s` to the player as a submission at this tick (seed 0.45), as
/// the parley does, and runs a tick so the hourly step holds the tribute.
fn submit(app: &mut App, s: Entity) {
    let seed = demand_data(app).seed;
    transfer_seeded(
        app.world_mut(),
        s,
        P,
        Cause::Submission { captain: 0 },
        Some(seed),
    )
    .expect("populated");
}

/// GP-32: a player captain at Aggressive sacks the hamlet at the hold.
#[test]
fn a_sack_empties_the_storehouse_and_scars_deeper() {
    let mut app = run();
    let towns = free_towns(&mut app);
    let band = player_captain_tiles(&mut app);
    // A hamlet far from the start band, so the band never meddles.
    let &(s, id, at, people) = towns
        .iter()
        .filter(|t| t.3 < 120 && band.iter().all(|b| dist(*b, t.2) > 120.0))
        .min_by_key(|t| t.3)
        .expect("a hamlet away from the band");
    stock(&mut app, s, 40.0);
    let before = store(&app, s);
    let cap = army(
        &mut app,
        P,
        at + IVec2::new(18, 0),
        30,
        true,
        Posture::Aggressive,
    );
    let captain = spawn_id(&app, cap);
    println!("hamlet #{id} ({people} people), store before {before:?}");
    order(&mut app, 1, cap, OrderSpec::Attack { settlement: id });
    let mut log = Log::default();
    while owner_of(&app, s) != P {
        assert!(tick(&app) < 6_000, "the hamlet did not fall");
        log.step(&mut app, 100);
    }
    let t = tenure_of(&app, s);
    let ev = chronicle(&app)
        .into_iter()
        .find(|e| e.event.kind == SimEventKind::Conquest)
        .expect("the sack is a Conquest");
    let after = store(&app, s);
    let data = app.world().resource::<Progression>().data.tenure;
    println!(
        "  taken at {}: {:?}, scar {:.3}, store after {after:?}, line {:?}",
        ev.tick,
        t.how,
        t.scar,
        log.find("sacked")
    );
    assert_eq!(ev.event.cause, Cause::Sack { captain });
    assert_eq!(t.how, TakenBy::Sacked);
    assert!(t.scar >= data.sack_scar && t.scar <= data.max_sack_scar);
    assert_eq!(
        app.world().resource::<ArmyStats>().get(P).conquests,
        1,
        "a sack counts as a conquest"
    );
    assert!(log.find("sacked").is_some(), "the sack is logged");
    assert!(after.0 < 1.0, "the storehouse's food went: {after:?}");
    assert_eq!(after.1, 0, "its arms went");
    assert!(deeds(&app, s, Deed::Plunder).iter().any(|d| d.1 < 0.0));
}

/// GP-32: the rival's band (Aggressive, as all its bands) retakes a town by
/// conquest: no sack.
#[test]
fn a_rival_retake_is_conquered_not_sacked() {
    let mut app = run();
    let band = player_captain_tiles(&mut app);
    let towns = free_towns(&mut app);
    let &(s, id, at, people) = towns
        .iter()
        .filter(|t| t.3 < 120 && band.iter().all(|b| dist(*b, t.2) > 120.0))
        .min_by_key(|t| t.3)
        .expect("a hamlet away from the band");
    transfer(app.world_mut(), s, P, Cause::Invitation).unwrap();
    stock(&mut app, s, 40.0);
    let before = store(&app, s);
    let cap = army(
        &mut app,
        FactionId::RIVAL,
        at + IVec2::new(18, 0),
        30,
        true,
        Posture::Aggressive,
    );
    let captain = spawn_id(&app, cap);
    order(
        &mut app,
        FactionId::RIVAL.0,
        cap,
        OrderSpec::Attack { settlement: id },
    );
    while owner_of(&app, s) != FactionId::RIVAL {
        assert!(tick(&app) < 6_000, "the rival did not retake it");
        app.step(100);
    }
    let t = tenure_of(&app, s);
    let ev = chronicle(&app)
        .into_iter()
        .rfind(|e| e.event.kind == SimEventKind::Conquest)
        .expect("a Conquest");
    let after = store(&app, s);
    println!(
        "town #{id} ({people} people): {:?}, cause {:?}, store {before:?} -> {after:?}",
        t.how, ev.event.cause
    );
    assert_eq!(ev.event.cause, Cause::Conquest { captain });
    assert_eq!(t.how, TakenBy::Conquered);
    assert_eq!(t.by, FactionId::RIVAL);
    assert!(
        deeds(&app, s, Deed::Plunder).is_empty(),
        "no sack: the player plundered nothing"
    );
}

/// GP-32: the parley's comparison, the refusal and the submission.
#[test]
fn demand_submits_at_the_ratio() {
    let mut app = run();
    let data = demand_data(&app);
    let band = player_captain_tiles(&mut app);
    let towns: Vec<_> = free_towns(&mut app)
        .into_iter()
        .filter(|t| band.iter().all(|b| dist(*b, t.2) > 100.0))
        .collect();
    // The biggest free town refuses a small band.
    let &(big, big_id, big_at, big_people) = towns.iter().max_by_key(|t| t.3).unwrap();
    let small = army(
        &mut app,
        P,
        big_at + IVec2::new(6, 0),
        4,
        false,
        Posture::Neutral,
    );
    app.step(1);
    let before = tenure::parley(app.world(), big, P).unwrap();
    println!("town #{big_id} ({big_people} people): {before:?}");
    assert!(!before.submits(), "five men don't frighten a town");
    assert_eq!(before.garrison, 0, "an independent town has no garrison");
    assert!((before.defence - data.militia_weight * before.able as f32).abs() < 1e-4);
    assert!((before.need - data.ratio * before.defence).abs() < 1e-4);
    assert_eq!(before.strength, 5.0, "five unarmed men within 40 tiles");
    let gw = goodwill(&app, big);
    let mut log = Log::default();
    order(&mut app, 1, small, OrderSpec::Demand { settlement: big_id });
    let ticks = 2 * hour(&app);
    log.step(&mut app, ticks);
    let t = tenure_of(&app, big);
    let line = log.find("refuses").cloned().expect("a refusal line");
    println!("  {line:?}; cooldown until {}", t.cool_until);
    assert_eq!(owner_of(&app, big), FactionId::INDEPENDENT);
    assert!(line.1.contains("would fight") && line.1.contains("within 40 tiles and have 5"));
    assert!(t.cool_until >= line.0 + (data.refuse_cooldown_days * tpd(&app) as f32) as u64);
    assert!(
        deeds(&app, big, Deed::Threat)
            .iter()
            .any(|d| (d.1 - (-0.05)).abs() < 1e-4),
        "a Threat deed"
    );
    assert!(goodwill(&app, big) < gw);
    // A second demand while it cools down is refused outright.
    let refused = rejected(&app, P);
    order(&mut app, 1, small, OrderSpec::Demand { settlement: big_id });
    app.step(1);
    assert_eq!(rejected(&app, P), refused + 1, "the cooldown holds");

    // A hostile army near the gate breaks a parley off.
    let &(mid, mid_id, mid_at, _) = towns
        .iter()
        .filter(|t| t.1 != big_id && dist(t.2, big_at) > 100.0)
        .min_by_key(|t| t.1)
        .unwrap();
    let envoy = army(
        &mut app,
        P,
        mid_at + IVec2::new(0, 6),
        2,
        false,
        Posture::Neutral,
    );
    army(
        &mut app,
        FactionId::RAIDERS,
        mid_at + IVec2::new(30, 0),
        3,
        false,
        Posture::Neutral,
    );
    order(&mut app, 1, envoy, OrderSpec::Demand { settlement: mid_id });
    let ticks = 2 * hour(&app);
    log.step(&mut app, ticks);
    assert!(
        log.find("was broken off").is_some(),
        "the raiders broke it off"
    );
    assert_eq!(owner_of(&app, mid), FactionId::INDEPENDENT);
    assert_eq!(
        tenure_of(&app, mid).cool_until,
        0,
        "no refusal, no cooldown"
    );

    // A band at the ratio makes a smaller town kneel.
    let &(s, id, at, people) = towns
        .iter()
        .filter(|t| t.1 != big_id && t.1 != mid_id && dist(t.2, mid_at) > 100.0)
        .filter(|t| dist(t.2, big_at) > 60.0)
        .min_by_key(|t| t.3)
        .unwrap();
    let p0 = tenure::parley(app.world(), s, P).unwrap();
    let men = (p0.need / data.armed_weight).ceil() as u32 + 2;
    let host = army(
        &mut app,
        P,
        at + IVec2::new(-6, 0),
        men,
        true,
        Posture::Neutral,
    );
    let conquests = app.world().resource::<ArmyStats>().get(P).conquests;
    order(&mut app, 1, host, OrderSpec::Demand { settlement: id });
    let ticks = 2 * hour(&app);
    log.step(&mut app, ticks);
    let ev = chronicle(&app)
        .into_iter()
        .find(|e| e.event.kind == SimEventKind::Submitted)
        .expect("it knelt");
    let t = tenure_of(&app, s);
    let terms = terms(&app, s);
    let edicts = app.world().get::<Edicts>(s).unwrap().clone();
    let mean = app.world().get::<SettlementLoyalty>(s).unwrap().mean;
    println!(
        "town #{id} ({people} people), {men} armed men: {p0:?}; knelt at {}, loyalty {mean:.3}, \
         awe {:.2}, dues {:?} locked until {}",
        ev.tick,
        terms.get(LoyaltyTerm::Awe),
        edicts.dues,
        edicts.dial(Dial::Dues).locked_until
    );
    let captain = spawn_id(&app, host);
    assert_eq!(ev.event.cause, Cause::Submission { captain });
    assert_eq!(owner_of(&app, s), P);
    assert_eq!((t.how, t.scar), (TakenBy::Submitted, 0.0));
    assert_eq!(edicts.dues, Dues::Tithe, "tribute");
    assert_eq!(
        edicts.dial(Dial::Dues).locked_until,
        tenure::settles_at(app.world(), t.since),
        "locked until it settles"
    );
    assert_eq!(terms.get(LoyaltyTerm::Awe), data.awe, "watched by the band");
    assert_eq!(terms.get(LoyaltyTerm::Dues), 0.0, "tribute is the deal");
    assert_eq!(
        app.world().resource::<ArmyStats>().get(P).conquests,
        conquests,
        "a submission is no conquest"
    );
    // Residents start at the seed (one tick of drift at most).
    let world = app.world_mut();
    let mut q = world.query::<(&Villager, &Loyalty)>();
    let l: Vec<f32> = q
        .iter(world)
        .filter(|(v, _)| v.settlement == s)
        .map(|(_, l)| l.0)
        .collect();
    assert!(l.iter().all(|x| (x - data.seed).abs() < 0.05), "{l:?}");
}

/// The first free town (by SpawnId) of at least `min_people` with no
/// player captain within 120 tiles: (entity, SpawnId, centre).
fn lonely_town(app: &mut App, min_people: u32) -> (Entity, u64, IVec2) {
    let band = player_captain_tiles(app);
    let towns = free_towns(app);
    towns
        .iter()
        .filter(|t| t.3 >= min_people && band.iter().all(|b| dist(*b, t.2) > 120.0))
        .map(|t| (t.0, t.1, t.2))
        .next()
        .expect("a town away from the band")
}

/// GP-32: left without a watch, a submitted town defies you after two weak
/// days; the few soldiers left there face its militia.
#[test]
fn a_submitted_town_defies_when_left() {
    let mut app = run();
    app.world_mut()
        .resource_mut::<Progression>()
        .data
        .influence
        .enabled = false;
    let (s, id, at) = lonely_town(&mut app, 80);
    // Two men stay at the gate: too few to watch it.
    army(
        &mut app,
        P,
        at + IVec2::new(4, 0),
        1,
        false,
        Posture::Neutral,
    );
    app.step(1);
    submit(&mut app, s);
    let data = demand_data(&app);
    let since = tick(&app);
    while owner_of(&app, s) == P {
        assert!(tick(&app) < since + 5 * tpd(&app), "it never defied");
        let next = (tick(&app) / tpd(&app) + 1) * tpd(&app);
        app.step(next - tick(&app) - 1);
        let t = tenure_of(&app, s);
        let w = tenure::watch(app.world(), s).unwrap();
        println!(
            "  before the day tick {next}: watched {} of 24 hours, weak days {}, {w:?}",
            t.watched_hours, t.weak_days
        );
        assert!(!w.watched);
        app.step(1);
    }
    let ev = chronicle(&app)
        .into_iter()
        .find(|e| e.event.kind == SimEventKind::Defied)
        .expect("a Defied event");
    let t = tenure_of(&app, s);
    let threat = *app.world().get::<Threat>(s).unwrap();
    println!(
        "town #{id} defied at {} (knelt at {since}), tenure {:?}, provoked by {:?}",
        ev.tick, t.how, threat.provoked_by
    );
    assert_eq!(ev.event.cause, Cause::Defiance);
    assert_eq!(ev.event.faction, P, "told like a revolt");
    assert_eq!(t.how, TakenBy::Freed);
    // The first, partial day is not judged; two whole weak days are.
    let elapsed = ev.tick - since;
    assert!(
        elapsed > 2 * tpd(&app) && elapsed <= 3 * tpd(&app),
        "{elapsed}"
    );
    assert_eq!(data.weak_days, 2);
    assert_eq!(threat.provoked_by, Some(P), "its militia rises at the gate");
}

/// GP-32: a garrisoned town of yours within 80 tiles, or your other towns'
/// influence, watches a submitted town with no army near it.
#[test]
fn a_garrisoned_neighbour_or_a_sphere_keeps_it_watched() {
    // (a) A garrisoned neighbour, influence off.
    let mut app = run();
    app.world_mut()
        .resource_mut::<Progression>()
        .data
        .influence
        .enabled = false;
    let band = player_captain_tiles(&mut app);
    let seat = app.world().resource::<RunProgress>().rival_seat;
    let all: Vec<_> = villages(&mut app)
        .into_iter()
        .filter(|t| Some(t.1) != seat)
        .collect();
    let towns = free_towns(&mut app);
    // The run island's towns stand at least about 100 tiles apart, so no
    // town has a neighbour within the shipped 80 (STEP5.md, open issues):
    // the scenario widens the watch to 125 to test the rule.
    app.world_mut()
        .resource_mut::<Progression>()
        .data
        .demand
        .watch_radius = 125.0;
    // A town of 100 or more (so the soldiers next door can't watch it by
    // their strength alone) with a neighbour within 120 tiles, the nearest
    // pair first.
    let (&(s, id, at, _), &(n, n_id, n_at, _)) = towns
        .iter()
        .filter(|t| t.3 >= 100)
        .flat_map(|a| all.iter().map(move |b| (a, b)))
        .filter(|(a, b)| a.1 != b.1 && dist(a.2, b.2) <= 120.0)
        .min_by(|x, y| dist(x.0.2, x.1.2).total_cmp(&dist(y.0.2, y.1.2)))
        .expect("a town with a neighbour within 120 tiles");
    transfer(app.world_mut(), n, P, Cause::Invitation).unwrap();
    let data = demand_data(&app);
    army(
        &mut app,
        P,
        n_at,
        data.watch_garrison - 1,
        false,
        Posture::Neutral,
    );
    app.step(1);
    submit(&mut app, s);
    app.step(1);
    let w = tenure::watch(app.world(), s).unwrap();
    println!(
        "town #{id}, neighbour #{n_id} {:.0} tiles away: {w:?}",
        dist(n_at, at)
    );
    assert_eq!(w.garrisoned_by, Some(n_id));
    assert!(w.watched);
    if band.iter().all(|b| dist(*b, at) > 125.0) {
        assert!(
            w.strength < w.hold,
            "the garrison watches, not the strength"
        );
    }
    app.step(4 * tpd(&app));
    let t = tenure_of(&app, s);
    println!("  after four days: {t:?}");
    assert_eq!(owner_of(&app, s), P, "still kneeling");
    assert_eq!(t.weak_days, 0);
    assert_eq!(terms(&app, s).get(LoyaltyTerm::Awe), data.awe);

    // (b) The sphere, its own power left out.
    let mut app = run();
    {
        let mut p = app.world_mut().resource_mut::<Progression>();
        p.data.influence.independence = 0.05;
    }
    let (s, id, at) = lonely_town(&mut app, 60);
    submit(&mut app, s);
    app.step(1);
    let alone = tenure::watch(app.world(), s).unwrap();
    let own = app
        .world()
        .get::<sim_military::Influence>(s)
        .unwrap()
        .player;
    println!("town #{id} alone: own share {own:.3}, {alone:?}");
    assert!(own > 0.0, "its own power counts in its own field");
    assert_eq!(alone.share, 0.0, "but never in its watch");
    let near: Vec<Entity> = free_towns(&mut app)
        .into_iter()
        .filter(|t| t.1 != id && dist(t.2, at) <= 160.0)
        .map(|t| t.0)
        .collect();
    for t in &near {
        transfer(app.world_mut(), *t, P, Cause::Invitation).unwrap();
    }
    app.step(hour(&app));
    let w = tenure::watch(app.world(), s).unwrap();
    println!("  with {} towns of yours near: {w:?}", near.len());
    assert!(w.share >= demand_data(&app).watch_share, "{w:?}");
    assert!(w.garrisoned_by.is_none());
    app.step(4 * tpd(&app));
    assert_eq!(owner_of(&app, s), P, "the sphere keeps it");
    assert_eq!(tenure_of(&app, s).weak_days, 0);
}

/// GP-32: after `settle_days` the tribute, the watch and the awe end.
#[test]
fn a_submitted_town_settles() {
    let mut app = run();
    assert_eq!(demand_data(&app).settle_days, 10.0);
    app.world_mut()
        .resource_mut::<Progression>()
        .data
        .demand
        .settle_days = 2.0;
    let (s, id, at) = lonely_town(&mut app, 60);
    let host = army(
        &mut app,
        P,
        at + IVec2::new(5, 0),
        40,
        true,
        Posture::Neutral,
    );
    app.step(1);
    submit(&mut app, s);
    let since = tick(&app);
    let settle = tenure::settles_at(app.world(), since);
    assert_eq!(settle, since + 2 * tpd(&app));
    let mut log = Log::default();
    let ticks = settle - tick(&app) - 1;
    log.step(&mut app, ticks);
    assert!(tenure::paying_tribute(
        app.world(),
        &tenure_of(&app, s),
        tick(&app)
    ));
    assert!(terms(&app, s).get(LoyaltyTerm::Awe) > 0.0);
    let ticks = hour(&app) + 1;
    log.step(&mut app, ticks);
    let t = tenure_of(&app, s);
    let e = app.world().get::<Edicts>(s).unwrap().clone();
    println!(
        "town #{id} settled: {:?}, awe {}, dues {:?} locked until {} (tick {})",
        log.find("settled"),
        terms(&app, s).get(LoyaltyTerm::Awe),
        e.dues,
        e.dial(Dial::Dues).locked_until,
        tick(&app)
    );
    assert!(log.find("has settled").is_some());
    assert_eq!(t.how, TakenBy::Submitted, "kept as a record");
    assert!(!tenure::paying_tribute(app.world(), &t, tick(&app)));
    assert_eq!(terms(&app, s).get(LoyaltyTerm::Awe), 0.0);
    assert!(
        e.dial(Dial::Dues).locked_until <= tick(&app),
        "the lock is over"
    );
    // No watch any more: the band leaves and it stays.
    let far = IVec2::new(at.x + 200, at.y).clamp(IVec2::ZERO, IVec2::splat(1000));
    order(&mut app, 1, host, OrderSpec::MoveTo { x: far.x, y: far.y });
    app.step(3 * tpd(&app));
    assert_eq!(owner_of(&app, s), P, "a settled town never defies");
}

/// A battle between a strong player army and a raider band beside town
/// `s`: returns when the player's battle report is in, with `bandits_home`
/// marking the raiders as the broken men of that town.
fn fight_beside(app: &mut App, s: Entity, bandits_home: Option<u64>) {
    // A drive-off answered there would be the day's defence (and pays its
    // own reward): keep it open, so the battle report is what counts.
    app.world_mut()
        .resource_mut::<Progression>()
        .data
        .requests
        .clear_hours = 1e6;
    let at = center(app, s);
    let host = army(
        app,
        P,
        at + IVec2::new(12, 0),
        30,
        true,
        Posture::Aggressive,
    );
    let raiders = army(
        app,
        FactionId::RAIDERS,
        at + IVec2::new(16, 0),
        6,
        false,
        Posture::Aggressive,
    );
    if let Some(home) = bandits_home {
        app.world_mut()
            .entity_mut(raiders)
            .insert(Bandits { home, raids: 0 });
    }
    let reports = app
        .world()
        .resource::<sim_core::Chronicle>()
        .reports()
        .len();
    let start = tick(app);
    loop {
        app.step(50);
        let world = app.world();
        let done = world.resource::<sim_core::Chronicle>().reports()[reports..]
            .iter()
            .any(|r| r.faction == P && r.kills > 0);
        if done {
            break;
        }
        assert!(tick(app) < start + 2 * tpd(app), "no battle report");
    }
    let _ = host;
    // The progress step after the report.
    app.step(hour(app));
}

/// GP-32: a gift, then a fight for a sworn town.
#[test]
fn gifts_and_defence_court_a_town() {
    let mut app = run();
    let (s, id, at) = lonely_town(&mut app, 60);
    let court = app.world().resource::<Progression>().data.court;
    let giver = army(
        &mut app,
        P,
        at + IVec2::new(4, 0),
        3,
        false,
        Posture::Passive,
    );
    app.step(1);
    let fed = app.world().get::<FoodSecurity>(s).unwrap().days;
    order(&mut app, 1, giver, OrderSpec::Give { settlement: id });
    app.step(3);
    let given = app.world().resource::<ArmyStats>().get(P).donated;
    let gifts = deeds(&app, s, Deed::Gift);
    let people = app.world().get::<SettlementEconomy>(s).unwrap().population;
    let need = app
        .world()
        .resource::<sim_agents::Villagers>()
        .data
        .needs
        .hunger_per_day
        * people as f32;
    let hungry = fed
        < app
            .world()
            .resource::<Progression>()
            .data
            .requests
            .food_days;
    let mut want = (court.gift_per_day_of_need * given / need).min(court.max_gift);
    if hungry {
        want *= court.hungry_mult;
    }
    println!(
        "town #{id}: gave {given:.1} food (need {need:.1} a day, food {fed:.2} days): {gifts:?}, \
         expected {want:.4}"
    );
    assert!(given > 0.0);
    assert_eq!(gifts.len(), 1);
    assert!((gifts[0].1 - want).abs() < 1e-4);
    // A second gift the same day counts for nothing.
    let host2 = army(
        &mut app,
        P,
        at + IVec2::new(-4, 0),
        3,
        false,
        Posture::Passive,
    );
    order(&mut app, 1, host2, OrderSpec::Give { settlement: id });
    app.step(3);
    assert!(app.world().resource::<ArmyStats>().get(P).donated > given);
    assert_eq!(deeds(&app, s, Deed::Gift), gifts, "once a town a day");

    // Defence of a sworn town.
    let mut app = run();
    let (s, id, _) = lonely_town(&mut app, 60);
    transfer(app.world_mut(), s, P, Cause::Invitation).unwrap();
    let tdata = app.world().resource::<Progression>().data.tenure;
    assert_eq!(terms(&app, s).get(LoyaltyTerm::Bond), tdata.bond);
    fight_beside(&mut app, s, None);
    let t = tenure_of(&app, s);
    let defended = deeds(&app, s, Deed::Defended);
    println!(
        "sworn town #{id}: defended {defended:?}, kept {}, bond {:.2}",
        t.kept,
        terms(&app, s).get(LoyaltyTerm::Bond)
    );
    assert_eq!(defended.len(), 1);
    assert!((defended[0].1 - 0.1).abs() < 1e-4);
    assert_eq!(t.kept, 1);
    assert!((terms(&app, s).get(LoyaltyTerm::Bond) - (tdata.bond + tdata.kept_bond)).abs() < 1e-6);
}

/// GP-32, GP-33: a fight against the town's own broken men is no defence.
#[test]
fn defence_against_its_own_bandits_is_not_defence() {
    let mut app = run();
    let (s, id, _) = lonely_town(&mut app, 60);
    transfer(app.world_mut(), s, P, Cause::Invitation).unwrap();
    fight_beside(&mut app, s, Some(id));
    let t = tenure_of(&app, s);
    println!(
        "town #{id}: defended {:?}, kept {}",
        deeds(&app, s, Deed::Defended),
        t.kept
    );
    assert!(deeds(&app, s, Deed::Defended).is_empty());
    assert_eq!((t.kept, t.defended_day), (0, 0));
}

/// GP-32: two drive-offs at a sworn town lapse unanswered: forsaken twice,
/// and the bond turns for good.
#[test]
fn an_unanswered_raid_forsakes() {
    let mut app = run();
    {
        let mut p = app.world_mut().resource_mut::<Progression>();
        p.data.requests.deadline_hours = 2.0;
        p.data.requests.cooldown_hours = 1.0;
    }
    let (s, id, at) = lonely_town(&mut app, 60);
    transfer(app.world_mut(), s, P, Cause::Invitation).unwrap();
    // Raiders camp within the threat radius and stay.
    army(
        &mut app,
        FactionId::RAIDERS,
        at + IVec2::new(30, 0),
        3,
        false,
        Posture::Passive,
    );
    let tdata = app.world().resource::<Progression>().data.tenure;
    let start = tick(&app);
    while tenure_of(&app, s).broken < tdata.forsaken_after {
        assert!(tick(&app) < start + tpd(&app), "no second lapse");
        app.step(hour(&app));
    }
    let events: Vec<_> = chronicle(&app)
        .into_iter()
        .filter(|e| e.event.kind == SimEventKind::Forsaken)
        .collect();
    let t = tenure_of(&app, s);
    println!(
        "town #{id}: broken {}, bond {:.2}, forsaken {:?}, deeds {:?}",
        t.broken,
        terms(&app, s).get(LoyaltyTerm::Bond),
        events
            .iter()
            .map(|e| (e.tick, e.event.amount))
            .collect::<Vec<_>>(),
        deeds(&app, s, Deed::Forsaken)
    );
    assert_eq!(events.len(), 2);
    assert_eq!(
        events.iter().map(|e| e.event.amount).collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert!(
        events
            .iter()
            .all(|e| e.event.subject == id && e.event.cause == Cause::Lapsed)
    );
    assert_eq!(terms(&app, s).get(LoyaltyTerm::Bond), tdata.forsaken_bond);
    assert!(!deeds(&app, s, Deed::Forsaken).is_empty());
}
