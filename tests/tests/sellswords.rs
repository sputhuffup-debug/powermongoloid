//! GP-27 scenario `sellswords`: the Sellswords run modifier camps a band
//! for hire near the start, and the player hires it, sends it away, or
//! lets the offer lapse.
//!
//! The tests play the run of seed 5, whose modifiers are "Bumper harvest"
//! and "Sellswords" and whose band ("Kin and neighbours") is twelve
//! followers and a champion with three days of food, driving it only
//! through player commands, as the UI would:
//!
//! - `a_band_for_hire_waits_near_the_start`: twelve armed men of the
//!   independents camp about `hire.distance` tiles from the start under a
//!   `Hire` offer (twelve men, 11 food, a day to answer); they hold their
//!   camp unfed and at peace with everyone. Seed 42 has no such band.
//! - `hiring_pays_and_the_band_follows_the_champion`: hiring pays the price
//!   from the champion's baggage and the twelve join the champion's army,
//!   eat from it and catch up with it; no captain is made.
//! - `a_hire_waits_until_the_champion_can_pay_and_is_near`: a hire with an
//!   empty baggage, or from too far, is refused and the offer stands.
//! - `declined_or_lapsed_the_band_moves_on`: declined, or unanswered past
//!   its expiry, the band leaves the world.
//! - `a_hire_replays_and_survives_a_save`: a hire replays in parallel and
//!   serially, and a game saved with the offer pending hires the same way.
//! - `the_bot_hires_a_band_it_can_feed`: the scripted player hires it at
//!   the start, when it can feed everyone half a day after paying.

mod common;

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use common::*;
use sim_agents::{CombatStats, Needs, Stance, Villagers};
use sim_core::{
    Allegiance, Calendar, CommandKind, FactionId, LoggedCommand, Offer, OfferKind, Offers,
    Position, RunConfig, SimCommand, SimEventKind, SimStepper, SimTick, SpawnId,
    compute_state_hash,
};
use sim_economy::{Economy, Stockpile};
use sim_headless::{HeadlessConfig, Threading, load_app, record_replay, save_app, verify_replay};
use sim_military::progression::Effect;
use sim_military::{
    Army, ArmyStats, Captain, Equipment, Follower, MilitaryLog, NoUpkeep, Progression, RunProgress,
    describe_entries,
};

/// The run whose modifiers include Sellswords.
const SEED: u64 = 5;
/// Men in the band (army.ron's `Sellswords(12)` card).
const MEN: u16 = 12;
/// The hire's price: 12 men x 0.9 food a day x 1 day, rounded up.
const PRICE: u16 = 11;

fn tick(app: &App) -> u64 {
    app.world().resource::<SimTick>().0
}

/// The pending Hire offer.
fn hire_offer(app: &App) -> Option<Offer> {
    app.world()
        .resource::<Offers>()
        .pending
        .iter()
        .find(|o| matches!(o.kind, OfferKind::Hire { .. }))
        .cloned()
}

/// The SpawnId of the band's captain named by `offer`.
fn band_captain(offer: &Offer) -> u64 {
    match offer.kind {
        OfferKind::Hire { captain, .. } => captain,
        _ => panic!("not a hire: {offer:?}"),
    }
}

fn entity(app: &mut App, id: u64) -> Option<Entity> {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &SpawnId)>();
    q.iter(world).find(|(_, s)| s.0 == id).map(|(e, _)| e)
}

/// The band led by captain `id`: the captain, then its followers by
/// SpawnId.
fn band(app: &mut App, id: u64) -> Vec<Entity> {
    let captain = entity(app, id).expect("the band's captain");
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &SpawnId, &Follower)>();
    let mut f: Vec<(SpawnId, Entity)> = q
        .iter(world)
        .filter(|(_, _, f)| f.captain == captain)
        .map(|(e, id, _)| (*id, e))
        .collect();
    f.sort_unstable();
    std::iter::once(captain)
        .chain(f.into_iter().map(|(_, e)| e))
        .collect()
}

