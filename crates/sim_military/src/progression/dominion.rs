//! Dominion (GP-24) and captain slots (GP-25): what holding land gives the
//! player in a run.
//!
//! - [`tally`] (slot 8 of the hourly progress driver, in a run only) sums
//!   each region's settlement population (`SettlementEconomy.population`)
//!   and the part of it the player owns into [`RunProgress::dominion`]. The
//!   first time the player's share of a region reaches a threshold of
//!   progression.ron `dominion.thresholds` (25% and 50%), the tally sets
//!   that threshold's flag and posts an `OfferKind::DominionBoon {region,
//!   step}` of three distinct Dominion boons, drawn by weight. Holdings
//!   raise the weights ([`boon_weight`]): farms, a smithy, the region's
//!   largest settlement, battles fought, enemies killed. Flags never reset,
//!   so losing land and taking it back offers nothing new.
//! - [`captain_slots`]: the player's captain slots, `min(cap, max(first
//!   village slots if any settlement is held, floor(held population /
//!   per_captain)) + boon slots + the CaptainSlots modifier)`.
//!   `AppointCaptain` is refused once every slot is filled; the champion's
//!   own army takes none. Outside a run, and for any other faction, the old
//!   rule (army.ron `max_captains`) alone applies.
//!
//! Streams: a dominion offer's choices come from
//! `sim_military::progression::boons ^ offer id` at the posting tick (the
//! start offer, id 0, drew from `boons ^ 0` at tick 0).

use bevy_ecs::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sim_agents::Owner;
use sim_core::rng::rng_key;
use sim_core::{
    Chronicle, FactionId, FactionModifiers, OfferKind, Offers, RunState, SimEventKind, SimSeed,
    SimTick, SpawnId, Stat, post_offer,
};
use sim_economy::{SettlementEconomy, Workplace, WorkplaceKind};
use sim_world::{NO_REGION, RegionId, Settlement, WorldMap};

use super::start::draw_cards;
use super::{BoonData, BoonWhen, Effect, Progression, RunProgress};
use crate::components::ArmyStats;
use crate::orders::{faction_name, log};

/// Dominion and captain slots (GP-24, GP-25): `dominion` in progression.ron.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DominionData {
    /// Shares (0..1, ascending, at most 8) of a region's population held
    /// that each offer one dominion boon, the first time they are reached.
    pub thresholds: Vec<f32>,
    /// One captain slot per this many people in settlements the player
    /// holds.
    pub per_captain: u32,
    /// At most this many captain slots, whatever the land and boons.
    pub cap: u32,
    /// Slots that come with the first settlement held, before
    /// `per_captain` gives as many (the first lieutenant).
    pub first_village_slots: u32,
}

impl DominionData {
    /// Checks the block: thresholds strictly ascending in (0, 1], at most
    /// 8 of them; `per_captain` > 0; `first_village_slots` <= `cap`.
    pub fn validate(&self) -> Result<(), String> {
        if self.thresholds.len() > 8 {
            return Err("progression: dominion allows at most 8 thresholds".into());
        }
        let mut last = 0.0;
        for &t in &self.thresholds {
            if !(t.is_finite() && t > last && t <= 1.0) {
                return Err("progression: dominion thresholds must be ascending, in (0, 1]".into());
            }
            last = t;
        }
        if self.per_captain == 0 || self.first_village_slots > self.cap {
            return Err(
                "progression: dominion needs per_captain > 0 and first_village_slots <= cap".into(),
            );
        }
        Ok(())
    }
}

/// What a Dominion boon's `favour` can name (GP-24). Append only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Holding {
    /// Farms in the region's settlements the player owns (amount: the
    /// count).
    Farms,
    /// A smithy in them (amount 1 if any, else 0).
    Smithy,
    /// The region's largest settlement by population (ties: lowest
    /// SpawnId) is the player's (1 or 0).
    LargestSettlement,
    /// Battles the player has fought in the run: `SimEventKind::Battle`
    /// chronicle entries of the player's faction (M4 step 6 writes them).
    Battles,
    /// Enemies the player's soldiers have killed (`FactionStats.kills`).
    Kills,
}

