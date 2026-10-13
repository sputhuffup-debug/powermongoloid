//! M4a step 6 scenario `fallout` (GP-33, ADR 0020, LORDSHIP_DESIGN section
//! 7): violence ripples. A player's conquest or sack of a big town raises
//! its broken men; a take from the rival brings a vow of reprisal.
//!
//! All scenarios play the run island of seed 42 (steady, the pacing
//! director silenced), driving the sim through its real paths
//! (`transfer_seeded` with a `Cause`, the hourly progress steps, real
//! fights), with the shipped data.
//!
//! - `broken_men_are_its_own_people`: a bloody conquest queues
//!   `band_size(..)` men; after `delay_hours` a RAIDERS band (`Bandits`,
//!   `Raider`) of that town's own villagers stands, the town's population
//!   dropped by the band, `BanditsRose` told with cause `Fallout`.
//! - `bandits_prefer_the_rival`: a stepped band's first target is a rival
//!   town (distance silenced), and after `turn_after_raids` raids the
//!   rival's weight is the player's (`bandit_score`).
//! - `peaceful_takes_raise_none`: a submission, an invitation and a conquest
//!   of a town under `min_people` queue no band.
//! - `the_cap_holds`: three bloody takes, at most `max_bands` bands alive,
//!   the third waits, its town broods once, and it rises when a band is gone.
//! - `heirs_keep_the_band`: a fallen bandit captain's heir is a bandit.
//! - `killing_its_bandits_costs_goodwill_at_their_home`: a player's blow
//!   that kills a bandit is a `Killing` deed at his home and no `Defended`.
//! - `a_vow_comes_due_at_the_newest_holding`: a take from the rival vows a
//!   reprisal 18 hours on (once per cooldown); when due, a band pinned to
//!   the player's newest holding splits off the garrison.

mod common;

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use common::*;
use sim_agents::{CombatStats, Deed, DeedLog, Goodwill, Owner, Villager};
use sim_core::{Calendar, Cause, FactionId, OrderSpec, Posture, SimEventKind, SimStepper, SimTick};
use sim_economy::SettlementEconomy;
use sim_military::fallout::{band_size, bandit_score, bands_alive};
use sim_military::{
    ArmySpec, Bandits, Captain, Fallout, Follower, Progression, Raider, RivalBand, RivalState,
    RunProgress, Tenure, spawn_army, transfer, transfer_seeded,
};
use sim_world::Settlement;

const SEED: u64 = 42;
const P: FactionId = FactionId::PLAYER;

fn tick(app: &App) -> u64 {
    app.world().resource::<SimTick>().0
}

fn hour(app: &App) -> u64 {
    app.world().resource::<Calendar>().ticks_per_hour()
}

fn run() -> App {
    let mut app = run_app(SEED);
    quiet_director(&mut app);
    app
}

fn data(app: &App) -> sim_military::FalloutData {
    app.world().resource::<Progression>().data.fallout
}

/// Free towns (not the seat) of at least `min` people, by SpawnId.
fn free_towns(app: &mut App, min: u32) -> Vec<(Entity, u64, IVec2, u32)> {
    let seat = app.world().resource::<RunProgress>().rival_seat;
    villages(app)
        .into_iter()
        .filter(|&(e, id, _, people)| {
            Some(id) != seat && people >= min && owner_of(app, e) == FactionId::INDEPENDENT
        })
        .collect()
}

/// Towns the rival holds besides its seat: (entity, SpawnId, centre).
fn rival_towns(app: &mut App) -> Vec<(Entity, u64, IVec2)> {
    let seat = app.world().resource::<RunProgress>().rival_seat;
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &sim_core::SpawnId, &Owner, &Settlement)>();
    let mut v: Vec<_> = q
        .iter(world)
        .filter(|(_, id, o, _)| o.faction == FactionId::RIVAL && Some(id.0) != seat)
        .map(|(e, id, _, s)| (e, id.0, s.center))
        .collect();
    v.sort_unstable_by_key(|t| t.1);
    v
}

fn people(app: &App, s: Entity) -> u32 {
    app.world().get::<SettlementEconomy>(s).unwrap().population
}

