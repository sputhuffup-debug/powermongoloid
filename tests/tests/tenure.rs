//! M4a step 1 scenario `tenure` (GP-32, SIM-60, SIM-61; ADR 0020): every
//! town in a run explains itself. A change of hands leaves a tenure (how,
//! by whom, when, its scar), the loyalty target is itemised in terms, the
//! dated deeds behind a town's goodwill are logged, and every cause and
//! event kind the milestone adds is mapped, classified and worded.
//!
//! - `transfer_writes_tenure`: each cause leaves its `TakenBy`, owner, tick
//!   and scar; militia killed add blood; the hourly step writes the
//!   Resentment term and the day tick fades the scar by 0.02.
//! - `every_cause_maps_to_its_kind_and_counters`: a sack is a conquest, a
//!   submission and a subversion are takes with no `conquests` count, a
//!   defiance reads like a revolt; the run summary counts them.
//! - `every_event_kind_is_classified`: all 34 kinds: chronicled, quiet,
//!   notable, and recorded accordingly.
//! - `the_log_dates_and_merges`: the deed log through the real ledger.
//! - `terms_sum_into_the_target`: each term moves the target by exactly its
//!   value, in the real schedule too.
//! - `a_term_is_named_when_it_decides`: the term that holds a town above the
//!   revolt line (or tips it under), and the largest pulls on a target.
//! - `every_cause_is_worded`: every cause has words in the chronicle.
//!
//! Each test prints the numbers it checks.

mod common;

use bevy_ecs::prelude::*;
use common::*;
use sim_agents::loyalty::{
    Pull, deciding_term, drift_toward, loyalty_parts, loyalty_pulls, loyalty_target,
};
use sim_agents::{
    Deed, DeedLog, Enlisted, FoodSecurity, Goodwill, GoodwillLedger, Loyalty, LoyaltyTerm,
    LoyaltyTerms, Owner, Villager, Villagers,
};
use sim_core::Posture;
use sim_core::{
    Calendar, Cause, ChronicleEntry, EventFeed, FactionId, FactionModifiers, SimEvent,
    SimEventKind, SimStepper, SimTick, SpawnId, Stat,
};
use sim_military::progression::ChampionRank;
use sim_military::{
    ArmySpec, ArmyStats, Casualties, MilitaryLog, Progression, RunProgress, TakenBy, Tenure,
    provoke, run_summary, spawn_army, taking_kind, transfer,
};
use sim_world::Settlement;

const SEED: u64 = 42;

/// A town nobody holds and that isn't the rival's seat (which ends the run
/// when taken): (entity, SpawnId). The first by SpawnId.
fn free_town(app: &mut App) -> (Entity, u64) {
    let seat = app.world().resource::<RunProgress>().rival_seat;
    let towns = villages(app);
    towns
        .into_iter()
        .find(|(e, id, ..)| Some(*id) != seat && owner_of(app, *e) == FactionId::INDEPENDENT)
        .map(|(e, id, ..)| (e, id))
        .expect("an independent town on the run island")
}

fn tenure(app: &App, s: Entity) -> Tenure {
    app.world()
        .get::<Tenure>(s)
        .expect("a run settlement has a tenure")
        .clone()
}

fn terms(app: &App, s: Entity) -> LoyaltyTerms {
    *app.world()
        .get::<LoyaltyTerms>(s)
        .expect("a run settlement has loyalty terms")
}

fn tick(app: &App) -> u64 {
    app.world().resource::<SimTick>().0
}

fn ticks_per_day(app: &App) -> u64 {
    app.world().resource::<Calendar>().ticks_per_day
}

/// The chronicle's newest entry about settlement `id`.
fn last_entry_about(app: &App, id: u64) -> ChronicleEntry {
    chronicle(app)
        .into_iter()
        .rev()
        .find(|e| e.event.subject == id)
        .expect("a chronicled entry about the settlement")
}

