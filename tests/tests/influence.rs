//! M4a step 3 scenario `influence` (GP-31, UI-12; ADR 0020, LORDSHIP_DESIGN
//! section 6): held towns radiate influence by their people and loyalty,
//! falling off with distance and contested by the rival's; a free town
//! inside your sphere warms with no deed done and invites you with no army
//! near (at most one such invitation a day); a border town in the other
//! side's sphere loses loyalty; influence never spreads by word of mouth;
//! the rival's seat is never swayed; and a realm ranks the champion and
//! draws drifters (design question 16).
//!
//! Every test plays the real schedule on the run island (`common::run_app`)
//! from state set up before the first tick: towns handed to the player with
//! `sim_military::transfer` (an invitation, no scar) and their villagers'
//! loyalty set, as a few days of play would leave them. Tick 0 is a day tick
//! and an hourly one, so the field, the day's warmth and the envoy flags all
//! come in the first tick; the ledger applies its deeds in the next.
//!
//! - `a_held_town_warms_its_neighbour`: a free town between two held ones
//!   gets exactly the `Deed::Influence` the formula gives, every other
//!   town warms exactly when its own net says so, and the field is what
//!   `field_at` makes of the held towns.
//! - `the_rival_contests_the_border`: a town in the other side's sphere
//!   carries a negative Contest term, and its loyalty ends a day lower
//!   than with influence off; a town of yours deep in your sphere carries
//!   the heartland bonus instead (never the rival's towns in its own).
//! - `envoys_come_without_a_captain`: with free towns inside the sphere
//!   that like you enough, one invites you with no captain within 40
//!   tiles, never two in a day; the next dawn sends another; the card
//!   hands the town over.
//! - `a_free_town_between_two_of_yours_invites_within_ten_days` (ignored,
//!   ten game days): the done-when of step 3 on the shipped tuning.
//! - `influence_deeds_do_not_spread`: word of an influence deed never
//!   reaches a neighbour, while any other deed's does.
//! - `the_seat_is_never_swayed`: the rival's seat gets no influence deed,
//!   no envoys and no Contest term, whatever surrounds it.
//! - `a_realm_ranks_the_champion_and_draws_drifters`: people held earn the
//!   champion rank points a day, and a village a little under the drifter
//!   line sends volunteers inside the sphere and not with the ease off.
//!
//! Each test prints the numbers it checks.

mod common;

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use common::*;
use sim_agents::loyalty::loyalty_target;
use sim_agents::{
    Deed, DeedEntry, DeedLog, FoodSecurity, Goodwill, GoodwillLedger, Loyalty, LoyaltyTerm,
    LoyaltyTerms, SettlementLoyalty, Villager, Villagers,
};
use sim_core::{
    Calendar, Cause, FactionId, OfferKind, Offers, Position, Posture, SimStepper, SimTick,
};
use sim_economy::{Economy, Stockpile};
use sim_military::influence::{self, InfluenceData, Source, field_at};
use sim_military::progression::ChampionRank;
use sim_military::{
    ArmySpec, Captain, Influence, Progression, RivalState, RunProgress, TakenBy, Tenure,
    spawn_army, transfer,
};
use sim_world::Settlement;

/// A town of the island: (entity, SpawnId, centre).
#[derive(Debug, Clone, Copy)]
struct Town {
    e: Entity,
    id: u64,
    at: IVec2,
}

fn tick(app: &App) -> u64 {
    app.world().resource::<SimTick>().0
}

fn ticks_per_day(app: &App) -> u64 {
    app.world().resource::<Calendar>().ticks_per_day
}

fn data(app: &App) -> InfluenceData {
    app.world().resource::<Progression>().data.influence
}

fn edit(app: &mut App, f: impl FnOnce(&mut InfluenceData)) {
    f(&mut app.world_mut().resource_mut::<Progression>().data.influence);
}

/// A quiet run of `seed` (no director bands).
fn run(seed: u64) -> App {
    let mut app = run_app(seed);
    quiet_director(&mut app);
    app
}

fn seat_id(app: &App) -> u64 {
    app.world()
        .resource::<RunProgress>()
        .rival_seat
        .expect("a rival seat")
}