fn tile(app: &App, e: Entity) -> IVec2 {
    app.world().get::<Position>(e).expect("a position").0
}

fn distance(app: &App, a: Entity, b: Entity) -> f32 {
    (tile(app, a) - tile(app, b)).as_vec2().length()
}

fn hunger(app: &App, e: Entity) -> f32 {
    app.world().get::<Needs>(e).expect("needs").hunger
}

fn food(app: &App, captain: Entity) -> f32 {
    let world = app.world();
    let econ = world.resource::<Economy>();
    world
        .get::<Stockpile>(captain)
        .map_or(0.0, |s| s.food_value(econ))
}

/// Faction-0 (independent) captains.
fn independent_captains(app: &mut App) -> usize {
    let world = app.world_mut();
    let mut q = world.query::<&Captain>();
    q.iter(world)
        .filter(|c| c.faction == FactionId::INDEPENDENT)
        .count()
}

fn player_captains(app: &mut App) -> usize {
    let world = app.world_mut();
    let mut q = world.query::<&Captain>();
    q.iter(world)
        .filter(|c| c.faction == FactionId::PLAYER)
        .count()
}

fn log_has(app: &App, text: &str) -> bool {
    app.world()
        .resource::<MilitaryLog>()
        .entries
        .iter()
        .any(|(_, l)| l.contains(text))
}

/// The run of `seed` before its first tick, with its first stat-only start
/// boon chosen (applied on the first tick), so the baggage holds exactly
/// the template's food. Returns the app and the champion.
fn start_with(seed: u64) -> (App, Entity) {
    let mut app = run_app(seed);
    let champion = champion(&mut app);
    let offer = start_offer(&app);
    let pick = stat_only_pick(&app);
    choose(&mut app, offer, pick);
    (app, champion)
}

fn start() -> (App, Entity) {
    start_with(SEED)
}

/// The index of the first start boon on offer whose effects are all stat
/// bonuses.
fn stat_only_pick(app: &App) -> u8 {
    let data = &app.world().resource::<Progression>().data;
    let cards = &app
        .world()
        .resource::<Offers>()
        .get(start_offer(app))
        .expect("the start offer")
        .choices;
    cards
        .iter()
        .position(|&c| {
            data.boons[c as usize]
                .effects
                .iter()
                .all(|e| matches!(e, Effect::Stat(..)))
        })
        .expect("a stat-only boon on offer") as u8
}

/// The champion and its army's people stop being hungry, so nobody eats
/// from the baggage on the next tick (the hire's price is then exact).
fn sate(app: &mut App, champion: Entity) {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &mut Needs, Option<&Follower>)>();
    for (e, mut n, f) in q.iter_mut(world) {
        if e == champion || f.is_some_and(|f| f.captain == champion) {
            n.hunger = 0.0;
        }
    }
}