/// SIM-60: every cause leaves its tenure. A militiaman's death in the
/// town's defence adds a drop of blood; a taking reads the blood for its
/// scar (Conquered 0.12 + 0.01 a kill, at most 0.3; Sacked 0.25 + 0.01 a
/// kill, at most 0.45; Subverted 0.1; the other takings none), and the
/// blood starts again. The hourly `tenure::step` writes the scar as the
/// Resentment term, and the day tick fades it by 0.02.
#[test]
fn transfer_writes_tenure() {
    let mut app = run_app(SEED);
    quiet_director(&mut app);
    let (s, id) = free_town(&mut app);
    let data = app.world().resource::<Progression>().data.tenure;
    let t0 = tenure(&app, s);
    println!("town #{id}: {:?} by {:?} since {}", t0.how, t0.by, t0.since);
    assert_eq!(
        (t0.how, t0.by, t0.since, t0.scar, t0.blood),
        (TakenBy::Native, FactionId::INDEPENDENT, 0, 0.0, 0)
    );
    assert_eq!(t0.history.len(), 1);
    assert_eq!(terms(&app, s), LoyaltyTerms::default(), "no pull yet");

    // Blood: an aggressive band at the gate provokes the militia, and each
    // militiaman who falls adds a drop (the casualties step counts them).
    let at = {
        let centre = app.world().get::<Settlement>(s).unwrap().center;
        passable_near(&app, centre).expect("somewhere to stand")
    };
    spawn_army(
        app.world_mut(),
        &ArmySpec {
            posture: Posture::Aggressive,
            ..ArmySpec::new(FactionId::PLAYER, at, 40)
        },
    );
    provoke(app.world_mut(), s, FactionId::PLAYER);
    let killed = |app: &App| {
        app.world()
            .resource::<ArmyStats>()
            .get(FactionId::PLAYER)
            .villagers_killed
    };
    while killed(&app) < 3 {
        assert!(tick(&app) < 2_000, "no militia fell in 2000 ticks");
        app.step(10);
    }
    // Every death is the town's own: it is the only town provoked.
    let blood = tenure(&app, s).blood;
    println!("  {} militia fell; blood {blood}", killed(&app));
    assert_eq!(blood, killed(&app), "blood counts the militia killed");
    assert!(app.world().resource::<Casualties>().pending.is_empty());

    // The taking reads the blood.
    let p = FactionId::PLAYER;
    transfer(app.world_mut(), s, p, Cause::Conquest { captain: 1 }).expect("populated");
    let t = tenure(&app, s);
    let since = app.world().get::<Owner>(s).unwrap().since;
    println!("  conquered: scar {} (blood reset to {})", t.scar, t.blood);
    assert_eq!((t.how, t.by, t.since), (TakenBy::Conquered, p, since));
    assert_eq!(t.scar, data.scar(TakenBy::Conquered, blood));
    assert!(
        t.scar >= data.conquest_scar + 3.0 * data.scar_per_kill - 1e-6,
        "at least three drops: {}",
        t.scar
    );
    assert_eq!(t.blood, 0, "the blood starts again");

    // The hourly step writes the scar as the Resentment term ...
    let hour = app.world().resource::<Calendar>().ticks_per_hour();
    app.step(hour + 1);
    let resentment = terms(&app, s).get(LoyaltyTerm::Resentment);
    println!("  an hour on: Resentment {resentment}");
    assert_eq!(resentment, -t.scar);
    assert_eq!(terms(&app, s).get(LoyaltyTerm::Stirred), 0.0);

    // ... and the day tick fades the scar by 0.02, term and all.
    let day = ticks_per_day(&app);
    app.step(day + 1 - tick(&app) % day);
    let faded = tenure(&app, s).scar;
    println!("  past the day tick (tick {}): scar {faded}", tick(&app));
    assert!((faded - (t.scar - data.scar_fade_per_day)).abs() < 1e-6);
    assert_eq!(terms(&app, s).get(LoyaltyTerm::Resentment), -faded);

    // Every cause, with blood to read; the ring keeps the last four.
    let cases = [
        (Cause::Revolt, FactionId::INDEPENDENT, TakenBy::Freed, 0),
        (Cause::Sack { captain: 1 }, p, TakenBy::Sacked, 20),
        (Cause::Defiance, FactionId::INDEPENDENT, TakenBy::Freed, 2),
        (Cause::Submission { captain: 1 }, p, TakenBy::Submitted, 5),
        (Cause::Invitation, p, TakenBy::Invited, 0),
        (Cause::Subversion, p, TakenBy::Subverted, 4),
        (Cause::None, FactionId::RIVAL, TakenBy::Conquered, 30),
    ];
    let mut taken = vec![(TakenBy::Native, FactionId::INDEPENDENT), (t.how, t.by)];
    for (cause, to, how, blood) in cases {
        app.step(1);
        app.world_mut().get_mut::<Tenure>(s).unwrap().blood = blood;
        transfer(app.world_mut(), s, to, cause).expect("populated");
        let t = tenure(&app, s);
        let owner = *app.world().get::<Owner>(s).unwrap();
        assert_eq!((t.how, t.by, t.since), (how, to, owner.since), "{cause:?}");
        assert_eq!(t.scar, data.scar(how, blood), "{cause:?}");
        assert_eq!(t.blood, 0, "{cause:?}: blood starts again");
        taken.push((how, to));
        let recent: Vec<_> = t.history.iter().map(|h| (h.how, h.by)).collect();
        let tail = &taken[taken.len().saturating_sub(Tenure::HISTORY)..];
        assert_eq!(recent, tail, "{cause:?}: the ring keeps the last four");
        println!("  {cause:?}: {how:?}, scar {}", t.scar);
    }
    assert_eq!(data.scar(TakenBy::Conquered, 40), data.max_conquest_scar);
    assert_eq!(data.scar(TakenBy::Sacked, 30), data.max_sack_scar);
    assert_eq!(data.scar(TakenBy::Subverted, 9), data.subverted_scar);
    assert_eq!(data.scar(TakenBy::Invited, 9), 0.0);
    assert_eq!(data.scar(TakenBy::Submitted, 9), 0.0);
}