/// Every town nobody holds that isn't the rival's seat, by SpawnId.
fn free_towns(app: &mut App) -> Vec<Town> {
    let seat = seat_id(app);
    villages(app)
        .into_iter()
        .filter(|&(e, id, ..)| id != seat && owner_of(app, e) == FactionId::INDEPENDENT)
        .map(|(e, id, at, _)| Town { e, id, at })
        .collect()
}

fn dist(a: IVec2, b: IVec2) -> f32 {
    (a - b).as_vec2().length()
}

/// `towns` nearest `to` first (ties by SpawnId), leaving out `skip`.
fn nearest(towns: &[Town], to: IVec2, skip: &[u64]) -> Vec<Town> {
    let mut v: Vec<Town> = towns
        .iter()
        .copied()
        .filter(|t| !skip.contains(&t.id))
        .collect();
    v.sort_by(|a, b| {
        dist(a.at, to)
            .total_cmp(&dist(b.at, to))
            .then(a.id.cmp(&b.id))
    });
    v
}

/// The distance from `at` to the player's nearest captain.
fn captain_distance(app: &mut App, at: IVec2) -> f32 {
    let world = app.world_mut();
    let mut q = world.query::<(&Captain, &Position)>();
    q.iter(world)
        .filter(|(c, _)| c.faction == FactionId::PLAYER)
        .map(|(_, p)| dist(p.0, at))
        .fold(f32::INFINITY, f32::min)
}

/// A free target town far from every captain, with the two free towns
/// nearest it: the pair of neighbours whose summed distance to it is the
/// least wins (ties by SpawnId). Returns (target, a, b).
fn trio(app: &mut App) -> (Town, Town, Town) {
    let free = free_towns(app);
    let radius = app
        .world()
        .resource::<Progression>()
        .data
        .invitations
        .radius;
    let mut best: Option<(f32, Town, Town, Town)> = None;
    for t in &free {
        if captain_distance(app, t.at) <= radius + 5.0 {
            continue;
        }
        let near = nearest(&free, t.at, &[t.id]);
        let (a, b) = (near[0], near[1]);
        let span = dist(a.at, t.at) + dist(b.at, t.at);
        if best.is_none_or(|(s, ..)| span < s) {
            best = Some((span, *t, a, b));
        }
    }
    let (span, t, a, b) = best.expect("a free town far from the band");
    println!(
        "target #{} at {}: neighbours #{} ({:.0} tiles) and #{} ({:.0} tiles), span {span:.0}",
        t.id,
        t.at,
        a.id,
        dist(a.at, t.at),
        b.id,
        dist(b.at, t.at),
    );
    (t, a, b)
}

/// Hands `s` to the player (an invitation: no scar) and sets its
/// villagers' loyalty, and the settlement's mean, to `loyalty`.
fn hold(app: &mut App, s: Town, loyalty: f32) {
    transfer(app.world_mut(), s.e, FactionId::PLAYER, Cause::Invitation).expect("populated");
    set_loyalty(app, s, loyalty);
}

fn set_loyalty(app: &mut App, s: Town, loyalty: f32) {
    let world = app.world_mut();
    let mut q = world.query::<(&Villager, &mut Loyalty)>();
    for (v, mut l) in q.iter_mut(world) {
        if v.settlement == s.e {
            l.0 = loyalty;
        }
    }
    world.get_mut::<SettlementLoyalty>(s.e).unwrap().mean = loyalty;
}

fn influence_at(app: &App, s: Town) -> Influence {
    *app.world().get::<Influence>(s.e).expect("a run town")
}

fn goodwill(app: &App, s: Town) -> f32 {
    app.world()
        .get::<Goodwill>(s.e)
        .unwrap()
        .get(FactionId::PLAYER)
}

fn set_goodwill(app: &mut App, s: Town, g: f32) {
    app.world_mut()
        .get_mut::<Goodwill>(s.e)
        .unwrap()
        .set(FactionId::PLAYER, g);
}

fn deeds(app: &App, s: Town) -> Vec<DeedEntry> {
    app.world()
        .get::<DeedLog>(s.e)
        .expect("a run town keeps a deed log")
        .entries
        .iter()
        .copied()
        .collect()
}

/// The direct `Deed::Influence` entries of the player in `s`'s log.
fn warmth_logged(app: &App, s: Town) -> Vec<DeedEntry> {
    deeds(app, s)
        .into_iter()
        .filter(|e| e.faction == FactionId::PLAYER && e.deed == Deed::Influence && !e.heard)
        .collect()
}