/// The player's holdings that weigh a dominion offer (GP-24): a region's,
/// plus the faction-wide fighting record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Holdings {
    /// Farms in the region's settlements the player owns.
    pub farms: u32,
    /// Whether one of them has a smithy.
    pub smithy: bool,
    /// Whether the player owns the region's largest settlement.
    pub largest: bool,
    /// Battles fought ([`Holding::Battles`]).
    pub battles: u32,
    /// Enemies killed.
    pub kills: u32,
}

impl Holdings {
    /// How much of `h` the player has: a count, or 1/0.
    pub fn amount(&self, h: Holding) -> f32 {
        match h {
            Holding::Farms => self.farms as f32,
            Holding::Smithy => f32::from(u8::from(self.smithy)),
            Holding::LargestSettlement => f32::from(u8::from(self.largest)),
            Holding::Battles => self.battles as f32,
            Holding::Kills => self.kills as f32,
        }
    }
}

/// One region's dominion tally (GP-24), in [`RunProgress::dominion`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RegionTally {
    /// People in the region's settlements.
    pub total: u32,
    /// People in the region's settlements the player owns.
    pub held_by_player: u32,
    /// Bit `i` is set once the player's share has reached threshold `i`
    /// (and its offer was posted). Never cleared.
    pub flags: u8,
}

impl RegionTally {
    /// The player's share of the region's people (0 if nobody lives
    /// there).
    pub fn share(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.held_by_player as f32 / self.total as f32
        }
    }

    /// Whether `held_by_player` reaches `threshold` of `total` (exact
    /// integer-safe comparison in f64; never for an empty region).
    pub fn reaches(&self, threshold: f32) -> bool {
        self.total > 0 && self.held_by_player as f64 >= threshold as f64 * self.total as f64
    }
}

/// GP-24: a Dominion boon's draw weight given the player's holdings:
/// `weight x (1 + sum(w x amount))` over its `favour`.
pub fn boon_weight(boon: &BoonData, holdings: &Holdings) -> f32 {
    let favour: f32 = boon
        .favour
        .iter()
        .map(|&(h, w)| w * holdings.amount(h))
        .sum();
    boon.weight * (1.0 + favour)
}

/// GP-24: up to three distinct Dominion boons (cards in `boons`), drawn by
/// [`boon_weight`] without replacement.
pub fn draw_dominion_boons<R: Rng + ?Sized>(
    rng: &mut R,
    data: &Progression,
    holdings: &Holdings,
) -> Vec<u16> {
    let cards: Vec<(u16, f32, Option<&str>)> = data
        .data
        .boons
        .iter()
        .filter(|b| b.when == BoonWhen::Dominion)
        .map(|b| (b.id, boon_weight(b, holdings), None))
        .collect();
    draw_cards(rng, &cards, 3)
}

/// GP-25: the captain-slot formula. `min(cap, max(first_village_slots if
/// held_settlements > 0, floor(held_pop / per_captain)) + boon_slots +
/// floor(extra))`, never below 0. `extra` is the faction's flat
/// `Stat::CaptainSlots` modifier (traits, monuments, Stat boons); a hair
/// of float error below a whole number still counts as that number.
pub fn slots_for(
    data: &DominionData,
    held_settlements: u32,
    held_pop: u32,
    boon_slots: u32,
    extra: f32,
) -> u32 {
    let first = if held_settlements > 0 {
        data.first_village_slots
    } else {
        0
    };
    let base = first.max(held_pop / data.per_captain.max(1));
    let extra = if extra.is_finite() {
        (extra + 1e-3).floor() as i64
    } else {
        0
    };
    (base as i64 + boon_slots as i64 + extra).clamp(0, data.cap as i64) as u32
}