/// GP-32 (LORDSHIP_DESIGN section 11): every cause maps to its event kind,
/// faction and other, its `conquests` count and its champion credit; the
/// run summary counts the takes and the losses; the military log words them.
#[test]
fn every_cause_maps_to_its_kind_and_counters() {
    let mut app = run_app(SEED);
    quiet_director(&mut app);
    let champion = champion(&mut app);
    let (s, id) = free_town(&mut app);
    let (pl, ind) = (FactionId::PLAYER, FactionId::INDEPENDENT);
    let captain = 1;
    // (cause, to, kind, event faction, event other, conquests +, credit)
    let cases = [
        (
            Cause::Conquest { captain },
            pl,
            SimEventKind::Conquest,
            pl,
            ind,
            1,
            true,
        ),
        (Cause::Revolt, ind, SimEventKind::Revolt, pl, ind, 0, false),
        (
            Cause::Sack { captain },
            pl,
            SimEventKind::Conquest,
            pl,
            ind,
            1,
            true,
        ),
        (
            Cause::Defiance,
            ind,
            SimEventKind::Defied,
            pl,
            ind,
            0,
            false,
        ),
        (
            Cause::Invitation,
            pl,
            SimEventKind::Invited,
            pl,
            ind,
            0,
            true,
        ),
        (Cause::Revolt, ind, SimEventKind::Revolt, pl, ind, 0, false),
        (
            Cause::Submission { captain },
            pl,
            SimEventKind::Submitted,
            pl,
            ind,
            0,
            true,
        ),
        (Cause::Revolt, ind, SimEventKind::Revolt, pl, ind, 0, false),
        (
            Cause::Subversion,
            pl,
            SimEventKind::Subverted,
            pl,
            ind,
            0,
            true,
        ),
    ];
    for (cause, to, kind, faction, other, conquests, credit) in cases {
        assert_eq!(taking_kind(cause), kind, "{cause:?}");
        let world = app.world_mut();
        let before = world.resource::<ArmyStats>().get(pl).conquests;
        let points = world.get::<ChampionRank>(champion).unwrap().points;
        transfer(world, s, to, cause).expect("populated");
        let gained = world.get::<ChampionRank>(champion).unwrap().points > points;
        let counted = world.resource::<ArmyStats>().get(pl).conquests - before;
        app.step(1);
        let e = last_entry_about(&app, id).event;
        println!(
            "{cause:?}: {:?}, conquests +{counted}, champion credit {gained}",
            e.kind
        );
        assert_eq!(counted, conquests, "{cause:?}: conquests");
        assert_eq!(gained, credit, "{cause:?}: champion credit");
        assert_eq!(
            (e.kind, e.faction, e.other, e.cause),
            (kind, faction, other.0 as u64, cause),
            "{cause:?}"
        );
    }
    // The run summary: five takes (a conquest, a sack, an invitation, a
    // submission, a turned gate), four losses (three revolts, a defiance).
    let summary = run_summary(app.world());
    println!(
        "summary: taken {}, lost {}, held {}",
        summary.settlements_taken, summary.settlements_lost, summary.settlements_held
    );
    assert_eq!(
        (
            summary.settlements_taken,
            summary.settlements_lost,
            summary.settlements_held
        ),
        (5, 4, 1)
    );
    // The military log tells each in its own words.
    let log: Vec<String> = app
        .world()
        .resource::<MilitaryLog>()
        .entries
        .iter()
        .map(|(_, l)| l.clone())
        .collect();
    for needle in [
        " took ",
        "threw off",
        "defied",
        "invited",
        "knelt to",
        "opened its gates to",
    ] {
        assert!(
            log.iter().any(|l| l.contains(needle)),
            "{needle:?}: {log:?}"
        );
    }
}