/// Blood the player's militia-killing spilt in `s` (a fight's worth).
fn spill(app: &mut App, s: Entity, n: u32) {
    let mut t = app.world_mut().get_mut::<Tenure>(s).expect("a run town");
    for _ in 0..n {
        t.spill(P);
    }
}

/// A conquest of `s` by the player with `blood` of its militia killed.
fn conquer(app: &mut App, s: Entity, blood: u32) {
    spill(app, s, blood);
    transfer(app.world_mut(), s, P, Cause::Conquest { captain: 0 }).expect("populated");
}

/// Bandit captains alive: (entity, SpawnId, Bandits), by SpawnId.
fn bands(app: &mut App) -> Vec<(Entity, u64, Bandits)> {
    let world = app.world_mut();
    let mut q = world.query_filtered::<(Entity, &sim_core::SpawnId, &Bandits), With<Captain>>();
    let mut v: Vec<_> = q.iter(world).map(|(e, id, b)| (e, id.0, *b)).collect();
    v.sort_unstable_by_key(|b| b.1);
    v
}

/// A band's people: its captain and living followers.
fn members(app: &mut App, lead: Entity) -> Vec<Entity> {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &Follower)>();
    let mut m: Vec<Entity> = q
        .iter(world)
        .filter(|(_, f)| f.captain == lead)
        .map(|(e, _)| e)
        .collect();
    m.push(lead);
    m
}

fn events(app: &App, kind: SimEventKind) -> Vec<sim_core::ChronicleEntry> {
    chronicle(app)
        .into_iter()
        .filter(|e| e.event.kind == kind)
        .collect()
}

fn deeds(app: &App, s: Entity, deed: Deed) -> Vec<f32> {
    app.world()
        .get::<DeedLog>(s)
        .expect("a deed log")
        .entries
        .iter()
        .filter(|d| d.deed == deed && d.faction == P && !d.heard)
        .map(|d| d.delta)
        .collect()
}

/// GP-33: a bloody conquest of a big town raises a band of its own people.
#[test]
fn broken_men_are_its_own_people() {
    let mut app = run();
    let d = data(&app);
    let &(s, id, _, pop) = free_towns(&mut app, 100).first().expect("a big town");
    let blood = 8;
    let want = band_size(&d, pop, blood, false, false).expect("a band");
    conquer(&mut app, s, blood);
    let pending = app.world().resource::<Fallout>().pending.clone();
    println!("town #{id} ({pop} people), blood {blood}: pending {pending:?}, want {want}");
    assert_eq!(pending.len(), 1);
    assert_eq!((pending[0].settlement, pending[0].men), (id, want));
    assert!(bands(&mut app).is_empty(), "nothing spawns at the taking");
    let queued = tick(&app);
    assert_eq!(
        pending[0].due,
        queued + (d.delay_hours * hour(&app) as f32) as u64
    );
    // Rises at the first hourly step at or after `due`.
    let before = people(&app, s);
    while bands(&mut app).is_empty() {
        assert!(tick(&app) < queued + 6 * hour(&app), "no band rose");
        app.step(hour(&app));
    }
    let bs = bands(&mut app);
    assert_eq!(bs.len(), 1);
    let (lead, lead_id, b) = bs[0];
    assert_eq!(b.home, id);
    let world = app.world();
    assert!(world.get::<Raider>(lead).is_some());
    assert_eq!(
        world.get::<Captain>(lead).unwrap().faction,
        FactionId::RAIDERS
    );
    let men = members(&mut app, lead);
    println!(
        "band #{lead_id}: {} men, population {before} -> {}",
        men.len(),
        people(&app, s)
    );
    assert_eq!(men.len() as u32, want);
    for m in &men {
        let v = app.world().get::<Villager>(*m).expect("a villager");
        assert_eq!(v.settlement, s, "his own town's man");
    }
    assert_eq!(people(&app, s), before - want, "the town gave them up");
    assert!(app.world().resource::<Fallout>().pending.is_empty());
    let rose = events(&app, SimEventKind::BanditsRose);
    assert_eq!(rose.len(), 1);
    assert_eq!(rose[0].event.cause, Cause::Fallout { settlement: id });
    assert_eq!(rose[0].event.subject, lead_id);
    assert_eq!(rose[0].event.amount as u32, want);
    assert_eq!(bands_alive(app.world_mut()), 1);
}