/// GP-25: `faction`'s captain slots ([`slots_for`] over the settlements it
/// owns, the `CaptainSlots(n)` effects of the boons it picked, and its
/// `Stat::CaptainSlots` modifier). `Some` only in a run and for the player;
/// `None` means the old rule (army.ron `max_captains` alone).
pub fn captain_slots(world: &mut World, faction: FactionId) -> Option<u32> {
    if !world.resource::<RunState>().in_run() || faction != FactionId::PLAYER {
        return None;
    }
    let mut q = world.query::<(&Owner, &SettlementEconomy)>();
    let (mut held, mut pop) = (0u32, 0u32);
    for (o, se) in q.iter(world) {
        if o.faction == faction {
            held += 1;
            pop = pop.saturating_add(se.population);
        }
    }
    let data = world.resource::<Progression>();
    let boon_slots: u32 = world
        .resource::<RunProgress>()
        .boons
        .iter()
        .filter_map(|&card| data.boon(card))
        .flat_map(|b| b.effects.iter())
        .map(|e| match *e {
            Effect::CaptainSlots(n) => u32::from(n),
            _ => 0,
        })
        .sum();
    let extra = world
        .resource::<FactionModifiers>()
        .value(faction, Stat::CaptainSlots);
    Some(slots_for(&data.data.dominion, held, pop, boon_slots, extra))
}

/// Stores the player's [`captain_slots`] in [`RunProgress::captain_slots`]
/// (for the UI), in a run only: after the hourly modifier rebuild and after
/// a boon pick.
pub fn refresh_slots(world: &mut World) {
    if let Some(slots) = captain_slots(world, FactionId::PLAYER)
        && world.resource::<RunProgress>().captain_slots != slots
    {
        world.resource_mut::<RunProgress>().captain_slots = slots;
    }
}

/// GP-24: the hourly dominion tally (slot 8 of the progress driver), in a
/// run only. Span `sim_military::dominion`. See the module docs.
pub fn tally(world: &mut World) {
    if !world.resource::<RunState>().in_run() {
        return;
    }
    let _span = tracing::info_span!("sim_military::dominion").entered();
    let regions = world.resource::<WorldMap>().region_count();
    let thresholds = world
        .resource::<Progression>()
        .data
        .dominion
        .thresholds
        .clone();
    let mut tallies = world.resource::<RunProgress>().dominion.clone();
    tallies.resize(regions, RegionTally::default());
    for t in &mut tallies {
        t.total = 0;
        t.held_by_player = 0;
    }
    for (_, region, owner, pop) in settlements(world) {
        let Some(t) = tallies.get_mut(region as usize) else {
            continue;
        };
        t.total = t.total.saturating_add(pop);
        if owner == FactionId::PLAYER {
            t.held_by_player = t.held_by_player.saturating_add(pop);
        }
    }
    let mut reached: Vec<(RegionId, u8)> = Vec::new();
    for (r, t) in tallies.iter_mut().enumerate() {
        for (step, &threshold) in thresholds.iter().enumerate() {
            let bit = 1u8 << step;
            if t.flags & bit == 0 && t.reaches(threshold) {
                t.flags |= bit;
                reached.push((r as RegionId, step as u8));
            }
        }
    }
    if world.resource::<RunProgress>().dominion != tallies {
        world.resource_mut::<RunProgress>().dominion = tallies;
    }
    for (region, step) in reached {
        offer_boons(world, region, step, thresholds[step as usize]);
    }
}

/// Populated settlements in a region as `(SpawnId, region, owner,
/// population)`, sorted by SpawnId (settlements outside any region are
/// left out).
fn settlements(world: &mut World) -> Vec<(u64, RegionId, FactionId, u32)> {
    let mut q = world.query::<(&SpawnId, &Settlement, &Owner, &SettlementEconomy)>();
    let mut v: Vec<(u64, RegionId, FactionId, u32)> = q
        .iter(world)
        .filter(|(_, s, ..)| s.region != NO_REGION)
        .map(|(id, s, o, se)| (id.0, s.region, o.faction, se.population))
        .collect();
    v.sort_unstable_by_key(|s| s.0);
    v
}