/// ADR 0020: all 34 event kinds, in a table: chronicled, quiet, notable. A
/// kind is always fed to the UI, kept in the chronicle only if chronicled,
/// and resets the director's quiet clock (`last_notable`) only if notable.
#[test]
fn every_event_kind_is_classified() {
    use SimEventKind as K;
    // (kind, chronicled, quiet, notable)
    let table = [
        (K::RunStarted, true, false, true),
        (K::OfferPosted, false, false, false),
        (K::OfferChosen, true, false, true),
        (K::MonumentTaken, true, false, true),
        (K::MonumentLost, true, false, true),
        (K::RequestPosted, false, false, true),
        (K::RequestDone, true, false, true),
        (K::RequestFailed, true, false, true),
        (K::Conquest, true, false, true),
        (K::Invited, true, false, true),
        (K::Revolt, true, false, true),
        (K::RankUp, true, false, true),
        (K::TraitGained, true, false, true),
        (K::VeteranRank, false, false, false),
        (K::ChampionFell, true, false, true),
        (K::Succession, true, false, true),
        (K::RivalLevy, false, false, false),
        (K::RivalRaid, true, false, true),
        (K::RaidersArrived, true, false, true),
        (K::Battle, true, false, true),
        (K::DriftersJoined, false, false, false),
        (K::RunEnded, true, false, true),
        (K::Submitted, true, false, true),
        (K::Defied, true, false, true),
        (K::Subverted, true, false, true),
        (K::BanditsRose, true, false, true),
        (K::ReprisalVowed, true, false, true),
        (K::RivalReprisal, true, false, true),
        (K::AgentCaught, true, false, true),
        (K::Forsaken, true, true, false),
        (K::Built, true, true, false),
        (K::StewardNamed, true, true, false),
        (K::EdictSet, false, false, false),
        (K::Tribute, false, false, false),
    ];
    assert_eq!(table.len(), 34);
    assert_eq!(table.len(), SimEventKind::ALL.len());
    let mut app = run_app(SEED);
    quiet_director(&mut app);
    app.step(5);
    let me = FactionId::PLAYER;
    for (i, (kind, chronicled, quiet, notable)) in table.into_iter().enumerate() {
        assert_eq!(kind as usize, i, "{}", kind.name());
        assert_eq!(kind.chronicled(), chronicled, "{}", kind.name());
        assert_eq!(kind.quiet(), quiet, "{}", kind.name());
        assert_eq!(kind.notable(), notable, "{}", kind.name());

        let at = tick(&app);
        let seq = app.world().resource::<EventFeed>().next_seq();
        let calm = app.world().resource::<sim_core::Chronicle>().last_notable();
        assert!(calm < at, "the quiet clock was last reset at {calm}");
        app.world_mut()
            .write_message(SimEvent::new(kind, me).subject(7).other(8).amount(1));
        app.step(1);

        let world = app.world();
        let feed = world.resource::<EventFeed>();
        let fed: Vec<_> = feed.since(seq).map(|f| (f.tick, f.event.kind)).collect();
        assert_eq!(fed, [(at, kind)], "{}: always fed to the UI", kind.name());
        let kept: Vec<_> = chronicle(&app)
            .into_iter()
            .filter(|e| e.tick == at && e.event.kind == kind)
            .collect();
        assert_eq!(kept.len(), usize::from(chronicled), "{}", kind.name());
        let last = world.resource::<sim_core::Chronicle>().last_notable();
        let reset = if notable { at } else { calm };
        assert_eq!(last, reset, "{}: the quiet clock", kind.name());
        // A kept entry reads as a sentence.
        for e in &kept {
            let text = sim_military::chronicle::describe(world, e);
            assert!(!text.is_empty() && !text.contains(['{', '}']), "{text}");
        }
    }
    println!("{} kinds classified", SimEventKind::ALL.len());
}