/// The invitations pending, by settlement.
fn pending_invitations(app: &App) -> Vec<u64> {
    app.world()
        .resource::<Offers>()
        .pending
        .iter()
        .filter_map(|o| match o.kind {
            OfferKind::Invitation { settlement } => Some(settlement),
            _ => None,
        })
        .collect()
}

fn contest(app: &App, s: Town) -> f32 {
    app.world()
        .get::<LoyaltyTerms>(s.e)
        .expect("a run town has loyalty terms")
        .get(LoyaltyTerm::Contest)
}

/// Every populated town as the sim sees it (id, centre, owner, people,
/// mean loyalty, scar) turned into field sources, by hand.
fn sources_by_hand(app: &mut App) -> Vec<Source> {
    let seat = seat_id(app);
    let d = data(app);
    let world = app.world_mut();
    let mut q = world.query::<(
        &sim_core::SpawnId,
        &Settlement,
        &sim_agents::Owner,
        &sim_economy::SettlementEconomy,
        &SettlementLoyalty,
        &Tenure,
    )>();
    let mut v: Vec<Source> = q
        .iter(world)
        .filter(|(_, _, o, ..)| matches!(o.faction, FactionId::PLAYER | FactionId::RIVAL))
        .map(|(id, st, o, se, sl, t)| Source {
            id: id.0,
            at: st.center.as_vec2(),
            faction: o.faction,
            power: influence::power(
                &d,
                se.population,
                sl.mean,
                t.scar,
                id.0 == seat && o.faction == FactionId::RIVAL,
            ),
        })
        .filter(|s| s.power > 0.0)
        .collect();
    v.sort_by_key(|s| s.id);
    v
}

/// GP-31: a free town between two held ones warms. The first tick is a
/// day tick: the hourly step writes every town's power and shares as
/// `field_at` gives them from the held towns; the day's warmth then goes
/// to the ledger and lands in the next tick as exactly one `Deed::Influence`
/// of `warmth(net, goodwill)` at every town the player doesn't own (the seat
/// excepted) whose net is above `warm_above`, and nowhere else.
#[test]
fn a_held_town_warms_its_neighbour() {
    let mut app = run(42);
    let (t, a, b) = trio(&mut app);
    let d = data(&app);
    assert_eq!(goodwill(&app, t), 0.0, "a stranger to start");
    hold(&mut app, a, 0.9);
    hold(&mut app, b, 0.9);
    app.step(2);

    // The field is what `field_at` makes of the held towns.
    let sources = sources_by_hand(&mut app);
    assert!(sources.iter().any(|s| s.faction == FactionId::PLAYER));
    assert!(sources.iter().any(|s| s.faction == FactionId::RIVAL));
    for town in villages(&mut app) {
        let (e, id, at, _) = town;
        let inf = *app.world().get::<Influence>(e).unwrap();
        let want = field_at(&d, &sources, at.as_vec2());
        assert_eq!(inf.shares(), want, "town #{id}");
    }
    let inf = influence_at(&app, t);
    println!(
        "target #{}: net {:.3} (you {:.3}, rival {:.3})",
        t.id,
        inf.net(),
        inf.player,
        inf.rival
    );
    assert!(inf.net() > d.warm_above, "warmed: {inf:?}");

    // The day's warmth at the target.
    let logged = warmth_logged(&app, t);
    assert_eq!(logged.len(), 1, "one deed: {logged:?}");
    let delta = logged[0].delta;
    let before = goodwill(&app, t) - delta;
    println!(
        "  warmth {delta:.4} from goodwill {before:.4} to {:.4}",
        goodwill(&app, t)
    );
    assert!(delta > 0.0);
    assert!((delta - influence::warmth(&d, inf.net(), before)).abs() < 1e-5);
    assert!(
        (before - 0.0).abs() < 1e-5,
        "nothing else moved it: {before}"
    );
    assert_eq!(logged[0].day, 0);

    // Every town warms exactly when its own net says so.
    let seat = seat_id(&app);
    let mut warmed = 0;
    for (e, id, at, _) in villages(&mut app) {
        let town = Town { e, id, at };
        let inf = influence_at(&app, town);
        let owner = owner_of(&app, e);
        let direct = warmth_logged(&app, town);
        let should = owner != FactionId::PLAYER && id != seat && inf.net() > d.warm_above;
        assert_eq!(
            !direct.is_empty(),
            should,
            "town #{id} ({owner:?}, net {:.3}, logged {direct:?})",
            inf.net()
        );
        warmed += usize::from(should);
    }
    println!("  {warmed} towns warmed on the first dawn");
    assert!(warmed >= 1);

    // Off the day tick nothing more is posted: the next hour adds none.
    let hour = app.world().resource::<Calendar>().ticks_per_hour();
    let logged_before = warmth_logged(&app, t).len();
    app.step(hour);
    assert_eq!(warmth_logged(&app, t).len(), logged_before);
}