/// The player's holdings in `region` (and fighting record), for weighing
/// that region's dominion offer.
pub fn holdings(world: &mut World, region: RegionId) -> Holdings {
    let mut out = Holdings::default();
    let all = settlements(world);
    let largest = all
        .iter()
        .filter(|s| s.1 == region)
        .max_by(|a, b| a.3.cmp(&b.3).then(b.0.cmp(&a.0)));
    out.largest = largest.is_some_and(|s| s.2 == FactionId::PLAYER);
    let mut q = world.query::<(&SpawnId, &Settlement, &Owner, &SettlementEconomy)>();
    let mut workplaces: Vec<(u64, Vec<Entity>)> = q
        .iter(world)
        .filter(|(_, s, o, _)| s.region == region && o.faction == FactionId::PLAYER)
        .map(|(id, _, _, se)| (id.0, se.workplaces.clone()))
        .collect();
    workplaces.sort_unstable_by_key(|w| w.0);
    for (_, places) in workplaces {
        for e in places {
            match world.get::<Workplace>(e).map(|w| w.kind) {
                Some(WorkplaceKind::Farm) => out.farms += 1,
                Some(WorkplaceKind::Smithy) => out.smithy = true,
                _ => {}
            }
        }
    }
    out.battles = world
        .resource::<Chronicle>()
        .entries()
        .iter()
        .filter(|e| e.event.kind == SimEventKind::Battle && e.event.faction == FactionId::PLAYER)
        .count() as u32;
    out.kills = world.resource::<ArmyStats>().get(FactionId::PLAYER).kills;
    out
}