/// GP-33: broken men go for the rival's towns first, until they have raided
/// `turn_after_raids` times.
#[test]
fn bandits_prefer_the_rival() {
    let mut app = run();
    let d = data(&app);
    {
        // Distance and garrisons aside, the owner's weight decides.
        let mut p = app.world_mut().resource_mut::<Progression>();
        p.data.fallout.distance_scale = 1e9;
        p.data.fallout.garrison_scale = 1e9;
    }
    let rivals: Vec<u64> = rival_towns(&mut app).iter().map(|t| t.1).collect();
    assert!(!rivals.is_empty(), "the rival holds towns besides its seat");
    let &(s, id, _, _) = free_towns(&mut app, 100).first().expect("a big town");
    conquer(&mut app, s, 8);
    while bands(&mut app).is_empty() {
        assert!(tick(&app) < 8 * hour(&app), "no band rose");
        app.step(hour(&app));
    }
    // Its first think (the tick it rose or the next).
    app.step(5);
    let (lead, _, b) = bands(&mut app)[0];
    let front = app
        .world()
        .get::<Captain>(lead)
        .unwrap()
        .orders
        .front()
        .map(|o| o.spec);
    println!("band of #{id}: front order {front:?}, raids {}", b.raids);
    let Some(OrderSpec::GatherFood { settlement }) = front else {
        panic!("a band forages the town it raids: {front:?}");
    };
    assert!(
        rivals.contains(&settlement),
        "a rival town (not #{settlement}); rival towns {rivals:?}"
    );
    assert_eq!(b.raids, 1);
    assert_ne!(settlement, id, "never its own home");

    // The weights: the rival 2.0 against the player's 1.0 at a first raid;
    // after the turn the rival's score is the player's.
    let at = |owner, raids| bandit_score(&d, owner, raids, 40.0, 0, 0.0);
    assert!(at(FactionId::RIVAL, 0) > at(P, 0));
    assert!(at(FactionId::RIVAL, 0) > at(FactionId::INDEPENDENT, 0));
    assert!(at(FactionId::RIVAL, d.turn_after_raids - 1) > at(P, 0));
    assert_eq!(at(FactionId::RIVAL, d.turn_after_raids), at(P, 0));
    // Garrisons and the player's influence protect.
    assert!(bandit_score(&d, FactionId::RIVAL, 0, 40.0, 10, 0.0) < at(FactionId::RIVAL, 0));
    assert!(bandit_score(&d, FactionId::RIVAL, 0, 40.0, 0, 0.5) < at(FactionId::RIVAL, 0));
    assert!(bandit_score(&d, FactionId::RIVAL, 0, 400.0, 0, 0.0) < at(FactionId::RIVAL, 0));
}

/// GP-33: only conquests and sacks of towns of `min_people` raise bands.
#[test]
fn peaceful_takes_raise_none() {
    let mut app = run();
    let d = data(&app);
    let towns = free_towns(&mut app, 100);
    assert!(towns.len() >= 3, "three big towns: {}", towns.len());
    // Blood or not, a submission and an invitation raise nobody.
    let (sub, inv, small) = (towns[0].0, towns[1].0, towns[2].0);
    spill(&mut app, sub, 12);
    transfer_seeded(
        app.world_mut(),
        sub,
        P,
        Cause::Submission { captain: 0 },
        Some(0.45),
    )
    .unwrap();
    spill(&mut app, inv, 12);
    transfer(app.world_mut(), inv, P, Cause::Invitation).unwrap();
    // A conquest of a town under the minimum: even a bloody one.
    app.world_mut()
        .get_mut::<SettlementEconomy>(small)
        .unwrap()
        .population = d.min_people - 1;
    spill(&mut app, small, 20);
    transfer(app.world_mut(), small, P, Cause::Conquest { captain: 0 }).unwrap();
    let f = app.world().resource::<Fallout>();
    println!(
        "pending {:?}, last_violent {:?}, vows {:?}",
        f.pending, f.last_violent, f.vows
    );
    assert!(f.pending.is_empty(), "no band queued");
    assert!(f.vows.is_empty(), "no vow: none were the rival's");
    app.step(6 * hour(&app));
    assert!(bands(&mut app).is_empty());
    assert!(events(&app, SimEventKind::BanditsRose).is_empty());
    // The rival's own take of a town (a retake) raises none either.
    let &(t, ..) = free_towns(&mut app, 100).first().expect("a fourth town");
    spill(&mut app, t, 12);
    transfer(
        app.world_mut(),
        t,
        FactionId::RIVAL,
        Cause::Conquest { captain: 0 },
    )
    .unwrap();
    assert!(app.world().resource::<Fallout>().pending.is_empty());
}