/// GP-31, LORDSHIP_DESIGN section 6: a border town in the other side's
/// sphere loses loyalty. The rival's own town handed to it in the middle
/// of three of yours feels the player's pull (a negative Contest term,
/// `-0.25 x` the lead), and a day on its residents are less loyal than
/// with influence off; a town of yours deep in your own sphere carries the
/// heartland bonus instead, and your border town beside the rival's lands
/// the rival's pull. The heartland is the player's only: the rival's towns
/// deep in its own sphere carry none.
#[test]
fn the_rival_contests_the_border() {
    let scene = |enabled: bool| {
        let mut app = run(42);
        edit(&mut app, |d| d.enabled = enabled);
        let (r, a, b) = trio(&mut app);
        let free = free_towns(&mut app);
        let c = nearest(&free, r.at, &[r.id, a.id, b.id])[0];
        for s in [a, b, c] {
            hold(&mut app, s, 0.9);
        }
        // The rival gets the town in the middle of them.
        transfer(app.world_mut(), r.e, FactionId::RIVAL, Cause::None).expect("populated");
        set_loyalty(&mut app, r, 0.8);
        (app, r, a, b, c)
    };
    let (mut app, r, a, b, c) = scene(true);
    let (mut off, ..) = scene(false);
    let d = data(&app);
    app.step(1);
    off.step(1);

    let inf = influence_at(&app, r);
    let term = contest(&app, r);
    println!(
        "rival's town #{}: you {:.3}, rival {:.3}; Contest {term:.4}",
        r.id, inf.player, inf.rival
    );
    assert!(inf.player > inf.rival, "the player leads there: {inf:?}");
    assert!(term < 0.0);
    assert!((term + d.contest * (inf.player - inf.rival)).abs() < 1e-5);
    assert_eq!(contest(&off, r), 0.0, "influence off: no pull");

    // The towns of yours deep inside your own sphere: the heartland.
    let mut heartland = 0;
    for s in [a, b, c] {
        let i = influence_at(&app, s);
        let t = contest(&app, s);
        println!(
            "  held #{}: you {:.3}, rival {:.3}; Contest {t:+.3}",
            s.id, i.player, i.rival
        );
        if i.player - i.rival >= d.sphere {
            assert_eq!(t, d.heartland, "town #{}", s.id);
            heartland += 1;
        } else if i.rival > i.player {
            assert!(t < 0.0);
        } else {
            assert_eq!(t, 0.0);
        }
    }
    assert!(heartland >= 1, "a held town leads by the sphere's width");

    // A day on: its loyalty target is lower by the term, and its people are
    // less loyal than with influence off.
    let day = ticks_per_day(&app);
    app.step(day + 1 - tick(&app));
    off.step(day + 1 - tick(&off));
    let mean = |app: &App| app.world().get::<SettlementLoyalty>(r.e).unwrap().mean;
    let terms = *app.world().get::<LoyaltyTerms>(r.e).unwrap();
    let villagers = app.world().resource::<Villagers>();
    let food = app.world().get::<FoodSecurity>(r.e).unwrap().days;
    let goodwill_to_rival = app
        .world()
        .get::<Goodwill>(r.e)
        .unwrap()
        .get(FactionId::RIVAL);
    let owner = *app.world().get::<sim_agents::Owner>(r.e).unwrap();
    let target = |bias: f32| {
        loyalty_target(
            &villagers.data.loyalty,
            owner,
            goodwill_to_rival,
            food,
            tick(&app),
            day,
            bias,
        )
    };
    let gap = target(terms.bias()) - target(terms.bias() - terms.get(LoyaltyTerm::Contest));
    println!(
        "  a day on: mean {:.4} against {:.4} with influence off; the term moves the target {gap:+.4}",
        mean(&app),
        mean(&off)
    );
    assert!(gap < -0.005, "{gap}");
    assert!(
        mean(&app) < mean(&off) - 0.005,
        "{} vs {}",
        mean(&app),
        mean(&off)
    );

    // Your town beside the rival's lands feels the rival's pull.
    let mut app = run(42);
    let seat_at = app.world().resource::<RivalState>().seat_center;
    let free = free_towns(&mut app);
    let border = nearest(&free, seat_at, &[])[0];
    hold(&mut app, border, 0.8);
    app.step(1);
    let i = influence_at(&app, border);
    let t = contest(&app, border);
    println!(
        "your border town #{} ({:.0} tiles from the seat): you {:.3}, rival {:.3}; Contest {t:.4}",
        border.id,
        dist(border.at, seat_at),
        i.player,
        i.rival
    );
    assert!(i.rival > i.player, "the rival leads at the border");
    assert!((t + d.contest * (i.rival - i.player)).abs() < 1e-5);
    assert!(t < 0.0);

    // The heartland is yours only: the rival's towns deep in its own
    // sphere carry no bonus (design section 6, "your heartland").
    let seat = seat_id(&app);
    let mut deep = 0;
    for (e, id, at, _) in villages(&mut app) {
        if id == seat || owner_of(&app, e) != FactionId::RIVAL {
            continue;
        }
        let town = Town { e, id, at };
        let i = influence_at(&app, town);
        if i.rival - i.player >= d.sphere {
            assert_eq!(contest(&app, town), 0.0, "the rival's #{id}: {i:?}");
            deep += 1;
        }
    }
    println!("  the rival's towns deep in its sphere, no heartland: {deep}");
    assert!(deep >= 1, "a rival town leads by the sphere's width");
}