/// SIM-61: through the real ledger, a town's log dates its deeds from day
/// 0, merges the same day's, faction's and deed's changes, marks word that
/// came from a neighbour as heard (a quarter of it, within the spread
/// radius), never spreads influence or whispers, and keeps eight.
#[test]
fn the_log_dates_and_merges() {
    let mut app = run_app(SEED);
    quiet_director(&mut app);
    let radius = app
        .world()
        .resource::<Villagers>()
        .data
        .goodwill
        .spread_radius;
    let fraction = app
        .world()
        .resource::<Villagers>()
        .data
        .goodwill
        .spread_fraction;
    // A town, a neighbour within the spread radius, and a town beyond it
    // from both.
    let towns = villages(&mut app);
    let dist = |a: usize, b: usize| (towns[a].2 - towns[b].2).as_vec2().length();
    let (a, n) = (0..towns.len())
        .flat_map(|a| (0..towns.len()).map(move |b| (a, b)))
        .find(|(a, b)| a != b && dist(*a, *b) <= radius)
        .expect("two towns within the spread radius");
    let far = (0..towns.len())
        .find(|f| dist(*f, a) > radius && dist(*f, n) > radius)
        .expect("a town beyond the radius");
    let (ae, aid) = (towns[a].0, towns[a].1);
    let (ne, _) = (towns[n].0, towns[n].1);
    let (fe, fid) = (towns[far].0, towns[far].1);
    println!(
        "town #{aid}, neighbour at {:.0} tiles, far town at {:.0} (radius {radius}, fraction {fraction})",
        dist(a, n),
        dist(far, a)
    );
    let (pl, rv) = (FactionId::PLAYER, FactionId::RIVAL);
    let log = |app: &App, e: Entity| -> Vec<sim_agents::DeedEntry> {
        app.world()
            .get::<DeedLog>(e)
            .expect("a run settlement has a deed log")
            .entries
            .iter()
            .copied()
            .collect()
    };
    let find = |log: &[sim_agents::DeedEntry], f: FactionId, d: Deed, heard: bool| {
        log.iter()
            .filter(|e| e.faction == f && e.deed == d && e.heard == heard)
            .copied()
            .collect::<Vec<_>>()
    };
    assert!(log(&app, ae).is_empty(), "nothing remembered at the start");

    // Day 0: two plunders by the player merge; the rival's is its own line;
    // an influence is computed and local.
    {
        let mut ledger = app.world_mut().resource_mut::<GoodwillLedger>();
        ledger.push(aid, pl, -0.10, Deed::Plunder);
        ledger.push(aid, pl, -0.05, Deed::Plunder);
        ledger.push(aid, rv, -0.05, Deed::Plunder);
        ledger.push(aid, pl, 0.03, Deed::Influence);
    }
    app.step(1);
    let here = log(&app, ae);
    println!("  day 0 at the town: {here:?}");
    let plunder = find(&here, pl, Deed::Plunder, false);
    assert_eq!(plunder.len(), 1, "same day, faction and deed merge");
    assert_eq!(plunder[0].day, 0, "days count from 0");
    assert!(
        (plunder[0].delta + 0.15).abs() < 1e-6,
        "{}",
        plunder[0].delta
    );
    let rivals = find(&here, rv, Deed::Plunder, false);
    assert_eq!(rivals.len(), 1, "another faction is another line");
    assert!((rivals[0].delta + 0.05).abs() < 1e-6);
    assert_eq!(find(&here, pl, Deed::Influence, false).len(), 1);
    // The neighbour hears of the plunders, a quarter of each, marked heard;
    // the influence never reaches it; the far town hears of nothing.
    let near = log(&app, ne);
    println!("  day 0 at the neighbour: {near:?}");
    let heard = find(&near, pl, Deed::Plunder, true);
    assert_eq!(heard.len(), 1, "word merges too");
    assert!(
        (heard[0].delta - (-0.15 * fraction)).abs() < 1e-6,
        "{}",
        heard[0].delta
    );
    assert!((find(&near, rv, Deed::Plunder, true)[0].delta + 0.05 * fraction).abs() < 1e-6);
    assert!(
        near.iter().all(|e| e.deed != Deed::Influence),
        "influence does not spread"
    );
    assert!(near.iter().all(|e| e.heard), "nothing was done there");
    assert!(
        log(&app, fe).iter().all(|e| e.deed != Deed::Plunder),
        "beyond the radius nobody hears"
    );

    // A day later, the same deed is a new line, dated day 1.
    app.step(ticks_per_day(&app));
    app.world_mut()
        .resource_mut::<GoodwillLedger>()
        .push(aid, pl, 0.09, Deed::Gift);
    app.step(1);
    let here = log(&app, ae);
    let gift = find(&here, pl, Deed::Gift, false);
    println!("  a gift at tick {}: {gift:?}", tick(&app));
    assert_eq!((gift.len(), gift[0].day), (1, 1));
    assert_eq!(
        find(&here, pl, Deed::Plunder, false)[0].day,
        0,
        "kept as dated"
    );

    // Eight entries: nine different deeds in one tick leave eight, the
    // first (in the ledger's order) dropped.
    {
        let mut ledger = app.world_mut().resource_mut::<GoodwillLedger>();
        for d in [
            Deed::Trade,
            Deed::Equip,
            Deed::Seize,
            Deed::Provoke,
            Deed::Killing,
            Deed::Request,
            Deed::Gift,
            Deed::Defended,
            Deed::Festival,
        ] {
            ledger.push(fid, pl, 0.01, d);
        }
    }
    app.step(1);
    let full = log(&app, fe);
    println!("  nine deeds at the far town leave {} entries", full.len());
    assert_eq!(full.len(), DeedLog::CAPACITY);
    assert!(
        full.iter().all(|e| e.deed != Deed::Trade),
        "the oldest drops off"
    );
    assert_eq!(full.last().map(|e| e.deed), Some(Deed::Festival));
}