/// Posts region `region`'s dominion offer for threshold `step`: three
/// distinct Dominion boons weighed by the player's holdings there, drawn
/// from `boons ^ offer id` at this tick.
fn offer_boons(world: &mut World, region: RegionId, step: u8, threshold: f32) {
    let tick = world.resource::<SimTick>().0;
    let id = world.resource::<Offers>().next_id;
    let held = holdings(world, region);
    let data = world.resource::<Progression>().clone();
    let mut rng = world
        .resource::<SimSeed>()
        .stream(rng_key("sim_military::progression::boons") ^ id, tick);
    let choices = draw_dominion_boons(&mut rng, &data, &held);
    let posted = post_offer(
        world,
        FactionId::PLAYER,
        OfferKind::DominionBoon { region, step },
        choices,
        None,
    );
    debug_assert_eq!(posted, id, "the offer takes the id its draw used");
    let who = faction_name(world, FactionId::PLAYER);
    log(
        world,
        format!(
            "{who} holds {:.0}% of region {region}: a dominion boon is offered",
            threshold * 100.0
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::rng::rng_key;

    fn data() -> DominionData {
        DominionData {
            thresholds: vec![0.25, 0.5],
            per_captain: 300,
            cap: 8,
            first_village_slots: 1,
        }
    }

    /// GP-25: the slot formula. Nothing held gives nothing; the first
    /// village gives its slot before 300 people do; then one per 300,
    /// plus boon slots and the flat modifier, capped at 8 and never
    /// negative.
    #[test]
    fn captain_slots_follow_the_population_held() {
        let d = data();
        assert_eq!(slots_for(&d, 0, 0, 0, 0.0), 0);
        assert_eq!(slots_for(&d, 0, 0, 1, 0.0), 1, "a slot boon with no land");
        assert_eq!(slots_for(&d, 1, 40, 0, 0.0), 1, "the first village");
        assert_eq!(slots_for(&d, 2, 299, 0, 0.0), 1);
        assert_eq!(slots_for(&d, 2, 600, 0, 0.0), 2);
        assert_eq!(slots_for(&d, 3, 899, 0, 0.0), 2, "floor(899 / 300)");
        assert_eq!(slots_for(&d, 3, 900, 1, 0.0), 4, "plus a boon");
        assert_eq!(slots_for(&d, 3, 900, 1, 0.999_9), 5, "a whole modifier");
        assert_eq!(slots_for(&d, 3, 900, 1, 0.5), 4, "half a slot is none");
        assert_eq!(slots_for(&d, 1, 100, 0, -3.0), 0, "never negative");
        assert_eq!(slots_for(&d, 20, 4_000, 0, 0.0), 8, "13 capped at 8");
        assert_eq!(slots_for(&d, 20, 4_000, 3, 2.0), 8);
        assert_eq!(slots_for(&d, 1, 100, 0, f32::NAN), 1);
        let generous = DominionData {
            first_village_slots: 3,
            ..data()
        };
        assert_eq!(slots_for(&generous, 1, 650, 0, 0.0), 3, "the larger rule");
    }

    /// GP-24: holdings multiply the weight of the boons that favour them,
    /// and favoured boons come up more often.
    #[test]
    fn holdings_weigh_the_dominion_draw() {
        let p = Progression::default_data();
        let favoured = |h: Holding| {
            p.data
                .boons
                .iter()
                .find(|b| b.when == BoonWhen::Dominion && b.favour.iter().any(|f| f.0 == h))
                .unwrap_or_else(|| panic!("a Dominion boon favours {h:?}"))
                .clone()
        };
        let none = Holdings::default();
        let rich = Holdings {
            farms: 6,
            smithy: true,
            largest: true,
            battles: 4,
            kills: 50,
        };
        for b in p.data.boons.iter().filter(|b| b.when == BoonWhen::Dominion) {
            assert_eq!(boon_weight(b, &none), b.weight, "{}: no holdings", b.name);
            let manual: f32 = b.favour.iter().map(|&(h, w)| w * rich.amount(h)).sum();
            assert!((boon_weight(b, &rich) - b.weight * (1.0 + manual)).abs() < 1e-6);
        }
        let granary = favoured(Holding::Farms);
        assert!(boon_weight(&granary, &rich) > granary.weight);
        let seat = favoured(Holding::LargestSettlement);
        let only_seat = Holdings {
            largest: true,
            ..none
        };
        assert!(boon_weight(&seat, &only_seat) > boon_weight(&seat, &none));

        // Over many streams: three distinct Dominion cards every time, and
        // holding the largest settlement makes "Seat of power" likelier.
        let count = |h: &Holdings| {
            let mut n = 0;
            for i in 0..400u64 {
                let mut rng = SimSeed(7).stream(rng_key("test::dominion") ^ i, 0);
                let cards = draw_dominion_boons(&mut rng, &p, h);
                assert_eq!(cards.len(), 3);
                let mut d = cards.clone();
                d.sort_unstable();
                d.dedup();
                assert_eq!(d.len(), 3, "distinct: {cards:?}");
                assert!(
                    cards
                        .iter()
                        .all(|&c| p.boon(c).unwrap().when == BoonWhen::Dominion)
                );
                n += cards.contains(&seat.id) as u32;
            }
            n
        };
        let (plain, held) = (count(&none), count(&only_seat));
        assert!(
            held > plain + 40,
            "Seat of power offered {held}/400 with the largest settlement, {plain}/400 without"
        );
    }

    /// GP-24: a region's share reaches a threshold exactly, and an empty
    /// region reaches nothing.
    #[test]
    fn a_tally_reaches_its_thresholds_exactly() {
        let t = RegionTally {
            total: 400,
            held_by_player: 100,
            flags: 0,
        };
        assert!(t.reaches(0.25));
        assert!(!t.reaches(0.5));
        assert_eq!(t.share(), 0.25);
        let below = RegionTally {
            held_by_player: 99,
            ..t
        };
        assert!(!below.reaches(0.25));
        assert!(!RegionTally::default().reaches(0.25));
        assert!(data().validate().is_ok());
        let nine: Vec<f32> = (1..=9).map(|i| i as f32 / 10.0).collect();
        for bad in [vec![0.5, 0.25], vec![0.0], vec![1.5], nine] {
            let d = DominionData {
                thresholds: bad.clone(),
                ..data()
            };
            assert!(d.validate().is_err(), "{bad:?}");
        }
        let d = DominionData {
            per_captain: 0,
            ..data()
        };
        assert!(d.validate().is_err());
        let d = DominionData {
            first_village_slots: 9,
            ..data()
        };
        assert!(d.validate().is_err());
    }
}
