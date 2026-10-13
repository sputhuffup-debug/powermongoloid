//! Sellswords (GP-27): the `Sellswords(n)` run modifier's band for hire,
//! tuned by progression.ron `hire`.
//!
//! - **Setup** ([`setup`], the last step of `setup_run`, after every other
//!   spawn, so no earlier SpawnId or draw moves): for each drawn run
//!   modifier with `Sellswords` effects, in draw order, `n` men (their sum,
//!   the captain included) camp `hire.distance` tiles from the start, in a
//!   direction drawn from stream `sim_military::sellswords ^ card` (tick
//!   0), snapped to the nearest tile the champion can walk to. The band is
//!   an ordinary army ([`spawn_army`]) of `FactionId::INDEPENDENT`: at
//!   peace with everyone (`Army::at_war` never counts faction 0), passive
//!   and without orders, so it holds its camp. Every man has `hire.unit`'s
//!   stats and `hire.armed`'s weapon and carries [`NoUpkeep`], so the band
//!   never eats and never gathers. Being nobody's ally, it is not shaken by
//!   independents falling nearby (`upkeep::soldier_upkeep`), frightens no
//!   settlement (`threat::update_threats`) and gives the independents no
//!   seat. A `Hire { captain, men, price }` offer
//!   (choices `[0 = hire, 1 = decline]`) is queued for the player
//!   (`Offers::post`), reported on tick 0 by
//!   [`crate::progression::report_start`], and stands for
//!   `hire.expiry_hours`; `price` ([`price`]) is `hire.price_days` of
//!   food per man.
//! - **Hiring** (`answer` with choice 0, from `progression::choose`):
//!   refused, the offer left standing, unless the champion stands within
//!   `hire.radius` of the band's captain and holds the price in food in
//!   its baggage (`check`). The price is paid from the baggage, least
//!   nourishing food first; then every man becomes a follower of the
//!   champion (the band's captain too), sworn to the player and eating
//!   from the baggage, which grows by army.ron `baggage_per_head` each.
//!   No captain is made, so captain slots (GP-25) are untouched.
//! - **Declined or lapsed**: the band leaves the world at once (`dismiss`
//!   from `answer` with choice 1, `lapsed` from the expiry pass of
//!   [`crate::invitations::step`], which drops every expired offer).
//!
//! Nothing new is stored: the band is captains, followers and markers that
//! are already hashed and saved, and the offer names it.

use bevy_ecs::prelude::*;
use bevy_math::{IVec2, Vec2};
use rand::RngExt;
use sim_agents::{CombatStats, Villagers};
use sim_core::rng::rng_key;
use sim_core::{
    Allegiance, Calendar, FactionId, OfferKind, Offers, Position, Posture, SimSeed, SimTick,
    SpawnId,
};
use sim_economy::{Economy, Stockpile};
use sim_world::{FlowField, WorldMap};

use crate::components::{ArmyStats, Captain, Factions, Follower};
use crate::data::Army;
use crate::monuments::{Guard, NoUpkeep};
use crate::orders::{faction_name, log};
use crate::progression::{Effect, HireData, Progression, champion_of};
use crate::spawn::{ArmySpec, nearest_reachable, soldier_stats, spawn_army};

/// Men in the band a run modifier with `effects` camps near the start: the
/// sum of its `Sellswords(n)` effects (0 for none).
pub fn band_size(effects: &[Effect]) -> u32 {
    effects
        .iter()
        .map(|e| match *e {
            Effect::Sellswords(n) => n,
            _ => 0,
        })
        .fold(0, u32::saturating_add)
}

/// The food value hiring `men` sellswords costs: `hire.price_days` of
/// `hunger_per_day` (villagers.ron `needs`) each, rounded up.
pub fn price(hire: &HireData, hunger_per_day: f32, men: u32) -> u16 {
    (men as f32 * hunger_per_day * hire.price_days)
        .ceil()
        .clamp(0.0, u16::MAX as f32) as u16
}