/// Tops up the champion's baggage with grain. A band with no one to feed
/// it starves in a few days and its fall ends the run, which freezes the
/// clock; the long scenario plays on.
fn feed_band(app: &mut App) {
    let band = champion(app);
    let grain = *app
        .world()
        .resource::<Economy>()
        .foods_by_value()
        .last()
        .expect("a food");
    app.world_mut()
        .get_mut::<Stockpile>(band)
        .expect("the band's baggage")
        .add(grain, 10_000);
}

/// The invitations seen pending, in the order they appeared, stepping
/// `hours` hours from now and looking after each.
fn watch_invitations(app: &mut App, hours: u64, seen: &mut Vec<u64>) {
    let hour = app.world().resource::<Calendar>().ticks_per_hour();
    for _ in 0..hours {
        app.step(hour);
        for id in pending_invitations(app) {
            if !seen.contains(&id) {
                seen.push(id);
            }
        }
    }
}

/// The free towns the envoy tests watch: the free town farthest from the
/// rival's seat is the player's (loyalty 1.0) and up to three free towns
/// within 150 tiles of it and out of every captain's reach like the player
/// as much as an invitation needs. The independence in the shares is
/// shrunk so they are deep in the sphere (the shipped tuning is the ten-day
/// test's).
fn sphere_with_candidates(seed: u64) -> (App, Vec<Town>) {
    let mut app = run(seed);
    edit(&mut app, |d| d.independence = 0.01);
    let rules = app.world().resource::<Progression>().data.invitations;
    let seat_at = app.world().resource::<RivalState>().seat_center;
    let free = free_towns(&mut app);
    let home = free
        .iter()
        .copied()
        .max_by(|a, b| {
            dist(a.at, seat_at)
                .total_cmp(&dist(b.at, seat_at))
                .then(b.id.cmp(&a.id))
        })
        .unwrap();
    hold(&mut app, home, 1.0);
    let mut liked = Vec::new();
    for t in nearest(&free, home.at, &[home.id]) {
        if liked.len() == 3 {
            break;
        }
        if captain_distance(&mut app, t.at) > rules.radius + 5.0 && dist(t.at, home.at) < 150.0 {
            set_goodwill(&mut app, t, rules.goodwill);
            liked.push(t);
        }
    }
    assert!(liked.len() >= 2, "candidates: {liked:?}");
    (app, liked)
}