/// GP-27: the band waits near the start under its offer, unfed, at peace
/// and holding its camp; seed 42 has none.
#[test]
fn a_band_for_hire_waits_near_the_start() {
    let (mut app, _) = start();
    let world = app.world();
    let data = world.resource::<Progression>().data.clone();
    let progress = world.resource::<RunProgress>().clone();
    let names: Vec<&str> = progress
        .modifiers
        .iter()
        .map(|&m| data.modifiers[m as usize].name.as_str())
        .collect();
    assert!(names.contains(&"Sellswords"), "seed {SEED} draws {names:?}");
    let hires: Vec<Offer> = world
        .resource::<Offers>()
        .pending
        .iter()
        .filter(|o| matches!(o.kind, OfferKind::Hire { .. }))
        .cloned()
        .collect();
    assert_eq!(hires.len(), 1, "one band for hire: {hires:?}");
    let offer = hires[0].clone();
    let per_day = world.resource::<Villagers>().data.needs.hunger_per_day;
    assert_eq!(
        PRICE,
        (MEN as f32 * per_day * data.hire.price_days).ceil() as u16
    );
    let id = band_captain(&offer);
    assert_eq!(
        offer.kind,
        OfferKind::Hire {
            captain: id,
            men: MEN,
            price: PRICE
        }
    );
    assert_eq!(offer.faction, FactionId::PLAYER);
    assert_eq!(offer.choices, vec![0, 1]);
    let hours = world.resource::<Calendar>().hours(data.hire.expiry_hours);
    assert_eq!(offer.expires, Some(hours));

    let members = band(&mut app, id);
    let captain = members[0];
    let world = app.world();
    let c = world.get::<Captain>(captain).expect("a captain");
    assert_eq!(c.faction, FactionId::INDEPENDENT);
    assert!(c.orders.is_empty(), "no orders: it holds its camp");
    let camp = tile(&app, captain);
    let from_start = (camp - progress.start).as_vec2().length();
    println!(
        "seed {SEED}: {} men camp at ({}, {}), {from_start:.1} tiles from the start; offer #{} ({} food, lapses at tick {hours})",
        members.len(),
        camp.x,
        camp.y,
        offer.id,
        PRICE
    );
    assert!(
        from_start <= data.hire.distance + 6.0,
        "{from_start:.1} tiles from the start"
    );
    // Not dropped on the start: clear of the champion's formation (its
    // band is twelve followers too).
    let clear = app.world().resource::<Army>().formation_radius(MEN as u32);
    assert!(
        from_start > clear,
        "{from_start:.1} tiles from the start, inside the champion's formation ({clear:.1})"
    );
    assert_eq!(members.len(), MEN as usize);
    let u = data.hire.unit;
    let stats = CombatStats {
        health: u.health,
        max_health: u.health,
        attack: u.attack,
        defense: u.defense,
        morale: u.morale,
        stance: Stance::Soldier,
    };
    for &m in &members {
        let w = app.world();
        assert_eq!(w.get::<Allegiance>(m).unwrap().0, FactionId::INDEPENDENT);
        assert!(w.get::<NoUpkeep>(m).is_some(), "never eats");
        assert!(w.get::<Equipment>(m).unwrap().weapon, "armed");
        assert_eq!(*w.get::<CombatStats>(m).unwrap(), stats);
    }
    let hungers: Vec<f32> = members.iter().map(|&m| hunger(&app, m)).collect();

    app.step(600);
    assert_eq!(band(&mut app, id), members, "the same men");
    for (i, &m) in members.iter().enumerate() {
        let d = (tile(&app, m) - camp).as_vec2().length();
        assert!(d <= 6.0, "member {i} is {d:.1} tiles from the camp");
        assert_eq!(hunger(&app, m), hungers[i], "member {i} never hungers");
    }
    assert!(hire_offer(&app).is_some(), "the offer stands");
    let world = app.world();
    let stats = world.resource::<ArmyStats>().get(FactionId::INDEPENDENT);
    assert_eq!((stats.kills, stats.lost), (0, 0), "nobody fought them");
    assert!(
        world
            .get::<Captain>(captain)
            .unwrap()
            .battle_since
            .is_none(),
        "never in a battle"
    );
    assert!(
        !chronicle(&app)
            .iter()
            .any(|e| e.event.kind == SimEventKind::Battle
                && e.event.faction == FactionId::INDEPENDENT),
        "no battle"
    );

    let mut other = run_app(42);
    assert!(hire_offer(&other).is_none(), "seed 42 draws no Sellswords");
    assert_eq!(independent_captains(&mut other), 0);
}