/// GP-27, run setup: camps a band for hire near `start` for each run
/// modifier in `cards` (draw order) with `Sellswords` effects, and queues
/// its `Hire` offer (see the module docs; reported on tick 0). Called last
/// by `setup_run`, after every other spawn. Span
/// `sim_military::sellswords::setup`.
pub fn setup(world: &mut World, data: &Progression, cards: &[u16], start: IVec2) {
    let _span = tracing::info_span!("sim_military::sellswords::setup").entered();
    let hire = data.data.hire;
    let seed = *world.resource::<SimSeed>();
    let tick = world.resource::<SimTick>().0;
    let expires = tick + world.resource::<Calendar>().hours(hire.expiry_hours);
    let villagers = world.resource::<Villagers>();
    let (costs, per_day) = (
        villagers.travel_costs.clone(),
        villagers.data.needs.hunger_per_day,
    );
    for &card in cards {
        let Some(m) = data.modifier(card) else {
            continue;
        };
        let n = band_size(&m.effects).min(u16::MAX as u32);
        if n == 0 {
            continue;
        }
        let mut rng = seed.stream(rng_key("sim_military::sellswords") ^ card as u64, 0);
        let angle = rng.random_range(0.0..std::f32::consts::TAU);
        let (id, price) = camp(world, &hire, &costs, per_day, start, angle, n);
        // Queued quietly: `report_start` reports it on tick 0.
        world.resource_mut::<Offers>().post(
            FactionId::PLAYER,
            OfferKind::Hire {
                captain: id,
                men: n as u16,
                price,
            },
            vec![0, 1],
            tick,
            Some(expires),
        );
        let who = faction_name(world, FactionId::PLAYER);
        log(
            world,
            format!("{n} sellswords camp near {who}'s start, for hire at {price} food"),
        );
    }
}

/// Camps a band of `n` men for hire (the captain one of them)
/// `hire.distance` tiles from `from` at `angle`, snapped to the nearest tile
/// one can walk to from `from`: an independent, passive army with
/// `hire.unit`'s stats and `hire.armed`'s weapon, every man [`NoUpkeep`].
/// Returns the captain's SpawnId and the band's price ([`price`]). The
/// caller posts the `Hire` offer. Used by [`setup`] (GP-27) and by Market
/// towns ([`crate::govern`], SIM-62).
pub(crate) fn camp(
    world: &mut World,
    hire: &HireData,
    costs: &sim_world::TravelCosts,
    hunger_per_day: f32,
    from: IVec2,
    angle: f32,
    n: u32,
) -> (u64, u16) {
    let want = from + (Vec2::from_angle(angle) * hire.distance).round().as_ivec2();
    // A tile the champion can walk to (no cut-off spit or islet).
    let at = {
        let map = world.resource::<WorldMap>();
        let r = hire.distance.ceil() as i32 + 24;
        let reach = FlowField::build(
            map,
            costs,
            from,
            from - IVec2::splat(r),
            from + IVec2::splat(r),
        );
        nearest_reachable(&reach, want).unwrap_or(from)
    };
    // `spawn_army` seats a faction that has none; the independents have
    // no seat, so theirs is put back as it was.
    let seats = world.resource::<Factions>().clone();
    let captain = spawn_army(
        world,
        &ArmySpec {
            posture: Posture::Passive,
            food_days: 0.0,
            armed: hire.armed,
            soldier: Some(hire.unit),
            ..ArmySpec::new(FactionId::INDEPENDENT, at, n - 1)
        },
    );
    *world.resource_mut::<Factions>() = seats;
    // The captain is one of them, not army.ron's captain.
    *world.get_mut::<CombatStats>(captain).expect("a soldier") = soldier_stats(hire.unit);
    let id = world.get::<SpawnId>(captain).expect("spawned").0;
    for m in members_of(world, captain) {
        world.entity_mut(m).insert(NoUpkeep);
    }
    (id, price(hire, hunger_per_day, n))
}

/// The band for hire led by `captain` (a SpawnId): its captain (an
/// independent captain with [`NoUpkeep`], not a guard) and every member,
/// the captain first, then its followers by SpawnId. `None` once it is
/// gone (hired, sent away or moved on).
pub(crate) fn band(world: &mut World, captain: u64) -> Option<(Entity, Vec<Entity>)> {
    let mut q =
        world.query_filtered::<(Entity, &SpawnId, &Captain), (With<NoUpkeep>, Without<Guard>)>();
    let e = q
        .iter(world)
        .find(|(_, id, c)| id.0 == captain && c.faction == FactionId::INDEPENDENT)
        .map(|(e, ..)| e)?;
    Some((e, members_of(world, e)))
}

/// `captain` and its followers by SpawnId.
fn members_of(world: &mut World, captain: Entity) -> Vec<Entity> {
    let mut q = world.query::<(Entity, &SpawnId, &Follower)>();
    let mut followers: Vec<(SpawnId, Entity)> = q
        .iter(world)
        .filter(|(_, _, f)| f.captain == captain)
        .map(|(m, id, _)| (*id, m))
        .collect();
    followers.sort_unstable();
    std::iter::once(captain)
        .chain(followers.into_iter().map(|(_, m)| m))
        .collect()
}