/// SIM-60: each term moves `loyalty_target` by exactly its value (inside
/// 0..1), and the terms add; absent terms are an exact no-op; the parts add
/// up to the unbiased target; in the real schedule a town's Resentment term
/// carries its scar and the day's drift follows the biased target.
#[test]
fn terms_sum_into_the_target() {
    let mut app = run_app(SEED);
    quiet_director(&mut app);
    let data = app.world().resource::<Villagers>().data.loyalty.clone();
    let tpd = ticks_per_day(&app);
    let owner = Owner {
        faction: FactionId::PLAYER,
        since: 0,
    };
    // A middling town: half-secure food, mild goodwill, three days old.
    let (goodwill, food, now) = (0.1, data.food_days * 0.5, 3 * tpd);
    let base = loyalty_target(&data, owner, goodwill, food, now, tpd, 0.0);
    let (f, t, d) = loyalty_parts(&data, owner, goodwill, food, now, tpd);
    println!("base {base:.3} = food {f:.3} + treatment {t:.3} + time {d:.3}");
    assert!((f + t + d - base).abs() < 1e-6);
    assert!(base > 0.3 && base < 0.7, "room on both sides: {base}");
    assert_eq!(
        loyalty_target(
            &data,
            owner,
            goodwill,
            food,
            now,
            tpd,
            LoyaltyTerms::default().bias()
        ),
        base,
        "absent terms are an exact no-op"
    );
    let mut all = LoyaltyTerms::default();
    let mut total = 0.0;
    for (i, term) in LoyaltyTerm::ALL.into_iter().enumerate() {
        let v = (if i % 2 == 0 { -0.03 } else { 0.02 }) * (i as f32 + 1.0);
        let mut one = LoyaltyTerms::default();
        one.set(term, v);
        let target = loyalty_target(&data, owner, goodwill, food, now, tpd, one.bias());
        assert!(
            (target - (base + v)).abs() < 1e-6,
            "{}: {target}",
            term.name()
        );
        all.set(term, v);
        total += v;
    }
    let target = loyalty_target(&data, owner, goodwill, food, now, tpd, all.bias());
    println!("every term: bias {:+.3}, target {target:.3}", all.bias());
    assert!((all.bias() - total).abs() < 1e-5);
    assert!((target - (base + total).clamp(0.0, 1.0)).abs() < 1e-6);
    assert_eq!(all.nonzero().count(), LoyaltyTerm::COUNT);
    assert_eq!(
        loyalty_target(&data, Owner::default(), goodwill, food, now, tpd, -0.4),
        data.independent_target,
        "nobody to be loyal to"
    );

    // The real schedule. Only run towns carry terms, and a free town's are
    // all zero.
    let (s, _) = free_town(&mut app);
    assert_eq!(terms(&app, s).bias(), 0.0);
    // A conquest with 4 militia dead: scar 0.16; an hour later the term
    // holds it, and the town's real target is lower by exactly that. (Not
    // at tick 0, which is itself a day tick that would fade the scar.)
    app.step(1);
    app.world_mut().get_mut::<Tenure>(s).unwrap().blood = 4;
    transfer(
        app.world_mut(),
        s,
        FactionId::PLAYER,
        Cause::Conquest { captain: 1 },
    )
    .expect("populated");
    let scar = tenure(&app, s).scar;
    let hour = app.world().resource::<Calendar>().ticks_per_hour();
    app.step(hour + 1);
    let carried = terms(&app, s);
    println!("  scar {scar}, terms {:?}", carried.0);
    assert!((scar - 0.16).abs() < 1e-6);
    assert_eq!(carried.get(LoyaltyTerm::Resentment), -scar);
    let target_of = |app: &App, bias: f32| {
        let w = app.world();
        loyalty_target(
            &data,
            *w.get::<Owner>(s).unwrap(),
            w.get::<Goodwill>(s).unwrap().get(FactionId::PLAYER),
            w.get::<FoodSecurity>(s).unwrap().days,
            tick(app),
            tpd,
            bias,
        )
    };
    let unbiased = target_of(&app, 0.0);
    let biased = target_of(&app, carried.bias());
    println!("  target {unbiased:.3} unbiased, {biased:.3} with the terms");
    assert!(
        (unbiased + carried.bias() - biased).abs() < 1e-6 || biased == 0.0,
        "{unbiased} + {} vs {biased}",
        carried.bias()
    );
    assert!(biased < unbiased);

    // The day tick drifts every resident toward the biased target.
    let seed_loyalty: Vec<f32> = residents_loyalty(&mut app, s);
    let first = seed_loyalty[0];
    assert!(seed_loyalty.iter().all(|l| *l == first), "seeded alike");
    app.step(tpd + 1 - tick(&app) % tpd);
    let after = residents_loyalty(&mut app, s);
    assert!(after.iter().all(|l| *l == after[0]), "drifted alike");
    let rate = data.drift_per_day
        * app
            .world()
            .resource::<FactionModifiers>()
            .mult(FactionId::PLAYER, Stat::LoyaltyDrift);
    // The drift ran before the progress step faded the scar, so the bias it
    // used is the one the town carried a day tick ago.
    let with = drift_toward(first, target_of(&app, carried.bias()), rate);
    let without = drift_toward(first, target_of(&app, 0.0), rate);
    println!(
        "  loyalty {first:.3} -> {:.3} (expected {with:.3}; {without:.3} without the terms)",
        after[0]
    );
    assert!(
        (after[0] - with).abs() < 0.02,
        "the drift follows the biased target: {} vs {with}",
        after[0]
    );
    assert!(
        (after[0] - without).abs() > 0.03,
        "and not the unbiased one: {} vs {without}",
        after[0]
    );
}