/// GP-33: three bloody takes: at most `max_bands` bands alive, the third
/// waits and its town broods once; it rises when a band is gone.
#[test]
fn the_cap_holds() {
    let mut app = run();
    let d = data(&app);
    let towns = free_towns(&mut app, 100);
    assert!(towns.len() >= 3, "three big towns: {}", towns.len());
    for t in &towns[..3] {
        conquer(&mut app, t.0, 10);
    }
    let third = (towns[2].0, towns[2].1);
    assert_eq!(app.world().resource::<Fallout>().pending.len(), 3);
    let scar0 = app.world().get::<Tenure>(third.0).unwrap().scar;
    let mut scars = vec![scar0];
    for _ in 0..8 {
        app.step(hour(&app));
        scars.push(app.world().get::<Tenure>(third.0).unwrap().scar);
        assert!(bands_alive(app.world_mut()) <= d.max_bands);
    }
    let alive = bands(&mut app);
    let pending = app.world().resource::<Fallout>().pending.clone();
    println!("bands {alive:?}, pending {pending:?}, third's scar {scars:?}");
    assert_eq!(alive.len() as u32, d.max_bands);
    assert_eq!(pending.len(), 1, "the third waits");
    assert_eq!(pending[0].settlement, third.1);
    assert!(pending[0].brooding);
    // The scar deepened by `brood_scar` once, never again (it only fades).
    let rises = scars.windows(2).filter(|w| w[1] > w[0]).count();
    let first = scars.windows(2).find(|w| w[1] > w[0]).unwrap();
    assert_eq!(rises, 1, "brooded once: {scars:?}");
    assert!((first[1] - first[0] - d.brood_scar).abs() < 1e-5);
    // Kill the first band to the last man: the third rises.
    let (lead, ..) = alive[0];
    for m in members(&mut app, lead) {
        app.world_mut().get_mut::<CombatStats>(m).unwrap().health = 0.0;
    }
    app.step(2 * hour(&app));
    let after = bands(&mut app);
    println!("after the killing: {after:?}");
    assert!(app.world().resource::<Fallout>().pending.is_empty());
    assert_eq!(after.len() as u32, d.max_bands);
    assert!(after.iter().any(|b| b.2.home == third.1), "the third rose");
    assert!(after.iter().all(|b| b.0 != lead));
}

/// GP-33: a fallen bandit captain's heir leads broken men still.
#[test]
fn heirs_keep_the_band() {
    let mut app = run();
    let &(s, id, _, _) = free_towns(&mut app, 100).first().expect("a big town");
    conquer(&mut app, s, 10);
    while bands(&mut app).is_empty() {
        assert!(tick(&app) < 8 * hour(&app), "no band rose");
        app.step(hour(&app));
    }
    let (lead, lead_id, b) = bands(&mut app)[0];
    assert!(members(&mut app, lead).len() >= 3);
    app.world_mut().get_mut::<CombatStats>(lead).unwrap().health = 0.0;
    app.step(4);
    let after = bands(&mut app);
    println!("{lead_id} fell: bands now {after:?}");
    assert_eq!(after.len(), 1, "the band goes on");
    let (heir, heir_id, hb) = after[0];
    assert_ne!(heir_id, lead_id);
    assert!(app.world().get_entity(lead).is_err());
    assert_eq!(hb.home, id);
    assert!(hb.raids >= b.raids, "the count goes with the band");
    assert!(app.world().get::<Raider>(heir).is_some(), "still a Raider");
    assert_eq!(
        app.world().get::<Captain>(heir).unwrap().faction,
        FactionId::RAIDERS
    );
}