/// Whether `issuer` may hire the band led by `captain` (a SpawnId) for
/// `price` now. Returns the issuer's champion and the band's captain.
/// Refusals, in this order: "no champion" (none, or it leads no army),
/// "the sellswords are gone", "too far from the sellswords" (farther than
/// `hire.radius` from their captain), "too little food to pay the
/// sellswords" (less than `price` in food value in the champion's
/// baggage).
pub(crate) fn check(
    world: &mut World,
    issuer: FactionId,
    captain: u64,
    price: u16,
) -> Result<(Entity, Entity), &'static str> {
    let champion = champion_of(world, issuer)
        .filter(|&c| world.get::<Captain>(c).is_some())
        .ok_or("no champion")?;
    let (band_captain, _) = band(world, captain).ok_or("the sellswords are gone")?;
    let radius = world.resource::<Progression>().data.hire.radius;
    let at = |e: Entity| world.get::<Position>(e).map_or(IVec2::ZERO, |p| p.0);
    if (at(champion) - at(band_captain)).as_vec2().length() > radius {
        return Err("too far from the sellswords");
    }
    let econ = world.resource::<Economy>();
    let food = world
        .get::<Stockpile>(champion)
        .map_or(0.0, |s| s.food_value(econ));
    if food + 1e-3 < price as f32 {
        return Err("too little food to pay the sellswords");
    }
    Ok((champion, band_captain))
}

/// Takes at least `price` in food value from `captain`'s baggage, least
/// nourishing food first, overpaying by less than one unit of the last food
/// taken. Returns the food value taken.
fn pay(world: &mut World, captain: Entity, price: f32) -> f32 {
    let econ = world.resource::<Economy>().clone();
    let Some(mut bag) = world.get_mut::<Stockpile>(captain) else {
        return 0.0;
    };
    let mut left = price;
    let mut paid = 0.0;
    for g in econ.foods_by_value().into_iter().rev() {
        if left <= 0.0 {
            break;
        }
        let v = econ.food_value(g);
        let qty = ((left / v).ceil() as u32).min(bag.get(g));
        if qty == 0 {
            continue;
        }
        bag.take(g, qty);
        left -= qty as f32 * v;
        paid += qty as f32 * v;
    }
    paid
}

/// Hires the band (`members`, its captain first) into `champion`'s army
/// for `price` (see the module docs). Call only after [`check`]. Returns
/// the men hired. Span `sim_military::sellswords::hire`.
fn hire(
    world: &mut World,
    champion: Entity,
    band_captain: Entity,
    members: &[Entity],
    price: u16,
) -> u32 {
    let _span = tracing::info_span!("sim_military::sellswords::hire").entered();
    pay(world, champion, price as f32);
    // Whatever the band carried (normally nothing) goes to the baggage.
    let carried = world
        .get::<Stockpile>(band_captain)
        .map(|s| s.goods.clone())
        .unwrap_or_default();
    if let Some(mut bag) = world.get_mut::<Stockpile>(champion) {
        for (g, q) in carried {
            bag.capacity = bag.capacity.max(bag.total() + q);
            bag.add(g, q);
        }
    }
    world
        .entity_mut(band_captain)
        .remove::<(Captain, Stockpile)>()
        .insert(Follower {
            captain: champion,
            slot: 0,
            deserting: false,
        });
    for &m in members {
        world
            .entity_mut(m)
            .remove::<NoUpkeep>()
            .insert(Allegiance(FactionId::PLAYER));
        if let Some(mut f) = world.get_mut::<Follower>(m) {
            f.captain = champion;
        }
    }
    let n = members.len() as u32;
    let per_head = world.resource::<Army>().data.baggage_per_head;
    if let Some(mut bag) = world.get_mut::<Stockpile>(champion) {
        bag.capacity += per_head * n;
    }
    world
        .resource_mut::<ArmyStats>()
        .of(FactionId::PLAYER)
        .recruited += n;
    // This tick's orders step rebuilds the rally field over the newcomers.
    if let Some(mut c) = world.get_mut::<Captain>(champion) {
        c.rally = None;
    }
    n
}