/// The loyalty of every resident of `s` who isn't enlisted, by SpawnId.
fn residents_loyalty(app: &mut App, s: Entity) -> Vec<f32> {
    let world = app.world_mut();
    let mut q = world.query_filtered::<(&SpawnId, &Villager, &Loyalty), Without<Enlisted>>();
    let mut v: Vec<_> = q
        .iter(world)
        .filter(|(_, v, _)| v.settlement == s)
        .map(|(id, _, l)| (*id, l.0))
        .collect();
    v.sort_unstable_by_key(|(id, _)| *id);
    v.into_iter().map(|(_, l)| l).collect()
}

/// LORDSHIP_DESIGN section 4, "terms are named when they decide": with the
/// shipped numbers, a small term that keeps a town above the revolt line
/// (Awe, or Bond) is the one named, and so is one that tips it under; the
/// two largest negative pulls are what a revolt would cite. (Wording the
/// revolt's chronicle entry with them, and the Dawn line, come with the
/// steps that post those events; the panel names them from step 1.)
#[test]
fn a_term_is_named_when_it_decides() {
    let app = run_app(SEED);
    let data = app.world().resource::<Villagers>().data.loyalty.clone();
    let tpd = ticks_per_day(&app);
    let owner = Owner {
        faction: FactionId::PLAYER,
        since: 0,
    };
    let line = data.revolt_threshold;
    // A starved town of neutral goodwill ten days under its owner: 0.30,
    // just under the line.
    let base = loyalty_target(&data, owner, 0.0, 0.0, 10 * tpd, tpd, 0.0);
    println!("base {base:.3}, the line {line}");
    assert!((0.25..line).contains(&base), "{base}");
    let mut awe = LoyaltyTerms::default();
    awe.set(LoyaltyTerm::Awe, 0.1);
    assert!(loyalty_target(&data, owner, 0.0, 0.0, 10 * tpd, tpd, awe.bias()) >= line);
    assert_eq!(
        deciding_term(base, &awe, line),
        Some((LoyaltyTerm::Awe, 0.1)),
        "awe holds the town above the line"
    );
    let mut bond = LoyaltyTerms::default();
    bond.set(LoyaltyTerm::Bond, 0.08);
    assert_eq!(
        deciding_term(base, &bond, line),
        Some((LoyaltyTerm::Bond, 0.08)),
        "so does the bond you kept"
    );
    // Both, on a town 0.15 short of the line without either: each alone
    // would still leave it above (0.30 + 0.08 and 0.30 + 0.10), so neither
    // decides; on one at 0.20 either alone is enough and the larger is named.
    awe.set(LoyaltyTerm::Bond, 0.08);
    assert_eq!(deciding_term(base, &awe, line), None);
    assert_eq!(
        deciding_term(0.2, &awe, line).map(|(t, _)| t),
        Some(LoyaltyTerm::Awe)
    );
    // A term that changes nothing about the side is not named.
    assert_eq!(deciding_term(0.8, &bond, line), None);
    assert_eq!(deciding_term(0.1, &bond, line), None);
    // A scar tips a town that would otherwise hold.
    let mut scar = LoyaltyTerms::default();
    scar.set(LoyaltyTerm::Resentment, -0.14);
    assert_eq!(
        deciding_term(0.45, &scar, line),
        Some((LoyaltyTerm::Resentment, -0.14))
    );

    // The pulls on a starved, resented town: hunger, then resentment, then
    // ill will; the first two are the ones a revolt would name.
    let mut pulled = LoyaltyTerms::default();
    pulled.set(LoyaltyTerm::Resentment, -0.14);
    pulled.set(LoyaltyTerm::Contest, -0.05);
    pulled.set(LoyaltyTerm::Awe, 0.1);
    let pulls = loyalty_pulls(&data, owner, -0.4, 0.0, &pulled);
    let names: Vec<&str> = pulls.iter().map(|(p, _)| p.name()).collect();
    println!("pulls: {pulls:?}");
    assert_eq!(
        names,
        ["hunger", "resentment", "ill will", "rival's pull"],
        "largest first, Awe is no pull"
    );
    assert_eq!(pulls[0].0, Pull::Hunger);
    assert_eq!(pulls[1].0, Pull::Term(LoyaltyTerm::Resentment));
    assert!(pulls.windows(2).all(|w| w[0].1 <= w[1].1));
    // They are exactly what the target is missing: a fed, liked, settled
    // town has none.
    assert!(loyalty_pulls(&data, owner, 0.5, data.food_days, &LoyaltyTerms::default()).is_empty());
}