/// GP-27: the hire pays the price from the baggage, and the twelve join
/// the champion's army: sworn to the player, eating, catching up. No
/// captain slot is used.
#[test]
fn hiring_pays_and_the_band_follows_the_champion() {
    let (mut app, champion) = start();
    app.step(1);
    let offer = hire_offer(&app).expect("the band for hire");
    let id = band_captain(&offer);
    let members = band(&mut app, id);
    let before_followers = followers(&mut app, champion).len();
    let before_food = food(&app, champion);
    let before_capacity = app.world().get::<Stockpile>(champion).unwrap().capacity;
    let before_recruited = app
        .world()
        .resource::<ArmyStats>()
        .get(FactionId::PLAYER)
        .recruited;
    let slots = app.world().resource::<RunProgress>().captain_slots;
    let captains = player_captains(&mut app);
    let hungers: Vec<f32> = members.iter().map(|&m| hunger(&app, m)).collect();
    assert!(
        distance(&app, champion, members[0])
            <= app.world().resource::<Progression>().data.hire.radius
    );
    sate(&mut app, champion);
    choose(&mut app, offer.id, 0);
    app.step(1);

    assert!(hire_offer(&app).is_none(), "the offer is taken");
    assert_eq!(rejected(&app, FactionId::PLAYER), 0);
    for &m in &members {
        let w = app.world();
        assert_eq!(w.get::<Allegiance>(m).unwrap().0, FactionId::PLAYER);
        assert_eq!(w.get::<Follower>(m).unwrap().captain, champion);
        assert!(w.get::<NoUpkeep>(m).is_none(), "they eat now");
    }
    let w = app.world();
    assert!(w.get::<Captain>(members[0]).is_none(), "no captain made");
    assert!(w.get::<Stockpile>(members[0]).is_none());
    assert_eq!(independent_captains(&mut app), 0);
    assert_eq!(player_captains(&mut app), captains, "captains unchanged");
    assert_eq!(app.world().resource::<RunProgress>().captain_slots, slots);
    assert_eq!(
        followers(&mut app, champion).len(),
        before_followers + MEN as usize
    );
    let paid = before_food - food(&app, champion);
    println!("paid {paid:.2} food value for {MEN} sellswords (price {PRICE})");
    assert!(
        paid >= PRICE as f32 - 1e-3 && paid < PRICE as f32 + 0.45,
        "paid {paid}"
    );
    let per_head = app.world().resource::<Army>().data.baggage_per_head;
    assert_eq!(
        app.world().get::<Stockpile>(champion).unwrap().capacity,
        before_capacity + per_head * MEN as u32
    );
    assert_eq!(
        app.world()
            .resource::<ArmyStats>()
            .get(FactionId::PLAYER)
            .recruited,
        before_recruited + MEN as u32
    );
    let entries = chronicle(&app);
    let chosen = entries
        .iter()
        .find(|e| e.event.kind == SimEventKind::OfferChosen && e.event.subject == offer.id)
        .expect("the hire is in the chronicle");
    assert_eq!((chosen.event.amount, chosen.event.other), (4, 0));
    let text = describe_entries(app.world(), std::slice::from_ref(chosen)).remove(0);
    assert!(text.contains("hired a band of sellswords"), "{text}");

    // Fed from the baggage now: hungrier by the hour (none reaches
    // villagers.ron's `hungry` 0.4 within 100 ticks, so none eats yet).
    app.step(100);
    for (i, &m) in members.iter().enumerate() {
        assert!(hunger(&app, m) > hungers[i], "member {i} gets hungry");
    }
    app.step(200);
    let army = app.world().resource::<Army>().clone();
    let reach = army.formation_radius(followers(&mut app, champion).len() as u32)
        + army.data.formation.catch_up;
    let worst = members
        .iter()
        .map(|&m| distance(&app, champion, m))
        .fold(0.0, f32::max);
    println!(
        "300 ticks after the hire, the farthest sellsword is {worst:.1} tiles from the champion (reach {reach:.1})"
    );
    assert!(worst <= reach, "{worst:.1} > {reach:.1}");
    for &m in &members {
        assert_eq!(app.world().get::<Follower>(m).unwrap().captain, champion);
    }
}