/// GP-27: `issuer` answers the Hire offer for the band led by `captain`
/// (`men` of them, for `price`) with `card`: 0 hires them (`choose` has
/// already checked it may, [`check`]), anything else sends them away
/// ([`dismiss`]). Returns what was done, for the log.
pub(crate) fn answer(
    world: &mut World,
    issuer: FactionId,
    captain: u64,
    men: u16,
    price: u16,
    card: u16,
) -> String {
    if card != 0 {
        dismiss(world, captain);
        return format!("to send the {men} sellswords away");
    }
    match check(world, issuer, captain, price) {
        Ok((champion, band_captain)) => {
            let members = band(world, captain).map(|b| b.1).unwrap_or_default();
            let n = hire(world, champion, band_captain, &members, price);
            format!("to hire {n} sellswords for {price} food")
        }
        // Unreachable: `choose` checks first, in the same tick. The offer
        // is taken already, so the band leaves rather than wait forever
        // under no offer.
        Err(why) => {
            dismiss(world, captain);
            format!("to hire the sellswords, but {why}, and they left")
        }
    }
}

/// Despawns every member of the band led by `captain` (a SpawnId),
/// followers first, and returns how many left (0 if it is gone already).
/// Followers name only their own captain, so nothing points at them
/// afterwards. Span `sim_military::sellswords::dismiss`.
pub(crate) fn dismiss(world: &mut World, captain: u64) -> u32 {
    let _span = tracing::info_span!("sim_military::sellswords::dismiss").entered();
    let Some((_, members)) = band(world, captain) else {
        return 0;
    };
    for &m in members.iter().rev() {
        world.despawn(m);
    }
    members.len() as u32
}