/// UI-09: every cause has words in the chronicle's sentences, naming the
/// captain or the town where it has one, and none prints as code.
#[test]
fn every_cause_is_worded() {
    let mut app = run_app(SEED);
    let (s, id) = free_town(&mut app);
    let champion = champion(&mut app);
    let captain = spawn_id(&app, champion);
    let f = FactionId::RIVAL;
    let causes = [
        Cause::None,
        Cause::Player,
        Cause::Request { id: 3 },
        Cause::Conquest { captain },
        Cause::Invitation,
        Cause::Starvation,
        Cause::Plunder,
        Cause::Revolt,
        Cause::Raid { captain },
        Cause::Hold,
        Cause::Boon { card: 2 },
        Cause::Death { faction: f },
        Cause::Lapsed,
        Cause::Neglect,
        Cause::Fight { faction: f },
        Cause::Sack { captain },
        Cause::Submission { captain },
        Cause::Subversion,
        Cause::Defiance,
        Cause::Fallout { settlement: id },
        Cause::Reprisal { settlement: id },
        Cause::Edict,
        Cause::Caught,
        Cause::Deposed,
    ];
    for (i, c) in causes.iter().enumerate() {
        assert_eq!(c.tag() as usize, i, "tags are positions: {}", c.describe());
    }
    let world = app.world();
    let town = world
        .resource::<sim_core::NameTable>()
        .get(world.get::<sim_world::Settlement>(s).unwrap().name)
        .to_string();
    let phrases = &world.resource::<Progression>().data.chronicle.causes;
    let mut seen = Vec::new();
    for cause in causes {
        let entry = ChronicleEntry {
            tick: 10,
            event: SimEvent::new(SimEventKind::Conquest, FactionId::PLAYER)
                .subject(id)
                .cause(cause),
        };
        let text = sim_military::chronicle::describe(world, &entry);
        let phrase = phrases.phrase(cause);
        println!("{cause:?}: {text}");
        assert!(!text.contains(['{', '}']), "{text}");
        assert_eq!(phrase.is_empty(), cause == Cause::None, "{cause:?}");
        if matches!(
            cause,
            Cause::Sack { .. } | Cause::Submission { .. } | Cause::Conquest { .. }
        ) {
            assert!(
                !text.contains("their captain"),
                "the captain is named: {text}"
            );
        }
        if matches!(cause, Cause::Fallout { .. } | Cause::Reprisal { .. }) {
            assert!(text.contains(&town), "{town} is named: {text}");
        }
        seen.push(text);
    }
    let mut unique = seen.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), causes.len(), "each cause reads differently");
}