/// GP-27: a hire the champion can't pay for, or makes from too far, is
/// refused and the offer and band stand; once both are right it goes
/// through, and a second pick finds no offer.
#[test]
fn a_hire_waits_until_the_champion_can_pay_and_is_near() {
    let (mut app, champion) = start();
    app.step(1);
    let offer = hire_offer(&app).expect("the band for hire");
    let id = band_captain(&offer);
    let members = band(&mut app, id);
    let econ = app.world().resource::<Economy>().clone();

    // An empty baggage.
    let bag = app.world().get::<Stockpile>(champion).unwrap().clone();
    app.world_mut()
        .get_mut::<Stockpile>(champion)
        .unwrap()
        .goods
        .retain(|g, _| !econ.is_food(*g));
    choose(&mut app, offer.id, 0);
    app.step(1);
    assert_eq!(rejected(&app, FactionId::PLAYER), 1);
    assert!(log_has(&app, "too little food to pay the sellswords"));
    assert_eq!(hire_offer(&app), Some(offer.clone()), "the offer stands");
    assert_eq!(band(&mut app, id), members, "the band stands");
    assert!(app.world().get::<NoUpkeep>(members[0]).is_some());

    // Food back, but the champion out of reach.
    *app.world_mut().get_mut::<Stockpile>(champion).unwrap() = bag;
    let away = distance(&app, champion, members[0]);
    let radius = app.world().resource::<Progression>().data.hire.radius;
    app.world_mut()
        .resource_mut::<Progression>()
        .data
        .hire
        .radius = away - 1.0;
    choose(&mut app, offer.id, 0);
    app.step(1);
    assert_eq!(rejected(&app, FactionId::PLAYER), 2);
    assert!(log_has(&app, "too far from the sellswords"));
    assert_eq!(hire_offer(&app), Some(offer.clone()));

    // Both right: hired.
    app.world_mut()
        .resource_mut::<Progression>()
        .data
        .hire
        .radius = radius;
    sate(&mut app, champion);
    choose(&mut app, offer.id, 0);
    app.step(1);
    assert_eq!(rejected(&app, FactionId::PLAYER), 2);
    assert!(hire_offer(&app).is_none());
    assert!(
        members
            .iter()
            .all(|&m| app.world().get::<Follower>(m).unwrap().captain == champion)
    );
    choose(&mut app, offer.id, 0);
    app.step(1);
    assert_eq!(rejected(&app, FactionId::PLAYER), 3, "no such offer");
}

/// GP-27: declined, the band leaves at once; unanswered past its expiry,
/// it moves on, and a pick on the expiry tick is refused.
#[test]
fn declined_or_lapsed_the_band_moves_on() {
    let ids = |app: &mut App, members: &[Entity]| -> Vec<u64> {
        members
            .iter()
            .map(|&m| app.world().get::<SpawnId>(m).unwrap().0)
            .collect()
    };
    let gone = |app: &mut App, ids: &[u64]| ids.iter().all(|&i| entity(app, i).is_none());

    let (mut app, _) = start();
    app.step(1);
    let offer = hire_offer(&app).expect("the band for hire");
    let members = band(&mut app, band_captain(&offer));
    let declined = ids(&mut app, &members);
    choose(&mut app, offer.id, 1);
    app.step(1);
    assert!(gone(&mut app, &declined), "sent away");
    assert!(hire_offer(&app).is_none());
    assert_eq!(independent_captains(&mut app), 0);
    let entries = chronicle(&app);
    let chosen = entries
        .iter()
        .find(|e| e.event.kind == SimEventKind::OfferChosen && e.event.subject == offer.id)
        .expect("the answer is in the chronicle");
    assert_eq!(chosen.event.other, 1);
    let text = describe_entries(app.world(), std::slice::from_ref(chosen)).remove(0);
    assert!(text.contains("sent the sellswords away"), "{text}");

    // Lapsed: the offer expires at tick 200 (an hour boundary).
    let (mut app, _) = start();
    for o in &mut app.world_mut().resource_mut::<Offers>().pending {
        if matches!(o.kind, OfferKind::Hire { .. }) {
            o.expires = Some(200);
        }
    }
    let offer = hire_offer(&app).expect("the band for hire");
    let members = band(&mut app, band_captain(&offer));
    let lapsing = ids(&mut app, &members);
    app.step(200);
    assert_eq!(tick(&app), 200);
    assert!(!gone(&mut app, &lapsing), "still waiting at tick 199");
    choose(&mut app, offer.id, 0);
    app.step(1);
    assert_eq!(rejected(&app, FactionId::PLAYER), 1, "too late");
    assert!(gone(&mut app, &lapsing), "moved on");
    assert!(hire_offer(&app).is_none());
    assert!(log_has(&app, "moved on"));
    assert_eq!(independent_captains(&mut app), 0);
}