/// An army of `faction` near `at`.
fn army(app: &mut App, faction: FactionId, at: IVec2, followers: u32, armed: bool) -> Entity {
    let at = passable_near(app, at).expect("a passable tile");
    spawn_army(
        app.world_mut(),
        &ArmySpec {
            posture: Posture::Aggressive,
            armed,
            food_days: 6.0,
            ..ArmySpec::new(faction, at, followers)
        },
    )
}

/// A battle beside player-owned town `s` between a strong player army and a
/// raider band, the band marked as `bandits_home`'s broken men or not;
/// returns once the player's battle report is in and the next progress step
/// has run.
fn fight_beside(app: &mut App, s: Entity, bandits_home: Option<u64>) {
    app.world_mut()
        .resource_mut::<Progression>()
        .data
        .requests
        .clear_hours = 1e6;
    let at = app.world().get::<Settlement>(s).unwrap().center;
    army(app, P, at + IVec2::new(12, 0), 30, true);
    let raiders = army(app, FactionId::RAIDERS, at + IVec2::new(16, 0), 6, false);
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
        let done = app.world().resource::<sim_core::Chronicle>().reports()[reports..]
            .iter()
            .any(|r| r.faction == P && r.kills > 0);
        if done {
            break;
        }
        assert!(tick(app) < start + 2 * 2400, "no battle report");
    }
    app.step(hour(app));
}

/// A town the player holds by invitation, away from the start band.
fn sworn_town(app: &mut App) -> (Entity, u64) {
    let band: Vec<IVec2> = {
        let world = app.world_mut();
        let mut q = world.query::<(&Captain, &sim_core::Position)>();
        q.iter(world)
            .filter(|(c, _)| c.faction == P)
            .map(|(_, p)| p.0)
            .collect()
    };
    let &(s, id, ..) = free_towns(app, 60)
        .iter()
        .find(|t| band.iter().all(|b| (*b - t.2).as_vec2().length() > 120.0))
        .expect("a town away from the band");
    transfer(app.world_mut(), s, P, Cause::Invitation).unwrap();
    (s, id)
}

/// GP-33: "you killed our sons in the woods": a player's blow that slays a
/// bandit is a Killing deed at his home, and the fight is no defence there.
#[test]
fn killing_its_bandits_costs_goodwill_at_their_home() {
    let mut app = run();
    let (s, id) = sworn_town(&mut app);
    fight_beside(&mut app, s, Some(id));
    let killing = deeds(&app, s, Deed::Killing);
    let g = app.world().get::<Goodwill>(s).unwrap().get(P);
    println!(
        "town #{id}: killing {killing:?}, defended {:?}, goodwill {g:.3}",
        deeds(&app, s, Deed::Defended)
    );
    assert!(!killing.is_empty(), "the town remembers its sons");
    assert!(killing.iter().all(|d| *d < 0.0));
    assert!(deeds(&app, s, Deed::Defended).is_empty());

    // Control: the same fight against strangers is a defence and no killing.
    let mut app = run();
    let (s, id) = sworn_town(&mut app);
    fight_beside(&mut app, s, None);
    println!(
        "control #{id}: killing {:?}, defended {:?}",
        deeds(&app, s, Deed::Killing),
        deeds(&app, s, Deed::Defended)
    );
    assert!(deeds(&app, s, Deed::Killing).is_empty());
    assert!(!deeds(&app, s, Deed::Defended).is_empty());
}