/// GP-31: envoys come without a captain, at most one a day. Three free
/// towns deep in the player's sphere, far from every captain, like the
/// player enough. The first tick is a dawn: only the highest net is
/// flagged; the first hour sends its envoys (an invitation with no captain
/// within 40 tiles) and spends the flag; no second invitation comes all
/// day; the player accepts and the town is his by invitation; the next
/// dawn another town sends envoys.
#[test]
fn envoys_come_without_a_captain() {
    let (mut app, liked) = sphere_with_candidates(42);
    let d = data(&app);
    let rules = app.world().resource::<Progression>().data.invitations;
    app.step(1);
    let nets: Vec<(u64, f32)> = liked
        .iter()
        .map(|t| (t.id, influence_at(&app, *t).net()))
        .collect();
    println!("candidates (id, net): {nets:?}");
    assert!(nets.iter().all(|n| n.1 >= d.envoy_net));
    let flagged: Vec<u64> = liked
        .iter()
        .filter(|t| influence_at(&app, **t).envoy)
        .map(|t| t.id)
        .collect();
    let best = nets
        .iter()
        .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
        .unwrap()
        .0;
    assert_eq!(flagged, vec![best], "one flagged: the highest net");

    // The day's first hour: the envoys arrive, with no captain near.
    let mut seen = Vec::new();
    watch_invitations(&mut app, 2, &mut seen);
    println!("  invitations seen on day 1: {seen:?}");
    assert_eq!(seen, vec![best]);
    let town = liked.iter().find(|t| t.id == best).copied().unwrap();
    let far = captain_distance(&mut app, town.at);
    println!(
        "  nearest captain to #{best}: {far:.0} tiles (radius {})",
        rules.radius
    );
    assert!(far > rules.radius, "no army near");
    assert!(!influence_at(&app, town).envoy, "the flag is spent");

    // The player accepts: ordinary card, the town is his by invitation.
    let offer = app
        .world()
        .resource::<Offers>()
        .pending
        .iter()
        .find(|o| o.kind == OfferKind::Invitation { settlement: best })
        .map(|o| o.id)
        .expect("the invitation");
    choose(&mut app, offer, 0);
    app.step(2);
    assert_eq!(owner_of(&app, town.e), FactionId::PLAYER);
    let how = app.world().get::<Tenure>(town.e).unwrap().how;
    assert_eq!(how, TakenBy::Invited);

    // Not two in a day: the rest of the day brings none.
    let day = ticks_per_day(&app);
    let hour = app.world().resource::<Calendar>().ticks_per_hour();
    let hours_left = (day - tick(&app)) / hour;
    watch_invitations(&mut app, hours_left, &mut seen);
    assert_eq!(seen, vec![best], "one a day: {seen:?}");
    assert!(tick(&app) < day);

    // The next dawn sends another (the first is the player's now).
    watch_invitations(&mut app, 2, &mut seen);
    println!("  invitations seen by the second morning: {seen:?}");
    assert_eq!(seen.len(), 2, "{seen:?}");
    assert_ne!(seen[1], best);
    assert!(liked.iter().any(|t| t.id == seen[1]));
    let second = liked.iter().find(|t| t.id == seen[1]).copied().unwrap();
    assert!(captain_distance(&mut app, second.at) > rules.radius);
}

/// The step-3 done-when on the shipped tuning: a free town between two of
/// yours, strangers at the start, invites you with no army near within ten
/// days. Ten game days of the real schedule; run it with `--ignored` at
/// merge points (CI does not run it).
#[test]
#[ignore = "ten game days; run with --ignored at merge points (not in CI)"]
fn a_free_town_between_two_of_yours_invites_within_ten_days() {
    for seed in [1u64, 42] {
        let mut app = run(seed);
        let (t, a, b) = trio(&mut app);
        let rules = app.world().resource::<Progression>().data.invitations;
        hold(&mut app, a, 0.8);
        hold(&mut app, b, 0.8);
        let mut seen = Vec::new();
        let mut when = None;
        for day in 1..=10u64 {
            feed_band(&mut app);
            watch_invitations(&mut app, 24, &mut seen);
            if when.is_none() && seen.contains(&t.id) {
                when = Some(day);
            }
            if when.is_some() {
                break;
            }
        }
        let inf = influence_at(&app, t);
        println!(
            "seed {seed}: #{} (net {:.3}, goodwill {:.3}) invited on day {when:?}",
            t.id,
            inf.net(),
            goodwill(&app, t)
        );
        assert!(
            app.world().resource::<sim_core::RunState>().in_run(),
            "the run ended"
        );
        assert!(when.is_some(), "no invitation in ten days: {seen:?}");
        assert!(
            captain_distance(&mut app, t.at) > rules.radius,
            "no army near"
        );
    }
}