/// GP-27, NFR-08, GP-12: a hire replays in parallel and serially, and a
/// game saved with the offer pending hires the same way after loading.
#[test]
fn a_hire_replays_and_survives_a_save() {
    let config = HeadlessConfig::run(SEED, RunConfig::default());
    let (probe, _) = start();
    let offer = hire_offer(&probe).expect("the band for hire");
    let pick = sim_headless::start_boon_pick(&probe, stat_only_pick(&probe)).expect("a pick");
    let hire = LoggedCommand {
        tick: 1,
        command: SimCommand::player(
            FactionId::PLAYER,
            CommandKind::ChooseOffer {
                offer: offer.id,
                choice: 0,
            },
        ),
    };
    let replay = record_replay(config, 1_200, &[pick, hire], 300);
    verify_replay(&replay, Threading::Multi).expect("a hire replays");
    verify_replay(&replay, Threading::Single).expect("a hire replays serially");

    let (mut app, _) = start();
    app.step(50);
    assert!(hire_offer(&app).is_some(), "pending at the save");
    let members = band(&mut app, band_captain(&offer));
    let ids: Vec<u64> = members
        .iter()
        .map(|&m| app.world().get::<SpawnId>(m).unwrap().0)
        .collect();
    let bytes = save_app(&mut app);
    let mut loaded = load_app(&bytes, Threading::Multi).expect("the save loads");
    assert_eq!(
        compute_state_hash(loaded.world_mut()),
        compute_state_hash(app.world_mut()),
        "loaded hash"
    );
    for a in [&mut app, &mut loaded] {
        choose(a, offer.id, 0);
    }
    while tick(&app) < 600 {
        app.step(50);
        loaded.step(50);
        assert_eq!(
            compute_state_hash(loaded.world_mut()),
            compute_state_hash(app.world_mut()),
            "tick {}",
            tick(&app)
        );
    }
    for a in [&mut app, &mut loaded] {
        let champion = champion(a);
        for &id in &ids {
            let m = entity(a, id).expect("a sellsword");
            let w = a.world();
            assert_eq!(w.get::<Follower>(m).unwrap().captain, champion);
            assert_eq!(w.get::<Allegiance>(m).unwrap().0, FactionId::PLAYER);
        }
    }
}

/// GP-27, GP-16: the scripted player hires the band on the first tick, as
/// its baggage feeds everyone half a day after paying.
#[test]
fn the_bot_hires_a_band_it_can_feed() {
    // The band's SpawnIds, from the same run before its first tick.
    let ids: Vec<u64> = {
        let mut fresh = run_app(SEED);
        let offer = hire_offer(&fresh).expect("a band for hire");
        band(&mut fresh, band_captain(&offer))
            .into_iter()
            .map(|e| fresh.world().get::<SpawnId>(e).unwrap().0)
            .collect()
    };
    assert_eq!(ids.len(), MEN as usize);
    let config = HeadlessConfig::run(SEED, RunConfig::default());
    let (mut app, run) = sim_headless::bot::play_bot_with(config, 200, 0, |_| {});
    assert!(hire_offer(&app).is_none(), "answered");
    let hired = run
        .chronicle
        .iter()
        .find(|e| e.event.kind == SimEventKind::OfferChosen && e.event.amount == 4)
        .expect("the bot answered the hire");
    assert_eq!((hired.tick, hired.event.other), (0, 0), "hired at once");
    assert_eq!(independent_captains(&mut app), 0);
    let champion = champion(&mut app);
    for id in ids {
        let m = entity(&mut app, id).expect("a sellsword");
        let w = app.world();
        assert_eq!(
            w.get::<Follower>(m).map(|f| f.captain),
            Some(champion),
            "sellsword {id} follows the champion"
        );
        assert_eq!(w.get::<Allegiance>(m).unwrap().0, FactionId::PLAYER);
    }
}