/// The Hire offer for the band led by `captain` lapsed unanswered (the
/// expiry pass of [`crate::invitations::step`]): the band moves on.
pub(crate) fn lapsed(world: &mut World, captain: u64) {
    if dismiss(world, captain) > 0 {
        let who = faction_name(world, FactionId::PLAYER);
        log(
            world,
            format!("The sellswords grew tired of waiting for {who} and moved on"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progression::tests::run_app;
    use sim_core::Offers;

    fn hire_offer(world: &World) -> Option<(u64, u64, u16, u16)> {
        world
            .resource::<Offers>()
            .pending
            .iter()
            .find_map(|o| match o.kind {
                OfferKind::Hire {
                    captain,
                    men,
                    price,
                } => Some((o.id, captain, men, price)),
                _ => None,
            })
    }

    fn entities(world: &mut World) -> Vec<(SpawnId, Entity)> {
        let mut q = world.query::<(Entity, &SpawnId)>();
        let mut v: Vec<_> = q.iter(world).map(|(e, id)| (*id, e)).collect();
        v.sort_unstable();
        v
    }

    /// GP-27: dismissing the band despawns its members and nothing else;
    /// a second dismissal finds nobody.
    #[test]
    fn dismiss_despawns_every_member_and_nothing_else() {
        let (mut app, _) = run_app(5);
        let world = app.world_mut();
        let (_, captain, men, _) = hire_offer(world).expect("seed 5 draws Sellswords");
        let (_, members) = band(world, captain).expect("the band");
        assert_eq!(members.len(), men as usize);
        let before = entities(world);
        assert_eq!(dismiss(world, captain), men as u32);
        let after = entities(world);
        let kept: Vec<_> = before
            .iter()
            .filter(|(_, e)| !members.contains(e))
            .copied()
            .collect();
        assert_eq!(after, kept, "only the band left");
        assert_eq!(dismiss(world, captain), 0);
    }

    /// GP-27: only the band for hire can be hired: the champion, a guard
    /// camp's captain or an unknown id are refused and change nothing.
    #[test]
    fn only_a_band_for_hire_can_be_hired() {
        let (mut app, start) = run_app(5);
        let world = app.world_mut();
        let (_, captain, _, price) = hire_offer(world).expect("seed 5 draws Sellswords");
        let champion = world.get::<SpawnId>(start.champion).unwrap().0;
        let mut q = world.query_filtered::<&SpawnId, (With<Captain>, With<Guard>)>();
        let guard = q.iter(world).map(|id| id.0).min();
        let hash = sim_core::compute_state_hash(world);
        for id in [Some(champion), guard, Some(u64::MAX)]
            .into_iter()
            .flatten()
        {
            assert_eq!(
                check(world, FactionId::PLAYER, id, price),
                Err("the sellswords are gone"),
                "captain {id}"
            );
            assert!(band(world, id).is_none());
            assert_eq!(dismiss(world, id), 0);
        }
        assert_eq!(sim_core::compute_state_hash(world), hash, "nothing changed");
        assert!(check(world, FactionId::PLAYER, captain, price).is_ok());
        assert_eq!(
            check(world, FactionId::RIVAL, captain, price),
            Err("no champion")
        );
    }

    /// GP-27: the band for hire is nobody's ally. The independents get no
    /// seat from it; independents falling at its camp don't shake it (or
    /// it could rout and desert while its offer still names every man),
    /// while the player's men are shaken by their own; and it frightens no
    /// owned settlement, even standing in its square.
    #[test]
    fn a_band_for_hire_is_nobodys_ally() {
        use crate::combat::Casualties;
        use crate::index::ArmyIndex;
        use bevy_ecs::system::RunSystemOnce;
        use sim_agents::{Owner, Threat};
        use sim_core::SimStepper;
        let (mut app, start) = run_app(5);
        assert_eq!(
            app.world()
                .resource::<Factions>()
                .seat(FactionId::INDEPENDENT),
            None,
            "no seat for the independents"
        );
        app.step(1);
        let world = app.world_mut();
        let (_, captain, ..) = hire_offer(world).expect("seed 5 draws Sellswords");
        let (e, members) = band(world, captain).unwrap();
        let centre = |world: &World, e: Entity| {
            world.get::<Position>(e).unwrap().0.as_vec2() + Vec2::splat(0.5)
        };
        let (camp, home) = (centre(world, e), centre(world, start.champion));
        let player = members_of(world, start.champion);
        assert!(player.len() > 1, "the champion leads men");
        let morale = |world: &World, v: &[Entity]| -> Vec<f32> {
            v.iter()
                .map(|&m| world.get::<CombatStats>(m).unwrap().morale)
                .collect()
        };
        let (band_before, player_before) = (morale(world, &members), morale(world, &player));
        {
            let mut c = world.resource_mut::<Casualties>();
            for _ in 0..20 {
                c.fallen.push((camp, FactionId::INDEPENDENT, true));
                c.fallen.push((home, FactionId::PLAYER, false));
            }
        }
        world
            .run_system_once(crate::upkeep::soldier_upkeep)
            .unwrap();
        for (i, (b, a)) in band_before.iter().zip(morale(world, &members)).enumerate() {
            assert!(a >= *b, "sellsword {i} shaken: {b} -> {a}");
        }
        for (i, (b, a)) in player_before.iter().zip(morale(world, &player)).enumerate() {
            assert!(a < *b, "the player's man {i} not shaken: {b} -> {a}");
        }

        // The band alone, in the square of a settlement the player holds.
        let mut q = world.query::<(&SpawnId, &sim_world::Settlement, &mut Owner)>();
        let (_, at, mut owner) = q
            .iter_mut(world)
            .min_by_key(|(id, ..)| id.0)
            .map(|(id, s, o)| (id.0, s.center, o))
            .expect("a settlement");
        owner.faction = FactionId::PLAYER;
        let mut index = world.resource_mut::<ArmyIndex>();
        index.armies.retain(|a| a.entity == e);
        assert_eq!(index.armies.len(), 1, "the band is indexed");
        index.armies[0].pos = at.as_vec2() + Vec2::splat(0.5);
        world
            .run_system_once(crate::threat::update_threats)
            .unwrap();
        let mut q = world.query::<(&Owner, &Threat)>();
        for (o, t) in q.iter(world) {
            assert_eq!(
                t.level, 0.0,
                "a settlement of faction {:?} frightened",
                o.faction
            );
        }
    }

    /// GP-27: a band that never eats never gathers either, even standing
    /// at an independent village (`forage` would otherwise draw on it as
    /// its owner's supplies, both being faction 0).
    #[test]
    fn a_band_for_hire_never_gathers_food() {
        use sim_core::SimStepper;
        use sim_economy::SettlementEconomy;
        let (mut app, _) = run_app(5);
        let world = app.world_mut();
        let (_, captain, ..) = hire_offer(world).expect("seed 5 draws Sellswords");
        let (e, _) = band(world, captain).unwrap();
        let mut q = world
            .query_filtered::<(Entity, &SpawnId, &sim_world::Settlement), With<SettlementEconomy>>(
            );
        let (_, s, centre) = q
            .iter(world)
            .map(|(s, id, st)| (id.0, s, st.center))
            .min_by_key(|x| x.0)
            .expect("a settlement");
        assert_eq!(crate::owner_of(world, s), FactionId::INDEPENDENT);
        world.get_mut::<Position>(e).unwrap().0 = centre + IVec2::new(2, 0);
        app.step(300);
        let world = app.world_mut();
        assert_eq!(
            world.get::<Stockpile>(e).map(|s| s.total()),
            Some(0),
            "the band gathered nothing"
        );
        assert_eq!(world.get::<Captain>(e).unwrap().last_forage, 0);
    }
}