/// GP-31: influence never spreads by word of mouth. Two deeds land on the
/// same tick: the day's `Deed::Influence` at the warmed target, and a
/// control `Deed::Trade` posted at the same town. Neighbours within 150
/// tiles hear a quarter of the trade, and not a word of the warmth: no
/// town anywhere logs a heard influence deed.
#[test]
fn influence_deeds_do_not_spread() {
    let mut app = run(42);
    let (t, a, b) = trio(&mut app);
    hold(&mut app, a, 0.9);
    hold(&mut app, b, 0.9);
    app.world_mut().resource_mut::<GoodwillLedger>().push(
        t.id,
        FactionId::PLAYER,
        0.1,
        Deed::Trade,
    );
    app.step(2);

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
    let mut heard_trades = 0;
    let mut direct_warmth = 0;
    for (e, id, at, _) in villages(&mut app) {
        let town = Town { e, id, at };
        for entry in deeds(&app, town) {
            if entry.faction != FactionId::PLAYER {
                continue;
            }
            match (entry.deed, entry.heard) {
                (Deed::Influence, true) => panic!("town #{id} heard of an influence deed"),
                (Deed::Influence, false) => direct_warmth += 1,
                (Deed::Trade, true) => {
                    assert!(id != t.id && dist(at, t.at) <= radius, "town #{id}");
                    assert!((entry.delta - 0.1 * fraction).abs() < 1e-6);
                    heard_trades += 1;
                }
                _ => {}
            }
        }
        // Inside the radius, the trade was heard; the warmth never was.
        if id != t.id && dist(at, t.at) <= radius {
            let heard = deeds(&app, town)
                .iter()
                .any(|e| e.deed == Deed::Trade && e.heard);
            assert!(heard, "town #{id} within {radius} tiles heard the trade");
        }
    }
    println!("{direct_warmth} towns warmed directly; {heard_trades} heard the control trade");
    assert!(direct_warmth >= 1 && heard_trades >= 1);

    // Goodwill adds up: the target holds its warmth and the trade, no more.
    let warmth = warmth_logged(&app, t)[0].delta;
    assert!((goodwill(&app, t) - (warmth + 0.1)).abs() < 1e-5);
}

/// GP-31: the rival's seat is never swayed. The player holds the four free
/// towns nearest it, the field is tilted the player's way and the seat
/// even likes the player above the invitation line; still it gets no
/// influence deed, sends no envoys and carries no Contest term, and stays
/// the rival's, a day on.
#[test]
fn the_seat_is_never_swayed() {
    let mut app = run(42);
    // The player holds every other town within the reach of the seat, the
    // seat projects nothing and a town's own independence is slight, so the
    // player's towns carry the field around it. Left to the formulas, any
    // other town there would warm, and a rival's would feel the pull.
    edit(&mut app, |d| {
        d.independence = 0.01;
        d.seat_power = 0.0;
    });
    let reach = data(&app).reach;
    let seat = seat_id(&app);
    let seat_at = app.world().resource::<RivalState>().seat_center;
    let mut held = 0;
    let mut seat_town = None;
    for (e, id, at, _) in villages(&mut app) {
        let town = Town { e, id, at };
        if id == seat {
            seat_town = Some(town);
        } else if dist(at, seat_at) <= reach {
            hold(&mut app, town, 1.0);
            held += 1;
        }
    }
    let seat_town = seat_town.expect("the seat");
    println!("{held} towns within {reach} tiles of the seat held");
    assert!(held >= 3);
    let line = app
        .world()
        .resource::<Progression>()
        .data
        .invitations
        .goodwill;
    set_goodwill(&mut app, seat_town, line + 0.05);
    app.step(1);
    let inf = influence_at(&app, seat_town);
    println!(
        "the seat: {inf:?}, goodwill {:.3}",
        goodwill(&app, seat_town)
    );
    let d = data(&app);
    assert!(
        inf.net() >= d.envoy_net,
        "the player's pull would sway any other town: {inf:?}"
    );
    assert!(
        influence::warmth(&d, inf.net(), line + 0.05) > 0.0,
        "any other town of that net would warm"
    );
    assert!(
        influence::contest_value(&d, FactionId::RIVAL, inf.rival, inf.player) < 0.0,
        "and a rival's other town would feel the pull"
    );
    assert!(!inf.envoy);
    assert_eq!(contest(&app, seat_town), 0.0);

    let day = ticks_per_day(&app);
    let hour = app.world().resource::<Calendar>().ticks_per_hour();
    let mut seen = Vec::new();
    watch_invitations(&mut app, day / hour - 1, &mut seen);
    println!("  a day on: invitations seen {seen:?}");
    assert!(!seen.contains(&seat), "the seat sent envoys");
    assert!(
        warmth_logged(&app, seat_town).is_empty(),
        "no influence deed"
    );
    assert_eq!(contest(&app, seat_town), 0.0);
    assert!(!influence_at(&app, seat_town).envoy);
    assert_eq!(owner_of(&app, seat_town.e), FactionId::RIVAL);
    assert!(app.world().resource::<sim_core::RunState>().in_run());
}