/// GP-33: a take from the rival vows a reprisal, due `vow_delay_hours`
/// later (once per cooldown); the band pins to the player's newest holding.
#[test]
fn a_vow_comes_due_at_the_newest_holding() {
    let mut app = run();
    let d = data(&app);
    let cal = app.world().resource::<Calendar>().clone();
    let raid_every;
    {
        // Reprisals from the first day, so the test waits hours, not days.
        let cfg = app.world().resource::<sim_core::RunState>().config.unwrap();
        let mut p = app.world_mut().resource_mut::<Progression>();
        let row = p
            .data
            .rival
            .iter_mut()
            .find(|r| r.difficulty == cfg.difficulty)
            .unwrap();
        row.reprisal_from_day = 0.0;
        raid_every = row.raid_every_days;
    }
    let lands = rival_towns(&mut app);
    assert!(
        lands.len() >= 2,
        "the rival holds two towns besides its seat"
    );
    let (a, a_id) = (lands[0].0, lands[0].1);
    let (b, b_id) = (lands[1].0, lands[1].1);
    transfer_seeded(
        app.world_mut(),
        a,
        P,
        Cause::Submission { captain: 0 },
        Some(0.45),
    )
    .unwrap();
    assert_eq!(app.world().resource::<Fallout>().vows.len(), 1);
    assert!(app.world().resource::<RivalState>().reprisal.is_none());
    app.step(hour(&app));
    let state = app.world().resource::<RivalState>().clone();
    let vow = state.reprisal.expect("the rival vowed");
    let made = state.last_reprisal.expect("the vow's tick");
    println!("vow {vow:?} made at {made}");
    assert_eq!(vow.settlement, a_id);
    assert_eq!(vow.due, made + cal.hours(d.vow_delay_hours));
    assert!(app.world().resource::<Fallout>().vows.is_empty());
    let vowed = events(&app, SimEventKind::ReprisalVowed);
    assert_eq!(vowed.len(), 1);
    assert_eq!(vowed[0].event.subject, a_id);
    assert_eq!(vowed[0].event.cause, Cause::Submission { captain: 0 });

    // A second take within the cooldown makes no new vow.
    app.step(2 * hour(&app));
    transfer_seeded(
        app.world_mut(),
        b,
        P,
        Cause::Submission { captain: 0 },
        Some(0.45),
    )
    .unwrap();
    app.step(hour(&app));
    assert_eq!(app.world().resource::<RivalState>().reprisal, Some(vow));
    assert_eq!(events(&app, SimEventKind::ReprisalVowed).len(), 1);

    // The newest holding is an independent town taken later.
    app.step(hour(&app));
    let &(c, c_id, ..) = free_towns(&mut app, 60).first().expect("a free town");
    transfer(app.world_mut(), c, P, Cause::Invitation).unwrap();
    let newest = sim_military::rival::newest_holding(app.world()).expect("holdings");
    assert_eq!(newest.0, c_id, "the last town taken is the newest");
    assert_ne!(newest.0, vow.settlement);

    // Until due nothing leaves; at the first clock run past `due` a band
    // splits off, pinned to the newest holding.
    let before = app.world().resource::<RivalState>().clone();
    assert_eq!(before.raids, 0);
    let mut last_next_raid = before.next_raid;
    while events(&app, SimEventKind::RivalReprisal).is_empty() {
        assert!(tick(&app) < vow.due + 3 * hour(&app), "no reprisal");
        assert!(tick(&app) < vow.due || app.world().resource::<RivalState>().raids == 0);
        last_next_raid = app.world().resource::<RivalState>().next_raid;
        app.step(hour(&app));
    }
    let launched = events(&app, SimEventKind::RivalReprisal);
    assert_eq!(launched.len(), 1);
    let ev = launched[0];
    let state = app.world().resource::<RivalState>().clone();
    println!(
        "reprisal at {} (due {}): {ev:?}; raids {}, next_raid {last_next_raid} -> {}",
        ev.tick, vow.due, state.raids, state.next_raid
    );
    assert!(ev.tick >= vow.due);
    assert_eq!(ev.event.cause, Cause::Reprisal { settlement: c_id });
    assert_eq!(ev.event.other, c_id);
    assert_eq!(state.raids, 1, "a reprisal is a raid in the books");
    assert!(state.reprisal.is_none(), "the vow is kept");
    let world = app.world_mut();
    let mut q = world.query::<(&sim_core::SpawnId, &RivalBand, &Captain)>();
    let bands: Vec<_> = q
        .iter(world)
        .map(|(id, b, c)| (id.0, *b, c.faction))
        .collect();
    assert_eq!(bands.len(), 1);
    assert_eq!(bands[0].0, ev.event.subject);
    assert_eq!(bands[0].1.pinned, Some(c_id));
    assert_eq!(bands[0].2, FactionId::RIVAL);
    assert_eq!(
        state.next_raid,
        last_next_raid + cal.days(raid_every).max(1) / 2,
        "the next raid is pushed back half a period"
    );
    let _ = b_id;
}