/// Design question 16: holdings feed your strength through land and
/// people. (1) The champion earns `rank_points` for every 500 people held,
/// on the day tick, and none with influence off. (2) Drifters come to a
/// band beside a village a little under the drifter line when it lies
/// inside the sphere (the line drops by `drifter_ease`), and not when the
/// ease is off.
#[test]
fn a_realm_ranks_the_champion_and_draws_drifters() {
    // (1) The rank credit.
    let realm = |enabled: bool| {
        let mut app = run(42);
        edit(&mut app, |d| d.enabled = enabled);
        let champion = champion(&mut app);
        let mut held = 0;
        for t in free_towns(&mut app) {
            if held >= 1_000 {
                break;
            }
            hold(&mut app, t, 0.5);
            held += villages(&mut app)
                .iter()
                .find(|v| v.0 == t.e)
                .map_or(0, |v| v.3);
        }
        let points = |app: &App| app.world().get::<ChampionRank>(champion).unwrap().points;
        let before = points(&app);
        app.step(1);
        // The people the credit counted: those held at the day tick.
        let people: u32 = villages(&mut app)
            .iter()
            .filter(|v| owner_of(&app, v.0) == FactionId::PLAYER)
            .map(|v| v.3)
            .sum();
        (points(&app) - before, people)
    };
    let ((on, held), (off, _)) = (realm(true), realm(false));
    let d = InfluenceData::default();
    let units = influence::realm_units(&d, held);
    println!(
        "{held} people held: +{on} rank points with influence, +{off} without ({units} units)"
    );
    assert!(units >= 2);
    assert_eq!(on - off, d.rank_points * units);

    // (2) The drifters: the same village, the same band, the ease on or off.
    let drifted = |ease: f32| {
        let (mut app, liked) = sphere_with_candidates(42);
        edit(&mut app, |d| d.drifter_ease = ease);
        app.world_mut()
            .resource_mut::<Progression>()
            .data
            .drifters
            .per_settlement = (2, 2);
        let drifters = app.world().resource::<Progression>().data.drifters;
        let v = liked[0];
        // The village in question likes you a little under the drifter
        // line; no other place near it likes you at all.
        set_goodwill(&mut app, v, drifters.goodwill - 0.05);
        for (e, id, at, _) in villages(&mut app) {
            if id != v.id && dist(at, v.at) <= drifters.radius + 10.0 {
                set_goodwill(&mut app, Town { e, id, at }, -0.5);
            }
        }
        let at = passable_near(&app, v.at).expect("somewhere to stand");
        spawn_army(
            app.world_mut(),
            &ArmySpec {
                posture: Posture::Passive,
                ..ArmySpec::new(FactionId::PLAYER, at, 10)
            },
        );
        app.step(2);
        let inf = influence_at(&app, v);
        (
            app.world().resource::<RunProgress>().drifters,
            inf.net(),
            goodwill(&app, v),
        )
    };
    let (eased, net, g) = drifted(d.drifter_ease);
    let (plain, ..) = drifted(0.0);
    println!(
        "village at net {net:.2}, goodwill {g:.2}: {eased} drifters eased, {plain} without the ease"
    );
    assert!(net >= d.sphere, "inside the sphere");
    assert_eq!(eased, 2, "a village inside the sphere sends volunteers");
    assert_eq!(plain, 0, "the same village without the ease sends none");
}
